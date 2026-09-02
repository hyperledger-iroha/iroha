fn p256_public_key_has_zero_coordinate_material(public_key: &[u8]) -> bool {
    public_key.len() == KAGEMUSHA_ATTESTATION_P256_UNCOMPRESSED_PUBLIC_KEY_LEN
        && public_key.first() == Some(&0x04)
        && public_key[1..].iter().all(|byte| *byte == 0)
}

fn validate_p256_uncompressed_public_key(public_key: &[u8]) -> Result<(), Error> {
    if public_key.len() != KAGEMUSHA_ATTESTATION_P256_UNCOMPRESSED_PUBLIC_KEY_LEN
        || public_key.first() != Some(&0x04)
    {
        return Err(labeled_invariant(
            "invalid_attestation",
            "Kagemusha device attestation assertion public key must be an uncompressed P-256 SEC1 key",
        )
        .into());
    }
    if p256_public_key_has_zero_coordinate_material(public_key) {
        return Err(labeled_invariant(
            "invalid_attestation",
            "Kagemusha device attestation assertion public key must be a valid uncompressed P-256 SEC1 point",
        )
        .into());
    }
    P256PublicKey::from_sec1_bytes(public_key).map(|_| ()).map_err(|_| {
        labeled_invariant(
            "invalid_attestation",
            "Kagemusha device attestation assertion public key must be a valid uncompressed P-256 SEC1 point",
        )
        .into()
    })
}

fn cbor_text_key_value<'a>(
    map: &'a [(ciborium::value::Value, ciborium::value::Value)],
    key: &str,
) -> Result<Option<&'a ciborium::value::Value>, Error> {
    let mut matches = map.iter().filter(
        |(candidate, _)| matches!(candidate, ciborium::value::Value::Text(text) if text == key),
    );
    let first = matches.next().map(|(_, value)| value);
    if matches.next().is_some() {
        return Err(labeled_invariant(
            "invalid_attestation",
            "attestation CBOR map contains a duplicate text key",
        )
        .into());
    }
    Ok(first)
}

fn cbor_integer_key_value<'a>(
    map: &'a [(ciborium::value::Value, ciborium::value::Value)],
    key: i128,
) -> Result<Option<&'a ciborium::value::Value>, Error> {
    let mut matches = map.iter().filter(|(candidate, _)| {
        matches!(candidate, ciborium::value::Value::Integer(value) if i128::from(value.clone()) == key)
    });
    let first = matches.next().map(|(_, value)| value);
    if matches.next().is_some() {
        return Err(labeled_invariant(
            "invalid_attestation",
            "attestation CBOR map contains a duplicate integer key",
        )
        .into());
    }
    Ok(first)
}

fn cbor_text_value(
    map: &[(ciborium::value::Value, ciborium::value::Value)],
    key: &str,
) -> Result<Option<String>, Error> {
    Ok(match cbor_text_key_value(map, key)? {
        Some(ciborium::value::Value::Text(text)) => Some(text.clone()),
        _ => None,
    })
}

fn cbor_bytes_value(
    map: &[(ciborium::value::Value, ciborium::value::Value)],
    key: &str,
) -> Result<Option<Vec<u8>>, Error> {
    Ok(match cbor_text_key_value(map, key)? {
        Some(ciborium::value::Value::Bytes(bytes)) => Some(bytes.clone()),
        _ => None,
    })
}

fn cbor_map_value<'a>(
    map: &'a [(ciborium::value::Value, ciborium::value::Value)],
    key: &str,
) -> Result<Option<&'a [(ciborium::value::Value, ciborium::value::Value)]>, Error> {
    Ok(match cbor_text_key_value(map, key)? {
        Some(ciborium::value::Value::Map(map)) => Some(map.as_slice()),
        _ => None,
    })
}

fn cbor_array_value<'a>(
    map: &'a [(ciborium::value::Value, ciborium::value::Value)],
    key: &str,
) -> Result<Option<&'a [ciborium::value::Value]>, Error> {
    Ok(match cbor_text_key_value(map, key)? {
        Some(ciborium::value::Value::Array(values)) => Some(values.as_slice()),
        _ => None,
    })
}

