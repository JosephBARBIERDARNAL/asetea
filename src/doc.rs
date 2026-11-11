//! In-memory sprite model: images, layers, cels, frames, tags, compositing.
//! Everything is stored as straight-alpha RGBA; the color mode only matters on save.

use std::collections::HashMap;
use std::sync::Arc;

pub type Rgba = [u8; 4];

pub const BLEND_NAMES: [&str; 19] = [
    "Normal",
    "Multiply",
    "Screen",
    "Overlay",
    "Darken",
    "Lighten",
    "Color Dodge",
    "Color Burn",
    "Hard Light",
    "Soft Light",
    "Difference",
    "Exclusion",
    "Hue",
    "Saturation",
    "Color",
    "Luminosity",
    "Addition",
    "Subtract",
    "Divide",
];

// ---------------------------------------------------------------- Image

#[derive(Clone, PartialEq)]
pub struct Image {
    pub w: i32,
    pub h: i32,
    pub data: Vec<u8>,
}

impl Image {
    pub fn new(w: i32, h: i32) -> Self {
        let (w, h) = (w.max(0), h.max(0));
        Self {
            w,
            h,
            data: vec![0; (w as usize) * (h as usize) * 4],
        }
    }

    pub fn filled(w: i32, h: i32, c: Rgba) -> Self {
        let mut img = Self::new(w, h);
        for px in img.data.chunks_exact_mut(4) {
            px.copy_from_slice(&c);
        }
        img
    }

    #[inline]
    pub fn get(&self, x: i32, y: i32) -> Rgba {
        if x < 0 || y < 0 || x >= self.w || y >= self.h {
            return [0; 4];
        }
        let i = ((y * self.w + x) * 4) as usize;
        [self.data[i], self.data[i + 1], self.data[i + 2], self.data[i + 3]]
    }

    #[inline]
    pub fn set(&mut self, x: i32, y: i32, c: Rgba) {
        if x < 0 || y < 0 || x >= self.w || y >= self.h {
            return;
        }
        let i = ((y * self.w + x) * 4) as usize;
        self.data[i..i + 4].copy_from_slice(&c);
    }

    /// Bounding box (x, y, w, h) of pixels with non-zero alpha.
    pub fn bounds(&self) -> Option<(i32, i32, i32, i32)> {
        let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, -1, -1);
        for y in 0..self.h {
            let row = &self.data[(y * self.w * 4) as usize..((y + 1) * self.w * 4) as usize];
            for x in 0..self.w {
                if row[(x * 4 + 3) as usize] != 0 {
                    x0 = x0.min(x);
                    x1 = x1.max(x);
                    y0 = y0.min(y);
                    y1 = y1.max(y);
                }
            }
        }
        if x1 < 0 {
            None
        } else {
            Some((x0, y0, x1 - x0 + 1, y1 - y0 + 1))
        }
    }

    /// Copy of the given rectangle; areas outside the image are transparent.
    pub fn crop(&self, x: i32, y: i32, w: i32, h: i32) -> Image {
        let mut out = Image::new(w, h);
        let sx0 = x.max(0);
        let sx1 = (x + w).min(self.w);
        if sx1 <= sx0 {
            return out;
        }
        let n = ((sx1 - sx0) * 4) as usize;
        for sy in y.max(0)..(y + h).min(self.h) {
            let s = ((sy * self.w + sx0) * 4) as usize;
            let d = (((sy - y) * out.w + (sx0 - x)) * 4) as usize;
            out.data[d..d + n].copy_from_slice(&self.data[s..s + n]);
        }
        out
    }

    /// Composite `src` onto self at (dx, dy).
    pub fn blend_from(&mut self, src: &Image, dx: i32, dy: i32, opacity: u8, mode: u16) {
        if opacity == 0 {
            return;
        }
        let x0 = dx.max(0);
        let y0 = dy.max(0);
        let x1 = (dx + src.w).min(self.w);
        let y1 = (dy + src.h).min(self.h);
        for y in y0..y1 {
            let mut si = (((y - dy) * src.w + (x0 - dx)) * 4) as usize;
            let mut di = ((y * self.w + x0) * 4) as usize;
            for _ in x0..x1 {
                let sa = src.data[si + 3];
                if sa != 0 {
                    let s = [src.data[si], src.data[si + 1], src.data[si + 2], sa];
                    if sa == 255 && opacity == 255 && mode == 0 {
                        self.data[di..di + 4].copy_from_slice(&s);
                    } else {
                        let d = [self.data[di], self.data[di + 1], self.data[di + 2], self.data[di + 3]];
                        let r = blend(d, s, opacity, mode);
                        self.data[di..di + 4].copy_from_slice(&r);
                    }
                }
                si += 4;
                di += 4;
            }
        }
    }

    pub fn flip_h(&self) -> Image {
        let mut out = Image::new(self.w, self.h);
        for y in 0..self.h {
            for x in 0..self.w {
                out.set(self.w - 1 - x, y, self.get(x, y));
            }
        }
        out
    }

    pub fn flip_v(&self) -> Image {
        let mut out = Image::new(self.w, self.h);
        for y in 0..self.h {
            for x in 0..self.w {
                out.set(x, self.h - 1 - y, self.get(x, y));
            }
        }
        out
    }

    pub fn rot90cw(&self) -> Image {
        let mut out = Image::new(self.h, self.w);
        for y in 0..self.h {
            for x in 0..self.w {
                out.set(self.h - 1 - y, x, self.get(x, y));
            }
        }
        out
    }

    pub fn scale_nearest(&self, nw: i32, nh: i32) -> Image {
        let mut out = Image::new(nw, nh);
        if self.w == 0 || self.h == 0 {
            return out;
        }
        for y in 0..nh {
            let sy = (y as i64 * self.h as i64 / nh as i64) as i32;
            for x in 0..nw {
                let sx = (x as i64 * self.w as i64 / nw as i64) as i32;
                out.set(x, y, self.get(sx, sy));
            }
        }
        out
    }
}

