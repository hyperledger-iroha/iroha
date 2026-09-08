//! Fixed-header rejection of unqualified compact artifacts at legacy ingress.
//!
//! The only successful branch delegates to the caller's unchanged strict legacy
//! decoder. A recognized compact schema is rejected before body decoding, CRC
//! work or proof hashing. Header classification is untrusted routing data and
//! cannot authenticate a statement, qualify a profile or authorize a result.

use iroha_data_model::fastpq::{
    FASTPQ_AXT_COMPACT_ARTIFACT_V1_SCHEMA_NAME, FASTPQ_ORDINARY_COMPACT_ARTIFACT_V1_SCHEMA_NAME,
};

use crate::{Error, Result};

/// Enforce the raw ceiling, reject compact schemas, then use one legacy decoder.
///
/// Invalid headers and unknown schemas deliberately reach the original decoder
/// so its exact framing, schema, layout, checksum and error policy is retained.
/// No body field, including an advertised profile, is read to reject a compact
/// artifact. There is no accepted compact branch or decoder-probing fallback.
pub(crate) fn decode_legacy_payload<T>(
    encoded: &[u8],
    max_bytes: usize,
    limit: &'static str,
    legacy_decode: impl FnOnce(&[u8]) -> Result<T>,
) -> Result<T> {
    if encoded.len() > max_bytes {
        return Err(Error::VerifierLimitExceeded {
            limit,
            actual: encoded.len(),
            max: max_bytes,
        });
    }
    // Header::read consumes only the fixed 40-byte header. It does not follow
    // the advertised body length or verify the checksum. Schema names are static.
    // TODO: Admit a compact route only after its exact profile, bounded codec,
    // transcript security and authenticated statement source are qualified.
    if let Ok(header) = norito::core::Header::read(encoded) {
        for schema in [
            FASTPQ_ORDINARY_COMPACT_ARTIFACT_V1_SCHEMA_NAME,
            FASTPQ_AXT_COMPACT_ARTIFACT_V1_SCHEMA_NAME,
        ] {
            if header.schema == norito::core::schema_hash_for_name(schema) {
                return Err(Error::UnqualifiedCompactArtifact { schema });
            }
        }
    }
    legacy_decode(encoded)
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use iroha_data_model::fastpq::{FastpqAxtCompactArtifactV1, FastpqOrdinaryCompactArtifactV1};
    use norito::{NoritoDeserialize, NoritoSerialize};

    use super::*;

    #[derive(Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
    #[norito(schema_name = "fastpq_prover::artifact_dispatch::LegacyFixtureV1")]
    struct LegacyFixture {
        number: u64,
        bytes: Vec<u8>,
    }

    fn legacy_decode(encoded: &[u8]) -> Result<LegacyFixture> {
        let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
        norito::decode_from_bytes(encoded).map_err(Error::Encode)
    }

    fn header(schema: &str) -> Vec<u8> {
        let mut encoded = norito::encode_canonical(&0_u8).unwrap();
        encoded[6..22].copy_from_slice(&norito::core::schema_hash_for_name(schema));
        encoded.truncate(norito::core::Header::SIZE);
        encoded
    }

    #[test]
    fn compact_dispatch_uses_distinct_exact_model_schema_identities() {
        let ordinary = <FastpqOrdinaryCompactArtifactV1 as NoritoSerialize>::schema_hash();
        let axt = <FastpqAxtCompactArtifactV1 as NoritoSerialize>::schema_hash();
        assert_eq!(
            ordinary,
            norito::core::schema_hash_for_name(FASTPQ_ORDINARY_COMPACT_ARTIFACT_V1_SCHEMA_NAME)
        );
        assert_eq!(
            axt,
            norito::core::schema_hash_for_name(FASTPQ_AXT_COMPACT_ARTIFACT_V1_SCHEMA_NAME)
        );
        assert_ne!(ordinary, axt);
        let legacy = <crate::axt_binding::AxtFastpqProofPayload as NoritoSerialize>::schema_hash();
        assert_ne!(legacy, ordinary);
        assert_ne!(legacy, axt);
    }

    #[test]
    fn compact_dispatch_rejects_header_only_and_hostile_bodies_before_decoder() {
        for schema in [
            FASTPQ_ORDINARY_COMPACT_ARTIFACT_V1_SCHEMA_NAME,
            FASTPQ_AXT_COMPACT_ARTIFACT_V1_SCHEMA_NAME,
        ] {
            for body_bytes in [0, 1, 128, 4096] {
                let mut encoded = header(schema);
                // Valid fixed fields advertise impossible lengths and a wrong
                // CRC. Neither field may make this branch inspect the body.
                encoded[23..31].copy_from_slice(&u64::MAX.to_le_bytes());
                encoded[31..39].copy_from_slice(&u64::MAX.to_le_bytes());
                encoded.resize(encoded.len() + body_bytes, 0xff);
                let decoded = Cell::new(false);
                let result: Result<()> =
                    decode_legacy_payload(&encoded, encoded.len(), "test_artifact_bytes", |_| {
                        decoded.set(true);
                        panic!("unqualified artifact reached legacy body decode/proof work")
                    });
                assert!(
                    matches!(result, Err(Error::UnqualifiedCompactArtifact { schema: actual }) if actual == schema)
                );
                assert!(!decoded.get());
            }
        }
    }

    #[test]
    fn compact_dispatch_invalid_compact_headers_keep_legacy_error_policy() {
        for schema in [
            FASTPQ_ORDINARY_COMPACT_ARTIFACT_V1_SCHEMA_NAME,
            FASTPQ_AXT_COMPACT_ARTIFACT_V1_SCHEMA_NAME,
        ] {
            let canonical_header = header(schema);
            let mut cases = Vec::new();
            for length in [0, 5, 21, norito::core::Header::SIZE - 1] {
                cases.push(canonical_header[..length].to_vec());
            }
            for offset in [0, 4, 5, 22, 39] {
                let mut malformed = canonical_header.clone();
                malformed[offset] ^= 0x80;
                cases.push(malformed);
            }
            for encoded in &cases {
                assert!(norito::core::Header::read(encoded.as_slice()).is_err());
                let expected = legacy_decode(encoded);
                assert!(expected.is_err());
                let decoded = Cell::new(false);
                let actual =
                    decode_legacy_payload(encoded, encoded.len(), "test_artifact_bytes", |bytes| {
                        decoded.set(true);
                        legacy_decode(bytes)
                    });
                assert!(decoded.get());
                assert_eq!(format!("{actual:?}"), format!("{expected:?}"));
            }
        }
    }

    #[test]
    fn compact_dispatch_raw_limit_precedes_any_header_or_decoder_work() {
        for encoded in [
            vec![0xff],
            header(FASTPQ_ORDINARY_COMPACT_ARTIFACT_V1_SCHEMA_NAME),
            header(FASTPQ_AXT_COMPACT_ARTIFACT_V1_SCHEMA_NAME),
        ] {
            let result: Result<()> =
                decode_legacy_payload(&encoded, encoded.len() - 1, "test_artifact_bytes", |_| {
                    panic!("raw over-limit input reached the legacy decoder")
                });
            assert!(matches!(result, Err(Error::VerifierLimitExceeded {
                limit: "test_artifact_bytes", actual, max,
            }) if actual == encoded.len() && max == encoded.len() - 1));
        }
        let decoded = Cell::new(false);
        let result = decode_legacy_payload(&[], 0, "test_artifact_bytes", |bytes| {
            decoded.set(true);
            legacy_decode(bytes)
        });
        assert!(matches!(result, Err(Error::Encode(_))));
        assert!(decoded.get());
    }

    #[test]
    fn compact_dispatch_preserves_the_exact_legacy_decoder_and_layout_state() {
        let fixture = LegacyFixture {
            number: u64::MAX,
            bytes: vec![0, 1, 2, 0xff],
        };
        let canonical = norito::encode_canonical(&fixture).unwrap();
        let mut cases = vec![canonical.clone(), Vec::new(), vec![0xff]];
        for length in [0, 5, 21, norito::core::Header::SIZE - 1] {
            cases.push(canonical[..length].to_vec());
        }
        for offset in [0, 4, 5, 22, 23, 31, 39] {
            let mut changed = canonical.clone();
            changed[offset] ^= 0x80;
            cases.push(changed);
        }
        let mut unknown = canonical.clone();
        unknown[6..22].copy_from_slice(&norito::core::schema_hash_for_name("unknown:artifact:v1"));
        assert!(matches!(
            legacy_decode(&unknown),
            Err(Error::Encode(norito::Error::SchemaMismatch))
        ));
        cases.push(unknown);
        let mut trailing = canonical.clone();
        trailing.push(0);
        cases.push(trailing);
        for flags in
            (u8::MIN..=u8::MAX).filter(|&flags| norito::core::validate_header_flags(flags).is_ok())
        {
            let _ambient = norito::core::DecodeFlagsGuard::enter(flags);
            let alternate = norito::to_bytes(&fixture).unwrap();
            for encoded in cases.iter().chain([&alternate]) {
                let expected = legacy_decode(encoded);
                let decoded = Cell::new(false);
                let actual =
                    decode_legacy_payload(encoded, encoded.len(), "test_artifact_bytes", |bytes| {
                        decoded.set(true);
                        assert_eq!(bytes.as_ptr(), encoded.as_ptr());
                        legacy_decode(bytes)
                    });
                assert!(decoded.get());
                assert_eq!(format!("{actual:?}"), format!("{expected:?}"));
                assert_eq!(norito::core::get_decode_flags(), flags);
            }
        }
        assert_eq!(
            decode_legacy_payload(
                &canonical,
                canonical.len(),
                "test_artifact_bytes",
                legacy_decode
            )
            .unwrap(),
            fixture
        );
    }
}
