//! Provider marker, output descriptor, recovery capsule and completion record tests.

use super::*;
use crate::kagemusha::kagemusha_wallet_v1::{
    KagemushaWalletEvidenceKindV1, KagemushaWalletLifecycleV1, KagemushaWalletReceiptBodyV1,
    KagemushaWalletValidationErrorV1,
    codec_tests::norito_tag,
    identity::identity_tests::{IdentityFixture, identity_fixture, raw_output},
    kagemusha_wallet_unload_nullifier_v1,
    messages::messages_tests::message_fixture,
    state::state_tests::{
        EMPTY_ROOTS, bootstrap_statement, commitment, send_effect, stand_in_proof,
        transition_statement,
    },
};

/// One mutation of a recovery capsule.
type CapsuleMutation = fn(&mut KagemushaWalletRecoveryCapsuleV1);

fn android() -> IdentityFixture {
    identity_fixture(KagemushaWalletEvidenceKindV1::AndroidKeyMintTee, 0x65)
}

#[track_caller]
fn assert_invalid<T: core::fmt::Debug>(result: WalletResult<T>, expected: &str) {
    match result {
        Err(KagemushaWalletValidationErrorV1::InvalidField { field }) if field == expected => {}
        other => panic!("expected invalid `{expected}`, got {other:?}"),
    }
}

fn enrollment_marker(f: &IdentityFixture) -> KagemushaWalletMarkerV1 {
    KagemushaWalletMarkerV1::enrollment(&f.challenge, f.credential.body.payment_key)
        .expect("enrollment marker")
}

/// Recovery capsule of `statement` and `proof` under `credential`, with a successor state that
/// agrees with the statement.
fn capsule_for(
    credential: &KagemushaWalletCredentialV1,
    statement: &KagemushaWalletStatementV1,
    proof: KagemushaWalletProofV1,
    predecessor_capsule_digest: [u8; 32],
    payment_certificates: Option<&KagemushaWalletCertificateSetV1>,
) -> KagemushaWalletRecoveryCapsuleV1 {
    let mut state =
        KagemushaWalletStateV1::bootstrap(credential, &EMPTY_ROOTS, [0x5d; 32]).expect("state");
    state.sequence = statement.sequence;
    state.next_load = statement.next_load;
    state.lifecycle = statement.lifecycle;
    let output =
        KagemushaWalletOutputDescriptorV1::for_transition(statement, &proof, payment_certificates)
            .expect("output");
    KagemushaWalletRecoveryCapsuleV1 {
        version: KAGEMUSHA_WALLET_VERSION_V1,
        scheme_id: credential.body.scheme_id,
        wallet_id: credential.body.wallet_id,
        operation_id: statement.operation_id(&credential.body.wallet_id),
        kind: statement.effect.kind(),
        predecessor_capsule_digest,
        successor_state: state,
        statement: *statement,
        proof,
        map_openings: vec![vec![0x01, 0x02, 0x03]],
        retained_inputs: vec![KagemushaWalletRetainedInputV1 {
            role: KagemushaWalletRetainedInputRoleV1::Request,
            bytes: vec![0xaa; 16],
        }],
        output,
    }
}

/// Receipt over `capsule`'s digest and the released package.
fn complete(
    f: &IdentityFixture,
    credential: &KagemushaWalletCredentialV1,
    capsule: &KagemushaWalletRecoveryCapsuleV1,
) -> (KagemushaWalletReceiptV1, KagemushaWalletPackageV1) {
    let digest = capsule.capsule_digest().expect("capsule digest");
    let body = KagemushaWalletReceiptBodyV1::derive(
        credential,
        &capsule.statement,
        &capsule.proof,
        digest,
    )
    .expect("receipt body");
    let receipt = KagemushaWalletReceiptV1::sign(
        credential,
        &capsule.statement,
        &capsule.proof,
        digest,
        raw_output(&f.payment, &body.signing_message()),
    )
    .expect("receipt");
    let package = KagemushaWalletPackageV1::new(capsule.statement, capsule.proof.clone(), receipt);
    (receipt, package)
}

