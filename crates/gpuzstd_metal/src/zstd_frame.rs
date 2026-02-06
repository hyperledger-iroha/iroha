use std::sync::OnceLock;

use crate::{
    GpuZstdSequence,
    fse::{self, FseCTable, FseDTable},
    huffman::{self, HuffmanError, HuffmanTable},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ZstdEncodeError {
    InvalidInput,
    Capacity,
    Huffman(HuffmanError),
    Fse(fse::FseError),
}

impl From<HuffmanError> for ZstdEncodeError {
    fn from(err: HuffmanError) -> Self {
        ZstdEncodeError::Huffman(err)
    }
}

impl From<fse::FseError> for ZstdEncodeError {
    fn from(err: fse::FseError) -> Self {
        ZstdEncodeError::Fse(err)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ZstdDecodeError {
    InvalidInput,
    Unsupported,
    Huffman(HuffmanError),
    Fse(fse::FseError),
}

impl From<HuffmanError> for ZstdDecodeError {
    fn from(err: HuffmanError) -> Self {
        ZstdDecodeError::Huffman(err)
    }
}

impl From<fse::FseError> for ZstdDecodeError {
    fn from(err: fse::FseError) -> Self {
        ZstdDecodeError::Fse(err)
    }
}

const ZSTD_MAGICNUMBER: u32 = 0xFD2FB528;
const ZSTD_WINDOWLOG_ABSOLUTEMIN: u8 = 10;
const MAX_LL: usize = 35;
const MAX_ML: usize = 52;
const MAX_OFF: usize = 31;
const DEFAULT_MAX_OFF: usize = 28;
const LONGNBSEQ: usize = 0x7F00;
const MIN_MATCH: u32 = 3;

const LL_BITS: [u8; MAX_LL + 1] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 3, 3, 4, 6, 7, 8, 9, 10, 11,
    12, 13, 14, 15, 16,
];
const ML_BITS: [u8; MAX_ML + 1] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    1, 1, 1, 1, 2, 2, 3, 3, 4, 4, 5, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16,
];
const LL_DEFAULT_NORM: [i16; MAX_LL + 1] = [
    4, 3, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 3, 2, 1, 1, 1, 1, 1,
    -1, -1, -1, -1,
];
const ML_DEFAULT_NORM: [i16; MAX_ML + 1] = [
    1, 4, 3, 2, 2, 2, 2, 2, 2, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, -1, -1, -1, -1, -1, -1, -1,
];
const OF_DEFAULT_NORM: [i16; DEFAULT_MAX_OFF + 1] = [
    1, 1, 1, 1, 1, 1, 2, 2, 2, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, -1, -1, -1, -1, -1,
];
const LL_DEFAULT_NORM_LOG: u8 = 6;
const ML_DEFAULT_NORM_LOG: u8 = 6;
const OF_DEFAULT_NORM_LOG: u8 = 5;

const LL_CODE: [u8; 64] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 16, 17, 17, 18, 18, 19, 19, 20, 20,
    20, 20, 21, 21, 21, 21, 22, 22, 22, 22, 22, 22, 22, 22, 23, 23, 23, 23, 23, 23, 23, 23, 24, 24,
    24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24,
];
const ML_CODE: [u8; 128] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
    26, 27, 28, 29, 30, 31, 32, 32, 33, 33, 34, 34, 35, 35, 36, 36, 36, 36, 37, 37, 37, 37, 38, 38,
    38, 38, 38, 38, 38, 38, 39, 39, 39, 39, 39, 39, 39, 39, 40, 40, 40, 40, 40, 40, 40, 40, 40, 40,
    40, 40, 40, 40, 40, 40, 41, 41, 41, 41, 41, 41, 41, 41, 41, 41, 41, 41, 41, 41, 41, 41, 42, 42,
    42, 42, 42, 42, 42, 42, 42, 42, 42, 42, 42, 42, 42, 42, 42, 42, 42, 42, 42, 42, 42, 42, 42, 42,
    42, 42, 42, 42, 42, 42,
];

#[derive(Debug, Clone, Copy)]
struct SeqDef {
    lit_len: u32,
    match_len: u32,
    offset: u32,
}

pub(crate) fn encode_frame(
    input: &[u8],
    chunk_size: usize,
    counts: &[u32],
    offsets: &[u32],
    sequences: &[GpuZstdSequence],
    checksum: bool,
) -> Result<Vec<u8>, ZstdEncodeError> {
    let mut out = Vec::with_capacity(input.len().saturating_add(64));
    let header = encode_frame_header(input.len(), chunk_size, checksum)?;
    out.extend_from_slice(&header);

    let chunk_count = counts.len();
    for chunk_idx in 0..chunk_count {
        let start = chunk_idx * chunk_size;
        let end = (start + chunk_size).min(input.len());
        let count = counts[chunk_idx] as usize;
        let offset = offsets[chunk_idx] as usize;
        if start >= end {
            continue;
        }
        let seq_slice = sequences
            .get(offset..offset.saturating_add(count))
            .ok_or(ZstdEncodeError::InvalidInput)?;
        let block = encode_block(&input[start..end], seq_slice)?;
        let last_block = chunk_idx + 1 == chunk_count;
        write_block(&mut out, &block, last_block)?;
    }

    if checksum {
        let hash = xxh32(input);
        out.extend_from_slice(&hash.to_le_bytes());
    }
    Ok(out)
}

pub(crate) fn decode_frame(input: &[u8]) -> Result<Vec<u8>, ZstdDecodeError> {
    let mut idx = 0usize;
    if input.len() < 5 {
        return Err(ZstdDecodeError::InvalidInput);
    }
    let magic = read_u32_le(input, &mut idx)?;
    if magic != ZSTD_MAGICNUMBER {
        return Err(ZstdDecodeError::InvalidInput);
    }
    let desc = read_u8(input, &mut idx)?;
    let dict_id = desc & 0x3;
    let checksum_flag = ((desc >> 2) & 0x1) != 0;
    let reserved = ((desc >> 3) & 0x1) != 0;
    let single_segment = ((desc >> 5) & 0x1) != 0;
    let fcs_code = desc >> 6;
    if reserved {
        return Err(ZstdDecodeError::InvalidInput);
    }
    if dict_id != 0 {
        return Err(ZstdDecodeError::Unsupported);
    }
    let mut window_size = 0u64;
    if !single_segment {
        let window_desc = read_u8(input, &mut idx)?;
        let window_log = (window_desc >> 3).saturating_add(ZSTD_WINDOWLOG_ABSOLUTEMIN);
        let window_base = window_desc & 0x7;
        if window_log < ZSTD_WINDOWLOG_ABSOLUTEMIN {
            return Err(ZstdDecodeError::InvalidInput);
        }
        window_size = (1u64 << window_log) + ((window_base as u64) << window_log.saturating_sub(3));
    }
    let content_size = match fcs_code {
        0 => {
            if single_segment {
                Some(read_u8(input, &mut idx)? as usize)
            } else {
                None
            }
        }
        1 => {
            let val = read_u16_le(input, &mut idx)? as usize;
            Some(val + 256)
        }
        2 => Some(read_u32_le(input, &mut idx)? as usize),
        3 => {
            let val = read_u64_le(input, &mut idx)?;
            if val > usize::MAX as u64 {
                return Err(ZstdDecodeError::InvalidInput);
            }
            Some(val as usize)
        }
        _ => return Err(ZstdDecodeError::InvalidInput),
    };
    if single_segment {
        window_size = content_size.ok_or(ZstdDecodeError::InvalidInput)? as u64;
    }

    let mut output = Vec::with_capacity(content_size.unwrap_or(0));
    loop {
        if idx + 3 > input.len() {
            return Err(ZstdDecodeError::InvalidInput);
        }
        let header = read_u24_le(input, &mut idx)?;
        let last = (header & 1) != 0;
        let block_type = (header >> 1) & 0x3;
        let block_size = (header >> 3) as usize;
        match block_type {
            0 => {
                if idx + block_size > input.len() {
                    return Err(ZstdDecodeError::InvalidInput);
                }
                let block_payload = &input[idx..idx + block_size];
                idx += block_size;
                output.extend_from_slice(block_payload);
            }
            1 => {
                if idx >= input.len() {
                    return Err(ZstdDecodeError::InvalidInput);
                }
                let byte = input[idx];
                idx += 1;
                output.extend(std::iter::repeat_n(byte, block_size));
            }
            2 => {
                if idx + block_size > input.len() {
                    return Err(ZstdDecodeError::InvalidInput);
                }
                let block_payload = &input[idx..idx + block_size];
                idx += block_size;
                decode_compressed_block(block_payload, &mut output, window_size)?
            }
            _ => return Err(ZstdDecodeError::Unsupported),
        }
        if last {
            break;
        }
    }
    if checksum_flag {
        if idx + 4 > input.len() {
            return Err(ZstdDecodeError::InvalidInput);
        }
        let expected = read_u32_le(input, &mut idx)?;
        let actual = xxh32(&output);
        if expected != actual {
            return Err(ZstdDecodeError::InvalidInput);
        }
    }
    if idx != input.len() {
        return Err(ZstdDecodeError::InvalidInput);
    }
    if let Some(expected) = content_size
        && output.len() != expected
    {
        return Err(ZstdDecodeError::InvalidInput);
    }
    Ok(output)
}

