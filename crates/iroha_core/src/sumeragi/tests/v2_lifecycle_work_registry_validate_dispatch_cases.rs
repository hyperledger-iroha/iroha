    use std::{
        collections::BTreeMap,
        panic::{AssertUnwindSafe, catch_unwind},
    };

    #[cfg(feature = "bls")]
    use std::num::NonZeroU64;

    #[cfg(feature = "bls")]
    use iroha_crypto::{Algorithm, KeyPair, SignatureOf};
    use iroha_crypto::{Hash, HashOf};
    use iroha_data_model::block::consensus_v2 as wire;
    #[cfg(feature = "bls")]
    use iroha_data_model::block::{
        BlockHeader, BlockSignature, CertifiedMergeLedgerReference, SignedBlock,
    };
    #[cfg(feature = "bls")]
    use iroha_data_model::merge::MergeQuorumCertificate;
    #[cfg(feature = "bls")]
    use iroha_data_model::peer::PeerId;
    #[cfg(feature = "bls")]
    use tempfile::TempDir;

    #[cfg(feature = "bls")]
    use super::super::{
        AdmissionDecision, CapacityClass, LifecycleCoordinator, LifecycleState, WaitToken,
        concrete_admission::{
            DurableValidateDispatchError, LifecycleWorkRegistryHolder,
            ReadyValidateDemandAttestationError,
        },
        schema::CapacityGeometry,
    };
    use super::*;
    #[cfg(feature = "bls")]
    use crate::sumeragi::v2_chunks::encode_payload;
    use crate::sumeragi::{
        v2::{ExactLiveWalPersistedContinuationCause, LiveWalFrameIdentity},
        v2_core::{EventTag, Generation},
        v2_runtime::{RuntimeEffectOwnership, bind_adapter_effect_batch_ownership},
    };

    fn effect_at_generation(marker: u8, generation: u64) -> AdapterEffect {
        let tag = EventTag::new(7, 2, Generation::new(generation.max(1)));
        AdapterEffect::StoreBody {
            tag,
            round: wire::ConsensusRound {
                context_id: wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
                    b"lifecycle-work-registry-context",
                ))),
                height: 7,
                view: 2,
            },
            subject: wire::BlockSubject {
                parent_block_hash: None,
                block_hash: HashOf::from_untyped_unchecked(Hash::new([marker, 1])),
                payload_hash: Hash::new([marker, 2]),
            },
        }
    }

    fn effect(marker: u8) -> AdapterEffect {
        effect_at_generation(marker, u64::from(marker))
    }

    fn direct_signed_pending(
        effect: &AdapterEffect,
        tag: EventTag,
        ordinal: u128,
    ) -> PendingRuntimeEffectBinding {
        bind_adapter_effect_batch_ownership(
            core::slice::from_ref(effect),
            vec![RuntimeEffectOwnership::fresh_for_test(tag, ordinal)],
        )
        .expect("bind direct signed registry fixture")
        .pop()
        .expect("one direct signed registry fixture owner")
        .pending_adapter_effect_binding(effect)
        .expect("mint direct signed pending binding")
    }

    fn direct_signed_vote(marker: u8, subject_marker: u8) -> wire::Vote {
        let context_id =
            wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new([marker, 0xD1])));
        let round = wire::ConsensusRound {
            context_id,
            height: 7,
            view: 2,
        };
        wire::Vote {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Prepare,
            subject: wire::BlockSubject {
                parent_block_hash: None,
                block_hash: HashOf::from_untyped_unchecked(Hash::new([subject_marker, 0xD2])),
                payload_hash: Hash::new([subject_marker, 0xD3]),
            },
            execution_commitment: wire::ExecutionCommitment::without_topups_or_merge_carrier(
                Hash::new([marker, 0xD4]),
                Hash::new([marker, 0xD5]),
                Hash::new([marker, 0xD6]),
                1,
                Hash::new([marker, 0xD7]),
            ),
            signer: 0,
            signature: vec![subject_marker, 0xD8],
        }
    }

    fn recovered_wal_projection_candidate(
        phase: LifecyclePhase,
        work_class: LifecycleWorkClass,
        stage_kind: LifecycleStageKind,
        marker: u8,
    ) -> CandidateAdmission {
        let context = LifecycleContext::new(LifecycleDigest::new([0x31; 32]), 7);
        let replay =
            super::super::replay_authority::exact_record_fixture(context, stage_kind, marker);
        assert_eq!((replay.key.phase(), replay.work_class), (phase, work_class));
        let root = super::super::CausalRoot::new(LifecycleDigest::new([0x34; 32]));
        let slot = PhysicalSlotId::for_capacity(CapacityClass::Effect, 0);
        CandidateAdmission::new(
            replay.key,
            root,
            work_class,
            LifecycleStage::new(stage_kind, PredecessorScope::Independent),
            InitialLifecycleState::Ready,
            root.digest(),
            replay.payload,
            replay.authority,
            super::super::PhysicalGeometry::new(
                [PhysicalSlot::new(slot, LifecycleDigest::new([marker; 32]))],
                [slot],
            ),
            None,
        )
    }

    #[test]
    fn recovered_wal_projection_never_overwrites_foreign_opposite_key_occupants() {
        let parent = recovered_wal_projection_candidate(
            LifecyclePhase::Validate,
            LifecycleWorkClass::Validate,
            LifecycleStageKind::ValidateBody,
            0x41,
        );
        let child = recovered_wal_projection_candidate(
            LifecyclePhase::Prepare,
            LifecycleWorkClass::SignVote,
            LifecycleStageKind::SignPrepareVote,
            0x42,
        );
        let owner = OwnerId::new(parent.causal_root, 1);
        let effect_slot = PhysicalSlotId::for_capacity(CapacityClass::Effect, 0);
        let projection = AuthenticatedRecoveredWalSignProjection {
            parent: parent.clone(),
            child: child.clone(),
            parent_address: ConcreteWorkAddress::new(owner, 1, effect_slot)
                .expect("exact recovered parent address"),
            child_address: ConcreteWorkAddress::new(owner, 2, effect_slot)
                .expect("exact recovered child address"),
        };

        let mut foreign_child = child.clone();
        foreign_child.reconstruction_source = LifecycleDigest::new([0x51; 32]);
        let mut parent_with_foreign_child =
            BTreeMap::from([(parent.key, parent.clone()), (child.key, foreign_child)]);
        let before = parent_with_foreign_child.clone();
        assert!(!projection.splice_candidates(&mut parent_with_foreign_child));
        assert_eq!(parent_with_foreign_child, before);

        let mut foreign_parent = parent.clone();
        foreign_parent.reconstruction_source = LifecycleDigest::new([0x52; 32]);
        let mut child_with_foreign_parent =
            BTreeMap::from([(parent.key, foreign_parent), (child.key, child)]);
        let before = child_with_foreign_parent.clone();
        assert!(!projection.splice_candidates(&mut child_with_foreign_parent));
        assert_eq!(child_with_foreign_parent, before);
    }

    fn concrete(effect: AdapterEffect, legacy_ordinal: u128) -> ConcreteLifecycleWork {
        let tag = match &effect {
            AdapterEffect::StoreBody { tag, .. } => *tag,
            _ => unreachable!("registry fixture uses one StoreBody effect"),
        };
        let ownership = bind_adapter_effect_batch_ownership(
            core::slice::from_ref(&effect),
            vec![RuntimeEffectOwnership::fresh_for_test(tag, legacy_ordinal)],
        )
        .expect("bind exact registry fixture")
        .pop()
        .expect("one registry fixture owner");
        let pending = ownership
            .pending_adapter_effect_binding(&effect)
            .expect("mint pending registry fixture");
        ConcreteLifecycleWork::from_exact(effect, pending).expect("construct exact concrete work")
    }

    fn owner(seed: u8, first_ordinal: u128) -> OwnerId {
        OwnerId::new(
            super::super::CausalRoot::new(LifecycleDigest::new([seed; 32])),
            first_ordinal,
        )
    }

    fn admitted_owner(work: &ConcreteLifecycleWork, first_ordinal: u128) -> OwnerId {
        OwnerId::new(work.causal_root(), first_ordinal)
    }

    fn key(seed: u8) -> super::super::LifecycleKey {
        super::super::LifecycleKey::new(
            LifecycleDigest::new([seed; 32]),
            super::super::LifecycleRound::new(7, 2),
            Some(super::super::LifecycleRound::new(7, 2)),
            Some(LifecycleDigest::new([seed.wrapping_add(1); 32])),
            super::super::LifecyclePhase::Store,
            None,
        )
    }

    fn lease(
        owner: OwnerId,
        ordinal: u128,
        slot: PhysicalSlotId,
        digest: LifecycleDigest,
    ) -> TurnLease {
        TurnLease {
            id: super::super::LeaseId(1),
            ordinal,
            owner,
            key: key(u8::try_from(ordinal).unwrap_or(0)),
            work_class: super::super::LifecycleWorkClass::Store,
            stage: super::super::LifecycleStage::new(
                super::super::LifecycleStageKind::StoreBody,
                super::super::PredecessorScope::Independent,
            ),
            rank: super::super::SchedulerRank::new(4, 0, 0, 0, 0, 0, 0, 0),
            physical_slots: BTreeMap::from([(slot, digest)]),
            output_reservation: None,
        }
    }

    fn fetch_lease(
        owner: OwnerId,
        ordinal: u128,
        slot: PhysicalSlotId,
        digest: LifecycleDigest,
    ) -> TurnLease {
        TurnLease {
            id: super::super::LeaseId(2),
            ordinal,
            owner,
            key: super::super::LifecycleKey::new(
                LifecycleDigest::new([u8::try_from(ordinal).unwrap_or(0); 32]),
                super::super::LifecycleRound::new(7, 2),
                Some(super::super::LifecycleRound::new(7, 2)),
                Some(LifecycleDigest::new([0xA5; 32])),
                super::super::LifecyclePhase::Fetch,
                None,
            ),
            work_class: super::super::LifecycleWorkClass::Fetch,
            stage: super::super::LifecycleStage::new(
                super::super::LifecycleStageKind::FetchBody,
                super::super::PredecessorScope::Independent,
            ),
            rank: super::super::SchedulerRank::new(5, 0, 0, 0, 0, 0, 0, 0),
            physical_slots: BTreeMap::from([(slot, digest)]),
            output_reservation: None,
        }
    }

    #[cfg(feature = "bls")]
    struct DurableStoreFixture {
        registry: ConcreteLifecycleWorkRegistry,
        verified: VerifiedHeightContext,
        address: ConcreteWorkAddress,
        lease: TurnLease,
        slot: PhysicalSlotId,
        effect: AdapterEffect,
        expected_manifest_hash: HashOf<wire::PayloadManifest>,
    }

    #[cfg(feature = "bls")]
    struct DurableValidateFixture {
        registry: ConcreteLifecycleWorkRegistry,
        verified: VerifiedHeightContext,
        address: ConcreteWorkAddress,
        lease: TurnLease,
        slot: PhysicalSlotId,
        effect: AdapterEffect,
        expected_manifest_hash: HashOf<wire::PayloadManifest>,
        canonical_wire: Vec<u8>,
        manifest: wire::PayloadManifest,
        store_ownership: RuntimeEffectOwnership,
    }

    #[cfg(feature = "bls")]
    fn durable_store_keys(marker: u8) -> Vec<KeyPair> {
        let mut keys = (1_u8..=4)
            .map(|seed| {
                KeyPair::try_from_seed(vec![seed.wrapping_add(marker); 32], Algorithm::BlsNormal)
                    .expect("deterministic durable Store BLS key")
            })
            .collect::<Vec<_>>();
        keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
        keys
    }

    #[cfg(feature = "bls")]
    fn verified_store_context(marker: u8) -> (VerifiedHeightContext, wire::HeightContext) {
        let keys = durable_store_keys(marker);
        let proofs = keys
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("durable Store proof of possession")
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
            network_id: crate::sumeragi::synthetic_network_id(&format!(
                "durable-store-registry-{marker}"
            )),
            protocol_version: wire::PROTOCOL_VERSION,
            height: 1,
            epoch: 1,
            epoch_end_height: 100,
            next_epoch_snapshot: None,
            mode: wire::ConsensusMode::Permissioned,
            parent_commit_qc: None,
            snapshot_bootstrap: None,
            quorum: wire::DualQuorum::from_roster(&roster).expect("durable Store fixture quorum"),
            roster,
            nexus_amx_context_hash: Hash::new([marker, 0xA1]),
            execution_policy_hash: Hash::new([marker, 0xA2]),
            da_layout: wire::DataAvailabilityLayout {
                encoding: wire::PayloadEncoding::ReedSolomon16,
                chunk_size_bytes: 1024,
                data_shards: 1,
                parity_shards: 1,
                max_payload_size_bytes: 512 * 1024,
                max_chunk_count: 1024,
            },
            leader_seed: [marker; 32],
        };
        let verified = VerifiedHeightContext::genesis(context.clone(), proofs)
            .expect("verified durable Store height context");
        (verified, context)
    }

    #[cfg(feature = "bls")]
    #[allow(clippy::too_many_lines)]
    fn durable_store_fixture(marker: u8) -> DurableStoreFixture {
        let (verified, context) = verified_store_context(marker);
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 2,
        };
        let tag = EventTag::new(
            round.height,
            round.view,
            Generation::new(u64::from(marker) + 1),
        );
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::new([marker, 0xB1])),
            payload_hash: Hash::new([marker, 0xB2]),
        };
        let manifest = wire::PayloadManifest {
            round,
            subject,
            payload_size_bytes: 1,
            layout: context.da_layout,
            chunk_hashes: vec![Hash::new([marker, 0xC1])],
            chunk_root: Hash::new([marker, 0xC2]),
        };
        let expected_manifest_hash = HashOf::new(&manifest);
        let durable_receipt =
            DurableBodyReceipt::for_test(round.context_id, round, subject, expected_manifest_hash);
        let fetch_effect = AdapterEffect::FetchBody {
            tag,
            round,
            subject,
            manifest: Some(manifest.clone()),
            certified_sources: Vec::new(),
            certificate: Some(certified_pipeline_prepare_certificate_for_test(
                &manifest,
                &durable_receipt,
            )),
        };
        let effect = AdapterEffect::StoreBody {
            tag,
            round,
            subject,
        };
        let fetch_ownership = bind_adapter_effect_batch_ownership(
            core::slice::from_ref(&fetch_effect),
            vec![RuntimeEffectOwnership::fresh_for_test(
                tag,
                u128::from(marker) + 1,
            )],
        )
        .expect("bind exact certified Fetch fixture")
        .pop()
        .expect("one certified Fetch fixture owner");
        let ownership = fetch_ownership
            .rebind_as_inherited_adapter_effect(&effect)
            .expect("carry certified Fetch authority into Store");
        let pending = ownership
            .pending_adapter_effect_binding(&effect)
            .expect("mint sealed durable Store binding");
        let validate_effect = AdapterEffect::ValidateBody {
            tag,
            round,
            subject,
        };
        let validate_pending = pending
            .project_store_validate_successor(&effect, &validate_effect)
            .expect("project exact certified Validate fixture pending");
        let (replay_evidence, _validate_evidence) = certified_pipeline_replay_evidence_for_test(
            tag,
            &manifest,
            &durable_receipt,
            &validate_pending,
        )
        .expect("build exact certified Store replay evidence");
        let candidate = replay_evidence
            .project_installed_store_candidate(
                InstalledBodyCandidateProjectionPermit::new(),
                &verified,
                &effect,
                &durable_receipt,
                &pending,
            )
            .expect("project exact replay-authorized durable Store fixture");
        let (physical_slots, slot_universe, consumed_slots) = candidate
            .physical_geometry
            .normalized()
            .expect("normalize durable Store fixture geometry");
        assert_eq!(slot_universe, consumed_slots);
        assert_eq!(physical_slots.len(), 1);
        let (&slot, &digest) = physical_slots
            .first_key_value()
            .expect("one durable Store fixture slot");
        let ordinal = u128::from(marker) + 1;
        let owner = OwnerId::new(candidate.causal_root, ordinal);
        let address = ConcreteWorkAddress::new(owner, ordinal, slot)
            .expect("exact durable Store registry address");
        let lease = TurnLease {
            id: super::super::LeaseId(u128::from(marker) + 1),
            ordinal,
            owner,
            key: candidate.key,
            work_class: candidate.work_class,
            stage: candidate.stage,
            rank: super::super::SchedulerRank::new(4, 0, 0, 0, 0, 0, 0, 0),
            physical_slots,
            output_reservation: None,
        };
        let store = DurableStoreBody {
            address,
            effect: effect.clone(),
            pending,
            durable_receipt,
            expected_manifest_hash,
            replay_evidence,
        };
        assert!(store.validates(digest));
        let work = ConcreteLifecycleWork {
            digest,
            kind: ConcreteLifecycleWorkKind::DurableStoreBody(store),
        };
        assert!(work.validate_exact());
        assert!(work.validates_at(address));
        assert_eq!(work.effect(), &effect);
        assert_eq!(work.causal_root(), owner.causal_root());
        let mut registry = ConcreteLifecycleWorkRegistry::default();
        assert!(registry.entries.insert(address, work).is_none());
        DurableStoreFixture {
            registry,
            verified,
            address,
            lease,
            slot,
            effect,
            expected_manifest_hash,
        }
    }

    #[cfg(feature = "bls")]
    #[allow(clippy::too_many_lines)]
    fn durable_validate_fixture(marker: u8) -> DurableValidateFixture {
        let (verified, context) = verified_store_context(marker);
        let keys = durable_store_keys(marker);
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 2,
        };
        let tag = EventTag::new(
            round.height,
            round.view,
            Generation::new(u64::from(marker) + 1),
        );
        let leader = context.leader(round.view);
        let leader_index = usize::try_from(leader).expect("durable Validate leader index");
        let header = BlockHeader::new(
            NonZeroU64::new(round.height).expect("non-zero durable Validate height"),
            None,
            None,
            None,
            1_000,
            round.view,
        );
        let signature = SignatureOf::try_from_hash(keys[leader_index].private_key(), header.hash())
            .expect("sign durable Validate fixture body");
        let block = SignedBlock::presigned(
            BlockSignature::new(u64::from(leader), signature),
            header,
            Vec::new(),
        );
        let canonical_wire = block
            .encode_wire()
            .expect("encode durable Validate fixture body");
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: block.hash(),
            payload_hash: Hash::new(&canonical_wire),
        };
        let manifest = encode_payload(&context, round, subject, &canonical_wire)
            .expect("encode durable Validate fixture payload")
            .manifest()
            .clone();
        let expected_manifest_hash = HashOf::new(&manifest);
        let durable_receipt =
            DurableBodyReceipt::for_test(round.context_id, round, subject, expected_manifest_hash);
        let fetch_effect = AdapterEffect::FetchBody {
            tag,
            round,
            subject,
            manifest: Some(manifest.clone()),
            certified_sources: Vec::new(),
            certificate: Some(certified_pipeline_prepare_certificate_for_test(
                &manifest,
                &durable_receipt,
            )),
        };
        let store_effect = AdapterEffect::StoreBody {
            tag,
            round,
            subject,
        };
        let fetch_ownership = bind_adapter_effect_batch_ownership(
            core::slice::from_ref(&fetch_effect),
            vec![RuntimeEffectOwnership::fresh_for_test(
                tag,
                u128::from(marker) + 1,
            )],
        )
        .expect("bind exact certified Validate Fetch fixture")
        .pop()
        .expect("one certified Validate Fetch fixture owner");
        let ownership = fetch_ownership
            .rebind_as_inherited_adapter_effect(&store_effect)
            .expect("carry certified Fetch authority into Validate parent Store");
        let store_pending = ownership
            .pending_adapter_effect_binding(&store_effect)
            .expect("mint sealed durable Validate parent binding");
        let effect = AdapterEffect::ValidateBody {
            tag,
            round,
            subject,
        };
        let pending = store_pending
            .project_store_validate_successor(&store_effect, &effect)
            .expect("project exact Store-to-Validate fixture lineage");
        assert_eq!(
            pending.causal_lifecycle_key(),
            store_pending.causal_lifecycle_key()
        );
        assert_eq!(
            pending.candidate_statement(),
            store_pending.candidate_statement()
        );
        assert_ne!(
            pending.exact_effect_identity(),
            store_pending.exact_effect_identity()
        );
        let (_store_evidence, replay_evidence) =
            certified_pipeline_replay_evidence_for_test(tag, &manifest, &durable_receipt, &pending)
                .expect("build exact certified Validate replay evidence");
        let replay_evidence = DurableValidateReplayEvidenceV1::certified(replay_evidence);
        let candidate = replay_evidence
            .project_installed_validate_candidate(
                InstalledBodyCandidateProjectionPermit::new(),
                &verified,
                &effect,
                &durable_receipt,
                &pending,
            )
            .expect("project exact replay-authorized durable Validate fixture");
        let (physical_slots, slot_universe, consumed_slots) = candidate
            .physical_geometry
            .normalized()
            .expect("normalize durable Validate fixture geometry");
        assert_eq!(slot_universe, consumed_slots);
        assert_eq!(physical_slots.len(), 1);
        let (&slot, &digest) = physical_slots
            .first_key_value()
            .expect("one durable Validate fixture slot");
        let ordinal = u128::from(marker) + 1;
        let owner = OwnerId::new(candidate.causal_root, ordinal);
        let address = ConcreteWorkAddress::new(owner, ordinal, slot)
            .expect("exact durable Validate registry address");
        let lease = TurnLease {
            id: super::super::LeaseId(u128::from(marker) + 1),
            ordinal,
            owner,
            key: candidate.key,
            work_class: candidate.work_class,
            stage: candidate.stage,
            rank: super::super::SchedulerRank::new(3, 0, 0, 0, 0, 0, 0, 0),
            physical_slots,
            output_reservation: None,
        };
        let validate = DurableValidateBody {
            address,
            effect: effect.clone(),
            pending,
            durable_receipt,
            expected_manifest_hash,
            replay_evidence,
        };
        assert!(validate.validates(digest));
        let work = ConcreteLifecycleWork {
            digest,
            kind: ConcreteLifecycleWorkKind::DurableValidateBody(validate),
        };
        assert!(work.validate_exact());
        assert!(work.validates_at(address));
        assert_eq!(work.effect(), &effect);
        assert_eq!(work.causal_root(), owner.causal_root());
        let mut registry = ConcreteLifecycleWorkRegistry::default();
        assert!(registry.entries.insert(address, work).is_none());
        DurableValidateFixture {
            registry,
            verified,
            address,
            lease,
            slot,
            effect,
            expected_manifest_hash,
            canonical_wire,
            manifest,
            store_ownership: ownership,
        }
    }

    #[cfg(feature = "bls")]
    #[derive(Debug)]
    #[allow(variant_size_differences, clippy::large_enum_variant)]
    enum DetachedValidationError {
        Invalid(&'static str),
        MissingMergeSidecar(CertifiedMergeLedgerReference),
    }

    #[cfg(feature = "bls")]
    impl std::fmt::Display for DetachedValidationError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::Invalid(reason) => formatter.write_str(reason),
                Self::MissingMergeSidecar(reference) => {
                    write!(formatter, "missing merge sidecar {}", reference.entry_hash)
                }
            }
        }
    }

    #[cfg(feature = "bls")]
    impl BodyValidationError for DetachedValidationError {
        fn missing_certified_merge_sidecar(&self) -> Option<&CertifiedMergeLedgerReference> {
            match self {
                Self::MissingMergeSidecar(reference) => Some(reference),
                Self::Invalid(_) => None,
            }
        }
    }

    #[cfg(feature = "bls")]
    fn detached_validation_merge_reference(
        durable: &DurableBodyReceipt,
    ) -> CertifiedMergeLedgerReference {
        CertifiedMergeLedgerReference {
            version: 1,
            entry_hash: HashOf::from_untyped_unchecked(Hash::new(
                b"detached Validate missing merge sidecar",
            )),
            encoded_len: 512,
            epoch_id: 7,
            execution_batch_hash: None,
            entrypoint_count: None,
            entrypoint_merkle_root: None,
            result_merkle_root: None,
            base_state_height: None,
            base_state_hash: None,
            merge_qc: MergeQuorumCertificate::new(
                durable.round().view,
                7,
                durable.round().height,
                HashOf::from_untyped_unchecked(Hash::new(b"detached Validate merge parent")),
                Hash::new(b"detached Validate merge chain"),
                1,
                HashOf::new(&Vec::<PeerId>::new()),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Hash::new(b"detached Validate merge certificate"),
            ),
        }
    }

    #[cfg(feature = "bls")]
    fn durable_validate_store_fixture(
        marker: u8,
    ) -> (
        DurableValidateFixture,
        TempDir,
        V2BodyStore,
        DurableBodyReceipt,
    ) {
        let mut fixture = durable_validate_fixture(marker);
        let directory = TempDir::new().expect("temporary detached Validate body store");
        let mut store = V2BodyStore::open(directory.path(), fixture.verified.context().clone())
            .expect("open detached Validate body store");
        let durable = store
            .store(fixture.manifest.clone(), fixture.canonical_wire.clone())
            .expect("persist detached Validate fixture body");
        assert_eq!(durable.manifest_hash(), fixture.expected_manifest_hash);
        let work = fixture
            .registry
            .entries
            .get_mut(&fixture.address)
            .expect("detached Validate fixture retains its closed row");
        let digest = work.digest;
        let ConcreteLifecycleWorkKind::DurableValidateBody(validate) = &mut work.kind else {
            unreachable!("detached Validate fixture retains one closed Validate")
        };
        validate.durable_receipt = durable.clone();
        assert!(validate.validates(digest));
        assert!(work.validates_at(fixture.address));
        (fixture, directory, store, durable)
    }

    #[cfg(feature = "bls")]
    fn seal_validate_fixture_commitment(
        fixture: &mut DurableValidateFixture,
        execution_commitment: wire::ExecutionCommitment,
    ) {
        let AdapterEffect::ValidateBody {
            tag,
            round,
            subject,
        } = fixture.effect.clone()
        else {
            unreachable!("fixture retains one Validate effect")
        };
        let store_effect = AdapterEffect::StoreBody {
            tag,
            round,
            subject,
        };
        let certified_fetch = AdapterEffect::FetchBody {
            tag,
            round,
            subject,
            manifest: Some(fixture.manifest.clone()),
            certified_sources: Vec::new(),
            certificate: Some(wire::QuorumCertificate {
                round,
                proposal_round: round,
                phase: wire::GlobalPhase::Commit,
                subject,
                execution_commitment,
                signers: Vec::new(),
                aggregate_signature: Vec::new(),
            }),
        };
        let certified_fetch_owner = bind_adapter_effect_batch_ownership(
            core::slice::from_ref(&certified_fetch),
            vec![RuntimeEffectOwnership::fresh_for_test(tag, 60_001)],
        )
        .expect("bind one commitment-authorized Fetch")
        .pop()
        .expect("one commitment-authorized Fetch owner");
        let incoming_store_owner = certified_fetch_owner
            .rebind_as_inherited_adapter_effect(&store_effect)
            .expect("carry commitment authority into Store");
        let adopted_store_owner = fixture
            .store_ownership
            .adopt_incumbent_body_stage_for_retry_or_authority(&incoming_store_owner, &store_effect)
            .expect("retain physical Store owner while sealing commitment authority");
        let upgraded_store = adopted_store_owner
            .pending_adapter_effect_binding(&store_effect)
            .expect("mint commitment-authorized Store binding");
        let upgraded_validate = upgraded_store
            .project_store_validate_successor(&store_effect, &fixture.effect)
            .expect("carry commitment authority into Validate");

        let work = fixture
            .registry
            .entries
            .get_mut(&fixture.address)
            .expect("commitment fixture retains exact Validate row");
        let digest = work.digest;
        let ConcreteLifecycleWorkKind::DurableValidateBody(validate) = &mut work.kind else {
            unreachable!("commitment fixture retains one closed Validate")
        };
        let (_store_replay, validate_replay) = certified_pipeline_replay_evidence_for_test(
            tag,
            &fixture.manifest,
            &validate.durable_receipt,
            &upgraded_validate,
        )
        .expect("rebind certified Validate replay to upgraded pending authority");
        validate.pending = upgraded_validate;
        validate.replay_evidence = DurableValidateReplayEvidenceV1::certified(validate_replay);
        assert!(validate.validates(digest));

        let candidate = validate
            .project_candidate(&fixture.verified)
            .expect("project commitment-authorized Validate fixture");
        assert!(fixture.registry.entries[&fixture.address].validates_at(fixture.address));
        assert_eq!(candidate.causal_root, fixture.lease.owner().causal_root());
        assert_eq!(candidate.work_class, fixture.lease.work_class());
        assert_eq!(candidate.stage, fixture.lease.stage());
        assert_eq!(
            candidate
                .physical_geometry
                .normalized()
                .expect("normalize commitment-authorized Validate geometry")
                .0,
            *fixture.lease.physical_slots()
        );
        fixture.lease.key = candidate.key;
    }

    #[cfg(feature = "bls")]
    fn claimed_durable_validate_coordinator(
        fixture: &DurableValidateFixture,
    ) -> LifecycleCoordinator {
        let work = fixture
            .registry
            .entries
            .get(&fixture.address)
            .expect("dispatch fixture retains its closed Validate row");
        let ConcreteLifecycleWorkKind::DurableValidateBody(validate) = &work.kind else {
            unreachable!("dispatch fixture retains one closed Validate carrier")
        };
        let context = fixture.verified.context();
        let mut context_id = [0_u8; 32];
        context_id.copy_from_slice(context.id().0.as_ref());
        let active_context = LifecycleContext::new(
            LifecycleDigest::new(context_id),
            fixture.lease.key().round().height(),
        );
        let candidate = validate
            .project_candidate(&fixture.verified)
            .expect("project dispatch fixture Validate carrier");
        let high_water = fixture
            .lease
            .ordinal()
            .checked_sub(1)
            .expect("dispatch fixture ordinal is non-zero");
        let mut coordinator = LifecycleCoordinator::new(
            active_context,
            high_water,
            CapacityGeometry::new(CapacityClass::ALL.into_iter().map(|class| (class, 64))),
        );
        assert!(matches!(
            coordinator.reduce_admit(AdmissionRequest::Candidate(candidate)),
            AdmissionDecision::Admitted {
                owner,
                ordinal,
                producer_turn_ordinal: None,
            } if owner == fixture.lease.owner() && ordinal == fixture.lease.ordinal()
        ));
        coordinator.ready_index.remove(&fixture.lease.ordinal());
        coordinator
            .records
            .get_mut(&fixture.lease.ordinal())
            .expect("dispatch fixture admitted its Validate row")
            .state = LifecycleState::Claimed(fixture.lease.id());
        coordinator.active_lease = Some(fixture.lease.clone());
        coordinator
    }

    #[cfg(feature = "bls")]
    fn durable_validation_source(fixture: &mut DurableValidateFixture) -> WaitSource {
        let prepared = fixture
            .registry
            .prepare_durable_validate_execution(&fixture.lease, fixture.slot, &fixture.verified)
            .expect("prepare dispatch fixture source");
        prepared.durable_validation_wait_source()
    }

    #[cfg(feature = "bls")]
    fn take_dispatch_registry(fixture: &mut DurableValidateFixture) -> LifecycleWorkRegistryHolder {
        LifecycleWorkRegistryHolder::from_registry_for_test(core::mem::take(&mut fixture.registry))
    }

    #[cfg(feature = "bls")]
    struct WaitingDurableValidateFixture {
        fixture: DurableValidateFixture,
        _directory: TempDir,
        store: V2BodyStore,
        durable: DurableBodyReceipt,
        coordinator: LifecycleCoordinator,
        holder: LifecycleWorkRegistryHolder,
        dispatch: DurableValidateDispatch,
    }

    #[cfg(feature = "bls")]
    fn waiting_durable_validate_fixture(marker: u8) -> WaitingDurableValidateFixture {
        let (mut fixture, directory, store, durable) = durable_validate_store_fixture(marker);
        let mut coordinator = claimed_durable_validate_coordinator(&fixture);
        let mut holder = take_dispatch_registry(&mut fixture);
        let dispatch = coordinator
            .begin_durable_validate_dispatch(&mut holder, fixture.lease.clone(), &fixture.verified)
            .expect("exact claimed Validate becomes one waiting dispatch");
        WaitingDurableValidateFixture {
            fixture,
            _directory: directory,
            store,
            durable,
            coordinator,
            holder,
            dispatch,
        }
    }

    #[cfg(feature = "bls")]
    #[derive(Clone, Copy)]
    enum ReadyDurableValidateFixtureOutcome {
        Validated,
        Rejected,
    }

    #[cfg(feature = "bls")]
    struct ReadyDurableValidateFixture {
        fixture: DurableValidateFixture,
        _directory: TempDir,
        holder: LifecycleWorkRegistryHolder,
        lease: TurnLease,
        durable: DurableBodyReceipt,
    }

    #[cfg(feature = "bls")]
    fn ready_durable_validate_fixture(
        marker: u8,
        outcome: ReadyDurableValidateFixtureOutcome,
    ) -> ReadyDurableValidateFixture {
        let WaitingDurableValidateFixture {
            fixture,
            _directory,
            mut store,
            durable,
            mut coordinator,
            mut holder,
            dispatch,
        } = waiting_durable_validate_fixture(marker);
        let executed = match outcome {
            ReadyDurableValidateFixtureOutcome::Validated => {
                let commitment =
                    ValidatedBodyReceipt::for_test(durable.clone()).execution_commitment();
                dispatch
                    .execute(&mut store, |_| Ok::<_, DetachedValidationError>(commitment))
                    .expect("execute successful Ready Validate fixture")
            }
            ReadyDurableValidateFixtureOutcome::Rejected => dispatch
                .execute(&mut store, |_| {
                    Err::<wire::ExecutionCommitment, _>(DetachedValidationError::Invalid(
                        "Ready Validate rejection diagnostic",
                    ))
                })
                .expect("execute rejected Ready Validate fixture"),
        };
        coordinator
            .complete_durable_validate_dispatch(&mut holder, executed)
            .expect("publish Ready Validate completion fixture");
        let replacement_digest = holder.registry_for_test().entries[&fixture.address].digest;
        let mut lease = fixture.lease.clone();
        assert_eq!(
            lease
                .physical_slots
                .insert(fixture.slot, replacement_digest),
            Some(fixture.lease.physical_slots()[&fixture.slot])
        );
        lease.output_reservation = match outcome {
            ReadyDurableValidateFixtureOutcome::Validated => None,
            ReadyDurableValidateFixtureOutcome::Rejected => {
                Some(super::super::schema::LeaseCapacityReservation::new(
                    CapacityClass::Consensus,
                    coordinator.capacity_generation[&CapacityClass::Consensus],
                ))
            }
        };
        assert_eq!(
            coordinator.records[&lease.ordinal()].state,
            LifecycleState::Ready
        );
        ReadyDurableValidateFixture {
            fixture,
            _directory,
            holder,
            lease,
            durable,
        }
    }

    #[cfg(feature = "bls")]
    #[test]
    fn durable_validate_dispatch_moves_claim_to_current_external_wait_and_executes() {
        let (mut fixture, _directory, mut store, durable) = durable_validate_store_fixture(0xB0);
        let source = durable_validation_source(&mut fixture);
        let mut coordinator = claimed_durable_validate_coordinator(&fixture);
        coordinator.observed_generation.insert(source, 7);
        let mut holder = take_dispatch_registry(&mut fixture);
        let registry_before = format!("{:?}", holder.registry_for_test());
        let commitment = ValidatedBodyReceipt::for_test(durable.clone()).execution_commitment();

        let dispatch = coordinator
            .begin_durable_validate_dispatch(&mut holder, fixture.lease.clone(), &fixture.verified)
            .expect("exact claimed Validate becomes one dispatch");
        let wait = dispatch.wait_token_for_test();
        assert_eq!(wait, WaitToken::new(source, 7));
        assert!(coordinator.active_lease.is_none());
        assert_eq!(
            coordinator.records[&fixture.lease.ordinal()].state,
            LifecycleState::Waiting(wait)
        );
        assert_eq!(coordinator.observed_generation.get(&source), Some(&7));
        assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);

        let executed = dispatch
            .execute(&mut store, |_| Ok::<_, DetachedValidationError>(commitment))
            .expect("execute exact waiting Validate request");
        assert_eq!(executed.wait_token_for_test(), wait);
        assert_eq!(executed.outcome().durable_body(), &durable);
        assert_eq!(
            executed
                .outcome()
                .validated_receipt()
                .map(ValidatedBodyReceipt::execution_commitment),
            Some(commitment)
        );
        assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
    }

    #[cfg(feature = "bls")]
    #[test]
    fn dropping_unexecuted_durable_validate_dispatch_preserves_wait_and_registry() {
        let (mut fixture, _directory, _store, _durable) = durable_validate_store_fixture(0xB1);
        let mut coordinator = claimed_durable_validate_coordinator(&fixture);
        let mut holder = take_dispatch_registry(&mut fixture);
        let registry_before = format!("{:?}", holder.registry_for_test());

        let dispatch = coordinator
            .begin_durable_validate_dispatch(&mut holder, fixture.lease.clone(), &fixture.verified)
            .expect("exact claimed Validate becomes one dispatch");
        let wait = dispatch.wait_token_for_test();
        drop(dispatch);

        assert!(coordinator.active_lease.is_none());
        assert_eq!(
            coordinator.records[&fixture.lease.ordinal()].state,
            LifecycleState::Waiting(wait)
        );
        assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
    }

    #[cfg(feature = "bls")]
    #[test]
    fn committed_durable_validate_dispatch_cannot_mint_a_second_request() {
        let (mut fixture, _directory, _store, _durable) = durable_validate_store_fixture(0xB2);
        let mut coordinator = claimed_durable_validate_coordinator(&fixture);
        let mut holder = take_dispatch_registry(&mut fixture);
        let registry_before = format!("{:?}", holder.registry_for_test());
        let lease = fixture.lease.clone();

        let dispatch = coordinator
            .begin_durable_validate_dispatch(&mut holder, lease.clone(), &fixture.verified)
            .expect("first exact claimed Validate mints one dispatch");
        let coordinator_after = format!("{coordinator:?}");
        let Err((error, returned_lease)) = coordinator.begin_durable_validate_dispatch(
            &mut holder,
            lease.clone(),
            &fixture.verified,
        ) else {
            panic!("waiting Validate must not mint a second dispatch")
        };
        assert_eq!(error, DurableValidateDispatchError::StaleLease);
        assert_eq!(returned_lease, lease);
        assert_eq!(format!("{coordinator:?}"), coordinator_after);
        assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
        drop(dispatch);
    }

    #[cfg(feature = "bls")]
    #[test]
    fn durable_validate_store_error_returns_the_exact_dispatch() {
        let (mut fixture, _directory, mut store, durable) = durable_validate_store_fixture(0xB3);
        let mut coordinator = claimed_durable_validate_coordinator(&fixture);
        let mut holder = take_dispatch_registry(&mut fixture);
        let dispatch = coordinator
            .begin_durable_validate_dispatch(&mut holder, fixture.lease.clone(), &fixture.verified)
            .expect("exact claimed Validate becomes one dispatch");
        let wait = dispatch.wait_token_for_test();
        let empty_directory = TempDir::new().expect("temporary empty Validate body store");
        let mut empty_store =
            V2BodyStore::open(empty_directory.path(), fixture.verified.context().clone())
                .expect("open empty Validate body store");

        let (error, dispatch) = dispatch
            .execute(&mut empty_store, |_| {
                Ok::<_, DetachedValidationError>(
                    ValidatedBodyReceipt::for_test(durable.clone()).execution_commitment(),
                )
            })
            .expect_err("missing durable catalog row returns the dispatch");
        assert!(matches!(error, V2BodyStoreError::ReceiptMismatch));
        assert_eq!(dispatch.wait_token_for_test(), wait);

        let commitment = ValidatedBodyReceipt::for_test(durable).execution_commitment();
        let executed = dispatch
            .execute(&mut store, |_| Ok::<_, DetachedValidationError>(commitment))
            .expect("returned dispatch remains executable against its exact store");
        assert_eq!(executed.wait_token_for_test(), wait);
        assert_eq!(
            executed
                .outcome()
                .validated_receipt()
                .map(ValidatedBodyReceipt::execution_commitment),
            Some(commitment)
        );
    }

    #[cfg(feature = "bls")]
    #[test]
    fn durable_validate_dispatch_rejects_stale_foreign_and_wrong_kind_without_mutation() {
        {
            let (mut fixture, _directory, _store, _durable) = durable_validate_store_fixture(0xB4);
            let mut coordinator = claimed_durable_validate_coordinator(&fixture);
            let mut holder = take_dispatch_registry(&mut fixture);
            let coordinator_before = format!("{coordinator:?}");
            let registry_before = format!("{:?}", holder.registry_for_test());
            let mut stale = fixture.lease.clone();
            stale.id = super::super::LeaseId(stale.id().0 + 1);

            let Err((error, returned)) = coordinator.begin_durable_validate_dispatch(
                &mut holder,
                stale.clone(),
                &fixture.verified,
            ) else {
                panic!("stale lease must not mint a Validate dispatch")
            };
            assert_eq!(error, DurableValidateDispatchError::StaleLease);
            assert_eq!(returned, stale);
            assert_eq!(format!("{coordinator:?}"), coordinator_before);
            assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
        }

        {
            let (fixture, _directory, _store, _durable) = durable_validate_store_fixture(0xB5);
            let mut coordinator = claimed_durable_validate_coordinator(&fixture);
            let mut holder = LifecycleWorkRegistryHolder::empty();
            let coordinator_before = format!("{coordinator:?}");
            let registry_before = format!("{:?}", holder.registry_for_test());
            let lease = fixture.lease.clone();

            let Err((error, returned)) = coordinator.begin_durable_validate_dispatch(
                &mut holder,
                lease.clone(),
                &fixture.verified,
            ) else {
                panic!("foreign empty registry must not mint a Validate dispatch")
            };
            assert_eq!(
                error,
                DurableValidateDispatchError::Registry(DurableValidateExecutionError::Registry(
                    RegistryError::Missing
                ))
            );
            assert_eq!(returned, lease);
            assert_eq!(format!("{coordinator:?}"), coordinator_before);
            assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
        }

        {
            let (mut fixture, _directory, _store, _durable) = durable_validate_store_fixture(0xB6);
            let mut coordinator = claimed_durable_validate_coordinator(&fixture);
            let incumbent = fixture
                .registry
                .entries
                .remove(&fixture.address)
                .expect("wrong-kind fixture removes its closed Validate");
            let ConcreteLifecycleWorkKind::DurableValidateBody(validate) = incumbent.kind else {
                unreachable!("wrong-kind fixture starts with one closed Validate")
            };
            let DurableValidateBody {
                effect, pending, ..
            } = validate;
            let pending = ConcreteLifecycleWork::from_exact(effect, pending)
                .expect("rebuild exact pending Validate work");
            assert!(
                fixture
                    .registry
                    .entries
                    .insert(fixture.address, pending)
                    .is_none()
            );
            let mut holder = take_dispatch_registry(&mut fixture);
            let coordinator_before = format!("{coordinator:?}");
            let registry_before = format!("{:?}", holder.registry_for_test());
            let lease = fixture.lease.clone();

            let Err((error, returned)) = coordinator.begin_durable_validate_dispatch(
                &mut holder,
                lease.clone(),
                &fixture.verified,
            ) else {
                panic!("pending Validate row must not cross the closed-carrier dispatch")
            };
            assert_eq!(
                error,
                DurableValidateDispatchError::Registry(
                    DurableValidateExecutionError::WrongWorkKind
                )
            );
            assert_eq!(returned, lease);
            assert_eq!(format!("{coordinator:?}"), coordinator_before);
            assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
        }
    }

    #[cfg(feature = "bls")]
    #[test]
    fn durable_validate_dispatch_rejects_a_substituted_ledger_body_frame() {
        let (mut fixture, _directory, _store, _durable) = durable_validate_store_fixture(0xBE);
        let mut coordinator = claimed_durable_validate_coordinator(&fixture);
        let metadata = coordinator
            .durable_records
            .get_mut(&fixture.lease.ordinal())
            .expect("claimed Validate retains durable metadata");
        let DurablePayloadReference::BodyFrame(mut substituted) = metadata.payload else {
            panic!("claimed Validate must retain one durable body frame")
        };
        substituted.frame = LifecycleDigest::new([0xEE; 32]);
        metadata.payload = DurablePayloadReference::BodyFrame(substituted);
        let mut holder = take_dispatch_registry(&mut fixture);
        let coordinator_before = format!("{coordinator:?}");
        let registry_before = format!("{:?}", holder.registry_for_test());
        let lease = fixture.lease.clone();

        let Err((error, returned)) = coordinator.begin_durable_validate_dispatch(
            &mut holder,
            lease.clone(),
            &fixture.verified,
        ) else {
            panic!("a ledger frame foreign to the installed carrier must fail closed")
        };
        assert_eq!(
            error,
            DurableValidateDispatchError::Registry(
                DurableValidateExecutionError::InvalidValidateShape
            )
        );
        assert_eq!(returned, lease);
        assert_eq!(format!("{coordinator:?}"), coordinator_before);
        assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
    }

    #[cfg(feature = "bls")]
    #[test]
    fn durable_validate_dispatch_rejects_max_generation_and_wait_source_alias() {
        {
            let (mut fixture, _directory, _store, _durable) = durable_validate_store_fixture(0xB7);
            let source = durable_validation_source(&mut fixture);
            let mut coordinator = claimed_durable_validate_coordinator(&fixture);
            coordinator.observed_generation.insert(source, u64::MAX);
            let mut holder = take_dispatch_registry(&mut fixture);
            let coordinator_before = format!("{coordinator:?}");
            let registry_before = format!("{:?}", holder.registry_for_test());
            let lease = fixture.lease.clone();

            let Err((error, returned)) = coordinator.begin_durable_validate_dispatch(
                &mut holder,
                lease.clone(),
                &fixture.verified,
            ) else {
                panic!("maximum wait generation must not mint a Validate dispatch")
            };
            assert_eq!(error, DurableValidateDispatchError::WaitGenerationExhausted);
            assert_eq!(returned, lease);
            assert_eq!(format!("{coordinator:?}"), coordinator_before);
            assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
        }

        {
            let (mut fixture, _directory, _store, _durable) = durable_validate_store_fixture(0xB8);
            let source = durable_validation_source(&mut fixture);
            let mut coordinator = claimed_durable_validate_coordinator(&fixture);
            let alias_ordinal = fixture.lease.ordinal() + 1000;
            let mut alias = coordinator.records[&fixture.lease.ordinal()].clone();
            alias.ordinal = alias_ordinal;
            alias.state = LifecycleState::Waiting(WaitToken::new(source, 0));
            assert!(coordinator.records.insert(alias_ordinal, alias).is_none());
            let mut holder = take_dispatch_registry(&mut fixture);
            let coordinator_before = format!("{coordinator:?}");
            let registry_before = format!("{:?}", holder.registry_for_test());
            let lease = fixture.lease.clone();

            let Err((error, returned)) = coordinator.begin_durable_validate_dispatch(
                &mut holder,
                lease.clone(),
                &fixture.verified,
            ) else {
                panic!("aliased external wait source must not mint a Validate dispatch")
            };
            assert_eq!(error, DurableValidateDispatchError::AliasedWaitSource);
            assert_eq!(returned, lease);
            assert_eq!(format!("{coordinator:?}"), coordinator_before);
            assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
        }
    }

    #[cfg(feature = "bls")]
    #[test]
    fn durable_validate_dispatch_rejects_reverse_identity_aliases() {
        {
            let (mut fixture, _directory, _store, _durable) = durable_validate_store_fixture(0xB9);
            let mut coordinator = claimed_durable_validate_coordinator(&fixture);
            let alias_key = fixture.lease.ordinal() + 1000;
            let alias = coordinator.records[&fixture.lease.ordinal()].clone();
            assert!(coordinator.records.insert(alias_key, alias).is_none());
            let mut holder = take_dispatch_registry(&mut fixture);
            let coordinator_before = format!("{coordinator:?}");
            let registry_before = format!("{:?}", holder.registry_for_test());
            let lease = fixture.lease.clone();

            let Err((error, returned)) = coordinator.begin_durable_validate_dispatch(
                &mut holder,
                lease.clone(),
                &fixture.verified,
            ) else {
                panic!("reverse internal-ordinal alias must fail before detachment")
            };
            assert_eq!(error, DurableValidateDispatchError::StaleLease);
            assert_eq!(returned, lease);
            assert_eq!(format!("{coordinator:?}"), coordinator_before);
            assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
        }

        {
            let (mut fixture, _directory, _store, _durable) = durable_validate_store_fixture(0xBA);
            let mut coordinator = claimed_durable_validate_coordinator(&fixture);
            let key = fixture.lease.key();
            let alias_key = super::super::LifecycleKey::new(
                key.context(),
                key.round(),
                key.proposal_round(),
                key.subject(),
                super::super::LifecyclePhase::Apply,
                key.execution_commitment(),
            );
            assert_ne!(alias_key, key);
            assert!(
                coordinator
                    .key_index
                    .insert(alias_key, fixture.lease.ordinal())
                    .is_none()
            );
            let mut holder = take_dispatch_registry(&mut fixture);
            let coordinator_before = format!("{coordinator:?}");
            let registry_before = format!("{:?}", holder.registry_for_test());
            let lease = fixture.lease.clone();

            let Err((error, returned)) = coordinator.begin_durable_validate_dispatch(
                &mut holder,
                lease.clone(),
                &fixture.verified,
            ) else {
                panic!("reverse key-index alias must fail before detachment")
            };
            assert_eq!(error, DurableValidateDispatchError::StaleLease);
            assert_eq!(returned, lease);
            assert_eq!(format!("{coordinator:?}"), coordinator_before);
            assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
        }

        {
            let (mut fixture, _directory, _store, _durable) = durable_validate_store_fixture(0xBB);
            let mut coordinator = claimed_durable_validate_coordinator(&fixture);
            let alias_root = super::super::CausalRoot::new(LifecycleDigest::new([0xBB; 32]));
            assert_ne!(alias_root, fixture.lease.owner().causal_root());
            assert!(
                coordinator
                    .owner_index
                    .insert(alias_root, fixture.lease.owner())
                    .is_none()
            );
            let mut holder = take_dispatch_registry(&mut fixture);
            let coordinator_before = format!("{coordinator:?}");
            let registry_before = format!("{:?}", holder.registry_for_test());
            let lease = fixture.lease.clone();

            let Err((error, returned)) = coordinator.begin_durable_validate_dispatch(
                &mut holder,
                lease.clone(),
                &fixture.verified,
            ) else {
                panic!("reverse owner-index alias must fail before detachment")
            };
            assert_eq!(error, DurableValidateDispatchError::StaleLease);
            assert_eq!(returned, lease);
            assert_eq!(format!("{coordinator:?}"), coordinator_before);
            assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
        }

        {
            let (mut fixture, _directory, _store, _durable) = durable_validate_store_fixture(0xBC);
            let mut coordinator = claimed_durable_validate_coordinator(&fixture);
            let alias_ordinal = fixture.lease.ordinal() + 1000;
            let mut alias = coordinator.records[&fixture.lease.ordinal()].clone();
            alias.ordinal = alias_ordinal;
            alias.state = LifecycleState::Ready;
            assert!(coordinator.records.insert(alias_ordinal, alias).is_none());
            let mut holder = take_dispatch_registry(&mut fixture);
            let coordinator_before = format!("{coordinator:?}");
            let registry_before = format!("{:?}", holder.registry_for_test());
            let lease = fixture.lease.clone();

            let Err((error, returned)) = coordinator.begin_durable_validate_dispatch(
                &mut holder,
                lease.clone(),
                &fixture.verified,
            ) else {
                panic!("duplicate lifecycle record key must fail before detachment")
            };
            assert_eq!(error, DurableValidateDispatchError::StaleLease);
            assert_eq!(returned, lease);
            assert_eq!(format!("{coordinator:?}"), coordinator_before);
            assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
        }
    }

    #[cfg(feature = "bls")]
    #[test]
    fn ready_validate_capacity_classifier_is_exact_and_drop_inert() {
        let (fixture, _directory, _store, _durable) = durable_validate_store_fixture(0xBF);
        let before = format!("{:?}", fixture.registry);
        let digest = fixture.lease.physical_slots()[&fixture.slot];

        let seal = fixture
            .registry
            .classify_ready_validate_carrier(fixture.address, digest)
            .expect("exact durable Validate carrier mints one opaque seal");
        assert!(seal.matches(
            fixture.address.owner,
            fixture.address.ordinal,
            fixture.address.slot,
            digest,
        ));
        assert!(!seal.requires_consensus_capacity());
        assert_eq!(
            fixture.registry.classify_ready_validate_carrier(
                fixture.address,
                LifecycleDigest::new([0xFF; 32]),
            ),
            Err(ReadyValidateCarrierError::Registry(
                RegistryError::DigestMismatch
            ))
        );
        assert_eq!(format!("{:?}", fixture.registry), before);
    }

    #[cfg(feature = "bls")]
    #[test]
    fn validated_completion_atomically_publishes_exact_ready_carrier() {
        let WaitingDurableValidateFixture {
            fixture,
            _directory,
            mut store,
            durable,
            mut coordinator,
            mut holder,
            dispatch,
        } = waiting_durable_validate_fixture(0xC0);
        let ordinal = fixture.lease.ordinal();
        let old_digest = fixture.lease.physical_slots()[&fixture.slot];
        let wait = dispatch.wait_token_for_test();
        let before_record = coordinator.records[&ordinal].clone();
        let before_records = coordinator.records.len();
        let before_high_water = coordinator.high_water;
        let before_capacity = coordinator.capacity_used.clone();
        let before_capacity_generation = coordinator.capacity_generation.clone();
        let before_durable = coordinator.durable_records.clone();
        let before_debts = coordinator.producer_debts.clone();
        let commitment = ValidatedBodyReceipt::for_test(durable.clone()).execution_commitment();
        let executed = dispatch
            .execute(&mut store, |_| Ok::<_, DetachedValidationError>(commitment))
            .expect("execute exact successful Validate dispatch");

        let publication = coordinator
            .complete_durable_validate_dispatch(&mut holder, executed)
            .expect("publish exact successful Validate completion");
        let DurableValidateCompletionPublication::PublishedValidated(published) = publication
        else {
            panic!("successful body validation publishes the validated carrier")
        };
        let location = published.location_for_test();
        assert_eq!(location.address, fixture.address);
        assert_eq!(location.incumbent_digest, old_digest);
        assert_ne!(location.replacement_digest, old_digest);

        let record = &coordinator.records[&ordinal];
        assert_eq!(record.owner, fixture.lease.owner());
        assert_eq!(record.ordinal, ordinal);
        assert_eq!(record.state, LifecycleState::Ready);
        assert_eq!(record.physical_slots.len(), 1);
        assert_eq!(
            record.physical_slots.get(&fixture.slot),
            Some(&location.replacement_digest)
        );
        assert_eq!(record.episode, before_record.episode);
        assert_eq!(coordinator.records.len(), before_records);
        assert_eq!(coordinator.high_water, before_high_water);
        assert_eq!(coordinator.capacity_used, before_capacity);
        assert_eq!(coordinator.capacity_generation, before_capacity_generation);
        assert_eq!(coordinator.durable_records, before_durable);
        assert_eq!(coordinator.producer_debts, before_debts);
        assert_eq!(coordinator.observed_generation[&wait.source()], 1);
        assert!(coordinator.ready_index.contains(&ordinal));
        assert!(coordinator.active_lease.is_none());
        assert!(coordinator.ledger_store.is_none());
        assert_eq!(
            coordinator
                .attest_ready_validate_demand(&holder, ordinal)
                .expect("validated completion mints one exact scheduler attestation")
                .capacity_class(),
            None
        );

        assert_eq!(holder.registry_for_test().entries.len(), 1);
        let installed = &holder.registry_for_test().entries[&fixture.address];
        assert_eq!(installed.digest, location.replacement_digest);
        assert!(installed.validates_at(fixture.address));
        let ConcreteLifecycleWorkKind::DurableValidateCompletion(completion) = &installed.kind
        else {
            panic!("successful validation installs one closed completion carrier")
        };
        assert_eq!(completion.address, fixture.address);
        assert_eq!(completion.incumbent_digest, old_digest);
        assert!(completion.incumbent.validates(old_digest));
        assert_eq!(completion.outcome.durable_body(), &durable);
        assert_eq!(
            completion
                .outcome
                .validated_receipt()
                .map(ValidatedBodyReceipt::execution_commitment),
            Some(commitment)
        );
    }

    #[cfg(feature = "bls")]
    #[test]
    fn validated_completion_rejects_conflicting_inherited_commitment_intact() {
        let (mut fixture, _directory, mut store, durable) = durable_validate_store_fixture(0xCD);
        let yielded_commitment =
            ValidatedBodyReceipt::for_test(durable.clone()).execution_commitment();
        let inherited_commitment = wire::ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new(b"inherited commitment parent"),
            Hash::new(b"inherited commitment post"),
            Hash::new(b"inherited commitment writes"),
            1,
            Hash::new(b"inherited commitment wire"),
        );
        assert!(inherited_commitment.validate().is_ok());
        assert_ne!(inherited_commitment, yielded_commitment);
        seal_validate_fixture_commitment(&mut fixture, inherited_commitment);
        let mut coordinator = claimed_durable_validate_coordinator(&fixture);
        let mut holder = take_dispatch_registry(&mut fixture);
        let dispatch = coordinator
            .begin_durable_validate_dispatch(&mut holder, fixture.lease.clone(), &fixture.verified)
            .expect("commitment-authorized Validate becomes one waiting dispatch");
        let executed = dispatch
            .execute(&mut store, |_| {
                Ok::<_, DetachedValidationError>(yielded_commitment)
            })
            .expect("body store retains the conflicting deterministic success");
        let coordinator_before = format!("{coordinator:?}");
        let registry_before = format!("{:?}", holder.registry_for_test());
        let dispatch_before = format!("{executed:?}");

        let Err((error, returned)) =
            coordinator.complete_durable_validate_dispatch(&mut holder, executed)
        else {
            panic!("inherited commitment must constrain asynchronous validation success")
        };
        assert_eq!(
            error,
            DurableValidateCompletionPublicationError::Registry(
                DurableValidateCompletionConversionError::Execution(
                    DurableValidateExecutionError::ConflictingValidationCommitment
                )
            )
        );
        assert_eq!(format!("{returned:?}"), dispatch_before);
        assert_eq!(format!("{coordinator:?}"), coordinator_before);
        assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
        assert_eq!(
            returned
                .outcome()
                .validated_receipt()
                .map(ValidatedBodyReceipt::execution_commitment),
            Some(yielded_commitment)
        );
        assert_eq!(
            returned
                .executed
                .request
                .candidate_statement
                .and_then(RuntimeCandidateSemanticStatement::execution_commitment),
            Some(inherited_commitment)
        );
    }

    #[cfg(feature = "bls")]
    #[test]
    fn rejected_completion_atomically_publishes_exact_ready_carrier() {
        let WaitingDurableValidateFixture {
            fixture,
            _directory,
            mut store,
            durable,
            mut coordinator,
            mut holder,
            dispatch,
        } = waiting_durable_validate_fixture(0xC1);
        let ordinal = fixture.lease.ordinal();
        let old_digest = fixture.lease.physical_slots()[&fixture.slot];
        let wait = dispatch.wait_token_for_test();
        let before_record = coordinator.records[&ordinal].clone();
        let before_records = coordinator.records.len();
        let before_high_water = coordinator.high_water;
        let before_capacity = coordinator.capacity_used.clone();
        let before_capacity_generation = coordinator.capacity_generation.clone();
        let before_durable = coordinator.durable_records.clone();
        let before_debts = coordinator.producer_debts.clone();
        let executed = dispatch
            .execute(&mut store, |_| {
                Err::<wire::ExecutionCommitment, _>(DetachedValidationError::Invalid(
                    "deterministic rejected completion",
                ))
            })
            .expect("execute exact rejected Validate dispatch");

        let publication = coordinator
            .complete_durable_validate_dispatch(&mut holder, executed)
            .expect("publish exact rejected Validate completion");
        let DurableValidateCompletionPublication::PublishedRejected(published) = publication else {
            panic!("deterministic rejection publishes the rejected carrier")
        };
        let location = published.location_for_test();
        assert_eq!(location.address, fixture.address);
        assert_eq!(location.incumbent_digest, old_digest);
        assert_ne!(location.replacement_digest, old_digest);

        let record = &coordinator.records[&ordinal];
        assert_eq!(record.owner, fixture.lease.owner());
        assert_eq!(record.ordinal, ordinal);
        assert_eq!(record.state, LifecycleState::Ready);
        assert_eq!(record.physical_slots.len(), 1);
        assert_eq!(
            record.physical_slots.get(&fixture.slot),
            Some(&location.replacement_digest)
        );
        assert_eq!(record.episode, before_record.episode);
        assert_eq!(coordinator.records.len(), before_records);
        assert_eq!(coordinator.high_water, before_high_water);
        assert_eq!(coordinator.capacity_used, before_capacity);
        assert_eq!(coordinator.capacity_generation, before_capacity_generation);
        assert_eq!(coordinator.durable_records, before_durable);
        assert_eq!(coordinator.producer_debts, before_debts);
        assert_eq!(coordinator.observed_generation[&wait.source()], 1);
        assert!(coordinator.ready_index.contains(&ordinal));
        assert!(coordinator.ledger_store.is_none());
        let attestation = coordinator
            .attest_ready_validate_demand(&holder, ordinal)
            .expect("rejected completion mints one exact scheduler attestation");
        assert_eq!(attestation.capacity_class(), Some(CapacityClass::Consensus));
        let ready = super::super::SchedulerReadyInputs::from_authenticated(
            &coordinator.records[&ordinal],
            Some(attestation),
            [0; 6],
        )
        .expect("registry attestation binds one exact scheduler row");

        let mut stale = coordinator.clone();
        stale
            .records
            .get_mut(&ordinal)
            .expect("rejected completion row")
            .physical_slots
            .insert(fixture.slot, LifecycleDigest::new([0xEF; 32]));
        let stale_before = format!("{stale:?}");
        assert_eq!(
            stale.attest_ready_validate_demand(&holder, ordinal),
            Err(ReadyValidateDemandAttestationError::Registry(
                ReadyValidateCarrierError::Registry(RegistryError::DigestMismatch)
            ))
        );
        assert_eq!(format!("{stale:?}"), stale_before);

        let mut substituted = coordinator.clone();
        let metadata = substituted
            .durable_records
            .get_mut(&ordinal)
            .expect("rejected completion retains durable metadata");
        let DurablePayloadReference::BodyFrame(mut foreign_frame) = metadata.payload else {
            panic!("rejected completion must retain one durable body frame")
        };
        foreign_frame.manifest = LifecycleDigest::new([0xED; 32]);
        metadata.payload = DurablePayloadReference::BodyFrame(foreign_frame);
        let substituted_before = format!("{substituted:?}");
        assert_eq!(
            substituted.attest_ready_validate_demand(&holder, ordinal),
            Err(ReadyValidateDemandAttestationError::InvalidCoordinatorIndex)
        );
        assert_eq!(format!("{substituted:?}"), substituted_before);

        let inputs = super::super::SchedulerInputs::new([], [(ordinal, ready)])
            .expect("one unique registry-attested Ready row");
        let super::super::TurnPlan::Execute(lease) = coordinator.plan_turn(inputs) else {
            panic!("registry-attested rejected Validate must claim with its reservation")
        };
        assert_eq!(lease.ordinal(), ordinal);
        assert_eq!(
            lease
                .output_reservation()
                .map(|reservation| reservation.class()),
            Some(CapacityClass::Consensus)
        );

        assert_eq!(holder.registry_for_test().entries.len(), 1);
        let installed = &holder.registry_for_test().entries[&fixture.address];
        assert_eq!(installed.digest, location.replacement_digest);
        assert!(installed.validates_at(fixture.address));
        let ConcreteLifecycleWorkKind::DurableValidateCompletion(completion) = &installed.kind
        else {
            panic!("rejection installs one closed completion carrier")
        };
        assert_eq!(completion.incumbent_digest, old_digest);
        assert!(completion.incumbent.validates(old_digest));
        assert_eq!(completion.outcome.durable_body(), &durable);
        assert_eq!(
            completion.outcome.rejection_reason(),
            Some("deterministic rejected completion")
        );
        assert!(completion.outcome.validated_receipt().is_none());
    }

    #[cfg(feature = "bls")]
    #[test]
    fn ready_validate_execution_preflight_binds_closed_outcomes_and_is_drop_inert() {
        {
            let ReadyDurableValidateFixture {
                fixture,
                _directory,
                mut holder,
                lease,
                durable,
            } = ready_durable_validate_fixture(0xD0, ReadyDurableValidateFixtureOutcome::Validated);
            let before = format!("{:?}", holder.registry_for_test());
            let prepared = holder
                .registry_for_test_mut()
                .prepare_ready_durable_validate_execution(&lease, fixture.slot, &fixture.verified)
                .expect("prepare exact validated Ready carrier");
            assert_eq!(
                prepared.outcome_kind(),
                ReadyDurableValidateOutcomeKind::Validated
            );
            assert!(prepared.matches_exact_lease(&lease));
            assert!(prepared.matches_exact_durable_receipt(&durable));
            let foreign_receipt = DurableBodyReceipt::for_test(
                durable.context_id(),
                durable.round(),
                durable.subject(),
                HashOf::from_untyped_unchecked(Hash::new(b"foreign Ready Validate manifest")),
            );
            assert!(!prepared.matches_exact_durable_receipt(&foreign_receipt));
            let mut foreign_lease = lease.clone();
            foreign_lease.id = LeaseId(
                foreign_lease
                    .id()
                    .0
                    .checked_add(1)
                    .expect("fixture lease id remains bounded"),
            );
            assert!(!prepared.matches_exact_lease(&foreign_lease));
            assert!(prepared.validated_authority().is_some());
            assert!(prepared.rejected_authority().is_none());
            drop(prepared);
            assert_eq!(format!("{:?}", holder.registry_for_test()), before);
        }

        {
            let ReadyDurableValidateFixture {
                fixture,
                _directory,
                mut holder,
                lease,
                durable,
            } = ready_durable_validate_fixture(0xD1, ReadyDurableValidateFixtureOutcome::Rejected);
            let before = format!("{:?}", holder.registry_for_test());
            let prepared = holder
                .registry_for_test_mut()
                .prepare_ready_durable_validate_execution(&lease, fixture.slot, &fixture.verified)
                .expect("prepare exact rejected Ready carrier");
            assert_eq!(
                prepared.outcome_kind(),
                ReadyDurableValidateOutcomeKind::Rejected
            );
            assert!(prepared.matches_exact_durable_receipt(&durable));
            assert!(prepared.rejected_authority().is_some());
            assert!(prepared.validated_authority().is_none());
            drop(prepared);
            assert_eq!(format!("{:?}", holder.registry_for_test()), before);
        }

        {
            let ReadyDurableValidateFixture {
                fixture,
                _directory,
                mut holder,
                mut lease,
                durable: _,
            } = ready_durable_validate_fixture(0xDA, ReadyDurableValidateFixtureOutcome::Rejected);
            lease.output_reservation = None;
            let before = format!("{:?}", holder.registry_for_test());
            assert!(matches!(
                holder
                    .registry_for_test_mut()
                    .prepare_ready_durable_validate_execution(
                        &lease,
                        fixture.slot,
                        &fixture.verified,
                    ),
                Err(ReadyDurableValidateExecutionError::InvalidLeaseShape)
            ));
            assert_eq!(format!("{:?}", holder.registry_for_test()), before);
        }

        {
            let ReadyDurableValidateFixture {
                fixture,
                _directory,
                mut holder,
                mut lease,
                durable: _,
            } = ready_durable_validate_fixture(0xDB, ReadyDurableValidateFixtureOutcome::Validated);
            lease.output_reservation = Some(super::super::schema::LeaseCapacityReservation::new(
                CapacityClass::Consensus,
                0,
            ));
            let before = format!("{:?}", holder.registry_for_test());
            assert!(matches!(
                holder
                    .registry_for_test_mut()
                    .prepare_ready_durable_validate_execution(
                        &lease,
                        fixture.slot,
                        &fixture.verified,
                    ),
                Err(ReadyDurableValidateExecutionError::InvalidLeaseShape)
            ));
            assert_eq!(format!("{:?}", holder.registry_for_test()), before);
        }
    }

    #[cfg(feature = "bls")]
    #[test]
    fn recovered_wal_validate_cut_detaches_only_validated_completion_and_restores_on_drop() {
        {
            let ReadyDurableValidateFixture {
                fixture,
                _directory,
                mut holder,
                lease,
                durable: _,
            } = ready_durable_validate_fixture(0xDC, ReadyDurableValidateFixtureOutcome::Validated);
            let before = format!("{:?}", holder.registry_for_test());
            let prepared = holder
                .registry_for_test_mut()
                .prepare_ready_durable_validate_execution(&lease, fixture.slot, &fixture.verified)
                .expect("prepare exact validated recovered-WAL parent");
            let cut = match prepared.into_recovered_wal_validate_registry_cut() {
                Ok(cut) => cut,
                Err(_prepared) => panic!("validated completion must detach into WAL parent cut"),
            };
            assert!(cut.detached_work_is_exact_for_test());
            drop(cut);
            assert_eq!(format!("{:?}", holder.registry_for_test()), before);
        }

        {
            let ReadyDurableValidateFixture {
                fixture,
                _directory,
                mut holder,
                lease,
                durable: _,
            } = ready_durable_validate_fixture(0xDD, ReadyDurableValidateFixtureOutcome::Rejected);
            let before = format!("{:?}", holder.registry_for_test());
            let prepared = holder
                .registry_for_test_mut()
                .prepare_ready_durable_validate_execution(&lease, fixture.slot, &fixture.verified)
                .expect("prepare exact rejected recovered-WAL parent candidate");
            let prepared = match prepared.into_recovered_wal_validate_registry_cut() {
                Ok(_cut) => panic!("rejected completion cannot become a WAL vote parent"),
                Err(prepared) => prepared,
            };
            drop(prepared);
            assert_eq!(format!("{:?}", holder.registry_for_test()), before);
        }
    }

    #[cfg(feature = "bls")]
    #[test]
    #[allow(clippy::too_many_lines)]
    fn ready_validate_execution_preflight_rejects_foreign_or_malformed_authority() {
        {
            let ReadyDurableValidateFixture {
                fixture,
                _directory,
                mut holder,
                mut lease,
                durable: _,
            } = ready_durable_validate_fixture(0xD2, ReadyDurableValidateFixtureOutcome::Validated);
            lease.owner = OwnerId::new(
                super::super::CausalRoot::new(LifecycleDigest::new([0xD2; 32])),
                lease.owner.first_admission_ordinal(),
            );
            assert!(matches!(
                holder
                    .registry_for_test_mut()
                    .prepare_ready_durable_validate_execution(
                        &lease,
                        fixture.slot,
                        &fixture.verified,
                    ),
                Err(ReadyDurableValidateExecutionError::Registry(
                    RegistryError::Missing
                ))
            ));
        }

        {
            let ReadyDurableValidateFixture {
                fixture,
                _directory,
                mut holder,
                mut lease,
                durable: _,
            } = ready_durable_validate_fixture(0xD3, ReadyDurableValidateFixtureOutcome::Validated);
            lease
                .physical_slots
                .insert(fixture.slot, LifecycleDigest::new([0xD3; 32]));
            assert!(matches!(
                holder
                    .registry_for_test_mut()
                    .prepare_ready_durable_validate_execution(
                        &lease,
                        fixture.slot,
                        &fixture.verified,
                    ),
                Err(ReadyDurableValidateExecutionError::Registry(
                    RegistryError::DigestMismatch
                ))
            ));
        }

        {
            let ReadyDurableValidateFixture {
                fixture,
                _directory,
                mut holder,
                mut lease,
                durable: _,
            } = ready_durable_validate_fixture(0xD4, ReadyDurableValidateFixtureOutcome::Rejected);
            lease.stage = super::super::LifecycleStage::new(
                super::super::LifecycleStageKind::StoreBody,
                super::super::PredecessorScope::Independent,
            );
            assert!(matches!(
                holder
                    .registry_for_test_mut()
                    .prepare_ready_durable_validate_execution(
                        &lease,
                        fixture.slot,
                        &fixture.verified,
                    ),
                Err(ReadyDurableValidateExecutionError::InvalidLeaseShape)
            ));
        }

        {
            let (mut fixture, _directory, _store, _durable) = durable_validate_store_fixture(0xD5);
            assert!(matches!(
                fixture.registry.prepare_ready_durable_validate_execution(
                    &fixture.lease,
                    fixture.slot,
                    &fixture.verified,
                ),
                Err(ReadyDurableValidateExecutionError::WrongWorkKind)
            ));
        }

        {
            let mut exact =
                ready_durable_validate_fixture(0xD6, ReadyDurableValidateFixtureOutcome::Validated);
            let WaitingDurableValidateFixture {
                fixture: deferred_fixture,
                _directory: deferred_directory,
                mut store,
                durable,
                coordinator: _,
                holder: _,
                dispatch,
            } = waiting_durable_validate_fixture(0xD7);
            let reference = detached_validation_merge_reference(&durable);
            let deferred = dispatch
                .execute(&mut store, |_| {
                    Err::<wire::ExecutionCommitment, _>(
                        DetachedValidationError::MissingMergeSidecar(reference),
                    )
                })
                .expect("execute foreign deferred outcome");
            let ExecutedDurableValidateDispatch {
                executed: ExecutedDurableValidateExecution { outcome, .. },
                ..
            } = deferred;
            let work = exact
                .holder
                .registry_for_test_mut()
                .entries
                .get_mut(&exact.fixture.address)
                .expect("exact fixture retains Ready carrier");
            let ConcreteLifecycleWorkKind::DurableValidateCompletion(completion) = &mut work.kind
            else {
                unreachable!("exact fixture retains Ready completion")
            };
            completion.outcome = outcome;
            let _keep_foreign_files = deferred_directory;
            assert_ne!(deferred_fixture.address, exact.fixture.address);
            assert!(matches!(
                exact
                    .holder
                    .registry_for_test_mut()
                    .prepare_ready_durable_validate_execution(
                        &exact.lease,
                        exact.fixture.slot,
                        &exact.fixture.verified,
                    ),
                Err(ReadyDurableValidateExecutionError::Registry(
                    RegistryError::CorruptWork
                ))
            ));
        }

        {
            let mut first =
                ready_durable_validate_fixture(0xD8, ReadyDurableValidateFixtureOutcome::Validated);
            let mut foreign =
                ready_durable_validate_fixture(0xD9, ReadyDurableValidateFixtureOutcome::Rejected);
            let first_work = first
                .holder
                .registry_for_test_mut()
                .entries
                .get_mut(&first.fixture.address)
                .expect("first fixture retains Ready carrier");
            let foreign_work = foreign
                .holder
                .registry_for_test_mut()
                .entries
                .get_mut(&foreign.fixture.address)
                .expect("foreign fixture retains Ready carrier");
            let ConcreteLifecycleWorkKind::DurableValidateCompletion(first_completion) =
                &mut first_work.kind
            else {
                unreachable!("first fixture retains Ready completion")
            };
            let ConcreteLifecycleWorkKind::DurableValidateCompletion(foreign_completion) =
                &mut foreign_work.kind
            else {
                unreachable!("foreign fixture retains Ready completion")
            };
            core::mem::swap(
                &mut first_completion.outcome,
                &mut foreign_completion.outcome,
            );
            assert!(matches!(
                first
                    .holder
                    .registry_for_test_mut()
                    .prepare_ready_durable_validate_execution(
                        &first.lease,
                        first.fixture.slot,
                        &first.fixture.verified,
                    ),
                Err(ReadyDurableValidateExecutionError::Registry(
                    RegistryError::CorruptWork
                ))
            ));
        }

        {
            let mut exact =
                ready_durable_validate_fixture(0xDE, ReadyDurableValidateFixtureOutcome::Rejected);
            let foreign = durable_validate_fixture(0xDF);
            let before = format!("{:?}", exact.holder.registry_for_test());
            assert!(matches!(
                exact
                    .holder
                    .registry_for_test_mut()
                    .prepare_ready_durable_validate_execution(
                        &exact.lease,
                        exact.fixture.slot,
                        &foreign.verified,
                    ),
                Err(ReadyDurableValidateExecutionError::Projection(_))
            ));
            assert_eq!(format!("{:?}", exact.holder.registry_for_test()), before);
        }
    }

    #[cfg(feature = "bls")]
    #[test]
    fn rejected_completion_digest_ignores_diagnostic_display_text() {
        let first = waiting_durable_validate_fixture(0xCE);
        let second = waiting_durable_validate_fixture(0xCE);
        let WaitingDurableValidateFixture {
            fixture: first_fixture,
            _directory: first_directory,
            store: mut first_store,
            durable: first_durable,
            coordinator: _first_coordinator,
            holder: _first_holder,
            dispatch: first_dispatch,
        } = first;
        let WaitingDurableValidateFixture {
            fixture: second_fixture,
            _directory: second_directory,
            store: mut second_store,
            durable: second_durable,
            coordinator: _second_coordinator,
            holder: _second_holder,
            dispatch: second_dispatch,
        } = second;
        assert_eq!(first_fixture.address, second_fixture.address);
        assert_eq!(first_durable, second_durable);
        let first_executed = first_dispatch
            .execute(&mut first_store, |_| {
                Err::<wire::ExecutionCommitment, _>(DetachedValidationError::Invalid(
                    "diagnostic wording alpha",
                ))
            })
            .expect("execute first deterministic rejection");
        let second_executed = second_dispatch
            .execute(&mut second_store, |_| {
                Err::<wire::ExecutionCommitment, _>(DetachedValidationError::Invalid(
                    "diagnostic wording beta",
                ))
            })
            .expect("execute second deterministic rejection");
        assert_ne!(
            first_executed.outcome().rejection_reason(),
            second_executed.outcome().rejection_reason()
        );
        assert_eq!(
            first_executed.outcome().rejection_identity(),
            Some(&BodyValidationRejectionIdentity::Rejected)
        );
        assert_eq!(
            first_executed.outcome().rejection_identity(),
            second_executed.outcome().rejection_identity()
        );
        let incumbent_digest = first_fixture.lease.physical_slots()[&first_fixture.slot];
        let first_digest = durable_validate_completion_digest(
            incumbent_digest,
            first_fixture.expected_manifest_hash,
            first_executed.outcome(),
        )
        .expect("first rejection derives one replacement digest");
        let second_digest = durable_validate_completion_digest(
            incumbent_digest,
            second_fixture.expected_manifest_hash,
            second_executed.outcome(),
        )
        .expect("second rejection derives one replacement digest");
        assert_ne!(first_digest, incumbent_digest);
        assert_eq!(first_digest, second_digest);
        drop(first_directory);
        drop(second_directory);
    }

    #[cfg(feature = "bls")]
    #[test]
    fn merge_sidecar_deferral_retains_dispatch_and_leaves_waiting_row_original() {
        let WaitingDurableValidateFixture {
            fixture,
            _directory,
            mut store,
            durable,
            mut coordinator,
            mut holder,
            dispatch,
        } = waiting_durable_validate_fixture(0xC2);
        let reference = detached_validation_merge_reference(&durable);
        let wait = dispatch.wait_token_for_test();
        let coordinator_before = format!("{coordinator:?}");
        let registry_before = format!("{:?}", holder.registry_for_test());
        let old_digest = fixture.lease.physical_slots()[&fixture.slot];
        let executed = dispatch
            .execute(&mut store, |_| {
                Err::<wire::ExecutionCommitment, _>(DetachedValidationError::MissingMergeSidecar(
                    reference.clone(),
                ))
            })
            .expect("execute exact deferred Validate dispatch");

        let publication = coordinator
            .complete_durable_validate_dispatch(&mut holder, executed)
            .expect("retain exact merge-sidecar deferral");
        let DurableValidateCompletionPublication::DeferredMergeSidecar(deferred) = publication
        else {
            panic!("missing merge sidecar must not publish an executable carrier")
        };
        assert_eq!(deferred.missing_reference(), &reference);
        assert_eq!(deferred.dispatch_for_test().wait_token_for_test(), wait);
        assert_eq!(
            deferred.dispatch_for_test().outcome().durable_body(),
            &durable
        );
        assert_eq!(format!("{coordinator:?}"), coordinator_before);
        assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
        assert_eq!(
            coordinator.records[&fixture.lease.ordinal()].state,
            LifecycleState::Waiting(wait)
        );
        assert_eq!(
            coordinator.records[&fixture.lease.ordinal()].physical_slots[&fixture.slot],
            old_digest
        );
        assert!(!coordinator.ready_index.contains(&fixture.lease.ordinal()));
        assert!(matches!(
            holder.registry_for_test().entries[&fixture.address].kind,
            ConcreteLifecycleWorkKind::DurableValidateBody(_)
        ));
    }

    #[cfg(feature = "bls")]
    #[test]
    #[allow(clippy::too_many_lines)]
    fn validate_completion_precommit_failures_preserve_both_sides_and_dispatch() {
        {
            let WaitingDurableValidateFixture {
                fixture,
                _directory,
                mut store,
                durable,
                mut coordinator,
                mut holder,
                dispatch,
            } = waiting_durable_validate_fixture(0xC3);
            let commitment = ValidatedBodyReceipt::for_test(durable.clone()).execution_commitment();
            let mut executed = dispatch
                .execute(&mut store, |_| Ok::<_, DetachedValidationError>(commitment))
                .expect("execute stale-digest completion fixture");
            executed.executed.request.incumbent_digest = LifecycleDigest::new([0xC3; 32]);
            let coordinator_before = format!("{coordinator:?}");
            let registry_before = format!("{:?}", holder.registry_for_test());
            let dispatch_before = format!("{executed:?}");

            let Err((error, returned)) =
                coordinator.complete_durable_validate_dispatch(&mut holder, executed)
            else {
                panic!("stale incumbent digest must fail before publication")
            };
            assert_eq!(
                error,
                DurableValidateCompletionPublicationError::Registry(
                    DurableValidateCompletionConversionError::Execution(
                        DurableValidateExecutionError::Registry(RegistryError::DigestMismatch)
                    )
                )
            );
            assert_eq!(format!("{returned:?}"), dispatch_before);
            assert_eq!(format!("{coordinator:?}"), coordinator_before);
            assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
            assert_eq!(returned.outcome().durable_body(), &durable);
            assert_eq!(returned.executed.request.address, fixture.address);
        }

        {
            let WaitingDurableValidateFixture {
                fixture: _,
                _directory,
                mut store,
                durable,
                mut coordinator,
                mut holder,
                dispatch,
            } = waiting_durable_validate_fixture(0xC4);
            let commitment = ValidatedBodyReceipt::for_test(durable.clone()).execution_commitment();
            let mut executed = dispatch
                .execute(&mut store, |_| Ok::<_, DetachedValidationError>(commitment))
                .expect("execute stale-address completion fixture");
            executed.executed.request.address.slot = PhysicalSlotId::for_capacity(
                CapacityClass::Effect,
                executed.executed.request.address.slot.1.saturating_add(1),
            );
            let coordinator_before = format!("{coordinator:?}");
            let registry_before = format!("{:?}", holder.registry_for_test());
            let dispatch_before = format!("{executed:?}");

            let Err((_, returned)) =
                coordinator.complete_durable_validate_dispatch(&mut holder, executed)
            else {
                panic!("foreign Validate address must fail before publication")
            };
            assert_eq!(format!("{returned:?}"), dispatch_before);
            assert_eq!(format!("{coordinator:?}"), coordinator_before);
            assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
            assert_eq!(returned.outcome().durable_body(), &durable);
        }

        {
            let WaitingDurableValidateFixture {
                fixture,
                _directory,
                mut store,
                durable,
                mut coordinator,
                mut holder,
                dispatch,
            } = waiting_durable_validate_fixture(0xC5);
            let commitment = ValidatedBodyReceipt::for_test(durable.clone()).execution_commitment();
            let executed = dispatch
                .execute(&mut store, |_| Ok::<_, DetachedValidationError>(commitment))
                .expect("execute wrong-carrier completion fixture");
            let incumbent = holder
                .registry_for_test_mut()
                .entries
                .remove(&fixture.address)
                .expect("wrong-carrier fixture removes exact Validate incumbent");
            let ConcreteLifecycleWorkKind::DurableValidateBody(validate) = incumbent.kind else {
                unreachable!("wrong-carrier fixture starts with durable Validate")
            };
            let pending = ConcreteLifecycleWork::from_exact(validate.effect, validate.pending)
                .expect("rebuild pending Validate wrong carrier");
            assert!(
                holder
                    .registry_for_test_mut()
                    .entries
                    .insert(fixture.address, pending)
                    .is_none()
            );
            let coordinator_before = format!("{coordinator:?}");
            let registry_before = format!("{:?}", holder.registry_for_test());
            let dispatch_before = format!("{executed:?}");

            let Err((error, returned)) =
                coordinator.complete_durable_validate_dispatch(&mut holder, executed)
            else {
                panic!("wrong concrete carrier must fail before publication")
            };
            assert_eq!(
                error,
                DurableValidateCompletionPublicationError::Registry(
                    DurableValidateCompletionConversionError::Execution(
                        DurableValidateExecutionError::WrongWorkKind
                    )
                )
            );
            assert_eq!(format!("{returned:?}"), dispatch_before);
            assert_eq!(format!("{coordinator:?}"), coordinator_before);
            assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
        }

        {
            let WaitingDurableValidateFixture {
                fixture,
                _directory,
                mut store,
                durable,
                mut coordinator,
                mut holder,
                dispatch,
            } = waiting_durable_validate_fixture(0xC6);
            let commitment = ValidatedBodyReceipt::for_test(durable.clone()).execution_commitment();
            let executed = dispatch
                .execute(&mut store, |_| Ok::<_, DetachedValidationError>(commitment))
                .expect("execute key-mutation completion fixture");
            let old_key = fixture.lease.key();
            let foreign_subject = LifecycleDigest::new([0xC6; 32]);
            let foreign_key = super::super::LifecycleKey::new(
                old_key.context(),
                old_key.round(),
                old_key.proposal_round(),
                Some(foreign_subject),
                LifecyclePhase::Validate,
                old_key.execution_commitment(),
            );
            assert_ne!(foreign_key, old_key);
            assert_eq!(
                coordinator.key_index.remove(&old_key),
                Some(fixture.lease.ordinal())
            );
            coordinator
                .records
                .get_mut(&fixture.lease.ordinal())
                .expect("key-mutation fixture retains target record")
                .key = foreign_key;
            assert!(
                coordinator
                    .key_index
                    .insert(foreign_key, fixture.lease.ordinal())
                    .is_none()
            );
            let coordinator_before = format!("{coordinator:?}");
            let registry_before = format!("{:?}", holder.registry_for_test());
            let dispatch_before = format!("{executed:?}");

            let Err((error, returned)) =
                coordinator.complete_durable_validate_dispatch(&mut holder, executed)
            else {
                panic!("consistent key/index mutation must fail exact async authority")
            };
            assert_eq!(
                error,
                DurableValidateCompletionPublicationError::InvalidWaitingState
            );
            assert_eq!(format!("{returned:?}"), dispatch_before);
            assert_eq!(format!("{coordinator:?}"), coordinator_before);
            assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
        }

        {
            let WaitingDurableValidateFixture {
                fixture,
                _directory,
                mut store,
                durable,
                mut coordinator,
                mut holder,
                dispatch,
            } = waiting_durable_validate_fixture(0xC7);
            let commitment = ValidatedBodyReceipt::for_test(durable.clone()).execution_commitment();
            let executed = dispatch
                .execute(&mut store, |_| Ok::<_, DetachedValidationError>(commitment))
                .expect("execute corrupt-episode completion fixture");
            coordinator
                .records
                .get_mut(&fixture.lease.ordinal())
                .expect("episode corruption fixture retains target record")
                .episode
                .frozen_predecessors
                .insert(fixture.lease.ordinal() + 1000);
            let coordinator_before = format!("{coordinator:?}");
            let registry_before = format!("{:?}", holder.registry_for_test());
            let dispatch_before = format!("{executed:?}");

            let Err((error, returned)) =
                coordinator.complete_durable_validate_dispatch(&mut holder, executed)
            else {
                panic!("corrupt independent episode must fail before publication")
            };
            assert_eq!(
                error,
                DurableValidateCompletionPublicationError::InvalidWaitingState
            );
            assert_eq!(format!("{returned:?}"), dispatch_before);
            assert_eq!(format!("{coordinator:?}"), coordinator_before);
            assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
        }
    }

    #[cfg(feature = "bls")]
    #[test]
    fn validate_completion_rejects_reverse_index_and_duplicate_record_key_intact() {
        {
            let WaitingDurableValidateFixture {
                fixture,
                _directory,
                mut store,
                durable,
                mut coordinator,
                mut holder,
                dispatch,
            } = waiting_durable_validate_fixture(0xCA);
            let commitment = ValidatedBodyReceipt::for_test(durable.clone()).execution_commitment();
            let executed = dispatch
                .execute(&mut store, |_| Ok::<_, DetachedValidationError>(commitment))
                .expect("execute reverse-index completion fixture");
            let key = fixture.lease.key();
            let alias_key = super::super::LifecycleKey::new(
                key.context(),
                key.round(),
                key.proposal_round(),
                key.subject(),
                LifecyclePhase::Apply,
                key.execution_commitment(),
            );
            assert_ne!(alias_key, key);
            assert!(
                coordinator
                    .key_index
                    .insert(alias_key, fixture.lease.ordinal())
                    .is_none()
            );
            let coordinator_before = format!("{coordinator:?}");
            let registry_before = format!("{:?}", holder.registry_for_test());
            let dispatch_before = format!("{executed:?}");

            let Err((error, returned)) =
                coordinator.complete_durable_validate_dispatch(&mut holder, executed)
            else {
                panic!("reverse key-index alias must fail completion preflight")
            };
            assert_eq!(
                error,
                DurableValidateCompletionPublicationError::InvalidWaitingState
            );
            assert_eq!(format!("{returned:?}"), dispatch_before);
            assert_eq!(format!("{coordinator:?}"), coordinator_before);
            assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
        }

        {
            let WaitingDurableValidateFixture {
                fixture,
                _directory,
                mut store,
                durable,
                mut coordinator,
                mut holder,
                dispatch,
            } = waiting_durable_validate_fixture(0xCB);
            let commitment = ValidatedBodyReceipt::for_test(durable.clone()).execution_commitment();
            let executed = dispatch
                .execute(&mut store, |_| Ok::<_, DetachedValidationError>(commitment))
                .expect("execute duplicate-key completion fixture");
            let alias_ordinal = fixture.lease.ordinal() + 1000;
            let mut alias = coordinator.records[&fixture.lease.ordinal()].clone();
            alias.ordinal = alias_ordinal;
            alias.state = LifecycleState::Ready;
            assert!(coordinator.records.insert(alias_ordinal, alias).is_none());
            let coordinator_before = format!("{coordinator:?}");
            let registry_before = format!("{:?}", holder.registry_for_test());
            let dispatch_before = format!("{executed:?}");

            let Err((error, returned)) =
                coordinator.complete_durable_validate_dispatch(&mut holder, executed)
            else {
                panic!("duplicate lifecycle record key must fail completion preflight")
            };
            assert_eq!(
                error,
                DurableValidateCompletionPublicationError::InvalidWaitingState
            );
            assert_eq!(format!("{returned:?}"), dispatch_before);
            assert_eq!(format!("{coordinator:?}"), coordinator_before);
            assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
        }
    }

    #[cfg(feature = "bls")]
    #[test]
    fn validate_completion_guard_restores_incumbent_on_unwind_before_swap() {
        let WaitingDurableValidateFixture {
            fixture: _,
            _directory,
            mut store,
            durable,
            coordinator,
            mut holder,
            dispatch,
        } = waiting_durable_validate_fixture(0xC8);
        let commitment = ValidatedBodyReceipt::for_test(durable).execution_commitment();
        let executed = dispatch
            .execute(&mut store, |_| Ok::<_, DetachedValidationError>(commitment))
            .expect("execute unwind completion fixture");
        let coordinator_before = format!("{coordinator:?}");
        let registry_before = format!("{:?}", holder.registry_for_test());
        let prepared = holder
            .registry_for_test_mut()
            .prepare_executed_durable_validate_completion(executed)
            .expect("reattach unwind completion fixture");

        let unwind = catch_unwind(AssertUnwindSafe(move || {
            let _staged = prepared
                .stage_executable_carrier()
                .expect("stage unwind-safe Validate carrier");
            panic!("test-only panic before coordinator swap");
        }));
        assert!(unwind.is_err());
        assert_eq!(format!("{coordinator:?}"), coordinator_before);
        assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
    }

    #[cfg(feature = "bls")]
    #[test]
    fn duplicate_old_digest_completion_cas_returns_exact_dispatch_intact() {
        let first = waiting_durable_validate_fixture(0xC9);
        let second = waiting_durable_validate_fixture(0xC9);
        let WaitingDurableValidateFixture {
            fixture: first_fixture,
            _directory: first_directory,
            store: mut first_store,
            durable: first_durable,
            coordinator: mut first_coordinator,
            holder: mut first_holder,
            dispatch: first_dispatch,
        } = first;
        let WaitingDurableValidateFixture {
            fixture: second_fixture,
            _directory: second_directory,
            store: mut second_store,
            durable: second_durable,
            coordinator: _second_coordinator,
            holder: _second_holder,
            dispatch: second_dispatch,
        } = second;
        assert_eq!(first_fixture.address, second_fixture.address);
        assert_eq!(first_durable, second_durable);
        let first_commitment =
            ValidatedBodyReceipt::for_test(first_durable.clone()).execution_commitment();
        let second_commitment =
            ValidatedBodyReceipt::for_test(second_durable).execution_commitment();
        let first_executed = first_dispatch
            .execute(&mut first_store, |_| {
                Ok::<_, DetachedValidationError>(first_commitment)
            })
            .expect("execute first duplicate-CAS fixture");
        let second_executed = second_dispatch
            .execute(&mut second_store, |_| {
                Ok::<_, DetachedValidationError>(second_commitment)
            })
            .expect("execute second duplicate-CAS fixture");
        let mut waiting_again = first_coordinator.clone();
        first_coordinator
            .complete_durable_validate_dispatch(&mut first_holder, first_executed)
            .expect("publish first exact completion carrier");
        let coordinator_before = format!("{waiting_again:?}");
        let registry_before = format!("{:?}", first_holder.registry_for_test());
        let dispatch_before = format!("{second_executed:?}");

        let Err((error, returned)) =
            waiting_again.complete_durable_validate_dispatch(&mut first_holder, second_executed)
        else {
            panic!("old-digest completion must not replace an installed completion")
        };
        assert!(matches!(
            error,
            DurableValidateCompletionPublicationError::Registry(
                DurableValidateCompletionConversionError::Execution(
                    DurableValidateExecutionError::Registry(RegistryError::DigestMismatch)
                        | DurableValidateExecutionError::WrongWorkKind
                )
            )
        ));
        assert_eq!(format!("{returned:?}"), dispatch_before);
        assert_eq!(format!("{waiting_again:?}"), coordinator_before);
        assert_eq!(
            format!("{:?}", first_holder.registry_for_test()),
            registry_before
        );
        drop(first_directory);
        drop(second_directory);
    }
