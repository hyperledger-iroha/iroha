//! Beacon protocol, DKG, and Parliament regression tests.

use super::{
    fixtures::{
        AdaptiveBeaconFixture, adaptive_beacon_fixture, adaptive_beacon_fixture_for_session,
        adaptive_dkg_session_fixture, beacon_fixture_network_id,
    },
    *,
};
use crate::{
    governance::parliament::{
        ParliamentAttemptStateV1, ParliamentDecisionModeV1, RequiredParliamentBodyV1,
    },
    kura::Kura,
    query::store::LiveQueryStore,
    state::{
        GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY, MusubiResolverIndexRevisionV1, State, World,
        WorldReadOnly as _,
    },
    sumeragi::v2_beacon::{
        V2GlobalBeaconError, V2GlobalBeaconIngressOutcome, V2GlobalBeaconLifecycle,
    },
    sumeragi::v2_context::{
        V2ContextBuildError, finalized_global_beacon_npos_successor_seed_from_sources,
    },
};
use iroha_config::parameters::actual::{Governance, LaneConfig as RuntimeLaneConfig};
use iroha_crypto::{
    Algorithm, HashOf, KeyPair,
    threshold_bls::{AdaptiveThresholdBlsSecretShare, TleReleasePurpose},
};
use iroha_data_model::{
    account::AccountId,
    block::{BlockHeader, consensus_v2 as wire},
    consensus::{
        GlobalThresholdBeaconDkgComplaintReasonV1, GlobalThresholdBeaconDkgConstantProofV1,
        GlobalThresholdBeaconPublicShareV1, NposConsensusEffects,
    },
    governance::types::{
        BeaconPulseId, BeaconSessionId, BodyElectionAttemptId, GovernanceAttemptId,
        GovernanceAttemptStatusV1, GovernanceAttemptV1, GovernanceExpectedHeadAbsentV1,
        GovernanceExpectedHeadV1, GovernanceStageV1, MIN_PARLIAMENT_HIDDEN_BALLOT_ANONYMITY_V1,
        ParliamentBody, ProposalContentId, RiskTierV1, SortitionRequestId, SortitionRequestV1,
        parliament_candidate_root_v1,
    },
    musubi::MusubiRegistrySnapshotV1,
};
use iroha_model_base::chain::ChainId;
use iroha_model_base::peer::PeerId;
use rand::rngs::StdRng;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

struct AcceptingAdaptiveDkgCrypto;

#[test]
fn active_global_beacon_session_projection_rejects_noncanonical_storage() {
    let world = World::new();
    assert_eq!(
        active_global_threshold_beacon_session_id_v1(&world.view()),
        Ok(None)
    );

    let canonical = [0x31; 32];
    {
        let mut block = world.block();
        block
            .global_beacon_active_session
            .insert(GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY, canonical);
        block.commit();
    }
    assert_eq!(
        active_global_threshold_beacon_session_id_v1(&world.view()),
        Ok(Some(canonical))
    );

    {
        let mut block = world.block();
        block
            .global_beacon_active_session
            .remove(GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY);
        block.global_beacon_active_session.insert(1, [0x41; 32]);
        block.commit();
    }
    assert_eq!(
        active_global_threshold_beacon_session_id_v1(&world.view()),
        Err(GlobalThresholdBeaconError::PersistenceConflict)
    );

    {
        let mut block = world.block();
        block
            .global_beacon_active_session
            .insert(GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY, canonical);
        block.commit();
    }
    assert_eq!(
        active_global_threshold_beacon_session_id_v1(&world.view()),
        Err(GlobalThresholdBeaconError::PersistenceConflict)
    );
}

impl GlobalThresholdBeaconDkgCryptoV1 for AcceptingAdaptiveDkgCrypto {
    fn derive_generators(
        &self,
        session: &GlobalThresholdBeaconDkgSessionV1,
    ) -> Result<([u8; 96], [u8; 96]), ThresholdBlsError> {
        let parameters = adaptive_beacon_parameters(session)?;
        Ok((*parameters.h_bytes(), *parameters.v_bytes()))
    }

    fn verify_dealer_commitment(
        &self,
        _session: &GlobalThresholdBeaconDkgSessionV1,
        _generator_h: &[u8; 96],
        _generator_v: &[u8; 96],
        _commitment: &GlobalThresholdBeaconDkgDealerCommitmentV1,
    ) -> Result<(), ThresholdBlsError> {
        Ok(())
    }

    fn verify_complaint_response(
        &self,
        _session: &GlobalThresholdBeaconDkgSessionV1,
        _generator_h: &[u8; 96],
        _generator_v: &[u8; 96],
        _commitment: &GlobalThresholdBeaconDkgDealerCommitmentV1,
        _response: &GlobalThresholdBeaconDkgComplaintResponseV1,
    ) -> Result<(), ThresholdBlsError> {
        Ok(())
    }

    fn finalize_qualified_dealers(
        &self,
        session: &GlobalThresholdBeaconDkgSessionV1,
        _generator_h: &[u8; 96],
        _generator_v: &[u8; 96],
        _dealer_commitments: &[GlobalThresholdBeaconDkgDealerCommitmentV1],
        _qualified_dealers: &[u16],
        event_hash: [u8; 32],
    ) -> Result<GlobalThresholdBeaconDkgDerivedPublicV1, ThresholdBlsError> {
        Ok(GlobalThresholdBeaconDkgDerivedPublicV1 {
            group_public_key: g2_generator(),
            public_shares: (1..=session.committee_size)
                .map(|index| GlobalThresholdBeaconPublicShareV1 {
                    index,
                    participant_seat_binding: [index as u8 + 0x40; 32],
                    public_key_share: g2_generator(),
                })
                .collect(),
            transcript_hash: event_hash,
        })
    }
}

fn adaptive_dkg_dealer_fixture(dealer_index: u16) -> GlobalThresholdBeaconDkgDealerCommitmentV1 {
    GlobalThresholdBeaconDkgDealerCommitmentV1 {
        dealer_index,
        coefficient_commitments: vec![g2_generator(), neg_g2_generator()],
        constant_term_proof: GlobalThresholdBeaconDkgConstantProofV1 {
            commitment: g2_generator(),
            response: [dealer_index as u8 + 0x30; 32],
        },
    }
}

#[test]
fn adaptive_dkg_reducer_derives_qualification_after_complaint_resolution() {
    let crypto = AcceptingAdaptiveDkgCrypto;
    let session = adaptive_dkg_session_fixture();
    let mut state =
        GlobalThresholdBeaconDkgStateV1::new(session, &crypto).expect("valid adaptive DKG state");
    for dealer_index in 1..=4 {
        state
            .record_dealer_commitment(1, adaptive_dkg_dealer_fixture(dealer_index), &crypto)
            .expect("dealer commitment");
    }
    let dealer = state
        .dealer_commitments
        .get(&1)
        .expect("dealer one commitment");
    let dealer_commitment_hash =
        global_threshold_beacon_dkg_dealer_commitment_hash_v1(&session, dealer);
    let reason = GlobalThresholdBeaconDkgComplaintReasonV1::InvalidPrivateShare;
    let complaint_id =
        global_threshold_beacon_dkg_complaint_id_v1(&session, 1, 2, dealer_commitment_hash, reason);
    state
        .record_complaint(
            10,
            GlobalThresholdBeaconDkgComplaintV1 {
                dealer_index: 1,
                complainant_index: 2,
                dealer_commitment_hash,
                reason,
                complaint_id,
            },
        )
        .expect("complaint");
    state
        .record_complaint_response(
            20,
            GlobalThresholdBeaconDkgComplaintResponseV1 {
                complaint_id,
                dealer_index: 1,
                recipient_index: 2,
                s_share: [1; 32],
                r_share: [2; 32],
                u_share: [3; 32],
            },
            &crypto,
        )
        .expect("valid public response");
    let finalized = state.finalize(30, &crypto).expect("finalized DKG");
    assert_eq!(finalized.adaptive_dkg.qualified_dealers, vec![1, 2, 3, 4]);
    assert_eq!(
        state.phase_at(30),
        GlobalThresholdBeaconDkgPhaseV1::Finalized
    );
}

#[test]
fn adaptive_dkg_reducer_rejects_phase_errors_equivocation_and_too_small_q() {
    let crypto = AcceptingAdaptiveDkgCrypto;
    let session = adaptive_dkg_session_fixture();
    let mut state =
        GlobalThresholdBeaconDkgStateV1::new(session, &crypto).expect("valid adaptive DKG state");
    assert_eq!(
        state.record_dealer_commitment(10, adaptive_dkg_dealer_fixture(1), &crypto),
        Err(GlobalThresholdBeaconError::WrongDkgPhase)
    );
    for dealer_index in 1..=4 {
        state
            .record_dealer_commitment(1, adaptive_dkg_dealer_fixture(dealer_index), &crypto)
            .expect("dealer commitment");
    }
    let mut equivocation = adaptive_dkg_dealer_fixture(1);
    equivocation.coefficient_commitments[0][0] ^= 1;
    assert_eq!(
        state.record_dealer_commitment(2, equivocation, &crypto),
        Err(GlobalThresholdBeaconError::DealerCommitmentEquivocation)
    );
    for (dealer_index, complainant_index) in [(1, 3), (2, 4)] {
        let dealer = state
            .dealer_commitments
            .get(&dealer_index)
            .expect("dealer commitment");
        let dealer_commitment_hash =
            global_threshold_beacon_dkg_dealer_commitment_hash_v1(&session, dealer);
        let reason = GlobalThresholdBeaconDkgComplaintReasonV1::MissingPrivateShare;
        state
            .record_complaint(
                10,
                GlobalThresholdBeaconDkgComplaintV1 {
                    dealer_index,
                    complainant_index,
                    dealer_commitment_hash,
                    reason,
                    complaint_id: global_threshold_beacon_dkg_complaint_id_v1(
                        &session,
                        dealer_index,
                        complainant_index,
                        dealer_commitment_hash,
                        reason,
                    ),
                },
            )
            .expect("unresolved complaint");
    }
    assert_eq!(
        state.finalize(30, &crypto),
        Err(GlobalThresholdBeaconError::InsufficientQualifiedDealers)
    );
    assert_eq!(state.phase_at(31), GlobalThresholdBeaconDkgPhaseV1::Aborted);
}

#[test]
fn adaptive_dkg_public_snapshot_roundtrips_and_restores() {
    let crypto = AcceptingAdaptiveDkgCrypto;
    let session = adaptive_dkg_session_fixture();
    let mut state =
        GlobalThresholdBeaconDkgStateV1::new(session, &crypto).expect("valid adaptive DKG state");
    for dealer_index in 1..=4 {
        state
            .record_dealer_commitment(1, adaptive_dkg_dealer_fixture(dealer_index), &crypto)
            .expect("dealer commitment");
    }
    let snapshot = state.public_snapshot().expect("public snapshot");
    let bytes = norito::to_bytes(&snapshot).expect("encode public DKG snapshot");
    let binary: GlobalThresholdBeaconDkgSnapshotV1 =
        norito::decode_from_bytes(&bytes).expect("decode public DKG snapshot");
    binary.validate().expect("validate decoded DKG snapshot");
    crate::private_settlement::global_state::tests::assert_private_settlement_frame_v1(
        &snapshot,
        "iroha_core::beacon::GlobalThresholdBeaconDkgSnapshotV1",
    );
    assert!(matches!(
        norito::decode_canonical::<GlobalThresholdBeaconKeySessionV1>(&bytes),
        Err(norito::Error::SchemaMismatch),
    ));
    assert_eq!(binary, snapshot);
    let json = norito::json::to_json(&snapshot).expect("encode public DKG snapshot JSON");
    let decoded_json: GlobalThresholdBeaconDkgSnapshotV1 =
        norito::json::from_str(&json).expect("decode public DKG snapshot JSON");
    assert_eq!(decoded_json, snapshot);
    let restored = GlobalThresholdBeaconDkgStateV1::from_snapshot(binary, &crypto)
        .expect("cryptographically restore public snapshot");
    assert_eq!(
        restored.public_snapshot().expect("restored snapshot"),
        snapshot
    );
}

