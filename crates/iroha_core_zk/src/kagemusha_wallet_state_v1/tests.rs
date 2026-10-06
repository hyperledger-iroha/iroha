//! State-control and persistence tests. Proof fixtures are explicit mocks; receipts use real
//! P-256 signatures. These tests do not establish Λ/Ω cryptographic validity or phone gates.

use super::*;
use crate::kagemusha_wallet_advance_v1::{
    KagemushaWalletCustodyDirV1 as Dir, KagemushaWalletEntryNameV1 as Name,
    KagemushaWalletFsV1 as Fs, KagemushaWalletMarkerRecordV1 as MarkerRecord,
    KagemushaWalletSimFaultV1 as Fault, KagemushaWalletSimFsV1 as SimFs,
    KagemushaWalletSimPowerLossV1 as PowerLoss, KagemushaWalletSlotIdV1 as Slot,
    KagemushaWalletTransitionOwnerV1 as OwnerTrait,
};
use p256::ecdsa::{Signature, SigningKey, signature::Signer as _};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

fn vectors() -> norito::json::Value {
    norito::json::from_str(include_str!(
        "../../../../fixtures/kagemusha/wallet_v1_vectors.json"
    ))
    .expect("vectors")
}

fn fixture<T>(name: &str) -> T
where
    T: norito::NoritoSerialize,
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    let all = vectors();
    let row = all["objects"]
        .as_array()
        .expect("objects")
        .iter()
        .find(|row| row["type"].as_str() == Some(name))
        .expect("fixture");
    archive::decode(&hex::decode(row["canonical_hex"].as_str().expect("hex")).expect("hex"))
        .expect("canonical fixture")
}

fn credential() -> KagemushaWalletCredentialV1 {
    fixture("KagemushaWalletCredentialV1")
}

fn signer(credential: &KagemushaWalletCredentialV1) -> SigningKey {
    for key in vectors()["keys"].as_array().expect("keys") {
        let signing = SigningKey::from_slice(
            &hex::decode(key["scalar_hex"].as_str().expect("scalar")).expect("hex"),
        )
        .expect("key");
        let public = KagemushaDevicePublicKeyV1::from_sec1_bytes(
            signing.verifying_key().to_encoded_point(false).as_bytes(),
        )
        .expect("public");
        if public == credential.body.payment_key {
            return signing;
        }
    }
    panic!("fixture payment key")
}

fn field(value: u8) -> [u8; 32] {
    let mut out = [0; 32];
    out[0] = value;
    out
}

// This envelope is constructed only by the test custody implementation. The production
// adapter obtains MarkerRecord exclusively from the actual Advance provider.
#[derive(norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::zk::kagemusha_wallet_advance_v1::MarkerFileV1")]
struct TestMarkerFile {
    version: u16,
    slot: [u8; 32],
    anchor_kind: u8,
    written_boot_id: [u8; 32],
    completion_digest: [u8; 32],
    selected_generation: u128,
    archive_checkpoint: [u8; 32],
    marker: Vec<u8>,
}

fn marker_record(marker: KagemushaWalletMarkerV1, completion_digest: [u8; 32]) -> MarkerRecord {
    let scheme = marker.scheme_id;
    let bytes = archive::encode(&TestMarkerFile {
        version: 1,
        slot: [0x55; 32],
        anchor_kind: 0,
        written_boot_id: [0x44; 32],
        completion_digest,
        selected_generation: if matches!(marker.state, KagemushaWalletMarkerStateV1::Head { .. }) {
            if completion_digest == [0; 32] {
                marker.generation
            } else {
                marker.generation - 1
            }
        } else {
            0
        },
        archive_checkpoint: [0; 32],
        marker: marker.to_canonical_bytes().expect("marker"),
    })
    .expect("envelope");
    MarkerRecord::decode(&bytes, &Slot([0x55; 32]), &scheme).expect("marker record")
}

fn enrollment() -> KagemushaWalletMarkerV1 {
    fixture("KagemushaWalletMarkerV1")
}

