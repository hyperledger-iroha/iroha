//! Byte-exact canonical query form shared by canonical-request signers and verifiers.
//!
//! The canonical request message carries the request query in one canonical
//! `application/x-www-form-urlencoded` form. The raw query is split on `&`
//! (empty components are ignored), each pair is decoded (`+` and `%XX`) with
//! lossy UTF-8, pairs are sorted by decoded key and then decoded value, and
//! every component is re-encoded with uppercase percent escapes and `+` for a
//! space. The client signer, Torii verifier and CLI signers all use this single
//! implementation, so their bytes cannot drift. Output buffers are sized exactly
//! before any byte is written.

/// Maximum number of non-empty `&`-delimited query pairs in a V1 canonical request.
pub const CANONICAL_REQUEST_MAX_QUERY_PAIRS_V1: usize = 64;

/// Reason a canonical query form could not be planned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanonicalFormError {
    /// The query holds more than [`CANONICAL_REQUEST_MAX_QUERY_PAIRS_V1`] non-empty pairs.
    TooManyPairs,
    /// The canonical byte length does not fit in `usize`.
    Capacity,
}

/// Count the non-empty `&`-delimited pairs of `raw`, stopping one past the V1 limit.
///
/// The count never percent-decodes and never exceeds
/// `CANONICAL_REQUEST_MAX_QUERY_PAIRS_V1 + 1`, so callers can reject oversized
/// queries before planning them.
#[must_use]
pub fn canonical_request_query_pair_count(raw: &str) -> usize {
    raw.as_bytes()
        .split(|byte| *byte == b'&')
        .filter(|pair| !pair.is_empty())
        .take(CANONICAL_REQUEST_MAX_QUERY_PAIRS_V1.saturating_add(1))
        .count()
}

#[derive(Clone, Copy)]
struct RawFormPair<'a> {
    key: &'a [u8],
    value: &'a [u8],
}

/// Sorted, length-checked plan for writing one canonical query form.
pub struct CanonicalRequestFormPlan<'a> {
    pairs: [RawFormPair<'a>; CANONICAL_REQUEST_MAX_QUERY_PAIRS_V1],
    pair_count: usize,
    encoded_bytes: usize,
}

impl<'a> CanonicalRequestFormPlan<'a> {
    /// Split, sort and measure the canonical form of `raw` without allocating.
    ///
    /// # Errors
    /// Returns [`CanonicalFormError::TooManyPairs`] when `raw` exceeds the V1
    /// pair limit and [`CanonicalFormError::Capacity`] when the encoded length
    /// overflows `usize`.
    pub fn new(raw: &'a str) -> Result<Self, CanonicalFormError> {
        let mut pairs = [RawFormPair {
            key: &[],
            value: &[],
        }; CANONICAL_REQUEST_MAX_QUERY_PAIRS_V1];
        let mut pair_count = 0;
        for sequence in raw
            .as_bytes()
            .split(|byte| *byte == b'&')
            .filter(|sequence| !sequence.is_empty())
        {
            if pair_count == pairs.len() {
                return Err(CanonicalFormError::TooManyPairs);
            }
            let separator = sequence
                .iter()
                .position(|byte| *byte == b'=')
                .unwrap_or(sequence.len());
            pairs[pair_count] = RawFormPair {
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
            FormLossyChars::new(left.key)
                .cmp(FormLossyChars::new(right.key))
                .then_with(|| FormLossyChars::new(left.value).cmp(FormLossyChars::new(right.value)))
        });
        let encoded_bytes = pairs[..pair_count]
            .iter()
            .enumerate()
            .try_fold(0_usize, |length, (index, pair)| {
                length
                    .checked_add(usize::from(index != 0))
                    .and_then(|length| {
                        form_component_len(pair.key).and_then(|key| length.checked_add(key))
                    })
                    .and_then(|length| length.checked_add(1))
                    .and_then(|length| {
                        form_component_len(pair.value).and_then(|value| length.checked_add(value))
                    })
            })
            .ok_or(CanonicalFormError::Capacity)?;
        Ok(Self {
            pairs,
            pair_count,
            encoded_bytes,
        })
    }

    /// Exact number of bytes [`Self::write_to`] writes.
    #[inline]
    #[must_use]
    pub fn encoded_bytes(&self) -> usize {
        self.encoded_bytes
    }

    /// Write the canonical form into `writer`, which must have
    /// [`Self::encoded_bytes`] bytes of remaining capacity.
    pub fn write_to(&self, writer: &mut CanonicalRequestExactWriter<'_>) {
        for (index, pair) in self.pairs[..self.pair_count].iter().enumerate() {
            if index != 0 {
                writer.push(b'&');
            }
            write_form_component(pair.key, writer);
            writer.push(b'=');
            write_form_component(pair.value, writer);
        }
    }
}

/// Writer over an exactly pre-sized canonical request buffer.
pub struct CanonicalRequestExactWriter<'a> {
    bytes: &'a mut [u8],
    offset: usize,
}

