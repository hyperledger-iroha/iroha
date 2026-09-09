//! Typed frame identities captured before the canonical codec cutover.

#[path = "../../../fixtures/sdk/frame_identity_assertions.rs"]
mod captured;
pub(crate) use captured::assert_bidirectional;

#[test]
fn original_package_observations_are_complete() {
    captured::assert_package_complete("iroha_executor_data_model", 15, 30);
}

#[test]
fn captured_public_frame_identities() {
    assert_bidirectional::<crate::isi::multisig::MultisigInstructionBox>(
        "iroha_executor_data_model::isi::multisig::MultisigInstructionBox",
    );
    assert_bidirectional::<crate::isi::multisig::MultisigRegister>(
        "iroha_executor_data_model::isi::multisig::MultisigRegister",
    );
    assert_bidirectional::<crate::isi::multisig::MultisigPropose>(
        "iroha_executor_data_model::isi::multisig::MultisigPropose",
    );
    assert_bidirectional::<crate::isi::multisig::MultisigApprove>(
        "iroha_executor_data_model::isi::multisig::MultisigApprove",
    );
    assert_bidirectional::<crate::isi::multisig::MultisigCancel>(
        "iroha_executor_data_model::isi::multisig::MultisigCancel",
    );
    assert_bidirectional::<crate::isi::multisig::MultisigInvalidateOutstanding>(
        "iroha_executor_data_model::isi::multisig::MultisigInvalidateOutstanding",
    );
    assert_bidirectional::<crate::isi::multisig::MultisigAccountState>(
        "iroha_executor_data_model::isi::multisig::MultisigAccountState",
    );
    assert_bidirectional::<crate::isi::multisig::MultisigProposalState>(
        "iroha_executor_data_model::isi::multisig::MultisigProposalState",
    );
    assert_bidirectional::<crate::isi::multisig::MultisigProposalTerminalStatus>(
        "iroha_executor_data_model::isi::multisig::MultisigProposalTerminalStatus",
    );
    assert_bidirectional::<crate::isi::multisig::MultisigProposalTerminalState>(
        "iroha_executor_data_model::isi::multisig::MultisigProposalTerminalState",
    );
    assert_bidirectional::<crate::isi::multisig::MultisigProposalTerminalExecutionStateV1>(
        "iroha_executor_data_model::isi::multisig::MultisigProposalTerminalExecutionStateV1",
    );
    assert_bidirectional::<crate::isi::multisig::MultisigApprovalOutcomeStatusV1>(
        "iroha_executor_data_model::isi::multisig::MultisigApprovalOutcomeStatusV1",
    );
    assert_bidirectional::<crate::isi::multisig::MultisigApprovalOutcomeV1>(
        "iroha_executor_data_model::isi::multisig::MultisigApprovalOutcomeV1",
    );
    assert_bidirectional::<crate::isi::multisig::MultisigSpec>(
        "iroha_executor_data_model::isi::multisig::MultisigSpec",
    );
    assert_bidirectional::<crate::isi::multisig::MultisigProposalValue>(
        "iroha_executor_data_model::isi::multisig::MultisigProposalValue",
    );
}
