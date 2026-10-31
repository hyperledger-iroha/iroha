//! Shared deterministic signed-DER fixture encoder; never part of a production module.
//!
//! Both the existing release fixture and ordinary integration tests compile
//! this exact owner. Fixed test keys are public fixture material.
use iroha_data_model::privacy::{
    PrivacyX509ExtendedKeyUsageV1, PrivacyZkX509CertificateValidityV1,
};
use p256::ecdsa::{
    Signature as P256Signature, SigningKey as P256SigningKey, signature::Signer as _,
};
use time::OffsetDateTime;
pub const OID_AUTHORITY_KEY_IDENTIFIER: &[u8] = &[0x55, 0x1d, 0x23];
pub const OID_SUBJECT_KEY_IDENTIFIER: &[u8] = &[0x55, 0x1d, 0x0e];
pub const OID_KEY_USAGE: &[u8] = &[0x55, 0x1d, 0x0f];
pub const OID_BASIC_CONSTRAINTS: &[u8] = &[0x55, 0x1d, 0x13];
pub const OID_EXTENDED_KEY_USAGE: &[u8] = &[0x55, 0x1d, 0x25];
pub const OID_CRL_NUMBER: &[u8] = &[0x55, 0x1d, 0x14];
pub const OID_COUNTRY_NAME: &[u8] = &[0x55, 0x04, 0x06];
pub const OID_ORGANIZATION_NAME: &[u8] = &[0x55, 0x04, 0x0a];
pub const OID_ORGANIZATIONAL_UNIT_NAME: &[u8] = &[0x55, 0x04, 0x0b];
pub const OID_COMMON_NAME: &[u8] = &[0x55, 0x04, 0x03];
pub const OID_CLIENT_AUTHENTICATION: &[u8] = &[0x2b, 0x06, 0x01, 0x05, 0x05, 0x07, 0x03, 0x02];
pub const ZK_X509_ECDSA_WITH_SHA256_ALGORITHM_IDENTIFIER_DER_V1: &[u8] = &[
    0x30, 0x0a, 0x06, 0x08, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x04, 0x03, 0x02,
];
pub const ZK_X509_P256_PUBLIC_KEY_ALGORITHM_IDENTIFIER_DER_V1: &[u8] = &[
    0x30, 0x13, 0x06, 0x07, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x02, 0x01, 0x06, 0x08, 0x2a, 0x86, 0x48,
    0xce, 0x3d, 0x03, 0x01, 0x07,
];
pub const ZK_X509_DOCUMENT_SIGNING_EKU_DER_VALUE_V1: &[u8] = &[
    0x69, 0xa9, 0x8b, 0xd6, 0xf7, 0xd8, 0x86, 0xaa, 0xc4, 0xb3, 0x8b, 0xcd, 0xbc, 0xba, 0xe7, 0x86,
    0x8c, 0xf1, 0x7b,
];
pub const ZK_X509_WALLET_IDENTITY_EKU_DER_VALUE_V1: &[u8] = &[
    0x69, 0x83, 0xe9, 0xce, 0xee, 0xa4, 0xdc, 0xb4, 0xda, 0xe5, 0xdf, 0xbe, 0xf0, 0xac, 0xd6, 0xd0,
    0xac, 0xc5, 0xa4, 0x6b,
];
pub fn p256_key(seed: u8) -> Result<P256SigningKey, &'static str> {
    let mut bytes = [0_u8; 32];
    bytes[31] = seed;
    P256SigningKey::from_slice(&bytes).map_err(|_| "invalid deterministic P-256 key")
}
#[allow(clippy::too_many_arguments)]
pub fn certificate(
    serial: &[u8],
    issuer: &[u8],
    subject: &[u8],
    subject_key: &P256SigningKey,
    issuer_key: &P256SigningKey,
    subject_key_identifier: &[u8],
    authority_key_identifier: &[u8],
    ca_path_len: Option<u32>,
    validity_der: &[u8],
) -> Vec<u8> {
    certificate_with_extension_profile_v1(
        serial,
        issuer,
        subject,
        subject_key,
        issuer_key,
        FixtureCertificateExtensionsV1 {
            subject_key_identifier,
            authority_key_identifier,
            ca_path_len,
            extended_key_usages: &[PrivacyX509ExtendedKeyUsageV1::ClientAuthentication],
        },
        validity_der,
    )
}

pub struct FixtureCertificateExtensionsV1<'a> {
    pub(super) subject_key_identifier: &'a [u8],
    pub(super) authority_key_identifier: &'a [u8],
    pub(super) ca_path_len: Option<u32>,
    pub(super) extended_key_usages: &'a [PrivacyX509ExtendedKeyUsageV1],
}

