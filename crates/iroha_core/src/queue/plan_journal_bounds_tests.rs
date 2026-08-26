// Queue-plan journal allocation, corruption, and cardinality regressions.
#[test]
fn complete_corruption_and_unsupported_versions_fail_without_truncation() {
    let valid = raw_frame(&QueuePlanJournalFrameV1::Put(record("corrupt")));
    let payload_offset = usize::try_from(FRAME_HEADER_BYTES).expect("header");
    let cases = [
        {
            let mut bytes = valid.clone();
            bytes[payload_offset] ^= 0x80;
            ("payload", bytes)
        },
        {
            let mut bytes = valid.clone();
            let checksum_offset =
                bytes.len() - QUEUE_PLAN_JOURNAL_FRAME_COMMIT.len() - Hash::LENGTH;
            bytes[checksum_offset] ^= 0x80;
            ("checksum", bytes)
        },
        {
            let mut bytes = valid.clone();
            bytes[QUEUE_PLAN_JOURNAL_FRAME_MAGIC.len()] ^= 0x01;
            ("outer-version", bytes)
        },
        {
            let mut bytes = valid.clone();
            let len_offset = QUEUE_PLAN_JOURNAL_FRAME_MAGIC.len() + 2;
            bytes[len_offset] ^= 0x01;
            ("length-guard", bytes)
        },
        (
            "norito",
            encode_payload(&[0xA5, 0x5A, 0xC3], 3).expect("encode invalid Norito payload"),
        ),
        {
            let mut unsupported = record("unsupported-record");
            unsupported.version = QUEUE_PLAN_JOURNAL_VERSION + 1;
            (
                "record-version",
                raw_frame(&QueuePlanJournalFrameV1::Put(unsupported)),
            )
        },
    ];
    for (label, corrupt_frame) in cases {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join(format!("{label}.norito"));
        let mut bytes = raw_bootstrap_frame();
        bytes.extend_from_slice(&corrupt_frame);
        fs::write(&path, &bytes).expect("write corrupt case");
        assert!(open(&path).is_err(), "{label} must fail closed");
        assert_eq!(fs::read(&path).expect("retain evidence"), bytes, "{label}");
    }
}
#[test]
fn full_length_uncommitted_body_checksum_and_marker_tears_truncate_only_terminal_frame() {
    let committed_record = record("committed-before-full-length-tear");
    let committed = raw_frame(&QueuePlanJournalFrameV1::Put(committed_record.clone()));
    let terminal = raw_frame(&QueuePlanJournalFrameV1::Put(record(
        "full-length-uncommitted-terminal",
    )));
    let header = usize::try_from(FRAME_HEADER_BYTES).expect("header");
    let commit_start = terminal
        .len()
        .checked_sub(QUEUE_PLAN_JOURNAL_FRAME_COMMIT.len())
        .expect("commit start");
    let checksum_start = commit_start
        .checked_sub(Hash::LENGTH)
        .expect("checksum start");
    let mut body_tear = terminal.clone();
    body_tear[header] ^= 0x80;
    body_tear[commit_start..].fill(0);
    let mut checksum_tear = terminal.clone();
    checksum_tear[checksum_start] ^= 0x80;
    checksum_tear[commit_start..].fill(0);
    let mut marker_tear = terminal;
    marker_tear[commit_start..].fill(0);
    let cases = [
        ("body", body_tear),
        ("checksum", checksum_tear),
        ("commit", marker_tear),
    ];
    for (case, torn) in cases {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join(format!("full-length-{case}-tear.norito"));
        let mut expected = raw_bootstrap_frame();
        expected.extend_from_slice(&committed);
        let mut bytes = expected.clone();
        bytes.extend_from_slice(&torn);
        fs::write(&path, bytes).expect("write full-length staged tear");
        let journal =
            open(&path).unwrap_or_else(|error| panic!("repair full-length {case} tear: {error}"));
        assert_eq!(
            journal.replay().expect("replay committed owner"),
            vec![committed_record.clone()],
            "case={case}"
        );
        drop(journal);
        assert_eq!(
            fs::read(&path).expect("read repaired history"),
            expected,
            "case={case}"
        );
    }
}
#[test]
fn invalid_commit_marker_is_never_repaired_in_the_middle_of_history() {
    let mut invalid = raw_frame(&QueuePlanJournalFrameV1::Put(record(
        "mid-history-invalid-commit",
    )));
    let commit_start = invalid
        .len()
        .checked_sub(QUEUE_PLAN_JOURNAL_FRAME_COMMIT.len())
        .expect("commit start");
    invalid[commit_start..].fill(0);
    let following = raw_frame(&QueuePlanJournalFrameV1::Put(record(
        "after-mid-history-invalid-commit",
    )));
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("mid-history-invalid-commit.norito");
    let mut bytes = raw_bootstrap_frame();
    bytes.extend_from_slice(&invalid);
    bytes.extend_from_slice(&following);
    fs::write(&path, &bytes).expect("write invalid mid-history marker");
    let error = open(&path)
        .err()
        .expect("invalid mid-history marker must fail closed");
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert_eq!(fs::read(&path).expect("retain corrupt history"), bytes);
}
#[test]
fn oversized_declared_frame_and_file_fail_before_allocation() {
    let dir = tempfile::tempdir().expect("tempdir");
    let oversized_frame = dir.path().join("oversized-frame.norito");
    let declared = u32::try_from(TEST_MAX_BYTES + 1).expect("declared length");
    let mut header = QUEUE_PLAN_JOURNAL_FRAME_MAGIC.to_vec();
    header.extend_from_slice(&QUEUE_PLAN_JOURNAL_FRAME_FORMAT_VERSION.to_le_bytes());
    header.extend_from_slice(&declared.to_le_bytes());
    header.extend_from_slice(&(!declared).to_le_bytes());
    let mut oversized_bytes = raw_bootstrap_frame();
    oversized_bytes.extend_from_slice(&header);
    fs::write(&oversized_frame, &oversized_bytes).expect("write oversized header");
    assert_eq!(
        open(&oversized_frame)
            .err()
            .expect("oversized frame")
            .kind(),
        io::ErrorKind::InvalidData
    );
    let oversized_file = dir.path().join("oversized-file.norito");
    let file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&oversized_file)
        .expect("create oversized file");
    file.set_len(TEST_MAX_BYTES + 1).expect("extend file");
    drop(file);
    assert_eq!(
        open(&oversized_file).err().expect("oversized file").kind(),
        io::ErrorKind::InvalidData
    );
}
#[test]
fn decode_budget_accepts_exact_wire_limit_and_rejects_one_byte_over() {
    assert_eq!(
        frame_decode_allocation_budget(usize::MAX),
        None,
        "allocation-budget arithmetic must fail closed instead of saturating"
    );
    let frame =
        QueuePlanJournalFrameV1::Put(record_with_message("decode-budget", "x".repeat(256 * 1024)));
    let payload = norito::encode_canonical(&frame).expect("encode large canonical frame payload");
    let payload_len = u64::try_from(payload.len()).expect("payload length fits u64");
    let exact_limits = QueuePlanJournalLimits::new(1, payload_len, TEST_MAX_BYTES, 1);
    let configured_element_budget = payload
        .len()
        .saturating_mul(FRAME_DECODE_ELEMENT_AMPLIFICATION_LIMIT);
    let configured_allocation_budget =
        frame_decode_allocation_budget(payload.len()).expect("fixture allocation budget");
    let (minimum_element_budget, minimum_allocation_budget) = minimum_decode_budgets(&payload);
    assert!(
        configured_element_budget >= minimum_element_budget,
        "configured element budget {configured_element_budget} is below measured canonical minimum {minimum_element_budget}"
    );
    assert!(
        configured_element_budget.saturating_sub(minimum_element_budget) <= payload.len(),
        "configured element budget {configured_element_budget} must remain within one frame ({}) of the measured minimum {minimum_element_budget}",
        payload.len()
    );
    assert!(
        configured_allocation_budget >= minimum_allocation_budget,
        "configured allocation budget {configured_allocation_budget} is below measured canonical minimum {minimum_allocation_budget}"
    );
    assert!(
        configured_allocation_budget.saturating_sub(minimum_allocation_budget)
            <= payload
                .len()
                .saturating_add(FRAME_DECODE_ALLOCATION_FIXED_OVERHEAD_BYTES),
        "configured allocation budget {configured_allocation_budget} must remain within one frame plus fixed metadata overhead ({}) of the measured minimum {minimum_allocation_budget}",
        payload
            .len()
            .saturating_add(FRAME_DECODE_ALLOCATION_FIXED_OVERHEAD_BYTES)
    );
    assert_eq!(
        decode_frame(&payload, exact_limits).expect("decode at exact configured wire limit"),
        frame
    );
    let one_byte_under = QueuePlanJournalLimits::new(1, payload_len - 1, TEST_MAX_BYTES, 1);
    let error = decode_frame(&payload, one_byte_under)
        .expect_err("one byte above the configured frame limit must fail before decode");
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert!(
        error
            .to_string()
            .contains("exceeds the configured frame limit")
    );
}
#[test]
fn decode_budget_covers_maximum_native_contract_upload_chunk() {
    // This is the production shape emitted for every non-final native
    // contract artifact chunk. Its 64 KiB byte vector is wrapped by a
    // dynamic InstructionBox inside a signed transaction, which exercises
    // more owned decode bookkeeping than the Log and flat-vector fixtures.
    let chunk = vec![0xA5; SMART_CONTRACT_CODE_CHUNK_BYTES];
    let frame = QueuePlanJournalFrameV1::Put(record_with_instructions(
        "native-contract-upload",
        [InstructionBox::from(UploadSmartContractCodeChunk {
            code_hash: Hash::new(b"native-contract-upload-artifact"),
            total_size: u64::try_from(SMART_CONTRACT_CODE_CHUNK_BYTES * 2)
                .expect("fixture size fits u64"),
            chunk_index: 0,
            chunk_count: 2,
            chunk,
        })],
    ));
    let payload =
        norito::encode_canonical(&frame).expect("encode native contract upload journal frame");
    assert!(
        payload.len() > SMART_CONTRACT_CODE_CHUNK_BYTES,
        "fixture must include the complete signed transaction and journal envelope"
    );
    let canonical_limits = norito::canonical_decode_limits(payload.len());
    let insufficient_element_budget = payload.len();
    let insufficient_allocation_budget = payload
        .len()
        .checked_mul(26)
        .and_then(|bytes| bytes.checked_add(64 * 1024))
        .expect("insufficient fixture allocation budget");
    assert!(
        matches!(
            decode_frame_with_budgets(
                &payload,
                insufficient_element_budget,
                canonical_limits.max_total_allocated_bytes(),
            ),
            Err(norito::Error::TotalElementsExceeded { .. })
        ),
        "the narrow one-element-per-wire-byte envelope must reject the native upload"
    );
    assert!(
        matches!(
            decode_frame_with_budgets(
                &payload,
                canonical_limits.max_total_elements(),
                insufficient_allocation_budget,
            ),
            Err(norito::Error::TotalAllocationExceeded { .. })
        ),
        "the narrow 26x-plus-64-KiB envelope must reject the native upload"
    );
    let payload_len = u64::try_from(payload.len()).expect("payload length fits u64");
    let exact_limits = QueuePlanJournalLimits::new(
        1,
        payload_len,
        payload_len
            .checked_add(FRAME_HEADER_BYTES)
            .and_then(|bytes| bytes.checked_add(FRAME_TRAILER_BYTES))
            .expect("fixture framed length fits u64"),
        1,
    );
    assert_eq!(
        decode_frame(&payload, exact_limits)
            .expect("decode maximum native upload chunk at production limits"),
        frame
    );
}
#[test]
fn decode_budget_covers_maximum_allocation_dense_instruction_vector() {
    const CALIBRATION_INSTRUCTION_COUNT: usize = 4_096;
    let calibration_instructions =
        std::iter::repeat_with(|| InstructionBox::from(Log::new(Level::INFO, String::new())))
            .take(CALIBRATION_INSTRUCTION_COUNT);
    let calibration_frame = QueuePlanJournalFrameV1::Put(record_with_instructions(
        "allocation-calibration",
        calibration_instructions,
    ));
    let calibration_payload =
        norito::encode_canonical(&calibration_frame).expect("encode allocation calibration frame");
    let configured_element_budget = calibration_payload
        .len()
        .checked_mul(FRAME_DECODE_ELEMENT_AMPLIFICATION_LIMIT)
        .expect("calibration element budget");
    let configured_allocation_budget = frame_decode_allocation_budget(calibration_payload.len())
        .expect("calibration allocation budget");
    let (minimum_element_budget, minimum_allocation_budget) =
        minimum_decode_budgets(&calibration_payload);
    assert!(
        configured_element_budget >= minimum_element_budget,
        "configured element budget {configured_element_budget} is below the allocation-dense minimum {minimum_element_budget} for {CALIBRATION_INSTRUCTION_COUNT} instructions and {} wire bytes",
        calibration_payload.len()
    );
    assert!(
        configured_allocation_budget >= minimum_allocation_budget,
        "configured allocation budget {configured_allocation_budget} is below the allocation-dense minimum {minimum_allocation_budget} for {CALIBRATION_INSTRUCTION_COUNT} instructions and {} wire bytes",
        calibration_payload.len()
    );
    let instruction_count =
        usize::try_from(iroha_config::parameters::defaults::transaction::max_instructions().get())
            .expect("default transaction instruction limit fits usize");
    assert_eq!(
        instruction_count, 100_000,
        "fixture must track the production admission maximum"
    );
    let instructions =
        std::iter::repeat_with(|| InstructionBox::from(Log::new(Level::INFO, String::new())))
            .take(instruction_count);
    let frame =
        QueuePlanJournalFrameV1::Put(record_with_instructions("allocation-dense", instructions));
    let payload = norito::encode_canonical(&frame).expect("encode allocation-dense frame");
    let payload_len = u64::try_from(payload.len()).expect("payload length fits u64");
    let exact_limits = QueuePlanJournalLimits::new(
        1,
        payload_len,
        payload_len
            .checked_add(FRAME_HEADER_BYTES)
            .and_then(|bytes| bytes.checked_add(FRAME_TRAILER_BYTES))
            .expect("fixture framed length fits u64"),
        1,
    );
    assert_eq!(
        decode_frame(&payload, exact_limits)
            .expect("decode allocation-dense frame at configured limits"),
        frame
    );
}
#[test]
fn compressed_frame_is_rejected_before_owned_decompression() {
    let frame = QueuePlanJournalFrameV1::Put(record_with_message(
        "compressed-frame",
        "z".repeat(2 * 1024 * 1024),
    ));
    let canonical = norito::encode_canonical(&frame).expect("encode canonical frame");
    let compressed =
        norito::to_compressed_bytes(&frame, Some(norito::CompressionConfig::default()))
            .expect("compress frame");
    assert!(
        compressed.len().saturating_mul(16) < canonical.len(),
        "fixture must have decompression amplification"
    );
    let limits = QueuePlanJournalLimits::new(
        1,
        u64::try_from(compressed.len()).expect("compressed length fits u64"),
        TEST_MAX_BYTES,
        1,
    );
    let error = decode_frame(&compressed, limits).expect_err("compressed journal frame must fail");
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert!(
        error.to_string().contains("unsupported compression"),
        "compressed input must fail during the uncompressed archive preflight: {error}"
    );
}
#[test]
fn replay_rejects_exactly_one_distinct_identity_above_live_bound() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("live-bound.norito");
    {
        let mut journal = QueuePlanJournal::open_with_limits(&path, limits(2), true).expect("open");
        for label in ["one", "two"] {
            journal
                .put_deferred_flush(record(label))
                .expect("append bounded record");
        }
        journal.sync_all_with_parent().expect("sync");
    }
    let journal = QueuePlanJournal::open_with_limits(&path, limits(1), true).expect("reopen");
    let error = journal
        .prepare_replay()
        .err()
        .expect("max + 1 distinct identities must fail");
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert!(
        error
            .to_string()
            .contains("distinct live-record reconstruction limit exceeded"),
        "unexpected bound error: {error}"
    );
}
#[test]
fn replay_rejects_transient_distinct_identity_amplification_even_if_final_set_is_small() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("transient-live-prefix.norito");
    let records = (0..16)
        .map(|index| record(&format!("transient-{index}")))
        .collect::<Vec<_>>();
    {
        let mut journal =
            QueuePlanJournal::open_with_limits(&path, limits(records.len()), true).expect("open");
        for record in &records {
            journal
                .put_deferred_flush(record.clone())
                .expect("append transient owner");
        }
        journal
            .remove_many_deferred_flush(records[..records.len() - 1].iter().map(|record| {
                (
                    record.entrypoint_hash,
                    record.plan_digest(),
                    record.claim_digest().expect("hash transient claim"),
                )
            }))
            .expect("append delayed tombstones");
        journal.sync_all_with_parent().expect("sync fixture");
    }
    let journal = QueuePlanJournal::open_with_limits(&path, limits(1), true).expect("reopen");
    let error = journal
        .prepare_replay()
        .err()
        .expect("transient reconstruction amplification must fail");
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert!(
        error
            .to_string()
            .contains("distinct live-record reconstruction limit exceeded"),
        "unexpected transient-bound error: {error}"
    );
}
#[test]
fn replay_allows_long_put_remove_history_with_bounded_live_cardinality() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("bounded-long-history.norito");
    let mut journal = QueuePlanJournal::open_with_limits(&path, limits(1), true).expect("open");
    for index in 0..64 {
        let historical = record(&format!("historical-{index}"));
        journal
            .put_deferred_flush(historical.clone())
            .expect("append historical Put");
        journal
            .remove_many_deferred_flush([(
                historical.entrypoint_hash,
                historical.plan_digest(),
                historical.claim_digest().expect("hash historical claim"),
            )])
            .expect("append matching historical Remove");
    }
    let live = record("long-history-live");
    journal
        .put_deferred_flush(live.clone())
        .expect("append final live owner");
    journal.sync_all_with_parent().expect("sync history");
    assert_eq!(
        journal.replay().expect("replay bounded history"),
        vec![live]
    );
}
#[test]
fn replay_same_entrypoint_replacements_do_not_grow_cardinality_and_stale_remove_is_ignored() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("same-entrypoint-replacements.norito");
    let original = record("same-entrypoint");
    let original_digest = original.plan_digest();
    let mut latest = original.clone();
    let mut journal = QueuePlanJournal::open_with_limits(&path, limits(1), true).expect("open");
    journal
        .put_deferred_flush(original.clone())
        .expect("append original plan");
    for route in 1..=128 {
        latest = with_single_route(original.clone(), route, route.saturating_add(1_000));
        journal
            .put_deferred_flush(latest.clone())
            .expect("append same-entrypoint replacement");
    }
    journal
        .remove_many_deferred_flush([(
            original.entrypoint_hash,
            original_digest,
            original.claim_digest().expect("hash original claim"),
        )])
        .expect("append stale original-plan Remove");
    journal.sync_all_with_parent().expect("sync replacements");
    assert_eq!(
        journal.replay().expect("replay latest replacement"),
        vec![latest],
        "a stale plan-specific Remove must not delete the latest plan for the same entrypoint"
    );
}
#[test]
fn replacement_preserves_original_fifo_ownership_through_compaction_and_reopen() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("replacement-fifo-compaction.norito");
    let first = record("replacement-fifo-first");
    let second = record("replacement-fifo-second");
    let replacement = with_single_route(first.clone(), 17, 29);
    let compact_limits = QueuePlanJournalLimits::new(
        u64::try_from(raw_bootstrap_frame().len()).expect("bootstrap length"),
        TEST_MAX_BYTES,
        TEST_MAX_BYTES,
        2,
    );
    let mut journal =
        QueuePlanJournal::open_with_limits(&path, compact_limits, true).expect("open");
    for record in [&first, &second, &replacement] {
        journal
            .put_deferred_flush(record.clone())
            .expect("append FIFO fixture");
    }
    journal.sync_all_with_parent().expect("sync FIFO fixture");
    assert_eq!(
        journal.replay().expect("replay before compaction"),
        vec![replacement.clone(), second.clone()],
        "replacing A after B must preserve A's original ownership position"
    );
    journal
        .compact_if_needed()
        .expect("compact replacement history");
    assert_eq!(
        read_frames(&path, compact_limits).expect("read compacted FIFO frames"),
        vec![
            QueuePlanJournalFrameV1::Put(replacement.clone()),
            QueuePlanJournalFrameV1::Put(second.clone()),
        ]
    );
    drop(journal);
    let reopened = QueuePlanJournal::open_with_limits(&path, compact_limits, true).expect("reopen");
    assert_eq!(
        reopened.replay().expect("replay compacted FIFO history"),
        vec![replacement, second]
    );
}
#[test]
fn matching_remove_ends_ownership_before_same_entrypoint_is_reinserted() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("replacement-fifo-remove-reinsert.norito");
    let first = record("replacement-remove-first");
    let second = record("replacement-remove-second");
    let replacement = with_single_route(first.clone(), 31, 37);
    let reinserted = with_single_route(first.clone(), 41, 43);
    let mut journal = QueuePlanJournal::open_with_limits(&path, limits(2), true).expect("open");
    for record in [&first, &second, &replacement] {
        journal
            .put_deferred_flush(record.clone())
            .expect("append ownership fixture");
    }
    journal
        .remove_many_deferred_flush([(
            replacement.entrypoint_hash,
            replacement.plan_digest(),
            replacement.claim_digest().expect("hash replacement claim"),
        )])
        .expect("remove latest ownership");
    journal
        .put_deferred_flush(reinserted.clone())
        .expect("reinsert same entrypoint");
    journal
        .sync_all_with_parent()
        .expect("sync ownership fixture");
    assert_eq!(
        journal.replay().expect("replay reset ownership"),
        vec![second, reinserted],
        "a Put after a matching Remove acquires a new FIFO ownership position"
    );
}
