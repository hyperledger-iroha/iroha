//! Fail-closed first-party Offline Cash V1 Halo2 verifier skeleton.
//!
//! Artifact hashing and release binding are complete here.  Semantic processed
//! key parsing, STATE circuit reconstruction, proof verification, and delayed
//! IPA decisions are intentionally not represented as successful operations.
//! Until those pieces land, every cryptographic entry point returns an error.
//!
//! `FULL_STATE_TYPED_PUBLIC_INSTANCES_REQUIRED_BEFORE_ACTIVATION`: the current
//! terminal trait supplies only a semantic digest plus proof/history bytes.
//! Activation is forbidden until that API reconstructs and passes the exact
//! typed 229-word/33-cell STATE ABI (including context, request, both parents,
//! result, link, transition, amount, scale, and history) to this backend.
//!
//! `SEND_SPLIT_STATE_RELATION_REQUIRED_BEFORE_ACTIVATION`: the receive-side
//! deterministic opening and digest relation is not proof-system readiness.
//! Activation also requires exact recursive guard/helper binding and canonical
//! send transition and semantic constraints. Deterministic send branch
//! openings alone do not authorize a transfer.

use std::{fmt, sync::Arc};

use iroha_data_model::offline::{
    OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1, OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1,
    OfflineCashArtifactBindingV1,
};

use super::{
    OfflineCashAuthenticatedVerifierArtifactsV1, OfflineCashHalo2ArtifactErrorV1,
    OfflineCashHalo2ArtifactManifestV1, OfflineCashHalo2ArtifactSourceV1, OfflineCashHalo2ParityV1,
    OfflineCashPairedProofVerifierV1, halo2_primitives::validate_offline_cash_history_v1,
};

/// First-party backend which cannot accept proofs until semantic verification exists.
pub(crate) struct OfflineCashHalo2VerifierBackendV1 {
    artifacts: OfflineCashAuthenticatedVerifierArtifactsV1,
}

impl fmt::Debug for OfflineCashHalo2VerifierBackendV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OfflineCashHalo2VerifierBackendV1")
            .field("artifacts", &self.artifacts)
            .field("verification", &"unavailable")
            .finish()
    }
}

impl OfflineCashHalo2VerifierBackendV1 {
    /// Authenticate the required Eq/Ep parameters and STATE verifier keys.
    ///
    /// This constructor does not activate proof acceptance.  It only creates a
    /// source-bound backend skeleton whose verifier methods remain fail-closed.
    pub(crate) fn from_artifact_source(
        source: Arc<dyn OfflineCashHalo2ArtifactSourceV1>,
    ) -> Result<Self, OfflineCashHalo2ArtifactErrorV1> {
        Ok(Self {
            artifacts: OfflineCashAuthenticatedVerifierArtifactsV1::load(source)?,
        })
    }

    /// Exact authenticated artifact manifest retained by this backend.
    pub(crate) const fn artifact_manifest(&self) -> &OfflineCashHalo2ArtifactManifestV1 {
        self.artifacts.manifest()
    }

    fn authenticate_call(
        &self,
        parity: OfflineCashHalo2ParityV1,
        verifying_key: OfflineCashArtifactBindingV1,
        protocol_digest: [u8; 32],
    ) -> Result<(), String> {
        self.artifacts
            .authenticate_state_verifier(parity, verifying_key, protocol_digest)
            .map_err(|error| error.to_string())
    }

    fn reject_current(
        &self,
        parity: OfflineCashHalo2ParityV1,
        verifying_key: OfflineCashArtifactBindingV1,
        protocol_digest: [u8; 32],
        semantic_digest: [u8; 32],
        proof: &[u8],
        history: &[u8],
    ) -> Result<(), String> {
        if semantic_digest == [0; 32]
            || proof.is_empty()
            || proof.len() > OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1
            || history.len() != OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1
        {
            return Err("invalid offline-cash STATE proof shape".to_owned());
        }
        validate_offline_cash_history_v1(parity, history).map_err(|error| error.to_string())?;
        self.authenticate_call(parity, verifying_key, protocol_digest)?;
        Err(OfflineCashHalo2ArtifactErrorV1::VerificationUnavailable.to_string())
    }

    fn reject_history(
        &self,
        parity: OfflineCashHalo2ParityV1,
        verifying_key: OfflineCashArtifactBindingV1,
        protocol_digest: [u8; 32],
        history: &[u8],
    ) -> Result<(), String> {
        if history.len() != OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1 {
            return Err("invalid offline-cash delayed-history shape".to_owned());
        }
        validate_offline_cash_history_v1(parity, history).map_err(|error| error.to_string())?;
        self.authenticate_call(parity, verifying_key, protocol_digest)?;
        Err(OfflineCashHalo2ArtifactErrorV1::VerificationUnavailable.to_string())
    }
}

impl super::paired_verifier_sealed::Sealed for OfflineCashHalo2VerifierBackendV1 {}

impl OfflineCashPairedProofVerifierV1 for OfflineCashHalo2VerifierBackendV1 {
    fn verify_eq_current(
        &self,
        verifying_key: OfflineCashArtifactBindingV1,
        protocol_digest: [u8; 32],
        semantic_digest: [u8; 32],
        proof: &[u8],
        history: &[u8],
    ) -> Result<(), String> {
        self.reject_current(
            OfflineCashHalo2ParityV1::Eq,
            verifying_key,
            protocol_digest,
            semantic_digest,
            proof,
            history,
        )
    }

    fn verify_ep_current(
        &self,
        verifying_key: OfflineCashArtifactBindingV1,
        protocol_digest: [u8; 32],
        semantic_digest: [u8; 32],
        proof: &[u8],
        history: &[u8],
    ) -> Result<(), String> {
        self.reject_current(
            OfflineCashHalo2ParityV1::Ep,
            verifying_key,
            protocol_digest,
            semantic_digest,
            proof,
            history,
        )
    }

    fn decide_eq_history(
        &self,
        verifying_key: OfflineCashArtifactBindingV1,
        protocol_digest: [u8; 32],
        history: &[u8],
    ) -> Result<(), String> {
        self.reject_history(
            OfflineCashHalo2ParityV1::Eq,
            verifying_key,
            protocol_digest,
            history,
        )
    }

    fn decide_ep_history(
        &self,
        verifying_key: OfflineCashArtifactBindingV1,
        protocol_digest: [u8; 32],
        history: &[u8],
    ) -> Result<(), String> {
        self.reject_history(
            OfflineCashHalo2ParityV1::Ep,
            verifying_key,
            protocol_digest,
            history,
        )
    }
}
