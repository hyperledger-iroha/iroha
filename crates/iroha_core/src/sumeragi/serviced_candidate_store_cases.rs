#[cfg(test)]
mod tests {
    use super::*;
    use crate::sumeragi::{
        FairV2IngressLeaderWireIdentity, FairV2IngressLeaderWirePhase,
        FairV2IngressLeaderWireSourceClass,
    };
    use tempfile::TempDir;
    const OWNER_A: [u8; 32] = [0xA1; 32];
    const OWNER_B: [u8; 32] = [0xB2; 32];
    fn safety_wal_identity(
        context: &wire::HeightContext,
        network_id: [u8; 32],
        key_hash: [u8; 32],
    ) -> super::super::v2_core::WalFileIdentity {
        super::super::v2_core::WalFileIdentity::new(
            wire::PROTOCOL_VERSION,
            network_id,
            super::super::v2_core::ContextId::new(*context.id().0.as_ref()),
            context.height,
            key_hash,
        )
    }
    fn context() -> wire::HeightContext {
        context_with_roster_len(4)
    }
    fn context_with_roster_len(roster_len: usize) -> wire::HeightContext {
        use iroha_crypto::{Algorithm, KeyPair};
        use iroha_data_model::{NetworkId, block::BlockHeader, peer::PeerId};
        assert!((4..=31).contains(&roster_len) && (roster_len - 1) % 3 == 0);
        let mut roster = (0..roster_len)
            .map(|index| {
                let seed = u8::try_from(index + 7).expect("bounded deterministic seed");
                let key = KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                    .expect("deterministic validator");
                wire::ValidatorPower {
                    validator: PeerId::new(key.public_key().clone()),
                    power: 1,
                }
            })
            .collect::<Vec<_>>();
        roster.sort_by(|left, right| left.validator.cmp(&right.validator));
        let network_id = NetworkId::from_genesis_hash(
            iroha_crypto::HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed(
                [0x95; Hash::LENGTH],
            )),
        );
        let (offline_cash_mint_finality_epoch_id, offline_cash_mint_finality_epoch_roster) =
            crate::offline_cash_v1_test_fixtures::mint_finality_roster_and_id(
                network_id, 1, &roster,
            );
        let context = wire::HeightContext {
            network_id,
            protocol_version: wire::PROTOCOL_VERSION,
            height: 7,
            epoch: 1,
            epoch_end_height: 100,
            next_epoch_snapshot: None,
            mode: wire::ConsensusMode::Permissioned,
            parent_commit_qc: None,
            snapshot_bootstrap: Some(wire::SnapshotBootstrapAnchor {
                snapshot_height: 6,
                snapshot_block_hash: iroha_crypto::HashOf::from_untyped_unchecked(Hash::new(
                    b"snapshot block",
                )),
                snapshot_block_creation_time_ms: 6_000,
                snapshot_state_hash: Hash::new(b"snapshot state"),
            }),
            quorum: wire::DualQuorum::from_roster(&roster).expect("quorum"),
            roster,
            offline_cash_mint_finality_epoch_id,
            offline_cash_mint_finality_epoch_roster,
            nexus_amx_context_hash: Hash::new(b"nexus"),
            execution_policy_hash: iroha_crypto::Hash::new(b"test execution policy"),
            da_layout: wire::DataAvailabilityLayout {
                encoding: wire::PayloadEncoding::ReedSolomon16,
                chunk_size_bytes: 1024,
                data_shards: 1,
                parity_shards: 1,
                max_payload_size_bytes: 4096,
                max_chunk_count: 8,
            },
            leader_seed: [9; 32],
        };
        context.validate().expect("valid snapshot-bound context");
        context
    }
    fn successor_context(predecessor: &wire::HeightContext) -> wire::HeightContext {
        let round = wire::ConsensusRound {
            context_id: predecessor.id(),
            height: predecessor.height,
            view: 0,
        };
        let parent = wire::QuorumCertificate {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Commit,
            subject: wire::BlockSubject {
                parent_block_hash: predecessor
                    .snapshot_bootstrap
                    .map(|anchor| anchor.snapshot_block_hash),
                block_hash: iroha_crypto::HashOf::from_untyped_unchecked(Hash::new(
                    b"predecessor block",
                )),
                payload_hash: Hash::new(b"predecessor payload"),
            },
            execution_commitment:
                wire::ExecutionCommitment::without_offline_cash_top_ups_or_merge_carrier(
                    Hash::new(b"predecessor parent state"),
                    Hash::new(b"predecessor post state"),
                    Hash::new(b"predecessor ordinary writes"),
                    1,
                    Hash::new(b"predecessor wire"),
                ),
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0xA7; 96],
        };
        parent
            .validate(predecessor)
            .expect("structurally quorum-valid predecessor CommitQC");
        let mut successor = predecessor.clone();
        successor.height = predecessor
            .height
            .checked_add(1)
            .expect("fixture height has a successor");
        successor.parent_commit_qc = Some(parent);
        successor.snapshot_bootstrap = None;
        successor.validate().expect("valid successor context");
        assert_ne!(successor.id(), predecessor.id());
        successor
    }
    fn leader_wire_recovery_authority(
        context: &wire::HeightContext,
    ) -> LeaderWireRecoveryAuthority {
        leader_wire_recovery_authority_at(context, OWNER_A, 0, false)
    }
    fn leader_wire_recovery_authority_at(
        context: &wire::HeightContext,
        owner: [u8; 32],
        durable_view: wire::View,
        decision_durable: bool,
    ) -> LeaderWireRecoveryAuthority {
        LeaderWireRecoveryAuthority::from_replayed_adapter(
            context.id(),
            context.height,
            owner,
            durable_view,
            decision_durable,
        )
    }
    fn key_with_kind(
        context: &wire::HeightContext,
        source_view: u64,
        evidence: u8,
        kind: u8,
    ) -> ServicedCandidateKey {
        ServicedCandidateKey::new(
            context.id(),
            context.height,
            OWNER_A,
            context.leader(source_view),
            source_view,
            Some([evidence; 32]),
            1,
            3,
            kind,
            [evidence; 32],
        )
    }
    fn key(context: &wire::HeightContext, source_view: u64, evidence: u8) -> ServicedCandidateKey {
        key_with_kind(context, source_view, evidence, 2)
    }
    fn candidate_kind_for_stage(stage: u8) -> u8 {
        match stage {
            0..=6 => stage,
            7 => 8,
            8 => 9,
            9 => 10,
            10 => 14,
            _ => panic!("test producer stage must be tracked"),
        }
    }
    fn state(
        store: &ServicedCandidateStore,
        records: Vec<PersistedServicedCandidate>,
        decision_reclaimed: bool,
    ) -> PersistedServicedCandidatesV4 {
        PersistedServicedCandidatesV4 {
            format_version: FORMAT_VERSION,
            context_id: store.context_id,
            height: store.height,
            owner: store.owner,
            serviced_capacity: u64::try_from(store.serviced_capacity)
                .expect("test serviced capacity fits u64"),
            producer_continuation_capacity: u64::try_from(store.producer_continuation_capacity)
                .expect("test producer-continuation capacity fits u64"),
            decision_reclaimed,
            records,
            producer_continuations: Vec::new(),
        }
    }
    fn continuation_identity(
        context: &wire::HeightContext,
        lifecycle_slot: u64,
        admission_ordinal: u128,
        stage: u8,
        evidence: u8,
    ) -> ProducerContinuationIdentity {
        ProducerContinuationIdentity::new(
            key_with_kind(context, 2, evidence, candidate_kind_for_stage(stage)),
            Hash::new([0xC1, evidence]),
            lifecycle_slot,
            admission_ordinal,
        )
        .expect("valid producer-continuation identity")
    }
    fn continuation_record(
        context: &wire::HeightContext,
        lifecycle_slot: u64,
        admission_ordinal: u128,
        stage: u8,
        status: ProducerContinuationStatus,
        handoff_stages: &[u8],
    ) -> ProducerContinuationRecord {
        let identity =
            continuation_identity(context, lifecycle_slot, admission_ordinal, stage, stage + 1);
        let mut handoff_candidates = handoff_stages
            .iter()
            .map(|successor_stage| {
                ProducerContinuationIdentity::new(
                    key_with_kind(
                        context,
                        2,
                        successor_stage + 32,
                        candidate_kind_for_stage(*successor_stage),
                    ),
                    identity.causal_lifecycle_key,
                    lifecycle_slot,
                    admission_ordinal,
                )
                .expect("valid exact successor identity")
            })
            .collect::<Vec<_>>();
        handoff_candidates.sort_unstable();
        ProducerContinuationRecord::new(identity, status, handoff_candidates)
            .expect("valid producer-continuation record")
    }
    fn leader_wire_token(
        context: &wire::HeightContext,
        view: wire::View,
        admission_ordinal: u64,
        scheduler_ordinal: u128,
        discriminator: u8,
    ) -> FairV2IngressLeaderWireToken {
        let origin = context.roster[0].validator.clone();
        let phase = FairV2IngressLeaderWirePhase::PrepareVote;
        FairV2IngressLeaderWireToken {
            identity: FairV2IngressLeaderWireIdentity {
                context_id: context.id(),
                height: context.height,
                view,
                subject_hash: Hash::new([0x51, discriminator]),
                manifest_hash: None,
                phase,
                semantic_origin: origin.clone(),
                canonical_wire_hash: Hash::new([0x52, discriminator]),
            },
            slot: FairV2IngressLeaderWireSlot {
                semantic_origin: origin,
                phase,
                chunk_index: None,
            },
            admission_ordinal,
            scheduler_ordinal,
            source_class: FairV2IngressLeaderWireSourceClass::Control,
        }
    }
    fn leader_wire_slot_token(
        context: &wire::HeightContext,
        origin: &PeerId,
        phase: FairV2IngressLeaderWirePhase,
        chunk_index: Option<u32>,
        admission_ordinal: u64,
        scheduler_ordinal: u128,
    ) -> FairV2IngressLeaderWireToken {
        let manifest_hash = matches!(
            phase,
            FairV2IngressLeaderWirePhase::Proposal
                | FairV2IngressLeaderWirePhase::Chunk
                | FairV2IngressLeaderWirePhase::CertifiedResponse
        )
        .then(|| Hash::new(b"shared leader-wire manifest"));
        FairV2IngressLeaderWireToken {
            identity: FairV2IngressLeaderWireIdentity {
                context_id: context.id(),
                height: context.height,
                view: 2,
                subject_hash: Hash::new(b"shared leader-wire subject"),
                manifest_hash,
                phase,
                semantic_origin: origin.clone(),
                canonical_wire_hash: Hash::new(b"shared leader-wire bytes"),
            },
            slot: FairV2IngressLeaderWireSlot {
                semantic_origin: origin.clone(),
                phase,
                chunk_index,
            },
            admission_ordinal,
            scheduler_ordinal,
            source_class: phase.source_class(),
        }
    }
    fn leader_wire_body_token(
        context: &wire::HeightContext,
        receipt: &DurableBodyReceipt,
        admission_ordinal: u64,
        scheduler_ordinal: u128,
    ) -> FairV2IngressLeaderWireToken {
        let origin = context.roster[0].validator.clone();
        let phase = FairV2IngressLeaderWirePhase::CertifiedResponse;
        FairV2IngressLeaderWireToken {
            identity: FairV2IngressLeaderWireIdentity {
                context_id: context.id(),
                height: context.height,
                view: receipt.round().view,
                subject_hash: Hash::new(receipt.subject().encode()),
                manifest_hash: Some(receipt.manifest_hash().into()),
                phase,
                semantic_origin: origin.clone(),
                canonical_wire_hash: Hash::new(b"durable body terminal response"),
            },
            slot: FairV2IngressLeaderWireSlot {
                semantic_origin: origin,
                phase,
                chunk_index: None,
            },
            admission_ordinal,
            scheduler_ordinal,
            source_class: FairV2IngressLeaderWireSourceClass::CertifiedResponse,
        }
    }
    fn matching_terminal(
        context: &wire::HeightContext,
        runtime_owner: LeaderWireRuntimeOwner,
        token: &FairV2IngressLeaderWireToken,
    ) -> ProducerContinuationTerminalToken {
        let (kind, phase) = match token.identity.phase {
            FairV2IngressLeaderWirePhase::Proposal => (1, 0),
            FairV2IngressLeaderWirePhase::PrepareVote => (2, 1),
            FairV2IngressLeaderWirePhase::CommitVote => (2, 2),
            FairV2IngressLeaderWirePhase::PrepareQc => (3, 1),
            FairV2IngressLeaderWirePhase::CommitQc => (3, 2),
            FairV2IngressLeaderWirePhase::TimeoutVote => (4, 3),
            FairV2IngressLeaderWirePhase::TimeoutCertificate => (5, 3),
            FairV2IngressLeaderWirePhase::Chunk
            | FairV2IngressLeaderWirePhase::CertifiedResponse => {
                panic!("body transport cannot mint a producer terminal fixture")
            }
        };
        let candidate = ServicedCandidateKey::new(
            context.id(),
            context.height,
            OWNER_A,
            context.leader(token.identity.view),
            token.identity.view,
            Some([0xD1; 32]),
            phase,
            3,
            kind,
            [0xD1; 32],
        );
        let identity = ProducerContinuationIdentity::new(
            candidate,
            runtime_owner.causal_lifecycle_key(),
            1,
            runtime_owner.admission_ordinal(),
        )
        .expect("matching producer identity");
        ProducerContinuationRecord::new(identity, ProducerContinuationStatus::Terminal, Vec::new())
            .expect("matching producer terminal")
            .terminal_token()
            .expect("terminal token")
    }
    fn terminal_continuation_at_view(
        context: &wire::HeightContext,
        lifecycle_slot: u64,
        admission_ordinal: u128,
        stage: u8,
        source_view: wire::View,
        evidence: u8,
    ) -> ProducerContinuationRecord {
        let identity = ProducerContinuationIdentity::new(
            key_with_kind(
                context,
                source_view,
                evidence,
                candidate_kind_for_stage(stage),
            ),
            Hash::new([0xD2, evidence]),
            lifecycle_slot,
            admission_ordinal,
        )
        .expect("valid terminal continuation identity");
        ProducerContinuationRecord::new(identity, ProducerContinuationStatus::Terminal, Vec::new())
            .expect("valid terminal continuation")
    }
    fn write_frame(store: &ServicedCandidateStore, state: &PersistedServicedCandidatesV4) {
        let frame = encode_frame_v4(state, store.max_frame_bytes).expect("encode fixture frame");
        fs::write(store.path_for_test(), frame).expect("write fixture frame");
    }
    #[test]
    fn snapshot_roundtrips_and_rejects_a_b_a_resurrection() {
        let directory = TempDir::new().expect("temporary directory");
        let context = context();
        let wal = directory.path().join("00000000000000000007.wal");
        let (store, restored) =
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 4)
                .expect("open snapshot");
        assert!(restored.records.is_empty());
        let a = key(&context, 2, 1);
        let b = key(&context, 2, 2);
        assert_eq!(a.class(), 3);
        let service_view = 5;
        let mut records = BTreeMap::from([(a, service_view), (b, service_view)]);
        store.persist(&records, false).expect("persist A and B");
        assert_eq!(
            records.insert(a, service_view),
            Some(service_view),
            "A remains serviced after equal-rank B replacement"
        );
        let (_reopened, restored) =
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 4)
                .expect("same-height reopen");
        assert_eq!(restored.records, records);
    }
    #[cfg(all(unix, not(target_os = "espidf")))]
    #[test]
    fn serviced_candidate_recovery_rejects_substituted_wal_directory() {
        use std::os::unix::fs::symlink;
        const NETWORK: [u8; 32] = [0x41; 32];
        const KEY: [u8; 32] = [0x42; 32];
        let context = context();
        let seed = TempDir::new().expect("seed directory");
        let seed_wal = seed.path().join("wal").join("00000000000000000007.wal");
        let seed_safety = super::super::safety_wal::SafetyWal::open(
            &seed_wal,
            safety_wal_identity(&context, NETWORK, KEY),
        )
        .expect("open seed safety WAL");
        let seed_authority = seed_safety
            .mint_serviced_candidate_store_authority(&seed_wal)
            .expect("mint seed serviced-candidate authority");
        let (seed_store, _) = ServicedCandidateStore::open_with_safety_wal_authority(
            seed_authority,
            context.id(),
            context.height,
            OWNER_A,
            4,
        )
        .expect("open seed serviced-candidate store");
        let recovered_key = key(&context, 2, 0x55);
        seed_store
            .persist(&BTreeMap::from([(recovered_key, 9)]), false)
            .expect("publish seed recovered state");
        let injected_frame = fs::read(seed_store.path_for_test()).expect("read seed frame");
        let target = TempDir::new().expect("target directory");
        let target_parent = target.path().join("wal");
        let target_wal = target_parent.join("00000000000000000007.wal");
        let target_safety = super::super::safety_wal::SafetyWal::open(
            &target_wal,
            safety_wal_identity(&context, NETWORK, KEY),
        )
        .expect("open target safety WAL");
        let target_authority = target_safety
            .mint_serviced_candidate_store_authority(&target_wal)
            .expect("mint target serviced-candidate authority");
        let retained = target.path().join("retained-wal");
        let foreign = target.path().join("foreign-wal");
        fs::rename(&target_parent, &retained).expect("move bound WAL directory");
        fs::create_dir(&foreign).expect("create foreign WAL directory");
        let adjacent_name = "00000000000000000007.wal.serviced-candidates";
        fs::write(foreign.join(adjacent_name), &injected_frame).expect("inject foreign snapshot");
        symlink(&foreign, &target_parent).expect("substitute WAL directory");
        assert!(
            ServicedCandidateStore::open_with_safety_wal_authority(
                target_authority,
                context.id(),
                context.height,
                OWNER_A,
                4,
            )
            .is_err()
        );
        assert!(!retained.join(adjacent_name).exists());
        assert_eq!(
            fs::read(foreign.join(adjacent_name)).expect("foreign frame remains untouched"),
            injected_frame
        );
    }
    #[test]
    fn v4_roundtrips_terminal_producer_continuations() {
        let directory = TempDir::new().expect("temporary directory");
        let context = context();
        let wal = directory.path().join("continuations.wal");
        let (store, _) =
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 4)
                .expect("open v4 snapshot");
        let first_terminal =
            continuation_record(&context, 1, 1, 1, ProducerContinuationStatus::Terminal, &[]);
        let terminal =
            continuation_record(&context, 2, 2, 3, ProducerContinuationStatus::Terminal, &[]);
        let producer_continuations = BTreeMap::from([
            (first_terminal.identity.address(), first_terminal),
            (terminal.identity.address(), terminal),
        ]);
        let serviced = producer_continuations
            .values()
            .map(|record| {
                let candidate = record.identity().candidate();
                (candidate, candidate.source_view())
            })
            .collect::<BTreeMap<_, _>>();
        assert!(
            store
                .persist_with_producer_continuations(
                    &BTreeMap::new(),
                    &producer_continuations,
                    false,
                )
                .is_err(),
            "a terminal producer cannot outlive its durable service tombstone"
        );
        store
            .persist_with_producer_continuations(&serviced, &producer_continuations, false)
            .expect("persist v4 producer continuations");
        let (_, restored) =
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 4)
                .expect("restore v4 producer continuations");
        assert_eq!(restored.records, serviced);
        assert_eq!(restored.producer_continuations, producer_continuations);
        let active =
            continuation_record(&context, 3, 3, 4, ProducerContinuationStatus::Reserved, &[]);
        let active_table = BTreeMap::from([(active.identity.address(), active.clone())]);
        store
            .persist_with_producer_continuations(&serviced, &active_table, false)
            .expect("persist exact active admission metadata");
        let (_, active_restored) =
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 4)
                .expect("restore active producer admission metadata");
        assert_eq!(active_restored.producer_continuations, active_table);
        assert_eq!(
            active_restored.producer_continuations[&active.identity.address()].status(),
            ProducerContinuationStatus::Reserved
        );
        let materialized = continuation_record(
            &context,
            4,
            4,
            1,
            ProducerContinuationStatus::Materialized,
            &[2],
        );
        store
            .persist_with_producer_continuations(
                &serviced,
                &BTreeMap::from([(materialized.identity.address(), materialized.clone())]),
                false,
            )
            .expect("persist materialized admission metadata");
        let (_, materialized_restored) =
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 4)
                .expect("restore materialized producer metadata");
        let reopened =
            &materialized_restored.producer_continuations[&materialized.identity.address()];
        assert_eq!(reopened.status(), ProducerContinuationStatus::Reserved);
        assert!(reopened.handoff_candidates.is_empty());
    }
    #[cfg(all(unix, not(target_os = "espidf")))]
    #[test]
    fn leader_wire_gate_rejects_substituted_wal_directory() {
        use std::os::unix::fs::symlink;
        let context = context();
        let root = TempDir::new().expect("leader-wire target directory");
        let parent = root.path().join("wal");
        let wal_path = parent.join("00000000000000000007.wal");
        let wal = super::super::safety_wal::SafetyWal::open(
            &wal_path,
            safety_wal_identity(&context, [0x61; 32], [0x62; 32]),
        )
        .expect("open leader-wire safety WAL");
        let storage = wal
            .mint_leader_wire_store_authority(&wal_path)
            .expect("mint leader-wire storage authority");
        let retained = root.path().join("retained-wal");
        let foreign = root.path().join("foreign-wal");
        fs::rename(&parent, &retained).expect("move bound WAL directory");
        fs::create_dir(&foreign).expect("create foreign WAL directory");
        symlink(&foreign, &parent).expect("substitute WAL directory");
        let roster = context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect::<BTreeSet<_>>();
        let capacity = LeaderWireLifecycleStoreGate::derived_capacity(
            roster.len(),
            context.da_layout.max_chunk_count,
        )
        .expect("derive leader-wire capacity");
        assert!(
            LeaderWireLifecycleStoreGate::open_with_safety_wal_authority(
                storage,
                context.id(),
                context.height,
                OWNER_A,
                roster,
                capacity,
                context.da_layout.max_chunk_count,
                leader_wire_recovery_authority(&context),
                &[],
                &[],
            )
            .is_err()
        );
        let adjacent = "00000000000000000007.wal.leader-wire-lifecycles";
        assert!(!retained.join(adjacent).exists());
        assert!(!foreign.join(adjacent).exists());
    }
    #[test]
    fn leader_wire_gate_restores_both_high_waters_and_normalizes_active_cuts() {
        let directory = TempDir::new().expect("temporary directory");
        let context = context();
        let wal = directory.path().join("leader-wire-active.wal");
        let roster = context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect::<BTreeSet<_>>();
        let max_chunks = 4;
        let capacity = LeaderWireLifecycleStoreGate::derived_capacity(roster.len(), max_chunks)
            .expect("derived gate capacity");
        let (gate, restore) = LeaderWireLifecycleStoreGate::open(
            &wal,
            context.id(),
            context.height,
            OWNER_A,
            roster.clone(),
            capacity,
            max_chunks,
            leader_wire_recovery_authority(&context),
            &[],
            &[],
        )
        .expect("open empty leader-wire gate");
        assert_eq!(restore.last_admission_ordinal(), 0);
        assert_eq!(restore.scheduler_ordinal_high_watermark(), 0);
        assert!(gate.matches_geometry(context.id(), context.height, &roster, capacity, max_chunks));
        let token = leader_wire_token(&context, 2, 7, 41, 1);
        let reserved = gate.reserve(token.clone()).expect("persist Reserved");
        assert!(reserved.inserted());
        gate.mark_ingress(&token).expect("persist Ingress");
        let runtime_owner =
            LeaderWireRuntimeOwner::new(token.identity_hash(), 41).expect("runtime owner");
        gate.mark_runtime(&token, runtime_owner)
            .expect("persist Runtime");
        let (reopened, restore) = LeaderWireLifecycleStoreGate::open(
            &wal,
            context.id(),
            context.height,
            OWNER_A,
            roster.clone(),
            capacity,
            max_chunks,
            leader_wire_recovery_authority(&context),
            &[],
            &[],
        )
        .expect("reopen active leader-wire gate");
        assert_eq!(restore.last_admission_ordinal(), 7);
        assert_eq!(restore.scheduler_ordinal_high_watermark(), 41);
        assert_eq!(restore.records().len(), 1);
        assert_eq!(
            restore.records()[0].status(),
            LeaderWireLifecycleStatus::Dormant
        );
        assert_eq!(restore.records()[0].runtime_owner(), Some(runtime_owner));
        assert_eq!(
            reopened
                .earliest_ingress_scheduler_ordinal()
                .expect("selector minimum"),
            None,
            "a restored lifecycle without a carrier is replay-dormant"
        );
        let retry = reopened
            .reserve(leader_wire_token(&context, 2, 99, 100, 1))
            .expect("exact retry coalesces to old durable token");
        assert!(!retry.inserted());
        assert_eq!(retry.token().admission_ordinal(), 7);
        assert_eq!(retry.token().scheduler_ordinal(), 41);
        reopened
            .mark_ingress(retry.token())
            .expect("exact physical retry reactivates the selector owner");
        assert_eq!(
            reopened
                .earliest_ingress_scheduler_ordinal()
                .expect("reactivated selector minimum"),
            Some(41)
        );
    }
    #[test]
    fn leader_wire_gate_retains_independent_cross_origin_phase_and_chunk_slots() {
        let directory = TempDir::new().expect("temporary directory");
        let context = context_with_roster_len(4);
        let wal = directory.path().join("leader-wire-owner-universe.wal");
        let roster = context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect::<BTreeSet<_>>();
        let max_chunks = 3;
        let per_origin_capacity = usize::try_from(max_chunks).expect("chunk count fits usize") + 8;
        let capacity = LeaderWireLifecycleStoreGate::derived_capacity(roster.len(), max_chunks)
            .expect("derived gate capacity");
        assert_eq!(capacity, roster.len() * per_origin_capacity);
        let (gate, _) = LeaderWireLifecycleStoreGate::open(
            &wal,
            context.id(),
            context.height,
            OWNER_A,
            roster.clone(),
            capacity,
            max_chunks,
            leader_wire_recovery_authority(&context),
            &[],
            &[],
        )
        .expect("open leader-wire owner-universe gate");
        let mut slots = vec![
            (FairV2IngressLeaderWirePhase::Proposal, None),
            (FairV2IngressLeaderWirePhase::PrepareVote, None),
            (FairV2IngressLeaderWirePhase::CommitVote, None),
            (FairV2IngressLeaderWirePhase::PrepareQc, None),
            (FairV2IngressLeaderWirePhase::CommitQc, None),
            (FairV2IngressLeaderWirePhase::TimeoutVote, None),
            (FairV2IngressLeaderWirePhase::TimeoutCertificate, None),
            (FairV2IngressLeaderWirePhase::CertifiedResponse, None),
        ];
        slots.extend(
            (0..max_chunks)
                .map(|chunk_index| (FairV2IngressLeaderWirePhase::Chunk, Some(chunk_index))),
        );
        assert_eq!(slots.len(), per_origin_capacity);
        let mut admitted = Vec::with_capacity(capacity);
        for origin in &roster {
            for (phase, chunk_index) in &slots {
                let ordinal =
                    u64::try_from(admitted.len() + 1).expect("test owner universe fits u64");
                let scheduler_ordinal = u128::from(ordinal) * 2;
                let token = leader_wire_slot_token(
                    &context,
                    origin,
                    *phase,
                    *chunk_index,
                    ordinal,
                    scheduler_ordinal,
                );
                let reserved = gate
                    .reserve(token.clone())
                    .expect("reserve exact owner slot");
                assert!(reserved.inserted());
                let mut retry = token.clone();
                retry.admission_ordinal = ordinal
                    .checked_add(u64::try_from(capacity).expect("capacity fits u64"))
                    .expect("retry ordinal fits u64");
                retry.scheduler_ordinal = scheduler_ordinal
                    .checked_add(u128::try_from(capacity).expect("capacity fits u128"))
                    .expect("retry scheduler ordinal fits u128");
                let coalesced = gate.reserve(retry).expect("coalesce only the exact slot");
                assert!(!coalesced.inserted());
                assert_eq!(coalesced.token(), &token);
                admitted.push(token);
            }
        }
        let (reopened, restored) = LeaderWireLifecycleStoreGate::open(
            &wal,
            context.id(),
            context.height,
            OWNER_A,
            roster.clone(),
            capacity,
            max_chunks,
            leader_wire_recovery_authority(&context),
            &[],
            &[],
        )
        .expect("reopen complete leader-wire owner universe");
        assert_eq!(restored.records().len(), capacity);
        let expected_slots = admitted
            .iter()
            .map(|token| token.slot.clone())
            .collect::<BTreeSet<_>>();
        let restored_slots = restored
            .records()
            .iter()
            .map(|record| record.token().slot.clone())
            .collect::<BTreeSet<_>>();
        assert_eq!(restored_slots, expected_slots);
        let expected_non_chunk_phases = slots
            .iter()
            .filter_map(|(phase, chunk_index)| chunk_index.is_none().then_some(*phase))
            .collect::<BTreeSet<_>>();
        let expected_chunk_indices = (0..max_chunks).collect::<BTreeSet<_>>();
        for origin in &roster {
            let records = restored
                .records()
                .iter()
                .filter(|record| record.token().slot.semantic_origin == *origin)
                .collect::<Vec<_>>();
            assert_eq!(records.len(), per_origin_capacity);
            let non_chunk_phases = records
                .iter()
                .filter_map(|record| {
                    record
                        .token()
                        .slot
                        .chunk_index
                        .is_none()
                        .then_some(record.token().slot.phase)
                })
                .collect::<BTreeSet<_>>();
            assert_eq!(non_chunk_phases, expected_non_chunk_phases);
            let chunk_indices = records
                .iter()
                .filter_map(|record| record.token().slot.chunk_index)
                .collect::<BTreeSet<_>>();
            assert_eq!(chunk_indices, expected_chunk_indices);
            let chunk_identity_hashes = records
                .iter()
                .filter(|record| record.token().slot.phase == FairV2IngressLeaderWirePhase::Chunk)
                .map(|record| record.token().identity_hash())
                .collect::<Vec<_>>();
            assert_eq!(
                chunk_identity_hashes.len(),
                usize::try_from(max_chunks).expect("chunk count fits usize")
            );
            assert!(
                chunk_identity_hashes
                    .iter()
                    .all(|identity_hash| *identity_hash == chunk_identity_hashes[0]),
                "chunk positions sharing every identity component still own distinct slots"
            );
        }
        assert_eq!(admitted.len(), capacity);
        let terminal_target = admitted
            .iter()
            .find(|token| token.slot.phase == FairV2IngressLeaderWirePhase::PrepareVote)
            .expect("one PrepareVote slot")
            .clone();
        let replay = reopened
            .reserve(terminal_target.clone())
            .expect("reactivate the exact restart-dormant target");
        assert!(!replay.inserted());
        assert_eq!(replay.token(), &terminal_target);
        reopened
            .mark_ingress(replay.token())
            .expect("replay target ingress after restart");
        let runtime_owner = LeaderWireRuntimeOwner::new(
            terminal_target.identity_hash(),
            terminal_target.scheduler_ordinal(),
        )
        .expect("exact runtime owner");
        let runtime = reopened
            .mark_runtime(replay.token(), runtime_owner)
            .expect("rebind exact runtime owner after restart");
        let producer_terminal = matching_terminal(&context, runtime_owner, &terminal_target);
        reopened
            .mark_producer_terminal(&runtime, producer_terminal)
            .expect("publish exact restart-stable terminal");
        let (terminal_gate, terminal_restore) = LeaderWireLifecycleStoreGate::open(
            &wal,
            context.id(),
            context.height,
            OWNER_A,
            roster,
            capacity,
            max_chunks,
            leader_wire_recovery_authority(&context),
            &[producer_terminal],
            &[],
        )
        .expect("reopen complete owner universe with exact terminal evidence");
        let terminal_record = terminal_restore
            .records()
            .iter()
            .find(|record| record.token().slot == terminal_target.slot)
            .expect("terminal slot remains present");
        assert_eq!(
            terminal_record.status(),
            LeaderWireLifecycleStatus::Terminal
        );
        assert_eq!(terminal_record.token(), &terminal_target);
        let mut terminal_retry = terminal_target.clone();
        terminal_retry.admission_ordinal =
            u64::try_from(capacity + 1).expect("capacity successor fits u64");
        terminal_retry.scheduler_ordinal =
            u128::try_from(2 * capacity + 1).expect("scheduler successor fits u128");
        let suppressed = terminal_gate
            .reserve(terminal_retry)
            .expect("exact terminal retry remains coalesced after restart");
        assert!(!suppressed.inserted());
        assert_eq!(suppressed.status(), LeaderWireLifecycleStatus::Terminal);
        assert_eq!(suppressed.token(), &terminal_target);
    }
    #[test]
    fn leader_wire_gate_reconciles_producer_first_terminal_crash() {
        let directory = TempDir::new().expect("temporary directory");
        let context = context();
        let wal = directory.path().join("leader-wire-terminal.wal");
        let roster = context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect::<BTreeSet<_>>();
        let max_chunks = 2;
        let capacity = LeaderWireLifecycleStoreGate::derived_capacity(roster.len(), max_chunks)
            .expect("derived gate capacity");
        let (gate, _) = LeaderWireLifecycleStoreGate::open(
            &wal,
            context.id(),
            context.height,
            OWNER_A,
            roster.clone(),
            capacity,
            max_chunks,
            leader_wire_recovery_authority(&context),
            &[],
            &[],
        )
        .expect("open leader-wire gate");
        let token = leader_wire_token(&context, 2, 11, 73, 2);
        gate.reserve(token.clone()).expect("reserve");
        gate.mark_ingress(&token).expect("mark ingress");
        let runtime_owner =
            LeaderWireRuntimeOwner::new(token.identity_hash(), 73).expect("runtime owner");
        gate.mark_runtime(&token, runtime_owner)
            .expect("mark runtime");
        let producer_terminal = matching_terminal(&context, runtime_owner, &token);
        let (reconciled, restore) = LeaderWireLifecycleStoreGate::open(
            &wal,
            context.id(),
            context.height,
            OWNER_A,
            roster.clone(),
            capacity,
            max_chunks,
            leader_wire_recovery_authority(&context),
            &[producer_terminal],
            &[],
        )
        .expect("producer-first crash promotes wire terminal");
        assert_eq!(
            restore.records()[0].status(),
            LeaderWireLifecycleStatus::Terminal
        );
        assert_eq!(
            restore.records()[0].terminal_evidence(),
            Some(&LeaderWireStableTerminalEvidence::Producer(
                producer_terminal
            ))
        );
        let suppressed = reconciled
            .reserve(leader_wire_token(&context, 2, 88, 101, 2))
            .expect("exact terminal retry is suppressed");
        assert_eq!(suppressed.status(), LeaderWireLifecycleStatus::Terminal);
        assert_eq!(suppressed.token().scheduler_ordinal(), 73);
        assert!(
            LeaderWireLifecycleStoreGate::open(
                &wal,
                context.id(),
                context.height,
                OWNER_A,
                roster,
                capacity,
                max_chunks,
                leader_wire_recovery_authority(&context),
                &[],
                &[],
            )
            .is_err(),
            "wire Terminal without its producer terminal fails closed"
        );
    }
    #[test]
    fn leader_wire_gate_rejects_producer_terminal_from_foreign_view_or_phase() {
        let directory = TempDir::new().expect("temporary directory");
        let context = context();
        let wal = directory.path().join("leader-wire-terminal-binding.wal");
        let roster = context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect::<BTreeSet<_>>();
        let max_chunks = 2;
        let capacity = LeaderWireLifecycleStoreGate::derived_capacity(roster.len(), max_chunks)
            .expect("derived gate capacity");
        let (gate, _) = LeaderWireLifecycleStoreGate::open(
            &wal,
            context.id(),
            context.height,
            OWNER_A,
            roster.clone(),
            capacity,
            max_chunks,
            leader_wire_recovery_authority(&context),
            &[],
            &[],
        )
        .expect("open leader-wire gate");
        let token = leader_wire_token(&context, 2, 11, 73, 2);
        gate.reserve(token.clone()).expect("reserve");
        gate.mark_ingress(&token).expect("mark ingress");
        let runtime_owner =
            LeaderWireRuntimeOwner::new(token.identity_hash(), 73).expect("runtime owner");
        let runtime = gate
            .mark_runtime(&token, runtime_owner)
            .expect("mark runtime");
        let mut foreign_view = token.clone();
        foreign_view.identity.view = 1;
        let foreign_view_terminal = matching_terminal(&context, runtime_owner, &foreign_view);
        assert!(
            gate.mark_producer_terminal(&runtime, foreign_view_terminal)
                .is_err(),
            "same causal owner and ordinal cannot authenticate a foreign source view"
        );
        let mut foreign_phase = token.clone();
        foreign_phase.identity.phase = FairV2IngressLeaderWirePhase::CommitVote;
        foreign_phase.slot.phase = FairV2IngressLeaderWirePhase::CommitVote;
        let foreign_phase_terminal = matching_terminal(&context, runtime_owner, &foreign_phase);
        assert!(
            gate.mark_producer_terminal(&runtime, foreign_phase_terminal)
                .is_err(),
            "same causal owner and ordinal cannot authenticate a foreign protocol phase"
        );
        let (reopened, restore) = LeaderWireLifecycleStoreGate::open(
            &wal,
            context.id(),
            context.height,
            OWNER_A,
            roster,
            capacity,
            max_chunks,
            leader_wire_recovery_authority(&context),
            &[foreign_view_terminal, foreign_phase_terminal],
            &[],
        )
        .expect("foreign producer terminals cannot suppress exact replay");
        assert_eq!(
            restore.records()[0].status(),
            LeaderWireLifecycleStatus::Dormant
        );
        assert!(restore.records()[0].terminal_evidence().is_none());
        let replay = reopened
            .reserve(token.clone())
            .expect("reactivate the exact restart-dormant owner");
        assert!(!replay.inserted());
        assert_eq!(replay.token(), &token);
        reopened.mark_ingress(&token).expect("replay exact ingress");
        let runtime = reopened
            .mark_runtime(replay.token(), runtime_owner)
            .expect("rebind exact runtime owner");
        reopened
            .mark_producer_terminal(&runtime, matching_terminal(&context, runtime_owner, &token))
            .expect("exact view and phase publish the producer terminal");
    }
    #[test]
    fn leader_wire_recovery_authority_retires_obsolete_records_and_retains_highwaters() {
        for (label, durable_view, decision_durable, publish_terminal) in [
            ("advanced-view", 3, false, false),
            ("decision", 2, true, true),
        ] {
            let directory = TempDir::new().expect("temporary directory");
            let context = context();
            let wal = directory.path().join(format!("leader-wire-{label}.wal"));
            let roster = context
                .roster
                .iter()
                .map(|entry| entry.validator.clone())
                .collect::<BTreeSet<_>>();
            let max_chunks = 2;
            let capacity = LeaderWireLifecycleStoreGate::derived_capacity(roster.len(), max_chunks)
                .expect("derived gate capacity");
            let (gate, _) = LeaderWireLifecycleStoreGate::open(
                &wal,
                context.id(),
                context.height,
                OWNER_A,
                roster.clone(),
                capacity,
                max_chunks,
                leader_wire_recovery_authority(&context),
                &[],
                &[],
            )
            .expect("open leader-wire gate");
            let token = leader_wire_token(&context, 2, 11, 73, 2);
            gate.reserve(token.clone()).expect("reserve");
            gate.mark_ingress(&token).expect("mark ingress");
            let runtime_owner =
                LeaderWireRuntimeOwner::new(token.identity_hash(), 73).expect("runtime owner");
            let runtime = gate
                .mark_runtime(&token, runtime_owner)
                .expect("mark runtime");
            if publish_terminal {
                gate.mark_producer_terminal(
                    &runtime,
                    matching_terminal(&context, runtime_owner, &token),
                )
                .expect("publish independently durable terminal before Decision");
            }
            let (reopened, restore) = LeaderWireLifecycleStoreGate::open(
                &wal,
                context.id(),
                context.height,
                OWNER_A,
                roster,
                capacity,
                max_chunks,
                leader_wire_recovery_authority_at(
                    &context,
                    OWNER_A,
                    durable_view,
                    decision_durable,
                ),
                &[],
                &[],
            )
            .expect("replay authority retires the obsolete lifecycle");
            assert!(restore.records().is_empty(), "{label}");
            assert_eq!(restore.last_admission_ordinal(), 11, "{label}");
            assert_eq!(restore.scheduler_ordinal_high_watermark(), 73, "{label}");
            assert!(
                reopened.reserve(token).is_err(),
                "{label} cannot reuse the retired physical ordinals"
            );
            let newer = leader_wire_token(
                &context,
                durable_view.checked_add(1).expect("fixture view advances"),
                12,
                74,
                3,
            );
            if decision_durable {
                assert!(
                    reopened.reserve(newer).is_err(),
                    "Decision retires every same-height view-scoped lifecycle"
                );
            } else {
                reopened
                    .reserve(newer)
                    .expect("a strictly newer view remains admissible");
            }
        }
    }
    #[test]
    fn leader_wire_recovery_cut_keeps_body_transport_admissible() {
        for (label, durable_view, decision_durable, control_view) in
            [("advanced-view", 3, false, 2), ("decision", 3, true, 4)]
        {
            let directory = TempDir::new().expect("temporary directory");
            let context = context();
            let wal = directory
                .path()
                .join(format!("leader-wire-body-{label}.wal"));
            let roster = context
                .roster
                .iter()
                .map(|entry| entry.validator.clone())
                .collect::<BTreeSet<_>>();
            let max_chunks = 2;
            let capacity = LeaderWireLifecycleStoreGate::derived_capacity(roster.len(), max_chunks)
                .expect("derived gate capacity");
            let (gate, _) = LeaderWireLifecycleStoreGate::open(
                &wal,
                context.id(),
                context.height,
                OWNER_A,
                roster,
                capacity,
                max_chunks,
                leader_wire_recovery_authority_at(
                    &context,
                    OWNER_A,
                    durable_view,
                    decision_durable,
                ),
                &[],
                &[],
            )
            .expect("open leader-wire gate at the durable cut");
            let control = leader_wire_token(&context, control_view, 1, 1, 0x91);
            let origin = context.roster[0].validator.clone();
            let chunk = leader_wire_slot_token(
                &context,
                &origin,
                FairV2IngressLeaderWirePhase::Chunk,
                Some(0),
                2,
                2,
            );
            let response = leader_wire_slot_token(
                &context,
                &origin,
                FairV2IngressLeaderWirePhase::CertifiedResponse,
                None,
                3,
                3,
            );
            assert!(
                gate.identity_is_obsolete(&control.identity)
                    .expect("inspect control identity"),
                "{label} closes obsolete control"
            );
            assert!(
                !gate
                    .identity_is_obsolete(&chunk.identity)
                    .expect("inspect chunk identity"),
                "{label} keeps an exact body chunk eligible"
            );
            assert!(
                !gate
                    .identity_is_obsolete(&response.identity)
                    .expect("inspect response identity"),
                "{label} keeps an exact certified body response eligible"
            );
            assert!(
                gate.reserve(control).is_err(),
                "{label} rejects obsolete control admission"
            );
            gate.reserve(chunk)
                .expect("the downstream fetch must decide whether the chunk is relevant");
            gate.reserve(response)
                .expect("the downstream request must authenticate the certified response");
        }
    }
    #[test]
    fn leader_wire_recovery_cuts_preserve_historical_response_and_conflict_fence() {
        let directory = TempDir::new().expect("temporary directory");
        let context = context();
        let wal = directory.path().join("leader-wire-certified-response.wal");
        let roster = context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect::<BTreeSet<_>>();
        let max_chunks = 2;
        let capacity = LeaderWireLifecycleStoreGate::derived_capacity(roster.len(), max_chunks)
            .expect("derived gate capacity");
        let initial_authority = leader_wire_recovery_authority(&context);
        let (gate, _) = LeaderWireLifecycleStoreGate::open(
            &wal,
            context.id(),
            context.height,
            OWNER_A,
            roster.clone(),
            capacity,
            max_chunks,
            initial_authority,
            &[],
            &[],
        )
        .expect("open leader-wire gate");
        let origin = context.roster[0].validator.clone();
        let response = leader_wire_slot_token(
            &context,
            &origin,
            FairV2IngressLeaderWirePhase::CertifiedResponse,
            None,
            11,
            73,
        );
        let proposal = leader_wire_slot_token(
            &context,
            &origin,
            FairV2IngressLeaderWirePhase::Proposal,
            None,
            12,
            74,
        );
        let protected_round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 2,
        };
        let protected_subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::new(
                b"protected historical Commit block",
            )),
            payload_hash: Hash::new(b"protected historical Commit payload"),
        };
        let mut protected_commit = leader_wire_slot_token(
            &context,
            &origin,
            FairV2IngressLeaderWirePhase::CommitVote,
            None,
            13,
            75,
        );
        protected_commit.identity.view = protected_round.view;
        protected_commit.identity.subject_hash = Hash::new(protected_subject.encode());
        let historical_commit_qc = leader_wire_slot_token(
            &context,
            &origin,
            FairV2IngressLeaderWirePhase::CommitQc,
            None,
            14,
            76,
        );
        gate.reserve(response.clone())
            .expect("reserve historical response");
        gate.reserve(proposal.clone())
            .expect("reserve view-scoped proposal");
        gate.reserve(protected_commit.clone())
            .expect("reserve historical Commit vote");
        gate.reserve(historical_commit_qc.clone())
            .expect("reserve historical CommitQC");
        drop(gate);
        let (gate, restore) = LeaderWireLifecycleStoreGate::open(
            &wal,
            context.id(),
            context.height,
            OWNER_A,
            roster.clone(),
            capacity,
            max_chunks,
            initial_authority,
            &[],
            &[],
        )
        .expect("reopen leader-wire owners as dormant");
        assert_eq!(restore.records().len(), 4);
        let advanced = leader_wire_recovery_authority_at(&context, OWNER_A, 3, false)
            .with_protected_lock(Some((protected_round, protected_subject)))
            .expect("project the replayed durable lock");
        gate.advance_recovery_cut(advanced, &BTreeSet::from([proposal.slot.clone()]))
            .expect("retire only the view-scoped dormant owner");
        assert!(
            gate.identity_is_obsolete(&proposal.identity)
                .expect("inspect proposal cut")
        );
        assert!(
            !gate
                .identity_is_obsolete(&response.identity)
                .expect("inspect historical response cut"),
            "an outstanding certified-body recovery must survive local view advance"
        );
        let restore = gate.restore().expect("inspect retained recovery owner");
        assert_eq!(restore.records().len(), 3);
        assert!(restore.records().iter().all(|record| {
            record.status() == LeaderWireLifecycleStatus::Dormant && record.token() != &proposal
        }));
        assert!(
            !gate
                .identity_is_obsolete(&protected_commit.identity)
                .expect("inspect protected Commit vote cut"),
            "the exact durable-lock Commit vote remains reducer input"
        );
        assert!(
            !gate
                .identity_is_obsolete(&historical_commit_qc.identity)
                .expect("inspect historical CommitQC cut"),
            "CommitQC remains terminal progress until Decision"
        );
        let replay = gate
            .admit_ingress(response.clone())
            .expect("reactivate the exact historical response");
        assert!(!replay.inserted());
        assert_eq!(replay.status(), LeaderWireLifecycleStatus::Ingress);
        let mut conflicting = response.clone();
        conflicting.identity.subject_hash = Hash::new(b"conflicting certified subject");
        conflicting.identity.canonical_wire_hash = Hash::new(b"conflicting certified bytes");
        conflicting.admission_ordinal = 13;
        conflicting.scheduler_ordinal = 75;
        assert!(
            gate.admit_ingress(conflicting).is_err(),
            "the view-cut exception must not weaken one-owner same-slot conflict fencing"
        );
        let commit_replay = gate
            .admit_ingress(protected_commit)
            .expect("reactivate the exact durable-lock Commit vote");
        assert!(!commit_replay.inserted());
        assert_eq!(commit_replay.status(), LeaderWireLifecycleStatus::Ingress);
        let decision = leader_wire_recovery_authority_at(&context, OWNER_A, 3, true);
        drop(gate);
        let (gate, restore) = LeaderWireLifecycleStoreGate::open(
            &wal,
            context.id(),
            context.height,
            OWNER_A,
            roster,
            capacity,
            max_chunks,
            decision,
            &[],
            &[],
        )
        .expect("replay the decided height before its exact body is recovered");
        assert_eq!(restore.records().len(), 1);
        assert_eq!(restore.records()[0].token(), &response);
        assert_eq!(
            restore.records()[0].status(),
            LeaderWireLifecycleStatus::Dormant
        );
        assert!(
            !gate
                .identity_is_obsolete(&response.identity)
                .expect("inspect decided-body recovery cut"),
            "durable Decision can precede exact decided-body recovery"
        );
        gate.admit_ingress(response)
            .expect("the exact decided-body response can reactivate after restart");
    }
    #[test]
    fn leader_wire_protected_lock_cut_is_exact_monotone_and_decision_closed() {
        let context = context();
        let origin = context.roster[0].validator.clone();
        let protected_round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 2,
        };
        let protected_subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"protected Commit block")),
            payload_hash: Hash::new(b"protected Commit payload"),
        };
        let replayed = leader_wire_recovery_authority_at(&context, OWNER_A, 2, false)
            .with_protected_lock(Some((protected_round, protected_subject)))
            .expect("a replayed current-round lock is authoritative");
        let advanced = replayed
            .advance_view(5, Some((protected_round, protected_subject)))
            .expect("the exact lock survives certified view churn");
        let mut protected_commit = leader_wire_slot_token(
            &context,
            &origin,
            FairV2IngressLeaderWirePhase::CommitVote,
            None,
            21,
            81,
        );
        protected_commit.identity.view = protected_round.view;
        protected_commit.identity.subject_hash = Hash::new(protected_subject.encode());
        assert!(!advanced.retires(&protected_commit));
        assert!(advanced.admits_ingress_identity(&protected_commit.identity));

        let mut wrong_subject = protected_commit.clone();
        wrong_subject.identity.subject_hash = Hash::new(b"wrong protected Commit subject");
        wrong_subject.identity.canonical_wire_hash = Hash::new(b"wrong protected Commit bytes");
        assert!(advanced.retires(&wrong_subject));
        assert!(!advanced.admits_ingress_identity(&wrong_subject.identity));

        let mut wrong_round = protected_commit.clone();
        wrong_round.identity.view = protected_round.view.saturating_sub(1);
        wrong_round.identity.canonical_wire_hash = Hash::new(b"wrong protected Commit round");
        assert!(advanced.retires(&wrong_round));
        assert!(!advanced.admits_ingress_identity(&wrong_round.identity));

        let mut wrong_phase = leader_wire_slot_token(
            &context,
            &origin,
            FairV2IngressLeaderWirePhase::PrepareVote,
            None,
            22,
            82,
        );
        wrong_phase.identity.view = protected_round.view;
        wrong_phase.identity.subject_hash = Hash::new(protected_subject.encode());
        assert!(advanced.retires(&wrong_phase));
        assert!(!advanced.admits_ingress_identity(&wrong_phase.identity));

        let historical_commit_qc = leader_wire_slot_token(
            &context,
            &origin,
            FairV2IngressLeaderWirePhase::CommitQc,
            None,
            23,
            83,
        );
        assert!(!advanced.retires(&historical_commit_qc));
        assert!(advanced.admits_ingress_identity(&historical_commit_qc.identity));

        assert!(
            advanced.advance_view(6, None).is_err(),
            "live authority cannot lose its durable lock"
        );
        let conflicting_subject = wire::BlockSubject {
            payload_hash: Hash::new(b"conflicting protected Commit payload"),
            ..protected_subject
        };
        assert!(
            advanced
                .advance_view(6, Some((protected_round, conflicting_subject)))
                .is_err(),
            "same-round lock authority cannot change subject"
        );
        let lower_round = wire::ConsensusRound {
            view: protected_round.view.saturating_sub(1),
            ..protected_round
        };
        assert!(
            advanced
                .advance_view(6, Some((lower_round, protected_subject)))
                .is_err(),
            "lock authority cannot regress"
        );
        let higher_round = wire::ConsensusRound {
            view: protected_round.view + 1,
            ..protected_round
        };
        advanced
            .advance_view(6, Some((higher_round, conflicting_subject)))
            .expect("a strictly higher durable lock may replace the protected subject");

        let decision = advanced.with_durable_decision();
        assert!(decision.retires(&protected_commit));
        assert!(!decision.admits_ingress_identity(&protected_commit.identity));
        assert!(decision.retires(&historical_commit_qc));
        assert!(!decision.admits_ingress_identity(&historical_commit_qc.identity));
    }
    #[test]
    fn leader_wire_live_recovery_cut_retires_only_dormant_records_and_is_monotone() {
        for (label, next_view, decision_durable) in
            [("advanced-view", 3, false), ("decision", 2, true)]
        {
            let directory = TempDir::new().expect("temporary directory");
            let context = context();
            let wal = directory
                .path()
                .join(format!("leader-wire-live-{label}.wal"));
            let roster = context
                .roster
                .iter()
                .map(|entry| entry.validator.clone())
                .collect::<BTreeSet<_>>();
            let max_chunks = 2;
            let capacity = LeaderWireLifecycleStoreGate::derived_capacity(roster.len(), max_chunks)
                .expect("derived gate capacity");
            let initial_authority = leader_wire_recovery_authority(&context);
            let (gate, _) = LeaderWireLifecycleStoreGate::open(
                &wal,
                context.id(),
                context.height,
                OWNER_A,
                roster.clone(),
                capacity,
                max_chunks,
                initial_authority,
                &[],
                &[],
            )
            .expect("open leader-wire gate");
            let token = leader_wire_token(&context, 2, 11, 73, 2);
            gate.reserve(token.clone()).expect("reserve restart owner");
            drop(gate);
            let (gate, restore) = LeaderWireLifecycleStoreGate::open(
                &wal,
                context.id(),
                context.height,
                OWNER_A,
                roster,
                capacity,
                max_chunks,
                initial_authority,
                &[],
                &[],
            )
            .expect("reopen restart owner as dormant");
            assert_eq!(restore.records().len(), 1, "{label}");
            assert_eq!(
                restore.records()[0].status(),
                LeaderWireLifecycleStatus::Dormant,
                "{label}"
            );
            let next =
                leader_wire_recovery_authority_at(&context, OWNER_A, next_view, decision_durable);
            let expected = BTreeSet::from([token.slot.clone()]);
            assert!(
                gate.advance_recovery_cut(
                    leader_wire_recovery_authority_at(
                        &context,
                        OWNER_B,
                        next_view,
                        decision_durable,
                    ),
                    &expected,
                )
                .is_err(),
                "{label} cannot cross immutable owner geometry"
            );
            gate.advance_recovery_cut(next, &expected)
                .expect("advance the live recovery cut");
            gate.advance_recovery_cut(next, &BTreeSet::new())
                .expect("repeating the exact recovery cut is idempotent");
            let restored = gate.restore().expect("inspect retired dormant owner");
            assert!(restored.records().is_empty(), "{label}");
            assert_eq!(restored.last_admission_ordinal(), 11, "{label}");
            assert_eq!(restored.scheduler_ordinal_high_watermark(), 73, "{label}");
            assert!(
                gate.identity_is_obsolete(&token.identity)
                    .expect("inspect live recovery cut"),
                "{label} rejects the retired identity without an exact retry"
            );
            let regressed = leader_wire_recovery_authority_at(
                &context,
                OWNER_A,
                next_view.saturating_sub(1),
                false,
            );
            assert!(
                gate.advance_recovery_cut(regressed, &BTreeSet::new())
                    .is_err(),
                "{label} cannot regress durable view/Decision authority"
            );
            let fresh = leader_wire_token(&context, 3, 12, 74, 3);
            if decision_durable {
                assert!(
                    gate.reserve(fresh).is_err(),
                    "Decision rejects every later admission at the closed height"
                );
            } else {
                gate.reserve(fresh)
                    .expect("the cut admits a current-view replacement");
            }
        }
        for retained_status in [
            LeaderWireLifecycleStatus::Ingress,
            LeaderWireLifecycleStatus::Runtime,
        ] {
            let directory = TempDir::new().expect("temporary active-owner directory");
            let context = context();
            let wal = directory
                .path()
                .join(format!("leader-wire-live-retains-{retained_status:?}.wal"));
            let roster = context
                .roster
                .iter()
                .map(|entry| entry.validator.clone())
                .collect::<BTreeSet<_>>();
            let max_chunks = 1;
            let capacity = LeaderWireLifecycleStoreGate::derived_capacity(roster.len(), max_chunks)
                .expect("derived gate capacity");
            let (gate, _) = LeaderWireLifecycleStoreGate::open(
                &wal,
                context.id(),
                context.height,
                OWNER_A,
                roster,
                capacity,
                max_chunks,
                leader_wire_recovery_authority(&context),
                &[],
                &[],
            )
            .expect("open active-owner gate");
            let token = leader_wire_token(&context, 2, 11, 73, 2);
            gate.admit_ingress(token.clone())
                .expect("publish active ingress owner");
            if retained_status == LeaderWireLifecycleStatus::Runtime {
                let runtime_owner =
                    LeaderWireRuntimeOwner::new(token.identity_hash(), 73).expect("runtime owner");
                gate.mark_runtime(&token, runtime_owner)
                    .expect("publish active runtime owner");
            }
            gate.advance_recovery_cut(
                leader_wire_recovery_authority_at(&context, OWNER_A, 3, false),
                &BTreeSet::new(),
            )
            .expect("advance while an active owner remains live");
            let restore = gate.restore().expect("inspect retained active owner");
            assert_eq!(restore.records().len(), 1, "{retained_status:?}");
            assert_eq!(
                restore.records()[0].status(),
                retained_status,
                "the live cut may reclaim only Dormant records"
            );
            assert_eq!(restore.records()[0].token(), &token);
            assert_eq!(restore.last_admission_ordinal(), 11);
            assert_eq!(restore.scheduler_ordinal_high_watermark(), 73);
            assert!(
                gate.identity_is_obsolete(&token.identity)
                    .expect("inspect advanced recovery authority"),
                "active retention must not roll the recovery authority back"
            );
        }
        {
            let directory = TempDir::new().expect("temporary recovery-cut rollback directory");
            let context = context();
            let wal = directory.path().join("leader-wire-live-cut-rollback.wal");
            let roster = context
                .roster
                .iter()
                .map(|entry| entry.validator.clone())
                .collect::<BTreeSet<_>>();
            let max_chunks = 1;
            let capacity = LeaderWireLifecycleStoreGate::derived_capacity(roster.len(), max_chunks)
                .expect("derived gate capacity");
            let initial_authority = leader_wire_recovery_authority(&context);
            let (gate, _) = LeaderWireLifecycleStoreGate::open(
                &wal,
                context.id(),
                context.height,
                OWNER_A,
                roster.clone(),
                capacity,
                max_chunks,
                initial_authority,
                &[],
                &[],
            )
            .expect("open rollback gate");
            let token = leader_wire_token(&context, 2, 11, 73, 2);
            gate.admit_ingress(token.clone())
                .expect("publish owner before restart");
            drop(gate);
            let (gate, restore) = LeaderWireLifecycleStoreGate::open(
                &wal,
                context.id(),
                context.height,
                OWNER_A,
                roster,
                capacity,
                max_chunks,
                initial_authority,
                &[],
                &[],
            )
            .expect("reopen rollback owner as dormant");
            assert_eq!(
                restore.records()[0].status(),
                LeaderWireLifecycleStatus::Dormant
            );
            std::fs::remove_file(&gate.path).expect("remove published snapshot");
            std::fs::create_dir(&gate.path).expect("block recovery-cut publication");
            assert!(
                gate.advance_recovery_cut(
                    leader_wire_recovery_authority_at(&context, OWNER_A, 3, false),
                    &BTreeSet::from([token.slot.clone()]),
                )
                .is_err(),
                "a failed atomic publication must reject the live cut"
            );
            let restored = gate.restore().expect("inspect recovery-cut rollback");
            assert_eq!(restored.records().len(), 1);
            assert_eq!(restored.records()[0].token(), &token);
            assert_eq!(
                restored.records()[0].status(),
                LeaderWireLifecycleStatus::Dormant
            );
            assert_eq!(restored.last_admission_ordinal(), 11);
            assert_eq!(restored.scheduler_ordinal_high_watermark(), 73);
            assert!(
                !gate
                    .identity_is_obsolete(&token.identity)
                    .expect("inspect rolled-back recovery authority"),
                "failed persistence must restore both the owner and recovery authority"
            );
        }
    }
    #[test]
    fn leader_wire_gate_rejects_foreign_recovery_authority() {
        let directory = TempDir::new().expect("temporary directory");
        let context = context();
        let wal = directory.path().join("leader-wire-foreign-authority.wal");
        let roster = context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect::<BTreeSet<_>>();
        let max_chunks = 2;
        let capacity = LeaderWireLifecycleStoreGate::derived_capacity(roster.len(), max_chunks)
            .expect("derived gate capacity");
        assert!(
            LeaderWireLifecycleStoreGate::open(
                &wal,
                context.id(),
                context.height,
                OWNER_A,
                roster,
                capacity,
                max_chunks,
                leader_wire_recovery_authority_at(&context, OWNER_B, 0, false),
                &[],
                &[],
            )
            .is_err(),
            "replay authority cannot cross the owner-bound snapshot geometry"
        );
    }
    include!("serviced_candidate_store/body_terminal_recovery_tests.rs");
    #[test]
    fn leader_wire_gate_rejects_duplicate_scheduler_and_low_high_watermarks() {
        for defect in ["duplicate-scheduler", "low-high-water"] {
            let directory = TempDir::new().expect("temporary directory");
            let context = context();
            let wal = directory.path().join(format!("leader-wire-{defect}.wal"));
            let roster = context
                .roster
                .iter()
                .map(|entry| entry.validator.clone())
                .collect::<BTreeSet<_>>();
            let max_chunks = 1;
            let capacity = LeaderWireLifecycleStoreGate::derived_capacity(roster.len(), max_chunks)
                .expect("derived gate capacity");
            let (gate, _) = LeaderWireLifecycleStoreGate::open(
                &wal,
                context.id(),
                context.height,
                OWNER_A,
                roster.clone(),
                capacity,
                max_chunks,
                leader_wire_recovery_authority(&context),
                &[],
                &[],
            )
            .expect("open empty gate");
            let first = leader_wire_token(&context, 2, 1, 7, 1);
            let mut second = leader_wire_token(&context, 2, 2, 9, 2);
            second.identity.phase = FairV2IngressLeaderWirePhase::CommitVote;
            second.slot.phase = FairV2IngressLeaderWirePhase::CommitVote;
            if defect == "duplicate-scheduler" {
                second.scheduler_ordinal = first.scheduler_ordinal;
            }
            let scheduler_ordinal_high_watermark = if defect == "low-high-water" {
                second.scheduler_ordinal - 1
            } else {
                second.scheduler_ordinal
            };
            let snapshot = PersistedLeaderWireLifecycles {
                format_version: LEADER_WIRE_FORMAT_VERSION,
                context_id: context.id(),
                height: context.height,
                owner: OWNER_A,
                capacity: u64::try_from(capacity).expect("capacity fits u64"),
                max_chunk_count: max_chunks,
                last_admission_ordinal: 2,
                scheduler_ordinal_high_watermark,
                records: vec![first, second]
                    .into_iter()
                    .map(|token| PersistedLeaderWireLifecycleRecord {
                        token,
                        status: LeaderWireLifecycleStatus::Dormant,
                        runtime_owner: None,
                        terminal_evidence: None,
                    })
                    .collect(),
            };
            let frame = encode_leader_wire_frame(&snapshot, gate.max_frame_bytes)
                .expect("encode corrupt-but-canonical fixture");
            fs::write(&gate.path, frame).expect("publish fixture");
            assert!(
                LeaderWireLifecycleStoreGate::open(
                    &wal,
                    context.id(),
                    context.height,
                    OWNER_A,
                    roster,
                    capacity,
                    max_chunks,
                    leader_wire_recovery_authority(&context),
                    &[],
                    &[],
                )
                .is_err(),
                "{defect} must fail closed"
            );
        }
    }
    #[test]
    fn leader_wire_gate_rolls_back_failed_atomic_status_publications() {
        fn replace_snapshot_with_directory(gate: &LeaderWireLifecycleStoreGate) {
            if let Err(error) = std::fs::remove_file(&gate.path)
                && error.kind() != std::io::ErrorKind::NotFound
            {
                panic!("remove prior gate snapshot: {error}");
            }
            std::fs::create_dir(&gate.path).expect("replace snapshot with directory");
        }
        {
            let directory = TempDir::new().expect("temporary directory");
            let context = context();
            let wal = directory.path().join("leader-wire-reserve.wal");
            let roster = context
                .roster
                .iter()
                .map(|entry| entry.validator.clone())
                .collect::<BTreeSet<_>>();
            let max_chunks = 1;
            let capacity = LeaderWireLifecycleStoreGate::derived_capacity(roster.len(), max_chunks)
                .expect("derived gate capacity");
            let (gate, _) = LeaderWireLifecycleStoreGate::open(
                &wal,
                context.id(),
                context.height,
                OWNER_A,
                roster,
                capacity,
                max_chunks,
                leader_wire_recovery_authority(&context),
                &[],
                &[],
            )
            .expect("open gate");
            std::fs::create_dir(&gate.path).expect("block first admission publication");
            assert!(
                gate.admit_ingress(leader_wire_token(&context, 2, 1, 5, 0))
                    .is_err()
            );
            let restored = gate.restore().expect("admission memory rollback");
            assert!(restored.records().is_empty());
            assert_eq!(restored.last_admission_ordinal(), 0);
            assert_eq!(restored.scheduler_ordinal_high_watermark(), 0);
        }
        for failed_cut in ["ingress", "runtime", "volatile-terminal", "terminal"] {
            let directory = TempDir::new().expect("temporary directory");
            let context = context();
            let wal = directory
                .path()
                .join(format!("leader-wire-{failed_cut}.wal"));
            let roster = context
                .roster
                .iter()
                .map(|entry| entry.validator.clone())
                .collect::<BTreeSet<_>>();
            let max_chunks = 1;
            let capacity = LeaderWireLifecycleStoreGate::derived_capacity(roster.len(), max_chunks)
                .expect("derived gate capacity");
            let (gate, _) = LeaderWireLifecycleStoreGate::open(
                &wal,
                context.id(),
                context.height,
                OWNER_A,
                roster,
                capacity,
                max_chunks,
                leader_wire_recovery_authority(&context),
                &[],
                &[],
            )
            .expect("open gate");
            let token = leader_wire_token(&context, 2, 13, 97, 3);
            if failed_cut != "ingress" {
                gate.admit_ingress(token.clone())
                    .expect("admit before later failure");
            }
            let runtime_owner =
                LeaderWireRuntimeOwner::new(token.identity_hash(), 97).expect("runtime owner");
            match failed_cut {
                "ingress" => {
                    replace_snapshot_with_directory(&gate);
                    assert!(gate.admit_ingress(token.clone()).is_err());
                    let restored = gate.restore().expect("memory rollback");
                    assert!(restored.records().is_empty());
                    assert_eq!(restored.last_admission_ordinal(), 0);
                    assert_eq!(restored.scheduler_ordinal_high_watermark(), 0);
                }
                "runtime" => {
                    replace_snapshot_with_directory(&gate);
                    assert!(gate.mark_runtime(&token, runtime_owner).is_err());
                    assert_eq!(
                        gate.restore().expect("memory rollback").records()[0].status(),
                        LeaderWireLifecycleStatus::Ingress
                    );
                }
                "terminal" => {
                    let receipt = gate
                        .mark_runtime(&token, runtime_owner)
                        .expect("mark runtime");
                    replace_snapshot_with_directory(&gate);
                    assert!(
                        gate.mark_terminal(
                            &receipt,
                            matching_terminal(&context, runtime_owner, &token),
                        )
                        .is_err()
                    );
                    assert_eq!(
                        gate.restore().expect("memory rollback").records()[0].status(),
                        LeaderWireLifecycleStatus::Runtime
                    );
                }
                "volatile-terminal" => {
                    let receipt = gate
                        .mark_runtime(&token, runtime_owner)
                        .expect("mark runtime");
                    replace_snapshot_with_directory(&gate);
                    assert!(gate.mark_volatile_terminal(&receipt).is_err());
                    assert_eq!(
                        gate.restore().expect("memory rollback").records()[0].status(),
                        LeaderWireLifecycleStatus::Runtime
                    );
                }
                _ => unreachable!(),
            }
        }
        {
            let directory = TempDir::new().expect("temporary directory");
            let context = context();
            let wal = directory.path().join("leader-wire-live-recovery-cut.wal");
            let roster = context
                .roster
                .iter()
                .map(|entry| entry.validator.clone())
                .collect::<BTreeSet<_>>();
            let max_chunks = 1;
            let capacity = LeaderWireLifecycleStoreGate::derived_capacity(roster.len(), max_chunks)
                .expect("derived gate capacity");
            let initial_authority = leader_wire_recovery_authority(&context);
            let (gate, _) = LeaderWireLifecycleStoreGate::open(
                &wal,
                context.id(),
                context.height,
                OWNER_A,
                roster.clone(),
                capacity,
                max_chunks,
                initial_authority,
                &[],
                &[],
            )
            .expect("open gate");
            let token = leader_wire_token(&context, 2, 13, 97, 3);
            gate.admit_ingress(token.clone())
                .expect("persist owner before recovery-cut rollback");
            drop(gate);
            let (gate, restore) = LeaderWireLifecycleStoreGate::open(
                &wal,
                context.id(),
                context.height,
                OWNER_A,
                roster,
                capacity,
                max_chunks,
                initial_authority,
                &[],
                &[],
            )
            .expect("reopen owner as dormant");
            assert_eq!(
                restore.records()[0].status(),
                LeaderWireLifecycleStatus::Dormant
            );
            replace_snapshot_with_directory(&gate);
            assert!(
                gate.advance_recovery_cut(
                    leader_wire_recovery_authority_at(&context, OWNER_A, 3, false),
                    &BTreeSet::from([token.slot.clone()]),
                )
                .is_err()
            );
            let restored = gate.restore().expect("recovery-cut memory rollback");
            assert_eq!(restored.records().len(), 1);
            assert_eq!(restored.records()[0].token(), &token);
            assert_eq!(
                restored.records()[0].status(),
                LeaderWireLifecycleStatus::Dormant
            );
            assert!(
                !gate
                    .identity_is_obsolete(&token.identity)
                    .expect("recovery authority rollback"),
                "failed persistence must roll the process-local cut back too"
            );
        }
    }
    #[test]
    fn snapshot_rejects_corruption_stale_context_and_capacity_exhaustion() {
        let directory = TempDir::new().expect("temporary directory");
        let context = context();
        let wal = directory.path().join("height.wal");
        let (store, _) = ServicedCandidateStore::open_with_capacities(
            &wal,
            context.id(),
            context.height,
            OWNER_A,
            1,
            SERVICED_CANDIDATE_STAGES_PER_LIFECYCLE,
        )
        .expect("open snapshot");
        let records = BTreeMap::from([(key(&context, 0, 1), 0)]);
        store.persist(&records, false).expect("persist record");
        assert!(
            ServicedCandidateStore::open_with_capacities(
                &wal,
                context.id(),
                context.height + 1,
                OWNER_A,
                1,
                SERVICED_CANDIDATE_STAGES_PER_LIFECYCLE,
            )
            .is_err(),
            "stale height is rejected"
        );
        assert!(
            ServicedCandidateStore::open_with_capacities(
                &wal,
                context.id(),
                context.height,
                OWNER_B,
                1,
                SERVICED_CANDIDATE_STAGES_PER_LIFECYCLE,
            )
            .is_err(),
            "a snapshot cannot be transplanted between local validator owners"
        );
        assert!(
            store
                .persist(
                    &BTreeMap::from([(key(&context, 0, 1), 0), (key(&context, 0, 2), 0)]),
                    false,
                )
                .is_err(),
            "capacity exhaustion fails closed instead of evicting A"
        );
        let mut bytes = fs::read(store.path_for_test()).expect("read snapshot");
        let last = bytes.last_mut().expect("nonempty snapshot");
        *last ^= 1;
        fs::write(store.path_for_test(), bytes).expect("corrupt snapshot");
        assert!(
            ServicedCandidateStore::open_with_capacities(
                &wal,
                context.id(),
                context.height,
                OWNER_A,
                1,
                SERVICED_CANDIDATE_STAGES_PER_LIFECYCLE,
            )
            .is_err(),
            "checksum corruption is rejected"
        );
    }
    #[test]
    fn decision_reclamation_is_canonical_only_for_an_empty_snapshot() {
        let directory = TempDir::new().expect("temporary directory");
        let context = context();
        let wal = directory.path().join("decision.wal");
        let (store, _) =
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 2)
                .expect("open snapshot");
        let record = key(&context, 0, 1);
        assert!(
            store.persist(&BTreeMap::from([(record, 0)]), true).is_err(),
            "Decision reclamation cannot coexist with an unreclaimed owner"
        );
        store
            .persist(&BTreeMap::new(), true)
            .expect("publish canonical reclaimed state");
        let (_, restored) =
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 2)
                .expect("restore canonical reclaimed state");
        assert!(restored.records.is_empty());
        assert!(restored.decision_reclaimed);
        let forged = state(
            &store,
            vec![PersistedServicedCandidate {
                key: record,
                service_view: 0,
            }],
            true,
        );
        write_frame(&store, &forged);
        assert!(
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 2).is_err(),
            "a checksummed nonempty Decision-reclaimed mutation fails closed"
        );
        let orphan =
            continuation_record(&context, 1, 1, 1, ProducerContinuationStatus::Terminal, &[]);
        let mut forged_orphan = state(&store, Vec::new(), true);
        forged_orphan.producer_continuations = vec![PersistedProducerContinuation {
            address: orphan.identity.address(),
            record: orphan,
        }];
        write_frame(&store, &forged_orphan);
        assert!(
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 2).is_err(),
            "a Decision-reclaimed snapshot cannot restore an orphan producer high-watermark"
        );
    }
    #[test]
    fn snapshot_rejects_truncation_version_ordering_duplicates_and_oversize() {
        let directory = TempDir::new().expect("temporary directory");
        let context = context();
        let wal = directory.path().join("truncated.wal");
        let (store, _) =
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 2)
                .expect("open truncated fixture");
        let valid = state(
            &store,
            vec![PersistedServicedCandidate {
                key: key(&context, 0, 1),
                service_view: 0,
            }],
            false,
        );
        let mut frame = encode_frame_v4(&valid, store.max_frame_bytes).expect("encode valid frame");
        frame.pop();
        fs::write(store.path_for_test(), frame).expect("write truncated frame");
        assert!(
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 2).is_err()
        );
        let wal = directory.path().join("version.wal");
        let (store, _) =
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 2)
                .expect("open version fixture");
        let valid = state(&store, Vec::new(), false);
        let mut frame = encode_frame_v4(&valid, store.max_frame_bytes).expect("encode valid frame");
        frame[FRAME_MAGIC.len()..FRAME_MAGIC.len() + 2]
            .copy_from_slice(&(FORMAT_VERSION - 1).to_le_bytes());
        fs::write(store.path_for_test(), frame).expect("write old-version frame");
        assert!(
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 2).is_err(),
            "an earlier schema fails closed instead of being guessed"
        );
        for (name, records) in [
            (
                "unordered",
                vec![
                    PersistedServicedCandidate {
                        key: key(&context, 0, 2),
                        service_view: 0,
                    },
                    PersistedServicedCandidate {
                        key: key(&context, 0, 1),
                        service_view: 0,
                    },
                ],
            ),
            (
                "duplicate",
                vec![
                    PersistedServicedCandidate {
                        key: key(&context, 0, 1),
                        service_view: 0,
                    },
                    PersistedServicedCandidate {
                        key: key(&context, 0, 1),
                        service_view: 1,
                    },
                ],
            ),
        ] {
            let wal = directory.path().join(format!("{name}.wal"));
            let (store, _) =
                ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 2)
                    .expect("open ordering fixture");
            write_frame(&store, &state(&store, records, false));
            assert!(
                ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 2)
                    .is_err(),
                "{name} records must be rejected"
            );
        }
        let wal = directory.path().join("oversize.wal");
        let (store, _) =
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 1)
                .expect("open oversize fixture");
        let oversized_len =
            usize::try_from(store.max_frame_bytes + 1).expect("small fixture bound fits usize");
        fs::write(store.path_for_test(), vec![0; oversized_len]).expect("write oversized frame");
        assert!(
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 1).is_err()
        );
    }
    #[test]
    fn v4_rejects_noncanonical_or_over_capacity_producer_tables() {
        let directory = TempDir::new().expect("temporary directory");
        let context = context();
        let first =
            continuation_record(&context, 1, 1, 1, ProducerContinuationStatus::Terminal, &[]);
        let second =
            continuation_record(&context, 2, 2, 2, ProducerContinuationStatus::Terminal, &[]);
        let first_persisted = PersistedProducerContinuation {
            address: first.identity.address(),
            record: first.clone(),
        };
        let second_persisted = PersistedProducerContinuation {
            address: second.identity.address(),
            record: second,
        };
        for (name, continuations) in [
            (
                "producer-unordered",
                vec![second_persisted.clone(), first_persisted.clone()],
            ),
            (
                "producer-duplicate-address",
                vec![first_persisted.clone(), first_persisted.clone()],
            ),
        ] {
            let wal = directory.path().join(format!("{name}.wal"));
            let (store, _) =
                ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 2)
                    .expect("open producer-ordering fixture");
            let mut invalid = state(&store, Vec::new(), false);
            invalid.producer_continuations = continuations;
            write_frame(&store, &invalid);
            assert!(
                ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 2)
                    .is_err(),
                "{name} must fail closed"
            );
        }
        let wal = directory.path().join("active-hash-only.wal");
        let (store, _) =
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 2)
                .expect("open active-record fixture");
        let mut malformed =
            continuation_record(&context, 1, 1, 1, ProducerContinuationStatus::Reserved, &[]);
        malformed.status = ProducerContinuationStatus::Materialized;
        let mut invalid = state(&store, Vec::new(), false);
        invalid.producer_continuations = vec![PersistedProducerContinuation {
            address: malformed.identity.address(),
            record: malformed,
        }];
        write_frame(&store, &invalid);
        assert!(
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 2).is_err(),
            "startup must reject Materialized metadata without an exact successor"
        );
        let wal = directory.path().join("producer-service-mismatch.wal");
        let (store, _) =
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 2)
                .expect("open producer/service binding fixture");
        let terminal =
            continuation_record(&context, 1, 1, 1, ProducerContinuationStatus::Terminal, &[]);
        let mut invalid = state(
            &store,
            vec![PersistedServicedCandidate {
                key: key(&context, 2, 0x71),
                service_view: 2,
            }],
            false,
        );
        invalid.producer_continuations = vec![PersistedProducerContinuation {
            address: terminal.identity.address(),
            record: terminal,
        }];
        write_frame(&store, &invalid);
        assert!(
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 2).is_err(),
            "a terminal producer cannot bind a different serviced identity"
        );
        let wal = directory.path().join("producer-capacity.wal");
        let (store, _) =
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 1)
                .expect("open producer-capacity fixture");
        let mut over_capacity = (0_u8..u8::try_from(SERVICED_CANDIDATE_STAGES_PER_LIFECYCLE)
            .expect("stage count fits u8"))
            .map(|stage| {
                terminal_continuation_at_view(
                    &context,
                    1,
                    u128::from(stage) + 1,
                    stage,
                    2,
                    stage + 1,
                )
            })
            .collect::<Vec<_>>();
        over_capacity.push(terminal_continuation_at_view(&context, 2, 12, 0, 2, 0x40));
        let mut serviced = over_capacity
            .iter()
            .map(|record| PersistedServicedCandidate {
                key: record.identity().candidate(),
                service_view: 2,
            })
            .collect::<Vec<_>>();
        serviced.sort_unstable_by_key(|record| record.key);
        let mut invalid = state(&store, serviced, false);
        invalid.producer_continuations = over_capacity
            .into_iter()
            .map(|record| PersistedProducerContinuation {
                address: record.identity().address(),
                record,
            })
            .collect();
        write_frame(&store, &invalid);
        assert!(
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 1).is_err(),
            "producer-continuation capacity is checked independently"
        );
        let wal = directory.path().join("version-layout-confusion.wal");
        let (store, _) =
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 1)
                .expect("open version-layout fixture");
        let valid = state(&store, Vec::new(), false);
        let mut frame = encode_frame_v4(&valid, store.max_frame_bytes).expect("encode v4 frame");
        frame[FRAME_MAGIC.len()..FRAME_MAGIC.len() + 2]
            .copy_from_slice(&(FORMAT_VERSION - 1).to_le_bytes());
        fs::write(store.path_for_test(), frame).expect("write mismatched version frame");
        assert!(
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 1).is_err(),
            "a v4 payload is never reinterpreted through an earlier decoder"
        );
    }
    #[test]
    fn producer_identity_stage_projection_rejects_foreign_root_and_successor_stages() {
        let context = context();
        for stage in
            0..u8::try_from(SERVICED_CANDIDATE_STAGES_PER_LIFECYCLE).expect("stage count fits u8")
        {
            let identity = continuation_identity(&context, 1, 1, stage, stage + 1);
            assert_eq!(identity.stage(), stage);
            assert_eq!(identity.address().stage(), stage);
            let record = ProducerContinuationRecord::new(
                identity,
                ProducerContinuationStatus::Terminal,
                Vec::new(),
            )
            .expect("tracked stage has one physical replay class");
            let expected_source_class = match stage {
                1..=5 => ProducerContinuationSourceClass::ConditionalTransport,
                7 => ProducerContinuationSourceClass::VolatileBody,
                _ => ProducerContinuationSourceClass::Local,
            };
            assert_eq!(record.source_class(), expected_source_class);
        }
        for untracked_kind in [7, 11, 12, 13, 15, u8::MAX] {
            assert!(
                ProducerContinuationIdentity::new(
                    key_with_kind(&context, 2, untracked_kind, untracked_kind),
                    Hash::new([0xE3, untracked_kind]),
                    1,
                    1,
                )
                .is_err(),
                "untracked event kind {untracked_kind} cannot claim a service stage"
            );
        }
        let directory = TempDir::new().expect("temporary directory");
        let wal = directory.path().join("foreign-root-stage.wal");
        let (store, _) =
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 1)
                .expect("open foreign-stage fixture");
        let mut root = continuation_record(
            &context,
            1,
            1,
            1,
            ProducerContinuationStatus::Reserved,
            &[2],
        );
        root.identity.stage = 2;
        let mut invalid = state(&store, Vec::new(), false);
        invalid.producer_continuations = vec![PersistedProducerContinuation {
            address: root.identity.address(),
            record: root,
        }];
        write_frame(&store, &invalid);
        assert!(
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 1).is_err(),
            "a decoded root cannot occupy a foreign service stage"
        );
        let wal = directory.path().join("foreign-successor-stage.wal");
        let (store, _) =
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 1)
                .expect("open foreign-successor fixture");
        let mut record = continuation_record(
            &context,
            1,
            1,
            1,
            ProducerContinuationStatus::Reserved,
            &[2],
        );
        record.handoff_candidates[0].stage = 3;
        let mut invalid = state(&store, Vec::new(), false);
        invalid.producer_continuations = vec![PersistedProducerContinuation {
            address: record.identity.address(),
            record,
        }];
        write_frame(&store, &invalid);
        assert!(
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 1).is_err(),
            "a decoded successor receives the same exact stage validation"
        );
        let wal = directory.path().join("foreign-source-class.wal");
        let (store, _) =
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 1)
                .expect("open foreign-source-class fixture");
        let mut record = terminal_continuation_at_view(&context, 1, 1, 1, 2, 0x61);
        assert_eq!(
            record.source_class(),
            ProducerContinuationSourceClass::ConditionalTransport
        );
        record.source_class = ProducerContinuationSourceClass::Local;
        let candidate = record.identity().candidate();
        let mut invalid = state(
            &store,
            vec![PersistedServicedCandidate {
                key: candidate,
                service_view: candidate.source_view(),
            }],
            false,
        );
        invalid.producer_continuations = vec![PersistedProducerContinuation {
            address: record.identity().address(),
            record,
        }];
        write_frame(&store, &invalid);
        assert!(
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 1).is_err(),
            "a decoded record cannot strengthen its physical replay source"
        );
    }
    #[test]
    fn bounded_slot_reuse_requires_terminal_strict_view_and_ordinal_advance() {
        let directory = TempDir::new().expect("temporary directory");
        let context = context();
        let wal = directory.path().join("bounded-slot-reuse.wal");
        let (store, _) =
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 1)
                .expect("open bounded-slot fixture");
        let first = terminal_continuation_at_view(&context, 1, 1, 2, 1, 1);
        let mut continuations = BTreeMap::new();
        assert_eq!(
            store
                .reserve_producer_continuation(&mut continuations, first.clone())
                .expect("reserve first bounded address"),
            ProducerContinuationReservation::Inserted
        );
        assert_eq!(
            store
                .reserve_producer_continuation(&mut continuations, first.clone())
                .expect("coalesce exact retry"),
            ProducerContinuationReservation::Coalesced
        );
        let same_view = terminal_continuation_at_view(&context, 1, 2, 2, 1, 2);
        assert!(
            store
                .reserve_producer_continuation(&mut continuations, same_view)
                .is_err(),
            "ordinal advance alone cannot reuse a terminal address"
        );
        let same_ordinal = terminal_continuation_at_view(&context, 1, 1, 2, 2, 3);
        assert!(
            store
                .reserve_producer_continuation(&mut continuations, same_ordinal)
                .is_err(),
            "view advance alone cannot reuse a terminal address"
        );
        for episode in 2_u8..=64 {
            let replacement = terminal_continuation_at_view(
                &context,
                1,
                u128::from(episode),
                2,
                u64::from(episode),
                episode,
            );
            assert_eq!(
                store
                    .reserve_producer_continuation(&mut continuations, replacement)
                    .expect("strictly advance terminal address"),
                ProducerContinuationReservation::ReplacedTerminal
            );
            assert_eq!(
                continuations.len(),
                1,
                "sequential lifecycles reuse one bounded address"
            );
        }
        assert!(
            store
                .reserve_producer_continuation(&mut continuations, first)
                .is_err(),
            "a stale ABA writer cannot replace the newer terminal owner"
        );
        let out_of_geometry = terminal_continuation_at_view(&context, 2, 65, 2, 65, 65);
        assert!(
            store
                .reserve_producer_continuation(&mut continuations, out_of_geometry)
                .is_err(),
            "the allocator slot must remain inside the frozen lifecycle capacity"
        );
        let mut active = terminal_continuation_at_view(&context, 1, 64, 2, 64, 64);
        active.status = ProducerContinuationStatus::Reserved;
        continuations.insert(active.identity.address(), active.clone());
        let later = terminal_continuation_at_view(&context, 1, 65, 2, 65, 65);
        assert!(
            store
                .reserve_producer_continuation(&mut continuations, later)
                .is_err(),
            "a live bounded address is never evicted"
        );
        store
            .persist_with_producer_continuations(&BTreeMap::new(), &continuations, false)
            .expect("persist a live bounded address as restart admission metadata");
        let (_, active_restored) =
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 1)
                .expect("restore live bounded address");
        assert_eq!(
            active_restored.producer_continuations[&active.identity.address()].status(),
            ProducerContinuationStatus::Reserved
        );
        assert_eq!(
            active_restored.producer_continuations[&active.identity.address()]
                .identity()
                .admission_ordinal(),
            active.identity().admission_ordinal()
        );
        continuations
            .values_mut()
            .for_each(|record| record.status = ProducerContinuationStatus::Terminal);
        let serviced = continuations
            .values()
            .map(|record| {
                let candidate = record.identity().candidate();
                (candidate, candidate.source_view())
            })
            .collect::<BTreeMap<_, _>>();
        store
            .persist_with_producer_continuations(&serviced, &continuations, false)
            .expect("persist bounded terminal table");
        let (_, restored) =
            ServicedCandidateStore::open(&wal, context.id(), context.height, OWNER_A, 1)
                .expect("restore bounded terminal table");
        assert_eq!(restored.producer_continuations, continuations);
    }
    include!("serviced_candidate_store_tail_tests.rs");
}
