use crate::bitstream::{BitWriter, BitstreamError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FseError {
    InvalidTable,
    InvalidSymbol,
    Bitstream(BitstreamError),
}

impl From<BitstreamError> for FseError {
    fn from(err: BitstreamError) -> Self {
        FseError::Bitstream(err)
    }
}

const MAX_SYMBOLS: usize = 256;
const MAX_TABLE_LOG: u8 = 12;

#[derive(Clone)]
pub(crate) struct FseCTable {
    pub(crate) table_log: u8,
    pub(crate) table_size: u32,
    pub(crate) state_table: Vec<u16>,
    pub(crate) symbol_tt: Vec<SymbolTransform>,
}

#[derive(Clone, Copy)]
pub(crate) struct SymbolTransform {
    pub(crate) delta_find_state: i32,
    pub(crate) delta_nb_bits: u32,
}

#[derive(Clone)]
pub(crate) struct FseDTable {
    pub(crate) table_log: u8,
    pub(crate) decode: Vec<DecodeEntry>,
}

#[derive(Clone, Copy)]
pub(crate) struct DecodeEntry {
    pub(crate) symbol: u16,
    pub(crate) nb_bits: u8,
    pub(crate) new_state: u32,
}

pub(crate) fn normalize_counts(counts: &[u32], table_log: u8) -> Result<Vec<i16>, FseError> {
    if table_log > MAX_TABLE_LOG {
        return Err(FseError::InvalidTable);
    }
    let table_size = 1u32 << table_log;
    let total: u64 = counts.iter().map(|&v| v as u64).sum();
    if total == 0 {
        return Err(FseError::InvalidTable);
    }
    let mut norm = vec![0i16; counts.len()];
    let mut residuals = vec![0u64; counts.len()];
    let mut sum_norm: u64 = 0;
    for (i, &count) in counts.iter().enumerate() {
        if count == 0 {
            norm[i] = 0;
            residuals[i] = 0;
            continue;
        }
        let scaled = (count as u64 * table_size as u64) / total;
        let n = scaled.max(1);
        norm[i] = n as i16;
        sum_norm += n;
        residuals[i] = (count as u64 * table_size as u64) % total;
    }
    while sum_norm < table_size as u64 {
        let mut best = None;
        for (i, &res) in residuals.iter().enumerate() {
            if norm[i] <= 0 {
                continue;
            }
            match best {
                None => best = Some((i, res)),
                Some((best_i, best_res)) => {
                    if res > best_res || (res == best_res && i < best_i) {
                        best = Some((i, res));
                    }
                }
            }
        }
        let (idx, _) = best.ok_or(FseError::InvalidTable)?;
        norm[idx] += 1;
        sum_norm += 1;
    }
    while sum_norm > table_size as u64 {
        let mut best = None;
        for (i, &res) in residuals.iter().enumerate() {
            if norm[i] <= 1 {
                continue;
            }
            match best {
                None => best = Some((i, res)),
                Some((best_i, best_res)) => {
                    if res < best_res || (res == best_res && i < best_i) {
                        best = Some((i, res));
                    }
                }
            }
        }
        let (idx, _) = best.ok_or(FseError::InvalidTable)?;
        norm[idx] -= 1;
        sum_norm -= 1;
    }
    Ok(norm)
}

pub(crate) fn build_tables(
    normalized: &[i16],
    max_symbol: usize,
    table_log: u8,
) -> Result<(FseCTable, FseDTable), FseError> {
    if max_symbol >= MAX_SYMBOLS || table_log > MAX_TABLE_LOG {
        return Err(FseError::InvalidTable);
    }
    let table_size = 1u32 << table_log;
    let table_mask = table_size - 1;
    let step = (table_size >> 1) + (table_size >> 3) + 3;
    let mut table_symbol = vec![0u16; table_size as usize];
    let mut high_threshold = table_size - 1;
    let mut cumul = vec![0u32; max_symbol + 2];
    cumul[0] = 0;
    for symbol in 0..=max_symbol {
        let norm = normalized[symbol];
        if norm == -1 {
            cumul[symbol + 1] = cumul[symbol] + 1;
            table_symbol[high_threshold as usize] = symbol as u16;
            if high_threshold == 0 {
                return Err(FseError::InvalidTable);
            }
            high_threshold -= 1;
        } else {
            let count = norm.max(0) as u32;
            cumul[symbol + 1] = cumul[symbol] + count;
        }
    }
    if cumul[max_symbol + 1] != table_size {
        return Err(FseError::InvalidTable);
    }

    let mut position = 0u32;
    for (symbol, &count) in normalized.iter().enumerate().take(max_symbol + 1) {
        if count <= 0 {
            continue;
        }
        for _ in 0..count {
            table_symbol[position as usize] = symbol as u16;
            position = (position + step) & table_mask;
            while position > high_threshold {
                position = (position + step) & table_mask;
            }
        }
    }
    if position != 0 {
        return Err(FseError::InvalidTable);
    }

    let mut state_table = vec![0u16; table_size as usize];
    let mut cumul_pos = cumul.clone();
    for (u, &symbol) in table_symbol.iter().enumerate().take(table_size as usize) {
        let symbol = symbol as usize;
        let pos = cumul_pos[symbol] as usize;
        state_table[pos] = (table_size as u16).wrapping_add(u as u16);
        cumul_pos[symbol] += 1;
    }

    let mut symbol_tt = vec![
        SymbolTransform {
            delta_find_state: 0,
            delta_nb_bits: 0,
        };
        max_symbol + 1
    ];
    let mut total = 0u32;
    for s in 0..=max_symbol {
        let norm = normalized[s];
        if norm == 0 {
            symbol_tt[s].delta_nb_bits = ((table_log as u32 + 1) << 16) - table_size;
            symbol_tt[s].delta_find_state = 0;
            continue;
        }
        if norm == -1 || norm == 1 {
            symbol_tt[s].delta_nb_bits = ((table_log as u32) << 16) - table_size;
            symbol_tt[s].delta_find_state = total as i32 - 1;
            total += 1;
            continue;
        }
        let count = norm as u32;
        let max_bits_out = table_log as u32 - highbit(count - 1);
        let min_state_plus = count << max_bits_out;
        symbol_tt[s].delta_nb_bits = (max_bits_out << 16) - min_state_plus;
        symbol_tt[s].delta_find_state = total as i32 - count as i32;
        total += count;
    }

    let mut symbol_next: Vec<u32> = vec![0; max_symbol + 1];
    for s in 0..=max_symbol {
        symbol_next[s] = if normalized[s] == -1 {
            1
        } else {
            normalized[s].max(0) as u32
        };
    }
    let mut decode = vec![
        DecodeEntry {
            symbol: 0,
            nb_bits: 0,
            new_state: 0,
        };
        table_size as usize
    ];
    for u in 0..table_size as usize {
        let symbol = table_symbol[u] as usize;
        let next_state = symbol_next[symbol];
        symbol_next[symbol] += 1;
        let nb_bits = table_log as u32 - highbit(next_state);
        let new_state = (next_state << nb_bits) - table_size;
        decode[u] = DecodeEntry {
            symbol: symbol as u16,
            nb_bits: nb_bits as u8,
            new_state,
        };
    }

    Ok((
        FseCTable {
            table_log,
            table_size,
            state_table,
            symbol_tt,
        },
        FseDTable { table_log, decode },
    ))
}

pub(crate) fn encode_symbols(symbols: &[u16], ct: &FseCTable) -> Result<Vec<u8>, FseError> {
    if symbols.is_empty() {
        return Ok(0u32.to_le_bytes().to_vec());
    }
    let mut writer = BitWriter::with_capacity(symbols.len().saturating_mul(4).saturating_add(8));
    let mut total_bits: u32 = 0;
    let mut state = ct.table_size;
    for &symbol in symbols.iter().rev() {
        let sym = symbol as usize;
        if sym >= ct.symbol_tt.len() {
            return Err(FseError::InvalidSymbol);
        }
        let tt = ct.symbol_tt[sym];
        let nb_bits_out = ((state as u64 + tt.delta_nb_bits as u64) >> 16) as u32;
        total_bits = total_bits
            .checked_add(nb_bits_out)
            .ok_or(FseError::InvalidTable)?;
        let mask = if nb_bits_out == 0 {
            0
        } else {
            (1u64 << nb_bits_out) - 1
        };
        writer.write_bits((state as u64) & mask, nb_bits_out)?;
        let next = (state >> nb_bits_out) as i32 + tt.delta_find_state;
        if next < 0 || next as usize >= ct.state_table.len() {
            return Err(FseError::InvalidTable);
        }
        state = ct.state_table[next as usize] as u32;
    }
    let flush_bits = ct.table_log as u32;
    total_bits = total_bits
        .checked_add(flush_bits)
        .ok_or(FseError::InvalidTable)?;
    let flush_mask = if flush_bits == 0 {
        0
    } else {
        (1u64 << flush_bits) - 1
    };
    writer.write_bits((state as u64) & flush_mask, flush_bits)?;
    let payload = writer.finish()?;
    let mut out = Vec::with_capacity(payload.len().saturating_add(4));
    out.extend_from_slice(&total_bits.to_le_bytes());
    out.extend_from_slice(&payload);
    Ok(out)
}

pub(crate) fn decode_symbols(
    encoded: &[u8],
    out_len: usize,
    dt: &FseDTable,
) -> Result<Vec<u16>, FseError> {
    if out_len == 0 {
        return Ok(Vec::new());
    }
    if encoded.len() < 4 {
        return Err(FseError::InvalidTable);
    }
    let bit_len = u32::from_le_bytes([encoded[0], encoded[1], encoded[2], encoded[3]]);
    let payload = &encoded[4..];
    if bit_len > payload.len().saturating_mul(8) as u32 {
        return Err(FseError::InvalidTable);
    }
    let mut reader = BitReaderRev::new(payload, bit_len);
    let table_size = 1u32 << dt.table_log;
    let mut state = reader.read_bits(dt.table_log as u32)? as u32 + table_size;
    let mut output = Vec::with_capacity(out_len);
    for _ in 0..out_len {
        let idx = state
            .checked_sub(table_size)
            .ok_or(FseError::InvalidTable)? as usize;
        let entry = dt.decode.get(idx).ok_or(FseError::InvalidTable)?;
        output.push(entry.symbol);
        let bits = reader.read_bits(entry.nb_bits as u32)? as u32;
        state = entry.new_state + bits + table_size;
    }
    Ok(output)
}

struct BitReaderRev<'a> {
    data: &'a [u8],
    bit_pos: u32,
}