fn encode_frame_header(
    src_size: usize,
    chunk_size: usize,
    checksum: bool,
) -> Result<Vec<u8>, ZstdEncodeError> {
    let window_log = (chunk_size as u32).next_power_of_two().trailing_zeros() as u8;
    let window_size = 1u64 << window_log;
    let single_segment = (src_size as u64) <= window_size;
    let fcs_code = if src_size < 256 {
        0
    } else if src_size < 65536 + 256 {
        1
    } else if src_size < 0xFFFF_FFFF {
        2
    } else {
        3
    };
    let mut out = Vec::with_capacity(14);
    out.extend_from_slice(&ZSTD_MAGICNUMBER.to_le_bytes());
    let mut desc = 0u8;
    if checksum {
        desc |= 1 << 2;
    }
    if single_segment {
        desc |= 1 << 5;
    }
    desc |= (fcs_code as u8) << 6;
    out.push(desc);
    if !single_segment {
        let win_desc = (window_log - ZSTD_WINDOWLOG_ABSOLUTEMIN) << 3;
        out.push(win_desc);
    }
    match fcs_code {
        0 => {
            if single_segment {
                out.push(src_size as u8);
            }
        }
        1 => {
            let size = (src_size - 256) as u16;
            out.extend_from_slice(&size.to_le_bytes());
        }
        2 => {
            out.extend_from_slice(&(src_size as u32).to_le_bytes());
        }
        3 => {
            out.extend_from_slice(&(src_size as u64).to_le_bytes());
        }
        _ => return Err(ZstdEncodeError::InvalidInput),
    }
    Ok(out)
}

fn decode_compressed_block(
    data: &[u8],
    output: &mut Vec<u8>,
    window_size: u64,
) -> Result<(), ZstdDecodeError> {
    let (literals, lit_consumed) = decode_literal_section(data)?;
    if lit_consumed > data.len() {
        return Err(ZstdDecodeError::InvalidInput);
    }
    let seq_data = &data[lit_consumed..];
    let (sequences, seq_consumed) = decode_sequence_section(seq_data)?;
    if lit_consumed + seq_consumed != data.len() {
        return Err(ZstdDecodeError::InvalidInput);
    }
    let mut lit_pos = 0usize;
    for seq in sequences {
        let lit_len = seq.lit_len as usize;
        let match_len = seq.match_len as usize;
        let offset = seq.offset as usize;
        if lit_pos + lit_len > literals.len() {
            return Err(ZstdDecodeError::InvalidInput);
        }
        output.extend_from_slice(&literals[lit_pos..lit_pos + lit_len]);
        lit_pos += lit_len;
        if match_len == 0 {
            continue;
        }
        if offset == 0 || offset > output.len() {
            return Err(ZstdDecodeError::InvalidInput);
        }
        if window_size > 0 && offset as u64 > window_size {
            return Err(ZstdDecodeError::InvalidInput);
        }
        for _ in 0..match_len {
            let idx = output
                .len()
                .checked_sub(offset)
                .ok_or(ZstdDecodeError::InvalidInput)?;
            let byte = output[idx];
            output.push(byte);
        }
    }
    if lit_pos > literals.len() {
        return Err(ZstdDecodeError::InvalidInput);
    }
    output.extend_from_slice(&literals[lit_pos..]);
    Ok(())
}

fn decode_literal_section(data: &[u8]) -> Result<(Vec<u8>, usize), ZstdDecodeError> {
    if data.is_empty() {
        return Err(ZstdDecodeError::InvalidInput);
    }
    let first = data[0];
    let h_type = first & 0x3;
    match h_type {
        0 => decode_raw_literals(data),
        1 => decode_rle_literals(data),
        2 => decode_huffman_literals(data),
        _ => Err(ZstdDecodeError::Unsupported),
    }
}

fn decode_raw_literals(data: &[u8]) -> Result<(Vec<u8>, usize), ZstdDecodeError> {
    let (lit_size, header_size) = decode_raw_rle_header(data)?;
    if data.len() < header_size + lit_size {
        return Err(ZstdDecodeError::InvalidInput);
    }
    Ok((
        data[header_size..header_size + lit_size].to_vec(),
        header_size + lit_size,
    ))
}

fn decode_rle_literals(data: &[u8]) -> Result<(Vec<u8>, usize), ZstdDecodeError> {
    let (lit_size, header_size) = decode_raw_rle_header(data)?;
    if data.len() < header_size + 1 {
        return Err(ZstdDecodeError::InvalidInput);
    }
    let byte = data[header_size];
    Ok((vec![byte; lit_size], header_size + 1))
}

fn decode_raw_rle_header(data: &[u8]) -> Result<(usize, usize), ZstdDecodeError> {
    if data.is_empty() {
        return Err(ZstdDecodeError::InvalidInput);
    }
    let size_format = (data[0] >> 2) & 0x3;
    let header_size = match size_format {
        0 => 1,
        1 => 2,
        3 => 3,
        _ => return Err(ZstdDecodeError::Unsupported),
    };
    if data.len() < header_size {
        return Err(ZstdDecodeError::InvalidInput);
    }
    let val = read_le_value(data, 0, header_size)?;
    let lit_size = (val >> 4) as usize;
    Ok((lit_size, header_size))
}

