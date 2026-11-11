//! Menus, toolbar, tool options, color/palette panel, timeline, status bar, welcome screen.

use crate::app::*;
use crate::doc::*;
use crate::palettes;
use eframe::egui::{self, Align2, Color32, FontId, Rect, RichText, Sense, Stroke, StrokeKind, pos2, vec2};

fn c32(c: Rgba) -> Color32 {
    Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3])
}

/// Paint a color swatch with a checkerboard behind translucent colors.
fn swatch(p: &egui::Painter, r: Rect, c: Rgba) {
    if c[3] < 255 {
        p.rect_filled(r, 2.0, Color32::from_gray(200));
        let h = r.size() / 2.0;
        p.rect_filled(Rect::from_min_size(r.min, h), 0.0, Color32::from_gray(120));
        p.rect_filled(Rect::from_min_size(r.min + h, h), 0.0, Color32::from_gray(120));
    }
    p.rect_filled(r, 2.0, c32(c));
}

pub fn tool_icon(p: &egui::Painter, r: Rect, tool: Tool, c: Color32) {
    let s = r.width() / 16.0;
    let pt = |x: f32, y: f32| pos2(r.left() + x * s, r.top() + y * s);
    let st = Stroke::new(1.6_f32, c);
    let line = |a: (f32, f32), b: (f32, f32)| p.line_segment([pt(a.0, a.1), pt(b.0, b.1)], st);
    let poly = |pts: &[(f32, f32)]| {
        p.add(egui::Shape::closed_line(pts.iter().map(|q| pt(q.0, q.1)).collect(), st));
    };
    match tool {
        Tool::Pencil => {
            poly(&[(11.0, 2.0), (14.0, 5.0), (6.0, 13.0), (2.0, 14.0), (3.0, 10.0)]);
            line((9.5, 3.5), (12.5, 6.5));
        }
        Tool::Eraser => {
            poly(&[(9.0, 2.0), (14.0, 7.0), (8.0, 13.0), (3.0, 8.0)]);
            line((6.0, 5.0), (11.0, 10.0));
            line((2.0, 14.5), (14.0, 14.5));
        }
        Tool::Bucket => {
            poly(&[(3.0, 6.0), (13.0, 6.0), (11.0, 13.5), (5.0, 13.5)]);
            line((5.0, 6.0), (8.0, 2.0));
            line((8.0, 2.0), (11.0, 6.0));
            p.circle_filled(pt(14.3, 11.0), 1.3 * s, c);
        }
        Tool::Eyedropper => {
            p.line_segment([pt(3.5, 12.5), pt(10.0, 6.0)], Stroke::new(2.4_f32, c));
            p.circle_filled(pt(11.6, 4.4), 2.4 * s, c);
            line((2.0, 14.0), (3.5, 12.5));
        }
        Tool::Line => {
            line((3.0, 13.0), (13.0, 3.0));
            p.circle_filled(pt(3.0, 13.0), 1.4 * s, c);
            p.circle_filled(pt(13.0, 3.0), 1.4 * s, c);
        }
        Tool::Rect => {
            p.rect_stroke(
                Rect::from_min_max(pt(2.5, 4.0), pt(13.5, 12.0)),
                0.0,
                st,
                StrokeKind::Middle,
            );
        }
        Tool::Ellipse => {
            p.circle_stroke(pt(8.0, 8.0), 5.2 * s, st);
        }
        Tool::Gradient => {
            p.add(egui::Shape::convex_polygon(
                vec![pt(2.5, 4.0), pt(13.5, 4.0), pt(2.5, 12.0)],
                c,
                Stroke::NONE,
            ));
            p.rect_stroke(
                Rect::from_min_max(pt(2.5, 4.0), pt(13.5, 12.0)),
                0.0,
                st,
                StrokeKind::Middle,
            );
        }
        Tool::Marquee => {
            let pts = [pt(2.5, 4.0), pt(13.5, 4.0), pt(13.5, 12.0), pt(2.5, 12.0), pt(2.5, 4.0)];
            p.extend(egui::Shape::dashed_line(&pts, st, 2.2 * s, 1.8 * s));
        }
        Tool::Lasso => {
            poly(&[
                (4.0, 5.0),
                (8.0, 3.0),
                (12.0, 4.0),
                (14.0, 7.0),
                (12.0, 10.0),
                (8.0, 11.0),
                (5.0, 10.0),
                (3.0, 8.0),
            ]);
            line((5.0, 10.0), (4.0, 14.0));
        }
        Tool::Wand => {
            p.line_segment([pt(2.5, 13.5), pt(9.0, 7.0)], Stroke::new(2.2_f32, c));
            line((11.5, 1.5), (11.5, 7.5));
            line((8.5, 4.5), (14.5, 4.5));
            line((9.5, 2.5), (13.5, 6.5));
            line((13.5, 2.5), (9.5, 6.5));
        }
        Tool::Move => {
            line((8.0, 2.0), (8.0, 14.0));
            line((2.0, 8.0), (14.0, 8.0));
            for (tip, a, b) in [
                ((8.0, 2.0), (6.0, 4.0), (10.0, 4.0)),
                ((8.0, 14.0), (6.0, 12.0), (10.0, 12.0)),
                ((2.0, 8.0), (4.0, 6.0), (4.0, 10.0)),
                ((14.0, 8.0), (12.0, 6.0), (12.0, 10.0)),
            ] {
                line(tip, a);
                line(tip, b);
            }
        }
        Tool::Hand => {
            p.rect_stroke(
                Rect::from_min_max(pt(4.5, 7.5), pt(12.5, 14.0)),
                2.0,
                st,
                StrokeKind::Middle,
            );
            for (x, y) in [(5.5, 4.0), (7.8, 2.5), (10.1, 2.5), (12.0, 4.0)] {
                line((x, 7.5), (x, y));
            }
            line((4.5, 11.0), (2.5, 8.5));
        }
        Tool::Zoom => {
            p.circle_stroke(pt(7.0, 7.0), 4.3 * s, st);
            p.line_segment([pt(10.2, 10.2), pt(14.0, 14.0)], Stroke::new(2.4_f32, c));
        }
    }
}

fn menu_cmds(ui: &mut egui::Ui, cmds: &[CmdInfo], list: &[Cmd], out: &mut Vec<Cmd>) {
    for cmd in list {
        if let Some(info) = cmds.iter().find(|c| c.cmd == *cmd) {
            let mut b = egui::Button::new(info.label);
            if let Some(sc) = &info.shortcut {
                b = b.shortcut_text(ui.ctx().format_shortcut(sc));
            }
            if ui.add(b).clicked() {
                out.push(*cmd);
                ui.close();
            }
        }
    }
}

