//! Sole bounded original platform-attestation container for ordinary identity.
//!
//! This data codec preserves ordered Android DERs and original Apple enrollment CBOR.
//! It performs no PKIX, `KeyDescription`, App Attest, Play, challenge/key or issuer verification.
//! Those original checks must run under independently held policy before raw314 admission.

use super::KagemushaHardwarePlatformClassV1;
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use norito::codec::{Decode, Encode};
use sha2::{Digest as _, Sha256};

/// Entire canonical original archive ceiling, including all Norito framing.
pub const KAGEMUSHA_PLATFORM_ATTESTATION_ORIGINAL_MAX_BYTES_V1: usize = 128 * 1024;
/// Existing raw Android verifier maximum ordered certificate count.
pub const KAGEMUSHA_PLATFORM_ANDROID_CERTIFICATE_MAX_COUNT_V1: usize = 8;
/// Existing raw Android verifier maximum original DER bytes per certificate.
pub const KAGEMUSHA_PLATFORM_ANDROID_CERTIFICATE_MAX_BYTES_V1: usize = 16 * 1024;
/// Existing Apple enrollment-object ceiling; distinct from an approval assertion bound.
pub const KAGEMUSHA_PLATFORM_APPLE_ATTESTATION_MAX_BYTES_V1: usize = 16 * 1024;

/// Untouched platform originals. Order and exact bytes participate in the complete digest.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    DeriveJsonDeserialize,
    DeriveJsonSerialize,
    norito::NoritoSchema,
)]
#[norito(tag = "platform", content = "original", rename_all = "snake_case")]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaPlatformAttestationEvidenceV1")]
pub enum KagemushaPlatformAttestationEvidenceV1 {
    /// Original platform leaf-to-root chain; no sorting, DER normalization or concatenation.
    AndroidKeyMint {
        /// Each exact original certificate from the existing raw verifier/collector contract.
        certificate_chain_der: Vec<Vec<u8>>,
    },
    /// Original enrollment attestation object, never an approval assertion or decoded projection.
    AppleAppAttest {
        /// Exact original CBOR, passed unchanged to the existing enrollment-object verifier.
        attestation_object_cbor: Vec<u8>,
    },
}
/// Sole first-release canonical original container; decoding grants no platform authority.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    DeriveJsonDeserialize,
    DeriveJsonSerialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaPlatformAttestationOriginalV1")]
