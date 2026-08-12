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
    concrete_admission::{DurableValidateDispatchError, LifecycleWorkRegistryHolder},
    schema::CapacityGeometry,
};
use super::*;
#[cfg(feature = "bls")]
use crate::sumeragi::v2_chunks::encode_payload;
use crate::sumeragi::{
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
        rank: super::super::SchedulerRank::new(3, 0, 0, 0, 0, 0, 0, 0),
        physical_slots: BTreeMap::from([(slot, digest)]),
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
        rank: super::super::SchedulerRank::new(3, 0, 0, 0, 0, 0, 0, 0),
        physical_slots: BTreeMap::from([(slot, digest)]),
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
    let effect = AdapterEffect::StoreBody {
        tag,
        round,
        subject,
    };
    let ownership = bind_adapter_effect_batch_ownership(
        core::slice::from_ref(&effect),
        vec![RuntimeEffectOwnership::fresh_for_test(
            tag,
            u128::from(marker) + 1,
        )],
    )
    .expect("bind exact durable Store fixture")
    .pop()
    .expect("one durable Store fixture owner");
    let pending = ownership
        .pending_adapter_effect_binding(&effect)
        .expect("mint sealed durable Store binding");
    let mut active_context_id = [0_u8; 32];
    active_context_id.copy_from_slice(context.id().0.as_ref());
    let request = projection::admission_request(
        LifecycleContext::new(LifecycleDigest::new(active_context_id), context.height),
        &verified,
        &effect,
        &pending,
    )
    .expect("project exact durable Store fixture");
    let AdmissionRequest::Candidate(candidate) = request else {
        panic!("Store fixture projects one lifecycle candidate")
    };
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
        rank: super::super::SchedulerRank::new(3, 0, 0, 0, 0, 0, 0, 0),
        physical_slots,
    };
    let expected_manifest_hash = HashOf::from_untyped_unchecked(Hash::new([marker, 0xC1]));
    let durable_receipt =
        DurableBodyReceipt::for_test(round.context_id, round, subject, expected_manifest_hash);
    let store = DurableStoreBody {
        address,
        effect: effect.clone(),
        pending,
        durable_receipt,
        expected_manifest_hash,
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
    let store_effect = AdapterEffect::StoreBody {
        tag,
        round,
        subject,
    };
    let ownership = bind_adapter_effect_batch_ownership(
        core::slice::from_ref(&store_effect),
        vec![RuntimeEffectOwnership::fresh_for_test(
            tag,
            u128::from(marker) + 1,
        )],
    )
    .expect("bind exact durable Validate parent fixture")
    .pop()
    .expect("one durable Validate parent fixture owner");
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
    let mut active_context_id = [0_u8; 32];
    active_context_id.copy_from_slice(context.id().0.as_ref());
    let request = projection::admission_request(
        LifecycleContext::new(LifecycleDigest::new(active_context_id), context.height),
        &verified,
        &effect,
        &pending,
    )
    .expect("project exact durable Validate fixture");
    let AdmissionRequest::Candidate(candidate) = request else {
        panic!("Validate fixture projects one lifecycle candidate")
    };
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
        rank: super::super::SchedulerRank::new(2, 0, 0, 0, 0, 0, 0, 0),
        physical_slots,
    };
    let expected_manifest_hash = HashOf::new(&manifest);
    let durable_receipt =
        DurableBodyReceipt::for_test(round.context_id, round, subject, expected_manifest_hash);
    let validate = DurableValidateBody {
        address,
        effect: effect.clone(),
        pending,
        durable_receipt,
        expected_manifest_hash,
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
            crate::sumeragi::synthetic_network_id("detached-validate-merge-chain"),
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
    validate.pending = upgraded_validate;
    assert!(validate.validates(digest));

    let context = fixture.verified.context();
    let mut context_id = [0_u8; 32];
    context_id.copy_from_slice(context.id().0.as_ref());
    let request = projection::admission_request(
        LifecycleContext::new(LifecycleDigest::new(context_id), context.height),
        &fixture.verified,
        &validate.effect,
        &validate.pending,
    )
    .expect("project commitment-authorized Validate fixture");
    assert!(fixture.registry.entries[&fixture.address].validates_at(fixture.address));
    let AdmissionRequest::Candidate(candidate) = request else {
        panic!("commitment-authorized Validate projects one candidate")
    };
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
    let request = projection::admission_request(
        active_context,
        &fixture.verified,
        &validate.effect,
        &validate.pending,
    )
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
        coordinator.reduce_admit(request),
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
