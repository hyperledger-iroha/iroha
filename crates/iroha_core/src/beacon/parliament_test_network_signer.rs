//! Feature-isolated threshold-beacon signer for genuine Parliament network tests.
//!
//! This module is compiled only into the dedicated test daemon and its network
//! harness. A fixture key session is an ordinary signed all-edge DKG session of
//! exactly four validators with threshold two, whose dealer polynomials derive
//! from the session's public identifiers. That makes it test material and never
//! production custody.
//!
//! The harness holds every validator's signing key and runs the complete signed
//! ceremony: signed recipient keys, signed dealer broadcasts, all encrypted
//! private edges and their signed acceptances. Ordinary execution replays that
//! transcript when the lifecycle certificate installs it. The bootstrap session
//! has a fixture identifier under the genesis authority. A successor must be the
//! exact session of a certified committee-transition preparation, because Core
//! installs no other successor; the harness passes that session in.
//!
//! A daemon holds only its own seat. For the committed session it is asked to
//! sign for, it finds its seat among the session's signed recipient keys,
//! re-derives the fixture dealers of that session's identifiers, and derives its
//! one share against the committed transcript. It never rebuilds or compares a
//! transcript: sealing a private edge draws fresh KEM randomness, so two
//! ceremonies differ. A session with other dealers yields no share. The native
//! producer signs only for a seat whose custody its signer attests, so the daemon
//! attests exactly what it signs with: the share it derives for its own seat of
//! the committed session, and nothing else. The signer
//! never emits pulses, final signatures, seeds, reducer state, or validation
//! results; the ordinary Sumeragi aggregator still independently verifies every
//! returned share and constructs the unique pulse.

#[cfg(not(debug_assertions))]
compile_error!(
    "the feature-isolated Parliament beacon fixture signer cannot be compiled into optimized code"
);

#[cfg(any(test, feature = "iroha-core-tests"))]
use super::{
    FinalizedGlobalThresholdBeaconKeySessionRecordV1, fixtures,
    global_threshold_beacon_roster_hash_v1,
};
use super::{
    GlobalThresholdBeaconPartialSignerV1, InMemoryGlobalThresholdBeaconPartialSignerV1,
    ValidatedGlobalThresholdBeaconSessionV1, adaptive_beacon_parameters,
};
#[cfg(any(test, feature = "iroha-core-tests"))]
use iroha_allocation::AllocationBudget;
use iroha_crypto::{
    Hash,
    threshold_bls::{
        AdaptiveThresholdBlsParameters, AdaptiveThresholdBlsSecretShare, BeaconPurpose,
        DasRenDealerSecret, ValidatedDealerCommitment,
    },
};
#[cfg(any(test, feature = "iroha-core-tests"))]
use iroha_data_model::consensus::GLOBAL_THRESHOLD_BEACON_VERSION_V1;
use iroha_data_model::{
    NetworkId,
    consensus::{GlobalThresholdBeaconDkgSessionV1, GlobalThresholdBeaconPartialSignatureV1},
};
use iroha_model_base::peer::PeerId;
use rand::{SeedableRng as _, rngs::StdRng};
use thiserror::Error;

const EXACT_TEST_VALIDATORS_V1: u16 = 4;
const EXACT_TEST_THRESHOLD_V1: u16 = 2;
#[cfg(any(test, feature = "iroha-core-tests"))]
const TEST_SESSION_ID_DOMAIN_V1: &[u8] = b"iroha.test-network.parliament-beacon.session.v1\0";
const TEST_DEALER_SEED_DOMAIN_V1: &[u8] = b"iroha.test-network.parliament-beacon.dealers.v1\0";
#[cfg(any(test, feature = "iroha-core-tests"))]
const TEST_RECIPIENT_SEED_DOMAIN_V1: &[u8] =
    b"iroha.test-network.parliament-beacon.recipients.v1\0";

