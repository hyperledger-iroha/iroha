//! Genuine lane admission, native quorum and global execution controls.

use super::*;
use crate::kura::scaling_evidence::lane_proof::*;
use std::{
    sync::{Arc, OnceLock},
    time::Duration,
};

use iroha_core::{
    state::{StateReadOnly, World, WorldReadOnly},
    sumeragi::{
        crypto::{BlsCrypto, KeyPairSigner},
        driver::{
            SharedCrypto,
            traits::{BlockStore, Executor},
        },
        lanes::{
            self, LaneBatch,
            executor::{LaneExecutor, LaneTransactions},
            global::{AppliedWatch, GlobalAnchors, StatelessChecks},
            merge::{CommittedLaneBlock, LaneBlockSource},
            registry::LaneStores,
        },
        test_chain::{CertifiedTestChain, TestChainConfig, fixture_validators},
    },
};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    account::Account,
    isi::Register,
    parameter::{Parameter, system::SumeragiParameters},
    sumeragi_lanes::{SumeragiFixedLane, SumeragiLaneMember, SumeragiLaneRoute, SumeragiLaneState},
};
use iroha_model_base::peer::PeerId;
use iroha_sumeragi::{
    api::ExecOutcome,
    crypto::{Signer, form_qc},
    message::{Block, BlockHeader as CoreHeader, SyncEntry, Vote, VoteKind},
    preimage::payload_hash,
    types::{Hash32, SIGNATURE_LEN, Signature},
};

#[derive(Default)]
pub(crate) struct Deferred(OnceLock<Arc<LaneStores>>);
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
struct NoTransactions;
impl LaneTransactions for NoTransactions {
    fn candidates(
        &self,
        _: u64,
        _: usize,
        _: &BTreeSet<HashOf<TransactionEntrypoint>>,
    ) -> Vec<SignedTransaction> {
        Vec::new()
    }
}

