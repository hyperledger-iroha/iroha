// Canonical PoR publisher frames paired with existing strict readers.

fn por_publisher_layouts() -> Vec<u8> {
    let layouts: Vec<_> = (0..=u8::MAX)
        .filter(|flags| norito::core::validate_header_flags(*flags).is_ok())
        .collect();
    assert_eq!(layouts.len(), 10, "exercise every valid V1 layout");
    layouts
}

fn assert_por_pending<T: norito::NoritoSerialize + Clone>(
    kind: GovernanceOutboxKindV1,
    value: &T,
    publish: fn(&NodeHandle, T) -> Result<(), GovernancePublishError>,
) {
    let (cfg, _dir) = storage_config_with_temp_dir();
    let path = auxiliary_runtime_checkpoint_path(cfg.data_dir());
    let canonical = norito::encode_canonical(value).expect("independent canonical payload");
    let digest = *blake3::hash(&canonical).as_bytes();
    let mut handle = NodeHandle::new(cfg.clone());
    let mut initial_checkpoint = None;
    let mut initial_outbox = None;
    for flags in por_publisher_layouts() {
        let _caller = norito::core::DecodeFlagsGuard::enter(flags);
        publish(&handle, value.clone()).expect("publish without a registered target");
        publish(&handle, value.clone()).expect("exact pending retry");
        let snapshot = handle.governance_outbox.read().expect("outbox").clone();
        assert_eq!(snapshot.entries.len(), 1);
        assert_eq!(snapshot.next_sequence, 2);
        let entry = snapshot.entries.get(&1).expect("only initial sequence");
        assert_eq!(entry.kind, kind);
        assert_eq!(entry.payload_bytes, canonical);
        assert_eq!(entry.payload_digest, digest);
        validate_governance_outbox_entry(entry).expect("actual queued entry admits");
        let bytes = fs::read(&path).expect("real durable auxiliary checkpoint");
        let checkpoint: AuxiliaryRuntimeCheckpointV5 =
            norito::decode_canonical(&bytes).expect("canonical checkpoint frame");
        assert_eq!(checkpoint.governance_outbox_entries, vec![entry.clone()]);
        assert_eq!(checkpoint.governance_outbox_next_sequence, 2);
        if let Some(original) = &initial_checkpoint {
            assert_eq!(
                &bytes, original,
                "cross-layout retry must not rewrite durable state"
            );
        } else {
            initial_checkpoint = Some(bytes.clone());
            initial_outbox = Some(snapshot.clone());
        }
        assert_eq!(Some(&snapshot), initial_outbox.as_ref());
        drop(handle);
        handle = NodeHandle::try_new(cfg.clone()).expect("actual restart under caller layout");
        assert_eq!(handle.pending_governance_publication_count(), 1);
        assert_eq!(
            &*handle.governance_outbox.read().expect("restored outbox"),
            &snapshot
        );
        assert_eq!(fs::read(&path).expect("retained checkpoint"), bytes);
        assert_eq!(norito::core::get_decode_flags(), flags);
    }
    let recording = Arc::new(RecordingPublisher::default());
    handle
        .try_set_governance_publisher(recording.clone())
        .expect("dispatch pending payload");
    assert_eq!(recording.take(), vec![canonical]);
    assert_eq!(handle.pending_governance_publication_count(), 0);
    drop(handle);
    let acknowledged = NodeHandle::try_new(cfg).expect("acknowledgement survives restart");
    acknowledged
        .try_set_governance_publisher(recording.clone())
        .expect("register empty target");
    assert!(
        recording.take().is_empty(),
        "acknowledged entry is not resent"
    );
    assert_eq!(acknowledged.pending_governance_publication_count(), 0);
}

#[test]
fn por_publishers_preserve_one_exact_pending_entry_across_all_layouts_and_restart() {
    assert_por_pending(
        GovernanceOutboxKindV1::PorChallengePublication,
        &por_challenge_publication_fixture(),
        NodeHandle::publish_por_challenge_publication,
    );
    assert_por_pending(
        GovernanceOutboxKindV1::PorWeeklyReport,
        &por_weekly_report_fixture(),
        NodeHandle::publish_por_weekly_report,
    );
}

fn por_rebind_payload(entry: &mut GovernanceOutboxEntryV1, payload: Vec<u8>) {
    entry.payload_bytes = payload;
    entry.payload_digest = *blake3::hash(&entry.payload_bytes).as_bytes();
    entry.binding_digest = governance_outbox_binding_digest(
        entry.version,
        entry.sequence,
        entry.kind,
        entry.payload_digest,
        entry.provenance.as_ref(),
    );
}

