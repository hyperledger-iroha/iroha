//! The executor of a lane instance (`specs/sumeragi_lanes.md` §3): lane execute-before-vote is
//! admission ([`super::admit`]); commit makes a block's chain facts the applied lane state; the
//! payload builder anchors a batch at the global chain's applied tip.

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::Arc,
    time::Duration,
};

use iroha_crypto::HashOf;
use iroha_data_model::{
    block::BlockHeader,
    sumeragi_lanes::SumeragiLaneRecord,
    transaction::{SignedTransaction, TransactionEntrypoint},
};
use iroha_sumeragi::{
    api::ExecOutcome,
    message::{Block, Qc},
    types::{Hash32, HeightConfig},
};

use super::{
    Admission, AnchorView, LANE_DEDUP_WINDOW, LaneBatch, LaneChainView, TransactionCheck, admit,
};
use crate::sumeragi::driver::traits::{BlockStore, Executor};

/// The global chain as a lane executor sees it: anchors, and a way to wait for one.
pub trait AnchorSource: AnchorView + Send + Sync {
    /// Block until the node has applied global height `height` or `timeout` passes; whether it
    /// has.
    fn wait_for(&self, height: u64, timeout: Duration) -> bool;
    /// The applied global tip: its height and block hash.
    fn tip(&self) -> (u64, HashOf<BlockHeader>);
}

/// Transactions the lane's payload builder may propose.
pub trait LaneTransactions: Send + Sync {
    /// Queued transactions routed to the lane at global height `height`, oldest first, at most
    /// `max_bytes` of encoded transactions, skipping `skip`.
    fn candidates(
        &self,
        height: u64,
        max_bytes: usize,
        skip: &BTreeSet<HashOf<TransactionEntrypoint>>,
    ) -> Vec<SignedTransaction>;
}

/// The chain facts after a lane block: its anchor and the anchored transactions of the last
/// [`LANE_DEDUP_WINDOW`] blocks.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct ChainState {
    anchor: u64,
    window: VecDeque<(u64, Vec<HashOf<TransactionEntrypoint>>)>,
}

impl ChainState {
    fn view(&self) -> LaneChainView {
        let mut recent = BTreeMap::new();
        for (anchor, hashes) in &self.window {
            for hash in hashes {
                // Anchors never regress along the chain: later blocks overwrite.
                recent.insert(*hash, *anchor);
            }
        }
        LaneChainView {
            previous_anchor: self.anchor,
            recent,
        }
    }

    fn after(&self, anchor: u64, txs: Vec<HashOf<TransactionEntrypoint>>) -> Self {
        let mut window = self.window.clone();
        window.push_back((anchor, txs));
        while window.len() > LANE_DEDUP_WINDOW {
            window.pop_front();
        }
        Self { anchor, window }
    }
}

#[derive(Clone, Debug)]
struct Executed {
    height: u64,
    parent: Hash32,
    result: Hash32,
    state: ChainState,
}

#[derive(Clone, Debug)]
struct Applied {
    height: u64,
    block_hash: Hash32,
    state: ChainState,
}

/// The executor of one lane instance.
pub struct LaneExecutor<A, C, T> {
    record: SumeragiLaneRecord,
    config: HeightConfig,
    anchors: Arc<A>,
    checks: C,
    transactions: Option<Arc<T>>,
    anchor_wait: Duration,
    applied: Applied,
    cache: BTreeMap<Hash32, Executed>,
}

impl<A, C, T> core::fmt::Debug for LaneExecutor<A, C, T> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("LaneExecutor")
            .field("lane", &self.record.lane)
            .field("applied", &self.applied.height)
            .field("cached", &self.cache.len())
            .finish_non_exhaustive()
    }
}

impl<A: AnchorSource, C: TransactionCheck + Send, T: LaneTransactions> LaneExecutor<A, C, T> {
    /// The executor of `record`'s instance over the lane blocks `store` holds: the applied state
    /// is rebuilt from the stored tip and its last [`LANE_DEDUP_WINDOW`] blocks.
    ///
    /// # Errors
    /// A stored block that is missing or whose payload is not a lane batch.
    pub fn recover(
        record: SumeragiLaneRecord,
        config: HeightConfig,
        anchors: Arc<A>,
        checks: C,
        transactions: Option<Arc<T>>,
        genesis_hash: Hash32,
        store: &dyn BlockStore,
    ) -> Result<Self, String> {
        let tip = store.height();
        let mut state = ChainState::default();
        let mut block_hash = genesis_hash;
        let first = tip
            .saturating_sub(u64::try_from(LANE_DEDUP_WINDOW).unwrap_or(u64::MAX))
            .saturating_add(1);
        for height in first.max(1)..=tip {
            let entry = store
                .entry(height)
                .ok_or_else(|| format!("lane block {height} is missing from the store"))?;
            let batch = LaneBatch::from_payload(&entry.block.payload)
                .map_err(|error| format!("lane block {height}: {error}"))?;
            state = state.after(
                batch.anchor_height,
                batch
                    .transactions
                    .iter()
                    .map(SignedTransaction::hash_as_entrypoint)
                    .collect(),
            );
            block_hash = entry.commit_qc.block_hash;
        }
        Ok(Self {
            anchor_wait: Duration::from_millis(config.params.e_max.into()),
            record,
            config,
            anchors,
            checks,
            transactions,
            applied: Applied {
                height: tip,
                block_hash,
                state,
            },
            cache: BTreeMap::new(),
        })
    }

