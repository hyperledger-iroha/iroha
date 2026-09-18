// Actual canonical certificate/queue fixtures, with no network workload or measurement claim.
fn gossip_publication_block(
    previous: Option<&iroha_data_model::block::SignedBlock>,
) -> iroha_data_model::block::SignedBlock {
    let key = KeyPair::try_from_seed(vec![0xD3; 32], Algorithm::BlsNormal).unwrap();
    let valid =
        crate::block::ValidBlock::new_dummy_and_modify_header(key.private_key(), |header| {
            header.set_height(
                std::num::NonZeroU64::new(
                    previous.map_or(1, |block| block.header().height().get() + 1),
                )
                .unwrap(),
            );
            header.set_prev_block_hash(previous.map(|block| block.hash()));
            header.creation_time_ms = previous.map_or(1_700_000_000_000, |block| {
                block.header().creation_time_ms + 1
            });
        });
    let mut block: iroha_data_model::block::SignedBlock = valid.into();
    { let outputs = crate::execution_output_test_support::structural_network_outputs(&block, &[], Vec::new());
let fragments = u64::try_from(outputs.iter().filter(|row| row.result().is_ok()).count()).unwrap();
block.set_execution_outputs(outputs, fragments, BTreeMap::new(),
Vec::new(),
iroha_data_model::nexus::AxtPolicySnapshot::default(),
Default::default(),
Vec::new(),
&crate::execution_output_test_support::structural_output_limits()) }
        .unwrap();

    block
}
fn retained_publication_message(
    signed: &SignedTransaction,
    certificate: Vec<u8>,
    shared: bool,
) -> (
    RetainedGossip<Arc<TransactionGossip>>,
    Arc<tokio::sync::Semaphore>,
    Option<Arc<TransactionGossip>>,
) {
    let message = TransactionGossip {
        txs: vec![
            GossipTransaction::from_queue_plan_admitted_input(Arc::new(certificate))
                .expect("structural complete input"),
        ],
        routes: vec![GossipRoute {
            lane_id: LaneId::SINGLE,
            dataspace_id: DataSpaceId::UNIVERSAL,
        }],
        plans: vec![default_plan()],
        plane: GossipPlane::Public,
    };
    assert_eq!(
        message.txs[0].payload().as_slice(),
        payload_for(signed).as_slice(),
        "fixture's sole certified input retains exact original signed bytes"
    );
    let message = Arc::new(decode_gossip_message(&message));
    let extra = shared.then(|| Arc::clone(&message));
    let (retained, count) = RetainedGossip::with_count_for_test(message);
    (retained, count, extra)
}
fn retry_publication_gossip(
    gossiper: &TransactionGossiper,
    pending: PendingGossip,
) -> Option<PendingGossip> {
    gossiper.handle_retained_gossip(pending.message, pending.progress, Some(pending.deadline))
}
fn assert_exact_publication_body(
    gossiper: &TransactionGossiper,
    signed: SignedTransaction,
    binding: crate::torii_proxy::QueuePlanAdmissionBindingV1,
    certificate: &[u8],
) {
    assert_eq!(gossiper.queue.queued_len(), 1);
    let tx = AcceptedTransaction::new_unchecked(Cow::Owned(signed));
    let claim = gossiper
        .queue
        .durable_plan_admission_claim_with_state(&tx, &gossiper.state)
        .unwrap()
        .expect("body owns the exact durable admission claim");
    assert_eq!(
        crate::torii_proxy::queue_plan_binding_from_durable_admission(&claim).unwrap(),
        binding
    );
    assert_eq!(
        gossiper
            .state
            .kura()
            .pending_queue_plan_admission_certificates()
            .unwrap(),
        vec![(Hash::new(certificate), certificate.to_vec())]
    );
}

