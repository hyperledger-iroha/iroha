//! Source-parser and processed-key preflight regression coverage.
use super::{
    KagemushaConfiguredVkWireShapeV4, KagemushaPkWirePreflightV4, KagemushaVkWirePreflightV4,
    KagemushaWireScannerV4, ensure_kagemusha_pk_preflight_matches_vk_v4,
    preflight_kagemusha_polynomial_v4, preflight_kagemusha_polynomial_vec_v4,
    preflight_kagemusha_processed_vk_v4,
};
use sha2::{Digest as _, Sha256};
use std::io::Cursor;
fn shape() -> KagemushaConfiguredVkWireShapeV4 {
    KagemushaConfiguredVkWireShapeV4 {
        k: 8,
        domain_size: 256,
        advice_columns: 7,
        base_fixed_columns: 2,
        selectors: 3,
        permutation_columns: 4,
        instance_columns: 1,
        curve_bytes: 32,
        scalar_bytes: 32,
    }
}
fn uncompressed_vk_bytes() -> Vec<u8> {
    let shape = shape();
    let fixed_columns = shape.base_fixed_columns + shape.selectors;
    let mut bytes = vec![0x02];
    bytes.extend_from_slice(&shape.k.to_le_bytes());
    bytes.push(0);
    bytes.extend_from_slice(&(fixed_columns as u32).to_le_bytes());
    bytes.resize(
        bytes.len() + (fixed_columns + shape.permutation_columns) * shape.curve_bytes,
        0,
    );
    bytes
}
#[test]
fn verifier_key_preflight_rejects_malformed_k_count_and_length() {
    let valid = uncompressed_vk_bytes();
    let mut cursor = Cursor::new(valid.as_slice());
    let mut scanner = KagemushaWireScannerV4::new(&mut cursor, "test VK");
    let parsed =
        preflight_kagemusha_processed_vk_v4(&mut scanner, shape(), Some(valid.len() as u64))
            .expect("configured VK wire");
    assert_eq!(parsed.fixed_columns, 5);
    let expected_digest: [u8; 32] = Sha256::digest(&valid).into();
    assert_eq!(
        scanner
            .finish_consumed_sha256()
            .expect("first digest finalization"),
        expected_digest
    );
    let mut bad_k = valid.clone();
    bad_k[1..5].copy_from_slice(&31_u32.to_le_bytes());
    let mut cursor = Cursor::new(bad_k.as_slice());
    let mut scanner = KagemushaWireScannerV4::new(&mut cursor, "bad-k VK");
    assert!(
        preflight_kagemusha_processed_vk_v4(&mut scanner, shape(), Some(bad_k.len() as u64),)
            .is_err()
    );
    let mut bad_count = valid.clone();
    bad_count[6..10].copy_from_slice(&u32::MAX.to_le_bytes());
    let mut cursor = Cursor::new(bad_count.as_slice());
    let mut scanner = KagemushaWireScannerV4::new(&mut cursor, "bad-count VK");
    assert!(
        preflight_kagemusha_processed_vk_v4(&mut scanner, shape(), Some(bad_count.len() as u64),)
            .is_err(),
        "attacker count must fail before any count-sized allocation"
    );
    let mut trailing = valid.clone();
    trailing.push(0);
    let mut cursor = Cursor::new(trailing.as_slice());
    let mut scanner = KagemushaWireScannerV4::new(&mut cursor, "trailing VK");
    assert!(
        preflight_kagemusha_processed_vk_v4(&mut scanner, shape(), Some(trailing.len() as u64),)
            .is_err()
    );
}
#[test]
fn proving_key_preflight_rejects_polynomial_length_and_vector_count_before_allocation() {
    let mut malicious_polynomial = Cursor::new(u32::MAX.to_be_bytes());
    let mut scanner =
        KagemushaWireScannerV4::new(&mut malicious_polynomial, "malicious polynomial");
    assert!(preflight_kagemusha_polynomial_v4(&mut scanner, 256, 32).is_err());
    assert_eq!(scanner.consumed, 4);
    let mut malicious_count = Cursor::new(u32::MAX.to_be_bytes());
    let mut scanner =
        KagemushaWireScannerV4::new(&mut malicious_count, "malicious polynomial vector");
    assert!(preflight_kagemusha_polynomial_vec_v4(&mut scanner, 4, 256, 32).is_err());
    assert_eq!(scanner.consumed, 4);
}
#[test]
fn wire_scanner_stops_hashing_after_digest_finalization() {
    let bytes = [1_u8, 2, 3, 4, 5, 6, 7, 8];
    let mut cursor = Cursor::new(bytes.as_slice());
    let mut scanner = KagemushaWireScannerV4::new(&mut cursor, "digest boundary");
    assert_eq!(
        scanner.read_array::<4>().expect("digest prefix"),
        [1, 2, 3, 4]
    );
    let expected_prefix_digest: [u8; 32] = Sha256::digest([1_u8, 2, 3, 4]).into();
    assert_eq!(
        scanner
            .finish_consumed_sha256()
            .expect("first digest finalization"),
        expected_prefix_digest
    );
    scanner.skip_exact(4).expect("unhashed suffix scan");
    assert_eq!(scanner.consumed, bytes.len() as u64);
    assert!(scanner.finish_consumed_sha256().is_err());
}
#[test]
fn proving_key_preflight_binds_the_exact_standalone_verifier_key() {
    let expected = [0x51; 32];
    let preflight = KagemushaPkWirePreflightV4 {
        vk: KagemushaVkWirePreflightV4 {
            serialized_len: 20_362,
            fixed_columns: 339,
            permutation_columns: 297,
        },
        embedded_verifying_key_sha256: expected,
    };
    ensure_kagemusha_pk_preflight_matches_vk_v4(&preflight, expected, "Eq")
        .expect("exact embedded verifier identity");
    assert!(
        ensure_kagemusha_pk_preflight_matches_vk_v4(&preflight, [0x52; 32], "Eq")
            .expect_err("substituted standalone verifier must fail")
            .contains("different verifier key")
    );
}
