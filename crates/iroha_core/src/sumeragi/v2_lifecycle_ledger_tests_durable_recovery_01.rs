use std::{collections::BTreeMap, fs, num::NonZeroU64, path::Path};

use iroha_crypto::{Algorithm, KeyPair, Signature, SignatureOf};
use iroha_data_model::{
    block::{BlockHeader, BlockSignature, SignedBlock, consensus_v2 as wire},
    peer::PeerId,
};
use tempfile::TempDir;

use super::*;
use crate::{
    kura::Kura,
    sumeragi::{
        v2::RecoveredLifecycleOwnerKuraBindingV1,
        v2_body_store::ValidatedBodyReceipt,
        v2_core::{EventTag, Generation},
        v2_transport::{AuthenticatedCertifiedBodyRequest, authenticate_certified_body_request},
    },
};

struct RecoveryFixture {
    verified: VerifiedHeightContext,
    keys: Vec<KeyPair>,
}

#[derive(Clone, Copy)]
enum ServeTerminalFixture {
    Completed,
    Negative(
        crate::sumeragi::v2_certified_serve_payload_store::CertifiedServePayloadNegativeOutcome,
    ),
}

#[derive(Clone, Copy)]
enum StagedTerminalDrift {
    Record,
    Index,
    Debt,
    Capacity,
    HighWater,
}

fn snapshot_files(root: &Path) -> BTreeMap<std::path::PathBuf, Vec<u8>> {
    fn visit(root: &Path, directory: &Path, snapshot: &mut BTreeMap<std::path::PathBuf, Vec<u8>>) {
        let mut entries = fs::read_dir(directory)
            .expect("read startup no-mutation fixture directory")
            .collect::<Result<Vec<_>, _>>()
            .expect("decode startup no-mutation directory entries");
        entries.sort_by_key(fs::DirEntry::path);
        for entry in entries {
            let path = entry.path();
            if path.is_dir() {
                visit(root, &path, snapshot);
            } else {
                let relative = path
                    .strip_prefix(root)
                    .expect("snapshot path remains under fixture root")
                    .to_path_buf();
                assert!(
                    snapshot
                        .insert(relative, fs::read(path).expect("read startup fixture file"))
                        .is_none()
                );
            }
        }
    }
    let mut snapshot = BTreeMap::new();
    visit(root, root, &mut snapshot);
    snapshot
}

impl RecoveryFixture {
    fn new(network: &str, first_seed: u8) -> Self {
        let mut keys = (first_seed..first_seed + 4)
            .map(|seed| {
                KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                    .expect("deterministic durable Ready-Fetch BLS key")
            })
            .collect::<Vec<_>>();
        keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
        let proofs = keys
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("durable Ready-Fetch proof of possession")
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
            network_id: crate::sumeragi::synthetic_network_id(network),
            protocol_version: wire::PROTOCOL_VERSION,
            height: 1,
            epoch: 1,
            epoch_end_height: 100,
            next_epoch_snapshot: None,
            mode: wire::ConsensusMode::Permissioned,
            parent_commit_qc: None,
            snapshot_bootstrap: None,
            quorum: wire::DualQuorum::from_roster(&roster)
                .expect("four-validator durable Ready-Fetch quorum"),
            roster,
            nexus_amx_context_hash: Hash::new(b"durable Ready-Fetch nexus context"),
            execution_policy_hash: Hash::new(b"durable Ready-Fetch execution policy"),
            da_layout: wire::DataAvailabilityLayout {
                encoding: wire::PayloadEncoding::ReedSolomon16,
                chunk_size_bytes: 1024,
                data_shards: 1,
                parity_shards: 1,
                max_payload_size_bytes: 512 * 1024,
                max_chunk_count: 1024,
            },
            leader_seed: [0xA5; 32],
        };
        let verified = VerifiedHeightContext::genesis(context, proofs)
            .expect("verified durable Ready-Fetch height context");
        Self { verified, keys }
    }

    fn lifecycle_context(&self) -> LifecycleContext {
        projection::lifecycle_context(self.verified.context())
    }

    fn open_store(&self, directory: &TempDir) -> V2BodyStore {
        V2BodyStore::open(directory.path(), self.verified.context().clone())
            .expect("open durable Ready-Fetch body store")
    }

    #[allow(clippy::too_many_lines)]
    fn fetch_record(
        &self,
        store: &mut V2BodyStore,
        view: u64,
        marker: u8,
        ordinal: u128,
        certified_sources: Option<Vec<PeerId>>,
        corrupt_qc: bool,
    ) -> LifecycleLedgerRecordV1 {
        self.fetch_record_with_block_signature(
            store,
            view,
            marker,
            ordinal,
            certified_sources,
            corrupt_qc,
            None,
        )
    }

    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    fn fetch_record_with_block_signature(
        &self,
        store: &mut V2BodyStore,
        view: u64,
        marker: u8,
        ordinal: u128,
        certified_sources: Option<Vec<PeerId>>,
        corrupt_qc: bool,
        block_signature_override: Option<(u64, usize)>,
    ) -> LifecycleLedgerRecordV1 {
        let context = self.verified.context();
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view,
        };
        let leader = context.leader(view);
        let leader_index = usize::try_from(leader).expect("fixture leader fits usize");
        let (block_signature_index, block_signer_index) =
            block_signature_override.unwrap_or((u64::from(leader), leader_index));
        let header = BlockHeader::new(
            NonZeroU64::new(context.height).expect("fixture height is non-zero"),
            None,
            None,
            None,
            1_000 + u64::from(marker),
            view,
        );
        let block_signature =
            SignatureOf::try_from_hash(self.keys[block_signer_index].private_key(), header.hash())
                .expect("sign durable Ready-Fetch block");
        let block = SignedBlock::presigned(
            BlockSignature::new(block_signature_index, block_signature),
            header,
            Vec::new(),
        );
        let body = block
            .encode_wire()
            .expect("encode canonical SignedBlockWire");
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: block.hash(),
            payload_hash: Hash::new(&body),
        };
        let chunks = wire::encode_payload_chunks(context.da_layout, &body)
            .expect("encode durable Ready-Fetch chunks");
        let manifest = wire::PayloadManifest::derive(
            context,
            round,
            subject,
            u64::try_from(body.len()).expect("fixture body length fits u64"),
            &chunks,
        )
        .expect("derive durable Ready-Fetch manifest");
        let receipt = store
            .store(manifest.clone(), body)
            .expect("fsync durable Ready-Fetch body");
        let execution_commitment = wire::ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new([marker, 1]),
            Hash::new([marker, 2]),
            Hash::new([marker, 3]),
            1,
            Hash::new([marker, 4]),
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
                    self.keys[usize::try_from(*signer).expect("fixture signer fits usize")]
                        .private_key(),
                    &preimage,
                )
                .payload()
                .to_vec()
            })
            .collect::<Vec<_>>();
        let share_refs = shares.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let mut certificate = wire::QuorumCertificate {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Prepare,
            subject,
            execution_commitment,
            signers,
            aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(&share_refs)
                .expect("aggregate durable Ready-Fetch PrepareQC"),
        };
        if corrupt_qc {
            certificate.aggregate_signature[0] ^= 1;
        }
        let sources = certified_sources.unwrap_or_else(|| {
            context
                .roster
                .iter()
                .map(|entry| entry.validator.clone())
                .collect()
        });
        let case =
            super::super::super::replay_authority::exact_durable_certified_fetch_record_fixture(
                self.lifecycle_context(),
                EventTag::new(context.height, view, Generation::new(u64::from(marker))),
                certificate,
                manifest,
                sources,
                &receipt,
            );
        let causal_root =
            CausalRoot::new(LifecycleDigest::new(*Hash::new([marker, 0xF0]).as_ref()));
        let owner = OwnerId::new(causal_root, ordinal);
        LifecycleLedgerRecordV1::new(
            case.key,
            owner,
            ordinal,
            LifecycleWorkClass::Fetch,
            LifecycleStage::new(LifecycleStageKind::FetchBody, PredecessorScope::Independent),
            None,
            causal_root.digest(),
            case.payload,
            case.authority,
            DurableContinuation::None,
        )
        .expect("construct durable Ready-Fetch LedgerV1 row")
    }

    fn terminal_validate_record(
        &self,
        store: &mut V2BodyStore,
        view: u64,
        marker: u8,
        ordinal: u128,
    ) -> LifecycleLedgerRecordV1 {
        let context = self.verified.context();
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view,
        };
        let leader = context.leader(view);
        let leader_index = usize::try_from(leader).expect("fixture leader fits usize");
        let header = BlockHeader::new(
            NonZeroU64::new(context.height).expect("fixture height is non-zero"),
            None,
            None,
            None,
            2_000 + u64::from(marker),
            view,
        );
        let signature =
            SignatureOf::try_from_hash(self.keys[leader_index].private_key(), header.hash())
                .expect("sign terminal Validate block");
        let block = SignedBlock::presigned(
            BlockSignature::new(u64::from(leader), signature),
            header,
            Vec::new(),
        );
        let body = block
            .encode_wire()
            .expect("encode terminal Validate SignedBlockWire");
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: block.hash(),
            payload_hash: Hash::new(&body),
        };
        let chunks = wire::encode_payload_chunks(context.da_layout, &body)
            .expect("encode terminal Validate chunks");
        let manifest = wire::PayloadManifest::derive(
            context,
            round,
            subject,
            u64::try_from(body.len()).expect("fixture body length fits u64"),
            &chunks,
        )
        .expect("derive terminal Validate manifest");
        let receipt = store
            .store(manifest.clone(), body)
            .expect("fsync terminal Validate body");
        let replay = super::super::super::replay_authority::exact_local_body_record_fixture(
            self.lifecycle_context(),
            EventTag::new(context.height, view, Generation::new(u64::from(marker))),
            manifest,
            &receipt,
            LifecycleStageKind::ValidateBody,
        )
        .expect("project exact terminal Validate replay row");
        let commitment = ValidatedBodyReceipt::for_test(receipt.clone()).execution_commitment();
        let _validated = store
            .execute_durable_validation(receipt.clone(), receipt.manifest_hash(), |_| {
                Ok::<_, String>(commitment)
            })
            .expect("persist terminal Validate success outcome");
        let causal_root = CausalRoot::new(LifecycleDigest::new([marker; 32]));
        LifecycleLedgerRecordV1::new(
            replay.key,
            OwnerId::new(causal_root, ordinal),
            ordinal,
            replay.work_class,
            replay.stage,
            Some(TerminalOutcome::Advanced),
            causal_root.digest(),
            replay.payload,
            replay.authority,
            DurableContinuation::AdvancedNoSuccessor,
        )
        .expect("construct terminal Validate ledger row")
    }

    fn authenticated_serve_request(
        &self,
        view: u64,
        marker: u8,
        requester_index: usize,
    ) -> AuthenticatedCertifiedBodyRequest {
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: iroha_crypto::HashOf::from_untyped_unchecked(Hash::new([marker, 0xA1])),
            payload_hash: Hash::new([marker, 0xA2]),
        };
        self.authenticated_serve_request_for_subject(view, marker, requester_index, subject)
    }

    fn authenticated_serve_request_for_subject(
        &self,
        view: u64,
        marker: u8,
        requester_index: usize,
        subject: wire::BlockSubject,
    ) -> AuthenticatedCertifiedBodyRequest {
        let context = self.verified.context();
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view,
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
                    self.keys[usize::try_from(*signer).expect("fixture signer fits usize")]
                        .private_key(),
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
                .expect("aggregate Certified-Serve PrepareQC"),
        };
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
        .expect("authenticate Certified-Serve request")
    }

    fn completed_serve_exchange(
        &self,
        store: &mut V2BodyStore,
        view: u64,
        marker: u8,
        requester_index: usize,
    ) -> (
        AuthenticatedCertifiedBodyRequest,
        crate::sumeragi::v2_body_store::DurableBodyReceipt,
        wire::CertifiedBodyResponse,
    ) {
        let context = self.verified.context();
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view,
        };
        let leader = context.leader(view);
        let leader_index = usize::try_from(leader).expect("fixture leader fits usize");
        let header = BlockHeader::new(
            NonZeroU64::new(context.height).expect("fixture height is non-zero"),
            None,
            None,
            None,
            3_000 + u64::from(marker),
            view,
        );
        let signature =
            SignatureOf::try_from_hash(self.keys[leader_index].private_key(), header.hash())
                .expect("sign completed Serve block");
        let block = SignedBlock::presigned(
            BlockSignature::new(u64::from(leader), signature),
            header,
            Vec::new(),
        );
        let body = block
            .encode_wire()
            .expect("encode completed Serve SignedBlockWire");
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: block.hash(),
            payload_hash: Hash::new(&body),
        };
        let chunks = wire::encode_payload_chunks(context.da_layout, &body)
            .expect("encode completed Serve chunks");
        let manifest = wire::PayloadManifest::derive(
            context,
            round,
            subject,
            u64::try_from(body.len()).expect("fixture body length fits u64"),
            &chunks,
        )
        .expect("derive completed Serve manifest");
        let durable_body = store
            .store(manifest.clone(), body.clone())
            .expect("fsync completed Serve body");
        let request =
            self.authenticated_serve_request_for_subject(view, marker, requester_index, subject);
        let responder_index = 0;
        let mut response = wire::CertifiedBodyResponse {
            request_hash: request.request_hash(),
            manifest,
            body,
            responder: context.roster[responder_index].validator.clone(),
            signature: Vec::new(),
        };
        response.signature = Signature::new(
            self.keys[responder_index].private_key(),
            &response.signature_preimage(),
        )
        .payload()
        .to_vec();
        (request, durable_body, response)
    }

    fn ledger(&self, records: Vec<LifecycleLedgerRecordV1>) -> LifecycleLedgerV1 {
        let high_water = records
            .iter()
            .map(LifecycleLedgerRecordV1::ordinal)
            .max()
            .unwrap_or(0);
        LifecycleLedgerV1::new(
            self.lifecycle_context(),
            high_water,
            records,
            BTreeMap::new(),
        )
        .expect("construct durable Ready-Fetch LedgerV1")
    }

    fn persist_ledger(
        &self,
        directory: &TempDir,
        ledger: &LifecycleLedgerV1,
    ) -> LifecycleLedgerStoreV1 {
        let (store, opened) =
            LifecycleLedgerStoreV1::open(directory.path(), self.lifecycle_context())
                .expect("open durable Ready-Fetch lifecycle ledger store");
        assert!(opened.records().is_empty());
        store
            .persist(ledger)
            .expect("persist durable Ready-Fetch lifecycle ledger");
        store
    }

    fn open_empty_serve_payloads(
        &self,
        directory: &TempDir,
        body_store: &V2BodyStore,
    ) -> (
        CertifiedServePayloadStoreV1,
        AuthenticatedCertifiedServePayloadRecoveryCut,
    ) {
        let (store, recovered) =
            CertifiedServePayloadStoreV1::open(directory.path(), self.verified.context())
                .expect("open empty Certified-Serve payload store");
        let authenticated = recovered
            .authenticate(&self.verified, &self.keys[0], body_store)
            .expect("authenticate empty Certified-Serve payload recovery");
        (store, authenticated)
    }

    fn open_empty_owner(
        &self,
        body_directory: &TempDir,
        payload_directory: &TempDir,
        ledger_directory: &TempDir,
    ) -> ProductionLifecycleOwnerV1 {
        let body_store = self.open_store(body_directory);
        let (payload_store, payloads) =
            self.open_empty_serve_payloads(payload_directory, &body_store);
        let ledger = self.ledger(Vec::new());
        let ledger_store = self.persist_ledger(ledger_directory, &ledger);
        let cut = ledger
            .into_durable_certified_body_pipeline_storage_recovery_cut(
                self.verified.clone(),
                ledger_store,
                body_store,
            )
            .expect("seal empty fresh-admission storage cut");
        cut.open_owner_for_test(payload_store, payloads)
            .expect("open empty fresh-admission production owner")
    }

    fn open_completed_serve_owner(
        &self,
        body_directory: &TempDir,
        payload_directory: &TempDir,
        ledger_directory: &TempDir,
    ) -> (
        ProductionLifecycleOwnerV1,
        AuthenticatedCertifiedBodyRequest,
        crate::sumeragi::v2_body_store::DurableBodyReceipt,
        wire::CertifiedBodyResponse,
    ) {
        let mut body_store = self.open_store(body_directory);
        let (request, durable_body, response) =
            self.completed_serve_exchange(&mut body_store, 0, 0xC1, 3);
        let (payload_store, payloads) =
            self.open_empty_serve_payloads(payload_directory, &body_store);
        let ledger = self.ledger(Vec::new());
        let ledger_store = self.persist_ledger(ledger_directory, &ledger);
        let cut = ledger
            .into_durable_certified_body_pipeline_storage_recovery_cut(
                self.verified.clone(),
                ledger_store,
                body_store,
            )
            .expect("seal completed-Serve production storage cut");
        let owner = cut
            .open_owner_for_test(payload_store, payloads)
            .expect("open completed-Serve production owner");
        (owner, request, durable_body, response)
    }

    fn open_terminal_serve_owner(
        &self,
        body_directory: &TempDir,
        payload_directory: &TempDir,
        ledger_directory: &TempDir,
        terminal: ServeTerminalFixture,
    ) -> (
        ProductionLifecycleOwnerV1,
        AuthenticatedCertifiedBodyRequest,
    ) {
        let mut body_store = self.open_store(body_directory);
        let (request, response) = match terminal {
            ServeTerminalFixture::Completed => {
                let (request, _durable_body, response) =
                    self.completed_serve_exchange(&mut body_store, 0, 0xD1, 3);
                (request, Some(response))
            }
            ServeTerminalFixture::Negative(_) => {
                (self.authenticated_serve_request(0, 0xD2, 3), None)
            }
        };
        let (mut payload_store, recovery) =
            CertifiedServePayloadStoreV1::open(payload_directory.path(), self.verified.context())
                .expect("open terminal Serve payload store");
        assert!(recovery.is_empty());
        let pending = payload_store
            .persist_pending_with_verified_retention(&self.verified, &self.keys[0], &request)
            .expect("persist terminal Serve Pending frame");
        let authority = authority::lifecycle_storage_owner_test_authority(&self.verified, 1, 1)
            .expect("construct terminal Serve lifecycle authority");
        let mut coordinator = LifecycleCoordinator::new_with_authority(authority, 0);
        assert!(matches!(
            coordinator
                .admit_certified_serve(&self.verified, &request, pending)
                .expect("project terminal Serve request"),
            super::super::super::AdmissionDecision::Admitted { .. }
        ));
        let ready = coordinator.ready_index.iter().map(|ordinal| {
            let record = &coordinator.records[ordinal];
            (
                *ordinal,
                super::super::super::SchedulerReadyInputs::new(record, None, [0; 6]),
            )
        });
        let TurnPlan::Execute(lease) = coordinator.plan_turn(
            super::super::super::SchedulerInputs::new([], ready)
                .expect("terminal Serve has one exact Ready row"),
        ) else {
            panic!("terminal Serve must own the selected turn")
        };
        match terminal {
            ServeTerminalFixture::Completed => {
                let response = response.expect("completed fixture retains response");
                let completed = payload_store
                    .persist_completed(&request, &response)
                    .expect("persist completed Serve tombstone");
                let producer = coordinator.producer_debts[&lease.ordinal];
                let terminal = CertifiedServeTerminalReplayAuthorityPairV1::from_completed_receipt(
                    coordinator.active_context,
                    &coordinator.records[&lease.ordinal],
                    &coordinator.durable_records[&lease.ordinal],
                    &coordinator.records[&producer],
                    &coordinator.durable_records[&producer],
                    completed,
                )
                .expect("close completed Serve terminal family");
                coordinator.reduce_settle_turn(
                    lease,
                    TurnOutcome::Terminal(terminal.terminal_outcome()),
                    Some(terminal),
                );
                assert_eq!(coordinator.fault(), None);
            }
            ServeTerminalFixture::Negative(outcome) => {
                let negative = payload_store
                    .persist_negative(pending.id(), outcome)
                    .expect("persist negative Serve tombstone");
                let producer = coordinator.producer_debts[&lease.ordinal];
                let terminal = CertifiedServeTerminalReplayAuthorityPairV1::from_negative_receipt(
                    coordinator.active_context,
                    &coordinator.records[&lease.ordinal],
                    &coordinator.durable_records[&lease.ordinal],
                    &coordinator.records[&producer],
                    &coordinator.durable_records[&producer],
                    negative,
                )
                .expect("close negative Serve terminal family");
                coordinator.reduce_settle_turn(
                    lease,
                    TurnOutcome::Terminal(terminal.terminal_outcome()),
                    Some(terminal),
                );
                assert_eq!(coordinator.fault(), None);
            }
        }
        let ledger = LifecycleLedgerV1::from_coordinator(&coordinator)
            .expect("project terminal Serve LedgerV1");
        let ledger_store = self.persist_ledger(ledger_directory, &ledger);
        drop(payload_store);
        let (payload_store, recovered) =
            CertifiedServePayloadStoreV1::open(payload_directory.path(), self.verified.context())
                .expect("reopen terminal Serve payload store");
        let payloads = recovered
            .authenticate(&self.verified, &self.keys[0], &body_store)
            .expect("authenticate terminal Serve payload");
        let cut = ledger
            .into_durable_certified_body_pipeline_storage_recovery_cut(
                self.verified.clone(),
                ledger_store,
                body_store,
            )
            .expect("seal terminal Serve storage cut");
        let owner = cut
            .open_owner_for_test(payload_store, payloads)
            .expect("open terminal Serve production owner");
        (owner, request)
    }
}

