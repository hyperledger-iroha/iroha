//! The RP57 Poseidon permutation and sponge against the vendored stack.
//!
//! `iroha_pasta::poseidon` replaces the vendored sponge: `snark-verifier`'s
//! native `Poseidon<F, F, 3, 2>` built from halo2-base's
//! `OptimizedPoseidonSpec::<F, 3, 2>::new::<8, 57, 0>()`, which the KAGEMUSHA
//! domain hash and the confidential V3 hash run on. On both fields this module
//! checks:
//!
//! - the pinned round constants and MDS matrix against halo2-base's
//!   `unoptimized_constants::<8, 57, 0>()`, the in-crate Grain regeneration and
//!   the `poseidon_constants` section of `kats_v1.json`;
//! - fresh-sponge hashes of every input length up to 40 (and 63..=65, 100)
//!   against `snark-verifier`, and long update/squeeze sequences with carried
//!   state (transcript use), including `clear`;
//! - `hash_with_domain` against every `kagemusha_v1_poseidon` and
//!   `confidential_v3_poseidon` vector of the fixture, the 256-level empty
//!   replay root and the depth-16 empty subtree roots;
//! - the challenges of every recorded KAGEMUSHA Poseidon transcript script,
//!   replayed through the native sponge from the recorded absorbed elements.

use halo2_axiom::halo2curves::ff::{Field, FromUniformBytes, PrimeField};
use halo2_base::poseidon::hasher::spec::OptimizedPoseidonSpec;
use iroha_pasta::poseidon::{
    FULL_ROUNDS, PARTIAL_ROUNDS, PoseidonField, PoseidonParams, RATE, SECURE_MDS, Sponge, WIDTH,
    hash, hash_with_domain,
};
use iroha_plonk_oracle::{
    convert::{CurveBridge, NativeScalar, Pallas, Vesta, native_scalar, native_scalars},
    pools::same_on_each_pool,
};
use norito::json::Value;
use rand_core::RngCore;
use snark_verifier::{loader::native::NativeLoader, util::hash::Poseidon};

use crate::{array_at, data_rng, field_from_hex, fixture_at, str_at};

/// The vendored native sponge.
type VendoredSponge<F> = Poseidon<F, F, WIDTH, RATE>;

/// A fresh vendored sponge, built exactly as the domain hashes build it.
fn vendored_sponge<F: PrimeField + FromUniformBytes<64> + Ord>() -> VendoredSponge<F> {
    VendoredSponge::<F>::from_spec(
        &NativeLoader,
        OptimizedPoseidonSpec::<F, WIDTH, RATE>::new::<FULL_ROUNDS, PARTIAL_ROUNDS, SECURE_MDS>(),
    )
}

/// The fixture key of the scalar field of `B` (`"fp"` or `"fq"`).
fn field_key<B: CurveBridge>() -> &'static str {
    match B::NAME {
        "eq" => "fp",
        _ => "fq",
    }
}

/// Canonical encodings of a row of field elements.
fn row_reprs<F: PrimeField<Repr = [u8; 32]>>(row: &[F]) -> Vec<[u8; 32]> {
    row.iter().map(PrimeField::to_repr).collect()
}

