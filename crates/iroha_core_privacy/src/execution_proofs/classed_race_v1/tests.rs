//! Reference acceptance boundaries; these tests do not stand in for AIR or proof qualification.
use super::*;
use iroha_data_model::classed_race_v1::*;

fn grid(players: u8) -> ClassedRaceStateV1 {
    initial_classed_race_state_v1(
        ClassedRaceClassV1::TouringS1,
        ClassedRaceTrackV1::NeonTokyo,
        players,
    )
    .unwrap()
}
fn tick(state: &mut ClassedRaceStateV1, controls: Vec<u16>) {
    let frame = ClassedRaceInputFrameV1 {
        tick: state.tick,
        controls,
    };
    step_classed_race_v1(state, &frame).unwrap();
}
fn prefix(ticks: u32) -> ClassedRaceReplayV1 {
    ClassedRaceReplayV1 {
        version: 1,
        class_id: ClassedRaceClassV1::TouringS1,
        track: ClassedRaceTrackV1::NeonTokyo,
        player_count: 2,
        frames: (0..ticks)
            .map(|tick| ClassedRaceInputFrameV1 {
                tick,
                controls: vec![33, 1],
            })
            .collect(),
        dnf_events: vec![],
    }
}

#[test]
fn stock_comparison_and_straight_line_are_exact_and_equal_for_both_cars() {
    let spec = class_performance_v1(ClassedRaceClassV1::TouringS1);
    assert_eq!(
        (
            spec.acceleration * 10,
            spec.normal_speed * 10,
            spec.boost_speed * 10
        ),
        (40 * 11, 2_400 * 11, 3_000 * 11)
    );
    let mut state = grid(2);
    for _ in 0..64 {
        tick(&mut state, vec![1, 1]);
    }
    // sum(44 * n, n=1..60) + 4 * 2640; the stock result is 82,800 mm.
    assert_eq!(
        state
            .cars
            .iter()
            .map(|car| car.progress_mm)
            .collect::<Vec<_>>(),
        vec![91_080; 2]
    );
    assert!(
        state
            .cars
            .iter()
            .all(|car| car.speed_mm_per_tick == 2_640 && car.boost_energy == 1_000)
    );
    assert_eq!(
        [state.cars[0].lateral_mm, state.cars[1].lateral_mm],
        [-1_800, 1_800]
    );
    assert!(classed_race_result_v1(&state).unwrap().winners.is_empty());
    assert!(initial_classed_race_state_v1(state.class_id, state.track, 0).is_err());
    assert!(initial_classed_race_state_v1(state.class_id, state.track, 9).is_err());
    assert_eq!(grid(8).cars[7].progress_mm, -12_000);
}

#[test]
fn boost_threshold_brake_precedence_and_negative_rounding_are_exact() {
    for (energy, expected_speed, remaining) in [(24, 2_640, 28), (25, 3_300, 0)] {
        let mut state = grid(2);
        state.cars[0].speed_mm_per_tick = 3_280;
        state.cars[0].boost_energy = energy;
        tick(&mut state, vec![33, 0]);
        assert_eq!(
            (state.cars[0].speed_mm_per_tick, state.cars[0].boost_energy),
            (expected_speed, remaining)
        );
    }
    let mut state = grid(2);
    state.cars[0].speed_mm_per_tick = 500;
    state.cars[0].lateral_velocity_mm_per_tick = -1;
    tick(&mut state, vec![1 | 2 | 4 | 8, 0]);
    assert_eq!(
        (
            state.cars[0].speed_mm_per_tick,
            state.cars[0].lateral_velocity_mm_per_tick
        ),
        (400, 0)
    );
    tick(&mut state, vec![4, 0]);
    assert_eq!(state.cars[0].lateral_velocity_mm_per_tick, -15);
    let mut drift = grid(2);
    tick(&mut drift, vec![4 | 16, 0]);
    assert_eq!(drift.cars[0].lateral_velocity_mm_per_tick, -24);
}

