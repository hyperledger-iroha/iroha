//! Original LaneExecutor results and real BLS quorum controls for historical lane evidence.

use std::{collections::BTreeSet, sync::Arc, time::Duration};

use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
use iroha_data_model::{
    account::AccountId,
    block::BlockHeader as GlobalHeader,
    isi::Log,
    sumeragi_lanes::SumeragiLaneMember,
    transaction::{FeePaymentIntent, SignedTransaction, TransactionBuilder, TransactionEntrypoint},
};
use iroha_model_base::{
    peer::PeerId,
    topology::{DataSpaceId, LaneId},
};
use iroha_sumeragi::{
    api::ExecOutcome,
    crypto::{Crypto as _, Signer as _},
    message::{Block, BlockHeader, Qc},
    preimage::payload_hash,
    types::{AggregateSignature, Bitmap, ControlWitness, SIGNATURE_LEN},
};

use super::*;
use crate::sumeragi::{
    crypto::KeyPairSigner,
    driver::traits::{BlockStore, Executor as _},
    lanes::{
        LaneBatch,
        executor::{AnchorSource, LaneExecutor, LaneTransactions},
    },
};

struct Anchors {
    hash: HashOf<GlobalHeader>,
    available: bool,
}
impl AnchorView for Anchors {
    fn applied_hash(&self, height: u64) -> Option<HashOf<GlobalHeader>> {
        (self.available && height == 3).then_some(self.hash)
    }
    fn creation_time_ms(&self, height: u64) -> Option<u64> {
        self.applied_hash(height).map(|_| 10_000)
    }
}
impl AnchorSource for Anchors {
    fn wait_for(&self, height: u64, _timeout: Duration) -> bool {
        self.applied_hash(height).is_some()
    }
    fn tip(&self) -> (u64, HashOf<GlobalHeader>) {
        (3, self.hash)
    }
}
struct NoTransactions;
impl LaneTransactions for NoTransactions {
    fn candidates(
        &self,
        _height: u64,
        _max_bytes: usize,
        _skip: &BTreeSet<HashOf<TransactionEntrypoint>>,
    ) -> Vec<SignedTransaction> {
        Vec::new()
    }
}
struct EmptyStore;
impl BlockStore for EmptyStore {
    fn height(&self) -> u64 {
        0
    }
    fn entry(&self, _height: u64) -> Option<SyncEntry> {
        None
    }
    fn append(&self, _block: &Block, _qc: &Qc) -> std::io::Result<()> {
        panic!("the executor fixture must not publish while constructing evidence")
    }
}

struct Fixture {
    record: SumeragiLaneRecord,
    network: NetworkId,
    anchors: Arc<Anchors>,
    prior: SumeragiLaneFrontier,
    entry: SyncEntry,
    keys: Vec<KeyPair>,
}
const CHAIN: &str = "native-lane-entry-test";
impl Fixture {
    fn new() -> Self {
        let network = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
            b"lane verifier network",
        )));
        let anchors = Arc::new(Anchors {
            hash: HashOf::from_untyped_unchecked(Hash::new(b"global anchor at three")),
            available: true,
        });
        let mut keys = (1..=4)
            .map(|seed| KeyPair::from_seed(vec![seed; 32], Algorithm::BlsNormal))
            .collect::<Vec<_>>();
        keys.sort_by_key(|key| PeerId::new(key.public_key().clone()));
        let mut record = SumeragiLaneRecord {
            lane: LaneId::new(2),
            dataspace: DataSpaceId::new(0),
            incarnation: [0x73; 32],
            params: Default::default(),
            committee: keys
                .iter()
                .map(|key| SumeragiLaneMember {
                    peer: PeerId::new(key.public_key().clone()),
                    pop: iroha_crypto::bls_normal_pop_prove(key.private_key()).unwrap(),
                })
                .collect(),
            created_at: 1,
            active_from: 3,
            closing: None,
            anchor_freshness: 16,
            merged: SumeragiLaneFrontier::default(),
            merged_at: 3,
            rescued: 0,
        };
        let prior = SumeragiLaneFrontier {
            height: 0,
            block_hash: lane_genesis_hash(&network, &record).0,
            result: lane_genesis_result(&record).0,
        };
        record.merged = prior;
        let user = KeyPair::from_seed(vec![8; 32], Algorithm::Ed25519);
        let mut tx = TransactionBuilder::new(
            network,
            AccountId::new(user.public_key().clone()),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(
            iroha_data_model::Level::INFO,
            "lane verifier execution".to_owned(),
        )]);
        tx.set_creation_time(Duration::from_millis(9_000));
        let batch = LaneBatch {
            anchor_height: 3,
            anchor_hash: anchors.hash,
            transactions: vec![tx.sign(user.private_key())],
        };
        let config = lane_height_config(&record).unwrap();
        let crypto = BlsCrypto::new();
        let payload = batch.to_payload();
        let block = Block {
            header: BlockHeader {
                control_witness: ControlWitness::empty(),
                instance: lane_instance(&crypto, &network, CHAIN, &record),
                epoch: config.epoch.id,
                height: 1,
                origin_view: 0,
                parent_hash: Hash32(prior.block_hash),
                parent_result: Hash32(prior.result),
                payload_hash: payload_hash(&crypto, &payload),
                payload_len: u32::try_from(payload.len()).unwrap(),
                proposer: 0,
                skipped_leaders: Vec::new(),
                attest: false,
            },
            payload,
        };
        let mut executor = LaneExecutor::<_, _, NoTransactions>::recover(
            record.clone(),
            config,
            Arc::clone(&anchors),
            StatelessChecks::new(network),
            None,
            Hash32(prior.block_hash),
            &EmptyStore,
        )
        .unwrap();
        let Some(ExecOutcome::Valid(result)) = executor.execute(&block, &block.hash(&crypto))
        else {
            panic!("original lane execution must be valid");
        };
        let commit_qc = Qc {
            kind: VoteKind::Commit,
            instance: block.header.instance,
            epoch: block.header.epoch,
            height: 1,
            view: 0,
            block_hash: block.hash(&crypto),
            result,
            attest: false,
            signers: Bitmap::from_indices(4, [0, 1, 2]).unwrap(),
            agg_sig: AggregateSignature([0; SIGNATURE_LEN]),
            attestations: Vec::new(),
            attestation_witness: None,
        };
        let mut fixture = Self {
            record,
            network,
            anchors,
            prior,
            entry: SyncEntry { block, commit_qc },
            keys,
        };
        fixture.resign(3);
        fixture
    }
    fn resign(&mut self, count: usize) {
        self.entry.commit_qc.signers =
            Bitmap::from_indices(4, (0..count).map(|index| index as u32)).unwrap();
        let preimage = self.entry.commit_qc.preimage();
        let signatures = self.keys[..count]
            .iter()
            .map(|key| KeyPairSigner::new(key).unwrap().sign(&preimage))
            .collect::<Vec<_>>();
        self.entry.commit_qc.agg_sig = BlsCrypto::new().aggregate(&signatures);
    }
    fn verify(&self) -> Result<LaneResult, LaneEntryError> {
        verify_lane_entry(
            &self.record,
            &self.network,
            CHAIN,
            &*self.anchors,
            &LaneChainView::default(),
            &self.prior,
            &self.entry,
        )
    }
}

