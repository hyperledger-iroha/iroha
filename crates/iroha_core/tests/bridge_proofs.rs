//! Bridge proof submission and retention tests.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]

use iroha_core::{
    executor::Executor,
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, WorldReadOnly},
    telemetry::StateTelemetry,
};
use iroha_data_model::{
    prelude::*,
    proof::{ProofBox, ProofId, ProofStatus},
};
use iroha_test_samples::ALICE_ID;
use mv::storage::StorageReadOnly;
use nonzero_ext::nonzero;

const SCCP_AUDITED_SOLANA_PROOF_MAX_BYTES: u32 = 8 * 1024 * 1024;

fn bridge_proof_id(proof: &BridgeProof) -> ProofId {
    let encoded = norito::to_bytes(proof).expect("encode bridge proof");
    let backend = proof.backend_label();
    let proof = ProofBox::new(backend.clone(), encoded);
    ProofId {
        backend,
        proof_hash: iroha_core::zk::hash_proof(&proof),
    }
}

fn solana_vote_keypairs() -> [iroha_crypto::KeyPair; 4] {
    [
        iroha_crypto::KeyPair::from_seed(
            b"iroha:core-test:sccp:sol-vote:0".to_vec(),
            iroha_crypto::Algorithm::Ed25519,
        ),
        iroha_crypto::KeyPair::from_seed(
            b"iroha:core-test:sccp:sol-vote:1".to_vec(),
            iroha_crypto::Algorithm::Ed25519,
        ),
        iroha_crypto::KeyPair::from_seed(
            b"iroha:core-test:sccp:sol-vote:2".to_vec(),
            iroha_crypto::Algorithm::Ed25519,
        ),
        iroha_crypto::KeyPair::from_seed(
            b"iroha:core-test:sccp:sol-vote:3".to_vec(),
            iroha_crypto::Algorithm::Ed25519,
        ),
    ]
}

fn solana_vote_public_keys(signers: &[iroha_crypto::KeyPair; 4]) -> Vec<Vec<u8>> {
    signers
        .iter()
        .map(|signer| {
            let (algorithm, bytes) = signer.public_key().to_bytes();
            assert_eq!(algorithm, iroha_crypto::Algorithm::Ed25519);
            bytes.to_vec()
        })
        .collect()
}

fn solana_vote_roster_hash() -> [u8; 32] {
    let signers = solana_vote_keypairs();
    iroha_sccp::sccp_solana_vote_roster_hash(&solana_vote_public_keys(&signers), &[1, 1, 1, 1])
        .expect("Solana vote roster hash")
}

fn solana_validator_stakes() -> Vec<u64> {
    vec![1, 1, 1, 1]
}

fn solana_validator_delegated_stakes() -> Vec<u64> {
    vec![1, 1, 1, 1]
}

fn solana_validator_activation_epochs() -> Vec<u64> {
    vec![0, 0, 0, 0]
}

fn solana_validator_deactivation_epochs() -> Vec<u64> {
    vec![u64::MAX, u64::MAX, u64::MAX, u64::MAX]
}

fn solana_validator_vote_account_addresses() -> Vec<Vec<u8>> {
    vec![
        vec![0x31; 32],
        vec![0x32; 32],
        vec![0x33; 32],
        vec![0x34; 32],
    ]
}

fn solana_validator_stake_account_addresses() -> Vec<Vec<u8>> {
    vec![
        vec![0x41; 32],
        vec![0x42; 32],
        vec![0x43; 32],
        vec![0x44; 32],
    ]
}

fn solana_rooted_slot(finalized_slot: u64) -> u64 {
    finalized_slot.saturating_sub(iroha_sccp::SCCP_SOLANA_TOWER_VOTE_STACK_DEPTH)
}

fn solana_tower_vote_slots(finalized_slot: u64) -> Vec<u64> {
    ((solana_rooted_slot(finalized_slot) + 1)..=finalized_slot).collect()
}

fn solana_validator_vote_account_data(
    finalized_slot: u64,
) -> Vec<iroha_sccp::SccpSolanaVoteAccountDataV1> {
    let tower_vote_slots = solana_tower_vote_slots(finalized_slot);
    let rooted_slot = solana_rooted_slot(finalized_slot);
    let signers = solana_vote_keypairs();
    let vote_account_addresses = solana_validator_vote_account_addresses();
    solana_vote_public_keys(&signers)
        .into_iter()
        .enumerate()
        .map(
            |(index, authorized_voter)| iroha_sccp::SccpSolanaVoteAccountDataV1 {
                node_pubkey: vec![0x51 + index as u8; 32],
                authorized_voter,
                authorized_withdrawer: vec![0x61 + index as u8; 32],
                inflation_rewards_collector: vote_account_addresses[index].clone(),
                block_revenue_collector: vec![0x51 + index as u8; 32],
                inflation_rewards_commission_bps: u16::try_from(index * 100)
                    .expect("sample index fits u16"),
                block_revenue_commission_bps: 10_000,
                pending_delegator_rewards: 0,
                bls_pubkey_compressed: Vec::new(),
                root_slot: rooted_slot,
                tower_vote_slots: tower_vote_slots.clone(),
            },
        )
        .collect()
}

fn solana_vote_state_account_raw_data(
    data: &iroha_sccp::SccpSolanaVoteAccountDataV1,
    epoch: u64,
) -> Vec<u8> {
    let mut raw = Vec::new();
    raw.extend_from_slice(&2u32.to_le_bytes());
    raw.extend_from_slice(&data.node_pubkey);
    raw.extend_from_slice(&data.authorized_withdrawer);
    raw.push(
        u8::try_from(data.inflation_rewards_commission_bps / 100)
            .expect("sample legacy commission fits u8"),
    );
    raw.extend_from_slice(&iroha_sccp::SCCP_SOLANA_TOWER_VOTE_STACK_DEPTH.to_le_bytes());
    for (index, slot) in data.tower_vote_slots.iter().enumerate() {
        raw.push(0);
        raw.extend_from_slice(&slot.to_le_bytes());
        let confirmation =
            u32::try_from(data.tower_vote_slots.len() - index).expect("confirmation fits u32");
        raw.extend_from_slice(&confirmation.to_le_bytes());
    }
    raw.push(1);
    raw.extend_from_slice(&data.root_slot.to_le_bytes());
    raw.extend_from_slice(&1u64.to_le_bytes());
    raw.extend_from_slice(&epoch.to_le_bytes());
    raw.extend_from_slice(&data.authorized_voter);
    raw.resize(iroha_sccp::SCCP_SOLANA_VOTE_STATE_ACCOUNT_DATA_LEN, 0);
    raw
}

fn solana_validator_vote_account_raw_data(finalized_slot: u64) -> Vec<Vec<u8>> {
    let epoch = iroha_sccp::sccp_solana_mainnet_epoch_for_slot(finalized_slot);
    solana_validator_vote_account_data(finalized_slot)
        .iter()
        .map(|data| solana_vote_state_account_raw_data(data, epoch))
        .collect()
}

fn solana_validator_stake_account_data() -> Vec<iroha_sccp::SccpSolanaStakeAccountDataV1> {
    let delegated_stakes = solana_validator_delegated_stakes();
    let activation_epochs = solana_validator_activation_epochs();
    let deactivation_epochs = solana_validator_deactivation_epochs();
    solana_validator_vote_account_addresses()
        .into_iter()
        .enumerate()
        .map(
            |(index, voter_pubkey)| iroha_sccp::SccpSolanaStakeAccountDataV1 {
                staker: vec![0x91 + index as u8; 32],
                withdrawer: vec![0xA1 + index as u8; 32],
                voter_pubkey,
                delegated_stake: delegated_stakes[index],
                activation_epoch: activation_epochs[index],
                deactivation_epoch: deactivation_epochs[index],
                warmup_cooldown_rate_bytes: vec![0x0a, 0xd7, 0xa3, 0x70, 0x3d, 0x0a, 0xb7, 0x3f],
                credits_observed: 10 + u64::try_from(index).expect("sample index fits u64"),
                stake_flags: 1,
            },
        )
        .collect()
}

fn solana_stake_state_v2_raw_data(data: &iroha_sccp::SccpSolanaStakeAccountDataV1) -> Vec<u8> {
    let mut raw = vec![0u8; iroha_sccp::SCCP_SOLANA_STAKE_STATE_V2_STAKE_ACCOUNT_DATA_LEN];
    raw[0..4].copy_from_slice(&2u32.to_le_bytes());
    raw[12..44].copy_from_slice(&data.staker);
    raw[44..76].copy_from_slice(&data.withdrawer);
    raw[124..156].copy_from_slice(&data.voter_pubkey);
    raw[156..164].copy_from_slice(&data.delegated_stake.to_le_bytes());
    raw[164..172].copy_from_slice(&data.activation_epoch.to_le_bytes());
    raw[172..180].copy_from_slice(&data.deactivation_epoch.to_le_bytes());
    raw[180..188].copy_from_slice(&data.warmup_cooldown_rate_bytes);
    raw[188..196].copy_from_slice(&data.credits_observed.to_le_bytes());
    raw[196] = data.stake_flags;
    raw
}

fn solana_validator_stake_account_raw_data() -> Vec<Vec<u8>> {
    solana_validator_stake_account_data()
        .iter()
        .map(solana_stake_state_v2_raw_data)
        .collect()
}

fn solana_vote_account_data_hash(data: &iroha_sccp::SccpSolanaVoteAccountDataV1) -> [u8; 32] {
    iroha_sccp::sccp_solana_vote_account_data_hash(
        &data.node_pubkey,
        &data.authorized_voter,
        &data.authorized_withdrawer,
        &data.inflation_rewards_collector,
        &data.block_revenue_collector,
        data.inflation_rewards_commission_bps,
        data.block_revenue_commission_bps,
        data.pending_delegator_rewards,
        &data.bls_pubkey_compressed,
        data.root_slot,
        &data.tower_vote_slots,
    )
    .expect("Solana vote account data hash")
}

fn solana_stake_account_data_hash(data: &iroha_sccp::SccpSolanaStakeAccountDataV1) -> [u8; 32] {
    iroha_sccp::sccp_solana_stake_account_data_hash(
        &data.staker,
        &data.withdrawer,
        &data.voter_pubkey,
        data.delegated_stake,
        data.activation_epoch,
        data.deactivation_epoch,
        &data.warmup_cooldown_rate_bytes,
        data.credits_observed,
        data.stake_flags,
    )
    .expect("Solana stake account data hash")
}

fn solana_account_opening_hash(opening: &iroha_sccp::SccpSolanaAccountOpeningV1) -> [u8; 32] {
    iroha_sccp::sccp_solana_account_opening_hash(
        &opening.address,
        &opening.owner,
        opening.lamports,
        opening.rent_epoch,
        opening.executable,
        opening.data_hash,
    )
    .expect("Solana account opening hash")
}

fn solana_account_openings(
    addresses: Vec<Vec<u8>>,
    owner: [u8; 32],
    data_hashes: Vec<[u8; 32]>,
) -> Vec<iroha_sccp::SccpSolanaAccountOpeningV1> {
    addresses
        .into_iter()
        .enumerate()
        .map(|(index, address)| iroha_sccp::SccpSolanaAccountOpeningV1 {
            address,
            owner: owner.to_vec(),
            lamports: 1_000_000 + u64::try_from(index).expect("sample index fits u64"),
            rent_epoch: 0,
            executable: false,
            data_hash: data_hashes[index],
        })
        .collect()
}

fn solana_validator_vote_account_openings(
    finalized_slot: u64,
) -> Vec<iroha_sccp::SccpSolanaAccountOpeningV1> {
    let data_hashes = solana_validator_vote_account_data(finalized_slot)
        .iter()
        .map(solana_vote_account_data_hash)
        .collect();
    solana_account_openings(
        solana_validator_vote_account_addresses(),
        iroha_sccp::SCCP_SOLANA_VOTE_PROGRAM_ID,
        data_hashes,
    )
}

fn solana_validator_stake_account_openings() -> Vec<iroha_sccp::SccpSolanaAccountOpeningV1> {
    let data_hashes = solana_validator_stake_account_data()
        .iter()
        .map(solana_stake_account_data_hash)
        .collect();
    solana_account_openings(
        solana_validator_stake_account_addresses(),
        iroha_sccp::SCCP_SOLANA_STAKE_PROGRAM_ID,
        data_hashes,
    )
}

fn solana_validator_vote_account_hashes(finalized_slot: u64) -> Vec<Vec<u8>> {
    solana_validator_vote_account_openings(finalized_slot)
        .iter()
        .map(|opening| solana_account_opening_hash(opening).to_vec())
        .collect()
}

fn solana_validator_stake_account_hashes() -> Vec<Vec<u8>> {
    solana_validator_stake_account_openings()
        .iter()
        .map(|opening| solana_account_opening_hash(opening).to_vec())
        .collect()
}

fn solana_stake_history_entries(epoch: u64) -> Vec<iroha_sccp::SccpSolanaStakeHistoryEntryV1> {
    let mut entries = Vec::new();
    if epoch > 0 {
        entries.push(iroha_sccp::SccpSolanaStakeHistoryEntryV1 {
            epoch: epoch - 1,
            effective: 4,
            activating: 4,
            deactivating: 0,
        });
    }
    entries.push(iroha_sccp::SccpSolanaStakeHistoryEntryV1 {
        epoch,
        effective: 4,
        activating: 0,
        deactivating: 0,
    });
    entries
}

fn solana_stake_history_sysvar_opening(epoch: u64) -> iroha_sccp::SccpSolanaAccountOpeningV1 {
    let entries = solana_stake_history_entries(epoch);
    iroha_sccp::SccpSolanaAccountOpeningV1 {
        address: iroha_sccp::SCCP_SOLANA_STAKE_HISTORY_SYSVAR_ID.to_vec(),
        owner: iroha_sccp::SCCP_SOLANA_SYSVAR_PROGRAM_ID.to_vec(),
        lamports: 1,
        rent_epoch: 0,
        executable: false,
        data_hash: iroha_sccp::sccp_solana_stake_history_sysvar_data_hash(&entries)
            .expect("Solana StakeHistory sysvar data hash"),
    }
}

fn solana_stake_history_sysvar_account_hash(epoch: u64) -> [u8; 32] {
    solana_account_opening_hash(&solana_stake_history_sysvar_opening(epoch))
}

fn solana_stake_history_sysvar_raw_data(epoch: u64) -> Vec<u8> {
    iroha_sccp::canonical_sccp_solana_stake_history_sysvar_data_bytes(
        &solana_stake_history_entries(epoch),
    )
    .expect("Solana StakeHistory sysvar raw data")
}

fn solana_unopened_bank_account(
    finalized_slot: u64,
) -> (iroha_sccp::SccpSolanaAccountOpeningV1, Vec<u8>) {
    let mut raw_data = Vec::new();
    raw_data.extend_from_slice(b"iroha:core-test:sccp:solana:unopened-account");
    raw_data.extend_from_slice(&finalized_slot.to_le_bytes());
    let opening = iroha_sccp::SccpSolanaAccountOpeningV1 {
        address: vec![0x71; 32],
        owner: vec![0x72; 32],
        lamports: 2_000_000,
        rent_epoch: iroha_sccp::sccp_solana_mainnet_epoch_for_slot(finalized_slot),
        executable: false,
        data_hash: iroha_sccp::sccp_solana_account_raw_data_hash(&raw_data)
            .expect("Solana unopened account raw-data hash"),
    };
    (opening, raw_data)
}

fn solana_account_inclusion_witness(
    finalized_slot: u64,
) -> (
    [u8; 32],
    Vec<iroha_sccp::SccpSolanaAccountInclusionBranchV1>,
    Vec<iroha_sccp::SccpSolanaAccountInclusionBranchV1>,
    iroha_sccp::SccpSolanaAccountInclusionBranchV1,
) {
    let epoch = iroha_sccp::sccp_solana_mainnet_epoch_for_slot(finalized_slot);
    let vote_openings = solana_validator_vote_account_openings(finalized_slot);
    let vote_raw_data = solana_validator_vote_account_raw_data(finalized_slot);
    let stake_openings = solana_validator_stake_account_openings();
    let stake_raw_data = solana_validator_stake_account_raw_data();
    let sysvar_opening = solana_stake_history_sysvar_opening(epoch);
    let sysvar_raw_data = solana_stake_history_sysvar_raw_data(epoch);

    let mut leaves = Vec::new();
    for (opening, raw_data) in vote_openings.iter().zip(vote_raw_data.iter()) {
        leaves.push(
            iroha_sccp::sccp_solana_account_inclusion_leaf_hash(finalized_slot, opening, raw_data)
                .expect("Solana vote account inclusion leaf"),
        );
    }
    for (opening, raw_data) in stake_openings.iter().zip(stake_raw_data.iter()) {
        leaves.push(
            iroha_sccp::sccp_solana_account_inclusion_leaf_hash(finalized_slot, opening, raw_data)
                .expect("Solana stake account inclusion leaf"),
        );
    }
    leaves.push(
        iroha_sccp::sccp_solana_account_inclusion_leaf_hash(
            finalized_slot,
            &sysvar_opening,
            &sysvar_raw_data,
        )
        .expect("Solana StakeHistory sysvar inclusion leaf"),
    );

    let vote_len = vote_openings.len();
    let stake_len = stake_openings.len();
    let (root, mut branches) = iroha_sccp::sccp_solana_account_inclusion_root_and_branches(&leaves)
        .expect("Solana account inclusion root and branches");
    let sysvar_branch = branches
        .pop()
        .expect("Solana StakeHistory sysvar inclusion branch");
    let stake_branches = branches.split_off(vote_len);
    assert_eq!(stake_branches.len(), stake_len);
    let vote_branches = branches;
    assert_eq!(vote_branches.len(), vote_len);
    (root, vote_branches, stake_branches, sysvar_branch)
}

