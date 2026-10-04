//! Private state, statement, proof, receipt and package tests.
//!
//! The fixtures are visible to the other wallet test modules so later areas build packages
//! from the same statements.

use super::*;
use crate::kagemusha::kagemusha_wallet_v1::{
    KAGEMUSHA_WALLET_ACTIVATION_MAX_BYTES_V1, KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1,
    KagemushaWalletValidationErrorV1,
    codec_tests::{assert_every_flip_rejected_or_rebound, norito_tag},
    decode_frame_v1,
    identity::{
        KagemushaWalletEvidenceKindV1,
        identity_tests::{IdentityFixture, identity_fixture, raw_output, signing_key},
    },
};

/// Stand-in empty-map roots; the real constants come from the G3 map owner.
pub(in crate::kagemusha::kagemusha_wallet_v1) const EMPTY_ROOTS: KagemushaWalletEmptyMapRootsV1 =
    KagemushaWalletEmptyMapRootsV1 {
        consumed_credit: [0xc1; 32],
        pending_outgoing: [0xc2; 32],
        load_recovery: [0xc3; 32],
        redeem_recovery: [0xc4; 32],
        fee_claim: [0xc5; 32],
        quota_usage: [0xc6; 32],
    };
/// Stand-in recovery capsule digest.
pub(in crate::kagemusha::kagemusha_wallet_v1) const CAPSULE: [u8; 32] = [0xca; 32];
const MARKER: [u8; 32] = [0x61; 32];

/// Distinct complete commitment for a small seed (`seed` must not be 0 or 0x80).
pub(in crate::kagemusha::kagemusha_wallet_v1) fn commitment(
    seed: u8,
) -> KagemushaWalletStateCommitmentV1 {
    KagemushaWalletStateCommitmentV1 {
        eq: [seed; 32],
        ep: [seed ^ 0x80; 32],
    }
}

/// Valid Bootstrap statement of `f`'s credential.
pub(in crate::kagemusha::kagemusha_wallet_v1) fn bootstrap_statement(
    f: &IdentityFixture,
) -> KagemushaWalletStatementV1 {
    KagemushaWalletStatementV1 {
        version: KAGEMUSHA_WALLET_VERSION_V1,
        scheme_id: f.credential.body.scheme_id,
        relation_id: f.scheme.relation_id,
        credential_digest: f.credential.credential_digest(),
        asset_digest: f.credential.body.asset_digest,
        lifecycle: KagemushaWalletLifecycleV1::Active,
        sequence: 0,
        next_load: 0,
        predecessor: KagemushaWalletStateCommitmentV1::ZERO,
        successor: commitment(1),
        effect: KagemushaWalletEffectV1::Bootstrap {
            enrollment_id: f.credential.body.enrollment_id,
            enrollment_marker: MARKER,
        },
    }
}

/// Valid non-Bootstrap statement at `sequence` (at most 100) with `effect`.
pub(in crate::kagemusha::kagemusha_wallet_v1) fn transition_statement(
    f: &IdentityFixture,
    sequence: u8,
    next_load: u128,
    lifecycle: KagemushaWalletLifecycleV1,
    effect: KagemushaWalletEffectV1,
) -> KagemushaWalletStatementV1 {
    KagemushaWalletStatementV1 {
        sequence: u128::from(sequence),
        next_load,
        lifecycle,
        predecessor: commitment(sequence),
        successor: commitment(sequence + 1),
        effect,
        ..bootstrap_statement(f)
    }
}

/// Valid Send effect to a foreign receiver.
pub(in crate::kagemusha::kagemusha_wallet_v1) fn send_effect() -> KagemushaWalletEffectV1 {
    KagemushaWalletEffectV1::Send {
        credit_id: [0x71; 32],
        receiver_wallet_id: [0x72; 32],
        send_ordinal: 4,
        amount: 1_000,
        fee: 10,
        request: [0x73; 32],
        dependencies: [0x74; 32],
        accepted_lower_ms: 5,
        accepted_upper_ms: 9,
    }
}

/// One valid effect of every kind for `f`'s credential, in tag order.
fn sample_effects(f: &IdentityFixture) -> [KagemushaWalletEffectV1; 8] {
    let body = &f.credential.body;
    [
        KagemushaWalletEffectV1::Bootstrap {
            enrollment_id: body.enrollment_id,
            enrollment_marker: MARKER,
        },
        KagemushaWalletEffectV1::Load {
            voucher: [0x62; 32],
            load_ordinal: 0,
            amount: 5_000,
            online_charge: 25,
        },
        send_effect(),
        KagemushaWalletEffectV1::Receive {
            credit_id: [0x75; 32],
            payer_wallet_id: [0x76; 32],
            payment: [0x77; 32],
            amount: 300,
        },
        KagemushaWalletEffectV1::ArchiveSent {
            credit_id: [0x71; 32],
            credited: [0x78; 32],
        },
        KagemushaWalletEffectV1::Unload {
            nullifier: kagemusha_wallet_unload_nullifier_v1(&body.scheme_id, &body.wallet_id, 2),
            redeem_ordinal: 2,
            amount: 700,
            online_charge: 7,
            charge_quote: [0x79; 32],
        },
        KagemushaWalletEffectV1::RefreshPolicy {
            update_kind: KagemushaWalletPolicyUpdateKindV1::SchemePolicy,
            update: [0x7a; 32],
            accepted_time_floor_ms: 12,
        },
        KagemushaWalletEffectV1::Retiring,
    ]
}

/// Valid statement carrying `effect` of `f`'s credential.
fn statement_for(
    f: &IdentityFixture,
    effect: KagemushaWalletEffectV1,
) -> KagemushaWalletStatementV1 {
    match effect {
        KagemushaWalletEffectV1::Bootstrap { .. } => bootstrap_statement(f),
        KagemushaWalletEffectV1::Load { load_ordinal, .. } => transition_statement(
            f,
            3,
            load_ordinal + 1,
            KagemushaWalletLifecycleV1::Active,
            effect,
        ),
        KagemushaWalletEffectV1::Retiring => {
            transition_statement(f, 3, 0, KagemushaWalletLifecycleV1::Retiring, effect)
        }
        _ => transition_statement(f, 3, 0, KagemushaWalletLifecycleV1::Active, effect),
    }
}

/// Stand-in transition proof of `len` bytes.
pub(in crate::kagemusha::kagemusha_wallet_v1) fn stand_in_proof(
    len: usize,
) -> KagemushaWalletProofV1 {
    KagemushaWalletProofV1 {
        bytes: (0..len)
            .map(|index| u8::try_from(index % 251).expect("byte"))
            .collect(),
    }
}

