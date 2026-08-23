#[derive(Clone, Copy)]
struct CanonicalRequestRawFormPair<'a> {
    key: &'a [u8],
    value: &'a [u8],
}

struct CanonicalRequestFormPlan<'a> {
    pairs: [CanonicalRequestRawFormPair<'a>; CANONICAL_REQUEST_MAX_QUERY_PAIRS_V1],
    pair_count: usize,
    encoded_bytes: usize,
}

#[derive(Clone)]
struct CanonicalRequestFormDecodedBytes<'a> {
    raw: &'a [u8],
    index: usize,
}

impl<'a> CanonicalRequestFormDecodedBytes<'a> {
    fn new(raw: &'a [u8]) -> Self {
        Self { raw, index: 0 }
    }
}

impl Iterator for CanonicalRequestFormDecodedBytes<'_> {
    type Item = u8;

    fn next(&mut self) -> Option<Self::Item> {
        let byte = *self.raw.get(self.index)?;
        if byte == b'+' {
            self.index += 1;
            return Some(b' ');
        }
        if byte == b'%'
            && let (Some(high), Some(low)) = (
                self.raw
                    .get(self.index + 1)
                    .and_then(|byte| canonical_request_hex_nibble(*byte)),
                self.raw
                    .get(self.index + 2)
                    .and_then(|byte| canonical_request_hex_nibble(*byte)),
            )
        {
            self.index += 3;
            return Some((high << 4) | low);
        }
        self.index += 1;
        Some(byte)
    }
}

#[derive(Clone)]
struct CanonicalRequestFormLossyChars<'a> {
    bytes: CanonicalRequestFormDecodedBytes<'a>,
}

impl<'a> CanonicalRequestFormLossyChars<'a> {
    fn new(raw: &'a [u8]) -> Self {
        Self {
            bytes: CanonicalRequestFormDecodedBytes::new(raw),
        }
    }

    fn advance(&mut self, bytes: usize) {
        for _ in 0..bytes {
            let _ = self.bytes.next();
        }
    }
}

impl Iterator for CanonicalRequestFormLossyChars<'_> {
    type Item = char;

    fn next(&mut self) -> Option<Self::Item> {
        let mut probe = self.bytes.clone();
        let mut encoded = [0_u8; 4];
        let mut length = 0;
        while length < encoded.len() {
            let Some(byte) = probe.next() else {
                break;
            };
            encoded[length] = byte;
            length += 1;
        }
        if length == 0 {
            return None;
        }
        match std::str::from_utf8(&encoded[..length]) {
            Ok(valid) => {
                let ch = valid.chars().next().expect("non-empty UTF-8 probe");
                self.advance(ch.len_utf8());
                Some(ch)
            }
            Err(error) if error.valid_up_to() != 0 => {
                let valid = std::str::from_utf8(&encoded[..error.valid_up_to()])
                    .expect("UTF-8 validation guarantees its reported prefix is valid");
                let ch = valid.chars().next().expect("non-empty valid UTF-8 prefix");
                self.advance(ch.len_utf8());
                Some(ch)
            }
            Err(error) => {
                self.advance(error.error_len().unwrap_or(length));
                Some(char::REPLACEMENT_CHARACTER)
            }
        }
    }
}

const fn canonical_request_hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

const fn canonical_request_form_byte_len(byte: u8) -> usize {
    match byte {
        b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'*' | b'-' | b'.' | b'_' | b' ' => 1,
        _ => 3,
    }
}

fn canonical_request_form_component_len(raw: &[u8]) -> Option<usize> {
    CanonicalRequestFormLossyChars::new(raw).try_fold(0_usize, |mut length, ch| {
        let mut encoded = [0_u8; 4];
        for byte in ch.encode_utf8(&mut encoded).as_bytes() {
            length = length.checked_add(canonical_request_form_byte_len(*byte))?;
        }
        Some(length)
    })
}

