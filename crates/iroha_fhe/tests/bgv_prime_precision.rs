//! Independent prime-power slot, master-error and physical-phase regressions.
//!
//! These arithmetic fixtures do not select secure parameters or implement refresh.
mod support;

use iroha_fhe::{
    bgv::{
        BgvError, BgvParameters, BgvProfile, Ciphertext, Plaintext, PlaintextPrecision, SecretKey,
        SlotEncoding,
        fixtures::{self, FixtureEntropy},
    },
    natural::Natural,
};
use num_bigint::{BigInt, Sign};
use support::oracle;

fn fixture_profile(log_degree: u32, exponent: u32) -> BgvProfile {
    BgvProfile::new(
        BgvParameters::derive(log_degree, 257, exponent, (50, 4), (60, 2), 2)
            .expect("unqualified arithmetic parameters"),
    )
    .expect("profile")
}

fn integer(value: &Natural) -> BigInt {
    let bytes: Vec<_> = value
        .words_le()
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .collect();
    BigInt::from_bytes_le(Sign::Plus, &bytes)
}

fn multiply_mod(lhs: u64, rhs: u64, modulus: u64) -> u64 {
    u64::try_from(u128::from(lhs) * u128::from(rhs) % u128::from(modulus)).expect("reduced word")
}

fn power_mod(mut base: u64, mut exponent: u64, modulus: u64) -> u64 {
    let mut value = 1;
    while exponent != 0 {
        if exponent & 1 != 0 {
            value = multiply_mod(value, base, modulus);
        }
        base = multiply_mod(base, base, modulus);
        exponent >>= 1;
    }
    value
}

/// Construct the phase from integer CRT and schoolbook/Kronecker convolution.
fn phase(profile: &BgvProfile, secret: &SecretKey, ciphertext: &Ciphertext) -> Vec<BigInt> {
    let primes = &profile.parameters().main_primes[..=ciphertext.level()];
    let modulus = oracle::product(primes);
    let component_words = primes.len() * profile.degree();
    let words = ciphertext.canonical_words();
    let s = oracle::small(&fixtures::secret_coefficients(secret));
    let mut secret_power = vec![BigInt::from(0); profile.degree()];
    secret_power[0] = BigInt::from(1);
    let mut result = vec![BigInt::from(0); profile.degree()];
    assert_eq!(words.len(), component_words * ciphertext.component_count());
    for component in words.chunks_exact(component_words) {
        let lifted = oracle::lift(component, primes);
        result = oracle::poly_add(
            &result,
            &oracle::poly_mul(&lifted, &secret_power, &modulus),
            &modulus,
        );
        secret_power = oracle::poly_mul(&secret_power, &s, &modulus);
    }
    result
        .iter()
        .map(|value| oracle::centered(value, &modulus))
        .collect()
}

fn check_phase(
    profile: &BgvProfile,
    secret: &SecretKey,
    ciphertext: &Ciphertext,
    expected: &[u64],
) {
    assert_eq!(expected.len(), profile.degree());
    let bound = integer(ciphertext.noise_bound().as_natural());
    let modulus = ciphertext.plaintext_precision().modulus();
    for (value, &coefficient) in phase(profile, secret, ciphertext).iter().zip(expected) {
        assert!(
            -&bound <= *value && *value <= bound,
            "phase exceeds public bound"
        );
        assert_eq!(oracle::word_mod(value, modulus), coefficient);
    }
    let plaintext = secret.decrypt(profile, ciphertext).expect("decrypt");
    assert_eq!(
        plaintext.plaintext_precision(),
        ciphertext.plaintext_precision()
    );
    assert_eq!(plaintext.coefficients(), expected);
}

fn check_slots(
    profile: &BgvProfile,
    precision: PlaintextPrecision,
    secret: &SecretKey,
    ciphertext: &Ciphertext,
    expected: &[u64],
) {
    let encoding = SlotEncoding::new(profile, precision).expect("encoding");
    assert_eq!(ciphertext.plaintext_precision(), precision);
    let plaintext = encoding
        .encode(profile, expected)
        .expect("expected encoding");
    check_phase(profile, secret, ciphertext, plaintext.coefficients());
    assert_eq!(
        encoding
            .decode(&secret.decrypt(profile, ciphertext).expect("decrypt"))
            .expect("scalar slots")
            .as_slice(),
        expected
    );
    let bytes = ciphertext.to_bytes(profile).expect("frame");
    assert_eq!(u32::from(bytes[35]), precision.exponent());
    assert_eq!(
        Ciphertext::from_bytes(profile, &bytes).expect("frame decode"),
        *ciphertext
    );
}

