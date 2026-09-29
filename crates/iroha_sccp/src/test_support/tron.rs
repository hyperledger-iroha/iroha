//! Synthetic TRON chain for TRON light-client tests (`specs/sccp.md` §4.13.3, §11).
//!
//! [`SyntheticTronChainV1`] derives 27 deterministic witnesses per roster from a seed and
//! produces signed `BlockHeader.raw` protobufs on the mainnet maintenance grid: height 1 000 is
//! the maintenance block of period [`SYNTHETIC_TRON_FIRST_PERIOD`] and height 8 200 the one of
//! the next period (3-second slots). Blocks are produced round-robin by the set of the parent's
//! period (so the maintenance block is produced by the outgoing set), signatures use the same
//! secp256k1 recovery the verifier checks, and any block may commit to real transactions, so
//! tests prove calls through the production light client.

use std::{
    collections::BTreeMap,
    sync::{Mutex, PoisonError},
};

use iroha_data_model::sccp::light_client::SccpLcBootstrapV1;
use sha2::{Digest as _, Sha256};

use crate::{
    light_client::{
        profile::{TRON_MAINNET, TronChainProfileV1},
        proof::SccpLcBootstrapDataV1,
        tron::{
            TronLcBootstrapV1, TronRawSegmentV1, TronSegmentV1, TronSignedHeaderV1,
            TronTransactionProofV1, TronWitnessSetV1, TronWitnessV1, merkle_root_and_branch,
        },
    },
    v1::{
        hashes::keccak256,
        signature::{address_of_secret, sign_digest},
    },
};

/// Height of the maintenance block of [`SYNTHETIC_TRON_FIRST_PERIOD`].
pub const SYNTHETIC_TRON_BOUNDARY_HEIGHT: u64 = 1_000;
/// Height of the maintenance block of the period after it.
pub const SYNTHETIC_TRON_NEXT_BOUNDARY_HEIGHT: u64 = 8_200;
/// Maintenance period starting at [`SYNTHETIC_TRON_BOUNDARY_HEIGHT`] (2026-09-21T22:40:00Z).
pub const SYNTHETIC_TRON_FIRST_PERIOD: u64 = 82_870;

#[derive(Clone, Copy, Debug)]
struct RosterV1 {
    from_period: u64,
    replaced: usize,
    rotated: Option<usize>,
}

#[derive(Clone, Copy)]
struct MemberV1 {
    account: [u8; 21],
    secret: [u8; 32],
    signer: [u8; 21],
}

/// One synthetic block.
#[derive(Clone, Debug)]
pub struct SyntheticTronBlockV1 {
    /// `BlockHeader.raw` bytes.
    pub raw: Vec<u8>,
    /// Witness signature (`v ∈ {0, 1}`).
    pub signature: Vec<u8>,
    /// Block id.
    pub id: [u8; 32],
    /// Time (ms).
    pub time_ms: u64,
}

/// Deterministic synthetic TRON chain.
pub struct SyntheticTronChainV1 {
    seed: [u8; 32],
    profile: TronChainProfileV1,
    rosters: Vec<RosterV1>,
    transactions: BTreeMap<u64, Vec<Vec<u8>>>,
    salts: BTreeMap<u64, [u8; 32]>,
    blocks: Mutex<BTreeMap<u64, SyntheticTronBlockV1>>,
    members: Mutex<BTreeMap<u64, Vec<MemberV1>>>,
}

impl Clone for SyntheticTronChainV1 {
    fn clone(&self) -> Self {
        Self {
            seed: self.seed,
            profile: self.profile,
            rosters: self.rosters.clone(),
            transactions: self.transactions.clone(),
            salts: self.salts.clone(),
            blocks: Mutex::new(BTreeMap::new()),
            members: Mutex::new(BTreeMap::new()),
        }
    }
}

fn seeded(parts: &[&[u8]]) -> [u8; 32] {
    let mut all: Vec<&[u8]> = vec![b"SCCP/SYNTHETIC/TRON/V1"];
    all.extend_from_slice(parts);
    keccak256(&all)
}