fn decode_huffman_literals(data: &[u8]) -> Result<(Vec<u8>, usize), ZstdDecodeError> {
    let size_format = (data[0] >> 2) & 0x3;
    let (header_size, single_stream, lit_size, c_lit_size) = match size_format {
        0 | 1 => {
            if data.len() < 3 {
                return Err(ZstdDecodeError::InvalidInput);
            }
            let val = read_le_value(data, 0, 3)?;
            let lit_size = ((val >> 4) & 0x3FF) as usize;
            let c_lit_size = ((val >> 14) & 0x3FF) as usize;
            let single_stream = size_format == 0;
            (3, single_stream, lit_size, c_lit_size)
        }
        2 => {
            if data.len() < 4 {
                return Err(ZstdDecodeError::InvalidInput);
            }
            let val = read_le_value(data, 0, 4)?;
            let lit_size = ((val >> 4) & 0x3FFF) as usize;
            let c_lit_size = ((val >> 18) & 0x3FFF) as usize;
            (4, false, lit_size, c_lit_size)
        }
        3 => {
            if data.len() < 5 {
                return Err(ZstdDecodeError::InvalidInput);
            }
            let val = read_le_value(data, 0, 4)?;
            let extra = data[4] as u64;
            let lit_size = ((val >> 4) & 0x3FFFF) as usize;
            let c_lit_low = (val >> 22) & 0x3FF;
            let c_lit_size = (c_lit_low | (extra << 10)) as usize;
            (5, false, lit_size, c_lit_size)
        }
        _ => return Err(ZstdDecodeError::Unsupported),
    };
    let total = header_size + c_lit_size;
    if data.len() < total {
        return Err(ZstdDecodeError::InvalidInput);
    }
    let section = &data[header_size..total];
    let (table, header_len) = parse_huffman_header(section)?;
    let encoded = &section[header_len..];
    let literals = if single_stream {
        decode_huffman_stream(encoded, &table, lit_size)?
    } else {
        if encoded.len() < 6 {
            return Err(ZstdDecodeError::InvalidInput);
        }
        let s0 = u16::from_le_bytes([encoded[0], encoded[1]]) as usize;
        let s1 = u16::from_le_bytes([encoded[2], encoded[3]]) as usize;
        let s2 = u16::from_le_bytes([encoded[4], encoded[5]]) as usize;
        let mut offset = 6usize;
        if encoded.len() < offset + s0 + s1 + s2 {
            return Err(ZstdDecodeError::InvalidInput);
        }
        let s3 = encoded.len() - offset - s0 - s1 - s2;
        let segment_size = lit_size.div_ceil(4);
        let seg_lens = [
            segment_size,
            segment_size,
            segment_size,
            lit_size.saturating_sub(segment_size * 3),
        ];
        let mut out = Vec::with_capacity(lit_size);
        let streams = [s0, s1, s2, s3];
        for (seg_idx, seg_len) in seg_lens.iter().enumerate() {
            let seg_size = streams[seg_idx];
            let seg = &encoded[offset..offset + seg_size];
            let decoded = decode_huffman_stream(seg, &table, *seg_len)?;
            out.extend_from_slice(&decoded);
            offset += seg_size;
        }
        out
    };
    Ok((literals, total))
}

fn parse_huffman_header(data: &[u8]) -> Result<(HuffmanTable, usize), ZstdDecodeError> {
    if data.is_empty() {
        return Err(ZstdDecodeError::InvalidInput);
    }
    let header = data[0];
    if header < 128 {
        return Err(ZstdDecodeError::Unsupported);
    }
    let max_symbol = (header as usize).saturating_sub(127);
    if max_symbol == 0 || max_symbol >= 256 {
        return Err(ZstdDecodeError::InvalidInput);
    }
    let weight_bytes = (max_symbol + 2) / 2;
    if data.len() < 1 + weight_bytes {
        return Err(ZstdDecodeError::InvalidInput);
    }
    let mut lengths = [0u8; 256];
    let mut max_weight = 0u8;
    let mut sym = 0usize;
    for byte in &data[1..1 + weight_bytes] {
        let hi = byte >> 4;
        let lo = byte & 0xF;
        if sym <= max_symbol {
            max_weight = max_weight.max(hi);
            lengths[sym] = hi;
            sym += 1;
        }
        if sym <= max_symbol {
            max_weight = max_weight.max(lo);
            lengths[sym] = lo;
            sym += 1;
        }
    }
    if max_weight == 0 {
        return Err(ZstdDecodeError::InvalidInput);
    }
    let mut decoded_lengths = [0u8; 256];
    for (idx, &weight) in lengths.iter().enumerate() {
        if weight == 0 {
            decoded_lengths[idx] = 0;
        } else {
            decoded_lengths[idx] = max_weight
                .checked_add(1)
                .and_then(|v| v.checked_sub(weight))
                .ok_or(ZstdDecodeError::InvalidInput)?;
        }
    }
    Ok((
        HuffmanTable {
            lengths: decoded_lengths,
            codes: [0u64; 256],
            max_len: max_weight,
        },
        1 + weight_bytes,
    ))
}

fn decode_huffman_stream(
    encoded: &[u8],
    table: &HuffmanTable,
    out_len: usize,
) -> Result<Vec<u8>, ZstdDecodeError> {
    if out_len == 0 {
        return Ok(Vec::new());
    }
    let decoded = huffman::decode_literals(encoded, table, out_len)?;
    Ok(decoded)
}

fn decode_sequence_section(data: &[u8]) -> Result<(Vec<SeqDef>, usize), ZstdDecodeError> {
    if data.is_empty() {
        return Err(ZstdDecodeError::InvalidInput);
    }
    let (nb_seq, mut consumed) = decode_nb_seq(data)?;
    if nb_seq == 0 {
        return Ok((Vec::new(), consumed));
    }
    if consumed >= data.len() {
        return Err(ZstdDecodeError::InvalidInput);
    }
    let seq_desc = data[consumed];
    consumed += 1;
    if seq_desc != 0 {
        return Err(ZstdDecodeError::Unsupported);
    }
    let bitstream = &data[consumed..];
    let sequences = decode_sequences_bitstream(nb_seq, bitstream)?;
    Ok((sequences, data.len()))
}

fn decode_nb_seq(data: &[u8]) -> Result<(usize, usize), ZstdDecodeError> {
    let first = data[0];
    if first < 128 {
        return Ok((first as usize, 1));
    }
    if first < 0xFF {
        if data.len() < 2 {
            return Err(ZstdDecodeError::InvalidInput);
        }
        let high = (first as usize - 0x80) << 8;
        let low = data[1] as usize;
        return Ok((high | low, 2));
    }
    if data.len() < 3 {
        return Err(ZstdDecodeError::InvalidInput);
    }
    let adj = u16::from_le_bytes([data[1], data[2]]) as usize;
    Ok((LONGNBSEQ + adj, 3))
}

fn decode_sequences_bitstream(
    nb_seq: usize,
    bitstream: &[u8],
) -> Result<Vec<SeqDef>, ZstdDecodeError> {
    if nb_seq == 0 {
        return Ok(Vec::new());
    }
    let bit_len = bitstream_end_mark_len(bitstream)?;
    let mut reader = BitReaderRev::new(bitstream, bit_len);
    let ll_table = default_ll_dtable()?;
    let ml_table = default_ml_dtable()?;
    let of_table = default_of_dtable()?;
    let ll_table_size = 1u32 << ll_table.table_log;
    let ml_table_size = 1u32 << ml_table.table_log;
    let of_table_size = 1u32 << of_table.table_log;
    let mut state_ll = reader.read_bits(ll_table.table_log as u32)? as u32 + ll_table_size;
    let mut state_of = reader.read_bits(of_table.table_log as u32)? as u32 + of_table_size;
    let mut state_ml = reader.read_bits(ml_table.table_log as u32)? as u32 + ml_table_size;
    let ll_base = ll_base_table();
    let ml_base = ml_base_table();
    let mut out = Vec::with_capacity(nb_seq);
    for _ in 0..nb_seq {
        let ll_symbol = fse_decode_symbol(&ll_table, &mut reader, &mut state_ll)?;
        let ml_symbol = fse_decode_symbol(&ml_table, &mut reader, &mut state_ml)?;
        let of_symbol = fse_decode_symbol(&of_table, &mut reader, &mut state_of)?;
        if ll_symbol as usize > MAX_LL || ml_symbol as usize > MAX_ML {
            return Err(ZstdDecodeError::InvalidInput);
        }
        if of_symbol as usize > DEFAULT_MAX_OFF {
            return Err(ZstdDecodeError::InvalidInput);
        }
        let ll_bits = LL_BITS[ll_symbol as usize] as u32;
        let ml_bits = ML_BITS[ml_symbol as usize] as u32;
        let of_bits = of_symbol as u32;
        let ll_extra = reader.read_bits(ll_bits)? as u32;
        let ml_extra = reader.read_bits(ml_bits)? as u32;
        let of_extra = reader.read_bits(of_bits)? as u32;
        let lit_len = ll_base[ll_symbol as usize].saturating_add(ll_extra);
        let ml_base_val = ml_base[ml_symbol as usize];
        let match_len = ml_base_val
            .saturating_add(ml_extra)
            .saturating_add(MIN_MATCH);
        let offset = (1u32 << of_bits).saturating_add(of_extra);
        out.push(SeqDef {
            lit_len,
            match_len,
            offset,
        });
    }
    Ok(out)
}