fn solana_accounts_lt_hash(finalized_slot: u64) -> Vec<u8> {
    let epoch = iroha_sccp::sccp_solana_mainnet_epoch_for_slot(finalized_slot);
    let mut openings = solana_validator_vote_account_openings(finalized_slot);
    let mut raw_data = solana_validator_vote_account_raw_data(finalized_slot);
    openings.extend(solana_validator_stake_account_openings());
    raw_data.extend(solana_validator_stake_account_raw_data());
    openings.push(solana_stake_history_sysvar_opening(epoch));
    raw_data.push(solana_stake_history_sysvar_raw_data(epoch));
    let (unopened_opening, unopened_raw_data) = solana_unopened_bank_account(finalized_slot);
    openings.push(unopened_opening);
    raw_data.push(unopened_raw_data);
    iroha_sccp::sccp_solana_accounts_lt_hash_from_openings(&openings, &raw_data)
        .expect("Solana AccountsLtHash")
}

fn solana_accounts_lt_hash_checksum(finalized_slot: u64) -> [u8; 32] {
    iroha_sccp::sccp_solana_accounts_lt_hash_checksum(&solana_accounts_lt_hash(finalized_slot))
        .expect("Solana AccountsLtHash checksum")
}

fn solana_parent_bank_hash() -> [u8; 32] {
    [0xC0; 32]
}

fn solana_bank_signature_count(_finalized_slot: u64) -> u64 {
    4
}

fn solana_bank_hash_hard_fork_data() -> Vec<u8> {
    Vec::new()
}

fn solana_bank_hash(finalized_slot: u64, blockhash: [u8; 32]) -> [u8; 32] {
    iroha_sccp::sccp_solana_agave_bank_hash(
        solana_parent_bank_hash(),
        solana_bank_signature_count(finalized_slot),
        blockhash,
        &solana_accounts_lt_hash(finalized_slot),
        &solana_bank_hash_hard_fork_data(),
    )
    .expect("Solana Agave bank hash")
}

fn solana_finality_context(
    finalized_slot: u64,
    blockhash: [u8; 32],
    bank_hash: [u8; 32],
    transaction_status_root: [u8; 32],
) -> iroha_sccp::SccpSolanaFinalityContextV1 {
    let epoch = iroha_sccp::sccp_solana_mainnet_epoch_for_slot(finalized_slot);
    let rooted_slot = solana_rooted_slot(finalized_slot);
    let parent_slot = finalized_slot.saturating_sub(1);
    let tower_vote_slots = solana_tower_vote_slots(finalized_slot);
    let parent_bank_hash = solana_parent_bank_hash();
    let bank_signature_count = solana_bank_signature_count(finalized_slot);
    let bank_hash_hard_fork_data = solana_bank_hash_hard_fork_data();
    let signers = solana_vote_keypairs();
    let validator_public_keys = solana_vote_public_keys(&signers);
    let (account_inclusion_root, _, _, _) = solana_account_inclusion_witness(finalized_slot);
    let accounts_lt_hash_checksum = solana_accounts_lt_hash_checksum(finalized_slot);
    let bank_fork_hash = iroha_sccp::sccp_solana_bank_fork_hash(
        epoch,
        finalized_slot,
        parent_slot,
        bank_signature_count,
        parent_bank_hash,
        bank_hash,
        blockhash,
        transaction_status_root,
        account_inclusion_root,
        accounts_lt_hash_checksum,
        &bank_hash_hard_fork_data,
    )
    .expect("Solana bank-fork hash");
    iroha_sccp::SccpSolanaFinalityContextV1 {
        version: 1,
        epoch,
        rooted_slot,
        parent_slot,
        tower_vote_slots: tower_vote_slots.clone(),
        parent_bank_hash,
        bank_signature_count,
        bank_hash_hard_fork_data: bank_hash_hard_fork_data.clone(),
        epoch_stake_root: iroha_sccp::sccp_solana_epoch_stake_root(
            epoch,
            &validator_public_keys,
            &solana_validator_stakes(),
        )
        .expect("Solana epoch stake root"),
        stake_activation_hash: iroha_sccp::sccp_solana_stake_activation_hash(
            epoch,
            &validator_public_keys,
            &solana_validator_stakes(),
            &solana_validator_activation_epochs(),
            &solana_validator_deactivation_epochs(),
        )
        .expect("Solana stake activation hash"),
        stake_account_state_hash: iroha_sccp::sccp_solana_stake_account_state_hash(
            epoch,
            &validator_public_keys,
            &solana_validator_delegated_stakes(),
            &solana_validator_activation_epochs(),
            &solana_validator_deactivation_epochs(),
            &solana_validator_vote_account_addresses(),
            &solana_validator_stake_account_addresses(),
            &solana_validator_vote_account_hashes(finalized_slot),
            &solana_validator_stake_account_hashes(),
        )
        .expect("Solana stake-account state hash"),
        stake_history_hash: iroha_sccp::sccp_solana_stake_history_hash(
            epoch,
            &validator_public_keys,
            &solana_validator_stakes(),
            &solana_validator_delegated_stakes(),
            &solana_validator_activation_epochs(),
            &solana_validator_deactivation_epochs(),
            &solana_validator_vote_account_addresses(),
            &solana_validator_stake_account_addresses(),
            &solana_validator_vote_account_hashes(finalized_slot),
            &solana_validator_stake_account_hashes(),
            &solana_stake_history_entries(epoch),
        )
        .expect("Solana stake-history hash"),
        stake_history_sysvar_account_hash: solana_stake_history_sysvar_account_hash(epoch),
        account_inclusion_root,
        accounts_lt_hash_checksum,
        accounts_lt_hash_proof_public_inputs_hash:
            iroha_sccp::sccp_solana_accounts_lt_hash_proof_public_inputs_hash(
                iroha_sccp::SCCP_DOMAIN_SOL,
                finalized_slot,
                parent_slot,
                bank_signature_count,
                parent_bank_hash,
                bank_hash,
                blockhash,
                transaction_status_root,
                account_inclusion_root,
                accounts_lt_hash_checksum,
                &bank_hash_hard_fork_data,
            )
            .expect("Solana AccountsLtHash proof public inputs hash"),
        tower_lockout_hash: iroha_sccp::sccp_solana_tower_lockout_hash(
            epoch,
            finalized_slot,
            rooted_slot,
            parent_slot,
            parent_bank_hash,
        )
        .expect("Solana tower lockout hash"),
        tower_replay_hash: iroha_sccp::sccp_solana_tower_replay_hash(
            epoch,
            finalized_slot,
            rooted_slot,
            parent_slot,
            bank_fork_hash,
            &tower_vote_slots,
        )
        .expect("Solana Tower replay hash"),
        bank_fork_hash,
    }
}

fn solana_vote_proof(
    source_domain: u32,
    finalized_slot: u64,
    blockhash: [u8; 32],
    bank_hash: [u8; 32],
    transaction_status_root: [u8; 32],
    message_proof_hash: [u8; 32],
    finality_context_hash: [u8; 32],
) -> iroha_sccp::SccpSolanaFinalizedVoteProofV1 {
    let signers = solana_vote_keypairs();
    let validator_public_keys = solana_vote_public_keys(&signers);
    let (_, vote_account_branches, stake_account_branches, sysvar_branch) =
        solana_account_inclusion_witness(finalized_slot);
    let vote_message_hash = iroha_sccp::sccp_solana_vote_message_hash(
        source_domain,
        finalized_slot,
        blockhash,
        bank_hash,
        transaction_status_root,
        message_proof_hash,
        finality_context_hash,
    );
    let signatures = signers[..3]
        .iter()
        .map(|signer| {
            iroha_crypto::Signature::new(signer.private_key(), &vote_message_hash)
                .payload()
                .to_vec()
        })
        .collect();
    iroha_sccp::SccpSolanaFinalizedVoteProofV1 {
        version: 1,
        total_stake: 4,
        signed_stake: 3,
        vote_message_hash,
        validator_public_keys,
        validator_stakes: solana_validator_stakes(),
        validator_delegated_stakes: solana_validator_delegated_stakes(),
        validator_activation_epochs: solana_validator_activation_epochs(),
        validator_deactivation_epochs: solana_validator_deactivation_epochs(),
        validator_vote_account_addresses: solana_validator_vote_account_addresses(),
        validator_stake_account_addresses: solana_validator_stake_account_addresses(),
        validator_vote_account_hashes: solana_validator_vote_account_hashes(finalized_slot),
        validator_stake_account_hashes: solana_validator_stake_account_hashes(),
        validator_vote_account_openings: solana_validator_vote_account_openings(finalized_slot),
        validator_stake_account_openings: solana_validator_stake_account_openings(),
        validator_vote_account_data: solana_validator_vote_account_data(finalized_slot),
        validator_stake_account_data: solana_validator_stake_account_data(),
        validator_vote_account_raw_data: solana_validator_vote_account_raw_data(finalized_slot),
        validator_stake_account_raw_data: solana_validator_stake_account_raw_data(),
        validator_vote_account_inclusion_branches: vote_account_branches,
        validator_stake_account_inclusion_branches: stake_account_branches,
        stake_history_sysvar_opening: solana_stake_history_sysvar_opening(
            iroha_sccp::sccp_solana_mainnet_epoch_for_slot(finalized_slot),
        ),
        stake_history_sysvar_raw_data: solana_stake_history_sysvar_raw_data(
            iroha_sccp::sccp_solana_mainnet_epoch_for_slot(finalized_slot),
        ),
        stake_history_sysvar_inclusion_branch: sysvar_branch,
        stake_history_entries: solana_stake_history_entries(
            iroha_sccp::sccp_solana_mainnet_epoch_for_slot(finalized_slot),
        ),
        accounts_lt_hash: solana_accounts_lt_hash(finalized_slot),
        accounts_lt_hash_proof: iroha_sccp::SccpSourceStateVerificationProofV1::default(),
        tower_replay_verification_proof: iroha_sccp::SccpSourceStateVerificationProofV1::default(),
        full_accountsdb_lattice_verification_proof:
            iroha_sccp::SccpSourceStateVerificationProofV1::default(),
        bank_fork_choice_verification_proof:
            iroha_sccp::SccpSourceStateVerificationProofV1::default(),
        signers_bitmap: vec![0b0000_0111],
        signatures,
    }
}

fn solana_transaction_signature() -> Vec<u8> {
    vec![0x55; 64]
}

fn solana_emitter_program_id() -> Vec<u8> {
    vec![0x42; 32]
}

fn configured_sol_source_verifier_material() -> iroha_sccp::SccpSourceVerifierMaterialV1 {
    let material =
        iroha_sccp::sccp_solana_mainnet_source_verifier_material_with_hashes_and_accounts_db_v1(
            solana_vote_roster_hash(),
            [0x22; 32],
            [0x33; 32],
            [0x35; 32],
            [0x44; 32],
        )
        .expect("SOL mainnet source verifier material");
    assert!(iroha_sccp::sccp_source_verifier_material_is_production_ready(&material));
    material
}

fn generic_sol_source_verifier_material() -> iroha_sccp::SccpSourceVerifierMaterialV1 {
    let mut material =
        iroha_sccp::sccp_source_verifier_material_for_domain(iroha_sccp::SCCP_DOMAIN_SOL)
            .expect("SOL source verifier material");
    material.placeholder_material = false;
    material.source_trust_anchor_id = "sccp:sol:source-trust-anchor:mainnet:v1".to_owned();
    material.source_trust_anchor_hash = solana_vote_roster_hash();
    material.consensus_verifier_id = "sccp:sol:consensus-verifier:mainnet:v1".to_owned();
    material.consensus_verifier_hash = [0x22; 32];
    material.message_inclusion_verifier_id =
        "sccp:sol:message-inclusion-verifier:mainnet:v1".to_owned();
    material.message_inclusion_verifier_hash = [0x33; 32];
    material.source_state_verifier_id = "sccp:sol:source-state-verifier:mainnet:v1".to_owned();
    material.source_state_verifier_hash = [0x35; 32];
    material.finality_policy_id = "sccp:sol:finality-policy:mainnet:v1".to_owned();
    material.finality_policy_hash = [0x44; 32];
    assert!(
        !iroha_sccp::sccp_source_verifier_material_is_production_ready(&material),
        "generic SOL material must remain fail-closed until it matches the exact mainnet profile"
    );
    material
}

fn actual_source_verifier_material(
    material: &iroha_sccp::SccpSourceVerifierMaterialV1,
) -> iroha_config::parameters::actual::SccpSourceVerifierMaterial {
    iroha_config::parameters::actual::SccpSourceVerifierMaterial {
        version: material.version,
        source_domain: material.source_domain,
        source_chain: material.source_chain.clone(),
        source_proof_plan: material.source_proof_plan.as_str().to_owned(),
        finality_model: material.finality_model.as_str().to_owned(),
        adapter_circuit_id: material.adapter_circuit_id.clone(),
        source_trust_anchor_id: material.source_trust_anchor_id.clone(),
        source_trust_anchor_hash: hex::encode(material.source_trust_anchor_hash),
        consensus_verifier_id: material.consensus_verifier_id.clone(),
        consensus_verifier_hash: hex::encode(material.consensus_verifier_hash),
        message_inclusion_verifier_id: material.message_inclusion_verifier_id.clone(),
        message_inclusion_verifier_hash: hex::encode(material.message_inclusion_verifier_hash),
        source_state_verifier_id: material.source_state_verifier_id.clone(),
        source_state_verifier_hash: hex::encode(material.source_state_verifier_hash),
        source_bridge_emitter_id: material.source_bridge_emitter_id.clone(),
        source_bridge_emitter_address: hex::encode(&material.source_bridge_emitter_address),
        source_bridge_emitter_code_hash: hex::encode(material.source_bridge_emitter_code_hash),
        source_bridge_network_id: hex::encode(material.source_bridge_network_id),
        source_bridge_owner_address: hex::encode(&material.source_bridge_owner_address),
        source_bridge_config_hash: hex::encode(material.source_bridge_config_hash),
        finality_policy_id: material.finality_policy_id.clone(),
        finality_policy_hash: hex::encode(material.finality_policy_hash),
        placeholder_material: material.placeholder_material,
    }
}

fn configured_sol_source_adapter_engine_deployment(
    material: &iroha_sccp::SccpSourceVerifierMaterialV1,
) -> iroha_sccp::SccpSourceAdapterEngineDeploymentV1 {
    let deployment =
        iroha_sccp::sccp_source_adapter_engine_deployment_from_material_v1(material, [0x55; 32])
            .expect("SOL source adapter engine deployment");
    assert!(
        iroha_sccp::sccp_source_adapter_engine_deployment_matches_material(material, &deployment,)
    );
    assert!(
        !iroha_sccp::sccp_source_adapter_ready_with_material_and_deployment_for_domain(
            iroha_sccp::SCCP_DOMAIN_SOL,
            material,
            &deployment,
        ),
        "configured Solana deployment metadata remains fail-closed until audited full-light-client evidence is attached"
    );
    deployment
}

fn configured_ton_source_verifier_material() -> iroha_sccp::SccpSourceVerifierMaterialV1 {
    let material =
        iroha_sccp::sccp_ton_mainnet_source_verifier_material_with_hashes_and_shard_state_v1(
            [0x44; 32], [0x55; 32], [0x66; 32], [0x77; 32], [0x88; 32],
        )
        .expect("TON mainnet source verifier material");
    assert!(iroha_sccp::sccp_source_verifier_material_is_production_ready(&material));
    material
}

fn configured_ton_source_adapter_engine_deployment(
    material: &iroha_sccp::SccpSourceVerifierMaterialV1,
) -> iroha_sccp::SccpSourceAdapterEngineDeploymentV1 {
    let deployment =
        iroha_sccp::sccp_source_adapter_engine_deployment_from_material_v1(material, [0x25; 32])
            .expect("TON source adapter engine deployment");
    assert!(
        iroha_sccp::sccp_source_adapter_engine_deployment_matches_material(material, &deployment,)
    );
    assert!(
        !iroha_sccp::sccp_source_adapter_ready_with_material_and_deployment_for_domain(
            iroha_sccp::SCCP_DOMAIN_TON,
            material,
            &deployment,
        ),
        "configured TON deployment metadata remains fail-closed until audited full-light-client evidence is attached"
    );
    deployment
}