// ---------------------------------------------------------------- Blending

fn lum(c: [f32; 3]) -> f32 {
    0.3 * c[0] + 0.59 * c[1] + 0.11 * c[2]
}

fn clip_color(mut c: [f32; 3]) -> [f32; 3] {
    let l = lum(c);
    let n = c[0].min(c[1]).min(c[2]);
    let x = c[0].max(c[1]).max(c[2]);
    if n < 0.0 {
        for v in &mut c {
            *v = l + (*v - l) * l / (l - n);
        }
    }
    if x > 1.0 {
        for v in &mut c {
            *v = l + (*v - l) * (1.0 - l) / (x - l);
        }
    }
    c
}

fn set_lum(c: [f32; 3], l: f32) -> [f32; 3] {
    let d = l - lum(c);
    clip_color([c[0] + d, c[1] + d, c[2] + d])
}

fn sat(c: [f32; 3]) -> f32 {
    c[0].max(c[1]).max(c[2]) - c[0].min(c[1]).min(c[2])
}

fn set_sat(c: [f32; 3], s: f32) -> [f32; 3] {
    let mx = c[0].max(c[1]).max(c[2]);
    let mn = c[0].min(c[1]).min(c[2]);
    if mx > mn {
        c.map(|v| (v - mn) * s / (mx - mn))
    } else {
        [0.0; 3]
    }
}

fn blend_channel(mode: u16, b: f32, s: f32) -> f32 {
    match mode {
        1 => b * s,
        2 => b + s - b * s,
        3 => {
            if b <= 0.5 {
                2.0 * b * s
            } else {
                1.0 - 2.0 * (1.0 - b) * (1.0 - s)
            }
        }
        4 => b.min(s),
        5 => b.max(s),
        6 => {
            if b == 0.0 {
                0.0
            } else if s >= 1.0 {
                1.0
            } else {
                (b / (1.0 - s)).min(1.0)
            }
        }
        7 => {
            if b >= 1.0 {
                1.0
            } else if s <= 0.0 {
                0.0
            } else {
                1.0 - ((1.0 - b) / s).min(1.0)
            }
        }
        8 => {
            if s <= 0.5 {
                2.0 * b * s
            } else {
                1.0 - 2.0 * (1.0 - b) * (1.0 - s)
            }
        }
        9 => {
            if s <= 0.5 {
                b - (1.0 - 2.0 * s) * b * (1.0 - b)
            } else {
                let d = if b <= 0.25 {
                    ((16.0 * b - 12.0) * b + 4.0) * b
                } else {
                    b.sqrt()
                };
                b + (2.0 * s - 1.0) * (d - b)
            }
        }
        10 => (b - s).abs(),
        11 => b + s - 2.0 * b * s,
        16 => (b + s).min(1.0),
        17 => (b - s).max(0.0),
        18 => {
            if b == 0.0 {
                0.0
            } else if s == 0.0 {
                1.0
            } else {
                (b / s).min(1.0)
            }
        }
        _ => s,
    }
}

