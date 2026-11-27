//! Bounded local handoff of producer-authorized DA records to ordinary native proposals.
use std::{collections::BTreeSet, io, sync::Arc};

use iroha_data_model::da::{
    commitment::{DaCommitmentBundle, DaCommitmentKey, DaCommitmentRecord},
    ingest::DaIngestAdmissionPolicyV1,
    pin_intent::{DaPinIntent, DaPinIntentBundle},
};
use iroha_fs::{OwnerDirectory, PublishMode};
use iroha_model_base::topology::LaneId;
use mv::storage::StorageReadOnly;
use norito::codec::{Decode, Encode};
use parking_lot::Mutex;

use super::{
    DaPinIntentValidationError, LaneEpoch, commitment_store::DaCommitmentStore,
    validate_commitment_bundle_against_nexus_at_height,
};
use crate::{
    block::ValidBlock,
    kura::Kura,
    state::{State, StateReadOnly, StateView, WorldReadOnly},
};

const JOURNAL: &str = "da-proposal-candidates.norito";
const MAX_PENDING: usize = 128;
const MAX_BYTES: usize = 1024 * 1024;
pub(crate) const MAX_RELAY_BYTES: usize = MAX_BYTES;
pub(crate) const MAX_RELAY_FRAME: usize = MAX_RELAY_BYTES + 1024;

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::da::proposal_queue::Candidate")]
struct Candidate {
    record: DaCommitmentRecord,
    intent: DaPinIntent,
}

/// The producer's pin-scope witnesses sign the digest of this exact record, including the
/// receipt signature; the stored scope does not carry that digest.
fn matches(record: &DaCommitmentRecord, intent: &DaPinIntent) -> bool {
    (record.lane_id, record.epoch, record.sequence)
        == (intent.lane_id, intent.epoch, intent.sequence)
        && record.manifest_hash == intent.manifest_hash
        && record.storage_ticket == intent.storage_ticket
        && intent
            .pin_scope_authorization
            .has_valid_canonical_signatures(&record.pin_scope_digest())
}

/// Both sections have canonical order by unique lane, epoch and sequence. Every author and
/// verifier checks the producer's pin-scope witnesses against the digest of the paired
/// original record, including the receipt signature.
pub(crate) fn validate_pairs(
    records: Option<&DaCommitmentBundle>,
    pins: Option<&DaPinIntentBundle>,
) -> Result<(), DaPinIntentValidationError> {
    let records = records.map_or(&[][..], DaCommitmentBundle::commitments);
    let pins = pins.map_or(&[][..], |bundle| bundle.intents.as_slice());
    if records.len() != pins.len()
        || !records
            .iter()
            .zip(pins)
            .all(|(record, pin)| matches(record, pin))
    {
        return Err(DaPinIntentValidationError::CommitmentScopeMismatch);
    }
    Ok(())
}

impl Candidate {
    fn collides(&self, other: &Self) -> bool {
        DaCommitmentKey::from_record(&self.record) == DaCommitmentKey::from_record(&other.record)
            || self.record.manifest_hash == other.record.manifest_hash
            || self.record.storage_ticket == other.record.storage_ticket
    }

    fn consumed(&self, world: &impl WorldReadOnly, store: &DaCommitmentStore) -> bool {
        let r = &self.record;
        store.contains_record_identity(r)
            || world
                .da_pin_intents_by_ticket()
                .get(&r.storage_ticket)
                .is_some()
            || world
                .da_pin_intents_by_manifest()
                .get(&r.manifest_hash)
                .is_some()
            || world
                .da_pin_intents_by_lane_epoch()
                .get(&(r.lane_id, r.epoch, r.sequence))
                .is_some()
    }
}

/// Local-only provenance prevents second-hop amplification. A receiving peer cannot set this flag.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::da::proposal_queue::QueueEntry")]
struct QueueEntry {
    candidate: Candidate,
    submitted_here: bool,
}

#[derive(Debug)]
struct Loaded {
    root: OwnerDirectory,
    entries: Arc<Vec<QueueEntry>>,
    checked_tip: Option<iroha_crypto::HashOf<iroha_data_model::block::BlockHeader>>,
    uncertain_write: bool,
}

