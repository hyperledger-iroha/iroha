use std::{
    cell::Cell,
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
    AdmissionDecision, CapacityClass, LifecycleCoordinator, LifecycleState,
    ProductionSchedulerInputsError, TerminalOutcome, WaitToken,
    concrete_admission::{
        DurableValidateDispatchError, LifecycleWorkRegistryHolder,
        ReadyValidateDemandAttestationError,
    },
    schema::CapacityGeometry,
};
use super::*;
#[cfg(feature = "bls")]
use crate::sumeragi::v2::{
    AdapterError, AdapterFingerprints, AuthenticatedConsensusMessage,
    DeferredAdmissionOrdinalSource,
};
#[cfg(feature = "bls")]
use crate::sumeragi::v2_chunks::encode_payload;
#[cfg(feature = "bls")]
use crate::sumeragi::v2_core as reducer;
use crate::sumeragi::{
    v2::{ExactLiveWalPersistedContinuationCause, LiveWalFrameIdentity},
    v2_core::{EventTag, Generation},
    v2_runtime::{RuntimeEffectOwnership, bind_adapter_effect_batch_ownership},
};

#[test]
fn registry_instance_identity_rejects_a_distinct_empty_registry() {
    let first = ConcreteLifecycleWorkRegistry::default();
    let identity = first.instance_identity();
    assert!(identity.same_instance(&first.instance_identity()));

    let second = ConcreteLifecycleWorkRegistry::default();
    assert!(
        !identity.same_instance(&second.instance_identity()),
        "equal empty registry contents cannot substitute for exact instance ownership"
    );
}

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
    .current_effect_producer(effect)
    .expect("seal direct signed producer")
    .mint_pending_binding()
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
    let replay = super::super::replay_authority::exact_record_fixture(context, stage_kind, marker);
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
        .current_effect_producer(&effect)
        .expect("seal registry fixture producer")
        .mint_pending_binding();
    ConcreteLifecycleWork::from_inert_fixture_for_test(effect, pending)
        .expect("construct exact concrete work")
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

#[test]
fn prospective_startup_census_rejects_extra_valid_carrier_before_publication() {
    let work = concrete(effect(0x61), 1);
    let expected_effect = work.effect().clone();
    let digest = work.digest();
    let owner = admitted_owner(&work, 1);
    let slot = PhysicalSlotId::for_capacity(CapacityClass::Effect, 0);
    let address = ConcreteWorkAddress::new(owner, 1, slot).expect("exact extra address");
    let mut registry = ConcreteLifecycleWorkRegistry::default();
    registry
        .install(address, digest, work)
        .expect("install internally valid but extraneous startup carrier");
    let coordinator = super::super::LifecycleCoordinator::new(
        LifecycleContext::new(LifecycleDigest::new([0x62; 32]), 7),
        0,
        super::super::schema::CapacityGeometry::new(
            CapacityClass::ALL.into_iter().map(|class| (class, 8)),
        ),
    );
    let batch = PreparedCertifiedServeRegistryBatchV1 {
        entries: Vec::new(),
    };
    let invoked = Cell::new(false);
    let result = registry.install_certified_serve_startup_batch_before_publication(
        batch,
        &coordinator,
        || {
            invoked.set(true);
            Ok::<(), ()>(())
        },
    );
    assert!(matches!(
        result,
        Err(CertifiedServeRegistryBatchPublicationError::Preflight(_))
    ));
    assert!(!invoked.get(), "Ledger publication must not be invoked");
    assert!(registry.exactly_contains(address, &expected_effect));
}

#[test]
fn complete_startup_census_rejects_live_store_without_a_carrier() {
    let candidate = recovered_wal_projection_candidate(
        LifecyclePhase::Store,
        LifecycleWorkClass::Store,
        LifecycleStageKind::StoreBody,
        0x63,
    );
    let context = LifecycleContext::new(candidate.key.context(), candidate.key.round().height());
    let mut coordinator = LifecycleCoordinator::new(
        context,
        0,
        super::super::schema::CapacityGeometry::new(
            CapacityClass::ALL.into_iter().map(|class| (class, 8)),
        ),
    );
    assert!(matches!(
        coordinator.reduce_admit(AdmissionRequest::Candidate(candidate)),
        super::super::AdmissionDecision::Admitted { .. }
    ));
    let registry = ConcreteLifecycleWorkRegistry::default();

    assert!(!registry.exactly_covers_recovered_ready_work(&coordinator));
    assert!(!registry.exactly_covers_recovered_ready_body_pipeline(&coordinator));
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
        .current_effect_producer(&effect)
        .expect("seal durable Store producer")
        .mint_pending_binding();
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
    durable_validate_fixture_at_view(marker, 2)
}

#[cfg(feature = "bls")]
#[allow(clippy::too_many_lines)]
fn durable_validate_fixture_at_view(marker: u8, view: wire::View) -> DurableValidateFixture {
    durable_validate_fixture_at_view_with_parent(marker, view, None)
}

