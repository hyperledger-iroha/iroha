// Committed root counts follow actual appends/pruning, independently of policy targets.

#[test]
fn root_count_planner_preserves_expansion_growth_and_transaction_rollback() {
    let state = state_with_default_privacy_policy();
    let mut block = state.block(test_header());
    let key = PrivacyRootHeadKeyV1::new(
        x509_ca_namespace(),
        PrivacyRootRoleV1::CertificateAuthorityMembership,
    )
    .expect("CA head key");
    {
        let mut transaction = block.transaction();
        transaction
            .world
            .privacy_consensus_policy
            .get_mut()
            .current_limits
            .retained_root_count = 3;
        let mut previous = None;
        let mut current = None;
        for epoch in 1..=6 {
            let record = x509_trust_anchor(
                epoch,
                u8::try_from(epoch + 60).expect("small epoch"),
                previous,
                PrivacyZkX509RecordLifecycleV1::Active,
            );
            let plan =
                plan_zk_x509_ca_root_append_v1(record, &transaction).expect("typed CA append plan");
            assert_eq!(
                plan.next_head.retained_root_count(),
                u32::try_from(epoch.min(3)).expect("small count")
            );
            previous = Some(record.record_digest);
            current = Some(record);
            apply_zk_x509_root_append_v1(plan, &mut transaction);
        }
        let head = transaction
            .world
            .privacy_root_heads
            .get(&key)
            .expect("attained head")
            .clone();
        let before = norito::to_bytes(&head).expect("head bytes");
        assert_eq!(head.retained_root_count(), 3);
        assert_eq!(transaction.world.privacy_roots.iter().count(), 3);
        transaction
            .world
            .privacy_consensus_policy
            .get_mut()
            .current_limits
            .retained_root_count = 5;
        validate_current_zk_x509_ca_root_v1(current.expect("current record"), &transaction)
            .expect("raised target preserves exact three-root history");
        assert_eq!(
            norito::to_bytes(
                transaction
                    .world
                    .privacy_root_heads
                    .get(&key)
                    .expect("head")
            )
            .expect("head bytes"),
            before
        );
        for (epoch, expected_count, expected_prunes) in [(7, 4, 0), (8, 5, 0), (9, 5, 1)] {
            let record = x509_trust_anchor(
                epoch,
                u8::try_from(epoch + 60).expect("small epoch"),
                previous,
                PrivacyZkX509RecordLifecycleV1::Active,
            );
            let before_roots = transaction
                .world
                .privacy_roots
                .iter()
                .map(|(key, value)| (*key, value.clone()))
                .collect::<Vec<_>>();
            let plan = plan_zk_x509_ca_root_append_v1(record, &transaction)
                .expect("growing CA append plan");
            assert_eq!(plan.next_head.retained_root_count(), expected_count);
            assert_eq!(plan.removals.len(), expected_prunes);
            assert_eq!(
                transaction
                    .world
                    .privacy_roots
                    .iter()
                    .map(|(key, value)| (*key, value.clone()))
                    .collect::<Vec<_>>(),
                before_roots,
                "planning itself cannot mutate retained roots"
            );
            previous = Some(record.record_digest);
            apply_zk_x509_root_append_v1(plan, &mut transaction);
            validate_current_zk_x509_ca_root_v1(record, &transaction)
                .expect("exact committed append");
        }
        // Dropping this StateTransaction must roll back every root, anchor, count and policy write.
    }
    assert!(block.world.privacy_roots.iter().next().is_none());
    assert!(block.world.privacy_root_heads.get(&key).is_none());
}

fn retained_x509_policy_world(current_cap: u32, next_cap: u32) -> (World, PrivacyRootHeadKeyV1) {
    let mut world = World::default();
    let namespace = x509_ca_namespace();
    let role = PrivacyRootRoleV1::CertificateAuthorityMembership;
    let head_key = PrivacyRootHeadKeyV1::new(namespace, role).expect("head key");
    let mut current_limits = PrivacyConsensusLimitsV1::taira_default();
    current_limits.retained_root_count = current_cap;
    let mut next_limits = current_limits;
    next_limits.retained_root_count = next_cap;
    world.privacy_consensus_policy =
        mv::cell::Cell::new(iroha_data_model::privacy::PrivacyConsensusPolicyV1 {
            current_limits,
            pending_update: Some(PrivacyConsensusPolicyUpdateV1 {
                scheduled_at_height: 100,
                effective_at_height: 400,
                next_limits,
            }),
        });
    let mut previous = None;
    let mut anchor = None;
    for epoch in 1..=6 {
        let record = x509_trust_anchor(
            epoch,
            u8::try_from(epoch + 80).expect("small epoch"),
            previous,
            PrivacyZkX509RecordLifecycleV1::Active,
        );
        previous = Some(record.record_digest);
        world.privacy_commitments.insert(
            PrivacyCommitmentKeyV1::zk_x509_trust_anchor_revision(
                record.trust_anchor_id,
                record.record_epoch,
            )
            .expect("revision key"),
            PrivacyStateItemRecordV1::zk_x509_trust_anchor_governance(record, 7)
                .expect("typed record"),
        );
        let publication =
            PrivacyRootPublicationV1::new(namespace, role, epoch, record.ca_membership_root)
                .expect("publication");
        let provenance = PrivacyRootProvenanceV1::zk_x509_ca_governance(
            publication.digest().expect("digest"),
            namespace,
            epoch,
            record.ca_membership_root,
            record,
            7,
        )
        .expect("typed root provenance");
        if epoch <= 6 - u64::from(current_cap) {
            anchor = Some(
                PrivacyRootRetentionAnchorV1::new(epoch, record.ca_membership_root)
                    .expect("last pruned root"),
            );
        } else {
            world.privacy_roots.insert(
                PrivacyRootKeyV1::new(namespace, role, epoch, record.ca_membership_root)
                    .expect("retained root key"),
                provenance.clone(),
            );
        }
        if epoch == 6 {
            world.privacy_root_heads.insert(
                head_key,
                PrivacyRootHeadRecordV1::new(
                    epoch,
                    record.ca_membership_root,
                    provenance,
                    anchor,
                    current_cap,
                )
                .expect("counted head"),
            );
        }
    }
    (world, head_key)
}

