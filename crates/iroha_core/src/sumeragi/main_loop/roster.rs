//! Helpers for deriving active topologies and validator rosters.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use iroha_crypto::blake2::{Blake2b512, Digest as BlakeDigest, digest::Update as BlakeUpdate};
use iroha_data_model::{ChainId, Encode as _, peer::PeerId};
use iroha_logger::prelude::*;

use crate::{
    state::{State, StateView, WorldReadOnly},
    sumeragi::{WsvEpochRosterAdapter, consensus::ValidatorIndex, epoch::EpochManager},
};

pub(super) fn canonicalize_roster(roster: Vec<PeerId>) -> Vec<PeerId> {
    // Preserve input ordering so topology rotation and config ordering remain stable.
    dedup_preserving_order(roster)
}

#[allow(dead_code)]
pub(super) fn compute_roster_indices_from_state(
    state: &State,
    provider: Option<&Arc<WsvEpochRosterAdapter>>,
) -> (u64, usize, Vec<u32>) {
    let view = state.view();
    let height = view.height() as u64;
    let commit_topology: Vec<PeerId> = view.commit_topology.iter().cloned().collect();
    drop(view);

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

fn guard_pop_quorum(
    filtered: Vec<PeerId>,
    baseline: Vec<PeerId>,
    pops: &BTreeMap<iroha_crypto::PublicKey, Vec<u8>>,
    me: &PeerId,
) -> Vec<PeerId> {
    if baseline.is_empty() {
        return baseline;
    }
    let baseline_has_me = baseline.iter().any(|peer| peer == me);
    let needed = if baseline.len() > 3 {
        ((baseline.len().saturating_sub(1)) / 3) * 2 + 1
    } else {
        baseline.len()
    };
    if filtered.len() >= needed {
        return filtered;
    }
    warn!(
        filtered = filtered.len(),
        baseline = baseline.len(),
        needed,
        pops = pops.len(),
        "PoP filtering produced sub-quorum roster; falling back to BLS roster to preserve quorum"
    );
    let mut roster = dedup_preserving_order(
        filtered.into_iter().chain(
            baseline
                .iter()
                .filter(|peer| roster_member_allowed(peer, pops))
                .cloned(),
        ),
    );
    if roster.len() < needed {
        iroha_logger::warn!(
            baseline = baseline.len(),
            filtered = roster.len(),
            needed,
            pops = pops.len(),
            "trusted PoP map incomplete; reintroducing BLS roster to preserve commit quorum"
        );
        roster = dedup_preserving_order(roster.into_iter().chain(baseline));
    }
    if baseline_has_me && roster_member_allowed(me, pops) && !roster.iter().any(|p| p == me) {
        roster.push(me.clone());
        roster = dedup_preserving_order(roster);
    }
    roster
}

#[allow(clippy::if_not_else)]
pub(super) fn derive_active_topology(
    view: &StateView<'_>,
    trusted: &iroha_config::parameters::actual::TrustedPeers,
    me: &PeerId,
) -> Vec<PeerId> {
    let world_peers = view.world.peers();
    let commit_topology = view.commit_topology();
    let use_commit = !commit_topology.is_empty();
    let mut roster = if trusted.pops.is_empty() {
        if use_commit {
            filter_roster_bls(commit_topology.iter().cloned())
        } else if !world_peers.is_empty() {
            filter_roster_bls(world_peers.iter().cloned())
        } else {
            filter_roster_bls(crate::sumeragi::filter_validators_from_trusted(trusted))
        }
    } else {
        let baseline = if use_commit {
            filter_roster_bls(commit_topology.iter().cloned())
        } else if !world_peers.is_empty() {
            filter_roster_bls(world_peers.iter().cloned())
        } else {
            filter_roster_bls(crate::sumeragi::filter_validators_from_trusted(trusted))
        };
        let filtered = filter_roster_with_pops(baseline.clone(), &trusted.pops);
        if filtered.len() < baseline.len() {
            iroha_logger::warn!(
                pops = trusted.pops.len(),
                baseline = baseline.len(),
                filtered = filtered.len(),
                "PoP map incomplete for active topology; preserving quorum with trusted roster fallback"
            );
        }
        guard_pop_quorum(filtered, baseline, &trusted.pops, me)
    };

    roster = canonicalize_roster(roster);
    if !roster.is_empty() {
        return roster;
    }

    iroha_logger::info!(
        world_peers = view.world.peers().len(),
        commit_topology_len = view.commit_topology().len(),
        "commit topology fallback to trusted peers"
    );

    let fallback = if trusted.pops.is_empty() {
        filter_roster_bls(crate::sumeragi::filter_validators_from_trusted(trusted))
    } else {
        let baseline = filter_roster_bls(crate::sumeragi::filter_validators_from_trusted(trusted));
        let filtered = filter_roster_with_pops(baseline.clone(), &trusted.pops);
        guard_pop_quorum(filtered, baseline, &trusted.pops, me)
    };
    canonicalize_roster(fallback)
}

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
    use iroha_crypto::{Algorithm, KeyPair};

    use super::*;

    #[test]
    fn canonicalize_roster_preserves_order_and_dedups() {
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

        assert_eq!(canonical, vec![first, second]);
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
}
