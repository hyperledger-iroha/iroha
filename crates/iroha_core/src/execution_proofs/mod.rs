//! Native deterministic execution relations, transparent proofs and public replay verification.

mod checked_integer_air;
/// Upgraded racing reference computation; its proof profile is not registered.
pub mod classed_race_v1;
mod environment;
mod environment_air;
mod error;
mod integer_air;
mod kernel_export;
mod poseidon2;
mod proof;
pub mod race;
mod race_air;
mod registry;
mod staged_race_air;
mod stark;

pub use error::ExecutionProofErrorV1;
pub use proof::{
    RACE_MAX_PROOF_BYTES_V1, RACE_MAX_STARK_BYTES_V1, compiled_race_profile_v1, prove_race_v1,
    race_game_transcript_v1, race_profile_id_v1, race_profile_is_qualified_v1, race_rules_hash_v1,
    race_state_root_v1, race_transcript_root_v1, verify_race_proof_v1,
};
pub use registry::{
    compiled_execution_profile_v1, compiled_execution_profiles_v1, game_resource_requirements_v1,
    initial_game_state_root_v1, validate_game_input_v1, validate_game_manifest_v1,
    validate_game_participant_v1, verify_execution_proof_v1, verify_game_proof_for_history_v1,
};

pub use race::{
    RaceSimulationErrorV1, apply_race_dnf_v1, initial_race_state_v1, race_result_v1,
    replay_race_v1, step_race_v1,
};

pub use kernel_export::{export_race_kernels_json_v1, export_race_parity_json_v1};
