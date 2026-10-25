//! End-to-end stream simulation: a screen animating Petal frames, a phone
//! camera watching it (frame blending and rolling-shutter tearing included).
//!
//! `cargo run --release -p iroha_petal --example stream_sim -- [trials] [payload bytes] [display fps] [cameras]
//! [--camera-fps N] [--exposure-ms N] [--readout-ms N] [--threads N]`
//!
//! `cameras` is a comma-separated list of camera names (default: all). `--camera-fps` models a
//! decoder that keeps up with only that many camera frames a second (default 30), `--readout-ms`
//! a rolling shutter that reads the rows out over that time (default 0: every row at once), and
//! `--threads` runs that many trials at once (default: every core). `T/F%` is the turn lanes read
//! per field lane decoded.
#[path = "support/count.rs"]
mod count;

use count::count_as_f64;

#[path = "support/trial.rs"]
mod trial;

use std::sync::atomic::AtomicUsize;
use trial::next_trial;

use iroha_petal::prng::Xorshift32;
use iroha_petal::qualify::{CameraModel, StreamTrial, TrialOutcome, run_stream_trial};

/// Keep trial seeds identical across debug and release builds; refuse overflow.
fn trial_seeds(trial: usize) -> Option<(u32, u64)> {
    let payload = u32::try_from(trial)
        .ok()?
        .checked_mul(7919)?
        .checked_add(0xABCD_0000)?;
    let camera = u64::try_from(trial).ok()?.checked_add(1)?;
    Some((payload, camera))
}

/// Floor of `(len - 1) * percentile / 100`, without overflow or float rounding.
fn percentile_index(len: usize, percentile: u8) -> Option<usize> {
    if percentile > 100 {
        return None;
    }
    let last = len.checked_sub(1)?;
    let percentile = usize::from(percentile);
    Some(last / 100 * percentile + last % 100 * percentile / 100)
}

/// Select an integer-indexed percentile; an empty sample has no value.
fn percentile(times: &[f64], value: u8) -> f64 {
    percentile_index(times.len(), value)
        .and_then(|index| times.get(index))
        .copied()
        .unwrap_or(f64::NAN)
}

/// Widen each trial's frame and field-lane counts before summing, refusing aggregate overflow.
fn frame_totals(outcomes: &[TrialOutcome]) -> Option<(u64, u64)> {
    outcomes
        .iter()
        .try_fold((0_u64, 0_u64), |(frames, field_lanes), outcome| {
            Some((
                frames.checked_add(u64::from(outcome.frames))?,
                field_lanes.checked_add(u64::from(outcome.field_lanes))?,
            ))
        })
}

