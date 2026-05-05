//! Internal helpers for Kaigi privacy-mode execution.
//!
//! When the `kaigi_privacy_mocks` feature is enabled, these helpers accept
//! deterministic mock proofs so that unit and integration tests can exercise
//! privacy-mode workflows. Production builds wire into the canonical verifier
//! pipeline to validate Halo2 envelopes against the configured roster circuit.

#[cfg(not(feature = "kaigi_privacy_mocks"))]
use std::str::FromStr;

use iroha_config::parameters::actual::VerifyingKeyRef;
use iroha_crypto::Hash;
#[cfg(not(feature = "kaigi_privacy_mocks"))]
use iroha_data_model::{
    confidential::ConfidentialStatus,
    proof::{ProofBox, VerifyingKeyId},
    zk::OpenVerifyEnvelope,
};
use iroha_data_model::{
    kaigi::{KaigiParticipantCommitment, KaigiParticipantNullifier},
    prelude::AccountId,
};
#[cfg(not(feature = "kaigi_privacy_mocks"))]
use iroha_schema::Ident;
#[cfg(not(feature = "kaigi_privacy_mocks"))]
use kaigi_zk::{KAIGI_ROSTER_BACKEND, KAIGI_ROSTER_ROOT_LIMBS, roster_root_limb_values};
#[cfg(not(feature = "kaigi_privacy_mocks"))]
use mv::storage::StorageReadOnly;

use super::{Error, privacy_error};
use crate::state::StateTransaction;
#[cfg(not(feature = "kaigi_privacy_mocks"))]
use crate::zk;

/// Information supplied with a privacy-mode join/leave request.
#[derive(Debug)]
pub struct PrivacyArtifacts<'a> {
    /// Account performing the instruction (host or participant).
    pub authority: &'a AccountId,
    /// Host account responsible for the Kaigi session.
    pub host: &'a AccountId,
    /// Optional commitment provided in the instruction.
    pub commitment: Option<&'a KaigiParticipantCommitment>,
    /// Optional nullifier provided in the instruction.
    pub nullifier: Option<&'a KaigiParticipantNullifier>,
    /// Optional roster root bound into the proof.
    pub roster_root: Option<&'a Hash>,
    /// Raw proof bytes (Norito-encoded `OpenVerifyEnvelope`).
    pub proof: Option<&'a [u8]>,
}

/// Information supplied with a privacy-mode host action.
#[derive(Debug)]
pub struct HostPrivacyArtifacts<'a> {
    /// Commitment describing the private host identity.
    pub commitment: Option<&'a KaigiParticipantCommitment>,
    /// Optional nullifier supplied by the host action.
    pub nullifier: Option<&'a KaigiParticipantNullifier>,
    /// Optional roster root bound into the proof.
    pub roster_root: Option<&'a Hash>,
    /// Raw proof bytes (Norito-encoded `OpenVerifyEnvelope`).
    pub proof: Option<&'a [u8]>,
}

/// Ensure that a transparent Kaigi does not receive privacy-artifact payloads.
pub fn ensure_transparent_payload(artifacts: &PrivacyArtifacts<'_>) -> Result<(), Error> {
    if artifacts.commitment.is_some()
        || artifacts.nullifier.is_some()
        || artifacts.roster_root.is_some()
        || artifacts.proof.is_some()
    {
        return Err(privacy_error(
            "privacy artifacts are not accepted by transparent Kaigi sessions",
        ));
    }
    Ok(())
}

