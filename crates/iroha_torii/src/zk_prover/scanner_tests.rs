fn scan_and_report_single_attachment() {
    configure_test_cfg(Vec::new());
    let _env = TestDataDirGuard::new();
    // Create an attachment manually
    let body = fixture_attachment_bytes();
    let id = attachment_body_id(&body);
    let tenant_key = anon_tenant_key();
    let meta = super::super::zk_attachments::AttachmentMeta {
        id: id.clone(),
        content_type: "application/x-norito".to_string(),
        size: body.len() as u64,
        created_ms: now_ms(),
        tenant: Some(tenant_key.clone()),
        provenance: Some(fixture_attachment_provenance(&body, "application/x-norito")),
        zk1_tags: None,
    };
    ensure_tenant_dir(&tenant_key);
    fs::write(attachment_bin_path(&tenant_key, &id), &body).unwrap();
    fs::write(
        attachment_meta_path(&tenant_key, &id),
        norito::json::to_json_pretty(&meta).unwrap(),
    )
    .unwrap();
    // Run one scan
    let stats = super::block_on_scan();
    assert_eq!(stats.processed_reports, 1, "one report created");
    assert_eq!(stats.budget_exhausted, None);
    let rep = load_report(&id).expect("report exists");
    assert!(rep.ok);
    assert_eq!(rep.content_type, "application/x-norito");
    assert_eq!(rep.size, body.len() as u64);
    assert_eq!(rep.backend.as_deref(), Some("halo2/ipa"));
    assert!(rep.proof_hash.is_some());
    assert!(rep.proofs.is_empty());
    assert_eq!(
        rep.latency_ms,
        rep.processed_ms.saturating_sub(rep.created_ms)
    );
}

#[test]
fn attachment_file_loading_is_bounded_and_metadata_size_is_not_trusted() {
    configure_test_cfg(Vec::new());
    let _env = TestDataDirGuard::new();
    let tenant_key = anon_tenant_key();
    ensure_tenant_dir(&tenant_key);

    let oversized_id = format!("{:064x}", 0xBAD0u64);
    let oversized_path = attachment_bin_path(&tenant_key, &oversized_id);
    fs::write(
        &oversized_path,
        vec![0_u8; PROOF_ATTACHMENT_BODY_MAX_BYTES_V1 as usize + 1],
    )
    .expect("write oversized body fixture");
    let oversized = load_attachment_body(&AttachmentLocation {
        tenant_key: tenant_key.clone(),
        id: oversized_id,
    })
    .expect("oversized regular file is classified");
    assert_eq!(
        oversized.observed_size,
        PROOF_ATTACHMENT_BODY_MAX_BYTES_V1 + 1
    );
    assert_eq!(oversized.bytes_read, 0);
    assert!(
        oversized
            .body
            .expect_err("oversized body must not be read")
            .contains("first-release limit")
    );

    let id = format!("{:064x}", 0xBAD1u64);
    let body = fixture_attachment_bytes();
    fs::write(attachment_bin_path(&tenant_key, &id), &body)
        .expect("write canonical attachment body");
    let meta = super::super::zk_attachments::AttachmentMeta {
        id: id.clone(),
        content_type: "application/x-norito".to_owned(),
        size: 1,
        created_ms: now_ms(),
        tenant: Some(tenant_key.clone()),
        provenance: Some(fixture_attachment_provenance(&body, "application/x-norito")),
        zk1_tags: None,
    };
    fs::write(
        attachment_meta_path(&tenant_key, &id),
        norito::json::to_json_pretty(&meta).expect("metadata JSON"),
    )
    .expect("write mismatched metadata");
    let report = process_attachment_once(&id).expect("mismatch produces a rejection report");
    assert!(!report.ok);
    assert_eq!(report.size, body.len() as u64);
    assert!(
        report
            .error
            .as_deref()
            .is_some_and(|error| error.contains("metadata size"))
    );
}

