//! Reference software renderer.
//!
//! The picture is black, with four sakura-blossom finders in the canvas corners,
//! a `天`-shaped field of 256 tiles inside three dotted rings. A light tile is
//! a pale rounded square with a near-black katakana; a dark tile is empty
//! except for its sakura-pink katakana. Platform SDKs may draw with their own
//! 2D APIs as long as geometry and polarity match this renderer.

use std::sync::OnceLock;

use crate::glyphs::{GLYPH_COUNT, GLYPH_GRID, is_inked};
use crate::image::Rgb;
use crate::lanes::FrameCells;
use crate::layout::{
    CANVAS, CENTER, DOT_RADIUS, FINDER_CENTERS, FINDER_OUTER, GLYPH_BOX, RING_COUNT, RING_RADII,
    RING_SLOTS, TILE_GRID, TILE_ORIGIN, TILE_PITCH, TILE_SIZE, TILES, finder_lit, ring_offset,
};

/// Colours of the picture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    /// Background.
    pub background: [u8; 3],
    /// Light tile fill and finders.
    pub light: [u8; 3],
    /// Sakura pink of dots and of glyphs on dark tiles.
    pub pink: [u8; 3],
    /// Glyph colour on a light tile.
    pub ink: [u8; 3],
}

impl Default for Palette {
    fn default() -> Self {
        Self {
            background: [0, 0, 0],
            light: [250, 235, 244],
            pink: [245, 175, 208],
            ink: [20, 4, 14],
        }
    }
}

/// Rendering options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderOptions {
    /// Output side in pixels.
    pub size: usize,
    /// Samples per pixel side for anti-aliasing (1–4).
    pub supersample: usize,
    /// Colours.
    pub palette: Palette,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            size: 1024,
            supersample: 3,
            palette: Palette::default(),
        }
    }
}

const BITMAP_N: usize = 128;

fn glyph_bitmaps() -> &'static Vec<Vec<bool>> {
    static BITMAPS: OnceLock<Vec<Vec<bool>>> = OnceLock::new();
    BITMAPS.get_or_init(|| {
        (0..GLYPH_COUNT)
            .map(|glyph| {
                let mut bitmap = vec![false; BITMAP_N * BITMAP_N];
                for v in 0..BITMAP_N {
                    for u in 0..BITMAP_N {
                        let x = (u as f64 + 0.5) / BITMAP_N as f64 * f64::from(GLYPH_GRID);
                        let y = (v as f64 + 0.5) / BITMAP_N as f64 * f64::from(GLYPH_GRID);
                        bitmap[v * BITMAP_N + u] = is_inked(glyph, x, y);
                    }
                }
                bitmap
            })
            .collect()
    })
}

fn tile_lookup() -> &'static [[Option<u16>; TILE_GRID]; TILE_GRID] {
    static LOOKUP: OnceLock<[[Option<u16>; TILE_GRID]; TILE_GRID]> = OnceLock::new();
    LOOKUP.get_or_init(|| {
        let mut table = [[None; TILE_GRID]; TILE_GRID];
        for (index, &(col, row)) in TILES.iter().enumerate() {
            table[usize::from(row)][usize::from(col)] = Some(index as u16);
        }
        table
    })
}

/// Inside test for a rounded square of half-side `half` and corner radius
/// `radius`, relative to its centre.
fn in_rounded_square(dx: f64, dy: f64, half: f64, radius: f64) -> bool {
    let (ax, ay) = (dx.abs(), dy.abs());
    if ax > half || ay > half {
        return false;
    }
    let (cx, cy) = (ax - (half - radius), ay - (half - radius));
    cx <= 0.0 || cy <= 0.0 || cx * cx + cy * cy <= radius * radius
}

struct Shader<'a> {
    cells: &'a FrameCells,
    palette: Palette,
}

