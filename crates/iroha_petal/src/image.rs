//! Minimal 8-bit pixel buffers.
//!
//! Pixel `(i, j)` covers `[i, i+1) × [j, j+1)` and its centre is at
//! `(i + 0.5, j + 0.5)`. All homographies in this crate map into these
//! pixel-edge coordinates.

/// A single-channel 8-bit image (camera luma plane).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Luma {
    /// Width in pixels.
    pub width: usize,
    /// Height in pixels.
    pub height: usize,
    /// Row-major samples, `width * height` bytes.
    pub data: Vec<u8>,
}

impl Luma {
    /// Creates a black image.
    #[must_use]
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            data: vec![0; width * height],
        }
    }

    /// Wraps an existing buffer; returns `None` on a size mismatch.
    #[must_use]
    pub fn from_raw(width: usize, height: usize, data: Vec<u8>) -> Option<Self> {
        (data.len() == width.checked_mul(height)?).then_some(Self {
            width,
            height,
            data,
        })
    }

    /// Wraps a strided plane (for example the Y plane of an NV21 camera frame).
    #[must_use]
    pub fn from_strided(width: usize, height: usize, stride: usize, plane: &[u8]) -> Option<Self> {
        if stride < width || plane.len() < stride.checked_mul(height.checked_sub(1)?)? + width {
            return None;
        }
        let mut data = Vec::with_capacity(width * height);
        for row in 0..height {
            data.extend_from_slice(&plane[row * stride..row * stride + width]);
        }
        Some(Self {
            width,
            height,
            data,
        })
    }

    /// Reads pixel `(x, y)`.
    #[must_use]
    pub fn at(&self, x: usize, y: usize) -> u8 {
        self.data[y * self.width + x]
    }

    /// Bilinear sample at continuous pixel-edge coordinates, clamped to the
    /// image border.
    #[must_use]
    pub fn sample(&self, x: f64, y: f64) -> f64 {
        let fx = (x - 0.5).clamp(0.0, (self.width - 1) as f64);
        let fy = (y - 0.5).clamp(0.0, (self.height - 1) as f64);
        let x0 = fx.floor() as usize;
        let y0 = fy.floor() as usize;
        let x1 = (x0 + 1).min(self.width - 1);
        let y1 = (y0 + 1).min(self.height - 1);
        let tx = fx - x0 as f64;
        let ty = fy - y0 as f64;
        let p = |xx: usize, yy: usize| f64::from(self.data[yy * self.width + xx]);
        let top = p(x0, y0) * (1.0 - tx) + p(x1, y0) * tx;
        let bottom = p(x0, y1) * (1.0 - tx) + p(x1, y1) * tx;
        top * (1.0 - ty) + bottom * ty
    }
}

/// An interleaved 8-bit RGB image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rgb {
    /// Width in pixels.
    pub width: usize,
    /// Height in pixels.
    pub height: usize,
    /// Row-major `r, g, b` triples.
    pub data: Vec<u8>,
}

impl Rgb {
    /// Rec. 601 luma of the image.
    #[must_use]
    pub fn to_luma(&self) -> Luma {
        let data = self
            .data
            .chunks_exact(3)
            .map(|px| {
                ((299 * u32::from(px[0]) + 587 * u32::from(px[1]) + 114 * u32::from(px[2]) + 500)
                    / 1000) as u8
            })
            .collect();
        Luma {
            width: self.width,
            height: self.height,
            data,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bilinear_sampling_interpolates_between_pixel_centres() {
        let image = Luma::from_raw(2, 1, vec![0, 100]).unwrap();
        assert!((image.sample(0.5, 0.5) - 0.0).abs() < 1e-9);
        assert!((image.sample(1.5, 0.5) - 100.0).abs() < 1e-9);
        assert!((image.sample(1.0, 0.5) - 50.0).abs() < 1e-9);
        assert!((image.sample(-5.0, 9.0) - 0.0).abs() < 1e-9);
    }

    #[test]
    fn strided_planes_drop_the_padding() {
        let plane = [1, 2, 9, 9, 3, 4, 9, 9];
        let image = Luma::from_strided(2, 2, 4, &plane).unwrap();
        assert_eq!(image.data, vec![1, 2, 3, 4]);
        assert!(Luma::from_strided(2, 2, 1, &plane).is_none());
        assert!(Luma::from_strided(2, 3, 4, &plane).is_none());
    }

    #[test]
    fn rgb_luma_uses_rec601_weights() {
        let rgb = Rgb {
            width: 3,
            height: 1,
            data: vec![255, 0, 0, 0, 255, 0, 0, 0, 255],
        };
        assert_eq!(rgb.to_luma().data, vec![76, 150, 29]);
    }
}
