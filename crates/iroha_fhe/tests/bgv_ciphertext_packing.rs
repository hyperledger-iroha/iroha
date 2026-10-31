//! Independent bit-level canonical ciphertext wire vectors and malformed controls.
//!
//! Imported word fixtures carry claimed bounds only; no honest-encryption or
//! external ciphertext authentication is inferred from canonical decoding.
use iroha_fhe::{
    bgv::{BgvParameters, BgvProfile, Ciphertext, NoiseBound, cost::CostModel},
    natural::Natural,
};

fn profile() -> BgvProfile {
    let mut parameters = BgvParameters::derive(2, 3, 2, (10, 1), (60, 1), 1).unwrap();
    parameters.main_primes.clear();
    parameters.main_roots.clear();
    for bits in [10, 11, 17, 31, 50, 61, 62] {
        let candidate = BgvParameters::derive(2, 3, 2, (bits, 1), (60, 1), 1).unwrap();
        parameters.main_primes.push(candidate.main_primes[0]);
        parameters.main_roots.push(candidate.main_roots[0]);
    }
    parameters.digit_primes = 2;
    BgvProfile::new(parameters).unwrap()
}

/// One bit per step, with fresh byte alignment at every public residue limb.
fn reference_bytes(
    profile: &BgvProfile,
    level: usize,
    components: usize,
    precision: u32,
    words: &[u64],
) -> Vec<u8> {
    let mut bytes = profile.identity().to_vec();
    bytes.extend_from_slice(&(level as u16).to_le_bytes());
    bytes.extend_from_slice(&[components as u8, precision as u8]);
    for index in 0..profile.noise_bound_words() {
        bytes.extend_from_slice(&u64::from(index == 0).to_le_bytes());
    }
    for component in 0..components {
        for limb in 0..=level {
            let prime = profile.parameters().main_primes[limb];
            let bits = (64 - prime.leading_zeros()) as usize;
            let start = bytes.len();
            bytes.resize(start + (profile.degree() * bits).div_ceil(8), 0);
            for coefficient in 0..profile.degree() {
                let value =
                    words[(component * (level + 1) + limb) * profile.degree() + coefficient];
                for bit in 0..bits {
                    let at = coefficient * bits + bit;
                    bytes[start + at / 8] |= (((value >> bit) & 1) as u8) << (at % 8);
                }
            }
        }
    }
    bytes
}

fn words(profile: &BgvProfile, level: usize, components: usize) -> Vec<u64> {
    let mut words = Vec::new();
    for component in 0..components {
        for &prime in &profile.parameters().main_primes[..=level] {
            for coefficient in 0..profile.degree() {
                words.push(match (coefficient + component) % 4 {
                    0 => 0,
                    1 => prime - 1,
                    2 => prime / 2,
                    _ => 1,
                });
            }
        }
    }
    words
}

#[test]
fn mixed_prime_widths_match_independent_bits_at_every_level_and_precision() {
    let profile = profile();
    for level in 0..=profile.top_level() {
        for components in 2..=3 {
            let words = words(&profile, level, components);
            for precision in 1..=2 {
                let ciphertext = Ciphertext::from_canonical_words(
                    &profile,
                    profile.plaintext_precision(precision).unwrap(),
                    level,
                    components,
                    &words,
                    &NoiseBound::new(Natural::from_u64(1)),
                )
                .unwrap();
                let expected = reference_bytes(&profile, level, components, precision, &words);
                assert_eq!(ciphertext.to_bytes(&profile).unwrap(), expected);
                assert_eq!(
                    Ciphertext::encoded_len(&profile, level, components).unwrap(),
                    expected.len()
                );
                assert_eq!(
                    CostModel::new(&profile)
                        .ciphertext_bytes(level, components)
                        .unwrap(),
                    expected.len() as u64
                );
                let decoded = Ciphertext::from_bytes(&profile, &expected).unwrap();
                assert_eq!(decoded.canonical_words(), words);
                assert_eq!(decoded.shape(), ciphertext.shape());
                assert_eq!(decoded.to_bytes(&profile).unwrap(), expected);
            }
        }
    }
}