#[test]
fn retained_queue_plan_body_recovers_from_real_one_ahead_fence_without_redelivery() {
    for shared in [false, true] {
        for duplicate in [false, true] {
            let (gossiper, signed, binding, certificate, _journal) =
                exact_pending_queue_plan_gossip_fixture("one-ahead single delivery");
            if duplicate {
                assert!(matches!(
                    persist_queue_plan_gossip_certificate(&gossiper.state, &certificate, &binding)
                        .unwrap(),
                    QueuePlanGossipCertificateDisposition::ExactPending
                ));
            }
            let successor = gossip_publication_block(None);
            gossiper
                .state
                .kura()
                .store_block(Arc::new(successor.clone()))
                .unwrap();
            let (message, count, _extra) =
                retained_publication_message(&signed, certificate.clone(), shared);
            let deadline = tokio::time::Instant::now() + gossiper.queue.tx_time_to_live;
            let pending = gossiper
                .handle_retained_gossip(message, GossipProgress::default(), Some(deadline))
                .expect("the real 250 ms Kura+1 refusal retains the sole delivered body");
            assert_eq!(pending.required_height, 1);
            assert_eq!(pending.deadline, deadline);
            assert_eq!(count.available_permits(), 0);
            assert_eq!(
                gossiper.queue.queued_len(),
                0,
                "no admission before exact publication"
            );
            assert_eq!(
                gossiper
                    .state
                    .kura()
                    .pending_queue_plan_admission_certificates()
                    .unwrap()
                    .len(),
                usize::from(duplicate)
            );
            // The physical attempt has returned its typed pending outcome. No timing
            // sleep guesses that it saw the overlap, and no second message is sent.
            gossiper
                .state
                .append_committed_block_header_for_tests(successor.header().clone());
            assert!(retry_publication_gossip(&gossiper, pending).is_none());
            assert_eq!(count.available_permits(), 1);
            assert_exact_publication_body(&gossiper, signed, binding, &certificate);
        }
    }
}

#[test]
fn retained_future_queue_plan_body_is_authenticated_and_reclassified_after_catchup() {
    for shared in [false, true] {
        let (gossiper, signed, binding, certificate, _journal) =
            publication_queue_plan_gossip_fixture("future single delivery", true);
        assert!(matches!(
            validate_queue_plan_gossip_certificate(
                &gossiper.state,
                &certificate,
                &TransactionEntrypoint::External(signed.clone()),
                &default_plan()
            )
            .unwrap()
            .1,
            QueuePlanGossipCertificateDisposition::AwaitPublication(1)
        ));
        let (message, count, _extra) =
            retained_publication_message(&signed, certificate.clone(), shared);
        let pending = gossiper
            .handle_retained_gossip(
                message,
                GossipProgress::default(),
                Some(tokio::time::Instant::now() + gossiper.queue.tx_time_to_live),
            )
            .unwrap();
        assert_eq!(pending.required_height, 1);
        let original_deadline = pending.deadline;
        let pending = retry_publication_gossip(&gossiper, pending).unwrap();
        assert_eq!(
            pending.deadline, original_deadline,
            "a repeated Future cannot renew the budget"
        );
        assert_eq!(count.available_permits(), 0);
        assert_eq!(gossiper.queue.queued_len(), 0);
        assert_eq!(
            gossiper
                .state
                .kura()
                .pending_queue_plan_admission_certificates()
                .unwrap(),
            vec![(Hash::new(&certificate), certificate.clone())],
            "authentic future evidence stays durable"
        );
        let successor = gossip_publication_block(None);
        gossiper
            .state
            .kura()
            .store_block(Arc::new(successor.clone()))
            .unwrap();
        gossiper
            .state
            .append_committed_block_header_for_tests(successor.header().clone());
        assert!(retry_publication_gossip(&gossiper, pending).is_none());
        assert_eq!(count.available_permits(), 1);
        assert_exact_publication_body(&gossiper, signed, binding, &certificate);
    }
}

