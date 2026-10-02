//! Ordinary incoming mathematical preview from real proof-admitted source operands.
//! This returns data for a Native-owned intent, not source/funding/approval or State authority.
//! Full finalized source/current FI/global DATA reservation and fresh W2/W1 remain mandatory.
use super::*;
use crate::kagemusha_v1_recursion::{
    KagemushaAuthenticatedRecursiveVerifierV1, KagemushaVerifiedOrdinaryReceivedCashOutputV1,
    ordinary_incoming_artifacts_v1, require_ordinary_incoming_predecessor_v1,
    verify_kagemusha_mint_finality_helper_v1,
};
use iroha_data_model::kagemusha::{
    KagemushaOrdinaryCashClockContextV1, KagemushaOrdinaryIncomingPreparationV1,
    KagemushaOrdinaryIncomingReservationV1, KagemushaOrdinaryIncomingSourceSelectionV1,
    KagemushaOrdinaryTopUpRequestV1, KagemushaVerifiedOrdinaryAppCredentialV1,
    kagemusha_ordinary_app_account_binding_v1, kagemusha_ordinary_financial_epoch_id_v1,
};

/// Genuine source proof operands, never an OEM credential or a decoded Native finality grant.
/// Mint source finality is independently admitted by its actual Node/Core owner; its actual
/// MintAuthority recursive credit additionally proves the same finalized statement. Receive
/// carries the already-admitted sender Wrapper and immutable installed DATA assertion.
pub(crate) enum OrdinaryIncomingMathSourceV1<'loan, 'owner> {
    /// Exact closed pre-debit authorization and independently genuine neutral MintAuthority proof.
    Mint {
        /// Same complete pre-debit request admitted under actual ordinary113 release material.
        source: &'loan KagemushaAuthenticatedOrdinaryFinalizedMintSourceV1<'owner>,
        /// Actual full finalized credit, with both genuine MintAuthority proofs and histories.
        credit: &'loan KagemushaMintCreditV1,
    },
    /// Closed actual sender proof/output admission; its original request key remains in Main.
    Receive(&'loan KagemushaVerifiedOrdinaryReceivedCashOutputV1),
}

/// Internal preview data for the actual Native Main WAL. None of these fields authorizes a
/// platform invocation, funds an account, consumes a credit or exposes the successor State.
pub(crate) struct OrdinaryIncomingPreviewV1 {
    /// Exact private successor with mathematical balance, index and replay-root updates.
    pub(crate) successor: KagemushaStateV1,
    /// Complete State transition statement consumed by both ordinary State parities.
    pub(crate) statement: TransitionProofStatementV1,
    /// Complete acyclic fresh incoming FI/clock/nonce preparation before W2.
    pub(crate) preparation: KagemushaOrdinaryIncomingPreparationV1,
    /// Exact normalized purpose2 Guard statement, with no terminal/outbox effect.
    pub(crate) normalized: KagemushaNormalizedGuardStatementV1,
    /// Same full normalized context retained by Native for reconstruction/recovery.
    pub(crate) guard_context: KagemushaGuardContextV1,
    /// Exact local transport semantic digest of the same financial edge.
    pub(crate) transport_semantic_digest: DigestV1,
}

struct SourceFacts {
    kind: KagemushaTransitionKindV1,
    credit_id: DigestV1,
    amount: u128,
    semantic_digest: DigestV1,
    mint_proof_binding: DigestV1,
    lifecycle_binding: DigestV1,
}

fn source_facts(
    verifier: &KagemushaAuthenticatedRecursiveVerifierV1,
    artifacts: KagemushaRecursionArtifactsV1,
    reservation: &KagemushaOrdinaryIncomingReservationV1,
    source: OrdinaryIncomingMathSourceV1<'_, '_>,
) -> Result<SourceFacts, KagemushaStateErrorV1> {
    let selection = &reservation.selection;
    let facts = match source {
        OrdinaryIncomingMathSourceV1::Mint { source, credit } => {
            source.recheck_retained_custody().map_err(material)?;
            let authorization = source.authorization().map_err(material)?;
            let request = KagemushaOrdinaryTopUpRequestV1::decode_canonical_exact(
                authorization.request_original(),
            )
            .map_err(material)?;
            selection
                .validate_against_topup(&request)
                .map_err(material)?;
            let expected = source.credit_statement().map_err(material)?;
            if &credit.statement != expected
                || reservation.finalized_source_original_sha256
                    != <DigestV1>::from(Sha256::digest(
                        source.finalized_original().map_err(material)?,
                    ))
                || reservation.source_semantic_digest
                    != source.source_semantic_digest().map_err(material)?
                || reservation.source_proof_original_sha256
                    != <DigestV1>::from(Sha256::digest(
                        norito::encode_canonical(credit).map_err(material)?,
                    ))
                || credit.encrypted_credit != request.encrypted_credit
                || request.authorization.binding_digest().map_err(material)?
                    != authorization.authorization_original_digest()
            {
                return Err(KagemushaStateErrorV1::InvalidMintCredit);
            }
            // No caller callback/boolean substitutes for either current proof or complete history.
            let admitted = verify_kagemusha_mint_finality_helper_v1(verifier, artifacts, credit)
                .map_err(material)?;
            source.recheck_retained_custody().map_err(material)?;
            SourceFacts {
                kind: KagemushaTransitionKindV1::MintFold,
                credit_id: credit.statement.lifecycle.credit_id,
                amount: credit.statement.amount,
                semantic_digest: admitted.semantic_digest(),
                mint_proof_binding: admitted.proof_binding_digest(),
                lifecycle_binding: credit
                    .statement
                    .lifecycle
                    .canonical_digest()
                    .map_err(material)?,
            }
        }
        OrdinaryIncomingMathSourceV1::Receive(received) => {
            let KagemushaOrdinaryIncomingSourceSelectionV1::Receive {
                sender_commit_transport_original_sha256,
                sender_outgoing_original_sha256,
                recipient_request_original_digest,
                encrypted_credit_original_sha256,
            } = selection.source
            else {
                return Err(KagemushaStateErrorV1::InvalidPeerCredit);
            };
            if sender_commit_transport_original_sha256
                != received.received_assertion_original_sha256()
                || sender_outgoing_original_sha256
                    != <DigestV1>::from(Sha256::digest(received.outgoing_original()))
                || reservation.source_proof_original_sha256 != sender_outgoing_original_sha256
                || recipient_request_original_digest != received.output().request_digest
                || encrypted_credit_original_sha256
                    != <DigestV1>::from(Sha256::digest(received.encrypted_credit()))
                || selection.recipient_app_credential_digest
                    != received.receiver_credential_digest()
            {
                return Err(KagemushaStateErrorV1::InvalidPeerCredit);
            }
            let body = &received.request().body;
            let owner = &selection.lineage.owner;
            if &body.network_id != owner.runtime.network_id.as_bytes()
                || body.normalized_asset_id
                    != kagemusha_asset_identity_digest_v1(&owner.runtime.asset).map_err(material)?
                || body.asset_incarnation != *owner.runtime.asset_incarnation.as_bytes()
                || body.scale != owner.runtime.scale
                || body.recipient_lane_id != owner.lane_id
            {
                return Err(KagemushaStateErrorV1::InvalidPeerCredit);
            }
            SourceFacts {
                kind: KagemushaTransitionKindV1::ReceiveFold,
                credit_id: received.credit_id(),
                amount: received.amount(),
                semantic_digest: received.output().binding_digest().map_err(material)?,
                mint_proof_binding: [0; 32],
                lifecycle_binding: [0; 32],
            }
        }
    };
    if facts.amount == 0
        || facts.credit_id != selection.credit_id
        || facts.amount != selection.amount
        || facts.semantic_digest != reservation.source_semantic_digest
        || KagemushaOperationKindV1::from(
            crate::kagemusha_v1_recursion::KagemushaOperationV1::from(facts.kind),
        ) != selection.source.operation()
    {
        return Err(KagemushaStateErrorV1::StateInvariant);
    }
    Ok(facts)
}

/// Derive checked incoming data from the actual owned predecessor and genuine proof operands.
/// `fresh_context`/FI SHA/nonce are actual separately captured Native values supplied only by
/// the Main driver; passing these data to this helper cannot manufacture their owners. The
/// dedicated Mint approval is never converted to the fresh incoming W2 or later purpose1 W1.
/// # Errors
/// Refuses any source/scope/predecessor/proof/history/replay mismatch, arithmetic overflow,
/// foreign signed clock or substituted credential/financial identity.
#[allow(clippy::too_many_arguments)]
pub(crate) fn derive_ordinary_incoming_preview_v1(
    verifier: &KagemushaAuthenticatedRecursiveVerifierV1,
    predecessor: &KagemushaStateV1,
    predecessor_public_original: &[u8],
    credential: &KagemushaVerifiedOrdinaryAppCredentialV1,
    reservation: &KagemushaOrdinaryIncomingReservationV1,
    source: OrdinaryIncomingMathSourceV1<'_, '_>,
    replay: &ConsumedCreditInsertWitnessV1,
    successor_nonce_commitment: DigestV1,
    journal_revision: u64,
    fresh_financial_control_original_sha256: DigestV1,
    fresh_context: &KagemushaOrdinaryCashClockContextV1,
    fresh_signed_clock: &KagemushaVerifiedOrdinaryNativeSignedClockOriginalV1,
    fresh_approval_nonce: DigestV1,
) -> Result<OrdinaryIncomingPreviewV1, KagemushaStateErrorV1> {
    predecessor.validate()?;
    reservation.validate_shape().map_err(material)?;
    fresh_signed_clock
        .recheck_cash_context(fresh_context)
        .map_err(material)?;
    if fresh_context.lower_at_ms < credential.subject().issued_at_ms
        || fresh_context.upper_at_ms >= credential.subject().expires_at_ms
    {
        return Err(KagemushaStateErrorV1::InvalidTrustedCommitTime);
    }
    let (artifacts, provider_root) = ordinary_incoming_artifacts_v1(verifier).map_err(material)?;
    require_scope(
        predecessor,
        predecessor_public_original,
        credential,
        reservation,
        artifacts.release_id,
        provider_root,
    )?;
    let facts = source_facts(verifier, artifacts, reservation, source)?;
    let preview = derive_math_preview(
        predecessor,
        reservation,
        &facts,
        replay,
        successor_nonce_commitment,
        journal_revision,
        fresh_financial_control_original_sha256,
        fresh_context,
        fresh_approval_nonce,
        artifacts,
    )?;
    require_ordinary_incoming_predecessor_v1(
        verifier,
        predecessor_public_original,
        &preview.normalized,
    )
    .map_err(material)?;
    Ok(preview)
}

fn require_scope(
    state: &KagemushaStateV1,
    predecessor_public: &[u8],
    credential: &KagemushaVerifiedOrdinaryAppCredentialV1,
    reservation: &KagemushaOrdinaryIncomingReservationV1,
    release_id: DigestV1,
    provider_root: DigestV1,
) -> Result<(), KagemushaStateErrorV1> {
    let selection = &reservation.selection;
    let owner = &selection.lineage.owner;
    let c = credential.subject();
    let epoch = kagemusha_ordinary_financial_epoch_id_v1(c).map_err(material)?;
    if state.release_id != release_id
        || state.release_id != c.release_id
        || state.suite_id != c.suite_id
        || state.hardware_profile_id != c.hardware_profile_id
        || state.policy_epoch != c.policy_epoch
        || state.lane.network_id.as_bytes() != &c.network_id
        || state.lane.device_lane_id != c.lane_id
        || state.device_policy_binding.device_key_reference != c.app_key_reference
        || state.device_policy_binding.hardware_policy_id != provider_root
        || state.hardware_epoch.epoch_id != epoch
        || state.hardware_epoch.generation != u128::from(c.hardware_epoch)
        || state.next_one_use_key_reference != [0; 32]
        || state.lane.network_id != owner.runtime.network_id
        || state.lane.asset != owner.runtime.asset
        || state.lane.scale != owner.runtime.scale
        || state.asset_incarnation != owner.runtime.asset_incarnation
        || state.lane.device_lane_id != owner.lane_id
        || kagemusha_ordinary_app_account_binding_v1(&owner.account_id) != c.account_binding
        || selection.lineage.financial_epoch_id != epoch
        || selection.lineage.financial_authority_commitment != c.financial_authority_commitment
        || selection.recipient_app_credential_digest != credential.digest()
        || selection.predecessor.state_commitment != state.state_commitment
        || selection.predecessor.logical_sequence != state.logical_sequence
        || selection.predecessor.state_original_sha256
            != <DigestV1>::from(Sha256::digest(predecessor_public))
    {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn derive_math_preview(
    before: &KagemushaStateV1,
    reservation: &KagemushaOrdinaryIncomingReservationV1,
    facts: &SourceFacts,
    replay: &ConsumedCreditInsertWitnessV1,
    nonce: DigestV1,
    journal_revision: u64,
    fresh_fi: DigestV1,
    fresh_clock: &KagemushaOrdinaryCashClockContextV1,
    approval_nonce: DigestV1,
    artifacts: KagemushaRecursionArtifactsV1,
) -> Result<OrdinaryIncomingPreviewV1, KagemushaStateErrorV1> {
    let envelope = reservation.digest().map_err(material)?;
    // This tree belongs to the actual recipient financial State/lineage. Its identity key is
    // the exact credit ID, independent of op/head/FI/clock/envelope; changed retries cannot re-key it.
    replay.verify()?;
    if replay.credit_id != CreditIdV1(facts.credit_id)
        || replay.envelope_digest != envelope
        || replay.predecessor_root != before.consumed_credit_root
        || nonce == before.state_nonce_commitment
    {
        return Err(KagemushaStateErrorV1::InvalidConsumedCreditInsertWitness);
    }
    let balance = before
        .balance
        .checked_add(facts.amount)
        .ok_or(KagemushaStateErrorV1::ArithmeticOverflow)?;
    let sequence = before
        .logical_sequence
        .checked_add(1)
        .ok_or(KagemushaStateErrorV1::StateInvariant)?;
    let index = before
        .secure_index
        .checked_add(1)
        .ok_or(KagemushaStateErrorV1::StateInvariant)?;
    let journal_after = journal_revision
        .checked_add(1)
        .ok_or(KagemushaStateErrorV1::JournalRevisionOverflow)?;
    let successor = KagemushaStateV1::build(
        before.context(),
        before.liability_pool_id,
        before.lane.clone(),
        balance,
        sequence,
        index,
        before.hardware_epoch,
        before.device_policy_binding,
        nonce,
        replay.successor_root,
    )?;
    let lifecycle = if facts.lifecycle_binding != [0; 32] {
        facts.lifecycle_binding
    } else {
        incoming_receive_lifecycle_tuple_v1(&(
            facts.kind,
            before.protocol_version,
            before.suite_id,
            before.vk_digest,
            before.release_id,
            before.asset_incarnation,
            before.liability_pool_id,
            before.hardware_profile_id,
            before.policy_epoch,
            envelope,
        ))?
    };
    let statement = TransitionProofStatementV1 {
        version: 1,
        protocol_version: before.protocol_version,
        predecessor_suite_id: before.suite_id,
        predecessor_vk_digest: before.vk_digest,
        successor_suite_id: before.suite_id,
        successor_vk_digest: before.vk_digest,
        kind: facts.kind,
        amount: facts.amount,
        mint_finality_semantic_digest: if facts.kind == KagemushaTransitionKindV1::MintFold {
            facts.semantic_digest
        } else {
            [0; 32]
        },
        mint_finality_proof_binding_digest: facts.mint_proof_binding,
        peer_credit_id: [0; 32],
        recipient_encryption_key_binding: [0; 32],
        lifecycle_binding_digest: lifecycle,
        prepared_transition_binding_digest: [0; 32],
        receive_credit_binding_digest: if facts.kind == KagemushaTransitionKindV1::ReceiveFold {
            envelope
        } else {
            [0; 32]
        },
        predecessor_release_id: before.release_id,
        release_id: before.release_id,
        asset_incarnation: before.asset_incarnation,
        liability_pool_id: before.liability_pool_id,
        hardware_profile_id: before.hardware_profile_id,
        policy_epoch: before.policy_epoch,
        lane: before.lane.clone(),
        predecessor_commitment: before.state_commitment,
        successor_commitment: successor.state_commitment,
        predecessor_sequence: before.logical_sequence,
        successor_sequence: successor.logical_sequence,
        predecessor_epoch: before.hardware_epoch,
        successor_epoch: before.hardware_epoch,
        predecessor_device_policy_binding: before.device_policy_binding,
        successor_device_policy_binding: before.device_policy_binding,
        predecessor_state_nonce_commitment: before.state_nonce_commitment,
        successor_state_nonce_commitment: nonce,
        journal_revision_before: u128::from(journal_revision),
        journal_revision_after: u128::from(journal_after),
        effect_digest: envelope,
    };
    let preparation = KagemushaOrdinaryIncomingPreparationV1 {
        version: 1,
        reservation_digest: envelope,
        operation_id: reservation.selection.operation_id,
        nonce: approval_nonce,
        transition_statement_digest: transition_statement_digest(&statement)?,
        predecessor_state_commitment: before.state_commitment,
        successor_state_commitment: successor.state_commitment,
        financial_control_original_sha256: fresh_fi,
        clock_context_digest: fresh_clock.binding_digest().map_err(material)?,
        financial_index_before: before.secure_index,
        financial_index_after: index,
        logical_journal_sequence_before: journal_revision,
        logical_journal_sequence_after: journal_after,
    };
    preparation.validate_shape().map_err(material)?;
    let context = ordinary_incoming_guard_context_v1(
        artifacts,
        &statement,
        &preparation,
        fresh_clock.upper_at_ms,
    )?;
    let normalized =
        KagemushaNormalizedGuardStatementV1::derive_from_transition(&statement, context)
            .map_err(material)?;
    let normalized_digest = normalized.canonical_digest().map_err(material)?;
    let transport_semantic_digest = local_transition_transport_digest(
        facts.kind,
        before.release_id,
        before.liability_pool_id,
        envelope,
        before.state_commitment,
        successor.state_commitment,
        normalized_digest,
    )?;
    Ok(OrdinaryIncomingPreviewV1 {
        successor,
        statement,
        preparation,
        normalized,
        guard_context: context,
        transport_semantic_digest,
    })
}
/// Recreate the existing Receive lifecycle transcript from the same whole transition fields.
/// This pure data hash creates no source, financial, clock, replay or proof capability.
pub(crate) fn ordinary_incoming_receive_lifecycle_binding_v1(
    statement: &TransitionProofStatementV1,
    envelope: DigestV1,
) -> Result<DigestV1, KagemushaStateErrorV1> {
    incoming_receive_lifecycle_tuple_v1(&(
        statement.kind,
        statement.protocol_version,
        statement.predecessor_suite_id,
        statement.predecessor_vk_digest,
        statement.release_id,
        statement.asset_incarnation,
        statement.liability_pool_id,
        statement.hardware_profile_id,
        statement.policy_epoch,
        envelope,
    ))
}
type IncomingReceiveLifecycleTupleV1 = (
    KagemushaTransitionKindV1,
    u16,
    DigestV1,
    DigestV1,
    DigestV1,
    AxtAssetIncarnationV1,
    DigestV1,
    DigestV1,
    u64,
    DigestV1,
);
fn incoming_receive_lifecycle_tuple_v1(
    fields: &IncomingReceiveLifecycleTupleV1,
) -> Result<DigestV1, KagemushaStateErrorV1> {
    if fields.0 != KagemushaTransitionKindV1::ReceiveFold {
        return Err(KagemushaStateErrorV1::InvalidPeerCredit);
    }
    canonical_sha256_digest(TRANSITION_LIFECYCLE_DOMAIN, fields)
}
#[cfg(test)]
mod received_lifecycle_tests {
    use super::*;
    #[test]
    fn received_lifecycle_uses_same_whole_transition_and_exact_reservation_value() {
        // Pure known-public mathematical vectors, never a ledger incarnation/source owner.
        let incarnation =
            AxtAssetIncarnationV1::try_from_bytes(*iroha_crypto::Hash::prehashed([6; 32]).as_ref())
                .unwrap();
        let fields = (
            KagemushaTransitionKindV1::ReceiveFold,
            1,
            [2; 32],
            [3; 32],
            [4; 32],
            incarnation,
            [7; 32],
            [8; 32],
            9,
            [10; 32],
        );
        let expected = canonical_sha256_digest(TRANSITION_LIFECYCLE_DOMAIN, &fields).unwrap();
        assert_eq!(
            incoming_receive_lifecycle_tuple_v1(&fields).unwrap(),
            expected
        );
        let mut changed = fields;
        changed.9[17] ^= 1;
        assert_ne!(
            incoming_receive_lifecycle_tuple_v1(&changed).unwrap(),
            expected
        );
        let mut changed = fields;
        changed.3[17] ^= 1;
        assert_ne!(
            incoming_receive_lifecycle_tuple_v1(&changed).unwrap(),
            expected
        );
        let mut changed = fields;
        changed.0 = KagemushaTransitionKindV1::MintFold;
        assert!(incoming_receive_lifecycle_tuple_v1(&changed).is_err());
    }
}
fn material(error: impl core::fmt::Display) -> KagemushaStateErrorV1 {
    KagemushaStateErrorV1::ProofRejected(error.to_string())
}

#[cfg(test)]
#[path = "ordinary_incoming_preview_tests.rs"]
mod tests;

/// Data-only whole-State qualification preview. It invokes the exact production arithmetic,
/// replay insertion and normalized context derivation with plain mathematical operands.
/// There is no financial/clock/source/approval owner, accepting verifier or effect constructor.
#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn ordinary_incoming_preview_for_qualification_v1(
    before: &KagemushaStateV1,
    reservation: &KagemushaOrdinaryIncomingReservationV1,
    kind: KagemushaTransitionKindV1,
    mint_proof_binding: DigestV1,
    lifecycle_binding: DigestV1,
    replay: &ConsumedCreditInsertWitnessV1,
    nonce: DigestV1,
    journal_revision: u64,
    fresh_fi: DigestV1,
    fresh_clock: &KagemushaOrdinaryCashClockContextV1,
    approval_nonce: DigestV1,
    artifacts: KagemushaRecursionArtifactsV1,
) -> Result<OrdinaryIncomingPreviewV1, KagemushaStateErrorV1> {
    before.validate()?;
    reservation.validate_shape().map_err(material)?;
    if !matches!(
        kind,
        KagemushaTransitionKindV1::MintFold | KagemushaTransitionKindV1::ReceiveFold
    ) || reservation.selection.source.operation()
        != KagemushaOperationKindV1::from(
            crate::kagemusha_v1_recursion::KagemushaOperationV1::from(kind),
        )
        || (kind == KagemushaTransitionKindV1::MintFold
            && (mint_proof_binding == [0; 32] || lifecycle_binding == [0; 32]))
        || (kind == KagemushaTransitionKindV1::ReceiveFold
            && (mint_proof_binding != [0; 32] || lifecycle_binding != [0; 32]))
    {
        return Err(KagemushaStateErrorV1::StateInvariant);
    }
    let facts = SourceFacts {
        kind,
        credit_id: reservation.selection.credit_id,
        amount: reservation.selection.amount,
        semantic_digest: reservation.source_semantic_digest,
        mint_proof_binding,
        lifecycle_binding,
    };
    derive_math_preview(
        before,
        reservation,
        &facts,
        replay,
        nonce,
        journal_revision,
        fresh_fi,
        fresh_clock,
        approval_nonce,
        artifacts,
    )
}

/// Test-only mathematical replay index. It lends no financial, Native or DATA capability.
#[cfg(all(test, unix))]
pub(crate) struct OrdinaryConsumedCreditsForQualificationV1(
    super::sparse_merkle::ExactConsumedCreditIndex,
);
#[cfg(all(test, unix))]
impl OrdinaryConsumedCreditsForQualificationV1 {
    /// Empty mathematical history under the maintained paired replay tree.
    pub(crate) fn empty() -> Self {
        Self(super::sparse_merkle::ExactConsumedCreditIndex::empty())
    }
    /// Exact paired root, used only as a State mathematical operand.
    pub(crate) fn root(&self) -> iroha_data_model::kagemusha::KagemushaPastaStateCommitmentV1 {
        self.0.root()
    }
    /// Use the maintained exact nonmembership/insertion path, never a fabricated sibling array.
    pub(crate) fn preview(
        &self,
        credit: CreditIdV1,
        envelope: DigestV1,
    ) -> Result<ConsumedCreditInsertWitnessV1, KagemushaStateErrorV1> {
        self.0.preview_insert_witness(credit, envelope)
    }
    /// Install only that same verified mathematical path. This is not a wallet effect.
    pub(crate) fn install(
        &mut self,
        witness: &ConsumedCreditInsertWitnessV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.0
            .insert_with_witness(witness.credit_id, witness.envelope_digest, witness)
    }
}