/// Bootstrap capsule bound to `f`'s generation-0 enrollment marker.
fn bootstrap_capsule(f: &IdentityFixture) -> KagemushaWalletRecoveryCapsuleV1 {
    let statement = KagemushaWalletStatementV1 {
        effect: enrollment_marker(f)
            .bootstrap_effect()
            .expect("bootstrap effect"),
        ..bootstrap_statement(f)
    };
    capsule_for(&f.credential, &statement, stand_in_proof(48), [0; 32], None)
}

#[test]
fn kagemusha_wallet_v1_custody_layouts_and_tags() {
    assert_eq!(KAGEMUSHA_WALLET_OUTPUT_KIND_INPUT_BYTES_V1, 96);
    assert_eq!(KAGEMUSHA_WALLET_OUTPUT_TRANSCRIPT_BYTES_V1, 161);
    let transcript = kagemusha_wallet_output_transcript_v1(
        KagemushaWalletOperationKindV1::Unload,
        &[1; 32],
        &[2; 32],
        &[3; 96],
    );
    assert_eq!(
        transcript.len(),
        KAGEMUSHA_WALLET_OUTPUT_TRANSCRIPT_BYTES_V1
    );
    assert_eq!(transcript[0], 6);
    assert_eq!(&transcript[1..33], &[1; 32]);
    assert_eq!(&transcript[33..65], &[2; 32]);
    assert_eq!(&transcript[65..], &[3; 96]);
    assert_eq!(
        kagemusha_wallet_output_digest_v1(
            KagemushaWalletOperationKindV1::Unload,
            &[1; 32],
            &[2; 32],
            &[3; 96]
        ),
        kagemusha_wallet_digest_v1(Role::Output, &transcript)
    );
    for reason in KagemushaWalletTerminalReasonV1::ALL {
        assert_eq!(norito_tag(&reason), u32::from(reason.tag()));
    }
    for role in KagemushaWalletRetainedInputRoleV1::ALL {
        assert_eq!(norito_tag(&role), u32::from(role.tag()));
    }
    let states = [
        KagemushaWalletMarkerStateV1::Enrollment {
            challenge_digest: [1; 32],
            enrollment_id: [2; 32],
        },
        KagemushaWalletMarkerStateV1::Head {
            sequence: 0,
            operation_id: [3; 32],
            head: commitment(1),
            capsule_digest: [4; 32],
            predecessor_capsule_digest: [0; 32],
        },
        KagemushaWalletMarkerStateV1::Terminal {
            reason: KagemushaWalletTerminalReasonV1::Abandoned,
            last_capsule_digest: [0; 32],
        },
    ];
    for (index, state) in states.iter().enumerate() {
        assert_eq!(usize::from(state.tag()), index + 1);
        assert_eq!(norito_tag(state), u32::from(state.tag()));
    }
}

