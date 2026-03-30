use std::str::FromStr;

use iroha_crypto::{Algorithm, Hash};
use iroha_data_model::{
    DataSpaceId,
    account::AccountId,
    asset::id::AssetDefinitionId,
    domain::DomainId,
    fastpq::{TRANSFER_TRANSCRIPTS_METADATA_KEY, TransferDeltaTranscript, TransferTranscript},
    nexus::{AxtEffectBinding, AxtFastpqBinding},
};
use iroha_primitives::numeric::Numeric;
use norito::to_bytes;
use sha2::Digest;

use crate::{Error, OperationKind, PublicInputs, Result, StateTransition, TransitionBatch};

/// Metadata key binding the structured AXT FASTPQ payload into the proof trace.
pub const AXT_FASTPQ_BINDING_METADATA_KEY: &str = "axt_fastpq_binding";

/// Canonical FASTPQ parameter name used by maintained AXT flows.
pub const DEFAULT_PARAMETER: &str = "fastpq-lane-balanced";

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
        verifier_id: normalized_verifier_id(&binding.verifier_id),
        verifier_version: normalized_verifier_version(&binding.verifier_version),
        target_dsids: binding.target_dsids.clone(),
        effect_binding: binding.effect_binding.clone(),
    })
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

/// Build the deterministic FASTPQ batch committed by an AXT proof envelope.
///
/// # Errors
/// Returns [`Error::InvalidAxtBinding`] when the binding cannot be canonicalized
/// into a supported deterministic batch or when transcript metadata cannot be encoded.
pub fn build_batch_from_binding(binding: &AxtFastpqBinding) -> Result<TransitionBatch> {
    let binding = canonicalize_binding(binding)?;
    let context = BindingContext::from_binding(&binding)?;
    let mut batch = TransitionBatch::new(binding.parameter.clone(), build_public_inputs(&context));
    let mut transfer_transcripts = Vec::new();

    insert_binding_metadata(&mut batch, &context)?;
    append_claim_transitions(&mut batch, &context, &mut transfer_transcripts)?;
    append_receipt_binding_transition(&mut batch, &context);

    if !transfer_transcripts.is_empty() {
        batch.metadata.insert(
            TRANSFER_TRANSCRIPTS_METADATA_KEY.into(),
            to_bytes(&transfer_transcripts).map_err(Error::Encode)?,
        );
    }
    batch.sort();
    Ok(batch)
}

