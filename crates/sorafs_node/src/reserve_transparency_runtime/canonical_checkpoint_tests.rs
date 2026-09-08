// Canonical framed scanner checkpoints retain exact source and committed-chain bindings.

#[test]
fn scanner_checkpoint_frames_and_digests_survive_every_caller_layout() {
    let mut expected_frames = None;
    let mut checked_layouts = 0;
    let mut alternate_frames = 0;
    for flags in 0..=u8::MAX {
        if norito::core::validate_header_flags(flags).is_err() {
            continue;
        }
        checked_layouts += 1;
        let _caller = norito::core::DecodeFlagsGuard::enter(flags);
        let temp = tempfile::tempdir().expect("private scanner root");
        let config = scanner_config(temp.path().canonicalize().unwrap().join("scanner"));
        let (query, projection, sink) = test_dependencies();
        let mut first = scanner(
            &config,
            Arc::clone(&query),
            Arc::clone(&projection),
            Arc::clone(&sink),
        );
        let outcome = first.tick().expect("persist the first bounded source page");
        assert_eq!(outcome.events, 1);
        assert!(!outcome.caught_up);
        let path = first.checkpoint_path.clone();
        let first_checkpoint = first.checkpoint.clone().unwrap();
        assert_eq!(first_checkpoint.payload.generation, 1);
        assert_eq!(
            first_checkpoint.payload.after,
            Some(query.events[0].cursor())
        );
        let first_bytes = std::fs::read(&path).unwrap();
        assert_eq!(
            first_bytes,
            norito::encode_canonical(&first_checkpoint).unwrap()
        );
        // The independent oracle retains the existing domain and full-frame length prefix.
        let payload_bytes = norito::encode_canonical(&first_checkpoint.payload).unwrap();
        let mut hasher = blake3::Hasher::new();
        hasher.update(CHECKPOINT_DIGEST_DOMAIN_V1);
        hasher.update(&u64::try_from(payload_bytes.len()).unwrap().to_le_bytes());
        hasher.update(&payload_bytes);
        assert_eq!(first_checkpoint.digest, *hasher.finalize().as_bytes());
        drop(first);

        let mut resumed = {
            let _reader =
                norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
            let resumed = scanner(
                &config,
                Arc::clone(&query),
                Arc::clone(&projection),
                Arc::clone(&sink),
            );
            assert_eq!(resumed.checkpoint.as_ref(), Some(&first_checkpoint));
            resumed
        };
        let outcome = resumed.tick().expect("resume the second exact event");
        assert_eq!(outcome.events, 1);
        assert!(outcome.caught_up);
        let retained = resumed.checkpoint.clone().unwrap();
        assert_eq!(retained.payload.generation, 2);
        assert_eq!(retained.payload.after, Some(query.events[1].cursor()));
        let retained_bytes = std::fs::read(&path).unwrap();
        assert_eq!(retained_bytes, norito::encode_canonical(&retained).unwrap());
        let frames = (first_bytes, retained_bytes.clone());
        if let Some(expected) = &expected_frames {
            assert_eq!(&frames, expected, "caller layout {flags:#04x}");
        } else {
            expected_frames = Some(frames);
        }
        assert_eq!(resumed.tick().unwrap().events, 0);
        assert_eq!(std::fs::read(&path).unwrap(), retained_bytes);
        assert_eq!(sink.attempts.load(Ordering::Relaxed), 2);
        assert_eq!(sink.entries.lock().unwrap().len(), 2);
        drop(resumed);

        let open = || {
            ReserveTransparencyScannerV1::try_new(
                &config,
                test_network_id(),
                query_qualification(),
                query.clone(),
                projection.clone(),
                sink.clone(),
            )
        };
        let alternate = norito::to_bytes(&retained).unwrap();
        if alternate != retained_bytes {
            alternate_frames += 1;
            assert_eq!(
                norito::decode_from_bytes::<ReserveTransparencyCheckpointV1>(&alternate).unwrap(),
                retained,
            );
            write_local_checkpoint_atomic_bounded(&path, &alternate, config.checkpoint_max_bytes.0)
                .unwrap();
            assert_eq!(
                open().unwrap_err(),
                ReserveTransparencyScannerErrorV1::Checkpoint
            );
            assert_eq!(std::fs::read(&path).unwrap(), alternate);
        }
        let mut substituted = retained.clone();
        substituted.payload.query_policy_digest[0] ^= 1;
        substituted.digest = checkpoint_payload_digest(&substituted.payload).unwrap();
        let substituted_bytes = norito::encode_canonical(&substituted).unwrap();
        write_local_checkpoint_atomic_bounded(
            &path,
            &substituted_bytes,
            config.checkpoint_max_bytes.0,
        )
        .unwrap();
        assert_eq!(
            open().unwrap_err(),
            ReserveTransparencyScannerErrorV1::Checkpoint
        );
        assert_eq!(std::fs::read(&path).unwrap(), substituted_bytes);
        write_local_checkpoint_atomic_bounded(
            &path,
            &retained_bytes,
            config.checkpoint_max_bytes.0,
        )
        .unwrap();
        let mut restored = open().expect("restore the exact retained checkpoint");
        projection.replace_hash(3, [0xF3; 32]);
        assert_eq!(
            restored.tick(),
            Err(ReserveTransparencyScannerErrorV1::ForkOrReorg)
        );
        assert_eq!(restored.checkpoint.as_ref(), Some(&retained));
        assert_eq!(std::fs::read(&path).unwrap(), retained_bytes);
        assert_eq!(sink.attempts.load(Ordering::Relaxed), 2);
        assert_eq!(sink.entries.lock().unwrap().len(), 2);
        assert_eq!(norito::core::get_decode_flags(), flags);
    }
    assert_eq!(checked_layouts, 10);
    assert!(alternate_frames > 0);
}
