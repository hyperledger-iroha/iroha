// Test body included from the parent module to keep its production source budget bounded.
use super::super::authentication_challenge;
use super::*;
use crate::vega::MaskedRelaxedRandomErrorV1;

const LINEAR_TEST_MODULI: [u64; 2] = [2_013_265_921, 1_811_939_329];
const LINEAR_TEST_ROOTS: [u64; 2] = [1_400_279_418, 677_356_115];

fn linear_test_profile() -> super::super::BgvProfile {
    super::super::BgvProfile {
        profile_id: [0x71; 32],
        ring_degree: 8,
        moduli: &LINEAR_TEST_MODULI,
        negacyclic_roots: &LINEAR_TEST_ROOTS,
        plaintext_modulus: super::super::PlaintextModulus::Tiny(17),
        error_eta: 2,
        hybrid_rns_decomposition: false,
        gadget_base_log: 8,
        gadget_digits: 8,
        max_ciphertext_bytes: 1 << 20,
        max_evaluated_key_bytes: 16 << 20,
        max_round_bytes: 16 << 20,
        max_share_bytes: 4 << 20,
        max_workspace_bytes: 16 << 20,
        max_work_units: 1 << 20,
    }
}

struct KatRandom {
    seed: Vec<u8>,
    counter: u64,
}

impl KatRandom {
    fn new(label: &[u8]) -> Self {
        Self {
            seed: label.to_vec(),
            counter: 0,
        }
    }
}

impl MaskedRelaxedRandomSourceV1 for KatRandom {
    fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), MaskedRelaxedRandomErrorV1> {
        let mut written = 0;
        while written < destination.len() {
            let mut frame = self.seed.clone();
            frame.extend_from_slice(&self.counter.to_be_bytes());
            let block = keccak256(&frame);
            let take = (destination.len() - written).min(block.len());
            destination[written..written + take].copy_from_slice(&block[..take]);
            written += take;
            self.counter = self.counter.wrapping_add(1);
        }
        Ok(())
    }
}

struct Fixture {
    roster: ZkAmsMkheGovernedActiveRosterV1,
    secrets: Vec<AuthenticationSecret>,
}

fn fixture(label: &[u8], epoch: u64) -> Fixture {
    let mut random = KatRandom::new(label);
    let mut secrets = (0..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)
        .map(|_| AuthenticationSecret::generate(&mut random).expect("authentication secret"))
        .collect::<Vec<_>>();
    secrets.sort_by_key(|secret| secret.party_id().expect("party"));
    let references: [&AuthenticationSecret; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1] = secrets
        .iter()
        .collect::<Vec<_>>()
        .try_into()
        .expect("eight secrets");
    let roster =
        assemble_governed_active_roster(epoch, references, &mut random).expect("governed roster");
    Fixture { roster, secrets }
}

fn contributions(
    fixture: &Fixture,
    round: ZkAmsMkheActiveRoundV1,
    transcript: [u8; 32],
    label: &[u8],
) -> Vec<ZkAmsMkheActiveContributionV1> {
    let mut random = KatRandom::new(label);
    fixture
        .secrets
        .iter()
        .enumerate()
        .map(|(index, secret)| {
            let mut payload_frame = label.to_vec();
            payload_frame.push(round.tag());
            payload_frame.extend_from_slice(&(index as u32).to_be_bytes());
            authenticate_active_contribution(
                &fixture.roster,
                transcript,
                round,
                index,
                keccak256(&payload_frame),
                secret,
                &mut random,
            )
            .expect("active contribution")
        })
        .collect()
}

#[test]
fn governed_roster_is_exactly_eight_ordered_key_bound_parties() {
    let fixture = fixture(b"active-roster-positive", 41);
    fixture.roster.validate().expect("valid roster");
    assert_eq!(fixture.roster.participants().len(), 8);
    assert_eq!(fixture.roster.epoch(), 41);
    assert_ne!(fixture.roster.roster_digest(), [0; 32]);
    for (participant, secret) in fixture.roster.participants().iter().zip(&fixture.secrets) {
        assert_eq!(participant.party(), secret.party_id().unwrap());
        assert_eq!(
            participant.authentication_public_key(),
            secret.public_key().unwrap()
        );
    }
}

#[test]
fn prepared_common_a_rejects_a_valid_roster_under_a_different_profile() {
    let fixture = fixture(b"prepared-common-a-profile-mismatch", 401);
    let transcript = keccak256(b"prepared-common-a-profile-mismatch-transcript");
    assert!(matches!(
        super::super::cpk_relation::prepare_active_collective_public_a_v1(
            &linear_test_profile(),
            &fixture.roster,
            transcript,
        ),
        Err(super::super::cpk_relation::ZkAmsMkheCpkRelationErrorV1::GovernedContext)
    ));
}

#[test]
#[ignore = "release-size native/prepared common-a parity exercise; isolated resource job only"]
fn prepared_common_a_is_byte_identical_to_the_native_whole_polynomial() {
    let fixture = fixture(b"prepared-common-a-release-parity", 402);
    let profile = release_profile_v1();
    let transcript = keccak256(b"prepared-common-a-release-parity-transcript");
    let native = derive_active_collective_public_a(&profile, &fixture.roster, transcript)
        .expect("native whole common-a");
    let prepared = super::super::cpk_relation::prepare_active_collective_public_a_v1(
        &profile,
        &fixture.roster,
        transcript,
    )
    .expect("prepared common-a");
    let mut remaining_candidates = u64::MAX;
    let mut coefficients = Vec::with_capacity(profile.ring_degree * profile.moduli.len());
    for limb in 0..profile.moduli.len() {
        coefficients.extend(
            prepared
                .derive_limb_budgeted_v1(limb, &mut remaining_candidates)
                .expect("prepared common-a limb"),
        );
    }
    let staged = super::super::RnsPolynomial::from_flat(&profile, coefficients)
        .expect("prepared whole common-a");
    assert_eq!(staged, native);
}

#[test]
fn every_public_witness_debug_representation_is_redacted() {
    let secret = [1_i64, -1, 0];
    let error = [2_i64, -2, 0];
    let public_key = ZkAmsMkheActiveCollectivePublicKeyWitnessV1 {
        secret: &secret,
        public_error: &error,
    };
    let round_one = ZkAmsMkheActiveRkgRoundOneWitnessV1 {
        secret: &secret,
        public_error: &error,
        ephemeral: &secret,
        error_zero: &error,
        error_one: &error,
    };
    let round_two = ZkAmsMkheActiveRkgRoundTwoWitnessV1 {
        round_one,
        error_two: &error,
    };
    for rendered in [
        format!("{public_key:?}"),
        format!("{round_one:?}"),
        format!("{round_two:?}"),
    ] {
        assert!(rendered.contains("[REDACTED]"));
        assert!(!rendered.contains("-1"));
        assert!(!rendered.contains("-2"));
    }
}

#[test]
fn active_and_wire_rosters_share_one_identity_but_not_the_key_certificate() {
    let fixture = fixture(b"active-wire-roster-identity", 410);
    let wire = fixture.roster.to_wire_roster().unwrap();
    assert_eq!(wire.profile_digest(), fixture.roster.profile_digest());
    assert_eq!(wire.epoch(), fixture.roster.epoch());
    assert_eq!(wire.roster_digest(), fixture.roster.roster_digest());
    let active_parties = fixture
        .roster
        .participants()
        .iter()
        .map(|participant| participant.party())
        .collect::<Vec<_>>();
    assert_eq!(wire.parties().as_slice(), active_parties.as_slice());
    assert_ne!(fixture.roster.key_material_digest(), wire.roster_digest());

    let encoded = wire.encode().unwrap();
    let decoded = super::super::ZkAmsMkheGovernedRosterWireV1::decode_exact(
        &encoded,
        fixture.roster.profile_digest(),
        fixture.roster.epoch(),
    )
    .unwrap();
    assert_eq!(decoded, wire);
    assert_eq!(decoded.roster_digest(), fixture.roster.roster_digest());
}

