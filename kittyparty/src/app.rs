//! Window, event loop and glue between the fetcher thread, the simulation,
//! the tray and the GPU.

use glam::Vec2;
use image::RgbaImage;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, Receiver};
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::platform::windows::WindowAttributesExtWindows;
use winit::window::{Window, WindowAttributes, WindowId, WindowLevel};

use crate::cards::{self, Card};
use crate::gpu::Gpu;
use crate::gpu_types::{Sprite, LAYER_HEART, LAYER_KITTY, LAYER_PAPER};
use crate::party::Party;
use crate::settings::{self, Settings};
use crate::win::{self, Tray, TrayCommand};

const BUNDLED_KITTY: &[u8] = include_bytes!("../assets/kitty.png");
/// Uploading a card builds mipmaps on this thread; cap it per frame.
const UPLOADS_PER_FRAME: usize = 2;

struct App {
    window: Option<Arc<Window>>,
    gpu: Option<Gpu>,
    party: Option<Party>,
    tray: Option<Tray>,
    origin: Vec2,
    data_dir: Option<PathBuf>,
    settings: Settings,
    rx: Option<Receiver<Card>>,
    held: Option<Card>,
    quit: Arc<AtomicBool>,
    fetch_paused: Arc<AtomicBool>,
    paused: bool,
    hotkey_was_down: bool,
    last: Option<Instant>,
    topmost_timer: f32,
    sprites: Vec<Sprite>,
}

impl App {
    fn new() -> Self {
        let data_dir = settings::data_dir();
        let settings = data_dir.as_deref().map(Settings::load).unwrap_or_default();
        Self {
            window: None,
            gpu: None,
            party: None,
            tray: None,
            origin: Vec2::ZERO,
            data_dir,
            settings,
            rx: None,
            held: None,
            quit: Arc::new(AtomicBool::new(false)),
            fetch_paused: Arc::new(AtomicBool::new(false)),
            paused: false,
            hotkey_was_down: false,
            last: None,
            topmost_timer: 0.0,
            sprites: Vec::with_capacity(crate::gpu::MAX_INSTANCES),
        }
    }

