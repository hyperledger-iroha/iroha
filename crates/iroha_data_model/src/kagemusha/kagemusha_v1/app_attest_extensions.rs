//! Bounded App Attest assertion extension parsing and release binding.

use sha2::{Digest, Sha256};

const MAX_AUTHENTICATOR_DATA_BYTES: usize = 1_024;
const MAX_RAW_ASSERTION_BYTES: usize = 8_192;
const MAX_BUNDLE_VERSION_BYTES: usize = 128;
const RELEASE_DOMAIN: &[u8] = b"iroha:kagemusha:v1:app-attest-release\0";

/// A malformed assertion extension or invalid release identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AppAttestExtensionError {
    /// The signed authenticator data has the wrong shape or a disallowed value.
    Malformed,
    /// The asserted release differs from the release selected by authenticated policy.
    ReleaseMismatch,
}

/// The two signed App Attest assertion release extensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AppAttestAssertionExtensions<'a> {
    /// Apple validation category, decoded from its four-byte little-endian CBOR byte string.
    pub(crate) validation_category: u32,
    /// Exact app bundle version in the signed authenticator data.
    pub(crate) bundle_version: &'a str,
}

/// Extract the two exact byte strings from Apple's original assertion object.
///
/// Keeping the original CBOR alongside the signed authenticator bytes prevents an SDK from
/// substituting a locally reconstructed object for the evidence that the platform returned.
/// Only canonical definite-length CBOR with the two first-release keys is accepted.
pub(crate) fn parse_app_attest_assertion(
    raw_assertion: &[u8],
) -> Result<(&[u8], &[u8]), AppAttestExtensionError> {
    if raw_assertion.is_empty() || raw_assertion.len() > MAX_RAW_ASSERTION_BYTES {
        return Err(AppAttestExtensionError::Malformed);
    }
    let mut reader = CborReader::new(raw_assertion);
    if reader.length(5, 2)? != 2 {
        return Err(AppAttestExtensionError::Malformed);
    }
    let mut authenticator_data = None;
    let mut signature_der = None;
    for _ in 0..2 {
        match reader.text(32)? {
            "authenticatorData" if authenticator_data.is_none() => {
                authenticator_data = Some(reader.byte_string(MAX_AUTHENTICATOR_DATA_BYTES)?);
            }
            "signature" if signature_der.is_none() => {
                signature_der = Some(reader.byte_string(72)?);
            }
            _ => return Err(AppAttestExtensionError::Malformed),
        }
    }
    if !reader.is_at_end() {
        return Err(AppAttestExtensionError::Malformed);
    }
    let authenticator_data = authenticator_data.ok_or(AppAttestExtensionError::Malformed)?;
    let signature_der = signature_der.ok_or(AppAttestExtensionError::Malformed)?;
    if authenticator_data.len() < 38 || signature_der.len() < 8 {
        return Err(AppAttestExtensionError::Malformed);
    }
    Ok((authenticator_data, signature_der))
}

impl AppAttestAssertionExtensions<'_> {
    /// Verify the digest of both signed values against the authenticated release digest.
    pub(crate) fn verify_release_digest(
        &self,
        expected_digest: [u8; 32],
    ) -> Result<(), AppAttestExtensionError> {
        let actual =
            app_attest_release_extensions_digest(self.validation_category, self.bundle_version)?;
        if actual == expected_digest {
            Ok(())
        } else {
            Err(AppAttestExtensionError::ReleaseMismatch)
        }
    }
}

