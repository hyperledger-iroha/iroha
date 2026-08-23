// Byte-exact DER validation for the X.509 envelope and signature algorithm.
// x509-parser deliberately exposes a permissive semantic view in a few nested
// AlgorithmIdentifier paths. Consensus admission first validates the original
// bytes so ignored remainders or retagged values cannot select a verifier.

#[derive(Copy, Clone)]
struct StrictX509AlgorithmIdentifier<'a> {
    oid: &'a [u8],
    parameters: Option<(DerTag, &'a [u8], &'a [u8])>,
}

fn parse_strict_x509_algorithm_identifier(
    encoded: &[u8],
) -> Result<StrictX509AlgorithmIdentifier<'_>, Error> {
    let mut sequence = DerReader::sequence(encoded)?;
    let oid = sequence.read_expected(0x06)?;
    reject_invalid_attestation!(
        oid.is_empty(),
        "attestation certificate signature algorithm OID is empty",
    );
    let parameters = if sequence.has_remaining() {
        Some(sequence.read_tlv_full_with_raw()?)
    } else {
        None
    };
    reject_invalid_attestation!(
        sequence.has_remaining(),
        "attestation certificate signature algorithm has trailing fields",
    );
    Ok(StrictX509AlgorithmIdentifier { oid, parameters })
}

fn strict_x509_algorithm_parameters_are_absent_or_null(
    parameters: Option<(DerTag, &[u8], &[u8])>,
) -> bool {
    parameters.is_none_or(|(tag, value, raw)| {
        tag.first_byte == 0x05 && value.is_empty() && raw == [0x05, 0x00]
    })
}

fn strict_x509_hash_algorithm(encoded: &[u8]) -> Result<(&[u8], i64), Error> {
    const SHA256_OID: &[u8] = &[0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02, 0x01];
    const SHA384_OID: &[u8] = &[0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02, 0x02];
    const SHA512_OID: &[u8] = &[0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02, 0x03];
    let algorithm = parse_strict_x509_algorithm_identifier(encoded)?;
    reject_invalid_attestation!(
        !strict_x509_algorithm_parameters_are_absent_or_null(algorithm.parameters),
        "attestation certificate RSA-PSS hash parameters are invalid",
    );
    let digest_bytes = if algorithm.oid == SHA256_OID {
        32
    } else if algorithm.oid == SHA384_OID {
        48
    } else if algorithm.oid == SHA512_OID {
        64
    } else {
        return Err(invalid_attestation(
            "attestation certificate RSA-PSS hash algorithm is not approved",
        )
        .into());
    };
    Ok((algorithm.oid, digest_bytes))
}

fn strict_single_der_tlv(input: &[u8], expected_tag: u8) -> Result<&[u8], Error> {
    let mut reader = DerReader::new(input);
    let (tag, _, raw) = reader.read_tlv_full_with_raw()?;
    reject_invalid_attestation!(
        tag.first_byte != expected_tag || reader.has_remaining(),
        "attestation certificate RSA-PSS explicit field is malformed",
    );
    Ok(raw)
}

fn validate_strict_x509_rsa_pss_parameters(parameters_der: &[u8]) -> Result<(), Error> {
    const MGF1_OID: &[u8] = &[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x08];
    let mut parameters = DerReader::sequence(parameters_der)?;
    let hash_field = parameters.read_expected(0xa0)?;
    let mask_field = parameters.read_expected(0xa1)?;
    let salt_field = parameters.read_expected(0xa2)?;
    reject_invalid_attestation!(
        parameters.has_remaining(),
        "attestation certificate RSA-PSS parameters contain trailing fields",
    );

    let hash_algorithm = strict_single_der_tlv(hash_field, 0x30)?;
    let (hash_oid, digest_bytes) = strict_x509_hash_algorithm(hash_algorithm)?;

    let mask_algorithm_der = strict_single_der_tlv(mask_field, 0x30)?;
    let mask_algorithm = parse_strict_x509_algorithm_identifier(mask_algorithm_der)?;
    reject_invalid_attestation!(
        mask_algorithm.oid != MGF1_OID,
        "attestation certificate RSA-PSS mask algorithm is not MGF1",
    );
    let Some((mask_parameters_tag, _, mask_parameters_raw)) = mask_algorithm.parameters else {
        return Err(invalid_attestation(
            "attestation certificate RSA-PSS MGF1 parameters are missing",
        )
        .into());
    };
    reject_invalid_attestation!(
        mask_parameters_tag.first_byte != 0x30,
        "attestation certificate RSA-PSS MGF1 parameters are malformed",
    );
    let mask_hash_algorithm = strict_single_der_tlv(mask_parameters_raw, 0x30)?;
    let (mask_hash_oid, mask_digest_bytes) = strict_x509_hash_algorithm(mask_hash_algorithm)?;
    reject_invalid_attestation!(
        mask_hash_oid != hash_oid || mask_digest_bytes != digest_bytes,
        "attestation certificate RSA-PSS MGF1 hash does not match the signature hash",
    );

    let mut salt = DerReader::new(salt_field);
    let salt_length = der_integer_to_i64(salt.read_single_expected(0x02)?)?;
    reject_invalid_attestation!(
        salt_length != digest_bytes,
        "attestation certificate RSA-PSS salt length does not match the signature hash",
    );
    Ok(())
}