#[test]
fn each_limb_rejects_padding_and_noncanonical_residues_without_alias_frames() {
    let profile = profile();
    let level = profile.top_level();
    let components = 3;
    let words = words(&profile, level, components);
    let expected = reference_bytes(&profile, level, components, 2, &words);
    let prefix = 36 + 8 * profile.noise_bound_words();
    let mut start = prefix;
    for component in 0..components {
        for limb in 0..=level {
            let prime = profile.parameters().main_primes[limb];
            let bits = (64 - prime.leading_zeros()) as usize;
            let count = (profile.degree() * bits).div_ceil(8);
            if !(profile.degree() * bits).is_multiple_of(8) {
                let mut nonzero_padding = expected.clone();
                nonzero_padding[start + count - 1] |= 0x80;
                assert!(Ciphertext::from_bytes(&profile, &nonzero_padding).is_err());
            }
            let mut invalid_words = words.clone();
            invalid_words[(component * (level + 1) + limb) * profile.degree() + 3] = prime;
            let noncanonical = reference_bytes(&profile, level, components, 2, &invalid_words);
            assert!(Ciphertext::from_bytes(&profile, &noncanonical).is_err());
            start += count;
        }
    }
    assert_eq!(start, expected.len());
    for length in 0..expected.len() {
        assert!(Ciphertext::from_bytes(&profile, &expected[..length]).is_err());
    }
    let mut trailing = expected.clone();
    trailing.push(0);
    assert!(Ciphertext::from_bytes(&profile, &trailing).is_err());
    for (offset, replacement) in [(0, expected[0] ^ 1), (34, 1), (35, 0), (35, 3)] {
        let mut altered = expected.clone();
        altered[offset] = replacement;
        assert!(Ciphertext::from_bytes(&profile, &altered).is_err());
    }
    let mut invalid_level = expected.clone();
    invalid_level[32..34].fill(0xff);
    assert!(Ciphertext::from_bytes(&profile, &invalid_level).is_err());
    let mut oversized_bound = expected.clone();
    oversized_bound[36..prefix].fill(0xff);
    assert!(Ciphertext::from_bytes(&profile, &oversized_bound).is_err());
    // Fixed-u64 residue bytes are not another accepted first-release encoding.
    let mut unpacked_words = expected[..prefix].to_vec();
    for word in &words {
        unpacked_words.extend_from_slice(&word.to_le_bytes());
    }
    assert!(Ciphertext::from_bytes(&profile, &unpacked_words).is_err());
}

#[test]
fn unselected_f257_single_limb_frame_fits_the_unchanged_input_byte_ceiling() {
    let profile =
        BgvProfile::new(BgvParameters::derive(16, 257, 3, (60, 24), (60, 2), 2).unwrap()).unwrap();
    let precision = profile.plaintext_precision(1).unwrap();
    let mut shape = profile.plan_fresh(precision).unwrap();
    while shape.level() > 0 {
        shape = profile.plan_switch_down(&shape).unwrap();
    }
    assert_eq!(shape.bound(), &Natural::from_u64(556_231_950_720));
    assert_eq!(Ciphertext::encoded_len(&profile, 0, 2).unwrap(), 983_260);
    // Public raw-residue fixtures exercise the real full-size byte codec. They
    // are not honest ciphertexts, authenticated inputs or local-bootstrap owners.
    let words = words(&profile, 0, 2);
    let ciphertext = Ciphertext::from_canonical_words(
        &profile,
        precision,
        0,
        2,
        &words,
        &NoiseBound::new(shape.bound().clone()),
    )
    .unwrap();
    let bytes = ciphertext.to_bytes(&profile).unwrap();
    assert_eq!(bytes.len(), 983_260);
    assert_eq!(1_048_576 - bytes.len(), 65_316);
    let decoded = Ciphertext::from_bytes(&profile, &bytes).unwrap();
    assert_eq!(decoded.canonical_words(), words);
    assert_eq!(decoded.shape(), shape);
    assert_eq!(decoded.to_bytes(&profile).unwrap(), bytes);
}
