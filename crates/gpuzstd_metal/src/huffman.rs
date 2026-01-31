use crate::bitstream::{BitReader, BitstreamError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HuffmanError {
    EmptyInput,
    TooDeep,
    Bitstream(BitstreamError),
    InvalidTable,
}

impl From<BitstreamError> for HuffmanError {
    fn from(err: BitstreamError) -> Self {
        HuffmanError::Bitstream(err)
    }
}

const MAX_SYMBOLS: usize = 256;
const MAX_NODES: usize = 512;
const MAX_BITS: u8 = 56;

#[derive(Debug, Clone)]
pub(crate) struct HuffmanTable {
    pub(crate) lengths: [u8; MAX_SYMBOLS],
    pub(crate) codes: [u64; MAX_SYMBOLS],
    pub(crate) max_len: u8,
}

pub(crate) fn encode_literals(input: &[u8]) -> Result<(Vec<u8>, HuffmanTable), HuffmanError> {
    if input.is_empty() {
        return Ok((
            Vec::new(),
            HuffmanTable {
                lengths: [0u8; MAX_SYMBOLS],
                codes: [0u64; MAX_SYMBOLS],
                max_len: 0,
            },
        ));
    }
    let table = build_table_for_input(input)?;
    let encoded = encode_with_table(input, &table)?;
    Ok((encoded, table))
}

pub(crate) fn decode_literals(
    encoded: &[u8],
    table: &HuffmanTable,
    output_len: usize,
) -> Result<Vec<u8>, HuffmanError> {
    if output_len == 0 {
        return Ok(Vec::new());
    }
    let mut reader = BitReader::new(encoded);
    let mut tree_child0 = vec![-1i32; MAX_NODES];
    let mut tree_child1 = vec![-1i32; MAX_NODES];
    let mut tree_symbol = vec![-1i16; MAX_NODES];
    let mut node_count = 1usize;

    let mut pairs = collect_pairs(&table.lengths)?;
    assign_tree(
        &mut pairs,
        &mut tree_symbol,
        &mut tree_child0,
        &mut tree_child1,
        &mut node_count,
    )?;

    let mut output = Vec::with_capacity(output_len);
    for _ in 0..output_len {
        let mut node = 0usize;
        loop {
            let symbol = tree_symbol[node];
            if symbol >= 0 {
                output.push(symbol as u8);
                break;
            }
            let bit = reader.read_bits(1)?;
            let next = if bit == 0 {
                tree_child0[node]
            } else {
                tree_child1[node]
            };
            if next < 0 {
                return Err(HuffmanError::InvalidTable);
            }
            node = next as usize;
        }
    }
    output.reverse();
    Ok(output)
}

pub(crate) fn build_table_for_input(input: &[u8]) -> Result<HuffmanTable, HuffmanError> {
    let counts = histogram(input);
    build_table(&counts)
}

pub(crate) fn encode_with_table(
    input: &[u8],
    table: &HuffmanTable,
) -> Result<Vec<u8>, HuffmanError> {
    if input.is_empty() {
        return Ok(Vec::new());
    }
    let mut capacity_bits = input.len() as u64 * table.max_len as u64 + 1;
    if capacity_bits == 0 {
        capacity_bits = 8;
    }
    let capacity_bytes = ((capacity_bits + 7) / 8) as usize;
    let mut writer = ZstdBitWriter::with_capacity(capacity_bytes);
    for &byte in input.iter().rev() {
        let code = table.codes[byte as usize];
        let len = table.lengths[byte as usize] as u32;
        writer.add_bits(code, len)?;
    }
    writer.close()
}

fn histogram(input: &[u8]) -> [u32; MAX_SYMBOLS] {
    let mut counts = [0u32; MAX_SYMBOLS];
    for &byte in input {
        counts[byte as usize] = counts[byte as usize].saturating_add(1);
    }
    counts
}

fn build_table(counts: &[u32; MAX_SYMBOLS]) -> Result<HuffmanTable, HuffmanError> {
    let mut lengths = [0u8; MAX_SYMBOLS];
    let mut codes = [0u64; MAX_SYMBOLS];
    let mut max_len = 0u8;
    let mut weights = [0u32; MAX_NODES];
    let mut parent = [-1i32; MAX_NODES];
    let mut node_id = [0u16; MAX_NODES];
    let mut left = [-1i32; MAX_NODES];
    let mut right = [-1i32; MAX_NODES];
    let mut leaves = [0i32; MAX_SYMBOLS];
    let mut leaf_count = 0usize;

    for (sym, &count) in counts.iter().enumerate() {
        if count == 0 {
            leaves[sym] = -1;
            continue;
        }
        let idx = leaf_count;
        weights[idx] = count;
        node_id[idx] = sym as u16;
        leaves[sym] = idx as i32;
        leaf_count += 1;
    }

    if leaf_count == 0 {
        return Ok(HuffmanTable {
            lengths,
            codes,
            max_len: 0,
        });
    }

    if leaf_count == 1 {
        let sym = leaves.iter().position(|&v| v >= 0).unwrap();
        lengths[sym] = 1;
        codes[sym] = 0;
        return Ok(HuffmanTable {
            lengths,
            codes,
            max_len: 1,
        });
    }

    let mut node_count = leaf_count;
    let mut remaining = leaf_count;
    while remaining > 1 {
        let (first, second) = select_two_smallest(&parent, &weights, &node_id, node_count)?;
        let (left_idx, right_idx) = order_nodes(first, second, &weights, &node_id);
        let new_idx = node_count;
        if new_idx >= MAX_NODES {
            return Err(HuffmanError::InvalidTable);
        }
        node_count += 1;
        weights[new_idx] = weights[left_idx].saturating_add(weights[right_idx]);
        node_id[new_idx] = node_id[left_idx].min(node_id[right_idx]);
        parent[left_idx] = new_idx as i32;
        parent[right_idx] = new_idx as i32;
        left[new_idx] = left_idx as i32;
        right[new_idx] = right_idx as i32;
        remaining -= 1;
    }

    for sym in 0..MAX_SYMBOLS {
        let leaf = leaves[sym];
        if leaf < 0 {
            continue;
        }
        let mut depth = 0u8;
        let mut node = leaf;
        while node >= 0 {
            let p = parent[node as usize];
            if p < 0 {
                break;
            }
            depth = depth.saturating_add(1);
            node = p;
        }
        if depth == 0 {
            depth = 1;
        }
        if depth > MAX_BITS {
            return Err(HuffmanError::TooDeep);
        }
        lengths[sym] = depth;
        if depth > max_len {
            max_len = depth;
        }
    }

    let mut pairs = collect_pairs(&lengths)?;
    assign_codes(&mut pairs, &mut codes, &lengths)?;
    Ok(HuffmanTable {
        lengths,
        codes,
        max_len,
    })
}

fn collect_pairs(lengths: &[u8; MAX_SYMBOLS]) -> Result<Vec<(u8, u16)>, HuffmanError> {
    let mut pairs = Vec::new();
    for (sym, &len) in lengths.iter().enumerate() {
        if len > 0 {
            pairs.push((len, sym as u16));
        }
    }
    if pairs.is_empty() {
        return Err(HuffmanError::EmptyInput);
    }
    pairs.sort_by(|(len_a, sym_a), (len_b, sym_b)| len_a.cmp(len_b).then(sym_a.cmp(sym_b)));
    Ok(pairs)
}

fn assign_codes(
    pairs: &mut [(u8, u16)],
    codes: &mut [u64; MAX_SYMBOLS],
    lengths: &[u8; MAX_SYMBOLS],
) -> Result<(), HuffmanError> {
    if pairs.is_empty() {
        return Err(HuffmanError::EmptyInput);
    }
    let mut code: u64 = 0;
    let mut cur_len = pairs[0].0;
    for &(len, sym) in pairs.iter() {
        if len > cur_len {
            code <<= (len - cur_len) as u64;
            cur_len = len;
        }
        let reversed = reverse_bits(code, len);
        codes[sym as usize] = reversed;
        code = code.saturating_add(1);
    }
    for (sym, &len) in lengths.iter().enumerate() {
        if len == 0 {
            codes[sym] = 0;
        }
    }
    Ok(())
}

fn assign_tree(
    pairs: &mut [(u8, u16)],
    tree_symbol: &mut [i16],
    tree_child0: &mut [i32],
    tree_child1: &mut [i32],
    node_count: &mut usize,
) -> Result<(), HuffmanError> {
    if pairs.is_empty() {
        return Err(HuffmanError::EmptyInput);
    }
    let mut code: u64 = 0;
    let mut cur_len = pairs[0].0;
    for &(len, sym) in pairs.iter() {
        if len > cur_len {
            code <<= (len - cur_len) as u64;
            cur_len = len;
        }
        let reversed = reverse_bits(code, len);
        let mut node = 0usize;
        for bit_index in 0..len {
            let bit = (reversed >> bit_index) & 1;
            let next = if bit == 0 {
                &mut tree_child0[node]
            } else {
                &mut tree_child1[node]
            };
            if *next < 0 {
                if *node_count >= MAX_NODES {
                    return Err(HuffmanError::InvalidTable);
                }
                *next = *node_count as i32;
                *node_count += 1;
            }
            node = *next as usize;
        }
        tree_symbol[node] = sym as i16;
        code = code.saturating_add(1);
    }
    Ok(())
}

struct ZstdBitWriter {
    buffer: u64,
    bit_count: u32,
    out: Vec<u8>,
    max_bytes: usize,
}

impl ZstdBitWriter {
    fn with_capacity(max_bytes: usize) -> Self {
        Self {
            buffer: 0,
            bit_count: 0,
            out: Vec::with_capacity(max_bytes.min(128)),
            max_bytes,
        }
    }

    fn add_bits(&mut self, value: u64, bits: u32) -> Result<(), HuffmanError> {
        if bits > MAX_BITS as u32 {
            return Err(HuffmanError::Bitstream(BitstreamError::InvalidBits));
        }
        if bits == 0 {
            return Ok(());
        }
        let masked = if bits == 64 {
            value
        } else {
            value & ((1u64 << bits) - 1)
        };
        self.buffer |= masked << self.bit_count;
        self.bit_count += bits;
        while self.bit_count >= 8 {
            self.push_byte()?;
        }
        Ok(())
    }

    fn close(mut self) -> Result<Vec<u8>, HuffmanError> {
        self.add_bits(1, 1)?;
        if self.bit_count > 0 {
            if self.out.len() >= self.max_bytes {
                return Err(HuffmanError::Bitstream(BitstreamError::NoSpace));
            }
            self.out.push((self.buffer & 0xFF) as u8);
            self.buffer = 0;
            self.bit_count = 0;
        }
        Ok(self.out)
    }

    fn push_byte(&mut self) -> Result<(), HuffmanError> {
        if self.out.len() >= self.max_bytes {
            return Err(HuffmanError::Bitstream(BitstreamError::NoSpace));
        }
        self.out.push((self.buffer & 0xFF) as u8);
        self.buffer >>= 8;
        self.bit_count -= 8;
        Ok(())
    }
}

fn reverse_bits(mut value: u64, bits: u8) -> u64 {
    let mut out = 0u64;
    for _ in 0..bits {
        out = (out << 1) | (value & 1);
        value >>= 1;
    }
    out
}

fn select_two_smallest(
    parent: &[i32; MAX_NODES],
    weights: &[u32; MAX_NODES],
    node_id: &[u16; MAX_NODES],
    node_count: usize,
) -> Result<(usize, usize), HuffmanError> {
    let mut first: Option<usize> = None;
    let mut second: Option<usize> = None;
    for idx in 0..node_count {
        if parent[idx] >= 0 {
            continue;
        }
        match first {
            None => first = Some(idx),
            Some(best) => {
                if cmp_node(idx, best, weights, node_id) {
                    second = first;
                    first = Some(idx);
                } else if second
                    .map(|s| cmp_node(idx, s, weights, node_id))
                    .unwrap_or(true)
                {
                    second = Some(idx);
                }
            }
        }
    }
    match (first, second) {
        (Some(a), Some(b)) => Ok((a, b)),
        _ => Err(HuffmanError::InvalidTable),
    }
}

fn cmp_node(a: usize, b: usize, weights: &[u32; MAX_NODES], node_id: &[u16; MAX_NODES]) -> bool {
    if weights[a] != weights[b] {
        return weights[a] < weights[b];
    }
    if node_id[a] != node_id[b] {
        return node_id[a] < node_id[b];
    }
    a < b
}

fn order_nodes(
    a: usize,
    b: usize,
    weights: &[u32; MAX_NODES],
    node_id: &[u16; MAX_NODES],
) -> (usize, usize) {
    if cmp_node(a, b, weights, node_id) {
        (a, b)
    } else {
        (b, a)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn huffman_roundtrip() {
        let payload = b"gpuzstd huffman roundtrip literals";
        let (encoded, table) = encode_literals(payload).expect("encode");
        let decoded = decode_literals(&encoded, &table, payload.len()).expect("decode");
        assert_eq!(decoded, payload);
    }

    #[test]
    fn huffman_single_symbol() {
        let payload = vec![0xaa; 32];
        let (encoded, table) = encode_literals(&payload).expect("encode");
        let decoded = decode_literals(&encoded, &table, payload.len()).expect("decode");
        assert_eq!(decoded, payload);
        assert_eq!(table.lengths[0xaa], 1);
    }
}
