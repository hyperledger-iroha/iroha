//! Data-only lineage envelope shape checks; no proof or Native authority is fabricated.
use super::*;
use iroha_data_model::{
    kagemusha::{
        KAGEMUSHA_ENCRYPTED_CREDIT_MAX_BYTES_V1, KagemushaOrdinaryPaymentRequestBodyV1,
        kagemusha_ordinary_credit_id_v1,
    },
    testing::ordinary_mint::kagemusha_ordinary_mint_codec_fixture_v1,
};

#[test]
fn outgoing_send_original_accepts_canonical_envelope_and_refuses_length_substitution() {
    let fixture = kagemusha_ordinary_mint_codec_fixture_v1();
    let context = &fixture.request.authorization.statement.context;
    let clock = KagemushaOrdinaryCashClockContextV1 {
        version: 1,
        request_nonce: [31; 32],
        signed_observations_original_digest: [32; 32],
        lower_at_ms: 1000,
        upper_at_ms: 1001,
    };
    let request = KagemushaOrdinaryPaymentRequestV1 {
        body: KagemushaOrdinaryPaymentRequestBodyV1 {
            version: 1,
            release_id: context.release_id,
            network_id: [33; 32],
            normalized_asset_id: [34; 32],
            asset_incarnation: [35; 32],
            scale: 4,
            reserve_pool_id: [36; 32],
            recipient_account_binding: [37; 32],
            amount: 17,
            recipient_encryption_key: context.recipient_one_time_key,
            recipient_credential_digest: context.recipient_app_credential_digest,
            recipient_lane_id: [38; 32],
            request_id: [39; 32],
            clock_context: clock,
            issued_at_ms: 1000,
            expires_at_ms: 1100,
        },
        // Only the data shape is admitted here. This signature is not for the request body.
        evidence: fixture.request.authorization.approval.evidence.clone(),
    };
    let request_digest = request.canonical_original_digest().unwrap();
    let nullifier = [40; 32];
    let output = KagemushaOrdinaryPaymentOutputV1 {
        version: 1,
        request_digest,
        amount: 17,
        sender_before_commitment: [41; 32],
        sender_after_commitment: [42; 32],
        transition_nullifier: nullifier,
        credit_id: kagemusha_ordinary_credit_id_v1(nullifier, request_digest),
        ciphertext_commitment: [43; 32],
        encrypted_credit_digest: kagemusha_ciphertext_digest_v1(&fixture.request.encrypted_credit),
        clock_context_digest: clock.binding_digest().unwrap(),
        prepared_at_ms: clock.upper_at_ms,
    };
    let original = KagemushaOrdinaryLineageOutgoingOriginalsV1::Send {
        request: Box::new(request),
        output,
        encrypted_credit: fixture.request.encrypted_credit.clone(),
        preparation_clock: clock,
    };
    assert_eq!(
        fixture.request.encrypted_credit.len(),
        KAGEMUSHA_ENCRYPTED_CREDIT_CANONICAL_BYTES_V1
    );
    original.validate_data().unwrap();
    let mut lengths = [
        fixture.request.encrypted_credit.clone(),
        fixture.request.encrypted_credit.clone(),
        fixture.request.encrypted_credit.clone(),
    ];
    lengths[0].pop();
    lengths[1].push(0);
    lengths[2].resize(KAGEMUSHA_ENCRYPTED_CREDIT_MAX_BYTES_V1, 0);
    for changed in lengths {
        let mut rejected = original.clone();
        let KagemushaOrdinaryLineageOutgoingOriginalsV1::Send {
            encrypted_credit, ..
        } = &mut rejected
        else {
            unreachable!()
        };
        *encrypted_credit = changed;
        assert!(rejected.validate_data().is_err());
    }
    original.validate_data().unwrap();
}