#[test]
fn nonregular_attachment_body_produces_a_zero_read_rejection_report() {
    init_test_cfg();
    let _env = TestDataDirGuard::new();
    let tenant_key = anon_tenant_key();
    ensure_tenant_dir(&tenant_key);
    let id = format!("{:064x}", 0xBAD4u64);
    fs::create_dir(attachment_bin_path(&tenant_key, &id)).expect("create nonregular body entry");
    let meta = super::super::zk_attachments::AttachmentMeta {
        id: id.clone(),
        content_type: "application/x-norito".to_owned(),
        size: 0,
        created_ms: now_ms(),
        tenant: Some(tenant_key.clone()),
        provenance: Some(fixture_attachment_provenance(&[], "application/x-norito")),
        zk1_tags: None,
    };
    fs::write(
        attachment_meta_path(&tenant_key, &id),
        norito::json::to_json_pretty(&meta).expect("metadata JSON"),
    )
    .expect("write nonregular-body metadata");
    let loc = AttachmentLocation {
        tenant_key,
        id: id.clone(),
    };

    let loaded = load_attachment_body(&loc).expect("nonregular body is classified");
    assert_eq!(loaded.observed_size, 0);
    assert_eq!(loaded.bytes_read, 0);
    assert!(
        loaded
            .body
            .expect_err("nonregular body must be rejected")
            .contains("securely open")
    );
    let report = process_attachment_once(&id).expect("nonregular body produces a report");
    assert!(!report.ok);
    assert_eq!(report.size, 0);
    assert!(
        report
            .error
            .as_deref()
            .is_some_and(|error| error.contains("securely open"))
    );
}

#[cfg(unix)]
#[test]
fn attachment_body_secure_open_rejects_symlinks_without_reading() {
    use std::os::unix::fs::symlink;

    init_test_cfg();
    let _env = TestDataDirGuard::new();
    let tenant_key = anon_tenant_key();
    ensure_tenant_dir(&tenant_key);
    let id = format!("{:064x}", 0xBAD5u64);
    let target = attachments_root_dir()
        .join(&tenant_key)
        .join("symlink-target.bin");
    fs::write(&target, fixture_attachment_bytes()).expect("write symlink target");
    symlink(&target, attachment_bin_path(&tenant_key, &id)).expect("create body symlink");

    let loaded = load_attachment_body(&AttachmentLocation { tenant_key, id })
        .expect("symlink body is classified");
    assert_eq!(loaded.bytes_read, 0);
    assert!(
        loaded
            .body
            .expect_err("symlink body must be rejected")
            .contains("securely open")
    );
}

#[cfg(unix)]
#[test]
fn attachment_body_secure_open_rejects_a_symlinked_tenant_anchor() {
    use std::os::unix::fs::symlink;

    init_test_cfg();
    let env = TestDataDirGuard::new();
    let tenant_key = anon_tenant_key();
    fs::create_dir_all(attachments_root_dir()).expect("create attachment root");
    let outside = env.path().join("outside-tenant");
    fs::create_dir(&outside).expect("create outside tenant directory");
    let id = format!("{:064x}", 0xBAD8u64);
    fs::write(
        outside.join(format!("{id}.bin")),
        fixture_attachment_bytes(),
    )
    .expect("write body outside attachment root");
    symlink(&outside, attachments_root_dir().join(&tenant_key))
        .expect("create tenant directory symlink");

    let loaded = load_attachment_body(&AttachmentLocation { tenant_key, id })
        .expect("symlink-anchored body is classified");
    assert_eq!(loaded.observed_size, 0);
    assert_eq!(loaded.bytes_read, 0);
    assert!(loaded.body.is_err());
}

#[cfg(any(unix, windows))]
#[test]
fn attachment_body_secure_open_rejects_hard_links_without_reading() {
    init_test_cfg();
    let _env = TestDataDirGuard::new();
    let tenant_key = anon_tenant_key();
    ensure_tenant_dir(&tenant_key);
    let id = format!("{:064x}", 0xBAD7u64);
    let target = attachments_root_dir()
        .join(&tenant_key)
        .join("hard-link-target.bin");
    fs::write(&target, fixture_attachment_bytes()).expect("write hard-link target");
    fs::hard_link(&target, attachment_bin_path(&tenant_key, &id)).expect("create body hard link");

    let loaded = load_attachment_body(&AttachmentLocation { tenant_key, id })
        .expect("hard-linked body is classified");
    assert_eq!(loaded.observed_size, 0);
    assert_eq!(loaded.bytes_read, 0);
    assert!(
        loaded
            .body
            .expect_err("hard-linked body must be rejected")
            .contains("securely open")
    );
}

