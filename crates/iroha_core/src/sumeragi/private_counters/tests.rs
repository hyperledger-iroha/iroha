//! Actual signed-envelope replay/clock admission controls, separate from native computation tests.

use super::*;
use iroha_crypto::{Algorithm, KeyPair};

fn installed_service(
    state: Arc<State>,
    owner: AccountId,
    key: &KeyPair,
) -> (PrivateCounterService, tempfile::TempDir) {
    use super::super::driver::traits::{LogEntry, RecordStore};
    let temporary = tempfile::tempdir().unwrap();
    let records = FileRecordStore::open(
        temporary.path().join("records"),
        temporary.path().join("installation.log"),
    )
    .unwrap();
    let view = state.view();
    let scope = super::super::lanes::routing::committed_root_scope(view.world()).unwrap();
    let instance = scope
        .instance_id(
            &super::super::crypto::BlsCrypto::new(),
            *view.network_id(),
            &view.chain_id().to_string(),
        )
        .unwrap();
    drop(view);
    let assertion = FreshKeyAssertion::from_operator_flag(true).unwrap();
    let service = PrivateCounterService::new(
        state,
        owner,
        PeerId::new(key.public_key().clone()),
        Arc::new(KeyPairSigner::new(key).unwrap()),
        instance,
        &records,
        Some(&assertion),
    )
    .unwrap();
    // The same durable Instance event that follows replay initialization at native startup.
    // This fixture does not pretend to start a separately running validator process.
    records.set_store_id(0x91).unwrap();
    records
        .append_log(&LogEntry::Instance {
            instance,
            key: super::super::crypto::core_key(key.public_key()).unwrap(),
            store_id: 0x91,
        })
        .unwrap();
    (service, temporary)
}

#[test]
fn installed_members_compute_genuine_matching_claims_before_signing_and_consume_the_nonce() {
    use crate::query::private_transaction_counters::tests::published_interactions_fixture_v1;
    use iroha_data_model::private_transaction_counters::collect_private_counters_v1;

    // Pin the original global epoch before this fixture creates charged retired
    // State generations. Keep it through response destruction so collector activity
    // cannot change this fixture's pre-call pool baseline during the exact checks.
    let retirement_epoch = mv::cell::Cell::<u8>::new(0);
    let retained_retirement_epoch = retirement_epoch.view();

    // One genuine certified State is shared solely by this component fixture. Live
    // deployment qualification still requires separately installed validator nodes.
    let fixture = published_interactions_fixture_v1();
    let owner = AccountId::new(fixture.native.policy_key.public_key().clone());
    let mut keys: Vec<_> = [0xC1, 0xC2, 0xC3, 0xC4]
        .into_iter()
        .map(|seed| KeyPair::from_seed(vec![seed; 32], Algorithm::BlsNormal))
        .collect();
    keys.sort_by_key(|key| PeerId::new(key.public_key().clone()));
    let services: Vec<_> = keys
        .iter()
        .take(3)
        .map(|key| installed_service(Arc::clone(fixture.native.chain.state()), owner.clone(), key))
        .collect();
    let mut payload = fixture.request(0).payload;
    payload.creation_time_ms = native_time_ms().unwrap();
    let request = payload.try_sign(&fixture.native.readers[0]).unwrap();
    let original = request.encode_canonical().unwrap();
    let mut originals = Vec::new();
    let mut expected_claim = None;
    for (index, (service, _directory)) in services.iter().enumerate() {
        let pool = fixture.native.chain.state().ivm_execution_budget();
        let before_response = pool.reserved_bytes();
        let response_original = service.compute_original(&original, || true).unwrap();
        assert!(response_original._operation_allocation.belongs_to(&pool));
        assert_eq!(
            response_original._operation_allocation.remaining_bytes(),
            MAX_COUNTER_OPERATION_BYTES
        );
        // The direct Native caller retains the genuine State charge after compute returns.
        assert_eq!(
            pool.reserved_bytes(),
            before_response + MAX_COUNTER_OPERATION_BYTES
        );
        let response =
            PrivateCountersResponseV1::decode_bounded_canonical(&response_original).unwrap();
        assert_eq!(response.attestation.body.member_index, index as u16);
        assert_eq!(
            response.attestation.body.claim_hash,
            response.claim.commitment().unwrap()
        );
        response
            .attestation
            .signature
            .verify(
                keys[index].public_key(),
                &response.attestation.body.signing_preimage().unwrap(),
            )
            .unwrap();
        assert_eq!(
            response
                .claim
                .groups
                .iter()
                .map(|group| group.count)
                .sum::<u64>(),
            30
        );
        if let Some(claim) = &expected_claim {
            assert_eq!(claim, &response.claim);
        } else {
            expected_claim = Some(response.claim);
        }
        assert!(matches!(
            service.compute_original(&original, || true),
            Err(PrivateCountersErrorV1::Replay)
        ));
        // The independent collector's test inputs remain original canonical frames.
        originals.push(response_original.as_ref().to_vec());
        drop(response_original);
        assert_eq!(pool.reserved_bytes(), before_response);
    }
    let certificate = collect_private_counters_v1(&originals).unwrap();
    let certificate = iroha_data_model::private_transaction_counters::PrivateCountersCertificateV1::decode_bounded_canonical(&certificate).unwrap();
    assert_eq!(certificate.attestations.len(), 3);
    assert_eq!(certificate.claim, expected_claim.unwrap());
    drop(retained_retirement_epoch);
}

#[test]
fn readiness_and_invalid_signature_refuse_before_computation_signing_or_nonce_consumption() {
    use crate::query::private_transaction_counters::tests::published_interactions_fixture_v1;

    let fixture = published_interactions_fixture_v1();
    let key = KeyPair::from_seed(vec![0xC1; 32], Algorithm::BlsNormal);
    let (service, _directory) = installed_service(
        Arc::clone(fixture.native.chain.state()),
        AccountId::new(fixture.native.policy_key.public_key().clone()),
        &key,
    );
    let mut payload = fixture.request(0).payload;
    payload.creation_time_ms = native_time_ms().unwrap();
    let request = payload.try_sign(&fixture.native.readers[0]).unwrap();
    let original = request.encode_canonical().unwrap();
    assert!(matches!(
        service.compute_original(&original, || false),
        Err(PrivateCountersErrorV1::Unavailable)
    ));
    assert!(service.replay.lock().consumed_len() == 0);
    let mut invalid = request.clone();
    invalid.payload.nonce[0] ^= 1;
    assert!(matches!(
        service.compute_original(&invalid.encode_canonical().unwrap(), || true),
        Err(PrivateCountersErrorV1::Signature)
    ));
    assert!(service.replay.lock().consumed_len() == 0);
    service.compute_original(&original, || true).unwrap();
    assert_eq!(service.replay.lock().consumed_len(), 1);
}
