// Dispatch and transport-boundary tests for every Exact12 action role.
// Included in `privacy::tests`.
//
// The role samples are structurally valid envelopes with placeholder proofs and a canonical
// profile that admits them structurally. Those profiles do not match compiled native profiles.
// Every test ends in a rejection and checks which part of the one handler produced it and
// that no privacy State was written. X.509 requires its canonical signed-attachment instruction,
// so the generic envelope entry rejects it before role resolution.

use iroha_core_privacy::{
    privacy_dispatch::{
        PRIVACY_ROLE_DISPATCH_TABLE_V1, PrivacyRoleVerificationV1, PrivacyUnsupportedOperationV1,
    },
    privacy_profiles::{
        CompiledPrivacyProfileValidationErrorV1, validate_compiled_privacy_profile_v1,
    },
    privacy_verifier::PrivacyEffectRoleMismatchFailureV1,
};
use iroha_data_model::{
    isi::InstructionBox,
    privacy::{PrivacyActionRoleSampleV1, privacy_action_role_samples_v1},
};

fn action_role_samples() -> Vec<PrivacyActionRoleSampleV1> {
    privacy_action_role_samples_v1().expect("Exact12 role samples")
}

const X509_ATTACHMENT_REQUIRED: &str =
    "X.509 presentations require the canonical signed-attachment instruction";

/// Check the exact error class and rejection boundary for a placeholder on blank State.
fn role_sample_rejection_message(role: PrivacyActionRoleV1, error: &Error) -> &str {
    use PrivacyActionRoleV1 as Role;
    let expected = match role {
        Role::ZkAceAuthorization => "ZK-ACE authoritative policy is not registered",
        Role::AnonymousPgcPayment => "trusted Anonymous PGC pool state failed validation",
        Role::ZkAmsBatchAdmission | Role::ZkAmsProvisionAccount => {
            "trusted ZK-AMS registry state failed validation"
        }
        Role::ZkX509CertificatePresentation => X509_ATTACHMENT_REQUIRED,
        Role::BootleLanternPresentation => {
            "Bootle/Lantern authoritative issuer policy is not registered"
        }
        Role::OrchardActions => "trusted Orchard pool state failed validation",
        Role::FcmpTransfer | Role::IvmPrivateNoteTransition | Role::PqMaspTransition => {
            "trusted proof-managed pool state failed validation"
        }
        Role::VeRangeRange | Role::VegaCredentialPresentation | Role::JindoBatchedEvaluation => {
            "privacy proof admission rejected"
        }
    };
    let message: &str = match (role, error) {
        (
            Role::AnonymousPgcPayment
            | Role::ZkAmsBatchAdmission
            | Role::ZkAmsProvisionAccount
            | Role::OrchardActions
            | Role::FcmpTransfer
            | Role::IvmPrivateNoteTransition
            | Role::PqMaspTransition,
            Error::InvariantViolation(message),
        ) => message,
        (
            Role::ZkAceAuthorization
            | Role::VeRangeRange
            | Role::VegaCredentialPresentation
            | Role::ZkX509CertificatePresentation
            | Role::JindoBatchedEvaluation
            | Role::BootleLanternPresentation,
            Error::InvalidParameter(InvalidParameterError::SmartContract(message)),
        ) => message,
        _ => panic!("{role}: unexpected role rejection class: {error:?}"),
    };
    assert!(message.starts_with(expected), "{role}: {message}");
    message
}

