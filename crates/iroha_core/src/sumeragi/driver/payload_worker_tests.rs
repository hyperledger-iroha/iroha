//! Worker controls using actual signed authoring, coded rows and production file storage.
use crate::sumeragi::{
    bodies::{BodyLimits, FileBodyStore},
    body_read::{BodyReadError, BodyReadJob, BodyReadPoll, BodyReader},
    crypto::{BlsCrypto, KeyPairSigner},
    driver::{
        payload_worker::{PayloadWork, PayloadWorker},
        traits::{BlockStore, BodyStore},
    },
};
use iroha_crypto::{Algorithm, KeyPair, bls_normal_pop_prove};
use iroha_sumeragi::{
    api::Event,
    availability::{AvailabilitySource, AvailableBody},
    crypto::Signer,
    message::{Qc, SyncEntry, WireMessage},
    types::Hash32,
};
use mv::allocation::{AllocationBudget, ChargedBuffer};
use std::{
    io,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};
#[derive(Default)]
pub(super) struct ObservedProgress {
    pub(super) events: Vec<Event>,
    pub(super) sends: Vec<(Vec<iroha_sumeragi::types::PublicKey>, WireMessage)>,
    pub(super) retry: bool,
    pub(super) refused: bool,
}
#[derive(Default)]
struct Capture(std::sync::Mutex<Vec<(iroha_sumeragi::types::PublicKey, super::traits::Frame)>>);
impl super::traits::Net for Capture {
    fn send(
        &self,
        peer: &iroha_sumeragi::types::PublicKey,
        frame: &super::traits::Frame,
    ) -> super::traits::SendOutcome {
        self.0.lock().unwrap().push((peer.clone(), frame.clone()));
        super::traits::SendOutcome::Admitted
    }
}
pub(super) struct Blocks {
    source: AvailabilitySource,
    unavailable: AtomicBool,
    calls: AtomicUsize,
}
struct Absent(AvailabilitySource);
impl BodyReadJob for Absent {
    fn source(&self) -> &AvailabilitySource {
        &self.0
    }
    fn poll(&mut self, _: &AllocationBudget) -> Result<BodyReadPoll, BodyReadError> {
        Ok(BodyReadPoll::Absent)
    }
}
impl BodyReader for Blocks {
    fn begin_read(&self, s: AvailabilitySource) -> Result<Box<dyn BodyReadJob>, BodyReadError> {
        Ok(Box::new(Absent(s)))
    }
}
impl BlockStore for Blocks {
    fn height(&self) -> u64 {
        0
    }
    fn entry(&self, _: u64) -> io::Result<Option<SyncEntry>> {
        Ok(None)
    }
    fn availability_source(&self, h: u64, hash: Hash32) -> io::Result<Option<AvailabilitySource>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.unavailable.load(Ordering::SeqCst) {
            return Err(io::Error::from(io::ErrorKind::WouldBlock));
        }
        Ok(Some(
            AvailabilitySource::new(
                self.source.instance(),
                h,
                hash,
                self.source.config().clone(),
            )
            .unwrap(),
        ))
    }
    fn append(&self, _: &AvailableBody, _: &Qc) -> io::Result<()> {
        unreachable!()
    }
}
pub(super) struct Fixture {
    pub(super) body: AvailableBody,
    pub(super) source: AvailabilitySource,
    pub(super) budget: AllocationBudget,
    crypto: Arc<BlsCrypto>,
    pub(super) signers: Vec<Arc<dyn Signer>>,
    pub(super) blocks: Blocks,
    pub(super) bodies: FileBodyStore,
    _dir: tempfile::TempDir,
}
impl Fixture {
    pub(super) fn new() -> Self {
        let (body, source, budget) = crate::sumeragi::body_record::tests::fixture(2049);
        let crypto = Arc::new(BlsCrypto::new());
        let mut signers: Vec<Arc<dyn Signer>> = (1..=4)
            .map(|seed| -> Arc<dyn Signer> {
                let pair = KeyPair::from_seed(vec![seed; 32], Algorithm::BlsNormal);
                crypto
                    .admit(
                        pair.public_key(),
                        &bls_normal_pop_prove(pair.private_key()).unwrap(),
                    )
                    .unwrap();
                Arc::new(KeyPairSigner::new(&pair).unwrap())
            })
            .collect();
        signers.sort_by(|a, b| a.public_key().cmp(b.public_key()));
        let dir = tempfile::tempdir().unwrap();
        let bodies = FileBodyStore::open(
            dir.path(),
            &source.instance(),
            crypto.clone(),
            BodyLimits::default(),
            budget.clone(),
        )
        .unwrap();
        let blocks = Blocks {
            source: source.clone(),
            unavailable: AtomicBool::new(false),
            calls: AtomicUsize::new(0),
        };
        Self {
            body,
            source,
            budget,
            crypto,
            signers,
            blocks,
            bodies,
            _dir: dir,
        }
    }
    pub(super) fn worker(&self) -> PayloadWorker {
        PayloadWorker::new(
            self.source.instance(),
            self.budget.clone(),
            self.crypto.clone(),
            vec![self.signers[0].clone()],
            4,
            0,
        )
    }
    pub(super) fn poll(&self, w: &mut PayloadWorker, work: PayloadWork) -> ObservedProgress {
        let net = Capture::default();
        let result = super::serve::serve(
            super::serve::ServeRequest::Payload(Box::new(work)),
            self.source.instance(),
            &self.bodies,
            &self.blocks,
            &net,
            w,
        )
        .unwrap();
        let sends = net
            .0
            .into_inner()
            .unwrap()
            .into_iter()
            .map(|(peer, frame)| {
                let mut message = WireMessage::decode(&frame.bytes, usize::MAX).unwrap();
                message.admit_owned_bytes(&self.budget).unwrap();
                (vec![peer], message)
            })
            .collect();
        ObservedProgress {
            events: result.events,
            sends,
            retry: result.retry,
            refused: result.refused,
        }
    }
    pub(super) fn drain(&self, w: &mut PayloadWorker, mut p: ObservedProgress) -> ObservedProgress {
        let mut all = ObservedProgress::default();
        for _ in 0..4096 {
            all.events.append(&mut p.events);
            all.sends.append(&mut p.sends);
            if !p.retry {
                return all;
            }
            p = self.poll(w, PayloadWork::Poll);
        }
        panic!("worker did not quiesce");
    }
    pub(super) fn disseminate(&self) -> PayloadWork {
        PayloadWork::Disseminate {
            peers: vec![self.signers[1].public_key().clone()],
            body: self.body.clone(),
        }
    }
}
#[test]
fn original_rows_reconstruct_through_worker_and_waiting_does_not_spin() {
    let f = Fixture::new();
    let mut sender = f.worker();
    let p = f.poll(&mut sender, f.disseminate());
    let sent = f.drain(&mut sender, p);
    let mut receiver = f.worker();
    let mut messages = sent.sends.into_iter().map(|(_, m)| m);
    let WireMessage::PayloadManifest(manifest) = messages.next().unwrap() else {
        panic!()
    };
    let p = f.poll(
        &mut receiver,
        PayloadWork::Acquire {
            source: f.source.clone(),
            manifest: manifest.clone(),
        },
    );
    assert!(p.events.is_empty());
    assert!(!p.retry, "a manifest without rows waits for actual ingress");
    let mut available = Vec::new();
    for msg in messages {
        if let WireMessage::PayloadChunk(chunk) = msg {
            let p = f.poll(
                &mut receiver,
                PayloadWork::Chunk {
                    from: f.signers[0].public_key().clone(),
                    chunk,
                },
            );
            available.extend(p.events);
        }
    }
    assert!(matches!(available.as_slice(),[Event::BodyAvailable{block}] if block==&f.body));
}
#[test]
fn rebroadcast_reuses_retained_codeword_and_original_signatures() {
    let f = Fixture::new();
    let mut w = f.worker();
    let p = f.poll(&mut w, f.disseminate());
    let first = f.drain(&mut w, p);
    let calls = f.blocks.calls.load(Ordering::SeqCst);
    let held = f.budget.reserved_bytes();
    let p = f.poll(&mut w, f.disseminate());
    let second = f.drain(&mut w, p);
    assert_eq!(first.sends, second.sends);
    assert_eq!(
        calls,
        f.blocks.calls.load(Ordering::SeqCst),
        "no second authority lookup/encoding job"
    );
    drop(first);
    drop(second);
    assert!(
        f.budget.reserved_bytes() < held,
        "only outgoing row handles were released; cache remains"
    );
    let before = f.budget.reserved_bytes();
    f.poll(&mut w, PayloadWork::Applied(1));
    assert!(
        f.budget.reserved_bytes() < before,
        "completed-height codeword released"
    );
}
#[test]
fn source_refusal_retains_exact_historical_serve_request() {
    let f = Fixture::new();
    f.bodies.put(&f.source.block_hash(), &f.body).unwrap();
    let mut w = f.worker();
    f.blocks.unavailable.store(true, Ordering::SeqCst);
    let p = f.poll(
        &mut w,
        PayloadWork::Serve {
            to: f.signers[1].public_key().clone(),
            height: f.source.height(),
            block_hash: f.source.block_hash(),
        },
    );
    assert!(p.retry && p.sends.is_empty() && p.events.is_empty());
    f.blocks.unavailable.store(false, Ordering::SeqCst);
    let p = f.poll(&mut w, PayloadWork::Poll);
    let all = f.drain(&mut w, p);
    assert!(
        matches!(&all.sends[0].1,WireMessage::PayloadManifest(m) if m.availability==*f.body.availability())
    );
}
#[test]
fn resource_refusal_preserves_author_and_never_becomes_empty_work() {
    let f = Fixture::new();
    let mut w = f.worker();
    let mut header = f.body.header().clone();
    header.availability_digest = Hash32::ZERO;
    let hold = ChargedBuffer::<u8>::new((1 << 25) - f.budget.reserved_bytes(), &f.budget).unwrap();
    let p = f.poll(
        &mut w,
        PayloadWork::Author {
            req: 19,
            config: f.source.config().clone(),
            header,
            payload: f.body.payload().clone(),
        },
    );
    assert!(p.retry && p.refused && p.events.is_empty());
    drop(hold);
    let p = f.poll(&mut w, PayloadWork::Poll);
    assert!(matches!(p.events.as_slice(),[Event::PayloadAuthored{req:19,body}] if body==&f.body));
    let p = f.poll(&mut w, f.disseminate());
    let sent = f.drain(&mut w, p);
    assert!(!sent.sends.is_empty());
    assert_eq!(
        f.blocks.calls.load(Ordering::SeqCst),
        0,
        "author retained original codeword and authority"
    );
}
#[test]
fn stored_fetch_verifies_actual_file_then_returns_custody_without_remote_request() {
    let f = Fixture::new();
    f.bodies.put(&f.source.block_hash(), &f.body).unwrap();
    let mut w = f.worker();
    let p = f.poll(
        &mut w,
        PayloadWork::Fetch {
            source: f.source.clone(),
            peers: vec![f.signers[1].public_key().clone()],
        },
    );
    let all = f.drain(&mut w, p);
    assert!(all.sends.is_empty());
    assert!(matches!(all.events.as_slice(),[Event::BodyAvailable{block}] if block==&f.body));
}
#[test]
fn both_stores_absent_is_the_only_remote_fetch_outcome() {
    let f = Fixture::new();
    let mut w = f.worker();
    let p = f.poll(
        &mut w,
        PayloadWork::Fetch {
            source: f.source.clone(),
            peers: vec![f.signers[1].public_key().clone()],
        },
    );
    let all = f.drain(&mut w, p);
    assert!(all.events.is_empty());
    assert!(
        matches!(all.sends.as_slice(),[(_,WireMessage::PayloadRequest(r))] if r.block_hash==f.source.block_hash())
    );
}
struct Corrupt;
impl BodyReader for Corrupt {
    fn begin_read(&self, _: AvailabilitySource) -> Result<Box<dyn BodyReadJob>, BodyReadError> {
        Err(BodyReadError::Io(io::Error::new(
            io::ErrorKind::InvalidData,
            "corrupt disk",
        )))
    }
}
impl BodyStore for Corrupt {
    fn put(&self, _: &Hash32, _: &AvailableBody) -> io::Result<()> {
        unreachable!()
    }
    fn prune_through(&self, _: u64) -> io::Result<()> {
        unreachable!()
    }
}
#[test]
fn corruption_cannot_be_translated_into_absence_or_remote_fetch() {
    let f = Fixture::new();
    let mut w = f.worker();
    let result = w.step(
        PayloadWork::Fetch {
            source: f.source.clone(),
            peers: vec![f.signers[1].public_key().clone()],
        },
        &Corrupt,
        &f.blocks,
        &Capture::default(),
    );
    assert!(result.is_err());
}
#[test]
fn wrong_instance_never_enters_storage_or_acquisition() {
    let f = Fixture::new();
    let mut w = f.worker();
    let source = AvailabilitySource::new(
        Hash32([7; 32]),
        1,
        f.source.block_hash(),
        f.source.config().clone(),
    )
    .unwrap();
    assert!(
        w.step(
            PayloadWork::Fetch {
                source,
                peers: vec![]
            },
            &f.bodies,
            &f.blocks,
            &Capture::default(),
        )
        .is_err()
    );
}
