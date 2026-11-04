//! Compares the engine's RaceV1 port with the node's own `race.rs` and `environment.rs`, compiled
//! here from the Iroha source tree against a minimal copy of the data-model types they use.

extern crate self as iroha_data_model;

/// Minimal copy of `iroha_data_model::execution_proofs` used by the node's race sources; the
/// node's `environment.rs` and `race.rs` are compiled as its submodules.
#[path = "../../iroha_core_privacy/src/execution_proofs"]
pub mod execution_proofs {
    #![allow(missing_copy_implementations)]
    /// Maximum number of input ticks in a RaceV1 replay.
    pub const RACE_MAX_TICKS_V1: u32 = 5_400;
    /// Maximum number of cars in a RaceV1 starting grid.
    pub const RACE_MAX_PLAYERS_V1: u8 = 8;
    /// Number of laps required to finish a RaceV1 race.
    pub const RACE_LAPS_V1: u32 = 3;
    /// Bit mask of controls accepted by the RaceV1 simulation.
    pub const RACE_CONTROL_MASK_V1: u16 = 0x3f;

    /// Stock track identifiers consumed by the node's reference simulation.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
    pub enum RaceTrackV1 {
        /// Neon Tokyo stock circuit.
        NeonTokyo,
        /// Harbor stock circuit.
        Harbor,
        /// Sakura stock circuit.
        Sakura,
    }
    impl RaceTrackV1 {
        /// Length of one lap, in millimetres.
        pub const fn length_mm(self) -> i64 {
            match self {
                Self::NeonTokyo => 2_000_000,
                Self::Harbor => 2_400_000,
                Self::Sakura => 1_800_000,
            }
        }
        /// Signed curvature values for the track's twelve equal-length segments.
        pub const fn curvature(self) -> [i32; 12] {
            match self {
                Self::NeonTokyo => [0, 1, 2, 1, 0, -1, -2, -1, 0, 2, -2, 0],
                Self::Harbor => [0, -2, -2, 0, 1, 3, 1, 0, -1, -3, -1, 0],
                Self::Sakura => [0, 1, 1, 0, -2, -1, 0, 2, 3, 1, -2, 0],
            }
        }
    }
    /// One car's integer state at a simulation tick boundary.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct RaceCarStateV1 {
        /// Cumulative longitudinal progress, including the initial grid offset.
        pub progress_mm: i64,
        /// Signed lateral offset from the track centre, in millimetres.
        pub lateral_mm: i32,
        /// Longitudinal speed, in millimetres per tick.
        pub speed_mm_per_tick: i32,
        /// Signed lateral velocity, in millimetres per tick.
        pub lateral_velocity_mm_per_tick: i32,
        /// Remaining integer boost energy.
        pub boost_energy: u16,
        /// Tick at which the car finished, when present.
        pub finish_tick: Option<u32>,
        /// Tick at which the car was removed, when present.
        pub dnf_tick: Option<u32>,
    }
    /// Reference simulation state for every car on one track.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct RaceStateV1 {
        /// Number of input ticks already processed.
        pub tick: u32,
        /// Stock track being simulated.
        pub track: RaceTrackV1,
        /// Car states in participant slot order.
        pub cars: Vec<RaceCarStateV1>,
    }
    /// Control inputs for one consecutive simulation tick.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct RaceInputFrameV1 {
        /// Zero-based tick to which these controls apply.
        pub tick: u32,
        /// One control bit mask per participant slot.
        pub controls: Vec<u16>,
    }
    /// Scheduled removal of participant slots from the race.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct RaceDnfEventV1 {
        /// Simulation tick boundary at which removal applies.
        pub tick: u32,
        /// Participant slots removed at that boundary.
        pub slots: Vec<u8>,
    }
    /// Complete public replay supplied to the node reference simulation.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct RaceReplayV1 {
        /// Stock track selected for this replay.
        pub track: RaceTrackV1,
        /// Number of participant slots in each input frame.
        pub player_count: u8,
        /// Consecutive control input frames.
        pub frames: Vec<RaceInputFrameV1>,
        /// Scheduled participant removal events.
        pub dnf_events: Vec<RaceDnfEventV1>,
    }
    /// One participant's position in the reference race result.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct RaceStandingV1 {
        /// Participant slot from the original starting grid.
        pub slot: u8,
        /// Finish tick, when the participant completed the race.
        pub finish_tick: Option<u32>,
        /// Removal tick, when the participant did not finish.
        pub dnf_tick: Option<u32>,
        /// Final cumulative longitudinal progress, in millimetres.
        pub progress_mm: i64,
    }
    /// Terminal race result produced by the node reference simulation.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct RaceResultV1 {
        /// Number of simulation ticks processed.
        pub ticks: u32,
        /// Final participant standings in the reference result order.
        pub standings: Vec<RaceStandingV1>,
        /// Participant slots identified as winners by the reference rules.
        pub winners: Vec<u8>,
    }

    #[allow(dead_code, clippy::all, clippy::pedantic)]
    mod environment;
    /// Node reference race simulation, compiled directly from its source file.
    #[allow(dead_code, clippy::all, clippy::pedantic)]
    pub mod race;
}

