//! Native producer coverage over an actually executed/certified four-validator prefix.
//!
//! The DKG/session is explicit component prestate; Parliament demand enters through canonical
//! attempt admission and its derived index. The prefix's genesis, native parent hash and R are
//! real; this fixture does not claim live-network qualification of the seeded component rows.

use super::*;
use crate::{
    beacon::{
        FinalizedGlobalThresholdBeaconKeySessionRecordV1, GlobalThresholdBeaconCapabilityErrorV1,
        GlobalThresholdBeaconPartialSigningCapabilityV1,
        InMemoryGlobalThresholdBeaconPartialSignerV1, ValidatedGlobalThresholdBeaconSessionV1,
        prepared_session_and_signers_fixture_for_keys_v1,
    },
    state::{GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY, World},
    sumeragi::test_chain::{CertifiedTestChain, TestChainConfig},
};
use iroha_crypto::{Algorithm, Hash, KeyPair};
use iroha_data_model::{
    consensus::{
        GLOBAL_THRESHOLD_BEACON_VERSION_V1, GlobalThresholdBeaconDkgSessionV1,
        GlobalThresholdBeaconPartialSignatureV1,
    },
    governance::types::{BeaconSessionId, GovernanceAttemptId},
};
use std::{
    collections::BTreeSet,
    io::Write as _,
    sync::atomic::{AtomicUsize, Ordering},
};

struct CountedSigner {
    inner: InMemoryGlobalThresholdBeaconPartialSignerV1,
    count: Arc<AtomicUsize>,
}
impl GlobalThresholdBeaconPartialSignerV1 for CountedSigner {
    fn attest_partial_signing_capability(
        &self,
        session: &ValidatedGlobalThresholdBeaconSessionV1,
        expected_signer_index: u16,
    ) -> Result<
        GlobalThresholdBeaconPartialSigningCapabilityV1,
        GlobalThresholdBeaconCapabilityErrorV1,
    > {
        self.inner
            .attest_partial_signing_capability(session, expected_signer_index)
    }
    fn sign_partial(
        &self,
        session: &ValidatedGlobalThresholdBeaconSessionV1,
        payload: &[u8],
    ) -> Result<GlobalThresholdBeaconPartialSignatureV1, String> {
        self.count.fetch_add(1, Ordering::SeqCst);
        self.inner.sign_partial(session, payload)
    }
}

