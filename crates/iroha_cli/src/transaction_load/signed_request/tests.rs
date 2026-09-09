//! Canonical framing, bounded writes and durable readiness controls without a network.

use super::*;
use iroha::data_model::transaction::signed::TransactionBuilder;
use std::{io, task::Poll};

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}

fn plan(sequence: usize) -> Planned {
    Planned {
        cohort: Cohort::Measurement,
        sequence,
        logical_id: hex::encode(Sha256::digest(format!("fixture:{sequence}"))),
        scheduled_offset_ns: sequence as i64 * NS,
        account_index: sequence % 4,
    }
}

fn transaction() -> SignedTransaction {
    transaction_with_metadata(Metadata::default())
}

fn transaction_with_metadata(metadata: Metadata) -> SignedTransaction {
    let keys = KeyPair::try_from_seed(vec![71; 32], Algorithm::Ed25519).unwrap();
    let authority = AccountId::new(keys.public_key().clone());
    TransactionBuilder::new(
        "hash:A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5A5#95D7"
            .parse()
            .unwrap(),
        authority.clone(),
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_executable(workload::executable(&authority, &plan(1)).unwrap())
    .with_metadata(metadata)
    .try_sign(keys.private_key())
    .unwrap()
}

// Framing-only controls deliberately use arbitrary bytes. Canonical transaction
// identity and decoding are exercised separately against the real signed fixture.
fn framing_request(length: usize) -> SignedRequest {
    let bytes = vec![0xab; length];
    SignedRequest {
        index: 0,
        plan: plan(1),
        hash: transaction().hash(),
        digest: hex::encode(Sha256::digest(&bytes)),
        bytes,
    }
}

#[derive(Default)]
struct Writer {
    bytes: Vec<u8>,
    fail_after: Option<usize>,
    fail_flush: bool,
    fail_sync: bool,
    flushed: bool,
    synced: bool,
}
impl Write for Writer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let available = self
            .fail_after
            .map_or(bytes.len(), |limit| limit.saturating_sub(self.bytes.len()));
        if available == 0 {
            return Err(io::Error::other("injected short write"));
        }
        let count = available.min(bytes.len());
        self.bytes.extend_from_slice(&bytes[..count]);
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        if self.fail_flush {
            return Err(io::Error::other("injected flush failure"));
        }
        self.flushed = true;
        Ok(())
    }
}
impl DurableWriter for Writer {
    fn sync_request(&mut self) -> io::Result<()> {
        self.flush()?;
        if self.fail_sync {
            return Err(io::Error::other("injected sync failure"));
        }
        self.synced = true;
        Ok(())
    }
}

fn events(bytes: &[u8]) -> Vec<Value> {
    bytes
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| json::from_slice(line).unwrap())
        .collect()
}

#[test]
fn capture_uses_exact_canonical_frame_of_the_sdk_signed_transaction() {
    let tx = transaction();
    let expected = norito::encode_canonical(&tx).unwrap();
    let captured = capture(tx.clone(), plan(1), 4).unwrap();
    assert_eq!(captured.request.bytes, expected);
    assert_eq!(
        captured.request.bytes.len(),
        norito::canonical_frame_len(&tx).unwrap()
    );
    assert_eq!(captured.request.hash, tx.hash());
    assert_eq!(captured.payload.hash(), tx.hash());
    assert_ne!(captured.payload.as_bytes(), captured.request.bytes);
    let decoded: SignedTransaction = norito::decode_canonical(&captured.request.bytes).unwrap();
    assert_eq!(decoded.hash(), tx.hash());
    assert!(norito::decode_canonical::<SignedTransaction>(captured.payload.as_bytes()).is_err());
    assert_eq!(
        captured.request.digest,
        hex::encode(Sha256::digest(&expected))
    );
}

#[test]
fn capture_binds_global_index_and_rejects_invalid_plan_before_retention() {
    assert_eq!(
        capture(transaction(), plan(1), 4).unwrap().request.index(),
        4
    );
    let mut warmup = plan(4);
    warmup.cohort = Cohort::Warmup;
    assert_eq!(
        capture(transaction(), warmup.clone(), 4)
            .unwrap()
            .request
            .index(),
        3
    );
    assert!(capture(transaction(), warmup, 3).is_err());
    for invalid in [
        Planned {
            sequence: 0,
            ..plan(1)
        },
        Planned {
            account_index: 64,
            ..plan(1)
        },
        Planned {
            logical_id: "A".repeat(64),
            ..plan(1)
        },
        Planned {
            logical_id: "a".repeat(65),
            ..plan(1)
        },
        Planned {
            sequence: MAX_ROWS + 1,
            ..plan(1)
        },
    ] {
        assert!(capture(transaction(), invalid, 0).is_err());
    }
    assert!(capture(transaction(), plan(2), usize::MAX).is_err());
}

