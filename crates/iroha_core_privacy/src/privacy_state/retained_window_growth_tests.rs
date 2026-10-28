// Exact attained cardinality is committed with each root head, independently of the policy cap.

macro_rules! assert_retained_window_mutations {
    ($fixture:ident) => {{
        let original = $fixture
            .roots
            .view()
            .iter()
            .map(|(key, origin)| (*key, origin.clone()))
            .collect::<Vec<_>>();
        let head = $fixture
            .root_heads
            .view()
            .get(&$fixture.head_key)
            .expect("head")
            .clone();
        assert_eq!(original.len(), 3);
        for mutation in 0..7 {
            $fixture.roots = original.clone().into_iter().collect();
            $fixture.root_heads.insert($fixture.head_key, head.clone());
            match mutation {
                0 => {
                    $fixture.roots = original[1..].iter().cloned().collect();
                }
                1 => {
                    // Rebinding the anchor to a missing first item must not hide truncation.
                    $fixture.roots = original[1..].iter().cloned().collect();
                    let mut changed = head.clone();
                    changed.retention_anchor = Some(
                        PrivacyRootRetentionAnchorV1::new(
                            original[0].0.epoch(),
                            original[0].0.root(),
                        )
                        .expect("locally valid advanced anchor"),
                    );
                    $fixture.root_heads.insert($fixture.head_key, changed);
                }
                2 => {
                    $fixture.roots = [original[0].clone(), original[2].clone()]
                        .into_iter()
                        .collect();
                }
                3 => {
                    let key = PrivacyRootKeyV1::new(
                        original[0].0.namespace(),
                        original[0].0.role(),
                        original[0].0.epoch(),
                        PrivacyRootV1::new(nonzero(0xFE)),
                    )
                    .expect("extra root key");
                    $fixture.roots.insert(key, original[0].1.clone());
                }
                4 => {
                    $fixture.roots.insert(original[0].0, original[1].1.clone());
                    $fixture.roots.insert(original[1].0, original[0].1.clone());
                }
                5 | 6 => {
                    let mut changed = head.clone();
                    changed.retained_root_count = if mutation == 5 { 2 } else { 4 };
                    $fixture.root_heads.insert($fixture.head_key, changed);
                }
                _ => unreachable!(),
            }
            assert!(
                $fixture.load_with_retention(5).is_err(),
                "mutation {mutation} must reject after expansion"
            );
        }
        $fixture.roots = original.into_iter().collect();
        $fixture.root_heads.insert($fixture.head_key, head);
        $fixture
            .load_with_retention(5)
            .expect("restored exact attained window");
    }};
}

macro_rules! assert_retention_growth {
    ($fixture:ident, $advance:expr) => {{
        for _ in 0..5 {
            ($advance)(&mut $fixture, 3);
        }
        let head = $fixture
            .root_heads
            .view()
            .get(&$fixture.head_key)
            .expect("head")
            .clone();
        assert_eq!(head.retained_root_count(), 3);
        assert!(head.retention_anchor().is_some());
        let before = head.encode();
        // This is the regression: the old anchored len == cap rule rejects here.
        $fixture
            .load_with_retention(5)
            .expect("increasing cap preserves attained three-root window");
        assert_eq!(
            $fixture
                .root_heads
                .view()
                .get(&$fixture.head_key)
                .expect("head")
                .encode(),
            before
        );
        let roots = norito::json::to_json(&$fixture.roots).expect("persist roots");
        let heads = norito::json::to_json(&$fixture.root_heads).expect("persist counted heads");
        $fixture.roots = norito::json::from_json(&roots).expect("restore roots");
        $fixture.root_heads = norito::json::from_json(&heads).expect("restore counted heads");
        $fixture
            .load_with_retention(5)
            .expect("unfilled expanded window survives restart");
        assert_retained_window_mutations!($fixture);
        for expected in [4, 5, 5] {
            ($advance)(&mut $fixture, 5);
            $fixture
                .load_with_retention(5)
                .expect("genuine committed append");
            assert_eq!($fixture.roots.view().iter().count(), expected);
            assert_eq!(
                $fixture
                    .root_heads
                    .view()
                    .get(&$fixture.head_key)
                    .expect("head")
                    .retained_root_count(),
                u32::try_from(expected).expect("small count")
            );
        }
        let plans = plan_privacy_root_retention_reduction_v1(
            &$fixture.roots.view(),
            $fixture.head_key.namespace().protocol_id(),
            3,
        )
        .expect("exact shrink plan");
        assert_eq!(plans.len(), 1);
        let plan = &plans[0];
        assert_eq!(plan.removal_keys.len(), 2);
        let head = $fixture
            .root_heads
            .view()
            .get(&$fixture.head_key)
            .expect("head")
            .clone();
        let count = head
            .retained_count_after_pruning(plan.removal_keys.len())
            .expect("attained shrink count");
        $fixture.roots = $fixture
            .roots
            .view()
            .iter()
            .filter(|(key, _)| !plan.removal_keys.contains(key))
            .map(|(key, origin)| (*key, origin.clone()))
            .collect();
        $fixture.root_heads.insert(
            $fixture.head_key,
            PrivacyRootHeadRecordV1::new(
                head.epoch(),
                head.root(),
                head.provenance().clone(),
                Some(plan.new_anchor),
                count,
            )
            .expect("atomic shrunken head"),
        );
        $fixture
            .load_with_retention(3)
            .expect("shrunk exact window");
        $fixture
            .load_with_retention(5)
            .expect("expansion after shrink does not recreate pruned roots");
        assert_retained_window_mutations!($fixture);
    }};
}

