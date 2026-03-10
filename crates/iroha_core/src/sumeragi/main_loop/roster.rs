//! Helpers for deriving active topologies and validator rosters.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use iroha_config::parameters::actual::ConsensusMode;
use iroha_crypto::blake2::{Blake2b512, Digest as BlakeDigest, digest::Update as BlakeUpdate};
use iroha_data_model::{
    ChainId, Encode as _, consensus::ConsensusKeyRole, nexus::PublicLaneValidatorStatus,
    peer::PeerId,
};
use iroha_logger::prelude::*;
use mv::storage::StorageReadOnly;

use crate::{
    state::{State, StateView, WorldReadOnly},
    sumeragi::{WsvEpochRosterAdapter, consensus::ValidatorIndex, epoch::EpochManager},
};

pub(super) fn canonicalize_roster(roster: Vec<PeerId>) -> Vec<PeerId> {
    // Sort to keep roster ordering deterministic across peers.
    let mut roster = dedup_preserving_order(roster);
    roster.sort();
    roster
}

pub(super) fn canonicalize_roster_for_mode(
    roster: Vec<PeerId>,
    consensus_mode: ConsensusMode,
) -> Vec<PeerId> {
    match consensus_mode {
        // Permissioned also needs deterministic ordering across peers; preserving local
        // trusted/commit order can diverge (e.g. self-first trusted peer lists).
        ConsensusMode::Permissioned => canonicalize_roster(roster),
        // NPoS roster hashes remain canonicalized for deterministic stake-quorum validation.
        ConsensusMode::Npos => canonicalize_roster(roster),
    }
}

#[allow(dead_code)]
pub(super) fn compute_roster_indices_from_state(
    state: &State,
    provider: Option<&Arc<WsvEpochRosterAdapter>>,
) -> (u64, usize, Vec<u32>) {
    let height = u64::try_from(state.committed_height()).unwrap_or(0);
    let commit_topology = state.commit_topology_snapshot();

    let roster_len = commit_topology.len();
    let indices = compute_roster_indices_from_topology(&commit_topology, provider);
    (height, roster_len, indices)
}

pub(super) fn compute_roster_indices_from_topology(
    topology: &[PeerId],
    provider: Option<&Arc<WsvEpochRosterAdapter>>,
) -> Vec<u32> {
    if topology.is_empty() {
        return Vec::new();
    }
    let fallback = || {
        (0..topology.len())
            .filter_map(|idx| u32::try_from(idx).ok())
            .collect::<Vec<_>>()
    };

    if let Some(provider) = provider {
        let mut positions: BTreeMap<PeerId, u32> = BTreeMap::new();
        for (idx, peer) in topology.iter().enumerate() {
            if let Ok(idx_u32) = u32::try_from(idx) {
                positions.insert(peer.clone(), idx_u32);
            } else {
                warn!(
                    roster_len = topology.len(),
                    idx, "validator roster index exceeds u32::MAX; omitting remaining peers"
                );
                return Vec::new();
            }
        }

        let mut missing = Vec::new();
        let mut indices = Vec::new();
        for peer in provider.peers() {
            if let Some(idx) = positions.get(peer) {
                indices.push(*idx);
            } else {
                missing.push(peer.clone());
            }
        }
        if !missing.is_empty() || indices.len() != topology.len() {
            warn!(
                missing = ?missing,
                provider_len = provider.peers().len(),
                topology_len = topology.len(),
                indices_len = indices.len(),
                "epoch roster provider incomplete for commit topology; falling back to full roster"
            );
            return fallback();
        }
        indices
    } else {
        fallback()
    }
}

pub(super) fn roster_member_allowed_bls(peer: &PeerId) -> bool {
    let pk = peer.public_key();
    if pk.algorithm() != iroha_crypto::Algorithm::BlsNormal {
        iroha_logger::warn!(
            ?pk,
            "excluding peer from active topology: validator identity must be BLS-normal"
        );
        return false;
    }
    true
}

fn roster_member_allowed(peer: &PeerId, pops: &BTreeMap<iroha_crypto::PublicKey, Vec<u8>>) -> bool {
    let pk = peer.public_key();
    if !roster_member_allowed_bls(peer) {
        return false;
    }
    let Some(pop) = pops.get(pk) else {
        iroha_logger::warn!(?pk, "excluding peer from active topology: missing PoP");
        return false;
    };
    if let Err(err) = iroha_crypto::bls_normal_pop_verify(pk, pop) {
        iroha_logger::warn!(
            ?pk,
            ?err,
            "excluding peer from active topology: invalid PoP"
        );
        return false;
    }
    true
}

fn dedup_preserving_order(roster: impl IntoIterator<Item = PeerId>) -> Vec<PeerId> {
    let mut seen = BTreeSet::new();
    let mut ordered = Vec::new();
    for peer in roster {
        if seen.insert(peer.clone()) {
            ordered.push(peer);
        }
    }
    ordered
}

fn filter_roster_with_pops(
    roster: impl IntoIterator<Item = PeerId>,
    pops: &BTreeMap<iroha_crypto::PublicKey, Vec<u8>>,
) -> Vec<PeerId> {
    dedup_preserving_order(
        roster
            .into_iter()
            .filter(|peer| roster_member_allowed(peer, pops)),
    )
}

fn filter_roster_bls(roster: impl IntoIterator<Item = PeerId>) -> Vec<PeerId> {
    dedup_preserving_order(roster.into_iter().filter(roster_member_allowed_bls))
}

fn missing_pops_for_roster(
    roster: &[PeerId],
    pops: &BTreeMap<iroha_crypto::PublicKey, Vec<u8>>,
) -> usize {
    roster
        .iter()
        .filter(|peer| !pops.contains_key(peer.public_key()))
        .count()
}