/// Package of `statement` and `proof` with a receipt signed by `f`'s payment key.
pub(in crate::kagemusha::kagemusha_wallet_v1) fn signed_package(
    f: &IdentityFixture,
    credential: &KagemushaWalletCredentialV1,
    statement: &KagemushaWalletStatementV1,
    proof: KagemushaWalletProofV1,
) -> KagemushaWalletPackageV1 {
    let body = KagemushaWalletReceiptBodyV1::derive(credential, statement, &proof, CAPSULE)
        .expect("receipt body");
    let receipt = KagemushaWalletReceiptV1::sign(
        credential,
        statement,
        &proof,
        CAPSULE,
        raw_output(&f.payment, &body.signing_message()),
    )
    .expect("receipt");
    KagemushaWalletPackageV1::new(*statement, proof, receipt)
}

fn android() -> IdentityFixture {
    identity_fixture(KagemushaWalletEvidenceKindV1::AndroidKeyMintTee, 0x47)
}

fn is_invalid<T>(result: WalletResult<T>, expected: &str) -> bool {
    matches!(
        result.err(),
        Some(KagemushaWalletValidationErrorV1::InvalidField { field }) if field == expected
    )
}

fn effect_fields_len(effect: &KagemushaWalletEffectV1) -> usize {
    effect
        .write_fields(WalletTranscriptV1::with_capacity(0))
        .finish()
        .len()
}

#[test]
fn kagemusha_wallet_v1_state_transcript_lengths_are_pinned() {
    let f = android();
    for (constant, expected) in [
        (KAGEMUSHA_WALLET_COMMITMENT_TRANSCRIPT_BYTES_V1, 64),
        (KAGEMUSHA_WALLET_EFFECT_UNION_BYTES_V1, 192),
        (KAGEMUSHA_WALLET_EFFECT_TRANSCRIPT_BYTES_V1, 193),
        (KAGEMUSHA_WALLET_STATEMENT_TRANSCRIPT_BYTES_V1, 484),
        (KAGEMUSHA_WALLET_RECEIPT_BODY_TRANSCRIPT_BYTES_V1, 370),
        (KAGEMUSHA_WALLET_OPERATION_ID_TRANSCRIPT_BYTES_V1, 65),
        (KAGEMUSHA_WALLET_PACKAGE_TRANSCRIPT_BYTES_V1, 96),
        (KAGEMUSHA_WALLET_UNLOAD_NULLIFIER_TRANSCRIPT_BYTES_V1, 80),
    ] {
        assert_eq!(constant, expected);
    }
    assert_eq!(max_width_v1(&[]), 0);
    assert_eq!(max_width_v1(&[3, 9, 4]), 9);
    assert_eq!(
        KAGEMUSHA_WALLET_EFFECT_UNION_BYTES_V1,
        *EFFECT_FIELDS_BYTES.iter().max().expect("variants")
    );
    let effects = sample_effects(&f);
    for (effect, width) in effects.iter().zip(EFFECT_FIELDS_BYTES) {
        assert_eq!(effect_fields_len(effect), width, "{effect:?}");
        assert_eq!(effect.fields_bytes(), width);
        assert_eq!(
            effect.transcript().len(),
            KAGEMUSHA_WALLET_EFFECT_TRANSCRIPT_BYTES_V1
        );
        let statement = statement_for(&f, *effect);
        assert_eq!(
            statement.transcript().len(),
            KAGEMUSHA_WALLET_STATEMENT_TRANSCRIPT_BYTES_V1
        );
    }
    assert_eq!(
        commitment(3).transcript().len(),
        KAGEMUSHA_WALLET_COMMITMENT_TRANSCRIPT_BYTES_V1
    );
    let statement = bootstrap_statement(&f);
    let proof = stand_in_proof(10);
    let body = KagemushaWalletReceiptBodyV1::derive(&f.credential, &statement, &proof, CAPSULE)
        .expect("receipt body");
    assert_eq!(
        body.transcript().len(),
        KAGEMUSHA_WALLET_RECEIPT_BODY_TRANSCRIPT_BYTES_V1
    );
}

#[test]
fn kagemusha_wallet_v1_state_enum_tags_equal_norito_tags() {
    let f = android();
    for lifecycle in KagemushaWalletLifecycleV1::ALL {
        assert_eq!(norito_tag(&lifecycle), u32::from(lifecycle.tag()));
    }
    for (index, kind) in KagemushaWalletOperationKindV1::ALL.into_iter().enumerate() {
        assert_eq!(norito_tag(&kind), u32::from(kind.tag()));
        assert_eq!(usize::from(kind.tag()), index + 1);
    }
    for kind in KagemushaWalletPolicyUpdateKindV1::ALL {
        assert_eq!(norito_tag(&kind), u32::from(kind.tag()));
    }
    for (effect, kind) in sample_effects(&f)
        .iter()
        .zip(KagemushaWalletOperationKindV1::ALL)
    {
        assert_eq!(effect.kind(), kind);
        assert_eq!(effect.tag(), kind.tag());
        assert_eq!(norito_tag(effect), u32::from(effect.tag()));
    }
}

#[test]
fn kagemusha_wallet_v1_effect_transcripts_are_zero_filled_unions() {
    let f = android();
    for effect in sample_effects(&f) {
        let transcript = effect.transcript();
        assert_eq!(transcript[0], effect.tag());
        let fields = effect_fields_len(&effect);
        assert!(transcript[1 + fields..].iter().all(|byte| *byte == 0));
    }
    assert_eq!(
        KagemushaWalletEffectV1::Retiring.transcript(),
        [&[8_u8][..], &[0; 192][..]].concat()
    );
    let mut expected = vec![3_u8];
    expected.extend_from_slice(&[0x71; 32]);
    expected.extend_from_slice(&[0x72; 32]);
    expected.extend_from_slice(&4_u128.to_le_bytes());
    expected.extend_from_slice(&1_000_u128.to_le_bytes());
    expected.extend_from_slice(&10_u128.to_le_bytes());
    expected.extend_from_slice(&[0x73; 32]);
    expected.extend_from_slice(&[0x74; 32]);
    expected.extend_from_slice(&5_u64.to_le_bytes());
    expected.extend_from_slice(&9_u64.to_le_bytes());
    assert_eq!(send_effect().transcript(), expected);

    let refresh = KagemushaWalletEffectV1::RefreshPolicy {
        update_kind: KagemushaWalletPolicyUpdateKindV1::TimeAnchor,
        update: [0x7b; 32],
        accepted_time_floor_ms: 0x0102,
    };
    let transcript = refresh.transcript();
    assert_eq!(&transcript[..2], &[7, 5]);
    assert_eq!(&transcript[2..34], &[0x7b; 32]);
    assert_eq!(&transcript[34..42], &0x0102_u64.to_le_bytes());
    assert!(transcript[42..].iter().all(|byte| *byte == 0));
}

