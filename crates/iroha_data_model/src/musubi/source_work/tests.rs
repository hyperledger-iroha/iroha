//! Exact shape ceilings, malformed nested sources, and byte-preservation controls.

use super::super::tests::{account, archive_commitment, provider_bundle_binding, release_manifest};
use super::*;
use crate::sorafs::pin_registry::{
    PinManifestRecord, PinPolicy, PinStatus, ReplicationOrderCompletionRecord,
    ReplicationOrderRecord, ReplicationOrderStatus,
};

fn allowance() -> SourceGeometryLimits {
    SourceGeometryLimits {
        elements: 1_000_000,
        variable_bytes: 1_000_000,
    }
}

#[test]
fn geometry_uses_utf8_bytes_and_exact_cumulative_boundaries() {
    let mut geometry = SourceGeometry::new(SourceGeometryLimits {
        elements: 2,
        variable_bytes: 7,
    });
    geometry.text("猫").unwrap();
    geometry.text("🦊").unwrap();
    assert_eq!(geometry.used().elements, 2);
    assert_eq!(geometry.used().variable_bytes, 7);
    assert_eq!(
        geometry.text(""),
        Err(SourceGeometryError::Limit {
            dimension: SourceGeometryDimension::Elements,
            requested: 3,
            limit: 2,
        })
    );
    let mut geometry = SourceGeometry::new(SourceGeometryLimits {
        elements: 1,
        variable_bytes: 2,
    });
    assert_eq!(
        geometry.text("猫"),
        Err(SourceGeometryError::Limit {
            dimension: SourceGeometryDimension::VariableBytes,
            requested: 3,
            limit: 2,
        })
    );
    assert_eq!(geometry.used().variable_bytes, 0);
}

#[test]
fn nested_comparator_container_is_admitted_before_any_element() {
    let version = MusubiVersionV1 {
        major: 1,
        minor: 0,
        patch: 0,
        prerelease: vec![MusubiPrereleaseIdentifierV1::AlphaNumeric("x".repeat(512))],
    };
    let comparator = MusubiVersionComparatorV1 {
        op: MusubiComparatorOpV1::GreaterOrEqual,
        version,
    };
    let requirement = MusubiVersionReqV1::Comparators(vec![comparator; 17]);
    let mut geometry = SourceGeometry::new(SourceGeometryLimits {
        elements: 17,
        variable_bytes: 0,
    });
    assert_eq!(
        geometry.requirement(&requirement),
        Err(SourceGeometryError::Limit {
            dimension: SourceGeometryDimension::Elements,
            requested: 18,
            limit: 17,
        })
    );
    assert_eq!(geometry.used().elements, 1);
    assert_eq!(geometry.used().variable_bytes, 0);
    // Invalid but bounded shape is not promoted into semantic validity.
    let mut geometry = SourceGeometry::new(allowance());
    geometry.requirement(&requirement).unwrap();
    assert!(requirement.validate().is_err());
    assert_eq!(geometry.used().variable_bytes, 17 * 512);
}

#[test]
fn account_keys_and_multisig_members_use_checked_borrowed_payloads() {
    let keys = [account(11), account(12)];
    let members = keys
        .iter()
        .map(|account| {
            let AccountController::Single(key) = account.controller() else {
                panic!("single-key fixture")
            };
            MultisigMember::new(key.clone(), 1).unwrap()
        })
        .collect();
    let account = AccountId::new_multisig(MultisigPolicy::new(2, members).unwrap());
    let mut geometry = SourceGeometry::new(SourceGeometryLimits {
        elements: 7,
        variable_bytes: 64,
    });
    geometry.admit(SourceShape::Account(&account)).unwrap();
    assert_eq!(geometry.used().elements, 7);
    assert_eq!(geometry.used().variable_bytes, 64);
    let AccountController::Single(key) = keys[0].controller() else {
        panic!("single-key fixture")
    };
    let mut discarded = key.clone();
    discarded.zeroize_for_confidential_discard();
    let mut geometry = SourceGeometry::new(allowance());
    geometry.key(&discarded).unwrap();
    assert_eq!(geometry.used().variable_bytes, 0);
    assert!(discarded.try_to_bytes().is_err());
}