impl Shader<'_> {
    fn shade(&self, x: f64, y: f64) -> [u8; 3] {
        let p = &self.palette;
        // finders
        for &(fx, fy) in &FINDER_CENTERS {
            let (dx, dy) = (x - f64::from(fx), y - f64::from(fy));
            if (dx * dx + dy * dy).sqrt() <= f64::from(FINDER_OUTER) {
                return if finder_lit(dx, dy) {
                    p.light
                } else {
                    p.background
                };
            }
        }
        // tiles
        let lattice = f64::from(TILE_ORIGIN);
        let pitch = f64::from(TILE_PITCH);
        if x >= lattice && y >= lattice {
            let col = ((x - lattice) / pitch) as usize;
            let row = ((y - lattice) / pitch) as usize;
            if col < TILE_GRID
                && row < TILE_GRID
                && let Some(tile) = tile_lookup()[row][col]
            {
                let tile = usize::from(tile);
                let cx = lattice + pitch * (col as f64 + 0.5);
                let cy = lattice + pitch * (row as f64 + 0.5);
                let (dx, dy) = (x - cx, y - cy);
                if !in_rounded_square(dx, dy, f64::from(TILE_SIZE) / 2.0, 3.0) {
                    return p.background;
                }
                let box_half = f64::from(GLYPH_BOX) / 2.0;
                let inked = dx.abs() < box_half && dy.abs() < box_half && {
                    let u = ((dx + box_half) / (2.0 * box_half) * BITMAP_N as f64) as usize;
                    let v = ((dy + box_half) / (2.0 * box_half) * BITMAP_N as f64) as usize;
                    glyph_bitmaps()[usize::from(self.cells.glyph[tile])]
                        [v.min(BITMAP_N - 1) * BITMAP_N + u.min(BITMAP_N - 1)]
                };
                return match (self.cells.light[tile], inked) {
                    (true, false) => p.light,
                    (true, true) => p.ink,
                    (false, true) => p.pink,
                    (false, false) => p.background,
                };
            }
        }
        // ring dots
        let (dx, dy) = (x - f64::from(CENTER), y - f64::from(CENTER));
        let radius = (dx * dx + dy * dy).sqrt();
        for ring in 0..RING_COUNT {
            let ring_radius = f64::from(RING_RADII[ring]);
            if (radius - ring_radius).abs() > f64::from(DOT_RADIUS) {
                continue;
            }
            let slots = RING_SLOTS[ring];
            let mut theta = dy.atan2(dx);
            if theta < 0.0 {
                theta += core::f64::consts::TAU;
            }
            let slot = ((theta / core::f64::consts::TAU * slots as f64).round() as usize) % slots;
            if !self.cells.dots[ring_offset(ring) + slot] {
                continue;
            }
            let angle = core::f64::consts::TAU * slot as f64 / slots as f64;
            let (px, py) = (ring_radius * angle.cos(), ring_radius * angle.sin());
            if ((dx - px).powi(2) + (dy - py).powi(2)).sqrt() <= f64::from(DOT_RADIUS) {
                return p.pink;
            }
        }
        p.background
    }
}

