//! Touring S1 reference, native execution proof and authenticated-history adapter.
//!
//! This application computation is not installed in the execution registry or qualified
//! for ledger settlement. Callers must authenticate retained consensus state; standalone
//! verification does not establish NFT eligibility, custody or finalized inclusion.

mod admission;
mod environment;
mod environment_air;
mod history;
mod kernel_export;
mod proof;
mod race_air;
mod reference;
mod rules;
mod staged_race_air;

pub use admission::{
    ClassedRaceParametersV1, ClassedRaceParticipantDataV1, classed_race_equipment_role_v1,
    classed_race_game_transcript_v1, classed_race_rules_hash_v1, classed_race_state_root_v1,
    classed_race_transcript_root_v1,
};
pub use history::{classed_race_dispute_root_v1, verify_classed_race_proof_for_session_v1};
pub use kernel_export::{export_classed_race_kernels_json_v1, export_classed_race_parity_json_v1};
pub use proof::{
    CLASSED_RACE_MAX_PROOF_BYTES_V1, CLASSED_RACE_MAX_STARK_BYTES_V1, ClassedRaceProofPayloadV1,
    ClassedRaceProverRequestV1, classed_race_profile_id_v1, prove_classed_race_v1,
    verify_classed_race_proof_v1,
};
pub use reference::{
    ClassedRaceSimulationErrorV1, apply_classed_race_dnf_v1, classed_race_is_terminal_v1,
    classed_race_result_v1, initial_classed_race_state_v1, replay_classed_race_v1,
    step_classed_race_v1,
};
pub use rules::{
    CLASS_RULES_V1, ClassPerformanceV1, class_performance_v1, classed_track_curvature_v1,
    classed_track_length_v1,
};

#[cfg(test)]
mod tests;
