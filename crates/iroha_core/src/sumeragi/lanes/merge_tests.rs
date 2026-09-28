//! The lane merge end to end on a certified test chain: a genesis lane policy creates a fixed
//! lane, certified lane blocks land in the node's lane store, and the chain's blocks merge them.

use std::sync::{Arc, OnceLock};

use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    IntoKeyValue, Level, Registrable,
    account::{Account, AccountId},
    isi::{InstructionBox, Log},
    parameter::{Parameter, system::SumeragiParameters},
    sumeragi_lanes::{
        SumeragiFixedLane, SumeragiLaneMember, SumeragiLaneRecord, SumeragiLaneRoute,
    },
};
use iroha_model_base::topology::DataSpaceId;
use iroha_sumeragi::{
    message::{Block, BlockHeader, Qc, VoteKind},
    preimage::payload_hash,
    types::{AggregateSignature, Bitmap, SIGNATURE_LEN},
};

use super::*;
use crate::{
    state::World,
    sumeragi::{
        crypto::BlsCrypto,
        driver::{SharedCrypto, traits::BlockStore as _},
        payload::{self, Assembly},
        test_chain::{CertifiedTestChain, TestChainConfig, fixture_validators},
    },
};

const LANE: LaneId = LaneId::new(2);
const GENESIS_MS: u64 = 10_000;

/// The lane stores, bound once the chain (and so its network id) exists.
#[derive(Default)]
struct Deferred(OnceLock<Arc<super::super::registry::LaneStores>>);

impl LaneBlockSource for Deferred {
    fn tip(&self, lane: LaneId, incarnation: &[u8; 32]) -> Option<u64> {
        self.0.get()?.tip(lane, incarnation)
    }
    fn block(
        &self,
        lane: LaneId,
        incarnation: &[u8; 32],
        height: u64,
    ) -> Option<CommittedLaneBlock> {
        self.0.get()?.block(lane, incarnation, height)
    }
    fn wait_for(
        &self,
        lane: LaneId,
        incarnation: &[u8; 32],
        height: u64,
        timeout: Duration,
    ) -> bool {
        self.0
            .get()
            .is_some_and(|stores| stores.wait_for(lane, incarnation, height, timeout))
    }
}

fn key(seed: u8) -> KeyPair {
    KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519)
}

/// Transactions of this account route to the fixed lane.
fn lane_user() -> KeyPair {
    key(0x51)
}

/// Transactions of this account take the default route (lane 0).
fn other_user() -> KeyPair {
    key(0x52)
}

fn policy() -> SumeragiLanePolicy {
    SumeragiLanePolicy {
        anchor_freshness: 4,
        max_merge_blocks: 8,
        stall_window: 1_000,
        lane_params: SumeragiParameters::default(),
        fixed: vec![SumeragiFixedLane {
            lane: LANE,
            dataspace: DataSpaceId::new(0),
            committee: fixture_validators()
                .into_iter()
                .map(|(peer, pop)| SumeragiLaneMember { peer, pop })
                .collect(),
        }],
        routes: vec![SumeragiLaneRoute {
            lane: LANE,
            account: Some(AccountId::new(lane_user().public_key().clone()).to_string()),
            instruction: None,
        }],
        autoscale: None,
    }
}

struct Fixture {
    chain: CertifiedTestChain,
    stores: Arc<super::super::registry::LaneStores>,
    crypto: SharedCrypto,
    _dir: tempfile::TempDir,
}

impl Fixture {
    fn start() -> Self {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut world = World::default();
        for key in [lane_user(), other_user()] {
            let id = AccountId::new(key.public_key().clone());
            let (id, value) = Account::new(id.clone()).build(&id).into_key_value();
            world.accounts.insert(id, value);
        }
        let deferred = Arc::new(Deferred::default());
        let mut config = TestChainConfig::new(world, GENESIS_MS);
        config
            .genesis_parameters
            .push(Parameter::Custom(policy().into_custom_parameter()));
        config.lane_blocks = deferred.clone();
        let chain = CertifiedTestChain::start(config).expect("the chain starts");
        let crypto: SharedCrypto = Arc::new(BlsCrypto::new());
        let stores = Arc::new(super::super::registry::LaneStores::new(
            dir.path().to_path_buf(),
            chain.network_id(),
            chain.state().view().chain_id().to_string(),
            Arc::clone(&crypto),
        ));
        assert!(deferred.0.set(Arc::clone(&stores)).is_ok());
        Self {
            chain,
            stores,
            crypto,
            _dir: dir,
        }
    }