struct Fixture {
    chain: CertifiedTestChain,
    keys: Vec<PublicKey>,
    producers: Vec<NativeBeaconProducer>,
    counts: Vec<Arc<AtomicUsize>>,
    context: ApplicationControlContext,
}
fn prefix() -> CertifiedTestChain {
    let mut chain = CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
    while chain.height() < 8 {
        chain.commit(Vec::new());
    }
    chain
}
fn context(chain: &CertifiedTestChain) -> ApplicationControlContext {
    let parent = chain.committed(chain.height());
    let view = chain.state().view();
    let scheduled = view
        .world()
        .consensus_schedule()
        .ready(chain.height() + 1)
        .unwrap();
    ApplicationControlContext {
        instance: chain.instance(),
        epoch: schedule::core_epoch(&scheduled.epoch).unwrap().id,
        height: chain.height() + 1,
        parent_hash: parent.core_hash(),
        parent_result: parent.result(),
    }
}
fn build_context(context: &ApplicationControlContext, view: u64) -> ControlWitnessContext {
    ControlWitnessContext {
        height: context.height,
        view,
        epoch: context.epoch,
        parent_hash: context.parent_hash,
        parent_result: context.parent_result,
    }
}
fn fixture() -> Fixture {
    let chain = prefix();
    let mut pairs = (0xC1..=0xC4)
        .map(|seed| KeyPair::from_seed(vec![seed; 32], Algorithm::BlsNormal))
        .collect::<Vec<_>>();
    pairs.sort_by(|left, right| left.public_key().cmp(right.public_key()));
    let roster = pairs
        .iter()
        .map(|pair| iroha_model_base::peer::PeerId::new(pair.public_key().clone()))
        .collect::<Vec<_>>();
    assert_eq!(
        roster,
        chain
            .validators()
            .iter()
            .map(|(peer, _)| peer.clone())
            .collect::<Vec<_>>()
    );
    let current = chain
        .state()
        .view()
        .world()
        .consensus_schedule()
        .ready(9)
        .unwrap()
        .epoch
        .clone();
    let id: [u8; 32] = Hash::new(b"native producer exact fixture DKG").into();
    let dkg = GlobalThresholdBeaconDkgSessionV1 {
        version: GLOBAL_THRESHOLD_BEACON_VERSION_V1,
        network_id: chain.network_id(),
        session_id: id,
        attempt_id: id,
        authority_generation: current.authority.generation,
        roster_hash: crate::beacon::global_threshold_beacon_roster_hash_v1(&roster),
        committee_size: 4,
        threshold: 2,
        start_height: 1,
        commitments_end_height: 2,
        deliveries_end_height: 3,
        acceptances_end_height: 4,
    };
    let (session, signers) = prepared_session_and_signers_fixture_for_keys_v1(dkg, &pairs);
    let mut record =
        FinalizedGlobalThresholdBeaconKeySessionRecordV1::new(session.record().clone()).unwrap();
    record
        .activate(session.record().adaptive_dkg.finalized_at_height)
        .unwrap();
    assert!(session.record().adaptive_dkg.finalized_at_height <= chain.height());
    let (attempt_id, _, attempt) =
        crate::beacon::tests::pending_batched_sortition_attempt(&chain.network_id(), &roster, 9);
    chain.setup_world_at(2_000, |transaction| {
        transaction
            .world
            .global_beacon_key_sessions
            .insert(id, record);
        transaction
            .world
            .global_beacon_active_session
            .insert(GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY, id);
        transaction
            .world
            .put_parliament_attempt(attempt)
            .expect("admit the producer's Parliament demand");
        assert_eq!(
            transaction
                .world
                .parliament_required_beacon_pulse_slots
                .get(&(BeaconSessionId::for_network_v1(&chain.network_id()), 9)),
            Some(&BTreeSet::from([attempt_id]))
        );
    });
    let counts = pairs
        .iter()
        .map(|_| Arc::new(AtomicUsize::new(0)))
        .collect::<Vec<_>>();
    let keys = pairs
        .iter()
        .map(|pair| PublicKey::new(pair.public_key().try_to_bytes().unwrap().1.to_vec()).unwrap())
        .collect::<Vec<_>>();
    let producers = keys
        .iter()
        .zip(signers)
        .zip(&counts)
        .map(|((key, signer), count)| {
            NativeBeaconProducer::new(
                chain.instance(),
                Some(key.as_bytes().try_into().unwrap()),
                Some(Arc::new(CountedSigner {
                    inner: signer,
                    count: count.clone(),
                })),
            )
        })
        .collect();
    let context = context(&chain);
    Fixture {
        chain,
        keys,
        producers,
        counts,
        context,
    }
}