#[test]
fn roster_and_key_material_digests_cannot_be_cross_spliced() {
    let primary = fixture(b"active-roster-digest-primary", 411);
    let other = fixture(b"active-roster-digest-other", 411);
    assert_ne!(primary.roster.roster_digest(), other.roster.roster_digest());
    assert_ne!(
        primary.roster.key_material_digest(),
        other.roster.key_material_digest()
    );

    let mut roster_splice = primary.roster;
    roster_splice.roster_digest = other.roster.roster_digest;
    assert_eq!(
        roster_splice.validate(),
        Err(ZkAmsMkheErrorV1::InvalidPartySet)
    );

    let mut key_splice = primary.roster;
    key_splice.key_material_digest = other.roster.key_material_digest;
    assert_eq!(
        key_splice.validate(),
        Err(ZkAmsMkheErrorV1::InvalidPartySet)
    );
}

#[test]
fn roster_rejects_reorder_duplicate_wrong_epoch_profile_and_key_splice() {
    let primary = fixture(b"active-roster-negative", 42);
    let mut reordered = primary.roster.clone();
    reordered.participants.swap(0, 1);
    assert_eq!(reordered.validate(), Err(ZkAmsMkheErrorV1::InvalidPartySet));

    let mut duplicate = primary.roster.clone();
    duplicate.participants[1] = duplicate.participants[0];
    assert_eq!(duplicate.validate(), Err(ZkAmsMkheErrorV1::InvalidPartySet));

    let mut zero_epoch = primary.roster.clone();
    zero_epoch.epoch = 0;
    assert_eq!(
        zero_epoch.validate(),
        Err(ZkAmsMkheErrorV1::InvalidPartySet)
    );

    let mut profile = primary.roster.clone();
    profile.profile_digest[0] ^= 1;
    assert_eq!(profile.validate(), Err(ZkAmsMkheErrorV1::InvalidPartySet));

    let other = fixture(b"active-roster-other", 42);
    let mut key_splice = primary.roster.clone();
    key_splice.participants[3].authentication_public_key =
        other.roster.participants[3].authentication_public_key;
    assert!(key_splice.validate().is_err());
}

#[test]
fn roster_pop_binds_full_roster_epoch_index_party_key_and_commitment() {
    let fixture = fixture(b"active-roster-pop-binding", 43);
    for mutation in 0..7 {
        let mut changed = fixture.roster.clone();
        match mutation {
            0 => changed.epoch += 1,
            1 => changed.roster_digest[0] ^= 1,
            2 => changed.participants[0].party = changed.participants[1].party,
            3 => {
                changed.participants[0].authentication_public_key =
                    changed.participants[1].authentication_public_key
            }
            4 => changed.participants[0].key_proof.commitment[8] ^= 1,
            5 => changed.participants[0].key_proof.response[31] ^= 1,
            6 => changed.participants.swap(0, 1),
            _ => unreachable!(),
        }
        assert!(changed.validate().is_err(), "mutation {mutation} must fail");
    }
}

#[test]
fn rogue_inverse_key_cannot_reuse_an_honest_key_proof() {
    let fixture = fixture(b"active-roster-rogue-inverse", 44);
    let mut rogue = fixture.roster.clone();
    let honest = rogue.participants[0];
    let inverse =
        VegaT256PointV1::from_non_identity_wire_bytes_exact(&honest.authentication_public_key)
            .unwrap()
            .negate()
            .to_non_identity_wire_bytes()
            .unwrap();
    rogue.participants[0].authentication_public_key = inverse;
    rogue.participants[0].party = ZkAmsMkhePartyIdV1::from_authentication_key(&inverse).unwrap();
    assert!(rogue.validate().is_err());
}

#[test]
fn complete_ordered_round_returns_exact_receipt() {
    let fixture = fixture(b"active-round-positive", 45);
    let transcript = keccak256(b"active-round-transcript");
    let values = contributions(
        &fixture,
        ZkAmsMkheActiveRoundV1::Cks,
        transcript,
        b"active-round-contributions",
    );
    let receipt = zk_ams_mkhe_collect_active_round_v1(
        &fixture.roster,
        transcript,
        ZkAmsMkheActiveRoundV1::Cks,
        &values,
    )
    .expect("complete round");
    assert_eq!(receipt.round(), ZkAmsMkheActiveRoundV1::Cks);
    assert_ne!(receipt.receipt_digest(), [0; 32]);
    let expected: [[u8; 32]; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1] = values
        .iter()
        .map(|value| value.digest().unwrap())
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();
    assert_eq!(receipt.contribution_digests(), &expected);
}

#[test]
fn missing_duplicate_reordered_and_excess_rounds_identify_first_offender() {
    let fixture = fixture(b"active-round-cardinality", 46);
    let transcript = keccak256(b"active-cardinality-transcript");
    let baseline = contributions(
        &fixture,
        ZkAmsMkheActiveRoundV1::RkgRoundOne,
        transcript,
        b"active-cardinality-contributions",
    );

    let missing = zk_ams_mkhe_collect_active_round_v1(
        &fixture.roster,
        transcript,
        ZkAmsMkheActiveRoundV1::RkgRoundOne,
        &baseline[..7],
    )
    .unwrap_err();
    assert_eq!(missing.reason(), ZkAmsMkheAbortReasonV1::MissingContributor);
    assert_eq!(missing.expected_index(), 7);
    assert_eq!(missing.observed_party(), None);

    let mut duplicate = baseline.clone();
    duplicate[4] = duplicate[3].clone();
    let duplicate = zk_ams_mkhe_collect_active_round_v1(
        &fixture.roster,
        transcript,
        ZkAmsMkheActiveRoundV1::RkgRoundOne,
        &duplicate,
    )
    .unwrap_err();
    assert_eq!(
        duplicate.reason(),
        ZkAmsMkheAbortReasonV1::DuplicateContributor
    );
    assert_eq!(duplicate.expected_index(), 4);

    let mut reordered = baseline.clone();
    reordered.swap(2, 3);
    let reordered = zk_ams_mkhe_collect_active_round_v1(
        &fixture.roster,
        transcript,
        ZkAmsMkheActiveRoundV1::RkgRoundOne,
        &reordered,
    )
    .unwrap_err();
    assert_eq!(
        reordered.reason(),
        ZkAmsMkheAbortReasonV1::ReorderedContributor
    );
    assert_eq!(reordered.expected_index(), 2);

    let mut excess = baseline.clone();
    excess.push(baseline[0].clone());
    let excess = zk_ams_mkhe_collect_active_round_v1(
        &fixture.roster,
        transcript,
        ZkAmsMkheActiveRoundV1::RkgRoundOne,
        &excess,
    )
    .unwrap_err();
    assert_eq!(excess.reason(), ZkAmsMkheAbortReasonV1::ExcessContributor);
}

#[test]
fn every_context_field_and_authentication_mutation_aborts() {
    let fixture = fixture(b"active-round-context", 47);
    let transcript = keccak256(b"active-context-transcript");
    let baseline = contributions(
        &fixture,
        ZkAmsMkheActiveRoundV1::RkgRoundTwo,
        transcript,
        b"active-context-contributions",
    );
    let expected = [
        ZkAmsMkheAbortReasonV1::InvalidVersion,
        ZkAmsMkheAbortReasonV1::SplicedProfile,
        ZkAmsMkheAbortReasonV1::SplicedRoster,
        ZkAmsMkheAbortReasonV1::SplicedEpoch,
        ZkAmsMkheAbortReasonV1::SplicedTranscript,
        ZkAmsMkheAbortReasonV1::SplicedRound,
        ZkAmsMkheAbortReasonV1::IndexMismatch,
        ZkAmsMkheAbortReasonV1::InvalidPayload,
        ZkAmsMkheAbortReasonV1::SplicedAuthenticationKey,
        ZkAmsMkheAbortReasonV1::InvalidAuthentication,
    ];
    for (mutation, expected_reason) in expected.into_iter().enumerate() {
        let mut changed = baseline.clone();
        match mutation {
            0 => changed[0].version += 1,
            1 => changed[0].profile_digest[0] ^= 1,
            2 => changed[0].roster_digest[0] ^= 1,
            3 => changed[0].epoch += 1,
            4 => changed[0].transcript_digest[0] ^= 1,
            5 => changed[0].round = ZkAmsMkheActiveRoundV1::Cks,
            6 => changed[0].contribution_index = 1,
            7 => changed[0].payload_digest = [0; 32],
            8 => changed[0].authentication.public_key = baseline[1].authentication.public_key,
            9 => changed[0].authentication.signature[64] ^= 1,
            _ => unreachable!(),
        }
        let abort = zk_ams_mkhe_collect_active_round_v1(
            &fixture.roster,
            transcript,
            ZkAmsMkheActiveRoundV1::RkgRoundTwo,
            &changed,
        )
        .unwrap_err();
        assert_eq!(abort.reason(), expected_reason, "mutation {mutation}");
        assert_eq!(abort.expected_index(), 0);
        assert_ne!(abort.evidence_digest(), [0; 32]);
    }
}

