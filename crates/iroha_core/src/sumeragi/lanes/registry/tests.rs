//! Actual model/BLS controls for retained registry ownership and fallible merge reads.

use super::*;
use crate::sumeragi::{crypto::KeyPairSigner, lanes::record::tests::fixture};
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
use iroha_sumeragi::{
    availability::{AvailabilitySource, AvailableBody, PayloadAuthoring, PayloadBytes},
    crypto::{NoAttestation, Signer},
    message::Qc,
    types::HeightConfig,
};
use mv::allocation::ChargedBuffer;
use std::{
    fs,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

const LANE: LaneId = LaneId::new(2);
const INCARNATION: [u8; 32] = [0x45; 32];
struct Schedule {
    instance: Hash32,
    config: HeightConfig,
}
impl AvailabilitySchedule for Schedule {
    fn instance(&self) -> Hash32 {
        self.instance
    }
    fn height_config(&self, _: u64) -> io::Result<Option<HeightConfig>> {
        Ok(Some(self.config.clone()))
    }
}
struct Authorities {
    schedule: Arc<Schedule>,
    missing: AtomicBool,
    corrupt: AtomicBool,
    calls: AtomicUsize,
}
impl LaneStoreAuthorities for Authorities {
    fn authority(
        &self,
        lane: LaneId,
        incarnation: &[u8; 32],
        instance: Hash32,
    ) -> io::Result<Option<LaneStoreAuthority>> {
        assert_eq!(lane, LANE);
        assert_eq!(*incarnation, INCARNATION);
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.missing.load(Ordering::SeqCst) {
            return Ok(None);
        }
        if self.corrupt.load(Ordering::SeqCst) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "bad authenticated activation",
            ));
        }
        // A separate test deliberately supplies a foreign schedule; registry must refuse it.
        assert_ne!(instance, Hash32::ZERO);
        Ok(Some(LaneStoreAuthority {
            schedule: self.schedule.clone(),
            verifier: Arc::new(NoAttestation),
        }))
    }
}
struct Fixture {
    dir: tempfile::TempDir,
    stores: LaneStores,
    body: AvailableBody,
    qc: Qc,
    source: AvailabilitySource,
    authorities: Arc<Authorities>,
}
impl Fixture {
    fn new(valid_batch: bool) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let (body, mut qc, old_source, budget, crypto) = fixture(1025, None);
        let crypto: SharedCrypto = Arc::new(crypto);
        let network = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
            b"registry network",
        )));
        let instance =
            incarnation_instance(&*crypto, &network, "registry-chain", LANE, &INCARNATION);
        let mut signers: Vec<_> = (1..=4)
            .map(|seed| {
                KeyPairSigner::new(&KeyPair::from_seed(vec![seed; 32], Algorithm::BlsNormal))
                    .unwrap()
            })
            .collect();
        signers.sort_by(|a, b| a.public_key().cmp(b.public_key()));
        let mut header = body.header().clone();
        header.instance = instance;
        let payload = if valid_batch {
            let bytes = LaneBatch {
                anchor_height: 7,
                anchor_hash: network.into_genesis_hash(),
                transactions: Vec::new(),
            }
            .to_payload();
            let mut charged = ChargedBuffer::new(bytes.len(), &budget).unwrap();
            for byte in bytes {
                charged.push_reserved(byte);
            }
            PayloadBytes::from_charged(charged, &budget)
                .unwrap_or_else(|_| panic!("original batch backing/control"))
        } else {
            body.payload().clone()
        };
        header.payload_hash = iroha_sumeragi::preimage::payload_hash(&*crypto, payload.as_slice());
        header.payload_len = u32::try_from(payload.as_slice().len()).unwrap();
        let authored = PayloadAuthoring::new(header, payload)
            .complete(
                instance,
                old_source.config(),
                &budget,
                &*crypto,
                &signers[0],
            )
            .unwrap_or_else(|(_, error)| panic!("original signed fixture: {error:?}"));
        drop(authored.codeword);
        let body = authored.body;
        qc.instance = instance;
        qc.block_hash = body.hash(&*crypto);
        qc.agg_sig = crypto.aggregate(
            &signers[..3]
                .iter()
                .map(|s| s.sign(&qc.preimage()))
                .collect::<Vec<_>>(),
        );
        let source =
            AvailabilitySource::new(instance, 1, qc.block_hash, old_source.config().clone())
                .unwrap();
        let authorities = Arc::new(Authorities {
            schedule: Arc::new(Schedule {
                instance,
                config: source.config().clone(),
            }),
            missing: AtomicBool::new(false),
            corrupt: AtomicBool::new(false),
            calls: AtomicUsize::new(0),
        });
        let stores = LaneStores::new(
            dir.path().to_path_buf(),
            network,
            "registry-chain".into(),
            crypto,
            budget,
            authorities.clone(),
        );
        Self {
            dir,
            stores,
            body,
            qc,
            source,
            authorities,
        }
    }
    fn path(&self) -> PathBuf {
        self.dir
            .path()
            .join(hex::encode(self.source.instance().0))
            .join("00000000000000000001.frame")
    }
    fn publish(&self) {
        self.stores
            .store(LANE, &INCARNATION)
            .unwrap()
            .append(&self.body, &self.qc)
            .unwrap();
    }
}