fn cbor_int_value(
    map: &[(ciborium::value::Value, ciborium::value::Value)],
    key: i128,
) -> Result<Option<i128>, Error> {
    Ok(match cbor_integer_key_value(map, key)? {
        Some(ciborium::value::Value::Integer(value)) => Some(i128::from(value.clone())),
        _ => None,
    })
}

fn cbor_bytes_value_i(
    map: &[(ciborium::value::Value, ciborium::value::Value)],
    key: i128,
) -> Result<Option<Vec<u8>>, Error> {
    Ok(match cbor_integer_key_value(map, key)? {
        Some(ciborium::value::Value::Bytes(bytes)) => Some(bytes.clone()),
        _ => None,
    })
}

fn decode_cbor_value_exact(
    input: &[u8],
    parse_message: &str,
    trailing_message: &str,
) -> Result<ciborium::value::Value, Error> {
    let mut cursor = Cursor::new(input);
    let value: ciborium::value::Value = ciborium::de::from_reader(&mut cursor)
        .map_err(|_| labeled_invariant("invalid_attestation", parse_message.to_owned()))?;
    if cursor.position() != input.len() as u64 {
        return Err(labeled_invariant("invalid_attestation", trailing_message).into());
    }
    Ok(value)
}

fn read_definite_cbor_header(
    input: &[u8],
    offset: &mut usize,
    source: &str,
) -> Result<(u8, u64), Error> {
    let first = *input.get(*offset).ok_or_else(|| {
        labeled_invariant(
            "invalid_attestation",
            format!("iOS App Attest {source} extensions contain truncated CBOR"),
        )
    })?;
    *offset += 1;
    let major = first >> 5;
    let additional = first & 0x1f;
    let argument_bytes = match additional {
        0..=23 => return Ok((major, u64::from(additional))),
        24 => 1,
        25 => 2,
        26 => 4,
        27 => 8,
        _ => {
            return Err(labeled_invariant(
                "invalid_attestation",
                format!("iOS App Attest {source} extensions must use definite valid CBOR"),
            )
            .into());
        }
    };
    let end = offset.checked_add(argument_bytes).ok_or_else(|| {
        labeled_invariant(
            "invalid_attestation",
            format!("iOS App Attest {source} CBOR length overflows"),
        )
    })?;
    let bytes = input.get(*offset..end).ok_or_else(|| {
        labeled_invariant(
            "invalid_attestation",
            format!("iOS App Attest {source} extensions contain truncated CBOR"),
        )
    })?;
    *offset = end;
    let mut argument = 0u64;
    for byte in bytes {
        argument = (argument << 8) | u64::from(*byte);
    }
    Ok((major, argument))
}

fn read_definite_cbor_text<'a>(
    input: &'a [u8],
    offset: &mut usize,
    source: &str,
) -> Result<&'a str, Error> {
    let (major, length) = read_definite_cbor_header(input, offset, source)?;
    if major != 3 {
        return Err(labeled_invariant(
            "invalid_attestation",
            format!("iOS App Attest {source} extension key/value must be text"),
        )
        .into());
    }
    let length = usize::try_from(length).map_err(|_| {
        labeled_invariant(
            "invalid_attestation",
            format!("iOS App Attest {source} CBOR text length is out of range"),
        )
    })?;
    let end = offset.checked_add(length).ok_or_else(|| {
        labeled_invariant(
            "invalid_attestation",
            format!("iOS App Attest {source} CBOR text length overflows"),
        )
    })?;
    let bytes = input.get(*offset..end).ok_or_else(|| {
        labeled_invariant(
            "invalid_attestation",
            format!("iOS App Attest {source} extensions contain truncated text"),
        )
    })?;
    *offset = end;
    std::str::from_utf8(bytes).map_err(|_| {
        labeled_invariant(
            "invalid_attestation",
            format!("iOS App Attest {source} extension text is not UTF-8"),
        )
        .into()
    })
}
