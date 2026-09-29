//! Actual owner capture, rollback and explicit refusal of incomplete quantity coverage.

use super::*;
use crate::{
    governance::manifest::{LaneManifestRegistry, LaneManifestStatus},
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::Execute,
};
use iroha_data_model::{
    account::Account,
    asset::{AssetBalancePolicy, AssetBalanceScope, AssetDefinition},
    block::builder::BlockBuilder,
    domain::Domain,
    isi::{Burn, Mint, Register, Transfer, Unregister},
    prelude::{InstructionBox, TransactionBuilder},
    transaction::FeePaymentIntent,
};
use iroha_model_base::domain::DomainId;
use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR, BOB_ID};
use std::{num::NonZeroU64, time::Duration};

fn on_stack(test: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .name("quantity-candidate".into())
        .stack_size(32 * 1024 * 1024)
        .spawn(test)
        .unwrap()
        .join()
        .unwrap();
}

fn fixture() -> (State, AssetId, AssetId) {
    let mut state = State::new_for_testing(
        World::default(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let statuses = state
        .nexus_snapshot()
        .lane_catalog
        .lanes()
        .iter()
        .map(|lane| {
            (
                lane.id,
                LaneManifestStatus {
                    lane: lane.id,
                    alias: lane.alias.clone(),
                    dataspace: lane.dataspace_id,
                    visibility: lane.visibility,
                    storage: lane.storage,
                    governance: None,
                    manifest_path: None,
                    governance_rules: None,
                    privacy_commitments: Vec::new(),
                },
            )
        })
        .collect();
    state.install_lane_manifests_for_testing(&Arc::new(LaneManifestRegistry::from_statuses(
        statuses,
    )));
    let fees = &mut state.nexus.get_mut().fees;
    fees.base_fee = Quantity::zero();
    fees.per_byte_fee = Quantity::zero();
    fees.per_instruction_fee = Quantity::zero();
    fees.per_gas_unit_fee = Quantity::zero();
    let domain = DomainId::try_new("quantity-capture", "universal").unwrap();
    let definition =
        AssetDefinitionId::derive_from_components(domain.clone(), "units".parse().unwrap());
    let alice = AssetId::of(definition.clone(), ALICE_ID.clone());
    let bob = AssetId::of(definition.clone(), BOB_ID.clone());
    {
        let mut setup = state.block(BlockHeader::new(NonZeroU64::MIN, None, None, 1, 0));
        let mut transaction = setup.transaction();
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
    (state, alice, bob)
}

fn source(state: &State, body: Vec<InstructionBox>) -> (SignedBlock, Hash) {
    source_with_fee(state, body, FeePaymentIntent::authority(vec![], None))
}

fn source_with_fee(
    state: &State,
    body: Vec<InstructionBox>,
    fee: FeePaymentIntent,
) -> (SignedBlock, Hash) {
    let mut transaction = TransactionBuilder::new(state.network_id, ALICE_ID.clone(), fee);
    transaction.set_creation_time(Duration::from_millis(1));
    let transaction = transaction
        .with_instructions(body)
        .sign(ALICE_KEYPAIR.private_key());
    let hash =
        Hash::from(TransactionEntrypoint::External(transaction.clone()).execution_call_hash());
    let mut builder = BlockBuilder::new(BlockHeader::new(
        NonZeroU64::new(2).unwrap(),
        None,
        None,
        2,
        0,
    ));
    builder.push_transaction(transaction);
    (
        builder.build_with_signature(0, ALICE_KEYPAIR.private_key()),
        hash,
    )
}

#[test]
fn actual_signed_interleaving_keeps_scoped_lifecycle_and_complete_order_without_export() {
    on_stack(|| {
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
    });
}

#[test]
fn rejected_signed_body_drops_captured_facts_and_unsupported_mutation_observation() {
    on_stack(|| {
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
    });
}

#[test]
fn unsupported_actual_mutation_poison_applies_and_rolls_back_with_world() {
    on_stack(|| {
        let (state, alice, _) = fixture();
        let mut block = state.block(BlockHeader::new(
            NonZeroU64::new(2).unwrap(),
            None,
            None,
            2,
            0,
        ));
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
    });
}

#[test]
fn returned_error_poison_and_scope_observation_never_grant_export() {
    on_stack(|| {
        let (state, _, _) = fixture();
        let mut block = state.block(BlockHeader::new(
            NonZeroU64::new(2).unwrap(),
            None,
            None,
            2,
            0,
        ));
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
    });
}

#[test]
fn exact_capture_rejects_wrong_prestate_and_checks_actual_poststate_and_incarnation() {
    on_stack(|| {
        let (state, alice, _) = fixture();
        let mut block = state.block(BlockHeader::new(
            NonZeroU64::new(2).unwrap(),
            None,
            None,
            2,
            0,
        ));
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
    });
}

fn assert_exact_applied_measurement(archive: &QuantityCandidateArchive) {
    let mut expected = QuantityCandidateUsage::default();
    for (hash, entry) in &archive.entries {
        let input_bytes = entry
            .effects
            .iter()
            .map(|effect| u64::try_from(norito::encode_canonical(effect).unwrap().len()).unwrap())
            .sum::<u64>();
        let full = QuantityCandidateUsage {
            entries: 1,
            deltas: u64::try_from(entry.effects.len()).unwrap(),
            input_bytes,
            statement_bytes: u64::try_from(norito::encode_canonical(entry).unwrap().len()).unwrap(),
        };
        let measured = archive.measured_entries.get(hash).unwrap();
        assert_eq!(measured.baseline, QuantityCandidateUsage::default());
        assert_eq!(measured.full, full);
        expected.entries += full.entries;
        expected.deltas += full.deltas;
        expected.input_bytes += full.input_bytes;
        expected.statement_bytes += full.statement_bytes;
    }
    assert!(archive.base_usage.is_none());
    assert_eq!(archive.measured_entries.len(), archive.entries.len());
    assert_eq!(archive.usage, expected);
}

#[test]
fn distinct_signed_entries_retain_exact_independent_counters() {
    on_stack(|| {
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
        let mut builder = BlockBuilder::new(BlockHeader::new(
            NonZeroU64::new(2).unwrap(),
            None,
            None,
            2,
            0,
        ));
        let mut hashes = Vec::new();
        for body in bodies {
            let mut transaction = TransactionBuilder::new(
                state.network_id,
                ALICE_ID.clone(),
                FeePaymentIntent::authority(vec![], None),
            );
            transaction.set_creation_time(Duration::from_millis(1));
            let transaction = transaction
                .with_instructions(body)
                .sign(ALICE_KEYPAIR.private_key());
            hashes.push(Hash::from(
                TransactionEntrypoint::External(transaction.clone()).execution_call_hash(),
            ));
            builder.push_transaction(transaction);
        }
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
    });
}

#[test]
fn body_and_real_pipeline_fee_replace_one_complete_entry_counter() {
    use iroha_config::parameters::actual::{GasLiquidity, GasRate, GasVolatility};
    use iroha_data_model::transaction::{FeeChargeKind, FeeChargeLimit};
    use iroha_primitives::numeric::Numeric;
    on_stack(|| {
        let _fee_guard = crate::status::nexus_fee_test_lock().lock().unwrap();
        let (mut state, alice, bob) = fixture();
        {
            let mut setup = state.block(BlockHeader::new(NonZeroU64::MIN, None, None, 1, 0));
            let mut transaction = setup.transaction();
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
    });
}

#[test]
fn capture_capacity_refusal_keeps_exact_prefix_counters_and_original_business_result() {
    use iroha_data_model::parameter::{BlockParameter, FastpqSourcePolicyV1, Parameter};
    on_stack(|| {
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
    });
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
    let (mut state, alice, custody) = fixture();
    let instruction = InstructionBox::from(Log::new(Level::INFO, "sponsored burn".into()));
    let wire_id = iroha_data_model::isi::instruction_wire_id(&instruction)
        .unwrap()
        .to_owned();
    let program_id = FeeSponsorProgramId::new(BOB_ID.clone(), "quantity".parse().unwrap());
    {
        let mut setup = state.block(BlockHeader::new(NonZeroU64::MIN, None, None, 1, 0));
        let mut transaction = setup.transaction();
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
    (state, custody, program_id, instruction)
}

#[test]
fn actual_signed_sponsor_debit_captures_exact_custody_balance_and_supply() {
    use iroha_data_model::{
        nexus::FeeSponsorVaultKey,
        transaction::{FeeChargeKind, FeeChargeLimit},
    };
    on_stack(|| {
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
    });
}

#[test]
fn sponsor_business_capability_without_original_signed_source_cannot_capture() {
    on_stack(|| {
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
    });
}
