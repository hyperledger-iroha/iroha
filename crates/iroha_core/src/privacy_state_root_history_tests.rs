#[test]
fn compiled_limits_keep_bounded_root_history() {
    assert_eq!(
        PrivacyConsensusLimitsV1::taira_default().retained_root_count,
        2_048
    );
}
#[test]
fn root_history_prunes_independently_per_namespace() {
    let mut roots = Storage::new();
    let record = root_provenance();
    let independent_namespace = pgc_namespace(0xE1);
    for epoch in 1..=2_048 {
        let root_byte = (epoch % 251 + 1) as u8;
        roots.insert(
            x509_root_key(
                PrivacyRootRoleV1::CertificateAuthorityMembership,
                epoch,
                root_byte,
            ),
            record,
        );
        roots.insert(
            PrivacyRootKeyV1::new(
                independent_namespace,
                PrivacyRootRoleV1::PgcAccountState,
                epoch,
                PrivacyRootV1::new(nonzero(root_byte)),
            )
            .expect("independent root key"),
            record,
        );
    }
    let added = x509_root_key(PrivacyRootRoleV1::CertificateAuthorityMembership, 2_049, 43);
    let removals =
        plan_privacy_root_history_update_v1(&roots.view(), &[added], 2_048).expect("valid plan");
    assert_eq!(
        removals,
        vec![x509_root_key(
            PrivacyRootRoleV1::CertificateAuthorityMembership,
            1,
            2
        )]
    );
    assert!(
        roots
            .view()
            .get(
                &PrivacyRootKeyV1::new(
                    independent_namespace,
                    PrivacyRootRoleV1::PgcAccountState,
                    1,
                    PrivacyRootV1::new(nonzero(2)),
                )
                .expect("independent root key")
            )
            .is_some(),
        "planning one namespace must not prune another namespace"
    );
}
#[test]
fn root_history_rejects_replays_epoch_conflicts_and_stale_epochs() {
    let mut roots = Storage::new();
    roots.insert(
        x509_root_key(PrivacyRootRoleV1::CertificateAuthorityMembership, 7, 70),
        root_provenance(),
    );
    let exact_replay = x509_root_key(PrivacyRootRoleV1::CertificateAuthorityMembership, 7, 70);
    assert!(matches!(
        plan_privacy_root_history_update_v1(&roots.view(), &[exact_replay], 8),
        Err(PrivacyRootHistoryErrorV1::ExistingRoot { key }) if key == exact_replay
    ));
    assert!(matches!(
        plan_privacy_root_history_update_v1(
            &roots.view(),
            &[x509_root_key(
                PrivacyRootRoleV1::CertificateAuthorityMembership,
                7,
                71,
            )],
            8,
        ),
        Err(PrivacyRootHistoryErrorV1::EpochConflict { epoch: 7, .. })
    ));
    assert!(matches!(
        plan_privacy_root_history_update_v1(
            &roots.view(),
            &[x509_root_key(
                PrivacyRootRoleV1::CertificateAuthorityMembership,
                6,
                60,
            )],
            8,
        ),
        Err(PrivacyRootHistoryErrorV1::NonMonotonicEpoch {
            latest_epoch: 7,
            added_epoch: 6,
            ..
        })
    ));
}
#[test]
fn root_history_rejects_duplicate_and_over_capacity_effects() {
    let roots = Storage::new();
    let duplicate = x509_root_key(PrivacyRootRoleV1::CertificateAuthorityMembership, 1, 10);
    assert!(matches!(
        plan_privacy_root_history_update_v1(&roots.view(), &[duplicate, duplicate], 8),
        Err(PrivacyRootHistoryErrorV1::DuplicateAddedRoot { key }) if key == duplicate
    ));
    let additions = [
        x509_root_key(PrivacyRootRoleV1::CertificateAuthorityMembership, 1, 10),
        x509_root_key(PrivacyRootRoleV1::CertificateAuthorityMembership, 2, 20),
        x509_root_key(PrivacyRootRoleV1::CertificateAuthorityMembership, 3, 30),
    ];
    assert!(matches!(
        plan_privacy_root_history_update_v1(&roots.view(), &additions, 2),
        Err(PrivacyRootHistoryErrorV1::AddedRootsExceedRetention { count: 3, max: 2 })
    ));
}

