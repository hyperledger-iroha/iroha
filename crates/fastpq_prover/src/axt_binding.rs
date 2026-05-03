use iroha_crypto::Hash;
use iroha_data_model::{
    DataSpaceId,
    fastpq::{
        FastpqOperationKind, FastpqPublicInputs, FastpqRolePermissionDelta, FastpqStateTransition,
        FastpqTransitionBatch, TRANSFER_TRANSCRIPTS_METADATA_KEY,
    },
    nexus::{AxtFastpqBinding, AxtProofEnvelope},
};
use norito::{NoritoDeserialize, NoritoSerialize, decode_from_bytes, to_bytes};
use sha2::Digest;

use crate::{
    Error, OperationKind, PublicInputs, Result, StateTransition, TransitionBatch,
    proof::{Proof, verify},
};

/// Metadata key binding the structured AXT FASTPQ payload into the proof trace.
pub const AXT_FASTPQ_BINDING_METADATA_KEY: &str = "axt_fastpq_binding";

/// Metadata key sealing a concrete FASTPQ batch to its AXT statement.
///
/// The seal is computed over the carried batch after AXT metadata has been
/// inserted and with this field removed. It prevents descriptor-only synthetic
/// batches from being accepted as AXT proof material.
pub const AXT_FASTPQ_BATCH_SEAL_METADATA_KEY: &str = "axt_fastpq_batch_seal_v1";

/// Canonical FASTPQ parameter name used by maintained AXT flows.
pub const DEFAULT_PARAMETER: &str = "fastpq-lane-balanced";
/// Maximum encoded AXT `FastPQ` batch/proof payload accepted before decoding.
const DEFAULT_MAX_AXT_FASTPQ_PAYLOAD_BYTES: usize = 1024 * 1024;
const AXT_STATEMENT_DOMAIN: &[u8] = b"fastpq:axt:statement:v1";
const AXT_BATCH_SEAL_DOMAIN: &[u8] = b"fastpq:axt:batch-seal:v1";
const ENTRY_HASH_METADATA_KEY: &str = "entry_hash";

/// `FastPQ` payload carried inside an [`AxtProofEnvelope`] proof field.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[repr(C)]
pub struct AxtFastpqProofPayload {
    /// Canonical transition batch proven by the embedded `FastPQ` proof.
    pub batch: FastpqTransitionBatch,
    /// `FastPQ` V1 proof for `batch`.
    pub proof: Proof,
}

/// Result returned after an AXT `FastPQ` envelope has been verified.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AxtVerifiedProof {
    /// Digest of the statement bound to the AXT descriptor and `FastPQ` batch.
    pub statement_digest: [u8; 32],
    /// Digest of the envelope-carried `FastPQ` proof payload.
    pub proof_digest: Hash,
}

/// Canonicalize a structured AXT FASTPQ binding before proving or verification.
///
/// # Errors
/// Returns [`Error::InvalidAxtBinding`] when required binding fields are empty,
/// malformed, or use an unsupported claim type.
pub fn canonicalize_binding(binding: &AxtFastpqBinding) -> Result<AxtFastpqBinding> {
    Ok(AxtFastpqBinding {
        parameter: normalized_parameter(&binding.parameter),
        source_dsid: binding.source_dsid,
        source_dataspace: required_string(&binding.source_dataspace, "source_dataspace")?,
        source_receipt_id: required_string(&binding.source_receipt_id, "source_receipt_id")?,
        source_tx_commitment: required_digest(
            &binding.source_tx_commitment,
            "source_tx_commitment",
        )?,
        claim_type: normalized_claim_type(&binding.claim_type)?,
        claim_digest: required_digest(&binding.claim_digest, "claim_digest")?,
        witness_commitment: required_digest(&binding.witness_commitment, "witness_commitment")?,
        policy_commitment: required_digest(&binding.policy_commitment, "policy_commitment")?,
        verified_effect_type: required_string(
            &binding.verified_effect_type,
            "verified_effect_type",
        )?,
        corridor: binding.corridor.trim().to_string(),
        verifier_id: normalized_verifier_id(&binding.verifier_id)?,
        verifier_version: normalized_verifier_version(&binding.verifier_version)?,
        target_dsids: required_target_dsids(&binding.target_dsids)?,
        effect_binding: binding.effect_binding.clone(),
    })
}