fn guard_pop_quorum(filtered: Vec<PeerId>, baseline: &[PeerId], pops_len: usize) -> Vec<PeerId> {
    if baseline.is_empty() {
        return Vec::new();
    }
    let baseline_len = baseline.len();
    let needed = if baseline_len > 3 {
        ((baseline_len.saturating_sub(1)) / 3) * 2 + 1
    } else {
        baseline_len
    };
    if filtered.len() < needed {
        warn!(
            filtered = filtered.len(),
            baseline = baseline_len,
            needed,
            pops = pops_len,
            "PoP filtering produced sub-quorum roster; falling back to BLS baseline"
        );
        return baseline.to_vec();
    }
    filtered
}

#[allow(clippy::if_not_else)]
pub(super) fn derive_active_topology_from_views(
    world: &impl WorldReadOnly,
    commit_topology: &[PeerId],
    trusted: &iroha_config::parameters::actual::TrustedPeers,
    _me: &PeerId,
) -> Vec<PeerId> {
    let world_peers = world.peers();
    let use_commit = !commit_topology.is_empty();
    // Trusted peers are the only source when both commit and world rosters are empty.
    let baseline_from_trusted = !use_commit && world_peers.is_empty();
    let mut baseline = if use_commit {
        commit_topology.to_vec()
    } else if !world_peers.is_empty() {
        world_peers.iter().cloned().collect()
    } else {
        crate::sumeragi::filter_validators_from_trusted(trusted)
    };
    if !use_commit {
        // Canonicalize unordered sources to avoid peer-order divergence before the first commit.
        baseline.sort();
    }
    let baseline = filter_roster_bls(baseline);
    let mut roster = if trusted.pops.is_empty() {
        baseline
    } else {
        let missing = missing_pops_for_roster(&baseline, &trusted.pops);
        if missing > 0 {
            if baseline_from_trusted {
                iroha_logger::warn!(
                    missing,
                    pops = trusted.pops.len(),
                    baseline = baseline.len(),
                    "PoP map incomplete for trusted roster; dropping peers without PoP"
                );
                filter_roster_with_pops(baseline.clone(), &trusted.pops)
            } else {
                iroha_logger::warn!(
                    missing,
                    pops = trusted.pops.len(),
                    baseline = baseline.len(),
                    "PoP map incomplete for active topology; skipping PoP filtering"
                );
                baseline
            }
        } else {
            let filtered = filter_roster_with_pops(baseline.clone(), &trusted.pops);
            if baseline_from_trusted {
                filtered
            } else {
                guard_pop_quorum(filtered, &baseline, trusted.pops.len())
            }
        }
    };

    // Keep ordering deterministic for both commit-derived and world/trusted-derived rosters.
    roster = canonicalize_roster(roster);
    if !roster.is_empty() {
        return roster;
    }

    iroha_logger::info!(
        world_peers = world.peers().len(),
        commit_topology_len = commit_topology.len(),
        "commit topology fallback to trusted peers"
    );

    let fallback = if trusted.pops.is_empty() {
        filter_roster_bls(crate::sumeragi::filter_validators_from_trusted(trusted))
    } else {
        let baseline = filter_roster_bls(crate::sumeragi::filter_validators_from_trusted(trusted));
        let missing = missing_pops_for_roster(&baseline, &trusted.pops);
        if missing > 0 {
            iroha_logger::warn!(
                missing,
                pops = trusted.pops.len(),
                baseline = baseline.len(),
                "PoP map incomplete for trusted roster fallback; dropping peers without PoP"
            );
        }
        filter_roster_with_pops(baseline, &trusted.pops)
    };
    canonicalize_roster(fallback)
}

fn roster_member_has_live_consensus_key(
    world: &impl WorldReadOnly,
    peer: &PeerId,
    height: u64,
    overlap_grace_blocks: u64,
    expiry_grace_blocks: u64,
) -> bool {
    let pk = peer.public_key();
    let pk_label = pk.to_string();
    let mut found_index_record = false;
    if let Some(ids) = world.consensus_keys_by_pk().get(&pk_label) {
        for id in ids {
            if let Some(record) = world.consensus_keys().get(id) {
                found_index_record = true;
                if record.id.role == ConsensusKeyRole::Validator
                    && record.is_live_at(height, overlap_grace_blocks, expiry_grace_blocks)
                {
                    return true;
                }
            }
        }
        if found_index_record {
            return false;
        }
    }
    world.consensus_keys().iter().any(|(id, record)| {
        id.role == ConsensusKeyRole::Validator
            && record.public_key == *pk
            && record.is_live_at(height, overlap_grace_blocks, expiry_grace_blocks)
    })
}

pub(super) fn filter_roster_with_live_consensus_keys_at_height_world(
    world: &impl WorldReadOnly,
    roster: Vec<PeerId>,
    height: u64,
) -> Vec<PeerId> {
    if world.consensus_keys().is_empty() {
        return roster;
    }
    let params = world.parameters();
    let sumeragi = params.sumeragi();
    let overlap_grace_blocks = sumeragi.key_overlap_grace_blocks;
    let expiry_grace_blocks = sumeragi.key_expiry_grace_blocks;
    roster
        .into_iter()
        .filter(|peer| {
            roster_member_has_live_consensus_key(
                world,
                peer,
                height,
                overlap_grace_blocks,
                expiry_grace_blocks,
            )
        })
        .collect()
}

#[cfg(test)]
pub(super) fn filter_roster_with_live_consensus_keys_at_height(
    view: &StateView<'_>,
    roster: Vec<PeerId>,
    height: u64,
) -> Vec<PeerId> {
    filter_roster_with_live_consensus_keys_at_height_world(view.world(), roster, height)
}