pub(crate) struct LaneProducer {
    pub(crate) chain: CertifiedTestChain,
    pub(crate) stores: Arc<LaneStores>,
    pub(crate) policy: SumeragiLanePolicy,
    pub(crate) users: Vec<KeyPair>,
    pub(crate) keys: Vec<KeyPair>,
    pub(crate) prefix: Vec<(SignedBlock, SumeragiLaneState)>,
}
impl LaneProducer {
    pub(crate) fn start(lanes: usize) -> Self {
        let users = (0..4)
            .map(|index| {
                KeyPair::from_seed(
                    vec![0x71 + u8::try_from(index).unwrap(); 32],
                    Algorithm::Ed25519,
                )
            })
            .collect::<Vec<_>>();
        let committee = fixture_validators()
            .into_iter()
            .map(|(peer, pop)| SumeragiLaneMember { peer, pop })
            .collect::<Vec<_>>();
        let policy = SumeragiLanePolicy {
            anchor_freshness: 16,
            max_merge_blocks: 32,
            stall_window: 256,
            lane_params: SumeragiParameters::default(),
            fixed: (1..lanes)
                .map(|index| SumeragiFixedLane {
                    lane: LaneId::new(u32::try_from(index).unwrap()),
                    dataspace: DataSpaceId::UNIVERSAL,
                    committee: committee.clone(),
                })
                .collect(),
            routes: users
                .iter()
                .enumerate()
                .map(|(index, key)| SumeragiLaneRoute {
                    lane: LaneId::new(u32::try_from(index % lanes).unwrap()),
                    account: Some(AccountId::new(key.public_key().clone()).to_string()),
                    instruction: None,
                })
                .collect(),
            autoscale: None,
        };
        let deferred = Arc::new(Deferred::default());
        let mut config = TestChainConfig::new(World::new(), 10_000);
        config
            .genesis_parameters
            .push(Parameter::Custom(policy.clone().into_custom_parameter()));
        config.genesis_instructions.extend(users.iter().map(|key| {
            InstructionBox::from(Register::account(Account::new(AccountId::new(
                key.public_key().clone(),
            ))))
        }));
        config.lane_blocks = deferred.clone();
        let chain =
            CertifiedTestChain::start(config).expect("execute original signed policy genesis");
        Self::from_chain(chain, deferred, policy, users, default_keys())
    }
    pub(crate) fn from_chain(
        chain: CertifiedTestChain,
        deferred: Arc<Deferred>,
        policy: SumeragiLanePolicy,
        users: Vec<KeyPair>,
        keys: Vec<KeyPair>,
    ) -> Self {
        let crypto: SharedCrypto = Arc::new(BlsCrypto::new());
        let stores = Arc::new(LaneStores::new(
            chain.kura().store_root().join("lanes"),
            chain.network_id(),
            chain.state().chain_id_ref().to_string(),
            crypto,
        ));
        assert!(deferred.0.set(Arc::clone(&stores)).is_ok());
        let mut producer = Self {
            chain,
            stores,
            policy,
            users,
            keys,
            prefix: Vec::new(),
        };
        producer.capture();
        producer.chain.commit(Vec::new());
        producer.capture();
        producer.chain.commit(Vec::new());
        producer.capture();
        producer
    }
    pub(crate) fn capture(&mut self) {
        let committed = self.chain.committed(self.chain.height());
        let lanes = self.chain.state().view().world().sumeragi_lanes().clone();
        assert!(
            committed
                .commitment()
                .native_lanes
                .matches_state(self.chain.network_id(), committed.height(), &lanes)
                .unwrap(),
            "original executor R binds exact actual World lane state"
        );
        self.prefix
            .push((committed.block().as_ref().clone(), lanes));
    }
    pub(crate) fn request(&self, lane: usize, logical: &str) -> SignedTransaction {
        let key = &self.users[lane];
        let account = AccountId::new(key.public_key().clone());
        let Executable::Instructions(instructions) =
            expected_executable(&account, logical).unwrap()
        else {
            unreachable!()
        };
        self.chain.sign(
            key,
            instructions.iter().cloned(),
            self.chain.committed(self.chain.height()).block_time_ms(),
        )
    }
    pub(crate) fn certify(&self, lane: usize, transactions: Vec<SignedTransaction>) -> LaneFrameV1 {
        let lane = LaneId::new(u32::try_from(lane).unwrap());
        let record = self
            .chain
            .state()
            .view()
            .world()
            .sumeragi_lanes()
            .lane(lane)
            .unwrap()
            .clone();
        let config = lanes::lane_height_config(&record).unwrap();
        let crypto = BlsCrypto::new();
        crypto
            .admit_committee(
                record
                    .committee
                    .iter()
                    .map(|member| (member.peer.public_key(), member.pop.as_slice())),
            )
            .unwrap();
        let store = self.stores.store(lane, &record.incarnation).unwrap();
        let height = store.height() + 1;
        let parent = store
            .entry(height - 1)
            .map(|entry| (entry.commit_qc.block_hash, entry.commit_qc.result))
            .unwrap_or_else(|| {
                (
                    lanes::lane_genesis_hash(&self.chain.network_id(), &record),
                    lanes::lane_genesis_result(&record),
                )
            });
        let anchor_height = self.chain.height();
        let anchor_hash = self.chain.committed(anchor_height).block().hash();
        let anchors = Arc::new(GlobalAnchors::new(
            Arc::clone(self.chain.state()),
            Arc::new(AppliedWatch::new(anchor_height, Some(anchor_hash))),
        ));
        let mut executor = LaneExecutor::<_, _, NoTransactions>::recover(
            record.clone(),
            config.clone(),
            anchors,
            StatelessChecks::new(self.chain.network_id()),
            None,
            lanes::lane_genesis_hash(&self.chain.network_id(), &record),
            store.as_ref(),
        )
        .unwrap();
        let payload = LaneBatch {
            anchor_height,
            anchor_hash,
            transactions,
        }
        .to_payload();
        let block = Block {
            header: CoreHeader {
                instance: self.stores.instance(lane, &record.incarnation),
                epoch: config.epoch.id,
                height,
                origin_view: 0,
                parent_hash: parent.0,
                parent_result: parent.1,
                payload_hash: payload_hash(&crypto, &payload),
                payload_len: u32::try_from(payload.len()).unwrap(),
                proposer: 0,
                skipped_leaders: Vec::new(),
                attest: false,
                control_witness: Default::default(),
            },
            payload,
        };
        let hash = block.hash(&crypto);
        let Some(ExecOutcome::Valid(result)) = executor.execute(&block, &hash) else {
            panic!("real lane admission failed")
        };
        let commit_qc = signed_qc(&block, result, &self.keys);
        let entry = SyncEntry { block, commit_qc };
        assert_eq!(
            executor.prepare(&entry.block, &entry.commit_qc).unwrap(),
            Some(result)
        );
        store.append(&entry.block, &entry.commit_qc).unwrap();
        executor.commit(&entry.block, &entry.commit_qc).unwrap();
        let original = std::fs::read(
            self.chain
                .kura()
                .store_root()
                .join("lanes")
                .join(hex::encode(entry.block.header.instance.0))
                .join(format!("{:020}.frame", height)),
        )
        .unwrap();
        assert_eq!(
            original,
            entry.encode(),
            "original stored frame must be the executed and signed value"
        );
        LaneFrameV1 {
            lane,
            frame: original,
        }
    }
    pub(crate) fn proof_owner(&self) -> LaneProofState {
        let mut proof = LaneProofState::default();
        proof
            .anchor_genesis(&self.prefix[0].0, self.prefix[0].1.clone())
            .unwrap();
        for (block, lanes) in &self.prefix[1..] {
            proof
                .verify(
                    block,
                    lanes,
                    &[],
                    &self.policy,
                    self.chain.network_id(),
                    self.chain.state().view().chain_id(),
                )
                .unwrap();
        }
        proof
    }
}

pub(crate) fn resign_lane(entry: &mut SyncEntry, keys: &[KeyPair]) {
    entry.commit_qc = signed_qc(&entry.block, entry.commit_qc.result, keys);
}
pub(crate) fn signed_qc(
    block: &Block,
    result: Hash32,
    keys: &[KeyPair],
) -> iroha_sumeragi::message::Qc {
    let crypto = BlsCrypto::new();
    let header = &block.header;
    let votes = keys
        .iter()
        .take(3)
        .enumerate()
        .map(|(index, key)| {
            let signer = KeyPairSigner::new(key).unwrap();
            let mut vote = Vote {
                kind: VoteKind::Commit,
                instance: header.instance,
                epoch: header.epoch,
                height: header.height,
                view: 0,
                block_hash: block.hash(&crypto),
                result,
                attest: false,
                signer: u32::try_from(index).unwrap(),
                sig: Signature([0; SIGNATURE_LEN]),
                attestation: None,
            };
            vote.sig = signer.sign(&vote.preimage());
            vote
        })
        .collect::<Vec<_>>();
    form_qc(&crypto, 4, &votes.iter().collect::<Vec<_>>()).unwrap()
}

fn default_keys() -> Vec<KeyPair> {
    let mut keys = (0xC1..=0xC4)
        .map(|seed| KeyPair::from_seed(vec![seed; 32], Algorithm::BlsNormal))
        .collect::<Vec<_>>();
    keys.sort_by_key(|key| PeerId::new(key.public_key().clone()));
    keys
}