/// Encode a `FastPQ` batch/proof pair for the `proof` field of an AXT envelope.
///
/// # Errors
/// Returns [`Error::Encode`] when Norito serialization fails.
pub fn encode_axt_fastpq_payload(batch: &TransitionBatch, proof: Proof) -> Result<Vec<u8>> {
    let payload = AxtFastpqProofPayload {
        batch: transition_batch_to_model(batch),
        proof,
    };
    to_bytes(&payload).map_err(Error::Encode)
}

/// Bind an already-captured FASTPQ batch to an AXT statement.
///
/// This helper inserts the canonical AXT metadata and batch seal required by
/// [`verify_axt_proof_envelope`]. Call it only after the batch transitions and
/// public inputs have been finalized and the batch already carries the
/// execution `entry_hash` metadata matching `source_tx_commitment`; changing
/// the batch after this call invalidates the seal.
///
/// # Errors
/// Returns [`Error::InvalidAxtBinding`] when the binding is malformed or does
/// not match the batch parameter/public dataspace, and [`Error::Encode`] when
/// Norito serialization fails.
pub fn bind_axt_batch(batch: &mut TransitionBatch, binding: &AxtFastpqBinding) -> Result<()> {
    let canonical = canonicalize_binding(binding)?;
    let context = BindingContext::from_binding(&canonical)?;
    if batch.parameter != canonical.parameter {
        return Err(Error::InvalidAxtBinding {
            details: "FastPQ batch parameter does not match AXT binding".into(),
        });
    }
    if batch.public_inputs.dsid != dsid_bytes(canonical.source_dsid) {
        return Err(Error::InvalidAxtBinding {
            details: "FastPQ batch public dsid does not match AXT binding".into(),
        });
    }
    require_concrete_execution_batch(batch, &context)?;
    batch.metadata.remove(AXT_FASTPQ_BATCH_SEAL_METADATA_KEY);
    insert_binding_metadata(batch, &context)?;
    let seal = axt_batch_seal(batch, &canonical)?;
    batch
        .metadata
        .insert(AXT_FASTPQ_BATCH_SEAL_METADATA_KEY.into(), seal.to_vec());
    Ok(())
}

/// Verify an AXT envelope that carries a real `FastPQ` batch and proof payload.
///
/// # Errors
/// Returns [`Error::InvalidAxtBinding`] when the envelope is missing the structured
/// binding/payload or when the carried batch does not bind to the AXT statement. Returns
/// any `FastPQ` proof verification error for invalid proof material.
pub fn verify_axt_proof_envelope(envelope: &AxtProofEnvelope) -> Result<AxtVerifiedProof> {
    let binding = envelope
        .fastpq_binding
        .as_ref()
        .ok_or_else(|| Error::InvalidAxtBinding {
            details: "AXT proof envelope is missing fastpq_binding".into(),
        })?;
    if binding.source_dsid != envelope.dsid.as_u64() {
        return Err(Error::InvalidAxtBinding {
            details: "AXT proof envelope source_dsid does not match dsid".into(),
        });
    }
    if envelope.proof.len() > DEFAULT_MAX_AXT_FASTPQ_PAYLOAD_BYTES {
        return Err(Error::VerifierLimitExceeded {
            limit: "max_axt_fastpq_payload_bytes",
            actual: envelope.proof.len(),
            max: DEFAULT_MAX_AXT_FASTPQ_PAYLOAD_BYTES,
        });
    }
    let payload: AxtFastpqProofPayload = decode_from_bytes(&envelope.proof)
        .map_err(|source| Error::AxtProofPayloadDecode { source })?;
    let batch = transition_batch_from_model(&payload.batch);
    verify_batch_matches_binding(&batch, binding)?;
    verify(&batch, &payload.proof)?;
    Ok(AxtVerifiedProof {
        statement_digest: axt_statement_digest(envelope, binding, &payload.batch)?,
        proof_digest: Hash::new(&envelope.proof),
    })
}