fn anchored_root_history_shape_for_test(
    roots: &Storage<PrivacyRootKeyV1, PrivacyRootProvenanceV1>,
    head_key: PrivacyRootHeadKeyV1,
    keep: usize,
) -> (
    Storage<PrivacyRootKeyV1, PrivacyRootProvenanceV1>,
    Storage<PrivacyRootHeadKeyV1, PrivacyRootHeadRecordV1>,
) {
    let ordered = roots
        .view()
        .iter()
        .map(|(key, provenance)| (*key, *provenance))
        .collect::<Vec<_>>();
    assert!(keep > 0 && keep < ordered.len());
    let first_retained = ordered.len() - keep;
    let anchor_key = ordered[first_retained - 1].0;
    let retained = ordered[first_retained..]
        .iter()
        .copied()
        .collect::<Storage<_, _>>();
    let (latest_key, latest_provenance) = *ordered.last().expect("non-empty root history");
    let anchor = PrivacyRootRetentionAnchorV1::new(anchor_key.epoch(), anchor_key.root())
        .expect("non-zero test retention anchor");
    let mut heads = Storage::new();
    heads.insert(
        head_key,
        PrivacyRootHeadRecordV1::new(
            latest_key.epoch(),
            latest_key.root(),
            latest_provenance,
            Some(anchor),
        )
        .expect("anchored test root head"),
    );
    (retained, heads)
}