fn frozen(
    previous: Option<&FrozenTransition>,
    effect: KagemushaWalletEffectV1,
) -> FrozenTransition {
    let credential = credential();
    let mut c: KagemushaWalletRecoveryCapsuleV1 = fixture("KagemushaWalletRecoveryCapsuleV1");
    let kind = effect.kind();
    let request_input = c
        .retained_inputs
        .iter()
        .find(|input| input.role == KagemushaWalletRetainedInputRoleV1::Request)
        .expect("fixture Request")
        .clone();
    c.scheme_id = credential.body.scheme_id;
    c.wallet_id = credential.body.wallet_id;
    c.kind = kind;
    c.statement.scheme_id = c.scheme_id;
    c.statement.asset_digest = credential.body.asset_digest;
    c.statement.credential_digest = credential.credential_digest();
    c.statement.lifecycle = if kind == KagemushaWalletOperationKindV1::Retiring {
        KagemushaWalletLifecycleV1::Retiring
    } else {
        KagemushaWalletLifecycleV1::Active
    };
    c.statement.sequence = previous.map_or(0, |p| p.capsule.statement.sequence + 1);
    c.statement.next_load = 0;
    c.statement.enabled_controls = 0;
    c.statement.lineage_burned_total = 0;
    c.statement.lineage_pending_outgoing_root = [0; 32];
    c.statement.predecessor = previous
        .map_or(KagemushaWalletStateCommitmentV1 { value: [0; 32] }, |p| {
            p.capsule.statement.successor
        });
    c.statement.effect = effect;
    c.predecessor_capsule_digest =
        previous.map_or([0; 32], |p| p.capsule.capsule_digest().expect("digest"));
    c.predecessor_lineage = KagemushaWalletLineageSlotV1::None;
    if kind.consumes_lineage() {
        let payment: KagemushaWalletPaymentV1 = fixture("KagemushaWalletPaymentV1");
        let mut lineage = payment.send.lineage.lineage().expect("lineage").clone();
        lineage.public.head = c.statement.predecessor;
        lineage.public.wallet_id = c.wallet_id;
        lineage.public.credential_digest = c.statement.credential_digest;
        lineage.public.payment_key = credential.body.payment_key;
        lineage.public.enabled_controls = c.statement.enabled_controls;
        lineage.public.lifecycle = KagemushaWalletLifecycleV1::Active;
        c.statement.lineage_burned_total = lineage.public.burned_total;
        c.statement.lineage_pending_outgoing_root = lineage.public.pending_outgoing_root;
        c.predecessor_lineage = KagemushaWalletLineageSlotV1::Present { lineage };
    }
    c.payment_digest = if kind == KagemushaWalletOperationKindV1::Receive {
        field(72)
    } else {
        [0; 32]
    };
    let state = &mut c.successor_state;
    state.core.scheme_id = c.scheme_id;
    state.core.wallet_id = c.wallet_id;
    state.core.asset_digest = credential.body.asset_digest;
    state.core.credential_digest = credential.credential_digest();
    state.core.sequence = c.statement.sequence;
    state.core.lifecycle = c.statement.lifecycle;
    state.core.next_load = c.statement.next_load;
    state.core.balance = 20;
    state.core.enabled_controls = 0;
    state.core.burned_total = c.statement.lineage_burned_total;
    c.statement.successor = state.commitment().expect("state");
    c.operation_id = c.statement.operation_id(&c.wallet_id).expect("operation");
    c.output = KagemushaWalletOutputDescriptorV1::for_transition(
        &c.statement,
        &c.proof_digest().expect("proof digest"),
        &c.payment_digest,
    )
    .expect("output");
    c.map_openings.clear();
    c.retained_inputs = match kind {
        KagemushaWalletOperationKindV1::Receive => vec![
            KagemushaWalletRetainedInputRoleV1::Request,
            KagemushaWalletRetainedInputRoleV1::Payment,
            KagemushaWalletRetainedInputRoleV1::CertificateSet,
            KagemushaWalletRetainedInputRoleV1::Credential,
        ],
        _ => Vec::new(),
    }
    .into_iter()
    .map(|role| KagemushaWalletRetainedInputV1 {
        role,
        bytes: vec![1],
    })
    .collect();
    if kind == KagemushaWalletOperationKindV1::Send {
        c.retained_inputs = vec![request_input];
    }
    let result = FrozenTransition {
        credential,
        capsule: c,
    };
    result.validate().expect("frozen");
    result
}

fn bootstrap() -> FrozenTransition {
    frozen(
        None,
        KagemushaWalletEffectV1::Bootstrap {
            enrollment_id: credential().body.enrollment_id,
            enrollment_marker: enrollment().marker_digest().expect("marker digest"),
        },
    )
}

#[derive(Clone)]
pub(super) struct MemoryArchive {
    scheme: [u8; 32],
    wallet: [u8; 32],
    records: Arc<Mutex<BTreeMap<ArchiveKey, Vec<u8>>>>,
    reads: Arc<AtomicUsize>,
}
impl MemoryArchive {
    pub(super) fn new() -> Self {
        let c = credential();
        Self {
            scheme: c.body.scheme_id,
            wallet: c.body.wallet_id,
            records: Arc::default(),
            reads: Arc::default(),
        }
    }
}
impl MemoryArchive {
    fn keys(&mut self) -> Result<Vec<ArchiveKey>, Error> {
        Ok(self
            .records
            .lock()
            .expect("records")
            .keys()
            .copied()
            .collect())
    }
}
impl ArchiveStore for MemoryArchive {
    fn binding(&self) -> ([u8; 32], [u8; 32]) {
        (self.scheme, self.wallet)
    }
    fn get(&mut self, key: ArchiveKey, max_bytes: usize) -> Result<Option<Vec<u8>>, Error> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        if self
            .records
            .lock()
            .expect("records")
            .get(&key)
            .is_some_and(|bytes| bytes.len() > max_bytes)
        {
            return Err(Error::WitnessLost("record size"));
        }
        Ok(self.records.lock().expect("records").get(&key).cloned())
    }
    fn put(&mut self, key: ArchiveKey, bytes: &[u8]) -> Result<(), Error> {
        let mut records = self.records.lock().expect("records");
        if records.get(&key).is_some_and(|old| old != bytes) {
            return Err(Error::WitnessLost("immutable conflict"));
        }
        records.insert(key, bytes.to_vec());
        Ok(())
    }
}