fn assert_por_alternate_denial<T>(
    value: &T,
    publish: fn(&NodeHandle, T) -> Result<(), GovernancePublishError>,
    decode: fn(&[u8]) -> Result<T, norito::Error>,
) where
    T: norito::NoritoSerialize
        + for<'de> norito::NoritoDeserialize<'de>
        + Clone
        + std::fmt::Debug
        + PartialEq,
{
    let canonical = norito::encode_canonical(value).expect("canonical oracle");
    let (cfg, _dir) = storage_config_with_temp_dir();
    let path = auxiliary_runtime_checkpoint_path(cfg.data_dir());
    let handle = NodeHandle::new(cfg.clone());
    publish(&handle, value.clone()).expect("real queued positive");
    drop(handle);
    let original = fs::read(&path).expect("retained pending checkpoint");
    let mut alternatives = 0;
    for flags in por_publisher_layouts() {
        let _caller = norito::core::DecodeFlagsGuard::enter(flags);
        assert_eq!(
            decode(&canonical).expect("canonical positive before tamper"),
            *value
        );
        let alternate = norito::to_bytes(value).expect("genuine advertised alternate frame");
        assert_eq!(
            norito::decode_from_bytes::<T>(&alternate).expect("valid general frame"),
            *value
        );
        if alternate == canonical {
            continue;
        }
        alternatives += 1;
        assert!(matches!(
            decode(&alternate),
            Err(norito::Error::NonCanonicalEncoding)
        ));
        let mut checkpoint: AuxiliaryRuntimeCheckpointV5 =
            norito::decode_canonical(&original).expect("original checkpoint");
        let entry = checkpoint
            .governance_outbox_entries
            .first_mut()
            .expect("pending entry");
        // Recompute both public digests, so neither stale hash nor wrong sequence masks the
        // payload reader's exact-layout rejection during restore and actual dispatch.
        por_rebind_payload(entry, alternate);
        let recording = RecordingPublisher::default();
        let error = publish_governance_outbox_entry(&recording, entry, None)
            .expect_err("actual dispatch rejects noncanonical inner payload");
        assert!(error.to_string().contains("decode"), "{error}");
        assert!(recording.take().is_empty());
        let poisoned = norito::encode_canonical(&checkpoint).expect("canonical outer frame");
        write_local_checkpoint_atomic(&path, &poisoned).expect("private tampered checkpoint");
        assert_checkpoint_component!("auxiliary runtime" => NodeHandle::try_new(cfg.clone()));
        assert_eq!(
            fs::read(&path).expect("failed open preserves bytes"),
            poisoned
        );
    }
    assert!(
        alternatives > 0,
        "same-value noncanonical frames were exercised"
    );
    write_local_checkpoint_atomic(&path, &original).expect("restore canonical checkpoint");
    let restored = NodeHandle::try_new(cfg).expect("original canonical state still opens");
    assert_eq!(restored.pending_governance_publication_count(), 1);
}

#[test]
fn por_publishers_reject_rebound_alternate_frames_on_restore_and_dispatch() {
    assert_por_alternate_denial(
        &por_challenge_publication_fixture(),
        NodeHandle::publish_por_challenge_publication,
        decode_por_challenge_publication_v1,
    );
    assert_por_alternate_denial(
        &por_weekly_report_fixture(),
        NodeHandle::publish_por_weekly_report,
        decode_por_weekly_report_v1,
    );
}