#[test]
fn actual_oversized_signed_frame_is_rejected_before_retained_encoding() {
    let mut metadata = Metadata::default();
    metadata.insert(
        "large_fixture".parse::<Name>().unwrap(),
        "x".repeat(MAX_SIGNED_REQUEST_BYTES),
    );
    let tx = transaction_with_metadata(metadata);
    assert!(norito::canonical_frame_len(&tx).unwrap() > MAX_SIGNED_REQUEST_BYTES);
    assert!(capture(tx, plan(1), 0).is_err());
}

#[test]
fn fixed_chunk_segmentation_and_closed_record_fields_cover_all_boundaries() {
    for length in [1, 4095, 4096, 4097, MAX_SIGNED_REQUEST_BYTES] {
        let request = framing_request(length);
        let mut bytes = Vec::new();
        request
            .encoded_records(|record| {
                assert!(record.len() <= 16 * 1024 + 1);
                assert_eq!(record.last(), Some(&b'\n'));
                bytes.extend_from_slice(record);
                Ok(())
            })
            .unwrap();
        let rows = events(&bytes);
        assert_eq!(rows.len(), length.div_ceil(4096) + 2);
        assert_eq!(rows[0]["event"].as_str(), Some("signed_request_begin"));
        assert_eq!(rows[0].as_object().unwrap().len(), 8);
        assert_eq!(
            rows[0]["encoding"].as_str(),
            Some("norito.canonical.signed_transaction.v1")
        );
        assert_eq!(rows.last().unwrap().as_object().unwrap().len(), 6);
        let mut reconstructed = Vec::new();
        for (index, row) in rows[1..rows.len() - 1].iter().enumerate() {
            assert_eq!(row.as_object().unwrap().len(), 5);
            assert_eq!(row["chunk_index"].as_u64(), Some(index as u64));
            assert_eq!(row["offset"].as_u64(), Some((index * 4096) as u64));
            let chunk = hex::decode(row["bytes_hex"].as_str().unwrap()).unwrap();
            assert_eq!(chunk.len(), 4096.min(length - index * 4096));
            reconstructed.extend(chunk);
        }
        assert_eq!(reconstructed, request.bytes);
        assert_eq!(
            rows.last().unwrap()["canonical_sha256"].as_str(),
            Some(request.digest.as_str())
        );
    }
    for length in [0, MAX_SIGNED_REQUEST_BYTES + 1] {
        let mut calls = 0;
        assert!(
            framing_request(length)
                .encoded_records(|_| {
                    calls += 1;
                    Ok(())
                })
                .is_err()
        );
        assert_eq!(calls, 0);
    }
}

#[test]
fn full_expanded_request_and_existing_prefix_fit_exactly_or_write_nothing() {
    let request = framing_request(4097);
    let mut required = 0;
    request
        .encoded_records(|bytes| {
            required += bytes.len();
            Ok(())
        })
        .unwrap();
    assert!(required > 4097 * 2);
    for limit in [0, required + 6, MAX_FILE_BYTES + 1] {
        let mut writer = Writer::default();
        let mut written = 7;
        assert!(
            framing_request(4097)
                .persist(&mut writer, &mut written, limit)
                .is_err()
        );
        assert!(writer.bytes.is_empty());
        assert!(!writer.flushed && !writer.synced);
        assert_eq!(written, 7);
    }
    let mut writer = Writer::default();
    let mut written = 7;
    let receipt = request
        .persist(&mut writer, &mut written, required + 7)
        .unwrap();
    assert_eq!(receipt.index, 0);
    assert_eq!(written, required + 7);
    assert_eq!(writer.bytes.len(), required);
    assert!(writer.flushed && writer.synced);
}

#[test]
fn partial_write_returns_no_receipt_and_keeps_bounded_failed_evidence() {
    let mut writer = Writer {
        fail_after: Some(37),
        ..Writer::default()
    };
    let mut written = 0;
    assert!(
        framing_request(4097)
            .persist(&mut writer, &mut written, MAX_FILE_BYTES)
            .is_err()
    );
    assert_eq!(writer.bytes.len(), 37);
    assert_eq!(written, 0);
    assert!(!writer.flushed && !writer.synced);
}