#[test]
fn kagemusha_wallet_v1_effect_validation_rules() {
    let f = android();
    for effect in sample_effects(&f) {
        effect.validate().expect("sample effect");
    }
    let KagemushaWalletEffectV1::Send {
        credit_id,
        receiver_wallet_id,
        send_ordinal,
        request,
        dependencies,
        ..
    } = send_effect()
    else {
        unreachable!("send effect");
    };
    let send = |amount, fee, lower, upper| KagemushaWalletEffectV1::Send {
        credit_id,
        receiver_wallet_id,
        send_ordinal,
        amount,
        fee,
        request,
        dependencies,
        accepted_lower_ms: lower,
        accepted_upper_ms: upper,
    };
    assert!(is_invalid(send(0, 0, 1, 1).validate(), "effect.amount"));
    assert!(matches!(
        send(u128::MAX, 1, 1, 1).validate(),
        Err(KagemushaWalletValidationErrorV1::ArithmeticOverflow {
            field: "effect.gross"
        })
    ));
    send(u128::MAX, 0, 1, 1).validate().expect("maximal gross");
    assert!(is_invalid(
        send(1, 0, 2, 1).validate(),
        "effect.accepted_time"
    ));
    let mut zero_request = send(1, 0, 1, 1);
    if let KagemushaWalletEffectV1::Send { request, .. } = &mut zero_request {
        *request = [0; 32];
    }
    assert!(is_invalid(zero_request.validate(), "effect.request"));

    let unload = |amount, online_charge, charge_quote| KagemushaWalletEffectV1::Unload {
        nullifier: [0x01; 32],
        redeem_ordinal: 0,
        amount,
        online_charge,
        charge_quote,
    };
    unload(10, 0, [0; 32]).validate().expect("no charge");
    unload(10, 10, [1; 32]).validate().expect("full charge");
    assert!(is_invalid(
        unload(0, 0, [0; 32]).validate(),
        "effect.amount"
    ));
    assert!(is_invalid(
        unload(10, 11, [1; 32]).validate(),
        "effect.online_charge"
    ));
    assert!(is_invalid(
        unload(10, 1, [0; 32]).validate(),
        "effect.charge_quote"
    ));
    assert!(is_invalid(
        unload(10, 0, [1; 32]).validate(),
        "effect.charge_quote"
    ));

    for (effect, field) in [
        (
            KagemushaWalletEffectV1::Bootstrap {
                enrollment_id: [0; 32],
                enrollment_marker: MARKER,
            },
            "effect.enrollment_id",
        ),
        (
            KagemushaWalletEffectV1::Bootstrap {
                enrollment_id: [1; 32],
                enrollment_marker: [0; 32],
            },
            "effect.enrollment_marker",
        ),
        (
            KagemushaWalletEffectV1::Load {
                voucher: [0; 32],
                load_ordinal: 0,
                amount: 1,
                online_charge: 0,
            },
            "effect.voucher",
        ),
        (
            KagemushaWalletEffectV1::Receive {
                credit_id: [1; 32],
                payer_wallet_id: [1; 32],
                payment: [1; 32],
                amount: 0,
            },
            "effect.amount",
        ),
        (
            KagemushaWalletEffectV1::Receive {
                credit_id: [1; 32],
                payer_wallet_id: [1; 32],
                payment: [0; 32],
                amount: 1,
            },
            "effect.payment",
        ),
        (
            KagemushaWalletEffectV1::ArchiveSent {
                credit_id: [1; 32],
                credited: [0; 32],
            },
            "effect.credited",
        ),
        (
            KagemushaWalletEffectV1::RefreshPolicy {
                update_kind: KagemushaWalletPolicyUpdateKindV1::Blacklist,
                update: [0; 32],
                accepted_time_floor_ms: 0,
            },
            "effect.update",
        ),
    ] {
        assert!(is_invalid(effect.validate(), field), "{field}");
    }
}

#[test]
fn kagemusha_wallet_v1_operation_id_derivation() {
    let f = android();
    let wallet_id = f.credential.body.wallet_id;
    let inputs = [
        f.credential.body.enrollment_id,
        [0x62; 32],
        [0x71; 32],
        [0x75; 32],
        [0x71; 32],
        sample_effects(&f)[5].operation_input(),
        [0x7a; 32],
        [0; 32],
    ];
    let mut seen = std::collections::BTreeSet::new();
    for ((effect, kind), input) in sample_effects(&f)
        .iter()
        .zip(KagemushaWalletOperationKindV1::ALL)
        .zip(inputs)
    {
        assert_eq!(effect.operation_input(), input);
        let mut transcript = wallet_id.to_vec();
        transcript.push(kind.tag());
        transcript.extend_from_slice(&input);
        assert_eq!(
            kagemusha_wallet_operation_id_transcript_v1(&wallet_id, kind, &input),
            transcript
        );
        let operation_id = kagemusha_wallet_operation_id_v1(&wallet_id, kind, &input);
        assert_eq!(
            operation_id,
            kagemusha_wallet_digest_v1(Role::OperationId, &transcript)
        );
        assert_eq!(
            statement_for(&f, *effect).operation_id(&wallet_id),
            operation_id
        );
        assert!(seen.insert(operation_id), "{kind:?}");
    }
    // Send and ArchiveSent of one credit share the input but not the identity.
    assert_ne!(
        kagemusha_wallet_operation_id_v1(
            &wallet_id,
            KagemushaWalletOperationKindV1::Send,
            &[1; 32]
        ),
        kagemusha_wallet_operation_id_v1(
            &wallet_id,
            KagemushaWalletOperationKindV1::ArchiveSent,
            &[1; 32]
        )
    );
}