#[test]
fn curve_samples_old_progress_and_uses_euclidean_wrapping() {
    let mut state = grid(2);
    state.track = ClassedRaceTrackV1::Harbor;
    state.cars[0].progress_mm = 199_999;
    state.cars[0].speed_mm_per_tick = 2_640;
    state.cars[0].lateral_mm = 0;
    tick(&mut state, vec![1, 0]);
    assert_eq!(
        state.cars[0].lateral_mm, 0,
        "crossing a cell cannot apply the next cell early"
    );
    tick(&mut state, vec![1, 0]);
    assert_eq!(state.cars[0].lateral_mm, -44);
    let mut wrapped = grid(2);
    wrapped.track = ClassedRaceTrackV1::Harbor;
    wrapped.cars[0].progress_mm = -1;
    wrapped.cars[0].speed_mm_per_tick = 2_640;
    tick(&mut wrapped, vec![1, 0]);
    assert_eq!(
        wrapped.cars[0].lateral_mm, -1_800,
        "negative starting progress remains bounded in the final zero-curvature cell"
    );
}

#[test]
fn contacts_have_strict_boundaries_and_canonical_multi_car_order() {
    let mut state = grid(3);
    for car in &mut state.cars {
        car.progress_mm = 0;
        car.lateral_mm = 0;
    }
    tick(&mut state, vec![0; 3]);
    assert_eq!(
        state
            .cars
            .iter()
            .map(|car| car.lateral_mm)
            .collect::<Vec<_>>(),
        vec![-1_350, 1_575, -225]
    );
    for (distance, width, expected) in [
        (3_600, 0, [0, 0]),
        (0, 1_800, [0, 1_800]),
        (0, 1_799, [-1, 1_800]),
    ] {
        let mut state = grid(2);
        state.cars[0].lateral_mm = 0;
        state.cars[1].lateral_mm = width;
        state.cars[1].progress_mm = distance;
        tick(&mut state, vec![0, 0]);
        assert_eq!(
            [state.cars[0].lateral_mm, state.cars[1].lateral_mm],
            expected
        );
    }
    let mut state = grid(2);
    state.cars[0].lateral_mm = 6_000;
    state.cars[1].lateral_mm = -6_001;
    tick(&mut state, vec![1, 1]);
    assert_eq!(
        [
            state.cars[0].speed_mm_per_tick,
            state.cars[1].speed_mm_per_tick
        ],
        [44, 0]
    );
}

#[test]
fn finishing_and_dnf_freeze_cars_but_all_forfeit_refunds_even_prior_finishers() {
    let mut state = grid(2);
    state.cars[0].progress_mm = 5_999_999;
    tick(&mut state, vec![1, 1]);
    assert_eq!(
        (state.cars[0].progress_mm, state.cars[0].finish_tick),
        (6_000_000, Some(1))
    );
    let prefix = classed_race_result_v1(&state).unwrap();
    assert!(!prefix.terminal && prefix.winners.is_empty());
    apply_classed_race_dnf_v1(&mut state, &[0, 1]).unwrap();
    let frozen = state.cars.clone();
    while state.tick < 6 {
        tick(&mut state, vec![63; 2]);
    }
    assert_eq!(state.cars, frozen);
    let result = classed_race_result_v1(&state).unwrap();
    assert!(result.terminal);
    assert!(
        result.winners.is_empty(),
        "all-forfeit must refund despite a physical finish"
    );
    assert_eq!(result.standings[0].finish_tick, Some(1));
    assert_eq!(result.standings[0].dnf_tick, Some(1));
    let before = state.clone();
    assert_eq!(
        step_classed_race_v1(
            &mut state,
            &ClassedRaceInputFrameV1 {
                tick: 6,
                controls: vec![0; 2]
            }
        ),
        Err(ClassedRaceSimulationErrorV1::Terminal)
    );
    assert_eq!(state, before);
}

