// Actual post-Apply persistence must use the identity of its immutable State cut.

fn post_checkpoint_tree(kura: &Kura) -> BTreeMap<std::path::PathBuf, Option<Vec<u8>>> {
    fn visit(
        root: &std::path::Path,
        path: &std::path::Path,
        entries: &mut BTreeMap<std::path::PathBuf, Option<Vec<u8>>>,
    ) {
        for entry in std::fs::read_dir(path).expect("read fixture Kura namespace") {
            let entry = entry.expect("read fixture Kura entry");
            let path = entry.path();
            let kind = entry.file_type().expect("read fixture Kura entry type");
            let relative = path.strip_prefix(root).expect("owned Kura path").to_owned();
            if kind.is_dir() {
                assert!(entries.insert(relative, None).is_none());
                visit(root, &path, entries);
            } else {
                assert!(kind.is_file(), "fixture has no links or special files");
                assert!(
                    entries
                        .insert(
                            relative,
                            Some(std::fs::read(path).expect("read Kura bytes"))
                        )
                        .is_none()
                );
            }
        }
    }
    let root = kura.store_root();
    let mut entries = BTreeMap::new();
    visit(&root, &root, &mut entries);
    entries
}

fn assert_post_checkpoint_boundary_refusal(error: &V2ApplyError, expected_component: &'static str) {
    assert!(
        matches!(
            error,
            V2ApplyError::SnapshotCapture(crate::snapshot::SnapshotCaptureError::CommitBoundary {
                component
            }) if *component == expected_component
        ),
        "expected {expected_component} capture refusal, got {error:?}"
    );
    assert!(error.rejection_identity().is_none());
    assert!(matches!(
        error.local_refusal(),
        Some(super::super::v2_body_store::LocalValidationRefusal::RecoveryRequired(_))
    ));
    // The real caller wraps post-publication failures as a local recovery
    // requirement. Neither the raw cause nor that wrapper authorizes rejection.
    let committed = V2ApplyError::committed_recovery_required("post-apply metadata", error);
    assert!(committed.requires_restart_recovery());
    assert!(committed.rejection_identity().is_none());
}

v2_apply_test!(
    post_apply_checkpoint_rejects_unpublished_state_before_missing_checkpoint_write,
    {
        let fixture = ApplyFixture::new();
        let mut store = fixture.reopen_body_store();
        fixture.kura.fail_next_wsv_checkpoint_write_for_tests();
        let failure = fixture
            .execute(&mut store)
            .expect_err("actual pre-WSV checkpoint write is interrupted");
        assert!(matches!(
            failure,
            V2ApplyError::CommittedRecoveryRequired {
                stage: "pre-WSV recovery checkpoint",
                ..
            }
        ));
        assert_eq!(fixture.state.committed_height(), 0);
        assert_eq!(fixture.kura.exact_durable_blocks_count().unwrap(), 1);
        assert!(fixture.kura.wsv_checkpoint(1).unwrap().is_none());
        assert!(fixture.kura.commit_manifest(1).unwrap().is_none());
        let artifact = fixture
            .kura
            .v2_finality_artifact(1)
            .expect("read actual durable finality")
            .expect("finality precedes the interrupted checkpoint");
        artifact
            .verify()
            .expect("original finality remains authentic");
        let files = post_checkpoint_tree(&fixture.kura);
        let state_hash = crate::snapshot::canonical_state_snapshot_hash(&fixture.state).unwrap();
        let error = fixture
            .service
            .persist_post_apply_metadata(&fixture.context, fixture.task.subject(), &artifact)
            .expect_err("State has not installed the durable block");
        assert_post_checkpoint_boundary_refusal(&error, "height");
        assert_eq!(post_checkpoint_tree(&fixture.kura), files);
        assert!(fixture.kura.wsv_checkpoint(1).unwrap().is_none());
        assert!(fixture.kura.commit_manifest(1).unwrap().is_none());
        assert_eq!(
            crate::snapshot::canonical_state_snapshot_hash(&fixture.state).unwrap(),
            state_hash
        );
        fixture
            .execute(&mut store)
            .expect("original actual Apply still completes after the local refusal");
        fixture.assert_complete();
    }
);

