// First-release AXT carrier admission controls for signed source-anchored spends.

use super::*;
use crate::{
    block::valid::{
        axt_fastpq_proof_verification_count, map_axt_fastpq_error,
        reset_axt_fastpq_proof_verification_count, validate_axt_envelopes,
    },
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, World},
};
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, Signature};
use iroha_data_model::{
    NetworkId,
    nexus::{
        AssetHandleDraft, AxtAnchoredSpendDraftV1, AxtAnchoredSpendV1, AxtBinding, AxtDescriptor,
        AxtEnvelopeRecord, AxtFastpqBinding, AxtFinalizedSpendAnchorV1, AxtHandleIssuerContextV1,
        AxtHandleReplayKey, AxtPolicyBinding, AxtPolicyEntry, AxtPolicySnapshot, AxtProofEnvelope,
        AxtSourceSuccessReceiptV1, AxtSourceTransferOccurrenceV1, AxtSpendNonceV1, GroupBinding,
        HandleBudget, HandleSubject, ProofBlob, RemoteSpendIntent, SpendOp, UniversalAccountId,
        compute_remote_spend_intent_commitment_v1,
    },
};
use iroha_primitives::{numeric::Quantity, time::TimeSource};
use std::{
    collections::{BTreeMap, BTreeSet},
    time::Duration,
};

