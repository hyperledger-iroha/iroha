// Certified gossip carries one canonical input and preserves physical queue/credit custody.

#[test]
fn certified_gossip_160_kib_fits_default_frame_and_delivers_once_after_publication() {
    for future in [false, true] {
        let label = "x".repeat(160 * 1024);
        let (mut gossiper, signed, binding, input, _journal) =
            publication_queue_plan_gossip_fixture(&label, future);
        let original_payload = payload_for(&signed);
        let config = test_network_config(socket_addr!(127.0.0.1:0));
        let sender_key = KeyPair::try_from_seed(vec![0xE1; 32], Algorithm::BlsNormal)
            .expect("actual BLS-normal relay origin");
        let target_key = KeyPair::try_from_seed(vec![0xE2; 32], Algorithm::BlsNormal)
            .expect("actual BLS-normal relay target");
        let sender = PeerId::new(sender_key.public_key().clone());
        let target = PeerId::new(target_key.public_key().clone());
        let cap = tx_gossip_frame_payload_cap(&config, &sender, &target);
        assert_eq!(config.max_frame_bytes_tx_gossip, 256 * 1024);
        assert!(
            cap > 160 * 1024,
            "authenticated relay identities must leave a usable default payload cap: {cap}"
        );
        assert!(
            cap < config.max_frame_bytes_tx_gossip,
            "real relay framing consumes part of the default cap"
        );
        gossiper.tx_frame_cap = cap;
        assert!(original_payload.len() >= 160 * 1024);
        let sole_input = GossipTransaction::from_queue_plan_admitted_input(Arc::new(input.clone()))
            .expect("the actual 160 KiB complete input decodes within its fixed bounds");
        assert_eq!(sole_input.payload().as_slice(), original_payload.as_slice());
        assert_eq!(sole_input.hash(), signed.hash_as_entrypoint());
        assert!(
            original_payload.len() + input.len() > config.max_frame_bytes_tx_gossip,
            "the former two-body representation necessarily exceeded the default cap"
        );
        let batch = partition_gossip_batch(
            1,
            cap,
            GossipPlane::Public,
            vec![GossipBatchEntry {
                tx: AcceptedTransaction::new_unchecked(Cow::Owned(signed.clone())),
                routing: RoutingDecision::default(),
                routing_plan: default_plan(),
                payload: Arc::clone(&original_payload),
                queue_plan_admission: QueuePlanGossipAdmission::Certified(Arc::new(input.clone())),
            }],
        );
        assert!(
            batch.requeue.is_empty(),
            "sole input {} bytes, entrypoint {} bytes, cap {cap}, accepted frame {} bytes",
            input.len(),
            original_payload.len(),
            batch.encoded_len
        );
        assert_eq!(batch.message.txs.len(), 1);
        assert_eq!(
            batch.message.txs[0].encode().len(),
            1 + input.len(),
            "the wire owns precisely one complete input plus its explicit tag"
        );
        assert_eq!(batch.encoded_len, batch.message.encode().len());
        let payload = NetworkMessage::TransactionGossiper(Arc::new(batch.message.clone()));
        for peer in [None, Some(&target)] {
            let bytes = iroha_p2p::network::data_frame_wire_len(&sender, peer, &payload);
            let materialized = iroha_p2p::network::materialized_signed_data_frame_len_for_test(
                &sender_key,
                peer.cloned(),
                payload.clone(),
            )
            .expect("encode and verify the actual signed relay/Data envelope");
            assert_eq!(
                bytes, materialized,
                "accounting must equal the real authenticated frame"
            );
            assert!(bytes <= config.max_frame_bytes_tx_gossip);
            assert!(bytes <= iroha_p2p::frame_plaintext_cap(config.max_frame_bytes));
        }
        let message = Arc::new(decode_gossip_message(&batch.message));
        let (retained, count) = RetainedGossip::with_count_for_test(Arc::clone(&message));
        let result = gossiper.handle_retained_gossip(
            retained,
            GossipProgress::default(),
            Some(tokio::time::Instant::now() + gossiper.queue.tx_time_to_live),
        );
        if future {
            let pending = result.expect("real future certificate retains sole delivery");
            assert_eq!(count.available_permits(), 0);
            assert_eq!(gossiper.queue.queued_len(), 0);
            assert_eq!(pending.required_height, 1);
            let carrier = gossip_publication_block(None);
            gossiper
                .state
                .kura()
                .store_block(Arc::new(carrier.clone()))
                .unwrap();
            gossiper
                .state
                .append_committed_block_header_for_tests(carrier.header());
            assert!(retry_publication_gossip(&gossiper, pending).is_none());
        } else {
            assert!(result.is_none());
        }
        assert_eq!(count.available_permits(), 1);
        assert_exact_publication_body(&gossiper, signed.clone(), binding.clone(), &input);
        let (repeat, repeated_count) = RetainedGossip::with_count_for_test(message);
        assert!(
            gossiper
                .handle_retained_gossip(
                    repeat,
                    GossipProgress::default(),
                    Some(tokio::time::Instant::now() + gossiper.queue.tx_time_to_live)
                )
                .is_none()
        );
        assert_eq!(repeated_count.available_permits(), 1);
        assert_exact_publication_body(&gossiper, signed.clone(), binding.clone(), &input);

        // Select from the actual retained queue, fail the exact byte budget, and
        // exercise the real deferred-ring handoff before selecting it again.
        let selected = gossiper.queue.gossip_batch_with_state(1, &gossiper.state);
        assert_eq!(selected.len(), 1);
        let too_small =
            partition_gossip_batch(1, batch.encoded_len - 1, GossipPlane::Public, selected);
        assert!(too_small.message.txs.is_empty());
        assert_eq!(too_small.requeue, vec![signed.hash_as_entrypoint()]);
        gossiper.defer_gossip_hashes(too_small.requeue);
        assert_eq!(gossiper.queue.queued_len(), 1);
        gossiper.advance_gossip_tick();
        gossiper.release_deferred_gossip();
        let restored = gossiper.queue.gossip_batch_with_state(1, &gossiper.state);
        assert_eq!(restored.len(), 1);
        let retry = partition_gossip_batch(1, cap, GossipPlane::Public, restored);
        assert!(retry.requeue.is_empty());
        assert_eq!(
            retry.message.txs[0].queue_plan_admitted_input(),
            Some(input.as_slice())
        );
        assert_exact_publication_body(&gossiper, signed, binding, &input);
    }
}

