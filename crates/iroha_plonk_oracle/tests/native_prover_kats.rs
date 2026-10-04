//! Known-answer vectors recorded from the vendored halo2 stack for the native prover.
//!
//! `fixtures/native_prover/kats_v1.json` pins the vendored behaviour that the
//! native crates (`iroha_pasta`, `iroha_plonk`, `iroha_plonk_gadgets`) must
//! reproduce before `vendor/` and the git dependencies are deleted. Sections:
//!
//! - `oracle_baseline`: the commits and toolchain the vectors were recorded from.
//! - `golden_proofs`: the proof SHA-256 tables of
//!   `vendor/halo2-axiom/tests/golden_proof_bytes.rs` (Blake2b transcript,
//!   compiled in) and `crates/iroha_core_zk/src/prover_golden_tests.rs`
//!   (KAGEMUSHA Poseidon transcript, read at run time; see
//!   [`KAGEMUSHA_GOLDEN_SOURCE`]).
//! - `params_ipa`: length and SHA-256 of `ParamsIPA::new(k)` written bytes for
//!   k = 6..=16 on both curves, with digests of the generator and Lagrange
//!   sections.
//! - `generators`: the first 64 `Halo2-Parameters` generators and `w`, `u`.
//! - `blake2b_transcript`: `Blake2bWrite`/`Blake2bRead` with `Challenge255`
//!   (personalization `Halo2-Transcript`), and decoding rejections.
//! - `poseidon_transcript`: snark-verifier
//!   `PoseidonTranscript<C, NativeLoader, _, 3, 2, 8, 57>::new::<0>`, the
//!   KAGEMUSHA transcript of `iroha_core_zk`, and decoding rejections.
//! - `poseidon_constants`: the unoptimized round constants and MDS matrix of
//!   halo2-base `OptimizedPoseidonSpec<F, 3, 2>` with 8 full and 57 partial
//!   rounds, for Fp and Fq.
//! - `kagemusha_v1_poseidon`: a reproduction of
//!   `iroha_core_zk::kagemusha_v1_poseidon::hash` (see below).
//! - `confidential_v3_poseidon`: a reproduction of
//!   `iroha_core_zk::confidential_v2::confidential_poseidon_hash_v3`.
//!
//! Field elements are canonical little-endian `to_repr` hex; points are
//! compressed `to_bytes` hex. The file is `norito::json` pretty output with
//! canonically ordered keys; the test rejects any other formatting.
//!
//! Both native hashes are reproduced here with the exact halo2-base and
//! snark-verifier types those modules use: they are `pub(crate)`, and
//! `iroha_core_zk` will take this crate as a dev-dependency, so calling them
//! would create a dependency cycle. Production anchors differ per section and
//! are recorded in the fixture (`production_anchors`):
//!
//! - the shared sponge and the confidential domains are anchored to the three
//!   KATs pinned in `crates/iroha_core_zk/src/confidential_v2_tests.rs`
//!   (`confidential_reproduction_matches_iroha_core_zk_kats`);
//! - no `iroha_core_zk` test pins a KAGEMUSHA hash value yet, so the
//!   `kagemusha_v1_poseidon` vectors are anchored only through that shared
//!   sponge. TODO (`iroha_core_zk` owner): add a `#[cfg(test)]` test in
//!   `kagemusha_v1_poseidon.rs` (and one for `confidential_poseidon_hash_v3`)
//!   that parses `fixtures/native_prover/kats_v1.json` with `norito::json` and
//!   asserts every vector and the empty depth-256 replay root on both fields.
//!
//! By default the test regenerates every vector and compares it with the
//! fixture, reporting the JSON path of the first difference. The k = 15 and
//! k = 16 params are checked by an ignored release test (the vendored
//! generator is unoptimised in the test profile). With
//! `IROHA_UPDATE_NATIVE_PROVER_KATS=1` the test rewrites the fixture instead,
//! every k included (run that in release). A changed vector means the vendored
//! behaviour changed: review it rather than regenerate it.

use std::{
    fmt::Write as _,
    io::{self, Read},
    path::PathBuf,
};

use halo2_axiom::{
    arithmetic::{CurveAffine, CurveExt},
    halo2curves::{
        Coordinates,
        ff::{Field, FromUniformBytes, PrimeField},
        group::{Curve, prime::PrimeCurveAffine},
        pasta::{EpAffine, EqAffine, Fp, Fq},
    },
    poly::{
        commitment::{Params, ParamsProver},
        ipa::commitment::ParamsIPA,
    },
    transcript::{
        Blake2bRead, Blake2bWrite, Challenge255, EncodedChallenge, TranscriptRead,
        TranscriptReadBuffer, TranscriptWrite, TranscriptWriterBuffer,
    },
};
use halo2_base::poseidon::hasher::spec::OptimizedPoseidonSpec;
use norito::json::{Map, Value};
use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;
use sha2::{Digest, Sha256};
use snark_verifier::{
    loader::native::NativeLoader,
    system::halo2::transcript::halo2::{ChallengeScalar, PoseidonTranscript},
    util::{
        arithmetic::{FieldExt, fe_to_fe},
        hash::Poseidon,
    },
};

/// Fixture path relative to this crate.
const FIXTURE: &str = "../../fixtures/native_prover/kats_v1.json";
/// Set to `1` to rewrite the fixture instead of comparing with it.
const UPDATE_ENV: &str = "IROHA_UPDATE_NATIVE_PROVER_KATS";
/// Format tag of the fixture.
const FORMAT: &str = "iroha.native_prover.kats.v1";

/// Where and from what the vectors were recorded.
///
/// `repository_head` is `git rev-parse HEAD` and the `*_last_commit` values are
/// `git log -1 -- <path>` at recording time. The vectors are valid for the
/// vendored stack at those commits; `vendor/` takes bug fixes only from M0.
const ORACLE_BASELINE: &[(&str, &str)] = &[
    (
        "repository_head",
        "1de7210a74d62ae5232c67b910dbf4b6b1bcf757",
    ),
    (
        "vendor_halo2_axiom_last_commit",
        "8f41274044c93ad7e363fdc8be30a671efb436d4",
    ),
    (
        "vendor_halo2_axiom_golden_proof_bytes_last_commit",
        "f5439174c4df53b48f5e888e132c1f6ec2e2524f",
    ),
    (
        "vendor_halo2curves_axiom_last_commit",
        "48dcbb6683c0763f3f4f47915f969a0b96d484da",
    ),
    (
        "vendor_halo2_base_last_commit",
        "48dcbb6683c0763f3f4f47915f969a0b96d484da",
    ),
    ("halo2_axiom_version", "0.5.1 (vendor/halo2-axiom)"),
    (
        "halo2curves_axiom_version",
        "0.7.0 (vendor/halo2curves-axiom)",
    ),
    ("pasta_curves_version", "0.5.2"),
    ("blake2b_simd_version", "1.0.4"),
    (
        "halo2_lib_tag",
        "v0.5.3 (halo2-base from vendor/halo2-base)",
    ),
    (
        "snark_verifier_rev",
        "bbfcc721d714bea0d44a27c8fc6c4736e73ca853",
    ),
    ("rust_toolchain", "1.93.1"),
    ("recorded_on", "aarch64-apple-darwin"),
    ("recorded_date", "2026-10-04"),
];

