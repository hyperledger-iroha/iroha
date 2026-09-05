//! Exact integer RaceV1 reference simulation shared by trace generation and replay tools.

use iroha_data_model::execution_proofs::{
    RACE_CONTROL_MASK_V1, RACE_LAPS_V1, RACE_MAX_PLAYERS_V1, RACE_MAX_TICKS_V1, RaceCarStateV1,
    RaceInputFrameV1, RaceReplayV1, RaceResultV1, RaceStandingV1, RaceStateV1, RaceTrackV1,
};
use thiserror::Error;

/// Invalid replay structure or simulation state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum RaceSimulationErrorV1 {
    /// Player count is outside the fixed catalog bound.
    #[error("race requires between one and eight players")]
    PlayerCount,
    /// Controls do not contain exactly one mask per participant.
    #[error("race input frame does not match the participant count")]
    ControlCount,
    /// Reserved control bits were set.
    #[error("race controls contain reserved bits")]
    ControlBits,
    /// Frame sequence differs from the current state tick.
    #[error("race frame tick is not consecutive")]
    TickSequence,
    /// The run has already reached its fixed terminal boundary.
    #[error("race is already terminal")]
    Terminal,
    /// A supplied state falls outside the reachable integer bounds.
    #[error("race state is outside the bounded simulation domain")]
    StateBounds,
}

/// Create the immutable two-column staggered starting grid.
pub fn initial_race_state_v1(
    track: RaceTrackV1,
    player_count: u8,
) -> Result<RaceStateV1, RaceSimulationErrorV1> {
    if !(1..=RACE_MAX_PLAYERS_V1).contains(&player_count) {
        return Err(RaceSimulationErrorV1::PlayerCount);
    }
    Ok(RaceStateV1 {
        tick: 0,
        track,
        cars: (0..player_count)
            .map(|slot| RaceCarStateV1 {
                progress_mm: -i64::from(slot / 2) * 4_000,
                lateral_mm: if slot % 2 == 0 { -1_800 } else { 1_800 },
                speed_mm_per_tick: 0,
                lateral_velocity_mm_per_tick: 0,
                boost_energy: 1_000,
                finish_tick: None,
                dnf_tick: None,
            })
            .collect(),
    })
}

fn validate_state_v1(state: &RaceStateV1) -> Result<(), RaceSimulationErrorV1> {
    if state.cars.is_empty() || state.cars.len() > usize::from(RACE_MAX_PLAYERS_V1) {
        return Err(RaceSimulationErrorV1::PlayerCount);
    }
    let finish = state.track.length_mm() * i64::from(RACE_LAPS_V1);
    if state.tick > RACE_MAX_TICKS_V1
        || state.cars.iter().any(|car| {
            !(-12_000..=finish).contains(&car.progress_mm)
                || !(-16_200..=16_200).contains(&car.lateral_mm)
                || !(0..=3_000).contains(&car.speed_mm_per_tick)
                || !(-320..=320).contains(&car.lateral_velocity_mm_per_tick)
                || car.boost_energy > 1_000
                || car
                    .finish_tick
                    .is_some_and(|tick| tick == 0 || tick > state.tick)
                || (car.finish_tick.is_some() && car.progress_mm != finish)
                || car.dnf_tick.is_some_and(|tick| tick > state.tick)
                || (car.finish_tick.is_some() && car.dnf_tick.is_some())
        })
    {
        return Err(RaceSimulationErrorV1::StateBounds);
    }
    Ok(())
}

