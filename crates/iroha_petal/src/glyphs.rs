//! The sixteen katakana of lane `K`.
//!
//! A tile's glyph is a four-bit symbol. The alphabet is the subset of the
//! Iroha ordering `イロハニホヘト…ス` whose members stay most distinguishable
//! after camera blur: イ ロ ハ ニ ヘ ト ワ カ レ ム ノ ケ ア ヒ ス ン.
//! Symbol `0` is `イ` and symbol `15` is `ン`.
//!
//! Glyphs are defined as stroke polylines on a 32×32 grid (`y` grows
//! downward) drawn with round caps and joins. The strokes are the rendering
//! definition; [`TEMPLATES`] is the derived, byte-exact matching table that
//! every decoder embeds, so classification never depends on a rasteriser.

/// Number of glyphs (symbols) in the alphabet.
pub const GLYPH_COUNT: usize = 16;
/// Side of the glyph design grid.
pub const GLYPH_GRID: f32 = 32.0;
/// Stroke width on the design grid.
pub const STROKE_WIDTH: f32 = 6.5;
/// Side of the matching template grid.
pub const TEMPLATE_N: usize = 8;

/// The sixteen characters in symbol order.
pub const GLYPH_CHARS: [char; GLYPH_COUNT] = [
    'イ', 'ロ', 'ハ', 'ニ', 'ヘ', 'ト', 'ワ', 'カ', 'レ', 'ム', 'ノ', 'ケ', 'ア', 'ヒ', 'ス', 'ン',
];

/// One stroke: a polyline of `(x, y)` points on the design grid.
pub type Stroke = &'static [(f32, f32)];

/// Stroke definitions for every glyph, in symbol order.
pub static STROKES: [&[Stroke]; GLYPH_COUNT] = [
    // イ
    &[&[(22.0, 4.0), (8.0, 18.0)], &[(19.0, 10.0), (19.0, 29.0)]],
    // ロ
    &[&[
        (6.0, 6.0),
        (26.0, 6.0),
        (26.0, 26.0),
        (6.0, 26.0),
        (6.0, 6.0),
    ]],
    // ハ
    &[&[(14.0, 6.0), (6.0, 27.0)], &[(18.0, 10.0), (27.0, 27.0)]],
    // ニ
    &[&[(8.0, 10.0), (24.0, 10.0)], &[(4.0, 24.0), (28.0, 24.0)]],
    // ヘ
    &[&[(3.0, 19.0), (12.0, 10.0), (29.0, 24.0)]],
    // ト
    &[&[(11.0, 3.0), (11.0, 29.0)], &[(11.0, 13.0), (26.0, 21.0)]],
    // ワ
    &[&[
        (7.0, 21.0),
        (7.0, 8.0),
        (25.0, 8.0),
        (23.0, 19.0),
        (10.0, 29.0),
    ]],
    // カ
    &[
        &[(14.0, 3.0), (14.0, 16.0), (8.0, 28.0)],
        &[(4.0, 12.0), (25.0, 12.0), (25.0, 23.0), (21.0, 28.0)],
    ],
    // レ
    &[&[(9.0, 4.0), (9.0, 27.0), (27.0, 8.0)]],
    // ム
    &[
        &[(17.0, 4.0), (7.0, 23.0), (27.0, 24.0)],
        &[(21.0, 16.0), (26.0, 22.0)],
    ],
    // ノ
    &[&[(22.0, 4.0), (17.0, 16.0), (9.0, 28.0)]],
    // ケ
    &[
        &[(13.0, 3.0), (6.0, 13.0)],
        &[(10.0, 12.0), (27.0, 12.0)],
        &[(19.0, 12.0), (16.0, 22.0), (9.0, 29.0)],
    ],
    // ア
    &[
        &[(5.0, 10.0), (26.0, 10.0), (25.0, 18.0), (19.0, 24.0)],
        &[(15.0, 10.0), (15.0, 21.0), (8.0, 29.0)],
    ],
    // ヒ
    &[
        &[(10.0, 5.0), (10.0, 26.0), (26.0, 26.0)],
        &[(10.0, 14.0), (25.0, 14.0)],
    ],
    // ス
    &[
        &[(6.0, 5.0), (24.0, 5.0), (17.0, 15.0), (6.0, 27.0)],
        &[(13.0, 15.0), (28.0, 28.0)],
    ],
    // ン
    &[
        &[(6.0, 7.0), (12.0, 13.0)],
        &[(6.0, 25.0), (14.0, 27.0), (27.0, 9.0)],
    ],
];