v2_apply_test!(
    post_apply_checkpoint_rejects_later_state_without_resampling_historical_capture,
    {
        let fixture = ApplyFixture::new();
        let mut store = fixture.reopen_body_store();
        fixture.execute(&mut store).expect("publish original block");
        fixture.assert_complete();
        let captured = crate::snapshot::CapturedStateSnapshot::capture(&fixture.state)
            .expect("capture actual completed first State");
        let first_hash = captured
            .canonical_hash_for_block(
                fixture.context.network_id,
                fixture.context.height,
                fixture.task.subject().block_hash,
            )
            .expect("captured first identity matches");
        assert_eq!(
            fixture
                .kura
                .wsv_checkpoint(1)
                .unwrap()
                .unwrap()
                .state_hash(),
            first_hash
        );
        let artifact = fixture.kura.v2_finality_artifact(1).unwrap().unwrap();
        let mut successor = build_successor_apply_fixture(&fixture);
        fixture
            .service
            .execute(&successor.context, &mut successor.store, &successor.task)
            .expect("publish actual authenticated successor");
        assert_eq!(fixture.state.committed_height(), 2);
        assert_eq!(
            fixture.state.latest_block_hash_fast(),
            Some(successor.body.hash())
        );
        let files = post_checkpoint_tree(&fixture.kura);
        let checkpoints =
            [1, 2].map(|height| fixture.kura.wsv_checkpoint(height).unwrap().unwrap());
        let manifests = [1, 2].map(|height| fixture.kura.commit_manifest(height).unwrap().unwrap());
        let error = fixture
            .service
            .persist_post_apply_metadata(&fixture.context, fixture.task.subject(), &artifact)
            .expect_err("a later State cut cannot be persisted for the first block");
        assert_post_checkpoint_boundary_refusal(&error, "height");
        assert_eq!(post_checkpoint_tree(&fixture.kura), files);
        assert_eq!(
            [1, 2].map(|height| fixture.kura.wsv_checkpoint(height).unwrap().unwrap()),
            checkpoints
        );
        assert_eq!(
            [1, 2].map(|height| fixture.kura.commit_manifest(height).unwrap().unwrap()),
            manifests
        );
        assert_eq!(
            captured
                .canonical_hash_for_block(
                    fixture.context.network_id,
                    fixture.context.height,
                    fixture.task.subject().block_hash,
                )
                .expect("retained immutable first cut never resamples the live State"),
            first_hash
        );
    }
);

v2_apply_test!(
    post_apply_checkpoint_rejects_wrong_tip_and_network_before_storage,
    {
        let fixture = ApplyFixture::new();
        let mut store = fixture.reopen_body_store();
        fixture
            .execute(&mut store)
            .expect("publish authentic fixture");
        fixture.assert_complete();
        let artifact = fixture.kura.v2_finality_artifact(1).unwrap().unwrap();
        let files = post_checkpoint_tree(&fixture.kura);
        let checkpoint = fixture.kura.wsv_checkpoint(1).unwrap().unwrap();
        let manifest = fixture.kura.commit_manifest(1).unwrap().unwrap();
        let state_hash = crate::snapshot::canonical_state_snapshot_hash(&fixture.state).unwrap();
        for component in ["block hash", "network"] {
            let mut context = fixture.context.clone();
            let mut subject = fixture.task.subject();
            if component == "block hash" {
                subject.block_hash =
                    HashOf::from_untyped_unchecked(Hash::new(b"another State tip"));
                assert_ne!(subject.block_hash, artifact.block_hash);
            } else {
                context.network_id = iroha_data_model::NetworkId::from_genesis_hash(
                    HashOf::from_untyped_unchecked(Hash::new(b"another snapshot network")),
                );
                assert_ne!(context.network_id, fixture.context.network_id);
            }
            // Keep the original real finality unchanged. A mismatched call must
            // fail on captured State identity before consulting durable writers.
            let error = fixture
                .service
                .persist_post_apply_metadata(&context, subject, &artifact)
                .expect_err("caller arguments cannot relabel the captured State");
            assert_post_checkpoint_boundary_refusal(&error, component);
            assert_eq!(post_checkpoint_tree(&fixture.kura), files);
            assert_eq!(fixture.kura.wsv_checkpoint(1).unwrap().unwrap(), checkpoint);
            assert_eq!(fixture.kura.commit_manifest(1).unwrap().unwrap(), manifest);
            assert_eq!(
                crate::snapshot::canonical_state_snapshot_hash(&fixture.state).unwrap(),
                state_hash
            );
        }
    }
);

v2_apply_test!(
    post_apply_checkpoint_matching_actual_committed_cut_is_idempotent,
    {
        let fixture = ApplyFixture::new();
        let mut store = fixture.reopen_body_store();
        fixture
            .execute(&mut store)
            .expect("publish authentic fixture");
        fixture.assert_complete();
        let artifact = fixture.kura.v2_finality_artifact(1).unwrap().unwrap();
        let files = post_checkpoint_tree(&fixture.kura);
        let checkpoint = fixture.kura.wsv_checkpoint(1).unwrap().unwrap();
        let manifest = fixture.kura.commit_manifest(1).unwrap().unwrap();
        let state_hash = crate::snapshot::canonical_state_snapshot_hash(&fixture.state).unwrap();
        for _ in 0..3 {
            fixture
                .service
                .persist_post_apply_metadata(&fixture.context, fixture.task.subject(), &artifact)
                .expect("exact original completion retry remains accepted");
            assert_eq!(post_checkpoint_tree(&fixture.kura), files);
            assert_eq!(fixture.kura.wsv_checkpoint(1).unwrap().unwrap(), checkpoint);
            assert_eq!(fixture.kura.commit_manifest(1).unwrap().unwrap(), manifest);
            assert_eq!(checkpoint.state_hash(), state_hash);
            assert!(
                fixture
                    .kura
                    .commit_manifest_has_wsv_binding(&manifest)
                    .unwrap()
            );
            assert_eq!(fixture.state.committed_height(), 1);
            assert_eq!(
                fixture.state.latest_block_hash_fast(),
                Some(fixture.body.hash())
            );
        }
    }
);