#[test]
fn root_window_policy_applies_growth_and_shrink_only_at_effective_block() {
    for (current_cap, next_cap, expected_count) in [(3, 5, 3), (5, 3, 3)] {
        let (world, key) = retained_x509_policy_world(current_cap, next_cap);
        let original_head = world
            .privacy_root_heads
            .view()
            .get(&key)
            .expect("initial head")
            .clone();
        // Construction exercises restored-state validation while the shrink is still pending.
        let state = State::new(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        {
            let block = state.block(test_header_at(399));
            assert_eq!(
                block
                    .world
                    .privacy_consensus_policy
                    .get()
                    .current_limits
                    .retained_root_count,
                current_cap
            );
            assert_eq!(
                block.world.privacy_root_heads.get(&key),
                Some(&original_head)
            );
            assert_eq!(
                block.world.privacy_roots.iter().count(),
                usize::try_from(current_cap).expect("small count")
            );
        }
        {
            let block = state.block(test_header_at(400));
            assert_eq!(
                block
                    .world
                    .privacy_consensus_policy
                    .get()
                    .current_limits
                    .retained_root_count,
                next_cap
            );
            assert!(
                block
                    .world
                    .privacy_consensus_policy
                    .get()
                    .pending_update
                    .is_none()
            );
            let head = block
                .world
                .privacy_root_heads
                .get(&key)
                .expect("effective head");
            assert_eq!(head.retained_root_count(), expected_count);
            assert_eq!(
                block.world.privacy_roots.iter().count(),
                usize::try_from(expected_count).expect("small count")
            );
            assert_eq!(head.epoch(), original_head.epoch());
            assert_eq!(head.root(), original_head.root());
            assert_eq!(head.provenance(), original_head.provenance());
            if next_cap > current_cap {
                assert_eq!(
                    head, &original_head,
                    "expansion cannot rewrite an attained window"
                );
            } else {
                assert_eq!(head.retention_anchor().expect("pruned prefix").epoch(), 3);
            }
        }
    }
}

#[test]
fn pending_root_shrink_reads_current_history_then_prunes_on_first_append() {
    let (world, key) = retained_x509_policy_world(5, 3);
    let state = State::new(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let mut block = state.block(test_header_at(399));
    let mut transaction = block.transaction();
    let predecessor = transaction
        .world
        .privacy_root_heads
        .get(&key)
        .expect("current head")
        .clone();
    let PrivacyRootProvenanceV1::ZkX509CaGovernance {
        trust_anchor_record,
        ..
    } = predecessor.provenance()
    else {
        panic!("typed CA head");
    };
    validate_current_zk_x509_ca_root_v1(**trust_anchor_record, &transaction)
        .expect("pending smaller admission cap must not reject the current predecessor");
    let next = x509_trust_anchor(
        7,
        87,
        Some(trust_anchor_record.record_digest),
        PrivacyZkX509RecordLifecycleV1::Active,
    );
    let plan =
        plan_zk_x509_ca_root_append_v1(next, &transaction).expect("append while decrease pending");
    assert_eq!(
        plan.removals.len(),
        3,
        "five existing plus one new minus cap three"
    );
    assert_eq!(plan.next_head.retained_root_count(), 3);
    assert_eq!(
        plan.next_head
            .retention_anchor()
            .expect("pruned root")
            .epoch(),
        4
    );
    apply_zk_x509_root_append_v1(plan, &mut transaction);
    assert_eq!(
        transaction
            .world
            .privacy_consensus_policy
            .get()
            .current_limits
            .retained_root_count,
        5
    );
    validate_current_zk_x509_ca_root_v1(next, &transaction)
        .expect("early-pruned exact count remains valid under current cap five");
    assert_eq!(transaction.world.privacy_roots.iter().count(), 3);
}