impl<'a> CanonicalRequestFormPlan<'a> {
    fn new(raw: &'a str) -> Result<Self> {
        validate_canonical_request_raw_query(raw)?;
        let mut pairs = [CanonicalRequestRawFormPair {
            key: &[],
            value: &[],
        }; CANONICAL_REQUEST_MAX_QUERY_PAIRS_V1];
        let mut pair_count = 0;
        for sequence in raw
            .as_bytes()
            .split(|byte| *byte == b'&')
            .filter(|sequence| !sequence.is_empty())
        {
            let separator = sequence
                .iter()
                .position(|byte| *byte == b'=')
                .unwrap_or(sequence.len());
            pairs[pair_count] = CanonicalRequestRawFormPair {
                key: &sequence[..separator],
                value: if separator < sequence.len() {
                    &sequence[separator + 1..]
                } else {
                    &[]
                },
            };
            pair_count += 1;
        }
        pairs[..pair_count].sort_unstable_by(|left, right| {
            CanonicalRequestFormLossyChars::new(left.key)
                .cmp(CanonicalRequestFormLossyChars::new(right.key))
                .then_with(|| {
                    CanonicalRequestFormLossyChars::new(left.value)
                        .cmp(CanonicalRequestFormLossyChars::new(right.value))
                })
        });
        let encoded_bytes = pairs[..pair_count]
            .iter()
            .enumerate()
            .try_fold(0_usize, |length, (index, pair)| {
                length
                    .checked_add(usize::from(index != 0))
                    .and_then(|length| {
                        canonical_request_form_component_len(pair.key)
                            .and_then(|key| length.checked_add(key))
                    })
                    .and_then(|length| length.checked_add(1))
                    .and_then(|length| {
                        canonical_request_form_component_len(pair.value)
                            .and_then(|value| length.checked_add(value))
                    })
            })
            .ok_or_else(canonical_request_capacity_error)?;
        Ok(Self {
            pairs,
            pair_count,
            encoded_bytes,
        })
    }

    fn write_to(&self, writer: &mut CanonicalRequestExactWriter<'_>) {
        for (index, pair) in self.pairs[..self.pair_count].iter().enumerate() {
            if index != 0 {
                writer.push(b'&');
            }
            write_canonical_request_form_component(pair.key, writer);
            writer.push(b'=');
            write_canonical_request_form_component(pair.value, writer);
        }
    }
}

fn write_canonical_request_form_component(
    raw: &[u8],
    writer: &mut CanonicalRequestExactWriter<'_>,
) {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    for ch in CanonicalRequestFormLossyChars::new(raw) {
        let mut encoded = [0_u8; 4];
        for byte in ch.encode_utf8(&mut encoded).as_bytes() {
            match *byte {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'*' | b'-' | b'.' | b'_' => {
                    writer.push(*byte);
                }
                b' ' => writer.push(b'+'),
                byte => {
                    writer.push(b'%');
                    writer.push(HEX[usize::from(byte >> 4)]);
                    writer.push(HEX[usize::from(byte & 0x0f)]);
                }
            }
        }
    }
}

struct CanonicalRequestExactWriter<'a> {
    bytes: &'a mut [u8],
    offset: usize,
}

impl<'a> CanonicalRequestExactWriter<'a> {
    fn new(bytes: &'a mut [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn push(&mut self, byte: u8) {
        self.bytes[self.offset] = byte;
        self.offset += 1;
    }

    fn extend(&mut self, bytes: &[u8]) {
        let end = self.offset + bytes.len();
        self.bytes[self.offset..end].copy_from_slice(bytes);
        self.offset = end;
    }
}

fn allocate_exact_canonical_request_bytes(length: usize) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| eyre!("failed to reserve {length} canonical request bytes"))?;
    bytes.resize(length, 0);
    Ok(bytes)
}

#[cfg(test)]
fn canonical_query_string_v1(raw: Option<&str>) -> Result<String> {
    let plan = CanonicalRequestFormPlan::new(raw.unwrap_or_default())?;
    let mut output = allocate_exact_canonical_request_bytes(plan.encoded_bytes)?;
    let mut writer = CanonicalRequestExactWriter::new(&mut output);
    plan.write_to(&mut writer);
    debug_assert_eq!(writer.offset, plan.encoded_bytes);
    String::from_utf8(output).map_err(|_| eyre!("canonical request query is not valid UTF-8"))
}