fn tron_address(secret: &[u8; 32]) -> [u8; 21] {
    let mut address = [0x41_u8; 21];
    address[1..].copy_from_slice(&address_of_secret(secret).expect("synthetic secrets are valid"));
    address
}

fn push_varint(out: &mut Vec<u8>, mut value: u64) {
    while value >= 0x80 {
        out.push(value.to_le_bytes()[0] | 0x80);
        value >>= 7;
    }
    out.push(value.to_le_bytes()[0]);
}

fn push_uint(out: &mut Vec<u8>, field: u64, value: u64) {
    push_varint(out, field << 3);
    push_varint(out, value);
}

fn push_bytes(out: &mut Vec<u8>, field: u64, value: &[u8]) {
    push_varint(out, (field << 3) | 2);
    push_varint(out, u64::try_from(value.len()).expect("small"));
    out.extend_from_slice(value);
}

/// A full `protocol.Transaction` of one `TriggerSmartContract` call with the given
/// `contractRet` (1 = SUCCESS) and TRX `call_value`.
#[must_use]
pub fn trigger_transaction(
    owner: &[u8; 21],
    contract: &[u8; 21],
    data: &[u8],
    contract_ret: u64,
    call_value: u64,
) -> Vec<u8> {
    let mut call = Vec::new();
    push_bytes(&mut call, 1, owner);
    push_bytes(&mut call, 2, contract);
    if call_value != 0 {
        push_uint(&mut call, 3, call_value);
    }
    push_bytes(&mut call, 4, data);
    let mut any = Vec::new();
    push_bytes(
        &mut any,
        1,
        b"type.googleapis.com/protocol.TriggerSmartContract",
    );
    push_bytes(&mut any, 2, &call);
    let mut contract_message = Vec::new();
    push_uint(&mut contract_message, 1, 31);
    push_bytes(&mut contract_message, 2, &any);
    push_uint(&mut contract_message, 5, 2);
    let mut raw = Vec::new();
    push_bytes(&mut raw, 1, &[0x12, 0x34]);
    push_bytes(&mut raw, 4, &[0x56; 8]);
    push_uint(&mut raw, 8, 1_790_000_060_000);
    push_bytes(&mut raw, 10, b"memo");
    push_bytes(&mut raw, 11, &contract_message);
    push_uint(&mut raw, 14, 1_790_000_000_000);
    push_uint(&mut raw, 18, 1_000_000_000);
    let mut result = Vec::new();
    push_uint(&mut result, 1, 42);
    push_uint(&mut result, 3, contract_ret);
    let mut transaction = Vec::new();
    push_bytes(&mut transaction, 1, &raw);
    push_bytes(&mut transaction, 2, &[0x77; 65]);
    push_bytes(&mut transaction, 5, &result);
    transaction
}

impl SyntheticTronChainV1 {
    /// A chain whose 27 witnesses never change, under the compiled mainnet profile.
    #[must_use]
    pub fn new(seed: [u8; 32]) -> Self {
        Self {
            seed,
            profile: TRON_MAINNET,
            rosters: vec![RosterV1 {
                from_period: 0,
                replaced: 0,
                rotated: None,
            }],
            transactions: BTreeMap::new(),
            salts: BTreeMap::new(),
            blocks: Mutex::new(BTreeMap::new()),
            members: Mutex::new(BTreeMap::new()),
        }
    }

    /// The same chain whose active set from `period` on has its last `replaced` witnesses
    /// replaced by new ones, and witness `rotated` (if any) signing with a new key.
    #[must_use]
    pub fn with_roster(mut self, period: u64, replaced: usize, rotated: Option<usize>) -> Self {
        self.rosters.push(RosterV1 {
            from_period: period,
            replaced,
            rotated,
        });
        self.rosters.sort_by_key(|roster| roster.from_period);
        self.blocks = Mutex::new(BTreeMap::new());
        self.members = Mutex::new(BTreeMap::new());
        self
    }

    /// The same chain whose block `height` commits to `transactions`.
    #[must_use]
    pub fn with_transactions(&self, height: u64, transactions: Vec<Vec<u8>>) -> Self {
        let mut chain = self.clone();
        chain.transactions.insert(height, transactions);
        chain
    }

