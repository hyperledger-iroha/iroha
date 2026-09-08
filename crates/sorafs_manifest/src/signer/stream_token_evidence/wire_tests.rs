//! Actual-schema observer wire/resource tests and cross-purpose replay controls.

use super::*;

fn signed_bare_body(body: &[u8], observer: &KeyPair) -> Vec<u8> {
    let flags = norito::core::default_encode_flags();
    let body_frame = norito::core::frame_bare_with_header_flags::<
        SignerStreamTokenStateObservationBodyV1,
    >(body, flags)
    .expect("actual state-body schema frame");
    let mut message = b"iroha.sorafs.stream-token.finalized-state.v1\0".to_vec();
    message.extend(body_frame);
    let signature = Signature::try_new(observer.private_key(), &message)
        .expect("independently signed actual body frame");
    let mut outer = Vec::new();
    norito::core::write_len_with_flags(&mut outer, body.len() as u64, flags)
        .expect("body field length");
    outer.extend_from_slice(body);
    norito::core::write_len_with_flags(&mut outer, 64, flags).expect("signature field length");
    outer.extend_from_slice(signature.payload());
    norito::core::frame_bare_with_header_flags::<SignerStreamTokenStateObservationV1>(&outer, flags)
        .expect("actual signed envelope schema")
}

#[test]
fn maximum_actual_observation_leaves_fit_finite_request_and_evidence_budgets() {
    for phase in [Phase::Startup, Phase::BeforeRelease] {
        let receipt = receipt_fixture::fixture_with(0x21, |body, binding, authority| {
            binding.chain_id = "a".repeat(128).parse().expect("maximum chain");
            binding.runtime_handle = format!("hsm:{}", "r".repeat(124));
            binding.key_handle = format!("kms:{}", "k".repeat(124));
            binding.service_id = "s".repeat(128);
            binding.administrator_id = "d".repeat(128);
            authority.service_id = "c".repeat(128);
            authority.administrator_id = "e".repeat(128);
            body.profile_handle = "b".repeat(128);
            body.manifest_cid = vec![0xff; 128];
        });
        let mut evidence = Evidence::with_receipt(phase, receipt);
        evidence.trust.authority.service_id = "o".repeat(128);
        evidence.trust.authority.administrator_id = "p".repeat(128);
        evidence.state.body.authority = evidence.trust.authority.clone();
        evidence.resign();
        let state = evidence
            .state
            .encode_canonical()
            .expect("maximum admitted variable leaves");
        let request = evidence
            .request
            .encode_canonical()
            .expect("bounded fixed-field request");
        assert_eq!(
            state.len(),
            norito::canonical_frame_len(&evidence.state).expect("complete evidence count")
        );
        assert_eq!(
            request.len(),
            norito::canonical_frame_len(&evidence.request).expect("complete request count")
        );
        assert!(state.len() <= 64 * 1024);
        assert!(request.len() <= 8 * 1024);
        for flags in receipt_fixture::layouts() {
            let _guard = norito::core::DecodeFlagsGuard::enter(flags);
            assert_eq!(
                evidence
                    .state
                    .encode_canonical()
                    .expect("maximum state layout pin"),
                state
            );
            assert_eq!(
                evidence
                    .request
                    .encode_canonical()
                    .expect("maximum request layout pin"),
                request
            );
            assert_eq!(
                evidence
                    .state
                    .body
                    .signing_payload()
                    .expect("maximum state preimage"),
                observer_payload(&evidence.state.body)
            );
            assert_eq!(
                SignerStreamTokenStateObservationV1::decode_canonical(&state)
                    .expect("maximum state under all ambient layouts"),
                evidence.state
            );
        }
        let allocation = (64 * 1024 + 8 * state.len()).min(512 * 1024);
        let (decoded, usage) = norito::core::with_decode_limits_measured(
            norito::DecodeLimits::new(4096, state.len(), 8192, allocation, 24),
            || SignerStreamTokenStateObservationV1::decode_canonical(&state),
        );
        let decoded = decoded.expect("maximum valid body within actual-frame resource budget");
        for leaf in [
            &decoded.body.chain_id,
            &decoded.body.authority.service_id,
            &decoded.body.authority.administrator_id,
        ] {
            assert_eq!(leaf.len(), 128);
        }
        assert!(usage.total_allocated_bytes() > 0);
        assert!(usage.total_allocated_bytes() <= allocation);
        assert!(usage.total_elements() <= 8192);
        assert_positive(&mut evidence);
        let (_, shallow_usage) = norito::core::with_decode_limits_measured(
            norito::DecodeLimits::new(4096, state.len(), 8192, allocation, 1),
            || {
                let decoded =
                    norito::decode_canonical_with_limits::<SignerStreamTokenStateObservationV1>(
                        &state,
                        norito::DecodeLimits::new(4096, state.len(), 8192, allocation, 24),
                    );
                assert!(
                    matches!(decoded, Err(norito::Error::NestingDepthExceeded { .. })),
                    "nested current schema must respect stricter outer depth"
                );
                assert_eq!(
                    SignerStreamTokenStateObservationV1::decode_canonical(&state),
                    Err(EvidenceError::InvalidDocument)
                );
            },
        );
        assert!(shallow_usage.total_allocated_bytes() <= allocation);
    }
    for field in 0..3 {
        let mut evidence = checked_fixture(Phase::Startup);
        match field {
            0 => evidence.state.body.chain_id = "x".repeat(129),
            1 => evidence.state.body.authority.service_id = "x".repeat(129),
            _ => evidence.state.body.authority.administrator_id = "x".repeat(129),
        }
        assert_eq!(
            evidence.state.body.signing_payload(),
            Err(EvidenceError::InvalidState)
        );
        assert_eq!(
            evidence.state.encode_canonical(),
            Err(EvidenceError::InvalidState)
        );
    }
    let evidence = checked_fixture(Phase::BeforeRelease);
    for invalid in [Vec::new(), vec![0; 8 * 1024 + 1]] {
        let (decoded, usage) = norito::core::with_decode_limits_measured(
            norito::DecodeLimits::new(4096, invalid.len(), 8192, 512 * 1024, 24),
            || SignerStreamTokenObservationRequestV1::decode_canonical(&invalid),
        );
        assert_eq!(decoded, Err(EvidenceError::InvalidDocument));
        assert_eq!(usage.total_allocated_bytes(), 0);
    }
    let oversized = vec![0; 64 * 1024 + 1];
    let (decoded, usage) = norito::core::with_decode_limits_measured(
        norito::DecodeLimits::new(4096, oversized.len(), 8192, 512 * 1024, 24),
        || SignerStreamTokenStateObservationV1::decode_canonical(&oversized),
    );
    assert_eq!(decoded, Err(EvidenceError::InvalidDocument));
    assert_eq!(usage.total_allocated_bytes(), 0);
    assert!(evidence.request.encode_canonical().is_ok());
}