#[test]
fn complete_registry_reads_share_owner_and_preserve_valid_or_byzantine_payload_distinction() {
    for valid_batch in [false, true] {
        let f = Fixture::new(valid_batch);
        assert_eq!(f.stores.root(), f.dir.path());
        assert_eq!(f.stores.instance(LANE, &INCARNATION), f.source.instance());
        assert_eq!(f.stores.tip(LANE, &INCARNATION).unwrap(), Some(0));
        assert!(f.stores.block(LANE, &INCARNATION, 1).unwrap().is_none());
        f.publish();
        let a = f.stores.store(LANE, &INCARNATION).unwrap();
        let b = f.stores.store(LANE, &INCARNATION).unwrap();
        assert!(Arc::ptr_eq(&a, &b));
        assert_eq!(f.authorities.calls.load(Ordering::SeqCst), 1);
        let block = f.stores.block(LANE, &INCARNATION, 1).unwrap().unwrap();
        assert_eq!(block.block_hash, f.qc.block_hash);
        assert_eq!(block.result, f.qc.result);
        assert_eq!(block.batch.is_some(), valid_batch);
        if let Some(batch) = block.batch {
            assert_eq!(batch.anchor_height, 7);
            assert!(batch.transactions.is_empty());
        }
        assert!(
            f.stores
                .wait_for(LANE, &INCARNATION, 1, Duration::ZERO)
                .unwrap()
        );
        assert!(
            !f.stores
                .wait_for(LANE, &INCARNATION, 2, Duration::ZERO)
                .unwrap()
        );
        f.stores.release(LANE, &INCARNATION);
        assert_eq!(
            f.stores.store(LANE, &INCARNATION).unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        drop((a, b));
        assert_eq!(f.stores.tip(LANE, &INCARNATION).unwrap(), Some(1));
    }
}

#[test]
fn unresolved_corrupt_and_foreign_authority_never_become_an_empty_store() {
    let mut f = Fixture::new(true);
    f.authorities.missing.store(true, Ordering::SeqCst);
    for error in [
        f.stores.tip(LANE, &INCARNATION).unwrap_err(),
        f.stores.block(LANE, &INCARNATION, 1).unwrap_err(),
        f.stores
            .wait_for(LANE, &INCARNATION, 1, Duration::ZERO)
            .unwrap_err(),
    ] {
        assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
    }
    assert!(f.stores.stores.lock().is_empty());
    f.authorities.missing.store(false, Ordering::SeqCst);
    f.authorities.corrupt.store(true, Ordering::SeqCst);
    assert_eq!(
        f.stores.tip(LANE, &INCARNATION).unwrap_err().kind(),
        io::ErrorKind::InvalidData
    );
    let provider = Arc::new(Authorities {
        schedule: Arc::new(Schedule {
            instance: Hash32([0x88; 32]),
            config: f.source.config().clone(),
        }),
        missing: AtomicBool::new(false),
        corrupt: AtomicBool::new(false),
        calls: AtomicUsize::new(0),
    });
    f.stores.authorities = provider;
    assert_eq!(
        f.stores.tip(LANE, &INCARNATION).unwrap_err().kind(),
        io::ErrorKind::InvalidData
    );
    assert!(!f.path().exists());
}

#[test]
fn registry_retains_opening_lock_and_exact_charges_across_refusal() {
    let f = Fixture::new(true);
    f.publish();
    f.stores.release(LANE, &INCARNATION);
    let raw = fs::metadata(f.path()).unwrap().len() as usize;
    let baseline = f.stores.budget.reserved_bytes();
    f.stores.budget.set_limit_bytes(baseline + raw);
    assert_eq!(
        f.stores.store(LANE, &INCARNATION).unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
    assert_eq!(f.stores.budget.reserved_bytes(), baseline + raw);
    assert!(matches!(
        f.stores.stores.lock().get(&(LANE, INCARNATION)),
        Some(StoreSlot::Opening(_))
    ));
    let calls = f.authorities.calls.load(Ordering::SeqCst);
    f.authorities.corrupt.store(true, Ordering::SeqCst);
    assert_eq!(
        f.stores.tip(LANE, &INCARNATION).unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
    assert_eq!(f.stores.budget.reserved_bytes(), baseline + raw);
    assert_eq!(
        f.authorities.calls.load(Ordering::SeqCst),
        calls,
        "retry retains the original independently selected authority"
    );
    assert!(
        FileLaneBlockStore::begin_open(
            f.dir.path(),
            &f.source.instance(),
            f.stores.crypto.clone(),
            f.stores.budget.clone(),
            f.authorities.schedule.clone(),
            Arc::new(NoAttestation)
        )
        .is_err()
    );
    f.stores.budget.set_limit_bytes(1 << 25);
    assert_eq!(f.stores.tip(LANE, &INCARNATION).unwrap(), Some(1));
    assert_eq!(f.stores.budget.reserved_bytes(), baseline);
    assert!(matches!(
        f.stores.stores.lock().get(&(LANE, INCARNATION)),
        Some(StoreSlot::Ready(_))
    ));
}

#[test]
fn storage_corruption_is_repeated_error_not_missing_block_or_recovered_tip() {
    let f = Fixture::new(true);
    f.publish();
    let mut bytes = fs::read(f.path()).unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 1;
    fs::write(f.path(), bytes).unwrap();
    for _ in 0..2 {
        assert_eq!(
            f.stores.block(LANE, &INCARNATION, 1).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
    }
    f.stores.release(LANE, &INCARNATION);
    for _ in 0..2 {
        assert_eq!(
            f.stores.tip(LANE, &INCARNATION).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
    }
    assert!(matches!(
        f.stores.stores.lock().get(&(LANE, INCARNATION)),
        Some(StoreSlot::Opening(_))
    ));
}