#[test]
fn kagemusha_wallet_v1_unload_nullifier_layout() {
    let scheme = [0x01; 32];
    let wallet = [0x02; 32];
    let transcript = kagemusha_wallet_unload_nullifier_transcript_v1(&scheme, &wallet, 0x0304);
    let mut expected = scheme.to_vec();
    expected.extend_from_slice(&wallet);
    expected.extend_from_slice(&0x0304_u128.to_le_bytes());
    assert_eq!(transcript, expected);
    assert_eq!(
        kagemusha_wallet_unload_nullifier_v1(&scheme, &wallet, 0x0304),
        kagemusha_wallet_digest_v1(Role::UnloadNullifier, &expected)
    );
    assert_ne!(
        kagemusha_wallet_unload_nullifier_v1(&scheme, &wallet, 0),
        kagemusha_wallet_unload_nullifier_v1(&scheme, &wallet, 1)
    );
}

#[test]
fn kagemusha_wallet_v1_statement_layout_and_validation() {
    let f = android();
    let statement = bootstrap_statement(&f);
    statement.validate().expect("bootstrap");
    let mut expected = 1_u16.to_le_bytes().to_vec();
    expected.extend_from_slice(&statement.scheme_id);
    expected.extend_from_slice(&statement.relation_id);
    expected.extend_from_slice(&statement.credential_digest);
    expected.extend_from_slice(&statement.asset_digest);
    expected.push(1);
    expected.extend_from_slice(&0_u128.to_le_bytes());
    expected.extend_from_slice(&0_u128.to_le_bytes());
    expected.extend_from_slice(&[0; 64]);
    expected.extend_from_slice(&commitment(1).transcript());
    expected.extend_from_slice(&statement.effect.transcript());
    assert_eq!(statement.transcript(), expected);
    assert_eq!(
        statement.statement_digest(),
        kagemusha_wallet_digest_v1(Role::Statement, &expected)
    );

    for effect in sample_effects(&f) {
        statement_for(&f, effect)
            .validate()
            .expect("sample statement");
    }

    let reject = |mutate: &dyn Fn(&mut KagemushaWalletStatementV1), field: &str| {
        let mut statement = bootstrap_statement(&f);
        mutate(&mut statement);
        assert!(is_invalid(statement.validate(), field), "{field}");
    };
    reject(&|s| s.sequence = 1, "statement.bootstrap");
    reject(&|s| s.predecessor = commitment(2), "statement.bootstrap");
    reject(
        &|s| s.lifecycle = KagemushaWalletLifecycleV1::Retiring,
        "statement.bootstrap",
    );
    reject(&|s| s.next_load = 1, "statement.bootstrap");
    reject(&|s| s.successor.ep = [0; 32], "statement.successor");
    reject(&|s| s.relation_id = [0; 32], "statement.relation_id");
    let mut version = bootstrap_statement(&f);
    version.version = 2;
    assert!(matches!(
        version.validate(),
        Err(KagemushaWalletValidationErrorV1::UnsupportedVersion { .. })
    ));

    let send = |sequence, predecessor| KagemushaWalletStatementV1 {
        sequence,
        predecessor,
        ..transition_statement(&f, 3, 0, KagemushaWalletLifecycleV1::Active, send_effect())
    };
    assert!(is_invalid(
        send(0, commitment(3)).validate(),
        "statement.sequence"
    ));
    assert!(is_invalid(
        send(3, KagemushaWalletStateCommitmentV1::ZERO).validate(),
        "statement.predecessor"
    ));
    let retiring = transition_statement(
        &f,
        3,
        0,
        KagemushaWalletLifecycleV1::Active,
        KagemushaWalletEffectV1::Retiring,
    );
    assert!(is_invalid(retiring.validate(), "statement.lifecycle"));
    let load = |next_load, load_ordinal| {
        transition_statement(
            &f,
            3,
            next_load,
            KagemushaWalletLifecycleV1::Retiring,
            KagemushaWalletEffectV1::Load {
                voucher: [1; 32],
                load_ordinal,
                amount: 1,
                online_charge: 0,
            },
        )
    };
    load(3, 2).validate().expect("retiring load");
    assert!(is_invalid(load(2, 2).validate(), "statement.next_load"));
    assert!(matches!(
        load(0, u128::MAX).validate(),
        Err(KagemushaWalletValidationErrorV1::ArithmeticOverflow { .. })
    ));
    let replacement = |update| {
        transition_statement(
            &f,
            3,
            0,
            KagemushaWalletLifecycleV1::Active,
            KagemushaWalletEffectV1::RefreshPolicy {
                update_kind: KagemushaWalletPolicyUpdateKindV1::Credential,
                update,
                accepted_time_floor_ms: 0,
            },
        )
    };
    replacement(f.credential.credential_digest())
        .validate()
        .expect("replacement statement names its credential");
    assert!(is_invalid(replacement([9; 32]).validate(), "effect.update"));
}

#[test]
fn kagemusha_wallet_v1_statement_scheme_and_credential_bindings() {
    let f = android();
    let statement = bootstrap_statement(&f);
    statement.validate_for_scheme(&f.scheme).expect("scheme");
    statement
        .validate_for_credential(&f.credential)
        .expect("credential");
    let mut other_relation = statement;
    other_relation.relation_id = [0x99; 32];
    assert!(matches!(
        other_relation.validate_for_scheme(&f.scheme),
        Err(KagemushaWalletValidationErrorV1::SchemeMismatch {
            field: "statement.relation_id"
        })
    ));
    let mut other_scheme = statement;
    other_scheme.scheme_id = [0x98; 32];
    assert!(matches!(
        other_scheme.validate_for_scheme(&f.scheme),
        Err(KagemushaWalletValidationErrorV1::SchemeMismatch { .. })
    ));
    assert!(matches!(
        other_scheme.validate_for_credential(&f.credential),
        Err(KagemushaWalletValidationErrorV1::SchemeMismatch { .. })
    ));

    let other = identity_fixture(KagemushaWalletEvidenceKindV1::AppleAppAttest, 0x48);
    assert!(is_invalid(
        statement.validate_for_credential(&other.credential),
        "statement.credential_digest"
    ));
    let mut other_asset = statement;
    other_asset.asset_digest = [0x97; 32];
    assert!(is_invalid(
        other_asset.validate_for_credential(&f.credential),
        "statement.asset_digest"
    ));
    let mut other_enrollment = statement;
    other_enrollment.effect = KagemushaWalletEffectV1::Bootstrap {
        enrollment_id: [0x96; 32],
        enrollment_marker: MARKER,
    };
    assert!(is_invalid(
        other_enrollment.validate_for_credential(&f.credential),
        "effect.enrollment_id"
    ));

    let wallet_id = f.credential.body.wallet_id;
    let mut to_self = send_effect();
    if let KagemushaWalletEffectV1::Send {
        receiver_wallet_id, ..
    } = &mut to_self
    {
        *receiver_wallet_id = wallet_id;
    }
    assert!(is_invalid(
        statement_for(&f, to_self).validate_for_credential(&f.credential),
        "effect.receiver_wallet_id"
    ));
    let from_self = KagemushaWalletEffectV1::Receive {
        credit_id: [1; 32],
        payer_wallet_id: wallet_id,
        payment: [1; 32],
        amount: 1,
    };
    assert!(is_invalid(
        statement_for(&f, from_self).validate_for_credential(&f.credential),
        "effect.payer_wallet_id"
    ));
    let wrong_nullifier = KagemushaWalletEffectV1::Unload {
        nullifier: kagemusha_wallet_unload_nullifier_v1(
            &f.credential.body.scheme_id,
            &wallet_id,
            3,
        ),
        redeem_ordinal: 2,
        amount: 1,
        online_charge: 0,
        charge_quote: [0; 32],
    };
    assert!(is_invalid(
        statement_for(&f, wrong_nullifier).validate_for_credential(&f.credential),
        "effect.nullifier"
    ));
    for effect in sample_effects(&f) {
        statement_for(&f, effect)
            .validate_for_credential(&f.credential)
            .expect("sample statement binds the credential");
    }
}