/// `ParamsIPA` sizes recorded by the default test.
const PARAMS_K_DEBUG: std::ops::RangeInclusive<u32> = 6..=14;
/// `ParamsIPA` sizes recorded only by the ignored release test (k >= 15 is
/// release-only in every suite).
const PARAMS_K_RELEASE: std::ops::RangeInclusive<u32> = 15..=16;
/// The KAGEMUSHA golden table, relative to this crate.
///
/// Read at run time rather than through `include_str!`, so that renaming or
/// deleting that private test file fails only `native_prover_kats_match_fixture`
/// (with this path in the message) instead of the whole test target.
/// TODO (`iroha_core_zk` owner): `prover_golden_tests.rs` should read its
/// `GOLDEN_SHA256` table from `golden_proofs.iroha_core_zk_kagemusha` in
/// `kats_v1.json` (fixture changes select the full CI run); this crate then
/// stops reading another crate's source. Until then a change to the table runs
/// this test only in a full CI run.
const KAGEMUSHA_GOLDEN_SOURCE: &str = "../iroha_core_zk/src/prover_golden_tests.rs";
/// Generators listed explicitly in the `generators` section.
const LISTED_GENERATORS: usize = 64;
/// Domain of the `ParamsIPA` hash-to-curve generators.
const PARAMS_DOMAIN: &str = "Halo2-Parameters";

/// Width of the KAGEMUSHA and confidential Poseidon permutation.
const POSEIDON_T: usize = 3;
/// Sponge rate of the KAGEMUSHA and confidential Poseidon permutation.
const POSEIDON_RATE: usize = 2;
/// Full rounds of the KAGEMUSHA and confidential Poseidon permutation.
const POSEIDON_FULL_ROUNDS: usize = 8;
/// Partial rounds of the KAGEMUSHA and confidential Poseidon permutation.
const POSEIDON_PARTIAL_ROUNDS: usize = 57;
/// Secure-MDS selector of the KAGEMUSHA and confidential Poseidon permutation.
const POSEIDON_SECURE_MDS: usize = 0;

/// The KAGEMUSHA proof transcript (`iroha_core_zk` `KAGEMUSHA_IPA_POSEIDON_*_V1`).
type KagemushaTranscript<C, S> = PoseidonTranscript<
    C,
    NativeLoader,
    S,
    POSEIDON_T,
    POSEIDON_RATE,
    POSEIDON_FULL_ROUNDS,
    POSEIDON_PARTIAL_ROUNDS,
>;
/// The native sponge behind `kagemusha_v1_poseidon` and `confidential_v2`.
type NativePoseidon<F> = Poseidon<F, F, POSEIDON_T, POSEIDON_RATE>;
/// The halo2-base specification behind [`NativePoseidon`].
type PoseidonSpec<F> = OptimizedPoseidonSpec<F, POSEIDON_T, POSEIDON_RATE>;

/// KAGEMUSHA domains (`iroha_core_zk::kagemusha_v1_poseidon`).
const KAGEMUSHA_DOMAINS: [[u8; 8]; 4] = [*b"kgmemp_1", *b"kgmleaf1", *b"kgmnode1", *b"kgmstate"];
/// Confidential V3 domains (`iroha_core_zk::confidential_v2`).
const CONFIDENTIAL_DOMAINS: [[u8; 8]; 7] = [
    *b"cfownr03",
    *b"cfnote03",
    *b"cfnull03",
    *b"cfleaf03",
    *b"cfnode03",
    *b"cfasst03",
    *b"cfnet_03",
];
/// Depth of the confidential V3 note tree (`CONFIDENTIAL_TREE_DEPTH_V2`).
const CONFIDENTIAL_TREE_DEPTH: usize = 16;
/// Depth of the KAGEMUSHA consumed-credit tree.
const KAGEMUSHA_REPLAY_DEPTH: usize = 256;

// ---------------------------------------------------------------------------
// JSON (norito::json; object keys are ordered canonically).

/// A string value.
fn s(value: impl Into<String>) -> Value {
    Value::String(value.into())
}

/// An integer value from any unsigned size.
fn int(value: usize) -> Value {
    Value::from(u64::try_from(value).expect("size fits u64"))
}

/// An object from `(key, value)` pairs; keys must be unique.
fn obj(entries: Vec<(&str, Value)>) -> Value {
    let mut map = Map::new();
    for (key, value) in entries {
        assert!(
            map.insert(key.to_owned(), value).is_none(),
            "duplicate key {key}"
        );
    }
    Value::Object(map)
}

/// The fixture text of `value`: Norito's pretty JSON and a trailing newline.
fn render(value: &Value) -> String {
    let mut text = norito::json::to_string_pretty(value).expect("render JSON");
    text.push('\n');
    text
}

/// Compact JSON of `value`, for diagnostics.
fn compact(value: &Value) -> String {
    norito::json::to_string(value).expect("render JSON")
}

/// The JSON path and values of the first difference between two documents.
fn first_difference(path: &str, expected: &Value, actual: &Value) -> Option<String> {
    match (expected, actual) {
        (Value::Object(left), Value::Object(right)) => {
            let left_keys = left.keys().collect::<Vec<_>>();
            let right_keys = right.keys().collect::<Vec<_>>();
            if left_keys != right_keys {
                return Some(format!(
                    "{path}: keys differ: expected {left_keys:?}, fixture has {right_keys:?}"
                ));
            }
            left.iter()
                .zip(right.values())
                .find_map(|((key, left), right)| {
                    first_difference(&format!("{path}.{key}"), left, right)
                })
        }
        (Value::Array(left), Value::Array(right)) => {
            if left.len() != right.len() {
                return Some(format!(
                    "{path}: expected {} items, fixture has {}",
                    left.len(),
                    right.len()
                ));
            }
            left.iter()
                .zip(right)
                .enumerate()
                .find_map(|(index, (left, right))| {
                    first_difference(&format!("{path}[{index}]"), left, right)
                })
        }
        _ => (expected != actual).then(|| {
            format!(
                "{path}: expected {}, fixture has {}",
                compact(expected),
                compact(actual)
            )
        }),
    }
}

// ---------------------------------------------------------------------------
// Encodings.

/// Lowercase hex of `bytes`.
fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut out, byte| {
        write!(out, "{byte:02x}").expect("write to String");
        out
    })
}

