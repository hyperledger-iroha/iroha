//! Synthetic Parlia chain for BSC light-client tests (`specs/sccp.md` §4.13.3, §11).
//!
//! [`SyntheticParliaChainV1`] derives deterministic BLS validator sets from a seed, builds
//! 21-field post-Mendel headers (millisecond timestamps in the mix digest, epoch checkpoints
//! carrying the announced roster and turn length) and signs fast-finality vote attestations with
//! the same BLS implementation (`blstrs`) and proof-of-possession DST the verifier uses. Any
//! header may commit to an arbitrary receipts root, so tests prove burns through the production
//! light client.

use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, PoisonError},
};

use blstrs::{G1Projective, G2Projective, Scalar};
use group::Group as _;
use iroha_crypto::ETHEREUM_BLS_POP_DST;
use iroha_data_model::sccp::light_client::SccpLcBootstrapV1;

use crate::{
    ethereum_source::{EMPTY_TRIE_ROOT, rlp_encode_bytes, rlp_encode_list, rlp_encode_u64},
    light_client::{
        bsc::{BscAdvanceStepV1, BscFinalityV1, BscLcBootstrapV1, BscValidatorV1, vote_data_hash},
        profile::{BSC_MAINNET, BscChainProfileV1},
        proof::SccpLcBootstrapDataV1,
    },
    v1::hashes::keccak256,
};

/// Time of height 0 of every synthetic chain (2026-09-21T20:53:20Z), inside the compiled window.
pub const SYNTHETIC_PARLIA_BASE_TIME_MS: u64 = 1_790_000_000_000;
/// Block interval of the synthetic chain.
pub const SYNTHETIC_PARLIA_INTERVAL_MS: u64 = 450;

/// One synthetic header.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyntheticParliaHeaderV1 {
    /// Header RLP.
    pub rlp: Vec<u8>,
    /// Block hash.
    pub hash: [u8; 32],
    /// Height.
    pub number: u64,
    /// Time (ms).
    pub time_ms: u64,
}

struct SyntheticMemberV1 {
    address: [u8; 20],
    secret: Scalar,
    public_key: [u8; 48],
}

/// A roster announced from `from_epoch` on.
#[derive(Clone, Copy, Debug)]
struct SyntheticRosterV1 {
    from_epoch: u64,
    index: u32,
    size: usize,
    turn_length: u8,
}

/// Deterministic synthetic Parlia chain starting at height 0.
pub struct SyntheticParliaChainV1 {
    seed: [u8; 32],
    profile: BscChainProfileV1,
    rosters: Vec<SyntheticRosterV1>,
    receipts_roots: BTreeMap<u64, [u8; 32]>,
    headers: Mutex<BTreeMap<u64, SyntheticParliaHeaderV1>>,
    members: Mutex<BTreeMap<u32, Arc<Vec<SyntheticMemberV1>>>>,
}

impl Clone for SyntheticParliaChainV1 {
    fn clone(&self) -> Self {
        Self {
            seed: self.seed,
            profile: self.profile,
            rosters: self.rosters.clone(),
            receipts_roots: self.receipts_roots.clone(),
            headers: Mutex::new(BTreeMap::new()),
            members: Mutex::new(BTreeMap::new()),
        }
    }
}

fn seeded(parts: &[&[u8]]) -> [u8; 32] {
    let mut all: Vec<&[u8]> = vec![b"SCCP/SYNTHETIC/PARLIA/V1"];
    all.extend_from_slice(parts);
    keccak256(&all)
}

fn scalar(bytes: [u8; 32]) -> Scalar {
    let mut bytes = bytes;
    // Clearing the top two bits keeps the value below the BLS12-381 group order.
    bytes[0] &= 0x3f;
    bytes[31] |= 1;
    Scalar::from_bytes_be(&bytes).expect("a value below 2^254 is a canonical scalar")
}

