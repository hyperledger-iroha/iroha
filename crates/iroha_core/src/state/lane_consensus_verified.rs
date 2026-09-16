//! Authenticate current lane instances without retaining State guards across disk I/O.

use std::collections::BTreeMap;

use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    NetworkId,
    block::{BlockHeader, consensus_v2 as wire},
};

use super::{FrozenLaneConsensusContextV1, LaneConsensusContextsV1, State};
use crate::sumeragi::v2_core as core;

/// Immutable native authority authenticated by a current global carrier.
///
/// This value proves an instance's identity, not that the instance remains open.
/// Signing, broadcast and application must also use a fresh complete-set observation.
#[derive(Debug, Clone)]
pub(crate) struct VerifiedLaneContext {
    frozen: FrozenLaneConsensusContextV1,
    reducer: core::HeightContext,
}

impl VerifiedLaneContext {
    /// Exact post-opening native roster, route, policy and admitted head.
    pub(crate) fn frozen(&self) -> &FrozenLaneConsensusContextV1 {
        &self.frozen
    }

    /// Shared reducer context derived only after native finality authentication.
    pub(crate) fn reducer_context(&self) -> &core::HeightContext {
        &self.reducer
    }

    /// Domain-separated identity of this exact opening and lane slot.
    pub(crate) fn instance_id(&self) -> wire::HeightContextId {
        wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::prehashed(
            *self.reducer.id().as_bytes(),
        )))
    }

    // Only the Kura-backed reader below may construct this type. The artifact
    // has already passed the native finality reader, including canonical-chain,
    // complete wire, aggregate signature and proof-of-possession checks.
    fn from_verified_opening(
        frozen: FrozenLaneConsensusContextV1,
        opening: &wire::finality::V2FinalityArtifact,
        expected_block_hash: HashOf<BlockHeader>,
    ) -> Result<Self, String> {
        let authority = &opening.height_context;
        if opening.block_hash != expected_block_hash
            || opening.height != frozen.opening_global_height
            || authority.network_id != frozen.network_id
            || authority.protocol_version != frozen.protocol_version
            || authority.id() != frozen.opening_global_context_id
            || authority.epoch != frozen.epoch
            || authority.mode != frozen.mode
            || authority.nexus_amx_context_hash != frozen.nexus_amx_context_hash
            || authority.execution_policy_hash != frozen.execution_policy_hash
            || authority.da_layout != frozen.da_layout
            || authority.leader_seed != frozen.leader_seed
        {
            return Err(
                "frozen lane context differs from its canonical opening authority".to_owned(),
            );
        }
        // The lane roster is post-carrier lane authority. It need not equal
        // the global roster which certified that carrier's transition.
        let frozen_hash = frozen.canonical_hash().map_err(|error| error.to_string())?;
        let subject_bytes =
            norito::encode_canonical(&opening.subject).map_err(|error| error.to_string())?;
        let subject_hash = Hash::new(&subject_bytes);
        let instance_hash = Hash::new_from_chunks(&[
            b"iroha:lane-consensus:finalized-instance:v1\0",
            frozen_hash.as_ref(),
            &subject_bytes,
        ]);
        let layout =
            norito::encode_canonical(&frozen.da_layout).map_err(|error| error.to_string())?;
        let leader_seed = Hash::new_from_chunks(&[
            b"iroha:lane-consensus:leader-seed:v1\0",
            &frozen.leader_seed,
            &frozen.lane_id.as_u32().to_be_bytes(),
            &frozen.dataspace_id.as_u64().to_be_bytes(),
            frozen.lane_incarnation.as_ref(),
            &frozen.next_lane_height.to_be_bytes(),
        ]);
        let roster = frozen
            .committee
            .iter()
            .enumerate()
            .map(|(index, _)| {
                let mut token = [0; Hash::LENGTH];
                token[28..].copy_from_slice(&(index as u32).to_be_bytes());
                core::Validator::new(core::ValidatorId::new(token), core::VotingPower::new(1))
            })
            .collect();
        let reducer = core::HeightContext::new_from_finalized_state(
            core::ContextId::new(*instance_hash.as_ref()),
            core::NetworkId::new(*frozen.network_id.as_bytes()),
            frozen.next_lane_height,
            core::FinalizedStateAnchor {
                context_id: core::ContextId::new(*frozen.opening_global_context_id.0.as_ref()),
                height: frozen.opening_global_height,
                subject: core::Subject::new(*subject_hash.as_ref()),
                predecessor_height: frozen.predecessor_height,
                predecessor_subject: frozen
                    .predecessor_hash
                    .map(|hash| core::Subject::new(*hash.as_ref())),
            },
            frozen.epoch,
            roster,
            match frozen.mode {
                wire::ConsensusMode::Permissioned => core::VotingMode::Permissioned,
                wire::ConsensusMode::Npos => core::VotingMode::Npos,
            },
            core::Digest::new(*frozen.nexus_amx_context_hash.as_ref()),
            core::Digest::new(*frozen.execution_policy_hash.as_ref()),
            core::Digest::new(*Hash::new(layout).as_ref()),
            core::Digest::new(*leader_seed.as_ref()),
        )
        .map_err(|error| error.to_string())?;
        Ok(Self { frozen, reducer })
    }
}