/// Closed construction failure for the feature-isolated Parliament beacon fixture.
#[derive(Debug, Error)]
pub enum TestNetworkParliamentBeaconSignerErrorV1 {
    /// The fixture is intentionally limited to the exact four-validator corridor.
    #[error("test Parliament beacon requires exactly four distinct validators")]
    InvalidRoster,
    /// The configured local peer does not occupy exactly one frozen roster seat.
    #[error("local test Parliament beacon seat is not exact")]
    InvalidLocalSeat,
    /// The session is not a four-seat, threshold-two session of the signing roster.
    #[error("test Parliament beacon session is not an exact four-seat session of its roster")]
    InvalidSession,
    /// The deterministic dealer material or the derived share was rejected.
    #[error("test Parliament beacon cryptographic fixture is invalid")]
    InvalidCryptographicFixture,
    /// The caller's original session admission or canonical validation failed.
    #[error(transparent)]
    Session(#[from] super::GlobalThresholdBeaconSessionError),
}

/// The deterministic dealers of one fixture session, in dealer-index order.
struct DeterministicDealersV1 {
    parameters: AdaptiveThresholdBlsParameters<BeaconPurpose>,
    secrets: Vec<DasRenDealerSecret<BeaconPurpose>>,
    commitments: Vec<ValidatedDealerCommitment<BeaconPurpose>>,
}

fn validate_roster_v1(roster: &[PeerId]) -> Result<(), TestNetworkParliamentBeaconSignerErrorV1> {
    if roster.len() != usize::from(EXACT_TEST_VALIDATORS_V1)
        || roster
            .iter()
            .enumerate()
            .any(|(index, peer)| roster[index + 1..].contains(peer))
    {
        return Err(TestNetworkParliamentBeaconSignerErrorV1::InvalidRoster);
    }
    Ok(())
}

/// The fixture never deals another committee size or threshold.
fn validate_session_shape_v1(
    session: &GlobalThresholdBeaconDkgSessionV1,
) -> Result<(), TestNetworkParliamentBeaconSignerErrorV1> {
    if session.committee_size != EXACT_TEST_VALIDATORS_V1
        || session.threshold != EXACT_TEST_THRESHOLD_V1
    {
        return Err(TestNetworkParliamentBeaconSignerErrorV1::InvalidSession);
    }
    Ok(())
}

/// The bootstrap session of a fresh network: a fixture identifier under the
/// genesis authority generation, with windows that finalize at height four.
#[cfg(any(test, feature = "iroha-core-tests"))]
fn bootstrap_session_v1(
    network_id: NetworkId,
    roster: &[PeerId],
) -> Result<GlobalThresholdBeaconDkgSessionV1, TestNetworkParliamentBeaconSignerErrorV1> {
    validate_roster_v1(roster)?;
    let roster_hash = global_threshold_beacon_roster_hash_v1(roster);
    let mut session_id: [u8; 32] = Hash::new_from_chunks(&[
        TEST_SESSION_ID_DOMAIN_V1,
        network_id.as_bytes(),
        &roster_hash,
    ])
    .into();
    if session_id == [0; 32] {
        session_id[0] = 1;
    }
    Ok(GlobalThresholdBeaconDkgSessionV1 {
        version: GLOBAL_THRESHOLD_BEACON_VERSION_V1,
        network_id,
        session_id,
        attempt_id: session_id,
        authority_generation: 0,
        roster_hash,
        committee_size: EXACT_TEST_VALIDATORS_V1,
        threshold: EXACT_TEST_THRESHOLD_V1,
        start_height: 1,
        commitments_end_height: 2,
        deliveries_end_height: 3,
        acceptances_end_height: 4,
    })
}

/// The one dealer stream of a fixture session. Only the in-order dealer
/// generation draws from it, so the harness ceremony and every daemon agree.
fn deterministic_dealer_rng_v1(session: &GlobalThresholdBeaconDkgSessionV1) -> StdRng {
    StdRng::from_seed(
        Hash::new_from_chunks(&[
            TEST_DEALER_SEED_DOMAIN_V1,
            session.network_id.as_bytes(),
            &session.session_id,
            &session.roster_hash,
        ])
        .into(),
    )
}

fn deterministic_dealers_v1(
    session: &GlobalThresholdBeaconDkgSessionV1,
) -> Result<DeterministicDealersV1, TestNetworkParliamentBeaconSignerErrorV1> {
    validate_session_shape_v1(session)?;
    let parameters = adaptive_beacon_parameters(session)
        .map_err(|_| TestNetworkParliamentBeaconSignerErrorV1::InvalidCryptographicFixture)?;
    let mut rng = deterministic_dealer_rng_v1(session);
    let mut secrets = Vec::with_capacity(usize::from(session.committee_size));
    let mut commitments = Vec::with_capacity(usize::from(session.committee_size));
    for dealer_index in 1_u16..=session.committee_size {
        let (secret, commitment) =
            DasRenDealerSecret::generate_with_rng(&parameters, dealer_index, &mut rng).map_err(
                |_| TestNetworkParliamentBeaconSignerErrorV1::InvalidCryptographicFixture,
            )?;
        secrets.push(secret);
        commitments.push(commitment);
    }
    Ok(DeterministicDealersV1 {
        parameters,
        secrets,
        commitments,
    })
}

/// Run the complete signed all-edge ceremony of one exact fixture session.
///
/// `ordered_signing_keys` are the validators' consensus keys in exact roster
/// order. Only a harness that owns every validator can call this.
#[cfg(any(test, feature = "iroha-core-tests"))]
fn signed_ceremony_v1(
    session: GlobalThresholdBeaconDkgSessionV1,
    ordered_signing_keys: &[iroha_crypto::KeyPair],
    budget: &AllocationBudget,
) -> Result<
    FinalizedGlobalThresholdBeaconKeySessionRecordV1,
    TestNetworkParliamentBeaconSignerErrorV1,
> {
    let roster = ordered_signing_keys
        .iter()
        .map(|key| PeerId::new(key.public_key().clone()))
        .collect::<Vec<_>>();
    validate_roster_v1(&roster)?;
    validate_session_shape_v1(&session)?;
    if session.roster_hash != global_threshold_beacon_roster_hash_v1(&roster) {
        return Err(TestNetworkParliamentBeaconSignerErrorV1::InvalidSession);
    }
    let mut recipient_rng = StdRng::from_seed(
        Hash::new_from_chunks(&[
            TEST_RECIPIENT_SEED_DOMAIN_V1,
            session.network_id.as_bytes(),
            &session.session_id,
            &session.roster_hash,
        ])
        .into(),
    );
    let mut dealer_rng = deterministic_dealer_rng_v1(&session);
    let ceremony = fixtures::adaptive_beacon_fixture_with_rngs(
        session,
        ordered_signing_keys,
        budget,
        &mut recipient_rng,
        Some(&mut dealer_rng),
    );
    FinalizedGlobalThresholdBeaconKeySessionRecordV1::new(ceremony.session.record().clone(), budget)
        .map_err(TestNetworkParliamentBeaconSignerErrorV1::Session)
}

/// Build the exact unactivated bootstrap key record installed by the network test.
///
/// This exports public transcript material only. No private share or dealer
/// secret is returned, serialized, or logged. Each call seals fresh private
/// edges, so a harness builds the record once and installs those exact bytes.
///
/// # Errors
/// Rejects a roster that is not exactly four distinct validators, or a
/// transcript that the ordinary public validation refuses.
#[cfg(any(test, feature = "iroha-core-tests"))]
pub fn deterministic_parliament_beacon_key_record_v1(
    network_id: NetworkId,
    ordered_signing_keys: &[iroha_crypto::KeyPair],
    budget: &AllocationBudget,
) -> Result<
    FinalizedGlobalThresholdBeaconKeySessionRecordV1,
    TestNetworkParliamentBeaconSignerErrorV1,
> {
    let roster = ordered_signing_keys
        .iter()
        .map(|key| PeerId::new(key.public_key().clone()))
        .collect::<Vec<_>>();
    signed_ceremony_v1(
        bootstrap_session_v1(network_id, &roster)?,
        ordered_signing_keys,
        budget,
    )
}

/// Build the key record of one exact session, for a genuine rotation.
///
/// Core installs a successor only when it is the session of a certified
/// committee-transition preparation: its session and attempt identifiers, its
/// authority generation and its phase windows. The harness reads that session
/// from the chain and passes it here unchanged. This is a second complete
/// signed ceremony with its own dealers, not an alias for another key.
///
/// # Errors
/// Rejects a roster that is not exactly four distinct validators, a session of
/// another committee size, threshold or roster, or a transcript that the
/// ordinary public validation refuses.
#[cfg(any(test, feature = "iroha-core-tests"))]
pub fn deterministic_parliament_beacon_key_record_for_session_v1(
    session: GlobalThresholdBeaconDkgSessionV1,
    ordered_signing_keys: &[iroha_crypto::KeyPair],
    budget: &AllocationBudget,
) -> Result<
    FinalizedGlobalThresholdBeaconKeySessionRecordV1,
    TestNetworkParliamentBeaconSignerErrorV1,
> {
    signed_ceremony_v1(session, ordered_signing_keys, budget)
}

/// Runtime-only signer bound to one exact peer on one exact network.
///
/// The object stores no scalar share. For the committed session it is asked to
/// sign for, it re-derives the feature-only dealers of that session, derives
/// the one share of its seat against the session's own transcript, produces one
/// partial signature, and drops the zeroizing temporary secret material.
pub struct TestNetworkParliamentBeaconPartialSignerV1 {
    network_id: NetworkId,
    local_peer: PeerId,
    emit_invalid_outbound: bool,
}

impl TestNetworkParliamentBeaconPartialSignerV1 {
    /// Bind the provider to the exact network and local peer.
    ///
    /// `ordered_roster` is the daemon's configured validator roster. It only
    /// guards the launch: the fixture corridor has exactly four validators and
    /// the local peer is one of them. The seat of a session comes from that
    /// session's own signed recipient keys.
    ///
    /// # Errors
    /// Rejects a roster that is not exactly four distinct validators, or a
    /// local peer that does not occupy exactly one seat.
    pub fn try_new(
        network_id: NetworkId,
        ordered_roster: &[PeerId],
        local_peer: &PeerId,
    ) -> Result<Self, TestNetworkParliamentBeaconSignerErrorV1> {
        validate_roster_v1(ordered_roster)?;
        if !ordered_roster.contains(local_peer) {
            return Err(TestNetworkParliamentBeaconSignerErrorV1::InvalidLocalSeat);
        }
        Ok(Self {
            network_id,
            local_peer: local_peer.clone(),
            emit_invalid_outbound: false,
        })
    }