/// Hex SHA-256 of `bytes`.
fn sha256_hex(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

/// Canonical little-endian hex of a field element.
fn fe_hex<F: PrimeField>(value: &F) -> String {
    hex(value.to_repr().as_ref())
}

/// Compressed hex of an affine point.
fn point_hex<C: CurveAffine>(point: &C) -> String {
    hex(point.to_bytes().as_ref())
}

/// Hex of a domain word: the eight ASCII bytes read as a little-endian `u64`.
fn domain_word(tag: [u8; 8]) -> u64 {
    u64::from_le_bytes(tag)
}

/// Little-endian bytes of the modulus of `F` (one more than `-1`).
fn modulus_bytes<F: PrimeField>() -> Vec<u8> {
    let mut bytes = (-F::ONE).to_repr().as_ref().to_vec();
    for byte in &mut bytes {
        let (sum, carry) = byte.overflowing_add(1);
        *byte = sum;
        if !carry {
            break;
        }
    }
    bytes
}

/// Deterministic value stream for vector inputs (never prover randomness).
fn data_rng(label: &str) -> ChaCha20Rng {
    ChaCha20Rng::from_seed(Sha256::digest(label.as_bytes()).into())
}

/// `(case, sha256)` pairs of a `GOLDEN_SHA256` table in a Rust test source.
fn golden_table(source: &str) -> Vec<(String, String)> {
    const HEAD: &str = "const GOLDEN_SHA256: &[(&str, &str)] = &[";
    let start = source.find(HEAD).expect("source declares GOLDEN_SHA256") + HEAD.len();
    let length = source[start..]
        .find("\n];")
        .expect("GOLDEN_SHA256 table is closed");
    let literals = source[start..start + length]
        .split('"')
        .skip(1)
        .step_by(2)
        .collect::<Vec<_>>();
    assert!(
        !literals.is_empty() && literals.len() % 2 == 0,
        "GOLDEN_SHA256 holds (case, digest) pairs"
    );
    literals
        .chunks_exact(2)
        .map(|pair| {
            assert!(
                pair[1].len() == 64 && pair[1].bytes().all(|byte| byte.is_ascii_hexdigit()),
                "{}: SHA-256 hex",
                pair[0]
            );
            (pair[0].to_owned(), pair[1].to_owned())
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Sections.

/// The `oracle_baseline` section.
fn oracle_baseline() -> Value {
    obj(ORACLE_BASELINE
        .iter()
        .map(|(key, value)| (*key, s(*value)))
        .collect())
}

/// Text of the KAGEMUSHA golden source ([`KAGEMUSHA_GOLDEN_SOURCE`]).
fn kagemusha_golden_source() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(KAGEMUSHA_GOLDEN_SOURCE);
    std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "read {}: {error}; the KAGEMUSHA GOLDEN_SHA256 table moved, update \
             KAGEMUSHA_GOLDEN_SOURCE",
            path.display()
        )
    })
}

/// The `golden_proofs` section, read from the two golden test sources.
fn golden_proofs() -> Value {
    let table = |path: &str, transcript: &str, source: &str| {
        let cases = golden_table(source)
            .into_iter()
            .map(|(case, digest)| obj(vec![("case", s(case)), ("sha256", s(digest))]))
            .collect();
        obj(vec![
            ("source", s(path)),
            ("transcript", s(transcript)),
            ("cases", Value::Array(cases)),
        ])
    };
    obj(vec![
        (
            "vendored_halo2_axiom",
            table(
                "vendor/halo2-axiom/tests/golden_proof_bytes.rs",
                "Blake2bWrite<_, C, Challenge255<C>>; prover RNG ChaCha20Rng::from_seed([seed; 32])",
                include_str!("../../../vendor/halo2-axiom/tests/golden_proof_bytes.rs"),
            ),
        ),
        (
            "iroha_core_zk_kagemusha",
            table(
                "crates/iroha_core_zk/src/prover_golden_tests.rs",
                "KAGEMUSHA PoseidonTranscript<C, NativeLoader, _, 3, 2, 8, 57>; recovery seed [7; 32]; folded generator appended",
                &kagemusha_golden_source(),
            ),
        ),
    ])
}

/// One `params_ipa` entry: written length and digests of `ParamsIPA::<C>::new(k)`.
fn params_entry<C: CurveAffine>(k: u32) -> Value {
    let params = ParamsIPA::<C>::new(k);
    let mut bytes = Vec::new();
    params.write(&mut bytes).expect("params write to Vec");
    let n = 1_usize << k;
    let point = C::Repr::default().as_ref().len();
    let generators_end = 4 + n * point;
    let lagrange_end = generators_end + n * point;
    assert_eq!(bytes.len(), lagrange_end + 2 * point, "k{k}: params layout");
    assert_eq!(bytes[..4], k.to_le_bytes(), "k{k}: params header");
    assert_eq!(
        &bytes[lagrange_end..lagrange_end + point],
        params.get_blind_base().to_bytes().as_ref(),
        "k{k}: w follows the Lagrange section"
    );
    obj(vec![
        ("k", Value::from(u64::from(k))),
        ("byte_len", int(bytes.len())),
        ("sha256", s(sha256_hex(&bytes))),
        ("g_sha256", s(sha256_hex(&bytes[4..generators_end]))),
        (
            "g_lagrange_sha256",
            s(sha256_hex(&bytes[generators_end..lagrange_end])),
        ),
    ])
}

/// The `params_ipa` section for the sizes in `ks`.
fn params_ipa(ks: &std::ops::RangeInclusive<u32>) -> Value {
    let entries = |entry: fn(u32) -> Value| Value::Array(ks.clone().map(entry).collect());
    obj(vec![
        (
            "layout",
            s("u32_le(k) || g[0..2^k] || g_lagrange[0..2^k] || w || u, points compressed (32 B)"),
        ),
        (
            "section_digests",
            s("g_sha256 covers the g section, g_lagrange_sha256 the g_lagrange section"),
        ),
        ("eq", entries(params_entry::<EqAffine>)),
        ("ep", entries(params_entry::<EpAffine>)),
    ])
}

/// Generator, `w` and `u` points of one curve, cross-checked against hash-to-curve.
fn generators_for<C: CurveAffine>() -> Value {
    let params = ParamsIPA::<C>::new(6);
    let mut bytes = Vec::new();
    params.write(&mut bytes).expect("params write to Vec");
    let point = C::Repr::default().as_ref().len();
    let hasher = C::CurveExt::hash_to_curve(PARAMS_DOMAIN);
    let generators = params.get_g()[..LISTED_GENERATORS]
        .iter()
        .enumerate()
        .map(|(index, generator)| {
            let mut message = [0_u8; 5];
            message[1..].copy_from_slice(&u32::try_from(index).expect("index").to_le_bytes());
            assert_eq!(hasher(&message).to_affine(), *generator, "g[{index}]");
            s(point_hex(generator))
        })
        .collect();
    let tail = &bytes[bytes.len() - 2 * point..];
    let w = hasher(&[1]).to_affine();
    let u = hasher(&[2]).to_affine();
    assert_eq!(&tail[..point], w.to_bytes().as_ref(), "w");
    assert_eq!(&tail[point..], u.to_bytes().as_ref(), "u");
    obj(vec![
        ("g", Value::Array(generators)),
        ("w", s(point_hex(&w))),
        ("u", s(point_hex(&u))),
    ])
}

/// The `generators` section.
fn generators() -> Value {
    obj(vec![
        ("hash_to_curve_domain", s(PARAMS_DOMAIN)),
        ("g_message", s("[0x00] || u32_le(i)")),
        ("w_message", s("[0x01]")),
        ("u_message", s("[0x02]")),
        ("eq", generators_for::<EqAffine>()),
        ("ep", generators_for::<EpAffine>()),
    ])
}

/// One transcript operation of a recorded script.
#[derive(Clone, Copy)]
enum Op<C: CurveAffine> {
    /// `common_point`.
    CommonPoint(C),
    /// `common_scalar`.
    CommonScalar(C::Scalar),
    /// `write_point` (prover) or `read_point` (verifier).
    WritePoint(C),
    /// `write_scalar` (prover) or `read_scalar` (verifier).
    WriteScalar(C::Scalar),
    /// `squeeze_challenge`.
    Squeeze,
}