/// Composite `src` over `dst` with the given layer opacity and Aseprite blend mode.
pub fn blend(dst: Rgba, src: Rgba, opacity: u8, mode: u16) -> Rgba {
    let sa = (src[3] as u32 * opacity as u32 + 127) / 255;
    if sa == 0 {
        return dst;
    }
    let da = dst[3] as u32;
    if da == 0 {
        return [src[0], src[1], src[2], sa as u8];
    }
    let mut s = [src[0], src[1], src[2]];
    if mode != 0 {
        let b = [dst[0] as f32 / 255.0, dst[1] as f32 / 255.0, dst[2] as f32 / 255.0];
        let sf = [src[0] as f32 / 255.0, src[1] as f32 / 255.0, src[2] as f32 / 255.0];
        let m = match mode {
            12 => set_lum(set_sat(sf, sat(b)), lum(b)),
            13 => set_lum(set_sat(b, sat(sf)), lum(b)),
            14 => set_lum(sf, lum(b)),
            15 => set_lum(b, lum(sf)),
            _ => [
                blend_channel(mode, b[0], sf[0]),
                blend_channel(mode, b[1], sf[1]),
                blend_channel(mode, b[2], sf[2]),
            ],
        };
        let t = da as f32 / 255.0;
        for i in 0..3 {
            s[i] = ((sf[i] + (m[i] - sf[i]) * t) * 255.0).round().clamp(0.0, 255.0) as u8;
        }
    }
    let ra = sa + da - (sa * da + 127) / 255;
    let mut out = [0u8; 4];
    for i in 0..3 {
        let num = s[i] as u32 * sa * 255 + dst[i] as u32 * da * (255 - sa);
        out[i] = ((num + ra * 255 / 2) / (ra * 255)).min(255) as u8;
    }
    out[3] = ra.min(255) as u8;
    out
}

// ---------------------------------------------------------------- Selection mask

#[derive(Clone, PartialEq)]
pub struct Mask {
    pub w: i32,
    pub h: i32,
    pub data: Vec<bool>,
}

impl Mask {
    pub fn new(w: i32, h: i32) -> Self {
        Self {
            w,
            h,
            data: vec![false; (w.max(0) * h.max(0)) as usize],
        }
    }

    pub fn full(w: i32, h: i32) -> Self {
        Self {
            w,
            h,
            data: vec![true; (w.max(0) * h.max(0)) as usize],
        }
    }

    pub fn rect(w: i32, h: i32, x: i32, y: i32, rw: i32, rh: i32) -> Self {
        let mut m = Self::new(w, h);
        for yy in y.max(0)..(y + rh).min(h) {
            for xx in x.max(0)..(x + rw).min(w) {
                m.data[(yy * w + xx) as usize] = true;
            }
        }
        m
    }

    #[inline]
    pub fn get(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.w && y < self.h && self.data[(y * self.w + x) as usize]
    }

    #[inline]
    pub fn set(&mut self, x: i32, y: i32, v: bool) {
        if x >= 0 && y >= 0 && x < self.w && y < self.h {
            self.data[(y * self.w + x) as usize] = v;
        }
    }

