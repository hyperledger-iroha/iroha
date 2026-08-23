// Focused Certified-Serve registry-to-scheduler ownership regressions.

use super::{
    CertifiedServeSchedulerClaimErrorV1, CertifiedServeSchedulerObservationV1,
    claim_certified_serve_turn_v1,
};
use crate::sumeragi::{
    v2::VerifiedHeightContext,
    v2_certified_serve_payload_store::CertifiedServePayloadStoreV1,
    v2_lifecycle_coordinator::{
        AdmissionDecision, AdmissionRequest, LifecycleCoordinator, LifecycleState,
        LifecycleWorkClass, authority,
        ledger::{LifecycleLedgerStoreV1, LifecycleLedgerV1},
        projection,
        work_registry::{
            CertifiedServeRegistryBatchPublicationError, ClaimedCertifiedServeDispatchErrorV1,
            ConcreteLifecycleWorkRegistry, ConcreteWorkAddress,
            PreparedCertifiedServeRegistryBatchV1,
        },
    },
    v2_transport::{AuthenticatedCertifiedBodyRequest, authenticate_certified_body_request},
};
use iroha_crypto::{Algorithm, Hash, KeyPair, Signature};
use iroha_data_model::{block::consensus_v2 as wire, peer::PeerId};

struct ServeSchedulerFixture {
    verified: VerifiedHeightContext,
    keys: Vec<KeyPair>,
    _directory: tempfile::TempDir,
    payload_store: CertifiedServePayloadStoreV1,
    coordinator: LifecycleCoordinator,
    ordinal_authority: authority::RuntimeLifecycleOrdinalAuthority,
    registry: ConcreteLifecycleWorkRegistry,
}
impl ServeSchedulerFixture {
    fn new(seed: u8) -> Self {
        let mut keys = (seed..seed + 4)
            .map(|byte| {
                KeyPair::try_from_seed(vec![byte; 32], Algorithm::BlsNormal)
                    .expect("deterministic Certified-Serve scheduler BLS key")
            })
            .collect::<Vec<_>>();
        keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
        let proofs = keys
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("Certified-Serve scheduler proof of possession")
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
            network_id: crate::sumeragi::synthetic_network_id("certified-serve-scheduler"),
            protocol_version: wire::PROTOCOL_VERSION,
            height: 1,
            epoch: 0,
            epoch_end_height: u64::MAX,
            next_epoch_snapshot: None,
            mode: wire::ConsensusMode::Permissioned,
            parent_commit_qc: None,
            snapshot_bootstrap: None,
            quorum: wire::DualQuorum::from_roster(&roster)
                .expect("four-validator Certified-Serve scheduler quorum"),
            roster,
            nexus_amx_context_hash: Hash::new(b"Certified-Serve scheduler nexus"),
            execution_policy_hash: Hash::new(b"Certified-Serve scheduler policy"),
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
        let verified = VerifiedHeightContext::genesis(context, proofs)
            .expect("verified Certified-Serve scheduler context");
        let authority = authority::lifecycle_storage_owner_test_authority(&verified, 1, 4)
            .expect("bounded Certified-Serve scheduler authority");
        let mut coordinator = LifecycleCoordinator::new_with_authority(authority, 0);
        let (ordinal_authority, coordinator_ordinal_authority) =
            authority::lifecycle_ordinal_authorities_after_high_watermark(0);
        coordinator.lifecycle_ordinal_authority = Some(coordinator_ordinal_authority);
        let directory =
            tempfile::TempDir::new().expect("temporary Certified-Serve scheduler payload store");
        let (payload_store, recovery) =
            CertifiedServePayloadStoreV1::open(directory.path(), verified.context())
                .expect("open Certified-Serve scheduler payload store");
        assert!(recovery.is_empty());
        Self {
            verified,
            keys,
            _directory: directory,
            payload_store,
            coordinator,
            ordinal_authority,
            registry: ConcreteLifecycleWorkRegistry::default(),
        }
    }

