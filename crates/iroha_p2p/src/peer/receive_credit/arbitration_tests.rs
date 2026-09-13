// Native controls included inside negotiation::tests: real pools and seven-byte
// duplex I/O, with the same central arbitration owner. No node is started.
fn authenticated_candidate(
    peer: &iroha_data_model::peer::Peer,
    id: ConnectionId,
    session: [u8; 32],
) -> (
    message::Authenticated,
    oneshot::Receiver<crate::peer::tenure::ReaderPermit>,
    tokio::sync::watch::Receiver<bool>,
) {
    let (cancel, cancelled) = tokio::sync::watch::channel(false);
    let (reply, permission) = oneshot::channel();
    (
        message::Authenticated {
            peer: peer.clone(),
            connection_id: id,
            session,
            relay_role: crate::RelayRole::Disabled,
            cancel,
            reply,
        },
        permission,
        cancelled,
    )
}

async fn observe_geometry(
    transport: OwnedTransport,
    cipher: crate::peer::cryptographer::Cryptographer<iroha_crypto::encryption::ChaCha20Poly1305>,
    network: iroha_data_model::NetworkId,
    peers: (iroha_model_base::peer::PeerId, iroha_model_base::peer::PeerId),
    seen: oneshot::Sender<([u8; 32], [u8; 32])>,
    finish: oneshot::Receiver<()>,
) {
    let negotiated = exchange(transport, &cipher, &network, &peers.0, &peers.1, [8; 32])
        .await
        .unwrap();
    seen.send((negotiated.binding.incoming, negotiated.binding.outgoing))
        .unwrap();
    finish.await.unwrap();
    drop(negotiated);
}

#[tokio::test]
async fn crossed_authenticated_candidates_complete_same_geometry_after_exact_reader_handoff() {
    use crate::network::reader_arbitration_fixture::Owner;
    use crate::peer::cryptographer::Cryptographer;
    use futures::FutureExt;
    use iroha_crypto::encryption::ChaCha20Poly1305;
    let a = peer(61);
    let b = peer(62);
    let pa = pool(6);
    let pb = pool(6);
    let mut ca = Owner::default();
    let mut cb = Owner::default();
    let first = Cryptographer::<ChaCha20Poly1305>::new_with_raw_key_bytes(&[61; 32]).unwrap();
    let second = Cryptographer::<ChaCha20Poly1305>::new_with_raw_key_bytes(&[62; 32]).unwrap();
    let (low, high) = if first.session_binding < second.session_binding {
        (first, second)
    } else {
        (second, first)
    };
    assert!(low.session_binding < high.session_binding);
    let (a_low, b_low) = tokio::io::duplex(7);
    let (a_high, b_high) = tokio::io::duplex(7);
    let (candidate, mut permission, a_cancel) = authenticated_candidate(&b, 1, low.session_binding);
    assert!(ca.admit(candidate));
    let a_permit = permission.try_recv().unwrap();
    let (ar, aw) = tokio::io::split(a_low);
    let old_transport = OwnedTransport {
        read: Box::new(ar),
        write: Box::new(aw),
        source: pa.bind(b.id()).unwrap(),
    };
    let mut old_reader = Box::pin(a_permit.run(async move {
        let _physical_owner = old_transport;
        std::future::pending::<()>().await;
    }));
    assert!(old_reader.as_mut().now_or_never().is_none());
    let (candidate, mut b_permission, _) = authenticated_candidate(&a, 2, high.session_binding);
    assert!(cb.admit(candidate));
    let b_permit = b_permission.try_recv().unwrap();
    let (candidate, mut a_permission, _) = authenticated_candidate(&b, 2, high.session_binding);
    assert!(ca.admit(candidate));
    assert!(*a_cancel.borrow());
    assert!(a_permission.try_recv().is_err());
    assert!(
        pa.bind(b.id()).is_err(),
        "old parser still physically owns the reader"
    );
    let (candidate, mut refused, b_cancel) = authenticated_candidate(&a, 1, low.session_binding);
    assert!(!cb.admit(candidate));
    assert!(*b_cancel.borrow());
    assert!(refused.try_recv().is_err());
    drop(b_low);
    drop(old_reader);
    ca.drain();
    let a_permit = a_permission
        .try_recv()
        .expect("release permits the common higher session");
    let network = test_network_id("central crossed authenticated geometry");
    let (ar, aw) = tokio::io::split(a_high);
    let (br, bw) = tokio::io::split(b_high);
    let left = OwnedTransport {
        read: Box::new(ar),
        write: Box::new(aw),
        source: pa.bind(b.id()).unwrap(),
    };
    let right = OwnedTransport {
        read: Box::new(br),
        write: Box::new(bw),
        source: pb.bind(a.id()).unwrap(),
    };
    let (seen_a, observe_a) = oneshot::channel();
    let (seen_b, observe_b) = oneshot::channel();
    let (release_a, finish_a) = oneshot::channel();
    let (release_b, finish_b) = oneshot::channel();
    let left_task = tokio::spawn(a_permit.run(observe_geometry(
        left, high.clone(), network, (a.id().clone(), b.id().clone()), seen_a, finish_a,
    )));
    let right_task = tokio::spawn(b_permit.run(observe_geometry(
        right, high.clone(), network, (b.id().clone(), a.id().clone()), seen_b, finish_b,
    )));
    let (left, right) = tokio::time::timeout(Duration::from_secs(2), async {
        tokio::try_join!(observe_a, observe_b)
    })
    .await
    .unwrap()
    .unwrap();
    assert_eq!(left.0, right.1);
    assert_eq!(left.1, right.0);
    assert!(ca.claim(2, b.id(), high.disambiguator));
    assert!(cb.claim(2, a.id(), high.disambiguator));
    assert!(pa.bind(b.id()).is_err() && pb.bind(a.id()).is_err());
    release_a.send(()).unwrap();
    release_b.send(()).unwrap();
    left_task.await.unwrap();
    right_task.await.unwrap();
    ca.drain();
    cb.drain();
    assert!(pa.bind(b.id()).is_ok() && pb.bind(a.id()).is_ok());
}

