//! Plane homographies.

/// A 3×3 projective transform stored row-major.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Homography(pub [f64; 9]);

impl Homography {
    /// The identity transform.
    pub const IDENTITY: Self = Self([1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]);

    /// Maps a point.
    #[must_use]
    pub fn apply(&self, x: f64, y: f64) -> (f64, f64) {
        let m = &self.0;
        let w = m[6] * x + m[7] * y + m[8];
        (
            (m[0] * x + m[1] * y + m[2]) / w,
            (m[3] * x + m[4] * y + m[5]) / w,
        )
    }

    /// The inverse transform, or `None` when singular.
    #[must_use]
    pub fn inverse(&self) -> Option<Self> {
        let m = &self.0;
        let c00 = m[4] * m[8] - m[5] * m[7];
        let c01 = m[5] * m[6] - m[3] * m[8];
        let c02 = m[3] * m[7] - m[4] * m[6];
        let det = m[0] * c00 + m[1] * c01 + m[2] * c02;
        if det.abs() < 1e-18 {
            return None;
        }
        let inv = 1.0 / det;
        Some(Self([
            c00 * inv,
            (m[2] * m[7] - m[1] * m[8]) * inv,
            (m[1] * m[5] - m[2] * m[4]) * inv,
            c01 * inv,
            (m[0] * m[8] - m[2] * m[6]) * inv,
            (m[2] * m[3] - m[0] * m[5]) * inv,
            c02 * inv,
            (m[1] * m[6] - m[0] * m[7]) * inv,
            (m[0] * m[4] - m[1] * m[3]) * inv,
        ]))
    }

    /// Fits the homography taking `src[i]` to `dst[i]` (least squares for more
    /// than four pairs, exact for four), using Hartley-normalised DLT.
    #[must_use]
    pub fn from_points(src: &[(f64, f64)], dst: &[(f64, f64)]) -> Option<Self> {
        if src.len() != dst.len() || src.len() < 4 {
            return None;
        }
        let (ts, scale_s) = normalisation(src);
        let (td, scale_d) = normalisation(dst);
        let mut ata = [[0.0f64; 8]; 8];
        let mut atb = [0.0f64; 8];
        for (s, d) in src.iter().zip(dst) {
            let (x, y) = ts.apply_affine(s.0, s.1);
            let (u, v) = td.apply_affine(d.0, d.1);
            let rows = [
                ([x, y, 1.0, 0.0, 0.0, 0.0, -u * x, -u * y], u),
                ([0.0, 0.0, 0.0, x, y, 1.0, -v * x, -v * y], v),
            ];
            for (row, rhs) in rows {
                for i in 0..8 {
                    for j in 0..8 {
                        ata[i][j] += row[i] * row[j];
                    }
                    atb[i] += row[i] * rhs;
                }
            }
        }
        let h = solve8(ata, atb)?;
        let normalised = Self([h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7], 1.0]);
        // H = Td^-1 * Hn * Ts
        let td_inv = Self([
            1.0 / scale_d.0,
            0.0,
            -td.0[2] / scale_d.0,
            0.0,
            1.0 / scale_d.0,
            -td.0[5] / scale_d.0,
            0.0,
            0.0,
            1.0,
        ]);
        let ts_mat = Self([
            scale_s.0, 0.0, ts.0[2], 0.0, scale_s.0, ts.0[5], 0.0, 0.0, 1.0,
        ]);
        let h = td_inv.compose(&normalised).compose(&ts_mat);
        let norm = h.0[8];
        if norm.abs() < 1e-15 {
            return None;
        }
        Some(Self(h.0.map(|v| v / norm)))
    }

    /// `self * other` (apply `other` first).
    #[must_use]
    pub fn compose(&self, other: &Self) -> Self {
        let a = &self.0;
        let b = &other.0;
        let mut out = [0.0; 9];
        for r in 0..3 {
            for c in 0..3 {
                out[r * 3 + c] = (0..3).map(|k| a[r * 3 + k] * b[k * 3 + c]).sum();
            }
        }
        Self(out)
    }

    fn apply_affine(&self, x: f64, y: f64) -> (f64, f64) {
        (self.0[0] * x + self.0[2], self.0[4] * y + self.0[5])
    }
}

