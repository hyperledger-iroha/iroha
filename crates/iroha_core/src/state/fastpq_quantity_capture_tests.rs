//! Actual owner capture, rollback and explicit refusal of incomplete quantity coverage.

use super::*;
use crate::{kura::Kura, query::store::LiveQueryStore, smartcontracts::Execute};
use iroha_data_model::{
    account::Account,
    asset::{AssetBalancePolicy, AssetBalanceScope, AssetDefinition},
    block::{BlockExecutionContextBundle, ExternalExecutionContext, builder::BlockBuilder},
    domain::Domain,
    isi::{Burn, Mint, Register, Transfer, Unregister},
    prelude::{InstructionBox, TransactionBuilder},
    transaction::{FeePaymentIntent, SignedTransaction},
};
use iroha_model_base::domain::DomainId;
use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR, BOB_ID};
use std::{num::NonZeroU64, time::Duration};

fn fixture() -> (State, AssetId, AssetId) {
    let domain = DomainId::try_new("quantity-capture", "universal").unwrap();
    let definition = AssetDefinitionId::derive_from_components(domain, "units".parse().unwrap());
    fixture_with_asset_definition(definition)
}

fn fixture_with_asset_definition(definition: AssetDefinitionId) -> (State, AssetId, AssetId) {
    let mut state = State::new_for_testing(
        World::default(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let fees = &mut state.nexus.get_mut().fees;
    fees.base_fee = Quantity::zero();
    fees.per_byte_fee = Quantity::zero();
    fees.per_instruction_fee = Quantity::zero();
    fees.per_gas_unit_fee = Quantity::zero();
    let domain = DomainId::try_new("quantity-capture", "universal").unwrap();
    let alice = AssetId::of(definition.clone(), ALICE_ID.clone());
    let bob = AssetId::of(definition.clone(), BOB_ID.clone());
    {
        let mut setup = state.block(BlockHeader::new(NonZeroU64::MIN, None, None, 1, 0));
        let mut transaction = setup.transaction_for_callback_testing();
        Register::account(Account::new(ALICE_ID.clone()))
            .execute(&ALICE_ID, &mut transaction)
            .unwrap();
        Register::account(Account::new(BOB_ID.clone()))
            .execute(&ALICE_ID, &mut transaction)
            .unwrap();
        Register::domain(Domain::new(domain))
            .execute(&ALICE_ID, &mut transaction)
            .unwrap();
        Register::asset_definition(AssetDefinition::numeric(
            definition,
            "Units",
            AssetBalancePolicy::Global,
            None,
        ))
        .execute(&ALICE_ID, &mut transaction)
        .unwrap();
        Mint::asset_quantity(10_u32, alice.clone())
            .execute(&ALICE_ID, &mut transaction)
            .unwrap();
        transaction.apply();
        setup.commit_world_overlay_for_testing().unwrap();
    }
    let state = authenticate_quantity_state(state);
    (state, alice, bob)
}

/// Retain the component setup, then authenticate the original Network root.
fn authenticate_quantity_state(component: State) -> State {
    use crate::sumeragi::{
        startup,
        test_chain::{CertifiedTestChain, TestChainConfig},
    };
    let nexus = component.nexus_snapshot();
    let mut config = TestChainConfig::new(component.world, 0);
    config.chain_id = component.chain_id;
    config.pipeline = component.pipeline;
    config.governance = Some(component.gov);
    config.nexus = Some(nexus);
    let genesis_account = AccountId::new(config.genesis_key.public_key().clone());
    let mode = config.consensus_mode;
    let prepared = CertifiedTestChain::prepare(config).expect("prepare quantity signed genesis");
    let state = Arc::try_unwrap(prepared.state)
        .unwrap_or_else(|_| panic!("unpublished quantity State is unique"));
    startup::apply_genesis(
        &state,
        prepared.genesis.block().clone(),
        &genesis_account,
        mode.into(),
        None,
    )
    .expect("apply quantity signed genesis");
    state
}

fn quantity_successor_header(state: &State) -> BlockHeader {
    let view = state.view();
    let parent = view.latest_block().expect("original quantity genesis");
    let time_ms = u64::try_from(parent.header().creation_time().as_millis())
        .expect("quantity fixture timestamp fits")
        + 2;
    BlockHeader::new(
        NonZeroU64::new(2).unwrap(),
        Some(parent.hash()),
        None,
        time_ms,
        0,
    )
}

#[test]
fn quantity_fixture_preserves_balances_and_original_authenticated_network_root() {
    let (state, alice, bob) = fixture();
    let view = state.view();
    assert_eq!(view.height(), 1);
    assert_eq!(view.kura().blocks_count(), 1);
    let parent = view.latest_block_hash().expect("original quantity genesis");
    assert_eq!(state.network_id_ref().into_genesis_hash(), parent);
    assert_eq!(
        quantity_successor_header(&state).prev_block_hash(),
        Some(parent)
    );
    assert!(
        quantity_successor_header(&state).creation_time()
            > view.latest_block().unwrap().header().creation_time()
    );
    assert_eq!(
        view.world.assets.get(&alice).unwrap().as_ref(),
        &Quantity::from(10_u32)
    );
    assert!(view.world.assets.get(&bob).is_none());
    assert_eq!(
        view.world
            .asset_definitions
            .get(alice.definition())
            .unwrap()
            .total_quantity(),
        &Quantity::from(10_u32)
    );
    assert!(state.nexus_snapshot().fees.base_fee.is_zero());
    assert!(crate::sumeragi::lanes::routing::committed_root_scope(view.world()).is_some());
}

fn source(state: &State, body: Vec<InstructionBox>) -> (SignedBlock, Hash) {
    source_with_fee(state, body, FeePaymentIntent::authority(vec![], None))
}

fn quantity_execution_context(
    state: &State,
    transaction: &SignedTransaction,
    header: &BlockHeader,
) -> ExternalExecutionContext {
    let accepted =
        crate::tx::AcceptedTransaction::new_unchecked(std::borrow::Cow::Borrowed(transaction));
    let view = state.view();
    let snapshot = crate::sumeragi::lanes::routing::RoutingSnapshot::of(&view);
    let native = snapshot
        .inputs(view.world())
        .execution_route(&accepted, header.height().get())
        .expect("quantity input retains its exact committed native route");
    ExternalExecutionContext::new(
        accepted.hash_as_entrypoint(),
        native.lane_id,
        native.dataspace_id,
    )
}

fn source_with_fee(
    state: &State,
    body: Vec<InstructionBox>,
    fee: FeePaymentIntent,
) -> (SignedBlock, Hash) {
    let mut transaction = TransactionBuilder::new(state.network_id, ALICE_ID.clone(), fee);
    transaction.set_creation_time(
        quantity_successor_header(state).creation_time() - Duration::from_millis(1),
    );
    let transaction = transaction
        .with_instructions(body)
        .sign(ALICE_KEYPAIR.private_key());
    let hash =
        Hash::from(TransactionEntrypoint::External(transaction.clone()).execution_call_hash());
    let header = quantity_successor_header(state);
    let context = quantity_execution_context(state, &transaction, &header);
    let mut builder = BlockBuilder::new(header);
    builder.set_execution_context(Some(BlockExecutionContextBundle::new(vec![context])));
    builder.push_transaction(transaction);
    (
        builder.build_with_signature(0, ALICE_KEYPAIR.private_key()),
        hash,
    )
}

#[test]
fn original_block_start_world_owners_preserve_exact_quantity_lineage() {
    let (state, alice, _) = fixture();
    let (source, _) = source(&state, vec![Mint::asset_quantity(1_u32, alice).into()]);
    let mut pristine = None;
    let (mut block, _recording) = state
        .block_with_recorded_pristine_carrier_stage(
            &source,
            |block| {
                pristine = Some((
                    block.world.assets.write_observation(),
                    block.world.asset_definitions.write_observation(),
                    block.fastpq_quantity_candidate.applied_world_transactions,
                ));
                Ok::<(), String>(())
            },
            |error| error,
        )
        .unwrap();
    assert_eq!(pristine, Some(((false, 0), (false, 0), 0)));
    let expected = block.fastpq_quantity_candidate.applied_world_transactions;
    assert!(
        expected >= 2,
        "both original World-only start owners applied"
    );
    assert_eq!(block.world.assets.write_observation(), (false, expected));
    assert_eq!(
        block.world.asset_definitions.write_observation(),
        (false, expected)
    );
    block.observe_quantity_block_journals();
    assert_eq!(block.fastpq_quantity_candidate.issue, None);
    assert!(block.fastpq_quantity_candidate.entries.is_empty());
    assert_eq!(
        block.fastpq_quantity_candidate.require_complete(),
        Err(QuantityCaptureIssue::IncompleteCoverage)
    );
}

#[test]
fn classified_original_world_child_keeps_metadata_rollback_and_restored_raw_write_facts() {
    for raw_write in [false, true] {
        for apply in [false, true] {
            with_signed_quantity_block(|block, alice, _, _| {
                let original_count = block.fastpq_quantity_candidate.applied_world_transactions;
                let key: iroha_model_base::name::Name = "quantity_child_metadata".parse().unwrap();
                let value = iroha_primitives::json::Json::new("original child");
                let mut world = block
                    .world
                    .transaction_without_telemetry(LaneConfig::default(), 0);
                assert!(
                    world
                        .account_mut(&ALICE_ID)
                        .unwrap()
                        .insert(key.clone(), value.clone())
                        .is_none()
                );
                if raw_write {
                    let balance = world.assets.get_mut(alice).unwrap();
                    **balance = Quantity::from(99_u32);
                    **balance = Quantity::from(9_u32);
                }
                let mut pending = QuantityCandidateArchive::default();
                pending.observe(&world);
                if apply {
                    world.apply();
                    block.fastpq_quantity_candidate.apply(pending);
                } else {
                    drop(world);
                    drop(pending);
                }
                block.observe_quantity_block_journals();
                assert_eq!(
                    block.fastpq_quantity_candidate.applied_world_transactions,
                    original_count + u64::from(apply)
                );
                assert_eq!(
                    block.fastpq_quantity_candidate.issue,
                    (raw_write && apply).then_some(QuantityCaptureIssue::UnownedMutation)
                );
                assert_eq!(
                    block.world.account(&ALICE_ID).unwrap().metadata().get(&key),
                    apply.then_some(&value)
                );
                assert_eq!(
                    block.world.assets.get(alice).unwrap().as_ref(),
                    &Quantity::from(9_u32)
                );
            });
        }
    }
}

#[test]
fn signed_quantity_source_without_execution_context_is_rejected_before_execution() {
    let (state, alice, bob) = fixture();
    let (valid, _) = source(
        &state,
        vec![Transfer::asset_quantity(alice.clone(), 1_u32, BOB_ID.clone()).into()],
    );
    assert!(valid.execution_context().is_some());
    let original = valid.external_transactions().next().unwrap();
    let mut builder = BlockBuilder::new(quantity_successor_header(&state));
    builder.push_transaction(original.clone());
    let missing = builder.build_with_signature(0, ALICE_KEYPAIR.private_key());
    assert!(missing.execution_context().is_none());
    assert!(missing.header().execution_context_hash().is_none());
    assert_eq!(missing.external_transactions().next().unwrap(), original);
    let (mut block, _recording) = state
        .block_with_recorded_pristine_carrier_stage(
            &missing,
            |_| Ok::<(), String>(()),
            |error| error,
        )
        .unwrap();
    block.reserve_ordinary_execution_outputs(&missing).unwrap();
    let fragments = block.committed_fragment_count();
    let error = block
        .execute_ordinary_output_plan(&missing, None)
        .unwrap_err();
    assert!(
        matches!(error, crate::execution_attempt::ExecutionAttemptError::Rejected(reason)
            if reason == "Network source has an invalid execution context")
    );
    assert_eq!(block.committed_fragment_count(), fragments);
    assert_eq!(
        block.world.assets.get(&alice).unwrap().as_ref(),
        &Quantity::from(10_u32)
    );
    assert!(block.world.assets.get(&bob).is_none());
    assert!(block.fastpq_quantity_candidate.entries.is_empty());
}

#[test]
fn actual_signed_interleaving_keeps_scoped_lifecycle_and_complete_order_without_export() {
    let (state, alice, bob) = fixture();
    let (source, call) = source(
        &state,
        vec![
            Transfer::asset_quantity(alice.clone(), 1_u32, BOB_ID.clone()).into(),
            Mint::asset_quantity(5_u32, alice.clone()).into(),
            Burn::asset_quantity(1_u32, alice.clone()).into(),
            Transfer::asset_quantity(alice.clone(), 2_u32, BOB_ID.clone()).into(),
        ],
    );
    let (mut block, _recording) = state
        .block_with_recorded_pristine_carrier_stage(
            &source,
            |_| Ok::<(), String>(()),
            |error| error,
        )
        .unwrap();
    block.reserve_ordinary_execution_outputs(&source).unwrap();
    block.execute_ordinary_output_plan(&source, None).unwrap();
    assert_eq!(
        block.world.assets.get(&alice).unwrap().as_ref(),
        &Quantity::from(11_u32)
    );
    assert_eq!(
        block.world.assets.get(&bob).unwrap().as_ref(),
        &Quantity::from(3_u32)
    );
    let candidate = &block.fastpq_quantity_candidate;
    assert_exact_applied_measurement(candidate);
    assert_eq!(candidate.issue, None);
    assert_eq!(candidate.entries.len(), 1);
    let entry = &candidate.entries[&call];
    assert_eq!(
        entry
            .effects
            .iter()
            .map(|effect| effect.ordinal)
            .collect::<Vec<_>>(),
        vec![0, 1, 2, 3]
    );
    assert_eq!(entry.context.entry.entry_hash, call);
    let token = *block
        .world
        .axt_asset_incarnations
        .get(alice.definition())
        .unwrap();
    for effect in &entry.effects {
        assert_eq!(
            effect.authority_digest,
            crate::fastpq::authority_digest(&ALICE_ID)
        );
        let balance = match &effect.kind {
            FastpqExecutionEffectKindV1::Transfer(effect) => &effect.source,
            FastpqExecutionEffectKindV1::Mint(effect)
            | FastpqExecutionEffectKindV1::Burn(effect) => &effect.balance,
        };
        assert_eq!(balance.asset.incarnation, token);
        assert_eq!(balance.asset.definition, *alice.definition());
        assert_eq!(balance.scope, AssetBalanceScope::Global);
    }
    assert!(matches!(
        entry.effects[1].kind,
        FastpqExecutionEffectKindV1::Mint(_)
    ));
    assert!(matches!(
        entry.effects[2].kind,
        FastpqExecutionEffectKindV1::Burn(_)
    ));
    assert_eq!(
        candidate.require_complete(),
        Err(QuantityCaptureIssue::IncompleteCoverage)
    );
}

#[test]
fn rejected_signed_body_drops_captured_facts_and_unsupported_mutation_observation() {
    let (state, alice, _) = fixture();
    let (source, _) = source(
        &state,
        vec![
            Mint::asset_quantity(5_u32, alice.clone()).into(),
            Unregister::asset_definition(alice.definition().clone()).into(),
            Mint::asset_quantity(1_u32, alice.clone()).into(),
        ],
    );
    let (mut block, _recording) = state
        .block_with_recorded_pristine_carrier_stage(
            &source,
            |_| Ok::<(), String>(()),
            |error| error,
        )
        .unwrap();
    block.reserve_ordinary_execution_outputs(&source).unwrap();
    block.execute_ordinary_output_plan(&source, None).unwrap();
    assert_eq!(
        block.world.assets.get(&alice).unwrap().as_ref(),
        &Quantity::from(10_u32)
    );
    assert!(block.fastpq_quantity_candidate.entries.is_empty());
    assert_exact_applied_measurement(&block.fastpq_quantity_candidate);
    assert_eq!(
        block.fastpq_quantity_candidate.usage,
        QuantityCandidateUsage::default()
    );
    assert_eq!(block.fastpq_quantity_candidate.issue, None);
    assert_eq!(
        block.fastpq_quantity_candidate.require_complete(),
        Err(QuantityCaptureIssue::IncompleteCoverage)
    );
}

#[test]
fn unsupported_actual_mutation_poison_applies_and_rolls_back_with_world() {
    let (state, alice, _) = fixture();
    let mut block = state.block(quantity_successor_header(&state));
    {
        let mut transaction = block.transaction();
        transaction
            .world
            .increase_asset_total_amount(alice.definition(), &Quantity::from(1_u32))
            .unwrap();
        transaction.poison_quantity_candidate_owner();
        assert!(transaction.world.quantity_mutation_observation.unowned);
        assert_eq!(
            transaction.pending_fastpq_quantity_candidate.issue,
            Some(QuantityCaptureIssue::UnsupportedOwner)
        );
    }
    assert_eq!(block.fastpq_quantity_candidate.issue, None);
    assert_eq!(
        block
            .world
            .asset_definition(alice.definition())
            .unwrap()
            .total_quantity(),
        &Quantity::from(10_u32)
    );
    let mut transaction = block.transaction();
    // The low-level helper is intentionally not a typed quantity owner.
    transaction
        .world
        .increase_asset_total_amount(alice.definition(), &Quantity::from(1_u32))
        .unwrap();
    transaction.apply();
    assert_eq!(
        block.fastpq_quantity_candidate.require_complete(),
        Err(QuantityCaptureIssue::UnownedMutation)
    );
    assert_eq!(
        block
            .world
            .asset_definition(alice.definition())
            .unwrap()
            .total_quantity(),
        &Quantity::from(11_u32)
    );
}

#[test]
fn returned_error_poison_and_scope_observation_never_grant_export() {
    let (state, _, _) = fixture();
    let mut block = state.block(quantity_successor_header(&state));
    let mut transaction = block.transaction();
    let result: Result<(), Error> = transaction.apply_with_quantity_candidate(
        Err(QuantityCaptureIssue::UnsupportedOwner),
        |_| {
            Err(Error::InvariantViolation(
                "original operation failure".into(),
            ))
        },
    );
    assert!(result.is_err());
    assert!(!transaction.world.quantity_mutation_observation.owned);
    assert_eq!(
        transaction
            .pending_fastpq_quantity_candidate
            .require_complete(),
        Err(QuantityCaptureIssue::UnsupportedOwner)
    );
}

#[test]
fn exact_capture_rejects_wrong_prestate_and_checks_actual_poststate_and_incarnation() {
    let (state, alice, _) = fixture();
    let mut block = state.block(quantity_successor_header(&state));
    let mut transaction = block.transaction();
    let balance = transaction.quantity_balance_identity(&alice).unwrap();
    let scoped = AssetId::with_scope(
        alice.definition().clone(),
        ALICE_ID.clone(),
        AssetBalanceScope::Dataspace(DataSpaceId::new(9)),
    );
    assert_ne!(
        transaction.quantity_balance_identity(&scoped).unwrap(),
        balance
    );
    let change = FastpqExecutionSupplyChangeV1 {
        balance,
        amount: Quantity::from(2_u32),
        balance_before: Quantity::from(10_u32),
        balance_after: Quantity::from(12_u32),
        supply_before: Quantity::from(10_u32),
        supply_after: Quantity::from(12_u32),
    };
    let expected = transaction
        .quantity_expected_state(&[FastpqExecutionEffectKindV1::Mint(change.clone())])
        .unwrap();
    assert!(!transaction.quantity_post_state_matches(&expected));
    let mut wrong = change.clone();
    wrong.balance_before = Quantity::from(9_u32);
    wrong.balance_after = Quantity::from(11_u32);
    assert!(matches!(
        transaction.quantity_expected_state(&[FastpqExecutionEffectKindV1::Mint(wrong)]),
        Err(QuantityCaptureIssue::InvalidFacts)
    ));
    transaction.world.assets.get_mut(&alice).unwrap().0 = Quantity::from(12_u32);
    transaction
        .world
        .increase_asset_total_amount(alice.definition(), &Quantity::from(2_u32))
        .unwrap();
    assert!(transaction.quantity_post_state_matches(&expected));
    transaction
        .world
        .axt_asset_incarnations
        .remove(alice.definition().clone());
    assert!(!transaction.quantity_post_state_matches(&expected));
    assert_eq!(
        transaction.quantity_balance_identity(&alice),
        Err(QuantityCaptureIssue::MissingIncarnation)
    );
}

fn assert_exact_applied_measurement(archive: &QuantityCandidateArchive) {
    let mut expected = QuantityCandidateUsage::default();
    for (hash, entry) in archive.entries.iter() {
        let input_bytes = entry
            .effects
            .iter()
            .map(|effect| u64::try_from(norito::encode_canonical(effect).unwrap().len()).unwrap())
            .sum::<u64>();
        let full = QuantityCandidateUsage {
            entries: 1,
            deltas: u64::try_from(entry.effects.len()).unwrap(),
            input_bytes,
            statement_bytes: u64::try_from(norito::encode_canonical(entry.wire()).unwrap().len())
                .unwrap(),
        };
        let measured = &archive.entries.get(hash).unwrap().measurement;
        assert_eq!(measured.baseline, QuantityCandidateUsage::default());
        assert_eq!(measured.full, full);
        expected.entries += full.entries;
        expected.deltas += full.deltas;
        expected.input_bytes += full.input_bytes;
        expected.statement_bytes += full.statement_bytes;
    }
    assert!(archive.base_usage.is_none());
    assert_eq!(expected.entries as usize, archive.entries.len());
    assert_eq!(archive.usage, expected);
}

#[test]
fn distinct_signed_entries_retain_exact_independent_counters() {
    let (state, alice, _) = fixture();
    let bodies: [Vec<InstructionBox>; 2] = [
        vec![
            Mint::asset_quantity(1_u32, alice.clone()).into(),
            Mint::asset_quantity(2_u32, alice.clone()).into(),
        ],
        vec![
            Burn::asset_quantity(1_u32, alice.clone()).into(),
            Mint::asset_quantity(3_u32, alice.clone()).into(),
        ],
    ];
    let header = quantity_successor_header(&state);
    let mut builder = BlockBuilder::new(header);
    let mut hashes = Vec::new();
    let mut contexts = Vec::new();
    for body in bodies {
        let mut transaction = TransactionBuilder::new(
            state.network_id,
            ALICE_ID.clone(),
            FeePaymentIntent::authority(vec![], None),
        );
        transaction.set_creation_time(
            quantity_successor_header(&state).creation_time() - Duration::from_millis(1),
        );
        let transaction = transaction
            .with_instructions(body)
            .sign(ALICE_KEYPAIR.private_key());
        hashes.push(Hash::from(
            TransactionEntrypoint::External(transaction.clone()).execution_call_hash(),
        ));
        contexts.push(quantity_execution_context(&state, &transaction, &header));
        builder.push_transaction(transaction);
    }
    builder.set_execution_context(Some(BlockExecutionContextBundle::new(contexts)));
    let source = builder.build_with_signature(0, ALICE_KEYPAIR.private_key());
    let (mut block, _recording) = state
        .block_with_recorded_pristine_carrier_stage(
            &source,
            |_| Ok::<(), String>(()),
            |error| error,
        )
        .unwrap();
    block.reserve_ordinary_execution_outputs(&source).unwrap();
    block.execute_ordinary_output_plan(&source, None).unwrap();
    let archive = &block.fastpq_quantity_candidate;
    assert_eq!(archive.issue, None);
    assert_eq!(archive.usage.entries, 2);
    assert_eq!(archive.usage.deltas, 4);
    assert_exact_applied_measurement(archive);
    for hash in hashes {
        assert_eq!(
            archive.entries[&hash]
                .effects
                .iter()
                .map(|effect| effect.ordinal)
                .collect::<Vec<_>>(),
            vec![0, 1]
        );
    }
    assert_eq!(
        block.world.assets.get(&alice).unwrap().as_ref(),
        &Quantity::from(15_u32)
    );
}

#[test]
fn body_and_real_pipeline_fee_replace_one_complete_entry_counter() {
    use iroha_config::parameters::actual::{GasLiquidity, GasRate, GasVolatility};
    use iroha_data_model::transaction::{FeeChargeKind, FeeChargeLimit};
    use iroha_primitives::numeric::Numeric;
    let _fee_guard = crate::status::nexus_fee_test_lock().lock().unwrap();
    let (mut state, alice, bob) = fixture();
    {
        let mut setup = state.block(quantity_successor_header(&state));
        let mut transaction = setup.transaction_for_callback_testing();
        Mint::asset_quantity(1_000_000_000_u64, alice.clone())
            .execute(&ALICE_ID, &mut transaction)
            .unwrap();
        transaction.apply();
        setup.commit_world_overlay_for_testing().unwrap();
    }
    state.pipeline.gas.tech_account_id = BOB_ID.to_string();
    state.pipeline.gas.accepted_assets = vec![alice.definition().canonical_address()];
    state.pipeline.gas.units_per_gas = vec![GasRate {
        asset: alice.definition().canonical_address(),
        units_per_gas: 1,
        twap_local_per_xor: Numeric::one(),
        liquidity: GasLiquidity::Tier1,
        volatility: GasVolatility::Stable,
    }];
    let body = vec![
        Mint::asset_quantity(2_u32, alice.clone()).into(),
        Transfer::asset_quantity(alice.clone(), 1_u32, BOB_ID.clone()).into(),
    ];
    let gas = crate::gas::meter_instructions(&body);
    assert!(gas > 0 && gas < 1_000_000_000);
    let fee = Quantity::from(gas);
    let (source, call) = source_with_fee(
        &state,
        body,
        FeePaymentIntent::authority(
            vec![FeeChargeLimit::new(
                FeeChargeKind::PipelineGas,
                alice.definition().clone(),
                fee.clone(),
            )],
            NonZeroU64::new(gas),
        ),
    );
    let (mut block, _recording) = state
        .block_with_recorded_pristine_carrier_stage(
            &source,
            |_| Ok::<(), String>(()),
            |error| error,
        )
        .unwrap();
    block.reserve_ordinary_execution_outputs(&source).unwrap();
    block.execute_ordinary_output_plan(&source, None).unwrap();
    let archive = &block.fastpq_quantity_candidate;
    assert_eq!(archive.issue, None);
    assert_eq!(archive.usage.entries, 1);
    assert_eq!(archive.usage.deltas, 3);
    assert_exact_applied_measurement(archive);
    let entry = &archive.entries[&call];
    assert_eq!(
        entry
            .effects
            .iter()
            .map(|effect| effect.ordinal)
            .collect::<Vec<_>>(),
        vec![0, 1, 2]
    );
    assert!(entry.effects.iter().any(|effect| matches!(&effect.kind, FastpqExecutionEffectKindV1::Transfer(transfer) if transfer.amount == fee)));
    assert_eq!(
        block.world.assets.get(&alice).unwrap().as_ref(),
        &Quantity::from(1_000_000_011_u64).checked_sub(&fee).unwrap()
    );
    assert_eq!(
        block.world.assets.get(&bob).unwrap().as_ref(),
        &Quantity::from(1_u32).checked_add(&fee).unwrap()
    );
}

#[test]
fn capture_capacity_refusal_keeps_exact_prefix_counters_and_original_business_result() {
    use iroha_data_model::parameter::{BlockParameter, FastpqSourcePolicyV1, Parameter};
    let (state, alice, _) = fixture();
    {
        let mut parameters = state.world.parameters.block();
        let previous = parameters.get().block().fastpq_source();
        let mut intrinsic = previous.intrinsic;
        intrinsic.max_transcripts = 1;
        intrinsic.max_deltas = 1;
        let policy = FastpqSourcePolicyV1::from_sizing(
            parameters.get().block().execution_output(),
            intrinsic,
            previous.mandatory,
            1,
        )
        .unwrap();
        parameters
            .get_mut()
            .set_parameter(Parameter::Block(BlockParameter::FastpqSource(policy)));
        parameters.commit();
    }
    let (source, call) = source(
        &state,
        vec![
            Mint::asset_quantity(1_u32, alice.clone()).into(),
            Mint::asset_quantity(2_u32, alice.clone()).into(),
        ],
    );
    let (mut block, _recording) = state
        .block_with_recorded_pristine_carrier_stage(
            &source,
            |_| Ok::<(), String>(()),
            |error| error,
        )
        .unwrap();
    block.reserve_ordinary_execution_outputs(&source).unwrap();
    block.execute_ordinary_output_plan(&source, None).unwrap();
    assert_eq!(
        block.world.assets.get(&alice).unwrap().as_ref(),
        &Quantity::from(13_u32)
    );
    let archive = &block.fastpq_quantity_candidate;
    assert_eq!(
        archive.require_complete(),
        Err(QuantityCaptureIssue::Capacity)
    );
    assert_eq!(archive.usage.entries, 1);
    assert_eq!(archive.usage.deltas, 1);
    assert_eq!(archive.entries[&call].effects.len(), 1);
    assert_exact_applied_measurement(archive);
}

#[test]
fn candidate_counter_arithmetic_refuses_overflow_and_underflow_in_every_dimension() {
    let unit = QuantityCandidateUsage {
        entries: 1,
        deltas: 1,
        input_bytes: 1,
        statement_bytes: 1,
    };
    let empty = QuantityCandidateUsage::default();
    assert_eq!(empty.checked_add(unit), Some(unit));
    assert_eq!(unit.checked_sub(unit), Some(empty));
    assert!(empty.checked_sub(unit).is_none());
    for dimension in 0..4 {
        let mut maximum = empty;
        match dimension {
            0 => maximum.entries = u64::MAX,
            1 => maximum.deltas = u64::MAX,
            2 => maximum.input_bytes = u64::MAX,
            3 => maximum.statement_bytes = u64::MAX,
            _ => unreachable!(),
        }
        assert!(maximum.checked_add(unit).is_none());
    }
}

fn sponsored_burn_fixture() -> (
    State,
    AssetId,
    iroha_data_model::nexus::FeeSponsorProgramId,
    InstructionBox,
) {
    use iroha_data_model::{isi::Log, nexus::*, prelude::Level};
    // Nexus debits must use the currency pinned by this original global root.
    let fee_asset = AssetDefinitionId::parse_address_literal(
        &iroha_config::parameters::defaults::nexus::fees::fee_asset_id(),
    )
    .unwrap();
    let (mut state, alice, custody) = fixture_with_asset_definition(fee_asset);
    let instruction = InstructionBox::from(Log::new(Level::INFO, "sponsored burn".into()));
    let wire_id = iroha_data_model::isi::instruction_wire_id(&instruction)
        .unwrap()
        .to_owned();
    let program_id = FeeSponsorProgramId::new(BOB_ID.clone(), "quantity".parse().unwrap());
    {
        let mut setup = state.block(quantity_successor_header(&state));
        let mut transaction = setup.transaction_for_callback_testing();
        Transfer::asset_quantity(alice, 10_u32, BOB_ID.clone())
            .execute(&ALICE_ID, &mut transaction)
            .unwrap();
        let mut program = FeeSponsorProgram::new(program_id.clone(), BOB_ID.clone());
        program.lifecycle = FeeSponsorProgramLifecycle::Active;
        program.active_revision = Some(1);
        transaction
            .world
            .fee_sponsor_programs
            .insert(program_id.clone(), program);
        transaction.world.fee_sponsor_program_revisions.insert(
            FeeSponsorProgramRevisionKey::new(program_id.clone(), 1),
            FeeSponsorProgramRevision {
                program_id: program_id.clone(),
                revision: 1,
                eligibility: FeeSponsorEligibility::EnrolledOnly,
                rules: vec![FeeSponsorRule {
                    id: "allow_log".parse().unwrap(),
                    effect: FeeSponsorRuleEffect::Allow,
                    selectors: vec![FeeSponsorRuleSelector::NativeInstruction(
                        FeeSponsorNativeInstructionSelector {
                            wire_id,
                            asset_definition_id: None,
                        },
                    )],
                }],
                asset_budgets: vec![FeeSponsorAssetBudget {
                    asset_definition_id: custody.definition().clone(),
                    per_transaction: Quantity::from(10_u32),
                    per_block: Quantity::from(10_u32),
                    per_program_epoch: Quantity::from(10_u32),
                    per_beneficiary_epoch: Quantity::from(10_u32),
                    reserve_floor: Quantity::zero(),
                    epoch_length_blocks: NonZeroU64::MIN,
                }],
            },
        );
        let enrollment_key = FeeSponsorEnrollmentKey {
            program_id: program_id.clone(),
            beneficiary: ALICE_ID.clone(),
        };
        transaction.world.fee_sponsor_enrollments.insert(
            enrollment_key.clone(),
            FeeSponsorEnrollment {
                key: enrollment_key,
                enrolled_at_height: 1,
            },
        );
        let vault_key = FeeSponsorVaultKey {
            program_id: program_id.clone(),
            asset_definition_id: custody.definition().clone(),
        };
        transaction.world.fee_sponsor_vaults.insert(
            vault_key.clone(),
            FeeSponsorVault {
                key: vault_key,
                balance: Quantity::from(10_u32),
            },
        );
        transaction.apply();
        setup.commit_world_overlay_for_testing().unwrap();
    }
    let fees = &mut state.nexus.get_mut().fees;
    fees.settlement_mode = iroha_config::parameters::actual::NexusFeeSettlementMode::Direct;
    fees.fee_asset_id = custody.definition().canonical_address();
    fees.sponsor_vault_custody_account_id = BOB_ID.clone();
    fees.base_fee = Quantity::from(2_u32);
    {
        let view = state.view();
        assert_eq!(
            crate::block::resolve_network_xor_asset_definition(
                view.world(),
                &view.nexus.fees.fee_asset_id,
                u64::try_from(
                    quantity_successor_header(&state)
                        .creation_time()
                        .as_millis()
                )
                .unwrap(),
            ),
            Some(custody.definition().clone()),
        );
    }
    (state, custody, program_id, instruction)
}

#[test]
fn actual_signed_sponsor_debit_captures_exact_custody_balance_and_supply() {
    use iroha_data_model::{
        nexus::FeeSponsorVaultKey,
        transaction::{FeeChargeKind, FeeChargeLimit},
    };
    let _fee_guard = crate::status::nexus_fee_test_lock().lock().unwrap();
    let (state, custody, program_id, instruction) = sponsored_burn_fixture();
    let (source, call) = source_with_fee(
        &state,
        vec![instruction],
        FeePaymentIntent::sponsor(
            program_id.clone(),
            1,
            vec![FeeChargeLimit::new(
                FeeChargeKind::Nexus,
                custody.definition().clone(),
                Quantity::from(2_u32),
            )],
            None,
        ),
    );
    let (mut block, _recording) = state
        .block_with_recorded_pristine_carrier_stage(
            &source,
            |_| Ok::<(), String>(()),
            |error| error,
        )
        .unwrap();
    block.reserve_ordinary_execution_outputs(&source).unwrap();
    block.execute_ordinary_output_plan(&source, None).unwrap();
    let outputs = block.retained_execution_outputs_for_test().unwrap();
    let iroha_data_model::block::execution_output::ExecutionOutputV1::Network(output) = &outputs[0]
    else {
        panic!("signed sponsor source must retain its Network row");
    };
    assert!(output.result.is_ok(), "sponsor result: {:?}", output.result);
    let receipt = output
        .result
        .nexus_fee_receipt()
        .expect("actual sponsor charge");
    receipt
        .validate_for_network_input(source.network_entrypoint_at(0).unwrap(), 2)
        .unwrap();
    assert_eq!(receipt.fee_asset_id, *custody.definition());
    assert_eq!(receipt.fee_amount, Quantity::from(2_u32));
    let archive = &block.fastpq_quantity_candidate;
    assert_eq!(archive.issue, None);
    assert_eq!(archive.usage.entries, 1);
    assert_eq!(archive.usage.deltas, 1);
    assert_exact_applied_measurement(archive);
    let effect = &archive.entries[&call].effects[0];
    assert_eq!(
        effect.authority_digest,
        crate::fastpq::authority_digest(&ALICE_ID)
    );
    let FastpqExecutionEffectKindV1::Burn(burn) = &effect.kind else {
        panic!("direct sponsor fee must retain its original burn operation");
    };
    assert_eq!(burn.balance.account, BOB_ID.clone());
    assert_eq!(burn.balance.scope, AssetBalanceScope::Global);
    assert_eq!(burn.balance.asset.definition, *custody.definition());
    assert_eq!(
        burn.balance.asset.incarnation,
        *block
            .world
            .axt_asset_incarnations
            .get(custody.definition())
            .unwrap()
    );
    assert_eq!(burn.amount, Quantity::from(2_u32));
    assert_eq!(burn.balance_before, Quantity::from(10_u32));
    assert_eq!(burn.balance_after, Quantity::from(8_u32));
    assert_eq!(burn.supply_before, Quantity::from(10_u32));
    assert_eq!(burn.supply_after, Quantity::from(8_u32));
    assert_eq!(
        block.world.assets.get(&custody).unwrap().as_ref(),
        &Quantity::from(8_u32)
    );
    assert_eq!(
        block
            .world
            .asset_definitions
            .get(custody.definition())
            .unwrap()
            .total_quantity(),
        &Quantity::from(8_u32)
    );
    assert_eq!(
        block
            .world
            .fee_sponsor_vaults
            .get(&FeeSponsorVaultKey {
                program_id,
                asset_definition_id: custody.definition().clone(),
            })
            .unwrap()
            .balance,
        Quantity::from(8_u32)
    );
    assert_eq!(
        archive.require_complete(),
        Err(QuantityCaptureIssue::IncompleteCoverage)
    );
}

#[test]
fn sponsor_business_capability_without_original_signed_source_cannot_capture() {
    let (mut state, custody, program_id, instruction) = sponsored_burn_fixture();
    state.nexus.get_mut().fees.base_fee = Quantity::zero();
    let (source, call) = source(&state, vec![instruction]);
    let (mut block, _recording) = state
        .block_with_recorded_pristine_carrier_stage(
            &source,
            |_| Ok::<(), String>(()),
            |error| error,
        )
        .unwrap();
    block.reserve_ordinary_execution_outputs(&source).unwrap();
    block.execute_ordinary_output_plan(&source, None).unwrap();
    assert!(block.fastpq_quantity_candidate.entries.is_empty());
    assert_eq!(block.fastpq_quantity_candidate.issue, None);
    let mut transaction = block.transaction();
    // The actual signed Log already retained E. Merely naming that entry
    // cannot supply the missing original signed sponsor-charge capability.
    transaction.tx_call_hash = Some(call);
    let charge = crate::executor::VerifiedFeeSponsorCharge::burn_for_test(
        ALICE_ID.clone(),
        program_id,
        custody.clone(),
        Quantity::from(2_u32),
    );
    crate::smartcontracts::isi::asset::isi::execute_verified_fee_sponsor_charge(
        &mut transaction,
        charge,
    )
    .unwrap();
    transaction.apply();
    assert_eq!(
        block.world.assets.get(&custody).unwrap().as_ref(),
        &Quantity::from(8_u32)
    );
    assert!(block.fastpq_quantity_candidate.entries.is_empty());
    assert_eq!(
        block.fastpq_quantity_candidate.require_complete(),
        Err(QuantityCaptureIssue::UnsupportedOwner)
    );
}

#[test]
#[ignore = "explicit genuine fixture producer; retain its source-bound native output"]
fn emit_genuine_signed_quantity_effect_capture() {
    let (state, alice, bob) = fixture();
    let (source, call) = source(
        &state,
        vec![
            Transfer::asset_quantity(alice.clone(), 1_u32, BOB_ID.clone()).into(),
            Mint::asset_quantity(5_u32, alice.clone()).into(),
            Burn::asset_quantity(1_u32, alice.clone()).into(),
            Transfer::asset_quantity(alice.clone(), 2_u32, BOB_ID.clone()).into(),
        ],
    );
    let (mut block, _recording) = state
        .block_with_recorded_pristine_carrier_stage(
            &source,
            |_| Ok::<(), String>(()),
            |error| error,
        )
        .unwrap();
    block.reserve_ordinary_execution_outputs(&source).unwrap();
    block.execute_ordinary_output_plan(&source, None).unwrap();
    assert_eq!(
        block.world.assets.get(&alice).unwrap().as_ref(),
        &Quantity::from(11_u32)
    );
    assert_eq!(
        block.world.assets.get(&bob).unwrap().as_ref(),
        &Quantity::from(3_u32)
    );
    assert_eq!(
        block
            .world
            .asset_definition(alice.definition())
            .unwrap()
            .total_quantity(),
        &Quantity::from(14_u32)
    );
    let candidate = &block.fastpq_quantity_candidate;
    assert_eq!(candidate.issue, None);
    assert_eq!(candidate.entries.len(), 1);
    assert_exact_applied_measurement(candidate);
    let entry = &candidate.entries[&call];
    assert_eq!(entry.effects.len(), 4);
    assert_eq!(entry.context.entry.entry_hash, call);
    assert_eq!(entry.context.source.network_id, state.network_id);
    assert_eq!(entry.context.source.height, source.header().height().get());
    for (ordinal, effect) in entry.effects.iter().enumerate() {
        assert_eq!(effect.ordinal, u32::try_from(ordinal).unwrap());
        assert_eq!(
            effect.authority_digest,
            crate::fastpq::authority_digest(&ALICE_ID)
        );
    }
    assert!(matches!(
        entry.effects[0].kind,
        FastpqExecutionEffectKindV1::Transfer(_)
    ));
    assert!(matches!(
        entry.effects[1].kind,
        FastpqExecutionEffectKindV1::Mint(_)
    ));
    assert!(matches!(
        entry.effects[2].kind,
        FastpqExecutionEffectKindV1::Burn(_)
    ));
    assert!(matches!(
        entry.effects[3].kind,
        FastpqExecutionEffectKindV1::Transfer(_)
    ));
    assert_eq!(
        candidate.require_complete(),
        Err(QuantityCaptureIssue::IncompleteCoverage)
    );

    let inputs = iroha_data_model::fastpq::FastpqPublicInputs {
        dsid: crate::fastpq::dataspace_id_bytes(entry.context.entry.dataspace_id),
        slot: source.header().creation_time_ms.saturating_mul(1_000_000),
        old_root: [0; 32],
        new_root: [0; 32],
        perm_root: crate::fastpq::permission_table_root(block.world.roles.iter()),
        tx_set_hash: iroha_data_model::nexus::axt_ordered_transaction_set_digest_v1(
            (0..source.network_entrypoint_count())
                .map(|index| source.network_entrypoint_at(index).unwrap()),
        )
        .unwrap()
        .into(),
    };
    // The genuine transfer projection retains both occurrences. It must not
    // hide the gap by splitting the entry or synthesizing mint/burn transfers.
    let transfers = &block.fastpq_transcripts[&call];
    assert_eq!(transfers.len(), 2);
    let transfer_only_result = crate::fastpq::quantity_statement_from_finalized_transcripts(
        inputs,
        transfers,
        fastpq_prover::gadgets::public_transfer_statement::PublicTransferLimits::default(),
        fastpq_prover::gadgets::public_transfer_statement::TransferSmtBuildLimits::for_update_limit(8).unwrap(),
    );
    assert!(
        matches!(transfer_only_result, Err(fastpq_prover::Error::TransferInvariant { details }) if details.contains("repeated-key"))
    );

    // Public disposable fixture only. This test-only diagnostic frame does
    // not bypass require_complete or create a source/finality credential.
    // The orchestration owner authenticates executable/source/log custody
    // and retains these exact bytes for the separate prover test.
    let bytes = norito::encode_canonical(&(entry.wire().clone(), inputs)).unwrap();
    assert!(bytes.len() <= 1_048_576);
    let decoded: (
        FastpqExecutionEffectsV1,
        iroha_data_model::fastpq::FastpqPublicInputs,
    ) = norito::decode_canonical(&bytes).unwrap();
    assert_eq!(decoded, (entry.wire().clone(), inputs));
    println!(
        "IROHA_FASTPQ_GENUINE_EFFECT_CAPTURE_V1 {} {}",
        hex::encode(Hash::new(&bytes).as_ref()),
        hex::encode(bytes)
    );
}

/// Begin from a genuine signed effect and its retained source/quota owner.
fn with_signed_quantity_block(test: impl FnOnce(&mut StateBlock<'_>, &AssetId, &AssetId, Hash)) {
    let (state, alice, bob) = fixture();
    let (source, call) = source(
        &state,
        vec![Transfer::asset_quantity(alice.clone(), 1_u32, BOB_ID.clone()).into()],
    );
    let (mut block, _recording) = state
        .block_with_recorded_pristine_carrier_stage(
            &source,
            |_| Ok::<(), String>(()),
            |error| error,
        )
        .unwrap();
    block.reserve_ordinary_execution_outputs(&source).unwrap();
    block.execute_ordinary_output_plan(&source, None).unwrap();
    block.observe_quantity_block_journals();
    assert_eq!(block.fastpq_quantity_candidate.issue, None);
    test(&mut block, &alice, &bob, call);
}

fn prepare_owned_test_mint(
    transaction: &mut StateTransaction<'_, '_>,
    alice: &AssetId,
    call: Hash,
) -> PreparedQuantityCapture {
    transaction.tx_call_hash = Some(call);
    // Continue the already captured original entry under its exact retained route.
    // A raw default transaction has no lane and must not conflict with a routed source.
    let retained = transaction.block_fastpq_quantity_candidate.entries[&call]
        .context
        .entry;
    transaction.current_lane_id = match retained.route {
        iroha_data_model::fastpq::FastpqSourceRouteV1::Unrouted => None,
        iroha_data_model::fastpq::FastpqSourceRouteV1::Lane(lane) => Some(lane.lane_id),
    };
    transaction.current_dataspace_id = Some(retained.dataspace_id);
    transaction.world.current_dataspace_id = Some(retained.dataspace_id);
    let change = FastpqExecutionSupplyChangeV1 {
        balance: transaction.quantity_balance_identity(alice).unwrap(),
        amount: Quantity::from(1_u32),
        balance_before: Quantity::from(9_u32),
        balance_after: Quantity::from(10_u32),
        supply_before: Quantity::from(10_u32),
        supply_after: Quantity::from(11_u32),
    };
    transaction
        .prepare_quantity_candidate(
            &ALICE_ID,
            call,
            Hash::new(b"quantity-write-owner-regression"),
            vec![FastpqExecutionEffectKindV1::Mint(change)],
        )
        .expect("retained original source admits exact next mint")
}

fn apply_owned_test_mint(transaction: &mut StateTransaction<'_, '_>, alice: &AssetId) {
    assert!(
        transaction
            .world
            .quantity_mutation_observation
            .plan
            .is_some()
    );
    transaction
        .world
        .assign_quantity_balance_exact(alice, Quantity::from(10_u32))
        .unwrap();
    transaction
        .world
        .increase_asset_total_amount(alice.definition(), &Quantity::from(1_u32))
        .unwrap();
}

#[test]
fn original_signed_owner_typed_writes_and_state_application_match_mv_lineage() {
    with_signed_quantity_block(|block, alice, _, call| {
        let mut transaction = block.transaction();
        let prepared = prepare_owned_test_mint(&mut transaction, alice, call);
        transaction
            .apply_with_quantity_candidate(Ok(prepared), |transaction| {
                apply_owned_test_mint(transaction, alice);
                Ok(())
            })
            .unwrap();
        assert_eq!(transaction.pending_fastpq_quantity_candidate.issue, None);
        transaction.apply();
        block.observe_quantity_block_journals();
        assert_eq!(block.fastpq_quantity_candidate.issue, None);
        assert_eq!(
            block.fastpq_quantity_candidate.entries[&call].effects.len(),
            2
        );
        assert_exact_applied_measurement(&block.fastpq_quantity_candidate);
        assert_eq!(
            block.fastpq_quantity_candidate.require_complete(),
            Err(QuantityCaptureIssue::IncompleteCoverage)
        );
    });
}

#[test]
fn restored_raw_balance_and_supply_leases_inside_original_owned_callback_refuse_capture() {
    for supply in [false, true] {
        with_signed_quantity_block(|block, alice, _, call| {
            let mut transaction = block.transaction();
            let prepared = prepare_owned_test_mint(&mut transaction, alice, call);
            transaction
                .apply_with_quantity_candidate(Ok(prepared), |transaction| {
                    if supply {
                        let definition = transaction
                            .world
                            .asset_definition_mut(alice.definition())
                            .unwrap();
                        definition.total_quantity = Quantity::from(99_u32);
                        definition.total_quantity = Quantity::from(10_u32);
                    } else {
                        let raw = transaction.world.assets.get_mut(alice).unwrap();
                        **raw = Quantity::from(99_u32);
                        **raw = Quantity::from(9_u32);
                    }
                    apply_owned_test_mint(transaction, alice);
                    Ok(())
                })
                .unwrap();
            assert_eq!(
                transaction.pending_fastpq_quantity_candidate.issue,
                Some(QuantityCaptureIssue::UnownedMutation)
            );
            assert!(
                transaction
                    .pending_fastpq_quantity_candidate
                    .entries
                    .is_empty()
            );
            transaction.apply();
            block.observe_quantity_block_journals();
            assert_eq!(
                block.fastpq_quantity_candidate.issue,
                Some(QuantityCaptureIssue::UnownedMutation)
            );
            assert_eq!(
                block.world.assets.get(alice).unwrap().as_ref(),
                &Quantity::from(10_u32)
            );
            assert_eq!(
                block
                    .world
                    .asset_definition(alice.definition())
                    .unwrap()
                    .total_quantity(),
                &Quantity::from(11_u32)
            );
        });
    }
}

#[test]
fn callback_errors_before_and_after_exact_final_port_preserve_business_and_refuse_facts() {
    for write_first in [false, true] {
        with_signed_quantity_block(|block, alice, _, call| {
            let mut transaction = block.transaction();
            let prepared = prepare_owned_test_mint(&mut transaction, alice, call);
            let result: Result<(), Error> =
                transaction.apply_with_quantity_candidate(Ok(prepared), |transaction| {
                    if write_first {
                        apply_owned_test_mint(transaction, alice);
                    }
                    Err(Error::InvariantViolation("original callback error".into()))
                });
            assert!(result.is_err());
            assert!(!transaction.world.quantity_mutation_observation.owned);
            assert!(
                transaction
                    .world
                    .quantity_mutation_observation
                    .plan
                    .is_none()
            );
            assert_eq!(
                transaction.pending_fastpq_quantity_candidate.issue,
                Some(if write_first {
                    QuantityCaptureIssue::InterruptedScope
                } else {
                    QuantityCaptureIssue::InvalidFacts
                })
            );
            assert!(
                transaction
                    .pending_fastpq_quantity_candidate
                    .entries
                    .is_empty()
            );
            transaction.apply();
            block.observe_quantity_block_journals();
            assert!(block.fastpq_quantity_candidate.issue.is_some());
            assert_eq!(
                block.world.assets.get(alice).unwrap().as_ref(),
                &Quantity::from(if write_first { 10_u32 } else { 9 })
            );
            assert_eq!(
                block.fastpq_quantity_candidate.entries[&call].effects.len(),
                1
            );
        });
    }
}

#[test]
fn unwind_before_and_after_final_port_refuses_apply_but_complete_rollback_has_no_parent_poison() {
    for write_first in [false, true] {
        for apply_after_catch in [false, true] {
            with_signed_quantity_block(|block, alice, _, call| {
                let mut transaction = block.transaction();
                let prepared = prepare_owned_test_mint(&mut transaction, alice, call);
                let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let _: Result<(), Error> =
                        transaction.apply_with_quantity_candidate(Ok(prepared), |transaction| {
                            if write_first {
                                apply_owned_test_mint(transaction, alice);
                            }
                            panic!("interrupt original capture scope");
                        });
                }));
                assert!(failure.is_err());
                assert!(transaction.world.quantity_mutation_observation.owned);
                assert!(
                    transaction
                        .world
                        .quantity_mutation_observation
                        .plan
                        .is_some()
                );
                if apply_after_catch {
                    transaction.apply();
                } else {
                    drop(transaction);
                }
                block.observe_quantity_block_journals();
                assert_eq!(
                    block.fastpq_quantity_candidate.issue,
                    apply_after_catch.then_some(QuantityCaptureIssue::InterruptedScope)
                );
                assert_eq!(
                    block.world.assets.get(alice).unwrap().as_ref(),
                    &Quantity::from(if write_first && apply_after_catch {
                        10_u32
                    } else {
                        9
                    })
                );
                assert_eq!(
                    block.fastpq_quantity_candidate.entries[&call].effects.len(),
                    1
                );
            });
        }
    }
}