fn sign(secret: &Scalar, message: &[u8; 32]) -> [u8; 96] {
    (G2Projective::hash_to_curve(message, ETHEREUM_BLS_POP_DST, &[]) * secret).to_compressed()
}

fn encode_attestation(
    bitmap: u64,
    signature: &[u8; 96],
    source: (u64, [u8; 32]),
    target: (u64, [u8; 32]),
) -> Vec<u8> {
    rlp_encode_list(&[
        rlp_encode_u64(bitmap),
        rlp_encode_bytes(signature),
        rlp_encode_list(&[
            rlp_encode_u64(source.0),
            rlp_encode_bytes(&source.1),
            rlp_encode_u64(target.0),
            rlp_encode_bytes(&target.1),
        ]),
        rlp_encode_bytes(&[]),
    ])
}

impl SyntheticParliaChainV1 {
    /// A chain whose checkpoints announce roster 0 of `size` validators with `turn_length`,
    /// under the compiled mainnet profile.
    #[must_use]
    pub fn new(seed: [u8; 32], size: usize, turn_length: u8) -> Self {
        Self {
            seed,
            profile: BSC_MAINNET,
            rosters: vec![SyntheticRosterV1 {
                from_epoch: 0,
                index: 0,
                size,
                turn_length,
            }],
            receipts_roots: BTreeMap::new(),
            headers: Mutex::new(BTreeMap::new()),
            members: Mutex::new(BTreeMap::new()),
        }
    }

    /// The same chain whose checkpoints announce roster `index` of `size` validators with
    /// `turn_length` from epoch `epoch` on.
    #[must_use]
    pub fn with_transition(mut self, epoch: u64, index: u32, size: usize, turn_length: u8) -> Self {
        self.rosters.push(SyntheticRosterV1 {
            from_epoch: epoch,
            index,
            size,
            turn_length,
        });
        self.rosters.sort_by_key(|roster| roster.from_epoch);
        self.headers = Mutex::new(BTreeMap::new());
        self
    }

    /// The same chain whose header at `height` commits to `receipts_root`.
    #[must_use]
    pub fn with_receipts_root(&self, height: u64, receipts_root: [u8; 32]) -> Self {
        let mut chain = self.clone();
        chain.receipts_roots.insert(height, receipts_root);
        chain
    }

    /// The chain profile.
    #[must_use]
    pub const fn profile(&self) -> &BscChainProfileV1 {
        &self.profile
    }

    /// Time of `height` (ms).
    #[must_use]
    pub const fn time_ms(&self, height: u64) -> u64 {
        SYNTHETIC_PARLIA_BASE_TIME_MS + height * SYNTHETIC_PARLIA_INTERVAL_MS
    }

    fn roster_at(&self, height: u64) -> SyntheticRosterV1 {
        let epoch = height / self.profile.epoch_length;
        *self
            .rosters
            .iter()
            .rev()
            .find(|roster| roster.from_epoch <= epoch)
            .expect("roster 0 starts at epoch 0")
    }

    fn members(&self, roster: SyntheticRosterV1) -> Arc<Vec<SyntheticMemberV1>> {
        let mut cache = self.members.lock().unwrap_or_else(PoisonError::into_inner);
        Arc::clone(cache.entry(roster.index).or_insert_with(|| {
            let mut members = (0..roster.size as u64)
                .map(|position| {
                    let key = seeded(&[
                        &self.seed,
                        b"member",
                        &roster.index.to_be_bytes(),
                        &position.to_be_bytes(),
                    ]);
                    let secret = scalar(key);
                    let mut address = [0_u8; 20];
                    address.copy_from_slice(&seeded(&[&key, b"address"])[12..]);
                    SyntheticMemberV1 {
                        address,
                        secret,
                        public_key: (G1Projective::generator() * secret).to_compressed(),
                    }
                })
                .collect::<Vec<_>>();
            members.sort_by_key(|member| member.address);
            Arc::new(members)
        }))
    }