#[test]
fn kagemusha_wallet_v1_enrollment_marker_and_bootstrap_effect() {
    let f = android();
    let marker = enrollment_marker(&f);
    marker.validate().expect("marker");
    assert_eq!(marker.generation, 0);
    assert_eq!(marker.wallet_id, f.credential.body.wallet_id);
    let frame = marker.to_canonical_bytes().expect("frame");
    assert!(frame.len() <= KAGEMUSHA_WALLET_MARKER_MAX_BYTES_V1);
    let digest = marker.marker_digest().expect("digest");
    assert_eq!(digest, kagemusha_wallet_digest_v1(Role::Marker, &frame));
    assert_eq!(
        marker.bootstrap_effect().expect("effect"),
        KagemushaWalletEffectV1::Bootstrap {
            enrollment_id: f.credential.body.enrollment_id,
            enrollment_marker: digest,
        }
    );
    // The Bootstrap statement binding this marker runs under the credential.
    bootstrap_capsule(&f)
        .statement
        .validate_for_credential(&f.credential)
        .expect("bootstrap statement");

    assert_eq!(
        KagemushaWalletMarkerV1::decode_canonical(&frame, &f.credential.body.scheme_id)
            .expect("decode"),
        marker
    );
    assert!(matches!(
        KagemushaWalletMarkerV1::decode_canonical(&frame, &[0x40; 32]),
        Err(KagemushaWalletValidationErrorV1::SchemeMismatch {
            field: "marker.scheme_id"
        })
    ));
    assert!(matches!(
        KagemushaWalletMarkerV1::decode_canonical(
            &vec![0; KAGEMUSHA_WALLET_MARKER_MAX_BYTES_V1 + 1],
            &f.credential.body.scheme_id
        ),
        Err(KagemushaWalletValidationErrorV1::EncodedSizeExceeded { .. })
    ));
    let version = KagemushaWalletMarkerV1 {
        version: 2,
        ..marker
    };
    assert!(matches!(
        KagemushaWalletMarkerV1::decode_canonical(
            &norito::encode_canonical(&version).expect("encode"),
            &f.credential.body.scheme_id
        ),
        Err(KagemushaWalletValidationErrorV1::UnsupportedVersion { .. })
    ));

    let KagemushaWalletMarkerStateV1::Enrollment {
        challenge_digest, ..
    } = marker.state
    else {
        panic!("enrollment state");
    };
    let generation = KagemushaWalletMarkerV1 {
        generation: 1,
        ..marker
    };
    assert_invalid(generation.validate(), "marker.generation");
    let enrollment_id = KagemushaWalletMarkerV1 {
        state: KagemushaWalletMarkerStateV1::Enrollment {
            challenge_digest,
            enrollment_id: [0x41; 32],
        },
        ..marker
    };
    assert_invalid(enrollment_id.validate(), "marker.enrollment_id");
    let wallet = KagemushaWalletMarkerV1 {
        wallet_id: [0x42; 32],
        ..marker
    };
    assert_invalid(wallet.validate(), "marker.wallet_id");
    let head = marker
        .successor(bootstrap_capsule(&f).head_marker_state().expect("head"))
        .expect("head marker");
    assert_invalid(head.bootstrap_effect(), "marker.state");
}