#[derive(Debug)]
enum Block {
    Raw(Vec<u8>),
    Rle { byte: u8, len: usize },
    Compressed(Vec<u8>),
}

fn encode_block(input: &[u8], sequences: &[GpuZstdSequence]) -> Result<Block, ZstdEncodeError> {
    if input.is_empty() {
        return Ok(Block::Raw(Vec::new()));
    }

    if input.iter().all(|b| *b == input[0]) {
        return Ok(Block::Rle {
            byte: input[0],
            len: input.len(),
        });
    }

    let (literals, seqs) = collect_literals_and_seqs(input, sequences)?;
    let literal_section = build_literal_section(&literals)?;
    let sequence_section = build_sequence_section(&seqs)?;
    let mut compressed = Vec::with_capacity(literal_section.len() + sequence_section.len());
    compressed.extend_from_slice(&literal_section);
    compressed.extend_from_slice(&sequence_section);

    if compressed.len() >= input.len() {
        return Ok(Block::Raw(input.to_vec()));
    }
    Ok(Block::Compressed(compressed))
}

fn collect_literals_and_seqs(
    input: &[u8],
    sequences: &[GpuZstdSequence],
) -> Result<(Vec<u8>, Vec<SeqDef>), ZstdEncodeError> {
    let mut literals = Vec::new();
    let mut seqs = Vec::new();
    let mut pos = 0usize;
    for seq in sequences {
        let lit_len = seq.lit_len as usize;
        if pos + lit_len > input.len() {
            return Err(ZstdEncodeError::InvalidInput);
        }
        literals.extend_from_slice(&input[pos..pos + lit_len]);
        pos += lit_len;
        if seq.match_len == 0 {
            if pos != input.len() {
                return Err(ZstdEncodeError::InvalidInput);
            }
            break;
        }
        let match_len = seq.match_len as usize;
        if pos + match_len > input.len() {
            return Err(ZstdEncodeError::InvalidInput);
        }
        seqs.push(SeqDef {
            lit_len: seq.lit_len,
            match_len: seq.match_len,
            offset: seq.offset,
        });
        pos += match_len;
    }
    if pos != input.len() {
        return Err(ZstdEncodeError::InvalidInput);
    }
    Ok((literals, seqs))
}

fn build_literal_section(literals: &[u8]) -> Result<Vec<u8>, ZstdEncodeError> {
    if literals.is_empty() {
        return write_raw_literals(literals);
    }
    if literals.iter().all(|b| *b == literals[0]) {
        return write_rle_literals(literals[0], literals.len());
    }

    let lh_size = 3 + (literals.len() >= 1024) as usize + (literals.len() >= 16 * 1024) as usize;
    let huf = match huffman::build_table_for_input(literals) {
        Ok(table) => table,
        Err(_) => return write_raw_literals(literals),
    };
    if huf.max_len == 0 || huf.max_len > 11 {
        return write_raw_literals(literals);
    }

    let header = match huffman_weights_header(&huf) {
        Ok(header) => header,
        Err(_) => return write_raw_literals(literals),
    };
    let huf_data = if literals.len() < 1024 {
        huffman::encode_with_table(literals, &huf)?
    } else {
        encode_huffman_4x(literals, &huf)?
    };
    let c_lit_size = header.len() + huf_data.len();
    if lh_size == 4 && c_lit_size >= (1 << 14) {
        return write_raw_literals(literals);
    }
    if lh_size == 5 && c_lit_size >= (1 << 18) {
        return write_raw_literals(literals);
    }

    let mut out = Vec::with_capacity(lh_size + c_lit_size);
    write_literal_header(
        &mut out,
        2,
        literals.len(),
        c_lit_size,
        literals.len() < 1024,
    )?;
    out.extend_from_slice(&header);
    out.extend_from_slice(&huf_data);
    Ok(out)
}

fn write_literal_header(
    out: &mut Vec<u8>,
    h_type: u8,
    lit_size: usize,
    c_lit_size: usize,
    single_stream: bool,
) -> Result<(), ZstdEncodeError> {
    let lh_size = 3 + (lit_size >= 1024) as usize + (lit_size >= 16 * 1024) as usize;
    match lh_size {
        3 => {
            let lhc = (h_type as u32)
                + ((u32::from(!single_stream)) << 2)
                + ((lit_size as u32) << 4)
                + ((c_lit_size as u32) << 14);
            out.extend_from_slice(&lhc.to_le_bytes()[..3]);
        }
        4 => {
            let lhc =
                (h_type as u32) + (2 << 2) + ((lit_size as u32) << 4) + ((c_lit_size as u32) << 18);
            out.extend_from_slice(&lhc.to_le_bytes());
        }
        5 => {
            let lhc =
                (h_type as u32) + (3 << 2) + ((lit_size as u32) << 4) + ((c_lit_size as u32) << 22);
            let bytes = lhc.to_le_bytes();
            out.extend_from_slice(&bytes);
            out.push((c_lit_size as u32 >> 10) as u8);
        }
        _ => return Err(ZstdEncodeError::InvalidInput),
    }
    Ok(())
}

fn write_raw_literals(literals: &[u8]) -> Result<Vec<u8>, ZstdEncodeError> {
    let size = literals.len();
    let header_size = 1 + (size > 31) as usize + (size > 4095) as usize;
    let mut out = Vec::with_capacity(header_size + size);
    match header_size {
        1 => out.push((size as u8) << 3),
        2 => {
            let val = (1 << 2) + ((size as u32) << 4);
            out.extend_from_slice(&(val as u16).to_le_bytes());
        }
        3 => {
            let val = (3 << 2) + ((size as u32) << 4);
            out.extend_from_slice(&val.to_le_bytes()[..3]);
        }
        _ => return Err(ZstdEncodeError::InvalidInput),
    }
    out.extend_from_slice(literals);
    Ok(out)
}

fn write_rle_literals(byte: u8, len: usize) -> Result<Vec<u8>, ZstdEncodeError> {
    let header_size = 1 + (len > 31) as usize + (len > 4095) as usize;
    let mut out = Vec::with_capacity(header_size + 1);
    match header_size {
        1 => out.push(1 + ((len as u8) << 3)),
        2 => {
            let val = 1 + (1 << 2) + ((len as u32) << 4);
            out.extend_from_slice(&(val as u16).to_le_bytes());
        }
        3 => {
            let val = 1 + (3 << 2) + ((len as u32) << 4);
            out.extend_from_slice(&val.to_le_bytes()[..3]);
        }
        _ => return Err(ZstdEncodeError::InvalidInput),
    }
    out.push(byte);
    Ok(out)
}

