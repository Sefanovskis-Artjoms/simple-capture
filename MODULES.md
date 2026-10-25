# Module map — study `simple-capture` in isolation

## Cargo features (plug / unplug)

```text
# Core only (tray + settings + active/all capture, no flash/region)
cargo run --release --no-default-features

# Region select without drawing/editor
cargo run --release --no-default-features --features region

# Full Windows feature set (default)
cargo run --release

# Drop flash only
cargo run --release --no-default-features --features "region,drawing,editor"
```

| Feature   | What it gates                                         |
|-----------|-------------------------------------------------------|
| `flash`   | White success flash after active/all capture          |
| `region`  | Freeze overlay, region select, primary/secondary flow |
| `drawing` | Phase-2 annotation toolkit (pure + overlay/editor UI) |
| `editor`  | Secondary ShareX-style editor window                  |

## Folder layout

```text
src/
  main.rs           Feature-gated module tree + event loop
  app/              Thin ApplicationHandler orchestration
  config/           Hotkeys + mode settings persistence
  capture/          GDI capture + stitch/crop (mostly pure)
  clipboard/        CF_DIBV5 write
  flash/            Brief success overlay          [feature flash]
  region/           Freeze + selection session     [feature region]
  drawing/          Tools, shapes, rasterize       [feature drawing]
  editor/           Secondary egui editor          [feature editor]
  settings/         Settings egui form
  tray/             System tray
  hotkeys/          Register / probe / apply
  gl_window/        OpenGL helper for egui windows
```

## Suggested reading order

1. `capture/image_buf.rs` — pure buffers, stitch, crop  
2. `drawing/` — shapes + raster (no OS)  
3. `region/selection.rs` — selection state machine  
4. `region/overlay.rs` / `region/mod.rs` — session wiring  
5. `app/mod.rs` — how hotkeys call into the above  

Prefer small, pure functions in `capture/` and `drawing/` when experimenting.
