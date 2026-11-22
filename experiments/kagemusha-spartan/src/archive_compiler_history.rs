//! Ordinary monetary witnesses for iterative compiler passes. Resolve A's exact
//! accepted Bootstrap and Load through the selected Send's complete predecessor
//! chain. Reconstruct B's genesis through the same isolated enrollment fixture.
use super::*;
use crate::{chain_fixture::Chain, full::WalletKey};
use std::path::Path;

pub(super) fn signed(
    prepared: &PreparedOperation,
    released: &CompletedOperation,
) -> native::ExactOperationWitness {
    released.released_core(prepared).unwrap();
    let mut exact = prepared.witness.as_ref().clone();
    let receipt = native::engine::receipt_mut(&mut exact);
    let bytes = released.completion().exact_receipt();
    assert_eq!(
        receipt.body.as_slice(),
        &bytes[..crate::receipt::BODY_BYTES]
    );
    receipt
        .signature
        .copy_from_slice(&bytes[crate::receipt::BODY_BYTES..]);
    exact
}

fn parent(
    child: &PreparedOperation,
    originals: &Path,
    engine: &NativeEngine,
    params: &native::AuthenticatedParameters,
) -> (PreparedOperation, CompletedOperation) {
    let scheme = native::device_fixture::scheme().scheme_id();
    let mut selected = None;
    for path in originals
        .read_dir()
        .unwrap()
        .map(|p| p.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "cap"))
    {
        let bytes = std::fs::read(&path).unwrap();
        if native::RecoveryCapsule::frame_digest(&bytes)
            != *child.capsule().predecessor_capsule_digest()
        {
            continue;
        }
        let cap = native::RecoveryCapsule::decode(&bytes, &scheme).unwrap();
        let record = native::CompletionRecord::decode(
            &std::fs::read(path.with_extension("cr")).unwrap(),
            cap.wallet(),
        )
        .unwrap();
        let actual = engine
            .verify_released_operation(&cap, &record, params)
            .unwrap();
        assert_eq!(child.predecessor.profile(), actual.1.head().profile());
        assert_eq!(child.predecessor.exact_pair(), actual.1.head().exact_pair());
        assert!(
            selected.replace(actual).is_none(),
            "exactly one retained parent"
        );
    }
    selected.expect("authenticated predecessor original")
}

pub(super) fn chain(
    send: &PreparedOperation,
    sent: &CompletedOperation,
    engine: &NativeEngine,
    params: &native::AuthenticatedParameters,
) -> Chain {
    let root = std::env::var_os("KAGEMUSHA_SPARTAN_ARCHIVE_RAW_ORIGINALS").unwrap();
    let (load, loaded) = parent(send, Path::new(&root), engine, params);
    let (bootstrap, booted) = parent(&load, Path::new(&root), engine, params);
    let signed_boot = signed(&bootstrap, &booted);
    let signed_load = signed(&load, &loaded);
    let signed_send = signed(send, sent);
    let native::ExactOperationWitness::Bootstrap(sender) = &signed_boot else {
        panic!("A Bootstrap")
    };
    let native::ExactOperationWitness::Load { step, .. } = &signed_load else {
        panic!("A Load")
    };
    let native::ExactOperationWitness::Send(sent_witness) = &signed_send else {
        panic!("A Send")
    };
    let counterparty =
        native::device_fixture::Counterparty::new(params, send.credential()).unwrap();
    let origin = counterparty.origin(engine, params).unwrap();
    let prepared = engine
        .prepare(counterparty.bootstrap(), &origin, params)
        .unwrap();
    let signed_receiver = counterparty.sign(&prepared).unwrap();
    let native::ExactOperationWitness::Bootstrap(b) = prepared.witness.as_ref() else {
        panic!("B Bootstrap")
    };
    let mut receiver = b.input.clone();
    receiver.receipt = signed_receiver.receipt().clone();
    let receiver_key = WalletKey::labelled("isolated fixture counterparty wallet B");
    let d = num_bigint::BigUint::from_bytes_be(&[0x5a; 32]);
    let sender_key = WalletKey {
        q: crate::p256::mul(&crate::p256::generator(), &d).unwrap(),
        d,
    };
    assert_eq!(
        receiver.credential.body,
        sent_witness.request_originals.receiver.body
    );
    assert_eq!(sender.input.credential.body, send.credential().body);
    let chain = Chain {
        sender_context: params.inner.context(),
        receiver_context: params.inner.context(),
        sender_key,
        receiver_key,
        sender_bootstrap: sender.input.clone(),
        receiver_bootstrap: receiver,
        load: *step.clone(),
        send: *sent_witness.clone(),
        after_load_public: *loaded.head().profile(),
        after_send_public: *sent.head().profile(),
    };
    assert_eq!(
        crate::bootstrap::public_profile(&chain.sender_context, &chain.sender_bootstrap),
        *booted.head().profile()
    );
    chain
}

#[test]
#[ignore = "requires the completed signed software wallet originals"]
fn resolves_actual_five_kind_sender_history_and_receiver_bootstrap() {
    with_retained_inputs(|_, send, sent, engine, params, _| {
        let chain = chain(send, sent, engine, params);
        assert_eq!(chain.after_send_public, *sent.head().profile());
        assert_eq!(chain.after_load_public, *send.predecessor.profile());
        assert_eq!(chain.send.before_core, chain.load.after_core);
        assert_eq!(
            chain.receiver_bootstrap.credential.body,
            chain.send.request_originals.receiver.body
        );
        let public =
            crate::bootstrap::public_profile(&chain.receiver_context, &chain.receiver_bootstrap);
        assert_eq!(
            public[full::RECEIPT_IO],
            crate::receipt::digest(&chain.receiver_bootstrap.receipt)
        );
    });
}
