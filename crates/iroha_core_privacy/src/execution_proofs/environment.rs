//! Immutable track roads, objects and public weather for the first-release racing relation.
//!
//! Each track is a real road of 1986 at true scale: its three laps are one stretch of road drawn end to
//! end. The bend under every 5 m of it is a generated integer table
//! (`iroha_execution_stark::games::race_v1_roads`, from sora-cars' traced roads), shared with the proof
//! circuit; the arithmetic that reads it is written here independently.

use iroha_data_model::execution_proofs::RaceTrackV1;
use iroha_execution_stark::games::race_v1_roads as roads;

pub(super) const TREE: i64 = 0;
pub(super) const SIGN: i64 = 1;
pub(super) const OIL: i64 = 2;
pub(super) const WIND_PATTERN: [i64; 8] = [0, 1, 2, 1, 0, -1, -2, -1];
pub(super) const OIL_HALF_LENGTH: i64 = 9_000;
pub(super) const OIL_HALF_WIDTH: i64 = 1_400;
pub(super) const WEATHER_TICKS: u32 = 300;
pub(super) const WIND_TICKS: u32 = 90;
/// The guard rails' faces either side of the centre line.
pub(super) const RAIL_MM: i32 = 6_520;
/// Beyond the road's edge a car is on the verge and loses speed.
pub(super) const VERGE_MM: i32 = 6_000;

/// The real road's bend coefficient for each 5 m station from 100 m before the start line.
pub(super) fn road(track: RaceTrackV1) -> &'static [i16] {
    match track {
        RaceTrackV1::NeonTokyo => &roads::NEON_TOKYO,
        RaceTrackV1::Harbor => &roads::HARBOR,
        RaceTrackV1::Sakura => &roads::SAKURA,
    }
}

/// The push of the bend under `progress` on a car at `speed`: `trunc(K * speed^2 / 2^24)` toward the
/// outside of the bend, or `None` off the road.
pub(super) fn bend_force(track: RaceTrackV1, progress: i64, speed: i32) -> Option<i32> {
    let station = usize::try_from(
        progress
            .checked_add(roads::LEAD_MM)?
            .div_euclid(roads::STATION_MM),
    )
    .ok()?;
    let coefficient = i64::from(*road(track).get(station)?);
    let speed = i64::from(speed);
    i32::try_from(coefficient * speed * speed / roads::FORCE_DIVISOR).ok()
}

