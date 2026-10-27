//! Reference software renderer.
//!
//! The picture is black, with four sakura-blossom finders in the canvas corners,
//! three dotted rings, and inside them a field of petals on concentric bands, each
//! lit petal turned about its centre by its turn. A lit petal whose centre lies
//! inside the `天` is drawn in the light colour, the others in sakura pink, over a
//! faint backdrop of the `天` itself, so the character shows through the storm of
//! petals. Above and below that square, the card's two bands show rows of
//! katakana tiles (see [`crate::kana`]): a light tile filled light with its glyph
//! in ink, a dark tile black with its glyph in pink. Platform SDKs may draw with
//! their own 2D APIs as long as geometry, turns, glyphs and polarity match this
//! renderer.

use crate::image::Rgb;
use crate::lanes::FrameCells;
use crate::layout::{
    BACKDROP_RADIUS, BAND_COUNT, BAND_INNER, BAND_OFFSETS, BAND_SLOTS, BAND_SPACING, CANVAS,
    CARD_HEIGHT, CENTER, DOT_RADIUS, FINDER_CENTERS, FINDER_OUTER, PETAL_HALF_LENGTH, RING_COUNT,
    RING_RADII, RING_SLOTS, SQUARE_TOP, band_phase, band_radius, cell_angle, cell_center,
    cell_light, finder_lit, in_mask, kana_inked, kana_inside, kana_tile_at, petal_lit, ring_offset,
};
use crate::turns::turn_angle;

/// Colours of the picture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    /// Background.
    pub background: [u8; 3],
    /// Finders and the petals inside the `天`.
    pub light: [u8; 3],
    /// Sakura pink of the ring dots and of the petals outside the `天`.
    pub pink: [u8; 3],
    /// The faint `天` drawn under the petals (a tenth of the light colour).
    pub backdrop: [u8; 3],
    /// Glyphs on light katakana tiles.
    pub ink: [u8; 3],
}

impl Default for Palette {
    fn default() -> Self {
        Self {
            background: [0, 0, 0],
            light: [250, 235, 244],
            pink: [245, 175, 208],
            backdrop: [25, 24, 24],
            ink: [20, 4, 14],
        }
    }
}

/// Rendering options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderOptions {
    /// Output width in pixels; the card is [`card_height_px`] pixels tall.
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

struct Shader<'a> {
    cells: &'a FrameCells,
    palette: Palette,
}

impl Shader<'_> {
    fn petal(&self, x: f64, y: f64, radius: f64, theta: f64) -> Option<[u8; 3]> {
        let reach = f64::from(PETAL_HALF_LENGTH);
        let nearest = ((radius - f64::from(BAND_INNER)) / f64::from(BAND_SPACING)).round();
        for band in [nearest - 1.0, nearest, nearest + 1.0] {
            if band < 0.0 || band >= BAND_COUNT as f64 {
                continue;
            }
            let band = band as usize;
            if (radius - f64::from(band_radius(band))).abs() > reach {
                continue;
            }
            let slots = f64::from(BAND_SLOTS[band]);
            let around = (theta / core::f64::consts::TAU * slots - band_phase(band)).round() as i64;
            for slot in [around - 1, around, around + 1] {
                let cell = BAND_OFFSETS[band] + slot.rem_euclid(slots as i64) as usize;
                if !self.cells.petals[cell] {
                    continue;
                }
                let (cx, cy) = cell_center(cell);
                let angle = cell_angle(cell) + turn_angle(self.cells.turns[cell]);
                if petal_lit(x - cx, y - cy, angle) {
                    return Some(if cell_light(cell) {
                        self.palette.light
                    } else {
                        self.palette.pink
                    });
                }
            }
        }
        None
    }

    fn shade(&self, x: f64, y: f64) -> [u8; 3] {
        let p = &self.palette;
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
        let (dx, dy) = (x - f64::from(CENTER), y - f64::from(CENTER));
        let radius = (dx * dx + dy * dy).sqrt();
        let mut theta = dy.atan2(dx);
        if theta < 0.0 {
            theta += core::f64::consts::TAU;
        }
        // ring dots
        for ring in 0..RING_COUNT {
            let ring_radius = f64::from(RING_RADII[ring]);
            if (radius - ring_radius).abs() > f64::from(DOT_RADIUS) {
                continue;
            }
            let slots = RING_SLOTS[ring];
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
        // petals, then the faint 天 under them
        if radius <= f64::from(BACKDROP_RADIUS) {
            if let Some(colour) = self.petal(x, y, radius, theta) {
                return colour;
            }
            if in_mask(x, y) {
                return p.backdrop;
            }
        }
        p.background
    }

    /// The colour of a katakana tile at the square-coordinate point `(x, y)` outside the square,
    /// or the background between tiles.
    fn kana(&self, x: f64, y: f64) -> [u8; 3] {
        let p = &self.palette;
        let Some((tile, u, v)) = kana_tile_at(x, y) else {
            return p.background;
        };
        if !kana_inside(u, v) {
            return p.background;
        }
        let symbol = self.cells.kana[tile];
        match (kana_inked(symbol.glyph, u, v), symbol.light) {
            (true, true) => p.ink,
            (false, true) => p.light,
            (true, false) => p.pink,
            (false, false) => p.background,
        }
    }
}

/// Height in pixels of a card rendered `size` pixels wide: `size · 1668 / 1024`, rounded.
#[must_use]
pub fn card_height_px(size: usize) -> usize {
    let height = size as f64 * f64::from(CARD_HEIGHT) / f64::from(CANVAS);
    (height + 0.5).floor() as usize
}