#[test]
fn all_seats_drive_real_shares_once_and_followers_use_only_transported_pulse() {
    let mut fixture = fixture();
    let source = fixture.chain.state().view();
    let applied = (8, fixture.context.parent_hash);
    let messages = fixture
        .producers
        .iter_mut()
        .map(|producer| ApplicationControl {
            context: fixture.context,
            bytes: producer
                .drive(&source, &fixture.context, applied)
                .unwrap()
                .unwrap(),
        })
        .collect::<Vec<_>>();
    let mut observer = NativeBeaconProducer::new(fixture.chain.instance(), None, None);
    assert!(
        observer
            .drive(&source, &fixture.context, applied)
            .unwrap()
            .is_none()
    );
    assert!(matches!(
        observer.build(&build_context(&fixture.context, 0)),
        Err(NativeBeaconError::AwaitingShares { height: 9 })
    ));
    for producer in &fixture.producers {
        assert!(matches!(
            producer.build(&build_context(&fixture.context, 0)),
            Err(NativeBeaconError::AwaitingShares { height: 9 })
        ));
    }
    for (key, message) in fixture.keys.iter().zip(&messages) {
        observer.accept(&source, applied, key, message).unwrap();
        for producer in &mut fixture.producers {
            producer.accept(&source, applied, key, message).unwrap();
        }
    }
    let (witness, attest) = observer.build(&build_context(&fixture.context, 0)).unwrap();
    assert!(
        !attest,
        "Permissioned Parliament pulse is not an epoch boundary"
    );
    assert!(!witness.is_empty());
    assert!(witness.len() < iroha_sumeragi::types::MAX_CONTROL_WITNESS_BYTES);
    let pulse = control::decode(&witness).unwrap().unwrap();
    let mut restarted = NativeBeaconProducer::new(fixture.chain.instance(), None, None);
    for index in [2, 3] {
        restarted
            .accept(&source, applied, &fixture.keys[index], &messages[index])
            .unwrap();
    }
    assert_eq!(
        restarted
            .build(&build_context(&fixture.context, 38))
            .unwrap()
            .0,
        witness,
        "a restarted reducer and a different valid threshold subset produce the exact same pulse"
    );
    let partial = control::decode_partial(&messages[0].bytes).unwrap();
    let session = fixture.producers[0]
        .active
        .as_ref()
        .unwrap()
        .aggregator
        .session();
    for mutation in 0..5 {
        let mut changed = pulse.context;
        match mutation {
            0 => changed.instance[0] ^= 1,
            1 => changed.epoch += 1,
            2 => changed.epoch_context_id[0] ^= 1,
            3 => changed.parent_consensus_hash[0] ^= 1,
            _ => changed.parent_result[0] ^= 1,
        }
        let mut foreign = GlobalThresholdBeaconPulseAggregatorV1::new(
            session.clone(),
            pulse.height,
            pulse.finalized_chain_anchor,
            changed,
        )
        .unwrap();
        assert!(
            foreign.accept_partial(partial).is_err(),
            "each complete native context field changes the threshold signing preimage"
        );
        let mut changed_pulse = pulse;
        changed_pulse.context = changed;
        assert!(
            crate::beacon::verify_finalized_global_threshold_beacon_pulse_v1(
                session,
                &changed_pulse,
                pulse.finalized_chain_anchor,
                &changed,
            )
            .is_err(),
            "changing both expected and supplied context cannot reuse the old signature"
        );
    }
    for (index, producer) in fixture.producers.iter_mut().enumerate() {
        assert_eq!(
            producer
                .build(&build_context(&fixture.context, 37))
                .unwrap(),
            (witness, false)
        );
        assert_eq!(
            producer.drive(&source, &fixture.context, applied).unwrap(),
            Some(messages[index].bytes)
        );
        assert_eq!(
            fixture.counts[index].load(Ordering::SeqCst),
            1,
            "build and retransmission never sign twice"
        );
    }
    assert_eq!(
        observer
            .accept(&source, applied, &fixture.keys[0], &messages[0])
            .unwrap(),
        IngressOutcome::Duplicate
    );
    control::verify_result(&witness, Some(&pulse)).unwrap();
    assert!(control::verify_result(&witness, None).is_err());
    let current = &source.world().consensus_schedule().ready(9).unwrap().epoch;
    super::super::capture(
        iroha_data_model::block::consensus::SumeragiRootScope::Global,
        source.world(),
        source.block_hashes(),
        current,
        9,
        Some(pulse),
        Some(pulse_context(&fixture.context)),
    )
    .unwrap();
    assert!(
        super::super::capture(
            iroha_data_model::block::consensus::SumeragiRootScope::Global,
            source.world(),
            source.block_hashes(),
            current,
            9,
            None,
            Some(pulse_context(&fixture.context))
        )
        .is_err()
    );
    for mutation in 0..7 {
        let mut altered = pulse;
        match mutation {
            0 => altered.signature[0] ^= 1,
            1 => altered.seed[0] ^= 1,
            2 => altered.height -= 1,
            3 => altered.transcript_hash[0] ^= 1,
            4 => {
                altered.network_id = iroha_data_model::NetworkId::from_genesis_hash(
                    fixture.chain.committed(2).block_hash(),
                )
            }
            5 => {
                altered.finalized_chain_anchor.block_hash = fixture.chain.committed(7).block_hash()
            }
            _ => altered.version ^= 1,
        }
        assert!(
            super::super::capture(
                iroha_data_model::block::consensus::SumeragiRootScope::Global,
                source.world(),
                source.block_hashes(),
                current,
                9,
                Some(altered),
                Some(pulse_context(&fixture.context))
            )
            .is_err()
        );
        assert!(control::verify_result(&witness, Some(&altered)).is_err());
    }
    assert!(
        control::decode(&messages[0].bytes).is_err(),
        "partial is not a finalized pulse frame"
    );
    assert!(control::decode_partial(&witness).is_err());
    let mut trailing = witness;
    trailing.write_all(&[0]).unwrap();
    assert!(control::decode(&trailing).is_err());
    let hashes = (1..=fixture.chain.height())
        .map(|height| fixture.chain.committed(height).block_hash())
        .collect::<Vec<_>>();
    crate::state::validator_committee::validate_committed_progress(
        source.world(),
        source.chain_id(),
        fixture.chain.network_id(),
        &hashes,
        fixture.chain.kura(),
    )
    .unwrap();
    drop(source);
    fixture.chain.setup_world_at(2_000, |transaction| {
        transaction
            .world
            .global_beacon_pulses
            .insert(pulse.pulse_id, pulse);
        transaction.world.global_beacon_pulse_slots.insert(
            (
                BeaconSessionId::for_network_v1(&fixture.chain.network_id()),
                pulse.height,
            ),
            pulse.pulse_id,
        );
        transaction.world.global_beacon_latest_pulse.insert(
            GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY,
            crate::beacon::validate_persisted_global_threshold_beacon_pulse_v1(&pulse).unwrap(),
        );
    });
    let source = fixture.chain.state().view();
    let error = crate::state::validator_committee::validate_committed_progress(
        source.world(),
        source.chain_id(),
        fixture.chain.network_id(),
        &hashes,
        fixture.chain.kura(),
    )
    .unwrap_err();
    assert_eq!(
        error,
        "restored beacon history adds or omits certified native pulse work"
    );
}