struct TestCustody {
    checkpoint: Option<([u8; 32], Vec<u8>)>,
    status: SlotStatus,
    retained: BTreeMap<[u8; 32], Retained<KagemushaWalletCompletionRecordV1>>,
    pending: Option<AdvanceRequest<KagemushaWalletRecoveryCapsuleV1>>,
    pause: bool,
    signatures: usize,
    unavailable: bool,
    delivery_lost: bool,
}
impl TestCustody {
    fn new() -> Self {
        Self {
            checkpoint: None,
            status: SlotStatus::Enrollment(marker_record(enrollment(), [0; 32])),
            retained: BTreeMap::new(),
            pending: None,
            pause: false,
            signatures: 0,
            unavailable: false,
            delivery_lost: false,
        }
    }
}
impl Custody for TestCustody {
    fn archive_checkpoint(&mut self) -> Result<Option<([u8; 32], Vec<u8>)>, ProviderError> {
        Ok(self.checkpoint.clone())
    }
    fn publish_archive_checkpoint(
        &mut self,
        expected: [u8; 32],
        bytes: &[u8],
    ) -> Result<[u8; 32], ProviderError> {
        let digest =
            crate::kagemusha_wallet_advance_v1::kagemusha_wallet_archive_checkpoint_digest_v1(
                bytes,
            );
        if self.checkpoint.as_ref().map_or([0; 32], |c| c.0) != expected {
            return Err(ProviderError::Invalid {
                field: "stale test checkpoint",
            });
        }
        self.checkpoint = Some((digest, bytes.to_vec()));
        Ok(digest)
    }

    fn status(&mut self) -> Result<SlotStatus, ProviderError> {
        if self.unavailable {
            return Err(ProviderError::Unavailable(
                crate::kagemusha_wallet_advance_v1::KagemushaWalletUnavailableV1::Locked,
            ));
        }
        Ok(self.status.clone())
    }
    fn lookup(
        &mut self,
        operation_id: &[u8; 32],
    ) -> Result<Lookup<KagemushaWalletCompletionRecordV1>, ProviderError> {
        if self.unavailable {
            return Err(ProviderError::Unavailable(
                crate::kagemusha_wallet_advance_v1::KagemushaWalletUnavailableV1::Locked,
            ));
        }
        if self.delivery_lost {
            return Ok(Lookup::DeliveryDataLoss);
        }
        Ok(if let Some(retained) = self.retained.get(operation_id) {
            Lookup::Retained(Box::new(retained.clone()))
        } else if let Some(request) = self
            .pending
            .as_ref()
            .filter(|r| r.operation_id == *operation_id)
        {
            Lookup::SelectedUnsigned {
                capsule_digest: request.capsule.capsule_digest().expect("digest"),
            }
        } else {
            Lookup::Unknown
        })
    }
    fn advance(
        &mut self,
        owner: &TransitionOwner,
        request: &AdvanceRequest<KagemushaWalletRecoveryCapsuleV1>,
    ) -> Result<AdvanceOutcome<KagemushaWalletCompletionRecordV1>, ProviderError> {
        let capsule = &request.capsule;
        let digest = capsule.capsule_digest().expect("digest");
        if let Some(retained) = self.retained.get(&request.operation_id) {
            if retained.capsule_digest != digest {
                return Err(ProviderError::Invalid {
                    field: "operation conflict",
                });
            }
            return Ok(AdvanceOutcome::Released {
                retained: Box::new(retained.clone()),
                resumed: true,
            });
        }
        if self.pause {
            self.pending = Some(request.clone());
            let mut marker = enrollment();
            marker.generation = capsule.statement.sequence * 2 + 1;
            marker.state = capsule.head_marker_state().expect("head");
            self.status = SlotStatus::Pending(marker_record(marker, [0; 32]));
            return Ok(AdvanceOutcome::Pending {
                operation_id: request.operation_id,
            });
        }
        let body = owner.receipt_body(capsule, &digest)?;
        let signing = signer(&credential());
        let message =
            kagemusha_wallet_signing_message_v1(KagemushaWalletSigningDomainV1::Receipt, &body);
        let signature: Signature = signing.sign(&message);
        let signature = signature.normalize_s().unwrap_or(signature);
        let signature =
            KagemushaDeviceSignatureV1::from_raw_bytes(&signature.to_bytes()).expect("signature");
        self.signatures += 1;
        let record = owner.assemble(capsule, &digest, &signature)?;
        let retained = Retained {
            operation_id: capsule.operation_id,
            capsule_digest: digest,
            selected_generation: capsule.statement.sequence * 2 + 1,
            completion_digest: record.completion_digest().expect("digest"),
            frame: record.to_canonical_bytes().expect("frame"),
            record,
        };
        let mut marker = enrollment();
        marker.generation = capsule.statement.sequence * 2 + 2;
        marker.state = capsule.head_marker_state().expect("head");
        self.status = SlotStatus::Released(marker_record(marker, retained.completion_digest));
        self.retained.insert(capsule.operation_id, retained.clone());
        self.pending = None;
        Ok(AdvanceOutcome::Released {
            retained: Box::new(retained),
            resumed: false,
        })
    }
}

