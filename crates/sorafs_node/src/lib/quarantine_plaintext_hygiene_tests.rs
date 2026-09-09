// Actual quarantine callers retain the plaintext scrub owner until explicit output.

mod quarantine_plaintext_hygiene_tests {
    use super::*;
    use crate::moderation::quarantine_plaintext_test_observer::{DropObservation, observe};

    const CAPTURED: u64 = 1_800_000_701;
    const OLD_KEY: &str = "software://sorafs/moderation/quarantine-hygiene-old";
    const NEW_KEY: &str = "software://sorafs/moderation/quarantine-hygiene-new";

    fn payload() -> Vec<u8> {
        // One chunk: each full open has one chunk allocation plus its aggregate.
        b"private quarantine plaintext: [17, 83, 29], never diagnostics".to_vec()
    }

    fn assert_scrubbed(observations: &[DropObservation], expected_lengths: &[usize]) {
        assert!(observations.iter().all(|event| event.all_zero));
        let live = observations
            .iter()
            .filter(|event| event.len != 0)
            .collect::<Vec<_>>();
        assert!(live.iter().all(|event| event.nonzero_before));
        assert_eq!(
            live.iter().map(|event| event.len).collect::<Vec<_>>(),
            expected_lengths
        );
    }

    fn stored_bytes(
        cfg: &StorageConfig,
        record: &ModerationQuarantineObjectRecord,
    ) -> (Vec<u8>, Vec<u8>) {
        (
            fs::read(
                moderation_quarantine_object_store_root(cfg.data_dir()).join(&record.envelope_path),
            )
            .unwrap(),
            fs::read(moderation_quarantine_object_index_path(cfg.data_dir())).unwrap(),
        )
    }

    #[test]
    fn duplicate_store_scrubs_successful_open_on_match_and_conflict() {
        let (cfg, _dir) = storage_config_with_temp_dir_and_quarantine_key_provider();
        let source = node_with_test_quarantine_key_wrapper(cfg.clone());
        let payload = payload();
        let (id, record) = store_moderation_quarantine_fixture(
            &source,
            "cid:bafy-hygiene-replay",
            &payload,
            CAPTURED,
            None,
        );
        let before = stored_bytes(&cfg, &record);
        for captured_at_unix in [CAPTURED, CAPTURED + 1] {
            let (result, observations) = observe(|| {
                source.store_moderation_quarantine_object(ModerationQuarantineObjectInput {
                    quarantine_id: id,
                    payload: payload.clone(),
                    captured_at_unix,
                    content_type: None,
                    notes: None,
                })
            });
            if captured_at_unix == CAPTURED {
                assert_eq!(result.unwrap(), record);
            } else {
                assert!(matches!(
                    result,
                    Err(ModerationQuarantineObjectError::ConflictingObject { .. })
                ));
            }
            // The input's separate scrub owner is not observed; both events
            // therefore prove actual decrypted chunk AND returned aggregate disposal.
            assert_scrubbed(&observations, &[payload.len(), payload.len()]);
            assert_eq!(stored_bytes(&cfg, &record), before);
        }
    }

    #[test]
    fn startup_audit_scrubs_successful_plaintext_before_returning_node() {
        let (cfg, _dir) = storage_config_with_temp_dir_and_quarantine_key_provider();
        let source = node_with_test_quarantine_key_wrapper(cfg.clone());
        let payload = payload();
        let (id, record) = store_moderation_quarantine_fixture(
            &source,
            "cid:bafy-hygiene-startup",
            &payload,
            CAPTURED,
            None,
        );
        let before = stored_bytes(&cfg, &record);
        drop(source);
        let (restored, observations) = observe(|| {
            NodeHandle::try_new_with_quarantine_key_wrapper(
                cfg.clone(),
                test_quarantine_key_wrapper(),
            )
        });
        let restored = restored.expect("actual startup authenticates its indexed object");
        assert_scrubbed(&observations, &[payload.len(), payload.len()]);
        assert_eq!(stored_bytes(&cfg, &record), before);
        let (result, observations) = observe(|| restored.read_moderation_quarantine_object(id));
        assert_eq!(result.unwrap().payload, payload);
        // A successful explicit application handoff releases only the temporary
        // chunk; its output remains caller-owned and is not falsely claimed wiped.
        assert_scrubbed(&observations, &[payload.len()]);
    }

