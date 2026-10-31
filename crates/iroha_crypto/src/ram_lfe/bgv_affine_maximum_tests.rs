//! Explicit full-size affine validation with deterministic synthetic fixtures.
//!
//! The ignored test exercises the current N16384/q6 candidate, not a reduced
//! arithmetic ring. It measures elapsed time and checks the exact centered
//! inner-product norm by independent arbitrary-integer CRT. It prints only
//! public geometry and elapsed milliseconds, never secret-derived values.
//! This is neither a resource ceiling nor parameter/privacy qualification.
//! BigInt oracle temporaries are test-only synthetic data, not clearing custody
//! owners; the production key and fixture residue buffers retain their owners.

use super::*;
use crate::ram_lfe::{
    HiddenRamFheInstruction as Op, HiddenRamFheProgram, RamLfeReferenceInputV1,
    ram_lfe_reference_execute_v1, ram_lfe_reference_normalize_v1,
};
use iroha_fhe::bgv::{
    BgvCandidate, KeySet, Plaintext,
    fixtures::{self, FixtureEntropy},
};
use num_bigint::{BigInt, Sign};
use std::time::Instant;
use zeroize::Zeroizing;

fn maximum_tape(operations: Vec<Op>) -> HiddenRamFheProgram {
    assert_eq!(operations.len(), 256, "public fixture instruction budget");
    let mut builder = HiddenRamFheProgram::builder().expect("fixture builder");
    for operation in operations {
        builder
            .push(operation)
            .expect("admitted fixture instruction");
    }
    builder.finish().expect("maximum fixture tape")
}

/// Read the fixture inner product but reconstruct every coefficient independently.
/// No reconstructed coefficient, exact norm, key word or plaintext is printed.
fn assert_exact_physical_noise(
    profile: &BgvProfile,
    keys: &KeySet,
    ciphertext: &Ciphertext,
    expected: &Plaintext,
) {
    let primes = &profile.parameters().main_primes[..=ciphertext.level()];
    let modulus = primes.iter().fold(BigInt::from(1), |q, &prime| q * prime);
    let capacity = (&modulus - 1_u32) / 2_u32;
    let basis: Vec<BigInt> = primes
        .iter()
        .map(|&prime| {
            let partial = &modulus / prime;
            // Fermat inversion of public prime-field data; no shared RNS CRT code.
            let inverse = partial.modpow(&BigInt::from(prime - 2), &BigInt::from(prime));
            partial * inverse
        })
        .collect();
    let declared = ciphertext
        .noise_bound()
        .as_natural()
        .words_le()
        .iter()
        .rev()
        .fold(BigInt::from(0), |value, &word| (value << 64) + word);
    assert!(declared <= capacity, "public plan exceeds capacity");
    let words = Zeroizing::new(
        fixtures::inner_product_words(profile, &keys.secret, ciphertext)
            .expect("fixture decryption inner product"),
    );
    let degree = profile.degree();
    assert_eq!(
        words.len(),
        degree * primes.len(),
        "public residue geometry"
    );
    let plaintext_modulus = BigInt::from(profile.master_plaintext_modulus());
    let mut exact_norm = BigInt::from(0);
    for coefficient in 0..degree {
        let canonical = basis
            .iter()
            .enumerate()
            .fold(BigInt::from(0), |sum, (limb, weight)| {
                sum + weight * words[limb * degree + coefficient]
            })
            % &modulus;
        let centered = if canonical > capacity {
            canonical - &modulus
        } else {
            canonical
        };
        let magnitude = if centered.sign() == Sign::Minus {
            -&centered
        } else {
            centered.clone()
        };
        if magnitude > exact_norm {
            exact_norm = magnitude;
        }
        let residue = ((&centered % &plaintext_modulus) + &plaintext_modulus) % &plaintext_modulus;
        assert!(
            residue == BigInt::from(expected.coefficients()[coefficient]),
            "independent centered CRT disagrees with expected plaintext"
        );
    }
    assert!(
        exact_norm <= declared,
        "measured norm exceeds declared bound"
    );
    assert!(
        exact_norm <= capacity,
        "measured norm exceeds decryption capacity"
    );
}

