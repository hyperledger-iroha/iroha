// Test body included from the parent module to keep its production source budget bounded.
use super::super::super::MaskedRelaxedRandomErrorV1;
use super::super::collective::{
    aggregate_zk_ams_mkhe_collective_public_key_v1, generate_zk_ams_mkhe_collective_party_state_v1,
};
use super::super::sample_uniform_rns;
use super::*;

const TEST_MODULI: [u64; 2] = [2_013_265_921, 1_811_939_329];
const TEST_ROOTS: [u64; 2] = [1_400_279_418, 677_356_115];
const TEST_SMUDGE_BITS: usize = 8;
const TEST_FINAL_RESIDUAL_BITS: usize = 24;

fn test_profile() -> BgvProfile {
    BgvProfile {
        profile_id: [0xd8; 32],
        ring_degree: 8,
        moduli: &TEST_MODULI,
        negacyclic_roots: &TEST_ROOTS,
        plaintext_modulus: PlaintextModulus::Tiny(17),
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
    state: [u8; 32],
    counter: u64,
}

impl KatRandom {
    fn new(label: &[u8]) -> Self {
        Self {
            state: keccak256(label),
            counter: 0,
        }
    }
}

impl MaskedRelaxedRandomSourceV1 for KatRandom {
    fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), MaskedRelaxedRandomErrorV1> {
        let mut cursor = 0;
        while cursor < destination.len() {
            let mut frame = Vec::with_capacity(40);
            frame.extend_from_slice(&self.state);
            frame.extend_from_slice(&self.counter.to_be_bytes());
            let block = shake256(&frame, 64);
            let take = (destination.len() - cursor).min(block.len());
            destination[cursor..cursor + take].copy_from_slice(&block[..take]);
            cursor += take;
            self.counter = self.counter.wrapping_add(1);
        }
        Ok(())
    }
}

struct FailingRandom;

impl MaskedRelaxedRandomSourceV1 for FailingRandom {
    fn fill_bytes(&mut self, _destination: &mut [u8]) -> Result<(), MaskedRelaxedRandomErrorV1> {
        Err(MaskedRelaxedRandomErrorV1::Unavailable)
    }
}

struct ConstantRandom(u8);

impl MaskedRelaxedRandomSourceV1 for ConstantRandom {
    fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), MaskedRelaxedRandomErrorV1> {
        destination.fill(self.0);
        Ok(())
    }
}

struct FastDeterministicRandom(u64);

impl FastDeterministicRandom {
    fn new(label: &[u8]) -> Self {
        let digest = keccak256(label);
        Self(u64::from_be_bytes(digest[..8].try_into().unwrap()))
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut value = self.0;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }
}

impl MaskedRelaxedRandomSourceV1 for FastDeterministicRandom {
    fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), MaskedRelaxedRandomErrorV1> {
        for chunk in destination.chunks_mut(8) {
            let block = self.next_u64().to_be_bytes();
            chunk.copy_from_slice(&block[..chunk.len()]);
        }
        Ok(())
    }
}

struct Fixture {
    profile: BgvProfile,
    parties: super::super::PartySet,
    authentication: Vec<AuthenticationSecret>,
    relations: Vec<DecryptionPublicRelationV1>,
    secrets: Vec<SecretPolynomial>,
    errors: Vec<SecretPolynomial>,
    ciphertext: ZkAmsMkheCollectiveCiphertextV1,
    message: Vec<u64>,
}

fn fixture(label: &[u8]) -> Fixture {
    let profile = test_profile();
    profile.validate().unwrap();
    let mut random = KatRandom::new(label);
    let mut authentication = (0..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)
        .map(|_| AuthenticationSecret::generate(&mut random).unwrap())
        .collect::<Vec<_>>();
    authentication.sort_by_key(|secret| secret.party_id().unwrap());
    let parties = super::super::PartySet::new(
        authentication
            .iter()
            .map(|secret| secret.party_id().unwrap())
            .collect(),
    )
    .unwrap();
    let common_a = sample_uniform_rns(&profile, &mut random).unwrap();
    let mut secrets = Vec::with_capacity(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1);
    let mut errors = Vec::with_capacity(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1);
    let mut party_bs = Vec::with_capacity(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1);
    let mut collective_b = RnsPolynomial::zero(&profile);
    for _ in 0..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 {
        let secret = SecretPolynomial::sample_ternary(&profile, &mut random).unwrap();
        let error = SecretPolynomial::sample_error(&profile, &mut random).unwrap();
        let party_b = common_a
            .mul(&secret.as_rns(&profile).unwrap(), &profile)
            .unwrap()
            .negate(&profile)
            .unwrap()
            .add(
                &error
                    .as_rns(&profile)
                    .unwrap()
                    .scale_plaintext_modulus(&profile)
                    .unwrap(),
                &profile,
            )
            .unwrap();
        collective_b = collective_b.add(&party_b, &profile).unwrap();
        secrets.push(secret);
        errors.push(error);
        party_bs.push(party_b);
    }
    let message = vec![0, 1, 2, 3, 8, 9, 15, 16];
    let message_rns = RnsPolynomial::from_test_plaintext(&profile, &message).unwrap();
    let ephemeral = SecretPolynomial::sample_ternary(&profile, &mut random).unwrap();
    let error_zero = SecretPolynomial::sample_error(&profile, &mut random).unwrap();
    let error_one = SecretPolynomial::sample_error(&profile, &mut random).unwrap();
    let ephemeral_rns = ephemeral.as_rns(&profile).unwrap();
    let constant = collective_b
        .mul(&ephemeral_rns, &profile)
        .unwrap()
        .add(
            &error_zero
                .as_rns(&profile)
                .unwrap()
                .scale_plaintext_modulus(&profile)
                .unwrap(),
            &profile,
        )
        .unwrap()
        .add(&message_rns, &profile)
        .unwrap();
    let linear = common_a
        .mul(&ephemeral_rns, &profile)
        .unwrap()
        .add(
            &error_one
                .as_rns(&profile)
                .unwrap()
                .scale_plaintext_modulus(&profile)
                .unwrap(),
            &profile,
        )
        .unwrap();
    let transcript_digest = keccak256(&[label, b".transcript"].concat());
    let epoch = 41;
    let ciphertext = ZkAmsMkheCollectiveCiphertextV1::new(
        &profile,
        &parties,
        epoch,
        transcript_digest,
        73,
        1,
        constant,
        linear,
    )
    .unwrap();
    let roster_digest = ciphertext.roster_digest();
    // Make the digest available to every exact per-party binding.
    let ciphertext_digest = ciphertext.digest();
    let key_context_digest = keccak256(&[label, b".key-context"].concat());
    let statement_binding_digest = keccak256(&[label, b".statement-binding"].concat());
    let mut relations = Vec::with_capacity(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1);
    for (index, party_b) in party_bs.into_iter().enumerate() {
        let binding = DecryptionBindingV1 {
            profile_digest: profile.digest().unwrap(),
            roster_digest,
            epoch: ciphertext.epoch(),
            transcript_digest,
            ciphertext_digest,
            key_context_digest,
            statement_binding_digest,
            ciphertext_record_index: 0,
            sample_index: ciphertext.sample_index(),
            party_index: u8::try_from(index).unwrap(),
            party: parties.parties[index],
            level: ciphertext.level(),
        };
        relations.push(DecryptionPublicRelationV1 {
            binding: binding.clone(),
            common_a: Arc::new(common_a.clone()),
            party_b,
        });
    }
    ciphertext.validate(&profile, &parties).unwrap();
    Fixture {
        profile,
        parties,
        authentication,
        relations,
        secrets,
        errors,
        ciphertext,
        message,
    }
}

