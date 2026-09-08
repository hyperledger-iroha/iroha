// Real local object recovery pairs canonical frames with fixed AEAD/key context.

#[test]
fn quarantine_object_store_read_rewrap_and_restart_ignore_caller_layout() {
    let payload = (0..70_000)
        .map(|index| (index % 251) as u8)
        .collect::<Vec<_>>();
    for flags in (0..=u8::MAX).filter(|flags| norito::core::validate_header_flags(*flags).is_ok()) {
        let (cfg, _dir) = storage_config_with_temp_dir_and_quarantine_key_provider();
        let old_wrapper: Arc<dyn ModerationQuarantineKeyWrapper> = Arc::new(
            TestQuarantineKeyWrapper::single("software://sorafs/moderation/quarantine-old", 0x31),
        );
        let source =
            NodeHandle::try_new_with_quarantine_key_wrapper(cfg.clone(), old_wrapper).unwrap();
        let quarantine_id = record_moderation_quarantine_fixture(
            &source,
            "cid:bafy-object-canonical-context",
            &payload,
        );
        let _caller = norito::core::DecodeFlagsGuard::enter(flags);
        let record = source
            .store_moderation_quarantine_object(ModerationQuarantineObjectInput {
                quarantine_id,
                payload: payload.clone(),
                captured_at_unix: 1_800_000_505,
                content_type: None,
                notes: None,
            })
            .expect("seal and persist under the caller layout");
        let path =
            moderation_quarantine_object_store_root(cfg.data_dir()).join(&record.envelope_path);
        let before_bytes = fs::read(&path).unwrap();
        let before =
            decode_moderation_quarantine_object_envelope(&before_bytes, 8 * 1024 * 1024).unwrap();
        assert_eq!(norito::encode_canonical(&before).unwrap(), before_bytes);
        assert_eq!(before.chunks.len(), 2);
        drop(source);

        let rotated_cfg = enabled_storage_builder(cfg.data_dir().clone())
            .moderation_quarantine_key_provider(Some(test_quarantine_key_provider_config_for(
                TEST_ROTATED_QUARANTINE_KEY_PROVIDER_QUALIFICATION,
            )))
            .build();
        let rotated_wrapper: Arc<dyn ModerationQuarantineKeyWrapper> =
            Arc::new(TestQuarantineKeyWrapper::rotated(
                "software://sorafs/moderation/quarantine-old",
                0x31,
                "software://sorafs/moderation/quarantine-new",
                0x52,
            ));
        let rotated =
            NodeHandle::try_new_with_quarantine_key_wrapper(rotated_cfg.clone(), rotated_wrapper)
                .expect("restart and audit the original canonical envelope");
        assert_eq!(
            rotated
                .read_moderation_quarantine_object(quarantine_id)
                .unwrap()
                .payload,
            payload
        );
        let range = rotated
            .read_moderation_quarantine_object_range(quarantine_id, 65_000, 66_000)
            .unwrap();
        assert_eq!(range.record, record);
        assert_eq!((range.start, range.end), (65_000, 66_000));
        assert_eq!(range.payload, payload[65_000..66_000]);
        assert_eq!(
            rotated
                .rewrap_moderation_quarantine_object_dek(quarantine_id)
                .unwrap(),
            record
        );
        let after_bytes = fs::read(&path).unwrap();
        assert_ne!(after_bytes, before_bytes);
        let after =
            decode_moderation_quarantine_object_envelope(&after_bytes, 8 * 1024 * 1024).unwrap();
        assert_eq!(norito::encode_canonical(&after).unwrap(), after_bytes);
        assert_eq!(after.object_id, before.object_id);
        assert_eq!(after.ciphertext_digest, before.ciphertext_digest);
        assert_eq!(after.chunks, before.chunks);
        assert_eq!(
            after.wrapping_key_id,
            "software://sorafs/moderation/quarantine-new"
        );
        assert_ne!(after.wrapped_dek, before.wrapped_dek);
        assert_eq!(
            rotated
                .export_moderation_quarantine_object_snapshot()
                .unwrap()
                .objects,
            vec![record.clone()]
        );
        drop(rotated);

        let replacement_only: Arc<dyn ModerationQuarantineKeyWrapper> =
            Arc::new(TestQuarantineKeyWrapper::single_with_qualification(
                "software://sorafs/moderation/quarantine-new",
                0x52,
                TEST_ROTATED_QUARANTINE_KEY_PROVIDER_QUALIFICATION,
            ));
        let restored =
            NodeHandle::try_new_with_quarantine_key_wrapper(rotated_cfg, replacement_only)
                .expect("restart and authenticate with only the replacement key");
        let recovered = restored
            .read_moderation_quarantine_object(quarantine_id)
            .unwrap();
        assert_eq!(recovered.record, record);
        assert_eq!(recovered.payload, payload);
        assert_eq!(
            restored
                .read_moderation_quarantine_object_range(quarantine_id, 65_000, 66_000)
                .unwrap()
                .payload,
            payload[65_000..66_000]
        );
        assert_eq!(norito::core::get_decode_flags(), flags);
    }
}

#[derive(Debug)]
struct CountingCanonicalQuarantineWrapper {
    inner: TestQuarantineKeyWrapper,
    calls: AtomicUsize,
}