/// Structural stand-in for the sealed adapter/WAL projection used only to exercise the
/// ledger oracle's census behavior. It deliberately authorizes no runtime operation.
struct TerminalDecisionProjectionFixture {
    context: LifecycleContext,
    fetch: LifecycleLedgerRecordV1,
    store: LifecycleLedgerRecordV1,
    validate: LifecycleLedgerRecordV1,
    apply: LifecycleLedgerRecordV1,
    subject: wire::BlockSubject,
    certificate: wire::QuorumCertificate,
}

impl TerminalRecoveredDecisionApplyProjectionV1 for TerminalDecisionProjectionFixture {
    fn belongs_to_context(&self, context: LifecycleContext) -> bool {
        self.context == context
    }

    fn names_fetch_record(&self, record: &LifecycleLedgerRecordV1) -> bool {
        record.key() == self.fetch.key()
    }

    fn exactly_matches_advanced_apply_parent(
        &self,
        fetch: &LifecycleLedgerRecordV1,
        store_ordinal: u128,
    ) -> bool {
        fetch == &self.fetch && store_ordinal == self.store.ordinal()
    }

    fn exactly_matches_terminal_successor_records(
        &self,
        owner: OwnerId,
        store: &LifecycleLedgerRecordV1,
        validate: &LifecycleLedgerRecordV1,
        apply: &LifecycleLedgerRecordV1,
    ) -> bool {
        owner == self.fetch.owner()
            && store == &self.store
            && validate == &self.validate
            && apply == &self.apply
    }
}

struct RecoveredDecisionStageProjectionFixture {
    context: LifecycleContext,
    live_fetch: LifecycleLedgerRecordV1,
    lineage: RecoveredDecisionApplyCandidateLineageV1,
    collision_validate: LifecycleLedgerRecordV1,
    validate_crash_prefix: LifecycleLedgerV1,
}

impl RecoveredDecisionApplyStageProjectionV1 for RecoveredDecisionStageProjectionFixture {
    fn belongs_to_context(&self, context: LifecycleContext) -> bool {
        self.context == context
    }

    fn names_fetch_record(&self, record: &LifecycleLedgerRecordV1) -> bool {
        record.key() == self.live_fetch.key()
    }

    fn exactly_matches_live_fetch(&self, fetch: &LifecycleLedgerRecordV1) -> bool {
        fetch == &self.live_fetch
    }

    fn exactly_matches_advanced_fetch(
        &self,
        fetch: &LifecycleLedgerRecordV1,
        store_ordinal: u128,
    ) -> bool {
        fetch.key() == self.live_fetch.key()
            && fetch.owner() == self.live_fetch.owner()
            && fetch.ordinal() == self.live_fetch.ordinal()
            && fetch.work_class() == self.live_fetch.work_class()
            && fetch.stage() == self.live_fetch.stage()
            && fetch.terminal() == Some(Some(TerminalOutcome::Advanced))
            && fetch.reconstruction_source() == self.live_fetch.reconstruction_source()
            && fetch.durable_payload() == self.live_fetch.durable_payload()
            && fetch.continuation()
                == Some(DurableContinuation::successor(
                    DurableContinuationEdge::FetchToStore,
                    store_ordinal,
                ))
            && fetch.replay_authority == self.live_fetch.replay_authority
    }

    fn lineage(&self) -> &RecoveredDecisionApplyCandidateLineageV1 {
        &self.lineage
    }
}

/// Structural stand-in for the cold released-Validate projection. It binds one
/// historical no-successor tombstone to the current recovered Decision lineage
/// without retaining any operation authority.
struct RecoveredReleasedDecisionStageProjectionFixture {
    context: LifecycleContext,
    live_fetch: LifecycleLedgerRecordV1,
    released_terminal: LifecycleLedgerRecordV1,
    lineage: RecoveredDecisionApplyCandidateLineageV1,
}

impl RecoveredDecisionReleasedApplyStageProjectionV1
    for RecoveredReleasedDecisionStageProjectionFixture
{
    fn belongs_to_context(&self, context: LifecycleContext) -> bool {
        self.context == context
    }

    fn names_fetch_record(&self, record: &LifecycleLedgerRecordV1) -> bool {
        record.key() == self.live_fetch.key()
    }

    fn names_terminal_validate_record(
        &self,
        context: LifecycleContext,
        record: &LifecycleLedgerRecordV1,
    ) -> bool {
        context == self.context && record == &self.released_terminal
    }

    fn lineage(&self) -> &RecoveredDecisionApplyCandidateLineageV1 {
        &self.lineage
    }
}

fn recovered_decision_store_crash_prefix_fixture(
    fixture: &RecoveryFixture,
) -> (LifecycleLedgerV1, RecoveredDecisionStageProjectionFixture) {
    let context = fixture.lifecycle_context();
    let certified_sources = fixture
        .verified
        .context()
        .roster
        .iter()
        .map(|entry| entry.validator.clone())
        .collect();
    let ([fetch_case, store_case, validate_case, apply_case], _, _) =
        super::super::super::replay_authority::exact_recovered_decision_terminal_family_fixture(
            context,
            certified_sources,
            0xD7,
        );
    let causal_root = CausalRoot::new(LifecycleDigest::new(
        *Hash::new(b"recovered Decision Store crash-prefix owner").as_ref(),
    ));
    let owner = OwnerId::new(causal_root, 1);
    let live_fetch = LifecycleLedgerRecordV1::new(
        fetch_case.key,
        owner,
        1,
        fetch_case.work_class,
        fetch_case.stage,
        None,
        causal_root.digest(),
        fetch_case.payload,
        fetch_case.authority.clone(),
        DurableContinuation::None,
    )
    .expect("construct live recovered Decision Fetch fixture");
    let advanced_fetch = LifecycleLedgerRecordV1::new(
        fetch_case.key,
        owner,
        1,
        fetch_case.work_class,
        fetch_case.stage,
        Some(TerminalOutcome::Advanced),
        causal_root.digest(),
        fetch_case.payload,
        fetch_case.authority.clone(),
        DurableContinuation::successor(DurableContinuationEdge::FetchToStore, 2),
    )
    .expect("construct advanced recovered Decision Fetch fixture");
    let live_store = LifecycleLedgerRecordV1::new(
        store_case.key,
        owner,
        2,
        store_case.work_class,
        store_case.stage,
        None,
        causal_root.digest(),
        store_case.payload,
        store_case.authority.clone(),
        DurableContinuation::None,
    )
    .expect("construct recovered Decision Store crash cut");
    let candidate = |case: &super::super::super::replay_authority::ReplayCase| {
        CandidateAdmission::new(
            case.key,
            causal_root,
            case.work_class,
            case.stage,
            InitialLifecycleState::Ready,
            causal_root.digest(),
            case.payload,
            case.authority.clone(),
            super::super::super::PhysicalGeometry::new([], []),
            None,
        )
    };
    let lineage = RecoveredDecisionApplyCandidateLineageV1::from_candidates_for_test(
        fetch_case.authority,
        candidate(&store_case),
        candidate(&validate_case),
        candidate(&apply_case),
    );
    let collision_root = CausalRoot::new(LifecycleDigest::new(
        *Hash::new(b"foreign recovered Decision Validate collision owner").as_ref(),
    ));
    let collision_owner = OwnerId::new(collision_root, 3);
    let collision_validate = LifecycleLedgerRecordV1::new(
        validate_case.key,
        collision_owner,
        3,
        validate_case.work_class,
        validate_case.stage,
        None,
        collision_root.digest(),
        validate_case.payload,
        validate_case.authority.clone(),
        DurableContinuation::None,
    )
    .expect("construct exact-key recovered Decision Validate collision");
    let unrelated_root = CausalRoot::new(LifecycleDigest::new(
        *Hash::new(b"unrelated row after recovered Decision Store").as_ref(),
    ));
    let unrelated = unrelated_live_record(context, OwnerId::new(unrelated_root, 3), 3, 0xD8);
    let advanced_store = LifecycleLedgerRecordV1::new(
        store_case.key,
        owner,
        2,
        store_case.work_class,
        store_case.stage,
        Some(TerminalOutcome::Advanced),
        causal_root.digest(),
        store_case.payload,
        store_case.authority,
        DurableContinuation::successor(DurableContinuationEdge::StoreToValidate, 4),
    )
    .expect("construct advanced recovered Decision Store fixture");
    let live_validate = LifecycleLedgerRecordV1::new(
        validate_case.key,
        owner,
        4,
        validate_case.work_class,
        validate_case.stage,
        None,
        causal_root.digest(),
        validate_case.payload,
        validate_case.authority.clone(),
        DurableContinuation::None,
    )
    .expect("construct recovered Decision Validate crash cut");
    let validate_crash_prefix = LifecycleLedgerV1::new(
        context,
        4,
        vec![
            advanced_fetch.clone(),
            advanced_store,
            unrelated.clone(),
            live_validate,
        ],
        BTreeMap::new(),
    )
    .expect("construct exact Validate crash prefix beside unrelated history");
    let ledger = LifecycleLedgerV1::new(
        context,
        3,
        vec![advanced_fetch, live_store, unrelated],
        BTreeMap::new(),
    )
    .expect("construct exact Store crash prefix beside unrelated history");
    (
        ledger,
        RecoveredDecisionStageProjectionFixture {
            context,
            live_fetch,
            lineage,
            collision_validate,
            validate_crash_prefix,
        },
    )
}

fn recovered_released_decision_apply_fixture(
    fixture: &RecoveryFixture,
) -> (
    LifecycleLedgerV1,
    RecoveredReleasedDecisionStageProjectionFixture,
) {
    let (_, ordinary) = recovered_decision_store_crash_prefix_fixture(fixture);
    let RecoveredDecisionStageProjectionFixture {
        context,
        live_fetch,
        lineage,
        collision_validate,
        ..
    } = ordinary;
    let terminal_root = CausalRoot::new(LifecycleDigest::new(
        *Hash::new(b"historical released recovered Decision Validate owner").as_ref(),
    ));
    let terminal_owner = OwnerId::new(terminal_root, 1);
    let released_terminal = LifecycleLedgerRecordV1::new(
        collision_validate
            .key()
            .expect("released fixture retains one Validate key"),
        terminal_owner,
        1,
        collision_validate
            .work_class()
            .expect("released fixture retains one Validate work class"),
        collision_validate
            .stage()
            .expect("released fixture retains one Validate stage"),
        Some(TerminalOutcome::Advanced),
        terminal_root.digest(),
        collision_validate
            .durable_payload()
            .expect("released fixture retains one body frame"),
        collision_validate.replay_authority,
        DurableContinuation::AdvancedNoSuccessor,
    )
    .expect("construct historical released Validate tombstone");
    assert_eq!(
        released_terminal.work_class(),
        Some(LifecycleWorkClass::Validate)
    );
    let ledger =
        LifecycleLedgerV1::new(context, 1, vec![released_terminal.clone()], BTreeMap::new())
            .expect("construct released-Validate cold recovery prefix");
    let projection = RecoveredReleasedDecisionStageProjectionFixture {
        context,
        live_fetch,
        released_terminal,
        lineage,
    };
    (ledger, projection)
}