#[test]
fn f257_prime_power_roots_and_every_slot_basis_match_direct_inverse_vandermonde() {
    // N=256 exposes the wrong composite-(T-1) slot-count rule at T=257^2.
    for master_exponent in [2, 3] {
        let profile = fixture_profile(8, master_exponent);
        let mut previous_root = None;
        for exponent in 1..=master_exponent {
            let precision = profile.plaintext_precision(exponent).expect("precision");
            let modulus = precision.modulus();
            let encoding = SlotEncoding::new(&profile, precision).expect("encoding");
            assert_eq!(encoding.slots(), 128);
            assert_eq!(encoding.stride(), 2);
            let root = power_mod(3, 257_u64.pow(exponent - 1), modulus);
            assert_eq!(encoding.root(), root);
            assert_eq!(power_mod(root, 128, modulus), modulus - 1);
            assert_eq!(power_mod(root, 256, modulus), 1);
            if let Some((previous_modulus, previous)) = previous_root {
                assert_eq!(root % previous_modulus, previous);
            }
            previous_root = Some((modulus, root));
            let inverse_slots = oracle::word_mod(
                &oracle::inverse(&BigInt::from(128), &BigInt::from(modulus)),
                modulus,
            );
            for slot in 0..128 {
                let mut values = vec![0; 128];
                values[slot] = 1;
                let plaintext = encoding.encode(&profile, &values).expect("basis");
                assert_eq!(plaintext.plaintext_precision(), precision);
                let inverse_beta = power_mod(root, 256 - (2 * slot + 1) as u64, modulus);
                let mut coefficient = inverse_slots;
                for coordinate in 0..128 {
                    assert_eq!(plaintext.coefficients()[2 * coordinate], coefficient);
                    assert_eq!(plaintext.coefficients()[2 * coordinate + 1], 0);
                    coefficient = multiply_mod(coefficient, inverse_beta, modulus);
                }
                assert_eq!(
                    encoding.decode(&plaintext).expect("decode").as_slice(),
                    values
                );
            }
        }
    }
}

#[test]
fn every_effective_precision_attains_the_master_scaled_fresh_bound() {
    for master_exponent in [2, 3] {
        let profile = fixture_profile(4, master_exponent);
        let degree = profile.degree();
        let aligned: Vec<i64> = (0..degree).map(|i| if i == 0 { 1 } else { -1 }).collect();
        let errors = vec![i64::try_from(profile.error_bound()).expect("error bound"); degree];
        let secret = fixtures::secret_key_from_coefficients(&profile, &aligned).expect("secret");
        let uniform: Vec<_> = profile
            .parameters()
            .main_primes
            .iter()
            .flat_map(|&prime| (0..degree as u64).map(move |i| (i * 77 + 5) % prime))
            .collect();
        let public =
            fixtures::public_key_from_coins(&profile, &secret, &uniform, &errors).expect("public");
        let error_term = BigInt::from(profile.master_plaintext_modulus())
            * profile.error_bound()
            * (2 * degree + 1);
        for exponent in 1..=master_exponent {
            let precision = profile.plaintext_precision(exponent).expect("precision");
            let half = (precision.modulus() - 1) / 2;
            let mut coefficients = vec![3; degree];
            coefficients[0] = half;
            let plaintext =
                Plaintext::new(&profile, precision, coefficients.clone()).expect("plain");
            let ciphertext = fixtures::encrypt_from_coins(
                &profile, &public, &plaintext, &aligned, &errors, &errors,
            )
            .expect("encrypt");
            let expected = &error_term + half;
            assert_eq!(integer(ciphertext.noise_bound().as_natural()), expected);
            assert_eq!(phase(&profile, &secret, &ciphertext)[0], expected);
            assert_eq!(
                ciphertext.shape(),
                profile.plan_fresh(precision).expect("plan")
            );
            check_phase(&profile, &secret, &ciphertext, &coefficients);
        }
    }
}

