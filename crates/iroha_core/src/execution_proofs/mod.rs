//! Native deterministic execution relations, transparent proofs and public replay verification.

mod integer_air;
mod proof;
pub mod race;
mod race_air;

pub use proof::{
    ExecutionProofErrorV1, RACE_MAX_PROOF_BYTES_V1, compiled_race_profile_v1, prove_race_v1,
    race_profile_id_v1, race_profile_is_qualified_v1, race_rules_hash_v1, race_state_root_v1,
    race_transcript_root_v1, verify_race_proof_for_history_v1, verify_race_proof_v1,
};

pub use race::{
    RaceSimulationErrorV1, apply_race_dnf_v1, initial_race_state_v1, race_result_v1,
    replay_race_v1, step_race_v1,
};
