//! Exact lane-frame authentication against the preceding certified global lane state.
//!
//! The original lane frames are evidence inputs. A store filename, append success or an
//! exported row never supplies certificate authority. This owner runs the active lane
//! admission rule and verifies the complete native commit quorum before linking the frame
//! to the global chain's merge references and executed transaction suffix.

use super::*;
use std::collections::VecDeque;

use iroha_core::{
    state::NativeLaneStateProjectionV1,
    sumeragi::lanes::{
        self, AnchorView, LANE_DEDUP_WINDOW, LaneBatch, LaneChainView, evidence::verify_lane_entry,
    },
};
use iroha_data_model::sumeragi_lanes::{SumeragiLanePolicy, SumeragiLaneRecord, SumeragiLaneState};
use iroha_sumeragi::{message::SyncEntry, types::Hash32};
use norito::codec::{DecodeAll as _, Encode as _};

/// One exact original lane store frame; the enclosing V1 format fixes its raw Norito layout.
#[derive(Clone, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_kagami::scaling_evidence::LaneFrameV1")]
pub(crate) struct LaneFrameV1 {
    /// Lane selected by the global merge reference, never a proof-supplied authority.
    pub(crate) lane: LaneId,
    /// Exact original SyncEntry bytes, retaining both header/payload and native CommitQC.
    pub(crate) frame: Vec<u8>,
}

/// Complete certified lane state and the exact frames referenced by one global carrier.
#[derive(Clone, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_kagami::scaling_evidence::LaneMergeEvidenceV1")]
pub(crate) struct LaneMergeEvidenceV1 {
    /// Complete poststate independently bound by the carrier's mandatory R.native_lanes.
    pub(crate) state: NativeLaneStateProjectionV1,
    /// One original frame per merged height, in lane, then lane-height order.
    pub(crate) frames: Vec<LaneFrameV1>,
}

/// Native lane provenance of an executed transaction. Lane zero has no such source.
#[derive(Debug, Clone, PartialEq, Eq, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_kagami::scaling_evidence::AuthenticatedLaneSourceV1")]
pub struct AuthenticatedLaneSourceV1 {
    /// Pinned incarnation from the preceding certified global state.
    pub incarnation: [u8; 32],
    /// Derived native consensus instance of this incarnation.
    pub instance: [u8; 32],
    /// Original lane height, distinct from the global execution height.
    pub height: u64,
    /// Original native lane header hash.
    pub block_hash: [u8; 32],
    /// Lane admission result derived before its commit votes.
    pub result: [u8; 32],
    /// Transaction index within the exact admitted lane batch.
    pub batch_index: u32,
    /// Applied global height at which the lane batch was anchored.
    pub anchor_height: u64,
    /// Exact certified global header hash at that anchor.
    pub anchor_hash: HashOf<BlockHeader>,
}

struct History {
    height: u64,
    hash: Hash32,
    result: Hash32,
    window: VecDeque<(u64, Vec<HashOf<TransactionEntrypoint>>)>,
}
impl History {
    fn genesis(network: NetworkId, record: &SumeragiLaneRecord) -> Self {
        Self {
            height: 0,
            hash: lanes::lane_genesis_hash(&network, record),
            result: lanes::lane_genesis_result(record),
            window: VecDeque::new(),
        }
    }
    fn chain_view(&self) -> LaneChainView {
        let mut recent = BTreeMap::new();
        for (anchor, hashes) in &self.window {
            for hash in hashes {
                recent.insert(*hash, *anchor);
            }
        }
        LaneChainView {
            previous_anchor: self.window.back().map_or(0, |(anchor, _)| *anchor),
            recent,
        }
    }
    fn apply(&mut self, entry: &SyncEntry, batch: &LaneBatch) {
        self.height = entry.block.header.height;
        self.hash = entry.commit_qc.block_hash;
        self.result = entry.commit_qc.result;
        self.window.push_back((
            batch.anchor_height,
            batch
                .transactions
                .iter()
                .map(SignedTransaction::hash_as_entrypoint)
                .collect(),
        ));
        while self.window.len() > LANE_DEDUP_WINDOW {
            self.window.pop_front();
        }
    }
}

#[derive(Default)]
struct Anchors(BTreeMap<u64, (HashOf<BlockHeader>, u64)>);
impl AnchorView for Anchors {
    fn applied_hash(&self, height: u64) -> Option<HashOf<BlockHeader>> {
        self.0.get(&height).map(|(hash, _)| *hash)
    }
    fn creation_time_ms(&self, height: u64) -> Option<u64> {
        self.0.get(&height).map(|(_, time)| *time)
    }
}

/// Chronological lane proof owner. Its caller poisons the entire run on any error.
#[derive(Default)]
pub(super) struct LaneProofState {
    anchors: Anchors,
    previous: Option<SumeragiLaneState>,
    histories: BTreeMap<(LaneId, [u8; 32]), History>,
}

