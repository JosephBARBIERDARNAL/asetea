//! Application state, documents, commands and the main update loop.

use crate::doc::*;
use crate::{ase, draw, export, palettes};
use eframe::egui::{self, Key, KeyboardShortcut, Modifiers};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);
fn next_id() -> u64 {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

pub const ZOOMS: [f32; 17] = [
    0.125, 0.25, 0.5, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 8.0, 12.0, 16.0, 24.0, 32.0, 48.0, 64.0, 96.0,
];

pub fn zoom_step(z: f32, up: bool) -> f32 {
    if up {
        ZOOMS.iter().copied().find(|&l| l > z * 1.001).unwrap_or(96.0)
    } else {
        ZOOMS.iter().rev().copied().find(|&l| l < z * 0.999).unwrap_or(0.125)
    }
}

// ---------------------------------------------------------------- Tools

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Tool {
    Pencil,
    Eraser,
    Bucket,
    Eyedropper,
    Line,
    Rect,
    Ellipse,
    Gradient,
    Marquee,
    Lasso,
    Wand,
    Move,
    Hand,
    Zoom,
}

impl Tool {
    pub const ALL: [Tool; 14] = [
        Tool::Pencil,
        Tool::Eraser,
        Tool::Bucket,
        Tool::Eyedropper,
        Tool::Line,
        Tool::Rect,
        Tool::Ellipse,
        Tool::Gradient,
        Tool::Marquee,
        Tool::Lasso,
        Tool::Wand,
        Tool::Move,
        Tool::Hand,
        Tool::Zoom,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Tool::Pencil => "Pencil",
            Tool::Eraser => "Eraser",
            Tool::Bucket => "Paint Bucket",
            Tool::Eyedropper => "Eyedropper",
            Tool::Line => "Line",
            Tool::Rect => "Rectangle",
            Tool::Ellipse => "Ellipse",
            Tool::Gradient => "Gradient",
            Tool::Marquee => "Rectangular Marquee",
            Tool::Lasso => "Lasso",
            Tool::Wand => "Magic Wand",
            Tool::Move => "Move",
            Tool::Hand => "Hand",
            Tool::Zoom => "Zoom",
        }
    }

    pub fn key(self) -> (Modifiers, Key, &'static str) {
        let n = Modifiers::NONE;
        match self {
            Tool::Pencil => (n, Key::B, "B"),
            Tool::Eraser => (n, Key::E, "E"),
            Tool::Bucket => (n, Key::G, "G"),
            Tool::Eyedropper => (n, Key::I, "I"),
            Tool::Line => (n, Key::L, "L"),
            Tool::Rect => (n, Key::U, "U"),
            Tool::Ellipse => (n, Key::O, "O"),
            Tool::Gradient => (Modifiers::SHIFT, Key::G, "Shift+G"),
            Tool::Marquee => (n, Key::M, "M"),
            Tool::Lasso => (n, Key::Q, "Q"),
            Tool::Wand => (n, Key::W, "W"),
            Tool::Move => (n, Key::V, "V"),
            Tool::Hand => (n, Key::H, "H"),
            Tool::Zoom => (n, Key::Z, "Z"),
        }
    }

    pub fn help(self) -> &'static str {
        match self {
            Tool::Pencil => {
                "Click and drag to draw. Right-click draws with the background color. Shift+click draws a straight line from the last point. Hold Alt to pick a color."
            }
            Tool::Eraser => "Click and drag to erase pixels to transparent.",
            Tool::Bucket => "Click to fill an area with the foreground color (right-click: background color).",
            Tool::Eyedropper => "Click a pixel to make it the foreground color (right-click: background color).",
            Tool::Line => "Drag to draw a straight line. Hold Shift to snap to 45° angles.",
            Tool::Rect => "Drag to draw a rectangle. Hold Shift for a square.",
            Tool::Ellipse => "Drag to draw an ellipse. Hold Shift for a circle.",
            Tool::Gradient => {
                "Drag to fill the selection (or the canvas) with a gradient from foreground to background color."
            }
            Tool::Marquee => {
                "Drag to select a rectangle. Shift adds, Alt subtracts. Drag inside a selection to move its pixels."
            }
            Tool::Lasso => "Draw a freehand outline to select. Shift adds, Alt subtracts.",
            Tool::Wand => "Click to select an area of similar color. Shift adds, Alt subtracts.",
            Tool::Move => "Drag to move the selected pixels, or the whole cel when nothing is selected.",
            Tool::Hand => {
                "Drag to scroll the canvas. Tip: hold Space with any tool, or drag with the middle mouse button."
            }
            Tool::Zoom => "Click to zoom in, right-click to zoom out. Tip: scroll wheel or pinch also zooms.",
        }
    }
}

// ---------------------------------------------------------------- Commands

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cmd {
    New,
    Open,
    Save,
    SaveAs,
    ExportImage,
    ExportSheet,
    CloseDoc,
    Quit,
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    Clear,
    FillSel,
    SelectAll,
    Deselect,
    InvertSel,
    FlipH,
    FlipV,
    Rot90Cw,
    Rot90Ccw,
    Rot180,
    Invert,
    Desaturate,
    ReplaceColor,
    Outline,
    CanvasSize,
    SpriteSize,
    CropToSel,
    Trim,
    ModeRgb,
    ModeIndexed,
    ModeGray,
    GridSettings,
    NewLayer,
    NewGroup,
    DupLayer,
    DelLayer,
    MergeDown,
    Flatten,
    LayerUp,
    LayerDown,
    LayerProps,
    SelLayerAbove,
    SelLayerBelow,
    NewFrame,
    NewEmptyFrame,
    DelFrame,
    FrameLeft,
    FrameRight,
    NextFrame,
    PrevFrame,
    FirstFrame,
    LastFrame,
    PlayToggle,
    NewTag,
    FrameProps,
    ZoomIn,
    ZoomOut,
    ZoomFit,
    Zoom100,
    ToggleGrid,
    TogglePixelGrid,
    ToggleOnion,
    SymH,
    SymV,
    ToggleTheme,
    SwapColors,
    DefaultColors,
    BrushInc,
    BrushDec,
    ImportPalette,
    ExportPalette,
    PaletteFromSprite,
    CommandPalette,
    Shortcuts,
    About,
    Cancel,
    Tool(Tool),
}

pub struct CmdInfo {
    pub cmd: Cmd,
    pub label: &'static str,
    pub cat: &'static str,
    pub shortcut: Option<KeyboardShortcut>,
}