impl Loaded {
    fn open(root: OwnerDirectory) -> io::Result<Self> {
        let entries: Vec<QueueEntry> = match root.read_private_optional(JOURNAL, MAX_BYTES)? {
            None => Vec::new(),
            Some(bytes) => {
                let limits =
                    norito::DecodeLimits::new(MAX_BYTES, MAX_BYTES, MAX_BYTES, MAX_BYTES * 8, 64);
                let items: Vec<QueueEntry> =
                    norito::decode_canonical_with_limits(&bytes, limits).map_err(invalid)?;
                if items.len() > MAX_PENDING
                    || items.iter().enumerate().any(|(index, item)| {
                        !matches(&item.candidate.record, &item.candidate.intent)
                            || items[..index]
                                .iter()
                                .any(|prior| prior.candidate.collides(&item.candidate))
                    })
                {
                    return Err(invalid("invalid durable DA proposal candidate inventory"));
                }
                // Resolve a previous process's uncertain post-rename durability barrier.
                root.sync()?;
                items
            }
        };
        Ok(Self {
            root,
            entries: Arc::new(entries),
            checked_tip: None,
            uncertain_write: false,
        })
    }

    fn prune(&mut self, obsolete: &BTreeSet<DaCommitmentKey>) -> io::Result<()> {
        if obsolete.is_empty() {
            return Ok(());
        }
        self.replace(
            self.entries
                .iter()
                .filter(|entry| {
                    !obsolete.contains(&DaCommitmentKey::from_record(&entry.candidate.record))
                })
                .cloned()
                .collect(),
        )
    }

    fn insert(&mut self, candidate: Candidate, submitted_here: bool) -> io::Result<()> {
        if let Some(index) = self
            .entries
            .iter()
            .position(|entry| entry.candidate == candidate)
        {
            if submitted_here && !self.entries[index].submitted_here {
                let mut next = self.entries.as_ref().clone();
                next[index].submitted_here = true;
                self.replace(next)?;
            }
            return Ok(());
        }
        let mut next = self.entries.as_ref().clone();
        if next.iter().any(|item| item.candidate.collides(&candidate)) {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "conflicting DA proposal candidate",
            ));
        }
        if next.len() >= MAX_PENDING {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "DA proposal queue is full",
            ));
        }
        next.push(QueueEntry {
            candidate,
            submitted_here,
        });
        self.replace(next)
    }

    fn replace(&mut self, next: Vec<QueueEntry>) -> io::Result<()> {
        let bytes = norito::encode_canonical(&next).map_err(invalid)?;
        if bytes.len() > MAX_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "DA proposal queue exceeds its byte limit",
            ));
        }
        if let Err(error) = self
            .root
            .write_private_atomic(JOURNAL, &bytes, PublishMode::Replace)
        {
            self.uncertain_write = true;
            return Err(error);
        }
        self.entries = Arc::new(next);
        Ok(())
    }
}

fn invalid(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}

#[derive(Debug, Default)]
pub(crate) struct ProposalQueue(Mutex<Option<Loaded>>);

impl ProposalQueue {
    fn load<'a>(slot: &'a mut Option<Loaded>, kura: &Kura) -> io::Result<&'a mut Loaded> {
        if slot.as_ref().is_none_or(|loaded| loaded.uncertain_write) {
            *slot = Some(Loaded::open(kura.da_proposal_root()?)?);
        }
        Ok(slot.as_mut().expect("loaded immediately above"))
    }

    fn snapshot(&self) -> Option<Arc<Vec<QueueEntry>>> {
        // Cold disk reads and durable writes belong to the ingress/relay worker. A busy
        // queue cannot hold up the native proposer while it owns its State view.
        let slot = self.0.try_lock()?;
        let loaded = slot.as_ref().filter(|loaded| !loaded.uncertain_write)?;
        let entries = &loaded.entries;
        (!entries.is_empty()).then(|| Arc::clone(entries))
    }
}