#[test]
fn actual_schema_omissions_alternate_layouts_and_compression_fail_before_authority() {
    for phase in [Phase::Startup, Phase::BeforeRelease] {
        let baseline = checked_fixture(phase);
        let state = baseline.observation_bytes();
        assert_eq!(
            signed_bare_body(&baseline.state.body.encode(), &baseline.observer),
            state,
            "manual current schema matches the actual positive envelope"
        );
        for omitted in 0..6 {
            let invalid = omit_field(&baseline.request, 6, omitted);
            assert_eq!(
                norito::core::Header::read(invalid.as_slice())
                    .expect("current request schema")
                    .schema,
                norito::core::Header::read(
                    baseline
                        .request
                        .encode_canonical()
                        .expect("request frame")
                        .as_slice()
                )
                .expect("positive request schema")
                .schema
            );
            assert_eq!(
                SignerStreamTokenObservationRequestV1::decode_canonical(&invalid),
                Err(EvidenceError::InvalidDocument)
            );
        }
        let bare = baseline.state.body.encode();
        for omitted in 0..13 {
            let mut changed = Vec::new();
            for (index, range) in fields(&bare, 13).into_iter().enumerate() {
                if index != omitted {
                    changed.extend_from_slice(&bare[range]);
                }
            }
            let invalid = signed_bare_body(&changed, &baseline.observer);
            let mut evidence = checked_fixture(phase);
            assert_eq!(
                evidence.verify_bytes(&invalid).err(),
                Some(EvidenceError::InvalidDocument)
            );
        }
        for omitted in 0..2 {
            let mut evidence = checked_fixture(phase);
            let invalid = omit_field(&evidence.state, 2, omitted);
            assert_eq!(
                evidence.verify_bytes(&invalid).err(),
                Some(EvidenceError::InvalidDocument)
            );
        }
        let canonical_request = baseline
            .request
            .encode_canonical()
            .expect("canonical query");
        let canonical_body =
            norito::encode_canonical(&baseline.state.body).expect("canonical body");
        let mut saw_alternate = [false; 3];
        for flags in receipt_fixture::layouts() {
            let (alternate_request, alternate_state, alternate_body) = {
                let _guard = norito::core::DecodeFlagsGuard::enter(flags);
                (
                    norito::core::to_bytes(&baseline.request).expect("alternate query"),
                    norito::core::to_bytes(&baseline.state).expect("alternate observation"),
                    norito::core::to_bytes(&baseline.state.body).expect("alternate body"),
                )
            };
            assert_eq!(
                norito::decode_from_bytes::<SignerStreamTokenObservationRequestV1>(
                    &alternate_request
                )
                .expect("ordinary request decode positive"),
                baseline.request
            );
            assert_eq!(
                norito::decode_from_bytes::<SignerStreamTokenStateObservationV1>(&alternate_state)
                    .expect("ordinary observation decode positive"),
                baseline.state
            );
            assert_eq!(
                norito::decode_from_bytes::<SignerStreamTokenStateObservationBodyV1>(
                    &alternate_body
                )
                .expect("ordinary body decode positive"),
                baseline.state.body
            );
            // Norito strips unused dynamic flags; an ambient guard alone does not prove an
            // alternate frame exists for these fixed-field subjects and string-only bodies.
            if alternate_request != canonical_request {
                saw_alternate[0] = true;
                assert_eq!(
                    SignerStreamTokenObservationRequestV1::decode_canonical(&alternate_request),
                    Err(EvidenceError::InvalidDocument)
                );
            }
            if alternate_state != state {
                saw_alternate[1] = true;
                let mut evidence = checked_fixture(phase);
                assert_eq!(
                    evidence.verify_bytes(&alternate_state).err(),
                    Some(EvidenceError::InvalidDocument)
                );
            }
            if alternate_body != canonical_body {
                saw_alternate[2] = true;
                let mut resigned = checked_fixture(phase);
                let mut wrong_message = b"iroha.sorafs.stream-token.finalized-state.v1\0".to_vec();
                wrong_message.extend(alternate_body);
                resigned.state.signature =
                    Signature::try_new(resigned.observer.private_key(), &wrong_message)
                        .expect("signed alternate body")
                        .payload()
                        .try_into()
                        .expect("Ed25519 width");
                assert_error(&mut resigned, EvidenceError::InvalidState);
            }
        }
        assert_eq!(
            saw_alternate, [true; 3],
            "each real schema has an ordinary-decodable alternate control"
        );
        let compressed = crate::canonical_test_support::with_compression_tag(&baseline.state);
        let mut huge_header = state.clone();
        huge_header[23..31].copy_from_slice(&u64::MAX.to_le_bytes());
        for invalid in [
            &compressed[..],
            &compressed[..norito::core::Header::SIZE],
            &huge_header[..],
        ] {
            let (decoded, usage) = norito::core::with_decode_limits_measured(
                norito::DecodeLimits::new(4096, invalid.len(), 8192, 512 * 1024, 24),
                || SignerStreamTokenStateObservationV1::decode_canonical(invalid),
            );
            assert_eq!(decoded, Err(EvidenceError::InvalidDocument));
            assert_eq!(
                usage.total_allocated_bytes(),
                0,
                "forbidden header rejected before decoder allocation"
            );
        }
        let mut trailing = state.clone();
        trailing.push(0);
        let mut bad_checksum = state.clone();
        *bad_checksum.last_mut().expect("payload byte") ^= 1;
        for invalid in [
            Vec::new(),
            state[..state.len() - 1].to_vec(),
            trailing,
            bad_checksum,
            baseline.state.signature.to_vec(),
        ] {
            let mut evidence = checked_fixture(phase);
            assert_eq!(
                evidence.verify_bytes(&invalid).err(),
                Some(EvidenceError::InvalidDocument)
            );
        }
    }
}

