//! Each registered private attempt uses one fresh wallet operation and independent governed auditors.
use super::*;
use iroha::{
    client::{PrivateSettlementRunOptions, VerifiedPrivateSettlementOutcome},
    workflow::{
        Digest32, FileWorkflowStorage, WorkflowId, WorkflowJournalV1, WorkflowRunOptions,
        WorkflowRunnerLease, WorkflowStorage, WorkflowStorageDurability, WorkflowStorageError,
        WorkflowStorageErrorKind,
    },
};
use iroha_data_model::{
    nexus::{
        PrivateSettlementSidecarAvailabilityV1, private_settlement_sidecar_material_digest_v1,
    },
    parameter::system::ConsensusMode,
    sumeragi_finality::{
        SignedGenesisPinsV1, SumeragiFinalityVerifier, authenticate_signed_genesis_v1,
    },
};
use iroha_private_settlement::{
    FilePrivateSettlementArtifacts, FilePrivateSettlementWallet,
    MAX_PRIVATE_SETTLEMENT_WALLET_BYTES, OneCallPrivateSettlementOperation,
    OneCallPrivateSettlementRequest, PrivateSettlementBusinessTerms,
    PrivateSettlementRestrictedOpening,
};
use std::sync::{Arc, Mutex, mpsc};

#[derive(Default)]
struct Marks {
    proved: Option<Instant>,
    steps: [Option<Instant>; 5],
}

// The file owner commits every original record. Observations come only after its CAS succeeds.
struct ObservedJournal {
    files: FileWorkflowStorage,
    checkpoints: mpsc::Sender<usize>,
    marks: Mutex<Marks>,
    certificates: mpsc::Sender<Vec<PrivateSettlementSidecarAvailabilityV1>>,
}
impl WorkflowStorage for ObservedJournal {
    fn durability(&self) -> WorkflowStorageDurability {
        self.files.durability()
    }
    fn load(&self, id: &WorkflowId) -> Result<Option<Vec<u8>>, WorkflowStorageError> {
        self.files.load(id)
    }
    fn acquire_runner(
        &self,
        id: &WorkflowId,
    ) -> Result<Box<dyn WorkflowRunnerLease>, WorkflowStorageError> {
        self.files.acquire_runner(id)
    }
    fn compare_and_store(
        &self,
        id: &WorkflowId,
        expected: Option<&Digest32>,
        bytes: &[u8],
    ) -> Result<(), WorkflowStorageError> {
        self.files.compare_and_store(id, expected, bytes)?;
        self.observe(bytes).map_err(|error| {
            WorkflowStorageError::new(
                WorkflowStorageErrorKind::WriteUncertain,
                format!("benchmark observation after durable write: {error}"),
            )
        })
    }
}

impl ObservedJournal {
    fn observe(&self, bytes: &[u8]) -> Result<()> {
        let journal = WorkflowJournalV1::decode_record(bytes)?;
        let mut marks = self
            .marks
            .lock()
            .map_err(|_| eyre!("benchmark journal lock poisoned"))?;
        let now = Instant::now();
        if journal.identity.driver.id == "private_settlement_preparation" {
            if journal.steps.first().is_some_and(|step| step.is_done()) {
                marks.proved.get_or_insert(now);
            }
            return Ok(());
        }
        ensure!(
            journal.identity.driver.id == "private_settlement"
                && journal.steps.len() == marks.steps.len(),
            "unexpected benchmark journal plan"
        );
        for (index, step) in journal.steps.iter().enumerate() {
            if !step.is_done() || marks.steps[index].is_some() {
                continue;
            }
            marks.steps[index] = Some(now);
            match index {
                0 => {
                    let transcript = step
                        .round
                        .as_ref()
                        .and_then(|r| r.transcript.as_deref())
                        .ok_or_else(|| eyre!("missing original availability transcript"))?;
                    self.certificates
                        .send(norito::decode_canonical(transcript)?)
                        .map_err(|_| eyre!("benchmark auditors stopped before availability"))?;
                }
                1..=3 => {
                    // The independent observer owns all peer I/O. Its failure is
                    // reported separately and never stalls the wallet's next step.
                    let _ = self.checkpoints.send(index);
                }
                _ => {}
            }
        }
        Ok(())
    }
}

pub(super) struct Case {
    operation: OneCallPrivateSettlementOperation,
    runtime: tokio::runtime::Runtime,
    journal: Arc<ObservedJournal>,
    certificates: mpsc::Receiver<Vec<PrivateSettlementSidecarAvailabilityV1>>,
    checkpoints: mpsc::Receiver<usize>,
    starting_balance: Quantity,
    manifest: AtomicPrivateSettlementV1,
    _custody: tempfile::TempDir,
}