impl<'a> CanonicalRequestExactWriter<'a> {
    /// Start writing at the beginning of `bytes`.
    #[inline]
    pub fn new(bytes: &'a mut [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    /// Append one byte.
    #[inline]
    pub fn push(&mut self, byte: u8) {
        self.bytes[self.offset] = byte;
        self.offset += 1;
    }

    /// Append a byte slice.
    #[inline]
    pub fn extend(&mut self, bytes: &[u8]) {
        let end = self.offset + bytes.len();
        self.bytes[self.offset..end].copy_from_slice(bytes);
        self.offset = end;
    }

    /// Number of bytes written so far.
    #[inline]
    #[must_use]
    pub fn offset(&self) -> usize {
        self.offset
    }
}

/// Number of ASCII decimal digits in `value`.
#[inline]
#[must_use]
pub fn canonical_request_decimal_len(mut value: u64) -> usize {
    let mut length = 1;
    while value >= 10 {
        value /= 10;
        length += 1;
    }
    length
}

/// Write `value` as ASCII decimal digits.
#[inline]
pub fn write_canonical_request_decimal(
    mut value: u64,
    writer: &mut CanonicalRequestExactWriter<'_>,
) {
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

#[derive(Clone)]
struct FormDecodedBytes<'a> {
    raw: &'a [u8],
    index: usize,
}

impl Iterator for FormDecodedBytes<'_> {
    type Item = u8;

    #[inline]
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
                    .and_then(|byte| hex_nibble(*byte)),
                self.raw
                    .get(self.index + 2)
                    .and_then(|byte| hex_nibble(*byte)),
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
struct FormLossyChars<'a> {
    bytes: FormDecodedBytes<'a>,
}

impl<'a> FormLossyChars<'a> {
    #[inline]
    fn new(raw: &'a [u8]) -> Self {
        Self {
            bytes: FormDecodedBytes { raw, index: 0 },
        }
    }

    #[inline]
    fn advance(&mut self, bytes: usize) {
        for _ in 0..bytes {
            let _ = self.bytes.next();
        }
    }
}

impl Iterator for FormLossyChars<'_> {
    type Item = char;

    #[inline]
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

#[inline]
const fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[inline]
const fn form_byte_len(byte: u8) -> usize {
    match byte {
        b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'*' | b'-' | b'.' | b'_' | b' ' => 1,
        _ => 3,
    }
}

fn form_component_len(raw: &[u8]) -> Option<usize> {
    FormLossyChars::new(raw).try_fold(0_usize, |mut length, ch| {
        let mut encoded = [0_u8; 4];
        for byte in ch.encode_utf8(&mut encoded).as_bytes() {
            length = length.checked_add(form_byte_len(*byte))?;
        }
        Some(length)
    })
}

fn write_form_component(raw: &[u8], writer: &mut CanonicalRequestExactWriter<'_>) {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    for ch in FormLossyChars::new(raw) {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn canonical(raw: &str) -> Result<String, CanonicalFormError> {
        let plan = CanonicalRequestFormPlan::new(raw)?;
        let mut output = vec![0_u8; plan.encoded_bytes()];
        let mut writer = CanonicalRequestExactWriter::new(&mut output);
        plan.write_to(&mut writer);
        assert_eq!(writer.offset(), plan.encoded_bytes());
        Ok(String::from_utf8(output).expect("canonical form is ASCII"))
    }

    #[test]
    fn sorts_decodes_and_reencodes_pairs() {
        assert_eq!(canonical("").unwrap(), "");
        assert_eq!(canonical("b=2&a=1").unwrap(), "a=1&b=2");
        assert_eq!(canonical("a=2&a=1").unwrap(), "a=1&a=2");
        assert_eq!(canonical("&&k&&").unwrap(), "k=");
        assert_eq!(canonical("q=a%20b+c").unwrap(), "q=a+b+c");
        assert_eq!(canonical("q=%7e%2a").unwrap(), "q=%7E*");
        assert_eq!(canonical("q=%zz").unwrap(), "q=%25zz");
        assert_eq!(canonical("q=%E2%82%AC").unwrap(), "q=%E2%82%AC");
        assert_eq!(canonical("q=%FF").unwrap(), "q=%EF%BF%BD");
    }

    #[test]
    fn lossy_chars_preserve_a_valid_prefix_before_invalid_utf8() {
        assert_eq!(
            FormLossyChars::new(b"%E2%82%AC%FF").collect::<String>(),
            "\u{20ac}\u{fffd}"
        );
    }

    #[test]
    fn enforces_the_v1_pair_limit() {
        let exact = std::iter::repeat_n("k=v", CANONICAL_REQUEST_MAX_QUERY_PAIRS_V1)
            .collect::<Vec<_>>()
            .join("&");
        assert!(CanonicalRequestFormPlan::new(&exact).is_ok());
        assert_eq!(
            canonical_request_query_pair_count(&exact),
            CANONICAL_REQUEST_MAX_QUERY_PAIRS_V1
        );
        let over = format!("{exact}&extra=v");
        assert_eq!(
            CanonicalRequestFormPlan::new(&over).err(),
            Some(CanonicalFormError::TooManyPairs)
        );
        assert_eq!(
            canonical_request_query_pair_count(&format!("{over}&more=v")),
            CANONICAL_REQUEST_MAX_QUERY_PAIRS_V1 + 1
        );
    }

    #[test]
    fn decimal_helpers_are_exact() {
        for value in [0, 9, 10, 99, 100, u64::MAX] {
            let expected = value.to_string();
            assert_eq!(canonical_request_decimal_len(value), expected.len());
            let mut output = vec![0_u8; expected.len()];
            let mut writer = CanonicalRequestExactWriter::new(&mut output);
            write_canonical_request_decimal(value, &mut writer);
            assert_eq!(output, expected.as_bytes());
        }
    }
}