#[cfg(test)]
fn filter_roster_with_live_consensus_keys(
    view: &StateView<'_>,
    roster: Vec<PeerId>,
) -> Vec<PeerId> {
    let next_height = u64::try_from(view.height())
        .unwrap_or(u64::MAX)
        .saturating_add(1);
    filter_roster_with_live_consensus_keys_at_height(view, roster, next_height)
}

#[allow(clippy::if_not_else)]
#[cfg(test)]
pub(super) fn derive_active_topology(
    view: &StateView<'_>,
    trusted: &iroha_config::parameters::actual::TrustedPeers,
    me: &PeerId,
) -> Vec<PeerId> {
    let roster = derive_active_topology_from_views(
        view.world(),
        view.commit_topology().as_slice(),
        trusted,
        me,
    );
    filter_roster_with_live_consensus_keys(view, roster)
}

/// Mode-aware roster selection with `NPoS` staking bootstrap.
pub(super) fn derive_active_topology_for_mode(
    view: &StateView<'_>,
    trusted: &iroha_config::parameters::actual::TrustedPeers,
    me: &PeerId,
    consensus_mode: ConsensusMode,
) -> Vec<PeerId> {
    let height = u64::try_from(view.height()).unwrap_or(u64::MAX);
    derive_active_topology_for_mode_from_world(
        view.world(),
        view.commit_topology().as_slice(),
        height,
        trusted,
        me,
        consensus_mode,
    )
}

/// Mode-aware roster selection with `NPoS` staking bootstrap from world snapshots.
pub(super) fn derive_active_topology_for_mode_from_world(
    world: &impl WorldReadOnly,
    commit_topology: &[PeerId],
    height: u64,
    trusted: &iroha_config::parameters::actual::TrustedPeers,
    me: &PeerId,
    consensus_mode: ConsensusMode,
) -> Vec<PeerId> {
    let use_commit = !commit_topology.is_empty();
    let next_height = height.saturating_add(1);
    if matches!(consensus_mode, ConsensusMode::Npos) {
        let active_roster = active_validator_roster_from_world(world);
        if !active_roster.is_empty() {
            let mut roster = if use_commit {
                let commit_set: BTreeSet<_> = commit_topology.iter().cloned().collect();
                let constrained: Vec<_> = active_roster
                    .iter()
                    .filter(|peer| commit_set.contains(*peer))
                    .cloned()
                    .collect();
                if constrained.is_empty() {
                    iroha_logger::warn!(
                        commit_topology_len = commit_topology.len(),
                        active_roster_len = active_roster.len(),
                        "commit topology has no active validators; using active roster for NPoS"
                    );
                    active_roster.clone()
                } else {
                    constrained
                }
            } else {
                active_roster.clone()
            };
            roster = if trusted.pops.is_empty() {
                roster
            } else {
                let missing = missing_pops_for_roster(&roster, &trusted.pops);
                if missing > 0 {
                    iroha_logger::warn!(
                        missing,
                        pops = trusted.pops.len(),
                        baseline = roster.len(),
                        "PoP map incomplete for NPoS validator roster; skipping PoP filtering"
                    );
                    roster
                } else {
                    let filtered = filter_roster_with_pops(roster.clone(), &trusted.pops);
                    guard_pop_quorum(filtered, &roster, trusted.pops.len())
                }
            };
            // NPoS needs a canonical roster order so signer indices stay consistent across peers.
            roster = canonicalize_roster(roster);
            if !roster.is_empty() {
                return filter_roster_with_live_consensus_keys_at_height_world(
                    world,
                    roster,
                    next_height,
                );
            }
        }
    }
    let roster = derive_active_topology_from_views(world, commit_topology, trusted, me);
    filter_roster_with_live_consensus_keys_at_height_world(world, roster, next_height)
}

#[cfg(test)]
pub(super) fn derive_local_validator_index(
    view: &StateView<'_>,
    trusted: &iroha_config::parameters::actual::TrustedPeers,
    me: &PeerId,
) -> Option<ValidatorIndex> {
    let roster = derive_active_topology(view, trusted, me);
    roster.iter().position(|peer| peer == me).and_then(|idx| {
        u32::try_from(idx)
            .inspect_err(|err| warn!(?idx, ?err, "local validator index exceeds u32 range"))
            .ok()
    })
}

pub(super) fn derive_local_validator_index_for_mode(
    view: &StateView<'_>,
    trusted: &iroha_config::parameters::actual::TrustedPeers,
    me: &PeerId,
    consensus_mode: ConsensusMode,
) -> Option<ValidatorIndex> {
    let roster = derive_active_topology_for_mode(view, trusted, me, consensus_mode);
    roster.iter().position(|peer| peer == me).and_then(|idx| {
        u32::try_from(idx)
            .inspect_err(|err| warn!(?idx, ?err, "local validator index exceeds u32 range"))
            .ok()
    })
}

pub(super) fn derive_local_validator_index_for_mode_from_world(
    world: &impl WorldReadOnly,
    commit_topology: &[PeerId],
    height: u64,
    trusted: &iroha_config::parameters::actual::TrustedPeers,
    me: &PeerId,
    consensus_mode: ConsensusMode,
) -> Option<ValidatorIndex> {
    let roster = derive_active_topology_for_mode_from_world(
        world,
        commit_topology,
        height,
        trusted,
        me,
        consensus_mode,
    );
    roster.iter().position(|peer| peer == me).and_then(|idx| {
        u32::try_from(idx)
            .inspect_err(|err| warn!(?idx, ?err, "local validator index exceeds u32 range"))
            .ok()
    })
}