fn make_shares(fixture: &Fixture, label: &[u8]) -> Vec<AuthenticatedDecryptionShareV1> {
    let mut random = KatRandom::new(label);
    (0..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)
        .map(|index| {
            let witness = DecryptionPartyWitnessV1 {
                binding: fixture.relations[index].binding.clone(),
                secret: &fixture.secrets[index],
                public_key_error: &fixture.errors[index],
            };
            create_authenticated_decryption_share(
                &fixture.profile,
                &fixture.parties,
                &fixture.relations[index],
                &witness,
                &fixture.ciphertext,
                &fixture.authentication[index],
                TEST_SMUDGE_BITS,
                &mut random,
            )
            .unwrap()
        })
        .collect()
}

struct PublicReleaseProvingFixture {
    party_secrets: Vec<ZkAmsMkheActivePartySecretV1>,
    party_states: Vec<ZkAmsMkheCollectivePartyStateV1>,
    public_key_shares: Vec<ZkAmsMkheCollectivePublicKeyShareV1>,
    collective_public_key: ZkAmsMkheCollectivePublicKeyV1,
    roster: ZkAmsMkheGovernedRosterWireV1,
    ciphertext: ZkAmsMkheCollectiveCiphertextWireV1,
}

fn public_release_proving_fixture() -> &'static PublicReleaseProvingFixture {
    static FIXTURE: std::sync::OnceLock<PublicReleaseProvingFixture> = std::sync::OnceLock::new();
    FIXTURE.get_or_init(|| {
        let mut random = FastDeterministicRandom::new(b"decryption-public-reachability-setup");
        let mut party_secrets = (0..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)
            .map(|_| ZkAmsMkheActivePartySecretV1::generate(&mut random).unwrap())
            .collect::<Vec<_>>();
        party_secrets.sort_by_key(|secret| secret.party().unwrap());
        let ordered_secrets: [&ZkAmsMkheActivePartySecretV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1] =
            std::array::from_fn(|index| &party_secrets[index]);
        let governed_roster = super::super::active::ZkAmsMkheGovernedActiveRosterV1::new(
            0xdec0_de01,
            ordered_secrets,
            &mut random,
        )
        .unwrap();
        let roster = governed_roster.to_wire_roster().unwrap();
        let transcript_digest = keccak256(b"decryption-public-reachability.transcript");
        // Each contribution has a disjoint witness and deterministic test
        // stream. Build all eight concurrently so the release-size fixture
        // does not serialize eight otherwise independent native proofs.
        let generated = std::thread::scope(|scope| {
            let handles = (0..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)
                .map(|party_index| {
                    let governed_roster = &governed_roster;
                    let party_secret = &party_secrets[party_index];
                    scope.spawn(move || {
                        let seed = [
                            b"decryption-public-reachability-party".as_slice(),
                            &[u8::try_from(party_index).unwrap()],
                        ]
                        .concat();
                        let mut party_random = FastDeterministicRandom::new(&seed);
                        generate_zk_ams_mkhe_collective_party_state_v1(
                            governed_roster,
                            transcript_digest,
                            party_index,
                            party_secret,
                            &mut party_random,
                        )
                    })
                })
                .collect::<Vec<_>>();
            handles
                .into_iter()
                .map(|handle| handle.join().unwrap().unwrap())
                .collect::<Vec<_>>()
        });
        let (party_states, public_key_shares): (Vec<_>, Vec<_>) = generated.into_iter().unzip();
        let share_references: [&ZkAmsMkheCollectivePublicKeyShareV1;
            ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1] =
            std::array::from_fn(|index| &public_key_shares[index]);
        let collective_public_key = aggregate_zk_ams_mkhe_collective_public_key_v1(
            &governed_roster,
            transcript_digest,
            share_references,
        )
        .unwrap();
        let binding = ZkAmsMkheWireBindingV1::new(
            &roster,
            keccak256(b"decryption-public-reachability.ciphertext-lineage"),
            0,
            1,
        )
        .unwrap();
        let common_a = public_key_shares[0].public_a().clone();
        let ciphertext = ZkAmsMkheCollectiveCiphertextWireV1::new(
            binding,
            0x0051_a2e0,
            common_a.clone(),
            common_a,
        )
        .unwrap();
        PublicReleaseProvingFixture {
            party_secrets,
            party_states,
            public_key_shares,
            collective_public_key,
            roster,
            ciphertext,
        }
    })
}

fn public_release_statement<'a>(
    fixture: &'a PublicReleaseProvingFixture,
    public_key_shares: &'a [&'a ZkAmsMkheCollectivePublicKeyShareV1;
            ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1],
) -> ZkAmsMkheDecryptionStatementV1<'a> {
    ZkAmsMkheDecryptionStatementV1::new(
        &fixture.roster,
        &fixture.ciphertext,
        &fixture.collective_public_key,
        public_key_shares,
    )
    .unwrap()
}

#[test]
fn exact_wide_sign_magnitude_boundaries_are_canonical_and_no_wrap() {
    let maximum = WideMagnitudeV1::max_for_bits(1_855).unwrap();
    assert_eq!(maximum.bit_len(), 1_855);
    for value in [
        SignedWideV1::zero(),
        SignedWideV1::new(false, maximum.clone()).unwrap(),
        SignedWideV1::new(true, maximum.clone()).unwrap(),
    ] {
        let encoded = value.encode_fixed(232).unwrap();
        assert_eq!(encoded.len(), 232);
        assert_eq!(SignedWideV1::decode_fixed(&encoded).unwrap(), value);
        for modulus in TEST_MODULI {
            assert!(value.mod_u64(modulus) < modulus);
        }
    }
    let mut negative_zero = vec![0_u8; 232];
    negative_zero[0] = 0x80;
    assert_eq!(
        SignedWideV1::decode_fixed(&negative_zero),
        Err(ZkAmsMkheErrorV1::InvalidWireEncoding)
    );
    let overflow = SignedWideV1::new(false, WideMagnitudeV1::max_for_bits(1_856).unwrap()).unwrap();
    assert_eq!(
        overflow.encode_fixed(232),
        Err(ZkAmsMkheErrorV1::InvalidWireEncoding)
    );
}

