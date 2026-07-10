//! Cross-language regression: the Kotlin SDK's Norito encoder must produce a
//! `KagemushaRecursiveSpendTopUpRequestV1` archive that the Rust decoder accepts,
//! exactly as the Torii offline issuer does (`norito::decode_from_bytes`) when
//! validating a wallet top-up request.
//!
//! The fixture `fixtures/kagemusha_topup_request_kotlin.bin` is produced by the
//! Kotlin test `kotlin top-up archive matches the shared Rust fixture` in
//! `KagemushaRecursiveSpendRequestCodecsTest`; regenerate it there if the encoder
//! or its sample inputs change.

use iroha_data_model::offline::KagemushaRecursiveSpendTopUpRequestV1;

#[test]
fn kotlin_topup_archive_decodes_in_rust() {
    let bytes = include_bytes!("fixtures/kagemusha_topup_request_kotlin.bin");
    if let Err(err) = norito::decode_from_bytes::<KagemushaRecursiveSpendTopUpRequestV1>(bytes) {
        panic!("Kotlin-encoded top-up archive rejected by Rust decoder: {err:?}");
    }
}