    pub fn bounds(&self) -> Option<(i32, i32, i32, i32)> {
        let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, -1, -1);
        for y in 0..self.h {
            for x in 0..self.w {
                if self.data[(y * self.w + x) as usize] {
                    x0 = x0.min(x);
                    x1 = x1.max(x);
                    y0 = y0.min(y);
                    y1 = y1.max(y);
                }
            }
        }
        if x1 < 0 {
            None
        } else {
            Some((x0, y0, x1 - x0 + 1, y1 - y0 + 1))
        }
    }

    pub fn is_empty(&self) -> bool {
        !self.data.iter().any(|&b| b)
    }

    pub fn inverted(&self) -> Mask {
        Mask {
            w: self.w,
            h: self.h,
            data: self.data.iter().map(|b| !b).collect(),
        }
    }

    /// Boundary edges as runs: (x0, y0, x1, y1) in pixel-corner coordinates.
    pub fn edges(&self) -> Vec<[i32; 4]> {
        let mut out = Vec::new();
        for y in 0..=self.h {
            let mut start: Option<i32> = None;
            for x in 0..=self.w {
                let e = x < self.w && self.get(x, y - 1) != self.get(x, y);
                match (e, start) {
                    (true, None) => start = Some(x),
                    (false, Some(s)) => {
                        out.push([s, y, x, y]);
                        start = None;
                    }
                    _ => {}
                }
            }
        }
        for x in 0..=self.w {
            let mut start: Option<i32> = None;
            for y in 0..=self.h {
                let e = y < self.h && self.get(x - 1, y) != self.get(x, y);
                match (e, start) {
                    (true, None) => start = Some(y),
                    (false, Some(s)) => {
                        out.push([x, s, x, y]);
                        start = None;
                    }
                    _ => {}
                }
            }
        }
        out
    }
}

// ---------------------------------------------------------------- Sprite

#[derive(Clone, Default, PartialEq)]
pub struct UserData {
    pub text: Option<String>,
    pub color: Option<Rgba>,
    /// Raw "properties" payload, preserved verbatim.
    pub props: Vec<u8>,
}

impl UserData {
    pub fn is_empty(&self) -> bool {
        self.text.is_none() && self.color.is_none() && self.props.is_empty()
    }
}

#[derive(Clone)]
pub struct Cel {
    pub x: i32,
    pub y: i32,
    pub opacity: u8,
    pub z: i16,
    pub image: Arc<Image>,
    pub user: UserData,
}

impl Cel {
    pub fn new(image: Image) -> Self {
        Self {
            x: 0,
            y: 0,
            opacity: 255,
            z: 0,
            image: Arc::new(image),
            user: UserData::default(),
        }
    }
}

#[derive(Clone)]
pub struct Layer {
    pub name: String,
    pub visible: bool,
    pub editable: bool,
    /// Remaining Aseprite layer flag bits (background, collapsed, reference…), preserved.
    pub flags: u16,
    pub group: bool,
    pub level: u16,
    pub blend: u16,
    pub opacity: u8,
    pub cels: Vec<Option<Cel>>,
    pub user: UserData,
}

impl Layer {
    pub fn new(name: &str, nframes: usize) -> Self {
        Self {
            name: name.to_string(),
            visible: true,
            editable: true,
            flags: 0,
            group: false,
            level: 0,
            blend: 0,
            opacity: 255,
            cels: vec![None; nframes],
            user: UserData::default(),
        }
    }

    pub fn is_background(&self) -> bool {
        self.flags & 8 != 0
    }
}

#[derive(Clone, PartialEq)]
pub struct Tag {
    pub name: String,
    pub from: usize,
    pub to: usize,
    /// 0 forward, 1 reverse, 2 ping-pong, 3 ping-pong reverse
    pub dir: u8,
    pub repeat: u16,
    pub color: [u8; 3],
    pub user: UserData,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ColorMode {
    Rgba,
    Gray,
    Indexed,
}

#[derive(Clone)]
pub struct Sprite {
    pub w: i32,
    pub h: i32,
    pub mode: ColorMode,
    pub palette: Vec<Rgba>,
    pub transparent_index: u8,
    pub layers: Vec<Layer>,
    pub durations: Vec<u16>,
    pub tags: Vec<Tag>,
    pub grid: (i16, i16, u16, u16),
    pub pixel_ratio: (u8, u8),
    pub color_profile: Option<Vec<u8>>,
    /// Chunks we don't edit (slices, external files, …), kept so saving doesn't lose them.
    pub extra: Vec<(u16, Vec<u8>)>,
}

impl Sprite {
    pub fn new(w: i32, h: i32, background: Option<Rgba>) -> Self {
        let mut layer = Layer::new("Layer 1", 1);
        if let Some(c) = background {
            layer.name = "Background".into();
            layer.cels[0] = Some(Cel::new(Image::filled(w, h, c)));
        }
        Self {
            w,
            h,
            mode: ColorMode::Rgba,
            palette: crate::palettes::PRESETS[0].colors(),
            transparent_index: 0,
            layers: vec![layer],
            durations: vec![100],
            tags: vec![],
            grid: (0, 0, 16, 16),
            pixel_ratio: (1, 1),
            color_profile: None,
            extra: vec![],
        }
    }

