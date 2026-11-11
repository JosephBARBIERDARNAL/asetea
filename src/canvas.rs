//! The drawing canvas: navigation, tool interaction and rendering.

use crate::app::*;
use crate::doc::*;
use crate::draw;
use eframe::egui::{self, Color32, Pos2, Rect, Stroke, StrokeKind, pos2, vec2};
use std::sync::Arc;

#[derive(Clone, Copy, PartialEq)]
enum PenOp {
    Paint,
    Erase,
    Restore,
}

/// Writes pixels into a cel image, honoring selection, alpha lock and symmetry.
struct Pen<'a> {
    img: &'a mut Image,
    base: &'a Image,
    ox: i32,
    oy: i32,
    w: i32,
    h: i32,
    sel: Option<&'a Mask>,
    color: Rgba,
    op: PenOp,
    alpha_lock: bool,
    sym_h: bool,
    sym_v: bool,
}

impl Pen<'_> {
    fn dot1(&mut self, x: i32, y: i32) {
        if x < 0 || y < 0 || x >= self.w || y >= self.h {
            return;
        }
        if self.sel.is_some_and(|s| !s.get(x, y)) {
            return;
        }
        let (ix, iy) = (x - self.ox, y - self.oy);
        let b = self.base.get(ix, iy);
        if self.alpha_lock && b[3] == 0 {
            return;
        }
        let c = match self.op {
            PenOp::Paint => {
                if self.color[3] == 255 {
                    self.color
                } else {
                    blend(b, self.color, 255, 0)
                }
            }
            PenOp::Erase => [0; 4],
            PenOp::Restore => b,
        };
        self.img.set(ix, iy, c);
    }

    fn dot(&mut self, x: i32, y: i32) {
        self.dot1(x, y);
        let (mx, my) = (self.w - 1 - x, self.h - 1 - y);
        if self.sym_h {
            self.dot1(mx, y);
        }
        if self.sym_v {
            self.dot1(x, my);
        }
        if self.sym_h && self.sym_v {
            self.dot1(mx, my);
        }
    }
}

fn zoom_at(d: &mut Doc, rect: Rect, p: Pos2, z: f32) {
    let z = z.clamp(0.125, 96.0);
    let size = vec2(d.sprite.w as f32, d.sprite.h as f32);
    let origin = rect.center() + d.pan - size * d.zoom / 2.0;
    let s = (p - origin) / d.zoom;
    let origin2 = p - s * z;
    d.pan = origin2 - rect.center() + size * z / 2.0;
    d.zoom = z;
}

fn combine(prev: Option<&Mask>, new: Mask, mode: u8) -> Mask {
    match (prev, mode) {
        (Some(p), 1) => Mask {
            w: p.w,
            h: p.h,
            data: p.data.iter().zip(&new.data).map(|(a, b)| *a || *b).collect(),
        },
        (Some(p), 2) => Mask {
            w: p.w,
            h: p.h,
            data: p.data.iter().zip(&new.data).map(|(a, b)| *a && !*b).collect(),
        },
        (None, 2) => Mask::new(new.w, new.h),
        _ => new,
    }
}

impl App {
    /// Stamp points into the active cel. `brush` applies the brush shape around each point.
    fn stamp(&mut self, base: &Image, color: Rgba, op: PenOp, pts: &[(i32, i32)], brush: bool) {
        let offsets = if brush {
            draw::brush_offsets(self.brush_size, self.brush_square)
        } else {
            vec![(0, 0)]
        };
        let (alpha_lock, sym_h, sym_v) = (self.alpha_lock, self.sym_h, self.sym_v);
        let d = &mut self.docs[self.active];
        let (w, h) = (d.sprite.w, d.sprite.h);
        let Doc {
            sprite,
            sel,
            layer,
            frame,
            ..
        } = d;
        let Some(cel) = sprite.layers[*layer].cels[*frame].as_mut() else {
            return;
        };
        let (ox, oy) = (cel.x, cel.y);
        let mut pen = Pen {
            img: Arc::make_mut(&mut cel.image),
            base,
            ox,
            oy,
            w,
            h,
            sel: sel.as_ref(),
            color,
            op,
            alpha_lock,
            sym_h: sym_h && brush,
            sym_v: sym_v && brush,
        };
        for &(x, y) in pts {
            for &(dx, dy) in &offsets {
                pen.dot(x + dx, y + dy);
            }
        }
        d.touch();
    }