#[test]
fn anchored_loaders_restore_current_or_pending_admission_windows() {
    let retention_window = PrivacyRootRetentionWindowV1 {
        current: 2,
        admission: 1,
    };

    // PGC: an existing two-root window remains valid during notice, while a
    // successor admitted in that window may atomically prune it to one root.
    let mut pgc = pgc_persisted_fixture();
    pgc.advance_with_retention(3);
    pgc.advance_with_retention(3);
    let (pgc_current_roots, pgc_current_heads) =
        anchored_root_history_shape_for_test(&pgc.roots, pgc.head_key, 2);
    load_privacy_pgc_pool_snapshot_v1(
        pgc.namespace,
        retention_window,
        &pgc.pgc_accounts.view(),
        &pgc.pgc_pool_invariants.view(),
        &pgc_current_roots.view(),
        &pgc_current_heads.view(),
    )
    .expect("pre-effective PGC window uses the current cap");
    let (pgc_admission_roots, pgc_admission_heads) =
        anchored_root_history_shape_for_test(&pgc.roots, pgc.head_key, 1);
    load_privacy_pgc_pool_snapshot_v1(
        pgc.namespace,
        retention_window,
        &pgc.pgc_accounts.view(),
        &pgc.pgc_pool_invariants.view(),
        &pgc_admission_roots.view(),
        &pgc_admission_heads.view(),
    )
    .expect("notice-window PGC successor may restore at the admission cap");

    let mut next_limits = PrivacyConsensusLimitsV1::taira_default();
    next_limits.retained_root_count = 1;
    let mut current_limits = PrivacyConsensusLimitsV1::taira_default();
    current_limits.retained_root_count = 2;
    let notice_policy = PrivacyConsensusPolicyV1 {
        current_limits,
        pending_tightening: Some(
            iroha_data_model::privacy::PrivacyConsensusPolicyTighteningV1 {
                scheduled_at_height: 1,
                effective_at_height: 301,
                next_limits,
            },
        ),
    };
    notice_policy
        .validate()
        .expect("valid two-to-one notice window");
    for (label, roots, heads) in [
        (
            "pre-effective current-size",
            &pgc_current_roots,
            &pgc_current_heads,
        ),
        (
            "post-successor admission-size",
            &pgc_admission_roots,
            &pgc_admission_heads,
        ),
    ] {
        validate_privacy_persisted_state_v1(
            &notice_policy,
            &pgc.activations.view(),
            &pgc.pgc_accounts.view(),
            &pgc.pgc_pool_invariants.view(),
            &pgc.nullifiers.view(),
            &pgc.commitments.view(),
            &roots.view(),
            &heads.view(),
        )
        .unwrap_or_else(|error| panic!("{label} PGC restore failed: {error}"));
    }

    // Orchard uses the same two-phase window, but its compact frontier must
    // still agree with the newest retained root after either pruning shape.
    let mut orchard = orchard_persisted_fixture();
    orchard.advance_with_retention(3, &[[0x31; 32]]);
    orchard.advance_with_retention(3, &[[0x32; 32]]);
    for (label, keep) in [("current", 2), ("admission", 1)] {
        let (roots, heads) =
            anchored_root_history_shape_for_test(&orchard.roots, orchard.head_key, keep);
        load_privacy_orchard_pool_snapshot_v1(
            orchard.namespace,
            retention_window,
            &orchard.commitments.view(),
            &roots.view(),
            &heads.view(),
        )
        .unwrap_or_else(|error| panic!("{label}-size Orchard restore failed: {error}"));
    }

    // The private-IVM fixture exercises the shared proof-managed loader used
    // by private-IVM, FCMP++, and PQ-MASP pools.
    let mut proof_managed = proof_managed_persisted_fixture();
    proof_managed.advance(&[PrivacyCommitmentV1::new(nonzero(0xC1))]);
    proof_managed.advance(&[PrivacyCommitmentV1::new(nonzero(0xC2))]);
    for (label, keep) in [("current", 2), ("admission", 1)] {
        let (roots, heads) = anchored_root_history_shape_for_test(
            &proof_managed.roots,
            proof_managed.head_key,
            keep,
        );
        load_privacy_proof_managed_pool_snapshot_v1(
            proof_managed.namespace,
            retention_window,
            &proof_managed.commitments.view(),
            &roots.view(),
            &heads.view(),
        )
        .unwrap_or_else(|error| panic!("{label}-size proof-managed restore failed: {error}"));
    }

    // ZK-AMS has no cheap native-proof fixture, but its authoritative loader
    // can restore a fully typed synthetic successor chain without proving.
    let zk_ams_namespace = zk_ams_namespace(0xD1);
    let zk_ams_bootstrap_digest = PrivacyZkAmsRegistryBootstrapDigestV1::new(nonzero(0xD2));
    let issuer_record_digest = PrivacyZkAmsIssuerPolicyRecordDigestV1::new(nonzero(0xD3));
    let issuer_record_key =
        PrivacyCommitmentKeyV1::zk_ams_issuer_policy_record(zk_ams_namespace, issuer_record_digest)
            .expect("ZK-AMS issuer-policy key");
    let mut zk_ams_commitments = Storage::new();
    zk_ams_commitments.insert(
        issuer_record_key,
        PrivacyStateItemRecordV1::zk_ams_governance(zk_ams_bootstrap_digest, 1)
            .expect("ZK-AMS governed issuer record"),
    );
    let mut zk_ams_roots = Storage::new();
    let mut zk_ams_parent = None;
    for epoch in 1..=3_u64 {
        let root = PrivacyRootV1::new(indexed_nonzero(0xD4, epoch));
        let provenance = match zk_ams_parent {
            None => PrivacyRootProvenanceV1::zk_ams_registry_bootstrap(zk_ams_bootstrap_digest, 1)
                .expect("ZK-AMS bootstrap provenance"),
            Some((parent_epoch, parent_root)) => {
                PrivacyRootProvenanceV1::zk_ams_registry_successor(
                    zk_ams_bootstrap_digest,
                    PrivacyStatementDigestV1::new(indexed_nonzero(0xD5, epoch)),
                    epoch,
                    0,
                    parent_epoch,
                    parent_root,
                )
                .expect("ZK-AMS successor provenance")
            }
        };
        zk_ams_roots.insert(
            PrivacyRootKeyV1::new(
                zk_ams_namespace,
                PrivacyRootRoleV1::AccountRegistry,
                epoch,
                root,
            )
            .expect("ZK-AMS root key"),
            provenance,
        );
        zk_ams_parent = Some((epoch, root));
    }
    let zk_ams_head_key =
        PrivacyRootHeadKeyV1::new(zk_ams_namespace, PrivacyRootRoleV1::AccountRegistry)
            .expect("ZK-AMS head key");
    for (label, keep) in [("current", 2), ("admission", 1)] {
        let (roots, heads) =
            anchored_root_history_shape_for_test(&zk_ams_roots, zk_ams_head_key, keep);
        load_privacy_zk_ams_registry_snapshot_v1(
            zk_ams_namespace,
            retention_window,
            &zk_ams_commitments.view(),
            &roots.view(),
            &heads.view(),
        )
        .unwrap_or_else(|error| panic!("{label}-size ZK-AMS restore failed: {error}"));
    }

    // X.509 checks the inverse path: the current-sized predecessor must load
    // before governance can append and prune its successor to the admission cap.
    let trust_anchor_id = PrivacyIssuerIdV1::new(nonzero(41));
    let policy_id = PrivacyPolicyIdV1::new(nonzero(42));
    let anchor_1 = x509_trust_anchor_record(
        trust_anchor_id,
        1,
        0x61,
        None,
        PrivacyZkX509RecordLifecycleV1::Active,
    );
    let anchor_2 = x509_trust_anchor_record(
        trust_anchor_id,
        2,
        0x62,
        Some(anchor_1.record_digest),
        PrivacyZkX509RecordLifecycleV1::Active,
    );
    let anchor_3 = x509_trust_anchor_record(
        trust_anchor_id,
        3,
        0x63,
        Some(anchor_2.record_digest),
        PrivacyZkX509RecordLifecycleV1::Active,
    );
    let mut x509_commitments = Storage::new();
    for (height, anchor) in [(1, anchor_1), (2, anchor_2), (3, anchor_3)] {
        insert_x509_trust_anchor(&mut x509_commitments, anchor, height);
    }
    insert_x509_certificate_policy(
        &mut x509_commitments,
        x509_certificate_policy_record(
            trust_anchor_id,
            policy_id,
            1,
            0x71,
            vec![0, 3],
            None,
            PrivacyZkX509RecordLifecycleV1::Active,
        ),
        1,
    );
    insert_x509_crl(
        &mut x509_commitments,
        x509_crl_record(
            trust_anchor_id,
            policy_id,
            1,
            1,
            0x72,
            None,
            PrivacyZkX509RecordLifecycleV1::Active,
        ),
        1,
    );
    let mut x509_roots = Storage::new();
    for (height, anchor) in [(1, anchor_1), (2, anchor_2), (3, anchor_3)] {
        let key = x509_root_key(
            PrivacyRootRoleV1::CertificateAuthorityMembership,
            anchor.ca_membership_root_epoch,
            anchor.ca_membership_root.as_bytes()[0],
        );
        x509_roots.insert(key, x509_root_provenance(key, anchor, height));
    }
    let x509_head_key = PrivacyRootHeadKeyV1::new(
        x509_ca_namespace(),
        PrivacyRootRoleV1::CertificateAuthorityMembership,
    )
    .expect("X.509 CA head key");
    for (label, keep) in [("current", 2), ("admission", 1)] {
        let (roots, heads) = anchored_root_history_shape_for_test(&x509_roots, x509_head_key, keep);
        load_privacy_zk_x509_authoritative_state_v1(
            trust_anchor_id,
            policy_id,
            retention_window,
            &x509_commitments.view(),
            &roots.view(),
            &heads.view(),
        )
        .unwrap_or_else(|error| panic!("{label}-size X.509 restore failed: {error}"));
    }

    assert!(
        validate_anchored_retention_window_len_v1(
            "test root",
            3,
            1,
            Some(
                PrivacyRootRetentionAnchorV1::new(1, PrivacyRootV1::new(nonzero(1)))
                    .expect("test anchor"),
            ),
            2,
        )
        .expect_err("an arbitrary anchored underfill must remain invalid")
        .contains("must fill current retention 3 or notice-window admission retention 1")
    );
}
