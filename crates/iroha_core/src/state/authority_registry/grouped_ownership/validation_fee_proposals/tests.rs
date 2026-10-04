//! Original fee-index projection, both images, work and identity regressions.

use super::*;
use crate::{state::GovernanceProposalStatus, test_allocations::allocations_during};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    governance::types::{
        AbiVersion, ContractAbiHash, ContractCodeHash, DeployContractProposal,
        ValidationFeePayoutLifecycleProposal,
    },
    smart_contract::ContractAddress,
    validation_fee::ValidationFeeTreasuryPayoutBindingV1,
};
use iroha_model_base::topology::DataSpaceId;
use mv::storage::Storage;

fn proposal(kind: u8, height: u64) -> GovernanceProposalRecord {
    let ProposalKind::ValidationFeePolicy(policy) =
        crate::governance::parliament::tests::validation_fee_policy_proposal()
    else {
        unreachable!("canonical typed fee fixture");
    };
    let proposer = policy.proposal_operator.clone();
    let kind = match kind {
        0 => ProposalKind::ValidationFeePolicy(policy),
        1 => {
            let pool = ContractAddress::derive(
                &policy.policy.network_id,
                &proposer,
                99,
                DataSpaceId::UNIVERSAL,
            )
            .unwrap();
            let custody = policy.policy.reward_custody;
            let binding = ValidationFeeTreasuryPayoutBindingV1 {
                contract_address: custody.contract_address,
                code_hash: [46; 32],
                entrypoint: "autonomous_validation_fee_tick".parse().unwrap(),
                treasury_account_id: custody.treasury_account_id,
                ds_asset_id: custody.ds_asset_id,
                xor_asset_id: custody.xor_asset_id,
                pool_vault_account_id: pool.subject_id(),
                pool_contract_address: pool,
                pool_code_hash: [48; 32],
                reward_pool_account_id: custody.reward_pool_account_id,
                reference_feed_id: "xor_per_sbd".parse().unwrap(),
                reference_feed_config_version: 1,
                reference_provider_accounts: (60..65)
                    .map(|tag| {
                        let key =
                            KeyPair::try_from_seed(vec![tag; 32], Algorithm::Ed25519).unwrap();
                        AccountId::new(key.public_key().clone())
                    })
                    .collect(),
                max_sbd_per_attempt_minor: 1000,
                max_sbd_per_day_minor: 100000,
                min_interval_ms: 60000,
                max_source_age_ms: 300000,
                max_slippage_bps: 100,
                validator_lane_id: custody.validator_lane_id,
                min_reward_claim_xor_minor: 1,
            };
            assert_eq!(binding.invariant_error(), None);
            ProposalKind::ValidationFeePayoutLifecycle(ValidationFeePayoutLifecycleProposal {
                proposal_operator: proposer.clone(),
                payout_binding: binding,
            })
        }
        2 => ProposalKind::DeployContract(DeployContractProposal {
            proposal_operator: proposer.clone(),
            contract_address: policy.policy.reward_custody.contract_address,
            code_hash: ContractCodeHash::new([31; 32]),
            abi_hash: ContractAbiHash::new([41; 32]),
            abi_version: AbiVersion::new(1),
            manifest_provenance: None,
        }),
        _ => unreachable!(),
    };
    GovernanceProposalRecord {
        proposer,
        kind,
        created_height: height,
        status: GovernanceProposalStatus::Proposed,
    }
}

fn fixture() -> Box<World> {
    let mut world = Box::new(World::default());
    for kind in 0..3 {
        world
            .governance_proposals
            .insert([kind; 32], proposal(kind, 41));
    }
    world.rebuild_governance_read_indexes().unwrap();
    world
}

fn check(world: &World, work: u64) -> Result<(), GroupedOwnershipError> {
    let mut result = None;
    assert_eq!(
        allocations_during(|| {
            result = Some(CheckedValidationFeeProposals::capture(world, work).map(|_| ()));
        }),
        0,
        "the original source and lookup require no scratch allocations"
    );
    result.unwrap()
}

fn corrupt(previous: bool, mismatch: GroupMismatch) -> GroupedOwnershipError {
    GroupedOwnershipError::Corrupt {
        index: INDEX,
        image: if previous {
            GroupImage::Predecessor
        } else {
            GroupImage::Current
        },
        mismatch,
    }
}

#[test]
fn both_fee_kinds_keep_every_status_and_non_fee_rows_are_excluded() {
    let mut world = Box::new(World::default());
    let statuses = [
        GovernanceProposalStatus::Proposed,
        GovernanceProposalStatus::Rejected,
        GovernanceProposalStatus::Enacted,
        GovernanceProposalStatus::Superseded,
        GovernanceProposalStatus::ExecutionFailed,
    ];
    for (ordinal, status) in statuses.into_iter().enumerate() {
        for kind in 0..3 {
            let mut row = proposal(kind, 41 + ordinal as u64);
            row.status = status;
            world
                .governance_proposals
                .insert([3 * ordinal as u8 + kind; 32], row);
        }
    }
    world.rebuild_governance_read_indexes().unwrap();
    assert_eq!(world.validation_fee_proposal_index.view().len(), 10);
    assert_eq!(check(&world, 90), Ok(()));
    assert_eq!(check(&world, 89), Err(GroupedOwnershipError::WorkLimit));
}