#[test]
fn corrupt_and_foreign_roster_future_certificates_never_install_pending_body() {
    for foreign_roster in [false, true] {
        let (gossiper, signed, _, certificate, _journal) =
            publication_queue_plan_gossip_fixture("unauthorized future", true);
        let mut certificate = norito::decode_canonical::<
            iroha_data_model::block::lane_admission::LaneAdmittedInputV1,
        >(&certificate)
        .unwrap()
        .certificate;
        if foreign_roster {
            let mut foreign = (0..4_u8)
                .map(|i| KeyPair::try_from_seed(vec![0xE0 + i; 32], Algorithm::BlsNormal).unwrap())
                .collect::<Vec<_>>();
            foreign.sort_by(|a, b| a.public_key().cmp(b.public_key()));
            let mut context = certificate.binding.admission_context.clone();
            let source = &mut context.route_incarnations[0];
            source.validator_set = foreign
                .iter()
                .map(|key| PeerId::new(key.public_key().clone()))
                .collect();
            source.validator_set_hash = HashOf::new(&source.validator_set);
            certificate.binding = crate::torii_proxy::new_queue_plan_admission_binding(
                gossiper.state.network_id_ref(),
                &TransactionEntrypoint::External(signed.clone()),
                &default_plan(),
                context,
                certificate.binding.enqueue_timestamp_ms,
            )
            .unwrap();
            let hash = certificate.binding.canonical_hash();
            for attestation in &mut certificate.attestations {
                let bytes = crate::torii_proxy::queue_plan_admission_attestation_signing_bytes_v1(
                    hash,
                    attestation.validator_index,
                )
                .unwrap();
                attestation.signature = iroha_crypto::Signature::try_new(
                    foreign[usize::from(attestation.validator_index)].private_key(),
                    &bytes,
                )
                .unwrap();
            }
        } else {
            certificate.attestations[0].signature =
                iroha_crypto::Signature::try_new(BOB_KEYPAIR.private_key(), b"wrong admission")
                    .unwrap();
        }
        let certificate = norito::encode_canonical(
            &iroha_data_model::block::lane_admission::LaneAdmittedInputV1 {
                entrypoint: TransactionEntrypoint::External(signed.clone()),
                certificate,
            },
        )
        .unwrap();
        if foreign_roster {
            crate::torii_proxy::decode_and_validate_lane_admitted_input_v1(
                gossiper.state.network_id_ref(),
                &certificate,
            )
            .expect("the internally valid foreign quorum must fail CURRENT State source authority");
        }
        let (message, count, _) = retained_publication_message(&signed, certificate, false);
        assert!(
            gossiper
                .handle_retained_gossip(
                    message,
                    GossipProgress::default(),
                    Some(tokio::time::Instant::now() + gossiper.queue.tx_time_to_live)
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
    }
}

#[test]
fn future_certificate_without_canonical_pending_cannot_override_local_routing() {
    let (mut gossiper, signed, _, certificate, journal) =
        publication_queue_plan_gossip_fixture("future still requires local route", true);
    let (_clock, time_source) = TimeSource::new_mock(Duration::default());
    let queue = Queue::test_with_router(
        QueueConfig::default(),
        &time_source,
        Arc::new(MismatchedQueuePlanRouter),
    );
    queue
        .install_plan_journal(journal.path().join("mismatched.norito"), 1024 * 1024, true)
        .unwrap();
    gossiper.queue = Arc::new(queue);
    let (message, count, _) = retained_publication_message(&signed, certificate, false);
    let pending = gossiper
        .handle_retained_gossip(
            message,
            GossipProgress::default(),
            Some(tokio::time::Instant::now() + gossiper.queue.tx_time_to_live),
        )
        .expect("authenticated Future can wait for publication");
    assert_eq!(count.available_permits(), 0);
    let successor = gossip_publication_block(None);
    gossiper
        .state
        .kura()
        .store_block(Arc::new(successor.clone()))
        .unwrap();
    gossiper
        .state
        .append_committed_block_header_for_tests(successor.header().clone());
    assert!(retry_publication_gossip(&gossiper, pending).is_none());
    assert_eq!(count.available_permits(), 1);
    assert_eq!(
        gossiper.queue.queued_len(),
        0,
        "a certificate alone cannot replace canonical WSV Pending route authority"
    );
}

#[test]
fn larger_durable_skew_retires_body_without_weakening_exact_fence() {
    let (gossiper, signed, binding, certificate, _journal) =
        exact_pending_queue_plan_gossip_fixture("hard skew");
    let first = gossip_publication_block(None);
    let second = gossip_publication_block(Some(&first));
    gossiper.state.kura().store_block(Arc::new(first)).unwrap();
    gossiper.state.kura().store_block(Arc::new(second)).unwrap();
    assert!(
        persist_queue_plan_gossip_certificate(&gossiper.state, &certificate, &binding).is_err()
    );
    let (message, count, _) = retained_publication_message(&signed, certificate, false);
    assert!(
        gossiper
            .handle_retained_gossip(
                message,
                GossipProgress::default(),
                Some(tokio::time::Instant::now() + gossiper.queue.tx_time_to_live)
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
}

#[test]
fn partial_batch_keeps_completed_and_rejected_entries_final_across_future_retry() {
    let (_, _first, _, first_certificate, _first_journal) =
        exact_pending_queue_plan_gossip_fixture("first ready");
    let (gossiper, _future, _, future_certificate, _journal) =
        publication_queue_plan_gossip_fixture("second future", true);
    let mut rejected = norito::decode_canonical::<
        iroha_data_model::block::lane_admission::LaneAdmittedInputV1,
    >(&future_certificate)
    .unwrap();
    rejected.certificate.attestations[0].signature =
        iroha_crypto::Signature::try_new(BOB_KEYPAIR.private_key(), b"bad partial batch authority")
            .unwrap();
    let rejected = norito::encode_canonical(&rejected).unwrap();
    let message = TransactionGossip {
        txs: vec![
            GossipTransaction::from_queue_plan_admitted_input(Arc::new(first_certificate))
                .expect("structural complete input"),
            GossipTransaction::from_queue_plan_admitted_input(Arc::new(rejected))
                .expect("structural complete input"),
            GossipTransaction::from_queue_plan_admitted_input(Arc::new(future_certificate))
                .expect("structural complete input"),
        ],
        routes: vec![
            GossipRoute {
                lane_id: LaneId::SINGLE,
                dataspace_id: DataSpaceId::UNIVERSAL
            };
            3
        ],
        plans: vec![default_plan(); 3],
        plane: GossipPlane::Public,
    };
    let (message, count) = RetainedGossip::with_count_for_test(Arc::new(message));
    let pending = gossiper
        .handle_retained_gossip(
            message,
            GossipProgress::default(),
            Some(tokio::time::Instant::now() + gossiper.queue.tx_time_to_live),
        )
        .unwrap();
    assert_eq!(gossiper.queue.queued_len(), 1);
    assert!(pending.progress.is_complete(0));
    assert!(pending.progress.is_complete(1));
    assert!(!pending.progress.is_complete(2));
    assert_eq!(count.available_permits(), 0);
    let successor = gossip_publication_block(None);
    gossiper
        .state
        .kura()
        .store_block(Arc::new(successor.clone()))
        .unwrap();
    gossiper
        .state
        .append_committed_block_header_for_tests(successor.header().clone());
    assert!(retry_publication_gossip(&gossiper, pending).is_none());
    assert_eq!(gossiper.queue.queued_len(), 2);
    assert_eq!(
        gossiper
            .state
            .kura()
            .pending_queue_plan_admission_certificates()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(count.available_permits(), 1);
}

#[test]
fn zero_queue_ttl_keeps_first_exact_pending_attempt_but_no_future_residence() {
    for future in [false, true] {
        let (mut gossiper, signed, _, certificate, _journal) =
            publication_queue_plan_gossip_fixture("zero ttl", future);
        Arc::get_mut(&mut gossiper.queue).unwrap().tx_time_to_live = Duration::ZERO;
        let (message, count, _) = retained_publication_message(&signed, certificate, false);
        assert!(
            gossiper
                .handle_retained_gossip(
                    message,
                    GossipProgress::default(),
                    Some(tokio::time::Instant::now())
                )
                .is_none()
        );
        assert_eq!(count.available_permits(), 1);
        assert_eq!(gossiper.queue.queued_len(), usize::from(!future));
    }
    let (mut ordinary, _, _, _, _journal) =
        exact_pending_queue_plan_gossip_fixture("ordinary zero ttl");
    let (_, clock) = TimeSource::new_mock(Duration::ZERO);
    ordinary.queue = Arc::new(Queue::test(
        QueueConfig {
            transaction_time_to_live: Duration::ZERO,
            ..QueueConfig::default()
        },
        &clock,
    ));
    let (signed, _) = build_transaction("ordinary first attempt");
    let (message, count) = RetainedGossip::with_count_for_test(Arc::new(TransactionGossip {
        txs: vec![signed.into()],
        routes: vec![GossipRoute {
            lane_id: LaneId::SINGLE,
            dataspace_id: DataSpaceId::UNIVERSAL,
        }],
        plans: vec![default_plan()],
        plane: GossipPlane::Public,
    }));
    assert!(
        ordinary
            .handle_retained_gossip(message, GossipProgress::default(), None)
            .is_none()
    );
    assert_eq!(count.available_permits(), 1);
    assert_eq!(
        ordinary.queue.queued_len(),
        1,
        "ordinary admission does not depend on a deferred deadline"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn unrepresentable_queue_ttl_refuses_before_actor_publication() {
    let (mut gossiper, _, _, _, _journal) =
        exact_pending_queue_plan_gossip_fixture("unrepresentable ttl");
    Arc::get_mut(&mut gossiper.queue).unwrap().tx_time_to_live = Duration::MAX;
    assert!(matches!(
        gossiper.start(ShutdownSignal::new()),
        Err(TransactionGossiperStartError::UnrepresentableRetentionBudget)
    ));
}

fn resign_current_gossip_authority(
    certificate: &mut crate::torii_proxy::QueuePlanAdmissionCertificateV1,
) {
    let keys = (0..4_u8)
        .map(|i| KeyPair::try_from_seed(vec![0xB4 + i; 32], Algorithm::BlsNormal).unwrap())
        .collect::<Vec<_>>();
    let hash = certificate.binding.canonical_hash();
    let roster = &certificate.binding.admission_context.route_incarnations[0].validator_set;
    for attestation in &mut certificate.attestations {
        let peer = &roster[usize::from(attestation.validator_index)];
        let key = keys
            .iter()
            .find(|key| key.public_key() == peer.public_key())
            .unwrap();
        let bytes = crate::torii_proxy::queue_plan_admission_attestation_signing_bytes_v1(
            hash,
            attestation.validator_index,
        )
        .unwrap();
        attestation.signature =
            iroha_crypto::Signature::try_new(key.private_key(), &bytes).unwrap();
    }
}

#[test]
fn authenticated_conflicting_binding_is_terminal_without_pending_retention() {
    let (gossiper, signed, _, certificate, _journal) =
        exact_pending_queue_plan_gossip_fixture("conflicting binding");
    let mut certificate = norito::decode_canonical::<
        iroha_data_model::block::lane_admission::LaneAdmittedInputV1,
    >(&certificate)
    .unwrap()
    .certificate;
    certificate.binding = crate::torii_proxy::new_queue_plan_admission_binding(
        gossiper.state.network_id_ref(),
        &TransactionEntrypoint::External(signed.clone()),
        &default_plan(),
        certificate.binding.admission_context.clone(),
        certificate.binding.enqueue_timestamp_ms + 1,
    )
    .unwrap();
    resign_current_gossip_authority(&mut certificate);
    let certificate = norito::encode_canonical(
        &iroha_data_model::block::lane_admission::LaneAdmittedInputV1 {
            entrypoint: TransactionEntrypoint::External(signed.clone()),
            certificate,
        },
    )
    .unwrap();
    crate::torii_proxy::decode_and_validate_lane_admitted_input_v1(
        gossiper.state.network_id_ref(),
        &certificate,
    )
    .unwrap();
    assert!(matches!(
        gossiper
            .state
            .classify_pending_queue_plan_admission(&certificate, 1)
            .unwrap()
            .1,
        PendingQueuePlanAdmissionDisposition::DefinitiveConflict
    ));
    let (message, count, _) = retained_publication_message(&signed, certificate, false);
    assert!(
        gossiper
            .handle_retained_gossip(
                message,
                GossipProgress::default(),
                Some(tokio::time::Instant::now() + gossiper.queue.tx_time_to_live)
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
}

#[test]
fn quorum_certified_future_with_invalid_body_signature_cannot_retain_credit() {
    let (gossiper, mut signed, _, certificate, _journal) =
        publication_queue_plan_gossip_fixture("bad future body", true);
    corrupt_signature(&mut signed);
    assert!(signed.verify_signature().is_err());
    let mut certificate = norito::decode_canonical::<
        iroha_data_model::block::lane_admission::LaneAdmittedInputV1,
    >(&certificate)
    .unwrap()
    .certificate;
    certificate.binding = crate::torii_proxy::new_queue_plan_admission_binding(
        gossiper.state.network_id_ref(),
        &TransactionEntrypoint::External(signed.clone()),
        &default_plan(),
        certificate.binding.admission_context.clone(),
        certificate.binding.enqueue_timestamp_ms,
    )
    .unwrap();
    resign_current_gossip_authority(&mut certificate);
    let certificate = norito::encode_canonical(
        &iroha_data_model::block::lane_admission::LaneAdmittedInputV1 {
            entrypoint: TransactionEntrypoint::External(signed.clone()),
            certificate,
        },
    )
    .unwrap();
    assert!(matches!(
        validate_queue_plan_gossip_certificate(
            &gossiper.state,
            &certificate,
            &TransactionEntrypoint::External(signed.clone()),
            &default_plan()
        )
        .unwrap()
        .1,
        QueuePlanGossipCertificateDisposition::AwaitPublication(1)
    ));
    let (message, count, _) = retained_publication_message(&signed, certificate, false);
    assert!(
        gossiper
            .handle_retained_gossip(
                message,
                GossipProgress::default(),
                Some(tokio::time::Instant::now() + gossiper.queue.tx_time_to_live)
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
}