impl State {
    /// Offer one locally pending ingress record to the validators of its committed route.
    /// Only the originating node forwards the pair. A receiving validator retains its copy for
    /// its proposer turn. A changed ledger tip sweeps the bounded inventory, and each tick
    /// sends at most one remaining entry.
    ///
    /// # Errors
    /// Returns a bounded local queue, encoding, or authoritative route resolution failure.
    pub fn relay_da_proposal_candidate(
        &self,
        local: &iroha_model_base::peer::PeerId,
        cursor: &mut usize,
        network: &crate::IrohaNetwork,
    ) -> io::Result<()> {
        let entries = self.reconcile_da_proposal_candidates(None)?;
        if entries.is_empty() {
            return Ok(());
        }
        let index = *cursor % entries.len();
        *cursor = index + 1;
        let entry = &entries[index];
        if !entry.submitted_here {
            return Ok(());
        }
        let candidate = &entry.candidate;
        let committee = self.da_relay_authority(candidate.record.lane_id)?;
        let bytes = Arc::new(norito::encode_canonical(candidate).map_err(invalid)?);
        if bytes.len() > MAX_RELAY_BYTES {
            return Err(invalid("DA relay frame exceeds byte limit"));
        }
        for peer_id in committee
            .into_validators()
            .into_iter()
            .filter(|peer| peer != local)
        {
            network.post(iroha_p2p::Post {
                data: crate::NetworkMessage::DaProposal(Arc::clone(&bytes)),
                peer_id,
                priority: iroha_p2p::Priority::Low,
            });
        }
        Ok(())
    }

    fn da_relay_authority(&self, lane: LaneId) -> io::Result<crate::state::LaneAuthorityCommittee> {
        let view = self.view();
        let height = u64::try_from(view.height()).map_err(invalid)?;
        let space = crate::state::consensus_lane_dataspace_at_height(lane, view.nexus(), height)
            .ok_or_else(|| invalid("DA relay lane has no committed route"))?;
        drop(view);
        self.resolve_route_authority_at_height(
            crate::state::LaneAuthorityRoute::new(lane, space),
            height,
        )
        .map_err(invalid)
    }