    fn parent_state(&self, block: &Block) -> Option<ChainState> {
        let parent = block.header.parent_hash;
        if parent == self.applied.block_hash && block.header.height == self.applied.height + 1 {
            return Some(self.applied.state.clone());
        }
        self.cache
            .get(&parent)
            .filter(|executed| executed.height + 1 == block.header.height)
            .map(|executed| executed.state.clone())
    }

    fn run(&mut self, block: &Block, block_hash: &Hash32, parent: ChainState) -> ExecOutcome {
        let admission = |executor: &Self| {
            admit(
                &executor.record,
                &*executor.anchors,
                &parent.view(),
                &executor.checks,
                &executor.config,
                &block.payload,
            )
        };
        let mut outcome = admission(self);
        if let Ok(Admission::Pending) = outcome
            && let Ok(batch) = LaneBatch::from_payload(&block.payload)
            && self.anchors.wait_for(batch.anchor_height, self.anchor_wait)
        {
            outcome = admission(self);
        }
        match outcome {
            Ok(Admission::Valid(result)) => {
                let r = result.hash();
                let state = parent.after(result.anchor_height, result.tx_hashes);
                self.cache.insert(
                    *block_hash,
                    Executed {
                        height: block.header.height,
                        parent: block.header.parent_hash,
                        result: r,
                        state,
                    },
                );
                ExecOutcome::Valid(r)
            }
            Ok(Admission::Pending) => {
                ExecOutcome::Failed("the lane block's anchor is not applied yet".into())
            }
            Err(error) => {
                iroha_logger::debug!(lane = %self.record.lane, %error, "lane block is not admissible");
                ExecOutcome::Invalid
            }
        }
    }
}