fn validate_strict_x509_signature_algorithm(encoded: &[u8]) -> Result<(), Error> {
    const RSA_PSS_OID: &[u8] = &[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x0a];
    const RSA_SHA256_OID: &[u8] = &[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x0b];
    const RSA_SHA384_OID: &[u8] = &[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x0c];
    const RSA_SHA512_OID: &[u8] = &[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x0d];
    const ECDSA_SHA256_OID: &[u8] = &[0x2a, 0x86, 0x48, 0xce, 0x3d, 0x04, 0x03, 0x02];
    const ECDSA_SHA384_OID: &[u8] = &[0x2a, 0x86, 0x48, 0xce, 0x3d, 0x04, 0x03, 0x03];
    const ED25519_OID: &[u8] = &[0x2b, 0x65, 0x70];
    let algorithm = parse_strict_x509_algorithm_identifier(encoded)?;
    if algorithm.oid == RSA_PSS_OID {
        let Some((tag, _, raw)) = algorithm.parameters else {
            return Err(invalid_attestation(
                "attestation certificate RSA-PSS parameters are missing",
            )
            .into());
        };
        reject_invalid_attestation!(
            tag.first_byte != 0x30,
            "attestation certificate RSA-PSS parameters are not a DER sequence",
        );
        validate_strict_x509_rsa_pss_parameters(raw)
    } else if [RSA_SHA256_OID, RSA_SHA384_OID, RSA_SHA512_OID].contains(&algorithm.oid) {
        reject_invalid_attestation!(
            !strict_x509_algorithm_parameters_are_absent_or_null(algorithm.parameters),
            "attestation certificate RSA signature parameters are invalid",
        );
        Ok(())
    } else if [ECDSA_SHA256_OID, ECDSA_SHA384_OID, ED25519_OID].contains(&algorithm.oid) {
        reject_invalid_attestation!(
            algorithm.parameters.is_some(),
            "attestation certificate signature parameters must be absent",
        );
        Ok(())
    } else {
        Err(
            invalid_attestation("attestation certificate signature algorithm is not approved")
                .into(),
        )
    }
}

fn validate_strict_x509_positive_serial(serial: &[u8]) -> Result<(), Error> {
    if serial.is_empty()
        || serial.len() > 20
        || serial[0] & 0x80 != 0
        || serial.iter().all(|byte| *byte == 0)
        || (serial.len() > 1 && serial[0] == 0 && serial[1] & 0x80 == 0)
    {
        return Err(invalid_attestation(
            "attestation certificate serial number is not a canonical positive integer",
        )
        .into());
    }
    Ok(())
}

fn validate_strict_x509_time(tag: DerTag, value: &[u8]) -> Result<(), Error> {
    let digit_count = match tag.first_byte {
        0x17 if value.len() == 13 => 12,
        0x18 if value.len() == 15 => 14,
        _ => {
            return Err(invalid_attestation(
                "attestation certificate validity time is outside the RFC 5280 profile",
            )
            .into());
        }
    };
    reject_invalid_attestation!(
        value.last() != Some(&b'Z') || !value[..digit_count].iter().all(u8::is_ascii_digit),
        "attestation certificate validity time is outside the RFC 5280 profile",
    );
    if tag.first_byte == 0x18 {
        let year = value[..4]
            .iter()
            .fold(0_u16, |year, digit| year * 10 + u16::from(*digit - b'0'));
        reject_invalid_attestation!(
            year < 2050,
            "attestation certificate must use UTCTime through year 2049",
        );
    }
    Ok(())
}

