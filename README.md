# PetToy

Transparent desktop toy for Windows. Spawns hordes of procedurally generated cats that roam your desktop, chase your mouse, interact with each other, and perch on your windows.

Built in Rust with wgpu for GPU-accelerated rendering. Targets 1000+ simultaneous cats at 60 FPS.

## Current Features

- **Transparent overlay** — cats render on top of everything, click-through so they never block your work
- **Mass simulation** — 1000 cats via ECS architecture with spatial hashing
- **Instanced rendering** — all cats drawn in a single GPU draw call
- **Procedural SDF cats** — 3 poses (sitting, walking, sleeping) drawn with signed distance fields
- **Mouse chasing** — cats notice and chase your cursor
- **Behavior state machine** — idle, walk, run, sleep, groom, chase with personality-weighted transitions
- **Procedural generation** — each cat has unique color, size, and personality traits

## Planned Features

- Cat-to-cat interaction (playing, napping together, chasing)
- Window awareness (cats walk on title bars, sit on taskbar)
- Enhanced personality system
- Better procedural visuals and animation
- System tray with settings

## Requirements

- Windows 10/11
- GPU with DX12 support
- Rust toolchain (1.75+)

## Build

```bash
cargo run --release
```

Press ESC to quit.

## License

MIT