impl App {
    fn item(&mut self, ui: &mut egui::Ui, cmd: Cmd) {
        let info = self.info(cmd);
        let label = match self.checked(cmd) {
            Some(true) => format!("✔ {}", info.label),
            Some(false) => format!("     {}", info.label),
            None => info.label.to_string(),
        };
        let mut b = egui::Button::new(label);
        if let Some(sc) = &info.shortcut {
            b = b.shortcut_text(ui.ctx().format_shortcut(sc));
        }
        if ui.add_enabled(self.enabled(cmd), b).clicked() {
            self.pending.push(cmd);
            ui.close();
        }
    }

    fn items(&mut self, ui: &mut egui::Ui, list: &[Option<Cmd>]) {
        for c in list {
            match c {
                Some(c) => self.item(ui, *c),
                None => {
                    ui.separator();
                }
            }
        }
    }

    /// Small button that runs a command; the tooltip shows its name and shortcut.
    fn cmd_button(&mut self, ui: &mut egui::Ui, text: &str, cmd: Cmd) {
        let info = self.info(cmd);
        let tip = match &info.shortcut {
            Some(sc) => format!("{}  ({})", info.label, ui.ctx().format_shortcut(sc)),
            None => info.label.to_string(),
        };
        if ui
            .add_enabled(self.enabled(cmd), egui::Button::new(text))
            .on_hover_text(tip)
            .clicked()
        {
            self.pending.push(cmd);
        }
    }

