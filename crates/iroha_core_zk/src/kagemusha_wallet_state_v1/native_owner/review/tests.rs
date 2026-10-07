//! Engineering original/binding tests. These never install an owner or qualify a fold/phone.

use super::*;

fn vectors() -> norito::json::Value {
    norito::json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/kagemusha/wallet_v1_vectors.json"
    )))
    .unwrap()
}
fn request() -> KagemushaWalletRequestV1 {
    let all = vectors();
    let row = all["envelopes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["kind"].as_str() == Some("Request"))
        .unwrap();
    let envelope: KagemushaWalletEnvelopeV1 =
        archive::decode(&hex::decode(row["canonical_hex"].as_str().unwrap()).unwrap()).unwrap();
    let KagemushaWalletMessageV1::Request { request } = envelope.message else {
        panic!("Request fixture");
    };
    request
}
fn scheme() -> KagemushaWalletSchemeV1 {
    super::super::super::tests::fixture("KagemushaWalletSchemeV1")
}
fn credential() -> KagemushaWalletCredentialV1 {
    super::super::super::tests::fixture("KagemushaWalletCredentialV1")
}
fn unadmitted_review_fixture() -> ReviewedOperationV1 {
    let request = request();
    request.verify(&scheme()).unwrap();
    let owner = credential();
    let head = KagemushaWalletStateCommitmentV1 {
        value: kagemusha_wallet_field_from_u128_v1(1),
    };
    ReviewedOperationV1 {
        projection: NativeOperationReviewV1 {
            kind: KagemushaWalletOperationKindV1::Send,
            amount: request.body.amount,
            fee: request.body.fee,
            gross_debit: request.body.amount.checked_add(request.body.fee).unwrap(),
            net_destination_amount: request.body.amount,
            receiver_wallet_id: Some(request.body.receiver_wallet_id),
            destination_account_digest: request.body.receiver_account_digest,
            request_digest: request.request_digest(),
            charge_quote_digest: [0; 32],
            scheme_id: request.body.scheme_id,
            wallet_id: request.body.payer_wallet_id,
            current_head: head.value,
            source_state_commitment: head.value,
            source_capsule_digest: [2; 32],
            credential_digest: owner.credential_digest(),
            payment_key: owner.body.payment_key,
            artifact_manifest_digest: [3; 32],
        },
        source: ReviewSourceV1 {
            marker: KagemushaWalletMarkerV1 {
                version: 1,
                scheme_id: request.body.scheme_id,
                asset_digest: request.body.asset_digest,
                wallet_id: request.body.payer_wallet_id,
                payment_key: owner.body.payment_key,
                generation: 5,
                state: KagemushaWalletMarkerStateV1::Head {
                    sequence: 0,
                    operation_id: [4; 32],
                    head,
                    capsule_digest: [2; 32],
                    predecessor_capsule_digest: [0; 32],
                },
            },
            completion: [5; 32],
            archive_checkpoint: [6; 32],
            artifact_manifest: [3; 32],
            selected_generation: 4,
        },
        action: OperationActionV1::Send {
            request: KagemushaWalletEnvelopeV1::new(KagemushaWalletMessageV1::Request { request })
                .to_canonical_bytes()
                .unwrap(),
        },
    }
}

#[test]
fn signed_request_cannot_display_a_false_fee_or_retargeted_destination() {
    let original = request();
    let scheme = scheme();
    original.verify(&scheme).unwrap();
    for mutation in 0..5 {
        let mut changed = original.clone();
        match mutation {
            0 => changed.body.fee = changed.body.fee.checked_add(1).unwrap(),
            1 => changed.body.amount = changed.body.amount.checked_add(1).unwrap(),
            2 => changed.body.receiver_wallet_id[0] ^= 1,
            3 => changed.body.receiver_account_digest[0] ^= 1,
            _ => changed.body.payer_wallet_id[0] ^= 1,
        }
        assert!(changed.verify(&scheme).is_err());
    }
}

#[test]
fn whole_send_check_rejects_ordinal_and_spendable_changes_even_with_a_valid_signature() {
    let request = request();
    let scheme = scheme();
    request.verify(&scheme).unwrap();
    let owner = credential();
    let usage = KagemushaWalletQuotaUsageArrayV1::empty();
    let mut state =
        KagemushaWalletStateV1::bootstrap(&owner, kagemusha_wallet_field_from_u128_v1(17)).unwrap();
    state.core.enabled_controls = 0;
    state.core.balance = request.body.amount.checked_add(request.body.fee).unwrap();
    state.core.next_send = request.body.send_ordinal;
    state.core.policy_epoch = request.body.policy_epoch;
    state.core.accepted_time_floor_ms = request.body.receiver_accepted_time_ms;
    state.rest.scheme_policy = request.body.scheme_policy;
    state.rest.fee_schedule = request.body.fee_schedule;
    let omega = |state: &KagemushaWalletStateV1| KagemushaWalletLineagePublicV1 {
        version: 1,
        scheme_id: scheme.scheme_id(),
        relation_id: scheme.relation_id,
        head: state.commitment().unwrap(),
        wallet_id: owner.body.wallet_id,
        credential_digest: owner.credential_digest(),
        payment_key: owner.body.payment_key,
        lifecycle: state.core.lifecycle,
        policy_epoch: state.core.policy_epoch,
        enabled_controls: state.core.enabled_controls,
        burned_total: 0,
        pending_outgoing_root: state.core.pending_outgoing_root,
        credit_digest_root: kagemusha_wallet_field_from_u128_v1(1),
    };
    let check = |state: &KagemushaWalletStateV1| {
        request.check_send(&KagemushaWalletSendInputsV1 {
            payer_credential: &owner,
            payer_state: state,
            omega: &omega(state),
            anchored: None,
            now: None,
            blacklist: None,
            quota_share: None,
            quota_usage: &usage,
        })
    };
    check(&state).unwrap(); // Pure Model projection, never an admitted/proven Native fold.
    let mut next = state;
    next.core.next_send += 1;
    assert!(check(&next).is_err());
    let mut low = state;
    low.core.balance -= 1;
    assert!(check(&low).is_err());
}

#[test]
fn hardware_ui_data_and_copied_originals_cannot_replace_retained_action() {
    let review = unadmitted_review_fixture();
    let original = review.send_original().unwrap().to_vec();
    let mut data = review.projection().clone();
    data.fee = 0;
    data.gross_debit = 1;
    data.receiver_wallet_id = Some([99; 32]);
    let mut copied = original.clone();
    copied[0] ^= 1;
    assert_ne!(data, *review.projection());
    assert_ne!(copied, review.send_original().unwrap());
    let request = review.into_request([7; 32]).unwrap();
    assert_eq!(request.request_id, [7; 32]);
    assert_eq!(
        request.action,
        OperationActionV1::Send { request: original }
    );
}

#[test]
fn recheck_rejects_source_runtime_completion_generation_and_key_changes() {
    for mutation in 0..8 {
        let review = unadmitted_review_fixture();
        let mut fresh = unadmitted_review_fixture();
        review.require_recheck(&fresh).unwrap();
        match mutation {
            0 => fresh.source.archive_checkpoint[0] ^= 1,
            1 => fresh.source.artifact_manifest[0] ^= 1,
            2 => fresh.source.completion[0] ^= 1,
            3 => fresh.source.selected_generation += 1,
            4 => fresh.source.marker.wallet_id[0] ^= 1,
            5 => fresh.source.marker.payment_key = request().receiver_credential.body.payment_key,
            6 => fresh.projection.current_head[0] ^= 1,
            _ => fresh.projection.credential_digest[0] ^= 1,
        }
        assert!(matches!(
            review.require_recheck(&fresh),
            Err(Error::OperationConflict)
        ));
    }
}

#[test]
fn exact_review_action_rejects_retarget_and_false_fee_data_at_consumption() {
    let review = unadmitted_review_fixture();
    let mut changed = unadmitted_review_fixture();
    changed.projection.fee += 1;
    assert!(review.require_recheck(&changed).is_err());
    let mut changed = unadmitted_review_fixture();
    let OperationActionV1::Send { request } = &mut changed.action else {
        panic!("Send");
    };
    request[0] ^= 1;
    assert!(review.require_recheck(&changed).is_err());
    assert!(unadmitted_review_fixture().into_request([0; 32]).is_err());
}