#[test]
fn nested_callback_cannot_reuse_outer_plan_or_authorize_outer_raw_writes() {
    with_signed_quantity_block(|block, alice, _, call| {
        let mut transaction = block.transaction();
        let outer = prepare_owned_test_mint(&mut transaction, alice, call);
        let inner = prepare_owned_test_mint(&mut transaction, alice, call);
        transaction
            .apply_with_quantity_candidate(Ok(outer), |transaction| {
                transaction.apply_with_quantity_candidate(Ok(inner), |transaction| {
                    apply_owned_test_mint(transaction, alice);
                    Ok(())
                })?;
                Ok(())
            })
            .unwrap();
        assert_eq!(
            transaction.pending_fastpq_quantity_candidate.issue,
            Some(QuantityCaptureIssue::InterruptedScope)
        );
        assert!(
            transaction
                .pending_fastpq_quantity_candidate
                .entries
                .is_empty()
        );
        drop(transaction);
        block.observe_quantity_block_journals();
        assert_eq!(block.fastpq_quantity_candidate.issue, None);
        assert_eq!(
            block.world.assets.get(alice).unwrap().as_ref(),
            &Quantity::from(9_u32)
        );
    });
}

#[test]
fn every_direct_world_block_quantity_write_survives_restore_and_refuses_capture() {
    for mutation in 0..8 {
        with_signed_quantity_block(|block, alice, bob, _| {
            match mutation {
                0 => {
                    let value = block.world.assets.get_mut(alice).unwrap();
                    **value = Quantity::from(99_u32);
                    **value = Quantity::from(9_u32);
                }
                1 => {
                    let original = block.world.assets.get(alice).unwrap().clone();
                    block.world.assets.insert(alice.clone(), original);
                }
                2 => {
                    let original = block.world.assets.remove(alice.clone()).unwrap();
                    block.world.assets.insert(alice.clone(), original);
                }
                3 => {
                    let absent = AssetId::with_scope(
                        bob.definition().clone(),
                        bob.account().clone(),
                        AssetBalanceScope::Dataspace(DataSpaceId::new(97)),
                    );
                    assert!(block.world.assets.remove(absent).is_none());
                }
                4 => {
                    let definition = block
                        .world
                        .asset_definitions
                        .get_mut(alice.definition())
                        .unwrap();
                    definition.total_quantity = Quantity::from(99_u32);
                    definition.total_quantity = Quantity::from(10_u32);
                }
                5 => {
                    let original = block
                        .world
                        .asset_definitions
                        .get(alice.definition())
                        .unwrap()
                        .clone();
                    block
                        .world
                        .asset_definitions
                        .insert(alice.definition().clone(), original);
                }
                6 => {
                    let original = block
                        .world
                        .asset_definitions
                        .remove(alice.definition().clone())
                        .unwrap();
                    block
                        .world
                        .asset_definitions
                        .insert(alice.definition().clone(), original);
                }
                7 => {
                    let absent = AssetDefinitionId::derive_from_components(
                        DomainId::try_new("absent-quantity-owner", "universal").unwrap(),
                        "absent".parse().unwrap(),
                    );
                    assert!(block.world.asset_definitions.remove(absent).is_none());
                }
                _ => unreachable!(),
            }
            assert_eq!(
                block.world.assets.get(alice).unwrap().as_ref(),
                &Quantity::from(9_u32)
            );
            assert_eq!(
                block
                    .world
                    .asset_definition(alice.definition())
                    .unwrap()
                    .total_quantity(),
                &Quantity::from(10_u32)
            );
            block.observe_quantity_block_journals();
            assert_eq!(
                block.fastpq_quantity_candidate.issue,
                Some(QuantityCaptureIssue::UnownedMutation)
            );
        });
    }
}