#[test]
fn kagemusha_wallet_v1_marker_generations_follow_the_provider_contract() {
    let f = android();
    let enrollment = enrollment_marker(&f);
    let bootstrap = bootstrap_capsule(&f);
    let head0 = enrollment
        .successor(bootstrap.head_marker_state().expect("head"))
        .expect("bootstrap head");
    head0
        .require_capsule(&bootstrap)
        .expect("bootstrap capsule");
    let bootstrap_digest = bootstrap.capsule_digest().expect("digest");
    let next = capsule_for(
        &f.credential,
        &transition_statement(&f, 1, 0, KagemushaWalletLifecycleV1::Active, send_effect()),
        stand_in_proof(32),
        bootstrap_digest,
        Some(&KagemushaWalletCertificateSetV1::default()),
    );
    let head1 = head0
        .successor(next.head_marker_state().expect("head"))
        .expect("next head");
    head1.require_capsule(&next).expect("next capsule");
    assert_invalid(head1.require_capsule(&bootstrap), "marker.capsule");
    let next_digest = next.capsule_digest().expect("digest");
    let deleted = head1
        .successor(KagemushaWalletMarkerStateV1::Terminal {
            reason: KagemushaWalletTerminalReasonV1::CustodyDeleted,
            last_capsule_digest: next_digest,
        })
        .expect("custody deleted");
    let abandoned = enrollment
        .successor(KagemushaWalletMarkerStateV1::Terminal {
            reason: KagemushaWalletTerminalReasonV1::Abandoned,
            last_capsule_digest: [0; 32],
        })
        .expect("abandoned");

    // Disallowed transitions.
    assert_invalid(
        enrollment.successor(next.head_marker_state().expect("head")),
        "marker.state",
    );
    assert_invalid(
        head0.successor(KagemushaWalletMarkerStateV1::Head {
            sequence: 1,
            operation_id: next.operation_id,
            head: next.statement.successor,
            capsule_digest: next_digest,
            predecessor_capsule_digest: [0x43; 32],
        }),
        "marker.state",
    );
    assert_invalid(
        head1.successor(KagemushaWalletMarkerStateV1::Terminal {
            reason: KagemushaWalletTerminalReasonV1::Abandoned,
            last_capsule_digest: [0; 32],
        }),
        "marker.state",
    );
    assert_invalid(
        head1.successor(KagemushaWalletMarkerStateV1::Terminal {
            reason: KagemushaWalletTerminalReasonV1::CustodyDeleted,
            last_capsule_digest: bootstrap_digest,
        }),
        "marker.state",
    );
    assert_invalid(
        enrollment.successor(KagemushaWalletMarkerStateV1::Terminal {
            reason: KagemushaWalletTerminalReasonV1::CustodyDeleted,
            last_capsule_digest: [0x44; 32],
        }),
        "marker.state",
    );
    for terminal in [deleted, abandoned] {
        assert_invalid(
            terminal.successor(KagemushaWalletMarkerStateV1::Terminal {
                reason: KagemushaWalletTerminalReasonV1::CustodyDeleted,
                last_capsule_digest: [0x45; 32],
            }),
            "marker.state",
        );
    }
    let skipped = KagemushaWalletMarkerV1 {
        generation: 3,
        ..head1
    };
    assert_invalid(skipped.validate_successor_of(&head0), "marker.generation");
    let mut other_key = head1;
    other_key.payment_key = identity_fixture(KagemushaWalletEvidenceKindV1::AppleAppAttest, 0x66)
        .credential
        .body
        .payment_key;
    assert_invalid(other_key.validate_successor_of(&head0), "marker.identity");

    // Self-contained head and terminal rules.
    let mut zero_generation = head0;
    zero_generation.generation = 0;
    assert_invalid(zero_generation.validate(), "marker.generation");
    let mut unlinked = head1;
    unlinked.state = KagemushaWalletMarkerStateV1::Head {
        sequence: 1,
        operation_id: next.operation_id,
        head: next.statement.successor,
        capsule_digest: next_digest,
        predecessor_capsule_digest: [0; 32],
    };
    assert_invalid(unlinked.validate(), "marker.predecessor_capsule_digest");
    let mut incomplete = head1;
    incomplete.state = KagemushaWalletMarkerStateV1::Head {
        sequence: 1,
        operation_id: next.operation_id,
        head: KagemushaWalletStateCommitmentV1 {
            eq: [0; 32],
            ep: [1; 32],
        },
        capsule_digest: next_digest,
        predecessor_capsule_digest: bootstrap_digest,
    };
    assert_invalid(incomplete.validate(), "marker.head");
    let mut abandoned_with_capsule = abandoned;
    abandoned_with_capsule.state = KagemushaWalletMarkerStateV1::Terminal {
        reason: KagemushaWalletTerminalReasonV1::Abandoned,
        last_capsule_digest: [0x46; 32],
    };
    assert_invalid(
        abandoned_with_capsule.validate(),
        "marker.last_capsule_digest",
    );
}

