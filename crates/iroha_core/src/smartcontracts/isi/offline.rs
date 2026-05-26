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
        offline::{AuditOfflineNote, IssueOfflineNote, RedeemOfflineNote},
    },
    offline::{
        OFFLINE_NOTE_RECURSIVE_PUBLIC_INPUTS_SCHEMA, OFFLINE_REJECTION_REASON_PREFIX,
        OfflineNoteAuditOutputClaim, OfflineNoteIssuedClaim, OfflineNoteKeyCertificate,
        OfflineNoteRecursiveProof, offline_note_recursive_public_inputs_schema_hash,
    },
    proof::{ProofBox, VerifyingKeyBox, VerifyingKeyId, VerifyingKeyRecord},
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
        let backend = verifier_id.backend.as_str();
        if backend.starts_with("debug/") {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "offline recursive proofs may not use debug proof backends",
            )
            .into());
        }
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
        if vk_box.backend != verifier_id.backend || vk_box.backend != proof.proof.backend {
            return Err(labeled_invariant(
                "verifier_key_invalid",
                "offline recursive verifier backend mismatch",
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
        if envelope.vk_hash != [0u8; 32] && envelope.vk_hash != record.commitment {
            return Err(labeled_invariant(
                "invalid_proof",
                "offline recursive proof verifier commitment mismatch",
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
            authority: &AccountId,
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
            if audit.output_claims.is_empty()
                || audit.output_claims.len() > audit.output_commitments.len()
            {
                return Err(labeled_invariant(
                    "invalid_proof",
                    "offline audit requires 1 to 2 output claims bound to output commitments",
                )
                .into());
            }
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
                ensure_offline_note_certificate_signature(
                    &output_claim.key_certificate,
                    authority,
                )?;
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
                let spec = state_transaction.numeric_spec_for(output_claim.asset.definition())?;
                assert_numeric_spec_with(&output_claim.amount, spec)?;
            }
            validate_offline_note_key_certificate(&audit.sender_key_certificate)?;
            ensure_can_submit_offline_note_for_account(
                &audit.sender_key_certificate.account_id,
                authority,
                state_transaction,
            )?;
            let certificate_payload_hash =
                audit.sender_key_certificate.payload_hash().map_err(|err| {
                    labeled_invariant(
                        "invalid_issuer_cert",
                        format!("failed to encode Offline key certificate payload: {err}"),
                    )
                })?;
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

    #[cfg(test)]
    mod tests {
        use std::{collections::BTreeSet, sync::Arc};

        use super::*;
        use crate::{
            kura::Kura,
            query::store::LiveQueryStore,
            state::{State, World},
        };
        use iroha_crypto::{KeyPair, Signature};
        use iroha_data_model::{
            Registrable,
            account::Account,
            asset::{Asset, AssetDefinition},
            block::BlockHeader,
            domain::{Domain, DomainId},
        };
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

        fn offline_note_verifier_test_state(
            status: ConfidentialStatus,
        ) -> (State, OfflineNoteRecursiveProof, Hash) {
            let verifier_id = VerifyingKeyId::new(
                crate::zk::ZK_BACKEND_HALO2_IPA,
                crate::zk::OFFLINE_NOTE_RECURSIVE_CIRCUIT_ID,
            );
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
            record.status = status;
            record.max_proof_bytes = 4096;
            record.vk_len = b"offline-note-test-verifying-key".len() as u32;

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