fn huffman_weights_header(table: &HuffmanTable) -> Result<Vec<u8>, ZstdEncodeError> {
    let max_symbol = table
        .lengths
        .iter()
        .rposition(|&v| v > 0)
        .ok_or(ZstdEncodeError::InvalidInput)?;
    if max_symbol == 0 {
        return Err(ZstdEncodeError::InvalidInput);
    }
    let table_log = table.max_len as i16;
    let mut weights = Vec::with_capacity(max_symbol + 2);
    for sym in 0..=max_symbol {
        let len = table.lengths[sym] as i16;
        let weight = if len == 0 { 0 } else { table_log + 1 - len };
        if !(0..=15).contains(&weight) {
            return Err(ZstdEncodeError::InvalidInput);
        }
        weights.push(weight as u8);
    }
    let mut out = Vec::with_capacity((max_symbol + 2) / 2 + 1);
    let header = 128u16 + (max_symbol as u16 - 1);
    if header > u8::MAX as u16 {
        return Err(ZstdEncodeError::InvalidInput);
    }
    out.push(header as u8);
    let mut idx = 0;
    while idx <= max_symbol {
        let hi = weights[idx];
        let lo = if idx < max_symbol {
            weights[idx + 1]
        } else {
            0
        };
        out.push((hi << 4) | lo);
        idx += 2;
    }
    Ok(out)
}

fn encode_huffman_4x(literals: &[u8], table: &HuffmanTable) -> Result<Vec<u8>, ZstdEncodeError> {
    if literals.len() < 12 {
        return Err(ZstdEncodeError::InvalidInput);
    }
    let segment_size = literals.len().div_ceil(4);
    let mut segments = [
        &literals[0..0],
        &literals[0..0],
        &literals[0..0],
        &literals[0..0],
    ];
    let mut start = 0;
    for segment in segments.iter_mut().take(3) {
        let end = (start + segment_size).min(literals.len());
        *segment = &literals[start..end];
        start = end;
    }
    segments[3] = &literals[start..];

    let mut streams = Vec::with_capacity(4);
    for seg in segments.iter() {
        let stream = huffman::encode_with_table(seg, table)?;
        if stream.is_empty() || stream.len() > u16::MAX as usize {
            return Err(ZstdEncodeError::InvalidInput);
        }
        streams.push(stream);
    }

    let mut out = Vec::with_capacity(6 + streams.iter().map(|s| s.len()).sum::<usize>());
    for stream in streams.iter().take(3) {
        let size = stream.len() as u16;
        out.extend_from_slice(&size.to_le_bytes());
    }
    for stream in streams {
        out.extend_from_slice(&stream);
    }
    Ok(out)
}

fn build_sequence_section(seqs: &[SeqDef]) -> Result<Vec<u8>, ZstdEncodeError> {
    let mut out = Vec::new();
    let nb_seq = seqs.len();
    if nb_seq < 128 {
        out.push(nb_seq as u8);
    } else if nb_seq < LONGNBSEQ {
        out.push(((nb_seq >> 8) as u8) + 0x80);
        out.push(nb_seq as u8);
    } else {
        out.push(0xFF);
        let adj = (nb_seq - LONGNBSEQ) as u16;
        out.extend_from_slice(&adj.to_le_bytes());
    }
    if nb_seq == 0 {
        return Ok(out);
    }
    out.push(0);
    let bitstream = encode_sequences_bitstream(seqs)?;
    out.extend_from_slice(&bitstream);
    Ok(out)
}

fn encode_sequences_bitstream(seqs: &[SeqDef]) -> Result<Vec<u8>, ZstdEncodeError> {
    let ll_table = default_ll_table()?;
    let ml_table = default_ml_table()?;
    let of_table = default_of_table()?;
    let ll_base = ll_base_table();
    let ml_base = ml_base_table();

    let mut ll_codes = Vec::with_capacity(seqs.len());
    let mut ml_codes = Vec::with_capacity(seqs.len());
    let mut of_codes = Vec::with_capacity(seqs.len());
    for seq in seqs {
        let ll_code = ll_code(seq.lit_len);
        let ml_base_val = seq
            .match_len
            .checked_sub(MIN_MATCH)
            .ok_or(ZstdEncodeError::InvalidInput)?;
        let ml_code = ml_code(ml_base_val);
        let of_code = of_code(seq.offset);
        if of_code as usize > DEFAULT_MAX_OFF {
            return Err(ZstdEncodeError::InvalidInput);
        }
        ll_codes.push(ll_code as u16);
        ml_codes.push(ml_code as u16);
        of_codes.push(of_code as u16);
    }

    let mut writer = ZstdBitWriter::with_capacity(seqs.len().saturating_mul(8).saturating_add(8));
    let last = seqs.len() - 1;
    let mut state_ll = fse_init_state2(&ll_table, ll_codes[last]);
    let mut state_ml = fse_init_state2(&ml_table, ml_codes[last]);
    let mut state_of = fse_init_state2(&of_table, of_codes[last]);

    let last_seq = seqs[last];
    let ll_bits = LL_BITS[ll_codes[last] as usize] as u32;
    let ml_bits = ML_BITS[ml_codes[last] as usize] as u32;
    let of_bits = of_codes[last] as u32;
    writer.add_bits(last_seq.lit_len as u64, ll_bits)?;
    writer.add_bits((last_seq.match_len - MIN_MATCH) as u64, ml_bits)?;
    writer.add_bits(last_seq.offset as u64, of_bits)?;

    for idx in (0..last).rev() {
        let ll_code = ll_codes[idx];
        let ml_code = ml_codes[idx];
        let of_code = of_codes[idx];
        fse_encode_symbol(&mut writer, &of_table, &mut state_of, of_code)?;
        fse_encode_symbol(&mut writer, &ml_table, &mut state_ml, ml_code)?;
        fse_encode_symbol(&mut writer, &ll_table, &mut state_ll, ll_code)?;

        let seq = seqs[idx];
        let ll_bits = LL_BITS[ll_code as usize] as u32;
        let ml_bits = ML_BITS[ml_code as usize] as u32;
        let of_bits = of_code as u32;
        let ml_base_val = seq
            .match_len
            .checked_sub(MIN_MATCH)
            .ok_or(ZstdEncodeError::InvalidInput)?;
        let ll_base_val = ll_base[ll_code as usize];
        let ml_base_val_min = ml_base[ml_code as usize];
        if seq.lit_len < ll_base_val || ml_base_val < ml_base_val_min {
            return Err(ZstdEncodeError::InvalidInput);
        }
        writer.add_bits(seq.lit_len as u64, ll_bits)?;
        writer.add_bits(ml_base_val as u64, ml_bits)?;
        writer.add_bits(seq.offset as u64, of_bits)?;
    }

    fse_flush_state(&mut writer, &ml_table, state_ml)?;
    fse_flush_state(&mut writer, &of_table, state_of)?;
    fse_flush_state(&mut writer, &ll_table, state_ll)?;
    writer.close()
}

fn write_block(out: &mut Vec<u8>, block: &Block, last: bool) -> Result<(), ZstdEncodeError> {
    let (block_type, content_len, content) = match block {
        Block::Raw(data) => (0u8, data.len(), data.as_slice()),
        Block::Rle { byte, len } => (1u8, *len, std::slice::from_ref(byte)),
        Block::Compressed(data) => (2u8, data.len(), data.as_slice()),
    };
    if content_len >= (1 << 21) {
        return Err(ZstdEncodeError::InvalidInput);
    }
    let mut header = (content_len as u32) << 3;
    header |= (block_type as u32) << 1;
    if last {
        header |= 1;
    }
    out.extend_from_slice(&header.to_le_bytes()[..3]);
    out.extend_from_slice(content);
    Ok(())
}