/// Renders one frame: the card, `options.size` pixels wide and [`card_height_px`] tall, whose
/// square starts [`SQUARE_TOP`] units below the top edge.
///
/// # Panics
/// Panics when `options.size` is zero or `options.supersample` is outside 1–4.
#[must_use]
pub fn render(cells: &FrameCells, options: &RenderOptions) -> Rgb {
    render_rows(
        cells,
        options,
        card_height_px(options.size),
        f64::from(SQUARE_TOP),
    )
}

/// Renders only the square of the card, `options.size` pixels on a side: the decoder's own unit
/// tests work on bare squares.
#[cfg(test)]
pub(crate) fn render_square(cells: &FrameCells, options: &RenderOptions) -> Rgb {
    render_rows(cells, options, options.size, 0.0)
}

/// Renders `height` pixel rows of the card, the first `top` canvas units above the square's top
/// edge.
fn render_rows(cells: &FrameCells, options: &RenderOptions, height: usize, top: f64) -> Rgb {
    assert!(options.size > 0 && (1..=4).contains(&options.supersample));
    let shader = Shader {
        cells,
        palette: options.palette,
    };
    let size = options.size;
    let s = options.supersample;
    let unit = f64::from(CANVAS) / size as f64;
    let mut data = vec![0u8; size * height * 3];
    for py in 0..height {
        for px in 0..size {
            let mut acc = [0u32; 3];
            for sy in 0..s {
                for sx in 0..s {
                    let x = (px as f64 + (sx as f64 + 0.5) / s as f64) * unit;
                    let y = (py as f64 + (sy as f64 + 0.5) / s as f64) * unit - top;
                    let c = if (0.0..f64::from(CANVAS)).contains(&y) {
                        shader.shade(x, y)
                    } else {
                        shader.kana(x, y)
                    };
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
        height,
        data,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lanes::{D_DATA, FIELD_DATA, Lane, encode_lane};
    use crate::layout::{FIELD_CELLS, TOTAL_SLOTS};

    fn cells(seed: u8) -> FrameCells {
        let d: Vec<u8> = (0..D_DATA as u8)
            .map(|b| b.wrapping_mul(13) ^ seed)
            .collect();
        let fields: Vec<Vec<u8>> = (0..4u8)
            .map(|s| {
                let data: Vec<u8> = (0..FIELD_DATA as u8)
                    .map(|b| b.wrapping_mul(17).wrapping_add(s) ^ seed)
                    .collect();
                encode_lane(Lane::Field(s), &data)
            })
            .collect();
        FrameCells::from_words(
            &encode_lane(Lane::D, &d),
            [&fields[0], &fields[1], &fields[2], &fields[3]],
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
        let scale = 256.0 / 1024.0;
        let top = (f64::from(SQUARE_TOP) * scale) as usize;
        let at = |x: usize, y: usize| image.data[((y + top) * 256 + x) * 3];
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
    fn lit_petals_are_drawn_in_their_colour_and_dark_cells_show_the_backdrop() {
        let frame = cells(2);
        let image = render(
            &frame,
            &RenderOptions {
                size: 1024,
                supersample: 1,
                palette: Palette::default(),
            },
        );
        let palette = Palette::default();
        for cell in 0..FIELD_CELLS {
            let (x, y) = cell_center(cell);
            let at = ((y as usize + SQUARE_TOP as usize) * 1024 + x as usize) * 3;
            let pixel = [image.data[at], image.data[at + 1], image.data[at + 2]];
            let expected = match (frame.petals[cell], cell_light(cell)) {
                (true, true) => palette.light,
                (true, false) => palette.pink,
                (false, _) if in_mask(x.floor() + 0.5, y.floor() + 0.5) => palette.backdrop,
                (false, _) => palette.background,
            };
            assert_eq!(pixel, expected, "cell {cell}");
        }
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
                let value =
                    image.data[((y as usize + SQUARE_TOP as usize) * 1024 + x as usize) * 3];
                if frame.dots[ring_offset(ring) + slot] {
                    assert!(value > 150, "ring {ring} slot {slot} should be lit");
                } else {
                    assert_eq!(value, 0, "ring {ring} slot {slot} should be dark");
                }
                checked += 1;
            }
        }
        assert_eq!(checked, TOTAL_SLOTS);
    }

    #[test]
    fn katakana_tiles_show_their_polarity_and_glyph() {
        let mut frame = cells(4);
        for (i, symbol) in frame.kana.iter_mut().enumerate() {
            symbol.glyph = (i % 16) as u8;
            symbol.light = i % 3 != 0;
        }
        let image = render(
            &frame,
            &RenderOptions {
                size: 1024,
                supersample: 1,
                palette: Palette::default(),
            },
        );
        assert_eq!(image.height, 1668);
        let palette = Palette::default();
        for (i, tile) in crate::layout::kana_tiles().iter().enumerate() {
            let symbol = frame.kana[i];
            // the pixel centres of the tile's square, against the definition
            let (x0, y0) = (
                tile.x.ceil() as usize,
                (tile.y + f64::from(SQUARE_TOP)).ceil() as usize,
            );
            for (dx, dy) in [(3, 3), (12, 12), (8, 17), (20, 6)] {
                let (px, py) = (x0 + dx, y0 + dy);
                let (u, v) = (
                    px as f64 + 0.5 - tile.x,
                    py as f64 + 0.5 - f64::from(SQUARE_TOP) - tile.y,
                );
                let expected = match (kana_inked(symbol.glyph, u, v), symbol.light) {
                    (true, true) => palette.ink,
                    (false, true) => palette.light,
                    (true, false) => palette.pink,
                    (false, false) => palette.background,
                };
                let at = (py * 1024 + px) * 3;
                assert_eq!(
                    [image.data[at], image.data[at + 1], image.data[at + 2]],
                    expected,
                    "tile {i} at ({dx}, {dy})"
                );
            }
        }
    }
}