/// Convert a prover batch into the shared `FastPQ` data-model representation.
#[must_use]
pub fn transition_batch_to_model(batch: &TransitionBatch) -> FastpqTransitionBatch {
    FastpqTransitionBatch {
        parameter: batch.parameter.clone(),
        public_inputs: FastpqPublicInputs {
            dsid: batch.public_inputs.dsid,
            slot: batch.public_inputs.slot,
            old_root: batch.public_inputs.old_root,
            new_root: batch.public_inputs.new_root,
            perm_root: batch.public_inputs.perm_root,
            tx_set_hash: batch.public_inputs.tx_set_hash,
        },
        transitions: batch
            .transitions
            .iter()
            .map(|transition| FastpqStateTransition {
                key: transition.key.clone(),
                pre_value: transition.pre_value.clone(),
                post_value: transition.post_value.clone(),
                operation: operation_to_model(&transition.operation),
            })
            .collect(),
        metadata: batch.metadata.clone(),
    }
}

/// Convert a shared `FastPQ` data-model batch into the prover representation.
#[must_use]
pub fn transition_batch_from_model(dto: &FastpqTransitionBatch) -> TransitionBatch {
    let mut batch = TransitionBatch::new(
        dto.parameter.clone(),
        PublicInputs {
            dsid: dto.public_inputs.dsid,
            slot: dto.public_inputs.slot,
            old_root: dto.public_inputs.old_root,
            new_root: dto.public_inputs.new_root,
            perm_root: dto.public_inputs.perm_root,
            tx_set_hash: dto.public_inputs.tx_set_hash,
        },
    );
    for transition in &dto.transitions {
        batch.push(StateTransition::new(
            transition.key.clone(),
            transition.pre_value.clone(),
            transition.post_value.clone(),
            operation_from_model(&transition.operation),
        ));
    }
    batch.metadata = dto.metadata.clone();
    batch
}

fn operation_to_model(operation: &OperationKind) -> FastpqOperationKind {
    match operation {
        OperationKind::Transfer => FastpqOperationKind::Transfer,
        OperationKind::Mint => FastpqOperationKind::Mint,
        OperationKind::Burn => FastpqOperationKind::Burn,
        OperationKind::RoleGrant {
            role_id,
            permission_id,
            epoch,
        } => FastpqOperationKind::RoleGrant(FastpqRolePermissionDelta {
            role_id: role_id.clone(),
            permission_id: permission_id.clone(),
            epoch: *epoch,
        }),
        OperationKind::RoleRevoke {
            role_id,
            permission_id,
            epoch,
        } => FastpqOperationKind::RoleRevoke(FastpqRolePermissionDelta {
            role_id: role_id.clone(),
            permission_id: permission_id.clone(),
            epoch: *epoch,
        }),
        OperationKind::MetaSet => FastpqOperationKind::MetaSet,
    }
}

fn operation_from_model(operation: &FastpqOperationKind) -> OperationKind {
    match operation {
        FastpqOperationKind::Transfer => OperationKind::Transfer,
        FastpqOperationKind::Mint => OperationKind::Mint,
        FastpqOperationKind::Burn => OperationKind::Burn,
        FastpqOperationKind::RoleGrant(delta) => OperationKind::RoleGrant {
            role_id: delta.role_id.clone(),
            permission_id: delta.permission_id.clone(),
            epoch: delta.epoch,
        },
        FastpqOperationKind::RoleRevoke(delta) => OperationKind::RoleRevoke {
            role_id: delta.role_id.clone(),
            permission_id: delta.permission_id.clone(),
            epoch: delta.epoch,
        },
        FastpqOperationKind::MetaSet => OperationKind::MetaSet,
    }
}

