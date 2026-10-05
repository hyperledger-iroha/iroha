//! `PINNED_PARAMS_V1` against the `iroha_pasta` derivation and the
//! `fixtures/native_prover/kats_v1.json` digests (spec section 3, S5).

use iroha_pasta::{Ep, Eq, PastaCurve, params::ParamsIpa};
use iroha_plonk::cs::{
    CurveV1,
    descriptor::{PINNED_PARAMS_V1, pinned_params_digest},
};
use sha2::{Digest, Sha256};

/// Fixture path relative to this crate.
const FIXTURE: &str = "../../fixtures/native_prover/kats_v1.json";

fn hex(bytes: &[u8]) -> String {
    use core::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut out, byte| {
        let _ = write!(out, "{byte:02x}");
        out
    })
}

fn derived_digest<C: PastaCurve>(k: u32) -> [u8; 32] {
    let params = ParamsIpa::<C>::new(k).expect("params derivation");
    Sha256::digest(params.to_bytes()).into()
}

fn check_range(range: core::ops::RangeInclusive<u32>) {
    for k in range {
        assert_eq!(
            pinned_params_digest(CurveV1::Pallas, k),
            Some(derived_digest::<Ep>(k)),
            "Pallas k = {k}"
        );
        assert_eq!(
            pinned_params_digest(CurveV1::Vesta, k),
            Some(derived_digest::<Eq>(k)),
            "Vesta k = {k}"
        );
    }
}

#[test]
fn pinned_digests_match_the_derivation() {
    check_range(6..=12);
}

#[test]
#[ignore = "release-mode derivation of k = 13..=16 params"]
fn pinned_digests_match_the_derivation_large_k() {
    check_range(13..=16);
}

#[test]
fn pinned_digests_match_the_fixture() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(FIXTURE);
    let text = std::fs::read_to_string(&path).expect("read the KAT fixture");
    let fixture = norito::json::parse_value(&text).expect("parse the KAT fixture");
    let params = fixture.get("params_ipa").expect("params_ipa section");
    let mut seen = 0;
    for (label, curve) in [("ep", CurveV1::Pallas), ("eq", CurveV1::Vesta)] {
        for entry in params
            .get(label)
            .and_then(norito::json::Value::as_array)
            .expect("curve entries")
        {
            let k = entry
                .get("k")
                .and_then(norito::json::Value::as_u64)
                .expect("k");
            let digest = entry
                .get("sha256")
                .and_then(norito::json::Value::as_str)
                .expect("sha256");
            let k = u32::try_from(k).expect("small k");
            let pinned = pinned_params_digest(curve, k).expect("every fixture k is pinned");
            assert_eq!(hex(&pinned), digest, "{label} k = {k}");
            seen += 1;
        }
    }
    assert_eq!(
        seen,
        PINNED_PARAMS_V1.len(),
        "the table holds exactly the fixture entries"
    );
}
