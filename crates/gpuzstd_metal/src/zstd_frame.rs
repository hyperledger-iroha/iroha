use std::sync::OnceLock;

use crate::{
    GpuZstdSequence,
    fse::{self, FseCTable},
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

    let header = huffman_weights_header(&huf)?;
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
        if weight < 0 || weight > 15 {
            return Err(ZstdEncodeError::InvalidInput);
        }
        weights.push(weight as u8);
    }
    let mut out = Vec::with_capacity((max_symbol + 2) / 2 + 1);
    out.push(128u8 + (max_symbol as u8 - 1));
    let mut idx = 0;
    while idx <= max_symbol {
        let hi = weights[idx];
        let lo = if idx + 1 <= max_symbol {
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
    let segment_size = (literals.len() + 3) / 4;
    let mut segments = [
        &literals[0..0],
        &literals[0..0],
        &literals[0..0],
        &literals[0..0],
    ];
    let mut start = 0;
    for i in 0..3 {
        let end = (start + segment_size).min(literals.len());
        segments[i] = &literals[start..end];
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
    for i in 0..3 {
        let size = streams[i].len() as u16;
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
            let (ct, _) =
                fse::build_tables(&LL_DEFAULT_NORM, MAX_LL, LL_DEFAULT_NORM_LOG).map_err(|e| {
                    ZstdEncodeError::Fse(e)
                })?;
            Ok(ct)
        })
        .clone()
}

fn default_ml_table() -> Result<FseCTable, ZstdEncodeError> {
    static TABLE: OnceLock<Result<FseCTable, ZstdEncodeError>> = OnceLock::new();
    TABLE
        .get_or_init(|| {
            let (ct, _) =
                fse::build_tables(&ML_DEFAULT_NORM, MAX_ML, ML_DEFAULT_NORM_LOG).map_err(|e| {
                    ZstdEncodeError::Fse(e)
                })?;
            Ok(ct)
        })
        .clone()
}

fn default_of_table() -> Result<FseCTable, ZstdEncodeError> {
    static TABLE: OnceLock<Result<FseCTable, ZstdEncodeError>> = OnceLock::new();
    TABLE
        .get_or_init(|| {
            let (ct, _) =
                fse::build_tables(&OF_DEFAULT_NORM, DEFAULT_MAX_OFF, OF_DEFAULT_NORM_LOG)
                    .map_err(|e| ZstdEncodeError::Fse(e))?;
            Ok(ct)
        })
        .clone()
}

fn fse_init_state2(ct: &FseCTable, symbol: u16) -> u32 {
    let tt = ct.symbol_tt[symbol as usize];
    let nb_bits_out = ((tt.delta_nb_bits + (1 << 15)) >> 16) as u32;
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
    let nb_bits_out = ((*state + tt.delta_nb_bits) >> 16) as u32;
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
        if bits > 0 && bits < 64 && (value >> bits) != 0 {
            return Err(ZstdEncodeError::InvalidInput);
        }
        if bits == 0 {
            return Ok(());
        }
        self.buffer |= value << self.bit_count;
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
    const PRIME1: u32 = 0x9E3779B1;
    const PRIME2: u32 = 0x85EBCA77;
    const PRIME3: u32 = 0xC2B2AE3D;
    const PRIME4: u32 = 0x27D4EB2F;
    const PRIME5: u32 = 0x165667B1;

    let mut hash;
    let mut idx = 0usize;
    if data.len() >= 16 {
        let mut v1 = PRIME1.wrapping_add(PRIME2);
        let mut v2 = PRIME2;
        let mut v3 = 0u32;
        let mut v4 = PRIME1.wrapping_neg();
        while idx + 16 <= data.len() {
            v1 = round(v1, read_u32(data, idx));
            v2 = round(v2, read_u32(data, idx + 4));
            v3 = round(v3, read_u32(data, idx + 8));
            v4 = round(v4, read_u32(data, idx + 12));
            idx += 16;
        }
        hash = v1
            .rotate_left(1)
            .wrapping_add(v2.rotate_left(7))
            .wrapping_add(v3.rotate_left(12))
            .wrapping_add(v4.rotate_left(18));
    } else {
        hash = PRIME5;
    }
    hash = hash.wrapping_add(data.len() as u32);
    while idx + 4 <= data.len() {
        hash = hash.wrapping_add(read_u32(data, idx).wrapping_mul(PRIME3));
        hash = hash.rotate_left(17).wrapping_mul(PRIME4);
        idx += 4;
    }
    while idx < data.len() {
        hash = hash.wrapping_add((data[idx] as u32).wrapping_mul(PRIME5));
        hash = hash.rotate_left(11).wrapping_mul(PRIME1);
        idx += 1;
    }
    avalanche(hash)
}

fn round(acc: u32, input: u32) -> u32 {
    const PRIME1: u32 = 0x9E3779B1;
    const PRIME2: u32 = 0x85EBCA77;
    acc.wrapping_add(input.wrapping_mul(PRIME2))
        .rotate_left(13)
        .wrapping_mul(PRIME1)
}

fn avalanche(mut hash: u32) -> u32 {
    hash ^= hash >> 15;
    hash = hash.wrapping_mul(0x85EBCA77);
    hash ^= hash >> 13;
    hash = hash.wrapping_mul(0xC2B2AE3D);
    hash ^= hash >> 16;
    hash
}

fn read_u32(data: &[u8], idx: usize) -> u32 {
    u32::from_le_bytes([data[idx], data[idx + 1], data[idx + 2], data[idx + 3]])
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
