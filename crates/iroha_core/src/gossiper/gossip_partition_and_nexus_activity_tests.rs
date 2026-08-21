#[test]
fn partition_respects_frame_cap() {
    let (small_signed, small_accepted) = build_transaction("small");
    let (large_signed, large_accepted) = build_transaction(&"x".repeat(512));
    let small_route = GossipRoute {
        lane_id: LaneId::SINGLE,
        dataspace_id: DataSpaceId::UNIVERSAL,
    };
    let large_route = GossipRoute {
        lane_id: LaneId::new(2),
        dataspace_id: DataSpaceId::new(7),
    };
    let small_len = norito::codec::Encode::encode(&TransactionGossip {
        txs: vec![small_signed.clone().into()],
        routes: vec![small_route],
        plans: vec![plan_for_route(small_route)],
        plane: GossipPlane::Public,
    })
    .len();
    let both_len = norito::codec::Encode::encode(&TransactionGossip {
        txs: vec![small_signed.clone().into(), large_signed.clone().into()],
        routes: vec![small_route, large_route],
        plans: vec![plan_for_route(small_route), plan_for_route(large_route)],
        plane: GossipPlane::Public,
    })
    .len();
    let frame_cap = small_len + 8;
    assert!(frame_cap < both_len, "cap should exclude both transactions");
    let partitioned = partition_gossip_batch(
        usize::MAX,
        frame_cap,
        GossipPlane::Public,
        vec![
            GossipBatchEntry {
                tx: small_accepted,
                routing: RoutingDecision::default(),
                routing_plan: default_plan(),
                payload: payload_for(&small_signed),
                queue_plan_admission: QueuePlanGossipAdmission::Ordinary,
            },
            GossipBatchEntry {
                tx: large_accepted,
                routing: RoutingDecision::default(),
                routing_plan: default_plan(),
                payload: payload_for(&large_signed),
                queue_plan_admission: QueuePlanGossipAdmission::Ordinary,
            },
        ],
    );
    let partitioned_hashes: Vec<_> = partitioned
        .message
        .txs
        .iter()
        .map(|tx| tx.as_signed().hash())
        .collect();
    assert_eq!(partitioned_hashes, vec![small_signed.hash()]);
    assert_eq!(
        partitioned.encoded_len,
        norito::codec::Encode::encode(&partitioned.message).len()
    );
    assert_eq!(partitioned.requeue, vec![large_signed.hash_as_entrypoint()]);
}
#[test]
fn transaction_gossip_decode_accepts_canonical_sequence_limit() {
    let limit = transaction_gossip_sequence_limit();
    let (signed, _accepted) = build_transaction("canonical-gossip-count-boundary");
    let route = GossipRoute {
        lane_id: LaneId::SINGLE,
        dataspace_id: DataSpaceId::UNIVERSAL,
    };
    let message = TransactionGossip {
        txs: vec![GossipTransaction::with_encoded(signed.clone(), payload_for(&signed)); limit],
        routes: vec![route; limit],
        plans: vec![plan_for_route(route); limit],
        plane: GossipPlane::Public,
    };
    let decoded = decode_gossip_message(&message);
    assert_eq!(decoded.txs.len(), limit);
    assert_eq!(decoded.routes.len(), limit);
    assert_eq!(decoded.plans.len(), limit);
}
#[test]
fn transaction_gossip_serialize_rejects_count_above_canonical_limit() {
    let limit = transaction_gossip_sequence_limit();
    let (signed, _accepted) = build_transaction("oversized-gossip-encode");
    let message = TransactionGossip {
        txs: vec![GossipTransaction::with_encoded(signed.clone(), payload_for(&signed)); limit + 1],
        routes: Vec::new(),
        plans: Vec::new(),
        plane: GossipPlane::Public,
    };
    let mut payload = Vec::new();
    let error = ncore::serialize_to_buffer(&message, &mut payload)
        .expect_err("an oversized transaction sequence must not be serialized");
    assert!(matches!(
        error,
        ncore::Error::SequenceLengthExceeded {
            length,
            limit: encoded_limit,
        } if length == u64::try_from(limit + 1).expect("test length fits u64")
            && encoded_limit == u64::try_from(limit).expect("test limit fits u64")
    ));
}
#[test]
fn transaction_gossip_decode_rejects_oversized_count_before_sequence_planning() {
    let limit = u64::from(TRANSACTION_GOSSIP_MAX_SIZE.get());
    let declared_count = limit + 1;
    let mut payload = Vec::new();
    ncore::write_len_header_to_vec(
        &mut payload,
        u64::try_from(core::mem::size_of::<u64>()).expect("u64 width fits u64"),
    );
    payload.extend_from_slice(&declared_count.to_le_bytes());
    let error = decode_transaction_gossip_payload(&payload)
        .expect_err("an oversized declared count must fail before element planning");
    assert!(matches!(
        error,
        ncore::Error::SequenceLengthExceeded {
            length,
            limit: encoded_limit,
        } if length == declared_count && encoded_limit == limit
    ));
}
#[test]
fn gossip_roundtrip_preserves_cached_payload() {
    let (signed, _accepted) = build_transaction("cached");
    let payload = payload_for(&signed);
    let message = TransactionGossip {
        txs: vec![GossipTransaction::with_encoded(
            signed.clone(),
            Arc::clone(&payload),
        )],
        routes: vec![GossipRoute {
            lane_id: LaneId::SINGLE,
            dataspace_id: DataSpaceId::UNIVERSAL,
        }],
        plans: vec![default_plan()],
        plane: GossipPlane::Public,
    };
    let encoded = message.encode();
    let decoded: TransactionGossip =
        Decode::decode(&mut encoded.as_slice()).expect("decode gossip");
    assert_eq!(decoded.txs.len(), 1);
    assert_eq!(decoded.routes.len(), 1);
    assert_eq!(decoded.txs[0].as_signed().hash(), signed.hash());
    assert_eq!(decoded.txs[0].encoded.as_slice(), payload.as_slice());
    assert_eq!(decoded.routes[0].lane_id, LaneId::SINGLE);
    assert_eq!(decoded.routes[0].dataspace_id, DataSpaceId::UNIVERSAL);
    assert_eq!(decoded.plane, GossipPlane::Public);
    assert!(decoded.txs[0].queue_plan_certificate().is_none());
    assert_eq!(
        decoded.txs[0].encoded_len_exact(),
        Some(decoded.txs[0].encode().len())
    );
}
#[test]
fn gossip_roundtrip_preserves_queue_plan_certificate_and_exact_length() {
    let (signed, _accepted) = build_transaction("certified-gossip");
    let payload = payload_for(&signed);
    let certificate = vec![0xA5; 257];
    let message = TransactionGossip {
        txs: vec![GossipTransaction::with_encoded_and_queue_plan_certificate(
            signed.clone(),
            Arc::clone(&payload),
            certificate.clone(),
        )],
        routes: vec![GossipRoute {
            lane_id: LaneId::SINGLE,
            dataspace_id: DataSpaceId::UNIVERSAL,
        }],
        plans: vec![default_plan()],
        plane: GossipPlane::Public,
    };
    let encoded = message.encode();
    assert_eq!(message.encoded_len_hint(), Some(encoded.len()));
    assert_eq!(message.encoded_len_exact(), Some(encoded.len()));
    let decoded: TransactionGossip =
        Decode::decode(&mut encoded.as_slice()).expect("decode certified gossip");
    assert_eq!(decoded.txs.len(), 1);
    assert_eq!(decoded.txs[0].as_signed().hash(), signed.hash());
    assert_eq!(
        decoded.txs[0].queue_plan_certificate(),
        Some(certificate.as_slice())
    );
    assert_eq!(decoded.txs[0].encoded.as_slice(), payload.as_slice());
    assert_eq!(decoded.encoded_len_exact(), Some(encoded.len()));
}
#[test]
fn gossip_roundtrip_preserves_sealed_commitment_entrypoint() {
    let entrypoint = build_sealed_commitment_entrypoint();
    let payload = Arc::new(encode_transaction_entrypoint(&entrypoint));
    let expected_hash = entrypoint.hash();
    let message = TransactionGossip {
        txs: vec![GossipTransaction::with_encoded(
            entrypoint,
            Arc::clone(&payload),
        )],
        routes: vec![GossipRoute {
            lane_id: LaneId::SINGLE,
            dataspace_id: DataSpaceId::UNIVERSAL,
        }],
        plans: vec![default_plan()],
        plane: GossipPlane::Public,
    };
    let encoded = message.encode();
    let decoded: TransactionGossip =
        Decode::decode(&mut encoded.as_slice()).expect("decode gossip");
    assert_eq!(decoded.txs.len(), 1);
    assert!(matches!(
        decoded.txs[0].as_entrypoint(),
        TransactionEntrypoint::SealedCommitment(_)
    ));
    assert_eq!(decoded.txs[0].hash(), expected_hash);
    assert_eq!(decoded.txs[0].as_entrypoint().hash(), expected_hash);
    assert_eq!(decoded.txs[0].encoded.as_slice(), payload.as_slice());
}
#[test]
fn partition_gossip_batch_keeps_sealed_commitments() {
    let entrypoint = build_sealed_commitment_entrypoint();
    let accepted = AcceptedTransaction::new_unchecked_entrypoint(Cow::Owned(entrypoint.clone()));
    let payload = Arc::new(encode_transaction_entrypoint(&entrypoint));
    let partitioned = partition_gossip_batch(
        usize::MAX,
        usize::MAX,
        GossipPlane::Public,
        vec![GossipBatchEntry {
            tx: accepted,
            routing: RoutingDecision::default(),
            routing_plan: default_plan(),
            payload,
            queue_plan_admission: QueuePlanGossipAdmission::Ordinary,
        }],
    );
    assert_eq!(partitioned.message.txs.len(), 1);
    assert!(partitioned.requeue.is_empty());
    assert!(matches!(
        partitioned.message.txs[0].as_entrypoint(),
        TransactionEntrypoint::SealedCommitment(_)
    ));
}
#[test]
fn gossip_transaction_len_hints_include_admission_suffix() {
    let (signed, _accepted) = build_transaction("hint");
    let payload = payload_for(&signed);
    let tx = GossipTransaction::with_encoded(signed, Arc::clone(&payload));
    let wire_len = tx.encode().len();
    assert!(
        wire_len > payload.len(),
        "wire includes the admission suffix"
    );
    assert_eq!(
        ncore::NoritoSerialize::encoded_len_hint(&tx),
        Some(wire_len)
    );
    assert_eq!(
        ncore::NoritoSerialize::encoded_len_exact(&tx),
        Some(wire_len)
    );
}
#[test]
fn partition_withholds_awaiting_certificate_and_counts_certified_wire() {
    let (signed, accepted) = build_transaction("awaiting-certificate");
    let awaiting = GossipBatchEntry {
        tx: accepted.clone(),
        routing: RoutingDecision::default(),
        routing_plan: default_plan(),
        payload: payload_for(&signed),
        queue_plan_admission: QueuePlanGossipAdmission::AwaitingCertificate,
    };
    let withheld =
        partition_gossip_batch(usize::MAX, usize::MAX, GossipPlane::Public, vec![awaiting]);
    assert!(withheld.message.txs.is_empty());
    assert_eq!(withheld.requeue, vec![signed.hash_as_entrypoint()]);

    let certificate = Arc::new(vec![0x5A; 257]);
    let certified = GossipBatchEntry {
        tx: accepted.clone(),
        routing: RoutingDecision::default(),
        routing_plan: default_plan(),
        payload: payload_for(&signed),
        queue_plan_admission: QueuePlanGossipAdmission::Certified(Arc::clone(&certificate)),
    };
    let included =
        partition_gossip_batch(usize::MAX, usize::MAX, GossipPlane::Public, vec![certified]);
    assert!(included.requeue.is_empty());
    assert_eq!(included.message.txs.len(), 1);
    assert_eq!(
        included.message.txs[0].queue_plan_certificate(),
        Some(certificate.as_slice())
    );
    let exact_len = included.message.encode().len();
    assert_eq!(included.encoded_len, exact_len);
    assert_eq!(included.message.encoded_len_exact(), Some(exact_len));

    let excluded = partition_gossip_batch(
        usize::MAX,
        exact_len - 1,
        GossipPlane::Public,
        vec![GossipBatchEntry {
            tx: accepted,
            routing: RoutingDecision::default(),
            routing_plan: default_plan(),
            payload: payload_for(&signed),
            queue_plan_admission: QueuePlanGossipAdmission::Certified(certificate),
        }],
    );
    assert!(excluded.message.txs.is_empty());
    assert_eq!(excluded.requeue, vec![signed.hash_as_entrypoint()]);
}
#[test]
fn gossip_route_encoded_len_matches_wire() {
    let route = GossipRoute {
        lane_id: LaneId::new(3),
        dataspace_id: DataSpaceId::new(7),
    };
    let encoded = route.encode();
    let expected = gossip_route_encoded_len().expect("gossip route len");
    assert_eq!(encoded.len(), expected);
    let decoded: GossipRoute =
        Decode::decode(&mut encoded.as_slice()).expect("decode gossip route");
    assert_eq!(decoded.lane_id, route.lane_id);
    assert_eq!(decoded.dataspace_id, route.dataspace_id);
}
#[test]
fn transaction_gossip_encoded_len_exact_matches_encode() {
    let (signed, _accepted) = build_transaction("len");
    let payload = payload_for(&signed);
    let message = TransactionGossip {
        txs: vec![GossipTransaction::with_encoded(
            signed,
            Arc::clone(&payload),
        )],
        routes: vec![GossipRoute {
            lane_id: LaneId::SINGLE,
            dataspace_id: DataSpaceId::UNIVERSAL,
        }],
        plans: vec![default_plan()],
        plane: GossipPlane::Public,
    };
    let encoded = message.encode();
    assert_eq!(
        ncore::NoritoSerialize::encoded_len_exact(&message),
        Some(encoded.len())
    );
}
#[test]
fn gossip_transaction_decode_rejects_trailing_bytes() {
    let (signed, _accepted) = build_transaction("trailing");
    let mut encoded = ncore::to_bytes(&TransactionEntrypoint::External(signed))
        .expect("encode transaction entrypoint");
    encoded.extend_from_slice(&[0xAA, 0xBB]);
    let err = ncore::decode_field_canonical::<GossipTransaction>(&encoded).expect_err("bad bytes");
    assert!(matches!(err, ncore::Error::LengthMismatch));
}
#[test]
fn gossip_transaction_decode_rejects_oversized_certificate_before_decode() {
    let (signed, _accepted) = build_transaction("oversized-certificate");
    let mut encoded = payload_for(&signed).as_ref().clone();
    let max_vec_payload = crate::kura::MAX_PENDING_QUEUE_PLAN_ADMISSION_CERTIFICATE_BYTES
        + core::mem::size_of::<u64>();
    let max_option_payload = 1 + ncore::len_prefix_len(max_vec_payload) + max_vec_payload;
    let oversized = max_option_payload + 1;
    ncore::write_len_header_to_vec(
        &mut encoded,
        u64::try_from(oversized).expect("test bound fits u64"),
    );
    encoded.resize(encoded.len() + oversized, 0);
    let error = decode_gossip_transaction_payload(&encoded)
        .expect_err("oversized certificate field must fail before semantic decode");
    assert!(matches!(error, ncore::Error::LengthMismatch));
}
#[test]
fn gossip_network_message_roundtrip_cached_payload_is_context_free() {
    let (signed, _accepted) = build_transaction("cached-network-flags");
    let canonical_payload = payload_for(&signed);
    let payload = {
        let _guard = ncore::DecodeFlagsGuard::enter(ncore::header_flags::COMPACT_LEN);
        Arc::new(
            ncore::to_bytes(&TransactionEntrypoint::External(signed.clone()))
                .expect("encode transaction entrypoint"),
        )
    };
    std::thread::spawn(move || {
        let message = TransactionGossip {
            txs: vec![GossipTransaction::with_encoded(
                signed.clone(),
                Arc::clone(&payload),
            )],
            routes: vec![GossipRoute {
                lane_id: LaneId::SINGLE,
                dataspace_id: DataSpaceId::UNIVERSAL,
            }],
            plans: vec![default_plan()],
            plane: GossipPlane::Public,
        };
        let encoded = NetworkMessage::TransactionGossiper(Arc::new(message)).encode();
        let decoded: NetworkMessage =
            Decode::decode(&mut encoded.as_slice()).expect("decode network message");
        match decoded {
            NetworkMessage::TransactionGossiper(message) => {
                assert_eq!(message.txs.len(), 1);
                assert_eq!(
                    message.txs[0].encoded.as_slice(),
                    canonical_payload.as_slice()
                );
                assert!(message.txs[0].encoded.starts_with(&ncore::MAGIC));
            }
            other => panic!("unexpected network message: {other:?}"),
        }
    })
    .join()
    .expect("context-free network gossip thread");
}
#[test]
fn transaction_gossip_roundtrip_cached_payload_is_context_free() {
    let (signed, _accepted) = build_transaction("cached-txgossip-flags");
    let canonical_payload = payload_for(&signed);
    let payload = {
        let _guard = ncore::DecodeFlagsGuard::enter(ncore::header_flags::COMPACT_LEN);
        Arc::new(
            ncore::to_bytes(&TransactionEntrypoint::External(signed.clone()))
                .expect("encode transaction entrypoint"),
        )
    };
    std::thread::spawn(move || {
        let message = TransactionGossip {
            txs: vec![GossipTransaction::with_encoded(
                signed.clone(),
                Arc::clone(&payload),
            )],
            routes: vec![GossipRoute {
                lane_id: LaneId::SINGLE,
                dataspace_id: DataSpaceId::UNIVERSAL,
            }],
            plans: vec![default_plan()],
            plane: GossipPlane::Public,
        };
        let encoded = message.encode();
        let decoded: TransactionGossip =
            Decode::decode(&mut encoded.as_slice()).expect("decode transaction gossip");
        assert_eq!(decoded.txs.len(), 1);
        assert_eq!(
            decoded.txs[0].encoded.as_slice(),
            canonical_payload.as_slice()
        );
        assert!(decoded.txs[0].encoded.starts_with(&ncore::MAGIC));
        assert_eq!(decoded.routes.len(), 1);
        assert_eq!(decoded.routes[0].lane_id, LaneId::SINGLE);
        assert_eq!(decoded.routes[0].dataspace_id, DataSpaceId::UNIVERSAL);
        assert_eq!(decoded.plane, GossipPlane::Public);
    })
    .join()
    .expect("context-free transaction gossip thread");
}
#[test]
fn gossip_roundtrip_preserves_large_ram_lfe_policy_transaction() {
    let signed = register_ram_lfe_program_policy_tx();
    let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(signed.clone()));
    let signed_encoded = signed.encode();
    let signed_decoded: SignedTransaction =
        Decode::decode(&mut signed_encoded.as_slice()).expect("decode signed transaction");
    assert_eq!(signed_decoded.hash(), signed.hash());
    let gossip_tx = GossipTransaction::new(accepted.clone());
    let gossip_tx_encoded = gossip_tx.encode();
    let gossip_tx_decoded: GossipTransaction =
        Decode::decode(&mut gossip_tx_encoded.as_slice()).expect("decode gossip transaction");
    assert_eq!(gossip_tx_decoded.as_signed().hash(), signed.hash());
    let message = TransactionGossip {
        txs: vec![gossip_tx],
        routes: vec![GossipRoute {
            lane_id: LaneId::SINGLE,
            dataspace_id: DataSpaceId::UNIVERSAL,
        }],
        plans: vec![default_plan()],
        plane: GossipPlane::Public,
    };
    let encoded = message.encode();
    let decoded: TransactionGossip =
        Decode::decode(&mut encoded.as_slice()).expect("decode gossip");
    assert_eq!(decoded.txs.len(), 1);
    assert_eq!(decoded.txs[0].as_signed().hash(), signed.hash());
}
#[test]
fn gossip_network_message_roundtrip_preserves_cached_payload() {
    let (signed, _accepted) = build_transaction("cached-network");
    let payload = payload_for(&signed);
    let message = TransactionGossip {
        txs: vec![GossipTransaction::with_encoded(
            signed.clone(),
            Arc::clone(&payload),
        )],
        routes: vec![GossipRoute {
            lane_id: LaneId::SINGLE,
            dataspace_id: DataSpaceId::UNIVERSAL,
        }],
        plans: vec![default_plan()],
        plane: GossipPlane::Public,
    };
    let network = NetworkMessage::TransactionGossiper(Arc::new(message));
    let encoded = network.encode();
    let decoded: NetworkMessage =
        Decode::decode(&mut encoded.as_slice()).expect("decode network gossip");
    match decoded {
        NetworkMessage::TransactionGossiper(message) => {
            assert_eq!(message.txs.len(), 1);
            assert_eq!(message.routes.len(), 1);
            assert_eq!(message.txs[0].as_signed().hash(), signed.hash());
            assert_eq!(message.txs[0].encoded.as_slice(), payload.as_slice());
            assert_eq!(message.routes[0].lane_id, LaneId::SINGLE);
            assert_eq!(message.routes[0].dataspace_id, DataSpaceId::UNIVERSAL);
            assert_eq!(message.plane, GossipPlane::Public);
        }
        other => panic!("unexpected network message: {other:?}"),
    }
}
#[test]
fn partition_yields_empty_when_cap_too_small() {
    let (signed, accepted) = build_transaction("tiny");
    let cap = 1;
    let partitioned = partition_gossip_batch(
        usize::MAX,
        cap,
        GossipPlane::Public,
        vec![GossipBatchEntry {
            tx: accepted,
            routing: RoutingDecision::default(),
            routing_plan: default_plan(),
            payload: payload_for(&signed),
            queue_plan_admission: QueuePlanGossipAdmission::Ordinary,
        }],
    );
    assert!(partitioned.message.txs.is_empty());
    assert!(partitioned.message.routes.is_empty());
    assert_eq!(partitioned.requeue, vec![signed.hash_as_entrypoint()]);
}
#[test]
fn partition_preserves_native_amx_full_routing_plan() {
    let (signed, accepted) = build_transaction("native-amx-gossip");
    let coordinator = RoutingDecision::new(LaneId::new(2), DataSpaceId::new(7));
    let first_participant = RoutingDecision::new(LaneId::new(2), DataSpaceId::new(7));
    let second_participant = RoutingDecision::new(LaneId::new(3), DataSpaceId::new(8));
    let plan = RoutingPlan::native_amx(
        coordinator,
        vec![
            crate::queue::RouteLeg::new(first_participant, crate::queue::RouteLegRole::Participant),
            crate::queue::RouteLeg::new(
                second_participant,
                crate::queue::RouteLegRole::Participant,
            ),
        ],
    );
    let partitioned = partition_gossip_batch(
        usize::MAX,
        usize::MAX,
        GossipPlane::Public,
        vec![GossipBatchEntry {
            tx: accepted,
            routing: coordinator,
            routing_plan: plan.clone(),
            payload: payload_for(&signed),
            queue_plan_admission: QueuePlanGossipAdmission::Ordinary,
        }],
    );
    assert!(partitioned.requeue.is_empty());
    assert_eq!(partitioned.message.plans, vec![plan.clone()]);
    let decoded = decode_gossip_message(&partitioned.message);
    assert_eq!(decoded.plans, vec![plan]);
    let RoutingPlan::NativeAmx(native_plan) = &decoded.plans[0] else {
        panic!("decoded gossip plan should remain native AMX");
    };
    assert_eq!(
        native_plan
            .participants
            .iter()
            .map(|leg| leg.route)
            .collect::<Vec<_>>(),
        vec![first_participant, second_participant]
    );
}
#[test]
fn partition_respects_max_count() {
    let (tx_a_signed, tx_a_accepted) = build_transaction("a");
    let (tx_b_signed, tx_b_accepted) = build_transaction("b");
    let cap = norito::codec::Encode::encode(&TransactionGossip {
        txs: vec![tx_a_signed.clone().into(), tx_b_signed.clone().into()],
        plans: vec![
            default_plan(),
            RoutingPlan::single(RoutingDecision::new(LaneId::new(2), DataSpaceId::new(5))),
        ],
        routes: vec![
            GossipRoute {
                lane_id: LaneId::SINGLE,
                dataspace_id: DataSpaceId::UNIVERSAL,
            },
            GossipRoute {
                lane_id: LaneId::new(2),
                dataspace_id: DataSpaceId::new(5),
            },
        ],
        plane: GossipPlane::Public,
    })
    .len()
        + 16;
    let partitioned = partition_gossip_batch(
        1,
        cap,
        GossipPlane::Public,
        vec![
            GossipBatchEntry {
                tx: tx_a_accepted,
                routing: RoutingDecision::default(),
                routing_plan: default_plan(),
                payload: payload_for(&tx_a_signed),
                queue_plan_admission: QueuePlanGossipAdmission::Ordinary,
            },
            GossipBatchEntry {
                tx: tx_b_accepted,
                routing: RoutingDecision::default(),
                routing_plan: default_plan(),
                payload: payload_for(&tx_b_signed),
                queue_plan_admission: QueuePlanGossipAdmission::Ordinary,
            },
        ],
    );
    let hashes: Vec<_> = partitioned
        .message
        .txs
        .iter()
        .map(|tx| tx.as_signed().hash())
        .collect();
    assert_eq!(hashes, vec![tx_a_signed.hash()]);
    assert_eq!(partitioned.requeue, vec![tx_b_signed.hash_as_entrypoint()]);
}
#[test]
fn validate_route_rejects_missing_lane() {
    let catalog = LaneCatalog::default();
    let dataspace_catalog = DataSpaceCatalog::default();
    let route = GossipRoute {
        lane_id: LaneId::new(5),
        dataspace_id: DataSpaceId::new(7),
    };
    assert_eq!(
        validate_route(&catalog, &dataspace_catalog, &BTreeSet::new(), route),
        Err("unknown_lane")
    );
}
#[test]
fn validate_route_rejects_dataspace_mismatch() {
    let lane = iroha_data_model::nexus::LaneConfig {
        id: LaneId::new(1),
        dataspace_id: DataSpaceId::new(2),
        alias: "alpha".to_string(),
        visibility: LaneVisibility::Public,
        ..iroha_data_model::nexus::LaneConfig::default()
    };
    let catalog = LaneCatalog::new(
        core::num::NonZeroU32::new(4).expect("nonzero lanes"),
        vec![lane],
    )
    .expect("lane catalog");
    let dataspace_catalog = DataSpaceCatalog::new(vec![
        DataSpaceMetadata {
            id: DataSpaceId::new(2),
            alias: "lane-dataspace".to_string(),
            description: None,
            fault_tolerance: 1,
        },
        DataSpaceMetadata {
            id: DataSpaceId::new(3),
            alias: "route-dataspace".to_string(),
            description: None,
            fault_tolerance: 1,
        },
    ])
    .expect("dataspace catalog");
    let route = GossipRoute {
        lane_id: LaneId::new(1),
        dataspace_id: DataSpaceId::new(3),
    };
    assert_eq!(
        validate_route(&catalog, &dataspace_catalog, &BTreeSet::new(), route),
        Err("lane_dataspace_mismatch")
    );
}
#[test]
fn validate_route_rejects_missing_dataspace() {
    let lane = iroha_data_model::nexus::LaneConfig {
        id: LaneId::new(1),
        dataspace_id: DataSpaceId::new(9),
        alias: "dangling".to_string(),
        visibility: LaneVisibility::Restricted,
        ..iroha_data_model::nexus::LaneConfig::default()
    };
    let catalog = LaneCatalog::new(
        core::num::NonZeroU32::new(2).expect("nonzero lanes"),
        vec![lane],
    )
    .expect("lane catalog");
    let dataspace_catalog = DataSpaceCatalog::default();
    let route = GossipRoute {
        lane_id: LaneId::new(1),
        dataspace_id: DataSpaceId::new(9),
    };
    assert_eq!(
        validate_route(&catalog, &dataspace_catalog, &BTreeSet::new(), route),
        Err("unknown_dataspace")
    );
}
#[test]
fn validate_route_accepts_matching_lane() {
    let lane = iroha_data_model::nexus::LaneConfig {
        id: LaneId::new(2),
        dataspace_id: DataSpaceId::new(9),
        alias: "beta".to_string(),
        visibility: LaneVisibility::Restricted,
        ..iroha_data_model::nexus::LaneConfig::default()
    };
    let catalog = LaneCatalog::new(
        core::num::NonZeroU32::new(3).expect("nonzero lanes"),
        vec![lane],
    )
    .expect("lane catalog");
    let dataspace_catalog = DataSpaceCatalog::new(vec![DataSpaceMetadata {
        id: DataSpaceId::new(9),
        alias: "beta-dataspace".to_string(),
        description: None,
        fault_tolerance: 1,
    }])
    .expect("dataspace catalog");
    let route = GossipRoute {
        lane_id: LaneId::new(2),
        dataspace_id: DataSpaceId::new(9),
    };
    let active_lane_ids = BTreeSet::from([route.lane_id]);
    assert_eq!(
        validate_route(&catalog, &dataspace_catalog, &active_lane_ids, route),
        Ok(())
    );
}
#[test]
fn disabled_nexus_gossip_activity_contains_only_canonical_single_lane() {
    let state = State::new_for_testing(
        World::new(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );

    let nexus = state.nexus_snapshot();
    assert_eq!(
        active_gossip_lane_ids(&state, &nexus),
        BTreeSet::from([LaneId::SINGLE])
    );
}