fn validate_canonical_request_raw_query(raw: &str) -> Result<()> {
    if raw.len() > CANONICAL_REQUEST_MAX_RAW_QUERY_BYTES_V1 {
        return Err(eyre!(
            "canonical request query exceeds the V1 limit of {CANONICAL_REQUEST_MAX_RAW_QUERY_BYTES_V1} raw bytes"
        ));
    }
    let pair_count = raw
        .as_bytes()
        .split(|byte| *byte == b'&')
        .filter(|pair| !pair.is_empty())
        .take(CANONICAL_REQUEST_MAX_QUERY_PAIRS_V1.saturating_add(1))
        .count();
    if pair_count > CANONICAL_REQUEST_MAX_QUERY_PAIRS_V1 {
        return Err(eyre!(
            "canonical request query exceeds the V1 limit of {CANONICAL_REQUEST_MAX_QUERY_PAIRS_V1} pairs"
        ));
    }
    Ok(())
}

fn validate_canonical_request_target(method: &HttpMethod, url: &Url) -> Result<()> {
    if method.as_str().len() > CANONICAL_REQUEST_MAX_METHOD_BYTES_V1 {
        return Err(eyre!(
            "canonical request method exceeds the V1 limit of {CANONICAL_REQUEST_MAX_METHOD_BYTES_V1} bytes"
        ));
    }
    if url.path().len() > CANONICAL_REQUEST_MAX_PATH_BYTES_V1 {
        return Err(eyre!(
            "canonical request path exceeds the V1 limit of {CANONICAL_REQUEST_MAX_PATH_BYTES_V1} bytes"
        ));
    }
    validate_canonical_request_raw_query(url.query().unwrap_or_default())
}

fn canonical_request_capacity_error() -> eyre::Report {
    eyre!("canonical request byte length exceeds platform capacity")
}

fn canonical_request_decimal_len(mut value: u64) -> usize {
    let mut length = 1;
    while value >= 10 {
        value /= 10;
        length += 1;
    }
    length
}

fn write_canonical_request_decimal(mut value: u64, writer: &mut CanonicalRequestExactWriter<'_>) {
    let mut digits = [0_u8; 20];
    let mut start = digits.len();
    loop {
        start -= 1;
        digits[start] = b'0' + u8::try_from(value % 10).expect("decimal digit fits in u8");
        value /= 10;
        if value == 0 {
            break;
        }
    }
    writer.extend(&digits[start..]);
}

fn canonical_request_nonce_is_valid(nonce: &str) -> bool {
    !nonce.is_empty()
        && nonce.len() <= CANONICAL_REQUEST_MAX_NONCE_BYTES_V1
        && nonce.bytes().all(|byte| (0x21..=0x7e).contains(&byte))
}