impl Case {
    pub(super) fn new(
        network: &Network,
        sponsor: &Client,
        governed: &[GovernedLeg],
        data: &[PrivateSettlementLegPrivateData],
        committees: &[CommitteeEndpoints],
        manifest: &AtomicPrivateSettlementV1,
    ) -> Result<Self> {
        TopologyShape::new(governed.len()).validate()?;
        ensure!(data.len() == governed.len() && committees.len() == governed.len());
        let custody = tempfile::tempdir_in(network.env_dir())?;
        let path = custody.path();
        let key = zeroize::Zeroizing::new(rand::random::<[u8; 32]>());
        let wallet = FilePrivateSettlementWallet::open(
            path.join("wallet"),
            &key,
            MAX_PRIVATE_SETTLEMENT_WALLET_BYTES,
        )?;
        let intent = manifest.proof_intent();
        let mut terms = Vec::with_capacity(governed.len());
        for (index, (leg, data)) in governed.iter().zip(data).enumerate() {
            let business = PrivateSettlementBusinessTerms {
                asset: leg.governance.body.asset_definition_id.clone(),
                payer: AccountId::new(data.payer.public_key().clone()),
                recipient: AccountId::new(data.recipient.public_key().clone()),
                amount: data.amount,
                reimbursement: if index == usize::from(intent.reimbursement_leg_ordinal) {
                    5
                } else {
                    0
                },
                memo: data.memo.clone(),
            };
            let funding =
                private_settlement_funding(intent.network_id, &leg.governance, index, data)?;
            wallet.stage_authorized(
                &intent,
                u8::try_from(index)?,
                &business,
                PrivateSettlementRestrictedOpening {
                    governance: leg.governance.clone(),
                    policy: leg.policy.clone(),
                },
                wallet_input::authorize_wallet_leg(&intent, index, &leg.governance, data, funding)?,
            )?;
            terms.push(business);
        }
        let genesis = network.genesis().0;
        let wire = genesis.encode_wire()?;
        let mut roster: Vec<_> = network.validators().iter().map(|p| p.id()).collect();
        roster.sort();
        let pins = SignedGenesisPinsV1 {
            chain_id: network.chain_id().to_string(),
            network_id: network.network_id(),
            genesis_hash: genesis.hash(),
            signed_genesis_sha256: Sha256::digest(&wire).into(),
            genesis_public_key: iroha_test_samples::SAMPLE_GENESIS_ACCOUNT_KEYPAIR
                .public_key()
                .clone(),
            roster,
            mode: ConsensusMode::Npos,
        };
        let authenticated = authenticate_signed_genesis_v1(&wire, &pins)?;
        let verifier = SumeragiFinalityVerifier::new(
            authenticated.block(),
            &pins.chain_id,
            authenticated.validators().to_vec(),
        )?;
        let request = OneCallPrivateSettlementRequest::new(
            intent.clone(),
            terms,
            iroha_core::state::State::native_world_schema_hash_v1()
                .map_err(|error| eyre!(error))?,
            genesis.header().creation_time().as_millis().try_into()?,
            committees.iter().map(|c| c.endpoints.clone()).collect(),
            PrivateSettlementCapsulePaddingV1::KiB16,
            intent.expiry_height + SIDECAR_RETENTION_BLOCKS,
            PRIVATE_SETTLEMENT_MAX_RECEIPT_BYTES_V1 as u64,
            32 * 1024 * 1024,
            128 * 1024 * 1024,
        )?;
        let (send, certificates) = mpsc::channel();
        let (step_sender, checkpoints) = mpsc::channel();
        let journal = Arc::new(ObservedJournal {
            files: FileWorkflowStorage::open_or_create(path.join("journal"))?,
            marks: Mutex::default(),
            checkpoints: step_sender,
            certificates: send,
        });
        let operation = OneCallPrivateSettlementOperation::new(
            sponsor.client().account_client()?,
            request,
            journal.clone(),
            Arc::new(FilePrivateSettlementArtifacts::open(
                path.join("artifacts"),
                &key,
            )?),
            Arc::new(wallet),
            verifier,
        )?;
        Ok(Self {
            operation,
            runtime: tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()?,
            journal,
            certificates,
            checkpoints,
            starting_balance: sponsor_nexus_fee_balance(sponsor)?,
            manifest: manifest.clone(),
            _custody: custody,
        })
    }