#[test]
fn wrong_source_sender_and_proof_never_change_the_owned_round() {
    let mut fixture = fixture();
    let source = fixture.chain.state().view();
    let applied = (8, fixture.context.parent_hash);
    let own = fixture.producers[0]
        .drive(&source, &fixture.context, applied)
        .unwrap()
        .unwrap();
    for mutation in 0..6 {
        let mut wrong = fixture.context;
        match mutation {
            0 => wrong.instance.0[0] ^= 1,
            1 => wrong.height += 1,
            2 => wrong.epoch.epoch += 1,
            3 => wrong.epoch.context.0[0] ^= 1,
            4 => wrong.parent_hash.0[0] ^= 1,
            _ => wrong.parent_result.0[0] ^= 1,
        }
        assert!(
            fixture.producers[0]
                .drive(&source, &wrong, applied)
                .is_err()
        );
        assert_eq!(
            fixture.producers[0]
                .drive(&source, &fixture.context, applied)
                .unwrap(),
            Some(own)
        );
    }
    assert!(
        fixture.producers[0]
            .drive(&source, &fixture.context, (7, applied.1))
            .is_err()
    );
    let other = ApplicationControl {
        context: fixture.context,
        bytes: fixture.producers[1]
            .drive(&source, &fixture.context, applied)
            .unwrap()
            .unwrap(),
    };
    assert!(matches!(
        fixture.producers[0].accept(&source, applied, &fixture.keys[2], &other),
        Err(NativeBeaconError::Sender)
    ));
    let mut partial = control::decode_partial(&other.bytes).unwrap();
    partial.signature_share[0] ^= 1;
    let bad = ApplicationControl {
        context: fixture.context,
        bytes: control::encode_partial(&partial).unwrap(),
    };
    assert!(
        fixture.producers[0]
            .accept(&source, applied, &fixture.keys[1], &bad)
            .is_err()
    );
    assert!(matches!(
        fixture.producers[0].build(&build_context(&fixture.context, 0)),
        Err(NativeBeaconError::AwaitingShares { .. })
    ));
    assert_eq!(
        fixture.producers[0]
            .accept(&source, applied, &fixture.keys[1], &other)
            .unwrap(),
        IngressOutcome::Finalized
    );
    assert_eq!(fixture.counts[0].load(Ordering::SeqCst), 1);
    drop(source);
    fixture.chain.setup_world_at(2_000, |transaction| {
        transaction
            .world
            .parliament_unavailable_beacon_pulse_slots
            .insert(
                (
                    BeaconSessionId::for_network_v1(&fixture.chain.network_id()),
                    9,
                ),
                BTreeSet::from([GovernanceAttemptId::new([0xA1; 32])]),
            );
    });
    let source = fixture.chain.state().view();
    let mut fresh = NativeBeaconProducer::new(fixture.chain.instance(), None, None);
    assert!(
        matches!(
            fresh.drive(&source, &fixture.context, applied),
            Err(NativeBeaconError::Source(_))
        ),
        "an unavailable committed slot cannot open a new local signing round"
    );
}

