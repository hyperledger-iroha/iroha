// Exact old economic controls executed through current signed native Decisions.

fn native_transfer_publication_fixture(
    mode: QueuePlanTransferFixture,
) -> (Box<NativeEconomicFixture>, SignedBlock, HeightContext) {
    let (case, funding) = match mode {
        QueuePlanTransferFixture::Single => (NativeEconomicCase::Transfer(3), 10_u32),
        QueuePlanTransferFixture::AtomicBatch => (NativeEconomicCase::AtomicBatchTransfer, 20_u32),
        QueuePlanTransferFixture::IndependentBatch => {
            (NativeEconomicCase::IndependentBatchTransfer, 20_u32)
        }
    };
    let fixture = native_economic_fixture_with_world_initializer(
        &[case],
        false,
        Some(DataAvailabilityLayout {
            encoding: PayloadEncoding::ReedSolomon16,
            chunk_size_bytes: 8192,
            data_shards: 1,
            parity_shards: 1,
            max_payload_size_bytes: 2 * 1024 * 1024,
            max_chunk_count: 512,
        }),
        None,
        |world| {
            let source_account = AccountId::new(
                KeyPair::try_from_seed(vec![0x71; 32], Algorithm::Ed25519)
                    .unwrap()
                    .public_key()
                    .clone(),
            );
            let domain = DomainId::try_new("native-economics", "universal").unwrap();
            let definition_id =
                AssetDefinitionId::derive_from_components(domain, "coin".parse().unwrap());
            let source = AssetId::new(definition_id.clone(), source_account.clone());
            let (id, balance) = Asset::new(source, funding).into_key_value();
            world.assets.insert(id, balance);
            let mut definition = world
                .asset_definitions
                .view()
                .get(&definition_id)
                .unwrap()
                .clone();
            definition.total_quantity = Quantity::from(funding);
            world.asset_definitions.insert(definition_id, definition);
            if !matches!(mode, QueuePlanTransferFixture::Single) {
                let recipient = AccountId::new(
                    KeyPair::try_from_seed(vec![0x73; 32], Algorithm::Ed25519)
                        .unwrap()
                        .public_key()
                        .clone(),
                );
                world
                    .accounts
                    .insert(recipient, AccountValue::new(AccountDetails::default()));
            }
        },
    );
    let (carrier, context) = native_publication_carrier_for_test(&fixture);
    (fixture, carrier, context)
}

fn native_transfer_commitment_for_test(
    block: &SignedBlock,
    witness: &ExecWitness,
) -> ExecutionCommitment {
    use crate::sumeragi::exec::{
        LaneFinalityManifestV1, NativeAmxApplicationManifestV1,
        execution_commitment_from_validated_block,
    };
    let native =
        NativeAmxApplicationManifestV1::from_result_bearing_block_and_merge_entry(block, None)
            .expect("current native Decisions have one actual result-bearing carrier");
    let lanes = LaneFinalityManifestV1::from_result_bearing_block(block)
        .expect("current carrier retains exact lane finality statements");
    execution_commitment_from_validated_block(witness, &native, &lanes, block)
        .expect("native execution commitment binds complete result wire")
}

fn native_transfer_output_mutant_for_test(
    original: &SignedBlock,
    outputs: Vec<iroha_data_model::block::execution_output::ExecutionOutputV1>,
    transcripts: BTreeMap<Hash, Vec<iroha_data_model::fastpq::TransferTranscript>>,
) -> SignedBlock {
    let mut changed = original.clone();
    changed
        .set_execution_outputs(
            outputs,
            original.committed_fragment_count().unwrap(),
            transcripts,
            original.axt_envelopes().unwrap().to_vec(),
            original.axt_policy_snapshot().unwrap().clone(),
            original.axt_transitioned_dataspaces().unwrap().clone(),
            original.lane_finality_statements().to_vec(),
            &crate::execution_output_test_support::structural_output_limits(),
        )
        .expect("tampered result remains structurally canonical for its negative control");
    changed
}

state_test!(consensus_stack native_preflight_distinguishes_first_admission_from_execution_membership
    native_preflight_distinguishes_first_admission_from_execution_membership_on_consensus_stack();
);
fn native_preflight_distinguishes_first_admission_from_execution_membership_on_consensus_stack() {
    let (fixture, carrier, _) =
        native_publication_fixture_for_test(&[NativeEconomicCase::Transfer(25)]);
    let state = &fixture.native.state;
    let groups = native_economic_groups(&fixture);
    let input_hash = groups[0].body().payload().input.entrypoint.hash();
    assert!(
        !state.has_committed_entrypoint(input_hash),
        "QueuePlan admission retains an obligation, not execution membership"
    );
    let (overlay, outputs) = state
        .preexecute_lane_decision_groups(carrier.header(), &groups)
        .expect("an authentic first admission remains eligible for native execution");
    assert!(outputs[0].result.is_ok());
    assert_eq!(
        overlay.world.assets.get(&fixture.source).unwrap().0,
        Quantity::from(75_u32)
    );
    drop(overlay);
    assert_eq!(
        state.world.assets.view().get(&fixture.source).unwrap().0,
        Quantity::from(100_u32)
    );
    state.record_committed_entrypoints_for_tests(
        [input_hash],
        NonZeroUsize::new(state.committed_height()).unwrap(),
    );
    let before = crate::snapshot::canonical_state_snapshot_hash(state).unwrap();
    match state.preexecute_lane_decision_groups(carrier.header(), &groups) {
        Err(MergeLedgerCommitError::ExecutionBatchInvalid(reason)) => assert_eq!(
            reason,
            "native execution reuses a committed carrier or sealed signed-execution identity"
        ),
        Err(error) => panic!("wrong native replay rejection: {error:?}"),
        Ok(_) => panic!("the same already executed Network input cannot execute twice"),
    }
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(state).unwrap(),
        before,
        "historical membership rejection precedes start hooks and economics"
    );
}