#[test]
fn terminal_record_without_successful_flush_does_not_create_a_receipt() {
    let mut writer = Writer {
        fail_flush: true,
        ..Writer::default()
    };
    assert!(
        framing_request(1)
            .persist(&mut writer, &mut 0, MAX_FILE_BYTES)
            .is_err()
    );
    assert_eq!(
        events(&writer.bytes).last().unwrap()["event"].as_str(),
        Some("signed_request_retained")
    );
    assert!(!writer.flushed && !writer.synced);
}

#[test]
fn terminal_record_without_successful_fsync_does_not_create_a_receipt() {
    let mut writer = Writer {
        fail_sync: true,
        ..Writer::default()
    };
    assert!(
        framing_request(1)
            .persist(&mut writer, &mut 0, MAX_FILE_BYTES)
            .is_err()
    );
    assert_eq!(
        events(&writer.bytes).last().unwrap()["event"].as_str(),
        Some("signed_request_retained")
    );
    assert!(writer.flushed && !writer.synced);
}

#[test]
fn production_preparation_stays_pending_until_the_actual_receipt_is_sent() {
    runtime().block_on(async {
        let (send, receive) = mpsc::sync_channel(1);
        let journal = JournalSender(send);
        let captured = capture(transaction(), plan(1), 0).unwrap();
        let expected = captured.payload.hash();
        let mut future = Box::pin(captured.retain(&journal));
        let mut submits = 0;
        assert!(matches!(futures::poll!(future.as_mut()), Poll::Pending));
        let JournalCommand::RetainSignedRequest(request, acknowledgment) =
            receive.try_recv().unwrap()
        else {
            panic!("typed retention command")
        };
        let mut writer = Writer::default();
        let receipt = request
            .persist(&mut writer, &mut 0, MAX_FILE_BYTES)
            .unwrap();
        assert!(writer.synced);
        assert!(matches!(futures::poll!(future.as_mut()), Poll::Pending));
        assert_eq!(submits, 0);
        assert!(acknowledgment.send(receipt).is_ok());
        let durable = future.await.unwrap();
        assert_eq!(durable.hash(), expected);
        let transport = durable.into_transport(expected).unwrap();
        submits += 1;
        assert_eq!(transport.hash(), expected);
        assert_eq!(submits, 1);
    });
}

#[test]
fn closed_or_saturated_queue_never_returns_a_submit_eligible_payload() {
    runtime().block_on(async {
        let (send, receive) = mpsc::sync_channel(1);
        let journal = JournalSender(send);
        journal
            .record(norito::json!({"event": "occupied"}))
            .unwrap();
        assert!(
            capture(transaction(), plan(1), 0)
                .unwrap()
                .retain(&journal)
                .await
                .is_err()
        );
        drop(receive);
        assert!(
            capture(transaction(), plan(2), 0)
                .unwrap()
                .retain(&journal)
                .await
                .is_err()
        );
    });
}

#[test]
fn dropped_or_foreign_receipt_never_returns_a_submit_eligible_payload() {
    runtime().block_on(async {
        for foreign in [false, true] {
            let (send, receive) = mpsc::sync_channel(1);
            let journal = JournalSender(send);
            let captured = capture(transaction(), plan(1), 0).unwrap();
            let hash = captured.payload.hash();
            let mut future = Box::pin(captured.retain(&journal));
            assert!(matches!(futures::poll!(future.as_mut()), Poll::Pending));
            let JournalCommand::RetainSignedRequest(_, acknowledgment) =
                receive.try_recv().unwrap()
            else {
                panic!("typed retention command")
            };
            if foreign {
                assert!(
                    acknowledgment
                        .send(DurableReceipt { index: 99, hash })
                        .is_ok()
                );
            } else {
                drop(acknowledgment);
            }
            assert!(future.await.is_err());
        }
    });
}

#[test]
fn durable_transport_rejects_a_substituted_transaction_hash() {
    let captured = capture(transaction(), plan(1), 0).unwrap();
    let mut writer = Writer::default();
    let receipt = captured
        .request
        .persist(&mut writer, &mut 0, MAX_FILE_BYTES)
        .unwrap();
    let durable = DurablyPrepared {
        payload: captured.payload,
        receipt,
    };
    let foreign = TransactionHash::from_untyped_unchecked(iroha_crypto::Hash::new(
        b"foreign submitted transaction",
    ));
    assert!(durable.into_transport(foreign).is_err());
}