#[test]
#[ignore = "full-size N16384 q6 keys and two 22-Galois shared-routing evaluations; run explicitly"]
fn current_n14_q6_maximum_affine_tapes_match_scalar_reference_and_physical_noise() {
    let started = Instant::now();
    let profile = BgvProfile::candidate(BgvCandidate::AffineF257).expect("current candidate");
    assert_eq!(profile.degree(), 1 << 14);
    assert_eq!(profile.top_level() + 1, 6);
    assert_eq!(profile.special_count(), 2);
    let plan = RamLfeBgvAffinePlanV1::new(&profile, profile.plaintext_precision(1).unwrap())
        .expect("complete affine noise plan");
    let encoding = SlotEncoding::new(&profile, profile.master_plaintext()).expect("F257 encoding");
    assert_eq!(plan.galois_applications(), 22);
    assert_eq!(plan.routing.plaintext_products() + 2, 130);
    let setup_ms = started.elapsed().as_millis();
    let key_started = Instant::now();
    let keys = fixtures::key_set(
        &profile,
        plan.galois_elements(),
        &mut FixtureEntropy::new([117; 64]),
    )
    .expect("genuine complete candidate key set");
    eprintln!(
        "affine_maximum n={} main_limbs={} special_limbs={} slots={} rows={} galois={} coefficient_words={} setup_ms={} keygen_ms={}",
        profile.degree(),
        profile.top_level() + 1,
        profile.special_count(),
        encoding.slots(),
        RAM_LFE_V1_MAX_OUTPUTS,
        plan.galois_applications(),
        WORDS,
        setup_ms,
        key_started.elapsed().as_millis(),
    );
    let bytes = std::array::from_fn::<_, 63, _>(|i| ((i * 43 + 7) % 256) as u8);
    let input = RamLfeReferenceInputV1::from_bytes(&bytes).expect("full input");
    let memory = RamLfeInitialMemoryV1::from_lanes(&std::array::from_fn::<_, 32, _>(|i| {
        ((i * 19 + 197) % 257) as u16
    }))
    .expect("fixture lanes");
    let mut input_slots = Zeroizing::new([0_u64; SLOTS]);
    for (to, &from) in input_slots.iter_mut().zip(input.slots()) {
        *to = u64::from(from);
    }
    let encrypt_started = Instant::now();
    let cipher = fixtures::encrypt(
        &profile,
        &keys.public,
        &encoding
            .encode(&profile, &*input_slots)
            .expect("encoded input"),
        &mut FixtureEntropy::new([119; 64]),
    )
    .expect("genuine input encryption");
    eprintln!(
        "affine_maximum encrypt_ms={}",
        encrypt_started.elapsed().as_millis()
    );

    for case in 0..2 {
        let mut operations = Vec::with_capacity(256);
        let mut expected = Zeroizing::new([0_u64; SLOTS]);
        if case == 0 {
            // 1 + 64*3 + 31*2 + 1 =256 instructions: every input is part of
            // one dense row;31 initialized lanes and constant256 contribute.
            operations.push(Op::LoadConst(0, 256));
            let mut scalar = 256_u64;
            for slot in 0..64_u16 {
                let coefficient = (u64::from(slot) * 37 + 256) % 257;
                operations.extend([
                    Op::LoadInput(1, slot),
                    Op::MulPlain(1, 1, coefficient),
                    Op::Add(0, 0, 1),
                ]);
                scalar = (scalar + coefficient * input_slots[usize::from(slot)]) % 257;
            }
            for lane in 0..31_u16 {
                operations.extend([Op::LoadState(2, lane), Op::Add(0, 0, 2)]);
                scalar = (scalar + u64::from(memory.lanes()[usize::from(lane)])) % 257;
            }
            operations.push(Op::Output(0));
            expected[0] = scalar;
        } else {
            // Exactly64 independently expected ordered outputs in256 instructions.
            for slot in 0..64_u16 {
                let bias = (u64::from(slot) * 19 + 256) % 257;
                let scale = (u64::from(slot) * 11 + 128) % 257;
                operations.extend([
                    Op::LoadInput(0, slot),
                    Op::AddPlain(0, 0, bias),
                    Op::MulPlain(0, 0, scale),
                    Op::Output(0),
                ]);
                expected[usize::from(slot)] = (input_slots[usize::from(slot)] + bias) * scale % 257;
            }
        }
        let program = maximum_tape(operations);
        let reference =
            ram_lfe_reference_execute_v1(RamLfeClassV1::Affine, &program, &memory, &input)
                .expect("canonical scalar reference");
        let reference_output = reference.output().padded_for_opening();
        assert!(
            reference_output
                .scalars()
                .iter()
                .zip(expected.iter())
                .all(|(&a, &b)| u64::from(a) == b),
            "independent scalar expected differs from canonical reference"
        );
        assert_eq!(
            reference.output().scalars().len(),
            if case == 0 { 1 } else { 64 }
        );
        let normal = ram_lfe_reference_normalize_v1(RamLfeClassV1::Affine, &program)
            .expect("canonical normalization");
        let prepared =
            RamLfeBgvAffineProgramV1::from_normal_form(&normal, &plan).expect("private rows");
        let evaluate_started = Instant::now();
        let result = prepared
            .evaluate(&profile, &plan, &memory, &cipher, &keys.evaluation)
            .expect("full padded affine evaluation");
        let evaluate_ms = evaluate_started.elapsed().as_millis();
        assert_eq!(&result.shape(), plan.output_shape(), "public output shape");
        let check_started = Instant::now();
        let opened = keys.secret.decrypt(&profile, &result).expect("decrypt");
        let actual = encoding.decode(&opened).expect("scalar-subalgebra output");
        assert!(
            actual.as_slice() == expected.as_slice(),
            "decrypted padded slots disagree"
        );
        let expected_plain = encoding
            .encode(&profile, &*expected)
            .expect("expected polynomial");
        assert_exact_physical_noise(&profile, &keys, &result, &expected_plain);
        eprintln!(
            "affine_maximum fixture={} ciphertext_words={} evaluate_ms={} validation_ms={}",
            case,
            result.component_count() * (result.level() + 1) * profile.degree(),
            evaluate_ms,
            check_started.elapsed().as_millis(),
        );
    }
    eprintln!("affine_maximum total_ms={}", started.elapsed().as_millis());
}