pub(super) struct ProvenSource {
    pub(super) lane: LaneId,
    pub(super) dataspace: DataSpaceId,
    pub(super) source: AuthenticatedLaneSourceV1,
}

impl LaneProofState {
    /// Accept the original H1 values only after the caller has verified its actual H2 anchor.
    pub(super) fn anchor_genesis(
        &mut self,
        block: &SignedBlock,
        state: SumeragiLaneState,
    ) -> Result<()> {
        ensure!(
            self.previous.is_none() && block.header().height().get() == 1,
            "lane evidence genesis is repeated or displaced"
        );
        self.anchor(block)?;
        self.previous = Some(state);
        Ok(())
    }

    fn anchor(&mut self, block: &SignedBlock) -> Result<()> {
        let height = block.header().height().get();
        ensure!(
            self.anchors
                .0
                .insert(
                    height,
                    (
                        block.hash(),
                        u64::try_from(block.header().creation_time().as_millis())?
                    )
                )
                .is_none(),
            "repeated global anchor"
        );
        Ok(())
    }

    /// Verify every original referenced frame, then join actual executed suffix positions.
    pub(super) fn verify(
        &mut self,
        block: &SignedBlock,
        poststate: &SumeragiLaneState,
        frames: &[LaneFrameV1],
        policy: &SumeragiLanePolicy,
        network: NetworkId,
        chain: &ChainId,
    ) -> Result<BTreeMap<usize, ProvenSource>> {
        let previous = self
            .previous
            .as_ref()
            .ok_or_else(|| eyre!("missing certified preceding lane state"))?;
        let height = block.header().height().get();
        let Some(section) = block.lane_merge() else {
            ensure!(frames.is_empty(), "extra lane frames without a merge");
            self.previous = Some(poststate.clone());
            self.anchor(block)?;
            return Ok(BTreeMap::new());
        };
        ensure!(
            !section.merges.is_empty() && section.merges.windows(2).all(|w| w[0].lane < w[1].lane),
            "empty or unordered lane merges"
        );
        let mut frame_index = 0usize;
        let mut candidates = Vec::new();
        let mut time_floor = 0u64;
        for merge in &section.merges {
            let record = previous
                .lane(merge.lane)
                .ok_or_else(|| eyre!("merge names no preceding lane record"))?;
            ensure!(
                record.lane.as_u32() != 0
                    && record.incarnation == merge.incarnation
                    && height > record.active_from
                    && merge.from
                        == record
                            .merged
                            .height
                            .checked_add(1)
                            .ok_or_else(|| eyre!("lane frontier overflow"))?
                    && !merge.is_empty()
                    && merge.len() <= u64::from(policy.max_merge_blocks),
                "merge differs from its active incarnation or exact next range"
            );
            // The independent policy and the authenticated record must agree. Lane committee
            // authority is never loaded from a frame or a standalone roster transport.
            if let Some(fixed) = policy.fixed_lane(record.lane) {
                ensure!(
                    fixed.dataspace == record.dataspace && fixed.committee == record.committee,
                    "fixed lane authority differs from the independently pinned policy"
                );
            } else {
                ensure!(
                    policy.is_elastic(record.lane),
                    "lane is absent from pinned policy"
                );
            }
            ensure!(
                record.params == policy.lane_params
                    && record.anchor_freshness == policy.anchor_freshness,
                "lane parameters differ from independently pinned policy"
            );
            let history = self
                .histories
                .entry((record.lane, record.incarnation))
                .or_insert_with(|| History::genesis(network, record));
            ensure!(
                history.height == record.merged.height
                    && (history.height == 0
                        || (history.hash.0 == record.merged.block_hash
                            && history.result.0 == record.merged.result)),
                "lane evidence history does not continue the certified merged frontier"
            );
            for expected_height in merge.from..=merge.to {
                let supplied = frames
                    .get(frame_index)
                    .ok_or_else(|| eyre!("missing original lane frame"))?;
                frame_index += 1;
                ensure!(
                    supplied.lane == merge.lane,
                    "lane frames are out of merge order"
                );
                let entry = decode_frame(&supplied.frame)?;
                ensure!(
                    entry.block.header.height == expected_height,
                    "lane frame height is out of merge order"
                );
                let predecessor = iroha_data_model::sumeragi_lanes::SumeragiLaneFrontier {
                    height: history.height,
                    block_hash: history.hash.0,
                    result: history.result.0,
                };
                verify_lane_entry(
                    record,
                    &network,
                    &chain.to_string(),
                    &self.anchors,
                    &history.chain_view(),
                    &predecessor,
                    &entry,
                )?;
                let qc = &entry.commit_qc;
                let batch = LaneBatch::from_payload(&entry.block.payload)?;
                if !record.is_stale(batch.anchor_height, height) {
                    for (index, transaction) in batch.transactions.iter().enumerate() {
                        time_floor = time_floor.max(
                            u64::try_from(transaction.creation_time().as_millis())?
                                .checked_add(1)
                                .ok_or_else(|| eyre!("merge time floor overflow"))?,
                        );
                        candidates.push((
                            transaction.hash_as_entrypoint(),
                            ProvenSource {
                                lane: record.lane,
                                dataspace: record.dataspace,
                                source: AuthenticatedLaneSourceV1 {
                                    incarnation: record.incarnation,
                                    instance: entry.block.header.instance.0,
                                    height: expected_height,
                                    block_hash: qc.block_hash.0,
                                    result: qc.result.0,
                                    batch_index: u32::try_from(index)?,
                                    anchor_height: batch.anchor_height,
                                    anchor_hash: batch.anchor_hash,
                                },
                            },
                        ));
                    }
                }
                history.apply(&entry, &batch);
            }
            ensure!(
                history.hash.0 == merge.tip_hash && history.result.0 == merge.tip_result,
                "original lane certificate differs from the global merge tip"
            );
            if let Some(after) = poststate
                .lane(record.lane)
                .filter(|after| after.incarnation == record.incarnation)
            {
                ensure!(
                    after.merged.height == merge.to
                        && after.merged.block_hash == merge.tip_hash
                        && after.merged.result == merge.tip_result
                        && after.merged_at == height,
                    "certified poststate does not apply the exact merge frontier"
                );
            } else {
                ensure!(
                    record
                        .retirement_height()
                        .is_some_and(|retirement| height >= retirement),
                    "merged lane disappeared before authenticated retirement"
                );
            }
        }
        ensure!(frame_index == frames.len(), "extra lane frames");
        ensure!(
            section.time_floor_ms == time_floor
                && u64::try_from(block.header().creation_time().as_millis())? >= time_floor,
            "merge time floor differs from original fresh lane transactions"
        );
        let inputs = block.external_entrypoints_slice();
        let suffix = inputs
            .len()
            .checked_sub(usize::try_from(section.merged_count)?)
            .ok_or_else(|| eyre!("merged suffix exceeds actual inputs"))?;
        let mut candidates = candidates.into_iter();
        let mut output = BTreeMap::new();
        for (index, entrypoint) in inputs.iter().enumerate().skip(suffix) {
            let context = block
                .execution_context()
                .and_then(|bundle| {
                    bundle
                        .external
                        .iter()
                        .find(|context| context.entrypoint_hash == entrypoint.hash())
                })
                .ok_or_else(|| eyre!("merged transaction has no exact execution route"))?;
            let source = candidates
                .find(|(hash, source)| {
                    *hash == entrypoint.hash()
                        && source.lane == context.lane_id
                        && source.dataspace == context.dataspace_id
                })
                .map(|(_, source)| source)
                .ok_or_else(|| {
                    eyre!("executed merge suffix is not an ordered subset of original lane batches")
                })?;
            output.insert(index, source);
        }
        self.previous = Some(poststate.clone());
        self.anchor(block)?;
        Ok(output)
    }
}