fn bounded_network_request_message(
    domain: &[u8],
    network_id: &NetworkId,
    method: &HttpMethod,
    url: &Url,
    body: &[u8],
    freshness: Option<(u64, &str)>,
) -> Result<Vec<u8>> {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    validate_canonical_request_target(method, url)?;
    if let Some((_, nonce)) = freshness
        && !canonical_request_nonce_is_valid(nonce)
    {
        return Err(eyre!("invalid canonical request nonce"));
    }
    let query = CanonicalRequestFormPlan::new(url.query().unwrap_or_default())?;
    let freshness_bytes = if let Some((timestamp_ms, nonce)) = freshness {
        1_usize
            .checked_add(canonical_request_decimal_len(timestamp_ms))
            .and_then(|length| length.checked_add(1))
            .and_then(|length| length.checked_add(nonce.len()))
            .ok_or_else(canonical_request_capacity_error)?
    } else {
        0
    };
    let total_bytes = domain
        .len()
        .checked_add(network_id.as_bytes().len())
        .and_then(|length| length.checked_add(method.as_str().len()))
        .and_then(|length| length.checked_add(1))
        .and_then(|length| length.checked_add(url.path().len()))
        .and_then(|length| length.checked_add(1))
        .and_then(|length| length.checked_add(query.encoded_bytes))
        .and_then(|length| length.checked_add(1 + 64))
        .and_then(|length| length.checked_add(freshness_bytes))
        .ok_or_else(canonical_request_capacity_error)?;
    let mut output = allocate_exact_canonical_request_bytes(total_bytes)?;
    let mut writer = CanonicalRequestExactWriter::new(&mut output);
    writer.extend(domain);
    writer.extend(network_id.as_bytes());
    for byte in method.as_str().bytes() {
        writer.push(byte.to_ascii_uppercase());
    }
    writer.push(b'\n');
    writer.extend(url.path().as_bytes());
    writer.push(b'\n');
    query.write_to(&mut writer);
    writer.push(b'\n');
    let body_hash = Sha256::digest(body);
    for byte in body_hash {
        writer.push(HEX[usize::from(byte >> 4)]);
        writer.push(HEX[usize::from(byte & 0x0f)]);
    }
    if let Some((timestamp_ms, nonce)) = freshness {
        writer.push(b'\n');
        write_canonical_request_decimal(timestamp_ms, &mut writer);
        writer.push(b'\n');
        writer.extend(nonce.as_bytes());
    }
    debug_assert_eq!(writer.offset, total_bytes);
    Ok(output)
}

/// Construct exact-network canonical V1 request bytes for signing or hashing.
///
/// The envelope binds the V1 domain and exact genesis-derived network, an
/// uppercase method, percent-encoded path, canonical query, and lowercase
/// SHA-256 body digest. Query components are form-decoded (`+` is space),
/// compared as lossy UTF-8 `(key, value)` pairs, and form-encoded with only
/// ASCII alphanumerics plus `*`, `-`, `.`, and `_` left unescaped.
///
/// # Errors
/// Returns an error when the method, path, or query exceeds the V1 bounds or
/// the exact output allocation fails.
pub fn canonical_network_request_message(
    network_id: &NetworkId,
    method: &HttpMethod,
    url: &Url,
    body: &[u8],
) -> Result<Vec<u8>> {
    bounded_network_request_message(
        b"iroha.app.request.network.v1\0",
        network_id,
        method,
        url,
        body,
        None,
    )
}

/// Hash one exact-network canonical V1 request for a multisignature witness.
///
/// # Errors
/// Returns an error when canonical request construction fails.
pub fn canonical_network_request_hash(
    network_id: &NetworkId,
    method: &HttpMethod,
    url: &Url,
    body: &[u8],
) -> Result<Hash> {
    canonical_network_request_message(network_id, method, url, body)
        .map(|message| Hash::new(&message))
}

/// Construct exact-network canonical V1 request bytes with freshness metadata.
///
/// # Errors
/// Returns an error when the target or nonce exceeds the V1 bounds or the
/// exact output allocation fails.
pub fn canonical_network_request_signature_message(
    network_id: &NetworkId,
    method: &HttpMethod,
    url: &Url,
    body: &[u8],
    timestamp_ms: u64,
    nonce: &str,
) -> Result<Vec<u8>> {
    bounded_network_request_message(
        b"iroha.app.request.network.v1\0",
        network_id,
        method,
        url,
        body,
        Some((timestamp_ms, nonce)),
    )
}

/// Render the strict ASCII account value used by canonical V1 auth headers.
///
/// # Errors
/// Returns an error when canonical address conversion fails or the resulting
/// literal exceeds the V1 account bound.
pub fn canonical_request_account_header_value(account: &AccountId) -> Result<String> {
    validate_canonical_request_account_encoded_size(account)?;
    let value = account
        .to_canonical_hex()
        .wrap_err("failed to encode canonical request account header")?;
    validate_canonical_request_account_literal(&value)?;
    Ok(value)
}