#[test]
fn t256_wide_reduction_and_centering_boundaries_are_exact() {
    let modulus = wide_from_be(&super::super::VEGA_T256_SCALAR_MODULUS_BE_V1).unwrap();
    let one = wide_from_u64(1);
    let modulus_minus_one = modulus.checked_sub(one).unwrap();
    let modulus_plus_one = wide_checked_add(modulus, one).unwrap();
    let twice_modulus = wide_checked_add(modulus, modulus).unwrap();
    assert_eq!(reduce_wide_mod_t256(WideUint::zero()), [0; 32]);
    let mut one_be = [0_u8; 32];
    one_be[31] = 1;
    assert_eq!(reduce_wide_mod_t256(one), one_be);
    assert_eq!(
        reduce_wide_mod_t256(modulus_minus_one),
        t256_subtract_modulus(one_be).unwrap()
    );
    assert_eq!(reduce_wide_mod_t256(modulus), [0; 32]);
    assert_eq!(reduce_wide_mod_t256(modulus_plus_one), one_be);
    assert_eq!(reduce_wide_mod_t256(twice_modulus), [0; 32]);

    let negative_one = SignedCrtV1::normalized(true, one);
    let canonical = t256_subtract_modulus(one_be).unwrap();
    let reduced = reduce_wide_mod_t256(negative_one.magnitude);
    assert_eq!(t256_subtract_modulus(reduced).unwrap(), canonical);
    assert!(canonical > super::super::T256_CENTERED_MAX_BE_V1);
}

#[test]
fn unavailable_zero_and_constant_random_sources_fail_closed() {
    assert_eq!(
        validate_wide_relation_random_health(&mut FailingRandom),
        Err(ZkAmsMkheErrorV1::RandomUnavailable)
    );
    assert_eq!(
        validate_wide_relation_random_health(&mut ConstantRandom(0)),
        Err(ZkAmsMkheErrorV1::RandomUnavailable)
    );
    assert_eq!(
        validate_wide_relation_random_health(&mut ConstantRandom(0xa5)),
        Err(ZkAmsMkheErrorV1::RandomUnavailable)
    );
    let bound = WideMagnitudeV1::max_for_bits(TEST_SMUDGE_BITS).unwrap();
    assert_eq!(
        sample_signed_wide(&bound, &mut FailingRandom),
        Err(ZkAmsMkheErrorV1::RandomUnavailable)
    );
}

#[test]
fn public_generated_opaque_state_reaches_native_prove_and_verify_end_to_end() {
    let fixture = public_release_proving_fixture();
    let public_key_shares: [&ZkAmsMkheCollectivePublicKeyShareV1;
        ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1] =
        std::array::from_fn(|index| &fixture.public_key_shares[index]);
    let statement = public_release_statement(fixture, &public_key_shares);
    let mut random = FastDeterministicRandom::new(b"decryption-public-reachability-proof");
    let share = prove_zk_ams_mkhe_decryption_share_v1(
        statement,
        0,
        &fixture.party_states[0],
        &fixture.party_secrets[0],
        &mut random,
    )
    .unwrap();
    assert_eq!(share.party_index(), 0);
    assert_eq!(share.party(), fixture.roster.parties()[0]);
    verify_zk_ams_mkhe_decryption_share_v1(statement, &share).unwrap();

    let transport = split_zk_ams_mkhe_decryption_share_v1(statement, &share).unwrap();
    let manifest = transport.manifest_bytes().unwrap();
    println!(
        "ZK_AMS_MKHE_DECRYPTION_SPLIT_RELEASE_KAT_DIGEST_V1={}",
        hex::encode(transport.manifest().manifest_digest())
    );
    assert_eq!(
        transport.manifest().manifest_digest(),
        ZK_AMS_MKHE_DECRYPTION_SPLIT_RELEASE_KAT_DIGEST_V1
    );
    assert_eq!(
        manifest.len(),
        ZK_AMS_MKHE_DECRYPTION_SPLIT_MANIFEST_BYTES_V1
    );
    assert_eq!(transport.polynomial_object().len(), 39_845_892);
    assert_eq!(transport.proof_envelope().len(), 33_030_199);
    assert!(transport.polynomial_object().len() <= 64 * 1024 * 1024);
    assert!(transport.proof_envelope().len() <= 32 * 1024 * 1024);
    let components = transport.ordered_components();
    let reconstructed =
        reconstruct_zk_ams_mkhe_decryption_share_v1(statement, &manifest, &components).unwrap();
    assert_eq!(reconstructed, share);
    verify_zk_ams_mkhe_decryption_share_v1(statement, &reconstructed).unwrap();

    // Every manifest statement/binding/authentication axis is covered by
    // the signed manifest digest. These offsets are fixed by the 498-byte
    // `ZDSM` layout and deliberately exercise each field independently.
    for offset in [
        0_usize, // tag
        4_usize, // version
        5,       // profile
        37,      // roster
        69,      // epoch
        77,      // transcript
        109,     // ciphertext digest
        141,     // collective-key context
        173,     // exact public statement binding
        205,     // ciphertext record index
        209,     // sample index
        217,     // party index
        218,     // party id
        250,     // level
        251,     // component count
        254,     // polynomial exact byte length
        262,     // polynomial BLAKE3 digest
        296,     // proof exact byte length
        304,     // proof BLAKE3 digest
        336,     // stored manifest digest
        368,     // authenticated party
        400,     // authentication public key
        433,     // authentication signature
    ] {
        let mut changed = manifest.clone();
        changed[offset] ^= 1;
        assert!(
            reconstruct_zk_ams_mkhe_decryption_share_v1(statement, &changed, &components,).is_err(),
            "manifest mutation offset {offset} must reject"
        );
    }

    // Kind and ordinal swaps are rejected during the small-header
    // preflight, before authentication hashing or component processing.
    for offset in [252_usize, 253, 294, 295] {
        let mut changed = manifest.clone();
        changed[offset] ^= 1;
        assert!(ZkAmsMkheDecryptionTransportManifestV1::decode_exact(statement, &changed).is_err());
    }
    let mut reordered_pointers = manifest.clone();
    let first_pointer = reordered_pointers[252..294].to_vec();
    let second_pointer = reordered_pointers[294..336].to_vec();
    reordered_pointers[252..294].copy_from_slice(&second_pointer);
    reordered_pointers[294..336].copy_from_slice(&first_pointer);
    assert!(
        ZkAmsMkheDecryptionTransportManifestV1::decode_exact(statement, &reordered_pointers,)
            .is_err()
    );
    let mut duplicate_pointer = manifest.clone();
    duplicate_pointer[294..336].copy_from_slice(&first_pointer);
    assert!(
        ZkAmsMkheDecryptionTransportManifestV1::decode_exact(statement, &duplicate_pointer,)
            .is_err()
    );

    // Manifest authentication is resolved before even the component-list
    // cardinality is inspected, hence before either large object can be
    // hashed or decoded.
    let mut forged_authentication = manifest.clone();
    *forged_authentication.last_mut().unwrap() ^= 1;
    assert_eq!(
        reconstruct_zk_ams_mkhe_decryption_share_v1(statement, &forged_authentication, &[],),
        Err(ZkAmsMkheErrorV1::InvalidAuthentication)
    );

    assert!(reconstruct_zk_ams_mkhe_decryption_share_v1(statement, &manifest, &[]).is_err());
    assert!(
        reconstruct_zk_ams_mkhe_decryption_share_v1(
            statement,
            &manifest,
            &[transport.polynomial_object()],
        )
        .is_err()
    );
    assert!(
        reconstruct_zk_ams_mkhe_decryption_share_v1(
            statement,
            &manifest,
            &[transport.polynomial_object(), transport.polynomial_object(),],
        )
        .is_err()
    );
    assert!(
        reconstruct_zk_ams_mkhe_decryption_share_v1(
            statement,
            &manifest,
            &[transport.proof_envelope(), transport.polynomial_object(),],
        )
        .is_err()
    );
    assert!(
        reconstruct_zk_ams_mkhe_decryption_share_v1(
            statement,
            &manifest,
            &[
                transport.polynomial_object(),
                transport.proof_envelope(),
                transport.proof_envelope(),
            ],
        )
        .is_err()
    );

    for (object_index, object) in components.into_iter().enumerate() {
        let truncated_components: [&[u8]; 2] = if object_index == 0 {
            [&object[..object.len() - 1], transport.proof_envelope()]
        } else {
            [transport.polynomial_object(), &object[..object.len() - 1]]
        };
        assert!(
            reconstruct_zk_ams_mkhe_decryption_share_v1(
                statement,
                &manifest,
                &truncated_components,
            )
            .is_err()
        );
        let mut extended = object.to_vec();
        extended.push(0);
        let extended_components: [&[u8]; 2] = if object_index == 0 {
            [&extended, transport.proof_envelope()]
        } else {
            [transport.polynomial_object(), &extended]
        };
        assert!(
                reconstruct_zk_ams_mkhe_decryption_share_v1(
                    statement,
                    &manifest,
                    &extended_components,
                )
                .is_err()
            );
        let mut mutated = object.to_vec();
        *mutated.last_mut().unwrap() ^= 1;
        let mutated_components: [&[u8]; 2] = if object_index == 0 {
            [&mutated, transport.proof_envelope()]
        } else {
            [transport.polynomial_object(), &mutated]
        };
        assert!(
            reconstruct_zk_ams_mkhe_decryption_share_v1(statement, &manifest, &mutated_components,)
                .is_err()
        );
    }
    assert!(
        reconstruct_zk_ams_mkhe_decryption_share_v1(
            statement,
            &manifest[..manifest.len() - 1],
            &transport.ordered_components(),
        )
        .is_err()
    );
    let mut extended_manifest = manifest.clone();
    extended_manifest.push(0);
    assert!(
        reconstruct_zk_ams_mkhe_decryption_share_v1(
            statement,
            &extended_manifest,
            &transport.ordered_components(),
        )
        .is_err()
    );

    let wrong_binding = ZkAmsMkheWireBindingV1::new(
        &fixture.roster,
        keccak256(b"decryption-public-reachability.wrong-operation-transcript"),
        fixture.ciphertext.binding().record_index() + 1,
        fixture.ciphertext.binding().level(),
    )
    .unwrap();
    let wrong_ciphertext = ZkAmsMkheCollectiveCiphertextWireV1::new(
        wrong_binding,
        fixture.ciphertext.sample_index(),
        fixture.ciphertext.constant().clone(),
        fixture.ciphertext.linear().clone(),
    )
    .unwrap();
    let wrong_statement = ZkAmsMkheDecryptionStatementV1::new(
        &fixture.roster,
        &wrong_ciphertext,
        &fixture.collective_public_key,
        &public_key_shares,
    )
    .unwrap();
    assert!(
        reconstruct_zk_ams_mkhe_decryption_share_v1(
            wrong_statement,
            &manifest,
            &transport.ordered_components(),
        )
        .is_err()
    );

    let wrong_sample_ciphertext = ZkAmsMkheCollectiveCiphertextWireV1::new(
        fixture.ciphertext.binding(),
        fixture.ciphertext.sample_index() + 1,
        fixture.ciphertext.constant().clone(),
        fixture.ciphertext.linear().clone(),
    )
    .unwrap();
    let wrong_sample_statement = ZkAmsMkheDecryptionStatementV1::new(
        &fixture.roster,
        &wrong_sample_ciphertext,
        &fixture.collective_public_key,
        &public_key_shares,
    )
    .unwrap();
    assert!(
        reconstruct_zk_ams_mkhe_decryption_share_v1(
            wrong_sample_statement,
            &manifest,
            &transport.ordered_components(),
        )
        .is_err()
    );
}

