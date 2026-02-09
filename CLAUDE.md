# PetToy - Project Guide

## What Is This
Transparent desktop toy for Windows. Spawns 1000+ procedural cats that roam the desktop, chase the mouse, interact with each other, sit on windows, and exhibit personality-driven behaviors. Built for maximum performance.

## Tech Stack
- **Language:** Rust (edition 2021)
- **Rendering:** wgpu 27 (DX12 backend, DirectComposition for transparency)
- **Windowing:** winit 0.30 with Win32 extensions for transparent click-through overlay
- **ECS:** hecs 0.10 (lightweight, archetypal)
- **Math:** glam 0.29 (SIMD-accelerated)
- **Spatial:** Custom spatial hash grid for neighbor queries
- **Platform:** `windows` crate 0.58 for Win32 interop

## Architecture Principles
- **Data-oriented design.** SoA over AoS. Think cache lines, not objects.
- **GPU-driven rendering.** Single instanced draw call for all cats. CPU uploads instance buffer per frame.
- **Fixed timestep simulation** (60Hz) with interpolated rendering. Decouple sim from render.
- **Spatial hash grid** over quadtree — uniform cell size, O(1) insert/query for neighbor lookups.
- **Behavior is state machines**, not behavior trees. Simpler, cache-friendlier, easier to debug.
- **No allocations in hot path.** Pre-allocate everything. Reuse buffers. Arena where needed.

## Current State
Milestones 0–3 are implemented and working:
- Transparent DX12 overlay with DirectComposition (`DxgiFromVisual`) + PreMultiplied alpha
- 1000 cats rendering via single instanced draw call with procedural SDF silhouettes (3 poses)
- ECS simulation: movement, friction, behavior state machines, mouse chasing
- Spatial hash grid rebuilt each tick (ready for Milestone 4 neighbor queries)
- ~47 FPS on RTX 5070 Ti at 4096x2160 (vsync)

## Build & Run
```bash
cargo run                      # dev build (opt-level 1 for playable perf)
cargo run --release            # release build (full LTO, stripped)
cargo test                     # run tests
RUST_LOG=info cargo run        # with debug logging
```
ESC to quit (polled via GetAsyncKeyState since window is click-through).

## Code Conventions
- `snake_case` for everything except types/traits (`PascalCase`)
- Systems are free functions: `fn system_name(world: &mut hecs::World, dt: f32)`
- Components are plain structs, `#[derive(Debug, Clone, Copy)]` where possible
- No `unwrap()` in production code — use `expect("reason")` or propagate errors
- Keep modules small. One concern per file.
- Comments explain *why*, not *what*

## Project Structure
```
src/
  main.rs          — entry point, event loop
  app.rs           — App state, init, frame orchestration
  ecs/
    mod.rs         — re-exports
    components.rs  — all ECS components
    systems/
      mod.rs       — system scheduling (tick order)
      movement.rs  — position/velocity integration, friction, bounds
      behavior.rs  — state machine transitions
      spatial.rs   — spatial hash update
      mouse.rs     — mouse tracking & chase logic
      interaction.rs — cat-to-cat interactions (stub, Milestone 4)
      window_aware.rs — Win32 window detection (stub, Milestone 6)
  render/
    mod.rs         — GpuState: device, surface, resize, render_frame
    pipeline.rs    — CatPipeline: render pipeline, buffers, draw
    instance.rs    — CatInstance: per-instance GPU data
    shaders/
      cat.wgsl     — procedural SDF cat shader (3 poses)
  spatial/
    mod.rs         — spatial hash grid implementation
  cat/
    mod.rs         — cat spawning, procedural color generation
    personality.rs — personality trait weights (stub, Milestone 5)
    animation.rs   — animation state struct (stub, Milestone 7)
  platform/
    mod.rs         — platform abstraction
    win32.rs       — Win32 overlay setup, mouse pos, ESC key, window enum
  util/
    mod.rs         — misc utilities
    pool.rs        — object pool / arena
```

## Key Technical Details

### Transparency
- DX12-only backend (Vulkan WSI doesn't support transparent composition on Windows)
- `Dx12SwapchainKind::DxgiFromVisual` for DirectComposition presentation
- `wgpu-types` crate needed as direct dep (Dx12SwapchainKind not re-exported from wgpu)
- Surface: Bgra8UnormSrgb, alpha_mode: PreMultiplied, PresentMode: Fifo
- Clear to (0,0,0,0) each frame for transparent background
- WS_EX_TOOLWINDOW + WS_EX_NOACTIVATE window styles
- DwmSetWindowAttribute to disable DWM border, corners, NC rendering

### Rendering
- Single quad (4 verts, 6 indices) shared by all cats via instanced rendering
- CatInstance: 24 bytes (position, size, color, frame, pad)
- Premultiplied alpha blending (src=One, dst=OneMinusSrcAlpha)
- Shader: 3 SDF poses (sitting, walking, sleeping) with smooth-min blending
- Max 4096 instances pre-allocated

### Simulation
- Fixed 60Hz timestep with interpolated rendering (accumulator + alpha lerp)
- Behavior state machine per cat: Idle, Walking, Running, Sleeping, Grooming, ChasingMouse
- Personality (laziness, energy, curiosity, skittishness) affects transition weights
- Spatial hash: 128px cells, 1024 table size, rebuilt each tick
- `fastrand::Rng` for behavior randomness (no allocation)

## Performance Targets
- 1000+ cats at 60 FPS on mid-range GPU
- < 2ms CPU frame time for simulation
- < 1ms for spatial hash rebuild
- Single draw call for all cats (instanced)
- Zero heap allocations per frame in steady state

## Key Perf Notes
- Spatial hash cell size = 2x largest cat interaction radius
- Instance buffer is persistent, write-only from CPU via queue.write_buffer
- Cats outside screen bounds are clamped (all simulate every tick currently)
- Personality values are f32 packed into components, no indirection
- Per-cat data ~60 bytes, 1000 cats = 60KB fits in L1 cache