fn terminal_decision_chain_fixture(
    fixture: &RecoveryFixture,
) -> (LifecycleLedgerV1, TerminalDecisionProjectionFixture) {
    terminal_decision_chain_fixture_with_seed(fixture, 0xE1)
}

fn terminal_decision_chain_fixture_with_seed(
    fixture: &RecoveryFixture,
    seed: u8,
) -> (LifecycleLedgerV1, TerminalDecisionProjectionFixture) {
    let context = fixture.lifecycle_context();
    let certified_sources = fixture
        .verified
        .context()
        .roster
        .iter()
        .map(|entry| entry.validator.clone())
        .collect();
    let ([fetch_case, store_case, validate_case, apply_case], subject, certificate) =
        super::super::super::replay_authority::exact_recovered_decision_terminal_family_fixture(
            context,
            certified_sources,
            seed,
        );
    assert_eq!(fetch_case.payload, DurablePayloadReference::None);
    let payload = store_case.payload;
    assert_eq!(validate_case.payload, payload);
    assert_eq!(apply_case.payload, payload);

    let causal_root = CausalRoot::new(LifecycleDigest::new(
        *Hash::new(b"terminal recovered Decision ledger fixture").as_ref(),
    ));
    let owner = OwnerId::new(causal_root, 1);
    let fetch = LifecycleLedgerRecordV1::new(
        fetch_case.key,
        owner,
        1,
        fetch_case.work_class,
        fetch_case.stage,
        Some(TerminalOutcome::Advanced),
        causal_root.digest(),
        fetch_case.payload,
        fetch_case.authority,
        DurableContinuation::successor(DurableContinuationEdge::FetchToStore, 2),
    )
    .expect("construct terminal Decision Fetch parent");
    let store = LifecycleLedgerRecordV1::new(
        store_case.key,
        owner,
        2,
        store_case.work_class,
        store_case.stage,
        Some(TerminalOutcome::Advanced),
        causal_root.digest(),
        store_case.payload,
        store_case.authority,
        DurableContinuation::successor(DurableContinuationEdge::StoreToValidate, 3),
    )
    .expect("construct terminal Decision Store row");
    let validate = LifecycleLedgerRecordV1::new(
        validate_case.key,
        owner,
        3,
        validate_case.work_class,
        validate_case.stage,
        Some(TerminalOutcome::Advanced),
        causal_root.digest(),
        validate_case.payload,
        validate_case.authority,
        DurableContinuation::successor(DurableContinuationEdge::ValidateToApply, 4),
    )
    .expect("construct terminal Decision Validate row");
    let apply = LifecycleLedgerRecordV1::new(
        apply_case.key,
        owner,
        4,
        apply_case.work_class,
        apply_case.stage,
        Some(TerminalOutcome::Advanced),
        causal_root.digest(),
        apply_case.payload,
        apply_case.authority,
        DurableContinuation::None,
    )
    .expect("construct terminal Decision Apply row");
    let projection = TerminalDecisionProjectionFixture {
        context,
        fetch: fetch.clone(),
        store: store.clone(),
        validate: validate.clone(),
        apply: apply.clone(),
        subject,
        certificate,
    };
    let ledger = LifecycleLedgerV1::new(
        context,
        4,
        vec![fetch, store, validate, apply],
        BTreeMap::new(),
    )
    .expect("construct exact terminal recovered Decision chain");
    (ledger, projection)
}

fn complete_tip_for_terminal_decision(
    fixture: &RecoveryFixture,
    projection: &TerminalDecisionProjectionFixture,
) -> crate::sumeragi::v2_recovery::RecoveredCompleteTipActivationAuthority {
    let artifact = wire::finality::V2FinalityArtifact::new(
        fixture.verified.context().clone(),
        projection.subject.clone(),
        projection.certificate.clone(),
        fixture.verified.proofs_of_possession().to_vec(),
    );
    let receipt = crate::kura::KuraV2CommitReceipt::for_test(&artifact);
    let predecessor = crate::sumeragi::v2_recovery::DurableV2PredecessorIdentity::authenticate(
        &artifact, &receipt,
    )
    .expect("terminal Decision finality and receipt authenticate");
    let successor_context_id = wire::HeightContextId(
        iroha_crypto::HashOf::<wire::HeightContext>::from_untyped_unchecked(Hash::new(
            b"terminal Decision CompleteTip successor context",
        )),
    );
    let activation = crate::sumeragi::v2_recovery::DurableSuccessorActivationAuthority::for_test(
        predecessor,
        successor_context_id,
    );
    crate::sumeragi::v2_recovery::RecoveredCompleteTipActivationAuthority::authenticate_for_test(
        artifact,
        receipt,
        successor_context_id,
        activation,
    )
    .expect("retain exact terminal Decision CompleteTip authority")
}

fn complete_tip_for_terminal_decision_at(
    fixture: &RecoveryFixture,
    projection: &TerminalDecisionProjectionFixture,
    predecessor_root: &Path,
) -> crate::sumeragi::v2_recovery::RecoveredCompleteTipActivationAuthority {
    let artifact = wire::finality::V2FinalityArtifact::new(
        fixture.verified.context().clone(),
        projection.subject.clone(),
        projection.certificate.clone(),
        fixture.verified.proofs_of_possession().to_vec(),
    );
    let receipt = crate::kura::KuraV2CommitReceipt::for_test(&artifact);
    let predecessor = crate::sumeragi::v2_recovery::DurableV2PredecessorIdentity::authenticate(
        &artifact, &receipt,
    )
    .expect("terminal Decision finality and receipt authenticate");
    let successor_context_id = wire::HeightContextId(
        iroha_crypto::HashOf::<wire::HeightContext>::from_untyped_unchecked(Hash::new(
            b"terminal Decision CompleteTip successor context",
        )),
    );
    let activation = crate::sumeragi::v2_recovery::DurableSuccessorActivationAuthority::for_test(
        predecessor,
        successor_context_id,
    );
    crate::sumeragi::v2_recovery::RecoveredCompleteTipActivationAuthority::authenticate_for_lifecycle_test(
                artifact,
                receipt,
                successor_context_id,
                activation,
                predecessor_root,
            )
            .expect("retain root-bound terminal Decision CompleteTip authority")
}

fn complete_tip_for_terminal_decision_on_kura(
    fixture: &RecoveryFixture,
    projection: &TerminalDecisionProjectionFixture,
    kura: &Kura,
) -> crate::sumeragi::v2_recovery::RecoveredCompleteTipActivationAuthority {
    complete_tip_for_terminal_decision_on_kura_with_policy(
        fixture,
        projection,
        kura,
        crate::sumeragi::v2_body_store::BlockSignaturePolicy::RotatingLeader,
    )
}

fn complete_tip_for_terminal_decision_on_kura_with_policy(
    fixture: &RecoveryFixture,
    projection: &TerminalDecisionProjectionFixture,
    kura: &Kura,
    predecessor_signature_policy: crate::sumeragi::v2_body_store::BlockSignaturePolicy,
) -> crate::sumeragi::v2_recovery::RecoveredCompleteTipActivationAuthority {
    let artifact = wire::finality::V2FinalityArtifact::new(
        fixture.verified.context().clone(),
        projection.subject.clone(),
        projection.certificate.clone(),
        fixture.verified.proofs_of_possession().to_vec(),
    );
    let receipt = crate::kura::KuraV2CommitReceipt::for_test(&artifact);
    let predecessor = crate::sumeragi::v2_recovery::DurableV2PredecessorIdentity::authenticate(
        &artifact, &receipt,
    )
    .expect("terminal Decision finality and receipt authenticate");
    let verified_successor = complete_tip_successor_fixture(fixture, projection);
    let successor_context_id = verified_successor.context().id();
    let activation = crate::sumeragi::v2_recovery::DurableSuccessorActivationAuthority::for_test(
        predecessor,
        successor_context_id,
    );
    crate::sumeragi::v2_recovery::RecoveredCompleteTipActivationAuthority::authenticate_for_canonical_lifecycle_test(
                artifact,
                receipt,
                fixture.verified.clone(),
                predecessor_signature_policy,
                successor_context_id,
                activation,
                kura,
            )
            .expect("retain Kura-bound terminal Decision CompleteTip authority")
}

fn complete_tip_successor_fixture(
    fixture: &RecoveryFixture,
    projection: &TerminalDecisionProjectionFixture,
) -> VerifiedHeightContext {
    let mut context = fixture.verified.context().clone();
    context.height = context
        .height
        .checked_add(1)
        .expect("fixture successor height is representable");
    context.parent_commit_qc = Some(projection.certificate.clone());
    context.snapshot_bootstrap = None;
    VerifiedHeightContext::successor_fixture_for_test(
        context,
        fixture.verified.proofs_of_possession().to_vec(),
        fixture.verified.context().clone(),
        fixture.verified.proofs_of_possession().to_vec(),
    )
}

fn successor_recovery_fixture(fixture: &RecoveryFixture) -> RecoveryFixture {
    let (_, projection) = terminal_decision_chain_fixture(fixture);
    RecoveryFixture {
        verified: complete_tip_successor_fixture(fixture, &projection),
        keys: fixture.keys.clone(),
    }
}

fn height_three_recovery_fixture(network: &str, first_seed: u8) -> RecoveryFixture {
    let height_one = RecoveryFixture::new(network, first_seed);
    let height_two = successor_recovery_fixture(&height_one);
    let height_three = successor_recovery_fixture(&height_two);
    assert_eq!(height_three.verified.context().height, 3);
    height_three
}

fn finalize_empty_lifecycle_floor_for_test(
    kura: &Kura,
    fixture: &RecoveryFixture,
    retained_high_water: u128,
) -> crate::sumeragi::v2::FinalizedLifecycleRetainedFloorV1 {
    let root = kura
        .sumeragi_v2_storage_root()
        .join("lifecycle-v1")
        .join(hex::encode(fixture.verified.context().id().0.as_ref()));
    let (store, opened) = LifecycleLedgerStoreV1::open(&root, fixture.lifecycle_context())
        .expect("open finalized-floor lifecycle store");
    let current = LifecycleLedgerV1::new(
        fixture.lifecycle_context(),
        retained_high_water,
        Vec::new(),
        BTreeMap::new(),
    )
    .expect("construct empty finalized-floor ledger");
    store
        .persist_exact_successor(&opened, &current)
        .expect("materialize finalized-floor ledger");
    let authority = authority::lifecycle_storage_owner_test_authority(&fixture.verified, 0, 0)
        .expect("construct finalized-floor lifecycle authority");
    let mut coordinator = LifecycleCoordinator::new_with_authority(authority, retained_high_water);
    coordinator.ledger_store = Some(store);
    let (_payload_store, recovered_payloads) =
        CertifiedServePayloadStoreV1::open_lifecycle_fixture_for_test(
            &root,
            fixture.verified.context(),
        )
        .expect("open empty finalized-floor Serve payload owner");
    let reconciliation = super::super::super::open::reconcile_complete_tip_serve_retirement(
        &current,
        recovered_payloads,
    )
    .expect("seal empty finalized-floor Serve census");
    let staged = current
        .stage_finalized_height_all_row_retirement(reconciliation)
        .expect("stage empty finalized-floor retirement");
    let publication = coordinator
        .persist_exact_finalization_successor(staged)
        .expect("publish empty finalized-floor retirement");
    let published = publication.consume_owners(LifecycleWorkRegistryHolder::empty());
    RecoveredLifecycleOwnerKuraBindingV1::for_test(kura, None)
        .bind_finalized_lifecycle_floor(published)
}

/// Build one genuinely retired CompleteTip/H+1 pair for the runner's
/// restart-activation boundary test.
pub(crate) fn complete_tip_restart_activation_fixture() -> (
    std::sync::Arc<Kura>,
    std::path::PathBuf,
    wire::HeightContext,
    RetiredRecoveredCompleteTipActivationAuthorityV1,
) {
    let fixture = RecoveryFixture::new("complete-tip-runner-restart", 0x48);
    let (predecessor, projection) = terminal_decision_chain_fixture(&fixture);
    let verified_successor = complete_tip_successor_fixture(&fixture, &projection);
    let successor_context = verified_successor.context().clone();
    let kura = Kura::blank_kura_for_testing();
    let predecessor_root = kura
        .sumeragi_v2_storage_root()
        .join("lifecycle-v1")
        .join(hex::encode(fixture.verified.context().id().0.as_ref()));
    let (predecessor_store, empty) =
        LifecycleLedgerStoreV1::open(&predecessor_root, fixture.lifecycle_context())
            .expect("open canonical runner-restart predecessor");
    assert!(empty.records().is_empty());
    predecessor_store
        .persist(&predecessor)
        .expect("persist runner-restart terminal predecessor");
    let retirement =
        complete_tip_for_terminal_decision_on_kura(&fixture, &projection, kura.as_ref())
            .into_canonical_predecessor_storage(&fixture.keys[0])
            .and_then(AuthenticatedCompleteTipPredecessorStorageV1::retire)
            .expect("retire the exact runner-restart predecessor");
    (kura, predecessor_root, successor_context, retirement)
}

fn empty_successor_owner_for_complete_tip(
    retirement: &RetiredRecoveredCompleteTipActivationAuthorityV1,
    kura: &Kura,
    verified: VerifiedHeightContext,
    body_root: &Path,
    payload_root: &Path,
    ledger_store: LifecycleLedgerStoreV1,
) -> ProductionLifecycleOwnerV1 {
    assert!(retirement.successor_ledger.records().is_empty());
    let body_store = V2BodyStore::open_lifecycle_fixture_for_test(
        body_root,
        verified.context().clone(),
        BlockSignaturePolicy::RotatingLeader,
    )
    .expect("open fixture H+1 body owner");
    let (payload_store, serve_payloads) =
        CertifiedServePayloadStoreV1::open_lifecycle_fixture_for_test(
            payload_root,
            verified.context(),
        )
        .expect("open fixture H+1 Serve-payload owner");
    let authority = authority::lifecycle_storage_owner_test_authority(&verified, 0, 0)
        .expect("construct empty H+1 lifecycle authority");
    let mut coordinator = LifecycleCoordinator::new_with_authority(
        authority,
        retirement.successor_ledger.high_water(),
    );
    coordinator.ledger_store = Some(ledger_store);
    ProductionLifecycleOwnerV1 {
        verified,
        coordinator,
        registry: LifecycleWorkRegistryHolder::empty(),
        recovered_lifecycle_outputs: None,
        payload_store,
        serve_payloads,
        body_store: Some(body_store),
        body_store_identity: None,
        kura_binding: Some(RecoveredLifecycleOwnerKuraBindingV1::for_test(kura, None)),
        apply_service: None,
        adapter_startup: Some(ProductionLifecycleAdapterStartupV1::fixture_for_test()),
        timeout_supersession_successor: None,
    }
}

fn unrelated_live_record(
    context: LifecycleContext,
    owner: OwnerId,
    ordinal: u128,
    seed: u8,
) -> LifecycleLedgerRecordV1 {
    let case = super::super::super::replay_authority::exact_record_fixture(
        context,
        LifecycleStageKind::SignPrepareVote,
        seed,
    );
    LifecycleLedgerRecordV1::new(
        case.key,
        owner,
        ordinal,
        case.work_class,
        case.stage,
        None,
        owner.causal_root().digest(),
        case.payload,
        case.authority,
        DurableContinuation::None,
    )
    .expect("construct unrelated live lifecycle row")
}

fn unrelated_terminal_record(
    context: LifecycleContext,
    owner: OwnerId,
    ordinal: u128,
    seed: u8,
) -> LifecycleLedgerRecordV1 {
    let case = super::super::super::replay_authority::exact_record_fixture(
        context,
        LifecycleStageKind::SignPrepareVote,
        seed,
    );
    LifecycleLedgerRecordV1::new(
        case.key,
        owner,
        ordinal,
        case.work_class,
        case.stage,
        Some(TerminalOutcome::Cancelled),
        owner.causal_root().digest(),
        case.payload,
        case.authority,
        DurableContinuation::None,
    )
    .expect("construct unrelated terminal lifecycle row")
}

#[test]
fn terminal_recovered_decision_oracle_accepts_the_exact_terminal_chain() {
    let fixture = RecoveryFixture::new("terminal-decision-exact", 0x31);
    let (ledger, projection) = terminal_decision_chain_fixture(&fixture);

    assert_eq!(
        ledger
            .authenticate_terminal_recovered_decision_apply_projection(&projection)
            .expect("authenticate exact terminal recovered Decision chain"),
        4
    );
}

