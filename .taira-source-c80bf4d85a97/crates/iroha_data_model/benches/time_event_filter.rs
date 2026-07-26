//! Benchmarks for `TimeEventFilter` matching performance.
//!
//! Measures counting matches for a frequent schedule with a very small period.
use std::time::Duration;

use criterion::Criterion;
use iroha_data_model::prelude::*;

/// Compute the number of scheduled executions that fall within `interval`.
///
/// Mirrors the library logic without requiring the `transparent_api` feature.
#[allow(clippy::option_if_let_else)]
fn count_matches_in_interval(schedule: &TimeSchedule, interval: &TimeInterval) -> u32 {
    match schedule.period() {
        None => u32::from(
            (interval.since()..interval.since() + interval.length()).contains(&schedule.start()),
        ),
        Some(period) => {
            #[allow(clippy::integer_division)]
            let k = interval
                .since()
                .saturating_sub(schedule.start())
                .as_millis()
                / period.as_millis();
            let start = schedule.start() + multiply_duration_by_u128(period, k);
            let range = interval.since()..interval.since() + interval.length();
            (0..)
                .map(|i| start + period * i)
                .skip_while(|time| *time < interval.since())
                .take_while(|time| range.contains(time))
                .count()
                .try_into()
                .expect("Overflow. The schedule is too frequent relative to the interval length")
        }
    }
}

/// Multiply `duration` by `n`, supporting large `n` values.
fn multiply_duration_by_u128(duration: Duration, n: u128) -> Duration {
    if let Ok(n) = u32::try_from(n) {
        return duration * n;
    }

    let new_ms = duration.as_millis() * n;
    if let Ok(ms) = u64::try_from(new_ms) {
        return Duration::from_millis(ms);
    }

    #[allow(clippy::integer_division)]
    let new_secs = u64::try_from(new_ms / 1000)
        .expect("Overflow. Resulting number in seconds can't be represented as `u64`");
    Duration::from_secs(new_secs)
}

/// Benchmark counting matches from zero with a very small period.
fn schedule_from_zero_with_little_period(criterion: &mut Criterion) {
    //                       *         *
    // --|-*-*-*- ... -*-*-*-[-*-...-*-)-*-*-*-
    //   p     52 years     i1   1sec  i2

    const TIMESTAMP: u64 = 1_647_443_386;

    let since = Duration::from_secs(TIMESTAMP);
    let length = Duration::from_secs(1);
    let interval = TimeInterval::new(since, length);
    let schedule = TimeSchedule::starting_at(Duration::ZERO).with_period(Duration::from_millis(1));

    criterion.bench_function("count_matches_from_zero", |b| {
        b.iter(|| {
            let total = count_matches_in_interval(
                std::hint::black_box(&schedule),
                std::hint::black_box(&interval),
            );
            std::hint::black_box(total);
        });
    });
}

/// Entry point for the benchmark binary.
fn main() {
    let mut c = Criterion::default().configure_from_args();
    schedule_from_zero_with_little_period(&mut c);
    c.final_summary();
}