    fn pick(&mut self, p: (i32, i32), secondary: bool) {
        let d = &self.docs[self.active];
        let c = d.sprite.composite(d.frame).get(p.0, p.1);
        if c[3] != 0 {
            if secondary { self.bg = c } else { self.fg = c }
        }
    }

    fn start_move(&mut self, p: (i32, i32)) {
        let a = self.active;
        if self.docs[a].floating.is_some() {
            self.drag = Drag::MoveSel { last: p };
        } else if !self.can_edit() {
        } else if self.docs[a].sel.is_some() {
            if self.docs[a].lift() {
                self.drag = Drag::MoveSel { last: p };
            }
        } else {
            let d = &mut self.docs[a];
            if d.sprite.layers[d.layer].cels[d.frame].is_some() {
                d.begin_edit();
                self.drag = Drag::MoveCel { last: p };
            } else {
                self.toast("There is nothing on this layer in this frame to move.");
            }
        }
    }

    pub fn press(
        &mut self,
        p: (i32, i32),
        secondary: bool,
        mods: egui::Modifiers,
        tool: Tool,
        screen: Pos2,
        rect: Rect,
    ) {
        let a = self.active;
        let mode = if mods.shift {
            1
        } else if mods.alt {
            2
        } else {
            0
        };
        self.docs[a].playing = false;
        match tool {
            Tool::Hand => self.drag = Drag::Pan,
            Tool::Zoom => {
                let d = &mut self.docs[a];
                let z = zoom_step(d.zoom, !secondary);
                zoom_at(d, rect, screen, z);
            }
            Tool::Eyedropper => {
                self.pick(p, secondary);
                self.drag = Drag::Pick { secondary };
            }
            Tool::Pencil | Tool::Eraser => {
                if !self.can_edit() {
                    return;
                }
                let color = if secondary { self.bg } else { self.fg };
                let d = &mut self.docs[a];
                d.commit_floating();
                d.begin_edit();
                let base = d.sprite.edit_cel(d.layer, d.frame).image.clone();
                let from = if mods.shift && tool == Tool::Pencil {
                    d.last_point
                } else {
                    None
                };
                let erase = tool == Tool::Eraser;
                let op = if erase { PenOp::Erase } else { PenOp::Paint };
                let mut pts = Vec::new();
                match from {
                    Some(f) => draw::line(f.0, f.1, p.0, p.1, |x, y| pts.push((x, y))),
                    None => pts.push(p),
                }
                self.stamp(&base, color, op, &pts, true);
                self.drag = Drag::Paint {
                    base,
                    last: p,
                    color,
                    erase,
                    trail: vec![p],
                };
            }
            Tool::Bucket => {
                if !self.can_edit() {
                    return;
                }
                let color = if secondary { self.bg } else { self.fg };
                let (tol, contiguous) = (self.tolerance, self.contiguous);
                let d = &mut self.docs[a];
                d.commit_floating();
                d.begin_edit();
                let (w, h) = (d.sprite.w, d.sprite.h);
                let cel = d.sprite.edit_cel(d.layer, d.frame);
                let (cx, cy) = (cel.x, cel.y);
                let base = cel.image.clone();
                let m = draw::flood(w, h, |x, y| base.get(x - cx, y - cy), p.0, p.1, tol, contiguous);
                let mut pts = Vec::new();
                for y in 0..h {
                    for x in 0..w {
                        if m.get(x, y) {
                            pts.push((x, y));
                        }
                    }
                }
                self.stamp(&base, color, PenOp::Paint, &pts, false);
            }
            Tool::Line | Tool::Rect | Tool::Ellipse | Tool::Gradient => {
                if !self.can_edit() {
                    return;
                }
                let (color, other) = if secondary {
                    (self.bg, self.fg)
                } else {
                    (self.fg, self.bg)
                };
                let d = &mut self.docs[a];
                d.commit_floating();
                d.begin_edit();
                let base = d.sprite.edit_cel(d.layer, d.frame).image.clone();
                self.drag = Drag::Shape {
                    base,
                    start: p,
                    color,
                    other,
                };
                self.drag_to(p, mods, egui::Vec2::ZERO);
            }
            Tool::Marquee | Tool::Lasso => {
                let d = &mut self.docs[a];
                let inside = d.sel.as_ref().is_some_and(|s| s.get(p.0, p.1));
                if mode == 0 && inside {
                    self.start_move(p);
                    return;
                }
                d.commit_floating();
                let prev = if mode == 0 { None } else { d.sel.clone() };
                self.drag = if tool == Tool::Marquee {
                    Drag::Marquee { start: p, prev, mode }
                } else {
                    Drag::Lasso {
                        pts: vec![p],
                        prev,
                        mode,
                    }
                };
            }
            Tool::Wand => {
                let (tol, contiguous) = (self.tolerance, self.contiguous);
                let d = &mut self.docs[a];
                d.commit_floating();
                let (w, h) = (d.sprite.w, d.sprite.h);
                let img = match &d.sprite.layers[d.layer].cels[d.frame] {
                    Some(c) => c.image.crop(-c.x, -c.y, w, h),
                    None => Image::new(w, h),
                };
                let m = draw::flood(w, h, |x, y| img.get(x, y), p.0, p.1, tol, contiguous);
                let m = combine(d.sel.as_ref(), m, mode);
                d.set_sel(Some(m));
            }
            Tool::Move => self.start_move(p),
        }
    }

