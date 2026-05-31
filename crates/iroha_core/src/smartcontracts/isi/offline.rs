//! Offline note instruction execution.

use super::prelude::*;
use crate::smartcontracts::isi::asset::isi::assert_numeric_spec_with;
use std::collections::BTreeSet;

use iroha_crypto::{Algorithm, Hash, PublicKey};
use iroha_data_model::{
    account::AccountId,
    asset::{AssetDefinitionId, AssetId},
    confidential::ConfidentialStatus,
    events::data::prelude::{
        OfflineNoteAuditRecorded, OfflineNoteEvent, OfflineNoteIssued, OfflineNoteRedeemed,
    },
    isi::{
        error::{InstructionExecutionError, MathError},
        offline::{AuditOfflineNote, IssueOfflineNote, KagemushaTransfer, RedeemOfflineNote},
    },
    offline::{
        OFFLINE_NOTE_RECURSIVE_PUBLIC_INPUTS_SCHEMA, OFFLINE_REJECTION_REASON_PREFIX,
        OfflineNoteAuditOutputClaim, OfflineNoteIssuedClaim, OfflineNoteKeyCertificate,
        OfflineNoteRecursiveProof, offline_note_recursive_public_inputs_schema_hash,
    },
    proof::{ProofAttachment, ProofBox, VerifyingKeyBox, VerifyingKeyId, VerifyingKeyRecord},
    zk::{BackendTag, OpenVerifyEnvelope, StarkFriOpenProofV1},
};
use iroha_primitives::numeric::Numeric;

const CAN_MANAGE_OFFLINE_ESCROW_PERMISSION: &str = "CanManageOfflineEscrow";

fn labeled_invariant(label: &str, message: impl Into<String>) -> InstructionExecutionError {
    let message = message.into();
    let boxed: Box<str> = format!("{OFFLINE_REJECTION_REASON_PREFIX}{label}:{message}").into();
    InstructionExecutionError::InvariantViolation(boxed)
}

fn resolve_offline_escrow_account(
    state_transaction: &mut StateTransaction<'_, '_>,
    definition: &AssetDefinitionId,
) -> Result<Option<AccountId>, Error> {
    let asset_definition = state_transaction.world.asset_definition(definition)?;
    if crate::smartcontracts::isi::domain::isi::asset_definition_offline_enabled(
        asset_definition.metadata(),
    )? {
        crate::smartcontracts::isi::domain::isi::ensure_offline_escrow_account(
            &asset_definition,
            asset_definition.owned_by(),
            state_transaction,
        )?;
        let derived = crate::smartcontracts::isi::domain::isi::offline_escrow_account_id(
            state_transaction.chain_id(),
            definition,
        );
        return Ok(Some(derived));
    }
    if let Some(account) = state_transaction
        .settlement
        .offline
        .escrow_accounts
        .get(definition)
    {
        return Ok(Some(account.clone()));
    }
    if state_transaction.settlement.offline.escrow_required {
        return Err(labeled_invariant(
            "escrow_missing",
            format!("offline escrow account not configured for asset definition `{definition}`"),
        )
        .into());
    }
    Ok(None)
}

pub(crate) fn is_offline_escrow_source_asset(
    state_transaction: &StateTransaction<'_, '_>,
    source_id: &AssetId,
) -> Result<bool, Error> {
    let asset_definition = state_transaction
        .world
        .asset_definition(source_id.definition())?;

    if crate::smartcontracts::isi::domain::isi::asset_definition_offline_enabled(
        asset_definition.metadata(),
    )? {
        let derived = crate::smartcontracts::isi::domain::isi::offline_escrow_account_id(
            state_transaction.chain_id(),
            source_id.definition(),
        );
        return Ok(&derived == source_id.account());
    }

    if let Some(account) = state_transaction
        .settlement
        .offline
        .escrow_accounts
        .get(source_id.definition())
    {
        return Ok(account == source_id.account());
    }
    Ok(false)
}

fn ensure_distinct_offline_escrow_account(
    escrow_account: &AccountId,
    participant_account: &AccountId,
    participant_role: &str,
    definition_id: &AssetDefinitionId,
) -> Result<(), Error> {
    if escrow_account == participant_account {
        return Err(labeled_invariant(
            "escrow_self_reference",
            format!(
                "offline escrow account for asset definition `{definition_id}` must be distinct from {participant_role} account `{participant_account}`",
            ),
        )
        .into());
    }
    Ok(())
}

fn reserve_offline_note_escrow(
    state_transaction: &mut StateTransaction<'_, '_>,
    asset: &AssetId,
    amount: &Numeric,
) -> Result<(), Error> {
    let escrow_account = resolve_offline_escrow_account(state_transaction, asset.definition())?;
    let escrow_account = escrow_account.ok_or_else(|| {
        labeled_invariant(
            "escrow_missing",
            format!(
                "offline escrow account not configured for asset definition `{}`",
                asset.definition(),
            ),
        )
    })?;
    if amount.is_zero() {
        return Ok(());
    }
    ensure_distinct_offline_escrow_account(
        &escrow_account,
        asset.account(),
        "note",
        asset.definition(),
    )?;
    let escrow_asset = AssetId::new(asset.definition().clone(), escrow_account);
    state_transaction
        .world
        .withdraw_numeric_asset(asset, amount)?;
    state_transaction
        .world
        .deposit_numeric_asset(&escrow_asset, amount)
}

fn credit_from_offline_note_escrow(
    state_transaction: &mut StateTransaction<'_, '_>,
    asset: &AssetId,
    recipient: &AccountId,
    amount: &Numeric,
) -> Result<(), Error> {
    let definition_id = asset.definition().clone();
    let recipient_asset = AssetId::new(definition_id.clone(), recipient.clone());
    let spec = state_transaction.numeric_spec_for(&definition_id)?;
    assert_numeric_spec_with(amount, spec)?;
    state_transaction.world.account(recipient)?;
    if !amount.is_zero() {
        let current_balance = state_transaction
            .world
            .assets
            .get(&recipient_asset)
            .map(|asset| asset.as_ref().clone())
            .unwrap_or_else(Numeric::zero);
        current_balance
            .checked_add(amount.clone())
            .ok_or(MathError::Overflow)?;
    }
    let escrow_account = resolve_offline_escrow_account(state_transaction, &definition_id)?;
    let escrow_account = escrow_account.ok_or_else(|| {
        labeled_invariant(
            "escrow_missing",
            format!("offline escrow account not configured for asset definition `{definition_id}`"),
        )
    })?;
    if amount.is_zero() {
        return state_transaction
            .world
            .deposit_numeric_asset(&recipient_asset, amount);
    }
    ensure_distinct_offline_escrow_account(
        &escrow_account,
        recipient,
        "recipient",
        &definition_id,
    )?;
    let escrow_asset = AssetId::new(definition_id, escrow_account);
    state_transaction
        .world
        .withdraw_numeric_asset(&escrow_asset, amount)?;
    if let Err(err) = state_transaction
        .world
        .deposit_numeric_asset(&recipient_asset, amount)
    {
        state_transaction
            .world
            .deposit_numeric_asset(&escrow_asset, amount)
            .expect("escrow refund must succeed after failed deposit credit");
        return Err(err);
    }
    Ok(())
}

/// Execution logic for Offline note instructions.
pub mod isi {
    use super::*;

    const OFFLINE_NOTE_VERIFIER_NAMESPACE: &str = "offline_note";
    const OFFLINE_NOTE_REPLAY_ISSUE_DOMAIN: &str = "offline-note-issued-note";
    const OFFLINE_NOTE_REPLAY_KEY_CERTIFICATE_DOMAIN: &str = "offline-note-issued-key-certificate";
    const OFFLINE_NOTE_REPLAY_ISSUED_CLAIM_DOMAIN: &str = "offline-note-issued-claim";
    const OFFLINE_NOTE_REPLAY_SPENT_CLAIM_DOMAIN: &str = "offline-note-spent-claim";
    const OFFLINE_NOTE_REPLAY_NULLIFIER_DOMAIN: &str = "offline-note-spent-nullifier";
    const OFFLINE_NOTE_REPLAY_AUDIT_TOKEN_DOMAIN: &str = "offline-note-audit-token";
    const OFFLINE_NOTE_REPLAY_AUDIT_RECORD_DOMAIN: &str = "offline-note-audit-record";
    const OFFLINE_NOTE_REPLAY_AUDIT_NULLIFIER_DOMAIN: &str = "offline-note-audit-nullifier";
    const OFFLINE_NOTE_REPLAY_AUDIT_OUTPUT_DOMAIN: &str = "offline-note-audit-output";

    fn validate_offline_note_key_certificate(
        certificate: &OfflineNoteKeyCertificate,
    ) -> Result<(), InstructionExecutionError> {
        if certificate.version != iroha_data_model::offline::OFFLINE_NOTE_KEY_CERTIFICATE_VERSION
            || !certificate.one_use
        {
            return Err(labeled_invariant(
                "invalid_issuer_cert",
                "offline note operation requires a compact one-use key certificate",
            ));
        }
        if certificate.public_key.is_empty() {
            return Err(labeled_invariant(
                "invalid_issuer_cert",
                "offline note certificate public key must be non-empty",
            ));
        }
        PublicKey::from_bytes(Algorithm::Ed25519, &certificate.public_key).map_err(|_| {
            labeled_invariant(
                "invalid_issuer_cert",
                "offline note certificate public key must be an Ed25519 public key",
            )
        })?;
        Ok(())
    }

    fn is_offline_note_transparent_backend(backend: &str) -> bool {
        backend == crate::zk::ZK_BACKEND_HALO2_IPA || crate::zk::is_stark_fri_v1_backend(backend)
    }