#[test]
fn certified_gossip_rejects_body_substitution_and_keeps_source_on_partition_mismatch() {
    let (gossiper, signed, _, input, _journal) =
        exact_pending_queue_plan_gossip_fixture("sole input owner");
    let mut substituted = norito::decode_canonical::<
        iroha_data_model::block::lane_admission::LaneAdmittedInputV1,
    >(&input)
    .unwrap();
    let different = TransactionBuilder::new(
        test_network_id(),
        (*ALICE_ID).clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(Level::INFO, "different signed body".to_owned())])
    .with_admission_intent(TransactionAdmissionIntent::QueuePlanSynced)
    .sign(ALICE_KEYPAIR.private_key());
    substituted.entrypoint = different.into();
    let substituted = Arc::new(norito::encode_canonical(&substituted).unwrap());
    let tx = GossipTransaction::from_queue_plan_admitted_input(Arc::clone(&substituted)).unwrap();
    let message = TransactionGossip {
        txs: vec![tx],
        routes: vec![GossipRoute {
            lane_id: LaneId::SINGLE,
            dataspace_id: DataSpaceId::UNIVERSAL,
        }],
        plans: vec![default_plan()],
        plane: GossipPlane::Public,
    };
    let (retained, count) =
        RetainedGossip::with_count_for_test(Arc::new(decode_gossip_message(&message)));
    assert!(
        gossiper
            .handle_retained_gossip(
                retained,
                GossipProgress::default(),
                Some(tokio::time::Instant::now() + gossiper.queue.tx_time_to_live),
            )
            .is_none()
    );
    assert_eq!(count.available_permits(), 1);
    assert_eq!(gossiper.queue.queued_len(), 0);
    assert!(
        gossiper
            .state
            .kura()
            .pending_queue_plan_admission_certificates()
            .unwrap()
            .is_empty()
    );
    let mismatch = partition_gossip_batch(
        1,
        usize::MAX,
        GossipPlane::Public,
        vec![GossipBatchEntry {
            tx: AcceptedTransaction::new_unchecked(Cow::Owned(signed.clone())),
            routing: RoutingDecision::default(),
            routing_plan: default_plan(),
            payload: payload_for(&signed),
            queue_plan_admission: QueuePlanGossipAdmission::Certified(substituted),
        }],
    );
    assert!(mismatch.message.txs.is_empty());
    assert_eq!(mismatch.requeue, vec![signed.hash_as_entrypoint()]);
    let mut proof_changed = norito::decode_canonical::<
        iroha_data_model::block::lane_admission::LaneAdmittedInputV1,
    >(&input)
    .unwrap();
    let mut same_intent = signed.clone();
    corrupt_signature(&mut same_intent);
    assert_eq!(
        same_intent.hash(),
        signed.hash(),
        "authorization proof is outside semantic transaction ID"
    );
    proof_changed.entrypoint = same_intent.into();
    let changed_bytes = Arc::new(norito::encode_canonical(&proof_changed).unwrap());
    let changed = partition_gossip_batch(
        1,
        usize::MAX,
        GossipPlane::Public,
        vec![GossipBatchEntry {
            tx: AcceptedTransaction::new_unchecked(Cow::Owned(signed.clone())),
            routing: RoutingDecision::default(),
            routing_plan: default_plan(),
            payload: payload_for(&signed),
            queue_plan_admission: QueuePlanGossipAdmission::Certified(changed_bytes),
        }],
    );
    assert!(
        changed.message.txs.is_empty(),
        "same intent cannot substitute different signed bytes"
    );
    assert_eq!(changed.requeue, vec![signed.hash_as_entrypoint()]);
}

