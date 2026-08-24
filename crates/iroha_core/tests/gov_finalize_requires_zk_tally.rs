#![doc = "FinalizeReferendum rejects when a ZK election exists but the tally is not finalized."]
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! FinalizeReferendum rejects when a ZK election exists but the tally is not finalized.
#[path = "common/governance.rs"]
mod governance_fixture;
use core::num::NonZeroU64;
use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::Execute,
    state::{
        ElectionState, GovernancePipeline, GovernanceProposalRecord, GovernanceProposalStatus,
        GovernanceReferendumMode, GovernanceReferendumRecord, GovernanceReferendumStatus, State,
        World,
    },
};
use iroha_data_model::{
    block::BlockHeader,
    governance::types::{
        AbiVersion, ContractAbiHash, ContractCodeHash, DeployContractProposal, ProposalKind,
    },
    isi::governance::FinalizeReferendum,
    nexus::DataSpaceId,
    smart_contract::ContractAddress,
};
use iroha_test_samples::ALICE_ID;
#[test]
fn finalize_referendum_rejects_unfinalized_zk_election() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new_for_testing(World::default(), kura, query_handle);
    let header = BlockHeader::new(NonZeroU64::new(1).unwrap(), None, None, None, 0, 0);
    let mut sblock = state.block(header);
    let mut stx = sblock.transaction();
    let proposal_id = [0x42; 32];
    let rid = hex::encode(proposal_id);
    let referendum = GovernanceReferendumRecord {
        h_start: 0,
        h_end: 10,
        status: GovernanceReferendumStatus::Open,
        mode: GovernanceReferendumMode::Zk,
    };
    let contract_address = ContractAddress::derive(
        &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
            .parse()
            .expect("canonical test network id"),
        &ALICE_ID,
        0,
        DataSpaceId::UNIVERSAL,
    )
    .expect("test contract address");
    stx.world.governance_proposals_mut().insert(
        proposal_id,
        GovernanceProposalRecord {
            proposer: ALICE_ID.clone(),
            kind: ProposalKind::DeployContract(DeployContractProposal {
                contract_address,
                code_hash_hex: ContractCodeHash::new([0x11; 32]),
                abi_hash_hex: ContractAbiHash::new(ivm::syscalls::compute_abi_hash(
                    ivm::SyscallPolicy::AbiV1,
                )),
                abi_version: AbiVersion::new(1),
                manifest_provenance: None,
            }),
            created_height: 1,
            status: GovernanceProposalStatus::Proposed,
            pipeline: GovernancePipeline::seeded(1, Some(&referendum), &stx.gov),
            parliament_snapshot: governance_fixture::single_member_parliament_snapshot(
                &ALICE_ID, 1,
            ),
            finalization_evidence: None,
            enacted_at_height: None,
        },
    );
    stx.world.elections_mut().insert(
        rid.clone(),
        ElectionState {
            options: 2,
            eligible_root: [0u8; 32],
            start_ts: 0,
            end_ts: 0,
            finalized: false,
            tally: vec![0, 0],
            ballot_nullifiers: std::collections::BTreeSet::default(),
            ciphertexts: Vec::new(),
            vk_ballot: None,
            vk_ballot_commitment: None,
            vk_tally: None,
            vk_tally_commitment: None,
            domain_tag: String::new(),
        },
    );
    stx.world
        .governance_referenda_mut()
        .insert(rid.clone(), referendum);
    let instr = FinalizeReferendum {
        referendum_id: rid,
        proposal_id,
    };
    let err = instr.execute(&ALICE_ID, &mut stx).unwrap_err();
    assert!(err.to_string().contains("election tally not finalized"));
}