    pub fn menu_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                use Cmd::*;
                ui.menu_button("File", |ui| {
                    self.items(ui, &[Some(New), Some(Open)]);
                    ui.menu_button("Open Recent", |ui| {
                        if self.recent.is_empty() {
                            ui.weak("No recent files yet");
                        }
                        for p in self.recent.clone() {
                            let name = p
                                .file_name()
                                .map(|n| n.to_string_lossy().into_owned())
                                .unwrap_or_default();
                            if ui.button(name).on_hover_text(p.display().to_string()).clicked() {
                                self.open_path(&p);
                                ui.close();
                            }
                        }
                    });
                    self.items(
                        ui,
                        &[
                            None,
                            Some(Save),
                            Some(SaveAs),
                            None,
                            Some(ExportImage),
                            Some(ExportSheet),
                            None,
                            Some(CloseDoc),
                            Some(Quit),
                        ],
                    );
                });
                ui.menu_button("Edit", |ui| {
                    self.items(
                        ui,
                        &[
                            Some(Undo),
                            Some(Redo),
                            None,
                            Some(Cut),
                            Some(Copy),
                            Some(Paste),
                            Some(Clear),
                            None,
                            Some(FillSel),
                            Some(FlipH),
                            Some(FlipV),
                            None,
                        ],
                    );
                    ui.menu_button("Adjust Colors", |ui| {
                        self.items(ui, &[Some(Invert), Some(Desaturate), Some(ReplaceColor), Some(Outline)]);
                    });
                    self.items(ui, &[None, Some(CommandPalette)]);
                });
                ui.menu_button("Select", |ui| {
                    self.items(
                        ui,
                        &[
                            Some(SelectAll),
                            Some(Deselect),
                            Some(InvertSel),
                            None,
                            Some(Tool(crate::app::Tool::Marquee)),
                            Some(Tool(crate::app::Tool::Lasso)),
                            Some(Tool(crate::app::Tool::Wand)),
                        ],
                    );
                });
                ui.menu_button("Sprite", |ui| {
                    self.items(
                        ui,
                        &[
                            Some(CanvasSize),
                            Some(SpriteSize),
                            Some(CropToSel),
                            Some(Trim),
                            None,
                            Some(Rot90Cw),
                            Some(Rot90Ccw),
                            Some(Rot180),
                            None,
                        ],
                    );
                    ui.menu_button("Color Mode", |ui| {
                        self.items(ui, &[Some(ModeRgb), Some(ModeIndexed), Some(ModeGray)]);
                    });
                    ui.menu_button("Palette", |ui| {
                        self.items(ui, &[Some(ImportPalette), Some(ExportPalette), Some(PaletteFromSprite)]);
                    });
                    self.items(ui, &[Some(GridSettings)]);
                });
                ui.menu_button("Layer", |ui| {
                    self.items(
                        ui,
                        &[
                            Some(NewLayer),
                            Some(NewGroup),
                            Some(DupLayer),
                            Some(DelLayer),
                            None,
                            Some(LayerProps),
                            None,
                            Some(LayerUp),
                            Some(LayerDown),
                            None,
                            Some(MergeDown),
                            Some(Flatten),
                        ],
                    );
                });
                ui.menu_button("Frame", |ui| {
                    self.items(
                        ui,
                        &[
                            Some(NewFrame),
                            Some(NewEmptyFrame),
                            Some(DelFrame),
                            None,
                            Some(FrameProps),
                            Some(NewTag),
                            None,
                            Some(FrameLeft),
                            Some(FrameRight),
                            None,
                            Some(PlayToggle),
                            Some(PrevFrame),
                            Some(NextFrame),
                            Some(FirstFrame),
                            Some(LastFrame),
                        ],
                    );
                });
                ui.menu_button("View", |ui| {
                    self.items(
                        ui,
                        &[
                            Some(ZoomIn),
                            Some(ZoomOut),
                            Some(ZoomFit),
                            Some(Zoom100),
                            None,
                            Some(ToggleGrid),
                            Some(TogglePixelGrid),
                            Some(ToggleOnion),
                            None,
                            Some(SymH),
                            Some(SymV),
                            None,
                            Some(ToggleTheme),
                        ],
                    );
                });
                ui.menu_button("Help", |ui| {
                    self.items(ui, &[Some(Shortcuts), Some(CommandPalette), None, Some(About)]);
                });
            });
        });
    }

    pub fn status_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let Some(d) = self.docs.get(self.active) else {
                    ui.weak("Create or open a sprite to get started.");
                    return;
                };
                let mode = match d.sprite.mode {
                    ColorMode::Rgba => "RGB",
                    ColorMode::Gray => "Gray",
                    ColorMode::Indexed => "Indexed",
                };
                ui.label(format!("{}×{} px · {mode}", d.sprite.w, d.sprite.h));
                ui.separator();
                ui.label(format!("{:.0}%", d.zoom * 100.0));
                ui.separator();
                match self.hover_px {
                    Some((x, y)) => ui.label(format!("x {x}  y {y}")),
                    None => ui.weak("x –  y –"),
                };
                if let Some((w, h)) = self.drag_size {
                    ui.separator();
                    ui.label(format!("{w}×{h}"));
                } else if let Some((_, _, w, h)) = d.sel.as_ref().and_then(|s| s.bounds()) {
                    ui.separator();
                    ui.label(format!("selection {w}×{h}"));
                }
                ui.separator();
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.add(egui::Label::new(RichText::new(self.tool.help()).weak()).truncate());
                });
            });
        });
    }

    pub fn welcome(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space((ui.available_height() * 0.16).max(12.0));
                ui.label(RichText::new("asetea").size(44.0).strong());
                ui.label(RichText::new("A pixel-art editor for Aseprite files").size(16.0).weak());
                ui.add_space(24.0);
                let big = |t: &str| egui::Button::new(RichText::new(t).size(16.0)).min_size(vec2(240.0, 38.0));
                if ui.add(big("New Sprite…")).clicked() {
                    self.pending.push(Cmd::New);
                }
                if ui.add(big("Open File…")).clicked() {
                    self.pending.push(Cmd::Open);
                }
                ui.add_space(6.0);
                ui.weak("…or drop .aseprite, .png or .gif files onto this window");
                if !self.recent.is_empty() {
                    ui.add_space(20.0);
                    ui.label(RichText::new("Recent files").strong());
                    for p in self.recent.clone() {
                        let name = p
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default();
                        if ui.link(name).on_hover_text(p.display().to_string()).clicked() {
                            self.open_path(&p);
                        }
                    }
                }
                ui.add_space(24.0);
                let key = ctx.format_shortcut(&egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::K));
                ui.weak(format!(
                    "Tip: press {key} anytime to search every command, or F1 for shortcuts."
                ));
            });
        });
    }

    pub fn tabs_and_options(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("tabs").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                let mut close = None;
                for i in 0..self.docs.len() {
                    let d = &self.docs[i];
                    let label = format!("{}{}", d.title, if d.modified() { " •" } else { "" });
                    let tip = d
                        .path
                        .as_ref()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| "Not saved yet".into());
                    ui.spacing_mut().item_spacing.x = 0.0;
                    if ui
                        .selectable_label(i == self.active, label)
                        .on_hover_text(tip)
                        .clicked()
                    {
                        self.active = i;
                        self.drag = Drag::None;
                    }
                    if ui
                        .add(egui::Button::new("×").frame(false))
                        .on_hover_text("Close")
                        .clicked()
                    {
                        close = Some(i);
                    }
                    ui.spacing_mut().item_spacing.x = 6.0;
                    ui.separator();
                }
                if let Some(i) = close {
                    self.request_close(i);
                }
            });
        });
        egui::TopBottomPanel::top("options").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.set_min_height(26.0);
                ui.label(RichText::new(self.tool.name()).strong());
                ui.separator();
                let brush = |app: &mut App, ui: &mut egui::Ui| {
                    ui.label("Size");
                    ui.add(egui::DragValue::new(&mut app.brush_size).range(1..=64).speed(0.15).suffix(" px"))
                        .on_hover_text("Brush size. Shortcuts: [ and ]");
                    ui.selectable_value(&mut app.brush_square, false, "Round");
                    ui.selectable_value(&mut app.brush_square, true, "Square");
                };
                match self.tool {
                    Tool::Pencil => {
                        brush(self, ui);
                        ui.separator();
                        ui.checkbox(&mut self.pixel_perfect, "Pixel-perfect")
                            .on_hover_text("Removes the extra corner pixels from freehand lines so they stay crisp and one pixel thin (1 px brush).");
                        ui.checkbox(&mut self.alpha_lock, "Lock alpha")
                            .on_hover_text("Only paint over pixels that already exist — great for shading inside a shape.");
                    }
                    Tool::Eraser | Tool::Line => brush(self, ui),
                    Tool::Rect | Tool::Ellipse => {
                        brush(self, ui);
                        ui.separator();
                        ui.checkbox(&mut self.filled, "Filled");
                    }
                    Tool::Bucket | Tool::Wand => {
                        ui.label("Tolerance");
                        ui.add(egui::Slider::new(&mut self.tolerance, 0..=255))
                            .on_hover_text("How different a color may be and still count as the same area. 0 = exact match.");
                        ui.checkbox(&mut self.contiguous, "Contiguous")
                            .on_hover_text("On: only the connected area. Off: every matching pixel on the layer.");
                    }
                    Tool::Gradient => {
                        ui.checkbox(&mut self.dither, "Dithered")
                            .on_hover_text("On: a pixel-art pattern using only the two colors. Off: a smooth blend.");
                    }
                    Tool::Marquee | Tool::Lasso => {
                        ui.weak("Shift: add · Alt: subtract");
                        self.cmd_button(ui, "Select All", Cmd::SelectAll);
                        self.cmd_button(ui, "Deselect", Cmd::Deselect);
                        self.cmd_button(ui, "Invert", Cmd::InvertSel);
                    }
                    Tool::Move => {
                        ui.weak("Drag the selection — or the whole cel if nothing is selected.");
                    }
                    Tool::Eyedropper => {
                        ui.weak("Left click: foreground · Right click: background");
                    }
                    Tool::Hand | Tool::Zoom => {
                        self.cmd_button(ui, "Fit", Cmd::ZoomFit);
                        self.cmd_button(ui, "100%", Cmd::Zoom100);
                        self.cmd_button(ui, "Zoom Out", Cmd::ZoomOut);
                        self.cmd_button(ui, "Zoom In", Cmd::ZoomIn);
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.toggle_value(&mut self.show_grid, "Grid").on_hover_text("Show the grid over the canvas");
                    ui.toggle_value(&mut self.sym_v, "Mirror ↕").on_hover_text("Mirror everything you draw top ↔ bottom");
                    ui.toggle_value(&mut self.sym_h, "Mirror ↔").on_hover_text("Mirror everything you draw left ↔ right");
                    self.cmd_button(ui, "Redo", Cmd::Redo);
                    self.cmd_button(ui, "Undo", Cmd::Undo);
                });
            });
        });
    }

    pub fn toolbar(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("tools")
            .exact_width(76.0)
            .resizable(false)
            .show(ctx, |ui| {
                ui.add_space(4.0);
                egui::ScrollArea::vertical()
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing = vec2(2.0, 2.0);
                        let (grid, _) = ui.allocate_exact_size(vec2(72.0, 7.0 * 34.0), Sense::hover());
                        for (i, tool) in Tool::ALL.into_iter().enumerate() {
                            let r = Rect::from_min_size(
                                grid.min + vec2((i % 2) as f32 * 37.0, (i / 2) as f32 * 34.0),
                                vec2(35.0, 32.0),
                            );
                            let resp = ui.interact(r, ui.id().with(("tool", i)), Sense::click());
                            let selected = self.tool == tool;
                            let vis = ui.visuals();
                            if selected {
                                ui.painter().rect_filled(r, 5.0, vis.selection.bg_fill);
                            } else if resp.hovered() {
                                ui.painter().rect_filled(r, 5.0, vis.widgets.hovered.weak_bg_fill);
                            }
                            let col = if selected {
                                vis.selection.stroke.color
                            } else {
                                vis.widgets.inactive.fg_stroke.color
                            };
                            tool_icon(
                                ui.painter(),
                                Rect::from_center_size(r.center(), vec2(18.0, 18.0)),
                                tool,
                                col,
                            );
                            let resp = resp.on_hover_ui(|ui| {
                                ui.set_max_width(260.0);
                                ui.label(RichText::new(format!("{}   [{}]", tool.name(), tool.key().2)).strong());
                                ui.label(tool.help());
                            });
                            if resp.clicked() {
                                self.pending.push(Cmd::Tool(tool));
                            }
                        }
                        ui.add_space(8.0);
                        // Foreground / background indicator; click to swap.
                        let (r, resp) = ui.allocate_exact_size(vec2(34.0, 34.0), Sense::click());
                        let p = ui.painter();
                        let outline = Stroke::new(1.0_f32, ui.visuals().widgets.noninteractive.fg_stroke.color);
                        let b = Rect::from_min_size(r.min + vec2(12.0, 12.0), vec2(20.0, 20.0));
                        let f = Rect::from_min_size(r.min + vec2(2.0, 2.0), vec2(20.0, 20.0));
                        swatch(p, b, self.bg);
                        p.rect_stroke(b, 2.0, outline, StrokeKind::Outside);
                        swatch(p, f, self.fg);
                        p.rect_stroke(f, 2.0, outline, StrokeKind::Outside);
                        if resp
                            .on_hover_text("Foreground / background color.\nClick to swap them (X).")
                            .clicked()
                        {
                            self.pending.push(Cmd::SwapColors);
                        }
                    });
            });
    }

    pub fn side_panel(&mut self, ctx: &egui::Context) {
        egui::SidePanel::right("side")
            .default_width(236.0)
            .width_range(200.0..=420.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    egui::CollapsingHeader::new(RichText::new("Color").strong())
                        .default_open(true)
                        .show(ui, |ui| self.color_section(ui));
                    egui::CollapsingHeader::new(RichText::new("Palette").strong())
                        .default_open(true)
                        .show(ui, |ui| self.palette_section(ui));
                    egui::CollapsingHeader::new(RichText::new("Preview").strong())
                        .default_open(true)
                        .show(ui, |ui| self.preview_section(ui));
                });
            });
    }

    fn color_section(&mut self, ui: &mut egui::Ui) {
        if self.picker_for != self.fg {
            self.picker = egui::ecolor::Hsva::from_srgba_unmultiplied(self.fg);
            self.picker_for = self.fg;
            self.hex_text = palettes::to_hex(self.fg);
        }
        ui.horizontal(|ui| {
            ui.color_edit_button_srgba_unmultiplied(&mut self.fg)
                .on_hover_text("Foreground color — used by the left mouse button");
            ui.color_edit_button_srgba_unmultiplied(&mut self.bg)
                .on_hover_text("Background color — used by the right mouse button");
            self.cmd_button(ui, "Swap", Cmd::SwapColors);
            ui.label("#");
            let r = ui.add(
                egui::TextEdit::singleline(&mut self.hex_text)
                    .desired_width(62.0)
                    .font(egui::TextStyle::Monospace),
            );
            if r.changed()
                && let Some(c) = palettes::parse_hex(&self.hex_text)
            {
                self.fg = c;
                self.picker = egui::ecolor::Hsva::from_srgba_unmultiplied(c);
                self.picker_for = c;
            }
            r.on_hover_text("Hex color (RRGGBB or RRGGBBAA)");
        });
        ui.spacing_mut().slider_width = (ui.available_width() - 8.0).clamp(120.0, 260.0);
        if egui::color_picker::color_picker_hsva_2d(ui, &mut self.picker, egui::color_picker::Alpha::OnlyBlend) {
            self.fg = self.picker.to_srgba_unmultiplied();
            self.picker_for = self.fg;
            self.hex_text = palettes::to_hex(self.fg);
        }
    }

    fn palette_section(&mut self, ui: &mut egui::Ui) {
        let a = self.active;
        let fg = self.fg;
        let mut new_pal: Option<Vec<Rgba>> = None;
        ui.horizontal(|ui| {
            ui.menu_button("Presets", |ui| {
                for p in palettes::PRESETS {
                    if ui.button(format!("{} ({})", p.name, p.colors().len())).clicked() {
                        new_pal = Some(p.colors());
                        ui.close();
                    }
                }
                ui.separator();
                self.items(
                    ui,
                    &[
                        Some(Cmd::ImportPalette),
                        Some(Cmd::ExportPalette),
                        Some(Cmd::PaletteFromSprite),
                    ],
                );
            });
            let pal = &self.docs[a].sprite.palette;
            if ui
                .button("+")
                .on_hover_text("Add the foreground color to the palette")
                .clicked()
                && !pal.contains(&fg)
            {
                let mut p = pal.clone();
                p.push(fg);
                new_pal = Some(p);
            }
            let has = pal.contains(&fg);
            if ui
                .add_enabled(has && pal.len() > 1, egui::Button::new("-"))
                .on_hover_text("Remove the foreground color from the palette")
                .clicked()
            {
                new_pal = Some(pal.iter().copied().filter(|c| *c != fg).collect());
            }
            ui.weak(format!("{} colors", pal.len()));
        });
        let pal = self.docs[a].sprite.palette.clone();
        let sz = 18.0;
        let cols = ((ui.available_width() + 2.0) / (sz + 2.0)).floor().max(1.0) as usize;
        let rows = pal.len().div_ceil(cols);
        let (rect, _) =
            ui.allocate_exact_size(vec2(cols as f32 * (sz + 2.0), rows as f32 * (sz + 2.0)), Sense::hover());
        for (i, c) in pal.iter().enumerate() {
            let r = Rect::from_min_size(
                rect.min + vec2((i % cols) as f32, (i / cols) as f32) * (sz + 2.0),
                vec2(sz, sz),
            );
            let resp = ui.interact(r, ui.id().with(("pal", i)), Sense::click());
            let p = ui.painter();
            swatch(p, r, *c);
            if *c == self.fg {
                p.rect_stroke(r, 2.0, Stroke::new(2.0_f32, Color32::WHITE), StrokeKind::Inside);
                p.rect_stroke(r, 2.0, Stroke::new(1.0_f32, Color32::BLACK), StrokeKind::Outside);
            } else if *c == self.bg {
                p.rect_stroke(r, 2.0, Stroke::new(1.0_f32, Color32::WHITE), StrokeKind::Inside);
            } else if resp.hovered() {
                p.rect_stroke(
                    r,
                    2.0,
                    Stroke::new(1.0_f32, Color32::from_white_alpha(160)),
                    StrokeKind::Inside,
                );
            }
            if resp.clicked() {
                self.fg = *c;
            }
            if resp.secondary_clicked() {
                self.bg = *c;
            }
            resp.on_hover_text(format!(
                "#{}  ·  index {i}\nClick: foreground · Right-click: background",
                palettes::to_hex(*c)
            ));
        }
        if let Some(p) = new_pal {
            let d = &mut self.docs[a];
            d.begin_edit();
            d.sprite.palette = p;
        }
    }

    fn preview_section(&mut self, ui: &mut egui::Ui) {
        let now = self.time;
        let ctx = ui.ctx().clone();
        let d = &mut self.docs[self.active];
        let n = d.sprite.nframes();
        let range = d.range_at(d.frame);
        if d.playing || n < 2 {
            d.preview_frame = d.frame;
        } else {
            if d.preview_frame < range.0 || d.preview_frame > range.1 || now > d.preview_next + 1.0 {
                d.preview_frame = range.0;
                d.preview_next = now + d.sprite.durations[range.0] as f64 / 1000.0;
                d.preview_sign = 0;
            }
            if now >= d.preview_next {
                d.preview_frame = advance(d.preview_frame, range, &mut d.preview_sign).min(n - 1);
                d.preview_next += d.sprite.durations[d.preview_frame].max(10) as f64 / 1000.0;
            }
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(
                (d.preview_next - now).clamp(0.0, 1.0),
            ));
        }
        let pf = d.preview_frame.min(n - 1);
        let (w, h) = (d.sprite.w as f32, d.sprite.h as f32);
        let avail = ui.available_width().min(260.0);
        let mut scale = (avail / w).min(170.0 / h);
        if scale >= 1.0 {
            scale = scale.floor();
        }
        let Doc {
            sprite,
            tex_preview,
            rev,
            ..
        } = d;
        let t = tex_preview.get(&ctx, "preview", (*rev, pf as u64), || sprite.composite(pf));
        let (r, _) = ui.allocate_exact_size(vec2(w, h) * scale, Sense::hover());
        let dark = ui.visuals().dark_mode;
        ui.painter().rect_filled(
            r,
            0.0,
            if dark {
                Color32::from_gray(110)
            } else {
                Color32::from_gray(215)
            },
        );
        ui.painter()
            .image(t, r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        let what = match d.tag_at(d.frame) {
            Some(t) if n > 1 => format!("Looping tag “{}” · frame {}", t.name, pf + 1),
            _ if n > 1 => format!("Looping all frames · frame {}", pf + 1),
            _ => "Single frame — add frames in the timeline to animate".into(),
        };
        ui.weak(what);
    }

    // ------------------------------------------------------------ timeline

    pub fn timeline_panel(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("timeline")
            .resizable(true)
            .default_height(240.0)
            .height_range(96.0..=600.0)
            .show(ctx, |ui| {
                self.timeline_controls(ui);
                ui.separator();
                self.timeline_grid(ui);
            });
    }

    /// Begin one undo step for a slider drag, however many frames it lasts.
    fn slider_step(&mut self, resp: &egui::Response) {
        if self.slider_edit != Some(resp.id) {
            self.docs[self.active].begin_edit();
            self.slider_edit = Some(resp.id);
        }
        self.docs[self.active].touch();
    }

    fn slider_done(&mut self, resp: &egui::Response) {
        if self.slider_edit == Some(resp.id) && !resp.dragged() && !resp.has_focus() {
            self.slider_edit = None;
        }
    }

    fn timeline_controls(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            let a = self.active;
            self.cmd_button(ui, "⏮", Cmd::FirstFrame);
            self.cmd_button(ui, "⏴", Cmd::PrevFrame);
            let playing = self.docs[a].playing;
            self.cmd_button(ui, if playing { "⏸ Pause" } else { "▶ Play" }, Cmd::PlayToggle);
            self.cmd_button(ui, "⏵", Cmd::NextFrame);
            self.cmd_button(ui, "⏭", Cmd::LastFrame);
            let (frame, n) = (self.docs[a].frame, self.docs[a].sprite.nframes());
            ui.label(format!("Frame {} / {n}", frame + 1));
            let mut ms = self.docs[a].sprite.durations[frame];
            let r = ui
                .add(egui::DragValue::new(&mut ms).range(10..=10000).speed(1.0).suffix(" ms"))
                .on_hover_text("How long this frame is shown. 100 ms = 10 frames per second.");
            if r.changed() {
                self.slider_step(&r);
                self.docs[a].sprite.durations[frame] = ms;
            }
            self.slider_done(&r);
            ui.separator();
            self.cmd_button(ui, "+ Frame", Cmd::NewFrame);
            self.cmd_button(ui, "- Frame", Cmd::DelFrame);
            self.cmd_button(ui, "Tag…", Cmd::NewTag);
            let mut onion = self.onion;
            if ui
                .toggle_value(&mut onion, "Onion skin")
                .on_hover_text("Ghost the previous (red) and next (blue) frames while you draw  (F3)")
                .changed()
            {
                self.pending.push(Cmd::ToggleOnion);
            }
            ui.separator();
            self.cmd_button(ui, "+ Layer", Cmd::NewLayer);
            self.cmd_button(ui, "Group", Cmd::NewGroup);
            self.cmd_button(ui, "Duplicate", Cmd::DupLayer);
            self.cmd_button(ui, "🗑", Cmd::DelLayer);
            self.cmd_button(ui, "⬆", Cmd::LayerUp);
            self.cmd_button(ui, "⬇", Cmd::LayerDown);
            let li = self.docs[a].layer;
            let (mut blend, mut opacity) = (
                self.docs[a].sprite.layers[li].blend,
                self.docs[a].sprite.layers[li].opacity,
            );
            let before = blend;
            egui::ComboBox::from_id_salt("blend")
                .selected_text(BLEND_NAMES[blend as usize])
                .width(96.0)
                .show_ui(ui, |ui| {
                    for (i, name) in BLEND_NAMES.iter().enumerate() {
                        ui.selectable_value(&mut blend, i as u16, *name);
                    }
                })
                .response
                .on_hover_text("Blend mode: how this layer mixes with the layers below it");
            if blend != before {
                let d = &mut self.docs[a];
                d.begin_edit();
                d.sprite.layers[li].blend = blend;
            }
            let mut pct = (opacity as f32 / 255.0 * 100.0).round();
            let r = ui
                .add(egui::Slider::new(&mut pct, 0.0..=100.0).fixed_decimals(0).suffix("%"))
                .on_hover_text("Layer opacity");
            if r.changed() {
                opacity = (pct / 100.0 * 255.0).round() as u8;
                self.slider_step(&r);
                self.docs[a].sprite.layers[li].opacity = opacity;
            }
            self.slider_done(&r);
        });
    }

    fn timeline_grid(&mut self, ui: &mut egui::Ui) {
        const ROW: f32 = 22.0;
        const CELL: f32 = 22.0;
        const LEFT: f32 = 210.0;
        const HEAD: f32 = 20.0;
        const LANE: f32 = 16.0;
        enum Act {
            Cell(usize, usize),
            Vis(usize),
            Lock(usize),
            Props(usize),
            TagEdit(usize),
            Scrolled,
            Run(Cmd),
        }
        let d = &self.docs[self.active];
        let cmds = &self.cmds;
        let sp = &d.sprite;
        let (nl, nf) = (sp.layers.len(), sp.nframes());

        // Stack overlapping tags into lanes.
        let mut lane_end: Vec<usize> = Vec::new();
        let mut tag_lane = vec![0usize; sp.tags.len()];
        let mut order: Vec<usize> = (0..sp.tags.len()).collect();
        order.sort_by_key(|&i| (sp.tags[i].from, std::cmp::Reverse(sp.tags[i].to)));
        for i in order {
            let t = &sp.tags[i];
            let lane = match lane_end.iter().position(|&e| e < t.from) {
                Some(l) => l,
                None => {
                    lane_end.push(0);
                    lane_end.len() - 1
                }
            };
            lane_end[lane] = t.to;
            tag_lane[i] = lane;
        }
        let top_h = HEAD + lane_end.len() as f32 * LANE + 2.0;
        let total_h = top_h + nl as f32 * ROW;
        let mut acts: Vec<Act> = Vec::new();
        let vis = ui.visuals().clone();
        let text_col = vis.text_color();
        let weak_col = vis.weak_text_color();
        let line_col = vis.widgets.noninteractive.bg_stroke.color;
        let sel_fill = vis.selection.bg_fill.gamma_multiply(0.45);
        let font = FontId::proportional(12.5);
        let layer_menu = [
            Cmd::NewLayer,
            Cmd::NewGroup,
            Cmd::DupLayer,
            Cmd::DelLayer,
            Cmd::LayerProps,
            Cmd::LayerUp,
            Cmd::LayerDown,
            Cmd::MergeDown,
        ];
        let frame_menu = [
            Cmd::NewFrame,
            Cmd::NewEmptyFrame,
            Cmd::DelFrame,
            Cmd::FrameProps,
            Cmd::NewTag,
            Cmd::FrameLeft,
            Cmd::FrameRight,
        ];

        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                // ---- layer names
                let (lrect, _) = ui.allocate_exact_size(vec2(LEFT, total_h), Sense::hover());
                ui.painter().text(
                    lrect.min + vec2(50.0, HEAD / 2.0),
                    Align2::LEFT_CENTER,
                    "Layers",
                    font.clone(),
                    weak_col,
                );
                for row in 0..nl {
                    let li = nl - 1 - row;
                    let l = &sp.layers[li];
                    let r = Rect::from_min_size(
                        pos2(lrect.left(), lrect.top() + top_h + row as f32 * ROW),
                        vec2(LEFT, ROW),
                    );
                    if !ui.is_rect_visible(r) {
                        continue;
                    }
                    let p = ui.painter_at(r);
                    if li == d.layer {
                        p.rect_filled(r, 0.0, sel_fill);
                    }
                    p.line_segment([r.left_bottom(), r.right_bottom()], Stroke::new(1.0_f32, line_col));
                    let eye_r = Rect::from_min_size(r.min, vec2(23.0, ROW));
                    let eye = ui.interact(eye_r, ui.id().with(("eye", li)), Sense::click());
                    let shown = sp.layer_visible(li);
                    let ec = if eye.hovered() {
                        text_col
                    } else if l.visible && shown {
                        text_col.gamma_multiply(0.85)
                    } else {
                        weak_col
                    };
                    let c = eye_r.center();
                    if l.visible {
                        p.add(egui::Shape::ellipse_stroke(c, vec2(6.0, 3.8), Stroke::new(1.3_f32, ec)));
                        p.circle_filled(c, 2.0, ec);
                    } else {
                        p.line_segment([c - vec2(5.0, 0.0), c + vec2(5.0, 0.0)], Stroke::new(1.3_f32, weak_col));
                    }
                    if eye
                        .on_hover_text(if l.visible {
                            "Visible — click to hide this layer"
                        } else {
                            "Hidden — click to show this layer"
                        })
                        .clicked()
                    {
                        acts.push(Act::Vis(li));
                    }
                    let lock_r = eye_r.translate(vec2(23.0, 0.0));
                    let lock = ui.interact(lock_r, ui.id().with(("lock", li)), Sense::click());
                    let c = lock_r.center();
                    if !l.editable {
                        p.circle_stroke(c - vec2(0.0, 3.0), 3.0, Stroke::new(1.3_f32, text_col));
                        p.rect_filled(
                            Rect::from_center_size(c + vec2(0.0, 2.0), vec2(9.0, 7.0)),
                            1.5,
                            text_col,
                        );
                    } else {
                        p.circle_filled(
                            c,
                            1.5,
                            if lock.hovered() {
                                text_col
                            } else {
                                weak_col.gamma_multiply(0.6)
                            },
                        );
                    }
                    if lock
                        .on_hover_text(if l.editable {
                            "Unlocked — click to protect this layer from edits"
                        } else {
                            "Locked — click to allow editing"
                        })
                        .clicked()
                    {
                        acts.push(Act::Lock(li));
                    }
                    let name_r = Rect::from_min_max(pos2(r.left() + 48.0, r.top()), r.max);
                    let name = ui.interact(name_r, ui.id().with(("name", li)), Sense::click());
                    let indent = l.level as f32 * 12.0;
                    let label = l.name.clone();
                    let indent = if l.group {
                        let c = pos2(name_r.left() + indent + 6.0, r.center().y);
                        let tri = vec![c + vec2(-4.0, -2.5), c + vec2(4.0, -2.5), c + vec2(0.0, 3.5)];
                        ui.painter_at(name_r)
                            .add(egui::Shape::convex_polygon(tri, weak_col, Stroke::NONE));
                        indent + 13.0
                    } else {
                        indent
                    };
                    let col = if shown { text_col } else { weak_col };
                    ui.painter_at(name_r).text(
                        pos2(name_r.left() + indent + 2.0, r.center().y),
                        Align2::LEFT_CENTER,
                        label,
                        font.clone(),
                        col,
                    );
                    if l.opacity < 255 || l.blend != 0 {
                        let info = if l.blend != 0 {
                            BLEND_NAMES[l.blend as usize].to_string()
                        } else {
                            format!("{}%", (l.opacity as f32 / 2.55).round())
                        };
                        ui.painter_at(name_r).text(
                            pos2(name_r.right() - 4.0, r.center().y),
                            Align2::RIGHT_CENTER,
                            info,
                            FontId::proportional(10.5),
                            weak_col,
                        );
                    }
                    if name.clicked() || name.secondary_clicked() {
                        acts.push(Act::Cell(li, d.frame));
                    }
                    if name.double_clicked() {
                        acts.push(Act::Props(li));
                    }
                    name.context_menu(|ui| {
                        let mut out = Vec::new();
                        menu_cmds(ui, cmds, &layer_menu, &mut out);
                        acts.extend(out.into_iter().map(Act::Run));
                    });
                }
                ui.painter().line_segment(
                    [lrect.right_top(), lrect.right_bottom()],
                    Stroke::new(1.0_f32, line_col),
                );

                // ---- frames
                egui::ScrollArea::horizontal()
                    .id_salt("tl_frames")
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        let (rect, _) = ui.allocate_exact_size(vec2(nf as f32 * CELL + 40.0, total_h), Sense::hover());
                        let clip = ui.clip_rect();
                        let f0 = (((clip.left() - rect.left()) / CELL).floor().max(0.0) as usize).min(nf);
                        let f1 = (((clip.right() - rect.left()) / CELL).ceil().max(0.0) as usize).min(nf);
                        let p = ui.painter_at(rect);
                        let col_x = |f: usize| rect.left() + f as f32 * CELL;
                        // Current-frame column highlight.
                        let cur = Rect::from_min_size(pos2(col_x(d.frame), rect.top()), vec2(CELL, total_h));
                        p.rect_filled(cur, 0.0, sel_fill.gamma_multiply(0.5));
                        for f in f0..f1 {
                            let r = Rect::from_min_size(pos2(col_x(f), rect.top()), vec2(CELL, HEAD));
                            let resp = ui.interact(r, ui.id().with(("fh", f)), Sense::click());
                            if f == d.frame {
                                p.rect_filled(r, 3.0, vis.selection.bg_fill);
                            } else if resp.hovered() {
                                p.rect_filled(r, 3.0, vis.widgets.hovered.weak_bg_fill);
                            }
                            let c = if f == d.frame {
                                vis.selection.stroke.color
                            } else if (f + 1) % 5 == 0 || f == 0 {
                                text_col
                            } else {
                                weak_col
                            };
                            p.text(
                                r.center(),
                                Align2::CENTER_CENTER,
                                (f + 1).to_string(),
                                FontId::proportional(11.0),
                                c,
                            );
                            if resp.clicked() || resp.secondary_clicked() {
                                acts.push(Act::Cell(d.layer, f));
                            }
                            resp.context_menu(|ui| {
                                let mut out = Vec::new();
                                menu_cmds(ui, cmds, &frame_menu, &mut out);
                                acts.extend(out.into_iter().map(Act::Run));
                            });
                        }
                        for (i, t) in sp.tags.iter().enumerate() {
                            let y = rect.top() + HEAD + tag_lane[i] as f32 * LANE;
                            let r = Rect::from_min_max(
                                pos2(col_x(t.from) + 1.0, y + 1.0),
                                pos2(col_x(t.to + 1) - 1.0, y + LANE - 1.0),
                            );
                            if !ui.is_rect_visible(r) {
                                continue;
                            }
                            let tc = if t.color == [0, 0, 0] {
                                Color32::from_rgb(120, 130, 150)
                            } else {
                                Color32::from_rgb(t.color[0], t.color[1], t.color[2])
                            };
                            let resp = ui.interact(r, ui.id().with(("tag", i)), Sense::click());
                            p.rect_filled(r, 3.0, tc.gamma_multiply(if resp.hovered() { 0.75 } else { 0.5 }));
                            p.rect_stroke(r, 3.0, Stroke::new(1.0_f32, tc), StrokeKind::Inside);
                            let lum = t.color[0] as u32 * 3 + t.color[1] as u32 * 6 + t.color[2] as u32;
                            let ink = if vis.dark_mode || lum < 1200 {
                                Color32::WHITE
                            } else {
                                Color32::BLACK
                            };
                            p.with_clip_rect(r.intersect(clip)).text(
                                pos2(r.left().max(clip.left()) + 4.0, r.center().y),
                                Align2::LEFT_CENTER,
                                &t.name,
                                FontId::proportional(10.5),
                                ink,
                            );
                            if resp.clicked() {
                                acts.push(Act::Cell(d.layer, t.from));
                            }
                            if resp.double_clicked() || resp.secondary_clicked() {
                                acts.push(Act::TagEdit(i));
                            }
                            resp.on_hover_text(format!(
                                "Tag “{}” · frames {}–{}\nClick: jump to it · Double-click: edit",
                                t.name,
                                t.from + 1,
                                t.to + 1
                            ));
                        }
                        for row in 0..nl {
                            let li = nl - 1 - row;
                            let l = &sp.layers[li];
                            let y = rect.top() + top_h + row as f32 * ROW;
                            let rr = Rect::from_min_size(pos2(rect.left(), y), vec2(rect.width(), ROW));
                            if !ui.is_rect_visible(rr) {
                                continue;
                            }
                            if li == d.layer {
                                p.rect_filled(rr, 0.0, sel_fill.gamma_multiply(0.5));
                            }
                            p.line_segment([rr.left_bottom(), rr.right_bottom()], Stroke::new(1.0_f32, line_col));
                            for f in f0..f1 {
                                let r = Rect::from_min_size(pos2(col_x(f), y), vec2(CELL, ROW));
                                let resp = ui.interact(r, ui.id().with(("cel", li, f)), Sense::click());
                                let active = li == d.layer && f == d.frame;
                                if active {
                                    p.rect_filled(r.shrink(1.0), 3.0, vis.selection.bg_fill);
                                } else if resp.hovered() {
                                    p.rect_filled(r.shrink(1.0), 3.0, vis.widgets.hovered.weak_bg_fill);
                                }
                                if l.group {
                                    p.line_segment([r.left_center(), r.right_center()], Stroke::new(1.0_f32, line_col));
                                } else {
                                    let c = if active {
                                        vis.selection.stroke.color
                                    } else if sp.layer_visible(li) {
                                        text_col
                                    } else {
                                        weak_col
                                    };
                                    match &l.cels[f] {
                                        Some(cel) => {
                                            // A cel linked to its left neighbor is drawn joined to it.
                                            let linked = f > 0
                                                && l.cels[f - 1]
                                                    .as_ref()
                                                    .is_some_and(|pc| std::sync::Arc::ptr_eq(&pc.image, &cel.image));
                                            if linked {
                                                p.line_segment([r.left_center(), r.center()], Stroke::new(2.0_f32, c));
                                            }
                                            p.circle_filled(r.center(), 4.0, c);
                                        }
                                        None => {
                                            p.circle_stroke(
                                                r.center(),
                                                2.5,
                                                Stroke::new(1.0_f32, weak_col.gamma_multiply(0.6)),
                                            );
                                        }
                                    }
                                }
                                if resp.clicked() || resp.secondary_clicked() {
                                    acts.push(Act::Cell(li, f));
                                }
                                resp.context_menu(|ui| {
                                    let mut out = Vec::new();
                                    menu_cmds(ui, cmds, &frame_menu, &mut out);
                                    ui.separator();
                                    menu_cmds(ui, cmds, &layer_menu[..5], &mut out);
                                    acts.extend(out.into_iter().map(Act::Run));
                                });
                            }
                        }
                        if d.scroll_to_frame {
                            let r =
                                Rect::from_min_size(pos2(col_x(d.frame) - CELL, rect.top()), vec2(CELL * 3.0, HEAD));
                            ui.scroll_to_rect(r, None);
                            acts.push(Act::Scrolled);
                        }
                    });
            });
        });

        let a = self.active;
        for act in acts {
            let d = &mut self.docs[a];
            match act {
                Act::Cell(l, f) => {
                    if f != d.frame {
                        d.playing = false;
                    }
                    let keep_scroll = d.scroll_to_frame;
                    d.set_cursor(l, f);
                    // Clicking a frame shouldn't make the timeline jump.
                    d.scroll_to_frame = keep_scroll;
                }
                Act::Vis(l) => {
                    d.begin_edit();
                    d.sprite.layers[l].visible = !d.sprite.layers[l].visible;
                }
                Act::Lock(l) => {
                    d.begin_edit();
                    d.sprite.layers[l].editable = !d.sprite.layers[l].editable;
                }
                Act::Props(l) => {
                    let layer = &d.sprite.layers[l];
                    self.dialog = Some(Dialog::LayerProps {
                        idx: l,
                        name: layer.name.clone(),
                        opacity: layer.opacity,
                        blend: layer.blend,
                    });
                }
                Act::TagEdit(i) => {
                    let t = &d.sprite.tags[i];
                    self.dialog = Some(Dialog::Tag {
                        idx: Some(i),
                        name: t.name.clone(),
                        from: t.from,
                        to: t.to,
                        dir: t.dir,
                        color: t.color,
                    });
                }
                Act::Scrolled => d.scroll_to_frame = false,
                Act::Run(c) => self.pending.push(c),
            }
        }
    }

    // ------------------------------------------------------------ overlays

    pub fn show_toast(&mut self, ctx: &egui::Context) {
        let Some((msg, at)) = &self.toast else { return };
        let age = self.time - at;
        if age > 4.5 {
            self.toast = None;
            return;
        }
        let alpha = ((4.5 - age) / 0.5).clamp(0.0, 1.0) as f32;
        egui::Area::new(egui::Id::new("toast"))
            .anchor(Align2::CENTER_BOTTOM, vec2(0.0, -46.0))
            .order(egui::Order::Tooltip)
            .interactable(false)
            .show(ctx, |ui| {
                ui.set_opacity(alpha);
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_max_width(520.0);
                    ui.label(msg);
                });
            });
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }

    pub fn show_cmd_palette(&mut self, ctx: &egui::Context) {
        let Some((mut query, mut sel)) = self.cmd_palette.take() else {
            return;
        };
        let mut run: Option<Cmd> = None;
        let resp = egui::Modal::new(egui::Id::new("cmd_palette")).show(ctx, |ui| {
            ui.set_width(440.0);
            ui.label(RichText::new("Command Palette").strong());
            let te = ui.add(
                egui::TextEdit::singleline(&mut query)
                    .hint_text("Type what you want to do — e.g. “flip”, “new layer”, “export”")
                    .desired_width(f32::INFINITY),
            );
            if te.changed() {
                sel = 0;
            }
            te.request_focus();
            let q = query.to_lowercase();
            let words: Vec<&str> = q.split_whitespace().collect();
            let matches: Vec<&CmdInfo> = self
                .cmds
                .iter()
                .filter(|c| !matches!(c.cmd, Cmd::CommandPalette | Cmd::Cancel))
                .filter(|c| {
                    let hay = format!("{} {}", c.cat, c.label).to_lowercase();
                    words.iter().all(|w| hay.contains(w))
                })
                .collect();
            let (up, down, enter) = ui.input(|i| {
                (
                    i.key_pressed(egui::Key::ArrowUp),
                    i.key_pressed(egui::Key::ArrowDown),
                    i.key_pressed(egui::Key::Enter),
                )
            });
            if down {
                sel += 1;
            }
            if up {
                sel = sel.saturating_sub(1);
            }
            sel = sel.min(matches.len().saturating_sub(1));
            ui.add_space(4.0);
            egui::ScrollArea::vertical().max_height(340.0).show(ui, |ui| {
                if matches.is_empty() {
                    ui.weak("No matching command.");
                }
                for (i, c) in matches.iter().enumerate() {
                    let enabled = self.enabled(c.cmd);
                    let mut b = egui::Button::new(format!("{}: {}", c.cat, c.label))
                        .selected(i == sel)
                        .frame_when_inactive(i == sel);
                    if let Some(sc) = &c.shortcut {
                        b = b.shortcut_text(ctx.format_shortcut(sc));
                    }
                    let r = ui.add_enabled(enabled, b.min_size(vec2(ui.available_width(), 0.0)));
                    if i == sel && (up || down) {
                        r.scroll_to_me(None);
                    }
                    if r.clicked() || (enter && i == sel && enabled) {
                        run = Some(c.cmd);
                    }
                }
            });
        });
        if let Some(cmd) = run {
            self.pending.push(cmd);
        } else if !resp.should_close() {
            self.cmd_palette = Some((query, sel));
        }
    }
}
