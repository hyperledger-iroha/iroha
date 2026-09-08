// Populated runtime checkpoint writers and readers share the fixed V1 frame.

#[test]
fn populated_local_checkpoint_writers_and_recovery_ignore_caller_layout() {
    let (cfg, _dir) = storage_config_with_temp_dir_and_quarantine_key_provider();
    let source = node_with_test_quarantine_key_wrapper(cfg.clone());
    source
        .admit_moderation_repro_manifest(moderation_repro_manifest_fixture(0x12, 0x32))
        .expect("admit reproducibility manifest");
    source
        .admit_moderation_corpus_manifest(adversarial_corpus_manifest_fixture())
        .expect("admit adversarial corpus");
    seed_moderation_evidence_viewer_activity(
        &source,
        "cid:bafy-canonical-checkpoints",
        b"canonical checkpoint recovery payload",
        1_800_000_100_000,
        1_800_000_200_000,
        &[(
            ModerationEvidenceViewerAccessKind::Viewed,
            1_800_000_110_000,
        )],
    );
    source
        .record_privacy_aggregate_source_event(privacy_source_event(
            "canonical-restart-event",
            "canonical-restart-population",
            0x42,
            1_800_000_101,
        ))
        .expect("record auxiliary state");
    let model = source.export_moderation_model_registry_snapshot().unwrap();
    let screening = source.export_moderation_screening_snapshot().unwrap();
    let objects = source
        .export_moderation_quarantine_object_snapshot()
        .unwrap();
    let viewer = source.export_moderation_evidence_viewer_snapshot().unwrap();
    let auxiliary = source.export_auxiliary_runtime_checkpoint().unwrap();
    assert!(!model.reproducibility_manifests.is_empty());
    assert!(!screening.screening_records.is_empty());
    assert!(!objects.objects.is_empty());
    assert!(!viewer.access_events.is_empty());
    assert!(!auxiliary.privacy_source_events.is_empty());
    let expected = [
        (
            moderation_model_registry_checkpoint_path(cfg.data_dir()),
            norito::encode_canonical(&model).unwrap(),
        ),
        (
            moderation_screening_checkpoint_path(cfg.data_dir()),
            norito::encode_canonical(&screening).unwrap(),
        ),
        (
            moderation_quarantine_object_index_path(cfg.data_dir()),
            norito::encode_canonical(&objects).unwrap(),
        ),
        (
            moderation_evidence_viewer_checkpoint_path(cfg.data_dir()),
            norito::encode_canonical(&viewer).unwrap(),
        ),
        (
            auxiliary_runtime_checkpoint_path(cfg.data_dir()),
            norito::encode_canonical(&auxiliary).unwrap(),
        ),
    ];
    let mut alternate_rejections = [0; 5];
    for flags in (0..=u8::MAX).filter(|flags| norito::core::validate_header_flags(*flags).is_ok()) {
        let _caller = norito::core::DecodeFlagsGuard::enter(flags);
        source
            .persist_moderation_model_registry_snapshot(&model)
            .unwrap();
        source
            .persist_moderation_screening_snapshot(&screening)
            .unwrap();
        source
            .persist_moderation_quarantine_object_index_snapshot(&objects)
            .unwrap();
        source
            .persist_moderation_evidence_viewer_snapshot(&viewer)
            .unwrap();
        source
            .persist_auxiliary_runtime_checkpoint_unlocked()
            .unwrap();
        for (path, canonical) in &expected {
            assert_eq!(
                &fs::read(path).unwrap(),
                canonical,
                "{} flags {flags}",
                path.display()
            );
        }
        source.load_moderation_model_registry_checkpoint().unwrap();
        source.load_moderation_screening_checkpoint().unwrap();
        source
            .load_moderation_quarantine_object_index_checkpoint()
            .unwrap();
        source.load_moderation_evidence_viewer_checkpoint().unwrap();
        source.load_auxiliary_runtime_checkpoint().unwrap();
        assert_eq!(
            source.export_moderation_model_registry_snapshot().unwrap(),
            model
        );
        assert_eq!(
            source.export_moderation_screening_snapshot().unwrap(),
            screening
        );
        assert_eq!(
            source
                .export_moderation_quarantine_object_snapshot()
                .unwrap(),
            objects
        );
        assert_eq!(
            source.export_moderation_evidence_viewer_snapshot().unwrap(),
            viewer
        );
        assert_eq!(
            norito::encode_canonical(&source.export_auxiliary_runtime_checkpoint().unwrap())
                .unwrap(),
            expected[4].1
        );
        macro_rules! reject_alternate {
            ($index:literal, $value:ident, $ty:ty, $component:literal, $load:ident, $export:ident) => {
                let alternate = norito::to_bytes(&$value).unwrap();
                let (path, canonical) = &expected[$index];
                if alternate != *canonical {
                    alternate_rejections[$index] += 1;
                    let decoded = norito::decode_from_bytes::<$ty>(&alternate).unwrap();
                    assert_eq!(&norito::encode_canonical(&decoded).unwrap(), canonical);
                    write_local_checkpoint_atomic_bounded(
                        path,
                        &alternate,
                        cfg.runtime_retention().checkpoint_max_bytes(),
                    )
                    .unwrap();
                    let error = source
                        .$load()
                        .expect_err("alternate frame must not install state");
                    assert!(matches!(
                        error,
                        NodeInitError::Checkpoint {
                            component: $component,
                            ..
                        }
                    ));
                    assert_eq!(
                        &norito::encode_canonical(&source.$export().unwrap()).unwrap(),
                        canonical
                    );
                    write_local_checkpoint_atomic_bounded(
                        path,
                        canonical,
                        cfg.runtime_retention().checkpoint_max_bytes(),
                    )
                    .unwrap();
                    source
                        .$load()
                        .expect("exact canonical recovery after rejected alternate");
                }
            };
        }
        reject_alternate!(
            0,
            model,
            ModerationModelRegistrySnapshot,
            "moderation model registry",
            load_moderation_model_registry_checkpoint,
            export_moderation_model_registry_snapshot
        );
        reject_alternate!(
            1,
            screening,
            ModerationScreeningSnapshot,
            "moderation screening",
            load_moderation_screening_checkpoint,
            export_moderation_screening_snapshot
        );
        reject_alternate!(
            2,
            objects,
            ModerationQuarantineObjectSnapshot,
            "moderation quarantine object index",
            load_moderation_quarantine_object_index_checkpoint,
            export_moderation_quarantine_object_snapshot
        );
        reject_alternate!(
            3,
            viewer,
            ModerationEvidenceViewerSnapshot,
            "moderation evidence viewer",
            load_moderation_evidence_viewer_checkpoint,
            export_moderation_evidence_viewer_snapshot
        );
        reject_alternate!(
            4,
            auxiliary,
            AuxiliaryRuntimeCheckpointV5,
            "auxiliary runtime",
            load_auxiliary_runtime_checkpoint,
            export_auxiliary_runtime_checkpoint
        );
        assert_eq!(norito::core::get_decode_flags(), flags);
    }
    assert!(alternate_rejections.iter().all(|count| *count > 0));
    drop(source);
    let restored = node_with_test_quarantine_key_wrapper(cfg);
    assert_eq!(
        restored
            .export_moderation_model_registry_snapshot()
            .unwrap(),
        model
    );
    assert_eq!(
        restored.export_moderation_screening_snapshot().unwrap(),
        screening
    );
    assert_eq!(
        restored
            .export_moderation_quarantine_object_snapshot()
            .unwrap(),
        objects
    );
    assert_eq!(
        restored
            .export_moderation_evidence_viewer_snapshot()
            .unwrap(),
        viewer
    );
    assert_eq!(restored.privacy_aggregate_source_event_count(), 1);
}