    /// Authenticate and durably enqueue an original peer candidate on a current route validator.
    ///
    /// # Errors
    /// Refuses malformed or over-budget frames, unauthorized recipients, producer signatures,
    /// policy violations and persistence failures.
    pub fn accept_relayed_da_proposal_candidate(
        &self,
        local: &iroha_model_base::peer::PeerId,
        bytes: &[u8],
    ) -> io::Result<()> {
        if bytes.is_empty() || bytes.len() > MAX_RELAY_BYTES {
            return Err(invalid("DA relay frame exceeds byte limit"));
        }
        let limits = norito::DecodeLimits::new(
            MAX_RELAY_BYTES,
            MAX_RELAY_BYTES,
            MAX_RELAY_BYTES,
            8 * MAX_RELAY_BYTES,
            64,
        );
        let candidate: Candidate =
            norito::decode_canonical_with_limits(bytes, limits).map_err(invalid)?;
        if !self
            .da_relay_authority(candidate.record.lane_id)?
            .validators()
            .contains(local)
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "local peer is not a validator for the signed DA route",
            ));
        }
        self.persist_da_proposal_candidate(&candidate.record, &candidate.intent, false)
    }

    /// Durably offer the producer-authorized record for publication with ordinary block work.
    /// An exact committed or pending retry succeeds. A proposal never consumes the entry.
    ///
    /// # Errors
    /// Returns validation, conflict, capacity, or durability errors to the submitter.
    pub fn enqueue_da_proposal_candidate(
        &self,
        record: &DaCommitmentRecord,
        intent: &DaPinIntent,
    ) -> io::Result<()> {
        self.persist_da_proposal_candidate(record, intent, true)
    }

    fn persist_da_proposal_candidate(
        &self,
        record: &DaCommitmentRecord,
        intent: &DaPinIntent,
        submitted_here: bool,
    ) -> io::Result<()> {
        if !matches(record, intent) {
            return Err(invalid("DA record does not match signed pin scope"));
        }
        let candidate = Candidate {
            record: record.clone(),
            intent: intent.clone(),
        };
        self.reconcile_da_proposal_candidates(Some((candidate, submitted_here)))
            .map(|_| ())
    }

    /// Sweep former epochs, settled identities and both cursor indexes even on a receiving
    /// node with no local uploads. Exact committed retries acknowledge the original result.
    fn reconcile_da_proposal_candidates(
        &self,
        offer: Option<(Candidate, bool)>,
    ) -> io::Result<Arc<Vec<QueueEntry>>> {
        let kura = self.kura();
        let queue = &kura.da_proposal_queue;
        if offer.is_none() {
            let mut slot = queue.0.lock();
            let loaded = ProposalQueue::load(&mut slot, kura)?;
            if loaded.entries.is_empty() {
                return Ok(Arc::clone(&loaded.entries));
            }
        }
        self.ensure_da_indexes_hydrated()
            .map_err(|error| io::Error::other(error.to_string()))?;
        // Serialize local journal authors before reading ledger state. An old offer cannot
        // overtake a completed sweep and reinstall a retired record from an earlier view.
        let mut slot = queue.0.lock();
        let loaded = ProposalQueue::load(&mut slot, kura)?;
        let view = self.view();
        let tip = view.latest_block_hash();
        if offer.is_none() && tip.is_some() && tip == loaded.checked_tip {
            return Ok(Arc::clone(&loaded.entries));
        }
        let store = self.da_commitments.read();
        let receipts = view.da_receipt_cursors.read();
        let shards = view.da_shard_cursors.read();
        let height = u64::try_from(view.height())
            .map_err(invalid)?
            .checked_add(1)
            .ok_or_else(|| invalid("next height overflow"))?;
        let policy = view
            .world
            .parameters
            .get()
            .custom()
            .get(&DaIngestAdmissionPolicyV1::parameter_id())
            .map(DaIngestAdmissionPolicyV1::from_custom_parameter)
            .transpose()
            .map_err(invalid)?
            .flatten();
        let retired = |entry: &Candidate| {
            let r = &entry.record;
            entry.consumed(view.world(), &store)
                || receipts
                    .highest(LaneEpoch::new(r.lane_id, r.epoch))
                    .is_some_and(|s| r.sequence <= s)
                || shards
                    .get(view.nexus.lane_config.shard_id(r.lane_id), r.lane_id)
                    .is_some_and(|c| (r.epoch, r.sequence) <= (c.epoch, c.sequence))
                || !policy.as_ref().is_some_and(|policy| {
                    view.lane_incarnation_at_height(r.lane_id, height)
                        .is_some_and(|incarnation| {
                            policy.authorizes(
                                &entry.intent.authorization.owner,
                                r.lane_id,
                                incarnation,
                                r.epoch,
                            )
                        })
                })
        };
        let obsolete = loaded
            .entries
            .iter()
            .map(|entry| &entry.candidate)
            .filter(|entry| retired(entry))
            .map(|entry| DaCommitmentKey::from_record(&entry.record))
            .collect::<BTreeSet<_>>();
        let admission = offer.map(|(candidate, submitted_here)| -> io::Result<_> {
            let record = &candidate.record;
            let intent = &candidate.intent;
            if store.get_committed_by_key(&DaCommitmentKey::from_record(record))
                .is_some_and(|entry| &entry.commitment == record)
                && view.world().da_pin_intents_by_ticket().get(&intent.storage_ticket)
                    .is_some_and(|entry| &entry.intent == intent) {
                return Ok(None);
            }
            if retired(&candidate) {
                return Err(io::Error::new(io::ErrorKind::AlreadyExists,
                    "DA candidate is settled, behind a cursor, or outside current producer authority"));
            }
            // Once an epoch has a cursor, queue only an uninterrupted prefix. A sender
            // can retry a gap after its predecessors arrive, without reserving journal space.
            if let Some(previous) = receipts.highest(LaneEpoch::new(record.lane_id, record.epoch)) {
                let prefix = loaded.entries.iter().map(|e| &e.candidate.record)
                    .filter(|r| r.lane_id == record.lane_id && r.epoch == record.epoch
                        && !obsolete.contains(&DaCommitmentKey::from_record(r)))
                    .map(|r| r.sequence).collect::<BTreeSet<_>>();
                if !has_predecessors(previous, record.sequence, &prefix) {
                    return Err(io::Error::new(io::ErrorKind::WouldBlock, "DA candidate is waiting for preceding sequences"));
                }
            }
            validate_commitment_bundle_against_nexus_at_height(
                &DaCommitmentBundle::new(vec![record.clone()]), view.nexus(), height).map_err(invalid)?;
            let quotas = crate::runtime::committed_policy::effective_nexus_quotas(
                view.world.runtime_selection.get(), view.world.parameters.get());
            ValidBlock::validate_da_proposal_pins(&DaPinIntentBundle::new(vec![intent.clone()]),
                Some(&DaCommitmentBundle::new(vec![record.clone()])), height, &view, &quotas).map_err(invalid)?;
            Ok(Some((candidate, submitted_here)))
        }).transpose();
        // Disk durability never holds ledger readers, including the exact-retry cleanup.
        drop(shards);
        drop(receipts);
        drop(store);
        drop(view);
        loaded.prune(&obsolete)?;
        loaded.checked_tip = tip;
        if let Some((candidate, submitted_here)) = admission?.flatten() {
            loaded.insert(candidate, submitted_here)?;
        }
        Ok(Arc::clone(&loaded.entries))
    }
}