#[test]
fn pgc_retention_expansion_preserves_attained_window_across_restart_and_shrink() {
    let mut fixture = pgc_persisted_fixture();
    assert_retention_growth!(fixture, |fixture: &mut PgcPersistedFixture, cap| {
        fixture.advance_with_retention(cap);
    });
}

#[test]
fn orchard_retention_expansion_preserves_attained_window_across_restart_and_shrink() {
    let mut fixture = orchard_persisted_fixture();
    assert_retention_growth!(fixture, |fixture: &mut OrchardPersistedFixture, cap| {
        fixture.advance_with_retention(cap, &[[0; 32]]);
    });
}

#[test]
fn proof_managed_retention_expansion_preserves_attained_window_across_restart_and_shrink() {
    for protocol in [
        PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1,
        PrivacyProtocolIdV1::PqMaspStarkV1,
    ] {
        let mut fixture = proof_managed_note_persisted_fixture(protocol);
        assert_retention_growth!(fixture, |fixture: &mut ProofManagedPersistedFixture,
                                           cap| {
            let epoch = fixture
                .root_heads
                .view()
                .get(&fixture.head_key)
                .expect("head")
                .epoch();
            let commitment =
                PrivacyCommitmentV1::new(nonzero(u8::try_from(epoch + 30).expect("small epoch")));
            fixture.advance_with_retention(cap, &[commitment]);
        });
    }
}

struct RetainedZkAmsFixture {
    namespace: PrivacyNamespaceV1,
    bootstrap_digest: PrivacyZkAmsRegistryBootstrapDigestV1,
    commitments: Storage<PrivacyCommitmentKeyV1, PrivacyStateItemRecordV1>,
    roots: Storage<PrivacyRootKeyV1, PrivacyRootProvenanceV1>,
    root_heads: Storage<PrivacyRootHeadKeyV1, PrivacyRootHeadRecordV1>,
    head_key: PrivacyRootHeadKeyV1,
}
impl RetainedZkAmsFixture {
    fn load_with_retention(&self, cap: u32) -> Result<PrivacyZkAmsRegistrySnapshotV1, String> {
        load_privacy_zk_ams_registry_snapshot_v1(
            self.namespace,
            cap,
            &self.commitments.view(),
            &self.roots.view(),
            &self.root_heads.view(),
        )
    }
    fn advance_with_retention(&mut self, cap: u32) {
        let predecessor = self.load_with_retention(cap).expect("exact predecessor");
        let head = self
            .root_heads
            .view()
            .get(&self.head_key)
            .expect("head")
            .clone();
        let epoch = head.epoch() + 1;
        let root = PrivacyRootV1::new(nonzero(u8::try_from(epoch + 60).expect("small epoch")));
        let key = PrivacyRootKeyV1::new(
            self.namespace,
            PrivacyRootRoleV1::AccountRegistry,
            epoch,
            root,
        )
        .expect("successor root key");
        let origin = PrivacyRootProvenanceV1::zk_ams_registry_successor(
            self.bootstrap_digest,
            PrivacyStatementDigestV1::new(nonzero(0xDC)),
            epoch + 10,
            0,
            predecessor.current_epoch(),
            predecessor.current_root(),
        )
        .expect("typed successor");
        let removals = plan_privacy_root_history_update_v1(&self.roots.view(), &[key], cap)
            .expect("pruning plan");
        let count = head
            .retained_count_after_append(removals.len())
            .expect("attained count");
        let anchor = removals
            .last()
            .map(|key| {
                PrivacyRootRetentionAnchorV1::new(key.epoch(), key.root())
                    .expect("exact last removed root")
            })
            .or_else(|| head.retention_anchor());
        self.roots = self
            .roots
            .view()
            .iter()
            .filter(|(key, _)| !removals.contains(key))
            .map(|(key, origin)| (*key, origin.clone()))
            .collect();
        self.roots.insert(key, origin.clone());
        self.root_heads.insert(
            self.head_key,
            PrivacyRootHeadRecordV1::new(epoch, root, origin, anchor, count)
                .expect("committed successor head"),
        );
    }
}