#[test]
fn kagemusha_wallet_v1_statement_successor_rules() {
    let f = android();
    let bootstrap = bootstrap_statement(&f);
    let load = |ordinal: u128, lifecycle| KagemushaWalletStatementV1 {
        predecessor: commitment(1),
        ..transition_statement(
            &f,
            1,
            ordinal + 1,
            lifecycle,
            KagemushaWalletEffectV1::Load {
                voucher: [0x62; 32],
                load_ordinal: ordinal,
                amount: 1,
                online_charge: 0,
            },
        )
    };
    let first = load(0, KagemushaWalletLifecycleV1::Active);
    first.validate_successor_of(&bootstrap).expect("first load");
    assert!(is_invalid(
        load(1, KagemushaWalletLifecycleV1::Active).validate_successor_of(&bootstrap),
        "statement.next_load"
    ));
    assert!(is_invalid(
        load(0, KagemushaWalletLifecycleV1::Retiring).validate_successor_of(&bootstrap),
        "statement.lifecycle"
    ));
    let mut unchained = first;
    unchained.predecessor = commitment(9);
    assert!(is_invalid(
        unchained.validate_successor_of(&bootstrap),
        "statement.predecessor"
    ));
    let mut skipped = first;
    skipped.sequence = 2;
    assert!(is_invalid(
        skipped.validate_successor_of(&bootstrap),
        "statement.sequence"
    ));
    let mut other_relation = first;
    other_relation.relation_id = [0x55; 32];
    assert!(matches!(
        other_relation.validate_successor_of(&bootstrap),
        Err(KagemushaWalletValidationErrorV1::SchemeMismatch {
            field: "statement.relation_id"
        })
    ));

    let retiring = KagemushaWalletStatementV1 {
        predecessor: commitment(2),
        ..transition_statement(
            &f,
            2,
            1,
            KagemushaWalletLifecycleV1::Retiring,
            KagemushaWalletEffectV1::Retiring,
        )
    };
    retiring.validate_successor_of(&first).expect("retire");
    let retiring_again = KagemushaWalletStatementV1 {
        predecessor: commitment(3),
        ..transition_statement(
            &f,
            3,
            1,
            KagemushaWalletLifecycleV1::Retiring,
            KagemushaWalletEffectV1::Retiring,
        )
    };
    assert!(is_invalid(
        retiring_again.validate_successor_of(&retiring),
        "statement.lifecycle"
    ));
    let reverted = KagemushaWalletStatementV1 {
        predecessor: commitment(3),
        ..transition_statement(&f, 3, 1, KagemushaWalletLifecycleV1::Active, send_effect())
    };
    assert!(is_invalid(
        reverted.validate_successor_of(&retiring),
        "statement.lifecycle"
    ));
    let mut late_send = reverted;
    late_send.lifecycle = KagemushaWalletLifecycleV1::Retiring;
    late_send
        .validate_successor_of(&retiring)
        .expect("a retiring wallet still sends");
    let mut stale_load = late_send;
    stale_load.next_load = 0;
    assert!(is_invalid(
        stale_load.validate_successor_of(&retiring),
        "statement.next_load"
    ));

    let mut other_credential = late_send;
    other_credential.credential_digest = [0x44; 32];
    assert!(is_invalid(
        other_credential.validate_successor_of(&retiring),
        "statement.credential_digest"
    ));
    let replacement = KagemushaWalletStatementV1 {
        credential_digest: [0x44; 32],
        predecessor: commitment(3),
        ..transition_statement(
            &f,
            3,
            1,
            KagemushaWalletLifecycleV1::Retiring,
            KagemushaWalletEffectV1::RefreshPolicy {
                update_kind: KagemushaWalletPolicyUpdateKindV1::Credential,
                update: [0x44; 32],
                accepted_time_floor_ms: 0,
            },
        )
    };
    replacement
        .validate_successor_of(&retiring)
        .expect("replacement credential changes the statement credential");
}

#[test]
fn kagemusha_wallet_v1_proof_bounds_and_digest() {
    let proof = stand_in_proof(3);
    assert_eq!(
        proof.proof_digest(),
        kagemusha_wallet_digest_v1(Role::Proof, &[0, 1, 2])
    );
    for (len, transition_ok, status_ok) in [
        (0, false, false),
        (1, true, true),
        (
            KAGEMUSHA_WALLET_CREDIT_STATUS_PROOF_MAX_BYTES_V1,
            true,
            true,
        ),
        (
            KAGEMUSHA_WALLET_CREDIT_STATUS_PROOF_MAX_BYTES_V1 + 1,
            true,
            false,
        ),
        (KAGEMUSHA_WALLET_PROOF_MAX_BYTES_V1, true, false),
        (KAGEMUSHA_WALLET_PROOF_MAX_BYTES_V1 + 1, false, false),
    ] {
        let proof = stand_in_proof(len);
        assert_eq!(proof.validate().is_ok(), transition_ok, "{len}");
        assert_eq!(proof.validate_credit_status().is_ok(), status_ok, "{len}");
    }
}