#[test]
fn public_prover_rejects_wrong_opaque_active_secret_and_roster_slot_before_rng() {
    let fixture = public_release_proving_fixture();
    let public_key_shares: [&ZkAmsMkheCollectivePublicKeyShareV1;
        ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1] =
        std::array::from_fn(|index| &fixture.public_key_shares[index]);
    let statement = public_release_statement(fixture, &public_key_shares);
    assert_eq!(
        prove_zk_ams_mkhe_decryption_share_v1(
            statement,
            0,
            &fixture.party_states[0],
            &fixture.party_secrets[1],
            &mut FailingRandom,
        ),
        Err(ZkAmsMkheErrorV1::InvalidAuthentication)
    );
    assert_eq!(
        prove_zk_ams_mkhe_decryption_share_v1(
            statement,
            ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1,
            &fixture.party_states[0],
            &fixture.party_secrets[0],
            &mut FailingRandom,
        ),
        Err(ZkAmsMkheErrorV1::InvalidPartySet)
    );
    assert_eq!(
        prove_zk_ams_mkhe_decryption_share_v1(
            statement,
            1,
            &fixture.party_states[0],
            &fixture.party_secrets[1],
            &mut FailingRandom,
        ),
        Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
    );
    assert_eq!(
        prove_zk_ams_mkhe_decryption_share_v1(
            statement,
            0,
            &fixture.party_states[0],
            &fixture.party_secrets[0],
            &mut ConstantRandom(0),
        ),
        Err(ZkAmsMkheErrorV1::RandomUnavailable)
    );
}