/// Complete authenticated observation of currently open lane instances.
///
/// An empty observation is authenticated too. Publication freshness is a
/// separate check, and must be repeated at each productive effect boundary.
#[derive(Debug)]
pub(crate) struct VerifiedLaneContexts {
    generation: u64,
    network_id: NetworkId,
    carrier_height: u64,
    carrier_hash: HashOf<BlockHeader>,
    contexts: Vec<VerifiedLaneContext>,
}

impl VerifiedLaneContexts {
    /// Complete immutable set in canonical lane order.
    pub(crate) fn contexts(&self) -> &[VerifiedLaneContext] {
        &self.contexts
    }

    /// Exact carrier authenticating membership and absence in this observation.
    pub(crate) fn carrier_height(&self) -> u64 {
        self.carrier_height
    }

    /// Whether this observation still names the same coherent State publication.
    ///
    /// This is an observation, not a lease spanning a subsequent state commit.
    pub(crate) fn is_current(&self, state: &State) -> bool {
        if !super::is_stable_state_view_generation(self.generation, state.state_view_generation())
            || self.network_id != *state.network_id_ref()
        {
            return false;
        }
        let (height, hash) = {
            let hashes = state.block_hashes.view();
            (hashes.len() as u64, hashes.last().copied())
        };
        height == self.carrier_height
            && hash == Some(self.carrier_hash)
            && super::is_stable_state_view_generation(
                self.generation,
                state.state_view_generation(),
            )
    }
}

impl State {
    /// Authenticate a coherent complete lane context set against durable global finality.
    ///
    /// `None` means State is empty, exact finality publication is pending, or a
    /// concurrent State publication invalidated the observation. Malformed or
    /// contradictory retained evidence is an error, never authenticated closure.
    /// No State/MV guard survives into any Kura read.
    pub(crate) fn verified_lane_consensus_contexts(
        &self,
    ) -> Result<Option<VerifiedLaneContexts>, String> {
        let generation = self.state_view_generation();
        if generation % 2 != 0 {
            return Ok(None);
        }
        let (network_id, carrier_height, carrier_hash, contexts, opening_hashes) = {
            let view = self.view();
            let Some(carrier_hash) = view.block_hashes.last().copied() else {
                return Ok(None);
            };
            let contexts = view.lane_consensus_contexts.clone();
            let opening_hashes = contexts
                .contexts
                .iter()
                .map(|context| {
                    let index = context
                        .opening_global_height
                        .checked_sub(1)
                        .and_then(|height| usize::try_from(height).ok());
                    (
                        context.opening_global_height,
                        index.and_then(|index| view.block_hashes.get(index).copied()),
                    )
                })
                .collect::<BTreeMap<_, _>>();
            (
                view.network_id,
                view.block_hashes.len() as u64,
                carrier_hash,
                contexts,
                opening_hashes,
            )
        };
        if !super::is_stable_state_view_generation(generation, self.state_view_generation()) {
            return Ok(None);
        }
        let mut observation = VerifiedLaneContexts {
            generation,
            network_id,
            carrier_height,
            carrier_hash,
            contexts: Vec::new(),
        };
        #[cfg(test)]
        io_observer::notify();
        let result =
            self.authenticate_lane_context_observation(&observation, contexts, opening_hashes);
        // A concurrent replacement can invalidate even an I/O error's chain
        // association. Retry from a fresh publication rather than mixing epochs.
        if !observation.is_current(self) {
            return Ok(None);
        }
        let Some(contexts) = result? else {
            return Ok(None);
        };
        observation.contexts = contexts;
        Ok(Some(observation))
    }

