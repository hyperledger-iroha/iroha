// Canonical bounded PoR wire admission is independent of the caller's codec layout.

fn assert_canonical_por_decoder<T>(
    value: &T,
    decode: impl Fn(&[u8]) -> Result<T, norito::core::Error>,
) where
    T: norito::NoritoSerialize
        + for<'de> norito::NoritoDeserialize<'de>
        + std::fmt::Debug
        + PartialEq,
{
    let canonical = norito::encode_canonical(value).unwrap();
    let alternate = encode_frame_with_flags(value, 0);
    assert_ne!(canonical, alternate);
    assert_eq!(
        &norito::decode_from_bytes::<T>(&alternate).unwrap(),
        value,
        "the alternate frame must encode the same value before canonical rejection"
    );
    let compressed = crate::canonical_test_support::with_compression_tag(value);
    for flags in supported_layouts() {
        let _context = norito::core::DecodeFlagsGuard::enter(flags);
        assert_eq!(&decode(&canonical).unwrap(), value);
        assert!(matches!(
            decode(&alternate),
            Err(norito::core::Error::NonCanonicalEncoding)
        ));
        assert!(matches!(
            decode(&compressed),
            Err(norito::core::Error::NonCanonicalEncoding)
        ));
        assert!(matches!(
            decode(&compressed[..norito::core::Header::SIZE]),
            Err(norito::core::Error::NonCanonicalEncoding)
        ));
    }
}

#[test]
fn por_decoders_require_exact_canonical_frames_under_every_caller_layout() {
    let challenge = challenge_fixture(vec![10, 42]);
    assert_canonical_por_decoder(&challenge, decode_por_challenge_v1);
    let publication = PorChallengePublicationV1::try_new(challenge, 0).unwrap();
    assert_canonical_por_decoder(&publication, decode_por_challenge_publication_v1);
    assert_canonical_por_decoder(&proof_fixture(), decode_por_proof_v1);
    assert_canonical_por_decoder(
        &provider_vrf_submission_fixture(),
        decode_provider_vrf_submission_v1,
    );
    let mut verdict = verdict_fixture();
    add_verdict_signature(&mut verdict, &SigningKey::from_bytes(&[0x5a; 32]));
    assert_canonical_por_decoder(&verdict, decode_audit_verdict_v1);
    assert_canonical_por_decoder(&canonical_weekly_report(), decode_por_weekly_report_v1);
    let status = PorChallengeStatusV1 {
        version: POR_CHALLENGE_STATUS_VERSION_V1,
        challenge_id: [1; 32],
        manifest_digest: [2; 32],
        provider_id: [3; 32],
        epoch_id: 10,
        drand_round: 42,
        status: PorChallengeOutcome::Verified,
        sample_count: 32,
        forced: false,
        issued_at: 1_700_000_000,
        responded_at: Some(1_700_000_050),
        proof_digest: Some([4; 32]),
        repair_task_id: None,
        failure_reason: None,
        verifier_latency_ms: Some(950),
    };
    assert_canonical_por_decoder(&status, decode_por_challenge_status_v1);
    assert_canonical_por_decoder(&vec![status], |bytes| {
        decode_por_challenge_status_page_v1(bytes, 1)
    });
}

#[test]
fn por_unsigned_size_preflights_use_canonical_lengths_under_every_caller_layout() {
    let challenge = challenge_fixture(vec![10, 42]);
    let publication = PorChallengePublicationV1::try_new(challenge.clone(), 0).unwrap();
    let report = canonical_weekly_report();
    let (challenge_len, publication_len, report_len) = {
        let _context = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
        (
            challenge.encoded_len_exact().unwrap(),
            publication.encoded_len_exact().unwrap(),
            report.encoded_len_exact().unwrap(),
        )
    };
    for flags in supported_layouts() {
        let _context = norito::core::DecodeFlagsGuard::enter(flags);
        assert_eq!(
            preflight_por_challenge_len(&challenge, challenge_len),
            Ok(challenge_len)
        );
        assert_eq!(
            preflight_por_challenge_len(&challenge, challenge_len - 1),
            Err(PorChallengeValidationError::PayloadTooLarge {
                found: challenge_len,
                maximum: challenge_len - 1,
            })
        );
        assert_eq!(
            preflight_por_challenge_publication_len(&publication, publication_len),
            Ok(publication_len)
        );
        assert_eq!(
            preflight_por_challenge_publication_len(&publication, publication_len - 1),
            Err(PorChallengePublicationValidationError::PayloadTooLarge {
                found: publication_len,
                maximum: publication_len - 1,
            })
        );
        assert_eq!(
            preflight_por_weekly_report_len(&report, report_len),
            Ok(report_len)
        );
        assert_eq!(
            preflight_por_weekly_report_len(&report, report_len - 1),
            Err(PorWeeklyReportValidationError::PayloadTooLarge {
                found: report_len,
                maximum: report_len - 1,
            })
        );
    }
}