#[test]
fn finalized_key_lifecycle_is_strict_and_roundtrips() {
    let (validated, _) = validated_threshold_session();
    let mut record =
        FinalizedGlobalThresholdBeaconKeySessionRecordV1::new(validated.record().clone())
            .expect("valid finalized public key record");
    let finalized_height = record.session.adaptive_dkg.finalized_at_height;
    assert_eq!(
        record.activate(finalized_height - 1),
        Err(GlobalThresholdBeaconError::InvalidKeyLifecycle)
    );
    record
        .activate(finalized_height)
        .expect("activate at finalization height");
    assert!(record.is_active_at(finalized_height));
    assert_eq!(
        record.retire(finalized_height),
        Err(GlobalThresholdBeaconError::InvalidKeyLifecycle)
    );
    record
        .retire(finalized_height + 1)
        .expect("strictly later retirement");
    assert!(!record.is_active_at(finalized_height + 1));
    let encoded = norito::to_bytes(&record).expect("encode key lifecycle");
    let decoded: FinalizedGlobalThresholdBeaconKeySessionRecordV1 =
        norito::decode_from_bytes(&encoded).expect("decode key lifecycle");
    decoded.validate().expect("validate decoded lifecycle");
    crate::private_settlement::global_state::tests::assert_private_settlement_frame_v1(
        &record,
        "iroha_core::beacon::FinalizedGlobalThresholdBeaconKeySessionRecordV1",
    );
    assert!(matches!(
        norito::decode_canonical::<GlobalThresholdBeaconKeySessionV1>(&encoded),
        Err(norito::Error::SchemaMismatch),
    ));
    assert_eq!(decoded, record);
}

fn decode_fixed<const N: usize>(encoded: &str) -> [u8; N] {
    let compact = encoded
        .bytes()
        .filter(u8::is_ascii_hexdigit)
        .collect::<Vec<_>>();
    hex::decode(compact)
        .expect("hex test vector")
        .try_into()
        .unwrap_or_else(|bytes: Vec<u8>| panic!("expected {N} bytes, got {}", bytes.len()))
}

fn g2_generator() -> [u8; 96] {
    decode_fixed(
        "93e02b6052719f607dacd3a088274f65596bd0d09920b61ab5da61bbdc7f5049\
             334cf11213945d57e5ac7d055d042b7e024aa2b2f08f0a91260805272dc51051\
             c6e47ad4fa403b02b4510b647ae3d1770bac0326a805bbefd48056c8c121bdb8",
    )
}

fn neg_g2_generator() -> [u8; 96] {
    let mut encoded = g2_generator();
    encoded[0] ^= 0x20;
    encoded
}

fn wrong_g1_signature() -> [u8; 48] {
    decode_fixed(
        "97f1d3a73197d7942695638c4fa9ac0f\
             c3688c4f9774b905a14e3a3f171bac58\
             6c55e83ff97a1aeffb3af00adb22c6bb",
    )
}

pub(crate) fn finalized_key_session_fixture_for_context_v1(
    network_id: NetworkId,
    session_id: [u8; 32],
    roster_hash: [u8; 32],
) -> FinalizedGlobalThresholdBeaconKeySessionRecordV1 {
    let mut dkg_session = adaptive_dkg_session_fixture();
    dkg_session.network_id = network_id;
    dkg_session.session_id = session_id;
    dkg_session.roster_hash = roster_hash;
    let fixture = adaptive_beacon_fixture_for_session(dkg_session);
    FinalizedGlobalThresholdBeaconKeySessionRecordV1::new(fixture.session.record().clone())
        .expect("proof-valid finalized global beacon key fixture")
}

fn validated_threshold_session() -> (
    ValidatedGlobalThresholdBeaconSessionV1,
    GlobalThresholdBeaconSessionBindingV1,
) {
    let fixture = adaptive_beacon_fixture();
    let validated = fixture.session;
    let expected = fixture.binding;
    (validated, expected)
}

fn pulse_fixture(
    session: &ValidatedGlobalThresholdBeaconSessionV1,
) -> (
    FinalizedGlobalThresholdBeaconPulseV1,
    GlobalThresholdBeaconPulseLinkV1,
    GlobalThresholdBeaconChainAnchorV1,
) {
    let cursor = GlobalThresholdBeaconPulseLinkV1 {
        pulse_id: [0x66; 32],
        seed: [0x77; 32],
        height: 0,
        round: 0,
    };
    let anchor = GlobalThresholdBeaconChainAnchorV1 {
        height: 40,
        block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x88; 32])),
    };
    let record = session.record();
    (
        FinalizedGlobalThresholdBeaconPulseV1 {
            version: GLOBAL_THRESHOLD_BEACON_VERSION_V1,
            network_id: record.network_id,
            session_id: record.session_id,
            roster_hash: record.roster_hash,
            transcript_hash: record.transcript_hash,
            height: 41,
            round: 0,
            finalized_chain_anchor: anchor,
            signature: wrong_g1_signature(),
            seed: [0x99; 32],
            pulse_id: [0xAA; 32],
        },
        cursor,
        anchor,
    )
}

fn signed_pulse_fixture() -> (
    AdaptiveBeaconFixture,
    FinalizedGlobalThresholdBeaconPulseV1,
    GlobalThresholdBeaconPulseLinkV1,
    GlobalThresholdBeaconChainAnchorV1,
) {
    let fixture = adaptive_beacon_fixture();
    let (pulse, cursor, anchor) = pulse_fixture(&fixture.session);
    let partials = pulse_partial_signatures(&fixture, &pulse, [0xA7; 32]);
    let mut aggregator =
        GlobalThresholdBeaconPulseAggregatorV1::new(fixture.session.clone(), pulse.height, anchor)
            .expect("open exact pulse reducer");
    for partial in partials.into_iter().take(usize::from(
        fixture.session.transcript.session().threshold(),
    )) {
        aggregator
            .accept_partial(partial)
            .expect("accept proof-verified partial");
    }
    let pulse = aggregator.finalize().expect("finalize unique pulse");
    (fixture, pulse, cursor, anchor)
}

fn pulse_partial_signatures(
    fixture: &AdaptiveBeaconFixture,
    pulse: &FinalizedGlobalThresholdBeaconPulseV1,
    rng_seed: [u8; 32],
) -> Vec<GlobalThresholdBeaconPartialSignatureV1> {
    let payload = global_threshold_beacon_pulse_payload_v1(&pulse);
    let mut proof_rng = StdRng::from_seed(rng_seed);
    let mut partials = Vec::new();

    for recipient_index in 1_u16..=fixture.session.transcript.session().committee_size() {
        let private_contributions = fixture
            .dealer_secrets
            .iter()
            .zip(&fixture.dealer_commitments)
            .map(|(secret, dealer)| {
                secret
                    .private_share(&fixture.parameters, dealer, recipient_index)
                    .expect("verified private DKG contribution")
            })
            .collect::<Vec<_>>();
        let signing_share = AdaptiveThresholdBlsSecretShare::from_dealer_shares(
            &fixture.session.transcript,
            &private_contributions,
        )
        .expect("aggregate exact qualified private contributions");
        partials.push(global_threshold_beacon_partial_signature_dto_v1(
            &signing_share
                .sign_payload_with_rng(&fixture.session.transcript, &payload, &mut proof_rng)
                .expect("adaptive signature share"),
        ));
    }
    partials
}

fn live_fixture_in_memory_signer(
    fixture: &AdaptiveBeaconFixture,
    recipient_index: u16,
) -> InMemoryGlobalThresholdBeaconPartialSignerV1 {
    let private_contributions = fixture
        .dealer_secrets
        .iter()
        .zip(&fixture.dealer_commitments)
        .map(|(secret, dealer)| {
            secret
                .private_share(&fixture.parameters, dealer, recipient_index)
                .expect("verified private DKG contribution")
        })
        .collect::<Vec<_>>();
    let share = AdaptiveThresholdBlsSecretShare::from_dealer_shares(
        &fixture.session.transcript,
        &private_contributions,
    )
    .expect("aggregate exact qualified private contributions");
    InMemoryGlobalThresholdBeaconPartialSignerV1::from_validated_share(
        fixture.session.clone(),
        share,
    )
    .expect("move the DKG share into the zeroizing runtime provider")
}

fn live_fixture_signer(
    fixture: &AdaptiveBeaconFixture,
    recipient_index: u16,
) -> Arc<dyn GlobalThresholdBeaconPartialSignerV1> {
    Arc::new(live_fixture_in_memory_signer(fixture, recipient_index))
}

struct FailOnceBeaconSigner {
    inner: Arc<dyn GlobalThresholdBeaconPartialSignerV1>,
    attempts: Arc<AtomicUsize>,
}

impl GlobalThresholdBeaconPartialSignerV1 for FailOnceBeaconSigner {
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
        if self.attempts.fetch_add(1, Ordering::SeqCst) == 0 {
            return Err("transient signer failure".to_owned());
        }
        self.inner.sign_partial(session, payload)
    }
}

#[cfg(feature = "test-network-parliament-signers")]
struct InvalidOutboundTestBeaconSigner {
    inner: Arc<dyn GlobalThresholdBeaconPartialSignerV1>,
}

#[cfg(feature = "test-network-parliament-signers")]
impl GlobalThresholdBeaconPartialSignerV1 for InvalidOutboundTestBeaconSigner {
    fn attest_partial_signing_capability(
        &self,
        session: &ValidatedGlobalThresholdBeaconSessionV1,
        expected_signer_index: u16,
    ) -> Result<
        GlobalThresholdBeaconPartialSigningCapabilityV1,
        GlobalThresholdBeaconCapabilityErrorV1,
    > {
        let _ = (session, expected_signer_index);
        Err(GlobalThresholdBeaconCapabilityErrorV1::NotOwned)
    }

    fn sign_partial(
        &self,
        session: &ValidatedGlobalThresholdBeaconSessionV1,
        payload: &[u8],
    ) -> Result<GlobalThresholdBeaconPartialSignatureV1, String> {
        self.inner.sign_partial(session, payload)
    }

    fn test_network_emit_invalid_outbound_partial_v1(&self) -> bool {
        true
    }
}

#[test]
fn runtime_beacon_capability_requires_exact_live_session_and_seat_without_signing() {
    let fixture = adaptive_beacon_fixture();
    let custody = RuntimeGlobalThresholdBeaconShareCustodyV1::new();
    assert_eq!(
        custody.attest_partial_signing_capability(&fixture.session, 1),
        Err(GlobalThresholdBeaconCapabilityErrorV1::NotOwned)
    );
    for index in [0, 5] {
        assert_eq!(
            custody.attest_partial_signing_capability(&fixture.session, index),
            Err(GlobalThresholdBeaconCapabilityErrorV1::InvalidRequest)
        );
    }
    custody
        .insert_validated_share(live_fixture_in_memory_signer(&fixture, 1))
        .expect("own seat one");
    let attempts = Arc::new(AtomicUsize::new(0));
    let provider = FailOnceBeaconSigner {
        inner: Arc::new(custody),
        attempts: Arc::clone(&attempts),
    };
    let capability = provider
        .attest_partial_signing_capability(&fixture.session, 1)
        .expect("exact live custody");
    assert!(capability.matches(&fixture.session, 1));
    assert_eq!(capability.session_id(), fixture.session.record().session_id);
    assert_eq!(
        capability.transcript_hash(),
        fixture.session.record().transcript_hash
    );
    assert_eq!(capability.signer_index(), 1);
    assert!(!capability.matches(&fixture.session, 2));
    assert_eq!(
        provider.attest_partial_signing_capability(&fixture.session, 2),
        Err(GlobalThresholdBeaconCapabilityErrorV1::NotOwned)
    );
    let mut other = adaptive_dkg_session_fixture();
    other.session_id = [0xC7; 32];
    let other = adaptive_beacon_fixture_for_session(other);
    assert_eq!(
        provider.attest_partial_signing_capability(&other.session, 1),
        Err(GlobalThresholdBeaconCapabilityErrorV1::NotOwned)
    );
    assert!(!capability.matches(&other.session, 1));
    assert_eq!(
        attempts.load(Ordering::SeqCst),
        0,
        "capability must never call sign_partial"
    );
}