#[cfg(any(target_os = "linux", target_os = "android"))]
#[test]
fn attachment_body_secure_open_rejects_fifo_without_blocking_or_reading() {
    init_test_cfg();
    let _env = TestDataDirGuard::new();
    let tenant_key = anon_tenant_key();
    ensure_tenant_dir(&tenant_key);
    let id = format!("{:064x}", 0xBAD6u64);
    let fifo_path = attachment_bin_path(&tenant_key, &id);
    rustix::fs::mkfifoat(
        rustix::fs::CWD,
        &fifo_path,
        rustix::fs::Mode::from_raw_mode(0o600),
    )
    .expect("create FIFO body entry");

    let started = std::time::Instant::now();
    let loaded = load_attachment_body(&AttachmentLocation { tenant_key, id })
        .expect("FIFO body is classified");
    assert!(started.elapsed() < Duration::from_secs(1));
    assert_eq!(loaded.bytes_read, 0);
    assert!(loaded.body.is_err());
}

#[test]
fn attachment_body_loader_accepts_the_closed_eight_mib_boundary() {
    init_test_cfg();
    let _env = TestDataDirGuard::new();
    let tenant_key = anon_tenant_key();
    ensure_tenant_dir(&tenant_key);
    let id = format!("{:064x}", 0xBAD3u64);
    fs::write(
        attachment_bin_path(&tenant_key, &id),
        vec![0_u8; PROOF_ATTACHMENT_BODY_MAX_BYTES_V1 as usize],
    )
    .expect("write exact-cap body fixture");

    let loaded = load_attachment_body(&AttachmentLocation { tenant_key, id })
        .expect("exact-cap body is present");
    assert_eq!(loaded.observed_size, PROOF_ATTACHMENT_BODY_MAX_BYTES_V1);
    assert_eq!(loaded.bytes_read, PROOF_ATTACHMENT_BODY_MAX_BYTES_V1);
    let body = loaded
        .body
        .expect("exact-cap body is within the closed limit");
    let decode_error = decode_proof_attachments("application/x-norito", &body)
        .expect_err("zero bytes are not a canonical attachment frame");
    assert!(
        !decode_error.contains("exceeds the"),
        "exact-cap input was rejected as oversized: {decode_error}"
    );
}

#[test]
fn immutable_snapshot_survives_path_replacement_without_reread() {
    configure_test_cfg(Vec::new());
    let _env = TestDataDirGuard::new();
    let tenant_key = anon_tenant_key();
    ensure_tenant_dir(&tenant_key);
    let body = fixture_attachment_bytes();
    let id = attachment_body_id(&body);
    let loc = AttachmentLocation {
        tenant_key: tenant_key.clone(),
        id: id.clone(),
    };
    let meta = super::super::zk_attachments::AttachmentMeta {
        id: id.clone(),
        content_type: "application/x-norito".to_owned(),
        size: body.len() as u64,
        created_ms: now_ms(),
        tenant: Some(tenant_key.clone()),
        provenance: Some(fixture_attachment_provenance(&body, "application/x-norito")),
        zk1_tags: None,
    };
    fs::write(attachment_bin_path(&tenant_key, &id), &body)
        .expect("write canonical attachment snapshot body");
    fs::write(
        attachment_meta_path(&tenant_key, &id),
        norito::json::to_json_pretty(&meta).expect("metadata JSON"),
    )
    .expect("write canonical attachment snapshot metadata");

    let snapshot = match load_attachment_snapshot(&loc, body.len() as u64)
        .expect("snapshot files are present")
    {
        AttachmentSnapshotLoad::Ready(snapshot) => snapshot,
        AttachmentSnapshotLoad::DeferredForByteBudget { .. } => {
            panic!("canonical fixture fits its exact read budget")
        }
    };
    assert_eq!(snapshot.body_load.bytes_read, body.len() as u64);
    fs::write(
        attachment_bin_path(&tenant_key, &id),
        vec![0_u8; PROOF_ATTACHMENT_BODY_MAX_BYTES_V1 as usize + 1],
    )
    .expect("replace path after immutable snapshot acquisition");

    let report = process_attachment_snapshot_at(&loc, snapshot)
        .expect("immutable snapshot produces a report");
    assert!(report.ok, "path replacement affected snapshot: {report:?}");
    assert_eq!(report.size, body.len() as u64);
}