/// The pinned tables against halo2-base, Grain and the fixture.
fn tables<B: CurveBridge>()
where
    B::VScalar: FromUniformBytes<64>,
    NativeScalar<B>: PoseidonField,
{
    let label = B::NAME;
    let pinned = NativeScalar::<B>::rp57();
    let (round_constants, mds) =
        OptimizedPoseidonSpec::<B::VScalar, WIDTH, RATE>::unoptimized_constants::<
            FULL_ROUNDS,
            PARTIAL_ROUNDS,
            SECURE_MDS,
        >();
    assert_eq!(
        round_constants.len(),
        pinned.round_constants().len(),
        "{label}: rounds"
    );
    for (round, (vendored, native)) in round_constants
        .iter()
        .zip(pinned.round_constants())
        .enumerate()
    {
        assert_eq!(
            row_reprs(vendored),
            row_reprs(native),
            "{label}: round constants {round}"
        );
    }
    for (row, (vendored, native)) in mds.iter().zip(pinned.mds()).enumerate() {
        assert_eq!(
            row_reprs(vendored),
            row_reprs(native),
            "{label}: MDS row {row}"
        );
    }
    assert_eq!(
        &PoseidonParams::<NativeScalar<B>>::generate(),
        pinned,
        "{label}: Grain regeneration"
    );
    let constants = fixture_at("poseidon_constants")
        .get(field_key::<B>())
        .expect("field");
    let parse = |rows: &[Value]| -> Vec<Vec<String>> {
        rows.iter()
            .map(|row| {
                row.as_array()
                    .expect("row")
                    .iter()
                    .map(|v| v.as_str().expect("hex").to_owned())
                    .collect()
            })
            .collect()
    };
    let hex_rows = |rows: &[[NativeScalar<B>; WIDTH]]| -> Vec<Vec<String>> {
        rows.iter()
            .map(|row| row.iter().map(|v| crate::hex(&v.to_repr())).collect())
            .collect()
    };
    assert_eq!(
        parse(array_at(constants, "round_constants")),
        hex_rows(&pinned.round_constants()[..]),
        "{label}: fixture round constants"
    );
    assert_eq!(
        parse(array_at(constants, "mds")),
        hex_rows(&pinned.mds()[..]),
        "{label}: fixture MDS"
    );
}

#[test]
fn tables_match_halo2_base_grain_and_the_fixture() {
    tables::<Vesta>();
    tables::<Pallas>();
}

/// Fresh hashes and long stateful sequences against `snark-verifier`.
fn sponge_parity<B: CurveBridge>()
where
    B::VScalar: FromUniformBytes<64>,
    NativeScalar<B>: PoseidonField,
{
    let label = B::NAME;
    same_on_each_pool(label, |threads| {
        let mut rng = data_rng(&format!("poseidon sponge {label}"));
        let mut outputs = Vec::new();
        // Building the vendored spec is slow in debug builds, so one sponge is
        // reused through `clear`, which `native_prover_kats` shows equals a
        // fresh sponge.
        let mut vendored = vendored_sponge::<B::VScalar>();
        let lengths = (0..=40).chain([63, 64, 65, 100]);
        for length in lengths {
            let inputs: Vec<B::VScalar> =
                (0..length).map(|_| B::VScalar::random(&mut rng)).collect();
            vendored.clear();
            vendored.update(&inputs);
            let expected = vendored.squeeze();
            let native = hash(&native_scalars::<B>(&inputs));
            assert_eq!(
                native,
                native_scalar::<B>(&expected),
                "{label}: hash of {length} elements at {threads} threads"
            );
            outputs.push(native);
        }
        // Transcript-style use: the state carries across squeezes.
        vendored.clear();
        let mut native = Sponge::<NativeScalar<B>>::new();
        for step in 0..200 {
            if step == 120 {
                vendored.clear();
                native.clear();
            }
            let length = usize::try_from(rng.next_u32() % 6).expect("small");
            let inputs: Vec<B::VScalar> =
                (0..length).map(|_| B::VScalar::random(&mut rng)).collect();
            vendored.update(&inputs);
            native.update(&native_scalars::<B>(&inputs));
            if !rng.next_u32().is_multiple_of(3) {
                let squeezed = native.squeeze();
                assert_eq!(
                    squeezed,
                    native_scalar::<B>(&vendored.squeeze()),
                    "{label}: stateful squeeze at step {step}"
                );
                outputs.push(squeezed);
            }
        }
        outputs
    });
}

#[test]
fn sponge_matches_snark_verifier() {
    sponge_parity::<Vesta>();
    sponge_parity::<Pallas>();
}

/// The domain word of an 8-byte ASCII tag.
fn domain_word(tag: &str) -> u64 {
    let bytes: [u8; 8] = tag.as_bytes().try_into().expect("8-byte domain tag");
    u64::from_le_bytes(bytes)
}

/// `hash_with_domain` against one fixture vector list.
fn check_vectors<B: CurveBridge>(section: &str, vectors: &[Value])
where
    NativeScalar<B>: PoseidonField,
{
    assert!(!vectors.is_empty(), "{section}: vectors");
    for (index, vector) in vectors.iter().enumerate() {
        let domain = str_at(vector, "domain");
        let inputs: Vec<NativeScalar<B>> = array_at(vector, "inputs")
            .iter()
            .map(|v| field_from_hex(v.as_str().expect("hex")))
            .collect();
        let expected: NativeScalar<B> = field_from_hex(str_at(vector, "output"));
        assert_eq!(
            hash_with_domain(domain_word(domain), &inputs),
            expected,
            "{section}[{index}] ({domain}, {} inputs)",
            inputs.len()
        );
    }
}

