//! Retained source custody tests use authentic G1 fixtures, not proof qualification.

use super::super::tests::{MemoryArchive, enrollment_issuer, fixture};
use super::*;
use KagemushaWalletOperationKindV1 as K;
use KagemushaWalletPolicyUpdateKindV1 as U;
use PreparationOriginalV1 as R;

fn setup() -> (MemoryArchive, KagemushaWalletStateV1, SourceCustodyV1) {
    let credential: KagemushaWalletCredentialV1 = fixture("KagemushaWalletCredentialV1");
    let certificate = enrollment_issuer(&credential);
    let certificates = KagemushaWalletCertificateSetV1::new(vec![certificate]).unwrap();
    let state =
        KagemushaWalletStateV1::bootstrap(&credential, kagemusha_wallet_field_from_u128_v1(8))
            .unwrap();
    let mut store = MemoryArchive::new();
    let source = SourceCustodyV1::bootstrap(
        &mut store,
        &state,
        &credential.to_canonical_bytes().unwrap(),
        &archive::encode(&certificates).unwrap(),
    )
    .unwrap();
    (store, state, source)
}

fn view<'a>(
    store: &'a mut MemoryArchive,
    state: &KagemushaWalletStateV1,
    source: &SourceCustodyV1,
) -> PreparationCustodyV1<'a> {
    PreparationCustodyV1::new(
        store,
        source,
        state,
        K::Send,
        None,
        IndexRoot::default(),
        IndexRoot::default(),
    )
    .unwrap()
}

#[test]
fn selected_originals_are_bound_and_roundtrip_without_invented_absence() {
    let (mut store, state, source) = setup();
    let bytes = archive::encode(&source).unwrap();
    let decoded: SourceCustodyV1 = archive::decode(&bytes).unwrap();
    let mut custody = view(&mut store, &state, &decoded);
    assert!(custody.original(R::CurrentCredential).unwrap().is_some());
    assert!(
        custody
            .original(R::EnrollmentCertificates)
            .unwrap()
            .is_some()
    );
    for role in [R::SchemePolicy, R::Blacklist, R::QuotaShare, R::TimeAnchor] {
        assert!(custody.original(role).unwrap().is_none());
    }
    assert!(custody.anchored_time().unwrap().is_none());
    let restored = custody.finish(&state).unwrap();
    assert_eq!(archive::encode(&restored).unwrap(), bytes);
    let mut missing = source.clone();
    missing.originals[R::CurrentCredential.index()] = None;
    assert!(matches!(
        missing.require(&mut store, &state),
        Err(Error::WitnessLost(_))
    ));
    let mut malformed = source.clone();
    malformed.version = 2;
    assert!(malformed.require(&mut store, &state).is_err());
    let mut changed = state;
    changed.core.credential_digest = kagemusha_wallet_field_from_u128_v1(9);
    assert!(source.require(&mut store, &changed).is_err());
    store
        .remove(ArchiveKey::Object(source.originals[0].unwrap()))
        .unwrap();
    assert!(matches!(
        source.require(&mut store, &state),
        Err(Error::WitnessLost(_))
    ));
}

#[test]
fn refresh_original_drafts_are_role_bound_and_only_published_with_the_successor() {
    let (mut store, state, source) = setup();
    let policy: KagemushaWalletSchemePolicyV1 = fixture("KagemushaWalletSchemePolicyV1");
    let bytes = policy.to_canonical_bytes().unwrap();
    let mut send = view(&mut store, &state, &source);
    assert!(
        send.retain_successor_original(R::SchemePolicy, &bytes)
            .is_err()
    );
    drop(send);
    let mut refresh = PreparationCustodyV1::new(
        &mut store,
        &source,
        &state,
        K::RefreshPolicy,
        Some(U::SchemePolicy),
        IndexRoot::default(),
        IndexRoot::default(),
    )
    .unwrap();
    refresh
        .retain_successor_original(R::SchemePolicy, &bytes)
        .unwrap();
    refresh
        .retain_successor_original(R::SchemePolicy, &bytes)
        .unwrap();
    assert!(refresh.original(R::SchemePolicy).unwrap().is_none());
    assert!(
        refresh
            .retain_successor_original(R::Blacklist, &bytes)
            .is_err()
    );
    assert!(refresh.finish(&state).is_err());
    source.require(&mut store, &state).unwrap();
    let mut refresh = PreparationCustodyV1::new(
        &mut store,
        &source,
        &state,
        K::RefreshPolicy,
        Some(U::SchemePolicy),
        IndexRoot::default(),
        IndexRoot::default(),
    )
    .unwrap();
    refresh
        .retain_successor_original(R::SchemePolicy, &bytes)
        .unwrap();
    let mut next = state;
    next.rest.scheme_policy = policy.scheme_policy_digest();
    next.core.policy_epoch = policy.body.policy_epoch;
    let updated = refresh.finish(&next).unwrap();
    let mut restored = view(&mut store, &next, &updated);
    assert_eq!(restored.original(R::SchemePolicy).unwrap(), Some(bytes));
}