fn validate_strict_x509_validity(validity: &[u8]) -> Result<(), Error> {
    let mut reader = DerReader::new(validity);
    let (not_before_tag, not_before, _) = reader.read_tlv_full_with_raw()?;
    let (not_after_tag, not_after, _) = reader.read_tlv_full_with_raw()?;
    reject_invalid_attestation!(
        reader.has_remaining(),
        "attestation certificate validity contains trailing fields",
    );
    validate_strict_x509_time(not_before_tag, not_before)?;
    validate_strict_x509_time(not_after_tag, not_after)
}

fn strict_x509_tbs_certificate_der(certificate_der: &[u8]) -> Result<&[u8], Error> {
    let mut certificate = DerReader::sequence(certificate_der)?;
    let (tbs_tag, tbs_value, tbs_raw) = certificate.read_tlv_full_with_raw()?;
    let (outer_algorithm_tag, _, outer_algorithm_raw) = certificate.read_tlv_full_with_raw()?;
    let (signature_tag, signature_value, _) = certificate.read_tlv_full_with_raw()?;
    reject_invalid_attestation!(
        tbs_tag.first_byte != 0x30
            || outer_algorithm_tag.first_byte != 0x30
            || signature_tag.first_byte != 0x03
            || signature_value.len() < 2
            || signature_value[0] != 0
            || certificate.has_remaining(),
        "attestation certificate outer DER envelope is malformed",
    );

    let mut tbs = DerReader::new(tbs_value);
    let (first_tag, first_value, _) = tbs.read_tlv_full_with_raw()?;
    reject_invalid_attestation!(
        first_tag.first_byte != 0xa0,
        "attestation certificate must explicitly declare X.509 version 3",
    );
    let mut version = DerReader::new(first_value);
    let version = der_integer_to_i64(version.read_single_expected(0x02)?)?;
    reject_invalid_attestation!(
        version != 2,
        "attestation certificate must use X.509 version 3",
    );
    validate_strict_x509_positive_serial(tbs.read_expected(0x02)?)?;
    let (inner_algorithm_tag, _, inner_algorithm_raw) = tbs.read_tlv_full_with_raw()?;
    reject_invalid_attestation!(
        inner_algorithm_tag.first_byte != 0x30 || inner_algorithm_raw != outer_algorithm_raw,
        "attestation certificate inner and outer signature algorithms do not match exactly",
    );
    validate_strict_x509_signature_algorithm(inner_algorithm_raw)?;
    tbs.read_expected(0x30)?;
    validate_strict_x509_validity(tbs.read_expected(0x30)?)?;
    Ok(tbs_raw)
}

#[cfg(test)]
mod strict_x509_der_profile_tests {
    use super::*;

    fn rsa_pss_algorithm(hash_last_arc: u8, salt_length: u8) -> Vec<u8> {
        let hash = vec![
            0x30,
            0x0d,
            0x06,
            0x09,
            0x60,
            0x86,
            0x48,
            0x01,
            0x65,
            0x03,
            0x04,
            0x02,
            hash_last_arc,
            0x05,
            0x00,
        ];
        let mut mgf = vec![
            0x30, 0x1a, 0x06, 0x09, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x08,
        ];
        mgf.extend_from_slice(&hash);
        let mut parameters = vec![0xa0, u8::try_from(hash.len()).unwrap()];
        parameters.extend_from_slice(&hash);
        parameters.extend_from_slice(&[0xa1, u8::try_from(mgf.len()).unwrap()]);
        parameters.extend_from_slice(&mgf);
        parameters.extend_from_slice(&[0xa2, 0x03, 0x02, 0x01, salt_length]);
        let mut algorithm = vec![
            0x06,
            0x09,
            0x2a,
            0x86,
            0x48,
            0x86,
            0xf7,
            0x0d,
            0x01,
            0x01,
            0x0a,
            0x30,
            u8::try_from(parameters.len()).unwrap(),
        ];
        algorithm.extend_from_slice(&parameters);
        let mut sequence = vec![0x30, u8::try_from(algorithm.len()).unwrap()];
        sequence.extend_from_slice(&algorithm);
        sequence
    }

    #[test]
    fn strict_rsa_pss_accepts_sha2_profiles_only() {
        for (hash_last_arc, salt_length) in [(1, 32), (2, 48), (3, 64)] {
            validate_strict_x509_signature_algorithm(&rsa_pss_algorithm(
                hash_last_arc,
                salt_length,
            ))
            .expect("exact SHA-2 RSA-PSS profile");
        }
        assert!(validate_strict_x509_signature_algorithm(&rsa_pss_algorithm(2, 32)).is_err());
    }

