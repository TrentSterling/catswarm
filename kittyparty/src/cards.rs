//! Turns cat pictures into little polaroid-style cards, and fetches them.
//!
//! Everything here is CPU-side and runs on a background thread so a slow
//! network never stutters the animation. Finished cards travel to the render
//! thread over a channel as ready-to-upload RGBA pixels.

use image::{imageops::FilterType, DynamicImage, RgbaImage};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::SyncSender;
use std::sync::Arc;
use std::time::Duration;

/// Edge length of every texture layer (sprite, heart, cards).
pub const TEX: u32 = 256;

/// A finished card, ready for upload.
pub struct Card {
    pub rgba: RgbaImage,
    /// Her own pictures stay loaded; internet ones rotate out over time.
    pub pinned: bool,
}

/// Signed distance to a rounded rectangle centred at `c` with half extents `h`.
fn rounded_rect_sd(px: f32, py: f32, c: f32, h: f32, r: f32) -> f32 {
    let qx = (px - c).abs() - (h - r);
    let qy = (py - c).abs() - (h - r);
    let ox = qx.max(0.0);
    let oy = qy.max(0.0);
    (ox * ox + oy * oy).sqrt() + qx.max(qy).min(0.0) - r
}

fn coverage(sd: f32) -> f32 {
    (0.5 - sd).clamp(0.0, 1.0)
}

/// Centre-crop to a square and frame it on a white rounded card.
pub fn make_card(img: &DynamicImage, frame: [u8; 3]) -> RgbaImage {
    let (w, h) = (img.width(), img.height());
    let side = w.min(h).max(1);
    let photo = img
        .crop_imm((w - side) / 2, (h - side) / 2, side, side)
        .resize_exact(TEX - 28, TEX - 28, FilterType::Triangle)
        .to_rgba8();

    let mut out = RgbaImage::new(TEX, TEX);
    let c = TEX as f32 / 2.0;
    for y in 0..TEX {
        for x in 0..TEX {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let card_a = coverage(rounded_rect_sd(px, py, c, c - 1.0, 22.0));
            if card_a <= 0.0 {
                continue;
            }
            let photo_a = coverage(rounded_rect_sd(px, py, c, c - 14.0, 12.0));
            let mut rgb = [frame[0] as f32, frame[1] as f32, frame[2] as f32];
            if photo_a > 0.0 {
                let p = photo.get_pixel(x.saturating_sub(14).min(TEX - 29), y.saturating_sub(14).min(TEX - 29));
                // Photos with transparency sit on the frame colour.
                let pa = photo_a * p[3] as f32 / 255.0;
                for i in 0..3 {
                    rgb[i] = rgb[i] * (1.0 - pa) + p[i] as f32 * pa;
                }
            }
            out.put_pixel(
                x,
                y,
                image::Rgba([rgb[0] as u8, rgb[1] as u8, rgb[2] as u8, (card_a * 255.0) as u8]),
            );
        }
    }
    out
}

/// Fit any picture into a square transparent layer without cropping (used for
/// the kitty sprite, so a custom kitty.png of any shape works).
pub fn fit_square(img: &DynamicImage) -> RgbaImage {
    let fitted = img.resize(TEX, TEX, FilterType::Lanczos3).to_rgba8();
    let mut out = RgbaImage::new(TEX, TEX);
    let ox = (TEX - fitted.width()) / 2;
    let oy = TEX - fitted.height(); // stand on the bottom edge
    image::imageops::overlay(&mut out, &fitted, ox as i64, oy as i64);
    out
}

/// A soft heart for the little love bubbles the kitty gives off.
pub fn make_heart() -> RgbaImage {
    let mut out = RgbaImage::new(TEX, TEX);
    for y in 0..TEX {
        for x in 0..TEX {
            // Classic implicit heart, scaled into the layer.
            let u = (x as f32 / TEX as f32 - 0.5) * 2.6;
            let v = -(y as f32 / TEX as f32 - 0.55) * 2.6;
            let f = (u * u + v * v - 1.0).powi(3) - u * u * v * v * v;
            let a = (-f * 40.0 + 0.5).clamp(0.0, 1.0);
            out.put_pixel(x, y, image::Rgba([255, 255, 255, (a * 255.0) as u8]));
        }
    }
    out
}

/// A plain white rounded rectangle, tinted per instance for paper confetti.
pub fn make_paper() -> RgbaImage {
    let mut out = RgbaImage::new(TEX, TEX);
    let c = TEX as f32 / 2.0;
    for y in 0..TEX {
        for x in 0..TEX {
            let a = coverage(rounded_rect_sd(x as f32 + 0.5, y as f32 + 0.5, c, c - 2.0, 40.0));
            out.put_pixel(x, y, image::Rgba([255, 255, 255, (a * 255.0) as u8]));
        }
    }
    out
}

const FRAMES: [[u8; 3]; 6] = [
    [255, 255, 255],
    [255, 228, 238],
    [255, 246, 214],
    [226, 243, 255],
    [234, 255, 236],
    [243, 232, 255],
];

pub fn random_frame() -> [u8; 3] {
    FRAMES[fastrand::usize(..FRAMES.len())]
}

/// Cards drawn from the kitty sprite, so there is something to rain even
/// with no internet and no pictures of her own.
pub fn fallback_cards(kitty: &RgbaImage) -> Vec<RgbaImage> {
    let bgs: [[u8; 3]; 4] = [[255, 205, 225], [200, 230, 255], [255, 240, 190], [215, 245, 215]];
    bgs.iter()
        .map(|bg| {
            let mut pic = RgbaImage::from_pixel(TEX, TEX, image::Rgba([bg[0], bg[1], bg[2], 255]));
            let small = image::imageops::resize(kitty, 200, 200, FilterType::Triangle);
            image::imageops::overlay(&mut pic, &small, 28, 40);
            make_card(&DynamicImage::ImageRgba8(pic), [255, 255, 255])
        })
        .collect()
}