    fn authenticate_lane_context_observation(
        &self,
        observation: &VerifiedLaneContexts,
        contexts: LaneConsensusContextsV1,
        opening_hashes: BTreeMap<u64, Option<HashOf<BlockHeader>>>,
    ) -> Result<Option<Vec<VerifiedLaneContext>>, String> {
        let Some((current, proof)) = self
            .kura
            .lane_consensus_contexts_finality(observation.carrier_height)
            .map_err(|error| error.to_string())?
        else {
            return Ok(None);
        };
        if current.block_hash != observation.carrier_hash
            || current.height_context.network_id != observation.network_id
            || !proof.verify(
                observation.network_id,
                observation.carrier_height,
                current.commit_qc.execution_commitment.ordinary_writes_root,
            )
            || !proof.matches_contexts(
                observation.network_id,
                observation.carrier_height,
                &contexts,
            )?
        {
            return Err(
                "current State lane context set differs from its finalized carrier".to_owned(),
            );
        }
        let mut openings = BTreeMap::new();
        openings.insert(current.height, current);
        let mut verified = Vec::with_capacity(contexts.contexts.len());
        for frozen in contexts.contexts {
            let expected_hash = opening_hashes
                .get(&frozen.opening_global_height)
                .copied()
                .flatten()
                .ok_or_else(|| {
                    "lane opening carrier is absent from coherent State history".to_owned()
                })?;
            if let std::collections::btree_map::Entry::Vacant(entry) =
                openings.entry(frozen.opening_global_height)
            {
                let artifact = self
                    .kura
                    .v2_finality_artifact(frozen.opening_global_height)
                    .map_err(|error| error.to_string())?
                    .ok_or_else(|| {
                        format!(
                            "required historical lane opening finality is missing at height {}",
                            frozen.opening_global_height,
                        )
                    })?;
                entry.insert(artifact);
            }
            let opening = openings
                .get(&frozen.opening_global_height)
                .ok_or_else(|| "authenticated lane opening was not retained".to_owned())?;
            verified.push(VerifiedLaneContext::from_verified_opening(
                frozen,
                opening,
                expected_hash,
            )?);
        }
        Ok(Some(verified))
    }
}

#[cfg(test)]
pub(super) mod io_observer {
    std::thread_local! {
        static OBSERVER: std::cell::RefCell<Option<std::rc::Rc<dyn Fn()>>> =
            const { std::cell::RefCell::new(None) };
    }

    pub(super) fn notify() {
        let observer = OBSERVER.with(|current| current.borrow().clone());
        if let Some(observer) = observer {
            observer();
        }
    }

    pub(in crate::state) fn observe<T>(
        observer: impl Fn() + 'static,
        action: impl FnOnce() -> T,
    ) -> T {
        struct Restore(Option<std::rc::Rc<dyn Fn()>>);
        impl Drop for Restore {
            fn drop(&mut self) {
                let _ = OBSERVER.with(|current| current.replace(self.0.take()));
            }
        }
        let _restore =
            Restore(OBSERVER.with(|current| current.replace(Some(std::rc::Rc::new(observer)))));
        action()
    }
}