/// Advance one exact tick. Rust integer division truncates toward zero, including negative values.
pub fn step_race_v1(
    state: &mut RaceStateV1,
    frame: &RaceInputFrameV1,
) -> Result<(), RaceSimulationErrorV1> {
    validate_state_v1(state)?;
    if state.tick == RACE_MAX_TICKS_V1
        || (state.tick % 6 == 0
            && state
                .cars
                .iter()
                .all(|car| car.finish_tick.is_some() || car.dnf_tick.is_some()))
    {
        return Err(RaceSimulationErrorV1::Terminal);
    }
    if frame.tick != state.tick {
        return Err(RaceSimulationErrorV1::TickSequence);
    }
    if frame.controls.len() != state.cars.len() {
        return Err(RaceSimulationErrorV1::ControlCount);
    }
    if frame
        .controls
        .iter()
        .any(|control| control & !RACE_CONTROL_MASK_V1 != 0)
    {
        return Err(RaceSimulationErrorV1::ControlBits);
    }
    let length = state.track.length_mm();
    let curves = state.track.curvature();
    for (car, control) in state.cars.iter_mut().zip(&frame.controls) {
        if car.finish_tick.is_some() || car.dnf_tick.is_some() {
            continue;
        }
        let boost = control & 32 != 0 && car.boost_energy >= 25;
        car.boost_energy = if boost {
            car.boost_energy - 25
        } else {
            (car.boost_energy + 4).min(1_000)
        };
        let maximum_speed = if boost { 3_000 } else { 2_400 };
        let acceleration = if control & 2 != 0 {
            -100
        } else if control & 1 != 0 {
            40
        } else {
            -12
        };
        car.speed_mm_per_tick = (car.speed_mm_per_tick + acceleration).clamp(0, maximum_speed);
        let steer = i32::from(control & 8 != 0) - i32::from(control & 4 != 0);
        let steering_force = if control & 16 != 0 { 28 } else { 18 };
        car.lateral_velocity_mm_per_tick =
            ((car.lateral_velocity_mm_per_tick + steer * steering_force) * 7 / 8).clamp(-320, 320);
        let segment = usize::try_from(car.progress_mm.rem_euclid(length) * 12 / length)
            .map_err(|_| RaceSimulationErrorV1::StateBounds)?;
        let curvature_force = curves[segment] * car.speed_mm_per_tick / 120;
        car.lateral_mm = (car.lateral_mm + car.lateral_velocity_mm_per_tick + curvature_force)
            .clamp(-9_000, 9_000);
        if car.lateral_mm.abs() > 6_000 {
            car.speed_mm_per_tick = (car.speed_mm_per_tick - 90).max(0);
        }
        car.progress_mm += i64::from(car.speed_mm_per_tick);
    }
    // Every pair observes effects of the preceding pairs in this exact slot order.
    for left_slot in 0..state.cars.len() {
        for right_slot in left_slot + 1..state.cars.len() {
            let (left, right) = state.cars.split_at_mut(right_slot);
            let a = &mut left[left_slot];
            let b = &mut right[0];
            if a.finish_tick.is_some()
                || b.finish_tick.is_some()
                || a.dnf_tick.is_some()
                || b.dnf_tick.is_some()
                || (a.progress_mm - b.progress_mm).abs() >= 3_600
                || (a.lateral_mm - b.lateral_mm).abs() >= 1_800
            {
                continue;
            }
            let push = (1_800 - (a.lateral_mm - b.lateral_mm).abs() + 1) / 2;
            if a.lateral_mm <= b.lateral_mm {
                a.lateral_mm -= push;
                b.lateral_mm += push;
            } else {
                a.lateral_mm += push;
                b.lateral_mm -= push;
            }
            a.speed_mm_per_tick = (a.speed_mm_per_tick - 120).max(0);
            b.speed_mm_per_tick = (b.speed_mm_per_tick - 120).max(0);
        }
    }
    state.tick += 1;
    let finish = length * i64::from(RACE_LAPS_V1);
    for car in &mut state.cars {
        if car.finish_tick.is_none() && car.dnf_tick.is_none() && car.progress_mm >= finish {
            car.progress_mm = finish;
            car.finish_tick = Some(state.tick);
        }
    }
    Ok(())
}

