//! Canonical fixed-width archive identifiers preserve decoding and failure classification.

use super::*;

fn assert_invalid_hex(raw: &str) {
    let provider = parse_provider_id(raw).expect_err("noncanonical provider identifier");
    assert_eq!(provider.code(), "MUSUBI_ARCHIVE_FETCH_PROVIDER_ID_INVALID");
    assert_eq!(
        provider.class(),
        MusubiArchiveRuntimeFailureClassV1::Permanent
    );
    assert_eq!(provider.integrity_surface(), None);

    let digest = parse_lower_hex_32(raw).expect_err("noncanonical archive digest");
    assert_eq!(digest.code(), "MUSUBI_ARCHIVE_RESPONSE_HEX_INVALID");
    assert_eq!(
        digest.class(),
        MusubiArchiveRuntimeFailureClassV1::Integrity
    );
    assert_eq!(
        digest.integrity_surface(),
        Some(MusubiArchiveRuntimeIntegritySurfaceV1::ArchiveCommitment)
    );
}

#[test]
fn fixed_hex_parsers_decode_every_byte_in_every_position() {
    for position in 0..32 {
        for byte in u8::MIN..=u8::MAX {
            let mut expected = [0; 32];
            expected[position] = byte;
            let encoded = hex::encode(expected);
            assert_eq!(parse_lower_hex_32(&encoded).unwrap(), expected);
            if byte != 0 {
                assert_eq!(parse_provider_id(&encoded).unwrap().as_bytes(), &expected);
            }
        }
    }
}

#[test]
fn fixed_hex_parsers_keep_the_provider_only_zero_rejection() {
    let encoded = "0".repeat(64);
    assert_eq!(parse_lower_hex_32(&encoded).unwrap(), [0; 32]);
    let error = parse_provider_id(&encoded).unwrap_err();
    assert_eq!(error.code(), "MUSUBI_ARCHIVE_FETCH_PROVIDER_ID_INVALID");
    assert_eq!(error.class(), MusubiArchiveRuntimeFailureClassV1::Permanent);
    assert_eq!(error.integrity_surface(), None);
}

#[test]
fn fixed_hex_parsers_reject_each_noncanonical_ascii_byte_at_every_position() {
    for position in 0..64 {
        for byte in 0..=127_u8 {
            if byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte) {
                continue;
            }
            let mut encoded = [b'1'; 64];
            encoded[position] = byte;
            assert_invalid_hex(std::str::from_utf8(&encoded).unwrap());
        }
    }
}

#[test]
fn fixed_hex_parsers_reject_lengths_and_non_ascii_without_normalizing() {
    for length in [0, 1, 31, 32, 62, 63, 65, 66, 128, 4096] {
        assert_invalid_hex(&"1".repeat(length));
    }
    for character in ['é', 'Ａ', '🦀'] {
        let mut encoded = "1".repeat(64);
        encoded.replace_range(..character.len_utf8(), &character.to_string());
        assert_eq!(encoded.len(), 64);
        assert_invalid_hex(&encoded);
    }
    let valid = "ab".repeat(32);
    for encoded in [
        format!(" {valid}"),
        format!("{valid}\n"),
        format!("0x{valid}"),
    ] {
        assert_invalid_hex(&encoded);
    }
}
