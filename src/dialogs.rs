//! Modal dialogs.

use crate::app::*;
use crate::doc::*;
use crate::export;
use eframe::egui::{self, RichText};

const KEEP: u8 = 0;
const CANCEL: u8 = 1;
const OK: u8 = 2;
const ALT: u8 = 3;

fn buttons(ui: &mut egui::Ui, ok: &str, out: &mut u8) {
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        if ui.button(RichText::new(ok).strong()).clicked() {
            *out = OK;
        }
        if ui.button("Cancel").clicked() {
            *out = CANCEL;
        }
    });
}

fn size_fields(ui: &mut egui::Ui, w: &mut i32, h: &mut i32) -> (bool, bool) {
    let mut changed = (false, false);
    egui::Grid::new("size").num_columns(2).show(ui, |ui| {
        ui.label("Width");
        changed.0 = ui.add(egui::DragValue::new(w).range(1..=8192).suffix(" px")).changed();
        ui.end_row();
        ui.label("Height");
        changed.1 = ui.add(egui::DragValue::new(h).range(1..=8192).suffix(" px")).changed();
        ui.end_row();
    });
    changed
}

impl App {
    pub fn show_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut dlg) = self.dialog.take() else { return };
        let mut out = KEEP;
        let enter = ctx.input(|i| i.key_pressed(egui::Key::Enter));
        let resp = egui::Modal::new(egui::Id::new("dialog")).show(ctx, |ui| {
            ui.set_min_width(300.0);
            ui.set_max_width(460.0);
            let title = |ui: &mut egui::Ui, t: &str| {
                ui.label(RichText::new(t).size(17.0).strong());
                ui.add_space(6.0);
            };
            let doc = self.docs.get(self.active);
            match &mut dlg {
                Dialog::New { w, h, bg } => {
                    title(ui, "New Sprite");
                    size_fields(ui, w, h);
                    ui.horizontal(|ui| {
                        ui.weak("Presets:");
                        for s in [16, 32, 64, 128, 256] {
                            if ui.small_button(format!("{s}×{s}")).clicked() {
                                *w = s;
                                *h = s;
                            }
                        }
                    });
                    ui.add_space(6.0);
                    ui.label("Background");
                    ui.horizontal(|ui| {
                        ui.radio_value(bg, 0, "Transparent");
                        ui.radio_value(bg, 1, "White");
                        ui.radio_value(bg, 2, "Black");
                    });
                    ui.weak("Not sure? 32×32 with a transparent background is a great start for a character.");
                    buttons(ui, "Create", &mut out);
                }
                Dialog::CanvasSize { w, h, ax, ay } => {
                    title(ui, "Canvas Size");
                    ui.weak("Adds or removes space around the artwork without scaling it.");
                    size_fields(ui, w, h);
                    ui.label("Keep the artwork anchored to:");
                    egui::Grid::new("anchor").spacing([2.0, 2.0]).show(ui, |ui| {
                        for y in 0..3u8 {
                            for x in 0..3u8 {
                                let on = *ax == x && *ay == y;
                                if ui
                                    .add(
                                        egui::Button::new(if on { "•" } else { " " })
                                            .selected(on)
                                            .min_size(egui::vec2(24.0, 24.0)),
                                    )
                                    .clicked()
                                {
                                    *ax = x;
                                    *ay = y;
                                }
                            }
                            ui.end_row();
                        }
                    });
                    buttons(ui, "Resize Canvas", &mut out);
                }
                Dialog::SpriteSize { w, h, lock } => {
                    title(ui, "Sprite Size");
                    ui.weak("Scales the whole sprite (all layers and frames) with crisp nearest-neighbor pixels.");
                    let (sw, sh) = doc.map(|d| (d.sprite.w, d.sprite.h)).unwrap_or((1, 1));
                    let (cw, ch) = size_fields(ui, w, h);
                    if *lock && cw {
                        *h = ((*w as f32 * sh as f32 / sw as f32).round() as i32).max(1);
                    } else if *lock && ch {
                        *w = ((*h as f32 * sw as f32 / sh as f32).round() as i32).max(1);
                    }
                    ui.checkbox(lock, "Keep proportions");
                    ui.horizontal(|ui| {
                        for (label, num, den) in [("50%", 1, 2), ("200%", 2, 1), ("300%", 3, 1), ("400%", 4, 1)] {
                            if ui.small_button(label).clicked() {
                                *w = (sw * num / den).max(1);
                                *h = (sh * num / den).max(1);
                            }
                        }
                    });
                    buttons(ui, "Scale Sprite", &mut out);
                }
                Dialog::Export { kind, scale, tag_only } => {
                    title(ui, "Export Image / Animation");
                    ui.radio_value(kind, 0, "PNG — current frame");
                    ui.radio_value(kind, 1, "PNG sequence — one file per frame");
                    ui.radio_value(kind, 2, "Animated GIF");
                    ui.horizontal(|ui| {
                        ui.label("Scale");
                        ui.add(egui::Slider::new(scale, 1..=16).suffix("×"));
                    });
                    if let Some(d) = doc {
                        ui.weak(format!(
                            "Output size: {}×{} px",
                            d.sprite.w as u32 * *scale,
                            d.sprite.h as u32 * *scale
                        ));
                        if let (Some(t), true) = (d.tag_at(d.frame), *kind != 0) {
                            ui.checkbox(
                                tag_only,
                                format!("Only the tag “{}” (frames {}–{})", t.name, t.from + 1, t.to + 1),
                            );
                        }
                    }
                    buttons(ui, "Export…", &mut out);
                }
                Dialog::Sheet { cols, scale, pad, json } => {
                    title(ui, "Export Sprite Sheet");
                    ui.weak("Packs every frame into one PNG — the format most game engines expect.");
                    let n = doc.map(|d| d.sprite.nframes()).unwrap_or(1) as u32;
                    egui::Grid::new("sheet").num_columns(2).show(ui, |ui| {
                        ui.label("Columns");
                        ui.add(egui::DragValue::new(cols).range(1..=n.max(1)));
                        ui.end_row();
                        ui.label("Scale");
                        ui.add(egui::DragValue::new(scale).range(1..=16).suffix("×"));
                        ui.end_row();
                        ui.label("Padding");
                        ui.add(egui::DragValue::new(pad).range(0..=64).suffix(" px"));
                        ui.end_row();
                    });
                    ui.horizontal(|ui| {
                        if ui.small_button("Single row").clicked() {
                            *cols = n;
                        }
                        if ui.small_button("Single column").clicked() {
                            *cols = 1;
                        }
                        if ui.small_button("Square").clicked() {
                            *cols = (n as f32).sqrt().ceil() as u32;
                        }
                    });
                    ui.checkbox(json, "Also write a .json file with frame positions and tags");
                    if let Some(d) = doc {
                        let c = (*cols).clamp(1, n);
                        let rows = n.div_ceil(c);
                        ui.weak(format!(
                            "{n} frames · {c} × {rows} · {}×{} px",
                            c * d.sprite.w as u32 * *scale + (c + 1) * *pad,
                            rows * d.sprite.h as u32 * *scale + (rows + 1) * *pad
                        ));
                    }
                    buttons(ui, "Export…", &mut out);
                }
                Dialog::LayerProps {
                    name, opacity, blend, ..
                } => {
                    title(ui, "Layer Properties");
                    egui::Grid::new("layer").num_columns(2).show(ui, |ui| {
                        ui.label("Name");
                        ui.text_edit_singleline(name);
                        ui.end_row();
                        ui.label("Opacity");
                        ui.add(egui::Slider::new(opacity, 0..=255));
                        ui.end_row();
                        ui.label("Blend mode");
                        egui::ComboBox::from_id_salt("dlg_blend")
                            .selected_text(BLEND_NAMES[*blend as usize])
                            .show_ui(ui, |ui| {
                                for (i, n) in BLEND_NAMES.iter().enumerate() {
                                    ui.selectable_value(blend, i as u16, *n);
                                }
                            });
                        ui.end_row();
                    });
                    buttons(ui, "OK", &mut out);
                }
                Dialog::Tag {
                    idx,
                    name,
                    from,
                    to,
                    dir,
                    color,
                } => {
                    title(ui, if idx.is_some() { "Edit Tag" } else { "New Tag" });
                    ui.weak(
                        "A tag names a range of frames — for example “Walk” or “Idle” — so it can loop on its own.",
                    );
                    let n = doc.map(|d| d.sprite.nframes()).unwrap_or(1);
                    let (mut f1, mut t1) = (*from + 1, *to + 1);
                    egui::Grid::new("tag").num_columns(2).show(ui, |ui| {
                        ui.label("Name");
                        ui.text_edit_singleline(name);
                        ui.end_row();
                        ui.label("From frame");
                        ui.add(egui::DragValue::new(&mut f1).range(1..=n));
                        ui.end_row();
                        ui.label("To frame");
                        ui.add(egui::DragValue::new(&mut t1).range(1..=n));
                        ui.end_row();
                        ui.label("Playback");
                        let names = ["Forward", "Reverse", "Ping-pong", "Ping-pong reverse"];
                        egui::ComboBox::from_id_salt("tag_dir")
                            .selected_text(names[(*dir as usize).min(3)])
                            .show_ui(ui, |ui| {
                                for (i, n) in names.iter().enumerate() {
                                    ui.selectable_value(dir, i as u8, *n);
                                }
                            });
                        ui.end_row();
                        ui.label("Color");
                        ui.color_edit_button_srgb(color);
                        ui.end_row();
                    });
                    *from = f1 - 1;
                    *to = (t1 - 1).max(*from);
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button(RichText::new("OK").strong()).clicked() {
                            out = OK;
                        }
                        if ui.button("Cancel").clicked() {
                            out = CANCEL;
                        }
                        if idx.is_some() && ui.button("Delete Tag").clicked() {
                            out = ALT;
                        }
                    });
                }
                Dialog::FrameDur { ms, all } => {
                    title(ui, "Frame Duration");
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(ms).range(10..=10000).suffix(" ms"));
                        ui.weak(format!("≈ {:.1} frames per second", 1000.0 / *ms as f32));
                    });
                    ui.horizontal(|ui| {
                        for (label, v) in [
                            ("24 fps", 42),
                            ("12 fps", 83),
                            ("10 fps", 100),
                            ("8 fps", 125),
                            ("5 fps", 200),
                        ] {
                            if ui.small_button(label).clicked() {
                                *ms = v;
                            }
                        }
                    });
                    ui.checkbox(all, "Apply to all frames");
                    buttons(ui, "OK", &mut out);
                }
                Dialog::Grid { w, h } => {
                    title(ui, "Grid Settings");
                    egui::Grid::new("grid").num_columns(2).show(ui, |ui| {
                        ui.label("Cell width");
                        ui.add(egui::DragValue::new(w).range(1..=1024).suffix(" px"));
                        ui.end_row();
                        ui.label("Cell height");
                        ui.add(egui::DragValue::new(h).range(1..=1024).suffix(" px"));
                        ui.end_row();
                    });
                    buttons(ui, "OK", &mut out);
                }
                Dialog::ReplaceColor { from, to, tol, all } => {
                    title(ui, "Replace Color");
                    egui::Grid::new("replace").num_columns(2).show(ui, |ui| {
                        ui.label("Find");
                        ui.color_edit_button_srgba_unmultiplied(from);
                        ui.end_row();
                        ui.label("Replace with");
                        ui.color_edit_button_srgba_unmultiplied(to);
                        ui.end_row();
                        ui.label("Tolerance");
                        ui.add(egui::Slider::new(tol, 0..=255));
                        ui.end_row();
                    });
                    ui.checkbox(all, "In every frame of this layer");
                    ui.weak("Applies to the active layer (inside the selection, if there is one).");
                    buttons(ui, "Replace", &mut out);
                }
                Dialog::Shortcuts => {
                    title(ui, "Keyboard Shortcuts & Tips");
                    egui::ScrollArea::vertical().max_height(430.0).show(ui, |ui| {
                        ui.set_min_width(420.0);
                        ui.label(RichText::new("Canvas").strong());
                        egui::Grid::new("tips").num_columns(2).striped(true).show(ui, |ui| {
                            for (a, b) in [
                                ("Zoom", "Mouse wheel, pinch, or Ctrl/Cmd + scroll"),
                                ("Pan", "Hold Space and drag, middle-drag, or two-finger scroll"),
                                ("Draw with background color", "Right mouse button"),
                                ("Pick a color while drawing", "Hold Alt and click"),
                                ("Straight line with the pencil", "Shift + click"),
                                ("Square / circle / 45° lines", "Hold Shift while dragging a shape"),
                                ("Add to / subtract from selection", "Shift / Alt with a selection tool"),
                                ("Move selected pixels", "Drag inside the selection"),
                            ] {
                                ui.label(a);
                                ui.label(b);
                                ui.end_row();
                            }
                        });
                        let mut cat = "";
                        for c in &self.cmds {
                            let Some(sc) = &c.shortcut else { continue };
                            if c.cat != cat {
                                cat = c.cat;
                                ui.add_space(8.0);
                                ui.label(RichText::new(cat).strong());
                            }
                            ui.horizontal(|ui| {
                                ui.label(c.label);
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.label(RichText::new(ctx.format_shortcut(sc)).monospace());
                                });
                            });
                        }
                    });
                    ui.add_space(8.0);
                    if ui.button("Close").clicked() {
                        out = CANCEL;
                    }
                }
                Dialog::About => {
                    title(ui, "asetea");
                    ui.label(format!("Version {}", env!("CARGO_PKG_VERSION")));
                    ui.label("A native pixel-art editor that reads and writes Aseprite (.aseprite / .ase) files.");
                    ui.weak("Not affiliated with Aseprite or Igara Studio.");
                    ui.add_space(8.0);
                    if ui.button("Close").clicked() {
                        out = CANCEL;
                    }
                }
                Dialog::Message { title: t, text } => {
                    title(ui, t);
                    ui.label(text.as_str());
                    ui.add_space(8.0);
                    if ui.button("OK").clicked() {
                        out = CANCEL;
                    }
                }
                Dialog::ConfirmClose { idx, .. } => {
                    let name = self.docs.get(*idx).map(|d| d.title.clone()).unwrap_or_default();
                    title(ui, "Unsaved changes");
                    ui.label(format!("Do you want to save the changes you made to “{name}”?"));
                    ui.weak("Your changes will be lost if you don't save them.");
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button(RichText::new("Save").strong()).clicked() {
                            out = OK;
                        }
                        if ui.button("Don't Save").clicked() {
                            out = ALT;
                        }
                        if ui.button("Cancel").clicked() {
                            out = CANCEL;
                        }
                    });
                }
            }
        });
        let form = !matches!(
            dlg,
            Dialog::ConfirmClose { .. } | Dialog::Shortcuts | Dialog::About | Dialog::Message { .. }
        );
        if out == KEEP && enter {
            out = if form {
                OK
            } else if matches!(dlg, Dialog::ConfirmClose { .. }) {
                KEEP
            } else {
                CANCEL
            };
        }
        if out == KEEP && resp.should_close() {
            out = CANCEL;
        }
        match out {
            KEEP => self.dialog = Some(dlg),
            CANCEL => {}
            _ => self.apply_dialog(dlg, out, ctx),
        }
    }

    fn apply_dialog(&mut self, dlg: Dialog, out: u8, ctx: &egui::Context) {
        let a = self.active;
        match dlg {
            Dialog::New { w, h, bg } => {
                let bg = match bg {
                    1 => Some([255, 255, 255, 255]),
                    2 => Some([0, 0, 0, 255]),
                    _ => None,
                };
                self.new_sprite(w.clamp(1, 8192), h.clamp(1, 8192), bg);
            }
            Dialog::CanvasSize { w, h, ax, ay } => {
                let d = &mut self.docs[a];
                d.commit_floating();
                d.begin_edit();
                let dx = (w - d.sprite.w) * ax as i32 / 2;
                let dy = (h - d.sprite.h) * ay as i32 / 2;
                d.sprite.resize_canvas(w, h, dx, dy);
                d.set_sel(None);
                d.fit_pending = true;
            }
            Dialog::SpriteSize { w, h, .. } => {
                let d = &mut self.docs[a];
                d.commit_floating();
                d.begin_edit();
                d.sprite.scale(w, h);
                d.set_sel(None);
                d.fit_pending = true;
            }
            Dialog::Export { kind, scale, tag_only } => {
                let d = &self.docs[a];
                let stem = std::path::Path::new(&d.title)
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let (ext, desc) = if kind == 2 {
                    ("gif", "GIF animation")
                } else {
                    ("png", "PNG image")
                };
                let Some(path) = rfd::FileDialog::new()
                    .add_filter(desc, &[ext])
                    .set_file_name(format!("{stem}.{ext}"))
                    .save_file()
                else {
                    self.dialog = Some(Dialog::Export { kind, scale, tag_only });
                    return;
                };
                let (from, to) = match (tag_only, d.tag_at(d.frame)) {
                    (true, Some(t)) => (t.from, t.to),
                    _ => (0, d.sprite.nframes() - 1),
                };
                let res = match kind {
                    0 => export::save_png(&path, &d.sprite.composite(d.frame), scale)
                        .map(|_| format!("Exported {}", path.display())),
                    1 => export::save_png_sequence(&path, &d.sprite, from, to, scale)
                        .map(|n| format!("Exported {n} PNG files")),
                    _ => export::save_gif(&path, &d.sprite, from, to, scale)
                        .map(|_| format!("Exported {}", path.display())),
                };
                match res {
                    Ok(msg) => self.toast(msg),
                    Err(e) => self.error("Export failed", e),
                }
            }
            Dialog::Sheet { cols, scale, pad, json } => {
                let d = &self.docs[a];
                let stem = std::path::Path::new(&d.title)
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let Some(path) = rfd::FileDialog::new()
                    .add_filter("PNG image", &["png"])
                    .set_file_name(format!("{stem}-sheet.png"))
                    .save_file()
                else {
                    self.dialog = Some(Dialog::Sheet { cols, scale, pad, json });
                    return;
                };
                match export::save_sheet(&path, &d.sprite, cols, scale, pad, json) {
                    Ok(()) => self.toast(format!("Exported {}", path.display())),
                    Err(e) => self.error("Export failed", e),
                }
            }
            Dialog::LayerProps {
                idx,
                name,
                opacity,
                blend,
            } => {
                let d = &mut self.docs[a];
                if idx < d.sprite.layers.len() {
                    d.begin_edit();
                    let l = &mut d.sprite.layers[idx];
                    l.name = name;
                    l.opacity = opacity;
                    l.blend = blend;
                }
            }
            Dialog::Tag {
                idx,
                name,
                from,
                to,
                dir,
                color,
            } => {
                let d = &mut self.docs[a];
                d.begin_edit();
                let n = d.sprite.nframes();
                let (from, to) = (from.min(n - 1), to.min(n - 1));
                match (idx, out) {
                    (Some(i), ALT) => {
                        d.sprite.tags.remove(i);
                    }
                    (Some(i), _) => {
                        let t = &mut d.sprite.tags[i];
                        (t.name, t.from, t.to, t.dir, t.color) = (name, from, to, dir, color);
                    }
                    (None, _) => {
                        d.sprite.tags.push(Tag {
                            name,
                            from,
                            to,
                            dir,
                            repeat: 0,
                            color,
                            user: UserData::default(),
                        });
                        d.sprite.tags.sort_by_key(|t| (t.from, std::cmp::Reverse(t.to)));
                    }
                }
            }
            Dialog::FrameDur { ms, all } => {
                let d = &mut self.docs[a];
                d.begin_edit();
                if all {
                    d.sprite.durations.iter_mut().for_each(|v| *v = ms);
                } else {
                    d.sprite.durations[d.frame] = ms;
                }
            }
            Dialog::Grid { w, h } => {
                let d = &mut self.docs[a];
                d.begin_edit();
                d.sprite.grid.2 = w;
                d.sprite.grid.3 = h;
                self.show_grid = true;
            }
            Dialog::ReplaceColor { from, to, tol, all } => {
                self.replace_color(from, to, tol, all);
            }
            Dialog::ConfirmClose { idx, then_quit } => {
                if out == OK && !self.save_doc(idx, false) {
                    return;
                }
                self.close_doc(idx);
                if then_quit {
                    self.request_quit(ctx);
                }
            }
            _ => {}
        }
    }
}