    /// Mark this feature-only provider as an adversarial signer.
    ///
    /// It attests its genuine seat custody but returns a share whose signature
    /// it corrupted after signing. The native producer verifies its own share
    /// before retaining or broadcasting it, so the faulty seat contributes
    /// nothing, and receivers reject the same share through their ordinary
    /// ingress verifier. Protocol code carries no fault-injection hook.
    #[must_use]
    pub fn with_deliberately_invalid_outbound(mut self) -> Self {
        self.emit_invalid_outbound = true;
        self
    }

    /// Find this peer's seat in a committed session of this network.
    ///
    /// The session's signed recipient keys name each seat's validator, so the
    /// seat follows the committed roster order, whatever the local configuration
    /// lists. A session of another network or shape, or one without exactly one
    /// seat for this peer, gets no share.
    fn committed_seat_v1(
        &self,
        session: &ValidatedGlobalThresholdBeaconSessionV1,
    ) -> Result<(GlobalThresholdBeaconDkgSessionV1, u16), String> {
        let record = session.record();
        let dkg_session = record.adaptive_dkg.session;
        if record.network_id != self.network_id
            || dkg_session.network_id != self.network_id
            || validate_session_shape_v1(&dkg_session).is_err()
        {
            return Err("test Parliament beacon session binding differs".to_owned());
        }
        let mut seats = record
            .adaptive_dkg
            .recipient_keys
            .iter()
            .filter(|key| key.validator == self.local_peer)
            .map(|key| key.recipient_index);
        match (seats.next(), seats.next()) {
            (Some(seat), None) => Ok((dkg_session, seat)),
            _ => Err("test Parliament beacon session has no exact local seat".to_owned()),
        }
    }