#[test]
fn kagemusha_wallet_v1_receipt_sign_verify_and_bindings() {
    let f = android();
    let statement = statement_for(&f, send_effect());
    let proof = stand_in_proof(64);
    let body = KagemushaWalletReceiptBodyV1::derive(&f.credential, &statement, &proof, CAPSULE)
        .expect("receipt body");
    let mut expected = 1_u16.to_le_bytes().to_vec();
    expected.extend_from_slice(&f.credential.body.scheme_id);
    expected.extend_from_slice(&f.credential.body.wallet_id);
    expected.extend_from_slice(&f.credential.body.provider_contract);
    expected.extend_from_slice(&3_u128.to_le_bytes());
    expected.extend_from_slice(&statement.operation_id(&f.credential.body.wallet_id));
    expected.extend_from_slice(&commitment(3).transcript());
    expected.extend_from_slice(&commitment(4).transcript());
    expected.extend_from_slice(&statement.statement_digest());
    expected.extend_from_slice(&proof.proof_digest());
    expected.extend_from_slice(&CAPSULE);
    assert_eq!(body.transcript(), expected);
    assert_eq!(
        body.body_digest(),
        kagemusha_wallet_digest_v1(Role::ReceiptBody, &expected)
    );
    assert_eq!(
        body.signing_message(),
        kagemusha_wallet_preimage_v1(Role::ReceiptBody, &expected)
    );

    let receipt = KagemushaWalletReceiptV1::sign(
        &f.credential,
        &statement,
        &proof,
        CAPSULE,
        raw_output(&f.payment, &body.signing_message()),
    )
    .expect("receipt");
    assert_eq!(receipt.operation_id, body.operation_id);
    let digest = receipt
        .verify(&f.credential, &statement, &proof)
        .expect("verify");
    assert_eq!(
        digest,
        kagemusha_wallet_signed_object_digest_v1(
            Role::Receipt,
            &body.body_digest(),
            &receipt.signature
        )
    );
    assert_eq!(
        receipt
            .body(&f.credential, &statement, &proof)
            .expect("body"),
        body
    );

    // Another key, another proof, another statement or another operation identity fail.
    let wrong_key = KagemushaWalletReceiptV1::sign(
        &f.credential,
        &statement,
        &proof,
        CAPSULE,
        raw_output(&signing_key(0x5e), &body.signing_message()),
    );
    assert!(matches!(
        wrong_key,
        Err(KagemushaWalletValidationErrorV1::InvalidSignature {
            role: Role::ReceiptBody
        })
    ));
    assert!(matches!(
        receipt.verify(&f.credential, &statement, &stand_in_proof(65)),
        Err(KagemushaWalletValidationErrorV1::InvalidSignature { .. })
    ));
    let mut later = statement;
    later.next_load = 1;
    assert!(matches!(
        receipt.verify(&f.credential, &later, &proof),
        Err(KagemushaWalletValidationErrorV1::InvalidSignature { .. })
    ));
    let mut other_operation = receipt;
    other_operation.operation_id = [0x31; 32];
    assert!(is_invalid(
        other_operation.verify(&f.credential, &statement, &proof),
        "receipt.operation_id"
    ));
    let mut other_capsule = receipt;
    other_capsule.capsule_digest = [0x32; 32];
    assert!(matches!(
        other_capsule.verify(&f.credential, &statement, &proof),
        Err(KagemushaWalletValidationErrorV1::InvalidSignature { .. })
    ));
    let mut zero_capsule = receipt;
    zero_capsule.capsule_digest = [0; 32];
    assert!(is_invalid(
        zero_capsule.validate(),
        "receipt.capsule_digest"
    ));
    assert!(is_invalid(
        KagemushaWalletReceiptBodyV1::derive(&f.credential, &statement, &proof, [0; 32]),
        "receipt.capsule_digest"
    ));
    let mut version = receipt;
    version.version = 0;
    assert!(matches!(
        version.verify(&f.credential, &statement, &proof),
        Err(KagemushaWalletValidationErrorV1::UnsupportedVersion { .. })
    ));
}

#[test]
fn kagemusha_wallet_v1_package_verify_digest_and_flips() {
    let f = android();
    let statement = statement_for(&f, send_effect());
    let package = signed_package(&f, &f.credential, &statement, stand_in_proof(96));
    let digests = package.verify(&f.credential).expect("verify");
    assert_eq!(digests.statement, statement.statement_digest());
    assert_eq!(digests.proof, package.proof.proof_digest());
    let mut transcript = digests.statement.to_vec();
    transcript.extend_from_slice(&digests.proof);
    transcript.extend_from_slice(&digests.receipt);
    assert_eq!(
        digests.package,
        kagemusha_wallet_digest_v1(Role::Package, &transcript)
    );
    assert_eq!(
        kagemusha_wallet_package_digest_v1(&digests.statement, &digests.proof, &digests.receipt),
        digests.package
    );
    assert_eq!(
        package.package_digest(&f.credential).expect("digest"),
        digests.package
    );

    let frame = norito::encode_canonical(&package).expect("encode");
    let decoded: KagemushaWalletPackageV1 =
        decode_frame_v1(&frame, KAGEMUSHA_WALLET_ACTIVATION_MAX_BYTES_V1).expect("decode");
    assert_eq!(decoded, package);
    assert_every_flip_rejected_or_rebound(&frame, digests.package, |bytes| {
        let package: KagemushaWalletPackageV1 =
            decode_frame_v1(bytes, KAGEMUSHA_WALLET_ACTIVATION_MAX_BYTES_V1).ok()?;
        package.package_digest(&f.credential).ok()
    });

    let other = identity_fixture(KagemushaWalletEvidenceKindV1::AndroidKeyMintStrongBox, 0x49);
    assert!(is_invalid(
        package.verify(&other.credential),
        "statement.credential_digest"
    ));
    let mut empty_proof = package.clone();
    empty_proof.proof = stand_in_proof(0);
    assert!(is_invalid(empty_proof.verify(&f.credential), "proof.bytes"));
    let mut version = package;
    version.version = 2;
    assert!(matches!(
        version.verify(&f.credential),
        Err(KagemushaWalletValidationErrorV1::UnsupportedVersion {
            field: "package.version",
            ..
        })
    ));
}