fn ll_code(lit_len: u32) -> u32 {
    if lit_len > 63 {
        highbit32(lit_len) + 19
    } else {
        LL_CODE[lit_len as usize] as u32
    }
}

fn ml_code(ml_base: u32) -> u32 {
    if ml_base > 127 {
        highbit32(ml_base) + 36
    } else {
        ML_CODE[ml_base as usize] as u32
    }
}

fn of_code(offset: u32) -> u32 {
    highbit32(offset)
}

fn highbit32(value: u32) -> u32 {
    31 - value.leading_zeros()
}

fn ll_base_table() -> &'static [u32] {
    static BASE: OnceLock<Vec<u32>> = OnceLock::new();
    BASE.get_or_init(|| build_base_table(MAX_LL as u32, ll_code, 1 << 16))
}

fn ml_base_table() -> &'static [u32] {
    static BASE: OnceLock<Vec<u32>> = OnceLock::new();
    BASE.get_or_init(|| build_base_table(MAX_ML as u32, ml_code, 1 << 16))
}

fn build_base_table(max_code: u32, code_fn: fn(u32) -> u32, max_value: u32) -> Vec<u32> {
    let mut base = vec![0u32; (max_code + 1) as usize];
    let mut seen = vec![false; (max_code + 1) as usize];
    for value in 0..=max_value {
        let code = code_fn(value) as usize;
        if code < base.len() && !seen[code] {
            base[code] = value;
            seen[code] = true;
        }
    }
    base
}

fn default_ll_table() -> Result<FseCTable, ZstdEncodeError> {
    static TABLE: OnceLock<Result<FseCTable, ZstdEncodeError>> = OnceLock::new();
    TABLE
        .get_or_init(|| {
            let (ct, _) = fse::build_tables(&LL_DEFAULT_NORM, MAX_LL, LL_DEFAULT_NORM_LOG)
                .map_err(ZstdEncodeError::Fse)?;
            Ok(ct)
        })
        .clone()
}

fn default_ml_table() -> Result<FseCTable, ZstdEncodeError> {
    static TABLE: OnceLock<Result<FseCTable, ZstdEncodeError>> = OnceLock::new();
    TABLE
        .get_or_init(|| {
            let (ct, _) = fse::build_tables(&ML_DEFAULT_NORM, MAX_ML, ML_DEFAULT_NORM_LOG)
                .map_err(ZstdEncodeError::Fse)?;
            Ok(ct)
        })
        .clone()
}

fn default_of_table() -> Result<FseCTable, ZstdEncodeError> {
    static TABLE: OnceLock<Result<FseCTable, ZstdEncodeError>> = OnceLock::new();
    TABLE
        .get_or_init(|| {
            let (ct, _) = fse::build_tables(&OF_DEFAULT_NORM, DEFAULT_MAX_OFF, OF_DEFAULT_NORM_LOG)
                .map_err(ZstdEncodeError::Fse)?;
            Ok(ct)
        })
        .clone()
}

fn default_ll_dtable() -> Result<FseDTable, ZstdDecodeError> {
    static TABLE: OnceLock<Result<FseDTable, ZstdDecodeError>> = OnceLock::new();
    TABLE
        .get_or_init(|| {
            let (_, dt) = fse::build_tables(&LL_DEFAULT_NORM, MAX_LL, LL_DEFAULT_NORM_LOG)?;
            Ok(dt)
        })
        .clone()
}

fn default_ml_dtable() -> Result<FseDTable, ZstdDecodeError> {
    static TABLE: OnceLock<Result<FseDTable, ZstdDecodeError>> = OnceLock::new();
    TABLE
        .get_or_init(|| {
            let (_, dt) = fse::build_tables(&ML_DEFAULT_NORM, MAX_ML, ML_DEFAULT_NORM_LOG)?;
            Ok(dt)
        })
        .clone()
}

fn default_of_dtable() -> Result<FseDTable, ZstdDecodeError> {
    static TABLE: OnceLock<Result<FseDTable, ZstdDecodeError>> = OnceLock::new();
    TABLE
        .get_or_init(|| {
            let (_, dt) =
                fse::build_tables(&OF_DEFAULT_NORM, DEFAULT_MAX_OFF, OF_DEFAULT_NORM_LOG)?;
            Ok(dt)
        })
        .clone()
}

fn fse_init_state2(ct: &FseCTable, symbol: u16) -> u32 {
    let tt = ct.symbol_tt[symbol as usize];
    let nb_bits_out = (tt.delta_nb_bits + (1 << 15)) >> 16;
    let value = (nb_bits_out << 16).wrapping_sub(tt.delta_nb_bits);
    let idx = ((value >> nb_bits_out) as i32 + tt.delta_find_state) as usize;
    ct.state_table[idx] as u32
}

fn fse_encode_symbol(
    writer: &mut ZstdBitWriter,
    ct: &FseCTable,
    state: &mut u32,
    symbol: u16,
) -> Result<(), ZstdEncodeError> {
    let tt = ct.symbol_tt[symbol as usize];
    let nb_bits_out = (*state + tt.delta_nb_bits) >> 16;
    writer.add_bits(*state as u64, nb_bits_out)?;
    let next = ((*state >> nb_bits_out) as i32 + tt.delta_find_state) as usize;
    *state = ct.state_table[next] as u32;
    Ok(())
}

fn fse_flush_state(
    writer: &mut ZstdBitWriter,
    ct: &FseCTable,
    state: u32,
) -> Result<(), ZstdEncodeError> {
    writer.add_bits(state as u64, ct.table_log as u32)?;
    Ok(())
}

fn fse_decode_symbol(
    table: &FseDTable,
    reader: &mut BitReaderRev<'_>,
    state: &mut u32,
) -> Result<u16, ZstdDecodeError> {
    let table_size = 1u32 << table.table_log;
    if *state < table_size {
        return Err(ZstdDecodeError::InvalidInput);
    }
    let idx = (*state - table_size) as usize;
    let entry = table.decode.get(idx).ok_or(ZstdDecodeError::InvalidInput)?;
    let bits = reader.read_bits(entry.nb_bits as u32)? as u32;
    *state = entry.new_state + bits + table_size;
    Ok(entry.symbol)
}

fn bitstream_end_mark_len(data: &[u8]) -> Result<u32, ZstdDecodeError> {
    if data.is_empty() {
        return Err(ZstdDecodeError::InvalidInput);
    }
    let last = *data.last().ok_or(ZstdDecodeError::InvalidInput)?;
    if last == 0 {
        return Err(ZstdDecodeError::InvalidInput);
    }
    let highest = 7u32.saturating_sub(last.leading_zeros());
    let total_bits = (data.len().saturating_sub(1) as u32)
        .saturating_mul(8)
        .saturating_add(highest + 1);
    total_bits
        .checked_sub(1)
        .ok_or(ZstdDecodeError::InvalidInput)
}

fn read_u8(data: &[u8], idx: &mut usize) -> Result<u8, ZstdDecodeError> {
    if *idx >= data.len() {
        return Err(ZstdDecodeError::InvalidInput);
    }
    let val = data[*idx];
    *idx += 1;
    Ok(val)
}

