use crate::{
    Error, OperationKind, PublicInputs, Result, StateTransition, TransitionBatch,
    gadgets::transfer::decode_transcripts,
    proof::{Proof, verify},
};
use iroha_crypto::Hash;
use iroha_data_model::{
    DataSpaceId,
    fastpq::{
        FastpqOperationKind, FastpqPublicInputs, FastpqRolePermissionDelta, FastpqStateTransition,
        FastpqTransitionBatch, TRANSFER_TRANSCRIPTS_METADATA_KEY,
    },
    nexus::{AxtEffectBinding, AxtFastpqBinding, AxtProofEnvelope, ProofBlob},
};
use norito::{NoritoDeserialize, NoritoSerialize, decode_from_bytes, to_bytes};
use sha2::Digest;
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
    /// Proven pre-execution state root.
    pub old_root: [u8; 32],
    /// Proven post-execution state root.
    pub new_root: [u8; 32],
    /// Proven transaction/statement-set commitment.
    pub tx_set_hash: [u8; 32],
}
/// Canonicalize a structured AXT FASTPQ binding before proving or verification.
///
/// # Errors
/// Returns [`Error::InvalidAxtBinding`] when required binding fields are empty,
/// malformed, or use an unsupported claim type.
pub fn canonicalize_binding(binding: &AxtFastpqBinding) -> Result<AxtFastpqBinding> {
    Ok(AxtFastpqBinding {
        parameter: normalized_parameter(&binding.parameter)?,
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
        effect_binding: binding
            .effect_binding
            .as_ref()
            .map(canonicalize_effect_binding)
            .transpose()?,
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
    encode_canonical_norito(&payload)
}
/// Decode the canonical AXT binding already embedded in a `FastPQ` batch.
///
/// This helper never mutates the batch. It is intended for export paths that need to package proof
/// material after the batch has already been bound before proof generation.
///
/// # Errors
/// Returns [`Error::MissingMetadata`] when the batch carries no AXT binding and
/// [`Error::InvalidAxtBinding`] when the embedded binding does not match the
/// concrete batch metadata.
pub fn embedded_axt_binding(batch: &TransitionBatch) -> Result<AxtFastpqBinding> {
    let encoded = required_metadata(batch, AXT_FASTPQ_BINDING_METADATA_KEY)?;
    let binding = decode_canonical_binding(encoded)?;
    verify_batch_matches_binding(batch, &binding)?;
    Ok(binding)
}
/// Build an AXT proof envelope from an already AXT-bound batch and proof.
///
/// The batch must already contain canonical AXT metadata and the batch seal created before proof
/// generation. This helper does not add or repair AXT binding metadata after the fact.
///
/// # Errors
/// Returns an error when the embedded binding is missing/malformed, does not
/// match the batch, or the proof payload cannot be encoded.
pub fn axt_proof_envelope_from_bound_batch(
    batch: &TransitionBatch,
    proof: Proof,
    manifest_root: [u8; 32],
    da_commitment: Option<[u8; 32]>,
) -> Result<AxtProofEnvelope> {
    let binding = embedded_axt_binding(batch)?;
    verify(batch, &proof)?;
    Ok(AxtProofEnvelope {
        dsid: DataSpaceId::new(binding.source_dsid),
        manifest_root,
        da_commitment,
        proof: encode_axt_fastpq_payload(batch, proof)?,
        fastpq_binding: Some(binding),
        committed_amount: None,
        amount_commitment: None,
    })
}
/// Build an AXT proof blob from an already AXT-bound batch and proof.
///
/// # Errors
/// Returns an error when envelope construction or Norito encoding fails.
pub fn axt_proof_blob_from_bound_batch(
    batch: &TransitionBatch,
    proof: Proof,
    manifest_root: [u8; 32],
    da_commitment: Option<[u8; 32]>,
    expiry_slot: Option<u64>,
) -> Result<ProofBlob> {
    let envelope = axt_proof_envelope_from_bound_batch(batch, proof, manifest_root, da_commitment)?;
    Ok(ProofBlob {
        payload: encode_canonical_norito(&envelope)?,
        expiry_slot,
    })
}
/// Bind an already-captured FASTPQ batch to an AXT statement.
///
/// This helper inserts the canonical AXT metadata and batch seal required by
/// [`verify_axt_proof_envelope`]. Call it only after the batch transitions and public inputs have
/// been finalized and the batch already carries the execution `entry_hash` metadata matching
/// `source_tx_commitment`; changing the batch after this call invalidates the seal.
///
/// # Errors
/// Returns [`Error::InvalidAxtBinding`] when the binding is malformed or does not match the batch
/// parameter/public dataspace, and [`Error::Encode`] when Norito serialization fails.
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
    require_canonical_binding(binding)?;
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
    let payload = decode_canonical_axt_fastpq_payload(&envelope.proof)?;
    let batch = transition_batch_from_model(&payload.batch);
    verify_batch_matches_binding(&batch, binding)?;
    verify(&batch, &payload.proof)?;
    Ok(AxtVerifiedProof {
        statement_digest: axt_statement_digest(envelope, binding, &payload.batch)?,
        proof_digest: Hash::new(&envelope.proof),
        old_root: batch.public_inputs.old_root,
        new_root: batch.public_inputs.new_root,
        tx_set_hash: batch.public_inputs.tx_set_hash,
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
    let canonical = require_canonical_binding(binding)?;
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
    let decoded = decode_canonical_binding(encoded)?;
    if decoded != canonical {
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
    require_transfer_claim_witnesses(batch, &context, canonical.claim_type.as_str())?;
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
fn require_transfer_claim_witnesses(
    batch: &TransitionBatch,
    context: &BindingContext<'_>,
    claim_type: &str,
) -> Result<()> {
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
        let transcripts =
            decode_transcripts(&batch.metadata)?.ok_or_else(|| Error::MissingMetadata {
                key: TRANSFER_TRANSCRIPTS_METADATA_KEY.to_owned(),
            })?;
        if transcripts.is_empty() {
            return Err(Error::InvalidAxtBinding {
                details: "transfer AXT claim must carry at least one transfer transcript".into(),
            });
        }
        if transcripts.iter().any(|transcript| {
            transcript.batch_hash.as_ref() != context.source_tx_commitment.as_slice()
        }) {
            return Err(Error::InvalidAxtBinding {
                details: "transfer transcript batch_hash does not match source_tx_commitment"
                    .into(),
            });
        }
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
    payload.extend_from_slice(&encode_canonical_norito(&canonicalize_binding(binding)?)?);
    payload.extend_from_slice(&encode_canonical_norito(batch)?);
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
    payload.extend_from_slice(&encode_canonical_norito(canonical_binding)?);
    payload.extend_from_slice(&encode_canonical_norito(&sealed_batch)?);
    Ok(Hash::new(payload).into())
}
fn insert_binding_metadata(
    batch: &mut TransitionBatch,
    context: &BindingContext<'_>,
) -> Result<()> {
    batch.metadata.insert(
        AXT_FASTPQ_BINDING_METADATA_KEY.into(),
        encode_canonical_norito(context.binding)?,
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
    let trimmed = value.trim().to_ascii_lowercase();
    decode_hex_digest(&trimmed, field)?;
    Ok(trimmed)
}
fn normalized_parameter(value: &str) -> Result<String> {
    required_string(value, "parameter")
}
fn normalized_verifier_id(value: &str) -> Result<String> {
    let verifier_id = required_string(value, "verifier_id")?.to_ascii_lowercase();
    if verifier_id == "fastpq" {
        Ok(verifier_id)
    } else {
        Err(Error::InvalidAxtBinding {
            details: format!("unsupported AXT verifier_id: {value}"),
        })
    }
}
fn normalized_verifier_version(value: &str) -> Result<String> {
    let verifier_version = required_string(value, "verifier_version")?.to_ascii_lowercase();
    if verifier_version == "v1" {
        Ok(verifier_version)
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
    for pair in values.windows(2) {
        if pair[0] == pair[1] {
            return Err(Error::InvalidAxtBinding {
                details: format!("target_dsids contains duplicate value: {}", pair[0]),
            });
        }
        if pair[0] > pair[1] {
            return Err(Error::InvalidAxtBinding {
                details: format!(
                    "target_dsids must be strictly ordered: {} precedes {}",
                    pair[0], pair[1]
                ),
            });
        }
    }
    Ok(values.to_vec())
}
fn normalized_claim_type(value: &str) -> Result<String> {
    let claim_type = value.trim().to_ascii_lowercase();
    match claim_type.as_str() {
        "authorization" | "compliance" | "tx_predicate" | "value_conservation" => Ok(claim_type),
        _ => Err(Error::InvalidAxtBinding {
            details: format!("unsupported claim_type: {value}"),
        }),
    }
}
fn canonicalize_effect_binding(binding: &AxtEffectBinding) -> Result<AxtEffectBinding> {
    Ok(AxtEffectBinding {
        destination_domain: canonical_optional_string(
            binding.destination_domain.as_deref(),
            "effect_binding.destination_domain",
        )?,
        destination_account_id: canonical_optional_string(
            binding.destination_account_id.as_deref(),
            "effect_binding.destination_account_id",
        )?,
        vault_account_id: canonical_optional_string(
            binding.vault_account_id.as_deref(),
            "effect_binding.vault_account_id",
        )?,
        issuance_account_id: canonical_optional_string(
            binding.issuance_account_id.as_deref(),
            "effect_binding.issuance_account_id",
        )?,
        source_asset_definition_id: canonical_optional_string(
            binding.source_asset_definition_id.as_deref(),
            "effect_binding.source_asset_definition_id",
        )?,
        destination_asset_definition_id: canonical_optional_string(
            binding.destination_asset_definition_id.as_deref(),
            "effect_binding.destination_asset_definition_id",
        )?,
        source_amount_i64: binding.source_amount_i64,
        destination_amount_i64: binding.destination_amount_i64,
    })
}
fn canonical_optional_string(value: Option<&str>, field: &str) -> Result<Option<String>> {
    value.map(|value| required_string(value, field)).transpose()
}
fn require_canonical_binding(binding: &AxtFastpqBinding) -> Result<AxtFastpqBinding> {
    let canonical = canonicalize_binding(binding)?;
    if &canonical != binding {
        return Err(Error::InvalidAxtBinding {
            details: "AXT FASTPQ binding must use its exact canonical field representation".into(),
        });
    }
    Ok(canonical)
}
fn encode_canonical_norito<T: NoritoSerialize>(value: &T) -> Result<Vec<u8>> {
    let _canonical_flags =
        norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    to_bytes(value).map_err(Error::Encode)
}
fn decode_canonical_binding(encoded: &[u8]) -> Result<AxtFastpqBinding> {
    let _canonical_flags =
        norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let binding: AxtFastpqBinding =
        decode_from_bytes(encoded).map_err(|source| Error::TransferMetadataDecode { source })?;
    if encode_canonical_norito(&binding)?.as_slice() != encoded {
        return Err(Error::InvalidAxtBinding {
            details: "FastPQ batch metadata binding must use canonical Norito bytes".into(),
        });
    }
    require_canonical_binding(&binding)
}
fn decode_canonical_axt_fastpq_payload(encoded: &[u8]) -> Result<AxtFastpqProofPayload> {
    let _canonical_flags =
        norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let payload: AxtFastpqProofPayload =
        decode_from_bytes(encoded).map_err(|source| Error::AxtProofPayloadDecode { source })?;
    if encode_canonical_norito(&payload)?.as_slice() != encoded {
        return Err(Error::InvalidAxtBinding {
            details: "AXT FastPQ proof payload must use canonical Norito bytes".into(),
        });
    }
    Ok(payload)
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
    let bytes = encode_canonical_norito(&canonical)?;
    Ok(format!("{:x}", sha2::Sha256::digest(bytes)))
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::proof::Prover;
    use iroha_crypto::{Algorithm, Hash, KeyPair};
    use iroha_data_model::{
        account::AccountId,
        asset::id::AssetDefinitionId,
        domain::DomainId,
        fastpq::{TransferDeltaTranscript, TransferSmtWitness, TransferTranscript},
    };
    use iroha_primitives::numeric::Quantity;
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
    fn alternate_norito_bytes<T: NoritoSerialize>(value: &T) -> Vec<u8> {
        let alternate_flags =
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        let _alternate = norito::core::DecodeFlagsGuard::enter(alternate_flags);
        to_bytes(value).expect("encode alternate-layout fixture")
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
    fn real_transfer_claim_batch(binding: &AxtFastpqBinding) -> TransitionBatch {
        const TRANSFER_AMOUNT: u64 = 35;
        const SENDER_START: u64 = 900;
        const RECEIVER_START: u64 = 120;
        let domain = DomainId::try_new("axt", "universal").expect("domain id");
        let asset_definition =
            AssetDefinitionId::derive_from_components(domain.clone(), "rose".parse().unwrap());
        let from_account = deterministic_account("transfer_sender", &domain);
        let to_account = deterministic_account("transfer_receiver", &domain);
        let entry_hash = decode_hex_digest(&binding.source_tx_commitment, "source_tx_commitment")
            .expect("entry hash");
        let transcript_batch_hash = Hash::prehashed(entry_hash);
        let mut batch = TransitionBatch::new(
            DEFAULT_PARAMETER,
            PublicInputs {
                dsid: dsid_bytes(binding.source_dsid),
                slot: 124,
                old_root: [0; 32],
                new_root: [0; 32],
                perm_root: [0x31; 32],
                tx_set_hash: [0x41; 32],
            },
        );
        batch.push(StateTransition::new(
            transfer_balance_key(&asset_definition, &from_account),
            SENDER_START.to_le_bytes().to_vec(),
            (SENDER_START - TRANSFER_AMOUNT).to_le_bytes().to_vec(),
            OperationKind::Transfer,
        ));
        batch.push(StateTransition::new(
            transfer_balance_key(&asset_definition, &to_account),
            RECEIVER_START.to_le_bytes().to_vec(),
            (RECEIVER_START + TRANSFER_AMOUNT).to_le_bytes().to_vec(),
            OperationKind::Transfer,
        ));
        let mut transcripts = vec![transfer_transcript(
            &asset_definition,
            &from_account,
            &to_account,
            TRANSFER_AMOUNT,
            SENDER_START,
            RECEIVER_START,
            transcript_batch_hash,
        )];
        let (old_root, new_root) =
            crate::gadgets::transfer::attach_transfer_smt_witnesses(&mut transcripts)
                .expect("attach transfer SMT witnesses");
        batch.public_inputs.old_root = old_root;
        batch.public_inputs.new_root = new_root;
        batch.metadata.insert(
            TRANSFER_TRANSCRIPTS_METADATA_KEY.into(),
            to_bytes(&transcripts).expect("encode transfer transcripts"),
        );
        batch
            .metadata
            .insert(ENTRY_HASH_METADATA_KEY.into(), entry_hash.to_vec());
        batch.sort();
        bind_axt_batch(&mut batch, binding).expect("bind transfer AXT batch");
        batch
    }
    fn deterministic_account(label: &str, domain: &DomainId) -> AccountId {
        let seed: [u8; Hash::LENGTH] = Hash::new(format!("{label}@{domain}")).into();
        let keypair = KeyPair::try_from_seed(seed.to_vec(), Algorithm::default())
            .expect("derive AXT fixture account key");
        AccountId::new(keypair.public_key().clone())
    }
    #[test]
    fn deterministic_account_uses_checked_seed_derivation() {
        let domain = DomainId::try_new("wonderland", "universal").expect("domain id");
        let seed: [u8; Hash::LENGTH] = Hash::new(format!("alice@{domain}")).into();
        let keypair = KeyPair::try_from_seed(seed.to_vec(), Algorithm::default())
            .expect("derive AXT fixture account key");
        assert_eq!(
            deterministic_account("alice", &domain),
            AccountId::new(keypair.public_key().clone())
        );
    }
    fn transfer_balance_key(asset: &AssetDefinitionId, account: &AccountId) -> Vec<u8> {
        format!("asset/{asset}/{account}").into_bytes()
    }
    fn transfer_transcript(
        asset_definition: &AssetDefinitionId,
        from_account: &AccountId,
        to_account: &AccountId,
        amount: u64,
        from_balance_before: u64,
        to_balance_before: u64,
        batch_hash: Hash,
    ) -> TransferTranscript {
        let delta = TransferDeltaTranscript {
            from_account: from_account.clone(),
            to_account: to_account.clone(),
            asset_definition: asset_definition.clone(),
            amount: Quantity::from(amount),
            from_balance_before: Quantity::from(from_balance_before),
            from_balance_after: Quantity::from(from_balance_before - amount),
            to_balance_before: Quantity::from(to_balance_before),
            to_balance_after: Quantity::from(to_balance_before + amount),
            from_smt_witness: TransferSmtWitness::default(),
            to_smt_witness: TransferSmtWitness::default(),
        };
        let digest = crate::gadgets::transfer::compute_poseidon_digest(&delta, &batch_hash);
        TransferTranscript {
            batch_hash,
            deltas: vec![delta],
            authority_digest: Hash::new(b"axt-transfer-authority"),
            poseidon_preimage_digest: Some(digest),
        }
    }
    #[test]
    fn canonicalize_binding_rejects_non_fastpq_v1_or_blank_verifier_labels() {
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
        let err = canonicalize_binding(&binding).expect_err("blank verifier id must fail");
        assert!(
            matches!(err, Error::InvalidAxtBinding { details } if details.contains("verifier_id"))
        );
        let mut binding = sample_binding();
        binding.verifier_version.clear();
        let err = canonicalize_binding(&binding).expect_err("blank verifier version must fail");
        assert!(
            matches!(err, Error::InvalidAxtBinding { details } if details.contains("verifier_version"))
        );
        let mut binding = sample_binding();
        binding.parameter.clear();
        let err = canonicalize_binding(&binding).expect_err("blank parameter must fail");
        assert!(matches!(
            err,
            Error::InvalidAxtBinding { details } if details.contains("parameter")
        ));
    }
    #[test]
    fn canonicalize_binding_requires_strictly_ordered_unique_targets() {
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
        let mut binding = sample_binding();
        binding.target_dsids = vec![11, 9];
        let err = canonicalize_binding(&binding).expect_err("out-of-order targets must fail");
        assert!(
            matches!(err, Error::InvalidAxtBinding { details } if details.contains("strictly ordered"))
        );
        let mut binding = sample_binding();
        binding.target_dsids = vec![9, 11];
        assert_eq!(
            canonicalize_binding(&binding)
                .expect("strict target order")
                .target_dsids,
            vec![9, 11]
        );
    }
    #[test]
    fn canonicalize_binding_normalizes_labels_and_manifest_hash() {
        let mut binding = sample_binding();
        binding.parameter = format!("  {DEFAULT_PARAMETER}  ");
        binding.source_dataspace = "  taira  ".to_owned();
        binding.source_receipt_id = "  receipt-0001  ".to_owned();
        binding.source_tx_commitment =
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".to_owned();
        binding.claim_type = "  AUTHORIZATION  ".to_owned();
        binding.claim_digest =
            "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB".to_owned();
        binding.verifier_id = "  fastpq  ".to_owned();
        binding.verifier_version = "  v1  ".to_owned();
        binding.corridor = "  corridor-a  ".to_owned();
        let canonical = canonicalize_binding(&binding).expect("canonical binding");
        assert_eq!(canonical.parameter, DEFAULT_PARAMETER);
        assert_eq!(canonical.source_dataspace, "taira");
        assert_eq!(canonical.source_receipt_id, "receipt-0001");
        assert_eq!(
            canonical.source_tx_commitment,
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        );
        assert_eq!(canonical.claim_type, "authorization");
        assert_eq!(
            canonical.claim_digest,
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
        );
        assert_eq!(canonical.verifier_id, "fastpq");
        assert_eq!(canonical.verifier_version, "v1");
        assert_eq!(canonical.corridor, "corridor-a");
        assert_eq!(
            batch_manifest_sha256(&binding).expect("raw manifest"),
            batch_manifest_sha256(&canonical).expect("canonical manifest")
        );
    }
    #[test]
    fn verification_rejects_normalizable_but_noncanonical_binding_values() {
        let mutations: [fn(&mut AxtFastpqBinding); 4] = [
            |binding: &mut AxtFastpqBinding| binding.parameter = format!(" {DEFAULT_PARAMETER}"),
            |binding: &mut AxtFastpqBinding| {
                binding.claim_type = "AUTHORIZATION".to_owned();
            },
            |binding: &mut AxtFastpqBinding| {
                binding.claim_digest =
                    "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".to_owned();
            },
            |binding: &mut AxtFastpqBinding| binding.verifier_id = "fastpq ".to_owned(),
        ];
        for mutate in mutations {
            let mut binding = sample_binding();
            mutate(&mut binding);
            let envelope = envelope_with_payload(binding, vec![0x00]);
            let err = verify_axt_proof_envelope(&envelope)
                .expect_err("normalizable noncanonical binding must fail before proof decoding");
            assert!(
                matches!(&err, Error::InvalidAxtBinding { details } if details.contains("exact canonical")),
                "unexpected error: {err:?}"
            );
        }
    }
    #[test]
    fn bind_axt_batch_pins_canonical_norito_flags_under_an_ambient_layout() {
        let binding = sample_binding();
        let canonical = real_authorization_batch(&binding);
        let alternate_flags =
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        let under_alternate_layout = {
            let _alternate = norito::core::DecodeFlagsGuard::enter(alternate_flags);
            real_authorization_batch(&binding)
        };
        assert_eq!(
            under_alternate_layout
                .metadata
                .get(AXT_FASTPQ_BINDING_METADATA_KEY),
            canonical.metadata.get(AXT_FASTPQ_BINDING_METADATA_KEY)
        );
        assert_eq!(
            under_alternate_layout
                .metadata
                .get(AXT_FASTPQ_BATCH_SEAL_METADATA_KEY),
            canonical.metadata.get(AXT_FASTPQ_BATCH_SEAL_METADATA_KEY)
        );
    }
    #[test]
    fn canonicalize_binding_rejects_malformed_digest_and_claim_type() {
        let mut binding = sample_binding();
        binding.claim_digest = "abcd".to_owned();
        let err = canonicalize_binding(&binding).expect_err("short digest must fail");
        assert!(
            matches!(err, Error::InvalidAxtBinding { details } if details.contains("claim_digest"))
        );
        let mut binding = sample_binding();
        binding.claim_type = "synthetic".to_owned();
        let err = canonicalize_binding(&binding).expect_err("unsupported claim type must fail");
        assert!(
            matches!(err, Error::InvalidAxtBinding { details } if details.contains("claim_type"))
        );
    }
    #[test]
    fn transition_batch_model_roundtrip_preserves_operations_and_metadata() {
        let mut batch = TransitionBatch::new(
            DEFAULT_PARAMETER,
            PublicInputs {
                dsid: dsid_bytes(77),
                slot: 321,
                old_root: [0x01; 32],
                new_root: [0x02; 32],
                perm_root: [0x03; 32],
                tx_set_hash: [0x04; 32],
            },
        );
        batch.push(StateTransition::new(
            b"asset/mint".to_vec(),
            vec![0],
            vec![1],
            OperationKind::Mint,
        ));
        batch.push(StateTransition::new(
            b"asset/burn".to_vec(),
            vec![2],
            vec![1],
            OperationKind::Burn,
        ));
        batch.push(StateTransition::new(
            b"role/revoke".to_vec(),
            vec![1],
            vec![0],
            OperationKind::RoleRevoke {
                role_id: vec![0xAA; 32],
                permission_id: vec![0xBB; 32],
                epoch: 9,
            },
        ));
        batch.push(StateTransition::new(
            b"account/meta".to_vec(),
            b"old".to_vec(),
            b"new".to_vec(),
            OperationKind::MetaSet,
        ));
        batch
            .metadata
            .insert("fixture".to_owned(), b"roundtrip".to_vec());
        let roundtrip = transition_batch_from_model(&transition_batch_to_model(&batch));
        assert_eq!(roundtrip, batch);
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
    fn verify_axt_envelope_rejects_missing_fastpq_binding() {
        let binding = sample_binding();
        let batch = real_authorization_batch(&binding);
        let proof = Prover::canonical(DEFAULT_PARAMETER)
            .expect("prover")
            .prove(&batch)
            .expect("proof");
        let payload = encode_axt_fastpq_payload(&batch, proof).expect("payload");
        let mut envelope = envelope_with_payload(binding, payload);
        envelope.fastpq_binding = None;
        let err = verify_axt_proof_envelope(&envelope).expect_err("missing binding must fail");
        assert!(
            matches!(err, Error::InvalidAxtBinding { details } if details.contains("fastpq_binding"))
        );
    }
    #[test]
    fn bind_axt_batch_rejects_empty_execution_batch() {
        let binding = sample_binding();
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
        let entry_hash = decode_hex_digest(&binding.source_tx_commitment, "source_tx_commitment")
            .expect("entry hash");
        batch
            .metadata
            .insert(ENTRY_HASH_METADATA_KEY.into(), entry_hash.to_vec());
        let err = bind_axt_batch(&mut batch, &binding).expect_err("empty batch must fail");
        assert!(
            matches!(err, Error::InvalidAxtBinding { details } if details.contains("state transitions"))
        );
    }
    #[test]
    fn bind_axt_batch_rejects_parameter_mismatch() {
        let binding = sample_binding();
        let mut batch = TransitionBatch::new(
            "fastpq-lane-minimal",
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
        let entry_hash = decode_hex_digest(&binding.source_tx_commitment, "source_tx_commitment")
            .expect("entry hash");
        batch
            .metadata
            .insert(ENTRY_HASH_METADATA_KEY.into(), entry_hash.to_vec());
        let err = bind_axt_batch(&mut batch, &binding).expect_err("parameter mismatch must fail");
        assert!(
            matches!(err, Error::InvalidAxtBinding { details } if details.contains("parameter"))
        );
    }
    #[test]
    fn bind_axt_batch_rejects_missing_entry_hash() {
        let binding = sample_binding();
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
        let err = bind_axt_batch(&mut batch, &binding).expect_err("entry hash is required");
        assert!(matches!(err, Error::MissingMetadata { key } if key == ENTRY_HASH_METADATA_KEY));
    }
    #[test]
    fn bind_axt_batch_rejects_entry_hash_mismatch() {
        let binding = sample_binding();
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
        batch
            .metadata
            .insert(ENTRY_HASH_METADATA_KEY.into(), vec![0xAA; 32]);
        let err = bind_axt_batch(&mut batch, &binding).expect_err("wrong entry hash fails");
        assert!(
            matches!(err, Error::InvalidAxtBinding { details } if details.contains(ENTRY_HASH_METADATA_KEY))
        );
    }
    #[test]
    fn verify_axt_envelope_rejects_missing_required_binding_metadata() {
        let binding = sample_binding();
        let batch = real_authorization_batch(&binding);
        let proof = Prover::canonical(DEFAULT_PARAMETER)
            .expect("prover")
            .prove(&batch)
            .expect("proof");
        for key in ["claim_digest", "policy_commitment", "verified_effect_type"] {
            let mut tampered = batch.clone();
            tampered.metadata.remove(key);
            let payload = encode_axt_fastpq_payload(&tampered, proof.clone()).expect("payload");
            let envelope = envelope_with_payload(binding.clone(), payload);
            let err = match verify_axt_proof_envelope(&envelope) {
                Ok(_) => panic!("missing {key} must fail"),
                Err(err) => err,
            };
            assert!(matches!(err, Error::MissingMetadata { key: missing } if missing == key));
        }
    }
    #[test]
    fn verify_axt_envelope_rejects_required_metadata_mismatches() {
        let binding = sample_binding();
        let batch = real_authorization_batch(&binding);
        let proof = Prover::canonical(DEFAULT_PARAMETER)
            .expect("prover")
            .prove(&batch)
            .expect("proof");
        for key in ["source_receipt_id", "witness_commitment", "corridor"] {
            let mut tampered = batch.clone();
            tampered.metadata.insert(key.to_owned(), b"wrong".to_vec());
            let payload = encode_axt_fastpq_payload(&tampered, proof.clone()).expect("payload");
            let envelope = envelope_with_payload(binding.clone(), payload);
            let err = match verify_axt_proof_envelope(&envelope) {
                Ok(_) => panic!("mismatched {key} must fail"),
                Err(err) => err,
            };
            assert!(
                matches!(err, Error::InvalidAxtBinding { ref details } if details.contains(key)),
                "unexpected error for {key}: {err:?}"
            );
        }
    }
    #[test]
    fn verify_axt_envelope_rejects_envelope_dsid_mismatch() {
        let binding = sample_binding();
        let batch = real_authorization_batch(&binding);
        let proof = Prover::canonical(DEFAULT_PARAMETER)
            .expect("prover")
            .prove(&batch)
            .expect("proof");
        let payload = encode_axt_fastpq_payload(&batch, proof).expect("payload");
        let mut envelope = envelope_with_payload(binding, payload);
        envelope.dsid = DataSpaceId::new(envelope.dsid.as_u64() + 1);
        let err = verify_axt_proof_envelope(&envelope).expect_err("envelope dsid mismatch");
        assert!(
            matches!(err, Error::InvalidAxtBinding { details } if details.contains("source_dsid"))
        );
    }
    #[test]
    fn verify_axt_envelope_rejects_batch_parameter_mismatch() {
        let binding = sample_binding();
        let mut batch = real_authorization_batch(&binding);
        let proof = Prover::canonical(DEFAULT_PARAMETER)
            .expect("prover")
            .prove(&batch)
            .expect("proof");
        batch.parameter = "fastpq-lane-minimal".to_owned();
        let payload = encode_axt_fastpq_payload(&batch, proof).expect("payload");
        let envelope = envelope_with_payload(binding, payload);
        let err = verify_axt_proof_envelope(&envelope).expect_err("parameter mismatch");
        assert!(
            matches!(err, Error::InvalidAxtBinding { details } if details.contains("parameter"))
        );
    }
    #[test]
    fn verify_axt_envelope_rejects_batch_public_dsid_mismatch() {
        let binding = sample_binding();
        let mut batch = real_authorization_batch(&binding);
        let proof = Prover::canonical(DEFAULT_PARAMETER)
            .expect("prover")
            .prove(&batch)
            .expect("proof");
        batch.public_inputs.dsid = dsid_bytes(binding.source_dsid + 1);
        let payload = encode_axt_fastpq_payload(&batch, proof).expect("payload");
        let envelope = envelope_with_payload(binding, payload);
        let err = verify_axt_proof_envelope(&envelope).expect_err("batch dsid mismatch");
        assert!(
            matches!(err, Error::InvalidAxtBinding { details } if details.contains("public dsid"))
        );
    }
    #[test]
    fn verify_axt_envelope_rejects_embedded_binding_metadata_mismatch() {
        let binding = sample_binding();
        let mut batch = real_authorization_batch(&binding);
        let proof = Prover::canonical(DEFAULT_PARAMETER)
            .expect("prover")
            .prove(&batch)
            .expect("proof");
        let mut embedded = binding.clone();
        embedded.claim_digest =
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned();
        batch.metadata.insert(
            AXT_FASTPQ_BINDING_METADATA_KEY.into(),
            to_bytes(&embedded).expect("encode embedded binding"),
        );
        let payload = encode_axt_fastpq_payload(&batch, proof).expect("payload");
        let envelope = envelope_with_payload(binding, payload);
        let err = verify_axt_proof_envelope(&envelope).expect_err("binding metadata mismatch");
        assert!(
            matches!(err, Error::InvalidAxtBinding { details } if details.contains("metadata binding"))
        );
    }
    #[test]
    fn verify_axt_envelope_rejects_malformed_embedded_binding_metadata() {
        let binding = sample_binding();
        let mut batch = real_authorization_batch(&binding);
        let proof = Prover::canonical(DEFAULT_PARAMETER)
            .expect("prover")
            .prove(&batch)
            .expect("proof");
        batch
            .metadata
            .insert(AXT_FASTPQ_BINDING_METADATA_KEY.into(), vec![0xFF, 0x00]);
        let payload = encode_axt_fastpq_payload(&batch, proof).expect("payload");
        let envelope = envelope_with_payload(binding, payload);
        let err = verify_axt_proof_envelope(&envelope).expect_err("malformed binding metadata");
        assert!(matches!(err, Error::TransferMetadataDecode { .. }));
    }
    #[test]
    fn embedded_axt_binding_rejects_alternate_norito_layout() {
        let binding = sample_binding();
        let mut batch = real_authorization_batch(&binding);
        let alternate = alternate_norito_bytes(&binding);
        assert_ne!(
            alternate,
            encode_canonical_norito(&binding).expect("canonical binding")
        );
        assert_eq!(
            decode_from_bytes::<AxtFastpqBinding>(&alternate)
                .expect("ordinary Norito accepts advertised alternate layout"),
            binding
        );
        batch
            .metadata
            .insert(AXT_FASTPQ_BINDING_METADATA_KEY.into(), alternate);
        let err = embedded_axt_binding(&batch).expect_err("alternate binding layout must fail");
        assert!(
            matches!(err, Error::InvalidAxtBinding { details } if details.contains("canonical Norito"))
        );
    }
    #[test]
    fn embedded_axt_binding_rejects_semantically_noncanonical_metadata() {
        let binding = sample_binding();
        let mut batch = real_authorization_batch(&binding);
        let mut noncanonical = binding;
        noncanonical.claim_type = " AUTHORIZATION ".to_owned();
        batch.metadata.insert(
            AXT_FASTPQ_BINDING_METADATA_KEY.into(),
            encode_canonical_norito(&noncanonical).expect("canonical Norito layout"),
        );
        let err = embedded_axt_binding(&batch)
            .expect_err("normalizable metadata value must not be accepted as canonical");
        assert!(
            matches!(err, Error::InvalidAxtBinding { details } if details.contains("exact canonical"))
        );
    }
    #[test]
    fn verify_axt_envelope_rejects_source_tx_commitment_metadata_mismatch() {
        let binding = sample_binding();
        let mut batch = real_authorization_batch(&binding);
        let proof = Prover::canonical(DEFAULT_PARAMETER)
            .expect("prover")
            .prove(&batch)
            .expect("proof");
        batch
            .metadata
            .insert("source_tx_commitment".into(), vec![0xAA; 32]);
        let payload = encode_axt_fastpq_payload(&batch, proof).expect("payload");
        let envelope = envelope_with_payload(binding, payload);
        let err = verify_axt_proof_envelope(&envelope).expect_err("commitment metadata mismatch");
        assert!(
            matches!(err, Error::InvalidAxtBinding { details } if details.contains("source_tx_commitment"))
        );
    }
    #[test]
    fn verify_axt_envelope_rejects_target_dsid_metadata_mismatch() {
        let binding = sample_binding();
        let mut batch = real_authorization_batch(&binding);
        let proof = Prover::canonical(DEFAULT_PARAMETER)
            .expect("prover")
            .prove(&batch)
            .expect("proof");
        batch.metadata.insert(
            "target_dsids".into(),
            encode_target_dsids(&[binding.target_dsids[0] + 1]),
        );
        let payload = encode_axt_fastpq_payload(&batch, proof).expect("payload");
        let envelope = envelope_with_payload(binding, payload);
        let err = verify_axt_proof_envelope(&envelope).expect_err("target dsid mismatch");
        assert!(
            matches!(err, Error::InvalidAxtBinding { details } if details.contains("target_dsids"))
        );
    }
    #[test]
    fn verify_axt_envelope_rejects_transfer_claim_missing_transcripts() {
        let mut binding = sample_binding();
        binding.claim_type = "tx_predicate".to_owned();
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
            b"asset/rose/alice".to_vec(),
            10_u64.to_le_bytes().to_vec(),
            7_u64.to_le_bytes().to_vec(),
            OperationKind::Transfer,
        ));
        let entry_hash = decode_hex_digest(&binding.source_tx_commitment, "source_tx_commitment")
            .expect("entry hash");
        batch
            .metadata
            .insert(ENTRY_HASH_METADATA_KEY.into(), entry_hash.to_vec());
        bind_axt_batch(&mut batch, &binding).expect("bind transfer claim batch");
        let proof_batch = real_authorization_batch(&sample_binding());
        let proof = Prover::canonical(DEFAULT_PARAMETER)
            .expect("prover")
            .prove(&proof_batch)
            .expect("proof");
        let payload = encode_axt_fastpq_payload(&batch, proof).expect("payload");
        let envelope = envelope_with_payload(binding, payload);
        let err = verify_axt_proof_envelope(&envelope).expect_err("missing transfer transcripts");
        assert!(
            matches!(err, Error::MissingMetadata { key } if key == TRANSFER_TRANSCRIPTS_METADATA_KEY)
        );
    }
    #[test]
    fn verify_axt_envelope_rejects_transfer_claim_without_transfer_rows() {
        let mut binding = sample_binding();
        binding.claim_type = "value_conservation".to_owned();
        let batch = real_authorization_batch(&binding);
        let proof = Prover::canonical(DEFAULT_PARAMETER)
            .expect("prover")
            .prove(&batch)
            .expect("proof");
        let payload = encode_axt_fastpq_payload(&batch, proof).expect("payload");
        let envelope = envelope_with_payload(binding, payload);
        let err = verify_axt_proof_envelope(&envelope).expect_err("non-transfer claim must fail");
        assert!(
            matches!(err, Error::InvalidAxtBinding { details } if details.contains("transfer transitions"))
        );
    }
    #[test]
    fn verify_axt_envelope_accepts_transfer_claims_with_real_transcripts() {
        for claim_type in ["tx_predicate", "value_conservation"] {
            let mut binding = sample_binding();
            binding.claim_type = claim_type.to_owned();
            let batch = real_transfer_claim_batch(&binding);
            assert!(
                batch
                    .metadata
                    .contains_key(TRANSFER_TRANSCRIPTS_METADATA_KEY),
                "transfer claim fixture must carry transcript metadata"
            );
            assert!(
                batch
                    .transitions
                    .iter()
                    .any(|transition| matches!(transition.operation, OperationKind::Transfer)),
                "transfer claim fixture must carry transfer rows"
            );
            let proof = Prover::canonical(DEFAULT_PARAMETER)
                .expect("prover")
                .prove(&batch)
                .expect("proof");
            let payload = encode_axt_fastpq_payload(&batch, proof).expect("payload");
            let envelope = envelope_with_payload(binding, payload);
            let verified = verify_axt_proof_envelope(&envelope)
                .unwrap_or_else(|err| panic!("{claim_type} transfer claim should verify: {err}"));
            assert!(verified.statement_digest.iter().any(|byte| *byte != 0));
            assert!(verified.proof_digest.as_ref().iter().any(|byte| *byte != 0));
        }
    }
    #[test]
    fn verify_axt_envelope_rejects_transfer_transcript_from_an_unrelated_transaction() {
        let mut binding = sample_binding();
        binding.claim_type = "value_conservation".to_owned();
        let mut batch = real_transfer_claim_batch(&binding);
        let mut transcripts = decode_transcripts(&batch.metadata)
            .expect("decode transcript metadata")
            .expect("transfer transcripts");
        let unrelated_hash = Hash::prehashed([0xAA; Hash::LENGTH]);
        let unrelated_digest = crate::gadgets::transfer::compute_poseidon_digest(
            &transcripts[0].deltas[0],
            &unrelated_hash,
        );
        transcripts[0].batch_hash = unrelated_hash;
        transcripts[0].poseidon_preimage_digest = Some(unrelated_digest);
        batch.metadata.insert(
            TRANSFER_TRANSCRIPTS_METADATA_KEY.into(),
            to_bytes(&transcripts).expect("encode unrelated transfer transcripts"),
        );
        bind_axt_batch(&mut batch, &binding).expect("reseal unrelated transfer batch");
        let proof = Prover::canonical(DEFAULT_PARAMETER)
            .expect("prover")
            .prove(&batch)
            .expect("proof for internally consistent unrelated transcript");
        let payload = encode_axt_fastpq_payload(&batch, proof).expect("payload");
        let envelope = envelope_with_payload(binding, payload);
        let err = verify_axt_proof_envelope(&envelope)
            .expect_err("unrelated transaction transcript must fail");
        assert!(
            matches!(err, Error::InvalidAxtBinding { details } if details.contains("batch_hash") && details.contains("source_tx_commitment"))
        );
    }
    #[test]
    fn verify_axt_envelope_accepts_empty_corridor_without_metadata() {
        let mut binding = sample_binding();
        binding.corridor.clear();
        let batch = real_authorization_batch(&binding);
        assert!(
            !batch.metadata.contains_key("corridor"),
            "empty corridor must not require a metadata field"
        );
        let proof = Prover::canonical(DEFAULT_PARAMETER)
            .expect("prover")
            .prove(&batch)
            .expect("proof");
        let payload = encode_axt_fastpq_payload(&batch, proof).expect("payload");
        let envelope = envelope_with_payload(binding, payload);
        verify_axt_proof_envelope(&envelope).expect("empty corridor binding verifies");
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
    fn verify_axt_envelope_rejects_alternate_payload_layout_and_encoder_is_pinned() {
        let binding = sample_binding();
        let batch = real_authorization_batch(&binding);
        let proof = Prover::canonical(DEFAULT_PARAMETER)
            .expect("prover")
            .prove(&batch)
            .expect("proof");
        let payload = AxtFastpqProofPayload {
            batch: transition_batch_to_model(&batch),
            proof: proof.clone(),
        };
        let canonical =
            encode_axt_fastpq_payload(&batch, proof.clone()).expect("canonical payload");
        let alternate = alternate_norito_bytes(&payload);
        assert_ne!(alternate, canonical);
        assert_eq!(
            decode_from_bytes::<AxtFastpqProofPayload>(&alternate)
                .expect("ordinary Norito accepts advertised alternate layout"),
            payload
        );
        let alternate_flags =
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        let pinned_under_ambient = {
            let _alternate = norito::core::DecodeFlagsGuard::enter(alternate_flags);
            encode_axt_fastpq_payload(&batch, proof).expect("pinned payload")
        };
        assert_eq!(pinned_under_ambient, canonical);
        let envelope = envelope_with_payload(binding, alternate);
        let err =
            verify_axt_proof_envelope(&envelope).expect_err("alternate payload layout must fail");
        assert!(
            matches!(err, Error::InvalidAxtBinding { details } if details.contains("proof payload") && details.contains("canonical Norito"))
        );
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
        assert_eq!(verified.old_root, batch.public_inputs.old_root);
        assert_eq!(verified.new_root, batch.public_inputs.new_root);
        assert_eq!(verified.tx_set_hash, batch.public_inputs.tx_set_hash);
    }
    #[test]
    fn verify_axt_envelope_statement_digest_binds_optional_da_commitment() {
        let binding = sample_binding();
        let batch = real_authorization_batch(&binding);
        let proof = Prover::canonical(DEFAULT_PARAMETER)
            .expect("prover")
            .prove(&batch)
            .expect("proof");
        let payload = encode_axt_fastpq_payload(&batch, proof).expect("payload");
        let with_da =
            verify_axt_proof_envelope(&envelope_with_payload(binding.clone(), payload.clone()))
                .expect("with DA commitment");
        let mut without_da = envelope_with_payload(binding, payload);
        without_da.da_commitment = None;
        let without_da = verify_axt_proof_envelope(&without_da).expect("without DA commitment");
        assert_ne!(with_da.statement_digest, without_da.statement_digest);
        assert_eq!(with_da.proof_digest, without_da.proof_digest);
    }
    #[test]
    fn verify_axt_envelope_rejects_batch_mutated_after_seal() {
        let binding = sample_binding();
        let mut batch = real_authorization_batch(&binding);
        let proof = Prover::canonical(DEFAULT_PARAMETER)
            .expect("prover")
            .prove(&batch)
            .expect("proof");
        batch.push(StateTransition::new(
            b"account/real/axt-tampered".to_vec(),
            b"before".to_vec(),
            b"after".to_vec(),
            OperationKind::MetaSet,
        ));
        batch.sort();
        let payload = encode_axt_fastpq_payload(&batch, proof).expect("payload");
        let envelope = envelope_with_payload(binding, payload);
        let err = verify_axt_proof_envelope(&envelope).expect_err("tampered seal must fail");
        assert!(
            matches!(err, Error::InvalidAxtBinding { details } if details.contains(AXT_FASTPQ_BATCH_SEAL_METADATA_KEY))
        );
    }
    #[test]
    fn verify_axt_envelope_rejects_proof_for_different_batch() {
        let binding = sample_binding();
        let batch = real_authorization_batch(&binding);
        let mut other_batch = real_authorization_batch(&binding);
        other_batch.push(StateTransition::new(
            b"account/real/axt-other".to_vec(),
            b"before".to_vec(),
            b"after".to_vec(),
            OperationKind::MetaSet,
        ));
        other_batch.sort();
        bind_axt_batch(&mut other_batch, &binding).expect("rebind mutated batch");
        let proof = Prover::canonical(DEFAULT_PARAMETER)
            .expect("prover")
            .prove(&other_batch)
            .expect("proof");
        let payload = encode_axt_fastpq_payload(&batch, proof).expect("payload");
        let envelope = envelope_with_payload(binding, payload);
        let err = verify_axt_proof_envelope(&envelope).expect_err("mismatched proof must fail");
        assert!(matches!(err, Error::CommitmentMismatch));
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
    fn axt_proof_blob_helper_accepts_already_bound_batch() {
        let binding = sample_binding();
        let batch = real_authorization_batch(&binding);
        let proof = Prover::canonical(DEFAULT_PARAMETER)
            .expect("prover")
            .prove(&batch)
            .expect("proof");
        let manifest_root = [0x42; 32];
        let blob = axt_proof_blob_from_bound_batch(
            &batch,
            proof,
            manifest_root,
            Some([0x24; 32]),
            Some(9),
        )
        .expect("AXT proof blob");
        assert_eq!(blob.expiry_slot, Some(9));
        assert!(iroha_data_model::nexus::proof_matches_manifest(
            &blob,
            DataSpaceId::new(binding.source_dsid),
            manifest_root,
        ));
        let envelope: AxtProofEnvelope =
            decode_from_bytes(&blob.payload).expect("decode AXT proof envelope");
        verify_axt_proof_envelope(&envelope).expect("packaged AXT proof verifies");
    }
    #[test]
    fn axt_proof_blob_helper_rejects_unbound_batch() {
        let binding = sample_binding();
        let mut batch = real_authorization_batch(&binding);
        batch.metadata.remove(AXT_FASTPQ_BINDING_METADATA_KEY);
        let proof = Prover::canonical(DEFAULT_PARAMETER)
            .expect("prover")
            .prove(&batch)
            .expect("proof");
        let err = axt_proof_blob_from_bound_batch(&batch, proof, [0x42; 32], None, None)
            .expect_err("unbound batch must fail");
        assert!(
            matches!(err, Error::MissingMetadata { key } if key == AXT_FASTPQ_BINDING_METADATA_KEY)
        );
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
