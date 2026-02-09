# PetToy — Feature Suggestions & Roadmap

## Status Legend
- [ ] Not started
- [~] In progress
- [x] Complete

---

## Low-Hanging Fruit (Stubbed / Partially Built)

### [~] Window Awareness (Milestone 6)
Cats interact with real desktop windows — walk along titlebars, sit on edges, peek over browser tabs, dangle tails off corners. `enumerate_windows()` and `WindowRect` already exist in `win32.rs`. Needs: periodic window enumeration, collision/platform detection in movement system, perching behavior state.

### [ ] Parade Detection
3+ cats walking in similar direction within 100px form a conga line. Leader keeps direction, followers steer to maintain ~40px spacing behind the cat in front. Constants (`PARADE_MIN_CATS`, `PARADE_RADIUS`, etc.) already defined in `interaction.rs`. Needs: detection in phase_read, steering in phase_write, new `Parading` behavior state.

### [ ] Heatmap Avoidance
Cats organically avoid areas where you work. `Heatmap::sample()` exists but isn't called from the movement system. Needs: sample heatmap at candidate movement direction, bias away from hot zones (>0.3 intensity). Creates natural "I work here" clear zones.

### [ ] Edge Affinity (Work Mode)
In Work mode, cats bias their walking direction toward screen edges, clearing the center. `ModeState::edge_affinity` is computed but not applied in `movement.rs`. Needs: blend walking direction toward nearest edge by `edge_affinity` factor when picking new direction.

---

## Interaction & Gameplay

### [ ] Cat Names & Tooltips
Each cat gets a procedurally generated name (e.g., "Professor Whiskers", "Captain Fluffbutt", "Mochi", "Sir Naps-a-Lot"). Hovering over a cat shows a floating tooltip with name, personality radar, current mood/behavior, and age. Needs: name generation on spawn, hit-testing cursor against cat positions, egui tooltip or custom text rendering.

### [ ] Toys — Yarn Ball
A draggable yarn ball that bounces around the screen. Cats within range chase it, bat at it (applying impulse). Physics: simple velocity + gravity + bounce off screen edges. Right-click to spawn, left-click-drag to throw. Leaves a yarn trail behind it.

### [ ] Toys — Feather Wand
A feather that dangles from the cursor on a "string" (simple pendulum physics). Cats jump at it when it swings low. The feather bobs and sways with cursor movement. More engaging than plain cursor chase.

### [ ] Toys — Laser Pointer (Enhanced)
Already have basic laser (double-click, 5s timer). Enhance: red dot rendered as a small glowing circle, cats do the classic butt-wiggle before pouncing, laser leaves fading trail, cats look confused when it disappears.

### [ ] Breeding & Kittens
Two cats that spend >30s playing together spawn a kitten with blended personality traits (average of parents ± mutation). Kittens are half-size, move 1.5x speed, have higher energy. Grow to full size over 2 minutes. Population cap still applies.

### [ ] Territory & Scent Marking
Cats claim screen regions based on where they spend time. Territorial cats (low friendliness) hiss at intruders. Scent fades over time (separate heatmap per "clan"). Creates emergent turf wars and neighborhoods.

---

## Visual Upgrades

### [ ] Animated Sprite Frames
Replace static SDF poses with multi-frame animations: walking legs cycle, tail swishes while idle, eyes blink randomly, ears twitch. Could remain SDF-based (parameterize limb positions) or switch to sprite atlas.

### [ ] Size Variation
Kittens (0.6x), normal (1.0x), chonkers (1.4x). Size affects speed (inverse), laziness (proportional), and visual presence. Chonkers are slower but have stronger separation force. Kittens dart around.

### [ ] Sleeping Piles
When 3+ cats sleep within 20px of each other, they visually merge into a "pile" — overlapping silhouettes, gentle breathing animation (scale oscillation), zzz particles. Disturbing one wakes them all (startled cascade).