    #[test]
    fn strict_rsa_pss_rejects_retagged_and_nested_remainder_fields() {
        let mut retagged = rsa_pss_algorithm(1, 32);
        let hash_explicit = retagged
            .iter()
            .position(|byte| *byte == 0xa0)
            .expect("hash explicit tag");
        retagged[hash_explicit] = 0x80;
        assert!(validate_strict_x509_signature_algorithm(&retagged).is_err());

        let mut retagged_oid = vec![
            0x30, 0x0a, 0x04, 0x08, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x04, 0x03, 0x02,
        ];
        assert!(validate_strict_x509_signature_algorithm(&retagged_oid).is_err());
        retagged_oid[2] = 0x06;
        validate_strict_x509_signature_algorithm(&retagged_oid)
            .expect("canonical ECDSA-SHA256 AlgorithmIdentifier");

        let mut nested_remainder = rsa_pss_algorithm(1, 32);
        let hash_length = usize::from(nested_remainder[16]);
        nested_remainder[1] += 2;
        nested_remainder[14] += 2;
        nested_remainder[16] += 2;
        nested_remainder.splice(17 + hash_length..17 + hash_length, [0x05, 0x00]);
        assert!(validate_strict_x509_signature_algorithm(&nested_remainder).is_err());
    }

    #[test]
    fn der_high_tag_number_overflow_cannot_alias_a_known_tag() {
        let mut canonical = DerReader::new(&[0xBF, 0x84, 0x58, 0x00]);
        let (tag, value) = canonical
            .read_tlv_full()
            .expect("canonical context tag 600");
        assert_eq!(tag.number, 600);
        assert!(value.is_empty());

        // 2^32 + 600 has a canonical five-octet base-128 encoding. An unchecked
        // u32 accumulator truncates it to tag 600 and can reinterpret an unknown
        // field as allApplications.
        let mut overflowing = DerReader::new(&[0xBF, 0x90, 0x80, 0x80, 0x84, 0x58, 0x00]);
        let error = overflowing
            .read_tlv_full()
            .err()
            .expect("a high-tag number outside u32 must be rejected");
        assert!(error.to_string().contains("high-tag number is invalid"));
    }

    #[test]
    fn certificate_validity_times_require_the_exact_rfc_5280_profile() {
        fn validity(
            not_before_tag: u8,
            not_before: &[u8],
            not_after_tag: u8,
            not_after: &[u8],
        ) -> Vec<u8> {
            let mut encoded = vec![not_before_tag, not_before.len() as u8];
            encoded.extend_from_slice(not_before);
            encoded.extend_from_slice(&[not_after_tag, not_after.len() as u8]);
            encoded.extend_from_slice(not_after);
            encoded
        }

        validate_strict_x509_validity(&validity(0x17, b"491231235959Z", 0x18, b"20500101000000Z"))
            .expect("whole-second Zulu times with the RFC year split are canonical");

        for invalid in [
            validity(0x18, b"20500101000000.1Z", 0x18, b"20510101000000Z"),
            validity(0x17, b"490101000000+0000", 0x17, b"491231235959Z"),
            validity(0x18, b"20490101000000Z", 0x18, b"20500101000000Z"),
            validity(0x17, b"4901010000Z", 0x17, b"491231235959Z"),
        ] {
            assert!(validate_strict_x509_validity(&invalid).is_err());
        }
    }

    #[test]
    fn embedded_production_roots_have_strict_canonical_envelopes() {
        for encoded in [
            APPLE_APP_ATTESTATION_ROOT_CA_DER_B64,
            ANDROID_KEY_ATTESTATION_ROOT_CA_DER_B64,
            ANDROID_KEY_ATTESTATION_CA_DER_B64,
        ] {
            let der = BASE64_STANDARD
                .decode(encoded)
                .expect("decode embedded root");
            let tbs = strict_x509_tbs_certificate_der(&der).expect("strict embedded root DER");
            assert_eq!(tbs.first(), Some(&0x30));
        }
    }

    #[test]
    fn noncanonical_certificate_outer_length_is_rejected() {
        let der = BASE64_STANDARD
            .decode(APPLE_APP_ATTESTATION_ROOT_CA_DER_B64)
            .expect("decode Apple root");
        assert_eq!(&der[..2], &[0x30, 0x82]);
        let mut noncanonical = Vec::with_capacity(der.len() + 1);
        noncanonical.extend_from_slice(&[0x30, 0x83, 0x00]);
        noncanonical.extend_from_slice(&der[2..]);
        assert!(strict_x509_tbs_certificate_der(&noncanonical).is_err());
    }
}