/// Translation to the centroid and isotropic scale to mean distance √2.
fn normalisation(points: &[(f64, f64)]) -> (Homography, (f64,)) {
    let n = points.len() as f64;
    let (cx, cy) = points
        .iter()
        .fold((0.0, 0.0), |a, p| (a.0 + p.0 / n, a.1 + p.1 / n));
    let mean = points
        .iter()
        .map(|p| ((p.0 - cx).powi(2) + (p.1 - cy).powi(2)).sqrt())
        .sum::<f64>()
        / n;
    let scale = if mean > 1e-12 {
        core::f64::consts::SQRT_2 / mean
    } else {
        1.0
    };
    (
        Homography([
            scale,
            0.0,
            -scale * cx,
            0.0,
            scale,
            -scale * cy,
            0.0,
            0.0,
            1.0,
        ]),
        (scale,),
    )
}

/// Gaussian elimination with partial pivoting for an 8×8 system.
fn solve8(mut a: [[f64; 8]; 8], mut b: [f64; 8]) -> Option<[f64; 8]> {
    for col in 0..8 {
        let pivot = (col..8).max_by(|&i, &j| a[i][col].abs().total_cmp(&a[j][col].abs()))?;
        if a[pivot][col].abs() < 1e-14 {
            return None;
        }
        a.swap(col, pivot);
        b.swap(col, pivot);
        for row in (col + 1)..8 {
            let factor = a[row][col] / a[col][col];
            let pivot_row = a[col];
            for (dst, src) in a[row][col..].iter_mut().zip(&pivot_row[col..]) {
                *dst -= factor * src;
            }
            b[row] -= factor * b[col];
        }
    }
    let mut x = [0.0; 8];
    for row in (0..8).rev() {
        let tail: f64 = ((row + 1)..8).map(|k| a[row][k] * x[k]).sum();
        x[row] = (b[row] - tail) / a[row][row];
    }
    Some(x)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_points_are_mapped_exactly() {
        let src = [(0.0, 0.0), (1024.0, 0.0), (1024.0, 1024.0), (0.0, 1024.0)];
        let dst = [(103.5, 40.25), (590.0, 70.0), (560.0, 420.0), (80.0, 380.0)];
        let h = Homography::from_points(&src, &dst).unwrap();
        for (s, d) in src.iter().zip(&dst) {
            let (x, y) = h.apply(s.0, s.1);
            assert!((x - d.0).abs() < 1e-7 && (y - d.1).abs() < 1e-7);
        }
    }

    #[test]
    fn inverse_roundtrips_and_least_squares_averages_noise() {
        let truth = Homography([0.4, -0.1, 130.0, 0.12, 0.38, 60.0, 1e-4, -2e-5, 1.0]);
        let src: Vec<(f64, f64)> = (0..30)
            .map(|i| {
                (
                    50.0 + 31.0 * f64::from(i % 6),
                    90.0 + 47.0 * f64::from(i / 6),
                )
            })
            .collect();
        let dst: Vec<(f64, f64)> = src
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let (x, y) = truth.apply(p.0, p.1);
                let jitter = if i % 2 == 0 { 0.05 } else { -0.05 };
                (x + jitter, y - jitter)
            })
            .collect();
        let fit = Homography::from_points(&src, &dst).unwrap();
        for p in &src {
            let (x0, y0) = truth.apply(p.0, p.1);
            let (x1, y1) = fit.apply(p.0, p.1);
            assert!((x0 - x1).abs() < 0.1 && (y0 - y1).abs() < 0.1);
        }
        let inverse = truth.inverse().unwrap();
        let (x, y) = truth.apply(300.0, 200.0);
        let (bx, by) = inverse.apply(x, y);
        assert!((bx - 300.0).abs() < 1e-6 && (by - 200.0).abs() < 1e-6);
    }

    #[test]
    fn degenerate_inputs_are_rejected() {
        let p = [(1.0, 1.0); 4];
        assert!(Homography::from_points(&p, &p).is_none());
        assert!(Homography::from_points(&p[..3], &p[..3]).is_none());
    }
}