fn active_validator_roster_from_world(world: &impl WorldReadOnly) -> Vec<PeerId> {
    let mut roster = BTreeSet::new();
    for ((_lane_id, validator_id), record) in world.public_lane_validators().iter() {
        if !matches!(record.status, PublicLaneValidatorStatus::Active) {
            continue;
        }
        let Some(pk) = validator_id.try_signatory() else {
            continue;
        };
        let peer_id = PeerId::from(pk.clone());
        if !roster_member_allowed_bls(&peer_id) {
            continue;
        }
        roster.insert(peer_id);
    }
    roster.into_iter().collect()
}

pub(super) fn compute_membership_view_hash(
    chain_id: &ChainId,
    height: u64,
    view: u64,
    epoch: u64,
    peers: &[PeerId],
) -> [u8; 32] {
    let mut hasher = Blake2b512::new();
    BlakeUpdate::update(&mut hasher, chain_id.clone().into_inner().as_bytes());
    BlakeUpdate::update(&mut hasher, &height.to_be_bytes());
    BlakeUpdate::update(&mut hasher, &view.to_be_bytes());
    BlakeUpdate::update(&mut hasher, &epoch.to_be_bytes());
    for peer in peers {
        let encoded = peer.encode();
        BlakeUpdate::update(&mut hasher, &encoded);
    }
    let digest = BlakeDigest::finalize(hasher);
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest[..32]);
    out
}