use execution_proofs::{RaceDnfEventV1, RaceInputFrameV1, RaceReplayV1, RaceTrackV1};
use iroha_execution_stark::games::race_v1::{self, Track};

fn node_track(track: Track) -> RaceTrackV1 {
    match track {
        Track::NeonTokyo => RaceTrackV1::NeonTokyo,
        Track::Harbor => RaceTrackV1::Harbor,
        Track::Sakura => RaceTrackV1::Sakura,
    }
}

fn node_replay(replay: &race_v1::Replay, frames: usize) -> RaceReplayV1 {
    RaceReplayV1 {
        track: node_track(replay.track),
        player_count: u8::try_from(replay.players).unwrap(),
        frames: replay.frames[..frames]
            .iter()
            .enumerate()
            .map(|(tick, controls)| RaceInputFrameV1 {
                tick: u32::try_from(tick).unwrap(),
                controls: controls.clone(),
            })
            .collect(),
        dnf_events: replay
            .dnf_events
            .iter()
            .filter(|(tick, _)| *tick as usize <= frames)
            .map(|(tick, slots)| RaceDnfEventV1 {
                tick: *tick,
                slots: slots.clone(),
            })
            .collect(),
    }
}

fn same(port: &race_v1::RaceState, node: &execution_proofs::RaceStateV1) -> bool {
    port.tick == node.tick
        && port.cars.len() == node.cars.len()
        && port.cars.iter().zip(&node.cars).all(|(a, b)| {
            a.progress == b.progress_mm
                && a.lateral == i64::from(b.lateral_mm)
                && a.speed == i64::from(b.speed_mm_per_tick)
                && a.lateral_velocity == i64::from(b.lateral_velocity_mm_per_tick)
                && a.energy == i64::from(b.boost_energy)
                && a.finish_tick == b.finish_tick
                && a.dnf_tick == b.dnf_tick
        })
}

#[test]
fn port_matches_the_node_simulation_tick_by_tick() {
    let mut compared = 0usize;
    for (t, track) in [Track::NeonTokyo, Track::Harbor, Track::Sakura]
        .into_iter()
        .enumerate()
    {
        for players in 1..=8usize {
            // Unoptimized builds compare a third of the races; optimized builds compare all.
            let seeds = if cfg!(debug_assertions) {
                0..2u64
            } else {
                0..6u64
            };
            for seed in seeds {
                let throttle = [97, 92, 80, 60, 40, 20][seed as usize];
                let replay = race_v1::sample_replay(
                    track,
                    players,
                    seed * 1_000 + (t * 10 + players) as u64,
                    throttle,
                    5_400,
                );
                let states = race_v1::replay_states(&replay).unwrap();
                // Compare the whole replay plus prefixes at irregular lengths (removal boundaries).
                let mut lengths: Vec<usize> = vec![replay.frames.len()];
                lengths.extend((1..replay.frames.len()).step_by(997));
                for frames in lengths {
                    let node_state =
                        execution_proofs::race::replay_race_v1(&node_replay(&replay, frames))
                            .unwrap();
                    let port = if frames == replay.frames.len() {
                        states[frames].clone()
                    } else {
                        let mut prefix = replay.clone();
                        prefix.frames.truncate(frames);
                        prefix
                            .dnf_events
                            .retain(|(tick, _)| *tick as usize <= frames);
                        race_v1::replay_states(&prefix).unwrap().pop().unwrap()
                    };
                    assert!(
                        same(&port, &node_state),
                        "{track:?} x{players} seed {seed} at {frames}: {port:?} vs {node_state:?}"
                    );
                    compared += 1;
                }
                // The node's terminal predicate agrees with the port's.
                let node_final = execution_proofs::race::replay_race_v1(&node_replay(
                    &replay,
                    replay.frames.len(),
                ))
                .unwrap();
                let result = execution_proofs::race::race_result_v1(&node_final).unwrap();
                assert_eq!(result.ticks as usize, replay.frames.len());
            }
        }
    }
    assert!(compared > if cfg!(debug_assertions) { 150 } else { 500 });
}

#[test]
fn port_rejects_what_the_node_rejects() {
    let replay = race_v1::sample_replay(Track::Harbor, 3, 5, 90, 60);
    let mut bad = replay.clone();
    bad.frames[10][1] |= 0x40;
    assert!(execution_proofs::race::replay_race_v1(&node_replay(&bad, bad.frames.len())).is_err());
    assert!(race_v1::replay_states(&bad).is_err());
    assert!(race_v1::validate_replay(&bad).is_err());
}