#[test]
fn genuine_release_and_stream_observations_cannot_cross_their_purpose_owners() {
    use crate::signer::release_evidence as release;
    let original = crate::signer::receipt::tests::fixture();
    let observer = receipt_fixture::key(0x73);
    let policy = release::SignerReleaseEvidencePolicyV1 {
        magic: release::SignerReleaseEvidencePolicyV1::magic(),
        binding: original.binding.clone(),
        operation_id: original.expected.operation_id,
        manifest_sha256: iroha_crypto::sha256(&original.manifest),
        manifest_size: original.expected.manifest_size,
        minimum_anchor: original.current.current_anchor,
    };
    let trust = release::SignerReleaseEvidenceTrustV1 {
        magic: release::SignerReleaseEvidenceTrustV1::magic(),
        custody_authority: original.trust.authority.clone(),
        custody_public_key: original.trust.public_key.clone(),
        custody_active_from_unix_ms: original.trust.active_from_unix_ms,
        custody_active_until_unix_ms: original.trust.active_until_unix_ms,
        custody_max_validity_ms: original.trust.max_validity_ms,
        state_authority: SignerCustodyAuthorityV1 {
            service_id: "finalized-state-observer".into(),
            administrator_id: "independent-state-reviewer".into(),
            key_revision: 2,
            policy_revision: 4,
            policy_digest: [0x74; 32],
        },
        state_public_key: observer.public_key().clone(),
        state_active_from_unix_ms: 90_000,
        state_active_until_unix_ms: 300_000,
        max_state_age_ms: 10_000,
    };
    let policy_bytes = norito::encode_canonical(&policy).expect("genuine release policy");
    let trust_bytes = norito::encode_canonical(&trust).expect("genuine release trust");
    let (_, role_public_key) = original
        .signer
        .public_key()
        .try_to_bytes()
        .expect("release role key");
    let expected = release::SignerReleaseEvidenceExpectedV1 {
        policy_sha256: iroha_crypto::sha256(&policy_bytes),
        trust_sha256: iroha_crypto::sha256(&trust_bytes),
        public_key_fingerprint_sha256: iroha_crypto::sha256(role_public_key),
        now_unix_ms: original.current.now_unix_ms,
    };
    let body = release::SignerReleaseStateObservationBodyV1 {
        magic: release::SignerReleaseStateObservationBodyV1::magic(),
        reviewed_policy_sha256: expected.policy_sha256,
        authority: trust.state_authority.clone(),
        chain_id: original.binding.chain_id.clone(),
        network_id: original.binding.network_id,
        deployment_id: "production-primary".into(),
        observed_at_unix_ms: original.current.anchor_observed_at_unix_ms,
        expires_at_unix_ms: original.current.now_unix_ms + 5_000,
        current_anchor: original.current.current_anchor,
        active_head: original.current.active_head,
        signer_revoked: false,
        attester_revoked: false,
        completed_operation: original.completion,
    };
    let mut release_preimage = b"iroha.sorafs.release-manifest.finalized-state.v1\0".to_vec();
    release_preimage
        .extend(norito::encode_canonical(&body).expect("independent release state oracle"));
    assert_eq!(
        body.signing_payload().expect("release owner preimage"),
        release_preimage
    );
    let signature = Signature::try_new(observer.private_key(), &release_preimage)
        .expect("real independent release observer signature")
        .payload()
        .try_into()
        .expect("Ed25519 width");
    let state = release::SignerReleaseStateObservationV1 { body, signature };
    let release_bytes = norito::encode_canonical(&state).expect("genuine release observation");
    let original_receipt =
        norito::encode_canonical(&original.receipt).expect("genuine release receipt");
    let public: [u8; 32] = role_public_key.try_into().expect("Ed25519 width");
    let verify_release = |observation: &[u8]| {
        release::verify_release_manifest_evidence_v1(
            &policy_bytes,
            &trust_bytes,
            observation,
            &original_receipt,
            &original.manifest,
            &original.receipt.signatures[0].signature,
            &public,
            &expected,
        )
    };
    verify_release(&release_bytes).expect("genuine release owner positive before replay");
    for phase in [Phase::Startup, Phase::BeforeRelease] {
        let mut stream = checked_fixture(phase);
        let stream_bytes = stream.observation_bytes();
        assert_eq!(
            stream.verify_bytes(&release_bytes).err(),
            Some(EvidenceError::InvalidDocument)
        );
        assert_eq!(
            verify_release(&stream_bytes).err(),
            Some(release::SignerReleaseEvidenceErrorV1::InvalidDocument)
        );
    }
    // Neither positive fixture is relabelled; exact canonical schemas and signing domains remain
    // at their purpose owners. An observer signature is not a replacement for release authority.
}