    fn record(&self) -> SumeragiLaneRecord {
        self.chain
            .state()
            .view()
            .world()
            .sumeragi_lanes()
            .lane(LANE)
            .cloned()
            .expect("the fixed lane exists")
    }

    fn anchor(&self, height: u64) -> iroha_crypto::HashOf<iroha_data_model::block::BlockHeader> {
        let view = self.chain.state().view();
        view.block_hashes()
            .iter()
            .nth(usize::try_from(height - 1).expect("small"))
            .copied()
            .expect("an applied anchor")
    }

    fn log(&self, key: &KeyPair, text: &str, created_ms: u64) -> SignedTransaction {
        self.chain.sign(
            key,
            [InstructionBox::from(Log::new(Level::INFO, text.to_owned()))],
            created_ms,
        )
    }

    /// Store a certified lane block at `height` carrying `transactions` anchored at `anchor`.
    fn certify(&self, height: u64, anchor: u64, transactions: Vec<SignedTransaction>) -> Hash32 {
        let record = self.record();
        let store = self
            .stores
            .store(LANE, &record.incarnation)
            .expect("lane store");
        let instance = self.stores.instance(LANE, &record.incarnation);
        let parent = if height == 1 {
            Hash32(record.merged.block_hash)
        } else {
            store
                .entry(height - 1)
                .expect("the parent lane block")
                .commit_qc
                .block_hash
        };
        let payload = LaneBatch {
            anchor_height: anchor,
            anchor_hash: self.anchor(anchor),
            transactions,
        }
        .to_payload();
        let block = Block {
            header: BlockHeader {
                instance,
                height,
                origin_view: 0,
                parent_hash: parent,
                parent_result: Hash32([0; 32]),
                payload_hash: payload_hash(&*self.crypto, &payload),
                payload_len: u32::try_from(payload.len()).expect("small"),
                proposer: 0,
                skipped_leaders: Vec::new(),
                attest: false,
            },
            payload,
        };
        let block_hash = block.hash(&*self.crypto);
        let qc = Qc {
            kind: VoteKind::Commit,
            instance,
            height,
            view: 0,
            block_hash,
            result: Hash32([u8::try_from(height).expect("small"); 32]),
            attest: false,
            signers: Bitmap::from_indices(4, [0, 1, 2]).expect("bitmap"),
            agg_sig: AggregateSignature([0; SIGNATURE_LEN]),
            attestations: Vec::new(),
        };
        store.append(&block, &qc).expect("append");
        block_hash
    }

    fn committed(&self, tx: &SignedTransaction) -> bool {
        self.chain
            .state()
            .view()
            .has_entrypoint(tx.hash_as_entrypoint())
    }

    /// A merge-only proposal for the next height carrying `merges`.
    fn proposal(&self, merges: &[SumeragiLaneMerge]) -> SignedBlock {
        let view = self.chain.state().view();
        let parent = view.latest_block().expect("parent");
        drop(view);
        payload::assemble_with_merges(
            self.chain.state(),
            Assembly {
                parent: &parent,
                view: 0,
                cadence: Duration::from_millis(1),
            },
            &[],
            merges,
            None,
        )
        .expect("a merge-only proposal")
    }
}

