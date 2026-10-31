//! Single-threaded decode cost of the reference decoder: a full search, a tracked read of the
//! next frame, the turn lanes of a decoded frame, the kana lanes of both bands (the phone held
//! upright, so the bands are in view), and a merge of three captures of one shown frame, for
//! 720p (a modern and a legacy camera), 1080p and 480p captures and for a frame whose corner
//! blossom a thumb covers.
//!
//! `cargo run --release -p iroha_petal --example decode_cost`
//!
//! Each figure is the fastest of 18 runs (six poses, three runs each), the run least disturbed by
//! other work on the machine, with the median beside it.
#[path = "support/count.rs"]
mod count;

use count::count_as_f64;

use std::hint::black_box;
use std::time::Instant;

use iroha_petal::decode::{DecodeOptions, decode, field_evidence, merge_fields, track};
use iroha_petal::image::{Luma, Rgb};
use iroha_petal::kana::read_kana;
use iroha_petal::layout::{FINDER_CENTERS, SECTORS, SQUARE_TOP};
use iroha_petal::render::{RenderOptions, render};
use iroha_petal::sim::{CaptureConfig, capture, fit_to_frame};
use iroha_petal::stream::StreamEncoder;
use iroha_petal::turns::read_turns;

/// Side of the rendered source frames in pixels.
const SIZE: usize = 768;

/// The fastest and the median of `samples`, in milliseconds.
fn summary(mut samples: Vec<f64>) -> String {
    samples.sort_by(f64::total_cmp);
    format!(
        "{:.2} ms (median {:.2})",
        samples[0],
        samples[samples.len() / 2]
    )
}

/// Runs `f` and records how long it took in `samples`.
fn timed<T>(samples: &mut Vec<f64>, f: impl FnOnce() -> T) -> T {
    let started = Instant::now();
    let out = black_box(f());
    samples.push(started.elapsed().as_secs_f64() * 1000.0);
    out
}

/// Covers corner blossom 0 of a rendered frame with a black disc of radius 80 canvas units, as a
/// thumb does.
fn thumb(mut source: Rgb) -> Rgb {
    let scale = count_as_f64(SIZE) / 1024.0;
    let (cx, cy) = FINDER_CENTERS[0];
    let (cx, cy) = (
        f64::from(cx) * scale,
        (f64::from(cy) + f64::from(SQUARE_TOP)) * scale,
    );
    for y in 0..source.height {
        for x in 0..SIZE {
            let (dx, dy) = (count_as_f64(x) + 0.5 - cx, count_as_f64(y) + 0.5 - cy);
            if dx.hypot(dy) <= 80.0 * scale {
                source.data[(y * SIZE + x) * 3..][..3].fill(0);
            }
        }
    }
    source
}

fn main() {
    let mut rng = iroha_petal::prng::Xorshift32::new(7);
    let payload: Vec<u8> = (0..12_182).map(|_| rng.next_byte()).collect();
    let encoder = StreamEncoder::new(&payload, 3).expect("payload fits a stream");
    let options = DecodeOptions::default();
    let shown = |frame: u16, covered: bool| {
        let source = render(
            &encoder.cells(frame),
            &RenderOptions {
                size: SIZE,
                supersample: 2,
                ..RenderOptions::default()
            },
        );
        if covered { thumb(source) } else { source }
    };
    let modern_720p = CaptureConfig {
        blur_sigma: 0.7,
        ..CaptureConfig::modern()
    };
    let cases = [
        ("modern 720p", modern_720p, false),
        (
            "legacy 720p (barrel distortion)",
            CaptureConfig::legacy(),
            false,
        ),
        (
            "modern 1080p",
            CaptureConfig {
                width: 1920,
                height: 1080,
                blur_sigma: 1.0,
                ..CaptureConfig::modern()
            },
            false,
        ),
        (
            "480p, blur 1.2 px",
            CaptureConfig {
                width: 640,
                height: 480,
                blur_sigma: 1.2,
                ..CaptureConfig::modern()
            },
            false,
        ),
        ("modern 720p, thumb over a blossom", modern_720p, true),
    ];
    println!(
        "{:<34} {:>24} {:>24} {:>24} {:>24} {:>24}",
        "capture", "full search", "tracked", "turn lanes", "kana lanes", "merge of 3 captures"
    );
    for (name, base, covered) in cases {
        let (mut full, mut tracked, mut turned, mut kana, mut merged) =
            (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
        let (now, next) = (shown(5, covered), shown(6, covered));
        for pose in 0..6u32 {
            let config = fit_to_frame(
                &CaptureConfig {
                    rotation_deg: f64::from(pose) * 37.0,
                    tilt_x_deg: base.tilt_x_deg.max(6.0),
                    seed: u64::from(pose) + 1,
                    ..base
                },
                4.0,
            );
            let this_frame = capture(&now, &config);
            let next_frame = capture(&next, &CaptureConfig { seed: 99, ..config });
            let repeats: Vec<Luma> = (0..3u64)
                .map(|i| {
                    capture(
                        &now,
                        &CaptureConfig {
                            seed: 200 + i,
                            ..config
                        },
                    )
                })
                .collect();
            let decoded: Vec<_> = repeats
                .iter()
                .map(|image| decode(image, &options).expect("decodes"))
                .collect();
            // the same camera held upright, for the kana lanes
            let upright = capture(
                &now,
                &fit_to_frame(
                    &CaptureConfig {
                        rotation_deg: f64::from(80 + 4 * pose),
                        ..config
                    },
                    4.0,
                ),
            );
            let upright_frame = decode(&upright, &options).expect("decodes upright");
            let header = upright_frame
                .atom_packets()
                .first()
                .map(|p| p.header)
                .or_else(|| upright_frame.beacon().map(|b| b.header))
                .expect("the frame names itself");
            for _ in 0..3 {
                let first = timed(&mut full, || {
                    decode(&this_frame, &options).expect("decodes")
                });
                let followed = timed(&mut tracked, || {
                    track(&next_frame, &first, &options).expect("tracks")
                });
                assert!(
                    followed.lanes_ok() >= 4,
                    "{name}: tracked {}",
                    followed.letters()
                );
                timed(&mut turned, || {
                    read_turns(&this_frame, &first, [false; SECTORS])
                });
                timed(&mut kana, || {
                    read_kana(&upright, &upright_frame, header, [false; 2])
                });
                timed(&mut merged, || {
                    let evidence: Vec<_> = repeats
                        .iter()
                        .zip(&decoded)
                        .map(|(image, frame)| {
                            field_evidence(image, frame, &options).expect("field evidence")
                        })
                        .collect();
                    merge_fields(&evidence, [false; SECTORS])
                });
            }
        }
        println!(
            "{name:<34} {:>24} {:>24} {:>24} {:>24} {:>24}",
            summary(full),
            summary(tracked),
            summary(turned),
            summary(kana),
            summary(merged)
        );
    }
}
