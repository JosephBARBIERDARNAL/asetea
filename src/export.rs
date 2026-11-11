//! Import and export of non-Aseprite formats: PNG, GIF, sprite sheets.

use crate::doc::*;
use image::AnimationDecoder;
use std::path::Path;

fn to_buf(img: &Image, scale: u32) -> image::RgbaImage {
    let s = scale.max(1) as i32;
    let scaled;
    let img = if s == 1 {
        img
    } else {
        scaled = img.scale_nearest(img.w * s, img.h * s);
        &scaled
    };
    image::RgbaImage::from_raw(img.w as u32, img.h as u32, img.data.clone()).expect("buffer size")
}

pub fn save_png(path: &Path, img: &Image, scale: u32) -> Result<(), String> {
    to_buf(img, scale).save(path).map_err(|e| e.to_string())
}

/// Frames `from..=to` written as name_001.png, name_002.png, …
pub fn save_png_sequence(path: &Path, s: &Sprite, from: usize, to: usize, scale: u32) -> Result<usize, String> {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "frame".into());
    let dir = path.parent().unwrap_or(Path::new("."));
    for f in from..=to {
        save_png(&dir.join(format!("{stem}_{:03}.png", f + 1)), &s.composite(f), scale)?;
    }
    Ok(to - from + 1)
}

pub fn save_gif(path: &Path, s: &Sprite, from: usize, to: usize, scale: u32) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut enc = image::codecs::gif::GifEncoder::new_with_speed(std::io::BufWriter::new(file), 10);
    enc.set_repeat(image::codecs::gif::Repeat::Infinite)
        .map_err(|e| e.to_string())?;
    for f in from..=to {
        let buf = to_buf(&s.composite(f), scale);
        let delay = image::Delay::from_numer_denom_ms(s.durations[f] as u32, 1);
        enc.encode_frame(image::Frame::from_parts(buf, 0, 0, delay))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn json_str(s: &str) -> String {
    let mut o = String::from("\"");
    for ch in s.chars() {
        match ch {
            '"' => o += "\\\"",
            '\\' => o += "\\\\",
            '\n' => o += "\\n",
            c if (c as u32) < 0x20 => o += &format!("\\u{:04x}", c as u32),
            c => o.push(c),
        }
    }
    o + "\""
}

/// Lay all frames out on a grid. Optionally writes Aseprite-compatible JSON metadata next to it.
pub fn save_sheet(path: &Path, s: &Sprite, cols: u32, scale: u32, pad: u32, json: bool) -> Result<(), String> {
    let n = s.nframes() as i32;
    let cols = (cols.max(1) as i32).min(n);
    let rows = (n + cols - 1) / cols;
    let sc = scale.max(1) as i32;
    let (fw, fh, pad) = (s.w * sc, s.h * sc, pad as i32);
    let (sw, sh) = (cols * fw + (cols + 1) * pad, rows * fh + (rows + 1) * pad);
    let mut sheet = Image::new(sw, sh);
    let mut frames = Vec::new();
    let name = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    for f in 0..n {
        let x = pad + (f % cols) * (fw + pad);
        let y = pad + (f / cols) * (fh + pad);
        let img = s.composite(f as usize);
        let img = if sc == 1 { img } else { img.scale_nearest(fw, fh) };
        sheet.blend_from(&img, x, y, 255, 0);
        frames.push(format!(
            "    {{\n      \"filename\": {},\n      \"frame\": {{ \"x\": {x}, \"y\": {y}, \"w\": {fw}, \"h\": {fh} }},\n      \"rotated\": false,\n      \"trimmed\": false,\n      \"spriteSourceSize\": {{ \"x\": 0, \"y\": 0, \"w\": {fw}, \"h\": {fh} }},\n      \"sourceSize\": {{ \"w\": {fw}, \"h\": {fh} }},\n      \"duration\": {}\n    }}",
            json_str(&format!("{name} {f}")),
            s.durations[f as usize]
        ));
    }
    save_png(path, &sheet, 1)?;
    if json {
        let dirs = ["forward", "reverse", "pingpong", "pingpong_reverse"];
        let tags: Vec<String> = s
            .tags
            .iter()
            .map(|t| {
                format!(
                    "      {{ \"name\": {}, \"from\": {}, \"to\": {}, \"direction\": \"{}\", \"color\": \"#{:02x}{:02x}{:02x}ff\" }}",
                    json_str(&t.name),
                    t.from,
                    t.to,
                    dirs[(t.dir as usize).min(3)],
                    t.color[0],
                    t.color[1],
                    t.color[2]
                )
            })
            .collect();
        let image = path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let text = format!(
            "{{\n  \"frames\": [\n{}\n  ],\n  \"meta\": {{\n    \"app\": \"asetea\",\n    \"image\": {},\n    \"format\": \"RGBA8888\",\n    \"size\": {{ \"w\": {sw}, \"h\": {sh} }},\n    \"scale\": \"{sc}\",\n    \"frameTags\": [\n{}\n    ]\n  }}\n}}\n",
            frames.join(",\n"),
            json_str(&image),
            tags.join(",\n")
        );
        std::fs::write(path.with_extension("json"), text).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Open a PNG/JPEG/BMP as a one-frame sprite, or a GIF as an animation.
pub fn import_image(path: &Path) -> Result<Sprite, String> {
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if ext == "gif" {
        let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
        let dec = image::codecs::gif::GifDecoder::new(std::io::BufReader::new(file)).map_err(|e| e.to_string())?;
        let frames = dec.into_frames().collect_frames().map_err(|e| e.to_string())?;
        let first = frames.first().ok_or("GIF has no frames")?;
        let (w, h) = first.buffer().dimensions();
        let mut s = Sprite::new(w as i32, h as i32, None);
        s.durations.clear();
        s.layers[0].cels.clear();
        for fr in frames {
            let (num, den) = fr.delay().numer_denom_ms();
            s.durations.push(((num / den.max(1)).clamp(10, 65535)) as u16);
            let buf = fr.into_buffer();
            let img = Image {
                w: buf.width() as i32,
                h: buf.height() as i32,
                data: buf.into_raw(),
            };
            s.layers[0].cels.push(Some(Cel::new(img)));
        }
        Ok(s)
    } else {
        let buf = image::open(path).map_err(|e| e.to_string())?.to_rgba8();
        let (w, h) = buf.dimensions();
        if w > 65535 || h > 65535 {
            return Err("image is too large".into());
        }
        let mut s = Sprite::new(w as i32, h as i32, None);
        s.layers[0].cels[0] = Some(Cel::new(Image {
            w: w as i32,
            h: h as i32,
            data: buf.into_raw(),
        }));
        Ok(s)
    }
}

/// Unique colors of a sprite (up to `max`), most used first.
pub fn palette_from_sprite(s: &Sprite, max: usize) -> Vec<Rgba> {
    let mut counts: std::collections::HashMap<Rgba, usize> = std::collections::HashMap::new();
    for l in &s.layers {
        for c in l.cels.iter().flatten() {
            for px in c.image.data.chunks_exact(4) {
                if px[3] != 0 {
                    *counts.entry([px[0], px[1], px[2], px[3]]).or_default() += 1;
                }
            }
        }
    }
    let mut v: Vec<(Rgba, usize)> = counts.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    v.truncate(max);
    v.into_iter().map(|(c, _)| c).collect()
}