#[test]
fn raw_world_child_apply_is_detected_even_when_empty_and_rollback_never_increments_lineage() {
    for write in [false, true] {
        for apply in [false, true] {
            with_signed_quantity_block(|block, alice, _, _| {
                let mut world = block
                    .world
                    .transaction_without_telemetry(LaneConfig::default(), 0);
                if write {
                    let value = world.assets.get_mut(alice).unwrap();
                    **value = Quantity::from(99_u32);
                    **value = Quantity::from(9_u32);
                }
                if apply {
                    world.apply();
                } else {
                    drop(world);
                }
                block.observe_quantity_block_journals();
                assert_eq!(
                    block.fastpq_quantity_candidate.issue,
                    apply.then_some(QuantityCaptureIssue::UnownedMutation)
                );
                assert_eq!(
                    block.world.assets.get(alice).unwrap().as_ref(),
                    &Quantity::from(9_u32)
                );
            });
        }
    }
}

#[test]
fn original_finite_port_reservation_is_atomic_retained_and_returned_on_refusal_or_drop() {
    with_signed_quantity_block(|block, _, _, call| {
        let effects = &block.fastpq_quantity_candidate.entries[&call].effects;
        let budget = iroha_allocation::AllocationBudget::new(1_048_576);
        let plan = QuantityWritePlan::from_effects(effects, 2, &budget).unwrap();
        let required = budget.reserved_bytes();
        assert!(required > 0);
        assert_eq!(budget.peak_reserved_bytes(), required);
        drop(plan);
        assert_eq!(budget.reserved_bytes(), 0);
        for limit in [0, required - 1] {
            let small = iroha_allocation::AllocationBudget::new(limit);
            assert!(matches!(
                QuantityWritePlan::from_effects(effects, 2, &small),
                Err(super::super::fastpq_quantity_write_plan::QuantityWritePlanError::Capacity)
            ));
            assert_eq!(small.reserved_bytes(), 0);
            assert_eq!(
                small.peak_reserved_bytes(),
                0,
                "refusal occurs before any plan allocation"
            );
        }
        let exact = iroha_allocation::AllocationBudget::new(required);
        let retained = QuantityWritePlan::from_effects(effects, 2, &exact).unwrap();
        assert_eq!(exact.reserved_bytes(), required);
        assert!(QuantityWritePlan::from_effects(effects, 2, &exact).is_err());
        assert_eq!(exact.reserved_bytes(), required);
        assert!(QuantityWritePlan::from_effects(effects, 1, &budget).is_err());
        assert_eq!(budget.reserved_bytes(), 0);
        drop(retained);
        assert_eq!(exact.reserved_bytes(), 0);
    });
}