#[test]
fn runtime_beacon_custody_selects_exact_rotating_session_and_rejects_replacement() {
    let fixture_a = adaptive_beacon_fixture();
    let mut session_b = adaptive_dkg_session_fixture();
    session_b.session_id = [0xB2; 32];
    let fixture_b = adaptive_beacon_fixture_for_session(session_b);
    let custody = RuntimeGlobalThresholdBeaconShareCustodyV1::new();
    custody
        .insert_validated_share(live_fixture_in_memory_signer(&fixture_a, 1))
        .expect("insert key-A share");
    custody
        .insert_validated_share(live_fixture_in_memory_signer(&fixture_b, 1))
        .expect("insert key-B share before key-A retirement");
    assert_eq!(
        custody.insert_validated_share(live_fixture_in_memory_signer(&fixture_a, 2)),
        Err(GlobalThresholdBeaconShareCustodyErrorV1::SessionAlreadyPresent),
    );

    for (fixture, height, anchor_byte) in [(&fixture_a, 41, 0xA1), (&fixture_b, 42, 0xB1)] {
        let anchor = GlobalThresholdBeaconChainAnchorV1 {
            height: height - 1,
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed(
                [anchor_byte; 32],
            )),
        };
        let mut aggregator =
            GlobalThresholdBeaconPulseAggregatorV1::new(fixture.session.clone(), height, anchor)
                .expect("open rotating-session pulse");
        let partial = custody
            .sign_partial(&fixture.session, aggregator.payload())
            .expect("select exact session share");
        assert!(
            aggregator
                .accept_partial(partial)
                .expect("independently verify custody output")
        );
    }
}

fn live_producer_keys() -> Vec<KeyPair> {
    let mut keys = (1_u8..=4)
        .map(|marker| {
            KeyPair::try_from_seed(vec![marker; 32], Algorithm::BlsNormal)
                .expect("deterministic BLS validator key")
        })
        .collect::<Vec<_>>();
    keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
    keys
}

pub(crate) fn pending_batched_sortition_attempt(
    network_id: &NetworkId,
    roster: &[PeerId],
    pulse_height: u64,
) -> (
    GovernanceAttemptId,
    Vec<SortitionRequestId>,
    ParliamentAttemptStateV1,
) {
    let proposal_content_id = ProposalContentId::new([0x41; 32]);
    let governance_attempt_id = GovernanceAttemptId::derive_v1(proposal_content_id, 0);
    let mut attempt = ParliamentAttemptStateV1::try_new(
        GovernanceAttemptV1 {
            id: governance_attempt_id,
            proposal_content_id,
            sequence: 0,
            risk_tier: RiskTierV1::Standard,
            stage: GovernanceStageV1::Qualification,
            status: GovernanceAttemptStatusV1::Active,
        },
        1,
        10,
        [0x42; 32],
        GovernanceExpectedHeadV1::Absent(GovernanceExpectedHeadAbsentV1 {
            subject_id: [0x43; 32],
        }),
        vec![
            RequiredParliamentBodyV1 {
                body: ParliamentBody::RulesCommittee,
                decision_mode: ParliamentDecisionModeV1::PublicFinding,
            },
            RequiredParliamentBodyV1 {
                body: ParliamentBody::PolicyJury,
                decision_mode: ParliamentDecisionModeV1::HiddenBindingBallot,
            },
        ],
    )
    .expect("construct pending Parliament attempt");
    attempt
        .complete_qualification(governance_attempt_id)
        .expect("enter the first required body stage");
    let mut candidates = roster
        .iter()
        .map(|peer| AccountId::new(peer.public_key().clone()))
        .collect::<Vec<_>>();
    candidates.sort_unstable();
    let mut request_ids = Vec::new();
    for body in [ParliamentBody::RulesCommittee, ParliamentBody::PolicyJury] {
        let election_attempt_id = BodyElectionAttemptId::derive_v1(governance_attempt_id, body, 0);
        let candidate_root = parliament_candidate_root_v1(governance_attempt_id, body, &candidates);
        let request = SortitionRequestV1::try_new_canonical(
            governance_attempt_id,
            election_attempt_id,
            body,
            candidate_root,
            u32::try_from(candidates.len()).expect("four candidates"),
            if body == ParliamentBody::PolicyJury {
                MIN_PARLIAMENT_HIDDEN_BALLOT_ANONYMITY_V1
            } else {
                2
            },
            pulse_height - 10,
            pulse_height,
            BeaconSessionId::for_network_v1(network_id),
            None,
        )
        .expect("construct batched logical-beacon sortition request");
        request_ids.push(request.id);
        attempt
            .register_sortition_request(governance_attempt_id, 0, request, candidates.clone())
            .expect("register pending logical-beacon request");
    }
    request_ids.sort_unstable();
    (governance_attempt_id, request_ids, attempt)
}

pub(super) fn live_producer_context(
    keys: &[KeyPair],
    network_id: NetworkId,
    parent_hash: HashOf<BlockHeader>,
    epoch_end_height: u64,
) -> wire::HeightContext {
    let roster = keys
        .iter()
        .map(|key| wire::ValidatorPower {
            validator: PeerId::new(key.public_key().clone()),
            power: 1,
        })
        .collect::<Vec<_>>();
    let parent_round = wire::ConsensusRound {
        context_id: wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
            b"threshold beacon fixture parent context",
        ))),
        height: 40,
        view: 0,
    };
    // This fixture enters epoch seven immediately after boundary block 40.
    // Retain the real generation-zero keys and installed beacon through that
    // exact predecessor, then derive the contiguous authorization once.
    let (previous_authorization, kagemusha_mint_finality_authority) =
        crate::kagemusha_v1_test_fixtures::mint_finality_retained_authorization(
            network_id,
            6,
            parent_round.height,
            &roster,
        );
    assert!(matches!(
        previous_authorization.beacon,
        iroha_data_model::isi::kagemusha_v1::BeaconEpochBindingV1::Installed(_)
    ));
    let kagemusha_mint_finality_authorization =
        crate::kagemusha_v1_test_fixtures::mint_finality_successor_authorization(
            &previous_authorization,
            &kagemusha_mint_finality_authority,
            epoch_end_height,
            previous_authorization.beacon,
            iroha_data_model::isi::kagemusha_v1::KagemushaMintFinalityEpochDecisionV1::Retain,
            [0; 32],
        );
    assert_eq!(kagemusha_mint_finality_authorization.epoch, 7);
    assert_eq!(kagemusha_mint_finality_authorization.first_height, 41);
    assert_eq!(
        kagemusha_mint_finality_authorization.last_height,
        epoch_end_height
    );
    let context = wire::HeightContext {
        network_id,
        protocol_version: wire::PROTOCOL_VERSION,
        height: 41,
        epoch: 7,
        epoch_end_height,
        next_epoch_snapshot: None,
        snapshot_bootstrap: None,
        mode: wire::ConsensusMode::Npos,
        parent_commit_qc: Some(wire::QuorumCertificate {
            round: parent_round,
            proposal_round: parent_round,
            phase: wire::GlobalPhase::Commit,
            subject: wire::BlockSubject {
                parent_block_hash: Some(HashOf::from_untyped_unchecked(Hash::new(
                    b"threshold beacon fixture grandparent",
                ))),
                block_hash: parent_hash,
                payload_hash: Hash::new(b"threshold beacon fixture parent payload"),
            },
            execution_commitment:
                wire::ExecutionCommitment::without_kagemusha_top_ups_or_merge_carrier(
                    Hash::new(b"threshold beacon fixture parent state"),
                    Hash::new(b"threshold beacon fixture post state"),
                    Hash::new(b"threshold beacon fixture ordinary writes"),
                    1,
                    Hash::new(b"threshold beacon fixture executed block"),
                ),
            signers: vec![0, 1, 2],
            aggregate_signature: vec![1],
        }),
        quorum: wire::DualQuorum::from_roster(&roster).expect("four-validator quorum"),
        roster,
        kagemusha_mint_finality_authorization,
        kagemusha_mint_finality_authority,
        nexus_amx_context_hash: Hash::new(b"threshold beacon fixture nexus"),
        execution_policy_hash: Hash::new(b"threshold beacon fixture execution policy"),
        da_layout: wire::DataAvailabilityLayout {
            encoding: wire::PayloadEncoding::ReedSolomon16,
            chunk_size_bytes: 1024,
            data_shards: 3,
            parity_shards: 1,
            max_payload_size_bytes: 4096,
            max_chunk_count: 8,
        },
        leader_seed: [0x91; 32],
    };
    context.validate().expect("valid live beacon context");
    context
}

fn live_producer_state(
    fixture: &AdaptiveBeaconFixture,
    cursor: GlobalThresholdBeaconPulseLinkV1,
    parent_hash: HashOf<BlockHeader>,
) -> State {
    let mut key_record =
        FinalizedGlobalThresholdBeaconKeySessionRecordV1::new(fixture.session.record().clone())
            .expect("valid finalized public key");
    key_record
        .activate(key_record.session.adaptive_dkg.finalized_at_height)
        .expect("activate finalized public key");
    let world = World::new();
    {
        let mut block = world.block();
        block
            .global_beacon_key_sessions
            .insert(key_record.session.session_id, key_record.clone());
        block.global_beacon_active_session.insert(
            GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY,
            key_record.session.session_id,
        );
        block
            .global_beacon_latest_pulse
            .insert(GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY, cursor);
        block.commit();
    }
    let mut state = State::new_with_chain_and_network_id_for_testing(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
        ChainId::from("live-global-threshold-beacon-producer"),
        fixture.session.record().network_id,
    );
    for marker in 1_u8..40 {
        state.push_block_hash_for_testing(HashOf::from_untyped_unchecked(Hash::prehashed(
            [marker; 32],
        )));
    }
    state.push_block_hash_for_testing(parent_hash);
    let snapshot_hashes = state
        .block_hashes
        .view()
        .iter()
        .copied()
        .collect::<Vec<_>>();
    let revision = MusubiResolverIndexRevisionV1::default();
    let checkpoint = MusubiRegistrySnapshotV1 {
        finalized_height: 1,
        finalized_block_hash: *snapshot_hashes
            .first()
            .expect("live-producer history has a genesis hash")
            .as_ref(),
        index_revision: revision.get(),
    };
    checkpoint
        .validate()
        .expect("valid live-producer genesis resolver checkpoint");
    {
        let mut block = state.world.block();
        assert!(
            block
                .musubi_resolver_index_checkpoints
                .insert(revision, checkpoint)
                .is_none(),
            "live-producer fixture must install one genesis resolver checkpoint"
        );
        block.commit();
    }
    state
        .kura()
        .extend_hash_only_suffix_from_verified_snapshot(&snapshot_hashes)
        .expect("install verified live-producer snapshot prefix");
    state
}

fn beacon_partial_payload(message: wire::ConsensusMessageV2) -> wire::GlobalBeaconPartialSignature {
    match message.payload {
        wire::ConsensusMessageV2Payload::GlobalBeaconPartialSignature(partial) => partial,
        other => panic!("expected global beacon partial, got {other:?}"),
    }
}

#[test]
fn threshold_beacon_context_roster_binding_rejects_active_foreign_committee() {
    let keys = live_producer_keys();
    let roster = keys
        .iter()
        .map(|key| PeerId::new(key.public_key().clone()))
        .collect::<Vec<_>>();
    let mut dkg_session = adaptive_dkg_session_fixture();
    dkg_session.roster_hash = global_threshold_beacon_roster_hash_v1(&roster);
    let fixture = adaptive_beacon_fixture_for_session(dkg_session);

    assert_eq!(
        authenticated_global_threshold_beacon_roster_hash_v1(fixture.session.record(), &roster,),
        Ok(fixture.session.record().roster_hash)
    );

    let mut reordered = roster.clone();
    reordered.swap(0, 1);
    assert_eq!(
        authenticated_global_threshold_beacon_roster_hash_v1(fixture.session.record(), &reordered,),
        Err(GlobalThresholdBeaconError::RosterMismatch)
    );
    assert_eq!(
        authenticated_global_threshold_beacon_roster_hash_v1(
            fixture.session.record(),
            &roster[..roster.len() - 1],
        ),
        Err(GlobalThresholdBeaconError::RosterMismatch)
    );
}