#[test]
fn opaque_state_and_verified_key_context_reject_every_splice_before_rng() {
    let fixture = public_release_proving_fixture();
    let public_key_shares: [&ZkAmsMkheCollectivePublicKeyShareV1;
        ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1] =
        std::array::from_fn(|index| &fixture.public_key_shares[index]);
    let statement = public_release_statement(fixture, &public_key_shares);
    let expected = expected_opaque_party_state_context(statement, 0).unwrap();
    validate_opaque_party_state_context(&fixture.party_states[0], expected).unwrap();

    for axis in 0..9 {
        let mut changed = expected;
        match axis {
            0 => changed.profile_digest[0] ^= 1,
            1 => changed.security_certificate_digest[0] ^= 1,
            2 => changed.roster_digest[0] ^= 1,
            3 => changed.key_material_digest[0] ^= 1,
            4 => changed.epoch += 1,
            5 => changed.transcript_digest[0] ^= 1,
            6 => changed.party_index = 1,
            7 => changed.party = fixture.roster.parties()[1],
            8 => changed.public_share_digest[0] ^= 1,
            _ => unreachable!(),
        }
        assert_eq!(
            validate_opaque_party_state_context(&fixture.party_states[0], changed),
            Err(ZkAmsMkheErrorV1::InvalidKeyMaterial),
            "opaque state context axis {axis} must reject"
        );
    }

    let mut swapped_public_key_shares = public_key_shares;
    swapped_public_key_shares.swap(0, 1);
    assert!(matches!(
        ZkAmsMkheDecryptionStatementV1::new(
            &fixture.roster,
            &fixture.ciphertext,
            &fixture.collective_public_key,
            &swapped_public_key_shares,
        ),
        Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
    ));

    let (profile, parties, ciphertext, common_a, party_b) =
        statement.internal_for_party(0).unwrap();
    let binding = DecryptionBindingV1 {
        profile_digest: statement.roster.profile_digest(),
        roster_digest: statement.roster.roster_digest(),
        epoch: statement.roster.epoch(),
        transcript_digest: statement.ciphertext.binding().transcript_digest(),
        ciphertext_digest: ciphertext.digest(),
        key_context_digest: statement.key_context_digest,
        statement_binding_digest: statement.binding_digest(),
        ciphertext_record_index: statement.ciphertext.binding().record_index(),
        sample_index: statement.ciphertext.sample_index(),
        party_index: 0,
        party: statement.roster.parties()[0],
        level: statement.ciphertext.binding().level(),
    };
    let witness = DecryptionPartyWitnessV1 {
        binding: binding.clone(),
        secret: fixture.party_states[0].secret(),
        public_key_error: fixture.party_states[0].public_error(),
    };
    let relation = DecryptionPublicRelationV1 {
        binding: binding.clone(),
        common_a: Arc::new(common_a.clone()),
        party_b: party_b.clone(),
    };
    validate_party_witness(&profile, &parties, &relation, &witness).unwrap();

    let mut wrong_a = common_a;
    wrong_a.coefficients[0] = (wrong_a.coefficients[0] + 1) % profile.moduli[0];
    let wrong_a_relation = DecryptionPublicRelationV1 {
        binding: binding.clone(),
        common_a: Arc::new(wrong_a),
        party_b: party_b.clone(),
    };
    assert_eq!(
        create_decryption_share(
            &profile,
            &parties,
            &wrong_a_relation,
            &witness,
            &ciphertext,
            usize::from(
                zk_ams_mkhe_noise_certificate_v1()
                    .unwrap()
                    .decryption_smudge_quotient_bits,
            ),
            &mut FailingRandom,
        ),
        Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
    );

    let mut wrong_b = party_b;
    wrong_b.coefficients[0] = (wrong_b.coefficients[0] + 1) % profile.moduli[0];
    let wrong_b_relation = DecryptionPublicRelationV1 {
        binding,
        common_a: relation.common_a,
        party_b: wrong_b,
    };
    assert_eq!(
        create_decryption_share(
            &profile,
            &parties,
            &wrong_b_relation,
            &witness,
            &ciphertext,
            usize::from(
                zk_ams_mkhe_noise_certificate_v1()
                    .unwrap()
                    .decryption_smudge_quotient_bits,
            ),
            &mut FailingRandom,
        ),
        Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
    );
}

#[test]
fn complete_eight_party_native_decryption_kat_recovers_plaintext() {
    let fixture = fixture(b"decryption-positive-kat");
    let shares = make_shares(&fixture, b"decryption-positive-shares");
    let result = aggregate_and_decrypt_full_roster(
        &fixture.profile,
        &fixture.parties,
        &fixture.relations,
        &fixture.ciphertext,
        &shares,
        TEST_SMUDGE_BITS,
        TEST_FINAL_RESIDUAL_BITS,
    )
    .unwrap();
    assert_eq!(
        result.plaintext,
        DecryptedPlaintextV1::Tiny(fixture.message)
    );
    assert_eq!(
        result.ordered_share_set_digest,
        [
            0x29, 0xa8, 0x11, 0x71, 0x08, 0x12, 0x26, 0x8b, 0x3d, 0x6f, 0x34, 0xc5, 0x39, 0x5f,
            0x2d, 0xf2, 0xc3, 0x28, 0x17, 0xef, 0x42, 0xaa, 0x2f, 0x70, 0xce, 0x1e, 0xdf, 0x1b,
            0x1b, 0xfb, 0x2c, 0x28,
        ]
    );
    assert!(result.maximum_residual_bits <= TEST_FINAL_RESIDUAL_BITS as u16);
    assert_ne!(result.ordered_share_set_digest, [0; 32]);
}

#[test]
fn canonical_share_wire_roundtrip_rehashes_and_reverifies_every_relation() {
    let fixture = fixture(b"decryption-wire-kat");
    let shares = make_shares(&fixture, b"decryption-wire-shares");
    for (index, share) in shares.iter().enumerate() {
        let encoded = share.encode(&fixture.profile, TEST_SMUDGE_BITS).unwrap();
        if index == 0 {
            assert_eq!(
                keccak256(&encoded),
                [
                    0x53, 0x7a, 0xd4, 0x71, 0xe1, 0xff, 0xed, 0xc2, 0x8c, 0x0a, 0x91, 0xf7, 0xce,
                    0x31, 0x99, 0xce, 0x26, 0x61, 0x90, 0x07, 0xff, 0xde, 0xef, 0x8b, 0x19, 0x8f,
                    0x05, 0xfa, 0x0a, 0xa1, 0xac, 0xea,
                ]
            );
        }
        let decoded = AuthenticatedDecryptionShareV1::decode_exact(
            &encoded,
            &fixture.profile,
            &fixture.parties,
            &fixture.relations[index].binding,
            TEST_SMUDGE_BITS,
        )
        .unwrap();
        assert_eq!(decoded, *share);
        assert_eq!(
            decoded.encode(&fixture.profile, TEST_SMUDGE_BITS).unwrap(),
            encoded
        );
        verify_authenticated_share(
            &fixture.profile,
            &fixture.parties,
            &fixture.relations[index],
            &fixture.ciphertext,
            &fixture.relations[index].binding,
            &decoded,
            TEST_SMUDGE_BITS,
        )
        .unwrap();
    }
}

