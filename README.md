# asetea

A native, cross-platform pixel-art editor for Aseprite files (`.aseprite` / `.ase`), written in Rust with [egui](https://github.com/emilk/egui).

```sh
cargo run --release                       # start with the welcome screen
cargo run --release -- tests/fixtures/The_Dude.aseprite  # open files directly
cargo test                                # file-format round-trips + simulated editing sessions
```

Runs on macOS, Windows and Linux (Linux needs the usual GTK/X11 or Wayland development packages for the file dialogs and windowing).

## Features

- **Files** – open/save Aseprite files (RGB, grayscale and indexed; layers, groups, blend modes, opacity, linked cels, tags, frame durations, palette, user data; slices and other unknown chunks are preserved). Import PNG/JPEG/BMP/GIF. Export PNG, PNG sequence, animated GIF, and sprite sheets with Aseprite-style JSON. Drag & drop, recent files, multiple documents in tabs, unsaved-changes prompts.
- **Tools** – pencil (pixel-perfect, alpha lock, Shift+click lines), eraser, paint bucket, eyedropper, line, rectangle, ellipse, dithered/smooth gradient, rectangular marquee, lasso, magic wand, move, hand, zoom. Round/square brushes up to 64 px, horizontal/vertical mirror drawing, right-click paints with the background color.
- **Selections** – add/subtract, invert, move pixels, cut/copy/paste between frames, layers and documents, fill, flip, crop.
- **Layers** – add, group, duplicate, delete, reorder, merge down, flatten, visibility, lock, opacity and all 19 Aseprite blend modes.
- **Animation** – timeline with cels and linked cels, tags (forward, reverse, ping-pong), per-frame duration, playback, onion skin, live preview panel.
- **Sprite** – canvas size, scale, trim, rotate, flip, color mode conversion, grid, replace color, invert, desaturate, outline.
- **Color** – HSV picker with alpha, hex input, palette editing, built-in palettes (DB16/32, PICO-8, Sweetie 16, Endesga 32, Game Boy…), load/save `.gpl` / `.hex`, palette from sprite.
- **For beginners** – tooltips explain every tool and option, the status bar shows what the current tool does, friendly messages explain why an action can't happen (locked/hidden layer…).
- **For advanced users** – Aseprite-style keyboard shortcuts, a command palette (Ctrl/Cmd+K) to search every command, 300-step undo.

## Known limitations

- Tilemap layers are converted to regular image layers when opened (a note is shown); saving does not keep the tilesets.
- Palette entry names are not kept.
- Copy/paste uses an internal clipboard (it does not exchange images with other apps).
