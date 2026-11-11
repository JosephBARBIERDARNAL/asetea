//! Reader and writer for the Aseprite (.ase / .aseprite) file format.

use crate::doc::*;
use flate2::Compression;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::Arc;

struct Rd<'a> {
    d: &'a [u8],
    p: usize,
}

impl<'a> Rd<'a> {
    fn new(d: &'a [u8]) -> Self {
        Self { d, p: 0 }
    }
    fn bytes(&mut self, n: usize) -> Result<&'a [u8], String> {
        if self.p + n > self.d.len() {
            return Err("unexpected end of file".into());
        }
        let s = &self.d[self.p..self.p + n];
        self.p += n;
        Ok(s)
    }
    fn skip(&mut self, n: usize) -> Result<(), String> {
        self.bytes(n).map(|_| ())
    }
    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.bytes(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, String> {
        let b = self.bytes(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }
    fn i16(&mut self) -> Result<i16, String> {
        Ok(self.u16()? as i16)
    }
    fn u32(&mut self) -> Result<u32, String> {
        let b = self.bytes(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn string(&mut self) -> Result<String, String> {
        let n = self.u16()? as usize;
        Ok(String::from_utf8_lossy(self.bytes(n)?).into_owned())
    }
    fn rest(&mut self) -> &'a [u8] {
        let s = &self.d[self.p..];
        self.p = self.d.len();
        s
    }
}

fn inflate(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    ZlibDecoder::new(data)
        .read_to_end(&mut out)
        .map_err(|e| format!("corrupt compressed data: {e}"))?;
    Ok(out)
}

enum RawCel {
    Image {
        w: i32,
        h: i32,
        px: Vec<u8>,
    },
    Linked(usize),
    Tilemap {
        w: i32,
        h: i32,
        tiles: Vec<u32>,
        id_mask: u32,
        xflip: u32,
        yflip: u32,
    },
}

struct RawCelEntry {
    frame: usize,
    layer: usize,
    x: i32,
    y: i32,
    opacity: u8,
    z: i16,
    kind: RawCel,
    user: UserData,
}

struct Tileset {
    tw: i32,
    th: i32,
    px: Vec<u8>,
}

enum Last {
    None,
    Layer(usize),
    Cel(usize),
    Tags(usize),
    Raw,
}

fn parse_user_data(r: &mut Rd) -> Result<UserData, String> {
    let flags = r.u32()?;
    let mut u = UserData::default();
    if flags & 1 != 0 {
        u.text = Some(r.string()?);
    }
    if flags & 2 != 0 {
        let b = r.bytes(4)?;
        u.color = Some([b[0], b[1], b[2], b[3]]);
    }
    if flags & 4 != 0 {
        u.props = r.rest().to_vec();
    }
    Ok(u)
}

/// Load a sprite. Returns the sprite and a list of human-readable warnings.
pub fn load(data: &[u8]) -> Result<(Sprite, Vec<String>), String> {
    let mut warnings = Vec::new();
    let mut r = Rd::new(data);
    r.u32()?;
    if r.u16()? != 0xA5E0 {
        return Err("not an Aseprite file (bad magic number)".into());
    }
    let nframes = r.u16()? as usize;
    let w = r.u16()? as i32;
    let h = r.u16()? as i32;
    let depth = r.u16()?;
    let hflags = r.u32()?;
    let speed = r.u16()?;
    r.skip(8)?;
    let transparent_index = r.u8()?;
    r.skip(3)?;
    r.u16()?;
    let pw = r.u8()?;
    let ph = r.u8()?;
    let gx = r.i16()?;
    let gy = r.i16()?;
    let gw = r.u16()?;
    let gh = r.u16()?;
    r.skip(84)?;
    let mode = match depth {
        32 => ColorMode::Rgba,
        16 => ColorMode::Gray,
        8 => ColorMode::Indexed,
        d => return Err(format!("unsupported color depth {d}")),
    };
    let bpp = (depth / 8) as usize;
    if w == 0 || h == 0 || nframes == 0 {
        return Err("sprite has no frames or zero size".into());
    }

    let mut layers: Vec<Layer> = Vec::new();
    let mut layer_tileset: HashMap<usize, u32> = HashMap::new();
    let mut cels: Vec<RawCelEntry> = Vec::new();
    let mut palette: Vec<Rgba> = Vec::new();
    let mut old_palette: Vec<Rgba> = Vec::new();
    let mut has_new_palette = false;
    let mut tilesets: HashMap<u32, Tileset> = HashMap::new();
    let mut tags: Vec<Tag> = Vec::new();
    let mut durations = Vec::new();
    let mut color_profile = None;
    let mut extra: Vec<(u16, Vec<u8>)> = Vec::new();
    let mut had_tilemaps = false;

    for frame in 0..nframes {
        let fstart = r.p;
        let fsize = r.u32()? as usize;
        if r.u16()? != 0xF1FA {
            return Err(format!("bad frame header at frame {frame}"));
        }
        let old_chunks = r.u16()? as u32;
        let mut dur = r.u16()?;
        if dur == 0 {
            dur = if speed > 0 { speed } else { 100 };
        }
        durations.push(dur);
        r.skip(2)?;
        let new_chunks = r.u32()?;
        let nchunks = if new_chunks != 0 { new_chunks } else { old_chunks };
        let mut last = Last::None;
        for _ in 0..nchunks {
            let cstart = r.p;
            let csize = r.u32()? as usize;
            let ctype = r.u16()?;
            if csize < 6 {
                return Err("corrupt chunk".into());
            }
            let body = r.bytes(csize - 6)?;
            let mut c = Rd::new(body);
            let _ = cstart;
            match ctype {
                0x0004 | 0x0011 => {
                    let packets = c.u16()?;
                    let mut idx = 0usize;
                    for _ in 0..packets {
                        idx += c.u8()? as usize;
                        let mut n = c.u8()? as usize;
                        if n == 0 {
                            n = 256;
                        }
                        for _ in 0..n {
                            let b = c.bytes(3)?;
                            let s = |v: u8| {
                                if ctype == 0x0011 {
                                    (v as u32 * 255 / 63) as u8
                                } else {
                                    v
                                }
                            };
                            if old_palette.len() <= idx {
                                old_palette.resize(idx + 1, [0, 0, 0, 255]);
                            }
                            old_palette[idx] = [s(b[0]), s(b[1]), s(b[2]), 255];
                            idx += 1;
                        }
                    }
                    last = Last::None;
                }
                0x2004 => {
                    let flags = c.u16()?;
                    let ltype = c.u16()?;
                    let level = c.u16()?;
                    c.skip(4)?;
                    let blend = c.u16()?;
                    let opacity = c.u8()?;
                    c.skip(3)?;
                    let name = c.string()?;
                    if ltype == 2 {
                        layer_tileset.insert(layers.len(), c.u32()?);
                        had_tilemaps = true;
                    }
                    layers.push(Layer {
                        name,
                        visible: flags & 1 != 0,
                        editable: flags & 2 != 0,
                        flags: flags & !3,
                        group: ltype == 1,
                        level,
                        blend: if blend > 18 { 0 } else { blend },
                        opacity: if hflags & 1 != 0 { opacity } else { 255 },
                        cels: vec![None; nframes],
                        user: UserData::default(),
                    });
                    last = Last::Layer(layers.len() - 1);
                }
                0x2005 => {
                    let layer = c.u16()? as usize;
                    let x = c.i16()? as i32;
                    let y = c.i16()? as i32;
                    let opacity = c.u8()?;
                    let ctype = c.u16()?;
                    let z = c.i16()?;
                    c.skip(5)?;
                    let kind = match ctype {
                        0 => {
                            let cw = c.u16()? as i32;
                            let ch = c.u16()? as i32;
                            RawCel::Image {
                                w: cw,
                                h: ch,
                                px: c.rest().to_vec(),
                            }
                        }
                        1 => RawCel::Linked(c.u16()? as usize),
                        2 => {
                            let cw = c.u16()? as i32;
                            let ch = c.u16()? as i32;
                            RawCel::Image {
                                w: cw,
                                h: ch,
                                px: inflate(c.rest())?,
                            }
                        }
                        3 => {
                            let cw = c.u16()? as i32;
                            let ch = c.u16()? as i32;
                            let bits = c.u16()?;
                            let id_mask = c.u32()?;
                            let xflip = c.u32()?;
                            let yflip = c.u32()?;
                            c.skip(4 + 10)?;
                            let raw = inflate(c.rest())?;
                            let bytes = (bits / 8).max(1) as usize;
                            let tiles = raw
                                .chunks_exact(bytes)
                                .map(|b| {
                                    let mut v = [0u8; 4];
                                    v[..bytes.min(4)].copy_from_slice(&b[..bytes.min(4)]);
                                    u32::from_le_bytes(v)
                                })
                                .collect();
                            RawCel::Tilemap {
                                w: cw,
                                h: ch,
                                tiles,
                                id_mask,
                                xflip,
                                yflip,
                            }
                        }
                        t => return Err(format!("unknown cel type {t}")),
                    };
                    cels.push(RawCelEntry {
                        frame,
                        layer,
                        x,
                        y,
                        opacity,
                        z,
                        kind,
                        user: UserData::default(),
                    });
                    last = Last::Cel(cels.len() - 1);
                }
                0x2007 => {
                    color_profile = Some(body.to_vec());
                    last = Last::None;
                }
                0x2018 => {
                    let n = c.u16()?;
                    c.skip(8)?;
                    let first = tags.len();
                    for _ in 0..n {
                        let from = c.u16()? as usize;
                        let to = c.u16()? as usize;
                        let dir = c.u8()?;
                        let repeat = c.u16()?;
                        c.skip(6)?;
                        let rgb = c.bytes(3)?;
                        c.skip(1)?;
                        let name = c.string()?;
                        tags.push(Tag {
                            name,
                            from: from.min(nframes - 1),
                            to: to.min(nframes - 1),
                            dir,
                            repeat,
                            color: [rgb[0], rgb[1], rgb[2]],
                            user: UserData::default(),
                        });
                    }
                    last = Last::Tags(first);
                }
                0x2019 => {
                    has_new_palette = true;
                    let size = c.u32()? as usize;
                    let from = c.u32()? as usize;
                    let to = c.u32()? as usize;
                    c.skip(8)?;
                    if size > 65536 || to >= 65536 {
                        return Err("corrupt palette".into());
                    }
                    palette.resize(size.max(palette.len().min(size)), [0, 0, 0, 255]);
                    palette.truncate(size);
                    for i in from..=to {
                        let f = c.u16()?;
                        let b = c.bytes(4)?;
                        if i < palette.len() {
                            palette[i] = [b[0], b[1], b[2], b[3]];
                        }
                        if f & 1 != 0 {
                            c.string()?;
                        }
                    }
                    last = Last::None;
                }
                0x2020 => match last {
                    Last::Layer(i) => {
                        layers[i].user = parse_user_data(&mut c)?;
                        last = Last::None;
                    }
                    Last::Cel(i) => {
                        cels[i].user = parse_user_data(&mut c)?;
                        last = Last::None;
                    }
                    Last::Tags(i) => {
                        if i < tags.len() {
                            let mut u = parse_user_data(&mut c)?;
                            if let Some(col) = u.color.take() {
                                tags[i].color = [col[0], col[1], col[2]];
                            }
                            tags[i].user = u;
                            last = Last::Tags(i + 1);
                        }
                    }
                    Last::Raw => extra.push((ctype, body.to_vec())),
                    Last::None => {}
                },
                0x2023 => {
                    let id = c.u32()?;
                    let flags = c.u32()?;
                    let ntiles = c.u32()? as i32;
                    let tw = c.u16()? as i32;
                    let th = c.u16()? as i32;
                    c.skip(2 + 14)?;
                    c.string()?;
                    if flags & 1 != 0 {
                        c.skip(8)?;
                    }
                    if flags & 2 != 0 {
                        let len = c.u32()? as usize;
                        let raw = inflate(c.bytes(len)?)?;
                        let _ = ntiles;
                        tilesets.insert(id, Tileset { tw, th, px: raw });
                    }
                    last = Last::None;
                }
                0x2006 => last = Last::None, // cel extra: precise bounds, not needed
                _ => {
                    // Slices, external files and anything newer: keep the bytes untouched.
                    extra.push((ctype, body.to_vec()));
                    last = Last::Raw;
                }
            }
        }
        // Trust the frame size over chunk bookkeeping.
        if fsize >= 16 && fstart + fsize <= data.len() {
            r.p = fstart + fsize;
        }
    }

    if !has_new_palette {
        palette = old_palette;
    }
    if palette.is_empty() && mode == ColorMode::Indexed {
        palette = (0..=255u8).map(|v| [v, v, v, 255]).collect();
    }

    let to_rgba = |px: &[u8], n: usize, background: bool| -> Vec<u8> {
        let mut out = vec![0u8; n * 4];
        for i in 0..n {
            let o = &mut out[i * 4..i * 4 + 4];
            match mode {
                ColorMode::Rgba => {
                    if let Some(s) = px.get(i * 4..i * 4 + 4) {
                        o.copy_from_slice(s);
                    }
                }
                ColorMode::Gray => {
                    if let Some(s) = px.get(i * 2..i * 2 + 2) {
                        o.copy_from_slice(&[s[0], s[0], s[0], s[1]]);
                    }
                }
                ColorMode::Indexed => {
                    if let Some(&idx) = px.get(i)
                        && (idx != transparent_index || background)
                        && let Some(c) = palette.get(idx as usize)
                    {
                        o.copy_from_slice(c);
                    }
                }
            }
        }
        out
    };
    let _ = bpp;

    for rc in cels {
        if rc.layer >= layers.len() || rc.frame >= nframes {
            continue;
        }
        let background = layers[rc.layer].is_background();
        let image = match rc.kind {
            RawCel::Image { w, h, px } => Arc::new(Image {
                w,
                h,
                data: to_rgba(&px, (w * h) as usize, background),
            }),
            RawCel::Linked(f) => match layers[rc.layer].cels.get(f).and_then(|c| c.as_ref()) {
                Some(src) => {
                    let mut c = src.clone();
                    c.user = rc.user;
                    layers[rc.layer].cels[rc.frame] = Some(c);
                    continue;
                }
                None => continue,
            },
            RawCel::Tilemap {
                w,
                h,
                tiles,
                id_mask,
                xflip,
                yflip,
            } => {
                let Some(ts) = layer_tileset.get(&rc.layer).and_then(|id| tilesets.get(id)) else {
                    continue;
                };
                let (tw, th) = (ts.tw, ts.th);
                let ntiles = if tw * th > 0 {
                    ts.px.len() / (tw * th) as usize / bpp
                } else {
                    0
                };
                let tpx = to_rgba(&ts.px, ntiles * (tw * th) as usize, false);
                let mut img = Image::new(w * tw, h * th);
                for ty in 0..h {
                    for tx in 0..w {
                        let Some(&t) = tiles.get((ty * w + tx) as usize) else {
                            continue;
                        };
                        let id = (t & id_mask) as usize;
                        if id == 0 || id >= ntiles {
                            continue;
                        }
                        for py in 0..th {
                            for px in 0..tw {
                                let sx = if t & xflip != 0 { tw - 1 - px } else { px };
                                let sy = if t & yflip != 0 { th - 1 - py } else { py };
                                let si = (id * (tw * th) as usize + (sy * tw + sx) as usize) * 4;
                                let c = [tpx[si], tpx[si + 1], tpx[si + 2], tpx[si + 3]];
                                img.set(tx * tw + px, ty * th + py, c);
                            }
                        }
                    }
                }
                Arc::new(img)
            }
        };
        layers[rc.layer].cels[rc.frame] = Some(Cel {
            x: rc.x,
            y: rc.y,
            opacity: rc.opacity,
            z: rc.z,
            image,
            user: rc.user,
        });
    }

    if had_tilemaps {
        warnings.push(
            "This file uses tilemap layers. They were converted to regular image layers; saving will not keep the tilesets."
                .into(),
        );
    }
    if layers.is_empty() {
        layers.push(Layer::new("Layer 1", nframes));
    }
    // Repair impossible hierarchies so the rest of the app can rely on them.
    let mut prev_level = 0u16;
    let mut prev_group = false;
    for (i, l) in layers.iter_mut().enumerate() {
        let max = if i == 0 {
            0
        } else if prev_group {
            prev_level + 1
        } else {
            prev_level
        };
        if l.level > max {
            l.level = max;
        }
        prev_level = l.level;
        prev_group = l.group;
    }

    Ok((
        Sprite {
            w,
            h,
            mode,
            palette,
            transparent_index,
            layers,
            durations,
            tags,
            grid: (gx, gy, if gw == 0 { 16 } else { gw }, if gh == 0 { 16 } else { gh }),
            pixel_ratio: (pw.max(1), ph.max(1)),
            color_profile,
            extra,
        },
        warnings,
    ))
}

// ---------------------------------------------------------------- Writer

fn put_u16(o: &mut Vec<u8>, v: u16) {
    o.extend_from_slice(&v.to_le_bytes());
}
fn put_u32(o: &mut Vec<u8>, v: u32) {
    o.extend_from_slice(&v.to_le_bytes());
}
fn put_str(o: &mut Vec<u8>, s: &str) {
    let b = &s.as_bytes()[..s.len().min(65535)];
    put_u16(o, b.len() as u16);
    o.extend_from_slice(b);
}
fn put_chunk(o: &mut Vec<u8>, count: &mut u32, ctype: u16, body: &[u8]) {
    put_u32(o, body.len() as u32 + 6);
    put_u16(o, ctype);
    o.extend_from_slice(body);
    *count += 1;
}
fn put_user(o: &mut Vec<u8>, count: &mut u32, u: &UserData) {
    let mut b = Vec::new();
    let flags = u.text.is_some() as u32 | (u.color.is_some() as u32) << 1 | (!u.props.is_empty() as u32) << 2;
    put_u32(&mut b, flags);
    if let Some(t) = &u.text {
        put_str(&mut b, t);
    }
    if let Some(c) = u.color {
        b.extend_from_slice(&c);
    }
    b.extend_from_slice(&u.props);
    put_chunk(o, count, 0x2020, &b);
}

pub fn save(s: &Sprite) -> Vec<u8> {
    let nframes = s.nframes();
    let depth: u16 = match s.mode {
        ColorMode::Rgba => 32,
        ColorMode::Gray => 16,
        ColorMode::Indexed => 8,
    };
    let palette: Vec<Rgba> = if s.palette.is_empty() {
        vec![[0, 0, 0, 255]]
    } else {
        s.palette.clone()
    };
    let ti = s.transparent_index as usize;
    let mut index_cache: HashMap<(Rgba, bool), u8> = HashMap::new();
    let pal256 = &palette[..palette.len().min(256)];
    let mut encode = |img: &Image, background: bool| -> Vec<u8> {
        match s.mode {
            ColorMode::Rgba => img.data.clone(),
            ColorMode::Gray => img
                .data
                .chunks_exact(4)
                .flat_map(|p| [luma([p[0], p[1], p[2], p[3]]), p[3]])
                .collect(),
            ColorMode::Indexed => img
                .data
                .chunks_exact(4)
                .map(|p| {
                    let c = [p[0], p[1], p[2], p[3]];
                    if c[3] == 0 && !background {
                        return ti as u8;
                    }
                    *index_cache
                        .entry((c, background))
                        .or_insert_with(|| nearest_color(c, pal256, if background { None } else { Some(ti) }) as u8)
                })
                .collect(),
        }
    };

    let mut out = vec![0u8; 128];
    for f in 0..nframes {
        let mut body = Vec::new();
        let mut n = 0u32;
        if f == 0 {
            match &s.color_profile {
                Some(cp) => put_chunk(&mut body, &mut n, 0x2007, cp),
                None => {
                    let mut b = Vec::new();
                    put_u16(&mut b, 1);
                    put_u16(&mut b, 0);
                    put_u32(&mut b, 0);
                    b.extend_from_slice(&[0; 8]);
                    put_chunk(&mut body, &mut n, 0x2007, &b);
                }
            }
            let mut b = Vec::new();
            put_u32(&mut b, palette.len() as u32);
            put_u32(&mut b, 0);
            put_u32(&mut b, palette.len() as u32 - 1);
            b.extend_from_slice(&[0; 8]);
            for c in &palette {
                put_u16(&mut b, 0);
                b.extend_from_slice(c);
            }
            put_chunk(&mut body, &mut n, 0x2019, &b);

            for l in &s.layers {
                let mut b = Vec::new();
                put_u16(&mut b, (l.flags & !3) | l.visible as u16 | (l.editable as u16) << 1);
                put_u16(&mut b, l.group as u16);
                put_u16(&mut b, l.level);
                put_u16(&mut b, 0);
                put_u16(&mut b, 0);
                put_u16(&mut b, l.blend);
                b.push(l.opacity);
                b.extend_from_slice(&[0; 3]);
                put_str(&mut b, &l.name);
                put_chunk(&mut body, &mut n, 0x2004, &b);
                if !l.user.is_empty() {
                    put_user(&mut body, &mut n, &l.user);
                }
            }
            if !s.tags.is_empty() {
                let mut b = Vec::new();
                put_u16(&mut b, s.tags.len() as u16);
                b.extend_from_slice(&[0; 8]);
                for t in &s.tags {
                    put_u16(&mut b, t.from as u16);
                    put_u16(&mut b, t.to as u16);
                    b.push(t.dir);
                    put_u16(&mut b, t.repeat);
                    b.extend_from_slice(&[0; 6]);
                    b.extend_from_slice(&t.color);
                    b.push(0);
                    put_str(&mut b, &t.name);
                }
                put_chunk(&mut body, &mut n, 0x2018, &b);
                for t in &s.tags {
                    let mut u = t.user.clone();
                    u.color = Some([t.color[0], t.color[1], t.color[2], 255]);
                    put_user(&mut body, &mut n, &u);
                }
            }
            for (ctype, data) in &s.extra {
                put_chunk(&mut body, &mut n, *ctype, data);
            }
        }
        for (li, l) in s.layers.iter().enumerate() {
            if l.group {
                continue;
            }
            let Some(cel) = &l.cels[f] else { continue };
            let Some((bx, by, bw, bh)) = cel.image.bounds() else {
                continue;
            };
            let mut b = Vec::new();
            put_u16(&mut b, li as u16);
            // A cel that shares pixels and placement with an earlier frame is written as a link.
            let link = (0..f).find(|&pf| {
                l.cels[pf].as_ref().is_some_and(|p| {
                    Arc::ptr_eq(&p.image, &cel.image) && p.x == cel.x && p.y == cel.y && p.opacity == cel.opacity
                })
            });
            let (x, y) = (cel.x + bx, cel.y + by);
            put_u16(&mut b, x.clamp(-32768, 32767) as i16 as u16);
            put_u16(&mut b, y.clamp(-32768, 32767) as i16 as u16);
            b.push(cel.opacity);
            put_u16(&mut b, if link.is_some() { 1 } else { 2 });
            put_u16(&mut b, cel.z as u16);
            b.extend_from_slice(&[0; 5]);
            match link {
                Some(pf) => put_u16(&mut b, pf as u16),
                None => {
                    put_u16(&mut b, bw as u16);
                    put_u16(&mut b, bh as u16);
                    let trimmed;
                    let img: &Image = if (bx, by, bw, bh) == (0, 0, cel.image.w, cel.image.h) {
                        &cel.image
                    } else {
                        trimmed = cel.image.crop(bx, by, bw, bh);
                        &trimmed
                    };
                    let raw = encode(img, l.is_background());
                    let mut z = ZlibEncoder::new(Vec::new(), Compression::default());
                    z.write_all(&raw).unwrap();
                    b.extend_from_slice(&z.finish().unwrap());
                }
            }
            put_chunk(&mut body, &mut n, 0x2005, &b);
            if !cel.user.is_empty() {
                put_user(&mut body, &mut n, &cel.user);
            }
        }
        put_u32(&mut out, body.len() as u32 + 16);
        put_u16(&mut out, 0xF1FA);
        put_u16(&mut out, n.min(0xFFFF) as u16);
        put_u16(&mut out, s.durations[f]);
        out.extend_from_slice(&[0; 2]);
        put_u32(&mut out, n);
        out.extend_from_slice(&body);
    }

    let mut hd = Vec::with_capacity(128);
    put_u32(&mut hd, out.len() as u32);
    put_u16(&mut hd, 0xA5E0);
    put_u16(&mut hd, nframes as u16);
    put_u16(&mut hd, s.w as u16);
    put_u16(&mut hd, s.h as u16);
    put_u16(&mut hd, depth);
    put_u32(&mut hd, 1);
    put_u16(&mut hd, s.durations[0]);
    put_u32(&mut hd, 0);
    put_u32(&mut hd, 0);
    hd.push(s.transparent_index);
    hd.extend_from_slice(&[0; 3]);
    put_u16(&mut hd, palette.len().min(65535) as u16);
    hd.push(s.pixel_ratio.0);
    hd.push(s.pixel_ratio.1);
    put_u16(&mut hd, s.grid.0 as u16);
    put_u16(&mut hd, s.grid.1 as u16);
    put_u16(&mut hd, s.grid.2);
    put_u16(&mut hd, s.grid.3);
    hd.resize(128, 0);
    out[..128].copy_from_slice(&hd);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn samples() -> Vec<std::path::PathBuf> {
        let mut v: Vec<_> = std::fs::read_dir(env!("CARGO_MANIFEST_DIR"))
            .unwrap()
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|e| e == "aseprite" || e == "ase"))
            .collect();
        v.sort();
        v
    }

    #[test]
    fn roundtrip_samples() {
        let files = samples();
        assert!(!files.is_empty());
        for p in files {
            let data = std::fs::read(&p).unwrap();
            let (a, warn) = load(&data).unwrap();
            println!(
                "{}: {}x{} {:?} frames={} layers={:?} tags={:?} palette={} extra={:?} warnings={:?}",
                p.file_name().unwrap().to_string_lossy(),
                a.w,
                a.h,
                a.mode,
                a.nframes(),
                a.layers
                    .iter()
                    .map(|l| (l.name.as_str(), l.level, l.group, l.blend, l.opacity))
                    .collect::<Vec<_>>(),
                a.tags
                    .iter()
                    .map(|t| (t.name.as_str(), t.from, t.to))
                    .collect::<Vec<_>>(),
                a.palette.len(),
                a.extra.iter().map(|e| (e.0, e.1.len())).collect::<Vec<_>>(),
                warn
            );
            let saved = save(&a);
            let (b, _) = load(&saved).unwrap();
            assert_eq!((a.w, a.h, a.mode, a.nframes()), (b.w, b.h, b.mode, b.nframes()));
            assert_eq!(a.palette, b.palette);
            assert_eq!(a.durations, b.durations);
            assert!(a.tags == b.tags);
            assert_eq!(a.layers.len(), b.layers.len());
            for (la, lb) in a.layers.iter().zip(&b.layers) {
                assert_eq!(la.name, lb.name);
                assert_eq!(
                    (la.visible, la.blend, la.opacity, la.level, la.group),
                    (lb.visible, lb.blend, lb.opacity, lb.level, lb.group)
                );
            }
            for f in 0..a.nframes() {
                assert!(a.composite(f) == b.composite(f), "frame {f} differs in {p:?}");
            }
            // Saving twice must be stable.
            assert_eq!(saved, save(&b));
        }
    }

    #[test]
    fn roundtrip_modes_and_links() {
        let mut s = Sprite::new(8, 8, None);
        s.palette = vec![[0, 0, 0, 0], [255, 0, 0, 255], [0, 255, 0, 255], [0, 0, 255, 255]];
        let c = s.edit_cel(0, 0);
        let img = Arc::make_mut(&mut c.image);
        img.set(1, 1, [255, 0, 0, 255]);
        img.set(6, 5, [0, 0, 255, 255]);
        s.durations.push(250);
        let linked = s.layers[0].cels[0].clone();
        s.layers[0].cels.push(linked);
        s.tags.push(Tag {
            name: "walk".into(),
            from: 0,
            to: 1,
            dir: 2,
            repeat: 0,
            color: [10, 20, 30],
            user: UserData::default(),
        });
        for mode in [ColorMode::Rgba, ColorMode::Indexed] {
            s.mode = mode;
            let (b, _) = load(&save(&s)).unwrap();
            assert!(b.mode == mode);
            assert!(s.composite(0) == b.composite(0));
            assert!(s.composite(1) == b.composite(1));
            assert!(Arc::ptr_eq(
                &b.layers[0].cels[0].as_ref().unwrap().image,
                &b.layers[0].cels[1].as_ref().unwrap().image
            ));
            assert!(b.tags == s.tags);
            assert_eq!(b.durations, vec![100, 250]);
        }
        s.mode = ColorMode::Gray;
        s.make_grayscale();
        let (b, _) = load(&save(&s)).unwrap();
        assert!(s.composite(0) == b.composite(0));
    }
}