/// Apply a unique ascending list of consensus-authorized removals atomically.
/// Already-finished cars retain their finish; repeated removals are invalid.
pub fn apply_race_dnf_v1(
    state: &mut RaceStateV1,
    slots: &[u8],
) -> Result<(), RaceSimulationErrorV1> {
    validate_state_v1(state)?;
    if slots.is_empty()
        || slots.windows(2).any(|pair| pair[0] >= pair[1])
        || slots.iter().any(|slot| {
            usize::from(*slot) >= state.cars.len()
                || state.cars[usize::from(*slot)].dnf_tick.is_some()
        })
    {
        return Err(RaceSimulationErrorV1::StateBounds);
    }
    for slot in slots {
        let car = &mut state.cars[usize::from(*slot)];
        if car.finish_tick.is_none() {
            car.dnf_tick = Some(state.tick);
        }
    }
    Ok(())
}

/// Replay a complete or prefix transcript with strict frame and integer-domain validation.
pub fn replay_race_v1(replay: &RaceReplayV1) -> Result<RaceStateV1, RaceSimulationErrorV1> {
    if replay.frames.len() > RACE_MAX_TICKS_V1 as usize {
        return Err(RaceSimulationErrorV1::Terminal);
    }
    let mut state = initial_race_state_v1(replay.track, replay.player_count)?;
    if replay
        .dnf_events
        .windows(2)
        .any(|pair| pair[0].tick >= pair[1].tick)
        || replay
            .dnf_events
            .iter()
            .any(|event| event.tick > replay.frames.len() as u32)
    {
        return Err(RaceSimulationErrorV1::TickSequence);
    }
    let mut events = replay.dnf_events.iter().peekable();
    for frame in &replay.frames {
        if events.peek().is_some_and(|event| event.tick == state.tick) {
            apply_race_dnf_v1(&mut state, &events.next().expect("peeked event").slots)?;
        }
        step_race_v1(&mut state, frame)?;
    }
    if let Some(event) = events.next() {
        apply_race_dnf_v1(&mut state, &event.slots)?;
    }
    Ok(state)
}