#[derive(Clone, Default)]
struct TestProofs {
    verifies: Arc<AtomicUsize>,
    folds: Arc<AtomicUsize>,
    reject: bool,
    burn: bool,
    checkpoint_bytes: u32,
}
impl NativeProofs for TestProofs {
    fn fold_schedule(
        &self,
        _witness: &ReleasedStep,
        _predecessor: Option<&KagemushaWalletFoldRecordV1>,
    ) -> Result<Vec<CheckpointLayout>, Error> {
        Ok(vec![CheckpointLayout {
            artifact_digest: field(99),
            payload_bytes: if self.checkpoint_bytes == 0 {
                3
            } else {
                self.checkpoint_bytes
            },
        }])
    }

    fn verify_transition(
        &self,
        next: &FrozenTransition,
        previous: Option<&FrozenTransition>,
        _folded: Option<&KagemushaWalletFoldRecordV1>,
    ) -> Result<(), Error> {
        if self.reject {
            return Err(Error::Proof("test rejection"));
        }
        if let Some(previous) = previous {
            assert_eq!(
                next.capsule.statement.predecessor,
                previous.capsule.statement.successor
            );
        }
        Ok(())
    }
    fn verify_lineage(&self, lineage: &KagemushaWalletLineageV1) -> Result<(), Error> {
        self.verifies.fetch_add(1, Ordering::SeqCst);
        if self.reject || lineage.proof != [1, 2, 3] {
            return Err(Error::Proof("mock proof identity"));
        }
        Ok(())
    }
    fn fold_next(
        &self,
        witness: &ReleasedStep,
        predecessor: Option<&KagemushaWalletFoldRecordV1>,
        checkpoint: Option<&[u8]>,
        cancellation: &Cancellation,
    ) -> Result<FoldProgress, Error> {
        cancellation.check()?;
        self.folds.fetch_add(1, Ordering::SeqCst);
        let expected = if self.checkpoint_bytes == 0 {
            vec![9, 8, 7]
        } else {
            vec![9; usize::try_from(self.checkpoint_bytes).expect("layout")]
        };
        if checkpoint.is_none() {
            return Ok(FoldProgress::Checkpoint(expected));
        }
        assert_eq!(checkpoint, Some(expected.as_slice()));
        let c = &witness.frozen.capsule;
        let mut tree = KagemushaWalletIndexedTreeV1::new();
        if let KagemushaWalletEffectV1::Receive { credit_id, .. } = c.statement.effect {
            // Tests use one Receive. Native production proofs must handle the complete map.
            KagemushaWalletCreditDigestLeafV1 {
                credit_id,
                payment_digest: c.payment_digest,
                burned: self.burn,
            }
            .record(&mut tree)
            .expect("credit");
        } else if let Some(pred) = predecessor {
            assert_eq!(pred.lineage.public.credit_digest_root, tree.root());
        }
        Ok(FoldProgress::Complete {
            lineage: KagemushaWalletLineageV1 {
                public: KagemushaWalletLineagePublicV1 {
                    version: 1,
                    scheme_id: c.scheme_id,
                    relation_id: c.statement.relation_id,
                    head: c.statement.successor,
                    wallet_id: c.wallet_id,
                    credential_digest: c.statement.credential_digest,
                    payment_key: witness.frozen.credential.body.payment_key,
                    lifecycle: c.statement.lifecycle,
                    policy_epoch: c.successor_state.core.policy_epoch,
                    enabled_controls: c.successor_state.core.enabled_controls,
                    burned_total: c.successor_state.core.burned_total,
                    pending_outgoing_root: c.successor_state.core.pending_outgoing_root,
                    credit_digest_root: tree.root(),
                },
                proof: vec![1, 2, 3],
            },
            burned: self.burn && c.kind == KagemushaWalletOperationKindV1::Receive,
        })
    }
}

type Wallet = Coordinator<TestCustody, MemoryArchive, TestProofs>;
fn wallet() -> Wallet {
    let c = credential();
    Coordinator::new(
        TestCustody::new(),
        MemoryArchive::new(),
        TestProofs::default(),
        c.body.scheme_id,
        c.body.wallet_id,
    )
    .expect("wallet")
}

