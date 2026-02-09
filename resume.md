# PetToy — Session Resume (2026-02-09)

## Current State: ALL COMPILED, RUNNING, PUSHED
Latest commit: `4807cc9` — pushed to origin/master.

## What Was Done

### v0.2.0 Desktop Companion Overhaul (5 Batches — COMPLETE)
All 5 batches from the original plan were implemented and committed:
1. **Cursor Intelligence** — Moses Effect, personality-driven flee/chase/ignore
2. **New Behaviors** — Zoomies, Startled, Yawning with contagion mechanics
3. **Modes + AFK** — Work/Play/Zen/Chaos modes, F11 cycling, AFK escalation via GetLastInputInfo
4. **Click Interactions** — Left-click startle, right-click treats, double-click laser pointer
5. **Visual Flourishes** — Cat trails (per-cat ring buffer, LineList pipeline), cursor heatmap (64x64, R8Unorm)

### Flocking + Population Growth
- Boid-like cohesion (radius=120px, strength=8) and alignment (strength=5)
- Colony starts at 20 cats, grows ~2/sec after 5s delay up to 1000
- Debug overlay slider overrides natural growth (only on explicit change)

### Feature Batch
- **Parade detection** — 3+ cats walking same direction, follow-the-leader (40px spacing)
- **Heatmap avoidance** — Cats steer away from cursor hot zones
- **Edge affinity** — Work mode pushes cats toward screen edges
- **Window awareness** — Cats perch on desktop window titlebars (enumerate every 2s)
- **Cat names + tooltips** — Procedural names ("Professor Whiskers"), hover tooltip
- **Yarn ball toy** — Middle-click spawns/throws, cats chase and bat it
- **Trail color by mood** — green (walking), red (zoomies), blue (sleeping), etc.

### Sleeping Piles + Size Variation + Day/Night
- **Sleeping piles**: 3+ sleeping cats within 40px, breathing animation (±4% sin), wake cascade
- **Size variation**: kittens (0.6x) move 1.3x speed, chonkers (1.4x) 0.7x, laziness/energy shift
- **Day/night cycle**: golden dawn, neutral day, orange dusk, blue night + energy 0.4x-1.0x
- **Dead code cleanup**: removed util/pool, cat/animation, cat/personality stubs

### Bug Fixes
- **Minimize/restore bug**: Windows sends small non-zero resize during minimize animation. Fix: ignore resize < 200x200, pause sim when `is_minimized()` in both `about_to_wait` AND `RedrawRequested`, reset frame timer
- **Flickering/teleporting bug**: Debug `target_cat_count=20` fought population growth (target=1000) every frame. Fix: init to 1000, only sync on explicit slider change via `cat_count_changed` flag

## Git Log (Latest First)
```
4807cc9 Add sleeping piles, size variation, day/night cycle, and fix minimize/flicker bugs
bd8d27c Add mood-colored trails: trail color reflects behavior state
33fbf6b Add resume.md and suggestions.md for session continuity
9eef6ce Add parade detection, heatmap avoidance, edge affinity, window awareness, cat names with tooltips, and yarn ball toy
8eebfe1 Add boid-like flocking and gradual population growth
603ebcc Bump to v0.2.0, update docs for Desktop Companion Overhaul
729f856 Add visual flourishes: cat trails and cursor heatmap
1cf8ab5 Add click interactions: startle, treats, and laser pointer
cbd8aeb Add mode system (Work/Play/Zen/Chaos) with AFK escalation
9d62fa5 Add emergent behaviors: zoomies, startled, yawning with contagion
```

## What's Left (from suggestions.md)

### Not Yet Implemented
- [ ] Emotion particles (hearts, zzz, !, stars)
- [ ] Animated sprite frames (multi-frame walking, tail swish, blinking)
- [ ] Breeding & kittens
- [ ] Territory & scent marking
- [ ] System tray icon
- [ ] Save/load colony
- [ ] Startup with Windows
- [ ] Performance auto-scaling
- [ ] Config file (pettoy.toml)
- [ ] Multi-monitor support
- [ ] Notification reactions
- [ ] Audio (opt-in purring/meow)
- [ ] Enhanced laser pointer (butt-wiggle, glow trail)
- [ ] Feather wand toy

### Wired But Could Be Improved
- **Parade** — No visual distinction (same walking frame). Could add parade animation.
- **Window awareness** — Basic perching. Could add corners, dangling tails, peeking.
- **Yarn ball** — Rendered as small red dot (reuses frame=0). Could get own sprite.
- **Heatmap avoidance** — Could be more aggressive or per-cat sensitivity.
- **Cat names** — Only in debug overlay. Could add floating name tags.

## Build & Run
```bash
cargo build          # ~6s incremental
cargo run            # launches overlay
```
- **ESC** quit | **F11** mode cycle | **F12** debug overlay
- **Left-click** startle | **Right-click** treat | **Double-click** laser (5s)
- **Middle-click** yarn ball (spawn/throw)

## Behavior State Machine (13 states)
```
Idle, Walking, Running, Sleeping, Grooming,
ChasingMouse, FleeingCursor, ChasingCat, Playing,
Zoomies, Startled, Yawning, Parading
```
