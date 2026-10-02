//! Deterministic camera-capture simulator for qualification.
//!
//! Models what a phone camera does to a frame shown on a screen: perspective
//! and rotation, lens distortion, area integration, optical blur, bloom,
//! auto-exposure clipping, screen glare and ambient reflections, vignetting,
//! sensor noise, gamma, in-camera sharpening and 8-bit quantisation. All
//! randomness comes from a seeded generator so a `(config, seed)` pair always
//! produces the same pixels on every host.

use crate::geometry::Homography;
use crate::image::{Luma, Rgb};

/// Camera and scene parameters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CaptureConfig {
    /// Output width in pixels.
    pub width: usize,
    /// Output height in pixels.
    pub height: usize,
    /// Canvas side as a fraction of the short image side.
    pub fill: f64,
    /// In-plane rotation of the code in degrees.
    pub rotation_deg: f64,
    /// Tilt about the horizontal axis in degrees.
    pub tilt_x_deg: f64,
    /// Tilt about the vertical axis in degrees.
    pub tilt_y_deg: f64,
    /// Offset of the code centre as a fraction of the image size.
    pub shift: (f64, f64),
    /// Radial lens distortion coefficient (negative is barrel).
    pub lens_k1: f64,
    /// Gaussian optical blur sigma in pixels.
    pub blur_sigma: f64,
    /// Linear motion blur length in pixels.
    pub motion_px: f64,
    /// Direction of the motion blur in degrees.
    pub motion_deg: f64,
    /// Fraction of light that spreads into a wide halo.
    pub bloom: f64,
    /// Sigma of the halo in pixels.
    pub bloom_sigma: f64,
    /// Exposure multiplier on top of the simulated auto-exposure (1 = nominal).
    pub exposure: f64,
    /// Ambient light added by reflections, as a fraction of the lit level.
    pub ambient: f64,
    /// Peak-to-peak illumination gradient as a fraction of the lit level.
    pub gradient: f64,
    /// Direction of the gradient in degrees.
    pub gradient_deg: f64,
    /// Corner falloff of the lens (0 = none, 0.4 = strong).
    pub vignette: f64,
    /// Peak glare as a fraction of the lit level.
    pub glare: f64,
    /// Glare centre as a fraction of the image size.
    pub glare_at: (f64, f64),
    /// Glare sigma in pixels.
    pub glare_sigma: f64,
    /// Sensor noise standard deviation in 8-bit levels at mid-grey.
    pub noise: f64,
    /// In-camera unsharp-mask amount.
    pub sharpen: f64,
    /// Seed of the noise generator.
    pub seed: u64,
}

impl CaptureConfig {
    /// A recent phone in good light: 1080p, sharp, quiet.
    #[must_use]
    pub fn modern() -> Self {
        Self {
            width: 1280,
            height: 720,
            fill: 0.85,
            rotation_deg: 0.0,
            tilt_x_deg: 0.0,
            tilt_y_deg: 0.0,
            shift: (0.0, 0.0),
            lens_k1: 0.0,
            blur_sigma: 0.7,
            motion_px: 0.0,
            motion_deg: 0.0,
            bloom: 0.03,
            bloom_sigma: 4.0,
            exposure: 1.0,
            ambient: 0.02,
            gradient: 0.05,
            gradient_deg: 30.0,
            vignette: 0.1,
            glare: 0.0,
            glare_at: (0.5, 0.5),
            glare_sigma: 40.0,
            noise: 2.5,
            sharpen: 0.0,
            seed: 1,
        }
    }

    /// An older phone: 720p, soft focus, noisy, some tilt and barrel distortion.
    #[must_use]
    pub fn legacy() -> Self {
        Self {
            width: 1280,
            height: 720,
            fill: 0.8,
            rotation_deg: 12.0,
            tilt_x_deg: 12.0,
            tilt_y_deg: -10.0,
            shift: (0.02, -0.01),
            lens_k1: -0.06,
            blur_sigma: 1.3,
            motion_px: 0.0,
            motion_deg: 0.0,
            bloom: 0.08,
            bloom_sigma: 5.0,
            exposure: 1.1,
            ambient: 0.06,
            gradient: 0.15,
            gradient_deg: 120.0,
            vignette: 0.25,
            glare: 0.0,
            glare_at: (0.3, 0.3),
            glare_sigma: 50.0,
            noise: 6.0,
            sharpen: 0.4,
            seed: 2,
        }
    }