#[test]
fn signed_zero_transfer_rejection_rolls_back_prior_capture_and_all_following_quantity_work() {
    let (state, alice, bob) = fixture();
    let (source, _) = source(
        &state,
        vec![
            Transfer::asset_quantity(alice.clone(), 5_u32, ALICE_ID.clone()).into(),
            Transfer::asset_quantity(alice.clone(), 0_u32, BOB_ID.clone()).into(),
            Burn::asset_quantity(10_u32, alice.clone()).into(),
            Mint::asset_quantity(3_u32, alice.clone()).into(),
            Transfer::asset_quantity(alice.clone(), 3_u32, BOB_ID.clone()).into(),
        ],
    );
    let (mut block, _recording) = state
        .block_with_recorded_pristine_carrier_stage(
            &source,
            |_| Ok::<(), String>(()),
            |error| error,
        )
        .unwrap();
    let fragments = block.committed_fragment_count();
    block.reserve_ordinary_execution_outputs(&source).unwrap();
    block.execute_ordinary_output_plan(&source, None).unwrap();
    let outputs = block.retained_execution_outputs_for_test().unwrap();
    let iroha_data_model::block::execution_output::ExecutionOutputV1::Network(output) = &outputs[0]
    else {
        panic!("original rejected source must retain its Network row");
    };
    assert!(
        matches!(output.result.as_ref(), Err(
        iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
            iroha_data_model::ValidationFail::InstructionFailed(
                iroha_data_model::isi::error::InstructionExecutionError::InvariantViolation(reason)
            )
        )
    ) if reason.as_ref() == "asset transfer amount must be non-zero"),
        "original rejection: {:?}",
        output.result
    );
    assert!(output.result.nexus_fee_receipt().is_none());
    assert!(output.result.batch_transfer_outcomes().is_empty());
    assert!(output.completions.is_empty());
    assert_eq!(block.committed_fragment_count(), fragments);
    block.observe_quantity_block_journals();
    assert_eq!(block.fastpq_quantity_candidate.issue, None);
    assert!(block.fastpq_quantity_candidate.entries.is_empty());
    assert_eq!(
        block.fastpq_quantity_candidate.usage,
        QuantityCandidateUsage::default()
    );
    assert_eq!(
        block.world.assets.get(&alice).unwrap().as_ref(),
        &Quantity::from(10_u32)
    );
    assert!(block.world.assets.get(&bob).is_none());
    assert_eq!(
        block
            .world
            .asset_definition(alice.definition())
            .unwrap()
            .total_quantity(),
        &Quantity::from(10_u32)
    );
    assert_eq!(
        block.fastpq_quantity_candidate.require_complete(),
        Err(QuantityCaptureIssue::IncompleteCoverage)
    );
}