#[test]
fn technical_win_occurs_before_forced_batch_motion_and_replay_rejects_extra_ticks() {
    let mut replay = prefix(6);
    let checkpoint = replay_classed_race_v1(&replay).unwrap();
    replay.dnf_events.push(ClassedRaceDnfEventV1 {
        tick: 6,
        slots: vec![1],
    });
    let state = replay_classed_race_v1(&replay).unwrap();
    assert_eq!(state.tick, checkpoint.tick);
    assert_eq!(state.cars[0], checkpoint.cars[0]);
    assert_eq!(classed_race_result_v1(&state).unwrap().winners, vec![0]);
    replay.frames.push(ClassedRaceInputFrameV1 {
        tick: 6,
        controls: vec![1, 0],
    });
    assert_eq!(
        replay_classed_race_v1(&replay),
        Err(ClassedRaceSimulationErrorV1::Terminal)
    );
}

#[test]
fn timeout_ties_use_exact_eligible_progress_and_all_forfeit_refunds() {
    let mut state = grid(3);
    state.tick = 5_400;
    for (car, progress) in state.cars.iter_mut().zip([500, 500, 999]) {
        car.progress_mm = progress;
    }
    state.cars[2].dnf_tick = Some(5_394);
    let result = classed_race_result_v1(&state).unwrap();
    assert!(result.terminal);
    assert_eq!(result.winners, vec![0, 1]);
    assert_eq!(
        result
            .standings
            .iter()
            .map(|car| car.slot)
            .collect::<Vec<_>>(),
        vec![0, 1, 2]
    );
    state.cars[1].progress_mm += 1;
    assert_eq!(classed_race_result_v1(&state).unwrap().winners, vec![1]);
    apply_classed_race_dnf_v1(&mut state, &[0, 1]).unwrap();
    assert!(classed_race_result_v1(&state).unwrap().winners.is_empty());
}

#[test]
fn malformed_inputs_events_and_overflow_states_fail_atomically() {
    for control in [64, u16::MAX] {
        let mut state = grid(2);
        let before = state.clone();
        assert_eq!(
            step_classed_race_v1(
                &mut state,
                &ClassedRaceInputFrameV1 {
                    tick: 0,
                    controls: vec![control, 1]
                }
            ),
            Err(ClassedRaceSimulationErrorV1::ControlBits)
        );
        assert_eq!(state, before);
    }
    for case in 0..7 {
        let mut state = grid(2);
        match case {
            0 => state.cars[0].progress_mm = i64::MAX,
            1 => state.cars[0].lateral_mm = i32::MIN,
            2 => state.cars[0].speed_mm_per_tick = i32::MAX,
            3 => state.cars[0].boost_energy = u16::MAX,
            4 => state.tick = u32::MAX,
            5 => state.cars[0].finish_tick = Some(1),
            _ => state.cars[0].progress_mm = 6_000_000,
        }
        let before = state.clone();
        assert_eq!(
            step_classed_race_v1(
                &mut state,
                &ClassedRaceInputFrameV1 {
                    tick: before.tick,
                    controls: vec![63; 2]
                }
            ),
            Err(ClassedRaceSimulationErrorV1::StateBounds)
        );
        assert_eq!(state, before);
    }
    for slots in [vec![], vec![1, 0], vec![0, 0], vec![2]] {
        let mut state = grid(2);
        let before = state.clone();
        assert_eq!(
            apply_classed_race_dnf_v1(&mut state, &slots),
            Err(ClassedRaceSimulationErrorV1::DnfSequence)
        );
        assert_eq!(state, before);
    }
    let mut replay = prefix(6);
    replay.version = 2;
    assert_eq!(
        replay_classed_race_v1(&replay),
        Err(ClassedRaceSimulationErrorV1::Version)
    );
    replay.version = 1;
    replay.frames[3].tick = 4;
    assert_eq!(
        replay_classed_race_v1(&replay),
        Err(ClassedRaceSimulationErrorV1::TickSequence)
    );
    replay = prefix(6);
    replay.dnf_events = vec![
        ClassedRaceDnfEventV1 {
            tick: 6,
            slots: vec![1],
        },
        ClassedRaceDnfEventV1 {
            tick: 6,
            slots: vec![0],
        },
    ];
    assert_eq!(
        replay_classed_race_v1(&replay),
        Err(ClassedRaceSimulationErrorV1::TickSequence)
    );
    replay.dnf_events.truncate(1);
    replay.dnf_events[0].tick = 7;
    assert_eq!(
        replay_classed_race_v1(&replay),
        Err(ClassedRaceSimulationErrorV1::TickSequence)
    );
}