/// Direct structural-profile dispatch must retain the verifier's exact profile rejection.
fn structural_role_sample_rejection_message<'a>(
    sample: &PrivacyActionRoleSampleV1,
    error: &'a Error,
) -> &'a str {
    use CompiledPrivacyProfileValidationErrorV1 as ProfileError;
    let expected = match sample.role {
        PrivacyActionRoleV1::VeRangeRange | PrivacyActionRoleV1::JindoBatchedEvaluation => {
            ProfileError::ParameterIdMismatch
        }
        PrivacyActionRoleV1::VegaCredentialPresentation => {
            ProfileError::Profile(CompiledPrivacyProfileErrorV1::EngineUnavailable {
                protocol_id: sample.role.protocol_id(),
            })
        }
        _ => return role_sample_rejection_message(sample.role, error),
    };
    assert_eq!(
        validate_compiled_privacy_profile_v1(&sample.profile),
        Err(expected),
        "{}: the structural profile has this exact native mismatch",
        sample.role
    );
    let Error::InvariantViolation(message) = error else {
        panic!(
            "{}: unexpected structural-profile rejection class: {error:?}",
            sample.role
        );
    };
    let message: &str = message;
    assert_eq!(
        message,
        format!(
            "privacy proof admission rejected: privacy profile does not match the compiled native profile: {expected}"
        ),
        "{}",
        sample.role
    );
    message
}

/// A blank State on the network the role samples bind, with one committed block hash.
fn action_role_sample_state(samples: &[PrivacyActionRoleSampleV1]) -> State {
    let network_id = samples[0].envelope.statement.context().network_id;
    assert!(
        samples
            .iter()
            .all(|sample| sample.envelope.statement.context().network_id == network_id),
        "every role sample binds one network"
    );
    let alice = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
    let world = World::with([], [alice], []);
    let mut state = State::new_with_chain_and_network_id_for_testing(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
        TEST_CHAIN_ID.into(),
        network_id,
    );
    install_synthetic_privacy_genesis_hash(&mut state, *network_id.as_bytes());
    state
}

/// Run the whole handler body under the supplied canonical profile. Only the selected admission lookup of
/// `Execute` is replaced by the supplied structural profile; intent binding, budget preflight, role
/// resolution, trusted-state loading, the verifier dispatch and the effect executor are the
/// production code.
fn execute_action_role_sample(
    instruction: SubmitPrivacyProofV1,
    profile: PrivacyCompiledProfileSnapshotV1,
    transaction: &mut StateTransaction<'_, '_>,
) -> Result<(), Error> {
    bind_submit_privacy_instruction(transaction, &instruction);
    let submission = PreparedPrivacySubmissionV1::prepare(instruction, &ALICE_ID, transaction)?;
    submission.execute_after_admission(&ALICE_ID, transaction, profile, profile.protocol_limits)
}

/// Every privacy map is empty, no account was added and no action budget is charged.
fn assert_no_privacy_state(transaction: &StateTransaction<'_, '_>, case: &str) {
    assert_no_privacy_state_with_account_count(transaction, case, 1);
}

/// No privacy write, account registration or budget charge changes the original fixture.
fn assert_no_privacy_state_with_account_count(
    transaction: &StateTransaction<'_, '_>,
    case: &str,
    original_accounts: usize,
) {
    assert_eq!(privacy_map_counts(transaction), (0, 0, 0, 0), "{case}");
    assert_eq!(
        transaction.world.privacy_nullifiers.iter().count(),
        0,
        "{case}"
    );
    assert_eq!(
        transaction.world.privacy_commitments.iter().count(),
        0,
        "{case}"
    );
    assert_eq!(
        transaction.world.accounts.iter().count(),
        original_accounts,
        "{case}"
    );
    assert_eq!(
        transaction.privacy_budget_for_testing(),
        (0, 0, 0, 0),
        "{case}"
    );
}

/// The envelope of `own` with the proof, the statement or the protocol identity of `other`.
fn substituted_envelopes(
    own: &PrivacyActionRoleSampleV1,
    other: &PrivacyActionRoleSampleV1,
) -> Vec<(&'static str, PrivacyProofEnvelopeV1)> {
    let mut substituted = Vec::new();
    let mut foreign_proof = own.envelope.clone();
    foreign_proof.proof = other.envelope.proof.clone();
    substituted.push(("proof", foreign_proof));
    let mut foreign_statement = own.envelope.clone();
    foreign_statement.statement = other.envelope.statement.clone();
    foreign_statement.statement_digest = other.envelope.statement_digest;
    substituted.push(("statement", foreign_statement));
    if own.role.protocol_id() != other.role.protocol_id() {
        let mut relabelled = own.envelope.clone();
        relabelled.protocol_id = other.envelope.protocol_id;
        relabelled.proof_system_id = other.envelope.proof_system_id;
        relabelled.engine_id = other.envelope.engine_id;
        substituted.push(("protocol", relabelled));
    }
    substituted
}