fn main() {
    let mut positional = Vec::new();
    // unset options keep `StreamTrial::typical` exactly
    let (mut camera_fps, mut exposure_ms, mut readout_ms) = (None, None, None);
    let mut threads = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let target = match arg.as_str() {
            "--camera-fps" => &mut camera_fps,
            "--exposure-ms" => &mut exposure_ms,
            "--readout-ms" => &mut readout_ms,
            "--threads" => {
                threads = Some(
                    args.next()
                        .and_then(|v| v.parse::<usize>().ok())
                        .filter(|v| *v > 0)
                        .expect("--threads needs a positive count"),
                );
                continue;
            }
            _ => {
                positional.push(arg);
                continue;
            }
        };
        *target = Some(
            args.next()
                .and_then(|v| v.parse::<f64>().ok())
                .filter(|v| v.is_finite() && *v > 0.0)
                .unwrap_or_else(|| panic!("{arg} needs a positive number")),
        );
    }
    let mut positional = positional.into_iter();
    let trials: usize = positional.next().and_then(|a| a.parse().ok()).unwrap_or(8);
    let payload_len: usize = positional
        .next()
        .and_then(|a| a.parse().ok())
        .unwrap_or(2048);
    let fps: f64 = positional
        .next()
        .and_then(|a| a.parse().ok())
        .unwrap_or(12.0);
    let filter = positional.next();
    if let Some(last) = trials.checked_sub(1) {
        trial_seeds(last).expect("requested trial range exceeds deterministic seed arithmetic");
    }
    println!(
        "payload {payload_len} B, display {fps} fps, camera {} fps, exposure {:.1} ms, readout {} ms",
        camera_fps.unwrap_or(30.0),
        exposure_ms.unwrap_or(1000.0 / 60.0),
        readout_ms.unwrap_or(0.0)
    );
    println!(
        "{:<10} {:>6} {:>8} {:>8} {:>8} {:>7} {:>9} {:>7} {:>7}",
        "camera", "done%", "mean s", "p50 s", "p90 s", "false", "ms/frame", "F%", "T/F%"
    );
    let threads =
        threads.unwrap_or_else(|| std::thread::available_parallelism().map_or(4, usize::from));
    for name in CameraModel::NAMES {
        if filter
            .as_ref()
            .is_some_and(|f| !f.split(',').any(|wanted| name.contains(wanted)))
        {
            continue;
        }
        let camera = CameraModel::named(name).expect("known camera");
        let next = AtomicUsize::new(0);
        let outcomes = std::sync::Mutex::new(Vec::<TrialOutcome>::new());
        std::thread::scope(|scope| {
            for _ in 0..threads {
                scope.spawn(|| {
                    while let Some(trial) = next_trial(&next, trials) {
                        let (payload_seed, camera_seed) =
                            trial_seeds(trial).expect("trial range was checked before spawning");
                        let mut rng = Xorshift32::new(payload_seed);
                        let payload: Vec<u8> = (0..payload_len).map(|_| rng.next_byte()).collect();
                        let typical = StreamTrial::typical(camera, camera_seed);
                        let spec = StreamTrial {
                            display_fps: fps,
                            camera_fps: camera_fps.unwrap_or(typical.camera_fps),
                            exposure_s: exposure_ms.map_or(typical.exposure_s, |ms| ms / 1000.0),
                            readout_s: readout_ms.map_or(typical.readout_s, |ms| ms / 1000.0),
                            ..typical
                        };
                        let outcome = run_stream_trial(&payload, 1, &spec);
                        outcomes.lock().unwrap().push(outcome);
                    }
                });
            }
        });
        let outcomes = outcomes.into_inner().unwrap();
        let mut times: Vec<f64> = outcomes
            .iter()
            .filter(|o| o.payload_matches)
            .filter_map(|o| o.completed_at_s)
            .collect();
        times.sort_by(f64::total_cmp);
        let n = count_as_f64(u64::try_from(outcomes.len()).expect("trial count fits u64"));
        let completed = count_as_f64(u64::try_from(times.len()).expect("completed count fits u64"));
        let wrong = outcomes
            .iter()
            .filter(|o| o.completed_at_s.is_some() && !o.payload_matches)
            .count();
        let (frames, field_lanes) =
            frame_totals(&outcomes).expect("aggregate frame counts fit u64");
        let turn_lanes: u64 = outcomes.iter().map(|o| u64::from(o.turn_lanes)).sum();
        println!(
            "{:<10} {:>6.0} {:>8.2} {:>8.2} {:>8.2} {:>7} {:>9.1} {:>6.0}% {:>6.0}%",
            name,
            100.0 * completed / n,
            times.iter().sum::<f64>() / completed.max(1.0),
            percentile(&times, 50),
            percentile(&times, 90),
            wrong,
            outcomes.iter().map(|o| o.mean_decode_ms).sum::<f64>() / n,
            25.0 * count_as_f64(field_lanes) / count_as_f64(frames.max(1)),
            100.0 * count_as_f64(turn_lanes) / count_as_f64(field_lanes.max(1))
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;

    #[test]
    fn trial_seeds_preserve_values_and_refuse_overflow() {
        assert_eq!(trial_seeds(0), Some((0xABCD_0000, 1)));
        assert_eq!(trial_seeds(1), Some((0xABCD_1EEF, 2)));
        let last = usize::try_from((u32::MAX - 0xABCD_0000) / 7919).unwrap();
        assert!(trial_seeds(last).is_some());
        assert_eq!(trial_seeds(last + 1), None);
        assert_eq!(trial_seeds(usize::MAX), None);
    }

    #[test]
    fn trial_allocation_stops_at_limit_without_wrapping() {
        let next = AtomicUsize::new(0);
        assert_eq!(next_trial(&next, 0), None);
        assert_eq!(next_trial(&next, 2), Some(0));
        assert_eq!(next_trial(&next, 2), Some(1));
        assert_eq!(next_trial(&next, 2), None);
        let next = AtomicUsize::new(usize::MAX - 1);
        assert_eq!(next_trial(&next, usize::MAX), Some(usize::MAX - 1));
        assert_eq!(next_trial(&next, usize::MAX), None);
        assert_eq!(next_trial(&next, usize::MAX), None);
        assert_eq!(next.load(Ordering::SeqCst), usize::MAX);
    }

    #[test]
    fn percentiles_handle_empty_singleton_and_integer_boundaries() {
        assert_eq!(percentile_index(0, 50), None);
        assert_eq!(percentile_index(1, 90), Some(0));
        assert_eq!(percentile_index(11, 90), Some(9));
        assert_eq!(percentile_index(11, 50), Some(5));
        assert_eq!(percentile_index(11, 101), None);
        assert_eq!(percentile_index(usize::MAX, 100), Some(usize::MAX - 1));
        assert!(percentile(&[], 50).is_nan());
        assert_eq!(percentile(&[3.0], 90), 3.0);
        assert_eq!(percentile(&[1.0, 2.0, 3.0], 50), 2.0);
        assert_eq!(percentile(&[1.0, 2.0, 3.0], 90), 2.0);
    }

    #[test]
    #[cfg(target_pointer_width = "64")]
    fn percentile_indices_preserve_integers_beyond_float_precision() {
        assert_eq!(
            percentile_index(9_007_199_254_740_993, 50),
            Some(4_503_599_627_370_496)
        );
        assert_eq!(
            percentile_index(9_007_199_254_740_993, 90),
            Some(8_106_479_329_266_892)
        );
    }

    #[test]
    fn report_counts_preserve_u32_and_exact_f64_integer_boundaries() {
        assert_eq!(count_as_f64(0), 0.0);
        assert_eq!(count_as_f64(u64::from(u32::MAX)), 4_294_967_295.0);
        assert_eq!(count_as_f64(u64::from(u32::MAX) + 1), 4_294_967_296.0);
        assert_eq!(count_as_f64((1_u64 << 53) - 1), 9_007_199_254_740_991.0);
        assert_eq!(count_as_f64(1_u64 << 53), 9_007_199_254_740_992.0);
        assert_eq!(count_as_f64(u64::MAX), 18_446_744_073_709_551_616.0);
    }

    #[test]
    fn frame_aggregation_widens_before_adding() {
        let outcome = TrialOutcome {
            completed_at_s: None,
            payload_matches: false,
            frames: u32::MAX,
            located: 0,
            lane_d: 0,
            field_lanes: u32::MAX,
            turn_lanes: 0,
            mean_decode_ms: 0.0,
        };
        assert_eq!(frame_totals(&[]), Some((0, 0)));
        let doubled = 2 * u64::from(u32::MAX);
        assert_eq!(frame_totals(&[outcome, outcome]), Some((doubled, doubled)));
    }
}