#[test]
fn explicitly_anchored_no_demand_needs_neither_session_nor_fake_observer_key() {
    let chain = prefix();
    let context = context(&chain);
    let source = chain.state().view();
    let mut observer = NativeBeaconProducer::new(chain.instance(), None, None);
    assert!(
        observer.build(&build_context(&context, 0)).is_err(),
        "no unanchored empty default"
    );
    assert!(
        observer
            .drive(&source, &context, (8, context.parent_hash))
            .unwrap()
            .is_none()
    );
    let (witness, attest) = observer.build(&build_context(&context, 5)).unwrap();
    assert!(witness.is_empty());
    assert!(!attest);
    assert_eq!(control::decode(&witness).unwrap(), None);
    control::verify_result(&witness, None).unwrap();
    let current = &source.world().consensus_schedule().ready(9).unwrap().epoch;
    super::super::capture(
        iroha_data_model::block::consensus::SumeragiRootScope::Global,
        source.world(),
        source.block_hashes(),
        current,
        9,
        None,
        Some(pulse_context(&context)),
    )
    .unwrap();
}

#[test]
fn refusal_classification_keeps_remote_faults_out_of_local_recovery() {
    use NativeControlFailure::{RecoveryRequired, Rejected, Retryable};
    assert_eq!(
        NativeBeaconError::Context.ingress_classification(),
        Rejected
    );
    assert_eq!(NativeBeaconError::Context.local_classification(), Retryable);
    assert_eq!(NativeBeaconError::Sender.ingress_classification(), Rejected);
    assert_eq!(
        NativeBeaconError::LocalSigning.local_classification(),
        Retryable
    );
    assert_eq!(
        NativeBeaconError::AwaitingShares { height: 9 }.local_classification(),
        Retryable
    );
    let source = NativeBeaconError::Source("claimed applied parent is missing".into());
    assert_eq!(source.local_classification(), RecoveryRequired);
    assert_eq!(source.ingress_classification(), RecoveryRequired);
    let malformed = NativeBeaconError::Codec(control::ControlCodecError::ResultMismatch);
    assert_eq!(malformed.ingress_classification(), Rejected);
    assert_eq!(malformed.local_classification(), RecoveryRequired);
}

struct FaultSigner {
    inner: Arc<dyn GlobalThresholdBeaconPartialSignerV1>,
    attempts: Arc<AtomicUsize>,
    corrupt: bool,
}
impl GlobalThresholdBeaconPartialSignerV1 for FaultSigner {
    fn attest_partial_signing_capability(
        &self,
        session: &ValidatedGlobalThresholdBeaconSessionV1,
        expected_signer_index: u16,
    ) -> Result<
        GlobalThresholdBeaconPartialSigningCapabilityV1,
        GlobalThresholdBeaconCapabilityErrorV1,
    > {
        self.inner
            .attest_partial_signing_capability(session, expected_signer_index)
    }
    fn sign_partial(
        &self,
        session: &ValidatedGlobalThresholdBeaconSessionV1,
        payload: &[u8],
    ) -> Result<GlobalThresholdBeaconPartialSignatureV1, String> {
        let attempt = self.attempts.fetch_add(1, Ordering::SeqCst);
        if !self.corrupt && attempt == 0 {
            return Err("explicit transient component signer refusal".into());
        }
        let mut partial = self.inner.sign_partial(session, payload)?;
        if self.corrupt {
            partial.signature_share[0] ^= 1;
        }
        Ok(partial)
    }
}