/// An envelope whose protocol, statement and proof name different roles is refused by the
/// single role resolver, or by the earlier mandatory X.509 attachment boundary, before
/// trusted state of either role is read and whatever canonical profile is offered.
#[test]
fn handler_refuses_role_protocol_and_statement_substitution_for_every_pair_of_roles() {
    let samples = action_role_samples();
    let state = action_role_sample_state(&samples);
    for own in &samples {
        let mut block = state.block(test_header());
        let mut transaction = block.transaction();
        for other in &samples {
            if own.role == other.role {
                continue;
            }
            for (what, envelope) in substituted_envelopes(own, other) {
                for profile in [own.profile, other.profile] {
                    let case = format!("{} with the {what} of {}", own.role, other.role);
                    assert!(
                        envelope.action_role_v1().is_err(),
                        "{case}: substitution must not resolve to an action role"
                    );
                    let error = execute_action_role_sample(
                        SubmitPrivacyProofV1::new(envelope.clone()),
                        profile,
                        &mut transaction,
                    )
                    .expect_err("a substituted envelope is never executed");
                    let message = smart_contract_parameter_message(&error);
                    if envelope.protocol_id == PrivacyProtocolIdV1::IrohaZkX509StarkP256V1 {
                        assert_eq!(message, X509_ATTACHMENT_REQUIRED, "{case}");
                    } else {
                        assert!(
                            message.starts_with("privacy envelope does not select one action role"),
                            "{case}: {message}"
                        );
                    }
                    assert_no_privacy_state(&transaction, &case);
                }
            }
        }
    }
}

/// Every role reaches its own trusted-state requirement or exact native-profile rejection;
/// X.509 instead requires its canonical attachment entry before generic dispatch.
#[test]
fn every_role_enters_its_own_branch_of_the_single_handler() {
    let samples = action_role_samples();
    assert_eq!(
        samples.iter().map(|sample| sample.role).collect::<Vec<_>>(),
        PrivacyActionRoleV1::ALL
    );
    let state = action_role_sample_state(&samples);
    for sample in &samples {
        let mut block = state.block(test_header());
        let mut transaction = block.transaction();
        let error = execute_action_role_sample(
            SubmitPrivacyProofV1::new(sample.envelope.clone()),
            sample.profile,
            &mut transaction,
        )
        .expect_err("a placeholder proof on a blank State is never applied");
        structural_role_sample_rejection_message(sample, &error);
        assert_no_privacy_state(&transaction, sample.role.canonical_label());
    }
}

