//! Canonical persisted-state and bounded recovery regressions.

use super::*;
use crate::smartcontracts::isi::query::SingularQueryCurrentAllocation;

type PersistedState = ReserveAppealRecordV1;

fn fixture() -> PersistedState {
    let private =
        iroha_crypto::PrivateKey::from_bytes(iroha_crypto::Algorithm::Ed25519, &[0x67; 32])
            .expect("deterministic test seed");
    let keypair = iroha_crypto::KeyPair::from_private_key(private).expect("test keypair");
    let account = AccountId::new(keypair.public_key().clone());
    ReserveAppealRecordV1 {
        appeal_id: [0x21; 32],
        provider_id: ProviderId::new([0x22; 32]),
        submitted_by: account.clone(),
        requested_stage: ReserveLifecycleStage::Active,
        reason: "reserve appeal canonical recovery".repeat(3),
        evidence_digest: Some([0x23; 32]),
        expected_provider_revision: 4,
        status: ReserveAppealStatusV1::Accepted,
        submitted_at_unix: 1_000,
        decided_by: Some(account),
        decided_at_unix: Some(1_001),
        rationale: Some("confirmed reserve appeal evidence".repeat(3)),
    }
}

fn valid_caller_layouts() -> impl Iterator<Item = u8> {
    (0..=u8::MAX).filter(|flags| norito::core::validate_header_flags(*flags).is_ok())
}

fn assert_noncanonical(error: InstructionExecutionError) {
    assert!(
        matches!(error, InstructionExecutionError::InvariantViolation(ref message)
        if message.to_string() == "canonical test state is not exact canonical Norito"),
        "{error:?}"
    );
}

#[test]
fn canonical_state_is_stable_and_recovers_with_measured_allocation_in_every_caller_layout() {
    let value = fixture();
    let expected = norito::encode_canonical(&value).expect("canonical fixture");
    let (decoded, measured) = norito::core::with_decode_limits_measured(STATE_LIMITS, || {
        norito::decode_canonical_with_limits::<PersistedState>(&expected, STATE_LIMITS)
    });
    assert_eq!(decoded.expect("canonical reference recovery"), value);
    assert!(
        measured.total_allocated_bytes() > 0,
        "fixture must allocate owned state"
    );
    for flags in valid_caller_layouts() {
        let _caller = norito::core::DecodeFlagsGuard::enter(flags);
        assert_eq!(
            encode_state(&value, "canonical test").expect("persist state"),
            expected,
            "caller flags {flags:#04x}"
        );
        assert_eq!(
            decode_state_with_current::<PersistedState>(&expected, "canonical test", None)
                .expect("recover state without current allocation"),
            value
        );
        let mut current = SingularQueryCurrentAllocation::new(17).expect("retained accounting");
        assert_eq!(
            decode_state_for_current::<PersistedState>(&expected, "canonical test", &mut current)
                .expect("recover state with current allocation"),
            value
        );
        assert_eq!(
            current.resident_bytes(),
            17 + measured.total_allocated_bytes(),
            "one exact nested allocation charge under flags {flags:#04x}"
        );
    }
}

#[test]
fn canonical_state_rejects_well_formed_alternate_layouts_in_every_caller_layout() {
    let value = fixture();
    let canonical = norito::encode_canonical(&value).expect("canonical fixture");
    let mut alternate_count = 0;
    for encoded_flags in valid_caller_layouts() {
        let alternate = {
            let _layout = norito::core::DecodeFlagsGuard::enter(encoded_flags);
            norito::to_bytes(&value).expect("advertised alternate frame")
        };
        if alternate == canonical {
            continue;
        }
        alternate_count += 1;
        let header = norito::core::Header::read(alternate.as_slice()).expect("valid header");
        {
            let _advertised = norito::core::DecodeFlagsGuard::enter(header.flags);
            assert_eq!(
                norito::decode_from_bytes_with_limits::<PersistedState>(&alternate, STATE_LIMITS)
                    .expect("alternate must be well formed and represent the same value"),
                value
            );
        }
        for caller_flags in valid_caller_layouts() {
            let _caller = norito::core::DecodeFlagsGuard::enter(caller_flags);
            assert_noncanonical(
                decode_state_with_current::<PersistedState>(&alternate, "canonical test", None)
                    .expect_err("alternate state is forbidden"),
            );
            let mut current = SingularQueryCurrentAllocation::new(17).expect("retained accounting");
            assert_noncanonical(
                decode_state_for_current::<PersistedState>(
                    &alternate,
                    "canonical test",
                    &mut current,
                )
                .expect_err("alternate state is forbidden"),
            );
            assert_eq!(
                current.resident_bytes(),
                17,
                "rejected state is not retained"
            );
        }
    }
    assert!(
        alternate_count > 0,
        "exercise actual same-value alternate encodings"
    );
}