#[test]
fn kagemusha_wallet_v1_output_descriptors_are_receipt_free() {
    let f = android();
    let proof = stand_in_proof(40);
    let body = &f.credential.body;
    let nullifier = kagemusha_wallet_unload_nullifier_v1(&body.scheme_id, &body.wallet_id, 0);
    let effects = [
        (
            KagemushaWalletEffectV1::Load {
                voucher: [0x51; 32],
                load_ordinal: 0,
                amount: 10,
                online_charge: 0,
            },
            [0x51; 32],
        ),
        (
            KagemushaWalletEffectV1::Receive {
                credit_id: [0x52; 32],
                payer_wallet_id: [0x53; 32],
                payment: [0x54; 32],
                amount: 3,
            },
            [0x54; 32],
        ),
        (
            KagemushaWalletEffectV1::ArchiveSent {
                credit_id: [0x55; 32],
                credited: [0x56; 32],
            },
            [0x56; 32],
        ),
        (
            KagemushaWalletEffectV1::Unload {
                nullifier,
                redeem_ordinal: 0,
                amount: 5,
                online_charge: 0,
                charge_quote: [0; 32],
            },
            nullifier,
        ),
        (KagemushaWalletEffectV1::Retiring, [0; 32]),
    ];
    for (effect, head) in effects {
        let lifecycle = if effect == KagemushaWalletEffectV1::Retiring {
            KagemushaWalletLifecycleV1::Retiring
        } else {
            KagemushaWalletLifecycleV1::Active
        };
        let next_load = match effect {
            KagemushaWalletEffectV1::Load { load_ordinal, .. } => load_ordinal + 1,
            _ => 0,
        };
        let statement = transition_statement(&f, 3, next_load, lifecycle, effect);
        let descriptor =
            KagemushaWalletOutputDescriptorV1::for_transition(&statement, &proof, None)
                .expect("descriptor");
        let mut input = [0; KAGEMUSHA_WALLET_OUTPUT_KIND_INPUT_BYTES_V1];
        input[..32].copy_from_slice(&head);
        assert_eq!(descriptor.kind, effect.kind());
        assert_eq!(
            descriptor.digest,
            kagemusha_wallet_output_digest_v1(
                effect.kind(),
                &statement.statement_digest(),
                &proof.proof_digest(),
                &input,
            )
        );
        assert_invalid(
            KagemushaWalletOutputDescriptorV1::for_transition(
                &statement,
                &proof,
                Some(&KagemushaWalletCertificateSetV1::default()),
            ),
            "output.certificates",
        );
    }

    let m = message_fixture();
    let payment = m.payment(true, 40);
    let statement = &payment.send.statement;
    let descriptor = KagemushaWalletOutputDescriptorV1::for_transition(
        statement,
        &payment.send.proof,
        Some(&payment.certificates),
    )
    .expect("send descriptor");
    let mut input = Vec::new();
    input.extend_from_slice(&payment.request.request_digest());
    input.extend_from_slice(&m.payer.credential.credential_digest());
    input.extend_from_slice(&payment.certificates.digest().expect("set"));
    assert_eq!(
        descriptor.digest,
        kagemusha_wallet_output_digest_v1(
            KagemushaWalletOperationKindV1::Send,
            &statement.statement_digest(),
            &payment.send.proof.proof_digest(),
            &input.try_into().expect("96 bytes"),
        )
    );
    assert_invalid(
        KagemushaWalletOutputDescriptorV1::for_transition(statement, &payment.send.proof, None),
        "output.certificates",
    );
    assert_invalid(
        KagemushaWalletOutputDescriptorV1 {
            kind: KagemushaWalletOperationKindV1::Send,
            digest: [0; 32],
        }
        .validate(),
        "output.digest",
    );
}