fn configured_tron_source_verifier_material() -> iroha_sccp::SccpSourceVerifierMaterialV1 {
    let source_bridge_address = [0x42; 20];
    let source_bridge_network_id = [0x43; 32];
    let source_bridge_owner_address = [0x44; 20];
    let source_bridge_config_hash = iroha_sccp::sccp_tron_source_bridge_config_hash_v1(
        source_bridge_network_id,
        iroha_sccp::SCCP_DOMAIN_TRON,
        iroha_sccp::SCCP_DOMAIN_SORA,
        source_bridge_address,
        source_bridge_owner_address,
    )
    .expect("TRON source bridge config hash");
    let material =
        iroha_sccp::sccp_tron_mainnet_source_verifier_material_with_hashes_and_emitter_v1(
            [0x31; 32],
            [0x32; 32],
            [0x33; 32],
            source_bridge_address,
            [0x34; 32],
            source_bridge_network_id,
            source_bridge_owner_address,
            source_bridge_config_hash,
            [0x35; 32],
        )
        .expect("TRON mainnet source verifier material");
    assert!(iroha_sccp::sccp_source_verifier_material_is_production_ready(&material));
    material
}

fn configured_tron_source_adapter_engine_deployment(
    material: &iroha_sccp::SccpSourceVerifierMaterialV1,
) -> iroha_sccp::SccpSourceAdapterEngineDeploymentV1 {
    let deployment =
        iroha_sccp::sccp_source_adapter_engine_deployment_from_material_v1(material, [0x36; 32])
            .expect("TRON source adapter engine deployment");
    assert!(
        iroha_sccp::sccp_source_adapter_engine_deployment_matches_material(material, &deployment,)
    );
    assert!(
        iroha_sccp::sccp_source_adapter_ready_with_material_and_deployment_for_domain(
            iroha_sccp::SCCP_DOMAIN_TRON,
            material,
            &deployment,
        ),
        "configured TRON deployment evidence should open the source-adapter gate"
    );
    deployment
}

fn actual_source_adapter_engine_deployment(
    deployment: &iroha_sccp::SccpSourceAdapterEngineDeploymentV1,
) -> iroha_config::parameters::actual::SccpSourceAdapterEngineDeployment {
    iroha_config::parameters::actual::SccpSourceAdapterEngineDeployment {
        version: deployment.version,
        source_domain: deployment.source_domain,
        target_domain: deployment.target_domain,
        source_chain: deployment.source_chain.clone(),
        source_proof_plan: deployment.source_proof_plan.as_str().to_owned(),
        finality_model: deployment.finality_model.as_str().to_owned(),
        adapter_proof_family: deployment.adapter_proof_family.clone(),
        adapter_circuit_id: deployment.adapter_circuit_id.clone(),
        adapter_verifier_vk_hash: hex::encode(deployment.adapter_verifier_vk_hash),
        source_trust_anchor_id: deployment.source_trust_anchor_id.clone(),
        source_trust_anchor_hash: hex::encode(deployment.source_trust_anchor_hash),
        consensus_verifier_id: deployment.consensus_verifier_id.clone(),
        consensus_verifier_hash: hex::encode(deployment.consensus_verifier_hash),
        message_inclusion_verifier_id: deployment.message_inclusion_verifier_id.clone(),
        message_inclusion_verifier_hash: hex::encode(deployment.message_inclusion_verifier_hash),
        source_state_verifier_id: deployment.source_state_verifier_id.clone(),
        source_state_verifier_hash: hex::encode(deployment.source_state_verifier_hash),
        source_bridge_emitter_id: deployment.source_bridge_emitter_id.clone(),
        source_bridge_emitter_address: hex::encode(&deployment.source_bridge_emitter_address),
        source_bridge_emitter_code_hash: hex::encode(deployment.source_bridge_emitter_code_hash),
        source_bridge_network_id: hex::encode(deployment.source_bridge_network_id),
        source_bridge_owner_address: hex::encode(&deployment.source_bridge_owner_address),
        source_bridge_config_hash: hex::encode(deployment.source_bridge_config_hash),
        finality_policy_id: deployment.finality_policy_id.clone(),
        finality_policy_hash: hex::encode(deployment.finality_policy_hash),
        deployment_receipt_hash: hex::encode(deployment.deployment_receipt_hash),
        solana_tower_replay_verifier_hash: String::new(),
        solana_full_accountsdb_lattice_verifier_hash: String::new(),
        solana_bank_fork_choice_verifier_hash: String::new(),
        solana_full_light_client_gate_hash: String::new(),
        ton_masterchain_config_verifier_hash: String::new(),
        ton_validator_set_transition_verifier_hash: String::new(),
        ton_shard_accounts_dictionary_verifier_hash: String::new(),
        ton_full_light_client_gate_hash: String::new(),
        tron_dpos_source_gate_hash: String::new(),
    }
}

fn attach_tron_dpos_source_gate(
    deployment_config: &mut iroha_config::parameters::actual::SccpSourceAdapterEngineDeployment,
    material: &iroha_sccp::SccpSourceVerifierMaterialV1,
    deployment: &iroha_sccp::SccpSourceAdapterEngineDeploymentV1,
) {
    let gate_hash =
        iroha_sccp::sccp_tron_dpos_source_gate_hash_from_deployment_v1(material, deployment)
            .expect("TRON DPoS source gate hash");
    deployment_config.tron_dpos_source_gate_hash = hex::encode(gate_hash);
}

fn attach_ton_full_light_client_audit(
    deployment_config: &mut iroha_config::parameters::actual::SccpSourceAdapterEngineDeployment,
    material: &iroha_sccp::SccpSourceVerifierMaterialV1,
    deployment: &iroha_sccp::SccpSourceAdapterEngineDeploymentV1,
) {
    let audited_deployment = audited_ton_source_adapter_engine_deployment(deployment);
    let gate_hash = iroha_sccp::sccp_ton_full_light_client_gate_hash_from_deployment_v1(
        material,
        &audited_deployment,
    )
    .expect("TON full-light-client audit hash");
    deployment_config.ton_masterchain_config_verifier_hash =
        hex::encode(audited_deployment.ton_masterchain_config_verifier_hash);
    deployment_config.ton_validator_set_transition_verifier_hash =
        hex::encode(audited_deployment.ton_validator_set_transition_verifier_hash);
    deployment_config.ton_shard_accounts_dictionary_verifier_hash =
        hex::encode(audited_deployment.ton_shard_accounts_dictionary_verifier_hash);
    deployment_config.ton_full_light_client_gate_hash = hex::encode(gate_hash);
}

fn attach_solana_full_light_client_audit(
    deployment_config: &mut iroha_config::parameters::actual::SccpSourceAdapterEngineDeployment,
    material: &iroha_sccp::SccpSourceVerifierMaterialV1,
    deployment: &iroha_sccp::SccpSourceAdapterEngineDeploymentV1,
) {
    let audited_deployment = audited_sol_source_adapter_engine_deployment(deployment);
    let gate_hash = iroha_sccp::sccp_solana_full_light_client_gate_hash_from_deployment_v1(
        material,
        &audited_deployment,
    )
    .expect("Solana full-light-client audit hash");
    deployment_config.solana_tower_replay_verifier_hash =
        hex::encode(audited_deployment.solana_tower_replay_verifier_hash);
    deployment_config.solana_full_accountsdb_lattice_verifier_hash =
        hex::encode(audited_deployment.solana_full_accountsdb_lattice_verifier_hash);
    deployment_config.solana_bank_fork_choice_verifier_hash =
        hex::encode(audited_deployment.solana_bank_fork_choice_verifier_hash);
    deployment_config.solana_full_light_client_gate_hash = hex::encode(gate_hash);
}

fn audited_sol_source_adapter_engine_deployment(
    deployment: &iroha_sccp::SccpSourceAdapterEngineDeploymentV1,
) -> iroha_sccp::SccpSourceAdapterEngineDeploymentV1 {
    let mut audited_deployment = deployment.clone();
    audited_deployment.solana_tower_replay_verifier_hash = [0xbb; 32];
    audited_deployment.solana_full_accountsdb_lattice_verifier_hash = [0xcc; 32];
    audited_deployment.solana_bank_fork_choice_verifier_hash = [0xdd; 32];
    audited_deployment
}

fn audited_ton_source_adapter_engine_deployment(
    deployment: &iroha_sccp::SccpSourceAdapterEngineDeploymentV1,
) -> iroha_sccp::SccpSourceAdapterEngineDeploymentV1 {
    let mut audited_deployment = deployment.clone();
    audited_deployment.ton_masterchain_config_verifier_hash = [0x26; 32];
    audited_deployment.ton_validator_set_transition_verifier_hash = [0x27; 32];
    audited_deployment.ton_shard_accounts_dictionary_verifier_hash = [0x28; 32];
    audited_deployment
}

fn configured_sol_destination_rollout() -> iroha_sccp::SccpDestinationRolloutV1 {
    let rollout = iroha_sccp::sccp_solana_mainnet_destination_rollout_with_live_evidence_v1(
        "3JF3sEqM796hk5WFqA6EtmEwJQ9quALszsfJyvXNQKy3".to_owned(),
        "0xc81178d11a4de525782fe7ac6f5accc2056fa15d1b8c2bfd819eb2ef179c3411".to_owned(),
        "29d2S7vB453rNYFdR5Ycwt7y9haRT5fwVwL9zTmBhfV2".to_owned(),
        "4321".to_owned(),
        "5000".to_owned(),
        "5001".to_owned(),
        "f0VMRgECAwQF".to_owned(),
    )
    .expect("SOL destination rollout");
    assert!(iroha_sccp::sccp_destination_rollout_is_production_ready(
        iroha_sccp::SCCP_DOMAIN_SOL,
        &rollout,
    ));
    rollout
}

fn configured_ton_destination_rollout() -> iroha_sccp::SccpDestinationRolloutV1 {
    let rollout = iroha_sccp::sccp_ton_mainnet_destination_rollout_with_live_evidence_v1(
        format!("0:{}", "11".repeat(32)),
        "0x49725ad44ef5ed5feaa27f88679cabae427209a6bea318cb9b66030131aae6fe".to_owned(),
        hex::encode([0x67; 32]),
        "123456".to_owned(),
        hex::encode([0x68; 32]),
        "0xb5ee9c720101020100070001020101000202".to_owned(),
    )
    .expect("TON destination rollout");
    assert!(iroha_sccp::sccp_destination_rollout_is_production_ready(
        iroha_sccp::SCCP_DOMAIN_TON,
        &rollout,
    ));
    rollout
}

fn configured_tron_destination_rollout(
    material: &iroha_sccp::SccpSourceVerifierMaterialV1,
) -> iroha_sccp::SccpDestinationRolloutV1 {
    let rollout = iroha_sccp::sccp_tron_mainnet_destination_rollout_with_binding_v1(
        "TJRabPrwbZy45sbavfcjinPJC18kjpRTv8".to_owned(),
        hex::encode([0x66; 32]),
        hex::encode([0x67; 32]),
        hex::encode(material.source_bridge_network_id),
    )
    .expect("TRON destination rollout");
    assert!(iroha_sccp::sccp_destination_rollout_is_production_ready(
        iroha_sccp::SCCP_DOMAIN_TRON,
        &rollout,
    ));
    rollout
}

fn actual_destination_rollout(
    rollout: &iroha_sccp::SccpDestinationRolloutV1,
) -> iroha_config::parameters::actual::SccpDestinationRollout {
    iroha_config::parameters::actual::SccpDestinationRollout {
        version: rollout.version,
        domain: rollout.domain,
        chain: rollout.chain.clone(),
        verifier_plan: rollout.verifier_plan.as_str().to_owned(),
        immutable_verifier_ready: rollout.immutable_verifier_ready,
        anchors_ready: rollout.anchors_ready,
        verifier_identity: rollout.verifier_identity.clone(),
        verifier_code_hash: rollout.verifier_code_hash.clone(),
        verifier_key_hash: rollout.verifier_key_hash.clone(),
        destination_network_id: rollout.destination_network_id.clone(),
        destination_bridge_address: rollout.destination_bridge_address.clone(),
        destination_binding_key: rollout.destination_binding_key.clone(),
        destination_binding_hash: rollout.destination_binding_hash.clone(),
        anchor_id: rollout.anchor_id.clone(),
        solana_rpc_commitment: rollout.solana_rpc_commitment.clone(),
        solana_program_owner: rollout.solana_program_owner.clone(),
        solana_programdata_owner: rollout.solana_programdata_owner.clone(),
        solana_program_immutable: rollout.solana_program_immutable,
        solana_program_account_data_base64: rollout.solana_program_account_data_base64.clone(),
        solana_programdata_address: rollout.solana_programdata_address.clone(),
        solana_programdata_slot: rollout.solana_programdata_slot.clone(),
        solana_expected_programdata_slot: rollout.solana_expected_programdata_slot.clone(),
        solana_program_account_context_slot: rollout.solana_program_account_context_slot.clone(),
        solana_programdata_account_context_slot: rollout
            .solana_programdata_account_context_slot
            .clone(),
        solana_programdata_metadata_blake2b256: rollout
            .solana_programdata_metadata_blake2b256
            .clone(),
        solana_programdata_metadata_base64: rollout.solana_programdata_metadata_base64.clone(),
        solana_programdata_executable_blake2b256: rollout
            .solana_programdata_executable_blake2b256
            .clone(),
        solana_programdata_executable_base64: rollout.solana_programdata_executable_base64.clone(),
        ton_account_status: rollout.ton_account_status.clone(),
        ton_account_state_hash: rollout.ton_account_state_hash.clone(),
        ton_last_transaction_lt: rollout.ton_last_transaction_lt.clone(),
        ton_last_transaction_hash: rollout.ton_last_transaction_hash.clone(),
        ton_verifier_code_boc_root_hash: rollout.ton_verifier_code_boc_root_hash.clone(),
        ton_verifier_code_boc: rollout.ton_verifier_code_boc.clone(),
        substrate_finalized_head: rollout.substrate_finalized_head.clone(),
        substrate_runtime_spec_name: rollout.substrate_runtime_spec_name.clone(),
        substrate_runtime_spec_version: rollout.substrate_runtime_spec_version.clone(),
        substrate_runtime_transaction_version: rollout
            .substrate_runtime_transaction_version
            .clone(),
        substrate_runtime_code_hash: rollout.substrate_runtime_code_hash.clone(),
        substrate_runtime_code_base64: rollout.substrate_runtime_code_base64.clone(),
        blockers: rollout.blockers.clone(),
    }
}

fn configured_sol_route_allowlist(
    material: &iroha_sccp::SccpSourceVerifierMaterialV1,
    deployment: &iroha_sccp::SccpSourceAdapterEngineDeploymentV1,
    destination_rollout: &iroha_sccp::SccpDestinationRolloutV1,
) -> iroha_sccp::SccpRouteAllowlistReadinessV1 {
    let allowlist = iroha_sccp::sccp_profiled_route_allowlist_for_lane_evidence_v1(
        iroha_sccp::SCCP_DOMAIN_SOL,
        material,
        deployment,
        destination_rollout,
    )
    .expect("SOL route allowlist");
    let allowlist = with_solana_route_canary(allowlist, material, deployment, destination_rollout);
    assert!(iroha_sccp::sccp_route_allowlist_is_production_ready(
        iroha_sccp::SCCP_DOMAIN_SOL,
        &allowlist,
    ));
    allowlist
}

fn configured_ton_route_allowlist(
    material: &iroha_sccp::SccpSourceVerifierMaterialV1,
    deployment: &iroha_sccp::SccpSourceAdapterEngineDeploymentV1,
    destination_rollout: &iroha_sccp::SccpDestinationRolloutV1,
) -> iroha_sccp::SccpRouteAllowlistReadinessV1 {
    let allowlist = iroha_sccp::sccp_profiled_route_allowlist_for_lane_evidence_v1(
        iroha_sccp::SCCP_DOMAIN_TON,
        material,
        deployment,
        destination_rollout,
    )
    .expect("TON route allowlist");
    let allowlist = with_ton_route_canary(allowlist, material, deployment, destination_rollout);
    assert!(iroha_sccp::sccp_route_allowlist_is_production_ready(
        iroha_sccp::SCCP_DOMAIN_TON,
        &allowlist,
    ));
    allowlist
}

fn configured_tron_route_allowlist(
    material: &iroha_sccp::SccpSourceVerifierMaterialV1,
    deployment: &iroha_sccp::SccpSourceAdapterEngineDeploymentV1,
    destination_rollout: &iroha_sccp::SccpDestinationRolloutV1,
) -> iroha_sccp::SccpRouteAllowlistReadinessV1 {
    let allowlist = iroha_sccp::sccp_profiled_route_allowlist_for_lane_evidence_v1(
        iroha_sccp::SCCP_DOMAIN_TRON,
        material,
        deployment,
        destination_rollout,
    )
    .expect("TRON route allowlist");
    let allowlist = with_tron_route_canary(allowlist, material, deployment, destination_rollout);
    assert!(iroha_sccp::sccp_route_allowlist_is_production_ready(
        iroha_sccp::SCCP_DOMAIN_TRON,
        &allowlist,
    ));
    allowlist
}