fn validate_canonical_request_account_encoded_size(account: &AccountId) -> Result<()> {
    const MAX_CANONICAL_BYTES: usize =
        (CANONICAL_REQUEST_MAX_ACCOUNT_LITERAL_BYTES_V1 - "0x".len()) / 2;
    let add = |total: &mut usize, bytes: usize| -> Result<()> {
        *total = total
            .checked_add(bytes)
            .ok_or_else(canonical_request_capacity_error)?;
        if *total > MAX_CANONICAL_BYTES {
            return Err(eyre!(
                "canonical request account exceeds the V1 limit of {CANONICAL_REQUEST_MAX_ACCOUNT_LITERAL_BYTES_V1} bytes"
            ));
        }
        Ok(())
    };
    // One byte is the address-class/domain header. The remaining lengths are
    // the exact `AccountAddress` controller wire prefixes.
    let mut canonical_bytes = 1_usize;
    match account.controller() {
        iroha_data_model::account::AccountController::Single(public_key) => {
            let (_, payload) = public_key
                .try_to_bytes()
                .wrap_err("canonical request account contains a malformed public key")?;
            add(
                &mut canonical_bytes,
                if u8::try_from(payload.len()).is_ok() {
                    3
                } else {
                    4
                },
            )?;
            add(&mut canonical_bytes, payload.len())?;
        }
        iroha_data_model::account::AccountController::Multisig(policy) => {
            add(&mut canonical_bytes, 6)?;
            for member in policy.members() {
                let (_, payload) = member
                    .public_key()
                    .try_to_bytes()
                    .wrap_err("canonical request account contains a malformed multisig key")?;
                add(&mut canonical_bytes, 5)?;
                add(&mut canonical_bytes, payload.len())?;
            }
        }
    }
    Ok(())
}

fn validate_canonical_request_account_literal(value: &str) -> Result<()> {
    if value.len() > CANONICAL_REQUEST_MAX_ACCOUNT_LITERAL_BYTES_V1 {
        return Err(eyre!(
            "canonical request account exceeds the V1 limit of {CANONICAL_REQUEST_MAX_ACCOUNT_LITERAL_BYTES_V1} bytes"
        ));
    }
    let Some(payload) = value.strip_prefix("0x") else {
        return Err(eyre!(
            "canonical request account header is not lowercase ASCII hexadecimal"
        ));
    };
    if payload.is_empty()
        || payload.len() % 2 != 0
        || !value.is_ascii()
        || !payload
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(eyre!(
            "canonical request account header is not lowercase ASCII hexadecimal"
        ));
    }
    Ok(())
}

/// Render one canonical V1 timestamp header without an infallible allocation.
///
/// # Errors
/// Returns an error if the exact decimal destination cannot be allocated.
pub fn canonical_request_timestamp_header_value(timestamp_ms: u64) -> Result<String> {
    let length = canonical_request_decimal_len(timestamp_ms);
    let mut output = allocate_exact_canonical_request_bytes(length)?;
    let mut writer = CanonicalRequestExactWriter::new(&mut output);
    write_canonical_request_decimal(timestamp_ms, &mut writer);
    debug_assert_eq!(writer.offset, length);
    String::from_utf8(output).map_err(|_| eyre!("canonical request timestamp is not valid UTF-8"))
}

fn encode_bounded_canonical_base64_value(
    bytes: &[u8],
    maximum_decoded_bytes: usize,
    context: &'static str,
) -> Result<String> {
    if bytes.len() > maximum_decoded_bytes {
        return Err(eyre!(
            "{context} exceeds the V1 limit of {maximum_decoded_bytes} decoded bytes"
        ));
    }
    let encoded_len = bytes
        .len()
        .checked_add(2)
        .map(|length| length / 3)
        .and_then(|length| length.checked_mul(4))
        .ok_or_else(canonical_request_capacity_error)?;
    let mut encoded = allocate_exact_canonical_request_bytes(encoded_len)?;
    let written = base64::engine::general_purpose::STANDARD
        .encode_slice(bytes, &mut encoded)
        .map_err(|_| eyre!("failed to encode {context} as canonical base64"))?;
    if written != encoded_len {
        return Err(eyre!("canonical base64 length mismatch for {context}"));
    }
    String::from_utf8(encoded).map_err(|_| eyre!("canonical base64 for {context} is not UTF-8"))
}

