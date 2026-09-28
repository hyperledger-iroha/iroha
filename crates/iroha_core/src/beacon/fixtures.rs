//! Deterministic threshold-beacon fixtures shared by crate tests.

use super::*;

#[cfg(any(test, feature = "iroha-core-tests"))]
pub(super) fn beacon_fixture_network_id(marker: u8) -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
        Hash::prehashed([marker; Hash::LENGTH]),
    ))
}

#[cfg(any(test, feature = "iroha-core-tests"))]
pub(super) fn adaptive_dkg_session_fixture() -> GlobalThresholdBeaconDkgSessionV1 {
    let roster = adaptive_fixture_signing_keys(4)
        .into_iter()
        .map(|key| PeerId::new(key.public_key().clone()))
        .collect::<Vec<_>>();
    GlobalThresholdBeaconDkgSessionV1 {
        version: GLOBAL_THRESHOLD_BEACON_VERSION_V1,
        network_id: beacon_fixture_network_id(0x81),
        session_id: [0x22; 32],
        attempt_id: [0x22; 32],
        authority_generation: 0,
        roster_hash: global_threshold_beacon_roster_hash_v1(&roster),
        committee_size: 4,
        threshold: 2,
        start_height: 1,
        commitments_end_height: 10,
        deliveries_end_height: 20,
        acceptances_end_height: 30,
    }
}

#[cfg(any(test, feature = "iroha-core-tests"))]
pub(super) fn adaptive_fixture_signing_keys(seats: u16) -> Vec<iroha_crypto::KeyPair> {
    let mut keys = (1..=seats)
        .map(|marker| {
            iroha_crypto::KeyPair::try_from_seed(
                vec![u8::try_from(marker).expect("fixture seats fit u8"); 32],
                iroha_crypto::Algorithm::BlsNormal,
            )
            .expect("deterministic fixture BLS key")
        })
        .collect::<Vec<_>>();
    keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
    keys
}

#[cfg(any(test, feature = "iroha-core-tests"))]
pub(super) struct AdaptiveBeaconFixture {
    pub(super) session: ValidatedGlobalThresholdBeaconSessionV1,
    #[cfg(test)]
    pub(super) binding: GlobalThresholdBeaconSessionBindingV1,
    pub(super) parameters: AdaptiveThresholdBlsParameters<BeaconPurpose>,
    pub(super) dealer_secrets: Vec<DasRenDealerSecret<BeaconPurpose>>,
    pub(super) dealer_commitments: Vec<ValidatedDealerCommitment<BeaconPurpose>>,
}

#[cfg(any(test, feature = "iroha-core-tests"))]
fn dealer_commitment_dto(
    dealer: &ValidatedDealerCommitment<BeaconPurpose>,
) -> GlobalThresholdBeaconDkgDealerCommitmentV1 {
    GlobalThresholdBeaconDkgDealerCommitmentV1 {
        dealer_index: dealer.dealer_index(),
        coefficient_commitments: dealer
            .coefficients()
            .iter()
            .map(|coefficient| *coefficient.as_bytes())
            .collect(),
        constant_term_proof: GlobalThresholdBeaconDkgConstantProofV1 {
            commitment: *dealer.constant_proof().commitment_bytes(),
            response: *dealer.constant_proof().response_bytes(),
        },
        signature: iroha_crypto::Signature::from_bytes(&[]),
    }
}

#[cfg(test)]
pub(super) fn adaptive_beacon_fixture() -> AdaptiveBeaconFixture {
    adaptive_beacon_fixture_for_session(adaptive_dkg_session_fixture())
}

#[cfg(test)]
pub(super) fn adaptive_beacon_fixture_for_session(
    dkg_session: GlobalThresholdBeaconDkgSessionV1,
) -> AdaptiveBeaconFixture {
    adaptive_beacon_fixture_for_session_and_keys(
        dkg_session,
        &adaptive_fixture_signing_keys(dkg_session.committee_size),
    )
}