#[test]
fn canonical_state_rejects_compression_and_invalid_layout_headers_before_allocation() {
    let canonical = norito::encode_canonical(&fixture()).expect("canonical fixture");
    let header = norito::core::Header::read(canonical.as_slice()).expect("canonical header");
    let mut compressed = canonical.clone();
    let compression_offset = header.magic.len() + 2 + header.schema.len();
    compressed[compression_offset] = norito::Compression::Zstd as u8;
    assert_eq!(
        norito::core::Header::read(compressed.as_slice())
            .expect("valid compressed header")
            .compression,
        norito::Compression::Zstd
    );
    let mut invalid_layout = canonical;
    invalid_layout[norito::core::Header::SIZE - 1] = norito::core::header_flags::FIELD_BITSET;
    assert!(norito::core::Header::read(invalid_layout.as_slice()).is_err());
    for flags in valid_caller_layouts() {
        let _caller = norito::core::DecodeFlagsGuard::enter(flags);
        for (frame, is_compressed) in [(&compressed, true), (&invalid_layout, false)] {
            for bytes in [frame.as_slice(), &frame[..norito::core::Header::SIZE]] {
                let mut current =
                    SingularQueryCurrentAllocation::new(17).expect("retained accounting");
                let (result, usage) =
                    norito::core::with_decode_limits_measured(STATE_LIMITS, || {
                        decode_state_for_current::<PersistedState>(
                            bytes,
                            "canonical test",
                            &mut current,
                        )
                    });
                let error = result.expect_err("forbidden header admitted");
                if is_compressed {
                    assert_noncanonical(error);
                } else {
                    assert!(matches!(
                        error,
                        InstructionExecutionError::InvariantViolation(_)
                    ));
                }
                assert_eq!(
                    usage.total_allocated_bytes(),
                    0,
                    "reject at header admission"
                );
                assert_eq!(current.resident_bytes(), 17);
            }
        }
    }
}

#[test]
fn canonical_state_preserves_frame_ceiling_and_stricter_nested_allocation_limits() {
    let oversized = vec![0; STATE_MAX_BYTES + 1];
    let (result, usage) = norito::core::with_decode_limits_measured(STATE_LIMITS, || {
        decode_state_with_current::<PersistedState>(&oversized, "canonical test", None)
    });
    let error = result.expect_err("oversized persisted state");
    assert!(
        matches!(error, InstructionExecutionError::InvariantViolation(ref message)
        if message.to_string() == format!("canonical test state exceeds {STATE_MAX_BYTES} bytes")),
        "{error:?}"
    );
    assert_eq!(usage.total_allocated_bytes(), 0);
    let canonical = norito::encode_canonical(&fixture()).expect("canonical fixture");
    let no_allocation = DecodeLimits::new(
        STATE_LIMITS.max_sequence_elements(),
        STATE_LIMITS.max_field_bytes(),
        STATE_LIMITS.max_total_elements(),
        0,
        STATE_LIMITS.max_nesting_depth(),
    );
    let mut current = SingularQueryCurrentAllocation::new(17).expect("retained accounting");
    let (result, usage) = norito::core::with_decode_limits_measured(no_allocation, || {
        decode_state_for_current::<PersistedState>(&canonical, "canonical test", &mut current)
    });
    assert!(
        matches!(
            result,
            Err(InstructionExecutionError::InvariantViolation(_))
        ),
        "stricter caller allocation limits must reject otherwise valid state: {result:?}"
    );
    assert_eq!(usage.total_allocated_bytes(), 0);
    assert_eq!(current.resident_bytes(), 17);
}