#[test]
fn moved_removed_retyped_inserted_and_redundant_rows_keep_exact_both_images() {
    let mut world = fixture();
    {
        let mut block = world.governance_proposals.block();
        block.insert([0; 32], proposal(0, 42));
        block.insert([1; 32], proposal(2, 43));
        block.insert([2; 32], proposal(1, 44));
        block.insert([3; 32], proposal(0, 45));
        block.remove([99; 32]);
        block.commit();
    }
    world.rebuild_governance_read_indexes().unwrap();
    assert_eq!(check(&world, 1024), Ok(()));
    {
        let checked = CheckedValidationFeeProposals::capture(&world, 1024).unwrap();
        for (key, current, previous) in [([0; 32], 42, 41), ([1; 32], 43, 41), ([2; 32], 44, 41)] {
            assert_eq!(
                get_at(checked.rows(), GroupImage::Current, &key)
                    .unwrap()
                    .created_height,
                current
            );
            assert_eq!(
                get_at(checked.rows(), GroupImage::Predecessor, &key)
                    .unwrap()
                    .created_height,
                previous
            );
        }
        assert!(get_at(checked.rows(), GroupImage::Predecessor, &[3; 32]).is_none());
        assert!(checked.rows().undo().contains_key(&[99; 32]));
        assert!(checked.matches_current().unwrap());
    }
    world.block_and_revert().commit();
    assert_eq!(check(&world, 1024), Ok(()));
    {
        let mut block = world.governance_proposals.block();
        block.remove([0; 32]);
        block.insert([1; 32], proposal(1, 41));
        block.commit();
    }
    world.rebuild_governance_read_indexes().unwrap();
    assert_eq!(check(&world, 1024), Ok(()));
}

#[test]
fn omitted_wrong_height_duplicate_non_fee_and_orphan_members_reject_in_both_images() {
    for previous in [false, true] {
        for defect in 0..5 {
            let mut world = fixture();
            if defect < 2 {
                world.validation_fee_proposal_index = Storage::from_iter([((41, [1; 32]), ())]);
            }
            let extra = match defect {
                0 => None,
                1 | 2 => Some((42, [0; 32])),
                3 => Some((41, [2; 32])),
                4 => Some((41, [99; 32])),
                _ => unreachable!(),
            };
            if let Some(key) = extra {
                world.validation_fee_proposal_index.insert(key, ());
            }
            if previous {
                let mut block = world.validation_fee_proposal_index.block();
                block.insert((41, [0; 32]), ());
                if let Some(key) = extra {
                    block.remove(key);
                }
                block.commit();
            }
            assert_eq!(
                check(&world, 1024),
                Err(corrupt(
                    previous,
                    if defect < 2 {
                        GroupMismatch::MissingMember
                    } else {
                        GroupMismatch::ForeignMember
                    }
                ))
            );
        }
    }
}

#[test]
fn omitted_non_fee_rows_and_physical_tombstones_still_consume_exact_work() {
    let world = fixture();
    assert_eq!(check(&world, 17), Err(GroupedOwnershipError::WorkLimit));
    assert_eq!(check(&world, 18), Ok(()));
    {
        let mut block = world.governance_proposals.block();
        block.insert([2; 32], proposal(2, 41));
        block.remove([99; 32]);
        block.commit();
    }
    assert_eq!(check(&world, 19), Err(GroupedOwnershipError::WorkLimit));
    assert_eq!(check(&world, 20), Ok(()));
    {
        let mut block = world.validation_fee_proposal_index.block();
        block.insert((41, [0; 32]), ());
        block.remove((99, [99; 32]));
        block.commit();
    }
    assert_eq!(check(&world, 21), Err(GroupedOwnershipError::WorkLimit));
    assert_eq!(check(&world, 22), Ok(()));
}

#[test]
fn either_original_reader_change_overrides_success_corruption_and_work_refusal() {
    for changed in 0..2 {
        for error in [
            None,
            Some(GroupedOwnershipError::WorkLimit),
            Some(corrupt(false, GroupMismatch::MissingMember)),
        ] {
            let world = fixture();
            let checked = CheckedValidationFeeProposals::retain(&world).unwrap();
            checked.validate(&mut Work(1024)).unwrap();
            match changed {
                0 => world.governance_proposals.block().commit(),
                1 => world.validation_fee_proposal_index.block().commit(),
                _ => unreachable!(),
            }
            assert!(!checked.matches_current().unwrap());
            assert_eq!(
                checked.finish_validation(error.map_or(Ok(()), Err)).err(),
                Some(GroupedOwnershipError::Publication(
                    PublicationPreparationError::Changed
                ))
            );
        }
    }
}