#[test]
fn release_resource_evidence_is_exact_for_split_transport() {
    let evidence = zk_ams_mkhe_decryption_resource_evidence_v1().unwrap();
    evidence.validate().unwrap();
    assert_eq!(evidence.ring_degree, 131_072);
    assert_eq!(evidence.rns_limb_count, 38);
    assert_eq!(evidence.roster_size, 8);
    assert_eq!(evidence.smudge_quotient_bits, 1_855);
    assert_eq!(evidence.challenge_weight, 20);
    assert_eq!(evidence.challenge_space_lower_bound_bits, 260);
    assert_eq!(evidence.statistical_security_bits, 128);
    assert_eq!(evidence.mask_slack_log2, 24);
    assert_eq!(evidence.wide_response_coefficient_bytes, 236);
    assert_eq!(evidence.share_polynomial_bytes, 39_845_892);
    assert_eq!(
        evidence.secret_response_bytes,
        131_072 * DECRYPTION_SIGNED_SMALL_BYTES_V1 as u64
    );
    assert_eq!(
        evidence.public_key_error_response_bytes,
        evidence.secret_response_bytes
    );
    assert_eq!(evidence.smudge_response_bytes, 30_932_992);
    assert_eq!(evidence.proof_header_bytes, 55);
    assert_eq!(evidence.proof_payload_bytes, 33_030_199);
    assert_eq!(
        evidence.governed_proof_payload_ceiling_bytes,
        32 * 1024 * 1024
    );
    assert_eq!(evidence.proof_payload_headroom_bytes, 524_233);
    assert!(evidence.proof_payload_ceiling_met);
    assert_eq!(evidence.split_polynomial_object_bytes, 39_845_892);
    assert_eq!(evidence.split_proof_envelope_bytes, 33_030_199);
    assert_eq!(evidence.split_manifest_bytes, 498);
    assert_eq!(evidence.split_polynomial_headroom_bytes, 27_262_972);
    assert_eq!(evidence.split_proof_headroom_bytes, 524_233);
    assert!(evidence.split_component_ceilings_met);
    assert_eq!(
        evidence.split_release_kat_digest,
        ZK_AMS_MKHE_DECRYPTION_SPLIT_RELEASE_KAT_DIGEST_V1
    );
    assert_ne!(evidence.split_release_kat_digest, [0; 32]);
    assert!(evidence.split_transport_ready);
    assert_eq!(evidence.record_overhead_bytes, 432);
    assert_eq!(evidence.total_share_record_bytes, 72_876_523);
    assert_eq!(evidence.governed_share_ceiling_bytes, 64 * 1024 * 1024);
    assert!(!evidence.share_ceiling_met);
    assert_eq!(evidence.ceiling_shortfall_bytes, 5_767_659);
    assert_eq!(
        evidence.minimum_sound_share_ceiling_bytes,
        evidence.total_share_record_bytes
    );
    assert_eq!(
        evidence.evidence_digest,
        [
            0x1a, 0x67, 0x05, 0xd1, 0xad, 0x09, 0xd0, 0x0c, 0x7f, 0x90, 0x2d, 0xf1, 0x9b, 0xff,
            0xe8, 0xc2, 0x37, 0x35, 0x93, 0x5d, 0x5b, 0x0c, 0x4f, 0x2d, 0xea, 0xd4, 0xb5, 0xf0,
            0x25, 0xd6, 0xc3, 0x67,
        ]
    );
}

#[test]
fn release_split_objects_have_exact_canonical_sizes_and_preflight() {
    let profile = release_profile_v1();
    let evidence = zk_ams_mkhe_decryption_resource_evidence_v1().unwrap();
    let polynomial = RnsPolynomial::zero(&profile);
    let polynomial_object = encode_decryption_polynomial_object(&profile, &polynomial).unwrap();
    assert_eq!(polynomial_object.len(), 39_845_892);
    assert!(polynomial_object.len() <= profile.max_share_bytes);
    assert_eq!(
        decode_decryption_polynomial_object(&polynomial_object).unwrap(),
        polynomial
    );
    assert!(
        decode_decryption_polynomial_object(&polynomial_object[..polynomial_object.len() - 1])
            .is_err()
    );
    let mut extended_polynomial = polynomial_object.clone();
    extended_polynomial.push(0);
    assert!(decode_decryption_polynomial_object(&extended_polynomial).is_err());
    drop(extended_polynomial);
    let mut wrong_polynomial_count = polynomial_object.clone();
    wrong_polynomial_count[..4].copy_from_slice(&u32::MAX.to_be_bytes());
    assert!(decode_decryption_polynomial_object(&wrong_polynomial_count).is_err());
    drop(wrong_polynomial_count);
    let mut noncanonical_polynomial = polynomial_object.clone();
    noncanonical_polynomial[4..12].copy_from_slice(&profile.moduli[0].to_be_bytes());
    assert!(decode_decryption_polynomial_object(&noncanonical_polynomial).is_err());
    drop(noncanonical_polynomial);

    let proof = DecryptionRelationProofV1 {
        wide_response_bytes: evidence.wide_response_coefficient_bytes,
        challenge_seed: [0x5a; 32],
        secret_response: vec![0; profile.ring_degree],
        public_key_error_response: vec![0; profile.ring_degree],
        smudge_response: vec![SignedWideV1::zero(); profile.ring_degree],
    };
    let proof_envelope = proof.encode().unwrap();
    assert_eq!(proof_envelope.len(), 33_030_199);
    assert!(proof_envelope.len() <= ZK_AMS_MKHE_MAX_PROOF_BYTES_V1);
    let decoded = ZkAmsMkheDecryptionProofV1::decode_release_exact(&proof_envelope).unwrap();
    assert_eq!(decoded, proof);
    assert!(matches!(
        ZkAmsMkheDecryptionProofV1::decode_release_exact(
            &proof_envelope[..proof_envelope.len() - 1]
        ),
        Err(ZkAmsMkheErrorV1::InvalidWireEncoding)
    ));
    let mut extended_proof = proof_envelope.clone();
    extended_proof.push(0);
    assert!(ZkAmsMkheDecryptionProofV1::decode_release_exact(&extended_proof).is_err());
    drop(extended_proof);
    for offset in [
        0_usize, // tag
        4,       // version
        5,       // fixed wide-response width
        7,       // ring degree
        43,      // secret-response count
        47,      // public-error-response count
        51,      // smudge-response count
    ] {
        let mut malformed = proof_envelope.clone();
        malformed[offset] ^= 1;
        assert!(
            ZkAmsMkheDecryptionProofV1::decode_release_exact(&malformed).is_err(),
            "proof header mutation offset {offset} must reject"
        );
    }
    let mut negative_zero = proof_envelope.clone();
    let first_wide_response = DECRYPTION_PROOF_HEADER_BYTES_V1
        + profile.ring_degree * 2 * DECRYPTION_SIGNED_SMALL_BYTES_V1;
    negative_zero
        [first_wide_response..first_wide_response + usize::from(proof.wide_response_bytes)]
        .fill(0);
    negative_zero[first_wide_response] = 0x80;
    assert!(ZkAmsMkheDecryptionProofV1::decode_release_exact(&negative_zero).is_err());
    let polynomial_pointer = ZkAmsMkheDecryptionTransportPointerV1::from_payload(
        ZkAmsMkheDecryptionTransportComponentKindV1::SharePolynomial,
        &polynomial_object,
    )
    .unwrap();
    let proof_pointer = ZkAmsMkheDecryptionTransportPointerV1::from_payload(
        ZkAmsMkheDecryptionTransportComponentKindV1::ProofEnvelope,
        &proof_envelope,
    )
    .unwrap();
    assert_eq!(polynomial_pointer.payload_bytes(), 39_845_892);
    assert_eq!(proof_pointer.payload_bytes(), 33_030_199);
    assert_ne!(polynomial_pointer.payload_blake3(), [0; 32]);
    assert_ne!(proof_pointer.payload_blake3(), [0; 32]);
}

#[test]
fn sound_share_ceiling_boundary_is_exact_to_one_byte() {
    let baseline = zk_ams_mkhe_decryption_resource_evidence_v1().unwrap();
    let mut one_short = release_profile_v1();
    one_short.max_share_bytes = usize::try_from(
        baseline
            .minimum_sound_share_ceiling_bytes
            .checked_sub(1)
            .unwrap(),
    )
    .unwrap();
    let short = derive_decryption_resource_evidence(&one_short).unwrap();
    assert!(!short.share_ceiling_met);
    assert_eq!(short.ceiling_shortfall_bytes, 1);

    let mut exact = release_profile_v1();
    exact.max_share_bytes = usize::try_from(baseline.minimum_sound_share_ceiling_bytes).unwrap();
    let exact = derive_decryption_resource_evidence(&exact).unwrap();
    assert!(exact.share_ceiling_met);
    assert_eq!(exact.ceiling_shortfall_bytes, 0);
    assert_eq!(
        production_decryption_share_record_bytes(
            &release_profile_v1(),
            usize::from(baseline.smudge_quotient_bits),
        )
        .unwrap()
        .1,
        usize::try_from(baseline.minimum_sound_share_ceiling_bytes).unwrap()
    );
}