#[test]
fn same_size_body_substitution_is_rejected_by_content_address() {
    configure_test_cfg(Vec::new());
    let _env = TestDataDirGuard::new();
    let tenant_key = anon_tenant_key();
    ensure_tenant_dir(&tenant_key);
    let body = fixture_attachment_bytes();
    let id = attachment_body_id(&body);
    let mut substituted = body.clone();
    substituted[0] ^= 0x01;
    assert_eq!(substituted.len(), body.len());
    let meta = super::super::zk_attachments::AttachmentMeta {
        id: id.clone(),
        content_type: "application/x-norito".to_owned(),
        size: body.len() as u64,
        created_ms: now_ms(),
        tenant: Some(tenant_key.clone()),
        provenance: Some(fixture_attachment_provenance(&body, "application/x-norito")),
        zk1_tags: None,
    };
    fs::write(attachment_bin_path(&tenant_key, &id), substituted)
        .expect("write same-size substituted body");
    fs::write(
        attachment_meta_path(&tenant_key, &id),
        norito::json::to_json_pretty(&meta).expect("metadata JSON"),
    )
    .expect("write original content-address metadata");

    let report = process_attachment_once(&id).expect("substitution produces rejection report");
    assert!(!report.ok);
    assert!(
        report
            .error
            .as_deref()
            .is_some_and(|error| error.contains("body digest")),
        "substitution rejected for the wrong reason: {:?}",
        report.error
    );
}

#[test]
fn snapshot_metadata_and_provenance_invariants_fail_closed() {
    let body = fixture_attachment_bytes();
    let id = attachment_body_id(&body);
    let tenant_key = anon_tenant_key();
    let loc = AttachmentLocation {
        tenant_key: tenant_key.clone(),
        id: id.clone(),
    };
    let base = super::super::zk_attachments::AttachmentMeta {
        id: id.clone(),
        content_type: "application/x-norito".to_owned(),
        size: body.len() as u64,
        created_ms: now_ms(),
        tenant: Some(tenant_key),
        provenance: Some(fixture_attachment_provenance(&body, "application/x-norito")),
        zk1_tags: None,
    };
    let body_load = AttachmentBodyLoad {
        observed_size: body.len() as u64,
        bytes_read: body.len() as u64,
        body: Ok(body),
    };
    validate_attachment_snapshot(&loc, &base, &body_load)
        .expect("canonical metadata and provenance must validate");

    let mut forged = base.clone();
    forged.id = "0".repeat(ATTACHMENT_ID_HEX_LEN);
    assert!(
        validate_attachment_snapshot(&loc, &forged, &body_load)
            .expect_err("metadata id mismatch must reject")
            .contains("metadata id")
    );

    let mut forged = base.clone();
    forged.tenant = Some("0".repeat(TENANT_KEY_HEX_LEN));
    assert!(
        validate_attachment_snapshot(&loc, &forged, &body_load)
            .expect_err("metadata tenant mismatch must reject")
            .contains("metadata tenant")
    );

    let mut forged = base.clone();
    forged.provenance = None;
    assert!(
        validate_attachment_snapshot(&loc, &forged, &body_load)
            .expect_err("missing provenance must reject")
            .contains("provenance is required")
    );

    let mut forged = base.clone();
    forged
        .provenance
        .as_mut()
        .expect("fixture provenance")
        .sanitizer
        .verdict = "rejected".to_owned();
    assert!(
        validate_attachment_snapshot(&loc, &forged, &body_load)
            .expect_err("non-accepted sanitizer verdict must reject")
            .contains("verdict")
    );

    let mut forged = base.clone();
    let incorrect_expanded_size = forged.size.saturating_add(1);
    forged
        .provenance
        .as_mut()
        .expect("fixture provenance")
        .sanitizer
        .expanded_bytes = incorrect_expanded_size;
    assert!(
        validate_attachment_snapshot(&loc, &forged, &body_load)
            .expect_err("expanded-size mismatch must reject")
            .contains("expanded size")
    );

    let mut forged = base.clone();
    forged
        .provenance
        .as_mut()
        .expect("fixture provenance")
        .hashes
        .blake2b_256 = "0".repeat(ATTACHMENT_ID_HEX_LEN);
    assert!(
        validate_attachment_snapshot(&loc, &forged, &body_load)
            .expect_err("provenance Blake2b mismatch must reject")
            .contains("Blake2b-256")
    );

    let mut forged = base.clone();
    forged
        .provenance
        .as_mut()
        .expect("fixture provenance")
        .hashes
        .sha256 = "0".repeat(64);
    assert!(
        validate_attachment_snapshot(&loc, &forged, &body_load)
            .expect_err("provenance SHA-256 mismatch must reject")
            .contains("SHA-256")
    );

    let mut forged = base.clone();
    forged
        .provenance
        .as_mut()
        .expect("fixture provenance")
        .sniffed_type = "application/json".to_owned();
    assert!(
        validate_attachment_snapshot(&loc, &forged, &body_load)
            .expect_err("provenance media mismatch must reject")
            .contains("media type")
    );

    let mut forged = base;
    forged.content_type = "application/json".to_owned();
    forged
        .provenance
        .as_mut()
        .expect("fixture provenance")
        .sniffed_type = "application/json".to_owned();
    assert!(
        validate_attachment_snapshot(&loc, &forged, &body_load)
            .expect_err("forged matching media labels must not override body sniffing")
            .contains("media type")
    );
}