    fn authenticated_request(&self, view: u64, marker: u8) -> AuthenticatedCertifiedBodyRequest {
        let context = self.verified.context();
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view,
        };
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: iroha_crypto::HashOf::from_untyped_unchecked(Hash::new([marker, 0xA1])),
            payload_hash: Hash::new([marker, 0xA2]),
        };
        let execution_commitment = wire::ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new([marker, 0xB1]),
            Hash::new([marker, 0xB2]),
            Hash::new([marker, 0xB3]),
            1,
            Hash::new([marker, 0xB4]),
        );
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
                    self.keys[usize::try_from(*signer).expect("small signer")].private_key(),
                    &preimage,
                )
                .payload()
                .to_vec()
            })
            .collect::<Vec<_>>();
        let share_refs = shares.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let certificate = wire::QuorumCertificate {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Prepare,
            subject,
            execution_commitment,
            signers,
            aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(&share_refs)
                .expect("aggregate Certified-Serve scheduler PrepareQC"),
        };
        let requester_index = 3;
        let mut request = wire::CertifiedBodyRequest {
            round,
            subject,
            certificate,
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
        authenticate_certified_body_request(context, request, &requester, |context, qc| {
            wire::finality::verify_quorum_certificate_with_validator_pops(
                context,
                qc,
                self.verified.proofs_of_possession(),
            )
            .map_err(|error| error.to_string())
        })
        .expect("authenticate Certified-Serve scheduler request")
    }

    fn admit(&mut self, view: u64, marker: u8) -> AuthenticatedCertifiedBodyRequest {
        let request = self.authenticated_request(view, marker);
        let receipt = self
            .payload_store
            .persist_pending_with_verified_retention(&self.verified, &self.keys[0], &request)
            .expect("persist Certified-Serve scheduler request");
        let prepared = projection::prepare_certified_serve_admission(
            self.coordinator.active_context(),
            &self.verified,
            &request,
            receipt,
        )
        .expect("prepare Certified-Serve scheduler admission");
        let (candidate, replay) = prepared.into_candidate_and_replay();
        let serve_key = candidate.key;
        let mut staged = self.coordinator.clone();
        let (decision, ordinal_reservation) =
            staged.reduce_admit_with_durable_ordinals(AdmissionRequest::Candidate(candidate));
        assert!(matches!(
            decision,
            AdmissionDecision::Admitted {
                producer_turn_ordinal: Some(_),
                ..
            }
        ));
        let ordinal_reservation =
            ordinal_reservation.expect("fresh Certified-Serve pair reserves shared ordinals");
        let batch = PreparedCertifiedServeRegistryBatchV1::from_fresh_admitted_pair(
            &staged, serve_key, replay,
        )
        .unwrap_or_else(|_| panic!("seal exact Certified-Serve scheduler registry pair"));
        let installed = self
            .registry
            .install_certified_serve_fresh_batch_before_publication(
                batch,
                &self.verified,
                &self.coordinator,
                &staged,
                || {
                    ordinal_reservation.mark_publication_started()?;
                    ordinal_reservation.commit_after_durable_publication()
                },
            );
        assert!(installed.is_ok(), "install exact Certified-Serve pair");
        self.coordinator = staged;
        request
    }

    fn ledger(&self) -> LifecycleLedgerV1 {
        LifecycleLedgerV1::from_coordinator(&self.coordinator)
            .expect("project exact Certified-Serve scheduler LedgerV1")
    }
}