    pub fn drag_to(&mut self, p: (i32, i32), mods: egui::Modifiers, screen_delta: egui::Vec2) {
        let a = self.active;
        let mut drag = std::mem::replace(&mut self.drag, Drag::None);
        match &mut drag {
            Drag::None => {}
            Drag::Pan => self.docs[a].pan += screen_delta,
            Drag::Pick { secondary } => self.pick(p, *secondary),
            Drag::Paint {
                base,
                last,
                color,
                erase,
                trail,
            } => {
                if *last != p {
                    let op = if *erase { PenOp::Erase } else { PenOp::Paint };
                    let mut pts = Vec::new();
                    draw::line(last.0, last.1, p.0, p.1, |x, y| pts.push((x, y)));
                    pts.remove(0);
                    if self.pixel_perfect && self.brush_size == 1 {
                        // Drop the corner pixel of every "L" so lines stay one pixel thin.
                        for &c in &pts {
                            trail.push(c);
                            let n = trail.len();
                            if n >= 3 {
                                let (pa, pb) = (trail[n - 3], trail[n - 2]);
                                if (pa.0 == pb.0 || pa.1 == pb.1)
                                    && (pb.0 == c.0 || pb.1 == c.1)
                                    && pa.0 != c.0
                                    && pa.1 != c.1
                                {
                                    self.stamp(base, *color, PenOp::Restore, &[pb], true);
                                    trail.remove(n - 2);
                                }
                            }
                            self.stamp(base, *color, op, &[c], true);
                        }
                        let keep = trail.len().saturating_sub(3);
                        trail.drain(..keep);
                    } else {
                        self.stamp(base, *color, op, &pts, true);
                    }
                    *last = p;
                }
            }
            Drag::Shape {
                base,
                start,
                color,
                other,
            } => {
                let d = &mut self.docs[a];
                let (w, h) = (d.sprite.w, d.sprite.h);
                if let Some(cel) = d.sprite.layers[d.layer].cels[d.frame].as_mut() {
                    cel.image = base.clone();
                }
                let s = *start;
                let e = match (self.tool, mods.shift) {
                    (Tool::Line | Tool::Gradient, true) => draw::snap_angle(s, p),
                    (Tool::Rect | Tool::Ellipse, true) => draw::snap_square(s, p),
                    _ => p,
                };
                self.drag_size = Some(((e.0 - s.0).abs() + 1, (e.1 - s.1).abs() + 1));
                let mut pts = Vec::new();
                match self.tool {
                    Tool::Line => {
                        draw::line(s.0, s.1, e.0, e.1, |x, y| pts.push((x, y)));
                        self.stamp(base, *color, PenOp::Paint, &pts, true);
                    }
                    Tool::Rect => {
                        draw::rect(s.0, s.1, e.0, e.1, self.filled, |x, y| pts.push((x, y)));
                        self.stamp(base, *color, PenOp::Paint, &pts, true);
                    }
                    Tool::Ellipse => {
                        draw::ellipse(s.0, s.1, e.0, e.1, self.filled, |x, y| pts.push((x, y)));
                        self.stamp(base, *color, PenOp::Paint, &pts, true);
                    }
                    _ => {
                        // Gradient: project each pixel onto the drag vector.
                        let (vx, vy) = ((e.0 - s.0) as f32, (e.1 - s.1) as f32);
                        let len2 = (vx * vx + vy * vy).max(1.0);
                        let (c0, c1) = (*color, *other);
                        let (mut first, mut second) = (Vec::new(), Vec::new());
                        for y in 0..h {
                            for x in 0..w {
                                let t = (((x - s.0) as f32 * vx + (y - s.1) as f32 * vy) / len2).clamp(0.0, 1.0);
                                if self.dither {
                                    let thr = (draw::BAYER4[(y & 3) as usize][(x & 3) as usize] as f32 + 0.5) / 16.0;
                                    if t < thr {
                                        first.push((x, y))
                                    } else {
                                        second.push((x, y))
                                    }
                                } else {
                                    let mut c = [0u8; 4];
                                    for i in 0..4 {
                                        c[i] = (c0[i] as f32 + (c1[i] as f32 - c0[i] as f32) * t).round() as u8;
                                    }
                                    self.stamp(base, c, PenOp::Paint, &[(x, y)], false);
                                }
                            }
                        }
                        self.stamp(base, c0, PenOp::Paint, &first, false);
                        self.stamp(base, c1, PenOp::Paint, &second, false);
                    }
                }
                self.docs[a].touch();
            }
            Drag::Marquee { start, prev, mode } => {
                let d = &mut self.docs[a];
                let (w, h) = (d.sprite.w, d.sprite.h);
                let q = if mods.shift && *mode == 0 {
                    draw::snap_square(*start, p)
                } else {
                    p
                };
                let (x0, y0) = (start.0.min(q.0).max(0), start.1.min(q.1).max(0));
                let (x1, y1) = (start.0.max(q.0).min(w - 1), start.1.max(q.1).min(h - 1));
                let m = Mask::rect(w, h, x0, y0, x1 - x0 + 1, y1 - y0 + 1);
                self.drag_size = Some(((x1 - x0 + 1).max(0), (y1 - y0 + 1).max(0)));
                d.set_sel(Some(combine(prev.as_ref(), m, *mode)));
            }
            Drag::Lasso { pts, .. } => {
                if pts.last() != Some(&p) {
                    pts.push(p);
                }
            }
            Drag::MoveSel { last } => {
                let (dx, dy) = (p.0 - last.0, p.1 - last.1);
                if dx != 0 || dy != 0 {
                    let d = &mut self.docs[a];
                    let (w, h) = (d.sprite.w, d.sprite.h);
                    if let Some(fl) = &mut d.floating {
                        fl.x += dx;
                        fl.y += dy;
                        let mut m = Mask::new(w, h);
                        for y in 0..fl.img.h {
                            for x in 0..fl.img.w {
                                if fl.shape[(y * fl.img.w + x) as usize] {
                                    m.set(fl.x + x, fl.y + y, true);
                                }
                            }
                        }
                        d.set_sel(Some(m));
                        d.touch();
                    }
                    *last = p;
                }
            }
            Drag::MoveCel { last } => {
                let d = &mut self.docs[a];
                if let Some(cel) = d.sprite.layers[d.layer].cels[d.frame].as_mut() {
                    cel.x += p.0 - last.0;
                    cel.y += p.1 - last.1;
                }
                d.touch();
                *last = p;
            }
        }
        self.drag = drag;
    }

