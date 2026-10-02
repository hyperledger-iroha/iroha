//! Complete incoming chronology quota sizing, never a source/approval/current grant.
//! The private deliberately unauthenticated plain-DATA specimen is measured by the existing
//! canonical uncompressed Record schema and drops here. It is never admitted/signed/persisted.
use super::*;
use iroha_data_model::kagemusha::{
    KAGEMUSHA_MINT_CREDIT_MAX_BYTES_V1, KAGEMUSHA_ORDINARY_FINALIZED_TOPUP_MAX_BYTES_V1,
    KAGEMUSHA_PLAY_INTEGRITY_REFRESH_LEASE_MAX_BYTES_V1, KagemushaHardwarePlatformClassV1,
    KagemushaHardwareTransitionSelectionV1, KagemushaOrdinaryIncomingSelectionV1,
    KagemushaOrdinaryIncomingSourceSelectionV1, KagemushaOrdinaryMintAuthorizationContextV1,
};

/// Count a whole sole record with checked conversions and the existing format bound.
fn frame_bytes(record: &Record, maximum_payload_bytes: u64) -> Result<u64, KagemushaStateErrorV1> {
    let bytes =
        u64::try_from(norito::canonical_frame_len(record).map_err(material)?).map_err(material)?;
    if bytes == 0 || bytes > maximum_payload_bytes {
        return Err(KagemushaStateErrorV1::InvalidDurableCapacity);
    }
    Ok(bytes)
}