fn with_solana_route_canary(
    allowlist: iroha_sccp::SccpRouteAllowlistReadinessV1,
    material: &iroha_sccp::SccpSourceVerifierMaterialV1,
    deployment: &iroha_sccp::SccpSourceAdapterEngineDeploymentV1,
    destination_rollout: &iroha_sccp::SccpDestinationRolloutV1,
) -> iroha_sccp::SccpRouteAllowlistReadinessV1 {
    let binding_hash_hex = destination_rollout
        .destination_binding_hash
        .as_deref()
        .expect("destination binding hash");
    let destination_binding_hash: [u8; 32] = hex::decode(binding_hash_hex.trim_start_matches("0x"))
        .expect("decode destination binding hash")
        .try_into()
        .expect("destination binding hash length");
    iroha_sccp::sccp_solana_route_allowlist_with_lane_canary_evidence_v1(
        allowlist,
        destination_rollout,
        destination_binding_hash,
        iroha_sccp::sccp_source_verifier_material_hash(material),
        iroha_sccp::sccp_source_adapter_engine_deployment_hash(deployment),
    )
    .expect("Solana route canary evidence")
}

fn with_ton_route_canary(
    allowlist: iroha_sccp::SccpRouteAllowlistReadinessV1,
    material: &iroha_sccp::SccpSourceVerifierMaterialV1,
    deployment: &iroha_sccp::SccpSourceAdapterEngineDeploymentV1,
    destination_rollout: &iroha_sccp::SccpDestinationRolloutV1,
) -> iroha_sccp::SccpRouteAllowlistReadinessV1 {
    let binding_hash_hex = destination_rollout
        .destination_binding_hash
        .as_deref()
        .expect("destination binding hash");
    let destination_binding_hash: [u8; 32] = hex::decode(binding_hash_hex.trim_start_matches("0x"))
        .expect("decode destination binding hash")
        .try_into()
        .expect("destination binding hash length");
    let account_state_hash: [u8; 32] = hex::decode(
        destination_rollout
            .ton_account_state_hash
            .as_deref()
            .expect("TON account state hash")
            .trim_start_matches("0x"),
    )
    .expect("decode TON account state hash")
    .try_into()
    .expect("TON account state hash length");
    let last_transaction_hash: [u8; 32] = hex::decode(
        destination_rollout
            .ton_last_transaction_hash
            .as_deref()
            .expect("TON last transaction hash")
            .trim_start_matches("0x"),
    )
    .expect("decode TON last transaction hash")
    .try_into()
    .expect("TON last transaction hash length");
    iroha_sccp::sccp_ton_route_allowlist_with_lane_canary_evidence_v1(
        allowlist,
        destination_rollout,
        destination_binding_hash,
        iroha_sccp::sccp_source_verifier_material_hash(material),
        iroha_sccp::sccp_source_adapter_engine_deployment_hash(deployment),
        account_state_hash,
        destination_rollout
            .ton_last_transaction_lt
            .clone()
            .expect("TON last transaction LT"),
        last_transaction_hash,
    )
    .expect("TON route canary evidence")
}

fn with_tron_route_canary(
    allowlist: iroha_sccp::SccpRouteAllowlistReadinessV1,
    material: &iroha_sccp::SccpSourceVerifierMaterialV1,
    deployment: &iroha_sccp::SccpSourceAdapterEngineDeploymentV1,
    destination_rollout: &iroha_sccp::SccpDestinationRolloutV1,
) -> iroha_sccp::SccpRouteAllowlistReadinessV1 {
    let binding_hash_hex = destination_rollout
        .destination_binding_hash
        .as_deref()
        .expect("destination binding hash");
    let destination_binding_hash: [u8; 32] = hex::decode(binding_hash_hex.trim_start_matches("0x"))
        .expect("decode destination binding hash")
        .try_into()
        .expect("destination binding hash length");
    iroha_sccp::sccp_tron_route_allowlist_with_lane_canary_evidence_v1(
        allowlist,
        destination_rollout,
        destination_binding_hash,
        iroha_sccp::sccp_source_verifier_material_hash(material),
        iroha_sccp::sccp_source_adapter_engine_deployment_hash(deployment),
        [0xfa; 32],
        [0x41; 21],
        234,
        567_000,
        0,
        [0xdd; 32],
        [0xa1; 32],
        [0xab; 32],
        iroha_sccp::SCCP_DOMAIN_TRON,
        [0xf1; 32],
        [0xee; 32],
        [0xa2; 32],
        [0xcd; 32],
        1,
        iroha_sccp::SCCP_DOMAIN_SORA,
        true,
        true,
        [0x5a; 32],
        [0x41; 21],
        true,
    )
    .expect("TRON route canary evidence")
}

fn actual_route_allowlist(
    allowlist: &iroha_sccp::SccpRouteAllowlistReadinessV1,
) -> iroha_config::parameters::actual::SccpRouteAllowlist {
    iroha_config::parameters::actual::SccpRouteAllowlist {
        version: allowlist.version,
        domain: allowlist.domain,
        chain: allowlist.chain.clone(),
        activation_policy: allowlist.activation_policy.as_str().to_owned(),
        route_allowlist_id: allowlist.route_allowlist_id.clone(),
        route_allowlist_hash: allowlist.route_allowlist_hash.clone(),
        route_canary_status: allowlist.route_canary_status.clone(),
        route_canary_evidence_hash: allowlist.route_canary_evidence_hash.clone(),
        route_canary_route_allowlist_hash: allowlist.route_canary_route_allowlist_hash.clone(),
        route_canary_destination_binding_hash: allowlist
            .route_canary_destination_binding_hash
            .clone(),
        evm_route_canary_transaction_hash: allowlist.evm_route_canary_transaction_hash.clone(),
        evm_route_canary_log_index: allowlist.evm_route_canary_log_index,
        evm_route_canary_call_data_sha256: allowlist.evm_route_canary_call_data_sha256.clone(),
        evm_route_canary_message_id: allowlist.evm_route_canary_message_id.clone(),
        evm_route_canary_payload_hash: allowlist.evm_route_canary_payload_hash.clone(),
        evm_route_canary_target_domain: allowlist.evm_route_canary_target_domain,
        evm_route_canary_statement_hash: allowlist.evm_route_canary_statement_hash.clone(),
        evm_route_canary_commitment_root: allowlist.evm_route_canary_commitment_root.clone(),
        evm_route_canary_finality_height: allowlist.evm_route_canary_finality_height.clone(),
        evm_route_canary_finality_block_hash: allowlist
            .evm_route_canary_finality_block_hash
            .clone(),
        evm_route_canary_proof_version: allowlist.evm_route_canary_proof_version,
        evm_route_canary_proof_source_domain: allowlist.evm_route_canary_proof_source_domain,
        evm_route_canary_used_message_proof: allowlist.evm_route_canary_used_message_proof,
        tron_route_canary_transaction_id: allowlist.tron_route_canary_transaction_id.clone(),
        tron_route_canary_transaction_owner_address: allowlist
            .tron_route_canary_transaction_owner_address
            .clone(),
        tron_route_canary_block_number: allowlist.tron_route_canary_block_number,
        tron_route_canary_block_timestamp: allowlist.tron_route_canary_block_timestamp,
        tron_route_canary_log_index: allowlist.tron_route_canary_log_index,
        tron_route_canary_message_id: allowlist.tron_route_canary_message_id.clone(),
        tron_route_canary_call_data_sha256: allowlist.tron_route_canary_call_data_sha256.clone(),
        tron_route_canary_payload_hash: allowlist.tron_route_canary_payload_hash.clone(),
        tron_route_canary_target_domain: allowlist.tron_route_canary_target_domain,
        tron_route_canary_statement_hash: allowlist.tron_route_canary_statement_hash.clone(),
        tron_route_canary_commitment_root: allowlist.tron_route_canary_commitment_root.clone(),
        tron_route_canary_finality_height: allowlist.tron_route_canary_finality_height.clone(),
        tron_route_canary_finality_block_hash: allowlist
            .tron_route_canary_finality_block_hash
            .clone(),
        tron_route_canary_proof_version: allowlist.tron_route_canary_proof_version,
        tron_route_canary_proof_source_domain: allowlist.tron_route_canary_proof_source_domain,
        tron_route_canary_used_message_proof: allowlist.tron_route_canary_used_message_proof,
        tron_route_canary_raw_data_owner_matches_transaction: allowlist
            .tron_route_canary_raw_data_owner_matches_transaction,
        tron_route_canary_signature_sha256: allowlist.tron_route_canary_signature_sha256.clone(),
        tron_route_canary_signature_recovered_address: allowlist
            .tron_route_canary_signature_recovered_address
            .clone(),
        tron_route_canary_signature_recovers_to_owner: allowlist
            .tron_route_canary_signature_recovers_to_owner,
        ton_route_canary_account_state_hash: allowlist.ton_route_canary_account_state_hash.clone(),
        ton_route_canary_last_transaction_lt: allowlist
            .ton_route_canary_last_transaction_lt
            .clone(),
        ton_route_canary_last_transaction_hash: allowlist
            .ton_route_canary_last_transaction_hash
            .clone(),
        routes_allowlisted: allowlist.routes_allowlisted,
        blockers: allowlist.blockers.clone(),
    }
}

fn make_ics_proof(leaf_fill: u8, range: (u64, u64), pinned: bool) -> BridgeProof {
    let leaves = vec![[leaf_fill; 32], [leaf_fill.wrapping_add(1); 32]];
    let tree = iroha_crypto::MerkleTree::<[u8; 32]>::from_hashed_leaves_sha256(leaves.clone());
    let root_bytes: [u8; 32] = *tree.root().expect("root").as_ref();
    let proof = tree.get_proof(0).expect("proof");

    BridgeProof {
        range: BridgeProofRange {
            start_height: range.0,
            end_height: range.1,
        },
        manifest_hash: [0xAA; 32],
        payload: BridgeProofPayload::Ics(BridgeIcsProof {
            state_root: root_bytes,
            leaf_hash: leaves[0],
            proof,
            hash_function: BridgeHashFunction::Sha256,
        }),
        pinned,
    }
}

fn make_sccp_sol_to_sora_message_bridge_proof(nonce: u64) -> BridgeProof {
    make_sccp_sol_to_sora_message_bridge_proof_with_material(nonce, None)
}

fn make_sccp_sol_to_sora_message_bridge_proof_with_material(
    nonce: u64,
    source_material: Option<&iroha_sccp::SccpSourceVerifierMaterialV1>,
) -> BridgeProof {
    make_sccp_sol_to_sora_message_bridge_proof_with_material_and_deployment(
        nonce,
        source_material,
        None,
    )
}