#[test]
fn cross_roster_epoch_transcript_round_and_index_replay_all_fail() {
    let primary = fixture(b"active-round-replay", 48);
    let other = fixture(b"active-round-replay-other", 49);
    let transcript = keccak256(b"active-replay-transcript");
    let baseline = contributions(
        &primary,
        ZkAmsMkheActiveRoundV1::Cks,
        transcript,
        b"active-replay-contributions",
    );

    assert!(
        zk_ams_mkhe_collect_active_round_v1(
            &other.roster,
            transcript,
            ZkAmsMkheActiveRoundV1::Cks,
            &baseline,
        )
        .is_err()
    );
    assert!(
        zk_ams_mkhe_collect_active_round_v1(
            &primary.roster,
            keccak256(b"other transcript"),
            ZkAmsMkheActiveRoundV1::Cks,
            &baseline,
        )
        .is_err()
    );
    assert!(
        zk_ams_mkhe_collect_active_round_v1(
            &primary.roster,
            transcript,
            ZkAmsMkheActiveRoundV1::RkgRoundOne,
            &baseline,
        )
        .is_err()
    );
    let mut moved = baseline.clone();
    moved[0].contribution_index = 7;
    assert!(
        zk_ams_mkhe_collect_active_round_v1(
            &primary.roster,
            transcript,
            ZkAmsMkheActiveRoundV1::Cks,
            &moved,
        )
        .is_err()
    );
}

#[test]
fn abort_evidence_is_deterministic_and_reason_separated() {
    let fixture = fixture(b"active-abort-determinism", 50);
    let transcript = keccak256(b"active-abort-transcript");
    let baseline = contributions(
        &fixture,
        ZkAmsMkheActiveRoundV1::CollectivePublicKey,
        transcript,
        b"active-abort-contributions",
    );
    let first = zk_ams_mkhe_collect_active_round_v1(
        &fixture.roster,
        transcript,
        ZkAmsMkheActiveRoundV1::CollectivePublicKey,
        &baseline[..6],
    )
    .unwrap_err();
    let second = zk_ams_mkhe_collect_active_round_v1(
        &fixture.roster,
        transcript,
        ZkAmsMkheActiveRoundV1::CollectivePublicKey,
        &baseline[..6],
    )
    .unwrap_err();
    assert_eq!(first, second);

    let mut invalid = baseline.clone();
    invalid[6].authentication.signature[40] ^= 1;
    let invalid = zk_ams_mkhe_collect_active_round_v1(
        &fixture.roster,
        transcript,
        ZkAmsMkheActiveRoundV1::CollectivePublicKey,
        &invalid,
    )
    .unwrap_err();
    assert_ne!(first.evidence_digest(), invalid.evidence_digest());
}

#[test]
fn material_identity_requires_four_complete_same_roster_rounds() {
    let fixture = fixture(b"active-material", 51);
    let make_receipt = |round, label: &[u8]| {
        let transcript = keccak256(label);
        let values = contributions(&fixture, round, transcript, label);
        zk_ams_mkhe_collect_active_round_v1(&fixture.roster, transcript, round, &values)
            .expect("receipt")
    };
    let public_key = make_receipt(
        ZkAmsMkheActiveRoundV1::CollectivePublicKey,
        b"active-material-pk",
    );
    let cks = make_receipt(ZkAmsMkheActiveRoundV1::Cks, b"active-material-cks");
    let rkg_one = make_receipt(
        ZkAmsMkheActiveRoundV1::RkgRoundOne,
        b"active-material-rkg-one",
    );
    let rkg_two = make_receipt(
        ZkAmsMkheActiveRoundV1::RkgRoundTwo,
        b"active-material-rkg-two",
    );
    let material = ZkAmsMkheGovernedCollectiveKeyMaterialIdentityV1::from_receipts(
        &fixture.roster,
        public_key,
        cks,
        rkg_one,
        rkg_two,
    )
    .expect("material");
    assert_ne!(material.material_digest(), [0; 32]);

    assert!(
        ZkAmsMkheGovernedCollectiveKeyMaterialIdentityV1::from_receipts(
            &fixture.roster,
            cks,
            public_key,
            rkg_one,
            rkg_two,
        )
        .is_err()
    );
    let mut tampered = rkg_two;
    tampered.receipt_digest[0] ^= 1;
    assert!(
        ZkAmsMkheGovernedCollectiveKeyMaterialIdentityV1::from_receipts(
            &fixture.roster,
            public_key,
            cks,
            rkg_one,
            tampered,
        )
        .is_err()
    );
}

#[test]
fn contribution_authentication_rejects_wrong_roster_secret() {
    let fixture = fixture(b"active-wrong-secret", 52);
    let mut random = KatRandom::new(b"active-wrong-secret-random");
    assert_eq!(
        authenticate_active_contribution(
            &fixture.roster,
            keccak256(b"active-wrong-secret-transcript"),
            ZkAmsMkheActiveRoundV1::Cks,
            0,
            keccak256(b"active-wrong-secret-payload"),
            &fixture.secrets[1],
            &mut random,
        ),
        Err(ZkAmsMkheErrorV1::InvalidAuthentication)
    );
}

#[test]
fn roster_proofs_and_contributions_reject_degenerate_randomness() {
    struct ZeroRandom;
    impl MaskedRelaxedRandomSourceV1 for ZeroRandom {
        fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), MaskedRelaxedRandomErrorV1> {
            destination.fill(0);
            Ok(())
        }
    }

    let fixture = fixture(b"active-zero-random", 53);
    let references: [&AuthenticationSecret; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1] = fixture
        .secrets
        .iter()
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();
    assert_eq!(
        assemble_governed_active_roster(53, references, &mut ZeroRandom),
        Err(ZkAmsMkheErrorV1::RandomUnavailable)
    );
}

#[test]
fn legacy_authentication_challenge_cannot_validate_roster_pop() {
    let fixture = fixture(b"active-domain-separation", 54);
    let participant = fixture.roster.participants[0];
    let legacy = authentication_challenge(
        ROSTER_POP_DOMAIN_V1,
        fixture.roster.roster_digest,
        participant.party,
        &participant.authentication_public_key,
        &participant.key_proof.commitment,
    )
    .unwrap();
    let exact = roster_pop_challenge(
        fixture.roster.profile_digest,
        fixture.roster.epoch,
        fixture.roster.roster_digest,
        fixture.roster.key_material_digest,
        0,
        participant.party,
        participant.authentication_public_key,
        participant.key_proof.commitment,
    )
    .unwrap();
    assert_ne!(legacy, exact);
    assert_ne!(participant.key_proof.signature_bytes(), [0; 65]);
}