#[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
#[test]
fn actual_journal_retains_canonical_bytes_before_prepared_and_offer() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory
        .path()
        .canonicalize()
        .unwrap()
        .join("journal.jsonl");
    let journal = Journal::start(
        &path,
        16,
        allocation::tests::writers(MAX_FILE_BYTES, MAX_FILE_BYTES).journal,
    )
    .unwrap();
    let tx = transaction();
    let expected = norito::encode_canonical(&tx).unwrap();
    let durable = runtime()
        .block_on(
            capture(tx, plan(1), 0)
                .unwrap()
                .retain(journal.sender.as_ref()),
        )
        .unwrap();
    let before_prepared = events(&std::fs::read(&path).unwrap());
    assert_eq!(
        before_prepared.last().unwrap()["event"].as_str(),
        Some("signed_request_retained")
    );
    let restored: Vec<u8> = before_prepared
        .iter()
        .filter(|row| row["event"].as_str() == Some("signed_request_chunk"))
        .flat_map(|row| hex::decode(row["bytes_hex"].as_str().unwrap()).unwrap())
        .collect();
    assert_eq!(restored, expected);
    let hash = durable.hash();
    journal.blocking_record(norito::json!({"event": "prepared", "index": 0, "hash": (hash.to_string()), "offset_ns": 0})).unwrap();
    let transport = durable.into_transport(hash).unwrap();
    assert_eq!(transport.hash(), hash);
    journal
        .blocking_record(norito::json!({"event": "offer", "index": 0, "offset_ns": 0}))
        .unwrap();
    journal.finish().unwrap();
    let rows = events(&std::fs::read(&path).unwrap());
    assert_eq!(rows[rows.len() - 2]["event"].as_str(), Some("prepared"));
    assert_eq!(rows.last().unwrap()["event"].as_str(), Some("offer"));
}

#[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
#[test]
fn actual_journal_duplicate_request_fails_collection_without_second_receipt() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory
        .path()
        .canonicalize()
        .unwrap()
        .join("journal.jsonl");
    let journal = Journal::start(
        &path,
        2,
        allocation::tests::writers(MAX_FILE_BYTES, MAX_FILE_BYTES).journal,
    )
    .unwrap();
    let runtime = runtime();
    assert!(
        runtime
            .block_on(
                capture(transaction(), plan(1), 0)
                    .unwrap()
                    .retain(journal.sender.as_ref())
            )
            .is_ok()
    );
    assert!(
        runtime
            .block_on(
                capture(transaction(), plan(1), 0)
                    .unwrap()
                    .retain(journal.sender.as_ref())
            )
            .is_err()
    );
    assert!(journal.finish().is_err());
    assert_eq!(
        events(&std::fs::read(&path).unwrap())
            .iter()
            .filter(|row| row["event"].as_str() == Some("signed_request_begin"))
            .count(),
        1
    );
}

#[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
#[test]
fn actual_journal_canceled_receipt_fails_before_writing_the_request() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory
        .path()
        .canonicalize()
        .unwrap()
        .join("journal.jsonl");
    let journal = Journal::start(
        &path,
        2,
        allocation::tests::writers(MAX_FILE_BYTES, MAX_FILE_BYTES).journal,
    )
    .unwrap();
    let (acknowledgment, completed) = tokio::sync::oneshot::channel();
    drop(completed);
    assert!(
        journal
            .sender
            .0
            .try_send(JournalCommand::RetainSignedRequest(
                capture(transaction(), plan(1), 0).unwrap().request,
                acknowledgment
            ))
            .is_ok()
    );
    assert!(journal.finish().is_err());
    assert!(std::fs::read(&path).unwrap().is_empty());
}

#[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
#[test]
fn bounded_concurrent_preparations_cannot_interleave_signed_request_groups() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory
        .path()
        .canonicalize()
        .unwrap()
        .join("journal.jsonl");
    let journal = Journal::start(
        &path,
        8,
        allocation::tests::writers(MAX_FILE_BYTES, MAX_FILE_BYTES).journal,
    )
    .unwrap();
    let prepared = runtime().block_on(futures::future::join_all((1..=8).rev().map(|sequence| {
        capture(transaction(), plan(sequence), 0)
            .unwrap()
            .retain(journal.sender.as_ref())
    })));
    assert!(prepared.iter().all(Result::is_ok));
    assert_eq!(prepared.len(), 8);
    journal.finish().unwrap();
    let rows = events(&std::fs::read(&path).unwrap());
    let mut active = None;
    let mut completed = BTreeSet::new();
    for row in rows {
        let index = row["index"].as_u64().unwrap();
        match row["event"].as_str().unwrap() {
            "signed_request_begin" => {
                assert!(active.replace(index).is_none());
            }
            "signed_request_chunk" => assert_eq!(active, Some(index)),
            "signed_request_retained" => {
                assert_eq!(active.take(), Some(index));
                assert!(completed.insert(index));
            }
            _ => panic!("unexpected event inside retained request command"),
        }
    }
    assert!(active.is_none());
    assert_eq!(completed, (0..8).collect());
}