#[test]
fn first_release_scanner_ignores_retired_root_attachment_layout() {
    init_test_cfg();
    let _env = TestDataDirGuard::new();
    fs::create_dir_all(attachments_root_dir()).expect("create attachment root");
    let body = fixture_attachment_bytes();
    let id = attachment_body_id(&body);
    let meta = super::super::zk_attachments::AttachmentMeta {
        id: id.clone(),
        content_type: "application/x-norito".to_owned(),
        size: body.len() as u64,
        created_ms: now_ms(),
        tenant: None,
        provenance: Some(fixture_attachment_provenance(&body, "application/x-norito")),
        zk1_tags: None,
    };
    fs::write(attachments_root_dir().join(format!("{id}.bin")), body)
        .expect("write retired root body");
    fs::write(
        attachments_root_dir().join(format!("{id}.json")),
        norito::json::to_json_pretty(&meta).expect("retired metadata JSON"),
    )
    .expect("write retired root metadata");

    assert!(list_attachment_locations().is_empty());
    assert!(find_attachment_location(&id).is_none());
    assert!(process_attachment_once(&id).is_none());
}

#[test]
fn oversized_first_attachment_cannot_starve_later_valid_work() {
    configure_test_cfg(Vec::new());
    let _env = TestDataDirGuard::new();
    let tenant_key = anon_tenant_key();
    ensure_tenant_dir(&tenant_key);

    let oversized_id = "0".repeat(ATTACHMENT_ID_HEX_LEN);
    let oversized_body = vec![0_u8; PROOF_ATTACHMENT_BODY_MAX_BYTES_V1 as usize + 1];
    let oversized_meta = super::super::zk_attachments::AttachmentMeta {
        id: oversized_id.clone(),
        content_type: "application/x-norito".to_owned(),
        size: oversized_body.len() as u64,
        created_ms: now_ms(),
        tenant: Some(tenant_key.clone()),
        provenance: Some(fixture_attachment_provenance(
            &oversized_body,
            "application/x-norito",
        )),
        zk1_tags: None,
    };
    fs::write(
        attachment_bin_path(&tenant_key, &oversized_id),
        oversized_body,
    )
    .expect("write oversized first body");
    fs::write(
        attachment_meta_path(&tenant_key, &oversized_id),
        norito::json::to_json_pretty(&oversized_meta).expect("oversized metadata JSON"),
    )
    .expect("write oversized first metadata");

    let valid_body = fixture_attachment_bytes();
    let valid_id = attachment_body_id(&valid_body);
    let valid_meta = super::super::zk_attachments::AttachmentMeta {
        id: valid_id.clone(),
        content_type: "application/x-norito".to_owned(),
        size: valid_body.len() as u64,
        created_ms: now_ms(),
        tenant: Some(tenant_key.clone()),
        provenance: Some(fixture_attachment_provenance(
            &valid_body,
            "application/x-norito",
        )),
        zk1_tags: None,
    };
    fs::write(attachment_bin_path(&tenant_key, &valid_id), &valid_body)
        .expect("write later valid body");
    fs::write(
        attachment_meta_path(&tenant_key, &valid_id),
        norito::json::to_json_pretty(&valid_meta).expect("valid metadata JSON"),
    )
    .expect("write later valid metadata");

    let stats = super::block_on_scan();
    assert_eq!(stats.processed_reports, 2);
    assert_eq!(stats.bytes_processed, valid_body.len() as u64);
    assert_eq!(stats.remaining_pending, 0);
    assert_eq!(stats.budget_exhausted, None);
    assert!(
        load_report(&oversized_id)
            .expect("oversized rejection report")
            .error
            .as_deref()
            .is_some_and(|error| error.contains("first-release limit"))
    );
    assert!(load_report(&valid_id).expect("valid later report").ok);
}