#[test]
fn completion_retry_and_pending_resume_use_exact_receipt_bytes() {
    let mut w = wallet();
    let boot = bootstrap();
    w.custody.pause = true;
    assert_eq!(w.commit(boot.clone()).expect("commit"), Completion::Pending);
    assert_eq!(
        w.retry(&boot.capsule.operation_id).expect("retry"),
        Some(Completion::Pending)
    );
    assert_eq!(w.custody.signatures, 0);
    w.custody.pause = false;
    let complete = w.resume().expect("resume").expect("pending");
    assert!(matches!(complete, Completion::Complete(_)));
    assert_eq!(
        w.retry(&boot.capsule.operation_id).expect("retry"),
        Some(complete.clone())
    );
    assert_eq!(w.commit(boot.clone()).expect("same inputs"), complete);
    assert_eq!(w.custody.signatures, 1);
    assert_eq!(w.released_steps().expect("steps").len(), 1);
    let mut changed = boot;
    changed.capsule.step_proof.bytes.push(9);
    changed.capsule.output = KagemushaWalletOutputDescriptorV1::for_transition(
        &changed.capsule.statement,
        &changed.capsule.proof_digest().expect("proof"),
        &changed.capsule.payment_digest,
    )
    .expect("output");
    assert!(w.commit(changed).is_err());
    assert_eq!(w.custody.signatures, 1);
}

#[test]
fn rejected_native_proof_never_reaches_advance_or_archive() {
    let mut w = wallet();
    w.proofs.reject = true;
    assert!(matches!(w.commit(bootstrap()), Err(Error::Proof(_))));
    assert_eq!(w.custody.signatures, 0);
    assert!(w.archive.keys().expect("keys").is_empty());
}

#[test]
fn checkpoint_restart_credit_status_and_duplicate_receive_preserve_first_digest() {
    for burn in [false, true] {
        let mut w = wallet();
        w.proofs.burn = burn;
        let boot = bootstrap();
        w.commit(boot.clone()).expect("bootstrap");
        assert_eq!(w.fold_once().expect("inactive"), FoldStatus::Idle);
        w.scheduler().set_activity(true, false);
        assert_eq!(
            w.fold_once().expect("checkpoint"),
            FoldStatus::Checkpoint {
                sequence: 0,
                ordinal: 0
            }
        );
        let (custody, archive, proofs, scheme, wallet) =
            (w.custody, w.archive, w.proofs, w.scheme_id, w.wallet_id);
        let mut w = Coordinator::new(custody, archive, proofs, scheme, wallet).expect("restart");
        w.scheduler().set_activity(false, true);
        assert_eq!(w.fold_once().expect("fold"), FoldStatus::Folded(0));
        let verifies = w.proofs.verifies.load(Ordering::SeqCst);
        assert_eq!(w.fold_once().expect("done"), FoldStatus::CaughtUp);
        assert_eq!(
            w.proofs.verifies.load(Ordering::SeqCst),
            verifies,
            "recorded Ω is reused"
        );
        let receive = frozen(
            Some(&boot),
            KagemushaWalletEffectV1::Receive {
                credit_id: field(71),
                payer_wallet_id: [0x88; 32],
                amount: 10,
            },
        );
        let exact = w.commit(receive.clone()).expect("Receive");
        assert_eq!(
            w.consumed_credit(&field(71), &field(72))
                .expect("credit")
                .expect("present")
                .amount,
            10
        );
        assert!(matches!(
            w.credit_status(&field(71), &field(72)),
            Err(Error::FoldRequired)
        ));
        let signatures = w.custody.signatures;
        assert_eq!(w.commit(receive.clone()).expect("duplicate"), exact);
        assert_eq!(w.custody.signatures, signatures);
        assert!(matches!(
            w.consumed_credit(&field(71), &field(73)),
            Err(Error::CreditConflict)
        ));
        assert!(matches!(
            w.fold_once().expect("checkpoint"),
            FoldStatus::Checkpoint { sequence: 1, .. }
        ));
        assert_eq!(w.fold_once().expect("fold receive"), FoldStatus::Folded(1));
        let status = w
            .credit_status(&field(71), &field(72))
            .expect("CreditStatus");
        assert_eq!(status.opening.burned, burn);
        assert_eq!(status.opening.payment_digest, field(72));
        status.validate().expect("native receipt and opening");
        let record = w
            .archive
            .get(ArchiveKey::Fold(1), 20_000)
            .expect("record")
            .expect("present");
        assert!(
            w.archive
                .put(ArchiveKey::Fold(1), b"replacement proof")
                .is_err()
        );
        assert_eq!(
            w.archive.get(ArchiveKey::Fold(1), 20_000).expect("record"),
            Some(record)
        );
        w.archive
            .records
            .lock()
            .expect("records")
            .remove(&ArchiveKey::Fold(1));
        assert!(matches!(w.fold_once(), Err(Error::WitnessLost(_))));
    }
}

#[test]
fn missing_marker_bound_witness_is_loss_and_archive_cannot_invent_completion() {
    let mut w = wallet();
    let boot = bootstrap();
    w.archive
        .put(
            ArchiveKey::Capsule(boot.capsule.capsule_digest().expect("digest")),
            &archive::encode(&boot).expect("encode"),
        )
        .expect("staging");
    assert!(
        w.retry(&boot.capsule.operation_id)
            .expect("retry")
            .is_none()
    );
    w.commit(boot.clone()).expect("commit");
    w.archive.records.lock().expect("records").clear();
    assert!(matches!(w.released_steps(), Err(Error::WitnessLost(_))));
    assert!(matches!(
        w.retry(&boot.capsule.operation_id)
            .expect("exact authority"),
        Some(Completion::Complete(_))
    ));
}

