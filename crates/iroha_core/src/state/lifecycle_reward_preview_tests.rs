//! Signed lane replacement must not erase unpaid economic obligations during preview.

use super::*;
use iroha_data_model::{
    isi::{
        InstructionBox, SetParameter,
        staking::{ClaimPublicLaneRewards, RecordPublicLaneRewards},
    },
    nexus::{
        LaneLifecycleParameterV1, LaneLifecyclePlan, PublicLaneMonetaryScopeV1,
        PublicLaneRewardClaimPlanV1, PublicLaneRewardClaimSourceV1, PublicLaneRewardRecordRefV1,
        public_lane_reward_record_commitment,
    },
};
use iroha_executor_data_model::permission::parameter::CanSetParameters;

const REPLACED_LANE: LaneId = LaneId::new(1);
const REWARD_EPOCH: u64 = 7;

fn replacement_fixture() -> (State, KeyPair, AssetId) {
    let (authority, signer) = gen_account_in("universal");
    let domain =
        Domain::new(DomainId::try_new("universal", "universal").unwrap()).build(&authority);
    let definition_id: AssetDefinitionId =
        iroha_config::parameters::defaults::nexus::fees::fee_asset_id()
            .parse()
            .expect("the canonical configured XOR identifier");
    let mut definition = AssetDefinition::numeric(
        definition_id.clone(),
        "XOR",
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&authority);
    definition.total_quantity = Quantity::from(100_u64);
    let source = AssetId::new(definition_id, authority.clone());
    let mut world = World::with_assets(
        [domain],
        [Account::new(authority.clone()).build(&authority)],
        [definition],
        [Asset::new(source.clone(), Quantity::from(100_u64))],
        [],
    );
    seed_snapshot_asset_incarnations(&mut world);
    let mut parameters = world.parameters.block();
    parameters.set_parameter(Parameter::Custom(
        iroha_data_model::parameter::system::SumeragiNposParameters::default()
            .into_custom_parameter(),
    ));
    parameters.commit();
    world.account_permissions.insert(
        authority.clone(),
        BTreeSet::from([Permission::from(CanSetParameters)]),
    );
    let mut nexus = iroha_config::parameters::actual::Nexus::default();
    nexus.lane_catalog = LaneCatalog::new(
        nonzero!(2_u32),
        vec![
            LaneConfig::default(),
            LaneConfig {
                id: REPLACED_LANE,
                alias: "replaceable-reward-lane".to_owned(),
                ..LaneConfig::default()
            },
        ],
    )
    .unwrap();
    nexus.staking.public_validator_mode =
        iroha_config::parameters::actual::LaneValidatorMode::StakeElected;
    nexus.staking.reward_dust_threshold = Quantity::from(10_u64);
    nexus.fees.fee_sink_account_id = authority.to_string();
    let state = State::new_with_nexus_for_testing(world, nexus, LiveQueryStore::start_test());
    seed_manual_lifecycle_replay_parent(&state);
    (state, signer, source)
}

fn signed_replacement(
    state: &State,
    signer: &KeyPair,
    source: &AssetId,
    reward: bool,
    process_dust: bool,
) -> SignedBlock {
    let nexus = state.nexus_snapshot();
    let lane = nexus
        .lane_catalog
        .lanes()
        .iter()
        .find(|lane| lane.id == REPLACED_LANE)
        .unwrap()
        .clone();
    let incarnations = LaneLifecycleParameterV1::canonical_incarnations(
        &nexus.lane_catalog,
        &state.lane_incarnations_snapshot(),
    )
    .unwrap();
    let payload = LaneLifecycleParameterV1::new(
        &nexus.lane_catalog,
        &incarnations,
        LaneLifecyclePlan {
            additions: vec![lane],
            retire: vec![REPLACED_LANE],
        },
    )
    .unwrap();
    let mut instructions: Vec<InstructionBox> =
        vec![SetParameter::new(Parameter::Custom(payload.into_custom_parameter())).into()];
    let parent = state
        .view()
        .latest_block()
        .expect("actual fixture predecessor");
    if reward {
        // A Nominator entitlement is valid without a validator row. The actual
        // configured treasury signs both its authorized record and the lifecycle.
        let record = PublicLaneRewardRecord {
            lane_id: REPLACED_LANE,
            epoch: REWARD_EPOCH,
            asset: source.clone(),
            total_reward: Quantity::from(5_u64),
            shares: vec![PublicLaneRewardShare {
                account: source.account().clone(),
                role: PublicLaneRewardRole::Nominator,
                amount: Quantity::from(5_u64),
            }],
            metadata: Metadata::default(),
        };
        instructions.push(
            RecordPublicLaneRewards {
                lane_id: record.lane_id,
                epoch: record.epoch,
                reward_asset: record.asset.clone(),
                total_reward: record.total_reward.clone(),
                shares: record.shares.clone(),
                metadata: record.metadata.clone(),
            }
            .into(),
        );
        if process_dust {
            instructions.push(
                ClaimPublicLaneRewards {
                    lane_id: REPLACED_LANE,
                    account: source.account().clone(),
                    claim_plan: PublicLaneRewardClaimPlanV1 {
                        network_scope: PublicLaneMonetaryScopeV1::Network(*state.network_id_ref()),
                        valid_until_height: parent.header().height().get() + 1,
                        expected_state: None,
                        records: vec![PublicLaneRewardRecordRefV1 {
                            epoch: REWARD_EPOCH,
                            record_hash: public_lane_reward_record_commitment(&record).unwrap(),
                        }],
                        sources: vec![PublicLaneRewardClaimSourceV1 {
                            source_asset: source.clone(),
                            destination_asset: source.clone(),
                            expected_accrued: None,
                            payout: Quantity::zero(),
                        }],
                    },
                }
                .into(),
            );
        }
    }
    let mut transaction = TransactionBuilder::new(
        *state.network_id_ref(),
        source.account().clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    );
    transaction.set_creation_time(Duration::from_millis(1));
    let transaction = transaction
        .with_instructions(instructions)
        .sign(signer.private_key());
    let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(transaction));
    let (_, time_source) = iroha_primitives::time::TimeSource::new_mock(Duration::from_millis(2));
    BlockBuilder::new_with_time_source(vec![accepted], time_source)
        .chain(0, Some(&parent))
        .sign(signer.private_key())
        .unpack(|_| {})
        .into()
}