#[test]
fn parliament_requested_slot_survives_key_rotation_and_produces_authoritative_pulse() {
    let keys = live_producer_keys();
    let network_id = beacon_fixture_network_id(0xB1);
    let parent_hash = HashOf::from_untyped_unchecked(Hash::prehashed([0xD3; 32]));
    let context = live_producer_context(&keys, network_id, parent_hash, 50);
    context.validate().expect("valid non-boundary context");
    assert_eq!(
        context.height, 41,
        "the fixture is the first height after boundary block 40"
    );
    assert_eq!(
        context.kagemusha_mint_finality_authorization.first_height, context.height,
        "the retained authorization must start at the post-boundary height"
    );
    assert_eq!(context.epoch_end_height, 50);
    let roster = context
        .roster
        .iter()
        .map(|entry| entry.validator.clone())
        .collect::<Vec<_>>();
    let mut predecessor_roster = roster.clone();
    predecessor_roster.rotate_left(1);
    assert_ne!(predecessor_roster, roster);

    let mut dkg_a = adaptive_dkg_session_fixture();
    dkg_a.network_id = network_id;
    dkg_a.session_id = [0xA1; 32];
    dkg_a.roster_hash = global_threshold_beacon_roster_hash_v1(&predecessor_roster);
    let fixture_a = adaptive_beacon_fixture_for_session(dkg_a);
    let cursor = GlobalThresholdBeaconPulseLinkV1 {
        pulse_id: [0x63; 32],
        seed: [0x64; 32],
        height: 0,
        round: 0,
    };
    let state = live_producer_state(&fixture_a, cursor, parent_hash);
    let (governance_attempt_id, request_ids, attempt) =
        pending_batched_sortition_attempt(&network_id, &roster, context.height);
    {
        let mut block = state.world.block();
        {
            let mut transaction = block.transaction_without_telemetry(
                iroha_config::parameters::actual::LaneConfig::default(),
                0,
            );
            transaction
                .put_parliament_attempt(attempt)
                .expect("persist the Parliament request and its beacon-slot index");
            transaction.apply();
        }
        block.commit();
    }
    assert!(matches!(
        V2GlobalBeaconLifecycle::open(&context, &state, Some(0), None),
        Err(V2GlobalBeaconError::RosterMismatch)
    ));

    let mut dkg_b = adaptive_dkg_session_fixture();
    dkg_b.network_id = network_id;
    dkg_b.session_id = [0xB2; 32];
    dkg_b.roster_hash = global_threshold_beacon_roster_hash_v1(&roster);
    let fixture_b = adaptive_beacon_fixture_for_session(dkg_b);
    let mut key_a =
        FinalizedGlobalThresholdBeaconKeySessionRecordV1::new(fixture_a.session.record().clone())
            .expect("valid key A");
    key_a
        .activate(key_a.session.adaptive_dkg.finalized_at_height)
        .expect("activate key A");
    key_a
        .retire(context.height)
        .expect("retire the predecessor at the first successor height");
    let mut key_b =
        FinalizedGlobalThresholdBeaconKeySessionRecordV1::new(fixture_b.session.record().clone())
            .expect("valid key B");
    key_b
        .activate(context.height)
        .expect("activate replacement key B at the first successor height");
    {
        let mut block = state.world.block();
        block
            .global_beacon_key_sessions
            .insert(key_a.session.session_id, key_a);
        block
            .global_beacon_key_sessions
            .insert(key_b.session.session_id, key_b);
        block.global_beacon_active_session.insert(
            GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY,
            fixture_b.session.record().session_id,
        );
        block.commit();
    }

    let signers = (1_u16..=4)
        .map(|index| live_fixture_signer(&fixture_b, index))
        .collect::<Vec<_>>();
    let mut messages = Vec::new();
    for (index, signer) in signers.iter().enumerate() {
        let mut producer = V2GlobalBeaconLifecycle::open(
            &context,
            &state,
            Some(u32::try_from(index).expect("validator index")),
            Some(Arc::clone(signer)),
        )
        .expect("mandatory Parliament producer opens after key rotation");
        assert!(producer.pulse_requested());
        assert!(producer.pulse_required_for_consensus());
        producer.begin_round(0).expect("sign requested slot");
        messages.push(beacon_partial_payload(
            producer.take_outbound().pop().expect("local partial"),
        ));
    }

    let mut reducer = V2GlobalBeaconLifecycle::open(&context, &state, Some(0), None)
        .expect("open signerless validator reducer after key rotation");
    reducer.begin_round(0).expect("open routing view");
    let mut absent = NposConsensusEffects::default();
    assert!(matches!(
        reducer.attach_candidate_effects(0, &mut absent),
        Err(V2GlobalBeaconError::State(_))
    ));
    assert!(absent.finalized_global_beacon_pulse.is_none());
    let mut invalid_optional_share = messages[0].clone();
    invalid_optional_share.partial.signature_share[0] ^= 1;
    assert!(matches!(
        reducer.accept_partial(invalid_optional_share, &roster[0], 0),
        Err(V2GlobalBeaconError::Beacon(
            GlobalThresholdBeaconError::ThresholdBls(_)
        ))
    ));
    let mut still_absent = NposConsensusEffects::default();
    assert!(matches!(
        reducer.attach_candidate_effects(0, &mut still_absent),
        Err(V2GlobalBeaconError::State(_))
    ));
    assert!(still_absent.finalized_global_beacon_pulse.is_none());
    assert_eq!(
        reducer
            .accept_partial(messages[0].clone(), &roster[0], 0)
            .expect("first key-B share"),
        V2GlobalBeaconIngressOutcome::Accepted
    );
    assert_eq!(
        reducer
            .accept_partial(messages[1].clone(), &roster[1], 0)
            .expect("threshold key-B share"),
        V2GlobalBeaconIngressOutcome::Finalized
    );
    let pulse = reducer
        .finalized_pulse(0)
        .expect("requested pulse finalized");
    assert_eq!(pulse.session_id, fixture_b.session.record().session_id);
    let mut effects = NposConsensusEffects::default();
    reducer
        .attach_candidate_effects(0, &mut effects)
        .expect("attach reconstructed requested pulse");
    assert_eq!(effects.finalized_global_beacon_pulse, Some(pulse));

    let expected_anchor = GlobalThresholdBeaconChainAnchorV1 {
        height: 40,
        block_hash: parent_hash,
    };
    {
        let mut block = state.world.block();
        {
            let mut transaction =
                block.transaction_without_telemetry(RuntimeLaneConfig::default(), 0);
            transaction
                .verify_and_advance_global_beacon_pulse(&fixture_b.session, pulse, expected_anchor)
                .expect("persist replacement-key pulse");
            transaction.apply();
        }
        block.commit();
    }
    let mut attempt = state
        .world
        .view()
        .parliament_attempts()
        .get(&governance_attempt_id)
        .cloned()
        .expect("pending Parliament attempt");
    let governance = Governance {
        rules_committee_size: 2,
        policy_jury_size: usize::try_from(MIN_PARLIAMENT_HIDDEN_BALLOT_ANONYMITY_V1)
            .expect("the V1 anonymity floor fits usize"),
        parliament_alternate_size: 2,
        ..Governance::default()
    };
    let pulse_id = BeaconPulseId::new(pulse.pulse_id);
    let pulse_output = global_threshold_beacon_governance_seed_v1(&pulse, pulse.height);
    assert_eq!(
        attempt.consume_sortition_pulse_batch(
            governance_attempt_id,
            request_ids.clone(),
            BeaconSessionId::new([0xEE; 32]),
            pulse.height,
            pulse_id,
            pulse_output,
            &network_id,
            &governance,
        ),
        Err(crate::governance::parliament::ParliamentReducerErrorV1::PulseBindingMismatch)
    );
    attempt
        .consume_sortition_pulse_batch(
            governance_attempt_id,
            request_ids,
            BeaconSessionId::for_network_v1(&network_id),
            pulse.height,
            pulse_id,
            pulse_output,
            &network_id,
            &governance,
        )
        .expect("logical request consumes replacement-key pulse");
}

fn assert_same_block_key_rotation_persists_requested_pulse(parliament_requested_slot: bool) {
    let keys = live_producer_keys();
    let network_id = beacon_fixture_network_id(if parliament_requested_slot {
        0xC1
    } else {
        0xC2
    });
    let parent_hash = HashOf::from_untyped_unchecked(Hash::prehashed([0xD4; 32]));
    let epoch_end_height = if parliament_requested_slot { 50 } else { 42 };
    let context = live_producer_context(&keys, network_id, parent_hash, epoch_end_height);
    if parliament_requested_slot {
        context.validate().expect("valid optional-slot context");
    }
    let roster = context
        .roster
        .iter()
        .map(|entry| entry.validator.clone())
        .collect::<Vec<_>>();

    let mut dkg_a = adaptive_dkg_session_fixture();
    dkg_a.network_id = network_id;
    dkg_a.session_id = [0xA5; 32];
    dkg_a.roster_hash = global_threshold_beacon_roster_hash_v1(&roster);
    let fixture_a = adaptive_beacon_fixture_for_session(dkg_a);
    let cursor = GlobalThresholdBeaconPulseLinkV1 {
        pulse_id: [0x65; 32],
        seed: [0x66; 32],
        height: 0,
        round: 0,
    };
    let mut state = live_producer_state(&fixture_a, cursor, parent_hash);
    let transient_governance_attempt_id = if parliament_requested_slot {
        let (governance_attempt_id, _request_ids, attempt) =
            pending_batched_sortition_attempt(&network_id, &roster, context.height);
        let mut block = state.world.block();
        {
            let mut transaction = block.transaction_without_telemetry(
                iroha_config::parameters::actual::LaneConfig::default(),
                0,
            );
            transaction
                .put_parliament_attempt(attempt)
                .expect("persist the Parliament request and its beacon-slot index");
            transaction.apply();
        }
        block.commit();
        Some(governance_attempt_id)
    } else {
        None
    };

    let signers = (1_u16..=2)
        .map(|index| live_fixture_signer(&fixture_a, index))
        .collect::<Vec<_>>();
    let mut messages = Vec::new();
    for (index, signer) in signers.iter().enumerate() {
        let mut producer = V2GlobalBeaconLifecycle::open(
            &context,
            &state,
            Some(u32::try_from(index).expect("small validator index")),
            Some(Arc::clone(signer)),
        )
        .expect("open exact pre-transaction pulse producer");
        assert!(producer.pulse_requested());
        assert!(producer.pulse_required_for_consensus());
        producer.begin_round(0).expect("sign exact pulse slot");
        messages.push(beacon_partial_payload(
            producer.take_outbound().pop().expect("local pulse share"),
        ));
    }
    let mut reducer = V2GlobalBeaconLifecycle::open(&context, &state, Some(0), None)
        .expect("open exact signerless validator reducer");
    reducer.begin_round(0).expect("open pulse routing view");
    for (index, message) in messages.into_iter().enumerate() {
        reducer
            .accept_partial(message, &roster[index], 0)
            .expect("accept exact pre-transaction key share");
    }
    let pulse = reducer
        .finalized_pulse(0)
        .expect("threshold reconstructs the pre-transaction pulse");
    let mut effects = NposConsensusEffects::default();
    reducer
        .attach_candidate_effects(0, &mut effects)
        .expect("attach requested pre-transaction pulse");

    let mut dkg_b = adaptive_dkg_session_fixture();
    dkg_b.network_id = network_id;
    dkg_b.session_id = [0xB5; 32];
    dkg_b.roster_hash = global_threshold_beacon_roster_hash_v1(&roster);
    let fixture_b = adaptive_beacon_fixture_for_session(dkg_b);
    let key_b =
        FinalizedGlobalThresholdBeaconKeySessionRecordV1::new(fixture_b.session.record().clone())
            .expect("valid successor beacon key");
    let next_height = context
        .height
        .checked_add(1)
        .expect("next lifecycle height");
    let header = BlockHeader::new(
        core::num::NonZeroU64::new(context.height).expect("nonzero pulse height"),
        Some(parent_hash),
        None,
        0,
        0,
    );
    let committed_hash = header.hash();
    let evidence_prune_keys =
        crate::sumeragi::evidence::v2_committed_evidence_prune_keys_from_state(
            &state,
            context.height,
        )
        .expect("fund exact committed-evidence prune keys");
    let mut state_block = state.block(header);
    let mut transaction = state_block.transaction();
    transaction
        .world
        .retire_global_beacon_key_session(pulse.session_id, next_height)
        .expect("schedule predecessor retirement after the pulse height");
    transaction
        .world
        .put_finalized_global_beacon_key_session(key_b)
        .expect("persist proof-valid successor key");
    transaction
        .world
        .activate_global_beacon_key_session(fixture_b.session.record().session_id, next_height)
        .expect("schedule successor activation after the pulse height");
    let expected_anchor = GlobalThresholdBeaconChainAnchorV1 {
        height: context.height - 1,
        block_hash: parent_hash,
    };
    let mut stale_roster = roster.clone();
    stale_roster.reverse();
    let stale_roster_error =
        crate::sumeragi::penalties::apply_npos_consensus_effects_to_transaction(
            &mut transaction,
            &effects,
            evidence_prune_keys.as_slice(),
            Some(expected_anchor),
            &stale_roster,
            context.height,
            0,
            0,
        )
        .err()
        .expect("a stale height roster must reject the otherwise valid pulse");
    assert!(
        stale_roster_error
            .to_string()
            .contains("authenticated height roster"),
        "unexpected stale-roster diagnostic: {stale_roster_error}"
    );
    crate::sumeragi::penalties::apply_npos_consensus_effects_to_transaction(
        &mut transaction,
        &effects,
        evidence_prune_keys.as_slice(),
        Some(expected_anchor),
        &roster,
        context.height,
        0,
        0,
    )
    .expect("post-transaction rotation must preserve the parent-authorized pulse");
    if let Some(governance_attempt_id) = transient_governance_attempt_id {
        assert!(
            transaction
                .world
                .remove_parliament_attempt_for_testing(&governance_attempt_id)
                .is_some(),
            "transient logical-pulse request must remain present through effect application"
        );
    }
    transaction.apply();
    state_block
        .commit_world_overlay_for_testing()
        .expect("commit rotated key lifecycle and finalized pulse atomically");
    state.push_block_hash_for_testing(committed_hash);
    let committed_snapshot_hashes = state
        .block_hashes
        .view()
        .iter()
        .copied()
        .collect::<Vec<_>>();
    state
        .kura()
        .extend_hash_only_suffix_from_verified_snapshot(&committed_snapshot_hashes)
        .expect("persist the committed pulse-height hash-only snapshot suffix");

    let snapshot = norito::json::to_value(&state)
        .expect("serialize the committed key rotation and pulse history");
    let restored = crate::state::deserialize::KuraSeed {
        lane_manifests: state.lane_manifests.read().clone(),
        kura: state.kura_handle(),
        query_handle: LiveQueryStore::start_test(),
        #[cfg(feature = "telemetry")]
        telemetry: crate::telemetry::StateTelemetry::default(),
    }
    .into_state_from_json(snapshot)
    .expect("restart must restore the pulse-height key lifecycle");
    let world = restored.world.view();
    let persisted_a = world
        .global_beacon_key_sessions()
        .get(&pulse.session_id)
        .expect("predecessor key remains in public history");
    let persisted_b = world
        .global_beacon_key_sessions()
        .get(&fixture_b.session.record().session_id)
        .expect("successor key remains in public history");
    assert!(persisted_a.is_active_at(context.height));
    assert!(!persisted_a.is_active_at(next_height));
    assert!(!persisted_b.is_active_at(context.height));
    assert!(persisted_b.is_active_at(next_height));
    assert_eq!(
        world
            .global_beacon_active_session()
            .get(&GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY),
        Some(&fixture_b.session.record().session_id)
    );
    assert_eq!(
        verified_latest_global_threshold_beacon_pulse_v1(&world, &network_id, context.height,),
        Ok(pulse),
        "restored state must accept the persisted pulse under key A"
    );

    if !parliament_requested_slot {
        let mut block_hashes = (1_u8..=41)
            .map(|marker| {
                HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([marker; 32]))
            })
            .collect::<Vec<_>>();
        block_hashes
            [usize::try_from(expected_anchor.height - 1).expect("small parent-anchor height")] =
            expected_anchor.block_hash;
        assert_eq!(
            finalized_global_beacon_npos_successor_seed_from_sources(
                &world,
                &block_hashes,
                &network_id,
                next_height,
                context.epoch + 1,
            ),
            Ok(global_threshold_beacon_npos_successor_seed_v1(
                &pulse,
                next_height,
                context.epoch + 1,
            )),
            "successor construction must use the pulse-height key, not the post-block pointer"
        );
    }
}