    /// The signer of this daemon's own seat of the committed `session`: its
    /// fixture share, derived from the session's identifiers and validated
    /// against the committed transcript.
    fn local_seat_signer_v1(
        &self,
        session: &ValidatedGlobalThresholdBeaconSessionV1,
    ) -> Result<InMemoryGlobalThresholdBeaconPartialSignerV1, String> {
        let (dkg_session, local_signer_index) = self.committed_seat_v1(session)?;
        let dealers = deterministic_dealers_v1(&dkg_session)
            .map_err(|_| "test Parliament beacon fixture is unavailable".to_owned())?;
        let private_contributions = dealers
            .secrets
            .iter()
            .zip(&dealers.commitments)
            .map(|(secret, dealer)| {
                secret.private_share(&dealers.parameters, dealer, local_signer_index)
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| "test Parliament beacon share derivation failed".to_owned())?;
        // The committed transcript is the authority: a session whose dealers are
        // not this fixture's refuses the derived contributions here.
        let share = AdaptiveThresholdBlsSecretShare::from_dealer_shares(
            session.transcript(),
            &private_contributions,
        )
        .map_err(|_| "test Parliament beacon share validation failed".to_owned())?;
        InMemoryGlobalThresholdBeaconPartialSignerV1::from_validated_share(session.clone(), share)
            .map_err(|_| "test Parliament beacon signer import failed".to_owned())
    }
}

impl GlobalThresholdBeaconPartialSignerV1 for TestNetworkParliamentBeaconPartialSignerV1 {
    fn attest_partial_signing_capability(
        &self,
        session: &ValidatedGlobalThresholdBeaconSessionV1,
        expected_signer_index: u16,
    ) -> Result<
        super::GlobalThresholdBeaconPartialSigningCapabilityV1,
        super::GlobalThresholdBeaconCapabilityErrorV1,
    > {
        // The custody attested is the custody signed with: the fixture share of
        // this daemon's own seat, validated against the committed transcript.
        self.local_seat_signer_v1(session)
            .map_err(|_| super::GlobalThresholdBeaconCapabilityErrorV1::NotOwned)?
            .attest_partial_signing_capability(session, expected_signer_index)
    }