/// Read the lane policy from the independently retained original signed genesis instructions.
/// Duplicate policy assignments are rejected instead of choosing an apparent later authority.
pub(super) fn signed_genesis_policy(block: &SignedBlock) -> Result<SumeragiLanePolicy> {
    use iroha_data_model::{isi::SetParameter, parameter::Parameter};
    ensure!(
        block.header().height().get() == 1,
        "lane policy source is not genesis"
    );
    let mut policy = None;
    for transaction in block.external_transactions() {
        let Executable::Instructions(instructions) = transaction.instructions() else {
            continue;
        };
        for instruction in instructions.iter() {
            let Some(parameter) = instruction.as_any().downcast_ref::<SetParameter>() else {
                continue;
            };
            let Parameter::Custom(custom) = parameter.inner() else {
                continue;
            };
            if let Some(decoded) = SumeragiLanePolicy::from_custom_parameter(custom) {
                ensure!(
                    policy.is_none(),
                    "signed genesis repeats its native lane policy"
                );
                policy = Some(decoded.map_err(|error| eyre!(error))?);
            }
        }
    }
    let policy =
        policy.ok_or_else(|| eyre!("original signed genesis has no native lane policy"))?;
    policy.validate()?;
    Ok(policy)
}

pub(super) fn decode_frame(bytes: &[u8]) -> Result<SyncEntry> {
    bounded(bytes, MAX_CARRIER_BYTES)?;
    let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let entry: SyncEntry = norito::with_decode_limits_scope(decode_limits(bytes.len()), || {
        SyncEntry::decode_all(&mut &bytes[..])
    })?;
    ensure!(entry.encode() == bytes, "noncanonical original lane frame");
    Ok(entry)
}

#[cfg(test)]
mod tests;
