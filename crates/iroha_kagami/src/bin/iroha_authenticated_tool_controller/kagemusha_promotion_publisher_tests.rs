//! Hostile unit coverage for the exact Kagemusha promotion publisher.

use super::*;

fn identity(inode: u64) -> FileIdentity {
    FileIdentity {
        device: 7,
        inode,
        mode: 0o100_444,
        uid: 0,
        gid: 0,
        links: 1,
        size: 128,
        modified_seconds: 11,
        modified_nanoseconds: 12,
        sha256: [u8::try_from(inode).unwrap_or(1); 32],
    }
}

fn candidate() -> BTreeMap<String, FileIdentity> {
    CANDIDATE_FILES
        .iter()
        .enumerate()
        .map(|(index, spec)| {
            let mut identity = identity(index as u64 + 1);
            if let Some(size) = spec.exact_size {
                identity.size = size;
            }
            (spec.name.to_owned(), identity)
        })
        .collect()
}

fn base_arguments() -> Vec<OsString> {
    [
        "--expected-macos-build",
        "24G90",
        "--kagami",
        "/Library/SORA/Kagemusha/bin/kagami",
        "--kagami-sha256",
        "1111111111111111111111111111111111111111111111111111111111111111",
        "--bundle-dir",
        "/Library/SORA/Kagemusha/releases/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "--release-policy",
        "/Library/SORA/Kagemusha/policy/release.norito",
    ]
    .into_iter()
    .map(OsString::from)
    .collect()
}

fn valid_report() -> (Vec<u8>, CanonicalReportV4) {
    let envelope = "aa".repeat(32);
    let policy = [0xbb; 32];
    let promotion = [0xcc; 32];
    let digest = "11".repeat(32);
    let candidate = candidate();
    let qualification = hex(&candidate["recursive-step-two-qualification-v4.norito"].sha256);
    let internal_validation =
        hex(&candidate[KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_RECEIPT_FILE_NAME_V1].sha256);
    let artifacts = REPORT_ARTIFACTS
        .iter()
        .enumerate()
        .map(|(index, (purpose, name))| {
            let identity = &candidate[*name];
            CanonicalReportArtifact {
                purpose: (*purpose).to_owned(),
                file_name: (*name).to_owned(),
                size_bytes: identity.size,
                sha256: hex(&identity.sha256),
                payload_size_bytes: (index + 1 != REPORT_ARTIFACTS.len()).then_some(1),
                payload_sha256: (index + 1 != REPORT_ARTIFACTS.len()).then(|| digest.clone()),
            }
        })
        .collect();
    let expected = CanonicalReportV4 {
        status: "verified".to_owned(),
        envelope_sha256: envelope,
        manifest_body_sha256: digest.clone(),
        candidate_sha256: digest.clone(),
        qualification_receipt_sha256: qualification,
        qualified_candidate_sha256: digest.clone(),
        internal_validation_receipt_sha256: internal_validation,
        promotion_record_sha256: hex(&promotion),
        release_policy_sha256: hex(&policy),
        authenticated_source_seal_projection_sha256: digest.clone(),
        reviewed_cargo_binary_sha256: digest.clone(),
        reviewed_rustc_binary_sha256: digest.clone(),
        generator_binary_sha256: digest.clone(),
        sealed_candidate_build_report_sha256: digest.clone(),
        generation: "g1".to_owned(),
        generation_memory_limit_bytes: 1,
        generation_memory_enforcement_profile: "self-physical-footprint-v1".to_owned(),
        network_id: "n1".to_owned(),
        asset_definition_id: "a1".to_owned(),
        asset_scale: 1,
        bridge_abi_version: 23,
        recursive_step_verifier_commitment: digest,
        artifacts,
    };
    let mut report = norito::json::to_json(&expected)
        .expect("canonical report JSON")
        .into_bytes();
    report.push(b'\n');
    (report, expected)
}