impl<A: AnchorSource, C: TransactionCheck + Send, T: LaneTransactions> Executor
    for LaneExecutor<A, C, T>
{
    fn execute(&mut self, block: &Block, block_hash: &Hash32) -> Option<ExecOutcome> {
        if let Some(executed) = self.cache.get(block_hash) {
            return Some(ExecOutcome::Valid(executed.result));
        }
        let parent = self.parent_state(block)?;
        Some(self.run(block, block_hash, parent))
    }

    fn discard(&mut self, height: u64, keep: &[Hash32]) {
        self.cache
            .retain(|hash, executed| executed.height != height || keep.contains(hash));
    }

    fn prepare(&mut self, block: &Block, commit_qc: &Qc) -> Result<Option<Hash32>, String> {
        let hash = commit_qc.block_hash;
        if let Some(executed) = self.cache.get(&hash)
            && executed.parent == self.applied.block_hash
        {
            return Ok((executed.result == commit_qc.result).then_some(executed.result));
        }
        let Some(parent) = self.parent_state(block) else {
            return Err("the committed lane block's parent is not applied".into());
        };
        match self.run(block, &hash, parent) {
            ExecOutcome::Valid(result) => Ok((result == commit_qc.result).then_some(result)),
            ExecOutcome::Invalid | ExecOutcome::Cancelled => Ok(None),
            ExecOutcome::Failed(reason) => Err(reason),
        }
    }

    fn commit(&mut self, block: &Block, commit_qc: &Qc) -> Result<HeightConfig, String> {
        let executed = self
            .cache
            .remove(&commit_qc.block_hash)
            .ok_or_else(|| "the committed lane block is not prepared".to_string())?;
        self.applied = Applied {
            height: block.header.height,
            block_hash: commit_qc.block_hash,
            state: executed.state,
        };
        let height = self.applied.height;
        self.cache.retain(|_, executed| executed.height > height);
        Ok(self.config.clone())
    }

    fn build(
        &mut self,
        _height: u64,
        _view: u64,
        max_bytes: u32,
        _exec_budget_ms: u32,
    ) -> (Vec<u8>, bool) {
        let Some(transactions) = self.transactions.as_ref() else {
            return (Vec::new(), false);
        };
        let (anchor_height, anchor_hash) = self.anchors.tip();
        if !self.record.admits_anchor(anchor_height) || anchor_height < self.applied.state.anchor {
            return (Vec::new(), false);
        }
        // Skip what the lane still carries: recent blocks the global chain may merge fresh, and
        // executed, uncommitted blocks.
        let view = self.applied.state.view();
        let mut skip = view
            .recent
            .keys()
            .filter(|hash| view.repeats(hash, anchor_height, self.record.anchor_freshness))
            .copied()
            .collect::<BTreeSet<_>>();
        for executed in self.cache.values() {
            if let Some((_, hashes)) = executed.state.window.back() {
                skip.extend(hashes.iter().copied());
            }
        }
        // Leave room for the batch framing.
        let budget = usize::try_from(max_bytes)
            .unwrap_or(usize::MAX)
            .saturating_sub(64);
        let selected = transactions.candidates(anchor_height, budget, &skip);
        if selected.is_empty() {
            return (Vec::new(), false);
        }
        let payload = LaneBatch {
            anchor_height,
            anchor_hash,
            transactions: selected,
        }
        .to_payload();
        if u32::try_from(payload.len()).map_or(true, |len| len > max_bytes) {
            return (Vec::new(), false);
        }
        (payload, false)
    }

    fn reject(&mut self, height: u64, view: u64, block_hash: &Hash32) {
        iroha_logger::debug!(
            lane = %self.record.lane,
            height,
            view,
            block = %hex::encode(block_hash.0),
            "a lane block was not admissible"
        );
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use iroha_crypto::{Algorithm, Hash, KeyPair};
    use iroha_data_model::{
        NetworkId,
        account::AccountId,
        isi::{InstructionBox, Log},
        parameter::system::SumeragiParameters,
        sumeragi_lanes::{SumeragiLaneFrontier, SumeragiLaneMember},
        transaction::{FeePaymentIntent, TransactionBuilder},
    };
    use iroha_model_base::{
        peer::PeerId,
        topology::{DataSpaceId, LaneId},
    };
    use iroha_sumeragi::{
        message::{BlockHeader as CoreHeader, VoteKind},
        preimage::payload_hash,
        testing::FakeCrypto,
        types::{AggregateSignature, Bitmap, SIGNATURE_LEN},
    };
    use parking_lot::Mutex;

    use super::*;
    use crate::sumeragi::lanes::lane_height_config;

    fn anchor_hash(height: u64) -> HashOf<BlockHeader> {
        HashOf::from_untyped_unchecked(Hash::prehashed([u8::try_from(height).unwrap(); 32]))
    }

    /// The global chain: applied up to `applied`.
    struct Global {
        applied: AtomicU64,
    }

    impl AnchorView for Global {
        fn applied_hash(&self, height: u64) -> Option<HashOf<BlockHeader>> {
            (height <= self.applied.load(Ordering::SeqCst)).then(|| anchor_hash(height))
        }
        fn creation_time_ms(&self, height: u64) -> Option<u64> {
            (height <= self.applied.load(Ordering::SeqCst)).then_some(height * 1000)
        }
    }

    impl AnchorSource for Global {
        fn wait_for(&self, height: u64, _timeout: Duration) -> bool {
            height <= self.applied.load(Ordering::SeqCst)
        }
        fn tip(&self) -> (u64, HashOf<BlockHeader>) {
            let height = self.applied.load(Ordering::SeqCst);
            (height, anchor_hash(height))
        }
    }

    struct Accept;
    impl TransactionCheck for Accept {
        fn check(&self, _tx: &SignedTransaction, _time: u64) -> Result<(), String> {
            Ok(())
        }
    }

    struct Queue(Mutex<Vec<SignedTransaction>>);
    impl LaneTransactions for Queue {
        fn candidates(
            &self,
            _height: u64,
            _max_bytes: usize,
            skip: &BTreeSet<HashOf<TransactionEntrypoint>>,
        ) -> Vec<SignedTransaction> {
            self.0
                .lock()
                .iter()
                .filter(|tx| !skip.contains(&tx.hash_as_entrypoint()))
                .cloned()
                .collect()
        }
    }

    struct NoStore;
    impl BlockStore for NoStore {
        fn height(&self) -> u64 {
            0
        }
        fn entry(&self, _height: u64) -> Option<iroha_sumeragi::message::SyncEntry> {
            None
        }
        fn append(&self, _block: &Block, _qc: &Qc) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn member(seed: u8) -> SumeragiLaneMember {
        let pair = KeyPair::from_seed(vec![seed; 32], Algorithm::BlsNormal);
        SumeragiLaneMember {
            peer: PeerId::new(pair.public_key().clone()),
            pop: iroha_crypto::bls_normal_pop_prove(pair.private_key()).expect("pop"),
        }
    }

    fn record() -> SumeragiLaneRecord {
        SumeragiLaneRecord {
            lane: LaneId::new(16),
            dataspace: DataSpaceId::new(0),
            incarnation: [4; 32],
            params: SumeragiParameters::default(),
            committee: vec![member(1)],
            created_at: 1,
            active_from: 3,
            closing: None,
            anchor_freshness: 16,
            merged: SumeragiLaneFrontier::default(),
            merged_at: 3,
            rescued: 0,
        }
    }

    fn tx(seed: u8) -> SignedTransaction {
        let pair = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
        TransactionBuilder::new(
            NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(b"lane"))),
            AccountId::new(pair.public_key().clone()),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([InstructionBox::from(Log::new(
            iroha_data_model::Level::INFO,
            format!("{seed}"),
        ))])
        .sign(pair.private_key())
    }

    fn block(height: u64, parent: Hash32, payload: Vec<u8>) -> Block {
        let crypto = FakeCrypto::new();
        Block {
            header: CoreHeader {
                instance: Hash32([1; 32]),
                height,
                origin_view: 0,
                parent_hash: parent,
                parent_result: Hash32([0; 32]),
                payload_hash: payload_hash(&crypto, &payload),
                payload_len: u32::try_from(payload.len()).unwrap(),
                proposer: 0,
                skipped_leaders: Vec::new(),
                attest: false,
            },
            payload,
        }
    }

    fn qc(block: &Block, result: Hash32) -> Qc {
        Qc {
            kind: VoteKind::Commit,
            instance: block.header.instance,
            height: block.header.height,
            view: 0,
            block_hash: block.hash(&FakeCrypto::new()),
            result,
            attest: false,
            signers: Bitmap::from_indices(1, [0]).unwrap(),
            agg_sig: AggregateSignature([0; SIGNATURE_LEN]),
            attestations: Vec::new(),
        }
    }

    fn executor(
        global: &Arc<Global>,
        queue: Option<Arc<Queue>>,
    ) -> LaneExecutor<Global, Accept, Queue> {
        let record = record();
        let config = lane_height_config(&record).expect("config");
        LaneExecutor::recover(
            record,
            config,
            Arc::clone(global),
            Accept,
            queue,
            Hash32([0; 32]),
            &NoStore,
        )
        .expect("recover")
    }

    #[test]
    fn builds_executes_commits_and_deduplicates() {
        let global = Arc::new(Global {
            applied: AtomicU64::new(5),
        });
        let first = tx(1);
        let queue = Arc::new(Queue(Mutex::new(vec![first.clone()])));
        let mut lane = executor(&global, Some(Arc::clone(&queue)));
        let (payload, attest) = lane.build(1, 0, 1 << 20, 100);
        assert!(!attest);
        let batch = LaneBatch::from_payload(&payload).expect("batch");
        assert_eq!(batch.anchor_height, 5);
        assert_eq!(batch.transactions, vec![first.clone()]);
        let b1 = block(1, Hash32([0; 32]), payload);
        let h1 = b1.hash(&FakeCrypto::new());
        let Some(ExecOutcome::Valid(r1)) = lane.execute(&b1, &h1) else {
            panic!("valid");
        };
        assert_eq!(lane.prepare(&b1, &qc(&b1, r1)), Ok(Some(r1)));
        lane.commit(&b1, &qc(&b1, r1)).expect("commit");
        // The committed transaction is not proposed again, and a block repeating it is invalid.
        assert!(lane.build(2, 0, 1 << 20, 100).0.is_empty());
        let repeat = LaneBatch {
            anchor_height: 5,
            anchor_hash: anchor_hash(5),
            transactions: vec![first],
        }
        .to_payload();
        let b2 = block(2, h1, repeat);
        assert_eq!(
            lane.execute(&b2, &b2.hash(&FakeCrypto::new())),
            Some(ExecOutcome::Invalid)
        );
        // A block whose parent is neither applied nor executed is parked.
        let orphan = block(3, Hash32([9; 32]), Vec::new());
        assert_eq!(lane.execute(&orphan, &Hash32([8; 32])), None);
    }

    #[test]
    fn an_anchor_the_node_has_not_applied_is_never_a_verdict() {
        let global = Arc::new(Global {
            applied: AtomicU64::new(4),
        });
        let mut lane = executor(&global, None);
        let ahead = LaneBatch {
            anchor_height: 6,
            anchor_hash: anchor_hash(6),
            transactions: vec![tx(2)],
        }
        .to_payload();
        let b1 = block(1, Hash32([0; 32]), ahead);
        let h1 = b1.hash(&FakeCrypto::new());
        assert!(matches!(
            lane.execute(&b1, &h1),
            Some(ExecOutcome::Failed(_))
        ));
        global.applied.store(6, Ordering::SeqCst);
        assert!(matches!(
            lane.execute(&b1, &h1),
            Some(ExecOutcome::Valid(_))
        ));
        // A lane without a transaction source builds nothing.
        assert!(lane.build(1, 0, 1 << 20, 100).0.is_empty());
    }
}