#[test]
fn mandatory_parliament_pulse_persists_across_same_block_key_rotation() {
    assert_same_block_key_rotation_persists_requested_pulse(true);
}

#[test]
fn mandatory_npos_pulse_persists_across_same_block_key_rotation() {
    assert_same_block_key_rotation_persists_requested_pulse(false);
}

#[cfg(feature = "test-network-parliament-signers")]
#[test]
fn invalid_test_outbound_is_not_locally_counted_and_is_rejected_on_ingress() {
    let keys = live_producer_keys();
    let network_id = beacon_fixture_network_id(0xA7);
    let parent_hash = HashOf::from_untyped_unchecked(Hash::prehashed([0xD7; 32]));
    let context = live_producer_context(&keys, network_id, parent_hash, 42);
    let roster = context
        .roster
        .iter()
        .map(|entry| entry.validator.clone())
        .collect::<Vec<_>>();
    let mut dkg_session = adaptive_dkg_session_fixture();
    dkg_session.network_id = network_id;
    dkg_session.roster_hash = global_threshold_beacon_roster_hash_v1(&roster);
    let fixture = adaptive_beacon_fixture_for_session(dkg_session);
    let cursor = GlobalThresholdBeaconPulseLinkV1 {
        pulse_id: [0x67; 32],
        seed: [0x68; 32],
        height: 0,
        round: 0,
    };
    let state = live_producer_state(&fixture, cursor, parent_hash);

    let invalid_signer: Arc<dyn GlobalThresholdBeaconPartialSignerV1> =
        Arc::new(InvalidOutboundTestBeaconSigner {
            inner: live_fixture_signer(&fixture, 1),
        });
    let mut invalid_producer =
        V2GlobalBeaconLifecycle::open(&context, &state, Some(0), Some(invalid_signer))
            .expect("open deliberately invalid feature-only producer");
    invalid_producer
        .begin_round(0)
        .expect("an invalid test share is broadcast without local admission");
    let invalid = beacon_partial_payload(
        invalid_producer
            .take_outbound()
            .pop()
            .expect("invalid outbound share"),
    );

    let mut valid_producer = V2GlobalBeaconLifecycle::open(
        &context,
        &state,
        Some(1),
        Some(live_fixture_signer(&fixture, 2)),
    )
    .expect("open ordinary valid producer");
    valid_producer.begin_round(0).expect("sign valid share");
    let valid = beacon_partial_payload(
        valid_producer
            .take_outbound()
            .pop()
            .expect("valid outbound share"),
    );

    assert_eq!(
        invalid_producer
            .accept_partial(valid, &roster[1], 0)
            .expect("retain the sole proof-valid contribution"),
        V2GlobalBeaconIngressOutcome::Accepted,
        "one valid share must remain below the exact threshold of two",
    );
    assert!(invalid_producer.finalized_pulse(0).is_none());
    assert!(matches!(
        invalid_producer.accept_partial(invalid, &roster[0], 0),
        Err(V2GlobalBeaconError::Beacon(
            GlobalThresholdBeaconError::ThresholdBls(_)
        ))
    ));
    assert!(
        invalid_producer.finalized_pulse(0).is_none(),
        "the malformed outbound share must neither be pre-counted locally nor admitted on ingress",
    );
}

#[test]
fn transient_local_signing_failure_retries_same_view_and_allows_inbound_progress() {
    let keys = live_producer_keys();
    let network_id = beacon_fixture_network_id(0xA8);
    let parent_hash = HashOf::from_untyped_unchecked(Hash::prehashed([0xD8; 32]));
    let context = live_producer_context(&keys, network_id, parent_hash, 42);
    let roster = context
        .roster
        .iter()
        .map(|entry| entry.validator.clone())
        .collect::<Vec<_>>();
    let mut dkg_session = adaptive_dkg_session_fixture();
    dkg_session.network_id = network_id;
    dkg_session.roster_hash = global_threshold_beacon_roster_hash_v1(&roster);
    let fixture = adaptive_beacon_fixture_for_session(dkg_session);
    let cursor = GlobalThresholdBeaconPulseLinkV1 {
        pulse_id: [0x69; 32],
        seed: [0x6A; 32],
        height: 0,
        round: 0,
    };
    let state = live_producer_state(&fixture, cursor, parent_hash);

    let mut remote = V2GlobalBeaconLifecycle::open(
        &context,
        &state,
        Some(1),
        Some(live_fixture_signer(&fixture, 2)),
    )
    .expect("open remote beacon producer");
    remote.begin_round(0).expect("produce remote beacon share");
    let remote =
        beacon_partial_payload(remote.take_outbound().pop().expect("remote beacon partial"));

    let attempts = Arc::new(AtomicUsize::new(0));
    let flaky: Arc<dyn GlobalThresholdBeaconPartialSignerV1> = Arc::new(FailOnceBeaconSigner {
        inner: live_fixture_signer(&fixture, 1),
        attempts: Arc::clone(&attempts),
    });
    let mut producer = V2GlobalBeaconLifecycle::open(&context, &state, Some(0), Some(flaky))
        .expect("open fail-once beacon producer");

    assert!(matches!(
        producer.begin_round(0),
        Err(V2GlobalBeaconError::LocalSigning)
    ));
    assert!(producer.take_outbound().is_empty());
    assert!(producer.retransmission().is_empty());
    assert_eq!(
        producer
            .accept_partial(remote, &roster[1], 0)
            .expect("retry local signing before admitting the inbound share"),
        V2GlobalBeaconIngressOutcome::Finalized
    );
    assert_eq!(attempts.load(Ordering::SeqCst), 2);
    assert_eq!(producer.take_outbound().len(), 1);
    assert_eq!(producer.retransmission().len(), 1);
    assert!(producer.finalized_pulse(0).is_some());
}

#[test]
fn threshold_beacon_deferred_mandatory_height_stays_idle_until_real_work() {
    let keys = live_producer_keys();
    let network_id = beacon_fixture_network_id(0xA9);
    let parent_hash = HashOf::from_untyped_unchecked(Hash::prehashed([0xD9; 32]));
    let context = live_producer_context(&keys, network_id, parent_hash, 42);
    let roster = context
        .roster
        .iter()
        .map(|entry| entry.validator.clone())
        .collect::<Vec<_>>();
    let mut dkg_session = adaptive_dkg_session_fixture();
    dkg_session.network_id = network_id;
    dkg_session.roster_hash = global_threshold_beacon_roster_hash_v1(&roster);
    let fixture = adaptive_beacon_fixture_for_session(dkg_session);
    let cursor = GlobalThresholdBeaconPulseLinkV1 {
        pulse_id: [0x69; 32],
        seed: [0x6A; 32],
        height: 0,
        round: 0,
    };
    let state = Arc::new(live_producer_state(&fixture, cursor, parent_hash));
    {
        let mut world = state.world.block();
        world
            .global_beacon_active_session
            .remove(GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY);
        world.commit();
    }
    let attempts = Arc::new(AtomicUsize::new(0));
    let signer: Arc<dyn GlobalThresholdBeaconPartialSignerV1> = Arc::new(FailOnceBeaconSigner {
        inner: live_fixture_signer(&fixture, 1),
        attempts: Arc::clone(&attempts),
    });
    assert_eq!(context.height + 1, context.epoch_end_height);
    let mut producer =
        V2GlobalBeaconLifecycle::open_deferred(&context, Arc::clone(&state), Some(0), Some(signer))
            .expect("an idle mandatory height does not require session activation");
    assert!(producer.pulse_requested());
    assert!(producer.pulse_required_for_consensus());
    for view in [0, 1] {
        producer
            .begin_round(view)
            .expect("idle view remains dormant");
        assert!(producer.take_outbound().is_empty());
        assert!(producer.retransmission().is_empty());
        assert!(producer.finalized_pulse(view).is_none());
    }
    assert_eq!(attempts.load(Ordering::SeqCst), 0);
    for _ in 0..2 {
        assert!(matches!(
            producer.activate(),
            Err(V2GlobalBeaconError::State("active key session is absent"))
        ));
        let mut effects = NposConsensusEffects::default();
        assert!(producer.attach_candidate_effects(1, &mut effects).is_err());
        assert!(effects.is_empty());
    }
    assert_eq!(attempts.load(Ordering::SeqCst), 0);
    assert!(producer.take_outbound().is_empty());
    assert_eq!(state.block_hashes.view().len(), 40);
    assert_eq!(state.block_hashes.view().last(), Some(&parent_hash));
    let world = state.world.view();
    assert!(world.global_beacon_pulses().iter().next().is_none());
    assert_eq!(
        world
            .global_beacon_latest_pulse()
            .get(&GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY),
        Some(&cursor),
        "idle and rejected activation must not advance the committed pulse cursor"
    );
}