    pub fn release(&mut self, p: (i32, i32)) {
        let a = self.active;
        self.drag_size = None;
        match std::mem::replace(&mut self.drag, Drag::None) {
            Drag::Paint { last, .. } => self.docs[a].last_point = Some(last),
            Drag::Marquee { start, mode, .. } => {
                if start == p && mode == 0 {
                    self.docs[a].set_sel(None);
                }
            }
            Drag::Lasso { pts, prev, mode } => {
                let d = &mut self.docs[a];
                if pts.len() < 3 {
                    if mode == 0 {
                        d.set_sel(None);
                    }
                } else {
                    let m = draw::polygon_mask(&pts, d.sprite.w, d.sprite.h);
                    d.set_sel(Some(combine(prev.as_ref(), m, mode)));
                }
            }
            _ => {}
        }
    }

    pub fn canvas(&mut self, ui: &mut egui::Ui) {
        let rect = ui.available_rect_before_wrap();
        let resp = ui.allocate_rect(rect, egui::Sense::drag());
        let painter = ui.painter_at(rect);
        let dark = ui.visuals().dark_mode;
        painter.rect_filled(
            rect,
            0.0,
            if dark {
                Color32::from_gray(34)
            } else {
                Color32::from_gray(205)
            },
        );
        let ctx = ui.ctx().clone();
        let a = self.active;
        let (w, h) = (self.docs[a].sprite.w, self.docs[a].sprite.h);

        if self.docs[a].fit_pending && rect.width() > 50.0 {
            let d = &mut self.docs[a];
            let z = ((rect.width() - 48.0) / w as f32).min((rect.height() - 48.0) / h as f32);
            d.zoom = ZOOMS.iter().rev().copied().find(|&l| l <= z).unwrap_or(0.125);
            d.pan = egui::Vec2::ZERO;
            d.fit_pending = false;
        }

        // ---- navigation
        let (scroll, zoom_delta, wheel, hover, mods, space) = ctx.input(|i| {
            let mut wheel = 0.0;
            for e in &i.events {
                if let egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Line,
                    delta,
                    modifiers,
                    ..
                } = e
                    && !modifiers.command
                    && !modifiers.ctrl
                {
                    wheel += delta.y;
                }
            }
            (
                i.smooth_scroll_delta,
                i.zoom_delta(),
                wheel,
                i.pointer.hover_pos(),
                i.modifiers,
                i.key_down(egui::Key::Space),
            )
        });
        let over = hover.filter(|p| rect.contains(*p) && resp.contains_pointer());
        if let Some(p) = over {
            let d = &mut self.docs[a];
            if zoom_delta != 1.0 {
                let z = d.zoom * zoom_delta;
                zoom_at(d, rect, p, z);
            }
            if wheel != 0.0 {
                let z = zoom_step(d.zoom, wheel > 0.0);
                zoom_at(d, rect, p, z);
                self.wheel_zoom_until = self.time + 0.3;
            } else if self.time > self.wheel_zoom_until && scroll != egui::Vec2::ZERO && zoom_delta == 1.0 {
                d.pan += scroll;
            }
        }