fn decode(bytes: &[u8]) -> Option<DynamicImage> {
    image::load_from_memory(bytes).ok()
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(8))
        .timeout(Duration::from_secs(20))
        .user_agent("KittyParty/0.1 (desktop toy)")
        .build()
}

fn get_bytes(agent: &ureq::Agent, url: &str) -> Option<Vec<u8>> {
    let resp = agent.get(url).call().ok()?;
    let mut buf = Vec::new();
    use std::io::Read;
    // Cap downloads so one huge GIF can't eat memory.
    resp.into_reader().take(12 * 1024 * 1024).read_to_end(&mut buf).ok()?;
    Some(buf)
}

/// One batch of cat picture URLs from The Cat API.
fn catapi_urls(agent: &ureq::Agent) -> Vec<String> {
    let Some(body) = get_bytes(agent, "https://api.thecatapi.com/v1/images/search?limit=10&mime_types=jpg,png") else {
        return Vec::new();
    };
    let Ok(json) = serde_json::from_slice::<serde_json::Value>(&body) else {
        return Vec::new();
    };
    json.as_array()
        .map(|a| a.iter().filter_map(|v| v["url"].as_str().map(String::from)).collect())
        .unwrap_or_default()
}

fn is_picture(p: &Path) -> bool {
    matches!(
        p.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref(),
        Some("png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp")
    )
}

/// Background loop: her own pictures first, then the internet, forever.
pub fn run_fetcher(tx: SyncSender<Card>, my_cats: Option<PathBuf>, quit: Arc<AtomicBool>, paused: Arc<AtomicBool>) {
    // Her own pictures first. Capped so a giant photo folder can't take over.
    if let Some(dir) = &my_cats {
        if let Ok(entries) = std::fs::read_dir(dir) {
            let mut files: Vec<PathBuf> = entries.flatten().map(|e| e.path()).filter(|p| is_picture(p)).collect();
            fastrand::shuffle(&mut files);
            for path in files.into_iter().take(24) {
                if quit.load(Ordering::Relaxed) {
                    return;
                }
                if let Some(img) = std::fs::read(&path).ok().and_then(|b| decode(&b)) {
                    if tx.send(Card { rgba: make_card(&img, random_frame()), pinned: true }).is_err() {
                        return;
                    }
                }
            }
        }
    }

    let agent = agent();
    let mut fetched = 0u32;
    let mut failures = 0u32;
    while !quit.load(Ordering::Relaxed) {
        if paused.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(500));
            continue;
        }
        let mut urls = catapi_urls(&agent);
        if urls.is_empty() {
            // Second source if The Cat API is down.
            urls.push(format!("https://cataas.com/cat?width=400&t={}", fastrand::u32(..)));
        }
        let mut got_any = false;
        for url in urls {
            if quit.load(Ordering::Relaxed) {
                return;
            }
            let Some(img) = get_bytes(&agent, &url).and_then(|b| decode(&b)) else {
                continue;
            };
            got_any = true;
            fetched += 1;
            if tx.send(Card { rgba: make_card(&img, random_frame()), pinned: false }).is_err() {
                return;
            }
            // Fill up quickly at first, then trickle in fresh cats for variety.
            let pause = if fetched < 40 { 300 } else { 20_000 };
            sleep_unless(&quit, pause);
        }
        failures = if got_any { 0 } else { (failures + 1).min(6) };
        if failures > 0 {
            log::info!("cat fetch failed ({failures} in a row), backing off");
            sleep_unless(&quit, 5_000 * (1 << failures));
        }
    }
}

fn sleep_unless(quit: &AtomicBool, ms: u64) {
    let mut left = ms;
    while left > 0 && !quit.load(Ordering::Relaxed) {
        let step = left.min(250);
        std::thread::sleep(Duration::from_millis(step));
        left -= step;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn card_has_transparent_corners_and_opaque_middle() {
        let img = DynamicImage::ImageRgba8(RgbaImage::from_pixel(640, 480, image::Rgba([10, 200, 30, 255])));
        let card = make_card(&img, [255, 255, 255]);
        assert_eq!(card.dimensions(), (TEX, TEX));
        assert_eq!(card.get_pixel(0, 0)[3], 0);
        let mid = card.get_pixel(TEX / 2, TEX / 2);
        assert_eq!(mid[3], 255);
        assert!(mid[1] > 150 && mid[0] < 50);
        // Frame is white.
        let edge = card.get_pixel(TEX / 2, 6);
        assert_eq!((edge[0], edge[1], edge[2]), (255, 255, 255));
    }

    #[test]
    fn fit_square_keeps_tall_images_whole() {
        let img = DynamicImage::ImageRgba8(RgbaImage::from_pixel(100, 400, image::Rgba([1, 2, 3, 255])));
        let out = fit_square(&img);
        assert_eq!(out.dimensions(), (TEX, TEX));
        assert_eq!(out.get_pixel(TEX / 2, 2)[3], 255);
        assert_eq!(out.get_pixel(2, TEX / 2)[3], 0);
    }

    #[test]
    fn heart_is_filled_in_the_middle() {
        let h = make_heart();
        assert!(h.get_pixel(TEX / 2, TEX / 2)[3] > 200);
        assert_eq!(h.get_pixel(1, 1)[3], 0);
    }
}