fn signed_spend(binding: AxtBinding) -> AxtAnchoredSpendV1 {
    let issuer = KeyPair::from_seed(vec![0x33; 32], Algorithm::Ed25519);
    let dsid = DataSpaceId::new(7);
    let lane = LaneId::new(0);
    let genesis = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"axt-v1-genesis"));
    let network_id = NetworkId::from_genesis_hash(genesis);
    let asset =
        AssetDefinitionId::from_uuid_bytes([0, 0, 0, 0, 0, 0, 0x40, 0, 0x80, 0, 0, 0, 0, 0, 0, 1])
            .expect("canonical asset id");
    let context = AxtHandleIssuerContextV1 {
        network_id,
        asset_dsid: dsid,
        asset_definition_incarnation: iroha_data_model::nexus::AxtAssetIncarnationV1::derive(
            &network_id,
            &asset,
            &HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"asset registration")),
            &Hash::new(b"asset execution"),
            0,
        ),
        issuer: UniversalAccountId::from_hash(Hash::new(b"axt-v1-issuer")),
        issuer_manifest_root: [0x5A; 32],
        code_root: [0xC0; 32],
        abi_version: 1,
        abi_hash: [0xAB; 32],
    };
    let handle = AssetHandleDraft {
        asset_definition_id: asset.clone(),
        scope: vec!["transfer".into()],
        subject: HandleSubject {
            account: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV".into(),
            origin_dsid: Some(dsid),
        },
        budget: HandleBudget {
            remaining: Quantity::from(10_u64),
            per_use: Some(Quantity::from(5_u64)),
        },
        handle_era: 1,
        sub_nonce: 1,
        group_binding: GroupBinding {
            composability_group_id: b"settlement".to_vec(),
            epoch_id: 1,
        },
        target_lane: lane,
        axt_binding: binding,
        manifest_view_root: [0x5A; 32],
        expiry_slot: 100,
        max_clock_skew_ms: Some(0),
    }
    .sign_by_issuer_v1(context, issuer.private_key())
    .expect("issuer signs reusable handle");
    let intent = RemoteSpendIntent {
        asset_dsid: dsid,
        op: SpendOp {
            asset_definition_id: asset,
            kind: "transfer".into(),
            from: handle.subject.account.clone(),
            to: "sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76".into(),
            amount: Some(Quantity::from(5_u64)),
        },
    };
    let claim = compute_remote_spend_intent_commitment_v1(
        AxtHandleReplayKey::from_handle(dsid, &handle),
        &intent.op.asset_definition_id,
        &intent.op.kind,
        &intent.op.from,
        &intent.op.to,
        intent.op.amount.as_ref().expect("clear amount"),
    );
    let anchor = AxtFinalizedSpendAnchorV1 {
        network_id,
        genesis_hash: *network_id.as_bytes(),
        dataspace_id: dsid,
        lane_id: lane,
        lane_incarnation: Hash::new(b"lane incarnation"),
        finalized_height: 2,
        block_header_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
            b"source block",
        )),
        quorum_certificate_digest: Hash::new(b"source QC"),
        committee_digest: Hash::new(b"source committee"),
        pre_state_root: Hash::new(b"source prestate"),
        post_state_root: Hash::new(b"source poststate"),
        transaction_set_digest: Hash::new(b"source transaction set"),
        da_manifest_digest: Hash::new(b"source DA"),
    };
    let receipt = AxtSourceSuccessReceiptV1 {
        finalized_anchor_digest: anchor.digest_v1(),
        source_tx_commitment: [0xAA; 32],
        source_tx_index: 0,
        post_transaction_state_root: [0xBB; 32],
        effect_set_digest: [0xCC; 32],
    };
    let occurrence = AxtSourceTransferOccurrenceV1 {
        source_tx_commitment: receipt.source_tx_commitment,
        source_success_receipt_digest: receipt.digest_v1(),
        source_tx_index: receipt.source_tx_index,
        transcript_index: 0,
        delta_index: 0,
        pair_ordinal: 0,
        transfer_digest: [0xDD; 32],
        remote_spend_claim_commitment: claim,
    };
    let proof = AxtProofEnvelope {
        dsid,
        manifest_root: handle.manifest_view_root,
        da_commitment: Some(anchor.da_manifest_digest.into()),
        proof: vec![0xA5, 0x5A],
        fastpq_binding: Some(AxtFastpqBinding {
            parameter: "fastpq-state-transition-stark-v1".into(),
            source_dsid: dsid.as_u64(),
            source_dataspace: "source".into(),
            source_receipt_id: "receipt".into(),
            source_tx_commitment: hex::encode(receipt.source_tx_commitment),
            claim_type: "tx_predicate".into(),
            claim_digest: "bb".repeat(32),
            witness_commitment: "cc".repeat(32),
            policy_commitment: "dd".repeat(32),
            verified_effect_type: "transfer".into(),
            corridor: "test".into(),
            verifier_id: "fastpq".into(),
            verifier_version: "v1".into(),
            target_dsids: vec![dsid.as_u64()],
            effect_binding: None,
            remote_spend_intent_commitments: vec![claim],
        }),
        committed_amount: Some(5),
        amount_commitment: None,
    };
    AxtAnchoredSpendDraftV1 {
        handle,
        intent,
        proof: Some(ProofBlob {
            payload: norito::to_bytes(&proof).expect("proof envelope"),
            expiry_slot: Some(100),
        }),
        amount: Some(Quantity::from(5_u64)),
        amount_commitment: None,
        source_receipt: receipt,
        source_occurrence: occurrence,
    }
    .sign_by_issuer_v1(
        anchor,
        100,
        AxtSpendNonceV1::try_new([0x77; 32]).expect("nonce"),
        issuer.private_key(),
    )
    .expect("issuer signs exact spend")
}

fn test_block(envelopes: Vec<AxtEnvelopeRecord>, snapshot: AxtPolicySnapshot) -> SignedBlock {
    let (_handle, time_source) = TimeSource::new_mock(Duration::from_millis(0));
    let signer = crate::block::checked_keypair();
    let mut block: SignedBlock = BlockBuilder::new_with_time_source(Vec::new(), time_source)
        .chain(0, None)
        .sign(signer.private_key())
        .unpack(|_| {})
        .into();
    block
        .set_execution_outputs(
            crate::execution_output_test_support::structural_network_outputs(
                &block,
                &Vec::<HashOf<TransactionEntrypoint>>::new(),
                Vec::<TransactionResultInner>::new(),
            ),
            u64::try_from(block.network_entrypoint_count()).expect("fixture input count"),
            BTreeMap::new(),
            envelopes,
            snapshot,
            BTreeSet::new(),
            Vec::new(),
            &crate::execution_output_test_support::structural_output_limits(),
        )
        .expect("test block carries canonical outputs");
    block
}

