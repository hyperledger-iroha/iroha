//! Exact stream receipt tests with independently signed simulations, not hardware qualification.
//!
//! TODO: native execution requires the atomic provider-scoped purpose change, sibling visibility
//! for custody::validate_binding, the Manifest token-body validator move and reviewed shared
//! receipt helper integration. This isolated source candidate does not alter those owners.

use super::*;
use crate::signer::{
    custody::*,
    protocol::*,
    receipt::{
        SignerCompletedOperationV1, SignerOperationFinalizedAnchorV1, SignerOperationProvenanceV1,
    },
};
use iroha_crypto::{Algorithm, KeyPair, Signature};

#[path = "receipt_test_support.rs"]
mod support;
use support::*;

type Error = SignerStreamTokenReceiptErrorV1;
type BodyMutation = (&'static str, fn(&mut StreamTokenBodyV1));
type BindingMutation = (&'static str, fn(&mut SignerCustodyBindingV1));

#[test]
fn signed_receipt_roundtrips_and_recovers_after_original_reservation_expiry() {
    let f = fixture();
    assert_positive(&f);
    assert!(f.completion.completed_at_unix_ms < f.receipt.reservation.expires_at_unix_ms);
    assert!(f.receipt.reservation.expires_at_unix_ms < f.current.now_unix_ms);
    assert!(f.current.now_unix_ms < f.token.body.ttl_epoch * 1_000);
    let bytes = f.receipt.encode_canonical().expect("bounded producer");
    assert_eq!(
        bytes,
        norito::encode_canonical(&f.receipt).expect("canonical oracle")
    );
    assert_eq!(
        bytes.len(),
        norito::canonical_frame_len(&f.receipt).expect("actual frame count")
    );
    assert_eq!(
        decode_receipt(&bytes).expect("bounded roundtrip"),
        f.receipt
    );
    let verified = verify_bytes(&f, &bytes).expect("complete recovery");
    assert_eq!(verified.completion(), &f.completion);
    for display in [
        format!("{:?}", f.receipt),
        format!("{verified:?}"),
        format!("{}", Error::InvalidSignature),
    ] {
        for private_context in [
            &f.token.body.token_id,
            &f.binding.key_handle,
            &f.binding.service_id,
        ] {
            assert!(!display.contains(private_context));
        }
    }
}

#[test]
fn independent_preimages_and_complete_receipt_are_invariant_in_all_ten_layouts() {
    let f = fixture();
    assert_positive(&f);
    let frame = f.receipt.encode_canonical().expect("baseline");
    let payload = oracle_payload(&f.token.body);
    let binding_digest = oracle_canonical(b"iroha.sorafs.signer.custody-binding.v1", &f.binding);
    let audit = oracle_audit(&f.receipt, &f.token.signature);
    for flags in layouts() {
        let _guard = norito::core::DecodeFlagsGuard::enter(flags);
        let expected =
            SignerStreamTokenExpectedV1::new(&f.token.body, &f.binding).expect("subject");
        assert_eq!(
            f.token.body.signing_payload_bytes().expect("payload"),
            payload
        );
        assert_eq!(expected.binding_digest(), binding_digest);
        assert_eq!(
            expected.operation_id(),
            oracle_digest(
                b"iroha.sorafs.signer.stream-token.operation.v1",
                &[&binding_digest, &payload]
            )
        );
        assert_eq!(
            expected.signing_payload_digest(),
            oracle_digest(b"iroha.sorafs.signer.stream-token.payload.v1", &[&payload])
        );
        assert_eq!(expected.signing_payload_size(), payload.len() as u64);
        assert_eq!(
            f.receipt.request.digest().expect("request"),
            oracle_canonical(
                b"iroha.sorafs.signer.stream-token.request.v1",
                &f.receipt.request
            )
        );
        assert_eq!(
            f.receipt.intent.digest().expect("intent"),
            oracle_intent(&f.receipt.intent)
        );
        assert_eq!(
            signer_stream_token_audit_v1(
                &f.receipt.request,
                &f.receipt.intent,
                f.receipt.reservation,
                &f.token.signature
            )
            .expect("audit"),
            audit
        );
        assert_eq!(
            audit.signing_message(),
            oracle_digest(
                b"iroha.external-signer.audit-attestation.v1",
                &[&audit.sequence.to_be_bytes(), &audit.digest]
            )
        );
        assert_eq!(
            f.receipt.provenance.signing_message().expect("provenance"),
            oracle_canonical(
                b"iroha.sorafs.signer.operation.provenance.v1",
                &f.receipt.provenance
            )
        );
        assert_eq!(
            signer_stream_token_response_digest_v1(
                &f.receipt.request,
                &f.receipt.provenance,
                &f.receipt.signatures[..3]
            )
            .expect("response"),
            oracle_response(&f.receipt)
        );
        assert_eq!(
            f.receipt.commitment.response_signing_message(),
            oracle_digest(
                b"iroha.external-signer.response-attestation.v1",
                &[&oracle_response(&f.receipt)]
            )
        );
        assert_eq!(
            signer_operation_signatures_digest_v1(&f.receipt.signatures).expect("all signatures"),
            oracle_signatures(&f.receipt.signatures)
        );
        assert_eq!(
            f.receipt.encode_canonical().expect("producer layout pin"),
            frame
        );
        assert_positive(&f);
        let alternate = norito::core::to_bytes(&f.receipt).expect("ordinary alternate frame");
        assert_eq!(
            norito::decode_from_bytes::<SignerStreamTokenReceiptV1>(&alternate)
                .expect("ordinary decoder positive"),
            f.receipt
        );
        if flags != norito::core::default_encode_flags() {
            assert_ne!(alternate, frame);
            assert_eq!(
                verify_bytes(&f, &alternate).unwrap_err(),
                Error::InvalidReceipt
            );
        }
    }
    assert_ne!(
        audit.digest,
        oracle_canonical(
            b"iroha.sorafs.signer.release-manifest.audit.v1",
            &(
                f.receipt.request,
                oracle_intent(&f.receipt.intent),
                f.receipt.reservation,
                oracle_digest(
                    b"iroha.sorafs.signer.operation.signature.v1",
                    &[&f.token.signature]
                )
            )
        )
    );
    assert_ne!(
        f.receipt.commitment.response_digest,
        oracle_canonical(
            b"iroha.sorafs.signer.release-manifest.response.v1",
            &(
                f.receipt.request,
                f.receipt.provenance,
                oracle_signatures(&f.receipt.signatures[..3])
            )
        )
    );
}

#[test]
fn maximum_valid_receipt_fits_actual_decode_budget_and_producer_bounds_all_leaves() {
    let f = fixture_with(0x21, |body, binding, authority| {
        body.manifest_cid = vec![0xff; 128];
        body.profile_handle = "a".repeat(128);
        body.max_streams = 1_024;
        body.ttl_epoch = body.issued_at + 3_600;
        body.rate_limit_bytes = 1_073_741_824;
        body.requests_per_minute = 10_000;
        body.token_pk_version = u32::MAX;
        binding.key_revision = u64::from(u32::MAX);
        binding.chain_id = "a".repeat(128).parse().expect("maximum chain");
        binding.runtime_handle = format!("hsm:{}", "r".repeat(124));
        binding.key_handle = format!("kms:{}", "k".repeat(124));
        binding.service_id = "s".repeat(128);
        binding.administrator_id = "d".repeat(128);
        authority.service_id = "a".repeat(128);
        authority.administrator_id = "b".repeat(128);
    });
    assert_positive(&f);
    let maximum_custody: SignerCustodyRecordV1 =
        norito::decode_canonical(&f.receipt.custody_record).expect("maximum valid signed custody");
    assert_eq!(maximum_custody.statement.binding, f.binding);
    assert_eq!(maximum_custody.statement.authority, f.trust.authority);
    for leaf in [
        maximum_custody.statement.binding.chain_id.as_str(),
        maximum_custody.statement.binding.runtime_handle.as_str(),
        maximum_custody.statement.binding.key_handle.as_str(),
        maximum_custody.statement.binding.service_id.as_str(),
        maximum_custody.statement.binding.administrator_id.as_str(),
        maximum_custody.statement.authority.service_id.as_str(),
        maximum_custody
            .statement
            .authority
            .administrator_id
            .as_str(),
        f.token.body.profile_handle.as_str(),
    ] {
        assert_eq!(leaf.len(), 128, "actual admitted maximum variable leaf");
    }
    assert_eq!(f.token.body.manifest_cid.len(), 128);
    assert!(oracle_payload(&f.token.body).len() <= 2_048);
    assert!(norito::canonical_frame_len(&f.token).expect("complete signed token") <= 2_048);
    let bytes = f
        .receipt
        .encode_canonical()
        .expect("largest valid variable leaves");
    assert!(bytes.len() <= 64 * 1024);
    let allocation = (64 * 1024 + 8 * bytes.len()).min(512 * 1024);
    let limits = norito::DecodeLimits::new(16 * 1024, bytes.len(), 8192, allocation, 24);
    let (decoded, usage) =
        norito::core::with_decode_limits_measured(limits, || decode_receipt(&bytes));
    assert_eq!(decoded.expect("maximum valid fixture budget"), f.receipt);
    assert!(usage.total_allocated_bytes() > 0);
    assert!(usage.total_allocated_bytes() <= allocation);
    assert!(usage.total_elements() <= 8192);
    for length in [0, 16 * 1024 + 1] {
        let mut receipt = f.receipt.clone();
        receipt.custody_record = vec![0; length];
        assert_eq!(receipt.encode_canonical(), Err(Error::InvalidReceipt));
    }
    for length in [0, 63, 65, 4097] {
        for index in 0..4 {
            let mut receipt = f.receipt.clone();
            receipt.signatures[index].signature = vec![0; length];
            assert_eq!(receipt.encode_canonical(), Err(Error::InvalidSignature));
        }
    }
}

#[test]
fn independently_retained_body_provider_key_and_binding_reject_valid_retargeted_receipts() {
    let baseline = fixture();
    assert_positive(&baseline);
    let bodies: &[BodyMutation] = &[
        ("token id", |v| v.token_id.replace_range(..1, "f")),
        ("manifest", |v| v.manifest_cid[0] ^= 1),
        ("profile", |v| v.profile_handle = "sorafs.sf2@1.0.0".into()),
        ("streams", |v| v.max_streams += 1),
        ("expiry", |v| v.ttl_epoch += 1),
        ("byte rate", |v| v.rate_limit_bytes += 1),
        ("issuance", |v| v.issued_at += 1),
        ("request rate", |v| v.requests_per_minute += 1),
        ("provider", |v| v.provider_id[0] ^= 1),
        ("key version", |v| v.token_pk_version += 1),
    ];
    for (name, mutate) in bodies {
        let mut changed = fixture_with(0x21, |body, binding, _| {
            mutate(body);
            binding.purpose = SignerPurposeBindingV1::StreamToken {
                provider_id: body.provider_id,
            };
            binding.key_revision = u64::from(body.token_pk_version);
        });
        assert_positive(&changed);
        assert_ne!(
            changed.expected.operation_id(),
            baseline.expected.operation_id(),
            "{name}"
        );
        changed.expected =
            SignerStreamTokenExpectedV1::new(&baseline.token.body, &baseline.binding)
                .expect("retained independent subject");
        assert_eq!(
            verify(&changed).unwrap_err(),
            Error::TokenMismatch,
            "{name}"
        );
    }
    let bindings: &[BindingMutation] = &[
        ("chain", |v| v.chain_id = "another-chain".into()),
        ("network", |v| v.network_id[0] ^= 1),
        ("runtime", |v| {
            v.runtime_handle = "hsm:another-runtime".into()
        }),
        ("key handle", |v| v.key_handle = "kms:another-key".into()),
        ("service", |v| v.service_id = "another-service".into()),
        ("administrator", |v| {
            v.administrator_id = "another-admin".into()
        }),
        ("policy revision", |v| v.policy_revision += 1),
        ("policy digest", |v| v.policy_digest[0] ^= 1),
    ];
    for (name, mutate) in bindings {
        let mut changed = fixture_with(0x21, |_, binding, _| mutate(binding));
        assert_positive(&changed);
        changed.expected =
            SignerStreamTokenExpectedV1::new(&baseline.token.body, &baseline.binding)
                .expect("retained subject");
        assert_eq!(
            verify(&changed).unwrap_err(),
            Error::TokenMismatch,
            "{name}"
        );
    }
    let mut another_key = fixture_with(0x22, |_, _, _| {});
    assert_positive(&another_key);
    another_key.expected =
        SignerStreamTokenExpectedV1::new(&baseline.token.body, &baseline.binding)
            .expect("retained key");
    assert_eq!(verify(&another_key).unwrap_err(), Error::TokenMismatch);
    let mut wrong_provider = baseline.token.body.clone();
    wrong_provider.provider_id[0] ^= 1;
    assert_eq!(
        SignerStreamTokenExpectedV1::new(&wrong_provider, &baseline.binding),
        Err(Error::WrongPurpose)
    );
    for revision in [8, u64::from(u32::MAX) + 1, u64::MAX] {
        let mut binding = baseline.binding.clone();
        binding.key_revision = revision;
        assert_eq!(
            SignerStreamTokenExpectedV1::new(&baseline.token.body, &binding),
            Err(Error::TokenMismatch)
        );
    }
    let requests: &[fn(&mut SignerStreamTokenRequestV1)] = &[
        |v| v.operation_id[0] ^= 1,
        |v| v.binding_digest[0] ^= 1,
        |v| v.original_custody.record_digest[0] ^= 1,
        |v| v.original_custody.control_state_digest[0] ^= 1,
        |v| v.signing_payload_digest[0] ^= 1,
        |v| v.signing_payload_size += 1,
    ];
    for mutate in requests {
        let mut changed = fixture();
        assert_positive(&changed);
        mutate(&mut changed.receipt.request);
        assert_eq!(verify(&changed).unwrap_err(), Error::TokenMismatch);
    }
}

#[test]
fn audit_and_response_reject_wrong_action_request_predecessor_and_signature_shape() {
    let f = fixture();
    assert_positive(&f);
    let mutations: &[fn(&mut SignerOperationIntentV1)] = &[
        |v| v.action = SignerOperationActionV1::Qualify,
        |v| v.operation_id[0] ^= 1,
        |v| v.request_digest[0] ^= 1,
        |v| v.previous_audit.sequence = 0,
        |v| v.previous_audit.digest = [0; 32],
        |v| v.previous_audit.sequence = u64::MAX,
    ];
    for mutate in mutations {
        let mut intent = f.receipt.intent;
        mutate(&mut intent);
        assert_eq!(
            signer_stream_token_audit_v1(
                &f.receipt.request,
                &intent,
                f.receipt.reservation,
                &f.token.signature
            ),
            Err(Error::InvalidReceipt)
        );
    }
    for length in [0, 63, 65] {
        assert_eq!(
            signer_stream_token_audit_v1(
                &f.receipt.request,
                &f.receipt.intent,
                f.receipt.reservation,
                &vec![0; length]
            ),
            Err(Error::InvalidReceipt)
        );
    }
    for length in [0, 1, 2, 4] {
        assert_eq!(
            signer_stream_token_response_digest_v1(
                &f.receipt.request,
                &f.receipt.provenance,
                &f.receipt.signatures[..length]
            ),
            Err(Error::InvalidSignature)
        );
    }
    let mut first = f.receipt.signatures[..3].to_vec();
    first.swap(1, 2);
    assert_eq!(
        signer_stream_token_response_digest_v1(&f.receipt.request, &f.receipt.provenance, &first),
        Err(Error::InvalidSignature)
    );
}

#[test]
fn exactly_four_ordered_signatures_bind_each_message_and_cryptographic_signature() {
    for index in 0..4 {
        for message_only in [false, true] {
            let mut f = fixture();
            assert_positive(&f);
            if message_only {
                f.receipt.signatures[index].message_digest[0] ^= 1;
            } else {
                f.receipt.signatures[index].signature[0] ^= 1;
            }
            if index < 3 {
                resign_response(&mut f);
            }
            assert_eq!(verify(&f).unwrap_err(), Error::InvalidSignature);
        }
    }
    let mutations: &[fn(&mut SignerStreamTokenReceiptV1)] = &[
        |r| r.signatures.swap(1, 2),
        |r| r.signatures.swap(0, 3),
        |r| {
            r.signatures.pop();
        },
        |r| r.signatures.push(r.signatures[3].clone()),
        |r| r.signatures[3].purpose = SignerKeyOperationPurposeV1::AuditRecord,
        |r| r.signatures[1].signature.truncate(63),
        |r| r.signatures[2].signature.push(0),
    ];
    for mutate in mutations {
        let mut f = fixture();
        assert_positive(&f);
        mutate(&mut f.receipt);
        assert_eq!(f.receipt.encode_canonical(), Err(Error::InvalidSignature));
        assert_eq!(verify(&f).unwrap_err(), Error::InvalidSignature);
    }
}

#[test]
fn resigned_follow_on_receipts_cannot_authorize_wrong_role_preimages_or_noncanonical_ed25519() {
    let baseline = fixture();
    assert_positive(&baseline);
    let body_bytes = norito::encode_canonical(&baseline.token.body).expect("bare body");
    let alternate = {
        let _guard = norito::core::DecodeFlagsGuard::enter(0);
        norito::core::to_bytes(&baseline.token.body).expect("alternate body")
    };
    assert_eq!(
        norito::decode_from_bytes::<StreamTokenBodyV1>(&alternate)
            .expect("ordinary alternate positive"),
        baseline.token.body
    );
    assert_ne!(alternate, body_bytes);
    let mut alternate_message = b"sorafs.stream-token.signature.v1\0".to_vec();
    alternate_message.extend(alternate);
    let mut wrong_domain = b"sorafs.stream-token.signature.v0\0".to_vec();
    wrong_domain.extend(&body_bytes);
    for message in [
        body_bytes,
        baseline.expected.signing_payload_digest().to_vec(),
        wrong_domain,
        alternate_message,
    ] {
        let mut f = fixture();
        assert_positive(&f);
        f.receipt.signatures[0] = sign(
            &f.signer,
            SignerKeyOperationPurposeV1::RolePayload,
            &message,
        );
        f.token.signature = f.receipt.signatures[0].signature.clone();
        rebuild_operation(&mut f);
        assert_eq!(verify(&f).unwrap_err(), Error::InvalidSignature);
    }
    for kind in 0..3 {
        let mut f = fixture();
        assert_positive(&f);
        match kind {
            0 => {
                f.token.signature[..32].fill(0);
                f.token.signature[0] = 1;
            }
            1 => {
                f.token.signature[..32].fill(0xff);
                f.token.signature[0] = 0xee;
                f.token.signature[31] = 0x7f;
            }
            _ => f.token.signature[32..].fill(0xff),
        }
        f.receipt.signatures[0].signature = f.token.signature.clone();
        rebuild_operation(&mut f);
        assert_eq!(verify(&f).unwrap_err(), Error::InvalidSignature);
    }
}

#[test]
fn receipt_decoder_rejects_headers_missing_current_fields_and_raw_software_signature() {
    let f = fixture();
    assert_positive(&f);
    let bytes = f
        .receipt
        .encode_canonical()
        .expect("positive current schema");
    let mut trailing = bytes.clone();
    trailing.push(0);
    let mut wrong_checksum = bytes.clone();
    *wrong_checksum.last_mut().expect("payload") ^= 1;
    let malformed = [
        Vec::new(),
        bytes[..bytes.len() - 1].to_vec(),
        trailing,
        wrong_checksum,
        vec![0; 64 * 1024 + 1],
        f.token.signature.clone(),
        norito::encode_canonical(&f.token).expect("correct signed software token"),
    ];
    for invalid in malformed {
        assert_eq!(
            verify_bytes(&f, &invalid).unwrap_err(),
            Error::InvalidReceipt
        );
    }
    // This is the unmodified genuine release fixture, verified by its own owner before replay.
    // Stream custody above is independently constructed; no release fixture is relabelled.
    let release = crate::signer::receipt::tests::fixture();
    let release_bytes = norito::encode_canonical(&release.receipt).expect("genuine release frame");
    crate::signer::receipt::verify_release_manifest_signer_receipt_v1(
        &release_bytes,
        &release.manifest,
        &release.receipt.signatures[0].signature,
        &release.expected,
        &release.binding,
        &release.trust,
        &release.current,
        &release.completion,
    )
    .expect("genuine release receipt positive");
    assert_eq!(
        verify_bytes(&f, &release_bytes).unwrap_err(),
        Error::InvalidReceipt
    );
    for length in [0, 63, 65] {
        let mut invalid = fixture();
        assert_positive(&invalid);
        invalid.token.signature = vec![0; length];
        assert_eq!(verify(&invalid).unwrap_err(), Error::InvalidReceipt);
    }
    for omitted in 0..9 {
        let invalid = without_receipt_field(&f.receipt, omitted);
        assert_eq!(
            norito::core::Header::read(invalid.as_slice())
                .expect("actual current schema header")
                .schema,
            norito::core::Header::read(bytes.as_slice())
                .expect("positive header")
                .schema
        );
        assert_eq!(
            verify_bytes(&f, &invalid).unwrap_err(),
            Error::InvalidReceipt
        );
    }
    let compressed = crate::canonical_test_support::with_compression_tag(&f.receipt);
    let mut huge_header = bytes.clone();
    huge_header[23..31].copy_from_slice(&u64::MAX.to_le_bytes());
    for invalid in [
        &compressed[..],
        &compressed[..norito::core::Header::SIZE],
        &huge_header[..],
    ] {
        let (result, usage) = norito::core::with_decode_limits_measured(
            norito::DecodeLimits::new(16 * 1024, invalid.len(), 8192, 512 * 1024, 24),
            || decode_receipt(invalid),
        );
        assert_eq!(result, Err(Error::InvalidReceipt));
        assert_eq!(
            usage.total_allocated_bytes(),
            0,
            "header rejection precedes decoder allocation"
        );
    }
    let flags = norito::core::default_encode_flags();
    let payload = f.receipt.encode();
    let (_, prefix) =
        norito::core::read_len_from_slice_with_flags(&payload, flags).expect("magic field prefix");
    let mut huge_field = Vec::new();
    norito::core::write_len_with_flags(&mut huge_field, 65_537, flags)
        .expect("oversized field claim");
    huge_field.extend_from_slice(&payload[prefix..]);
    let huge_field = norito::core::frame_bare_with_header_flags::<SignerStreamTokenReceiptV1>(
        &huge_field,
        flags,
    )
    .expect("actual schema with oversized field");
    assert!(matches!(
        norito::decode_canonical_with_limits::<SignerStreamTokenReceiptV1>(
            &huge_field,
            norito::DecodeLimits::new(16 * 1024, huge_field.len(), 8192, 512 * 1024, 24),
        ),
        Err(norito::Error::FieldLengthExceeded { .. })
    ));
    assert_eq!(decode_receipt(&huge_field), Err(Error::InvalidReceipt));
    let mut offset = 0;
    for _ in 0..2 {
        let (len, prefix) = norito::core::read_len_from_slice_with_flags(&payload[offset..], flags)
            .expect("leading field");
        offset += prefix + len;
    }
    let (_, prefix) = norito::core::read_len_from_slice_with_flags(&payload[offset..], flags)
        .expect("custody field prefix");
    let custody_start = offset + prefix;
    for count in [8193_u64, 16 * 1024 + 1] {
        let mut claimed = payload.clone();
        claimed[custody_start..custody_start + 8].copy_from_slice(&count.to_le_bytes());
        let claimed = norito::core::frame_bare_with_header_flags::<SignerStreamTokenReceiptV1>(
            &claimed, flags,
        )
        .expect("actual custody vector count claim");
        let error = norito::decode_canonical_with_limits::<SignerStreamTokenReceiptV1>(
            &claimed,
            norito::DecodeLimits::new(16 * 1024, claimed.len(), 8192, 512 * 1024, 24),
        )
        .unwrap_err();
        if count == 8193 {
            assert!(matches!(error, norito::Error::TotalElementsExceeded { .. }));
        } else {
            assert!(matches!(
                error,
                norito::Error::SequenceLengthExceeded { .. }
            ));
        }
        assert_eq!(decode_receipt(&claimed), Err(Error::InvalidReceipt));
    }
    let _guard = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    assert_eq!(
        norito::decode_canonical::<SignerPurposeBindingV1>(
            &norito::encode_canonical(&f.binding.purpose).expect("current provider purpose"),
        )
        .expect("current required provider positive"),
        f.binding.purpose,
    );
    let purpose = f.binding.purpose.encode();
    let unit_frame = norito::core::frame_bare_with_header_flags::<SignerPurposeBindingV1>(
        &purpose[..4],
        norito::core::default_encode_flags(),
    )
    .expect("actual current purpose schema with omitted provider");
    assert!(norito::decode_canonical::<SignerPurposeBindingV1>(&unit_frame).is_err());
    let mut zero_provider = f.binding.clone();
    zero_provider.purpose = SignerPurposeBindingV1::StreamToken {
        provider_id: [0; 32],
    };
    assert_eq!(
        SignerStreamTokenExpectedV1::new(&f.token.body, &zero_provider),
        Err(Error::InvalidReceipt)
    );
    for marker in [*b"IRSRMR01", *b"IRSTKR00"] {
        let mut receipt = f.receipt.clone();
        receipt.magic = marker;
        assert_eq!(receipt.encode_canonical(), Err(Error::InvalidReceipt));
        assert_eq!(
            verify_bytes(
                &f,
                &norito::encode_canonical(&receipt).expect("wrong marker")
            )
            .unwrap_err(),
            Error::InvalidReceipt
        );
    }
    for version in [0, 2, u16::MAX] {
        let mut receipt = f.receipt.clone();
        receipt.version = version;
        assert_eq!(receipt.encode_canonical(), Err(Error::InvalidReceipt));
        assert_eq!(
            verify_bytes(
                &f,
                &norito::encode_canonical(&receipt).expect("wrong version")
            )
            .unwrap_err(),
            Error::InvalidReceipt
        );
    }
}

#[test]
fn every_independent_completion_coordinate_and_finalized_lineage_is_required() {
    let mutations: &[fn(&mut SignerCompletedOperationV1)] = &[
        |c| c.operation_id[0] ^= 1,
        |c| c.intent_digest[0] ^= 1,
        |c| c.original_custody.record_digest[0] ^= 1,
        |c| c.original_custody.control_state_digest[0] ^= 1,
        |c| c.reservation.reservation_id[0] ^= 1,
        |c| c.reservation.fence += 1,
        |c| c.reservation.expires_at_unix_ms += 1,
        |c| c.commitment.audit.sequence += 1,
        |c| c.commitment.audit.digest[0] ^= 1,
        |c| c.commitment.response_digest[0] ^= 1,
        |c| c.signatures_digest[0] ^= 1,
        |c| c.completed_at_unix_ms = 899_999,
        |c| c.completed_at_unix_ms = 1_150_000,
        |c| c.completed_at_unix_ms = 1_160_001,
        |c| c.completed_at_unix_ms = 1_089_999,
        |c| c.anchor.height = 99,
        |c| c.anchor.height = 111,
        |c| c.anchor.block_hash = [0; 32],
        |c| c.anchor.operation_state_digest = [0; 32],
    ];
    for mutate in mutations {
        let mut f = fixture();
        assert_positive(&f);
        mutate(&mut f.completion);
        assert_eq!(verify(&f).unwrap_err(), Error::CompletionMismatch);
    }
    let mut same_height = fixture();
    assert_positive(&same_height);
    same_height.completion.anchor.height = same_height.current.current_anchor.height;
    same_height.completion.anchor.block_hash = same_height.current.current_anchor.block_hash;
    assert_ne!(
        same_height.completion.anchor.operation_state_digest,
        same_height.current.current_anchor.state_digest
    );
    assert_positive(&same_height); // Operation proof is independently authenticated, not a custody-state digest.
    same_height.completion.anchor.block_hash[0] ^= 1;
    assert_eq!(verify(&same_height).unwrap_err(), Error::CompletionMismatch);
    let mut signing_height = fixture();
    assert_positive(&signing_height);
    signing_height.completion.anchor.height =
        signing_height.receipt.provenance.signing_anchor.height;
    signing_height.completion.anchor.block_hash =
        signing_height.receipt.provenance.signing_anchor.block_hash;
    assert_positive(&signing_height);
    signing_height.completion.anchor.block_hash[0] ^= 1;
    assert_eq!(
        verify(&signing_height).unwrap_err(),
        Error::CompletionMismatch
    );
}

#[test]
fn valid_signatures_cannot_replace_original_provenance_or_nonzero_reservation_fences() {
    let mutations: &[fn(&mut SignerOperationProvenanceV1)] = &[
        |p| p.original_custody.record_digest[0] ^= 1,
        |p| p.original_custody.control_state_digest[0] ^= 1,
        |p| p.intent_digest[0] ^= 1,
        |p| p.reservation.reservation_id[0] ^= 1,
        |p| p.reservation.fence += 1,
        |p| p.reservation.expires_at_unix_ms += 1,
        |p| p.audit.sequence += 1,
        |p| p.audit.digest[0] ^= 1,
        |p| p.signing_anchor.state_digest[0] ^= 1,
        |p| p.signing_anchor.height = 89,
        |p| p.signing_anchor.height = 111,
        |p| p.signing_anchor.block_hash = [0; 32],
        |p| {
            p.signing_anchor.height = 90;
            p.signing_anchor.block_hash = [0x99; 32];
        },
        |p| {
            p.signing_anchor.height = 110;
            p.signing_anchor.block_hash = [0x99; 32];
        },
    ];
    for mutate in mutations {
        let mut f = fixture();
        assert_positive(&f);
        mutate(&mut f.receipt.provenance);
        resign_follow_on(&mut f);
        assert_eq!(verify(&f).unwrap_err(), Error::InvalidReceipt);
    }
    for kind in 0..3 {
        let mut f = fixture();
        assert_positive(&f);
        match kind {
            0 => f.receipt.reservation.reservation_id = [0; 32],
            1 => f.receipt.reservation.fence = 0,
            _ => f.receipt.reservation.expires_at_unix_ms = 1_900_001,
        }
        rebuild_operation(&mut f);
        validate_stream_token_signatures_v1(&f.receipt, &f.token, &f.expected, &active(&f))
            .expect("signatures alone do not authorize release");
        assert_eq!(verify(&f).unwrap_err(), Error::CompletionMismatch);
    }
}

#[test]
fn renewed_same_key_custody_cannot_qualify_honest_or_relabelled_old_completion() {
    let mut f = fixture();
    assert_positive(&f);
    let old_window = (
        f.expected.issued_at_unix_ms(),
        f.expected.expires_at_unix_ms(),
    );
    let old_completion = f.completion;
    let old_custody = f.current.active_head.record_digest;
    let mut record: SignerCustodyRecordV1 =
        norito::decode_canonical(&f.receipt.custody_record).expect("original record");
    record.statement.sequence = 2;
    record.statement.predecessor_digest = old_custody;
    // A correctly signed but unenrolled successor retains the currently approved anchor, so
    // the anchor guard passes and the exact ACTIVE generation/record guard must reject it.
    let original_record = f.receipt.custody_record.clone();
    f.receipt.custody_record = attest_unchecked(record.statement.clone(), &f.attester);
    let unenrolled: SignerCustodyRecordV1 =
        norito::decode_canonical(&f.receipt.custody_record).expect("signed unenrolled successor");
    Signature::try_from_bytes(&unenrolled.attestation)
        .expect("strict independent authority signature")
        .verify(
            &f.trust.public_key,
            &unenrolled.statement.signing_payload().unwrap(),
        )
        .expect("the candidate's attestation is valid");
    assert_eq!(
        unenrolled.statement.anchor,
        f.current.active_head.approved_anchor
    );
    assert_ne!(
        unenrolled.statement.sequence,
        f.current.active_head.sequence
    );
    assert_eq!(
        verify(&f).unwrap_err(),
        Error::Custody(SignerCustodyErrorV1::ReplayOrRollback)
    );
    assert_eq!(f.completion, old_completion);
    f.receipt.custody_record = original_record;
    assert_positive(&f);
    record.statement.anchor = f.current.current_anchor;
    record.statement.issued_at_unix_ms = 1_155_000;
    let renewed = attest_unchecked(record.statement.clone(), &f.attester);
    let enrolled = verify_signer_custody_enrollment_v1(
        &renewed,
        &f.binding,
        &f.trust,
        &SignerCustodyEnrollmentContextV1 {
            now_unix_ms: f.current.now_unix_ms,
            anchor_observed_at_unix_ms: f.current.anchor_observed_at_unix_ms,
            current_anchor: record.statement.anchor,
            next_sequence: 2,
            predecessor_digest: old_custody,
            signer_revoked: false,
            attester_revoked: false,
        },
    )
    .expect("valid independently signed same-key renewal");
    f.current.active_head.record_digest = enrolled.record_digest();
    f.current.active_head.sequence = 2;
    f.current.active_head.approved_anchor = record.statement.anchor;
    f.current.current_anchor = SignerCustodyAnchorV1 {
        height: 111,
        block_hash: [0x75; 32],
        state_digest: [0x76; 32],
    };
    let custody = verify_signer_custody_use_v1(&renewed, &f.binding, &f.trust, &f.current)
        .expect("renewed ACTIVE positive");
    assert_eq!(
        SignerStreamTokenExpectedV1::new(&f.token.body, &f.binding)
            .expect("same retained operation"),
        f.expected
    );
    // The old honest record was approved at height 90; the renewed ACTIVE record was approved
    // at height 110. Exact approval-anchor identity is checked before active record/sequence.
    let old_record: SignerCustodyRecordV1 =
        norito::decode_canonical(&f.receipt.custody_record).expect("unchanged honest old record");
    assert_ne!(
        old_record.statement.anchor,
        f.current.active_head.approved_anchor
    );
    assert_eq!(f.completion, old_completion);
    assert_eq!(
        verify(&f).unwrap_err(),
        Error::Custody(SignerCustodyErrorV1::AnchorMismatch)
    );
    f.receipt.custody_record = renewed;
    assert_eq!(verify(&f).unwrap_err(), Error::TokenMismatch);
    f.receipt.request = SignerStreamTokenRequestV1::new(&custody, &f.expected, &f.token.body)
        .expect("same operation new-generation claim");
    assert_eq!(
        (
            f.receipt.request.issued_at_unix_ms,
            f.receipt.request.expires_at_unix_ms
        ),
        old_window
    );
    f.receipt.provenance.signing_anchor = f.current.current_anchor;
    rebuild_operation(&mut f);
    f.completion = old_completion;
    validate_stream_token_signatures_v1(&f.receipt, &f.token, &f.expected, &custody)
        .expect("new signatures cannot replace old immutable completion");
    assert_eq!(verify(&f).unwrap_err(), Error::CompletionMismatch);
}

#[test]
fn independent_current_trust_revocation_generation_and_freshness_cannot_be_supplied_by_receipt() {
    let mutations: &[(fn(&mut Fixture), SignerCustodyErrorV1)] = &[
        (
            |f| f.current.signer_revoked = true,
            SignerCustodyErrorV1::Revoked,
        ),
        (
            |f| f.current.attester_revoked = true,
            SignerCustodyErrorV1::Revoked,
        ),
        (
            |f| f.current.active_head.record_digest[0] ^= 1,
            SignerCustodyErrorV1::ReplayOrRollback,
        ),
        (
            |f| f.current.active_head.sequence += 1,
            SignerCustodyErrorV1::ReplayOrRollback,
        ),
        (
            |f| f.current.active_head.key_revision += 1,
            SignerCustodyErrorV1::ReplayOrRollback,
        ),
        (
            |f| f.current.active_head.policy_revision += 1,
            SignerCustodyErrorV1::ReplayOrRollback,
        ),
        (
            |f| f.current.active_head.policy_digest[0] ^= 1,
            SignerCustodyErrorV1::ReplayOrRollback,
        ),
        (
            |f| f.current.active_head.approved_anchor.state_digest[0] ^= 1,
            SignerCustodyErrorV1::AnchorMismatch,
        ),
        (
            |f| f.current.current_anchor.height = 89,
            SignerCustodyErrorV1::AnchorMismatch,
        ),
        (
            |f| {
                f.current.current_anchor = SignerCustodyAnchorV1 {
                    height: 90,
                    block_hash: [0x43; 32],
                    state_digest: [0x99; 32],
                }
            },
            SignerCustodyErrorV1::AnchorMismatch,
        ),
        (
            |f| f.current.anchor_observed_at_unix_ms = 1_149_999,
            SignerCustodyErrorV1::Freshness,
        ),
        (
            |f| f.current.anchor_observed_at_unix_ms = 1_160_001,
            SignerCustodyErrorV1::Freshness,
        ),
        (
            |f| f.trust.public_key = f.signer.public_key().clone(),
            SignerCustodyErrorV1::SelfAttestation,
        ),
        (
            |f| f.trust.public_key = key(0x32).public_key().clone(),
            SignerCustodyErrorV1::InvalidAttestation,
        ),
        (
            |f| f.trust.authority.policy_revision += 1,
            SignerCustodyErrorV1::UntrustedAuthority,
        ),
    ];
    for (mutate, expected) in mutations {
        let mut f = fixture();
        assert_positive(&f);
        mutate(&mut f);
        assert_eq!(verify(&f).unwrap_err(), Error::Custody(*expected));
    }
    let mut changed_control = fixture();
    assert_positive(&changed_control);
    changed_control.current.current_anchor.state_digest[0] ^= 1;
    active(&changed_control); // A valid current head does not excuse a changed original control generation.
    assert_eq!(verify(&changed_control).unwrap_err(), Error::TokenMismatch);
}

#[test]
fn token_time_checks_use_original_completion_observation_and_exclusive_token_expiry() {
    let mut f = fixture();
    assert_positive(&f);
    f.current.now_unix_ms = f.token.body.ttl_epoch * 1_000 - 1;
    f.current.anchor_observed_at_unix_ms = f.current.now_unix_ms;
    assert_positive(&f);
    f.current.now_unix_ms += 1;
    f.current.anchor_observed_at_unix_ms += 1;
    assert_eq!(verify(&f).unwrap_err(), Error::TokenExpired);
    validate_stream_token_signatures_v1(&f.receipt, &f.token, &f.expected, &active(&f))
        .expect("signature-only result does not claim token liveness");
    let mut observation = fixture();
    assert_positive(&observation);
    observation.current.now_unix_ms = 1_140_000;
    observation.current.anchor_observed_at_unix_ms = 1_131_000;
    assert_positive(&observation);
    observation.completion.completed_at_unix_ms = 1_132_000;
    assert_eq!(verify(&observation).unwrap_err(), Error::InvalidTime);
    let mut future = fixture_with(0x21, |body, _, _| {
        body.issued_at = 1_126;
        body.ttl_epoch = 1_200;
    });
    future.completion.completed_at_unix_ms = 1_126_000;
    assert_positive(&future);
    future.completion.completed_at_unix_ms -= 1;
    assert_eq!(verify(&future).unwrap_err(), Error::InvalidTime);
    let mut boundary = fixture();
    assert_positive(&boundary);
    boundary.completion.completed_at_unix_ms = boundary.receipt.reservation.expires_at_unix_ms - 1;
    assert_positive(&boundary);
    boundary.completion.completed_at_unix_ms += 1;
    assert_eq!(verify(&boundary).unwrap_err(), Error::CompletionMismatch);
    let mut overflow = fixture();
    assert_positive(&overflow);
    overflow.token.body.issued_at = u64::MAX / 1_000;
    overflow.token.body.ttl_epoch = overflow.token.body.issued_at + 1;
    assert_eq!(verify(&overflow).unwrap_err(), Error::InvalidTime);
}

#[test]
fn resigned_wrong_purpose_and_software_origin_custody_never_authorize_stream_receipts() {
    let mutations: &[(fn(&mut SignerCustodyStatementV1), SignerCustodyErrorV1)] = &[
        (
            |s| s.exportable = true,
            SignerCustodyErrorV1::HardwareCustodyRequired,
        ),
        (
            |s| s.generated_in_hardware = false,
            SignerCustodyErrorV1::HardwareCustodyRequired,
        ),
        (
            |s| s.ever_exported = true,
            SignerCustodyErrorV1::HardwareCustodyRequired,
        ),
        (|s| s.revoked = true, SignerCustodyErrorV1::Revoked),
    ];
    for (mutate, expected) in mutations {
        let mut f = fixture();
        assert_positive(&f);
        let mut record: SignerCustodyRecordV1 =
            norito::decode_canonical(&f.receipt.custody_record).expect("record");
        mutate(&mut record.statement);
        f.receipt.custody_record = attest_unchecked(record.statement, &f.attester);
        assert_eq!(verify(&f).unwrap_err(), Error::Custody(*expected));
    }
    let mut f = fixture();
    assert_positive(&f);
    f.binding.role = SignerRoleV1::Promotion;
    f.binding.purpose = SignerPurposeBindingV1::NativeOrPromotion;
    let mut record: SignerCustodyRecordV1 =
        norito::decode_canonical(&f.receipt.custody_record).expect("record");
    record.statement.binding = f.binding.clone();
    f.receipt.custody_record = attest_unchecked(record.statement.clone(), &f.attester);
    let enrolled = verify_signer_custody_enrollment_v1(
        &f.receipt.custody_record,
        &f.binding,
        &f.trust,
        &SignerCustodyEnrollmentContextV1 {
            now_unix_ms: f.current.now_unix_ms,
            anchor_observed_at_unix_ms: f.current.anchor_observed_at_unix_ms,
            current_anchor: record.statement.anchor,
            next_sequence: 1,
            predecessor_digest: [0; 32],
            signer_revoked: false,
            attester_revoked: false,
        },
    )
    .expect("coherent signed Promotion enrollment positive");
    f.current.active_head.record_digest = enrolled.record_digest();
    active(&f);
    assert_eq!(verify(&f).unwrap_err(), Error::WrongPurpose);
}

#[test]
fn public_structural_receipt_decode_and_role_claim_never_authorize_a_signature() {
    let f = fixture();
    assert_positive(&f);
    let bytes = f.receipt.encode_canonical().unwrap();
    for flags in layouts() {
        let _guard = norito::core::DecodeFlagsGuard::enter(flags);
        let claim = SignerStreamTokenReceiptV1::decode_canonical(&bytes).unwrap();
        assert_eq!(claim, f.receipt);
        assert_eq!(
            claim.role_signature_claim().unwrap().as_slice(),
            f.token.signature
        );
    }
    let mut forged = f.receipt.clone();
    forged.signatures[0].signature[0] ^= 1;
    let bytes = forged.encode_canonical().unwrap();
    let claim = SignerStreamTokenReceiptV1::decode_canonical(&bytes).unwrap();
    assert_ne!(
        claim.role_signature_claim().unwrap().as_slice(),
        f.token.signature
    );
    assert!(
        verify_bytes(&f, &bytes).is_err(),
        "structural extraction cannot qualify a forged signature"
    );
    for index in 0..4 {
        let mut wrong = f.receipt.clone();
        wrong.signatures[index].signature.pop();
        assert_eq!(wrong.role_signature_claim(), Err(Error::InvalidSignature));
        let mut wrong = f.receipt.clone();
        wrong.signatures[index].purpose = if index == 0 {
            SignerKeyOperationPurposeV1::Response
        } else {
            SignerKeyOperationPurposeV1::RolePayload
        };
        assert_eq!(wrong.role_signature_claim(), Err(Error::InvalidSignature));
    }
    for length in [0, 1, 3, 5] {
        let mut wrong = f.receipt.clone();
        wrong
            .signatures
            .resize(length, f.receipt.signatures[0].clone());
        assert_eq!(wrong.role_signature_claim(), Err(Error::InvalidSignature));
    }
    for invalid in [
        Vec::new(),
        vec![0; SIGNER_STREAM_TOKEN_RECEIPT_MAX_BYTES_V1 + 1],
        crate::canonical_test_support::with_compression_tag(&f.receipt),
    ] {
        let (result, usage) = norito::core::with_decode_limits_measured(
            norito::DecodeLimits::new(
                16 * 1024,
                SIGNER_STREAM_TOKEN_RECEIPT_MAX_BYTES_V1,
                8192,
                512 * 1024,
                24,
            ),
            || SignerStreamTokenReceiptV1::decode_canonical(&invalid),
        );
        assert_eq!(result, Err(Error::InvalidReceipt));
        assert_eq!(usage.total_allocated_bytes(), 0);
    }
}

include!("prepared_window_tests.rs");