/// Renders one frame.
///
/// # Panics
/// Panics when `options.size` is zero or `options.supersample` is outside 1–4.
#[must_use]
pub fn render(cells: &FrameCells, options: &RenderOptions) -> Rgb {
    assert!(options.size > 0 && (1..=4).contains(&options.supersample));
    let shader = Shader {
        cells,
        palette: options.palette,
    };
    let size = options.size;
    let s = options.supersample;
    let unit = f64::from(CANVAS) / size as f64;
    let mut data = vec![0u8; size * size * 3];
    for py in 0..size {
        for px in 0..size {
            let mut acc = [0u32; 3];
            for sy in 0..s {
                for sx in 0..s {
                    let x = (px as f64 + (sx as f64 + 0.5) / s as f64) * unit;
                    let y = (py as f64 + (sy as f64 + 0.5) / s as f64) * unit;
                    let c = shader.shade(x, y);
                    for k in 0..3 {
                        acc[k] += u32::from(c[k]);
                    }
                }
            }
            let n = (s * s) as u32;
            let at = (py * size + px) * 3;
            for k in 0..3 {
                data[at + k] = ((acc[k] + n / 2) / n) as u8;
            }
        }
    }
    Rgb {
        width: size,
        height: size,
        data,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lanes::{D_DATA, K_DATA, Lane, P_DATA, encode_lane};

    fn cells(seed: u8) -> FrameCells {
        let p: Vec<u8> = (0..P_DATA as u8)
            .map(|b| b.wrapping_mul(31).wrapping_add(seed))
            .collect();
        let k: Vec<u8> = (0..K_DATA as u8)
            .map(|b| b.wrapping_mul(17) ^ seed)
            .collect();
        let d: Vec<u8> = (0..D_DATA as u8)
            .map(|b| b.wrapping_mul(13) ^ seed)
            .collect();
        FrameCells::from_words(
            &encode_lane(Lane::P, &p),
            &encode_lane(Lane::K, &k),
            &encode_lane(Lane::D, &d),
        )
    }

    #[test]
    fn finders_are_solid_blossoms_and_corners_are_otherwise_black() {
        let image = render(
            &cells(1),
            &RenderOptions {
                size: 256,
                supersample: 2,
                palette: Palette::default(),
            },
        );
        let at = |x: usize, y: usize| image.data[(y * 256 + x) * 3];
        let scale = 256.0 / 1024.0;
        let (fx, fy) = (
            f64::from(FINDER_CENTERS[0].0) * scale,
            f64::from(FINDER_CENTERS[0].1) * scale,
        );
        assert!(at(fx as usize, fy as usize) > 200, "core must be lit");
        assert!(
            at(fx as usize, (fy - 34.0 * scale) as usize) > 200,
            "upper petal must be lit"
        );
        assert!(
            at((fx + 20.0 * scale) as usize, (fy + 20.0 * scale) as usize) > 200,
            "blossom body must be lit"
        );
        assert_eq!(at(2, 255), 0);
    }

    #[test]
    fn light_tiles_are_bright_and_dark_tiles_are_mostly_black() {
        let frame = cells(2);
        let image = render(
            &frame,
            &RenderOptions {
                size: 512,
                supersample: 2,
                palette: Palette::default(),
            },
        );
        let scale = 512.0 / 1024.0;
        let mut light_means = Vec::new();
        let mut dark_means = Vec::new();
        for tile in 0..crate::layout::TILE_COUNT {
            let (cx, cy) = crate::layout::tile_center(tile);
            let (x0, y0) = (
                (f64::from(cx) - 10.0) * scale,
                (f64::from(cy) - 10.0) * scale,
            );
            let mut sum = 0u32;
            for j in 0..10 {
                for i in 0..10 {
                    sum += u32::from(image.data[((y0 as usize + j) * 512 + x0 as usize + i) * 3]);
                }
            }
            let mean = f64::from(sum) / 100.0;
            if frame.light[tile] {
                light_means.push(mean);
            } else {
                dark_means.push(mean);
            }
        }
        let light = light_means.iter().sum::<f64>() / light_means.len() as f64;
        let dark = dark_means.iter().sum::<f64>() / dark_means.len() as f64;
        // bold glyphs ink the middle of every tile, so only the ordering is stable
        assert!(
            light > dark + 25.0,
            "light tiles average {light}, dark tiles {dark}"
        );
    }

    #[test]
    fn lit_dots_are_drawn_and_unlit_slots_are_black() {
        let frame = cells(3);
        let image = render(
            &frame,
            &RenderOptions {
                size: 1024,
                supersample: 1,
                palette: Palette::default(),
            },
        );
        let mut checked = 0;
        for (ring, &slots) in RING_SLOTS.iter().enumerate() {
            for slot in 0..slots {
                let (x, y) = crate::layout::slot_center(ring, slot);
                let value = image.data[((y as usize) * 1024 + x as usize) * 3];
                if frame.dots[ring_offset(ring) + slot] {
                    assert!(value > 150, "ring {ring} slot {slot} should be lit");
                } else {
                    assert_eq!(value, 0, "ring {ring} slot {slot} should be dark");
                }
                checked += 1;
            }
        }
        assert_eq!(checked, crate::layout::TOTAL_SLOTS);
    }
}