/// Exact upper full Prepared record layout under the actual retained Native Mint scope.
/// All effect/clock/counter fields are fixed primitives/digests. Variable lineage/account/runtime
/// and lane/asset values are the actual same owner; the two source blobs and permitted PI/counter
/// option shapes use their existing full supported bounds. No Current check or capacity recursion.
/// Deliberately zero future selectors/invalid indexes/time prevent this DATA specimen from being
/// mistaken for a genuine incoming approval; it never leaves here except as a numeric byte count.
pub(in super::super) fn mint_incoming_prepared_byte_budget_v1(
    owner: &KagemushaNativeOrdinaryCashOwnerV1,
    context: &KagemushaOrdinaryMintAuthorizationContextV1,
) -> Result<u64, KagemushaStateErrorV1> {
    let before = &owner.state;
    if context.lineage != owner.initial_lineage_anchor.lineage
        || context.predecessor.state_commitment != before.state_commitment
        || context.predecessor.logical_sequence != before.logical_sequence
    {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    let (lease_original, previous_counter) = match owner
        .publication
        .cash_financial()
        .enrollment()
        .app_credential()
        .subject()
        .platform_class
    {
        KagemushaHardwarePlatformClassV1::AndroidKeyMint => (
            Some(vec![0; KAGEMUSHA_PLAY_INTEGRITY_REFRESH_LEASE_MAX_BYTES_V1]),
            None,
        ),
        KagemushaHardwarePlatformClassV1::AppleAppAttest => (None, Some(u32::MAX)),
        _ => return Err(KagemushaStateErrorV1::InvalidDurableCapacity),
    };
    let selection = KagemushaOrdinaryIncomingSelectionV1 {
        version: 1,
        lineage: context.lineage.clone(),
        operation_id: context.operation_id,
        predecessor: context.predecessor.clone(),
        source: KagemushaOrdinaryIncomingSourceSelectionV1::Mint {
            topup_request_original_sha256: [0; 32],
        },
        credit_id: [0; 32],
        amount: context.amount,
        scale: context.lineage.owner.runtime.scale,
        recipient_app_credential_digest: context.recipient_app_credential_digest,
        financial_control_original_sha256: context.financial_control_original_sha256,
        clock_context_digest: [0; 32],
    };
    let reservation = KagemushaOrdinaryIncomingReservationV1 {
        selection,
        finalized_source_original_sha256: [0; 32],
        source_proof_original_sha256: [0; 32],
        source_semantic_digest: [0; 32],
    };
    let statement = TransitionProofStatementV1 {
        version: 0,
        protocol_version: 0,
        predecessor_suite_id: [0; 32],
        predecessor_vk_digest: [0; 32],
        successor_suite_id: [0; 32],
        successor_vk_digest: [0; 32],
        kind: KagemushaTransitionKindV1::MintFold,
        amount: 0,
        mint_finality_semantic_digest: [0; 32],
        mint_finality_proof_binding_digest: [0; 32],
        peer_credit_id: [0; 32],
        recipient_encryption_key_binding: [0; 32],
        lifecycle_binding_digest: [0; 32],
        prepared_transition_binding_digest: [0; 32],
        receive_credit_binding_digest: [0; 32],
        predecessor_release_id: [0; 32],
        release_id: [0; 32],
        asset_incarnation: before.asset_incarnation,
        liability_pool_id: [0; 32],
        hardware_profile_id: [0; 32],
        policy_epoch: 0,
        lane: before.lane.clone(),
        predecessor_commitment: [0; 32],
        successor_commitment: [0; 32],
        predecessor_sequence: 0,
        successor_sequence: 0,
        predecessor_epoch: before.hardware_epoch,
        successor_epoch: before.hardware_epoch,
        predecessor_device_policy_binding: before.device_policy_binding,
        successor_device_policy_binding: before.device_policy_binding,
        predecessor_state_nonce_commitment: [0; 32],
        successor_state_nonce_commitment: [0; 32],
        journal_revision_before: 0,
        journal_revision_after: 0,
        effect_digest: [0; 32],
    };
    let guard_context = KagemushaGuardContextV1 {
        release_id: [0; 32],
        liability_pool_id: [0; 32],
        lifecycle_binding_digest: [0; 32],
        prepared_transition_binding_digest: [0; 32],
        terminal_commit_binding_digest: [0; 32],
        sender_one_time_authorization_digest: [0; 32],
        receive_credit_binding_digest: [0; 32],
        transition_intent_digest: [0; 32],
        transition_effect_digest: [0; 32],
        recovery_record_digest: [0; 32],
        durable_inbox_effect_digest: [0; 32],
        durable_outbox_effect_digest: [0; 32],
        canonical_empty_effect_digest: [0; 32],
    };
    let normalized = KagemushaNormalizedGuardStatementV1 {
        version: 0,
        protocol_version: 0,
        predecessor_suite_id: [0; 32],
        predecessor_vk_digest: [0; 32],
        successor_suite_id: [0; 32],
        successor_vk_digest: [0; 32],
        operation: crate::kagemusha_v1_recursion::KagemushaOperationV1::MintFold,
        amount: 0,
        peer_credit_id: [0; 32],
        recipient_encryption_key_binding: [0; 32],
        mint_finality_proof_binding_digest: [0; 32],
        predecessor_release_id: [0; 32],
        release_id: [0; 32],
        network_id: [0; 32],
        asset_id: [0; 32],
        asset_incarnation: before.asset_incarnation,
        asset_scale: 0,
        liability_pool_id: [0; 32],
        hardware_profile_id: [0; 32],
        policy_epoch: 0,
        lane_id: [0; 32],
        predecessor_state_commitment: [0; 32],
        successor_state_commitment: [0; 32],
        predecessor_state_nonce_commitment: [0; 32],
        successor_state_nonce_commitment: [0; 32],
        predecessor_logical_sequence: 0,
        successor_logical_sequence: 0,
        predecessor_hardware_epoch_generation: 0,
        successor_hardware_epoch_generation: 0,
        predecessor_hardware_epoch_id: [0; 32],
        successor_hardware_epoch_id: [0; 32],
        predecessor_key_reference: [0; 32],
        successor_key_reference: [0; 32],
        predecessor_hardware_policy_id: [0; 32],
        successor_hardware_policy_id: [0; 32],
        journal_revision_before: 0,
        journal_revision_after: 0,
        lifecycle_binding_digest: [0; 32],
        prepared_transition_binding_digest: [0; 32],
        terminal_commit_binding_digest: [0; 32],
        sender_one_time_authorization_digest: [0; 32],
        receive_credit_binding_digest: [0; 32],
        transition_intent_digest: [0; 32],
        transition_effect_digest: [0; 32],
        recovery_record_digest: [0; 32],
        durable_inbox_effect_digest: [0; 32],
        durable_outbox_effect_digest: [0; 32],
    };
    let subject = KagemushaHardwareTransitionSelectionV1 {
        version: 0,
        release_id: [0; 32],
        provider_policy_root: [0; 32],
        app_policy_digest: [0; 32],
        credential_id: [0; 32],
        network_id: before.lane.network_id.clone(),
        lane_commitment: [0; 32],
        hardware_profile_id: [0; 32],
        policy_epoch: 0,
        hardware_epoch_id: [0; 32],
        hardware_epoch_generation: 0,
        operation_kind: KagemushaOperationKindV1::MintFold,
        transition_statement_digest: [0; 32],
        candidate_envelope_digest: [0; 32],
        terminal_body_commitment: [0; 32],
        secure_index_before: 0,
        secure_index_after: 0,
    };
    let preparation = KagemushaOrdinaryIncomingPreparationV1 {
        version: 1,
        reservation_digest: [0; 32],
        operation_id: context.operation_id,
        nonce: [0; 32],
        transition_statement_digest: [0; 32],
        predecessor_state_commitment: before.state_commitment,
        successor_state_commitment: [0; 32],
        financial_control_original_sha256: [0; 32],
        clock_context_digest: [0; 32],
        financial_index_before: before.secure_index,
        financial_index_after: 0,
        logical_journal_sequence_before: owner.financial_journal_revision,
        logical_journal_sequence_after: 0,
    };
    let challenge = KagemushaAppOperationApprovalChallengeV1 {
        version: 1,
        purpose: KagemushaAppOperationApprovalPurposeV1::PrepareTransition,
        operation_id: context.operation_id,
        nonce: [0; 32],
        account_binding: [0; 32],
        authority_policy_digest: [0; 32],
        attested_key_id: [0; 32],
        enrollment_digest: [0; 32],
        subject_signing_digest: [0; 32],
        normalized_guard_digest: [0; 32],
        issued_at_ms: 0,
        expires_at_ms: 0,
        subject,
    };
    let originals = IncomingPreparedOriginals {
        reservation,
        mint_originals: Some((
            vec![0; KAGEMUSHA_ORDINARY_FINALIZED_TOPUP_MAX_BYTES_V1],
            vec![0; KAGEMUSHA_MINT_CREDIT_MAX_BYTES_V1],
        )),
        financial_control: CapturedFinancialControlIdentity {
            original_sha256: context.financial_control_original_sha256,
            lower_ms: context.clock_context.lower_at_ms,
            upper_ms: context.clock_context.upper_at_ms,
        },
        clock: context.clock_context.clone(),
        nonce: [0; 32],
        successor_nonce: [0; 32],
        preparation,
        statement,
        successor: before.clone(),
        normalized,
        context: guard_context,
        challenge,
        lease_original,
        previous_counter,
        maximum_record_payload_bytes: owner.maximum_record_payload_bytes,
        private_checkpoint_maximum_bytes: u64::try_from(crate::kagemusha_v1_recursion::KagemushaRecursiveStateCheckpointV1::maximum_encoded_bytes(&owner.verifier).map_err(material)?).map_err(material)?,
    };
    incoming_prepared_capacity_v1(&originals)
}

/// Number of Main rows reserved before irreversible incoming platform/global actions. The
/// allowance includes W2 original/capture, candidate before global Reserve, W1 selection/fence/original/capture, PreparedCommit,
/// StateAdvance/Ack and intent/framing. Core CAS has its own separate retained journal quota.
pub(in super::super) const INCOMING_COMPLETION_MAIN_ROWS_V1: u64 = 13;

/// Full supported suffix, measured from this same owner's canonical fixed typed layout. The
/// complete W2 structural row already contains State, lineage, lane, subject, normalized Guard,
/// challenge, clock and FI primitives; sixteen such rows conservatively retain the smaller
/// fixed incoming/W1/commit/Ack layouts and their finite framing. Variable byte carriers are
/// independently added at their sole protocol maxima. No 122-MiB allocation is needed to count.
/// Each row is separately bounded by the same selected release's physical row ceiling.
pub(super) fn incoming_completion_capacity_v1(
    originals: &IncomingPreparedOriginals,
) -> Result<u64, KagemushaStateErrorV1> {
    let mut structural = originals.clone();
    structural.mint_originals = None;
    structural.lease_original = None;
    let fixed = frame_bytes(
        &Record::IncomingApproval(IncomingApprovalRecord::Prepared(structural)),
        originals.maximum_record_payload_bytes,
    )?;
    let variable = [
        crate::kagemusha_v1_recursion::KAGEMUSHA_ORDINARY_INCOMING_COMMIT_BUNDLE_MAX_BYTES_V1,
        crate::kagemusha_v1_recursion::KAGEMUSHA_ORDINARY_INCOMING_RESERVATION_BUNDLE_MAX_BYTES_V1,
        2 * 32 * 1024, // Candidate row and PreparedCommit retain separate complete PUBLIC originals.
        crate::kagemusha_v1_state::KAGEMUSHA_GUARD_BUNDLE_MAX_BYTES_V1, // Durable pre-Reserve W2 Guard.
        iroha_data_model::kagemusha::KAGEMUSHA_ORDINARY_LINEAGE_ORIGINAL_MAX_BYTES_V1,
        crate::kagemusha_v1_state::KAGEMUSHA_GUARD_BUNDLE_MAX_BYTES_V1,
        2 * iroha_data_model::kagemusha::KAGEMUSHA_APP_OPERATION_APPROVAL_MAX_BYTES_V1,
        KAGEMUSHA_PLAY_INTEGRITY_REFRESH_LEASE_MAX_BYTES_V1,
    ]
    .into_iter()
    .try_fold(0u64, |n, v| {
        n.checked_add(u64::try_from(v).map_err(material)?)
            .ok_or(KagemushaStateErrorV1::InvalidDurableCapacity)
    })?;
    fixed
        .checked_add(256)
        .and_then(|n| n.checked_mul(16))
        .and_then(|n| n.checked_add(variable))
        // Three complete private originals: pre-Reserve candidate, W1Select and PreparedCommit. All are reserved before irreversible debit.
        .and_then(|n| {
            originals
                .private_checkpoint_maximum_bytes
                .checked_mul(3)
                .and_then(|private| n.checked_add(private))
        })
        .ok_or(KagemushaStateErrorV1::InvalidDurableCapacity)
}

/// Reserve the complete W2 frame and supported whole post-proof chronology before the W2
/// platform fence. Mint holds this exact original/recomputed allowance before its debit fence.
pub(super) fn incoming_prepared_capacity_v1(
    originals: &IncomingPreparedOriginals,
) -> Result<u64, KagemushaStateErrorV1> {
    let completion = incoming_completion_capacity_v1(originals)?;
    frame_bytes(
        &Record::IncomingApproval(IncomingApprovalRecord::Prepared(originals.clone())),
        originals.maximum_record_payload_bytes,
    )?
    .checked_add(256)
    .and_then(|n| n.checked_add(completion))
    .ok_or(KagemushaStateErrorV1::InvalidDurableCapacity)
}

/// Pure numeric recorded/recomputed equality, never an owner or approval admission.
pub(in super::super) fn require_recorded_allowance_v1(
    recorded: u64,
    recomputed: u64,
) -> Result<(), KagemushaStateErrorV1> {
    if recorded == 0 || recorded != recomputed {
        return Err(KagemushaStateErrorV1::InvalidDurableCapacity);
    }
    Ok(())
}

/// Pure full-frame/reservation/aggregate/quota relation; the caller supplies actual held values.
/// Its unit result never carries source/finality/Current or a Native owner capability.
pub(in super::super) fn require_prepared_frame_quota_v1(
    actual: u64,
    reserved: u64,
    aggregate: u64,
    quota: u64,
    maximum_payload_bytes: u64,
) -> Result<(), KagemushaStateErrorV1> {
    if actual == 0
        || actual > maximum_payload_bytes
        || reserved == 0
        || actual > reserved
        || aggregate < reserved
        || aggregate > quota
    {
        return Err(KagemushaStateErrorV1::InvalidDurableCapacity);
    }
    Ok(())
}

fn checked_inbox_add(current: u64, additional: u64) -> Result<u64, KagemushaStateErrorV1> {
    current
        .checked_add(additional)
        .ok_or(KagemushaStateErrorV1::InvalidDurableCapacity)
}

/// Require the actual complete Prepared row under its real held aggregate inbox reservation.
/// This numeric check never substitutes for the caller's existing genuine derive/readmission.
pub(super) fn require_prepared_record_capacity_v1(
    owner: &KagemushaNativeOrdinaryCashOwnerV1,
    record: &Record,
) -> Result<(), KagemushaStateErrorV1> {
    let Record::IncomingApproval(IncomingApprovalRecord::Prepared(originals)) = record else {
        return Err(KagemushaStateErrorV1::InvalidDurableCapacity);
    };
    if originals.maximum_record_payload_bytes != owner.maximum_record_payload_bytes {
        return Err(KagemushaStateErrorV1::InvalidDurableCapacity);
    }
    let actual = frame_bytes(record, owner.maximum_record_payload_bytes)?;
    let reserved = incoming_prepared_capacity_v1(originals)?;
    let intent = &owner
        .pending_incoming
        .as_ref()
        .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?
        .intent;
    match (intent.source, originals.mint_originals.as_ref()) {
        (SourceLocator::Mint, Some(_)) => owner.require_mint_incoming_prepared_capacity(actual),
        (SourceLocator::Receive { .. }, None) => {
            let mut aggregate =
                checked_inbox_add(owner.retained_received_source_capacity_charge()?, reserved)?;
            for retained in owner.retained_receiver_requests.values() {
                aggregate = checked_inbox_add(
                    aggregate,
                    retained.captured.reservation().capacity_charge_bytes()?,
                )?;
            }
            require_prepared_frame_quota_v1(
                actual,
                reserved,
                aggregate,
                owner.capacity.inbox_bytes,
                owner.maximum_record_payload_bytes,
            )
        }
        _ => Err(KagemushaStateErrorV1::SnapshotIntegrity),
    }
}

// DRAFT/UNEXECUTED genuine owner controls: insufficient complete pre-fence quota, changed/missing
// old-layout allowance, full max finalized original + neutral credit + future PI/counter shape,
// actual Prepared one byte over held frame allowance, aggregate retained source/request exhaustion,
// checked arithmetic/format overflow and cold Prepared replay overage. No fake authority fixture.
// The same genuine source/FI/clock/finality/Current/opening/consumption controls remain mandatory.

// All source tests below are DRAFT/UNEXECUTED. Their plain DATA record is deliberately not an
// admitted request/proof or Native owner, and is never persisted. They exercise generic canonical
// Record framing and numeric quota rules; genuine complete Prepared/owner controls stay separate.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draft_complete_canonical_frame_cannot_use_raw_blob_only_allowance() {
        let raw = vec![0; 97];
        let record = Record::Mint(MintRecord::ProvenRequest {
            operation: [0; 32],
            original: raw.clone(),
        });
        let whole = frame_bytes(&record, 16 * 1024).unwrap();
        let encoded = norito::encode_canonical(&record).unwrap();
        assert_eq!(whole, u64::try_from(encoded.len()).unwrap());
        let raw_only = u64::try_from(raw.len()).unwrap();
        assert!(whole > raw_only);
        assert!(require_prepared_frame_quota_v1(whole, raw_only, whole, whole, 16 * 1024).is_err());
        assert!(require_prepared_frame_quota_v1(whole, whole, whole, whole, 16 * 1024).is_ok());
        assert!(
            require_prepared_frame_quota_v1(whole + 1, whole, whole, whole + 1, 16 * 1024).is_err()
        );
        assert!(
            require_prepared_frame_quota_v1(whole, whole, whole, whole - 1, 16 * 1024).is_err()
        );
    }

    #[test]
    fn draft_zero_changed_or_omitted_recorded_capacity_never_fits_quota() {
        assert!(require_recorded_allowance_v1(0, 128).is_err());
        assert!(require_recorded_allowance_v1(127, 128).is_err());
        assert!(require_recorded_allowance_v1(129, 128).is_err());
        assert!(require_prepared_frame_quota_v1(0, 128, 128, 128, 16 * 1024).is_err());
        assert!(require_prepared_frame_quota_v1(128, 0, 128, 128, 16 * 1024).is_err());
        assert!(require_prepared_frame_quota_v1(128, 128, 127, 128, 16 * 1024).is_err());
        let over_format = 16 * 1024u64.checked_add(1).unwrap();
        assert!(require_recorded_allowance_v1(over_format, over_format).is_ok());
        assert!(
            require_prepared_frame_quota_v1(
                over_format,
                over_format,
                over_format,
                over_format,
                16 * 1024
            )
            .is_err()
        );
        assert!(
            require_prepared_frame_quota_v1(128, over_format, over_format, over_format, 16 * 1024)
                .is_ok()
        );
    }

    #[test]
    fn draft_aggregate_retention_exhaustion_and_addition_overflow_are_rejected() {
        let aggregate = checked_inbox_add(128, 64).unwrap();
        assert!(require_prepared_frame_quota_v1(128, 128, aggregate, 191, 16 * 1024).is_err());
        assert!(checked_inbox_add(u64::MAX, 1).is_err());
        assert!(checked_inbox_add(u64::MAX - 63, 64).is_err());
    }
}