pub struct KagemushaPlatformAttestationOriginalV1 {
    /// Sole accepted container version; no legacy raw concatenation fallback.
    pub version: u16,
    /// Exact bounded role-tagged originals; a public data value is not a checked raw result.
    pub evidence: KagemushaPlatformAttestationEvidenceV1,
}
impl KagemushaPlatformAttestationOriginalV1 {
    /// Exact platform role chosen by the sole evidence variant.
    #[must_use]
    pub const fn platform_class(&self) -> KagemushaHardwarePlatformClassV1 {
        match &self.evidence {
            KagemushaPlatformAttestationEvidenceV1::AndroidKeyMint { .. } => {
                KagemushaHardwarePlatformClassV1::AndroidKeyMint
            }
            KagemushaPlatformAttestationEvidenceV1::AppleAppAttest { .. } => {
                KagemushaHardwarePlatformClassV1::AppleAppAttest
            }
        }
    }
    /// Borrow original ordered DERs for the real raw verifier; no key/PKIX fact is granted.
    #[must_use]
    pub fn android_certificate_chain_der(&self) -> Option<&[Vec<u8>]> {
        match &self.evidence {
            KagemushaPlatformAttestationEvidenceV1::AndroidKeyMint {
                certificate_chain_der,
            } => Some(certificate_chain_der),
            _ => None,
        }
    }
    /// Borrow exact Apple enrollment object for the real shared verifier; no assertion parsing.
    #[must_use]
    pub fn apple_attestation_object_cbor(&self) -> Option<&[u8]> {
        match &self.evidence {
            KagemushaPlatformAttestationEvidenceV1::AppleAppAttest {
                attestation_object_cbor,
            } => Some(attestation_object_cbor),
            _ => None,
        }
    }
    /// Check data/resource shape only; certificate order/semantics remain raw-verifier duties.
    /// # Errors
    /// Rejects another version, empty/oversized original or chain outside existing bounds.
    pub fn validate_shape(&self) -> Result<(), String> {
        if self.version != 1 {
            return Err("platform original version differs".into());
        }
        match &self.evidence {
            KagemushaPlatformAttestationEvidenceV1::AndroidKeyMint {
                certificate_chain_der,
            } => {
                if !(2..=KAGEMUSHA_PLATFORM_ANDROID_CERTIFICATE_MAX_COUNT_V1)
                    .contains(&certificate_chain_der.len())
                    || certificate_chain_der.iter().any(|cert| {
                        cert.is_empty()
                            || cert.len() > KAGEMUSHA_PLATFORM_ANDROID_CERTIFICATE_MAX_BYTES_V1
                    })
                {
                    return Err("Android original certificate resource bounds differ".into());
                }
            }
            KagemushaPlatformAttestationEvidenceV1::AppleAppAttest {
                attestation_object_cbor,
            } => {
                if attestation_object_cbor.is_empty()
                    || attestation_object_cbor.len()
                        > KAGEMUSHA_PLATFORM_APPLE_ATTESTATION_MAX_BYTES_V1
                {
                    return Err("Apple original enrollment-object resource bound differs".into());
                }
            }
        }
        Ok(())
    }
    /// Encode the sole complete canonical original without altering any inner byte or order.
    /// # Errors
    /// Rejects shape or actual complete frame overflow; never truncates/splits an original.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        self.validate_shape()?;
        let len = norito::canonical_frame_len(self).map_err(|e| e.to_string())?;
        if len > KAGEMUSHA_PLATFORM_ATTESTATION_ORIGINAL_MAX_BYTES_V1 {
            return Err("platform original canonical archive exceeds resource bound".into());
        }
        norito::encode_canonical(self).map_err(|e| e.to_string())
    }
    /// Decode only the exact bounded canonical original, before any issuer/verifier consumption.
    /// # Errors
    /// Rejects unknown version/variant, malformed layout, noncanonical framing or trailing bytes.
    pub fn decode_canonical_exact(original: &[u8]) -> Result<Self, String> {
        if original.is_empty()
            || original.len() > KAGEMUSHA_PLATFORM_ATTESTATION_ORIGINAL_MAX_BYTES_V1
        {
            return Err("platform original archive resource bound differs".into());
        }
        let value: Self = norito::decode_canonical_with_limits(
            original,
            norito::canonical_decode_limits(original.len()),
        )
        .map_err(|e| e.to_string())?;
        if value.canonical_bytes()? != original {
            return Err("platform original archive is not canonical".into());
        }
        Ok(value)
    }
    /// SHA256 of the complete sole canonical original, including version/platform/Norito framing.
    /// This selector is not issuer, root, platform or native authority.
    /// # Errors
    /// Rejects malformed/oversized original before returning the data-only commitment.
    pub fn canonical_digest(&self) -> Result<[u8; 32], String> {
        Ok(Sha256::digest(self.canonical_bytes()?).into())
    }
}

