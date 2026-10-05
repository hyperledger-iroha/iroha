//! Byte parity of `iroha_pasta` with the vendored halo2 stack (milestone M1a).
//!
//! Each module compares one `iroha_pasta` component with the vendored code it
//! replaces, directly in this process (no exported digests in between):
//!
//! - `encoding`: field and curve constants, encodings, decoding verdicts,
//!   arithmetic, seeded sampling, hash-to-curve and the halo2-axiom serde
//!   formats against the `halo2curves-axiom` Pasta types.
//! - `params`: `ParamsIpa` bytes against `ParamsIPA::write` for k = 6..=14 on
//!   both curves (k = 15 and 16 are ignored release tests), codec round trips,
//!   the documented stricter decoding, and the `kats_v1.json` digests.
//! - `msm`: `msm_public`, `msm_secret` and `FixedBaseTable` against
//!   `best_multiexp` on random and adversarial scalars and bases.
//! - `fold`: the lockstep generator fold against the vendored IPA collapse,
//!   per round on adversarial lanes and over full real vendored IPA proofs
//!   (the native folded generator must satisfy the vendored verifier).
//! - `fft`: FFT, IFFT and coset transforms against `EvaluationDomain` and every
//!   vendored FFT backend.
//! - `poseidon`: the RP57 tables, sponge and domain hash against halo2-base,
//!   `snark-verifier` and the `kats_v1.json` vectors.
//!
//! Every comparison runs inside the shared Rayon pools of 1, 2, 4 and 7 threads
//! (`iroha_plonk_oracle::pools`). Both the native and the vendored computation
//! run in the pool, the native result must be identical in every pool, and it
//! must equal the vendored result byte for byte. Inputs are seeded `ChaCha20`
//! streams, so every run checks the same cases.
//!
//! Run:
//! - `cargo test -p iroha_plonk_oracle --test pasta_parity`
//! - `cargo test --release -p iroha_plonk_oracle --test pasta_parity -- --include-ignored`

mod encoding;
mod fft;
mod fold;
mod msm;
mod params;
mod poseidon;

use std::{fmt::Write as _, io::Read as _, path::PathBuf, sync::OnceLock};

use halo2_axiom::halo2curves::ff::PrimeField;
use norito::json::Value;
use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;
use sha2::{Digest, Sha256};

/// Fixture path relative to this crate.
const FIXTURE: &str = "../../fixtures/native_prover/kats_v1.json";

/// A deterministic input stream named by `label` (never prover randomness).
fn data_rng(label: &str) -> ChaCha20Rng {
    ChaCha20Rng::from_seed(Sha256::digest(label.as_bytes()).into())
}

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

/// Bytes of a lowercase hex string.
fn unhex(text: &str) -> Vec<u8> {
    assert!(
        text.len().is_multiple_of(2),
        "hex has an even length: {text}"
    );
    (0..text.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&text[at..at + 2], 16).expect("hex digit pair"))
        .collect()
}

/// The field element whose canonical little-endian encoding is `text` (hex).
fn field_from_hex<F: PrimeField<Repr = [u8; 32]>>(text: &str) -> F {
    let bytes: [u8; 32] = unhex(text).try_into().expect("32-byte field encoding");
    Option::from(F::from_repr(bytes)).expect("canonical field encoding")
}

/// The parsed `fixtures/native_prover/kats_v1.json`, read once.
fn fixture() -> &'static Value {
    static FIXTURE_VALUE: OnceLock<Value> = OnceLock::new();
    FIXTURE_VALUE.get_or_init(|| {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(FIXTURE);
        let mut text = String::new();
        std::fs::File::open(&path)
            .and_then(|mut file| file.read_to_string(&mut text))
            .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
        norito::json::parse_value(&text)
            .unwrap_or_else(|error| panic!("parse {}: {error}", path.display()))
    })
}

/// The value at a `.`-separated path of object keys in the fixture.
fn fixture_at(path: &str) -> &'static Value {
    path.split('.').fold(fixture(), |value, key| {
        value
            .get(key)
            .unwrap_or_else(|| panic!("fixture has no key {key:?} in {path}"))
    })
}

/// The string at `key` of a fixture object.
fn str_at<'a>(value: &'a Value, key: &str) -> &'a str {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("fixture entry has no string {key:?}"))
}

/// The array at `key` of a fixture object.
fn array_at<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    value.get(key).and_then(Value::as_array).map_or_else(
        || panic!("fixture entry has no array {key:?}"),
        Vec::as_slice,
    )
}

#[test]
fn helpers_decode_and_encode_consistently() {
    use halo2_axiom::halo2curves::{ff::Field, pasta::Fp};
    assert_eq!(hex(&[0x00, 0x0f, 0xa0, 0xff]), "000fa0ff");
    assert_eq!(unhex("000fa0ff"), vec![0x00, 0x0f, 0xa0, 0xff]);
    assert_eq!(
        sha256_hex(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    let one: Fp =
        field_from_hex("0100000000000000000000000000000000000000000000000000000000000000");
    assert_eq!(one, Fp::ONE);
    let mut first = data_rng("label");
    let mut second = data_rng("label");
    assert_eq!(Fp::random(&mut first), Fp::random(&mut second));
    assert_eq!(
        str_at(fixture(), "format"),
        "iroha.native_prover.kats.v1",
        "the fixture parses"
    );
    assert_eq!(array_at(fixture_at("params_ipa"), "eq").len(), 11);
}

#[test]
#[should_panic(expected = "canonical field encoding")]
fn field_from_hex_rejects_non_canonical_encodings() {
    use halo2_axiom::halo2curves::pasta::Fp;
    let _: Fp = field_from_hex(&"ff".repeat(32));
}