    pub fn nframes(&self) -> usize {
        self.durations.len()
    }

    /// A layer is shown only if it and all its parent groups are visible.
    pub fn layer_visible(&self, i: usize) -> bool {
        self.inherited(i, |l| l.visible)
    }

    pub fn layer_editable(&self, i: usize) -> bool {
        self.inherited(i, |l| l.editable)
    }

    fn inherited(&self, i: usize, f: impl Fn(&Layer) -> bool) -> bool {
        if !f(&self.layers[i]) {
            return false;
        }
        let mut level = self.layers[i].level;
        let mut j = i;
        while level > 0 && j > 0 {
            j -= 1;
            if self.layers[j].level < level {
                if !f(&self.layers[j]) {
                    return false;
                }
                level = self.layers[j].level;
            }
        }
        true
    }

    /// Index one past the last descendant of layer `i`.
    pub fn block_end(&self, i: usize) -> usize {
        let level = self.layers[i].level;
        let mut e = i + 1;
        while e < self.layers.len() && self.layers[e].level > level {
            e += 1;
        }
        e
    }

    pub fn composite(&self, frame: usize) -> Image {
        let mut out = Image::new(self.w, self.h);
        self.composite_into(&mut out, frame);
        out
    }

    pub fn composite_into(&self, out: &mut Image, frame: usize) {
        let mut order: Vec<(i32, i16, usize)> = Vec::new();
        for (i, l) in self.layers.iter().enumerate() {
            if l.group || !self.layer_visible(i) {
                continue;
            }
            if let Some(Some(c)) = l.cels.get(frame) {
                order.push((i as i32 + c.z as i32, c.z, i));
            }
        }
        order.sort();
        for (_, _, i) in order {
            let l = &self.layers[i];
            let c = l.cels[frame].as_ref().unwrap();
            let op = ((l.opacity as u32 * c.opacity as u32 + 127) / 255) as u8;
            out.blend_from(&c.image, c.x, c.y, op, l.blend);
        }
    }

    /// Make sure a cel exists at (layer, frame) and covers the whole canvas; returns it for editing.
    pub fn edit_cel(&mut self, layer: usize, frame: usize) -> &mut Cel {
        let (w, h) = (self.w, self.h);
        let slot = &mut self.layers[layer].cels[frame];
        match slot {
            None => *slot = Some(Cel::new(Image::new(w, h))),
            Some(c) => {
                let x0 = c.x.min(0);
                let y0 = c.y.min(0);
                let x1 = (c.x + c.image.w).max(w);
                let y1 = (c.y + c.image.h).max(h);
                if (x0, y0, x1 - x0, y1 - y0) != (c.x, c.y, c.image.w, c.image.h) {
                    c.image = Arc::new(c.image.crop(x0 - c.x, y0 - c.y, x1 - x0, y1 - y0));
                    c.x = x0;
                    c.y = y0;
                }
            }
        }
        slot.as_mut().unwrap()
    }

    // ---- frames

    pub fn insert_frame(&mut self, at: usize, copy_from: Option<usize>) {
        let dur = copy_from.map(|f| self.durations[f]).unwrap_or(100);
        self.durations.insert(at, dur);
        for l in &mut self.layers {
            let cel = copy_from.and_then(|f| l.cels[f].clone()).map(|mut c| {
                c.image = Arc::new((*c.image).clone());
                c
            });
            l.cels.insert(at, cel);
        }
        for t in &mut self.tags {
            // Inserting right after a tag's last frame via duplication extends the tag.
            if t.from >= at {
                t.from += 1;
            }
            if t.to + 1 >= at && (t.to >= at || copy_from == Some(t.to)) {
                t.to += 1;
            }
        }
    }