#[test]
fn genuine_self_zero_exhaustion_burn_and_recreation_consume_exact_ordered_ports() {
    let (state, alice, bob) = fixture();
    let (source, call) = source(
        &state,
        vec![
            Transfer::asset_quantity(alice.clone(), 5_u32, ALICE_ID.clone()).into(),
            Burn::asset_quantity(0_u32, alice.clone()).into(),
            Burn::asset_quantity(10_u32, alice.clone()).into(),
            Mint::asset_quantity(3_u32, alice.clone()).into(),
            Transfer::asset_quantity(alice.clone(), 3_u32, BOB_ID.clone()).into(),
        ],
    );
    let (mut block, _recording) = state
        .block_with_recorded_pristine_carrier_stage(
            &source,
            |_| Ok::<(), String>(()),
            |error| error,
        )
        .unwrap();
    block.reserve_ordinary_execution_outputs(&source).unwrap();
    block.execute_ordinary_output_plan(&source, None).unwrap();
    let outputs = block.retained_execution_outputs_for_test().unwrap();
    let iroha_data_model::block::execution_output::ExecutionOutputV1::Network(output) = &outputs[0]
    else {
        panic!("original successful source must retain its Network row");
    };
    assert!(
        output.result.is_ok(),
        "original source: {:?}",
        output.result
    );
    block.observe_quantity_block_journals();
    assert_eq!(block.fastpq_quantity_candidate.issue, None);
    assert_eq!(
        block.fastpq_quantity_candidate.entries[&call].effects.len(),
        5
    );
    let FastpqExecutionEffectKindV1::Burn(zero) =
        &block.fastpq_quantity_candidate.entries[&call].effects[1].kind
    else {
        panic!("zero burn must retain its exact original operation");
    };
    assert_eq!(zero.amount, Quantity::zero());
    assert_eq!(zero.balance_before, Quantity::from(10_u32));
    assert_eq!(zero.balance_after, Quantity::from(10_u32));
    assert_eq!(zero.supply_before, Quantity::from(10_u32));
    assert_eq!(zero.supply_after, Quantity::from(10_u32));
    assert_exact_applied_measurement(&block.fastpq_quantity_candidate);
    assert!(block.world.assets.get(&alice).is_none());
    assert_eq!(
        block.world.assets.get(&bob).unwrap().as_ref(),
        &Quantity::from(3_u32)
    );
    assert_eq!(
        block
            .world
            .asset_definition(alice.definition())
            .unwrap()
            .total_quantity(),
        &Quantity::from(3_u32)
    );
    assert_eq!(
        block.fastpq_quantity_candidate.require_complete(),
        Err(QuantityCaptureIssue::IncompleteCoverage)
    );
}