fn make_sccp_sol_to_sora_message_bridge_proof_with_material_and_deployment(
    nonce: u64,
    source_material: Option<&iroha_sccp::SccpSourceVerifierMaterialV1>,
    source_deployment: Option<&iroha_sccp::SccpSourceAdapterEngineDeploymentV1>,
) -> BridgeProof {
    let payload = iroha_sccp::SccpPayloadV1::Transfer(iroha_sccp::TransferPayloadV1 {
        version: 1,
        source_domain: iroha_sccp::SCCP_DOMAIN_SOL,
        dest_domain: iroha_sccp::SCCP_DOMAIN_SORA,
        nonce,
        asset_home_domain: iroha_sccp::SCCP_DOMAIN_SOL,
        asset_id_codec: iroha_sccp::SCCP_CODEC_TEXT_UTF8,
        asset_id: b"wsol#sol".to_vec(),
        amount: 7,
        sender_codec: iroha_sccp::SCCP_CODEC_SOLANA_BASE58,
        sender: b"11111111111111111111111111111111".to_vec(),
        recipient_codec: iroha_sccp::SCCP_CODEC_TEXT_UTF8,
        recipient: b"alice@universal".to_vec(),
        route_id_codec: iroha_sccp::SCCP_CODEC_TEXT_UTF8,
        route_id: b"sol:sora:wsol".to_vec(),
    });
    let payload_hash =
        iroha_sccp::payload_hash(&iroha_sccp::canonical_sccp_payload_bytes(&payload));
    let commitment = iroha_sccp::SccpHubCommitmentV1 {
        version: 1,
        kind: iroha_sccp::sccp_message_kind(&payload),
        target_domain: iroha_sccp::sccp_message_target_domain(&payload),
        message_id: iroha_sccp::sccp_message_id(&payload),
        payload_hash,
    };
    let merkle_proof = iroha_sccp::SccpMerkleProofV1 { steps: Vec::new() };
    let commitment_root = iroha_sccp::merkle_root_from_commitment(&commitment, &merkle_proof);

    let source_domain = iroha_sccp::SCCP_DOMAIN_SOL;
    let target_domain = iroha_sccp::SCCP_DOMAIN_SORA;
    let source_chain = iroha_sccp::sccp_chain_key_for_domain(source_domain)
        .expect("SOL chain key")
        .to_owned();
    let source_proof_plan = iroha_sccp::sccp_source_proof_plan_for_domain(source_domain)
        .expect("SOL source proof plan");
    let finality_model = iroha_sccp::sccp_proof_finality_model_for_domain(source_domain)
        .expect("SOL finality model");
    let finality_height = iroha_sccp::SCCP_SOLANA_MAINNET_SLOTS_PER_EPOCH + 64;
    let finality_block_hash = [0x55; 32];
    let bank_hash = solana_bank_hash(finality_height, finality_block_hash);
    let source_event_digest = iroha_sccp::sccp_source_event_digest(
        source_domain,
        target_domain,
        commitment.message_id,
        commitment.payload_hash,
    );
    let transaction_signature = solana_transaction_signature();
    let emitter_program_id = solana_emitter_program_id();
    let source_event_leaf_hash = iroha_sccp::sccp_solana_transaction_status_leaf_hash(
        source_event_digest,
        &transaction_signature,
        &emitter_program_id,
    )
    .expect("Solana transaction-status leaf hash");
    let inclusion_branch = vec![[0x44; 32].to_vec()];
    let receipt_or_message_root = iroha_sccp::sccp_source_message_root_from_branch(
        source_event_leaf_hash,
        0,
        &inclusion_branch,
    )
    .expect("source receipt root");
    let message_proof_hash = iroha_sccp::sccp_solana_message_proof_hash(
        source_event_digest,
        receipt_or_message_root,
        &transaction_signature,
        &emitter_program_id,
        &inclusion_branch,
    )
    .expect("Solana source message proof hash");
    let finality_context = solana_finality_context(
        finality_height,
        finality_block_hash,
        bank_hash,
        receipt_or_message_root,
    );
    let finality_context_hash = iroha_sccp::sccp_solana_finality_context_hash(&finality_context);
    let finalized_header_hash = iroha_sccp::sccp_source_finalized_header_hash(
        source_domain,
        finality_model,
        finality_height,
        finality_block_hash,
        receipt_or_message_root,
    );
    let mut adapter_proof = iroha_sccp::SccpSourceAdapterProofV1::SolanaFinalizedTransaction(
        iroha_sccp::SccpSolanaFinalizedSourceProofV1 {
            version: 1,
            source_domain,
            finalized_slot: finality_height,
            blockhash: finality_block_hash,
            bank_hash,
            transaction_status_root: receipt_or_message_root,
            message_proof_hash,
            transaction_signature,
            emitter_program_id,
            finality_context,
            vote_proof: solana_vote_proof(
                source_domain,
                finality_height,
                finality_block_hash,
                bank_hash,
                receipt_or_message_root,
                message_proof_hash,
                finality_context_hash,
            ),
        },
    );
    if let (
        Some(material),
        iroha_sccp::SccpSourceAdapterProofV1::SolanaFinalizedTransaction(adapter),
    ) = (source_material, &mut adapter_proof)
    {
        if let Some(proof) =
            iroha_sccp::build_sccp_solana_accounts_lt_hash_verification_proof(adapter, material)
        {
            adapter.vote_proof.accounts_lt_hash_proof = proof;
        }
        if let Some(deployment) = source_deployment
            && iroha_sccp::sccp_solana_full_light_client_gate_hash_from_deployment_v1(
                material, deployment,
            )
            .is_some()
        {
            adapter.vote_proof.tower_replay_verification_proof =
                iroha_sccp::build_sccp_solana_tower_replay_verification_proof(
                    adapter, material, deployment,
                )
                .expect("build Solana Tower replay verification proof");
            adapter
                .vote_proof
                .full_accountsdb_lattice_verification_proof =
                iroha_sccp::build_sccp_solana_full_accountsdb_lattice_verification_proof(
                    adapter, material, deployment,
                )
                .expect("build Solana full AccountsDB lattice verification proof");
            adapter.vote_proof.bank_fork_choice_verification_proof =
                iroha_sccp::build_sccp_solana_bank_fork_choice_verification_proof(
                    adapter, material, deployment,
                )
                .expect("build Solana bank/fork-choice verification proof");
        }
    }
    let adapter_transcript_hash = iroha_sccp::sccp_source_adapter_transcript_hash(
        source_domain,
        target_domain,
        source_proof_plan,
        finality_model,
        finality_height,
        finality_block_hash,
        receipt_or_message_root,
        source_event_digest,
        &adapter_proof,
    );
    let envelope_context = iroha_sccp::SccpSourceChainProofEnvelopeV1 {
        version: 1,
        source_domain,
        target_domain,
        source_chain: source_chain.clone(),
        source_proof_plan,
        finality_model,
        message_id: commitment.message_id,
        payload_hash: commitment.payload_hash,
        source_event_digest,
        commitment_root,
        finality_height,
        finality_block_hash,
        finalized_header_hash,
        receipt_or_message_root,
        consensus_proof: Vec::new(),
        message_inclusion_proof: Vec::new(),
        inclusion_branch: inclusion_branch.clone(),
    };
    let adapter_verification_proof =
        if let (Some(material), Some(deployment)) = (source_material, source_deployment) {
            iroha_sccp::build_sccp_source_adapter_verification_proof_with_material_and_deployment(
                &envelope_context,
                &adapter_proof,
                adapter_transcript_hash,
                material,
                deployment,
            )
        } else if let Some(material) = source_material {
            iroha_sccp::build_sccp_source_adapter_verification_proof_with_material(
                &envelope_context,
                &adapter_proof,
                adapter_transcript_hash,
                material,
            )
        } else {
            iroha_sccp::build_sccp_source_adapter_verification_proof(
                &envelope_context,
                &adapter_proof,
                adapter_transcript_hash,
            )
        }
        .expect("build source adapter verification proof");
    let verifier_evidence =
        if let (Some(material), Some(deployment)) = (source_material, source_deployment) {
            iroha_sccp::build_sccp_source_verifier_evidence_with_material_and_deployment(
                &envelope_context,
                &adapter_proof,
                adapter_transcript_hash,
                material,
                deployment,
            )
        } else if let Some(material) = source_material {
            iroha_sccp::build_sccp_source_verifier_evidence_with_material(
                &envelope_context,
                &adapter_proof,
                adapter_transcript_hash,
                material,
            )
        } else {
            iroha_sccp::build_sccp_source_verifier_evidence(
                &envelope_context,
                &adapter_proof,
                adapter_transcript_hash,
            )
        }
        .expect("build source verifier evidence");
    let consensus_proof = norito::to_bytes(&iroha_sccp::SccpSourceConsensusProofV1 {
        version: 1,
        source_domain,
        source_chain: source_chain.clone(),
        source_proof_plan,
        finality_model,
        finality_height,
        finality_block_hash,
        receipt_or_message_root,
        finalized_header_hash,
        adapter_proof,
        adapter_transcript_hash,
        verifier_evidence,
        adapter_verification_proof,
    })
    .expect("encode source consensus proof");
    let message_inclusion_proof =
        norito::to_bytes(&iroha_sccp::SccpSourceMessageInclusionProofV1 {
            version: 1,
            source_domain,
            target_domain,
            message_id: commitment.message_id,
            payload_hash: commitment.payload_hash,
            source_event_digest,
            source_event_leaf_hash,
            receipt_or_message_root,
            leaf_index: 0,
        })
        .expect("encode source inclusion proof");
    let finality_proof = norito::to_bytes(&iroha_sccp::SccpSourceChainProofEnvelopeV1 {
        version: 1,
        source_domain,
        target_domain,
        source_chain,
        source_proof_plan,
        finality_model,
        message_id: commitment.message_id,
        payload_hash: commitment.payload_hash,
        source_event_digest,
        commitment_root,
        finality_height,
        finality_block_hash,
        finalized_header_hash,
        receipt_or_message_root,
        consensus_proof,
        message_inclusion_proof,
        inclusion_branch,
    })
    .expect("encode source-chain proof envelope");
    let bundle = iroha_sccp::NexusSccpMessageProofV1 {
        version: 1,
        commitment_root,
        commitment,
        merkle_proof,
        payload,
        finality_proof,
    };
    assert!(iroha_sccp::verified_sccp_message_nexus_finality_proof(&bundle).is_none());
    if let Some(material) = source_material {
        assert!(!iroha_sccp::verify_message_bundle_structure(&bundle));
        let source_proof =
            iroha_sccp::decode_sccp_source_chain_proof_envelope(&bundle.finality_proof)
                .expect("decode source proof");
        let material_bound_bundle_is_structural =
            iroha_sccp::verify_message_bundle_structure_with_source_verifier_material(
                &bundle, material,
            );
        let deployment_bound_bundle_is_structural = source_deployment.is_some_and(|deployment| {
            iroha_sccp::verify_message_bundle_structure_with_source_verifier_material_and_deployment(
                &bundle, material, deployment,
            )
        });
        if source_deployment.is_some() {
            assert!(
                material_bound_bundle_is_structural,
                "deployment-bound source proofs should remain structurally inspectable with source material",
            );
            assert!(
                deployment_bound_bundle_is_structural,
                "deployment-bound bundle should verify structurally: deployment_matches_material={}, adapter_ready={}, proof_matches_deployment={}",
                source_deployment.is_some_and(|deployment| {
                    iroha_sccp::sccp_source_adapter_engine_deployment_matches_material(
                        material, deployment,
                    )
                }),
                source_deployment.is_some_and(|deployment| {
                    iroha_sccp::sccp_source_adapter_ready_with_material_and_deployment_for_domain(
                        source_domain,
                        material,
                        deployment,
                    )
                }),
                source_deployment.is_some_and(|deployment| {
                    iroha_sccp::sccp_source_chain_proof_matches_adapter_deployment(
                        &source_proof,
                        deployment,
                    )
                }),
            );
        } else {
            assert!(material_bound_bundle_is_structural);
        }
        assert_eq!(source_proof.source_domain, source_domain);
        assert_eq!(source_proof.target_domain, target_domain);
        assert_eq!(source_proof.message_id, bundle.commitment.message_id);
        assert_eq!(source_proof.payload_hash, bundle.commitment.payload_hash);
        assert_eq!(source_proof.commitment_root, bundle.commitment_root);
        if let Some(deployment) = source_deployment {
            assert!(
                iroha_sccp::verify_sccp_source_chain_proof_envelope_structure_with_material_and_deployment(
                    &source_proof,
                    material,
                    deployment,
                )
            );
            let deployment_ready =
                iroha_sccp::sccp_source_adapter_ready_with_material_and_deployment_for_domain(
                    source_domain,
                    material,
                    deployment,
                );
            assert_eq!(
                iroha_sccp::verify_sccp_source_chain_proof_envelope_production_with_material_and_deployment(
                    &source_proof,
                    material,
                    deployment,
                ),
                deployment_ready,
                "source envelope production helper must track deployment-backed readiness"
            );
            assert_eq!(
                iroha_sccp::verified_sccp_message_source_chain_proof_envelope_for_production_with_material_and_deployment(
                    &bundle,
                    material,
                    deployment,
                )
                .is_some(),
                deployment_ready,
                "source bundle production helper must track deployment-backed readiness"
            );
            assert_eq!(
                iroha_sccp::sccp_source_chain_proof_adapter_verifier_commitment(&source_proof),
                Some(deployment.adapter_verifier_vk_hash),
            );
        } else {
            assert!(
                iroha_sccp::verify_sccp_source_chain_proof_envelope_structure_with_material(
                    &source_proof,
                    material,
                )
            );
            let source_adapter_ready =
                iroha_sccp::sccp_source_adapter_ready_with_material_for_domain(
                    source_domain,
                    material,
                );
            assert_eq!(
                iroha_sccp::verify_sccp_source_chain_proof_envelope_production_with_material(
                    &source_proof,
                    material,
                ),
                source_adapter_ready,
                "source envelope production helper must track the source-adapter readiness gate"
            );
            assert_eq!(
                iroha_sccp::verified_sccp_message_source_chain_proof_envelope_for_production_with_material(
                    &bundle,
                    material,
                )
                .is_some(),
                source_adapter_ready,
                "source bundle production helper must track the source-adapter readiness gate"
            );
        }
    } else {
        assert!(iroha_sccp::verify_message_bundle_structure(&bundle));
        assert!(iroha_sccp::verified_sccp_message_source_chain_proof_envelope(&bundle).is_some());
    }

    let artifact = if let (Some(material), Some(deployment)) = (source_material, source_deployment)
    {
        iroha_sccp::build_nexus_sccp_message_transparent_proof_with_source_verifier_material_and_deployment_allow_unready(
            &bundle,
            material,
            deployment,
            true,
        )
    } else if let Some(material) = source_material {
        iroha_sccp::build_nexus_sccp_message_transparent_proof_with_source_verifier_material_allow_unready(
            &bundle,
            material,
            true,
        )
    } else {
        iroha_sccp::build_nexus_sccp_message_transparent_proof_allow_unready(&bundle, true)
    }
    .expect("build SOL SCCP transparent proof");
    let manifest_hash = iroha_sccp::sccp_bridge_manifest_hash_for_seed(&artifact.manifest_seed);
    let backend = artifact.message_backend.clone();
    let proof_bytes = norito::to_bytes(&artifact).expect("encode SCCP transparent artifact");
    BridgeProof {
        range: BridgeProofRange {
            start_height: finality_height,
            end_height: finality_height,
        },
        manifest_hash,
        payload: BridgeProofPayload::TransparentZk(BridgeTransparentProof {
            proof: ProofBox::new(backend, proof_bytes),
            recursion_depth: None,
        }),
        pinned: false,
    }
}

fn make_sccp_ton_to_sora_unverified_message_bridge_proof(nonce: u64) -> BridgeProof {
    let mut proof = make_sccp_sol_to_sora_message_bridge_proof(nonce);
    let BridgeProofPayload::TransparentZk(transparent) = &mut proof.payload else {
        panic!("expected transparent SCCP proof");
    };
    let mut artifact =
        iroha_sccp::decode_nexus_sccp_message_transparent_proof(&transparent.proof.bytes)
            .expect("decode SCCP transparent artifact");
    let sender = format!("0:{}", "22".repeat(32));
    let payload = iroha_sccp::SccpPayloadV1::Transfer(iroha_sccp::TransferPayloadV1 {
        version: 1,
        source_domain: iroha_sccp::SCCP_DOMAIN_TON,
        dest_domain: iroha_sccp::SCCP_DOMAIN_SORA,
        nonce,
        asset_home_domain: iroha_sccp::SCCP_DOMAIN_TON,
        asset_id_codec: iroha_sccp::SCCP_CODEC_TEXT_UTF8,
        asset_id: b"wton#ton".to_vec(),
        amount: 7,
        sender_codec: iroha_sccp::SCCP_CODEC_TON_RAW,
        sender: sender.into_bytes(),
        recipient_codec: iroha_sccp::SCCP_CODEC_TEXT_UTF8,
        recipient: b"alice@universal".to_vec(),
        route_id_codec: iroha_sccp::SCCP_CODEC_TEXT_UTF8,
        route_id: b"ton:sora:wton".to_vec(),
    });
    let payload_hash =
        iroha_sccp::payload_hash(&iroha_sccp::canonical_sccp_payload_bytes(&payload));
    let commitment = iroha_sccp::SccpHubCommitmentV1 {
        version: 1,
        kind: iroha_sccp::sccp_message_kind(&payload),
        target_domain: iroha_sccp::sccp_message_target_domain(&payload),
        message_id: iroha_sccp::sccp_message_id(&payload),
        payload_hash,
    };
    let merkle_proof = iroha_sccp::SccpMerkleProofV1 { steps: Vec::new() };
    let commitment_root = iroha_sccp::merkle_root_from_commitment(&commitment, &merkle_proof);

    artifact.public_inputs.message_id = commitment.message_id;
    artifact.public_inputs.payload_hash = commitment.payload_hash;
    artifact.public_inputs.target_domain = commitment.target_domain;
    artifact.public_inputs.commitment_root = commitment_root;
    artifact.bundle.payload = payload;
    artifact.bundle.commitment = commitment;
    artifact.bundle.merkle_proof = merkle_proof;
    artifact.bundle.commitment_root = commitment_root;
    proof.manifest_hash = iroha_sccp::sccp_bridge_manifest_hash_for_seed(&artifact.manifest_seed);
    transparent.proof.bytes = norito::to_bytes(&artifact).expect("encode TON SCCP skeleton");
    proof
}

fn make_sccp_tron_to_sora_unverified_message_bridge_proof(nonce: u64) -> BridgeProof {
    let mut proof = make_sccp_sol_to_sora_message_bridge_proof(nonce);
    let BridgeProofPayload::TransparentZk(transparent) = &mut proof.payload else {
        panic!("expected transparent SCCP proof");
    };
    let mut artifact =
        iroha_sccp::decode_nexus_sccp_message_transparent_proof(&transparent.proof.bytes)
            .expect("decode SCCP transparent artifact");
    let payload = iroha_sccp::SccpPayloadV1::Transfer(iroha_sccp::TransferPayloadV1 {
        version: 1,
        source_domain: iroha_sccp::SCCP_DOMAIN_TRON,
        dest_domain: iroha_sccp::SCCP_DOMAIN_SORA,
        nonce,
        asset_home_domain: iroha_sccp::SCCP_DOMAIN_TRON,
        asset_id_codec: iroha_sccp::SCCP_CODEC_TEXT_UTF8,
        asset_id: b"wtrx#tron".to_vec(),
        amount: 7,
        sender_codec: iroha_sccp::SCCP_CODEC_TRON_BASE58CHECK,
        sender: b"TJRabPrwbZy45sbavfcjinPJC18kjpRTv8".to_vec(),
        recipient_codec: iroha_sccp::SCCP_CODEC_TEXT_UTF8,
        recipient: b"alice@universal".to_vec(),
        route_id_codec: iroha_sccp::SCCP_CODEC_TEXT_UTF8,
        route_id: b"tron:sora:wtrx".to_vec(),
    });
    let payload_hash =
        iroha_sccp::payload_hash(&iroha_sccp::canonical_sccp_payload_bytes(&payload));
    let commitment = iroha_sccp::SccpHubCommitmentV1 {
        version: 1,
        kind: iroha_sccp::sccp_message_kind(&payload),
        target_domain: iroha_sccp::sccp_message_target_domain(&payload),
        message_id: iroha_sccp::sccp_message_id(&payload),
        payload_hash,
    };
    let merkle_proof = iroha_sccp::SccpMerkleProofV1 { steps: Vec::new() };
    let commitment_root = iroha_sccp::merkle_root_from_commitment(&commitment, &merkle_proof);

    artifact.public_inputs.message_id = commitment.message_id;
    artifact.public_inputs.payload_hash = commitment.payload_hash;
    artifact.public_inputs.target_domain = commitment.target_domain;
    artifact.public_inputs.commitment_root = commitment_root;
    artifact.bundle.payload = payload;
    artifact.bundle.commitment = commitment;
    artifact.bundle.merkle_proof = merkle_proof;
    artifact.bundle.commitment_root = commitment_root;
    proof.manifest_hash = iroha_sccp::sccp_bridge_manifest_hash_for_seed(&artifact.manifest_seed);
    transparent.proof.bytes = norito::to_bytes(&artifact).expect("encode TRON SCCP skeleton");
    proof
}

#[test]
fn submit_bridge_proof_records_metadata() {
    let world = iroha_core::state::World::new();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let telemetry = StateTelemetry::default();
    let state = State::with_telemetry(world, kura, query_handle, telemetry);

    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    let exec = Executor::default();

    let proof = make_ics_proof(0x11, (1, 1), false);
    let expected_id = bridge_proof_id(&proof);
    let encoded_len = u32::try_from(norito::to_bytes(&proof).expect("encode proof").len())
        .expect("bridge proof length fits in u32");

    let submit: InstructionBox =
        iroha_data_model::isi::bridge::SubmitBridgeProof::new(proof).into();
    exec.execute_instruction(&mut stx, &ALICE_ID.clone(), submit)
        .expect("bridge proof accepted");

    let rec = stx
        .world
        .proofs()
        .get(&expected_id)
        .expect("proof recorded");
    assert_eq!(rec.status, ProofStatus::Verified);
    let bridge = rec.bridge.as_ref().expect("bridge metadata stored");
    assert_eq!(bridge.commitment, expected_id.proof_hash);
    assert_eq!(bridge.size_bytes, encoded_len);
    assert_eq!(bridge.proof.range.start_height, 1);
    assert_eq!(bridge.proof.range.end_height, 1);
}

