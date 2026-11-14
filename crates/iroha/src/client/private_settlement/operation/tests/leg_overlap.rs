//! Whole-phase leg overlap over scripted committees with real signatures and QCs.
//! Proof bytes are synthetic; these controls qualify orchestration, not native proving.

use super::*;
use crate::{client::evidence_http_tests::with_mock_http, http_default::RequestSnapshot};
use iroha_torii_shared::private_settlement_api::PrivateSettlementLegUploadDispositionV1;
use std::sync::Condvar;

/// Requires `size` requests of one stage to be in flight together.
struct Together {
    size: usize,
    entered: Mutex<BTreeMap<&'static str, usize>>,
    changed: Condvar,
}
impl Together {
    fn new(size: usize) -> Self {
        Self {
            size,
            entered: Mutex::new(BTreeMap::new()),
            changed: Condvar::new(),
        }
    }
    // A bounded gate makes leg-by-leg dispatch fail instead of deadlocking the suite.
    fn enter(&self, stage: &'static str) -> Result<()> {
        let mut entered = self.entered.lock().unwrap();
        let count = entered.entry(stage).or_default();
        *count += 1;
        let target = count.div_ceil(self.size) * self.size;
        self.changed.notify_all();
        let (entered, _) = self
            .changed
            .wait_timeout_while(entered, Duration::from_secs(10), |entered| {
                entered[stage] < target
            })
            .unwrap();
        if entered[stage] < target {
            return Err(eyre!(
                "every leg's {stage} requests must be in flight together"
            ));
        }
        Ok(())
    }
}

/// Scripted committees of the two-leg fixture: real votes, QCs and durable certificates.
struct Committees {
    request: PrivateSettlementRequest,
    bundle_id: Hash,
    barrier: Option<PrivateSettlementPrepareBarrierV1>,
    together: Option<Together>,
    // Stage of every request, in arrival order.
    events: Mutex<Vec<&'static str>>,
    // Certificates each member of each leg's committee holds durably.
    prepare: Mutex<BTreeMap<(usize, usize), PrivateSettlementPhaseCertificateV1>>,
    commit: Mutex<BTreeMap<(usize, usize), PrivateSettlementPhaseCertificateV1>>,
    // A leg whose committee is unavailable for votes.
    refuse_votes: Mutex<Option<usize>>,
}

fn json(value: &impl norito::json::JsonSerialize) -> Result<Response<Vec<u8>>> {
    Ok(Response::builder()
        .status(200)
        .header("Content-Type", APPLICATION_JSON)
        .body(norito::json::to_vec(value)?)?)
}

impl Committees {
    fn new(
        request: &PrivateSettlementRequest,
        certificates: &[PrivateSettlementSidecarAvailabilityV1],
        barrier: Option<PrivateSettlementPrepareBarrierV1>,
        together: Option<usize>,
    ) -> Arc<Self> {
        Arc::new(Self {
            request: request.clone(),
            bundle_id: request.manifest_with(certificates).unwrap().bundle_id,
            barrier,
            together: together.map(Together::new),
            events: Mutex::new(Vec::new()),
            prepare: Mutex::new(BTreeMap::new()),
            commit: Mutex::new(BTreeMap::new()),
            refuse_votes: Mutex::new(None),
        })
    }