fn read_u16_le(data: &[u8], idx: &mut usize) -> Result<u16, ZstdDecodeError> {
    if *idx + 2 > data.len() {
        return Err(ZstdDecodeError::InvalidInput);
    }
    let val = u16::from_le_bytes([data[*idx], data[*idx + 1]]);
    *idx += 2;
    Ok(val)
}

fn read_u32_le(data: &[u8], idx: &mut usize) -> Result<u32, ZstdDecodeError> {
    if *idx + 4 > data.len() {
        return Err(ZstdDecodeError::InvalidInput);
    }
    let val = u32::from_le_bytes([data[*idx], data[*idx + 1], data[*idx + 2], data[*idx + 3]]);
    *idx += 4;
    Ok(val)
}

fn read_u64_le(data: &[u8], idx: &mut usize) -> Result<u64, ZstdDecodeError> {
    if *idx + 8 > data.len() {
        return Err(ZstdDecodeError::InvalidInput);
    }
    let val = u64::from_le_bytes([
        data[*idx],
        data[*idx + 1],
        data[*idx + 2],
        data[*idx + 3],
        data[*idx + 4],
        data[*idx + 5],
        data[*idx + 6],
        data[*idx + 7],
    ]);
    *idx += 8;
    Ok(val)
}

fn read_u24_le(data: &[u8], idx: &mut usize) -> Result<u32, ZstdDecodeError> {
    if *idx + 3 > data.len() {
        return Err(ZstdDecodeError::InvalidInput);
    }
    let val = u32::from_le_bytes([data[*idx], data[*idx + 1], data[*idx + 2], 0]);
    *idx += 3;
    Ok(val)
}

