// Scrub observations are made only while each decrypted allocation is still live.

mod quarantine_plaintext_hygiene_tests {
    use super::*;
    use crate::moderation::quarantine_plaintext_test_observer::observe;

    #[test]
    fn plaintext_owner_zeroizes_live_bytes_and_explicit_transfer_keeps_output() {
        let payload = b"owned plaintext survives only an explicit output transfer".to_vec();
        let ((), observations) = observe(|| {
            let owner = ModerationQuarantinePlaintext(payload.clone());
            assert_eq!(owner.as_slice(), payload);
            let debug = format!("{owner:?}");
            assert!(debug.contains("payload: \"<redacted>\""));
            assert!(!debug.contains(&format!("{payload:?}")));
            drop(owner);
        });
        assert_eq!(observations.len(), 1);
        assert_eq!(observations[0].len, payload.len());
        assert!(observations[0].nonzero_before);
        assert!(observations[0].all_zero);
        let (transferred, observations) =
            observe(|| ModerationQuarantinePlaintext(payload.clone()).into_authorized_payload());
        assert_eq!(transferred, payload);
        assert_eq!(observations.len(), 1);
        assert_eq!(observations[0].len, 0);
        assert!(!observations[0].nonzero_before);
        assert!(observations[0].all_zero);
    }

    #[test]
    fn late_chunk_authentication_failure_scrubs_accumulated_plaintext() {
        let wrapper = test_key_wrapper(0x71, "software://sorafs/moderation/active");
        let binding = test_key_provider_binding();
        let chunk_len = MODERATION_QUARANTINE_OBJECT_CHUNK_BYTES_V1 as usize;
        let payload = vec![0x5B; chunk_len + 137];
        let (record, bytes) = seal_moderation_quarantine_object(
            ModerationQuarantineObjectInput {
                quarantine_id: [0x64; 16],
                payload: payload.clone(),
                captured_at_unix: 1_800_000_701,
                content_type: None,
                notes: None,
            },
            &binding,
            &wrapper,
        )
        .unwrap();
        let mut envelope =
            decode_moderation_quarantine_object_envelope(&bytes, 8 * 1024 * 1024).unwrap();
        assert_eq!(
            open_moderation_quarantine_object(&envelope, &record, &binding, &wrapper)
                .unwrap()
                .as_slice(),
            payload
        );
        assert_eq!(envelope.chunks.len(), 2);
        let tag_index = envelope.chunks[1].ciphertext.len() - 1;
        envelope.chunks[1].ciphertext[tag_index] ^= 1;
        envelope.ciphertext_digest = moderation_quarantine_ciphertext_digest(&envelope.chunks);
        let changed_record = moderation_quarantine_object_record_from_envelope(
            &envelope,
            record.envelope_path.clone(),
        )
        .unwrap();
        validate_quarantine_object_envelope(&envelope).unwrap();
        let (result, observations) = observe(|| {
            open_moderation_quarantine_object_range(
                &envelope,
                &changed_record,
                &binding,
                &wrapper,
                0..payload.len() as u64,
            )
        });
        assert!(matches!(
            result,
            Err(ModerationQuarantineObjectError::AuthenticationFailed { .. })
        ));
        // The first authenticated chunk and the accumulated output both contain
        // initialized plaintext; the second chunk's invalid tag yields no plaintext.
        assert_eq!(observations.len(), 2);
        for observation in observations {
            assert_eq!(observation.len, chunk_len);
            assert!(observation.nonzero_before);
            assert!(observation.all_zero);
        }
    }
}