#[cfg(any(test, feature = "iroha-core-tests"))]
pub(super) fn adaptive_beacon_fixture_for_session_and_keys(
    dkg_session: GlobalThresholdBeaconDkgSessionV1,
    signing_keys: &[iroha_crypto::KeyPair],
) -> AdaptiveBeaconFixture {
    assert_eq!(signing_keys.len(), usize::from(dkg_session.committee_size));
    let roster = signing_keys
        .iter()
        .map(|key| PeerId::new(key.public_key().clone()))
        .collect::<Vec<_>>();
    assert_eq!(
        dkg_session.roster_hash,
        global_threshold_beacon_roster_hash_v1(&roster),
        "fixture DKG must bind the exact signing roster"
    );
    let crypto = AdaptiveGlobalThresholdBeaconDkgCryptoV1;
    let parameters = adaptive_beacon_parameters(&dkg_session).expect("adaptive parameters");
    let mut state = GlobalThresholdBeaconDkgStateV1::new(dkg_session, &crypto)
        .expect("valid adaptive DKG state");
    let mut rng = StdRng::from_seed([0x5A; 32]);
    let encryption_keys = (0..dkg_session.committee_size)
        .map(|_| {
            iroha_crypto::hybrid::HybridKeyPair::generate(&mut rng)
                .expect("fixture hybrid recipient key")
        })
        .collect::<Vec<_>>();
    let recipient_keys = signing_keys
        .iter()
        .zip(&encryption_keys)
        .enumerate()
        .map(|(offset, (signer, encryption))| {
            sign_global_threshold_beacon_dkg_recipient_key_v1(
                &dkg_session,
                u16::try_from(offset + 1).expect("fixture recipient index"),
                signer,
                encryption.public(),
            )
            .expect("signed fixture recipient key")
        })
        .collect::<Vec<_>>();
    for key in &recipient_keys {
        state
            .record_recipient_key(dkg_session.start_height, key.clone())
            .expect("register signed fixture encryption key");
    }
    let mut dealer_secrets = Vec::new();
    let mut dealer_commitments = Vec::new();
    let mut dealer_dtos = Vec::new();
    for dealer_index in 1_u16..=dkg_session.committee_size {
        let (secret, commitment) =
            DasRenDealerSecret::generate_with_rng(&parameters, dealer_index, &mut rng)
                .expect("generate adaptive dealer");
        let dto = sign_global_threshold_beacon_dkg_dealer_commitment_v1(
            &dkg_session,
            &recipient_keys[usize::from(dealer_index - 1)],
            &signing_keys[usize::from(dealer_index - 1)],
            dealer_commitment_dto(&commitment),
        )
        .expect("sign fixture dealer broadcast");
        state
            .record_dealer_commitment(dkg_session.start_height, dto.clone(), &crypto)
            .expect("verify adaptive dealer broadcast");
        dealer_secrets.push(secret);
        dealer_commitments.push(commitment);
        dealer_dtos.push(dto);
    }
    for (dealer_offset, (secret, commitment)) in
        dealer_secrets.iter().zip(&dealer_commitments).enumerate()
    {
        for (recipient_offset, recipient_key) in recipient_keys.iter().enumerate() {
            let recipient_index = u16::try_from(recipient_offset + 1).expect("fixture seat");
            let share = secret
                .private_share(&parameters, commitment, recipient_index)
                .expect("fixture private contribution");
            let edge = seal_global_threshold_beacon_dkg_private_edge_v1(
                &dkg_session,
                &recipient_keys[dealer_offset],
                &signing_keys[dealer_offset],
                &dealer_dtos[dealer_offset],
                recipient_key,
                &share,
                dkg_session.commitments_end_height,
            )
            .expect("fixture encrypted contribution");
            state
                .record_encrypted_share(dkg_session.commitments_end_height, edge)
                .expect("record encrypted fixture contribution");
        }
    }
    let edges = state
        .public_snapshot()
        .expect("fixture delivery snapshot")
        .encrypted_shares;
    for edge in &edges {
        let dealer_offset = usize::from(edge.dealer_index - 1);
        let recipient_offset = usize::from(edge.recipient_index - 1);
        let (_, acceptance) = accept_global_threshold_beacon_dkg_private_edge_v1(
            &dkg_session,
            &recipient_keys[dealer_offset],
            &recipient_keys[recipient_offset],
            &signing_keys[recipient_offset],
            encryption_keys[recipient_offset].secret(),
            &dealer_dtos[dealer_offset],
            edge,
            dkg_session.deliveries_end_height,
        )
        .expect("verify and accept fixture edge");
        state
            .record_share_acceptance(dkg_session.deliveries_end_height, acceptance)
            .expect("record fixture acceptance");
    }
    let record = state
        .finalize(dkg_session.acceptances_end_height, &crypto)
        .expect("finalize adaptive DKG")
        .clone();
    let binding = GlobalThresholdBeaconSessionBindingV1 {
        network_id: record.network_id,
        session_id: record.session_id,
        roster_hash: record.roster_hash,
        transcript_hash: record.transcript_hash,
    };
    let session = validate_global_threshold_beacon_session_v1(record, &binding)
        .expect("validate adaptive beacon transcript DTO");
    AdaptiveBeaconFixture {
        session,
        #[cfg(test)]
        binding,
        parameters,
        dealer_secrets,
        dealer_commitments,
    }
}