#[cfg(any(test, feature = "kaigi_privacy_mocks"))]
fn verify_roster_stub(artifacts: &PrivacyArtifacts<'_>, expected_root: &Hash) -> Result<(), Error> {
    let commitment = artifacts
        .commitment
        .ok_or_else(|| privacy_error("privacy mode requires commitment"))?;
    artifacts
        .nullifier
        .ok_or_else(|| privacy_error("privacy mode requires nullifier"))?;

    if commitment
        .alias_tag
        .as_deref()
        .is_some_and(|tag| tag.len() > 64)
    {
        return Err(privacy_error("commitment alias_tag exceeds 64 characters"));
    }

    if artifacts.host == artifacts.authority {
        return Err(privacy_error("host must not re-enter privacy roster"));
    }

    let proof = artifacts
        .proof
        .ok_or_else(|| privacy_error("privacy mode requires proof"))?;
    if proof.is_empty() {
        return Err(privacy_error("privacy proof payload must be non-empty"));
    }

    let Some(advertised_root) = artifacts.roster_root else {
        return Err(privacy_error("privacy mode requires roster root"));
    };
    if advertised_root != expected_root {
        return Err(privacy_error("roster root mismatch"));
    }

    Ok(())
}

#[cfg(any(test, feature = "kaigi_privacy_mocks"))]
fn verify_usage_stub(proof: Option<&[u8]>) -> Result<(), Error> {
    let proof_bytes = proof.ok_or_else(|| privacy_error("privacy mode requires proof"))?;
    if proof_bytes.is_empty() {
        return Err(privacy_error("privacy proof payload must be non-empty"));
    }
    Ok(())
}

#[cfg(any(test, feature = "kaigi_privacy_mocks"))]
fn verify_host_stub(
    artifacts: &HostPrivacyArtifacts<'_>,
    expected_root: &Hash,
    expected_commitment: Option<&KaigiParticipantCommitment>,
) -> Result<(), Error> {
    let commitment = artifacts
        .commitment
        .ok_or_else(|| privacy_error("privacy mode requires commitment"))?;
    let nullifier = artifacts
        .nullifier
        .ok_or_else(|| privacy_error("privacy mode requires nullifier"))?;

    if commitment
        .alias_tag
        .as_deref()
        .is_some_and(|tag| tag.len() > 64)
    {
        return Err(privacy_error("commitment alias_tag exceeds 64 characters"));
    }

    if let Some(expected_commitment) = expected_commitment
        && commitment.commitment != expected_commitment.commitment
    {
        return Err(privacy_error("host commitment mismatch"));
    }

    let proof = artifacts
        .proof
        .ok_or_else(|| privacy_error("privacy mode requires proof"))?;
    if proof.is_empty() {
        return Err(privacy_error("privacy proof payload must be non-empty"));
    }

    let Some(advertised_root) = artifacts.roster_root else {
        return Err(privacy_error("privacy mode requires roster root"));
    };
    if advertised_root != expected_root {
        return Err(privacy_error("roster root mismatch"));
    }

    if nullifier.digest == Hash::prehashed([0u8; Hash::LENGTH]) {
        return Err(privacy_error("privacy nullifier must be non-zero"));
    }

    Ok(())
}

pub fn verify_roster_join(
    state_transaction: &mut StateTransaction<'_, '_>,
    artifacts: &PrivacyArtifacts<'_>,
    expected_root: &Hash,
) -> Result<(), Error> {
    #[cfg(any(test, feature = "kaigi_privacy_mocks"))]
    {
        let _ = state_transaction;
        return verify_roster_stub(artifacts, expected_root);
    }

    #[cfg(not(any(test, feature = "kaigi_privacy_mocks")))]
    {
        let proof_bytes = validate_roster_artifacts(artifacts, expected_root)?;
        let vk_cfg = state_transaction.zk.kaigi_roster_join_vk.clone();
        return verify_with_config(state_transaction, proof_bytes, vk_cfg, "kaigi roster join");
    }

    #[allow(unreachable_code)]
    Err(privacy_error("kaigi privacy mode unavailable"))
}