#[test]
fn threshold_beacon_live_v2_producer_is_bound_restartable_and_persists_effect() {
    let keys = live_producer_keys();
    let network_id = beacon_fixture_network_id(0xA1);
    let parent_hash = HashOf::from_untyped_unchecked(Hash::prehashed([0xD1; 32]));
    let context = live_producer_context(&keys, network_id, parent_hash, 42);
    let roster = context
        .roster
        .iter()
        .map(|entry| entry.validator.clone())
        .collect::<Vec<_>>();
    let mut dkg_session = adaptive_dkg_session_fixture();
    dkg_session.network_id = network_id;
    dkg_session.roster_hash = global_threshold_beacon_roster_hash_v1(&roster);
    let fixture = adaptive_beacon_fixture_for_session(dkg_session);
    let cursor = GlobalThresholdBeaconPulseLinkV1 {
        pulse_id: [0x61; 32],
        seed: [0x62; 32],
        height: 0,
        round: 0,
    };
    let state = Arc::new(live_producer_state(&fixture, cursor, parent_hash));
    let signers = (1_u16..=4)
        .map(|index| live_fixture_signer(&fixture, index))
        .collect::<Vec<_>>();

    let mut messages = Vec::new();
    for (index, signer) in signers.iter().enumerate() {
        let mut producer = V2GlobalBeaconLifecycle::open_deferred(
            &context,
            Arc::clone(&state),
            Some(u32::try_from(index).expect("validator index")),
            Some(Arc::clone(signer)),
        )
        .expect("open deferred validator producer");
        producer.begin_round(0).expect("idle round remains dormant");
        assert!(producer.take_outbound().is_empty());
        assert!(producer.retransmission().is_empty());
        assert!(producer.finalized_pulse(0).is_none());
        producer
            .activate()
            .expect("real carrier demand activates the exact session");
        producer.activate().expect("activation is idempotent");
        producer.begin_round(0).expect("sign exact round");
        let outbound = producer.take_outbound();
        assert_eq!(outbound.len(), 1);
        messages.push(beacon_partial_payload(
            outbound.into_iter().next().expect("local partial"),
        ));
    }

    let mut reducer = V2GlobalBeaconLifecycle::open(&context, &state, Some(0), None)
        .expect("open signerless validator reducer");
    reducer.begin_round(0).expect("open exact round");
    let mut absent_effects = NposConsensusEffects::default();
    assert!(matches!(
        reducer.attach_candidate_effects(0, &mut absent_effects),
        Err(V2GlobalBeaconError::State(_))
    ));

    let mut wrong_sender = messages[0].clone();
    assert_eq!(wrong_sender.partial.signer_index, 1);
    assert!(matches!(
        reducer.accept_partial(wrong_sender.clone(), &roster[1], 0),
        Err(V2GlobalBeaconError::SenderMismatch)
    ));
    wrong_sender.round.view = 1;
    assert!(matches!(
        reducer.accept_partial(wrong_sender, &roster[0], 0),
        Err(V2GlobalBeaconError::WrongView)
    ));
    let mut wrong_session = messages[0].clone();
    wrong_session.partial.session_id[0] ^= 1;
    assert!(matches!(
        reducer.accept_partial(wrong_session, &roster[0], 0),
        Err(V2GlobalBeaconError::Beacon(
            GlobalThresholdBeaconError::SessionMismatch
        ))
    ));

    assert_eq!(
        reducer
            .accept_partial(messages[0].clone(), &roster[0], 0)
            .expect("first verified share"),
        V2GlobalBeaconIngressOutcome::Accepted
    );
    let mut conflicting = messages[0].clone();
    conflicting.partial.signature_share[0] ^= 1;
    assert!(matches!(
        reducer.accept_partial(conflicting, &roster[0], 0),
        Err(V2GlobalBeaconError::Beacon(
            GlobalThresholdBeaconError::ThresholdBls(_)
        ))
    ));

    let mut restarted_signer =
        V2GlobalBeaconLifecycle::open(&context, &state, Some(0), Some(Arc::clone(&signers[0])))
            .expect("restart signer");
    restarted_signer.begin_round(0).expect("retry exact share");
    let retry = beacon_partial_payload(
        restarted_signer
            .take_outbound()
            .pop()
            .expect("retried local partial"),
    );
    assert_eq!(
        retry.partial.signature_share,
        messages[0].partial.signature_share
    );
    assert_ne!(retry.partial.proof, messages[0].partial.proof);
    assert_eq!(
        reducer
            .accept_partial(retry, &roster[0], 0)
            .expect("fresh-proof retry"),
        V2GlobalBeaconIngressOutcome::Duplicate,
        "fresh proof randomness for one verified share must stay idempotent"
    );

    assert_eq!(
        reducer
            .accept_partial(messages[1].clone(), &roster[1], 0)
            .expect("threshold share"),
        V2GlobalBeaconIngressOutcome::Finalized
    );
    let pulse = reducer.finalized_pulse(0).expect("unique finalized pulse");
    for index in 2..4 {
        assert_eq!(
            reducer
                .accept_partial(messages[index].clone(), &roster[index], 0)
                .expect("additional four-validator-path share"),
            V2GlobalBeaconIngressOutcome::Duplicate
        );
        assert_eq!(reducer.finalized_pulse(0), Some(pulse));
    }

    let mut effects = NposConsensusEffects::default();
    reducer
        .attach_candidate_effects(0, &mut effects)
        .expect("attach exact finalized pulse to candidate effects");
    assert_eq!(effects.finalized_global_beacon_pulse, Some(pulse));

    let mut pulse_only: iroha_data_model::block::SignedBlock =
        crate::block::ValidBlock::new_dummy_and_modify_header(keys[0].private_key(), |header| {
            header.set_height(core::num::NonZeroU64::new(context.height).expect("pulse height"));
            header.set_prev_block_hash(Some(parent_hash));
        })
        .into();
    pulse_only.set_npos_consensus_effects(Some(effects.clone()));
    assert!(
        !crate::sumeragi::v2_candidate::candidate_block_has_proposal_work(
            &pulse_only,
            &state,
            false,
        )
        .unwrap(),
        "a cryptographically finalized pulse cannot manufacture proposal work"
    );
    let account_key =
        KeyPair::try_from_seed(vec![0xE1; 32], Algorithm::Ed25519).expect("external operation key");
    let transaction = iroha_data_model::transaction::TransactionBuilder::new(
        network_id,
        AccountId::new(account_key.public_key().clone()),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([iroha_data_model::isi::Log::new(
        iroha_data_model::Level::INFO,
        "genuine operation accompanying the required pulse".to_owned(),
    )])
    .sign(account_key.private_key());
    pulse_only.set_external_entrypoints(vec![
        iroha_data_model::transaction::TransactionEntrypoint::External(transaction),
    ]);
    assert!(
        crate::sumeragi::v2_candidate::candidate_block_has_proposal_work(
            &pulse_only,
            &state,
            false,
        )
        .unwrap(),
        "the same authenticated pulse may accompany a genuine external operation"
    );

    let mut restarted = V2GlobalBeaconLifecycle::open(&context, &state, Some(0), None)
        .expect("restart signerless validator reducer");
    restarted.begin_round(0).expect("reopen exact round");
    assert_eq!(
        restarted
            .accept_partial(messages[0].clone(), &roster[0], 0)
            .expect("replayed first share after restart"),
        V2GlobalBeaconIngressOutcome::Accepted
    );
    assert_eq!(
        restarted
            .accept_partial(messages[1].clone(), &roster[1], 0)
            .expect("replayed threshold after restart"),
        V2GlobalBeaconIngressOutcome::Finalized
    );
    assert_eq!(restarted.finalized_pulse(0), Some(pulse));
    assert_eq!(pulse.round, GLOBAL_THRESHOLD_BEACON_PULSE_ROUND_V1);
    restarted
        .begin_round(1)
        .expect("advance routing view without changing pulse payload");
    assert_eq!(
        restarted.finalized_pulse(1),
        Some(pulse),
        "a view change must retain the already reconstructed unique pulse"
    );

    let mut view_one_producer =
        V2GlobalBeaconLifecycle::open(&context, &state, Some(0), Some(Arc::clone(&signers[0])))
            .expect("view-one producer");
    view_one_producer.begin_round(1).expect("sign view one");
    let view_one_partial = beacon_partial_payload(
        view_one_producer
            .take_outbound()
            .pop()
            .expect("view-one partial"),
    );
    assert_eq!(
        view_one_partial.partial.signature_share, messages[0].partial.signature_share,
        "consensus view must not alter the threshold-signed message"
    );
    assert_eq!(
        restarted
            .accept_partial(view_one_partial, &roster[0], 1)
            .expect("cross-view retry of the same height-bound share"),
        V2GlobalBeaconIngressOutcome::Duplicate
    );

    let wrong_payload = wire::GlobalBeaconPartialSignature {
        round: wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 1,
        },
        partial: signers[0]
            .sign_partial(&fixture.session, b"wrong beacon payload")
            .expect("sign deliberately wrong payload"),
    };
    assert!(matches!(
        restarted.accept_partial(wrong_payload, &roster[0], 1),
        Err(V2GlobalBeaconError::Beacon(
            GlobalThresholdBeaconError::ThresholdBls(_)
        ))
    ));

    let other_parent = HashOf::from_untyped_unchecked(Hash::prehashed([0xD2; 32]));
    let other_context = live_producer_context(&keys, network_id, other_parent, 42);
    let other_state = live_producer_state(&fixture, cursor, other_parent);
    let mut other_anchor_producer = V2GlobalBeaconLifecycle::open(
        &other_context,
        &other_state,
        Some(0),
        Some(Arc::clone(&signers[0])),
    )
    .expect("other-anchor producer");
    other_anchor_producer
        .begin_round(0)
        .expect("sign other anchor");
    let mut wrong_anchor = beacon_partial_payload(
        other_anchor_producer
            .take_outbound()
            .pop()
            .expect("other-anchor partial"),
    );
    wrong_anchor.round.context_id = context.id();
    wrong_anchor.round.view = 1;
    assert!(matches!(
        restarted.accept_partial(wrong_anchor, &roster[0], 1),
        Err(V2GlobalBeaconError::Beacon(
            GlobalThresholdBeaconError::ThresholdBls(_)
        ))
    ));

    let expected_anchor = GlobalThresholdBeaconChainAnchorV1 {
        height: 40,
        block_hash: parent_hash,
    };
    {
        let mut block = state.world.block();
        {
            let mut transaction =
                block.transaction_without_telemetry(RuntimeLaneConfig::default(), 0);
            transaction
                .verify_and_advance_global_beacon_pulse(&fixture.session, pulse, expected_anchor)
                .expect("persist finalized pulse through authoritative World corridor");
            transaction.apply();
        }
        block.commit();
    }
    assert_eq!(
        verified_latest_global_threshold_beacon_pulse_v1(&state.world.view(), &network_id, 41,),
        Ok(pulse)
    );
}

#[test]
fn threshold_beacon_session_validates_complete_canonical_transcript() {
    let (session, expected) = validated_threshold_session();
    assert_eq!(session.record().transcript_hash, expected.transcript_hash);
    assert_eq!(session.record().public_shares.len(), 4);
    assert_eq!(session.ensure_adaptive_protocol_ready(), Ok(()));
}

#[test]
fn threshold_beacon_session_rejects_wrong_bindings_and_malformed_points() {
    let (session, expected) = validated_threshold_session();
    let record = session.record().clone();

    let mut wrong_network = record.clone();
    wrong_network.network_id = beacon_fixture_network_id(0x82);
    assert_eq!(
        validate_global_threshold_beacon_session_v1(wrong_network, &expected),
        Err(GlobalThresholdBeaconError::NetworkMismatch)
    );

    let mut wrong_roster = record.clone();
    wrong_roster.roster_hash[0] ^= 1;
    assert_eq!(
        validate_global_threshold_beacon_session_v1(wrong_roster, &expected),
        Err(GlobalThresholdBeaconError::RosterMismatch)
    );

    let mut zero_roster = record.clone();
    zero_roster.roster_hash = [0; 32];
    zero_roster.adaptive_dkg.session.roster_hash = [0; 32];
    let zero_roster_binding = GlobalThresholdBeaconSessionBindingV1 {
        roster_hash: [0; 32],
        ..expected
    };
    assert_eq!(
        validate_global_threshold_beacon_session_v1(zero_roster, &zero_roster_binding),
        Err(GlobalThresholdBeaconError::InvalidDkgSession)
    );

    let mut wrong_transcript = record.clone();
    wrong_transcript.transcript_hash[0] ^= 1;
    assert_eq!(
        validate_global_threshold_beacon_session_v1(wrong_transcript, &expected),
        Err(GlobalThresholdBeaconError::TranscriptMismatch)
    );

    let mut malformed_key = record;
    malformed_key.group_public_key = [0; 96];
    assert_eq!(
        validate_global_threshold_beacon_session_v1(malformed_key, &expected),
        Err(GlobalThresholdBeaconError::ThresholdBls(
            ThresholdBlsError::InvalidPublicKey
        ))
    );
}