    /// The validators announced at the checkpoint of `height`'s epoch.
    #[must_use]
    pub fn validators(&self, height: u64) -> Vec<BscValidatorV1> {
        self.members(self.roster_at(height))
            .iter()
            .map(|member| BscValidatorV1 {
                consensus_address: member.address.to_vec(),
                vote_public_key: member.public_key.to_vec(),
            })
            .collect()
    }

    fn build(&self, height: u64, parent_hash: [u8; 32]) -> SyntheticParliaHeaderV1 {
        let mut extra = vec![0_u8; 32];
        let roster = self.roster_at(height);
        let members = self.members(roster);
        if height.is_multiple_of(self.profile.epoch_length) {
            extra.push(u8::try_from(members.len()).expect("at most 64 validators"));
            for member in members.iter() {
                extra.extend_from_slice(&member.address);
                extra.extend_from_slice(&member.public_key);
            }
            extra.push(roster.turn_length);
        }
        extra.extend_from_slice(&[0_u8; 65]);
        let time_ms = self.time_ms(height);
        let mut mix = [0_u8; 32];
        mix[24..].copy_from_slice(&(time_ms % 1_000).to_be_bytes());
        let receipts_root = self
            .receipts_roots
            .get(&height)
            .copied()
            .unwrap_or_else(|| seeded(&[&self.seed, b"receipts", &height.to_be_bytes()]));
        let rlp = rlp_encode_list(&[
            rlp_encode_bytes(&parent_hash),
            rlp_encode_bytes(&[0x1d; 32]),
            rlp_encode_bytes(&members[0].address),
            rlp_encode_bytes(&seeded(&[&self.seed, b"state", &height.to_be_bytes()])),
            rlp_encode_bytes(&seeded(&[
                &self.seed,
                b"transactions",
                &height.to_be_bytes(),
            ])),
            rlp_encode_bytes(&receipts_root),
            rlp_encode_bytes(&[0_u8; 256]),
            rlp_encode_u64(2),
            rlp_encode_u64(height),
            rlp_encode_u64(100_000_000),
            rlp_encode_u64(21_000),
            rlp_encode_u64(time_ms / 1_000),
            rlp_encode_bytes(&extra),
            rlp_encode_bytes(&mix),
            rlp_encode_bytes(&[0_u8; 8]),
            rlp_encode_u64(0),
            rlp_encode_bytes(&EMPTY_TRIE_ROOT),
            rlp_encode_u64(0),
            rlp_encode_u64(0),
            rlp_encode_bytes(&[0_u8; 32]),
            rlp_encode_bytes(&[0xe3; 32]),
        ]);
        SyntheticParliaHeaderV1 {
            hash: keccak256(&[&rlp]),
            rlp,
            number: height,
            time_ms,
        }
    }

    /// The header at `height`.
    #[must_use]
    pub fn header(&self, height: u64) -> SyntheticParliaHeaderV1 {
        let mut cache = self.headers.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(header) = cache.get(&height) {
            return header.clone();
        }
        let (mut next, mut parent_hash) = match cache.range(..height).next_back() {
            Some((number, header)) => (number + 1, header.hash),
            None => (0, seeded(&[&self.seed, b"genesis-parent"])),
        };
        while next <= height {
            let header = self.build(next, parent_hash);
            parent_hash = header.hash;
            cache.insert(next, header);
            next += 1;
        }
        cache.get(&height).cloned().expect("built above")
    }

    /// An attestation `(source → source + 1)` signed by the first `signers` validators announced
    /// at `set_height`.
    #[must_use]
    pub fn attestation(&self, set_height: u64, signers: usize, source: u64) -> Vec<u8> {
        let members = self.members(self.roster_at(set_height));
        let signers = signers.min(members.len());
        let source_header = self.header(source);
        let target_header = self.header(source + 1);
        let message = vote_data_hash(source, source_header.hash, source + 1, target_header.hash);
        let secret = members[..signers]
            .iter()
            .fold(Scalar::from(0_u64), |sum, member| sum + member.secret);
        let bitmap = if signers == 64 {
            u64::MAX
        } else {
            (1_u64 << signers) - 1
        };
        encode_attestation(
            bitmap,
            &sign(&secret, &message),
            (source, source_header.hash),
            (source + 1, target_header.hash),
        )
    }