#[cfg(test)]
pub(crate) fn pending_after_sweep(state: &State) -> Vec<DaCommitmentRecord> {
    let entries = state.reconcile_da_proposal_candidates(None).unwrap();
    let durable = Loaded::open(state.kura().da_proposal_root().unwrap()).unwrap();
    assert_eq!(entries, durable.entries);
    entries
        .iter()
        .map(|entry| entry.candidate.record.clone())
        .collect()
}

#[cfg(test)]
pub(crate) fn relay_bytes_for_test(record: &DaCommitmentRecord, intent: &DaPinIntent) -> Vec<u8> {
    norito::encode_canonical(&Candidate {
        record: record.clone(),
        intent: intent.clone(),
    })
    .unwrap()
}

fn optional_rank(index: usize, lanes: usize, height: u64) -> usize {
    let start = (height % lanes as u64) as usize;
    (index + lanes - start) % lanes
}

fn has_predecessors(previous: u64, sequence: u64, pending: &BTreeSet<u64>) -> bool {
    let Some(gap) = sequence
        .checked_sub(previous)
        .and_then(|n| n.checked_sub(1))
    else {
        return false;
    };
    gap <= pending.len() as u64 && (previous + 1..sequence).all(|index| pending.contains(&index))
}

/// Select at most one candidate per lane, covering every external or merged work lane. Check
/// the authenticated parent again, since accounts, quotas and receipts can change after enqueue.
pub(crate) fn select(
    state: &State,
    view: &StateView<'_>,
    height: u64,
    routed_lanes: impl Iterator<Item = LaneId>,
) -> io::Result<Option<(DaCommitmentBundle, DaPinIntentBundle)>> {
    let Some(entries) = state.kura().da_proposal_queue.snapshot() else {
        return Ok(None);
    };
    if !state.da_indexes_ready_for_proposal() {
        return Ok(None);
    }
    let required = routed_lanes.collect::<BTreeSet<_>>();
    let Some(store) = state.da_commitments.try_read() else {
        return Ok(None);
    };
    let Some(receipts) = view.da_receipt_cursors.try_read() else {
        return Ok(None);
    };
    let Some(shards) = view.da_shard_cursors.try_read() else {
        return Ok(None);
    };
    let quotas = crate::runtime::committed_policy::effective_nexus_quotas(
        view.world.runtime_selection.get(),
        view.world.parameters.get(),
    );
    let mut records = Vec::new();
    let mut pins = Vec::new();
    let mut lanes = BTreeSet::new();
    // Rotate optional lanes across paid heights. Within each lane the previous epoch and
    // its lowest outstanding sequence get the first chance, independently of relay order.
    let optional_lanes = entries
        .iter()
        .map(|entry| entry.candidate.record.lane_id)
        .filter(|lane| !required.contains(lane))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut candidates = entries
        .iter()
        .map(|entry| &entry.candidate)
        .collect::<Vec<_>>();
    candidates.sort_by_key(|entry| {
        let r = &entry.record;
        let rank = optional_lanes
            .binary_search(&r.lane_id)
            .ok()
            .map_or(0, |i| 1 + optional_rank(i, optional_lanes.len(), height));
        (rank, DaCommitmentKey::from_record(r))
    });
    for candidate in candidates {
        let record = &candidate.record;
        let lane = record.lane_id;
        if !required.contains(&lane) && !required.is_subset(&lanes) {
            break;
        }
        if lanes.contains(&lane)
            || candidate.consumed(view.world(), &store)
            || shards
                .get(view.nexus.lane_config.shard_id(lane), lane)
                .is_some_and(|c| (record.epoch, record.sequence) <= (c.epoch, c.sequence))
            || receipts
                .validate_bundle(height, std::slice::from_ref(record))
                .is_err()
            || validate_commitment_bundle_against_nexus_at_height(
                &DaCommitmentBundle::new(vec![record.clone()]),
                view.nexus(),
                height,
            )
            .is_err()
        {
            continue;
        }
        pins.push(candidate.intent.clone());
        let paired = DaCommitmentBundle::new(
            records
                .iter()
                .chain(std::iter::once(record))
                .cloned()
                .collect(),
        );
        if ValidBlock::validate_da_proposal_pins(
            &DaPinIntentBundle::new(pins.clone()),
            Some(&paired),
            height,
            view,
            &quotas,
        )
        .is_err()
        {
            pins.pop();
            continue;
        }
        records.push(record.clone());
        lanes.insert(lane);
        // Also advance one waiting lane that currently has no ordinary work of its own.
        if !required.contains(&lane) && required.is_subset(&lanes) {
            break;
        }
    }
    Ok(
        (!records.is_empty() && required.is_subset(&lanes)).then(|| {
            (
                DaCommitmentBundle::new(records),
                DaPinIntentBundle::new(pins),
            )
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
    use iroha_data_model::{
        NetworkId,
        block::BlockHeader,
        da::types::{BlobDigest, StorageTicketId},
        sorafs::pin_registry::ManifestDigest,
    };

    fn candidate(index: u8) -> Candidate {
        let key = KeyPair::try_from_seed(vec![7; 32], Algorithm::Ed25519).unwrap();
        let network = NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
            Hash::prehashed([3; 32]),
        ));
        let auth = crate::da::signed_test_ingest_authorization(
            network,
            &key,
            LaneId::SINGLE,
            1,
            u64::from(index),
            1,
        );
        let ticket = StorageTicketId::new([index; 32]);
        let manifest = ManifestDigest::new([index; 32]);
        let record = crate::da::test_pin_commitment(&auth, ticket, manifest);
        let intent = crate::da::signed_test_pin_intent(auth, &key, ticket, manifest, None);
        Candidate { record, intent }
    }

    #[test]
    fn optional_da_lanes_each_get_one_turn_and_cursor_gaps_are_bounded() {
        for n in 1..=MAX_PENDING {
            let first = (0..n)
                .map(|height| {
                    (0..n)
                        .find(|&index| optional_rank(index, n, height as u64) == 0)
                        .unwrap()
                })
                .collect::<BTreeSet<_>>();
            assert_eq!(first, (0..n).collect());
        }
        let pending = BTreeSet::from([7, 8, 10]);
        assert!(has_predecessors(6, 7, &pending));
        assert!(has_predecessors(6, 9, &pending));
        assert!(!has_predecessors(6, 11, &pending));
        assert!(!has_predecessors(6, u64::MAX, &pending));
        assert!(!has_predecessors(u64::MAX, 0, &pending));
        assert!(has_predecessors(u64::MAX - 1, u64::MAX, &pending));
    }

    #[test]
    fn native_da_proposal_queue_recovers_retains_conflicts_and_prunes_committed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().canonicalize().unwrap();
        let open = || Loaded::open(OwnerDirectory::open(&path).unwrap()).unwrap();
        let first = candidate(1);
        let second = candidate(2);
        let mut persisted = open();
        persisted.insert(first.clone(), false).unwrap();
        drop(persisted);
        let mut reopened = open();
        assert_eq!(reopened.entries.len(), 1);
        assert_eq!(reopened.entries[0].candidate, first);
        assert!(!reopened.entries[0].submitted_here);
        reopened.insert(first.clone(), true).unwrap();
        let mut conflict = first.clone();
        conflict.record.chunk_root = BlobDigest::new([99; 32]);
        assert_eq!(
            reopened.insert(conflict, true).unwrap_err().kind(),
            io::ErrorKind::AlreadyExists
        );
        assert_eq!(open().entries[0].candidate, first);
        assert!(open().entries[0].submitted_here);
        reopened
            .prune(&BTreeSet::from([DaCommitmentKey::from_record(
                &first.record,
            )]))
            .unwrap();
        assert!(open().entries.is_empty());
        reopened.insert(second.clone(), true).unwrap();
        assert_eq!(open().entries.len(), 1);
        assert_eq!(open().entries[0].candidate, second);
        reopened
            .root
            .write_private_atomic(JOURNAL, b"corrupt", PublishMode::Replace)
            .unwrap();
        assert_eq!(
            Loaded::open(OwnerDirectory::open(&path).unwrap())
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidData
        );
    }
}