/// Parse only the signed App Attest assertion release extensions from authenticator data.
///
/// The caller must independently verify the exact original authenticator bytes under the
/// enrolled App Attest key, RP ID, and exact-next signature counter. This parser requires the
/// signed extension bytes, forbids the attested-credential flag, bounds all CBOR lengths, rejects
/// indefinite forms, duplicate/missing/unknown keys and trailing bytes, and does not guess a
/// legacy extension-free assertion format.
pub(crate) fn parse_app_attest_assertion_extensions(
    authenticator_data: &[u8],
) -> Result<AppAttestAssertionExtensions<'_>, AppAttestExtensionError> {
    if !(38..=MAX_AUTHENTICATOR_DATA_BYTES).contains(&authenticator_data.len())
        || authenticator_data[32] & 0x40 != 0
    {
        return Err(AppAttestExtensionError::Malformed);
    }
    let mut reader = CborReader::new(&authenticator_data[37..]);
    if reader.length(5, 2)? != 2 {
        return Err(AppAttestExtensionError::Malformed);
    }
    let mut validation_category = None;
    let mut bundle_version = None;
    for _ in 0..2 {
        match reader.text(32)? {
            "validationCategory" if validation_category.is_none() => {
                let bytes = reader.byte_string(4)?;
                if bytes.len() != 4 {
                    return Err(AppAttestExtensionError::Malformed);
                }
                let category = u32::from_le_bytes(
                    bytes
                        .try_into()
                        .map_err(|_| AppAttestExtensionError::Malformed)?,
                );
                if !valid_category(category) {
                    return Err(AppAttestExtensionError::Malformed);
                }
                validation_category = Some(category);
            }
            "bundleVersion" if bundle_version.is_none() => {
                let version = reader.text(MAX_BUNDLE_VERSION_BYTES)?;
                validate_version(version)?;
                bundle_version = Some(version);
            }
            _ => return Err(AppAttestExtensionError::Malformed),
        }
    }
    if !reader.is_at_end() {
        return Err(AppAttestExtensionError::Malformed);
    }
    Ok(AppAttestAssertionExtensions {
        validation_category: validation_category.ok_or(AppAttestExtensionError::Malformed)?,
        bundle_version: bundle_version.ok_or(AppAttestExtensionError::Malformed)?,
    })
}

/// SHA-256 of a fixed domain, LE category, LE UTF-8 byte length, and exact version bytes.
///
/// This digest binds assertion extension values to `app_release_digest` in authenticated
/// release policy. It must be computed from release metadata independently of the assertion.
pub(crate) fn app_attest_release_extensions_digest(
    validation_category: u32,
    bundle_version: &str,
) -> Result<[u8; 32], AppAttestExtensionError> {
    if !valid_category(validation_category) {
        return Err(AppAttestExtensionError::Malformed);
    }
    validate_version(bundle_version)?;
    let version = bundle_version.as_bytes();
    let version_length =
        u16::try_from(version.len()).map_err(|_| AppAttestExtensionError::Malformed)?;
    let mut digest = Sha256::new();
    digest.update(RELEASE_DOMAIN);
    digest.update(validation_category.to_le_bytes());
    digest.update(version_length.to_le_bytes());
    digest.update(version);
    Ok(digest.finalize().into())
}

fn valid_category(category: u32) -> bool {
    matches!(category, 1..=6 | 10)
}

fn validate_version(version: &str) -> Result<(), AppAttestExtensionError> {
    if version.is_empty()
        || version.len() > MAX_BUNDLE_VERSION_BYTES
        || version.as_bytes().contains(&0)
    {
        return Err(AppAttestExtensionError::Malformed);
    }
    Ok(())
}