#[cfg(test)]
pub(super) fn platform_original_fixture(apple: bool) -> KagemushaPlatformAttestationOriginalV1 {
    // Exact retained public fixture bytes: Android explicitly synthetic DERs, Apple guide sample.
    // No PKIX/current-device/governed-release success is claimed by this data codec fixture.
    KagemushaPlatformAttestationOriginalV1 {
        version: 1,
        evidence: if apple {
            KagemushaPlatformAttestationEvidenceV1::AppleAppAttest {
                attestation_object_cbor: include_bytes!("../../../../fixtures/kagemusha/platform-original-container-v1/apple-guide-original.cbor").to_vec(),
            }
        } else {
            KagemushaPlatformAttestationEvidenceV1::AndroidKeyMint {
                certificate_chain_der: vec![
                    include_bytes!("../../../../fixtures/kagemusha/platform-original-container-v1/mock_osp-0.der").to_vec(),
                    include_bytes!("../../../../fixtures/kagemusha/platform-original-container-v1/mock_osp-1.der").to_vec(),
                ],
            }
        },
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn android_exact_original_der_order_roundtrips_without_normalization() {
        let value = platform_original_fixture(false);
        let original = value.canonical_bytes().unwrap();
        println!(
            "PLATFORM_ANDROID_ORIGINAL_GOLDEN={}",
            hex::encode(&original)
        );
        let decoded =
            KagemushaPlatformAttestationOriginalV1::decode_canonical_exact(&original).unwrap();
        assert_eq!(decoded, value);
        assert_eq!(hex::encode(&original),
            include_str!("../../../../fixtures/kagemusha/platform-original-container-v1/android-mock-osp-unit.hex").trim());
        assert_eq!(
            decoded.android_certificate_chain_der().unwrap()[0],
            include_bytes!(
                "../../../../fixtures/kagemusha/platform-original-container-v1/mock_osp-0.der"
            )
        );
        assert_eq!(
            decoded.android_certificate_chain_der().unwrap()[1],
            include_bytes!(
                "../../../../fixtures/kagemusha/platform-original-container-v1/mock_osp-1.der"
            )
        );
        assert!(decoded.apple_attestation_object_cbor().is_none());
        assert_eq!(
            value.canonical_digest().unwrap(),
            Sha256::digest(&original).as_slice()
        );
    }
    #[test]
    fn apple_original_enrollment_cbor_roundtrips_unchanged_and_is_distinct_role() {
        let value = platform_original_fixture(true);
        let original = value.canonical_bytes().unwrap();
        println!("PLATFORM_APPLE_ORIGINAL_GOLDEN={}", hex::encode(&original));
        let decoded =
            KagemushaPlatformAttestationOriginalV1::decode_canonical_exact(&original).unwrap();
        assert_eq!(
            decoded.apple_attestation_object_cbor().unwrap(),
            include_bytes!(
                "../../../../fixtures/kagemusha/platform-original-container-v1/apple-guide-original.cbor"
            )
        );
        assert_eq!(
            decoded.platform_class(),
            KagemushaHardwarePlatformClassV1::AppleAppAttest
        );
        assert!(decoded.android_certificate_chain_der().is_none());
        assert_eq!(
            hex::encode(&original),
            include_str!(
                "../../../../fixtures/kagemusha/platform-original-container-v1/apple-guide-unit.hex"
            )
            .trim()
        );
        assert_ne!(
            value.canonical_digest().unwrap(),
            platform_original_fixture(false).canonical_digest().unwrap()
        );
    }
    #[test]
    fn chain_order_and_every_original_byte_bind_the_complete_digest() {
        let mut value = platform_original_fixture(false);
        let original = value.canonical_digest().unwrap();
        let KagemushaPlatformAttestationEvidenceV1::AndroidKeyMint {
            certificate_chain_der,
        } = &mut value.evidence
        else {
            panic!("Android fixture")
        };
        certificate_chain_der.reverse();
        assert_ne!(value.canonical_digest().unwrap(), original);
        let mut value = platform_original_fixture(false);
        let KagemushaPlatformAttestationEvidenceV1::AndroidKeyMint {
            certificate_chain_der,
        } = &mut value.evidence
        else {
            panic!("Android fixture")
        };
        certificate_chain_der[0][20] ^= 1;
        assert_ne!(value.canonical_digest().unwrap(), original);
    }
    #[test]
    fn exact_version_codec_tails_and_legacy_raw_concatenation_are_rejected() {
        let value = platform_original_fixture(false);
        let mut original = value.canonical_bytes().unwrap();
        original.push(0);
        assert!(KagemushaPlatformAttestationOriginalV1::decode_canonical_exact(&original).is_err());
        let mut value = value;
        value.version = 2;
        assert!(value.canonical_bytes().is_err());
        let invalid = norito::encode_canonical(&value).unwrap();
        assert!(KagemushaPlatformAttestationOriginalV1::decode_canonical_exact(&invalid).is_err());
        let mut raw = Vec::new();
        for certificate in value.android_certificate_chain_der().unwrap() {
            raw.extend(certificate);
        }
        assert!(KagemushaPlatformAttestationOriginalV1::decode_canonical_exact(&raw).is_err());
        assert!(
            KagemushaPlatformAttestationOriginalV1::decode_canonical_exact(
                platform_original_fixture(true)
                    .apple_attestation_object_cbor()
                    .unwrap()
            )
            .is_err()
        );
    }
    #[test]
    fn original_component_and_archive_limits_reject_without_trimming() {
        for chain in [
            vec![],
            vec![vec![1]],
            vec![vec![1]; 9],
            vec![vec![], vec![1]],
            vec![vec![1; 16385], vec![1]],
        ] {
            let value = KagemushaPlatformAttestationOriginalV1 {
                version: 1,
                evidence: KagemushaPlatformAttestationEvidenceV1::AndroidKeyMint {
                    certificate_chain_der: chain,
                },
            };
            assert!(value.canonical_bytes().is_err());
            let invalid = norito::encode_canonical(&value).unwrap();
            assert!(
                KagemushaPlatformAttestationOriginalV1::decode_canonical_exact(&invalid).is_err()
            );
        }
        let full = KagemushaPlatformAttestationOriginalV1 {
            version: 1,
            evidence: KagemushaPlatformAttestationEvidenceV1::AndroidKeyMint {
                certificate_chain_der: vec![vec![1; 16384]; 8],
            },
        };
        assert!(full.canonical_bytes().is_err()); // raw payload alone equals the complete archive bound.
        let oversize_archive = norito::encode_canonical(&full).unwrap();
        assert!(oversize_archive.len() > KAGEMUSHA_PLATFORM_ATTESTATION_ORIGINAL_MAX_BYTES_V1);
        assert!(
            KagemushaPlatformAttestationOriginalV1::decode_canonical_exact(&oversize_archive)
                .is_err()
        );
        let bounded_android = KagemushaPlatformAttestationOriginalV1 {
            version: 1,
            evidence: KagemushaPlatformAttestationEvidenceV1::AndroidKeyMint {
                certificate_chain_der: vec![vec![1; 16384]; 2],
            },
        };
        let bounded_apple = KagemushaPlatformAttestationOriginalV1 {
            version: 1,
            evidence: KagemushaPlatformAttestationEvidenceV1::AppleAppAttest {
                attestation_object_cbor: vec![1; 16384],
            },
        };
        for bounded in [bounded_android, bounded_apple] {
            let bytes = bounded.canonical_bytes().unwrap();
            assert_eq!(
                KagemushaPlatformAttestationOriginalV1::decode_canonical_exact(&bytes).unwrap(),
                bounded
            );
        }
        for raw in [vec![], vec![1; 16385]] {
            let value = KagemushaPlatformAttestationOriginalV1 {
                version: 1,
                evidence: KagemushaPlatformAttestationEvidenceV1::AppleAppAttest {
                    attestation_object_cbor: raw,
                },
            };
            assert!(value.canonical_bytes().is_err());
            let invalid = norito::encode_canonical(&value).unwrap();
            assert!(
                KagemushaPlatformAttestationOriginalV1::decode_canonical_exact(&invalid).is_err()
            );
        }
        assert!(
            KagemushaPlatformAttestationOriginalV1::decode_canonical_exact(&vec![
                1;
                128 * 1024 + 1
            ])
            .is_err()
        );
    }
}