#[test]
fn recovered_decision_store_crash_prefix_restarts_once_then_stutters() {
    let fixture = RecoveryFixture::new("decision-store-restart-stutter", 0x33);
    let (prefix, projection) = recovered_decision_store_crash_prefix_fixture(&fixture);

    let (successor, apply_ordinal, changed) = prefix
        .stage_recovered_decision_apply_projection(&projection)
        .expect("advance exact Fetch-to-Store crash prefix");
    assert!(changed);
    assert_eq!(apply_ordinal, 5);
    assert_eq!(successor.high_water(), 5);
    assert_eq!(successor.records().len(), 5);
    let store = &successor.records()[1];
    assert_eq!(store.ordinal(), 2);
    assert_eq!(
        store.continuation(),
        Some(DurableContinuation::successor(
            DurableContinuationEdge::StoreToValidate,
            4,
        ))
    );

    let (stutter, stutter_apply, stutter_changed) = successor
        .stage_recovered_decision_apply_projection(&projection)
        .expect("coalesce the already complete recovered Decision chain");
    assert!(!stutter_changed);
    assert_eq!(stutter_apply, apply_ordinal);
    assert_eq!(stutter, successor);
}

#[test]
fn recovered_decision_validate_crash_prefix_restarts_once_then_stutters() {
    let fixture = RecoveryFixture::new("decision-validate-restart-stutter", 0x35);
    let (_, projection) = recovered_decision_store_crash_prefix_fixture(&fixture);
    let prefix = projection.validate_crash_prefix.clone();

    let (successor, apply_ordinal, changed) = prefix
        .stage_recovered_decision_apply_projection(&projection)
        .expect("advance exact Fetch/Store-to-Validate crash prefix");
    assert!(changed);
    assert_eq!(apply_ordinal, 5);
    assert_eq!(successor.high_water(), 5);
    assert_eq!(successor.records().len(), 5);
    let store = successor
        .records()
        .iter()
        .find(|record| record.ordinal() == 2)
        .expect("retain exact advanced Store");
    let validate = successor
        .records()
        .iter()
        .find(|record| record.ordinal() == 4)
        .expect("retain exact advanced Validate");
    assert_eq!(
        store.continuation(),
        Some(DurableContinuation::successor(
            DurableContinuationEdge::StoreToValidate,
            4,
        ))
    );
    assert_eq!(validate.terminal(), Some(Some(TerminalOutcome::Advanced)));
    assert_eq!(
        validate.continuation(),
        Some(DurableContinuation::successor(
            DurableContinuationEdge::ValidateToApply,
            apply_ordinal,
        ))
    );

    let (stutter, stutter_apply, stutter_changed) = successor
        .stage_recovered_decision_apply_projection(&projection)
        .expect("coalesce the recovered Decision chain repaired from Validate");
    assert!(!stutter_changed);
    assert_eq!(stutter_apply, apply_ordinal);
    assert_eq!(stutter, successor);
}

#[test]
fn recovered_decision_apply_predecessor_oracle_follows_non_adjacent_validate_continuation() {
    let fixture = RecoveryFixture::new("decision-validate-predecessor-gap", 0x36);
    let (_, projection) = recovered_decision_store_crash_prefix_fixture(&fixture);
    let mut records = projection.validate_crash_prefix.records().to_vec();
    let intervening_root = CausalRoot::new(LifecycleDigest::new(
        *Hash::new(b"intervening actor-global row before recovered Decision Apply").as_ref(),
    ));
    records.push(unrelated_live_record(
        fixture.lifecycle_context(),
        OwnerId::new(intervening_root, 5),
        5,
        0xD9,
    ));
    let prefix = LifecycleLedgerV1::new(fixture.lifecycle_context(), 5, records, BTreeMap::new())
        .expect("construct recovered Validate prefix with an intervening actor-global row");

    let (successor, apply_ordinal, changed) = prefix
        .stage_recovered_decision_apply_projection(&projection)
        .expect("advance the non-adjacent recovered Validate predecessor");
    assert!(changed);
    assert_eq!(apply_ordinal, 6);
    assert_eq!(successor.high_water(), 6);
    assert_eq!(
        successor.recovered_decision_apply_validate_predecessor_ordinal_projection(
            &projection,
            apply_ordinal,
        ),
        Some(4),
        "the authenticated continuation, not Apply ordinal minus one, identifies Validate"
    );
    assert_eq!(
        successor
            .records()
            .iter()
            .find(|record| record.ordinal() == 5)
            .expect("retain the intervening actor-global row")
            .owner()
            .first_admission_ordinal(),
        5
    );
    assert_eq!(
        successor.recovered_decision_apply_validate_predecessor_ordinal_projection(
            &projection,
            apply_ordinal - 1,
        ),
        None,
        "a substituted Apply ordinal cannot authenticate predecessor retirement"
    );
}

#[test]
fn recovered_released_decision_apply_classifies_only_the_released_shape() {
    let fixture = RecoveryFixture::new("decision-released-classification", 0x36);
    let (released, projection) = recovered_released_decision_apply_fixture(&fixture);

    assert_eq!(
        released
            .classify_recovered_decision_apply_startup_projection(&projection)
            .expect("classify one exact released Validate tombstone"),
        RecoveredDecisionApplyStartupShapeV1::ReleasedTerminal
    );

    let ordinary = LifecycleLedgerV1::new(
        released.context(),
        projection.live_fetch.ordinal(),
        vec![projection.live_fetch.clone()],
        BTreeMap::new(),
    )
    .expect("construct ordinary current-Fetch startup shape");
    assert_eq!(
        ordinary
            .classify_recovered_decision_apply_startup_projection(&projection)
            .expect("classify the retained current Fetch"),
        RecoveredDecisionApplyStartupShapeV1::FullChain
    );
}

#[test]
fn recovered_released_decision_apply_appends_only_a_distinct_owned_apply() {
    let fixture = RecoveryFixture::new("decision-released-standalone-apply", 0x38);
    let (prefix, projection) = recovered_released_decision_apply_fixture(&fixture);
    let terminal_bytes = projection.released_terminal.encode();

    let (successor, apply_ordinal, changed) = prefix
        .stage_recovered_released_decision_apply_projection(&projection)
        .expect("stage one standalone recovered Decision Apply");

    assert!(changed);
    assert_eq!(apply_ordinal, 2);
    assert_eq!(successor.high_water(), apply_ordinal);
    assert_eq!(successor.records().len(), 2);
    let terminal = &successor.records()[0];
    let apply = &successor.records()[1];
    assert_eq!(terminal, &projection.released_terminal);
    assert_eq!(
        terminal.encode(),
        terminal_bytes,
        "standalone Apply staging must retain every tombstone byte"
    );
    assert_eq!(
        terminal.continuation(),
        Some(DurableContinuation::AdvancedNoSuccessor)
    );
    assert_eq!(terminal.terminal(), Some(Some(TerminalOutcome::Advanced)));
    assert!(
        projection
            .lineage
            .exactly_matches_standalone_apply_record(successor.context(), apply)
    );
    assert_eq!(apply.work_class(), Some(LifecycleWorkClass::Apply));
    assert_eq!(apply.terminal(), Some(None));
    assert_eq!(apply.continuation(), Some(DurableContinuation::None));
    assert_ne!(terminal.owner(), apply.owner());
    assert_ne!(
        terminal.owner().causal_root(),
        apply.owner().causal_root(),
        "the historical Validate owner cannot own the current Apply"
    );
    assert_eq!(
        apply.owner().first_admission_ordinal(),
        apply.ordinal(),
        "the standalone Apply must start its own owner lineage"
    );
    assert_eq!(
        successor
            .records()
            .iter()
            .filter(|record| record.work_class() == Some(LifecycleWorkClass::Fetch))
            .count(),
        0,
        "released recovery must not fabricate a current Fetch"
    );
    assert_eq!(
        successor
            .records()
            .iter()
            .filter(|record| record.work_class() == Some(LifecycleWorkClass::Store))
            .count(),
        0,
        "released recovery must not fabricate a current Store"
    );
    assert_eq!(
        successor
            .records()
            .iter()
            .filter(|record| record.work_class() == Some(LifecycleWorkClass::Validate))
            .count(),
        1,
        "only the historical Validate tombstone may remain"
    );
}

#[test]
fn recovered_released_decision_apply_exact_replay_is_a_stutter() {
    let fixture = RecoveryFixture::new("decision-released-exact-stutter", 0x3A);
    let (prefix, projection) = recovered_released_decision_apply_fixture(&fixture);
    let (successor, apply_ordinal, changed) = prefix
        .stage_recovered_released_decision_apply_projection(&projection)
        .expect("stage the initial standalone recovered Decision Apply");
    assert!(changed);
    let successor_bytes = successor.encode();

    let (stutter, stutter_ordinal, stutter_changed) = successor
        .stage_recovered_released_decision_apply_projection(&projection)
        .expect("coalesce the exact standalone Apply replay");

    assert!(!stutter_changed);
    assert_eq!(stutter_ordinal, apply_ordinal);
    assert_eq!(stutter, successor);
    assert_eq!(
        stutter.encode(),
        successor_bytes,
        "an exact replay must not change any durable ledger byte"
    );
    assert_eq!(
        stutter
            .classify_recovered_decision_apply_startup_projection(&projection)
            .expect("classify the coalesced standalone Apply"),
        RecoveredDecisionApplyStartupShapeV1::ReleasedTerminal
    );
}

#[test]
fn complete_tip_terminalizes_and_coalesces_a_released_standalone_apply() {
    let fixture = RecoveryFixture::new("complete-tip-released-standalone-apply", 0x3C);
    let (prefix, projection) = recovered_released_decision_apply_fixture(&fixture);
    let (live, apply_ordinal, changed) = prefix
        .stage_recovered_released_decision_apply_projection(&projection)
        .expect("stage the live standalone Apply crash window");
    assert!(changed);
    let released_terminal_bytes = projection.released_terminal.encode();
    let (_, finality_projection) = terminal_decision_chain_fixture_with_seed(&fixture, 0xD7);
    let complete_tip = complete_tip_for_terminal_decision(&fixture, &finality_projection);

    let (terminalized, terminalized_changed, evidence) = live
        .stage_complete_tip_terminal_apply_recovery(&complete_tip, None)
        .expect("terminalize the exact released standalone Apply");

    assert!(terminalized_changed);
    assert!(matches!(
        evidence,
        CompleteTipPredecessorLifecycleEvidenceV1::TerminalApply(ordinal)
            if ordinal == apply_ordinal
    ));
    assert_eq!(
        terminalized.records()[0].encode(),
        released_terminal_bytes,
        "CompleteTip recovery must not rewrite the released Validate tombstone"
    );
    let apply = terminalized
        .records()
        .iter()
        .find(|record| record.ordinal() == apply_ordinal)
        .expect("retain the standalone Apply row");
    assert_eq!(apply.owner().first_admission_ordinal(), apply_ordinal);
    assert_eq!(apply.terminal(), Some(Some(TerminalOutcome::Advanced)));
    assert_eq!(
        terminalized
            .authenticate_complete_tip_terminal_apply(&complete_tip)
            .expect("authenticate the released-terminal CompleteTip join"),
        apply_ordinal
    );
    let terminalized_bytes = terminalized.encode();

    let (stutter, stutter_changed, stutter_evidence) = terminalized
        .stage_complete_tip_terminal_apply_recovery(&complete_tip, None)
        .expect("coalesce the already-terminal standalone Apply");
    assert!(!stutter_changed);
    assert!(matches!(
        stutter_evidence,
        CompleteTipPredecessorLifecycleEvidenceV1::TerminalApply(ordinal)
            if ordinal == apply_ordinal
    ));
    assert_eq!(stutter, terminalized);
    assert_eq!(stutter.encode(), terminalized_bytes);
}

#[test]
fn recovered_decision_store_restart_rejects_an_exact_child_key_collision() {
    let fixture = RecoveryFixture::new("decision-store-key-collision", 0x37);
    let (prefix, projection) = recovered_decision_store_crash_prefix_fixture(&fixture);
    let collision = LifecycleLedgerV1::new(
        prefix.context(),
        3,
        vec![
            prefix.records()[0].clone(),
            prefix.records()[1].clone(),
            projection.collision_validate.clone(),
        ],
        BTreeMap::new(),
    )
    .expect("construct structurally valid foreign Validate-key collision");

    assert!(
        collision
            .stage_recovered_decision_apply_projection(&projection)
            .is_err(),
        "restart must not alias the exact recovered Validate key to another owner"
    );
    assert_eq!(collision.high_water(), 3);
    assert_eq!(collision.records().len(), 3);
}

#[test]
fn complete_tip_terminal_join_binds_the_full_finality_family() {
    let fixture = RecoveryFixture::new("complete-tip-terminal-exact", 0x41);
    let (ledger, projection) = terminal_decision_chain_fixture(&fixture);
    let complete_tip = complete_tip_for_terminal_decision(&fixture, &projection);

    assert_eq!(
        ledger
            .authenticate_complete_tip_terminal_apply(&complete_tip)
            .expect("join exact CompleteTip finality to terminal Apply"),
        4
    );

    let (foreign, _) = terminal_decision_chain_fixture_with_seed(&fixture, 0xE2);
    assert!(
        foreign
            .authenticate_complete_tip_terminal_apply(&complete_tip)
            .is_err(),
        "another canonical Decision certificate cannot enter the CompleteTip join"
    );
}

#[test]
fn complete_tip_terminal_join_rejects_foreign_apply_reconstruction_source() {
    let fixture = RecoveryFixture::new("complete-tip-terminal-source", 0x43);
    let (ledger, projection) = terminal_decision_chain_fixture(&fixture);
    let complete_tip = complete_tip_for_terminal_decision(&fixture, &projection);
    let mut records = ledger.records.clone();
    records[3].reconstruction_source = [0xFA; 32];
    assert!(matches!(
        LifecycleLedgerV1::new(
            ledger.context(),
            ledger.high_water(),
            records,
            BTreeMap::new(),
        ),
        Err(LifecycleLedgerError::InvalidLedger(_))
    ));
    assert_eq!(
        ledger
            .authenticate_complete_tip_terminal_apply(&complete_tip)
            .expect("the untouched terminal Apply retains the exact CompleteTip join"),
        4
    );
}

#[test]
fn complete_tip_terminal_apply_store_join_consumes_the_exact_opened_frame() {
    let fixture = RecoveryFixture::new("complete-tip-predecessor-cut", 0x45);
    let (ledger, projection) = terminal_decision_chain_fixture(&fixture);
    let kura = Kura::blank_kura_for_testing();
    let predecessor_root = kura
        .sumeragi_v2_storage_root()
        .join("lifecycle-v1")
        .join(hex::encode(fixture.verified.context().id().0.as_ref()));
    let complete_tip =
        complete_tip_for_terminal_decision_on_kura(&fixture, &projection, kura.as_ref());
    let (store, empty) =
        LifecycleLedgerStoreV1::open(&predecessor_root, fixture.lifecycle_context())
            .expect("open canonical CompleteTip predecessor store");
    assert!(empty.records().is_empty());
    store
        .persist(&ledger)
        .expect("persist terminal CompleteTip predecessor");

    assert!(
        complete_tip
            .into_canonical_predecessor_storage(&fixture.keys[0])
            .and_then(|cut| cut.is_exact())
            .is_ok_and(|exact| exact),
        "the capability must open its exact ledger, body, and Serve-payload owners"
    );
}

#[test]
fn empty_genesis_complete_tip_retires_without_a_synthetic_decision_chain() {
    let fixture = RecoveryFixture::new("empty-genesis-complete-tip", 0x95);
    let (_, projection) = terminal_decision_chain_fixture(&fixture);
    let kura = Kura::blank_kura_for_testing();
    let predecessor_root = kura
        .sumeragi_v2_storage_root()
        .join("lifecycle-v1")
        .join(hex::encode(fixture.verified.context().id().0.as_ref()));
    let (predecessor_store, empty) =
        LifecycleLedgerStoreV1::open(&predecessor_root, fixture.lifecycle_context())
            .expect("open canonical empty genesis predecessor store");
    assert_eq!(empty.high_water(), 0);
    assert!(empty.records().is_empty());
    let complete_tip = complete_tip_for_terminal_decision_on_kura_with_policy(
        &fixture,
        &projection,
        kura.as_ref(),
        crate::sumeragi::v2_body_store::BlockSignaturePolicy::GenesisAuthority(
            fixture.keys[0].public_key().clone(),
        ),
    );

    let retired = complete_tip
        .into_canonical_predecessor_storage(&fixture.keys[0])
        .and_then(AuthenticatedCompleteTipPredecessorStorageV1::retire)
        .expect("retire authenticated empty genesis predecessor");
    assert_eq!(retired.retained_high_water(), 0);
    assert!(retired.authorizes_retained_successor());
    let persisted = predecessor_store
        .load()
        .expect("reload durably retired empty genesis predecessor");
    assert_eq!(persisted.high_water(), 0);
    assert!(persisted.records().is_empty());
}

