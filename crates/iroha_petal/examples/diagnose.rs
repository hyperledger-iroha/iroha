//! Per-cell error rates of the decoder under a simulated camera: petals and ring dots read
//! under the decoder's own pose and under the camera's true (undistorted) pose.
//!
//! `cargo run --release -p iroha_petal --example diagnose -- blur=1.4 noise=5 fill=0.8 w=1280 h=720 tilt=10 trials=20`
#[path = "support/count.rs"]
mod count;

use count::count_as_f64;

use iroha_petal::decode::{DecodeOptions, decode, decode_at, observed_cells};
use iroha_petal::geometry::Pose;
use iroha_petal::lanes::FIELD_WORD;
use iroha_petal::layout::{SECTORS, SlotRole, sector_cells, slot_roles};
use iroha_petal::render::{RenderOptions, render};
use iroha_petal::sim::{CaptureConfig, camera_homography, capture, fit_to_frame};
use iroha_petal::stream::StreamEncoder;

/// Parse each setting in its owning type instead of saturating float-to-count casts.
fn apply_setting(
    config: &mut CaptureConfig,
    trials: &mut usize,
    tilt: &mut f64,
    key: &str,
    value: &str,
) {
    match key {
        "w" => config.width = value.parse().expect("integer width"),
        "h" => config.height = value.parse().expect("integer height"),
        "trials" => *trials = value.parse().expect("integer trials"),
        _ => {
            let value: f64 = value.parse().expect("number");
            match key {
                "blur" => config.blur_sigma = value,
                "noise" => config.noise = value,
                "fill" => config.fill = value,
                "tilt" => *tilt = value,
                "k1" => config.lens_k1 = value,
                "ambient" => config.ambient = value,
                "gradient" => config.gradient = value,
                "bloom" => config.bloom = value,
                "sharpen" => config.sharpen = value,
                "exposure" => config.exposure = value,
                _ => panic!("unknown key {key}"),
            }
        }
    }
}

/// Reduce the trial before multiplication, preserving each intended modular pose.
fn trial_geometry(trial: usize) -> (u16, u16, u8, u8) {
    let frame = u16::try_from((trial % 60_000) * 977 % 60_000).expect("frame is below 60000");
    let rotation = u16::try_from((trial % 360) * 47 % 360).expect("rotation is below 360");
    let tilt_x = u8::try_from(trial % 5).expect("tilt phase is below five");
    let tilt_y = (tilt_x + 2) % 5;
    (frame, rotation, tilt_x, tilt_y)
}