    pub fn remove_frame(&mut self, i: usize) {
        if self.nframes() <= 1 {
            return;
        }
        self.durations.remove(i);
        for l in &mut self.layers {
            l.cels.remove(i);
        }
        self.tags.retain_mut(|t| {
            if t.from == i && t.to == i {
                return false;
            }
            if t.from > i {
                t.from -= 1;
            }
            if t.to >= i {
                t.to -= 1;
            }
            true
        });
    }

    pub fn swap_frames(&mut self, a: usize, b: usize) {
        self.durations.swap(a, b);
        for l in &mut self.layers {
            l.cels.swap(a, b);
        }
    }

    // ---- layers

    /// Insert a new layer above `above` (inside it when it is a group). Returns the new index.
    pub fn add_layer(&mut self, above: usize, group: bool) -> usize {
        let n = self.nframes();
        let base = if group { "Group" } else { "Layer" };
        let mut k = self.layers.len() + 1;
        let mut name = format!("{base} {k}");
        while self.layers.iter().any(|l| l.name == name) {
            k += 1;
            name = format!("{base} {k}");
        }
        let mut l = Layer::new(&name, n);
        l.group = group;
        let at = if self.layers.is_empty() {
            0
        } else if self.layers[above].group {
            l.level = self.layers[above].level + 1;
            self.block_end(above)
        } else {
            l.level = self.layers[above].level;
            above + 1
        };
        self.layers.insert(at, l);
        at
    }

    pub fn remove_layer(&mut self, i: usize) {
        let e = self.block_end(i);
        self.layers.drain(i..e);
    }

    pub fn duplicate_layer(&mut self, i: usize) -> usize {
        let e = self.block_end(i);
        let mut copies: Vec<Layer> = self.layers[i..e].to_vec();
        for l in &mut copies {
            let mut map: HashMap<*const Image, Arc<Image>> = HashMap::new();
            for c in l.cels.iter_mut().flatten() {
                let key = Arc::as_ptr(&c.image);
                c.image = map.entry(key).or_insert_with(|| Arc::new((*c.image).clone())).clone();
            }
        }
        copies[0].name = format!("{} copy", copies[0].name);
        copies[0].flags &= !8;
        let n = copies.len();
        self.layers.splice(e..e, copies);
        let _ = n;
        e
    }

    /// Move a layer (with its children) one step up/down among its siblings.
    pub fn move_layer(&mut self, i: usize, up: bool) -> Option<usize> {
        let level = self.layers[i].level;
        let e = self.block_end(i);
        if up {
            if e >= self.layers.len() || self.layers[e].level != level {
                return None;
            }
            let e2 = self.block_end(e);
            self.layers[i..e2].rotate_left(e - i);
            Some(i + (e2 - e))
        } else {
            if i == 0 {
                return None;
            }
            let mut j = i - 1;
            while self.layers[j].level > level && j > 0 {
                j -= 1;
            }
            if self.layers[j].level != level {
                return None;
            }
            self.layers[j..e].rotate_left(i - j);
            Some(j)
        }
    }

    pub fn can_merge_down(&self, i: usize) -> bool {
        i > 0 && !self.layers[i].group && !self.layers[i - 1].group && self.layers[i].level == self.layers[i - 1].level
    }

    pub fn merge_down(&mut self, i: usize) {
        if !self.can_merge_down(i) {
            return;
        }
        let upper = self.layers.remove(i);
        for f in 0..self.nframes() {
            if let Some(uc) = &upper.cels[f] {
                let op = ((upper.opacity as u32 * uc.opacity as u32 + 127) / 255) as u8;
                let lc = self.edit_cel(i - 1, f);
                let (lx, ly) = (lc.x, lc.y);
                // The upper cel may extend past the lower one; grow to fit.
                let x0 = lx.min(uc.x);
                let y0 = ly.min(uc.y);
                let x1 = (lx + lc.image.w).max(uc.x + uc.image.w);
                let y1 = (ly + lc.image.h).max(uc.y + uc.image.h);
                let mut img = lc.image.crop(x0 - lx, y0 - ly, x1 - x0, y1 - y0);
                img.blend_from(&uc.image, uc.x - x0, uc.y - y0, op, upper.blend);
                lc.image = Arc::new(img);
                lc.x = x0;
                lc.y = y0;
            }
        }
    }