fn linear_context(
    profile: &super::super::BgvProfile,
    round: ZkAmsMkheActiveRoundV1,
) -> LinearProofContextV1 {
    LinearProofContextV1 {
        profile_digest: profile.digest().unwrap(),
        roster_digest: keccak256(b"linear-proof-test-roster"),
        epoch: 91,
        transcript_digest: keccak256(b"linear-proof-test-transcript"),
        round,
        party_index: 3,
        party: ZkAmsMkhePartyIdV1::new([0x44; 32]).unwrap(),
        record_index: 17,
        relation_index: 9,
    }
}

fn linear_statement_fixture(
    profile: &super::super::BgvProfile,
) -> (
    LinearRelationStatementV1,
    super::super::SecretPolynomial,
    super::super::SecretPolynomial,
) {
    let a = super::super::RnsPolynomial::from_unsigned(profile, &[3, 5, 7, 11, 13, 17, 19, 23])
        .unwrap();
    let mut plaintext = vec![0_i64; profile.ring_degree];
    plaintext[0] = 17;
    let plaintext = super::super::RnsPolynomial::from_signed(profile, &plaintext).unwrap();
    let secret = super::super::SecretPolynomial {
        coefficients: vec![-1, 0, 1, 1, 0, -1, 1, 0],
    };
    let error = super::super::SecretPolynomial {
        coefficients: vec![2, -1, 0, 1, -2, 2, 0, -1],
    };
    let target = a
        .mul(&secret.as_rns(profile).unwrap(), profile)
        .unwrap()
        .add(
            &plaintext
                .mul(&error.as_rns(profile).unwrap(), profile)
                .unwrap(),
            profile,
        )
        .unwrap();
    (
        LinearRelationStatementV1 {
            witness_bounds: vec![1, 2],
            witness_challenge_automorphism_exponents: vec![1, 1],
            outputs: vec![LinearRelationOutputV1 {
                target,
                challenge_automorphism_exponent: 1,
                terms: vec![
                    LinearRelationTermV1 {
                        witness_index: 0,
                        multiplier: a,
                        witness_automorphism_exponent: 1,
                    },
                    LinearRelationTermV1 {
                        witness_index: 1,
                        multiplier: plaintext,
                        witness_automorphism_exponent: 1,
                    },
                ],
            }],
        },
        secret,
        error,
    )
}

#[test]
fn narrow_lattice_proof_is_explicitly_limited_to_governed_relation_rounds() {
    let profile = linear_test_profile();
    let (statement, secret, error) = linear_statement_fixture(&profile);
    for round in [
        ZkAmsMkheActiveRoundV1::CollectivePublicKey,
        ZkAmsMkheActiveRoundV1::RkgRoundOne,
        ZkAmsMkheActiveRoundV1::RkgRoundTwo,
        ZkAmsMkheActiveRoundV1::GaloisSource,
    ] {
        let context = linear_context(&profile, round);
        let mut random =
            KatRandom::new(&[b"linear-proof-positive-".as_slice(), &[round.tag()]].concat());
        let proof = prove_linear_relation_v1(
            &profile,
            context,
            &statement,
            &[&secret, &error],
            &mut random,
        )
        .expect("linear relation proof");
        verify_linear_relation_proof(&profile, context, &statement, &proof)
            .expect("verified relation");
        assert_ne!(proof.challenge_seed, [0; 32]);
        assert_ne!(
            proof.digest(&profile, context, &statement).unwrap(),
            [0; 32]
        );
    }
    assert_eq!(
        linear_context(&profile, ZkAmsMkheActiveRoundV1::Cks).validate(&profile),
        Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
    );
}