    #[test]
    fn full_read_and_startup_audit_scrub_before_post_open_subject_failure() {
        let (cfg, _dir) = storage_config_with_temp_dir_and_quarantine_key_provider();
        let source = node_with_test_quarantine_key_wrapper(cfg.clone());
        let expected = payload();
        let (id, _) = store_moderation_quarantine_fixture(
            &source,
            "cid:bafy-hygiene-subject",
            &expected,
            CAPTURED,
            None,
        );
        assert_eq!(
            source
                .read_moderation_quarantine_object(id)
                .unwrap()
                .payload,
            expected
        );

        // Independently seal a real alternate object for the same quarantine ID.
        // Only the lower-level local object index is substituted; the real queue
        // stays unchanged. Full startup rejects this earlier during cross-index
        // validation, so call its actual audit owner directly to cover its later
        // post-open digest failure without introducing a production bypass.
        let different = b"different authenticated content owned by another subject".to_vec();
        let wrapper = test_quarantine_key_wrapper();
        let (record, bytes) = seal_moderation_quarantine_object(
            ModerationQuarantineObjectInput {
                quarantine_id: id,
                payload: different.clone(),
                captured_at_unix: CAPTURED,
                content_type: None,
                notes: None,
            },
            source
                .moderation_quarantine_key_provider_binding
                .as_ref()
                .unwrap(),
            wrapper.as_ref(),
        )
        .unwrap();
        let envelope = decode_moderation_quarantine_object_envelope(
            &bytes,
            cfg.runtime_retention().checkpoint_max_bytes(),
        )
        .unwrap();
        assert_eq!(
            open_moderation_quarantine_object(
                &envelope,
                &record,
                source
                    .moderation_quarantine_key_provider_binding
                    .as_ref()
                    .unwrap(),
                wrapper.as_ref()
            )
            .unwrap()
            .as_slice(),
            different
        );
        let path =
            moderation_quarantine_object_store_root(cfg.data_dir()).join(&record.envelope_path);
        write_local_checkpoint_atomic_bounded(
            &path,
            &bytes,
            cfg.runtime_retention().checkpoint_max_bytes(),
        )
        .unwrap();
        source
            .moderation_quarantine_objects
            .write()
            .unwrap()
            .restore_snapshot(ModerationQuarantineObjectSnapshot {
                objects: vec![record.clone()],
            })
            .unwrap();
        let before = stored_bytes(&cfg, &record);

        let (result, observations) = observe(|| source.read_moderation_quarantine_object(id));
        assert!(matches!(
            result,
            Err(ModerationQuarantineObjectError::AuthenticationFailed { .. })
        ));
        assert_scrubbed(&observations, &[different.len(), different.len()]);
        let (result, observations) = observe(|| source.audit_moderation_quarantine_object_store());
        let error = result
            .expect_err("actual startup audit rejects mismatched queue digest after decryption");
        assert!(matches!(error, NodeInitError::Checkpoint {
            component: "moderation quarantine object envelope", message, ..
        } if message == "decrypted payload digest does not match quarantine subject"));
        assert_scrubbed(&observations, &[different.len(), different.len()]);
        assert_eq!(stored_bytes(&cfg, &record), before);
    }

    #[derive(Debug)]
    struct ReplacementWrapper {
        inner: TestQuarantineKeyWrapper,
        replace_dek: bool,
    }
    impl ModerationQuarantineKeyWrapper for ReplacementWrapper {
        fn provider_handle(&self) -> &str {
            self.inner.provider_handle()
        }
        fn qualification(
            &self,
        ) -> Result<
            ModerationQuarantineKeyProviderQualificationV1,
            ModerationQuarantineKeyProviderReadinessErrorV1,
        > {
            self.inner.qualification()
        }
        fn active_key_id(&self) -> &str {
            self.inner.active_key_id()
        }
        fn wrap_dek(
            &self,
            context: [u8; 32],
            dek: &[u8; 32],
        ) -> Result<Vec<u8>, ModerationQuarantineKeyOperationErrorV1> {
            if self.replace_dek {
                // A misbehaving injected adapter still performs real wrapping.
                // The original chunks cannot authenticate under this altered DEK.
                let mut changed = *dek;
                changed[0] ^= 1;
                let result = self.inner.wrap_dek(context, &changed);
                iroha_crypto::zeroize_value_for_confidential_discard(&mut changed);
                result
            } else {
                self.inner.wrap_dek(context, dek)
            }
        }
        fn unwrap_dek(
            &self,
            key_id: &str,
            context: [u8; 32],
            wrapped: &[u8],
        ) -> Result<[u8; 32], ModerationQuarantineKeyOperationErrorV1> {
            self.inner.unwrap_dek(key_id, context, wrapped)
        }
    }