#[test]
fn shape_admission_preserves_canonical_bytes_without_validating_nested_text() {
    let mut manifest = release_manifest();
    manifest.metadata.description = Some(MusubiDescriptionV1("z".repeat(4097)));
    manifest.metadata.readme = Some("README.md".parse().unwrap());
    manifest.metadata.license = Some("Apache-2.0".parse().unwrap());
    manifest.metadata.repository = Some("https://example.invalid/repo".parse().unwrap());
    manifest.metadata.keywords = vec![MusubiKeywordV1("BAD".to_owned())];
    let mut dependency_package = manifest.release.package.clone();
    dependency_package.scope = MusubiPackageScopeV1::Domain("source".parse().unwrap());
    manifest.dependencies = vec![MusubiDependencyReqV1 {
        alias: "dep".parse().unwrap(),
        package: dependency_package,
        requirement: MusubiVersionReqV1::Caret("1.2.3-alpha.7".parse().unwrap()),
    }];
    let before = manifest.encode();
    let mut geometry = SourceGeometry::new(allowance());
    geometry.manifest(&manifest).unwrap();
    let exact = geometry.used();
    SourceGeometry::new(exact).manifest(&manifest).unwrap();
    let mut short = exact;
    short.variable_bytes -= 1;
    assert!(matches!(
        SourceGeometry::new(short).manifest(&manifest),
        Err(SourceGeometryError::Limit {
            dimension: SourceGeometryDimension::VariableBytes,
            ..
        })
    ));
    assert!(manifest.validate().is_err());
    assert_eq!(manifest.encode(), before);
}

#[test]
fn counters_refuse_overflow_without_wrapping_or_allocating_an_error_message() {
    let mut geometry = SourceGeometry::new(SourceGeometryLimits {
        elements: u64::MAX,
        variable_bytes: u64::MAX,
    });
    geometry.used.elements = u64::MAX;
    geometry.used.variable_bytes = u64::MAX;
    for dimension in [
        SourceGeometryDimension::Elements,
        SourceGeometryDimension::VariableBytes,
    ] {
        assert_eq!(
            geometry.add(dimension, 1),
            Err(SourceGeometryError::Overflow(dimension))
        );
    }
    assert_eq!(geometry.used().elements, u64::MAX);
    assert_eq!(geometry.used().variable_bytes, u64::MAX);
}

#[test]
fn current_evidence_bounds_only_the_fields_the_shared_predicate_reads() {
    let commitment = archive_commitment();
    let binding = provider_bundle_binding(account(13));
    let mut order = ReplicationOrderRecord {
        order_id: binding.replication_order,
        manifest_digest: ManifestDigest::new([4; 32]),
        manifest_root_cid: commitment.root_cid,
        musubi_archive: Some(binding.archive_id),
        issued_by: account(14),
        issued_epoch: 1,
        deadline_epoch: 2,
        canonical_order: vec![0x90; 32_768],
        assignment_revision: 1,
        provider_completions: vec![ReplicationOrderCompletionRecord {
            provider_id: binding.provider_id,
            completed_by: binding.completed_by.clone(),
            completion_epoch: binding.completion_epoch,
            assignment_revision: binding.assignment_revision,
            completion_authority: binding.completion_authority,
            finalized_anchor: binding.finalized_anchor,
        }],
        status: ReplicationOrderStatus::Completed(1),
    };
    let mut geometry = SourceGeometry::new(allowance());
    geometry.admit(SourceShape::Order(&order)).unwrap();
    let measured = geometry.used();
    order.canonical_order.clear();
    let mut smaller = SourceGeometry::new(measured);
    smaller.admit(SourceShape::Order(&order)).unwrap();
    assert_eq!(smaller.used(), measured);
    order
        .provider_completions
        .push(order.provider_completions[0].clone());
    assert!(
        SourceGeometry::new(measured)
            .admit(SourceShape::Order(&order))
            .is_err()
    );

    let pin = PinManifestRecord {
        digest: order.manifest_digest,
        root_cid: commitment.root_cid,
        chunker: commitment.chunker,
        chunk_digest_sha3_256: [2; 32],
        por_root: [3; 32],
        content_length: 1,
        policy: PinPolicy::default(),
        submitted_by: account(15),
        submitted_epoch: 1,
        approved_epoch: None,
        alias: None,
        successor_of: None,
        metadata: iroha_model_base::metadata::Metadata::default(),
        status: PinStatus::Pending,
        retirement_reason: Some("ignored".repeat(1024)),
        council_envelope_digest: None,
        pin_fee_payment: None,
    };
    let mut geometry = SourceGeometry::new(allowance());
    geometry.admit(SourceShape::Pin(&pin)).unwrap();
    assert_eq!(
        geometry.used().variable_bytes,
        u64::try_from(
            pin.chunker.namespace.len() + pin.chunker.name.len() + pin.chunker.semver.len()
        )
        .unwrap()
    );
}