#[test]
fn gossip_item_has_one_explicit_schema_and_rejects_legacy_pair_or_wrong_tag() {
    let (_gossiper, signed, _, input, _journal) =
        exact_pending_queue_plan_gossip_fixture("one wire schema");
    let item = GossipTransaction::from_queue_plan_admitted_input(Arc::new(input.clone())).unwrap();
    let encoded = item.encode();
    assert_eq!(encoded[0], GossipTransactionKind::CertifiedInput as u8);
    assert_eq!(&encoded[1..], input.as_slice());
    let decoded = ncore::decode_field_canonical::<GossipTransaction>(&encoded)
        .unwrap()
        .0;
    assert_eq!(decoded.hash(), signed.hash_as_entrypoint());
    assert_eq!(
        decoded.payload().as_slice(),
        payload_for(&signed).as_slice()
    );
    let mut old_pair = payload_for(&signed).as_ref().clone();
    ncore::write_len_prefixed(&mut ncore::Encoder::new(&mut old_pair), &Some(input)).unwrap();
    assert!(ncore::decode_field_canonical::<GossipTransaction>(&old_pair).is_err());
    for tag in [GossipTransactionKind::Ordinary as u8, 2, u8::MAX] {
        let mut wrong = encoded.clone();
        wrong[0] = tag;
        assert!(ncore::decode_field_canonical::<GossipTransaction>(&wrong).is_err());
    }
    let mut trailing = encoded;
    trailing.push(0);
    assert!(ncore::decode_field_canonical::<GossipTransaction>(&trailing).is_err());
}

// Real signed wire measurements guard the cap counter and its canonical framing.
fn gossip_boundary_message(transaction: GossipTransaction) -> TransactionGossip {
    TransactionGossip {
        txs: vec![transaction],
        routes: vec![GossipRoute {
            lane_id: LaneId::SINGLE,
            dataspace_id: DataSpaceId::UNIVERSAL,
        }],
        plans: vec![default_plan()],
        plane: GossipPlane::Public,
    }
}

fn gossip_boundary_relay_keys() -> (KeyPair, PeerId) {
    let sender = KeyPair::try_from_seed(vec![0xE5; 32], Algorithm::BlsNormal).unwrap();
    let target = KeyPair::try_from_seed(vec![0xE6; 32], Algorithm::BlsNormal).unwrap();
    (sender, PeerId::new(target.public_key().clone()))
}