    /// The same chain forked at `height`: that block commits to another transaction root, so it
    /// and every descendant get other ids while the same witnesses sign them.
    #[must_use]
    pub fn forked_at(&self, height: u64) -> Self {
        let mut chain = self.clone();
        chain.salts.insert(
            height,
            seeded(&[&self.seed, b"fork", &height.to_be_bytes()]),
        );
        chain
    }

    /// The chain profile.
    #[must_use]
    pub const fn profile(&self) -> &TronChainProfileV1 {
        &self.profile
    }

    /// Time of `height` (ms).
    #[must_use]
    pub fn time_ms(&self, height: u64) -> u64 {
        let start = self
            .profile
            .period_start_ms(SYNTHETIC_TRON_FIRST_PERIOD)
            .expect("period start");
        let offset = i128::from(height) - i128::from(SYNTHETIC_TRON_BOUNDARY_HEIGHT);
        u64::try_from(i128::from(start) + offset * 3_000 + 1_500).expect("positive time")
    }

    /// Maintenance period of `height`.
    #[must_use]
    pub fn period(&self, height: u64) -> u64 {
        self.profile.period_at(self.time_ms(height))
    }

    fn members(&self, period: u64) -> Vec<MemberV1> {
        let roster = self
            .rosters
            .iter()
            .rev()
            .find(|roster| roster.from_period <= period)
            .copied()
            .expect("roster 0 covers every period");
        let mut cache = self.members.lock().unwrap_or_else(PoisonError::into_inner);
        cache
            .entry(roster.from_period)
            .or_insert_with(|| self.derive_members(roster))
            .clone()
    }

    fn derive_members(&self, roster: RosterV1) -> Vec<MemberV1> {
        let mut members: Vec<MemberV1> = (0..27_u64)
            .map(|position| {
                let replaced = u64::try_from(roster.replaced).expect("small");
                let index = if position >= 27 - replaced {
                    1_000 + position
                } else {
                    position
                };
                let secret = seeded(&[&self.seed, b"account", &index.to_be_bytes()]);
                let signing = if roster
                    .rotated
                    .and_then(|rotated| u64::try_from(rotated).ok())
                    == Some(position)
                {
                    seeded(&[&self.seed, b"rotated", &index.to_be_bytes()])
                } else {
                    secret
                };
                MemberV1 {
                    account: tron_address(&secret),
                    secret: signing,
                    signer: tron_address(&signing),
                }
            })
            .collect();
        members.sort_by_key(|member| member.account);
        members
    }

    /// The active witness set of `period`.
    #[must_use]
    pub fn witness_set(&self, period: u64) -> TronWitnessSetV1 {
        TronWitnessSetV1 {
            period,
            witnesses: self
                .members(period)
                .into_iter()
                .map(|member| TronWitnessV1 {
                    account_address: member.account.to_vec(),
                    signing_address: member.signer.to_vec(),
                })
                .collect(),
        }
    }

    fn transaction_leaves(&self, height: u64) -> Option<Vec<[u8; 32]>> {
        self.transactions.get(&height).map(|transactions| {
            transactions
                .iter()
                .map(|transaction| Sha256::digest(transaction).into())
                .collect()
        })
    }

    fn build(&self, height: u64, parent_id: [u8; 32]) -> SyntheticTronBlockV1 {
        let producer_period = self.period(height - 1);
        let members = self.members(producer_period);
        let producer = members[usize::try_from(height % 27).expect("small")];
        let tx_root = self.transaction_leaves(height).map_or_else(
            || {
                seeded(&[
                    &self.seed,
                    b"transactions",
                    &height.to_be_bytes(),
                    &self.salts.get(&height).copied().unwrap_or_default(),
                ])
            },
            |leaves| merkle_root_and_branch(&leaves, 0).expect("leaves").0,
        );
        let time_ms = self.time_ms(height);
        let mut raw = Vec::new();
        push_uint(&mut raw, 1, time_ms);
        push_bytes(&mut raw, 2, &tx_root);
        push_bytes(&mut raw, 3, &parent_id);
        push_uint(&mut raw, 7, height);
        push_bytes(&mut raw, 9, &producer.account);
        push_uint(&mut raw, 10, 31);
        let raw_hash: [u8; 32] = Sha256::digest(&raw).into();
        let mut signature = sign_digest(&producer.secret, &raw_hash)
            .expect("signs")
            .to_vec();
        signature[64] -= 27;
        let mut id = raw_hash;
        id[..8].copy_from_slice(&height.to_be_bytes());
        SyntheticTronBlockV1 {
            raw,
            signature,
            id,
            time_ms,
        }
    }