fn read_le_value(data: &[u8], offset: usize, len: usize) -> Result<u64, ZstdDecodeError> {
    if offset + len > data.len() {
        return Err(ZstdDecodeError::InvalidInput);
    }
    let mut out = 0u64;
    for i in 0..len {
        out |= (data[offset + i] as u64) << (i * 8);
    }
    Ok(out)
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

    fn read_bits(&mut self, bits: u32) -> Result<u64, ZstdDecodeError> {
        if bits > 56 {
            return Err(ZstdDecodeError::InvalidInput);
        }
        if bits == 0 {
            return Ok(0);
        }
        if bits > self.bit_pos {
            return Err(ZstdDecodeError::InvalidInput);
        }
        let mut value = 0u64;
        for i in 0..bits {
            self.bit_pos -= 1;
            let idx = self.bit_pos;
            let byte_idx = (idx >> 3) as usize;
            let bit_idx = idx & 7;
            if byte_idx >= self.data.len() {
                return Err(ZstdDecodeError::InvalidInput);
            }
            let bit = (self.data[byte_idx] >> bit_idx) & 1;
            let shift = bits - 1 - i;
            value |= (bit as u64) << shift;
        }
        Ok(value)
    }
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

    fn add_bits(&mut self, value: u64, bits: u32) -> Result<(), ZstdEncodeError> {
        if bits > 56 {
            return Err(ZstdEncodeError::InvalidInput);
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

    fn close(mut self) -> Result<Vec<u8>, ZstdEncodeError> {
        self.add_bits(1, 1)?;
        if self.bit_count > 0 {
            if self.out.len() >= self.max_bytes {
                return Err(ZstdEncodeError::Capacity);
            }
            self.out.push((self.buffer & 0xFF) as u8);
            self.buffer = 0;
            self.bit_count = 0;
        }
        Ok(self.out)
    }

    fn push_byte(&mut self) -> Result<(), ZstdEncodeError> {
        if self.out.len() >= self.max_bytes {
            return Err(ZstdEncodeError::Capacity);
        }
        self.out.push((self.buffer & 0xFF) as u8);
        self.buffer >>= 8;
        self.bit_count -= 8;
        Ok(())
    }
}

fn xxh32(data: &[u8]) -> u32 {
    xxh64(data) as u32
}

fn xxh64(data: &[u8]) -> u64 {
    const PRIME1: u64 = 0x9E3779B185EBCA87;
    const PRIME2: u64 = 0xC2B2AE3D27D4EB4F;
    const PRIME3: u64 = 0x165667B19E3779F9;
    const PRIME4: u64 = 0x85EBCA77C2B2AE63;
    const PRIME5: u64 = 0x27D4EB2F165667C5;

    let mut hash;
    let mut idx = 0usize;
    if data.len() >= 32 {
        let mut v1 = PRIME1.wrapping_add(PRIME2);
        let mut v2 = PRIME2;
        let mut v3 = 0u64;
        let mut v4 = PRIME1.wrapping_neg();
        while idx + 32 <= data.len() {
            v1 = round64(v1, read_u64(data, idx));
            v2 = round64(v2, read_u64(data, idx + 8));
            v3 = round64(v3, read_u64(data, idx + 16));
            v4 = round64(v4, read_u64(data, idx + 24));
            idx += 32;
        }
        hash = v1
            .rotate_left(1)
            .wrapping_add(v2.rotate_left(7))
            .wrapping_add(v3.rotate_left(12))
            .wrapping_add(v4.rotate_left(18));
        hash = merge_round(hash, v1);
        hash = merge_round(hash, v2);
        hash = merge_round(hash, v3);
        hash = merge_round(hash, v4);
    } else {
        hash = PRIME5;
    }

    hash = hash.wrapping_add(data.len() as u64);
    while idx + 8 <= data.len() {
        let k1 = round64(0, read_u64(data, idx));
        idx += 8;
        hash ^= k1;
        hash = hash
            .rotate_left(27)
            .wrapping_mul(PRIME1)
            .wrapping_add(PRIME4);
    }
    if idx + 4 <= data.len() {
        hash ^= (read_u32(data, idx) as u64).wrapping_mul(PRIME1);
        idx += 4;
        hash = hash
            .rotate_left(23)
            .wrapping_mul(PRIME2)
            .wrapping_add(PRIME3);
    }
    while idx < data.len() {
        hash ^= (data[idx] as u64).wrapping_mul(PRIME5);
        hash = hash.rotate_left(11).wrapping_mul(PRIME1);
        idx += 1;
    }
    avalanche64(hash)
}

fn round64(acc: u64, input: u64) -> u64 {
    const PRIME1: u64 = 0x9E3779B185EBCA87;
    const PRIME2: u64 = 0xC2B2AE3D27D4EB4F;
    let mut acc = acc.wrapping_add(input.wrapping_mul(PRIME2));
    acc = acc.rotate_left(31);
    acc.wrapping_mul(PRIME1)
}

fn merge_round(acc: u64, val: u64) -> u64 {
    const PRIME1: u64 = 0x9E3779B185EBCA87;
    const PRIME4: u64 = 0x85EBCA77C2B2AE63;
    let val = round64(0, val);
    let acc = acc ^ val;
    acc.wrapping_mul(PRIME1).wrapping_add(PRIME4)
}

fn avalanche64(mut hash: u64) -> u64 {
    const PRIME2: u64 = 0xC2B2AE3D27D4EB4F;
    const PRIME3: u64 = 0x165667B19E3779F9;
    hash ^= hash >> 33;
    hash = hash.wrapping_mul(PRIME2);
    hash ^= hash >> 29;
    hash = hash.wrapping_mul(PRIME3);
    hash ^= hash >> 32;
    hash
}

fn read_u64(data: &[u8], idx: usize) -> u64 {
    u64::from_le_bytes([
        data[idx],
        data[idx + 1],
        data[idx + 2],
        data[idx + 3],
        data[idx + 4],
        data[idx + 5],
        data[idx + 6],
        data[idx + 7],
    ])
}

fn read_u32(data: &[u8], idx: usize) -> u32 {
    u32::from_le_bytes([data[idx], data[idx + 1], data[idx + 2], data[idx + 3]])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GpuZstdSequence;
    use std::io::Write;

    fn literal_sequences(
        input_len: usize,
        chunk_size: usize,
    ) -> (Vec<GpuZstdSequence>, Vec<u32>, Vec<u32>) {
        let mut seqs = Vec::new();
        let mut counts = Vec::new();
        let mut offsets = Vec::new();
        let mut offset = 0u32;
        let mut start = 0usize;
        while start < input_len {
            let end = (start + chunk_size).min(input_len);
            let len = end - start;
            offsets.push(offset);
            counts.push(1);
            seqs.push(GpuZstdSequence {
                lit_len: len as u32,
                match_len: 0,
                offset: 0,
                reserved: 0,
            });
            offset = offset.saturating_add(1);
            start = end;
        }
        (seqs, counts, offsets)
    }

    fn fill_lcg(buf: &mut [u8], mut seed: u32) -> u32 {
        for byte in buf.iter_mut() {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            *byte = (seed >> 24) as u8;
        }
        seed
    }

    #[test]
    fn encode_frame_roundtrip_no_matches() {
        let payload = b"zstd frame no matches payload";
        let chunk_size = 1 << 15;
        let sequences = vec![GpuZstdSequence {
            lit_len: payload.len() as u32,
            match_len: 0,
            offset: 0,
            reserved: 0,
        }];
        let counts = vec![1u32];
        let offsets = vec![0u32];
        let encoded = encode_frame(payload, chunk_size, &counts, &offsets, &sequences, false)
            .expect("encode");
        let decoded = zstd::decode_all(std::io::Cursor::new(&encoded)).expect("decode");
        assert_eq!(decoded, payload);
    }

    #[test]
    fn xxh32_matches_zstd_checksum() {
        let payload = b"zstd checksum match payload";
        let mut encoder = zstd::stream::Encoder::new(Vec::new(), 1).expect("encoder");
        encoder.include_checksum(true).expect("checksum");
        encoder.write_all(payload).expect("write");
        let compressed = encoder.finish().expect("finish");
        let checksum_bytes = &compressed[compressed.len().saturating_sub(4)..];
        let expected = u32::from_le_bytes(
            checksum_bytes
                .try_into()
                .expect("checksum bytes should be 4"),
        );
        assert_eq!(xxh32(payload), expected);
    }

    #[test]
    fn encode_frame_roundtrip_multi_chunk_checksum() {
        let chunk_size = 1 << 10;
        let mut payload = vec![0u8; chunk_size * 2 + 137];
        fill_lcg(&mut payload, 0x1234_5678);
        let (sequences, counts, offsets) = literal_sequences(payload.len(), chunk_size);
        let encoded = encode_frame(&payload, chunk_size, &counts, &offsets, &sequences, true)
            .expect("encode");
        let decoded = zstd::decode_all(std::io::Cursor::new(&encoded)).expect("decode");
        assert_eq!(decoded, payload);
    }

    #[test]
    fn encode_frame_roundtrip_with_match() {
        let payload = b"abcabcabc";
        let sequences = vec![
            GpuZstdSequence {
                lit_len: 3,
                match_len: 6,
                offset: 3,
                reserved: 0,
            },
            GpuZstdSequence {
                lit_len: 0,
                match_len: 0,
                offset: 0,
                reserved: 0,
            },
        ];
        let counts = vec![2u32];
        let offsets = vec![0u32];
        let encoded =
            encode_frame(payload, 1 << 15, &counts, &offsets, &sequences, false).expect("encode");
        let decoded = zstd::decode_all(std::io::Cursor::new(&encoded)).expect("decode");
        assert_eq!(decoded, payload);
    }

    #[test]
    fn decode_frame_roundtrip_with_match() {
        let payload = b"abcabcabc";
        let sequences = vec![
            GpuZstdSequence {
                lit_len: 3,
                match_len: 6,
                offset: 3,
                reserved: 0,
            },
            GpuZstdSequence {
                lit_len: 0,
                match_len: 0,
                offset: 0,
                reserved: 0,
            },
        ];
        let counts = vec![2u32];
        let offsets = vec![0u32];
        let encoded =
            encode_frame(payload, 1 << 15, &counts, &offsets, &sequences, false).expect("encode");
        let decoded = decode_frame(&encoded).expect("decode");
        assert_eq!(decoded, payload);
    }

    #[test]
    fn decode_frame_roundtrip_large_multi_chunk_checksum() {
        let chunk_size = 1 << 15;
        let mut payload = vec![0u8; chunk_size * 2 + 137];
        fill_lcg(&mut payload, 0xCAFE_BABE);
        let (sequences, counts, offsets) = literal_sequences(payload.len(), chunk_size);
        let encoded = encode_frame(&payload, chunk_size, &counts, &offsets, &sequences, true)
            .expect("encode");
        let decoded = decode_frame(&encoded).expect("decode");
        assert_eq!(decoded, payload);
    }

    #[test]
    fn decode_frame_rejects_bad_checksum() {
        let payload = b"checksum payload";
        let sequences = vec![GpuZstdSequence {
            lit_len: payload.len() as u32,
            match_len: 0,
            offset: 0,
            reserved: 0,
        }];
        let counts = vec![1u32];
        let offsets = vec![0u32];
        let mut encoded =
            encode_frame(payload, 1 << 15, &counts, &offsets, &sequences, true).expect("encode");
        let last = encoded.len().saturating_sub(1);
        encoded[last] ^= 0xFF;
        let err = decode_frame(&encoded).expect_err("bad checksum");
        assert!(matches!(err, ZstdDecodeError::InvalidInput));
    }

    #[test]
    fn encode_frame_is_deterministic() {
        let payload = b"abcabcabcabc";
        let sequences = vec![
            GpuZstdSequence {
                lit_len: 3,
                match_len: 9,
                offset: 3,
                reserved: 0,
            },
            GpuZstdSequence {
                lit_len: 0,
                match_len: 0,
                offset: 0,
                reserved: 0,
            },
        ];
        let counts = vec![2u32];
        let offsets = vec![0u32];
        let first =
            encode_frame(payload, 1 << 10, &counts, &offsets, &sequences, false).expect("encode");
        let second =
            encode_frame(payload, 1 << 10, &counts, &offsets, &sequences, false).expect("encode");
        assert_eq!(first, second);
    }

    #[test]
    fn encode_frame_rejects_invalid_sequences() {
        let payload = b"short";
        let sequences = vec![GpuZstdSequence {
            lit_len: 10,
            match_len: 0,
            offset: 0,
            reserved: 0,
        }];
        let counts = vec![1u32];
        let offsets = vec![0u32];
        let err = encode_frame(payload, 1 << 10, &counts, &offsets, &sequences, false)
            .expect_err("invalid sequences should fail");
        assert!(matches!(err, ZstdEncodeError::InvalidInput));
    }

    #[test]
    fn encode_frame_roundtrip_corpus() {
        let chunk_size = 1 << 10;
        let mut seed = 0x9E37_79B9;
        for size in [1usize, 3, 7, 15, 31, 64, 128, 1024, 2048] {
            let mut payload = vec![0u8; size];
            seed = fill_lcg(&mut payload, seed);
            let (sequences, counts, offsets) = literal_sequences(size, chunk_size);
            let encoded = encode_frame(&payload, chunk_size, &counts, &offsets, &sequences, false)
                .expect("encode");
            let decoded = zstd::decode_all(std::io::Cursor::new(&encoded)).expect("decode");
            assert_eq!(decoded, payload);
        }
    }
}
