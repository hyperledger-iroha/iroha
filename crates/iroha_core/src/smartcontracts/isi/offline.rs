//! Offline V2 note instruction execution.

use super::prelude::*;
use crate::smartcontracts::isi::asset::isi::assert_numeric_spec_with;
use std::{collections::BTreeSet, time::Duration};

use iroha_crypto::{Algorithm, Hash, PublicKey};
use iroha_data_model::{
    account::AccountId,
    asset::{AssetDefinitionId, AssetId},
    confidential::ConfidentialStatus,
    events::data::prelude::{
        OfflineNoteAuditRecorded, OfflineNoteEvent, OfflineNoteIssued, OfflineNoteRedeemed,
    },
    isi::{
        error::{InstructionExecutionError, InvalidParameterError, MathError},
        offline::{AuditOfflineNoteV2, IssueOfflineNoteV2, RedeemOfflineNoteV2},
    },
    offline::{
        OFFLINE_NOTE_V2_RECURSIVE_PUBLIC_INPUTS_SCHEMA_V1, OFFLINE_REJECTION_REASON_PREFIX,
        OfflineNoteIssuedClaimV2, OfflineNoteKeyCertificateV2, OfflineNoteRecursiveProofV2,
        offline_note_v2_recursive_public_inputs_schema_hash,
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
    if let Some(account) = state_transaction
        .settlement
        .offline
        .escrow_accounts
        .get(definition)
    {
        return Ok(Some(account.clone()));
    }
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
    if let Some(account) = state_transaction
        .settlement
        .offline
        .escrow_accounts
        .get(source_id.definition())
    {
        return Ok(account == source_id.account());
    }

    let asset_definition = state_transaction
        .world
        .asset_definition(source_id.definition())?;

    if !crate::smartcontracts::isi::domain::isi::asset_definition_offline_enabled(
        asset_definition.metadata(),
    )? {
        return Ok(false);
    }

    let derived = crate::smartcontracts::isi::domain::isi::offline_escrow_account_id(
        state_transaction.chain_id(),
        source_id.definition(),
    );
    Ok(&derived == source_id.account())
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

/// Execution logic for Offline V2 note instructions.
pub mod isi {
    use super::*;

    const OFFLINE_NOTE_V2_VERIFIER_NAMESPACE: &str = "offline_note_v2";
    const OFFLINE_NOTE_V2_REPLAY_ISSUE_DOMAIN: &str = "offline-note-v2-issued-note-v1";
    const OFFLINE_NOTE_V2_REPLAY_KEY_CERTIFICATE_DOMAIN: &str =
        "offline-note-v2-issued-key-certificate-v1";
    const OFFLINE_NOTE_V2_REPLAY_ISSUED_CLAIM_DOMAIN: &str = "offline-note-v2-issued-claim-v1";
    const OFFLINE_NOTE_V2_REPLAY_SPENT_CLAIM_DOMAIN: &str = "offline-note-v2-spent-claim-v1";
    const OFFLINE_NOTE_V2_REPLAY_NULLIFIER_DOMAIN: &str = "offline-note-v2-spent-nullifier-v1";
    const OFFLINE_NOTE_V2_REPLAY_AUDIT_TOKEN_DOMAIN: &str = "offline-note-v2-audit-token-v1";
    const OFFLINE_NOTE_V2_REPLAY_AUDIT_RECORD_DOMAIN: &str = "offline-note-v2-audit-record-v1";
    const OFFLINE_NOTE_V2_REPLAY_AUDIT_NULLIFIER_DOMAIN: &str =
        "offline-note-v2-audit-nullifier-v1";
    const OFFLINE_NOTE_V2_REPLAY_AUDIT_OUTPUT_DOMAIN: &str = "offline-note-v2-audit-output-v1";

    fn validate_offline_note_v2_key_certificate(
        certificate: &OfflineNoteKeyCertificateV2,
    ) -> Result<(), InstructionExecutionError> {
        if certificate.version != 2 || !certificate.one_use {
            return Err(InstructionExecutionError::InvariantViolation(
                "offline V2 note operation requires a compact one-use key certificate".into(),
            ));
        }
        if certificate.public_key.is_empty() {
            return Err(InstructionExecutionError::InvariantViolation(
                "offline V2 note certificate public key must be non-empty".into(),
            ));
        }
        PublicKey::from_bytes(Algorithm::Ed25519, &certificate.public_key).map_err(|_| {
            InstructionExecutionError::InvariantViolation(
                "offline V2 note certificate public key must be an Ed25519 public key".into(),
            )
        })?;
        Ok(())
    }

    fn offline_note_v2_expected_public_instances(public_inputs_hash: &Hash) -> Vec<Vec<[u8; 32]>> {
        fn limb_columns(hash: &Hash) -> impl Iterator<Item = Vec<[u8; 32]>> + '_ {
            let bytes: &[u8; Hash::LENGTH] = hash.as_ref();
            (0..4).map(move |index| {
                let mut scalar = [0u8; 32];
                let start = index * 8;
                scalar[..8].copy_from_slice(&bytes[start..start + 8]);
                vec![scalar]
            })
        }

        let reserved_sentinel = Hash::prehashed([0u8; Hash::LENGTH]);
        let mut columns = Vec::with_capacity(16);
        columns.extend(limb_columns(public_inputs_hash));
        for _ in 0..3 {
            columns.extend(limb_columns(&reserved_sentinel));
        }
        columns
    }

    fn offline_note_v2_public_instances_from_envelope(
        proof: &ProofBox,
        envelope: &OpenVerifyEnvelope,
    ) -> Result<Vec<Vec<[u8; 32]>>, Error> {
        match envelope.backend {
            BackendTag::Halo2IpaPasta => crate::zk::extract_pasta_instance_columns_bytes(
                &envelope.proof_bytes,
            )
            .ok_or_else(|| {
                InstructionExecutionError::InvariantViolation(
                    "offline V2 recursive proof does not expose Halo2 public instances".into(),
                )
                .into()
            }),
            BackendTag::Stark => {
                if !crate::zk::is_stark_fri_v1_backend(proof.backend.as_str()) {
                    return Err(InstructionExecutionError::InvariantViolation(
                        "offline V2 recursive proof Stark backend is unsupported".into(),
                    )
                    .into());
                }
                let open: StarkFriOpenProofV1 = norito::decode_from_bytes(&envelope.proof_bytes)
                    .map_err(|_| {
                        InstructionExecutionError::InvariantViolation(
                            "offline V2 recursive proof has invalid STARK public inputs".into(),
                        )
                    })?;
                Ok(open.public_inputs)
            }
            _ => Err(InstructionExecutionError::InvariantViolation(
                "offline V2 recursive proof backend is unsupported".into(),
            )
            .into()),
        }
    }

    fn offline_note_v2_resolve_verifier(
        proof: &OfflineNoteRecursiveProofV2,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<(VerifyingKeyRecord, VerifyingKeyBox, OpenVerifyEnvelope), Error> {
        let verifier_id: &VerifyingKeyId = &proof.verifier_key_id;
        if proof.proof.backend != verifier_id.backend {
            return Err(InstructionExecutionError::InvariantViolation(
                "offline V2 recursive proof backend does not match verifier key id".into(),
            )
            .into());
        }
        let backend = verifier_id.backend.as_str();
        if backend.starts_with("debug/") {
            return Err(InstructionExecutionError::InvariantViolation(
                "offline V2 recursive proofs may not use debug proof backends".into(),
            )
            .into());
        }
        if proof.proof.bytes.is_empty() {
            return Err(InstructionExecutionError::InvariantViolation(
                "offline V2 recursive proof must not be empty".into(),
            )
            .into());
        }

        let record = state_transaction
            .world
            .verifying_keys
            .get(verifier_id)
            .cloned()
            .ok_or_else(|| {
                InstructionExecutionError::InvariantViolation(
                    "offline V2 recursive verifier key is not registered".into(),
                )
            })?;
        if record.status != ConfidentialStatus::Active {
            return Err(InstructionExecutionError::InvariantViolation(
                "offline V2 recursive verifier key is not active".into(),
            )
            .into());
        }
        if record.namespace != OFFLINE_NOTE_V2_VERIFIER_NAMESPACE {
            return Err(InstructionExecutionError::InvariantViolation(
                "offline V2 recursive verifier key is not in the Offline V2 namespace".into(),
            )
            .into());
        }
        if record.public_inputs_schema_hash != offline_note_v2_recursive_public_inputs_schema_hash()
        {
            return Err(InstructionExecutionError::InvariantViolation(
                "offline V2 recursive verifier key uses an unexpected public-input schema".into(),
            )
            .into());
        }
        if record.max_proof_bytes == 0 {
            return Err(InstructionExecutionError::InvariantViolation(
                "offline V2 recursive verifier key must set max_proof_bytes".into(),
            )
            .into());
        }
        if proof.proof.bytes.len() > record.max_proof_bytes as usize {
            return Err(InstructionExecutionError::InvariantViolation(
                "offline V2 recursive proof exceeds verifier max_proof_bytes".into(),
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
                return Err(InstructionExecutionError::InvariantViolation(
                    "offline V2 recursive verifier circuit/version is not active".into(),
                )
                .into());
            }
        }

        let vk_box = record.key.clone().ok_or_else(|| {
            InstructionExecutionError::InvariantViolation(
                "offline V2 recursive verifier key bytes are not available inline".into(),
            )
        })?;
        if vk_box.backend != verifier_id.backend || vk_box.backend != proof.proof.backend {
            return Err(InstructionExecutionError::InvariantViolation(
                "offline V2 recursive verifier backend mismatch".into(),
            )
            .into());
        }
        if crate::zk::hash_vk(&vk_box) != record.commitment {
            return Err(InstructionExecutionError::InvariantViolation(
                "offline V2 recursive verifier commitment mismatch".into(),
            )
            .into());
        }

        let envelope: OpenVerifyEnvelope =
            norito::decode_from_bytes(&proof.proof.bytes).map_err(|_| {
                InstructionExecutionError::InvariantViolation(
                    "offline V2 recursive proof must be an OpenVerifyEnvelope".into(),
                )
            })?;
        if envelope.backend != record.backend {
            return Err(InstructionExecutionError::InvariantViolation(
                "offline V2 recursive proof envelope backend mismatch".into(),
            )
            .into());
        }
        if envelope.circuit_id != record.circuit_id {
            return Err(InstructionExecutionError::InvariantViolation(
                "offline V2 recursive proof circuit id mismatch".into(),
            )
            .into());
        }
        if envelope.vk_hash != [0u8; 32] && envelope.vk_hash != record.commitment {
            return Err(InstructionExecutionError::InvariantViolation(
                "offline V2 recursive proof verifier commitment mismatch".into(),
            )
            .into());
        }
        if envelope.public_inputs != OFFLINE_NOTE_V2_RECURSIVE_PUBLIC_INPUTS_SCHEMA_V1 {
            return Err(InstructionExecutionError::InvariantViolation(
                "offline V2 recursive proof public-input schema mismatch".into(),
            )
            .into());
        }

        Ok((record, vk_box, envelope))
    }

    fn verify_offline_note_v2_recursive_proof(
        proof: &OfflineNoteRecursiveProofV2,
        expected_public_inputs_hash: &Hash,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        if &proof.public_inputs_hash != expected_public_inputs_hash {
            return Err(InstructionExecutionError::InvariantViolation(
                "offline V2 recursive proof is not bound to expected public inputs".into(),
            )
            .into());
        }

        let (_record, vk_box, envelope) =
            offline_note_v2_resolve_verifier(proof, state_transaction)?;
        let actual_instances =
            offline_note_v2_public_instances_from_envelope(&proof.proof, &envelope)?;
        let expected_instances =
            offline_note_v2_expected_public_instances(expected_public_inputs_hash);
        if actual_instances != expected_instances {
            return Err(InstructionExecutionError::InvariantViolation(
                "offline V2 recursive proof public instances do not match expected public inputs"
                    .into(),
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
        let timeout_budget = state_transaction.zk.verify_timeout;
        if timeout_budget > Duration::ZERO && report.elapsed > timeout_budget {
            return Err(InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(
                    "offline V2 recursive proof verification exceeded timeout".into(),
                ),
            )
            .into());
        }
        if !report.ok {
            return Err(InstructionExecutionError::InvariantViolation(
                "offline V2 recursive proof verification failed".into(),
            )
            .into());
        }
        Ok(())
    }

    fn offline_note_v2_replay_key(domain: &str, value: &Hash) -> Hash {
        let mut preimage = Vec::with_capacity(domain.len() + Hash::LENGTH + 1);
        preimage.extend_from_slice(domain.as_bytes());
        preimage.push(b':');
        preimage.extend_from_slice(value.as_ref());
        Hash::new(&preimage)
    }

    fn offline_note_v2_issue_key(note_commitment: &Hash) -> Hash {
        offline_note_v2_replay_key(OFFLINE_NOTE_V2_REPLAY_ISSUE_DOMAIN, note_commitment)
    }

    fn offline_note_v2_key_certificate_key(certificate_payload_hash: &Hash) -> Hash {
        offline_note_v2_replay_key(
            OFFLINE_NOTE_V2_REPLAY_KEY_CERTIFICATE_DOMAIN,
            certificate_payload_hash,
        )
    }

    fn offline_note_v2_issued_claim_key(claim_hash: &Hash) -> Hash {
        offline_note_v2_replay_key(OFFLINE_NOTE_V2_REPLAY_ISSUED_CLAIM_DOMAIN, claim_hash)
    }

    fn offline_note_v2_spent_claim_key(claim_hash: &Hash) -> Hash {
        offline_note_v2_replay_key(OFFLINE_NOTE_V2_REPLAY_SPENT_CLAIM_DOMAIN, claim_hash)
    }

    fn offline_note_v2_nullifier_key(nullifier: &Hash) -> Hash {
        offline_note_v2_replay_key(OFFLINE_NOTE_V2_REPLAY_NULLIFIER_DOMAIN, nullifier)
    }

    fn offline_note_v2_audit_token_key(token_id: &Hash) -> Hash {
        offline_note_v2_replay_key(OFFLINE_NOTE_V2_REPLAY_AUDIT_TOKEN_DOMAIN, token_id)
    }

    fn offline_note_v2_audit_record_key(public_inputs_hash: &Hash) -> Hash {
        offline_note_v2_replay_key(
            OFFLINE_NOTE_V2_REPLAY_AUDIT_RECORD_DOMAIN,
            public_inputs_hash,
        )
    }

    fn offline_note_v2_audit_nullifier_key(nullifier: &Hash) -> Hash {
        offline_note_v2_replay_key(OFFLINE_NOTE_V2_REPLAY_AUDIT_NULLIFIER_DOMAIN, nullifier)
    }

    fn offline_note_v2_audit_output_key(output_commitment: &Hash) -> Hash {
        offline_note_v2_replay_key(
            OFFLINE_NOTE_V2_REPLAY_AUDIT_OUTPUT_DOMAIN,
            output_commitment,
        )
    }

    fn ensure_unique_hashes(
        hashes: &[Hash],
        message: &'static str,
    ) -> Result<(), InstructionExecutionError> {
        let mut seen = BTreeSet::new();
        for hash in hashes {
            if !seen.insert(*hash) {
                return Err(InstructionExecutionError::InvariantViolation(
                    message.into(),
                ));
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
                "only the note account or an offline escrow manager may submit Offline V2 notes",
            )
            .into())
        }
    }

    fn ensure_can_issue_offline_note_v2(
        authority: &AccountId,
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        if is_offline_escrow_manager(authority, state_transaction) {
            Ok(())
        } else {
            Err(labeled_invariant(
                "unauthorized_controller",
                "only an offline escrow manager may issue Offline V2 notes",
            )
            .into())
        }
    }

    fn ensure_offline_note_v2_certificate_signature(
        certificate: &OfflineNoteKeyCertificateV2,
        issuer: &AccountId,
    ) -> Result<(), Error> {
        let payload = certificate.signing_bytes().map_err(|err| {
            InstructionExecutionError::InvariantViolation(
                format!("failed to encode Offline V2 key certificate payload: {err}").into(),
            )
        })?;
        let issuer_key = issuer.try_signatory().ok_or_else(|| {
            InstructionExecutionError::InvariantViolation(
                "offline V2 note issuer account must be single-signature".into(),
            )
        })?;
        certificate
            .issuer_signature
            .verify(issuer_key, &payload)
            .map_err(|_| {
                InstructionExecutionError::InvariantViolation(
                    "offline V2 key certificate signature does not match issuer account".into(),
                )
                .into()
            })
    }

    fn offline_note_v2_issued_claim_hash(claim: OfflineNoteIssuedClaimV2) -> Result<Hash, Error> {
        claim.claim_hash().map_err(|err| {
            InstructionExecutionError::InvariantViolation(
                format!("failed to encode Offline V2 issued-note claim: {err}").into(),
            )
            .into()
        })
    }

    impl Execute for IssueOfflineNoteV2 {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let issue = self.issue;
            validate_offline_note_v2_key_certificate(&issue.key_certificate)?;
            if issue.amount <= Numeric::zero() {
                return Err(InstructionExecutionError::InvariantViolation(
                    "offline V2 note issue amount must be positive".into(),
                )
                .into());
            }
            if issue.key_certificate.account_id != *issue.asset.account() {
                return Err(InstructionExecutionError::InvariantViolation(
                    "offline V2 note issue certificate account must match the debited asset owner"
                        .into(),
                )
                .into());
            }
            ensure_can_issue_offline_note_v2(authority, state_transaction)?;
            ensure_offline_note_v2_certificate_signature(&issue.key_certificate, authority)?;
            let spec = state_transaction.numeric_spec_for(issue.asset.definition())?;
            assert_numeric_spec_with(&issue.amount, spec)?;
            let certificate_payload_hash = issue.key_certificate.payload_hash().map_err(|err| {
                InstructionExecutionError::InvariantViolation(
                    format!("failed to encode Offline V2 key certificate payload: {err}").into(),
                )
            })?;
            let issue_key = offline_note_v2_issue_key(&issue.note_commitment);
            let certificate_key = offline_note_v2_key_certificate_key(&certificate_payload_hash);
            let issued_claim_hash = offline_note_v2_issued_claim_hash(
                OfflineNoteIssuedClaimV2::from_issue(&issue).map_err(|err| {
                    InstructionExecutionError::InvariantViolation(
                        format!("failed to encode Offline V2 issued-note claim: {err}").into(),
                    )
                })?,
            )?;
            let issued_claim_key = offline_note_v2_issued_claim_key(&issued_claim_hash);
            if state_transaction
                .world
                .offline_note_v2_replay_keys
                .get(&issue_key)
                .is_some()
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    "offline V2 note commitment is already issued".into(),
                )
                .into());
            }
            if state_transaction
                .world
                .offline_note_v2_replay_keys
                .get(&certificate_key)
                .is_some()
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    "offline V2 key certificate is already issued".into(),
                )
                .into());
            }
            reserve_offline_note_escrow(state_transaction, &issue.asset, &issue.amount)?;
            state_transaction
                .world
                .offline_note_v2_replay_keys
                .insert(issue_key, ());
            state_transaction
                .world
                .offline_note_v2_replay_keys
                .insert(certificate_key, ());
            state_transaction
                .world
                .offline_note_v2_replay_keys
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

    impl Execute for RedeemOfflineNoteV2 {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let redemption = self.redemption;
            if redemption.input_nullifiers.is_empty() || redemption.input_nullifiers.len() > 4 {
                return Err(InstructionExecutionError::InvariantViolation(
                    "offline V2 redemption requires 1 to 4 input nullifiers".into(),
                )
                .into());
            }
            if redemption.amount <= Numeric::zero() {
                return Err(InstructionExecutionError::InvariantViolation(
                    "offline V2 redemption amount must be positive".into(),
                )
                .into());
            }
            if redemption.asset.account() != &redemption.recipient {
                return Err(InstructionExecutionError::InvariantViolation(
                    "offline V2 redemption asset owner must match recipient".into(),
                )
                .into());
            }
            ensure_unique_hashes(
                &redemption.input_nullifiers,
                "offline V2 redemption input nullifiers must be unique",
            )?;
            validate_offline_note_v2_key_certificate(&redemption.sender_key_certificate)?;
            ensure_can_submit_offline_note_for_account(
                &redemption.recipient,
                authority,
                state_transaction,
            )?;
            let spec = state_transaction.numeric_spec_for(redemption.asset.definition())?;
            assert_numeric_spec_with(&redemption.amount, spec)?;
            let expected_public_inputs_hash = redemption.public_inputs_hash().map_err(|err| {
                InstructionExecutionError::InvariantViolation(
                    format!("failed to encode Offline V2 redemption public inputs: {err}").into(),
                )
            })?;
            if redemption.recursive_proof.public_inputs_hash != expected_public_inputs_hash {
                return Err(InstructionExecutionError::InvariantViolation(
                    "offline V2 recursive proof is not bound to redemption public inputs".into(),
                )
                .into());
            }
            let issued_claim_hash = offline_note_v2_issued_claim_hash(
                OfflineNoteIssuedClaimV2::from_redemption(&redemption).map_err(|err| {
                    InstructionExecutionError::InvariantViolation(
                        format!("failed to encode Offline V2 issued-note claim: {err}").into(),
                    )
                })?,
            )?;
            let issued_claim_key = offline_note_v2_issued_claim_key(&issued_claim_hash);
            let spent_claim_key = offline_note_v2_spent_claim_key(&issued_claim_hash);
            if state_transaction
                .world
                .offline_note_v2_replay_keys
                .get(&issued_claim_key)
                .is_none()
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    "offline V2 note was not issued for this source commitment, recipient, asset, and amount".into(),
                )
                .into());
            }
            if state_transaction
                .world
                .offline_note_v2_replay_keys
                .get(&spent_claim_key)
                .is_some()
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    "offline V2 issued note is already redeemed".into(),
                )
                .into());
            }
            let consumed_keys = redemption
                .input_nullifiers
                .iter()
                .map(offline_note_v2_nullifier_key)
                .collect::<Vec<_>>();
            for consumed_key in &consumed_keys {
                if state_transaction
                    .world
                    .offline_note_v2_replay_keys
                    .get(consumed_key)
                    .is_some()
                {
                    return Err(InstructionExecutionError::InvariantViolation(
                        "offline V2 nullifier is already redeemed".into(),
                    )
                    .into());
                }
            }
            verify_offline_note_v2_recursive_proof(
                &redemption.recursive_proof,
                &expected_public_inputs_hash,
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
                    .offline_note_v2_replay_keys
                    .insert(consumed_key, ());
            }
            state_transaction
                .world
                .offline_note_v2_replay_keys
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

    impl Execute for AuditOfflineNoteV2 {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let audit = self.audit;
            if audit.input_nullifiers.is_empty() || audit.input_nullifiers.len() > 4 {
                return Err(InstructionExecutionError::InvariantViolation(
                    "offline V2 audit requires 1 to 4 input nullifiers".into(),
                )
                .into());
            }
            if audit.output_commitments.is_empty() || audit.output_commitments.len() > 2 {
                return Err(InstructionExecutionError::InvariantViolation(
                    "offline V2 audit requires 1 to 2 output commitments".into(),
                )
                .into());
            }
            ensure_unique_hashes(
                &audit.input_nullifiers,
                "offline V2 audit input nullifiers must be unique",
            )?;
            ensure_unique_hashes(
                &audit.output_commitments,
                "offline V2 audit output commitments must be unique",
            )?;
            validate_offline_note_v2_key_certificate(&audit.sender_key_certificate)?;
            ensure_can_submit_offline_note_for_account(
                &audit.sender_key_certificate.account_id,
                authority,
                state_transaction,
            )?;
            let certificate_payload_hash =
                audit.sender_key_certificate.payload_hash().map_err(|err| {
                    InstructionExecutionError::InvariantViolation(
                        format!("failed to encode Offline V2 key certificate payload: {err}")
                            .into(),
                    )
                })?;
            let certificate_key = offline_note_v2_key_certificate_key(&certificate_payload_hash);
            if state_transaction
                .world
                .offline_note_v2_replay_keys
                .get(&certificate_key)
                .is_none()
            {
                return Err(InstructionExecutionError::InvariantViolation(
                    "offline V2 audit key certificate was not issued".into(),
                )
                .into());
            }
            let expected_public_inputs_hash = audit.public_inputs_hash().map_err(|err| {
                InstructionExecutionError::InvariantViolation(
                    format!("failed to encode Offline V2 audit public inputs: {err}").into(),
                )
            })?;
            if audit.recursive_proof.public_inputs_hash != expected_public_inputs_hash {
                return Err(InstructionExecutionError::InvariantViolation(
                    "offline V2 recursive proof is not bound to audit public inputs".into(),
                )
                .into());
            }
            let audit_token_key = offline_note_v2_audit_token_key(&audit.token_id);
            let audit_record_key = offline_note_v2_audit_record_key(&expected_public_inputs_hash);
            if state_transaction
                .world
                .offline_note_v2_replay_keys
                .get(&audit_token_key)
                .is_some()
            {
                if state_transaction
                    .world
                    .offline_note_v2_replay_keys
                    .get(&audit_record_key)
                    .is_some()
                {
                    return Ok(());
                }
                return Err(InstructionExecutionError::InvariantViolation(
                    "offline V2 audit token already records different public inputs".into(),
                )
                .into());
            }
            let observed_nullifier_keys = audit
                .input_nullifiers
                .iter()
                .map(offline_note_v2_audit_nullifier_key)
                .collect::<Vec<_>>();
            let observed_output_keys = audit
                .output_commitments
                .iter()
                .map(offline_note_v2_audit_output_key)
                .collect::<Vec<_>>();
            for observed_key in &observed_nullifier_keys {
                if state_transaction
                    .world
                    .offline_note_v2_replay_keys
                    .get(observed_key)
                    .is_some()
                {
                    return Err(InstructionExecutionError::InvariantViolation(
                        "offline V2 audit observed a duplicate nullifier".into(),
                    )
                    .into());
                }
            }
            for observed_key in &observed_output_keys {
                if state_transaction
                    .world
                    .offline_note_v2_replay_keys
                    .get(observed_key)
                    .is_some()
                {
                    return Err(InstructionExecutionError::InvariantViolation(
                        "offline V2 audit observed a duplicate output commitment".into(),
                    )
                    .into());
                }
            }
            verify_offline_note_v2_recursive_proof(
                &audit.recursive_proof,
                &expected_public_inputs_hash,
                state_transaction,
            )?;
            state_transaction
                .world
                .offline_note_v2_replay_keys
                .insert(audit_token_key, ());
            state_transaction
                .world
                .offline_note_v2_replay_keys
                .insert(audit_record_key, ());
            for observed_key in observed_nullifier_keys {
                state_transaction
                    .world
                    .offline_note_v2_replay_keys
                    .insert(observed_key, ());
            }
            for observed_key in observed_output_keys {
                state_transaction
                    .world
                    .offline_note_v2_replay_keys
                    .insert(observed_key, ());
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
        use super::*;
        use iroha_crypto::{KeyPair, Signature};

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

        fn sample_certificate() -> OfflineNoteKeyCertificateV2 {
            let keypair = KeyPair::from_seed(vec![0xAA; 32], Algorithm::Ed25519);
            let (_algorithm, public_key) = keypair.public_key().to_bytes();
            OfflineNoteKeyCertificateV2 {
                version: 2,
                platform: "ios-appattest".to_owned(),
                key_id: "one-use-key".to_owned(),
                device_id: "device-1".to_owned(),
                account_id: sample_account(0x01),
                public_key: public_key.to_vec(),
                one_use: true,
                issuer_signature: sample_signature(0x44),
            }
        }

        #[test]
        fn expected_public_instances_encode_hash_limbs_and_reserved_sentinel() {
            let hash = Hash::prehashed([0x11; Hash::LENGTH]);
            let instances = offline_note_v2_expected_public_instances(&hash);

            assert_eq!(instances.len(), 16);
            for index in 0..4 {
                let mut expected = [0u8; 32];
                expected[..8].copy_from_slice(&hash.as_ref()[index * 8..index * 8 + 8]);
                assert_eq!(instances[index], vec![expected]);
            }
            let reserved =
                offline_note_v2_expected_public_instances(&Hash::prehashed([0u8; Hash::LENGTH]));
            assert_eq!(&instances[4..], &reserved[..12]);
        }

        #[test]
        fn key_certificate_requires_v2_one_use_ed25519_key() {
            let mut certificate = sample_certificate();
            assert!(validate_offline_note_v2_key_certificate(&certificate).is_ok());

            certificate.one_use = false;
            assert!(validate_offline_note_v2_key_certificate(&certificate).is_err());

            certificate.one_use = true;
            certificate.public_key.clear();
            assert!(validate_offline_note_v2_key_certificate(&certificate).is_err());
        }

        #[test]
        fn duplicate_hashes_are_rejected() {
            let hash = Hash::new(b"duplicate");
            let err = ensure_unique_hashes(&[hash, hash], "duplicate hashes")
                .expect_err("duplicate hash should fail");
            assert!(matches!(
                err,
                InstructionExecutionError::InvariantViolation(_)
            ));
        }
    }
}