#[test]
fn whole_lineage_bytes_are_required_for_session_reuse() {
    let mut w = wallet();
    w.commit(bootstrap()).expect("commit");
    w.scheduler().set_activity(true, false);
    w.fold_once().expect("checkpoint");
    w.fold_once().expect("fold");
    let step = w.released_steps().expect("steps").remove(0);
    let mut lineage = w
        .read_fold(&step)
        .expect("fold")
        .expect("present")
        .record
        .lineage;
    let mut cache = LineageCache::default();
    assert!(!cache.verify(&w.proofs, &lineage).expect("verify"));
    assert!(cache.verify(&w.proofs, &lineage).expect("reuse"));
    let before = w.proofs.verifies.load(Ordering::SeqCst);
    lineage.public.policy_epoch += 1;
    assert!(
        !cache
            .verify(&w.proofs, &lineage)
            .expect("public bytes differ")
    );
    assert_eq!(w.proofs.verifies.load(Ordering::SeqCst), before + 1);
    lineage.proof[0] ^= 1;
    assert!(cache.verify(&w.proofs, &lineage).is_err());
}

#[test]
fn payment_preempts_fold_and_frees_memory_before_admission() {
    let scheduler = Scheduler::new();
    scheduler.set_activity(true, false);
    let (started_tx, started_rx) = std::sync::mpsc::channel();
    let dropped = Arc::new(AtomicUsize::new(0));
    let worker = scheduler.clone();
    let freed = dropped.clone();
    let join = std::thread::spawn(move || {
        let guard = worker.start().expect("fold permit");
        struct Workspace(Arc<AtomicUsize>, Vec<u8>);
        impl Drop for Workspace {
            fn drop(&mut self) {
                self.0.store(self.1.len(), Ordering::SeqCst);
            }
        }
        let _workspace = Workspace(freed, vec![0; 1024]);
        started_tx.send(()).expect("started");
        while guard.token.check().is_ok() {
            std::thread::yield_now();
        }
    });
    started_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("worker");
    let payment = scheduler.payment();
    assert_eq!(dropped.load(Ordering::SeqCst), 1024);
    assert!(scheduler.start().is_none());
    drop(payment);
    join.join().expect("worker");
    assert!(scheduler.start().is_some());
    scheduler.set_activity(false, false);
    assert!(scheduler.start().is_none());
}

fn prepared_fs() -> (SimFs, Dir) {
    let fs = SimFs::new();
    let mut directory = Dir::root();
    for component in ["slots", &hex::encode([0x55; 32]), "archive"] {
        fs.mkdir(&directory, component).expect("mkdir");
        fs.sync_dir(&directory).expect("sync parent");
        directory = directory.child(&Name::new(component).expect("name"));
    }
    (fs, directory)
}

#[test]
fn fs_archive_is_canonical_bound_redundant_and_immutable() {
    let (fs, directory) = prepared_fs();
    let c = credential();
    let mut store = FsArchive::new(
        fs.clone(),
        Slot([0x55; 32]),
        c.body.scheme_id,
        c.body.wallet_id,
    )
    .expect("archive");
    let key = ArchiveKey::Checkpoint {
        sequence: 9,
        ordinal: 2,
    };
    assert!(store.get(key, 100).expect("absent").is_none());
    store.put(key, b"proof checkpoint").expect("put");
    fs.power_loss(PowerLoss::DropUnsynced);
    assert_eq!(
        store.get(key, 100).expect("get"),
        Some(b"proof checkpoint".to_vec())
    );
    assert_eq!(store.keys().expect("keys"), [key]);
    assert!(store.put(key, b"different proof").is_err());
    let primary = fs
        .visible_names(&directory)
        .into_iter()
        .find(|name| name.ends_with(".arc"))
        .expect("primary");
    fs.unlink(&directory, &primary).expect("remove first copy");
    fs.sync_dir(&directory).expect("sync");
    assert_eq!(
        store.get(key, 100).expect("replica"),
        Some(b"proof checkpoint".to_vec())
    );
    store.put(key, b"proof checkpoint").expect("repair");
    let mut other = FsArchive::new(fs.clone(), Slot([0x55; 32]), c.body.scheme_id, [0x99; 32])
        .expect("other archive");
    assert!(other.get(key, 100).is_err());
    fs.inject(fs.steps(), Fault::Error);
    assert!(
        store.keys().is_err(),
        "list error must not read as no checkpoints"
    );
}