fn assert_exact_gossip_frame_boundary(
    message: &TransactionGossip,
    config: &NetworkConfig,
    sender_key: &KeyPair,
    target: &PeerId,
) {
    let sender = PeerId::new(sender_key.public_key().clone());
    let gossip_len = message.encode().len();
    assert_eq!(
        gossip_len,
        tx_gossip_frame_payload_cap(config, &sender, target)
    );
    let payload = NetworkMessage::TransactionGossiper(Arc::new(message.clone()));
    for peer in [None, Some(target)] {
        let materialized = iroha_p2p::network::materialized_signed_data_frame_len_for_test(
            sender_key,
            peer.cloned(),
            payload.clone(),
        )
        .expect("the real direct/broadcast envelope must sign, encode and authenticate");
        assert_eq!(
            Some(materialized),
            tx_gossip_data_frame_len(&sender, peer, gossip_len),
            "count NetworkMessage and P2P framing exactly, including compact prefixes"
        );
        assert_eq!(
            materialized,
            iroha_p2p::network::data_frame_wire_len(&sender, peer, &payload)
        );
        assert!(materialized <= config.max_frame_bytes_tx_gossip);
        assert!(materialized <= iroha_p2p::frame_plaintext_cap(config.max_frame_bytes));
        if peer.is_some() {
            assert_eq!(materialized, config.max_frame_bytes_tx_gossip);
        }
    }
    let mut below = config.clone();
    below.max_frame_bytes_tx_gossip -= 1;
    assert_eq!(
        tx_gossip_frame_payload_cap(&below, &sender, target),
        gossip_len - 1,
        "a one-byte-smaller topic cap must exclude this exact signed frame"
    );
    assert!(
        tx_gossip_data_frame_len(&sender, Some(target), gossip_len + 1).unwrap()
            > config.max_frame_bytes_tx_gossip,
        "the accepted inner budget must be maximal"
    );
    // Independently make the encrypted frame limit the binding constraint.
    let encryption_overhead = 1024 - iroha_p2p::frame_plaintext_cap(1024);
    let mut encrypted = config.clone();
    encrypted.max_frame_bytes_tx_gossip += 1;
    encrypted.max_frame_bytes = config.max_frame_bytes_tx_gossip + encryption_overhead;
    assert_eq!(
        iroha_p2p::frame_plaintext_cap(encrypted.max_frame_bytes),
        config.max_frame_bytes_tx_gossip
    );
    assert_eq!(
        tx_gossip_frame_payload_cap(&encrypted, &sender, target),
        gossip_len
    );
    encrypted.max_frame_bytes -= 1;
    assert_eq!(
        tx_gossip_frame_payload_cap(&encrypted, &sender, target),
        gossip_len - 1
    );
}

#[test]
fn ordinary_gossip_exact_frame_boundary_preserves_original_body_on_requeue() {
    let config = test_network_config(socket_addr!(127.0.0.1:0));
    assert_eq!(config.max_frame_bytes_tx_gossip, 256 * 1024);
    let (sender_key, target) = gossip_boundary_relay_keys();
    let sender = PeerId::new(sender_key.public_key().clone());
    let cap = tx_gossip_frame_payload_cap(&config, &sender, &target);
    let initial_label_len = 160 * 1024;
    let (initial, _) = build_transaction(&"x".repeat(initial_label_len));
    let initial_len = gossip_boundary_message(initial.into()).encode().len();
    let label_len = initial_label_len + cap.checked_sub(initial_len).unwrap();
    let (signed, accepted) = build_transaction(&"x".repeat(label_len));
    signed.verify_signature().unwrap();
    let original_payload = payload_for(&signed);
    let message = gossip_boundary_message(signed.clone().into());
    assert_exact_gossip_frame_boundary(&message, &config, &sender_key, &target);

    let mut gossiper = closed_test_gossiper(NonZeroU32::new(1).unwrap());
    gossiper
        .queue
        .push(accepted, gossiper.state.view())
        .unwrap();
    let selected = gossiper.queue.gossip_batch_with_state(1, &gossiper.state);
    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].payload.as_slice(), original_payload.as_slice());
    let mut below = config.clone();
    below.max_frame_bytes_tx_gossip -= 1;
    let refused = partition_gossip_batch(
        1,
        tx_gossip_frame_payload_cap(&below, &sender, &target),
        GossipPlane::Public,
        selected,
    );
    assert!(refused.message.txs.is_empty());
    assert_eq!(refused.requeue, vec![signed.hash_as_entrypoint()]);
    gossiper.defer_gossip_hashes(refused.requeue);
    assert_eq!(gossiper.queue.queued_len(), 1);
    gossiper.advance_gossip_tick();
    gossiper.release_deferred_gossip();
    let restored = gossiper.queue.gossip_batch_with_state(1, &gossiper.state);
    assert_eq!(restored.len(), 1);
    assert_eq!(restored[0].payload.as_slice(), original_payload.as_slice());
    let retry = partition_gossip_batch(1, cap, GossipPlane::Public, restored);
    assert!(retry.requeue.is_empty());
    assert_eq!(retry.message.encode(), message.encode());
    assert_eq!(retry.message.txs[0].hash(), signed.hash_as_entrypoint());
    assert_eq!(
        retry.message.txs[0].payload().as_slice(),
        original_payload.as_slice()
    );
    assert_eq!(gossiper.queue.queued_len(), 1);
}

