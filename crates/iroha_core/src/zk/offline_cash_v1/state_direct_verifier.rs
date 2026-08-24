//! Direct verification candidate for exact typed Offline Cash V1 STATE claims.
//!
//! This module performs the native current-proof check and its delayed IPA
//! decision against source-authenticated artifacts. It deliberately returns no
//! receipt or capability and remains disconnected from the fail-closed terminal
//! backend while recursive parents and helper relations are incomplete.

use core::fmt;

use halo2_proofs::halo2curves::pasta::{EpAffine, EqAffine, Fp, Fq};
use iroha_data_model::offline::{OFFLINE_CASH_HALO2_K_V1, OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1};

use super::{
    OfflineCashAuthenticatedVerifierArtifactsV1, OfflineCashHalo2ArtifactErrorV1,
    OfflineCashHalo2ParityV1,
    artifacts::OfflineCashAuthenticatedArtifactParseErrorV1,
    halo2_primitives::{
        OfflineCashHalo2PrimitiveErrorV1, OfflineCashIpaHistoryV1, decide_ep_history_v1,
        decide_eq_history_v1, parse_offline_cash_ep_params_v1, parse_offline_cash_eq_params_v1,
        parse_processed_verifier_key_v1, verify_augmented_ipa_proof_v1,
    },
    state_abi::OfflineCashStatePublicInstancesV1,
    state_circuit::{OfflineCashEpStateCircuitV1, OfflineCashEqStateCircuitV1},
};

const FOLDED_GENERATOR_BYTES: usize = 32;

/// Exact direct-verification stage that rejected a typed STATE claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OfflineCashDirectStateVerifierStageV1 {
    /// Typed public history reconstruction.
    PublicInstances,
    /// Transparent parity parameters.
    Parameters,
    /// Processed STATE verifying key.
    VerifyingKey,
    /// Current augmented Halo2 proof.
    CurrentProof,
    /// Delayed IPA generator decision.
    DelayedHistory,
}

/// Fail-closed direct STATE candidate error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OfflineCashDirectStateVerifierErrorV1 {
    /// The augmented transcript is empty, suffix-only, or above the wire cap.
    InvalidProofShape,
    /// A governed artifact failed source authentication.
    Artifact(OfflineCashHalo2ArtifactErrorV1),
    /// A typed parser or native verification primitive rejected the claim.
    Primitive {
        /// Stage that rejected the claim.
        stage: OfflineCashDirectStateVerifierStageV1,
        /// Exact primitive diagnostic.
        error: OfflineCashHalo2PrimitiveErrorV1,
    },
}

impl fmt::Display for OfflineCashDirectStateVerifierErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidProofShape => {
                formatter.write_str("invalid offline-cash direct STATE proof shape")
            }
            Self::Artifact(error) => write!(formatter, "{error}"),
            Self::Primitive { stage, error } => {
                write!(
                    formatter,
                    "offline-cash direct STATE {stage:?} failed: {error}"
                )
            }
        }
    }
}

impl std::error::Error for OfflineCashDirectStateVerifierErrorV1 {}

fn map_artifact_parse_error(
    stage: OfflineCashDirectStateVerifierStageV1,
    error: OfflineCashAuthenticatedArtifactParseErrorV1<OfflineCashHalo2PrimitiveErrorV1>,
) -> OfflineCashDirectStateVerifierErrorV1 {
    match error {
        OfflineCashAuthenticatedArtifactParseErrorV1::Authentication(error) => {
            OfflineCashDirectStateVerifierErrorV1::Artifact(error)
        }
        OfflineCashAuthenticatedArtifactParseErrorV1::Parser(error) => {
            OfflineCashDirectStateVerifierErrorV1::Primitive { stage, error }
        }
    }
}

