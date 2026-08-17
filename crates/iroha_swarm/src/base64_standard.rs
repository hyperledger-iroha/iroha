//! Canonical RFC 4648 standard-alphabet Base64 encoding.
const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
fn symbol(index: u8) -> char {
    char::from(ALPHABET[usize::from(index)])
}
/// Encodes bytes with the RFC 4648 standard alphabet and mandatory padding.
pub fn encode(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len().div_ceil(3).saturating_mul(4));
    let mut chunks = bytes.chunks_exact(3);
    for chunk in &mut chunks {
        encoded.push(symbol(chunk[0] >> 2));
        encoded.push(symbol(((chunk[0] & 0x03) << 4) | (chunk[1] >> 4)));
        encoded.push(symbol(((chunk[1] & 0x0f) << 2) | (chunk[2] >> 6)));
        encoded.push(symbol(chunk[2] & 0x3f));
    }
    match chunks.remainder() {
        [] => {}
        [first] => {
            encoded.push(symbol(*first >> 2));
            encoded.push(symbol((*first & 0x03) << 4));
            encoded.push('=');
            encoded.push('=');
        }
        [first, second] => {
            encoded.push(symbol(*first >> 2));
            encoded.push(symbol(((*first & 0x03) << 4) | (*second >> 4)));
            encoded.push(symbol((*second & 0x0f) << 2));
            encoded.push('=');
        }
        _ => unreachable!("chunks_exact(3) leaves at most two bytes"),
    }
    encoded
}
#[cfg(test)]
mod tests {
    use super::encode;
    #[test]
    fn matches_rfc_4648_test_vectors() {
        for (plain, expected) in [
            (b"".as_slice(), ""),
            (b"f".as_slice(), "Zg=="),
            (b"fo".as_slice(), "Zm8="),
            (b"foo".as_slice(), "Zm9v"),
            (b"foob".as_slice(), "Zm9vYg=="),
            (b"fooba".as_slice(), "Zm9vYmE="),
            (b"foobar".as_slice(), "Zm9vYmFy"),
        ] {
            assert_eq!(encode(plain), expected);
        }
    }
    #[test]
    fn encodes_binary_remainder_lengths_with_canonical_padding() {
        for (plain, expected) in [
            (&[][..], ""),
            (&[0x00][..], "AA=="),
            (&[0x00, 0x00][..], "AAA="),
            (&[0x00, 0x00, 0x00][..], "AAAA"),
            (&[0xff][..], "/w=="),
            (&[0xff, 0xee][..], "/+4="),
            (&[0xff, 0xee, 0xdd][..], "/+7d"),
            (&[0xff, 0xee, 0xdd, 0xcc][..], "/+7dzA=="),
        ] {
            assert_eq!(encode(plain), expected);
        }
    }
}