/// Every native hash vector of the fixture for the scalar field of `B`.
fn fixture_vectors<B: CurveBridge>()
where
    NativeScalar<B>: PoseidonField,
{
    let key = field_key::<B>();
    same_on_each_pool(key, |_| {
        let kagemusha = fixture_at("kagemusha_v1_poseidon").get(key).expect("field");
        check_vectors::<B>(
            &format!("kagemusha_v1_poseidon.{key}"),
            array_at(kagemusha, "vectors"),
        );
        let mut root = hash_with_domain::<NativeScalar<B>>(domain_word("kgmemp_1"), &[]);
        for _ in 0..256 {
            root = hash_with_domain(domain_word("kgmnode1"), &[root, root]);
        }
        let expected: NativeScalar<B> = field_from_hex(str_at(kagemusha, "empty_replay_root"));
        assert_eq!(
            root, expected,
            "kagemusha_v1_poseidon.{key}: empty replay root"
        );

        let confidential = fixture_at("confidential_v3_poseidon")
            .get(key)
            .expect("field");
        check_vectors::<B>(
            &format!("confidential_v3_poseidon.{key}"),
            array_at(confidential, "vectors"),
        );
        let mut root = hash_with_domain(domain_word("cfleaf03"), &[NativeScalar::<B>::ZERO]);
        for (level, expected) in array_at(confidential, "empty_subtree_roots")
            .iter()
            .enumerate()
        {
            let expected: NativeScalar<B> = field_from_hex(expected.as_str().expect("hex"));
            assert_eq!(
                root, expected,
                "confidential_v3_poseidon.{key}: empty root {level}"
            );
            root = hash_with_domain(domain_word("cfnode03"), &[root, root]);
        }
        root
    });
}

#[test]
fn domain_hashes_match_the_fixture_vectors() {
    fixture_vectors::<Vesta>();
    fixture_vectors::<Pallas>();
}

/// The recorded KAGEMUSHA Poseidon transcript challenges of curve `B`.
fn transcript_scripts<B: CurveBridge>()
where
    NativeScalar<B>: PoseidonField,
{
    let section = fixture_at("poseidon_transcript")
        .get(B::NAME)
        .expect("curve");
    let scripts = array_at(section, "scripts");
    assert_eq!(scripts.len(), 4, "{}: scripts", B::NAME);
    for script in scripts {
        let name = str_at(script, "name");
        let absorbed: Vec<NativeScalar<B>> = array_at(script, "absorbed")
            .iter()
            .map(|v| field_from_hex(v.as_str().expect("hex")))
            .collect();
        let mut sponge = Sponge::<NativeScalar<B>>::new();
        let mut consumed = 0;
        let mut squeezes = 0;
        for op in array_at(script, "ops") {
            if str_at(op, "op") != "squeeze" {
                continue;
            }
            let count = op
                .get("absorbed")
                .and_then(Value::as_u64)
                .and_then(|count| usize::try_from(count).ok())
                .expect("absorbed count");
            sponge.update(&absorbed[consumed..count]);
            consumed = count;
            let expected: NativeScalar<B> = field_from_hex(str_at(op, "challenge"));
            assert_eq!(
                sponge.squeeze(),
                expected,
                "{} {name}: squeeze {squeezes}",
                B::NAME
            );
            squeezes += 1;
        }
        assert!(squeezes > 0, "{name}: has squeezes");
    }
}

#[test]
fn transcript_challenges_replay_through_the_native_sponge() {
    transcript_scripts::<Vesta>();
    transcript_scripts::<Pallas>();
}

#[test]
fn domain_words_and_field_keys_are_canonical() {
    assert_eq!(domain_word("kgmnode1"), u64::from_le_bytes(*b"kgmnode1"));
    assert_eq!(field_key::<Vesta>(), "fp");
    assert_eq!(field_key::<Pallas>(), "fq");
    assert_eq!(
        (WIDTH, RATE, FULL_ROUNDS, PARTIAL_ROUNDS, SECURE_MDS),
        (3, 2, 8, 57, 0)
    );
}