    pub(super) fn measure(
        self,
        network: &Network,
        sponsor: &Client,
        governed: &[GovernedLeg],
        committees: &[CommitteeEndpoints],
        started: Instant,
    ) -> Result<PrivateBenchmarkExecution> {
        let options = PrivateSettlementRunOptions {
            workflow: WorkflowRunOptions {
                poll_interval: POLL_INTERVAL,
                observation_timeout: FINALITY_TIMEOUT,
                ..Default::default()
            },
            ..Default::default()
        };
        let operation = &self.operation;
        let runtime = &self.runtime;
        let checkpoints = self.checkpoints;
        let bundle = self.manifest.bundle_id;
        let (receipt, audits, fee_before_finalization, finished) =
            thread::scope(|scope| -> Result<_> {
                let work = thread::Builder::new()
                    .name("private-benchmark-wallet".into())
                    .stack_size(TEST_STACK_BYTES)
                    .spawn_scoped(scope, move || {
                        let result = runtime.block_on(operation.run(&options))?;
                        let VerifiedPrivateSettlementOutcome::Settled(receipt) = result.outcome
                        else {
                            return Err(eyre!("one-call benchmark returned an aborted payment"));
                        };
                        Ok((receipt, Instant::now()))
                    })?;
                let (audits, checkpoint_result) = thread::scope(|observers| {
                    let handle = &work;
                    let reader = observers.spawn(move || {
                        observe_checkpoints(checkpoints, network, sponsor, bundle, || {
                            handle.is_finished()
                        })
                    });
                    // Independent auditors observe original availability certificates. A premature
                    // SDK refusal is a failed attempt. Never replay the call to construct a latency.
                    let got = loop {
                        match self.certificates.recv_timeout(Duration::from_millis(20)) {
                            Ok(certs) => break Some(certs),
                            Err(_) if work.is_finished() => break None,
                            Err(_) => {}
                        }
                    };
                    let audits = got.map(|certs| -> Result<_> {
                        ensure!(
                            certs.len() == committees.len(),
                            "incomplete certificate matrix"
                        );
                        benchmark_fanout(certs.len(), |i| {
                            observe_leg(
                                network,
                                sponsor,
                                &self.manifest,
                                i,
                                &governed[i],
                                &committees[i],
                                &certs[i],
                                started + FINALITY_TIMEOUT,
                                || work.is_finished(),
                            )
                        })
                    });
                    (audits, reader.join())
                });
                let (receipt, finished) = work
                    .join()
                    .map_err(|_| eyre!("one-call SDK worker panicked"))??;
                Ok((
                    receipt,
                    audits.ok_or_else(|| eyre!("one-call attempt omitted availability"))??,
                    checkpoint_result
                        .map_err(|_| eyre!("benchmark checkpoint observer panicked"))??,
                    finished,
                ))
            })?;
        ensure!(receipt.receipt().manifest.proof_intent() == self.manifest.proof_intent());
        let TransactionEntrypoint::External(transaction) = receipt.evidence().entrypoint() else {
            return Err(eyre!("verified original Finalize was not external"));
        };
        ensure!(
            transaction.hash() == receipt.transaction_hash()
                && receipt.nexus_fee_receipt().is_some()
        );
        let submit = PrivateSettlementBundleSubmitRequestV1 {
            transaction: transaction.clone(),
        };
        let marks = self
            .journal
            .marks
            .lock()
            .map_err(|_| eyre!("benchmark observations unavailable"))?;
        let Some(proved) = marks.proved else {
            return Err(eyre!("original proof journal did not complete"));
        };
        let [
            Some(available),
            Some(prepared),
            Some(registered),
            Some(committed),
            Some(_),
        ] = marks.steps
        else {
            return Err(eyre!(
                "one-call benchmark omitted a durable settlement stage"
            ));
        };
        ensure_exact_private_settlement_carrier_fee(
            &self.starting_balance,
            &fee_before_finalization,
            "one-call Prepare registration",
        )?;
        let promoted = audits
            .iter()
            .map(|leg| leg.promoted)
            .max()
            .ok_or_else(|| eyre!("missing promotion observations"))?;
        let audit_start = audits.iter().map(|leg| leg.promoted).min().unwrap();
        let audited = audits.iter().map(|leg| leg.approved).max().unwrap();
        let proof_read_start = audits.iter().map(|leg| leg.approved).min().unwrap();
        let proof_read_end = audits.iter().map(|leg| leg.verified).max().unwrap();
        ensure!(
            available >= proved && promoted >= available && proof_read_end <= finished,
            "committee observations fell outside the measured proof-through-receipt window"
        );
        Ok(PrivateBenchmarkExecution {
            receipt: receipt.receipt().clone(),
            submit,
            fee_before_finalization,
            proof_bytes: audits.iter().map(|a| a.bytes).sum(),
            proof_generation: stage_ms(started, proved)?,
            restricted_upload_availability: stage_ms(proved, promoted)?,
            auditor_response: stage_ms(audit_start, audited)?,
            committee_verification: stage_ms(proof_read_start, proof_read_end)?,
            prepare: stage_ms(available, prepared)?,
            prepare_registration: stage_ms(prepared, registered)?,
            commit: stage_ms(registered, committed)?,
            global_finality: stage_ms(committed, finished)?,
            end_to_end: stage_ms(started, finished)?,
        })
    }
}