#[test]
fn certified_gossip_exact_frame_boundary_preserves_original_body_on_requeue() {
    let config = test_network_config(socket_addr!(127.0.0.1:0));
    assert_eq!(config.max_frame_bytes_tx_gossip, 256 * 1024);
    let (sender_key, target) = gossip_boundary_relay_keys();
    let sender = PeerId::new(sender_key.public_key().clone());
    let cap = tx_gossip_frame_payload_cap(&config, &sender, &target);
    let initial_label_len = 160 * 1024;
    let initial_len = {
        let (_gossiper, _signed, _binding, input, _journal) =
            publication_queue_plan_gossip_fixture(&"x".repeat(initial_label_len), false);
        let item = GossipTransaction::from_queue_plan_admitted_input(Arc::new(input)).unwrap();
        gossip_boundary_message(item).encode().len()
    };
    let label_len = initial_label_len + cap.checked_sub(initial_len).unwrap();
    let (mut gossiper, signed, binding, input, _journal) =
        publication_queue_plan_gossip_fixture(&"x".repeat(label_len), false);
    let original_payload = payload_for(&signed);
    let item = GossipTransaction::from_queue_plan_admitted_input(Arc::new(input.clone())).unwrap();
    let message = gossip_boundary_message(item);
    assert_exact_gossip_frame_boundary(&message, &config, &sender_key, &target);
    assert_eq!(message.txs[0].encode().len(), 1 + input.len());
    let (retained, credit) =
        RetainedGossip::with_count_for_test(Arc::new(decode_gossip_message(&message)));
    assert!(
        gossiper
            .handle_retained_gossip(
                retained,
                GossipProgress::default(),
                Some(tokio::time::Instant::now() + gossiper.queue.tx_time_to_live),
            )
            .is_none()
    );
    assert_eq!(credit.available_permits(), 1);
    assert_exact_publication_body(&gossiper, signed.clone(), binding.clone(), &input);

    let selected = gossiper.queue.gossip_batch_with_state(1, &gossiper.state);
    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].payload.as_slice(), original_payload.as_slice());
    let mut below = config.clone();
    below.max_frame_bytes_tx_gossip -= 1;
    let refused = partition_gossip_batch(
        1,
        tx_gossip_frame_payload_cap(&below, &sender, &target),
        GossipPlane::Public,
        selected,
    );
    assert!(refused.message.txs.is_empty());
    assert_eq!(refused.requeue, vec![signed.hash_as_entrypoint()]);
    gossiper.defer_gossip_hashes(refused.requeue);
    assert_exact_publication_body(&gossiper, signed.clone(), binding.clone(), &input);
    gossiper.advance_gossip_tick();
    gossiper.release_deferred_gossip();
    let restored = gossiper.queue.gossip_batch_with_state(1, &gossiper.state);
    assert_eq!(restored.len(), 1);
    assert_eq!(restored[0].payload.as_slice(), original_payload.as_slice());
    let retry = partition_gossip_batch(1, cap, GossipPlane::Public, restored);
    assert!(retry.requeue.is_empty());
    assert_eq!(retry.message.encode(), message.encode());
    assert_eq!(
        retry.message.txs[0].queue_plan_admitted_input(),
        Some(input.as_slice())
    );
    assert_eq!(
        retry.message.txs[0].payload().as_slice(),
        original_payload.as_slice()
    );
    assert_exact_publication_body(&gossiper, signed, binding, &input);
}