#[test]
fn missing_excess_duplicate_and_reordered_sets_identify_first_slot() {
    let fixture = fixture(b"decryption-set-negative");
    let shares = make_shares(&fixture, b"decryption-set-negative-shares");
    let missing = aggregate_and_decrypt_full_roster(
        &fixture.profile,
        &fixture.parties,
        &fixture.relations,
        &fixture.ciphertext,
        &shares[..7],
        TEST_SMUDGE_BITS,
        TEST_FINAL_RESIDUAL_BITS,
    )
    .unwrap_err();
    assert_eq!(missing.reason, DecryptionAbortReasonV1::MissingShare);
    assert_eq!(missing.party_index, 7);

    let mut excess = shares.clone();
    excess.push(shares[0].clone());
    assert_eq!(
        aggregate_and_decrypt_full_roster(
            &fixture.profile,
            &fixture.parties,
            &fixture.relations,
            &fixture.ciphertext,
            &excess,
            TEST_SMUDGE_BITS,
            TEST_FINAL_RESIDUAL_BITS,
        )
        .unwrap_err()
        .reason,
        DecryptionAbortReasonV1::ExcessShare
    );

    for mutation in [
        {
            let mut values = shares.clone();
            values[1] = values[0].clone();
            values
        },
        {
            let mut values = shares.clone();
            values.swap(2, 3);
            values
        },
    ] {
        let abort = aggregate_and_decrypt_full_roster(
            &fixture.profile,
            &fixture.parties,
            &fixture.relations,
            &fixture.ciphertext,
            &mutation,
            TEST_SMUDGE_BITS,
            TEST_FINAL_RESIDUAL_BITS,
        )
        .unwrap_err();
        assert_eq!(
            abort.reason,
            DecryptionAbortReasonV1::ReorderedOrDuplicateShare
        );
        assert_ne!(abort.evidence_digest, [0; 32]);
    }
}

#[test]
fn every_binding_axis_and_replay_axis_fails_closed() {
    let fixture = fixture(b"decryption-binding-negative");
    let shares = make_shares(&fixture, b"decryption-binding-negative-shares");
    for axis in 0..12 {
        let mut mutation = shares[3].clone();
        match axis {
            0 => mutation.binding.profile_digest[0] ^= 1,
            1 => mutation.binding.roster_digest[0] ^= 1,
            2 => mutation.binding.epoch += 1,
            3 => mutation.binding.transcript_digest[0] ^= 1,
            4 => mutation.binding.ciphertext_digest[0] ^= 1,
            5 => mutation.binding.key_context_digest[0] ^= 1,
            6 => mutation.binding.statement_binding_digest[0] ^= 1,
            7 => mutation.binding.ciphertext_record_index += 1,
            8 => mutation.binding.sample_index += 1,
            9 => mutation.binding.party_index = 4,
            10 => mutation.binding.party = fixture.parties.parties[4],
            11 => mutation.binding.level = 0,
            _ => unreachable!(),
        }
        assert!(
            verify_authenticated_share(
                &fixture.profile,
                &fixture.parties,
                &fixture.relations[3],
                &fixture.ciphertext,
                &fixture.relations[3].binding,
                &mutation,
                TEST_SMUDGE_BITS,
            )
            .is_err(),
            "binding axis {axis} must reject"
        );
    }
    let other_fixture = self::fixture(b"decryption-binding-other-session");
    assert!(
        verify_authenticated_share(
            &other_fixture.profile,
            &other_fixture.parties,
            &other_fixture.relations[3],
            &other_fixture.ciphertext,
            &other_fixture.relations[3].binding,
            &shares[3],
            TEST_SMUDGE_BITS,
        )
        .is_err()
    );
}

#[test]
fn polynomial_public_key_proof_and_authentication_mutations_are_rejected() {
    let fixture = fixture(b"decryption-proof-negative");
    let shares = make_shares(&fixture, b"decryption-proof-negative-shares");

    let mut share_poly = shares[2].clone();
    share_poly.share.coefficients[0] =
        (share_poly.share.coefficients[0] + 1) % fixture.profile.moduli[0];
    assert_eq!(
        verify_authenticated_share(
            &fixture.profile,
            &fixture.parties,
            &fixture.relations[2],
            &fixture.ciphertext,
            &fixture.relations[2].binding,
            &share_poly,
            TEST_SMUDGE_BITS,
        ),
        Err(ZkAmsMkheErrorV1::InvalidAuthentication)
    );

    let mut bad_relation = fixture.relations[2].clone();
    bad_relation.party_b.coefficients[0] =
        (bad_relation.party_b.coefficients[0] + 1) % fixture.profile.moduli[0];
    assert!(
        verify_authenticated_share(
            &fixture.profile,
            &fixture.parties,
            &bad_relation,
            &fixture.ciphertext,
            &fixture.relations[2].binding,
            &shares[2],
            TEST_SMUDGE_BITS,
        )
        .is_err()
    );

    for mutation in 0..4 {
        let mut proof = shares[2].clone();
        match mutation {
            0 => proof.proof.challenge_seed[0] ^= 1,
            1 => proof.proof.secret_response[0] += 1,
            2 => proof.proof.public_key_error_response[0] += 1,
            3 => {
                proof.proof.smudge_response[0] = proof.proof.smudge_response[0]
                    .checked_add(&SignedWideV1::from_i64(1))
                    .unwrap();
            }
            _ => unreachable!(),
        }
        assert!(
            verify_decryption_relation(
                &fixture.profile,
                &fixture.parties,
                &fixture.relations[2],
                &fixture.ciphertext,
                &proof.share,
                TEST_SMUDGE_BITS,
                &proof.proof,
            )
            .is_err(),
            "proof mutation {mutation} must reject"
        );
    }
    assert!(
        verify_decryption_relation(
            &fixture.profile,
            &fixture.parties,
            &fixture.relations[2],
            &fixture.ciphertext,
            &shares[2].share,
            TEST_SMUDGE_BITS + 1,
            &shares[2].proof,
        )
        .is_err(),
        "a proof must not replay under another smudging bound"
    );

    let mut signature = shares[2].clone();
    signature.authentication.signature[64] ^= 1;
    assert_eq!(
        verify_authenticated_share(
            &fixture.profile,
            &fixture.parties,
            &fixture.relations[2],
            &fixture.ciphertext,
            &fixture.relations[2].binding,
            &signature,
            TEST_SMUDGE_BITS,
        ),
        Err(ZkAmsMkheErrorV1::InvalidAuthentication)
    );
}

