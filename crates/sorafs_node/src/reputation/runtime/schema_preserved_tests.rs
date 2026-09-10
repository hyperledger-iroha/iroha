// Existing tests kept in their original parent test namespace.
#[test]
fn counted_validation_allocates_atomic_gateway_sequences_and_recovers() {
    let policy = producer_policy();
    let (temp, outbox) = initialized_outbox(policy.clone());
    let producer = token_producer(Arc::clone(&outbox));
    let gateway_id = [0x91; 32];
    let mut workers = Vec::new();
    for index in 0_u8..16 {
        let producer = producer.clone();
        workers.push(std::thread::spawn(move || {
            let context = request_context(&format!("concurrent-nonce-{index:02}"));
            enqueue_accepted(
                &producer,
                gateway_id,
                &context,
                FINALIZED_AT_MS - 500 + u64::from(index),
            )
        }));
    }
    for worker in workers {
        assert!(matches!(
            worker.join().expect("join validation worker"),
            Ok(ReputationJournalEnqueueOutcomeV1::Inserted { .. })
        ));
    }
    let mut gateway_sequences = {
        let state = outbox.state.lock().expect("producer state");
        state
            .checkpoint
            .pending
            .iter()
            .filter_map(|delivery| {
                let ReputationJournalPayloadV1::StreamTokenValidation(outcome) =
                    &delivery.entry.payload
                else {
                    return None;
                };
                Some(outcome.binding.gateway_sequence)
            })
            .collect::<Vec<_>>()
    };
    gateway_sequences.sort_unstable();
    assert_eq!(gateway_sequences, (1_u64..=16).collect::<Vec<_>>());
    let (latest_context, earlier_context) = {
        let state = outbox.state.lock().expect("producer state");
        let latest = state
            .checkpoint
            .stream_token_gateway_heads
            .iter()
            .find(|head| head.binding.gateway_id == gateway_id)
            .expect("gateway head");
        let latest_context = state
            .checkpoint
            .pending
            .iter()
            .find_map(|delivery| {
                let ReputationJournalPayloadV1::StreamTokenValidation(outcome) =
                    &delivery.entry.payload
                else {
                    return None;
                };
                (outcome.binding == latest.binding).then_some((
                    latest.binding.request_context_digest,
                    delivery.entry.event_id,
                ))
            })
            .expect("latest pending gateway event");
        let earlier = state
            .checkpoint
            .pending
            .iter()
            .find_map(|delivery| {
                let ReputationJournalPayloadV1::StreamTokenValidation(outcome) =
                    &delivery.entry.payload
                else {
                    return None;
                };
                (outcome.binding.gateway_id == gateway_id && outcome.binding != latest.binding)
                    .then(|| {
                        (
                            outcome.binding.request_context_digest,
                            delivery.entry.event_id,
                            delivery.entry.clone(),
                        )
                    })
            })
            .expect("earlier pending gateway event");
        (latest_context, earlier)
    };
    let earlier_replay_context = (0_u8..16)
        .map(|index| request_context(&format!("concurrent-nonce-{index:02}")))
        .find(|context| context.digest().ok() == Some(earlier_context.0))
        .expect("recover earlier exact request context");
    assert_eq!(
        enqueue_accepted(
            &producer,
            gateway_id,
            &earlier_replay_context,
            FINALIZED_AT_MS + 200,
        )
        .expect("non-head retry is retained"),
        ReputationJournalEnqueueOutcomeV1::ExactReplay {
            event_id: earlier_context.1
        }
    );
    outbox
        .reconcile_finalized_journal_page(terminal_page(
            10,
            [0xA4; 32],
            FINALIZED_AT_MS + 220,
            vec![ReputationJournalFinalizedEventV1 {
                sequence: 1,
                block_height: 10,
                block_hash: [0xA4; 32],
                event_index: 0,
                recorded_at_unix_ms: FINALIZED_AT_MS + 210,
                entry: earlier_context.2,
            }],
        ))
        .expect("finalize earlier retained validation");
    assert_eq!(
        enqueue_accepted(
            &producer,
            gateway_id,
            &earlier_replay_context,
            FINALIZED_AT_MS + 230,
        )
        .expect("completed non-head retry is retained"),
        ReputationJournalEnqueueOutcomeV1::ExactReplay {
            event_id: earlier_context.1
        }
    );
    let replay_context = (0_u8..16)
        .map(|index| request_context(&format!("concurrent-nonce-{index:02}")))
        .find(|context| context.digest().ok() == Some(latest_context.0))
        .expect("recover latest exact request context");
    assert_eq!(
        enqueue_accepted(
            &producer,
            gateway_id,
            &replay_context,
            FINALIZED_AT_MS + 250,
        )
        .expect("latest retry is an exact replay"),
        ReputationJournalEnqueueOutcomeV1::ExactReplay {
            event_id: latest_context.1
        }
    );
    assert_eq!(outbox.status().expect("status").ready, 15);
    assert_eq!(outbox.status().expect("status").completed, 1);
    assert!(matches!(
        producer.enqueue_validation(
            gateway_id,
            &replay_context,
            counted_validation(
                FINALIZED_AT_MS + 251,
                StreamTokenValidationStatusV1::ProviderViolation(
                    iroha_data_model::sorafs::reputation::StreamTokenViolationKindV1::RequestQuotaExceeded,
                ),
            ),
        ),
        Err(ReputationRuntimeError::JournalSourceConflict)
    ));
    drop(producer);
    drop(outbox);
    let restored = Arc::new(
        ReputationJournalProducerOutboxV1::open(temp.path(), policy)
            .expect("restore producer checkpoint"),
    );
    enqueue_accepted(
        &token_producer(Arc::clone(&restored)),
        gateway_id,
        &request_context("restart-nonce-17"),
        FINALIZED_AT_MS + 300,
    )
    .expect("post-restart validation");
    let state = restored.state.lock().expect("restored producer state");
    assert_eq!(
        state
            .checkpoint
            .stream_token_gateway_heads
            .iter()
            .find(|head| head.binding.gateway_id == gateway_id)
            .expect("restored gateway head")
            .binding
            .gateway_sequence,
        17
    );
}