        let zoom = self.docs[a].zoom;
        let size = vec2(w as f32, h as f32) * zoom;
        let origin = (rect.center() + self.docs[a].pan - size / 2.0).round();
        let canvas_rect = Rect::from_min_size(origin, size);
        let to_px = |p: Pos2| {
            (
                ((p.x - origin.x) / zoom).floor() as i32,
                ((p.y - origin.y) / zoom).floor() as i32,
            )
        };
        let to_screen = |x: f32, y: f32| pos2(origin.x + x * zoom, origin.y + y * zoom);

        // ---- tool interaction
        let paint_tool = matches!(
            self.tool,
            Tool::Pencil | Tool::Eraser | Tool::Bucket | Tool::Line | Tool::Rect | Tool::Ellipse
        );
        let tool = if space {
            Tool::Hand
        } else if mods.alt && paint_tool {
            Tool::Eyedropper
        } else {
            self.tool
        };
        if let Some(pos) = resp.interact_pointer_pos() {
            let p = to_px(pos);
            if resp.drag_started() {
                ctx.memory_mut(|m| {
                    if let Some(id) = m.focused() {
                        m.surrender_focus(id);
                    }
                });
            }
            if resp.drag_started_by(egui::PointerButton::Middle) {
                self.drag = Drag::Pan;
            } else if resp.drag_started_by(egui::PointerButton::Primary) {
                self.press(p, false, mods, tool, pos, rect);
            } else if resp.drag_started_by(egui::PointerButton::Secondary) {
                self.press(p, true, mods, tool, pos, rect);
            } else if resp.dragged() {
                self.drag_to(p, mods, resp.drag_delta());
            }
            if resp.drag_stopped() {
                self.release(p);
            }
        } else if !matches!(self.drag, Drag::None) && !ctx.input(|i| i.pointer.any_down()) {
            self.release((0, 0));
        }
        self.hover_px = over.map(to_px);
        if over.is_some() {
            ctx.set_cursor_icon(match tool {
                Tool::Hand => {
                    if matches!(self.drag, Drag::Pan) {
                        egui::CursorIcon::Grabbing
                    } else {
                        egui::CursorIcon::Grab
                    }
                }
                Tool::Zoom => egui::CursorIcon::ZoomIn,
                Tool::Move => egui::CursorIcon::Move,
                _ => egui::CursorIcon::Crosshair,
            });
        }