fn wait_for_checkpoint(
    checkpoints: &mpsc::Receiver<usize>,
    expected: usize,
    done: &impl Fn() -> bool,
) -> Result<()> {
    loop {
        match checkpoints.recv_timeout(Duration::from_millis(20)) {
            Ok(index) => {
                ensure!(index == expected, "one-call checkpoint order changed");
                return Ok(());
            }
            Err(mpsc::RecvTimeoutError::Timeout) if !done() => {}
            Err(_) => return Err(eyre!("one-call checkpoint {expected} absent")),
        }
    }
}

// Each read starts after its actual durable phase, independently of the wallet.
// Missing a pending or fee observation fails the attempt even if the payment settled.
fn observe_checkpoints(
    checkpoints: mpsc::Receiver<usize>,
    network: &Network,
    sponsor: &Client,
    bundle: Hash,
    done: impl Fn() -> bool,
) -> Result<Quantity> {
    thread::scope(|scope| -> Result<_> {
        wait_for_checkpoint(&checkpoints, 1, &done)?;
        let prepared = scope.spawn(|| assert_no_partial_visibility(network, bundle, "prepared"));
        wait_for_checkpoint(&checkpoints, 2, &done)?;
        let balance = scope.spawn(|| sponsor_nexus_fee_balance(sponsor));
        wait_for_checkpoint(&checkpoints, 3, &done)?;
        assert_no_partial_visibility(network, bundle, "commit-certified")?;
        prepared
            .join()
            .map_err(|_| eyre!("benchmark Prepare observer panicked"))??;
        balance
            .join()
            .map_err(|_| eyre!("benchmark Register observer panicked"))?
    })
}

struct LegAudit {
    promoted: Instant,
    approved: Instant,
    verified: Instant,
    bytes: u64,
}