struct BindingContext<'a> {
    binding: &'a AxtFastpqBinding,
    source_tx_commitment: [u8; 32],
    claim_digest: [u8; 32],
    witness_commitment: [u8; 32],
    policy_commitment: [u8; 32],
    effect_type: String,
}

impl<'a> BindingContext<'a> {
    fn from_binding(binding: &'a AxtFastpqBinding) -> Result<Self> {
        Ok(Self {
            binding,
            source_tx_commitment: decode_hex_digest(
                &binding.source_tx_commitment,
                "source_tx_commitment",
            )?,
            claim_digest: decode_hex_digest(&binding.claim_digest, "claim_digest")?,
            witness_commitment: decode_hex_digest(
                &binding.witness_commitment,
                "witness_commitment",
            )?,
            policy_commitment: decode_hex_digest(&binding.policy_commitment, "policy_commitment")?,
            effect_type: required_string(&binding.verified_effect_type, "verified_effect_type")?,
        })
    }
}

fn verify_batch_matches_binding(batch: &TransitionBatch, binding: &AxtFastpqBinding) -> Result<()> {
    let canonical = canonicalize_binding(binding)?;
    let context = BindingContext::from_binding(&canonical)?;
    if batch.parameter != canonical.parameter {
        return Err(Error::InvalidAxtBinding {
            details: "FastPQ batch parameter does not match AXT binding".into(),
        });
    }
    if batch.public_inputs.dsid != dsid_bytes(canonical.source_dsid) {
        return Err(Error::InvalidAxtBinding {
            details: "FastPQ batch public dsid does not match AXT binding".into(),
        });
    }
    require_concrete_execution_batch(batch, &context)?;
    let encoded = required_metadata(batch, AXT_FASTPQ_BINDING_METADATA_KEY)?;
    let decoded: AxtFastpqBinding =
        decode_from_bytes(encoded).map_err(|source| Error::TransferMetadataDecode { source })?;
    if canonicalize_binding(&decoded)? != canonical {
        return Err(Error::InvalidAxtBinding {
            details: "FastPQ batch metadata binding does not match AXT binding".into(),
        });
    }
    require_metadata_eq(batch, "source_tx_commitment", &context.source_tx_commitment)?;
    require_metadata_eq(batch, "claim_digest", &context.claim_digest)?;
    require_metadata_eq(batch, "witness_commitment", &context.witness_commitment)?;
    require_metadata_eq(batch, "policy_commitment", &context.policy_commitment)?;
    require_metadata_eq(
        batch,
        "source_receipt_id",
        canonical.source_receipt_id.as_bytes(),
    )?;
    require_metadata_eq(
        batch,
        "target_dsids",
        &encode_target_dsids(&canonical.target_dsids),
    )?;
    require_metadata_eq(
        batch,
        "verified_effect_type",
        context.effect_type.as_bytes(),
    )?;
    if !canonical.corridor.is_empty() {
        require_metadata_eq(batch, "corridor", canonical.corridor.as_bytes())?;
    }
    let seal = axt_batch_seal(batch, &canonical)?;
    require_metadata_eq(batch, AXT_FASTPQ_BATCH_SEAL_METADATA_KEY, &seal)?;
    require_transfer_claim_witnesses(batch, canonical.claim_type.as_str())?;
    Ok(())
}

fn required_metadata<'a>(batch: &'a TransitionBatch, key: &str) -> Result<&'a [u8]> {
    batch
        .metadata
        .get(key)
        .map(Vec::as_slice)
        .ok_or_else(|| Error::MissingMetadata {
            key: key.to_string(),
        })
}

fn require_metadata_eq(batch: &TransitionBatch, key: &str, expected: &[u8]) -> Result<()> {
    let actual = required_metadata(batch, key)?;
    if actual == expected {
        Ok(())
    } else {
        Err(Error::InvalidAxtBinding {
            details: format!("FastPQ batch metadata `{key}` does not match AXT binding"),
        })
    }
}