/// The transcript scripts recorded for both transcripts on curve `C`.
fn scripts<C: CurveAffine>(curve: &str) -> Vec<(&'static str, Vec<Op<C>>)> {
    let mut rng = data_rng(&format!("iroha native prover kats v1 transcript {curve}"));
    let mut scalar = || C::Scalar::random(&mut rng);
    let scalars: [C::Scalar; 8] = std::array::from_fn(|_| scalar());
    let mut rng = data_rng(&format!("iroha native prover kats v1 points {curve}"));
    let points: [C; 6] =
        std::array::from_fn(|_| (C::generator() * C::Scalar::random(&mut rng)).to_affine());
    let g = C::generator();
    vec![
        ("squeeze_only", vec![Op::Squeeze, Op::Squeeze, Op::Squeeze]),
        (
            "scalars",
            vec![
                Op::CommonScalar(C::Scalar::ZERO),
                Op::Squeeze,
                Op::CommonScalar(C::Scalar::ONE),
                Op::CommonScalar(-C::Scalar::ONE),
                Op::Squeeze,
                Op::CommonScalar(scalars[0]),
                Op::CommonScalar(scalars[1]),
                Op::CommonScalar(scalars[2]),
                Op::Squeeze,
                Op::Squeeze,
            ],
        ),
        (
            "points",
            vec![
                Op::CommonPoint(g),
                Op::Squeeze,
                Op::CommonPoint((g + g).to_affine()),
                Op::CommonPoint(-g),
                Op::Squeeze,
            ],
        ),
        (
            "proof_shaped",
            vec![
                Op::CommonScalar(scalars[3]),
                Op::WritePoint(points[0]),
                Op::WritePoint(points[1]),
                Op::Squeeze,
                Op::WritePoint(points[2]),
                Op::Squeeze,
                Op::Squeeze,
                Op::WritePoint(points[3]),
                Op::Squeeze,
                Op::WriteScalar(scalars[4]),
                Op::WriteScalar(scalars[5]),
                Op::WriteScalar(scalars[6]),
                Op::Squeeze,
                Op::WritePoint(points[4]),
                Op::WritePoint(points[5]),
                Op::WriteScalar(scalars[7]),
                Op::Squeeze,
            ],
        ),
    ]
}

/// Run `ops` on a prover transcript and return the challenges.
fn run_prover<C, E, T>(transcript: &mut T, ops: &[Op<C>]) -> Vec<C::Scalar>
where
    C: CurveAffine,
    E: EncodedChallenge<C>,
    T: TranscriptWrite<C, E>,
{
    let mut challenges = Vec::new();
    for op in ops {
        match *op {
            Op::CommonPoint(point) => transcript.common_point(point).expect("common_point"),
            Op::CommonScalar(value) => transcript.common_scalar(value).expect("common_scalar"),
            Op::WritePoint(point) => transcript.write_point(point).expect("write_point"),
            Op::WriteScalar(value) => transcript.write_scalar(value).expect("write_scalar"),
            Op::Squeeze => challenges.push(transcript.squeeze_challenge().get_scalar()),
        }
    }
    challenges
}

/// Replay `ops` on a verifier transcript and return the challenges.
fn run_verifier<C, E, T>(transcript: &mut T, ops: &[Op<C>]) -> Vec<C::Scalar>
where
    C: CurveAffine,
    E: EncodedChallenge<C>,
    T: TranscriptRead<C, E>,
{
    let mut challenges = Vec::new();
    for op in ops {
        match *op {
            Op::CommonPoint(point) => transcript.common_point(point).expect("common_point"),
            Op::CommonScalar(value) => transcript.common_scalar(value).expect("common_scalar"),
            Op::WritePoint(point) => {
                assert!(transcript.read_point().expect("read_point") == point);
            }
            Op::WriteScalar(value) => {
                assert!(transcript.read_scalar().expect("read_scalar") == value);
            }
            Op::Squeeze => challenges.push(transcript.squeeze_challenge().get_scalar()),
        }
    }
    challenges
}

/// Bytes the Blake2b transcript absorbs for `op`.
fn blake2b_absorbed<C: CurveAffine>(op: &Op<C>) -> Vec<u8> {
    let point_bytes = |point: &C| {
        let coordinates = Option::from(point.coordinates()).expect("finite point");
        let coordinates: Coordinates<C> = coordinates;
        let mut bytes = vec![1_u8];
        bytes.extend_from_slice(coordinates.x().to_repr().as_ref());
        bytes.extend_from_slice(coordinates.y().to_repr().as_ref());
        bytes
    };
    let scalar_bytes = |value: &C::Scalar| {
        let mut bytes = vec![2_u8];
        bytes.extend_from_slice(value.to_repr().as_ref());
        bytes
    };
    match op {
        Op::CommonPoint(point) | Op::WritePoint(point) => point_bytes(point),
        Op::CommonScalar(value) | Op::WriteScalar(value) => scalar_bytes(value),
        Op::Squeeze => vec![0],
    }
}

/// Field elements the Poseidon transcript absorbs for `op`.
fn poseidon_absorbed<C: CurveAffine>(op: &Op<C>) -> Vec<C::Scalar> {
    match op {
        Op::CommonPoint(point) | Op::WritePoint(point) => {
            let coordinates: Coordinates<C> =
                Option::from(point.coordinates()).expect("finite point");
            vec![fe_to_fe(*coordinates.x()), fe_to_fe(*coordinates.y())]
        }
        Op::CommonScalar(value) | Op::WriteScalar(value) => vec![*value],
        Op::Squeeze => Vec::new(),
    }
}

/// JSON of one recorded operation; `absorbed` is the running absorbed length.
fn op_json<C: CurveAffine>(op: &Op<C>, challenge: Option<&C::Scalar>, absorbed: usize) -> Value {
    match op {
        Op::CommonPoint(point) => obj(vec![
            ("op", s("common_point")),
            ("value", s(point_hex(point))),
        ]),
        Op::CommonScalar(value) => obj(vec![
            ("op", s("common_scalar")),
            ("value", s(fe_hex(value))),
        ]),
        Op::WritePoint(point) => obj(vec![
            ("op", s("write_point")),
            ("value", s(point_hex(point))),
        ]),
        Op::WriteScalar(value) => obj(vec![("op", s("write_scalar")), ("value", s(fe_hex(value)))]),
        Op::Squeeze => obj(vec![
            ("op", s("squeeze")),
            (
                "challenge",
                s(fe_hex(challenge.expect("squeeze has a challenge"))),
            ),
            ("absorbed", int(absorbed)),
        ]),
    }
}

/// Record `ops` on the Blake2b transcript, checking the verifier replay.
fn blake2b_script<C>(name: &str, ops: &[Op<C>]) -> Value
where
    C: CurveAffine,
    C::Scalar: FromUniformBytes<64>,
{
    let mut prover = Blake2bWrite::<Vec<u8>, C, Challenge255<C>>::init(Vec::new());
    let challenges = run_prover(&mut prover, ops);
    let stream = prover.finalize();
    let mut reader = stream.as_slice();
    let replayed = {
        let mut verifier = Blake2bRead::<&mut &[u8], C, Challenge255<C>>::init(&mut reader);
        run_verifier(&mut verifier, ops)
    };
    assert!(replayed == challenges, "{name}: replay");
    assert!(reader.is_empty(), "{name}: replay consumes the stream");
    let mut absorbed = Vec::new();
    let mut next = challenges.iter();
    let entries = ops
        .iter()
        .map(|op| {
            absorbed.extend(blake2b_absorbed(op));
            let challenge = matches!(op, Op::Squeeze).then(|| next.next().expect("challenge"));
            op_json(op, challenge, absorbed.len())
        })
        .collect();
    obj(vec![
        ("name", s(name)),
        ("ops", Value::Array(entries)),
        ("absorbed_hex", s(hex(&absorbed))),
        ("stream_hex", s(hex(&stream))),
    ])
}