#[test]
fn threshold_beacon_and_tle_sessions_are_type_and_domain_separated() {
    let (session, _) = validated_threshold_session();
    let record = session.record();
    let beacon = ThresholdBlsSession::<BeaconPurpose>::new(
        *record.network_id.as_bytes(),
        record.session_id,
        record.roster_hash,
        record.committee_size,
        record.threshold,
    )
    .expect("beacon session");
    let tle = ThresholdBlsSession::<TleReleasePurpose>::new(
        *record.network_id.as_bytes(),
        record.session_id,
        record.roster_hash,
        record.committee_size,
        record.threshold,
    )
    .expect("TLE session");
    assert_ne!(
        beacon
            .signing_message(b"same payload")
            .expect("beacon message"),
        tle.signing_message(b"same payload").expect("TLE message")
    );
}

#[test]
fn threshold_beacon_pulse_payload_binds_every_consensus_field() {
    let (session, _) = validated_threshold_session();
    let (pulse, _, _) = pulse_fixture(&session);
    let baseline = global_threshold_beacon_pulse_payload_v1(&pulse);

    let mut mutations = Vec::new();
    let mut changed = pulse.clone();
    changed.network_id = beacon_fixture_network_id(0x82);
    mutations.push(changed);
    let mut changed = pulse.clone();
    changed.session_id[0] ^= 1;
    mutations.push(changed);
    let mut changed = pulse.clone();
    changed.roster_hash[0] ^= 1;
    mutations.push(changed);
    let mut changed = pulse.clone();
    changed.transcript_hash[0] ^= 1;
    mutations.push(changed);
    let mut changed = pulse.clone();
    changed.height += 1;
    mutations.push(changed);
    let mut changed = pulse.clone();
    changed.round += 1;
    mutations.push(changed);
    let mut changed = pulse;
    changed.finalized_chain_anchor.block_hash =
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x89; 32]));
    mutations.push(changed);

    for mutation in mutations {
        assert_ne!(
            global_threshold_beacon_pulse_payload_v1(&mutation),
            baseline
        );
    }
}

#[test]
fn threshold_beacon_accepts_one_adaptive_final_signature_and_seed() {
    let (fixture, pulse, _cursor, anchor) = signed_pulse_fixture();

    assert_eq!(
        verify_finalized_global_threshold_beacon_pulse_v1(&fixture.session, &pulse, anchor,),
        Ok(GlobalThresholdBeaconPulseLinkV1 {
            pulse_id: pulse.pulse_id,
            seed: pulse.seed,
            height: pulse.height,
            round: pulse.round,
        })
    );
}

#[test]
fn first_finalized_pulse_initializes_an_empty_ingestion_cursor() {
    let (fixture, pulse, _origin, anchor) = signed_pulse_fixture();
    let mut key_record =
        FinalizedGlobalThresholdBeaconKeySessionRecordV1::new(fixture.session.record().clone())
            .expect("valid finalized beacon key");
    key_record
        .activate(key_record.session.adaptive_dkg.finalized_at_height)
        .expect("activate finalized beacon key");
    let world = World::new();
    {
        let mut block = world.block();
        {
            let mut transaction =
                block.transaction_without_telemetry(RuntimeLaneConfig::default(), 0);
            transaction
                .global_beacon_key_sessions
                .insert(pulse.session_id, key_record);
            transaction
                .global_beacon_active_session
                .insert(GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY, pulse.session_id);
            assert_eq!(
                transaction
                    .verify_and_advance_global_beacon_pulse(&fixture.session, pulse, anchor,),
                Ok(GlobalThresholdBeaconPulseLinkV1 {
                    pulse_id: pulse.pulse_id,
                    seed: pulse.seed,
                    height: pulse.height,
                    round: pulse.round,
                })
            );
            transaction.apply();
        }
        block.commit();
    }
    assert_eq!(
        verified_latest_global_threshold_beacon_pulse_v1(
            &world.view(),
            &pulse.network_id,
            pulse.height,
        ),
        Ok(pulse),
        "the first certified pulse must create the authoritative cursor without a test-seeded origin"
    );
    let world_view = world.view();
    assert_eq!(
        world_view.global_beacon_pulse_at_slot(&pulse.network_id, pulse.height),
        Some(&pulse),
        "the authoritative insertion corridor must update the exact slot index atomically"
    );
    drop(world_view);
    assert_eq!(
        verified_global_threshold_beacon_pulse_at_or_before_v1(
            &world.view(),
            &pulse.network_id,
            pulse.height,
        ),
        Ok(pulse)
    );
    assert_eq!(
        verified_global_threshold_beacon_pulse_at_or_before_v1(
            &world.view(),
            &pulse.network_id,
            pulse.height - 1,
        ),
        Err(GlobalThresholdBeaconError::InvalidPulseHistory)
    );

    let mut conflicting_slot = pulse;
    conflicting_slot.pulse_id[0] ^= 1;
    let mut block = world.block();
    let mut transaction = block.transaction_without_telemetry(RuntimeLaneConfig::default(), 0);
    assert_eq!(
        transaction.verify_and_advance_global_beacon_pulse(
            &fixture.session,
            conflicting_slot,
            anchor,
        ),
        Err(GlobalThresholdBeaconError::ReusedPulse),
        "a distinct pulse id cannot claim an already indexed logical-beacon-height slot"
    );
}

#[test]
fn late_sortition_pulse_is_rejected_after_parliament_transcript_restart_roundtrip() {
    let (fixture, pulse, _origin, anchor) = signed_pulse_fixture();
    let roster = live_producer_keys()
        .into_iter()
        .map(|key| PeerId::new(key.public_key().clone()))
        .collect::<Vec<_>>();
    let (governance_attempt_id, _, mut attempt) =
        pending_batched_sortition_attempt(&pulse.network_id, &roster, pulse.height);
    let failed_election_id =
        BodyElectionAttemptId::derive_v1(governance_attempt_id, ParliamentBody::RulesCommittee, 0);
    attempt
        .fail_body_election_no_roster(
            governance_attempt_id,
            failed_election_id,
            false,
            pulse.height + 1,
        )
        .expect("terminally classify the missing initial sortition slot");
    let logical_session = BeaconSessionId::for_network_v1(&pulse.network_id);
    assert!(attempt.classifies_beacon_pulse_unavailable_at(logical_session, pulse.height));

    let encoded = norito::json::to_json(&attempt)
        .expect("serialize the terminal Parliament transcript for restart");
    let restored_attempt: ParliamentAttemptStateV1 =
        norito::json::from_str(&encoded).expect("restore the terminal Parliament transcript");
    restored_attempt
        .validate()
        .expect("restored missing-pulse transcript remains canonical");
    assert!(restored_attempt.classifies_beacon_pulse_unavailable_at(logical_session, pulse.height));

    let mut key_record =
        FinalizedGlobalThresholdBeaconKeySessionRecordV1::new(fixture.session.record().clone())
            .expect("valid finalized beacon key");
    key_record
        .activate(key_record.session.adaptive_dkg.finalized_at_height)
        .expect("activate finalized beacon key");
    let world = World::new();
    let mut block = world.block();
    let mut transaction = block.transaction_without_telemetry(RuntimeLaneConfig::default(), 0);
    transaction
        .global_beacon_key_sessions
        .insert(pulse.session_id, key_record);
    transaction
        .put_parliament_attempt(restored_attempt)
        .expect("index restored terminal Parliament pulse classification");
    assert_eq!(
        transaction.verify_and_advance_global_beacon_pulse(&fixture.session, pulse, anchor,),
        Err(GlobalThresholdBeaconError::PersistenceConflict),
        "restart must not reopen a sortition slot already closed as unavailable"
    );
}

#[test]
fn threshold_beacon_slot_is_identical_when_prior_unrelated_height_is_persisted_or_omitted() {
    let fixture = adaptive_beacon_fixture();
    let (template, origin, target_anchor) = pulse_fixture(&fixture.session);
    let finalize_slot = |height: u64,
                         anchor: GlobalThresholdBeaconChainAnchorV1,
                         proof_seed: [u8; 32]| {
        let mut unsigned = template;
        unsigned.height = height;
        unsigned.finalized_chain_anchor = anchor;
        let partials = pulse_partial_signatures(&fixture, &unsigned, proof_seed);
        let mut aggregator =
            GlobalThresholdBeaconPulseAggregatorV1::new(fixture.session.clone(), height, anchor)
                .expect("open unchained slot aggregator");
        for partial in partials.into_iter().take(usize::from(
            fixture.session.transcript.session().threshold(),
        )) {
            aggregator
                .accept_partial(partial)
                .expect("accept exact slot partial");
        }
        aggregator.finalize().expect("finalize exact slot")
    };

    let target_without_prior = finalize_slot(41, target_anchor, [0x31; 32]);
    let prior_anchor = GlobalThresholdBeaconChainAnchorV1 {
        height: 39,
        block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x87; 32])),
    };
    let prior = finalize_slot(40, prior_anchor, [0x32; 32]);
    let mut key_record =
        FinalizedGlobalThresholdBeaconKeySessionRecordV1::new(fixture.session.record().clone())
            .expect("valid finalized key");
    key_record
        .activate(key_record.session.adaptive_dkg.finalized_at_height)
        .expect("activate finalized key");
    let world = World::new();
    {
        let mut block = world.block();
        {
            let mut transaction =
                block.transaction_without_telemetry(RuntimeLaneConfig::default(), 0);
            transaction
                .global_beacon_key_sessions
                .insert(prior.session_id, key_record);
            transaction
                .global_beacon_active_session
                .insert(GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY, prior.session_id);
            transaction
                .global_beacon_latest_pulse
                .insert(GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY, origin);
            transaction
                .verify_and_advance_global_beacon_pulse(&fixture.session, prior, prior_anchor)
                .expect("persist prior unrelated pulse");
            transaction.apply();
        }
        block.commit();
    }
    assert_eq!(
        verified_latest_global_threshold_beacon_pulse_v1(
            &world.view(),
            &prior.network_id,
            prior.height,
        ),
        Ok(prior)
    );

    let target_after_persisted_prior = finalize_slot(41, target_anchor, [0x33; 32]);
    assert_eq!(target_after_persisted_prior, target_without_prior);
    assert_eq!(
        target_after_persisted_prior.seed, target_without_prior.seed,
        "an unrelated earlier pulse must not influence the later slot seed"
    );
}

#[test]
fn parliament_seed_reverifies_valid_and_rejects_tampered_persisted_pulse() {
    let (fixture, pulse, _origin, _anchor) = signed_pulse_fixture();
    let mut key_record =
        FinalizedGlobalThresholdBeaconKeySessionRecordV1::new(fixture.session.record().clone())
            .expect("valid finalized key");
    key_record
        .activate(key_record.session.adaptive_dkg.finalized_at_height)
        .expect("activate finalized key");
    let link = GlobalThresholdBeaconPulseLinkV1 {
        pulse_id: pulse.pulse_id,
        seed: pulse.seed,
        height: pulse.height,
        round: pulse.round,
    };
    let persisted_world = |stored_pulse: FinalizedGlobalThresholdBeaconPulseV1| {
        let world = World::new();
        {
            let mut block = world.block();
            block
                .global_beacon_key_sessions
                .insert(pulse.session_id, key_record.clone());
            block
                .global_beacon_active_session
                .insert(GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY, pulse.session_id);
            block
                .global_beacon_pulses
                .insert(stored_pulse.pulse_id, stored_pulse);
            block.global_beacon_pulse_slots.insert(
                (
                    BeaconSessionId::for_network_v1(&stored_pulse.network_id),
                    stored_pulse.height,
                ),
                stored_pulse.pulse_id,
            );
            block
                .global_beacon_latest_pulse
                .insert(GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY, link);
            block.commit();
        }
        world
    };

    let valid_world = persisted_world(pulse);
    assert_eq!(
        verified_persisted_global_threshold_beacon_governance_seed_v1(
            &valid_world.view(),
            &pulse.network_id,
            pulse,
            pulse.height,
        ),
        Ok(global_threshold_beacon_governance_seed_v1(
            &pulse,
            pulse.height,
        ))
    );

    let mut tampered = pulse;
    tampered.signature[0] ^= 1;
    let tampered_world = persisted_world(tampered);
    assert!(
        verified_persisted_global_threshold_beacon_governance_seed_v1(
            &tampered_world.view(),
            &pulse.network_id,
            tampered,
            pulse.height,
        )
        .is_err(),
        "Parliament must not derive entropy from an invalid restored pulse"
    );
}