    /// Her own kitty.png if she dropped one in, otherwise the bundled one.
    fn load_kitty(&self) -> RgbaImage {
        let custom = self
            .data_dir
            .as_ref()
            .map(|d| d.join("kitty.png"))
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|b| image::load_from_memory(&b).ok());
        let img = custom.unwrap_or_else(|| image::load_from_memory(BUNDLED_KITTY).expect("bundled kitty.png"));
        cards::fit_square(&img)
    }

    fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
        self.fetch_paused.store(paused, Ordering::Relaxed);
        win::MENU_PAUSED.store(paused as u32, Ordering::Relaxed);
        if let Some(w) = &self.window {
            w.set_visible(!paused);
        }
        self.last = None;
    }

    fn sync_menu(&self) {
        win::MENU_RAIN.store(self.settings.rain_level as u32, Ordering::Relaxed);
        win::MENU_CLICK.store(self.settings.click_bursts as u32, Ordering::Relaxed);
    }

    fn save_settings(&self) {
        if let Some(d) = &self.data_dir {
            self.settings.save(d);
        }
        self.sync_menu();
    }

    fn handle_tray(&mut self, event_loop: &ActiveEventLoop) {
        let Some(cmd) = self.tray.as_ref().and_then(|t| t.poll()) else {
            return;
        };
        match cmd {
            TrayCommand::Quit => event_loop.exit(),
            TrayCommand::TogglePause => self.set_paused(!self.paused),
            TrayCommand::BurstNow => {
                if self.paused {
                    self.set_paused(false);
                }
                if let Some(p) = &mut self.party {
                    p.big_burst();
                }
            }
            TrayCommand::SetRain(level) => {
                self.settings.rain_level = level as usize;
                if let Some(p) = &mut self.party {
                    p.rain_level = self.settings.rain_level;
                }
                self.save_settings();
            }
            TrayCommand::ToggleClickBursts => {
                self.settings.click_bursts = !self.settings.click_bursts;
                if let Some(p) = &mut self.party {
                    p.click_bursts = self.settings.click_bursts;
                }
                self.save_settings();
            }
            TrayCommand::ToggleAutostart => win::set_autostart(!win::autostart_enabled()),
            TrayCommand::OpenFolder => {
                if let Some(d) = &self.data_dir {
                    win::open_folder(&d.join("My Cats"));
                }
            }
        }
    }

    /// Move freshly fetched cat cards onto the GPU.
    fn take_cards(&mut self) {
        let (Some(gpu), Some(party)) = (&self.gpu, &mut self.party) else {
            return;
        };
        for _ in 0..UPLOADS_PER_FRAME {
            let card = match self.held.take() {
                Some(c) => c,
                None => match self.rx.as_ref().and_then(|rx| rx.try_recv().ok()) {
                    Some(c) => c,
                    None => return,
                },
            };
            match party.place_card(card.pinned, false) {
                Some(layer) => gpu.upload_layer(layer, &card.rgba),
                None => {
                    // Every slot is mid-fall; try again next frame.
                    self.held = Some(card);
                    return;
                }
            }
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let monitor = event_loop
            .primary_monitor()
            .or_else(|| event_loop.available_monitors().next())
            .expect("no monitor found");
        let pos = monitor.position();
        // Not with_transparent: that adds WS_EX_LAYERED, which fights
        // DirectComposition. Start hidden so DWM never caches a white frame.
        let attrs = WindowAttributes::default()
            .with_title("KittyParty")
            .with_decorations(false)
            .with_visible(false)
            .with_resizable(false)
            .with_window_level(WindowLevel::AlwaysOnTop)
            .with_inner_size(monitor.size())
            .with_position(pos)
            .with_skip_taskbar(true)
            .with_no_redirection_bitmap(true);
        let window = Arc::new(event_loop.create_window(attrs).expect("failed to create window"));
        win::make_overlay(&window);
        self.origin = Vec2::new(pos.x as f32, pos.y as f32);

        let size = window.inner_size();
        let gpu = Gpu::new(window.clone());
        let mut party = Party::new(size.width as f32, size.height as f32, window.scale_factor() as f32);
        party.rain_level = self.settings.rain_level;
        party.click_bursts = self.settings.click_bursts;

        let kitty = self.load_kitty();
        gpu.upload_layer(LAYER_KITTY, &kitty);
        gpu.upload_layer(LAYER_HEART, &cards::make_heart());
        gpu.upload_layer(LAYER_PAPER, &cards::make_paper());
        for card in cards::fallback_cards(&kitty) {
            if let Some(layer) = party.place_card(false, true) {
                gpu.upload_layer(layer, &card);
            }
        }

        let icon = image::imageops::resize(&kitty, 32, 32, image::imageops::FilterType::Lanczos3);
        self.tray = Tray::new(icon.as_raw());
        self.sync_menu();

        // Cat pictures arrive in the background.
        let (tx, rx) = sync_channel::<Card>(2);
        let my_cats = self.data_dir.as_ref().map(|d| d.join("My Cats"));
        let (quit, paused) = (self.quit.clone(), self.fetch_paused.clone());
        let _ = std::thread::Builder::new()
            .name("cat-fetcher".into())
            .spawn(move || cards::run_fetcher(tx, my_cats, quit, paused));
        self.rx = Some(rx);

        window.set_visible(true);
        win::make_overlay(&window);
        event_loop.set_control_flow(ControlFlow::Poll);
        self.gpu = Some(gpu);
        self.party = Some(party);
        self.window = Some(window);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(g) = &mut self.gpu {
                    g.resize(size.width, size.height);
                }
                if let Some(p) = &mut self.party {
                    p.resize(size.width as f32, size.height as f32);
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.handle_tray(event_loop);

        let hotkey = win::hotkey_down();
        if hotkey && !self.hotkey_was_down {
            self.set_paused(!self.paused);
        }
        self.hotkey_was_down = hotkey;

        if self.paused {
            // Hidden: just keep an ear out for the tray and the hotkey.
            event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(100)));
            return;
        }
        event_loop.set_control_flow(ControlFlow::Poll);

        self.take_cards();

        let now = Instant::now();
        let dt = self.last.map(|t| (now - t).as_secs_f32()).unwrap_or(0.0).min(0.05);
        self.last = Some(now);

        self.topmost_timer -= dt;
        if self.topmost_timer <= 0.0 {
            self.topmost_timer = 5.0;
            if let Some(w) = &self.window {
                win::keep_on_top(w);
            }
        }

        if let Some(w) = &self.window {
            win::apply_styles(win::hwnd_of(w));
        }

        let (cx, cy) = win::cursor_pos();
        let cursor = Vec2::new(cx, cy) - self.origin;
        let (Some(party), Some(gpu)) = (&mut self.party, &mut self.gpu) else {
            return;
        };
        party.update(dt, cursor, win::left_button_down());
        party.sprites(&mut self.sprites);
        // Fifo present blocks until vsync, which paces this loop.
        gpu.draw(&self.sprites);
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        self.quit.store(true, Ordering::Relaxed);
        self.tray = None;
    }
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::new()?;
    let mut app = App::new();
    event_loop.run_app(&mut app)?;
    Ok(())
}
