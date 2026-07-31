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
    zk::{BackendTag, OpenVerifyEnvelope},
};
use iroha_data_model::{
    kaigi::{KaigiParticipantCommitment, KaigiParticipantNullifier},
    prelude::AccountId,
};
#[cfg(not(feature = "kaigi_privacy_mocks"))]
use iroha_schema::Ident;
#[cfg(not(feature = "kaigi_privacy_mocks"))]
use kaigi_zk::{
    KAIGI_ROSTER_BACKEND, KAIGI_ROSTER_ROOT_LIMBS, roster_root_limb_values, scalar_from_hash,
};
#[cfg(not(feature = "kaigi_privacy_mocks"))]
use mv::storage::StorageReadOnly;

use super::{Error, privacy_error};
use crate::state::StateTransaction;
#[cfg(not(feature = "kaigi_privacy_mocks"))]
use crate::zk;

/// Information supplied with a privacy-mode join/leave request.
#[derive(Debug)]
pub struct PrivacyArtifacts<'a> {
    /// Roster subject represented by the proof or signed instruction.
    pub subject: &'a AccountId,
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

    if artifacts.host == artifacts.subject {
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

    if artifacts.host == artifacts.subject {
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

    let envelope = decode_privacy_proof_envelope(proof_bytes)?;
    if envelope.circuit_id != KAIGI_ROSTER_BACKEND {
        return Err(privacy_error(
            "privacy roster proof must use the canonical Kaigi roster circuit",
        ));
    }
    let instance_cols = crate::zk::extract_pasta_fp_instances(&envelope.proof_bytes)
        .ok_or_else(|| privacy_error("failed to parse roster privacy proof instances"))?;
    verify_roster_public_inputs(
        &instance_cols,
        expected_root,
        &commitment.commitment,
        &nullifier.digest,
    )?;

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

    let envelope = decode_privacy_proof_envelope(proof_bytes)?;
    if envelope.circuit_id != KAIGI_ROSTER_BACKEND {
        return Err(privacy_error(
            "privacy roster proof must use the canonical Kaigi roster circuit",
        ));
    }
    let instance_cols = crate::zk::extract_pasta_fp_instances(&envelope.proof_bytes)
        .ok_or_else(|| privacy_error("failed to parse roster privacy proof instances"))?;
    verify_roster_public_inputs(
        &instance_cols,
        expected_root,
        &commitment.commitment,
        &nullifier.digest,
    )?;

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

    let envelope = decode_privacy_proof_envelope(proof_bytes)?;

    validate_privacy_proof_envelope_metadata(
        &envelope,
        backend_tag.as_str(),
        record_backend,
        &record_circuit_id,
        record_commitment,
    )?;

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
fn decode_privacy_proof_envelope(proof_bytes: &[u8]) -> Result<OpenVerifyEnvelope, Error> {
    norito::decode_canonical(proof_bytes).map_err(|err| {
        privacy_error(format!(
            "failed to decode canonical privacy proof envelope: {err}"
        ))
    })
}

#[cfg(not(feature = "kaigi_privacy_mocks"))]
fn validate_privacy_proof_envelope_metadata(
    envelope: &OpenVerifyEnvelope,
    configured_backend: &str,
    record_backend: BackendTag,
    record_circuit_id: &str,
    record_commitment: [u8; Hash::LENGTH],
) -> Result<(), Error> {
    let Some(expected_backend) = zk::verifier_backend_registry_tag_v1(configured_backend) else {
        return Err(privacy_error(
            "privacy proof verifier backend is not admitted by the native verifier registry",
        ));
    };
    if expected_backend != record_backend {
        return Err(privacy_error("privacy verifier backend tag mismatch"));
    }
    if record_backend != envelope.backend {
        return Err(privacy_error("privacy proof backend mismatch"));
    }
    if record_circuit_id != envelope.circuit_id.as_str() {
        return Err(privacy_error("privacy proof circuit mismatch"));
    }
    if !envelope.aux.is_empty() {
        return Err(privacy_error(
            "privacy proof envelope auxiliary bytes must be empty",
        ));
    }
    if envelope.vk_hash == [0u8; Hash::LENGTH] {
        return Err(privacy_error(
            "privacy proof verifier-key hash must be non-zero",
        ));
    }
    if envelope.vk_hash != record_commitment {
        return Err(privacy_error("privacy proof verifier commitment mismatch"));
    }
    Ok(())
}

#[cfg(not(feature = "kaigi_privacy_mocks"))]
fn verify_roster_public_inputs(
    instance_cols: &[Vec<halo2_proofs::halo2curves::pasta::Fp>],
    expected_root: &Hash,
    expected_commitment: &Hash,
    expected_nullifier: &Hash,
) -> Result<(), Error> {
    const OFFSET: usize = 2;
    if instance_cols.len() != OFFSET + KAIGI_ROSTER_ROOT_LIMBS {
        return Err(privacy_error(
            "privacy proof must expose exactly commitment, nullifier, and four roster root limbs",
        ));
    }

    verify_hash_public_input(&instance_cols[0], expected_commitment, "commitment")?;
    verify_hash_public_input(&instance_cols[1], expected_nullifier, "nullifier")?;

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
fn verify_hash_public_input(
    column: &[halo2_proofs::halo2curves::pasta::Fp],
    expected: &Hash,
    label: &str,
) -> Result<(), Error> {
    if column.len() != 1 {
        return Err(privacy_error(format!(
            "privacy proof {label} must be a single-row public input"
        )));
    }
    let expected_scalar = scalar_from_hash(expected).ok_or_else(|| {
        privacy_error(format!(
            "privacy proof {label} is not a canonical Pasta scalar"
        ))
    })?;
    if column[0] != expected_scalar {
        return Err(privacy_error(format!(
            "privacy proof {label} does not match the instruction artifact"
        )));
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
    use kaigi_zk::{compute_commitment_hash, compute_nullifier_hash, empty_roster_root_hash};

    use super::*;

    #[test]
    fn roster_public_input_validation_binds_every_instruction_artifact() {
        let root = empty_roster_root_hash();
        let commitment = compute_commitment_hash(11, 31);
        let nullifier = compute_nullifier_hash(11, 57);
        let mut columns = vec![
            vec![scalar_from_hash(&commitment).expect("canonical commitment")],
            vec![scalar_from_hash(&nullifier).expect("canonical nullifier")],
        ];
        for limb in roster_root_limb_values(&root) {
            columns.push(vec![Fp::from(limb)]);
        }
        assert!(verify_roster_public_inputs(&columns, &root, &commitment, &nullifier).is_ok());

        let mut wrong_commitment = columns.clone();
        wrong_commitment[0][0] += Fp::from(1u64);
        assert!(
            verify_roster_public_inputs(&wrong_commitment, &root, &commitment, &nullifier).is_err()
        );

        let mut wrong_nullifier = columns.clone();
        wrong_nullifier[1][0] += Fp::from(1u64);
        assert!(
            verify_roster_public_inputs(&wrong_nullifier, &root, &commitment, &nullifier).is_err()
        );

        let mut wrong_root = columns.clone();
        wrong_root[2][0] = Fp::from(999u64);
        assert!(verify_roster_public_inputs(&wrong_root, &root, &commitment, &nullifier).is_err());

        let mut extra_column = columns;
        extra_column.push(vec![Fp::from(0u64)]);
        assert!(
            verify_roster_public_inputs(&extra_column, &root, &commitment, &nullifier).is_err()
        );
    }

    #[test]
    fn privacy_proof_envelope_metadata_rejects_zero_verifier_hash() {
        let commitment = Hash::new(b"kaigi-privacy-verifier-key");
        let commitment: [u8; Hash::LENGTH] = commitment.into();
        let mut envelope = OpenVerifyEnvelope {
            backend: BackendTag::Halo2IpaPasta,
            circuit_id: "kaigi/roster".to_owned(),
            vk_hash: commitment,
            public_inputs: Vec::new(),
            proof_bytes: Vec::new(),
            aux: Vec::new(),
        };
        assert!(
            validate_privacy_proof_envelope_metadata(
                &envelope,
                "halo2/pasta/kaigi-roster-v1",
                BackendTag::Halo2IpaPasta,
                "kaigi/roster",
                commitment,
            )
            .is_ok()
        );

        let err = validate_privacy_proof_envelope_metadata(
            &envelope,
            "halo2/ipa:production-ready",
            BackendTag::Halo2IpaPasta,
            "kaigi/roster",
            commitment,
        )
        .expect_err("readiness-claim verifier backend must reject");
        let Error::InvalidParameter(
            iroha_data_model::isi::error::InvalidParameterError::SmartContract(message),
        ) = err
        else {
            panic!("unexpected readiness-claim backend rejection: {err:?}");
        };
        assert!(
            message.contains("native verifier registry"),
            "unexpected error: {message}"
        );

        let err = validate_privacy_proof_envelope_metadata(
            &envelope,
            "stark/fri/sha256-goldilocks",
            BackendTag::Halo2IpaPasta,
            "kaigi/roster",
            commitment,
        )
        .expect_err("configured backend tag drift must reject");
        let Error::InvalidParameter(
            iroha_data_model::isi::error::InvalidParameterError::SmartContract(message),
        ) = err
        else {
            panic!("unexpected backend tag mismatch rejection: {err:?}");
        };
        assert!(
            message.contains("backend tag mismatch"),
            "unexpected error: {message}"
        );

        envelope.vk_hash = [0u8; Hash::LENGTH];
        let err = validate_privacy_proof_envelope_metadata(
            &envelope,
            "halo2/pasta/kaigi-roster-v1",
            BackendTag::Halo2IpaPasta,
            "kaigi/roster",
            commitment,
        )
        .expect_err("zero verifier-key hash must reject");
        let Error::InvalidParameter(
            iroha_data_model::isi::error::InvalidParameterError::SmartContract(message),
        ) = err
        else {
            panic!("unexpected zero-hash rejection: {err:?}");
        };
        assert!(message.contains("non-zero"), "unexpected error: {message}");
    }

    #[test]
    fn privacy_proof_admission_rejects_alternate_norito_layout() {
        let envelope = OpenVerifyEnvelope {
            backend: BackendTag::Halo2IpaPasta,
            circuit_id: "kaigi/roster".to_owned(),
            vk_hash: [0xA5; Hash::LENGTH],
            public_inputs: vec![0x11; 32],
            proof_bytes: vec![0x22; 64],
            aux: Vec::new(),
        };
        let canonical =
            norito::encode_canonical(&envelope).expect("encode canonical privacy envelope");
        assert_eq!(
            decode_privacy_proof_envelope(&canonical)
                .expect("canonical privacy envelope must decode"),
            envelope
        );

        let alternate_flags =
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        let alternate = {
            let _guard = norito::core::DecodeFlagsGuard::enter(alternate_flags);
            norito::to_bytes(&envelope).expect("encode alternate-layout privacy envelope")
        };
        assert_ne!(alternate, canonical);
        let err = decode_privacy_proof_envelope(&alternate)
            .expect_err("alternate-layout privacy envelope must reject");
        let Error::InvalidParameter(
            iroha_data_model::isi::error::InvalidParameterError::SmartContract(message),
        ) = err
        else {
            panic!("unexpected alternate-layout rejection: {err:?}");
        };
        assert!(
            message.contains("canonical privacy proof envelope"),
            "unexpected error: {message}"
        );
    }
}
