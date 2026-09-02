//! Validation-fee proposals reject the standalone public PLAIN ballot path.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]

use core::num::NonZeroU64;

use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::Execute,
    state::{
        GovernanceProposalRecord, GovernanceProposalStatus, GovernanceReferendumRecord,
        GovernanceReferendumStatus, State, World, WorldReadOnly,
    },
};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    Registrable,
    account::{Account, AccountId},
    asset::AssetDefinitionId,
    block::BlockHeader,
    domain::{Domain, DomainId},
    governance::types::{ProposalKind, ValidationFeePolicyProposal},
    isi::{Grant, governance::CastPlainBallot},
    permission::Permission,
    validation_fee::{
        VALIDATION_FEE_DS_SCALE, VALIDATION_FEE_POLICY_ACTIVATION_DELAY_BLOCKS,
        VALIDATION_FEE_POLICY_SCHEMA_VERSION, ValidationFeeChargingMode, ValidationFeePolicyV1,
    },
};
use iroha_executor_data_model::permission::governance::CanSubmitGovernanceBallot;
use iroha_primitives::numeric::Quantity;
use mv::storage::StorageReadOnly;

const BALLOT_HEIGHT: u64 = 10;

fn account(seed: u8) -> AccountId {
    let key_pair =
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).expect("deterministic key pair");
    AccountId::new(key_pair.public_key().clone())
}

#[test]
fn validation_fee_proposal_rejects_plain_ballot_without_state_effects() {
    let proposer = account(1);
    let domain_id = DomainId::try_new("validation_fee", "universal").expect("domain");
    let fee_asset_id = AssetDefinitionId::derive_from_components(
        domain_id.clone(),
        "xor".parse().expect("asset name"),
    );
    let domain = Domain::new(domain_id).build(&proposer);
    let account = Account::new(proposer.clone()).build(&proposer);
    let world = World::with([domain], [account], []);
    let mut state = State::new_for_testing(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let mut governance = state.gov.clone();
    governance.plain_voting_enabled = true;
    governance.min_bond_amount = Quantity::zero();
    governance.conviction_step_blocks = 1;
    state.set_gov(governance);

    let policy = ValidationFeePolicyV1 {
        schema_version: VALIDATION_FEE_POLICY_SCHEMA_VERSION,
        network_id: *state.network_id_ref(),
        policy_version: 1,
        previous_policy_hash: None,
        ds_asset_id: fee_asset_id,
        ds_scale: VALIDATION_FEE_DS_SCALE,
        fee: Quantity::zero(),
        treasury_account_id: proposer.clone(),
        charging_mode: ValidationFeeChargingMode::Disabled,
        effective_from_height: BALLOT_HEIGHT + VALIDATION_FEE_POLICY_ACTIVATION_DELAY_BLOCKS,
        expires_after_height: None,
        exemption_classes: Vec::new(),
        treasury_payout_binding: None,
    };
    assert_eq!(policy.policy_invariant_error(), None);
    let proposal_kind = ProposalKind::ValidationFeePolicy(ValidationFeePolicyProposal {
        proposal_operator: proposer.clone(),
        policy,
        payout_lifecycle_proposal_id: None,
    });
    let proposal_id = proposal_kind.fingerprint();
    let referendum_id = hex::encode(proposal_id);

    let header = BlockHeader::new(
        NonZeroU64::new(BALLOT_HEIGHT).expect("non-zero ballot height"),
        None,
        None,
        None,
        0,
        0,
    );
    let mut block = state.block(header);
    let mut state_transaction = block.transaction();
    let ballot_permission: Permission = CanSubmitGovernanceBallot {
        referendum_id: referendum_id.clone(),
    }
    .into();
    Grant::account_permission(ballot_permission, proposer.clone())
        .execute(&proposer, &mut state_transaction)
        .expect("grant exact generic ballot permission");
    state_transaction.world.governance_proposals_mut().insert(
        proposal_id,
        GovernanceProposalRecord {
            proposer: proposer.clone(),
            kind: proposal_kind,
            created_height: BALLOT_HEIGHT,
            status: GovernanceProposalStatus::Proposed,
        },
    );
    state_transaction
        .world
        .put_governance_referendum_for_testing(
            referendum_id.clone(),
            GovernanceReferendumRecord {
                h_start: BALLOT_HEIGHT,
                h_end: BALLOT_HEIGHT + 100,
                status: GovernanceReferendumStatus::Open,
                final_tally: None,
            },
        );

    let error = CastPlainBallot {
        referendum_id: referendum_id.clone(),
        direction: iroha_data_model::isi::governance::GovernancePlainBallotDirectionV1::Aye,
        lock: iroha_data_model::isi::governance::GovernanceParticipationLockV1 {
            amount: Quantity::from(1_u32),
            duration_blocks: core::num::NonZeroU64::new(100).expect("non-zero lock duration"),
        },
    }
    .execute(&proposer, &mut state_transaction)
    .expect_err("validation-fee proposals must reject public PLAIN ballots");
    let message = error.to_string();
    assert!(
        message.contains("typed governance proposals") && message.contains("private Parliament"),
        "unexpected validation-fee PLAIN rejection: {message}"
    );
    assert!(
        state_transaction
            .world
            .governance_locks()
            .get(&referendum_id)
            .is_none(),
        "a rejected validation-fee PLAIN ballot must not create a governance lock"
    );
}