    #[test]
    fn rewrap_scrubs_original_and_replacement_verification_before_persistence() {
        for replace_dek in [false, true] {
            let (cfg, _dir) = storage_config_with_temp_dir_and_quarantine_key_provider();
            let source = NodeHandle::try_new_with_quarantine_key_wrapper(
                cfg.clone(),
                Arc::new(TestQuarantineKeyWrapper::single(OLD_KEY, 0x31)),
            )
            .unwrap();
            let payload = payload();
            let (id, record) = store_moderation_quarantine_fixture(
                &source,
                "cid:bafy-hygiene-rewrap",
                &payload,
                CAPTURED,
                None,
            );
            let before = stored_bytes(&cfg, &record);
            drop(source);
            let rotated_cfg = enabled_storage_builder(cfg.data_dir().clone())
                .moderation_quarantine_key_provider(Some(test_quarantine_key_provider_config_for(
                    TEST_ROTATED_QUARANTINE_KEY_PROVIDER_QUALIFICATION,
                )))
                .build();
            let rotated = NodeHandle::try_new_with_quarantine_key_wrapper(
                rotated_cfg.clone(),
                Arc::new(ReplacementWrapper {
                    inner: TestQuarantineKeyWrapper::rotated(OLD_KEY, 0x31, NEW_KEY, 0x52),
                    replace_dek,
                }),
            )
            .unwrap();
            let (result, observations) =
                observe(|| rotated.rewrap_moderation_quarantine_object_dek(id));
            if replace_dek {
                assert!(matches!(
                    result,
                    Err(ModerationQuarantineObjectError::AuthenticationFailed { .. })
                ));
                assert_scrubbed(&observations, &[payload.len()]);
                assert_eq!(stored_bytes(&cfg, &record), before);
                assert_eq!(
                    rotated
                        .read_moderation_quarantine_object(id)
                        .unwrap()
                        .payload,
                    payload
                );
            } else {
                assert_eq!(result.unwrap(), record);
                // Original streaming authentication, replacement chunk, and
                // replacement aggregate. Omitting the caller guard loses event 3.
                assert_scrubbed(
                    &observations,
                    &[payload.len(), payload.len(), payload.len()],
                );
                let after = stored_bytes(&cfg, &record);
                assert_ne!(after.0, before.0);
                assert_eq!(after.1, before.1);
                let original = decode_moderation_quarantine_object_envelope(
                    &before.0,
                    cfg.runtime_retention().checkpoint_max_bytes(),
                )
                .unwrap();
                let replacement = decode_moderation_quarantine_object_envelope(
                    &after.0,
                    cfg.runtime_retention().checkpoint_max_bytes(),
                )
                .unwrap();
                assert_eq!(replacement.chunks, original.chunks);
                assert_eq!(replacement.object_id, original.object_id);
                assert_eq!(replacement.ciphertext_digest, original.ciphertext_digest);
                drop(rotated);
                let restored = NodeHandle::try_new_with_quarantine_key_wrapper(
                    rotated_cfg,
                    Arc::new(TestQuarantineKeyWrapper::single_with_qualification(
                        NEW_KEY,
                        0x52,
                        TEST_ROTATED_QUARANTINE_KEY_PROVIDER_QUALIFICATION,
                    )),
                )
                .unwrap();
                assert_eq!(
                    restored
                        .read_moderation_quarantine_object(id)
                        .unwrap()
                        .payload,
                    payload
                );
            }
        }
    }

    #[test]
    fn successful_full_and_range_output_debug_redacts_decimal_plaintext() {
        let (cfg, _dir) = storage_config_with_temp_dir_and_quarantine_key_provider();
        let source = node_with_test_quarantine_key_wrapper(cfg);
        let payload = payload();
        let (id, record) = store_moderation_quarantine_fixture(
            &source,
            "cid:bafy-hygiene-debug",
            &payload,
            CAPTURED,
            None,
        );
        let full = source.read_moderation_quarantine_object(id).unwrap();
        let (range, observations) =
            observe(|| source.read_moderation_quarantine_object_range(id, 3, 27));
        let range = range.unwrap();
        assert_eq!(full.record, record);
        assert_eq!(full.payload, payload);
        assert_eq!(range.record, record);
        assert_eq!((range.start, range.end), (3, 27));
        assert_eq!(range.payload, payload[3..27]);
        assert_scrubbed(&observations, &[payload.len()]);
        for (debug, bytes) in [
            (format!("{full:?}"), full.payload.as_slice()),
            (format!("{range:?}"), range.payload.as_slice()),
        ] {
            assert!(debug.contains("payload: \"<redacted>\""));
            assert!(debug.contains(&format!("payload_len: {}", bytes.len())));
            assert!(
                !debug.contains(&format!("{bytes:?}")),
                "decimal plaintext bytes must not appear in output Debug"
            );
        }
        assert!(format!("{range:?}").contains("start: 3, end: 27"));
    }
}