pub fn verify_usage_commitment(
    state_transaction: &mut StateTransaction<'_, '_>,
    proof: Option<&[u8]>,
) -> Result<(), Error> {
    #[cfg(any(test, feature = "kaigi_privacy_mocks"))]
    {
        let _ = state_transaction;
        return verify_usage_stub(proof);
    }

    #[cfg(not(any(test, feature = "kaigi_privacy_mocks")))]
    {
        let proof_bytes = proof.ok_or_else(|| privacy_error("privacy mode requires proof"))?;
        if proof_bytes.is_empty() {
            return Err(privacy_error("privacy proof payload must be non-empty"));
        }
        let vk_cfg = state_transaction.zk.kaigi_usage_vk.clone();
        return verify_with_config(state_transaction, proof_bytes, vk_cfg, "kaigi usage");
    }

    #[allow(unreachable_code)]
    Err(privacy_error("kaigi privacy mode unavailable"))
}

pub fn verify_host_create(
    state_transaction: &mut StateTransaction<'_, '_>,
    artifacts: &HostPrivacyArtifacts<'_>,
    expected_root: &Hash,
) -> Result<(), Error> {
    #[cfg(any(test, feature = "kaigi_privacy_mocks"))]
    {
        let _ = state_transaction;
        return verify_host_stub(artifacts, expected_root, None);
    }

    #[cfg(not(any(test, feature = "kaigi_privacy_mocks")))]
    {
        let proof_bytes = validate_host_artifacts(artifacts, expected_root, None)?;
        let vk_cfg = state_transaction.zk.kaigi_roster_join_vk.clone();
        return verify_with_config(state_transaction, proof_bytes, vk_cfg, "kaigi host create");
    }

    #[allow(unreachable_code)]
    Err(privacy_error("kaigi privacy mode unavailable"))
}

pub fn verify_host_action(
    state_transaction: &mut StateTransaction<'_, '_>,
    artifacts: &HostPrivacyArtifacts<'_>,
    expected_root: &Hash,
    expected_commitment: &KaigiParticipantCommitment,
) -> Result<(), Error> {
    #[cfg(any(test, feature = "kaigi_privacy_mocks"))]
    {
        let _ = state_transaction;
        return verify_host_stub(artifacts, expected_root, Some(expected_commitment));
    }

    #[cfg(not(any(test, feature = "kaigi_privacy_mocks")))]
    {
        let proof_bytes =
            validate_host_artifacts(artifacts, expected_root, Some(expected_commitment))?;
        let vk_cfg = state_transaction
            .zk
            .kaigi_roster_leave_vk
            .clone()
            .or_else(|| state_transaction.zk.kaigi_roster_join_vk.clone());
        return verify_with_config(state_transaction, proof_bytes, vk_cfg, "kaigi host action");
    }

    #[allow(unreachable_code)]
    Err(privacy_error("kaigi privacy mode unavailable"))
}

#[cfg(not(feature = "kaigi_privacy_mocks"))]
#[allow(dead_code)]
fn validate_roster_artifacts<'a>(
    artifacts: &'a PrivacyArtifacts<'a>,
    expected_root: &Hash,
) -> Result<&'a [u8], Error> {
    let commitment = artifacts
        .commitment
        .ok_or_else(|| privacy_error("privacy mode requires commitment"))?;
    artifacts
        .nullifier
        .ok_or_else(|| privacy_error("privacy mode requires nullifier"))?;

    if commitment
        .alias_tag
        .as_deref()
        .is_some_and(|tag| tag.len() > 64)
    {
        return Err(privacy_error("commitment alias_tag exceeds 64 characters"));
    }

    if artifacts.host == artifacts.authority {
        return Err(privacy_error("host must not re-enter privacy roster"));
    }

    let proof_bytes = artifacts
        .proof
        .ok_or_else(|| privacy_error("privacy mode requires proof"))?;
    if proof_bytes.is_empty() {
        return Err(privacy_error("privacy proof payload must be non-empty"));
    }

    let Some(advertised_root) = artifacts.roster_root else {
        return Err(privacy_error("privacy mode requires roster root"));
    };
    if advertised_root != expected_root {
        return Err(privacy_error("roster root mismatch"));
    }

    let envelope: OpenVerifyEnvelope = norito::decode_from_bytes(proof_bytes)
        .map_err(|_| privacy_error("failed to decode privacy proof envelope"))?;
    if envelope.circuit_id == KAIGI_ROSTER_BACKEND {
        let instance_cols = crate::zk::extract_pasta_fp_instances(&envelope.proof_bytes)
            .ok_or_else(|| privacy_error("failed to parse roster privacy proof instances"))?;
        verify_roster_root_limbs(&instance_cols, expected_root)?;
    }

    Ok(proof_bytes)
}

