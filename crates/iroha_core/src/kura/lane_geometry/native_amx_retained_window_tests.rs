#[test]
fn native_amx_retained_windows_require_one_exact_contiguous_suffix() {
    assert!(!native_amx_retained_windows_are_complete(
        &BTreeSet::from([7, 8, 9, 10]),
        &BTreeSet::from([9, 10]),
    ));
    assert!(native_amx_retained_windows_are_complete(
        &BTreeSet::from([9, 10]),
        &BTreeSet::from([9, 10]),
    ));
    assert!(!native_amx_retained_windows_are_complete(
        &BTreeSet::from([7, 9, 10]),
        &BTreeSet::from([7, 9, 10]),
    ));
    assert!(!native_amx_retained_windows_are_complete(
        &BTreeSet::from([8, 9, 10, 11]),
        &BTreeSet::from([9, 10]),
    ));
    assert!(!native_amx_retained_windows_are_complete(
        &BTreeSet::from([8]),
        &BTreeSet::new(),
    ));
    assert!(native_amx_retained_windows_are_complete(
        &BTreeSet::new(),
        &BTreeSet::new(),
    ));
    assert_eq!(
        Kura::parse_native_amx_evidence_path(Path::new(
            "native_amx_manifest_v1_00000000000000000001.norito"
        ))
        .expect("parse canonical Native manifest filename"),
        Some((NativeAmxEvidenceKind::Manifest, 1, false)),
    );
    assert_eq!(
        Kura::parse_native_amx_evidence_path(Path::new(
            "native_amx_receipt_v1_18446744073709551615.norito"
        ))
        .expect("parse canonical Native receipt filename"),
        Some((NativeAmxEvidenceKind::Receipt, u64::MAX, false)),
    );
    for obsolete_or_non_canonical in [
        "native_amx_manifest_v1_00000000000000000000.norito",
        "native_amx_manifest_v1_1.norito",
        "native_amx_receipt_v1_000000000000000000001.norito",
        "native_amx_application_manifests.norito",
        "native_amx_participant_receipts.index",
    ] {
        assert!(
            Kura::parse_native_amx_evidence_path(Path::new(obsolete_or_non_canonical)).is_err(),
            "{obsolete_or_non_canonical} must not enter the first-release evidence allowlist",
        );
    }
    assert_eq!(
        Kura::parse_native_amx_evidence_path(Path::new(
            "native_amx_receipt_v1_00000000000000000001.norito.tmp"
        ))
        .expect("parse canonical Native receipt temp filename"),
        Some((NativeAmxEvidenceKind::Receipt, 1, true)),
    );
}
