//! CRC-32C (Castagnoli) used to bind a reassembled payload to its beacon.

/// Reflected CRC-32C polynomial.
const POLY: u32 = 0x82F6_3B78;

const fn make_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        let mut crc = i as u32;
        let mut bit = 0;
        while bit < 8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ POLY
            } else {
                crc >> 1
            };
            bit += 1;
        }
        table[i] = crc;
        i += 1;
    }
    table
}

static TABLE: [u32; 256] = make_table();

/// Computes CRC-32C (init `0xFFFFFFFF`, reflected, final xor `0xFFFFFFFF`).
#[must_use]
pub fn crc32c(bytes: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in bytes {
        crc = TABLE[usize::from((crc as u8) ^ byte)] ^ (crc >> 8);
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_the_published_check_value() {
        assert_eq!(crc32c(b"123456789"), 0xE306_9283);
    }

    #[test]
    fn empty_input_is_zero() {
        assert_eq!(crc32c(&[]), 0);
    }
}
