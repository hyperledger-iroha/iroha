//! Replay a retained host fixture through the provider's actual custody formats and native verifier.
use std::{
    fs,
    path::{Path, PathBuf},
};

use ff::PrimeField;
use iroha_data_model::{
    kagemusha::{KagemushaWalletIndexedTreeV1, KagemushaWalletSchemeV1},
    sumeragi_finality::{SumeragiCommitVerifierV1, test_fixtures::NativeFinalityFixture},
};
use zk_spartan::native::{
    self, AuthenticatedParameters, CompletionRecord, Engine as _, NativeEngine, ParameterManifest,
    RecoveryCapsule, VerifiedCreditStatus,
};

use super::framed_payload;
use crate::kagemusha_wallet_advance_v1::{
    capsule::{
        KAGEMUSHA_WALLET_FROZEN_FILE_VERSION_V1, KagemushaWalletFrozenFileV1,
        KagemushaWalletFrozenFrameV1, file_max,
    },
    decode_envelope_v1,
};

fn fixture_path(key: &str) -> PathBuf {
    std::env::var_os(key).map(PathBuf::from).expect(key)
}

fn files(dir: &Path) -> impl Iterator<Item = PathBuf> {
    fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
}

fn envelope<T: KagemushaWalletFrozenFrameV1>(path: &Path) -> KagemushaWalletFrozenFileV1 {
    let file: KagemushaWalletFrozenFileV1 =
        decode_envelope_v1(&fs::read(path).unwrap(), file_max::<T>()).unwrap();
    assert_eq!(file.version, KAGEMUSHA_WALLET_FROZEN_FILE_VERSION_V1);
    file
}

#[test]
#[ignore = "requires retained host custody originals and the signed device fixture bundle and scheme"]
fn verifies_retained_sent_delivery_against_the_later_payer_head() {
    let wallet = fixture_path("KAGEMUSHA_SPARTAN_DEVICE_ROOT");
    let bundle = fixture_path("KAGEMUSHA_SPARTAN_DEVICE_ARTIFACTS");
    let scheme_path = fixture_path("KAGEMUSHA_SPARTAN_REGISTERED_SCHEME");
    let read_bundle = |name: &str| fs::read(bundle.join(name)).unwrap();
    let manifest = read_bundle("parameter-manifest.norito");
    let decoded: ParameterManifest = norito::decode_from_bytes(&manifest).unwrap();
    let scheme =
        KagemushaWalletSchemeV1::decode_canonical(&fs::read(scheme_path).unwrap(), &decoded.scheme)
            .unwrap();
    let finality = SumeragiCommitVerifierV1::new(
        &NativeFinalityFixture::new_with_explicit_parameters().verifier(),
    )
    .unwrap();
    let parameters = AuthenticatedParameters::install(
        &read_bundle("program-bundle.norito"),
        &manifest,
        read_bundle("parameter-signature.bin").try_into().unwrap(),
        &scheme,
        finality,
    )
    .unwrap();
    let engine = NativeEngine::default();
    let slots: Vec<_> = files(&wallet.join("custody/slots")).collect();
    let [slot] = slots.as_slice() else {
        panic!("expected one primary fixture slot");
    };
    let operations: Vec<_> = files(&slot.join("capsules"))
        .filter(|path| path.extension().is_some_and(|ext| ext == "cap"))
        .map(|path| {
            let cap_file = envelope::<RecoveryCapsule>(&path);
            let cap = RecoveryCapsule::decode(&cap_file.frame, &scheme.scheme_id()).unwrap();
            let record_path = slot
                .join("completion")
                .join(format!("{}.cr", hex::encode(cap.operation_id())));
            let record_file = envelope::<CompletionRecord>(&record_path);
            assert_eq!(
                cap_file.selected_generation,
                record_file.selected_generation
            );
            let record = CompletionRecord::decode(&record_file.frame, cap.wallet()).unwrap();
            assert_eq!(
                record.capsule_digest(),
                &RecoveryCapsule::frame_digest(&cap_file.frame)
            );
            (cap, record)
        })
        .collect();
    let released = |kind| {
        let (cap, record) = operations
            .iter()
            .find(|(cap, _)| cap.kind() == kind)
            .unwrap();
        engine
            .verify_released_operation(cap, record, &parameters)
            .unwrap()
    };
    let (send, sent) = released(3);
    let payment_bytes = fs::read(wallet.join("complete-payment.bin")).unwrap();
    let payment = engine.verify_payment(&payment_bytes, &parameters).unwrap();
    let request = files(&slot.join("archive"))
        .filter(|p| p.extension().is_some_and(|ext| ext == "rec"))
        .find_map(|p| {
            let record = fs::read(p).unwrap();
            let original = framed_payload(&record).unwrap();
            original
                .starts_with(b"IPM1")
                .then(|| native::decode_open_request_peer(original).ok())
                .flatten()
                .filter(|open| open.request.ends_with(&payment.request().signature))
                .map(|_| original.to_vec())
        })
        .expect("recipient-signed Request retained in the payer archive");
    let status = fs::read(wallet.join("outgoing-credit-status.bin")).unwrap();
    let evidence = engine
        .verify_sent_delivery(&send, &sent, &request, &status, &parameters)
        .unwrap();
    assert_eq!(evidence.outcome(), VerifiedCreditStatus::Credited);
    assert_eq!(evidence.operation_id(), send.capsule().operation_id());
    assert_eq!(evidence.payment().exact_bytes(), payment_bytes);
    assert_eq!(evidence.open_request(), request);
    assert_eq!(evidence.credit_status(), status);

    let fields = payment.statement();
    let key = fields[17].to_repr();
    let descriptor =
        iroha_pasta::poseidon::hash_with_domain(u64::from_le_bytes(*b"kgwpout1"), &fields[17..24])
            .to_repr();
    let mut pending = KagemushaWalletIndexedTreeV1::new();
    pending.insert(key, descriptor).unwrap();
    let removal = pending.remove(&key).unwrap();
    assert_eq!(
        evidence.verify_pending_removal(&sent, &removal).unwrap(),
        pending.root()
    );
    let (receive, later) = released(4);
    assert!(receive.capsule().sequence() > send.capsule().sequence());
    assert_eq!(
        evidence.verify_pending_removal(&later, &removal).unwrap(),
        pending.root()
    );
    assert!(
        engine
            .verify_sent_delivery(&receive, &later, &request, &status, &parameters)
            .is_err()
    );
    assert!(
        engine
            .verify_sent_delivery(&send, &later, &request, &status, &parameters)
            .is_err()
    );
    let (_, before_send) = released(2);
    assert!(
        evidence
            .verify_pending_removal(&before_send, &removal)
            .is_err()
    );
}
