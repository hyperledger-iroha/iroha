//! Original signed X.509 custody controls; malformed bytes never stand in for a valid proof.

use super::*;
use crate::executor::attachment_custody::AttachmentCustody;
use iroha_data_model::{
    Level,
    isi::{InstructionBox, Log},
    privacy::{PrivacyProtocolIdV1, PrivacyStatementV1, PrivacyTransactionIntentDigestV1},
    transaction::{Executable, FeePaymentIntent, TransactionBuilder, signed_attachments::*},
};
use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR, BOB_ID};

pub(crate) fn request(network: NetworkId, wallet: AccountId) -> SubmitPrivacyZkX509CertificateV1 {
    let bundle = iroha_data_model::privacy::privacy_exact12_fixture_bundle_v1().unwrap();
    let row = bundle
        .rows
        .iter()
        .find(|row| row.protocol_id == PrivacyProtocolIdV1::IrohaZkX509StarkP256V1)
        .unwrap();
    let PrivacyStatementV1::IrohaZkX509StarkP256V1(mut statement) =
        norito::decode_canonical(&row.statement_norito).unwrap()
    else {
        panic!("typed X.509 row")
    };
    statement.context.network_id = network;
    statement.context.action_index = 0;
    statement.context.transaction_intent_digest = PrivacyTransactionIntentDigestV1::new([0; 32]);
    statement.wallet_account = wallet;
    SubmitPrivacyZkX509CertificateV1::new(
        statement,
        ActionAttachmentRefV1 {
            action_index: 0,
            slot_index: 0,
        },
    )
}

pub(crate) fn planned(
    request: &SubmitPrivacyZkX509CertificateV1,
    invocation: ActionInvocationBindingV1,
    authority: ActionAuthorityBindingV1,
) -> PlannedActionV1 {
    PlannedActionV1 {
        operation: ActionOperationV1::NativeInstruction(
            SubmitPrivacyZkX509CertificateV1::WIRE_ID.into(),
        ),
        role: ActionRoleV1::ProofEffect,
        authority,
        invocation,
        public_inputs_digest: request.public_inputs_digest_v1().unwrap(),
        // Deliberately malformed native proof: only custody/negative tests use this fixture.
        attachments: vec![PlannedAttachmentV1 {
            kind: AttachmentKindV1::Proof,
            role: AttachmentRoleV1::ActionProof,
            bytes: vec![0xab; 128],
        }],
    }
}

