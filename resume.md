# PetToy — Session Resume (2026-02-09)

## What Was Done This Session

### v0.2.0 Desktop Companion Overhaul (5 Batches — COMPLETE)
All 5 batches from the original plan were implemented and committed:
1. **Cursor Intelligence** — Moses Effect, personality-driven flee/chase/ignore
2. **New Behaviors** — Zoomies, Startled, Yawning with contagion mechanics
3. **Modes + AFK** — Work/Play/Zen/Chaos modes, F11 cycling, AFK escalation via GetLastInputInfo
4. **Click Interactions** — Left-click startle, right-click treats, double-click laser pointer
5. **Visual Flourishes** — Cat trails (per-cat ring buffer, LineList pipeline), cursor heatmap (64x64, R8Unorm)

### Flocking + Population Growth
- Added boid-like cohesion (radius=120px, strength=8) and alignment (strength=5) to the existing separation system
- Colony starts at 20 cats, grows ~2/sec after 5s delay up to 1000
- Debug overlay slider overrides natural growth

### Feature Batch (Latest Commit)
All implemented and compiling:
- **Parade detection** — 3+ cats walking same direction form conga lines (interaction.rs)
- **Heatmap avoidance** — Cats steer away from cursor hot zones (movement.rs)
- **Edge affinity** — Work mode pushes cats toward screen edges (movement.rs)
- **Window awareness** — Cats perch on desktop window titlebars (window_aware.rs, enumerate every 2s)
- **Cat names + tooltips** — Procedural names ("Professor Whiskers"), hover tooltip in debug overlay
- **Yarn ball toy** — Middle-click spawns/throws, cats chase and bat it, rendered as red dot
- **Middle-click detection** — Added to Win32 input polling

## Git Log (Latest First)
```
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
- [ ] Animated sprite frames (multi-frame walking, tail swish, blinking)
- [ ] Size variation beyond Appearance.size (kittens vs chonkers behavior)
- [ ] Sleeping piles (visual merge when 3+ cats sleep together)
- [ ] Day/night cycle (system clock tinting)
- [ ] Emotion particles (hearts, zzz, !, etc.)
- [ ] Trail color by mood
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
- [ ] Cat economy
- [ ] Seasonal events
- [ ] Personality archetypes v2
- [ ] Screenshot mode
- [ ] Achievements

### Wired But Could Be Improved
- **Parade detection** — Works but no visual distinction (same walking frame). Could add parade-specific animation.
- **Window awareness** — Basic perching on titlebars. Could add sitting on corners, dangling tails, peeking over browser tabs.
- **Yarn ball** — Rendered as a small red cat silhouette (reuses frame=0). Could get its own shader/sprite.
- **Heatmap avoidance** — Uses gradient sampling. Could be more aggressive or have per-cat sensitivity.
- **Cat names** — Only visible in debug overlay. Could add floating name tags option.

## Architecture Notes for Resuming

### Key Files Modified This Session
| File | What Changed |
|------|-------------|
| `src/ecs/components.rs` | Added `Parading` state, `CatName` component |
| `src/ecs/systems/interaction.rs` | Flocking buffers, parade detection+application |
| `src/ecs/systems/movement.rs` | Heatmap avoidance, edge affinity |
| `src/ecs/systems/mod.rs` | Expanded tick() with heatmap, edge_affinity, platforms params |
| `src/ecs/systems/window_aware.rs` | Full implementation with DesktopWindow struct |
| `src/ecs/systems/click.rs` | Yarn ball chase + bat logic |
| `src/ecs/systems/behavior.rs` | Parading state handling |
| `src/cat/mod.rs` | Name generation, CatName in spawn |
| `src/click.rs` | Middle-click support |
| `src/toy.rs` | **NEW** — Yarn ball physics |
| `src/platform/win32.rs` | Middle-click detection |
| `src/debug/mod.rs` | HoveredCatInfo, tooltip rendering |
| `src/app.rs` | Wired everything: yarn ball, platforms, tooltips, heatmap/edge params |
| `suggestions.md` | **NEW** — Full feature roadmap |

### Build & Run
```bash
cargo build          # ~6s incremental
cargo run            # launches overlay
```
- **ESC** quit | **F11** mode cycle | **F12** debug overlay
- **Left-click** startle | **Right-click** treat | **Double-click** laser (5s)
- **Middle-click** yarn ball (spawn/throw)

### Behavior State Machine (12 states)
```
Idle, Walking, Running, Sleeping, Grooming,
ChasingMouse, FleeingCursor, ChasingCat, Playing,
Zoomies, Startled, Yawning, Parading
```

### Performance at 4096 Cats (4K Display)
- ~560-630 FPS
- Interaction system is main bottleneck (78% of CPU time)
- Total CPU: ~2.8ms per frame