        // ---- rendering
        let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
        let onion = self.onion;
        let d = &mut self.docs[a];
        let cell = (w.max(h) / 16).clamp(1, 16);
        let checker = d
            .tex_checker
            .get(&ctx, "checker", (((w as u64) << 20) | h as u64, dark as u64), || {
                let (c0, c1) = if dark { (96, 122) } else { (190, 225) };
                let mut img = Image::new(w, h);
                for y in 0..h {
                    for x in 0..w {
                        let v = if ((x / cell) + (y / cell)) % 2 == 0 { c0 } else { c1 };
                        img.set(x, y, [v, v, v, 255]);
                    }
                }
                img
            });
        painter.image(checker, canvas_rect, uv, Color32::WHITE);

        let frame = d.frame;
        let key = (d.rev, frame as u64);
        let playing = d.playing;
        let Doc {
            sprite,
            tex,
            tex_onion_prev,
            tex_onion_next,
            tex_float,
            floating,
            ..
        } = &mut *d;
        let main = tex.get(&ctx, "canvas", key, || sprite.composite(frame));
        painter.image(main, canvas_rect, uv, Color32::WHITE);

        if onion && !playing && sprite.nframes() > 1 {
            // Ghost only the pixels that differ from the current frame: red = previous, blue = next.
            let ghost = |other: usize, tint: [u32; 3]| {
                let cur = sprite.composite(frame);
                let mut o = sprite.composite(other);
                for (p, c) in o.data.chunks_exact_mut(4).zip(cur.data.chunks_exact(4)) {
                    if p[3] == 0 || p == c {
                        p.copy_from_slice(&[0; 4]);
                    } else {
                        for i in 0..3 {
                            p[i] = ((p[i] as u32 + tint[i]) / 2) as u8;
                        }
                        p[3] = (p[3] as u32 * 120 / 255) as u8;
                    }
                }
                o
            };
            if frame > 0 {
                let t = tex_onion_prev.get(&ctx, "onion_prev", key, || ghost(frame - 1, [255, 40, 40]));
                painter.image(t, canvas_rect, uv, Color32::WHITE);
            }
            if frame + 1 < sprite.nframes() {
                let t = tex_onion_next.get(&ctx, "onion_next", key, || ghost(frame + 1, [40, 90, 255]));
                painter.image(t, canvas_rect, uv, Color32::WHITE);
            }
        }

        if let Some(fl) = floating {
            let t = tex_float.get(&ctx, "floating", (fl.id, 0), || fl.img.clone());
            let r = Rect::from_min_size(
                to_screen(fl.x as f32, fl.y as f32),
                vec2(fl.img.w as f32, fl.img.h as f32) * zoom,
            );
            painter.image(t, r, uv, Color32::WHITE);
        }