/// Record `ops` on the KAGEMUSHA Poseidon transcript, checking the verifier replay.
fn poseidon_script<C>(name: &str, ops: &[Op<C>]) -> Value
where
    C: CurveAffine,
    C::Scalar: FieldExt,
{
    let mut prover = KagemushaTranscript::<C, Vec<u8>>::new::<POSEIDON_SECURE_MDS>(Vec::new());
    let challenges = run_prover::<C, ChallengeScalar<C>, _>(&mut prover, ops);
    let stream = prover.finalize();
    let mut reader = stream.as_slice();
    let replayed = {
        let mut verifier =
            KagemushaTranscript::<C, &mut &[u8]>::new::<POSEIDON_SECURE_MDS>(&mut reader);
        run_verifier::<C, ChallengeScalar<C>, _>(&mut verifier, ops)
    };
    assert!(replayed == challenges, "{name}: replay");
    assert!(reader.is_empty(), "{name}: replay consumes the stream");
    let mut absorbed = Vec::new();
    let mut next = challenges.iter();
    let entries = ops
        .iter()
        .map(|op| {
            absorbed.extend(poseidon_absorbed(op));
            let challenge = matches!(op, Op::Squeeze).then(|| next.next().expect("challenge"));
            op_json(op, challenge, absorbed.len())
        })
        .collect();
    obj(vec![
        ("name", s(name)),
        ("ops", Value::Array(entries)),
        (
            "absorbed",
            Value::Array(absorbed.iter().map(|value| s(fe_hex(value))).collect()),
        ),
        ("stream_hex", s(hex(&stream))),
    ])
}

/// Encodings every transcript reader must reject, as `(name, kind, bytes)`.
fn rejected_encodings<C: CurveAffine>() -> Vec<(&'static str, &'static str, Vec<u8>)> {
    let width = C::Repr::default().as_ref().len();
    let mut non_canonical_x = modulus_bytes::<C::Base>();
    non_canonical_x.resize(width, 0);
    // Start at x = 1: x = 0 has no point on either Pasta curve (5 is a
    // non-residue in both fields), but its sign-0 encoding is the identity
    // encoding, which `point_identity` already covers.
    let off_curve_x = (1_u64..256)
        .map(C::Base::from)
        .find(|x| bool::from((x.square() * x + C::a() * x + C::b()).sqrt().is_none()))
        .expect("a small x with a non-residue right-hand side exists");
    vec![
        ("scalar_modulus", "scalar", modulus_bytes::<C::Scalar>()),
        ("scalar_all_ones", "scalar", vec![0xff; width]),
        ("point_identity", "point", vec![0; width]),
        ("point_non_canonical_x", "point", non_canonical_x),
        (
            "point_off_curve",
            "point",
            off_curve_x.to_repr().as_ref().to_vec(),
        ),
    ]
}

/// Whether a verifier transcript built by `init` rejects `bytes` read as `kind`.
fn reader_rejects<C, E, T>(kind: &str, bytes: &[u8], init: impl FnOnce(&[u8]) -> T) -> bool
where
    C: CurveAffine,
    E: EncodedChallenge<C>,
    T: TranscriptRead<C, E>,
{
    let mut transcript = init(bytes);
    match kind {
        "scalar" => transcript.read_scalar().is_err(),
        _ => transcript.read_point().is_err(),
    }
}

/// Decoding and absorption rejections shared by both transcripts on curve `C`.
fn rejections<C, E, T>(
    init: impl Fn(&[u8]) -> T,
    common_identity: impl FnOnce() -> io::Result<()>,
) -> Value
where
    C: CurveAffine,
    E: EncodedChallenge<C>,
    T: TranscriptRead<C, E>,
{
    assert!(
        common_identity().is_err(),
        "common_point(identity) is rejected"
    );
    let mut entries = vec![obj(vec![
        ("name", s("common_point_identity")),
        ("op", s("common_point")),
        ("result", s("rejected")),
    ])];
    for (name, kind, bytes) in rejected_encodings::<C>() {
        assert!(
            reader_rejects::<C, E, T>(kind, &bytes, &init),
            "{name}: read_{kind} rejects"
        );
        entries.push(obj(vec![
            ("name", s(name)),
            ("op", s(format!("read_{kind}"))),
            ("input_hex", s(hex(&bytes))),
            ("result", s("rejected")),
        ]));
    }
    Value::Array(entries)
}

/// Blake2b transcript vectors for curve `C`.
fn blake2b_for<C>(curve: &str) -> Value
where
    C: CurveAffine,
    C::Scalar: FromUniformBytes<64>,
{
    let scripts = scripts::<C>(curve)
        .iter()
        .map(|(name, ops)| blake2b_script(name, ops))
        .collect();
    let rejections = rejections::<C, Challenge255<C>, _>(
        |bytes| Blake2bRead::<_, C, Challenge255<C>>::init(io::Cursor::new(bytes.to_vec())),
        || {
            let mut transcript = Blake2bWrite::<Vec<u8>, C, Challenge255<C>>::init(Vec::new());
            halo2_axiom::transcript::Transcript::common_point(&mut transcript, C::identity())
        },
    );
    obj(vec![
        ("scripts", Value::Array(scripts)),
        ("rejections", rejections),
    ])
}

/// Poseidon transcript vectors for curve `C`.
fn poseidon_transcript_for<C>(curve: &str) -> Value
where
    C: CurveAffine,
    C::Scalar: FieldExt,
{
    let scripts = scripts::<C>(curve)
        .iter()
        .map(|(name, ops)| poseidon_script(name, ops))
        .collect();
    let rejections = rejections::<C, ChallengeScalar<C>, _>(
        |bytes| {
            KagemushaTranscript::<C, _>::new::<POSEIDON_SECURE_MDS>(io::Cursor::new(bytes.to_vec()))
        },
        || {
            let mut transcript =
                KagemushaTranscript::<C, Vec<u8>>::new::<POSEIDON_SECURE_MDS>(Vec::new());
            halo2_axiom::transcript::Transcript::<C, ChallengeScalar<C>>::common_point(
                &mut transcript,
                C::identity(),
            )
        },
    );
    obj(vec![
        ("scripts", Value::Array(scripts)),
        ("rejections", rejections),
    ])
}

/// The `blake2b_transcript` section.
fn blake2b_transcript() -> Value {
    obj(vec![
        (
            "transcript",
            s("halo2_axiom::transcript::Blake2bWrite/Blake2bRead<_, C, Challenge255<C>>"),
        ),
        (
            "hash",
            s("BLAKE2b, 64-byte output, personalization \"Halo2-Transcript\""),
        ),
        (
            "absorb",
            s(
                "common_point: 0x01 || x || y (affine, canonical LE); common_scalar: 0x02 || repr; write_* absorbs as common_* and appends the compressed point or repr to the stream",
            ),
        ),
        (
            "squeeze",
            s(
                "absorb 0x00, finalize a copy of the state, challenge = from_uniform_bytes(64 bytes); the state continues",
            ),
        ),
        (
            "absorbed",
            s("byte length of absorbed_hex consumed when the challenge was squeezed"),
        ),
        ("eq", blake2b_for::<EqAffine>("eq")),
        ("ep", blake2b_for::<EpAffine>("ep")),
    ])
}