fn build_commands() -> (Vec<CmdInfo>, Vec<(KeyboardShortcut, Cmd)>) {
    let n = Modifiers::NONE;
    let c = Modifiers::COMMAND;
    let s = Modifiers::SHIFT;
    let a = Modifiers::ALT;
    let cs = c | s;
    let sc = |m: Modifiers, k: Key| Some(KeyboardShortcut::new(m, k));
    let mut v: Vec<CmdInfo> = Vec::new();
    let mut add = |cmd, cat, label, shortcut| {
        v.push(CmdInfo {
            cmd,
            label,
            cat,
            shortcut,
        })
    };
    add(Cmd::New, "File", "New Sprite…", sc(c, Key::N));
    add(Cmd::Open, "File", "Open…", sc(c, Key::O));
    add(Cmd::Save, "File", "Save", sc(c, Key::S));
    add(Cmd::SaveAs, "File", "Save As…", sc(cs, Key::S));
    add(Cmd::ExportImage, "File", "Export Image / Animation…", sc(cs, Key::E));
    add(Cmd::ExportSheet, "File", "Export Sprite Sheet…", sc(c, Key::E));
    add(Cmd::CloseDoc, "File", "Close", sc(c, Key::W));
    add(Cmd::Quit, "File", "Quit", sc(c, Key::Q));
    add(Cmd::Undo, "Edit", "Undo", sc(c, Key::Z));
    add(Cmd::Redo, "Edit", "Redo", sc(cs, Key::Z));
    add(Cmd::Cut, "Edit", "Cut", sc(c, Key::X));
    add(Cmd::Copy, "Edit", "Copy", sc(c, Key::C));
    add(Cmd::Paste, "Edit", "Paste", sc(c, Key::V));
    add(Cmd::Clear, "Edit", "Delete Pixels", sc(n, Key::Delete));
    add(Cmd::FillSel, "Edit", "Fill with Foreground Color", sc(n, Key::F));
    add(Cmd::FlipH, "Edit", "Flip Horizontal", sc(s, Key::H));
    add(Cmd::FlipV, "Edit", "Flip Vertical", sc(s, Key::V));
    add(Cmd::Invert, "Edit", "Invert Colors", sc(c, Key::I));
    add(Cmd::Desaturate, "Edit", "Desaturate", None);
    add(Cmd::ReplaceColor, "Edit", "Replace Color…", sc(s, Key::R));
    add(Cmd::Outline, "Edit", "Outline with Foreground Color", sc(s, Key::O));
    add(Cmd::SelectAll, "Select", "Select All", sc(c, Key::A));
    add(Cmd::Deselect, "Select", "Deselect", sc(c, Key::D));
    add(Cmd::InvertSel, "Select", "Invert Selection", sc(cs, Key::I));
    add(Cmd::CanvasSize, "Sprite", "Canvas Size…", sc(n, Key::C));
    add(Cmd::SpriteSize, "Sprite", "Sprite Size (Scale)…", None);
    add(Cmd::CropToSel, "Sprite", "Crop to Selection", None);
    add(Cmd::Trim, "Sprite", "Trim Empty Borders", None);
    add(Cmd::Rot90Cw, "Sprite", "Rotate 90° Clockwise", None);
    add(Cmd::Rot90Ccw, "Sprite", "Rotate 90° Counter-clockwise", None);
    add(Cmd::Rot180, "Sprite", "Rotate 180°", None);
    add(Cmd::ModeRgb, "Sprite", "Color Mode: RGB", None);
    add(Cmd::ModeIndexed, "Sprite", "Color Mode: Indexed (palette)", None);
    add(Cmd::ModeGray, "Sprite", "Color Mode: Grayscale", None);
    add(Cmd::GridSettings, "Sprite", "Grid Settings…", None);
    add(Cmd::NewLayer, "Layer", "New Layer", sc(s, Key::N));
    add(Cmd::NewGroup, "Layer", "New Group", sc(a | s, Key::N));
    add(Cmd::DupLayer, "Layer", "Duplicate Layer", None);
    add(Cmd::DelLayer, "Layer", "Delete Layer", None);
    add(Cmd::MergeDown, "Layer", "Merge Down", sc(cs, Key::M));
    add(Cmd::Flatten, "Layer", "Flatten All Layers", None);
    add(Cmd::LayerUp, "Layer", "Move Layer Up", sc(c, Key::ArrowUp));
    add(Cmd::LayerDown, "Layer", "Move Layer Down", sc(c, Key::ArrowDown));
    add(Cmd::LayerProps, "Layer", "Layer Properties…", sc(s, Key::P));
    add(Cmd::SelLayerAbove, "Layer", "Select Layer Above", sc(n, Key::ArrowUp));
    add(Cmd::SelLayerBelow, "Layer", "Select Layer Below", sc(n, Key::ArrowDown));
    add(Cmd::NewFrame, "Frame", "New Frame (copy of current)", sc(a, Key::N));
    add(Cmd::NewEmptyFrame, "Frame", "New Empty Frame", sc(a, Key::B));
    add(Cmd::DelFrame, "Frame", "Delete Frame", sc(a, Key::C));
    add(Cmd::FrameLeft, "Frame", "Move Frame Earlier", sc(c, Key::ArrowLeft));
    add(Cmd::FrameRight, "Frame", "Move Frame Later", sc(c, Key::ArrowRight));
    add(Cmd::FrameProps, "Frame", "Frame Duration…", sc(a, Key::P));
    add(Cmd::NewTag, "Frame", "New Tag…", sc(a, Key::T));
    add(Cmd::PlayToggle, "Frame", "Play / Pause Animation", sc(n, Key::Enter));
    add(Cmd::NextFrame, "Frame", "Next Frame", sc(n, Key::ArrowRight));
    add(Cmd::PrevFrame, "Frame", "Previous Frame", sc(n, Key::ArrowLeft));
    add(Cmd::FirstFrame, "Frame", "First Frame", sc(n, Key::Home));
    add(Cmd::LastFrame, "Frame", "Last Frame", sc(n, Key::End));
    add(Cmd::ZoomIn, "View", "Zoom In", sc(c, Key::Equals));
    add(Cmd::ZoomOut, "View", "Zoom Out", sc(c, Key::Minus));
    add(Cmd::ZoomFit, "View", "Fit on Screen", sc(c, Key::Num0));
    add(Cmd::Zoom100, "View", "Zoom 100%", sc(c, Key::Num1));
    add(Cmd::ToggleGrid, "View", "Show Grid", sc(c, Key::Quote));
    add(Cmd::TogglePixelGrid, "View", "Show Pixel Grid", sc(cs, Key::Quote));
    add(Cmd::ToggleOnion, "View", "Onion Skin", sc(n, Key::F3));
    add(Cmd::SymH, "View", "Mirror Drawing Left ↔ Right", None);
    add(Cmd::SymV, "View", "Mirror Drawing Top ↔ Bottom", None);
    add(Cmd::ToggleTheme, "View", "Light Theme", None);
    add(Cmd::SwapColors, "Color", "Swap Foreground / Background", sc(n, Key::X));
    add(
        Cmd::DefaultColors,
        "Color",
        "Reset Colors to Black & White",
        sc(n, Key::D),
    );
    add(Cmd::BrushInc, "Brush", "Increase Brush Size", sc(n, Key::CloseBracket));
    add(Cmd::BrushDec, "Brush", "Decrease Brush Size", sc(n, Key::OpenBracket));
    add(Cmd::ImportPalette, "Palette", "Load Palette from File…", None);
    add(Cmd::ExportPalette, "Palette", "Save Palette to File…", None);
    add(
        Cmd::PaletteFromSprite,
        "Palette",
        "Create Palette from Sprite Colors",
        None,
    );
    add(Cmd::CommandPalette, "Help", "Command Palette…", sc(c, Key::K));
    add(Cmd::Shortcuts, "Help", "Keyboard Shortcuts & Tips", sc(n, Key::F1));
    add(Cmd::About, "Help", "About asetea", None);
    add(Cmd::Cancel, "Select", "Apply Move / Deselect", sc(n, Key::Escape));
    for t in Tool::ALL {
        let (m, k, _) = t.key();
        add(Cmd::Tool(t), "Tool", t.name(), sc(m, k));
    }
    let mut binds: Vec<(KeyboardShortcut, Cmd)> = v.iter().filter_map(|i| i.shortcut.map(|s| (s, i.cmd))).collect();
    let extra = [
        (c, Key::Y, Cmd::Redo),
        (n, Key::Backspace, Cmd::Clear),
        (cs, Key::P, Cmd::CommandPalette),
        (c, Key::Plus, Cmd::ZoomIn),
        (n, Key::Period, Cmd::NextFrame),
        (n, Key::Comma, Cmd::PrevFrame),
    ];
    for (m, k, cmd) in extra {
        binds.push((KeyboardShortcut::new(m, k), cmd));
    }
    let weight = |m: Modifiers| m.alt as u8 + m.shift as u8 + (m.command || m.ctrl || m.mac_cmd) as u8;
    binds.sort_by_key(|(s, _)| std::cmp::Reverse(weight(s.modifiers)));
    (v, binds)
}

// ---------------------------------------------------------------- Document

#[derive(Default)]
pub struct TexCache {
    tex: Option<egui::TextureHandle>,
    key: (u64, u64),
}