#[cfg(feature = "bls")]
#[allow(clippy::too_many_lines)]
fn durable_validate_fixture_at_view_with_parent(
    marker: u8,
    view: wire::View,
    parent_block_hash: Option<HashOf<BlockHeader>>,
) -> DurableValidateFixture {
    let (mut verified, mut context) = verified_store_context(marker);
    if let Some(parent_block_hash) = parent_block_hash.as_ref() {
        let predecessor_context = context.clone();
        let predecessor_proofs = verified.proofs_of_possession().to_vec();
        let parent_round = wire::ConsensusRound {
            context_id: predecessor_context.id(),
            height: predecessor_context.height,
            view: 0,
        };
        context.height = predecessor_context.height + 1;
        context.parent_commit_qc = Some(wire::QuorumCertificate {
            round: parent_round,
            proposal_round: parent_round,
            phase: wire::GlobalPhase::Commit,
            subject: wire::BlockSubject {
                parent_block_hash: None,
                block_hash: *parent_block_hash,
                payload_hash: Hash::new([marker, 0xD1]),
            },
            execution_commitment: wire::ExecutionCommitment::without_topups_or_merge_carrier(
                Hash::new([marker, 0xD2]),
                Hash::new([marker, 0xD3]),
                Hash::new([marker, 0xD4]),
                1,
                Hash::new([marker, 0xD5]),
            ),
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0xD6],
        });
        verified = VerifiedHeightContext::successor_fixture_for_test(
            context.clone(),
            predecessor_proofs.clone(),
            predecessor_context,
            predecessor_proofs,
        );
    }
    let keys = durable_store_keys(marker);
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view,
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
        parent_block_hash,
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
        parent_block_hash,
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
        .current_effect_producer(&store_effect)
        .expect("seal durable Validate parent producer")
        .mint_pending_binding();
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
            crate::sumeragi::synthetic_network_id("detached Validate merge chain"),
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
fn exact_detached_validation_merge_reference(
    durable: &DurableBodyReceipt,
) -> CertifiedMergeLedgerReference {
    let mut reference = detached_validation_merge_reference(durable);
    reference.merge_qc.carrier_parent_hash = durable
        .subject()
        .parent_block_hash
        .expect("sidecar Validate fixture retains a carrier parent");
    reference
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
    durable_validate_store_fixture_at_view(marker, 2)
}

#[cfg(feature = "bls")]
fn durable_validate_store_fixture_at_view(
    marker: u8,
    view: wire::View,
) -> (
    DurableValidateFixture,
    TempDir,
    V2BodyStore,
    DurableBodyReceipt,
) {
    durable_validate_store_fixture_at_view_with_commitment(marker, view, None)
}

#[cfg(feature = "bls")]
fn durable_validate_store_fixture_at_view_with_commitment(
    marker: u8,
    view: wire::View,
    execution_commitment: Option<wire::ExecutionCommitment>,
) -> (
    DurableValidateFixture,
    TempDir,
    V2BodyStore,
    DurableBodyReceipt,
) {
    let fixture = durable_validate_fixture_at_view(marker, view);
    durable_validate_store_fixture_from_fixture(fixture, execution_commitment)
}

#[cfg(feature = "bls")]
fn durable_validate_sidecar_store_fixture(
    marker: u8,
) -> (
    DurableValidateFixture,
    TempDir,
    V2BodyStore,
    DurableBodyReceipt,
) {
    let parent = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new([marker, 0xE1]));
    let fixture = durable_validate_fixture_at_view_with_parent(marker, 2, Some(parent));
    durable_validate_store_fixture_from_fixture(fixture, None)
}

