//! `ParamsIpa` bytes against the vendored `ParamsIPA`.
//!
//! For each k, `iroha_pasta::params::ParamsIpa::<C>::new(k).to_bytes()` must
//! equal the vendored `ParamsIPA::<C>::new(k)` written with `Params::write`,
//! byte for byte, in every pool. A mismatch names the first differing point
//! (`g[i]`, `g_lagrange[i]`, `w` or `u`). The tests also check:
//!
//! - codec round trips across the implementations (the vendored reader accepts
//!   native bytes, the native reader accepts vendored bytes);
//! - the documented stricter native decoding: identity points and trailing
//!   bytes, which the vendored reader accepts, are rejected; truncated input,
//!   a wrong `k` header and non-canonical points are rejected by both;
//! - the `params_ipa` digests of `fixtures/native_prover/kats_v1.json`;
//! - `downsize` and coefficient/Lagrange commitments for small k.
//!
//! k = 6..=14 run by default on both curves. k = 15 and 16 are ignored release
//! tests.

use halo2_axiom::{
    arithmetic::CurveAffine,
    halo2curves::{
        ff::Field,
        group::{Curve, GroupEncoding, prime::PrimeCurveAffine},
    },
    poly::{
        EvaluationDomain,
        commitment::{Blind, Params, ParamsProver},
        ipa::commitment::ParamsIPA,
    },
};
use iroha_pasta::{
    msm::{MemoryBudget, msm_public},
    params::{ParamsError, ParamsIpa, encoded_len, lagrange_basis},
};
use iroha_plonk_oracle::{
    convert::{
        CurveBridge, NativeAffine, Pallas, Vesta, native_affine, native_scalars, vendored_point,
    },
    pools::{on_each_pool, same_on_each_pool},
};

use crate::{array_at, data_rng, fixture_at, sha256_hex, str_at};

/// Pool-dependence of the vendored generation is checked up to this k; above
/// it the vendored bytes are computed once and every native pool result is
/// compared with them.
const VENDORED_IN_EVERY_POOL_MAX_K: u32 = 10;
/// Bytes per compressed point.
const POINT: usize = 32;

/// The vendored `ParamsIPA::<C>::new(k)` bytes.
fn vendored_bytes<C: CurveAffine>(k: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    ParamsIPA::<C>::new(k)
        .write(&mut bytes)
        .expect("params write to Vec");
    bytes
}

/// Names the params element covering byte `offset` for size `k`.
fn locate(k: u32, offset: usize) -> String {
    let n = 1_usize << k;
    if offset < 4 {
        return "the k header".to_owned();
    }
    match (offset - 4) / POINT {
        index if index < n => format!("g[{index}]"),
        index if index < 2 * n => format!("g_lagrange[{}]", index - n),
        index if index == 2 * n => "w".to_owned(),
        index if index == 2 * n + 1 => "u".to_owned(),
        _ => "trailing bytes".to_owned(),
    }
}

/// Asserts byte equality, naming the first differing params element.
fn assert_params_bytes(label: &str, k: u32, expected: &[u8], actual: &[u8]) {
    assert_eq!(expected.len(), actual.len(), "{label}: params length");
    if let Some(offset) = expected.iter().zip(actual).position(|(e, a)| e != a) {
        panic!(
            "{label}: params bytes differ first at byte {offset}, in {}",
            locate(k, offset)
        );
    }
}