#[test]
fn certified_serve_registry_accepts_an_actor_global_gap_but_rejects_backfill() {
    let mut fixture = ServeSchedulerFixture::new(0x18);
    let ledger_directory =
        tempfile::TempDir::new().expect("temporary actor-global-gap lifecycle ledger");
    fixture
        .coordinator
        .attach_empty_test_ledger(ledger_directory.path())
        .expect("attach the exact empty lifecycle ledger");
    let runtime_ordinals =
        crate::sumeragi::v2_runtime::RuntimeLifecycleOrdinalSource::from_authority(
            fixture.ordinal_authority.clone(),
        );
    runtime_ordinals
        .advance_past(13)
        .expect("reserve the actor-global prefix outside durable lifecycle work");
    let request = fixture.authenticated_request(0, 0x28);
    let receipt = fixture
        .payload_store
        .persist_pending_with_verified_retention(&fixture.verified, &fixture.keys[0], &request)
        .expect("persist the exact gapped Certified-Serve request");
    let prepared = projection::prepare_certified_serve_admission(
        fixture.coordinator.active_context(),
        &fixture.verified,
        &request,
        receipt,
    )
    .expect("prepare the exact gapped Certified-Serve admission");
    let (candidate, replay) = prepared.into_candidate_and_replay();
    let serve_key = candidate.key;
    let mut staged = fixture.coordinator.stage_durable_transaction();
    let (decision, ordinal_reservation) =
        staged.reduce_admit_with_durable_ordinals(AdmissionRequest::Candidate(candidate));
    assert!(matches!(
        decision,
        AdmissionDecision::Admitted {
            ordinal: 14,
            producer_turn_ordinal: Some(15),
            ..
        }
    ));
    let ordinal_reservation =
        ordinal_reservation.expect("gapped Serve owns one adjacent durable reservation");
    let batch =
        PreparedCertifiedServeRegistryBatchV1::from_fresh_admitted_pair(&staged, serve_key, replay)
            .unwrap_or_else(|_| panic!("seal the exact gapped Serve/Producer registry pair"));

    let mut backfill = fixture.coordinator.clone();
    backfill.high_water = 14;
    let backfill_before = format!("{backfill:?}");
    let publication_called = std::cell::Cell::new(false);
    let batch = match fixture
        .registry
        .install_certified_serve_fresh_batch_before_publication(
            batch,
            &fixture.verified,
            &backfill,
            &staged,
            || {
                publication_called.set(true);
                Ok::<(), ()>(())
            },
        ) {
        Err(CertifiedServeRegistryBatchPublicationError::Preflight(batch)) => batch,
        Err(CertifiedServeRegistryBatchPublicationError::Publication(_, _)) => {
            panic!("backfill must fail before publication")
        }
        Ok(()) => panic!("a durable Serve ordinal cannot backfill the current high-water mark"),
    };
    assert!(!publication_called.get());
    assert_eq!(format!("{backfill:?}"), backfill_before);
    assert_eq!(fixture.registry.len(), 0);

    publication_called.set(false);
    assert_eq!(
        runtime_ordinals
            .next_ordinal_for_test()
            .expect("inspect the fenced actor-global cursor"),
        Some(14)
    );
    let mut logical_current = fixture.coordinator.clone();
    logical_current.lifecycle_ordinal_authority = None;
    let current_before = format!("{logical_current:?}");
    fixture
        .registry
        .install_certified_serve_fresh_batch_before_publication(
            batch,
            &fixture.verified,
            &fixture.coordinator,
            &staged,
            || {
                publication_called.set(true);
                fixture
                    .coordinator
                    .persist_exact_staged_successor_with_ordinal_reservation(
                        &staged,
                        &ordinal_reservation,
                    )
            },
        )
        .unwrap_or_else(|_| panic!("publish the exact actor-global Serve/Producer gap"));
    assert!(publication_called.get());
    let mut logical_current = fixture.coordinator.clone();
    logical_current.lifecycle_ordinal_authority = None;
    assert_eq!(format!("{logical_current:?}"), current_before);
    assert_eq!(fixture.registry.len(), 2);
    fixture.coordinator = staged;
    assert_eq!(fixture.coordinator.high_water, 15);
    assert!(
        fixture
            .registry
            .exactly_covers_all_live_work(&fixture.verified, &fixture.coordinator)
    );
    assert_eq!(
        runtime_ordinals
            .next_ordinal_for_test()
            .expect("inspect the committed actor-global cursor"),
        Some(16)
    );
    let (_, persisted) = LifecycleLedgerStoreV1::open(
        ledger_directory.path(),
        fixture.coordinator.active_context(),
    )
    .expect("reopen the exact gapped Serve/Producer ledger");
    assert_eq!(persisted.high_water(), 15);
}

#[test]
fn certified_serve_claim_rolls_back_when_its_exact_carrier_drifted() {
    let mut fixture = ServeSchedulerFixture::new(0x21);
    let request = fixture.admit(0, 0x31);
    let ledger = fixture.ledger();
    let attestation = fixture
        .registry
        .attest_ready_certified_serve_request(&fixture.coordinator, &ledger, &request)
        .expect("attest exact Serve before carrier drift");
    let ordinal = *fixture
        .coordinator
        .ready_index
        .first()
        .expect("one Ready Certified-Serve row");
    let record = &fixture.coordinator.records[&ordinal];
    let slot = *record
        .physical_slots
        .first_key_value()
        .expect("one Serve slot")
        .0;
    let address = ConcreteWorkAddress::new(record.owner, ordinal, slot)
        .expect("exact Serve registry address");
    assert!(fixture.registry.remove_exact_for_test(address));
    let ready_before = fixture.coordinator.ready_index.clone();

    assert_eq!(
        claim_certified_serve_turn_v1(
            &mut fixture.coordinator,
            &fixture.registry,
            &ledger,
            vec![CertifiedServeSchedulerObservationV1::new(
                attestation,
                0,
                0,
                0,
            )],
        )
        .expect_err("drifted Serve carrier must fail after claim"),
        CertifiedServeSchedulerClaimErrorV1::InvalidClaimedCarrier(
            ClaimedCertifiedServeDispatchErrorV1::InvalidCarrier,
        )
    );
    assert_eq!(fixture.coordinator.ready_index, ready_before);
    assert!(fixture.coordinator.active_lease.is_none());
    assert!(fixture.coordinator.fault.is_none());
}