    fn ensure_offline_note_transparent_backend(
        backend: &str,
        backend_tag: BackendTag,
    ) -> Result<(), Error> {
        if !is_offline_note_transparent_backend(backend) {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "offline recursive proofs require a transparent halo2/ipa or stark/fri backend",
            )
            .into());
        }
        let expected_tag = if backend == crate::zk::ZK_BACKEND_HALO2_IPA {
            BackendTag::Halo2IpaPasta
        } else {
            BackendTag::Stark
        };
        if backend_tag != expected_tag {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "offline recursive verifier backend tag does not match the transparent backend",
            )
            .into());
        }
        Ok(())
    }

    fn ensure_kagemusha_transparent_attachment(attachment: &ProofAttachment) -> Result<(), Error> {
        if attachment.backend != attachment.proof.backend
            || attachment.backend != attachment.vk_ref.backend
        {
            return Err(labeled_invariant(
                "proof_binding",
                "Kagemusha proof backend, proof payload backend, and verifier key backend must match",
            )
            .into());
        }
        if attachment.vk_ref.name.trim().is_empty() {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "Kagemusha proof verifier key id name must be non-empty",
            )
            .into());
        }
        let backend = attachment.backend.as_str();
        let backend_tag = if backend == crate::zk::ZK_BACKEND_HALO2_IPA {
            BackendTag::Halo2IpaPasta
        } else if crate::zk::is_stark_fri_v1_backend(backend) {
            BackendTag::Stark
        } else {
            BackendTag::Unsupported
        };
        ensure_offline_note_transparent_backend(backend, backend_tag)
    }

    fn ensure_kagemusha_transfer_verifier_binding(
        asset: &AssetDefinitionId,
        proof: &ProofAttachment,
        root_hint: Option<[u8; 32]>,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let zk_state = state_transaction
            .world
            .zk_assets
            .get(asset)
            .ok_or_else(|| {
                labeled_invariant(
                    "verifier_key_invalid",
                    "Kagemusha transfers require a configured shielded asset verifier binding",
                )
            })?;
        let binding = zk_state.vk_transfer.as_ref().ok_or_else(|| {
            labeled_invariant(
                "verifier_key_invalid",
                "Kagemusha transfers require a bound confidential transfer verifier key",
            )
        })?;
        if proof.vk_ref != binding.id || proof.backend != binding.id.backend {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "Kagemusha transfer proof must reference the asset-bound verifier key",
            )
            .into());
        }
        let Some(commitment) = proof.vk_commitment else {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "Kagemusha transfer proof must publish the asset-bound verifier-key commitment",
            )
            .into());
        };
        if commitment != binding.commitment {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "Kagemusha transfer verifier-key commitment does not match the asset binding",
            )
            .into());
        }

        let record = state_transaction
            .world
            .verifying_keys
            .get(&binding.id)
            .ok_or_else(|| {
                labeled_invariant(
                    "verifier_key_invalid",
                    "Kagemusha transfer verifier key is not registered",
                )
            })?;
        if record.status != ConfidentialStatus::Active {
            return Err(labeled_invariant(
                "verifier_key_inactive",
                "Kagemusha transfer verifier key is not active",
            )
            .into());
        }
        if record.namespace != crate::zk::KAGEMUSHA_VERIFIER_NAMESPACE {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "Kagemusha transfer verifier key is not in the Kagemusha namespace",
            )
            .into());
        }
        let circuit_key = (record.circuit_id.clone(), record.version);
        match state_transaction
            .world
            .verifying_keys_by_circuit
            .get(&circuit_key)
        {
            Some(active_id) if active_id == &binding.id => {}
            _ => {
                return Err(labeled_invariant(
                    "verifier_key_inactive",
                    "Kagemusha transfer verifier circuit/version is not active",
                )
                .into());
            }
        }
        if record.commitment != binding.commitment {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "Kagemusha transfer verifier-key registry commitment does not match the asset binding",
            )
            .into());
        }
        if record.backend != BackendTag::Halo2IpaPasta
            || proof.backend.as_str() != crate::zk::ZK_BACKEND_HALO2_IPA
            || record.circuit_id != crate::zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_CIRCUIT_ID
        {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "Kagemusha transfers require the canonical transparent confidential-transfer-v2 Halo2/IPA verifier",
            )
            .into());
        }
        let expected_schema_hash: [u8; 32] =
            Hash::new(crate::zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_PUBLIC_INPUTS_SCHEMA_V1)
                .into();
        if record.public_inputs_schema_hash != expected_schema_hash {
            return Err(labeled_invariant(
                "verifier_schema_mismatch",
                "Kagemusha transfer verifier key uses an unexpected public-input schema",
            )
            .into());
        }
        if record.max_proof_bytes == 0 {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "Kagemusha transfer verifier key must publish a non-zero max_proof_bytes cap",
            )
            .into());
        }
        if proof.proof.bytes.len() > record.max_proof_bytes as usize {
            return Err(labeled_invariant(
                "invalid_proof",
                "Kagemusha transfer proof exceeds verifier record max_proof_bytes",
            )
            .into());
        }
        let vk_box = record.key.as_ref().ok_or_else(|| {
            labeled_invariant(
                "verifier_key_invalid",
                "Kagemusha transfer verifier key is not available inline",
            )
        })?;
        if vk_box.backend.as_str() != crate::zk::ZK_BACKEND_HALO2_IPA {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "Kagemusha transfer inline verifier key backend does not match Halo2/IPA",
            )
            .into());
        }
        if vk_box.bytes.is_empty() {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "Kagemusha transfer verifier key bytes must be non-empty",
            )
            .into());
        }
        if u32::try_from(vk_box.bytes.len()).ok() != Some(record.vk_len) {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "Kagemusha transfer verifier key length mismatch",
            )
            .into());
        }
        if crate::zk::hash_vk(vk_box) != record.commitment {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "Kagemusha transfer inline verifier-key commitment mismatch",
            )
            .into());
        }
        crate::zk::confidential_v2::ensure_confidential_transfer_v2_canonical_vk_box(vk_box)
            .map_err(|err| labeled_invariant("verifier_key_invalid", err))?;
        let envelope: OpenVerifyEnvelope =
            norito::decode_from_bytes(&proof.proof.bytes).map_err(|_| {
                labeled_invariant(
                    "invalid_proof",
                    "Kagemusha transfer proof must be an OpenVerifyEnvelope",
                )
            })?;
        if envelope.backend != BackendTag::Halo2IpaPasta {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "Kagemusha transfer proof envelope backend does not match Halo2/IPA Pasta",
            )
            .into());
        }
        if envelope.circuit_id != crate::zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_CIRCUIT_ID
            || envelope.circuit_id != record.circuit_id
        {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "Kagemusha transfer proof envelope must use the canonical asset-bound confidential-transfer-v2 circuit",
            )
            .into());
        }
        if envelope.public_inputs
            != crate::zk::confidential_v2::CONFIDENTIAL_TRANSFER_V2_PUBLIC_INPUTS_SCHEMA_V1
        {
            return Err(labeled_invariant(
                "verifier_schema_mismatch",
                "Kagemusha transfer proof envelope public-input schema mismatch",
            )
            .into());
        }
        if !envelope.aux.is_empty() {
            return Err(labeled_invariant(
                "invalid_proof",
                "Kagemusha transfer proof envelope must have empty auxiliary bytes",
            )
            .into());
        }
        if envelope.vk_hash != binding.commitment {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "Kagemusha transfer proof envelope verifier-key hash does not match the asset binding",
            )
            .into());
        }
        if let Some(envelope_hash) = proof.envelope_hash {
            let expected_hash: [u8; 32] = Hash::new(&proof.proof.bytes).into();
            if envelope_hash != expected_hash {
                return Err(labeled_invariant(
                    "invalid_proof",
                    "Kagemusha transfer proof envelope hash does not match the submitted envelope",
                )
                .into());
            }
        }
        if root_hint.is_none() {
            return Err(labeled_invariant(
                "invalid_proof",
                "Kagemusha confidential transfers require a root hint",
            )
            .into());
        }
        Ok(())
    }

    fn offline_note_public_instances_from_envelope(
        proof: &ProofBox,
        envelope: &OpenVerifyEnvelope,
    ) -> Result<Vec<Vec<[u8; 32]>>, Error> {
        match envelope.backend {
            BackendTag::Halo2IpaPasta => crate::zk::extract_pasta_instance_columns_bytes(
                &envelope.proof_bytes,
            )
            .ok_or_else(|| {
                labeled_invariant(
                    "invalid_proof",
                    "offline recursive proof does not expose Halo2 public instances",
                )
                .into()
            }),
            BackendTag::Stark => {
                if !crate::zk::is_stark_fri_v1_backend(proof.backend.as_str()) {
                    return Err(labeled_invariant(
                        "invalid_proof",
                        "offline recursive proof Stark backend is unsupported",
                    )
                    .into());
                }
                let open: StarkFriOpenProofV1 = norito::decode_from_bytes(&envelope.proof_bytes)
                    .map_err(|_| {
                        labeled_invariant(
                            "invalid_proof",
                            "offline recursive proof has invalid STARK public inputs",
                        )
                    })?;
                Ok(open.public_inputs)
            }
            _ => Err(labeled_invariant(
                "invalid_proof",
                "offline recursive proof backend is unsupported",
            )
            .into()),
        }
    }

    fn offline_note_resolve_verifier(
        proof: &OfflineNoteRecursiveProof,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<(VerifyingKeyRecord, VerifyingKeyBox, OpenVerifyEnvelope), Error> {
        let verifier_id: &VerifyingKeyId = &proof.verifier_key_id;
        if proof.proof.backend != verifier_id.backend {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "offline recursive proof backend does not match verifier key id",
            )
            .into());
        }
        if verifier_id.name.trim().is_empty() {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "offline recursive verifier key id name must be non-empty",
            )
            .into());
        }
        let backend = verifier_id.backend.as_str();
        if crate::zk::is_trusted_setup_backend_label(backend)
            || crate::zk::is_developer_only_backend_label(backend)
        {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "offline recursive proofs may not use trusted-setup or developer-only proof backends",
            )
            .into());
        }
        let backend_tag = if backend == crate::zk::ZK_BACKEND_HALO2_IPA {
            BackendTag::Halo2IpaPasta
        } else if crate::zk::is_stark_fri_v1_backend(backend) {
            BackendTag::Stark
        } else {
            BackendTag::Unsupported
        };
        ensure_offline_note_transparent_backend(backend, backend_tag)?;
        if proof.proof.bytes.is_empty() {
            return Err(labeled_invariant(
                "invalid_proof",
                "offline recursive proof must not be empty",
            )
            .into());
        }

        let record = state_transaction
            .world
            .verifying_keys
            .get(verifier_id)
            .cloned()
            .ok_or_else(|| {
                labeled_invariant(
                    "verifier_key_invalid",
                    "offline recursive verifier key is not registered",
                )
            })?;
        if record.status != ConfidentialStatus::Active {
            return Err(labeled_invariant(
                "verifier_key_inactive",
                "offline recursive verifier key is not active",
            )
            .into());
        }
        ensure_offline_note_transparent_backend(backend, record.backend)?;
        if record.namespace != OFFLINE_NOTE_VERIFIER_NAMESPACE {
            return Err(labeled_invariant(
                "verifier_schema_mismatch",
                "offline recursive verifier key is not in the Offline namespace",
            )
            .into());
        }
        if record.public_inputs_schema_hash != offline_note_recursive_public_inputs_schema_hash() {
            return Err(labeled_invariant(
                "verifier_schema_mismatch",
                "offline recursive verifier key uses an unexpected public-input schema",
            )
            .into());
        }
        if record.circuit_id != crate::zk::OFFLINE_NOTE_RECURSIVE_CIRCUIT_ID {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "offline recursive verifier must use the canonical offline-note-recursive circuit",
            )
            .into());
        }
        if record.max_proof_bytes == 0 {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "offline recursive verifier key must set max_proof_bytes",
            )
            .into());
        }
        if proof.proof.bytes.len() > record.max_proof_bytes as usize {
            return Err(labeled_invariant(
                "invalid_proof",
                "offline recursive proof exceeds verifier max_proof_bytes",
            )
            .into());
        }
        let circuit_key = (record.circuit_id.clone(), record.version);
        match state_transaction
            .world
            .verifying_keys_by_circuit
            .get(&circuit_key)
        {
            Some(active_id) if active_id == verifier_id => {}
            _ => {
                return Err(labeled_invariant(
                    "verifier_key_inactive",
                    "offline recursive verifier circuit/version is not active",
                )
                .into());
            }
        }

        let vk_box = record.key.clone().ok_or_else(|| {
            labeled_invariant(
                "verifier_key_invalid",
                "offline recursive verifier key bytes are not available inline",
            )
        })?;
        if vk_box.bytes.is_empty() {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "offline recursive verifier key bytes must be non-empty",
            )
            .into());
        }
        if vk_box.backend != verifier_id.backend || vk_box.backend != proof.proof.backend {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "offline recursive verifier backend mismatch",
            )
            .into());
        }
        if u32::try_from(vk_box.bytes.len()).ok() != Some(record.vk_len) {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "offline recursive verifier key length mismatch",
            )
            .into());
        }
        if crate::zk::hash_vk(&vk_box) != record.commitment {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "offline recursive verifier commitment mismatch",
            )
            .into());
        }
        crate::zk::ensure_offline_note_recursive_canonical_vk_box(&vk_box)
            .map_err(|err| labeled_invariant("verifier_key_invalid", err))?;

        let envelope: OpenVerifyEnvelope =
            norito::decode_from_bytes(&proof.proof.bytes).map_err(|_| {
                labeled_invariant(
                    "invalid_proof",
                    "offline recursive proof must be an OpenVerifyEnvelope",
                )
            })?;
        if envelope.backend != record.backend {
            return Err(labeled_invariant(
                "invalid_proof",
                "offline recursive proof envelope backend mismatch",
            )
            .into());
        }
        if envelope.circuit_id != record.circuit_id {
            return Err(labeled_invariant(
                "invalid_proof",
                "offline recursive proof circuit id mismatch",
            )
            .into());
        }
        if envelope.vk_hash != record.commitment {
            return Err(labeled_invariant(
                "invalid_proof",
                "offline recursive proof verifier commitment mismatch",
            )
            .into());
        }
        if !envelope.aux.is_empty() {
            return Err(labeled_invariant(
                "invalid_proof",
                "offline recursive proof envelope must have empty auxiliary bytes",
            )
            .into());
        }
        if envelope.public_inputs != OFFLINE_NOTE_RECURSIVE_PUBLIC_INPUTS_SCHEMA {
            return Err(labeled_invariant(
                "verifier_schema_mismatch",
                "offline recursive proof public-input schema mismatch",
            )
            .into());
        }

        Ok((record, vk_box, envelope))
    }

    fn verify_offline_note_recursive_proof(
        proof: &OfflineNoteRecursiveProof,
        expected_public_inputs_hash: &Hash,
        expected_public_instances: Vec<Vec<[u8; 32]>>,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        if &proof.public_inputs_hash != expected_public_inputs_hash {
            return Err(labeled_invariant(
                "proof_binding",
                "offline recursive proof is not bound to expected public inputs",
            )
            .into());
        }

        let (_record, vk_box, envelope) = offline_note_resolve_verifier(proof, state_transaction)?;
        let actual_instances =
            offline_note_public_instances_from_envelope(&proof.proof, &envelope)?;
        if actual_instances != expected_public_instances {
            return Err(labeled_invariant(
                "proof_binding",
                "offline recursive proof public instances do not match expected public inputs",
            )
            .into());
        }

        state_transaction.register_confidential_proof(proof.proof.bytes.len())?;
        let report = crate::zk::verify_backend_with_timing_checked(
            proof.proof.backend.as_str(),
            &proof.proof,
            Some(&vk_box),
            &state_transaction.zk,
        );
        if !report.ok {
            return Err(labeled_invariant(
                "invalid_proof",
                "offline recursive proof verification failed",
            )
            .into());
        }
        Ok(())
    }

    fn offline_note_replay_key(domain: &str, value: &Hash) -> Hash {
        let mut preimage = Vec::with_capacity(domain.len() + Hash::LENGTH + 1);
        preimage.extend_from_slice(domain.as_bytes());
        preimage.push(b':');
        preimage.extend_from_slice(value.as_ref());
        Hash::new(&preimage)
    }

    fn offline_note_issue_key(note_commitment: &Hash) -> Hash {
        offline_note_replay_key(OFFLINE_NOTE_REPLAY_ISSUE_DOMAIN, note_commitment)
    }

    fn offline_note_key_certificate_key(certificate_payload_hash: &Hash) -> Hash {
        offline_note_replay_key(
            OFFLINE_NOTE_REPLAY_KEY_CERTIFICATE_DOMAIN,
            certificate_payload_hash,
        )
    }

    fn offline_note_issued_claim_key(claim_hash: &Hash) -> Hash {
        offline_note_replay_key(OFFLINE_NOTE_REPLAY_ISSUED_CLAIM_DOMAIN, claim_hash)
    }

    fn offline_note_spent_claim_key(claim_hash: &Hash) -> Hash {
        offline_note_replay_key(OFFLINE_NOTE_REPLAY_SPENT_CLAIM_DOMAIN, claim_hash)
    }

    fn offline_note_nullifier_key(nullifier: &Hash) -> Hash {
        offline_note_replay_key(OFFLINE_NOTE_REPLAY_NULLIFIER_DOMAIN, nullifier)
    }

    fn offline_note_audit_token_key(token_id: &Hash) -> Hash {
        offline_note_replay_key(OFFLINE_NOTE_REPLAY_AUDIT_TOKEN_DOMAIN, token_id)
    }

    fn offline_note_audit_record_key(public_inputs_hash: &Hash) -> Hash {
        offline_note_replay_key(OFFLINE_NOTE_REPLAY_AUDIT_RECORD_DOMAIN, public_inputs_hash)
    }

    fn offline_note_audit_nullifier_key(nullifier: &Hash) -> Hash {
        offline_note_replay_key(OFFLINE_NOTE_REPLAY_AUDIT_NULLIFIER_DOMAIN, nullifier)
    }

    fn offline_note_audit_output_key(output_commitment: &Hash) -> Hash {
        offline_note_replay_key(OFFLINE_NOTE_REPLAY_AUDIT_OUTPUT_DOMAIN, output_commitment)
    }

    fn ensure_unique_hashes(
        hashes: &[Hash],
        label: &'static str,
        message: &'static str,
    ) -> Result<(), InstructionExecutionError> {
        let mut seen = BTreeSet::new();
        for hash in hashes {
            if !seen.insert(*hash) {
                return Err(labeled_invariant(label, message));
            }
        }
        Ok(())
    }

    fn ensure_unique_bytes32(
        values: &[[u8; 32]],
        label: &'static str,
        message: &'static str,
    ) -> Result<(), InstructionExecutionError> {
        let mut seen = BTreeSet::new();
        for value in values {
            if !seen.insert(*value) {
                return Err(labeled_invariant(label, message));
            }
        }
        Ok(())
    }

    fn ensure_non_zero_bytes32(
        values: &[[u8; 32]],
        label: &'static str,
        message: &'static str,
    ) -> Result<(), InstructionExecutionError> {
        for value in values {
            if *value == [0u8; 32] {
                return Err(labeled_invariant(label, message));
            }
        }
        Ok(())
    }

    fn ensure_offline_audit_output_claim_count(
        output_commitments_len: usize,
        output_claims_len: usize,
    ) -> Result<(), InstructionExecutionError> {
        if output_claims_len == 0 || output_claims_len != output_commitments_len {
            return Err(labeled_invariant(
                "invalid_proof",
                "offline audit requires output claims to match output commitments one-to-one",
            ));
        }
        Ok(())
    }

    fn ensure_offline_audit_output_claim_binding(
        output_commitments: &[Hash],
        output_claims: &[OfflineNoteAuditOutputClaim],
    ) -> Result<(), InstructionExecutionError> {
        ensure_offline_audit_output_claim_count(output_commitments.len(), output_claims.len())?;
        for (commitment, claim) in output_commitments.iter().zip(output_claims) {
            if commitment != &claim.note_commitment {
                return Err(labeled_invariant(
                    "proof_binding",
                    "offline audit output claims must be ordered one-to-one with output commitments",
                ));
            }
        }
        Ok(())
    }

    fn ensure_offline_audit_input_claim_anchor(
        input_claims: &[OfflineNoteIssuedClaim],
        certificate_payload_hash: &Hash,
    ) -> Result<(), InstructionExecutionError> {
        for claim in input_claims {
            if &claim.key_certificate_payload_hash != certificate_payload_hash {
                return Err(labeled_invariant(
                    "proof_binding",
                    "offline audit input claim is not anchored to the sender key certificate",
                ));
            }
        }
        Ok(())
    }

    fn ensure_offline_audit_conserves_asset_amounts(
        input_claims: &[OfflineNoteIssuedClaim],
        output_claims: &[OfflineNoteAuditOutputClaim],
    ) -> Result<(), Error> {
        let input_definition = input_claims
            .first()
            .ok_or_else(|| labeled_invariant("invalid_proof", "offline audit requires inputs"))?
            .asset
            .definition();

        let mut input_total = Numeric::zero();
        for claim in input_claims {
            if claim.amount <= Numeric::zero() {
                return Err(labeled_invariant(
                    "invalid_amount",
                    "offline audit input claim amount must be positive",
                )
                .into());
            }
            if claim.asset.definition() != input_definition {
                return Err(labeled_invariant(
                    "asset_mismatch",
                    "offline audit input claims must use one asset definition",
                )
                .into());
            }
            input_total = input_total
                .checked_add(claim.amount.clone())
                .ok_or(MathError::Overflow)?;
        }

        let mut output_total = Numeric::zero();
        for claim in output_claims {
            if claim.amount <= Numeric::zero() {
                return Err(labeled_invariant(
                    "invalid_amount",
                    "offline audit output claim amount must be positive",
                )
                .into());
            }
            if claim.asset.definition() != input_definition {
                return Err(labeled_invariant(
                    "asset_mismatch",
                    "offline audit output claims must use the input asset definition",
                )
                .into());
            }
            output_total = output_total
                .checked_add(claim.amount.clone())
                .ok_or(MathError::Overflow)?;
        }

        if input_total != output_total {
            return Err(labeled_invariant(
                "amount_conservation",
                "offline audit input amount total must equal output amount total",
            )
            .into());
        }
        Ok(())
    }

    fn is_offline_escrow_manager(
        authority: &AccountId,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> bool {
        state_transaction
            .world
            .account_permissions
            .get(authority)
            .is_some_and(|perms| {
                perms
                    .iter()
                    .any(|permission| permission.name() == CAN_MANAGE_OFFLINE_ESCROW_PERMISSION)
            })
    }

    fn ensure_can_submit_offline_note_for_account(
        account: &AccountId,
        authority: &AccountId,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        if account == authority || is_offline_escrow_manager(authority, state_transaction) {
            Ok(())
        } else {
            Err(labeled_invariant(
                "unauthorized_controller",
                "only the note account or an offline escrow manager may submit Offline notes",
            )
            .into())
        }
    }

    fn ensure_can_issue_offline_note(
        authority: &AccountId,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        if is_offline_escrow_manager(authority, state_transaction) {
            Ok(())
        } else {
            Err(labeled_invariant(
                "unauthorized_controller",
                "only an offline escrow manager may issue Offline notes",
            )
            .into())
        }
    }

    fn ensure_offline_note_certificate_signature(
        certificate: &OfflineNoteKeyCertificate,
        issuer: &AccountId,
    ) -> Result<(), Error> {
        let payload = certificate.signing_bytes().map_err(|err| {
            labeled_invariant(
                "invalid_issuer_cert",
                format!("failed to encode Offline key certificate payload: {err}"),
            )
        })?;
        let issuer_key = issuer.try_signatory().ok_or_else(|| {
            labeled_invariant(
                "invalid_issuer_cert",
                "offline note issuer account must be single-signature",
            )
        })?;
        certificate
            .issuer_signature
            .verify(issuer_key, &payload)
            .map_err(|_| {
                labeled_invariant(
                    "invalid_issuer_cert",
                    "offline key certificate signature does not match issuer account",
                )
                .into()
            })
    }

    fn offline_note_issued_claim_hash(claim: OfflineNoteIssuedClaim) -> Result<Hash, Error> {
        claim.claim_hash().map_err(|err| {
            labeled_invariant(
                "invalid_proof",
                format!("failed to encode Offline issued-note claim: {err}"),
            )
            .into()
        })
    }

    impl Execute for IssueOfflineNote {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let issue = self.issue;
            validate_offline_note_key_certificate(&issue.key_certificate)?;
            if issue.amount <= Numeric::zero() {
                return Err(labeled_invariant(
                    "invalid_amount",
                    "offline note issue amount must be positive",
                )
                .into());
            }
            if issue.key_certificate.account_id != *issue.asset.account() {
                return Err(labeled_invariant(
                    "invalid_issuer_cert",
                    "offline note issue certificate account must match the debited asset owner",
                )
                .into());
            }
            ensure_can_issue_offline_note(authority, state_transaction)?;
            ensure_offline_note_certificate_signature(&issue.key_certificate, authority)?;
            let spec = state_transaction.numeric_spec_for(issue.asset.definition())?;
            assert_numeric_spec_with(&issue.amount, spec)?;
            let certificate_payload_hash = issue.key_certificate.payload_hash().map_err(|err| {
                labeled_invariant(
                    "invalid_issuer_cert",
                    format!("failed to encode Offline key certificate payload: {err}"),
                )
            })?;
            let issue_key = offline_note_issue_key(&issue.note_commitment);
            let audit_output_key = offline_note_audit_output_key(&issue.note_commitment);
            let certificate_key = offline_note_key_certificate_key(&certificate_payload_hash);
            let issued_claim_hash = offline_note_issued_claim_hash(
                OfflineNoteIssuedClaim::from_issue(&issue).map_err(|err| {
                    labeled_invariant(
                        "invalid_proof",
                        format!("failed to encode Offline issued-note claim: {err}"),
                    )
                })?,
            )?;
            let issued_claim_key = offline_note_issued_claim_key(&issued_claim_hash);
            if state_transaction
                .world
                .offline_note_replay_keys
                .get(&issue_key)
                .is_some()
            {
                return Err(labeled_invariant(
                    "duplicate_issue",
                    "offline note commitment is already issued",
                )
                .into());
            }
            if state_transaction
                .world
                .offline_note_replay_keys
                .get(&audit_output_key)
                .is_some()
            {
                return Err(labeled_invariant(
                    "duplicate_issue",
                    "offline note commitment is already issued",
                )
                .into());
            }
            if state_transaction
                .world
                .offline_note_replay_keys
                .get(&certificate_key)
                .is_some()
            {
                return Err(labeled_invariant(
                    "duplicate_key_certificate",
                    "offline key certificate is already issued",
                )
                .into());
            }
            reserve_offline_note_escrow(state_transaction, &issue.asset, &issue.amount)?;
            state_transaction
                .world
                .offline_note_replay_keys
                .insert(issue_key, ());
            state_transaction
                .world
                .offline_note_replay_keys
                .insert(certificate_key, ());
            state_transaction
                .world
                .offline_note_replay_keys
                .insert(issued_claim_key, ());
            let recorded_at_ms = state_transaction.block_unix_timestamp_ms();
            state_transaction
                .world
                .emit_events(Some(OfflineNoteEvent::NoteIssued(OfflineNoteIssued {
                    note_commitment: issue.note_commitment,
                    account: issue.key_certificate.account_id,
                    asset: issue.asset,
                    amount: issue.amount,
                    recorded_at_ms,
                })));
            Ok(())
        }
    }

    impl Execute for RedeemOfflineNote {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let redemption = self.redemption;
            if redemption.input_nullifiers.is_empty() || redemption.input_nullifiers.len() > 4 {
                return Err(labeled_invariant(
                    "invalid_proof",
                    "offline redemption requires 1 to 4 input nullifiers",
                )
                .into());
            }
            if redemption.amount <= Numeric::zero() {
                return Err(labeled_invariant(
                    "invalid_amount",
                    "offline redemption amount must be positive",
                )
                .into());
            }
            if redemption.asset.account() != &redemption.recipient {
                return Err(labeled_invariant(
                    "proof_binding",
                    "offline redemption asset owner must match recipient",
                )
                .into());
            }
            ensure_unique_hashes(
                &redemption.input_nullifiers,
                "duplicate_nullifier",
                "offline redemption input nullifiers must be unique",
            )?;
            validate_offline_note_key_certificate(&redemption.sender_key_certificate)?;
            ensure_can_submit_offline_note_for_account(
                &redemption.recipient,
                authority,
                state_transaction,
            )?;
            let spec = state_transaction.numeric_spec_for(redemption.asset.definition())?;
            assert_numeric_spec_with(&redemption.amount, spec)?;
            let expected_public_inputs_hash = redemption.public_inputs_hash().map_err(|err| {
                labeled_invariant(
                    "invalid_proof",
                    format!("failed to encode Offline redemption public inputs: {err}"),
                )
            })?;
            if redemption.recursive_proof.public_inputs_hash != expected_public_inputs_hash {
                return Err(labeled_invariant(
                    "proof_binding",
                    "offline recursive proof is not bound to redemption public inputs",
                )
                .into());
            }
            let expected_public_instances =
                crate::zk::offline_note_redeem_instance_values(&redemption)
                    .map_err(|err| labeled_invariant("invalid_proof", err))?
                    .public_instance_columns();
            let issued_claim_hash = offline_note_issued_claim_hash(
                OfflineNoteIssuedClaim::from_redemption(&redemption).map_err(|err| {
                    labeled_invariant(
                        "invalid_proof",
                        format!("failed to encode Offline issued-note claim: {err}"),
                    )
                })?,
            )?;
            let issued_claim_key = offline_note_issued_claim_key(&issued_claim_hash);
            let spent_claim_key = offline_note_spent_claim_key(&issued_claim_hash);
            if state_transaction
                .world
                .offline_note_replay_keys
                .get(&issued_claim_key)
                .is_none()
            {
                return Err(labeled_invariant(
                    "note_not_issued",
                    "offline note was not issued for this source commitment, recipient, asset, and amount",
                )
                .into());
            }
            if state_transaction
                .world
                .offline_note_replay_keys
                .get(&spent_claim_key)
                .is_some()
            {
                return Err(labeled_invariant(
                    "duplicate_redeem",
                    "offline issued note is already redeemed",
                )
                .into());
            }
            let consumed_keys = redemption
                .input_nullifiers
                .iter()
                .map(offline_note_nullifier_key)
                .collect::<Vec<_>>();
            for consumed_key in &consumed_keys {
                if state_transaction
                    .world
                    .offline_note_replay_keys
                    .get(consumed_key)
                    .is_some()
                {
                    return Err(labeled_invariant(
                        "duplicate_nullifier",
                        "offline nullifier is already redeemed",
                    )
                    .into());
                }
            }
            verify_offline_note_recursive_proof(
                &redemption.recursive_proof,
                &expected_public_inputs_hash,
                expected_public_instances,
                state_transaction,
            )?;
            credit_from_offline_note_escrow(
                state_transaction,
                &redemption.asset,
                &redemption.recipient,
                &redemption.amount,
            )?;
            for consumed_key in consumed_keys {
                state_transaction
                    .world
                    .offline_note_replay_keys
                    .insert(consumed_key, ());
            }
            state_transaction
                .world
                .offline_note_replay_keys
                .insert(spent_claim_key, ());
            let recorded_at_ms = state_transaction.block_unix_timestamp_ms();
            state_transaction
                .world
                .emit_events(Some(OfflineNoteEvent::NoteRedeemed(OfflineNoteRedeemed {
                    source_note_commitment: redemption.source_note_commitment,
                    recipient: redemption.recipient,
                    asset: redemption.asset,
                    amount: redemption.amount,
                    recorded_at_ms,
                })));
            Ok(())
        }
    }

    impl Execute for AuditOfflineNote {
        fn execute(
            self,
            _authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let audit = self.audit;
            if audit.input_nullifiers.is_empty() || audit.input_nullifiers.len() > 4 {
                return Err(labeled_invariant(
                    "invalid_proof",
                    "offline audit requires 1 to 4 input nullifiers",
                )
                .into());
            }
            if audit.input_claims.is_empty()
                || audit.input_claims.len() > 4
                || audit.input_claims.len() != audit.input_nullifiers.len()
            {
                return Err(labeled_invariant(
                    "invalid_proof",
                    "offline audit requires 1 to 4 input claims matching input nullifiers",
                )
                .into());
            }
            if audit.output_commitments.is_empty() || audit.output_commitments.len() > 2 {
                return Err(labeled_invariant(
                    "invalid_proof",
                    "offline audit requires 1 to 2 output commitments",
                )
                .into());
            }
            ensure_offline_audit_output_claim_binding(
                &audit.output_commitments,
                &audit.output_claims,
            )?;
            ensure_unique_hashes(
                &audit.input_nullifiers,
                "audit_duplicate_nullifier",
                "offline audit input nullifiers must be unique",
            )?;
            ensure_unique_hashes(
                &audit.output_commitments,
                "audit_duplicate_output",
                "offline audit output commitments must be unique",
            )?;
            let output_commitment_set = audit
                .output_commitments
                .iter()
                .copied()
                .collect::<BTreeSet<_>>();
            let mut output_claim_commitments = BTreeSet::new();
            for output_claim in &audit.output_claims {
                if !output_commitment_set.contains(&output_claim.note_commitment) {
                    return Err(labeled_invariant(
                        "proof_binding",
                        "offline audit output claim is not bound to an output commitment",
                    )
                    .into());
                }
                if !output_claim_commitments.insert(output_claim.note_commitment) {
                    return Err(labeled_invariant(
                        "audit_duplicate_output",
                        "offline audit output claims must be unique",
                    )
                    .into());
                }
                validate_offline_note_key_certificate(&output_claim.key_certificate)?;
                if output_claim.amount <= Numeric::zero() {
                    return Err(labeled_invariant(
                        "invalid_amount",
                        "offline audit output claim amount must be positive",
                    )
                    .into());
                }
                if output_claim.key_certificate.account_id != *output_claim.asset.account() {
                    return Err(labeled_invariant(
                        "invalid_issuer_cert",
                        "offline audit output claim certificate account must match the note asset owner",
                    )
                    .into());
                }
                ensure_offline_note_certificate_signature(
                    &output_claim.key_certificate,
                    &output_claim.key_certificate.account_id,
                )?;
                let spec = state_transaction.numeric_spec_for(output_claim.asset.definition())?;
                assert_numeric_spec_with(&output_claim.amount, spec)?;
            }
            validate_offline_note_key_certificate(&audit.sender_key_certificate)?;
            let certificate_payload_hash =
                audit.sender_key_certificate.payload_hash().map_err(|err| {
                    labeled_invariant(
                        "invalid_issuer_cert",
                        format!("failed to encode Offline key certificate payload: {err}"),
                    )
                })?;
            ensure_offline_audit_input_claim_anchor(
                &audit.input_claims,
                &certificate_payload_hash,
            )?;
            ensure_offline_audit_conserves_asset_amounts(
                &audit.input_claims,
                &audit.output_claims,
            )?;
            for input_claim in &audit.input_claims {
                let spec = state_transaction.numeric_spec_for(input_claim.asset.definition())?;
                assert_numeric_spec_with(&input_claim.amount, spec)?;
            }
            let certificate_key = offline_note_key_certificate_key(&certificate_payload_hash);
            if state_transaction
                .world
                .offline_note_replay_keys
                .get(&certificate_key)
                .is_none()
            {
                return Err(labeled_invariant(
                    "invalid_issuer_cert",
                    "offline audit key certificate was not issued",
                )
                .into());
            }
            let expected_public_inputs_hash = audit.public_inputs_hash().map_err(|err| {
                labeled_invariant(
                    "invalid_proof",
                    format!("failed to encode Offline audit public inputs: {err}"),
                )
            })?;
            if audit.recursive_proof.public_inputs_hash != expected_public_inputs_hash {
                return Err(labeled_invariant(
                    "proof_binding",
                    "offline recursive proof is not bound to audit public inputs",
                )
                .into());
            }
            let expected_public_instances = crate::zk::offline_note_audit_instance_values(&audit)
                .map_err(|err| labeled_invariant("invalid_proof", err))?
                .public_instance_columns();
            let audit_token_key = offline_note_audit_token_key(&audit.token_id);
            let audit_record_key = offline_note_audit_record_key(&expected_public_inputs_hash);
            let issued_output_claim_keys = audit
                .output_claims
                .iter()
                .map(|output_claim: &OfflineNoteAuditOutputClaim| {
                    let claim =
                        OfflineNoteIssuedClaim::from_audit_output(output_claim).map_err(|err| {
                            labeled_invariant(
                                "invalid_proof",
                                format!("failed to encode Offline audited output claim: {err}"),
                            )
                        })?;
                    Ok(offline_note_issued_claim_key(
                        &offline_note_issued_claim_hash(claim)?,
                    ))
                })
                .collect::<Result<Vec<_>, Error>>()?;
            let output_certificate_payload_hashes = audit
                .output_claims
                .iter()
                .map(|output_claim| {
                    output_claim.key_certificate.payload_hash().map_err(|err| {
                        labeled_invariant(
                            "invalid_issuer_cert",
                            format!(
                                "failed to encode Offline output key certificate payload: {err}"
                            ),
                        )
                        .into()
                    })
                })
                .collect::<Result<Vec<_>, Error>>()?;
            ensure_unique_hashes(
                &output_certificate_payload_hashes,
                "duplicate_key_certificate",
                "offline audit output key certificates must be unique",
            )?;
            let issued_output_certificate_keys = output_certificate_payload_hashes
                .iter()
                .map(offline_note_key_certificate_key)
                .collect::<Vec<_>>();
            let input_claim_hashes = audit
                .input_claims
                .iter()
                .cloned()
                .map(offline_note_issued_claim_hash)
                .collect::<Result<Vec<_>, Error>>()?;
            ensure_unique_hashes(
                &input_claim_hashes,
                "duplicate_redeem",
                "offline audit input claims must be unique",
            )?;
            let issued_input_claim_keys = input_claim_hashes
                .iter()
                .map(offline_note_issued_claim_key)
                .collect::<Vec<_>>();
            let spent_input_claim_keys = input_claim_hashes
                .iter()
                .map(offline_note_spent_claim_key)
                .collect::<Vec<_>>();
            if state_transaction
                .world
                .offline_note_replay_keys
                .get(&audit_token_key)
                .is_some()
            {
                if state_transaction
                    .world
                    .offline_note_replay_keys
                    .get(&audit_record_key)
                    .is_some()
                {
                    return Ok(());
                }
                return Err(labeled_invariant(
                    "audit_conflict",
                    "offline audit token already records different public inputs",
                )
                .into());
            }
            let consumed_nullifier_keys = audit
                .input_nullifiers
                .iter()
                .map(offline_note_nullifier_key)
                .collect::<Vec<_>>();
            let observed_nullifier_keys = audit
                .input_nullifiers
                .iter()
                .map(offline_note_audit_nullifier_key)
                .collect::<Vec<_>>();
            let observed_output_keys = audit
                .output_commitments
                .iter()
                .map(offline_note_audit_output_key)
                .collect::<Vec<_>>();
            let issued_output_commitment_keys = audit
                .output_commitments
                .iter()
                .map(offline_note_issue_key)
                .collect::<Vec<_>>();
            for issued_claim_key in &issued_input_claim_keys {
                if state_transaction
                    .world
                    .offline_note_replay_keys
                    .get(issued_claim_key)
                    .is_none()
                {
                    return Err(labeled_invariant(
                        "note_not_issued",
                        "offline audit input claim was not issued",
                    )
                    .into());
                }
            }
            for spent_claim_key in &spent_input_claim_keys {
                if state_transaction
                    .world
                    .offline_note_replay_keys
                    .get(spent_claim_key)
                    .is_some()
                {
                    return Err(labeled_invariant(
                        "duplicate_redeem",
                        "offline audit input claim is already redeemed",
                    )
                    .into());
                }
            }
            for consumed_key in &consumed_nullifier_keys {
                if state_transaction
                    .world
                    .offline_note_replay_keys
                    .get(consumed_key)
                    .is_some()
                {
                    return Err(labeled_invariant(
                        "duplicate_nullifier",
                        "offline audit nullifier is already redeemed",
                    )
                    .into());
                }
            }
            for observed_key in &observed_nullifier_keys {
                if state_transaction
                    .world
                    .offline_note_replay_keys
                    .get(observed_key)
                    .is_some()
                {
                    return Err(labeled_invariant(
                        "audit_duplicate_nullifier",
                        "offline audit observed a duplicate nullifier",
                    )
                    .into());
                }
            }
            for observed_key in &observed_output_keys {
                if state_transaction
                    .world
                    .offline_note_replay_keys
                    .get(observed_key)
                    .is_some()
                {
                    return Err(labeled_invariant(
                        "audit_duplicate_output",
                        "offline audit observed a duplicate output commitment",
                    )
                    .into());
                }
            }
            for issued_output_commitment_key in &issued_output_commitment_keys {
                if state_transaction
                    .world
                    .offline_note_replay_keys
                    .get(issued_output_commitment_key)
                    .is_some()
                {
                    return Err(labeled_invariant(
                        "duplicate_issue",
                        "offline audit output commitment is already issued",
                    )
                    .into());
                }
            }
            for issued_claim_key in &issued_output_claim_keys {
                if state_transaction
                    .world
                    .offline_note_replay_keys
                    .get(issued_claim_key)
                    .is_some()
                {
                    return Err(labeled_invariant(
                        "duplicate_issue",
                        "offline audit output claim is already issued",
                    )
                    .into());
                }
            }
            for issued_certificate_key in &issued_output_certificate_keys {
                if state_transaction
                    .world
                    .offline_note_replay_keys
                    .get(issued_certificate_key)
                    .is_some()
                {
                    return Err(labeled_invariant(
                        "duplicate_key_certificate",
                        "offline audit output key certificate is already issued",
                    )
                    .into());
                }
            }
            verify_offline_note_recursive_proof(
                &audit.recursive_proof,
                &expected_public_inputs_hash,
                expected_public_instances,
                state_transaction,
            )?;
            state_transaction
                .world
                .offline_note_replay_keys
                .insert(audit_token_key, ());
            state_transaction
                .world
                .offline_note_replay_keys
                .insert(audit_record_key, ());
            for consumed_key in consumed_nullifier_keys {
                state_transaction
                    .world
                    .offline_note_replay_keys
                    .insert(consumed_key, ());
            }
            for spent_claim_key in spent_input_claim_keys {
                state_transaction
                    .world
                    .offline_note_replay_keys
                    .insert(spent_claim_key, ());
            }
            for observed_key in observed_nullifier_keys {
                state_transaction
                    .world
                    .offline_note_replay_keys
                    .insert(observed_key, ());
            }
            for observed_key in observed_output_keys {
                state_transaction
                    .world
                    .offline_note_replay_keys
                    .insert(observed_key, ());
            }
            for issued_claim_key in issued_output_claim_keys {
                state_transaction
                    .world
                    .offline_note_replay_keys
                    .insert(issued_claim_key, ());
            }
            for issued_certificate_key in issued_output_certificate_keys {
                state_transaction
                    .world
                    .offline_note_replay_keys
                    .insert(issued_certificate_key, ());
            }
            let recorded_at_ms = state_transaction.block_unix_timestamp_ms();
            state_transaction
                .world
                .emit_events(Some(OfflineNoteEvent::AuditRecorded(
                    OfflineNoteAuditRecorded {
                        token_id: audit.token_id,
                        account: audit.sender_key_certificate.account_id,
                        public_inputs_hash: expected_public_inputs_hash,
                        recorded_at_ms,
                    },
                )));
            Ok(())
        }
    }

    impl Execute for KagemushaTransfer {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            if !state_transaction.settlement.offline.kagemusha_enabled {
                return Err(labeled_invariant(
                    "kagemusha_disabled",
                    "Kagemusha offline-offline settlement is disabled by configuration",
                )
                .into());
            }
            if state_transaction.settlement.offline.kagemusha_force_legacy {
                return Err(labeled_invariant(
                    "kagemusha_legacy_forced",
                    "Kagemusha offline-offline settlement is bypassed by explicit legacy fallback",
                )
                .into());
            }
            if self.inputs.is_empty()
                || self.inputs.len() > 2
                || self.outputs.is_empty()
                || self.outputs.len() > 2
            {
                return Err(labeled_invariant(
                    "invalid_proof",
                    "Kagemusha transfers require 1 to 2 input nullifiers and 1 to 2 output commitments",
                )
                .into());
            }
            ensure_non_zero_bytes32(
                &self.inputs,
                "invalid_proof",
                "Kagemusha transfer input nullifiers must be non-zero",
            )?;
            ensure_non_zero_bytes32(
                &self.outputs,
                "invalid_proof",
                "Kagemusha transfer output commitments must be non-zero",
            )?;
            ensure_unique_bytes32(
                &self.inputs,
                "duplicate_nullifier",
                "Kagemusha transfer input nullifiers must be unique",
            )?;
            ensure_unique_bytes32(
                &self.outputs,
                "duplicate_output",
                "Kagemusha transfer output commitments must be unique",
            )?;
            ensure_kagemusha_transparent_attachment(&self.proof)?;
            ensure_kagemusha_transfer_verifier_binding(
                &self.asset,
                &self.proof,
                self.root_hint,
                state_transaction,
            )?;
            let transfer = iroha_data_model::isi::zk::ZkTransfer::new(
                self.asset,
                self.inputs,
                self.outputs,
                self.proof,
                self.root_hint,
            );
            transfer.execute(authority, state_transaction)
        }
    }

    #[cfg(test)]
    mod tests {
        use std::{collections::BTreeSet, sync::Arc};

        use super::*;
        use crate::{
            kura::Kura,
            query::store::LiveQueryStore,
            state::{State, World, ZkAssetState, ZkAssetVerifierBinding},
        };
        use iroha_crypto::{KeyPair, Signature};
        use iroha_data_model::{
            Registrable,
            account::Account,
            asset::{Asset, AssetDefinition, definition::AssetConfidentialPolicy},
            block::BlockHeader,
            domain::{Domain, DomainId},
            offline::OfflineNoteAuditBundle,
            permission::Permission,
            proof::ProofAttachment,
        };
        use iroha_primitives::json::Json;
        use iroha_primitives::numeric::NumericSpec;
        use nonzero_ext::nonzero;

        fn sample_account(seed: u8) -> AccountId {
            let keypair = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
            AccountId::new(keypair.public_key().clone())
        }

        fn sample_signature(seed: u8) -> Signature {
            let mut payload = [0u8; 64];
            for (idx, byte) in payload.iter_mut().enumerate() {
                let offset = u8::try_from(idx).expect("index fits into u8");
                *byte = seed.wrapping_add(offset);
            }
            Signature::from_bytes(&payload)
        }

        fn sample_certificate() -> OfflineNoteKeyCertificate {
            let keypair = KeyPair::from_seed(vec![0xAA; 32], Algorithm::Ed25519);
            let (_algorithm, public_key) = keypair.public_key().to_bytes();
            OfflineNoteKeyCertificate {
                version: iroha_data_model::offline::OFFLINE_NOTE_KEY_CERTIFICATE_VERSION,
                platform: "ios-appattest".to_owned(),
                key_id: "one-use-key".to_owned(),
                device_id: "device-1".to_owned(),
                account_id: sample_account(0x01),
                public_key: public_key.to_vec(),
                assertion_scheme: "apple-appattest-counter".to_owned(),
                assertion_key_algorithm: "app-attest-p256".to_owned(),
                assertion_public_key: vec![0x04; 65],
                assertion_usage_count_limit: None,
                one_use: true,
                issuer_signature: sample_signature(0x44),
            }
        }

        fn signed_sample_certificate(
            issuer: &KeyPair,
            account_id: AccountId,
            note_seed: u8,
            key_id: &str,
        ) -> OfflineNoteKeyCertificate {
            let note_key = KeyPair::from_seed(vec![note_seed; 32], Algorithm::Ed25519);
            let (_algorithm, public_key) = note_key.public_key().to_bytes();
            let mut certificate = OfflineNoteKeyCertificate {
                version: iroha_data_model::offline::OFFLINE_NOTE_KEY_CERTIFICATE_VERSION,
                platform: "offline-unit-test".to_owned(),
                key_id: key_id.to_owned(),
                device_id: "offline-unit-device".to_owned(),
                account_id,
                public_key: public_key.to_vec(),
                assertion_scheme: "unit-test-one-use".to_owned(),
                assertion_key_algorithm: "ed25519-test".to_owned(),
                assertion_public_key: public_key.to_vec(),
                assertion_usage_count_limit: Some(1),
                one_use: true,
                issuer_signature: Signature::new(issuer.private_key(), b"placeholder"),
            };
            let payload = certificate
                .signing_bytes()
                .expect("certificate signing payload encodes");
            certificate.issuer_signature = Signature::new(issuer.private_key(), &payload);
            certificate
        }

        fn sample_issued_claim() -> OfflineNoteIssuedClaim {
            let account_id = sample_account(0x01);
            let definition_id = AssetDefinitionId::new(
                DomainId::try_new("offline", "universal").expect("domain id"),
                "xor".parse().expect("asset definition name"),
            );
            let issue = iroha_data_model::offline::OfflineNoteIssue {
                note_commitment: Hash::new(b"offline-note-source-note"),
                key_certificate: sample_certificate(),
                asset: AssetId::new(definition_id, account_id),
                amount: Numeric::new(10, 0),
            };
            OfflineNoteIssuedClaim::from_issue(&issue).expect("issued claim")
        }

        fn placeholder_recursive_proof(public_inputs_hash: Hash) -> OfflineNoteRecursiveProof {
            OfflineNoteRecursiveProof {
                verifier_key_id: VerifyingKeyId::new(
                    crate::zk::ZK_BACKEND_HALO2_IPA,
                    crate::zk::OFFLINE_NOTE_RECURSIVE_CIRCUIT_ID,
                ),
                public_inputs_hash,
                proof: ProofBox::new(crate::zk::ZK_BACKEND_HALO2_IPA.to_owned(), Vec::new()),
            }
        }

        fn sample_audit_bundle_for_issue(
            issue: &iroha_data_model::offline::OfflineNoteIssue,
            output_certificate: OfflineNoteKeyCertificate,
        ) -> OfflineNoteAuditBundle {
            let input_claim = OfflineNoteIssuedClaim::from_issue(issue).expect("issued claim");
            let output_commitment = Hash::new(b"offline-audit-output-note");
            let mut audit = OfflineNoteAuditBundle {
                token_id: Hash::new(b"offline-audit-token"),
                sender_key_certificate: issue.key_certificate.clone(),
                input_nullifiers: vec![Hash::new(b"offline-audit-input-nullifier")],
                input_claims: vec![input_claim],
                output_commitments: vec![output_commitment],
                output_claims: vec![OfflineNoteAuditOutputClaim {
                    note_commitment: output_commitment,
                    key_certificate: output_certificate,
                    asset: issue.asset.clone(),
                    amount: issue.amount.clone(),
                }],
                recursive_proof: placeholder_recursive_proof(Hash::new(
                    b"offline-placeholder-public-inputs",
                )),
            };
            let public_inputs_hash = audit.public_inputs_hash().expect("audit hash");
            audit.recursive_proof = placeholder_recursive_proof(public_inputs_hash);
            audit
        }

        fn self_escrow_test_state(
            balance: Numeric,
        ) -> (State, AssetId, AccountId, AssetDefinitionId) {
            let account_id = sample_account(0x01);
            let domain_id: DomainId = DomainId::try_new("offline", "universal").expect("domain id");
            let definition_id = AssetDefinitionId::new(
                domain_id.clone(),
                "xor".parse().expect("asset definition name"),
            );
            let asset_id = AssetId::new(definition_id.clone(), account_id.clone());
            let domain = Domain::new(domain_id).build(&account_id);
            let account = Account::new(account_id.clone()).build(&account_id);
            let asset_definition =
                AssetDefinition::new(definition_id.clone(), NumericSpec::integer())
                    .with_name("xor".to_owned())
                    .build(&account_id);
            let asset = Asset::new(asset_id.clone(), balance);
            let world = World::with_assets([domain], [account], [asset_definition], [asset], []);
            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let mut state = State::new(world, Arc::clone(&kura), query);
            state.settlement.offline.escrow_required = true;
            state
                .settlement
                .offline
                .escrow_accounts
                .insert(definition_id.clone(), account_id.clone());

            (state, asset_id, account_id, definition_id)
        }

        fn distinct_escrow_test_state(
            balance: Numeric,
            escrow_seed: u8,
        ) -> (State, AssetId, AccountId, AssetDefinitionId) {
            let account_id = sample_account(0x01);
            let escrow_account_id = sample_account(escrow_seed);
            let domain_id: DomainId = DomainId::try_new("offline", "universal").expect("domain id");
            let definition_id = AssetDefinitionId::new(
                domain_id.clone(),
                "xor".parse().expect("asset definition name"),
            );
            let asset_id = AssetId::new(definition_id.clone(), account_id.clone());
            let escrow_asset_id = AssetId::new(definition_id.clone(), escrow_account_id.clone());
            let domain = Domain::new(domain_id).build(&account_id);
            let account = Account::new(account_id.clone()).build(&account_id);
            let escrow_account = Account::new(escrow_account_id.clone()).build(&account_id);
            let asset_definition =
                AssetDefinition::new(definition_id.clone(), NumericSpec::integer())
                    .with_name("xor".to_owned())
                    .build(&account_id);
            let asset = Asset::new(asset_id.clone(), balance);
            let escrow_asset = Asset::new(escrow_asset_id, Numeric::zero());
            let world = World::with_assets(
                [domain],
                [account, escrow_account],
                [asset_definition],
                [asset, escrow_asset],
                [],
            );
            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let mut state = State::new(world, Arc::clone(&kura), query);
            state.settlement.offline.escrow_required = true;
            state
                .settlement
                .offline
                .escrow_accounts
                .insert(definition_id.clone(), escrow_account_id);

            (state, asset_id, account_id, definition_id)
        }

        fn offline_note_verifier_test_state(
            status: ConfidentialStatus,
        ) -> (State, OfflineNoteRecursiveProof, Hash) {
            let verifier_id = VerifyingKeyId::new(
                crate::zk::ZK_BACKEND_HALO2_IPA,
                crate::zk::OFFLINE_NOTE_RECURSIVE_CIRCUIT_ID,
            );
            #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
            let mut record =
                crate::zk::offline_note_recursive_vk_record(OFFLINE_NOTE_VERIFIER_NAMESPACE, 1)
                    .expect("offline recursive verifier record");
            #[cfg(not(any(feature = "zk-halo2", feature = "zk-halo2-ipa")))]
            let mut record = {
                let vk_box = VerifyingKeyBox::new(
                    crate::zk::ZK_BACKEND_HALO2_IPA.to_owned(),
                    b"offline-note-test-verifying-key".to_vec(),
                );
                let commitment = crate::zk::hash_vk(&vk_box);
                let mut record = VerifyingKeyRecord::new_with_owner(
                    1,
                    crate::zk::OFFLINE_NOTE_RECURSIVE_CIRCUIT_ID,
                    None,
                    OFFLINE_NOTE_VERIFIER_NAMESPACE,
                    BackendTag::Halo2IpaPasta,
                    "pasta",
                    offline_note_recursive_public_inputs_schema_hash(),
                    commitment,
                );
                record.key = Some(vk_box);
                record.max_proof_bytes = 4096;
                record.vk_len = b"offline-note-test-verifying-key".len() as u32;
                record
            };
            record.status = status;
            let commitment = record.commitment;

            let envelope = OpenVerifyEnvelope::new(
                BackendTag::Halo2IpaPasta,
                crate::zk::OFFLINE_NOTE_RECURSIVE_CIRCUIT_ID,
                commitment,
                OFFLINE_NOTE_RECURSIVE_PUBLIC_INPUTS_SCHEMA.to_vec(),
                b"offline-note-test-proof".to_vec(),
            );
            let proof_bytes = norito::to_bytes(&envelope).expect("encode OpenVerifyEnvelope");
            let public_inputs_hash = Hash::new(b"offline-note-public-inputs");

            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let mut state = State::new(World::default(), Arc::clone(&kura), query);
            state
                .world
                .verifying_keys
                .insert(verifier_id.clone(), record);
            state.world.verifying_keys_by_circuit.insert(
                (crate::zk::OFFLINE_NOTE_RECURSIVE_CIRCUIT_ID.to_owned(), 1),
                verifier_id.clone(),
            );

            let proof = OfflineNoteRecursiveProof {
                verifier_key_id: verifier_id,
                public_inputs_hash: public_inputs_hash.clone(),
                proof: ProofBox::new(crate::zk::ZK_BACKEND_HALO2_IPA.to_owned(), proof_bytes),
            };

            (state, proof, public_inputs_hash)
        }

        fn mutate_offline_note_recursive_envelope(
            proof: &mut OfflineNoteRecursiveProof,
            mutate: impl FnOnce(&mut OpenVerifyEnvelope),
        ) {
            let mut envelope: OpenVerifyEnvelope = norito::decode_from_bytes(&proof.proof.bytes)
                .expect("offline recursive proof should be an OpenVerifyEnvelope");
            mutate(&mut envelope);
            proof.proof.bytes =
                norito::to_bytes(&envelope).expect("encode mutated OpenVerifyEnvelope");
        }

        fn mutate_verifier_record(
            transaction: &mut StateTransaction<'_, '_>,
            verifier_id: &VerifyingKeyId,
            mutate: impl FnOnce(&mut VerifyingKeyRecord),
        ) {
            let mut record = transaction
                .world
                .verifying_keys
                .get(verifier_id)
                .expect("verifier record")
                .clone();
            mutate(&mut record);
            transaction
                .world
                .verifying_keys
                .insert(verifier_id.clone(), record);
        }

        fn assert_offline_note_record_mutation_rejects(
            mutate: impl for<'block, 'state> FnOnce(
                &mut StateTransaction<'block, 'state>,
                &VerifyingKeyId,
                &OfflineNoteRecursiveProof,
            ),
            label: &str,
            detail: &str,
        ) {
            let (state, proof, _public_inputs_hash) =
                offline_note_verifier_test_state(ConfidentialStatus::Active);
            let verifier_id = proof.verifier_key_id.clone();
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            {
                let mut mutation_transaction = block.transaction();
                mutate(&mut mutation_transaction, &verifier_id, &proof);
                mutation_transaction.apply();
            }
            let transaction = block.transaction();

            let err = offline_note_resolve_verifier(&proof, &transaction)
                .expect_err("mutated offline recursive verifier metadata must reject");
            assert_offline_rejection(err, label, detail);
        }

        fn sample_kagemusha_transfer(backend: &str) -> KagemushaTransfer {
            KagemushaTransfer::new(
                sample_issued_claim().asset.definition().clone(),
                vec![[0x11; 32]],
                vec![[0x22; 32]],
                ProofAttachment::new_ref(
                    backend.into(),
                    ProofBox::new(backend.into(), vec![0xCA, 0xFE]),
                    VerifyingKeyId::new(backend, "offline-kagemusha-transfer"),
                ),
                Some([0x33; 32]),
            )
        }

        fn real_kagemusha_test_state() -> (
            State,
            AccountId,
            AssetDefinitionId,
            KagemushaTransfer,
            Vec<[u8; 32]>,
            Vec<[u8; 32]>,
        ) {
            let authority = sample_account(0x46);
            let chain_id: iroha_data_model::ChainId =
                "kagemusha-transfer-chain".parse().expect("chain id");
            let domain_id = DomainId::try_new("offline", "universal").expect("domain id");
            let definition_id = AssetDefinitionId::new(
                domain_id.clone(),
                "kgm".parse().expect("asset definition name"),
            );
            let asset_id = AssetId::new(definition_id.clone(), authority.clone());
            let domain = Domain::new(domain_id).build(&authority);
            let account = Account::new(authority.clone()).build(&authority);
            let asset_definition =
                AssetDefinition::new(definition_id.clone(), NumericSpec::integer())
                    .with_name("kgm".to_owned())
                    .confidential_policy(AssetConfidentialPolicy::convertible())
                    .build(&authority);
            let asset = Asset::new(asset_id, Numeric::zero());

            let mut vk_record = crate::zk::confidential_v2::confidential_transfer_v2_vk_record(
                crate::zk::KAGEMUSHA_VERIFIER_NAMESPACE,
                1,
            )
            .expect("confidential transfer v2 verifier record");
            let vk_box = vk_record
                .key
                .clone()
                .expect("confidential transfer v2 verifier key");
            let vk_commitment = vk_record.commitment;
            let vk_id = VerifyingKeyId::new(
                crate::zk::ZK_BACKEND_HALO2_IPA,
                "offline-kagemusha-confidential-transfer-v2",
            );
            vk_record.status = ConfidentialStatus::Active;

            let spend_key = [0x11_u8; 32];
            let input_rho = [0x21_u8; 32];
            let input_diversifier =
                crate::zk::confidential_v2::derive_confidential_diversifier_v2(b"kagemusha-input");
            let input_owner_tag =
                crate::zk::confidential_v2::derive_confidential_owner_tag_v2_with_diversifier(
                    &spend_key,
                    input_diversifier,
                )
                .expect("input owner tag");
            let input_commitment = crate::zk::confidential_v2::derive_confidential_note_v2(
                &definition_id.to_string(),
                7,
                input_rho,
                input_owner_tag,
            )
            .expect("input commitment");
            let initial_commitments = vec![input_commitment];
            let root_hint =
                crate::zk::confidential_v2::compute_confidential_root_v2(&initial_commitments)
                    .expect("initial confidential root");

            let output_rho = [0x31_u8; 32];
            let recipient_diversifier =
                crate::zk::confidential_v2::derive_confidential_diversifier_v2(
                    b"kagemusha-recipient",
                );
            let output_owner_tag =
                crate::zk::confidential_v2::derive_confidential_owner_tag_v2_with_diversifier(
                    &[0x41_u8; 32],
                    recipient_diversifier,
                )
                .expect("output owner tag");
            let proof = crate::zk::confidential_v2::build_confidential_transfer_proof_v2(
                &chain_id,
                &definition_id.to_string(),
                &spend_key,
                &initial_commitments,
                &[crate::zk::confidential_v2::ConfidentialTransferInputV2 {
                    amount: 7,
                    rho: input_rho,
                    diversifier: input_diversifier,
                    leaf_index: 0,
                }],
                &[crate::zk::confidential_v2::ConfidentialTransferOutputV2 {
                    amount: 7,
                    rho: output_rho,
                    owner_tag: output_owner_tag,
                }],
                root_hint,
                &vk_record.circuit_id,
                &vk_box,
            )
            .expect("real confidential transfer v2 proof");

            let mut expected_commitments = initial_commitments.clone();
            expected_commitments.extend(proof.output_commitments.iter().copied());
            let expected_final_root =
                crate::zk::confidential_v2::compute_confidential_root_v2(&expected_commitments)
                    .expect("final confidential root");

            let mut world =
                World::with_assets([domain], [account], [asset_definition], [asset], []);
            world.verifying_keys_by_circuit.insert(
                (vk_record.circuit_id.clone(), vk_record.version),
                vk_id.clone(),
            );
            world.verifying_keys.insert(vk_id.clone(), vk_record);
            world.zk_assets.insert(definition_id.clone(), {
                let mut zk_state = ZkAssetState::default();
                zk_state.commitments = initial_commitments;
                zk_state.root_history = vec![root_hint];
                zk_state.vk_transfer = Some(ZkAssetVerifierBinding {
                    id: vk_id.clone(),
                    commitment: vk_commitment,
                });
                zk_state
            });

            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let mut state = State::new_with_chain(world, Arc::clone(&kura), query, chain_id);
            assert!(
                state.settlement.offline.kagemusha_enabled,
                "Kagemusha must remain enabled by default"
            );
            assert!(
                !state.settlement.offline.kagemusha_force_legacy,
                "Kagemusha legacy fallback must not be forced by default"
            );
            let mut zk = state.zk.clone();
            zk.halo2.enabled = true;
            zk.halo2.max_envelope_bytes = usize::MAX;
            zk.halo2.max_proof_bytes = usize::MAX;
            state.set_zk(zk);

            let mut proof_attachment = ProofAttachment::new_ref(
                crate::zk::ZK_BACKEND_HALO2_IPA.into(),
                proof.proof,
                vk_id,
            );
            proof_attachment.vk_commitment = Some(vk_commitment);
            proof_attachment.envelope_hash = Some(Hash::new(&proof_attachment.proof.bytes).into());
            let transfer = KagemushaTransfer::new(
                definition_id.clone(),
                proof.nullifiers.clone(),
                proof.output_commitments.clone(),
                proof_attachment,
                Some(root_hint),
            );

            (
                state,
                authority,
                definition_id,
                transfer,
                expected_commitments,
                vec![expected_final_root],
            )
        }

        fn mutate_kagemusha_transfer_envelope(
            transfer: &mut KagemushaTransfer,
            mutate: impl FnOnce(&mut OpenVerifyEnvelope),
        ) {
            let mut envelope: OpenVerifyEnvelope =
                norito::decode_from_bytes(&transfer.proof.proof.bytes)
                    .expect("Kagemusha transfer proof should be an OpenVerifyEnvelope");
            mutate(&mut envelope);
            transfer.proof.proof.bytes =
                norito::to_bytes(&envelope).expect("encode mutated OpenVerifyEnvelope");
        }

        fn assert_kagemusha_transfer_record_mutation_rejects(
            mutate: impl for<'block, 'state> FnOnce(
                &mut StateTransaction<'block, 'state>,
                &VerifyingKeyId,
            ),
            label: &str,
            detail: &str,
        ) {
            let (state, authority, _definition_id, mut transfer, _commitments, _roots) =
                real_kagemusha_test_state();
            let verifier_id = transfer.proof.vk_ref.clone();
            transfer.proof.proof.bytes = b"not-an-open-verify-envelope".to_vec();
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            {
                let mut mutation_transaction = block.transaction();
                mutate(&mut mutation_transaction, &verifier_id);
                mutation_transaction.apply();
            }
            let mut transaction = block.transaction();

            let err = transfer
                .execute(&authority, &mut transaction)
                .expect_err("mutated Kagemusha verifier metadata must reject before proof decode");
            assert_offline_rejection(err, label, detail);
        }

        fn assert_offline_rejection(err: Error, label: &str, detail: &str) {
            let message = err.to_string();
            assert!(
                message.contains(label),
                "expected error label `{label}`, got: {message}"
            );
            assert!(
                message.contains(detail),
                "expected error detail `{detail}`, got: {message}"
            );
        }

        #[test]
        fn kagemusha_transfer_rejects_confidential_v2_envelope_mismatches() {
            let cases: [(&str, &str, fn(&mut OpenVerifyEnvelope)); 6] = [
                ("verifier_key_invalid", "backend", |envelope| {
                    envelope.backend = BackendTag::Stark;
                }),
                (
                    "verifier_key_invalid",
                    "confidential-transfer-v2",
                    |envelope| {
                        envelope.circuit_id = "halo2/pasta/ipa/tiny-add".to_owned();
                    },
                ),
                ("verifier_key_invalid", "canonical", |envelope| {
                    envelope.circuit_id =
                        "anon-transfer-2x2-merkle16-poseidon-diversified".to_owned();
                }),
                ("verifier_schema_mismatch", "schema", |envelope| {
                    envelope.public_inputs = b"not-confidential-transfer-v2".to_vec();
                }),
                ("verifier_key_invalid", "verifier-key hash", |envelope| {
                    envelope.vk_hash = [0xA5; 32];
                }),
                ("invalid_proof", "auxiliary bytes", |envelope| {
                    envelope.aux = b"kagemusha-forged-chain-aux".to_vec();
                }),
            ];

            for (label, detail, mutate) in cases {
                let (state, authority, _definition_id, mut transfer, _commitments, _roots) =
                    real_kagemusha_test_state();
                mutate_kagemusha_transfer_envelope(&mut transfer, mutate);
                let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
                let mut block = state.block(header);
                let mut transaction = block.transaction();

                let err = transfer
                    .execute(&authority, &mut transaction)
                    .expect_err("mutated Kagemusha proof envelope must reject");
                assert_offline_rejection(err, label, detail);
            }
        }

        #[test]
        fn kagemusha_transfer_rejects_malformed_envelope_and_missing_root_hint() {
            let (state, authority, _definition_id, mut transfer, _commitments, _roots) =
                real_kagemusha_test_state();
            transfer.proof.proof.bytes = b"not-an-open-verify-envelope".to_vec();
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = transfer
                .execute(&authority, &mut transaction)
                .expect_err("malformed Kagemusha proof envelope must reject");
            assert_offline_rejection(err, "invalid_proof", "OpenVerifyEnvelope");

            let (state, authority, _definition_id, mut transfer, _commitments, _roots) =
                real_kagemusha_test_state();
            transfer.root_hint = None;
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = transfer
                .execute(&authority, &mut transaction)
                .expect_err("Kagemusha confidential transfer must require root_hint");
            assert_offline_rejection(err, "invalid_proof", "root hint");
        }

        #[test]
        fn kagemusha_transfer_rejects_forged_envelope_hash_metadata() {
            let (state, authority, _definition_id, mut transfer, _commitments, _roots) =
                real_kagemusha_test_state();
            transfer.proof.envelope_hash = Some([0xA7; 32]);
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = transfer
                .execute(&authority, &mut transaction)
                .expect_err("forged Kagemusha envelope hash must reject");
            assert_offline_rejection(err, "invalid_proof", "envelope hash");
        }

        #[test]
        fn kagemusha_transfer_rejects_missing_verifier_key_commitment_metadata() {
            let (state, authority, _definition_id, mut transfer, _commitments, _roots) =
                real_kagemusha_test_state();
            transfer.proof.vk_commitment = None;
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = transfer
                .execute(&authority, &mut transaction)
                .expect_err("missing Kagemusha verifier-key commitment must reject");
            assert_offline_rejection(
                err,
                "verifier_key_invalid",
                "must publish the asset-bound verifier-key commitment",
            );
        }

        #[test]
        fn kagemusha_transfer_rejects_empty_verifier_key_id_name() {
            let (state, authority, _definition_id, mut transfer, _commitments, _roots) =
                real_kagemusha_test_state();
            transfer.proof.vk_ref.name = "   ".to_owned();
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = transfer
                .execute(&authority, &mut transaction)
                .expect_err("empty Kagemusha verifier-key id name must reject");
            assert_offline_rejection(
                err,
                "verifier_key_invalid",
                "verifier key id name must be non-empty",
            );
        }

        #[test]
        fn reserve_offline_note_escrow_rejects_escrow_self_reference() {
            let (state, asset_id, _account_id, _definition_id) =
                self_escrow_test_state(Numeric::new(100, 0));
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err =
                reserve_offline_note_escrow(&mut transaction, &asset_id, &Numeric::new(25, 0))
                    .expect_err("self-referenced escrow must reject note reservation");
            assert!(
                err.to_string().contains("escrow_self_reference"),
                "unexpected error: {err}"
            );

            let balance = transaction
                .world
                .assets
                .get(&asset_id)
                .map(|asset| asset.as_ref().clone())
                .unwrap_or_else(Numeric::zero);
            assert_eq!(balance, Numeric::new(100, 0));
        }

        #[test]
        fn credit_from_offline_note_escrow_rejects_escrow_self_reference() {
            let (state, asset_id, account_id, _definition_id) =
                self_escrow_test_state(Numeric::new(100, 0));
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = credit_from_offline_note_escrow(
                &mut transaction,
                &asset_id,
                &account_id,
                &Numeric::new(25, 0),
            )
            .expect_err("self-referenced escrow must reject note credit");
            assert!(
                err.to_string().contains("escrow_self_reference"),
                "unexpected error: {err}"
            );

            let balance = transaction
                .world
                .assets
                .get(&asset_id)
                .map(|asset| asset.as_ref().clone())
                .unwrap_or_else(Numeric::zero);
            assert_eq!(balance, Numeric::new(100, 0));
        }

        #[test]
        fn expected_public_instances_encode_semantic_columns() {
            let values = crate::zk::OfflineNoteInstanceValues {
                public_values: [11, 22, 33, 44, 1, 1, 1, 10, 10, 55, 0, 66, 77, 88, 0, 0],
                input_amounts: [10, 0, 0, 0],
                output_amounts: [10, 0],
            };
            let instances = values.public_instance_columns();

            assert_eq!(instances.len(), crate::zk::OFFLINE_NOTE_INSTANCE_COLUMNS);
            for (index, value) in values.public_values.iter().copied().enumerate() {
                let mut expected = [0u8; 32];
                expected[..8].copy_from_slice(&value.to_le_bytes());
                assert_eq!(instances[index], vec![expected]);
            }
        }

        #[test]
        fn key_certificate_requires_one_use_ed25519_key() {
            let mut certificate = sample_certificate();
            assert!(validate_offline_note_key_certificate(&certificate).is_ok());

            certificate.one_use = false;
            assert!(validate_offline_note_key_certificate(&certificate).is_err());

            certificate.one_use = true;
            certificate.public_key.clear();
            assert!(validate_offline_note_key_certificate(&certificate).is_err());
        }

        #[test]
        fn duplicate_hashes_are_rejected() {
            let hash = Hash::new(b"duplicate");
            let err = ensure_unique_hashes(&[hash, hash], "duplicate_hash", "duplicate hashes")
                .expect_err("duplicate hash should fail");
            let InstructionExecutionError::InvariantViolation(message) = err else {
                panic!("expected invariant violation");
            };
            assert!(message.contains("offline_reason::duplicate_hash:"));
        }

        #[test]
        fn audit_output_claim_count_must_match_commitments_one_to_one() {
            let err = ensure_offline_audit_output_claim_count(2, 1)
                .expect_err("hidden audit output commitments must be rejected");
            let InstructionExecutionError::InvariantViolation(message) = err else {
                panic!("expected invariant violation");
            };
            assert!(message.contains("offline_reason::invalid_proof:"));
            assert!(message.contains("one-to-one"));
        }

        #[test]
        fn audit_output_claims_must_be_ordered_one_to_one_with_commitments() {
            let input = sample_issued_claim();
            let commitment = Hash::new(b"offline-note-commitment-a");
            let claims = [OfflineNoteAuditOutputClaim {
                note_commitment: Hash::new(b"offline-note-commitment-b"),
                key_certificate: sample_certificate(),
                asset: input.asset,
                amount: Numeric::new(10, 0),
            }];

            let err = ensure_offline_audit_output_claim_binding(&[commitment], &claims)
                .expect_err("mismatched output claim commitment must reject");
            let InstructionExecutionError::InvariantViolation(message) = err else {
                panic!("expected invariant violation");
            };
            assert!(message.contains("offline_reason::proof_binding:"));
            assert!(message.contains("ordered one-to-one"));
        }

        #[test]
        fn audit_input_claims_must_be_anchored_to_sender_certificate() {
            let claim = sample_issued_claim();
            let mut other_certificate = sample_certificate();
            other_certificate.key_id = "different-one-use-key".to_owned();
            let other_hash = other_certificate.payload_hash().expect("certificate hash");

            let err = ensure_offline_audit_input_claim_anchor(&[claim], &other_hash)
                .expect_err("input claim must match sender certificate hash");
            let InstructionExecutionError::InvariantViolation(message) = err else {
                panic!("expected invariant violation");
            };
            assert!(message.contains("offline_reason::proof_binding:"));
            assert!(message.contains("sender key certificate"));
        }

        #[test]
        fn audit_public_amounts_must_conserve_one_asset_definition() {
            let input = sample_issued_claim();
            let output = OfflineNoteAuditOutputClaim {
                note_commitment: Hash::new(b"offline-note-output-note"),
                key_certificate: sample_certificate(),
                asset: input.asset.clone(),
                amount: Numeric::new(9, 0),
            };

            let err = ensure_offline_audit_conserves_asset_amounts(&[input], &[output])
                .expect_err("audit must conserve public input and output amounts");
            assert_offline_rejection(
                err,
                "amount_conservation",
                "input amount total must equal output amount total",
            );
        }

        #[test]
        fn audit_rejects_certificate_anchor_without_topup_issued_claim() {
            let (state, asset_id, account_id, _definition_id) =
                self_escrow_test_state(Numeric::new(100, 0));
            let input_certificate = sample_certificate();
            let account_keypair = KeyPair::from_seed(vec![0x01; 32], Algorithm::Ed25519);
            let output_certificate =
                signed_sample_certificate(&account_keypair, account_id, 0x78, "audit-output-key");
            let issue = iroha_data_model::offline::OfflineNoteIssue {
                note_commitment: Hash::new(b"offline-topup-note"),
                key_certificate: input_certificate.clone(),
                asset: asset_id,
                amount: Numeric::new(10, 0),
            };
            let audit = sample_audit_bundle_for_issue(&issue, output_certificate);
            let certificate_payload_hash = input_certificate
                .payload_hash()
                .expect("certificate payload hash");
            let certificate_key = offline_note_key_certificate_key(&certificate_payload_hash);
            let relayer = sample_account(0x71);
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();
            transaction
                .world
                .offline_note_replay_keys
                .insert(certificate_key, ());

            let err = AuditOfflineNote::new(audit)
                .execute(&relayer, &mut transaction)
                .expect_err("certificate-only lineage must not anchor an input claim");
            assert_offline_rejection(err, "note_not_issued", "input claim was not issued");
        }

        #[test]
        fn audit_rejects_input_claim_without_sender_certificate_topup_anchor() {
            let (state, asset_id, account_id, _definition_id) =
                self_escrow_test_state(Numeric::new(100, 0));
            let input_certificate = sample_certificate();
            let account_keypair = KeyPair::from_seed(vec![0x01; 32], Algorithm::Ed25519);
            let output_certificate =
                signed_sample_certificate(&account_keypair, account_id, 0x79, "audit-output-key");
            let issue = iroha_data_model::offline::OfflineNoteIssue {
                note_commitment: Hash::new(b"offline-topup-note"),
                key_certificate: input_certificate,
                asset: asset_id,
                amount: Numeric::new(10, 0),
            };
            let audit = sample_audit_bundle_for_issue(&issue, output_certificate);
            let relayer = sample_account(0x72);
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = AuditOfflineNote::new(audit)
                .execute(&relayer, &mut transaction)
                .expect_err("audit must require a sender certificate anchored by topup");
            assert_offline_rejection(err, "invalid_issuer_cert", "key certificate was not issued");
        }

        #[test]
        fn audit_rejects_mutated_input_claim_even_when_certificate_topup_is_anchored() {
            let (state, asset_id, account_id, _definition_id) =
                distinct_escrow_test_state(Numeric::new(100, 0), 0x7D);
            let account_keypair = KeyPair::from_seed(vec![0x01; 32], Algorithm::Ed25519);
            let input_certificate = signed_sample_certificate(
                &account_keypair,
                account_id.clone(),
                0x7A,
                "topup-input-key",
            );
            let output_certificate = signed_sample_certificate(
                &account_keypair,
                account_id.clone(),
                0x7B,
                "audit-output-key",
            );
            let issue = iroha_data_model::offline::OfflineNoteIssue {
                note_commitment: Hash::new(b"offline-topup-note"),
                key_certificate: input_certificate,
                asset: asset_id,
                amount: Numeric::new(10, 0),
            };
            let mut audit = sample_audit_bundle_for_issue(&issue, output_certificate);
            audit.input_claims[0].note_commitment = Hash::new(b"offline-forged-topup-note");
            let public_inputs_hash = audit.public_inputs_hash().expect("mutated audit hash");
            audit.recursive_proof = placeholder_recursive_proof(public_inputs_hash);

            let relayer = sample_account(0x7C);
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();
            transaction.world.account_permissions.insert(
                account_id.clone(),
                BTreeSet::from([Permission::new(
                    CAN_MANAGE_OFFLINE_ESCROW_PERMISSION.into(),
                    Json::new(()),
                )]),
            );
            IssueOfflineNote::new(issue)
                .execute(&account_id, &mut transaction)
                .expect("online-to-offline topup should issue the original claim");

            let err = AuditOfflineNote::new(audit)
                .execute(&relayer, &mut transaction)
                .expect_err("audit must reject a claim mutation under the issued certificate");
            assert_offline_rejection(err, "note_not_issued", "input claim was not issued");
        }

        #[test]
        fn issue_rejects_note_commitment_reused_from_audit_output() {
            let (state, asset_id, account_id, _definition_id) =
                self_escrow_test_state(Numeric::new(100, 0));
            let account_keypair = KeyPair::from_seed(vec![0x01; 32], Algorithm::Ed25519);
            let key_certificate =
                signed_sample_certificate(&account_keypair, account_id.clone(), 0x86, "topup-key");
            let issue = iroha_data_model::offline::OfflineNoteIssue {
                note_commitment: Hash::new(b"offline-reused-audit-output-note"),
                key_certificate,
                asset: asset_id,
                amount: Numeric::new(10, 0),
            };
            let audit_output_key = offline_note_audit_output_key(&issue.note_commitment);
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();
            transaction.world.account_permissions.insert(
                account_id.clone(),
                BTreeSet::from([Permission::new(
                    CAN_MANAGE_OFFLINE_ESCROW_PERMISSION.into(),
                    Json::new(()),
                )]),
            );
            transaction
                .world
                .offline_note_replay_keys
                .insert(audit_output_key, ());

            let err = IssueOfflineNote::new(issue)
                .execute(&account_id, &mut transaction)
                .expect_err("topup issue must not reuse a prior audit output commitment");
            assert_offline_rejection(err, "duplicate_issue", "commitment is already issued");
        }

        #[test]
        fn audit_rejects_output_commitment_reused_from_topup_before_proof() {
            let (state, asset_id, account_id, _definition_id) =
                self_escrow_test_state(Numeric::new(100, 0));
            let account_keypair = KeyPair::from_seed(vec![0x01; 32], Algorithm::Ed25519);
            let input_certificate = signed_sample_certificate(
                &account_keypair,
                account_id.clone(),
                0x87,
                "topup-input-key",
            );
            let output_certificate =
                signed_sample_certificate(&account_keypair, account_id, 0x88, "audit-output-key");
            let issue = iroha_data_model::offline::OfflineNoteIssue {
                note_commitment: Hash::new(b"offline-topup-note"),
                key_certificate: input_certificate.clone(),
                asset: asset_id,
                amount: Numeric::new(10, 0),
            };
            let audit = sample_audit_bundle_for_issue(&issue, output_certificate);
            let certificate_payload_hash = input_certificate
                .payload_hash()
                .expect("certificate payload hash");
            let certificate_key = offline_note_key_certificate_key(&certificate_payload_hash);
            let issued_claim = OfflineNoteIssuedClaim::from_issue(&issue).expect("issued claim");
            let issued_claim_hash =
                offline_note_issued_claim_hash(issued_claim).expect("issued claim hash");
            let issued_claim_key = offline_note_issued_claim_key(&issued_claim_hash);
            let issued_output_commitment_key =
                offline_note_issue_key(audit.output_commitments.first().expect("output"));
            let relayer = sample_account(0x89);
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();
            transaction
                .world
                .offline_note_replay_keys
                .insert(certificate_key, ());
            transaction
                .world
                .offline_note_replay_keys
                .insert(issued_claim_key, ());
            transaction
                .world
                .offline_note_replay_keys
                .insert(issued_output_commitment_key, ());

            let err = AuditOfflineNote::new(audit)
                .execute(&relayer, &mut transaction)
                .expect_err("audit output must not reuse a topup note commitment");
            assert_offline_rejection(
                err,
                "duplicate_issue",
                "output commitment is already issued",
            );
        }

        #[test]
        fn audit_rejects_reused_output_certificate_from_topup_before_proof() {
            let (state, asset_id, account_id, _definition_id) =
                self_escrow_test_state(Numeric::new(100, 0));
            let account_keypair = KeyPair::from_seed(vec![0x01; 32], Algorithm::Ed25519);
            let input_certificate =
                signed_sample_certificate(&account_keypair, account_id, 0x84, "topup-input-key");
            let issue = iroha_data_model::offline::OfflineNoteIssue {
                note_commitment: Hash::new(b"offline-topup-note"),
                key_certificate: input_certificate.clone(),
                asset: asset_id,
                amount: Numeric::new(10, 0),
            };
            let audit = sample_audit_bundle_for_issue(&issue, input_certificate.clone());
            let certificate_payload_hash = input_certificate
                .payload_hash()
                .expect("certificate payload hash");
            let certificate_key = offline_note_key_certificate_key(&certificate_payload_hash);
            let issued_claim = OfflineNoteIssuedClaim::from_issue(&issue).expect("issued claim");
            let issued_claim_hash =
                offline_note_issued_claim_hash(issued_claim).expect("issued claim hash");
            let issued_claim_key = offline_note_issued_claim_key(&issued_claim_hash);
            let relayer = sample_account(0x85);
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();
            transaction
                .world
                .offline_note_replay_keys
                .insert(certificate_key, ());
            transaction
                .world
                .offline_note_replay_keys
                .insert(issued_claim_key, ());

            let err = AuditOfflineNote::new(audit)
                .execute(&relayer, &mut transaction)
                .expect_err("audit output must not reuse an issued topup key certificate");
            assert_offline_rejection(
                err,
                "duplicate_key_certificate",
                "output key certificate is already issued",
            );
        }

        #[test]
        fn audit_rejects_output_certificate_signature_before_proof() {
            let (state, asset_id, account_id, _definition_id) =
                self_escrow_test_state(Numeric::new(100, 0));
            let account_keypair = KeyPair::from_seed(vec![0x01; 32], Algorithm::Ed25519);
            let input_certificate = signed_sample_certificate(
                &account_keypair,
                account_id.clone(),
                0x80,
                "topup-input-key",
            );
            let mut output_certificate =
                signed_sample_certificate(&account_keypair, account_id, 0x81, "audit-output-key");
            output_certificate.issuer_signature = sample_signature(0x82);
            let issue = iroha_data_model::offline::OfflineNoteIssue {
                note_commitment: Hash::new(b"offline-topup-note"),
                key_certificate: input_certificate,
                asset: asset_id,
                amount: Numeric::new(10, 0),
            };
            let audit = sample_audit_bundle_for_issue(&issue, output_certificate);
            let relayer = sample_account(0x83);
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = AuditOfflineNote::new(audit)
                .execute(&relayer, &mut transaction)
                .expect_err("audited output certificate signatures must be verified");
            assert_offline_rejection(
                err,
                "invalid_issuer_cert",
                "signature does not match issuer account",
            );
        }

        #[cfg(feature = "zk-halo2-ipa")]
        #[test]
        fn audit_accepts_independent_relayer_when_topup_claim_is_anchored() {
            let issuer = KeyPair::from_seed(vec![0x73; 32], Algorithm::Ed25519);
            let authority = AccountId::new(issuer.public_key().clone());
            let relayer = sample_account(0x74);
            let escrow_account_id = sample_account(0x77);
            let domain_id = DomainId::try_new("offline", "universal").expect("domain id");
            let definition_id = AssetDefinitionId::new(
                domain_id.clone(),
                "xor".parse().expect("asset definition name"),
            );
            let asset_id = AssetId::new(definition_id.clone(), authority.clone());
            let escrow_asset_id = AssetId::new(definition_id.clone(), escrow_account_id.clone());
            let domain = Domain::new(domain_id).build(&authority);
            let account = Account::new(authority.clone()).build(&authority);
            let relayer_account = Account::new(relayer.clone()).build(&authority);
            let escrow_account = Account::new(escrow_account_id.clone()).build(&authority);
            let asset_definition =
                AssetDefinition::new(definition_id.clone(), NumericSpec::integer())
                    .with_name("xor".to_owned())
                    .build(&authority);
            let asset = Asset::new(asset_id.clone(), Numeric::new(100, 0));
            let escrow_asset = Asset::new(escrow_asset_id, Numeric::zero());
            let verifier_id = VerifyingKeyId::new(
                crate::zk::ZK_BACKEND_HALO2_IPA,
                crate::zk::OFFLINE_NOTE_RECURSIVE_CIRCUIT_ID,
            );
            let verifier_record =
                crate::zk::offline_note_recursive_vk_record(OFFLINE_NOTE_VERIFIER_NAMESPACE, 1)
                    .expect("offline recursive verifier record");
            let verifier_key = verifier_record.key.clone().expect("inline verifier key");
            let mut world = World::with_assets(
                [domain],
                [account, relayer_account, escrow_account],
                [asset_definition],
                [asset, escrow_asset],
                [],
            );
            world.account_permissions.insert(
                authority.clone(),
                BTreeSet::from([Permission::new(
                    CAN_MANAGE_OFFLINE_ESCROW_PERMISSION.into(),
                    Json::new(()),
                )]),
            );
            world
                .verifying_keys
                .insert(verifier_id.clone(), verifier_record);
            world.verifying_keys_by_circuit.insert(
                (crate::zk::OFFLINE_NOTE_RECURSIVE_CIRCUIT_ID.to_owned(), 1),
                verifier_id.clone(),
            );
            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let mut state = State::new(world, Arc::clone(&kura), query);
            state.settlement.offline.escrow_required = true;
            state
                .settlement
                .offline
                .escrow_accounts
                .insert(definition_id, escrow_account_id);
            let mut zk = state.zk.clone();
            zk.halo2.enabled = true;
            zk.halo2.max_envelope_bytes = usize::MAX;
            zk.halo2.max_proof_bytes = usize::MAX;
            state.set_zk(zk);

            let input_certificate =
                signed_sample_certificate(&issuer, authority.clone(), 0x75, "topup-input-key");
            let output_certificate =
                signed_sample_certificate(&issuer, authority.clone(), 0x76, "audit-output-key");
            let issue = iroha_data_model::offline::OfflineNoteIssue {
                note_commitment: Hash::new(b"offline-topup-note"),
                key_certificate: input_certificate,
                asset: asset_id.clone(),
                amount: Numeric::new(10, 0),
            };
            let mut audit = sample_audit_bundle_for_issue(&issue, output_certificate);
            let proving_key =
                crate::zk::derive_halo2_ipa_offline_note_proving_key_bytes(&verifier_key)
                    .expect("offline recursive proving key");
            let audit_hash = audit.public_inputs_hash().expect("audit public-input hash");
            let proof = crate::zk::prove_offline_note_audit(
                crate::zk::OFFLINE_NOTE_RECURSIVE_CIRCUIT_ID,
                &verifier_key,
                &audit,
                Some(&proving_key),
            )
            .expect("real offline audit proof");
            audit.recursive_proof = OfflineNoteRecursiveProof {
                verifier_key_id: verifier_id.clone(),
                public_inputs_hash: audit_hash,
                proof,
            };

            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();
            IssueOfflineNote::new(issue)
                .execute(&authority, &mut transaction)
                .expect("online-to-offline topup anchors input claim");
            AuditOfflineNote::new(audit.clone())
                .execute(&relayer, &mut transaction)
                .expect("bearer audit lineage may be submitted by an independent relayer");

            let output_claim = OfflineNoteIssuedClaim::from_audit_output(
                audit.output_claims.first().expect("output claim"),
            )
            .expect("issued output claim");
            let output_claim_hash =
                offline_note_issued_claim_hash(output_claim.clone()).expect("output claim hash");
            let output_claim_key = offline_note_issued_claim_key(&output_claim_hash);
            assert!(
                transaction
                    .world
                    .offline_note_replay_keys
                    .get(&output_claim_key)
                    .is_some(),
                "audit should anchor output claim lineage for later redemption"
            );
            let first_audit_output_claim = audit.output_claims.first().expect("output claim");
            let second_output_certificate =
                signed_sample_certificate(&issuer, authority.clone(), 0x79, "audit-output-key-2");
            let second_output_commitment = Hash::new(b"offline-audit-output-note-2");
            let mut second_audit = OfflineNoteAuditBundle {
                token_id: Hash::new(b"offline-audit-token-2"),
                sender_key_certificate: first_audit_output_claim.key_certificate.clone(),
                input_nullifiers: vec![Hash::new(b"offline-audit-input-nullifier-2")],
                input_claims: vec![output_claim],
                output_commitments: vec![second_output_commitment],
                output_claims: vec![OfflineNoteAuditOutputClaim {
                    note_commitment: second_output_commitment,
                    key_certificate: second_output_certificate,
                    asset: asset_id.clone(),
                    amount: Numeric::new(10, 0),
                }],
                recursive_proof: placeholder_recursive_proof(Hash::new(
                    b"offline-placeholder-public-inputs-2",
                )),
            };
            let second_audit_hash = second_audit
                .public_inputs_hash()
                .expect("second audit public-input hash");
            let second_proof = crate::zk::prove_offline_note_audit(
                crate::zk::OFFLINE_NOTE_RECURSIVE_CIRCUIT_ID,
                &verifier_key,
                &second_audit,
                Some(&proving_key),
            )
            .expect("real second-hop offline audit proof");
            second_audit.recursive_proof = OfflineNoteRecursiveProof {
                verifier_key_id: verifier_id,
                public_inputs_hash: second_audit_hash,
                proof: second_proof,
            };
            AuditOfflineNote::new(second_audit.clone())
                .execute(&relayer, &mut transaction)
                .expect("prior audit output should anchor the next offline-offline hop");
            let spent_output_claim_key = offline_note_spent_claim_key(&output_claim_hash);
            assert!(
                transaction
                    .world
                    .offline_note_replay_keys
                    .get(&spent_output_claim_key)
                    .is_some(),
                "second audit should consume the first audit output claim"
            );
            let second_output_claim = OfflineNoteIssuedClaim::from_audit_output(
                second_audit
                    .output_claims
                    .first()
                    .expect("second output claim"),
            )
            .expect("second issued output claim");
            let second_output_claim_hash =
                offline_note_issued_claim_hash(second_output_claim).expect("second claim hash");
            let second_output_claim_key = offline_note_issued_claim_key(&second_output_claim_hash);
            assert!(
                transaction
                    .world
                    .offline_note_replay_keys
                    .get(&second_output_claim_key)
                    .is_some(),
                "second audit should anchor the next output claim lineage"
            );
            let balance = transaction
                .world
                .assets
                .get(&asset_id)
                .map(|asset| asset.as_ref().clone())
                .unwrap_or_else(Numeric::zero);
            assert_eq!(balance, Numeric::new(90, 0));
        }

        #[test]
        fn offline_note_rejects_non_open_verify_envelope_proof_bytes() {
            let (state, mut proof, _public_inputs_hash) =
                offline_note_verifier_test_state(ConfidentialStatus::Active);
            proof.proof.bytes = b"legacy transcript payload".to_vec();
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let transaction = block.transaction();

            let err = offline_note_resolve_verifier(&proof, &transaction)
                .expect_err("legacy transcript bytes must not decode as OpenVerifyEnvelope");
            assert_offline_rejection(err, "invalid_proof", "OpenVerifyEnvelope");
        }

        #[test]
        fn offline_note_rejects_wrong_verifier_key_id_backend() {
            let (state, mut proof, _public_inputs_hash) =
                offline_note_verifier_test_state(ConfidentialStatus::Active);
            proof.verifier_key_id =
                VerifyingKeyId::new("stark/fri", crate::zk::OFFLINE_NOTE_RECURSIVE_CIRCUIT_ID);
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let transaction = block.transaction();

            let err = offline_note_resolve_verifier(&proof, &transaction)
                .expect_err("proof backend must match the selected verifier key id");
            assert_offline_rejection(err, "verifier_key_invalid", "backend");
        }

        #[test]
        fn offline_note_rejects_non_transparent_proof_backends() {
            for backend in ["halo2/pasta", "stark/fri/"] {
                let (state, mut proof, _public_inputs_hash) =
                    offline_note_verifier_test_state(ConfidentialStatus::Active);
                proof.verifier_key_id =
                    VerifyingKeyId::new(backend, crate::zk::OFFLINE_NOTE_RECURSIVE_CIRCUIT_ID);
                proof.proof.backend = backend.to_owned();
                let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
                let mut block = state.block(header);
                let transaction = block.transaction();

                let err = offline_note_resolve_verifier(&proof, &transaction)
                    .expect_err("offline proofs must not accept trusted-setup backend labels");
                assert_offline_rejection(err, "verifier_key_invalid", "transparent");
            }
        }

        #[test]
        fn offline_note_rejects_non_production_backend_labels_before_registry_lookup() {
            for backend in [
                "groth16/bn254",
                "halo2/kzg",
                "halo2/ipa:KZG",
                "halo2/ipa: KZG",
                "halo2/bn254",
                "debug-proof",
                "Debug-Proof",
                "mock-proof",
                "Mock-Proof",
                "stark/fri/debug-proof",
                "stark/fri/Debug-Proof",
                "stark/fri/mock-proof",
                "halo2/ipa:mock-proof",
                "halo2/ipa:Mock-Proof",
            ] {
                let (state, mut proof, _public_inputs_hash) =
                    offline_note_verifier_test_state(ConfidentialStatus::Active);
                proof.verifier_key_id =
                    VerifyingKeyId::new(backend, crate::zk::OFFLINE_NOTE_RECURSIVE_CIRCUIT_ID);
                proof.proof.backend = backend.to_owned();
                let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
                let mut block = state.block(header);
                let transaction = block.transaction();

                let err = offline_note_resolve_verifier(&proof, &transaction)
                    .expect_err("offline proofs must not accept non-production backend labels");
                assert_offline_rejection(
                    err,
                    "verifier_key_invalid",
                    "trusted-setup or developer-only",
                );
            }
        }

        #[test]
        fn offline_note_rejects_transparent_backend_tag_mismatch() {
            let (mut state, proof, _public_inputs_hash) =
                offline_note_verifier_test_state(ConfidentialStatus::Active);
            let verifier_id = proof.verifier_key_id.clone();
            let vk_box = VerifyingKeyBox::new(
                crate::zk::ZK_BACKEND_HALO2_IPA.to_owned(),
                b"offline-note-test-verifying-key".to_vec(),
            );
            let commitment = crate::zk::hash_vk(&vk_box);
            let mut record = VerifyingKeyRecord::new_with_owner(
                1,
                crate::zk::OFFLINE_NOTE_RECURSIVE_CIRCUIT_ID,
                None,
                OFFLINE_NOTE_VERIFIER_NAMESPACE,
                BackendTag::Halo2IpaPasta,
                "pasta",
                offline_note_recursive_public_inputs_schema_hash(),
                commitment,
            );
            record.key = Some(vk_box);
            record.status = ConfidentialStatus::Active;
            record.max_proof_bytes = 4096;
            record.vk_len = b"offline-note-test-verifying-key".len() as u32;
            record.backend = BackendTag::Halo2Bn254;
            state.world.verifying_keys.insert(verifier_id, record);
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let transaction = block.transaction();

            let err = offline_note_resolve_verifier(&proof, &transaction)
                .expect_err("offline verifier backend tag must match backend label");
            assert_offline_rejection(err, "verifier_key_invalid", "backend tag");
        }

        #[test]
        fn offline_note_rejects_recursive_envelope_unbound_metadata() {
            let cases: [(&str, fn(&mut OpenVerifyEnvelope)); 2] = [
                ("verifier commitment mismatch", |envelope| {
                    envelope.vk_hash = [0u8; 32];
                }),
                ("empty auxiliary bytes", |envelope| {
                    envelope.aux = b"offline-note-forged-aux".to_vec();
                }),
            ];

            for (detail, mutate) in cases {
                let (state, mut proof, _public_inputs_hash) =
                    offline_note_verifier_test_state(ConfidentialStatus::Active);
                mutate_offline_note_recursive_envelope(&mut proof, mutate);
                let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
                let mut block = state.block(header);
                let transaction = block.transaction();

                let err = offline_note_resolve_verifier(&proof, &transaction)
                    .expect_err("offline recursive envelope metadata substitution must reject");
                assert_offline_rejection(err, "invalid_proof", detail);
            }
        }

        #[test]
        fn offline_note_rejects_empty_recursive_verifier_key_id_name() {
            let (state, mut proof, _public_inputs_hash) =
                offline_note_verifier_test_state(ConfidentialStatus::Active);
            proof.verifier_key_id.name = "   ".to_owned();
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let transaction = block.transaction();

            let err = offline_note_resolve_verifier(&proof, &transaction)
                .expect_err("empty Offline recursive verifier-key id name must reject");
            assert_offline_rejection(
                err,
                "verifier_key_invalid",
                "verifier key id name must be non-empty",
            );
        }

        #[test]
        fn offline_note_rejects_self_consistent_noncanonical_recursive_circuit() {
            for forged_circuit in [
                "halo2/ipa:offline-note-recursive",
                "halo2/ipa/offline-note-recursive",
                "halo2/pasta/offline-note-recursive",
                "halo2/pasta/ipa/offline-note-recursive",
                "halo2/ipa:offline-note-recursive-shadow",
            ] {
                let (state, mut proof, _public_inputs_hash) =
                    offline_note_verifier_test_state(ConfidentialStatus::Active);
                mutate_offline_note_recursive_envelope(&mut proof, |envelope| {
                    envelope.circuit_id = forged_circuit.to_owned();
                });
                let verifier_id = proof.verifier_key_id.clone();
                let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
                let mut block = state.block(header);
                {
                    let mut mutation_transaction = block.transaction();
                    let mut record = mutation_transaction
                        .world
                        .verifying_keys
                        .get(&verifier_id)
                        .expect("verifier record")
                        .clone();
                    mutation_transaction
                        .world
                        .verifying_keys_by_circuit
                        .remove((record.circuit_id.clone(), record.version));
                    record.circuit_id = forged_circuit.to_owned();
                    mutation_transaction.world.verifying_keys_by_circuit.insert(
                        (record.circuit_id.clone(), record.version),
                        verifier_id.clone(),
                    );
                    mutation_transaction
                        .world
                        .verifying_keys
                        .insert(verifier_id.clone(), record);
                    mutation_transaction.apply();
                }
                let transaction = block.transaction();

                let err = offline_note_resolve_verifier(&proof, &transaction).expect_err(
                    "self-consistent noncanonical Offline recursive circuit must reject",
                );
                assert_offline_rejection(err, "verifier_key_invalid", "offline-note-recursive");
            }
        }

        #[test]
        fn offline_note_rejects_recursive_verifier_record_mismatches() {
            assert_offline_note_record_mutation_rejects(
                |state, verifier_id, _proof| {
                    mutate_verifier_record(state, verifier_id, |record| {
                        record.key = None;
                    });
                },
                "verifier_key_invalid",
                "not available inline",
            );
            assert_offline_note_record_mutation_rejects(
                |state, verifier_id, _proof| {
                    mutate_verifier_record(state, verifier_id, |record| {
                        let empty_key = VerifyingKeyBox::new(
                            record.key.as_ref().expect("key").backend.clone(),
                            Vec::new(),
                        );
                        record.commitment = crate::zk::hash_vk(&empty_key);
                        record.vk_len = 0;
                        record.key = Some(empty_key);
                    });
                },
                "verifier_key_invalid",
                "key bytes must be non-empty",
            );
            assert_offline_note_record_mutation_rejects(
                |state, verifier_id, _proof| {
                    mutate_verifier_record(state, verifier_id, |record| {
                        record.vk_len += 1;
                    });
                },
                "verifier_key_invalid",
                "key length",
            );
            assert_offline_note_record_mutation_rejects(
                |state, verifier_id, _proof| {
                    mutate_verifier_record(state, verifier_id, |record| {
                        record.max_proof_bytes = 0;
                    });
                },
                "verifier_key_invalid",
                "max_proof_bytes",
            );
            assert_offline_note_record_mutation_rejects(
                |state, verifier_id, proof| {
                    mutate_verifier_record(state, verifier_id, |record| {
                        record.max_proof_bytes =
                            u32::try_from(proof.proof.bytes.len().saturating_sub(1))
                                .expect("proof length fits u32");
                    });
                },
                "invalid_proof",
                "max_proof_bytes",
            );
            assert_offline_note_record_mutation_rejects(
                |state, verifier_id, _proof| {
                    mutate_verifier_record(state, verifier_id, |record| {
                        record.namespace = "not_offline_note".to_owned();
                    });
                },
                "verifier_schema_mismatch",
                "Offline namespace",
            );
            assert_offline_note_record_mutation_rejects(
                |state, verifier_id, _proof| {
                    mutate_verifier_record(state, verifier_id, |record| {
                        record.public_inputs_schema_hash = [0xA5; 32];
                    });
                },
                "verifier_schema_mismatch",
                "public-input schema",
            );
            assert_offline_note_record_mutation_rejects(
                |transaction, verifier_id, _proof| {
                    let record = transaction
                        .world
                        .verifying_keys
                        .get(verifier_id)
                        .expect("verifier record")
                        .clone();
                    transaction
                        .world
                        .verifying_keys_by_circuit
                        .remove((record.circuit_id, record.version));
                },
                "verifier_key_inactive",
                "circuit/version",
            );
            assert_offline_note_record_mutation_rejects(
                |state, verifier_id, _proof| {
                    mutate_verifier_record(state, verifier_id, |record| {
                        record.commitment = [0x77; 32];
                    });
                },
                "verifier_key_invalid",
                "commitment",
            );
            #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
            assert_offline_note_record_mutation_rejects(
                |state, verifier_id, _proof| {
                    mutate_verifier_record(state, verifier_id, |record| {
                        let mut noncanonical_key = crate::zk::offline_note_recursive_vk_box()
                            .expect("Offline recursive key");
                        let last = noncanonical_key
                            .bytes
                            .last_mut()
                            .expect("Offline recursive key bytes");
                        *last ^= 0x01;
                        record.commitment = crate::zk::hash_vk(&noncanonical_key);
                        record.vk_len = u32::try_from(noncanonical_key.bytes.len())
                            .expect("Offline recursive verifier key length fits u32");
                        record.key = Some(noncanonical_key);
                    });
                },
                "verifier_key_invalid",
                "canonical semantic circuit key",
            );
        }

        #[test]
        fn offline_note_rejects_inactive_verifier_key() {
            let (state, proof, _public_inputs_hash) =
                offline_note_verifier_test_state(ConfidentialStatus::Proposed);
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let transaction = block.transaction();

            let err = offline_note_resolve_verifier(&proof, &transaction)
                .expect_err("inactive Offline verifier keys must reject proofs");
            assert_offline_rejection(err, "verifier_key_inactive", "not active");
        }

        #[test]
        fn offline_note_redeem_and_audit_reject_public_input_hash_mismatch() {
            let (state, proof, _public_inputs_hash) =
                offline_note_verifier_test_state(ConfidentialStatus::Active);
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let redeem_err = verify_offline_note_recursive_proof(
                &proof,
                &Hash::new(b"offline-note-wrong-redeem-inputs"),
                Vec::new(),
                &mut transaction,
            )
            .expect_err("redeem proof must be bound to the expected public inputs");
            assert_offline_rejection(redeem_err, "proof_binding", "expected public inputs");

            let audit_err = verify_offline_note_recursive_proof(
                &proof,
                &Hash::new(b"offline-note-wrong-audit-inputs"),
                Vec::new(),
                &mut transaction,
            )
            .expect_err("audit proof must be bound to the expected public inputs");
            assert_offline_rejection(audit_err, "proof_binding", "expected public inputs");
        }

        #[test]
        fn kagemusha_transfer_rejects_disabled_or_legacy_forced_config() {
            let authority = sample_account(0x41);
            for (enabled, force_legacy, label) in [
                (false, false, "kagemusha_disabled"),
                (true, true, "kagemusha_legacy_forced"),
            ] {
                let kura = Kura::blank_kura_for_testing();
                let query = LiveQueryStore::start_test();
                let mut state = State::new(World::default(), Arc::clone(&kura), query);
                state.settlement.offline.kagemusha_enabled = enabled;
                state.settlement.offline.kagemusha_force_legacy = force_legacy;
                let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
                let mut block = state.block(header);
                let mut transaction = block.transaction();

                let err = sample_kagemusha_transfer(crate::zk::ZK_BACKEND_HALO2_IPA)
                    .execute(&authority, &mut transaction)
                    .expect_err("Kagemusha config gate must reject");
                assert_offline_rejection(err, label, "Kagemusha");
            }
        }

        #[test]
        fn kagemusha_transfer_rejects_trusted_setup_backend_labels() {
            let authority = sample_account(0x42);
            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let state = State::new(World::default(), Arc::clone(&kura), query);
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            for backend in [
                "halo2/pasta",
                "halo2/ipa:KZG",
                "halo2/ipa: KZG",
                "halo2/ipa:Mock-Proof",
            ] {
                let err = sample_kagemusha_transfer(backend)
                    .execute(&authority, &mut transaction)
                    .expect_err("Kagemusha must reject non-transparent proof backends");
                assert_offline_rejection(err, "verifier_key_invalid", "transparent");
            }
        }

        #[test]
        fn kagemusha_transfer_rejects_unbound_asset_verifier() {
            let authority = sample_account(0x43);
            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let state = State::new(World::default(), Arc::clone(&kura), query);
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = sample_kagemusha_transfer(crate::zk::ZK_BACKEND_HALO2_IPA)
                .execute(&authority, &mut transaction)
                .expect_err("Kagemusha must require an asset-bound verifier");
            assert_offline_rejection(err, "verifier_key_invalid", "configured shielded asset");
        }

        #[test]
        fn kagemusha_transfer_executes_real_confidential_transfer_v2_proof() {
            let (
                state,
                authority,
                definition_id,
                transfer,
                expected_commitments,
                expected_new_roots,
            ) = real_kagemusha_test_state();
            let input = transfer.inputs[0];
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            transfer
                .execute(&authority, &mut transaction)
                .expect("real Halo2 IPA Kagemusha transfer should execute");

            let shielded_state = transaction
                .world
                .zk_assets
                .get(&definition_id)
                .expect("Kagemusha transfer must create shielded asset state");
            assert!(
                shielded_state.nullifiers.contains(&input),
                "input nullifier should be recorded as spent"
            );
            assert_eq!(shielded_state.commitments, expected_commitments);
            assert_eq!(
                shielded_state.root_history.last().copied(),
                expected_new_roots.last().copied(),
                "final confidential root should be recorded after appending outputs"
            );
        }

        #[test]
        fn kagemusha_transfer_rejects_tampered_real_halo2_ipa_proof() {
            let (state, authority, _definition_id, mut transfer, _commitments, _roots) =
                real_kagemusha_test_state();
            let last = transfer
                .proof
                .proof
                .bytes
                .last_mut()
                .expect("fixture proof bytes must not be empty");
            *last ^= 0x01;
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = transfer
                .execute(&authority, &mut transaction)
                .expect_err("tampered real Kagemusha proof must reject");
            let message = err.to_string();
            assert!(
                message.contains("invalid transfer proof")
                    || message.contains("OpenVerifyEnvelope")
                    || message.contains("invalid OpenVerifyEnvelope payload")
                    || message.contains("invalid confidential transfer v2 public inputs"),
                "unexpected error: {message}"
            );
        }

        #[test]
        fn kagemusha_transfer_rejects_verifier_record_mismatches_before_proof_decode() {
            assert_kagemusha_transfer_record_mutation_rejects(
                |transaction, verifier_id| {
                    let record = transaction
                        .world
                        .verifying_keys
                        .get(verifier_id)
                        .expect("verifier record")
                        .clone();
                    transaction
                        .world
                        .verifying_keys_by_circuit
                        .remove((record.circuit_id, record.version));
                },
                "verifier_key_inactive",
                "circuit/version",
            );
            assert_kagemusha_transfer_record_mutation_rejects(
                |transaction, verifier_id| {
                    mutate_verifier_record(transaction, verifier_id, |record| {
                        record.namespace = "generic_confidential_transfer".to_owned();
                    });
                },
                "verifier_key_invalid",
                "Kagemusha namespace",
            );
            assert_kagemusha_transfer_record_mutation_rejects(
                |transaction, verifier_id| {
                    let mut record = transaction
                        .world
                        .verifying_keys
                        .get(verifier_id)
                        .expect("verifier record")
                        .clone();
                    transaction
                        .world
                        .verifying_keys_by_circuit
                        .remove((record.circuit_id.clone(), record.version));
                    record.circuit_id =
                        "anon-transfer-2x2-merkle16-poseidon-diversified".to_owned();
                    transaction.world.verifying_keys_by_circuit.insert(
                        (record.circuit_id.clone(), record.version),
                        verifier_id.clone(),
                    );
                    transaction
                        .world
                        .verifying_keys
                        .insert(verifier_id.clone(), record);
                },
                "verifier_key_invalid",
                "canonical",
            );
            assert_kagemusha_transfer_record_mutation_rejects(
                |transaction, verifier_id| {
                    mutate_verifier_record(transaction, verifier_id, |record| {
                        record.key = None;
                    });
                },
                "verifier_key_invalid",
                "not available inline",
            );
            assert_kagemusha_transfer_record_mutation_rejects(
                |transaction, verifier_id| {
                    mutate_verifier_record(transaction, verifier_id, |record| {
                        record.vk_len += 1;
                    });
                },
                "verifier_key_invalid",
                "key length",
            );
            assert_kagemusha_transfer_record_mutation_rejects(
                |transaction, verifier_id| {
                    mutate_verifier_record(transaction, verifier_id, |record| {
                        record.max_proof_bytes = 0;
                    });
                },
                "verifier_key_invalid",
                "non-zero max_proof_bytes",
            );
            assert_kagemusha_transfer_record_mutation_rejects(
                |transaction, verifier_id| {
                    mutate_verifier_record(transaction, verifier_id, |record| {
                        record.max_proof_bytes = 1;
                    });
                },
                "invalid_proof",
                "max_proof_bytes",
            );
            assert_kagemusha_transfer_record_mutation_rejects(
                |transaction, verifier_id| {
                    mutate_verifier_record(transaction, verifier_id, |record| {
                        let key = record.key.as_mut().expect("inline verifier key");
                        key.backend = "stark/fri".to_owned();
                    });
                },
                "verifier_key_invalid",
                "inline verifier key backend",
            );
            assert_kagemusha_transfer_record_mutation_rejects(
                |transaction, verifier_id| {
                    mutate_verifier_record(transaction, verifier_id, |record| {
                        let key = record.key.as_mut().expect("inline verifier key");
                        key.bytes.clear();
                        record.vk_len = 0;
                    });
                },
                "verifier_key_invalid",
                "key bytes must be non-empty",
            );
            assert_kagemusha_transfer_record_mutation_rejects(
                |transaction, verifier_id| {
                    mutate_verifier_record(transaction, verifier_id, |record| {
                        let key = record.key.as_mut().expect("inline verifier key");
                        let first = key.bytes.first_mut().expect("non-empty verifier key");
                        *first ^= 0x01;
                    });
                },
                "verifier_key_invalid",
                "inline verifier-key commitment",
            );

            let (state, authority, definition_id, mut transfer, _commitments, _roots) =
                real_kagemusha_test_state();
            let verifier_id = transfer.proof.vk_ref.clone();
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            {
                let mut mutation_transaction = block.transaction();
                let mut noncanonical_key = {
                    let record = mutation_transaction
                        .world
                        .verifying_keys
                        .get(&verifier_id)
                        .expect("verifier record");
                    record.key.clone().expect("inline verifier key")
                };
                let last = noncanonical_key
                    .bytes
                    .last_mut()
                    .expect("non-empty verifier key");
                *last ^= 0x01;
                let noncanonical_commitment = crate::zk::hash_vk(&noncanonical_key);
                transfer.proof.vk_commitment = Some(noncanonical_commitment);
                transfer.proof.proof.bytes = b"not-an-open-verify-envelope".to_vec();
                mutate_verifier_record(&mut mutation_transaction, &verifier_id, |record| {
                    record.commitment = noncanonical_commitment;
                    record.vk_len = u32::try_from(noncanonical_key.bytes.len())
                        .expect("verifier key length fits u32");
                    record.key = Some(noncanonical_key);
                });
                let zk_state = mutation_transaction
                    .world
                    .zk_assets
                    .get_mut(&definition_id)
                    .expect("Kagemusha zk asset state");
                let binding = zk_state
                    .vk_transfer
                    .as_mut()
                    .expect("Kagemusha transfer verifier binding");
                binding.commitment = noncanonical_commitment;
                mutation_transaction.apply();
            }
            let mut transaction = block.transaction();

            let err = transfer
                .execute(&authority, &mut transaction)
                .expect_err("self-consistent noncanonical transfer verifier must reject");
            assert_offline_rejection(
                err,
                "verifier_key_invalid",
                "canonical semantic circuit key",
            );
        }

        #[test]
        fn kagemusha_transfer_rejects_confidential_v2_public_input_mismatches() {
            let cases: [(
                &str,
                fn(&mut State, &AssetDefinitionId, &mut KagemushaTransfer),
            ); 5] = [
                ("root_hint mismatch", |state, definition_id, transfer| {
                    let forged_root = [0xA4; 32];
                    let envelope: OpenVerifyEnvelope =
                        norito::decode_from_bytes(&transfer.proof.proof.bytes)
                            .expect("OpenVerifyEnvelope");
                    let mut zk_state = ZkAssetState::default();
                    zk_state
                        .root_history
                        .push(transfer.root_hint.expect("sample has root hint"));
                    zk_state.root_history.push(forged_root);
                    zk_state.vk_transfer = Some(ZkAssetVerifierBinding {
                        id: transfer.proof.vk_ref.clone(),
                        commitment: envelope.vk_hash,
                    });
                    state
                        .world
                        .zk_assets
                        .insert(definition_id.clone(), zk_state);
                    transfer.root_hint = Some(forged_root);
                }),
                ("nullifier mismatch", |_state, _definition_id, transfer| {
                    transfer.inputs[0][0] ^= 0x01;
                }),
                (
                    "output commitment mismatch",
                    |_state, _definition_id, transfer| {
                        transfer.outputs[0][0] ^= 0x01;
                    },
                ),
                ("chain tag mismatch", |state, _definition_id, _transfer| {
                    state.chain_id = "kagemusha-transfer-other-chain".parse().expect("chain id");
                }),
                ("asset tag mismatch", |state, definition_id, transfer| {
                    let other_definition_id = AssetDefinitionId::new(
                        definition_id.domain().clone(),
                        "kgm-other".parse().expect("asset definition name"),
                    );
                    let other_definition =
                        AssetDefinition::new(other_definition_id.clone(), NumericSpec::integer())
                            .with_name("kgm-other".to_owned())
                            .confidential_policy(AssetConfidentialPolicy::convertible())
                            .build(&sample_account(0x46));
                    state
                        .world
                        .asset_definitions
                        .insert(other_definition_id.clone(), other_definition);
                    let envelope: OpenVerifyEnvelope =
                        norito::decode_from_bytes(&transfer.proof.proof.bytes)
                            .expect("OpenVerifyEnvelope");
                    let mut zk_state = ZkAssetState::default();
                    zk_state
                        .root_history
                        .push(transfer.root_hint.expect("sample has root hint"));
                    zk_state.vk_transfer = Some(ZkAssetVerifierBinding {
                        id: transfer.proof.vk_ref.clone(),
                        commitment: envelope.vk_hash,
                    });
                    state
                        .world
                        .zk_assets
                        .insert(other_definition_id.clone(), zk_state);
                    transfer.asset = other_definition_id;
                }),
            ];

            for (expected, mutate) in cases {
                let (mut state, authority, definition_id, mut transfer, _commitments, _roots) =
                    real_kagemusha_test_state();
                mutate(&mut state, &definition_id, &mut transfer);
                let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
                let mut block = state.block(header);
                let mut transaction = block.transaction();

                let err = transfer
                    .execute(&authority, &mut transaction)
                    .expect_err("forged Kagemusha public inputs must reject");
                let message = err.to_string();
                assert!(
                    message.contains(expected),
                    "expected {expected:?} rejection, got {message}"
                );
            }
        }

        #[test]
        fn kagemusha_transfer_rejects_backend_field_mismatches() {
            let authority = sample_account(0x44);
            let cases: [fn(&mut KagemushaTransfer); 2] = [
                |transfer: &mut KagemushaTransfer| {
                    transfer.proof.proof.backend = "stark/fri".to_owned();
                },
                |transfer: &mut KagemushaTransfer| {
                    transfer.proof.vk_ref =
                        VerifyingKeyId::new("stark/fri", "offline-kagemusha-transfer");
                },
            ];
            for mutate in cases {
                let mut transfer = sample_kagemusha_transfer(crate::zk::ZK_BACKEND_HALO2_IPA);
                mutate(&mut transfer);
                let kura = Kura::blank_kura_for_testing();
                let query = LiveQueryStore::start_test();
                let state = State::new(World::default(), Arc::clone(&kura), query);
                let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
                let mut block = state.block(header);
                let mut transaction = block.transaction();

                let err = transfer
                    .execute(&authority, &mut transaction)
                    .expect_err("Kagemusha must bind attachment backend fields");
                assert_offline_rejection(err, "proof_binding", "must match");
            }
        }

        #[test]
        fn kagemusha_transfer_rejects_empty_input_or_output_shape() {
            let authority = sample_account(0x43);
            let mut transfer = sample_kagemusha_transfer(crate::zk::ZK_BACKEND_HALO2_IPA);
            transfer.inputs.clear();
            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let state = State::new(World::default(), Arc::clone(&kura), query);
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut transaction = block.transaction();

            let err = transfer
                .execute(&authority, &mut transaction)
                .expect_err("Kagemusha must reject empty input nullifiers");
            assert_offline_rejection(err, "invalid_proof", "1 to 2 input nullifiers");
        }

        #[test]
        fn kagemusha_transfer_rejects_duplicate_sets_before_proof_decode() {
            let cases: [(&str, &str, fn(&mut KagemushaTransfer)); 2] = [
                (
                    "duplicate_nullifier",
                    "input nullifiers must be unique",
                    |transfer: &mut KagemushaTransfer| {
                        transfer.inputs.push(transfer.inputs[0]);
                    },
                ),
                (
                    "duplicate_output",
                    "output commitments must be unique",
                    |transfer: &mut KagemushaTransfer| {
                        transfer.outputs.push(transfer.outputs[0]);
                    },
                ),
            ];

            for (label, detail, mutate) in cases {
                let (state, authority, _definition_id, mut transfer, _commitments, _roots) =
                    real_kagemusha_test_state();
                mutate(&mut transfer);
                transfer.proof.proof.bytes = b"not-an-open-verify-envelope".to_vec();
                let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
                let mut block = state.block(header);
                let mut transaction = block.transaction();

                let err = transfer.execute(&authority, &mut transaction).expect_err(
                    "duplicate Kagemusha transfer sets must reject before proof decode",
                );
                assert_offline_rejection(err, label, detail);
            }
        }

        #[test]
        fn kagemusha_transfer_rejects_zero_sets_before_proof_decode() {
            let cases: [(&str, fn(&mut KagemushaTransfer)); 2] = [
                ("input nullifiers must be non-zero", |transfer| {
                    transfer.inputs[0] = [0u8; 32];
                }),
                ("output commitments must be non-zero", |transfer| {
                    transfer.outputs[0] = [0u8; 32];
                }),
            ];

            for (detail, mutate) in cases {
                let (state, authority, _definition_id, mut transfer, _commitments, _roots) =
                    real_kagemusha_test_state();
                mutate(&mut transfer);
                transfer.proof.proof.bytes = b"not-an-open-verify-envelope".to_vec();
                let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
                let mut block = state.block(header);
                let mut transaction = block.transaction();

                let err = transfer
                    .execute(&authority, &mut transaction)
                    .expect_err("zero Kagemusha transfer sets must reject before proof decode");
                assert_offline_rejection(err, "invalid_proof", detail);
            }
        }

        #[test]
        fn kagemusha_transfer_rejects_oversized_input_or_output_shape() {
            let authority = sample_account(0x45);
            let cases: [fn(&mut KagemushaTransfer); 2] = [
                |transfer: &mut KagemushaTransfer| transfer.inputs.push([0x44; 32]),
                |transfer: &mut KagemushaTransfer| transfer.outputs.push([0x55; 32]),
            ];
            for mutate in cases {
                let mut transfer = sample_kagemusha_transfer(crate::zk::ZK_BACKEND_HALO2_IPA);
                transfer.inputs.push([0x66; 32]);
                transfer.outputs.push([0x77; 32]);
                mutate(&mut transfer);
                let kura = Kura::blank_kura_for_testing();
                let query = LiveQueryStore::start_test();
                let state = State::new(World::default(), Arc::clone(&kura), query);
                let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
                let mut block = state.block(header);
                let mut transaction = block.transaction();

                let err = transfer
                    .execute(&authority, &mut transaction)
                    .expect_err("Kagemusha must reject more than two inputs or outputs");
                assert_offline_rejection(err, "invalid_proof", "1 to 2 input nullifiers");
            }
        }

        #[test]
        fn audit_replay_keys_cover_input_spend_and_output_issue_domains() {
            let claim_hash =
                offline_note_issued_claim_hash(sample_issued_claim()).expect("claim hash");
            let nullifier = Hash::new(b"offline-note-input-nullifier");
            let output_commitment = Hash::new(b"offline-note-output-commitment");

            let issued_claim_key = offline_note_issued_claim_key(&claim_hash);
            let spent_claim_key = offline_note_spent_claim_key(&claim_hash);
            let nullifier_key = offline_note_nullifier_key(&nullifier);
            let audit_nullifier_key = offline_note_audit_nullifier_key(&nullifier);
            let audit_output_key = offline_note_audit_output_key(&output_commitment);

            assert_eq!(
                issued_claim_key,
                offline_note_replay_key(OFFLINE_NOTE_REPLAY_ISSUED_CLAIM_DOMAIN, &claim_hash)
            );
            assert_eq!(
                spent_claim_key,
                offline_note_replay_key(OFFLINE_NOTE_REPLAY_SPENT_CLAIM_DOMAIN, &claim_hash)
            );
            assert_eq!(
                nullifier_key,
                offline_note_replay_key(OFFLINE_NOTE_REPLAY_NULLIFIER_DOMAIN, &nullifier)
            );
            assert_eq!(
                audit_output_key,
                offline_note_replay_key(
                    OFFLINE_NOTE_REPLAY_AUDIT_OUTPUT_DOMAIN,
                    &output_commitment,
                )
            );
            assert_eq!(
                BTreeSet::from([
                    issued_claim_key,
                    spent_claim_key,
                    nullifier_key,
                    audit_nullifier_key,
                    audit_output_key,
                ])
                .len(),
                5
            );
        }
    }
}