#[test]
fn linear_proof_reconstructs_commitment_instead_of_accepting_a_digest_claim() {
    let profile = linear_test_profile();
    let (statement, secret, error) = linear_statement_fixture(&profile);
    let context = linear_context(&profile, ZkAmsMkheActiveRoundV1::RkgRoundOne);
    let mut random = KatRandom::new(b"linear-proof-reconstruction");
    let proof = prove_linear_relation_v1(
        &profile,
        context,
        &statement,
        &[&secret, &error],
        &mut random,
    )
    .unwrap();
    let challenge = derive_sparse_challenge(profile.ring_degree, proof.challenge_seed).unwrap();
    let challenge_rns = super::super::RnsPolynomial::from_signed(&profile, &challenge).unwrap();
    let response_rns = proof
        .responses
        .iter()
        .map(|response| super::super::RnsPolynomial::from_signed(&profile, response).unwrap())
        .collect::<Vec<_>>();
    let applied = apply_linear_relation(&profile, &statement, &response_rns).unwrap();
    let reconstructed = applied
        .into_iter()
        .zip(&statement.outputs)
        .map(|(response, output)| {
            response
                .sub(
                    &output.target.mul(&challenge_rns, &profile).unwrap(),
                    &profile,
                )
                .unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        linear_commitment_challenge_seed(&profile, context, &statement, &reconstructed).unwrap(),
        proof.challenge_seed
    );
}

#[test]
fn rkg_proof_wire_has_one_exact_roundtrip_and_rejects_header_splices() {
    let profile = linear_test_profile();
    let (statement, secret, error) = linear_statement_fixture(&profile);
    let context = linear_context(&profile, ZkAmsMkheActiveRoundV1::RkgRoundOne);
    let proof = prove_linear_relation_v1(
        &profile,
        context,
        &statement,
        &[&secret, &error],
        &mut KatRandom::new(b"rkg-proof-wire-roundtrip"),
    )
    .unwrap();
    let encoded = proof.encode_wire().unwrap();
    assert_eq!(
        encoded.len(),
        linear_proof_wire_bytes(2, profile.ring_degree).unwrap()
    );
    let decoded =
        LinearRelationProofV1::decode_wire_exact(&encoded, 2, profile.ring_degree).unwrap();
    assert_eq!(decoded, proof);
    verify_linear_relation_proof(&profile, context, &statement, &decoded).unwrap();
    assert_eq!(decoded.encode_wire().unwrap(), encoded);

    for offset in [0, 4, 37, 38, 41] {
        let mut changed = encoded.clone();
        changed[offset] ^= 1;
        assert!(
            LinearRelationProofV1::decode_wire_exact(&changed, 2, profile.ring_degree,).is_err(),
            "header mutation at {offset} must fail"
        );
    }
    for offset in [5, 36] {
        let mut changed = encoded.clone();
        changed[offset] ^= 1;
        let decoded =
            LinearRelationProofV1::decode_wire_exact(&changed, 2, profile.ring_degree).unwrap();
        assert!(verify_linear_relation_proof(&profile, context, &statement, &decoded).is_err());
    }
    let mut zero_seed = encoded.clone();
    zero_seed[5..37].fill(0);
    assert!(LinearRelationProofV1::decode_wire_exact(&zero_seed, 2, profile.ring_degree,).is_err());
    for malformed in [&encoded[..encoded.len() - 1], &encoded[..41]] {
        assert!(
            LinearRelationProofV1::decode_wire_exact(malformed, 2, profile.ring_degree,).is_err()
        );
    }
    let mut trailing = encoded.clone();
    trailing.push(0);
    assert!(LinearRelationProofV1::decode_wire_exact(&trailing, 2, profile.ring_degree,).is_err());
    assert!(LinearRelationProofV1::decode_wire_exact(&encoded, 1, profile.ring_degree).is_err());
    assert!(
        LinearRelationProofV1::decode_wire_exact(&encoded, 2, profile.ring_degree * 2).is_err()
    );
}

#[test]
fn active_rkg_evidence_codec_is_exact_and_rejects_every_authenticated_splice_class() {
    let profile = release_profile_v1();
    let proof_bytes = LinearRelationProofV1 {
        challenge_seed: keccak256(b"active-rkg-evidence-release-proof"),
        responses: vec![vec![0; profile.ring_degree]; 2],
    }
    .encode_wire()
    .unwrap();
    let fixture = fixture(b"active-rkg-evidence", 98);
    let transcript = keccak256(b"active-rkg-evidence-transcript");
    let contribution = authenticate_active_contribution(
        &fixture.roster,
        transcript,
        ZkAmsMkheActiveRoundV1::GaloisSource,
        0,
        keccak256(&proof_bytes),
        &fixture.secrets[0],
        &mut KatRandom::new(b"active-rkg-evidence-authentication"),
    )
    .unwrap();
    validate_active_contribution(
        &fixture.roster,
        transcript,
        ZkAmsMkheActiveRoundV1::GaloisSource,
        0,
        &contribution,
    )
    .unwrap();
    let evidence = ZkAmsMkheActiveRkgProofV1 {
        statement_digest: keccak256(b"active-rkg-evidence-statement"),
        witness_polynomials: 2,
        proof_bytes,
        contribution,
    };
    let encoded = evidence.encode_evidence().unwrap();
    let decoded = ZkAmsMkheActiveRkgProofV1::decode_evidence_exact(&encoded).unwrap();
    assert_eq!(decoded, evidence);
    assert_eq!(decoded.encode_evidence().unwrap(), encoded);
    let mut streamed = Vec::with_capacity(encoded.len());
    evidence
        .write_evidence_chunks(|chunk| {
            streamed.extend_from_slice(chunk);
            Ok(())
        })
        .unwrap();
    assert_eq!(streamed, encoded);
    let mut reader = encoded.as_slice();
    let decoded_from_reader =
        ZkAmsMkheActiveRkgProofV1::decode_evidence_from_reader(&mut reader, encoded.len() as u64)
            .unwrap();
    assert_eq!(decoded_from_reader, evidence);
    assert!(reader.is_empty());

    let proof_start = ACTIVE_RKG_EVIDENCE_HEADER_BYTES_V1;
    let contribution_start = proof_start + evidence.proof_bytes.len();
    let authenticated_mutations = [
        ("profile", contribution_start + 1),
        ("roster", contribution_start + 33),
        ("epoch", contribution_start + 72),
        ("transcript", contribution_start + 73),
        ("round", contribution_start + 105),
        ("index", contribution_start + 109),
        ("party", contribution_start + 110),
        ("payload", contribution_start + 142),
        ("authentication party", contribution_start + 175),
        ("authentication key", contribution_start + 207),
        ("authentication signature", contribution_start + 304),
    ];
    for (label, offset) in authenticated_mutations {
        let mut changed = encoded.clone();
        changed[offset] ^= 1;
        assert!(
            ZkAmsMkheActiveRkgProofV1::decode_evidence_exact(&changed).is_err(),
            "authenticated {label} splice must fail"
        );
    }

    for (label, offset) in [
        ("outer tag", 0),
        ("outer version", 4),
        ("witness count", 37),
        ("proof length", 38),
        ("proof tag", proof_start),
        ("proof version", proof_start + 4),
        ("proof witness count", proof_start + 37),
        ("proof ring degree", proof_start + 38),
        ("contribution version", contribution_start),
        ("authentication version", contribution_start + 174),
    ] {
        let mut changed = encoded.clone();
        changed[offset] ^= 1;
        assert!(
            ZkAmsMkheActiveRkgProofV1::decode_evidence_exact(&changed).is_err(),
            "structural {label} splice must fail"
        );
    }

    let mut zero_statement = encoded.clone();
    zero_statement[5..37].fill(0);
    assert!(ZkAmsMkheActiveRkgProofV1::decode_evidence_exact(&zero_statement).is_err());
    assert!(
        ZkAmsMkheActiveRkgProofV1::decode_evidence_exact(&encoded[..encoded.len() - 1]).is_err()
    );
    let mut trailing = encoded.clone();
    trailing.push(0);
    assert!(ZkAmsMkheActiveRkgProofV1::decode_evidence_exact(&trailing).is_err());

    // The enclosing source-evidence verifier owns algebraic statement replay.
    // This codec must preserve those bytes exactly so that replay sees any
    // statement, challenge, or response mutation rather than normalizing it.
    for offset in [5, proof_start + 5, proof_start + 42] {
        let mut changed = encoded.clone();
        changed[offset] ^= 1;
        let decoded = ZkAmsMkheActiveRkgProofV1::decode_evidence_exact(&changed).unwrap();
        assert_eq!(decoded.encode_evidence().unwrap(), changed);
        assert_ne!(changed, encoded);
    }
}

#[test]
fn rkg_wire_decodes_all_i64_patterns_but_verification_enforces_exact_bounds() {
    let profile = linear_test_profile();
    let (statement, secret, error) = linear_statement_fixture(&profile);
    let context = linear_context(&profile, ZkAmsMkheActiveRoundV1::RkgRoundOne);
    let proof = prove_linear_relation_v1(
        &profile,
        context,
        &statement,
        &[&secret, &error],
        &mut KatRandom::new(b"rkg-proof-wire-i64-boundaries"),
    )
    .unwrap();
    let mut encoded = proof.encode_wire().unwrap();
    let response_start = RKG_LINEAR_PROOF_WIRE_HEADER_BYTES_V1;
    for boundary in [i64::MIN, i64::MAX] {
        encoded[response_start..response_start + 8].copy_from_slice(&boundary.to_be_bytes());
        let decoded =
            LinearRelationProofV1::decode_wire_exact(&encoded, 2, profile.ring_degree).unwrap();
        assert_eq!(decoded.responses[0][0], boundary);
        assert!(verify_linear_relation_proof(&profile, context, &statement, &decoded).is_err());
    }
}

#[test]
fn linear_proof_rejects_challenge_response_shape_bound_and_order_mutations() {
    let profile = linear_test_profile();
    let (statement, secret, error) = linear_statement_fixture(&profile);
    let context = linear_context(&profile, ZkAmsMkheActiveRoundV1::RkgRoundOne);
    let mut random = KatRandom::new(b"linear-proof-response-negative");
    let baseline = prove_linear_relation_v1(
        &profile,
        context,
        &statement,
        &[&secret, &error],
        &mut random,
    )
    .unwrap();

    let mut challenge = baseline.clone();
    challenge.challenge_seed[0] ^= 1;
    assert!(verify_linear_relation_proof(&profile, context, &statement, &challenge).is_err());

    let mut response = baseline.clone();
    response.responses[0][3] += 1;
    assert!(verify_linear_relation_proof(&profile, context, &statement, &response).is_err());

    let mut truncated = baseline.clone();
    truncated.responses[0].pop();
    assert!(verify_linear_relation_proof(&profile, context, &statement, &truncated).is_err());

    let mut missing = baseline.clone();
    missing.responses.pop();
    assert!(verify_linear_relation_proof(&profile, context, &statement, &missing).is_err());

    let mut reordered = baseline.clone();
    reordered.responses.swap(0, 1);
    assert!(verify_linear_relation_proof(&profile, context, &statement, &reordered).is_err());

    let mut out_of_bound = baseline;
    let (_, response_limit) = linear_response_parameters(
        statement.witness_bounds[0],
        linear_challenge_weight(profile.ring_degree).unwrap(),
    )
    .unwrap();
    out_of_bound.responses[0][0] = response_limit + 1;
    assert!(verify_linear_relation_proof(&profile, context, &statement, &out_of_bound).is_err());
}

#[test]
fn linear_proof_binds_every_context_field() {
    let profile = linear_test_profile();
    let (statement, secret, error) = linear_statement_fixture(&profile);
    let context = linear_context(&profile, ZkAmsMkheActiveRoundV1::RkgRoundTwo);
    let mut random = KatRandom::new(b"linear-proof-context-negative");
    let proof = prove_linear_relation_v1(
        &profile,
        context,
        &statement,
        &[&secret, &error],
        &mut random,
    )
    .unwrap();
    for mutation in 0..9 {
        let mut changed = context;
        match mutation {
            0 => changed.profile_digest[0] ^= 1,
            1 => changed.roster_digest[0] ^= 1,
            2 => changed.epoch += 1,
            3 => changed.transcript_digest[0] ^= 1,
            4 => changed.round = ZkAmsMkheActiveRoundV1::Cks,
            5 => changed.party_index += 1,
            6 => changed.party = ZkAmsMkhePartyIdV1::new([0x45; 32]).unwrap(),
            7 => changed.record_index += 1,
            8 => changed.relation_index += 1,
            _ => unreachable!(),
        }
        assert!(
            verify_linear_relation_proof(&profile, changed, &statement, &proof).is_err(),
            "context mutation {mutation} must fail"
        );
    }
}

#[test]
fn linear_proof_binds_target_multiplier_bounds_and_term_order() {
    let profile = linear_test_profile();
    let (statement, secret, error) = linear_statement_fixture(&profile);
    let context = linear_context(&profile, ZkAmsMkheActiveRoundV1::RkgRoundOne);
    let mut random = KatRandom::new(b"linear-proof-statement-negative");
    let proof = prove_linear_relation_v1(
        &profile,
        context,
        &statement,
        &[&secret, &error],
        &mut random,
    )
    .unwrap();

    let mut target = statement.clone();
    target.outputs[0].target.coefficients[0] =
        (target.outputs[0].target.coefficients[0] + 1) % profile.moduli[0];
    assert!(verify_linear_relation_proof(&profile, context, &target, &proof).is_err());

    let mut multiplier = statement.clone();
    multiplier.outputs[0].terms[0].multiplier.coefficients[0] =
        (multiplier.outputs[0].terms[0].multiplier.coefficients[0] + 1) % profile.moduli[0];
    assert!(verify_linear_relation_proof(&profile, context, &multiplier, &proof).is_err());

    let mut bound = statement.clone();
    bound.witness_bounds[0] += 1;
    assert!(verify_linear_relation_proof(&profile, context, &bound, &proof).is_err());

    let mut reordered = statement;
    reordered.outputs[0].terms.swap(0, 1);
    assert!(verify_linear_relation_proof(&profile, context, &reordered, &proof).is_err());
}

#[test]
fn invalid_linear_witness_fails_before_randomness() {
    struct NeverRandom;
    impl MaskedRelaxedRandomSourceV1 for NeverRandom {
        fn fill_bytes(
            &mut self,
            _destination: &mut [u8],
        ) -> Result<(), MaskedRelaxedRandomErrorV1> {
            panic!("invalid witness must fail before prover randomness")
        }
    }

    let profile = linear_test_profile();
    let (statement, mut secret, error) = linear_statement_fixture(&profile);
    secret.coefficients[0] = 2;
    assert_eq!(
        prove_linear_relation_v1(
            &profile,
            linear_context(&profile, ZkAmsMkheActiveRoundV1::RkgRoundOne),
            &statement,
            &[&secret, &error],
            &mut NeverRandom,
        ),
        Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
    );

    let (mut inconsistent, secret, error) = linear_statement_fixture(&profile);
    inconsistent.outputs[0].target.coefficients[0] =
        (inconsistent.outputs[0].target.coefficients[0] + 1) % profile.moduli[0];
    assert_eq!(
        prove_linear_relation_v1(
            &profile,
            linear_context(&profile, ZkAmsMkheActiveRoundV1::RkgRoundOne),
            &inconsistent,
            &[&secret, &error],
            &mut NeverRandom,
        ),
        Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
    );
}

#[test]
fn linear_proof_rejects_zero_and_repeating_entropy() {
    struct ConstantRandom(u8);
    impl MaskedRelaxedRandomSourceV1 for ConstantRandom {
        fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), MaskedRelaxedRandomErrorV1> {
            destination.fill(self.0);
            Ok(())
        }
    }

    let profile = linear_test_profile();
    let (statement, secret, error) = linear_statement_fixture(&profile);
    for byte in [0, 1, 0xff] {
        assert_eq!(
            prove_linear_relation_v1(
                &profile,
                linear_context(&profile, ZkAmsMkheActiveRoundV1::RkgRoundOne),
                &statement,
                &[&secret, &error],
                &mut ConstantRandom(byte),
            ),
            Err(ZkAmsMkheErrorV1::RandomUnavailable)
        );
    }
}