#[test]
fn charged_complete_tape_preserves_exact_native_frame_and_refuses_before_clone_without_credit() {
    with_signed_quantity_block(|block, _, _, call| {
        let entry = &block.fastpq_quantity_candidate.entries[&call];
        let budget = iroha_allocation::AllocationBudget::new(1_048_576);
        let tape = QuantityTape::prepare(
            entry.context,
            &entry.effects,
            &[],
            Hash::new(b"unused"),
            Hash::new(b"unused"),
            entry.effects.len(),
            &budget,
        )
        .unwrap();
        assert_eq!(
            norito::encode_canonical(tape.wire()).unwrap(),
            norito::encode_canonical(entry.wire()).unwrap()
        );
        let required = budget.reserved_bytes();
        assert!(required > 0);
        assert_eq!(budget.peak_reserved_bytes(), required);
        drop(tape);
        assert_eq!(budget.reserved_bytes(), 0);
        let limited = iroha_allocation::AllocationBudget::new(required - 1);
        assert!(matches!(
            QuantityTape::prepare(
                entry.context,
                &entry.effects,
                &[],
                Hash::new(b"unused"),
                Hash::new(b"unused"),
                entry.effects.len(),
                &limited
            ),
            Err(QuantityCaptureIssue::Capacity)
        ));
        assert_eq!(limited.reserved_bytes(), 0);
        assert_eq!(limited.peak_reserved_bytes(), 0);
        let mut malformed = entry.effects.clone();
        malformed[0].ordinal = 1;
        assert!(matches!(
            QuantityTape::prepare(
                entry.context,
                &malformed,
                &[],
                Hash::new(b"unused"),
                Hash::new(b"unused"),
                malformed.len(),
                &budget
            ),
            Err(QuantityCaptureIssue::InvalidFacts)
        ));
        assert_eq!(budget.reserved_bytes(), 0);
    });
}

#[test]
fn capture_preparation_and_transaction_rollback_return_every_archive_and_projection_credit() {
    with_signed_quantity_block(|block, alice, _, call| {
        let budget = block.pipeline_ivm_prepared_cache.execution_budget().clone();
        let original_parent = budget.reserved_bytes();
        let mut transaction = block.transaction();
        let original = budget.reserved_bytes();
        let prepared = prepare_owned_test_mint(&mut transaction, alice, call);
        assert!(budget.reserved_bytes() > original);
        drop(prepared);
        assert_eq!(budget.reserved_bytes(), original);
        let prepared = prepare_owned_test_mint(&mut transaction, alice, call);
        transaction
            .apply_with_quantity_candidate(Ok(prepared), |transaction| {
                assert!(budget.reserved_bytes() > original);
                apply_owned_test_mint(transaction, alice);
                Ok(())
            })
            .unwrap();
        assert_eq!(transaction.pending_fastpq_quantity_candidate.issue, None);
        assert!(
            budget.reserved_bytes() > original,
            "pending full tape and pre-admitted parent slots retain original credits"
        );
        drop(transaction);
        assert_eq!(budget.reserved_bytes(), original_parent);
        block.observe_quantity_block_journals();
        assert_eq!(block.fastpq_quantity_candidate.issue, None);
        assert_eq!(
            block.fastpq_quantity_candidate.entries[&call].effects.len(),
            1
        );
    });
}

#[test]
fn nested_canonical_payloads_remain_charged_through_unwind_and_release_after_destruction() {
    with_signed_quantity_block(|block, _, _, call| {
        let entry = &block.fastpq_quantity_candidate.entries[&call];
        let budget = iroha_allocation::AllocationBudget::new(1_048_576);
        let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let tape = QuantityTape::prepare(
                entry.context,
                &entry.effects,
                &[],
                Hash::new(b"unused"),
                Hash::new(b"unused"),
                entry.effects.len(),
                &budget,
            )
            .unwrap();
            assert_eq!(tape.wire(), entry.wire());
            assert!(
                budget.reserved_bytes() > 0,
                "original nested controller and quantity owners retain their credits"
            );
            panic!("unwind while exact nested canonical owners are retained");
        }));
        assert!(failure.is_err());
        assert_eq!(budget.reserved_bytes(), 0);
        assert!(budget.peak_reserved_bytes() > 0);
    });
}