#[test]
fn bridge_retention_prunes_oldest_unpinned() {
    let world = iroha_core::state::World::new();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let telemetry = StateTelemetry::default();
    let mut state = State::with_telemetry(world, kura, query_handle, telemetry);

    state.zk.proof_history_cap = 1;
    state.zk.proof_retention_grace_blocks = 0;
    state.zk.proof_prune_batch = 10;

    let exec = Executor::default();

    let header1 =
        iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block1 = state.block(header1);
    let mut stx1 = block1.transaction();
    let proof1 = make_ics_proof(0x21, (1, 1), false);
    let id1 = bridge_proof_id(&proof1);
    let submit1: InstructionBox =
        iroha_data_model::isi::bridge::SubmitBridgeProof::new(proof1).into();
    exec.execute_instruction(&mut stx1, &ALICE_ID.clone(), submit1)
        .expect("first proof accepted");
    stx1.apply();
    block1
        .commit()
        .expect("commit first bridge-proof block snapshot");

    let header2 =
        iroha_data_model::block::BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
    let mut block2 = state.block(header2);
    let mut stx2 = block2.transaction();
    let proof2 = make_ics_proof(0x33, (2, 2), false);
    let id2 = bridge_proof_id(&proof2);
    let submit2: InstructionBox =
        iroha_data_model::isi::bridge::SubmitBridgeProof::new(proof2).into();
    exec.execute_instruction(&mut stx2, &ALICE_ID.clone(), submit2)
        .expect("second proof accepted");

    assert!(stx2.world.proofs().get(&id2).is_some());
    assert!(
        stx2.world.proofs().get(&id1).is_none(),
        "older unpinned proof should be pruned when cap is hit"
    );
}

#[test]
fn bridge_range_length_cap_enforced() {
    let world = iroha_core::state::World::new();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let telemetry = StateTelemetry::default();
    let mut state = State::with_telemetry(world, kura, query_handle, telemetry);

    state.zk.bridge_proof_max_range_len = 2;

    let exec = Executor::default();
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let proof = make_ics_proof(0x44, (5, 10), false);
    let submit: InstructionBox =
        iroha_data_model::isi::bridge::SubmitBridgeProof::new(proof).into();
    let err = exec
        .execute_instruction(&mut stx, &ALICE_ID.clone(), submit)
        .expect_err("range cap should reject long bridge proofs");
    assert!(
        format!("{err:?}").contains("range too large"),
        "unexpected error: {err:?}"
    );
}

#[test]
fn bridge_height_window_respected() {
    let world = iroha_core::state::World::new();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let telemetry = StateTelemetry::default();
    let mut state = State::with_telemetry(world, kura, query_handle, telemetry);

    let exec = Executor::default();

    state.zk.bridge_proof_max_future_drift_blocks = 1;
    let header_future =
        iroha_data_model::block::BlockHeader::new(nonzero!(5_u64), None, None, None, 0, 0);
    let mut block_future = state.block(header_future);
    let mut stx_future = block_future.transaction();
    let future_proof = make_ics_proof(0x55, (7, 7), false);
    let submit_future: InstructionBox =
        iroha_data_model::isi::bridge::SubmitBridgeProof::new(future_proof).into();
    let err = exec
        .execute_instruction(&mut stx_future, &ALICE_ID.clone(), submit_future)
        .expect_err("future drift guard should reject proof ahead of window");
    assert!(
        format!("{err:?}").contains("future window"),
        "unexpected error for future drift: {err:?}"
    );
    drop(stx_future);
    drop(block_future);

    state.zk.bridge_proof_max_future_drift_blocks = 10;
    state.zk.bridge_proof_max_past_age_blocks = 2;
    let header_past =
        iroha_data_model::block::BlockHeader::new(nonzero!(10_u64), None, None, None, 0, 0);
    let mut block_past = state.block(header_past);
    let mut stx_past = block_past.transaction();
    let stale_proof = make_ics_proof(0x66, (1, 7), false);
    let submit_past: InstructionBox =
        iroha_data_model::isi::bridge::SubmitBridgeProof::new(stale_proof).into();
    let err = exec
        .execute_instruction(&mut stx_past, &ALICE_ID.clone(), submit_past)
        .expect_err("past window should reject stale proof");
    assert!(
        format!("{err:?}").contains("past window"),
        "unexpected error for stale proof: {err:?}"
    );
}

#[test]
fn generic_ics_proof_rejects_reserved_sccp_manifest_hash() {
    let world = iroha_core::state::World::new();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let telemetry = StateTelemetry::default();
    let state = State::with_telemetry(world, kura, query_handle, telemetry);

    let exec = Executor::default();
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let mut proof = make_ics_proof(0x67, (1, 1), false);
    proof.manifest_hash = iroha_sccp::sccp_burn_bridge_manifest_hash_v1();
    let submit: InstructionBox =
        iroha_data_model::isi::bridge::SubmitBridgeProof::new(proof).into();
    let err = exec
        .execute_instruction(&mut stx, &ALICE_ID.clone(), submit)
        .expect_err("generic ICS SCCP manifest bypass must be rejected");
    assert!(
        format!("{err:?}").contains("typed SCCP bridge proof backends"),
        "unexpected error for reserved manifest bypass: {err:?}"
    );
}

#[test]
fn bridge_overlapping_ranges_are_rejected() {
    let world = iroha_core::state::World::new();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let telemetry = StateTelemetry::default();
    let state = State::with_telemetry(world, kura, query_handle, telemetry);

    let exec = Executor::default();

    let header1 =
        iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block1 = state.block(header1);
    let mut stx1 = block1.transaction();
    let proof1 = make_ics_proof(0x71, (10, 12), false);
    let submit1: InstructionBox =
        iroha_data_model::isi::bridge::SubmitBridgeProof::new(proof1).into();
    exec.execute_instruction(&mut stx1, &ALICE_ID.clone(), submit1)
        .expect("first proof accepted");
    stx1.apply();
    block1
        .commit()
        .expect("commit first bridge-proof block snapshot");

    let header2 =
        iroha_data_model::block::BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
    let mut block2 = state.block(header2);
    let mut stx2 = block2.transaction();
    let proof2 = make_ics_proof(0x72, (11, 13), false);
    let submit2: InstructionBox =
        iroha_data_model::isi::bridge::SubmitBridgeProof::new(proof2).into();
    let err = exec
        .execute_instruction(&mut stx2, &ALICE_ID.clone(), submit2)
        .expect_err("overlapping bridge proof must be rejected");
    assert!(
        format!("{err:?}").contains("overlaps existing proof"),
        "unexpected error for overlap: {err:?}"
    );
}

#[test]
fn re_submitting_identical_bridge_proof_is_idempotent() {
    let world = iroha_core::state::World::new();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let telemetry = StateTelemetry::default();
    let state = State::with_telemetry(world, kura, query_handle, telemetry);

    let exec = Executor::default();
    let proof = make_ics_proof(0x73, (21, 21), false);
    let proof_id = bridge_proof_id(&proof);

    let header1 =
        iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block1 = state.block(header1);
    let mut stx1 = block1.transaction();
    let submit1: InstructionBox =
        iroha_data_model::isi::bridge::SubmitBridgeProof::new(proof.clone()).into();
    exec.execute_instruction(&mut stx1, &ALICE_ID.clone(), submit1)
        .expect("first proof accepted");
    stx1.apply();
    block1
        .commit()
        .expect("commit first bridge-proof block snapshot");

    let header2 =
        iroha_data_model::block::BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
    let mut block2 = state.block(header2);
    let mut stx2 = block2.transaction();
    let submit2: InstructionBox =
        iroha_data_model::isi::bridge::SubmitBridgeProof::new(proof).into();
    exec.execute_instruction(&mut stx2, &ALICE_ID.clone(), submit2)
        .expect("identical proof should be a no-op");

    let rec = stx2
        .world
        .proofs()
        .get(&proof_id)
        .expect("original proof remains recorded");
    assert_eq!(rec.status, ProofStatus::Verified);
    let bridge = rec.bridge.as_ref().expect("bridge metadata stored");
    assert_eq!(bridge.commitment, proof_id.proof_hash);
    assert_eq!(bridge.proof.range.start_height, 21);
    assert_eq!(bridge.proof.range.end_height, 21);
}

#[test]
fn malformed_sccp_transparent_bridge_proof_is_rejected() {
    let world = iroha_core::state::World::new();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let telemetry = StateTelemetry::default();
    let state = State::with_telemetry(world, kura, query_handle, telemetry);

    let exec = Executor::default();
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let proof = BridgeProof {
        range: BridgeProofRange {
            start_height: 1,
            end_height: 1,
        },
        manifest_hash: [0x44; 32],
        payload: BridgeProofPayload::TransparentZk(BridgeTransparentProof {
            proof: ProofBox::new("sccp/stark-fri-v1/eth".into(), vec![0xAA]),
            recursion_depth: None,
        }),
        pinned: false,
    };
    let submit: InstructionBox =
        iroha_data_model::isi::bridge::SubmitBridgeProof::new(proof).into();
    let err = exec
        .execute_instruction(&mut stx, &ALICE_ID.clone(), submit)
        .expect_err("malformed SCCP artifact must be rejected");
    assert!(
        format!("{err:?}").contains("typed message artifacts"),
        "unexpected error: {err:?}"
    );
}