#[test]
fn every_requirement_variant_and_retired_binding_remains_bounded() {
    let version = "1.2.3-alpha.7".parse::<MusubiVersionV1>().unwrap();
    let variants = [
        MusubiVersionReqV1::Any,
        MusubiVersionReqV1::MajorWildcard(1),
        MusubiVersionReqV1::MinorWildcard(MusubiMinorWildcardV1 { major: 1, minor: 2 }),
        MusubiVersionReqV1::Caret(version.clone()),
        MusubiVersionReqV1::Tilde(version.clone()),
        MusubiVersionReqV1::Exact(version),
    ];
    for (index, requirement) in variants.iter().enumerate() {
        let mut geometry = SourceGeometry::new(allowance());
        geometry.requirement(requirement).unwrap();
        assert_eq!(
            geometry.used().variable_bytes,
            if index < 3 { 0 } else { 5 }
        );
    }
    let commitment = archive_commitment();
    let binding = MusubiReplicationOrderArchiveBindingV1 {
        replication_order: ReplicationOrderId::new([0xa1; 32]),
        archive_id: commitment.archive_id(),
        commitment,
    };
    let location = MusubiArchiveLocationKeyV1::new(
        binding.archive_id,
        MusubiArchiveLocationIdV1::new([0xa2; 32]),
    );
    let mut reference = MusubiReplicationOrderLocationReferenceV1 {
        binding,
        lifecycle: MusubiReplicationOrderLocationLifecycleV1::PreLocation,
    };
    let mut geometry = SourceGeometry::new(allowance());
    geometry
        .admit(SourceShape::OrderBinding(&reference))
        .unwrap();
    let fixed = geometry.used();
    reference.lifecycle = MusubiReplicationOrderLocationLifecycleV1::Active(location);
    let mut geometry = SourceGeometry::new(fixed);
    geometry
        .admit(SourceShape::OrderBinding(&reference))
        .unwrap();
    assert_eq!(geometry.used(), fixed);
    reference.lifecycle = MusubiReplicationOrderLocationLifecycleV1::Retired(
        MusubiRetiredReplicationOrderLocationV1 {
            location,
            providers: vec![ProviderId::new([0xa3; 32]); 65],
        },
    );
    let mut geometry = SourceGeometry::new(fixed);
    assert_eq!(
        geometry.admit(SourceShape::OrderBinding(&reference)),
        Err(SourceGeometryError::Limit {
            dimension: SourceGeometryDimension::Elements,
            requested: fixed.elements + 65,
            limit: fixed.elements,
        })
    );
    assert_eq!(geometry.used(), fixed);
    SourceGeometry::new(allowance())
        .admit(SourceShape::OrderBinding(&reference))
        .unwrap();
    assert!(reference.validate().is_err());
}

#[test]
fn publication_shape_visits_governance_yank_and_optional_latest_version() {
    let manifest = release_manifest();
    let mut record = MusubiReleaseRecordV1 {
        release_digest: manifest.release_digest(),
        yank: MusubiReleaseYankV1 {
            release: manifest.release.clone(),
            yanked: false,
            reason: "initial".parse().unwrap(),
            changed_by: account(19),
            changed_at_height: 1,
            revision: 1,
        },
        manifest,
        published_by: account(20),
        published_at_height: 1,
        artifact_governance: MusubiArtifactGovernanceStateV1::Available,
        revisions: MusubiReleaseRevisionsV1 {
            yank: 1,
            artifact_governance: 1,
        },
    };
    let mut initial = SourceGeometry::new(allowance());
    initial.admit(SourceShape::Release(&record)).unwrap();
    record.artifact_governance =
        MusubiArtifactGovernanceStateV1::TakenDown(MusubiArtifactTakedownV1 {
            action_digest: MusubiGovernanceActionDigestV1::new([0x75; 32]),
            reason: "retained reason".parse().unwrap(),
            applied_at_height: 2,
        });
    let mut changed = SourceGeometry::new(allowance());
    changed.admit(SourceShape::Release(&record)).unwrap();
    assert_eq!(
        changed.used().variable_bytes,
        initial.used().variable_bytes + 15
    );
    let mut directory = MusubiOrderedPackageEntryV1 {
        selector: MusubiPackageSelectorV1 {
            namespace: "sora".parse().unwrap(),
            name: record.manifest.release.package.name.clone(),
        },
        package: record.manifest.release.package,
        latest_selectable: None,
        metadata_revision: 1,
        index_revision: 1,
    };
    let mut initial = SourceGeometry::new(allowance());
    initial.admit(SourceShape::Directory(&directory)).unwrap();
    directory.latest_selectable = Some("1.2.3-alpha.7".parse().unwrap());
    let mut changed = SourceGeometry::new(allowance());
    changed.admit(SourceShape::Directory(&directory)).unwrap();
    assert_eq!(
        changed.used().variable_bytes,
        initial.used().variable_bytes + 5
    );
}