#[cfg(feature = "bls")]
fn durable_validate_store_fixture_from_fixture(
    mut fixture: DurableValidateFixture,
    execution_commitment: Option<wire::ExecutionCommitment>,
) -> (
    DurableValidateFixture,
    TempDir,
    V2BodyStore,
    DurableBodyReceipt,
) {
    let directory = TempDir::new().expect("temporary detached Validate body store");
    let mut store = V2BodyStore::open(directory.path(), fixture.verified.context().clone())
        .expect("open detached Validate body store");
    let durable = store
        .store(fixture.manifest.clone(), fixture.canonical_wire.clone())
        .expect("persist detached Validate fixture body");
    assert_eq!(durable.manifest_hash(), fixture.expected_manifest_hash);
    let AdapterEffect::ValidateBody {
        tag,
        round,
        subject,
    } = fixture.effect.clone()
    else {
        unreachable!("detached Validate fixture retains one Validate effect")
    };
    let mut certificate =
        certified_pipeline_prepare_certificate_for_test(&fixture.manifest, &durable);
    if let Some(execution_commitment) = execution_commitment {
        certificate.execution_commitment = execution_commitment;
    }
    let fetch_effect = AdapterEffect::FetchBody {
        tag,
        round,
        subject,
        manifest: Some(fixture.manifest.clone()),
        certified_sources: Vec::new(),
        certificate: Some(certificate.clone()),
    };
    let store_effect = AdapterEffect::StoreBody {
        tag,
        round,
        subject,
    };
    let ordinal = fixture.lease.ordinal();
    let fetch_ownership = bind_adapter_effect_batch_ownership(
        core::slice::from_ref(&fetch_effect),
        vec![RuntimeEffectOwnership::fresh_for_test(tag, ordinal)],
    )
    .expect("bind persisted Validate Fetch fixture")
    .pop()
    .expect("one persisted Validate Fetch fixture owner");
    let store_ownership = fetch_ownership
        .rebind_as_inherited_adapter_effect(&store_effect)
        .expect("carry persisted Fetch authority into Validate parent Store");
    let store_pending = store_ownership
        .current_effect_producer(&store_effect)
        .map(|producer| producer.mint_pending_binding())
        .expect("mint persisted Validate parent binding");
    let pending = store_pending
        .project_store_validate_successor(&store_effect, &fixture.effect)
        .expect("project persisted Store-to-Validate fixture lineage");
    let (_store_replay, validate_replay) =
        certified_pipeline_replay_evidence_with_certificate_for_test(
            tag,
            &fixture.manifest,
            &durable,
            &pending,
            certificate,
        )
        .expect("bind persisted Validate replay to its exact body receipt");
    let replay_evidence = DurableValidateReplayEvidenceV1::certified(validate_replay);
    let candidate = replay_evidence
        .project_installed_validate_candidate(
            InstalledBodyCandidateProjectionPermit::new(),
            &fixture.verified,
            &fixture.effect,
            &durable,
            &pending,
        )
        .expect("project persisted replay-authorized Validate fixture");
    let (physical_slots, slot_universe, consumed_slots) = candidate
        .physical_geometry
        .normalized()
        .expect("normalize persisted Validate fixture geometry");
    assert_eq!(slot_universe, consumed_slots);
    assert_eq!(physical_slots.len(), 1);
    let (&slot, &digest) = physical_slots
        .first_key_value()
        .expect("one persisted Validate fixture slot");
    let owner = OwnerId::new(candidate.causal_root, ordinal);
    let address = ConcreteWorkAddress::new(owner, ordinal, slot)
        .expect("exact persisted Validate registry address");
    let removed = fixture
        .registry
        .entries
        .remove(&fixture.address)
        .expect("replace the synthetic Validate fixture after persistence");
    assert!(fixture.registry.entries.is_empty());
    drop(removed);
    let validate = DurableValidateBody {
        address,
        effect: fixture.effect.clone(),
        pending,
        durable_receipt: durable.clone(),
        expected_manifest_hash: fixture.expected_manifest_hash,
        replay_evidence,
    };
    assert!(validate.validates(digest));
    let work = ConcreteLifecycleWork {
        digest,
        kind: ConcreteLifecycleWorkKind::DurableValidateBody(validate),
    };
    assert!(work.validates_at(address));
    fixture.address = address;
    fixture.slot = slot;
    fixture.lease.owner = owner;
    fixture.lease.key = candidate.key;
    fixture.lease.work_class = candidate.work_class;
    fixture.lease.stage = candidate.stage;
    fixture.lease.physical_slots = physical_slots;
    fixture.store_ownership = store_ownership;
    assert!(work.validates_at(fixture.address));
    assert!(fixture.registry.entries.insert(address, work).is_none());
    (fixture, directory, store, durable)
}

#[cfg(feature = "bls")]
fn claimed_durable_validate_coordinator(fixture: &DurableValidateFixture) -> LifecycleCoordinator {
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
    waiting_durable_validate_fixture_at_view(marker, 2)
}

#[cfg(feature = "bls")]
fn waiting_durable_validate_fixture_at_view(
    marker: u8,
    view: wire::View,
) -> WaitingDurableValidateFixture {
    waiting_durable_validate_fixture_from_store(durable_validate_store_fixture_at_view(
        marker, view,
    ))
}

#[cfg(feature = "bls")]
fn waiting_durable_validate_sidecar_fixture(marker: u8) -> WaitingDurableValidateFixture {
    waiting_durable_validate_fixture_from_store(durable_validate_sidecar_store_fixture(marker))
}

#[cfg(feature = "bls")]
fn waiting_durable_validate_fixture_from_store(
    (mut fixture, directory, store, durable): (
        DurableValidateFixture,
        TempDir,
        V2BodyStore,
        DurableBodyReceipt,
    ),
) -> WaitingDurableValidateFixture {
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
    ready_durable_validate_fixture_at_view(marker, 2, outcome)
}

#[cfg(feature = "bls")]
fn ready_durable_validate_fixture_at_view(
    marker: u8,
    view: wire::View,
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
    } = waiting_durable_validate_fixture_at_view(marker, view);
    let executed = match outcome {
        ReadyDurableValidateFixtureOutcome::Validated => {
            let commitment = ValidatedBodyReceipt::for_test(durable.clone()).execution_commitment();
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
    let _publication = coordinator
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