/// Derive standings and every tied winner without allowing a display tie-break to redirect prizes.
pub fn race_result_v1(state: &RaceStateV1) -> Result<RaceResultV1, RaceSimulationErrorV1> {
    validate_state_v1(state)?;
    let mut standings: Vec<_> = state
        .cars
        .iter()
        .enumerate()
        .map(|(slot, car)| RaceStandingV1 {
            slot: slot as u8,
            finish_tick: car.finish_tick,
            dnf_tick: car.dnf_tick,
            progress_mm: car.progress_mm,
        })
        .collect();
    standings.sort_by(|a, b| match (a.finish_tick, b.finish_tick) {
        (Some(a_tick), Some(b_tick)) => a_tick.cmp(&b_tick).then(a.slot.cmp(&b.slot)),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => a
            .dnf_tick
            .is_some()
            .cmp(&b.dnf_tick.is_some())
            .then(b.progress_mm.cmp(&a.progress_mm))
            .then(a.slot.cmp(&b.slot)),
    });
    let earliest = standings.first().and_then(|standing| standing.finish_tick);
    let winners = standings
        .iter()
        .filter(|standing| earliest.is_some() && standing.finish_tick == earliest)
        .map(|standing| standing.slot)
        .collect();
    Ok(RaceResultV1 {
        ticks: state.tick,
        standings,
        winners,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dnf_removes_contact_and_cannot_change_a_finished_winner() {
        let mut state = initial_race_state_v1(RaceTrackV1::NeonTokyo, 2).expect("grid");
        state.cars[0].lateral_mm = 0;
        state.cars[1].lateral_mm = 0;
        apply_race_dnf_v1(&mut state, &[0]).expect("remove");
        let removed = state.cars[0].clone();
        step_race_v1(
            &mut state,
            &RaceInputFrameV1 {
                tick: 0,
                controls: vec![1, 1],
            },
        )
        .expect("tick");
        assert_eq!(state.cars[0], removed);
        assert_eq!(state.cars[1].lateral_mm, 0);
        state.cars[1].progress_mm = 6_000_000;
        state.cars[1].finish_tick = Some(1);
        apply_race_dnf_v1(&mut state, &[1]).expect("finished car retained");
        assert_eq!(state.cars[1].dnf_tick, None);
        assert_eq!(race_result_v1(&state).expect("result").winners, vec![1]);
    }

    #[test]
    fn grid_and_first_tick_are_exact() {
        let mut state = initial_race_state_v1(RaceTrackV1::NeonTokyo, 8).expect("grid");
        assert_eq!(state.cars[7].progress_mm, -12_000);
        step_race_v1(
            &mut state,
            &RaceInputFrameV1 {
                tick: 0,
                controls: vec![1; 8],
            },
        )
        .expect("tick");
        assert_eq!(state.cars[0].progress_mm, 40);
        assert_eq!(state.cars[0].speed_mm_per_tick, 40);
        assert_eq!(state.cars[0].lateral_mm, -1_800);
        assert_eq!(state.tick, 1);
    }

    #[test]
    fn invalid_inputs_leave_state_unchanged() {
        let mut state = initial_race_state_v1(RaceTrackV1::Sakura, 2).expect("grid");
        let before = state.clone();
        assert_eq!(
            step_race_v1(
                &mut state,
                &RaceInputFrameV1 {
                    tick: 0,
                    controls: vec![64, 1]
                }
            ),
            Err(RaceSimulationErrorV1::ControlBits)
        );
        assert_eq!(state, before);
        assert_eq!(
            step_race_v1(
                &mut state,
                &RaceInputFrameV1 {
                    tick: 1,
                    controls: vec![1, 1]
                }
            ),
            Err(RaceSimulationErrorV1::TickSequence)
        );
        assert_eq!(state, before);
    }

    #[test]
    fn contacts_have_deterministic_tie_direction() {
        let mut state = initial_race_state_v1(RaceTrackV1::NeonTokyo, 2).expect("grid");
        state.cars[0].lateral_mm = 0;
        state.cars[1].lateral_mm = 0;
        step_race_v1(
            &mut state,
            &RaceInputFrameV1 {
                tick: 0,
                controls: vec![1, 1],
            },
        )
        .expect("tick");
        assert_eq!(state.cars[0].lateral_mm, -900);
        assert_eq!(state.cars[1].lateral_mm, 900);
        assert_eq!(state.cars[0].speed_mm_per_tick, 0);
    }

    #[test]
    fn curvature_and_negative_division_are_exact() {
        let mut state = initial_race_state_v1(RaceTrackV1::NeonTokyo, 1).expect("grid");
        state.cars[0].progress_mm = 400_000;
        state.cars[0].speed_mm_per_tick = 1_200;
        state.cars[0].lateral_velocity_mm_per_tick = -3;
        step_race_v1(
            &mut state,
            &RaceInputFrameV1 {
                tick: 0,
                controls: vec![4],
            },
        )
        .expect("tick");
        assert_eq!(state.cars[0].lateral_velocity_mm_per_tick, -18);
        assert_eq!(state.cars[0].lateral_mm, -1_799);
    }

    #[test]
    fn tied_finishers_share_win_and_are_frozen() {
        let mut state = initial_race_state_v1(RaceTrackV1::NeonTokyo, 3).expect("grid");
        for car in &mut state.cars[..2] {
            car.progress_mm = 5_999_990;
        }
        step_race_v1(
            &mut state,
            &RaceInputFrameV1 {
                tick: 0,
                controls: vec![1, 1, 0],
            },
        )
        .expect("finish");
        assert_eq!(race_result_v1(&state).expect("result").winners, vec![0, 1]);
        let first = state.cars[0].clone();
        step_race_v1(
            &mut state,
            &RaceInputFrameV1 {
                tick: 1,
                controls: vec![63, 63, 0],
            },
        )
        .expect("continue");
        assert_eq!(state.cars[0], first);
    }
}