/// The `poseidon_transcript` section.
fn poseidon_transcript() -> Value {
    obj(vec![
        (
            "transcript",
            s(
                "snark_verifier::system::halo2::transcript::halo2::PoseidonTranscript<C, NativeLoader, _, 3, 2, 8, 57>::new::<0>",
            ),
        ),
        (
            "users",
            s(
                "iroha_core_zk KAGEMUSHA_IPA_POSEIDON_{WIDTH,RATE,FULL_ROUNDS,PARTIAL_ROUNDS,SECURE_MDS}_V1 = 3, 2, 8, 57, 0",
            ),
        ),
        (
            "absorb",
            s(
                "common_point: [fe_to_fe(x), fe_to_fe(y)] in the scalar field; common_scalar: the scalar; write_* absorbs as common_* and appends the compressed point or repr to the stream",
            ),
        ),
        (
            "squeeze",
            s(
                "snark-verifier Poseidon sponge over the buffered elements (initial capacity 2^64, pad 1 then 0s, an extra permutation when the buffer length is a multiple of the rate); challenge = state[1]",
            ),
        ),
        (
            "absorbed",
            s("number of absorbed elements consumed when the challenge was squeezed"),
        ),
        ("eq", poseidon_transcript_for::<EqAffine>("eq")),
        ("ep", poseidon_transcript_for::<EpAffine>("ep")),
    ])
}

/// Unoptimized round constants and MDS matrix of one field.
fn poseidon_constants_for<F: FieldExt>() -> Value {
    let (round_constants, mds) = PoseidonSpec::<F>::unoptimized_constants::<
        POSEIDON_FULL_ROUNDS,
        POSEIDON_PARTIAL_ROUNDS,
        POSEIDON_SECURE_MDS,
    >();
    assert_eq!(
        round_constants.len(),
        POSEIDON_FULL_ROUNDS + POSEIDON_PARTIAL_ROUNDS
    );
    let row =
        |values: &[F; POSEIDON_T]| Value::Array(values.iter().map(|v| s(fe_hex(v))).collect());
    obj(vec![
        (
            "round_constants",
            Value::Array(round_constants.iter().map(row).collect()),
        ),
        ("mds", Value::Array(mds.iter().map(row).collect())),
    ])
}

/// The `poseidon_constants` section.
fn poseidon_constants() -> Value {
    obj(vec![
        (
            "spec",
            s(
                "halo2_base OptimizedPoseidonSpec<F, 3, 2>::unoptimized_constants::<8, 57, 0>() (poseidon-primitives Grain LFSR generator)",
            ),
        ),
        ("t", int(POSEIDON_T)),
        ("rate", int(POSEIDON_RATE)),
        ("full_rounds", int(POSEIDON_FULL_ROUNDS)),
        ("partial_rounds", int(POSEIDON_PARTIAL_ROUNDS)),
        ("secure_mds", int(POSEIDON_SECURE_MDS)),
        ("sbox", s("x^5")),
        ("fp", poseidon_constants_for::<Fp>()),
        ("fq", poseidon_constants_for::<Fq>()),
    ])
}

/// The native sponge exactly as `kagemusha_v1_poseidon` and `confidential_v2` build it.
fn native_sponge<F: FieldExt>() -> NativePoseidon<F> {
    NativePoseidon::<F>::from_spec(
        &NativeLoader,
        PoseidonSpec::<F>::new::<POSEIDON_FULL_ROUNDS, POSEIDON_PARTIAL_ROUNDS, POSEIDON_SECURE_MDS>(
        ),
    )
}

/// `hash(domain, inputs)`: one fresh sponge over `[domain, len, inputs...]`.
///
/// This is `kagemusha_v1_poseidon::hash` and `confidential_poseidon_hash_v3`.
fn domain_hash<F: FieldExt>(sponge: &mut NativePoseidon<F>, domain: u64, inputs: &[F]) -> F {
    let mut preimage = Vec::with_capacity(inputs.len() + 2);
    preimage.push(F::from(domain));
    preimage.push(F::from(
        u64::try_from(inputs.len()).expect("arity fits u64"),
    ));
    preimage.extend_from_slice(inputs);
    sponge.clear();
    sponge.update(&preimage);
    sponge.squeeze()
}

/// One `{domain, inputs, output}` vector.
fn hash_vector<F: FieldExt>(sponge: &mut NativePoseidon<F>, tag: [u8; 8], inputs: &[F]) -> Value {
    let output = domain_hash(sponge, domain_word(tag), inputs);
    obj(vec![
        (
            "domain",
            s(std::str::from_utf8(&tag).expect("ASCII domain tag")),
        ),
        (
            "inputs",
            Value::Array(inputs.iter().map(|value| s(fe_hex(value))).collect()),
        ),
        ("output", s(fe_hex(&output))),
    ])
}

/// KAGEMUSHA vectors for one field.
fn kagemusha_for<F: FieldExt>(field: &str) -> Value {
    let mut sponge = native_sponge::<F>();
    let mut rng = data_rng(&format!("iroha native prover kats v1 kagemusha {field}"));
    let mut random = |count: usize| (0..count).map(|_| F::random(&mut rng)).collect::<Vec<F>>();
    let [empty, leaf, node, state] = KAGEMUSHA_DOMAINS;
    let mut vectors = vec![
        hash_vector(&mut sponge, empty, &[]),
        hash_vector(&mut sponge, node, &[F::ZERO]),
        hash_vector(&mut sponge, node, &[F::ONE, -F::ONE]),
        hash_vector(&mut sponge, node, &[F::from(7), F::from(9)]),
        hash_vector(&mut sponge, leaf, &[F::from(7), F::from(9)]),
    ];
    for (tag, arity) in [
        (leaf, 1),
        (leaf, 3),
        (state, 4),
        (node, 2),
        (node, 23),
        (node, 32),
        (node, 33),
    ] {
        let inputs = random(arity);
        vectors.push(hash_vector(&mut sponge, tag, &inputs));
    }
    let mut root = domain_hash(&mut sponge, domain_word(empty), &[]);
    for _ in 0..KAGEMUSHA_REPLAY_DEPTH {
        root = domain_hash(&mut sponge, domain_word(node), &[root, root]);
    }
    obj(vec![
        ("vectors", Value::Array(vectors)),
        ("empty_replay_root", s(fe_hex(&root))),
    ])
}

/// The `kagemusha_v1_poseidon` section.
fn kagemusha_v1_poseidon() -> Value {
    obj(vec![
        (
            "reproduces",
            s("iroha_core_zk::kagemusha_v1_poseidon::hash::<F>(domain, inputs)"),
        ),
        (
            "production_anchors",
            s(
                "none yet: no iroha_core_zk test pins a KAGEMUSHA hash value (the module is pub(crate)). These vectors come from the oracle reproduction (construction below, built from the same halo2-base and snark-verifier types as production), whose sponge is anchored by the confidential_v3_poseidon production anchors. TODO: an iroha_core_zk test asserts these vectors and the empty replay root",
            ),
        ),
        (
            "construction",
            s(
                "snark_verifier Poseidon<F, F, 3, 2>::from_spec(NativeLoader, OptimizedPoseidonSpec<F, 3, 2>::new::<8, 57, 0>()); clear; update([F::from(domain), F::from(len), inputs..]); squeeze",
            ),
        ),
        (
            "domain",
            s("the 8 ASCII bytes read as a little-endian u64 (u64::from_le_bytes(*b\"kgmnode1\"))"),
        ),
        (
            "empty_replay_root",
            s("r = hash(kgmemp_1, []); 256 times r = hash(kgmnode1, [r, r])"),
        ),
        ("fp", kagemusha_for::<Fp>("fp")),
        ("fq", kagemusha_for::<Fq>("fq")),
    ])
}