#[test]
fn kagemusha_wallet_v1_replacement_credential_package() {
    let f = android();
    let mut body = f.credential.body;
    body.renewal_sequence = 1;
    body.issued_at_ms += 60_000;
    body.fresh_evidence.time_ms = body.issued_at_ms - 500;
    let replacement = f.issue(&body).expect("replacement");
    replacement
        .validate_replacement_of(&f.credential)
        .expect("replacement rules");
    let statement = KagemushaWalletStatementV1 {
        credential_digest: replacement.credential_digest(),
        ..statement_for(
            &f,
            KagemushaWalletEffectV1::RefreshPolicy {
                update_kind: KagemushaWalletPolicyUpdateKindV1::Credential,
                update: replacement.credential_digest(),
                accepted_time_floor_ms: body.issued_at_ms,
            },
        )
    };
    // The receipt is signed by the unchanged payment key and verified against the new credential.
    let package = signed_package(&f, &replacement, &statement, stand_in_proof(32));
    package.verify(&replacement).expect("replacement package");
    assert!(is_invalid(
        package.verify(&f.credential),
        "statement.credential_digest"
    ));
}

#[test]
fn kagemusha_wallet_v1_package_size_with_maximum_proof() {
    let f = android();
    let package = signed_package(
        &f,
        &f.credential,
        &statement_for(&f, send_effect()),
        stand_in_proof(KAGEMUSHA_WALLET_PROOF_MAX_BYTES_V1),
    );
    let frame = norito::encode_canonical(&package).expect("encode");
    let overhead = frame.len() - KAGEMUSHA_WALLET_PROOF_MAX_BYTES_V1;
    println!(
        "KAGEMUSHA wallet V1 Send package with a {}-byte proof: {} bytes (overhead {overhead})",
        KAGEMUSHA_WALLET_PROOF_MAX_BYTES_V1,
        frame.len()
    );
    assert!(frame.len() < KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1);
    assert!(overhead < 1_024, "package overhead {overhead}");
}

#[test]
fn kagemusha_wallet_v1_bootstrap_state_and_policy() {
    let f = android();
    let state =
        KagemushaWalletStateV1::bootstrap(&f.credential, &EMPTY_ROOTS, [0x5c; 32]).expect("state");
    assert_eq!(state.version, KAGEMUSHA_WALLET_VERSION_V1);
    assert_eq!(state.wallet_id, f.credential.body.wallet_id);
    assert_eq!(state.credential_digest, f.credential.credential_digest());
    assert_eq!(state.lifecycle, KagemushaWalletLifecycleV1::Active);
    assert_eq!(
        (
            state.balance,
            state.sequence,
            state.next_send,
            state.next_load,
            state.next_redeem
        ),
        (0, 0, 0, 0, 0)
    );
    assert_eq!(state.consumed_credit_root, EMPTY_ROOTS.consumed_credit);
    assert_eq!(state.fee_claim_root, EMPTY_ROOTS.fee_claim);
    let policy = state.policy;
    assert_eq!(
        policy.regulatory_policy,
        f.credential.body.regulatory_policy
    );
    assert_eq!(policy.quota_usage_root, EMPTY_ROOTS.quota_usage);
    assert_eq!(policy.quota_windows_root, [0; 32]);
    assert_eq!(policy.lease_expires_at_ms, 0);
    assert_eq!(policy.accepted_time_floor_ms, 0);
    assert_eq!(policy.enabled_controls, 0);
    assert!(!policy.send_requires_time_anchor());
    state
        .validate_for_credential(&f.credential)
        .expect("credential");

    let frame = norito::encode_canonical(&state).expect("encode");
    let decoded: KagemushaWalletStateV1 = decode_frame_v1(&frame, 4_096).expect("decode");
    assert_eq!(decoded, state);

    let mut zero_root = EMPTY_ROOTS;
    zero_root.load_recovery = [0; 32];
    assert!(is_invalid(
        KagemushaWalletStateV1::bootstrap(&f.credential, &zero_root, [1; 32]),
        "empty_roots.load_recovery"
    ));
    assert!(is_invalid(
        KagemushaWalletStateV1::bootstrap(&f.credential, &EMPTY_ROOTS, [0; 32]),
        "state.state_nonce"
    ));
    let other = identity_fixture(KagemushaWalletEvidenceKindV1::AppleAppAttest, 0x4a);
    assert!(state.validate_for_credential(&other.credential).is_err());
    let mut other_lease = state;
    other_lease.policy.lease_expires_at_ms = 5;
    assert!(other_lease.validate_for_credential(&f.credential).is_err());

    let reject = |mutate: &dyn Fn(&mut KagemushaWalletPolicyStateV1), field: &str| {
        let mut policy = state.policy;
        mutate(&mut policy);
        assert!(is_invalid(policy.validate(), field), "{field}");
    };
    reject(
        &|p| p.enabled_controls = KAGEMUSHA_WALLET_CONTROL_QUOTAS_V1,
        "policy.enabled_controls",
    );
    reject(&|p| p.policy_epoch = 1, "policy.scheme_policy");
    reject(&|p| p.scheme_policy = [1; 32], "policy.scheme_policy");
    reject(&|p| p.fee_schedule = [1; 32], "policy.scheme_policy");
    reject(&|p| p.blacklist_version = 1, "policy.blacklist");
    reject(&|p| p.blacklist_issued_at_ms = 1, "policy.blacklist");
    reject(
        &|p| {
            p.blacklist_version = 1;
            p.blacklist = [1; 32];
        },
        "policy.blacklist",
    );
    reject(&|p| p.quota_share_id = 1, "policy.quota_share");
    reject(&|p| p.quota_windows_root = [1; 32], "policy.quota_share");
    reject(&|p| p.quota_usage_root = [0; 32], "policy.quota_usage_root");
    reject(&|p| p.lease_expires_at_ms = 1, "policy.lease_expires_at_ms");
    let mut held = state.policy;
    held.policy_epoch = 1;
    held.scheme_policy = [1; 32];
    held.fee_schedule = [2; 32];
    held.blacklist_version = 3;
    held.blacklist = [3; 32];
    held.blacklist_root = [4; 32];
    held.quota_share_id = 5;
    held.quota_share = [5; 32];
    held.quota_windows_root = [6; 32];
    held.validate().expect("held policy objects");
    assert!(!held.is_active(0));
}

