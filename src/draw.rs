//! Pixel drawing algorithms: lines, ellipses, flood fill, polygon fill, brushes.

use crate::doc::{Mask, Rgba};

pub const BAYER4: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];

pub fn line(x0: i32, y0: i32, x1: i32, y1: i32, mut f: impl FnMut(i32, i32)) {
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let (mut x, mut y, mut err) = (x0, y0, dx + dy);
    loop {
        f(x, y);
        if x == x1 && y == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

pub fn rect(x0: i32, y0: i32, x1: i32, y1: i32, filled: bool, mut f: impl FnMut(i32, i32)) {
    let (xa, xb) = (x0.min(x1), x0.max(x1));
    let (ya, yb) = (y0.min(y1), y0.max(y1));
    for y in ya..=yb {
        for x in xa..=xb {
            if filled || x == xa || x == xb || y == ya || y == yb {
                f(x, y);
            }
        }
    }
}

/// Ellipse inscribed in the rectangle (x0,y0)-(x1,y1), after Alois Zingl.
pub fn ellipse(x0: i32, y0: i32, x1: i32, y1: i32, filled: bool, mut f: impl FnMut(i32, i32)) {
    let (mut x0, mut x1) = (x0.min(x1) as i64, x0.max(x1) as i64);
    let (ya, yb) = (y0.min(y1) as i64, y0.max(y1) as i64);
    let mut a = x1 - x0;
    let b = yb - ya;
    let mut b1 = b & 1;
    let mut dx = 4 * (1 - a) * b * b;
    let mut dy = 4 * (b1 + 1) * a * a;
    let mut err = dx + dy + b1 * a * a;
    let mut y0 = ya + (b + 1) / 2;
    let mut y1 = y0 - b1;
    a = 8 * a * a;
    b1 = 8 * b * b;
    let mut span = |xa: i64, xb: i64, y: i64| {
        if filled {
            for x in xa..=xb {
                f(x as i32, y as i32);
            }
        } else {
            f(xa as i32, y as i32);
            if xb != xa {
                f(xb as i32, y as i32);
            }
        }
    };
    loop {
        span(x0, x1, y0);
        if y1 != y0 {
            span(x0, x1, y1);
        }
        let e2 = 2 * err;
        if e2 <= dy {
            y0 += 1;
            y1 -= 1;
            dy += a;
            err += dy;
        }
        if e2 >= dx || 2 * err > dy {
            x0 += 1;
            x1 -= 1;
            dx += b1;
            err += dx;
        }
        if x0 > x1 {
            break;
        }
    }
    while y0 - y1 < b {
        span(x0 - 1, x1 + 1, y0);
        y0 += 1;
        span(x0 - 1, x1 + 1, y1);
        y1 -= 1;
    }
}

/// Pixel offsets covered by a brush of the given diameter.
pub fn brush_offsets(size: i32, square: bool) -> Vec<(i32, i32)> {
    let size = size.max(1);
    let lo = -(size / 2);
    let c = (size - 1) as f32 / 2.0;
    let r2 = (size as f32 / 2.0 - 0.25).powi(2);
    let mut out = Vec::new();
    for j in 0..size {
        for i in 0..size {
            let d = (i as f32 - c).powi(2) + (j as f32 - c).powi(2);
            if square || size <= 2 || d <= r2 {
                out.push((lo + i, lo + j));
            }
        }
    }
    out
}

pub fn color_close(a: Rgba, b: Rgba, tol: u8) -> bool {
    if a[3] == 0 && b[3] == 0 {
        return true;
    }
    (0..4).all(|i| a[i].abs_diff(b[i]) <= tol)
}

/// Mask of pixels matching the color at (sx, sy), either connected to it or anywhere.
pub fn flood(w: i32, h: i32, get: impl Fn(i32, i32) -> Rgba, sx: i32, sy: i32, tol: u8, contiguous: bool) -> Mask {
    let mut m = Mask::new(w, h);
    if sx < 0 || sy < 0 || sx >= w || sy >= h {
        return m;
    }
    let target = get(sx, sy);
    if !contiguous {
        for y in 0..h {
            for x in 0..w {
                if color_close(get(x, y), target, tol) {
                    m.set(x, y, true);
                }
            }
        }
        return m;
    }
    let mut stack = vec![(sx, sy)];
    m.set(sx, sy, true);
    while let Some((x, y)) = stack.pop() {
        for (nx, ny) in [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)] {
            if nx >= 0 && ny >= 0 && nx < w && ny < h && !m.get(nx, ny) && color_close(get(nx, ny), target, tol) {
                m.set(nx, ny, true);
                stack.push((nx, ny));
            }
        }
    }
    m
}

/// Even-odd fill of a closed polygon given in pixel coordinates (plus its outline).
pub fn polygon_mask(pts: &[(i32, i32)], w: i32, h: i32) -> Mask {
    let mut m = Mask::new(w, h);
    if pts.is_empty() {
        return m;
    }
    for y in 0..h {
        let yc = y as f32 + 0.5;
        let mut xs: Vec<f32> = Vec::new();
        for i in 0..pts.len() {
            let (ax, ay) = pts[i];
            let (bx, by) = pts[(i + 1) % pts.len()];
            let (ax, ay, bx, by) = (ax as f32 + 0.5, ay as f32 + 0.5, bx as f32 + 0.5, by as f32 + 0.5);
            if (ay <= yc && by > yc) || (by <= yc && ay > yc) {
                xs.push(ax + (yc - ay) / (by - ay) * (bx - ax));
            }
        }
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        for pair in xs.chunks_exact(2) {
            for x in (pair[0] - 0.5).ceil() as i32..=(pair[1] - 0.5).floor() as i32 {
                m.set(x, y, true);
            }
        }
    }
    for i in 0..pts.len() {
        let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
        line(a.0, a.1, b.0, b.1, |x, y| m.set(x, y, true));
    }
    m
}

/// Constrain the end point to multiples of 45° from the start.
pub fn snap_angle(a: (i32, i32), b: (i32, i32)) -> (i32, i32) {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let (adx, ady) = (dx.abs(), dy.abs());
    if adx > 2 * ady {
        (b.0, a.1)
    } else if ady > 2 * adx {
        (a.0, b.1)
    } else {
        let d = adx.max(ady);
        (a.0 + d * dx.signum(), a.1 + d * dy.signum())
    }
}

/// Constrain a rectangle's far corner so it is square.
pub fn snap_square(a: (i32, i32), b: (i32, i32)) -> (i32, i32) {
    let d = (b.0 - a.0).abs().max((b.1 - a.1).abs());
    (
        a.0 + d * if b.0 < a.0 { -1 } else { 1 },
        a.1 + d * if b.1 < a.1 { -1 } else { 1 },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shapes() {
        let mut n = 0;
        line(0, 0, 5, 3, |_, _| n += 1);
        assert_eq!(n, 6);
        // A filled ellipse stays inside its box and is symmetric.
        let mut m = Mask::new(9, 6);
        ellipse(0, 0, 8, 5, true, |x, y| {
            assert!((0..9).contains(&x) && (0..6).contains(&y));
            m.set(x, y, true)
        });
        for y in 0..6 {
            for x in 0..9 {
                assert_eq!(m.get(x, y), m.get(8 - x, y));
                assert_eq!(m.get(x, y), m.get(x, 5 - y));
            }
        }
        assert!(m.get(4, 0) && m.get(0, 2) && !m.get(0, 0));
        assert_eq!(brush_offsets(1, false), vec![(0, 0)]);
        assert_eq!(brush_offsets(3, false).len(), 5);
        let f = flood(4, 4, |x, _| if x < 2 { [0; 4] } else { [9, 9, 9, 255] }, 0, 0, 0, true);
        assert_eq!(f.data.iter().filter(|b| **b).count(), 8);
        let p = polygon_mask(&[(0, 0), (3, 0), (3, 3), (0, 3)], 5, 5);
        assert_eq!(p.data.iter().filter(|b| **b).count(), 16);
    }
}