#[test]
fn parser_accepts_only_the_exact_ordered_promotion_contract() {
    let parsed = parse_request(&base_arguments()).expect("exact request");
    assert_eq!(parsed.expected_macos_build, "24G90");
    assert_eq!(
        parsed.kagami,
        Path::new("/Library/SORA/Kagemusha/bin/kagami")
    );
    for mutate in 0..base_arguments().len() {
        let mut arguments = base_arguments();
        if mutate % 2 == 0 {
            arguments[mutate] = OsString::from("--unexpected");
        } else {
            arguments[mutate] = OsString::from("../escape");
        }
        assert!(parse_request(&arguments).is_err(), "mutation {mutate}");
    }
    let mut trailing = base_arguments();
    trailing.push(OsString::from("--memory-limit-bytes"));
    trailing.push(OsString::from("1"));
    assert!(parse_request(&trailing).is_err());
}

#[test]
fn temporary_name_grammar_is_exact() {
    assert!(valid_temp_name(
        ".promotion-record-v4.norito.tmp.0123456789abcdef0123456789abcdef"
    ));
    for invalid in [
        "promotion-record-v4.norito",
        ".promotion-record-v4.norito.tmp.0123",
        ".promotion-record-v4.norito.tmp.0123456789abcdef0123456789abcdeF",
        ".promotion-record-v4.norito.tmp.0123456789abcdef0123456789abcdef.extra",
        ".promotion-record-v4.norito.tmp../0123456789abcdef0123456789abcdef",
    ] {
        assert!(!valid_temp_name(invalid), "accepted {invalid}");
    }
}

#[test]
fn exact_inventory_allows_only_seventeen_to_one_temp_to_eighteen() {
    let initial = candidate();
    assert_eq!(initial.len(), 17);
    assert!(
        initial.contains_key(KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_RECEIPT_FILE_NAME_V1)
    );
    assert_eq!(
        classify_inventory(&initial, &initial).expect("candidate"),
        PublicationPhase::Candidate
    );
    let mut staging = initial.clone();
    let mut temporary = identity(90);
    temporary.mode = 0o100_600;
    temporary.size = 0;
    staging.insert(
        ".promotion-record-v4.norito.tmp.0123456789abcdef0123456789abcdef".to_owned(),
        temporary,
    );
    assert_eq!(
        classify_inventory(&initial, &staging).expect("staging"),
        PublicationPhase::Staging
    );
    let mut committed = initial.clone();
    let mut final_identity = identity(91);
    final_identity.mode = 0o100_600;
    committed.insert(FINAL_NAME.to_owned(), final_identity);
    assert_eq!(
        classify_inventory(&initial, &committed).expect("committed"),
        PublicationPhase::Committed
    );
}

#[test]
fn candidate_contract_requires_the_exact_bounded_internal_validation_receipt() {
    let receipt = CANDIDATE_FILES
        .iter()
        .find(|spec| {
            spec.name == KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_RECEIPT_FILE_NAME_V1
        })
        .expect("internal-validation receipt is mandatory");
    assert_eq!(
        usize::try_from(receipt.maximum),
        Ok(KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_RECEIPT_MAX_BYTES_V1)
    );
    assert_eq!(receipt.exact_size, None);

    let initial = candidate();
    let mut missing = initial.clone();
    missing.remove(KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_RECEIPT_FILE_NAME_V1);
    assert!(classify_inventory(&missing, &missing).is_err());
    assert!(classify_inventory(&initial, &missing).is_err());
}

#[test]
fn inventory_rejects_every_candidate_mutation_and_existing_final() {
    let initial = candidate();
    for name in initial.keys() {
        let mut current = initial.clone();
        current.get_mut(name).expect("identity").inode += 1000;
        assert!(classify_inventory(&initial, &current).is_err(), "{name}");
    }
    let mut existing_final = initial.clone();
    existing_final.insert(FINAL_NAME.to_owned(), identity(99));
    assert!(classify_inventory(&existing_final, &existing_final).is_err());

    let mut substituted_receipt = initial.clone();
    substituted_receipt
        .get_mut(KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_RECEIPT_FILE_NAME_V1)
        .expect("required receipt")
        .sha256[0] ^= 1;
    assert!(classify_inventory(&initial, &substituted_receipt).is_err());
}

