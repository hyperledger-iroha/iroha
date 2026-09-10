// Prepared-window commitments use actual signed custody and receipts; no hardware qualification.

fn window_request_oracle(f: &Fixture) -> Vec<u8> {
    let flags = norito::core::default_encode_flags();
    let _guard = norito::core::DecodeFlagsGuard::enter(flags);
    // Assemble the seven declared field frames independently of the request's derived encoder.
    // The two time leaves come from the retained body, never from the request under test.
    // Derived fixed-byte-array fields are raw bytes inside their field frame. The standalone
    // array codec prefixes each element and is not the declared struct-field representation.
    let fields = [
        f.expected.operation_id().to_vec(),
        f.expected.binding_digest().to_vec(),
        f.receipt.request.original_custody.encode(),
        f.expected.signing_payload_digest().to_vec(),
        f.expected.signing_payload_size().encode(),
        f.token.body.issued_at.checked_mul(1_000).unwrap().encode(),
        f.token.body.ttl_epoch.checked_mul(1_000).unwrap().encode(),
    ];
    let mut bare = Vec::new();
    for field in fields {
        norito::core::write_len_to_vec_with_flags(&mut bare, field.len() as u64, flags);
        bare.extend(field);
    }
    norito::core::frame_bare_with_header_flags::<SignerStreamTokenRequestV1>(&bare, flags)
        .expect("exact current request schema")
}

fn assert_window_signatures_are_real(f: &Fixture) {
    let messages = [
        oracle_payload(&f.token.body),
        f.receipt.commitment.audit.signing_message().to_vec(),
        f.receipt.provenance.signing_message().unwrap().to_vec(),
        f.receipt.commitment.response_signing_message().to_vec(),
    ];
    assert_eq!(f.receipt.signatures.len(), messages.len());
    for (signature, message) in f.receipt.signatures.iter().zip(messages) {
        assert_eq!(
            signature.message_digest,
            oracle_digest(b"iroha.sorafs.signer.operation.message.v1", &[&message])
        );
        Signature::try_from_bytes(&signature.signature)
            .unwrap()
            .verify(&f.binding.public_key, &message)
            .expect("real valid signature even when its public window claim is unauthorized");
    }
}

#[test]
fn prepared_window_request_preimage_and_token_identity_match_independent_oracles_in_ten_layouts() {
    let f = fixture();
    assert_positive(&f);
    let payload = oracle_payload(&f.token.body);
    let token_bytes = norito::encode_canonical(&f.token).unwrap();
    let original_signature = f.token.signature.clone();
    let binding_digest = oracle_canonical(b"iroha.sorafs.signer.custody-binding.v1", &f.binding);
    let operation_id = oracle_digest(
        b"iroha.sorafs.signer.stream-token.operation.v1",
        &[&binding_digest, &payload],
    );
    let frame = window_request_oracle(&f);
    for flags in layouts() {
        let _guard = norito::core::DecodeFlagsGuard::enter(flags);
        let expected = SignerStreamTokenExpectedV1::new(&f.token.body, &f.binding).unwrap();
        assert_eq!(expected.issued_at_unix_ms(), 1_000_000);
        assert_eq!(expected.expires_at_unix_ms(), 1_200_000);
        assert_eq!(expected.operation_id(), operation_id);
        assert_eq!(expected.signing_payload_size(), payload.len() as u64);
        assert_eq!(f.token.body.signing_payload_bytes().unwrap(), payload);
        assert_eq!(norito::encode_canonical(&f.token).unwrap(), token_bytes);
        assert_eq!(f.token.signature, original_signature);
        let request =
            SignerStreamTokenRequestV1::new(&active(&f), &expected, &f.token.body).unwrap();
        assert_eq!(norito::encode_canonical(&request).unwrap(), frame);
        assert_eq!(
            request.digest().unwrap(),
            oracle_digest(b"iroha.sorafs.signer.stream-token.request.v1", &[&frame])
        );
        assert_eq!(f.receipt.intent.request_digest, request.digest().unwrap());
        assert_window_signatures_are_real(&f);
        assert_positive(&f);
    }
    // Both objects intentionally hide bearer context and raw timestamp claims in Debug.
    assert_eq!(
        format!("{:?}", f.expected),
        "SignerStreamTokenExpectedV1 { .. }"
    );
    assert_eq!(
        format!("{:?}", f.receipt.request),
        "SignerStreamTokenRequestV1 { .. }"
    );
}

#[test]
fn prepared_window_checks_both_millisecond_overflows_and_exact_chronology_boundaries() {
    let f = fixture();
    let seconds_limit = u64::MAX / 1_000;
    for (issued, expires, error) in [
        (0, 1, Error::TokenMismatch),
        (1_000, 1_000, Error::TokenMismatch),
        (1_001, 1_000, Error::TokenMismatch),
        (1_000, 4_601, Error::TokenMismatch),
        (seconds_limit - 1, seconds_limit + 1, Error::InvalidTime),
        (seconds_limit + 1, seconds_limit + 2, Error::InvalidTime),
    ] {
        let mut body = f.token.body.clone();
        body.issued_at = issued;
        body.ttl_epoch = expires;
        assert_eq!(
            SignerStreamTokenExpectedV1::new(&body, &f.binding).unwrap_err(),
            error
        );
    }
    for (issued, expires) in [(1, 3_601), (seconds_limit - 1, seconds_limit)] {
        let mut body = f.token.body.clone();
        body.issued_at = issued;
        body.ttl_epoch = expires;
        let expected = SignerStreamTokenExpectedV1::new(&body, &f.binding).unwrap();
        assert_eq!(
            expected.issued_at_unix_ms(),
            issued.checked_mul(1_000).unwrap()
        );
        assert_eq!(
            expected.expires_at_unix_ms(),
            expires.checked_mul(1_000).unwrap()
        );
        assert_eq!(
            expected.validate_time_at(expected.issued_at_unix_ms() - 1),
            Err(Error::InvalidTime)
        );
        assert_eq!(
            expected.validate_time_at(expected.issued_at_unix_ms()),
            Ok(())
        );
        assert_eq!(
            expected.validate_time_at(expected.expires_at_unix_ms() - 1),
            Ok(())
        );
        assert_eq!(
            expected.validate_time_at(expected.expires_at_unix_ms()),
            Err(Error::TokenExpired)
        );
        assert_eq!(
            expected.validate_time_at(u64::MAX),
            Err(Error::TokenExpired)
        );
    }
}