    fn sign_partial(
        &self,
        session: &ValidatedGlobalThresholdBeaconSessionV1,
        payload: &[u8],
    ) -> Result<GlobalThresholdBeaconPartialSignatureV1, String> {
        let mut partial = self
            .local_seat_signer_v1(session)?
            .sign_partial(session, payload)?;
        if self.emit_invalid_outbound {
            partial.signature_share[0] ^= 1;
        }
        Ok(partial)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::beacon::{
        GlobalThresholdBeaconSessionBindingV1, adaptive_partial_signature_from_dto_v1,
        fixtures::fixture_budget, validate_global_threshold_beacon_session_v1,
    };
    use iroha_crypto::{Algorithm, HashOf, KeyPair};
    use iroha_data_model::block::BlockHeader;

    fn network_id() -> NetworkId {
        NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
            Hash::prehashed([0x71; 32]),
        ))
    }

    fn signing_keys() -> Vec<KeyPair> {
        (1_u8..=4)
            .map(|tag| {
                KeyPair::try_from_seed(vec![tag; 32], Algorithm::BlsNormal)
                    .expect("derive test BLS peer key")
            })
            .collect()
    }

    fn roster(keys: &[KeyPair]) -> Vec<PeerId> {
        keys.iter()
            .map(|key| PeerId::new(key.public_key().clone()))
            .collect()
    }

    /// Replay one installed record exactly as ordinary execution does.
    fn validated(
        record: &FinalizedGlobalThresholdBeaconKeySessionRecordV1,
    ) -> ValidatedGlobalThresholdBeaconSessionV1 {
        let binding = GlobalThresholdBeaconSessionBindingV1 {
            network_id: record.session.network_id,
            session_id: record.session.session_id,
            roster_hash: record.session.roster_hash,
            transcript_hash: record.session.transcript_hash,
        };
        validate_global_threshold_beacon_session_v1(&record.session, &binding, &fixture_budget())
            .expect("replay the complete signed fixture transcript")
    }

    fn seat_signer(
        roster: &[PeerId],
        local_peer: &PeerId,
    ) -> TestNetworkParliamentBeaconPartialSignerV1 {
        TestNetworkParliamentBeaconPartialSignerV1::try_new(network_id(), roster, local_peer)
            .expect("bind exact test seat")
    }

    fn assert_every_seat_signs(
        session: &ValidatedGlobalThresholdBeaconSessionV1,
        roster: &[PeerId],
        payload: &[u8],
    ) {
        for peer in roster {
            let partial = seat_signer(roster, peer)
                .sign_partial(session, payload)
                .expect("produce proof-valid share");
            let partial =
                adaptive_partial_signature_from_dto_v1(&partial).expect("decode partial share");
            session
                .transcript()
                .verify_partial_signature(payload, &partial)
                .expect("independently verify test share");
        }
    }

