# Changelog

All notable changes to PetToy will be documented in this file.

Format based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
This project uses [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

**Milestone 0 — Bootstrapping**
- Win32 transparent overlay window (DWM DirectComposition + WS_EX_TOOLWINDOW)
- Click-through via winit `set_cursor_hittest` + WS_EX_NOACTIVATE
- Always-on-top fullscreen borderless window via winit WindowLevel
- winit 0.30 ApplicationHandler event loop
- wgpu 27 DX12 backend with DirectComposition (`Dx12SwapchainKind::DxgiFromVisual`)
- Surface: Bgra8UnormSrgb format, PreMultiplied alpha, Fifo present mode
- `DwmExtendFrameIntoClientArea` for flash prevention on window creation
- `DwmSetWindowAttribute` to disable DWM border, rounded corners, and NC rendering

**Milestone 1 — Core Rendering**
- Full instanced cat rendering pipeline (CatPipeline: vertex/index/instance buffers)
- cat.wgsl shader: procedural SDF cat silhouettes with 3 poses (sitting, walking, sleeping)
- SDF shapes using signed distance fields: circles, ellipses, triangles, smooth-min blending
- Instance buffer upload from ECS with position interpolation (CatInstance::from_components)
- Single `draw_indexed` call for all cats, pre-allocated for 4096 instances
- Premultiplied alpha blending (src=One, dst=OneMinusSrcAlpha)

**Milestone 2 — ECS & Spatial**
- hecs ECS world with 1000 cats spawned at random positions
- Cat components: Position, PrevPosition, Velocity, CatState, Personality, Appearance, SpatialCell
- Spatial hash grid: 128px cells, 1024 table size, multiplicative hash, O(1) insert/query
- Movement system with velocity integration, friction (0.92/tick), and velocity snapping
- Screen bounds clamping with margin
- Fixed timestep game loop (60Hz sim, decoupled rendering, spiral-of-death clamp)
- ControlFlow::Poll for continuous rendering
- FrameStats: FPS counter with avg/min/max frame times, logged every 5 seconds

**Milestone 3 — Cat Behaviors**
- Behavior state machine: Idle, Walking, Running, Sleeping, Grooming, ChasingMouse
- Personality-weighted state transitions (laziness, energy affect idle/walk/run ratios)
- Mouse tracking via Win32 GetCursorPos (polled once per frame)
- Mouse chase behavior: cats within 200px notice cursor, curiosity affects chase chance
- Random walk direction on Walking/Running transitions
- Staggered initial timers to avoid synchronized behavior transitions
- Procedural cat color palette (8 cat-ish colors: tabby, gray, black, white, sienna, cream, etc.)
- Cat size variation (0.7x to 1.3x)

**Infrastructure**
- Win32 EnumWindows: enumerate visible desktop windows (position, size, title) — ready for Milestone 6
- Personality trait system stub — ready for Milestone 5
- Animation state struct — ready for Milestone 7
- Object pool utility — ready for future use
- ESC to quit (polled via GetAsyncKeyState since window is click-through)