fn main() {
    let mut config = CaptureConfig {
        width: 1280,
        height: 720,
        fill: 0.8,
        blur_sigma: 1.2,
        noise: 5.0,
        ..CaptureConfig::modern()
    };
    let mut trials = 12usize;
    let mut tilt = 8.0f64;
    for arg in std::env::args().skip(1) {
        let (k, v) = arg.split_once('=').expect("key=value");
        apply_setting(&mut config, &mut trials, &mut tilt, k, v);
    }
    let payload: Vec<u8> = (0..900u32)
        .map(|i| (i.wrapping_mul(2_654_435_761) >> 11).to_le_bytes()[0])
        .collect();
    let encoder = StreamEncoder::new(&payload, 2).unwrap();
    let options = DecodeOptions::default();
    let roles = slot_roles();
    let coded: Vec<usize> = (0..SECTORS)
        .flat_map(|s| sector_cells(s)[..FIELD_WORD * 8].iter().copied())
        .collect();
    let mut totals = [[0usize; 3]; 2]; // [estimated, oracle] x [petal err, dot err, field lanes ok]
    let mut counted = [0usize; 2];
    for trial in 0..trials {
        let (frame, rotation, tilt_x, tilt_y) = trial_geometry(trial);
        let truth = encoder.cells(frame);
        let source = render(
            &truth,
            &RenderOptions {
                size: 768,
                supersample: 2,
                ..RenderOptions::default()
            },
        );
        let mut c = config;
        c.seed = u64::try_from(trial)
            .expect("trial fits u64")
            .checked_add(1)
            .expect("camera seed fits u64");
        c.rotation_deg = f64::from(rotation);
        c.tilt_x_deg = tilt * (f64::from(tilt_x) - 2.0) / 2.0;
        c.tilt_y_deg = tilt * (f64::from(tilt_y) - 2.0) / 2.0;
        let c = fit_to_frame(&c, 4.0);
        let image = capture(&source, &c);
        let estimated = decode(&image, &options).ok();
        let oracle = decode_at(&image, Pose::plain(camera_homography(&c)), &options);
        for (slot, decoded) in [(0, estimated), (1, oracle)] {
            let Some(decoded) = decoded else { continue };
            let Some(observed) = observed_cells(&image, &decoded, &options) else {
                continue;
            };
            let petal_err = coded
                .iter()
                .filter(|&&cell| observed.petals[cell] != truth.petals[cell])
                .count();
            let dot_err = (0..roles.len())
                .filter(|&i| {
                    matches!(roles[i], SlotRole::Data(_)) && observed.dots[i] != truth.dots[i]
                })
                .count();
            let fields = decoded.fields.iter().filter(|f| f.is_some()).count();
            let t = &mut totals[slot];
            for (total, add) in t.iter_mut().zip([petal_err, dot_err, fields]) {
                *total = total.checked_add(add).expect("diagnostic count fits usize");
            }
            counted[slot] = counted[slot]
                .checked_add(1)
                .expect("trial count fits usize");
        }
    }
    for (slot, name) in [(0, "finder-based"), (1, "oracle pose ")] {
        let n = count_as_f64(counted[slot].max(1));
        println!(
            "{name}: n={} petal err {:.2}%  dot err {:.2}%  field lanes ok {:.0}%",
            counted[slot],
            100.0 * count_as_f64(totals[slot][0]) / n / count_as_f64(coded.len()),
            100.0 * count_as_f64(totals[slot][1]) / n / 240.0,
            25.0 * count_as_f64(totals[slot][2]) / n
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integer_settings_refuse_fractional_negative_and_nonfinite_counts() {
        for key in ["w", "h", "trials"] {
            for value in ["-1", "1.5", "NaN", "inf", "18446744073709551616"] {
                assert!(
                    std::panic::catch_unwind(|| {
                        apply_setting(&mut CaptureConfig::modern(), &mut 12, &mut 8.0, key, value);
                    })
                    .is_err()
                );
            }
        }
        let mut config = CaptureConfig::modern();
        let mut trials = 12;
        let mut tilt = 8.0;
        apply_setting(&mut config, &mut trials, &mut tilt, "w", "1920");
        apply_setting(&mut config, &mut trials, &mut tilt, "h", "1080");
        apply_setting(&mut config, &mut trials, &mut tilt, "trials", "24");
        apply_setting(&mut config, &mut trials, &mut tilt, "tilt", "2.5");
        assert_eq!((config.width, config.height, trials), (1920, 1080, 24));
        assert_eq!(tilt.to_bits(), 2.5_f64.to_bits());
    }

    #[test]
    fn trial_geometry_keeps_modular_boundaries_without_overflow() {
        assert_eq!(trial_geometry(0), (0, 0, 0, 2));
        assert_eq!(trial_geometry(1), (977, 47, 1, 3));
        assert_eq!(trial_geometry(60_000), (0, 120, 0, 2));
        let trial = u128::try_from(usize::MAX).unwrap();
        assert_eq!(
            trial_geometry(usize::MAX),
            (
                u16::try_from(trial * 977 % 60_000).unwrap(),
                u16::try_from(trial * 47 % 360).unwrap(),
                u8::try_from(trial % 5).unwrap(),
                u8::try_from((trial + 2) % 5).unwrap(),
            )
        );
    }
}