fn build_public_inputs(context: &BindingContext<'_>) -> PublicInputs {
    PublicInputs {
        dsid: dsid_bytes(context.binding.source_dsid),
        slot: u64::from_le_bytes(
            context.source_tx_commitment[..8]
                .try_into()
                .expect("digest slice"),
        ),
        old_root: digest32(
            b"fastpq-json:old_root",
            &[
                &context.source_tx_commitment,
                &context.policy_commitment,
                context.effect_type.as_bytes(),
            ],
        ),
        new_root: digest32(
            b"fastpq-json:new_root",
            &[
                &context.source_tx_commitment,
                &context.claim_digest,
                context.effect_type.as_bytes(),
            ],
        ),
        perm_root: digest32(
            b"fastpq-json:perm_root",
            &[
                &context.policy_commitment,
                context.binding.verifier_id.as_bytes(),
                context.binding.verifier_version.as_bytes(),
            ],
        ),
        tx_set_hash: digest32(
            b"fastpq-json:tx_set_hash",
            &[
                &context.source_tx_commitment,
                &context.claim_digest,
                &context.witness_commitment,
            ],
        ),
    }
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

fn append_claim_transitions(
    batch: &mut TransitionBatch,
    context: &BindingContext<'_>,
    transfer_transcripts: &mut Vec<TransferTranscript>,
) -> Result<()> {
    match context.binding.claim_type.as_str() {
        "authorization" => {
            append_authorization_transitions(batch, context);
            Ok(())
        }
        "compliance" => {
            append_compliance_transitions(batch, context);
            Ok(())
        }
        "tx_predicate" => append_tx_predicate_transitions(batch, context, transfer_transcripts),
        "value_conservation" => {
            append_value_conservation_transitions(batch, context, transfer_transcripts)
        }
        other => Err(Error::InvalidAxtBinding {
            details: format!("unsupported claim_type: {other}"),
        }),
    }
}

fn append_authorization_transitions(batch: &mut TransitionBatch, context: &BindingContext<'_>) {
    batch.push(StateTransition::new(
        key_bytes(
            "authorization",
            &context.source_tx_commitment,
            context.effect_type.as_bytes(),
        ),
        vec![0],
        vec![1],
        OperationKind::RoleGrant {
            role_id: context.claim_digest.to_vec(),
            permission_id: context.witness_commitment.to_vec(),
            epoch: u64::from_le_bytes(
                context.policy_commitment[..8]
                    .try_into()
                    .expect("digest slice"),
            ),
        },
    ));
    batch.push(StateTransition::new(
        key_bytes(
            "authorization-policy",
            &context.policy_commitment,
            context.binding.verifier_id.as_bytes(),
        ),
        vec![],
        context.claim_digest.to_vec(),
        OperationKind::MetaSet,
    ));
}

fn append_compliance_transitions(batch: &mut TransitionBatch, context: &BindingContext<'_>) {
    batch.push(StateTransition::new(
        key_bytes(
            "compliance-policy",
            &context.policy_commitment,
            context.effect_type.as_bytes(),
        ),
        context.claim_digest.to_vec(),
        context.witness_commitment.to_vec(),
        OperationKind::MetaSet,
    ));
    batch.push(StateTransition::new(
        key_bytes(
            "compliance-targets",
            &context.source_tx_commitment,
            context.binding.verifier_version.as_bytes(),
        ),
        encode_target_dsids(&context.binding.target_dsids),
        context.policy_commitment.to_vec(),
        OperationKind::MetaSet,
    ));
}

fn append_tx_predicate_transitions(
    batch: &mut TransitionBatch,
    context: &BindingContext<'_>,
    transfer_transcripts: &mut Vec<TransferTranscript>,
) -> Result<()> {
    let amount = transfer_amount(
        context.binding.effect_binding.as_ref(),
        &context.claim_digest,
        10_000,
        200_000,
    )?;
    let sender_before = amount + bounded_amount(&context.witness_commitment, 5_000, 25_000);
    let receiver_before = bounded_amount(&context.policy_commitment, 1_000, 10_000);
    let transfer = build_transfer_witness_bundle(
        "predicate",
        context.binding,
        &context.source_tx_commitment,
        &context.claim_digest,
        amount,
        sender_before,
        receiver_before,
    )?;
    batch.push(transfer.sender);
    batch.push(transfer.receiver);
    transfer_transcripts.push(transfer.transcript);
    Ok(())
}

fn append_value_conservation_transitions(
    batch: &mut TransitionBatch,
    context: &BindingContext<'_>,
    transfer_transcripts: &mut Vec<TransferTranscript>,
) -> Result<()> {
    let transferred = transfer_amount(
        context.binding.effect_binding.as_ref(),
        &context.claim_digest,
        25_000,
        250_000,
    )?;
    let source_before = transferred + bounded_amount(&context.witness_commitment, 25_000, 250_000);
    let target_before = bounded_amount(&context.policy_commitment, 1_000, 50_000);
    let transfer = build_transfer_witness_bundle(
        "conservation",
        context.binding,
        &context.source_tx_commitment,
        &context.claim_digest,
        transferred,
        source_before,
        target_before,
    )?;
    batch.push(transfer.sender);
    batch.push(transfer.receiver);
    transfer_transcripts.push(transfer.transcript);
    Ok(())
}

fn append_receipt_binding_transition(batch: &mut TransitionBatch, context: &BindingContext<'_>) {
    batch.push(StateTransition::new(
        key_bytes(
            "receipt-binding",
            &context.policy_commitment,
            context.binding.verifier_version.as_bytes(),
        ),
        context.claim_digest.to_vec(),
        context.witness_commitment.to_vec(),
        OperationKind::MetaSet,
    ));
}

fn transfer_amount(
    effect_binding: Option<&AxtEffectBinding>,
    digest: &[u8; 32],
    min: u64,
    span: u64,
) -> Result<u64> {
    if let Some(binding) = effect_binding
        && let Some(value) = binding.destination_amount_i64.or(binding.source_amount_i64)
    {
        let amount = u64::try_from(value).map_err(|_| Error::InvalidAxtBinding {
            details: "effect amount must be non-negative".into(),
        })?;
        if amount == 0 {
            return Err(Error::InvalidAxtBinding {
                details: "effect amount must be positive".into(),
            });
        }
        return Ok(amount);
    }
    Ok(bounded_amount(digest, min, span))
}

struct TransferWitnessBundle {
    transcript: TransferTranscript,
    sender: StateTransition,
    receiver: StateTransition,
}

fn build_transfer_witness_bundle(
    label: &str,
    binding: &AxtFastpqBinding,
    source_tx_commitment: &[u8; 32],
    claim_digest: &[u8; 32],
    amount: u64,
    sender_before: u64,
    receiver_before: u64,
) -> Result<TransferWitnessBundle> {
    let sender_after =
        sender_before
            .checked_sub(amount)
            .ok_or_else(|| Error::InvalidAxtBinding {
                details: format!("{label} sender balance underflow"),
            })?;
    let receiver_after =
        receiver_before
            .checked_add(amount)
            .ok_or_else(|| Error::InvalidAxtBinding {
                details: format!("{label} receiver balance overflow"),
            })?;
    let domain = DomainId::from_str("lane").map_err(|err| Error::InvalidAxtBinding {
        details: format!("invalid FASTPQ lane domain: {err}"),
    })?;
    let sender = deterministic_account(
        &format!("{label}:source:{}", &binding.source_tx_commitment[..16]),
        source_tx_commitment,
    );
    let receiver = deterministic_account(
        &format!("{label}:target:{}", &binding.claim_digest[..16]),
        claim_digest,
    );
    let asset_definition = AssetDefinitionId::new(
        domain,
        format!("asset_{}", &binding.claim_digest[..12])
            .parse()
            .map_err(|err| Error::InvalidAxtBinding {
                details: format!("invalid FASTPQ asset name: {err}"),
            })?,
    );
    let delta = TransferDeltaTranscript {
        from_account: sender.clone(),
        to_account: receiver.clone(),
        asset_definition: asset_definition.clone(),
        amount: Numeric::from(amount),
        from_balance_before: Numeric::from(sender_before),
        from_balance_after: Numeric::from(sender_after),
        to_balance_before: Numeric::from(receiver_before),
        to_balance_after: Numeric::from(receiver_after),
        from_merkle_proof: None,
        to_merkle_proof: None,
    };
    let mut batch_hash_payload = Vec::new();
    batch_hash_payload.extend_from_slice(label.as_bytes());
    batch_hash_payload.extend_from_slice(binding.corridor.as_bytes());
    batch_hash_payload.extend_from_slice(source_tx_commitment);
    batch_hash_payload.extend_from_slice(claim_digest);
    let batch_hash = Hash::new(batch_hash_payload);
    let digest = crate::gadgets::transfer::compute_poseidon_digest(&delta, &batch_hash);
    let transcript = TransferTranscript {
        batch_hash,
        deltas: vec![delta],
        authority_digest: Hash::new(
            [
                label.as_bytes(),
                binding.verifier_id.as_bytes(),
                binding.verifier_version.as_bytes(),
            ]
            .concat(),
        ),
        poseidon_preimage_digest: Some(digest),
    };
    Ok(TransferWitnessBundle {
        sender: StateTransition::new(
            format!("asset/{asset_definition}/{sender}").into_bytes(),
            encode_u64(sender_before),
            encode_u64(sender_after),
            OperationKind::Transfer,
        ),
        receiver: StateTransition::new(
            format!("asset/{asset_definition}/{receiver}").into_bytes(),
            encode_u64(receiver_before),
            encode_u64(receiver_after),
            OperationKind::Transfer,
        ),
        transcript,
    })
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

fn normalized_verifier_id(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        "fastpq".to_string()
    } else {
        trimmed.to_string()
    }
}

fn normalized_verifier_version(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        "v1".to_string()
    } else {
        trimmed.to_string()
    }
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

fn deterministic_account(label: &str, entropy: &[u8; 32]) -> AccountId {
    let key_seed = Hash::new([label.as_bytes(), entropy].concat());
    let mut seed = [0u8; 32];
    seed.copy_from_slice(&key_seed.as_ref()[..32]);
    let keypair = iroha_crypto::KeyPair::from_seed(seed.to_vec(), Algorithm::default());
    AccountId::new(keypair.public_key().clone())
}

fn key_bytes(prefix: &str, primary: &[u8; 32], suffix: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(prefix.len() + primary.len() + suffix.len() + 2);
    bytes.extend_from_slice(prefix.as_bytes());
    bytes.push(b'/');
    bytes.extend_from_slice(primary);
    bytes.push(b'/');
    bytes.extend_from_slice(suffix);
    bytes
}

fn encode_u64(value: u64) -> Vec<u8> {
    value.to_le_bytes().to_vec()
}

fn bounded_amount(digest: &[u8; 32], min: u64, span: u64) -> u64 {
    let raw = u64::from_le_bytes(digest[..8].try_into().expect("digest slice"));
    min.saturating_add(raw % span.max(1))
}

fn decode_hex_digest(value: &str, field: &str) -> Result<[u8; 32]> {
    let decoded = hex::decode(value).map_err(|err| Error::InvalidAxtBinding {
        details: format!("{field} is not valid hex: {err}"),
    })?;
    decoded.try_into().map_err(|_| Error::InvalidAxtBinding {
        details: format!("{field} must be exactly 32 bytes"),
    })
}

fn digest32(domain: &[u8], parts: &[&[u8]]) -> [u8; 32] {
    let mut payload = Vec::new();
    payload.extend_from_slice(domain);
    for part in parts {
        payload.extend_from_slice(part);
    }
    Hash::new(payload).into()
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