#[test]
fn attachment_metadata_loading_rejects_oversized_files_before_parsing() {
    init_test_cfg();
    let _env = TestDataDirGuard::new();
    let tenant_key = anon_tenant_key();
    ensure_tenant_dir(&tenant_key);
    let id = format!("{:064x}", 0xBAD2u64);
    fs::write(
        attachment_meta_path(&tenant_key, &id),
        vec![b' '; ATTACHMENT_META_FILE_MAX_BYTES as usize + 1],
    )
    .expect("write oversized metadata fixture");
    assert!(
        load_attachment_meta(&AttachmentLocation { tenant_key, id }).is_none(),
        "oversized metadata must fail before JSON parsing"
    );
}

#[test]
fn scan_respects_byte_budget() {
    init_test_cfg();
    let _env = TestDataDirGuard::new();
    let budget = super::cfg_max_scan_bytes().max(2);
    let budget = usize::try_from(budget).unwrap_or(usize::MAX);
    let first_size = budget.saturating_sub(1).max(1);
    let sizes = [first_size, 2usize];
    let tenant_key = anon_tenant_key();
    ensure_tenant_dir(&tenant_key);
    // Create two attachments totalling more than the configured byte budget.
    for (idx, size) in sizes.into_iter().enumerate() {
        let id = format!("{:064x}", idx + 1);
        let body = vec![b'A'; size];
        let meta = super::super::zk_attachments::AttachmentMeta {
            id: id.clone(),
            content_type: "application/json".to_string(),
            size: body.len() as u64,
            created_ms: now_ms(),
            tenant: Some(tenant_key.clone()),
            provenance: Some(fixture_attachment_provenance(&body, "application/json")),
            zk1_tags: None,
        };
        fs::write(attachment_bin_path(&tenant_key, &id), body).unwrap();
        fs::write(
            attachment_meta_path(&tenant_key, &id),
            norito::json::to_json_pretty(&meta).unwrap(),
        )
        .unwrap();
    }

    let stats = super::block_on_scan();
    assert_eq!(
        stats.processed_reports, 1,
        "only first attachment fits budget"
    );
    assert_eq!(stats.budget_exhausted, Some("bytes"));
    assert_eq!(stats.remaining_pending, 1);
    assert_eq!(stats.bytes_processed, first_size as u64);
}

#[test]
fn deferred_attachment_cannot_head_of_line_block_later_fitting_work() {
    init_test_cfg();
    let _env = TestDataDirGuard::new();
    let budget = cfg_max_scan_bytes();
    assert!(budget > 8, "test scan budget must leave a tail");
    let first_size = usize::try_from(budget - 4).expect("test budget fits usize");
    let sizes = [first_size, 5, 4];
    let tenant_key = anon_tenant_key();
    ensure_tenant_dir(&tenant_key);

    for (index, size) in sizes.into_iter().enumerate() {
        let id = format!("{:064x}", index + 1);
        let body = vec![b'C' + u8::try_from(index).expect("small index"); size];
        let meta = super::super::zk_attachments::AttachmentMeta {
            id: id.clone(),
            content_type: "application/json".to_owned(),
            size: body.len() as u64,
            created_ms: now_ms(),
            tenant: Some(tenant_key.clone()),
            provenance: Some(fixture_attachment_provenance(&body, "application/json")),
            zk1_tags: None,
        };
        fs::write(attachment_bin_path(&tenant_key, &id), body).expect("write budget-order body");
        fs::write(
            attachment_meta_path(&tenant_key, &id),
            norito::json::to_json_pretty(&meta).expect("budget-order metadata JSON"),
        )
        .expect("write budget-order metadata");
    }

    let stats = block_on_scan();
    assert_eq!(stats.processed_reports, 2);
    assert_eq!(stats.bytes_processed, budget);
    assert_eq!(stats.remaining_pending, 1);
    assert_eq!(stats.budget_exhausted, Some("bytes"));
    assert!(load_report(&format!("{:064x}", 1)).is_some());
    assert!(
        load_report(&format!("{:064x}", 2)).is_none(),
        "the body that does not fit must remain pending without being read"
    );
    assert!(
        load_report(&format!("{:064x}", 3)).is_some(),
        "a later body that fits the remaining budget must still be processed"
    );
}