#[test]
fn native_invalid_local_share_is_never_retained_or_counted() {
    let mut fixture = fixture();
    let attempts = Arc::new(AtomicUsize::new(0));
    let signer = fixture.producers[0].signer.take().unwrap();
    fixture.producers[0].signer = Some(Arc::new(FaultSigner {
        inner: signer,
        attempts: attempts.clone(),
        corrupt: true,
    }));
    let source = fixture.chain.state().view();
    let applied = (8, fixture.context.parent_hash);
    assert!(matches!(
        fixture.producers[0].drive(&source, &fixture.context, applied),
        Err(NativeBeaconError::LocalSigning)
    ));
    let active = fixture.producers[0].active.as_ref().unwrap();
    assert!(active.own.is_none());
    assert!(active.finalized.is_none());
    let payload = active.aggregator.payload().as_ptr();
    let remote = ApplicationControl {
        context: fixture.context,
        bytes: fixture.producers[1]
            .drive(&source, &fixture.context, applied)
            .unwrap()
            .unwrap(),
    };
    assert_eq!(
        fixture.producers[0]
            .accept(&source, applied, &fixture.keys[1], &remote)
            .unwrap(),
        IngressOutcome::Accepted,
        "one remote share cannot combine with an invalid local share"
    );
    assert!(matches!(
        fixture.producers[0].build(&build_context(&fixture.context, 0)),
        Err(NativeBeaconError::AwaitingShares { height: 9 })
    ));
    assert!(matches!(
        fixture.producers[0].drive(&source, &fixture.context, applied),
        Err(NativeBeaconError::LocalSigning)
    ));
    let active = fixture.producers[0].active.as_ref().unwrap();
    assert_eq!(active.aggregator.payload().as_ptr(), payload);
    assert!(active.own.is_none());
    assert!(active.finalized.is_none());
    assert_eq!(attempts.load(Ordering::SeqCst), 2);
    let mut bad = control::decode_partial(&remote.bytes).unwrap();
    bad.signature_share[0] ^= 1;
    assert!(
        fixture.producers[0]
            .accept(
                &source,
                applied,
                &fixture.keys[1],
                &ApplicationControl {
                    context: fixture.context,
                    bytes: control::encode_partial(&bad).unwrap(),
                }
            )
            .is_err()
    );
}

#[test]
fn native_transient_signer_refusal_retains_remote_progress_and_exact_retry_payload() {
    let mut fixture = fixture();
    let attempts = Arc::new(AtomicUsize::new(0));
    let signer = fixture.producers[0].signer.take().unwrap();
    fixture.producers[0].signer = Some(Arc::new(FaultSigner {
        inner: signer,
        attempts: attempts.clone(),
        corrupt: false,
    }));
    let source = fixture.chain.state().view();
    let applied = (8, fixture.context.parent_hash);
    assert!(matches!(
        fixture.producers[0].drive(&source, &fixture.context, applied),
        Err(NativeBeaconError::LocalSigning)
    ));
    let active = fixture.producers[0].active.as_ref().unwrap();
    let original_payload = active.aggregator.payload().as_ptr();
    assert!(active.own.is_none());
    let remote = ApplicationControl {
        context: fixture.context,
        bytes: fixture.producers[1]
            .drive(&source, &fixture.context, applied)
            .unwrap()
            .unwrap(),
    };
    assert_eq!(
        fixture.producers[0]
            .accept(&source, applied, &fixture.keys[1], &remote)
            .unwrap(),
        IngressOutcome::Accepted
    );
    assert_eq!(
        attempts.load(Ordering::SeqCst),
        1,
        "incoming authentic progress does not rerun a failed local signer"
    );
    let own = fixture.producers[0]
        .drive(&source, &fixture.context, applied)
        .unwrap()
        .unwrap();
    assert_eq!(
        fixture.producers[0]
            .active
            .as_ref()
            .unwrap()
            .aggregator
            .payload()
            .as_ptr(),
        original_payload
    );
    let finalized = fixture.producers[0]
        .build(&build_context(&fixture.context, 0))
        .unwrap();
    assert!(!finalized.0.is_empty());
    assert_eq!(attempts.load(Ordering::SeqCst), 2);
    assert_eq!(
        fixture.producers[0]
            .drive(&source, &fixture.context, applied)
            .unwrap(),
        Some(own)
    );
    assert_eq!(
        fixture.producers[0]
            .build(&build_context(&fixture.context, 37))
            .unwrap(),
        finalized
    );
    assert_eq!(attempts.load(Ordering::SeqCst), 2);
    assert_eq!(
        fixture.producers[0]
            .accept(&source, applied, &fixture.keys[1], &remote)
            .unwrap(),
        IngressOutcome::Duplicate
    );
}

