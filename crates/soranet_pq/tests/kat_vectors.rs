//! Known-answer tests for the `SoraNet` PQ helper crate.

use soranet_pq::{
    MlDsaSuite, MlKemSuite, decapsulate_mlkem, sign_mldsa, validate_mlkem_ciphertext,
    validate_mlkem_public_key, validate_mlkem_secret_key, verify_mldsa,
};

const KAT_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/soranet_pq/pq_kat.json"
));

#[test]
fn mlkem_known_answer_vectors_decapsulate_correctly() {
    let fixtures = load_fixtures().mlkem;
    assert!(
        !fixtures.is_empty(),
        "mlkem fixtures must be present in pq_kat.json"
    );

    for fixture in fixtures {
        let suite = parse_mlkem_suite(&fixture.suite);
        let pk = hex_to_bytes(&fixture.pk);
        let sk = hex_to_bytes(&fixture.sk);
        let ct = hex_to_bytes(fixture.ct.as_ref().expect("ciphertext present"));
        let ss = hex_to_bytes(fixture.ss.as_ref().expect("shared secret present"));

        validate_mlkem_public_key(suite, &pk)
            .unwrap_or_else(|err| panic!("{} public key invalid: {err}", fixture.suite));
        validate_mlkem_secret_key(suite, &sk)
            .unwrap_or_else(|err| panic!("{} secret key invalid: {err}", fixture.suite));
        validate_mlkem_ciphertext(suite, &ct)
            .unwrap_or_else(|err| panic!("{} ciphertext invalid: {err}", fixture.suite));

        let derived = decapsulate_mlkem(suite, &sk, &ct)
            .unwrap_or_else(|err| panic!("{} decapsulation failed: {err}", fixture.suite));
        assert_eq!(
            derived.as_bytes(),
            ss.as_slice(),
            "{} shared secret mismatch",
            fixture.suite
        );
    }
}

#[test]
fn mldsa_known_answer_vectors_verify_and_sign() {
    let fixtures = load_fixtures().mldsa;
    assert!(
        !fixtures.is_empty(),
        "mldsa fixtures must be present in pq_kat.json"
    );

    for fixture in fixtures {
        let suite = parse_mldsa_suite(&fixture.suite);
        let pk = hex_to_bytes(&fixture.pk);
        let sk = hex_to_bytes(&fixture.sk);
        let msg = hex_to_bytes(fixture.msg.as_ref().expect("message present"));
        let sig = hex_to_bytes(fixture.sig.as_ref().expect("signature present"));

        verify_mldsa(suite, &pk, &msg, &sig)
            .unwrap_or_else(|err| panic!("{} verification failed: {err}", fixture.suite));

        let regenerated = sign_mldsa(suite, &sk, &msg)
            .unwrap_or_else(|err| panic!("{} signing failed: {err}", fixture.suite));
        assert_eq!(
            regenerated.as_bytes(),
            sig.as_slice(),
            "{} regenerated signature mismatch",
            fixture.suite
        );
    }
}

/// Fixture file parsed with Norito JSON to keep consistency with workspace requirements.
fn load_fixtures() -> PqKatFixtures {
    norito::json::from_str(KAT_JSON).expect("parse pq_kat.json")
}

fn parse_mlkem_suite(name: &str) -> MlKemSuite {
    name.parse()
        .unwrap_or_else(|_| panic!("unsupported ML-KEM suite '{name}'"))
}

fn parse_mldsa_suite(name: &str) -> MlDsaSuite {
    match name {
        "MlDsa44" => MlDsaSuite::MlDsa44,
        "MlDsa65" => MlDsaSuite::MlDsa65,
        "MlDsa87" => MlDsaSuite::MlDsa87,
        other => panic!("unsupported ML-DSA suite '{other}'"),
    }
}

fn hex_to_bytes(input: &str) -> Vec<u8> {
    assert!(
        input.len().is_multiple_of(2),
        "hex string must have even length"
    );
    let mut out = Vec::with_capacity(input.len() / 2);
    for pair in input.as_bytes().chunks_exact(2) {
        let hi = nybble(pair[0]);
        let lo = nybble(pair[1]);
        out.push((hi << 4) | lo);
    }
    out
}

fn nybble(b: u8) -> u8 {
    match b {
        b'0'..=b'9' => b - b'0',
        b'a'..=b'f' => 10 + (b - b'a'),
        b'A'..=b'F' => 10 + (b - b'A'),
        _ => panic!("invalid hex digit {b}"),
    }
}

/// Structure mirroring the JSON fixture layout.
#[derive(Debug, Clone, norito::json::Deserialize)]
struct PqKatFixtures {
    #[norito(default)]
    mlkem: Vec<MlKemKat>,
    #[norito(default)]
    mldsa: Vec<MlDsaKat>,
}

#[derive(Debug, Clone, norito::json::Deserialize)]
struct MlKemKat {
    suite: String,
    pk: String,
    sk: String,
    #[norito(default)]
    ct: Option<String>,
    #[norito(default)]
    ss: Option<String>,
}

#[derive(Debug, Clone, norito::json::Deserialize)]
struct MlDsaKat {
    suite: String,
    pk: String,
    sk: String,
    #[norito(default)]
    msg: Option<String>,
    #[norito(default)]
    sig: Option<String>,
}
