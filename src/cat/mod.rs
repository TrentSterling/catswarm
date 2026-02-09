use crate::ecs::components::*;
use glam::Vec2;

/// Spawn a batch of cats with randomized attributes.
pub fn spawn_cats(world: &mut hecs::World, count: usize, screen_w: f32, screen_h: f32) {
    let mut rng = fastrand::Rng::new();

    for _ in 0..count {
        let pos = Vec2::new(rng.f32() * screen_w, rng.f32() * screen_h);

        world.spawn((
            Position(pos),
            PrevPosition(pos),
            Velocity(Vec2::ZERO),
            CatState {
                state: BehaviorState::Idle,
                timer: rng.f32() * 3.0, // stagger initial timers
            },
            Personality {
                laziness: rng.f32(),
                energy: rng.f32(),
                curiosity: rng.f32(),
                skittishness: rng.f32(),
            },
            Appearance {
                color: random_cat_color(&mut rng),
                pattern: rng.u8(0..4),
                size: 0.6 + rng.f32() * 0.8, // 0.6x to 1.4x
            },
            SpatialCell(0),
            CatName(generate_cat_name(&mut rng)),
        ));
    }
}

/// Generate a procedural cat name from name parts.
fn generate_cat_name(rng: &mut fastrand::Rng) -> String {
    const PREFIXES: &[&str] = &[
        "", "", "", "", "", "Sir ", "Lady ", "Professor ", "Captain ",
        "Dr. ", "Little ", "Big ", "Lord ", "Princess ",
    ];
    const NAMES: &[&str] = &[
        "Whiskers", "Mittens", "Shadow", "Luna", "Mochi", "Noodle", "Biscuit",
        "Pepper", "Ginger", "Oreo", "Tofu", "Pickles", "Beans", "Nugget",
        "Waffles", "Muffin", "Cleo", "Felix", "Salem", "Ziggy",
        "Pumpkin", "Smokey", "Tiger", "Patches", "Boots", "Socks",
        "Marble", "Dusty", "Cinnamon", "Toffee", "Chai", "Latte",
        "Sprout", "Pixel", "Widget", "Byte", "Cookie", "Pretzel",
    ];
    const SUFFIXES: &[&str] = &[
        "", "", "", "", "", " Jr.", " III", " the Great", " McFluff",
    ];
    format!(
        "{}{}{}",
        PREFIXES[rng.usize(0..PREFIXES.len())],
        NAMES[rng.usize(0..NAMES.len())],
        SUFFIXES[rng.usize(0..SUFFIXES.len())],
    )
}

/// Generate a random cat-ish color (warm tones, grays, blacks, whites).
fn random_cat_color(rng: &mut fastrand::Rng) -> u32 {
    let palette: &[[u8; 3]] = &[
        [255, 165, 50],  // orange tabby
        [80, 80, 80],    // gray
        [30, 30, 30],    // black
        [240, 240, 235], // white
        [180, 130, 70],  // brown/sienna
        [255, 200, 150], // cream
        [100, 100, 110], // blue-gray
        [200, 100, 50],  // ginger
    ];
    let [r, g, b] = palette[rng.usize(0..palette.len())];
    (r as u32) << 24 | (g as u32) << 16 | (b as u32) << 8 | 0xFF
}