/// Shape-only native context for primitive/component tests. These values do not authenticate
/// any native State, block or scheduling epoch; authority tests must supply their actual source.
#[cfg(any(test, feature = "iroha-core-tests"))]
#[doc(hidden)]
pub fn pulse_context_fixture_v1() -> GlobalThresholdBeaconPulseContextV1 {
    GlobalThresholdBeaconPulseContextV1 {
        instance: [0xB1; 32],
        epoch: 0,
        epoch_context_id: [0xB2; 32],
        parent_consensus_hash: [0xB3; 32],
        parent_result: [0xB4; 32],
    }
}

/// Build one fully signed, proof-valid persisted beacon fixture.
#[cfg(any(test, feature = "iroha-core-tests"))]
#[doc(hidden)]
pub fn signed_persisted_pulse_fixture_for_world(
    network_id: NetworkId,
    height: u64,
) -> (
    FinalizedGlobalThresholdBeaconKeySessionRecordV1,
    FinalizedGlobalThresholdBeaconPulseV1,
) {
    let anchor = GlobalThresholdBeaconChainAnchorV1 {
        height: height
            .checked_sub(1)
            .expect("positive fixture pulse height"),
        block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x88; 32])),
    };
    let (key_record, mut pulses) = signed_pulses_fixture_with_binding(
        network_id,
        &adaptive_fixture_signing_keys(4),
        &[(anchor, pulse_context_fixture_v1())],
    );
    (
        key_record,
        pulses.pop().expect("one requested fixture pulse"),
    )
}

/// Produce real threshold signatures for exact native parent anchors and roster.
#[cfg(test)]
pub(crate) fn signed_pulses_fixture_for_roster_and_anchors(
    network_id: NetworkId,
    signing_keys: &[iroha_crypto::KeyPair],
    anchors: &[(
        GlobalThresholdBeaconChainAnchorV1,
        GlobalThresholdBeaconPulseContextV1,
    )],
) -> (
    FinalizedGlobalThresholdBeaconKeySessionRecordV1,
    Vec<FinalizedGlobalThresholdBeaconPulseV1>,
) {
    assert_eq!(
        signing_keys.len(),
        4,
        "the adaptive fixture has four DKG seats"
    );
    signed_pulses_fixture_with_binding(network_id, signing_keys, anchors)
}