    /// A quorum-valid vote of the full set at `set_height` for another block at `source + 1`
    /// (a double vote).
    #[must_use]
    pub fn double_vote(&self, set_height: u64, source: u64) -> Vec<u8> {
        let members = self.members(self.roster_at(set_height));
        let source_hash = self.header(source).hash;
        let forged_target = seeded(&[&self.seed, b"forged-target", &source.to_be_bytes()]);
        let message = vote_data_hash(source, source_hash, source + 1, forged_target);
        let secret = members
            .iter()
            .fold(Scalar::from(0_u64), |sum, member| sum + member.secret);
        encode_attestation(
            (1_u64 << members.len()) - 1,
            &sign(&secret, &message),
            (source, source_hash),
            (source + 1, forged_target),
        )
    }

    /// An attestation naming every member of the set at `set_height` but signed by other keys.
    #[must_use]
    pub fn misattributed_attestation(&self, set_height: u64, source: u64) -> Vec<u8> {
        let members = self.members(self.roster_at(set_height));
        let source_hash = self.header(source).hash;
        let target_hash = self.header(source + 1).hash;
        let message = vote_data_hash(source, source_hash, source + 1, target_hash);
        let outsider = scalar(seeded(&[&self.seed, b"outsider"]));
        encode_attestation(
            (1_u64 << members.len()) - 1,
            &sign(&outsider, &message),
            (source, source_hash),
            (source + 1, target_hash),
        )
    }

    /// An advance step over headers `first..=last`, finalized by the full set announced at
    /// `set_height`.
    #[must_use]
    pub fn step(
        &self,
        first: u64,
        last: u64,
        set_height: u64,
        successor: Option<u64>,
    ) -> BscAdvanceStepV1 {
        BscAdvanceStepV1 {
            headers: (first..=last)
                .map(|height| self.header(height).rlp)
                .collect(),
            finality: BscFinalityV1 {
                set_id: set_height,
                successor_set_id: successor,
                attestation: self.attestation(set_height, usize::MAX, last),
            },
        }
    }

    /// The bootstrap frame of the checkpoint of `epoch`.
    #[must_use]
    pub fn bootstrap_data(&self, epoch: u64) -> BscLcBootstrapV1 {
        let length = self.profile.epoch_length;
        BscLcBootstrapV1 {
            checkpoint_header: self.header(epoch * length).rlp,
            previous_checkpoint_header: self.header((epoch - 1) * length).rlp,
        }
    }

    /// `InitializeLightClient` bootstrap of the checkpoint of `epoch`.
    #[must_use]
    pub fn bootstrap(&self, epoch: u64) -> SccpLcBootstrapV1 {
        SccpLcBootstrapDataV1::Bsc(self.bootstrap_data(epoch))
            .to_bootstrap()
            .expect("synthetic bootstraps encode")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headers_link_and_checkpoints_announce_their_roster() {
        let chain = SyntheticParliaChainV1::new([1; 32], 5, 2).with_transition(2, 1, 7, 3);
        let first = chain.header(1_999);
        let second = chain.header(2_000);
        assert_eq!(second.number, 2_000);
        assert_eq!(second.time_ms - first.time_ms, SYNTHETIC_PARLIA_INTERVAL_MS);
        assert_eq!(chain.validators(1_999).len(), 5);
        assert_eq!(chain.validators(2_000).len(), 7);
        assert!(second.rlp.len() > first.rlp.len());
        let modified = chain.with_receipts_root(1_999, [7; 32]);
        assert_ne!(modified.header(2_000).hash, second.hash);
        assert_eq!(modified.header(1_998).hash, chain.header(1_998).hash);
        assert!(!chain.attestation(0, 3, 10).is_empty());
    }
}
