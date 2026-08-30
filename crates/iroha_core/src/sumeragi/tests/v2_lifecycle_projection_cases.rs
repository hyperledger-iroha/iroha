use super::super::{
    AdmissionDecision, AdmissionRejection, AuthenticatedLifecycleRecoveryCut, LifecycleCoordinator,
    LifecycleState, LifecycleWorkRegistryHolder, RetryAction, RolloverSnapshot, SchedulerInputs,
    SchedulerReadyInputs, TurnPlan, WaitSource,
    schema::{CapacityClass, CapacityGeometry},
};
use super::*;
use crate::sumeragi::{
    v2::AdapterEquivocationEvidence,
    v2_body_store::V2BodyStore,
    v2_certified_serve_payload_store::{
        AuthenticatedCertifiedServePayloadRecoveryCut, CertifiedServePayloadNegativeOutcome,
        CertifiedServePayloadStoreV1,
    },
    v2_chunks::encode_payload,
    v2_core::{EventTag, Generation},
    v2_runtime::{RuntimeEffectOwnership, bind_adapter_effect_batch_ownership},
    v2_transport::authenticate_certified_body_request,
};
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, Signature, SignatureOf};
use iroha_data_model::{
    block::{BlockHeader, BlockSignature, SignedBlock},
    peer::PeerId,
};
use std::{collections::BTreeSet, num::NonZeroU64};
use tempfile::TempDir;
struct Fixture {
    verified: VerifiedHeightContext,
    keys: Vec<KeyPair>,
    context: wire::HeightContext,
    round: wire::ConsensusRound,
    tag: EventTag,
    body: Vec<u8>,
    encoded_chunks: Vec<Vec<u8>>,
    subject: wire::BlockSubject,
    manifest: wire::PayloadManifest,
    proposal: wire::Proposal,
    prepare_vote: wire::Vote,
    commit_vote: wire::Vote,
    prepare_qc: wire::QuorumCertificate,
    commit_qc: wire::QuorumCertificate,
    timeout_vote: wire::TimeoutVote,
    timeout_certificate: wire::TimeoutCertificate,
}
type ExpectedProjection = (
    AdapterEffect,
    LifecycleWorkClass,
    LifecyclePhase,
    LifecycleStageKind,
);
impl Fixture {
    #[allow(clippy::too_many_lines)]
    fn new() -> Self {
        let mut keys = (1_u8..=4)
            .map(|seed| {
                KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                    .expect("deterministic lifecycle-projection BLS key")
            })
            .collect::<Vec<_>>();
        keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
        let proofs = keys
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("fixture BLS proof of possession")
            })
            .collect::<Vec<_>>();
        let roster = keys
            .iter()
            .map(|key| wire::ValidatorPower {
                validator: PeerId::new(key.public_key().clone()),
                power: 1,
            })
            .collect::<Vec<_>>();
        let context = wire::HeightContext {
            network_id: crate::sumeragi::synthetic_network_id(
                "sumeragi-v2-lifecycle-projection-test",
            ),
            protocol_version: wire::PROTOCOL_VERSION,
            height: 1,
            epoch: 1,
            epoch_end_height: 100,
            next_epoch_snapshot: None,
            mode: wire::ConsensusMode::Permissioned,
            parent_commit_qc: None,
            snapshot_bootstrap: None,
            quorum: wire::DualQuorum::from_roster(&roster).expect("fixture quorum"),
            roster,
            nexus_amx_context_hash: Hash::new(b"lifecycle projection nexus context"),
            execution_policy_hash: Hash::new(b"lifecycle projection execution policy"),
            da_layout: wire::DataAvailabilityLayout {
                encoding: wire::PayloadEncoding::ReedSolomon16,
                chunk_size_bytes: 1024,
                data_shards: 1,
                parity_shards: 1,
                max_payload_size_bytes: 512 * 1024,
                max_chunk_count: 1024,
            },
            leader_seed: [0xA7; 32],
        };
        let verified = VerifiedHeightContext::genesis(context.clone(), proofs)
            .expect("verified lifecycle-projection context");
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 0,
        };
        let tag = EventTag::new(context.height, round.view, Generation::new(1));
        let body = vec![0x41; 4];
        let subject = block_subject_for_body(&body, 0x41);
        let encoded_chunks =
            wire::encode_payload_chunks(context.da_layout, &body).expect("encode fixture body");
        let manifest = wire::PayloadManifest::derive(
            &context,
            round,
            subject,
            u64::try_from(body.len()).expect("small fixture body"),
            &encoded_chunks,
        )
        .expect("derive fixture manifest");
        let proposal = wire::Proposal {
            round,
            proposer: context.leader(round.view),
            subject,
            manifest: manifest.clone(),
            justification: wire::ProposalJustification::ParentCommit(
                wire::ParentCommitJustification { certificate: None },
            ),
            signature: vec![0x41],
        };
        let commitment = execution_commitment_for(0x41);
        let prepare_vote = wire::Vote {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Prepare,
            subject,
            execution_commitment: commitment,
            signer: 0,
            signature: vec![0x42],
        };
        let commit_vote = wire::Vote {
            phase: wire::GlobalPhase::Commit,
            signature: vec![0x43],
            ..prepare_vote.clone()
        };
        let prepare_qc = wire::QuorumCertificate {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Prepare,
            subject,
            execution_commitment: commitment,
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0x44],
        };
        let commit_qc = wire::QuorumCertificate {
            phase: wire::GlobalPhase::Commit,
            aggregate_signature: vec![0x45],
            ..prepare_qc.clone()
        };
        let timeout_vote = wire::TimeoutVote {
            round,
            highest_prepare_qc: None,
            signer: 0,
            signature: vec![0x46],
        };
        let timeout_certificate = wire::TimeoutCertificate {
            round,
            groups: vec![wire::TimeoutVoteGroup {
                highest_prepare_qc: None,
                signers: vec![0, 1, 2],
                aggregate_signature: vec![0x47],
            }],
        };
        Self {
            verified,
            keys,
            context,
            round,
            tag,
            body,
            encoded_chunks,
            subject,
            manifest,
            proposal,
            prepare_vote,
            commit_vote,
            prepare_qc,
            commit_qc,
            timeout_vote,
            timeout_certificate,
        }
    }
    fn coordinator(&self) -> LifecycleCoordinator {
        LifecycleCoordinator::new(
            lifecycle_context(&self.context),
            0,
            CapacityGeometry::new(CapacityClass::ALL.map(|class| (class, 64))),
        )
    }
    fn authenticated_timeout_certificate(
        &self,
        signers: Vec<wire::ValidatorIndex>,
    ) -> wire::TimeoutCertificate {
        let signer = signers[0];
        let preimage = wire::TimeoutVote {
            round: self.round,
            highest_prepare_qc: None,
            signer,
            signature: Vec::new(),
        }
        .signature_preimage();
        let shares = signers
            .iter()
            .map(|signer| {
                Signature::new(
                    self.keys[usize::try_from(*signer).expect("small timeout signer")]
                        .private_key(),
                    &preimage,
                )
                .payload()
                .to_vec()
            })
            .collect::<Vec<_>>();
        let aggregate_signature = iroha_crypto::bls_normal_aggregate_signatures(
            &shares.iter().map(Vec::as_slice).collect::<Vec<_>>(),
        )
        .expect("aggregate authenticated timeout certificate");
        wire::TimeoutCertificate {
            round: self.round,
            groups: vec![wire::TimeoutVoteGroup {
                highest_prepare_qc: None,
                signers,
                aggregate_signature,
            }],
        }
    }
    fn authenticated_serve_request(
        &self,
        requester_index: usize,
    ) -> AuthenticatedCertifiedBodyRequest {
        self.authenticated_serve_request_for(self.round, self.subject, requester_index)
    }
    fn authenticated_serve_request_for(
        &self,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        requester_index: usize,
    ) -> AuthenticatedCertifiedBodyRequest {
        let execution_commitment = execution_commitment_for(0x81);
        let signers = vec![0, 1, 2];
        let preimage = wire::Vote {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Prepare,
            subject,
            execution_commitment,
            signer: 0,
            signature: Vec::new(),
        }
        .signature_preimage();
        let shares = signers
            .iter()
            .map(|signer| {
                Signature::new(
                    self.keys[usize::try_from(*signer).expect("small fixture signer")]
                        .private_key(),
                    &preimage,
                )
                .payload()
                .to_vec()
            })
            .collect::<Vec<_>>();
        let aggregate_signature = iroha_crypto::bls_normal_aggregate_signatures(
            &shares.iter().map(Vec::as_slice).collect::<Vec<_>>(),
        )
        .expect("aggregate fixture PrepareQC");
        let mut request = wire::CertifiedBodyRequest {
            round,
            subject,
            certificate: wire::QuorumCertificate {
                round,
                proposal_round: round,
                phase: wire::GlobalPhase::Prepare,
                subject,
                execution_commitment,
                signers,
                aggregate_signature,
            },
            requester: PeerId::new(self.keys[requester_index].public_key().clone()),
            signature: Vec::new(),
        };
        request.signature = Signature::new(
            self.keys[requester_index].private_key(),
            &request.signature_preimage(),
        )
        .payload()
        .to_vec();
        let requester = request.requester.clone();
        authenticate_certified_body_request(
            &self.context,
            request,
            &requester,
            |context, certificate| {
                wire::finality::verify_quorum_certificate_with_validator_pops(
                    context,
                    certificate,
                    self.verified.proofs_of_possession(),
                )
                .map_err(|error| error.to_string())
            },
        )
        .expect("authenticate exact fixture CertifiedBodyRequest")
    }
    fn canonical_body_and_manifest(&self) -> (Vec<u8>, wire::PayloadManifest) {
        let leader = self.context.leader(self.round.view);
        let leader_index = usize::try_from(leader).expect("fixture leader fits usize");
        let header = BlockHeader::new(
            NonZeroU64::new(self.round.height).expect("non-zero fixture height"),
            None,
            None,
            None,
            1_000,
            self.round.view,
        );
        let signature =
            SignatureOf::try_from_hash(self.keys[leader_index].private_key(), header.hash())
                .expect("sign fixture block header");
        let block = SignedBlock::presigned(
            BlockSignature::new(u64::from(leader), signature),
            header,
            Vec::new(),
        );
        let body = block.encode_wire().expect("canonical SignedBlockWire");
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: block.hash(),
            payload_hash: Hash::new(&body),
        };
        let manifest = encode_payload(&self.context, self.round, subject, &body)
            .expect("encode canonical fixture payload")
            .manifest()
            .clone();
        (body, manifest)
    }
    fn proposal_for(&self, marker: u8, signature: u8) -> wire::Proposal {
        let body = vec![marker; 4];
        let subject = block_subject_for_body(&body, marker);
        let encoded_chunks = wire::encode_payload_chunks(self.context.da_layout, &body)
            .expect("encode conflicting fixture body");
        let manifest = wire::PayloadManifest::derive(
            &self.context,
            self.round,
            subject,
            u64::try_from(body.len()).expect("small fixture body"),
            &encoded_chunks,
        )
        .expect("derive conflicting fixture manifest");
        wire::Proposal {
            round: self.round,
            proposer: self.context.leader(self.round.view),
            subject,
            manifest,
            justification: wire::ProposalJustification::ParentCommit(
                wire::ParentCommitJustification { certificate: None },
            ),
            signature: vec![signature],
        }
    }
}
fn block_subject_for_body(body: &[u8], marker: u8) -> wire::BlockSubject {
    wire::BlockSubject {
        parent_block_hash: None,
        block_hash: HashOf::from_untyped_unchecked(Hash::new([marker, 0xB1])),
        payload_hash: Hash::new(body),
    }
}
fn execution_commitment_for(marker: u8) -> wire::ExecutionCommitment {
    wire::ExecutionCommitment::without_topups_or_merge_carrier(
        Hash::new([marker, 1]),
        Hash::new([marker, 2]),
        Hash::new([marker, 3]),
        1,
        Hash::new([marker, 4]),
    )
}
fn bound_ownership(
    effect: &AdapterEffect,
    owner_tag: EventTag,
    ordinal: u128,
) -> RuntimeEffectOwnership {
    bind_adapter_effect_batch_ownership(
        core::slice::from_ref(effect),
        vec![RuntimeEffectOwnership::fresh_for_test(owner_tag, ordinal)],
    )
    .expect("bind exact lifecycle-projection effect")
    .pop()
    .expect("one bound ownership")
}
fn candidate(
    fixture: &Fixture,
    effect: &AdapterEffect,
    ownership: &RuntimeEffectOwnership,
) -> AuthorityFreeAdmissionProjection {
    let pending = ownership
        .exact_pending_adapter_effect_binding(effect)
        .expect("mint ordinal-free pending lifecycle binding");
    authority_free_admission_projection(
        lifecycle_context(&fixture.context),
        &fixture.verified,
        effect,
        &pending,
    )
    .expect("project exact bound adapter coordinates")
}
fn prepare_direct_signed(
    fixture: &Fixture,
    coordinator: &LifecycleCoordinator,
    effect: &AdapterEffect,
    ownership: &RuntimeEffectOwnership,
) -> super::super::work_registry::PreparedLifecycleAdmissionV1 {
    let pending = ownership
        .exact_pending_adapter_effect_binding(effect)
        .expect("mint exact direct-signed pending owner");
    coordinator
        .prepare_direct_signed_lifecycle_admission(&fixture.verified, effect.clone(), pending)
        .expect("direct-signed fixture retains its canonical replay authority")
}
fn certified_validate_candidate(
    fixture: &Fixture,
    fetch: &AdapterEffect,
    store: &AdapterEffect,
    validate: &AdapterEffect,
    validate_owner: &RuntimeEffectOwnership,
    receipt: &crate::sumeragi::v2_body_store::DurableBodyReceipt,
) -> CandidateAdmission {
    let response = wire::CertifiedBodyResponse {
        request_hash: HashOf::from_untyped_unchecked(Hash::new(
            b"lifecycle projection certified response request",
        )),
        manifest: fixture.manifest.clone(),
        body: fixture.body.clone(),
        responder: fixture.context.roster[0].validator.clone(),
        signature: vec![0xA5],
    };
    let fetch_evidence =
        CertifiedFetchReplayEvidenceV1::from_signed_response_for_test(fetch, &response, receipt)
            .expect("seal exact certified Fetch evidence");
    let store_evidence = fetch_evidence
        .project_store_for_test(store, receipt)
        .expect("project exact certified Store evidence");
    let validate_pending = validate_owner
        .exact_pending_adapter_effect_binding(validate)
        .expect("retain exact Validate pending binding");
    let validate_evidence = store_evidence
        .project_validate(store, receipt, validate, &validate_pending)
        .expect("project exact certified Validate evidence");
    super::super::replay_authority::DurableValidateReplayEvidenceV1::certified(validate_evidence)
        .project_candidate_for_test(&fixture.verified, validate, receipt, &validate_pending)
        .expect("sealed certified Validate evidence projects one candidate")
}
fn assert_candidate_shape(
    candidate: &AuthorityFreeAdmissionProjection,
    effect: &AdapterEffect,
    ownership: &RuntimeEffectOwnership,
    work_class: LifecycleWorkClass,
    phase: LifecyclePhase,
    stage_kind: LifecycleStageKind,
) {
    assert_eq!(candidate.work_class, work_class);
    assert_eq!(candidate.key.phase(), phase);
    assert_eq!(candidate.stage.kind(), stage_kind);
    assert_eq!(
        candidate.stage.predecessor_scope(),
        PredecessorScope::Independent
    );
    assert_eq!(candidate.initial_state, InitialLifecycleState::Ready);
    assert_eq!(
        candidate.causal_root.digest(),
        candidate.reconstruction_source
    );
    assert_eq!(candidate.physical_geometry.initial.len(), 1);
    assert_eq!(candidate.physical_geometry.replenishment_slots.len(), 1);
    let slot = candidate.physical_geometry.initial[0];
    assert_eq!(
        slot.id().capacity_class(),
        Some(work_class.capacity_class())
    );
    assert_eq!(slot.id().index(), 0);
    assert!(
        candidate
            .physical_geometry
            .replenishment_slots
            .contains(&slot.id())
    );
    let authority = ownership
        .exact_pending_adapter_effect_binding(effect)
        .expect("the tested effect remains exactly bound");
    assert_eq!(
        slot.digest(),
        digest_from_hash(authority.exact_effect_identity())
    );
}
fn vote_conflict(fixture: &Fixture) -> (wire::Vote, wire::Vote) {
    let first = fixture.prepare_vote.clone();
    let second = wire::Vote {
        subject: fixture.proposal_for(0x52, 0x53).subject,
        execution_commitment: execution_commitment_for(0x52),
        signature: vec![0x53],
        ..first.clone()
    };
    (first, second)
}
fn authenticated_payload_cut(
    fixture: &Fixture,
    payload_root: &std::path::Path,
    body_store: &V2BodyStore,
    local_signer: &KeyPair,
) -> (
    CertifiedServePayloadStoreV1,
    AuthenticatedCertifiedServePayloadRecoveryCut,
) {
    let (store, recovery) = CertifiedServePayloadStoreV1::open(payload_root, &fixture.context)
        .expect("reopen exact Certified-Serve payload store");
    let authenticated = recovery
        .authenticate(&fixture.verified, local_signer, body_store)
        .expect("authenticate exact Certified-Serve recovery cut");
    (store, authenticated)
}
fn lifecycle_recovery_cut(
    fixture: &Fixture,
    ledger_root: &std::path::Path,
    payloads: AuthenticatedCertifiedServePayloadRecoveryCut,
) -> AuthenticatedLifecycleRecoveryCut {
    let (_store, ledger) = super::super::ledger::LifecycleLedgerStoreV1::open(
        ledger_root,
        lifecycle_context(&fixture.context),
    )
    .expect("decode the exact lifecycle ledger authenticated by the fixture cut");
    AuthenticatedLifecycleRecoveryCut::from_authenticated_parts(ledger, [], [], payloads)
        .expect("assemble sealed lifecycle recovery cut")
}
fn execute_ready_turn(coordinator: &mut LifecycleCoordinator) -> super::super::TurnLease {
    let ready = coordinator.ready_index.iter().map(|ordinal| {
        let record = &coordinator.records[ordinal];
        (*ordinal, SchedulerReadyInputs::new(record, None, [0; 6]))
    });
    let TurnPlan::Execute(lease) = coordinator
        .plan_turn(SchedulerInputs::new([], ready).expect("Serve ready rows have unique ordinals"))
    else {
        panic!("one ready Certified-Serve record must execute")
    };
    lease
}
fn reduce_completed_serve_for_test(
    coordinator: &mut LifecycleCoordinator,
    lease: super::super::TurnLease,
    receipt: DurableCertifiedServeCompletedReceipt,
) -> bool {
    let Some(producer_ordinal) = coordinator.producer_debts.get(&lease.ordinal).copied() else {
        return false;
    };
    let terminal = CertifiedServeTerminalReplayAuthorityPairV1::from_completed_receipt(
        coordinator.active_context,
        &coordinator.records[&lease.ordinal],
        &coordinator.durable_records[&lease.ordinal],
        &coordinator.records[&producer_ordinal],
        &coordinator.durable_records[&producer_ordinal],
        receipt,
    );
    let Some(terminal) = terminal else {
        return false;
    };
    coordinator.settle_turn_with_durable_serve_terminal(lease, terminal);
    coordinator.fault().is_none()
}
fn reduce_negative_serve_for_test(
    coordinator: &mut LifecycleCoordinator,
    lease: super::super::TurnLease,
    receipt: DurableCertifiedServeNegativeReceipt,
) -> bool {
    let Some(producer_ordinal) = coordinator.producer_debts.get(&lease.ordinal).copied() else {
        return false;
    };
    let terminal = CertifiedServeTerminalReplayAuthorityPairV1::from_negative_receipt(
        coordinator.active_context,
        &coordinator.records[&lease.ordinal],
        &coordinator.durable_records[&lease.ordinal],
        &coordinator.records[&producer_ordinal],
        &coordinator.durable_records[&producer_ordinal],
        receipt,
    );
    let Some(terminal) = terminal else {
        return false;
    };
    coordinator.settle_turn_with_durable_serve_terminal(lease, terminal);
    coordinator.fault().is_none()
}
#[test]
#[allow(clippy::too_many_lines)]
fn pending_certified_serve_admits_one_ready_serve_and_adjacent_dormant_producer() {
    let temporary = TempDir::new().expect("temporary directory");
    let fixture = Fixture::new();
    let request = fixture.authenticated_serve_request(3);
    let (mut payload_store, _) =
        CertifiedServePayloadStoreV1::open(temporary.path(), &fixture.context)
            .expect("open payload store");
    let receipt = payload_store
        .persist_pending(&request)
        .expect("persist signed request before admission");
    let mut coordinator = fixture.coordinator();
    let decision = coordinator
        .admit_certified_serve(&fixture.verified, &request, receipt)
        .expect("project exact durable request");
    let AdmissionDecision::Admitted {
        ordinal,
        producer_turn_ordinal,
        ..
    } = decision
    else {
        panic!("fresh Certified-Serve request must be admitted")
    };
    assert_eq!(ordinal, 1);
    assert_eq!(producer_turn_ordinal, Some(2));
    assert_eq!(coordinator.records.len(), 2);
    assert!(matches!(
        coordinator.admit_certified_serve(&fixture.verified, &request, receipt),
        Ok(AdmissionDecision::Retry {
            ordinal: 1,
            action: RetryAction::ReenqueueIncumbent,
            ..
        })
    ));
    assert_eq!(coordinator.records.len(), 2, "exact retry remains 1 + 1");
    let serve = &coordinator.records[&1];
    let producer = &coordinator.records[&2];
    assert_eq!(serve.work_class, LifecycleWorkClass::CertifiedServe);
    assert_eq!(serve.stage.kind(), LifecycleStageKind::CertifiedServe);
    assert_eq!(
        serve.stage.predecessor_scope(),
        PredecessorScope::ReadyOrdinalPrefix
    );
    assert_eq!(serve.state, LifecycleState::Ready);
    assert_eq!(producer.work_class, LifecycleWorkClass::ProducerTurn);
    assert_eq!(producer.stage.kind(), LifecycleStageKind::ProducerTurn);
    assert_eq!(
        producer.stage.predecessor_scope(),
        PredecessorScope::ProducerHandoffBarrier
    );
    assert!(matches!(
        producer.state,
        LifecycleState::Waiting(wait)
            if wait.source() == WaitSource::ProducerTurn(ordinal)
    ));
    assert_eq!(producer.ordinal, ordinal + 1);
    assert_eq!(producer.owner, serve.owner);
    let expected_request = digest_from_bytes(request.request_hash().as_ref());
    let expected_certificate =
        digest_from_bytes(HashOf::new(&request.request().certificate).as_ref());
    assert_eq!(serve.owner.causal_root().digest(), expected_request);
    assert_eq!(
        coordinator.durable_records[&1].reconstruction_source,
        expected_request
    );
    assert_eq!(
        coordinator.durable_records[&2].reconstruction_source,
        expected_request
    );
    assert_eq!(
        coordinator.durable_records[&1].payload,
        DurablePayloadReference::CertifiedServePending {
            request: expected_request,
            certificate: expected_certificate,
        }
    );
    assert_eq!(
        serve.key.context(),
        lifecycle_context(&fixture.context).id()
    );
    assert_eq!(
        serve.key.round(),
        LifecycleRound::new(fixture.round.height, fixture.round.view)
    );
    assert_eq!(
        serve.key.proposal_round(),
        Some(LifecycleRound::new(
            fixture.round.height,
            fixture.round.view,
        ))
    );
    assert_eq!(
        serve.key.subject(),
        Some(certified_serve_key_subject(
            request.request().subject,
            request.request_hash(),
        ))
    );
    assert_ne!(
        serve.key.subject(),
        Some(block_subject(request.request().subject)),
        "Serve key subject is request-bound rather than a raw block subject"
    );
    assert_eq!(serve.key.phase(), LifecyclePhase::Serve);
    assert_eq!(
        serve.key.execution_commitment(),
        Some(execution_commitment(
            request.request().certificate.execution_commitment,
        ))
    );
    assert_eq!(producer.key.phase(), LifecyclePhase::ProducerTurn);
    assert_eq!(producer.key.subject(), serve.key.subject());
    assert_eq!(producer.key.context(), serve.key.context());
    assert_eq!(producer.key.round(), serve.key.round());
    assert_eq!(producer.key.proposal_round(), serve.key.proposal_round());
    assert_eq!(
        producer.key.execution_commitment(),
        serve.key.execution_commitment()
    );
    assert_eq!(
        serve.physical_slots.values().copied().collect::<Vec<_>>(),
        vec![digest_from_hash(&receipt.payload_hash())]
    );
    assert_eq!(
        serve.physical_slots.keys().copied().collect::<Vec<_>>(),
        vec![PhysicalSlotId::for_capacity(CapacityClass::Serve, 0)]
    );
    assert_eq!(
        producer
            .physical_slots
            .values()
            .copied()
            .collect::<Vec<_>>(),
        vec![domain_digest(
            PRODUCER_TURN_PHYSICAL_DOMAIN,
            request.request_hash().as_ref(),
        )]
    );
    assert_eq!(
        producer.physical_slots.keys().copied().collect::<Vec<_>>(),
        vec![PhysicalSlotId::for_capacity(CapacityClass::Producer, 0)]
    );
}
#[test]
fn capacity_wait_retains_one_bounded_payload_publication() {
    let temporary = TempDir::new().expect("temporary directory");
    let fixture = Fixture::new();
    let first = fixture.authenticated_serve_request(2);
    let waiting = fixture.authenticated_serve_request(3);
    let (mut payload_store, _) =
        CertifiedServePayloadStoreV1::open(temporary.path(), &fixture.context)
            .expect("open payload store");
    let mut coordinator = LifecycleCoordinator::new(
        lifecycle_context(&fixture.context),
        0,
        CapacityGeometry::new([
            (CapacityClass::Consensus, 64),
            (CapacityClass::Effect, 64),
            (CapacityClass::Serve, 1),
            (CapacityClass::Producer, 2),
        ]),
    );
    assert!(matches!(
        coordinator.persist_and_admit_certified_serve(
            &mut payload_store,
            &fixture.verified,
            &fixture.keys[0],
            &first,
        ),
        Ok(AdmissionDecision::Admitted { ordinal: 1, .. })
    ));
    assert!(matches!(
        coordinator.persist_and_admit_certified_serve(
            &mut payload_store,
            &fixture.verified,
            &fixture.keys[0],
            &waiting,
        ),
        Ok(AdmissionDecision::WaitForCapacity(_))
    ));
    drop(payload_store);
    let (mut payload_store, recovery) =
        CertifiedServePayloadStoreV1::open(temporary.path(), &fixture.context)
            .expect("reopen payload store with one bounded wait");
    assert_eq!(recovery.len(), 2);
    assert!(
        recovery
            .iter()
            .any(|payload| payload.id().request_hash() == first.request_hash())
    );
    assert!(
        recovery
            .iter()
            .any(|payload| payload.id().request_hash() == waiting.request_hash())
    );
    assert!(matches!(
        coordinator.persist_and_admit_certified_serve(
            &mut payload_store,
            &fixture.verified,
            &fixture.keys[0],
            &waiting,
        ),
        Ok(AdmissionDecision::WaitForCapacity(_))
    ));
    drop(payload_store);
    let (_, recovery) = CertifiedServePayloadStoreV1::open(temporary.path(), &fixture.context)
        .expect("reopen after unchanged-generation retry");
    assert_eq!(
        recovery.len(),
        2,
        "retries reuse the single payload owned by the admission wait"
    );
}
#[test]
fn conclusive_admission_rejection_rolls_back_the_pending_payload() {
    let temporary = TempDir::new().expect("temporary directory");
    let fixture = Fixture::new();
    let request = fixture.authenticated_serve_request(3);
    let (mut payload_store, _) =
        CertifiedServePayloadStoreV1::open(temporary.path(), &fixture.context)
            .expect("open payload store");
    let mut coordinator = LifecycleCoordinator::new(
        lifecycle_context(&fixture.context),
        0,
        CapacityGeometry::new([
            (CapacityClass::Consensus, 64),
            (CapacityClass::Effect, 64),
            (CapacityClass::Serve, 0),
            (CapacityClass::Producer, 1),
        ]),
    );
    assert!(matches!(
        coordinator.persist_and_admit_certified_serve(
            &mut payload_store,
            &fixture.verified,
            &fixture.keys[0],
            &request,
        ),
        Ok(AdmissionDecision::Rejected(
            AdmissionRejection::InvalidEpisodeUniverse
        ))
    ));
    drop(payload_store);
    let (_, recovery) = CertifiedServePayloadStoreV1::open(temporary.path(), &fixture.context)
        .expect("reopen after conclusive rejection");
    assert!(
        recovery.is_empty(),
        "a rejected request cannot consume durable payload capacity"
    );
}
#[test]
fn certified_serve_negative_settlement_requires_the_exact_post_fsync_receipt() {
    let temporary = TempDir::new().expect("temporary directory");
    let fixture = Fixture::new();
    let request = fixture.authenticated_serve_request(3);
    let other = fixture.authenticated_serve_request(2);
    let (mut payload_store, _) =
        CertifiedServePayloadStoreV1::open(temporary.path(), &fixture.context)
            .expect("open payload store");
    let pending = payload_store
        .persist_pending(&request)
        .expect("persist admitted request");
    let other_pending = payload_store
        .persist_pending(&other)
        .expect("persist foreign request");
    let foreign_terminal = payload_store
        .persist_negative(
            other_pending.id(),
            CertifiedServePayloadNegativeOutcome::Rejected(17),
        )
        .expect("persist foreign negative result");
    let mut coordinator = fixture.coordinator();
    assert!(matches!(
        coordinator.admit_certified_serve(&fixture.verified, &request, pending),
        Ok(AdmissionDecision::Admitted { ordinal: 1, .. })
    ));
    let pending_serve_replay = coordinator.durable_records[&1].replay_authority.clone();
    let pending_producer_replay = coordinator.durable_records[&2].replay_authority.clone();
    let lease = execute_ready_turn(&mut coordinator);
    assert!(!reduce_negative_serve_for_test(
        &mut coordinator,
        lease.clone(),
        foreign_terminal,
    ));
    assert_eq!(coordinator.active_lease, Some(lease.clone()));
    let terminal = payload_store
        .persist_negative(
            pending.id(),
            CertifiedServePayloadNegativeOutcome::Rejected(19),
        )
        .expect("persist exact negative result");
    assert!(reduce_negative_serve_for_test(
        &mut coordinator,
        lease,
        terminal,
    ));
    assert_eq!(
        coordinator.records[&1].state,
        LifecycleState::Terminal(TerminalOutcome::Rejected(19))
    );
    assert_eq!(coordinator.records[&2].state, LifecycleState::Ready);
    assert!(matches!(
        coordinator.durable_records[&1].payload,
        DurablePayloadReference::CertifiedServeNegative {
            outcome: DurableServeNegativeOutcome::Rejected(19),
            ..
        }
    ));
    assert!(
        !pending_serve_replay
            .same_persisted_family(&coordinator.durable_records[&1].replay_authority)
    );
    assert!(
        !pending_producer_replay
            .same_persisted_family(&coordinator.durable_records[&2].replay_authority)
    );
    assert!(
        coordinator.durable_records[&1]
            .replay_authority
            .same_persisted_family(&coordinator.durable_records[&2].replay_authority)
    );
    assert!(
        coordinator.durable_records[&1]
            .replay_authority
            .certified_serve_frame_hash_is(terminal.payload_hash())
    );
    assert!(
        coordinator.durable_records[&2]
            .replay_authority
            .certified_serve_frame_hash_is(terminal.payload_hash())
    );
    assert!(matches!(
        coordinator.persist_and_admit_certified_serve(
            &mut payload_store,
            &fixture.verified,
            &fixture.keys[0],
            &request,
        ),
        Ok(AdmissionDecision::StutterTerminal { .. })
    ));
}
#[test]
fn certified_serve_terminal_family_mismatch_fails_without_state_mutation() {
    let temporary = TempDir::new().expect("temporary directory");
    let fixture = Fixture::new();
    let request = fixture.authenticated_serve_request(3);
    let (mut payload_store, _) =
        CertifiedServePayloadStoreV1::open(temporary.path(), &fixture.context)
            .expect("open payload store");
    let pending = payload_store
        .persist_pending(&request)
        .expect("persist admitted request");
    let mut coordinator = fixture.coordinator();
    assert!(matches!(
        coordinator.admit_certified_serve(&fixture.verified, &request, pending),
        Ok(AdmissionDecision::Admitted { ordinal: 1, .. })
    ));
    let lease = execute_ready_turn(&mut coordinator);
    let terminal = payload_store
        .persist_negative(
            pending.id(),
            CertifiedServePayloadNegativeOutcome::Failed(31),
        )
        .expect("persist exact negative result");
    let foreign_producer_replay = coordinator.durable_records[&2]
        .replay_authority
        .with_certified_serve_frame_hash_for_test(Hash::new(
            b"foreign pending ProducerTurn payload frame",
        ))
        .expect("ProducerTurn retains a Certified-Serve storage source");
    coordinator
        .durable_records
        .get_mut(&2)
        .expect("admission retained ProducerTurn metadata")
        .replay_authority = foreign_producer_replay;
    let records = coordinator.records.clone();
    let durable_records = coordinator.durable_records.clone();
    let ready_index = coordinator.ready_index.clone();
    let producer_debts = coordinator.producer_debts.clone();
    let capacity_used = coordinator.capacity_used.clone();
    let active_lease = coordinator.active_lease.clone();
    assert!(!reduce_negative_serve_for_test(
        &mut coordinator,
        lease,
        terminal,
    ));
    assert_eq!(coordinator.records, records);
    assert_eq!(coordinator.durable_records, durable_records);
    assert_eq!(coordinator.ready_index, ready_index);
    assert_eq!(coordinator.producer_debts, producer_debts);
    assert_eq!(coordinator.capacity_used, capacity_used);
    assert_eq!(coordinator.active_lease, active_lease);
    assert_eq!(coordinator.fault(), None);
}
#[test]
fn cancelled_certified_serve_tombstone_replays_with_its_terminal_producer_pair() {
    let temporary = TempDir::new().expect("temporary directory");
    let fixture = Fixture::new();
    let request = fixture.authenticated_serve_request(3);
    let (mut payload_store, _) =
        CertifiedServePayloadStoreV1::open(temporary.path(), &fixture.context)
            .expect("open payload store");
    let pending = payload_store
        .persist_pending(&request)
        .expect("persist admitted request");
    let mut coordinator = fixture.coordinator();
    assert!(matches!(
        coordinator.admit_certified_serve(&fixture.verified, &request, pending),
        Ok(AdmissionDecision::Admitted { ordinal: 1, .. })
    ));
    let lease = execute_ready_turn(&mut coordinator);
    let terminal = payload_store
        .persist_negative(
            pending.id(),
            CertifiedServePayloadNegativeOutcome::Cancelled,
        )
        .expect("persist cancellation before ledger settlement");
    assert!(reduce_negative_serve_for_test(
        &mut coordinator,
        lease,
        terminal,
    ));
    assert_eq!(
        coordinator.records[&1].state,
        LifecycleState::Terminal(TerminalOutcome::Cancelled)
    );
    assert_eq!(
        coordinator.records[&2].state,
        LifecycleState::Terminal(TerminalOutcome::Cancelled)
    );
    assert!(!coordinator.producer_debts.contains_key(&1));
    assert!(matches!(
        coordinator.persist_and_admit_certified_serve(
            &mut payload_store,
            &fixture.verified,
            &fixture.keys[0],
            &request,
        ),
        Ok(AdmissionDecision::StutterTerminal { .. })
    ));
}
#[test]
fn certified_serve_completion_settles_from_the_post_fsync_response_receipt() {
    let temporary = TempDir::new().expect("temporary directory");
    let fixture = Fixture::new();
    let request = fixture.authenticated_serve_request(3);
    let (mut payload_store, _) =
        CertifiedServePayloadStoreV1::open(temporary.path(), &fixture.context)
            .expect("open payload store");
    let pending = payload_store
        .persist_pending(&request)
        .expect("persist admitted request");
    let mut coordinator = fixture.coordinator();
    assert!(matches!(
        coordinator.admit_certified_serve(&fixture.verified, &request, pending),
        Ok(AdmissionDecision::Admitted { ordinal: 1, .. })
    ));
    let pending_serve_replay = coordinator.durable_records[&1].replay_authority.clone();
    let pending_producer_replay = coordinator.durable_records[&2].replay_authority.clone();
    let lease = execute_ready_turn(&mut coordinator);
    let responder_index = 0;
    let mut response = wire::CertifiedBodyResponse {
        request_hash: request.request_hash(),
        manifest: fixture.manifest.clone(),
        body: fixture.body.clone(),
        responder: fixture.context.roster[responder_index].validator.clone(),
        signature: Vec::new(),
    };
    response.signature = Signature::new(
        fixture.keys[responder_index].private_key(),
        &response.signature_preimage(),
    )
    .payload()
    .to_vec();
    let terminal = payload_store
        .persist_completed(&request, &response)
        .expect("persist exact response metadata");
    assert!(reduce_completed_serve_for_test(
        &mut coordinator,
        lease,
        terminal,
    ));
    let response = digest_from_bytes(HashOf::new(&response).as_ref());
    assert_eq!(
        coordinator.records[&1].state,
        LifecycleState::Terminal(TerminalOutcome::Completed(Some(response)))
    );
    assert_eq!(coordinator.records[&2].state, LifecycleState::Ready);
    assert!(matches!(
        coordinator.durable_records[&1].payload,
        DurablePayloadReference::CertifiedServeCompleted {
            response: retained,
            ..
        } if retained == response
    ));
    assert!(
        !pending_serve_replay
            .same_persisted_family(&coordinator.durable_records[&1].replay_authority)
    );
    assert!(
        !pending_producer_replay
            .same_persisted_family(&coordinator.durable_records[&2].replay_authority)
    );
    assert!(
        coordinator.durable_records[&1]
            .replay_authority
            .same_persisted_family(&coordinator.durable_records[&2].replay_authority)
    );
    assert!(
        coordinator.durable_records[&1]
            .replay_authority
            .certified_serve_frame_hash_is(terminal.payload_hash())
    );
    assert!(
        coordinator.durable_records[&2]
            .replay_authority
            .certified_serve_frame_hash_is(terminal.payload_hash())
    );
    assert!(matches!(
        coordinator.persist_and_admit_certified_serve(
            &mut payload_store,
            &fixture.verified,
            &fixture.keys[0],
            &request,
        ),
        Ok(AdmissionDecision::ReplayTerminal {
            outcome: TerminalOutcome::Completed(Some(retained)),
            ..
        }) if retained == response
    ));
}
#[test]
fn certified_serve_rejects_a_receipt_for_another_signed_request() {
    let temporary = TempDir::new().expect("temporary directory");
    let fixture = Fixture::new();
    let first = fixture.authenticated_serve_request(2);
    let second = fixture.authenticated_serve_request(3);
    let (mut payload_store, _) =
        CertifiedServePayloadStoreV1::open(temporary.path(), &fixture.context)
            .expect("open payload store");
    let _ = payload_store
        .persist_pending(&first)
        .expect("persist first request");
    let second_receipt = payload_store
        .persist_pending(&second)
        .expect("persist second request");
    let mut coordinator = fixture.coordinator();
    assert_eq!(
        coordinator.admit_certified_serve(&fixture.verified, &first, second_receipt),
        Err(CertifiedServeAdmissionError::ReceiptMismatch)
    );
    assert!(coordinator.records.is_empty());
}
#[test]
fn two_signed_requests_for_one_body_have_distinct_serve_lifecycles() {
    let temporary = TempDir::new().expect("temporary directory");
    let fixture = Fixture::new();
    let first = fixture.authenticated_serve_request(2);
    let second = fixture.authenticated_serve_request(3);
    assert_eq!(first.request().subject, second.request().subject);
    assert_ne!(first.request_hash(), second.request_hash());
    let (mut payload_store, _) =
        CertifiedServePayloadStoreV1::open(temporary.path(), &fixture.context)
            .expect("open payload store");
    let first_receipt = payload_store
        .persist_pending(&first)
        .expect("persist first request");
    let second_receipt = payload_store
        .persist_pending(&second)
        .expect("persist second request");
    let mut coordinator = fixture.coordinator();
    assert!(matches!(
        coordinator.admit_certified_serve(&fixture.verified, &first, first_receipt),
        Ok(AdmissionDecision::Admitted {
            ordinal: 1,
            producer_turn_ordinal: Some(2),
            ..
        })
    ));
    assert!(matches!(
        coordinator.admit_certified_serve(&fixture.verified, &second, second_receipt),
        Ok(AdmissionDecision::Admitted {
            ordinal: 3,
            producer_turn_ordinal: Some(4),
            ..
        })
    ));
    assert_ne!(coordinator.records[&1].key, coordinator.records[&3].key);
    assert_ne!(
        coordinator.records[&1].key.subject(),
        coordinator.records[&3].key.subject()
    );
}
#[test]
fn durable_rollover_removes_the_exact_capacity_wait_payload() {
    let temporary = TempDir::new().expect("temporary directory");
    let retired_ledger_root = temporary.path().join("retired-ledger");
    let successor_ledger_root = temporary.path().join("successor-ledger");
    let payload_root = temporary.path().join("payloads");
    let fixture = Fixture::new();
    let first = fixture.authenticated_serve_request(2);
    let waiting = fixture.authenticated_serve_request(3);
    let geometry = CapacityGeometry::new([
        (CapacityClass::Consensus, 4),
        (CapacityClass::Effect, 4),
        (CapacityClass::Serve, 1),
        (CapacityClass::Producer, 1),
    ]);
    let mut coordinator =
        LifecycleCoordinator::new(lifecycle_context(&fixture.context), 0, geometry.clone());
    coordinator
        .attach_empty_test_ledger(&retired_ledger_root)
        .expect("attach retired lifecycle ledger");
    let (mut payload_store, _) =
        CertifiedServePayloadStoreV1::open(&payload_root, &fixture.context)
            .expect("open retired payload store");
    let first_receipt = payload_store
        .persist_pending_with_verified_retention(&fixture.verified, &fixture.keys[0], &first)
        .expect("persist first request");
    assert!(matches!(
        coordinator
            .persist_and_admit_certified_serve(
                &mut payload_store,
                &fixture.verified,
                &fixture.keys[0],
                &first,
            )
            .expect("admit first Serve"),
        AdmissionDecision::Admitted {
            ordinal: 1,
            producer_turn_ordinal: Some(2),
            ..
        }
    ));
    assert!(matches!(
        coordinator
            .persist_and_admit_certified_serve(
                &mut payload_store,
                &fixture.verified,
                &fixture.keys[0],
                &waiting,
            )
            .expect("retain one exact capacity fence"),
        AdmissionDecision::WaitForCapacity(_)
    ));
    let waiting_key = *coordinator
        .admission_waits
        .keys()
        .next()
        .expect("capacity wait remains coordinator-owned");
    assert!(
        coordinator.admission_waits[&waiting_key]
            .serve_payload_receipt
            .is_some(),
        "the sealed admission boundary retains its own rollback receipt"
    );
    let live_cut = payload_store
        .authenticate_current_for_lifecycle_retirement(
            super::super::ProductionLifecycleServeRetirementAuthenticationPermitV1::for_test(),
            &fixture.verified,
            &fixture.keys[0],
        )
        .expect("authenticate admitted and wait-owned live Serve payloads");
    let live_ledger = super::super::ledger::LifecycleLedgerV1::from_coordinator(&coordinator)
        .expect("project the exact live finalization ledger");
    let retained = super::super::open::authenticate_live_finalization_serve_census(
        &fixture.verified,
        &live_ledger,
        &coordinator,
        &live_cut,
    )
    .expect("join the exact ledger and admission-wait payload census");
    assert_eq!(retained, BTreeSet::from([first_receipt.id()]));
    let exact_wait_receipt = coordinator.admission_waits[&waiting_key]
        .serve_payload_receipt
        .expect("capacity wait owns its exact payload receipt");
    coordinator
        .admission_waits
        .get_mut(&waiting_key)
        .expect("capacity wait remains installed")
        .serve_payload_receipt = Some(
        exact_wait_receipt
            .with_request_hash_for_test(HashOf::from_untyped_unchecked(Hash::new([0xE7; 32]))),
    );
    assert!(
        super::super::open::authenticate_live_finalization_serve_census(
            &fixture.verified,
            &live_ledger,
            &coordinator,
            &live_cut,
        )
        .is_err(),
        "a drifted wait receipt must not authenticate an unrelated pending payload"
    );
    coordinator
        .admission_waits
        .get_mut(&waiting_key)
        .expect("capacity wait remains installed")
        .serve_payload_receipt = Some(exact_wait_receipt);
    let cancellation = payload_store
        .persist_negative(
            first_receipt.id(),
            CertifiedServePayloadNegativeOutcome::Cancelled,
        )
        .expect("persist exact admitted-Serve cancellation");
    let successor = LifecycleContext::new(LifecycleDigest::new([0xDD; 32]), 2);
    let successor_authority = super::super::authority::test_authority(
        successor,
        (2_u8..=5).map(|byte| LifecycleDigest::new([byte; 32])),
        0,
        geometry,
    )
    .expect("construct successor test authority");
    coordinator.rollover_with_payload_store(
        RolloverSnapshot {
            retired_context: lifecycle_context(&fixture.context),
            successor_context: successor,
            successor_predecessor: lifecycle_context(&fixture.context).id(),
            successor_authority,
            successor_ledger_root: Some(successor_ledger_root),
            serve_cancellations: vec![cancellation],
            retained_high_water: 2,
            retire_ordinals: BTreeSet::from([1, 2]),
            retire_admission_keys: BTreeSet::from([waiting_key]),
        },
        &mut payload_store,
    );
    assert_eq!(coordinator.fault(), None);
    assert_eq!(coordinator.active_context(), successor);
    drop(payload_store);
    let (_, recovered) = CertifiedServePayloadStoreV1::open(&payload_root, &fixture.context)
        .expect("reopen retired payload store");
    assert_eq!(recovered.len(), 1);
    assert!(recovered.get(first_receipt.id()).is_some());
}
#[test]
fn durable_open_prunes_authenticated_pending_store_only_orphans() {
    let temporary = TempDir::new().expect("temporary directory");
    let ledger_root = temporary.path().join("ledger");
    let payload_root = temporary.path().join("payloads");
    let body_root = temporary.path().join("bodies");
    let fixture = Fixture::new();
    let admitted_request = fixture.authenticated_serve_request(2);
    let orphan_request = fixture.authenticated_serve_request(3);
    let (mut payload_store, _) =
        CertifiedServePayloadStoreV1::open(&payload_root, &fixture.context)
            .expect("open payload store");
    let admitted_receipt = payload_store
        .persist_pending(&admitted_request)
        .expect("persist ledger-backed request");
    let _ = payload_store
        .persist_pending(&orphan_request)
        .expect("persist payload-only crash tail");
    drop(payload_store);
    let mut coordinator = fixture.coordinator();
    let authority = coordinator.episode_authority.clone();
    coordinator
        .attach_empty_test_ledger(&ledger_root)
        .expect("attach empty durable ledger");
    assert!(matches!(
        coordinator.admit_certified_serve(&fixture.verified, &admitted_request, admitted_receipt,),
        Ok(AdmissionDecision::Admitted { ordinal: 1, .. })
    ));
    drop(coordinator);
    let body_store =
        V2BodyStore::open(&body_root, fixture.context.clone()).expect("open exact body store");
    let (mut payload_store, payloads) =
        authenticated_payload_cut(&fixture, &payload_root, &body_store, &fixture.keys[0]);
    assert_eq!(payloads.len(), 2);
    let cut = lifecycle_recovery_cut(&fixture, &ledger_root, payloads);
    let restarted =
        LifecycleCoordinator::open_with_authority(authority, &ledger_root, &mut payload_store, cut)
            .expect("ledger-backed request resolves while store-only orphan is pruned");
    assert_eq!(restarted.high_water, 2);
    assert_eq!(restarted.records.len(), 2);
    assert_eq!(restarted.records[&1].state, LifecycleState::Ready);
    assert!(matches!(
        restarted.records[&2].state,
        LifecycleState::Waiting(wait) if wait.source() == WaitSource::ProducerTurn(1)
    ));
    let orphan_subject = certified_serve_key_subject(
        orphan_request.request().subject,
        orphan_request.request_hash(),
    );
    assert!(
        restarted
            .records
            .values()
            .all(|record| record.key.subject() != Some(orphan_subject))
    );
    drop(restarted);
    drop(payload_store);
    let (_, pruned) = CertifiedServePayloadStoreV1::open(&payload_root, &fixture.context)
        .expect("reopen pruned payload store");
    assert_eq!(pruned.len(), 1, "store-only crash tail is removed durably");
}
#[test]
fn durable_open_rejects_a_terminal_store_only_payload() {
    let temporary = TempDir::new().expect("temporary directory");
    let ledger_root = temporary.path().join("ledger");
    let payload_root = temporary.path().join("payloads");
    let body_root = temporary.path().join("bodies");
    let fixture = Fixture::new();
    let request = fixture.authenticated_serve_request(3);
    let (mut payload_store, _) =
        CertifiedServePayloadStoreV1::open(&payload_root, &fixture.context)
            .expect("open payload store");
    let pending = payload_store
        .persist_pending(&request)
        .expect("persist pending orphan");
    let _ = payload_store
        .persist_negative(
            pending.id(),
            CertifiedServePayloadNegativeOutcome::Failed(7),
        )
        .expect("persist impossible terminal orphan");
    drop(payload_store);
    let body_store =
        V2BodyStore::open(&body_root, fixture.context.clone()).expect("open exact body store");
    let (mut payload_store, payloads) =
        authenticated_payload_cut(&fixture, &payload_root, &body_store, &fixture.keys[0]);
    let cut = lifecycle_recovery_cut(&fixture, &ledger_root, payloads);
    let authority = fixture.coordinator().episode_authority;
    assert!(
            LifecycleCoordinator::open_with_authority(
                authority,
                &ledger_root,
                &mut payload_store,
                cut,
            )
            .is_err(),
            "terminal payloads cannot exist without a ledger admission"
        );
    drop(payload_store);
    let (_, recovered) = CertifiedServePayloadStoreV1::open(&payload_root, &fixture.context)
        .expect("failed open preserves terminal evidence");
    assert_eq!(recovered.len(), 1);
}
#[test]
fn durable_open_rejects_a_recovery_cut_from_another_same_context_store() {
    let temporary = TempDir::new().expect("temporary directory");
    let ledger_root = temporary.path().join("ledger");
    let first_root = temporary.path().join("first-payloads");
    let second_root = temporary.path().join("second-payloads");
    let body_root = temporary.path().join("bodies");
    let fixture = Fixture::new();
    let first = fixture.authenticated_serve_request(2);
    let second = fixture.authenticated_serve_request(3);
    let (mut first_store, _) = CertifiedServePayloadStoreV1::open(&first_root, &fixture.context)
        .expect("open first payload store");
    let _ = first_store
        .persist_pending(&first)
        .expect("persist first-store payload");
    drop(first_store);
    let (mut second_store, _) = CertifiedServePayloadStoreV1::open(&second_root, &fixture.context)
        .expect("open second payload store");
    let _ = second_store
        .persist_pending(&second)
        .expect("persist second-store payload");
    let body_store =
        V2BodyStore::open(&body_root, fixture.context.clone()).expect("open exact body store");
    let (first_store, payloads) =
        authenticated_payload_cut(&fixture, &first_root, &body_store, &fixture.keys[0]);
    drop(first_store);
    let cut = lifecycle_recovery_cut(&fixture, &ledger_root, payloads);
    let authority = fixture.coordinator().episode_authority;
    assert!(
        LifecycleCoordinator::open_with_authority(authority, &ledger_root, &mut second_store, cut,)
            .is_err(),
        "same-context stores cannot exchange authenticated recovery cuts"
    );
}
#[test]
fn durable_open_rejects_a_ledger_serve_missing_from_authenticated_storage() {
    let temporary = TempDir::new().expect("temporary directory");
    let ledger_root = temporary.path().join("ledger");
    let admitted_payload_root = temporary.path().join("admitted-payloads");
    let empty_payload_root = temporary.path().join("empty-payloads");
    let body_root = temporary.path().join("bodies");
    let fixture = Fixture::new();
    let request = fixture.authenticated_serve_request(3);
    let (mut payload_store, _) =
        CertifiedServePayloadStoreV1::open(&admitted_payload_root, &fixture.context)
            .expect("open admitted payload store");
    let receipt = payload_store
        .persist_pending(&request)
        .expect("persist admitted request");
    let mut coordinator = fixture.coordinator();
    let authority = coordinator.episode_authority.clone();
    coordinator
        .attach_empty_test_ledger(&ledger_root)
        .expect("attach empty durable ledger");
    assert!(matches!(
        coordinator.admit_certified_serve(&fixture.verified, &request, receipt),
        Ok(AdmissionDecision::Admitted { ordinal: 1, .. })
    ));
    drop(coordinator);
    let body_store =
        V2BodyStore::open(&body_root, fixture.context.clone()).expect("open exact body store");
    let (mut payload_store, payloads) =
        authenticated_payload_cut(&fixture, &empty_payload_root, &body_store, &fixture.keys[0]);
    let cut = lifecycle_recovery_cut(&fixture, &ledger_root, payloads);
    assert!(
            LifecycleCoordinator::open_with_authority(
                authority,
                &ledger_root,
                &mut payload_store,
                cut,
            )
            .is_err()
        );
    let (_, ledger) = super::super::ledger::LifecycleLedgerStoreV1::open(
        &ledger_root,
        lifecycle_context(&fixture.context),
    )
    .expect("failed open leaves ledger readable");
    assert_eq!(ledger.high_water(), 2);
    assert_eq!(ledger.records()[0].terminal(), Some(None));
}
#[test]
fn durable_open_applies_typed_negative_payload_store_ahead_cut() {
    let temporary = TempDir::new().expect("temporary directory");
    let ledger_root = temporary.path().join("ledger");
    let payload_root = temporary.path().join("payloads");
    let body_root = temporary.path().join("bodies");
    let fixture = Fixture::new();
    let request = fixture.authenticated_serve_request(3);
    let (mut payload_store, _) =
        CertifiedServePayloadStoreV1::open(&payload_root, &fixture.context)
            .expect("open payload store");
    let pending = payload_store
        .persist_pending(&request)
        .expect("persist pending request");
    let mut coordinator = fixture.coordinator();
    let authority = coordinator.episode_authority.clone();
    coordinator
        .attach_empty_test_ledger(&ledger_root)
        .expect("attach empty durable ledger");
    assert!(matches!(
        coordinator.admit_certified_serve(&fixture.verified, &request, pending),
        Ok(AdmissionDecision::Admitted { ordinal: 1, .. })
    ));
    let _ = payload_store
        .persist_negative(
            pending.id(),
            CertifiedServePayloadNegativeOutcome::Rejected(19),
        )
        .expect("persist typed negative store-ahead cut");
    drop(payload_store);
    drop(coordinator);
    let body_store =
        V2BodyStore::open(&body_root, fixture.context.clone()).expect("open exact body store");
    let (mut payload_store, payloads) =
        authenticated_payload_cut(&fixture, &payload_root, &body_store, &fixture.keys[0]);
    let cut = lifecycle_recovery_cut(&fixture, &ledger_root, payloads);
    let restarted =
        LifecycleCoordinator::open_with_authority(authority, &ledger_root, &mut payload_store, cut)
            .expect("typed negative store-ahead cut settles atomically");
    assert_eq!(
        restarted.records[&1].state,
        LifecycleState::Terminal(TerminalOutcome::Rejected(19))
    );
    assert_eq!(restarted.records[&2].state, LifecycleState::Ready);
    assert!(matches!(
        restarted.durable_records[&1].payload,
        DurablePayloadReference::CertifiedServeNegative {
            outcome: DurableServeNegativeOutcome::Rejected(19),
            ..
        }
    ));
    let (_, ledger) = super::super::ledger::LifecycleLedgerStoreV1::open(
        &ledger_root,
        lifecycle_context(&fixture.context),
    )
    .expect("reload reconciled negative ledger");
    assert_eq!(
        ledger.records()[0].terminal(),
        Some(Some(TerminalOutcome::Rejected(19)))
    );
}
#[test]
fn durable_open_applies_completed_payload_store_ahead_cut() {
    let temporary = TempDir::new().expect("temporary directory");
    let ledger_root = temporary.path().join("ledger");
    let payload_root = temporary.path().join("payloads");
    let body_root = temporary.path().join("bodies");
    let fixture = Fixture::new();
    let (body, manifest) = fixture.canonical_body_and_manifest();
    let request = fixture.authenticated_serve_request_for(manifest.round, manifest.subject, 3);
    let mut body_store =
        V2BodyStore::open(&body_root, fixture.context.clone()).expect("open exact body store");
    let _ = body_store
        .store(manifest.clone(), body.clone())
        .expect("persist canonical response body");
    let (mut payload_store, _) =
        CertifiedServePayloadStoreV1::open(&payload_root, &fixture.context)
            .expect("open payload store");
    let pending = payload_store
        .persist_pending(&request)
        .expect("persist pending request");
    let mut coordinator = fixture.coordinator();
    let authority = coordinator.episode_authority.clone();
    coordinator
        .attach_empty_test_ledger(&ledger_root)
        .expect("attach empty durable ledger");
    assert!(matches!(
        coordinator.admit_certified_serve(&fixture.verified, &request, pending),
        Ok(AdmissionDecision::Admitted { ordinal: 1, .. })
    ));
    let responder_index = 0;
    let mut response = wire::CertifiedBodyResponse {
        request_hash: request.request_hash(),
        manifest,
        body,
        responder: fixture.context.roster[responder_index].validator.clone(),
        signature: Vec::new(),
    };
    response.signature = Signature::new(
        fixture.keys[responder_index].private_key(),
        &response.signature_preimage(),
    )
    .payload()
    .to_vec();
    let _ = payload_store
        .persist_completed(&request, &response)
        .expect("persist completed response metadata");
    drop(payload_store);
    drop(coordinator);
    let (mut payload_store, payloads) = authenticated_payload_cut(
        &fixture,
        &payload_root,
        &body_store,
        &fixture.keys[responder_index],
    );
    let cut = lifecycle_recovery_cut(&fixture, &ledger_root, payloads);
    let restarted =
        LifecycleCoordinator::open_with_authority(authority, &ledger_root, &mut payload_store, cut)
            .expect("completed store-ahead cut settles atomically");
    let response_digest = digest_from_bytes(HashOf::new(&response).as_ref());
    assert_eq!(
        restarted.records[&1].state,
        LifecycleState::Terminal(TerminalOutcome::Completed(Some(response_digest)))
    );
    assert_eq!(restarted.records[&2].state, LifecycleState::Ready);
    assert!(matches!(
        restarted.durable_records[&1].payload,
        DurablePayloadReference::CertifiedServeCompleted {
            response,
            ..
        } if response == response_digest
    ));
    let (_, ledger) = super::super::ledger::LifecycleLedgerStoreV1::open(
        &ledger_root,
        lifecycle_context(&fixture.context),
    )
    .expect("reload reconciled completion ledger");
    assert_eq!(
        ledger.records()[0].terminal(),
        Some(Some(TerminalOutcome::Completed(Some(response_digest))))
    );
}
#[test]
#[allow(clippy::too_many_lines)]
fn settled_negative_frame_persists_and_reopens_with_the_exact_replay_pair() {
    let temporary = TempDir::new().expect("temporary directory");
    let ledger_root = temporary.path().join("ledger");
    let payload_root = temporary.path().join("payloads");
    let body_root = temporary.path().join("bodies");
    let fixture = Fixture::new();
    let request = fixture.authenticated_serve_request(3);
    let (mut payload_store, _) =
        CertifiedServePayloadStoreV1::open(&payload_root, &fixture.context)
            .expect("open payload store");
    let pending = payload_store
        .persist_pending(&request)
        .expect("persist pending request");
    let mut coordinator = fixture.coordinator();
    let authority = coordinator.episode_authority.clone();
    coordinator
        .attach_empty_test_ledger(&ledger_root)
        .expect("attach empty durable ledger");
    assert!(matches!(
        coordinator.admit_certified_serve(&fixture.verified, &request, pending),
        Ok(AdmissionDecision::Admitted { ordinal: 1, .. })
    ));
    let lease = execute_ready_turn(&mut coordinator);
    let terminal = payload_store
        .persist_negative(
            pending.id(),
            CertifiedServePayloadNegativeOutcome::Rejected(41),
        )
        .expect("persist exact terminal frame");
    assert!(reduce_negative_serve_for_test(
        &mut coordinator,
        lease,
        terminal,
    ));
    assert!(
        coordinator.durable_records[&1]
            .replay_authority
            .same_persisted_family(&coordinator.durable_records[&2].replay_authority)
    );
    assert!(
        coordinator.durable_records[&1]
            .replay_authority
            .certified_serve_frame_hash_is(terminal.payload_hash())
    );
    drop(coordinator);
    drop(payload_store);
    let body_store =
        V2BodyStore::open(&body_root, fixture.context.clone()).expect("open exact body store");
    let (mut payload_store, payloads) =
        authenticated_payload_cut(&fixture, &payload_root, &body_store, &fixture.keys[0]);
    let recovered = payloads
        .get(pending.id())
        .expect("authenticated cut retains terminal request");
    let projection =
        recovered_certified_serve_projection(lifecycle_context(&fixture.context), recovered)
            .expect("project exact terminal recovery frame");
    let (candidate, payload, outcome, replay) = projection.into_parts();
    assert_eq!(candidate.payload, payload);
    assert_eq!(outcome, Some(TerminalOutcome::Rejected(41)));
    assert!(replay.as_ref().is_some_and(|replay| {
        replay.exactly_matches_recovered_candidate(lifecycle_context(&fixture.context), &candidate)
    }));
    let cut = lifecycle_recovery_cut(&fixture, &ledger_root, payloads);
    let restarted =
        LifecycleCoordinator::open_with_authority(authority, &ledger_root, &mut payload_store, cut)
            .expect("steady terminal negative frame reopens exactly");
    assert_eq!(
        restarted.records[&1].state,
        LifecycleState::Terminal(TerminalOutcome::Rejected(41))
    );
    assert!(
        restarted.durable_records[&1]
            .replay_authority
            .same_persisted_family(&restarted.durable_records[&2].replay_authority)
    );
    assert!(
        restarted.durable_records[&2]
            .replay_authority
            .certified_serve_frame_hash_is(terminal.payload_hash())
    );
}
#[test]
#[allow(clippy::too_many_lines)]
fn settled_completed_frame_persists_and_reopens_with_the_exact_replay_pair() {
    let temporary = TempDir::new().expect("temporary directory");
    let ledger_root = temporary.path().join("ledger");
    let payload_root = temporary.path().join("payloads");
    let body_root = temporary.path().join("bodies");
    let fixture = Fixture::new();
    let (body, manifest) = fixture.canonical_body_and_manifest();
    let request = fixture.authenticated_serve_request_for(manifest.round, manifest.subject, 3);
    let mut body_store =
        V2BodyStore::open(&body_root, fixture.context.clone()).expect("open exact body store");
    let _ = body_store
        .store(manifest.clone(), body.clone())
        .expect("persist canonical response body");
    let (mut payload_store, _) =
        CertifiedServePayloadStoreV1::open(&payload_root, &fixture.context)
            .expect("open payload store");
    let pending = payload_store
        .persist_pending(&request)
        .expect("persist pending request");
    let mut coordinator = fixture.coordinator();
    let authority = coordinator.episode_authority.clone();
    coordinator
        .attach_empty_test_ledger(&ledger_root)
        .expect("attach empty durable ledger");
    assert!(matches!(
        coordinator.admit_certified_serve(&fixture.verified, &request, pending),
        Ok(AdmissionDecision::Admitted { ordinal: 1, .. })
    ));
    let lease = execute_ready_turn(&mut coordinator);
    let responder_index = 0;
    let mut response = wire::CertifiedBodyResponse {
        request_hash: request.request_hash(),
        manifest,
        body,
        responder: fixture.context.roster[responder_index].validator.clone(),
        signature: Vec::new(),
    };
    response.signature = Signature::new(
        fixture.keys[responder_index].private_key(),
        &response.signature_preimage(),
    )
    .payload()
    .to_vec();
    let terminal = payload_store
        .persist_completed(&request, &response)
        .expect("persist exact completed frame");
    assert!(reduce_completed_serve_for_test(
        &mut coordinator,
        lease,
        terminal,
    ));
    assert!(
        coordinator.durable_records[&1]
            .replay_authority
            .same_persisted_family(&coordinator.durable_records[&2].replay_authority)
    );
    assert!(
        coordinator.durable_records[&2]
            .replay_authority
            .certified_serve_frame_hash_is(terminal.payload_hash())
    );
    drop(coordinator);
    drop(payload_store);
    let (mut payload_store, payloads) = authenticated_payload_cut(
        &fixture,
        &payload_root,
        &body_store,
        &fixture.keys[responder_index],
    );
    let recovered = payloads
        .get(pending.id())
        .expect("authenticated cut retains completed request");
    let projection =
        recovered_certified_serve_projection(lifecycle_context(&fixture.context), recovered)
            .expect("project exact completed recovery frame");
    let (candidate, payload, outcome, replay) = projection.into_parts();
    let response_digest = digest_from_bytes(HashOf::new(&response).as_ref());
    assert_eq!(candidate.payload, payload);
    assert_eq!(
        outcome,
        Some(TerminalOutcome::Completed(Some(response_digest)))
    );
    assert!(replay.as_ref().is_some_and(|replay| {
        replay.exactly_matches_recovered_candidate(lifecycle_context(&fixture.context), &candidate)
    }));
    let cut = lifecycle_recovery_cut(&fixture, &ledger_root, payloads);
    let restarted =
        LifecycleCoordinator::open_with_authority(authority, &ledger_root, &mut payload_store, cut)
            .expect("steady completed frame reopens exactly");
    assert_eq!(
        restarted.records[&1].state,
        LifecycleState::Terminal(TerminalOutcome::Completed(Some(response_digest)))
    );
    assert!(
        restarted.durable_records[&1]
            .replay_authority
            .same_persisted_family(&restarted.durable_records[&2].replay_authority)
    );
    assert!(
        restarted.durable_records[&1]
            .replay_authority
            .certified_serve_frame_hash_is(terminal.payload_hash())
    );
}
#[allow(clippy::too_many_lines)]
fn accepted_effects(fixture: &Fixture) -> Vec<ExpectedProjection> {
    let mut unsigned_proposal = fixture.proposal.clone();
    unsigned_proposal.signature.clear();
    let mut unsigned_prepare_vote = fixture.prepare_vote.clone();
    unsigned_prepare_vote.signature.clear();
    let mut unsigned_commit_vote = fixture.commit_vote.clone();
    unsigned_commit_vote.signature.clear();
    let mut unsigned_timeout_vote = fixture.timeout_vote.clone();
    unsigned_timeout_vote.signature.clear();
    let certified_sources = fixture
        .context
        .roster
        .iter()
        .map(|entry| entry.validator.clone())
        .collect::<Vec<_>>();
    let entered_tag = EventTag::new(
        fixture.context.height,
        fixture.round.view + 1,
        Generation::new(0),
    );
    let proposal_conflict = AdapterEquivocationEvidence::proposal_for_test(
        fixture.proposal.clone(),
        fixture.proposal_for(0x51, 0x51),
    );
    let (first_vote, second_vote) = vote_conflict(fixture);
    let vote_conflict = AdapterEquivocationEvidence::vote_for_test(first_vote, second_vote);
    let timeout_conflict = AdapterEquivocationEvidence::timeout_vote_for_test(
        fixture.timeout_vote.clone(),
        wire::TimeoutVote {
            highest_prepare_qc: Some(fixture.prepare_qc.clone()),
            signature: vec![0x54],
            ..fixture.timeout_vote.clone()
        },
    );
    vec![
        (
            AdapterEffect::Sign {
                tag: fixture.tag,
                request: SignRequest::Proposal(unsigned_proposal),
            },
            LifecycleWorkClass::SignProposal,
            LifecyclePhase::Proposal,
            LifecycleStageKind::SignProposal,
        ),
        (
            AdapterEffect::Sign {
                tag: fixture.tag,
                request: SignRequest::Vote(unsigned_prepare_vote),
            },
            LifecycleWorkClass::SignVote,
            LifecyclePhase::Prepare,
            LifecycleStageKind::SignPrepareVote,
        ),
        (
            AdapterEffect::Sign {
                tag: fixture.tag,
                request: SignRequest::Vote(unsigned_commit_vote),
            },
            LifecycleWorkClass::SignVote,
            LifecyclePhase::Commit,
            LifecycleStageKind::SignCommitVote,
        ),
        (
            AdapterEffect::Sign {
                tag: fixture.tag,
                request: SignRequest::TimeoutVote(unsigned_timeout_vote),
            },
            LifecycleWorkClass::SignTimeout,
            LifecyclePhase::Timeout,
            LifecycleStageKind::SignTimeoutVote,
        ),
        (
            AdapterEffect::FetchBody {
                tag: fixture.tag,
                round: fixture.round,
                subject: fixture.subject,
                manifest: Some(fixture.manifest.clone()),
                certified_sources: Vec::new(),
                certificate: None,
            },
            LifecycleWorkClass::Fetch,
            LifecyclePhase::Fetch,
            LifecycleStageKind::FetchBody,
        ),
        (
            AdapterEffect::FetchBody {
                tag: fixture.tag,
                round: fixture.round,
                subject: fixture.subject,
                manifest: None,
                certified_sources,
                certificate: Some(fixture.prepare_qc.clone()),
            },
            LifecycleWorkClass::Fetch,
            LifecyclePhase::Fetch,
            LifecycleStageKind::FetchBody,
        ),
        (
            AdapterEffect::Apply {
                tag: fixture.tag,
                subject: fixture.subject,
                certificate: fixture.commit_qc.clone(),
            },
            LifecycleWorkClass::Apply,
            LifecyclePhase::Apply,
            LifecycleStageKind::ApplyDecision,
        ),
        (
            AdapterEffect::Broadcast(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::Proposal(fixture.proposal.clone()),
            )),
            LifecycleWorkClass::Broadcast,
            LifecyclePhase::BroadcastProposal,
            LifecycleStageKind::BroadcastProposal,
        ),
        (
            AdapterEffect::Broadcast(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::Vote(fixture.prepare_vote.clone()),
            )),
            LifecycleWorkClass::Broadcast,
            LifecyclePhase::BroadcastPrepareVote,
            LifecycleStageKind::BroadcastPrepareVote,
        ),
        (
            AdapterEffect::Broadcast(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::Vote(fixture.commit_vote.clone()),
            )),
            LifecycleWorkClass::Broadcast,
            LifecyclePhase::BroadcastCommitVote,
            LifecycleStageKind::BroadcastCommitVote,
        ),
        (
            AdapterEffect::Broadcast(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::QuorumCertificate(fixture.prepare_qc.clone()),
            )),
            LifecycleWorkClass::Broadcast,
            LifecyclePhase::BroadcastPrepareQc,
            LifecycleStageKind::BroadcastPrepareQc,
        ),
        (
            AdapterEffect::Broadcast(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::QuorumCertificate(fixture.commit_qc.clone()),
            )),
            LifecycleWorkClass::Broadcast,
            LifecyclePhase::BroadcastCommitQc,
            LifecycleStageKind::BroadcastCommitQc,
        ),
        (
            AdapterEffect::Broadcast(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::TimeoutVote(fixture.timeout_vote.clone()),
            )),
            LifecycleWorkClass::Broadcast,
            LifecyclePhase::BroadcastTimeoutVote,
            LifecycleStageKind::BroadcastTimeoutVote,
        ),
        (
            AdapterEffect::Broadcast(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::TimeoutCertificate(
                    fixture.timeout_certificate.clone(),
                ),
            )),
            LifecycleWorkClass::Broadcast,
            LifecyclePhase::BroadcastTc,
            LifecycleStageKind::BroadcastTc,
        ),
        (
            AdapterEffect::EnterView {
                tag: entered_tag,
                certificate: fixture.timeout_certificate.clone(),
                protected_lock: Some(fixture.prepare_qc.clone()),
            },
            LifecycleWorkClass::EnterView,
            LifecyclePhase::EnterView,
            LifecycleStageKind::EnterView,
        ),
        (
            AdapterEffect::ReportEquivocation {
                evidence: proposal_conflict,
            },
            LifecycleWorkClass::EquivocationReport,
            LifecyclePhase::DiagnosticProposalEquivocation,
            LifecycleStageKind::ReportProposalEquivocation,
        ),
        (
            AdapterEffect::ReportEquivocation {
                evidence: vote_conflict,
            },
            LifecycleWorkClass::EquivocationReport,
            LifecyclePhase::DiagnosticVoteEquivocation,
            LifecycleStageKind::ReportVoteEquivocation,
        ),
        (
            AdapterEffect::ReportEquivocation {
                evidence: timeout_conflict,
            },
            LifecycleWorkClass::EquivocationReport,
            LifecyclePhase::DiagnosticTimeoutEquivocation,
            LifecycleStageKind::ReportTimeoutEquivocation,
        ),
        (
            AdapterEffect::ReportInvalidCertifiedBody {
                subject: fixture.subject,
                certificate: fixture.prepare_qc.clone(),
            },
            LifecycleWorkClass::InvalidBodyReport,
            LifecyclePhase::DiagnosticInvalidBody,
            LifecycleStageKind::ReportInvalidBody,
        ),
    ]
}
#[test]
fn every_adapter_effect_class_and_specialized_phase_projects_ready_one_slot_work() {
    let fixture = Fixture::new();
    let cases = accepted_effects(&fixture);
    assert_eq!(cases.len(), 19);
    for (ordinal, (effect, work_class, phase, stage_kind)) in (1_u128..).zip(cases) {
        let ownership = bound_ownership(&effect, fixture.tag, ordinal);
        let projected = candidate(&fixture, &effect, &ownership);
        assert_candidate_shape(
            &projected, &effect, &ownership, work_class, phase, stage_kind,
        );
        if phase == LifecyclePhase::Timeout {
            assert_eq!(projected.key.proposal_round(), None);
            assert_eq!(projected.key.subject(), None);
            assert_eq!(projected.key.execution_commitment(), None);
        }
        let mut coordinator = fixture.coordinator();
        if matches!(
            work_class,
            LifecycleWorkClass::Broadcast | LifecycleWorkClass::EquivocationReport
        ) {
            let prepared = prepare_direct_signed(&fixture, &coordinator, &effect, &ownership);
            let mut registry = LifecycleWorkRegistryHolder::empty();
            assert!(matches!(
                coordinator.admit_prepared_lifecycle(&mut registry, prepared),
                super::super::concrete_admission::AdapterEffectAdmissionTransaction::Admitted(
                    AdmissionDecision::Admitted { ordinal: 1, .. }
                )
            ));
        } else {
            let pending = ownership
                .exact_pending_adapter_effect_binding(&effect)
                .expect("mint exact unsupported pending owner");
            assert!(matches!(
                coordinator.prepare_direct_signed_lifecycle_admission(
                    &fixture.verified,
                    effect,
                    pending,
                ),
                Err(super::super::work_registry::PreparedLifecycleAdmissionErrorV1::Binding(_))
            ));
            assert!(coordinator.records.is_empty());
        }
    }
}
#[test]
fn timeout_certificate_retransmit_key_binds_the_complete_valid_envelope() {
    let fixture = Fixture::new();
    let first_certificate = fixture.authenticated_timeout_certificate(vec![0, 1, 2]);
    let revised_certificate = fixture.authenticated_timeout_certificate(vec![0, 1, 3]);
    let first_effect = AdapterEffect::Broadcast(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::TimeoutCertificate(first_certificate),
    ));
    let revised_effect = AdapterEffect::Broadcast(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::TimeoutCertificate(revised_certificate),
    ));
    let first_owner = bound_ownership(&first_effect, fixture.tag, 20);
    let revised_owner = bound_ownership(&revised_effect, fixture.tag, 21);
    let first = candidate(&fixture, &first_effect, &first_owner);
    let revised = candidate(&fixture, &revised_effect, &revised_owner);

    assert_eq!(first.key.round(), revised.key.round());
    assert_eq!(first.key.proposal_round(), revised.key.proposal_round());
    assert_eq!(
        first.key.execution_commitment(),
        revised.key.execution_commitment()
    );
    assert_ne!(first.key.subject(), revised.key.subject());
    assert_ne!(first.key, revised.key);
    assert_ne!(
        first.physical_geometry.initial[0].digest(),
        revised.physical_geometry.initial[0].digest()
    );
}
#[test]
fn certified_store_and_validate_inherit_authority_but_require_receipt_bound_staging() {
    let fixture = Fixture::new();
    let fetch = AdapterEffect::FetchBody {
        tag: fixture.tag,
        round: fixture.round,
        subject: fixture.subject,
        manifest: None,
        certified_sources: fixture
            .context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect(),
        certificate: Some(fixture.prepare_qc.clone()),
    };
    let fetch_owner = bound_ownership(&fetch, fixture.tag, 20);
    let store = AdapterEffect::StoreBody {
        tag: fixture.tag,
        round: fixture.round,
        subject: fixture.subject,
    };
    let store_owner = fetch_owner
        .rebind_as_inherited_adapter_effect(&store)
        .expect("Fetch authorizes exact Store successor");
    let validate = AdapterEffect::ValidateBody {
        tag: fixture.tag,
        round: fixture.round,
        subject: fixture.subject,
    };
    let validate_owner = store_owner
        .rebind_as_inherited_adapter_effect(&validate)
        .expect("Store authorizes exact Validate successor");
    let store_candidate = candidate(&fixture, &store, &store_owner);
    let validate_candidate = candidate(&fixture, &validate, &validate_owner);
    assert_candidate_shape(
        &store_candidate,
        &store,
        &store_owner,
        LifecycleWorkClass::Store,
        LifecyclePhase::Store,
        LifecycleStageKind::StoreBody,
    );
    assert_candidate_shape(
        &validate_candidate,
        &validate,
        &validate_owner,
        LifecycleWorkClass::Validate,
        LifecyclePhase::Validate,
        LifecycleStageKind::ValidateBody,
    );
    let expected_commitment = Some(execution_commitment(
        fixture.prepare_qc.execution_commitment,
    ));
    assert_eq!(
        store_candidate.key.execution_commitment(),
        expected_commitment
    );
    assert_eq!(
        validate_candidate.key.execution_commitment(),
        expected_commitment
    );
    assert_eq!(store_candidate.causal_root, validate_candidate.causal_root);
    let coordinator = fixture.coordinator();
    for (effect, ownership) in [(&store, &store_owner), (&validate, &validate_owner)] {
        let pending = ownership
            .exact_pending_adapter_effect_binding(effect)
            .expect("mint exact receipt-bound pending owner");
        assert!(matches!(
            coordinator.prepare_direct_signed_lifecycle_admission(
                &fixture.verified,
                effect.clone(),
                pending,
            ),
            Err(super::super::work_registry::PreparedLifecycleAdmissionErrorV1::Binding(_))
        ));
    }
    assert!(coordinator.records.is_empty());
}
#[test]
fn future_view_commit_decision_retains_the_lagging_reducer_owner_through_application() {
    let fixture = Fixture::new();
    let future_round = wire::ConsensusRound {
        view: fixture
            .round
            .view
            .checked_add(3)
            .expect("small future Decision view"),
        ..fixture.round
    };
    let future_commit = wire::QuorumCertificate {
        round: future_round,
        proposal_round: future_round,
        ..fixture.commit_qc.clone()
    };
    let fetch = AdapterEffect::FetchBody {
        tag: fixture.tag,
        round: future_round,
        subject: fixture.subject,
        manifest: None,
        certified_sources: fixture
            .context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect(),
        certificate: Some(future_commit.clone()),
    };
    let fetch_owner = bound_ownership(&fetch, fixture.tag, 21);
    let store = AdapterEffect::StoreBody {
        tag: fixture.tag,
        round: future_round,
        subject: fixture.subject,
    };
    let store_owner = fetch_owner
        .rebind_as_inherited_adapter_effect(&store)
        .expect("future Decision Fetch authorizes its Store successor");
    let validate = AdapterEffect::ValidateBody {
        tag: fixture.tag,
        round: future_round,
        subject: fixture.subject,
    };
    let validate_owner = store_owner
        .rebind_as_inherited_adapter_effect(&validate)
        .expect("future Decision Store authorizes its Validate successor");
    let apply = AdapterEffect::Apply {
        tag: fixture.tag,
        subject: fixture.subject,
        certificate: future_commit.clone(),
    };
    let apply_owner = validate_owner
        .rebind_as_inherited_adapter_effect(&apply)
        .expect("future Decision Validate authorizes its Apply successor");

    for (effect, ownership) in [
        (&fetch, &fetch_owner),
        (&store, &store_owner),
        (&validate, &validate_owner),
        (&apply, &apply_owner),
    ] {
        let projected = candidate(&fixture, effect, ownership);
        assert_eq!(projected.key.round().height(), future_round.height);
        assert_eq!(projected.key.round().view(), future_round.view);
    }

    let future_prepare = AdapterEffect::FetchBody {
        tag: fixture.tag,
        round: future_round,
        subject: fixture.subject,
        manifest: None,
        certified_sources: fixture
            .context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect(),
        certificate: Some(wire::QuorumCertificate {
            phase: wire::GlobalPhase::Prepare,
            ..future_commit
        }),
    };
    let prepare_owner = bound_ownership(&future_prepare, fixture.tag, 22);
    let prepare_store = AdapterEffect::StoreBody {
        tag: fixture.tag,
        round: future_round,
        subject: fixture.subject,
    };
    let prepare_store_owner = prepare_owner
        .rebind_as_inherited_adapter_effect(&prepare_store)
        .expect("future Prepare Fetch retains its strict Store successor");
    let prepare_validate = AdapterEffect::ValidateBody {
        tag: fixture.tag,
        round: future_round,
        subject: fixture.subject,
    };
    let prepare_validate_owner = prepare_store_owner
        .rebind_as_inherited_adapter_effect(&prepare_validate)
        .expect("future Prepare Store retains its strict Validate successor");
    for (effect, ownership) in [
        (&future_prepare, &prepare_owner),
        (&prepare_store, &prepare_store_owner),
        (&prepare_validate, &prepare_validate_owner),
    ] {
        let pending = ownership
            .exact_pending_adapter_effect_binding(effect)
            .expect("mint exact future Prepare pending owner");
        assert!(
            matches!(
                authority_free_admission_projection(
                    lifecycle_context(&fixture.context),
                    &fixture.verified,
                    effect,
                    &pending,
                ),
                Err(AdapterEffectAdmissionError::InvalidCarrier)
            ),
            "ordinary Prepare authority must not bypass reducer-view ordering"
        );
    }
}
#[test]
fn recovery_cut_consumes_exact_terminal_validate_body_outcome() {
    let temporary = TempDir::new().expect("temporary lifecycle recovery roots");
    let fixture = Fixture::new();
    let fetch = AdapterEffect::FetchBody {
        tag: fixture.tag,
        round: fixture.round,
        subject: fixture.subject,
        manifest: None,
        certified_sources: fixture
            .context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect(),
        certificate: Some(fixture.prepare_qc.clone()),
    };
    let fetch_owner = bound_ownership(&fetch, fixture.tag, 20);
    let store = AdapterEffect::StoreBody {
        tag: fixture.tag,
        round: fixture.round,
        subject: fixture.subject,
    };
    let store_owner = fetch_owner
        .rebind_as_inherited_adapter_effect(&store)
        .expect("Fetch authorizes exact Store successor");
    let validate = AdapterEffect::ValidateBody {
        tag: fixture.tag,
        round: fixture.round,
        subject: fixture.subject,
    };
    let validate_owner = store_owner
        .rebind_as_inherited_adapter_effect(&validate)
        .expect("Store authorizes exact Validate successor");
    let durable = crate::sumeragi::v2_body_store::DurableBodyReceipt::for_test(
        fixture.context.id(),
        fixture.round,
        fixture.subject,
        HashOf::new(&fixture.manifest),
    );
    let validate_candidate = certified_validate_candidate(
        &fixture,
        &fetch,
        &store,
        &validate,
        &validate_owner,
        &durable,
    );
    let outcome = crate::sumeragi::v2_body_store::DurableBodyValidationOutcome::validated_for_test(
        crate::sumeragi::v2_body_store::ValidatedBodyReceipt::for_test_with_commitment(
            durable,
            fixture.prepare_qc.execution_commitment,
        ),
    );
    let body_store = V2BodyStore::open(temporary.path().join("body"), fixture.context.clone())
        .expect("open exact-context body store");
    let (mut payload_store, payloads) = authenticated_payload_cut(
        &fixture,
        &temporary.path().join("payload"),
        &body_store,
        &fixture.keys[0],
    );
    let owner = OwnerId::new(validate_candidate.causal_root, 1);
    let record = super::super::ledger::LifecycleLedgerRecordV1::new(
        validate_candidate.key,
        owner,
        1,
        validate_candidate.work_class,
        validate_candidate.stage,
        Some(TerminalOutcome::Advanced),
        validate_candidate.reconstruction_source,
        validate_candidate.payload,
        validate_candidate.replay_authority.clone(),
        super::super::schema::DurableContinuation::AdvancedNoSuccessor,
    )
    .expect("construct terminal Validate ledger row");
    let ledger = super::super::ledger::LifecycleLedgerV1::new(
        lifecycle_context(&fixture.context),
        1,
        vec![record],
        std::collections::BTreeMap::new(),
    )
    .expect("construct exact no-child terminal ledger");
    let recovery = AuthenticatedLifecycleRecoveryCut::from_authenticated_parts(
        ledger.clone(),
        [],
        [(validate_candidate.clone(), outcome)],
        payloads,
    )
    .expect("exact body outcome seals the terminal Validate recovery identity");
    let ledger_root = temporary.path().join("ledger");
    let (ledger_store, empty) = super::super::ledger::LifecycleLedgerStoreV1::open(
        &ledger_root,
        lifecycle_context(&fixture.context),
    )
    .expect("open empty lifecycle ledger");
    assert!(empty.records().is_empty());
    ledger_store
        .persist(&ledger)
        .expect("persist exact no-child terminal ledger");
    drop(ledger_store);
    let authority = fixture.coordinator().episode_authority;
    let reopened = LifecycleCoordinator::open_with_authority(
        authority,
        &ledger_root,
        &mut payload_store,
        recovery,
    )
    .expect("open terminal Validate with exact no-child recovery proof");
    assert_eq!(
        reopened.records[&1].state,
        LifecycleState::Terminal(TerminalOutcome::Advanced)
    );
    assert_eq!(
        reopened.durable_records[&1].continuation,
        super::super::schema::DurableContinuation::AdvancedNoSuccessor
    );
    let rejected = crate::sumeragi::v2_body_store::DurableBodyValidationOutcome::rejected_for_test(
        crate::sumeragi::v2_body_store::DurableBodyReceipt::for_test(
            fixture.context.id(),
            fixture.round,
            fixture.subject,
            HashOf::new(&fixture.manifest),
        ),
    );
    let (_payload_store, payloads) = authenticated_payload_cut(
        &fixture,
        &temporary.path().join("payload-foreign"),
        &body_store,
        &fixture.keys[0],
    );
    assert!(
        AuthenticatedLifecycleRecoveryCut::from_authenticated_parts(
            ledger,
            [validate_candidate.clone()],
            [(validate_candidate, rejected)],
            payloads,
        )
        .is_none(),
        "one semantic key cannot be both live recovery work and a no-child tombstone proof"
    );
}
#[test]
fn coordinator_method_enforces_zero_to_one_retry_and_foreign_owner_rejection() {
    let fixture = Fixture::new();
    let effect = AdapterEffect::Broadcast(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::Proposal(fixture.proposal.clone()),
    ));
    let exact = bound_ownership(&effect, fixture.tag, 30);
    let foreign_tag = EventTag::new(
        fixture.tag.height(),
        fixture.tag.view(),
        Generation::new(fixture.tag.generation().get() + 1),
    );
    let foreign = bind_adapter_effect_batch_ownership(
        core::slice::from_ref(&effect),
        vec![
            RuntimeEffectOwnership::fresh_for_test_with_semantic_identity(
                foreign_tag,
                31,
                b"foreign projection admission owner",
            ),
        ],
    )
    .expect("bind semantically foreign projection owner")
    .pop()
    .expect("one foreign projection owner");
    let mut coordinator = fixture.coordinator();
    let mut registry = LifecycleWorkRegistryHolder::empty();
    let prepared = prepare_direct_signed(&fixture, &coordinator, &effect, &exact);
    assert!(matches!(
        coordinator.admit_prepared_lifecycle(&mut registry, prepared),
        super::super::concrete_admission::AdapterEffectAdmissionTransaction::Admitted(
            AdmissionDecision::Admitted { ordinal: 1, .. }
        )
    ));
    assert_eq!(coordinator.records.len(), 1, "0 -> 1 owner admission");
    let prepared = prepare_direct_signed(&fixture, &coordinator, &effect, &exact);
    assert!(matches!(
        coordinator.admit_prepared_lifecycle(&mut registry, prepared),
        super::super::concrete_admission::AdapterEffectAdmissionTransaction::Returned {
            decision: AdmissionDecision::Retry {
                ordinal: 1,
                action: RetryAction::RefanoutEnvelope,
                ..
            },
            ..
        }
    ));
    assert_eq!(coordinator.records.len(), 1, "same-owner retry is 1 -> 1");
    let prepared = prepare_direct_signed(&fixture, &coordinator, &effect, &foreign);
    assert!(matches!(
        coordinator.admit_prepared_lifecycle(&mut registry, prepared),
        super::super::concrete_admission::AdapterEffectAdmissionTransaction::Returned {
            decision: AdmissionDecision::Rejected(AdmissionRejection::ForeignOwner),
            ..
        }
    ));
    assert_eq!(coordinator.records.len(), 1);
}
#[test]
fn mismatched_and_foreign_context_effects_fail_before_admission() {
    let fixture = Fixture::new();
    let effect = AdapterEffect::Broadcast(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::Vote(fixture.prepare_vote.clone()),
    ));
    let other_effect = AdapterEffect::Broadcast(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::Vote(fixture.commit_vote.clone()),
    ));
    let mismatched = bound_ownership(&other_effect, fixture.tag, 40);
    let coordinator = fixture.coordinator();
    assert!(
        mismatched
            .exact_pending_adapter_effect_binding(&effect)
            .is_err()
    );
    assert!(coordinator.records.is_empty());
    let ownership = bound_ownership(&effect, fixture.tag, 41);
    let pending = ownership
        .exact_pending_adapter_effect_binding(&effect)
        .expect("mint exact foreign-context pending owner");
    let foreign_context = LifecycleContext::new(LifecycleDigest::new([0xFF; 32]), 1);
    let foreign = LifecycleCoordinator::new(
        foreign_context,
        0,
        CapacityGeometry::new(CapacityClass::ALL.map(|class| (class, 64))),
    );
    assert!(matches!(
        foreign.prepare_direct_signed_lifecycle_admission(&fixture.verified, effect, pending),
        Err(
            super::super::work_registry::PreparedLifecycleAdmissionErrorV1::Projection {
                failure: AdapterEffectAdmissionError::ForeignContext,
                ..
            }
        )
    ));
    assert!(foreign.records.is_empty());
}
#[test]
fn broadcast_vote_and_qc_have_collision_free_specialized_keys() {
    let fixture = Fixture::new();
    let vote = AdapterEffect::Broadcast(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::Vote(fixture.prepare_vote.clone()),
    ));
    let qc = AdapterEffect::Broadcast(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::QuorumCertificate(fixture.prepare_qc.clone()),
    ));
    let vote_owner = bound_ownership(&vote, fixture.tag, 50);
    let qc_owner = bound_ownership(&qc, fixture.tag, 51);
    let vote_candidate = candidate(&fixture, &vote, &vote_owner);
    let qc_candidate = candidate(&fixture, &qc, &qc_owner);
    assert_eq!(vote_candidate.key.subject(), qc_candidate.key.subject());
    assert_eq!(
        vote_candidate.key.execution_commitment(),
        qc_candidate.key.execution_commitment()
    );
    assert_eq!(
        vote_candidate.key.phase(),
        LifecyclePhase::BroadcastPrepareVote
    );
    assert_eq!(qc_candidate.key.phase(), LifecyclePhase::BroadcastPrepareQc);
    assert_ne!(vote_candidate.key, qc_candidate.key);
    assert_ne!(
        vote_candidate.physical_geometry.initial[0].digest(),
        qc_candidate.physical_geometry.initial[0].digest()
    );
}
#[test]
fn all_auxiliary_broadcast_payloads_are_explicitly_rejected() {
    let fixture = Fixture::new();
    let certified_request = wire::CertifiedBodyRequest {
        round: fixture.round,
        subject: fixture.subject,
        certificate: fixture.prepare_qc.clone(),
        requester: fixture.context.roster[0].validator.clone(),
        signature: vec![0x61],
    };
    let commit_request = wire::CommitCertificateRequest {
        protocol_version: wire::PROTOCOL_VERSION,
        network_id: fixture.context.network_id,
        context_id: fixture.context.id(),
        height: fixture.context.height,
        requester: fixture.context.roster[0].validator.clone(),
        signature: vec![0x62],
    };
    let payloads = vec![
        wire::ConsensusMessageV2Payload::PayloadChunk(wire::PayloadChunk {
            manifest_hash: HashOf::new(&fixture.manifest),
            index: 0,
            bytes: fixture.encoded_chunks[0].clone(),
            sender: 0,
            signature: vec![0x63],
        }),
        wire::ConsensusMessageV2Payload::CertifiedBodyRequest(certified_request.clone()),
        wire::ConsensusMessageV2Payload::CertifiedBodyResponse(wire::CertifiedBodyResponse {
            request_hash: HashOf::new(&certified_request),
            manifest: fixture.manifest.clone(),
            body: fixture.body.clone(),
            responder: fixture.context.roster[0].validator.clone(),
            signature: vec![0x64],
        }),
        wire::ConsensusMessageV2Payload::CommitCertificateRequest(commit_request.clone()),
        wire::ConsensusMessageV2Payload::CommitCertificateResponse(
            wire::CommitCertificateResponse {
                request_hash: HashOf::new(&commit_request),
                certificate: fixture.commit_qc.clone(),
                responder: fixture.context.roster[0].validator.clone(),
                signature: vec![0x65],
            },
        ),
        wire::ConsensusMessageV2Payload::GlobalBeaconPartialSignature(
            wire::GlobalBeaconPartialSignature {
                round: fixture.round,
                partial: iroha_data_model::consensus::GlobalThresholdBeaconPartialSignatureV1 {
                    session_id: [0x68; 32],
                    signer_index: 1,
                    signature_share: [0x69; 48],
                    proof:
                        iroha_data_model::consensus::GlobalThresholdBeaconPartialSignatureProofV1 {
                            x: [0x6A; 96],
                            y: [0x6B; 48],
                            z_s: [0x6C; 32],
                            z_r: [0x6D; 32],
                            z_u: [0x6E; 32],
                        },
                },
            },
        ),
    ];
    assert_eq!(payloads.len(), 7);
    for (ordinal, payload) in (60_u128..).zip(payloads) {
        let effect = AdapterEffect::Broadcast(wire::ConsensusMessageV2::new(payload));
        let ownership = bound_ownership(&effect, fixture.tag, ordinal);
        let coordinator = fixture.coordinator();
        let pending = ownership
            .exact_pending_adapter_effect_binding(&effect)
            .expect("mint exact auxiliary broadcast pending owner");
        assert!(matches!(
            coordinator.prepare_direct_signed_lifecycle_admission(
                &fixture.verified,
                effect,
                pending,
            ),
            Err(super::super::work_registry::PreparedLifecycleAdmissionErrorV1::Binding(_))
        ));
        assert!(coordinator.records.is_empty());
    }
}
#[test]
fn enter_view_key_and_physical_identity_retain_protected_commitment() {
    let fixture = Fixture::new();
    let entered_tag = EventTag::new(
        fixture.context.height,
        fixture.round.view + 1,
        Generation::new(0),
    );
    let second_lock = wire::QuorumCertificate {
        execution_commitment: execution_commitment_for(0x71),
        aggregate_signature: vec![0x71],
        ..fixture.prepare_qc.clone()
    };
    let first = AdapterEffect::EnterView {
        tag: entered_tag,
        certificate: fixture.timeout_certificate.clone(),
        protected_lock: Some(fixture.prepare_qc.clone()),
    };
    let second = AdapterEffect::EnterView {
        tag: entered_tag,
        certificate: fixture.timeout_certificate.clone(),
        protected_lock: Some(second_lock),
    };
    let first_owner = bound_ownership(&first, fixture.tag, 70);
    let second_owner = bound_ownership(&second, fixture.tag, 71);
    let first_candidate = candidate(&fixture, &first, &first_owner);
    let second_candidate = candidate(&fixture, &second, &second_owner);
    assert_eq!(
        first_candidate.key.subject(),
        second_candidate.key.subject()
    );
    assert_ne!(
        first_candidate.key.execution_commitment(),
        second_candidate.key.execution_commitment()
    );
    assert_ne!(first_candidate.key, second_candidate.key);
    assert_ne!(
        first_candidate.physical_geometry.initial[0].digest(),
        second_candidate.physical_geometry.initial[0].digest()
    );
}
#[test]
fn diagnostic_logical_identity_normalizes_order_and_signatures_but_physical_does_not() {
    let fixture = Fixture::new();
    let (first, second) = vote_conflict(&fixture);
    let mut resigned = first.clone();
    resigned.signature = vec![0x7F];
    let forward = AdapterEffect::ReportEquivocation {
        evidence: AdapterEquivocationEvidence::vote_for_test(first.clone(), second.clone()),
    };
    let reversed = AdapterEffect::ReportEquivocation {
        evidence: AdapterEquivocationEvidence::vote_for_test(second.clone(), first.clone()),
    };
    let re_signed = AdapterEffect::ReportEquivocation {
        evidence: AdapterEquivocationEvidence::vote_for_test(resigned, second),
    };
    let forward_owner = bound_ownership(&forward, fixture.tag, 80);
    let reversed_owner = bound_ownership(&reversed, fixture.tag, 81);
    let re_signed_owner = bound_ownership(&re_signed, fixture.tag, 82);
    let forward_candidate = candidate(&fixture, &forward, &forward_owner);
    let reversed_candidate = candidate(&fixture, &reversed, &reversed_owner);
    let re_signed_candidate = candidate(&fixture, &re_signed, &re_signed_owner);
    assert_eq!(forward_candidate.key, reversed_candidate.key);
    assert_eq!(forward_candidate.key, re_signed_candidate.key);
    let forward_digest = forward_candidate.physical_geometry.initial[0].digest();
    let reversed_digest = reversed_candidate.physical_geometry.initial[0].digest();
    let re_signed_digest = re_signed_candidate.physical_geometry.initial[0].digest();
    assert_ne!(forward_digest, reversed_digest);
    assert_ne!(forward_digest, re_signed_digest);
    assert_ne!(reversed_digest, re_signed_digest);
}
#[test]
fn bound_but_drifted_carriers_fail_closed_without_records() {
    let fixture = Fixture::new();
    let mut signed_proposal = fixture.proposal.clone();
    signed_proposal.signature = vec![0x91];
    let pre_signed = AdapterEffect::Sign {
        tag: fixture.tag,
        request: SignRequest::Proposal(signed_proposal),
    };
    let invalid_body = AdapterEffect::ReportInvalidCertifiedBody {
        subject: fixture.proposal_for(0x92, 0x92).subject,
        certificate: fixture.prepare_qc.clone(),
    };
    let foreign_protocol = AdapterEffect::Broadcast(wire::ConsensusMessageV2 {
        protocol_version: wire::PROTOCOL_VERSION + 1,
        payload: wire::ConsensusMessageV2Payload::Vote(fixture.prepare_vote.clone()),
    });
    for (ordinal, effect) in (90_u128..).zip([pre_signed, invalid_body]) {
        let ownership = bound_ownership(&effect, fixture.tag, ordinal);
        let coordinator = fixture.coordinator();
        let pending = ownership
            .exact_pending_adapter_effect_binding(&effect)
            .expect("mint exact unsupported drifted pending owner");
        assert!(matches!(
            coordinator.prepare_direct_signed_lifecycle_admission(
                &fixture.verified,
                effect,
                pending,
            ),
            Err(super::super::work_registry::PreparedLifecycleAdmissionErrorV1::Binding(_))
        ));
        assert!(coordinator.records.is_empty());
    }
    let ownership = bound_ownership(&foreign_protocol, fixture.tag, 92);
    let coordinator = fixture.coordinator();
    let pending = ownership
        .exact_pending_adapter_effect_binding(&foreign_protocol)
        .expect("mint exact foreign-protocol pending owner");
    assert!(matches!(
        coordinator.prepare_direct_signed_lifecycle_admission(
            &fixture.verified,
            foreign_protocol,
            pending,
        ),
        Err(super::super::work_registry::PreparedLifecycleAdmissionErrorV1::Binding(_))
    ));
    assert!(coordinator.records.is_empty());
}