impl TexCache {
    pub fn get(
        &mut self,
        ctx: &egui::Context,
        name: &str,
        key: (u64, u64),
        make: impl FnOnce() -> Image,
    ) -> egui::TextureId {
        if self.tex.is_none() || self.key != key {
            let img = make();
            let ci = egui::ColorImage::from_rgba_unmultiplied(
                [img.w.max(1) as usize, img.h.max(1) as usize],
                &if img.data.is_empty() { vec![0; 4] } else { img.data },
            );
            let opt = egui::TextureOptions::NEAREST;
            match &mut self.tex {
                Some(t) => t.set(ci, opt),
                None => self.tex = Some(ctx.load_texture(name, ci, opt)),
            }
            self.key = key;
        }
        self.tex.as_ref().unwrap().id()
    }
}

pub struct Floating {
    pub img: Image,
    pub x: i32,
    pub y: i32,
    /// True when the pixels were cut out of the cel (an undo step already exists).
    pub lifted: bool,
    /// Which pixels of `img` belong to the selection shape.
    pub shape: Vec<bool>,
    pub id: u64,
}

struct Snap {
    sprite: Sprite,
    layer: usize,
    frame: usize,
    id: u64,
}

pub struct Doc {
    pub sprite: Sprite,
    pub path: Option<PathBuf>,
    pub title: String,
    undo: Vec<Snap>,
    redo: Vec<Snap>,
    state_id: u64,
    saved_id: u64,
    /// Bumped on every visible change; texture caches key on it.
    pub rev: u64,
    pub layer: usize,
    pub frame: usize,
    pub sel: Option<Mask>,
    pub sel_gen: u64,
    pub sel_edges: (u64, Vec<[i32; 4]>),
    pub floating: Option<Floating>,
    pub zoom: f32,
    pub pan: egui::Vec2,
    pub fit_pending: bool,
    pub playing: bool,
    pub play_next: f64,
    pub play_range: (usize, usize, u8),
    pub play_sign: i32,
    pub preview_frame: usize,
    pub preview_next: f64,
    pub preview_sign: i32,
    pub last_point: Option<(i32, i32)>,
    pub tex: TexCache,
    pub tex_checker: TexCache,
    pub tex_onion_prev: TexCache,
    pub tex_onion_next: TexCache,
    pub tex_float: TexCache,
    pub tex_preview: TexCache,
    pub scroll_to_frame: bool,
}

impl Doc {
    pub fn new(sprite: Sprite, path: Option<PathBuf>, title: String) -> Self {
        let id = next_id();
        let layer = sprite.layers.iter().rposition(|l| !l.group).unwrap_or(0);
        Self {
            sprite,
            path,
            title,
            undo: vec![],
            redo: vec![],
            state_id: id,
            saved_id: id,
            rev: next_id(),
            layer,
            frame: 0,
            sel: None,
            sel_gen: 0,
            sel_edges: (u64::MAX, vec![]),
            floating: None,
            zoom: 8.0,
            pan: egui::Vec2::ZERO,
            fit_pending: true,
            playing: false,
            play_next: 0.0,
            play_range: (0, 0, 0),
            play_sign: 1,
            preview_frame: 0,
            preview_next: 0.0,
            preview_sign: 1,
            last_point: None,
            tex: TexCache::default(),
            tex_checker: TexCache::default(),
            tex_onion_prev: TexCache::default(),
            tex_onion_next: TexCache::default(),
            tex_float: TexCache::default(),
            tex_preview: TexCache::default(),
            scroll_to_frame: true,
        }
    }

    pub fn modified(&self) -> bool {
        self.state_id != self.saved_id
    }

    pub fn mark_saved(&mut self) {
        self.saved_id = self.state_id;
    }

    pub fn touch(&mut self) {
        self.rev = next_id();
    }

    fn snap(&self) -> Snap {
        Snap {
            sprite: self.sprite.clone(),
            layer: self.layer,
            frame: self.frame,
            id: self.state_id,
        }
    }

