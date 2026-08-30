use super::*;
use crate::vega::{MaskedRelaxedRandomErrorV1, sponge::shake256};
const TEST_MODULI: [u64; 2] = [2_013_265_921, 1_811_939_329];
const TEST_ROOTS: [u64; 2] = [1_400_279_418, 677_356_115];
pub(super) fn test_profile() -> BgvProfile {
    BgvProfile {
        profile_id: [0x61; 32],
        ring_degree: 8,
        moduli: &TEST_MODULI,
        negacyclic_roots: &TEST_ROOTS,
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
pub(super) struct KatRandom {
    state: [u8; 32],
    counter: u64,
}
impl KatRandom {
    pub(super) fn new(label: &[u8]) -> Self {
        Self {
            state: keccak256(label),
            counter: 0,
        }
    }
}
impl MaskedRelaxedRandomSourceV1 for KatRandom {
    fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), MaskedRelaxedRandomErrorV1> {
        let mut written = 0;
        while written < destination.len() {
            let mut frame = Vec::with_capacity(40);
            frame.extend_from_slice(&self.state);
            frame.extend_from_slice(&self.counter.to_be_bytes());
            let block = shake256(&frame, 64);
            let take = (destination.len() - written).min(block.len());
            destination[written..written + take].copy_from_slice(&block[..take]);
            self.state = keccak256(&block);
            self.counter = self.counter.wrapping_add(1);
            written += take;
        }
        Ok(())
    }
}
struct ConstantRandom(u8);
impl MaskedRelaxedRandomSourceV1 for ConstantRandom {
    fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), MaskedRelaxedRandomErrorV1> {
        destination.fill(self.0);
        Ok(())
    }
}
struct FailingRandom;
impl MaskedRelaxedRandomSourceV1 for FailingRandom {
    fn fill_bytes(&mut self, _destination: &mut [u8]) -> Result<(), MaskedRelaxedRandomErrorV1> {
        Err(MaskedRelaxedRandomErrorV1::Unavailable)
    }
}
fn persistent_blinding_uniform_block(value: u64) -> [u8; 64] {
    let mut block = [0; 64];
    block[..8].copy_from_slice(&value.to_le_bytes());
    block
}
struct ScriptedPersistentBlindingRandom {
    blocks: Vec<[u8; 64]>,
    next: usize,
    request_lengths: Vec<usize>,
}
impl ScriptedPersistentBlindingRandom {
    fn from_scalars(values: impl IntoIterator<Item = u64>) -> Self {
        Self {
            blocks: values
                .into_iter()
                .map(persistent_blinding_uniform_block)
                .collect(),
            next: 0,
            request_lengths: Vec::new(),
        }
    }
}
impl MaskedRelaxedRandomSourceV1 for ScriptedPersistentBlindingRandom {
    fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), MaskedRelaxedRandomErrorV1> {
        self.request_lengths.push(destination.len());
        let Some(block) = self.blocks.get(self.next) else {
            return Err(MaskedRelaxedRandomErrorV1::Unavailable);
        };
        self.next += 1;
        if destination.len() != block.len() {
            return Err(MaskedRelaxedRandomErrorV1::Unavailable);
        }
        destination.copy_from_slice(block);
        Ok(())
    }
}
struct PartialFailurePersistentBlindingRandom {
    successful_requests: usize,
    calls: usize,
    partial_bytes: usize,
}
impl MaskedRelaxedRandomSourceV1 for PartialFailurePersistentBlindingRandom {
    fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), MaskedRelaxedRandomErrorV1> {
        assert_eq!(destination.len(), PERSISTENT_BLINDING_ENTROPY_BYTES_V1);
        let call = self.calls;
        self.calls += 1;
        if call < self.successful_requests {
            destination.copy_from_slice(&persistent_blinding_uniform_block(
                u64::try_from(call + 1).expect("test request index fits u64"),
            ));
            return Ok(());
        }
        destination[..self.partial_bytes].fill(0xa5);
        Err(MaskedRelaxedRandomErrorV1::Unavailable)
    }
}
struct PartialPanicPersistentBlindingRandom {
    successful_requests: usize,
    calls: usize,
    partial_bytes: usize,
}
impl MaskedRelaxedRandomSourceV1 for PartialPanicPersistentBlindingRandom {
    fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), MaskedRelaxedRandomErrorV1> {
        assert_eq!(destination.len(), PERSISTENT_BLINDING_ENTROPY_BYTES_V1);
        let call = self.calls;
        self.calls += 1;
        if call < self.successful_requests {
            destination.copy_from_slice(&persistent_blinding_uniform_block(
                u64::try_from(call + 1).expect("test request index fits u64"),
            ));
            return Ok(());
        }
        destination[..self.partial_bytes].fill(0x5a);
        panic!("injected persistent-blinding entropy panic");
    }
}
fn reset_persistent_blinding_drop_audits() {
    PERSISTENT_BLINDING_ENTROPY_ZEROIZED_DROPS_V1.with(|drops| drops.set(0));
    PERSISTENT_BLINDING_OWNER_ZEROIZED_DROPS_V1.with(|drops| drops.set(0));
}
fn persistent_blinding_drop_audits() -> (usize, usize) {
    let entropy = PERSISTENT_BLINDING_ENTROPY_ZEROIZED_DROPS_V1.with(std::cell::Cell::get);
    let owner = PERSISTENT_BLINDING_OWNER_ZEROIZED_DROPS_V1.with(std::cell::Cell::get);
    (entropy, owner)
}
fn test_persistent_secret_commitment_blindings() -> ZeroizingCpkMembershipBlindingsV1 {
    ZeroizingCpkMembershipBlindingsV1(core::array::from_fn(|index| {
        Scalar::from_u64(u64::try_from(index + 17).expect("test blinding index fits u64"))
    }))
}
struct RepeatedHealthyBlockRandom;
impl MaskedRelaxedRandomSourceV1 for RepeatedHealthyBlockRandom {
    fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), MaskedRelaxedRandomErrorV1> {
        for (index, byte) in destination.iter_mut().enumerate() {
            *byte = u8::try_from(index).unwrap_or(0).wrapping_mul(29) ^ 0xa7;
        }
        Ok(())
    }
}
struct DistinctOddPeriodProbeRandom {
    calls: usize,
}
impl DistinctOddPeriodProbeRandom {
    const fn new() -> Self {
        Self { calls: 0 }
    }
}
impl MaskedRelaxedRandomSourceV1 for DistinctOddPeriodProbeRandom {
    fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), MaskedRelaxedRandomErrorV1> {
        let pattern = if self.calls == 0 {
            [0x19, 0x4d, 0xc2]
        } else {
            [0x27, 0xa6, 0x58]
        };
        for (index, byte) in destination.iter_mut().enumerate() {
            *byte = pattern[index % pattern.len()];
        }
        self.calls = self.calls.saturating_add(1);
        Ok(())
    }
}
struct ProbeThenConstantRandom {
    calls: usize,
}
impl ProbeThenConstantRandom {
    const fn new() -> Self {
        Self { calls: 0 }
    }
}
impl MaskedRelaxedRandomSourceV1 for ProbeThenConstantRandom {
    fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), MaskedRelaxedRandomErrorV1> {
        match self.calls {
            0 | 1 => {
                let domain = if self.calls == 0 { 0x39 } else { 0xd2 };
                for (index, byte) in destination.iter_mut().enumerate() {
                    *byte = u8::try_from(index)
                        .unwrap_or(0)
                        .wrapping_mul(37)
                        .wrapping_add(domain)
                        ^ u8::try_from(index * index).unwrap_or(0);
                }
            }
            _ => destination.fill(0x55),
        }
        self.calls += 1;
        Ok(())
    }
}
fn test_parties() -> super::super::PartySet {
    super::super::PartySet::new(
        (1_u8..=ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 as u8)
            .map(|tag| {
                let mut bytes = [0_u8; 32];
                bytes[31] = tag;
                ZkAmsMkhePartyIdV1::new(bytes).unwrap()
            })
            .collect(),
    )
    .unwrap()
}
pub(super) fn test_key(label: u8) -> (ZkAmsMkheCollectivePublicKeyV1, SecretPolynomial) {
    let profile = test_profile();
    profile.validate().unwrap();
    let parties = test_parties();
    let aggregate_secret = SecretPolynomial {
        coefficients: vec![8, 0, 0, 0, 0, 0, 0, 0],
    };
    let public_a = RnsPolynomial::from_unsigned(&profile, &[1, 2, 3, 4, 5, 6, 7, 8]).unwrap();
    let collective_public_b = public_a
        .mul(&aggregate_secret.as_rns(&profile).unwrap(), &profile)
        .unwrap()
        .negate(&profile)
        .unwrap();
    let epoch = 19;
    let mut key = ZkAmsMkheCollectivePublicKeyV1 {
        version: MKHE_VERSION_V1,
        profile_digest: profile.digest().unwrap(),
        security_certificate_digest: [0x22; 32],
        roster_digest: governed_roster_digest(profile.digest().unwrap(), epoch, &parties.parties),
        key_material_digest: [label; 32],
        epoch,
        transcript_digest: [label.wrapping_add(1); 32],
        parties,
        public_a,
        collective_public_b,
        share_digests: core::array::from_fn(|index| [index as u8 + 1; 32]),
        digest: [0; 32],
    };
    key.digest = collective_public_key_digest(&key, &profile).unwrap();
    key.validate(&profile).unwrap();
    (key, aggregate_secret)
}
pub(super) fn test_canonical_plaintext(values: &[u64; 8]) -> Vec<[u8; 32]> {
    values
        .iter()
        .map(|value| {
            let mut coefficient = [0; 32];
            coefficient[24..].copy_from_slice(&value.to_be_bytes());
            coefficient
        })
        .collect()
}
pub(super) fn test_input_topology(
    profile: &BgvProfile,
    label: &[u8],
) -> CollectiveEncryptionInputTopologyV1 {
    CollectiveEncryptionInputTopologyV1 {
        layout_digest: keccak256(&[b"layout".as_slice(), label].concat()),
        plaintext_chunk_index: 0,
        plaintext_used_slots: u32::try_from(profile.ring_degree).unwrap(),
    }
}
pub(super) fn encrypt_test_with_opening(
    profile: &BgvProfile,
    key: &ZkAmsMkheCollectivePublicKeyV1,
    values: &[u64; 8],
    sample_index: u64,
    label: &[u8],
) -> (
    ZkAmsMkheCollectiveCiphertextV1,
    ZkAmsMkheCollectiveEncryptionOpeningV1,
    RnsPolynomial,
    Vec<[u8; 32]>,
    CollectiveEncryptionInputTopologyV1,
    [u8; 32],
) {
    let message = RnsPolynomial::from_test_plaintext(profile, values).unwrap();
    let canonical_plaintext = test_canonical_plaintext(values);
    let input_topology = test_input_topology(profile, label);
    let (ciphertext, opening) = encrypt_collective_native_with_opening(
        profile,
        key,
        ZeroizingRns(message.clone()),
        ZeroizingCanonicalPlaintext(canonical_plaintext.clone()),
        input_topology,
        sample_index,
        &mut KatRandom::new(label),
    )
    .unwrap();
    let transcript_digest = ciphertext.transcript_digest;
    (
        ciphertext,
        opening,
        message,
        canonical_plaintext,
        input_topology,
        transcript_digest,
    )
}
fn try_encrypt_test_with_random<R: MaskedRelaxedRandomSourceV1>(
    profile: &BgvProfile,
    key: &ZkAmsMkheCollectivePublicKeyV1,
    values: &[u64; 8],
    sample_index: u64,
    label: &[u8],
    random: &mut R,
) -> Result<
    (
        ZkAmsMkheCollectiveCiphertextV1,
        ZkAmsMkheCollectiveEncryptionOpeningV1,
    ),
    ZkAmsMkheErrorV1,