fn assert_late_reward_refused(process_dust: bool) {
    let (state, signer, source) = replacement_fixture();
    let original_incarnation = state.lane_incarnation(REPLACED_LANE).unwrap();
    let mut signed = signed_replacement(&state, &signer, &source, true, process_dust);
    let mut block = state.block(signed.header());
    ValidBlock::execute_block_outputs_for_test(&mut signed, &mut block, None)
        .expect("the signed sequence executes through the real permission and reward paths");
    assert!(
        signed.output_error(0).is_none(),
        "{:?}",
        signed.output_error(0)
    );
    assert!(
        block
            .pending_autoscale_lifecycle
            .as_ref()
            .is_some_and(|pending| {
                pending
                    .catalog_update
                    .lanes_to_reset
                    .contains(&REPLACED_LANE)
                    && pending
                        .catalog_update
                        .replaced_lane_ids
                        .contains(&REPLACED_LANE)
            })
    );
    assert_ne!(
        block.lane_incarnations[&REPLACED_LANE],
        original_incarnation
    );
    assert!(
        block
            .world
            .public_lane_rewards
            .get(&(REPLACED_LANE, REWARD_EPOCH))
            .is_some()
    );
    assert_eq!(
        block.world.public_lane_reward_reserves.get(&source),
        Some(&Quantity::from(5_u64))
    );
    let claim_key = (REPLACED_LANE, source.account().clone());
    let accrual_key = (REPLACED_LANE, source.account().clone(), source.clone());
    if process_dust {
        assert_eq!(
            block.world.public_lane_reward_claims.get(&claim_key),
            Some(&PublicLaneRewardClaimStateV1 {
                through_epoch: Some(REWARD_EPOCH)
            })
        );
        assert_eq!(
            block.world.public_lane_reward_accruals.get(&accrual_key),
            Some(&Quantity::from(5_u64))
        );
    } else {
        assert!(
            block
                .world
                .public_lane_reward_claims
                .get(&claim_key)
                .is_none()
        );
        assert!(
            block
                .world
                .public_lane_reward_accruals
                .get(&accrual_key)
                .is_none()
        );
    }
    validate_public_lane_reward_reserves(&block.world).expect("exact backed original obligation");
    // Event delivery is a separate prerequisite of this existing finalization API.
    let _events = block.world.take_external_events();
    let before = block.world.net_state_delta().unwrap();
    let expected_reason = if process_dust {
        "public-lane accrued rewards remain unpaid"
    } else {
        "public-lane rewards remain unpaid or their retained record is invalid"
    };
    let error = block
        .prepare_finalized_publication_surface()
        .expect_err("preview must refuse before removing the obligation's source records");
    assert!(error.contains(expected_reason), "{error}");
    assert_eq!(
        block.world.net_state_delta().unwrap(),
        before,
        "refused preview preserves every original World write"
    );
    validate_public_lane_reward_reserves(&block.world).expect("refusal preserves exact backing");
    assert_eq!(
        block.world.assets.get(&source).unwrap().as_ref(),
        &Quantity::from(100_u64)
    );
    drop(block);
    let world = state.world.view();
    assert!(
        world
            .public_lane_rewards()
            .get(&(REPLACED_LANE, REWARD_EPOCH))
            .is_none()
    );
    assert!(world.public_lane_reward_reserves().get(&source).is_none());
    assert!(
        world
            .public_lane_reward_accruals()
            .get(&accrual_key)
            .is_none()
    );
    assert_eq!(
        state.lane_incarnation(REPLACED_LANE),
        Some(original_incarnation)
    );
}

#[test]
fn manual_replacement_preview_rejects_late_signed_reward_without_mutation() {
    assert_late_reward_refused(false);
}

#[test]
fn manual_replacement_preview_rejects_late_signed_dust_accrual_without_mutation() {
    assert_late_reward_refused(true);
}

#[test]
fn manual_replacement_preview_clean_surface_is_idempotent() {
    let (state, signer, source) = replacement_fixture();
    let mut signed = signed_replacement(&state, &signer, &source, false, false);
    let mut block = state.block(signed.header());
    ValidBlock::execute_block_outputs_for_test(&mut signed, &mut block, None).unwrap();
    assert!(
        signed.output_error(0).is_none(),
        "{:?}",
        signed.output_error(0)
    );
    assert!(block.pending_autoscale_lifecycle.is_some());
    let _events = block.world.take_external_events();
    let first = block
        .prepare_finalized_publication_surface()
        .expect("no unpaid custody");
    first.verify(&block).unwrap();
    let second = block.prepare_finalized_publication_surface().unwrap();
    assert_eq!(first, second);
    validate_public_lane_reward_reserves(&block.world).unwrap();
}