pub(super) fn kinds(track: RaceTrackV1) -> [i64; 12] {
    match track {
        RaceTrackV1::NeonTokyo => [0, 1, 0, 2, 0, 1, 2, 0, 1, 0, 2, 1],
        RaceTrackV1::Harbor => [1, 2, 0, 1, 0, 2, 1, 0, 2, 1, 0, 2],
        RaceTrackV1::Sakura => [0, 0, 2, 1, 0, 2, 0, 1, 0, 2, 0, 1],
    }
}
pub(super) fn laterals(track: RaceTrackV1) -> [i64; 12] {
    let types = kinds(track);
    std::array::from_fn(|cell| {
        let magnitude = match types[cell] {
            TREE => 7_400,
            SIGN => 5_600,
            _ => 1_800,
        };
        if cell % 2 == 0 { -magnitude } else { magnitude }
    })
}
pub(super) fn centers(track: RaceTrackV1) -> [i64; 12] {
    std::array::from_fn(|cell| {
        (2 * i64::try_from(cell).expect("bounded circuit index") + 1) * track.length_mm() / 24
    })
}
pub(super) fn rain_pattern(track: RaceTrackV1) -> [i64; 4] {
    match track {
        RaceTrackV1::NeonTokyo => [0, 0, 1, 0],
        RaceTrackV1::Harbor => [0, 1, 1, 0],
        RaceTrackV1::Sakura => [0, 0, 0, 1],
    }
}
pub(super) fn wind_strength(track: RaceTrackV1) -> i64 {
    match track {
        RaceTrackV1::NeonTokyo => 4,
        RaceTrackV1::Harbor => 16,
        RaceTrackV1::Sakura => 8,
    }
}
pub(super) fn weather(track: RaceTrackV1, tick: u32) -> (i64, i64) {
    (
        rain_pattern(track)[((tick / WEATHER_TICKS) % 4) as usize],
        WIND_PATTERN[((tick / WIND_TICKS) % 8) as usize] * wind_strength(track),
    )
}
pub(super) fn object(track: RaceTrackV1, progress: i64) -> (i64, i64, i64, i64) {
    let wrapped = progress.rem_euclid(track.length_mm());
    let cell = usize::try_from(wrapped * 12 / track.length_mm()).expect("bounded circuit index");
    (
        wrapped,
        centers(track)[cell],
        kinds(track)[cell],
        laterals(track)[cell],
    )
}
pub(super) fn oil(track: RaceTrackV1, progress: i64, lateral: i32) -> bool {
    let (wrapped, center, kind, x) = object(track, progress);
    kind == OIL
        && (wrapped - center).abs() <= OIL_HALF_LENGTH
        && (i64::from(lateral) - x).abs() <= OIL_HALF_WIDTH
}
pub(super) fn lateral_velocity(
    track: RaceTrackV1,
    tick: u32,
    progress: i64,
    lateral: i32,
    velocity: i32,
    steer: i32,
) -> i32 {
    let rain = weather(track, tick).0 != 0;
    let oil = oil(track, progress, lateral);
    let steer = if rain { steer * 3 / 4 } else { steer };
    let steer = if oil { steer / 2 } else { steer };
    let velocity = if rain || oil {
        (velocity + steer) * 15 / 16
    } else {
        (velocity + steer) * 7 / 8
    };
    let limit = if rain { 280 } else { 320 };
    velocity.clamp(-limit, limit)
}
/// Crossing a sign's center within its 1.3 m radius costs 260 mm/tick once; backward/zero movement
/// cannot retrigger it. Trees stand beyond the rails and oil is driven through, so neither is hit.
pub(super) fn impact(
    track: RaceTrackV1,
    old_progress: i64,
    progress: i64,
    lateral: i32,
    speed: i32,
) -> i32 {
    let (wrapped, center, kind, x) = object(track, old_progress);
    if kind != SIGN
        || wrapped >= center
        || wrapped + (progress - old_progress) < center
        || (i64::from(lateral) - x).abs() >= 1_300
    {
        return speed;
    }
    (speed - 260).max(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn signs_use_swept_crossing_and_exact_contact_edges_without_retrigger() {
        for track in [
            RaceTrackV1::NeonTokyo,
            RaceTrackV1::Harbor,
            RaceTrackV1::Sakura,
        ] {
            for cell in 0..12 {
                let center = centers(track)[cell];
                let x = i32::try_from(laterals(track)[cell]).expect("bounded fixture value");
                let kind = kinds(track)[cell];
                assert_eq!(impact(track, center - 500, center - 1, x, 1000), 1000);
                assert_eq!(impact(track, center, center + 1000, x, 1000), 1000);
                assert_eq!(
                    impact(track, center - 500, center + 500, x + 1300, 1000),
                    1000
                );
                let hit = impact(track, center - 500, center + 500, x, 1000);
                // Only signs are hit; trees stand beyond the rails and oil is driven through.
                assert_eq!(hit, if kind == SIGN { 740 } else { 1000 });
                if kind == TREE {
                    assert!(x.abs() - 1_400 > RAIL_MM - 1_300, "trees are out of reach");
                }
            }
        }
    }
    #[test]
    fn bend_force_reads_the_station_and_truncates_toward_zero() {
        for track in [
            RaceTrackV1::NeonTokyo,
            RaceTrackV1::Harbor,
            RaceTrackV1::Sakura,
        ] {
            let table = road(track);
            let finish = track.length_mm() * 3;
            // Station 0 begins 100 m before the line; the finish lies in the last station.
            assert_eq!(table.len() as i64, (finish + 100_000) / 5_000 + 1);
            assert_eq!(bend_force(track, -100_001, 1_000), None);
            assert!(bend_force(track, finish, 3_000).is_some());
            assert_eq!(bend_force(track, finish + 5_000, 1_000), None);
            assert_eq!(bend_force(track, i64::MAX, 1_000), None);
            assert_eq!(bend_force(track, i64::MIN, 1_000), None);
            for (station, coefficient) in table.iter().enumerate() {
                let start = station as i64 * 5_000 - 100_000;
                for progress in [start, start + 4_999] {
                    let expected = i64::from(*coefficient) * 2_000 * 2_000 / (1 << 24);
                    assert_eq!(
                        bend_force(track, progress, 2_000),
                        Some(expected as i32),
                        "{track:?} station {station}"
                    );
                }
            }
            // A negative push truncates toward zero, like the positive one.
            let negative = table.iter().position(|k| *k < -100).expect("a left push");
            let at = negative as i64 * 5_000 - 100_000;
            let k = i64::from(table[negative]);
            assert_eq!(
                bend_force(track, at, 1_500),
                Some(-((-k * 1_500 * 1_500) / (1 << 24)) as i32)
            );
            assert_eq!(bend_force(track, at, 0), Some(0));
        }
    }
    #[test]
    fn wind_never_moves_a_parked_car_and_inactive_cars_ignore_objects() {
        use super::super::race::{initial_race_state_v1, step_race_v1};
        use iroha_data_model::execution_proofs::RaceInputFrameV1;
        for track in [
            RaceTrackV1::NeonTokyo,
            RaceTrackV1::Harbor,
            RaceTrackV1::Sakura,
        ] {
            let mut state = initial_race_state_v1(track, 2).unwrap();
            for tick in 0..1200 {
                step_race_v1(
                    &mut state,
                    &RaceInputFrameV1 {
                        tick,
                        controls: vec![0, 0],
                    },
                )
                .unwrap();
                assert_eq!(state.cars[0].lateral_mm, -1800);
                assert_eq!(state.cars[1].lateral_mm, 1800);
            }
        }
    }
}