    /// Record an undo step. Call before changing the sprite.
    pub fn begin_edit(&mut self) {
        let s = self.snap();
        self.undo.push(s);
        if self.undo.len() > 300 {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.state_id = next_id();
        self.touch();
    }

    fn restore(&mut self, s: Snap) {
        self.sprite = s.sprite;
        self.layer = s.layer;
        self.frame = s.frame;
        self.state_id = s.id;
        self.clamp();
        self.touch();
    }

    pub fn clamp(&mut self) {
        self.layer = self.layer.min(self.sprite.layers.len().saturating_sub(1));
        self.frame = self.frame.min(self.sprite.nframes().saturating_sub(1));
        if self
            .sel
            .as_ref()
            .is_some_and(|m| m.w != self.sprite.w || m.h != self.sprite.h)
        {
            self.set_sel(None);
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty() || self.floating.is_some()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo(&mut self) {
        if let Some(fl) = self.floating.take() {
            self.touch();
            if !fl.lifted {
                self.set_sel(None);
                return;
            }
        }
        if let Some(s) = self.undo.pop() {
            let cur = self.snap();
            self.redo.push(cur);
            self.restore(s);
        }
    }

    pub fn redo(&mut self) {
        self.commit_floating();
        if let Some(s) = self.redo.pop() {
            let cur = self.snap();
            self.undo.push(cur);
            self.restore(s);
        }
    }

    pub fn set_sel(&mut self, m: Option<Mask>) {
        self.sel = m.filter(|m| !m.is_empty());
        self.sel_gen = next_id();
    }

    pub fn set_cursor(&mut self, layer: usize, frame: usize) {
        if layer != self.layer || frame != self.frame {
            self.commit_floating();
            if frame != self.frame {
                self.scroll_to_frame = true;
            }
            self.layer = layer;
            self.frame = frame;
            self.clamp();
            self.touch();
        }
    }

    /// Stamp floating pixels (a paste or a moved selection) into the active cel.
    pub fn commit_floating(&mut self) {
        if let Some(fl) = self.floating.take() {
            if !fl.lifted {
                self.begin_edit();
            }
            let (l, f) = (self.layer, self.frame);
            if !self.sprite.layers[l].group {
                let cel = self.sprite.edit_cel(l, f);
                let (cx, cy) = (cel.x, cel.y);
                Arc::make_mut(&mut cel.image).blend_from(&fl.img, fl.x - cx, fl.y - cy, 255, 0);
            }
            self.touch();
        }
    }

    /// Cut the selected pixels out of the active cel into a floating image.
    pub fn lift(&mut self) -> bool {
        let Some(sel) = self.sel.clone() else { return false };
        let Some((bx, by, bw, bh)) = sel.bounds() else {
            return false;
        };
        self.begin_edit();
        let (l, f) = (self.layer, self.frame);
        let cel = self.sprite.edit_cel(l, f);
        let (cx, cy) = (cel.x, cel.y);
        let img = Arc::make_mut(&mut cel.image);
        let mut out = Image::new(bw, bh);
        let mut shape = vec![false; (bw * bh) as usize];
        for y in by..by + bh {
            for x in bx..bx + bw {
                if sel.get(x, y) {
                    shape[((y - by) * bw + (x - bx)) as usize] = true;
                    out.set(x - bx, y - by, img.get(x - cx, y - cy));
                    img.set(x - cx, y - cy, [0; 4]);
                }
            }
        }
        self.floating = Some(Floating {
            img: out,
            x: bx,
            y: by,
            lifted: true,
            shape,
            id: next_id(),
        });
        true
    }

    /// The tag whose range contains `frame` (the tightest one if several do).
    pub fn tag_at(&self, frame: usize) -> Option<&Tag> {
        self.sprite
            .tags
            .iter()
            .filter(|t| t.from <= frame && frame <= t.to)
            .min_by_key(|t| t.to - t.from)
    }

    pub fn range_at(&self, frame: usize) -> (usize, usize, u8) {
        match self.tag_at(frame) {
            Some(t) => (t.from, t.to, t.dir),
            None => (0, self.sprite.nframes() - 1, 0),
        }
    }
}

/// Next frame when playing `range` in the given direction mode.
pub fn advance(frame: usize, range: (usize, usize, u8), sign: &mut i32) -> usize {
    let (from, to, dir) = range;
    if from >= to {
        return from;
    }
    if frame < from || frame > to {
        return if dir == 1 || dir == 3 { to } else { from };
    }
    match dir {
        1 => {
            if frame == from {
                to
            } else {
                frame - 1
            }
        }
        2 | 3 => {
            if *sign == 0 {
                *sign = if dir == 3 { -1 } else { 1 };
            }
            if frame == to {
                *sign = -1;
            } else if frame == from {
                *sign = 1;
            }
            (frame as i32 + *sign) as usize
        }
        _ => {
            if frame == to {
                from
            } else {
                frame + 1
            }
        }
    }
}

// ---------------------------------------------------------------- App

pub enum Dialog {
    New {
        w: i32,
        h: i32,
        bg: u8,
    },
    CanvasSize {
        w: i32,
        h: i32,
        ax: u8,
        ay: u8,
    },
    SpriteSize {
        w: i32,
        h: i32,
        lock: bool,
    },
    Export {
        kind: u8,
        scale: u32,
        tag_only: bool,
    },
    Sheet {
        cols: u32,
        scale: u32,
        pad: u32,
        json: bool,
    },
    LayerProps {
        idx: usize,
        name: String,
        opacity: u8,
        blend: u16,
    },
    Tag {
        idx: Option<usize>,
        name: String,
        from: usize,
        to: usize,
        dir: u8,
        color: [u8; 3],
    },
    FrameDur {
        ms: u16,
        all: bool,
    },
    Grid {
        w: u16,
        h: u16,
    },
    ReplaceColor {
        from: Rgba,
        to: Rgba,
        tol: u8,
        all: bool,
    },
    Shortcuts,
    About,
    Message {
        title: String,
        text: String,
    },
    ConfirmClose {
        idx: usize,
        then_quit: bool,
    },
}

pub enum Drag {
    None,
    Paint {
        base: Arc<Image>,
        last: (i32, i32),
        color: Rgba,
        erase: bool,
        trail: Vec<(i32, i32)>,
    },
    Shape {
        base: Arc<Image>,
        start: (i32, i32),
        color: Rgba,
        other: Rgba,
    },
    Marquee {
        start: (i32, i32),
        prev: Option<Mask>,
        mode: u8,
    },
    Lasso {
        pts: Vec<(i32, i32)>,
        prev: Option<Mask>,
        mode: u8,
    },
    MoveSel {
        last: (i32, i32),
    },
    MoveCel {
        last: (i32, i32),
    },
    Pick {
        secondary: bool,
    },
    Pan,
}

pub struct App {
    pub docs: Vec<Doc>,
    pub active: usize,
    pub tool: Tool,
    pub fg: Rgba,
    pub bg: Rgba,
    pub picker: egui::ecolor::Hsva,
    pub picker_for: Rgba,
    pub hex_text: String,
    pub brush_size: i32,
    pub brush_square: bool,
    pub pixel_perfect: bool,
    pub filled: bool,
    pub tolerance: u8,
    pub contiguous: bool,
    pub alpha_lock: bool,
    pub dither: bool,
    pub sym_h: bool,
    pub sym_v: bool,
    pub show_grid: bool,
    pub show_pixel_grid: bool,
    pub onion: bool,
    pub light_theme: bool,
    pub clipboard: Option<(Image, i32, i32)>,
    pub dialog: Option<Dialog>,
    pub cmd_palette: Option<(String, usize)>,
    pub toast: Option<(String, f64)>,
    pub recent: Vec<PathBuf>,
    pub cmds: Vec<CmdInfo>,
    binds: Vec<(KeyboardShortcut, Cmd)>,
    pub pending: Vec<Cmd>,
    pub drag: Drag,
    pub hover_px: Option<(i32, i32)>,
    pub drag_size: Option<(i32, i32)>,
    pub slider_edit: Option<egui::Id>,
    last_title: String,
    pub wheel_zoom_until: f64,
    pub time: f64,
    untitled: u32,
    allow_close: bool,
    frame_no: u64,
    shot: Option<PathBuf>,
    script: Vec<Cmd>,
    mouse: Vec<egui::Pos2>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, files: Vec<PathBuf>) -> Self {
        let mut app = Self::base();
        let get = |k: &str| cc.storage.and_then(|s| s.get_string(k));
        app.recent = get("recent")
            .map(|s| s.lines().filter(|l| !l.is_empty()).map(PathBuf::from).collect())
            .unwrap_or_default();
        app.light_theme = get("theme").is_some_and(|t| t == "light");
        cc.egui_ctx.options_mut(|o| o.zoom_with_keyboard = false);
        cc.egui_ctx.set_theme(if app.light_theme {
            egui::Theme::Light
        } else {
            egui::Theme::Dark
        });
        cc.egui_ctx.style_mut(|s| {
            s.interaction.tooltip_delay = 0.4;
            s.spacing.item_spacing = egui::vec2(6.0, 5.0);
        });
        app.script = std::env::var("ASETEA_RUN")
            .map(|s| {
                s.split(',')
                    .filter_map(|n| {
                        app.cmds
                            .iter()
                            .find(|c| format!("{:?}", c.cmd) == n.trim())
                            .map(|c| c.cmd)
                    })
                    .collect()
            })
            .unwrap_or_default();
        app.shot = std::env::var_os("ASETEA_SHOT").map(PathBuf::from);
        app.mouse = std::env::var("ASETEA_MOUSE")
            .map(|s| {
                s.split(';')
                    .filter_map(|p| {
                        let (x, y) = p.split_once(',')?;
                        Some(egui::pos2(x.trim().parse().ok()?, y.trim().parse().ok()?))
                    })
                    .collect()
            })
            .unwrap_or_default();
        for f in files {
            app.open_path(&f);
        }
        app
    }

    /// App state with defaults and no window (also used by tests).
    pub fn base() -> Self {
        let (cmds, binds) = build_commands();
        Self {
            docs: vec![],
            active: 0,
            tool: Tool::Pencil,
            fg: [0, 0, 0, 255],
            bg: [255, 255, 255, 255],
            picker: egui::ecolor::Hsva::from_srgba_unmultiplied([0, 0, 0, 255]),
            picker_for: [0, 0, 0, 255],
            hex_text: "000000".into(),
            brush_size: 1,
            brush_square: false,
            pixel_perfect: false,
            filled: false,
            tolerance: 0,
            contiguous: true,
            alpha_lock: false,
            dither: true,
            sym_h: false,
            sym_v: false,
            show_grid: false,
            show_pixel_grid: true,
            onion: false,
            light_theme: false,
            clipboard: None,
            dialog: None,
            cmd_palette: None,
            toast: None,
            recent: vec![],
            cmds,
            binds,
            pending: vec![],
            drag: Drag::None,
            hover_px: None,
            drag_size: None,
            slider_edit: None,
            last_title: String::new(),
            wheel_zoom_until: 0.0,
            time: 0.0,
            untitled: 0,
            allow_close: false,
            frame_no: 0,
            shot: None,
            script: vec![],
            mouse: vec![],
        }
    }

    pub fn doc(&mut self) -> Option<&mut Doc> {
        self.docs.get_mut(self.active)
    }

    pub fn toast(&mut self, msg: impl Into<String>) {
        self.toast = Some((msg.into(), self.time));
    }

    pub fn error(&mut self, title: &str, text: impl Into<String>) {
        self.dialog = Some(Dialog::Message {
            title: title.into(),
            text: text.into(),
        });
    }

    pub fn info(&self, cmd: Cmd) -> &CmdInfo {
        self.cmds.iter().find(|c| c.cmd == cmd).expect("command registered")
    }

    // ---- files

    pub fn open_path(&mut self, path: &Path) {
        if let Some(i) = self.docs.iter().position(|d| d.path.as_deref() == Some(path)) {
            self.active = i;
            return;
        }
        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let title = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let result = if ext == "ase" || ext == "aseprite" {
            std::fs::read(path)
                .map_err(|e| e.to_string())
                .and_then(|d| ase::load(&d))
        } else {
            export::import_image(path).map(|s| (s, vec![]))
        };
        match result {
            Ok((sprite, warnings)) => {
                let native = ext == "ase" || ext == "aseprite";
                let mut doc = Doc::new(sprite, native.then(|| path.to_path_buf()), title);
                if !native {
                    doc.title = format!("{} (imported)", doc.title);
                }
                self.docs.push(doc);
                self.active = self.docs.len() - 1;
                if native {
                    self.add_recent(path);
                }
                if !warnings.is_empty() {
                    self.error("Opened with notes", warnings.join("\n\n"));
                }
            }
            Err(e) => self.error("Couldn't open file", format!("{}\n\n{e}", path.display())),
        }
    }

    fn add_recent(&mut self, path: &Path) {
        self.recent.retain(|p| p != path);
        self.recent.insert(0, path.to_path_buf());
        self.recent.truncate(10);
    }

    /// Save the document at `idx`. Returns false if the user cancelled or it failed.
    pub fn save_doc(&mut self, idx: usize, save_as: bool) -> bool {
        self.docs[idx].commit_floating();
        let mut path = self.docs[idx].path.clone();
        if save_as || path.is_none() {
            let mut dlg = rfd::FileDialog::new().add_filter("Aseprite", &["aseprite", "ase"]);
            let stem = Path::new(&self.docs[idx].title)
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "sprite".into());
            dlg = dlg.set_file_name(format!("{stem}.aseprite"));
            if let Some(dir) = path.as_ref().and_then(|p| p.parent()) {
                dlg = dlg.set_directory(dir);
            }
            path = dlg.save_file();
        }
        let Some(mut path) = path else { return false };
        if path.extension().is_none() {
            path.set_extension("aseprite");
        }
        let data = ase::save(&self.docs[idx].sprite);
        // Write to a temporary file first so a failed save never truncates the original.
        let tmp = path.with_extension("aseprite.tmp");
        let res = std::fs::write(&tmp, &data).and_then(|_| std::fs::rename(&tmp, &path));
        match res {
            Ok(()) => {
                let d = &mut self.docs[idx];
                d.title = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                d.path = Some(path.clone());
                d.mark_saved();
                self.add_recent(&path);
                self.toast(format!("Saved {}", path.display()));
                true
            }
            Err(e) => {
                let _ = std::fs::remove_file(&tmp);
                self.error("Couldn't save file", format!("{}\n\n{e}", path.display()));
                false
            }
        }
    }

    pub fn close_doc(&mut self, idx: usize) {
        if idx < self.docs.len() {
            self.docs.remove(idx);
            if self.active >= idx && self.active > 0 {
                self.active -= 1;
            }
            self.drag = Drag::None;
        }
    }

    pub fn request_close(&mut self, idx: usize) {
        if idx >= self.docs.len() {
            return;
        }
        if self.docs[idx].modified() {
            self.active = idx;
            self.dialog = Some(Dialog::ConfirmClose { idx, then_quit: false });
        } else {
            self.close_doc(idx);
        }
    }

    pub fn request_quit(&mut self, ctx: &egui::Context) {
        if let Some(idx) = self.docs.iter().position(|d| d.modified()) {
            self.active = idx;
            self.dialog = Some(Dialog::ConfirmClose { idx, then_quit: true });
        } else {
            self.allow_close = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    pub fn new_sprite(&mut self, w: i32, h: i32, bg: Option<Rgba>) {
        self.untitled += 1;
        let doc = Doc::new(Sprite::new(w, h, bg), None, format!("Untitled-{}", self.untitled));
        self.docs.push(doc);
        self.active = self.docs.len() - 1;
    }

    // ---- editing helpers

    /// Whether the active layer can receive pixels; explains why not otherwise.
    pub fn can_edit(&mut self) -> bool {
        let Some(d) = self.docs.get(self.active) else {
            return false;
        };
        let msg = if d.sprite.layers[d.layer].group {
            "A group can't hold pixels — select an image layer in the timeline."
        } else if !d.sprite.layer_editable(d.layer) {
            "This layer is locked. Click its lock icon in the timeline to unlock it."
        } else if !d.sprite.layer_visible(d.layer) {
            "This layer is hidden. Click its eye icon in the timeline to show it."
        } else {
            return true;
        };
        self.toast(msg);
        false
    }

    /// Apply `f` to every selected pixel of the active cel (or the whole cel).
    fn edit_pixels(&mut self, all_frames: bool, f: impl Fn(Rgba) -> Rgba) {
        if !self.can_edit() {
            return;
        }
        let d = &mut self.docs[self.active];
        d.commit_floating();
        d.begin_edit();
        let frames: Vec<usize> = if all_frames {
            (0..d.sprite.nframes()).collect()
        } else {
            vec![d.frame]
        };
        let (w, h) = (d.sprite.w, d.sprite.h);
        for fr in frames {
            if d.sprite.layers[d.layer].cels[fr].is_none() {
                continue;
            }
            let cel = d.sprite.edit_cel(d.layer, fr);
            let (cx, cy) = (cel.x, cel.y);
            let img = Arc::make_mut(&mut cel.image);
            for y in 0..h {
                for x in 0..w {
                    if d.sel.as_ref().is_none_or(|s| s.get(x, y)) {
                        let c = img.get(x - cx, y - cy);
                        img.set(x - cx, y - cy, f(c));
                    }
                }
            }
        }
    }

    fn copy(&mut self, ctx: &egui::Context) -> bool {
        let Some(d) = self.docs.get(self.active) else {
            return false;
        };
        let clip = if let Some(fl) = &d.floating {
            Some((fl.img.clone(), fl.x, fl.y))
        } else if let Some(cel) = &d.sprite.layers[d.layer].cels[d.frame] {
            let img = cel.image.crop(-cel.x, -cel.y, d.sprite.w, d.sprite.h);
            match &d.sel {
                Some(sel) => sel.bounds().map(|(bx, by, bw, bh)| {
                    let mut out = img.crop(bx, by, bw, bh);
                    for y in 0..bh {
                        for x in 0..bw {
                            if !sel.get(bx + x, by + y) {
                                out.set(x, y, [0; 4]);
                            }
                        }
                    }
                    (out, bx, by)
                }),
                None => img.bounds().map(|(bx, by, bw, bh)| (img.crop(bx, by, bw, bh), bx, by)),
            }
        } else {
            None
        };
        match clip {
            Some(c) => {
                self.clipboard = Some(c);
                // Keeps the OS paste shortcut alive: the toolkit only reports paste when text exists.
                ctx.copy_text("asetea pixels".into());
                true
            }
            None => {
                self.toast("Nothing to copy on this layer.");
                false
            }
        }
    }

    fn paste(&mut self) {
        let Some((img, x, y)) = self.clipboard.clone() else {
            self.toast("The clipboard is empty — copy some pixels first.");
            return;
        };
        if !self.can_edit() {
            return;
        }
        let d = &mut self.docs[self.active];
        d.commit_floating();
        let (w, h) = (d.sprite.w, d.sprite.h);
        let (x, y) = if x >= 0 && y >= 0 && x + img.w <= w && y + img.h <= h {
            (x, y)
        } else {
            ((w - img.w) / 2, (h - img.h) / 2)
        };
        d.set_sel(Some(Mask::rect(w, h, x, y, img.w, img.h)));
        let shape = vec![true; (img.w * img.h) as usize];
        d.floating = Some(Floating {
            img,
            x,
            y,
            lifted: false,
            shape,
            id: next_id(),
        });
        d.touch();
        self.tool = Tool::Move;
        self.toast("Pasted — drag to position, then press Enter (or Esc) to place it.");
    }

    fn clear(&mut self) {
        let Some(d) = self.docs.get_mut(self.active) else {
            return;
        };
        if d.floating.take().is_some() {
            d.touch();
            return;
        }
        if d.sel.is_some() {
            self.edit_pixels(false, |_| [0; 4]);
        } else if self.can_edit() {
            let d = &mut self.docs[self.active];
            d.begin_edit();
            d.sprite.layers[d.layer].cels[d.frame] = None;
        }
    }

    fn flip_selection(&mut self, horizontal: bool) {
        let d = &mut self.docs[self.active];
        d.commit_floating();
        match d.sel.as_ref().and_then(|s| s.bounds()) {
            None => {
                d.begin_edit();
                d.sprite.flip(horizontal);
            }
            Some((bx, by, bw, bh)) => {
                if !self.can_edit() {
                    return;
                }
                let d = &mut self.docs[self.active];
                d.begin_edit();
                let cel = d.sprite.edit_cel(d.layer, d.frame);
                let (cx, cy) = (cel.x, cel.y);
                let img = Arc::make_mut(&mut cel.image);
                let region = img.crop(bx - cx, by - cy, bw, bh);
                let flipped = if horizontal { region.flip_h() } else { region.flip_v() };
                for y in 0..bh {
                    for x in 0..bw {
                        img.set(bx - cx + x, by - cy + y, flipped.get(x, y));
                    }
                }
            }
        }
    }

    pub fn replace_color(&mut self, from: Rgba, to: Rgba, tol: u8, all_frames: bool) {
        self.edit_pixels(all_frames, |c| {
            if c[3] != 0 && draw::color_close(c, from, tol) {
                to
            } else {
                c
            }
        });
    }

    fn outline(&mut self) {
        if !self.can_edit() {
            return;
        }
        let fg = self.fg;
        let d = &mut self.docs[self.active];
        d.commit_floating();
        d.begin_edit();
        let (w, h) = (d.sprite.w, d.sprite.h);
        let cel = d.sprite.edit_cel(d.layer, d.frame);
        let (cx, cy) = (cel.x, cel.y);
        let src = cel.image.clone();
        let img = Arc::make_mut(&mut cel.image);
        for y in 0..h {
            for x in 0..w {
                let (ix, iy) = (x - cx, y - cy);
                if src.get(ix, iy)[3] == 0
                    && [(1, 0), (-1, 0), (0, 1), (0, -1)]
                        .iter()
                        .any(|(dx, dy)| src.get(ix + dx, iy + dy)[3] != 0)
                    && d.sel.as_ref().is_none_or(|s| s.get(x, y))
                {
                    img.set(ix, iy, fg);
                }
            }
        }
    }

    pub fn zoom_by(&mut self, up: bool) {
        if let Some(d) = self.doc() {
            let z = zoom_step(d.zoom, up);
            d.pan *= z / d.zoom;
            d.zoom = z;
        }
    }

    pub fn start_playback(&mut self) {
        let now = self.time;
        if let Some(d) = self.doc() {
            d.commit_floating();
            d.playing = !d.playing;
            if d.playing {
                d.play_range = d.range_at(d.frame);
                d.play_sign = 0;
                d.play_next = now + d.sprite.durations[d.frame] as f64 / 1000.0;
            }
        }
    }

    pub fn enabled(&self, cmd: Cmd) -> bool {
        let d = self.docs.get(self.active);
        match cmd {
            Cmd::New | Cmd::Open | Cmd::Quit | Cmd::CommandPalette | Cmd::Shortcuts | Cmd::About | Cmd::ToggleTheme => {
                true
            }
            Cmd::Undo => d.is_some_and(|d| d.can_undo()),
            Cmd::Redo => d.is_some_and(|d| d.can_redo()),
            Cmd::Paste => d.is_some() && self.clipboard.is_some(),
            Cmd::CropToSel | Cmd::InvertSel => d.is_some_and(|d| d.sel.is_some()),
            Cmd::Deselect => d.is_some_and(|d| d.sel.is_some() || d.floating.is_some()),
            Cmd::DelFrame | Cmd::FrameLeft | Cmd::FrameRight => d.is_some_and(|d| d.sprite.nframes() > 1),
            Cmd::MergeDown => d.is_some_and(|d| d.sprite.can_merge_down(d.layer)),
            Cmd::DelLayer => d.is_some_and(|d| d.sprite.block_end(d.layer) - d.layer < d.sprite.layers.len()),
            _ => d.is_some(),
        }
    }

    pub fn checked(&self, cmd: Cmd) -> Option<bool> {
        let d = self.docs.get(self.active);
        Some(match cmd {
            Cmd::ToggleGrid => self.show_grid,
            Cmd::TogglePixelGrid => self.show_pixel_grid,
            Cmd::ToggleOnion => self.onion,
            Cmd::SymH => self.sym_h,
            Cmd::SymV => self.sym_v,
            Cmd::ToggleTheme => self.light_theme,
            Cmd::ModeRgb => d?.sprite.mode == ColorMode::Rgba,
            Cmd::ModeIndexed => d?.sprite.mode == ColorMode::Indexed,
            Cmd::ModeGray => d?.sprite.mode == ColorMode::Gray,
            _ => return None,
        })
    }

    // ---- command dispatch

    pub fn run(&mut self, cmd: Cmd, ctx: &egui::Context) {
        if !self.enabled(cmd) {
            return;
        }
        // Commands that work without a document.
        match cmd {
            Cmd::New => {
                self.dialog = Some(Dialog::New { w: 32, h: 32, bg: 0 });
                return;
            }
            Cmd::Open => {
                let files = rfd::FileDialog::new()
                    .add_filter(
                        "Sprites and images",
                        &["aseprite", "ase", "png", "gif", "jpg", "jpeg", "bmp"],
                    )
                    .add_filter("Aseprite", &["aseprite", "ase"])
                    .pick_files();
                for f in files.unwrap_or_default() {
                    self.open_path(&f);
                }
                return;
            }
            Cmd::Quit => {
                self.request_quit(ctx);
                return;
            }
            Cmd::CommandPalette => {
                self.cmd_palette = Some((String::new(), 0));
                return;
            }
            Cmd::Shortcuts => {
                self.dialog = Some(Dialog::Shortcuts);
                return;
            }
            Cmd::About => {
                self.dialog = Some(Dialog::About);
                return;
            }
            Cmd::ToggleTheme => {
                self.light_theme = !self.light_theme;
                ctx.set_theme(if self.light_theme {
                    egui::Theme::Light
                } else {
                    egui::Theme::Dark
                });
                return;
            }
            _ => {}
        }
        if self.docs.is_empty() {
            return;
        }
        let a = self.active;
        match cmd {
            Cmd::Save => {
                self.save_doc(a, false);
            }
            Cmd::SaveAs => {
                self.save_doc(a, true);
            }
            Cmd::ExportImage => {
                let anim = self.docs[a].sprite.nframes() > 1;
                self.dialog = Some(Dialog::Export {
                    kind: if anim { 2 } else { 0 },
                    scale: 1,
                    tag_only: false,
                });
            }
            Cmd::ExportSheet => {
                let n = self.docs[a].sprite.nframes() as f32;
                self.dialog = Some(Dialog::Sheet {
                    cols: n.sqrt().ceil() as u32,
                    scale: 1,
                    pad: 0,
                    json: true,
                });
            }
            Cmd::CloseDoc => self.request_close(a),
            Cmd::Undo => self.docs[a].undo(),
            Cmd::Redo => self.docs[a].redo(),
            Cmd::Copy => {
                if self.copy(ctx) {
                    self.toast("Copied.");
                }
            }
            Cmd::Cut => {
                if self.copy(ctx) {
                    self.clear();
                }
            }
            Cmd::Paste => self.paste(),
            Cmd::Clear => self.clear(),
            Cmd::FillSel => {
                let fg = self.fg;
                self.edit_pixels(false, |c| if fg[3] == 255 { fg } else { blend(c, fg, 255, 0) });
            }
            Cmd::SelectAll => {
                let d = &mut self.docs[a];
                d.commit_floating();
                d.set_sel(Some(Mask::full(d.sprite.w, d.sprite.h)));
            }
            Cmd::Deselect | Cmd::Cancel => {
                let d = &mut self.docs[a];
                if d.floating.is_some() {
                    d.commit_floating();
                    if cmd == Cmd::Cancel {
                        return;
                    }
                }
                d.set_sel(None);
            }
            Cmd::InvertSel => {
                let d = &mut self.docs[a];
                d.commit_floating();
                let inv = d.sel.as_ref().map(|m| m.inverted());
                d.set_sel(inv);
            }
            Cmd::FlipH => self.flip_selection(true),
            Cmd::FlipV => self.flip_selection(false),
            Cmd::Rot90Cw | Cmd::Rot90Ccw | Cmd::Rot180 => {
                let d = &mut self.docs[a];
                d.commit_floating();
                d.begin_edit();
                d.sprite.rotate(match cmd {
                    Cmd::Rot90Cw => 1,
                    Cmd::Rot180 => 2,
                    _ => 3,
                });
                d.set_sel(None);
                d.fit_pending = true;
            }
            Cmd::Invert => self.edit_pixels(false, |c| [255 - c[0], 255 - c[1], 255 - c[2], c[3]]),
            Cmd::Desaturate => self.edit_pixels(false, |c| {
                let v = luma(c);
                [v, v, v, c[3]]
            }),
            Cmd::ReplaceColor => {
                self.dialog = Some(Dialog::ReplaceColor {
                    from: self.fg,
                    to: self.bg,
                    tol: 0,
                    all: false,
                });
            }
            Cmd::Outline => self.outline(),
            Cmd::CanvasSize => {
                let s = &self.docs[a].sprite;
                self.dialog = Some(Dialog::CanvasSize {
                    w: s.w,
                    h: s.h,
                    ax: 1,
                    ay: 1,
                });
            }
            Cmd::SpriteSize => {
                let s = &self.docs[a].sprite;
                self.dialog = Some(Dialog::SpriteSize {
                    w: s.w,
                    h: s.h,
                    lock: true,
                });
            }
            Cmd::CropToSel | Cmd::Trim => {
                let d = &mut self.docs[a];
                d.commit_floating();
                let b = if cmd == Cmd::Trim {
                    d.sprite.content_bounds()
                } else {
                    d.sel.as_ref().and_then(|s| s.bounds())
                };
                match b {
                    Some((x, y, w, h)) => {
                        d.begin_edit();
                        d.sprite.resize_canvas(w, h, -x, -y);
                        d.set_sel(None);
                        d.fit_pending = true;
                    }
                    None => self.toast("Nothing to crop to."),
                }
            }
            Cmd::ModeRgb | Cmd::ModeIndexed | Cmd::ModeGray => {
                let d = &mut self.docs[a];
                d.commit_floating();
                d.begin_edit();
                match cmd {
                    Cmd::ModeRgb => d.sprite.mode = ColorMode::Rgba,
                    Cmd::ModeGray => {
                        d.sprite.mode = ColorMode::Gray;
                        d.sprite.make_grayscale();
                    }
                    _ => {
                        if d.sprite.palette.len() > 256 {
                            d.sprite.palette.truncate(256);
                        }
                        if d.sprite.palette.len() < 2 {
                            d.sprite.palette = palettes::PRESETS[0].colors();
                        }
                        if d.sprite.transparent_index as usize >= d.sprite.palette.len() {
                            d.sprite.transparent_index = 0;
                        }
                        d.sprite.mode = ColorMode::Indexed;
                        d.sprite.quantize_to_palette();
                        self.toast(
                            "Indexed mode: colors were snapped to the palette. Index 0 is saved as transparent.",
                        );
                    }
                }
            }
            Cmd::GridSettings => {
                let g = self.docs[a].sprite.grid;
                self.dialog = Some(Dialog::Grid { w: g.2, h: g.3 });
            }
            Cmd::NewLayer | Cmd::NewGroup => {
                let d = &mut self.docs[a];
                d.commit_floating();
                d.begin_edit();
                d.layer = d.sprite.add_layer(d.layer, cmd == Cmd::NewGroup);
            }
            Cmd::DupLayer => {
                let d = &mut self.docs[a];
                d.commit_floating();
                d.begin_edit();
                d.layer = d.sprite.duplicate_layer(d.layer);
            }
            Cmd::DelLayer => {
                let d = &mut self.docs[a];
                d.floating = None;
                d.begin_edit();
                d.sprite.remove_layer(d.layer);
                d.layer = d.layer.saturating_sub(1);
                d.clamp();
            }
            Cmd::MergeDown => {
                let d = &mut self.docs[a];
                d.commit_floating();
                d.begin_edit();
                d.sprite.merge_down(d.layer);
                d.layer -= 1;
            }
            Cmd::Flatten => {
                let d = &mut self.docs[a];
                d.commit_floating();
                d.begin_edit();
                d.sprite.flatten();
                d.layer = 0;
            }
            Cmd::LayerUp | Cmd::LayerDown => {
                let d = &mut self.docs[a];
                d.commit_floating();
                let before = d.snap();
                match d.sprite.move_layer(d.layer, cmd == Cmd::LayerUp) {
                    Some(i) => {
                        d.undo.push(before);
                        d.redo.clear();
                        d.state_id = next_id();
                        d.layer = i;
                        d.touch();
                    }
                    None => self.toast("The layer can't move further within its group."),
                }
            }
            Cmd::LayerProps => {
                let d = &self.docs[a];
                let l = &d.sprite.layers[d.layer];
                self.dialog = Some(Dialog::LayerProps {
                    idx: d.layer,
                    name: l.name.clone(),
                    opacity: l.opacity,
                    blend: l.blend,
                });
            }
            Cmd::SelLayerAbove | Cmd::SelLayerBelow => {
                let d = &mut self.docs[a];
                let n = d.sprite.layers.len();
                let l = if cmd == Cmd::SelLayerAbove {
                    (d.layer + 1).min(n - 1)
                } else {
                    d.layer.saturating_sub(1)
                };
                d.set_cursor(l, d.frame);
            }
            Cmd::NewFrame | Cmd::NewEmptyFrame => {
                let d = &mut self.docs[a];
                d.commit_floating();
                d.begin_edit();
                let f = d.frame;
                d.sprite.insert_frame(f + 1, (cmd == Cmd::NewFrame).then_some(f));
                if cmd == Cmd::NewEmptyFrame {
                    d.sprite.durations[f + 1] = d.sprite.durations[f];
                }
                d.frame = f + 1;
                d.scroll_to_frame = true;
            }
            Cmd::DelFrame => {
                let d = &mut self.docs[a];
                d.floating = None;
                d.begin_edit();
                d.sprite.remove_frame(d.frame);
                d.clamp();
            }
            Cmd::FrameLeft | Cmd::FrameRight => {
                let d = &mut self.docs[a];
                let f = d.frame;
                let g = if cmd == Cmd::FrameLeft {
                    f.checked_sub(1)
                } else {
                    (f + 1 < d.sprite.nframes()).then_some(f + 1)
                };
                if let Some(g) = g {
                    d.commit_floating();
                    d.begin_edit();
                    d.sprite.swap_frames(f, g);
                    d.frame = g;
                    d.scroll_to_frame = true;
                }
            }
            Cmd::NextFrame | Cmd::PrevFrame | Cmd::FirstFrame | Cmd::LastFrame => {
                let d = &mut self.docs[a];
                let n = d.sprite.nframes();
                let f = match cmd {
                    Cmd::NextFrame => (d.frame + 1) % n,
                    Cmd::PrevFrame => (d.frame + n - 1) % n,
                    Cmd::FirstFrame => 0,
                    _ => n - 1,
                };
                d.playing = false;
                d.set_cursor(d.layer, f);
            }
            Cmd::PlayToggle => {
                if self.docs[a].floating.is_some() {
                    self.docs[a].commit_floating();
                } else {
                    self.start_playback();
                }
            }
            Cmd::NewTag => {
                let d = &self.docs[a];
                let colors = [
                    [254, 91, 89],
                    [247, 165, 71],
                    [243, 206, 82],
                    [106, 205, 91],
                    [87, 185, 242],
                    [209, 134, 223],
                ];
                self.dialog = Some(Dialog::Tag {
                    idx: None,
                    name: format!("Tag {}", d.sprite.tags.len() + 1),
                    from: d.frame,
                    to: d.frame,
                    dir: 0,
                    color: colors[d.sprite.tags.len() % colors.len()],
                });
            }
            Cmd::FrameProps => {
                let d = &self.docs[a];
                self.dialog = Some(Dialog::FrameDur {
                    ms: d.sprite.durations[d.frame],
                    all: false,
                });
            }
            Cmd::ZoomIn => self.zoom_by(true),
            Cmd::ZoomOut => self.zoom_by(false),
            Cmd::ZoomFit => self.docs[a].fit_pending = true,
            Cmd::Zoom100 => {
                let d = &mut self.docs[a];
                d.zoom = 1.0;
                d.pan = egui::Vec2::ZERO;
            }
            Cmd::ToggleGrid => self.show_grid = !self.show_grid,
            Cmd::TogglePixelGrid => self.show_pixel_grid = !self.show_pixel_grid,
            Cmd::ToggleOnion => {
                self.onion = !self.onion;
                self.docs[a].touch();
            }
            Cmd::SymH => self.sym_h = !self.sym_h,
            Cmd::SymV => self.sym_v = !self.sym_v,
            Cmd::SwapColors => std::mem::swap(&mut self.fg, &mut self.bg),
            Cmd::DefaultColors => {
                self.fg = [0, 0, 0, 255];
                self.bg = [255, 255, 255, 255];
            }
            Cmd::BrushInc => self.brush_size = (self.brush_size + 1).min(64),
            Cmd::BrushDec => self.brush_size = (self.brush_size - 1).max(1),
            Cmd::ImportPalette => {
                let f = rfd::FileDialog::new()
                    .add_filter("Palettes and sprites", &["gpl", "hex", "txt", "aseprite", "ase", "png"])
                    .pick_file();
                if let Some(f) = f {
                    let ext = f
                        .extension()
                        .map(|e| e.to_string_lossy().to_lowercase())
                        .unwrap_or_default();
                    let pal = match ext.as_str() {
                        "aseprite" | "ase" => std::fs::read(&f)
                            .ok()
                            .and_then(|d| ase::load(&d).ok())
                            .map(|(s, _)| s.palette),
                        "png" => export::import_image(&f)
                            .ok()
                            .map(|s| export::palette_from_sprite(&s, 256)),
                        _ => std::fs::read_to_string(&f)
                            .ok()
                            .map(|t| palettes::parse_palette_file(&t)),
                    };
                    match pal {
                        Some(p) if !p.is_empty() => {
                            let d = &mut self.docs[a];
                            d.begin_edit();
                            d.sprite.palette = p;
                        }
                        _ => self.error("Couldn't load palette", "No colors were found in that file."),
                    }
                }
            }
            Cmd::ExportPalette => {
                let f = rfd::FileDialog::new()
                    .add_filter("GIMP palette", &["gpl"])
                    .add_filter("Hex colors", &["hex"])
                    .set_file_name("palette.gpl")
                    .save_file();
                if let Some(f) = f {
                    let pal = &self.docs[a].sprite.palette;
                    let text = if f.extension().is_some_and(|e| e == "hex") {
                        palettes::to_hex_file(pal)
                    } else {
                        palettes::to_gpl(&self.docs[a].title, pal)
                    };
                    if let Err(e) = std::fs::write(&f, text) {
                        self.error("Couldn't save palette", e.to_string());
                    }
                }
            }
            Cmd::PaletteFromSprite => {
                let d = &mut self.docs[a];
                let pal = export::palette_from_sprite(&d.sprite, 256);
                if pal.is_empty() {
                    self.toast("The sprite has no pixels yet.");
                } else {
                    d.begin_edit();
                    d.sprite.palette = pal;
                }
            }
            Cmd::Tool(t) => {
                self.tool = t;
                self.drag = Drag::None;
            }
            _ => {}
        }
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        if self.dialog.is_some() || self.cmd_palette.is_some() || ctx.wants_keyboard_input() {
            return;
        }
        let mut fired = Vec::new();
        ctx.input_mut(|i| {
            for (sc, cmd) in &self.binds {
                if i.consume_shortcut(sc) {
                    fired.push(*cmd);
                    break;
                }
            }
            for e in &i.events {
                match e {
                    egui::Event::Copy => fired.push(Cmd::Copy),
                    egui::Event::Cut => fired.push(Cmd::Cut),
                    egui::Event::Paste(_) => fired.push(Cmd::Paste),
                    _ => {}
                }
            }
        });
        self.pending.extend(fired);
    }

    fn tick_playback(&mut self, ctx: &egui::Context) {
        let now = self.time;
        let Some(d) = self.docs.get_mut(self.active) else {
            return;
        };
        if d.playing {
            if d.sprite.nframes() < 2 {
                d.playing = false;
                return;
            }
            let mut guard = 0;
            while now >= d.play_next && guard < 8 {
                d.frame = advance(d.frame, d.play_range, &mut d.play_sign).min(d.sprite.nframes() - 1);
                d.play_next += d.sprite.durations[d.frame].max(10) as f64 / 1000.0;
                d.scroll_to_frame = true;
                guard += 1;
            }
            if now >= d.play_next {
                d.play_next = now + 0.01;
            }
            d.touch();
            ctx.request_repaint_after(std::time::Duration::from_secs_f64((d.play_next - now).max(0.0)));
        }
    }
}

impl eframe::App for App {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        let recent: Vec<String> = self.recent.iter().map(|p| p.to_string_lossy().into_owned()).collect();
        storage.set_string("recent", recent.join("\n"));
        storage.set_string("theme", if self.light_theme { "light" } else { "dark" }.into());
    }

    /// Test hook: replay a scripted left-button drag (ASETEA_MOUSE="x,y;x,y;…").
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw: &mut egui::RawInput) {
        if self.mouse.is_empty() || self.frame_no < 12 {
            return;
        }
        let i = (self.frame_no - 12) as usize;
        let n = self.mouse.len();
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        if i < n {
            raw.events.push(egui::Event::PointerMoved(self.mouse[i]));
            if i == 0 {
                raw.events.push(button(self.mouse[0], true));
            }
        } else if i == n {
            raw.events.push(button(self.mouse[n - 1], false));
        }
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.frame_no += 1;
        self.time = ctx.input(|i| i.time);

        // Test hooks: run a list of commands, then save a screenshot and exit.
        if self.frame_no == 5 && !self.script.is_empty() {
            let script = std::mem::take(&mut self.script);
            self.pending.extend(script);
        }
        if let Some(path) = self.shot.clone() {
            if self.frame_no == 40 {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            }
            let img = ctx.input(|i| {
                i.events.iter().find_map(|e| match e {
                    egui::Event::Screenshot { image, .. } => Some(image.clone()),
                    _ => None,
                })
            });
            if let Some(img) = img {
                let data: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
                let im = Image {
                    w: img.size[0] as i32,
                    h: img.size[1] as i32,
                    data,
                };
                let _ = export::save_png(&path, &im, 1);
                std::process::exit(0);
            }
            ctx.request_repaint();
        }

        let dropped: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect());
        for f in dropped {
            self.open_path(&f);
        }
        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close && self.docs.iter().any(|d| d.modified())
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.request_quit(ctx);
        }

        self.handle_shortcuts(ctx);
        self.tick_playback(ctx);

        self.menu_bar(ctx);
        self.status_bar(ctx);
        if self.docs.is_empty() {
            self.welcome(ctx);
        } else {
            self.tabs_and_options(ctx);
            self.timeline_panel(ctx);
            self.toolbar(ctx);
            self.side_panel(ctx);
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ctx, |ui| self.canvas(ui));
        }
        self.show_dialog(ctx);
        self.show_cmd_palette(ctx);
        self.show_toast(ctx);

        let pending = std::mem::take(&mut self.pending);
        for cmd in pending {
            self.run(cmd, ctx);
            ctx.request_repaint();
        }
        if self.active >= self.docs.len() {
            self.active = self.docs.len().saturating_sub(1);
        }
        let title = match self.docs.get(self.active) {
            Some(d) => format!("{}{} — asetea", d.title, if d.modified() { " •" } else { "" }),
            None => "asetea".into(),
        };
        if title != self.last_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.last_title = title;
        }
    }
}