struct CborReader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> CborReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn is_at_end(&self) -> bool {
        self.offset == self.bytes.len()
    }

    fn length(&mut self, major: u8, maximum: usize) -> Result<usize, AppAttestExtensionError> {
        let first = *self
            .take(1)?
            .first()
            .ok_or(AppAttestExtensionError::Malformed)?;
        if first >> 5 != major {
            return Err(AppAttestExtensionError::Malformed);
        }
        let additional = first & 0x1f;
        let value = match additional {
            0..=23 => usize::from(additional),
            24 => {
                let value = usize::from(self.take(1)?[0]);
                if value < 24 {
                    return Err(AppAttestExtensionError::Malformed);
                }
                value
            }
            25 => {
                let bytes: [u8; 2] = self
                    .take(2)?
                    .try_into()
                    .map_err(|_| AppAttestExtensionError::Malformed)?;
                let value = usize::from(u16::from_be_bytes(bytes));
                if value <= u8::MAX as usize {
                    return Err(AppAttestExtensionError::Malformed);
                }
                value
            }
            26 => {
                let bytes: [u8; 4] = self
                    .take(4)?
                    .try_into()
                    .map_err(|_| AppAttestExtensionError::Malformed)?;
                let value = usize::try_from(u32::from_be_bytes(bytes))
                    .map_err(|_| AppAttestExtensionError::Malformed)?;
                if value <= u16::MAX as usize {
                    return Err(AppAttestExtensionError::Malformed);
                }
                value
            }
            _ => return Err(AppAttestExtensionError::Malformed),
        };
        if value > maximum {
            return Err(AppAttestExtensionError::Malformed);
        }
        Ok(value)
    }

    fn byte_string(&mut self, maximum: usize) -> Result<&'a [u8], AppAttestExtensionError> {
        let length = self.length(2, maximum)?;
        self.take(length)
    }

    fn text(&mut self, maximum: usize) -> Result<&'a str, AppAttestExtensionError> {
        let length = self.length(3, maximum)?;
        core::str::from_utf8(self.take(length)?).map_err(|_| AppAttestExtensionError::Malformed)
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], AppAttestExtensionError> {
        let end = self
            .offset
            .checked_add(count)
            .filter(|end| *end <= self.bytes.len())
            .ok_or(AppAttestExtensionError::Malformed)?;
        let result = &self.bytes[self.offset..end];
        self.offset = end;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cbor_bytes(bytes: &[u8]) -> Vec<u8> {
        let mut encoded = if bytes.len() < 24 {
            vec![0x40 | u8::try_from(bytes.len()).unwrap()]
        } else {
            vec![0x58, u8::try_from(bytes.len()).unwrap()]
        };
        encoded.extend_from_slice(bytes);
        encoded
    }

    #[test]
    fn raw_assertion_requires_exact_original_two_field_map() {
        let authenticator_data = vec![0x39; 38];
        let signature = vec![0x30; 8];
        let mut raw = vec![0xa2];
        raw.extend(cbor_text("authenticatorData"));
        raw.extend(cbor_bytes(&authenticator_data));
        raw.extend(cbor_text("signature"));
        raw.extend(cbor_bytes(&signature));
        assert_eq!(
            parse_app_attest_assertion(&raw),
            Ok((authenticator_data.as_slice(), signature.as_slice()))
        );
        let mut trailing = raw.clone();
        trailing.push(0);
        assert_eq!(
            parse_app_attest_assertion(&trailing),
            Err(AppAttestExtensionError::Malformed)
        );
        let mut duplicate = vec![0xa2];
        duplicate.extend(cbor_text("authenticatorData"));
        duplicate.extend(cbor_bytes(&authenticator_data));
        duplicate.extend(cbor_text("authenticatorData"));
        duplicate.extend(cbor_bytes(&authenticator_data));
        assert_eq!(
            parse_app_attest_assertion(&duplicate),
            Err(AppAttestExtensionError::Malformed)
        );
        let mut indefinite = raw;
        indefinite[0] = 0xbf;
        assert_eq!(
            parse_app_attest_assertion(&indefinite),
            Err(AppAttestExtensionError::Malformed)
        );
        assert_eq!(
            parse_app_attest_assertion(&vec![0xa2; MAX_RAW_ASSERTION_BYTES + 1]),
            Err(AppAttestExtensionError::Malformed)
        );
    }

    fn cbor_text(text: &str) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(text.len() + 2);
        if text.len() < 24 {
            bytes.push(0x60 | u8::try_from(text.len()).unwrap());
        } else {
            bytes.extend_from_slice(&[0x78, u8::try_from(text.len()).unwrap()]);
        }
        bytes.extend_from_slice(text.as_bytes());
        bytes
    }

    fn auth_data(extensions: &[u8]) -> Vec<u8> {
        let mut bytes = vec![0x39; 32];
        bytes.push(0x81);
        bytes.extend_from_slice(&1_u32.to_be_bytes());
        bytes.extend_from_slice(extensions);
        bytes
    }

    fn valid_extensions() -> Vec<u8> {
        let mut bytes = vec![0xa2];
        bytes.extend(cbor_text("validationCategory"));
        bytes.extend_from_slice(&[0x44, 4, 0, 0, 0]);
        bytes.extend(cbor_text("bundleVersion"));
        bytes.extend(cbor_text("1.0"));
        bytes
    }

    #[test]
    fn parses_exact_signed_release_and_matches_digest() {
        let data = auth_data(&valid_extensions());
        let parsed = parse_app_attest_assertion_extensions(&data).unwrap();
        assert_eq!(parsed.validation_category, 4);
        assert_eq!(parsed.bundle_version, "1.0");
        let digest = app_attest_release_extensions_digest(4, "1.0").unwrap();
        assert_eq!(parsed.verify_release_digest(digest), Ok(()));
        assert_eq!(
            parsed.verify_release_digest(app_attest_release_extensions_digest(2, "1.0").unwrap()),
            Err(AppAttestExtensionError::ReleaseMismatch)
        );
        assert_eq!(
            digest,
            [
                0x6b, 0xba, 0x24, 0x1e, 0x1c, 0xae, 0x5b, 0xce, 0x0b, 0x29, 0xe7, 0xe2, 0xfb, 0x72,
                0x93, 0xce, 0x82, 0x65, 0x3a, 0xf8, 0xdc, 0x39, 0xc2, 0x77, 0xd1, 0x2c, 0x51, 0xc0,
                0x75, 0x0c, 0x3e, 0xc0,
            ]
        );
    }

    #[test]
    fn rejects_missing_duplicate_unknown_or_trailing_fields() {
        let valid = valid_extensions();
        for malformed in [
            vec![],
            vec![0xa0],
            vec![0xbf],
            [
                vec![0xa1],
                valid[1..(1 + cbor_text("validationCategory").len() + 5)].to_vec(),
            ]
            .concat(),
            {
                let mut duplicate = vec![0xa2];
                duplicate.extend(cbor_text("validationCategory"));
                duplicate.extend_from_slice(&[0x44, 4, 0, 0, 0]);
                duplicate.extend(cbor_text("validationCategory"));
                duplicate.extend_from_slice(&[0x44, 4, 0, 0, 0]);
                duplicate
            },
            {
                let mut unknown = valid.clone();
                unknown[1] = 0x61;
                unknown
            },
            {
                let mut trailing = valid.clone();
                trailing.push(0);
                trailing
            },
        ] {
            assert_eq!(
                parse_app_attest_assertion_extensions(&auth_data(&malformed)),
                Err(AppAttestExtensionError::Malformed)
            );
        }
    }

    #[test]
    fn rejects_invalid_types_sizes_categories_flags_and_versions() {
        let valid = valid_extensions();
        let category_marker = 1 + cbor_text("validationCategory").len();
        for replacement in [
            vec![0x04, 4, 0, 0, 0],
            vec![0x43, 4, 0, 0, 0],
            vec![0x44, 0, 0, 0, 0],
            vec![0x44, 7, 0, 0, 0],
            vec![0x44, 0, 0, 0, 4],
            vec![0x5f, 4, 0, 0, 0],
        ] {
            let mut malformed = valid.clone();
            malformed.splice(category_marker..category_marker + 5, replacement);
            assert_eq!(
                parse_app_attest_assertion_extensions(&auth_data(&malformed)),
                Err(AppAttestExtensionError::Malformed)
            );
        }
        let mut signed_suffix_without_extension_flag = auth_data(&valid);
        signed_suffix_without_extension_flag[32] = 0x01;
        assert_eq!(
            parse_app_attest_assertion_extensions(&signed_suffix_without_extension_flag),
            parse_app_attest_assertion_extensions(&auth_data(&valid))
        );
        let mut credential_flag = auth_data(&valid);
        credential_flag[32] = 0xc1;
        assert!(parse_app_attest_assertion_extensions(&credential_flag).is_err());
        let mut bad_utf8 = valid.clone();
        let last = bad_utf8.len() - 1;
        bad_utf8[last] = 0xff;
        assert!(parse_app_attest_assertion_extensions(&auth_data(&bad_utf8)).is_err());
        let mut truncated = valid.clone();
        truncated.pop();
        assert!(parse_app_attest_assertion_extensions(&auth_data(&truncated)).is_err());
        let too_large = vec![0; MAX_AUTHENTICATOR_DATA_BYTES + 1];
        assert!(parse_app_attest_assertion_extensions(&too_large).is_err());
        assert!(app_attest_release_extensions_digest(0, "1.0").is_err());
        assert!(app_attest_release_extensions_digest(4, "").is_err());
        assert!(app_attest_release_extensions_digest(4, &"1".repeat(129)).is_err());
    }
}