#[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
fn cancel_pending_preparation_after_actual_sync(queued_later: bool) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory
        .path()
        .canonicalize()
        .unwrap()
        .join("late-cancel.jsonl");
    let (synced, reached_sync) = mpsc::sync_channel(1);
    let (resume, resumed) = mpsc::sync_channel(1);
    let wait_bound = std::time::Duration::from_secs(30);
    let journal = Journal::start_with_post_sync_hook(
        &path,
        2,
        allocation::tests::writers(MAX_FILE_BYTES, MAX_FILE_BYTES).journal,
        move |index| {
            // This per-call hook is reached by the actual worker only after
            // request.persist has flushed and synced the owned file. A bounded
            // channel handshake makes cancellation independent of timing races.
            if index == 0 {
                synced.send(index).unwrap();
                resumed.recv_timeout(wait_bound).unwrap();
            }
        },
    )
    .unwrap();
    let tx = transaction();
    let canonical = norito::encode_canonical(&tx).unwrap();
    runtime().block_on(async {
        let sender = journal.sender.clone();
        let mut first = Box::pin(capture(tx, plan(1), 0).unwrap().retain(sender.as_ref()));
        assert!(matches!(futures::poll!(first.as_mut()), Poll::Pending));
        assert_eq!(reached_sync.recv_timeout(wait_bound).unwrap(), 0);

        let durable_rows = events(&std::fs::read(&path).unwrap());
        assert_eq!(
            durable_rows.last().unwrap()["event"].as_str(),
            Some("signed_request_retained")
        );
        assert_eq!(
            durable_rows.len(),
            canonical.len().div_ceil(CHUNK_BYTES) + 2
        );
        let retained: Vec<u8> = durable_rows
            .iter()
            .filter(|row| row["event"].as_str() == Some("signed_request_chunk"))
            .flat_map(|row| hex::decode(row["bytes_hex"].as_str().unwrap()).unwrap())
            .collect();
        assert_eq!(retained, canonical);
        assert!(matches!(futures::poll!(first.as_mut()), Poll::Pending));

        let mut later = if queued_later {
            Some(Box::pin(
                capture(transaction(), plan(2), 0)
                    .unwrap()
                    .retain(sender.as_ref()),
            ))
        } else {
            None
        };
        if let Some(pending) = later.as_mut() {
            // Its real command is queued behind the blocked post-sync send.
            assert!(matches!(futures::poll!(pending.as_mut()), Poll::Pending));
        }

        // Cancel the exact preparation future awaited by the SDK backend. It
        // has not returned DurablyPrepared despite the complete retained bytes.
        drop(first);
        resume.send(()).unwrap();
        if let Some(pending) = later.take() {
            assert!(
                tokio::time::timeout(wait_bound, pending)
                    .await
                    .expect("failed worker must close its queued acknowledgment")
                    .is_err()
            );
        }
        drop(later);
        drop(sender);
        let error = journal.finish().unwrap_err();
        assert!(
            error
                .to_string()
                .contains("signed request durable acknowledgment was canceled")
        );
    });
    let rows = events(&std::fs::read(&path).unwrap());
    assert_eq!(rows.len(), canonical.len().div_ceil(CHUNK_BYTES) + 2);
    assert!(rows.iter().all(|row| row["index"].as_u64() == Some(0)));
    assert!(
        rows.iter()
            .all(|row| !matches!(row["event"].as_str(), Some("prepared" | "offer")))
    );
    assert_eq!(
        rows.last().unwrap()["event"].as_str(),
        Some("signed_request_retained")
    );
}

#[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
#[test]
fn actual_journal_cancellation_after_fsync_rejects_the_complete_terminal_record() {
    cancel_pending_preparation_after_actual_sync(false);
}

#[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
#[test]
fn actual_journal_late_cancellation_prevents_a_queued_later_receipt() {
    cancel_pending_preparation_after_actual_sync(true);
}