/// Confidential V3 vectors for one field.
fn confidential_for<F: FieldExt>(field: &str) -> Value {
    let mut sponge = native_sponge::<F>();
    let small = |values: &[u64]| values.iter().copied().map(F::from).collect::<Vec<F>>();
    let [owner, note, nullifier, leaf, parent, asset, network] = CONFIDENTIAL_DOMAINS;
    let mut vectors = Vec::new();
    for (tag, inputs) in [
        (owner, small(&[3, 5])),
        (owner, small(&[3, 5, 8, 13])),
        (note, small(&[3, 5, 8, 13])),
        (nullifier, small(&[3, 5, 8, 13])),
        (leaf, small(&[3])),
        (leaf, small(&[3, 5, 8, 13])),
        (parent, small(&[3, 5])),
        (parent, small(&[3, 5, 8, 13])),
        (asset, small(&[3])),
        (network, small(&[3])),
        (leaf, small(&[0])),
    ] {
        vectors.push(hash_vector(&mut sponge, tag, &inputs));
    }
    let mut rng = data_rng(&format!("iroha native prover kats v1 confidential {field}"));
    let mut random = || F::random(&mut rng);
    let amount = F::from_u128(1_000_000_000_000_000_007);
    let note_inputs = [amount, random(), random(), random()];
    vectors.push(hash_vector(&mut sponge, note, &note_inputs));
    let nullifier_inputs = [random(), random(), random(), random()];
    vectors.push(hash_vector(&mut sponge, nullifier, &nullifier_inputs));
    let mut roots = vec![domain_hash(&mut sponge, domain_word(leaf), &[F::ZERO])];
    for level in 0..CONFIDENTIAL_TREE_DEPTH {
        let child = roots[level];
        roots.push(domain_hash(
            &mut sponge,
            domain_word(parent),
            &[child, child],
        ));
    }
    obj(vec![
        ("vectors", Value::Array(vectors)),
        (
            "empty_subtree_roots",
            Value::Array(roots.iter().map(|root| s(fe_hex(root))).collect()),
        ),
    ])
}

/// The `confidential_v3_poseidon` section.
fn confidential_v3_poseidon() -> Value {
    obj(vec![
        (
            "reproduces",
            s("iroha_core_zk::confidential_v2::confidential_poseidon_hash_v3::<F>(domain, inputs)"),
        ),
        (
            "production_anchors",
            s(
                "hash(cfownr03, [3, 5]), hash(cfnote03, [3, 5, 8, 13]) and hash(cfnet_03, [3]) on Fp and Fq, pinned by secure_confidential_poseidon_kats_pin_both_pasta_fields_and_domains in crates/iroha_core_zk/src/confidential_v2_tests.rs and checked against this reproduction by confidential_reproduction_matches_iroha_core_zk_kats",
            ),
        ),
        (
            "construction",
            s(
                "identical to kagemusha_v1_poseidon (the length word is F::from_u128(len)); production uses Fp only, Fq is pinned by the existing tests",
            ),
        ),
        (
            "note_commitment",
            s(
                "hash(cfnote03, [amount as F::from_u128, rho scalar, owner tag, asset tag]); the 12th vector is note-shaped",
            ),
        ),
        (
            "empty_subtree_roots",
            s(
                "roots[0] = hash(cfleaf03, [0]); roots[i + 1] = hash(cfnode03, [roots[i], roots[i]]) up to depth 16",
            ),
        ),
        ("fp", confidential_for::<Fp>("fp")),
        ("fq", confidential_for::<Fq>("fq")),
    ])
}

/// Build the fixture document with the `params_ipa` sizes in `params_ks`.
fn build(params_ks: &std::ops::RangeInclusive<u32>) -> Value {
    obj(vec![
        ("format", s(FORMAT)),
        ("oracle_baseline", oracle_baseline()),
        ("golden_proofs", golden_proofs()),
        ("params_ipa", params_ipa(params_ks)),
        ("generators", generators()),
        ("blake2b_transcript", blake2b_transcript()),
        ("poseidon_transcript", poseidon_transcript()),
        ("poseidon_constants", poseidon_constants()),
        ("kagemusha_v1_poseidon", kagemusha_v1_poseidon()),
        ("confidential_v3_poseidon", confidential_v3_poseidon()),
    ])
}

/// Absolute path of the fixture.
fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(FIXTURE)
}

/// Read, parse and canonicality-check the fixture.
fn read_fixture() -> Value {
    let path = fixture_path();
    let mut text = String::new();
    std::fs::File::open(&path)
        .and_then(|mut file| file.read_to_string(&mut text))
        .unwrap_or_else(|error| {
            panic!(
                "read {}: {error}; record it with {UPDATE_ENV}=1",
                path.display()
            )
        });
    let fixture = norito::json::parse_value(&text)
        .unwrap_or_else(|error| panic!("parse {}: {error}", path.display()));
    assert!(
        render(&fixture) == text,
        "{} is not in canonical form; regenerate it with {UPDATE_ENV}=1",
        path.display()
    );
    fixture
}

/// Keep only the `params_ipa` entries whose `k` lies in `ks`.
fn retain_params(document: &mut Value, ks: &std::ops::RangeInclusive<u32>) {
    let params = document.get_mut("params_ipa").expect("params_ipa section");
    for curve in ["eq", "ep"] {
        if let Some(entries) = params.get_mut(curve).and_then(Value::as_array_mut) {
            entries.retain(|entry| {
                entry
                    .get("k")
                    .and_then(Value::as_u64)
                    .and_then(|k| u32::try_from(k).ok())
                    .is_none_or(|k| ks.contains(&k))
            });
        }
    }
}

/// Fail with the first difference between `expected` and the fixture.
fn assert_matches(expected: &Value, fixture: &Value) {
    if let Some(difference) = first_difference("$", expected, fixture) {
        panic!(
            "{} differs from the vendored stack at {difference}\n\
             A changed vector means the vendored behaviour changed; review it before \
             regenerating with {UPDATE_ENV}=1.",
            fixture_path().display()
        );
    }
}

#[test]
fn native_prover_kats_match_fixture() {
    if std::env::var(UPDATE_ENV).as_deref() == Ok("1") {
        let all = *PARAMS_K_DEBUG.start()..=*PARAMS_K_RELEASE.end();
        let text = render(&build(&all));
        std::fs::write(fixture_path(), text).expect("write the fixture");
        println!("wrote {}", fixture_path().display());
        return;
    }
    let mut fixture = read_fixture();
    retain_params(&mut fixture, &PARAMS_K_DEBUG);
    assert_matches(&build(&PARAMS_K_DEBUG), &fixture);
}

#[test]
#[ignore = "k = 15 and 16 params; run in release"]
fn native_prover_release_params_match_fixture() {
    let mut fixture = read_fixture();
    retain_params(&mut fixture, &PARAMS_K_RELEASE);
    let fixture = fixture.get("params_ipa").expect("params_ipa section");
    assert_matches(&params_ipa(&PARAMS_K_RELEASE), fixture);
}