#[cfg(not(feature = "kaigi_privacy_mocks"))]
#[allow(dead_code)]
fn validate_host_artifacts<'a>(
    artifacts: &'a HostPrivacyArtifacts<'a>,
    expected_root: &Hash,
    expected_commitment: Option<&KaigiParticipantCommitment>,
) -> Result<&'a [u8], Error> {
    let commitment = artifacts
        .commitment
        .ok_or_else(|| privacy_error("privacy mode requires commitment"))?;
    artifacts
        .nullifier
        .ok_or_else(|| privacy_error("privacy mode requires nullifier"))?;

    if commitment
        .alias_tag
        .as_deref()
        .is_some_and(|tag| tag.len() > 64)
    {
        return Err(privacy_error("commitment alias_tag exceeds 64 characters"));
    }

    if let Some(expected_commitment) = expected_commitment
        && commitment.commitment != expected_commitment.commitment
    {
        return Err(privacy_error("host commitment mismatch"));
    }

    let proof_bytes = artifacts
        .proof
        .ok_or_else(|| privacy_error("privacy mode requires proof"))?;
    if proof_bytes.is_empty() {
        return Err(privacy_error("privacy proof payload must be non-empty"));
    }

    let Some(advertised_root) = artifacts.roster_root else {
        return Err(privacy_error("privacy mode requires roster root"));
    };
    if advertised_root != expected_root {
        return Err(privacy_error("roster root mismatch"));
    }

    let envelope: OpenVerifyEnvelope = norito::decode_from_bytes(proof_bytes)
        .map_err(|_| privacy_error("failed to decode privacy proof envelope"))?;
    if envelope.circuit_id == KAIGI_ROSTER_BACKEND {
        let instance_cols = crate::zk::extract_pasta_fp_instances(&envelope.proof_bytes)
            .ok_or_else(|| privacy_error("failed to parse roster privacy proof instances"))?;
        verify_roster_root_limbs(&instance_cols, expected_root)?;
    }

    Ok(proof_bytes)
}

#[cfg(not(feature = "kaigi_privacy_mocks"))]
#[allow(clippy::needless_pass_by_value)]
#[allow(dead_code)]
fn verify_with_config(
    state_transaction: &mut StateTransaction<'_, '_>,
    proof_bytes: &[u8],
    vk_cfg: Option<VerifyingKeyRef>,
    purpose: &str,
) -> Result<(), Error> {
    let Some(vk_cfg) = vk_cfg.as_ref() else {
        return Err(privacy_error(format!("{purpose} verifier not configured")));
    };

    let backend_tag = vk_cfg.backend.clone();
    let circuit_name = vk_cfg.name.clone();
    let vk_id = VerifyingKeyId::new(backend_tag.clone(), circuit_name.clone());
    let Some(record) = state_transaction.world.verifying_keys.get(&vk_id) else {
        return Err(privacy_error(format!("{purpose} verifier not registered")));
    };

    if record.status != ConfidentialStatus::Active {
        return Err(privacy_error(format!("{purpose} verifier is not active")));
    }
    if record.gas_schedule_id.is_none() {
        return Err(privacy_error(format!(
            "{purpose} verifier missing gas schedule reference"
        )));
    }

    let record_backend = record.backend;
    let record_circuit_id = record.circuit_id.clone();
    let record_commitment = record.commitment;
    let record_key = record.key.clone();

    let envelope: OpenVerifyEnvelope = norito::decode_from_bytes(proof_bytes)
        .map_err(|_| privacy_error("failed to decode privacy proof envelope"))?;

    if record_backend != envelope.backend {
        return Err(privacy_error("privacy proof backend mismatch"));
    }
    if record_circuit_id != envelope.circuit_id {
        return Err(privacy_error("privacy proof circuit mismatch"));
    }
    if envelope.vk_hash != [0u8; 32] && envelope.vk_hash != record_commitment {
        return Err(privacy_error("privacy proof verifier commitment mismatch"));
    }

    state_transaction.register_confidential_proof(proof_bytes.len())?;

    let backend_ident = Ident::from_str(backend_tag.as_str())
        .map_err(|_| privacy_error("invalid verifier backend identifier"))?;
    let proof_box = ProofBox::new(backend_ident, proof_bytes.to_vec());
    let report = zk::verify_backend_with_timing_checked(
        backend_tag.as_str(),
        &proof_box,
        record_key.as_ref(),
        &state_transaction.zk,
    );

    #[cfg(feature = "telemetry")]
    {
        let status = if report.ok {
            iroha_data_model::proof::ProofStatus::Verified
        } else {
            iroha_data_model::proof::ProofStatus::Rejected
        };
        let latency_ms = u64::try_from(report.elapsed.as_millis()).unwrap_or(u64::MAX);
        state_transaction.telemetry.record_zk_verify(
            backend_tag.as_str(),
            status,
            proof_bytes.len(),
            latency_ms,
        );
    }

    if !report.ok {
        return Err(privacy_error("privacy proof verification failed"));
    }

    Ok(())
}