#[test]
fn inventory_rejects_temp_multiplicity_and_every_other_delta() {
    let initial = candidate();
    let mut current = initial.clone();
    for suffix in [
        "0123456789abcdef0123456789abcdef",
        "fedcba9876543210fedcba9876543210",
    ] {
        let mut temporary = identity(current.len() as u64 + 40);
        temporary.mode = 0o100_600;
        current.insert(format!("{TEMP_PREFIX}{suffix}"), temporary);
    }
    assert!(classify_inventory(&initial, &current).is_err());
    current = initial.clone();
    current.insert("attacker".to_owned(), identity(77));
    assert!(classify_inventory(&initial, &current).is_err());
}

#[test]
fn source_path_swap_and_snapshot_digest_mismatch_fail_closed() {
    let expected = identity(1);
    for mutation in 0..10 {
        let mut observed = expected.clone();
        match mutation {
            0 => observed.device += 1,
            1 => observed.inode += 1,
            2 => observed.mode ^= 0o100,
            3 => observed.uid += 1,
            4 => observed.gid += 1,
            5 => observed.links += 1,
            6 => observed.size += 1,
            7 => observed.modified_seconds += 1,
            8 => observed.modified_nanoseconds += 1,
            _ => observed.sha256[0] ^= 1,
        }
        assert_ne!(expected, observed, "mutation {mutation}");
        assert!(
            !stable_candidate_identity(
                KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_RECEIPT_FILE_NAME_V1,
                &expected,
                &observed,
            ),
            "stable receipt identity accepted mutation {mutation}"
        );
        assert_eq!(stable_identity(&expected, &observed), mutation == 9);
    }
    assert_ne!(
        parse_sha256(&"11".repeat(32), "snapshot").expect("digest"),
        parse_sha256(&"12".repeat(32), "snapshot").expect("digest")
    );
}

#[test]
fn nonzero_child_and_cleanup_ambiguity_distinguish_uncertain_commit() {
    assert_eq!(
        failed_child_result(9, Some(PublicationPhase::Candidate), true).expect("clean failure"),
        9
    );
    for (phase, cleanup) in [
        (Some(PublicationPhase::Staging), true),
        (Some(PublicationPhase::Committed), true),
        (Some(PublicationPhase::Candidate), false),
        (None, true),
    ] {
        let error = failed_child_result(9, phase, cleanup).expect_err("uncertain");
        assert_eq!(error.exit, COMMIT_UNCERTAIN_EXIT);
    }
}

