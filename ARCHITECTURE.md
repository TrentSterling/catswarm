# Architecture

## High-Level Overview

```
┌─────────────────────────────────────────────┐
│                 Event Loop                   │
│  (winit) fixed timestep + interpolated render│
├──────────────┬──────────────────────────────┤
│   Simulate   │         Render               │
│   (60 Hz)    │    (vsync / uncapped)        │
├──────────────┤──────────────────────────────┤
│ ECS Systems  │  Instance Buffer Upload      │
│  - Movement  │  Single Instanced Draw Call  │
│  - Behavior  │  wgpu Pipeline               │
│  - Spatial   │                              │
│  - Mouse     │                              │
│  - Interact  │                              │
│  - Windows   │                              │
├──────────────┴──────────────────────────────┤
│              Spatial Hash Grid               │
│   O(1) insert, O(1) neighbor query          │
├─────────────────────────────────────────────┤
│         Platform Layer (Win32)               │
│  Transparent window, click-through,          │
│  window enumeration, global mouse pos        │
└─────────────────────────────────────────────┘
```

## Frame Timeline

```
Frame N:
  1. Poll input events (winit)
  2. Accumulate time into fixed-step budget
  3. While budget >= TICK_RATE (1/60s):
     a. Run ECS systems (simulate)
     b. Rebuild spatial hash
     c. Decrement budget
  4. Compute interpolation alpha = budget / TICK_RATE
  5. Build instance buffer (lerp positions by alpha)
  6. Upload instance buffer to GPU
  7. Submit render pass (single instanced draw)
  8. Present
```

## Spatial Hash Grid

Grid of cells, each cell is a `Vec<Entity>` (pre-allocated).

- **Cell size:** 2x interaction radius (typically 64-128px)
- **Insert:** `cell[hash(x/cell_size, y/cell_size)].push(entity)`
- **Query neighbors:** check 9 cells (self + 8 surrounding)
- **Rebuild every sim tick** — clear all cells, re-insert all entities
- Hash function: `(x_cell * 73856093) ^ (y_cell * 19349663)` mod table_size

Why spatial hash over quadtree:
- Simpler, fewer cache misses
- O(1) insert vs O(log n)
- Better for uniform-ish distribution (cats on a 2D desktop)
- Quadtrees shine for clustered/hierarchical data — not our use case

## ECS Layout (hecs)

Components are small, Copy types. No heap indirection in components.

| Component      | Fields                              | Size  |
|---------------|-------------------------------------|-------|
| Position       | Vec2 (x, y)                        | 8B    |
| PrevPosition   | Vec2 (for interpolation)           | 8B    |
| Velocity       | Vec2 (vx, vy)                      | 8B    |
| CatState       | enum (u8) + timer (f32)            | 8B    |
| Personality    | 4x f32 (lazy, energy, curious, shy)| 16B   |
| Appearance     | color (u32), pattern (u8), size (f32)| 8B  |
| SpatialCell    | cell index (u32)                    | 4B    |

Total per cat: ~60 bytes. 1000 cats = 60KB. Fits in L1 cache.

## Rendering Pipeline

- **Vertex buffer:** single quad (4 verts, 6 indices) — shared by all cats
- **Instance buffer:** per-cat data (position, size, color, animation frame) — 24 bytes per instance
- **Shader:** vertex shader reads instance data, fragment shader draws procedural SDF cat silhouettes
- **SDF shapes:** 3 poses (sitting, walking, sleeping) built from circles, ellipses, triangles + smooth-min blending
- **Blend mode:** premultiplied alpha (src=One, dst=OneMinusSrcAlpha) for transparent overlay
- **Draw call:** `draw_indexed(6, instance_count)` — ONE call for all cats
- **Max instances:** 4096 pre-allocated

## Behavior State Machine

```
        ┌──────────┐
   ┌───>│  Idle    │<───┐
   │    └────┬─────┘    │
   │         │          │
   │    ┌────▼─────┐    │
   │    │ Walking  │────┤
   │    └────┬─────┘    │
   │         │          │
   │    ┌────▼─────┐    │
   ├────│ Sleeping │    │
   │    └──────────┘    │
   │                    │
   │    ┌──────────┐    │
   ├────│ Grooming │────┤
   │    └──────────┘    │
   │                    │
   │    ┌──────────┐    │
   └────│ Chasing  │────┘
        └──────────┘
```

Each state has: enter/exit logic, update fn, transition conditions.
Transitions weighted by personality values.