#[test]
fn callback_error_and_unwind_discard_new_tape_while_transaction_rollback_preserves_parent() {
    for unwind in [false, true] {
        with_signed_quantity_block(|block, alice, _, call| {
            let budget = block.pipeline_ivm_prepared_cache.execution_budget().clone();
            let parent_bytes = budget.reserved_bytes();
            let mut transaction = block.transaction();
            let prepared = prepare_owned_test_mint(&mut transaction, alice, call);
            let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let result: Result<(), Error> =
                    transaction.apply_with_quantity_candidate(Ok(prepared), |transaction| {
                        apply_owned_test_mint(transaction, alice);
                        if unwind {
                            panic!("interrupt after final original port");
                        }
                        Err(Error::InvariantViolation(
                            "return after final original port".into(),
                        ))
                    });
                assert!(result.is_err());
            }));
            assert_eq!(failure.is_err(), unwind);
            assert!(
                transaction
                    .pending_fastpq_quantity_candidate
                    .entries
                    .is_empty()
            );
            assert_eq!(
                transaction
                    .world
                    .quantity_mutation_observation
                    .plan
                    .is_some(),
                unwind
            );
            drop(transaction);
            assert_eq!(budget.reserved_bytes(), parent_bytes);
            block.observe_quantity_block_journals();
            assert_eq!(block.fastpq_quantity_candidate.issue, None);
            assert_eq!(
                block.fastpq_quantity_candidate.entries[&call].effects.len(),
                1
            );
            assert_eq!(
                block.world.assets.get(alice).unwrap().as_ref(),
                &Quantity::from(9_u32)
            );
        });
    }
}

#[test]
fn complete_charged_replacement_moves_to_original_parent_before_world_publication() {
    with_signed_quantity_block(|block, alice, _, call| {
        let mut transaction = block.transaction();
        let prepared = prepare_owned_test_mint(&mut transaction, alice, call);
        transaction
            .apply_with_quantity_candidate(Ok(prepared), |transaction| {
                apply_owned_test_mint(transaction, alice);
                Ok(())
            })
            .unwrap();
        let complete = norito::encode_canonical(
            transaction.pending_fastpq_quantity_candidate.entries[&call].wire(),
        )
        .unwrap();
        transaction.apply();
        block.observe_quantity_block_journals();
        assert_eq!(block.fastpq_quantity_candidate.issue, None);
        assert_eq!(
            block.fastpq_quantity_candidate.entries[&call].effects.len(),
            2
        );
        assert_eq!(
            norito::encode_canonical(block.fastpq_quantity_candidate.entries[&call].wire())
                .unwrap(),
            complete
        );
        assert_exact_applied_measurement(&block.fastpq_quantity_candidate);
        assert_eq!(
            block.fastpq_quantity_candidate.require_complete(),
            Err(QuantityCaptureIssue::IncompleteCoverage)
        );
    });
}

#[test]
fn genuine_signed_quantity_execution_publishes_original_world_with_capture_gate_still_closed() {
    let (state, alice, bob) = fixture();
    let (source, call) = source(
        &state,
        vec![Transfer::asset_quantity(alice.clone(), 1_u32, BOB_ID.clone()).into()],
    );
    let (mut block, _recording) = state
        .block_with_recorded_pristine_carrier_stage(
            &source,
            |_| Ok::<(), String>(()),
            |error| error,
        )
        .unwrap();
    block.reserve_ordinary_execution_outputs(&source).unwrap();
    block.execute_ordinary_output_plan(&source, None).unwrap();
    block.observe_quantity_block_journals();
    assert_eq!(block.fastpq_quantity_candidate.issue, None);
    assert_eq!(
        block.fastpq_quantity_candidate.entries[&call].effects.len(),
        1
    );
    assert_eq!(
        block.fastpq_quantity_candidate.require_complete(),
        Err(QuantityCaptureIssue::IncompleteCoverage)
    );
    // This explicitly test-owned component publication does not mint a finality credential.
    block.commit_world_overlay_for_testing().unwrap();
    let view = state.view();
    assert_eq!(
        view.world.assets.get(&alice).unwrap().as_ref(),
        &Quantity::from(9_u32)
    );
    assert_eq!(
        view.world.assets.get(&bob).unwrap().as_ref(),
        &Quantity::from(1_u32)
    );
    assert_eq!(
        view.world
            .asset_definition(alice.definition())
            .unwrap()
            .total_quantity(),
        &Quantity::from(10_u32)
    );
}

#[test]
fn borrowed_effect_inputs_preserve_every_owned_canonical_frame_and_exact_original_admission() {
    with_signed_quantity_block(|block, _, _, call| {
        let entry = &block.fastpq_quantity_candidate.entries[&call];
        let FastpqExecutionEffectKindV1::Transfer(original) = &entry.effects[0].kind else {
            panic!("genuine source supplies transfer identities")
        };
        let amount: Quantity = "18446744073709551616.0000000000000000000000000001"
            .parse()
            .unwrap();
        let before: Quantity = "36893488147419103232.0000000000000000000000000002"
            .parse()
            .unwrap();
        let zero = Quantity::zero();
        let transfer = FastpqExecutionTransferV1 {
            source: original.source.clone(),
            destination: original.destination.clone(),
            amount: amount.clone(),
            source_before: before.clone(),
            source_after: amount.clone(),
            destination_before: zero.clone(),
            destination_after: amount.clone(),
        };
        let mint = FastpqExecutionSupplyChangeV1 {
            balance: original.source.clone(),
            amount: amount.clone(),
            balance_before: amount.clone(),
            balance_after: before.clone(),
            supply_before: before.clone(),
            supply_after: before.checked_add(&amount).unwrap(),
        };
        let burn = FastpqExecutionSupplyChangeV1 {
            balance: original.destination.clone(),
            amount: amount.clone(),
            balance_before: amount.clone(),
            balance_after: zero,
            supply_before: mint.supply_after.clone(),
            supply_after: before,
        };
        let kinds = [
            FastpqExecutionEffectKindV1::Transfer(transfer),
            FastpqExecutionEffectKindV1::Mint(mint),
            FastpqExecutionEffectKindV1::Burn(burn),
        ];
        let authority = entry.effects[0].authority_digest;
        let authorization = entry.effects[0].authorization_context;
        let owned = FastpqExecutionEffectsV1 {
            context: entry.context,
            effects: kinds
                .iter()
                .enumerate()
                .map(|(ordinal, kind)| FastpqExecutionEffectV1 {
                    ordinal: u32::try_from(ordinal).unwrap(),
                    authority_digest: authority,
                    authorization_context: authorization,
                    kind: kind.clone(),
                })
                .collect(),
        };
        let expected = norito::encode_canonical(&owned).unwrap();
        for prefix_len in 0..=kinds.len() {
            let budget = iroha_allocation::AllocationBudget::new(1_048_576);
            let tape = QuantityTape::prepare_inputs(
                entry.context,
                &owned.effects[..prefix_len],
                kinds[prefix_len..].iter().map(|kind| Ok(kind.into())),
                authority,
                authorization,
                kinds.len(),
                &budget,
            )
            .unwrap();
            assert_eq!(norito::encode_canonical(tape.wire()).unwrap(), expected);
            let required = budget.reserved_bytes();
            assert!(required > 0);
            drop(tape);
            assert_eq!(budget.reserved_bytes(), 0);
            let limited = iroha_allocation::AllocationBudget::new(required - 1);
            assert!(matches!(
                QuantityTape::prepare_inputs(
                    entry.context,
                    &owned.effects[..prefix_len],
                    kinds[prefix_len..].iter().map(|kind| Ok(kind.into())),
                    authority,
                    authorization,
                    kinds.len(),
                    &limited,
                ),
                Err(QuantityCaptureIssue::Capacity)
            ));
            assert_eq!(limited.peak_reserved_bytes(), 0);
            assert_eq!(limited.reserved_bytes(), 0);
        }
        let budget = iroha_allocation::AllocationBudget::new(1_048_576);
        assert!(matches!(
            QuantityTape::prepare_inputs(
                entry.context,
                &[],
                std::iter::once(Err(QuantityCaptureIssue::MissingIncarnation)),
                authority,
                authorization,
                1,
                &budget,
            ),
            Err(QuantityCaptureIssue::MissingIncarnation)
        ));
        assert_eq!(budget.peak_reserved_bytes(), 0);
    });
}

#[test]
fn genuine_wide_decimal_supply_and_transfer_retain_original_values_without_narrowing() {
    let (state, alice, bob) = fixture();
    let amount: Quantity = "18446744073709551616.0000000000000000000000000001"
        .parse()
        .unwrap();
    let tiny: Quantity = "0.0000000000000000000000000001".parse().unwrap();
    let expected_after_mint = Quantity::from(10_u32).checked_add(&amount).unwrap();
    let expected_after_burn = expected_after_mint.checked_sub(&tiny).unwrap();
    let (source, call) = source(
        &state,
        vec![
            Mint::asset_quantity(amount.clone(), alice.clone()).into(),
            Burn::asset_quantity(tiny.clone(), alice.clone()).into(),
            Transfer::asset_quantity(alice.clone(), amount.clone(), BOB_ID.clone()).into(),
        ],
    );
    let (mut block, _recording) = state
        .block_with_recorded_pristine_carrier_stage(
            &source,
            |_| Ok::<(), String>(()),
            |error| error,
        )
        .unwrap();
    block.reserve_ordinary_execution_outputs(&source).unwrap();
    block.execute_ordinary_output_plan(&source, None).unwrap();
    block.observe_quantity_block_journals();
    assert_eq!(block.fastpq_quantity_candidate.issue, None);
    let entry = &block.fastpq_quantity_candidate.entries[&call];
    assert_eq!(entry.effects.len(), 3);
    let FastpqExecutionEffectKindV1::Mint(mint) = &entry.effects[0].kind else {
        panic!("mint")
    };
    assert_eq!(mint.amount, amount);
    assert_eq!(mint.balance_before, Quantity::from(10_u32));
    assert_eq!(mint.balance_after, expected_after_mint);
    assert_eq!(mint.supply_before, Quantity::from(10_u32));
    assert_eq!(mint.supply_after, expected_after_mint);
    let FastpqExecutionEffectKindV1::Burn(burn) = &entry.effects[1].kind else {
        panic!("burn")
    };
    assert_eq!(burn.amount, tiny);
    assert_eq!(burn.balance_before, expected_after_mint);
    assert_eq!(burn.balance_after, expected_after_burn);
    assert_eq!(burn.supply_before, expected_after_mint);
    assert_eq!(burn.supply_after, expected_after_burn);
    let FastpqExecutionEffectKindV1::Transfer(transfer) = &entry.effects[2].kind else {
        panic!("transfer")
    };
    assert_eq!(transfer.amount, amount);
    assert_eq!(transfer.source_before, expected_after_burn);
    assert_eq!(
        &transfer.source_after,
        block.world.assets.get(&alice).unwrap().as_ref()
    );
    assert_eq!(
        &transfer.destination_after,
        block.world.assets.get(&bob).unwrap().as_ref()
    );
    assert_eq!(
        block
            .world
            .asset_definition(alice.definition())
            .unwrap()
            .total_quantity(),
        &expected_after_burn
    );
    assert_exact_applied_measurement(&block.fastpq_quantity_candidate);
    assert_eq!(
        block.fastpq_quantity_candidate.require_complete(),
        Err(QuantityCaptureIssue::IncompleteCoverage)
    );
}

