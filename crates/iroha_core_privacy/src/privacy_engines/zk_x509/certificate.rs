//! Authoritative closed-profile certificate and CRL parsing shared by holder preflight and the reference relation.
//!
//! DER byte fields borrow the exact input; parsed EKU and CRL lists retain their
//! existing bounded Vec allocations. Structural and encoding checks do not
//! authenticate signatures, path linkage, governance records, or revocation.
use super::{
    der::{
        ZK_X509_ECDSA_WITH_SHA256_ALGORITHM_IDENTIFIER_DER_V1,
        ZK_X509_P256_PUBLIC_KEY_ALGORITHM_IDENTIFIER_DER_V1, ZkX509DerLimitsV1, ZkX509DerValueV1,
        parse_single_der_value_v1, validate_ecdsa_with_sha256_algorithm_identifier_v1,
        validate_p256_public_key_algorithm_identifier_v1,
    },
    errors::{ZkX509DerClassV1, ZkX509DerTagV1, ZkX509RelationErrorV1},
    profile::{
        ZK_X509_CLIENT_AUTHENTICATION_EKU_OID_V1, ZK_X509_DOCUMENT_SIGNING_EKU_DER_VALUE_V1,
        ZK_X509_MAX_ATTRIBUTE_VALUE_BYTES_V1, ZK_X509_MAX_CRL_ENTRIES_V1,
        ZK_X509_MAX_SERIAL_BYTES_V1, ZK_X509_UNCOMPRESSED_P256_BYTES_V1,
        ZK_X509_WALLET_IDENTITY_EKU_DER_VALUE_V1,
    },
};
use iroha_data_model::privacy::PrivacyX509ExtendedKeyUsageV1;
use p256::ecdsa::{Signature as P256Signature, VerifyingKey as P256VerifyingKey};
use time::{Date, Month, PrimitiveDateTime, Time};
pub(super) const OID_AUTHORITY_KEY_IDENTIFIER: &[u8] = &[0x55, 0x1d, 0x23];
pub(super) const OID_SUBJECT_KEY_IDENTIFIER: &[u8] = &[0x55, 0x1d, 0x0e];
pub(super) const OID_KEY_USAGE: &[u8] = &[0x55, 0x1d, 0x0f];
pub(super) const OID_BASIC_CONSTRAINTS: &[u8] = &[0x55, 0x1d, 0x13];
pub(super) const OID_EXTENDED_KEY_USAGE: &[u8] = &[0x55, 0x1d, 0x25];
pub(super) const OID_CRL_NUMBER: &[u8] = &[0x55, 0x1d, 0x14];
pub(super) const OID_COUNTRY_NAME: &[u8] = &[0x55, 0x04, 0x06];
pub(super) const OID_ORGANIZATION_NAME: &[u8] = &[0x55, 0x04, 0x0a];
pub(super) const OID_ORGANIZATIONAL_UNIT_NAME: &[u8] = &[0x55, 0x04, 0x0b];
pub(super) const OID_COMMON_NAME: &[u8] = &[0x55, 0x04, 0x03];
pub(super) const OID_CLIENT_AUTHENTICATION: &[u8] =
    &[0x2b, 0x06, 0x01, 0x05, 0x05, 0x07, 0x03, 0x02];