fn require_concrete_execution_batch(
    batch: &TransitionBatch,
    context: &BindingContext<'_>,
) -> Result<()> {
    if batch.transitions.is_empty() {
        return Err(Error::InvalidAxtBinding {
            details: "AXT FastPQ batch must contain execution-captured state transitions".into(),
        });
    }
    require_metadata_eq(
        batch,
        ENTRY_HASH_METADATA_KEY,
        &context.source_tx_commitment,
    )
}

fn require_transfer_claim_witnesses(batch: &TransitionBatch, claim_type: &str) -> Result<()> {
    if matches!(claim_type, "tx_predicate" | "value_conservation") {
        let has_transfer = batch
            .transitions
            .iter()
            .any(|transition| matches!(transition.operation, OperationKind::Transfer));
        if !has_transfer {
            return Err(Error::InvalidAxtBinding {
                details: "transfer AXT claim must carry transfer transitions".into(),
            });
        }
        required_metadata(batch, TRANSFER_TRANSCRIPTS_METADATA_KEY)?;
    }
    Ok(())
}

fn axt_statement_digest(
    envelope: &AxtProofEnvelope,
    binding: &AxtFastpqBinding,
    batch: &FastpqTransitionBatch,
) -> Result<[u8; 32]> {
    let mut payload = Vec::new();
    payload.extend_from_slice(AXT_STATEMENT_DOMAIN);
    payload.extend_from_slice(&envelope.dsid.as_u64().to_le_bytes());
    payload.extend_from_slice(&envelope.manifest_root);
    if let Some(da_commitment) = envelope.da_commitment {
        payload.extend_from_slice(&da_commitment);
    }
    payload.extend_from_slice(&to_bytes(&canonicalize_binding(binding)?)?);
    payload.extend_from_slice(&to_bytes(batch)?);
    Ok(Hash::new(payload).into())
}

fn axt_batch_seal(
    batch: &TransitionBatch,
    canonical_binding: &AxtFastpqBinding,
) -> Result<[u8; 32]> {
    let mut sealed_batch = batch.clone();
    sealed_batch
        .metadata
        .remove(AXT_FASTPQ_BATCH_SEAL_METADATA_KEY);
    let mut payload = Vec::new();
    payload.extend_from_slice(AXT_BATCH_SEAL_DOMAIN);
    payload.extend_from_slice(&to_bytes(canonical_binding)?);
    payload.extend_from_slice(&to_bytes(&sealed_batch)?);
    Ok(Hash::new(payload).into())
}

fn insert_binding_metadata(
    batch: &mut TransitionBatch,
    context: &BindingContext<'_>,
) -> Result<()> {
    batch.metadata.insert(
        AXT_FASTPQ_BINDING_METADATA_KEY.into(),
        to_bytes(context.binding).map_err(Error::Encode)?,
    );
    batch.metadata.insert(
        "source_tx_commitment".into(),
        context.source_tx_commitment.to_vec(),
    );
    batch
        .metadata
        .insert("claim_digest".into(), context.claim_digest.to_vec());
    batch.metadata.insert(
        "witness_commitment".into(),
        context.witness_commitment.to_vec(),
    );
    batch.metadata.insert(
        "policy_commitment".into(),
        context.policy_commitment.to_vec(),
    );
    batch.metadata.insert(
        "source_receipt_id".into(),
        context.binding.source_receipt_id.as_bytes().to_vec(),
    );
    batch.metadata.insert(
        "target_dsids".into(),
        encode_target_dsids(&context.binding.target_dsids),
    );
    batch.metadata.insert(
        "verified_effect_type".into(),
        context.effect_type.as_bytes().to_vec(),
    );
    if !context.binding.corridor.is_empty() {
        batch.metadata.insert(
            "corridor".into(),
            context.binding.corridor.as_bytes().to_vec(),
        );
    }
    Ok(())
}