#[test]
fn native_mandatory_slot_starts_without_transaction_work_and_missing_key_refuses() {
    let mut fixture = fixture();
    assert!(
        fixture
            .counts
            .iter()
            .all(|count| count.load(Ordering::SeqCst) == 0)
    );
    let source = fixture.chain.state().view();
    let applied = (8, fixture.context.parent_hash);
    let own = fixture.producers[0]
        .drive(&source, &fixture.context, applied)
        .unwrap();
    assert!(
        own.is_some(),
        "mandatory control work must make progress even without user transactions"
    );
    assert_eq!(fixture.counts[0].load(Ordering::SeqCst), 1);
    assert!(matches!(
        fixture.producers[0].build(&build_context(&fixture.context, 0)),
        Err(NativeBeaconError::AwaitingShares { height: 9 })
    ));
    drop(source);
    fixture.chain.setup_world_at(2_000, |transaction| {
        transaction
            .world
            .global_beacon_active_session
            .remove(GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY);
    });
    let source = fixture.chain.state().view();
    for producer in &mut fixture.producers[1..] {
        assert!(matches!(
            producer.drive(&source, &fixture.context, applied),
            Err(NativeBeaconError::Source(_))
        ));
        assert!(producer.active.is_none());
        assert!(producer.build(&build_context(&fixture.context, 0)).is_err());
    }
    assert!(
        fixture.counts[1..]
            .iter()
            .all(|count| count.load(Ordering::SeqCst) == 0)
    );
}

#[test]
fn native_active_session_from_a_foreign_real_committee_refuses_before_signing() {
    let mut fixture = fixture();
    let mut pairs = (0xD1..=0xD4)
        .map(|seed| KeyPair::from_seed(vec![seed; 32], Algorithm::BlsNormal))
        .collect::<Vec<_>>();
    pairs.sort_by(|a, b| a.public_key().cmp(b.public_key()));
    let peers = pairs
        .iter()
        .map(|pair| iroha_model_base::peer::PeerId::new(pair.public_key().clone()))
        .collect::<Vec<_>>();
    let id: [u8; 32] = Hash::new(b"foreign native beacon roster").into();
    let generation = fixture
        .chain
        .state()
        .view()
        .world()
        .consensus_schedule()
        .ready(9)
        .unwrap()
        .epoch
        .authority
        .generation;
    let (session, _) = prepared_session_and_signers_fixture_for_keys_v1(
        GlobalThresholdBeaconDkgSessionV1 {
            version: GLOBAL_THRESHOLD_BEACON_VERSION_V1,
            network_id: fixture.chain.network_id(),
            session_id: id,
            attempt_id: id,
            authority_generation: generation,
            roster_hash: crate::beacon::global_threshold_beacon_roster_hash_v1(&peers),
            committee_size: 4,
            threshold: 2,
            start_height: 1,
            commitments_end_height: 2,
            deliveries_end_height: 3,
            acceptances_end_height: 4,
        },
        &pairs,
    );
    let mut record =
        FinalizedGlobalThresholdBeaconKeySessionRecordV1::new(session.record().clone()).unwrap();
    record
        .activate(session.record().adaptive_dkg.finalized_at_height)
        .unwrap();
    fixture.chain.setup_world_at(2_000, |transaction| {
        transaction
            .world
            .global_beacon_key_sessions
            .insert(id, record);
        transaction
            .world
            .global_beacon_active_session
            .insert(GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY, id);
    });
    let source = fixture.chain.state().view();
    let applied = (8, fixture.context.parent_hash);
    for producer in &mut fixture.producers {
        assert!(matches!(
            producer.drive(&source, &fixture.context, applied),
            Err(NativeBeaconError::Source(_))
        ));
        assert!(producer.active.is_none());
        assert!(producer.build(&build_context(&fixture.context, 0)).is_err());
    }
    assert!(
        fixture
            .counts
            .iter()
            .all(|count| count.load(Ordering::SeqCst) == 0)
    );
}

