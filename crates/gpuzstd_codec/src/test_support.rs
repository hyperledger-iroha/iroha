//! CPU references for the GPU backend entropy-coding parity tests.

use crate::{fse, huffman};

/// Encode literals and return the exact code lengths supplied to GPU parity tests.
pub fn huffman_encode(input: &[u8]) -> Result<(Vec<u8>, [u8; 256]), huffman::HuffmanError> {
    let (encoded, table) = huffman::encode_literals(input)?;
    Ok((encoded, table.lengths))
}

/// Normalize symbol counts with the shared deterministic FSE reference.
pub fn fse_normalize(counts: &[u32], table_log: u8) -> Result<Vec<i16>, fse::FseError> {
    fse::normalize_counts(counts, table_log)
}

/// Encode symbols using the normalized table supplied to the GPU parity test.
pub fn fse_encode(
    symbols: &[u16],
    normalized: &[i16],
    max_symbol: usize,
    table_log: u8,
) -> Result<Vec<u8>, fse::FseError> {
    if normalized.len() <= max_symbol {
        return Err(fse::FseError::InvalidTable);
    }
    let (table, _) = fse::build_tables(normalized, max_symbol, table_log)?;
    fse::encode_symbols(symbols, &table)
}

/// Independently decode the FSE reference payload used by GPU parity tests.
pub fn fse_decode(
    encoded: &[u8],
    output_len: usize,
    normalized: &[i16],
    max_symbol: usize,
    table_log: u8,
) -> Result<Vec<u16>, fse::FseError> {
    if normalized.len() <= max_symbol {
        return Err(fse::FseError::InvalidTable);
    }
    let (_, table) = fse::build_tables(normalized, max_symbol, table_log)?;
    fse::decode_symbols(encoded, output_len, &table)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn huffman_reference_preserves_the_shared_table() {
        let input = b"GPU literal parity reference";
        let (encoded, lengths) = huffman_encode(input).unwrap();
        let table = huffman::build_table_for_input(input).unwrap();
        assert_eq!(lengths, table.lengths);
        assert_eq!(
            huffman::decode_literals(&encoded, &table, input.len()).unwrap(),
            input
        );
    }

    #[test]
    fn fse_reference_roundtrips_and_rejects_invalid_tables() {
        let symbols = [0, 1, 2, 3, 2, 1, 0, 3, 3, 2, 1];
        let normalized = fse_normalize(&[2, 3, 3, 3], 5).unwrap();
        let encoded = fse_encode(&symbols, &normalized, 3, 5).unwrap();
        assert_eq!(
            fse_decode(&encoded, symbols.len(), &normalized, 3, 5).unwrap(),
            symbols
        );
        assert!(fse_normalize(&[0, 0], 5).is_err());
        assert!(fse_encode(&symbols, &[], 3, 5).is_err());
        assert!(fse_decode(&encoded, symbols.len(), &[], 3, 5).is_err());
    }
}