#[test]
fn zk_ams_retention_expansion_preserves_attained_window_across_restart_and_shrink() {
    let namespace = zk_ams_namespace(0xDA);
    let bootstrap_digest = PrivacyZkAmsRegistryBootstrapDigestV1::new(nonzero(0xDB));
    let root = PrivacyRootV1::new(nonzero(61));
    let head_key =
        PrivacyRootHeadKeyV1::new(namespace, PrivacyRootRoleV1::AccountRegistry).expect("head key");
    let key = PrivacyRootKeyV1::new(namespace, PrivacyRootRoleV1::AccountRegistry, 1, root)
        .expect("root key");
    let origin =
        PrivacyRootProvenanceV1::zk_ams_registry_bootstrap(bootstrap_digest, 7).expect("origin");
    let issuer_key = PrivacyCommitmentKeyV1::zk_ams_issuer_policy_record(
        namespace,
        PrivacyZkAmsIssuerPolicyRecordDigestV1::new(nonzero(0xDD)),
    )
    .expect("issuer key");
    let mut fixture = RetainedZkAmsFixture {
        namespace,
        bootstrap_digest,
        head_key,
        commitments: [(
            issuer_key,
            PrivacyStateItemRecordV1::zk_ams_governance(bootstrap_digest, 7)
                .expect("issuer record"),
        )]
        .into_iter()
        .collect(),
        roots: [(key, origin.clone())].into_iter().collect(),
        root_heads: [(
            head_key,
            PrivacyRootHeadRecordV1::new(1, root, origin, None, 1).expect("bootstrap head"),
        )]
        .into_iter()
        .collect(),
    };
    assert_retention_growth!(fixture, |fixture: &mut RetainedZkAmsFixture, cap| {
        fixture.advance_with_retention(cap);
    });
}

#[test]
fn retained_head_count_transitions_reject_overpruning_overflow_and_retired_layouts() {
    let fixture = pgc_persisted_fixture();
    let head = fixture
        .root_heads
        .view()
        .get(&fixture.head_key)
        .expect("head")
        .clone();
    assert_eq!(head.retained_root_count(), 1);
    assert_eq!(head.retained_count_after_append(0), Ok(2));
    assert_eq!(head.retained_count_after_append(1), Ok(1));
    assert!(head.retained_count_after_append(2).is_err());
    assert!(head.retained_count_after_append(usize::MAX).is_err());
    assert_eq!(head.retained_count_after_pruning(0), Ok(1));
    assert!(head.retained_count_after_pruning(1).is_err());
    assert!(head.retained_count_after_pruning(usize::MAX).is_err());
    let mut maximum = head.clone();
    maximum.retained_root_count = u32::MAX;
    assert!(maximum.retained_count_after_append(0).is_err());
    assert_eq!(maximum.retained_count_after_append(1), Ok(u32::MAX));
    for (observed, cap) in [(0, 5), (2, 5), (1, 0)] {
        assert!(head.validate_retained_history_count(observed, cap).is_err());
    }
    head.validate_retained_history_count(1, 5)
        .expect("larger cap does not fabricate cardinality");
    assert!(
        PrivacyRootHeadRecordV1::new(
            head.epoch(),
            head.root(),
            head.provenance().clone(),
            None,
            0
        )
        .is_err()
    );
    let encoded = head.encode();
    assert_eq!(
        norito::codec::decode_adaptive::<PrivacyRootHeadRecordV1>(&encoded)
            .expect("counted binary roundtrip"),
        head
    );
    let json = norito::json::to_json(&head).expect("counted JSON");
    assert_eq!(
        norito::json::from_json::<PrivacyRootHeadRecordV1>(&json).expect("counted JSON roundtrip"),
        head
    );
    let retired_json = json.replace(",\"retained_root_count\":1", "");
    assert_ne!(retired_json, json);
    assert!(norito::json::from_json::<PrivacyRootHeadRecordV1>(&retired_json).is_err());
    let mut retired_binary = Vec::new();
    for field in [
        head.epoch().encode(),
        head.root().encode(),
        head.provenance().encode(),
        head.retention_anchor().encode(),
    ] {
        append_state_item_wire_field(&mut retired_binary, &field);
    }
    assert!(norito::codec::decode_adaptive::<PrivacyRootHeadRecordV1>(&retired_binary).is_err());
}