#[test]
fn seatbelt_profile_has_only_snapshot_exec_and_publisher_write_shapes() {
    let snapshot = Path::new("/private/var/db/iroha-kagemusha-promotion-v1/active/kagami");
    let bundle = Path::new(
        "/Library/SORA/releases/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    );
    let policy = Path::new("/Library/SORA/policy/release.norito");
    let profile = sandbox_profile(snapshot, bundle, policy).expect("profile");
    assert!(profile.contains("(deny network*)"));
    assert!(profile.contains("(deny process-fork)"));
    assert!(profile.contains(&format!(
        "(allow process-exec (literal \"{}\"))",
        snapshot.display()
    )));
    assert!(profile.contains(&format!(
        "(allow file-write-create (require-all (vnode-type REGULAR-FILE) (literal \"{}\")))",
        bundle.join(FINAL_NAME).display()
    )));
    assert!(
        profile.contains("(allow file-write* (require-all (vnode-type REGULAR-FILE) (regex #\"^")
    );
    assert!(profile.contains(&format!(
        "(allow file-write-data (literal \"{}\"))",
        bundle.display()
    )));
    assert_eq!(profile.matches("(allow file-write").count(), 3);
}

#[test]
fn report_cross_checks_exact_fields_inventory_and_digests() {
    let (report, expected) = valid_report();
    canonical_report(&report, &expected).expect("valid report");
    let text = String::from_utf8(report).expect("UTF-8");
    let digest_mutations = [
        ("envelope_sha256", expected.envelope_sha256.as_str()),
        (
            "manifest_body_sha256",
            expected.manifest_body_sha256.as_str(),
        ),
        ("candidate_sha256", expected.candidate_sha256.as_str()),
        (
            "qualification_receipt_sha256",
            expected.qualification_receipt_sha256.as_str(),
        ),
        (
            "qualified_candidate_sha256",
            expected.qualified_candidate_sha256.as_str(),
        ),
        (
            "internal_validation_receipt_sha256",
            expected.internal_validation_receipt_sha256.as_str(),
        ),
        (
            "promotion_record_sha256",
            expected.promotion_record_sha256.as_str(),
        ),
        (
            "release_policy_sha256",
            expected.release_policy_sha256.as_str(),
        ),
        (
            "authenticated_source_seal_projection_sha256",
            expected
                .authenticated_source_seal_projection_sha256
                .as_str(),
        ),
        (
            "reviewed_cargo_binary_sha256",
            expected.reviewed_cargo_binary_sha256.as_str(),
        ),
        (
            "reviewed_rustc_binary_sha256",
            expected.reviewed_rustc_binary_sha256.as_str(),
        ),
        (
            "generator_binary_sha256",
            expected.generator_binary_sha256.as_str(),
        ),
        (
            "sealed_candidate_build_report_sha256",
            expected.sealed_candidate_build_report_sha256.as_str(),
        ),
        (
            "recursive_step_verifier_commitment",
            expected.recursive_step_verifier_commitment.as_str(),
        ),
    ];
    for (field, value) in digest_mutations {
        let mutation = text.replacen(
            &format!("\"{field}\":\"{value}\""),
            &format!("\"{field}\":\"{}\"", "ef".repeat(32)),
            1,
        );
        assert_ne!(mutation, text, "test mutation missed {field}");
        assert!(
            canonical_report(mutation.as_bytes(), &expected).is_err(),
            "{field}"
        );
    }
}

#[test]
#[expect(clippy::too_many_lines, reason = "complete report mutation matrix")]
fn report_rejects_every_noncanonical_type_order_scalar_and_artifact_binding() {
    let (report, expected) = valid_report();
    let text = String::from_utf8(report).expect("UTF-8");
    let first = &expected.artifacts[0];
    let first_artifact = norito::json::to_json(first).expect("first artifact JSON");
    let second_artifact =
        norito::json::to_json(&expected.artifacts[1]).expect("second artifact JSON");
    let mutations = [
        text.replacen('{', "{ ", 1),
        text.replacen("\"status\":\"verified\"", "\"status\":\"failed\"", 1),
        text.replacen("\"status\":\"verified\",", "", 1),
        text.replacen(
            &format!(
                "\"status\":\"verified\",\"envelope_sha256\":\"{}\"",
                expected.envelope_sha256
            ),
            &format!(
                "\"envelope_sha256\":\"{}\",\"status\":\"verified\"",
                expected.envelope_sha256
            ),
            1,
        ),
        text.replacen("\"generation\":\"g1\"", "\"generation\":\"g2\"", 1),
        text.replacen(
            "\"generation_memory_limit_bytes\":1",
            "\"generation_memory_limit_bytes\":2",
            1,
        ),
        text.replacen(
            "\"generation_memory_enforcement_profile\":\"self-physical-footprint-v1\"",
            "\"generation_memory_enforcement_profile\":\"other\"",
            1,
        ),
        text.replacen("\"network_id\":\"n1\"", "\"network_id\":\"n2\"", 1),
        text.replacen(
            "\"asset_definition_id\":\"a1\"",
            "\"asset_definition_id\":\"a2\"",
            1,
        ),
        text.replacen("\"asset_scale\":1", "\"asset_scale\":\"1\"", 1),
        text.replacen("\"bridge_abi_version\":23", "\"bridge_abi_version\":22", 1),
        text.replacen(
            &format!("\"purpose\":\"{}\"", first.purpose),
            "\"purpose\":\"wrong_role\"",
            1,
        ),
        text.replacen(
            &format!("\"file_name\":\"{}\"", first.file_name),
            "\"file_name\":\"wrong.krv4\"",
            1,
        ),
        text.replacen(
            &format!("\"size_bytes\":{}", first.size_bytes),
            "\"size_bytes\":127",
            1,
        ),
        text.replacen(
            &format!("\"sha256\":\"{}\"", first.sha256),
            &format!("\"sha256\":\"{}\"", "ee".repeat(32)),
            1,
        ),
        text.replacen("\"payload_size_bytes\":1", "\"payload_size_bytes\":2", 1),
        text.replacen(
            &format!(
                "\"payload_sha256\":\"{}\"",
                first.payload_sha256.as_deref().expect("payload digest")
            ),
            &format!("\"payload_sha256\":\"{}\"", "dd".repeat(32)),
            1,
        ),
        text.replacen(
            &format!(
                "\"purpose\":\"{}\",\"file_name\":\"{}\"",
                first.purpose, first.file_name
            ),
            &format!(
                "\"file_name\":\"{}\",\"purpose\":\"{}\"",
                first.file_name, first.purpose
            ),
            1,
        ),
        text.replacen(
            &format!("{first_artifact},{second_artifact}"),
            &format!("{second_artifact},{first_artifact}"),
            1,
        ),
        text.replacen(
            "\"payload_size_bytes\":null,\"payload_sha256\":null",
            &format!(
                "\"payload_size_bytes\":1,\"payload_sha256\":\"{}\"",
                "cc".repeat(32)
            ),
            1,
        ),
        text.replacen("}\n", ",\"unexpected\":true}\n", 1),
    ];
    for (index, mutation) in mutations.into_iter().enumerate() {
        assert_ne!(mutation, text, "test mutation {index} missed its target");
        assert!(
            canonical_report(mutation.as_bytes(), &expected).is_err(),
            "mutation {index}"
        );
    }
}

#[test]
fn bounded_identity_rejects_special_empty_oversized_and_inexact_members() {
    let bounds = FileBounds {
        maximum: 128,
        exact_size: Some(128),
        allow_empty: false,
    };
    let mut observed = identity(1);
    validate_bounded_identity("member", &observed, bounds).expect("bounded regular file");
    for mutation in 0..4 {
        observed = identity(1);
        match mutation {
            0 => observed.mode = 0o010_600,
            1 => observed.size = 0,
            2 => observed.size = 129,
            _ => observed.size = 127,
        }
        assert!(
            validate_bounded_identity("member", &observed, bounds).is_err(),
            "mutation {mutation}"
        );
    }
}

#[cfg(target_os = "macos")]
#[test]
fn member_open_rejects_special_files_before_custody_or_content_reads() {
    use std::os::unix::net::UnixListener;

    let root = tempfile::tempdir().expect("temporary promotion directory");
    let special = root.path().join(FINAL_NAME);
    let _listener = UnixListener::bind(&special).expect("Unix socket fixture");
    let directory = open_directory(root.path()).expect("open fixture directory");
    assert!(
        open_member(
            &directory,
            FINAL_NAME,
            inventoried_member_bounds(FINAL_NAME).expect("final bounds"),
            true,
        )
        .is_err()
    );
}

#[cfg(target_os = "macos")]
#[test]
fn active_snapshot_reservation_rejects_stale_or_concurrent_state() {
    let parent = tempfile::tempdir().expect("snapshot parent");
    let staging = SnapshotStaging::create(parent.path()).expect("first active reservation");
    assert!(SnapshotStaging::create(parent.path()).is_err());
    drop(staging);
    assert!(!parent.path().join("active").exists());
    fs::create_dir(parent.path().join("active")).expect("stale active state");
    assert!(SnapshotStaging::create(parent.path()).is_err());
    assert!(parent.path().join("active").is_dir());
}

#[cfg(target_os = "macos")]
#[test]
fn staging_guard_cleans_partial_snapshot_on_create_stage_failure() {
    let parent = tempfile::tempdir().expect("snapshot parent");
    let staging = SnapshotStaging::create(parent.path()).expect("active reservation");
    fs::write(&staging.path, b"partial executable").expect("partial snapshot");
    drop(staging);
    assert!(!parent.path().join("active").exists());
}