#[test]
fn confidential_reproduction_matches_iroha_core_zk_kats() {
    fn anchor<F: FieldExt>(tag: [u8; 8], inputs: &[u64]) -> String {
        let inputs = inputs.iter().copied().map(F::from).collect::<Vec<_>>();
        fe_hex(&domain_hash(
            &mut native_sponge::<F>(),
            domain_word(tag),
            &inputs,
        ))
    }
    // Pinned by secure_confidential_poseidon_kats_pin_both_pasta_fields_and_domains in
    // crates/iroha_core_zk/src/confidential_v2_tests.rs.
    let anchors: [([u8; 8], &[u64], &str, &str); 3] = [
        (
            *b"cfownr03",
            &[3, 5],
            "612ad09a40970302036fef4c16385a98a7b337143c086d7ec4c0f9fc4792610d",
            "da41767db79387f7bfb20625144da612661c38f7ea94dc3a62f330e9ddbbef10",
        ),
        (
            *b"cfnote03",
            &[3, 5, 8, 13],
            "cdb844f8a478ebf314546cc9a8145bbca05b4221a31a9cee2a34a6b2d898862c",
            "222fe8dfb11b68b93847d28694db28c5636c5bbf78a7b7db73c62b3e389ac02d",
        ),
        (
            *b"cfnet_03",
            &[3],
            "971c0d57fd63afa24ea0d1c6206a4873d7be0b283848eba6fc9fd929d2747d04",
            "569c33f348ee0dd7714f5b10a50f797ada3f0b49f373de606445c0ddb338f737",
        ),
    ];
    for (tag, inputs, expected_fp, expected_fq) in anchors {
        assert_eq!(anchor::<Fp>(tag, inputs), expected_fp);
        assert_eq!(anchor::<Fq>(tag, inputs), expected_fq);
    }
}

#[test]
fn reused_sponge_matches_a_fresh_sponge() {
    let mut reused = native_sponge::<Fp>();
    let first = domain_hash(&mut reused, 1, &[Fp::from(11); 33]);
    let second = domain_hash(&mut reused, 2, &[Fp::from(7), Fp::from(9)]);
    assert_eq!(
        second,
        domain_hash(&mut native_sponge(), 2, &[Fp::from(7), Fp::from(9)])
    );
    assert_ne!(first, second);
}

#[test]
fn json_helpers_render_canonically_and_round_trip() {
    let document = obj(vec![
        ("text", s("quote \" backslash \\ newline \n")),
        ("number", int(42)),
        ("empty", Value::Array(Vec::new())),
        (
            "nested",
            Value::Array(vec![obj(vec![("k", int(1))]), obj(Vec::new())]),
        ),
    ]);
    let text = render(&document);
    assert!(text.ends_with("}\n"));
    // Keys are ordered canonically, whatever the insertion order.
    assert!(text.find("\"empty\"") < text.find("\"text\""));
    let parsed = norito::json::parse_value(&text).expect("parse rendered JSON");
    assert_eq!(parsed, document);
    assert_eq!(render(&parsed), text);
    assert_eq!(compact(&int(7)), "7");
}

#[test]
#[should_panic(expected = "duplicate key")]
fn obj_rejects_duplicate_keys() {
    let _ = obj(vec![("a", int(1)), ("a", int(2))]);
}

#[test]
fn first_difference_reports_the_json_path() {
    let left = obj(vec![("a", Value::Array(vec![int(1), s("x")]))]);
    let right = obj(vec![("a", Value::Array(vec![int(1), s("y")]))]);
    assert_eq!(first_difference("$", &left, &left), None);
    let difference = first_difference("$", &left, &right).expect("difference");
    assert!(
        difference.starts_with("$.a[1]: expected \"x\""),
        "{difference}"
    );
    let shorter = obj(vec![("a", Value::Array(vec![int(1)]))]);
    assert!(
        first_difference("$", &left, &shorter)
            .expect("length difference")
            .contains("expected 2 items")
    );
    let renamed = obj(vec![("b", Value::Array(Vec::new()))]);
    assert!(
        first_difference("$", &left, &renamed)
            .expect("key difference")
            .contains("keys differ")
    );
}

#[test]
fn retain_params_filters_by_k() {
    let entry = |k: usize| obj(vec![("k", int(k))]);
    let mut document = obj(vec![(
        "params_ipa",
        obj(vec![
            ("eq", Value::Array(vec![entry(15), entry(16)])),
            ("ep", Value::Array(vec![entry(16)])),
        ]),
    )]);
    retain_params(&mut document, &(16..=16));
    let params = document.get("params_ipa").expect("section");
    assert_eq!(params.get("eq"), Some(&Value::Array(vec![entry(16)])));
    assert_eq!(params.get("ep"), Some(&Value::Array(vec![entry(16)])));
}

#[test]
fn kagemusha_golden_source_is_read_at_run_time() {
    let table = golden_table(&kagemusha_golden_source());
    assert_eq!(table.len(), 8, "the eight KAGEMUSHA prover goldens");
    let fixture = read_fixture();
    let cases = fixture
        .get("golden_proofs")
        .and_then(|section| section.get("iroha_core_zk_kagemusha"))
        .and_then(|section| section.get("cases"))
        .and_then(Value::as_array)
        .expect("golden_proofs.iroha_core_zk_kagemusha.cases");
    assert_eq!(cases.len(), table.len());
    assert!(PARAMS_K_DEBUG.end() < PARAMS_K_RELEASE.start());
    assert_eq!(*PARAMS_K_RELEASE.start(), 15);
}

#[test]
fn golden_table_parser_pairs_cases_with_digests() {
    let source = "x\nconst GOLDEN_SHA256: &[(&str, &str)] = &[\n    (\n        \"a/b\",\n        \"\
        0000000000000000000000000000000000000000000000000000000000000001\",\n    ),\n];\nconst Y: &str = \"z\";\n";
    assert_eq!(
        golden_table(source),
        vec![(
            "a/b".to_owned(),
            "0000000000000000000000000000000000000000000000000000000000000001".to_owned()
        )]
    );
    let vendored = golden_table(include_str!(
        "../../../vendor/halo2-axiom/tests/golden_proof_bytes.rs"
    ));
    assert_eq!(vendored.len(), 20);
    assert!(
        vendored
            .iter()
            .any(|(case, _)| case == "sigma/eq/k11/seed42")
    );
}

#[test]
fn rejected_encodings_are_distinct_and_off_curve() {
    fn check<C: CurveAffine>() {
        let encodings = rejected_encodings::<C>();
        for (index, (name, _, bytes)) in encodings.iter().enumerate() {
            assert!(
                encodings[index + 1..]
                    .iter()
                    .all(|(_, _, other)| other != bytes),
                "{name} duplicates a later rejection"
            );
        }
        let (_, _, off_curve) = encodings
            .iter()
            .find(|(name, _, _)| *name == "point_off_curve")
            .expect("off-curve entry");
        let mut repr = C::Repr::default();
        repr.as_mut().copy_from_slice(off_curve);
        assert!(bool::from(C::from_bytes(&repr).is_none()));
        assert_ne!(off_curve.as_slice(), C::identity().to_bytes().as_ref());
    }
    check::<EqAffine>();
    check::<EpAffine>();
}

#[test]
fn encoding_helpers_are_canonical() {
    assert_eq!(hex(&[0x00, 0x0f, 0xa0, 0xff]), "000fa0ff");
    assert_eq!(
        sha256_hex(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(domain_word(*b"\x01\0\0\0\0\0\0\0"), 1);
    assert_eq!(
        fe_hex(&Fp::ONE),
        "0100000000000000000000000000000000000000000000000000000000000000"
    );
    // p = 0x40000000000000000000000000000000224698fc094cf91b992d30ed00000001.
    assert_eq!(
        hex(&modulus_bytes::<Fp>()),
        "01000000ed302d991bf94c09fc98462200000000000000000000000000000040"
    );
    assert_eq!(
        point_hex(&EqAffine::identity()),
        "0000000000000000000000000000000000000000000000000000000000000000"
    );
    let mut first = data_rng("label");
    let mut second = data_rng("label");
    assert_eq!(Fp::random(&mut first), Fp::random(&mut second));
}