#[cfg(not(feature = "kaigi_privacy_mocks"))]
fn verify_roster_root_limbs(
    instance_cols: &[Vec<halo2_proofs::halo2curves::pasta::Fp>],
    expected_root: &Hash,
) -> Result<(), Error> {
    const OFFSET: usize = 2;
    if instance_cols.len() < OFFSET + KAIGI_ROSTER_ROOT_LIMBS {
        return Err(privacy_error(
            "privacy proof missing roster root limbs in public inputs",
        ));
    }

    let expected_limbs = roster_root_limb_values(expected_root);
    for (idx, expected) in expected_limbs.iter().enumerate() {
        let column = &instance_cols[OFFSET + idx];
        if column.len() != 1 {
            return Err(privacy_error(
                "privacy proof roster root limbs must be single-row columns",
            ));
        }
        let limb = scalar_le_u64(column[0])
            .ok_or_else(|| privacy_error("privacy proof roster root limb exceeds 64-bit range"))?;
        if limb != *expected {
            return Err(privacy_error("roster root limb mismatch"));
        }
    }
    Ok(())
}

#[cfg(not(feature = "kaigi_privacy_mocks"))]
fn scalar_le_u64(value: halo2_proofs::halo2curves::pasta::Fp) -> Option<u64> {
    use halo2_proofs::halo2curves::ff::PrimeField as _;

    let repr = value.to_repr();
    let (lo, hi) = repr.as_ref().split_at(8);
    if hi.iter().any(|&b| b != 0) {
        return None;
    }
    let mut chunk = [0u8; 8];
    chunk.copy_from_slice(lo);
    Some(u64::from_le_bytes(chunk))
}

#[cfg(all(test, not(feature = "kaigi_privacy_mocks")))]
mod tests {
    use halo2_proofs::halo2curves::pasta::Fp;
    use kaigi_zk::empty_roster_root_hash;

    use super::*;

    #[test]
    fn roster_root_limb_validation_checks_values() {
        let root = empty_roster_root_hash();
        let mut columns = vec![vec![Fp::from(1u64)], vec![Fp::from(2u64)]];
        for limb in roster_root_limb_values(&root) {
            columns.push(vec![Fp::from(limb)]);
        }
        assert!(verify_roster_root_limbs(&columns, &root).is_ok());

        let mut mismatched = columns.clone();
        mismatched[2][0] = Fp::from(999u64);
        assert!(verify_roster_root_limbs(&mismatched, &root).is_err());
    }
}