    fn events(&self) -> Vec<&'static str> {
        self.events.lock().unwrap().clone()
    }

    fn leg_of(&self, payload_digest: Hash) -> usize {
        self.request
            .0
            .materials
            .iter()
            .position(|material| material.payload_digest().unwrap() == payload_digest)
            .expect("request names one of the original legs")
    }

    fn vote(
        &self,
        manifest: &AtomicPrivateSettlementV1,
        payload_digest: Hash,
        member: usize,
        phase: PrivateSettlementPhaseV1,
        prepared_bundle_digest: Hash,
    ) -> Result<Response<Vec<u8>>> {
        let leg = self.leg_of(payload_digest);
        if *self.refuse_votes.lock().unwrap() == Some(leg) {
            return Err(eyre!("fixture committee is unavailable"));
        }
        let material = &self.request.0.materials[leg];
        let ordinal = material.statement.leg_ordinal;
        let (authority, keys) =
            fixtures::finalization_authority_v1(material.statement.route, ordinal);
        let body =
            expected_phase_body_v1(manifest, ordinal, &authority, phase, prepared_bundle_digest)?;
        let vote = super::super::super::tests::phase_votes_v1(&authority, &keys, &body, &[member])
            .remove(0);
        json(&PrivateSettlementPhaseVoteResponseV1 {
            bundle_id: manifest.bundle_id,
            payload_digest,
            leg_ordinal: ordinal,
            vote,
        })
    }

    fn respond(&self, http: &RequestSnapshot) -> Result<Response<Vec<u8>>> {
        let path = http.url.path();
        let member = self.request.0.endpoints[0]
            .iter()
            .position(|endpoint| endpoint.host_str() == http.url.host_str())
            .expect("fixture endpoint");
        let stage = if path.ends_with("/availability-shares") {
            "availability"
        } else if path.ends_with("/legs") {
            "promotion"
        } else if path.ends_with("/phase-certificates") {
            "recover"
        } else if path.ends_with("/prepare-votes") || path.ends_with("/commit-votes") {
            "vote"
        } else if path.ends_with("/phases/certificates") {
            "persist"
        } else {
            panic!("unexpected committee request {path}")
        };
        self.events.lock().unwrap().push(stage);
        if let Some(together) = &self.together {
            together.enter(stage)?;
        }
        match stage {
            "availability" => {
                let upload: PrivateSettlementAvailabilityShareRequestV1 =
                    norito::json::from_slice(&http.body)?;
                let material = upload.material;
                let ordinal = material.statement.leg_ordinal;
                assert_eq!(material, self.request.0.materials[usize::from(ordinal)]);
                let (_, keys) =
                    fixtures::finalization_authority_v1(material.statement.route, ordinal);
                let share = PrivateSettlementAvailabilityShareV1 {
                    version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
                    body: material.availability_body,
                    signer: material.committee_authority.validators[member].clone(),
                    signature: Signature::try_new(
                        keys[member].private_key(),
                        &material.availability_body.signature_preimage()?,
                    )?
                    .payload()
                    .to_vec(),
                };
                json(&PrivateSettlementAvailabilityShareResponseV1 {
                    bundle_id: share.body.bundle_id,
                    payload_digest: share.body.payload_digest,
                    leg_ordinal: share.body.leg_ordinal,
                    disposition: PrivateSettlementLegUploadDispositionV1::Stored,
                    share,
                })
            }
            "promotion" => {
                let upload: PrivateSettlementLegUploadRequestV1 =
                    norito::json::from_slice(&http.body)?;
                json(&PrivateSettlementLegUploadResponseV1 {
                    bundle_id: upload.manifest.bundle_id,
                    payload_digest: upload.payload.availability.body.payload_digest,
                    leg_ordinal: upload.payload.statement.leg_ordinal,
                    disposition: PrivateSettlementLegUploadDispositionV1::AlreadyStored,
                    lifecycle: PrivateSettlementLifecycleDtoV1::Collecting,
                })
            }
            "recover" => {
                let leg = (0..self.request.0.materials.len())
                    .find(|leg| {
                        let digest = self.request.0.materials[*leg].payload_digest().unwrap();
                        path.contains(&digest.to_string())
                    })
                    .expect("recovery names one of the original legs");
                let material = &self.request.0.materials[leg];
                let prepare_certificate = self.barrier.as_ref().map_or_else(
                    || self.prepare.lock().unwrap().get(&(leg, member)).cloned(),
                    |barrier| Some(barrier.prepare_certificates[leg].clone()),
                );
                let commit_certificate = self.commit.lock().unwrap().get(&(leg, member)).cloned();
                json(&PrivateSettlementPhaseCertificatesResponseV1 {
                    bundle_id: self.bundle_id,
                    payload_digest: material.payload_digest().unwrap(),
                    leg_ordinal: material.statement.leg_ordinal,
                    lifecycle: if commit_certificate.is_some() {
                        PrivateSettlementLifecycleDtoV1::CommitCertified
                    } else if prepare_certificate.is_some() {
                        PrivateSettlementLifecycleDtoV1::Prepared
                    } else {
                        PrivateSettlementLifecycleDtoV1::Audited
                    },
                    prepare_certificate,
                    commit_certificate,
                })
            }
            "vote" if path.ends_with("/prepare-votes") => {
                let vote: PrivateSettlementPrepareVoteRequestV1 =
                    norito::json::from_slice(&http.body)?;
                self.vote(
                    &vote.manifest,
                    vote.payload_digest,
                    member,
                    PrivateSettlementPhaseV1::Prepare,
                    private_settlement_reserved_prepared_digest_v1(),
                )
            }
            "vote" => {
                let vote: PrivateSettlementCommitVoteRequestV1 =
                    norito::json::from_slice(&http.body)?;
                self.vote(
                    &vote.barrier.manifest,
                    vote.payload_digest,
                    member,
                    PrivateSettlementPhaseV1::Commit,
                    vote.barrier.prepared_bundle_digest,
                )
            }
            _ => {
                let handoff: PrivateSettlementPhaseCertificateRequestV1 =
                    norito::json::from_slice(&http.body)?;
                let leg = self.leg_of(handoff.payload_digest);
                let phase = handoff.certificate.body.phase;
                let (held, lifecycle) = match phase {
                    PrivateSettlementPhaseV1::Prepare => {
                        (&self.prepare, PrivateSettlementLifecycleDtoV1::Prepared)
                    }
                    PrivateSettlementPhaseV1::Commit => (
                        &self.commit,
                        PrivateSettlementLifecycleDtoV1::CommitCertified,
                    ),
                };
                let leg_ordinal = handoff.certificate.body.leg_ordinal;
                held.lock()
                    .unwrap()
                    .insert((leg, member), handoff.certificate);
                json(&PrivateSettlementPhaseCertificateResponseV1 {
                    bundle_id: handoff.manifest.bundle_id,
                    payload_digest: handoff.payload_digest,
                    leg_ordinal,
                    phase,
                    lifecycle,
                })
            }
        }
    }
}