#[test]
fn kagemusha_wallet_v1_recovery_capsule_rules() {
    let f = android();
    let capsule = capsule_for(
        &f.credential,
        &transition_statement(
            &f,
            3,
            0,
            KagemushaWalletLifecycleV1::Active,
            KagemushaWalletEffectV1::Receive {
                credit_id: [0x61; 32],
                payer_wallet_id: [0x62; 32],
                payment: [0x63; 32],
                amount: 9,
            },
        ),
        stand_in_proof(64),
        [0x64; 32],
        None,
    );
    capsule.validate().expect("capsule");
    let frame = capsule.to_canonical_bytes().expect("frame");
    assert!(frame.len() <= KAGEMUSHA_WALLET_CAPSULE_MAX_BYTES_V1);
    assert_eq!(
        capsule.capsule_digest().expect("digest"),
        kagemusha_wallet_digest_v1(Role::Capsule, &frame)
    );
    assert_eq!(
        KagemushaWalletRecoveryCapsuleV1::decode_canonical(&frame, &f.credential.body.scheme_id)
            .expect("decode"),
        capsule
    );
    assert!(matches!(
        KagemushaWalletRecoveryCapsuleV1::decode_canonical(&frame, &[0x65; 32]),
        Err(KagemushaWalletValidationErrorV1::SchemeMismatch { .. })
    ));
    assert_eq!(
        capsule.head_marker_state().expect("head"),
        KagemushaWalletMarkerStateV1::Head {
            sequence: 3,
            operation_id: capsule.operation_id,
            head: capsule.statement.successor,
            capsule_digest: capsule.capsule_digest().expect("digest"),
            predecessor_capsule_digest: [0x64; 32],
        }
    );
    bootstrap_capsule(&f).validate().expect("bootstrap capsule");

    let mutations: [(CapsuleMutation, &str); 13] = [
        (
            |capsule| capsule.kind = KagemushaWalletOperationKindV1::Load,
            "capsule.kind",
        ),
        (
            |capsule| capsule.output.kind = KagemushaWalletOperationKindV1::Load,
            "capsule.output.kind",
        ),
        (
            |capsule| capsule.operation_id = [0x66; 32],
            "capsule.operation_id",
        ),
        (
            |capsule| capsule.successor_state.wallet_id = [0x67; 32],
            "capsule.successor_state.wallet_id",
        ),
        (
            |capsule| capsule.successor_state.credential_digest = [0x68; 32],
            "capsule.successor_state.credential_digest",
        ),
        (
            |capsule| capsule.successor_state.asset_digest = [0x69; 32],
            "capsule.successor_state.asset_digest",
        ),
        (
            |capsule| capsule.successor_state.lifecycle = KagemushaWalletLifecycleV1::Retiring,
            "capsule.successor_state.lifecycle",
        ),
        (
            |capsule| capsule.successor_state.sequence += 1,
            "capsule.successor_state.sequence",
        ),
        (
            |capsule| capsule.successor_state.next_load += 1,
            "capsule.successor_state.next_load",
        ),
        (
            |capsule| capsule.predecessor_capsule_digest = [0; 32],
            "capsule.predecessor_capsule_digest",
        ),
        (
            |capsule| capsule.output.digest = [0x6a; 32],
            "capsule.output.digest",
        ),
        (
            |capsule| capsule.map_openings.push(Vec::new()),
            "capsule.map_openings",
        ),
        (
            |capsule| capsule.retained_inputs[0].bytes.clear(),
            "capsule.retained_input",
        ),
    ];
    for (mutate, field) in mutations {
        let mut mutated = capsule.clone();
        mutate(&mut mutated);
        assert_invalid(mutated.validate(), field);
    }
    let mut bootstrap = bootstrap_capsule(&f);
    bootstrap.predecessor_capsule_digest = [0x6b; 32];
    assert_invalid(bootstrap.validate(), "capsule.predecessor_capsule_digest");
    let mut empty_proof = capsule.clone();
    empty_proof.proof = stand_in_proof(0);
    assert_invalid(empty_proof.validate(), "proof.bytes");
    let mut version = capsule;
    version.version = 2;
    assert!(matches!(
        version.validate(),
        Err(KagemushaWalletValidationErrorV1::UnsupportedVersion { .. })
    ));
}