#[test]
fn sparse_challenge_is_canonical_and_negacyclic_multiplication_matches_rns() {
    let profile = linear_test_profile();
    let challenge =
        derive_sparse_challenge(profile.ring_degree, keccak256(b"sparse-challenge-kat")).unwrap();
    assert_eq!(
        challenge
            .iter()
            .filter(|coefficient| **coefficient != 0)
            .count(),
        linear_challenge_weight(profile.ring_degree).unwrap()
    );
    assert!(
        challenge
            .iter()
            .all(|coefficient| [-1, 0, 1].contains(coefficient))
    );

    let dense = [-1, 0, 1, 2, -2, 1, 0, -1];
    let signed = sparse_negacyclic_mul_signed(&challenge, &dense).unwrap();
    let expected = super::super::RnsPolynomial::from_signed(&profile, &challenge)
        .unwrap()
        .mul(
            &super::super::RnsPolynomial::from_signed(&profile, &dense).unwrap(),
            &profile,
        )
        .unwrap();
    assert_eq!(
        super::super::RnsPolynomial::from_signed(&profile, &signed).unwrap(),
        expected
    );
}

#[test]
fn release_rkg_linear_proof_security_parameters_are_exact_and_no_wrap() {
    let certificate = zk_ams_mkhe_active_rkg_linear_proof_security_v1().unwrap();
    certificate.validate().unwrap();
    assert_eq!(certificate.ring_degree, 131_072);
    assert_eq!(certificate.max_witness_polynomials, 8);
    assert_eq!(certificate.challenge_weight, 60);
    assert_eq!(certificate.challenge_space_lower_bound_bits, 720);
    assert_eq!(certificate.fiat_shamir_bits, 256);
    assert_eq!(certificate.challenge_min_entropy_bits, 256);
    assert_eq!(certificate.transcript_binding_bits, 128);
    assert_eq!(certificate.soundness_bits, 128);
    assert_eq!(certificate.max_witness_coefficient, 2);
    assert_eq!(certificate.challenge_response_slack, 120);
    assert_eq!(certificate.mask_coefficient_bound, 2_013_265_920);
    assert_eq!(certificate.response_coefficient_bound, 2_013_265_800);
    assert_eq!(certificate.max_response_coordinates, 1_048_576);
    assert_eq!(certificate.rejection_probability_denominator, 16);
    assert_eq!(certificate.retry_ceiling, 128);
    assert_eq!(certificate.retry_exhaustion_bits, 512);
    assert_eq!(certificate.signed_coefficient_bytes, 8);
    assert_eq!(certificate.max_proof_bytes, 8_388_650);
    assert!(
        u64::try_from(certificate.response_coefficient_bound).unwrap()
            < (certificate.minimum_rns_modulus - 1) / 2
    );
    assert_ne!(certificate.parameter_digest, [0; 32]);

    let mut changed = certificate;
    changed.challenge_weight -= 1;
    assert_eq!(changed.validate(), Err(ZkAmsMkheErrorV1::InvalidProfile));
    let mut changed = certificate;
    changed.parameter_digest[0] ^= 1;
    assert_eq!(changed.validate(), Err(ZkAmsMkheErrorV1::InvalidProfile));
}

#[test]
fn sparse_challenge_rejects_zero_seed_duplicate_reorder_bad_sign_and_bounds() {
    let profile = linear_test_profile();
    assert_eq!(
        derive_sparse_challenge(profile.ring_degree, [0; 32]),
        Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
    );
    let valid = vec![
        SparseChallengeTermV1 {
            position: 0,
            sign: -1,
        },
        SparseChallengeTermV1 {
            position: 2,
            sign: 1,
        },
        SparseChallengeTermV1 {
            position: 4,
            sign: -1,
        },
        SparseChallengeTermV1 {
            position: 7,
            sign: 1,
        },
    ];
    SparseChallengeV1::new(profile.ring_degree, valid.clone()).unwrap();

    let mut duplicate = valid.clone();
    duplicate[2].position = duplicate[1].position;
    assert!(SparseChallengeV1::new(profile.ring_degree, duplicate).is_err());

    let mut reordered = valid.clone();
    reordered.swap(1, 2);
    assert!(SparseChallengeV1::new(profile.ring_degree, reordered).is_err());

    for sign in [0, -2, 2, i8::MIN, i8::MAX] {
        let mut bad_sign = valid.clone();
        bad_sign[0].sign = sign;
        assert!(SparseChallengeV1::new(profile.ring_degree, bad_sign).is_err());
    }

    let mut out_of_range = valid.clone();
    out_of_range[3].position = profile.ring_degree as u32;
    assert!(SparseChallengeV1::new(profile.ring_degree, out_of_range).is_err());

    assert!(SparseChallengeV1::new(profile.ring_degree, valid[..3].to_vec()).is_err());
}