fn run_phase(
    client: &Client,
    committees: &Arc<Committees>,
    phase: usize,
    journal: &WorkflowJournalV1,
    read_only: bool,
) -> std::result::Result<Option<Vec<u8>>, PrivateSettlementError> {
    let scripted = Arc::clone(committees);
    with_mock_http(
        move |http| scripted.respond(&http),
        |transport| {
            phases::run(
                &client.clone().with_test_http_transport(transport),
                &committees.request,
                phase,
                journal,
                read_only,
            )
        },
    )
}

fn journal_with_availability(
    request: &PrivateSettlementRequest,
    certificates: &[PrivateSettlementSidecarAvailabilityV1],
) -> WorkflowJournalV1 {
    let binding = WorkflowBindingV1 {
        network: request.manifest().network_id.to_string(),
        authority: "test-sponsor".into(),
    };
    let mut journal = WorkflowJournalV1::new(&request.plan(), &binding, 1).unwrap();
    complete_round(
        &mut journal,
        AVAILABILITY,
        norito::encode_canonical(&certificates.to_vec()).unwrap(),
    );
    journal
}

// Every stage of a phase arrives as one block holding all legs' committee requests.
fn assert_stage_blocks(events: &[&'static str], stages: &[&'static str], requests: usize) {
    assert_eq!(events.len(), stages.len() * requests, "{events:?}");
    for (block, stage) in events.chunks(requests).zip(stages) {
        assert!(
            block.iter().all(|event| event == stage),
            "a stage started before the previous one joined every leg: {events:?}"
        );
    }
}

#[test]
fn availability_phase_certifies_every_leg_together() {
    let client = client_with_base_url(base_url());
    let (request, certificates) = prepared(&client);
    let requests = request.0.materials.len() * 4;
    let committees = Committees::new(&request, &certificates, None, Some(requests));
    let binding = WorkflowBindingV1 {
        network: request.manifest().network_id.to_string(),
        authority: "test-sponsor".into(),
    };
    let journal = WorkflowJournalV1::new(&request.plan(), &binding, 1).unwrap();
    let transcript = run_phase(&client, &committees, AVAILABILITY, &journal, false)
        .expect("both legs' availability requests are in flight together")
        .expect("availability completes");
    assert_eq!(
        transcript,
        norito::encode_canonical(&certificates).unwrap(),
        "the canonical lowest three shares of each leg are recorded in leg order"
    );
    assert_stage_blocks(&committees.events(), &["availability"], requests);
    // A read-only reopen never asks a committee for provisional custody.
    assert_eq!(
        run_phase(&client, &committees, AVAILABILITY, &journal, true).unwrap(),
        None
    );
    assert_eq!(committees.events().len(), requests);
}