#[test]
fn every_archive_write_crash_resumes_only_identical_bytes() {
    let (base, _) = prepared_fs();
    let c = credential();
    let probe = base.fork();
    let mut store = FsArchive::new(
        probe.clone(),
        Slot([0x55; 32]),
        c.body.scheme_id,
        c.body.wallet_id,
    )
    .expect("archive");
    let from = probe.steps();
    store
        .put(ArchiveKey::Fold(2), b"exact recorded proof")
        .expect("probe");
    let count = probe.steps() - from;
    for offset in 0..count {
        let fs = base.fork();
        let mut store = FsArchive::new(
            fs.clone(),
            Slot([0x55; 32]),
            c.body.scheme_id,
            c.body.wallet_id,
        )
        .expect("archive");
        fs.inject(fs.steps() + offset, Fault::CrashAfter);
        let _ = store.put(ArchiveKey::Fold(2), b"exact recorded proof");
        fs.power_loss(PowerLoss::DropUnsynced);
        store
            .put(ArchiveKey::Fold(2), b"exact recorded proof")
            .expect("resume identical");
        fs.power_loss(PowerLoss::DropUnsynced);
        assert_eq!(
            store.get(ArchiveKey::Fold(2), 20_000).expect("read"),
            Some(b"exact recorded proof".to_vec())
        );
    }
}

#[test]
fn checkpoint_binding_gap_and_interrupted_fold_publication_fail_closed() {
    let mut w = wallet();
    w.commit(bootstrap()).expect("commit");
    w.scheduler().set_activity(true, false);
    w.fold_once().expect("checkpoint");
    let key = ArchiveKey::Checkpoint {
        sequence: 0,
        ordinal: 0,
    };
    let original = w
        .archive
        .get(key, 20_000)
        .expect("get")
        .expect("checkpoint");
    let mut wrong: Checkpoint = archive::decode(&original).expect("decode");
    wrong.capsule_digest[0] ^= 1;
    w.archive
        .records
        .lock()
        .expect("records")
        .insert(key, archive::encode(&wrong).expect("encode"));
    assert!(matches!(w.fold_once(), Err(Error::WitnessLost(_))));
    {
        let mut records = w.archive.records.lock().expect("records");
        records.remove(&key);
        records.insert(
            ArchiveKey::Checkpoint {
                sequence: 0,
                ordinal: 1,
            },
            original.clone(),
        );
    }
    assert!(matches!(w.fold_once(), Err(Error::WitnessLost(_))));
    {
        let mut records = w.archive.records.lock().expect("records");
        records.remove(&ArchiveKey::Checkpoint {
            sequence: 0,
            ordinal: 1,
        });
        records.insert(key, original);
    }
    let prior_checkpoint = w.custody.checkpoint.clone();
    assert_eq!(w.fold_once().expect("fold"), FoldStatus::Folded(0));
    let folds = w.proofs.folds.load(Ordering::SeqCst);
    let before = w.archive.get(ArchiveKey::Fold(0), 20_000).expect("get");
    // Simulate publication interrupted after exact Ω bytes, before the source manifest.
    w.custody.checkpoint = prior_checkpoint;
    w.verified_folds.clear();
    assert_eq!(
        w.fold_once().expect("adopt publication"),
        FoldStatus::Folded(0)
    );
    assert_eq!(
        w.proofs.folds.load(Ordering::SeqCst),
        folds,
        "no replacement proof"
    );
    assert_eq!(
        w.archive.get(ArchiveKey::Fold(0), 20_000).expect("get"),
        before
    );
    assert_eq!(w.manifest().expect("source manifest").1.folded, Some(0));
}

#[test]
fn native_decide_failure_does_not_record_a_fold() {
    let mut w = wallet();
    w.commit(bootstrap()).expect("commit");
    w.scheduler().set_activity(true, false);
    w.fold_once().expect("checkpoint");
    w.proofs.reject = true;
    assert!(matches!(w.fold_once(), Err(Error::Proof(_))));
    assert!(
        w.archive
            .get(ArchiveKey::Fold(0), 20_000)
            .expect("record")
            .is_none()
    );
    assert_eq!(w.manifest().expect("source manifest").1.folded, None);
}

#[test]
fn archive_binding_rejects_mixing_wallets_before_work() {
    let c = credential();
    let mut archive = MemoryArchive::new();
    archive.wallet[0] ^= 1;
    assert!(matches!(
        Coordinator::new(
            TestCustody::new(),
            archive,
            TestProofs::default(),
            c.body.scheme_id,
            c.body.wallet_id
        ),
        Err(Error::Invalid(_))
    ));
}

#[test]
fn send_requires_recorded_predecessor_and_owner_assembles_canonical_payment() {
    let mut w = wallet();
    let boot = bootstrap();
    w.commit(boot.clone()).expect("boot");
    let template: KagemushaWalletPaymentV1 = fixture("KagemushaWalletPaymentV1");
    let send = frozen(Some(&boot), template.send.statement.effect);
    assert!(matches!(w.commit(send.clone()), Err(Error::FoldRequired)));
    assert_eq!(w.custody.signatures, 1);
    let owner = TransitionOwner::new(send.credential.clone());
    let c = &send.capsule;
    let digest = c.capsule_digest().expect("digest");
    let body = owner.receipt_body(c, &digest).expect("preflight");
    let signature: Signature = signer(&send.credential).sign(&kagemusha_wallet_signing_message_v1(
        KagemushaWalletSigningDomainV1::Receipt,
        &body,
    ));
    let signature = signature.normalize_s().unwrap_or(signature);
    let signature =
        KagemushaDeviceSignatureV1::from_raw_bytes(&signature.to_bytes()).expect("signature");
    let record = owner.assemble(c, &digest, &signature).expect("Payment");
    let payment = KagemushaWalletPaymentV1::decode_canonical(&record.output, &c.scheme_id)
        .expect("canonical Payment");
    assert_eq!(payment.send.statement, c.statement);
    assert_eq!(payment.send.step_proof, c.step_proof);
    assert_eq!(payment.send.receipt, record.receipt);
    record
        .verify(&send.credential, c)
        .expect("receipt and exact capsule binding");
    let mut duplicate = c.clone();
    duplicate
        .retained_inputs
        .push(duplicate.retained_inputs[0].clone());
    assert!(
        owner
            .receipt_body(&duplicate, &duplicate.capsule_digest().expect("digest"))
            .is_err()
    );
}