impl<'a> BitReaderRev<'a> {
    fn new(data: &'a [u8], bit_len: u32) -> Self {
        Self {
            data,
            bit_pos: bit_len,
        }
    }

    fn read_bits(&mut self, bits: u32) -> Result<u64, BitstreamError> {
        if bits > 56 {
            return Err(BitstreamError::InvalidBits);
        }
        if bits == 0 {
            return Ok(0);
        }
        if bits > self.bit_pos {
            return Err(BitstreamError::UnexpectedEof);
        }
        let mut value = 0u64;
        for i in 0..bits {
            self.bit_pos -= 1;
            let idx = self.bit_pos;
            let byte = self.data[(idx >> 3) as usize];
            let bit = (byte >> (idx & 7)) & 1;
            let shift = bits - 1 - i;
            value |= (bit as u64) << shift;
        }
        Ok(value)
    }
}

fn highbit(value: u32) -> u32 {
    31 - value.leading_zeros()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fse_roundtrip() {
        let symbols = vec![0u16, 1, 2, 3, 2, 1, 0, 3, 3, 2, 1];
        let mut counts = vec![0u32; 4];
        for &sym in &symbols {
            counts[sym as usize] += 1;
        }
        let norm = normalize_counts(&counts, 5).expect("normalize");
        let (ct, dt) = build_tables(&norm, 3, 5).expect("tables");
        let encoded = encode_symbols(&symbols, &ct).expect("encode");
        let decoded = decode_symbols(&encoded, symbols.len(), &dt).expect("decode");
        assert_eq!(decoded, symbols);
    }

    #[test]
    fn fse_empty_payload_is_header_only() {
        let counts = vec![1u32, 1, 1, 1];
        let norm = normalize_counts(&counts, 5).expect("normalize");
        let (ct, dt) = build_tables(&norm, 3, 5).expect("tables");
        let encoded = encode_symbols(&[], &ct).expect("encode");
        assert_eq!(encoded, vec![0u8, 0, 0, 0]);
        let decoded = decode_symbols(&encoded, 0, &dt).expect("decode");
        assert!(decoded.is_empty());
    }
}
