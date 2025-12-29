//! Wycheproof-derived regression tests for the TC26 GOST backend.
#![cfg(feature = "gost")]

use hex::decode as hex_decode;
use iroha_crypto::{Algorithm, PublicKey, Signature};
use norito::json::Value;

fn parse_algorithm(name: &str) -> Algorithm {
    match name {
        "Gost3410_2012_256ParamSetA" => Algorithm::Gost3410_2012_256ParamSetA,
        "Gost3410_2012_256ParamSetB" => Algorithm::Gost3410_2012_256ParamSetB,
        "Gost3410_2012_256ParamSetC" => Algorithm::Gost3410_2012_256ParamSetC,
        "Gost3410_2012_512ParamSetA" => Algorithm::Gost3410_2012_512ParamSetA,
        "Gost3410_2012_512ParamSetB" => Algorithm::Gost3410_2012_512ParamSetB,
        other => panic!("unknown algorithm in fixture: {other}"),
    }
}

fn hex_to_vec(hex: &str) -> Vec<u8> {
    hex_decode(hex).unwrap_or_else(|err| panic!("invalid hex {hex}: {err}"))
}

#[test]
fn wycheproof_gost_signatures() {
    let data = include_str!("fixtures/wycheproof_gost.json");
    let root: Value = norito::json::from_str(data).expect("parse wycheproof gost suite");
    let groups = root["testGroups"]
        .as_array()
        .expect("testGroups array missing");

    for group in groups {
        let algorithm_name = group["algorithm"].as_str().expect("algorithm name missing");
        let algorithm = parse_algorithm(algorithm_name);

        let public_hex = group["public"].as_str().expect("public key missing");
        let public_key =
            PublicKey::from_hex(algorithm, public_hex).expect("invalid public key payload");

        let tests = group["tests"].as_array().expect("tests array missing");
        for case in tests {
            let tc_id = case["tcId"].as_u64().unwrap_or_default();
            let msg_hex = case["msg"].as_str().expect("message missing");
            let sig_hex = case["sig"].as_str().expect("signature missing");
            let result = case["result"].as_str().expect("result missing");

            let message = hex_to_vec(msg_hex);
            let signature_bytes = hex_to_vec(sig_hex);
            let signature = Signature::from_bytes(&signature_bytes);

            let verification = signature.verify(&public_key, &message);
            match result {
                "valid" => {
                    assert!(
                        verification.is_ok(),
                        "tcId {tc_id} ({algorithm_name}) expected valid signature"
                    );
                }
                "invalid" => {
                    assert!(
                        verification.is_err(),
                        "tcId {tc_id} ({algorithm_name}) expected invalid signature"
                    );
                }
                other => panic!("unknown result '{other}' for tcId {tc_id}"),
            }
        }
    }
}