#[test]
fn present_empty_height_three_complete_tip_restart_initializes_then_stutters_successor() {
    let fixture = height_three_recovery_fixture("empty-height-three-complete-tip", 0x96);
    let (_, projection) = terminal_decision_chain_fixture(&fixture);
    let kura = Kura::blank_kura_for_testing();
    let predecessor_root = kura
        .sumeragi_v2_storage_root()
        .join("lifecycle-v1")
        .join(hex::encode(fixture.verified.context().id().0.as_ref()));
    let (predecessor_store, empty_predecessor) =
        LifecycleLedgerStoreV1::open(&predecessor_root, fixture.lifecycle_context())
            .expect("open canonical height-three lifecycle store");
    predecessor_store
        .persist_exact_successor(&empty_predecessor, &empty_predecessor)
        .expect("materialize the live height-three empty frame");
    let predecessor_bytes =
        fs::read(predecessor_root.join(LEDGER_FILE)).expect("read materialized H3 frame");

    let verified_successor = complete_tip_successor_fixture(&fixture, &projection);
    let successor_context = projection::lifecycle_context(verified_successor.context());
    let successor_root = kura
        .sumeragi_v2_storage_root()
        .join("lifecycle-v1")
        .join(hex::encode(verified_successor.context().id().0.as_ref()));
    assert!(
        !successor_root.join(LEDGER_FILE).exists(),
        "the first restart models a crash after H3 retirement but before H4 initialization"
    );

    let retired = complete_tip_for_terminal_decision_on_kura(&fixture, &projection, kura.as_ref())
        .into_canonical_predecessor_storage(&fixture.keys[0])
        .and_then(AuthenticatedCompleteTipPredecessorStorageV1::retire)
        .expect("cold restart authenticates empty H3 and initializes H4");
    let empty_successor = retired.successor_ledger.clone();
    assert_eq!(empty_successor.context(), successor_context);
    let successor_bytes =
        fs::read(successor_root.join(LEDGER_FILE)).expect("read initialized H4 frame");
    assert_eq!(
        retired.retained_high_water(),
        empty_predecessor.high_water()
    );
    assert!(retired.authorizes_retained_successor());
    assert_eq!(
        predecessor_store
            .load()
            .expect("reload exact empty H3 predecessor"),
        empty_predecessor
    );
    assert_eq!(retired.successor_ledger, empty_successor);

    let repeated = complete_tip_for_terminal_decision_on_kura(&fixture, &projection, kura.as_ref())
        .into_canonical_predecessor_storage(&fixture.keys[0])
        .and_then(AuthenticatedCompleteTipPredecessorStorageV1::retire)
        .expect("repeated cold restart stutters exact empty H3/H4 frames");
    assert_eq!(
        repeated.predecessor_frame_identity,
        retired.predecessor_frame_identity
    );
    assert_eq!(
        repeated.successor_frame_identity,
        retired.successor_frame_identity
    );
    assert_eq!(
        fs::read(predecessor_root.join(LEDGER_FILE)).expect("reread exact H3 frame"),
        predecessor_bytes,
        "cold retirement must not rewrite the physically present empty predecessor"
    );
    assert_eq!(
        fs::read(successor_root.join(LEDGER_FILE)).expect("reread exact H4 frame"),
        successor_bytes,
        "repeated cold retirement must preserve the already-initialized exact successor"
    );
}

#[test]
fn live_floor_two_initializes_height_three_and_first_admission_uses_ordinal_three() {
    let height_one = RecoveryFixture::new("live-floor-two-height-three", 0xB0);
    let height_two = successor_recovery_fixture(&height_one);
    let height_three = successor_recovery_fixture(&height_two);
    let (_, height_two_projection) = terminal_decision_chain_fixture(&height_two);
    let kura = Kura::blank_kura_for_testing();
    let floor = finalize_empty_lifecycle_floor_for_test(kura.as_ref(), &height_two, 2);
    let _storage = crate::sumeragi::v2::RecoveredLifecycleStorageAuthorityV1::for_test(
        kura.as_ref(),
        &height_three.verified,
        BlockSignaturePolicy::RotatingLeader,
        iroha_data_model::account::AccountId::new(height_three.keys[0].public_key().clone()),
    )
    .bind_finalized_predecessor_floor(floor)
    .expect("live H2 floor initializes exact H3 storage");

    let height_three_root = kura
        .sumeragi_v2_storage_root()
        .join("lifecycle-v1")
        .join(hex::encode(height_three.verified.context().id().0.as_ref()));
    let (height_three_store, initialized) =
        LifecycleLedgerStoreV1::open(&height_three_root, height_three.lifecycle_context())
            .expect("open initialized H3 ledger");
    assert_eq!(initialized.high_water(), 2);
    assert!(initialized.records().is_empty());

    let authority = authority::lifecycle_storage_owner_test_authority(&height_three.verified, 1, 0)
        .expect("construct H3 first-admission authority");
    let mut coordinator = LifecycleCoordinator::new_with_authority(authority, 2);
    coordinator.ledger_store = Some(height_three_store);
    let replay = super::super::super::replay_authority::exact_record_fixture(
        height_three.lifecycle_context(),
        LifecycleStageKind::SignPrepareVote,
        0xB1,
    );
    let causal_root = CausalRoot::new(LifecycleDigest::new(
        *Hash::new(b"first H3 admission above inherited floor").as_ref(),
    ));
    let candidate = CandidateAdmission::new(
        replay.key,
        causal_root,
        replay.work_class,
        replay.stage,
        InitialLifecycleState::Ready,
        causal_root.digest(),
        replay.payload,
        replay.authority,
        super::super::super::PhysicalGeometry::new([], []),
        None,
    );
    let super::super::super::AdmissionDecision::Admitted {
        ordinal,
        producer_turn_ordinal: None,
        ..
    } = coordinator.admit(super::super::super::AdmissionRequest::Candidate(candidate))
    else {
        panic!("first H3 work must admit above the inherited floor")
    };
    assert_eq!(ordinal, 3);
    let (_, persisted_height_three) =
        LifecycleLedgerStoreV1::open(&height_three_root, height_three.lifecycle_context())
            .expect("reopen H3 after first admission");
    assert_eq!(persisted_height_three.high_water(), 3);
    assert_eq!(persisted_height_three.records()[0].ordinal(), 3);

    let restarted = complete_tip_for_terminal_decision_on_kura(
        &height_two,
        &height_two_projection,
        kura.as_ref(),
    )
    .into_canonical_predecessor_storage(&height_two.keys[0])
    .and_then(AuthenticatedCompleteTipPredecessorStorageV1::retire)
    .expect("cold restart accepts H3 descendant above retained floor two");
    assert_eq!(restarted.retained_high_water(), 2);
    assert_eq!(restarted.successor_ledger.high_water(), 3);
    assert_eq!(restarted.successor_ledger.records()[0].ordinal(), 3);
}

#[test]
fn idle_rollover_retains_floor_two_across_preopen_crash_and_height_four() {
    let height_one = RecoveryFixture::new("idle-floor-two-rollover", 0xB4);
    let height_two = successor_recovery_fixture(&height_one);
    let height_three = successor_recovery_fixture(&height_two);
    let height_four = successor_recovery_fixture(&height_three);
    let (_, height_two_projection) = terminal_decision_chain_fixture(&height_two);
    let kura = Kura::blank_kura_for_testing();

    let _crash_cut = finalize_empty_lifecycle_floor_for_test(kura.as_ref(), &height_two, 2);
    let cold_h3 = complete_tip_for_terminal_decision_on_kura(
        &height_two,
        &height_two_projection,
        kura.as_ref(),
    )
    .into_canonical_predecessor_storage(&height_two.keys[0])
    .and_then(AuthenticatedCompleteTipPredecessorStorageV1::retire)
    .expect("restart after H2 retirement initializes missing H3 at floor two");
    assert_eq!(cold_h3.retained_high_water(), 2);
    assert_eq!(cold_h3.successor_ledger.high_water(), 2);
    assert!(cold_h3.successor_ledger.records().is_empty());

    let height_three_floor =
        finalize_empty_lifecycle_floor_for_test(kura.as_ref(), &height_three, 2);
    let _height_four_storage = crate::sumeragi::v2::RecoveredLifecycleStorageAuthorityV1::for_test(
        kura.as_ref(),
        &height_four.verified,
        BlockSignaturePolicy::RotatingLeader,
        iroha_data_model::account::AccountId::new(height_four.keys[0].public_key().clone()),
    )
    .bind_finalized_predecessor_floor(height_three_floor)
    .expect("idle H3 retirement carries inherited floor two into H4");
    let height_four_root = kura
        .sumeragi_v2_storage_root()
        .join("lifecycle-v1")
        .join(hex::encode(height_four.verified.context().id().0.as_ref()));
    let (_, initialized_height_four) =
        LifecycleLedgerStoreV1::open(&height_four_root, height_four.lifecycle_context())
            .expect("open initialized idle H4 ledger");
    assert_eq!(initialized_height_four.high_water(), 2);
    assert!(initialized_height_four.records().is_empty());
}

#[test]
fn live_retained_floor_rejects_lower_and_foreign_successor_storage() {
    let height_one = RecoveryFixture::new("live-floor-negative", 0xB8);
    let height_two = successor_recovery_fixture(&height_one);
    let height_three = successor_recovery_fixture(&height_two);
    let lower_kura = Kura::blank_kura_for_testing();
    let lower_floor = finalize_empty_lifecycle_floor_for_test(lower_kura.as_ref(), &height_two, 2);
    let lower_root = lower_kura
        .sumeragi_v2_storage_root()
        .join("lifecycle-v1")
        .join(hex::encode(height_three.verified.context().id().0.as_ref()));
    let (lower_store, empty) =
        LifecycleLedgerStoreV1::open(&lower_root, height_three.lifecycle_context())
            .expect("open lower-floor H3 store");
    let lower_owner = OwnerId::new(CausalRoot::new(LifecycleDigest::new([0xB9; 32])), 1);
    let lower = LifecycleLedgerV1::new(
        height_three.lifecycle_context(),
        1,
        vec![unrelated_live_record(
            height_three.lifecycle_context(),
            lower_owner,
            1,
            0xBA,
        )],
        BTreeMap::new(),
    )
    .expect("construct lower-floor H3 descendant");
    lower_store
        .persist_exact_successor(&empty, &lower)
        .expect("persist lower-floor H3 descendant");
    let lower_before = fs::read(lower_root.join(LEDGER_FILE)).expect("read lower-floor H3 frame");
    assert!(
        crate::sumeragi::v2::RecoveredLifecycleStorageAuthorityV1::for_test(
            lower_kura.as_ref(),
            &height_three.verified,
            BlockSignaturePolicy::RotatingLeader,
            iroha_data_model::account::AccountId::new(height_three.keys[0].public_key().clone(),),
        )
        .bind_finalized_predecessor_floor(lower_floor)
        .is_err(),
        "a nonempty H3 lineage at or below the retained floor must fail closed"
    );
    assert_eq!(
        fs::read(lower_root.join(LEDGER_FILE)).expect("reread lower-floor H3 frame"),
        lower_before,
        "lower-floor rejection must not rewrite the foreign successor"
    );

    let context_kura = Kura::blank_kura_for_testing();
    let context_floor =
        finalize_empty_lifecycle_floor_for_test(context_kura.as_ref(), &height_two, 2);
    let foreign_height_one = RecoveryFixture::new("live-floor-foreign-context", 0xBC);
    let foreign_height_two = successor_recovery_fixture(&foreign_height_one);
    let foreign_height_three = successor_recovery_fixture(&foreign_height_two);
    let foreign_context_root = context_kura
        .sumeragi_v2_storage_root()
        .join("lifecycle-v1")
        .join(hex::encode(
            foreign_height_three.verified.context().id().0.as_ref(),
        ));
    assert!(
        crate::sumeragi::v2::RecoveredLifecycleStorageAuthorityV1::for_test(
            context_kura.as_ref(),
            &foreign_height_three.verified,
            BlockSignaturePolicy::RotatingLeader,
            iroha_data_model::account::AccountId::new(
                foreign_height_three.keys[0].public_key().clone(),
            ),
        )
        .bind_finalized_predecessor_floor(context_floor)
        .is_err(),
        "a same-Kura H+1 seal for another predecessor context cannot consume the floor"
    );
    assert!(
        !foreign_context_root.join(LEDGER_FILE).exists(),
        "foreign-context rejection must occur before successor materialization"
    );

    let canonical_kura = Kura::blank_kura_for_testing();
    let foreign_kura = Kura::blank_kura_for_testing();
    let foreign_floor =
        finalize_empty_lifecycle_floor_for_test(canonical_kura.as_ref(), &height_two, 2);
    let foreign_root = foreign_kura
        .sumeragi_v2_storage_root()
        .join("lifecycle-v1")
        .join(hex::encode(height_three.verified.context().id().0.as_ref()));
    assert!(
        crate::sumeragi::v2::RecoveredLifecycleStorageAuthorityV1::for_test(
            foreign_kura.as_ref(),
            &height_three.verified,
            BlockSignaturePolicy::RotatingLeader,
            iroha_data_model::account::AccountId::new(height_three.keys[0].public_key().clone(),),
        )
        .bind_finalized_predecessor_floor(foreign_floor)
        .is_err(),
        "a finalized floor from another live Kura cannot initialize H3"
    );
    assert!(
        !foreign_root.join(LEDGER_FILE).exists(),
        "foreign-Kura rejection must occur before successor materialization"
    );

    let policy_kura = Kura::blank_kura_for_testing();
    let policy_floor =
        finalize_empty_lifecycle_floor_for_test(policy_kura.as_ref(), &height_two, 2);
    let policy_root = policy_kura
        .sumeragi_v2_storage_root()
        .join("lifecycle-v1")
        .join(hex::encode(height_three.verified.context().id().0.as_ref()));
    assert!(
        crate::sumeragi::v2::RecoveredLifecycleStorageAuthorityV1::for_test(
            policy_kura.as_ref(),
            &height_three.verified,
            BlockSignaturePolicy::GenesisAuthority(height_three.keys[0].public_key().clone(),),
            iroha_data_model::account::AccountId::new(height_three.keys[0].public_key().clone(),),
        )
        .bind_finalized_predecessor_floor(policy_floor)
        .is_err(),
        "post-genesis floor inheritance requires rotating-leader storage policy"
    );
    assert!(
        !policy_root.join(LEDGER_FILE).exists(),
        "wrong-policy rejection must occur before successor materialization"
    );
}

#[test]
fn non_genesis_complete_tip_rejects_a_missing_logical_empty_frame() {
    let fixture = height_three_recovery_fixture("missing-empty-height-three", 0x9A);
    let (_, projection) = terminal_decision_chain_fixture(&fixture);
    let kura = Kura::blank_kura_for_testing();
    let predecessor_root = kura
        .sumeragi_v2_storage_root()
        .join("lifecycle-v1")
        .join(hex::encode(fixture.verified.context().id().0.as_ref()));
    let (_store, logical_empty) =
        LifecycleLedgerStoreV1::open(&predecessor_root, fixture.lifecycle_context())
            .expect("open missing logical-empty predecessor");
    assert!(logical_empty.records().is_empty());
    assert!(!predecessor_root.join(LEDGER_FILE).exists());

    assert!(
        complete_tip_for_terminal_decision_on_kura(&fixture, &projection, kura.as_ref())
            .into_canonical_predecessor_storage(&fixture.keys[0])
            .is_err(),
        "Kura CompleteTip must not turn a missing frame into empty-retired authority"
    );
    assert!(
        !predecessor_root.join(LEDGER_FILE).exists(),
        "failed authentication must not materialize the missing predecessor"
    );
}