#[test]
fn readiness_reprobes_same_applied_cut_without_signing_and_excludes_stale_generation() {
    let mut fixture = fixture();
    let state = fixture.chain.state();
    let generation = state.state_view_generation();
    let budget = state.ivm_execution_budget();
    let reporting = fixture.producers[0].attach_readiness(&budget).unwrap();
    assert!(reporting.read(generation, 9, 8).is_none());
    let view = state.view();
    fixture.producers[0]
        .refresh_readiness(
            &view,
            &fixture.context,
            (8, fixture.context.parent_hash),
            generation,
        )
        .unwrap();
    let (horizon, ready) = reporting.read(generation, 9, 8).unwrap();
    assert!(ready && horizon.local_provider_ready && horizon.session_covers_next_pulse);
    assert_eq!(horizon.next_required_pulse_height, Some(9));
    assert_eq!(
        fixture.counts[0].load(Ordering::SeqCst),
        0,
        "readiness never signs a partial"
    );
    let signer = fixture.producers[0].signer.take();
    fixture.producers[0]
        .refresh_readiness(
            &view,
            &fixture.context,
            (8, fixture.context.parent_hash),
            generation,
        )
        .unwrap();
    let (unavailable, ready) = reporting.read(generation, 9, 8).unwrap();
    assert!(!ready && !unavailable.local_provider_ready);
    assert_eq!(unavailable.active_session_id, horizon.active_session_id);
    fixture.producers[0].signer = signer;
    fixture.producers[0]
        .refresh_readiness(
            &view,
            &fixture.context,
            (8, fixture.context.parent_hash),
            generation,
        )
        .unwrap();
    assert!(reporting.read(generation, 9, 8).unwrap().1);
    assert!(reporting.read(generation + 2, 9, 8).is_none());
    assert!(reporting.read(generation, 10, 8).is_none());
    assert!(reporting.read(generation, 9, 7).is_none());
    let mut observer = NativeBeaconProducer::new(fixture.chain.instance(), None, None);
    let report = observer.attach_readiness(&budget).unwrap();
    observer
        .refresh_readiness(
            &view,
            &fixture.context,
            (8, fixture.context.parent_hash),
            generation,
        )
        .unwrap();
    let (horizon, ready) = report.read(generation, 9, 8).unwrap();
    assert!(ready && !horizon.local_provider_ready);
    assert_eq!(
        fixture.chain.height(),
        8,
        "readiness recovery requires no extra block"
    );
}

#[test]
fn readiness_no_demand_does_not_require_a_beacon_session() {
    let chain = prefix();
    let context = context(&chain);
    let state = chain.state();
    let generation = state.state_view_generation();
    let key: [u8; 48] = chain.validators()[0]
        .0
        .public_key()
        .try_to_bytes()
        .unwrap()
        .1
        .try_into()
        .unwrap();
    let mut producer = NativeBeaconProducer::new(chain.instance(), Some(key), None);
    let report = producer
        .attach_readiness(&state.ivm_execution_budget())
        .unwrap();
    producer
        .refresh_readiness(
            &state.view(),
            &context,
            (8, context.parent_hash),
            generation,
        )
        .unwrap();
    let (horizon, ready) = report.read(generation, 9, 8).unwrap();
    assert!(ready);
    assert_eq!(horizon.next_required_pulse_height, None);
    assert_eq!(horizon.active_session_id, None);
    assert!(!horizon.local_provider_ready);
}

#[path = "execution_tests.rs"]
mod execution_tests;