#[test]
fn cross_party_relation_proof_authentication_and_ciphertext_splices_fail() {
    let fixture = fixture(b"decryption-splice-negative");
    let shares = make_shares(&fixture, b"decryption-splice-negative-shares");
    let index = 4;

    let mut proof_splice = shares[index].clone();
    proof_splice.proof = shares[index + 1].proof.clone();
    assert!(
        verify_decryption_relation(
            &fixture.profile,
            &fixture.parties,
            &fixture.relations[index],
            &fixture.ciphertext,
            &proof_splice.share,
            TEST_SMUDGE_BITS,
            &proof_splice.proof,
        )
        .is_err()
    );

    let mut authentication_splice = shares[index].clone();
    authentication_splice.authentication = shares[index + 1].authentication.clone();
    assert!(
        verify_authenticated_share(
            &fixture.profile,
            &fixture.parties,
            &fixture.relations[index],
            &fixture.ciphertext,
            &fixture.relations[index].binding,
            &authentication_splice,
            TEST_SMUDGE_BITS,
        )
        .is_err()
    );

    assert!(
        verify_authenticated_share(
            &fixture.profile,
            &fixture.parties,
            &fixture.relations[index + 1],
            &fixture.ciphertext,
            &fixture.relations[index].binding,
            &shares[index],
            TEST_SMUDGE_BITS,
        )
        .is_err()
    );

    for mutate_constant in [false, true] {
        let mut constant = fixture.ciphertext.constant().clone();
        let mut linear = fixture.ciphertext.linear().clone();
        let polynomial = if mutate_constant {
            &mut constant
        } else {
            &mut linear
        };
        polynomial.coefficients[0] = (polynomial.coefficients[0] + 1) % fixture.profile.moduli[0];
        let ciphertext = ZkAmsMkheCollectiveCiphertextV1::new(
            &fixture.profile,
            &fixture.parties,
            fixture.ciphertext.epoch(),
            fixture.ciphertext.transcript_digest(),
            fixture.ciphertext.sample_index(),
            fixture.ciphertext.level(),
            constant,
            linear,
        )
        .unwrap();
        assert!(
            verify_authenticated_share(
                &fixture.profile,
                &fixture.parties,
                &fixture.relations[index],
                &ciphertext,
                &fixture.relations[index].binding,
                &shares[index],
                TEST_SMUDGE_BITS,
            )
            .is_err()
        );
    }

    let mut wrong_profile = fixture.profile.clone();
    wrong_profile.profile_id[0] ^= 1;
    assert!(
        verify_authenticated_share(
            &wrong_profile,
            &fixture.parties,
            &fixture.relations[index],
            &fixture.ciphertext,
            &fixture.relations[index].binding,
            &shares[index],
            TEST_SMUDGE_BITS,
        )
        .is_err()
    );
}

#[test]
fn response_bounds_reject_one_step_over_every_family() {
    let fixture = fixture(b"decryption-response-bound-negative");
    let share = make_shares(&fixture, b"decryption-response-bound-shares").remove(0);
    let weight = wide_relation_challenge_weight(fixture.profile.ring_degree).unwrap();
    let (_, secret_limit) = small_response_parameters(1, weight, &fixture.profile).unwrap();
    let (_, error_limit) = small_response_parameters(
        i64::from(fixture.profile.error_eta),
        weight,
        &fixture.profile,
    )
    .unwrap();
    let (_, wide_limit, _) = wide_response_parameters(TEST_SMUDGE_BITS, weight).unwrap();

    let mut secret = share.proof.clone();
    secret.secret_response[0] = secret_limit + 1;
    let mut error = share.proof.clone();
    error.public_key_error_response[0] = error_limit + 1;
    let mut wide = share.proof.clone();
    wide.smudge_response[0] = SignedWideV1::new(
        false,
        wide_limit
            .checked_add(&WideMagnitudeV1 {
                limbs: {
                    let mut limbs = [0_u64; DECRYPTION_MAX_WIDE_LIMBS_V1];
                    limbs[0] = 1;
                    limbs
                },
            })
            .unwrap(),
    )
    .unwrap();
    for proof in [secret, error, wide] {
        assert_eq!(
            verify_decryption_relation(
                &fixture.profile,
                &fixture.parties,
                &fixture.relations[0],
                &fixture.ciphertext,
                &share.share,
                TEST_SMUDGE_BITS,
                &proof,
            ),
            Err(ZkAmsMkheErrorV1::InvalidShareProof)
        );
    }
}

#[test]
fn decoder_preflights_truncation_extension_counts_residues_and_negative_zero() {
    let fixture = fixture(b"decryption-wire-negative");
    let share = make_shares(&fixture, b"decryption-wire-negative-shares").remove(0);
    let encoded = share.encode(&fixture.profile, TEST_SMUDGE_BITS).unwrap();
    for malformed in [
        encoded[..encoded.len() - 1].to_vec(),
        {
            let mut value = encoded.clone();
            value.push(0);
            value
        },
        {
            let mut value = encoded.clone();
            // Residue count begins immediately before the proof length.
            let offset = TEST_DECRYPTION_SHARE_HEADER_BYTES_V1 - 4;
            value[offset..offset + 4].copy_from_slice(&u32::MAX.to_be_bytes());
            value
        },
    ] {
        assert!(
            AuthenticatedDecryptionShareV1::decode_exact(
                &malformed,
                &fixture.profile,
                &fixture.parties,
                &fixture.relations[0].binding,
                TEST_SMUDGE_BITS,
            )
            .is_err()
        );
    }

    let polynomial_offset = TEST_DECRYPTION_SHARE_HEADER_BYTES_V1 + 4;
    let mut noncanonical = encoded.clone();
    noncanonical[polynomial_offset..polynomial_offset + 8]
        .copy_from_slice(&fixture.profile.moduli[0].to_be_bytes());
    assert!(
        AuthenticatedDecryptionShareV1::decode_exact(
            &noncanonical,
            &fixture.profile,
            &fixture.parties,
            &fixture.relations[0].binding,
            TEST_SMUDGE_BITS,
        )
        .is_err()
    );

    let proof_offset = TEST_DECRYPTION_SHARE_HEADER_BYTES_V1
        + checked_rns_polynomial_bytes(&fixture.profile).unwrap();
    let wide_bytes = usize::from(share.proof.wide_response_bytes);
    let wide_offset =
        proof_offset + DECRYPTION_PROOF_HEADER_BYTES_V1 + fixture.profile.ring_degree * 16;
    let mut negative_zero = encoded.clone();
    negative_zero[wide_offset..wide_offset + wide_bytes].fill(0);
    negative_zero[wide_offset] = 0x80;
    assert!(
        AuthenticatedDecryptionShareV1::decode_exact(
            &negative_zero,
            &fixture.profile,
            &fixture.parties,
            &fixture.relations[0].binding,
            TEST_SMUDGE_BITS,
        )
        .is_err()
    );
}

#[test]
fn final_centered_correctness_bound_is_enforced_after_all_proofs() {
    let fixture = fixture(b"decryption-bound-negative");
    let shares = make_shares(&fixture, b"decryption-bound-negative-shares");
    let abort = aggregate_and_decrypt_full_roster(
        &fixture.profile,
        &fixture.parties,
        &fixture.relations,
        &fixture.ciphertext,
        &shares,
        TEST_SMUDGE_BITS,
        1,
    )
    .unwrap_err();
    assert_eq!(
        abort.reason,
        DecryptionAbortReasonV1::CorrectnessBoundExceeded
    );
}