#[test]
fn empty_complete_tip_exception_rejects_wrong_policy_context_and_nonempty_ledger() {
    let fixture = RecoveryFixture::new("empty-genesis-complete-tip-negative", 0x99);
    let (_, projection) = terminal_decision_chain_fixture(&fixture);
    let rotating_kura = Kura::blank_kura_for_testing();
    let rotating =
        complete_tip_for_terminal_decision_on_kura(&fixture, &projection, rotating_kura.as_ref());
    assert!(!rotating.authorizes_empty_genesis_lifecycle(fixture.lifecycle_context()));

    let genesis_kura = Kura::blank_kura_for_testing();
    let genesis = complete_tip_for_terminal_decision_on_kura_with_policy(
        &fixture,
        &projection,
        genesis_kura.as_ref(),
        crate::sumeragi::v2_body_store::BlockSignaturePolicy::GenesisAuthority(
            fixture.keys[0].public_key().clone(),
        ),
    );
    assert!(
        !genesis.authorizes_empty_genesis_lifecycle(LifecycleContext::new(
            fixture.lifecycle_context().id(),
            2,
        ))
    );
    assert!(
        !genesis.authorizes_empty_genesis_lifecycle(LifecycleContext::new(
            LifecycleDigest::new([0xFF; 32]),
            1,
        ))
    );

    let predecessor_root = genesis_kura
        .sumeragi_v2_storage_root()
        .join("lifecycle-v1")
        .join(hex::encode(fixture.verified.context().id().0.as_ref()));
    let (store, empty) =
        LifecycleLedgerStoreV1::open(&predecessor_root, fixture.lifecycle_context())
            .expect("open malformed genesis predecessor store");
    let malformed = LifecycleLedgerV1::new(
        fixture.lifecycle_context(),
        1,
        vec![unrelated_live_record(
            fixture.lifecycle_context(),
            OwnerId::new(CausalRoot::new(LifecycleDigest::new([0xAB; 32])), 1),
            1,
            0xBC,
        )],
        BTreeMap::new(),
    )
    .expect("construct valid but nonempty genesis lifecycle ledger");
    store
        .persist_exact_successor(&empty, &malformed)
        .expect("persist nonempty genesis lifecycle ledger");
    assert!(
        genesis
            .into_canonical_predecessor_storage(&fixture.keys[0])
            .is_err(),
        "empty-genesis authority must not retire any nonempty malformed ledger"
    );
}

#[test]
fn empty_retired_frame_authority_rejects_foreign_path_context_and_digest_drift() {
    let fixture = height_three_recovery_fixture("empty-retired-frame-negative", 0x9E);
    let (_, projection) = terminal_decision_chain_fixture(&fixture);
    let kura = Kura::blank_kura_for_testing();
    let complete_tip =
        complete_tip_for_terminal_decision_on_kura(&fixture, &projection, kura.as_ref());
    let wrong_policy_kura = Kura::blank_kura_for_testing();
    let wrong_policy = complete_tip_for_terminal_decision_on_kura_with_policy(
        &fixture,
        &projection,
        wrong_policy_kura.as_ref(),
        crate::sumeragi::v2_body_store::BlockSignaturePolicy::GenesisAuthority(
            fixture.keys[0].public_key().clone(),
        ),
    );
    assert!(
        !wrong_policy.authorizes_retired_lifecycle(fixture.lifecycle_context()),
        "non-genesis frame retirement requires rotating-leader finality policy"
    );

    let foreign_directory = TempDir::new().expect("foreign empty-retired frame directory");
    let (foreign_store, foreign_empty) =
        LifecycleLedgerStoreV1::open(foreign_directory.path(), fixture.lifecycle_context())
            .expect("open foreign empty-retired store");
    foreign_store
        .persist_exact_successor(&foreign_empty, &foreign_empty)
        .expect("materialize foreign empty-retired frame");
    let foreign_presence = foreign_store
        .authenticate_present_frame(&foreign_empty)
        .expect("inspect foreign frame presence")
        .expect("foreign frame is physically present");
    assert!(
        foreign_empty
            .stage_complete_tip_terminal_apply_recovery(&complete_tip, Some(foreign_presence),)
            .is_err(),
        "a byte-identical empty frame at a foreign root must not enter recovery"
    );

    let canonical_root = kura
        .sumeragi_v2_storage_root()
        .join("lifecycle-v1")
        .join(hex::encode(fixture.verified.context().id().0.as_ref()));
    let wrong_context = LifecycleContext::new(
        LifecycleDigest::new([0xCE; 32]),
        fixture.lifecycle_context().height(),
    );
    let (wrong_context_store, wrong_context_empty) =
        LifecycleLedgerStoreV1::open(&canonical_root, wrong_context)
            .expect("open canonical path with a foreign context");
    wrong_context_store
        .persist_exact_successor(&wrong_context_empty, &wrong_context_empty)
        .expect("materialize wrong-context empty frame");
    let wrong_context_presence = wrong_context_store
        .authenticate_present_frame(&wrong_context_empty)
        .expect("inspect wrong-context frame presence")
        .expect("wrong-context frame is physically present");
    assert!(
        wrong_context_empty
            .stage_complete_tip_terminal_apply_recovery(
                &complete_tip,
                Some(wrong_context_presence),
            )
            .is_err(),
        "the canonical path cannot substitute a foreign lifecycle context"
    );

    let drift_fixture = height_three_recovery_fixture("empty-retired-frame-digest-drift", 0xA2);
    let (_, drift_projection) = terminal_decision_chain_fixture(&drift_fixture);
    let drift_kura = Kura::blank_kura_for_testing();
    let drift_complete_tip = complete_tip_for_terminal_decision_on_kura(
        &drift_fixture,
        &drift_projection,
        drift_kura.as_ref(),
    );
    let drift_root = drift_kura
        .sumeragi_v2_storage_root()
        .join("lifecycle-v1")
        .join(hex::encode(
            drift_fixture.verified.context().id().0.as_ref(),
        ));
    let (drift_store, drift_empty) =
        LifecycleLedgerStoreV1::open(&drift_root, drift_fixture.lifecycle_context())
            .expect("open digest-drift predecessor store");
    drift_store
        .persist_exact_successor(&drift_empty, &drift_empty)
        .expect("materialize digest-drift predecessor");
    let drift_presence = drift_store
        .authenticate_present_frame(&drift_empty)
        .expect("inspect digest-drift frame presence")
        .expect("digest-drift frame is physically present");
    let (staged, changed, evidence) = drift_empty
        .stage_complete_tip_terminal_apply_recovery(&drift_complete_tip, Some(drift_presence))
        .expect("seal exact empty-retired evidence before drift");
    assert!(!changed);
    let drifted = LifecycleLedgerV1::new(
        drift_fixture.lifecycle_context(),
        1,
        Vec::new(),
        BTreeMap::new(),
    )
    .expect("construct valid changed empty frame");
    drift_store
        .persist(&drifted)
        .expect("replace the frame after presence authentication");
    assert!(
        staged
            .into_complete_tip_terminal_apply_store_join(drift_store, drift_complete_tip, evidence,)
            .is_err(),
        "the move-only presence proof must fail after same-store frame drift"
    );
}

#[test]
fn canonical_complete_tip_retires_physical_nonempty_predecessor_without_apply() {
    let fixture = height_three_recovery_fixture("empty-retired-nonempty-negative", 0xA6);
    let (_, projection) = terminal_decision_chain_fixture(&fixture);
    let kura = Kura::blank_kura_for_testing();
    let predecessor_root = kura
        .sumeragi_v2_storage_root()
        .join("lifecycle-v1")
        .join(hex::encode(fixture.verified.context().id().0.as_ref()));
    let (store, empty) =
        LifecycleLedgerStoreV1::open(&predecessor_root, fixture.lifecycle_context())
            .expect("open nonempty predecessor-negative store");
    let unrelated_owner = OwnerId::new(CausalRoot::new(LifecycleDigest::new([0xA7; 32])), 1);
    let unrelated = LifecycleLedgerV1::new(
        fixture.lifecycle_context(),
        1,
        vec![unrelated_live_record(
            fixture.lifecycle_context(),
            unrelated_owner,
            1,
            0xA8,
        )],
        BTreeMap::new(),
    )
    .expect("construct nonempty predecessor without Decision Apply");
    store
        .persist_exact_successor(&empty, &unrelated)
        .expect("persist nonempty predecessor negative fixture");
    let before = fs::read(predecessor_root.join(LEDGER_FILE)).expect("read nonempty predecessor");
    let retired = complete_tip_for_terminal_decision_on_kura(&fixture, &projection, kura.as_ref())
        .into_canonical_predecessor_storage(&fixture.keys[0])
        .and_then(AuthenticatedCompleteTipPredecessorStorageV1::retire)
        .expect("canonical finality retires physically present unrelated local work");
    let reopened = store
        .load()
        .expect("reload canonically retired nonempty predecessor");
    assert_eq!(retired.retained_high_water(), unrelated.high_water());
    assert_eq!(reopened.records().len(), 1);
    assert_eq!(
        reopened.records()[0].terminal(),
        Some(Some(TerminalOutcome::Cancelled))
    );
    assert_ne!(
        fs::read(predecessor_root.join(LEDGER_FILE)).expect("reread retired predecessor"),
        before,
        "canonical retirement must publish the exact terminal successor frame"
    );
}

#[test]
fn empty_retired_complete_tip_rejects_a_foreign_successor_floor() {
    let fixture = height_three_recovery_fixture("empty-retired-successor-floor", 0xAA);
    let (_, projection) = terminal_decision_chain_fixture(&fixture);
    let kura = Kura::blank_kura_for_testing();
    let predecessor_root = kura
        .sumeragi_v2_storage_root()
        .join("lifecycle-v1")
        .join(hex::encode(fixture.verified.context().id().0.as_ref()));
    let (predecessor_store, empty_predecessor) =
        LifecycleLedgerStoreV1::open(&predecessor_root, fixture.lifecycle_context())
            .expect("open successor-floor predecessor");
    predecessor_store
        .persist_exact_successor(&empty_predecessor, &empty_predecessor)
        .expect("materialize successor-floor predecessor");
    let predecessor_before =
        fs::read(predecessor_root.join(LEDGER_FILE)).expect("read successor-floor predecessor");

    let verified_successor = complete_tip_successor_fixture(&fixture, &projection);
    let successor_context = projection::lifecycle_context(verified_successor.context());
    let successor_root = kura
        .sumeragi_v2_storage_root()
        .join("lifecycle-v1")
        .join(hex::encode(verified_successor.context().id().0.as_ref()));
    let (successor_store, logical_successor) =
        LifecycleLedgerStoreV1::open(&successor_root, successor_context)
            .expect("open foreign-floor successor");
    let foreign_floor = LifecycleLedgerV1::new(
        successor_context,
        empty_predecessor.high_water() + 1,
        Vec::new(),
        BTreeMap::new(),
    )
    .expect("construct empty successor at a foreign ordinal floor");
    successor_store
        .persist_exact_successor(&logical_successor, &foreign_floor)
        .expect("persist foreign successor floor");

    assert!(
        complete_tip_for_terminal_decision_on_kura(&fixture, &projection, kura.as_ref())
            .into_canonical_predecessor_storage(&fixture.keys[0])
            .and_then(AuthenticatedCompleteTipPredecessorStorageV1::retire)
            .is_err(),
        "empty-retired predecessor cannot bless a mismatched successor floor"
    );
    assert_eq!(
        fs::read(predecessor_root.join(LEDGER_FILE)).expect("reread successor-floor predecessor"),
        predecessor_before,
        "successor rejection must preserve the exact predecessor frame"
    );
}

#[test]
fn complete_tip_recovery_terminalizes_the_exact_live_apply_crash_window() {
    let fixture = RecoveryFixture::new("complete-tip-live-apply-recovery", 0xA5);
    let (terminal, projection) = terminal_decision_chain_fixture(&fixture);
    let mut live_records = terminal.records.clone();
    live_records[3].terminal = None;
    let live = LifecycleLedgerV1::new(
        terminal.context(),
        terminal.high_water(),
        live_records,
        BTreeMap::new(),
    )
    .expect("construct exact live Apply crash-window predecessor");
    let kura = Kura::blank_kura_for_testing();
    let predecessor_root = kura
        .sumeragi_v2_storage_root()
        .join("lifecycle-v1")
        .join(hex::encode(fixture.verified.context().id().0.as_ref()));
    let (predecessor_store, empty) =
        LifecycleLedgerStoreV1::open(&predecessor_root, fixture.lifecycle_context())
            .expect("open canonical live-Apply predecessor store");
    assert!(empty.records().is_empty());
    predecessor_store
        .persist(&live)
        .expect("persist live Apply crash-window predecessor");

    let complete_tip =
        complete_tip_for_terminal_decision_on_kura(&fixture, &projection, kura.as_ref());
    let retired = complete_tip
        .into_canonical_predecessor_storage(&fixture.keys[0])
        .and_then(AuthenticatedCompleteTipPredecessorStorageV1::retire)
        .expect("recover live Apply and retire its CompleteTip predecessor exactly");
    assert_eq!(retired.retained_high_water(), 4);
    let reopened = predecessor_store
        .load()
        .expect("reload recovered and retired CompleteTip predecessor");
    assert!(
        reopened
            .records()
            .iter()
            .all(|record| record.terminal().is_some_and(|terminal| terminal.is_some()))
    );
}

#[test]
fn complete_tip_live_apply_recovery_rejects_foreign_and_multiple_candidates() {
    let fixture = RecoveryFixture::new("complete-tip-live-apply-negative", 0xB1);
    let (terminal, projection) = terminal_decision_chain_fixture(&fixture);
    let mut live_records = terminal.records.clone();
    live_records[3].terminal = None;
    let live = LifecycleLedgerV1::new(
        terminal.context(),
        terminal.high_water(),
        live_records,
        BTreeMap::new(),
    )
    .expect("construct exact live Apply negative fixture");

    let foreign_fixture = RecoveryFixture::new("complete-tip-live-apply-foreign", 0xB9);
    let (_, foreign_projection) = terminal_decision_chain_fixture(&foreign_fixture);
    let foreign_complete_tip =
        complete_tip_for_terminal_decision(&foreign_fixture, &foreign_projection);
    assert!(
        live.stage_complete_tip_terminal_apply_recovery(&foreign_complete_tip, None)
            .is_err(),
        "foreign finality must not terminalize a live Apply"
    );

    let complete_tip = complete_tip_for_terminal_decision(&fixture, &projection);
    let mut multiple = live.clone();
    let mut duplicate = multiple.records[3].clone();
    duplicate.ordinal = 5;
    multiple.records.push(duplicate);
    multiple.high_water = 5;
    assert!(
        multiple
            .stage_complete_tip_terminal_apply_recovery(&complete_tip, None)
            .is_err(),
        "multiple matching live Apply rows must fail closed"
    );
}

