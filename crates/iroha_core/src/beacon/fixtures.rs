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
    GlobalThresholdBeaconDkgSessionV1 {
        version: GLOBAL_THRESHOLD_BEACON_VERSION_V1,
        network_id: beacon_fixture_network_id(0x81),
        session_id: [0x22; 32],
        roster_hash: [0x33; 32],
        committee_size: 4,
        threshold: 2,
        start_height: 1,
        sharing_end_height: 10,
        complaints_end_height: 20,
        responses_end_height: 30,
    }
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
    }
}

#[cfg(test)]
pub(super) fn adaptive_beacon_fixture() -> AdaptiveBeaconFixture {
    adaptive_beacon_fixture_for_session(adaptive_dkg_session_fixture())
}

#[cfg(any(test, feature = "iroha-core-tests"))]
pub(super) fn adaptive_beacon_fixture_for_session(
    dkg_session: GlobalThresholdBeaconDkgSessionV1,
) -> AdaptiveBeaconFixture {
    let crypto = AdaptiveGlobalThresholdBeaconDkgCryptoV1;
    let parameters = adaptive_beacon_parameters(&dkg_session).expect("adaptive parameters");
    let mut state = GlobalThresholdBeaconDkgStateV1::new(dkg_session, &crypto)
        .expect("valid adaptive DKG state");
    let mut rng = StdRng::from_seed([0x5A; 32]);
    let mut dealer_secrets = Vec::new();
    let mut dealer_commitments = Vec::new();
    for dealer_index in 1_u16..=dkg_session.committee_size {
        let (secret, commitment) =
            DasRenDealerSecret::generate_with_rng(&parameters, dealer_index, &mut rng)
                .expect("generate adaptive dealer");
        state
            .record_dealer_commitment(1, dealer_commitment_dto(&commitment), &crypto)
            .expect("verify adaptive dealer broadcast");
        dealer_secrets.push(secret);
        dealer_commitments.push(commitment);
    }
    let record = state
        .finalize(dkg_session.responses_end_height, &crypto)
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
    assert!(height > 4, "fixture pulse follows DKG finalization");
    let mut dkg_session = adaptive_dkg_session_fixture();
    dkg_session.network_id = network_id;
    dkg_session.sharing_end_height = 2;
    dkg_session.complaints_end_height = 3;
    dkg_session.responses_end_height = 4;
    dkg_session.session_id = Hash::new_from_chunks(&[
        b"iroha.beacon.world-test-session.v1\0",
        network_id.as_bytes(),
    ])
    .into();
    let fixture = adaptive_beacon_fixture_for_session(dkg_session);
    let anchor = GlobalThresholdBeaconChainAnchorV1 {
        height: height - 1,
        block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x88; 32])),
    };
    let mut aggregator =
        GlobalThresholdBeaconPulseAggregatorV1::new(fixture.session.clone(), height, anchor)
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
    let pulse = aggregator.finalize().expect("finalize world-test pulse");
    let mut key_record =
        FinalizedGlobalThresholdBeaconKeySessionRecordV1::new(fixture.session.record().clone())
            .expect("construct world-test key lifecycle");
    key_record
        .activate(fixture.session.record().adaptive_dkg.finalized_at_height)
        .expect("activate world-test key at DKG finalization");
    (key_record, pulse)
}