pub fn certificate_with_extension_profile_v1(
    serial: &[u8],
    issuer: &[u8],
    subject: &[u8],
    subject_key: &P256SigningKey,
    issuer_key: &P256SigningKey,
    extensions: FixtureCertificateExtensionsV1<'_>,
    validity_der: &[u8],
) -> Vec<u8> {
    let FixtureCertificateExtensionsV1 {
        subject_key_identifier,
        authority_key_identifier,
        ca_path_len,
        extended_key_usages,
    } = extensions;
    let spki = spki(subject_key);
    let basic_constraints = ca_path_len.map_or_else(
        || sequence(&[]),
        |path_len| sequence(&[tlv(0x01, &[0xff]), integer(u64::from(path_len))]),
    );
    let key_usage = if ca_path_len.is_some() {
        bit_string(&[0x06], 1)
    } else {
        bit_string(&[0x80], 7)
    };
    let mut extensions = vec![
        extension(
            OID_AUTHORITY_KEY_IDENTIFIER,
            false,
            &aki_inner(authority_key_identifier),
        ),
        extension(
            OID_SUBJECT_KEY_IDENTIFIER,
            false,
            &octet_string(subject_key_identifier),
        ),
        extension(OID_KEY_USAGE, true, &key_usage),
        extension(OID_BASIC_CONSTRAINTS, true, &basic_constraints),
    ];
    if ca_path_len.is_none() {
        extensions.push(extension(
            OID_EXTENDED_KEY_USAGE,
            true,
            &sequence(
                &extended_key_usages
                    .iter()
                    .map(|usage| {
                        object_identifier(match usage {
                            PrivacyX509ExtendedKeyUsageV1::ClientAuthentication => {
                                OID_CLIENT_AUTHENTICATION
                            }
                            PrivacyX509ExtendedKeyUsageV1::DocumentSigning => {
                                ZK_X509_DOCUMENT_SIGNING_EKU_DER_VALUE_V1
                            }
                            PrivacyX509ExtendedKeyUsageV1::WalletIdentity => {
                                ZK_X509_WALLET_IDENTITY_EKU_DER_VALUE_V1
                            }
                        })
                    })
                    .collect::<Vec<_>>(),
            ),
        ));
    }
    let tbs = sequence(&[
        tlv(0xa0, &integer(2)),
        positive_integer(serial),
        ZK_X509_ECDSA_WITH_SHA256_ALGORITHM_IDENTIFIER_DER_V1.to_vec(),
        issuer.to_vec(),
        validity_der.to_vec(),
        subject.to_vec(),
        spki,
        tlv(0xa3, &sequence(&extensions)),
    ]);
    let signature: P256Signature = issuer_key.sign(&tbs);
    sequence(&[
        tbs,
        ZK_X509_ECDSA_WITH_SHA256_ALGORITHM_IDENTIFIER_DER_V1.to_vec(),
        bit_string(signature.to_der().as_bytes(), 0),
    ])
}
#[allow(clippy::too_many_arguments)]
pub fn crl(
    issuer: &[u8],
    issuer_key: &P256SigningKey,
    authority_key_identifier: &[u8],
    number: u64,
    revoked_serials: &[Vec<u8>],
    this_update: u64,
    next_update: u64,
    revoked_at: u64,
) -> Result<Vec<u8>, &'static str> {
    let extensions = sequence(&[
        extension(
            OID_AUTHORITY_KEY_IDENTIFIER,
            false,
            &aki_inner(authority_key_identifier),
        ),
        extension(OID_CRL_NUMBER, false, &integer(number)),
    ]);
    let mut fields = vec![
        integer(1),
        ZK_X509_ECDSA_WITH_SHA256_ALGORITHM_IDENTIFIER_DER_V1.to_vec(),
        issuer.to_vec(),
        time_tlv_v1(this_update)?,
        time_tlv_v1(next_update)?,
    ];
    if !revoked_serials.is_empty() {
        let revoked_at = time_tlv_v1(revoked_at)?;
        fields.push(sequence(
            &revoked_serials
                .iter()
                .map(|serial| sequence(&[positive_integer(serial), revoked_at.clone()]))
                .collect::<Vec<_>>(),
        ));
    }
    fields.push(tlv(0xa0, &extensions));
    let tbs = sequence(&fields);
    let signature: P256Signature = issuer_key.sign(&tbs);
    Ok(sequence(&[
        tbs,
        ZK_X509_ECDSA_WITH_SHA256_ALGORITHM_IDENTIFIER_DER_V1.to_vec(),
        bit_string(signature.to_der().as_bytes(), 0),
    ]))
}
/// Encode one RFC 5280 `Validity` sequence.
pub fn validity_der_v1(
    validity: PrivacyZkX509CertificateValidityV1,
) -> Result<Vec<u8>, &'static str> {
    Ok(sequence(&[
        time_tlv_v1(validity.not_before_unix_seconds())?,
        time_tlv_v1(validity.not_after_unix_seconds())?,
    ]))
}
/// Encode one RFC 5280 `Time`: `UTCTime` through 2049 and `GeneralizedTime`
/// from 2050, exactly as the closed profile requires.
pub fn time_tlv_v1(unix_seconds: u64) -> Result<Vec<u8>, &'static str> {
    let unix_seconds = i64::try_from(unix_seconds)
        .map_err(|_| "release fixture time exceeds signed timestamp range")?;
    let date_time = OffsetDateTime::from_unix_timestamp(unix_seconds)
        .map_err(|_| "release fixture time is outside the supported calendar")?;
    let year = date_time.year();
    let (tag, year_text) = match year {
        1950..=2049 => (0x17, format!("{:02}", year.rem_euclid(100))),
        2050..=9999 => (0x18, format!("{year:04}")),
        _ => return Err("release fixture time is outside the RFC 5280 calendar"),
    };
    let encoded = format!(
        "{year_text}{:02}{:02}{:02}{:02}{:02}Z",
        u8::from(date_time.month()),
        date_time.day(),
        date_time.hour(),
        date_time.minute(),
        date_time.second(),
    );
    Ok(tlv(tag, encoded.as_bytes()))
}
pub fn spki(key: &P256SigningKey) -> Vec<u8> {
    let point = key.verifying_key().to_sec1_point(false);
    sequence(&[
        ZK_X509_P256_PUBLIC_KEY_ALGORITHM_IDENTIFIER_DER_V1.to_vec(),
        bit_string(point.as_bytes(), 0),
    ])
}
pub fn name(attributes: &[(&[u8], u8, &[u8])]) -> Vec<u8> {
    sequence(
        &attributes
            .iter()
            .map(|(oid, tag, value)| {
                tlv(0x31, &sequence(&[object_identifier(oid), tlv(*tag, value)]))
            })
            .collect::<Vec<_>>(),
    )
}
pub fn extension(oid: &[u8], critical: bool, inner: &[u8]) -> Vec<u8> {
    let mut fields = vec![object_identifier(oid)];
    if critical {
        fields.push(tlv(0x01, &[0xff]));
    }
    fields.push(octet_string(inner));
    sequence(&fields)
}
pub fn aki_inner(identifier: &[u8]) -> Vec<u8> {
    sequence(&[tlv(0x80, identifier)])
}
pub fn object_identifier(contents: &[u8]) -> Vec<u8> {
    tlv(0x06, contents)
}
pub fn octet_string(contents: &[u8]) -> Vec<u8> {
    tlv(0x04, contents)
}
pub fn bit_string(bytes: &[u8], unused_bits: u8) -> Vec<u8> {
    let mut contents = Vec::with_capacity(bytes.len() + 1);
    contents.push(unused_bits);
    contents.extend_from_slice(bytes);
    tlv(0x03, &contents)
}
pub fn integer(value: u64) -> Vec<u8> {
    if value == 0 {
        return tlv(0x02, &[0]);
    }
    let bytes = value.to_be_bytes();
    let first = bytes
        .iter()
        .position(|byte| *byte != 0)
        .unwrap_or(bytes.len() - 1);
    let mut magnitude = bytes[first..].to_vec();
    if magnitude[0] & 0x80 != 0 {
        magnitude.insert(0, 0);
    }
    tlv(0x02, &magnitude)
}
pub fn positive_integer(magnitude: &[u8]) -> Vec<u8> {
    let mut contents = magnitude.to_vec();
    if contents.first().is_some_and(|byte| byte & 0x80 != 0) {
        contents.insert(0, 0);
    }
    tlv(0x02, &contents)
}
pub fn sequence(values: &[Vec<u8>]) -> Vec<u8> {
    tlv(0x30, &values.concat())
}
pub fn tlv(tag: u8, contents: &[u8]) -> Vec<u8> {
    let mut encoded = vec![tag];
    if contents.len() < 128 {
        encoded.push(contents.len() as u8);
    } else {
        let bytes = contents.len().to_be_bytes();
        let first = bytes
            .iter()
            .position(|byte| *byte != 0)
            .unwrap_or(bytes.len() - 1);
        let length = &bytes[first..];
        encoded.push(0x80 | length.len() as u8);
        encoded.extend_from_slice(length);
    }
    encoded.extend_from_slice(contents);
    encoded
}