#[cfg(any(test, feature = "iroha-core-tests"))]
fn signed_pulses_fixture_with_binding(
    network_id: NetworkId,
    signing_keys: &[iroha_crypto::KeyPair],
    anchors: &[(
        GlobalThresholdBeaconChainAnchorV1,
        GlobalThresholdBeaconPulseContextV1,
    )],
) -> (
    FinalizedGlobalThresholdBeaconKeySessionRecordV1,
    Vec<FinalizedGlobalThresholdBeaconPulseV1>,
) {
    let mut dkg_session = adaptive_dkg_session_fixture();
    dkg_session.network_id = network_id;
    dkg_session.roster_hash = global_threshold_beacon_roster_hash_v1(
        &signing_keys
            .iter()
            .map(|key| PeerId::new(key.public_key().clone()))
            .collect::<Vec<_>>(),
    );
    dkg_session.commitments_end_height = 2;
    dkg_session.deliveries_end_height = 3;
    dkg_session.acceptances_end_height = 4;
    dkg_session.session_id = Hash::new_from_chunks(&[
        b"iroha.beacon.world-test-session.v1\0",
        network_id.as_bytes(),
    ])
    .into();
    let fixture = adaptive_beacon_fixture_for_session_and_keys(dkg_session, signing_keys);
    let mut pulses = Vec::new();
    for (anchor, context) in anchors {
        let height = anchor
            .height
            .checked_add(1)
            .expect("fixture pulse height fits");
        assert!(height > 4, "fixture pulse follows DKG finalization");
        let mut aggregator = GlobalThresholdBeaconPulseAggregatorV1::new(
            fixture.session.clone(),
            height,
            *anchor,
            *context,
        )
        .expect("open exact world-test pulse reducer");
        let payload = aggregator.payload().to_vec();
        for recipient_index in 1_u16..=fixture.session.transcript.session().threshold() {
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
            aggregator
                .accept_partial(global_threshold_beacon_partial_signature_dto_v1(
                    &signing_share
                        .sign_payload(&fixture.session.transcript, &payload)
                        .expect("sign exact world-test pulse payload"),
                ))
                .expect("accept proof-verified world-test partial");
        }
        pulses.push(aggregator.finalize().expect("finalize world-test pulse"));
    }
    let mut key_record =
        FinalizedGlobalThresholdBeaconKeySessionRecordV1::new(fixture.session.record().clone())
            .expect("construct world-test key lifecycle");
    key_record
        .activate(fixture.session.record().adaptive_dkg.finalized_at_height)
        .expect("activate world-test key at DKG finalization");
    (key_record, pulses)
}

/// Produce a complete verified test DKG and zeroizing providers for every exact seat.
#[cfg(test)]
pub(crate) fn prepared_session_and_signers_fixture_v1(
    dkg_session: GlobalThresholdBeaconDkgSessionV1,
) -> (
    ValidatedGlobalThresholdBeaconSessionV1,
    Vec<InMemoryGlobalThresholdBeaconPartialSignerV1>,
) {
    prepared_session_and_signers_fixture_for_keys_v1(
        dkg_session,
        &adaptive_fixture_signing_keys(dkg_session.committee_size),
    )
}

/// Produce providers bound to an exact authenticated test-chain committee.
#[cfg(test)]
pub(crate) fn prepared_session_and_signers_fixture_for_keys_v1(
    dkg_session: GlobalThresholdBeaconDkgSessionV1,
    keys: &[iroha_crypto::KeyPair],
) -> (
    ValidatedGlobalThresholdBeaconSessionV1,
    Vec<InMemoryGlobalThresholdBeaconPartialSignerV1>,
) {
    let fixture = adaptive_beacon_fixture_for_session_and_keys(dkg_session, keys);
    let signers = (1..=dkg_session.committee_size)
        .map(|recipient_index| {
            let contributions = fixture
                .dealer_secrets
                .iter()
                .zip(&fixture.dealer_commitments)
                .map(|(secret, dealer)| {
                    secret
                        .private_share(&fixture.parameters, dealer, recipient_index)
                        .expect("verified exact target DKG contribution")
                })
                .collect::<Vec<_>>();
            let share = AdaptiveThresholdBlsSecretShare::from_dealer_shares(
                &fixture.session.transcript,
                &contributions,
            )
            .expect("complete target private share");
            InMemoryGlobalThresholdBeaconPartialSignerV1::from_validated_share(
                fixture.session.clone(),
                share,
            )
            .expect("zeroizing provider for exact target seat")
        })
        .collect();
    (fixture.session, signers)
}