/// The typed instruction handler and the instruction registry are one entry: the boxed
/// instruction reaches the same handler and returns the same result for every role.
#[test]
fn typed_handler_and_registry_dispatch_are_one_entry_for_every_role() {
    let samples = action_role_samples();
    // The boxed route reads execution authority before entering the privacy handler.
    // Use the original signed genesis, with no optional protocol policy rows.
    let state = governance_execution_state();
    let original_accounts = state.view().world.accounts.iter().count();
    for sample in &samples {
        let instruction = SubmitPrivacyProofV1::new(sample.envelope.clone());
        let mut block = state.block(governance_execution_header(&state));
        let mut transaction = block.transaction();
        assert!(
            crate::executor::root_scope::execution_root_scope(&mut transaction).is_ok(),
            "both instruction routes start from the original authenticated execution root"
        );
        bind_submit_privacy_instruction(&mut transaction, &instruction);
        let typed = instruction
            .clone()
            .execute(&ALICE_ID, &mut transaction)
            .expect_err("placeholder proof or its actual selected implementation is rejected");
        assert_no_privacy_state_with_account_count(
            &transaction,
            sample.role.canonical_label(),
            original_accounts,
        );
        bind_submit_privacy_instruction(&mut transaction, &instruction);
        let boxed = InstructionBox::from(instruction)
            .execute(&ALICE_ID, &mut transaction)
            .expect_err("the registry reaches the same handler");
        assert_no_privacy_state_with_account_count(
            &transaction,
            sample.role.canonical_label(),
            original_accounts,
        );
        assert_eq!(
            format!("{typed:?}"),
            format!("{boxed:?}"),
            "{}",
            sample.role
        );
        let message = if sample.role == PrivacyActionRoleV1::ZkX509CertificatePresentation {
            // The generic envelope must not become an alternate X.509 transport,
            // even when the selected implementation is locally unavailable.
            let message = role_sample_rejection_message(sample.role, &typed);
            assert_eq!(message, X509_ATTACHMENT_REQUIRED);
            message
        } else {
            match compiled_privacy_profile_v1(sample.role.protocol_id()) {
                Ok(_) => role_sample_rejection_message(sample.role, &typed),
                Err(unavailable) => {
                    let message = smart_contract_parameter_message(&typed);
                    assert!(message.contains(&unavailable.to_string()), "{message}");
                    message
                }
            }
        };
        assert!(
            !message.contains("qualification") && !message.contains("activation"),
            "{message}"
        );
    }
}

/// A role the dispatch registry does not verify is a typed parameter rejection that names the
/// role and its limitation; an effect of another role is an internal invariant violation.
#[test]
fn unsupported_operation_and_foreign_effect_map_to_distinct_instruction_errors() {
    let mut refused = Vec::new();
    for row in PRIVACY_ROLE_DISPATCH_TABLE_V1 {
        let PrivacyRoleVerificationV1::Unsupported(limitation) = row.verification else {
            continue;
        };
        let error = privacy_verification_error(PrivacyVerificationErrorV1::UnsupportedOperation(
            Box::new(PrivacyUnsupportedOperationV1::new(row.role, limitation)),
        ));
        let crate::execution_attempt::ExecutionAttemptError::Rejected(error) = error else {
            panic!("unsupported operation must remain a completed rejection");
        };
        let message = smart_contract_parameter_message(&error);
        assert!(
            message.starts_with("privacy proof admission rejected"),
            "{message}"
        );
        assert!(message.contains("is unsupported"), "{message}");
        assert!(message.contains(row.role.canonical_label()), "{message}");
        assert!(message.contains(limitation.description()), "{message}");
        refused.push(row.role);
    }
    assert_eq!(
        refused,
        [
            PrivacyActionRoleV1::ZkAceAuthorization,
            PrivacyActionRoleV1::ZkAmsBatchAdmission,
            PrivacyActionRoleV1::ZkAmsProvisionAccount,
            PrivacyActionRoleV1::VegaCredentialPresentation,
            PrivacyActionRoleV1::ZkX509CertificatePresentation,
        ]
    );
    for role in PrivacyActionRoleV1::ALL {
        let error = privacy_verification_error(PrivacyVerificationErrorV1::EffectRoleMismatch(
            Box::new(PrivacyEffectRoleMismatchFailureV1 { role }),
        ));
        let crate::execution_attempt::ExecutionAttemptError::Rejected(Error::InvariantViolation(
            message,
        )) = &error
        else {
            panic!("{role}: a foreign effect must be an invariant violation, got {error:?}");
        };
        assert!(message.contains(role.canonical_label()), "{message}");
        let error = role_statement_invariant_v1(role);
        let Error::InvariantViolation(message) = &error else {
            panic!("{role}: expected an invariant violation, got {error:?}");
        };
        assert!(message.contains(role.canonical_label()), "{message}");
    }
}