#[derive(Clone, Copy)]
// Keep the complete strict-parser evidence for the reference relation. Ordinary
// holder preflight reads only the signed interval fields; it still validates
// the same full profile. Reference builds retain dead-field lint coverage.
#[cfg_attr(not(any(test, feature = "privacy-release-evidence")), allow(dead_code))]
pub(super) struct ParsedNameV1<'a> {
    pub(super) encoded: &'a [u8],
    pub(super) attributes: [Option<&'a [u8]>; 4],
}
#[derive(Clone, Copy)]
// Keep the complete strict-parser evidence for the reference relation. Ordinary
// holder preflight reads only the signed interval fields; it still validates
// the same full profile. Reference builds retain dead-field lint coverage.
#[cfg_attr(not(any(test, feature = "privacy-release-evidence")), allow(dead_code))]
pub(super) struct BasicConstraintsV1 {
    pub(super) ca: bool,
    pub(super) path_len: Option<u32>,
}
#[derive(Clone)]
// Keep the complete strict-parser evidence for the reference relation. Ordinary
// holder preflight reads only the signed interval fields; it still validates
// the same full profile. Reference builds retain dead-field lint coverage.
#[cfg_attr(not(any(test, feature = "privacy-release-evidence")), allow(dead_code))]
pub(super) struct CertificateExtensionsV1<'a> {
    pub(super) authority_key_identifier: &'a [u8],
    pub(super) subject_key_identifier: &'a [u8],
    pub(super) basic_constraints: BasicConstraintsV1,
    pub(super) key_usage: u16,
    pub(super) extended_key_usages: Option<Vec<PrivacyX509ExtendedKeyUsageV1>>,
}
#[derive(Clone)]
// Keep the complete strict-parser evidence for the reference relation. Ordinary
// holder preflight reads only the signed interval fields; it still validates
// the same full profile. Reference builds retain dead-field lint coverage.
#[cfg_attr(not(any(test, feature = "privacy-release-evidence")), allow(dead_code))]
pub(super) struct ParsedCertificateV1<'a> {
    pub(super) tbs_der: &'a [u8],
    pub(super) serial: &'a [u8],
    pub(super) issuer: ParsedNameV1<'a>,
    pub(super) subject: ParsedNameV1<'a>,
    pub(super) not_before: u64,
    pub(super) not_after: u64,
    pub(super) spki_der: &'a [u8],
    pub(super) public_key: &'a [u8],
    pub(super) signature_der: &'a [u8],
    pub(super) extensions: CertificateExtensionsV1<'a>,
}
pub(super) fn parse_certificate_v1(
    der: &[u8],
) -> Result<ParsedCertificateV1<'_>, ZkX509RelationErrorV1> {
    let limits = ZkX509DerLimitsV1::profile();
    let certificate =
        parse_single_der_value_v1(der, limits)?.require_tag(ZkX509DerTagV1::SEQUENCE)?;
    let mut outer = certificate.children(limits)?;
    let tbs = outer.read_value()?.require_tag(ZkX509DerTagV1::SEQUENCE)?;
    let outer_algorithm = outer.read_value()?;
    if outer_algorithm.encoded() != ZK_X509_ECDSA_WITH_SHA256_ALGORITHM_IDENTIFIER_DER_V1 {
        return Err(ZkX509RelationErrorV1::InvalidCertificateStructure);
    }
    validate_ecdsa_with_sha256_algorithm_identifier_v1(outer_algorithm.encoded(), limits)?;
    let signature_value = outer.read_value()?.as_bit_string()?;
    if !outer.is_empty() || signature_value.unused_bits() != 0 {
        return Err(ZkX509RelationErrorV1::InvalidCertificateStructure);
    }
    validate_ecdsa_signature_der_v1(signature_value.bytes())?;
    let mut fields = tbs.children(limits)?;
    let version = fields.read_value()?;
    if version.tag() != context_tag_v1(0, true) || version.contents() != [0x02, 0x01, 0x02] {
        return Err(ZkX509RelationErrorV1::InvalidCertificateStructure);
    }
    let serial = fields
        .read_value()?
        .as_integer()?
        .positive_unsigned(ZK_X509_MAX_SERIAL_BYTES_V1)?
        .bytes();
    let tbs_algorithm = fields.read_value()?;
    if tbs_algorithm.encoded() != outer_algorithm.encoded() {
        return Err(ZkX509RelationErrorV1::InvalidCertificateStructure);
    }
    let issuer_value = fields.read_value()?.require_tag(ZkX509DerTagV1::SEQUENCE)?;
    let issuer = parse_name_v1(issuer_value)?;
    let validity = fields.read_value()?.require_tag(ZkX509DerTagV1::SEQUENCE)?;
    let (not_before, not_after) = parse_validity_v1(validity)?;
    if not_after < not_before {
        return Err(ZkX509RelationErrorV1::InvalidTime);
    }
    let subject_value = fields.read_value()?.require_tag(ZkX509DerTagV1::SEQUENCE)?;
    let subject = parse_name_v1(subject_value)?;
    let spki = fields.read_value()?.require_tag(ZkX509DerTagV1::SEQUENCE)?;
    let (spki_der, public_key) = parse_spki_v1(spki)?;
    let extension_wrapper = fields.read_value()?;
    if extension_wrapper.tag() != context_tag_v1(3, true) || !fields.is_empty() {
        return Err(ZkX509RelationErrorV1::InvalidCertificateStructure);
    }
    let extensions = parse_certificate_extensions_v1(extension_wrapper.contents())?;
    Ok(ParsedCertificateV1 {
        tbs_der: tbs.encoded(),
        serial,
        issuer,
        subject,
        not_before,
        not_after,
        spki_der,
        public_key,
        signature_der: signature_value.bytes(),
        extensions,
    })
}
pub(super) fn parse_spki_v1(
    spki: ZkX509DerValueV1<'_>,
) -> Result<(&[u8], &[u8]), ZkX509RelationErrorV1> {
    let limits = ZkX509DerLimitsV1::profile();
    let mut fields = spki.children(limits)?;
    let algorithm = fields.read_value()?;
    if algorithm.encoded() != ZK_X509_P256_PUBLIC_KEY_ALGORITHM_IDENTIFIER_DER_V1 {
        return Err(ZkX509RelationErrorV1::InvalidPublicKey);
    }
    validate_p256_public_key_algorithm_identifier_v1(algorithm.encoded(), limits)?;
    let key = fields.read_value()?.as_bit_string()?;
    if !fields.is_empty()
        || key.unused_bits() != 0
        || key.bytes().len() != ZK_X509_UNCOMPRESSED_P256_BYTES_V1
        || key.bytes().first() != Some(&0x04)
        || P256VerifyingKey::from_sec1_bytes(key.bytes()).is_err()
    {
        return Err(ZkX509RelationErrorV1::InvalidPublicKey);
    }
    Ok((spki.encoded(), key.bytes()))
}
pub(super) fn parse_validity_v1(
    validity: ZkX509DerValueV1<'_>,
) -> Result<(u64, u64), ZkX509RelationErrorV1> {
    let mut fields = validity.children(ZkX509DerLimitsV1::profile())?;
    let not_before = parse_time_v1(fields.read_value()?)?;
    let not_after = parse_time_v1(fields.read_value()?)?;
    if !fields.is_empty() {
        return Err(ZkX509RelationErrorV1::InvalidTime);
    }
    Ok((not_before, not_after))
}
pub(super) fn parse_time_v1(value: ZkX509DerValueV1<'_>) -> Result<u64, ZkX509RelationErrorV1> {
    let (year, offset) = match value.tag() {
        tag if tag == universal_primitive_tag_v1(23) => {
            if value.contents().len() != 13 || value.contents()[12] != b'Z' {
                return Err(ZkX509RelationErrorV1::InvalidTime);
            }
            let short = parse_decimal_v1(&value.contents()[..2])?;
            let year = if short >= 50 {
                1900 + i32::from(short)
            } else {
                2000 + i32::from(short)
            };
            if !(1970..=2049).contains(&year) {
                return Err(ZkX509RelationErrorV1::InvalidTime);
            }
            (year, 2)
        }
        tag if tag == universal_primitive_tag_v1(24) => {
            if value.contents().len() != 15 || value.contents()[14] != b'Z' {
                return Err(ZkX509RelationErrorV1::InvalidTime);
            }
            let year = i32::from(parse_decimal_v1(&value.contents()[..4])?);
            if !(2050..=9999).contains(&year) {
                return Err(ZkX509RelationErrorV1::InvalidTime);
            }
            (year, 4)
        }
        _ => return Err(ZkX509RelationErrorV1::InvalidTime),
    };
    let bytes = value.contents();
    let month = u8::try_from(parse_decimal_v1(&bytes[offset..offset + 2])?)
        .map_err(|_| ZkX509RelationErrorV1::InvalidTime)?;
    let day = u8::try_from(parse_decimal_v1(&bytes[offset + 2..offset + 4])?)
        .map_err(|_| ZkX509RelationErrorV1::InvalidTime)?;
    let hour = u8::try_from(parse_decimal_v1(&bytes[offset + 4..offset + 6])?)
        .map_err(|_| ZkX509RelationErrorV1::InvalidTime)?;
    let minute = u8::try_from(parse_decimal_v1(&bytes[offset + 6..offset + 8])?)
        .map_err(|_| ZkX509RelationErrorV1::InvalidTime)?;
    let second = u8::try_from(parse_decimal_v1(&bytes[offset + 8..offset + 10])?)
        .map_err(|_| ZkX509RelationErrorV1::InvalidTime)?;
    let month = Month::try_from(month).map_err(|_| ZkX509RelationErrorV1::InvalidTime)?;
    let date = Date::from_calendar_date(year, month, day)
        .map_err(|_| ZkX509RelationErrorV1::InvalidTime)?;
    let time =
        Time::from_hms(hour, minute, second).map_err(|_| ZkX509RelationErrorV1::InvalidTime)?;
    u64::try_from(
        PrimitiveDateTime::new(date, time)
            .assume_utc()
            .unix_timestamp(),
    )
    .map_err(|_| ZkX509RelationErrorV1::InvalidTime)
}
pub(super) fn parse_decimal_v1(bytes: &[u8]) -> Result<u16, ZkX509RelationErrorV1> {
    let mut value = 0_u16;
    for byte in bytes {
        if !byte.is_ascii_digit() {
            return Err(ZkX509RelationErrorV1::InvalidTime);
        }
        value = value
            .checked_mul(10)
            .and_then(|value| value.checked_add(u16::from(*byte - b'0')))
            .ok_or(ZkX509RelationErrorV1::InvalidTime)?;
    }
    Ok(value)
}
pub(super) fn parse_name_v1(
    name: ZkX509DerValueV1<'_>,
) -> Result<ParsedNameV1<'_>, ZkX509RelationErrorV1> {
    let limits = ZkX509DerLimitsV1::profile();
    let mut rdns = name.children(limits)?;
    if rdns.is_empty() {
        return Err(ZkX509RelationErrorV1::InvalidName);
    }
    let mut attributes = [None; 4];
    while !rdns.is_empty() {
        let rdn = rdns.read_value()?.require_tag(ZkX509DerTagV1::SET)?;
        let mut values = rdn.children(limits)?;
        if values.is_empty() {
            return Err(ZkX509RelationErrorV1::InvalidName);
        }
        while !values.is_empty() {
            let attribute = values.read_value()?.require_tag(ZkX509DerTagV1::SEQUENCE)?;
            let mut fields = attribute.children(limits)?;
            let oid = fields.read_value()?.as_object_identifier()?;
            let value = fields.read_value()?;
            if !fields.is_empty() {
                return Err(ZkX509RelationErrorV1::InvalidName);
            }
            let index = if oid.equals(OID_COUNTRY_NAME) {
                0
            } else if oid.equals(OID_ORGANIZATION_NAME) {
                1
            } else if oid.equals(OID_ORGANIZATIONAL_UNIT_NAME) {
                2
            } else if oid.equals(OID_COMMON_NAME) {
                3
            } else {
                return Err(ZkX509RelationErrorV1::InvalidName);
            };
            validate_directory_string_v1(index, value)?;
            if attributes[index].replace(value.contents()).is_some() {
                return Err(ZkX509RelationErrorV1::InvalidName);
            }
        }
    }
    Ok(ParsedNameV1 {
        encoded: name.encoded(),
        attributes,
    })
}
pub(super) fn validate_directory_string_v1(
    index: usize,
    value: ZkX509DerValueV1<'_>,
) -> Result<(), ZkX509RelationErrorV1> {
    if value.contents().is_empty() || value.contents().len() > ZK_X509_MAX_ATTRIBUTE_VALUE_BYTES_V1
    {
        return Err(ZkX509RelationErrorV1::InvalidName);
    }
    if index == 0 {
        if value.tag() != universal_primitive_tag_v1(19)
            || value.contents().len() != 2
            || !value
                .contents()
                .iter()
                .all(|byte| byte.is_ascii_uppercase())
        {
            return Err(ZkX509RelationErrorV1::InvalidName);
        }
        return Ok(());
    }
    match value.tag() {
        tag if tag == universal_primitive_tag_v1(12) => {
            let string = core::str::from_utf8(value.contents())
                .map_err(|_| ZkX509RelationErrorV1::InvalidName)?;
            if string.chars().any(|character| {
                matches!(
                    u32::from(character),
                    0x0000..=0x001f | 0x007f..=0x009f
                )
            }) {
                return Err(ZkX509RelationErrorV1::InvalidName);
            }
        }
        tag if tag == universal_primitive_tag_v1(19) => {
            if !value.contents().iter().all(|byte| {
                byte.is_ascii_alphanumeric()
                    || matches!(
                        *byte,
                        b' ' | b'\''
                            | b'('
                            | b')'
                            | b'+'
                            | b','
                            | b'-'
                            | b'.'
                            | b'/'
                            | b':'
                            | b'='
                            | b'?'
                    )
            }) {
                return Err(ZkX509RelationErrorV1::InvalidName);
            }
        }
        _ => return Err(ZkX509RelationErrorV1::InvalidName),
    }
    Ok(())
}
pub(super) fn context_tag_v1(number: u32, constructed: bool) -> ZkX509DerTagV1 {
    ZkX509DerTagV1 {
        class: ZkX509DerClassV1::ContextSpecific,
        constructed,
        number,
    }
}
pub(super) fn universal_primitive_tag_v1(number: u32) -> ZkX509DerTagV1 {
    ZkX509DerTagV1 {
        class: ZkX509DerClassV1::Universal,
        constructed: false,
        number,
    }
}
pub(super) fn parse_certificate_extensions_v1(
    explicit_contents: &[u8],
) -> Result<CertificateExtensionsV1<'_>, ZkX509RelationErrorV1> {
    let limits = ZkX509DerLimitsV1::profile();
    let sequence = parse_single_der_value_v1(explicit_contents, limits)?
        .require_tag(ZkX509DerTagV1::SEQUENCE)?;
    let mut extensions = sequence.children(limits)?;
    if extensions.is_empty() {
        return Err(ZkX509RelationErrorV1::InvalidExtensions);
    }
    let mut authority_key_identifier = None;
    let mut subject_key_identifier = None;
    let mut basic_constraints = None;
    let mut key_usage = None;
    let mut extended_key_usages = None;
    let mut previous_rank = None;
    while !extensions.is_empty() {
        let (oid, critical, value) = parse_extension_v1(extensions.read_value()?)?;
        let rank = if oid == OID_AUTHORITY_KEY_IDENTIFIER {
            0
        } else if oid == OID_SUBJECT_KEY_IDENTIFIER {
            1
        } else if oid == OID_KEY_USAGE {
            2
        } else if oid == OID_BASIC_CONSTRAINTS {
            3
        } else if oid == OID_EXTENDED_KEY_USAGE {
            4
        } else {
            return Err(ZkX509RelationErrorV1::InvalidExtensions);
        };
        if previous_rank.is_some_and(|previous| previous >= rank) {
            return Err(ZkX509RelationErrorV1::InvalidExtensions);
        }
        previous_rank = Some(rank);
        if rank == 0 {
            if critical || authority_key_identifier.is_some() {
                return Err(ZkX509RelationErrorV1::InvalidExtensions);
            }
            authority_key_identifier = Some(parse_authority_key_identifier_v1(value)?);
        } else if rank == 1 {
            if critical || subject_key_identifier.is_some() {
                return Err(ZkX509RelationErrorV1::InvalidExtensions);
            }
            subject_key_identifier = Some(parse_subject_key_identifier_v1(value)?);
        } else if rank == 2 {
            if !critical || key_usage.is_some() {
                return Err(ZkX509RelationErrorV1::InvalidExtensions);
            }
            key_usage = Some(parse_key_usage_v1(value)?);
        } else if rank == 3 {
            if !critical || basic_constraints.is_some() {
                return Err(ZkX509RelationErrorV1::InvalidExtensions);
            }
            basic_constraints = Some(parse_basic_constraints_v1(value)?);
        } else {
            if !critical || extended_key_usages.is_some() {
                return Err(ZkX509RelationErrorV1::InvalidExtensions);
            }
            extended_key_usages = Some(parse_extended_key_usages_v1(value)?);
        }
    }
    Ok(CertificateExtensionsV1 {
        authority_key_identifier: authority_key_identifier
            .ok_or(ZkX509RelationErrorV1::InvalidExtensions)?,
        subject_key_identifier: subject_key_identifier
            .ok_or(ZkX509RelationErrorV1::InvalidExtensions)?,
        basic_constraints: basic_constraints.ok_or(ZkX509RelationErrorV1::InvalidExtensions)?,
        key_usage: key_usage.ok_or(ZkX509RelationErrorV1::InvalidExtensions)?,
        extended_key_usages,
    })
}
pub(super) fn parse_extension_v1(
    extension: ZkX509DerValueV1<'_>,
) -> Result<(&[u8], bool, &[u8]), ZkX509RelationErrorV1> {
    let limits = ZkX509DerLimitsV1::profile();
    let extension = extension.require_tag(ZkX509DerTagV1::SEQUENCE)?;
    let mut fields = extension.children(limits)?;
    let oid = fields.read_value()?.as_object_identifier()?.contents();
    let critical_or_value = fields.read_value()?;
    let (critical, value) = if critical_or_value.tag() == ZkX509DerTagV1::BOOLEAN {
        if critical_or_value.contents() != [0xff] {
            // DER requires a DEFAULT FALSE field to be omitted.
            return Err(ZkX509RelationErrorV1::InvalidExtensions);
        }
        (
            true,
            fields
                .read_value()?
                .require_tag(ZkX509DerTagV1::OCTET_STRING)?
                .contents(),
        )
    } else {
        (
            false,
            critical_or_value
                .require_tag(ZkX509DerTagV1::OCTET_STRING)?
                .contents(),
        )
    };
    if !fields.is_empty() {
        return Err(ZkX509RelationErrorV1::InvalidExtensions);
    }
    Ok((oid, critical, value))
}
pub(super) fn parse_authority_key_identifier_v1(
    encoded: &[u8],
) -> Result<&[u8], ZkX509RelationErrorV1> {
    let limits = ZkX509DerLimitsV1::profile();
    let sequence =
        parse_single_der_value_v1(encoded, limits)?.require_tag(ZkX509DerTagV1::SEQUENCE)?;
    let mut fields = sequence.children(limits)?;
    let key_identifier = fields.read_value()?;
    if key_identifier.tag() != context_tag_v1(0, false)
        || key_identifier.contents().is_empty()
        || key_identifier.contents().len() > 64
        || !fields.is_empty()
    {
        return Err(ZkX509RelationErrorV1::InvalidExtensions);
    }
    Ok(key_identifier.contents())
}
pub(super) fn parse_subject_key_identifier_v1(
    encoded: &[u8],
) -> Result<&[u8], ZkX509RelationErrorV1> {
    let identifier = parse_single_der_value_v1(encoded, ZkX509DerLimitsV1::profile())?
        .require_tag(ZkX509DerTagV1::OCTET_STRING)?;
    if identifier.contents().is_empty() || identifier.contents().len() > 64 {
        return Err(ZkX509RelationErrorV1::InvalidExtensions);
    }
    Ok(identifier.contents())
}
pub(super) fn parse_basic_constraints_v1(
    encoded: &[u8],
) -> Result<BasicConstraintsV1, ZkX509RelationErrorV1> {
    let limits = ZkX509DerLimitsV1::profile();
    let sequence =
        parse_single_der_value_v1(encoded, limits)?.require_tag(ZkX509DerTagV1::SEQUENCE)?;
    let mut fields = sequence.children(limits)?;
    if fields.is_empty() {
        return Ok(BasicConstraintsV1 {
            ca: false,
            path_len: None,
        });
    }
    let first = fields.read_value()?;
    if first.tag() != ZkX509DerTagV1::BOOLEAN || first.contents() != [0xff] {
        // `cA DEFAULT FALSE` is either omitted or the canonical TRUE value.
        return Err(ZkX509RelationErrorV1::InvalidExtensions);
    }
    let path_len = if fields.is_empty() {
        None
    } else {
        let value = parse_nonnegative_integer_v1(fields.read_value()?, 4)?;
        Some(u32::try_from(value).map_err(|_| ZkX509RelationErrorV1::InvalidInteger)?)
    };
    if !fields.is_empty() {
        return Err(ZkX509RelationErrorV1::InvalidExtensions);
    }
    Ok(BasicConstraintsV1 { ca: true, path_len })
}
pub(super) fn parse_key_usage_v1(encoded: &[u8]) -> Result<u16, ZkX509RelationErrorV1> {
    let bit_string =
        parse_single_der_value_v1(encoded, ZkX509DerLimitsV1::profile())?.as_bit_string()?;
    let bytes = bit_string.bytes();
    if bytes.is_empty()
        || bytes.len() > 2
        || (bytes.last().copied().unwrap_or_default() & (1 << bit_string.unused_bits())) == 0
    {
        // NamedBitList DER omits all trailing zero bits.
        return Err(ZkX509RelationErrorV1::InvalidExtensions);
    }
    let mut flags = 0_u16;
    for (byte_index, byte) in bytes.iter().copied().enumerate() {
        for bit in 0..8 {
            if byte & (0x80 >> bit) != 0 {
                let flag_index = byte_index * 8 + bit;
                if flag_index >= u16::BITS as usize {
                    return Err(ZkX509RelationErrorV1::InvalidExtensions);
                }
                flags |= 1_u16 << flag_index;
            }
        }
    }
    Ok(flags)
}
pub(super) fn parse_extended_key_usages_v1(
    encoded: &[u8],
) -> Result<Vec<PrivacyX509ExtendedKeyUsageV1>, ZkX509RelationErrorV1> {
    let limits = ZkX509DerLimitsV1::profile();
    let sequence =
        parse_single_der_value_v1(encoded, limits)?.require_tag(ZkX509DerTagV1::SEQUENCE)?;
    let mut fields = sequence.children(limits)?;
    if fields.is_empty() {
        return Err(ZkX509RelationErrorV1::InvalidExtensions);
    }
    let mut usages = Vec::new();
    while !fields.is_empty() {
        let oid = fields.read_value()?.as_object_identifier()?.contents();
        let usage = if oid == OID_CLIENT_AUTHENTICATION {
            debug_assert_eq!(
                ZK_X509_CLIENT_AUTHENTICATION_EKU_OID_V1,
                "1.3.6.1.5.5.7.3.2"
            );
            PrivacyX509ExtendedKeyUsageV1::ClientAuthentication
        } else if oid == ZK_X509_DOCUMENT_SIGNING_EKU_DER_VALUE_V1 {
            PrivacyX509ExtendedKeyUsageV1::DocumentSigning
        } else if oid == ZK_X509_WALLET_IDENTITY_EKU_DER_VALUE_V1 {
            PrivacyX509ExtendedKeyUsageV1::WalletIdentity
        } else {
            return Err(ZkX509RelationErrorV1::InvalidExtensions);
        };
        if usages.last().is_some_and(|previous| *previous >= usage) {
            return Err(ZkX509RelationErrorV1::InvalidExtensions);
        }
        usages.push(usage);
    }
    Ok(usages)
}
pub(super) fn parse_nonnegative_integer_v1(
    value: ZkX509DerValueV1<'_>,
    max_bytes: usize,
) -> Result<u64, ZkX509RelationErrorV1> {
    let integer = value.as_integer()?.contents();
    if integer.first().is_some_and(|first| first & 0x80 != 0) {
        return Err(ZkX509RelationErrorV1::InvalidInteger);
    }
    let unsigned = if integer.len() > 1 && integer[0] == 0 {
        &integer[1..]
    } else {
        integer
    };
    if unsigned.len() > max_bytes || unsigned.len() > 8 {
        return Err(ZkX509RelationErrorV1::InvalidInteger);
    }
    let mut result = 0_u64;
    for byte in unsigned {
        result = result
            .checked_mul(256)
            .and_then(|value| value.checked_add(u64::from(*byte)))
            .ok_or(ZkX509RelationErrorV1::InvalidInteger)?;
    }
    Ok(result)
}
pub(super) fn validate_ecdsa_signature_der_v1(
    signature_der: &[u8],
) -> Result<P256Signature, ZkX509RelationErrorV1> {
    let limits = ZkX509DerLimitsV1::profile();
    let sequence =
        parse_single_der_value_v1(signature_der, limits)?.require_tag(ZkX509DerTagV1::SEQUENCE)?;
    let mut fields = sequence.children(limits)?;
    fields.read_value()?.as_integer()?.positive_unsigned(32)?;
    fields.read_value()?.as_integer()?.positive_unsigned(32)?;
    if !fields.is_empty() {
        return Err(ZkX509RelationErrorV1::InvalidSignatureEncoding);
    }
    P256Signature::from_der(signature_der)
        .map_err(|_| ZkX509RelationErrorV1::InvalidSignatureEncoding)
}
// Keep the complete strict-parser evidence for the reference relation. Ordinary
// holder preflight reads only the signed interval fields; it still validates
// the same full profile. Reference builds retain dead-field lint coverage.
#[cfg_attr(not(any(test, feature = "privacy-release-evidence")), allow(dead_code))]
pub(super) struct ParsedCrlV1<'a> {
    pub(super) tbs_der: &'a [u8],
    pub(super) issuer: ParsedNameV1<'a>,
    pub(super) this_update: u64,
    pub(super) next_update: u64,
    pub(super) revoked_serials: Vec<&'a [u8]>,
    pub(super) authority_key_identifier: &'a [u8],
    pub(super) crl_number: u64,
    pub(super) signature_der: &'a [u8],
}
pub(super) fn parse_crl_v1(der: &[u8]) -> Result<ParsedCrlV1<'_>, ZkX509RelationErrorV1> {
    let limits = ZkX509DerLimitsV1::profile();
    let crl = parse_single_der_value_v1(der, limits)?.require_tag(ZkX509DerTagV1::SEQUENCE)?;
    let mut outer = crl.children(limits)?;
    let tbs = outer.read_value()?.require_tag(ZkX509DerTagV1::SEQUENCE)?;
    let outer_algorithm = outer.read_value()?;
    if outer_algorithm.encoded() != ZK_X509_ECDSA_WITH_SHA256_ALGORITHM_IDENTIFIER_DER_V1 {
        return Err(ZkX509RelationErrorV1::InvalidCrl);
    }
    validate_ecdsa_with_sha256_algorithm_identifier_v1(outer_algorithm.encoded(), limits)?;
    let signature = outer.read_value()?.as_bit_string()?;
    if !outer.is_empty() || signature.unused_bits() != 0 {
        return Err(ZkX509RelationErrorV1::InvalidCrl);
    }
    validate_ecdsa_signature_der_v1(signature.bytes())?;
    let mut fields = tbs.children(limits)?;
    let version = fields.read_value()?.as_integer()?;
    if version.contents() != [1] {
        return Err(ZkX509RelationErrorV1::InvalidCrl);
    }
    let tbs_algorithm = fields.read_value()?;
    if tbs_algorithm.encoded() != outer_algorithm.encoded() {
        return Err(ZkX509RelationErrorV1::InvalidCrl);
    }
    let issuer_value = fields.read_value()?.require_tag(ZkX509DerTagV1::SEQUENCE)?;
    let issuer = parse_name_v1(issuer_value)?;
    let this_update = parse_time_v1(fields.read_value()?)?;
    let next_update = parse_time_v1(fields.read_value()?)?;
    if next_update <= this_update {
        return Err(ZkX509RelationErrorV1::InvalidCrl);
    }
    let entries_or_extensions = fields.read_value()?;
    let (revoked_serials, extension_wrapper) =
        if entries_or_extensions.tag() == ZkX509DerTagV1::SEQUENCE {
            let serials: Vec<&[u8]> =
                parse_revoked_certificates_v1(entries_or_extensions, this_update)?;
            (serials, fields.read_value()?)
        } else {
            (Vec::new(), entries_or_extensions)
        };
    if extension_wrapper.tag() != context_tag_v1(0, true) || !fields.is_empty() {
        return Err(ZkX509RelationErrorV1::InvalidCrl);
    }
    let (authority_key_identifier, crl_number) =
        parse_crl_extensions_v1(extension_wrapper.contents())?;
    Ok(ParsedCrlV1 {
        tbs_der: tbs.encoded(),
        issuer,
        this_update,
        next_update,
        revoked_serials,
        authority_key_identifier,
        crl_number,
        signature_der: signature.bytes(),
    })
}
pub(super) fn parse_revoked_certificates_v1(
    sequence: ZkX509DerValueV1<'_>,
    this_update: u64,
) -> Result<Vec<&[u8]>, ZkX509RelationErrorV1> {
    let limits = ZkX509DerLimitsV1::profile();
    let mut entries = sequence.children(limits)?;
    if entries.is_empty() {
        // An empty OPTIONAL sequence has a unique DER representation: absent.
        return Err(ZkX509RelationErrorV1::InvalidCrl);
    }
    let mut serials: Vec<&[u8]> = Vec::new();
    while !entries.is_empty() {
        if serials.len() >= ZK_X509_MAX_CRL_ENTRIES_V1 {
            return Err(ZkX509RelationErrorV1::InvalidCrl);
        }
        let entry = entries
            .read_value()?
            .require_tag(ZkX509DerTagV1::SEQUENCE)?;
        let mut fields = entry.children(limits)?;
        let serial = fields
            .read_value()?
            .as_integer()?
            .positive_unsigned(ZK_X509_MAX_SERIAL_BYTES_V1)?
            .bytes();
        let revocation_time = parse_time_v1(fields.read_value()?)?;
        if revocation_time > this_update || !fields.is_empty() {
            // Entry extensions, including reasonCode/certificateIssuer, are
            // outside the complete direct-CRL V1 profile.
            return Err(ZkX509RelationErrorV1::InvalidCrl);
        }
        if serials.last().is_some_and(|previous| {
            previous.len() > serial.len() || (previous.len() == serial.len() && *previous >= serial)
        }) {
            // The closed profile admits one canonical complete list: positive
            // serial magnitudes in strict unsigned numeric order.  This makes
            // duplicates, permutations, and omitted/reinserted entries
            // algebraically distinguishable without a second accumulator.
            return Err(ZkX509RelationErrorV1::InvalidCrl);
        }
        serials.push(serial);
    }
    Ok(serials)
}
pub(super) fn parse_crl_extensions_v1(
    explicit_contents: &[u8],
) -> Result<(&[u8], u64), ZkX509RelationErrorV1> {
    let limits = ZkX509DerLimitsV1::profile();
    let sequence = parse_single_der_value_v1(explicit_contents, limits)?
        .require_tag(ZkX509DerTagV1::SEQUENCE)?;
    let mut extensions = sequence.children(limits)?;
    let mut authority_key_identifier = None;
    let mut crl_number = None;
    let mut previous_rank = None;
    while !extensions.is_empty() {
        let (oid, critical, value) = parse_extension_v1(extensions.read_value()?)?;
        if critical {
            return Err(ZkX509RelationErrorV1::InvalidCrl);
        }
        let rank = if oid == OID_AUTHORITY_KEY_IDENTIFIER {
            0
        } else if oid == OID_CRL_NUMBER {
            1
        } else {
            return Err(ZkX509RelationErrorV1::InvalidCrl);
        };
        if previous_rank.is_some_and(|previous| previous >= rank) {
            return Err(ZkX509RelationErrorV1::InvalidCrl);
        }
        previous_rank = Some(rank);
        if rank == 0 {
            if authority_key_identifier.is_some() {
                return Err(ZkX509RelationErrorV1::InvalidCrl);
            }
            authority_key_identifier = Some(parse_authority_key_identifier_v1(value)?);
        } else {
            if crl_number.is_some() {
                return Err(ZkX509RelationErrorV1::InvalidCrl);
            }
            let number = parse_single_der_value_v1(value, limits)?;
            crl_number = Some(parse_nonnegative_integer_v1(number, 8)?);
        }
    }
    Ok((
        authority_key_identifier.ok_or(ZkX509RelationErrorV1::InvalidCrl)?,
        crl_number.ok_or(ZkX509RelationErrorV1::InvalidCrl)?,
    ))
}