/// Verify one exact typed STATE proof and decide the history embedded in it.
///
/// Artifact roles are derived only from the instance parity. The full 229-word
/// value supplies the sole public-instance column and the sole delayed history;
/// callers cannot substitute a semantic digest or a second raw history slice.
/// Success is intentionally only `()` and grants no terminal authority.
pub(super) fn verify_and_decide_state_candidate_v1(
    artifacts: &OfflineCashAuthenticatedVerifierArtifactsV1,
    instances: &OfflineCashStatePublicInstancesV1,
    augmented_proof: &[u8],
) -> Result<(), OfflineCashDirectStateVerifierErrorV1> {
    if augmented_proof.len() <= FOLDED_GENERATOR_BYTES
        || augmented_proof.len() > OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1
    {
        return Err(OfflineCashDirectStateVerifierErrorV1::InvalidProofShape);
    }

    match instances.parity() {
        OfflineCashHalo2ParityV1::Eq => {
            let history_bytes = instances.history_bytes();
            let history =
                OfflineCashIpaHistoryV1::<EqAffine>::parse(&history_bytes).map_err(|error| {
                    OfflineCashDirectStateVerifierErrorV1::Primitive {
                        stage: OfflineCashDirectStateVerifierStageV1::PublicInstances,
                        error,
                    }
                })?;
            let params = artifacts
                .with_authenticated_state_params(
                    OfflineCashHalo2ParityV1::Eq,
                    parse_offline_cash_eq_params_v1,
                )
                .map_err(|error| {
                    map_artifact_parse_error(
                        OfflineCashDirectStateVerifierStageV1::Parameters,
                        error,
                    )
                })?;
            let verifying_key = artifacts
                .with_authenticated_state_verifying_key(OfflineCashHalo2ParityV1::Eq, |bytes| {
                    parse_processed_verifier_key_v1::<EqAffine, OfflineCashEqStateCircuitV1>(
                        bytes,
                        OFFLINE_CASH_HALO2_K_V1,
                    )
                })
                .map_err(|error| {
                    map_artifact_parse_error(
                        OfflineCashDirectStateVerifierStageV1::VerifyingKey,
                        error,
                    )
                })?;
            let instance_column = instances.field_instances::<Fp>();
            let instance_columns: [&[Fp]; 1] = [&instance_column];
            let proof_instances: [&[&[Fp]]; 1] = [&instance_columns];
            verify_augmented_ipa_proof_v1(
                &params,
                &verifying_key,
                augmented_proof,
                &proof_instances,
                &history,
            )
            .map_err(|error| OfflineCashDirectStateVerifierErrorV1::Primitive {
                stage: OfflineCashDirectStateVerifierStageV1::CurrentProof,
                error,
            })?;
            decide_eq_history_v1(&params, &history).map_err(|error| {
                OfflineCashDirectStateVerifierErrorV1::Primitive {
                    stage: OfflineCashDirectStateVerifierStageV1::DelayedHistory,
                    error,
                }
            })
        }
        OfflineCashHalo2ParityV1::Ep => {
            let history_bytes = instances.history_bytes();
            let history =
                OfflineCashIpaHistoryV1::<EpAffine>::parse(&history_bytes).map_err(|error| {
                    OfflineCashDirectStateVerifierErrorV1::Primitive {
                        stage: OfflineCashDirectStateVerifierStageV1::PublicInstances,
                        error,
                    }
                })?;
            let params = artifacts
                .with_authenticated_state_params(
                    OfflineCashHalo2ParityV1::Ep,
                    parse_offline_cash_ep_params_v1,
                )
                .map_err(|error| {
                    map_artifact_parse_error(
                        OfflineCashDirectStateVerifierStageV1::Parameters,
                        error,
                    )
                })?;
            let verifying_key = artifacts
                .with_authenticated_state_verifying_key(OfflineCashHalo2ParityV1::Ep, |bytes| {
                    parse_processed_verifier_key_v1::<EpAffine, OfflineCashEpStateCircuitV1>(
                        bytes,
                        OFFLINE_CASH_HALO2_K_V1,
                    )
                })
                .map_err(|error| {
                    map_artifact_parse_error(
                        OfflineCashDirectStateVerifierStageV1::VerifyingKey,
                        error,
                    )
                })?;
            let instance_column = instances.field_instances::<Fq>();
            let instance_columns: [&[Fq]; 1] = [&instance_column];
            let proof_instances: [&[&[Fq]]; 1] = [&instance_columns];
            verify_augmented_ipa_proof_v1(
                &params,
                &verifying_key,
                augmented_proof,
                &proof_instances,
                &history,
            )
            .map_err(|error| OfflineCashDirectStateVerifierErrorV1::Primitive {
                stage: OfflineCashDirectStateVerifierStageV1::CurrentProof,
                error,
            })?;
            decide_ep_history_v1(&params, &history).map_err(|error| {
                OfflineCashDirectStateVerifierErrorV1::Primitive {
                    stage: OfflineCashDirectStateVerifierStageV1::DelayedHistory,
                    error,
                }
            })
        }
    }
}