#[test]
fn exact_authenticated_checkpoint_layout_accepts_large_real_stage_shapes() {
    let mut w = wallet();
    w.proofs.checkpoint_bytes = 10_528;
    w.commit(bootstrap()).expect("commit");
    w.scheduler().set_activity(true, false);
    assert!(matches!(
        w.fold_once().expect("descriptor-sized checkpoint"),
        FoldStatus::Checkpoint { .. }
    ));
    let key = ArchiveKey::Checkpoint {
        sequence: 0,
        ordinal: 0,
    };
    let saved = w
        .archive
        .get(key, 20_000)
        .expect("read")
        .expect("checkpoint");
    let mut checkpoint: Checkpoint = archive::decode(&saved).expect("decode");
    assert_eq!(checkpoint.proof.len(), 10_528);
    checkpoint.layout.artifact_digest[0] ^= 1;
    w.archive
        .records
        .lock()
        .expect("records")
        .insert(key, archive::encode(&checkpoint).expect("encode"));
    assert!(matches!(w.fold_once(), Err(Error::WitnessLost(_))));
    w.archive
        .records
        .lock()
        .expect("records")
        .insert(key, saved);
    assert_eq!(w.fold_once().expect("fold"), FoldStatus::Folded(0));
}

#[test]
fn source_manifest_detects_fold_pair_deletion_and_index_loss_after_restart() {
    for delete_index in [false, true] {
        let mut w = wallet();
        w.commit(bootstrap()).expect("boot");
        w.scheduler().set_activity(true, false);
        w.fold_once().expect("checkpoint");
        w.fold_once().expect("fold");
        if delete_index {
            let (_, manifest) = w.manifest().expect("manifest");
            w.archive
                .records
                .lock()
                .expect("records")
                .remove(&ArchiveKey::Object(manifest.folds.0));
        } else {
            let mut records = w.archive.records.lock().expect("records");
            records.remove(&ArchiveKey::Fold(0));
        }
        let (custody, archive, proofs, scheme, wallet) =
            (w.custody, w.archive, w.proofs, w.scheme_id, w.wallet_id);
        assert!(matches!(
            Coordinator::new(custody, archive, proofs, scheme, wallet),
            Err(Error::WitnessLost(_))
        ));
    }
}

#[test]
fn replay_reads_are_bounded_and_retry_never_converts_unavailable_to_unknown() {
    let mut w = wallet();
    let mut previous = bootstrap();
    w.commit(previous.clone()).expect("boot");
    let mut first = None;
    for number in 1u8..=48 {
        let next = frozen(
            Some(&previous),
            KagemushaWalletEffectV1::Receive {
                credit_id: field(number + 100),
                payer_wallet_id: [0x88; 32],
                amount: 1,
            },
        );
        let complete = w.commit(next.clone()).expect("receive");
        if first.is_none() {
            first = Some((next.clone(), complete));
        }
        previous = next;
    }
    let (first, exact) = first.expect("first");
    let KagemushaWalletEffectV1::Receive { credit_id, .. } = first.capsule.statement.effect else {
        panic!("receive")
    };
    let before = w.archive.reads.load(Ordering::SeqCst);
    assert!(
        w.consumed_credit(&credit_id, &first.capsule.payment_digest)
            .expect("index")
            .is_some()
    );
    assert!(
        w.archive.reads.load(Ordering::SeqCst) - before <= 257,
        "one bounded Patricia path"
    );
    assert_eq!(
        w.retry(&first.capsule.operation_id).expect("exact"),
        Some(exact)
    );
    assert_eq!(w.retry(&[0xa5; 32]).expect("unknown"), None);
    let signatures = w.custody.signatures;
    w.custody.unavailable = true;
    assert!(matches!(
        w.retry(&first.capsule.operation_id),
        Err(Error::Provider(ProviderError::Unavailable(_)))
    ));
    assert!(matches!(
        w.retry(&[0xa5; 32]),
        Err(Error::Provider(ProviderError::Unavailable(_)))
    ));
    w.custody.unavailable = false;
    w.custody.delivery_lost = true;
    assert_eq!(
        w.retry(&first.capsule.operation_id).expect("loss"),
        Some(Completion::DeliveryDataLoss)
    );
    assert_eq!(w.custody.signatures, signatures);
}