#[test]
fn kagemusha_wallet_v1_completion_record_rebuilds_the_output() {
    // A non-Send transition releases its package.
    let f = android();
    let capsule = bootstrap_capsule(&f);
    let (receipt, package) = complete(&f, &f.credential, &capsule);
    let output = norito::encode_canonical(&package).expect("package frame");
    let record =
        KagemushaWalletCompletionRecordV1::new(&capsule, receipt, output.clone()).expect("record");
    assert_eq!(
        record.verify(&f.credential, &capsule).expect("verify"),
        package.verify(&f.credential).expect("package")
    );
    let frame = record.to_canonical_bytes().expect("frame");
    assert!(frame.len() <= KAGEMUSHA_WALLET_COMPLETION_RECORD_MAX_BYTES_V1);
    assert_eq!(
        record.completion_digest().expect("digest"),
        kagemusha_wallet_digest_v1(Role::Completion, &frame)
    );
    assert_eq!(
        KagemushaWalletCompletionRecordV1::decode_canonical(&frame, &f.credential.body.wallet_id)
            .expect("decode"),
        record
    );
    assert_invalid(
        KagemushaWalletCompletionRecordV1::decode_canonical(&frame, &[0x71; 32]),
        "completion.wallet_id",
    );

    // A Send releases its canonical Payment.
    let m = message_fixture();
    let request = m.request(true);
    let template = m.payment(true, 48);
    let send = capsule_for(
        &m.payer.credential,
        &template.send.statement,
        template.send.proof.clone(),
        [0x72; 32],
        Some(&template.certificates),
    );
    let (send_receipt, send_package) = complete(&m.payer, &m.payer.credential, &send);
    let payment = KagemushaWalletPaymentV1::assemble(
        request,
        m.payer.credential,
        &m.payer.enrollment_certificate,
        send_package,
    )
    .expect("payment");
    let payment_bytes = payment.to_canonical_bytes().expect("payment frame");
    let send_record =
        KagemushaWalletCompletionRecordV1::new(&send, send_receipt, payment_bytes.clone())
            .expect("send record");
    send_record
        .verify(&m.payer.credential, &send)
        .expect("send record verifies");
    // The Send output must be a valid Payment by this payer.
    assert!(
        KagemushaWalletCompletionRecordV1 {
            output: output.clone(),
            ..send_record.clone()
        }
        .verify(&m.payer.credential, &send)
        .is_err()
    );
    let mut other_payer = m.payer.credential;
    other_payer.body.issued_at_ms += 1;
    assert!(send_record.verify(&other_payer, &send).is_err());

    // Record and capsule bindings.
    let mut other_receipt = record.clone();
    other_receipt.receipt.capsule_digest = [0x73; 32];
    assert_invalid(
        other_receipt.validate(),
        "completion.receipt.capsule_digest",
    );
    let mut other_operation = record.clone();
    other_operation.receipt.operation_id = [0x74; 32];
    assert_invalid(
        other_operation.validate(),
        "completion.receipt.operation_id",
    );
    let mut empty = record.clone();
    empty.output.clear();
    assert_invalid(empty.validate(), "completion.output");
    let mut oversized = record.clone();
    oversized.output = vec![0; KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1 + 1];
    assert_invalid(oversized.validate(), "completion.output");
    let next = capsule_for(
        &f.credential,
        &transition_statement(&f, 1, 0, KagemushaWalletLifecycleV1::Active, send_effect()),
        stand_in_proof(16),
        capsule.capsule_digest().expect("digest"),
        Some(&KagemushaWalletCertificateSetV1::default()),
    );
    assert_invalid(
        record.verify(&f.credential, &next),
        "completion.capsule_digest",
    );
    // An output whose embedded receipt differs from the record's.
    let mut retargeted = package.clone();
    retargeted.receipt = send_receipt;
    let retargeted_record = KagemushaWalletCompletionRecordV1 {
        output: norito::encode_canonical(&retargeted).expect("frame"),
        ..record.clone()
    };
    assert_invalid(
        retargeted_record.verify(&f.credential, &capsule),
        "completion.receipt",
    );
    // An output of another statement under the same receipt.
    let mut other_statement = package.clone();
    other_statement.statement.successor = commitment(9);
    let other_statement_record = KagemushaWalletCompletionRecordV1 {
        output: norito::encode_canonical(&other_statement).expect("frame"),
        ..record.clone()
    };
    assert_invalid(
        other_statement_record.verify(&f.credential, &capsule),
        "completion.output",
    );
    // A capsule whose descriptor names another output.
    let mut redirected = capsule.clone();
    redirected.output.digest = [0x75; 32];
    assert!(record.verify(&f.credential, &redirected).is_err());
    let mut version = record;
    version.version = 2;
    assert!(matches!(
        version.validate(),
        Err(KagemushaWalletValidationErrorV1::UnsupportedVersion { .. })
    ));
}