#[test]
fn one_master_key_preserves_every_precision_through_real_arithmetic_and_switching() {
    for master_exponent in [2, 3] {
        let profile = fixture_profile(7, master_exponent);
        let keys =
            fixtures::key_set(&profile, &[5], &mut FixtureEntropy::new([84; 64])).expect("keys");
        let mut encrypted = Vec::new();
        let mut plaintexts = Vec::new();
        for exponent in 1..=master_exponent {
            let precision = profile.plaintext_precision(exponent).expect("precision");
            let modulus = precision.modulus();
            let encoding = SlotEncoding::new(&profile, precision).expect("encoding");
            let edges = [
                0,
                1,
                modulus - 1,
                (modulus - 1) / 2,
                modulus.div_ceil(2),
                256,
            ];
            let values: Vec<_> = (0..128).map(|i| edges[i % edges.len()]).collect();
            let plain = encoding.encode(&profile, &values).expect("plaintext");
            let ciphertext = fixtures::encrypt(
                &profile,
                &keys.public,
                &plain,
                &mut FixtureEntropy::new([85; 64]),
            )
            .expect("encrypt");
            check_slots(&profile, precision, &keys.secret, &ciphertext, &values);
            let doubled: Vec<_> = values.iter().map(|&x| (2 * x) % modulus).collect();
            let squares: Vec<_> = values
                .iter()
                .map(|&x| multiply_mod(x, x, modulus))
                .collect();
            let negatives: Vec<_> = values.iter().map(|&x| (modulus - x) % modulus).collect();
            for (result, expected) in [
                (
                    profile.add(&ciphertext, &ciphertext).expect("sum"),
                    &doubled,
                ),
                (
                    profile.add_plain(&ciphertext, &plain).expect("plain sum"),
                    &doubled,
                ),
                (
                    profile
                        .mul_plain(&ciphertext, &plain)
                        .expect("plain product"),
                    &squares,
                ),
                (profile.negate(&ciphertext).expect("negate"), &negatives),
                (
                    profile.tensor(&ciphertext, &ciphertext).expect("tensor"),
                    &squares,
                ),
            ] {
                check_slots(&profile, precision, &keys.secret, &result, expected);
            }
            let product = profile
                .multiply(&ciphertext, &ciphertext, keys.evaluation.relinearization())
                .expect("product");
            check_slots(&profile, precision, &keys.secret, &product, &squares);
            let moved = profile
                .apply_galois(&ciphertext, keys.evaluation.galois(5).expect("galois key"))
                .expect("automorphism");
            let permuted: Vec<_> = (0..128)
                .map(|j| values[((5 * (2 * j + 1)) % 256 - 1) / 2])
                .collect();
            check_slots(&profile, precision, &keys.secret, &moved, &permuted);
            let mut switched = product;
            while switched.level() != 0 {
                let dropped = profile.parameters().main_primes[switched.level()];
                let expected_bound = (integer(switched.noise_bound().as_natural())
                    + BigInt::from(profile.master_plaintext_modulus())
                        * ((dropped - 1) / 2)
                        * (1 + profile.degree()))
                    / dropped;
                switched = profile.switch_down(&switched).expect("switch");
                assert_eq!(integer(switched.noise_bound().as_natural()), expected_bound);
                check_slots(&profile, precision, &keys.secret, &switched, &squares);
            }
            let frame = ciphertext.to_bytes(&profile).expect("frame");
            for bad_precision in [0, u8::try_from(master_exponent + 1).expect("byte")] {
                let mut malformed = frame.clone();
                malformed[35] = bad_precision;
                assert_eq!(
                    Ciphertext::from_bytes(&profile, &malformed).err(),
                    Some(BgvError::InvalidPlaintextPrecision)
                );
            }
            encrypted.push(ciphertext);
            plaintexts.push(plain);
        }
        let mismatch = Some(BgvError::PlaintextPrecisionMismatch);
        for i in 0..encrypted.len() {
            for j in 0..encrypted.len() {
                if i == j {
                    continue;
                }
                assert_eq!(profile.add(&encrypted[i], &encrypted[j]).err(), mismatch);
                assert_eq!(profile.tensor(&encrypted[i], &encrypted[j]).err(), mismatch);
                assert_eq!(
                    profile.add_plain(&encrypted[i], &plaintexts[j]).err(),
                    mismatch
                );
                assert_eq!(
                    profile.mul_plain(&encrypted[i], &plaintexts[j]).err(),
                    mismatch
                );
                assert_eq!(
                    profile
                        .plan_add(&encrypted[i].shape(), &encrypted[j].shape())
                        .err(),
                    mismatch
                );
                let encoding = SlotEncoding::new(&profile, encrypted[i].plaintext_precision())
                    .expect("encoding");
                assert_eq!(encoding.decode(&plaintexts[j]).err(), mismatch);
            }
        }
        let foreign = fixture_profile(7, 1);
        assert_eq!(
            foreign.plan_galois(&encrypted[0].shape()).err(),
            Some(BgvError::ProfileMismatch)
        );
    }
}
