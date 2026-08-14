const REPUTATION_CANONICAL_MAX_QUERY_PAIRS_V1: usize = 64;
const REPUTATION_CANONICAL_MAX_RAW_QUERY_BYTES_V1: usize = 64 * 1024;
const REPUTATION_CANONICAL_MAX_PATH_BYTES_V1: usize = 64 * 1024;
const REPUTATION_CANONICAL_MAX_NONCE_BYTES_V1: usize = 256;

#[derive(Clone, Copy)]
struct ReputationRawFormPair<'a> {
    key: &'a [u8],
    value: &'a [u8],
}

struct ReputationFormPlan<'a> {
    pairs: [ReputationRawFormPair<'a>; REPUTATION_CANONICAL_MAX_QUERY_PAIRS_V1],
    pair_count: usize,
    encoded_bytes: usize,
}

#[derive(Clone)]
struct ReputationFormDecodedBytes<'a> {
    raw: &'a [u8],
    index: usize,
}

impl<'a> ReputationFormDecodedBytes<'a> {
    fn new(raw: &'a [u8]) -> Self {
        Self { raw, index: 0 }
    }
}

impl Iterator for ReputationFormDecodedBytes<'_> {
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
                    .and_then(|byte| reputation_hex_nibble(*byte)),
                self.raw
                    .get(self.index + 2)
                    .and_then(|byte| reputation_hex_nibble(*byte)),
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
struct ReputationFormLossyChars<'a> {
    bytes: ReputationFormDecodedBytes<'a>,
}

impl<'a> ReputationFormLossyChars<'a> {
    fn new(raw: &'a [u8]) -> Self {
        Self {
            bytes: ReputationFormDecodedBytes::new(raw),
        }
    }

    fn advance(&mut self, bytes: usize) {
        for _ in 0..bytes {
            let _ = self.bytes.next();
        }
    }
}

impl Iterator for ReputationFormLossyChars<'_> {
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

const fn reputation_hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

const fn reputation_form_byte_len(byte: u8) -> usize {
    match byte {
        b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'*' | b'-' | b'.' | b'_' | b' ' => 1,
        _ => 3,
    }
}

fn reputation_form_component_len(raw: &[u8]) -> Option<usize> {
    ReputationFormLossyChars::new(raw).try_fold(0_usize, |mut length, ch| {
        let mut encoded = [0_u8; 4];
        for byte in ch.encode_utf8(&mut encoded).as_bytes() {
            length = length.checked_add(reputation_form_byte_len(*byte))?;
        }
        Some(length)
    })
}

impl<'a> ReputationFormPlan<'a> {
    fn new(raw: &'a str) -> Result<Self, String> {
        if raw.len() > REPUTATION_CANONICAL_MAX_RAW_QUERY_BYTES_V1 {
            return Err("reputation request query exceeds the canonical V1 byte limit".to_owned());
        }
        let mut pairs = [ReputationRawFormPair {
            key: &[],
            value: &[],
        }; REPUTATION_CANONICAL_MAX_QUERY_PAIRS_V1];
        let mut pair_count = 0;
        for sequence in raw
            .as_bytes()
            .split(|byte| *byte == b'&')
            .filter(|sequence| !sequence.is_empty())
        {
            if pair_count == pairs.len() {
                return Err(
                    "reputation request query exceeds the canonical V1 pair limit".to_owned(),
                );
            }
            let separator = sequence
                .iter()
                .position(|byte| *byte == b'=')
                .unwrap_or(sequence.len());
            pairs[pair_count] = ReputationRawFormPair {
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
            ReputationFormLossyChars::new(left.key)
                .cmp(ReputationFormLossyChars::new(right.key))
                .then_with(|| {
                    ReputationFormLossyChars::new(left.value)
                        .cmp(ReputationFormLossyChars::new(right.value))
                })
        });
        let encoded_bytes = pairs[..pair_count]
            .iter()
            .enumerate()
            .try_fold(0_usize, |length, (index, pair)| {
                length
                    .checked_add(usize::from(index != 0))
                    .and_then(|length| {
                        reputation_form_component_len(pair.key)
                            .and_then(|key| length.checked_add(key))
                    })
                    .and_then(|length| length.checked_add(1))
                    .and_then(|length| {
                        reputation_form_component_len(pair.value)
                            .and_then(|value| length.checked_add(value))
                    })
            })
            .ok_or_else(|| "canonical reputation query length overflow".to_owned())?;
        Ok(Self {
            pairs,
            pair_count,
            encoded_bytes,
        })
    }