    pub fn flatten(&mut self) {
        let n = self.nframes();
        let mut l = Layer::new("Flattened", n);
        for f in 0..n {
            let img = self.composite(f);
            if img.bounds().is_some() {
                l.cels[f] = Some(Cel::new(img));
            }
        }
        self.layers = vec![l];
    }

    // ---- whole-sprite transforms

    /// Replace every cel image with `f(canvas-aligned image)`. Linked cels stay linked.
    pub fn map_cels(&mut self, f: impl Fn(&Image) -> Image) {
        let (w, h) = (self.w, self.h);
        let mut cache: HashMap<(*const Image, i32, i32), Arc<Image>> = HashMap::new();
        for l in &mut self.layers {
            for c in l.cels.iter_mut().flatten() {
                let key = (Arc::as_ptr(&c.image), c.x, c.y);
                let img = cache
                    .entry(key)
                    .or_insert_with(|| Arc::new(f(&c.image.crop(-c.x, -c.y, w, h))))
                    .clone();
                c.image = img;
                c.x = 0;
                c.y = 0;
            }
        }
    }

    pub fn resize_canvas(&mut self, nw: i32, nh: i32, dx: i32, dy: i32) {
        self.map_cels(|img| img.crop(-dx, -dy, nw, nh));
        self.w = nw;
        self.h = nh;
    }

    pub fn scale(&mut self, nw: i32, nh: i32) {
        self.map_cels(|img| img.scale_nearest(nw, nh));
        self.w = nw;
        self.h = nh;
    }

    pub fn flip(&mut self, horizontal: bool) {
        self.map_cels(|img| if horizontal { img.flip_h() } else { img.flip_v() });
    }

    pub fn rotate(&mut self, quarter_turns_cw: u8) {
        for _ in 0..quarter_turns_cw % 4 {
            self.map_cels(|img| img.rot90cw());
            std::mem::swap(&mut self.w, &mut self.h);
        }
    }

    /// Bounding box of all visible content across all frames.
    pub fn content_bounds(&self) -> Option<(i32, i32, i32, i32)> {
        let mut r: Option<(i32, i32, i32, i32)> = None;
        for f in 0..self.nframes() {
            if let Some((x, y, w, h)) = self.composite(f).bounds() {
                r = Some(match r {
                    None => (x, y, x + w, y + h),
                    Some((a, b, c, d)) => (a.min(x), b.min(y), c.max(x + w), d.max(y + h)),
                });
            }
        }
        r.map(|(a, b, c, d)| (a, b, c - a, d - b))
    }

    /// Snap every pixel to the nearest palette color (used when switching to indexed mode).
    pub fn quantize_to_palette(&mut self) {
        let pal = self.palette.clone();
        let ti = self.transparent_index as usize;
        self.map_cels(|img| {
            let mut out = img.clone();
            let mut cache: HashMap<Rgba, Rgba> = HashMap::new();
            for px in out.data.chunks_exact_mut(4) {
                let c = [px[0], px[1], px[2], px[3]];
                if c[3] == 0 {
                    continue;
                }
                let q = *cache.entry(c).or_insert_with(|| pal[nearest_color(c, &pal, Some(ti))]);
                px.copy_from_slice(&q);
            }
            out
        });
    }

    pub fn make_grayscale(&mut self) {
        self.map_cels(|img| {
            let mut out = img.clone();
            for px in out.data.chunks_exact_mut(4) {
                let v = luma([px[0], px[1], px[2], px[3]]);
                px[0] = v;
                px[1] = v;
                px[2] = v;
            }
            out
        });
    }
}

pub fn luma(c: Rgba) -> u8 {
    ((c[0] as u32 * 2126 + c[1] as u32 * 7152 + c[2] as u32 * 722 + 5000) / 10000) as u8
}

/// Index of the palette entry closest to `c`, optionally skipping one index.
pub fn nearest_color(c: Rgba, pal: &[Rgba], skip: Option<usize>) -> usize {
    let mut best = 0;
    let mut best_d = i64::MAX;
    for (i, p) in pal.iter().enumerate() {
        if Some(i) == skip && pal.len() > 1 {
            continue;
        }
        let d: i64 = (0..4)
            .map(|k| {
                let d = c[k] as i64 - p[k] as i64;
                d * d
            })
            .sum();
        if d < best_d {
            best_d = d;
            best = i;
        }
    }
    best
}