#[test]
fn signed_time_original_cannot_manufacture_direct_clock_custody() {
    let (mut store, mut state, mut source) = setup();
    let mut anchored: KagemushaWalletAnchoredTimeV1 = fixture("KagemushaWalletAnchoredTimeV1");
    anchored.receive_monotonic_ms = anchored.request_monotonic_ms;
    let anchor = anchored.anchor.to_canonical_bytes().unwrap();
    source.originals[R::TimeAnchor.index()] = Some(
        store
            .write_object(&anchor, R::TimeAnchor.maximum())
            .unwrap(),
    );
    state.rest.time_anchor = anchored.anchor.time_anchor_digest();
    let mut custody = view(&mut store, &state, &source);
    assert!(custody.original(R::TimeAnchor).unwrap().is_some());
    assert!(matches!(
        custody.anchored_time(),
        Err(Error::WitnessLost(_))
    ));
    drop(custody);
    let address = store
        .write_object(
            &archive::encode(&anchored).unwrap(),
            KAGEMUSHA_WALLET_TIME_ANCHOR_MAX_BYTES_V1 + 1024,
        )
        .unwrap();
    let anchors = IndexRoot::default()
        .set(&mut store, state.rest.time_anchor, &address)
        .unwrap();
    let mut custody = PreparationCustodyV1::new(
        &mut store,
        &source,
        &state,
        K::Send,
        None,
        IndexRoot::default(),
        anchors,
    )
    .unwrap();
    assert_eq!(custody.anchored_time().unwrap(), Some(anchored));
}

#[test]
fn issued_request_is_exact_local_custody_and_missing_gap_is_not_a_fallback() {
    let vectors: norito::json::Value = norito::json::from_str(include_str!(
        "../../../../../fixtures/kagemusha/wallet_v1_vectors.json"
    ))
    .unwrap();
    let row = vectors["envelopes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["variant"].as_str() == Some("Request"))
        .unwrap();
    let envelope: KagemushaWalletEnvelopeV1 =
        archive::decode(&hex::decode(row["canonical_hex"].as_str().unwrap()).unwrap()).unwrap();
    let KagemushaWalletMessageV1::Request { request } = envelope.message else {
        panic!("Request fixture")
    };
    let state = KagemushaWalletStateV1::bootstrap(
        &request.receiver_credential,
        kagemusha_wallet_field_from_u128_v1(10),
    )
    .unwrap();
    let mut store = MemoryArchive::new();
    let source = SourceCustodyV1::bootstrap(
        &mut store,
        &state,
        &request.receiver_credential.to_canonical_bytes().unwrap(),
        &archive::encode(&request.certificates).unwrap(),
    )
    .unwrap();
    let original = archive::encode(&request).unwrap();
    let key = request.request_digest();
    let record = IssuedRequestCustodyV1 {
        request: store
            .write_object(&original, KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1)
            .unwrap(),
        gap: None,
    };
    let issued = IndexRoot::default()
        .set(&mut store, key, &archive::encode(&record).unwrap())
        .unwrap();
    let mut custody = PreparationCustodyV1::new(
        &mut store,
        &source,
        &state,
        K::Receive,
        None,
        issued,
        IndexRoot::default(),
    )
    .unwrap();
    assert!(matches!(
        custody.issued_request(&[42; 32]),
        Err(Error::WitnessLost(_))
    ));
    if request.body.receiver_blacklist_version == 0 {
        assert_eq!(custody.issued_request(&key).unwrap(), original);
        assert!(custody.issued_request_gap(&key).unwrap().is_none());
    } else {
        assert!(matches!(
            custody.issued_request(&key),
            Err(Error::WitnessLost(_))
        ));
    }
}

#[test]
fn unavailable_original_storage_is_never_absence_or_missing_witness() {
    struct Unavailable;
    impl ObjectStore for Unavailable {
        fn read_object(&mut self, _: &[u8; 32], _: usize) -> Result<Vec<u8>, Error> {
            Err(Error::Storage(std::io::Error::other("unavailable")))
        }
        fn write_object(&mut self, _: &[u8], _: usize) -> Result<[u8; 32], Error> {
            Err(Error::Storage(std::io::Error::other("unavailable")))
        }
    }
    let (_, state, source) = setup();
    assert!(matches!(
        source.require(&mut Unavailable, &state),
        Err(Error::Storage(_))
    ));
    assert!(matches!(
        SourceCustodyV1::bootstrap(&mut Unavailable, &state, &[0], &[0]),
        Err(Error::Invalid(_)) | Err(Error::WitnessLost(_))
    ));
}