#[test]
fn complete_tip_all_row_retirement_is_exact_and_restart_idempotent() {
    let fixture = RecoveryFixture::new("complete-tip-all-row-retirement", 0x46);
    let (terminal_chain, projection) = terminal_decision_chain_fixture(&fixture);
    let foreign_owner = OwnerId::new(CausalRoot::new(LifecycleDigest::new([0x46; 32])), 5);
    let mut records = terminal_chain.records.clone();
    records.push(unrelated_live_record(
        terminal_chain.context(),
        foreign_owner,
        5,
        0xE5,
    ));
    let predecessor = LifecycleLedgerV1::new(terminal_chain.context(), 5, records, BTreeMap::new())
        .expect("construct CompleteTip predecessor with unrelated live work");
    let kura = Kura::blank_kura_for_testing();
    let predecessor_root = kura
        .sumeragi_v2_storage_root()
        .join("lifecycle-v1")
        .join(hex::encode(fixture.verified.context().id().0.as_ref()));
    let (predecessor_store, empty) =
        LifecycleLedgerStoreV1::open(&predecessor_root, fixture.lifecycle_context())
            .expect("open canonical all-row predecessor store");
    assert!(empty.records().is_empty());
    predecessor_store
        .persist(&predecessor)
        .expect("persist all-row CompleteTip predecessor");

    let complete_tip =
        complete_tip_for_terminal_decision_on_kura(&fixture, &projection, kura.as_ref());
    let retired = complete_tip
        .into_canonical_predecessor_storage(&fixture.keys[0])
        .and_then(AuthenticatedCompleteTipPredecessorStorageV1::retire)
        .expect("retire every predecessor row and initialize successor");
    assert_eq!(retired.retained_high_water(), 5);
    let reopened_predecessor = predecessor_store
        .load()
        .expect("reload retired predecessor frame");
    assert!(reopened_predecessor.producer_debts().is_empty());
    assert!(
        reopened_predecessor
            .records()
            .iter()
            .all(|record| record.terminal().is_some_and(|terminal| terminal.is_some()))
    );
    assert_eq!(
        reopened_predecessor.records()[4].terminal(),
        Some(Some(TerminalOutcome::Cancelled))
    );
    assert_eq!(
        reopened_predecessor.records()[..4],
        terminal_chain.records()[..4],
        "the exact CompleteTip Decision tombstones remain byte-identical"
    );
    assert_eq!(
        retired.predecessor_frame_identity,
        reopened_predecessor.frame_identity()
    );

    let successor_context_id = retired.successor_context_id();
    let mut successor_context_bytes = [0_u8; 32];
    successor_context_bytes.copy_from_slice(successor_context_id.0.as_ref());
    let successor_context = LifecycleContext::new(
        LifecycleDigest::new(successor_context_bytes),
        fixture.lifecycle_context().height() + 1,
    );
    let successor_root = kura
        .sumeragi_v2_storage_root()
        .join("lifecycle-v1")
        .join(hex::encode(successor_context_id.0.as_ref()));
    let (successor_store, initialized_successor) =
        LifecycleLedgerStoreV1::open(&successor_root, successor_context)
            .expect("reopen initialized CompleteTip successor");
    assert!(initialized_successor.records().is_empty());
    assert!(initialized_successor.producer_debts().is_empty());
    assert_eq!(initialized_successor.high_water(), 5);
    assert_eq!(
        retired.successor_frame_identity,
        initialized_successor.frame_identity()
    );

    let descendant_owner = OwnerId::new(CausalRoot::new(LifecycleDigest::new([0x47; 32])), 6);
    let descendant = LifecycleLedgerV1::new(
        successor_context,
        6,
        vec![unrelated_live_record(
            successor_context,
            descendant_owner,
            6,
            0xE6,
        )],
        BTreeMap::new(),
    )
    .expect("construct later exact successor descendant");
    successor_store
        .persist_exact_successor(&initialized_successor, &descendant)
        .expect("publish later successor work above retained high-water");

    let retired_body_root = kura.sumeragi_v2_storage_root().join("bodies");
    std::fs::create_dir_all(&retired_body_root)
        .expect("materialize the obsolete predecessor body-owner root");
    std::fs::remove_dir_all(&retired_body_root)
        .expect("remove obsolete predecessor body owner after retirement");
    std::fs::write(
        &retired_body_root,
        b"retired body owner is no longer opened",
    )
    .expect("make any accidental predecessor body-store reopen fail");

    let repeated = complete_tip_for_terminal_decision_on_kura(&fixture, &projection, kura.as_ref())
        .into_canonical_predecessor_storage(&fixture.keys[0])
        .and_then(AuthenticatedCompleteTipPredecessorStorageV1::retire)
        .expect("exact retirement restart must stutter");
    assert_eq!(
        repeated.predecessor_frame_identity,
        retired.predecessor_frame_identity
    );
    assert_eq!(
        repeated.successor_frame_identity,
        descendant.frame_identity(),
        "restart must preserve a valid later successor without reopening obsolete predecessor bodies"
    );
    assert_eq!(repeated.predecessor(), retired.predecessor());
    assert_eq!(repeated.successor_context_id(), successor_context_id);
}