fn source() -> (SignedTransaction, SubmitPrivacyZkX509CertificateV1) {
    let network = NetworkId::from_genesis_hash(iroha_crypto::HashOf::<
        iroha_data_model::block::BlockHeader,
    >::from_untyped_unchecked(Hash::new(
        b"x509-custody",
    )));
    let request = request(network, ALICE_ID.clone());
    let mut plan = SignedAttachmentPlanV1::new();
    plan.push_action(planned(
        &request,
        ActionInvocationBindingV1::Direct {
            instruction_index: 1,
        },
        ActionAuthorityBindingV1::TransactionAuthority,
    ));
    let executable = Executable::Instructions(
        vec![
            InstructionBox::from(Log::new(Level::INFO, "mixed ordinary action".into())),
            request.clone().into(),
        ]
        .into(),
    );
    let source = TransactionBuilder::new(
        network,
        ALICE_ID.clone(),
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_executable(executable)
    .with_attachment_plan(plan)
    .unwrap()
    .sign(ALICE_KEYPAIR.private_key());
    (source, request)
}

#[test]
fn original_mixed_action_binds_privacy_ordinal_without_copying_or_accepting_proof() {
    let (source, request) = source();
    let mut custody = AttachmentCustody::new(&source).unwrap();
    let budget = AllocationBudget::new(1024 * 1024 * 1024);
    let original = &source.attachments().unwrap().occurrences()[0].bytes;
    let binding = custody
        .direct(1, &ALICE_ID, |frame| {
            frame.with_x509_action(
                &request,
                &PrivacyConsensusLimitsV1::taira_default(),
                10_485_760,
                &budget,
                |binding, bytes| {
                    assert!(core::ptr::eq(bytes.as_ptr(), original.as_ptr()));
                    assert_eq!(
                        binding.statement().context.action_index,
                        0,
                        "privacy ordinal is not executable index"
                    );
                    assert_eq!(
                        binding
                            .statement()
                            .context
                            .transaction_intent_digest
                            .as_bytes(),
                        source
                            .attachments()
                            .unwrap()
                            .action(0)
                            .unwrap()
                            .statement_digest
                            .as_bytes()
                    );
                    assert!(budget.reserved_bytes() > 0);
                    Ok(binding)
                },
            )
        })
        .unwrap();
    custody.finish().unwrap();
    assert!(
        budget.reserved_bytes() > 0,
        "materialized owner retains original-pool funding"
    );
    drop(binding);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn substituted_source_scope_authority_and_inputs_poison_the_original_attempt() {
    for case in 0..4 {
        let (source, mut request) = source();
        let mut custody = AttachmentCustody::new(&source).unwrap();
        let budget = AllocationBudget::new(1024 * 1024 * 1024);
        if case == 2 {
            request.statement.context.action_index = 1;
        }
        if case == 3 {
            request.proof_ref.slot_index = 1;
        }
        let result = custody.direct(
            if case == 0 { 0 } else { 1 },
            if case == 1 { &BOB_ID } else { &ALICE_ID },
            |frame| {
                frame.with_x509_action(
                    &request,
                    &PrivacyConsensusLimitsV1::taira_default(),
                    10_485_760,
                    &budget,
                    |_, _| -> Result<(), X509Error> {
                        panic!("substitution reached native consumer")
                    },
                )
            },
        );
        assert!(result.is_err());
        assert_eq!(custody.finish(), Err(CustodyError::FailedAttempt));
        assert_eq!(budget.reserved_bytes(), 0);
    }
}

#[test]
fn capacity_native_failure_unwind_and_repeat_never_restore_consumption() {
    for case in 0..4 {
        let (source, request) = source();
        let mut custody = AttachmentCustody::new(&source).unwrap();
        let budget = AllocationBudget::new(if case == 0 { 1 } else { 1024 * 1024 * 1024 });
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            custody.direct(1, &ALICE_ID, |frame| {
                frame.with_x509_action(
                    &request,
                    &PrivacyConsensusLimitsV1::taira_default(),
                    10_485_760,
                    &budget,
                    |_, _| {
                        if case == 2 {
                            panic!("injected consumer unwind");
                        }
                        if case == 1 {
                            return Err(X509Error::ProofShape);
                        }
                        Ok(())
                    },
                )?;
                frame.with_x509_action(
                    &request,
                    &PrivacyConsensusLimitsV1::taira_default(),
                    10_485_760,
                    &budget,
                    |_, _| -> Result<(), X509Error> {
                        panic!("same occurrence was consumed twice")
                    },
                )
            })
        }));
        match case {
            0 => assert!(matches!(result, Ok(Err(X509Error::Capacity(_))))),
            2 => assert!(result.is_err()),
            _ => assert!(result.unwrap().is_err()),
        }
        assert_eq!(custody.finish(), Err(CustodyError::FailedAttempt));
        assert_eq!(
            budget.reserved_bytes(),
            0,
            "failure and unwind refund only after owned data drops"
        );
    }
}

#[test]
fn whole_source_workspace_is_admitted_before_original_intent_normalization() {
    let (source, request) = source();
    let mut custody = AttachmentCustody::new(&source).unwrap();
    // Fits the bounded request graph, but not the independently governed full
    // transaction normalization/hash workspace. The original pool must refuse.
    let budget = AllocationBudget::new(32 * 1024 * 1024);
    let result = custody.direct(1, &ALICE_ID, |frame| {
        frame.with_x509_action(
            &request,
            &PrivacyConsensusLimitsV1::taira_default(),
            10_485_760,
            &budget,
            |_, _| -> Result<(), X509Error> { panic!("source workspace was not admitted") },
        )
    });
    assert!(matches!(result, Err(X509Error::Capacity(_))));
    assert_eq!(budget.reserved_bytes(), 0);
    assert_eq!(custody.finish(), Err(CustodyError::FailedAttempt));
}
