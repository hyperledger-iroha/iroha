//! Immutable track objects and public weather for the first-release racing relation.

use iroha_data_model::execution_proofs::RaceTrackV1;

pub(super) const TREE: i64 = 0;
pub(super) const SIGN: i64 = 1;
pub(super) const OIL: i64 = 2;
pub(super) const WIND_PATTERN: [i64; 8] = [0, 1, 2, 1, 0, -1, -2, -1];
pub(super) const OIL_HALF_LENGTH: i64 = 9_000;
pub(super) const OIL_HALF_WIDTH: i64 = 1_400;
pub(super) const WEATHER_TICKS: u32 = 300;
pub(super) const WIND_TICKS: u32 = 90;

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
    std::array::from_fn(|cell| (2 * cell as i64 + 1) * track.length_mm() / 24)
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
    let cell = (wrapped * 12 / track.length_mm()) as usize;
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
/// Crossing a solid object's center applies once; backward/zero movement cannot retrigger it.
pub(super) fn impact(
    track: RaceTrackV1,
    old_progress: i64,
    progress: i64,
    lateral: i32,
    speed: i32,
) -> (i32, i32) {
    let (wrapped, center, kind, x) = object(track, old_progress);
    let radius = if kind == TREE { 1_400 } else { 1_300 };
    if kind == OIL
        || wrapped >= center
        || wrapped + (progress - old_progress) < center
        || (i64::from(lateral) - x).abs() >= radius
    {
        return (lateral, speed);
    }
    if kind == TREE {
        let pushed = x + if i64::from(lateral) > x {
            1_800
        } else {
            -1_800
        };
        (pushed.clamp(-9_000, 9_000) as i32, (speed - 600).max(0))
    } else {
        (lateral, (speed - 260).max(0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn solids_use_swept_crossing_and_exact_contact_edges_without_retrigger() {
        for track in [
            RaceTrackV1::NeonTokyo,
            RaceTrackV1::Harbor,
            RaceTrackV1::Sakura,
        ] {
            for cell in 0..12 {
                let center = centers(track)[cell];
                let x = laterals(track)[cell] as i32;
                let kind = kinds(track)[cell];
                if kind == OIL {
                    continue;
                }
                let radius = if kind == TREE { 1400 } else { 1300 };
                assert_eq!(impact(track, center - 500, center - 1, x, 1000), (x, 1000));
                assert_eq!(impact(track, center, center + 1000, x, 1000), (x, 1000));
                assert_eq!(
                    impact(track, center - 500, center + 500, x + radius, 1000),
                    (x + radius, 1000)
                );
                let hit = impact(track, center - 500, center + 500, x, 1000);
                assert_eq!(hit.1, if kind == TREE { 400 } else { 740 });
                if kind == TREE {
                    assert_ne!(hit.0, x);
                }
            }
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