#[test]
fn global_blocks_merge_fresh_lane_blocks_and_drop_what_they_must_not_execute() {
    let mut fixture = Fixture::start();
    let record = fixture.record();
    assert_eq!(
        record.created_at, 1,
        "genesis creates the policy's fixed lane"
    );
    assert_eq!(record.active_from, 3);
    fixture.chain.commit(Vec::new());
    fixture.chain.commit(Vec::new());
    assert_eq!(fixture.chain.height(), 3);

    // Lane block 1, anchored at global height 3: a transaction routed to the lane, one routed
    // to lane 0 (misrouted here) and a repeat of the first.
    let routed = fixture.log(&lane_user(), "lane", GENESIS_MS - 10);
    let misrouted = fixture.log(&other_user(), "misrouted", GENESIS_MS - 9);
    let tip = fixture.certify(
        1,
        3,
        vec![routed.clone(), misrouted.clone(), routed.clone()],
    );
    fixture.chain.commit(Vec::new());
    let merged = fixture.chain.committed(4);
    assert_eq!(merged.block().merged_entrypoint_count(), 1);
    assert!(
        fixture.committed(&routed),
        "the routed transaction executes"
    );
    assert!(
        !fixture.committed(&misrouted),
        "a misrouted one has no effect"
    );
    let record = fixture.record();
    assert_eq!(record.merged.height, 1);
    assert_eq!(record.merged.block_hash, tip.0);
    assert_eq!(record.merged_at, 4);

    // Blocks without new lane blocks merge nothing.
    fixture.chain.commit(Vec::new());
    fixture.chain.commit(Vec::new());
    fixture.chain.commit(Vec::new());
    assert_eq!(fixture.record().merged.height, 1);

    // Lane block 2, anchored at 3, merged at 8 > 3 + A: stale, merged without effect.
    let late = fixture.log(&lane_user(), "late", GENESIS_MS - 8);
    fixture.certify(2, 3, vec![late.clone()]);
    fixture.chain.commit(Vec::new());
    assert_eq!(
        fixture.chain.committed(8).block().merged_entrypoint_count(),
        0
    );
    assert!(
        !fixture.committed(&late),
        "a stale block's transactions do not execute"
    );
    assert_eq!(
        fixture.record().merged.height,
        2,
        "the frontier moves past it"
    );
}

#[test]
fn malformed_merges_are_invalid_and_missing_blocks_pending() {
    let mut fixture = Fixture::start();
    fixture.chain.commit(Vec::new());
    fixture.chain.commit(Vec::new());
    let tx = fixture.log(&lane_user(), "lane", GENESIS_MS - 10);
    let tip = fixture.certify(1, 3, vec![tx]);
    let record = fixture.record();
    let merge = SumeragiLaneMerge {
        lane: LANE,
        incarnation: record.incarnation,
        from: 1,
        to: 1,
        tip_hash: tip.0,
        tip_result: [1; 32],
    };
    let expand_at = |merges: &[SumeragiLaneMerge]| {
        expand(
            &fixture.chain.state().view(),
            &fixture.proposal(merges),
            &*fixture.stores,
            Duration::ZERO,
        )
    };
    assert!(expand_at(&[merge]).is_ok());
    // A tip that is not the lane's committed block, a gap, another incarnation, a lane that
    // does not exist: invalid.
    for bad in [
        SumeragiLaneMerge {
            tip_hash: [9; 32],
            ..merge
        },
        SumeragiLaneMerge { from: 2, ..merge },
        SumeragiLaneMerge {
            incarnation: [7; 32],
            ..merge
        },
        SumeragiLaneMerge {
            lane: LaneId::new(5),
            ..merge
        },
    ] {
        assert!(
            matches!(expand_at(&[bad]), Err(MergeError::Invalid(_))),
            "{bad:?} is invalid"
        );
    }
    // Heights the local store has not committed: pending, never a verdict.
    assert!(matches!(
        expand_at(&[SumeragiLaneMerge { to: 2, ..merge }]),
        Err(MergeError::Pending(_))
    ));
    // A leader's proposal merges what the store holds.
    let (proposed, reserved) = propose(
        &fixture.chain.state().view(),
        &*fixture.stores,
        fixture.chain.height() + 1,
    );
    assert_eq!(proposed, vec![merge]);
    assert_eq!(reserved, 1);
}