#[test]
fn fcmp_retention_expansion_preserves_attained_window_across_restart_and_shrink() {
    let mut fixture = fcmp_persisted_fixture();
    assert_retention_growth!(fixture, |fixture: &mut FcmpPersistedFixture, cap| {
        let epoch = fixture
            .root_heads
            .view()
            .get(&fixture.head_key)
            .expect("head")
            .epoch();
        let outputs = sorted_fcmp_output_tuples(&[u64::from(
            u8::try_from(epoch + 100).expect("small epoch"),
        )]);
        fixture.advance_with_retention(cap, &outputs);
    });
}

#[test]
fn pending_retention_decrease_restores_current_window_and_accepts_early_pruning() {
    let mut fixture = orchard_persisted_fixture();
    for _ in 0..6 {
        fixture.advance_with_retention(5, &[[0; 32]]);
    }
    let mut current_limits = PrivacyConsensusLimitsV1::taira_default();
    current_limits.retained_root_count = 5;
    let mut next_limits = current_limits;
    next_limits.retained_root_count = 3;
    let policy = PrivacyConsensusPolicyV1 {
        current_limits,
        pending_update: Some(iroha_data_model::privacy::PrivacyConsensusPolicyUpdateV1 {
            scheduled_at_height: 100,
            effective_at_height: 400,
            next_limits,
        }),
    };
    policy
        .validate_at_committed_height(100)
        .expect("future decrease");
    assert_eq!(policy.admission_retained_root_count(), 3);
    let policies = Storage::<PrivacyProtocolPolicyKeyV1, PrivacyProtocolPolicyV1>::new();
    let validate = |fixture: &OrchardPersistedFixture| {
        validate_privacy_persisted_state_v1(
            &policy,
            &policies.view(),
            PrivacyPgcStateReadV1 {
                accounts: &fixture.pgc_accounts.view(),
                pool_invariants: &fixture.pgc_pool_invariants.view(),
            },
            &fixture.nullifiers.view(),
            &fixture.commitments.view(),
            &fixture.roots.view(),
            &fixture.root_heads.view(),
        )
    };
    assert_eq!(fixture.roots.view().iter().count(), 5);
    validate(&fixture).expect("pending decrease must not invalidate committed history");
    let roots = norito::json::to_json(&fixture.roots).expect("persist roots");
    let heads = norito::json::to_json(&fixture.root_heads).expect("persist heads");
    fixture.roots = norito::json::from_json(&roots).expect("restore roots");
    fixture.root_heads = norito::json::from_json(&heads).expect("restore heads");
    validate(&fixture).expect("pending decrease survives restart before its effective height");
    fixture
        .load_with_retention(policy.current_limits.retained_root_count)
        .expect("read the current predecessor before planning a smaller append window");
    assert!(
        fixture
            .load_with_retention(policy.admission_retained_root_count())
            .is_err(),
        "using admission cap to read the old window is the regression"
    );
    let plan = plan_privacy_root_retention_reduction_v1(
        &fixture.roots.view(),
        fixture.namespace.protocol_id(),
        policy.admission_retained_root_count(),
    )
    .expect("exact prefix plan")
    .pop()
    .expect("one window");
    let head = fixture
        .root_heads
        .view()
        .get(&fixture.head_key)
        .expect("head")
        .clone();
    let count = head
        .retained_count_after_pruning(plan.removal_keys.len())
        .expect("exact count");
    fixture.roots = fixture
        .roots
        .view()
        .iter()
        .filter(|(key, _)| !plan.removal_keys.contains(key))
        .map(|(key, origin)| (*key, origin.clone()))
        .collect();
    fixture.root_heads.insert(
        fixture.head_key,
        PrivacyRootHeadRecordV1::new(
            head.epoch(),
            head.root(),
            head.provenance().clone(),
            Some(plan.new_anchor),
            count,
        )
        .expect("counted pruned head"),
    );
    assert_eq!(count, 3);
    validate(&fixture)
        .expect("an authenticated shorter window is valid under the current larger cap");
}