> {
    let message = RnsPolynomial::from_test_plaintext(profile, values).unwrap();
    let canonical_plaintext = test_canonical_plaintext(values);
    let input_topology = test_input_topology(profile, label);
    encrypt_collective_native_with_opening(
        profile,
        key,
        ZeroizingRns(message),
        ZeroizingCanonicalPlaintext(canonical_plaintext),
        input_topology,
        sample_index,
        random,
    )
}
fn encrypt_test(
    profile: &BgvProfile,
    key: &ZkAmsMkheCollectivePublicKeyV1,
    values: &[u64; 8],
    sample_index: u64,
    label: &[u8],
) -> ZkAmsMkheCollectiveCiphertextV1 {
    let (ciphertext, opening, ..) =
        encrypt_test_with_opening(profile, key, values, sample_index, label);
    drop(opening);
    ciphertext
}
fn decrypt_compact(
    profile: &BgvProfile,
    ciphertext: &ZkAmsMkheCollectiveCiphertextV1,
    secret: &SecretPolynomial,
) -> Vec<u64> {
    let value = ciphertext
        .constant
        .add(
            &ciphertext
                .linear
                .mul(&secret.as_rns(profile).unwrap(), profile)
                .unwrap(),
            profile,
        )
        .unwrap();
    super::super::reduce_test_polynomial(profile, &value).unwrap()
}
fn decrypt_level_one(
    profile: &BgvProfile,
    ciphertext: &ZkAmsMkheCollectiveLevelOneV1,
    secret: &SecretPolynomial,
) -> Vec<u64> {
    let secret = secret.as_rns(profile).unwrap();
    let secret_square = secret.mul(&secret, profile).unwrap();
    let value = ciphertext
        .constant
        .add(&ciphertext.linear.mul(&secret, profile).unwrap(), profile)
        .unwrap()
        .add(
            &ciphertext.quadratic.mul(&secret_square, profile).unwrap(),
            profile,
        )
        .unwrap();
    super::super::reduce_test_polynomial(profile, &value).unwrap()
}
fn negacyclic_plaintext_product(left: &[u64; 8], right: &[u64; 8]) -> Vec<u64> {
    let mut output = [0_i128; 8];
    for (left_index, left_value) in left.iter().copied().enumerate() {
        for (right_index, right_value) in right.iter().copied().enumerate() {
            let product = i128::from(left_value) * i128::from(right_value);
            let index = left_index + right_index;
            if index < 8 {
                output[index] += product;
            } else {
                output[index - 8] -= product;
            }
        }
    }
    output
        .into_iter()
        .map(|value| value.rem_euclid(17) as u64)
        .collect()
}
#[test]
fn tiny_collective_algebra_matches_plaintext_oracle() {
    let profile = test_profile();
    let (key, secret) = test_key(0x31);
    let left_values = [1, 2, 3, 4, 5, 6, 7, 8];
    let right_values = [8, 0, 2, 0, 4, 0, 6, 0];
    let left = encrypt_test(&profile, &key, &left_values, 11, b"collective-left");
    let right = encrypt_test(&profile, &key, &right_values, 17, b"collective-right");
    assert_eq!(decrypt_compact(&profile, &left, &secret), left_values);
    assert_eq!(decrypt_compact(&profile, &right, &secret), right_values);
    let sum = compact_binary_with_profile(
        &profile,
        &left,
        &key,
        &right,
        COLLECTIVE_ADD_DOMAIN_V1,
        RnsPolynomial::add,
    )
    .unwrap();
    assert_eq!(
        decrypt_compact(&profile, &sum, &secret),
        left_values
            .iter()
            .zip(right_values)
            .map(|(left, right)| (*left + right) % 17)
            .collect::<Vec<_>>()
    );
    assert_eq!(sum.sample_index(), 11);
    assert_ne!(sum.transcript_digest(), left.transcript_digest());
    let difference = compact_binary_with_profile(
        &profile,
        &left,
        &key,
        &right,
        COLLECTIVE_SUB_DOMAIN_V1,
        RnsPolynomial::sub,
    )
    .unwrap();
    assert_eq!(
        decrypt_compact(&profile, &difference, &secret),
        left_values
            .iter()
            .zip(right_values)
            .map(|(left, right)| (17 + *left - right) % 17)
            .collect::<Vec<_>>()
    );
    assert_ne!(sum.digest(), difference.digest());
    let expected_product = negacyclic_plaintext_product(&left_values, &right_values);
    let product = multiply_with_profile(&profile, &left, &key, &right).unwrap();
    assert_eq!(
        decrypt_level_one(&profile, &product, &secret),
        expected_product
    );
    assert_eq!(product.evaluation_key_digest(), key.digest());
    let plaintext_multiplier = RnsPolynomial::from_test_plaintext(&profile, &right_values).unwrap();
    let scaled = compact_plaintext_mul_with_profile(
        &profile,
        &left,
        &key,
        &plaintext_multiplier,
        &[b"canonical-test-plaintext"],
    )
    .unwrap();
    assert_eq!(
        decrypt_compact(&profile, &scaled, &secret),
        expected_product
    );
    let doubled_product = level_one_binary_with_profile(
        &profile,
        &product,
        &key,
        &product,
        COLLECTIVE_ADD_DOMAIN_V1,
        RnsPolynomial::add,
    )
    .unwrap();
    assert_eq!(
        decrypt_level_one(&profile, &doubled_product, &secret),
        expected_product
            .iter()
            .map(|value| (2 * value) % 17)
            .collect::<Vec<_>>()
    );
    let zero_product = level_one_binary_with_profile(
        &profile,
        &product,
        &key,
        &product,
        COLLECTIVE_SUB_DOMAIN_V1,
        RnsPolynomial::sub,
    )
    .unwrap();
    assert_eq!(
        decrypt_level_one(&profile, &zero_product, &secret),
        vec![0; 8]
    );
    let scaled_product = level_one_plaintext_mul_with_profile(
        &profile,
        &product,
        &key,
        &plaintext_multiplier,
        &[b"canonical-level-one-test-plaintext"],
    )
    .unwrap();
    let expected_product_array: [u64; 8] = expected_product.clone().try_into().unwrap();
    assert_eq!(
        decrypt_level_one(&profile, &scaled_product, &secret),
        negacyclic_plaintext_product(&expected_product_array, &right_values)
    );
    let transformed_product =
        level_one_automorphism_with_profile(&profile, &product, &key, 3, 3_u64.to_be_bytes())
            .unwrap();
    let transformed_secret = secret.automorphism(3, &profile).unwrap();
    let expected_transformed =
        RnsPolynomial::from_test_plaintext(&profile, &expected_product_array)
            .unwrap()
            .automorphism(3, &profile)
            .unwrap();
    assert_eq!(
        decrypt_level_one(&profile, &transformed_product, &transformed_secret),
        super::super::reduce_test_polynomial(&profile, &expected_transformed).unwrap()
    );
}
#[test]
fn raw_automorphism_moves_to_exact_automorphed_key_domain() {
    let profile = test_profile();
    let (key, secret) = test_key(0x41);
    let values = [1, 2, 4, 8, 3, 6, 12, 7];
    let ciphertext = encrypt_test(&profile, &key, &values, 5, b"collective-auto");
    let exponent = 3;
    let transformed = compact_automorphism_with_profile(
        &profile,
        &ciphertext,
        &key,
        exponent,
        (exponent as u64).to_be_bytes(),
    )
    .unwrap();
    assert_ne!(transformed.evaluation_key_digest(), Some(key.digest()));
    assert_eq!(
        validate_compact_for_key(&transformed, &key, &profile),
        Err(ZkAmsMkheErrorV1::InvalidCiphertext)
    );
    let transformed_secret = secret.automorphism(exponent, &profile).unwrap();
    let expected = RnsPolynomial::from_test_plaintext(&profile, &values)
        .unwrap()
        .automorphism(exponent, &profile)
        .unwrap();
    assert_eq!(
        decrypt_compact(&profile, &transformed, &transformed_secret),
        super::super::reduce_test_polynomial(&profile, &expected).unwrap()
    );
    for invalid in [0, 2, 16, usize::MAX] {
        assert!(
            compact_automorphism_with_profile(
                &profile,
                &ciphertext,
                &key,
                invalid,
                u64::try_from(invalid).unwrap_or(u64::MAX).to_be_bytes(),
            )
            .is_err()
        );
    }
}
#[test]
fn cross_key_unbound_and_tampered_ciphertexts_fail_closed() {
    let profile = test_profile();
    let (key, _) = test_key(0x51);
    let (other_key, _) = test_key(0x52);
    let values = [1, 0, 0, 0, 0, 0, 0, 0];
    let ciphertext = encrypt_test(&profile, &key, &values, 3, b"collective-binding");
    assert_eq!(
        compact_binary_with_profile(
            &profile,
            &ciphertext,
            &other_key,
            &ciphertext,
            COLLECTIVE_ADD_DOMAIN_V1,
            RnsPolynomial::add,
        ),
        Err(ZkAmsMkheErrorV1::InvalidCiphertext)
    );
    let unbound = ZkAmsMkheCollectiveCiphertextV1::new(
        &profile,
        &key.parties,
        key.epoch,
        [0x71; 32],
        3,
        0,
        ciphertext.constant.clone(),
        ciphertext.linear.clone(),
    )
    .unwrap();
    assert_eq!(
        compact_binary_with_profile(
            &profile,
            &unbound,
            &key,
            &ciphertext,
            COLLECTIVE_ADD_DOMAIN_V1,
            RnsPolynomial::add,
        ),
        Err(ZkAmsMkheErrorV1::InvalidCiphertext)
    );
    for axis in 0..4 {
        let mut tampered = ciphertext.clone();
        match axis {
            0 => tampered.profile_digest[0] ^= 1,
            1 => tampered.roster_digest[0] ^= 1,
            2 => tampered.epoch ^= 1,
            _ => tampered.transcript_digest[0] ^= 1,
        }
        assert!(tampered.validate(&profile, &key.parties).is_err());
    }
    let mut tampered_component = ciphertext.clone();
    tampered_component.constant.coefficients[0] = TEST_MODULI[0];
    assert_eq!(
        tampered_component.validate(&profile, &key.parties),
        Err(ZkAmsMkheErrorV1::InvalidPolynomial)
    );
}
#[test]
fn level_one_component_and_digest_tampering_is_rejected() {
    let profile = test_profile();
    let (key, _) = test_key(0x61);
    let values = [1, 2, 3, 4, 5, 6, 7, 8];
    let left = encrypt_test(&profile, &key, &values, 1, b"level-one-left");
    let right = encrypt_test(&profile, &key, &values, 2, b"level-one-right");
    let mut product = multiply_with_profile(&profile, &left, &key, &right).unwrap();
    product.quadratic.coefficients[0] ^= 1;
    assert_eq!(
        product.validate(&profile),
        Err(ZkAmsMkheErrorV1::InvalidCiphertext)
    );
    let mut product = multiply_with_profile(&profile, &left, &key, &right).unwrap();
    product.evaluation_key_digest[0] ^= 1;
    assert_eq!(
        product.validate(&profile),
        Err(ZkAmsMkheErrorV1::InvalidCiphertext)
    );
}
#[test]
fn deterministic_zero_ternary_rng_exhausts_without_emitting_secret_or_ciphertext() {
    let profile = test_profile();
    let mut zero_ternary = ConstantRandom(0x55);
    assert!(matches!(
        sample_nonzero_ternary(&profile, &mut zero_ternary),
        Err(ZkAmsMkheErrorV1::RandomUnavailable)
    ));
    let (key, _) = test_key(0x71);
    // The two initial probes are distinct and non-periodic; all subsequent
    // ternary bytes encode zero, so bounded rejection must still stop.
    let mut zero_ternary = ProbeThenConstantRandom::new();
    assert!(matches!(
        try_encrypt_test_with_random(&profile, &key, &[0; 8], 0, b"all-zero-r", &mut zero_ternary,),
        Err(ZkAmsMkheErrorV1::RandomUnavailable)
    ));
}
#[test]
fn collective_opening_adapter_recomputes_both_rlwe_equations_independently() {
    let profile = test_profile();
    let (key, _) = test_key(0x72);
    let values = [1, 16, 3, 14, 5, 12, 7, 10];
    let (ciphertext, opening, message, canonical, input_topology, _) =
        encrypt_test_with_opening(&profile, &key, &values, 29, b"opening-equations");
    opening
        .with_validated_native_proof_witness_v1(
            &profile,
            &key,
            &message,
            &canonical,
            input_topology,
            &ciphertext,
            |actual_canonical, actual_message, ephemeral, error_zero, error_one| {
                assert_eq!(actual_canonical, canonical.as_slice());
                assert_eq!(actual_message, &message);
                assert!(
                    ephemeral
                        .coefficients
                        .iter()
                        .any(|coefficient| *coefficient != 0)
                );
                assert!(bounded_error_polynomial(&profile, error_zero));
                assert!(bounded_error_polynomial(&profile, error_one));
                // Recompute from the raw validated witnesses rather than
                // relying on the opening's validation result.
                let ephemeral_rns = ZeroizingRns(ephemeral.as_rns(&profile)?);
                let error_zero_rns = ZeroizingRns(error_zero.as_rns(&profile)?);
                let error_one_rns = ZeroizingRns(error_one.as_rns(&profile)?);
                let scaled_error_zero =
                    ZeroizingRns(error_zero_rns.0.scale_plaintext_modulus(&profile)?);
                let scaled_error_one =
                    ZeroizingRns(error_one_rns.0.scale_plaintext_modulus(&profile)?);
                let public_b_product =
                    ZeroizingRns(key.collective_public_b.mul(&ephemeral_rns.0, &profile)?);
                let constant_with_error =
                    ZeroizingRns(public_b_product.0.add(&scaled_error_zero.0, &profile)?);
                let independently_recomputed_constant =
                    ZeroizingRns(constant_with_error.0.add(actual_message, &profile)?);
                let public_a_product = ZeroizingRns(key.public_a.mul(&ephemeral_rns.0, &profile)?);
                let independently_recomputed_linear =
                    ZeroizingRns(public_a_product.0.add(&scaled_error_one.0, &profile)?);
                assert_eq!(independently_recomputed_constant.0, *ciphertext.constant());
                assert_eq!(independently_recomputed_linear.0, *ciphertext.linear());
                Ok(())
            },
        )
        .unwrap();
}
#[test]
fn collective_opening_rejects_key_ciphertext_message_and_context_splices() {
    let profile = test_profile();
    let (key, _) = test_key(0x73);
    let (other_key, _) = test_key(0x74);
    let values = [2, 4, 6, 8, 10, 12, 14, 16];
    let (ciphertext, opening, message, canonical, topology, transcript_digest) =
        encrypt_test_with_opening(&profile, &key, &values, 31, b"opening-splices");
    assert!(
        opening
            .validate_against(
                &profile,
                &other_key,
                &message,
                &canonical,
                topology,
                &ciphertext,
            )
            .is_err()
    );
    let other_ciphertext = encrypt_test(
        &profile,
        &key,
        &[1, 3, 5, 7, 9, 11, 13, 15],
        32,
        b"opening-other-ciphertext",
    );
    assert!(
        opening
            .validate_against(
                &profile,
                &key,
                &message,
                &canonical,
                topology,
                &other_ciphertext,
            )
            .is_err()
    );
    let wrong_message =
        RnsPolynomial::from_test_plaintext(&profile, &[3, 4, 6, 8, 10, 12, 14, 16]).unwrap();
    assert!(
        opening
            .validate_against(
                &profile,
                &key,
                &wrong_message,
                &canonical,
                topology,
                &ciphertext,
            )
            .is_err()
    );
    let mut wrong_canonical = canonical.clone();
    wrong_canonical[0][31] ^= 1;
    assert!(
        opening
            .validate_against(
                &profile,
                &key,
                &message,
                &wrong_canonical,
                topology,
                &ciphertext,
            )
            .is_err()
    );
    for axis in 0..3 {
        let mut wrong_topology = topology;
        match axis {
            0 => wrong_topology.layout_digest[0] ^= 1,
            1 => wrong_topology.plaintext_chunk_index ^= 1,
            _ => wrong_topology.plaintext_used_slots -= 1,
        }
        assert!(
            opening
                .validate_against(
                    &profile,
                    &key,
                    &message,
                    &canonical,
                    wrong_topology,
                    &ciphertext,
                )
                .is_err(),
            "context splice axis {axis} was accepted"
        );
    }
    let mut wrong_transcript_ciphertext = ciphertext.clone();
    wrong_transcript_ciphertext.transcript_digest[0] ^= 1;
    wrong_transcript_ciphertext.digest = wrong_transcript_ciphertext
        .compute_digest(&profile)
        .unwrap();
    assert!(
        opening
            .validate_against(
                &profile,
                &key,
                &message,
                &canonical,
                topology,
                &wrong_transcript_ciphertext,
            )
            .is_err()
    );
    assert_ne!(
        wrong_transcript_ciphertext.transcript_digest,
        transcript_digest
    );
    let mut wrong_sample_ciphertext = ciphertext.clone();
    wrong_sample_ciphertext.sample_index ^= 1;
    wrong_sample_ciphertext.digest = wrong_sample_ciphertext.compute_digest(&profile).unwrap();
    assert!(
        opening
            .validate_against(
                &profile,
                &key,
                &message,
                &canonical,
                topology,
                &wrong_sample_ciphertext,
            )
            .is_err()
    );
    let mut corrupted_ciphertext = ciphertext.clone();
    corrupted_ciphertext.digest[0] ^= 1;
    assert!(
        opening
            .validate_against(
                &profile,
                &key,
                &message,
                &canonical,
                topology,
                &corrupted_ciphertext,
            )
            .is_err()
    );
    let (ciphertext, mut opening, message, canonical, topology, _) =
        encrypt_test_with_opening(&profile, &key, &values, 31, b"opening-nonce-splice");
    opening.input_identity.encryption_nonce.as_mut_bytes()[0] ^= 1;
    assert!(
        opening
            .validate_against(&profile, &key, &message, &canonical, topology, &ciphertext,)
            .is_err(),
        "an opening-owned encryption nonce splice was accepted"
    );
}
#[test]
fn collective_opening_rejects_out_of_range_or_tampered_secret_witnesses() {
    let profile = test_profile();
    let (key, _) = test_key(0x75);
    let values = [1, 2, 3, 4, 5, 6, 7, 8];
    for axis in 0..4 {
        let (ciphertext, mut opening, message, canonical, topology, _) = encrypt_test_with_opening(
            &profile,
            &key,
            &values,
            40 + axis,
            &[b"opening-witness-tamper".as_slice(), &[axis as u8]].concat(),
        );
        match axis {
            0 => opening.ephemeral.coefficients.fill(0),
            1 => opening.ephemeral.coefficients[0] = 2,
            2 => {
                opening.error_zero.coefficients[0] = i64::from(profile.error_eta) + 1;
            }
            _ => opening.error_one.coefficients[0] ^= 1,
        }
        assert!(
            opening
                .validate_against(&profile, &key, &message, &canonical, topology, &ciphertext,)
                .is_err(),
            "secret witness tamper axis {axis} was accepted"
        );
    }
}
#[test]
fn changed_plaintext_is_rejected_by_the_constant_rlwe_equation() {
    let profile = test_profile();
    let (key, _) = test_key(0x78);
    let original = [2, 4, 6, 8, 10, 12, 14, 16];
    let changed = [3, 4, 6, 8, 10, 12, 14, 16];
    let (ciphertext, mut opening, _, _, topology, _) =
        encrypt_test_with_opening(&profile, &key, &original, 48, b"changed-plaintext-equation");
    let changed_message = RnsPolynomial::from_test_plaintext(&profile, &changed).unwrap();
    let changed_canonical = test_canonical_plaintext(&changed);
    // Make the retained canonical views agree with the hostile caller so
    // rejection cannot rely on the removed deterministic plaintext
    // lineage. The unchanged RLWE constant must still fail independently.
    opening.plaintext_lift = ZeroizingRns(changed_message.clone());
    opening.canonical_plaintext = ZeroizingCanonicalPlaintext(changed_canonical.clone());
    let ephemeral_rns = ZeroizingRns(opening.ephemeral.as_rns(&profile).unwrap());
    let scaled_error_zero = scaled_public_error(&profile, &opening.error_zero).unwrap();
    let product = ZeroizingRns(
        key.collective_public_b
            .mul(&ephemeral_rns.0, &profile)
            .unwrap(),
    );
    let with_error = ZeroizingRns(product.0.add(&scaled_error_zero.0, &profile).unwrap());
    let hostile_constant = ZeroizingRns(with_error.0.add(&changed_message, &profile).unwrap());
    assert_ne!(hostile_constant.0, *ciphertext.constant());
    assert!(
        opening
            .validate_against(
                &profile,
                &key,
                &changed_message,
                &changed_canonical,
                topology,
                &ciphertext,
            )
            .is_err(),
        "changed plaintext passed the constant RLWE equation"
    );
}
#[test]
fn independent_entropy_gives_same_input_distinct_public_lineage() {
    let profile = test_profile();
    let (key, _) = test_key(0x79);
    let values = [1, 1, 2, 3, 5, 8, 13, 16];
    let mut first_random = KatRandom::new(b"lineage-independent-entropy-one");
    let mut second_random = KatRandom::new(b"lineage-independent-entropy-two");
    let (first, first_opening) = try_encrypt_test_with_random(
        &profile,
        &key,
        &values,
        49,
        b"identical-public-topology",
        &mut first_random,
    )
    .unwrap();
    let (second, second_opening) = try_encrypt_test_with_random(
        &profile,
        &key,
        &values,
        49,
        b"identical-public-topology",
        &mut second_random,
    )
    .unwrap();
    assert_eq!(
        first_opening.input_identity.topology,
        second_opening.input_identity.topology
    );
    assert_ne!(
        first_opening.input_identity.encryption_nonce.as_bytes(),
        second_opening.input_identity.encryption_nonce.as_bytes()
    );
    assert_ne!(first.transcript_digest, second.transcript_digest);
    drop(first_opening);
    drop(second_opening);
}
#[test]
fn fresh_encryption_lineage_source_excludes_plaintext_identity() {
    let source = include_str!("../collective.rs");
    let module_source = source
        .split("\n#[cfg(test)]\nmod tests {")
        .next()
        .expect("collective module source prefix");
    assert!(!module_source.contains("plaintext_digest"));
    let identity = module_source
        .split("struct CollectiveEncryptionInputIdentityV1")
        .nth(1)
        .expect("private encryption identity")
        .split("pub(super) struct ZkAmsMkheCollectiveEncryptionOpeningV1")
        .next()
        .expect("identity source slice");
    assert!(identity.contains("encryption_nonce: ZeroizingEncryptionNonce"));
    assert!(module_source.contains("struct ZeroizingEncryptionNonce(Box<[u8; 32]>)"));
    assert!(module_source.contains("Self(Box::new([0; 32]))"));
    assert!(!identity.contains("plaintext"));
    assert!(!module_source.contains("fn encryption_nonce("));
    assert!(!module_source.contains("pub encryption_nonce"));
    assert!(module_source.contains(".field(\"encryption_nonce\", &\"[REDACTED]\")"));
    let public_ciphertext = module_source
        .split("pub struct ZkAmsMkheCollectiveCiphertextV1")
        .nth(1)
        .expect("public ciphertext")
        .split("impl ZkAmsMkheCollectiveCiphertextV1")
        .next()
        .expect("public ciphertext source slice");
    assert!(public_ciphertext.contains("transcript_digest: [u8; 32]"));
    assert!(!public_ciphertext.contains("encryption_nonce"));
    let fresh_encryption = module_source
        .split("fn encrypt_zk_ams_mkhe_collective_packed_with_opening_v1")
        .nth(1)
        .expect("fresh encryption implementation")
        .split("impl ZkAmsMkheCollectiveCiphertextV1")
        .next()
        .expect("fresh encryption source slice");
    assert!(!fresh_encryption.contains("plaintext.digest"));
    assert!(fresh_encryption.contains("CollectiveEncryptionInputTopologyV1::from_packed"));
    assert_eq!(
        module_source
            .matches("verify_and_consume_phase23_native_bgv_opening_v1")
            .count(),
        1
    );
    let transcript = module_source
        .split("fn collective_encryption_transcript_digest_v1")
        .nth(1)
        .expect("fresh encryption transcript")
        .split("fn scaled_public_error")
        .next()
        .expect("transcript source slice");
    for binding in [
        "hash.update(&topology.layout_digest)",
        "hash.update(&chunk_index)",
        "hash.update(&used_slots)",
        "hash.update(&sample_index)",
        "hash.update(encryption_nonce)",
    ] {
        assert!(
            transcript.contains(binding),
            "missing lineage binding: {binding}"
        );
    }
    assert!(!transcript.contains("plaintext.digest"));
    assert!(!transcript.contains("rns_polynomial_digest"));
    assert!(!transcript.contains("collective_lineage_digest("));
    assert!(!transcript.contains("Vec<"));
    assert!(transcript.contains("let mut hash = Box::new(Keccak256::new())"));
    assert!(transcript.contains("hash.finalize_into(&mut digest)"));
    assert!(transcript.contains("drop(hash)"));
    assert!(!transcript.contains("hash.finalize()"));
    let entropy = module_source
        .split("fn derive_collective_encryption_nonce_v1")
        .nth(1)
        .expect("nonce derivation")
        .split("fn entropy_probe_has_short_period")
        .next()
        .expect("nonce derivation source slice");
    assert_eq!(entropy.matches(".fill_bytes").count(), 2);
    assert!(entropy.contains("COLLECTIVE_ENCRYPTION_NONCE_DOMAIN_V1"));
    assert_eq!(entropy.matches("ZeroizingEntropyProbe([0; 32])").count(), 2);
    assert!(entropy.contains("hash.update(&first.0)"));
    assert!(entropy.contains("hash.update(&second.0)"));
    assert!(entropy.contains("let mut hash = Box::new(Keccak256::new())"));
    assert!(entropy.contains("let mut nonce = ZeroizingEncryptionNonce::zeroed()"));
    assert!(entropy.contains("hash.finalize_into(nonce.as_mut_bytes())"));
    assert!(entropy.contains("drop(hash)"));
    assert!(entropy.contains("nonce.is_zero()"));
    assert!(!entropy.contains("let nonce = hash.finalize()"));
    let short_period = module_source
        .split("fn entropy_probe_has_short_period")
        .nth(1)
        .expect("short-period rejection")
        .split("fn sample_ternary_zeroizing")
        .next()
        .expect("short-period source slice");
    assert!(short_period.contains("probe[period..]"));
    assert!(short_period.contains(".zip(&probe[..probe.len() - period])"));
    assert!(!short_period.contains("is_multiple_of"));
    let native_encryption = module_source
        .split("fn encrypt_collective_native_with_opening")
        .nth(1)
        .expect("native encryption")
        .split("fn collective_lineage_digest")
        .next()
        .expect("native encryption source slice");
    let nonce_position = native_encryption
        .find("derive_collective_encryption_nonce_v1(random)?")
        .expect("nonce derivation before witnesses");
    let witness_position = native_encryption
        .find("sample_nonzero_ternary_zeroizing(profile, random)?")
        .expect("ephemeral witness sampling");
    assert!(nonce_position < witness_position);
    // Public multipliers deliberately retain their own digest-bound
    // evaluation lineage; this test certifies fresh encryption only.
    assert_eq!(
        module_source.matches("plaintext.digest.as_slice()").count(),
        2
    );
    assert!(module_source.contains("This evaluation operand is public"));
}
#[test]
fn native_whole_owner_reference_surface_is_test_only() {
    let source = include_str!("../collective.rs");
    assert!(source.contains(
        "#[cfg(test)]\n#[derive(Clone, Debug, PartialEq, Eq)]\npub struct ZkAmsMkheCollectiveCiphertextV1"
    ));
    for marker in [
        "const COLLECTIVE_ADD_DOMAIN_V1",
        "const COLLECTIVE_SUB_DOMAIN_V1",
        "const COLLECTIVE_PLAINTEXT_MUL_DOMAIN_V1",
        "const COLLECTIVE_AUTOMORPHISM_DOMAIN_V1",
        "const COLLECTIVE_MULTIPLY_DOMAIN_V1",
        "const COLLECTIVE_LEVEL_ONE_DOMAIN_V1",
        "fn clear_secret_canonical_plaintext_v1",
        "struct ZeroizingSecretCoefficients",
        "impl Drop for ZeroizingSecretCoefficients",
        "struct ZeroizingCanonicalPlaintext",
        "impl Drop for ZeroizingCanonicalPlaintext",
        "pub(super) struct ZkAmsMkheCollectiveEncryptionOpeningV1",
        "impl core::fmt::Debug for ZkAmsMkheCollectiveEncryptionOpeningV1",
        "impl Drop for ZkAmsMkheCollectiveEncryptionOpeningV1",
        "impl ZkAmsMkheCollectiveEncryptionOpeningV1",
        "pub fn public_a_wire",
        "pub fn collective_public_b_wire",
        "fn compact_binary_with_profile",
        "fn multiply_with_profile",
        "fn compact_plaintext_mul_with_profile",
        "fn compact_automorphism_with_profile",
        "pub struct ZkAmsMkheCollectiveLevelOneV1",
        "impl core::fmt::Debug for ZkAmsMkheCollectiveLevelOneV1",
        "impl ZkAmsMkheCollectiveLevelOneV1",
        "fn level_one_binary_with_profile",
        "fn level_one_plaintext_mul_with_profile",
        "fn level_one_automorphism_with_profile",
        "pub(super) fn validate_compact_for_key",
        "fn collective_lineage_digest",
        "fn collective_encryption_transcript_digest_v1",
        "fn scaled_public_error",
        "fn derive_natural_lift_effective_error_zero",
    ] {
        let positions: Vec<_> = source
            .match_indices(marker)
            .map(|(position, _)| position)
            .collect();
        assert!(
            !positions.is_empty(),
            "missing native reference item: {marker}"
        );
        for position in positions {
            let preceding_line = source[..position]
                .lines()
                .rev()
                .find(|line| !line.trim().is_empty())
                .map(str::trim);
            assert_eq!(
                preceding_line,
                Some("#[cfg(test)]"),
                "native reference item is production-compiled: {marker}",
            );
        }
    }
    for marker in ["impl ZkAmsMkheCollectiveCiphertextV1"] {
        let positions: Vec<_> = source
            .match_indices(marker)
            .map(|(position, _)| position)
            .collect();
        assert!(
            !positions.is_empty(),
            "missing native reference item: {marker}"
        );
        for position in positions {
            let preceding_line = source[..position]
                .lines()
                .rev()
                .find(|line| !line.trim().is_empty())
                .map(str::trim);
            assert_eq!(preceding_line, Some("#[cfg(test)]"), "ungated {marker}");
        }
    }
}
#[test]
fn collective_encryption_rejects_zero_repeating_and_failing_entropy() {
    let mut healthy = KatRandom::new(b"entropy-healthy");
    let nonce = derive_collective_encryption_nonce_v1(&mut healthy).unwrap();
    assert_ne!(nonce.as_bytes(), &[0; 32]);
    drop(nonce);
    let mut zero = ConstantRandom(0);
    assert!(matches!(
        derive_collective_encryption_nonce_v1(&mut zero),
        Err(ZkAmsMkheErrorV1::RandomUnavailable)
    ));
    let mut constant = ConstantRandom(0xa5);
    assert!(matches!(
        derive_collective_encryption_nonce_v1(&mut constant),
        Err(ZkAmsMkheErrorV1::RandomUnavailable)
    ));
    let mut repeating = RepeatedHealthyBlockRandom;
    assert!(matches!(
        derive_collective_encryption_nonce_v1(&mut repeating),
        Err(ZkAmsMkheErrorV1::RandomUnavailable)
    ));
    let mut odd_period = DistinctOddPeriodProbeRandom::new();
    assert!(matches!(
        derive_collective_encryption_nonce_v1(&mut odd_period),
        Err(ZkAmsMkheErrorV1::RandomUnavailable)
    ));
    let mut failing = FailingRandom;
    assert!(matches!(
        derive_collective_encryption_nonce_v1(&mut failing),
        Err(ZkAmsMkheErrorV1::RandomUnavailable)
    ));
    let profile = test_profile();
    let (key, _) = test_key(0x76);
    let mut repeating = RepeatedHealthyBlockRandom;
    assert!(matches!(
        try_encrypt_test_with_random(
            &profile,
            &key,
            &[0; 8],
            0,
            b"repeating-entropy-encryption",
            &mut repeating,
        ),
        Err(ZkAmsMkheErrorV1::RandomUnavailable)
    ));
    let mut failing = FailingRandom;
    assert!(matches!(
        try_encrypt_test_with_random(
            &profile,
            &key,
            &[0; 8],
            0,
            b"failing-entropy-encryption",
            &mut failing,
        ),
        Err(ZkAmsMkheErrorV1::RandomUnavailable)
    ));
}
#[test]
fn encryption_nonce_allocation_is_stable_and_zeroizes_on_success_error_and_unwind() {
    let reset_drops = || ENCRYPTION_NONCE_ZEROIZED_DROPS_V1.with(|drops| drops.set(0));
    let drop_count = || {
        ENCRYPTION_NONCE_ZEROIZED_DROPS_V1
            .try_with(std::cell::Cell::get)
            .unwrap_or(0)
    };
    reset_drops();
    let mut nonce = ZeroizingEncryptionNonce::zeroed();
    nonce.as_mut_bytes().fill(0x39);
    let address = nonce.as_bytes().as_ptr();
    let moved = nonce;
    assert_eq!(moved.as_bytes().as_ptr(), address);
    drop(moved);
    assert_eq!(drop_count(), 1);
    reset_drops();
    let error = {
        let mut nonce = ZeroizingEncryptionNonce::zeroed();
        nonce.as_mut_bytes().fill(0x72);
        Err::<(), _>(ZkAmsMkheErrorV1::RandomUnavailable)
    };
    assert_eq!(error, Err(ZkAmsMkheErrorV1::RandomUnavailable));
    assert_eq!(drop_count(), 1);
    reset_drops();
    let unwind = std::panic::catch_unwind(|| {
        let mut nonce = ZeroizingEncryptionNonce::zeroed();
        nonce.as_mut_bytes().fill(0xa4);
        let address = nonce.as_bytes().as_ptr();
        let moved = nonce;
        assert_eq!(moved.as_bytes().as_ptr(), address);
        panic!("exercise heap-stable nonce drop during unwind");
    });
    assert!(unwind.is_err());
    assert_eq!(drop_count(), 1);
    let source = include_str!("../collective.rs");
    let test_module_marker = "\n#[cfg(test)]\n#[path = \"collective/tests.rs\"]\nmod tests;";
    assert_eq!(source.matches(test_module_marker).count(), 1);
    let production = source
        .split_once(test_module_marker)
        .expect("extracted test-module boundary")
        .0;
    assert!(production.contains("struct ZeroizingEncryptionNonce(Box<[u8; 32]>)"));
    assert!(production.contains("Self(Box::new([0; 32]))"));
    assert!(production.contains("clear_secret_bytes_v1(self.0.as_mut())"));
    assert!(!production.contains("struct ZeroizingEncryptionNonce([u8; 32])"));
}
#[test]
fn collective_opening_debug_is_redacted_and_drop_zeroizes_every_witness() {
    let profile = test_profile();
    let (key, _) = test_key(0x77);
    let (_, mut opening, ..) = encrypt_test_with_opening(
        &profile,
        &key,
        &[16, 15, 14, 13, 12, 11, 10, 9],
        47,
        b"opening-redaction-drop",
    );
    let debug = format!("{opening:?}");
    assert_eq!(debug.matches("[REDACTED]").count(), 6);
    assert!(!debug.contains(&hex::encode(
        opening.input_identity.encryption_nonce.as_bytes()
    )));
    assert!(!debug.contains("coefficients:"));
    let audit = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    opening.arm_drop_zeroization_audit(audit.clone());
    drop(opening);
    assert!(audit.load(std::sync::atomic::Ordering::SeqCst));
}
#[test]
fn natural_lift_effective_error_uses_the_exact_centered_boundary() {
    let profile = release_profile_v1();
    let mut canonical = vec![[0; 32]; profile.ring_degree];
    canonical[0] = super::super::T256_CENTERED_MAX_BE_V1;
    canonical[1] = super::super::T256_CENTERED_MAX_BE_V1;
    for byte in canonical[1].iter_mut().rev() {
        let (incremented, carried) = byte.overflowing_add(1);
        *byte = incremented;
        if !carried {
            break;
        }
    }
    let mut sampled = SecretPolynomial {
        coefficients: vec![0; profile.ring_degree],
    };
    sampled.coefficients[0] = i64::from(profile.error_eta);
    sampled.coefficients[1] = -i64::from(profile.error_eta);
    let effective =
        derive_natural_lift_effective_error_zero(&profile, &canonical, &sampled).unwrap();
    assert_eq!(effective.coefficients[0], i64::from(profile.error_eta));
    assert_eq!(effective.coefficients[1], -i64::from(profile.error_eta) - 1);
    assert!(
        effective.coefficients[2..]
            .iter()
            .all(|coefficient| *coefficient == 0)
    );
    assert!(matches!(
        derive_natural_lift_effective_error_zero(&test_profile(), &canonical[..8], &sampled),
        Err(ZkAmsMkheErrorV1::InvalidCiphertext)
    ));
}
#[test]
fn persistent_commitment_blindings_have_exact_shape_order_and_redaction() {
    reset_persistent_blinding_drop_audits();
    let mut random = ScriptedPersistentBlindingRandom::from_scalars(
        1..=u64::try_from(ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1)
            .expect("release chunk count fits u64"),
    );
    let owner = ZeroizingCpkMembershipBlindingsV1::sample(&mut random)
        .expect("eight nonzero scripted blindings");
    assert_eq!(
        random.request_lengths,
        vec![PERSISTENT_BLINDING_ENTROPY_BYTES_V1; ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1]
    );
    for (index, scalar) in owner.as_array().iter().enumerate() {
        assert_eq!(
            *scalar,
            Scalar::from_u64(u64::try_from(index + 1).expect("test index fits u64"))
        );
        assert!(!scalar.is_zero());
    }
    let canonical_bytes = owner
        .as_array()
        .iter()
        .flat_map(|scalar| scalar.to_le_bytes())
        .collect::<Vec<_>>();
    assert_eq!(canonical_bytes.len(), PERSISTENT_BLINDING_STATE_BYTES_V1);
    assert_eq!(canonical_bytes.len(), 256);
    assert_eq!(
        core::mem::size_of::<ZeroizingCpkMembershipBlindingsV1>(),
        256
    );
    assert_eq!(
        format!("{owner:?}"),
        "PersistentSecretCommitmentBlindingsV1([REDACTED])"
    );
    assert_eq!(persistent_blinding_drop_audits(), (8, 0));
    drop(owner);
    assert_eq!(persistent_blinding_drop_audits(), (8, 1));
}
#[test]
fn persistent_secret_membership_view_rejects_wrong_shape_and_non_ternary_state() {
    let exact_len =
        ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1 * ZK_AMS_MEMBERSHIP_CHUNK_COEFFICIENTS_V1;
    let mut valid = SecretPolynomial {
        coefficients: vec![0; exact_len],
    };
    valid.coefficients[0] = -1;
    valid.coefficients[exact_len - 1] = 1;
    let narrowed = ZeroizingT256MembershipCoefficientsV1::from_bounded(&valid, 1)
        .expect("exact release ternary secret narrows without changing order");
    assert_eq!(narrowed.as_slice().len(), exact_len);
    assert_eq!(narrowed.as_slice()[0], -1);
    assert_eq!(narrowed.as_slice()[exact_len - 1], 1);
    drop(narrowed);
    let short = SecretPolynomial {
        coefficients: vec![0; exact_len - 1],
    };
    assert!(matches!(
        ZeroizingT256MembershipCoefficientsV1::from_bounded(&short, 1),
        Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
    ));
    valid.coefficients[ZK_AMS_MEMBERSHIP_CHUNK_COEFFICIENTS_V1] = 2;
    assert!(matches!(
        ZeroizingT256MembershipCoefficientsV1::from_bounded(&valid, 1),
        Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
    ));
}
#[test]
fn state_owned_cpk_commitments_reject_secret_blinding_order_and_splice_changes() {
    let exact_len =
        ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1 * ZK_AMS_MEMBERSHIP_CHUNK_COEFFICIENTS_V1;
    let mut coefficients = vec![0_i8; exact_len];
    coefficients[0] = -1;
    coefficients[ZK_AMS_MEMBERSHIP_CHUNK_COEFFICIENTS_V1] = 1;
    let blindings = core::array::from_fn(|index| {
        Scalar::from_u64(u64::try_from(index + 17).expect("test index fits u64"))
    });
    let bound = ZkAmsT256MembershipBoundV1::One;
    let expected = commit_cpk_membership_opening_v1(&coefficients, &blindings, bound)
        .expect("exact state-owned opening commits all eight chunks");
    ensure_state_owned_cpk_commitments_v1(&expected, &expected)
        .expect("the exact ordered set is accepted");
    let mut changed_coefficients = coefficients[..ZK_AMS_MEMBERSHIP_CHUNK_COEFFICIENTS_V1].to_vec();
    changed_coefficients[0] = 0;
    let changed_secret_point = commit_zk_ams_t256_membership_chunk_v1(
        ZkAmsT256MembershipBoundV1::One,
        &changed_coefficients,
        &blindings[0],
    )
    .expect("mutated ternary chunk remains a valid commitment opening");
    assert_ne!(changed_secret_point, expected[0]);
    let changed_blinding_point = commit_zk_ams_t256_membership_chunk_v1(
        ZkAmsT256MembershipBoundV1::One,
        &coefficients[..ZK_AMS_MEMBERSHIP_CHUNK_COEFFICIENTS_V1],
        &Scalar::from_u64(0x5a),
    )
    .expect("replacement nonzero blinding remains a valid opening");
    assert_ne!(changed_blinding_point, expected[0]);
    let mut reordered = expected;
    reordered.swap(0, 1);
    assert!(matches!(
        ensure_state_owned_cpk_commitments_v1(&reordered, &expected),
        Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
    ));
    let mut duplicated = expected;
    duplicated[1] = duplicated[0];
    assert!(matches!(
        ensure_state_owned_cpk_commitments_v1(&duplicated, &expected),
        Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
    ));
    let mut spliced = expected;
    spliced[0] = changed_secret_point;
    assert!(matches!(
        ensure_state_owned_cpk_commitments_v1(&spliced, &expected),
        Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
    ));
    let mut zero_blinding = blindings;
    zero_blinding[3] = Scalar::zero();
    assert!(matches!(
        commit_cpk_membership_opening_v1(&coefficients, &zero_blinding, bound),
        Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
    ));
    assert!(matches!(
        commit_cpk_membership_opening_v1(&coefficients[..exact_len - 1], &blindings, bound),
        Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
    ));
}
#[test]
fn persistent_commitment_blindings_retry_zero_without_reordering() {
    reset_persistent_blinding_drop_audits();
    let mut random =
        ScriptedPersistentBlindingRandom::from_scalars(core::iter::once(0).chain(1..=8));
    let owner = ZeroizingCpkMembershipBlindingsV1::sample(&mut random)
        .expect("zero is retried before the ordered nonzero values");
    assert_eq!(random.request_lengths, vec![64; 9]);
    assert_eq!(
        owner
            .as_array()
            .iter()
            .map(|scalar| scalar.to_le_bytes()[0])
            .collect::<Vec<_>>(),
        (1_u8..=8).collect::<Vec<_>>()
    );
    assert_eq!(persistent_blinding_drop_audits(), (9, 0));
    drop(owner);
    assert_eq!(persistent_blinding_drop_audits(), (9, 1));
}
#[test]
fn persistent_commitment_blindings_stop_at_exact_zero_rejection_ceiling() {
    reset_persistent_blinding_drop_audits();
    let mut random = ScriptedPersistentBlindingRandom::from_scalars(core::iter::repeat_n(
        0,
        MAX_RANDOM_REJECTION_ATTEMPTS_V1,
    ));
    assert!(matches!(
        ZeroizingCpkMembershipBlindingsV1::sample(&mut random),
        Err(ZkAmsMkheErrorV1::RandomUnavailable)
    ));
    assert_eq!(random.next, MAX_RANDOM_REJECTION_ATTEMPTS_V1);
    assert_eq!(
        random.request_lengths,
        vec![PERSISTENT_BLINDING_ENTROPY_BYTES_V1; MAX_RANDOM_REJECTION_ATTEMPTS_V1]
    );
    assert_eq!(
        persistent_blinding_drop_audits(),
        (MAX_RANDOM_REJECTION_ATTEMPTS_V1, 1)
    );
}
#[test]
fn persistent_commitment_blindings_erase_partial_state_on_rng_failure() {
    reset_persistent_blinding_drop_audits();
    let mut random = PartialFailurePersistentBlindingRandom {
        successful_requests: 3,
        calls: 0,
        partial_bytes: 23,
    };
    assert!(matches!(
        ZeroizingCpkMembershipBlindingsV1::sample(&mut random),
        Err(ZkAmsMkheErrorV1::RandomUnavailable)
    ));
    assert_eq!(random.calls, 4);
    assert_eq!(persistent_blinding_drop_audits(), (4, 1));
}
#[test]
fn persistent_commitment_blindings_erase_partial_state_during_unwind() {
    reset_persistent_blinding_drop_audits();
    let mut random = PartialPanicPersistentBlindingRandom {
        successful_requests: 2,
        calls: 0,
        partial_bytes: 41,
    };
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = ZeroizingCpkMembershipBlindingsV1::sample(&mut random);
    }));
    assert!(panic.is_err());
    assert_eq!(random.calls, 3);
    assert_eq!(persistent_blinding_drop_audits(), (3, 1));
}
#[test]
fn persistent_commitment_blindings_move_without_duplicate_drop() {
    fn move_once(owner: ZeroizingCpkMembershipBlindingsV1) -> ZeroizingCpkMembershipBlindingsV1 {
        owner
    }
    reset_persistent_blinding_drop_audits();
    let mut random = ScriptedPersistentBlindingRandom::from_scalars(1..=8);
    let owner =
        ZeroizingCpkMembershipBlindingsV1::sample(&mut random).expect("scripted nonzero blindings");
    let owner = move_once(owner);
    let owner = move_once(owner);
    assert_eq!(persistent_blinding_drop_audits(), (8, 0));
    drop(owner);
    assert_eq!(persistent_blinding_drop_audits(), (8, 1));
}
#[test]
fn opaque_party_state_debug_and_api_do_not_expose_rlwe_coefficients() {
    let state = ZkAmsMkheCollectivePartyStateV1 {
        persistent_direct_opening: PersistentDirectOpeningOwnerV1 {
            axes: PersistentDirectOpeningAxesV1 {
                profile_digest: [1; 32],
                security_certificate_digest: [2; 32],
                roster_digest: [3; 32],
                key_material_digest: [4; 32],
                epoch: 1,
                cpk_transcript_digest: [5; 32],
                party_index: 0,
                party: test_parties().parties[0],
                public_share_digest: [6; 32],
            },
            verified_binding: None,
            blindings: test_persistent_secret_commitment_blindings(),
            secret: SecretPolynomial {
                coefficients: vec![1, -1, 0],
            },
            retained_commitment_wire: [[7; PERSISTENT_OPENING_POINT_WIRE_BYTES_V1];
                ZK_AMS_MKHE_PERSISTENT_MEMBERSHIP_CHUNKS_V1],
        },
        public_error: SecretPolynomial {
            coefficients: vec![2, -2, 0],
        },
        party_local_rkg_ephemeral_opening: None,
        party_local_rkg_ephemeral_creation_mask: 0,
    };
    let debug = format!("{state:?}");
    assert_eq!(debug.matches("[REDACTED]").count(), 3);
    assert!(!debug.contains("-1"));
    assert!(!debug.contains("-2"));
    assert!(!debug.contains("17"));
    assert_eq!(state.secret().coefficients.len(), 3);
    assert_eq!(state.public_error().coefficients.len(), 3);
}
#[test]
fn in_place_party_b_finish_matches_owned_polynomial_algebra() {
    let profile = test_profile();
    let product = RnsPolynomial::from_unsigned(&profile, &[1, 3, 5, 7, 9, 11, 13, 15]).unwrap();
    let error = SecretPolynomial {
        coefficients: vec![-2, -1, 0, 1, 2, 1, 0, -1],
    };
    let expected = product
        .negate(&profile)
        .unwrap()
        .add(&scaled_public_error(&profile, &error).unwrap().0, &profile)
        .unwrap();
    let mut actual = product;
    negate_and_add_scaled_error_in_place(&mut actual, &error, &profile).unwrap();
    assert_eq!(actual, expected);
}
#[test]
fn in_place_collective_aggregation_matches_owned_algebra_and_rejects_atomically() {
    let profile = test_profile();
    let first = RnsPolynomial::from_unsigned(&profile, &[1, 3, 5, 7, 9, 11, 13, 15]).unwrap();
    let second = RnsPolynomial::from_unsigned(&profile, &[2, 4, 6, 8, 10, 12, 14, 16]).unwrap();
    let expected = first.add(&second, &profile).unwrap();
    let mut actual = first.coefficients.clone();
    add_canonical_residues_in_place_v1(&mut actual, &second.coefficients, &profile).unwrap();
    assert_eq!(actual, expected.coefficients);
    let accepted = actual.clone();
    assert_eq!(
        add_canonical_residues_in_place_v1(
            &mut actual,
            &second.coefficients[..second.coefficients.len() - 1],
            &profile,
        ),
        Err(ZkAmsMkheErrorV1::InvalidPolynomial)
    );
    assert_eq!(actual, accepted);
    let mut noncanonical = second.coefficients.clone();
    noncanonical[profile.ring_degree] = profile.moduli[1];
    assert_eq!(
        add_canonical_residues_in_place_v1(&mut actual, &noncanonical, &profile),
        Err(ZkAmsMkheErrorV1::InvalidPolynomial)
    );
    assert_eq!(actual, accepted);
}
#[test]
fn collective_aggregation_source_has_no_party_b_release_copy_or_add_output() {
    let source = include_str!("../collective.rs");
    let aggregate = source
        .split("fn aggregate_zk_ams_mkhe_collective_public_key_v1")
        .nth(1)
        .expect("collective aggregate")
        .split("fn staged_collective_public_key_admission_digest_v1")
        .next()
        .expect("collective aggregate corridor");
    assert!(aggregate.contains("add_canonical_residues_in_place_v1"));
    assert!(aggregate.contains("Validate the complete input before mutating"));
    assert!(!aggregate.contains("party_public_b.residues().to_vec()"));
    assert!(!aggregate.contains("aggregate_b = aggregate_b.add"));
}
#[test]
fn prepared_party_generator_has_no_separate_scaled_error_table() {
    let source = include_str!("../collective.rs");
    let generator = source
        .split("pub fn generate_zk_ams_mkhe_collective_party_state_with_prepared_public_a_v1")
        .nth(1)
        .expect("prepared generator")
        .split("fn aggregate_zk_ams_mkhe_collective_public_key_v1")
        .next()
        .expect("prepared generator body");
    assert!(generator.contains("negate_and_add_scaled_error_in_place"));
    assert!(!generator.contains("let scaled_error"));
    assert!(!generator.contains(".negate(&profile)?"));
    assert!(!generator.contains(".add(&scaled_error"));
    assert!(generator.contains("multiply_public_residues_by_secret_signed_v1"));
    assert!(!generator.contains("secret.as_rns"));
    assert!(!generator.contains("let secret_rns"));
    assert!(!generator.contains("public_a.residues().to_vec()"));
    assert!(!generator.contains("public_a_native"));
    let validator = source
        .split("fn validate_collective_public_key_share_unsealed_v1")
        .nth(1)
        .expect("share validator")
        .split("fn active_collective_public_key_share_admission_digest_v1")
        .next()
        .expect("share validator body");
    assert!(!validator.contains("zk_ams_mkhe_active_collective_public_a_v1("));
    assert!(validator.contains("verify_zk_ams_mkhe_active_collective_public_key_v1("));
}