fn required_string(value: &str, field: &str) -> Result<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(Error::InvalidAxtBinding {
            details: format!("{field} must not be empty"),
        })
    } else {
        Ok(trimmed.to_string())
    }
}

fn required_digest(value: &str, field: &str) -> Result<String> {
    let trimmed = value.trim().to_lowercase();
    decode_hex_digest(&trimmed, field)?;
    Ok(trimmed)
}

fn normalized_parameter(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        DEFAULT_PARAMETER.to_string()
    } else {
        trimmed.to_string()
    }
}

fn normalized_verifier_id(value: &str) -> Result<String> {
    let trimmed = value.trim();
    let verifier_id = if trimmed.is_empty() {
        "fastpq"
    } else {
        trimmed
    };
    if verifier_id == "fastpq" {
        Ok(verifier_id.to_string())
    } else {
        Err(Error::InvalidAxtBinding {
            details: format!("unsupported AXT verifier_id: {value}"),
        })
    }
}

fn normalized_verifier_version(value: &str) -> Result<String> {
    let trimmed = value.trim();
    let verifier_version = if trimmed.is_empty() { "v1" } else { trimmed };
    if verifier_version == "v1" {
        Ok(verifier_version.to_string())
    } else {
        Err(Error::InvalidAxtBinding {
            details: format!("unsupported AXT verifier_version: {value}"),
        })
    }
}

fn required_target_dsids(values: &[u64]) -> Result<Vec<u64>> {
    if values.is_empty() {
        return Err(Error::InvalidAxtBinding {
            details: "target_dsids must not be empty".into(),
        });
    }
    for (idx, value) in values.iter().enumerate() {
        if values[..idx].contains(value) {
            return Err(Error::InvalidAxtBinding {
                details: format!("target_dsids contains duplicate value: {value}"),
            });
        }
    }
    Ok(values.to_vec())
}

fn normalized_claim_type(value: &str) -> Result<String> {
    let claim_type = value.trim().to_lowercase();
    match claim_type.as_str() {
        "authorization" | "compliance" | "tx_predicate" | "value_conservation" => Ok(claim_type),
        _ => Err(Error::InvalidAxtBinding {
            details: format!("unsupported claim_type: {value}"),
        }),
    }
}

fn dsid_bytes(source_dsid: u64) -> [u8; 16] {
    let mut output = [0_u8; 16];
    output[..8].copy_from_slice(&DataSpaceId::new(source_dsid).as_u64().to_le_bytes());
    output
}

fn encode_target_dsids(values: &[u64]) -> Vec<u8> {
    let mut output = Vec::with_capacity(values.len() * 8);
    for value in values {
        output.extend_from_slice(&value.to_le_bytes());
    }
    output
}

fn decode_hex_digest(value: &str, field: &str) -> Result<[u8; 32]> {
    let decoded = hex::decode(value).map_err(|err| Error::InvalidAxtBinding {
        details: format!("{field} is not valid hex: {err}"),
    })?;
    decoded.try_into().map_err(|_| Error::InvalidAxtBinding {
        details: format!("{field} must be exactly 32 bytes"),
    })
}