#[test]
/// Prove retirement binds only the exact unlaunched H+1 owner.
pub(crate) fn complete_tip_retirement_binds_only_the_exact_unlaunched_successor_owner() {
    let fixture = RecoveryFixture::new("complete-tip-successor-owner-bind", 0x49);
    let (predecessor, projection) = terminal_decision_chain_fixture(&fixture);
    let verified_successor = complete_tip_successor_fixture(&fixture, &projection);
    let kura = Kura::blank_kura_for_testing();
    let predecessor_root = kura
        .sumeragi_v2_storage_root()
        .join("lifecycle-v1")
        .join(hex::encode(fixture.verified.context().id().0.as_ref()));
    let (predecessor_store, empty) =
        LifecycleLedgerStoreV1::open(&predecessor_root, fixture.lifecycle_context())
            .expect("open canonical owner-binding predecessor");
    assert!(empty.records().is_empty());
    predecessor_store
        .persist(&predecessor)
        .expect("persist owner-binding predecessor");
    let retire = || {
        complete_tip_for_terminal_decision_on_kura(&fixture, &projection, kura.as_ref())
            .into_canonical_predecessor_storage(&fixture.keys[0])
            .and_then(AuthenticatedCompleteTipPredecessorStorageV1::retire)
            .expect("retire predecessor and authenticate exact H+1 target")
    };
    let body_root = kura.sumeragi_v2_storage_root().join("bodies");

    let retirement = retire();
    let foreign_root = TempDir::new().expect("foreign successor root");
    let (foreign_store, foreign_empty) =
        LifecycleLedgerStoreV1::open(foreign_root.path(), retirement.successor_ledger.context())
            .expect("open copied H+1 ledger target");
    assert!(foreign_empty.records().is_empty());
    foreign_store
        .persist(&retirement.successor_ledger)
        .expect("copy exact H+1 bytes to foreign target");
    let foreign_owner = empty_successor_owner_for_complete_tip(
        &retirement,
        kura.as_ref(),
        verified_successor.clone(),
        &body_root,
        foreign_root.path(),
        foreign_store,
    );
    assert!(
        retirement.bind_successor_owner(foreign_owner).is_err(),
        "byte-identical H+1 state at another publication target must fail closed"
    );

    let retirement = retire();
    let foreign_payload_root = TempDir::new().expect("foreign H+1 payload root");
    let foreign_payload_owner = empty_successor_owner_for_complete_tip(
        &retirement,
        kura.as_ref(),
        verified_successor.clone(),
        &body_root,
        foreign_payload_root.path(),
        retirement.successor_store.clone(),
    );
    assert!(
        retirement
            .bind_successor_owner(foreign_payload_owner)
            .is_err(),
        "the exact ledger cannot authorize a separately rooted Serve-payload owner"
    );

    let retirement = retire();
    let successor_root = retirement
        .successor_store
        .path
        .parent()
        .expect("canonical H+1 ledger has a parent root")
        .to_path_buf();
    let foreign_body_root = TempDir::new().expect("foreign H+1 body root");
    let foreign_body_owner = empty_successor_owner_for_complete_tip(
        &retirement,
        kura.as_ref(),
        verified_successor.clone(),
        foreign_body_root.path(),
        &successor_root,
        retirement.successor_store.clone(),
    );
    assert!(
        retirement.bind_successor_owner(foreign_body_owner).is_err(),
        "the exact ledger cannot authorize a separately rooted body owner"
    );

    let retirement = retire();
    let successor_root = retirement
        .successor_store
        .path
        .parent()
        .expect("canonical H+1 ledger has a parent root")
        .to_path_buf();
    let foreign_kura = Kura::blank_kura_for_testing();
    let foreign_kura_owner = empty_successor_owner_for_complete_tip(
        &retirement,
        foreign_kura.as_ref(),
        verified_successor.clone(),
        &body_root,
        &successor_root,
        retirement.successor_store.clone(),
    );
    assert!(
        retirement.bind_successor_owner(foreign_kura_owner).is_err(),
        "canonical H+1 storage cannot launch against another live Kura instance"
    );

    let retirement = retire();
    let successor_root = retirement
        .successor_store
        .path
        .parent()
        .expect("canonical H+1 ledger has a parent root")
        .to_path_buf();
    let exact_owner = empty_successor_owner_for_complete_tip(
        &retirement,
        kura.as_ref(),
        verified_successor,
        &body_root,
        &successor_root,
        retirement.successor_store.clone(),
    );
    let mut bound = retirement
        .bind_successor_owner(exact_owner)
        .expect("bind exact canonical unlaunched H+1 owner");
    assert!(bound.remains_exact_for_test());

    let old = bound.retirement.successor_ledger.clone();
    let next_ordinal = old
        .high_water()
        .checked_add(1)
        .expect("fixture H+1 ordinal remains representable");
    let owner = OwnerId::new(
        CausalRoot::new(LifecycleDigest::new([0x4A; 32])),
        next_ordinal,
    );
    let drifted = LifecycleLedgerV1::new(
        old.context(),
        next_ordinal,
        vec![unrelated_live_record(
            old.context(),
            owner,
            next_ordinal,
            0xEA,
        )],
        BTreeMap::new(),
    )
    .expect("construct post-bind H+1 storage drift");
    bound
        .retirement
        .successor_store
        .persist_exact_successor(&old, &drifted)
        .expect("publish test-only H+1 drift");
    assert!(
        !bound.remains_exact_for_test(),
        "the bound owner must detect canonical H+1 drift before launch"
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn complete_tip_nonempty_successor_consumes_only_the_exact_owner_open_witness() {
    let fixture = RecoveryFixture::new("complete-tip-nonempty-owner-open", 0x52);
    let (predecessor, projection) = terminal_decision_chain_fixture(&fixture);
    let verified_successor = complete_tip_successor_fixture(&fixture, &projection);
    let successor_context = projection::lifecycle_context(verified_successor.context());
    let kura = Kura::blank_kura_for_testing();
    let lifecycle_root = kura.sumeragi_v2_storage_root().join("lifecycle-v1");
    let predecessor_root =
        lifecycle_root.join(hex::encode(fixture.verified.context().id().0.as_ref()));
    let successor_root =
        lifecycle_root.join(hex::encode(verified_successor.context().id().0.as_ref()));
    let (predecessor_store, empty_predecessor) =
        LifecycleLedgerStoreV1::open(&predecessor_root, fixture.lifecycle_context())
            .expect("open nonempty-witness CompleteTip predecessor");
    assert!(empty_predecessor.records().is_empty());
    predecessor_store
        .persist(&predecessor)
        .expect("persist nonempty-witness CompleteTip predecessor");

    let first_ordinal = predecessor
        .high_water()
        .checked_add(1)
        .expect("successor ordinal is representable");
    let first_owner = OwnerId::new(
        CausalRoot::new(LifecycleDigest::new([0x53; 32])),
        first_ordinal,
    );
    let frozen = LifecycleLedgerV1::new(
        successor_context,
        first_ordinal,
        vec![unrelated_terminal_record(
            successor_context,
            first_owner,
            first_ordinal,
            0x54,
        )],
        BTreeMap::new(),
    )
    .expect("construct frozen nonempty CompleteTip successor");
    let second_ordinal = first_ordinal
        .checked_add(1)
        .expect("owner-open successor ordinal is representable");
    let second_owner = OwnerId::new(
        CausalRoot::new(LifecycleDigest::new([0x55; 32])),
        second_ordinal,
    );
    let post = LifecycleLedgerV1::new(
        successor_context,
        second_ordinal,
        vec![
            frozen.records()[0].clone(),
            unrelated_terminal_record(successor_context, second_owner, second_ordinal, 0x56),
        ],
        BTreeMap::new(),
    )
    .expect("construct exact owner-open successor frame");
    let third_ordinal = second_ordinal
        .checked_add(1)
        .expect("post-witness drift ordinal is representable");
    let third_owner = OwnerId::new(
        CausalRoot::new(LifecycleDigest::new([0x57; 32])),
        third_ordinal,
    );
    let drifted = LifecycleLedgerV1::new(
        successor_context,
        third_ordinal,
        vec![
            post.records()[0].clone(),
            post.records()[1].clone(),
            unrelated_terminal_record(successor_context, third_owner, third_ordinal, 0x58),
        ],
        BTreeMap::new(),
    )
    .expect("construct post-witness storage drift");
    let (successor_store, empty_successor) =
        LifecycleLedgerStoreV1::open(&successor_root, successor_context)
            .expect("open canonical nonempty CompleteTip successor");
    successor_store
        .persist_exact_successor(&empty_successor, &frozen)
        .expect("seed frozen nonempty CompleteTip successor");

    let retire = || {
        complete_tip_for_terminal_decision_on_kura(&fixture, &projection, kura.as_ref())
            .into_canonical_predecessor_storage(&fixture.keys[0])
            .and_then(AuthenticatedCompleteTipPredecessorStorageV1::retire)
            .expect("retire predecessor against the frozen nonempty successor")
    };
    let open_post_owner = || {
        let body_root = kura.sumeragi_v2_storage_root().join("bodies");
        let body_store = V2BodyStore::open_lifecycle_fixture_for_test(
            &body_root,
            verified_successor.context().clone(),
            BlockSignaturePolicy::RotatingLeader,
        )
        .expect("open exact CompleteTip successor body owner");
        let (payload_store, payloads) =
            CertifiedServePayloadStoreV1::open_lifecycle_fixture_for_test(
                &successor_root,
                verified_successor.context(),
            )
            .expect("open exact CompleteTip successor payload owner");
        let cut = post
            .clone()
            .into_durable_certified_body_pipeline_storage_recovery_cut(
                verified_successor.clone(),
                successor_store.clone(),
                body_store,
            )
            .expect("authenticate terminal post-frame storage census");
        let mut owner = cut
            .open_owner_for_test(payload_store, payloads)
            .expect("open exact terminal post-frame owner");
        owner.kura_binding = Some(RecoveredLifecycleOwnerKuraBindingV1::for_test(
            kura.as_ref(),
            None,
        ));
        owner
    };

    let retirement = retire();
    successor_store
        .persist_exact_successor(&frozen, &post)
        .expect("publish exact simulated owner-open successor");
    let mut exact_owner = open_post_owner();
    exact_owner.timeout_supersession_successor = Some(
        AuthenticatedRecoveredTimeoutSupersessionSuccessorV1::for_exact_store_successor_test(
            &successor_store,
            &frozen,
            &post,
        ),
    );
    let mut bound = retirement
        .bind_successor_owner(exact_owner)
        .expect("exact nonempty owner-open witness binds CompleteTip successor");
    assert!(bound.remains_exact_for_test());
    drop(bound);

    let repaired_bytes =
        fs::read(successor_root.join(LEDGER_FILE)).expect("read repaired nonempty successor frame");
    #[cfg(unix)]
    let repaired_inode = {
        use std::os::unix::fs::MetadataExt as _;
        fs::metadata(successor_root.join(LEDGER_FILE))
            .expect("inspect repaired nonempty successor frame")
            .ino()
    };
    let repeated = retire();
    let repeated_owner = open_post_owner();
    let repeated_bound = repeated
        .bind_successor_owner(repeated_owner)
        .expect("cold repaired successor stutters and binds without a new witness");
    drop(repeated_bound);
    assert_eq!(
        fs::read(successor_root.join(LEDGER_FILE)).expect("reread repaired successor frame"),
        repaired_bytes
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        assert_eq!(
            fs::metadata(successor_root.join(LEDGER_FILE))
                .expect("reinspect repaired nonempty successor frame")
                .ino(),
            repaired_inode,
            "cold exact repaired startup must not replace the successor frame"
        );
    }

    successor_store
        .persist_exact_successor(&post, &frozen)
        .expect("restore frozen frame for no-witness rejection");
    let no_witness_retirement = retire();
    successor_store
        .persist_exact_successor(&frozen, &post)
        .expect("republish post frame without witness");
    assert!(
        no_witness_retirement
            .bind_successor_owner(open_post_owner())
            .is_err(),
        "a valid nonempty descendant without exact owner-open proof remains rejected"
    );

    successor_store
        .persist_exact_successor(&post, &frozen)
        .expect("restore frozen frame for foreign-target rejection");
    let foreign_retirement = retire();
    successor_store
        .persist_exact_successor(&frozen, &post)
        .expect("republish canonical post frame");
    let foreign_root = TempDir::new().expect("foreign timeout-supersession witness target");
    let (foreign_store, foreign_empty) =
        LifecycleLedgerStoreV1::open(foreign_root.path(), successor_context)
            .expect("open foreign witness ledger target");
    foreign_store
        .persist_exact_successor(&foreign_empty, &post)
        .expect("copy post frame to foreign witness target");
    let mut foreign_owner = open_post_owner();
    foreign_owner.timeout_supersession_successor = Some(
        AuthenticatedRecoveredTimeoutSupersessionSuccessorV1::for_exact_store_successor_test(
            &foreign_store,
            &frozen,
            &post,
        ),
    );
    assert!(
        foreign_retirement
            .bind_successor_owner(foreign_owner)
            .is_err(),
        "a byte-identical witness from another publication target remains rejected"
    );

    successor_store
        .persist_exact_successor(&post, &frozen)
        .expect("restore frozen frame for foreign-context rejection");
    let context_retirement = retire();
    successor_store
        .persist_exact_successor(&frozen, &post)
        .expect("republish post frame for foreign-context rejection");
    let mut context_owner = open_post_owner();
    context_owner.timeout_supersession_successor = Some(
        AuthenticatedRecoveredTimeoutSupersessionSuccessorV1::for_exact_store_successor_test(
            &successor_store,
            &frozen,
            &post,
        )
        .with_context_for_test(LifecycleContext::new(
            LifecycleDigest::new([0x59; 32]),
            successor_context.height(),
        )),
    );
    assert!(
        context_retirement
            .bind_successor_owner(context_owner)
            .is_err(),
        "an otherwise exact witness from another lifecycle context remains rejected"
    );

    successor_store
        .persist_exact_successor(&post, &frozen)
        .expect("restore frozen frame for post-witness drift rejection");
    let drift_retirement = retire();
    successor_store
        .persist_exact_successor(&frozen, &post)
        .expect("republish exact witnessed post frame");
    let mut drift_owner = open_post_owner();
    drift_owner.timeout_supersession_successor = Some(
        AuthenticatedRecoveredTimeoutSupersessionSuccessorV1::for_exact_store_successor_test(
            &successor_store,
            &frozen,
            &post,
        ),
    );
    successor_store
        .persist_exact_successor(&post, &drifted)
        .expect("publish post-witness drift");
    assert!(
        drift_retirement.bind_successor_owner(drift_owner).is_err(),
        "storage drift after witness mint remains fail-closed"
    );
}

#[test]
fn complete_tip_all_row_retirement_consumes_pending_serve_terminal_update() {
    let fixture = RecoveryFixture::new("complete-tip-serve-retirement", 0x4C);
    let (terminal_chain, projection) = terminal_decision_chain_fixture(&fixture);
    let kura = Kura::blank_kura_for_testing();
    let predecessor_root = kura
        .sumeragi_v2_storage_root()
        .join("lifecycle-v1")
        .join(hex::encode(fixture.verified.context().id().0.as_ref()));
    let body_root = kura.sumeragi_v2_storage_root().join("bodies");
    let body_store = V2BodyStore::open(&body_root, fixture.verified.context().clone())
        .expect("open canonical CompleteTip body store");
    let request = fixture.authenticated_serve_request(0, 0x4C, 0);
    let (mut payload_store, recovered) =
        CertifiedServePayloadStoreV1::open(&predecessor_root, fixture.verified.context())
            .expect("open canonical CompleteTip Serve payload store");
    assert!(recovered.is_empty());
    let pending = payload_store
        .persist_pending_with_verified_retention(&fixture.verified, &fixture.keys[0], &request)
        .expect("persist retained CompleteTip Serve payload");
    let authority = authority::lifecycle_storage_owner_test_authority(&fixture.verified, 1, 1)
        .expect("construct CompleteTip Serve lifecycle authority");
    let mut coordinator = LifecycleCoordinator::new_with_authority(authority, 4);
    assert!(matches!(
        coordinator
            .admit_certified_serve(&fixture.verified, &request, pending)
            .expect("project retained CompleteTip Serve request"),
        super::super::super::AdmissionDecision::Admitted { ordinal: 5, .. }
    ));
    let serve_ledger = LifecycleLedgerV1::from_coordinator(&coordinator)
        .expect("project live CompleteTip Serve pair");
    assert_eq!(serve_ledger.records().len(), 2);
    assert_eq!(
        serve_ledger.producer_debts(),
        &[LifecycleProducerDebtV1::new(5, 6)]
    );

    let mut records = terminal_chain.records.clone();
    records.extend_from_slice(serve_ledger.records());
    let predecessor = LifecycleLedgerV1::new(
        terminal_chain.context(),
        6,
        records,
        BTreeMap::from([(5, 6)]),
    )
    .expect("join terminal Decision chain with live Serve pair");
    let (predecessor_store, empty) =
        LifecycleLedgerStoreV1::open(&predecessor_root, fixture.lifecycle_context())
            .expect("open canonical Serve predecessor ledger");
    assert!(empty.records().is_empty());
    predecessor_store
        .persist(&predecessor)
        .expect("persist live Serve predecessor ledger");
    drop(payload_store);
    drop(body_store);

    let _retired = complete_tip_for_terminal_decision_on_kura(&fixture, &projection, kura.as_ref())
        .into_canonical_predecessor_storage(&fixture.keys[0])
        .and_then(AuthenticatedCompleteTipPredecessorStorageV1::retire)
        .expect("retire exact Pending Serve and its ProducerTurn");

    let retired = predecessor_store
        .load()
        .expect("reload Serve-retired predecessor ledger");
    assert!(retired.producer_debts().is_empty());
    assert_eq!(
        retired.records()[4].terminal(),
        Some(Some(TerminalOutcome::Cancelled))
    );
    assert_eq!(
        retired.records()[5].terminal(),
        Some(Some(TerminalOutcome::Cancelled))
    );
    let reopened_body = V2BodyStore::open(&body_root, fixture.verified.context().clone())
        .expect("reopen canonical CompleteTip body store");
    let (reopened_payload_store, recovered) =
        CertifiedServePayloadStoreV1::open(&predecessor_root, fixture.verified.context())
            .expect("reopen retired CompleteTip Serve payload store");
    let authenticated = recovered
        .authenticate(&fixture.verified, &fixture.keys[0], &reopened_body)
        .expect("authenticate retired CompleteTip Serve payload cut");
    reopened_payload_store
        .validate_authenticated_cut(&authenticated)
        .expect("retired Serve payload cut remains exact");
    assert!(authenticated.iter().all(|payload| matches!(
                payload.state(),
                crate::sumeragi::v2_certified_serve_payload_store::AuthenticatedRecoveredCertifiedServePayloadState::Negative(
                    crate::sumeragi::v2_certified_serve_payload_store::CertifiedServePayloadNegativeOutcome::Cancelled,
                )
            )));
}

#[test]
/// Prove CompleteTip retirement survives normal predecessor-body cleanup.
pub(crate) fn complete_tip_retirement_survives_completed_serve_body_cleanup_with_live_work() {
    let fixture = RecoveryFixture::new("complete-tip-completed-serve-cleanup", 0x50);
    let (terminal_chain, projection) = terminal_decision_chain_fixture(&fixture);
    let kura = Kura::blank_kura_for_testing();
    let predecessor_root = kura
        .sumeragi_v2_storage_root()
        .join("lifecycle-v1")
        .join(hex::encode(fixture.verified.context().id().0.as_ref()));
    let body_root = kura.sumeragi_v2_storage_root().join("bodies");
    let mut body_store = V2BodyStore::open(&body_root, fixture.verified.context().clone())
        .expect("open canonical CompleteTip body store");
    let (request, durable_body, response) =
        fixture.completed_serve_exchange(&mut body_store, 0, 0x50, 3);
    let (mut payload_store, recovered) =
        CertifiedServePayloadStoreV1::open(&predecessor_root, fixture.verified.context())
            .expect("open canonical CompleteTip Serve payload store");
    assert!(recovered.is_empty());
    let pending = payload_store
        .persist_pending_with_verified_retention(&fixture.verified, &fixture.keys[0], &request)
        .expect("persist retained Completed-Serve request");

    let authority = authority::lifecycle_storage_owner_test_authority(&fixture.verified, 1, 1)
        .expect("construct Completed-Serve lifecycle authority");
    let mut coordinator = LifecycleCoordinator::new_with_authority(authority, 4);
    assert!(matches!(
        coordinator
            .admit_certified_serve(&fixture.verified, &request, pending)
            .expect("project retained Completed-Serve request"),
        super::super::super::AdmissionDecision::Admitted { ordinal: 5, .. }
    ));
    let pending_serve_ledger = LifecycleLedgerV1::from_coordinator(&coordinator)
        .expect("project the pre-completion Serve pair");
    let ready = coordinator.ready_index.iter().map(|ordinal| {
        let record = &coordinator.records[ordinal];
        (
            *ordinal,
            super::super::super::SchedulerReadyInputs::new(record, None, [0; 6]),
        )
    });
    let TurnPlan::Execute(lease) = coordinator.plan_turn(
        super::super::super::SchedulerInputs::new([], ready)
            .expect("Completed Serve is the sole Ready row"),
    ) else {
        panic!("Completed Serve must own the selected turn")
    };
    let completed = payload_store
        .persist_completed_with_exact_body(&request, &durable_body, &body_store, &response)
        .expect("persist exact Completed-Serve tombstone");
    let serve_ordinal = lease.ordinal();
    let producer_ordinal = coordinator.producer_debts[&serve_ordinal];
    let terminal = CertifiedServeTerminalReplayAuthorityPairV1::from_completed_receipt(
        coordinator.active_context,
        &coordinator.records[&serve_ordinal],
        &coordinator.durable_records[&serve_ordinal],
        &coordinator.records[&producer_ordinal],
        &coordinator.durable_records[&producer_ordinal],
        completed,
    )
    .expect("close exact Completed-Serve replay family");
    coordinator.reduce_settle_turn(
        lease,
        TurnOutcome::Terminal(terminal.terminal_outcome()),
        Some(terminal),
    );
    assert_eq!(coordinator.fault(), None);
    let serve_ledger = LifecycleLedgerV1::from_coordinator(&coordinator)
        .expect("project terminal Serve with live ProducerTurn");
    assert_eq!(serve_ledger.records().len(), 2);
    let response_digest =
        LifecycleDigest::new((*iroha_crypto::HashOf::new(&response).as_ref()).into());
    assert_eq!(
        serve_ledger.records()[0].terminal(),
        Some(Some(TerminalOutcome::Completed(Some(response_digest))))
    );
    assert_eq!(serve_ledger.records()[1].terminal(), Some(None));

    let unrelated_owner = OwnerId::new(CausalRoot::new(LifecycleDigest::new([0x51; 32])), 7);
    let mut records = terminal_chain.records.clone();
    records.extend_from_slice(serve_ledger.records());
    records.push(unrelated_live_record(
        terminal_chain.context(),
        unrelated_owner,
        7,
        0xF0,
    ));
    let predecessor = LifecycleLedgerV1::new(
        terminal_chain.context(),
        7,
        records,
        BTreeMap::from([(serve_ordinal, producer_ordinal)]),
    )
    .expect("join terminal Decision, Completed Serve, and unrelated live work");
    let (predecessor_store, empty) =
        LifecycleLedgerStoreV1::open(&predecessor_root, fixture.lifecycle_context())
            .expect("open canonical Completed-Serve predecessor ledger");
    assert!(empty.records().is_empty());
    predecessor_store
        .persist(&predecessor)
        .expect("persist Completed-Serve predecessor ledger");
    drop(payload_store);
    drop(body_store);
    std::fs::remove_dir_all(&body_root)
        .expect("simulate normal post-finality predecessor body cleanup");

    {
        let (_bodyless_payload_store, recovered) =
            CertifiedServePayloadStoreV1::open(&predecessor_root, fixture.verified.context())
                .expect("reopen Completed metadata after body cleanup");
        let bodyless = recovered
            .authenticate_for_complete_tip_retirement(&fixture.verified, &fixture.keys[0])
            .expect("authenticate retirement-only Completed metadata");
        assert!(
            super::super::super::open::authenticate_complete_tip_serve_census(
                &pending_serve_ledger,
                &bodyless,
            )
            .is_err(),
            "bodyless metadata must not promote a Pending Serve ledger row"
        );
    }

    let _retired = complete_tip_for_terminal_decision_on_kura(&fixture, &projection, kura.as_ref())
        .into_canonical_predecessor_storage(&fixture.keys[0])
        .and_then(AuthenticatedCompleteTipPredecessorStorageV1::retire)
        .expect("retire Completed Serve after body cleanup");

    let retired = predecessor_store
        .load()
        .expect("reload Completed-Serve retired predecessor");
    assert!(retired.producer_debts().is_empty());
    assert!(
        retired
            .records()
            .iter()
            .all(|record| { record.terminal().is_some_and(|terminal| terminal.is_some()) })
    );
    assert_eq!(retired.records()[4], serve_ledger.records()[0]);
    assert_eq!(
        retired.records()[5].terminal(),
        Some(Some(TerminalOutcome::Cancelled))
    );
    assert_eq!(
        retired.records()[6].terminal(),
        Some(Some(TerminalOutcome::Cancelled))
    );
    assert!(!body_root.exists());
    let (reopened_payload_store, recovered) =
        CertifiedServePayloadStoreV1::open(&predecessor_root, fixture.verified.context())
            .expect("reopen body-independent retired Serve payload store");
    let authenticated = recovered
        .authenticate_for_complete_tip_retirement(&fixture.verified, &fixture.keys[0])
        .expect("reauthenticate Completed metadata without body bytes");
    reopened_payload_store
        .validate_authenticated_cut(&authenticated)
        .expect("retired Completed payload cut remains exact");
    assert_eq!(authenticated.len(), 1);
}

#[test]
fn complete_tip_terminal_apply_store_join_detects_later_same_store_drift() {
    let fixture = RecoveryFixture::new("complete-tip-predecessor-later-drift", 0x47);
    let (ledger, projection) = terminal_decision_chain_fixture(&fixture);
    let directory = TempDir::new().expect("temporary later-drift predecessor ledger");
    let complete_tip =
        complete_tip_for_terminal_decision_at(&fixture, &projection, directory.path());
    let (store, empty) =
        LifecycleLedgerStoreV1::open(directory.path(), fixture.lifecycle_context())
            .expect("open later-drift predecessor store");
    store
        .persist(&ledger)
        .expect("persist terminal CompleteTip predecessor");
    let same_store_writer = store.clone();
    let apply_ordinal = ledger
        .authenticate_complete_tip_terminal_apply(&complete_tip)
        .expect("authenticate exact terminal Apply evidence");
    let cut = ledger
        .into_complete_tip_terminal_apply_store_join(
            store,
            complete_tip,
            CompleteTipPredecessorLifecycleEvidenceV1::TerminalApply(apply_ordinal),
        )
        .expect("authenticate exact predecessor before drift");
    same_store_writer
        .persist(&empty)
        .expect("replace predecessor after cut authentication");

    assert!(
        !cut.is_exact().expect("reload retained predecessor store"),
        "the retained cut must detect later writes through another handle"
    );
}

#[test]
fn complete_tip_terminal_apply_store_join_is_not_an_all_row_retirement() {
    let fixture = RecoveryFixture::new("complete-tip-predecessor-chain-local", 0x48);
    let (ledger, projection) = terminal_decision_chain_fixture(&fixture);
    let foreign_owner = OwnerId::new(CausalRoot::new(LifecycleDigest::new([0x48; 32])), 5);
    let mut records = ledger.records.clone();
    records.push(unrelated_live_record(
        ledger.context(),
        foreign_owner,
        5,
        0xE4,
    ));
    let chain_local = LifecycleLedgerV1::new(ledger.context(), 5, records, BTreeMap::new())
        .expect("construct predecessor with unrelated live work");
    let directory = TempDir::new().expect("temporary chain-local predecessor ledger");
    let complete_tip =
        complete_tip_for_terminal_decision_at(&fixture, &projection, directory.path());
    let (store, empty) =
        LifecycleLedgerStoreV1::open(directory.path(), fixture.lifecycle_context())
            .expect("open chain-local predecessor store");
    assert!(empty.records().is_empty());
    store
        .persist(&chain_local)
        .expect("persist chain-local predecessor");
    let apply_ordinal = chain_local
        .authenticate_complete_tip_terminal_apply(&complete_tip)
        .expect("authenticate chain-local terminal Apply evidence");

    assert!(
        chain_local
            .into_complete_tip_terminal_apply_store_join(
                store,
                complete_tip,
                CompleteTipPredecessorLifecycleEvidenceV1::TerminalApply(apply_ordinal),
            )
            .is_ok(),
        "this prerequisite must not masquerade as exhaustive retirement"
    );
}