fn assert_por_semantic_and_retention_denial<T: norito::NoritoSerialize + Clone>(
    value: &T,
    invalid: &T,
    different: &T,
    publish: fn(&NodeHandle, T) -> Result<(), GovernancePublishError>,
) {
    let (base, _dir) = storage_config_with_temp_dir();
    let cfg = enabled_storage_builder(base.data_dir().clone())
        .runtime_retention(RuntimeRetentionPolicy::new(1, 1, 1024 * 1024))
        .build();
    let path = auxiliary_runtime_checkpoint_path(cfg.data_dir());
    let handle = NodeHandle::new(cfg);
    publish(&handle, value.clone()).expect("valid one-slot admission");
    let original = fs::read(&path).expect("original checkpoint");
    let snapshot = handle.governance_outbox.read().expect("outbox").clone();
    for flags in por_publisher_layouts() {
        let _caller = norito::core::DecodeFlagsGuard::enter(flags);
        let error = publish(&handle, invalid.clone()).expect_err("semantic invalidity is retained");
        assert!(error.to_string().starts_with("invalid PoR"), "{error}");
        publish(&handle, value.clone()).expect("canonical duplicate fits full one-slot outbox");
        let error = publish(&handle, different.clone())
            .expect_err("distinct valid payload exceeds retention");
        assert!(error.to_string().contains("retention exhausted"), "{error}");
        assert_eq!(
            &*handle.governance_outbox.read().expect("unchanged outbox"),
            &snapshot
        );
        assert_eq!(fs::read(&path).expect("unchanged checkpoint"), original);
    }
    // The paired writer correction does not authorize rewritten public payload identities.
    let mut tampered = snapshot.entries.get(&1).expect("pending entry").clone();
    tampered.payload_digest[0] ^= 1;
    let error = validate_governance_outbox_entry(&tampered).expect_err("digest tamper");
    assert!(
        error.to_string().contains("payload digest mismatch"),
        "{error}"
    );
    assert_eq!(
        &*handle.governance_outbox.read().expect("unchanged outbox"),
        &snapshot
    );
}

#[test]
fn por_publishers_preserve_semantic_validation_and_retention_without_state_mutation() {
    let publication = por_challenge_publication_fixture();
    let mut invalid = publication.clone();
    invalid.duplicate_samples += 1;
    assert!(invalid.validate().is_err());
    let mut different = publication.clone();
    different.challenge.deadline_at += 1;
    different
        .validate()
        .expect("distinct valid challenge remains a real retention test");
    assert_por_semantic_and_retention_denial(
        &publication,
        &invalid,
        &different,
        NodeHandle::publish_por_challenge_publication,
    );
    let report = por_weekly_report_fixture();
    let mut invalid = report.clone();
    invalid.challenges_verified = report.challenges_total + 1;
    assert!(invalid.validate().is_err());
    let mut different = report.clone();
    different.generated_at += 1;
    different.validate().expect("distinct valid report");
    assert_por_semantic_and_retention_denial(
        &report,
        &invalid,
        &different,
        NodeHandle::publish_por_weekly_report,
    );
}

#[test]
fn publish_por_governance_payloads_use_canonical_outbox_dispatch() {
    for flags in por_publisher_layouts() {
        let _caller = norito::core::DecodeFlagsGuard::enter(flags);
        let (handle, _dir) = node_with_temp_storage();
        let publisher = Arc::new(RecordingPublisher::default());
        handle
            .try_set_governance_publisher(publisher.clone())
            .expect("register recording publisher");
        let publication = por_challenge_publication_fixture();
        let report = por_weekly_report_fixture();
        let expected_publication =
            norito::encode_canonical(&publication).expect("encode PoR challenge publication");
        let expected_report = norito::encode_canonical(&report).expect("encode PoR weekly report");
        handle
            .publish_por_challenge_publication(publication)
            .expect("publish PoR challenge publication");
        handle
            .publish_por_weekly_report(report)
            .expect("publish PoR weekly report");
        assert_eq!(
            publisher.take(),
            vec![expected_publication, expected_report]
        );
        assert_eq!(handle.pending_governance_publication_count(), 0);
    }
}
#[test]
fn por_governance_payloads_remain_ordered_and_retryable_after_publish_failure() {
    let (handle, _dir) = node_with_temp_storage();
    let failing = Arc::new(FailingPublisher::default());
    handle
        .try_set_governance_publisher(failing.clone())
        .expect("register failing publisher before enqueue");
    let publication = por_challenge_publication_fixture();
    let report = por_weekly_report_fixture();
    let expected_publication =
        norito::encode_canonical(&publication).expect("encode PoR challenge publication");
    let expected_report = norito::encode_canonical(&report).expect("encode PoR weekly report");
    handle
        .publish_por_challenge_publication(publication)
        .expect_err("challenge publish failure remains durable");
    handle
        .publish_por_weekly_report(report)
        .expect_err("report publish failure remains durable");
    assert_eq!(failing.attempts(), 2);
    assert_eq!(handle.pending_governance_publication_count(), 2);
    let recording = Arc::new(RecordingPublisher::default());
    handle
        .try_set_governance_publisher(recording.clone())
        .expect("retry queued PoR publications");
    assert_eq!(
        recording.take(),
        vec![expected_publication, expected_report]
    );
    assert_eq!(handle.pending_governance_publication_count(), 0);
}