#[test]
fn snapshot_that_crosses_time_budget_is_charged_and_completed_once() {
    let body = fixture_attachment_bytes();
    let body_size = body.len() as u64;
    let max_scan_millis = 100;
    configure_test_cfg(iroha_config::parameters::defaults::torii::zk_prover_allowed_circuits());
    let _env = TestDataDirGuard::new();
    let _delay_reset = SnapshotLoadDelayReset;
    let tenant_key = anon_tenant_key();
    ensure_tenant_dir(&tenant_key);
    let id = attachment_body_id(&body);
    let meta = super::super::zk_attachments::AttachmentMeta {
        id: id.clone(),
        content_type: "application/x-norito".to_owned(),
        size: body_size,
        created_ms: now_ms(),
        tenant: Some(tenant_key.clone()),
        provenance: Some(fixture_attachment_provenance(&body, "application/x-norito")),
        zk1_tags: None,
    };
    fs::write(attachment_bin_path(&tenant_key, &id), body).expect("write delayed snapshot body");
    fs::write(
        attachment_meta_path(&tenant_key, &id),
        norito::json::to_json_pretty(&meta).expect("delayed snapshot metadata JSON"),
    )
    .expect("write delayed snapshot metadata");
    super::TEST_MAX_SCAN_MILLIS_OVERRIDE.store(max_scan_millis, AtomicOrdering::SeqCst);
    super::TEST_SNAPSHOT_LOAD_DELAY_MS.store(150, AtomicOrdering::SeqCst);

    let stats = block_on_scan();
    assert_eq!(stats.processed_reports, 1);
    assert_eq!(stats.bytes_processed, body_size);
    assert_eq!(stats.remaining_pending, 0);
    assert_eq!(stats.budget_exhausted, Some("time"));
    assert!(stats.duration_ms >= max_scan_millis);
    assert!(
        load_report(&id).expect("cross-budget snapshot report").ok,
        "an immutable snapshot read before the time check must complete exactly once"
    );
}

#[test]
fn scan_bounds_concurrency() {
    init_test_cfg();
    let _env = TestDataDirGuard::new();
    super::TEST_PROCESSING_DELAY_MS.store(50, AtomicOrdering::SeqCst);
    let tenant_key = anon_tenant_key();
    ensure_tenant_dir(&tenant_key);
    // Create four small attachments to trigger overlapping work.
    for idx in 0..4 {
        let id = format!("{:064x}", idx + 10);
        let body = vec![b'B'; 16];
        let meta = super::super::zk_attachments::AttachmentMeta {
            id: id.clone(),
            content_type: "application/json".to_string(),
            size: body.len() as u64,
            created_ms: now_ms(),
            tenant: Some(tenant_key.clone()),
            provenance: Some(fixture_attachment_provenance(&body, "application/json")),
            zk1_tags: None,
        };
        fs::write(attachment_bin_path(&tenant_key, &id), body).unwrap();
        fs::write(
            attachment_meta_path(&tenant_key, &id),
            norito::json::to_json_pretty(&meta).unwrap(),
        )
        .unwrap();
    }

    let stats = super::block_on_scan();
    assert_eq!(stats.budget_exhausted, None);
    let observed = super::MAX_INFLIGHT_OBSERVED.load(AtomicOrdering::SeqCst);
    assert!(
        observed <= super::cfg_max_inflight(),
        "observed inflight {} exceeds cap",
        observed
    );
    super::TEST_PROCESSING_DELAY_MS.store(0, AtomicOrdering::SeqCst);
}

#[tokio::test(flavor = "current_thread")]
async fn scan_once_handles_current_thread_runtime() {
    init_test_cfg();
    let _env = TestDataDirGuard::new();
    assert_eq!(super::scan_once(), 0);
}