#[test]
fn response_bounds_accept_both_exact_edges_and_reject_one_past_each_edge() {
    let profile = linear_test_profile();
    let weight = linear_challenge_weight(profile.ring_degree).unwrap();
    for witness_bound in [1, 2] {
        let (_, limit) = linear_response_parameters(witness_bound, weight).unwrap();
        for edge in [limit, -limit] {
            assert!(
                validate_linear_response_coefficients(
                    &vec![edge; profile.ring_degree],
                    profile.ring_degree,
                    witness_bound,
                    weight,
                )
                .is_ok()
            );
        }
        for outside in [limit + 1, -limit - 1] {
            assert!(
                validate_linear_response_coefficients(
                    &vec![outside; profile.ring_degree],
                    profile.ring_degree,
                    witness_bound,
                    weight,
                )
                .is_err()
            );
        }
    }
}

#[test]
fn fiat_shamir_with_aborts_hits_the_exact_retry_ceiling() {
    struct BoundaryRandom {
        calls: usize,
    }

    impl MaskedRelaxedRandomSourceV1 for BoundaryRandom {
        fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), MaskedRelaxedRandomErrorV1> {
            self.calls += 1;
            match self.calls {
                1 => destination.fill(1),
                2 => destination.fill(2),
                _ => destination.fill(0),
            }
            Ok(())
        }
    }

    let profile = linear_test_profile();
    let (statement, secret, error) = linear_statement_fixture(&profile);
    let mut random = BoundaryRandom { calls: 0 };
    assert_eq!(
        prove_linear_relation_v1(
            &profile,
            linear_context(&profile, ZkAmsMkheActiveRoundV1::RkgRoundOne),
            &statement,
            &[&secret, &error],
            &mut random,
        ),
        Err(ZkAmsMkheErrorV1::RandomUnavailable)
    );
    assert!(random.calls >= 2 + RANDOM_REJECTION_ATTEMPTS_V1);
}

#[test]
fn proof_replay_fails_across_every_round_digit_party_epoch_profile_and_roster_class() {
    let profile = linear_test_profile();
    let (statement, secret, error) = linear_statement_fixture(&profile);
    let context = linear_context(&profile, ZkAmsMkheActiveRoundV1::RkgRoundOne);
    let mut random = KatRandom::new(b"linear-proof-replay-matrix");
    let proof = prove_linear_relation_v1(
        &profile,
        context,
        &statement,
        &[&secret, &error],
        &mut random,
    )
    .unwrap();

    for round in [
        ZkAmsMkheActiveRoundV1::CollectivePublicKey,
        ZkAmsMkheActiveRoundV1::Cks,
        ZkAmsMkheActiveRoundV1::RkgRoundTwo,
    ] {
        let mut changed = context;
        changed.round = round;
        assert!(verify_linear_relation_proof(&profile, changed, &statement, &proof).is_err());
    }
    for mutate in 0..7 {
        let mut changed = context;
        match mutate {
            0 => changed.relation_index = changed.relation_index.wrapping_add(1),
            1 => changed.record_index = changed.record_index.wrapping_add(1),
            2 => changed.party_index += 1,
            3 => changed.party = ZkAmsMkhePartyIdV1::new([0x99; 32]).unwrap(),
            4 => changed.epoch += 1,
            5 => changed.profile_digest[31] ^= 1,
            6 => changed.roster_digest[31] ^= 1,
            _ => unreachable!(),
        }
        assert!(verify_linear_relation_proof(&profile, changed, &statement, &proof).is_err());
    }
}

#[test]
fn statement_rejects_target_multiplier_and_witness_dimension_mismatches() {
    let profile = linear_test_profile();
    let (statement, secret, error) = linear_statement_fixture(&profile);
    let context = linear_context(&profile, ZkAmsMkheActiveRoundV1::RkgRoundOne);

    let mut target = statement.clone();
    target.outputs[0].target.coefficients.pop();
    assert!(target.validate(&profile).is_err());

    let mut multiplier = statement.clone();
    multiplier.outputs[0].terms[0].multiplier.coefficients.pop();
    assert!(multiplier.validate(&profile).is_err());

    let mut missing_witness = statement.clone();
    missing_witness.witness_bounds.push(1);
    assert!(missing_witness.validate(&profile).is_err());

    let mut duplicate_term = statement.clone();
    duplicate_term.outputs[0].terms[1].witness_index = 0;
    assert!(duplicate_term.validate(&profile).is_err());

    let mut zero_multiplier = statement;
    zero_multiplier.outputs[0].terms[0].multiplier = super::super::RnsPolynomial::zero(&profile);
    assert!(zero_multiplier.validate(&profile).is_err());

    assert_eq!(
        prove_linear_relation_v1(
            &profile,
            context,
            &LinearRelationStatementV1 {
                witness_bounds: vec![1, 2],
                witness_challenge_automorphism_exponents: vec![1, 1],
                outputs: vec![],
            },
            &[&secret, &error],
            &mut KatRandom::new(b"dimension-mismatch-random"),
        ),
        Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
    );
}

fn tiny_galois_relation_fixture(
    profile: &super::super::BgvProfile,
    exponent: usize,
) -> (
    LinearRelationStatementV1,
    Vec<super::super::SecretPolynomial>,
) {
    let secret = super::super::SecretPolynomial {
        coefficients: vec![-1, 0, 1, 1, 0, -1, 1, 0],
    };
    let public_error = super::super::SecretPolynomial {
        coefficients: vec![2, -1, 0, 1, -2, 2, 0, -1],
    };
    let ephemeral = super::super::SecretPolynomial {
        coefficients: vec![1, 0, -1, 0, 1, 1, 0, -1],
    };
    let error_zero = super::super::SecretPolynomial {
        coefficients: vec![0, 2, -1, 0, 1, -2, 1, 0],
    };
    let error_one = super::super::SecretPolynomial {
        coefficients: vec![-2, 0, 1, 2, 0, -1, 0, 1],
    };
    let public_a =
        super::super::RnsPolynomial::from_unsigned(profile, &[3, 5, 7, 11, 13, 17, 19, 23])
            .unwrap();
    let plaintext = ring_one(profile)
        .unwrap()
        .scale_plaintext_modulus(profile)
        .unwrap();
    let gadget = ring_one(profile).unwrap().scale_gadget(0, profile).unwrap();
    let public_b = public_a
        .mul(&secret.as_rns(profile).unwrap(), profile)
        .unwrap()
        .negate(profile)
        .unwrap()
        .add(
            &plaintext
                .mul(&public_error.as_rns(profile).unwrap(), profile)
                .unwrap(),
            profile,
        )
        .unwrap();
    let transformed_secret = secret
        .automorphism(exponent, profile)
        .unwrap()
        .as_rns(profile)
        .unwrap();
    let source_constant = public_b
        .mul(&ephemeral.as_rns(profile).unwrap(), profile)
        .unwrap()
        .add(
            &plaintext
                .mul(&error_zero.as_rns(profile).unwrap(), profile)
                .unwrap(),
            profile,
        )
        .unwrap()
        .add(&gadget.mul(&transformed_secret, profile).unwrap(), profile)
        .unwrap();
    let source_linear = public_a
        .mul(&ephemeral.as_rns(profile).unwrap(), profile)
        .unwrap()
        .add(
            &plaintext
                .mul(&error_one.as_rns(profile).unwrap(), profile)
                .unwrap(),
            profile,
        )
        .unwrap();
    (
        LinearRelationStatementV1 {
            witness_bounds: vec![1, 2, 1, 2, 2],
            witness_challenge_automorphism_exponents: vec![1, 1, exponent, exponent, exponent],
            outputs: vec![
                LinearRelationOutputV1 {
                    target: public_b.clone(),
                    challenge_automorphism_exponent: 1,
                    terms: vec![
                        LinearRelationTermV1 {
                            witness_index: 0,
                            multiplier: public_a.negate(profile).unwrap(),
                            witness_automorphism_exponent: 1,
                        },
                        LinearRelationTermV1 {
                            witness_index: 1,
                            multiplier: plaintext.clone(),
                            witness_automorphism_exponent: 1,
                        },
                    ],
                },
                LinearRelationOutputV1 {
                    target: source_constant,
                    challenge_automorphism_exponent: exponent,
                    terms: vec![
                        LinearRelationTermV1 {
                            witness_index: 0,
                            multiplier: gadget,
                            witness_automorphism_exponent: exponent,
                        },
                        LinearRelationTermV1 {
                            witness_index: 2,
                            multiplier: public_b,
                            witness_automorphism_exponent: 1,
                        },
                        LinearRelationTermV1 {
                            witness_index: 3,
                            multiplier: plaintext.clone(),
                            witness_automorphism_exponent: 1,
                        },
                    ],
                },
                LinearRelationOutputV1 {
                    target: source_linear,
                    challenge_automorphism_exponent: exponent,
                    terms: vec![
                        LinearRelationTermV1 {
                            witness_index: 2,
                            multiplier: public_a,
                            witness_automorphism_exponent: 1,
                        },
                        LinearRelationTermV1 {
                            witness_index: 4,
                            multiplier: plaintext,
                            witness_automorphism_exponent: 1,
                        },
                    ],
                },
            ],
        },
        vec![secret, public_error, ephemeral, error_zero, error_one],
    )
}