/// Byte parity, cross-implementation round trips and fixture digests at `k`.
fn params_parity<B: CurveBridge>(k: u32) {
    let label = format!("{} k{k}", B::NAME);
    let vendored_once =
        (k > VENDORED_IN_EVERY_POOL_MAX_K).then(|| vendored_bytes::<B::Vendored>(k));
    let native = same_on_each_pool(&label, |threads| {
        let native = ParamsIpa::<B::Native>::new(k)
            .expect("supported k")
            .to_bytes();
        let vendored = vendored_once
            .clone()
            .unwrap_or_else(|| vendored_bytes::<B::Vendored>(k));
        assert_params_bytes(
            &format!("{label} at {threads} threads"),
            k,
            &vendored,
            &native,
        );
        native
    });
    assert_eq!(Some(native.len()), encoded_len(k), "{label}: encoded_len");

    // Each implementation reads the other's bytes.
    let decoded = ParamsIpa::<B::Native>::from_bytes(&native).expect("native reads its bytes");
    assert!(decoded.matches_derivation(), "{label}: derivation check");
    let vendored = ParamsIPA::<B::Vendored>::read(&mut native.as_slice()).expect("vendored read");
    let mut rewritten = Vec::new();
    vendored.write(&mut rewritten).expect("params write to Vec");
    assert_params_bytes(&label, k, &native, &rewritten);
    assert_eq!(vendored.k(), decoded.k(), "{label}: k");
    assert_eq!(
        vendored.get_g()[..4]
            .iter()
            .map(GroupEncoding::to_bytes)
            .collect::<Vec<_>>(),
        decoded.g()[..4]
            .iter()
            .map(GroupEncoding::to_bytes)
            .collect::<Vec<_>>(),
        "{label}: g accessors"
    );
    assert_eq!(
        vendored.get_g_lagrange()[1].to_bytes(),
        decoded.g_lagrange()[1].to_bytes(),
        "{label}: g_lagrange accessor"
    );
    assert_eq!(
        vendored.get_blind_base().to_bytes(),
        decoded.w().to_bytes(),
        "{label}: w accessor"
    );

    // The M0 fixture digests.
    let entry = array_at(fixture_at("params_ipa"), B::NAME)
        .iter()
        .find(|entry| entry.get("k").and_then(norito::json::Value::as_u64) == Some(u64::from(k)))
        .unwrap_or_else(|| panic!("{label}: fixture entry"));
    let n = 1_usize << k;
    assert_eq!(
        entry.get("byte_len").and_then(norito::json::Value::as_u64),
        u64::try_from(native.len()).ok(),
        "{label}: fixture byte_len"
    );
    assert_eq!(
        str_at(entry, "sha256"),
        sha256_hex(&native),
        "{label}: fixture sha256"
    );
    assert_eq!(
        str_at(entry, "g_sha256"),
        sha256_hex(&native[4..4 + n * POINT]),
        "{label}: fixture g_sha256"
    );
    assert_eq!(
        str_at(entry, "g_lagrange_sha256"),
        sha256_hex(&native[4 + n * POINT..4 + 2 * n * POINT]),
        "{label}: fixture g_lagrange_sha256"
    );
}

#[test]
fn params_bytes_match_vendored_k6_to_k14() {
    for k in 6..=14 {
        params_parity::<Vesta>(k);
        params_parity::<Pallas>(k);
    }
}

#[test]
#[ignore = "k = 15 params on both curves; run in release"]
fn params_bytes_match_vendored_k15() {
    params_parity::<Vesta>(15);
    params_parity::<Pallas>(15);
}

#[test]
#[ignore = "k = 16 params on both curves; run in release"]
fn params_bytes_match_vendored_k16() {
    params_parity::<Vesta>(16);
    params_parity::<Pallas>(16);
}

/// The decoding verdicts of both readers on malformed params.
fn decoding_strictness<B: CurveBridge>() {
    let k = 4;
    let n = 1_usize << k;
    let honest = ParamsIpa::<B::Native>::new(k).expect("k4").to_bytes();
    let vendored_accepts = |bytes: &[u8]| ParamsIPA::<B::Vendored>::read(&mut &bytes[..]).is_ok();
    let native = |bytes: &[u8]| ParamsIpa::<B::Native>::from_bytes(bytes);
    assert!(vendored_accepts(&honest) && native(&honest).is_ok());

    // Stricter natively: an identity generator.
    let mut identity = honest.clone();
    identity[4 + 3 * POINT..4 + 4 * POINT].fill(0);
    assert!(
        vendored_accepts(&identity),
        "vendored reader accepts an identity point"
    );
    assert_eq!(
        native(&identity),
        Err(ParamsError::InvalidPoint { index: 3 })
    );

    // Stricter natively: trailing bytes.
    let mut trailing = honest.clone();
    trailing.push(0);
    assert!(
        vendored_accepts(&trailing),
        "vendored reader ignores trailing bytes"
    );
    assert!(matches!(
        native(&trailing),
        Err(ParamsError::WrongLength { .. })
    ));

    // Rejected by both: truncation, a larger k header, a non-canonical point.
    let truncated = &honest[..honest.len() - 1];
    assert!(!vendored_accepts(truncated));
    assert!(matches!(
        native(truncated),
        Err(ParamsError::WrongLength { .. })
    ));
    let mut larger_k = honest.clone();
    larger_k[..4].copy_from_slice(&(k + 1).to_le_bytes());
    assert!(!vendored_accepts(&larger_k));
    assert!(native(&larger_k).is_err());
    let mut non_canonical = honest.clone();
    let at = 4 + (n + 2) * POINT;
    non_canonical[at..at + POINT].fill(0xff);
    non_canonical[at + POINT - 1] = 0x7f;
    assert!(!vendored_accepts(&non_canonical));
    assert_eq!(
        native(&non_canonical),
        Err(ParamsError::InvalidPoint { index: n + 2 })
    );

    // Honestly encoded but not the derivation: swapped generators.
    let mut swapped = honest;
    let (first, rest) = swapped[4..].split_at_mut(POINT);
    first.swap_with_slice(&mut rest[..POINT]);
    assert!(vendored_accepts(&swapped));
    let decoded = native(&swapped).expect("well-formed points");
    assert!(!decoded.matches_derivation());
}