    /// The block at `height` (≥ 1).
    #[must_use]
    pub fn block(&self, height: u64) -> SyntheticTronBlockV1 {
        let mut cache = self.blocks.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(block) = cache.get(&height) {
            return block.clone();
        }
        let (mut next, mut parent) = match cache.range(..height).next_back() {
            Some((number, block)) => (number + 1, block.id),
            None => (1, seeded(&[&self.seed, b"genesis"])),
        };
        while next <= height {
            let block = self.build(next, parent);
            parent = block.id;
            cache.insert(next, block);
            next += 1;
        }
        cache.get(&height).cloned().expect("built above")
    }

    /// Signed headers `first..=last`.
    #[must_use]
    pub fn segment(&self, first: u64, last: u64) -> TronSegmentV1 {
        TronSegmentV1 {
            headers: (first..=last)
                .map(|height| {
                    let block = self.block(height);
                    TronSignedHeaderV1 {
                        raw_data: block.raw,
                        witness_signature: block.signature,
                    }
                })
                .collect(),
        }
    }

    /// Unsigned headers `first..=last`.
    #[must_use]
    pub fn raw_segment(&self, first: u64, last: u64) -> TronRawSegmentV1 {
        TronRawSegmentV1 {
            headers: (first..=last)
                .map(|height| self.block(height).raw)
                .collect(),
        }
    }

    /// Inclusion proof of transaction `index` of block `height`.
    #[must_use]
    pub fn transaction_proof(&self, height: u64, index: usize) -> TronTransactionProofV1 {
        let transactions = &self.transactions[&height];
        let leaves = self.transaction_leaves(height).expect("transactions");
        let (_, merkle_branch) = merkle_root_and_branch(&leaves, index).expect("index");
        TronTransactionProofV1 {
            transaction_index: u32::try_from(index).expect("small"),
            transaction_count: u32::try_from(transactions.len()).expect("small"),
            transaction: transactions[index].clone(),
            merkle_branch,
        }
    }

    /// The bootstrap of the period of `height`, checkpointed at `height`.
    #[must_use]
    pub fn bootstrap(&self, height: u64) -> SccpLcBootstrapV1 {
        SccpLcBootstrapDataV1::Tron(TronLcBootstrapV1 {
            set: self.witness_set(self.period(height)),
            checkpoint_header: self.block(height).raw,
        })
        .to_bootstrap()
        .expect("synthetic bootstraps encode")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn periods_rosters_and_links_follow_the_grid() {
        let chain = SyntheticTronChainV1::new([1; 32]).with_roster(
            SYNTHETIC_TRON_FIRST_PERIOD + 1,
            3,
            Some(0),
        );
        assert_eq!(chain.period(999), SYNTHETIC_TRON_FIRST_PERIOD - 1);
        assert_eq!(chain.period(1_000), SYNTHETIC_TRON_FIRST_PERIOD);
        assert_eq!(chain.period(8_199), SYNTHETIC_TRON_FIRST_PERIOD);
        assert_eq!(chain.period(8_200), SYNTHETIC_TRON_FIRST_PERIOD + 1);
        let before = chain.witness_set(SYNTHETIC_TRON_FIRST_PERIOD);
        let after = chain.witness_set(SYNTHETIC_TRON_FIRST_PERIOD + 1);
        let shared = before
            .witnesses
            .iter()
            .filter(|witness| {
                after
                    .witnesses
                    .iter()
                    .any(|other| other.account_address == witness.account_address)
            })
            .count();
        assert_eq!(shared, 24);
        assert_eq!(chain.block(5).time_ms + 3_000, chain.block(6).time_ms);
        assert!(!trigger_transaction(&[0x41; 21], &[0x41; 21], &[1], 1, 0).is_empty());
    }
}