impl ModerationQuarantineKeyWrapper for CountingCanonicalQuarantineWrapper {
    fn provider_handle(&self) -> &str {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.inner.provider_handle()
    }
    fn qualification(
        &self,
    ) -> Result<
        ModerationQuarantineKeyProviderQualificationV1,
        ModerationQuarantineKeyProviderReadinessErrorV1,
    > {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.inner.qualification()
    }
    fn active_key_id(&self) -> &str {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.inner.active_key_id()
    }
    fn wrap_dek(
        &self,
        context_digest: [u8; 32],
        dek: &[u8; 32],
    ) -> Result<Vec<u8>, ModerationQuarantineKeyOperationErrorV1> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.inner.wrap_dek(context_digest, dek)
    }
    fn unwrap_dek(
        &self,
        key_id: &str,
        context_digest: [u8; 32],
        wrapped_dek: &[u8],
    ) -> Result<[u8; 32], ModerationQuarantineKeyOperationErrorV1> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.inner.unwrap_dek(key_id, context_digest, wrapped_dek)
    }
}

#[test]
fn quarantine_object_noncanonical_envelopes_fail_before_key_provider_calls() {
    let (cfg, _dir) = storage_config_with_temp_dir_and_quarantine_key_provider();
    let wrapper = Arc::new(CountingCanonicalQuarantineWrapper {
        inner: TestQuarantineKeyWrapper::single("software://sorafs/moderation/quarantine-v1", 0xA5),
        calls: AtomicUsize::new(0),
    });
    let source =
        NodeHandle::try_new_with_quarantine_key_wrapper(cfg.clone(), wrapper.clone()).unwrap();
    let payload = vec![0xC3; 70_000];
    let (quarantine_id, record) = store_moderation_quarantine_fixture(
        &source,
        "cid:bafy-object-reject-before-key",
        &payload,
        1_800_000_506,
        None,
    );
    let root = moderation_quarantine_object_store_root(cfg.data_dir());
    let path = root.join(&record.envelope_path);
    let original = fs::read(&path).unwrap();
    let envelope =
        decode_moderation_quarantine_object_envelope(&original, 8 * 1024 * 1024).unwrap();
    let snapshot = source
        .export_moderation_quarantine_object_snapshot()
        .unwrap();
    let index_path = moderation_quarantine_object_index_path(cfg.data_dir());
    let index_bytes = fs::read(&index_path).unwrap();
    wrapper.calls.store(0, Ordering::SeqCst);
    assert_eq!(
        source
            .read_moderation_quarantine_object(quarantine_id)
            .unwrap()
            .payload,
        payload
    );
    assert!(
        wrapper.calls.load(Ordering::SeqCst) > 0,
        "positive read exercises the actual key provider"
    );

    let alternate = (0..=u8::MAX)
        .filter(|flags| norito::core::validate_header_flags(*flags).is_ok())
        .find_map(|flags| {
            let _layout = norito::core::DecodeFlagsGuard::enter(flags);
            let bytes = norito::to_bytes(&envelope).unwrap();
            assert_eq!(
                norito::decode_from_bytes::<ModerationQuarantineObjectEnvelopeV1>(&bytes).unwrap(),
                envelope
            );
            (bytes != original).then_some(bytes)
        })
        .expect("construct a genuine same-value alternate frame");
    let header = norito::core::Header::read(original.as_slice()).unwrap();
    let offset = header.magic.len() + 2 + header.schema.len();
    let mut compressed = original.clone();
    compressed[offset] = norito::Compression::Zstd as u8;
    compressed[offset + 1..offset + 9].copy_from_slice(&u64::MAX.to_le_bytes());
    for flags in (0..=u8::MAX).filter(|flags| norito::core::validate_header_flags(*flags).is_ok()) {
        let _caller = norito::core::DecodeFlagsGuard::enter(flags);
        for rejected in [
            alternate.as_slice(),
            compressed.as_slice(),
            &compressed[..norito::core::Header::SIZE],
        ] {
            fs::write(&path, rejected).unwrap();
            wrapper.calls.store(0, Ordering::SeqCst);
            assert!(matches!(
                source.read_moderation_quarantine_object(quarantine_id),
                Err(ModerationQuarantineObjectError::AuthenticationFailed { .. })
            ));
            assert!(matches!(
                source.read_moderation_quarantine_object_range(quarantine_id, 65_000, 66_000),
                Err(ModerationQuarantineObjectError::AuthenticationFailed { .. })
            ));
            assert!(matches!(
                source.rewrap_moderation_quarantine_object_dek(quarantine_id),
                Err(ModerationQuarantineObjectError::AuthenticationFailed { .. })
            ));
            let audit = source
                .audit_moderation_quarantine_object_store()
                .expect_err("startup audit rejects the frame before key access");
            assert!(
                audit
                    .to_string()
                    .contains("envelope is not canonically encoded"),
                "{audit}"
            );
            assert!(
                !source
                    .recover_unindexed_moderation_quarantine_envelope(&root, &path)
                    .unwrap()
            );
            assert_eq!(wrapper.calls.load(Ordering::SeqCst), 0);
            assert_eq!(
                fs::read(&path).unwrap(),
                rejected,
                "rejected orphan input is never removed or rewritten"
            );
            assert_eq!(
                source
                    .export_moderation_quarantine_object_snapshot()
                    .unwrap(),
                snapshot
            );
            assert_eq!(fs::read(&index_path).unwrap(), index_bytes);
        }
        assert_eq!(norito::core::get_decode_flags(), flags);
    }
    fs::write(&path, &original).unwrap();
    wrapper.calls.store(0, Ordering::SeqCst);
    source
        .audit_moderation_quarantine_object_store()
        .expect("restored original envelope still authenticates");
    assert_eq!(
        source
            .read_moderation_quarantine_object(quarantine_id)
            .unwrap()
            .payload,
        payload
    );
    assert!(wrapper.calls.load(Ordering::SeqCst) > 0);
}