/// Encode a checked detached signature for a canonical V1 request header.
///
/// # Errors
/// Returns an error for an empty, all-zero, or excessive signature payload or
/// when the exact base64 destination cannot be allocated.
pub fn canonical_request_signature_header_value(signature: &Signature) -> Result<String> {
    let payload = signature.payload();
    if payload.is_empty()
        || payload.len() > CANONICAL_REQUEST_MAX_SIGNATURE_BYTES_V1
        || payload.iter().all(|byte| *byte == 0)
    {
        return Err(eyre!("invalid canonical request signature"));
    }
    encode_bounded_canonical_base64_value(
        payload,
        CANONICAL_REQUEST_MAX_SIGNATURE_BYTES_V1,
        "canonical request signature",
    )
}

fn validate_canonical_request_witness_for_encoding(
    witness: &CanonicalRequestWitnessV1,
) -> Result<()> {
    if witness.schema_version != CANONICAL_REQUEST_WITNESS_VERSION_V1 {
        return Err(eyre!(
            "unsupported canonical request witness schema version"
        ));
    }
    if !canonical_request_nonce_is_valid(&witness.nonce) {
        return Err(eyre!("invalid canonical request witness nonce"));
    }
    if witness.signatures.len() > CANONICAL_REQUEST_WITNESS_MAX_SIGNATURES_V1 {
        return Err(eyre!(
            "canonical request witness exceeds the V1 limit of {CANONICAL_REQUEST_WITNESS_MAX_SIGNATURES_V1} signatures"
        ));
    }
    for signature in &witness.signatures {
        let payload = signature.signature.payload();
        if payload.is_empty()
            || payload.len() > CANONICAL_REQUEST_MAX_SIGNATURE_BYTES_V1
            || payload.iter().all(|byte| *byte == 0)
        {
            return Err(eyre!("invalid canonical request witness signature"));
        }
    }
    Ok(())
}

/// Construct the exact canonical V1 payload signed by every request-witness member.
///
/// The signature vector is intentionally excluded: each detached signer binds the
/// subject, freshness fields, and exact canonical request hash, then an assembler
/// may add the independently produced signatures to the witness header.
///
/// # Errors
/// Returns an error for an invalid witness envelope or a failed bounded encoding.
pub fn canonical_request_witness_message(
    witness: &CanonicalRequestWitnessV1,
) -> Result<Vec<u8>> {
    #[derive(norito::derive::Encode)]
    struct CanonicalRequestWitnessPayloadV1 {
        schema_version: u16,
        subject_account: AccountId,
        timestamp_ms: u64,
        nonce: String,
        canonical_request_hash: Hash,
    }

    validate_canonical_request_witness_for_encoding(witness)?;
    let payload = CanonicalRequestWitnessPayloadV1 {
        schema_version: witness.schema_version,
        subject_account: witness.subject_account.clone(),
        timestamp_ms: witness.timestamp_ms,
        nonce: witness.nonce.clone(),
        canonical_request_hash: witness.canonical_request_hash,
    };
    let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    norito::core::to_bytes_bounded(
        &payload,
        CANONICAL_REQUEST_WITNESS_MAX_DECODED_BYTES_V1,
    )
    .wrap_err("failed to encode bounded canonical request witness message")
}

/// Encode one bounded canonical V1 multisignature witness header.
///
/// # Errors
/// Returns an error for invalid nonce, schema, signature-count, or signature
/// payload bounds, an excessive encoded witness, or a failed exact allocation.
pub fn canonical_request_witness_header_value(
    witness: &CanonicalRequestWitnessV1,
) -> Result<String> {
    validate_canonical_request_witness_for_encoding(witness)?;
    let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let bytes =
        norito::core::to_bytes_bounded(witness, CANONICAL_REQUEST_WITNESS_MAX_DECODED_BYTES_V1)
            .wrap_err("failed to encode bounded canonical request witness")?;
    encode_bounded_canonical_base64_value(
        &bytes,
        CANONICAL_REQUEST_WITNESS_MAX_DECODED_BYTES_V1,
        "canonical request witness",
    )
}