#[test]
fn certified_serve_scheduler_cannot_overtake_its_ready_predecessor() {
    let mut fixture = ServeSchedulerFixture::new(0x31);
    let older = fixture.admit(0, 0x41);
    let newer = fixture.admit(1, 0x42);
    let ledger = fixture.ledger();
    let older_attestation = fixture
        .registry
        .attest_ready_certified_serve_request(&fixture.coordinator, &ledger, &older)
        .expect("attest older Ready Serve");
    let newer_attestation = fixture
        .registry
        .attest_ready_certified_serve_request(&fixture.coordinator, &ledger, &newer)
        .expect("attest newer Ready Serve");
    let older_ordinal = fixture.coordinator.key_index[&fixture
        .coordinator
        .records
        .values()
        .find(|record| {
            fixture.coordinator.durable_records[&record.ordinal]
                .replay_authority
                .exactly_matches_certified_serve_request(&older)
        })
        .expect("older Serve row")
        .key];
    let newer_ordinal = fixture.coordinator.key_index[&fixture
        .coordinator
        .records
        .values()
        .find(|record| {
            fixture.coordinator.durable_records[&record.ordinal]
                .replay_authority
                .exactly_matches_certified_serve_request(&newer)
        })
        .expect("newer Serve row")
        .key];
    let dispatch = claim_certified_serve_turn_v1(
        &mut fixture.coordinator,
        &fixture.registry,
        &ledger,
        vec![
            CertifiedServeSchedulerObservationV1::new(newer_attestation, 0, 0, 0),
            CertifiedServeSchedulerObservationV1::new(
                older_attestation,
                u64::MAX,
                u64::MAX,
                u64::MAX,
            ),
        ],
    )
    .expect("claim predecessor-ordered Certified-Serve turn");
    let (lease, authenticated) = dispatch.into_worker_parts();

    assert_eq!(lease.ordinal(), older_ordinal);
    assert_eq!(authenticated, older);
    assert!(matches!(
        fixture.coordinator.records[&older_ordinal].state,
        LifecycleState::Claimed(_)
    ));
    assert_eq!(
        fixture.coordinator.records[&newer_ordinal].state,
        LifecycleState::Ready
    );
    assert_eq!(fixture.coordinator.active_lease.as_ref(), Some(&lease));
}

#[test]
fn certified_serve_scheduler_creates_exactly_one_live_claim() {
    let mut fixture = ServeSchedulerFixture::new(0x41);
    let first = fixture.admit(0, 0x51);
    let second = fixture.admit(1, 0x52);
    let ledger = fixture.ledger();
    let first = fixture
        .registry
        .attest_ready_certified_serve_request(&fixture.coordinator, &ledger, &first)
        .expect("attest first exact Serve");
    let second = fixture
        .registry
        .attest_ready_certified_serve_request(&fixture.coordinator, &ledger, &second)
        .expect("attest second exact Serve");
    let dispatch = claim_certified_serve_turn_v1(
        &mut fixture.coordinator,
        &fixture.registry,
        &ledger,
        vec![
            CertifiedServeSchedulerObservationV1::new(first, 2, 3, 4),
            CertifiedServeSchedulerObservationV1::new(second, 0, 0, 0),
        ],
    )
    .expect("claim exactly one Certified-Serve turn");

    assert_eq!(
        fixture
            .coordinator
            .records
            .values()
            .filter(|record| matches!(record.state, LifecycleState::Claimed(_)))
            .count(),
        1
    );
    assert_eq!(
        fixture
            .coordinator
            .records
            .values()
            .filter(|record| {
                record.work_class == LifecycleWorkClass::CertifiedServe
                    && record.state == LifecycleState::Ready
            })
            .count(),
        1
    );
    assert_eq!(
        fixture.coordinator.active_lease.as_ref(),
        Some(dispatch.lease())
    );
    assert!(matches!(
        claim_certified_serve_turn_v1(
            &mut fixture.coordinator,
            &fixture.registry,
            &ledger,
            Vec::new(),
        ),
        Err(CertifiedServeSchedulerClaimErrorV1::UnsettledLease(_))
    ));
    assert_eq!(
        fixture
            .coordinator
            .records
            .values()
            .filter(|record| matches!(record.state, LifecycleState::Claimed(_)))
            .count(),
        1
    );
}