#[test]
fn original_execution_and_exact_real_quorum_reproduce_lane_result() {
    let fixture = Fixture::new();
    let result = fixture.verify().unwrap();
    assert_eq!(result.hash(), fixture.entry.commit_qc.result);
    assert_eq!(result.anchor_hash, fixture.anchors.hash);
    assert_eq!(result.tx_hashes.len(), 1);
}

#[test]
fn source_context_predecessor_pop_and_exact_quorum_are_all_required() {
    for mutation in 0..10 {
        let mut fixture = Fixture::new();
        match mutation {
            0 => fixture.record.incarnation[0] ^= 1,
            1 => fixture.record.committee[0].pop[0] ^= 1,
            2 => fixture.prior.result[0] ^= 1,
            3 => fixture.entry.commit_qc.agg_sig.0[0] ^= 1,
            4 => fixture.resign(2),
            5 => fixture.resign(4),
            6 => {
                fixture.entry.commit_qc.result.0[0] ^= 1;
                fixture.resign(3);
            }
            7 => fixture.record.committee.swap(0, 1),
            8 => {
                fixture.network = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(
                    Hash::new(b"other network"),
                ))
            }
            9 => fixture.prior.height = u64::MAX,
            _ => unreachable!(),
        }
        assert!(fixture.verify().is_err(), "mutation {mutation}");
    }
    let fixture = Fixture::new();
    assert!(
        verify_lane_certificate(
            &fixture.record,
            &fixture.network,
            "other-chain",
            &fixture.prior,
            &fixture.entry
        )
        .is_err()
    );
}

#[test]
fn unavailable_anchor_is_distinct_from_bad_admission_or_bad_certificate() {
    let fixture = Fixture::new();
    let missing = Anchors {
        hash: fixture.anchors.hash,
        available: false,
    };
    assert!(matches!(
        verify_lane_entry(
            &fixture.record,
            &fixture.network,
            CHAIN,
            &missing,
            &LaneChainView::default(),
            &fixture.prior,
            &fixture.entry
        ),
        Err(LaneEntryError::UnavailableAnchor)
    ));
    let result = fixture.verify().unwrap();
    let history = LaneChainView {
        previous_anchor: result.anchor_height,
        recent: result
            .tx_hashes
            .iter()
            .map(|hash| (*hash, result.anchor_height))
            .collect(),
    };
    assert!(matches!(
        verify_lane_entry(
            &fixture.record,
            &fixture.network,
            CHAIN,
            &*fixture.anchors,
            &history,
            &fixture.prior,
            &fixture.entry
        ),
        Err(LaneEntryError::Admission(AdmissionError::Duplicate(0)))
    ));
}

#[test]
fn valid_certificate_does_not_assert_its_payload_is_an_admissible_batch() {
    let mut fixture = Fixture::new();
    fixture.entry.block.payload = vec![0xff];
    let crypto = BlsCrypto::new();
    fixture.entry.block.header.payload_len = 1;
    fixture.entry.block.header.payload_hash = payload_hash(&crypto, &fixture.entry.block.payload);
    fixture.entry.commit_qc.block_hash = fixture.entry.block.hash(&crypto);
    fixture.resign(3);
    assert!(
        verify_lane_certificate(
            &fixture.record,
            &fixture.network,
            CHAIN,
            &fixture.prior,
            &fixture.entry
        )
        .is_ok()
    );
    assert!(matches!(
        fixture.verify(),
        Err(LaneEntryError::Admission(AdmissionError::Encoding(_)))
    ));
}