pub(super) fn apply_roster_indices_to_manager(
    manager: &mut EpochManager,
    roster_len: usize,
    roster_indices: Vec<u32>,
) {
    if roster_indices.is_empty() {
        if let Ok(len_u32) = u32::try_from(roster_len) {
            manager.set_validator_roster_indices(0..len_u32);
        } else {
            warn!(
                roster_len,
                "validator roster exceeds u32::MAX; skipping roster snapshot"
            );
        }
    } else {
        manager.set_validator_roster_indices(roster_indices);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{CellVecExt, State, World},
    };
    use iroha_config::parameters::actual::ConsensusMode;
    use iroha_crypto::{Algorithm, KeyPair, bls_normal_pop_prove};
    use iroha_data_model::{
        account::AccountId,
        consensus::{ConsensusKeyRecord, ConsensusKeyStatus},
        metadata::Metadata,
        nexus::{LaneId, PublicLaneValidatorRecord, PublicLaneValidatorStatus},
        peer::Peer,
    };
    use iroha_primitives::numeric::Numeric;
    use iroha_primitives::unique_vec::UniqueVec;

    use super::*;

    fn make_peer(peer_id: PeerId, port: u16) -> Peer {
        Peer::new(format!("127.0.0.1:{port}").parse().expect("addr"), peer_id)
    }

    #[test]
    fn canonicalize_roster_sorts_and_dedups() {
        let first = PeerId::new(
            KeyPair::random_with_algorithm(Algorithm::BlsNormal)
                .public_key()
                .clone(),
        );
        let second = PeerId::new(
            KeyPair::random_with_algorithm(Algorithm::BlsNormal)
                .public_key()
                .clone(),
        );

        let roster = vec![first.clone(), second.clone(), first.clone()];
        let canonical = canonicalize_roster(roster);

        let mut expected = vec![first, second];
        expected.sort();
        assert_eq!(canonical, expected);
    }

    #[test]
    fn canonicalize_roster_for_permissioned_sorts_for_determinism() {
        let first = PeerId::new(
            KeyPair::from_seed(b"roster-a".to_vec(), Algorithm::BlsNormal)
                .public_key()
                .clone(),
        );
        let second = PeerId::new(
            KeyPair::from_seed(b"roster-b".to_vec(), Algorithm::BlsNormal)
                .public_key()
                .clone(),
        );

        let roster = vec![second.clone(), first.clone()];
        let canonical = canonicalize_roster_for_mode(roster, ConsensusMode::Permissioned);

        let mut expected = vec![second, first];
        expected.sort();
        assert_eq!(canonical, expected);
    }

    #[test]
    fn roster_indices_fall_back_when_provider_incomplete() {
        let peer_a = PeerId::new(
            KeyPair::random_with_algorithm(Algorithm::BlsNormal)
                .public_key()
                .clone(),
        );
        let peer_b = PeerId::new(
            KeyPair::random_with_algorithm(Algorithm::BlsNormal)
                .public_key()
                .clone(),
        );
        let peer_c = PeerId::new(
            KeyPair::random_with_algorithm(Algorithm::BlsNormal)
                .public_key()
                .clone(),
        );
        let topology = vec![peer_a.clone(), peer_b.clone(), peer_c.clone()];
        let provider = Arc::new(WsvEpochRosterAdapter::from_peer_iter([peer_a, peer_b]));

        let indices = compute_roster_indices_from_topology(&topology, Some(&provider));
        assert_eq!(indices, vec![0, 1, 2]);
    }

    #[test]
    fn pop_filter_falls_back_to_baseline_when_subquorum() {
        let keypairs: Vec<KeyPair> = (0..4)
            .map(|_| KeyPair::random_with_algorithm(Algorithm::BlsNormal))
            .collect();
        let peers: Vec<PeerId> = keypairs
            .iter()
            .map(|kp| PeerId::new(kp.public_key().clone()))
            .collect();
        let mut pops = BTreeMap::new();
        for kp in keypairs.iter().take(2) {
            let pop = bls_normal_pop_prove(kp.private_key()).expect("pop");
            pops.insert(kp.public_key().clone(), pop);
        }

        let filtered = filter_roster_with_pops(peers.clone(), &pops);
        let guarded = guard_pop_quorum(filtered.clone(), &peers, pops.len());

        assert_eq!(guarded, peers);
    }

    #[test]
    fn active_topology_skips_incomplete_pops() {
        let keypairs: Vec<KeyPair> = (0..4)
            .map(|_| KeyPair::random_with_algorithm(Algorithm::BlsNormal))
            .collect();
        let peers: Vec<PeerId> = keypairs
            .iter()
            .map(|kp| PeerId::new(kp.public_key().clone()))
            .collect();

        let world = World::new();
        {
            let mut block = world.block();
            let peers_cell = block.peers.get_mut();
            for peer in &peers {
                let _ = peers_cell.push(peer.clone());
            }
            block.commit();
        }

        let mut pops = BTreeMap::new();
        for kp in keypairs.iter().take(3) {
            let pop = bls_normal_pop_prove(kp.private_key()).expect("pop");
            pops.insert(kp.public_key().clone(), pop);
        }

        let trusted = iroha_config::parameters::actual::TrustedPeers {
            myself: make_peer(peers[0].clone(), 10_000),
            others: peers
                .iter()
                .skip(1)
                .enumerate()
                .map(|(idx, peer_id)| {
                    let port = 10_001 + u16::try_from(idx).expect("peer index fits u16");
                    make_peer(peer_id.clone(), port)
                })
                .collect::<UniqueVec<_>>(),
            pops,
        };

        let kura = Kura::blank_kura_for_testing();
        let state = State::new_for_testing(world, kura, LiveQueryStore::start_test());
        let view = state.view();
        let roster = derive_active_topology(&view, &trusted, &peers[0]);

        let mut expected = peers.clone();
        expected.sort();
        assert_eq!(roster, expected);
    }

    #[test]
    fn active_topology_sorts_world_peers_when_commit_topology_empty() {
        let mut peers: Vec<PeerId> = (0..4)
            .map(|_| {
                PeerId::new(
                    KeyPair::random_with_algorithm(Algorithm::BlsNormal)
                        .public_key()
                        .clone(),
                )
            })
            .collect();
        peers.sort();

        let mut reversed = peers.clone();
        reversed.reverse();

        let world = World::new();
        {
            let mut block = world.block();
            let peers_cell = block.peers.get_mut();
            for peer in reversed {
                let _ = peers_cell.push(peer);
            }
            block.commit();
        }

        let kura = Kura::blank_kura_for_testing();
        let state = State::new_for_testing(world, kura, LiveQueryStore::start_test());
        let trusted = iroha_config::parameters::actual::TrustedPeers {
            myself: Peer::new("127.0.0.1:10000".parse().expect("addr"), peers[0].clone()),
            others: UniqueVec::new(),
            pops: BTreeMap::new(),
        };

        let view = state.view();
        assert!(view.commit_topology().is_empty());
        let roster = derive_active_topology(&view, &trusted, &peers[0]);

        assert_eq!(roster, peers);
    }

    #[test]
    fn active_topology_filters_inactive_consensus_keys() {
        let keypairs: Vec<KeyPair> = (0..3)
            .map(|_| KeyPair::random_with_algorithm(Algorithm::BlsNormal))
            .collect();
        let peers: Vec<PeerId> = keypairs
            .iter()
            .map(|kp| PeerId::new(kp.public_key().clone()))
            .collect();

        let world = World::new();
        {
            let mut block = world.block();
            let peers_cell = block.peers.get_mut();
            for peer in &peers {
                let _ = peers_cell.push(peer.clone());
            }
            for (idx, kp) in keypairs.iter().enumerate() {
                let id = crate::state::derive_validator_key_id(kp.public_key());
                let status = if idx == 2 {
                    ConsensusKeyStatus::Disabled
                } else {
                    ConsensusKeyStatus::Active
                };
                let record = ConsensusKeyRecord {
                    id: id.clone(),
                    public_key: kp.public_key().clone(),
                    pop: None,
                    activation_height: 1,
                    expiry_height: None,
                    hsm: None,
                    replaces: None,
                    status,
                };
                block.consensus_keys.insert(id.clone(), record);
                let pk_label = kp.public_key().to_string();
                let mut by_pk = block
                    .consensus_keys_by_pk
                    .get(&pk_label)
                    .cloned()
                    .unwrap_or_default();
                by_pk.push(id);
                block.consensus_keys_by_pk.insert(pk_label, by_pk);
            }
            block.commit();
        }

        let kura = Kura::blank_kura_for_testing();
        let mut state = State::new_for_testing(world, kura, LiveQueryStore::start_test());
        state.commit_topology.mutate_vec(|vec| *vec = peers.clone());

        let trusted = iroha_config::parameters::actual::TrustedPeers {
            myself: Peer::new("127.0.0.1:10000".parse().expect("addr"), peers[0].clone()),
            others: peers
                .iter()
                .skip(1)
                .enumerate()
                .map(|(idx, peer_id)| {
                    let port = 10_001 + u16::try_from(idx).expect("peer index fits u16");
                    make_peer(peer_id.clone(), port)
                })
                .collect::<UniqueVec<_>>(),
            pops: BTreeMap::new(),
        };

        let view = state.view();
        let roster = derive_active_topology_for_mode(
            &view,
            &trusted,
            &peers[0],
            ConsensusMode::Permissioned,
        );

        let mut expected = peers[..2].to_vec();
        expected.sort();
        assert_eq!(roster, expected);
    }

    #[test]
    fn active_topology_for_mode_from_world_matches_view_path() {
        let keypairs: Vec<KeyPair> = (0..3)
            .map(|_| KeyPair::random_with_algorithm(Algorithm::BlsNormal))
            .collect();
        let peers: Vec<PeerId> = keypairs
            .iter()
            .map(|kp| PeerId::new(kp.public_key().clone()))
            .collect();

        let world = World::new();
        {
            let mut block = world.block();
            let peers_cell = block.peers.get_mut();
            for peer in &peers {
                let _ = peers_cell.push(peer.clone());
            }
            block.commit();
        }

        let kura = Kura::blank_kura_for_testing();
        let mut state = State::new_for_testing(world, kura, LiveQueryStore::start_test());
        state.commit_topology.mutate_vec(|vec| *vec = peers.clone());
        let trusted = iroha_config::parameters::actual::TrustedPeers {
            myself: Peer::new("127.0.0.1:10000".parse().expect("addr"), peers[0].clone()),
            others: peers
                .iter()
                .skip(1)
                .enumerate()
                .map(|(idx, peer_id)| {
                    let port = 10_001 + u16::try_from(idx).expect("peer index fits u16");
                    make_peer(peer_id.clone(), port)
                })
                .collect::<UniqueVec<_>>(),
            pops: BTreeMap::new(),
        };
        let mode = ConsensusMode::Permissioned;
        let expected = {
            let view = state.view();
            derive_active_topology_for_mode(&view, &trusted, &peers[0], mode)
        };
        let world = state.world_view();
        let commit_topology = state.commit_topology_snapshot();
        let height = u64::try_from(state.committed_height()).unwrap_or(u64::MAX);
        let actual = derive_active_topology_for_mode_from_world(
            &world,
            commit_topology.as_slice(),
            height,
            &trusted,
            &peers[0],
            mode,
        );
        assert_eq!(actual, expected);
    }

    #[test]
    fn active_topology_for_npos_prefers_active_validators() {
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(World::default(), kura, query);

        let keypair_active = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let keypair_pending = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let account_active = AccountId::new(keypair_active.public_key().clone());
        let account_pending = AccountId::new(keypair_pending.public_key().clone());
        let peer_active = PeerId::new(keypair_active.public_key().clone());

        {
            let mut block = state.world.public_lane_validators.block();
            block.insert(
                (LaneId::new(1), account_active.clone()),
                PublicLaneValidatorRecord {
                    lane_id: LaneId::new(1),
                    validator: account_active.clone(),
                    stake_account: account_active,
                    total_stake: Numeric::new(10, 0),
                    self_stake: Numeric::new(10, 0),
                    metadata: Metadata::default(),
                    status: PublicLaneValidatorStatus::Active,
                    activation_epoch: None,
                    activation_height: None,
                    last_reward_epoch: None,
                },
            );
            block.insert(
                (LaneId::new(2), account_pending.clone()),
                PublicLaneValidatorRecord {
                    lane_id: LaneId::new(2),
                    validator: account_pending.clone(),
                    stake_account: account_pending,
                    total_stake: Numeric::new(15, 0),
                    self_stake: Numeric::new(15, 0),
                    metadata: Metadata::default(),
                    status: PublicLaneValidatorStatus::PendingActivation(3),
                    activation_epoch: None,
                    activation_height: None,
                    last_reward_epoch: None,
                },
            );
            block.commit();
        }

        let trusted = iroha_config::parameters::actual::TrustedPeers {
            myself: Peer::new(
                "127.0.0.1:10000".parse().expect("addr"),
                peer_active.clone(),
            ),
            others: UniqueVec::new(),
            pops: BTreeMap::new(),
        };
        let view = state.view();
        let roster =
            derive_active_topology_for_mode(&view, &trusted, &peer_active, ConsensusMode::Npos);

        assert_eq!(roster, vec![peer_active]);
    }

    #[test]
    fn active_topology_for_npos_skips_incomplete_pops() {
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(World::default(), kura, query);

        let keypairs: Vec<KeyPair> = (0..3)
            .map(|_| KeyPair::random_with_algorithm(Algorithm::BlsNormal))
            .collect();
        let accounts: Vec<AccountId> = keypairs
            .iter()
            .map(|kp| AccountId::new(kp.public_key().clone()))
            .collect();
        let peers: Vec<PeerId> = keypairs
            .iter()
            .map(|kp| PeerId::new(kp.public_key().clone()))
            .collect();

        {
            let mut block = state.world.public_lane_validators.block();
            for (idx, account) in accounts.iter().enumerate() {
                let lane_id = LaneId::new(u32::try_from(idx).expect("lane index fits u32") + 1);
                block.insert(
                    (lane_id, account.clone()),
                    PublicLaneValidatorRecord {
                        lane_id,
                        validator: account.clone(),
                        stake_account: account.clone(),
                        total_stake: Numeric::new(10, 0),
                        self_stake: Numeric::new(10, 0),
                        metadata: Metadata::default(),
                        status: PublicLaneValidatorStatus::Active,
                        activation_epoch: None,
                        activation_height: None,
                        last_reward_epoch: None,
                    },
                );
            }
            block.commit();
        }

        let mut pops = BTreeMap::new();
        for kp in keypairs.iter().take(2) {
            let pop = bls_normal_pop_prove(kp.private_key()).expect("pop");
            pops.insert(kp.public_key().clone(), pop);
        }

        let trusted = iroha_config::parameters::actual::TrustedPeers {
            myself: make_peer(peers[0].clone(), 11_000),
            others: peers
                .iter()
                .skip(1)
                .enumerate()
                .map(|(idx, peer_id)| {
                    let port = 11_001 + u16::try_from(idx).expect("peer index fits u16");
                    make_peer(peer_id.clone(), port)
                })
                .collect::<UniqueVec<_>>(),
            pops,
        };

        let view = state.view();
        let roster =
            derive_active_topology_for_mode(&view, &trusted, &peers[0], ConsensusMode::Npos);

        let mut expected = peers.clone();
        expected.sort();
        assert_eq!(roster, expected);
    }

    #[test]
    fn active_topology_for_npos_filters_inactive_commit_topology() {
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(World::default(), kura, query);

        let keypair_active = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let keypair_inactive = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let account_active = AccountId::new(keypair_active.public_key().clone());
        let account_inactive = AccountId::new(keypair_inactive.public_key().clone());
        let peer_active = PeerId::new(keypair_active.public_key().clone());
        let peer_inactive = PeerId::new(keypair_inactive.public_key().clone());

        {
            let mut block = state.world.public_lane_validators.block();
            block.insert(
                (LaneId::new(1), account_active.clone()),
                PublicLaneValidatorRecord {
                    lane_id: LaneId::new(1),
                    validator: account_active.clone(),
                    stake_account: account_active,
                    total_stake: Numeric::new(10, 0),
                    self_stake: Numeric::new(10, 0),
                    metadata: Metadata::default(),
                    status: PublicLaneValidatorStatus::Active,
                    activation_epoch: None,
                    activation_height: None,
                    last_reward_epoch: None,
                },
            );
            block.insert(
                (LaneId::new(1), account_inactive.clone()),
                PublicLaneValidatorRecord {
                    lane_id: LaneId::new(1),
                    validator: account_inactive.clone(),
                    stake_account: account_inactive,
                    total_stake: Numeric::new(15, 0),
                    self_stake: Numeric::new(15, 0),
                    metadata: Metadata::default(),
                    status: PublicLaneValidatorStatus::PendingActivation(1),
                    activation_epoch: None,
                    activation_height: None,
                    last_reward_epoch: None,
                },
            );
            block.commit();
        }
        {
            let mut block = state.commit_topology.block();
            let mut tx = block.transaction();
            *tx = vec![peer_inactive.clone(), peer_active.clone()];
            tx.apply();
            block.commit();
        }

        let trusted = iroha_config::parameters::actual::TrustedPeers {
            myself: Peer::new(
                "127.0.0.1:10000".parse().expect("addr"),
                peer_active.clone(),
            ),
            others: UniqueVec::new(),
            pops: BTreeMap::new(),
        };
        let view = state.view();
        let roster =
            derive_active_topology_for_mode(&view, &trusted, &peer_active, ConsensusMode::Npos);

        assert_eq!(roster, vec![peer_active]);
    }

    #[test]
    fn active_topology_for_npos_canonicalizes_commit_topology_order() {
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(World::default(), kura, query);

        let keypair_a = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let keypair_b = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let keypair_c = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let account_a = AccountId::new(keypair_a.public_key().clone());
        let account_b = AccountId::new(keypair_b.public_key().clone());
        let account_c = AccountId::new(keypair_c.public_key().clone());
        let peer_a = PeerId::new(keypair_a.public_key().clone());
        let peer_b = PeerId::new(keypair_b.public_key().clone());

        {
            let mut block = state.world.public_lane_validators.block();
            for (account, stake) in [
                (account_a.clone(), Numeric::new(10, 0)),
                (account_b.clone(), Numeric::new(12, 0)),
                (account_c.clone(), Numeric::new(14, 0)),
            ] {
                block.insert(
                    (LaneId::new(1), account.clone()),
                    PublicLaneValidatorRecord {
                        lane_id: LaneId::new(1),
                        validator: account.clone(),
                        stake_account: account,
                        total_stake: stake.clone(),
                        self_stake: stake,
                        metadata: Metadata::default(),
                        status: PublicLaneValidatorStatus::Active,
                        activation_epoch: None,
                        activation_height: None,
                        last_reward_epoch: None,
                    },
                );
            }
            block.commit();
        }
        {
            let mut block = state.commit_topology.block();
            let mut tx = block.transaction();
            *tx = vec![peer_b.clone(), peer_a.clone()];
            tx.apply();
            block.commit();
        }

        let trusted = iroha_config::parameters::actual::TrustedPeers {
            myself: Peer::new("127.0.0.1:10000".parse().expect("addr"), peer_a.clone()),
            others: UniqueVec::new(),
            pops: BTreeMap::new(),
        };
        let view = state.view();
        let roster = derive_active_topology_for_mode(&view, &trusted, &peer_a, ConsensusMode::Npos);

        let mut expected = vec![peer_a, peer_b];
        expected.sort();
        assert_eq!(roster, expected);
    }

    #[test]
    fn active_topology_for_npos_falls_back_when_commit_topology_has_no_active_members() {
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(World::default(), kura, query);

        let keypair_active_a = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let keypair_active_b = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let keypair_pending = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let account_active_a = AccountId::new(keypair_active_a.public_key().clone());
        let account_active_b = AccountId::new(keypair_active_b.public_key().clone());
        let account_pending = AccountId::new(keypair_pending.public_key().clone());
        let peer_active_a = PeerId::new(keypair_active_a.public_key().clone());
        let peer_active_b = PeerId::new(keypair_active_b.public_key().clone());
        let peer_pending = PeerId::new(keypair_pending.public_key().clone());

        {
            let mut block = state.world.public_lane_validators.block();
            for account in [account_active_a.clone(), account_active_b.clone()] {
                block.insert(
                    (LaneId::new(1), account.clone()),
                    PublicLaneValidatorRecord {
                        lane_id: LaneId::new(1),
                        validator: account.clone(),
                        stake_account: account,
                        total_stake: Numeric::new(10, 0),
                        self_stake: Numeric::new(10, 0),
                        metadata: Metadata::default(),
                        status: PublicLaneValidatorStatus::Active,
                        activation_epoch: None,
                        activation_height: None,
                        last_reward_epoch: None,
                    },
                );
            }
            block.insert(
                (LaneId::new(1), account_pending.clone()),
                PublicLaneValidatorRecord {
                    lane_id: LaneId::new(1),
                    validator: account_pending.clone(),
                    stake_account: account_pending,
                    total_stake: Numeric::new(10, 0),
                    self_stake: Numeric::new(10, 0),
                    metadata: Metadata::default(),
                    status: PublicLaneValidatorStatus::PendingActivation(7),
                    activation_epoch: None,
                    activation_height: None,
                    last_reward_epoch: None,
                },
            );
            block.commit();
        }
        {
            let mut block = state.commit_topology.block();
            let mut tx = block.transaction();
            *tx = vec![peer_pending.clone()];
            tx.apply();
            block.commit();
        }

        let trusted = iroha_config::parameters::actual::TrustedPeers {
            myself: Peer::new(
                "127.0.0.1:10000".parse().expect("addr"),
                peer_active_a.clone(),
            ),
            others: UniqueVec::new(),
            pops: BTreeMap::new(),
        };
        let view = state.view();
        let roster =
            derive_active_topology_for_mode(&view, &trusted, &peer_active_a, ConsensusMode::Npos);

        let mut expected = vec![peer_active_a, peer_active_b];
        expected.sort();
        assert_eq!(roster, expected);
    }

    #[test]
    fn local_validator_index_for_mode_uses_active_validator_roster() {
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(World::default(), kura, query);

        let keypair_active = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let account_active = AccountId::new(keypair_active.public_key().clone());
        let peer_active = PeerId::new(keypair_active.public_key().clone());

        {
            let mut block = state.world.public_lane_validators.block();
            block.insert(
                (LaneId::new(1), account_active.clone()),
                PublicLaneValidatorRecord {
                    lane_id: LaneId::new(1),
                    validator: account_active.clone(),
                    stake_account: account_active,
                    total_stake: Numeric::new(8, 0),
                    self_stake: Numeric::new(8, 0),
                    metadata: Metadata::default(),
                    status: PublicLaneValidatorStatus::Active,
                    activation_epoch: None,
                    activation_height: None,
                    last_reward_epoch: None,
                },
            );
            block.commit();
        }

        let trusted = iroha_config::parameters::actual::TrustedPeers {
            myself: Peer::new(
                "127.0.0.1:10000".parse().expect("addr"),
                peer_active.clone(),
            ),
            others: UniqueVec::new(),
            pops: BTreeMap::new(),
        };
        let view = state.view();
        let idx = derive_local_validator_index_for_mode(
            &view,
            &trusted,
            &peer_active,
            ConsensusMode::Npos,
        );

        assert_eq!(idx, ValidatorIndex::try_from(0).ok());
    }

    #[test]
    fn local_validator_index_for_mode_from_world_matches_view_path() {
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(World::default(), kura, query);

        let keypair_active = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let account_active = AccountId::new(keypair_active.public_key().clone());
        let peer_active = PeerId::new(keypair_active.public_key().clone());

        {
            let mut block = state.world.public_lane_validators.block();
            block.insert(
                (LaneId::new(1), account_active.clone()),
                PublicLaneValidatorRecord {
                    lane_id: LaneId::new(1),
                    validator: account_active.clone(),
                    stake_account: account_active,
                    total_stake: Numeric::new(8, 0),
                    self_stake: Numeric::new(8, 0),
                    metadata: Metadata::default(),
                    status: PublicLaneValidatorStatus::Active,
                    activation_epoch: None,
                    activation_height: None,
                    last_reward_epoch: None,
                },
            );
            block.commit();
        }

        let trusted = iroha_config::parameters::actual::TrustedPeers {
            myself: Peer::new(
                "127.0.0.1:10000".parse().expect("addr"),
                peer_active.clone(),
            ),
            others: UniqueVec::new(),
            pops: BTreeMap::new(),
        };
        let mode = ConsensusMode::Npos;
        let idx_from_view = {
            let view = state.view();
            derive_local_validator_index_for_mode(&view, &trusted, &peer_active, mode)
        };
        let world_view = state.world_view();
        let commit_topology = state.commit_topology_snapshot();
        let height = u64::try_from(state.committed_height()).unwrap_or(u64::MAX);
        let idx_from_world = derive_local_validator_index_for_mode_from_world(
            &world_view,
            commit_topology.as_slice(),
            height,
            &trusted,
            &peer_active,
            mode,
        );

        assert_eq!(idx_from_world, idx_from_view);
    }

    #[test]
    fn active_topology_from_views_matches_state_view() {
        let peers: Vec<PeerId> = (0..3)
            .map(|_| {
                PeerId::new(
                    KeyPair::random_with_algorithm(Algorithm::BlsNormal)
                        .public_key()
                        .clone(),
                )
            })
            .collect();
        let world = World::new();
        {
            let mut block = world.block();
            let peers_cell = block.peers.get_mut();
            for peer in &peers {
                let _ = peers_cell.push(peer.clone());
            }
            block.commit();
        }

        let kura = Kura::blank_kura_for_testing();
        let state = State::new_for_testing(world, kura, LiveQueryStore::start_test());
        let trusted = iroha_config::parameters::actual::TrustedPeers {
            myself: Peer::new("127.0.0.1:10000".parse().expect("addr"), peers[0].clone()),
            others: UniqueVec::new(),
            pops: BTreeMap::new(),
        };

        let view = state.view();
        let world_view = state.world.view();
        let commit_topology = state.commit_topology.view();
        let from_view = derive_active_topology(&view, &trusted, &peers[0]);
        let from_views = derive_active_topology_from_views(
            &world_view,
            commit_topology.as_slice(),
            &trusted,
            &peers[0],
        );

        assert_eq!(from_views, from_view);
    }
}