#[test]
fn native_decoding_is_at_least_as_strict_as_vendored() {
    decoding_strictness::<Vesta>();
    decoding_strictness::<Pallas>();
}

/// `downsize`, `lagrange_basis` and commitments on small parameters.
fn downsize_and_commitments<B: CurveBridge>() {
    let label = B::NAME;
    on_each_pool(|threads| {
        let mut vendored = ParamsIPA::<B::Vendored>::new(8);
        vendored.downsize(5);
        let native = ParamsIpa::<B::Native>::new(5).expect("k5");
        let mut written = Vec::new();
        vendored.write(&mut written).expect("params write to Vec");
        assert_params_bytes(
            &format!("{label} downsize 8 -> 5 at {threads} threads"),
            5,
            &written,
            &native.to_bytes(),
        );
        let projective: Vec<B::Native> =
            native.g().iter().map(PrimeCurveAffine::to_curve).collect();
        let lagrange = lagrange_basis(&projective, 5).expect("k5 basis");
        assert_eq!(lagrange, native.g_lagrange(), "{label}: lagrange_basis");

        let mut rng = data_rng(&format!("params commitments {label}"));
        let domain = EvaluationDomain::<B::VScalar>::new(1, 5);
        let values: Vec<B::VScalar> = (0..32).map(|_| B::VScalar::random(&mut rng)).collect();
        let blind = B::VScalar::random(&mut rng);
        let mut scalars = native_scalars::<B>(&values);
        scalars.push(iroha_plonk_oracle::convert::native_scalar::<B>(&blind));
        let budget = MemoryBudget::DEFAULT;
        let mut coefficient_bases: Vec<NativeAffine<B>> = native.g().to_vec();
        coefficient_bases.push(native.w());
        let mut lagrange_bases: Vec<NativeAffine<B>> = native.g_lagrange().to_vec();
        lagrange_bases.push(native.w());
        let coefficient =
            msm_public::<B::Native>(&scalars, &coefficient_bases, budget).expect("commitment MSM");
        let evaluation =
            msm_public::<B::Native>(&scalars, &lagrange_bases, budget).expect("commitment MSM");
        let commit = vendored.commit(&domain.coeff_from_vec(values.clone()), Blind(blind));
        let commit_lagrange =
            vendored.commit_lagrange(&domain.lagrange_from_vec(values), Blind(blind));
        assert_eq!(
            native_affine::<B>(&commit.to_affine()).to_bytes(),
            coefficient.to_affine().to_bytes(),
            "{label}: commit"
        );
        assert_eq!(
            vendored_point::<B>(&evaluation),
            commit_lagrange,
            "{label}: commit_lagrange"
        );
    });
}

#[test]
fn downsize_and_commitments_match_vendored() {
    downsize_and_commitments::<Vesta>();
    downsize_and_commitments::<Pallas>();
}

#[test]
fn locate_names_every_params_section() {
    assert_eq!(locate(2, 0), "the k header");
    assert_eq!(locate(2, 4), "g[0]");
    assert_eq!(locate(2, 4 + 3 * POINT + 31), "g[3]");
    assert_eq!(locate(2, 4 + 4 * POINT), "g_lagrange[0]");
    assert_eq!(locate(2, 4 + 8 * POINT), "w");
    assert_eq!(locate(2, 4 + 9 * POINT), "u");
    assert_eq!(locate(2, 4 + 10 * POINT), "trailing bytes");
}
