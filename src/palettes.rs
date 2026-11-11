//! Built-in palettes and palette file import/export (.gpl, .hex).

use crate::doc::Rgba;

pub struct Preset {
    pub name: &'static str,
    pub hex: &'static str,
}

impl Preset {
    pub fn colors(&self) -> Vec<Rgba> {
        parse_hex_list(self.hex)
    }
}

pub const PRESETS: &[Preset] = &[
    Preset {
        name: "DawnBringer 32",
        hex: "000000 222034 45283c 663931 8f563b df7126 d9a066 eec39a fbf236 99e550 6abe30 37946e 4b692f 524b24 323c39 3f3f74 306082 5b6ee1 639bff 5fcde4 cbdbfc ffffff 9badb7 847e87 696a6a 595652 76428a ac3232 d95763 d77bba 8f974a 8a6f30",
    },
    Preset {
        name: "DawnBringer 16",
        hex: "140c1c 442434 30346d 4e4a4e 854c30 346524 d04648 757161 597dce d27d2c 8595a1 6daa2c d2aa99 6dc2ca dad45e deeed6",
    },
    Preset {
        name: "PICO-8",
        hex: "000000 1d2b53 7e2553 008751 ab5236 5f574f c2c3c7 fff1e8 ff004d ffa300 ffec27 00e436 29adff 83769c ff77a8 ffccaa",
    },
    Preset {
        name: "Sweetie 16",
        hex: "1a1c2c 5d275d b13e53 ef7d57 ffcd75 a7f070 38b764 257179 29366f 3b5dc9 41a6f6 73eff7 f4f4f4 94b0c2 566c86 333c57",
    },
    Preset {
        name: "Endesga 32",
        hex: "be4a2f d77643 ead4aa e4a672 b86f50 733e39 3e2731 a22633 e43b44 f77622 feae34 fee761 63c74d 3e8948 265c42 193c3e 124e89 0099db 2ce8f5 ffffff c0cbdc 8b9bb4 5a6988 3a4466 262b44 181425 ff0044 68386c b55088 f6757a e8b796 c28569",
    },
    Preset {
        name: "Game Boy",
        hex: "0f380f 306230 8bac0f 9bbc0f",
    },
    Preset {
        name: "Grayscale 16",
        hex: "000000 111111 222222 333333 444444 555555 666666 777777 888888 999999 aaaaaa bbbbbb cccccc dddddd eeeeee ffffff",
    },
    Preset {
        name: "1-bit",
        hex: "000000 ffffff",
    },
];

pub fn parse_hex(s: &str) -> Option<Rgba> {
    let s = s.trim().trim_start_matches('#');
    let v = |i: usize| u8::from_str_radix(s.get(i..i + 2)?, 16).ok();
    match s.len() {
        6 => Some([v(0)?, v(2)?, v(4)?, 255]),
        8 => Some([v(0)?, v(2)?, v(4)?, v(6)?]),
        3 => {
            let d = |i: usize| u8::from_str_radix(s.get(i..i + 1)?, 16).ok().map(|x| x * 17);
            Some([d(0)?, d(1)?, d(2)?, 255])
        }
        _ => None,
    }
}

pub fn to_hex(c: Rgba) -> String {
    if c[3] == 255 {
        format!("{:02x}{:02x}{:02x}", c[0], c[1], c[2])
    } else {
        format!("{:02x}{:02x}{:02x}{:02x}", c[0], c[1], c[2], c[3])
    }
}

pub fn parse_hex_list(s: &str) -> Vec<Rgba> {
    s.split_whitespace().filter_map(parse_hex).collect()
}

/// Parse a GIMP .gpl palette or a plain list of hex colors.
pub fn parse_palette_file(text: &str) -> Vec<Rgba> {
    if text.trim_start().starts_with("GIMP Palette") {
        let mut out = Vec::new();
        for line in text.lines().skip(1) {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.contains(':') {
                continue;
            }
            let mut it = line.split_whitespace().map(|t| t.parse::<u8>());
            if let (Some(Ok(r)), Some(Ok(g)), Some(Ok(b))) = (it.next(), it.next(), it.next()) {
                out.push([r, g, b, 255]);
            }
        }
        out
    } else {
        parse_hex_list(text)
    }
}

pub fn to_gpl(name: &str, pal: &[Rgba]) -> String {
    let mut s = format!("GIMP Palette\nName: {name}\nColumns: 8\n#\n");
    for c in pal {
        s += &format!(
            "{:3} {:3} {:3}\t#{}\n",
            c[0],
            c[1],
            c[2],
            to_hex([c[0], c[1], c[2], 255])
        );
    }
    s
}

pub fn to_hex_file(pal: &[Rgba]) -> String {
    pal.iter().map(|c| to_hex([c[0], c[1], c[2], 255]) + "\n").collect()
}