#[test]
fn zk1_extracts_tags_prof_and_ipak() {
    let mut v = b"ZK1\0".to_vec();
    // PROF with 0 payload
    v.extend_from_slice(b"PROF");
    v.extend_from_slice(&0u32.to_le_bytes());
    // IPAK with 4-byte payload
    v.extend_from_slice(b"IPAK");
    v.extend_from_slice(&4u32.to_le_bytes());
    v.extend_from_slice(&5u32.to_le_bytes());
    let tags = parse_zk1_tags(&v).expect("valid bounded ZK1 envelope");
    assert!(tags.starts_with(&["PROF".to_string(), "IPAK".to_string()]));
}

#[test]
fn zk1_tlv_count_is_bounded_and_duplicate_tags_are_compacted() {
    let mut envelope = b"ZK1\0".to_vec();
    for _ in 0..ZK1_MAX_TLV_COUNT {
        envelope.extend_from_slice(b"PROF");
        envelope.extend_from_slice(&0u32.to_le_bytes());
    }
    assert_eq!(parse_zk1_tags(&envelope), Ok(vec!["PROF".to_string()]));

    envelope.extend_from_slice(b"IPAK");
    envelope.extend_from_slice(&0u32.to_le_bytes());
    let error = parse_zk1_tags(&envelope).expect_err("65th TLV must be rejected");
    assert!(error.contains("too many ZK1 TLVs"));
    let decode_error = decode_proof_attachments("application/x-zk1", &envelope)
        .expect_err("prover ingress must apply the same TLV limit");
    assert!(decode_error.contains("too many ZK1 TLVs"));
    assert!(
        parse_zk1_tags(&envelope).is_err(),
        "invalid envelopes must not expose partial tag metadata"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn background_worker_processes_pending_attachments() {
    configure_test_cfg(Vec::new());
    let _env = TestDataDirGuard::new();

    // Prepare attachment directory with one valid proof attachment and one malformed ZK1 payload.
    let tenant_key = anon_tenant_key();
    ensure_tenant_dir(&tenant_key);

    let ok_body = fixture_attachment_bytes();
    let ok_id = attachment_body_id(&ok_body);
    fs::write(attachment_bin_path(&tenant_key, &ok_id), &ok_body).expect("write ok body");
    let ok_meta = super::super::zk_attachments::AttachmentMeta {
        id: ok_id.clone(),
        content_type: "application/x-norito".to_string(),
        size: ok_body.len() as u64,
        created_ms: super::now_ms(),
        tenant: Some(tenant_key.clone()),
        provenance: Some(fixture_attachment_provenance(
            &ok_body,
            "application/x-norito",
        )),
        zk1_tags: None,
    };
    fs::write(
        attachment_meta_path(&tenant_key, &ok_id),
        norito::json::to_json_pretty(&ok_meta).expect("ok meta json"),
    )
    .expect("write ok meta");

    let mut err_body = b"ZK1\0".to_vec();
    err_body.extend_from_slice(b"PROF");
    err_body.extend_from_slice(&10u32.to_le_bytes());
    let err_id = attachment_body_id(&err_body);
    fs::write(attachment_bin_path(&tenant_key, &err_id), &err_body).expect("write err body");
    let err_meta = super::super::zk_attachments::AttachmentMeta {
        id: err_id.clone(),
        content_type: "application/x-norito".to_string(),
        size: err_body.len() as u64,
        created_ms: super::now_ms(),
        tenant: Some(tenant_key.clone()),
        provenance: Some(fixture_attachment_provenance(
            &err_body,
            "application/x-norito",
        )),
        zk1_tags: None,
    };
    fs::write(
        attachment_meta_path(&tenant_key, &err_id),
        norito::json::to_json_pretty(&err_meta).expect("err meta json"),
    )
    .expect("write err meta");

    super::start_worker();

    use tokio::time::{Duration, Instant, sleep};
    let deadline = Instant::now() + Duration::from_secs(6);
    let mut ok_report_ready = false;
    let mut err_ready = false;
    while Instant::now() < deadline {
        if !ok_report_ready {
            ok_report_ready = super::load_report(&ok_id).is_some();
        }
        if !err_ready {
            err_ready = super::load_report(&err_id)
                .map(|rep| !rep.ok)
                .unwrap_or(false);
        }
        if ok_report_ready && err_ready {
            break;
        }
        sleep(Duration::from_millis(100)).await;
    }

    assert!(ok_report_ready, "Proof attachment should produce a report");
    assert!(
        err_ready,
        "Malformed Norito attachment should produce an error report"
    );

    assert_eq!(
        super::scan_once(),
        0,
        "worker should drain pending attachments"
    );
}