#[test]
fn threshold_beacon_partial_reducer_is_bound_fail_closed_and_subset_invariant() {
    let fixture = adaptive_beacon_fixture();
    let (pulse, _cursor, anchor) = pulse_fixture(&fixture.session);
    let partials = pulse_partial_signatures(&fixture, &pulse, [0xA7; 32]);
    let threshold = usize::from(fixture.session.transcript.session().threshold());

    let open_reducer = || {
        GlobalThresholdBeaconPulseAggregatorV1::new(fixture.session.clone(), pulse.height, anchor)
            .expect("open exact pulse reducer")
    };
    let mut insufficient = open_reducer();
    for partial in partials.iter().take(threshold - 1).cloned() {
        assert_eq!(insufficient.accept_partial(partial), Ok(true));
    }
    assert_eq!(
        insufficient.finalize(),
        Err(GlobalThresholdBeaconError::InsufficientPartialSignatures)
    );

    let mut low_indices = open_reducer();
    for partial in partials.iter().take(threshold).cloned() {
        assert_eq!(low_indices.accept_partial(partial), Ok(true));
    }
    assert_eq!(
        low_indices.accept_partial(partials[0]),
        Ok(false),
        "exact retransmissions must be idempotent"
    );
    let low_pulse = low_indices.finalize().expect("low-index threshold");

    let mut high_indices = open_reducer();
    for partial in partials.iter().rev().take(threshold).cloned() {
        assert_eq!(high_indices.accept_partial(partial), Ok(true));
    }
    assert_eq!(
        high_indices.finalize().expect("high-index threshold"),
        low_pulse,
        "the public pulse must not expose or depend on the reconstruction subset"
    );

    let mut wrong_session = partials[0];
    wrong_session.session_id[0] ^= 1;
    assert_eq!(
        open_reducer().accept_partial(wrong_session),
        Err(GlobalThresholdBeaconError::SessionMismatch)
    );

    let distinct_valid_proof = pulse_partial_signatures(&fixture, &pulse, [0xB8; 32])[0];
    assert_ne!(partials[0], distinct_valid_proof);
    let mut equivocating = open_reducer();
    assert_eq!(equivocating.accept_partial(partials[0]), Ok(true));
    assert_eq!(
        equivocating.accept_partial(distinct_valid_proof),
        Ok(false),
        "fresh proof randomness for the same verified share is an idempotent retry"
    );

    let mismatched_anchor = GlobalThresholdBeaconChainAnchorV1 {
        height: pulse.height,
        block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x89; 32])),
    };
    let mut other_height = GlobalThresholdBeaconPulseAggregatorV1::new(
        fixture.session.clone(),
        pulse.height + 1,
        mismatched_anchor,
    )
    .expect("open another exact pulse round");
    assert!(matches!(
        other_height.accept_partial(partials[0]),
        Err(GlobalThresholdBeaconError::ThresholdBls(_))
    ));
}

#[test]
fn npos_successor_seed_requires_one_exact_finalized_pulse_and_chain_anchor() {
    const BOUNDARY_HEIGHT: u64 = 42;
    const SUCCESSOR_EPOCH: u64 = 9;
    let (fixture, pulse, _cursor, anchor) = signed_pulse_fixture();
    let mut key_record =
        FinalizedGlobalThresholdBeaconKeySessionRecordV1::new(fixture.session.record().clone())
            .expect("valid finalized beacon key");
    key_record
        .activate(key_record.session.adaptive_dkg.finalized_at_height)
        .expect("activate finalized beacon key");
    let link = GlobalThresholdBeaconPulseLinkV1 {
        pulse_id: pulse.pulse_id,
        seed: pulse.seed,
        height: pulse.height,
        round: pulse.round,
    };
    let world = World::new();
    {
        let mut block = world.block();
        block
            .global_beacon_key_sessions
            .insert(pulse.session_id, key_record);
        block
            .global_beacon_active_session
            .insert(GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY, pulse.session_id);
        block.global_beacon_pulses.insert(pulse.pulse_id, pulse);
        block.global_beacon_pulse_slots.insert(
            (
                BeaconSessionId::for_network_v1(&pulse.network_id),
                pulse.height,
            ),
            pulse.pulse_id,
        );
        block
            .global_beacon_latest_pulse
            .insert(GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY, link);
        block.commit();
    }
    let mut block_hashes = (1_u8..=41)
        .map(|marker| HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([marker; 32])))
        .collect::<Vec<_>>();
    block_hashes[usize::try_from(anchor.height - 1).expect("small anchor height")] =
        anchor.block_hash;
    let world_view = world.view();
    let expected_seed =
        global_threshold_beacon_npos_successor_seed_v1(&pulse, BOUNDARY_HEIGHT, SUCCESSOR_EPOCH);
    assert_ne!(
        expected_seed, pulse.seed,
        "NPoS must not reuse the raw pulse seed"
    );
    assert_ne!(
        expected_seed,
        global_threshold_beacon_npos_successor_seed_v1(
            &pulse,
            BOUNDARY_HEIGHT,
            SUCCESSOR_EPOCH + 1,
        ),
        "the target epoch is part of the NPoS seed domain"
    );
    assert_eq!(
        finalized_global_beacon_npos_successor_seed_from_sources(
            &world_view,
            &block_hashes,
            &pulse.network_id,
            BOUNDARY_HEIGHT,
            SUCCESSOR_EPOCH,
        ),
        Ok(expected_seed)
    );

    let empty_world = World::new();
    assert_eq!(
        finalized_global_beacon_npos_successor_seed_from_sources(
            &empty_world.view(),
            &block_hashes,
            &pulse.network_id,
            BOUNDARY_HEIGHT,
            SUCCESSOR_EPOCH,
        ),
        Err(V2ContextBuildError::MissingPreBoundaryBeaconPulse)
    );
    assert_eq!(
        finalized_global_beacon_npos_successor_seed_from_sources(
            &world_view,
            &block_hashes,
            &beacon_fixture_network_id(0x82),
            BOUNDARY_HEIGHT,
            SUCCESSOR_EPOCH,
        ),
        Err(V2ContextBuildError::InvalidPreBoundaryBeaconPulse)
    );
    block_hashes[usize::try_from(anchor.height - 1).expect("small anchor height")] =
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xFE; 32]));
    assert_eq!(
        finalized_global_beacon_npos_successor_seed_from_sources(
            &world_view,
            &block_hashes,
            &pulse.network_id,
            BOUNDARY_HEIGHT,
            SUCCESSOR_EPOCH,
        ),
        Err(V2ContextBuildError::InvalidPreBoundaryBeaconPulse)
    );
}

#[test]
fn threshold_beacon_pulse_rejects_zero_noncanonical_round_and_wrong_anchor() {
    let (session, _) = validated_threshold_session();
    let (pulse, _cursor, anchor) = pulse_fixture(&session);

    let mut zero = pulse.clone();
    zero.seed = [0; 32];
    assert_eq!(
        verify_finalized_global_threshold_beacon_pulse_v1(&session, &zero, anchor),
        Err(GlobalThresholdBeaconError::ZeroPulse)
    );

    let mut noncanonical_round = pulse;
    noncanonical_round.round = 1;
    assert_eq!(
        verify_finalized_global_threshold_beacon_pulse_v1(&session, &noncanonical_round, anchor,),
        Err(GlobalThresholdBeaconError::NonCanonicalRound)
    );

    let wrong_anchor = GlobalThresholdBeaconChainAnchorV1 {
        height: anchor.height + 1,
        ..anchor
    };
    assert_eq!(
        verify_finalized_global_threshold_beacon_pulse_v1(&session, &pulse, wrong_anchor),
        Err(GlobalThresholdBeaconError::FinalizedAnchorMismatch)
    );
}

#[test]
fn threshold_beacon_pulse_rejects_malformed_and_wrong_final_signatures() {
    let (session, _) = validated_threshold_session();
    let (pulse, _cursor, anchor) = pulse_fixture(&session);

    let mut malformed = pulse.clone();
    malformed.signature = [0; 48];
    assert_eq!(
        verify_finalized_global_threshold_beacon_pulse_v1(&session, &malformed, anchor),
        Err(GlobalThresholdBeaconError::ThresholdBls(
            ThresholdBlsError::InvalidSignature
        ))
    );
    assert_eq!(
        verify_finalized_global_threshold_beacon_pulse_v1(&session, &pulse, anchor),
        Err(GlobalThresholdBeaconError::ThresholdBls(
            ThresholdBlsError::SignatureMismatch
        ))
    );
}

#[test]
fn shared_beacon_frame_owners_pass_the_production_session_and_pulse_boundaries() {
    use crate::private_settlement::global_state::tests::assert_private_settlement_frame_v1 as check;
    let (fixture, pulse, _cursor, anchor) = signed_pulse_fixture();
    let session = &fixture.session;
    let record = session.record();
    let expected = GlobalThresholdBeaconSessionBindingV1 {
        network_id: record.network_id,
        session_id: record.session_id,
        roster_hash: record.roster_hash,
        transcript_hash: record.transcript_hash,
    };
    check(
        record,
        "iroha_data_model::consensus::GlobalThresholdBeaconKeySessionV1",
    );
    check(
        &pulse,
        "iroha_data_model::consensus::FinalizedGlobalThresholdBeaconPulseV1",
    );
    let session_frame = norito::encode_canonical(record).expect("shared session frame");
    let decoded = decode_global_threshold_beacon_session_v1(&session_frame, &expected)
        .expect("canonical session passes production transcript validation");
    assert_eq!(decoded.record(), record);
    let pulse_frame = norito::encode_canonical(&pulse).expect("shared pulse frame");
    let link = decode_finalized_global_threshold_beacon_pulse_v1(&pulse_frame, session, anchor)
        .expect("canonical pulse passes production signature validation");
    assert_eq!(link.height, pulse.height);
    assert_eq!(link.pulse_id, pulse.pulse_id);
    assert!(matches!(
        decode_global_threshold_beacon_session_v1(&pulse_frame, &expected),
        Err(GlobalThresholdBeaconError::InvalidEncoding)
    ));
    assert!(matches!(
        decode_finalized_global_threshold_beacon_pulse_v1(&session_frame, session, anchor),
        Err(GlobalThresholdBeaconError::InvalidEncoding)
    ));
    assert!(
        decode_global_threshold_beacon_session_v1(
            &session_frame[..session_frame.len() - 1],
            &expected
        )
        .is_err()
    );
    assert!(
        decode_finalized_global_threshold_beacon_pulse_v1(
            &pulse_frame[..pulse_frame.len() - 1],
            session,
            anchor
        )
        .is_err()
    );
    let mut substituted = pulse;
    substituted.seed[0] ^= 1;
    let altered =
        norito::encode_canonical(&substituted).expect("encode altered pulse with valid frame");
    assert!(decode_finalized_global_threshold_beacon_pulse_v1(&altered, session, anchor).is_err());
}
#[test]
fn threshold_beacon_canonical_decoders_reject_trailing_wire_data() {
    let (session, expected) = validated_threshold_session();
    let mut encoded_session = norito::to_bytes(session.record()).expect("encode session");
    encoded_session.push(0);
    assert!(matches!(
        decode_global_threshold_beacon_session_v1(&encoded_session, &expected),
        Err(GlobalThresholdBeaconError::InvalidEncoding)
            | Err(GlobalThresholdBeaconError::NonCanonicalEncoding)
    ));

    let (pulse, _cursor, anchor) = pulse_fixture(&session);
    let mut encoded_pulse = norito::to_bytes(&pulse).expect("encode pulse");
    encoded_pulse.push(0);
    assert!(matches!(
        decode_finalized_global_threshold_beacon_pulse_v1(&encoded_pulse, &session, anchor,),
        Err(GlobalThresholdBeaconError::InvalidEncoding)
            | Err(GlobalThresholdBeaconError::NonCanonicalEncoding)
    ));
}