    /// The session a certified committee-transition preparation would name: not a
    /// fixture identifier, a later authority generation and later phase windows.
    fn successor_session(roster: &[PeerId]) -> GlobalThresholdBeaconDkgSessionV1 {
        GlobalThresholdBeaconDkgSessionV1 {
            version: GLOBAL_THRESHOLD_BEACON_VERSION_V1,
            network_id: network_id(),
            session_id: [0xA7; 32],
            attempt_id: [0xA8; 32],
            authority_generation: 1,
            roster_hash: global_threshold_beacon_roster_hash_v1(roster),
            committee_size: EXACT_TEST_VALIDATORS_V1,
            threshold: EXACT_TEST_THRESHOLD_V1,
            start_height: 130,
            commitments_end_height: 140,
            deliveries_end_height: 150,
            acceptances_end_height: 160,
        }
    }

    #[test]
    fn installed_record_is_a_complete_signed_all_edge_transcript() {
        let keys = signing_keys();
        let record =
            deterministic_parliament_beacon_key_record_v1(network_id(), &keys, &fixture_budget())
                .expect("build the signed fixture ceremony");
        let session = &record.session;
        assert_eq!(session.committee_size, 4);
        assert_eq!(session.threshold, EXACT_TEST_THRESHOLD_V1);
        assert_eq!(
            session.roster_hash,
            global_threshold_beacon_roster_hash_v1(&roster(&keys))
        );
        assert_eq!(session.adaptive_dkg.session.authority_generation, 0);
        assert_eq!(session.adaptive_dkg.recipient_keys.len(), 4);
        assert_eq!(session.adaptive_dkg.dealer_commitments.len(), 4);
        assert_eq!(session.adaptive_dkg.encrypted_shares.len(), 16);
        assert_eq!(session.adaptive_dkg.share_acceptances.len(), 16);
        assert!(
            session
                .adaptive_dkg
                .dealer_commitments
                .iter()
                .all(|dealer| !dealer.signature.payload().is_empty()),
            "every dealer broadcast carries its validator's signature",
        );
        // The record replays through the ordinary public validation.
        let _ = validated(&record);
    }

