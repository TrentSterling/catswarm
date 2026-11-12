# PetToy — TODO

## Milestone 0: Bootstrapping ✅
- [x] Win32 transparent, always-on-top, click-through window
- [x] wgpu device/surface init on transparent window
- [x] Clear to transparent + render a single quad (proof of life)
- [x] Basic event loop with fixed timestep

## Milestone 1: Core Rendering ✅
- [x] Instanced sprite rendering pipeline (vertex + instance buffers)
- [x] Write cat quad shader (.wgsl) — procedural SDF cat silhouettes (3 poses)
- [x] Instance buffer upload path (CPU -> GPU each frame)
- [x] Batch 1000 quads in a single draw call
- [x] Frame timing / delta time tracking

## Milestone 2: ECS & Spatial ✅
- [x] Set up hecs world with basic components (Position, Velocity, CatState)
- [x] Spatial hash grid implementation (insert, query neighbors, clear+rebuild)
- [x] Benchmark spatial hash with 1000+ entities
- [x] Movement system (integrate velocity, apply friction/damping)
- [x] Screen bounds clamping / wrapping

## Milestone 3: Cat Behaviors (State Machine) ✅
- [x] Define cat states: Idle, Walking, Running, Sleeping, Grooming, Chasing, Playing
- [x] State machine transitions with cooldowns
- [x] Mouse tracking system (global mouse pos via Win32)
- [x] Mouse chase behavior (cats near cursor get attracted)
- [x] Idle behaviors (random walk, sit, sleep after timeout)

## Milestone 4: Cat-to-Cat Interaction ✅
- [x] Neighbor query via spatial hash
- [x] Cats play with nearby cats (interaction.rs + contagion mechanics)
- [x] Cats nap together (yawning contagion spreads to nearby idle cats at 30%)
- [x] Chase/flee interactions between cats (ChasingCat + FleeingCursor states)
- [x] Social distance — Moses Effect scatters crowded cats

## Milestone 5: Personality System ✅
- [x] Personality component (laziness, energy, curiosity, skittishness) as f32s
- [x] Personality affects state transition weights
- [x] Personality affects movement speed, idle duration, chase eagerness
- [ ] Procedural name generation (optional/fun)

## Milestone 6: Window Awareness
- [ ] Enumerate desktop windows via Win32 (position, size)
- [ ] Cats walk on window title bars
- [ ] Cats sit on taskbar
- [ ] Cats "knock things off" edges (visual gag animation)
- [ ] Refresh window list periodically (not every frame)

## Milestone 7: Procedural Cat Visuals
- [ ] Decide art approach (procedural shapes vs pixel art vs skeletal)
- [ ] Procedural color/pattern generation per cat
- [ ] Animation frames for each state
- [ ] Cat size variation
- [ ] Rendering LOD for distant/small cats

## Milestone 8: Polish & Optimization
- [ ] Profiling pass — identify and fix hotspots
- [ ] GPU profiling — ensure single draw call holds
- [ ] Memory audit — verify zero steady-state allocations
- [ ] System tray icon with right-click menu (spawn more, quit, settings)
- [ ] Config file (cat count, behavior weights, performance settings)
- [ ] Startup on boot option (registry)

## Backlog / Ideas
- [ ] Cats react to window drag/resize
- [ ] Seasonal themes (santa hats in december)
- [ ] Rare cat variants (golden cat, tiny cat, chonk cat)
- [ ] Sound effects (tiny meows, purring) — opt-in only
- [ ] Multi-monitor support
- [ ] Cat stats overlay (debug mode)
- [ ] Compute shader simulation path (move sim to GPU entirely)