#[test]
fn every_control_mask_preserves_extreme_valid_state_bounds() {
    for mask in 0..64 {
        for x in [-15_300, 15_300] {
            for vx in [-320, 320] {
                for energy in [0, 24, 25, 1_000] {
                    let mut state = grid(8);
                    for (slot, car) in state.cars.iter_mut().enumerate() {
                        car.lateral_mm = x;
                        car.lateral_velocity_mm_per_tick = vx;
                        car.speed_mm_per_tick = if slot % 2 == 0 { 3_300 } else { 0 };
                        car.boost_energy = energy;
                    }
                    tick(&mut state, vec![mask; 8]);
                    assert!(state.cars.iter().all(|car| car.lateral_mm.abs() <= 15_300
                        && (0..=3_300).contains(&car.speed_mm_per_tick)
                        && car.boost_energy <= 1_000));
                }
            }
        }
    }
}

#[test]
fn all_three_tracks_accept_full_eight_car_maximum_duration_replays() {
    for track in [
        ClassedRaceTrackV1::NeonTokyo,
        ClassedRaceTrackV1::Harbor,
        ClassedRaceTrackV1::Sakura,
    ] {
        assert!(classed_track_length_v1(track) > 0);
        assert!(
            classed_track_curvature_v1(track)
                .iter()
                .all(|curve| curve.abs() <= 3)
        );
        let replay = ClassedRaceReplayV1 {
            version: 1,
            class_id: ClassedRaceClassV1::TouringS1,
            track,
            player_count: 8,
            frames: (0..5_400)
                .map(|tick| ClassedRaceInputFrameV1 {
                    tick,
                    controls: (0..8)
                        .map(|slot| ((tick * 17 + slot * 11) % 64) as u16)
                        .collect(),
                })
                .collect(),
            dnf_events: vec![],
        };
        let state = replay_classed_race_v1(&replay).unwrap();
        assert_eq!(state.tick, 5_400);
        assert!(classed_race_is_terminal_v1(&state).unwrap());
        let result = classed_race_result_v1(&state).unwrap();
        assert!(
            !result.winners.is_empty(),
            "eligible timeout racers cannot force a refund"
        );
        assert_eq!(result.standings.len(), 8);
        assert!(result.winners.windows(2).all(|pair| pair[0] < pair[1]));
    }
}

#[test]
fn forfeit_after_finishing_ranks_below_every_eligible_car() {
    let mut state = grid(3);
    state.tick = 5_400;
    state.cars[0].progress_mm = 6_000_000;
    state.cars[0].finish_tick = Some(5_000);
    state.cars[0].dnf_tick = Some(5_004);
    state.cars[1].progress_mm = 250;
    state.cars[2].progress_mm = 300;
    let result = classed_race_result_v1(&state).unwrap();
    assert_eq!(result.winners, vec![2]);
    assert_eq!(
        result
            .standings
            .iter()
            .map(|car| car.slot)
            .collect::<Vec<_>>(),
        vec![2, 1, 0]
    );
    state.cars[1].progress_mm = 6_000_000;
    state.cars[1].finish_tick = Some(5_001);
    let result = classed_race_result_v1(&state).unwrap();
    assert_eq!(
        result.winners,
        vec![1],
        "forfeited earlier finisher cannot take the prize"
    );
    assert_eq!(
        result
            .standings
            .iter()
            .map(|car| car.slot)
            .collect::<Vec<_>>(),
        vec![1, 2, 0]
    );
}