#[allow(clippy::too_many_arguments)]
fn observe_leg(
    network: &Network,
    sponsor: &Client,
    original: &AtomicPrivateSettlementV1,
    index: usize,
    leg: &GovernedLeg,
    committee: &CommitteeEndpoints,
    cert: &PrivateSettlementSidecarAvailabilityV1,
    deadline: Instant,
    done: impl Fn() -> bool,
) -> Result<LegAudit> {
    ensure!(
        usize::from(cert.body.leg_ordinal) == index
            && cert.body.bundle_id == original.bundle_id
            && cert.body.authority_digest == committee.authority.digest()?
    );
    let digest = cert.body.payload_digest;
    let signer = BorrowedKeyPairIdentityRequestSignerV1::new(&leg.auditor_signing);
    let request = PrivateSettlementAuditorCapsuleRequestV1 {
        audit_policy: leg.policy.clone(),
    };
    let client = sponsor.client().with_request_deadline(deadline);
    let fetched = loop {
        // The SDK promotes the certified leg in its next phase. Read-only observations
        // may precede promotion. Only an authenticated quorum response can advance.
        match client.private_settlement_auditor_capsule_quorum_for_authority_v1(
            &committee.endpoints,
            &committee.authority,
            digest,
            &request,
            &signer,
        ) {
            Ok(view) => break view,
            Err(error) if done() || Instant::now() >= deadline => return Err(error),
            Err(_) => thread::sleep(Duration::from_millis(10)),
        }
    };
    let promoted = Instant::now();
    ensure!(
        fetched.lifecycle == PrivateSettlementLifecycleDtoV1::Collecting
            && fetched.manifest.proof_intent() == original.proof_intent()
            && fetched.availability == *cert
            && fetched.audit_policy == leg.policy
            && fetched.committee_authority == committee.authority
    );
    let height = fetched.authoritative_height;
    let view = PrivateSettlementAuditorSidecarViewV1 {
        manifest: fetched.manifest,
        policy: fetched.audit_policy,
        authority: fetched.committee_authority,
        statement: fetched.statement,
        delta: fetched.delta,
        audit_capsule: fetched.audit_capsule,
        availability: fetched.availability,
        lifecycle: PrivateSettlementSidecarLifecycleV1::Collecting,
    };
    let approval = approve_private_settlement_leg_v1(
        &view,
        &leg.governance,
        height,
        &AccountId::new(leg.auditor_signing.public_key().clone()),
        leg.auditor_encryption.secret(),
        &leg.auditor_signing,
        &approve_all_audit_material,
    )?;
    let approved = client.submit_private_settlement_audit_approval_quorum_for_authority_v1(
        &committee.endpoints,
        &committee.authority,
        digest,
        &signer,
        &PrivateSettlementAuditApprovalRequestV1 {
            audit_policy: leg.policy.clone(),
            approval: approval.clone(),
        },
    )?;
    ensure!(approved.lifecycle == PrivateSettlementLifecycleDtoV1::Audited);
    let approved = Instant::now();
    ensure!(
        committee.endpoints.len() == VALIDATORS_PER_LANE
            && committee.validator_keys.len() == VALIDATORS_PER_LANE
    );
    let proof_lengths = benchmark_fanout(committee.endpoints.len(), |index| {
        let endpoint = &committee.endpoints[index];
        let key = &committee.validator_keys[index];
        let peer = network
            .all_peers()
            .find(|p| Url::parse(&p.torii_url()).is_ok_and(|url| url == *endpoint))
            .ok_or_else(|| eyre!("proof endpoint absent from network"))?;
        let proof = peer
            .client_for(
                sponsor.account_client().authority(),
                sponsor.client().key_pair().private_key().clone(),
            )
            .client()
            .with_request_deadline(deadline)
            .private_settlement_committee_proof_v1(digest, key)?;
        ensure!(
            proof.manifest == view.manifest
                && proof.statement == view.statement
                && proof.delta == view.delta
                && proof.committee_authority == view.authority
                && proof.audit_policy == view.policy
                && proof.availability == *cert
                && proof.audit_capsule_digest == view.audit_capsule.digest()?
                && proof.audit_approvals.contains(&approval)
                && matches!(
                    proof.lifecycle,
                    PrivateSettlementLifecycleDtoV1::Audited
                        | PrivateSettlementLifecycleDtoV1::Prepared
                        | PrivateSettlementLifecycleDtoV1::CommitCertified
                        | PrivateSettlementLifecycleDtoV1::Finalized
                )
                && private_settlement_sidecar_material_digest_v1(
                    &proof.statement,
                    &proof.proof,
                    &proof.delta,
                    &view.audit_capsule
                )? == digest,
            "committee proof view differs from the original signed, audited material"
        );
        Ok(u64::try_from(proof.proof.len())?)
    })?;
    let bytes = *proof_lengths
        .first()
        .ok_or_else(|| eyre!("empty committee proof matrix"))?;
    ensure!(
        proof_lengths.iter().all(|length| *length == bytes),
        "committee validators disagree on original proof length"
    );
    Ok(LegAudit {
        promoted,
        approved,
        verified: Instant::now(),
        bytes,
    })
}

fn stage_ms(start: Instant, end: Instant) -> Result<f64> {
    Ok(end
        .checked_duration_since(start)
        .filter(|duration| !duration.is_zero())
        .ok_or_else(|| eyre!("benchmark stage timestamps missing or out of order"))?
        .as_secs_f64()
        * 1000.0)
}

#[test]
fn one_call_stage_requires_distinct_ordered_observations() {
    let start = Instant::now();
    let end = start + Duration::from_millis(5);
    assert_eq!(stage_ms(start, end).unwrap(), 5.0);
    assert!(stage_ms(end, start).is_err());
    assert!(stage_ms(start, start).is_err());
}

#[test]
fn one_call_checkpoints_require_the_original_order_and_completion() {
    let (send, receive) = mpsc::channel();
    send.send(1).unwrap();
    wait_for_checkpoint(&receive, 1, &|| false).unwrap();
    send.send(3).unwrap();
    assert!(wait_for_checkpoint(&receive, 2, &|| false).is_err());
    assert!(wait_for_checkpoint(&receive, 2, &|| true).is_err());
    drop(send);
    assert!(wait_for_checkpoint(&receive, 3, &|| false).is_err());
}