/// Distance from point `(px, py)` to the segment `a`–`b`.
fn segment_distance(px: f64, py: f64, a: (f32, f32), b: (f32, f32)) -> f64 {
    let (ax, ay, bx, by) = (
        f64::from(a.0),
        f64::from(a.1),
        f64::from(b.0),
        f64::from(b.1),
    );
    let (dx, dy) = (bx - ax, by - ay);
    let length_sq = dx * dx + dy * dy;
    let t = if length_sq == 0.0 {
        0.0
    } else {
        (((px - ax) * dx + (py - ay) * dy) / length_sq).clamp(0.0, 1.0)
    };
    ((px - (ax + t * dx)).powi(2) + (py - (ay + t * dy)).powi(2)).sqrt()
}

/// Returns whether design-grid point `(x, y)` is inked in `glyph`.
#[must_use]
pub fn is_inked(glyph: usize, x: f64, y: f64) -> bool {
    let radius = f64::from(STROKE_WIDTH) / 2.0;
    STROKES[glyph].iter().any(|stroke| {
        stroke
            .windows(2)
            .any(|pair| segment_distance(x, y, pair[0], pair[1]) <= radius)
    })
}

/// Derives the matching templates from the stroke definitions.
///
/// Each of the `8 × 8` cells holds the inked fraction of its `4 × 4` design
/// cells, sampled on a 16×16 grid and scaled to `0..=255`.
#[must_use]
pub fn generate_templates() -> [[u8; TEMPLATE_N * TEMPLATE_N]; GLYPH_COUNT] {
    const SUPER: usize = 16;
    let cell = f64::from(GLYPH_GRID) / TEMPLATE_N as f64;
    let mut out = [[0u8; TEMPLATE_N * TEMPLATE_N]; GLYPH_COUNT];
    for (glyph, table) in out.iter_mut().enumerate() {
        for v in 0..TEMPLATE_N {
            for u in 0..TEMPLATE_N {
                let mut inked = 0u32;
                for sy in 0..SUPER {
                    for sx in 0..SUPER {
                        let x = (u as f64 + (sx as f64 + 0.5) / SUPER as f64) * cell;
                        let y = (v as f64 + (sy as f64 + 0.5) / SUPER as f64) * cell;
                        if is_inked(glyph, x, y) {
                            inked += 1;
                        }
                    }
                }
                let coverage = f64::from(inked) / (SUPER * SUPER) as f64;
                table[v * TEMPLATE_N + u] = (coverage * 255.0 + 0.5).floor() as u8;
            }
        }
    }
    out
}

include!("glyph_templates.rs");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_in_templates_match_the_stroke_definitions() {
        assert_eq!(generate_templates(), TEMPLATES);
    }

    #[test]
    fn every_glyph_has_ink_inside_the_design_grid() {
        for (glyph, strokes) in STROKES.iter().enumerate() {
            let total: u32 = TEMPLATES[glyph].iter().map(|&c| u32::from(c)).sum();
            assert!(total > 255 * 6, "glyph {glyph} has too little ink");
            for stroke in *strokes {
                for &(x, y) in *stroke {
                    assert!((2.0..=30.0).contains(&x) && (2.0..=30.0).contains(&y));
                }
            }
        }
    }

    #[test]
    fn glyphs_are_pairwise_distinct_under_blur() {
        // Zero-mean cosine distance of the raw templates must stay well apart.
        let feature = |glyph: usize| -> Vec<f64> {
            let values: Vec<f64> = TEMPLATES[glyph].iter().map(|&c| f64::from(c)).collect();
            let mean = values.iter().sum::<f64>() / values.len() as f64;
            let centered: Vec<f64> = values.iter().map(|v| v - mean).collect();
            let norm = centered.iter().map(|v| v * v).sum::<f64>().sqrt();
            centered.iter().map(|v| v / norm).collect()
        };
        for a in 0..GLYPH_COUNT {
            for b in (a + 1)..GLYPH_COUNT {
                let dot: f64 = feature(a).iter().zip(feature(b)).map(|(x, y)| x * y).sum();
                assert!(
                    1.0 - dot > 0.2,
                    "glyphs {a} and {b} are too similar: {}",
                    1.0 - dot
                );
            }
        }
    }
}