#[test]
fn resigned_issue_or_expiry_claims_cannot_replace_the_independently_prepared_body() {
    for field in 0..2 {
        for replacement in [0, 1_000_001, 1_199_999, u64::MAX] {
            let mut f = fixture();
            assert_positive(&f);
            let original_digest = f.receipt.request.digest().unwrap();
            if field == 0 {
                f.receipt.request.issued_at_unix_ms = replacement;
            } else {
                f.receipt.request.expires_at_unix_ms = replacement;
            }
            assert_ne!(f.receipt.request.digest().unwrap(), original_digest);
            // Even grant the adversary all follow-on signatures and a matching completion claim.
            rebuild_operation(&mut f);
            assert_window_signatures_are_real(&f);
            assert_eq!(
                f.completion.intent_digest,
                f.receipt.intent.digest().unwrap()
            );
            let bytes = f.receipt.encode_canonical().unwrap();
            assert_eq!(
                prevalidate_stream_token_receipt_v1(&bytes, &f.token, &f.expected, &f.binding)
                    .unwrap_err(),
                Error::TokenMismatch
            );
            assert_eq!(
                validate_stream_token_signatures_v1(&f.receipt, &f.token, &f.expected, &active(&f))
                    .unwrap_err(),
                Error::TokenMismatch
            );
            assert_eq!(verify(&f).unwrap_err(), Error::TokenMismatch);
        }
    }
}

#[test]
fn authentic_window_chronology_cannot_be_extended_or_backdated_by_resigned_request_claims() {
    let mut expired = fixture();
    assert_positive(&expired);
    expired.current.now_unix_ms = expired.expected.expires_at_unix_ms();
    expired.current.anchor_observed_at_unix_ms = expired.current.now_unix_ms;
    assert_eq!(verify(&expired).unwrap_err(), Error::TokenExpired);
    expired.receipt.request.expires_at_unix_ms += 1_000;
    rebuild_operation(&mut expired);
    assert_window_signatures_are_real(&expired);
    assert_eq!(verify(&expired).unwrap_err(), Error::TokenMismatch);

    let mut future = fixture();
    assert_positive(&future);
    // A real newly signed body starts one second after its purported immutable completion.
    future.token.body.issued_at = 1_126;
    future.expected =
        SignerStreamTokenExpectedV1::new(&future.token.body, &future.binding).unwrap();
    let custody = active(&future);
    future.receipt.request =
        SignerStreamTokenRequestV1::new(&custody, &future.expected, &future.token.body).unwrap();
    future.receipt.intent.operation_id = future.expected.operation_id();
    let role = sign(
        &future.signer,
        SignerKeyOperationPurposeV1::RolePayload,
        &oracle_payload(&future.token.body),
    );
    future.token.signature = role.signature.clone();
    future.receipt.signatures[0] = role;
    rebuild_operation(&mut future);
    assert_window_signatures_are_real(&future);
    validate_stream_token_signatures_v1(&future.receipt, &future.token, &future.expected, &custody)
        .unwrap();
    assert_eq!(verify(&future).unwrap_err(), Error::InvalidTime);
    future.receipt.request.issued_at_unix_ms = future.completion.completed_at_unix_ms;
    rebuild_operation(&mut future);
    assert_window_signatures_are_real(&future);
    assert_eq!(verify(&future).unwrap_err(), Error::TokenMismatch);
}

#[test]
fn current_request_schema_rejects_each_missing_window_field_without_a_legacy_decoder() {
    let f = fixture();
    let flags = norito::core::default_encode_flags();
    let _guard = norito::core::DecodeFlagsGuard::enter(flags);
    let payload = f.receipt.request.encode();
    let mut fields = Vec::new();
    let mut offset = 0;
    for _ in 0..7 {
        let start = offset;
        let (len, prefix) =
            norito::core::read_len_from_slice_with_flags(&payload[offset..], flags).unwrap();
        offset += prefix + len;
        fields.push(&payload[start..offset]);
    }
    assert_eq!(offset, payload.len());
    assert_eq!(
        norito::decode_canonical::<SignerStreamTokenRequestV1>(&window_request_oracle(&f)).unwrap(),
        f.receipt.request
    );
    for omitted in [&[5][..], &[6][..], &[5, 6][..]] {
        let shortened: Vec<u8> = fields
            .iter()
            .enumerate()
            .filter(|(index, _)| !omitted.contains(index))
            .flat_map(|(_, bytes)| bytes.iter().copied())
            .collect();
        let frame = norito::core::frame_bare_with_header_flags::<SignerStreamTokenRequestV1>(
            &shortened, flags,
        )
        .unwrap();
        assert!(norito::decode_canonical::<SignerStreamTokenRequestV1>(&frame).is_err());
    }
}