fn with_sealed_quantity_source_census(
    action: impl FnOnce(&mut StateBlock<'_>, &SignedBlock, &AssetId, &AssetId),
) {
    use iroha_data_model::{isi::Log, prelude::Level};
    let (state, alice, bob) = fixture();
    let header = quantity_successor_header(&state);
    let mut builder = BlockBuilder::new(header);
    let mut contexts = Vec::new();
    let bodies: [Vec<InstructionBox>; 3] = [
        vec![Transfer::asset_quantity(alice.clone(), 1_u32, BOB_ID.clone()).into()],
        vec![Log::new(Level::INFO, "quantity census empty source".to_owned()).into()],
        vec![Burn::asset_quantity(99_u32, alice.clone()).into()],
    ];
    for body in bodies {
        let mut transaction = TransactionBuilder::new(
            state.network_id,
            ALICE_ID.clone(),
            FeePaymentIntent::authority(vec![], None),
        );
        transaction.set_creation_time(header.creation_time() - Duration::from_millis(1));
        let signed = transaction
            .with_instructions(body)
            .sign(ALICE_KEYPAIR.private_key());
        contexts.push(quantity_execution_context(&state, &signed, &header));
        builder.push_transaction(signed);
    }
    builder.set_execution_context(Some(BlockExecutionContextBundle::new(contexts)));
    let mut source = builder.build_with_signature(0, ALICE_KEYPAIR.private_key());
    let (mut block, _recording) = state
        .block_with_recorded_pristine_carrier_stage(
            &source,
            |_| Ok::<(), String>(()),
            |error| error,
        )
        .unwrap();
    block.reserve_ordinary_execution_outputs(&source).unwrap();
    block.execute_ordinary_output_plan(&source, None).unwrap();
    let rows = block.retained_execution_outputs_for_test().unwrap();
    assert_eq!(rows.len(), 3);
    assert!(rows[0].result().is_ok());
    assert!(rows[1].result().is_ok());
    assert!(rows[2].result().is_err());
    block
        .seal_execution_outputs(&mut source, |state, _, routes| {
            assert_eq!(routes.len(), 3);
            Ok::<_, String>(crate::state::output_capacity::ExecutionOutputSealMetadata {
                committed_fragment_count: u64::try_from(state.committed_fragment_count()).unwrap(),
            })
        })
        .unwrap();
    block.observe_quantity_block_journals();
    assert_eq!(block.fastpq_quantity_candidate.issue, None);
    assert!(matches!(
        block.fastpq_quantity_candidate.source_census,
        super::source_census::QuantitySourceCensusState::Sealed(_)
    ));
    action(&mut block, &source, &alice, &bob);
}

#[test]
fn original_quantity_census_retains_ordered_success_empty_and_rejected_sources_with_native_bytes() {
    with_sealed_quantity_source_census(|block, source, alice, bob| {
        let super::source_census::QuantitySourceCensusState::Sealed(census) =
            &block.fastpq_quantity_candidate.source_census
        else {
            panic!("original census retained");
        };
        assert_eq!(census.entries().len(), 3);
        for (index, row) in census.entries().iter().enumerate() {
            assert_eq!(row.context.source.network_id, block.network_id);
            assert_eq!(row.context.source.height, source.header().height().get());
            assert_eq!(
                row.context.entry.entry_hash,
                Hash::from(
                    source
                        .network_entrypoint_at(index)
                        .unwrap()
                        .execution_call_hash()
                )
            );
            let empty = FastpqExecutionEffectsV1 {
                context: row.context,
                effects: Vec::new(),
            };
            let wire = block
                .fastpq_quantity_candidate
                .entries
                .get(&row.context.entry.entry_hash)
                .map_or(&empty, QuantityArchivedEntry::wire);
            let native = norito::encode_canonical(wire).unwrap();
            assert_eq!(row.effects_digest, Hash::new(&native));
            assert_eq!(row.frame_bytes, u64::try_from(native.len()).unwrap());
            assert_eq!(row.effect_count, if index == 0 { 1 } else { 0 });
        }
        assert_eq!(
            block.world.assets.get(alice).unwrap().as_ref(),
            &Quantity::from(9_u32)
        );
        assert_eq!(
            block.world.assets.get(bob).unwrap().as_ref(),
            &Quantity::from(1_u32)
        );
        assert_eq!(block.fastpq_quantity_candidate.usage.entries, 1);
        assert_eq!(
            block.fastpq_quantity_candidate.require_complete(),
            Err(QuantityCaptureIssue::IncompleteCoverage)
        );
    });
}

#[test]
fn original_quantity_census_reserves_exact_fixed_backing_and_returns_credit_on_refusal_and_unwind()
{
    use super::source_census::{QuantitySourceCensus, QuantitySourceEntrySeal};
    with_sealed_quantity_source_census(|block, _, _, _| {
        let required = std::alloc::Layout::array::<QuantitySourceEntrySeal>(3)
            .unwrap()
            .size();
        assert!(required > 0);
        let insufficient = iroha_allocation::AllocationBudget::new(required - 1);
        assert!(matches!(
            QuantitySourceCensus::prepare(block, &insufficient),
            Err(QuantityCaptureIssue::Capacity)
        ));
        assert_eq!(insufficient.reserved_bytes(), 0);
        assert_eq!(insufficient.peak_reserved_bytes(), 0);
        let exact = iroha_allocation::AllocationBudget::new(required);
        let retained = QuantitySourceCensus::prepare(block, &exact).unwrap();
        assert_eq!(exact.reserved_bytes(), required);
        assert!(matches!(
            QuantitySourceCensus::prepare(block, &exact),
            Err(QuantityCaptureIssue::Capacity)
        ));
        assert_eq!(exact.reserved_bytes(), required);
        assert_eq!(retained.entries().len(), 3);
        drop(retained);
        assert_eq!(exact.reserved_bytes(), 0);
        let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let retained = QuantitySourceCensus::prepare(block, &exact).unwrap();
            assert_eq!(retained.entries().len(), 3);
            assert_eq!(exact.reserved_bytes(), required);
            panic!("drop original census backing during unwind");
        }));
        assert!(unwind.is_err());
        assert_eq!(exact.reserved_bytes(), 0);
        block.observe_quantity_block_journals();
        assert_eq!(block.fastpq_quantity_candidate.issue, None);
        let original_pool = block.pipeline_ivm_prepared_cache.execution_budget().clone();
        let original_before = original_pool.reserved_bytes();
        block.reject_quantity_source_census();
        assert_eq!(original_pool.reserved_bytes() + required, original_before);
        assert_eq!(
            block.fastpq_quantity_candidate.issue,
            Some(QuantityCaptureIssue::UnsupportedOwner)
        );
    });
}

#[test]
fn original_quantity_census_refuses_source_reconstruction_context_change_and_missing_effects() {
    for mutation in 0..5 {
        with_sealed_quantity_source_census(|block, _, _, _| {
            match mutation {
                0 => {
                    let original = block
                        .fastpq_source_inventory
                        .as_ref()
                        .unwrap()
                        .as_ref()
                        .unwrap();
                    block.fastpq_source_inventory = Some(Ok(Arc::new((**original).clone())));
                }
                1 => {
                    let original = block.fastpq_source_context.as_ref().unwrap();
                    block.fastpq_source_context = Some(Arc::new((**original).clone()));
                }
                2 => block._curr_block.creation_time_ms += 1,
                3 => {
                    block.fastpq_quantity_candidate.entries =
                        super::super::fastpq_quantity_archive::QuantityArchiveMap::default()
                }
                4 => block
                    .fastpq_quantity_candidate
                    .entries
                    .for_each_mut(|entry| {
                        entry.tape.substitute_entry_hash_for_test(Hash::new(
                            b"substituted quantity entry context",
                        ));
                    }),
                _ => unreachable!(),
            }
            block.observe_quantity_block_journals();
            assert!(block.fastpq_quantity_candidate.issue.is_some());
            assert!(matches!(
                block.fastpq_quantity_candidate.source_census,
                super::source_census::QuantitySourceCensusState::Failed
            ));
            block.retain_quantity_source_census();
            assert!(matches!(
                block.fastpq_quantity_candidate.source_census,
                super::source_census::QuantitySourceCensusState::Failed
            ));
            assert!(block.fastpq_quantity_candidate.require_complete().is_err());
        });
    }
}

#[test]
fn original_quantity_census_keeps_dropped_child_but_refuses_late_apply_and_restored_raw_write() {
    for mutation in 0..3 {
        with_sealed_quantity_source_census(|block, _, alice, _| {
            match mutation {
                0 | 1 => {
                    let mut child = block
                        .world
                        .transaction_without_telemetry(LaneConfig::default(), 0);
                    child.account_mut(&ALICE_ID).unwrap().insert(
                        "after_census".parse().unwrap(),
                        iroha_primitives::json::Json::new(true),
                    );
                    let mut pending = QuantityCandidateArchive::default();
                    pending.observe(&child);
                    if mutation == 0 {
                        drop(child);
                        drop(pending);
                    } else {
                        child.apply();
                        block.fastpq_quantity_candidate.apply(pending);
                    }
                }
                2 => {
                    let original = block.world.assets.get_mut(alice).unwrap();
                    **original = Quantity::from(99_u32);
                    **original = Quantity::from(9_u32);
                }
                _ => unreachable!(),
            }
            block.observe_quantity_block_journals();
            assert_eq!(
                block.world.assets.get(alice).unwrap().as_ref(),
                &Quantity::from(9_u32)
            );
            if mutation == 0 {
                assert_eq!(block.fastpq_quantity_candidate.issue, None);
                assert!(matches!(
                    block.fastpq_quantity_candidate.source_census,
                    super::source_census::QuantitySourceCensusState::Sealed(_)
                ));
            } else {
                assert!(block.fastpq_quantity_candidate.issue.is_some());
                assert!(matches!(
                    block.fastpq_quantity_candidate.source_census,
                    super::source_census::QuantitySourceCensusState::Failed
                ));
            }
            assert!(block.fastpq_quantity_candidate.require_complete().is_err());
        });
    }
}

#[test]
fn original_quantity_census_interrupted_preparation_latch_cannot_reseal_or_recover() {
    with_sealed_quantity_source_census(|block, _, _, _| {
        // Inject the exact retained state left by a caught unwind after original
        // construction enters Preparing. The separate owner test proves credit Drop.
        block.fastpq_quantity_candidate.source_census =
            super::source_census::QuantitySourceCensusState::Preparing;
        assert_eq!(block.fastpq_quantity_candidate.issue, None);
        block.observe_quantity_block_journals();
        assert_eq!(
            block.fastpq_quantity_candidate.issue,
            Some(QuantityCaptureIssue::InterruptedScope)
        );
        assert!(matches!(
            block.fastpq_quantity_candidate.source_census,
            super::source_census::QuantitySourceCensusState::Failed
        ));
        block.retain_quantity_source_census();
        block.observe_quantity_block_journals();
        assert_eq!(
            block.fastpq_quantity_candidate.issue,
            Some(QuantityCaptureIssue::InterruptedScope)
        );
        assert!(matches!(
            block.fastpq_quantity_candidate.source_census,
            super::source_census::QuantitySourceCensusState::Failed
        ));
        assert_eq!(
            block.fastpq_quantity_candidate.require_complete(),
            Err(QuantityCaptureIssue::InterruptedScope)
        );
    });
}
