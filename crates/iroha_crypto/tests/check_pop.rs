//! Test PoP verification.
//! Requires `--features bls`.

#![cfg(feature = "bls")]

use iroha_crypto::{PublicKey, bls_normal_pop_verify};
use std::str::FromStr;

#[test]
fn check_all_pops() {
    let peers = vec![
        (
            "ea0130999C999F728B0829387F4E93732EE0479F911DE0CE1E9409C8CEA66CF99376F57DB2E709892648F222D9F4E90DB29B84",
            "afcaaf39164b7ce18a311e28efec542e2df6b32158331d2443ba9617ff8b9a7fb2fbd79d93f83b610542090a2bae89f701aff035b911cdeb3a60bdb4227a25b6c452a6664077a1bab551e537c8faa289a3031ee8dccd463e3f09344fa72fb0ae",
        ),
        (
            "ea0130A947D2F40023034C1514F07182B201E744A218D7082434474806F16BF9365761B31997709D21BB0BA8F275936D1926CC",
            "b44ab74f8a27c956fa51cb8c7ebafcf2255fe750373720bf29c0a6636a46be654d959c0b3d667989ef8c52da79d8a838062836830bb3e00b68ceb8b0d8a6235c68fb37a77257bc3ed1cb6f853e223c13a63ee0653f71b9cf6d7a89bf27778a20",
        ),
        (
            "ea0130841A9B897EF137FCF265621293E262AFEF741BCAB8EBE5FE2933AC4EF43E115C6DE6843EDA328393165BEE39D8C5BAB0",
            "aca452832166acc649051eb44a77cb445c8dd5eae349c478ee8d90ea883ddd1ae28b5510536a25f61ddd990e8ea15939063ade254d5ec98464139ebea7507b493df6c2311cc0d92d79f7571779e4fb773f57e6af605a508515cfe800b1e253ea",
        ),
        (
            "ea013083128161698668C68AC2BFE35E5FA5B6494ED1937A9C8DD5DDE8B2A666052EC13CAAD31AB50D6C74DB477CAA5021F5D4",
            "ab81a6465485581900f96925840fccd9345010f8ece130a4a547af2ceea1ed9fabe9c1fbd1d15be3378a777269673d000dc6a7fd37d7d2d3cf75ea8c9ba27ada8e5c2b95629b3fac7f05059f99609c389f49d335a083c050ad4cb3bf8a2bb3db",
        ),
    ];

    for (i, (pub_hex, pop_hex)) in peers.iter().enumerate() {
        let pub_key = PublicKey::from_str(pub_hex).expect("parse pub");
        let pop = hex::decode(pop_hex).expect("parse pop");
        bls_normal_pop_verify(&pub_key, &pop)
            .unwrap_or_else(|e| panic!("PoP verification failed for peer {}: {:?}", i, e));
    }
}