fn test_state() -> State {
    State::new_for_testing(
        World::new(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    )
}

fn signed_envelope(spend: AxtAnchoredSpendV1) -> AxtEnvelopeRecord {
    let descriptor = AxtDescriptor {
        dsids: vec![DataSpaceId::new(7)],
        touches: Vec::new(),
    };
    AxtEnvelopeRecord {
        binding: descriptor.binding().expect("descriptor binding"),
        lane: LaneId::new(0),
        descriptor,
        touches: Vec::new(),
        proofs: Vec::new(),
        spends: vec![spend],
        commit_height: 1,
    }
}

#[test]
fn local_fastpq_allocation_refusal_defers_without_axt_reject_reason() {
    let deferred = map_axt_fastpq_error(
        fastpq_prover::Error::LocalAllocationUnavailable {
            context: "axt test allocation",
        },
        "FASTPQ verification failed",
        1,
        DataSpaceId::new(7),
        LaneId::new(0),
    );
    assert!(matches!(
        deferred,
        BlockValidationError::ExecutionDeferred(reason)
            if reason.reason() == ivm::error::ExecutionDeferral::AllocationUnavailable
    ));
    let invalid = map_axt_fastpq_error(
        fastpq_prover::Error::CommitmentMismatch,
        "FASTPQ verification failed",
        1,
        DataSpaceId::new(7),
        LaneId::new(0),
    );
    assert!(matches!(
        invalid,
        BlockValidationError::AxtEnvelopeValidationFailed(details)
            if details.reason == AxtRejectReason::Proof
    ));
}

fn empty_envelope(commit_height: u64) -> AxtEnvelopeRecord {
    let descriptor = AxtDescriptor {
        dsids: vec![DataSpaceId::new(7)],
        touches: Vec::new(),
    };
    AxtEnvelopeRecord {
        binding: descriptor.binding().expect("descriptor binding"),
        lane: LaneId::new(0),
        descriptor,
        touches: Vec::new(),
        proofs: Vec::new(),
        spends: Vec::new(),
        commit_height,
    }
}

#[test]
fn envelope_commit_height_must_match_block_height() {
    for height in [0, 2] {
        let state = test_state();
        let block = test_block(vec![empty_envelope(height)], AxtPolicySnapshot::default());
        let state_block = state.block(block.header());
        assert!(matches!(
            validate_axt_envelopes(&block, &state_block),
            Err(BlockValidationError::AxtEnvelopeValidationFailed(details))
                if details.reason == AxtRejectReason::Descriptor
                    && details.message.contains("commit height")
        ));
    }
}

#[test]
fn resultless_carrier_cannot_borrow_live_policy_snapshot() {
    let state = test_state();
    let block = test_block(vec![empty_envelope(1)], AxtPolicySnapshot::default())
        .canonical_resultless_proposal();
    let state_block = state.block(block.header());
    assert!(matches!(
        validate_axt_envelopes(&block, &state_block),
        Err(BlockValidationError::AxtEnvelopeValidationFailed(details))
            if details.reason == AxtRejectReason::MissingPolicy
                && details.snapshot_version.is_none()
    ));
}

#[test]
fn embedded_policy_snapshot_must_be_canonical() {
    let state = test_state();
    let base = test_block(vec![empty_envelope(1)], AxtPolicySnapshot::default());
    let policy = AxtPolicyEntry {
        manifest_root: [0x42; 32],
        target_lane: LaneId::new(0),
        active_handle_era: 1,
        next_handle_counter: 1,
        current_slot: 1,
    };
    let first = AxtPolicyBinding {
        dsid: DataSpaceId::new(1),
        policy,
    };
    let second = AxtPolicyBinding {
        dsid: DataSpaceId::new(2),
        policy,
    };
    let invalid_rows = [vec![first, first], vec![second, first]];
    for entries in invalid_rows {
        let mut block = base.clone();
        block
            .replace_axt_policy_snapshot_for_testing(AxtPolicySnapshot {
                version: AxtPolicySnapshot::compute_version(&entries),
                entries,
            })
            .expect("test block has outputs");
        let state_block = state.block(block.header());
        assert!(matches!(
            validate_axt_envelopes(&block, &state_block),
            Err(BlockValidationError::AxtEnvelopeValidationFailed(details))
                if details.reason == AxtRejectReason::PolicyDenied
                    && details.message.contains("invalid AXT policy snapshot")
        ));
    }
}

#[test]
fn signed_spends_are_rejected_before_fastpq_verification() {
    let descriptor = AxtDescriptor {
        dsids: vec![DataSpaceId::new(7)],
        touches: Vec::new(),
    };
    let binding = descriptor.binding().expect("descriptor binding");
    let signed = signed_spend(binding);
    assert!(signed.issuer_payload_v1().is_ok());
    reset_axt_fastpq_proof_verification_count();
    let state = test_state();
    let block = test_block(vec![signed_envelope(signed)], AxtPolicySnapshot::default());
    let state_block = state.block(block.header());
    let err = validate_axt_envelopes(&block, &state_block).expect_err("remote spend unavailable");
    let BlockValidationError::AxtEnvelopeValidationFailed(details) = err else {
        panic!("expected typed AXT rejection: {err:?}")
    };
    assert_eq!(details.reason, AxtRejectReason::Proof);
    assert!(
        details
            .message
            .contains(crate::fastpq::AXT_UNANCHORED_REMOTE_SPEND_REJECTION)
    );
    assert_eq!(axt_fastpq_proof_verification_count(), 0);
}

#[test]
fn malformed_signed_spends_are_all_rejected_before_verifier_dispatch() {
    let descriptor = AxtDescriptor {
        dsids: vec![DataSpaceId::new(7)],
        touches: Vec::new(),
    };
    let signed = signed_spend(descriptor.binding().expect("descriptor binding"));
    let mut wrong_signature = signed.clone();
    wrong_signature.authorization.issuer_signature = Signature::from_bytes(&[0x11; 64]);
    let mut wrong_receipt = signed.clone();
    wrong_receipt.draft.source_receipt.effect_set_digest = [0x22; 32];
    let mut wrong_occurrence = signed.clone();
    wrong_occurrence.draft.source_occurrence.pair_ordinal += 1;
    for spend in [wrong_signature, wrong_receipt, wrong_occurrence] {
        reset_axt_fastpq_proof_verification_count();
        let state = test_state();
        let block = test_block(vec![signed_envelope(spend)], AxtPolicySnapshot::default());
        let state_block = state.block(block.header());
        assert!(matches!(
            validate_axt_envelopes(&block, &state_block),
            Err(BlockValidationError::AxtEnvelopeValidationFailed(details))
                if details.reason == AxtRejectReason::Proof
        ));
        assert_eq!(axt_fastpq_proof_verification_count(), 0);
    }
}

#[test]
fn retired_fragment_envelope_cannot_decode_as_signed_spend_wire() {
    #[derive(norito::Encode, norito::NoritoSchema)]
    #[norito_schema(
        name = "test::RetiredFragmentEnvelope",
        frame = "iroha_data_model::nexus::axt::AxtEnvelopeRecord"
    )]
    struct RetiredFragmentEnvelope {
        binding: AxtBinding,
        lane: LaneId,
        descriptor: AxtDescriptor,
        touches: Vec<iroha_data_model::nexus::AxtTouchFragment>,
        proofs: Vec<iroha_data_model::nexus::AxtProofFragment>,
        handles: Vec<u8>,
        commit_height: u64,
    }
    let descriptor = AxtDescriptor {
        dsids: vec![DataSpaceId::new(7)],
        touches: Vec::new(),
    };
    let retired = RetiredFragmentEnvelope {
        binding: descriptor.binding().expect("descriptor binding"),
        lane: LaneId::new(0),
        descriptor,
        touches: Vec::new(),
        proofs: Vec::new(),
        handles: vec![1],
        commit_height: 1,
    };
    let wire = norito::to_bytes(&retired).expect("retired frame");
    assert!(norito::decode_from_bytes::<AxtEnvelopeRecord>(&wire).is_err());
}
