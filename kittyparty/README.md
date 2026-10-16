# KittyParty

A white kitty with a pink bow trots after your mouse while cat pictures rain
down your desktop like confetti. Every so often there's a surprise cat burst.

Same transparent, click-through DX12 overlay as catswarm and DuckToy: it sits
on top of everything and never gets in the way of your clicks.

## Using it

Double-click `KittyParty.exe`. Click (or right-click) the kitty in the tray,
next to the clock, for options. Windows 11 tucks new tray icons under the
**^** arrow at first; drag the kitty out of there to keep her handy.

| Menu item | What it does |
| --- | --- |
| Cat burst now! | A big pop of cats and confetti |
| Cat rain: gentle / cozy / party! | How much falls from the top |
| Clicking makes cats pop | Each click pops a few cats from the cursor (with a short cooldown) |
| Hide kitty | Hides everything. **Ctrl+Alt+K** does the same from anywhere |
| Start with Windows | Launch at sign-in |
| Add my own pictures... | Opens the "My Cats" folder |
| Quit | Bye kitty |

Click right on the kitty for hearts. When she sits still for a moment she
gives off hearts on her own.

### Your own pictures and your own kitty

Everything lives in `%APPDATA%\KittyParty`:

- **My Cats\\** — drop in jpg/png/gif/webp pictures. They join the confetti
  (and are never rotated out) next time KittyParty starts.
- **kitty.png** — save any picture here (a transparent PNG looks best) to
  replace the follower kitty. Restart KittyParty to see it.
- **settings.txt** — remembers the rain level and click setting.

Cat pictures come from [The Cat API](https://thecatapi.com) with
[CATAAS](https://cataas.com) as a backup. With no internet, drawn kitty cards
fall instead.

## Building

Windows 10/11 with a DX12 GPU.

```bash
cd kittyparty
cargo build --release
target\release\kittyparty.exe
```

Cross-compiling from Linux works too:

```bash
rustup target add x86_64-pc-windows-gnu   # plus the mingw-w64 package
cargo build --release --target x86_64-pc-windows-gnu
```

`cargo test` runs the simulation and image tests on any OS.

The kitty sprite and icon are drawn by `tools/make_art.py` (Pillow); re-run it
from this folder after editing.

## How it works

- `party.rs` — the kitty follower, confetti physics, hearts, and which texture
  layer each cat picture lives in. No Win32 or GPU code, so it tests anywhere.
- `cards.rs` — turns pictures into rounded polaroid cards and fetches cats on
  a background thread.
- `gpu.rs` + `sprite.wgsl` — one instanced draw for everything, from a 64-layer
  texture array. Layers are stored as linear premultiplied colour with a
  box-filtered mip chain (DuckToy's edge lesson), so rotating, shrinking cards
  keep soft edges with no dark fringes.
- `win.rs` — overlay styles, tray menu, start-with-Windows, single instance.
  Click-through needs `WS_EX_LAYERED | WS_EX_TRANSPARENT` (with an opaque
  layer alpha, the DirectComposition visual still shows). Plain
  `WS_EX_TRANSPARENT` still eats clicks meant for other apps. winit rewrites
  the extended style when it applies its own flags, so the styles are
  re-checked every frame.
