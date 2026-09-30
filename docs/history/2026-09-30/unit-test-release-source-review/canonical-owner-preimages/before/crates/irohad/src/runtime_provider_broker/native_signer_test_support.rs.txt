// Role-owned native transaction payload fixtures; field-level eligibility remains a Core concern.
fn transaction_signer_test_payload_for_network(
    network_id: NetworkId,
    authority: AccountId,
) -> TransactionPayload {
    TransactionBuilder::new(
        network_id,
        authority,
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .into_payload()
    .expect("build native signer test payload")
}
fn native_signer_test_payload_for_network(
    role: iroha_torii::SorafsNativeTransactionSignerRoleV1,
    network_id: NetworkId,
    authority: AccountId,
) -> TransactionPayload {
    TransactionBuilder::new(
        network_id,
        authority,
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([native_signer_test_instruction(role)])
    .into_payload()
    .expect("build role-owned native signer test payload")
}
fn native_signer_test_payload(
    role: iroha_torii::SorafsNativeTransactionSignerRoleV1,
    authority: AccountId,
) -> TransactionPayload {
    native_signer_test_payload_for_network(role, network_id(), authority)
}
fn native_signer_test_instruction(
    role: iroha_torii::SorafsNativeTransactionSignerRoleV1,
) -> iroha_data_model::isi::InstructionBox {
    use iroha_data_model::isi::sorafs::{
        ChargeSorafsReserveRent, MatchSorafsOrderbook, SorafsPdpProofOutcomeSubmissionV1,
        SorafsProofOutcomeSubmissionV1, SubmitSorafsProofOutcome, SubmitSorafsRepairTask,
    };
    use iroha_torii::SorafsNativeTransactionSignerRoleV1 as Role;
    match role {
        Role::ProofOutcome => SubmitSorafsProofOutcome::new(SorafsProofOutcomeSubmissionV1::Pdp(
            SorafsPdpProofOutcomeSubmissionV1 {
                archive_payload: vec![1],
            },
        ))
        .into(),
        Role::Repair => SubmitSorafsRepairTask::new([1; 32], vec![1]).into(),
        Role::Reserve => ChargeSorafsReserveRent::new(
            iroha_data_model::sorafs::capacity::ProviderId::new([1; 32]),
            1,
            1,
            [2; 32],
        )
        .into(),
        Role::Orderbook => MatchSorafsOrderbook::new([2; 32], 1, 1).into(),
    }
}