    fn write_to(&self, writer: &mut ReputationExactWriter<'_>) {
        for (index, pair) in self.pairs[..self.pair_count].iter().enumerate() {
            if index != 0 {
                writer.push(b'&');
            }
            write_reputation_form_component(pair.key, writer);
            writer.push(b'=');
            write_reputation_form_component(pair.value, writer);
        }
    }
}

fn write_reputation_form_component(raw: &[u8], writer: &mut ReputationExactWriter<'_>) {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    for ch in ReputationFormLossyChars::new(raw) {
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

struct ReputationExactWriter<'a> {
    bytes: &'a mut [u8],
    offset: usize,
}

impl<'a> ReputationExactWriter<'a> {
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

fn reputation_decimal_len(mut value: u64) -> usize {
    let mut length = 1;
    while value >= 10 {
        value /= 10;
        length += 1;
    }
    length
}

fn write_reputation_decimal(mut value: u64, writer: &mut ReputationExactWriter<'_>) {
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

fn canonical_reputation_request_message(
    network_id: &NetworkId,
    endpoint: &Url,
    timestamp_ms: u64,
    nonce: &str,
) -> Result<Vec<u8>, String> {
    const DOMAIN: &[u8] = b"iroha.app.request.network.v1\0";
    const EMPTY_BODY_HASH: &[u8; 64] =
        b"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    if endpoint.path().len() > REPUTATION_CANONICAL_MAX_PATH_BYTES_V1 {
        return Err("reputation request path exceeds the canonical V1 byte limit".to_owned());
    }
    if nonce.is_empty()
        || nonce.len() > REPUTATION_CANONICAL_MAX_NONCE_BYTES_V1
        || !nonce.bytes().all(|byte| (0x21..=0x7e).contains(&byte))
    {
        return Err("reputation request nonce is outside canonical V1 bounds".to_owned());
    }
    let query = ReputationFormPlan::new(endpoint.query().unwrap_or_default())?;
    let total_bytes = DOMAIN
        .len()
        .checked_add(network_id.as_bytes().len())
        .and_then(|length| length.checked_add(b"GET\n".len()))
        .and_then(|length| length.checked_add(endpoint.path().len()))
        .and_then(|length| length.checked_add(1))
        .and_then(|length| length.checked_add(query.encoded_bytes))
        .and_then(|length| length.checked_add(1 + EMPTY_BODY_HASH.len() + 1))
        .and_then(|length| length.checked_add(reputation_decimal_len(timestamp_ms)))
        .and_then(|length| length.checked_add(1 + nonce.len()))
        .ok_or_else(|| "canonical reputation request length overflow".to_owned())?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(total_bytes)
        .map_err(|_| "failed to allocate the canonical reputation request".to_owned())?;
    output.resize(total_bytes, 0);
    let mut writer = ReputationExactWriter::new(&mut output);
    writer.extend(DOMAIN);
    writer.extend(network_id.as_bytes());
    writer.extend(b"GET\n");
    writer.extend(endpoint.path().as_bytes());
    writer.push(b'\n');
    query.write_to(&mut writer);
    writer.push(b'\n');
    writer.extend(EMPTY_BODY_HASH);
    writer.push(b'\n');
    write_reputation_decimal(timestamp_ms, &mut writer);
    writer.push(b'\n');
    writer.extend(nonce.as_bytes());
    debug_assert_eq!(writer.offset, total_bytes);
    Ok(output)
}