#[test]
fn prepare_phase_joins_every_promotion_before_any_vote_and_stages_every_leg_together() {
    let client = client_with_base_url(base_url());
    let (request, certificates) = prepared(&client);
    let requests = request.0.materials.len() * 4;
    let committees = Committees::new(&request, &certificates, None, Some(requests));
    let journal = journal_with_availability(&request, &certificates);
    let transcript = run_phase(&client, &committees, PREPARE, &journal, false)
        .expect("each stage holds both legs' requests in flight together")
        .expect("Prepare completes");
    let manifest = request.manifest_with(&certificates).unwrap();
    let expected = phase_certificates(
        &request,
        &manifest,
        PrivateSettlementPhaseV1::Prepare,
        private_settlement_reserved_prepared_digest_v1(),
    );
    assert_eq!(
        transcript,
        norito::encode_canonical(&expected).unwrap(),
        "each leg records its canonical lowest-three QC, in leg order"
    );
    // Promotion of every leg joins before any committee is asked to stage or vote; each
    // later round of every leg then overlaps the other leg's same round.
    assert_stage_blocks(
        &committees.events(),
        &["promotion", "recover", "vote", "persist", "recover"],
        requests,
    );
    // Read-only recovery overlaps the legs too and performs reads only.
    let before = committees.events().len();
    let recovered = run_phase(&client, &committees, PREPARE, &journal, true)
        .unwrap()
        .expect("both durable QCs are recovered");
    assert_eq!(recovered, transcript);
    assert_stage_blocks(&committees.events()[before..], &["recover"], requests);
}

#[test]
fn commit_phase_votes_every_leg_together_over_the_registered_barrier() {
    let client = client_with_base_url(base_url());
    let (request, certificates) = prepared(&client);
    let requests = request.0.materials.len() * 4;
    let journal = checkpoint(&request, &certificates);
    let barrier = request.barrier(&journal).unwrap();
    let committees = Committees::new(
        &request,
        &certificates,
        Some(barrier.clone()),
        Some(requests),
    );
    // Nothing is durable yet: a read-only reopen reports Waiting after reads alone.
    assert_eq!(
        run_phase(&client, &committees, COMMIT, &journal, true).unwrap(),
        None
    );
    assert_stage_blocks(&committees.events(), &["recover"], requests);
    let before = committees.events().len();
    let transcript = run_phase(&client, &committees, COMMIT, &journal, false)
        .expect("each stage holds both legs' requests in flight together")
        .expect("Commit completes");
    let expected = phase_certificates(
        &request,
        &barrier.manifest,
        PrivateSettlementPhaseV1::Commit,
        barrier.prepared_bundle_digest,
    );
    assert_eq!(transcript, norito::encode_canonical(&expected).unwrap());
    assert_stage_blocks(
        &committees.events()[before..],
        &["recover", "vote", "persist", "recover"],
        requests,
    );
}

#[test]
fn a_refused_leg_fails_prepare_and_the_later_leg_recovers_its_durable_certificate() {
    let client = client_with_base_url(base_url());
    let (request, certificates) = prepared(&client);
    let committees = Committees::new(&request, &certificates, None, None);
    let journal = journal_with_availability(&request, &certificates);
    *committees.refuse_votes.lock().unwrap() = Some(0);
    let error = run_phase(&client, &committees, PREPARE, &journal, false)
        .expect_err("a leg without a vote quorum fails the phase");
    let PrivateSettlementError::Protocol(cause) = error else {
        panic!("the original committee refusal was replaced: {error:?}");
    };
    assert!(
        cause.to_string().contains("phase quorum is unavailable"),
        "the first leg's refusal is returned: {cause}"
    );
    // The later leg shared the window: its committee holds a durable QC, the refused leg none.
    let held = committees.prepare.lock().unwrap().clone();
    assert_eq!(
        held.keys().copied().collect::<Vec<_>>(),
        (0..4).map(|member| (1, member)).collect::<Vec<_>>()
    );
    let original = held[&(1, 0)].clone();
    assert!(held.values().all(|certificate| certificate == &original));
    // The retry recovers that exact certificate instead of collecting a new one.
    *committees.refuse_votes.lock().unwrap() = None;
    let transcript = run_phase(&client, &committees, PREPARE, &journal, false)
        .unwrap()
        .expect("both legs are certified");
    let recorded: Vec<PrivateSettlementPhaseCertificateV1> =
        norito::decode_canonical(&transcript).unwrap();
    assert_eq!(recorded.len(), 2);
    assert_eq!(
        recorded[1], original,
        "the later leg keeps the QC it obtained beside the refused leg"
    );
    assert_eq!(recorded[0].body.leg_ordinal, 0);
    assert_eq!(
        committees.prepare.lock().unwrap().len(),
        8,
        "every member of both committees now holds its leg's QC"
    );
}