/// Build an exact signed all-edge DKG fixture and one private seat credential.
///
/// The roster consists of deterministic, real BLS fixture keys in canonical
/// public-key order. Production callers must use independently held seat keys.
#[cfg(any(test, feature = "iroha-core-tests"))]
#[doc(hidden)]
pub fn complete_beacon_dkg_fixture_for_seat_v1(
    network_id: NetworkId,
    session_id: [u8; 32],
    committee_size: u16,
    signer_index: u16,
) -> (GlobalThresholdBeaconKeySessionV1, Zeroizing<[[u8; 32]; 3]>) {
    assert!(signer_index > 0 && signer_index <= committee_size);
    let keys = adaptive_fixture_signing_keys(committee_size);
    let roster = keys
        .iter()
        .map(|key| PeerId::new(key.public_key().clone()))
        .collect::<Vec<_>>();
    let mut session = adaptive_dkg_session_fixture();
    session.network_id = network_id;
    session.session_id = session_id;
    session.attempt_id = session_id;
    session.roster_hash = global_threshold_beacon_roster_hash_v1(&roster);
    session.committee_size = committee_size;
    session.threshold = (committee_size - 1) / 3 + 1;
    session.commitments_end_height = 2;
    session.deliveries_end_height = 3;
    session.acceptances_end_height = 4;
    complete_beacon_dkg_fixture_for_exact_session_v1(session, signer_index)
}

/// Build a fully signed test DKG for one exact session and deterministic roster.
///
/// Every call for the same session returns the same transcript within one
/// process (see [`complete_beacon_dkg_fixture_v1`]).
#[cfg(any(test, feature = "iroha-core-tests"))]
#[doc(hidden)]
pub fn complete_beacon_dkg_fixture_for_exact_session_v1(
    session: GlobalThresholdBeaconDkgSessionV1,
    signer_index: u16,
) -> (GlobalThresholdBeaconKeySessionV1, Zeroizing<[[u8; 32]; 3]>) {
    assert!(signer_index > 0 && signer_index <= session.committee_size);
    let fixture = complete_beacon_dkg_fixture_v1(session);
    (
        fixture.record.clone(),
        Zeroizing::new(*fixture.seat_components[usize::from(signer_index - 1)]),
    )
}

/// One complete signed all-edge test DKG: its public record and every seat's share.
#[cfg(any(test, feature = "iroha-core-tests"))]
struct CompleteBeaconDkgFixtureV1 {
    record: GlobalThresholdBeaconKeySessionV1,
    seat_components: Vec<Zeroizing<[[u8; 32]; 3]>>,
}

/// Build, or reuse, the complete test DKG of one exact session.
///
/// Sealing a private edge draws fresh KEM randomness, so rebuilding a session
/// would yield another transcript. Tests that rebuild "the same" fixture (a
/// restarted provider or a replacement credential) must see one transcript, so
/// the first completed build of each session is kept for the process.
#[cfg(any(test, feature = "iroha-core-tests"))]
fn complete_beacon_dkg_fixture_v1(
    session: GlobalThresholdBeaconDkgSessionV1,
) -> std::sync::Arc<CompleteBeaconDkgFixtureV1> {
    use std::{
        collections::BTreeMap,
        sync::{Arc, LazyLock, Mutex, PoisonError},
    };
    type Fixtures =
        Mutex<BTreeMap<GlobalThresholdBeaconDkgSessionV1, Arc<CompleteBeaconDkgFixtureV1>>>;
    static FIXTURES: LazyLock<Fixtures> = LazyLock::new(Fixtures::default);
    if let Some(built) = FIXTURES
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(&session)
    {
        return Arc::clone(built);
    }
    // Build outside the lock; a concurrent build of the same session loses to the first insert.
    let keys = adaptive_fixture_signing_keys(session.committee_size);
    let fixture = adaptive_beacon_fixture_for_session_and_keys(session, &keys);
    let seat_components = (1..=session.committee_size)
        .map(|signer_index| {
            let shares = fixture
                .dealer_secrets
                .iter()
                .zip(&fixture.dealer_commitments)
                .map(|(secret, dealer)| {
                    secret
                        .private_share(&fixture.parameters, dealer, signer_index)
                        .expect("verified fixture contribution")
                })
                .collect::<Vec<_>>();
            AdaptiveThresholdBlsSecretShare::from_dealer_shares(
                &fixture.session.transcript,
                &shares,
            )
            .expect("complete fixture seat share")
            .into_components_for_runtime_custody()
        })
        .collect();
    let built = Arc::new(CompleteBeaconDkgFixtureV1 {
        record: fixture.session.record().clone(),
        seat_components,
    });
    Arc::clone(
        FIXTURES
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .entry(session)
            .or_insert(built),
    )
}