    #[test]
    fn exact_seat_signer_matches_the_public_fixture_and_rejects_foreign_bindings() {
        let keys = signing_keys();
        let roster = roster(&keys);
        let record =
            deterministic_parliament_beacon_key_record_v1(network_id(), &keys, &fixture_budget())
                .expect("derive public fixture");
        let validated = validated(&record);
        let payload = b"exact feature-isolated test payload";
        assert_every_seat_signs(&validated, &roster, payload);

        // A peer of another corridor has no seat among the signed recipient keys.
        let outsider = PeerId::new(
            KeyPair::try_from_seed(vec![9; 32], Algorithm::BlsNormal)
                .expect("derive outsider key")
                .public_key()
                .clone(),
        );
        let mut foreign = roster.clone();
        foreign[3] = outsider.clone();
        let signer =
            TestNetworkParliamentBeaconPartialSignerV1::try_new(network_id(), &foreign, &outsider)
                .expect("bind a seat of a different roster");
        assert_eq!(
            signer.sign_partial(&validated, payload),
            Err("test Parliament beacon session has no exact local seat".to_owned())
        );

        let foreign_network = NetworkId::from_genesis_hash(
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x73; 32])),
        );
        let signer = TestNetworkParliamentBeaconPartialSignerV1::try_new(
            foreign_network,
            &roster,
            &roster[0],
        )
        .expect("bind exact seat to a different network");
        assert_eq!(
            signer.sign_partial(&validated, payload),
            Err("test Parliament beacon session binding differs".to_owned())
        );
    }

    /// The native producer signs only for a seat whose custody its signer attests.
    #[test]
    fn the_seat_attests_exactly_the_custody_it_signs_with() {
        use crate::beacon::GlobalThresholdBeaconCapabilityErrorV1 as CapabilityError;

        let keys = signing_keys();
        let roster = roster(&keys);
        let record =
            deterministic_parliament_beacon_key_record_v1(network_id(), &keys, &fixture_budget())
                .expect("derive public fixture");
        let validated = validated(&record);
        let payload = b"attested custody is signing custody";
        let seats = u16::try_from(roster.len()).expect("four seats");
        for peer in &roster {
            let signer = seat_signer(&roster, peer);
            let seat = signer
                .sign_partial(&validated, payload)
                .expect("own seat signs")
                .signer_index;
            let capability = signer
                .attest_partial_signing_capability(&validated, seat)
                .expect("own seat attests");
            assert!(capability.matches(&validated, seat));
            // Another seat of the same session is not this daemon's custody, and a seat
            // outside the transcript is no request at all.
            assert_eq!(
                signer.attest_partial_signing_capability(&validated, seat % seats + 1),
                Err(CapabilityError::NotOwned)
            );
            assert_eq!(
                signer.attest_partial_signing_capability(&validated, seats + 1),
                Err(CapabilityError::InvalidRequest)
            );
        }

        // A peer without a seat among the signed recipient keys owns nothing.
        let outsider = PeerId::new(
            KeyPair::try_from_seed(vec![9; 32], Algorithm::BlsNormal)
                .expect("derive outsider key")
                .public_key()
                .clone(),
        );
        let mut foreign = roster.clone();
        foreign[3] = outsider.clone();
        let signer =
            TestNetworkParliamentBeaconPartialSignerV1::try_new(network_id(), &foreign, &outsider)
                .expect("bind a seat of a different roster");
        for seat in 1..=seats {
            assert_eq!(
                signer.attest_partial_signing_capability(&validated, seat),
                Err(CapabilityError::NotOwned)
            );
        }
    }

    /// The seat follows the committed roster order, not the local configuration.
    #[test]
    fn the_seat_comes_from_the_committed_recipient_keys() {
        let keys = signing_keys();
        let roster = roster(&keys);
        let record =
            deterministic_parliament_beacon_key_record_v1(network_id(), &keys, &fixture_budget())
                .expect("derive public fixture");
        let validated = validated(&record);
        let mut reordered = roster.clone();
        reordered.reverse();
        let payload = b"configured order differs from the committed order";
        for peer in &roster {
            let partial = seat_signer(&reordered, peer)
                .sign_partial(&validated, payload)
                .expect("the committed session names the seat");
            let partial =
                adaptive_partial_signature_from_dto_v1(&partial).expect("decode partial share");
            validated
                .transcript()
                .verify_partial_signature(payload, &partial)
                .expect("the share opens the committed seat");
        }
    }

    #[test]
    fn exact_seat_signer_supports_a_certified_successor_session() {
        let keys = signing_keys();
        let roster = roster(&keys);
        let initial =
            deterministic_parliament_beacon_key_record_v1(network_id(), &keys, &fixture_budget())
                .expect("derive initial public fixture");
        let successor = deterministic_parliament_beacon_key_record_for_session_v1(
            successor_session(&roster),
            &keys,
            &fixture_budget(),
        )
        .expect("derive successor public fixture");
        assert_ne!(initial.session.session_id, successor.session.session_id);
        assert_eq!(successor.session.session_id, [0xA7; 32]);
        assert_eq!(
            successor.session.adaptive_dkg.session,
            successor_session(&roster),
            "the successor keeps the exact certified session",
        );
        assert_ne!(
            initial.session.transcript_hash,
            successor.session.transcript_hash
        );
        assert_ne!(
            initial.session.group_public_key, successor.session.group_public_key,
            "the successor is a second key, not an alias of the initial one",
        );
        let payload = b"exact feature-isolated successor payload";
        assert_every_seat_signs(&validated(&successor), &roster, payload);
        assert_every_seat_signs(&validated(&initial), &roster, payload);
    }

    /// A daemon never rebuilds the ceremony: sealing a private edge draws fresh
    /// KEM randomness, so no two ceremonies share a transcript. The seat share
    /// depends only on the dealers, so one signer serves either transcript.
    #[test]
    fn seat_signer_follows_the_committed_transcript_not_a_rebuilt_one() {
        let keys = signing_keys();
        let roster = roster(&keys);
        let first =
            deterministic_parliament_beacon_key_record_v1(network_id(), &keys, &fixture_budget())
                .expect("first signed ceremony");
        let second =
            deterministic_parliament_beacon_key_record_v1(network_id(), &keys, &fixture_budget())
                .expect("second signed ceremony");
        assert_eq!(first.session.session_id, second.session.session_id);
        assert_eq!(
            first.session.group_public_key, second.session.group_public_key,
            "both ceremonies run the same deterministic dealers",
        );
        assert_ne!(
            first.session.transcript_hash, second.session.transcript_hash,
            "each ceremony seals its private edges with fresh randomness",
        );
        let payload = b"committed transcript payload";
        assert_every_seat_signs(&validated(&first), &roster, payload);
        assert_every_seat_signs(&validated(&second), &roster, payload);
    }

    /// A session that reuses the fixture identifiers with other dealers is not
    /// the fixture: the derived contributions do not open its public shares.
    #[test]
    fn seat_signer_refuses_a_fixture_identifier_with_foreign_dealers() {
        let keys = signing_keys();
        let roster = roster(&keys);
        let session =
            bootstrap_session_v1(network_id(), &roster).expect("exact fixture session identifiers");
        let mut foreign_rng = StdRng::from_seed([0x5C; 32]);
        let foreign = fixtures::adaptive_beacon_fixture_with_rngs(
            session,
            &keys,
            &fixture_budget(),
            &mut foreign_rng,
            None,
        );
        assert_eq!(
            seat_signer(&roster, &roster[0]).sign_partial(&foreign.session, b"foreign dealers"),
            Err("test Parliament beacon share validation failed".to_owned())
        );
    }

    #[test]
    fn roster_seat_and_session_binding_are_exact() {
        let keys = signing_keys();
        let roster = roster(&keys);
        assert!(matches!(
            deterministic_parliament_beacon_key_record_v1(
                network_id(),
                &keys[..3],
                &fixture_budget()
            ),
            Err(TestNetworkParliamentBeaconSignerErrorV1::InvalidRoster)
        ));
        let mut duplicated = roster.clone();
        duplicated[3] = duplicated[0].clone();
        assert!(matches!(
            TestNetworkParliamentBeaconPartialSignerV1::try_new(
                network_id(),
                &duplicated,
                &roster[0]
            ),
            Err(TestNetworkParliamentBeaconSignerErrorV1::InvalidRoster)
        ));
        let outsider = PeerId::new(
            KeyPair::try_from_seed(vec![9; 32], Algorithm::BlsNormal)
                .expect("derive outsider key")
                .public_key()
                .clone(),
        );
        assert!(matches!(
            TestNetworkParliamentBeaconPartialSignerV1::try_new(network_id(), &roster, &outsider),
            Err(TestNetworkParliamentBeaconSignerErrorV1::InvalidLocalSeat)
        ));

        // A successor of another threshold or roster is not a fixture session.
        let mut wide_threshold = successor_session(&roster);
        wide_threshold.threshold = 3;
        assert!(matches!(
            deterministic_parliament_beacon_key_record_for_session_v1(
                wide_threshold,
                &keys,
                &fixture_budget()
            ),
            Err(TestNetworkParliamentBeaconSignerErrorV1::InvalidSession)
        ));
        let mut foreign_roster = successor_session(&roster);
        foreign_roster.roster_hash = [0x44; 32];
        assert!(matches!(
            deterministic_parliament_beacon_key_record_for_session_v1(
                foreign_roster,
                &keys,
                &fixture_budget()
            ),
            Err(TestNetworkParliamentBeaconSignerErrorV1::InvalidSession)
        ));
    }

    #[test]
    fn invalid_outbound_mode_attests_its_seat_but_signs_a_share_that_never_verifies() {
        let keys = signing_keys();
        let roster = roster(&keys);
        let record =
            deterministic_parliament_beacon_key_record_v1(network_id(), &keys, &fixture_budget())
                .expect("derive public fixture");
        let validated = validated(&record);
        let honest = seat_signer(&roster, &roster[0]);
        let faulty = seat_signer(&roster, &roster[0]).with_deliberately_invalid_outbound();
        let payload = b"feature-isolated invalid-outbound fixture";
        let verifies = |partial: &GlobalThresholdBeaconPartialSignatureV1| {
            adaptive_partial_signature_from_dto_v1(partial).is_ok_and(|decoded| {
                validated
                    .transcript()
                    .verify_partial_signature(payload, &decoded)
                    .is_ok()
            })
        };
        let honest_share = honest
            .sign_partial(&validated, payload)
            .expect("the honest seat signs");
        assert!(verifies(&honest_share));
        // The faulty provider's custody is genuine: it attests the same seat.
        faulty
            .attest_partial_signing_capability(&validated, honest_share.signer_index)
            .expect("attest the genuine seat");
        let faulty_share = faulty
            .sign_partial(&validated, payload)
            .expect("the faulty provider still answers");
        assert_eq!(faulty_share.signer_index, honest_share.signer_index);
        assert!(
            !verifies(&faulty_share),
            "the producer must refuse the faulty share before retaining or broadcasting it"
        );
    }
}