/// Deterministic manifest digest for a binding payload.
///
/// # Errors
/// Returns [`Error::InvalidAxtBinding`] when the binding is not canonical and
/// [`Error::Encode`] when Norito serialization of the canonical binding fails.
pub fn batch_manifest_sha256(binding: &AxtFastpqBinding) -> Result<String> {
    let canonical = canonicalize_binding(binding)?;
    let bytes = to_bytes(&canonical).map_err(Error::Encode)?;
    Ok(format!("{:x}", sha2::Sha256::digest(bytes)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proof::Prover;

    fn sample_binding() -> AxtFastpqBinding {
        AxtFastpqBinding {
            parameter: DEFAULT_PARAMETER.to_string(),
            source_dsid: 7,
            source_dataspace: "taira".to_string(),
            source_receipt_id: "receipt-0001".to_string(),
            source_tx_commitment:
                "1111111111111111111111111111111111111111111111111111111111111111".to_string(),
            claim_type: "authorization".to_string(),
            claim_digest: "2222222222222222222222222222222222222222222222222222222222222222"
                .to_string(),
            witness_commitment: "3333333333333333333333333333333333333333333333333333333333333333"
                .to_string(),
            policy_commitment: "4444444444444444444444444444444444444444444444444444444444444444"
                .to_string(),
            verified_effect_type: "restricted_effect".to_string(),
            corridor: "test-corridor".to_string(),
            verifier_id: "fastpq".to_string(),
            verifier_version: "v1".to_string(),
            target_dsids: vec![9],
            effect_binding: None,
        }
    }

    fn envelope_with_payload(binding: AxtFastpqBinding, proof: Vec<u8>) -> AxtProofEnvelope {
        AxtProofEnvelope {
            dsid: DataSpaceId::new(binding.source_dsid),
            manifest_root: [0x42; 32],
            da_commitment: Some([0x24; 32]),
            proof,
            fastpq_binding: Some(binding),
            committed_amount: None,
            amount_commitment: None,
        }
    }

    fn real_authorization_batch(binding: &AxtFastpqBinding) -> TransitionBatch {
        let mut batch = TransitionBatch::new(
            DEFAULT_PARAMETER,
            PublicInputs {
                dsid: dsid_bytes(binding.source_dsid),
                slot: 123,
                old_root: [0x10; 32],
                new_root: [0x20; 32],
                perm_root: [0x30; 32],
                tx_set_hash: [0x40; 32],
            },
        );
        batch.push(StateTransition::new(
            b"account/real/axt-authorized".to_vec(),
            b"pending".to_vec(),
            b"authorized".to_vec(),
            OperationKind::MetaSet,
        ));
        batch.push(StateTransition::new(
            b"role/real/axt-permission".to_vec(),
            vec![0],
            vec![1],
            OperationKind::RoleGrant {
                role_id: vec![0x11; 32],
                permission_id: vec![0x22; 32],
                epoch: 7,
            },
        ));
        batch.sort();
        let entry_hash = decode_hex_digest(&binding.source_tx_commitment, "source_tx_commitment")
            .expect("entry hash");
        batch
            .metadata
            .insert(ENTRY_HASH_METADATA_KEY.into(), entry_hash.to_vec());
        bind_axt_batch(&mut batch, binding).expect("bind AXT batch");
        batch
    }

    #[test]
    fn canonicalize_binding_rejects_non_fastpq_v1_verifier_labels() {
        let mut binding = sample_binding();
        binding.verifier_id = "halo2".to_owned();
        let err = canonicalize_binding(&binding).expect_err("wrong verifier id must fail");
        assert!(
            matches!(err, Error::InvalidAxtBinding { details } if details.contains("verifier_id"))
        );

        let mut binding = sample_binding();
        binding.verifier_version = "v2".to_owned();
        let err = canonicalize_binding(&binding).expect_err("wrong verifier version must fail");
        assert!(
            matches!(err, Error::InvalidAxtBinding { details } if details.contains("verifier_version"))
        );

        let mut binding = sample_binding();
        binding.verifier_id.clear();
        binding.verifier_version.clear();
        let canonical = canonicalize_binding(&binding).expect("empty labels default to FastPQ V1");
        assert_eq!(canonical.verifier_id, "fastpq");
        assert_eq!(canonical.verifier_version, "v1");
    }

    #[test]
    fn canonicalize_binding_rejects_empty_or_duplicate_targets() {
        let mut binding = sample_binding();
        binding.target_dsids.clear();
        let err = canonicalize_binding(&binding).expect_err("empty targets must fail");
        assert!(
            matches!(err, Error::InvalidAxtBinding { details } if details.contains("target_dsids"))
        );

        let mut binding = sample_binding();
        binding.target_dsids = vec![9, 9];
        let err = canonicalize_binding(&binding).expect_err("duplicate targets must fail");
        assert!(
            matches!(err, Error::InvalidAxtBinding { details } if details.contains("duplicate"))
        );
    }

    #[test]
    fn verify_axt_envelope_rejects_mismatched_verifier_label() {
        let good_binding = sample_binding();
        let batch = real_authorization_batch(&good_binding);
        let proof = Prover::canonical(DEFAULT_PARAMETER)
            .expect("prover")
            .prove(&batch)
            .expect("proof");
        let payload = encode_axt_fastpq_payload(&batch, proof).expect("payload");
        let mut bad_binding = good_binding;
        bad_binding.verifier_id = "synthetic".to_owned();
        let envelope = envelope_with_payload(bad_binding, payload);

        let err =
            verify_axt_proof_envelope(&envelope).expect_err("non-FastPQ verifier label must fail");
        assert!(
            matches!(err, Error::InvalidAxtBinding { details } if details.contains("verifier_id"))
        );
    }

    #[test]
    fn verify_axt_envelope_rejects_oversized_payload_before_decode() {
        let binding = sample_binding();
        let oversized = vec![0xA5; DEFAULT_MAX_AXT_FASTPQ_PAYLOAD_BYTES + 1];
        let envelope = envelope_with_payload(binding, oversized);
        let err = verify_axt_proof_envelope(&envelope).expect_err("oversized payload must fail");
        assert!(matches!(
            err,
            Error::VerifierLimitExceeded {
                limit: "max_axt_fastpq_payload_bytes",
                actual,
                max,
            } if actual == DEFAULT_MAX_AXT_FASTPQ_PAYLOAD_BYTES + 1
                && max == DEFAULT_MAX_AXT_FASTPQ_PAYLOAD_BYTES
        ));
    }

    #[test]
    fn verify_axt_envelope_accepts_embedded_batch_and_proof() {
        let binding = sample_binding();
        let batch = real_authorization_batch(&binding);
        let proof = Prover::canonical(DEFAULT_PARAMETER)
            .expect("prover")
            .prove(&batch)
            .expect("proof");
        let payload = encode_axt_fastpq_payload(&batch, proof).expect("payload");
        let envelope = envelope_with_payload(binding, payload);

        let verified = verify_axt_proof_envelope(&envelope).expect("verified AXT proof");
        assert!(verified.statement_digest.iter().any(|byte| *byte != 0));
        assert!(verified.proof_digest.as_ref().iter().any(|byte| *byte != 0));
    }

    #[test]
    fn verify_axt_envelope_rejects_raw_fastpq_proof_without_batch() {
        let binding = sample_binding();
        let batch = real_authorization_batch(&binding);
        let proof = Prover::canonical(DEFAULT_PARAMETER)
            .expect("prover")
            .prove(&batch)
            .expect("proof");
        let raw_proof = to_bytes(&proof).expect("proof bytes");
        let envelope = envelope_with_payload(binding, raw_proof);

        let err = verify_axt_proof_envelope(&envelope).expect_err("raw proof must fail");
        assert!(matches!(err, Error::AxtProofPayloadDecode { .. }));
    }

    #[test]
    fn verify_axt_envelope_rejects_batch_without_axt_binding_metadata() {
        let binding = sample_binding();
        let mut batch = real_authorization_batch(&binding);
        batch.metadata.remove(AXT_FASTPQ_BINDING_METADATA_KEY);
        let proof = Prover::canonical(DEFAULT_PARAMETER)
            .expect("prover")
            .prove(&batch)
            .expect("proof");
        let payload = encode_axt_fastpq_payload(&batch, proof).expect("payload");
        let envelope = envelope_with_payload(binding, payload);

        let err = verify_axt_proof_envelope(&envelope).expect_err("missing binding must fail");
        assert!(
            matches!(err, Error::MissingMetadata { key } if key == AXT_FASTPQ_BINDING_METADATA_KEY)
        );
    }
}