### [ ] Day/Night Cycle
Tint overlay based on system clock. Dawn (6-8am): cats wake up, stretch, increased energy. Day: normal. Dusk (6-8pm): yawning increases, sleep clusters form. Night (10pm-6am): most cats sleeping, occasional nocturnal zoomies. Subtle warm/cool color shift on cat colors.

### [ ] Emotion Particles
Tiny floating symbols above cats: hearts (playing), zzz (sleeping), ! (startled), ~ (grooming), stars (zoomies), ? (confused/looking for laser). Rendered as simple SDF shapes in the cat shader or as a separate particle pass.

### [ ] Trail Color by Mood
Trail color reflects behavior: white (idle), green (walking), blue (sleeping/grooming), red (zoomies/running), pink (playing/chasing), yellow (startled). Creates a mood map of recent activity.

---

## System & Polish

### [ ] System Tray Icon
Proper Windows system tray integration. Right-click menu: mode selector, cat count slider, toggle trails/heatmap, toggle click-through, pause, about, quit. Removes dependency on ESC-only exit and F-key hotkeys.

### [ ] Save/Load Colony
Persist cat personalities, names, positions, and behavior states to a JSON/binary file. Auto-save every 5 minutes. Load on startup. Your colony is *yours* — same cats every day, growing and evolving.

### [ ] Startup with Windows
Optional registry entry (`HKCU\Software\Microsoft\Windows\CurrentVersion\Run`) to launch PetToy on login. Toggle from system tray menu. Start minimized to tray.

### [ ] Performance Auto-Scaling
If FPS drops below target (30), gradually reduce cat count. If FPS is well above target, allow growth. Adaptive quality: disable trails/heatmap at low FPS. Report performance class in debug overlay.

### [ ] Config File
`pettoy.toml` for persistent settings: default mode, cat count target, growth rate, visual toggles, hotkey bindings, monitor preference. Loaded on startup, saved on change.

---

## Wild / Future Ideas

### [ ] Multi-Monitor Support
Cats walk between monitors, jumping across the gap. Detect monitor layout via `EnumDisplayMonitors`. Cats on secondary monitors are slightly more adventurous (exploring new territory).

### [ ] Notification Reactions
Detect Windows toast notifications (via accessibility API or window detection). Cats nearest to the notification look at it, some bat at it. Dismissing it scatters nearby cats.

### [ ] Audio (Opt-In)
Faint ambient purring when cats cluster (volume scales with cluster size). Soft meow on startle. Tiny pitter-patter when many cats run. All optional, default off. Use `rodio` or `cpal` for low-latency audio.

### [ ] Cat Economy
"Fish" currency accumulates over time (1/min while app runs). Spend fish on: treats (already free, could cost fish), new toys, cosmetic items (hats, bowties), speed up breeding. Encourages daily check-ins.

### [ ] Seasonal Events
Holiday-themed behavior: Santa hats in December, pumpkin cats in October, heart particles on Valentine's Day. Cherry blossom particles in spring. Snowfall in winter (cats try to catch snowflakes).

### [ ] Cat Personalities v2
Named personality archetypes beyond just trait weights: "The Scholar" (sits on book-like windows, curious), "The Athlete" (zoomies champion, high energy), "The Diva" (demands attention, startles dramatically), "The Zen Master" (always near sleep piles, calms others).

### [ ] Screenshot Mode
Hotkey to hide debug overlay, pause simulation, and take a transparent PNG screenshot of just the cats. Share-worthy desktop art. Could also record GIF of last 5 seconds.

### [ ] Achievements
Track milestones: "First Zoomies", "100 Cats", "Sleep Pile of 10", "Moses Parting", "Full Parade". Show as toast notifications. Adds a sense of progression.

### [ ] Community Cats
Export/import individual cats (personality + name + color) as small JSON blobs. Trade cats with friends. "Adopt" community-made cats from a shared repository.