        // Grids (only the visible part).
        let vis = canvas_rect.intersect(rect);
        if vis.is_positive() {
            let (px0, py0) = to_px(vis.min);
            let (px1, py1) = to_px(vis.max);
            let lines = |step_x: i32, step_y: i32, off: (i32, i32), stroke: Stroke| {
                let mut x = px0.max(0) - (px0.max(0) - off.0).rem_euclid(step_x) + step_x;
                while x <= px1.min(w - 1) {
                    let sx = origin.x + x as f32 * zoom;
                    painter.line_segment([pos2(sx, vis.min.y), pos2(sx, vis.max.y)], stroke);
                    x += step_x;
                }
                let mut y = py0.max(0) - (py0.max(0) - off.1).rem_euclid(step_y) + step_y;
                while y <= py1.min(h - 1) {
                    let sy = origin.y + y as f32 * zoom;
                    painter.line_segment([pos2(vis.min.x, sy), pos2(vis.max.x, sy)], stroke);
                    y += step_y;
                }
            };
            if self.show_pixel_grid && zoom >= 12.0 {
                lines(1, 1, (0, 0), Stroke::new(1.0_f32, Color32::from_black_alpha(36)));
            }
            let g = sprite.grid;
            if self.show_grid && g.2 as f32 * zoom >= 4.0 && g.3 as f32 * zoom >= 4.0 {
                lines(
                    g.2.max(1) as i32,
                    g.3.max(1) as i32,
                    (g.0 as i32, g.1 as i32),
                    Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(70, 150, 255, 150)),
                );
            }
        }
        painter.rect_stroke(
            canvas_rect.expand(1.0),
            0.0,
            Stroke::new(1.0_f32, Color32::from_black_alpha(160)),
            StrokeKind::Outside,
        );

        // Symmetry axes.
        let axis = Stroke::new(1.5_f32, Color32::from_rgb(255, 80, 200));
        if self.sym_h {
            let x = origin.x + size.x / 2.0;
            painter.line_segment(
                [pos2(x, canvas_rect.top() - 8.0), pos2(x, canvas_rect.bottom() + 8.0)],
                axis,
            );
        }
        if self.sym_v {
            let y = origin.y + size.y / 2.0;
            painter.line_segment(
                [pos2(canvas_rect.left() - 8.0, y), pos2(canvas_rect.right() + 8.0, y)],
                axis,
            );
        }

        // Selection outline ("marching ants").
        if let Some(sel) = &d.sel {
            if d.sel_edges.0 != d.sel_gen {
                d.sel_edges = (d.sel_gen, sel.edges());
            }
            let off = (self.time * 12.0 % 8.0) as f32;
            let mut shapes = Vec::new();
            for e in &d.sel_edges.1 {
                let (p0, p1) = (to_screen(e[0] as f32, e[1] as f32), to_screen(e[2] as f32, e[3] as f32));
                if !rect.expand(4.0).intersects(Rect::from_two_pos(p0, p1)) {
                    continue;
                }
                shapes.push(egui::Shape::line_segment(
                    [p0, p1],
                    Stroke::new(1.0_f32, Color32::BLACK),
                ));
                shapes.extend(egui::Shape::dashed_line_with_offset(
                    &[p0, p1],
                    Stroke::new(1.0_f32, Color32::WHITE),
                    &[4.0],
                    &[4.0],
                    off,
                ));
            }
            painter.extend(shapes);
            ctx.request_repaint_after(std::time::Duration::from_millis(90));
        }
        if let Drag::Lasso { pts, .. } = &self.drag {
            let line: Vec<Pos2> = pts
                .iter()
                .map(|p| to_screen(p.0 as f32 + 0.5, p.1 as f32 + 0.5))
                .collect();
            painter.add(egui::Shape::line(line.clone(), Stroke::new(2.0_f32, Color32::BLACK)));
            painter.add(egui::Shape::line(line, Stroke::new(1.0_f32, Color32::WHITE)));
        }

        // Brush / pixel preview under the cursor.
        if let (Some(p), true) = (self.hover_px, matches!(self.drag, Drag::None)) {
            let px_rect = |x: i32, y: i32| Rect::from_min_size(to_screen(x as f32, y as f32), vec2(zoom, zoom));
            let brush_tool = matches!(
                tool,
                Tool::Pencil | Tool::Eraser | Tool::Line | Tool::Rect | Tool::Ellipse
            );
            if brush_tool && zoom >= 2.0 {
                let c = if tool == Tool::Eraser {
                    Color32::from_white_alpha(90)
                } else {
                    Color32::from_rgba_unmultiplied(self.fg[0], self.fg[1], self.fg[2], 170)
                };
                let offs = draw::brush_offsets(self.brush_size, self.brush_square);
                for (dx, dy) in &offs {
                    painter.rect_filled(px_rect(p.0 + dx, p.1 + dy), 0.0, c);
                }
                let lo = -(self.brush_size / 2);
                let r = Rect::from_min_size(
                    to_screen((p.0 + lo) as f32, (p.1 + lo) as f32),
                    vec2(zoom, zoom) * self.brush_size as f32,
                );
                painter.rect_stroke(
                    r,
                    0.0,
                    Stroke::new(1.0_f32, Color32::from_white_alpha(140)),
                    StrokeKind::Outside,
                );
                painter.rect_stroke(
                    r.expand(1.0),
                    0.0,
                    Stroke::new(1.0_f32, Color32::from_black_alpha(140)),
                    StrokeKind::Outside,
                );
            } else if !matches!(tool, Tool::Hand | Tool::Zoom) && zoom >= 4.0 {
                painter.rect_stroke(
                    px_rect(p.0, p.1),
                    0.0,
                    Stroke::new(1.0_f32, Color32::from_white_alpha(170)),
                    StrokeKind::Outside,
                );
            }
        }
    }
}