    /// A weak 480p preview with strong blur, noise, tilt and distortion.
    #[must_use]
    pub fn worst() -> Self {
        Self {
            width: 640,
            height: 480,
            fill: 0.8,
            rotation_deg: -25.0,
            tilt_x_deg: 20.0,
            tilt_y_deg: 18.0,
            shift: (-0.03, 0.02),
            lens_k1: -0.10,
            blur_sigma: 1.7,
            motion_px: 1.5,
            motion_deg: 35.0,
            bloom: 0.12,
            bloom_sigma: 4.0,
            exposure: 1.25,
            ambient: 0.10,
            gradient: 0.25,
            gradient_deg: 200.0,
            vignette: 0.35,
            glare: 0.0,
            glare_at: (0.7, 0.25),
            glare_sigma: 45.0,
            noise: 9.0,
            sharpen: 0.5,
            seed: 3,
        }
    }
}

/// `SplitMix64` with a Box–Muller Gaussian.
struct Rng {
    state: u64,
    spare: Option<f64>,
}

impl Rng {
    fn new(seed: u64) -> Self {
        Self {
            state: seed ^ 0x9E37_79B9_7F4A_7C15,
            spare: None,
        }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn uniform(&mut self) -> f64 {
        ((self.next_u64() >> 11) as f64 + 0.5) / (1u64 << 53) as f64
    }

    fn gaussian(&mut self) -> f64 {
        if let Some(value) = self.spare.take() {
            return value;
        }
        let (u1, u2) = (self.uniform(), self.uniform());
        let radius = (-2.0 * u1.ln()).sqrt();
        let angle = core::f64::consts::TAU * u2;
        self.spare = Some(radius * angle.sin());
        radius * angle.cos()
    }
}

fn srgb_decode(value: f64) -> f64 {
    value.powf(2.2)
}

fn srgb_encode(value: f64) -> f64 {
    value.max(0.0).powf(1.0 / 2.2)
}

fn gaussian_kernel(sigma: f64) -> Vec<f64> {
    let radius = (3.0 * sigma).ceil().max(1.0) as isize;
    let mut kernel: Vec<f64> = (-radius..=radius)
        .map(|i| (-(i as f64).powi(2) / (2.0 * sigma * sigma)).exp())
        .collect();
    let sum: f64 = kernel.iter().sum();
    for k in &mut kernel {
        *k /= sum;
    }
    kernel
}

fn blur(plane: &[f64], width: usize, height: usize, sigma: f64) -> Vec<f64> {
    if sigma < 0.05 {
        return plane.to_vec();
    }
    let kernel = gaussian_kernel(sigma);
    let radius = (kernel.len() / 2) as isize;
    let mut horizontal = vec![0.0; plane.len()];
    for y in 0..height {
        for x in 0..width {
            let mut acc = 0.0;
            for (k, weight) in kernel.iter().enumerate() {
                let sx = (x as isize + k as isize - radius).clamp(0, width as isize - 1) as usize;
                acc += weight * plane[y * width + sx];
            }
            horizontal[y * width + x] = acc;
        }
    }
    let mut out = vec![0.0; plane.len()];
    for y in 0..height {
        for x in 0..width {
            let mut acc = 0.0;
            for (k, weight) in kernel.iter().enumerate() {
                let sy = (y as isize + k as isize - radius).clamp(0, height as isize - 1) as usize;
                acc += weight * horizontal[sy * width + x];
            }
            out[y * width + x] = acc;
        }
    }
    out
}

fn motion_blur(plane: &[f64], width: usize, height: usize, length: f64, degrees: f64) -> Vec<f64> {
    if length < 0.5 {
        return plane.to_vec();
    }
    let steps = (length.ceil() as usize).max(2);
    let (dx, dy) = (degrees.to_radians().cos(), degrees.to_radians().sin());
    let mut out = vec![0.0; plane.len()];
    for y in 0..height {
        for x in 0..width {
            let mut acc = 0.0;
            for s in 0..steps {
                let t = (s as f64 / (steps - 1) as f64 - 0.5) * length;
                let sx = (x as f64 + dx * t).round().clamp(0.0, width as f64 - 1.0) as usize;
                let sy = (y as f64 + dy * t).round().clamp(0.0, height as f64 - 1.0) as usize;
                acc += plane[sy * width + sx];
            }
            out[y * width + x] = acc / steps as f64;
        }
    }
    out
}

/// The ground-truth homography from canvas units to ideal (undistorted) image
/// pixels; exact when `lens_k1` is zero.
#[must_use]
pub fn camera_homography(config: &CaptureConfig) -> Homography {
    let (w, h) = (config.width as f64, config.height as f64);
    let focal = 0.8 * w.max(h);
    let distance = focal * 1024.0 / (config.fill * w.min(h));
    let (rz, rx, ry) = (
        config.rotation_deg.to_radians(),
        config.tilt_x_deg.to_radians(),
        config.tilt_y_deg.to_radians(),
    );
    let (sz, cz) = rz.sin_cos();
    let (sx, cx) = rx.sin_cos();
    let (sy, cy) = ry.sin_cos();
    // R = Rz * Ry * Rx applied to the centred canvas plane
    let rot_x = [[1.0, 0.0, 0.0], [0.0, cx, -sx], [0.0, sx, cx]];
    let rot_y = [[cy, 0.0, sy], [0.0, 1.0, 0.0], [-sy, 0.0, cy]];
    let rot_z = [[cz, -sz, 0.0], [sz, cz, 0.0], [0.0, 0.0, 1.0]];
    let mul = |a: [[f64; 3]; 3], b: [[f64; 3]; 3]| {
        let mut out = [[0.0; 3]; 3];
        for i in 0..3 {
            for j in 0..3 {
                out[i][j] = (0..3).map(|k| a[i][k] * b[k][j]).sum();
            }
        }
        out
    };
    let r = mul(rot_z, mul(rot_y, rot_x));
    // columns r1, r2 and translation (0, 0, distance); centre the canvas first
    let t = [
        distance * 0.0 - 512.0 * r[0][0] - 512.0 * r[0][1],
        -512.0 * r[1][0] - 512.0 * r[1][1],
        distance - 512.0 * r[2][0] - 512.0 * r[2][1],
    ];
    let cx_img = w / 2.0 + config.shift.0 * w;
    let cy_img = h / 2.0 + config.shift.1 * h;
    let k = [[focal, 0.0, cx_img], [0.0, focal, cy_img], [0.0, 0.0, 1.0]];
    let m = [
        [r[0][0], r[0][1], t[0]],
        [r[1][0], r[1][1], t[1]],
        [r[2][0], r[2][1], t[2]],
    ];
    let hm = mul(k, m);
    Homography([
        hm[0][0], hm[0][1], hm[0][2], hm[1][0], hm[1][1], hm[1][2], hm[2][0], hm[2][1], hm[2][2],
    ])
}

/// Shrinks `fill` until all four corner finders lie inside the frame, as a
/// user would by backing away from the screen.
#[must_use]
pub fn fit_to_frame(config: &CaptureConfig, margin_px: f64) -> CaptureConfig {
    let inside = |c: &CaptureConfig| {
        let h = camera_homography(c);
        [(0.0, 0.0), (1024.0, 0.0), (1024.0, 1024.0), (0.0, 1024.0)]
            .iter()
            .all(|&(x, y)| {
                let (px, py) = h.apply(x, y);
                px >= margin_px
                    && py >= margin_px
                    && px <= c.width as f64 - margin_px
                    && py <= c.height as f64 - margin_px
            })
    };
    let mut fitted = *config;
    for _ in 0..40 {
        if inside(&fitted) {
            break;
        }
        fitted.fill *= 0.97;
    }
    fitted
}

/// Pixels per canvas unit at the centre of the code under `config`.
#[must_use]
pub fn pixels_per_unit(config: &CaptureConfig) -> f64 {
    let h = camera_homography(config);
    let (x0, y0) = h.apply(412.0, 512.0);
    let (x1, y1) = h.apply(612.0, 512.0);
    let (x2, y2) = h.apply(512.0, 412.0);
    let (x3, y3) = h.apply(512.0, 612.0);
    0.5 * (((x1 - x0).powi(2) + (y1 - y0).powi(2)).sqrt()
        + ((x3 - x2).powi(2) + (y3 - y2).powi(2)).sqrt())
        / 200.0
}

/// Supersampling factor of the geometric stage of [`capture`].
const SS: usize = 3;

/// Captures `source` (a rendered frame) with the simulated camera.
#[must_use]
pub fn capture(source: &Rgb, config: &CaptureConfig) -> Luma {
    let (w, h) = (config.width, config.height);
    let focal = 0.8 * w.max(h) as f64;
    let forward = camera_homography(config);
    let backward = forward.inverse().expect("camera homography is invertible");
    let (cx_img, cy_img) = (
        w as f64 / 2.0 + config.shift.0 * w as f64,
        h as f64 / 2.0 + config.shift.1 * h as f64,
    );
    // linear-light source
    let src_w = source.width;
    let linear: Vec<f64> = source
        .data
        .chunks_exact(3)
        .map(|px| {
            srgb_decode(
                (0.299 * f64::from(px[0]) + 0.587 * f64::from(px[1]) + 0.114 * f64::from(px[2]))
                    / 255.0,
            )
        })
        .collect();
    let scale = src_w as f64 / 1024.0;
    let sample_source = |x: f64, y: f64| -> f64 {
        let (sx, sy) = (x * scale, y * scale);
        if sx < 0.0 || sy < 0.0 || sx >= src_w as f64 || sy >= source.height as f64 {
            return 0.0;
        }
        let fx = (sx - 0.5).clamp(0.0, (src_w - 1) as f64);
        let fy = (sy - 0.5).clamp(0.0, (source.height - 1) as f64);
        let (x0, y0) = (fx.floor() as usize, fy.floor() as usize);
        let (x1, y1) = ((x0 + 1).min(src_w - 1), (y0 + 1).min(source.height - 1));
        let (tx, ty) = (fx - x0 as f64, fy - y0 as f64);
        let p = |xx: usize, yy: usize| linear[yy * src_w + xx];
        (p(x0, y0) * (1.0 - tx) + p(x1, y0) * tx) * (1.0 - ty)
            + (p(x0, y1) * (1.0 - tx) + p(x1, y1) * tx) * ty
    };
    // geometry with 3x3 supersampling
    let mut plane = vec![0.0f64; w * h];
    for y in 0..h {
        for x in 0..w {
            let mut acc = 0.0;
            for sy in 0..SS {
                for sx in 0..SS {
                    let (mut px, mut py) = (
                        x as f64 + (sx as f64 + 0.5) / SS as f64,
                        y as f64 + (sy as f64 + 0.5) / SS as f64,
                    );
                    // undo lens distortion: p_d = c + (p_u - c)(1 + k1 r^2)
                    if config.lens_k1 != 0.0 {
                        let (dx, dy) = (px - cx_img, py - cy_img);
                        let (mut ux, mut uy) = (dx, dy);
                        for _ in 0..5 {
                            let r2 = (ux * ux + uy * uy) / (focal * focal);
                            let factor = 1.0 + config.lens_k1 * r2;
                            ux = dx / factor;
                            uy = dy / factor;
                        }
                        px = cx_img + ux;
                        py = cy_img + uy;
                    }
                    let (cx, cy) = backward.apply(px, py);
                    acc += sample_source(cx, cy);
                }
            }
            plane[y * w + x] = acc / (SS * SS) as f64;
        }
    }
    // optics
    let mut plane = motion_blur(&plane, w, h, config.motion_px, config.motion_deg);
    let optical = blur(&plane, w, h, config.blur_sigma);
    if config.bloom > 0.0 {
        let halo = blur(&plane, w, h, config.bloom_sigma);
        plane = optical
            .iter()
            .zip(&halo)
            .map(|(o, hl)| (1.0 - config.bloom) * o + config.bloom * hl)
            .collect();
    } else {
        plane = optical;
    }
    // auto exposure: map the 99.5th percentile to 0.85, then apply the multiplier
    let mut sorted = plane.clone();
    sorted.sort_by(f64::total_cmp);
    let p995 = sorted[((sorted.len() - 1) as f64 * 0.995) as usize].max(1e-4);
    let lit_level = p995;
    let gain = 0.85 / p995 * config.exposure;
    let (gx, gy) = (
        config.gradient_deg.to_radians().cos(),
        config.gradient_deg.to_radians().sin(),
    );
    let mut rng = Rng::new(config.seed);
    let (diag_x, diag_y) = (w as f64 / 2.0, h as f64 / 2.0);
    let max_r2 = diag_x * diag_x + diag_y * diag_y;
    let mut encoded = vec![0.0f64; w * h];
    for y in 0..h {
        for x in 0..w {
            let (dx, dy) = (x as f64 - diag_x, y as f64 - diag_y);
            let mut value = plane[y * w + x];
            value *= 1.0 - config.vignette * (dx * dx + dy * dy) / max_r2;
            let along = (dx * gx + dy * gy) / (max_r2.sqrt());
            value += lit_level
                * (config.ambient + config.gradient * (0.5 + 0.5 * along.clamp(-1.0, 1.0)));
            if config.glare > 0.0 {
                let (ex, ey) = (
                    x as f64 - config.glare_at.0 * w as f64,
                    y as f64 - config.glare_at.1 * h as f64,
                );
                value += lit_level
                    * config.glare
                    * (-(ex * ex + ey * ey) / (2.0 * config.glare_sigma.powi(2))).exp();
            }
            let level = srgb_encode((value * gain).clamp(0.0, 1.0)) * 255.0;
            let sigma = config.noise * (0.3 + 0.7 * level / 255.0).sqrt();
            encoded[y * w + x] = level + sigma * rng.gaussian();
        }
    }
    if config.sharpen > 0.0 {
        let soft = blur(&encoded, w, h, 1.4);
        for (v, s) in encoded.iter_mut().zip(&soft) {
            *v += config.sharpen * (*v - s);
        }
    }
    Luma {
        width: w,
        height: h,
        data: encoded
            .iter()
            .map(|v| v.round().clamp(0.0, 255.0) as u8)
            .collect(),
    }
}

/// Mixes two captures to model an exposure that straddles a frame change.
///
/// # Panics
/// Panics when the images differ in size.
#[must_use]
pub fn blend(a: &Luma, b: &Luma, alpha: f64) -> Luma {
    assert_eq!((a.width, a.height), (b.width, b.height));
    Luma {
        width: a.width,
        height: a.height,
        data: a
            .data
            .iter()
            .zip(&b.data)
            .map(|(&x, &y)| (f64::from(x) * (1.0 - alpha) + f64::from(y) * alpha).round() as u8)
            .collect(),
    }
}

/// Rolling-shutter tearing: rows above `row` come from `a`, the rest from `b`.
///
/// # Panics
/// Panics when the images differ in size.
#[must_use]
pub fn tear(a: &Luma, b: &Luma, row: usize) -> Luma {
    assert_eq!((a.width, a.height), (b.width, b.height));
    let split = row.min(a.height) * a.width;
    let mut data = a.data[..split].to_vec();
    data.extend_from_slice(&b.data[split..]);
    Luma {
        width: a.width,
        height: a.height,
        data,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::{RenderOptions, render};
    use crate::stream::StreamEncoder;

    #[test]
    fn capture_is_deterministic_and_places_the_code_in_view() {
        let encoder = StreamEncoder::new(&[7u8; 120], 1).unwrap();
        let frame = render(
            &encoder.cells(0),
            &RenderOptions {
                size: 512,
                supersample: 2,
                ..RenderOptions::default()
            },
        );
        let config = CaptureConfig {
            width: 320,
            height: 240,
            ..CaptureConfig::legacy()
        };
        let a = capture(&frame, &config);
        let b = capture(&frame, &config);
        assert_eq!(a, b);
        let mean = a.data.iter().map(|&v| f64::from(v)).sum::<f64>() / a.data.len() as f64;
        assert!(mean > 8.0 && mean < 120.0, "mean {mean}");
        let max = a.data.iter().copied().max().unwrap();
        assert!(max > 180, "lit regions must be bright, max {max}");
    }

    #[test]
    fn blend_and_tear_combine_frames() {
        let a = Luma::from_raw(2, 2, vec![0, 0, 0, 0]).unwrap();
        let b = Luma::from_raw(2, 2, vec![100, 100, 100, 100]).unwrap();
        assert_eq!(blend(&a, &b, 0.5).data, vec![50; 4]);
        assert_eq!(tear(&a, &b, 1).data, vec![0, 0, 100, 100]);
    }

    #[test]
    fn identity_camera_homography_centres_the_canvas() {
        let config = CaptureConfig {
            width: 640,
            height: 480,
            fill: 0.8,
            rotation_deg: 0.0,
            tilt_x_deg: 0.0,
            tilt_y_deg: 0.0,
            shift: (0.0, 0.0),
            ..CaptureConfig::modern()
        };
        let h = camera_homography(&config);
        let (cx, cy) = h.apply(512.0, 512.0);
        assert!((cx - 320.0).abs() < 1e-6 && (cy - 240.0).abs() < 1e-6);
        let (left, _) = h.apply(0.0, 512.0);
        let (right, _) = h.apply(1024.0, 512.0);
        assert!((right - left - 0.8 * 480.0).abs() < 1e-6);
    }
}