#[test]
fn kagemusha_wallet_v1_map_leaf_limbs_and_domains() {
    let mut digest = [0_u8; 32];
    for (index, byte) in digest.iter_mut().enumerate() {
        *byte = u8::try_from(index).expect("byte");
    }
    let [low, high] = kagemusha_wallet_digest_limbs_v1(&digest);
    assert_eq!(
        low,
        u128::from_le_bytes(digest[..16].try_into().expect("half"))
    );
    assert_eq!(
        high,
        u128::from_le_bytes(digest[16..].try_into().expect("half"))
    );
    assert_eq!(low & 0xff, 0);
    assert_eq!(high >> 120, 31);

    let limbs = |value: &[u8; 32]| kagemusha_wallet_digest_limbs_v1(value);
    let consumed = KagemushaWalletConsumedCreditLeafV1 {
        credit_id: [1; 32],
        payment_digest: [2; 32],
    };
    assert_eq!(
        consumed.limbs(),
        [limbs(&[1; 32]), limbs(&[2; 32])].concat()[..]
    );
    let pending = KagemushaWalletPendingOutgoingLeafV1 {
        credit_id: [1; 32],
        receiver_wallet_id: [2; 32],
        send_ordinal: 3,
        amount: 4,
        fee: 5,
        request_digest: [6; 32],
    };
    assert_eq!(
        pending.limbs(),
        [
            &limbs(&[1; 32])[..],
            &limbs(&[2; 32])[..],
            &[3, 4, 5],
            &limbs(&[6; 32])[..]
        ]
        .concat()[..]
    );
    let load = KagemushaWalletLoadLeafV1 {
        ordinal: 7,
        voucher_digest: [8; 32],
        amount: 9,
    };
    assert_eq!(
        load.limbs(),
        [&[7][..], &limbs(&[8; 32])[..], &[9]].concat()[..]
    );
    let redeem = KagemushaWalletRedeemLeafV1 {
        ordinal: 1,
        nullifier: [2; 32],
        amount: 3,
        online_charge: 4,
    };
    assert_eq!(
        redeem.limbs(),
        [&[1][..], &limbs(&[2; 32])[..], &[3, 4]].concat()[..]
    );
    let fee = KagemushaWalletFeeClaimLeafV1 {
        credit_id: [1; 32],
        fee: 2,
        fee_schedule_digest: [3; 32],
    };
    assert_eq!(
        fee.limbs(),
        [&limbs(&[1; 32])[..], &[2], &limbs(&[3; 32])[..]].concat()[..]
    );
    let usage = KagemushaWalletQuotaUsageLeafV1 {
        window_kind: KagemushaWalletQuotaWindowKindV1::Monthly,
        window_start_ms: 10,
        window_end_ms: 20,
        used: 30,
    };
    assert_eq!(usage.limbs(), [2, 10, 20, 30]);

    let domains = [
        KagemushaWalletConsumedCreditLeafV1::DOMAIN,
        KagemushaWalletPendingOutgoingLeafV1::DOMAIN,
        KagemushaWalletLoadLeafV1::DOMAIN,
        KagemushaWalletRedeemLeafV1::DOMAIN,
        KagemushaWalletFeeClaimLeafV1::DOMAIN,
        KagemushaWalletQuotaUsageLeafV1::DOMAIN,
    ];
    assert_eq!(
        domains,
        [
            KAGEMUSHA_WALLET_CONSUMED_CREDIT_LEAF_DOMAIN_V1,
            KAGEMUSHA_WALLET_PENDING_OUTGOING_LEAF_DOMAIN_V1,
            KAGEMUSHA_WALLET_LOAD_RECOVERY_LEAF_DOMAIN_V1,
            KAGEMUSHA_WALLET_REDEEM_RECOVERY_LEAF_DOMAIN_V1,
            KAGEMUSHA_WALLET_FEE_CLAIM_LEAF_DOMAIN_V1,
            KAGEMUSHA_WALLET_QUOTA_USAGE_LEAF_DOMAIN_V1,
        ]
    );
    // Existing iroha_core_zk Poseidon domains the wallet maps must not reuse.
    let existing: Vec<u64> = [
        b"kgmemp_1",
        b"kgmleaf1",
        b"kgmnode1",
        b"kgmstate",
        b"kgmmntl1",
        b"kgminte1",
        b"kgmmntn1",
        b"kgmhpln1",
        b"kgmhmsg0",
        b"kgmhmsg1",
        b"kgmhseed",
        b"kgmhjob1",
        b"kgmhpc20",
        b"kgmhpc21",
        b"kgmrlc_2",
        b"kgmbnd_1",
        b"kgmeqn_1",
        b"kgminp_1",
        b"kgmsrc_1",
        b"kgpcrlc1",
        b"cfownr03",
        b"cfnote03",
        b"cfnull03",
        b"cfleaf03",
        b"cfnode03",
        b"cfasst03",
        b"cfnet_03",
    ]
    .iter()
    .map(|tag| u64::from_le_bytes(**tag))
    .collect();
    let mut seen = std::collections::BTreeSet::new();
    for domain in domains {
        assert!(seen.insert(domain), "duplicate wallet domain {domain:#x}");
        assert!(!existing.contains(&domain), "reused domain {domain:#x}");
    }
}

#[test]
fn kagemusha_wallet_v1_state_objects_round_trip() {
    let f = android();
    for effect in sample_effects(&f) {
        let statement = statement_for(&f, effect);
        let frame = norito::encode_canonical(&statement).expect("encode");
        let decoded: KagemushaWalletStatementV1 = decode_frame_v1(&frame, 4_096).expect("decode");
        assert_eq!(decoded, statement);
        assert_eq!(decoded.statement_digest(), statement.statement_digest());
    }
    let leaf = KagemushaWalletPendingOutgoingLeafV1 {
        credit_id: [1; 32],
        receiver_wallet_id: [2; 32],
        send_ordinal: u128::MAX,
        amount: 4,
        fee: 5,
        request_digest: [6; 32],
    };
    let frame = norito::encode_canonical(&leaf).expect("encode");
    let decoded: KagemushaWalletPendingOutgoingLeafV1 =
        decode_frame_v1(&frame, 1_024).expect("decode");
    assert_eq!(decoded, leaf);
    let proof = stand_in_proof(17);
    let frame = norito::encode_canonical(&proof).expect("encode");
    let decoded: KagemushaWalletProofV1 = decode_frame_v1(&frame, 1_024).expect("decode");
    assert_eq!(decoded, proof);
    assert!(commitment(5).is_complete());
    assert!(!KagemushaWalletStateCommitmentV1::ZERO.is_complete());
    assert!(KagemushaWalletStateCommitmentV1::default().is_zero());
    assert_eq!(
        KagemushaWalletEmptyMapRootsV1::validate(&EMPTY_ROOTS).ok(),
        Some(())
    );
}