#[test]
fn galois_source_proof_links_the_transformed_secret_and_rejects_every_splice_class() {
    let profile = linear_test_profile();
    let exponent = 3;
    let (statement, witnesses) = tiny_galois_relation_fixture(&profile, exponent);
    statement.validate(&profile).unwrap();
    let context = linear_context(&profile, ZkAmsMkheActiveRoundV1::GaloisSource);
    let witness_refs = witnesses.iter().collect::<Vec<_>>();
    let proof = prove_linear_relation_v1(
        &profile,
        context,
        &statement,
        &witness_refs,
        &mut KatRandom::new(b"galois-source-proof-positive"),
    )
    .unwrap();
    verify_linear_relation_proof(&profile, context, &statement, &proof).unwrap();

    let mut wrong_exponent = statement.clone();
    wrong_exponent.witness_challenge_automorphism_exponents[2..].fill(5);
    wrong_exponent.outputs[1].challenge_automorphism_exponent = 5;
    wrong_exponent.outputs[1].terms[0].witness_automorphism_exponent = 5;
    wrong_exponent.outputs[2].challenge_automorphism_exponent = 5;
    wrong_exponent.validate(&profile).unwrap();
    assert!(verify_linear_relation_proof(&profile, context, &wrong_exponent, &proof).is_err());

    for mutation in 0..5 {
        let mut changed = statement.clone();
        match mutation {
            0 => changed.outputs[1].target.coefficients[0] ^= 1,
            1 => changed.outputs[1].terms[0].multiplier.coefficients[0] ^= 1,
            2 => {
                changed.outputs[0].target.coefficients[1] ^= 1;
                changed.outputs[1].terms[1].multiplier.coefficients[1] ^= 1;
            }
            3 => {
                changed.outputs[0].terms[0].multiplier.coefficients[2] ^= 1;
                changed.outputs[2].terms[0].multiplier.coefficients[2] ^= 1;
            }
            4 => changed.outputs[2].target.coefficients[3] ^= 1,
            _ => unreachable!(),
        }
        assert!(
            verify_linear_relation_proof(&profile, context, &changed, &proof).is_err(),
            "statement splice {mutation} must fail"
        );
    }

    for mutation in 0..4 {
        let mut changed = context;
        match mutation {
            0 => changed.transcript_digest[0] ^= 1,
            1 => changed.party_index += 1,
            2 => changed.party = ZkAmsMkhePartyIdV1::new([0x72; 32]).unwrap(),
            3 => changed.relation_index += 1,
            _ => unreachable!(),
        }
        assert!(verify_linear_relation_proof(&profile, changed, &statement, &proof).is_err());
    }

    let mut changed_proof = proof.clone();
    changed_proof.responses[0][0] += 1;
    assert!(verify_linear_relation_proof(&profile, context, &statement, &changed_proof).is_err());
    let mut changed_proof = proof.clone();
    changed_proof.challenge_seed[0] ^= 1;
    assert!(verify_linear_relation_proof(&profile, context, &statement, &changed_proof).is_err());

    let encoded = proof.encode_wire().unwrap();
    assert!(
        LinearRelationProofV1::decode_wire_exact(
            &encoded[..encoded.len() - 1],
            5,
            profile.ring_degree,
        )
        .is_err()
    );
    let mut trailing = encoded;
    trailing.push(0);
    assert!(LinearRelationProofV1::decode_wire_exact(&trailing, 5, profile.ring_degree).is_err());
}

#[test]
fn galois_source_coordinate_is_exactly_the_frozen_schedule_and_38_digits() {
    let schedule = zk_ams_t256_galois_key_schedule_v1().unwrap();
    assert_eq!(schedule.entries.len(), ZK_AMS_T256_GALOIS_KEY_COUNT_V1);
    for (index, entry) in schedule.entries.iter().enumerate() {
        validate_galois_source_coordinate(index, entry.exponent, 0).unwrap();
        validate_galois_source_coordinate(index, entry.exponent, 37).unwrap();
        assert!(validate_galois_source_coordinate(index, entry.exponent ^ 2, 0).is_err());
    }
    assert!(
        validate_galois_source_coordinate(
            ZK_AMS_T256_GALOIS_KEY_COUNT_V1,
            schedule.entries[0].exponent,
            0,
        )
        .is_err()
    );
    assert!(validate_galois_source_coordinate(0, schedule.entries[0].exponent, 38).is_err());
}

#[test]
fn galois_source_authentication_rejects_party_transcript_round_proof_and_key_mutations() {
    let fixture = fixture(b"galois-source-auth", 97);
    let transcript = keccak256(b"galois-source-auth-transcript");
    let baseline = contributions(
        &fixture,
        ZkAmsMkheActiveRoundV1::GaloisSource,
        transcript,
        b"galois-source-auth-contributions",
    );
    zk_ams_mkhe_collect_active_round_v1(
        &fixture.roster,
        transcript,
        ZkAmsMkheActiveRoundV1::GaloisSource,
        &baseline,
    )
    .unwrap();
    for mutation in 0..6 {
        let mut changed = baseline.clone();
        match mutation {
            0 => changed[0].party = changed[1].party,
            1 => changed[0].contribution_index = 1,
            2 => changed[0].transcript_digest[0] ^= 1,
            3 => changed[0].round = ZkAmsMkheActiveRoundV1::RkgRoundOne,
            4 => changed[0].payload_digest[0] ^= 1,
            5 => changed[0].authentication.public_key = changed[1].authentication.public_key,
            _ => unreachable!(),
        }
        assert!(
            zk_ams_mkhe_collect_active_round_v1(
                &fixture.roster,
                transcript,
                ZkAmsMkheActiveRoundV1::GaloisSource,
                &changed,
            )
            .is_err(),
            "authentication splice {mutation} must fail"
        );
    }
    let mut changed = baseline;
    changed[0].authentication.signature[64] ^= 1;
    assert!(
        zk_ams_mkhe_collect_active_round_v1(
            &fixture.roster,
            transcript,
            ZkAmsMkheActiveRoundV1::GaloisSource,
            &changed,
        )
        .is_err()
    );
}