#[tokio::test]
async fn replacement_reuses_peer_partition_while_old_delivered_guard_keeps_exact_count() {
    use crate::network::{
        admission_class_tests::AdmissionFixture as Fixture, reader_arbitration_fixture::Owner,
    };
    use futures::FutureExt;
    let peer = peer(63);
    let pool = pool(6);
    let mut owner = Owner::default();
    let (candidate, mut permission, cancelled) = authenticated_candidate(&peer, 1, [1; 32]);
    assert!(owner.admit(candidate));
    let permit = permission.try_recv().unwrap();
    let source = pool.bind(peer.id()).unwrap();
    let partition = Arc::clone(&source.partition);
    assert_eq!(pool.counts[Class::Availability.index()], 1);
    let delivered = source.reserve(Class::Availability, 200).unwrap().delivered(
        peer.clone(),
        Fixture::Availability(1),
        1,
    );
    let (_, _, _, _, _, retained) = delivered.into_parts_with_reply_route();
    let mut physical = Box::pin(permit.run(async move {
        let _source = source;
        std::future::pending::<()>().await;
    }));
    assert!(physical.as_mut().now_or_never().is_none());
    let (candidate, mut permission, _) = authenticated_candidate(&peer, 2, [2; 32]);
    assert!(owner.admit(candidate));
    assert!(*cancelled.borrow());
    assert!(permission.try_recv().is_err());
    drop(physical);
    owner.drain();
    let _permit = permission.try_recv().unwrap();
    let successor = pool.bind(peer.id()).unwrap();
    assert!(
        Arc::ptr_eq(&partition, &successor.partition),
        "same strong PeerId registry owner"
    );
    assert!(
        successor.reserve(Class::Availability, 200).is_none(),
        "old delivered count is not replenished"
    );
    drop(retained);
    assert!(successor.reserve(Class::Availability, 200).is_some());
}