#[test]
fn submit_sccp_inbound_message_rejects_unready_lane_even_if_config_allows() {
    let world = iroha_core::state::World::new();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let telemetry = StateTelemetry::default();
    let mut state = State::with_telemetry(world, kura, query_handle, telemetry);
    state.zk.sccp_allow_unready_transparent_proofs = true;
    state.zk.max_proof_size_bytes = 4 * 1024 * 1024;

    let exec = Executor::default();
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(3_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let proof = make_sccp_sol_to_sora_message_bridge_proof(99);
    let submit: InstructionBox =
        iroha_data_model::isi::bridge::SubmitBridgeProof::new(proof).into();
    let err = exec
        .execute_instruction(&mut stx, &ALICE_ID.clone(), submit)
        .expect_err("unready SCCP lanes must not be accepted on-chain");
    assert!(
        format!("{err:?}").contains("structural verification"),
        "unexpected error: {err:?}"
    );
}

#[test]
fn submit_sccp_inbound_message_with_audited_sol_source_adapter_reaches_all_lanes_gate() {
    let world = iroha_core::state::World::new();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let telemetry = StateTelemetry::default();
    let mut state = State::with_telemetry(world, kura, query_handle, telemetry);
    state.zk.max_proof_size_bytes = SCCP_AUDITED_SOLANA_PROOF_MAX_BYTES;
    let material = configured_sol_source_verifier_material();
    let deployment = configured_sol_source_adapter_engine_deployment(&material);
    let audited_deployment = audited_sol_source_adapter_engine_deployment(&deployment);
    assert!(
        iroha_sccp::sccp_source_adapter_ready_with_material_and_deployment_for_domain(
            iroha_sccp::SCCP_DOMAIN_SOL,
            &material,
            &audited_deployment,
        ),
        "audited Solana full-light-client deployment evidence should open the source-adapter gate"
    );
    state
        .zk
        .sccp_source_verifier_materials
        .push(actual_source_verifier_material(&material));
    let mut deployment_config = actual_source_adapter_engine_deployment(&deployment);
    attach_solana_full_light_client_audit(&mut deployment_config, &material, &deployment);
    state
        .zk
        .sccp_source_adapter_engine_deployments
        .push(deployment_config);
    let destination_rollout = configured_sol_destination_rollout();
    let route_allowlist =
        configured_sol_route_allowlist(&material, &audited_deployment, &destination_rollout);
    state
        .zk
        .sccp_destination_rollouts
        .push(actual_destination_rollout(&destination_rollout));
    state
        .zk
        .sccp_route_allowlists
        .push(actual_route_allowlist(&route_allowlist));

    let exec = Executor::default();
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(3_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let proof = make_sccp_sol_to_sora_message_bridge_proof_with_material_and_deployment(
        99,
        Some(&material),
        Some(&audited_deployment),
    );
    let proof_id = bridge_proof_id(&proof);
    let submit: InstructionBox =
        iroha_data_model::isi::bridge::SubmitBridgeProof::new(proof.clone()).into();
    let err = exec
        .execute_instruction(&mut stx, &ALICE_ID.clone(), submit)
        .expect_err("single-lane Solana material must still wait for all-lanes launch readiness");
    assert!(
        format!("{err:?}").contains("all-lanes launch policy"),
        "unexpected error: {err:?}"
    );
    assert!(stx.world.proofs().get(&proof_id).is_none());
}

#[test]
fn submit_sccp_inbound_message_with_audited_ton_source_adapter_reaches_all_lanes_gate() {
    let world = iroha_core::state::World::new();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let telemetry = StateTelemetry::default();
    let mut state = State::with_telemetry(world, kura, query_handle, telemetry);
    state.zk.max_proof_size_bytes = 4 * 1024 * 1024;
    let material = configured_ton_source_verifier_material();
    let deployment = configured_ton_source_adapter_engine_deployment(&material);
    let audited_deployment = audited_ton_source_adapter_engine_deployment(&deployment);
    assert!(
        iroha_sccp::sccp_source_adapter_ready_with_material_and_deployment_for_domain(
            iroha_sccp::SCCP_DOMAIN_TON,
            &material,
            &audited_deployment,
        ),
        "audited TON full-light-client deployment evidence should open the source-adapter gate"
    );
    state
        .zk
        .sccp_source_verifier_materials
        .push(actual_source_verifier_material(&material));
    let mut deployment_config = actual_source_adapter_engine_deployment(&deployment);
    attach_ton_full_light_client_audit(&mut deployment_config, &material, &deployment);
    state
        .zk
        .sccp_source_adapter_engine_deployments
        .push(deployment_config);
    let destination_rollout = configured_ton_destination_rollout();
    let route_allowlist =
        configured_ton_route_allowlist(&material, &audited_deployment, &destination_rollout);
    state
        .zk
        .sccp_destination_rollouts
        .push(actual_destination_rollout(&destination_rollout));
    state
        .zk
        .sccp_route_allowlists
        .push(actual_route_allowlist(&route_allowlist));

    let exec = Executor::default();
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(3_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let proof = make_sccp_ton_to_sora_unverified_message_bridge_proof(120);
    let proof_id = bridge_proof_id(&proof);
    let submit: InstructionBox =
        iroha_data_model::isi::bridge::SubmitBridgeProof::new(proof.clone()).into();
    let err = exec
        .execute_instruction(&mut stx, &ALICE_ID.clone(), submit)
        .expect_err("single-lane TON material must still wait for all-lanes launch readiness");
    assert!(
        format!("{err:?}").contains("all-lanes launch policy"),
        "unexpected error: {err:?}"
    );
    assert!(stx.world.proofs().get(&proof_id).is_none());
}

#[test]
fn submit_sccp_inbound_message_with_configured_tron_source_adapter_reaches_all_lanes_gate() {
    let world = iroha_core::state::World::new();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let telemetry = StateTelemetry::default();
    let mut state = State::with_telemetry(world, kura, query_handle, telemetry);
    state.zk.max_proof_size_bytes = 4 * 1024 * 1024;
    let material = configured_tron_source_verifier_material();
    let deployment = configured_tron_source_adapter_engine_deployment(&material);
    assert!(
        iroha_sccp::sccp_source_adapter_ready_with_material_and_deployment_for_domain(
            iroha_sccp::SCCP_DOMAIN_TRON,
            &material,
            &deployment,
        ),
        "configured TRON source-adapter deployment evidence should open the source-adapter gate"
    );
    state
        .zk
        .sccp_source_verifier_materials
        .push(actual_source_verifier_material(&material));
    let mut deployment_config = actual_source_adapter_engine_deployment(&deployment);
    attach_tron_dpos_source_gate(&mut deployment_config, &material, &deployment);
    state
        .zk
        .sccp_source_adapter_engine_deployments
        .push(deployment_config);
    let destination_rollout = configured_tron_destination_rollout(&material);
    let route_allowlist =
        configured_tron_route_allowlist(&material, &deployment, &destination_rollout);
    state
        .zk
        .sccp_destination_rollouts
        .push(actual_destination_rollout(&destination_rollout));
    state
        .zk
        .sccp_route_allowlists
        .push(actual_route_allowlist(&route_allowlist));

    let exec = Executor::default();
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(3_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let proof = make_sccp_tron_to_sora_unverified_message_bridge_proof(121);
    let proof_id = bridge_proof_id(&proof);
    let submit: InstructionBox =
        iroha_data_model::isi::bridge::SubmitBridgeProof::new(proof.clone()).into();
    let err = exec
        .execute_instruction(&mut stx, &ALICE_ID.clone(), submit)
        .expect_err("single-lane TRON material must still wait for all-lanes launch readiness");
    assert!(
        format!("{err:?}").contains("all-lanes launch policy"),
        "unexpected error: {err:?}"
    );
    assert!(stx.world.proofs().get(&proof_id).is_none());
}

#[test]
fn submit_sccp_inbound_message_rejects_mismatched_tron_dpos_source_gate_hash() {
    let world = iroha_core::state::World::new();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let telemetry = StateTelemetry::default();
    let mut state = State::with_telemetry(world, kura, query_handle, telemetry);
    state.zk.max_proof_size_bytes = 4 * 1024 * 1024;
    let material = configured_tron_source_verifier_material();
    let deployment = configured_tron_source_adapter_engine_deployment(&material);
    state
        .zk
        .sccp_source_verifier_materials
        .push(actual_source_verifier_material(&material));
    let mut deployment_config = actual_source_adapter_engine_deployment(&deployment);
    attach_tron_dpos_source_gate(&mut deployment_config, &material, &deployment);
    deployment_config.tron_dpos_source_gate_hash = hex::encode([0xee; 32]);
    state
        .zk
        .sccp_source_adapter_engine_deployments
        .push(deployment_config);

    let exec = Executor::default();
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(3_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let proof = make_sccp_tron_to_sora_unverified_message_bridge_proof(122);
    let proof_id = bridge_proof_id(&proof);
    let submit: InstructionBox =
        iroha_data_model::isi::bridge::SubmitBridgeProof::new(proof.clone()).into();
    let err = exec
        .execute_instruction(&mut stx, &ALICE_ID.clone(), submit)
        .expect_err("mismatched TRON DPoS source gate hash must fail closed");
    assert!(
        format!("{err:?}").contains("TRON DPoS source gate hash"),
        "unexpected error: {err:?}"
    );
    assert!(stx.world.proofs().get(&proof_id).is_none());
}

#[test]
fn submit_sccp_inbound_message_rejects_mismatched_sol_full_light_client_audit_hash() {
    let world = iroha_core::state::World::new();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let telemetry = StateTelemetry::default();
    let mut state = State::with_telemetry(world, kura, query_handle, telemetry);
    state.zk.max_proof_size_bytes = 4 * 1024 * 1024;
    let material = configured_sol_source_verifier_material();
    let deployment = configured_sol_source_adapter_engine_deployment(&material);
    state
        .zk
        .sccp_source_verifier_materials
        .push(actual_source_verifier_material(&material));
    let mut deployment_config = actual_source_adapter_engine_deployment(&deployment);
    attach_solana_full_light_client_audit(&mut deployment_config, &material, &deployment);
    deployment_config.solana_full_light_client_gate_hash = hex::encode([0xee; 32]);
    state
        .zk
        .sccp_source_adapter_engine_deployments
        .push(deployment_config);

    let exec = Executor::default();
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(3_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let proof = make_sccp_sol_to_sora_message_bridge_proof_with_material_and_deployment(
        99,
        Some(&material),
        Some(&deployment),
    );
    let proof_id = bridge_proof_id(&proof);
    let submit: InstructionBox =
        iroha_data_model::isi::bridge::SubmitBridgeProof::new(proof.clone()).into();
    let err = exec
        .execute_instruction(&mut stx, &ALICE_ID.clone(), submit)
        .expect_err("mismatched Solana full-light-client audit hash must fail closed");
    assert!(
        format!("{err:?}").contains("full-light-client audit gate hash"),
        "unexpected error: {err:?}"
    );
    assert!(stx.world.proofs().get(&proof_id).is_none());
}

#[test]
fn submit_sccp_inbound_message_rejects_mismatched_ton_full_light_client_audit_hash() {
    let world = iroha_core::state::World::new();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let telemetry = StateTelemetry::default();
    let mut state = State::with_telemetry(world, kura, query_handle, telemetry);
    state.zk.max_proof_size_bytes = 4 * 1024 * 1024;
    let material = configured_ton_source_verifier_material();
    let deployment = configured_ton_source_adapter_engine_deployment(&material);
    state
        .zk
        .sccp_source_verifier_materials
        .push(actual_source_verifier_material(&material));
    let mut deployment_config = actual_source_adapter_engine_deployment(&deployment);
    attach_ton_full_light_client_audit(&mut deployment_config, &material, &deployment);
    deployment_config.ton_full_light_client_gate_hash = hex::encode([0xee; 32]);
    state
        .zk
        .sccp_source_adapter_engine_deployments
        .push(deployment_config);

    let exec = Executor::default();
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(3_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let proof = make_sccp_ton_to_sora_unverified_message_bridge_proof(121);
    let proof_id = bridge_proof_id(&proof);
    let submit: InstructionBox =
        iroha_data_model::isi::bridge::SubmitBridgeProof::new(proof.clone()).into();
    let err = exec
        .execute_instruction(&mut stx, &ALICE_ID.clone(), submit)
        .expect_err("mismatched TON full-light-client audit hash must fail closed");
    assert!(
        format!("{err:?}").contains("TON full-light-client audit gate hash"),
        "unexpected error: {err:?}"
    );
    assert!(stx.world.proofs().get(&proof_id).is_none());
}

#[test]
fn submit_sccp_inbound_message_rejects_partial_ton_full_light_client_audit_hash() {
    let world = iroha_core::state::World::new();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let telemetry = StateTelemetry::default();
    let mut state = State::with_telemetry(world, kura, query_handle, telemetry);
    state.zk.max_proof_size_bytes = 4 * 1024 * 1024;
    let material = configured_ton_source_verifier_material();
    let deployment = configured_ton_source_adapter_engine_deployment(&material);
    state
        .zk
        .sccp_source_verifier_materials
        .push(actual_source_verifier_material(&material));
    let mut deployment_config = actual_source_adapter_engine_deployment(&deployment);
    deployment_config.ton_masterchain_config_verifier_hash = hex::encode([0x26; 32]);
    state
        .zk
        .sccp_source_adapter_engine_deployments
        .push(deployment_config);

    let exec = Executor::default();
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(3_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let proof = make_sccp_ton_to_sora_unverified_message_bridge_proof(122);
    let proof_id = bridge_proof_id(&proof);
    let submit: InstructionBox =
        iroha_data_model::isi::bridge::SubmitBridgeProof::new(proof.clone()).into();
    let err = exec
        .execute_instruction(&mut stx, &ALICE_ID.clone(), submit)
        .expect_err("partial TON full-light-client audit evidence must fail closed");
    assert!(
        format!("{err:?}")
            .contains("TON full-light-client audit evidence must include masterchain config"),
        "unexpected error: {err:?}"
    );
    assert!(stx.world.proofs().get(&proof_id).is_none());
}

#[test]
fn submit_sccp_inbound_message_rejects_source_proof_without_deployment_binding_before_all_lanes_ready()
 {
    let world = iroha_core::state::World::new();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let telemetry = StateTelemetry::default();
    let mut state = State::with_telemetry(world, kura, query_handle, telemetry);
    state.zk.max_proof_size_bytes = 4 * 1024 * 1024;
    let material = configured_sol_source_verifier_material();
    let deployment = configured_sol_source_adapter_engine_deployment(&material);
    let audited_deployment = audited_sol_source_adapter_engine_deployment(&deployment);
    state
        .zk
        .sccp_source_verifier_materials
        .push(actual_source_verifier_material(&material));
    let mut deployment_config = actual_source_adapter_engine_deployment(&deployment);
    attach_solana_full_light_client_audit(&mut deployment_config, &material, &deployment);
    state
        .zk
        .sccp_source_adapter_engine_deployments
        .push(deployment_config);
    let destination_rollout = configured_sol_destination_rollout();
    let route_allowlist =
        configured_sol_route_allowlist(&material, &audited_deployment, &destination_rollout);
    state
        .zk
        .sccp_destination_rollouts
        .push(actual_destination_rollout(&destination_rollout));
    state
        .zk
        .sccp_route_allowlists
        .push(actual_route_allowlist(&route_allowlist));

    let exec = Executor::default();
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(3_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let proof = make_sccp_sol_to_sora_message_bridge_proof_with_material(97, Some(&material));
    let proof_id = bridge_proof_id(&proof);
    let submit: InstructionBox =
        iroha_data_model::isi::bridge::SubmitBridgeProof::new(proof).into();
    let err = exec
        .execute_instruction(&mut stx, &ALICE_ID.clone(), submit)
        .expect_err("source proof without deployment evidence must not bypass all-lanes launch");
    assert!(
        format!("{err:?}").contains("all-lanes launch policy")
            && format!("{err:?}").contains("source verifier material"),
        "unexpected error: {err:?}"
    );
    assert!(stx.world.proofs().get(&proof_id).is_none());
}

#[test]
fn submit_sccp_inbound_message_rejects_missing_route_allowlist_material_behind_sol_source_gate() {
    let world = iroha_core::state::World::new();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let telemetry = StateTelemetry::default();
    let mut state = State::with_telemetry(world, kura, query_handle, telemetry);
    state.zk.max_proof_size_bytes = 4 * 1024 * 1024;
    let material = configured_sol_source_verifier_material();
    let deployment = configured_sol_source_adapter_engine_deployment(&material);
    let destination_rollout = configured_sol_destination_rollout();
    state
        .zk
        .sccp_source_verifier_materials
        .push(actual_source_verifier_material(&material));
    state
        .zk
        .sccp_source_adapter_engine_deployments
        .push(actual_source_adapter_engine_deployment(&deployment));
    state
        .zk
        .sccp_destination_rollouts
        .push(actual_destination_rollout(&destination_rollout));

    let exec = Executor::default();
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(3_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let proof = make_sccp_sol_to_sora_message_bridge_proof_with_material(97, Some(&material));
    let proof_id = bridge_proof_id(&proof);
    let submit: InstructionBox =
        iroha_data_model::isi::bridge::SubmitBridgeProof::new(proof).into();
    let err = exec
        .execute_instruction(&mut stx, &ALICE_ID.clone(), submit)
        .expect_err(
            "Solana source adapter gate must fail closed before route allowlist evaluation",
        );
    assert!(
        format!("{err:?}").contains("source adapter")
            && format!("{err:?}").contains("not production-ready"),
        "unexpected error: {err:?}"
    );
    assert!(stx.world.proofs().get(&proof_id).is_none());
}

#[test]
fn submit_sccp_inbound_message_rejects_replayed_route_allowlist_material_behind_sol_source_gate() {
    let world = iroha_core::state::World::new();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let telemetry = StateTelemetry::default();
    let mut state = State::with_telemetry(world, kura, query_handle, telemetry);
    state.zk.max_proof_size_bytes = 4 * 1024 * 1024;
    let material = configured_sol_source_verifier_material();
    let deployment = configured_sol_source_adapter_engine_deployment(&material);
    let destination_rollout = configured_sol_destination_rollout();
    let mut route_allowlist = iroha_sccp::sccp_profiled_route_allowlist_v1(
        iroha_sccp::SCCP_DOMAIN_SOL,
        hex::encode([0x62; 32]),
    )
    .expect("standalone SOL route allowlist for source-gate regression");
    route_allowlist.route_allowlist_id = Some("sccp:bsc:route-allowlist:bsc-mainnet:v1".to_owned());
    state
        .zk
        .sccp_source_verifier_materials
        .push(actual_source_verifier_material(&material));
    state
        .zk
        .sccp_source_adapter_engine_deployments
        .push(actual_source_adapter_engine_deployment(&deployment));
    state
        .zk
        .sccp_destination_rollouts
        .push(actual_destination_rollout(&destination_rollout));
    state
        .zk
        .sccp_route_allowlists
        .push(actual_route_allowlist(&route_allowlist));

    let exec = Executor::default();
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(3_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let proof = make_sccp_sol_to_sora_message_bridge_proof_with_material(96, Some(&material));
    let proof_id = bridge_proof_id(&proof);
    let submit: InstructionBox =
        iroha_data_model::isi::bridge::SubmitBridgeProof::new(proof).into();
    let err = exec
        .execute_instruction(&mut stx, &ALICE_ID.clone(), submit)
        .expect_err(
            "Solana source adapter gate must fail closed before route allowlist evaluation",
        );
    assert!(
        format!("{err:?}").contains("source adapter")
            && format!("{err:?}").contains("not production-ready"),
        "unexpected error: {err:?}"
    );
    assert!(stx.world.proofs().get(&proof_id).is_none());
}

#[test]
fn submit_sccp_inbound_message_rejects_replayed_route_allowlist_after_sol_source_gate() {
    let world = iroha_core::state::World::new();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let telemetry = StateTelemetry::default();
    let mut state = State::with_telemetry(world, kura, query_handle, telemetry);
    state.zk.max_proof_size_bytes = SCCP_AUDITED_SOLANA_PROOF_MAX_BYTES;
    let material = configured_sol_source_verifier_material();
    let deployment = configured_sol_source_adapter_engine_deployment(&material);
    let audited_deployment = audited_sol_source_adapter_engine_deployment(&deployment);
    let destination_rollout = configured_sol_destination_rollout();
    let mut route_allowlist =
        configured_sol_route_allowlist(&material, &audited_deployment, &destination_rollout);
    route_allowlist.route_allowlist_hash = Some(hex::encode([0xde; 32]));
    state
        .zk
        .sccp_source_verifier_materials
        .push(actual_source_verifier_material(&material));
    let mut deployment_config = actual_source_adapter_engine_deployment(&deployment);
    attach_solana_full_light_client_audit(&mut deployment_config, &material, &deployment);
    state
        .zk
        .sccp_source_adapter_engine_deployments
        .push(deployment_config);
    state
        .zk
        .sccp_destination_rollouts
        .push(actual_destination_rollout(&destination_rollout));
    state
        .zk
        .sccp_route_allowlists
        .push(actual_route_allowlist(&route_allowlist));

    let exec = Executor::default();
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(3_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let proof = make_sccp_sol_to_sora_message_bridge_proof_with_material_and_deployment(
        96,
        Some(&material),
        Some(&audited_deployment),
    );
    let proof_id = bridge_proof_id(&proof);
    let submit: InstructionBox =
        iroha_data_model::isi::bridge::SubmitBridgeProof::new(proof).into();
    let err = exec
        .execute_instruction(&mut stx, &ALICE_ID.clone(), submit)
        .expect_err("replayed route allowlist hash must fail after source gate opens");
    assert!(
        format!("{err:?}").contains("route allowlist hash does not match the canonical"),
        "unexpected error: {err:?}"
    );
    assert!(stx.world.proofs().get(&proof_id).is_none());
}

#[test]
fn submit_sccp_inbound_message_rejects_replayed_route_allowlist_after_tron_source_gate() {
    let world = iroha_core::state::World::new();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let telemetry = StateTelemetry::default();
    let mut state = State::with_telemetry(world, kura, query_handle, telemetry);
    state.zk.max_proof_size_bytes = 4 * 1024 * 1024;
    let material = configured_tron_source_verifier_material();
    let deployment = configured_tron_source_adapter_engine_deployment(&material);
    let destination_rollout = configured_tron_destination_rollout(&material);
    let mut route_allowlist =
        configured_tron_route_allowlist(&material, &deployment, &destination_rollout);
    route_allowlist.route_allowlist_hash = Some(hex::encode([0xde; 32]));
    state
        .zk
        .sccp_source_verifier_materials
        .push(actual_source_verifier_material(&material));
    let mut deployment_config = actual_source_adapter_engine_deployment(&deployment);
    attach_tron_dpos_source_gate(&mut deployment_config, &material, &deployment);
    state
        .zk
        .sccp_source_adapter_engine_deployments
        .push(deployment_config);
    state
        .zk
        .sccp_destination_rollouts
        .push(actual_destination_rollout(&destination_rollout));
    state
        .zk
        .sccp_route_allowlists
        .push(actual_route_allowlist(&route_allowlist));

    let exec = Executor::default();
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(3_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let proof = make_sccp_tron_to_sora_unverified_message_bridge_proof(96);
    let proof_id = bridge_proof_id(&proof);
    let submit: InstructionBox =
        iroha_data_model::isi::bridge::SubmitBridgeProof::new(proof).into();
    let err = exec
        .execute_instruction(&mut stx, &ALICE_ID.clone(), submit)
        .expect_err("replayed TRON route allowlist hash must fail after source gate opens");
    assert!(
        format!("{err:?}").contains("route allowlist hash does not match the canonical"),
        "unexpected error: {err:?}"
    );
    assert!(stx.world.proofs().get(&proof_id).is_none());
}

#[test]
fn submit_sccp_inbound_message_rejects_tron_route_canary_transcript_drift_after_source_gate() {
    macro_rules! assert_tron_canary_drift_rejected {
        ($label:literal, $field:ident, $value:expr, $seed:expr) => {{
            let world = iroha_core::state::World::new();
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let telemetry = StateTelemetry::default();
            let mut state = State::with_telemetry(world, kura, query_handle, telemetry);
            state.zk.max_proof_size_bytes = 4 * 1024 * 1024;

            let material = configured_tron_source_verifier_material();
            let deployment = configured_tron_source_adapter_engine_deployment(&material);
            let destination_rollout = configured_tron_destination_rollout(&material);
            let mut route_allowlist =
                configured_tron_route_allowlist(&material, &deployment, &destination_rollout);
            route_allowlist.$field = $value;

            state
                .zk
                .sccp_source_verifier_materials
                .push(actual_source_verifier_material(&material));
            let mut deployment_config = actual_source_adapter_engine_deployment(&deployment);
            attach_tron_dpos_source_gate(&mut deployment_config, &material, &deployment);
            state
                .zk
                .sccp_source_adapter_engine_deployments
                .push(deployment_config);
            state
                .zk
                .sccp_destination_rollouts
                .push(actual_destination_rollout(&destination_rollout));
            state
                .zk
                .sccp_route_allowlists
                .push(actual_route_allowlist(&route_allowlist));

            let exec = Executor::default();
            let header =
                iroha_data_model::block::BlockHeader::new(nonzero!(3_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut stx = block.transaction();

            let proof = make_sccp_tron_to_sora_unverified_message_bridge_proof($seed);
            let proof_id = bridge_proof_id(&proof);
            let submit: InstructionBox =
                iroha_data_model::isi::bridge::SubmitBridgeProof::new(proof).into();
            let err = exec
                .execute_instruction(&mut stx, &ALICE_ID.clone(), submit)
                .expect_err(concat!(
                    "TRON route canary ",
                    $label,
                    " drift must fail after source gate opens"
                ));
            assert!(
                format!("{err:?}").contains("route canary evidence is not bound"),
                "unexpected {label} error: {err:?}",
                label = $label,
            );
            assert!(stx.world.proofs().get(&proof_id).is_none());
        }};
    }

    assert_tron_canary_drift_rejected!(
        "call-data hash",
        tron_route_canary_call_data_sha256,
        Some(hex::encode([0x01; 32])),
        123
    );
    assert_tron_canary_drift_rejected!(
        "payload hash",
        tron_route_canary_payload_hash,
        Some(hex::encode([0x02; 32])),
        124
    );
    assert_tron_canary_drift_rejected!(
        "statement hash",
        tron_route_canary_statement_hash,
        Some(hex::encode([0x03; 32])),
        125
    );
    assert_tron_canary_drift_rejected!(
        "commitment root",
        tron_route_canary_commitment_root,
        Some(hex::encode([0x04; 32])),
        126
    );
    assert_tron_canary_drift_rejected!(
        "finality height",
        tron_route_canary_finality_height,
        Some(hex::encode([0x05; 32])),
        127
    );
    assert_tron_canary_drift_rejected!(
        "finality block hash",
        tron_route_canary_finality_block_hash,
        Some(hex::encode([0x06; 32])),
        128
    );
    assert_tron_canary_drift_rejected!(
        "proof version",
        tron_route_canary_proof_version,
        Some(2),
        129
    );
    assert_tron_canary_drift_rejected!(
        "proof source domain",
        tron_route_canary_proof_source_domain,
        Some(iroha_sccp::SCCP_DOMAIN_ETH),
        130
    );
}

#[test]
fn submit_sccp_inbound_message_rejects_tron_destination_network_drift_after_source_gate() {
    let world = iroha_core::state::World::new();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let telemetry = StateTelemetry::default();
    let mut state = State::with_telemetry(world, kura, query_handle, telemetry);
    state.zk.max_proof_size_bytes = 4 * 1024 * 1024;
    let material = configured_tron_source_verifier_material();
    let deployment = configured_tron_source_adapter_engine_deployment(&material);
    let drifted_destination_rollout =
        iroha_sccp::sccp_tron_mainnet_destination_rollout_with_binding_v1(
            "TJRabPrwbZy45sbavfcjinPJC18kjpRTv8".to_owned(),
            hex::encode([0x66; 32]),
            hex::encode([0x67; 32]),
            hex::encode([0x77; 32]),
        )
        .expect("TRON destination rollout with mismatched network id");
    assert!(
        iroha_sccp::sccp_destination_rollout_is_production_ready(
            iroha_sccp::SCCP_DOMAIN_TRON,
            &drifted_destination_rollout,
        ),
        "the destination record is internally production-shaped before lane coherence is checked"
    );
    let route_allowlist = iroha_sccp::sccp_profiled_route_allowlist_v1(
        iroha_sccp::SCCP_DOMAIN_TRON,
        hex::encode([0x78; 32]),
    )
    .expect("standalone TRON route allowlist for destination-drift regression");
    state
        .zk
        .sccp_source_verifier_materials
        .push(actual_source_verifier_material(&material));
    let mut deployment_config = actual_source_adapter_engine_deployment(&deployment);
    attach_tron_dpos_source_gate(&mut deployment_config, &material, &deployment);
    state
        .zk
        .sccp_source_adapter_engine_deployments
        .push(deployment_config);
    state
        .zk
        .sccp_destination_rollouts
        .push(actual_destination_rollout(&drifted_destination_rollout));
    state
        .zk
        .sccp_route_allowlists
        .push(actual_route_allowlist(&route_allowlist));

    let exec = Executor::default();
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(3_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let proof = make_sccp_tron_to_sora_unverified_message_bridge_proof(97);
    let proof_id = bridge_proof_id(&proof);
    let submit: InstructionBox =
        iroha_data_model::isi::bridge::SubmitBridgeProof::new(proof).into();
    let err = exec
        .execute_instruction(&mut stx, &ALICE_ID.clone(), submit)
        .expect_err("TRON destination network drift must fail after source gate opens");
    assert!(
        format!("{err:?}")
            .contains("destination verifier rollout material is not production-ready"),
        "unexpected error: {err:?}"
    );
    assert!(stx.world.proofs().get(&proof_id).is_none());
}

#[test]
fn submit_sccp_inbound_message_rejects_replayed_source_adapter_deployment() {
    let world = iroha_core::state::World::new();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let telemetry = StateTelemetry::default();
    let mut state = State::with_telemetry(world, kura, query_handle, telemetry);
    state.zk.max_proof_size_bytes = 4 * 1024 * 1024;
    let material = configured_sol_source_verifier_material();
    let mut deployment = configured_sol_source_adapter_engine_deployment(&material);
    deployment.consensus_verifier_hash = [0xEE; 32];
    state
        .zk
        .sccp_source_verifier_materials
        .push(actual_source_verifier_material(&material));
    state
        .zk
        .sccp_source_adapter_engine_deployments
        .push(actual_source_adapter_engine_deployment(&deployment));

    let exec = Executor::default();
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(3_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let proof = make_sccp_sol_to_sora_message_bridge_proof_with_material(98, Some(&material));
    let proof_id = bridge_proof_id(&proof);
    let submit: InstructionBox =
        iroha_data_model::isi::bridge::SubmitBridgeProof::new(proof).into();
    let err = exec
        .execute_instruction(&mut stx, &ALICE_ID.clone(), submit)
        .expect_err("replayed source adapter deployment must fail closed");
    assert!(
        format!("{err:?}").contains("source adapter")
            && format!("{err:?}").contains("not production-ready"),
        "unexpected error: {err:?}"
    );
    assert!(stx.world.proofs().get(&proof_id).is_none());
}

#[test]
fn submit_sccp_inbound_message_rejects_replayed_source_adapter_verifier_commitment() {
    let world = iroha_core::state::World::new();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let telemetry = StateTelemetry::default();
    let mut state = State::with_telemetry(world, kura, query_handle, telemetry);
    state.zk.max_proof_size_bytes = 4 * 1024 * 1024;
    let material = configured_sol_source_verifier_material();
    let mut deployment = configured_sol_source_adapter_engine_deployment(&material);
    deployment.adapter_verifier_vk_hash[0] ^= 0x01;
    state
        .zk
        .sccp_source_verifier_materials
        .push(actual_source_verifier_material(&material));
    state
        .zk
        .sccp_source_adapter_engine_deployments
        .push(actual_source_adapter_engine_deployment(&deployment));

    let exec = Executor::default();
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(3_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let proof = make_sccp_sol_to_sora_message_bridge_proof_with_material(95, Some(&material));
    let proof_id = bridge_proof_id(&proof);
    let submit: InstructionBox =
        iroha_data_model::isi::bridge::SubmitBridgeProof::new(proof).into();
    let err = exec
        .execute_instruction(&mut stx, &ALICE_ID.clone(), submit)
        .expect_err("replayed source adapter verifier commitment must fail closed");
    assert!(
        format!("{err:?}").contains("source adapter")
            && format!("{err:?}").contains("not production-ready"),
        "unexpected error: {err:?}"
    );
    assert!(stx.world.proofs().get(&proof_id).is_none());
}

#[test]
fn submit_sccp_inbound_message_rejects_source_adapter_deployment_for_non_sora_target() {
    let world = iroha_core::state::World::new();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let telemetry = StateTelemetry::default();
    let mut state = State::with_telemetry(world, kura, query_handle, telemetry);
    state.zk.max_proof_size_bytes = 4 * 1024 * 1024;
    let material = configured_sol_source_verifier_material();
    let deployment = iroha_sccp::sccp_source_adapter_engine_deployment_from_material_for_target_v1(
        &material,
        iroha_sccp::SCCP_DOMAIN_ETH,
        [0x56; 32],
    )
    .expect("SOL source adapter deployment for non-SORA target");
    assert_ne!(deployment.target_domain, iroha_sccp::SCCP_DOMAIN_SORA);
    assert!(
        iroha_sccp::sccp_source_adapter_engine_deployment_matches_material(&material, &deployment,)
    );
    assert!(
        !iroha_sccp::sccp_source_adapter_ready_with_material_and_deployment_for_domain(
            iroha_sccp::SCCP_DOMAIN_SOL,
            &material,
            &deployment,
        ),
        "inbound source adapter deployments must target SORA"
    );
    state
        .zk
        .sccp_source_verifier_materials
        .push(actual_source_verifier_material(&material));
    state
        .zk
        .sccp_source_adapter_engine_deployments
        .push(actual_source_adapter_engine_deployment(&deployment));

    let exec = Executor::default();
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(3_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let proof = make_sccp_sol_to_sora_message_bridge_proof_with_material(94, Some(&material));
    let proof_id = bridge_proof_id(&proof);
    let submit: InstructionBox =
        iroha_data_model::isi::bridge::SubmitBridgeProof::new(proof).into();
    let err = exec
        .execute_instruction(&mut stx, &ALICE_ID.clone(), submit)
        .expect_err("source adapter deployment for another target must fail closed");
    assert!(
        format!("{err:?}").contains("must target SORA"),
        "unexpected error: {err:?}"
    );
    assert!(stx.world.proofs().get(&proof_id).is_none());
}

#[test]
fn submit_sccp_inbound_message_rejects_configured_source_verifier_material_until_engines_ready() {
    let world = iroha_core::state::World::new();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let telemetry = StateTelemetry::default();
    let mut state = State::with_telemetry(world, kura, query_handle, telemetry);
    state.zk.max_proof_size_bytes = 4 * 1024 * 1024;
    let material = configured_sol_source_verifier_material();
    state
        .zk
        .sccp_source_verifier_materials
        .push(actual_source_verifier_material(&material));

    let exec = Executor::default();
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(3_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let proof = make_sccp_sol_to_sora_message_bridge_proof_with_material(100, Some(&material));
    let proof_id = bridge_proof_id(&proof);
    let submit: InstructionBox =
        iroha_data_model::isi::bridge::SubmitBridgeProof::new(proof.clone()).into();
    let err = exec
        .execute_instruction(&mut stx, &ALICE_ID.clone(), submit)
        .expect_err("configured source verifier material must not bypass missing source engines");
    assert!(
        format!("{err:?}").contains("source adapter")
            && format!("{err:?}").contains("not production-ready"),
        "unexpected error: {err:?}"
    );

    assert!(stx.world.proofs().get(&proof_id).is_none());
}

#[test]
fn submit_sccp_inbound_message_rejects_generic_source_verifier_material() {
    let world = iroha_core::state::World::new();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let telemetry = StateTelemetry::default();
    let mut state = State::with_telemetry(world, kura, query_handle, telemetry);
    state.zk.max_proof_size_bytes = 4 * 1024 * 1024;
    let material = generic_sol_source_verifier_material();
    state
        .zk
        .sccp_source_verifier_materials
        .push(actual_source_verifier_material(&material));

    let exec = Executor::default();
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(3_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let proof = make_sccp_sol_to_sora_message_bridge_proof_with_material(100, Some(&material));
    let submit: InstructionBox =
        iroha_data_model::isi::bridge::SubmitBridgeProof::new(proof.clone()).into();
    let err = exec
        .execute_instruction(&mut stx, &ALICE_ID.clone(), submit)
        .expect_err("generic source verifier material must fail closed");
    assert!(
        format!("{err:?}").contains("not production-ready"),
        "unexpected error: {err:?}"
    );

    let proof_id = bridge_proof_id(&proof);
    assert!(stx.world.proofs().get(&proof_id).is_none());
}

#[test]
fn submit_sccp_inbound_message_rejects_duplicate_source_verifier_material() {
    let world = iroha_core::state::World::new();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let telemetry = StateTelemetry::default();
    let mut state = State::with_telemetry(world, kura, query_handle, telemetry);
    state.zk.max_proof_size_bytes = 4 * 1024 * 1024;
    let material = configured_sol_source_verifier_material();
    let actual = actual_source_verifier_material(&material);
    state.zk.sccp_source_verifier_materials.push(actual.clone());
    state.zk.sccp_source_verifier_materials.push(actual);

    let exec = Executor::default();
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(3_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let proof = make_sccp_sol_to_sora_message_bridge_proof_with_material(101, Some(&material));
    let submit: InstructionBox =
        iroha_data_model::isi::bridge::SubmitBridgeProof::new(proof).into();
    let err = exec
        .execute_instruction(&mut stx, &ALICE_ID.clone(), submit)
        .expect_err("duplicate source verifier material must fail closed");
    assert!(
        format!("{err:?}").contains("duplicated"),
        "unexpected error: {err:?}"
    );
}

#[test]
fn submit_sccp_inbound_message_rejects_placeholder_source_verifier_material() {
    let world = iroha_core::state::World::new();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let telemetry = StateTelemetry::default();
    let mut state = State::with_telemetry(world, kura, query_handle, telemetry);
    state.zk.max_proof_size_bytes = 4 * 1024 * 1024;
    let material = configured_sol_source_verifier_material();
    let mut placeholder = actual_source_verifier_material(&material);
    placeholder.placeholder_material = true;
    state.zk.sccp_source_verifier_materials.push(placeholder);

    let exec = Executor::default();
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(3_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let proof = make_sccp_sol_to_sora_message_bridge_proof_with_material(102, Some(&material));
    let submit: InstructionBox =
        iroha_data_model::isi::bridge::SubmitBridgeProof::new(proof).into();
    let err = exec
        .execute_instruction(&mut stx, &ALICE_ID.clone(), submit)
        .expect_err("placeholder material must remain disabled");
    assert!(
        format!("{err:?}").contains("not production-ready"),
        "unexpected error: {err:?}"
    );
}

#[test]
fn submit_sccp_inbound_message_rejects_malformed_source_verifier_material_hash() {
    let world = iroha_core::state::World::new();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let telemetry = StateTelemetry::default();
    let mut state = State::with_telemetry(world, kura, query_handle, telemetry);
    state.zk.max_proof_size_bytes = 4 * 1024 * 1024;
    let material = configured_sol_source_verifier_material();
    let mut actual = actual_source_verifier_material(&material);
    actual.source_trust_anchor_hash = "not-hex".to_owned();
    state.zk.sccp_source_verifier_materials.push(actual);

    let exec = Executor::default();
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(3_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let proof = make_sccp_sol_to_sora_message_bridge_proof_with_material(103, Some(&material));
    let submit: InstructionBox =
        iroha_data_model::isi::bridge::SubmitBridgeProof::new(proof).into();
    let err = exec
        .execute_instruction(&mut stx, &ALICE_ID.clone(), submit)
        .expect_err("malformed material hash must fail closed");
    assert!(
        format!("{err:?}").contains("source_trust_anchor_hash"),
        "unexpected error: {err:?}"
    );
}

#[test]
fn submit_sccp_inbound_message_rejects_replayed_source_verifier_material() {
    let world = iroha_core::state::World::new();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let telemetry = StateTelemetry::default();
    let mut state = State::with_telemetry(world, kura, query_handle, telemetry);
    state.zk.max_proof_size_bytes = 4 * 1024 * 1024;
    let material = configured_sol_source_verifier_material();
    let mut replayed = material.clone();
    replayed.consensus_verifier_hash = [0xEE; 32];
    state
        .zk
        .sccp_source_verifier_materials
        .push(actual_source_verifier_material(&replayed));

    let exec = Executor::default();
    let header = iroha_data_model::block::BlockHeader::new(nonzero!(3_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let proof = make_sccp_sol_to_sora_message_bridge_proof_with_material(104, Some(&material));
    let submit: InstructionBox =
        iroha_data_model::isi::bridge::SubmitBridgeProof::new(proof).into();
    let err = exec
        .execute_instruction(&mut stx, &ALICE_ID.clone(), submit)
        .expect_err("replayed verifier material must not bypass missing source engines");
    assert!(
        format!("{err:?}").contains("source adapter")
            && format!("{err:?}").contains("not production-ready"),
        "unexpected error: {err:?}"
    );
}
