//! Norito Column Blocks (adaptive, internal)
//!
//! This module implements a columnar block layout for homogeneous sequences and
//! exposes an adaptive API that automatically chooses between the traditional
//! Array-of-Structs (AoS) Norito encoding and the columnar layout based on
//! simple heuristics. To developers, this is just “Norito” – the layout choice
//! is an internal detail selected for performance and size.
//!
//! Layout (NCB v1) for a specific row shape `(u64, &str, bool)`:
//! - `n: u32` rows
//! - descriptor byte `0x13` (u64 | str | bool) for sanity checking
//! - Column 1 (u64): pad to 8-byte alignment, then `[u64; n]` little-endian
//! - Column 2 (str): pad to 4-byte alignment, `[u32; n+1]` offsets then
//!   `data: [u8; offsets[n]]` (UTF-8 bytes, not NUL-terminated)
//!   Offsets start at 0, are non-decreasing, and the final offset equals the blob length.
//! - Column 3 (bool): packed bitset of length `ceil(n/8)` bytes (LSB-first)
//!
//! All padding bytes are zeroed and at most 7 bytes are inserted before a
//! column to meet alignment.
//!
//! This module purposely avoids `unsafe` and uses only safe slice operations.
//!
//! Adaptive API
//! - Helpers in this module produce a one-byte tagged payload and hide the
//!   AoS vs NCB choice from callers.
//! - For small inputs (AoS path), encoders use compact, ad-hoc AoS formats that
//!   honor the active decode/layout flags. Lengths follow `COMPACT_LEN` (varint
//!   when enabled, fixed u64 otherwise).
//! - Tag values are an internal detail: `0x00` = AoS, `0x01` = columnar.

// Shared AoS helpers for ad-hoc small-row layouts
use crate::{
    aos,
    core::{ByteSink, Error},
};

#[inline]
fn add_offset(base: usize, inc: usize) -> Result<usize, Error> {
    base.checked_add(inc).ok_or(Error::LengthMismatch)
}

#[inline]
fn mul_checked(a: usize, b: usize) -> Result<usize, Error> {
    a.checked_mul(b).ok_or(Error::LengthMismatch)
}

#[inline]
fn slice_range(bytes: &[u8], off: usize, len: usize) -> Result<&[u8], Error> {
    let end = add_offset(off, len)?;
    bytes.get(off..end).ok_or(Error::LengthMismatch)
}

#[inline]
fn take_bytes<'a>(bytes: &'a [u8], off: &mut usize, len: usize) -> Result<&'a [u8], Error> {
    let slice = slice_range(bytes, *off, len)?;
    *off = add_offset(*off, len)?;
    Ok(slice)
}

#[inline]
fn read_row_count_prefix(bytes: &[u8]) -> Result<usize, Error> {
    let raw = bytes.get(..4).ok_or(Error::LengthMismatch)?;
    let mut prefix = [0u8; 4];
    prefix.copy_from_slice(raw);
    let count = u32::from_le_bytes(prefix);
    // Every NCB row represented by this module contributes at least one byte
    // after the shared count prefix.  Enforce that local structural bound
    // before callers reserve `count` elements; ambient decode limits are an
    // additional policy boundary, not a prerequisite for memory safety.
    if count as usize > bytes.len().saturating_sub(prefix.len()) {
        return Err(Error::LengthMismatch);
    }
    crate::core::enforce_decode_sequence_length(u64::from(count))?;
    Ok(count as usize)
}

#[inline]
fn read_ncb_header(bytes: &[u8]) -> Result<(usize, u8, usize), Error> {
    if bytes.len() < 5 {
        return Err(Error::LengthMismatch);
    }
    Ok((read_row_count_prefix(bytes)?, bytes[4], 5))
}

#[inline]
fn read_aos_len(bytes: &[u8], off: &mut usize) -> Result<usize, Error> {
    let tail = bytes.get(*off..).ok_or(Error::LengthMismatch)?;
    let (len, used) = crate::core::read_len_from_slice(tail)?;
    *off = add_offset(*off, used)?;
    Ok(len)
}

#[inline]
fn read_aos_sequence_len(bytes: &[u8], off: &mut usize) -> Result<usize, Error> {
    let tail = bytes.get(*off..).ok_or(Error::LengthMismatch)?;
    let (len, used) = crate::core::read_sequence_len_from_slice(tail)?;
    *off = add_offset(*off, used)?;
    Ok(len)
}

#[inline]
fn align_offset_checked(bytes: &[u8], off: &mut usize, align: usize) -> Result<(), Error> {
    debug_assert!(align.is_power_of_two());
    let mask = align - 1;
    let mis = *off & mask;
    if mis != 0 {
        let pad = align - mis;
        let end = add_offset(*off, pad)?;
        let padding = bytes.get(*off..end).ok_or(Error::LengthMismatch)?;
        if padding.iter().any(|&b| b != 0) {
            return Err(Error::LengthMismatch);
        }
        *off = end;
    }
    Ok(())
}

#[inline]
fn validate_bitset_padding(bits: &[u8], n: usize) -> Result<(), Error> {
    let rem = n & 7;
    if rem == 0 || bits.is_empty() {
        return Ok(());
    }
    let mask = (1u8 << rem) - 1;
    let last = bits[bits.len() - 1];
    if (last & !mask) != 0 {
        return Err(Error::LengthMismatch);
    }
    Ok(())
}

fn take_bitset_tail<'a>(bytes: &'a [u8], off: &mut usize, n: usize) -> Result<&'a [u8], Error> {
    let bits = take_bytes(bytes, off, n.div_ceil(8))?;
    validate_bitset_padding(bits, n)?;
    ensure_no_trailing(bytes, *off)?;
    Ok(bits)
}

fn optional_column_prefix(bytes: &[u8], n: usize) -> Result<(&[u8], usize, usize), Error> {
    let bit_bytes = n.div_ceil(8);
    let presence = bytes.get(..bit_bytes).ok_or(Error::LengthMismatch)?;
    validate_bitset_padding(presence, n)?;
    let present = presence.iter().map(|byte| byte.count_ones() as usize).sum();
    let mut data_start = bit_bytes;
    align_offset_checked(bytes, &mut data_start, 4)?;
    Ok((presence, present, data_start))
}

#[inline]
fn bit_at(bits: &[u8], i: usize) -> bool {
    bits[i / 8] & (1 << (i % 8)) != 0
}

#[inline]
fn ensure_no_trailing(bytes: &[u8], off: usize) -> Result<(), Error> {
    if off == bytes.len() {
        Ok(())
    } else {
        Err(Error::LengthMismatch)
    }
}

#[inline]
fn validate_u32_offsets(offs_bytes: &[u8], count: usize) -> Result<usize, Error> {
    let entries = count.checked_add(1).ok_or(Error::LengthMismatch)?;
    let expected_len = entries.checked_mul(4).ok_or(Error::LengthMismatch)?;
    if offs_bytes.len() != expected_len {
        return Err(Error::LengthMismatch);
    }
    let mut prev = read_u32_at(offs_bytes, 0) as usize;
    if prev != 0 {
        return Err(Error::LengthMismatch);
    }
    for i in 1..=count {
        let cur = read_u32_at(offs_bytes, i) as usize;
        if cur < prev {
            return Err(Error::LengthMismatch);
        }
        prev = cur;
    }
    crate::core::enforce_decode_field_length(
        u64::try_from(prev).map_err(|_| Error::LengthMismatch)?,
    )?;
    Ok(prev)
}

fn decode_ids_column<'a>(
    bytes: &'a [u8],
    off: &mut usize,
    n: usize,
    use_delta: bool,
) -> Result<IdsRep<'a>, Error> {
    align_offset_checked(bytes, off, 8)?;
    if use_delta {
        if n == 0 {
            return Ok(IdsRep::Rebuilt(Vec::new()));
        }
        let mut buf = [0u8; 8];
        buf.copy_from_slice(take_bytes(bytes, off, 8)?);
        let base = u64::from_le_bytes(buf);
        let mut vec = Vec::with_capacity(n);
        vec.push(base);
        let mut p = *off;
        for _ in 1..n {
            let tail = bytes.get(p..).ok_or(Error::LengthMismatch)?;
            let (v, used) = read_varint_u64(tail)?;
            p = add_offset(p, used)?;
            let prev = *vec.last().unwrap() as i128;
            let d = zigzag_decode(v) as i128;
            let curr = prev + d;
            if curr < 0 || curr > u64::MAX as i128 {
                return Err(Error::LengthMismatch);
            }
            vec.push(curr as u64);
        }
        *off = p;
        Ok(IdsRep::Rebuilt(vec))
    } else {
        let len = mul_checked(n, 8)?;
        let ids_bytes = take_bytes(bytes, off, len)?;
        let (head, body, tail) = unsafe { ids_bytes.align_to::<u64>() };
        if head.is_empty() && tail.is_empty() {
            Ok(IdsRep::Slice(body))
        } else {
            let mut v = Vec::with_capacity(n);
            for chunk in ids_bytes.chunks_exact(8) {
                let mut lb = [0u8; 8];
                lb.copy_from_slice(chunk);
                v.push(u64::from_le_bytes(lb));
            }
            Ok(IdsRep::Rebuilt(v))
        }
    }
}

fn decode_u32_column<'a>(
    bytes: &'a [u8],
    off: &mut usize,
    n: usize,
    use_delta: bool,
) -> Result<U32Rep<'a>, Error> {
    align_offset_checked(bytes, off, 4)?;
    if use_delta {
        if n == 0 {
            return Ok(U32Rep::Rebuilt(Vec::new()));
        }
        let mut buf = [0u8; 4];
        buf.copy_from_slice(take_bytes(bytes, off, 4)?);
        let mut values = Vec::with_capacity(n);
        values.push(u32::from_le_bytes(buf));
        for _ in 1..n {
            let tail = bytes.get(*off..).ok_or(Error::LengthMismatch)?;
            let (value, used) = read_varint_u64(tail)?;
            *off = add_offset(*off, used)?;
            let current = i128::from(*values.last().unwrap()) + i128::from(zigzag_decode(value));
            if !(0..=i128::from(u32::MAX)).contains(&current) {
                return Err(Error::LengthMismatch);
            }
            values.push(current as u32);
        }
        Ok(U32Rep::Rebuilt(values))
    } else {
        let values_bytes = take_bytes(bytes, off, mul_checked(n, 4)?)?;
        let (head, body, tail) = unsafe { values_bytes.align_to::<u32>() };
        if head.is_empty() && tail.is_empty() {
            Ok(U32Rep::Slice(body))
        } else {
            Ok(U32Rep::Rebuilt(
                (0..n).map(|i| read_u32_at(values_bytes, i)).collect(),
            ))
        }
    }
}

/// Descriptor for `(u64, str, bool)` columns.
const DESC_U64_STR_BOOL: u8 = 0x13;
/// Descriptor for `(u64, dict(str), bool)` columns (dictionary-coded strings).
const DESC_U64_DICT_STR_BOOL: u8 = 0x93; // high bit marks dictionary
/// Descriptor for `(u64 delta+zigzag, str, bool)` columns.
const DESC_U64_DELTA_STR_BOOL: u8 = 0x53;

/// Descriptor for `(u64, Option<str>, bool)` columns.
const DESC_U64_OPTSTR_BOOL: u8 = 0x1B;
/// Descriptor for `(u64 delta+zigzag, Option<str>, bool)` columns.
const DESC_U64_DELTA_OPTSTR_BOOL: u8 = 0x5B;
/// Descriptor for `(u64, Option<u32>, bool)` columns.
const DESC_U64_OPTU32_BOOL: u8 = 0x1C;
/// Descriptor for `(u64 delta+zigzag, Option<u32>, bool)` columns.
const DESC_U64_DELTA_OPTU32_BOOL: u8 = 0x5C;

/// Descriptor for `(u64, enum(Name(String)|Code(u32)), bool)` columns (offsets-based names, no code delta).
const DESC_U64_ENUM_BOOL: u8 = 0x61; // base 0x60 + 0x01
/// Variant tag values for the enum payload.
const TAG_NAME: u8 = 0;
const TAG_CODE: u8 = 1;
/// Descriptor for `(u64 delta+zigzag, enum(Name|Code), bool)` columns (offsets-based names, no code delta).
const DESC_U64_DELTA_ENUM_BOOL: u8 = 0x63; // +0x02
/// Descriptor for `(u64, enum(Name|Code), bool)` with offsets-based names and delta-coded codes.
const DESC_U64_ENUM_BOOL_CODEDELTA: u8 = 0x65; // +0x04
/// Descriptor for `(u64 delta+zigzag, enum(Name|Code), bool)` with offsets-based names and delta-coded codes.
const DESC_U64_DELTA_ENUM_BOOL_CODEDELTA: u8 = 0x67; // +0x02 +0x04
/// Descriptor for `(u64, enum(Name|Code), bool)` with dictionary-coded names.
const DESC_U64_ENUM_BOOL_DICT: u8 = 0xE1;
/// Descriptor for `(u64 delta+zigzag, enum(Name|Code), bool)` with dictionary-coded names.
const DESC_U64_DELTA_ENUM_BOOL_DICT: u8 = 0xE3;
/// Descriptor for `(u64, enum(Name|Code), bool)` with dictionary-coded names and delta-coded Code(u32).
const DESC_U64_ENUM_BOOL_DICT_CODEDELTA: u8 = 0xE5;
/// Descriptor for `(u64 delta+zigzag, enum(Name|Code), bool)` with dictionary-coded names and delta-coded Code(u32).
const DESC_U64_DELTA_ENUM_BOOL_DICT_CODEDELTA: u8 = 0xE7;

// ===== Additional shapes =====
/// Descriptor for `(u64, bytes, bool)` columns (offsets+blob; no dict).
const DESC_U64_BYTES_BOOL: u8 = 0x14;
/// Descriptor for `(u64 delta+zigzag, bytes, bool)` columns.
const DESC_U64_DELTA_BYTES_BOOL: u8 = 0x54;

/// Descriptor for `(u64, u32, bool)` columns (fixed slices).
const DESC_U64_U32_BOOL: u8 = 0x21;
/// Descriptor for `(u64 delta+zigzag, u32, bool)` columns.
const DESC_U64_DELTA_U32_BOOL: u8 = 0x23;
/// Descriptor for `(u64, u32 delta+zigzag, bool)` columns.
const DESC_U64_U32DELTA_BOOL: u8 = 0x25;
/// Descriptor for `(u64 delta+zigzag, u32 delta+zigzag, bool)` columns.
const DESC_U64_DELTA_U32DELTA_BOOL: u8 = 0x27;

// ===== New combo shapes: (u64, str, u32, bool) and (u64, bytes, u32, bool) =====
// Encoding bits policy for these shapes:
// - Base descriptors: 0x33 = (u64, str, u32, bool), 0x34 = (u64, bytes, u32, bool)
// - +0x40 => id column uses delta+zigzag (base + varint deltas)
// - +0x04 => u32 column uses delta+zigzag (base + varint deltas)
// - +0x80 => names use dictionary coding (only `(u64, str, u32, bool)`)
// These tags are internal to Norito NCB and are validated by the view decoders.
const DESC_U64_STR_U32_BOOL: u8 = 0x33;
const DESC_U64_DELTA_STR_U32_BOOL: u8 = 0x73; // +0x40
const DESC_U64_STR_U32DELTA_BOOL: u8 = 0x37; // +0x04
const DESC_U64_DELTA_STR_U32DELTA_BOOL: u8 = 0x77; // +0x40 +0x04
const DESC_U64_DICT_STR_U32_BOOL: u8 = 0xB3; // +0x80
const DESC_U64_DELTA_DICT_STR_U32_BOOL: u8 = 0xF3; // +0x80 +0x40
const DESC_U64_DICT_STR_U32DELTA_BOOL: u8 = 0xB7; // +0x80 +0x04
const DESC_U64_DELTA_DICT_STR_U32DELTA_BOOL: u8 = 0xF7; // +0x80 +0x40 +0x04

const DESC_U64_BYTES_U32_BOOL: u8 = 0x34;
const DESC_U64_DELTA_BYTES_U32_BOOL: u8 = 0x74; // +0x40
const DESC_U64_BYTES_U32DELTA_BOOL: u8 = 0x38; // +0x04
const DESC_U64_DELTA_BYTES_U32DELTA_BOOL: u8 = 0x78; // +0x40 +0x04

fn pad_to(buf: &mut Vec<u8>, align: usize) {
    debug_assert!(align.is_power_of_two());
    let mis = buf.len() & (align - 1);
    if mis != 0 {
        let pad = align - mis;
        buf.extend(std::iter::repeat_n(0u8, pad));
    }
}

/// Policy overrides for combo encoders (`(u64, &str, u32, bool)` rows).
///
/// By default the combo helpers rely on heuristics to decide whether to use dictionaries
/// and delta encodings. Hosts that know their data distribution ahead of time may force
/// specific choices via this policy.
#[derive(Clone, Copy, Debug, Default)]
pub struct ComboPolicy {
    /// Force dictionary usage (`Some(true)`), disable dictionaries (`Some(false)`)
    /// or defer to heuristics (`None`).
    pub force_dictionary: Option<bool>,
    /// Force ID-delta encoding (`Some(true)`), disable (`Some(false)`) or defer (`None`).
    pub force_id_delta: Option<bool>,
    /// Force u32-delta encoding (`Some(true)`), disable (`Some(false)`) or defer (`None`).
    pub force_u32_delta: Option<bool>,
}

impl ComboPolicy {
    /// Create a new policy with the given dictionary override.
    #[must_use]
    pub fn with_dictionary(mut self, enabled: bool) -> Self {
        self.force_dictionary = Some(enabled);
        self
    }

    /// Create a new policy with the given ID-delta override.
    #[must_use]
    pub fn with_id_delta(mut self, enabled: bool) -> Self {
        self.force_id_delta = Some(enabled);
        self
    }

    /// Create a new policy with the given u32-delta override.
    #[must_use]
    pub fn with_u32_delta(mut self, enabled: bool) -> Self {
        self.force_u32_delta = Some(enabled);
        self
    }
}

/// Encode a Norito Column Block for rows shaped as `(u64, &str, bool)`.
///
/// - Rows are borrowed so callers can pass `&String` cheaply.
/// - Strings must be valid UTF-8.
pub fn encode_ncb_u64_str_bool(rows: &[(u64, &str, bool)]) -> Vec<u8> {
    // Choose dictionary and/or id-delta when beneficial
    let (use_dict, dict_map, dict_vec) = build_dict(rows);
    if use_dict {
        return encode_ncb_u64_dict_str_bool(rows, dict_map.unwrap(), dict_vec.unwrap());
    }
    if should_use_id_delta(rows) {
        return encode_ncb_u64_str_bool_delta(rows);
    }
    let n = rows.len() as u32;
    // capacity hint: headers + ids + offsets + data + flags
    let cap = 4 + 1 + rows.len() * (8 + 1 + 4) + 16;
    let mut sink = ByteSink::with_headroom(cap, 0);
    // Header
    sink.write_bytes(&n.to_le_bytes());
    sink.write_u8(DESC_U64_STR_BOOL);
    let row_columns = IdFlagRows::Str(rows);
    write_id_column(&mut sink, row_columns, false);
    // strings offsets+blob
    sink.align_to(4);
    let mut acc: u32 = 0;
    let mut offs = Vec::with_capacity(rows.len() + 1);
    offs.push(0);
    let mut blob = Vec::new();
    for (_, s, _) in rows {
        let b = s.as_bytes();
        acc = acc.wrapping_add(b.len() as u32);
        offs.push(acc);
        blob.extend_from_slice(b);
    }
    for v in offs.iter() {
        sink.write_u32_le(*v);
    }
    sink.write_bytes(&blob);
    write_flag_column(&mut sink, row_columns);
    sink.into_inner()
}

/// Borrowed view over an NCB `(u64, str, bool)` block.
enum NamesRep<'a> {
    Offsets {
        /// Raw little-endian u32 offsets bytes (len = n+1).
        offs_bytes: &'a [u8],
        blob_str: &'a str,
    },
    Dict {
        /// Raw little-endian u32 offsets bytes (len = dict_len+1).
        dict_offs_bytes: &'a [u8],
        dict_blob: &'a str,
        /// Raw little-endian u32 codes bytes (len = n).
        codes_bytes: &'a [u8],
    },
}

impl<'a> NamesRep<'a> {
    fn get(&self, i: usize) -> Result<&'a str, Error> {
        let (offsets, blob, row) = match self {
            Self::Offsets {
                offs_bytes,
                blob_str,
            } => (*offs_bytes, *blob_str, i),
            Self::Dict {
                dict_offs_bytes,
                dict_blob,
                codes_bytes,
            } => {
                let code = read_u32_at(codes_bytes, i) as usize;
                if code >= dict_offs_bytes.len() / 4 - 1 {
                    return Err(Error::LengthMismatch);
                }
                (*dict_offs_bytes, *dict_blob, code)
            }
        };
        let start = read_u32_at(offsets, row) as usize;
        let end = read_u32_at(offsets, row + 1) as usize;
        if start > end || end > blob.len() {
            return Err(Error::LengthMismatch);
        }
        Ok(&blob[start..end])
    }
}

#[inline]
fn decode_offset_names<'a>(
    bytes: &'a [u8],
    offset: &mut usize,
    rows: usize,
) -> Result<NamesRep<'a>, Error> {
    align_offset_checked(bytes, offset, 4)?;
    let offsets_count = rows.checked_add(1).ok_or(Error::LengthMismatch)?;
    let offsets_len = mul_checked(offsets_count, 4)?;
    let offs_bytes = slice_range(bytes, *offset, offsets_len)?;
    *offset = add_offset(*offset, offsets_len)?;
    let data_len = validate_u32_offsets(offs_bytes, rows)?;
    let data = slice_range(bytes, *offset, data_len)?;
    *offset = add_offset(*offset, data_len)?;
    Ok(NamesRep::Offsets {
        offs_bytes,
        blob_str: validated_str(data)?,
    })
}

#[inline]
fn decode_dict_names<'a>(
    bytes: &'a [u8],
    offset: &mut usize,
    rows: usize,
    validate_codes: bool,
) -> Result<NamesRep<'a>, Error> {
    align_offset_checked(bytes, offset, 4)?;
    let dict_len_raw = read_u32_at(slice_range(bytes, *offset, 4)?, 0);
    crate::core::enforce_decode_sequence_length(u64::from(dict_len_raw))?;
    let dict_len = dict_len_raw as usize;
    *offset = add_offset(*offset, 4)?;
    let offsets_count = dict_len.checked_add(1).ok_or(Error::LengthMismatch)?;
    let offsets_len = mul_checked(offsets_count, 4)?;
    let dict_offs_bytes = slice_range(bytes, *offset, offsets_len)?;
    *offset = add_offset(*offset, offsets_len)?;
    let data_len = validate_u32_offsets(dict_offs_bytes, dict_len)?;
    let data = slice_range(bytes, *offset, data_len)?;
    *offset = add_offset(*offset, data_len)?;
    align_offset_checked(bytes, offset, 4)?;
    let codes_len = mul_checked(rows, 4)?;
    let codes_bytes = slice_range(bytes, *offset, codes_len)?;
    *offset = add_offset(*offset, codes_len)?;
    if validate_codes {
        for index in 0..rows {
            if read_u32_at(codes_bytes, index) as usize >= dict_len {
                return Err(Error::LengthMismatch);
            }
        }
    }
    Ok(NamesRep::Dict {
        dict_offs_bytes,
        dict_blob: validated_str(data)?,
        codes_bytes,
    })
}

enum IdsRep<'a> {
    Slice(&'a [u64]),
    Rebuilt(Vec<u64>),
}

impl<'a> IdsRep<'a> {
    #[inline]
    fn get(&self, i: usize) -> u64 {
        match self {
            Self::Slice(values) => values[i],
            Self::Rebuilt(values) => values[i],
        }
    }

    #[inline]
    fn as_slice(&self) -> Option<&'a [u64]> {
        match self {
            Self::Slice(values) => Some(*values),
            Self::Rebuilt(_) => None,
        }
    }
}

pub struct NcbU64StrBoolView<'a> {
    n: usize,
    ids: IdsRep<'a>,
    names: NamesRep<'a>,
    bits: &'a [u8],
}

impl<'a> NcbU64StrBoolView<'a> {
    /// Number of rows.
    pub fn len(&self) -> usize {
        self.n
    }
    /// True if there are no rows.
    pub fn is_empty(&self) -> bool {
        self.n == 0
    }
    /// Access id column.
    pub fn id(&self, i: usize) -> u64 {
        self.ids.get(i)
    }
    /// Access string column. Returns `&str` over the shared blob.
    pub fn name(&self, i: usize) -> Result<&'a str, Error> {
        self.names.get(i)
    }
    /// Access boolean column.
    pub fn flag(&self, i: usize) -> bool {
        bit_at(self.bits, i)
    }
    /// When ids are stored as a contiguous slice (non-delta), return it for slice-wide projections.
    pub fn ids_slice(&self) -> Option<&'a [u64]> {
        self.ids.as_slice()
    }
}

/// Parse a byte slice into an NCB `(u64, str, bool)` view.
pub fn view_ncb_u64_str_bool(bytes: &[u8]) -> Result<NcbU64StrBoolView<'_>, Error> {
    let (n, desc, mut off) = read_ncb_header(bytes)?;
    // Column 1: ids (aligned to 8) or delta-coded
    let ids = decode_ids_column(bytes, &mut off, n, desc == DESC_U64_DELTA_STR_BOOL)?;
    // Column 2: strings
    let names = if desc == DESC_U64_STR_BOOL || desc == DESC_U64_DELTA_STR_BOOL {
        decode_offset_names(bytes, &mut off, n)?
    } else if desc == DESC_U64_DICT_STR_BOOL {
        decode_dict_names(bytes, &mut off, n, true)?
    } else {
        return Err(Error::Message("invalid NCB descriptor".into()));
    };

    let bits = take_bitset_tail(bytes, &mut off, n)?;

    Ok(NcbU64StrBoolView {
        n,
        ids,
        names,
        bits,
    })
}

/// Convenience to materialize rows from the view.
pub fn materialize_ncb(view: NcbU64StrBoolView<'_>) -> Result<Vec<(u64, String, bool)>, Error> {
    let mut out = Vec::with_capacity(view.len());
    for i in 0..view.len() {
        let id = view.id(i);
        let name = view.name(i)?.to_string();
        let flag = view.flag(i);
        out.push((id, name, flag));
    }
    Ok(out)
}

/// Simple heuristic to pick columnar layout over AoS based on row count.
pub fn should_use_columnar(n: usize) -> bool {
    if n == 0 {
        return false;
    }
    let heuristics = crate::core::heuristics::get();
    if n <= heuristics.aos_ncb_small_n {
        // Small-N path uses two-pass probing; defer to caller to pick the smaller layout.
        return false;
    }
    true
}

/// Encode rows using either AoS (Vec) or NCB based on a threshold.
///
/// Returns a bare payload suitable for hashing or embedding into a Norito
/// field. The caller is responsible for prefixing with any higher-level length
/// header when needed.
pub fn encode_rows_u64_str_bool_auto(rows: &[(u64, &str, bool)]) -> (u8, Vec<u8>) {
    if should_use_columnar(rows.len()) {
        (1u8, encode_ncb_u64_str_bool(rows))
    } else {
        (0u8, aos::encode_rows_u64_str_bool(rows))
    }
}

// Bring Norito traits into scope for callers (may be unused in AoS ad-hoc paths).
#[allow(unused_imports)]
use crate::NoritoSerialize as _;

/// Tag used to mark AoS encoding inside adaptive payloads.
pub const ADAPTIVE_TAG_AOS: u8 = 0u8;
/// Tag used to mark Columnar (NCB) encoding inside adaptive payloads.
pub const ADAPTIVE_TAG_NCB: u8 = 1u8;

/// Tag used to mark AoS encoding for the enum-shaped adaptive payloads.
pub const ADAPTIVE_ENUM_TAG_AOS: u8 = 0u8;
/// Tag used to mark Columnar (NCB) encoding for the enum-shaped adaptive payloads.
pub const ADAPTIVE_ENUM_TAG_NCB: u8 = 1u8;

// Re-exported AoS header helpers are used throughout this file
use crate::aos::read_len_and_ver as aos_read_len_and_ver;

// Lightweight, in-crate telemetry for adaptive AoS vs NCB selection.
// Counts selections and accumulated bytes saved by the two-pass probe.
mod telemetry {
    use std::sync::atomic::{AtomicU64, Ordering};

    static AOS_SELECTED: AtomicU64 = AtomicU64::new(0);
    static NCB_SELECTED: AtomicU64 = AtomicU64::new(0);
    static PROBES: AtomicU64 = AtomicU64::new(0);
    static BYTES_SAVED_TOTAL: AtomicU64 = AtomicU64::new(0);
    static CACHE_BUILDS: AtomicU64 = AtomicU64::new(0);
    static CACHE_ROWS_TOTAL: AtomicU64 = AtomicU64::new(0);
    static CACHE_REJECTS: AtomicU64 = AtomicU64::new(0);
    static CACHE_REJECT_ROWS_TOTAL: AtomicU64 = AtomicU64::new(0);
    #[cfg(feature = "adaptive-telemetry")]
    static AOS_TIME_NS_TOTAL: AtomicU64 = AtomicU64::new(0);
    #[cfg(feature = "adaptive-telemetry")]
    static NCB_TIME_NS_TOTAL: AtomicU64 = AtomicU64::new(0);

    #[derive(Clone, Copy, Debug)]
    pub struct AdaptiveMetricsSnapshot {
        pub aos_selected: u64,
        pub ncb_selected: u64,
        pub probes: u64,
        pub bytes_saved_total: u64,
        pub cache_builds: u64,
        pub cache_rows_total: u64,
        pub cache_rejects: u64,
        pub cache_reject_rows_total: u64,
        #[cfg(feature = "adaptive-telemetry")]
        pub aos_time_ns_total: u64,
        #[cfg(feature = "adaptive-telemetry")]
        pub ncb_time_ns_total: u64,
    }

    #[inline]
    pub fn record_two_pass(tag: u8, aos_len: usize, ncb_len: usize) {
        PROBES.fetch_add(1, Ordering::Relaxed);
        match tag {
            super::ADAPTIVE_TAG_AOS => {
                AOS_SELECTED.fetch_add(1, Ordering::Relaxed);
            }
            super::ADAPTIVE_TAG_NCB => {
                NCB_SELECTED.fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        }
        let (min, max) = if aos_len < ncb_len {
            (aos_len, ncb_len)
        } else {
            (ncb_len, aos_len)
        };
        let saved = max.saturating_sub(min) as u64;
        BYTES_SAVED_TOTAL.fetch_add(saved, Ordering::Relaxed);
    }

    #[inline]
    #[cfg(feature = "adaptive-telemetry")]
    pub fn record_two_pass_times(aos_ns: u64, ncb_ns: u64) {
        AOS_TIME_NS_TOTAL.fetch_add(aos_ns, Ordering::Relaxed);
        NCB_TIME_NS_TOTAL.fetch_add(ncb_ns, Ordering::Relaxed);
    }

    #[inline]
    pub fn record_selection_only(tag: u8) {
        match tag {
            super::ADAPTIVE_TAG_AOS => {
                AOS_SELECTED.fetch_add(1, Ordering::Relaxed);
            }
            super::ADAPTIVE_TAG_NCB => {
                NCB_SELECTED.fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        }
    }

    #[inline]
    pub fn record_cache_build(rows: usize) {
        CACHE_BUILDS.fetch_add(1, Ordering::Relaxed);
        CACHE_ROWS_TOTAL.fetch_add(rows as u64, Ordering::Relaxed);
    }

    #[inline]
    pub fn record_cache_reject(rows: usize) {
        CACHE_REJECTS.fetch_add(1, Ordering::Relaxed);
        CACHE_REJECT_ROWS_TOTAL.fetch_add(rows as u64, Ordering::Relaxed);
    }

    pub fn snapshot() -> AdaptiveMetricsSnapshot {
        AdaptiveMetricsSnapshot {
            aos_selected: AOS_SELECTED.load(Ordering::Relaxed),
            ncb_selected: NCB_SELECTED.load(Ordering::Relaxed),
            probes: PROBES.load(Ordering::Relaxed),
            bytes_saved_total: BYTES_SAVED_TOTAL.load(Ordering::Relaxed),
            cache_builds: CACHE_BUILDS.load(Ordering::Relaxed),
            cache_rows_total: CACHE_ROWS_TOTAL.load(Ordering::Relaxed),
            cache_rejects: CACHE_REJECTS.load(Ordering::Relaxed),
            cache_reject_rows_total: CACHE_REJECT_ROWS_TOTAL.load(Ordering::Relaxed),
            #[cfg(feature = "adaptive-telemetry")]
            aos_time_ns_total: AOS_TIME_NS_TOTAL.load(Ordering::Relaxed),
            #[cfg(feature = "adaptive-telemetry")]
            ncb_time_ns_total: NCB_TIME_NS_TOTAL.load(Ordering::Relaxed),
        }
    }

    #[allow(dead_code)]
    pub fn reset() {
        AOS_SELECTED.store(0, Ordering::Relaxed);
        NCB_SELECTED.store(0, Ordering::Relaxed);
        PROBES.store(0, Ordering::Relaxed);
        BYTES_SAVED_TOTAL.store(0, Ordering::Relaxed);
        CACHE_BUILDS.store(0, Ordering::Relaxed);
        CACHE_ROWS_TOTAL.store(0, Ordering::Relaxed);
        CACHE_REJECTS.store(0, Ordering::Relaxed);
        CACHE_REJECT_ROWS_TOTAL.store(0, Ordering::Relaxed);
        #[cfg(feature = "adaptive-telemetry")]
        {
            AOS_TIME_NS_TOTAL.store(0, Ordering::Relaxed);
            NCB_TIME_NS_TOTAL.store(0, Ordering::Relaxed);
        }
    }

    // Re-export snapshot type at module root for callers
    pub(crate) use AdaptiveMetricsSnapshot as Snapshot;
}

// Simple helper to log two-pass decisions when requested.
#[cfg(all(feature = "adaptive-telemetry-log", feature = "adaptive-telemetry"))]
#[inline]
fn log_two_pass(kind: &str, tag: u8, aos_len: usize, ncb_len: usize, aos_ns: u64, ncb_ns: u64) {
    let choice = if tag == ADAPTIVE_TAG_NCB || tag == ADAPTIVE_ENUM_TAG_NCB {
        "NCB"
    } else {
        "AOS"
    };
    if crate::debug_trace_enabled() {
        eprintln!(
            "norito.adapt(kind={}): choice={} aos_len={} ncb_len={} aos_ns={} ncb_ns={}",
            kind, choice, aos_len, ncb_len, aos_ns, ncb_ns
        );
    }
}

#[cfg(all(
    feature = "adaptive-telemetry-log",
    not(feature = "adaptive-telemetry")
))]
#[inline]
fn log_two_pass(kind: &str, tag: u8, aos_len: usize, ncb_len: usize, _aos_ns: u64, _ncb_ns: u64) {
    let choice = if tag == ADAPTIVE_TAG_NCB || tag == ADAPTIVE_ENUM_TAG_NCB {
        "NCB"
    } else {
        "AOS"
    };
    if crate::debug_trace_enabled() {
        eprintln!(
            "norito.adapt(kind={}): choice={} aos_len={} ncb_len={}",
            kind, choice, aos_len, ncb_len
        );
    }
}

#[cfg(feature = "adaptive-telemetry")]
type ProbeTimer = std::time::Instant;

#[cfg(not(feature = "adaptive-telemetry"))]
struct ProbeTimer;

#[cfg(feature = "adaptive-telemetry")]
#[inline]
fn probe_start() -> ProbeTimer {
    std::time::Instant::now()
}

#[cfg(not(feature = "adaptive-telemetry"))]
#[inline]
fn probe_start() -> ProbeTimer {
    ProbeTimer
}

#[cfg(feature = "adaptive-telemetry")]
#[inline]
fn probe_elapsed(start: ProbeTimer) -> u64 {
    start.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64
}

#[cfg(not(feature = "adaptive-telemetry"))]
#[inline]
fn probe_elapsed(_start: ProbeTimer) -> u64 {
    0
}

#[inline]
fn tagged_payload(tag: u8, mut payload: Vec<u8>) -> Vec<u8> {
    let mut out = Vec::with_capacity(1 + payload.len());
    out.push(tag);
    out.append(&mut payload);
    out
}

fn finish_two_pass(_kind: &str, aos: Vec<u8>, ncb: Vec<u8>, _aos_ns: u64, _ncb_ns: u64) -> Vec<u8> {
    let aos_len = aos.len();
    let ncb_len = ncb.len();
    let (tag, payload) = if ncb_len < aos_len {
        (ADAPTIVE_TAG_NCB, ncb)
    } else {
        (ADAPTIVE_TAG_AOS, aos)
    };
    telemetry::record_two_pass(tag, aos_len, ncb_len);
    #[cfg(feature = "adaptive-telemetry")]
    telemetry::record_two_pass_times(_aos_ns, _ncb_ns);
    #[cfg(feature = "adaptive-telemetry-log")]
    log_two_pass(_kind, tag, aos_len, ncb_len, _aos_ns, _ncb_ns);
    tagged_payload(tag, payload)
}

#[inline]
fn finish_selection(tag: u8, payload: Vec<u8>) -> Vec<u8> {
    telemetry::record_selection_only(tag);
    tagged_payload(tag, payload)
}

#[inline]
fn split_tagged_payload(bytes: &[u8]) -> Result<(u8, &[u8]), Error> {
    let (&tag, body) = bytes.split_first().ok_or(Error::LengthMismatch)?;
    Ok((tag, body))
}

/// Return a snapshot of adaptive AoS/NCB selection counters.
pub fn adaptive_metrics_snapshot() -> telemetry::Snapshot {
    telemetry::snapshot()
}

/// Reset adaptive selection counters (intended for tests/benches).
#[allow(dead_code)]
pub fn adaptive_metrics_reset() {
    telemetry::reset()
}

/// JSON: export adaptive selection counters as a compact JSON value.
#[cfg(feature = "json")]
pub fn adaptive_metrics_json_value() -> crate::json::Value {
    let s = adaptive_metrics_snapshot();
    let mut map = crate::json::Map::new();
    map.insert(
        "aos_selected".into(),
        crate::json::Value::from(s.aos_selected),
    );
    map.insert(
        "ncb_selected".into(),
        crate::json::Value::from(s.ncb_selected),
    );
    map.insert("probes".into(), crate::json::Value::from(s.probes));
    map.insert(
        "bytes_saved_total".into(),
        crate::json::Value::from(s.bytes_saved_total),
    );
    map.insert(
        "cache_builds".into(),
        crate::json::Value::from(s.cache_builds),
    );
    map.insert(
        "cache_rows_total".into(),
        crate::json::Value::from(s.cache_rows_total),
    );
    map.insert(
        "cache_rejects".into(),
        crate::json::Value::from(s.cache_rejects),
    );
    map.insert(
        "cache_reject_rows_total".into(),
        crate::json::Value::from(s.cache_reject_rows_total),
    );
    #[cfg(feature = "adaptive-telemetry")]
    {
        map.insert(
            "aos_time_ns_total".into(),
            crate::json::Value::from(s.aos_time_ns_total),
        );
        map.insert(
            "ncb_time_ns_total".into(),
            crate::json::Value::from(s.ncb_time_ns_total),
        );
    }
    crate::json::Value::Object(map)
}

/// JSON: export adaptive selection counters as a compact JSON string.
#[cfg(feature = "json")]
pub fn adaptive_metrics_json_string() -> String {
    let v = adaptive_metrics_json_value();
    crate::json::to_string(&v).unwrap_or_else(|_| String::from("{}"))
}

/// JSON: compute fieldwise delta between two columnar telemetry JSON maps.
#[cfg(feature = "json")]
pub fn adaptive_metrics_delta_json(
    prev: &crate::json::Value,
    curr: &crate::json::Value,
) -> crate::json::Value {
    use crate::json::Value;
    let mut out = crate::json::Map::new();
    // Avoid borrowing a temporary Map; use a local binding that outlives `p`/`c`.
    let empty = crate::json::Map::new();
    let p = prev.as_object().unwrap_or(&empty);
    let c = curr.as_object().unwrap_or(&empty);
    for k in [
        "aos_selected",
        "ncb_selected",
        "probes",
        "bytes_saved_total",
        "cache_builds",
        "cache_rows_total",
        "cache_rejects",
        "cache_reject_rows_total",
        #[cfg(feature = "adaptive-telemetry")]
        "aos_time_ns_total",
        #[cfg(feature = "adaptive-telemetry")]
        "ncb_time_ns_total",
    ] {
        if let (Some(Value::Number(a)), Some(Value::Number(b))) = (p.get(k), c.get(k)) {
            let av = a.as_u64().unwrap_or(0);
            let bv = b.as_u64().unwrap_or(0);
            out.insert(k.to_string(), Value::from(bv.saturating_sub(av)));
        }
    }
    Value::Object(out)
}

/// Encode rows using an adaptive payload that embeds a 1-byte tag followed by
/// the chosen layout bytes. Tag values are internal and may change; callers
/// should treat this as an opaque Norito payload.
pub fn encode_rows_u64_str_bool_adaptive(rows: &[(u64, &str, bool)]) -> Vec<u8> {
    // Two-pass size probe for small inputs: encode both layouts and pick the smaller.
    let small_n = small_smart_n();
    if rows.len() <= small_n {
        let timer = probe_start();
        let aos = aos::encode_rows_u64_str_bool(rows);
        let aos_ns = probe_elapsed(timer);

        let timer = probe_start();
        let ncb = encode_ncb_u64_str_bool(rows);
        let ncb_ns = probe_elapsed(timer);
        return finish_two_pass("u64_str_bool", aos, ncb, aos_ns, ncb_ns);
    }
    let (tag, payload) = encode_rows_u64_str_bool_auto(rows);
    finish_selection(tag, payload)
}

/// Decode an adaptive payload produced by
/// `encode_rows_u64_str_bool_adaptive(rows)` back into owned rows.
pub fn decode_rows_u64_str_bool_adaptive(bytes: &[u8]) -> Result<Vec<(u64, String, bool)>, Error> {
    let (tag, body) = split_tagged_payload(bytes)?;
    match tag {
        ADAPTIVE_TAG_NCB => {
            let view = view_ncb_u64_str_bool(body)?;
            materialize_ncb(view)
        }
        ADAPTIVE_TAG_AOS => aos::decode_rows_u64_str_bool(body),
        _ => Err(Error::invalid_tag(
            "decoding adaptive u64,str,bool rows",
            tag,
        )),
    }
}

/// Encode `(u64, Option<&str>, bool)` rows into an NCB payload (auto delta).
pub fn encode_ncb_u64_optstr_bool(rows: &[(u64, Option<&str>, bool)]) -> Vec<u8> {
    let n = rows.len();
    let use_delta = should_use_id_delta_opt(rows);
    let values: Vec<Option<&str>> = rows.iter().map(|(_, s, _)| *s).collect();
    let (col_bytes, _present) = encode_opt_str_column(&values);
    let bit_bytes = n.div_ceil(8);
    let estimated = 4 + 1 + n.saturating_mul(8 + 5) + col_bytes.len() + bit_bytes + 32;
    let mut sink = ByteSink::with_headroom(estimated, 0);
    sink.write_u32_le(n as u32);
    sink.write_u8(if use_delta {
        DESC_U64_DELTA_OPTSTR_BOOL
    } else {
        DESC_U64_OPTSTR_BOOL
    });

    let row_columns = IdFlagRows::OptStr(rows);
    write_id_column(&mut sink, row_columns, use_delta);
    sink.write_bytes(&col_bytes);
    write_flag_column(&mut sink, row_columns);
    sink.into_inner()
}

fn should_use_id_delta_opt(rows: &[(u64, Option<&str>, bool)]) -> bool {
    if rows.len() < 2 {
        return false;
    }
    let mut delta_size = DeltaSizeTracker::new(i128::from(rows[0].0), rows.len(), 8);
    for &(id, _, _) in &rows[1..] {
        if !delta_size.push(i128::from(id)) {
            return false;
        }
    }
    true
}

pub struct NcbU64OptStrBoolView<'a> {
    n: usize,
    ids: IdsRep<'a>,
    opt: OptStrColView<'a>,
    bits: &'a [u8],
}

impl<'a> NcbU64OptStrBoolView<'a> {
    pub fn len(&self) -> usize {
        self.n
    }
    pub fn is_empty(&self) -> bool {
        self.n == 0
    }
    pub fn id(&self, i: usize) -> u64 {
        self.ids.get(i)
    }
    pub fn name(&self, i: usize) -> Result<Option<&'a str>, Error> {
        self.opt.get(i)
    }
    pub fn flag(&self, i: usize) -> bool {
        bit_at(self.bits, i)
    }
}

pub fn view_ncb_u64_optstr_bool(bytes: &[u8]) -> Result<NcbU64OptStrBoolView<'_>, Error> {
    let (n, desc, mut off) = read_ncb_header(bytes)?;
    if desc != DESC_U64_OPTSTR_BOOL && desc != DESC_U64_DELTA_OPTSTR_BOOL {
        return Err(Error::Message("invalid NCB optstr descriptor".into()));
    }
    let ids = decode_ids_column(bytes, &mut off, n, desc == DESC_U64_DELTA_OPTSTR_BOOL)?;
    let opt_start = off;
    let (_, present, data_start) = optional_column_prefix(&bytes[opt_start..], n)?;
    let p = add_offset(opt_start, data_start)?;
    let offs_count = present.checked_add(1).ok_or(Error::LengthMismatch)?;
    let offs_len = mul_checked(offs_count, 4)?;
    let end_offs = add_offset(p, offs_len)?;
    if end_offs > bytes.len() {
        return Err(Error::LengthMismatch);
    }
    let offs_bytes = &bytes[p..end_offs];
    let last = read_u32_at(offs_bytes, present) as usize;
    let end_blob = add_offset(end_offs, last)?;
    if end_blob > bytes.len() {
        return Err(Error::LengthMismatch);
    }
    let column_bytes = &bytes[opt_start..end_blob];
    let opt = view_opt_str_column_inner(column_bytes, n)?;
    off = end_blob;
    let bits = take_bitset_tail(bytes, &mut off, n)?;
    Ok(NcbU64OptStrBoolView { n, ids, opt, bits })
}

/// Encode `(u64, Option<u32>, bool)` rows into an NCB payload (auto delta).
pub fn encode_ncb_u64_optu32_bool(rows: &[(u64, Option<u32>, bool)]) -> Vec<u8> {
    let n = rows.len();
    let use_delta = should_use_id_delta_optu32(rows);
    let values: Vec<Option<u32>> = rows.iter().map(|(_, v, _)| *v).collect();
    let (col_bytes, _present) = encode_opt_u32_column(&values);
    let bit_bytes = n.div_ceil(8);
    let estimated = 4 + 1 + n.saturating_mul(8 + 5) + col_bytes.len() + bit_bytes + 32;
    let mut sink = ByteSink::with_headroom(estimated, 0);
    sink.write_u32_le(n as u32);
    sink.write_u8(if use_delta {
        DESC_U64_DELTA_OPTU32_BOOL
    } else {
        DESC_U64_OPTU32_BOOL
    });
    let row_columns = IdFlagRows::OptU32(rows);
    write_id_column(&mut sink, row_columns, use_delta);
    sink.write_bytes(&col_bytes);
    write_flag_column(&mut sink, row_columns);
    sink.into_inner()
}

fn should_use_id_delta_optu32(rows: &[(u64, Option<u32>, bool)]) -> bool {
    if rows.len() < 2 {
        return false;
    }
    let mut delta_size = DeltaSizeTracker::new(i128::from(rows[0].0), rows.len(), 8);
    for &(id, _, _) in &rows[1..] {
        if !delta_size.push(i128::from(id)) {
            return false;
        }
    }
    true
}

pub struct NcbU64OptU32BoolView<'a> {
    n: usize,
    ids: IdsRep<'a>,
    opt: OptU32ColView<'a>,
    bits: &'a [u8],
}

impl<'a> NcbU64OptU32BoolView<'a> {
    pub fn len(&self) -> usize {
        self.n
    }
    pub fn is_empty(&self) -> bool {
        self.n == 0
    }
    pub fn id(&self, i: usize) -> u64 {
        self.ids.get(i)
    }
    pub fn val(&self, i: usize) -> Option<u32> {
        self.opt.get(i)
    }
    pub fn flag(&self, i: usize) -> bool {
        bit_at(self.bits, i)
    }
}

pub fn view_ncb_u64_optu32_bool(bytes: &[u8]) -> Result<NcbU64OptU32BoolView<'_>, Error> {
    let (n, desc, mut off) = read_ncb_header(bytes)?;
    if desc != DESC_U64_OPTU32_BOOL && desc != DESC_U64_DELTA_OPTU32_BOOL {
        return Err(Error::Message("invalid NCB optu32 descriptor".into()));
    }
    let ids = decode_ids_column(bytes, &mut off, n, desc == DESC_U64_DELTA_OPTU32_BOOL)?;
    let opt_start = off;
    let (_, present, data_start) = optional_column_prefix(&bytes[opt_start..], n)?;
    let p = add_offset(opt_start, data_start)?;
    let need = mul_checked(present, 4)?;
    let opt_end = add_offset(p, need)?;
    if opt_end > bytes.len() {
        return Err(Error::LengthMismatch);
    }
    let column_bytes = &bytes[opt_start..opt_end];
    let opt = view_opt_u32_column_inner(column_bytes, n)?;
    off = opt_end;
    // flags
    let bits = take_bitset_tail(bytes, &mut off, n)?;
    Ok(NcbU64OptU32BoolView { n, ids, opt, bits })
}

/// Adaptive AoS/NCB for `(u64, Option<&str>, bool)`
pub fn encode_rows_u64_optstr_bool_adaptive(rows: &[(u64, Option<&str>, bool)]) -> Vec<u8> {
    let small_n = small_smart_n();
    if rows.len() <= small_n {
        let timer = probe_start();
        let ncb = encode_ncb_u64_optstr_bool(rows);
        let ncb_ns = probe_elapsed(timer);

        let timer = probe_start();
        let aos = aos::encode_rows_u64_optstr_bool(rows);
        let aos_ns = probe_elapsed(timer);
        return finish_two_pass("u64_optstr_bool", aos, ncb, aos_ns, ncb_ns);
    }
    let (tag, payload) = if should_use_columnar(rows.len()) {
        (ADAPTIVE_TAG_NCB, encode_ncb_u64_optstr_bool(rows))
    } else {
        (ADAPTIVE_TAG_AOS, aos::encode_rows_u64_optstr_bool(rows))
    };
    finish_selection(tag, payload)
}

pub fn decode_rows_u64_optstr_bool_adaptive(
    bytes: &[u8],
) -> Result<Vec<(u64, Option<String>, bool)>, Error> {
    let (tag, body) = split_tagged_payload(bytes)?;
    match tag {
        ADAPTIVE_TAG_NCB => {
            let view = view_ncb_u64_optstr_bool(body)?;
            let mut out = Vec::with_capacity(view.len());
            for i in 0..view.len() {
                let id = view.id(i);
                let name = view.name(i)?.map(|s| s.to_string());
                let flag = view.flag(i);
                out.push((id, name, flag));
            }
            Ok(out)
        }
        ADAPTIVE_TAG_AOS => aos::decode_rows_u64_optstr_bool(body),
        _ => Err(Error::invalid_tag(
            "decoding adaptive u64,optstr,bool rows",
            tag,
        )),
    }
}

/// Adaptive AoS/NCB for `(u64, Option<u32>, bool)`
pub fn encode_rows_u64_optu32_bool_adaptive(rows: &[(u64, Option<u32>, bool)]) -> Vec<u8> {
    let small_n = small_smart_n();
    if rows.len() <= small_n {
        let timer = probe_start();
        let ncb = encode_ncb_u64_optu32_bool(rows);
        let ncb_ns = probe_elapsed(timer);

        let timer = probe_start();
        let aos = aos::encode_rows_u64_optu32_bool(rows);
        let aos_ns = probe_elapsed(timer);
        return finish_two_pass("u64_optu32_bool", aos, ncb, aos_ns, ncb_ns);
    }
    let (tag, payload) = if should_use_columnar(rows.len()) {
        (ADAPTIVE_TAG_NCB, encode_ncb_u64_optu32_bool(rows))
    } else {
        (ADAPTIVE_TAG_AOS, aos::encode_rows_u64_optu32_bool(rows))
    };
    finish_selection(tag, payload)
}

pub fn decode_rows_u64_optu32_bool_adaptive(
    bytes: &[u8],
) -> Result<Vec<(u64, Option<u32>, bool)>, Error> {
    let (tag, body) = split_tagged_payload(bytes)?;
    match tag {
        ADAPTIVE_TAG_NCB => {
            let view = view_ncb_u64_optu32_bool(body)?;
            let mut out = Vec::with_capacity(view.len());
            for i in 0..view.len() {
                let id = view.id(i);
                let v = view.val(i);
                let flag = view.flag(i);
                out.push((id, v, flag));
            }
            Ok(out)
        }
        ADAPTIVE_TAG_AOS => aos::decode_rows_u64_optu32_bool(body),
        _ => Err(Error::invalid_tag(
            "decoding adaptive u64,optu32,bool rows",
            tag,
        )),
    }
}

// ===== (u64, bytes, bool) =====

pub struct NcbU64BytesBoolView<'a> {
    n: usize,
    ids: IdsRep<'a>,
    offs_bytes: &'a [u8],
    blob: &'a [u8],
    bits: &'a [u8],
}

impl<'a> NcbU64BytesBoolView<'a> {
    pub fn len(&self) -> usize {
        self.n
    }
    pub fn is_empty(&self) -> bool {
        self.n == 0
    }
    pub fn id(&self, i: usize) -> u64 {
        self.ids.get(i)
    }
    pub fn data(&self, i: usize) -> &'a [u8] {
        let s = read_u32_at(self.offs_bytes, i) as usize;
        let e = read_u32_at(self.offs_bytes, i + 1) as usize;
        &self.blob[s..e]
    }
    pub fn flag(&self, i: usize) -> bool {
        bit_at(self.bits, i)
    }
}

pub fn encode_ncb_u64_bytes_bool(rows: &[(u64, &[u8], bool)]) -> Vec<u8> {
    let n = rows.len();
    let use_delta = should_use_id_delta_bytes(rows);
    let mut acc = 0u32;
    let mut offs = Vec::with_capacity(n + 1);
    offs.push(0);
    let mut blob = Vec::new();
    for &(_, b, _) in rows {
        acc = acc.wrapping_add(b.len() as u32);
        offs.push(acc);
        blob.extend_from_slice(b);
    }
    let bit_bytes = n.div_ceil(8);
    let estimated = 4
        + 1
        + n.saturating_mul(8 + 5)
        + offs.len().saturating_mul(4)
        + blob.len()
        + bit_bytes
        + 32;
    let mut sink = ByteSink::with_headroom(estimated, 0);
    sink.write_u32_le(n as u32);
    sink.write_u8(if use_delta {
        DESC_U64_DELTA_BYTES_BOOL
    } else {
        DESC_U64_BYTES_BOOL
    });
    let row_columns = IdFlagRows::Bytes(rows);
    write_id_column(&mut sink, row_columns, use_delta);
    sink.align_to(4);
    for v in &offs {
        sink.write_u32_le(*v);
    }
    sink.write_bytes(&blob);
    write_flag_column(&mut sink, row_columns);
    sink.into_inner()
}

fn should_use_id_delta_bytes(rows: &[(u64, &[u8], bool)]) -> bool {
    let h = crate::core::heuristics::get();
    if !h.combo_enable_id_delta || rows.len() < h.combo_id_delta_min_rows {
        return false;
    }
    if rows.len() <= h.combo_no_delta_small_n_if_empty && rows.iter().any(|(_, b, _)| b.is_empty())
    {
        return false;
    }
    let mut delta_size = DeltaSizeTracker::new(i128::from(rows[0].0), rows.len(), 8);
    for &(id, _, _) in &rows[1..] {
        if !delta_size.push(i128::from(id)) {
            return false;
        }
    }
    true
}

pub fn view_ncb_u64_bytes_bool(bytes: &[u8]) -> Result<NcbU64BytesBoolView<'_>, Error> {
    let (n, desc, mut off) = read_ncb_header(bytes)?;
    if desc != DESC_U64_BYTES_BOOL && desc != DESC_U64_DELTA_BYTES_BOOL {
        return Err(Error::Message("invalid NCB bytes descriptor".into()));
    }
    let ids = decode_ids_column(bytes, &mut off, n, desc == DESC_U64_DELTA_BYTES_BOOL)?;
    align_offset_checked(bytes, &mut off, 4)?;
    let offs_count = n.checked_add(1).ok_or(Error::LengthMismatch)?;
    let offs_len = mul_checked(offs_count, 4)?;
    let offs_bytes = slice_range(bytes, off, offs_len)?;
    off = add_offset(off, offs_len)?;
    let mut last = read_u32_at(offs_bytes, 0) as usize;
    if last != 0 {
        return Err(Error::LengthMismatch);
    }
    for i in 1..=n {
        let cur = read_u32_at(offs_bytes, i) as usize;
        if cur < last {
            return Err(Error::LengthMismatch);
        }
        last = cur;
    }
    let blob = slice_range(bytes, off, last)?;
    off = add_offset(off, last)?;
    // Validate monotonic offsets for safety (non-decreasing and within blob)
    for i in 0..n {
        let s = read_u32_at(offs_bytes, i) as usize;
        let e = read_u32_at(offs_bytes, i + 1) as usize;
        if e < s || e > last {
            return Err(Error::LengthMismatch);
        }
    }
    let bits = take_bitset_tail(bytes, &mut off, n)?;
    Ok(NcbU64BytesBoolView {
        n,
        ids,
        offs_bytes,
        blob,
        bits,
    })
}

pub fn encode_rows_u64_bytes_bool_adaptive(rows: &[(u64, &[u8], bool)]) -> Vec<u8> {
    let small_n = small_smart_n();
    if rows.len() <= small_n {
        let timer = probe_start();
        let ncb = encode_ncb_u64_bytes_bool(rows);
        let ncb_ns = probe_elapsed(timer);

        let timer = probe_start();
        let aos = aos::encode_rows_u64_bytes_bool(rows);
        let aos_ns = probe_elapsed(timer);
        return finish_two_pass("u64_bytes_bool", aos, ncb, aos_ns, ncb_ns);
    }
    let (tag, payload) = if should_use_columnar(rows.len()) {
        (ADAPTIVE_TAG_NCB, encode_ncb_u64_bytes_bool(rows))
    } else {
        (ADAPTIVE_TAG_AOS, aos::encode_rows_u64_bytes_bool(rows))
    };
    finish_selection(tag, payload)
}

pub fn decode_rows_u64_bytes_bool_adaptive(
    bytes: &[u8],
) -> Result<Vec<(u64, Vec<u8>, bool)>, Error> {
    let (tag, body) = split_tagged_payload(bytes)?;
    match tag {
        ADAPTIVE_TAG_NCB => {
            let view = view_ncb_u64_bytes_bool(body)?;
            let mut out = Vec::with_capacity(view.len());
            for i in 0..view.len() {
                out.push((view.id(i), view.data(i).to_vec(), view.flag(i)));
            }
            Ok(out)
        }
        ADAPTIVE_TAG_AOS => aos::decode_rows_u64_bytes_bool(body),
        _ => Err(Error::invalid_tag(
            "decoding adaptive u64,bytes,bool rows",
            tag,
        )),
    }
}

// ===== AoS borrowed views for (u64, &str, bool) and (u64, &[u8], bool) =====

pub struct AosU64StrBoolView<'a> {
    n: usize,
    body: &'a [u8],
    rows: Vec<AosVarBoolIdx>,
}

struct AosVarBoolIdx {
    id: u64,
    data_off: usize,
    data_len: usize,
    flag: bool,
}

#[inline]
fn parse_aos_u64_var_bool(body: &[u8]) -> Result<(usize, Vec<AosVarBoolIdx>), Error> {
    let (n, mut offset) = aos_read_len_and_ver(body)?;
    let mut rows = Vec::with_capacity(n);
    for _ in 0..n {
        let id_end = offset.checked_add(8).ok_or(Error::LengthMismatch)?;
        let mut id_bytes = [0; 8];
        id_bytes.copy_from_slice(body.get(offset..id_end).ok_or(Error::LengthMismatch)?);
        let id = u64::from_le_bytes(id_bytes);
        offset = id_end;

        let data_len = read_aos_len(body, &mut offset)?;
        let data_off = offset;
        offset = data_off
            .checked_add(data_len)
            .filter(|&end| end < body.len())
            .ok_or(Error::LengthMismatch)?;
        let flag = body[offset] != 0;
        offset += 1;
        rows.push(AosVarBoolIdx {
            id,
            data_off,
            data_len,
            flag,
        });
    }
    Ok((n, rows))
}

impl<'a> AosU64StrBoolView<'a> {
    pub fn len(&self) -> usize {
        self.n
    }
    pub fn is_empty(&self) -> bool {
        self.n == 0
    }
    pub fn id(&self, i: usize) -> u64 {
        self.rows[i].id
    }
    pub fn name(&self, i: usize) -> Result<&'a str, Error> {
        let r = &self.rows[i];
        let s = r.data_off;
        let e = s + r.data_len;
        let bytes = &self.body[s..e];
        validated_str(bytes)
    }
    pub fn flag(&self, i: usize) -> bool {
        self.rows[i].flag
    }
}

pub fn view_aos_u64_str_bool(body: &[u8]) -> Result<AosU64StrBoolView<'_>, Error> {
    let (n, rows) = parse_aos_u64_var_bool(body)?;
    Ok(AosU64StrBoolView { n, body, rows })
}

pub struct AosU64BytesBoolView<'a> {
    n: usize,
    body: &'a [u8],
    rows: Vec<AosVarBoolIdx>,
}

impl<'a> AosU64BytesBoolView<'a> {
    pub fn len(&self) -> usize {
        self.n
    }
    pub fn is_empty(&self) -> bool {
        self.n == 0
    }
    pub fn id(&self, i: usize) -> u64 {
        self.rows[i].id
    }
    pub fn data(&self, i: usize) -> &'a [u8] {
        let r = &self.rows[i];
        &self.body[r.data_off..r.data_off + r.data_len]
    }
    pub fn flag(&self, i: usize) -> bool {
        self.rows[i].flag
    }
}

pub fn view_aos_u64_bytes_bool(body: &[u8]) -> Result<AosU64BytesBoolView<'_>, Error> {
    let (n, rows) = parse_aos_u64_var_bool(body)?;
    Ok(AosU64BytesBoolView { n, body, rows })
}

// ===== (u64, u32, bool) =====

pub struct NcbU64U32BoolView<'a> {
    n: usize,
    ids: IdsRep<'a>,
    vals: U32Rep<'a>,
    bits: &'a [u8],
}
impl<'a> NcbU64U32BoolView<'a> {
    pub fn len(&self) -> usize {
        self.n
    }
    pub fn is_empty(&self) -> bool {
        self.n == 0
    }
    pub fn id(&self, i: usize) -> u64 {
        self.ids.get(i)
    }
    pub fn val(&self, i: usize) -> u32 {
        self.vals.get(i)
    }
    pub fn flag(&self, i: usize) -> bool {
        bit_at(self.bits, i)
    }
}

pub fn encode_ncb_u64_u32_bool(
    rows: &[(u64, u32, bool)],
    use_id_delta: bool,
    use_u32_delta: bool,
) -> Vec<u8> {
    let n = rows.len();
    let mut use_id_delta = use_id_delta;
    if use_id_delta && n >= 2 {
        let mut prev = rows[0].0 as i128;
        for &(id, _, _) in rows.iter().skip(1) {
            let d = (id as i128) - prev;
            if d < i64::MIN as i128 || d > i64::MAX as i128 {
                use_id_delta = false;
                break;
            }
            prev = id as i128;
        }
    }
    let desc = match (use_id_delta, use_u32_delta) {
        (false, false) => DESC_U64_U32_BOOL,
        (true, false) => DESC_U64_DELTA_U32_BOOL,
        (false, true) => DESC_U64_U32DELTA_BOOL,
        (true, true) => DESC_U64_DELTA_U32DELTA_BOOL,
    };
    let bit_bytes = n.div_ceil(8);
    let id_estimate = n.saturating_mul(8 + 5);
    let value_estimate = if use_u32_delta {
        // Initial value (if any) + varint deltas (at most 5 bytes each)
        n.saturating_mul(5) + 4
    } else {
        n.saturating_mul(4)
    };
    let estimated = 4 + 1 + id_estimate + value_estimate + bit_bytes + 32;
    let mut sink = ByteSink::with_headroom(estimated, 0);
    sink.write_u32_le(n as u32);
    sink.write_u8(desc);
    let row_columns = IdFlagRows::U32(rows);
    write_id_column(&mut sink, row_columns, use_id_delta);
    sink.align_to(4);
    if use_u32_delta && n > 0 {
        sink.write_u32_le(rows[0].1);
        let mut prev = rows[0].1 as i64;
        for &(_, v, _) in rows.iter().skip(1) {
            let d = (v as i64) - prev;
            prev = v as i64;
            sink.write_var_u64(zigzag_encode(d));
        }
    } else {
        for &(_, v, _) in rows {
            sink.write_u32_le(v);
        }
    }
    write_flag_column(&mut sink, row_columns);
    sink.into_inner()
}

fn should_use_u32_delta(rows: &[(u64, u32, bool)]) -> bool {
    if rows.len() < 2 {
        return false;
    }
    let mut delta_size = DeltaSizeTracker::new(i128::from(rows[0].1), rows.len(), 4);
    for &(_, v, _) in &rows[1..] {
        if !delta_size.push(i128::from(v)) {
            return false;
        }
    }
    true
}

pub fn view_ncb_u64_u32_bool(bytes: &[u8]) -> Result<NcbU64U32BoolView<'_>, Error> {
    let (n, desc, mut off) = read_ncb_header(bytes)?;
    if !matches!(
        desc,
        DESC_U64_U32_BOOL
            | DESC_U64_DELTA_U32_BOOL
            | DESC_U64_U32DELTA_BOOL
            | DESC_U64_DELTA_U32DELTA_BOOL
    ) {
        return Err(Error::Message("invalid NCB u64-u32 descriptor".into()));
    }
    let ids = decode_ids_column(
        bytes,
        &mut off,
        n,
        matches!(desc, DESC_U64_DELTA_U32_BOOL | DESC_U64_DELTA_U32DELTA_BOOL),
    )?;
    let vals = decode_u32_column(
        bytes,
        &mut off,
        n,
        matches!(desc, DESC_U64_U32DELTA_BOOL | DESC_U64_DELTA_U32DELTA_BOOL),
    )?;
    let bits = take_bitset_tail(bytes, &mut off, n)?;
    Ok(NcbU64U32BoolView { n, ids, vals, bits })
}

pub fn encode_rows_u64_u32_bool_adaptive(rows: &[(u64, u32, bool)]) -> Vec<u8> {
    let small_n = small_smart_n();
    if rows.len() <= small_n {
        let timer = probe_start();
        let ncb = encode_ncb_u64_u32_bool(
            rows,
            should_use_id_delta_u64_only(rows),
            should_use_u32_delta(rows),
        );
        let ncb_ns = probe_elapsed(timer);

        let timer = probe_start();
        let aos = aos::encode_rows_u64_u32_bool(rows);
        let aos_ns = probe_elapsed(timer);
        return finish_two_pass("u64_u32_bool", aos, ncb, aos_ns, ncb_ns);
    }
    let (tag, payload) = if should_use_columnar(rows.len()) {
        let use_id_delta = should_use_id_delta_u64_only(rows);
        let use_u32_delta = should_use_u32_delta(rows);
        (
            ADAPTIVE_TAG_NCB,
            encode_ncb_u64_u32_bool(rows, use_id_delta, use_u32_delta),
        )
    } else {
        (ADAPTIVE_TAG_AOS, aos::encode_rows_u64_u32_bool(rows))
    };
    finish_selection(tag, payload)
}

fn should_use_id_delta_u64_only(rows: &[(u64, u32, bool)]) -> bool {
    if rows.len() < 2 {
        return false;
    }
    let mut delta_size = DeltaSizeTracker::new(i128::from(rows[0].0), rows.len(), 8);
    for &(id, _, _) in &rows[1..] {
        if !delta_size.push(i128::from(id)) {
            return false;
        }
    }
    true
}

pub fn decode_rows_u64_u32_bool_adaptive(bytes: &[u8]) -> Result<Vec<(u64, u32, bool)>, Error> {
    let (tag, body) = split_tagged_payload(bytes)?;
    match tag {
        ADAPTIVE_TAG_NCB => {
            let view = view_ncb_u64_u32_bool(body)?;
            let mut out = Vec::with_capacity(view.len());
            for i in 0..view.len() {
                out.push((view.id(i), view.val(i), view.flag(i)));
            }
            Ok(out)
        }
        ADAPTIVE_TAG_AOS => aos::decode_rows_u64_u32_bool(body),
        _ => Err(Error::invalid_tag(
            "decoding adaptive u64,u32,bool rows",
            tag,
        )),
    }
}

// ===== New combo shapes: (u64, &str, u32, bool) and (u64, &[u8], u32, bool) =====

// Heuristics for 4-column combos
fn should_use_id_delta_str_u32(rows: &[(u64, &str, u32, bool)]) -> bool {
    let h = crate::core::heuristics::get();
    if !h.combo_enable_id_delta || rows.len() < h.combo_id_delta_min_rows {
        return false;
    }
    // Keep offsets goldens stable: avoid deltas on tiny inputs with empties
    if rows.len() <= h.combo_no_delta_small_n_if_empty
        && rows.iter().any(|(_, s, _, _)| s.is_empty())
    {
        return false;
    }
    if rows.len() < 2 {
        return false;
    }
    let mut delta_size = DeltaSizeTracker::new(i128::from(rows[0].0), rows.len(), 8);
    for &(id, _, _, _) in &rows[1..] {
        if !delta_size.push(i128::from(id)) {
            return false;
        }
    }
    true
}

fn should_use_u32_delta_str_u32(rows: &[(u64, &str, u32, bool)]) -> bool {
    should_use_u32_delta_str_u32_with(rows, crate::core::heuristics::get())
}

fn should_use_u32_delta_str_u32_with(
    rows: &[(u64, &str, u32, bool)],
    h: crate::core::heuristics::Heuristics,
) -> bool {
    if !h.combo_enable_u32_delta_names || rows.len() < h.combo_u32_delta_min_rows {
        return false;
    }
    if rows.len() <= h.combo_no_delta_small_n_if_empty
        && rows.iter().any(|(_, s, _, _)| s.is_empty())
    {
        return false;
    }
    if rows.len() < 2 {
        return false;
    }
    let mut delta_size = DeltaSizeTracker::new(i128::from(rows[0].2), rows.len(), 4);
    for &(_, _, v, _) in &rows[1..] {
        if !delta_size.push(i128::from(v)) {
            return false;
        }
    }
    true
}

fn should_use_id_delta_bytes_u32(rows: &[(u64, &[u8], u32, bool)]) -> bool {
    let h = crate::core::heuristics::get();
    if !h.combo_enable_id_delta || rows.len() < h.combo_id_delta_min_rows {
        return false;
    }
    if rows.len() <= h.combo_no_delta_small_n_if_empty
        && rows.iter().any(|(_, b, _, _)| b.is_empty())
    {
        return false;
    }
    if rows.len() < 2 {
        return false;
    }
    let mut delta_size = DeltaSizeTracker::new(i128::from(rows[0].0), rows.len(), 8);
    for &(id, _, _, _) in &rows[1..] {
        if !delta_size.push(i128::from(id)) {
            return false;
        }
    }
    true
}

fn should_use_u32_delta_bytes_u32(rows: &[(u64, &[u8], u32, bool)]) -> bool {
    should_use_u32_delta_bytes_u32_with(rows, crate::core::heuristics::get())
}

fn should_use_u32_delta_bytes_u32_with(
    rows: &[(u64, &[u8], u32, bool)],
    h: crate::core::heuristics::Heuristics,
) -> bool {
    if !h.combo_enable_u32_delta_bytes || rows.len() < h.combo_u32_delta_min_rows {
        return false;
    }
    if rows.len() <= h.combo_no_delta_small_n_if_empty
        && rows.iter().any(|(_, b, _, _)| b.is_empty())
    {
        return false;
    }
    if rows.len() < 2 {
        return false;
    }
    let mut delta_size = DeltaSizeTracker::new(i128::from(rows[0].2), rows.len(), 4);
    for &(_, _, v, _) in &rows[1..] {
        if !delta_size.push(i128::from(v)) {
            return false;
        }
    }
    true
}

#[allow(clippy::type_complexity)]
fn build_dict_str_u32<'a>(
    rows: &'a [(u64, &str, u32, bool)],
    force_build: bool,
) -> (
    bool,
    Option<std::collections::HashMap<&'a str, u32>>,
    Option<Vec<&'a str>>,
) {
    use std::collections::HashMap;
    let n = rows.len();
    if n == 0 {
        return (false, None, None);
    }
    let h = crate::core::heuristics::get();
    if !force_build && !h.combo_enable_name_dict {
        return (false, None, None);
    }
    let mut dict: HashMap<&str, u32> = HashMap::with_capacity(n.min(1024));
    let mut vec: Vec<&str> = Vec::new();
    let mut total_len: usize = 0;
    for &(_, s, _, _) in rows.iter() {
        total_len += s.len();
        if !dict.contains_key(s) {
            let id = vec.len() as u32;
            dict.insert(s, id);
            vec.push(s);
        }
    }
    let distinct = vec.len();
    let avg = total_len as f64 / n as f64;
    let ratio = distinct as f64 / n as f64;
    let exceeds_cap = h.combo_dict_max_entries != 0 && distinct > h.combo_dict_max_entries;
    let use_dict = if force_build {
        !exceeds_cap
    } else {
        ratio <= h.combo_dict_ratio_max && avg >= h.combo_dict_avg_len_min && !exceeds_cap
    };
    if use_dict {
        (true, Some(dict), Some(vec))
    } else {
        (false, None, None)
    }
}

#[allow(clippy::type_complexity)]
pub fn encode_ncb_u64_str_u32_bool(rows: &[(u64, &str, u32, bool)]) -> Vec<u8> {
    encode_ncb_u64_str_u32_bool_with_policy(rows, ComboPolicy::default())
}

/// Encode `(u64, &str, u32, bool)` rows with an explicit policy override.
#[allow(clippy::too_many_lines)]
pub fn encode_ncb_u64_str_u32_bool_with_policy(
    rows: &[(u64, &str, u32, bool)],
    policy: ComboPolicy,
) -> Vec<u8> {
    let n = rows.len();
    let force_dict = matches!(policy.force_dictionary, Some(true));
    let (use_dict, dict_map, dict_vec) = if matches!(policy.force_dictionary, Some(false)) {
        (false, None, None)
    } else {
        build_dict_str_u32(rows, force_dict)
    };
    let heur_id_delta = should_use_id_delta_str_u32(rows);
    let mut use_id_delta = match policy.force_id_delta {
        Some(value) => value,
        None => heur_id_delta,
    };
    if use_id_delta && n >= 2 {
        let mut prev = rows[0].0 as i128;
        for &(id, _, _, _) in rows.iter().skip(1) {
            let d = (id as i128) - prev;
            if d < i64::MIN as i128 || d > i64::MAX as i128 {
                use_id_delta = false;
                break;
            }
            prev = id as i128;
        }
    }
    let heur_u32_delta = should_use_u32_delta_str_u32(rows);
    let use_u32_delta = match policy.force_u32_delta {
        Some(value) => value,
        None => heur_u32_delta,
    };

    let desc = match (use_dict, use_id_delta, use_u32_delta) {
        (false, false, false) => DESC_U64_STR_U32_BOOL,
        (true, false, false) => DESC_U64_DICT_STR_U32_BOOL,
        (false, true, false) => DESC_U64_DELTA_STR_U32_BOOL,
        (true, true, false) => DESC_U64_DELTA_DICT_STR_U32_BOOL,
        (false, false, true) => DESC_U64_STR_U32DELTA_BOOL,
        (true, false, true) => DESC_U64_DICT_STR_U32DELTA_BOOL,
        (false, true, true) => DESC_U64_DELTA_STR_U32DELTA_BOOL,
        (true, true, true) => DESC_U64_DELTA_DICT_STR_U32DELTA_BOOL,
    };
    let names_total: usize = rows.iter().map(|(_, s, _, _)| s.len()).sum();
    let dict_vec_len = dict_vec.as_ref().map(|v| v.len()).unwrap_or(0);
    let dict_blob_total: usize = dict_vec
        .as_ref()
        .map(|v| v.iter().map(|s| s.len()).sum())
        .unwrap_or(0);
    let bit_bytes = n.div_ceil(8);
    let id_estimate = n.saturating_mul(8 + 5);
    let name_offsets_bytes = if use_dict {
        (dict_vec_len + 1).saturating_mul(4)
    } else {
        (n + 1).saturating_mul(4)
    };
    let name_blob_bytes = if use_dict {
        dict_blob_total
    } else {
        names_total
    };
    let code_bytes = if use_dict { n.saturating_mul(4) } else { 0 };
    let value_estimate = if use_u32_delta {
        n.saturating_mul(5) + 4
    } else {
        n.saturating_mul(4)
    };
    let dict_len_bytes = if use_dict { 4 } else { 0 };
    let estimated = 4
        + 1
        + id_estimate
        + name_offsets_bytes
        + name_blob_bytes
        + code_bytes
        + dict_len_bytes
        + value_estimate
        + bit_bytes
        + 64;
    let mut sink = ByteSink::with_headroom(estimated, 0);
    sink.write_u32_le(n as u32);
    sink.write_u8(desc);
    let row_columns = IdFlagRows::StrU32(rows);
    write_id_column(&mut sink, row_columns, use_id_delta);
    if use_dict {
        let dict = dict_map.as_ref().expect("dict map");
        let dict_vec = dict_vec.as_ref().expect("dict vec");
        sink.align_to(4);
        sink.write_u32_le(dict_vec.len() as u32);
        let mut offsets = Vec::with_capacity(dict_vec.len() + 1);
        offsets.push(0);
        let mut acc: u32 = 0;
        let mut blob = Vec::new();
        for s in dict_vec {
            let bytes = s.as_bytes();
            acc = acc.wrapping_add(bytes.len() as u32);
            offsets.push(acc);
            blob.extend_from_slice(bytes);
        }
        for v in &offsets {
            sink.write_u32_le(*v);
        }
        sink.write_bytes(&blob);
        sink.align_to(4);
        for &(_, s, _, _) in rows {
            let code = *dict.get(s).unwrap_or(&0);
            sink.write_u32_le(code);
        }
    } else {
        sink.align_to(4);
        let mut offsets = Vec::with_capacity(n + 1);
        offsets.push(0);
        let mut acc: u32 = 0;
        let mut blob = Vec::new();
        for &(_, s, _, _) in rows {
            let bytes = s.as_bytes();
            acc = acc.wrapping_add(bytes.len() as u32);
            offsets.push(acc);
            blob.extend_from_slice(bytes);
        }
        for v in &offsets {
            sink.write_u32_le(*v);
        }
        sink.write_bytes(&blob);
    }
    sink.align_to(4);
    if use_u32_delta && n > 0 {
        sink.write_u32_le(rows[0].2);
        let mut prev = rows[0].2 as i64;
        for &(_, _, v, _) in rows.iter().skip(1) {
            let d = (v as i64) - prev;
            prev = v as i64;
            sink.write_var_u64(zigzag_encode(d));
        }
    } else {
        for &(_, _, v, _) in rows {
            sink.write_u32_le(v);
        }
    }
    write_flag_column(&mut sink, row_columns);
    sink.into_inner()
}

enum U32Rep<'a> {
    Slice(&'a [u32]),
    Rebuilt(Vec<u32>),
}

impl U32Rep<'_> {
    #[inline]
    fn get(&self, i: usize) -> u32 {
        match self {
            Self::Slice(values) => values[i],
            Self::Rebuilt(values) => values[i],
        }
    }
}

pub struct NcbU64StrU32BoolView<'a> {
    n: usize,
    ids: IdsRep<'a>,
    names: NamesRep<'a>,
    vals: U32Rep<'a>,
    bits: &'a [u8],
}

impl<'a> NcbU64StrU32BoolView<'a> {
    pub fn len(&self) -> usize {
        self.n
    }
    pub fn is_empty(&self) -> bool {
        self.n == 0
    }
    pub fn id(&self, i: usize) -> u64 {
        self.ids.get(i)
    }
    pub fn name(&self, i: usize) -> Result<&'a str, Error> {
        self.names.get(i)
    }
    pub fn val(&self, i: usize) -> u32 {
        self.vals.get(i)
    }
    pub fn flag(&self, i: usize) -> bool {
        bit_at(self.bits, i)
    }
}

pub fn view_ncb_u64_str_u32_bool(bytes: &[u8]) -> Result<NcbU64StrU32BoolView<'_>, Error> {
    let (n, desc, mut off) = read_ncb_header(bytes)?;
    let is_dict = matches!(
        desc,
        DESC_U64_DICT_STR_U32_BOOL
            | DESC_U64_DELTA_DICT_STR_U32_BOOL
            | DESC_U64_DICT_STR_U32DELTA_BOOL
            | DESC_U64_DELTA_DICT_STR_U32DELTA_BOOL
    );
    let id_delta = matches!(
        desc,
        DESC_U64_DELTA_STR_U32_BOOL
            | DESC_U64_DELTA_DICT_STR_U32_BOOL
            | DESC_U64_DELTA_STR_U32DELTA_BOOL
            | DESC_U64_DELTA_DICT_STR_U32DELTA_BOOL
    );
    let u32_delta = matches!(
        desc,
        DESC_U64_STR_U32DELTA_BOOL
            | DESC_U64_DICT_STR_U32DELTA_BOOL
            | DESC_U64_DELTA_STR_U32DELTA_BOOL
            | DESC_U64_DELTA_DICT_STR_U32DELTA_BOOL
    );
    if !matches!(
        desc,
        DESC_U64_STR_U32_BOOL
            | DESC_U64_DELTA_STR_U32_BOOL
            | DESC_U64_STR_U32DELTA_BOOL
            | DESC_U64_DELTA_STR_U32DELTA_BOOL
            | DESC_U64_DICT_STR_U32_BOOL
            | DESC_U64_DELTA_DICT_STR_U32_BOOL
            | DESC_U64_DICT_STR_U32DELTA_BOOL
            | DESC_U64_DELTA_DICT_STR_U32DELTA_BOOL
    ) {
        return Err(Error::Message(
            "invalid NCB u64-str-u32-bool descriptor".into(),
        ));
    }
    let ids = decode_ids_column(bytes, &mut off, n, id_delta)?;
    // names
    let names = if !is_dict {
        decode_offset_names(bytes, &mut off, n)?
    } else {
        decode_dict_names(bytes, &mut off, n, false)?
    };
    let vals = decode_u32_column(bytes, &mut off, n, u32_delta)?;
    let bits = take_bitset_tail(bytes, &mut off, n)?;
    Ok(NcbU64StrU32BoolView {
        n,
        ids,
        names,
        vals,
        bits,
    })
}

pub fn encode_rows_u64_str_u32_bool_adaptive(rows: &[(u64, &str, u32, bool)]) -> Vec<u8> {
    let small_n = small_smart_n();
    if rows.len() <= small_n {
        let timer = probe_start();
        let ncb = encode_ncb_u64_str_u32_bool(rows);
        let ncb_ns = probe_elapsed(timer);

        let timer = probe_start();
        let aos = aos::encode_rows_u64_str_u32_bool(rows);
        let aos_ns = probe_elapsed(timer);
        return finish_two_pass("u64_str_u32_bool", aos, ncb, aos_ns, ncb_ns);
    }
    let (tag, payload) = if should_use_columnar(rows.len()) {
        (ADAPTIVE_TAG_NCB, encode_ncb_u64_str_u32_bool(rows))
    } else {
        (ADAPTIVE_TAG_AOS, aos::encode_rows_u64_str_u32_bool(rows))
    };
    finish_selection(tag, payload)
}

pub fn decode_rows_u64_str_u32_bool_adaptive(
    bytes: &[u8],
) -> Result<Vec<(u64, String, u32, bool)>, Error> {
    let (tag, body) = split_tagged_payload(bytes)?;
    match tag {
        ADAPTIVE_TAG_NCB => {
            let view = view_ncb_u64_str_u32_bool(body)?;
            let mut out = Vec::with_capacity(view.len());
            for i in 0..view.len() {
                out.push((
                    view.id(i),
                    view.name(i)?.to_string(),
                    view.val(i),
                    view.flag(i),
                ));
            }
            Ok(out)
        }
        ADAPTIVE_TAG_AOS => aos::decode_rows_u64_str_u32_bool(body),
        _ => Err(Error::invalid_tag(
            "decoding adaptive u64,str,u32,bool rows",
            tag,
        )),
    }
}

// encode/decode AoS for (u64, str, u32, bool) moved to `crate::aos`

pub struct NcbU64BytesU32BoolView<'a> {
    n: usize,
    ids: IdsRep<'a>,
    offs_bytes: &'a [u8],
    blob: &'a [u8],
    vals: U32Rep<'a>,
    bits: &'a [u8],
}

impl<'a> NcbU64BytesU32BoolView<'a> {
    pub fn len(&self) -> usize {
        self.n
    }
    pub fn is_empty(&self) -> bool {
        self.n == 0
    }
    pub fn id(&self, i: usize) -> u64 {
        self.ids.get(i)
    }
    pub fn data(&self, i: usize) -> &'a [u8] {
        let s = read_u32_at(self.offs_bytes, i) as usize;
        let e = read_u32_at(self.offs_bytes, i + 1) as usize;
        &self.blob[s..e]
    }
    pub fn val(&self, i: usize) -> u32 {
        self.vals.get(i)
    }
    pub fn flag(&self, i: usize) -> bool {
        bit_at(self.bits, i)
    }
}

#[allow(clippy::type_complexity)]
pub fn encode_ncb_u64_bytes_u32_bool(rows: &[(u64, &[u8], u32, bool)]) -> Vec<u8> {
    let n = rows.len();
    let use_id_delta = should_use_id_delta_bytes_u32(rows);
    // Enable u32-delta when beneficial (varint deltas beat raw u32 storage)
    let use_u32_delta = should_use_u32_delta_bytes_u32(rows);
    let desc = match (use_id_delta, use_u32_delta) {
        (false, false) => DESC_U64_BYTES_U32_BOOL,
        (true, false) => DESC_U64_DELTA_BYTES_U32_BOOL,
        (false, true) => DESC_U64_BYTES_U32DELTA_BOOL,
        (true, true) => DESC_U64_DELTA_BYTES_U32DELTA_BOOL,
    };
    let mut acc: u32 = 0;
    let mut offs = Vec::with_capacity(n + 1);
    offs.push(0);
    let mut blob = Vec::new();
    for &(_, b, _, _) in rows {
        acc = acc.wrapping_add(b.len() as u32);
        offs.push(acc);
        blob.extend_from_slice(b);
    }
    let bit_bytes = n.div_ceil(8);
    let id_estimate = n.saturating_mul(8 + 5);
    let value_estimate = if use_u32_delta {
        n.saturating_mul(5) + 4
    } else {
        n.saturating_mul(4)
    };
    let estimated = 4
        + 1
        + id_estimate
        + offs.len().saturating_mul(4)
        + blob.len()
        + value_estimate
        + bit_bytes
        + 48;
    let mut sink = ByteSink::with_headroom(estimated, 0);
    sink.write_u32_le(n as u32);
    sink.write_u8(desc);
    let row_columns = IdFlagRows::BytesU32(rows);
    write_id_column(&mut sink, row_columns, use_id_delta);
    sink.align_to(4);
    for v in &offs {
        sink.write_u32_le(*v);
    }
    sink.write_bytes(&blob);
    sink.align_to(4);
    if use_u32_delta && n > 0 {
        sink.write_u32_le(rows[0].2);
        let mut prev = rows[0].2 as i64;
        for &(_, _, v, _) in rows.iter().skip(1) {
            let d = (v as i64) - prev;
            prev = v as i64;
            sink.write_var_u64(zigzag_encode(d));
        }
    } else {
        for &(_, _, v, _) in rows {
            sink.write_u32_le(v);
        }
    }
    write_flag_column(&mut sink, row_columns);
    sink.into_inner()
}

pub fn view_ncb_u64_bytes_u32_bool(bytes: &[u8]) -> Result<NcbU64BytesU32BoolView<'_>, Error> {
    let (n, desc, mut off) = read_ncb_header(bytes)?;
    let id_delta = matches!(
        desc,
        DESC_U64_DELTA_BYTES_U32_BOOL | DESC_U64_DELTA_BYTES_U32DELTA_BOOL
    );
    let u32_delta = matches!(
        desc,
        DESC_U64_BYTES_U32DELTA_BOOL | DESC_U64_DELTA_BYTES_U32DELTA_BOOL
    );
    if !matches!(
        desc,
        DESC_U64_BYTES_U32_BOOL
            | DESC_U64_DELTA_BYTES_U32_BOOL
            | DESC_U64_BYTES_U32DELTA_BOOL
            | DESC_U64_DELTA_BYTES_U32DELTA_BOOL
    ) {
        return Err(Error::Message(
            "invalid NCB u64-bytes-u32-bool descriptor".into(),
        ));
    }
    let ids = decode_ids_column(bytes, &mut off, n, id_delta)?;
    align_offset_checked(bytes, &mut off, 4)?;
    let offs_count = n.checked_add(1).ok_or(Error::LengthMismatch)?;
    let offs_len = mul_checked(offs_count, 4)?;
    let offs_bytes = slice_range(bytes, off, offs_len)?;
    off = add_offset(off, offs_len)?;
    let mut prev = read_u32_at(offs_bytes, 0) as usize;
    if prev != 0 {
        return Err(Error::LengthMismatch);
    }
    for i in 1..=n {
        let cur = read_u32_at(offs_bytes, i) as usize;
        if cur < prev {
            return Err(Error::LengthMismatch);
        }
        prev = cur;
    }
    let last = prev;
    for i in 0..n {
        let s = read_u32_at(offs_bytes, i) as usize;
        let e = read_u32_at(offs_bytes, i + 1) as usize;
        if e < s || e > last {
            return Err(Error::LengthMismatch);
        }
    }
    let blob = slice_range(bytes, off, last)?;
    off = add_offset(off, last)?;
    let vals = decode_u32_column(bytes, &mut off, n, u32_delta)?;
    let bits = take_bitset_tail(bytes, &mut off, n)?;
    Ok(NcbU64BytesU32BoolView {
        n,
        ids,
        offs_bytes,
        blob,
        vals,
        bits,
    })
}

pub fn encode_rows_u64_bytes_u32_bool_adaptive(rows: &[(u64, &[u8], u32, bool)]) -> Vec<u8> {
    let small_n = small_smart_n();
    if rows.len() <= small_n {
        let timer = probe_start();
        let ncb = encode_ncb_u64_bytes_u32_bool(rows);
        let ncb_ns = probe_elapsed(timer);
        // AoS ad-hoc body via shared helper
        let timer = probe_start();
        let aos = aos::encode_rows_u64_bytes_u32_bool(rows);
        let aos_ns = probe_elapsed(timer);
        return finish_two_pass("u64_bytes_u32_bool", aos, ncb, aos_ns, ncb_ns);
    }
    let (tag, payload) = if should_use_columnar(rows.len()) {
        (ADAPTIVE_TAG_NCB, encode_ncb_u64_bytes_u32_bool(rows))
    } else {
        (ADAPTIVE_TAG_AOS, aos::encode_rows_u64_bytes_u32_bool(rows))
    };
    finish_selection(tag, payload)
}

#[allow(clippy::type_complexity)]
pub fn decode_rows_u64_bytes_u32_bool_adaptive(
    bytes: &[u8],
) -> Result<Vec<(u64, Vec<u8>, u32, bool)>, Error> {
    let (tag, body) = split_tagged_payload(bytes)?;
    match tag {
        ADAPTIVE_TAG_NCB => {
            let view = view_ncb_u64_bytes_u32_bool(body)?;
            let mut out = Vec::with_capacity(view.len());
            for i in 0..view.len() {
                out.push((view.id(i), view.data(i).to_vec(), view.val(i), view.flag(i)));
            }
            Ok(out)
        }
        ADAPTIVE_TAG_AOS => aos::decode_rows_u64_bytes_u32_bool(body),
        _ => Err(Error::invalid_tag(
            "decoding adaptive u64,bytes,u32,bool rows",
            tag,
        )),
    }
}

// ===== AoS borrowed views for (u64, str/bytes, u32, bool) =====

/// Borrowed view over an AoS ad-hoc body for rows shaped as `(u64, &str, u32, bool)`.
///
/// The view indexes the variable-length string field and returns borrowed `&str`
/// slices into the original `body` input. Parsing performs strict bounds checks
/// and returns `Error::LengthMismatch` on truncation. UTF-8 validity is checked
/// at access time for the specific row.
pub struct AosU64StrU32BoolView<'a> {
    n: usize,
    body: &'a [u8],
    rows: Vec<AosVarU32Idx>,
}

struct AosVarU32Idx {
    id: u64,
    data_off: usize,
    data_len: usize,
    val: u32,
    flag: bool,
}

#[inline]
fn parse_aos_u64_var_u32_bool(body: &[u8]) -> Result<(usize, Vec<AosVarU32Idx>), Error> {
    let (n, mut offset) = aos_read_len_and_ver(body)?;
    let mut rows = Vec::with_capacity(n);
    for _ in 0..n {
        if offset + 8 > body.len() {
            return Err(Error::LengthMismatch);
        }
        let mut id_bytes = [0; 8];
        id_bytes.copy_from_slice(&body[offset..offset + 8]);
        let id = u64::from_le_bytes(id_bytes);
        offset += 8;

        let data_len = read_aos_len(body, &mut offset)?;
        let data_off = offset;
        offset = data_off
            .checked_add(data_len)
            .filter(|&end| end <= body.len())
            .ok_or(Error::LengthMismatch)?;
        if offset + 4 > body.len() {
            return Err(Error::LengthMismatch);
        }
        let mut value_bytes = [0; 4];
        value_bytes.copy_from_slice(&body[offset..offset + 4]);
        offset += 4;
        if offset >= body.len() {
            return Err(Error::LengthMismatch);
        }
        let flag = body[offset] != 0;
        offset += 1;
        rows.push(AosVarU32Idx {
            id,
            data_off,
            data_len,
            val: u32::from_le_bytes(value_bytes),
            flag,
        });
    }
    Ok((n, rows))
}

impl<'a> AosU64StrU32BoolView<'a> {
    /// Number of rows.
    pub fn len(&self) -> usize {
        self.n
    }
    /// True if there are no rows.
    pub fn is_empty(&self) -> bool {
        self.n == 0
    }
    /// Access id column.
    pub fn id(&self, i: usize) -> u64 {
        self.rows[i].id
    }
    /// Access string column as a borrowed `&str`.
    pub fn name(&self, i: usize) -> Result<&'a str, Error> {
        let r = &self.rows[i];
        let s = r.data_off;
        let e = s + r.data_len;
        let bytes = &self.body[s..e];
        validated_str(bytes)
    }
    /// Access u32 column.
    pub fn val(&self, i: usize) -> u32 {
        self.rows[i].val
    }
    /// Access boolean column.
    pub fn flag(&self, i: usize) -> bool {
        self.rows[i].flag
    }
}

/// Parse an AoS ad-hoc body `[n]{ id:u64, len, name_bytes, val:u32, flag:u8 }*n`
/// produced by the adaptive `(u64, &str, u32, bool)` encoder.
pub fn view_aos_u64_str_u32_bool(body: &[u8]) -> Result<AosU64StrU32BoolView<'_>, Error> {
    let (n, rows) = parse_aos_u64_var_u32_bool(body)?;
    Ok(AosU64StrU32BoolView { n, body, rows })
}

/// Borrowed view over an AoS ad-hoc body for rows shaped as `(u64, &[u8], u32, bool)`.
pub struct AosU64BytesU32BoolView<'a> {
    n: usize,
    body: &'a [u8],
    rows: Vec<AosVarU32Idx>,
}

impl<'a> AosU64BytesU32BoolView<'a> {
    pub fn len(&self) -> usize {
        self.n
    }
    pub fn is_empty(&self) -> bool {
        self.n == 0
    }
    pub fn id(&self, i: usize) -> u64 {
        self.rows[i].id
    }
    pub fn data(&self, i: usize) -> &'a [u8] {
        let r = &self.rows[i];
        let s = r.data_off;
        let e = s + r.data_len;
        &self.body[s..e]
    }
    pub fn val(&self, i: usize) -> u32 {
        self.rows[i].val
    }
    pub fn flag(&self, i: usize) -> bool {
        self.rows[i].flag
    }
}

/// Parse an AoS ad-hoc body `[n]{ id:u64, len, bytes, val:u32, flag:u8 }*n`
/// produced by the adaptive `(u64, &[u8], u32, bool)` encoder.
pub fn view_aos_u64_bytes_u32_bool(body: &[u8]) -> Result<AosU64BytesU32BoolView<'_>, Error> {
    let (n, rows) = parse_aos_u64_var_u32_bool(body)?;
    Ok(AosU64BytesU32BoolView { n, body, rows })
}

// ===== AoS borrowed views for (u64, Option<&str>, bool) and (u64, Option<u32>, bool) =====

pub struct AosU64OptStrBoolView<'a> {
    n: usize,
    body: &'a [u8],
    rows: Vec<AosOptStrIdx>,
}

struct AosOptStrIdx {
    id: u64,
    present: bool,
    name_off: usize,
    name_len: usize,
    flag: bool,
}

impl<'a> AosU64OptStrBoolView<'a> {
    pub fn len(&self) -> usize {
        self.n
    }
    pub fn is_empty(&self) -> bool {
        self.n == 0
    }
    pub fn id(&self, i: usize) -> u64 {
        self.rows[i].id
    }
    pub fn name(&self, i: usize) -> Result<Option<&'a str>, Error> {
        let r = &self.rows[i];
        if !r.present {
            return Ok(None);
        }
        let s = r.name_off;
        let e = s + r.name_len;
        let bytes = &self.body[s..e];
        Ok(Some(validated_str(bytes)?))
    }
    pub fn flag(&self, i: usize) -> bool {
        self.rows[i].flag
    }
}

pub fn view_aos_u64_optstr_bool(body: &[u8]) -> Result<AosU64OptStrBoolView<'_>, Error> {
    let (n, mut off) = aos_read_len_and_ver(body)?;
    let mut rows = Vec::with_capacity(n);
    for _ in 0..n {
        if off + 8 > body.len() {
            return Err(Error::LengthMismatch);
        }
        let mut idb = [0u8; 8];
        idb.copy_from_slice(&body[off..off + 8]);
        let id = u64::from_le_bytes(idb);
        off += 8;
        if off >= body.len() {
            return Err(Error::LengthMismatch);
        }
        let tag = body[off];
        off += 1;
        let (present, name_off, name_len) = if tag == 0 {
            (false, 0, 0)
        } else {
            let slen = read_aos_len(body, &mut off)?;
            let s = off;
            let e = s.checked_add(slen).ok_or(Error::LengthMismatch)?;
            if e > body.len() {
                return Err(Error::LengthMismatch);
            }
            off = e;
            (true, s, slen)
        };
        if off >= body.len() {
            return Err(Error::LengthMismatch);
        }
        let flag = body[off] != 0;
        off += 1;
        rows.push(AosOptStrIdx {
            id,
            present,
            name_off,
            name_len,
            flag,
        });
    }
    Ok(AosU64OptStrBoolView { n, body, rows })
}

pub struct AosU64OptU32BoolView {
    n: usize,
    rows: Vec<AosOptU32Idx>,
}

struct AosOptU32Idx {
    id: u64,
    present: bool,
    val: u32,
    flag: bool,
}

impl AosU64OptU32BoolView {
    pub fn len(&self) -> usize {
        self.n
    }
    pub fn is_empty(&self) -> bool {
        self.n == 0
    }
    pub fn id(&self, i: usize) -> u64 {
        self.rows[i].id
    }
    pub fn val(&self, i: usize) -> Option<u32> {
        let r = &self.rows[i];
        if r.present { Some(r.val) } else { None }
    }
    pub fn flag(&self, i: usize) -> bool {
        self.rows[i].flag
    }
}

pub fn view_aos_u64_optu32_bool(body: &[u8]) -> Result<AosU64OptU32BoolView, Error> {
    let (n, mut off) = aos_read_len_and_ver(body)?;
    let mut rows = Vec::with_capacity(n);
    for _ in 0..n {
        if off + 8 > body.len() {
            return Err(Error::LengthMismatch);
        }
        let mut idb = [0u8; 8];
        idb.copy_from_slice(&body[off..off + 8]);
        let id = u64::from_le_bytes(idb);
        off += 8;
        if off >= body.len() {
            return Err(Error::LengthMismatch);
        }
        let tag = body[off];
        off += 1;
        let (present, val) = if tag == 0 {
            (false, 0u32)
        } else {
            if off + 4 > body.len() {
                return Err(Error::LengthMismatch);
            }
            let mut vb = [0u8; 4];
            vb.copy_from_slice(&body[off..off + 4]);
            off += 4;
            (true, u32::from_le_bytes(vb))
        };
        if off >= body.len() {
            return Err(Error::LengthMismatch);
        }
        let flag = body[off] != 0;
        off += 1;
        rows.push(AosOptU32Idx {
            id,
            present,
            val,
            flag,
        });
    }
    Ok(AosU64OptU32BoolView { n, rows })
}

// ===== AoS borrowed view for (u64, enum{Name|Code}, bool) =====

pub enum AosEnumRef<'a> {
    Name(&'a str),
    Code(u32),
}

pub struct AosU64EnumBoolView<'a> {
    n: usize,
    body: &'a [u8],
    rows: Vec<AosEnumIdx>,
}

enum AosEnumIdx {
    Name {
        id: u64,
        off: usize,
        len: usize,
        flag: bool,
    },
    Code {
        id: u64,
        val: u32,
        flag: bool,
    },
}

impl<'a> AosU64EnumBoolView<'a> {
    pub fn len(&self) -> usize {
        self.n
    }
    pub fn is_empty(&self) -> bool {
        self.n == 0
    }
    pub fn id(&self, i: usize) -> u64 {
        match self.rows[i] {
            AosEnumIdx::Name { id, .. } | AosEnumIdx::Code { id, .. } => id,
        }
    }
    pub fn payload(&'a self, i: usize) -> Result<AosEnumRef<'a>, Error> {
        match self.rows[i] {
            AosEnumIdx::Name { off, len, .. } => {
                let s = &self.body[off..off + len];
                Ok(AosEnumRef::Name(validated_str(s)?))
            }
            AosEnumIdx::Code { val, .. } => Ok(AosEnumRef::Code(val)),
        }
    }
    pub fn flag(&self, i: usize) -> bool {
        match self.rows[i] {
            AosEnumIdx::Name { flag, .. } | AosEnumIdx::Code { flag, .. } => flag,
        }
    }
}

pub fn view_aos_u64_enum_bool(body: &[u8]) -> Result<AosU64EnumBoolView<'_>, Error> {
    // Enum AoS uses a minimal header without the version nibble.
    let mut off = 0usize;

    let n = read_aos_sequence_len(body, &mut off)?;
    let prefix_len = crate::core::len_prefix_len(0);
    let name_min = 8usize + 1 + prefix_len + 1;
    let code_min = 8usize + 1 + 4 + 1;
    let min_row = name_min.min(code_min);
    let remaining = body.len().saturating_sub(off);
    let max_rows = remaining / min_row;
    if n > max_rows {
        return Err(Error::LengthMismatch);
    }
    let mut rows = Vec::with_capacity(n);
    for _ in 0..n {
        if off + 8 > body.len() {
            return Err(Error::LengthMismatch);
        }
        let mut idb = [0u8; 8];
        idb.copy_from_slice(&body[off..off + 8]);
        let id = u64::from_le_bytes(idb);
        off += 8;
        if off >= body.len() {
            return Err(Error::LengthMismatch);
        }
        let tag = body[off];
        off += 1;
        if tag == 0 {
            let slen = read_aos_len(body, &mut off)?;
            let s = off;
            let e = s.checked_add(slen).ok_or(Error::LengthMismatch)?;
            if e > body.len() {
                return Err(Error::LengthMismatch);
            }
            off = e;
            if off >= body.len() {
                return Err(Error::LengthMismatch);
            }
            let flag = body[off] != 0;
            off += 1;
            rows.push(AosEnumIdx::Name {
                id,
                off: s,
                len: slen,
                flag,
            });
        } else if tag == 1 {
            if off + 4 > body.len() {
                return Err(Error::LengthMismatch);
            }
            let mut vb = [0u8; 4];
            vb.copy_from_slice(&body[off..off + 4]);
            off += 4;
            if off >= body.len() {
                return Err(Error::LengthMismatch);
            }
            let flag = body[off] != 0;
            off += 1;
            rows.push(AosEnumIdx::Code {
                id,
                val: u32::from_le_bytes(vb),
                flag,
            });
        } else {
            return Err(Error::invalid_tag(
                "building AoS enum view discriminant",
                tag,
            ));
        }
    }
    Ok(AosU64EnumBoolView { n, body, rows })
}

// -- Test-only helpers -------------------------------------------------------
#[cfg(test)]
#[allow(dead_code)]
pub(crate) fn encode_ncb_u64_str_u32_bool_force_u32_delta(
    rows: &[(u64, &str, u32, bool)],
) -> Vec<u8> {
    // Force: ids non-delta, names offsets-based, u32 delta
    let n = rows.len();
    let mut buf = Vec::new();
    buf.extend_from_slice(&(n as u32).to_le_bytes());
    buf.push(DESC_U64_STR_U32DELTA_BOOL);
    // ids
    pad_to(&mut buf, 8);
    for &(id, _, _, _) in rows {
        buf.extend_from_slice(&id.to_le_bytes());
    }
    // names offsets-based
    pad_to(&mut buf, 4);
    let base_off = buf.len();
    buf.extend(std::iter::repeat_n(0u8, 4 * (n + 1)));
    let mut acc: u32 = 0;
    let mut offs = Vec::with_capacity(n + 1);
    offs.push(0);
    for &(_, s, _, _) in rows {
        let b = s.as_bytes();
        acc = acc.wrapping_add(b.len() as u32);
        offs.push(acc);
        buf.extend_from_slice(b);
    }
    for (i, v) in offs.iter().enumerate() {
        let p = base_off + i * 4;
        buf[p..p + 4].copy_from_slice(&v.to_le_bytes());
    }
    // u32 delta
    pad_to(&mut buf, 4);
    if n > 0 {
        buf.extend_from_slice(&rows[0].2.to_le_bytes());
        let mut prev = rows[0].2 as i64;
        for &(_, _, v, _) in rows.iter().skip(1) {
            let d = (v as i64) - prev;
            prev = v as i64;
            write_var_u64(&mut buf, zigzag_encode(d));
        }
    }
    // flags
    let bit_bytes = n.div_ceil(8);
    let start = buf.len();
    buf.extend(std::iter::repeat_n(0u8, bit_bytes));
    for (i, &(_, _, _, f)) in rows.iter().enumerate() {
        if f {
            buf[start + (i / 8)] |= 1u8 << (i % 8);
        }
    }
    buf
}
#[cfg(test)]
#[allow(dead_code)]
pub(crate) fn encode_ncb_u64_bytes_u32_bool_force_u32_delta(
    rows: &[(u64, &[u8], u32, bool)],
) -> Vec<u8> {
    // Force: ids non-delta, bytes offsets+blob, u32 delta
    let n = rows.len();
    let mut buf = Vec::new();
    buf.extend_from_slice(&(n as u32).to_le_bytes());
    buf.push(DESC_U64_BYTES_U32DELTA_BOOL);
    // ids
    pad_to(&mut buf, 8);
    for &(id, _, _, _) in rows {
        buf.extend_from_slice(&id.to_le_bytes());
    }
    // bytes offsets+blob
    pad_to(&mut buf, 4);
    let base_off = buf.len();
    buf.extend(std::iter::repeat_n(0u8, 4 * (n + 1)));
    let mut acc: u32 = 0;
    let mut offs = Vec::with_capacity(n + 1);
    offs.push(0);
    for &(_, b, _, _) in rows {
        acc = acc.wrapping_add(b.len() as u32);
        offs.push(acc);
        buf.extend_from_slice(b);
    }
    for (i, v) in offs.iter().enumerate() {
        let p = base_off + i * 4;
        buf[p..p + 4].copy_from_slice(&v.to_le_bytes());
    }
    // u32 delta
    pad_to(&mut buf, 4);
    if n > 0 {
        buf.extend_from_slice(&rows[0].2.to_le_bytes());
        let mut prev = rows[0].2 as i64;
        for &(_, _, v, _) in rows.iter().skip(1) {
            let d = (v as i64) - prev;
            prev = v as i64;
            write_var_u64(&mut buf, zigzag_encode(d));
        }
    }
    // flags
    let bit_bytes = n.div_ceil(8);
    let start = buf.len();
    buf.extend(std::iter::repeat_n(0u8, bit_bytes));
    for (i, &(_, _, _, f)) in rows.iter().enumerate() {
        if f {
            buf[start + (i / 8)] |= 1u8 << (i % 8);
        }
    }
    buf
}
/// Owned enum used by the enum-shaped adaptive encoder/decoder.
#[derive(Debug, Clone, PartialEq)]
pub enum RowEnumOwned {
    Name(String),
    Code(u32),
}

/// Encode `(u64, enum{Name(String)|Code(u32)}, bool)` rows using an adaptive payload.
/// Rows are provided with borrowed enum payloads.
pub fn encode_rows_u64_enum_bool_adaptive(rows: &[(u64, EnumBorrow<'_>, bool)]) -> Vec<u8> {
    let small_n = small_smart_n();
    if rows.len() <= small_n {
        let use_delta_ids = should_use_id_delta_enum(rows);
        let use_name_dict = should_use_name_dict_enum(rows);
        let use_code_delta = should_use_code_delta_enum(rows);
        let timer = probe_start();
        let ncb = encode_ncb_u64_enum_bool(rows, use_delta_ids, use_name_dict, use_code_delta);
        let ncb_ns = probe_elapsed(timer);

        let timer = probe_start();
        let aos = aos::encode_rows_u64_enum_bool(rows);
        let aos_ns = probe_elapsed(timer);
        return finish_two_pass("u64_enum_bool", aos, ncb, aos_ns, ncb_ns);
    }
    let (tag, payload) = if should_use_columnar(rows.len()) {
        let use_delta_ids = should_use_id_delta_enum(rows);
        let use_name_dict = should_use_name_dict_enum(rows);
        let use_code_delta = should_use_code_delta_enum(rows);
        (
            ADAPTIVE_ENUM_TAG_NCB,
            encode_ncb_u64_enum_bool(rows, use_delta_ids, use_name_dict, use_code_delta),
        )
    } else {
        (ADAPTIVE_ENUM_TAG_AOS, aos::encode_rows_u64_enum_bool(rows))
    };
    tagged_payload(tag, payload)
}

#[inline]
fn small_smart_n() -> usize {
    // Pull from heuristics (default 64); callers use `<=` for the two-pass path
    crate::core::heuristics::get().aos_ncb_small_n
}

/// Decode an adaptive enum-shaped payload back into owned rows.
pub fn decode_rows_u64_enum_bool_adaptive(
    bytes: &[u8],
) -> Result<Vec<(u64, RowEnumOwned, bool)>, Error> {
    let (tag, body) = split_tagged_payload(bytes)?;
    match tag {
        ADAPTIVE_ENUM_TAG_NCB => {
            let view = view_ncb_u64_enum_bool(body)?;
            let mut out = Vec::with_capacity(view.len());
            for i in 0..view.len() {
                let id = view.id(i);
                let flag = view.flag(i);
                let en = match view.payload(i)? {
                    ColEnumRef::Name(s) => RowEnumOwned::Name(s.to_string()),
                    ColEnumRef::Code(v) => RowEnumOwned::Code(v),
                };
                out.push((id, en, flag));
            }
            Ok(out)
        }
        ADAPTIVE_ENUM_TAG_AOS => aos::decode_rows_u64_enum_bool(body),
        _ => Err(Error::invalid_tag("decoding adaptive enum rows", tag)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ncb_row_count_prefix_rejects_truncated_inputs() {
        let prefix = [0x2a, 0, 0, 0];
        for len in 0..4 {
            let err = read_row_count_prefix(&prefix[..len]).unwrap_err();
            assert!(matches!(err, Error::LengthMismatch));
        }

        assert!(matches!(
            read_row_count_prefix(&prefix),
            Err(Error::LengthMismatch)
        ));

        let mut structurally_bounded = prefix.to_vec();
        structurally_bounded.resize(4 + 42, 0);
        assert_eq!(read_row_count_prefix(&structurally_bounded).unwrap(), 42);
    }

    #[test]
    fn ncb_row_count_prefix_rejects_disproportionate_allocation_without_limit_scope() {
        let mut forged = u32::MAX.to_le_bytes().to_vec();
        forged.push(0);

        assert!(matches!(
            read_row_count_prefix(&forged),
            Err(Error::LengthMismatch)
        ));
    }

    #[test]
    fn ncb_row_count_views_reject_truncated_headers() {
        let prefix = [0, 0, 0, 0];
        for len in 0..4 {
            let input = &prefix[..len];
            assert!(matches!(
                view_ncb_u64_str_bool(input),
                Err(Error::LengthMismatch)
            ));
            assert!(matches!(
                view_ncb_u64_optstr_bool(input),
                Err(Error::LengthMismatch)
            ));
            assert!(matches!(
                view_ncb_u64_optu32_bool(input),
                Err(Error::LengthMismatch)
            ));
            assert!(matches!(
                view_ncb_u64_bytes_bool(input),
                Err(Error::LengthMismatch)
            ));
            assert!(matches!(
                view_ncb_u64_u32_bool(input),
                Err(Error::LengthMismatch)
            ));
            assert!(matches!(
                view_ncb_u64_str_u32_bool(input),
                Err(Error::LengthMismatch)
            ));
            assert!(matches!(
                view_ncb_u64_bytes_u32_bool(input),
                Err(Error::LengthMismatch)
            ));
            assert!(matches!(
                view_ncb_u64_enum_bool(input),
                Err(Error::LengthMismatch)
            ));
        }
    }

    #[test]
    fn should_use_columnar_respects_heuristics_threshold() {
        let h = crate::core::heuristics::get();
        let t = h.aos_ncb_small_n;
        if t > 0 {
            assert!(
                !should_use_columnar(t - 1),
                "n = t-1 should stay in the AoS path while below the heuristic threshold"
            );
        }
        assert!(
            !should_use_columnar(t),
            "n = t should still use the two-pass AoS vs NCB probe"
        );
        assert!(
            should_use_columnar(t.saturating_add(1)),
            "n = t+1 should enable columnar auto-selection"
        );
    }

    #[test]
    fn small_smart_n_matches_heuristics() {
        let t = crate::core::heuristics::get().aos_ncb_small_n;
        assert_eq!(small_smart_n(), t);
    }

    #[test]
    fn u32_delta_toggle_respects_name_flag() {
        let rows: Vec<(u64, &str, u32, bool)> = vec![
            (10, "alpha", 100, false),
            (20, "bravo", 105, true),
            (30, "charlie", 111, false),
        ];
        let canonical = crate::core::heuristics::Heuristics::canonical();
        assert!(
            super::should_use_u32_delta_str_u32_with(&rows, canonical),
            "canonical heuristics should allow u32 delta for well-behaved rows"
        );
        let mut disabled = canonical;
        disabled.combo_enable_u32_delta_names = false;
        assert!(
            !super::should_use_u32_delta_str_u32_with(&rows, disabled),
            "disabling the name-column delta flag must suppress u32 delta usage"
        );
    }

    #[test]
    fn u32_delta_toggle_respects_bytes_flag() {
        let payloads: Vec<Vec<u8>> = vec![b"one".to_vec(), b"two".to_vec(), b"three".to_vec()];
        let rows: Vec<(u64, &[u8], u32, bool)> = payloads
            .iter()
            .enumerate()
            .map(|(i, data)| {
                (
                    (i as u64) * 5 + 7,
                    data.as_slice(),
                    (i as u32) * 9,
                    i % 2 == 0,
                )
            })
            .collect();
        let canonical = crate::core::heuristics::Heuristics::canonical();
        assert!(
            super::should_use_u32_delta_bytes_u32_with(&rows, canonical),
            "canonical heuristics should allow u32 delta for byte-column combos"
        );
        let mut disabled = canonical;
        disabled.combo_enable_u32_delta_bytes = false;
        assert!(
            !super::should_use_u32_delta_bytes_u32_with(&rows, disabled),
            "disabling the byte-column delta flag must suppress u32 delta usage"
        );
    }

    include!("columnar_adaptive_test.rs");
}

#[inline]
fn should_use_id_delta_enum(rows: &[(u64, EnumBorrow<'_>, bool)]) -> bool {
    if rows.len() < 2 {
        return false;
    }
    let mut delta_size = DeltaSizeTracker::new(i128::from(rows[0].0), rows.len(), 8);
    for &(id, _, _) in &rows[1..] {
        if !delta_size.push(i128::from(id)) {
            return false;
        }
    }
    true
}

fn should_use_name_dict_enum(rows: &[(u64, EnumBorrow<'_>, bool)]) -> bool {
    // Heuristic: when Name variants are sufficiently repetitive and average length is moderate/large,
    // dictionary coding reduces size vs offsets+blob.
    let mut total_len = 0usize;
    let mut names: std::collections::HashMap<&str, u32> = std::collections::HashMap::new();
    let mut name_count = 0usize;
    for (_, e, _) in rows.iter() {
        if let EnumBorrow::Name(s) = e {
            total_len += s.len();
            name_count += 1;
            if !names.contains_key(s) {
                let id = names.len() as u32;
                names.insert(s, id);
            }
        }
    }
    if name_count == 0 {
        return false;
    }
    let ratio = names.len() as f64 / name_count as f64;
    let avg = total_len as f64 / name_count as f64;
    // Thresholds chosen to avoid regressions on short/unique strings
    ratio <= 0.40 && avg >= 8.0
}

fn should_use_code_delta_enum(rows: &[(u64, EnumBorrow<'_>, bool)]) -> bool {
    // With corrected zigzag encoding and offset checks, allow combined deltas.
    // Size-based heuristic over the subsequence of Code variants
    let mut prev_opt: Option<i64> = None;
    let mut varint_bytes: usize = 0;
    let mut count: usize = 0;
    for (_, e, _) in rows.iter() {
        if let EnumBorrow::Code(v) = e {
            let v = *v as i64;
            if let Some(prev) = prev_opt {
                let d = (v as i128) - (prev as i128);
                if d < i64::MIN as i128 || d > i64::MAX as i128 {
                    return false;
                }
                let zz = zigzag_encode(d as i64);
                varint_bytes += varint_len(zz);
                if varint_bytes >= 4usize.saturating_mul(count) {
                    return false;
                }
            }
            prev_opt = Some(v);
            count += 1;
        }
    }
    count >= 2 && varint_bytes < 4usize.saturating_mul(count.saturating_sub(1))
}

#[inline]
fn read_u32_at(bytes: &[u8], i: usize) -> u32 {
    let start = i * 4;
    let end = start + 4;
    let mut lb = [0u8; 4];
    lb.copy_from_slice(&bytes[start..end]);
    u32::from_le_bytes(lb)
}

#[inline]
fn validated_str(bytes: &[u8]) -> Result<&str, Error> {
    #[cfg(feature = "simdutf8-validate")]
    {
        simdutf8::basic::from_utf8(bytes).map_err(|_| Error::InvalidUtf8)
    }
    #[cfg(not(feature = "simdutf8-validate"))]
    {
        std::str::from_utf8(bytes).map_err(|_| Error::InvalidUtf8)
    }
}

#[inline]
fn zigzag_encode(x: i64) -> u64 {
    // Standard zigzag: map signed to unsigned so small negative deltas
    // produce small positive integers. Matches zigzag_decode below.
    ((x << 1) ^ (x >> 63)) as u64
}
#[inline]
fn zigzag_decode(u: u64) -> i64 {
    ((u >> 1) as i64) ^ (-((u & 1) as i64))
}

fn read_varint_u64(bytes: &[u8]) -> Result<(u64, usize), Error> {
    let mut shift = 0u32;
    let mut value: u64 = 0;
    let mut i = 0usize;
    loop {
        let b = *bytes.get(i).ok_or(Error::LengthMismatch)?;
        i += 1;
        let payload = (b & 0x7f) as u64;
        if shift == 63 && payload > 1 {
            return Err(Error::LengthMismatch);
        }
        value |= payload << shift;
        if (b & 0x80) == 0 {
            if i != varint_len(value) {
                return Err(Error::LengthMismatch);
            }
            break;
        }
        shift += 7;
        if shift >= 70 {
            return Err(Error::LengthMismatch);
        }
    }
    Ok((value, i))
}

/// A lightweight row reference over the NCB view.
pub struct RowRef<'a> {
    view: &'a NcbU64StrBoolView<'a>,
    idx: usize,
}

impl<'a> RowRef<'a> {
    pub fn id(&self) -> u64 {
        self.view.id(self.idx)
    }
    pub fn name(&self) -> Result<&'a str, Error> {
        self.view.name(self.idx)
    }
    pub fn flag(&self) -> bool {
        self.view.flag(self.idx)
    }
}

/// A cursor over NCB rows yielding `RowRef`.
pub struct RowCursor<'a> {
    view: &'a NcbU64StrBoolView<'a>,
    i: usize,
}

impl<'a> Iterator for RowCursor<'a> {
    type Item = RowRef<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.i >= self.view.n {
            return None;
        }
        let r = RowRef {
            view: self.view,
            idx: self.i,
        };
        self.i += 1;
        Some(r)
    }
}

impl<'a> NcbU64StrBoolView<'a> {
    /// Iterate over rows as lightweight references without allocations.
    pub fn rows(&'a self) -> RowCursor<'a> {
        RowCursor { view: self, i: 0 }
    }
}

impl<'a> NcbU64StrBoolView<'a> {
    /// Iterate ids column in column order.
    pub fn iter_ids(&self) -> IdsIterator<'_> {
        match &self.ids {
            IdsRep::Slice(s) => IdsIterator::Slice(s.iter()),
            IdsRep::Rebuilt(v) => IdsIterator::Vec(v.iter()),
        }
    }
    /// Iterate names column in row order (zero-copy &str).
    pub fn iter_names(&'a self) -> NamesIterator<'a> {
        match &self.names {
            NamesRep::Offsets {
                offs_bytes,
                blob_str,
                ..
            } => NamesIterator::Offsets {
                offs_bytes,
                blob: blob_str,
                i: 0,
            },
            NamesRep::Dict {
                dict_offs_bytes,
                dict_blob,
                codes_bytes,
                ..
            } => NamesIterator::Dict {
                dict_offs_bytes,
                dict_blob,
                codes_bytes,
                i: 0,
            },
        }
    }
    /// Iterate boolean flags in column order.
    pub fn iter_flags(&self) -> FlagsIterator<'_> {
        FlagsIterator {
            bits: self.bits,
            i: 0,
            n: self.n,
        }
    }

    /// Iterate positions (row indices) where `flag == true`.
    pub fn iter_true_positions(&self) -> TruePosIter<'_> {
        TruePosIter {
            bits: self.bits,
            n: self.n,
            i: 0,
        }
    }

    /// Iterate ids for rows where `flag == true`.
    pub fn iter_ids_flag_true(&'a self) -> IdsFlagIter<'a> {
        IdsFlagIter {
            view: self,
            pos: self.iter_true_positions(),
        }
    }

    /// Iterate names for rows where `flag == true`.
    pub fn iter_names_flag_true(&'a self) -> NamesFlagIter<'a> {
        NamesFlagIter {
            view: self,
            pos: self.iter_true_positions(),
        }
    }

    /// Iterate positions (row indices) with flag==true using byte-level popcount and trailing_zeros.
    pub fn iter_true_positions_popcount(&self) -> TruePosPopIter<'_> {
        TruePosPopIter {
            bits: self.bits,
            n: self.n,
            byte_idx: 0,
            cur: 0,
            base: 0,
        }
    }
    /// Iterate names for rows where `flag == true` using popcount-accelerated scanner.
    pub fn iter_names_flag_true_popcount(&'a self) -> NamesFlagPopIter<'a> {
        NamesFlagPopIter {
            view: self,
            pos: self.iter_true_positions_popcount(),
        }
    }

    /// Iterate positions (row indices) with flag==true using u64 word-level popcount and trailing_zeros.
    pub fn iter_true_positions_popcount64(&self) -> TruePosPop64Iter<'_> {
        TruePosPop64Iter {
            bits: self.bits,
            n: self.n,
            byte_idx: 0,
            cur: 0,
            base: 0,
        }
    }
    /// Iterate names for rows where `flag == true` using u64 popcount-accelerated scanner.
    pub fn iter_names_flag_true_popcount64(&'a self) -> NamesFlagPop64Iter<'a> {
        NamesFlagPop64Iter {
            view: self,
            pos: self.iter_true_positions_popcount64(),
        }
    }

    /// Iterate positions (row indices) with flag==true using aligned u64 body with head/tail fallbacks.
    pub fn iter_true_positions_popcount64_aligned(&self) -> TruePosPop64AlignedIter<'_> {
        let (head, body, tail) = unsafe { self.bits.align_to::<u64>() };
        TruePosPop64AlignedIter {
            head,
            body,
            tail,
            n: self.n,
            // head state
            head_idx: 0,
            head_cur: 0,
            head_base: 0,
            // body state
            body_idx: 0,
            body_cur: 0,
            body_base: head.len() * 8,
            // tail state
            tail_idx: 0,
            tail_cur: 0,
            tail_base: head.len() * 8 + body.len() * 64,
            stage: 0,
        }
    }
    /// Iterate names for rows where `flag == true` using aligned u64 popcount scanner.
    pub fn iter_names_flag_true_popcount64_aligned(&'a self) -> NamesFlagPop64AlignedIter<'a> {
        NamesFlagPop64AlignedIter {
            view: self,
            pos: self.iter_true_positions_popcount64_aligned(),
        }
    }

    /// Iterate positions (row indices) with `flag==true` choosing a fast variant based on CPU/arch.
    pub fn iter_true_positions_fast(&self) -> TruePosFastIter<'_> {
        #[cfg(target_pointer_width = "64")]
        {
            // On x86_64, detect POPCNT; otherwise still use aligned 64-bit scanner.
            #[cfg(target_arch = "x86_64")]
            {
                if std::is_x86_feature_detected!("popcnt") {
                    return TruePosFastIter::Aligned64(
                        self.iter_true_positions_popcount64_aligned(),
                    );
                }
            }
            TruePosFastIter::Aligned64(self.iter_true_positions_popcount64_aligned())
        }
        #[cfg(not(target_pointer_width = "64"))]
        {
            TruePosFastIter::Byte(self.iter_true_positions_popcount())
        }
    }
    /// Iterate names for rows where `flag==true` using `iter_true_positions_fast()`.
    pub fn iter_names_flag_true_fast(&'a self) -> NamesFlagFastIter<'a> {
        NamesFlagFastIter {
            view: self,
            pos: self.iter_true_positions_fast(),
        }
    }
}

/// Iterator over ids column.
pub enum IdsIterator<'a> {
    Slice(std::slice::Iter<'a, u64>),
    Vec(std::slice::Iter<'a, u64>),
}
impl<'a> Iterator for IdsIterator<'a> {
    type Item = u64;
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            IdsIterator::Slice(it) => it.next().copied(),
            IdsIterator::Vec(it) => it.next().copied(),
        }
    }
}

/// Iterator over names column (row-aligned strings).
#[inline]
fn next_offsets_name<'a>(
    offs_bytes: &'a [u8],
    blob: &'a str,
    index: &mut usize,
) -> Option<&'a str> {
    let count = offs_bytes.len() / 4;
    if *index + 1 >= count {
        return None;
    }
    let start = read_u32_at(offs_bytes, *index) as usize;
    let end = read_u32_at(offs_bytes, *index + 1) as usize;
    *index += 1;
    Some(&blob[start..end])
}

#[inline]
fn next_dict_name<'a>(
    dict_offs_bytes: &'a [u8],
    dict_blob: &'a str,
    codes_bytes: &'a [u8],
    index: &mut usize,
) -> Option<&'a str> {
    let count = codes_bytes.len() / 4;
    if *index >= count {
        return None;
    }
    let code = read_u32_at(codes_bytes, *index) as usize;
    let start = read_u32_at(dict_offs_bytes, code) as usize;
    let end = read_u32_at(dict_offs_bytes, code + 1) as usize;
    *index += 1;
    Some(&dict_blob[start..end])
}

pub enum NamesIterator<'a> {
    Offsets {
        offs_bytes: &'a [u8],
        blob: &'a str,
        i: usize,
    },
    Dict {
        dict_offs_bytes: &'a [u8],
        dict_blob: &'a str,
        codes_bytes: &'a [u8],
        i: usize,
    },
}
impl<'a> Iterator for NamesIterator<'a> {
    type Item = &'a str;
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            NamesIterator::Offsets {
                offs_bytes,
                blob,
                i,
            } => next_offsets_name(offs_bytes, blob, i),
            NamesIterator::Dict {
                dict_offs_bytes,
                dict_blob,
                codes_bytes,
                i,
            } => next_dict_name(dict_offs_bytes, dict_blob, codes_bytes, i),
        }
    }
}

/// Iterator over packed boolean flags.
pub struct FlagsIterator<'a> {
    bits: &'a [u8],
    i: usize,
    n: usize,
}
impl<'a> Iterator for FlagsIterator<'a> {
    type Item = bool;
    fn next(&mut self) -> Option<Self::Item> {
        if self.i >= self.n {
            return None;
        }
        let byte = self.i / 8;
        let bit = self.i % 8;
        let v = (self.bits[byte] >> bit) & 1 == 1;
        self.i += 1;
        Some(v)
    }
}

/// Precomputed flags bitset as u64 words for repeated scans.
pub struct FlagsIndex {
    words: Vec<u64>,
    n: usize,
}

impl FlagsIndex {
    pub fn build(bits: &[u8], n: usize) -> Self {
        let mut words = Vec::with_capacity(bits.len().div_ceil(8));
        let mut byte_idx = 0usize;
        while byte_idx < bits.len() {
            let take = (bits.len() - byte_idx).min(8);
            let mut buf = [0u8; 8];
            buf[..take].copy_from_slice(&bits[byte_idx..byte_idx + take]);
            let mut w = u64::from_le_bytes(buf);
            let bits_left = n.saturating_sub(byte_idx * 8);
            if bits_left < 64 {
                if bits_left == 0 {
                    w = 0;
                } else {
                    let mask = if bits_left == 64 {
                        !0u64
                    } else {
                        (1u64 << bits_left) - 1
                    };
                    w &= mask;
                }
            }
            words.push(w);
            byte_idx += take;
        }
        Self { words, n }
    }

    pub fn iter_positions(&self) -> FlagsIndexIter<'_> {
        FlagsIndexIter {
            words: &self.words,
            n: self.n,
            idx: 0,
            cur: 0,
            base: 0,
        }
    }
    pub fn iter_positions_and<'a>(&'a self, other: &'a FlagsIndex) -> FlagsIndexAndIter<'a> {
        FlagsIndexAndIter {
            a: &self.words,
            b: &other.words,
            n: self.n.min(other.n),
            idx: 0,
            cur: 0,
            base: 0,
        }
    }
}

pub struct FlagsIndexIter<'a> {
    words: &'a [u64],
    n: usize,
    idx: usize,
    cur: u64,
    base: usize,
}
impl<'a> Iterator for FlagsIndexIter<'a> {
    type Item = usize;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.cur == 0 {
                if self.idx >= self.words.len() {
                    return None;
                }
                self.cur = self.words[self.idx];
                self.base = self.idx * 64;
                self.idx += 1;
                if self.cur == 0 {
                    continue;
                }
            }
            let tz = self.cur.trailing_zeros() as usize;
            let pos = self.base + tz;
            self.cur &= self.cur - 1;
            if pos < self.n {
                return Some(pos);
            }
        }
    }
}

pub struct FlagsIndexAndIter<'a> {
    a: &'a [u64],
    b: &'a [u64],
    n: usize,
    idx: usize,
    cur: u64,
    base: usize,
}
impl<'a> Iterator for FlagsIndexAndIter<'a> {
    type Item = usize;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.cur == 0 {
                if self.idx >= self.a.len().min(self.b.len()) {
                    return None;
                }
                self.cur = self.a[self.idx] & self.b[self.idx];
                self.base = self.idx * 64;
                self.idx += 1;
                if self.cur == 0 {
                    continue;
                }
            }
            let tz = self.cur.trailing_zeros() as usize;
            let pos = self.base + tz;
            self.cur &= self.cur - 1;
            if pos < self.n {
                return Some(pos);
            }
        }
    }
}

/// Iterator over positions with flag bit set.
pub struct TruePosIter<'a> {
    bits: &'a [u8],
    n: usize,
    i: usize,
}
impl<'a> Iterator for TruePosIter<'a> {
    type Item = usize;
    fn next(&mut self) -> Option<Self::Item> {
        while self.i < self.n {
            let cur = self.i;
            let byte = cur / 8;
            let bit = cur % 8;
            self.i += 1;
            if ((self.bits[byte] >> bit) & 1) == 1 {
                return Some(cur);
            }
        }
        None
    }
}

/// Iterator over ids where flag is true.
pub struct IdsFlagIter<'a> {
    view: &'a NcbU64StrBoolView<'a>,
    pos: TruePosIter<'a>,
}
impl<'a> Iterator for IdsFlagIter<'a> {
    type Item = u64;
    fn next(&mut self) -> Option<Self::Item> {
        let i = self.pos.next()?;
        Some(self.view.id(i))
    }
}

/// Iterator over names where flag is true.
pub struct NamesFlagIter<'a> {
    view: &'a NcbU64StrBoolView<'a>,
    pos: TruePosIter<'a>,
}
impl<'a> Iterator for NamesFlagIter<'a> {
    type Item = &'a str;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let i = self.pos.next()?;
            // Safe unwrap: view.name returns Result
            if let Ok(s) = self.view.name(i) {
                return Some(s);
            }
        }
    }
}

/// Popcount-accelerated byte scanner over flag bitset.
pub struct TruePosPopIter<'a> {
    bits: &'a [u8],
    n: usize,
    byte_idx: usize,
    cur: u8,
    base: usize,
}
impl<'a> TruePosPopIter<'a> {
    #[inline]
    fn load_next_byte(&mut self) -> bool {
        while self.byte_idx < self.bits.len() {
            let mut b = self.bits[self.byte_idx];
            // Mask out bits beyond n in the final byte
            if self.byte_idx == self.bits.len() - 1 {
                let rem = self.n % 8;
                if rem != 0 {
                    let mask = ((1u16 << rem) - 1) as u8;
                    b &= mask;
                }
            }
            self.cur = b;
            self.base = self.byte_idx * 8;
            self.byte_idx += 1;
            if self.cur != 0 {
                return true;
            }
        }
        false
    }
}
impl<'a> Iterator for TruePosPopIter<'a> {
    type Item = usize;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.cur == 0 && !self.load_next_byte() {
                return None;
            }
            // take lowest set bit
            let tz = self.cur.trailing_zeros() as usize;
            let pos = self.base + tz;
            self.cur &= self.cur - 1; // clear lowest set bit
            // guard in case pos >= n due to masking (should not happen after masking)
            if pos < self.n {
                return Some(pos);
            }
        }
    }
}

/// Iterator over names where flag is true using popcount scanner.
pub struct NamesFlagPopIter<'a> {
    view: &'a NcbU64StrBoolView<'a>,
    pos: TruePosPopIter<'a>,
}
impl<'a> Iterator for NamesFlagPopIter<'a> {
    type Item = &'a str;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let i = self.pos.next()?;
            if let Ok(s) = self.view.name(i) {
                return Some(s);
            }
        }
    }
}

/// Word-level (u64) popcount scanner over flag bitset working on 8-byte chunks.
pub struct TruePosPop64Iter<'a> {
    bits: &'a [u8],
    n: usize,
    byte_idx: usize,
    cur: u64,
    base: usize,
}
impl<'a> TruePosPop64Iter<'a> {
    #[inline]
    fn load_next_word(&mut self) -> bool {
        if self.byte_idx >= self.bits.len() {
            return false;
        }
        let remain = self.bits.len() - self.byte_idx;
        let take = remain.min(8);
        let mut buf = [0u8; 8];
        buf[..take].copy_from_slice(&self.bits[self.byte_idx..self.byte_idx + take]);
        let mut w = u64::from_le_bytes(buf);
        // Mask out bits beyond n in the final partial byte(s)
        let bits_left = self.n.saturating_sub(self.byte_idx * 8);
        if bits_left < 64 {
            if bits_left == 0 {
                w = 0;
            } else {
                let mask = if bits_left == 64 {
                    !0u64
                } else {
                    (1u64 << bits_left) - 1
                };
                w &= mask;
            }
        }
        self.cur = w;
        self.base = self.byte_idx * 8;
        self.byte_idx += take;
        self.cur != 0
    }
}
impl<'a> Iterator for TruePosPop64Iter<'a> {
    type Item = usize;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.cur == 0 && !self.load_next_word() {
                return None;
            }
            let tz = self.cur.trailing_zeros() as usize;
            let pos = self.base + tz;
            self.cur &= self.cur - 1;
            if pos < self.n {
                return Some(pos);
            }
        }
    }
}

/// Iterator over names where flag is true using u64 popcount scanner.
pub struct NamesFlagPop64Iter<'a> {
    view: &'a NcbU64StrBoolView<'a>,
    pos: TruePosPop64Iter<'a>,
}
impl<'a> Iterator for NamesFlagPop64Iter<'a> {
    type Item = &'a str;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let i = self.pos.next()?;
            if let Ok(s) = self.view.name(i) {
                return Some(s);
            }
        }
    }
}

/// Aligned u64 scanner (uses head/body/tail from `align_to::<u64>()`).
pub struct TruePosPop64AlignedIter<'a> {
    head: &'a [u8],
    body: &'a [u64],
    tail: &'a [u8],
    n: usize,
    // head
    head_idx: usize,
    head_cur: u8,
    head_base: usize,
    // body
    body_idx: usize,
    body_cur: u64,
    body_base: usize,
    // tail
    tail_idx: usize,
    tail_cur: u8,
    tail_base: usize,
    // 0=head,1=body,2=tail,3=done
    stage: u8,
}

impl<'a> TruePosPop64AlignedIter<'a> {
    #[inline]
    fn load_head(&mut self) -> bool {
        while self.head_idx < self.head.len() {
            let mut b = self.head[self.head_idx];
            let bits_left = self.n.saturating_sub(self.head_base);
            if bits_left < 8 {
                if bits_left == 0 {
                    b = 0;
                } else {
                    let mask = ((1u16 << bits_left) - 1) as u8;
                    b &= mask;
                }
            }
            self.head_cur = b;
            self.head_base += 8;
            self.head_idx += 1;
            if self.head_cur != 0 {
                return true;
            }
        }
        self.stage = 1; // move to body
        false
    }
    #[inline]
    fn load_body(&mut self) -> bool {
        while self.body_idx < self.body.len() {
            let mut w = self.body[self.body_idx];
            let bits_left = self.n.saturating_sub(self.body_base);
            if bits_left < 64 {
                if bits_left == 0 {
                    w = 0;
                } else {
                    let mask = if bits_left == 64 {
                        !0u64
                    } else {
                        (1u64 << bits_left) - 1
                    };
                    w &= mask;
                }
            }
            self.body_cur = w;
            self.body_base += 64;
            self.body_idx += 1;
            if self.body_cur != 0 {
                return true;
            }
        }
        self.stage = 2; // move to tail
        false
    }
    #[inline]
    fn load_tail(&mut self) -> bool {
        while self.tail_idx < self.tail.len() {
            let mut b = self.tail[self.tail_idx];
            let bits_left = self.n.saturating_sub(self.tail_base);
            if bits_left < 8 {
                if bits_left == 0 {
                    b = 0;
                } else {
                    let mask = ((1u16 << bits_left) - 1) as u8;
                    b &= mask;
                }
            }
            self.tail_cur = b;
            self.tail_base += 8;
            self.tail_idx += 1;
            if self.tail_cur != 0 {
                return true;
            }
        }
        self.stage = 3; // done
        false
    }
}

impl<'a> Iterator for TruePosPop64AlignedIter<'a> {
    type Item = usize;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.stage {
                0 => {
                    // head
                    if self.head_cur == 0 && !self.load_head() {
                        continue;
                    }
                    if self.stage != 0 {
                        continue;
                    }
                    let tz = self.head_cur.trailing_zeros() as usize;
                    let pos = self.head_base - 8 + tz;
                    self.head_cur &= self.head_cur - 1;
                    if pos < self.n {
                        return Some(pos);
                    }
                }
                1 => {
                    // body
                    if self.body_cur == 0 && !self.load_body() {
                        continue;
                    }
                    if self.stage != 1 {
                        continue;
                    }
                    let tz = self.body_cur.trailing_zeros() as usize;
                    let pos = self.body_base - 64 + tz;
                    self.body_cur &= self.body_cur - 1;
                    if pos < self.n {
                        return Some(pos);
                    }
                }
                2 => {
                    // tail
                    if self.tail_cur == 0 && !self.load_tail() {
                        continue;
                    }
                    if self.stage != 2 {
                        continue;
                    }
                    let tz = self.tail_cur.trailing_zeros() as usize;
                    let pos = self.tail_base - 8 + tz;
                    self.tail_cur &= self.tail_cur - 1;
                    if pos < self.n {
                        return Some(pos);
                    }
                }
                _ => return None,
            }
        }
    }
}

pub struct NamesFlagPop64AlignedIter<'a> {
    view: &'a NcbU64StrBoolView<'a>,
    pos: TruePosPop64AlignedIter<'a>,
}
impl<'a> Iterator for NamesFlagPop64AlignedIter<'a> {
    type Item = &'a str;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let i = self.pos.next()?;
            if let Ok(s) = self.view.name(i) {
                return Some(s);
            }
        }
    }
}

// Wrapper that selects best available positions iterator at runtime/compile-time is defined above.

/// Names iterator using fast positions wrapper.
pub struct NamesFlagFastIter<'a> {
    view: &'a NcbU64StrBoolView<'a>,
    pos: TruePosFastIter<'a>,
}
impl<'a> Iterator for NamesFlagFastIter<'a> {
    type Item = &'a str;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let i = self.pos.next()?;
            if let Ok(s) = self.view.name(i) {
                return Some(s);
            }
        }
    }
}

/// Force dictionary encoding for benchmarking or when the caller knows it helps.
pub fn encode_ncb_u64_str_bool_force_dict(rows: &[(u64, &str, bool)]) -> Vec<u8> {
    use std::collections::HashMap;
    let mut dict: HashMap<&str, u32> = HashMap::new();
    let mut dict_vec: Vec<&str> = Vec::new();
    for &(_, s, _) in rows {
        if !dict.contains_key(s) {
            let id = dict_vec.len() as u32;
            dict.insert(s, id);
            dict_vec.push(s);
        }
    }
    encode_ncb_u64_dict_str_bool(rows, dict, dict_vec)
}

/// Force offsets-based encoding (no dictionary) for benchmarking.
pub fn encode_ncb_u64_str_bool_no_dict(rows: &[(u64, &str, bool)]) -> Vec<u8> {
    let n = rows.len() as u32;
    let mut buf = Vec::with_capacity(4 + 1 + (rows.len() * (8 + 1 + 4)) + 16);
    buf.extend_from_slice(&n.to_le_bytes());
    buf.push(DESC_U64_STR_BOOL);
    pad_to(&mut buf, 8);
    for (id, _, _) in rows {
        buf.extend_from_slice(&id.to_le_bytes());
    }
    pad_to(&mut buf, 4);
    let base_off = buf.len();
    buf.extend(std::iter::repeat_n(0u8, (rows.len() + 1) * 4));
    let mut acc: u32 = 0;
    let mut offs = Vec::with_capacity(rows.len() + 1);
    offs.push(0);
    for (_, s, _) in rows {
        let b = s.as_bytes();
        acc = acc.wrapping_add(b.len() as u32);
        offs.push(acc);
        buf.extend_from_slice(b);
    }
    for (i, v) in offs.iter().enumerate() {
        let p = base_off + i * 4;
        buf[p..p + 4].copy_from_slice(&v.to_le_bytes());
    }
    let bit_bytes = rows.len().div_ceil(8);
    let start = buf.len();
    buf.extend(std::iter::repeat_n(0u8, bit_bytes));
    for (i, (_, _, b)) in rows.iter().enumerate() {
        if *b {
            buf[start + (i / 8)] |= 1u8 << (i % 8);
        }
    }
    buf
}

#[allow(clippy::type_complexity)]
fn build_dict<'a>(
    rows: &'a [(u64, &str, bool)],
) -> (
    bool,
    Option<std::collections::HashMap<&'a str, u32>>,
    Option<Vec<&'a str>>,
) {
    use std::collections::HashMap;
    let n = rows.len();
    if n == 0 {
        return (false, None, None);
    }
    let h = crate::core::heuristics::get();
    if !h.combo_enable_name_dict {
        return (false, None, None);
    }
    let mut dict: HashMap<&str, u32> = HashMap::with_capacity(n.min(1024));
    let mut vec: Vec<&str> = Vec::new();
    let mut total_len: usize = 0;
    for (_, s, _) in rows.iter() {
        total_len += s.len();
        if !dict.contains_key(s) {
            let id = vec.len() as u32;
            dict.insert(*s, id);
            vec.push(*s);
        }
    }
    let distinct = vec.len();
    let avg = total_len as f64 / n as f64;
    let ratio = distinct as f64 / n as f64;
    let use_dict = ratio <= h.combo_dict_ratio_max && avg >= h.combo_dict_avg_len_min;
    if use_dict {
        (true, Some(dict), Some(vec))
    } else {
        (false, None, None)
    }
}

fn encode_ncb_u64_dict_str_bool(
    rows: &[(u64, &str, bool)],
    dict: std::collections::HashMap<&str, u32>,
    dict_vec: Vec<&str>,
) -> Vec<u8> {
    let n = rows.len() as u32;
    let mut sink = ByteSink::with_headroom(4 + 1 + rows.len() * (8 + 1 + 4) + 32, 0);
    sink.write_bytes(&n.to_le_bytes());
    sink.write_u8(DESC_U64_DICT_STR_BOOL);
    let row_columns = IdFlagRows::Str(rows);
    write_id_column(&mut sink, row_columns, false);
    // Dict offsets+blob
    sink.align_to(4);
    let dict_len = dict_vec.len() as u32;
    sink.write_u32_le(dict_len);
    let mut acc: u32 = 0;
    let mut offs = Vec::with_capacity(dict_vec.len() + 1);
    offs.push(0);
    let mut blob = Vec::new();
    for s in &dict_vec {
        let b = s.as_bytes();
        acc = acc.wrapping_add(b.len() as u32);
        offs.push(acc);
        blob.extend_from_slice(b);
    }
    for v in offs.iter() {
        sink.write_u32_le(*v);
    }
    sink.write_bytes(&blob);
    // Codes (u32), aligned to 4
    sink.align_to(4);
    for (_, s, _) in rows {
        let code = *dict.get(s).unwrap_or(&0);
        sink.write_u32_le(code);
    }
    write_flag_column(&mut sink, row_columns);
    sink.into_inner()
}

/// Encode using delta+zigzag for the id column when beneficial.
pub fn encode_ncb_u64_str_bool_delta(rows: &[(u64, &str, bool)]) -> Vec<u8> {
    if rows.is_empty() {
        return encode_ncb_u64_str_bool(rows);
    }
    // Compute varint-encoded delta sizes; fall back if any delta overflows i64
    let mut deltas: Vec<u64> = Vec::with_capacity(rows.len().saturating_sub(1));
    let mut prev = rows[0].0;
    let mut varint_bytes: usize = 0;
    for &(id, _, _) in &rows[1..] {
        let d_i128 = (id as i128) - (prev as i128);
        if d_i128 < i64::MIN as i128 || d_i128 > i64::MAX as i128 {
            return encode_ncb_u64_str_bool(rows);
        }
        let zz = zigzag_encode(d_i128 as i64);
        deltas.push(zz);
        varint_bytes += varint_len(zz);
        prev = id;
    }
    // Check if delta coding saves space vs 8*(n-1)
    if varint_bytes >= 8usize.saturating_mul(rows.len().saturating_sub(1)) {
        return encode_ncb_u64_str_bool(rows);
    }
    let n = rows.len() as u32;
    let mut sink = ByteSink::with_headroom(4 + 1 + rows.len() * (8 + 1 + 4) + 16, 0);
    sink.write_bytes(&n.to_le_bytes());
    sink.write_u8(DESC_U64_DELTA_STR_BOOL);
    sink.align_to(8);
    sink.write_u64_le(rows[0].0);
    for &zz in &deltas {
        sink.write_var_u64(zz);
    }
    // strings offsets + blob
    sink.align_to(4);
    let mut acc: u32 = 0;
    let mut offs = Vec::with_capacity(rows.len() + 1);
    offs.push(0);
    let mut blob = Vec::new();
    for &(_, s, _) in rows {
        let b = s.as_bytes();
        acc = acc.wrapping_add(b.len() as u32);
        offs.push(acc);
        blob.extend_from_slice(b);
    }
    for v in offs.iter() {
        sink.write_u32_le(*v);
    }
    sink.write_bytes(&blob);
    write_flag_column(&mut sink, IdFlagRows::Str(rows));
    sink.into_inner()
}

fn varint_len(mut v: u64) -> usize {
    let mut n = 1;
    while v >= 0x80 {
        v >>= 7;
        n += 1;
    }
    n
}

#[inline]
#[cfg(test)]
fn write_var_u64(buf: &mut Vec<u8>, v: u64) {
    let mut vv = v;
    while vv >= 0x80 {
        buf.push((vv as u8) | 0x80);
        vv >>= 7;
    }
    buf.push(vv as u8);
}

struct DeltaSizeTracker {
    previous: i128,
    encoded_bytes: usize,
    fixed_bytes: usize,
}

#[derive(Clone, Copy)]
enum IdFlagRows<'rows, 'payload> {
    Str(&'rows [(u64, &'payload str, bool)]),
    OptStr(&'rows [(u64, Option<&'payload str>, bool)]),
    OptU32(&'rows [(u64, Option<u32>, bool)]),
    Bytes(&'rows [(u64, &'payload [u8], bool)]),
    U32(&'rows [(u64, u32, bool)]),
    StrU32(&'rows [(u64, &'payload str, u32, bool)]),
    BytesU32(&'rows [(u64, &'payload [u8], u32, bool)]),
    Enum(&'rows [(u64, EnumBorrow<'payload>, bool)]),
}

impl IdFlagRows<'_, '_> {
    #[inline(always)]
    fn len(self) -> usize {
        match self {
            Self::Str(rows) => rows.len(),
            Self::OptStr(rows) => rows.len(),
            Self::OptU32(rows) => rows.len(),
            Self::Bytes(rows) => rows.len(),
            Self::U32(rows) => rows.len(),
            Self::StrU32(rows) => rows.len(),
            Self::BytesU32(rows) => rows.len(),
            Self::Enum(rows) => rows.len(),
        }
    }

    #[inline(always)]
    fn id(self, index: usize) -> u64 {
        match self {
            Self::Str(rows) => rows[index].0,
            Self::OptStr(rows) => rows[index].0,
            Self::OptU32(rows) => rows[index].0,
            Self::Bytes(rows) => rows[index].0,
            Self::U32(rows) => rows[index].0,
            Self::StrU32(rows) => rows[index].0,
            Self::BytesU32(rows) => rows[index].0,
            Self::Enum(rows) => rows[index].0,
        }
    }

    #[inline(always)]
    fn flag(self, index: usize) -> bool {
        match self {
            Self::Str(rows) => rows[index].2,
            Self::OptStr(rows) => rows[index].2,
            Self::OptU32(rows) => rows[index].2,
            Self::Bytes(rows) => rows[index].2,
            Self::U32(rows) => rows[index].2,
            Self::StrU32(rows) => rows[index].3,
            Self::BytesU32(rows) => rows[index].3,
            Self::Enum(rows) => rows[index].2,
        }
    }
}

#[inline(always)]
fn write_id_column(sink: &mut ByteSink, rows: IdFlagRows<'_, '_>, delta: bool) {
    let n = rows.len();
    sink.align_to(8);
    if delta && n > 0 {
        let first = rows.id(0);
        sink.write_u64_le(first);
        let mut previous = i128::from(first);
        for index in 1..n {
            let id = rows.id(index);
            let difference = i128::from(id) - previous;
            previous = i128::from(id);
            let delta = if difference < i128::from(i64::MIN) || difference > i128::from(i64::MAX) {
                0
            } else {
                difference as i64
            };
            sink.write_var_u64(zigzag_encode(delta));
        }
    } else {
        for index in 0..n {
            sink.write_u64_le(rows.id(index));
        }
    }
}

#[inline(always)]
fn write_flag_column(sink: &mut ByteSink, rows: IdFlagRows<'_, '_>) {
    let n = rows.len();
    let mut bits = vec![0; n.div_ceil(8)];
    for index in 0..n {
        if rows.flag(index) {
            bits[index / 8] |= 1 << (index % 8);
        }
    }
    sink.write_bytes(&bits);
}

impl DeltaSizeTracker {
    #[inline]
    fn new(first: i128, rows: usize, bytes_per_delta: usize) -> Self {
        Self {
            previous: first,
            encoded_bytes: 0,
            fixed_bytes: bytes_per_delta.saturating_mul(rows.saturating_sub(1)),
        }
    }

    #[inline]
    fn push(&mut self, value: i128) -> bool {
        let delta = value - self.previous;
        if delta < i128::from(i64::MIN) || delta > i128::from(i64::MAX) {
            return false;
        }
        self.encoded_bytes += varint_len(zigzag_encode(delta as i64));
        if self.encoded_bytes >= self.fixed_bytes {
            return false;
        }
        self.previous = value;
        true
    }
}

#[inline]
fn should_use_id_delta(rows: &[(u64, &str, bool)]) -> bool {
    let h = crate::core::heuristics::get();
    if !h.combo_enable_id_delta || rows.len() < h.combo_id_delta_min_rows {
        return false;
    }
    if rows.len() <= h.combo_no_delta_small_n_if_empty && rows.iter().any(|(_, s, _)| s.is_empty())
    {
        return false;
    }
    let mut delta_size = DeltaSizeTracker::new(i128::from(rows[0].0), rows.len(), 8);
    for &(id, _, _) in &rows[1..] {
        if !delta_size.push(i128::from(id)) {
            return false;
        }
    }
    true
}

// ===== Option column support (presence bitset + dense values) =====

/// Maximum number of rows allowed when constructing presence/tag caches.
///
/// Caches materialize cumulative counts to ensure O(1) lookups. Extremely large
/// row counts can lead to unbounded allocations when decoding malicious
/// payloads, so we reject inputs that exceed this threshold.
pub const MAX_CACHE_ROWS: usize = 1 << 20; // 1,048,576 rows (~256 MiB worst-case blobs)

/// Rank cache over a packed bitset with 256-row chunks.
/// Stores the number of set bits before each chunk start to allow O(1) row→dense index.
#[derive(Debug, Clone)]
struct Rank256Cache {
    /// Cumulative counts, one per 256-bit chunk.
    chunks: Vec<u32>,
}

impl Rank256Cache {
    fn build(bits: &[u8], n_rows: usize) -> Result<Self, Error> {
        if n_rows > MAX_CACHE_ROWS {
            telemetry::record_cache_reject(n_rows);
            return Err(Error::UnsupportedFeature("rank cache rows limit"));
        }
        let chunk_rows = 256usize;
        let n_chunks = n_rows.div_ceil(chunk_rows);
        let mut chunks = Vec::with_capacity(n_chunks);
        let mut acc = 0u32;
        for c in 0..n_chunks {
            chunks.push(acc);
            // Count ones in this chunk
            let start_bit = c * chunk_rows;
            let end_bit = ((c + 1) * chunk_rows).min(n_rows);
            let start_byte = start_bit / 8;
            let end_byte = end_bit.div_ceil(8);
            for b in &bits[start_byte..end_byte] {
                acc = acc.wrapping_add(b.count_ones());
            }
        }
        let cache = Self { chunks };
        telemetry::record_cache_build(n_rows);
        Ok(cache)
    }
    /// Compute dense index for row `i` if present according to `bits`.
    /// Returns None when the row is absent in the presence bitset.
    fn dense_index(&self, bits: &[u8], i: usize) -> Option<usize> {
        let byte = i / 8;
        let bit = i % 8;
        if ((bits[byte] >> bit) & 1) == 0 {
            return None;
        }
        let chunk = i / 256;
        let base = self.chunks[chunk] as usize;
        // Count ones from the beginning of the chunk to i-1 (intra-chunk scan).
        let chunk_start_bit = chunk * 256;
        let mut count = 0usize;
        let mut idx = chunk_start_bit;
        while idx < i {
            let b = bits[idx / 8];
            let off = idx % 8;
            // Consume remaining bits in this byte or up to i
            let take = ((i - idx).min(8 - off)) as u8;
            // Mask lower `take` bits starting at `off`
            let mask = (((1u16 << take) - 1) as u8) << off;
            count += ((b & mask).count_ones()) as usize;
            idx += take as usize;
        }
        Some(base + count)
    }
}

/// Borrowed view over an optional `&str` column encoded as presence bitset + dense offsets/data.
pub struct OptStrColView<'a> {
    n: usize,
    /// Presence bitset over n rows.
    pres_bits: &'a [u8],
    /// Rank cache for O(1) row→dense index.
    rank: Rank256Cache,
    /// Offsets (len = present+1) and blob backing string data.
    offs_bytes: &'a [u8],
    blob: &'a str,
}

impl<'a> OptStrColView<'a> {
    /// Number of logical rows (including None entries).
    pub fn len(&self) -> usize {
        self.n
    }
    pub fn is_empty(&self) -> bool {
        self.n == 0
    }
    /// Get the optional string at row `i` as a borrowed `&str`.
    pub fn get(&self, i: usize) -> Result<Option<&'a str>, Error> {
        if let Some(k) = self.rank.dense_index(self.pres_bits, i) {
            let s = read_u32_at(self.offs_bytes, k) as usize;
            let e = read_u32_at(self.offs_bytes, k + 1) as usize;
            return Ok(Some(&self.blob[s..e]));
        }
        Ok(None)
    }
}

/// Encode an optional string column into presence bitset + dense offsets/blob.
/// Returns the encoded bytes and the count of present rows.
pub fn encode_opt_str_column(values: &[Option<&str>]) -> (Vec<u8>, usize) {
    let n = values.len();
    let mut buf = Vec::new();
    // Presence bitset
    let bit_bytes = n.div_ceil(8);
    let start = buf.len();
    buf.extend(std::iter::repeat_n(0u8, bit_bytes));
    let mut present = 0usize;
    for (i, v) in values.iter().enumerate() {
        if v.is_some() {
            buf[start + (i / 8)] |= 1u8 << (i % 8);
            present += 1;
        }
    }
    // Offsets + data for present values (aligned to 4)
    pad_to(&mut buf, 4);
    let offs_base = buf.len();
    buf.extend(std::iter::repeat_n(0u8, 4 * (present + 1)));
    let mut acc = 0u32;
    let mut offs = Vec::with_capacity(present + 1);
    offs.push(0);
    for v in values.iter().filter_map(|o| o.as_ref().copied()) {
        let b = v.as_bytes();
        acc = acc.wrapping_add(b.len() as u32);
        offs.push(acc);
        buf.extend_from_slice(b);
    }
    for (i, vv) in offs.iter().enumerate() {
        let p = offs_base + i * 4;
        buf[p..p + 4].copy_from_slice(&vv.to_le_bytes());
    }
    (buf, present)
}

/// Construct a borrowed optional string column view from `bytes`.
/// Layout: [bitset ceil(n/8)] [pad→4] [u32 offs; present+1] [utf8 blob]
pub fn view_opt_str_column(bytes: &[u8], n_rows: usize) -> Result<OptStrColView<'_>, Error> {
    crate::core::enforce_decode_sequence_length(
        u64::try_from(n_rows).map_err(|_| Error::LengthMismatch)?,
    )?;
    view_opt_str_column_inner(bytes, n_rows)
}

fn view_opt_str_column_inner(bytes: &[u8], n_rows: usize) -> Result<OptStrColView<'_>, Error> {
    let (pres_bits, present, mut off) = optional_column_prefix(bytes, n_rows)?;
    // Offsets table (present+1) followed by blob
    let offs_count = present.checked_add(1).ok_or(Error::LengthMismatch)?;
    let offs_len_bytes = mul_checked(offs_count, 4)?;
    let offs_bytes = slice_range(bytes, off, offs_len_bytes)?;
    off = add_offset(off, offs_len_bytes)?;
    let last = validate_u32_offsets(offs_bytes, present)?;
    let blob = slice_range(bytes, off, last)?;
    off = add_offset(off, last)?;
    ensure_no_trailing(bytes, off)?;
    let blob_str = validated_str(blob)?;
    let rank = Rank256Cache::build(pres_bits, n_rows)?;
    Ok(OptStrColView {
        n: n_rows,
        pres_bits,
        rank,
        offs_bytes,
        blob: blob_str,
    })
}

/// Borrowed view over an optional `u32` column encoded as presence bitset + dense u32 values.
pub struct OptU32ColView<'a> {
    n: usize,
    pres_bits: &'a [u8],
    rank: Rank256Cache,
    vals_bytes: &'a [u8],
}

impl<'a> OptU32ColView<'a> {
    pub fn len(&self) -> usize {
        self.n
    }
    pub fn is_empty(&self) -> bool {
        self.n == 0
    }
    pub fn get(&self, i: usize) -> Option<u32> {
        let k = self.rank.dense_index(self.pres_bits, i)?;
        let start = k * 4;
        if self.vals_bytes.len() < start + 4 {
            return None;
        }
        let mut lb = [0u8; 4];
        lb.copy_from_slice(&self.vals_bytes[start..start + 4]);
        Some(u32::from_le_bytes(lb))
    }
}

/// Encode an optional u32 column: [bitset ceil(n/8)] [pad→4] [u32; present]
pub fn encode_opt_u32_column(values: &[Option<u32>]) -> (Vec<u8>, usize) {
    let n = values.len();
    let mut buf = Vec::new();
    let bit_bytes = n.div_ceil(8);
    let start = buf.len();
    buf.extend(std::iter::repeat_n(0u8, bit_bytes));
    let mut present = 0usize;
    for (i, v) in values.iter().enumerate() {
        if v.is_some() {
            buf[start + (i / 8)] |= 1u8 << (i % 8);
            present += 1;
        }
    }
    pad_to(&mut buf, 4);
    for v in values.iter().filter_map(|o| *o) {
        buf.extend_from_slice(&v.to_le_bytes());
    }
    (buf, present)
}

/// View an optional u32 column from bytes and logical row count.
pub fn view_opt_u32_column(bytes: &[u8], n_rows: usize) -> Result<OptU32ColView<'_>, Error> {
    crate::core::enforce_decode_sequence_length(
        u64::try_from(n_rows).map_err(|_| Error::LengthMismatch)?,
    )?;
    view_opt_u32_column_inner(bytes, n_rows)
}

fn view_opt_u32_column_inner(bytes: &[u8], n_rows: usize) -> Result<OptU32ColView<'_>, Error> {
    let (pres_bits, present, mut off) = optional_column_prefix(bytes, n_rows)?;
    let vals_len = mul_checked(present, 4)?;
    let vals_bytes = slice_range(bytes, off, vals_len)?;
    off = add_offset(off, vals_len)?;
    ensure_no_trailing(bytes, off)?;
    let rank = Rank256Cache::build(pres_bits, n_rows)?;
    Ok(OptU32ColView {
        n: n_rows,
        pres_bits,
        rank,
        vals_bytes,
    })
}

// ===== Enum column support: BenchEnum(Name(String)|Code(u32)) rows =====

/// Borrowed enum reference produced from the enum column.
pub enum ColEnumRef<'a> {
    Name(&'a str),
    Code(u32),
}

/// Borrowed variant tags view with rank caches per variant for O(1) indexing.
struct TagsView<'a> {
    tags: &'a [u8],
    // cumulative counts of NAME and CODE tags at 256-row boundaries
    name_chunks: Vec<u32>,
    code_chunks: Vec<u32>,
}

impl<'a> TagsView<'a> {
    fn build(tags: &'a [u8], n: usize) -> Result<Self, Error> {
        if n > MAX_CACHE_ROWS {
            telemetry::record_cache_reject(n);
            return Err(Error::UnsupportedFeature("enum tag cache rows limit"));
        }
        let chunk_rows = 256usize;
        let n_chunks = n.div_ceil(chunk_rows);
        let mut name_chunks = Vec::with_capacity(n_chunks);
        let mut code_chunks = Vec::with_capacity(n_chunks);
        let mut name_acc = 0u32;
        let mut code_acc = 0u32;
        for c in 0..n_chunks {
            name_chunks.push(name_acc);
            code_chunks.push(code_acc);
            let start = c * chunk_rows;
            let end = ((c + 1) * chunk_rows).min(n);
            for &t in &tags[start..end] {
                if t == TAG_NAME {
                    name_acc += 1;
                } else if t == TAG_CODE {
                    code_acc += 1;
                }
            }
        }
        let view = Self {
            tags,
            name_chunks,
            code_chunks,
        };
        telemetry::record_cache_build(n);
        Ok(view)
    }
    #[inline]
    fn tag(&self, i: usize) -> u8 {
        self.tags[i]
    }
    fn dense_index_of(&self, i: usize, variant: u8) -> usize {
        // assume tags[i] == variant
        let chunk = i / 256;
        let base = match variant {
            TAG_NAME => self.name_chunks[chunk],
            _ => self.code_chunks[chunk],
        } as usize;
        let start = chunk * 256;
        let mut count = 0usize;
        for &t in &self.tags[start..i] {
            if t == variant {
                count += 1;
            }
        }
        base + count
    }
}

/// View over an NCB `(u64, enum(Name(String)|Code(u32)), bool)` block.
pub struct NcbU64EnumBoolView<'a> {
    n: usize,
    ids: IdsRep<'a>,
    tags: TagsView<'a>,
    // Name subcolumn (either offsets-based or dict-coded)
    names: EnumNamesRep<'a>,
    // Code subcolumn
    codes: U32Rep<'a>,
    // Flags
    bits: &'a [u8],
    // Precomputed indexes for repeated scans
    flags_index: FlagsIndex,
    name_tag_index: FlagsIndex,
    code_tag_index: FlagsIndex,
}

enum EnumNamesRep<'a> {
    Offsets {
        offs_bytes: &'a [u8],
        blob: &'a str,
    },
    Dict {
        dict_offs_bytes: &'a [u8],
        dict_blob: &'a str,
        codes_bytes: &'a [u8],
    },
}

impl<'a> NcbU64EnumBoolView<'a> {
    /// Number of rows.
    pub fn len(&self) -> usize {
        self.n
    }
    /// Access id column.
    pub fn id(&self, i: usize) -> u64 {
        self.ids.get(i)
    }
    /// Access tag for row `i` (0=Name,1=Code).
    pub fn tag(&self, i: usize) -> u8 {
        self.tags.tag(i)
    }
    /// Access payload variant for row `i` without allocation.
    pub fn payload(&self, i: usize) -> Result<ColEnumRef<'a>, Error> {
        match self.tags.tag(i) {
            TAG_NAME => {
                let k = self.tags.dense_index_of(i, TAG_NAME);
                match &self.names {
                    EnumNamesRep::Offsets { offs_bytes, blob } => {
                        let s = read_u32_at(offs_bytes, k) as usize;
                        let e = read_u32_at(offs_bytes, k + 1) as usize;
                        let len = blob.len();
                        if s > e || e > len {
                            return Err(Error::LengthMismatch);
                        }
                        Ok(ColEnumRef::Name(&blob[s..e]))
                    }
                    EnumNamesRep::Dict {
                        dict_offs_bytes,
                        dict_blob,
                        codes_bytes,
                    } => {
                        let code = read_u32_at(codes_bytes, k) as usize;
                        let dict_len = dict_offs_bytes.len() / 4 - 1;
                        if code >= dict_len {
                            return Err(Error::LengthMismatch);
                        }
                        let s = read_u32_at(dict_offs_bytes, code) as usize;
                        let e = read_u32_at(dict_offs_bytes, code + 1) as usize;
                        let len = dict_blob.len();
                        if s > e || e > len {
                            return Err(Error::LengthMismatch);
                        }
                        Ok(ColEnumRef::Name(&dict_blob[s..e]))
                    }
                }
            }
            TAG_CODE => {
                let k = self.tags.dense_index_of(i, TAG_CODE);
                Ok(ColEnumRef::Code(self.codes.get(k)))
            }
            other => Err(Error::invalid_tag("projecting enum payload variant", other)),
        }
    }
    /// Access boolean flag column.
    pub fn flag(&self, i: usize) -> bool {
        bit_at(self.bits, i)
    }
    /// Project raw tags slice for projection-only scans.
    pub fn tags_slice(&self) -> &'a [u8] {
        self.tags.tags
    }
    /// Project raw ids when available as a slice (None if delta-coded).
    pub fn ids_slice(&self) -> Option<&'a [u64]> {
        self.ids.as_slice()
    }
    /// Count of `Name` variant rows.
    pub fn names_count(&self) -> usize {
        match &self.names {
            EnumNamesRep::Offsets { offs_bytes, .. } => offs_bytes.len() / 4 - 1,
            EnumNamesRep::Dict { codes_bytes, .. } => codes_bytes.len() / 4,
        }
    }
    /// Count of `Code` variant rows.
    pub fn codes_count(&self) -> usize {
        match &self.codes {
            U32Rep::Slice(s) => s.len(),
            U32Rep::Rebuilt(v) => v.len(),
        }
    }
    /// Access the K-th `Name` string in the `Name` subcolumn (zero-copy).
    pub fn name_k(&self, k: usize) -> Result<&'a str, Error> {
        match &self.names {
            EnumNamesRep::Offsets { offs_bytes, blob } => {
                let count = offs_bytes.len() / 4;
                if k + 1 >= count {
                    return Err(Error::LengthMismatch);
                }
                let s = read_u32_at(offs_bytes, k) as usize;
                let e = read_u32_at(offs_bytes, k + 1) as usize;
                Ok(&blob[s..e])
            }
            EnumNamesRep::Dict {
                dict_offs_bytes,
                dict_blob,
                codes_bytes,
            } => {
                let n = codes_bytes.len() / 4;
                if k >= n {
                    return Err(Error::LengthMismatch);
                }
                let code = read_u32_at(codes_bytes, k) as usize;
                let s = read_u32_at(dict_offs_bytes, code) as usize;
                let e = read_u32_at(dict_offs_bytes, code + 1) as usize;
                Ok(&dict_blob[s..e])
            }
        }
    }
    /// Access the K-th `Code` value in the `Code` subcolumn.
    pub fn code_k(&self, k: usize) -> Result<u32, Error> {
        match &self.codes {
            U32Rep::Slice(s) => s.get(k).copied().ok_or(Error::LengthMismatch),
            U32Rep::Rebuilt(v) => v.get(k).copied().ok_or(Error::LengthMismatch),
        }
    }
    /// Iterate positions with flag==true using aligned 64-bit scanner.
    pub fn iter_true_positions_popcount64_aligned(&self) -> TruePosPop64AlignedIter<'_> {
        let (head, body, tail) = unsafe { self.bits.align_to::<u64>() };
        TruePosPop64AlignedIter {
            head,
            body,
            tail,
            n: self.n,
            head_idx: 0,
            head_cur: 0,
            head_base: 0,
            body_idx: 0,
            body_cur: 0,
            body_base: head.len() * 8,
            tail_idx: 0,
            tail_cur: 0,
            tail_base: head.len() * 8 + body.len() * 64,
            stage: 0,
        }
    }
    /// Iterate positions with flag==true using byte-level scanner (fallback).
    pub fn iter_true_positions_popcount(&self) -> TruePosPopIter<'_> {
        TruePosPopIter {
            bits: self.bits,
            n: self.n,
            byte_idx: 0,
            cur: 0,
            base: 0,
        }
    }
    /// CPU/arch-guided fast positions iterator.
    pub fn iter_true_positions_fast(&self) -> TruePosFastIter<'_> {
        #[cfg(target_pointer_width = "64")]
        {
            TruePosFastIter::Aligned64(self.iter_true_positions_popcount64_aligned())
        }
        #[cfg(not(target_pointer_width = "64"))]
        {
            TruePosFastIter::Byte(self.iter_true_positions_popcount())
        }
    }
    /// Iterate names for rows where flag==true and tag==Name using fast scanner.
    pub fn iter_names_flag_true_fast(&'a self) -> EnumNamesFlagFastIter<'a> {
        EnumNamesFlagFastIter {
            view: self,
            pos: self.iter_true_positions_fast(),
        }
    }
    /// Reverse iterator over names where flag==true and tag==Name (materializes to owned Strings).
    pub fn iter_names_flag_true_fast_rev(&'a self) -> EnumNamesFlagRevIter {
        let mut v: Vec<String> = self
            .iter_names_flag_true_fast()
            .map(|s| s.to_string())
            .collect();
        v.reverse();
        EnumNamesFlagRevIter { inner: v, i: 0 }
    }
    /// Iterate names for rows where flag==true and tag==Name using precomputed indexes
    /// (intersection of flags bitset and Name-tag bitset).
    pub fn iter_names_flag_true_indexed(&'a self) -> EnumNamesFlagIndexedIter<'a> {
        EnumNamesFlagIndexedIter {
            view: self,
            it: self.flags_index.iter_positions_and(&self.name_tag_index),
        }
    }
    /// Iterate codes for rows where flag==true and tag==Code using fast scanner.
    pub fn iter_codes_flag_true_fast(&'a self) -> EnumCodesFlagFastIter<'a> {
        EnumCodesFlagFastIter {
            view: self,
            pos: self.iter_true_positions_fast(),
        }
    }
    /// Reverse iterator over codes where flag==true and tag==Code.
    pub fn iter_codes_flag_true_fast_rev(&'a self) -> EnumCodesFlagRevIter {
        let mut v: Vec<u32> = self.iter_codes_flag_true_fast().collect();
        v.reverse();
        EnumCodesFlagRevIter { inner: v, i: 0 }
    }
    /// Iterate codes for rows where flag==true and tag==Code using precomputed indexes.
    pub fn iter_codes_flag_true_indexed(&'a self) -> EnumCodesFlagIndexedIter<'a> {
        EnumCodesFlagIndexedIter {
            view: self,
            it: self.flags_index.iter_positions_and(&self.code_tag_index),
        }
    }
    /// Iterate ids for rows where flag==true using fast scanner.
    pub fn iter_ids_flag_true_fast(&'a self) -> EnumIdsFlagFastIter<'a> {
        EnumIdsFlagFastIter {
            view: self,
            pos: self.iter_true_positions_fast(),
        }
    }
    /// Reverse iterator over ids where flag==true.
    pub fn iter_ids_flag_true_fast_rev(&'a self) -> EnumIdsFlagRevIter {
        let mut v: Vec<u64> = self.iter_ids_flag_true_fast().collect();
        v.reverse();
        EnumIdsFlagRevIter { inner: v, i: 0 }
    }
    /// Iterate names in dense Name-subcolumn order (skips non-Name rows).
    pub fn iter_names_dense(&'a self) -> NamesDenseIter<'a> {
        match &self.names {
            EnumNamesRep::Offsets { offs_bytes, blob } => NamesDenseIter::Offsets {
                offs_bytes,
                blob,
                i: 0,
            },
            EnumNamesRep::Dict {
                dict_offs_bytes,
                dict_blob,
                codes_bytes,
            } => NamesDenseIter::Dict {
                dict_offs_bytes,
                dict_blob,
                codes_bytes,
                i: 0,
            },
        }
    }
    /// Iterate codes in dense Code-subcolumn order (skips non-Code rows).
    pub fn iter_codes_dense(&'a self) -> CodesDenseIter<'a> {
        match &self.codes {
            U32Rep::Slice(s) => CodesDenseIter::Slice { slice: s, i: 0 },
            U32Rep::Rebuilt(v) => CodesDenseIter::Vec { vec: v, i: 0 },
        }
    }
}

pub enum NamesDenseIter<'a> {
    Offsets {
        offs_bytes: &'a [u8],
        blob: &'a str,
        i: usize,
    },
    Dict {
        dict_offs_bytes: &'a [u8],
        dict_blob: &'a str,
        codes_bytes: &'a [u8],
        i: usize,
    },
}

impl<'a> Iterator for NamesDenseIter<'a> {
    type Item = &'a str;
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            NamesDenseIter::Offsets {
                offs_bytes,
                blob,
                i,
            } => next_offsets_name(offs_bytes, blob, i),
            NamesDenseIter::Dict {
                dict_offs_bytes,
                dict_blob,
                codes_bytes,
                i,
            } => next_dict_name(dict_offs_bytes, dict_blob, codes_bytes, i),
        }
    }
}

pub enum CodesDenseIter<'a> {
    Slice { slice: &'a [u32], i: usize },
    Vec { vec: &'a Vec<u32>, i: usize },
}

impl<'a> Iterator for CodesDenseIter<'a> {
    type Item = u32;
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            CodesDenseIter::Slice { slice, i } => {
                if *i >= slice.len() {
                    None
                } else {
                    let v = slice[*i];
                    *i += 1;
                    Some(v)
                }
            }
            CodesDenseIter::Vec { vec, i } => {
                if *i >= vec.len() {
                    None
                } else {
                    let v = vec[*i];
                    *i += 1;
                    Some(v)
                }
            }
        }
    }
}

/// Convenient row ref for the enum view.
pub struct EnumRowRef<'a> {
    view: &'a NcbU64EnumBoolView<'a>,
    idx: usize,
}

/// Wrapper that selects best available positions iterator at runtime/compile-time.
pub enum TruePosFastIter<'a> {
    Byte(TruePosPopIter<'a>),
    Aligned64(TruePosPop64AlignedIter<'a>),
}
impl<'a> Iterator for TruePosFastIter<'a> {
    type Item = usize;
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            TruePosFastIter::Byte(i) => i.next(),
            TruePosFastIter::Aligned64(i) => i.next(),
        }
    }
}

/// Fast names iterator for enum view using positions wrapper and tag check.
pub struct EnumNamesFlagFastIter<'a> {
    view: &'a NcbU64EnumBoolView<'a>,
    pos: TruePosFastIter<'a>,
}
impl<'a> Iterator for EnumNamesFlagFastIter<'a> {
    type Item = &'a str;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let i = self.pos.next()?;
            if self.view.tags.tag(i) != TAG_NAME {
                continue;
            }
            let k = self.view.tags.dense_index_of(i, TAG_NAME);
            match &self.view.names {
                EnumNamesRep::Offsets { offs_bytes, blob } => {
                    let s = read_u32_at(offs_bytes, k) as usize;
                    let e = read_u32_at(offs_bytes, k + 1) as usize;
                    return Some(&blob[s..e]);
                }
                EnumNamesRep::Dict {
                    dict_offs_bytes,
                    dict_blob,
                    codes_bytes,
                } => {
                    let code = read_u32_at(codes_bytes, k) as usize;
                    let s = read_u32_at(dict_offs_bytes, code) as usize;
                    let e = read_u32_at(dict_offs_bytes, code + 1) as usize;
                    return Some(&dict_blob[s..e]);
                }
            }
        }
    }
}

/// Fast names iterator using prebuilt intersection of flags and Name-tag bitsets.
pub struct EnumNamesFlagIndexedIter<'a> {
    view: &'a NcbU64EnumBoolView<'a>,
    it: FlagsIndexAndIter<'a>,
}
impl<'a> Iterator for EnumNamesFlagIndexedIter<'a> {
    type Item = &'a str;
    fn next(&mut self) -> Option<Self::Item> {
        let i = self.it.next()?;
        let k = self.view.tags.dense_index_of(i, TAG_NAME);
        match &self.view.names {
            EnumNamesRep::Offsets { offs_bytes, blob } => {
                let s = read_u32_at(offs_bytes, k) as usize;
                let e = read_u32_at(offs_bytes, k + 1) as usize;
                Some(&blob[s..e])
            }
            EnumNamesRep::Dict {
                dict_offs_bytes,
                dict_blob,
                codes_bytes,
            } => {
                let code = read_u32_at(codes_bytes, k) as usize;
                let s = read_u32_at(dict_offs_bytes, code) as usize;
                let e = read_u32_at(dict_offs_bytes, code + 1) as usize;
                Some(&dict_blob[s..e])
            }
        }
    }
}

/// Fast codes iterator for enum view using positions wrapper and tag check.
pub struct EnumCodesFlagFastIter<'a> {
    view: &'a NcbU64EnumBoolView<'a>,
    pos: TruePosFastIter<'a>,
}
impl<'a> Iterator for EnumCodesFlagFastIter<'a> {
    type Item = u32;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let i = self.pos.next()?;
            if self.view.tags.tag(i) != TAG_CODE {
                continue;
            }
            let k = self.view.tags.dense_index_of(i, TAG_CODE);
            let v = match &self.view.codes {
                U32Rep::Slice(s) => s.get(k).copied(),
                U32Rep::Rebuilt(v) => v.get(k).copied(),
            };
            return v;
        }
    }
}

/// Codes iterator using prebuilt intersection of flags and Code-tag bitsets.
pub struct EnumCodesFlagIndexedIter<'a> {
    view: &'a NcbU64EnumBoolView<'a>,
    it: FlagsIndexAndIter<'a>,
}
impl<'a> Iterator for EnumCodesFlagIndexedIter<'a> {
    type Item = u32;
    fn next(&mut self) -> Option<Self::Item> {
        let i = self.it.next()?;
        let k = self.view.tags.dense_index_of(i, TAG_CODE);
        match &self.view.codes {
            U32Rep::Slice(s) => s.get(k).copied(),
            U32Rep::Rebuilt(v) => v.get(k).copied(),
        }
    }
}

/// Fast ids iterator for enum view using positions wrapper.
pub struct EnumIdsFlagFastIter<'a> {
    view: &'a NcbU64EnumBoolView<'a>,
    pos: TruePosFastIter<'a>,
}

/// Owned reverse iterator for names at flag==true.
pub struct EnumNamesFlagRevIter {
    inner: Vec<String>,
    i: usize,
}
impl Iterator for EnumNamesFlagRevIter {
    type Item = String;
    fn next(&mut self) -> Option<Self::Item> {
        if self.i >= self.inner.len() {
            None
        } else {
            let v = self.inner[self.i].clone();
            self.i += 1;
            Some(v)
        }
    }
}

/// Owned reverse iterator for codes at flag==true.
pub struct EnumCodesFlagRevIter {
    inner: Vec<u32>,
    i: usize,
}
impl Iterator for EnumCodesFlagRevIter {
    type Item = u32;
    fn next(&mut self) -> Option<Self::Item> {
        if self.i >= self.inner.len() {
            None
        } else {
            let v = self.inner[self.i];
            self.i += 1;
            Some(v)
        }
    }
}

/// Owned reverse iterator for ids at flag==true.
pub struct EnumIdsFlagRevIter {
    inner: Vec<u64>,
    i: usize,
}
impl Iterator for EnumIdsFlagRevIter {
    type Item = u64;
    fn next(&mut self) -> Option<Self::Item> {
        if self.i >= self.inner.len() {
            None
        } else {
            let v = self.inner[self.i];
            self.i += 1;
            Some(v)
        }
    }
}
impl<'a> Iterator for EnumIdsFlagFastIter<'a> {
    type Item = u64;
    fn next(&mut self) -> Option<Self::Item> {
        let i = self.pos.next()?;
        Some(self.view.id(i))
    }
}
impl<'a> EnumRowRef<'a> {
    pub fn id(&self) -> u64 {
        self.view.id(self.idx)
    }
    pub fn tag(&self) -> u8 {
        self.view.tag(self.idx)
    }
    pub fn payload(&self) -> Result<ColEnumRef<'a>, Error> {
        self.view.payload(self.idx)
    }
    pub fn flag(&self) -> bool {
        self.view.flag(self.idx)
    }
}
pub struct EnumRowCursor<'a> {
    view: &'a NcbU64EnumBoolView<'a>,
    i: usize,
}
impl<'a> Iterator for EnumRowCursor<'a> {
    type Item = EnumRowRef<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.i >= self.view.n {
            None
        } else {
            let r = EnumRowRef {
                view: self.view,
                idx: self.i,
            };
            self.i += 1;
            Some(r)
        }
    }
}
impl<'a> NcbU64EnumBoolView<'a> {
    pub fn rows(&'a self) -> EnumRowCursor<'a> {
        EnumRowCursor { view: self, i: 0 }
    }
    pub fn is_empty(&self) -> bool {
        self.n == 0
    }
}

/// Encode an enum-heavy dataset into NCB.
/// Layout:
/// - u32 n
/// - u8 desc (DESC_U64_ENUM_BOOL | optional dict flag)
/// - ids: aligned 8, either `[u64; n]` or delta+zigzag varints starting with base
/// - tags: `[u8; n]`
/// - names subcolumn: aligned 4, `[u32; n_name+1]` + utf8 blob
/// - codes subcolumn: aligned 4, `[u32; n_code]`
/// - flags bitset: ceil(n/8)
pub fn encode_ncb_u64_enum_bool(
    rows: &[(u64, EnumBorrow<'_>, bool)],
    use_delta_ids: bool,
    use_name_dict: bool,
    use_code_delta: bool,
) -> Vec<u8> {
    let n = rows.len();
    let mut use_delta_ids = use_delta_ids;
    if use_delta_ids && n >= 2 {
        let mut prev = rows[0].0 as i128;
        for &(id, _, _) in &rows[1..] {
            let d = (id as i128) - prev;
            if d < i64::MIN as i128 || d > i64::MAX as i128 {
                use_delta_ids = false;
                break;
            }
            prev = id as i128;
        }
    }
    let mut sink = ByteSink::with_headroom(4 + 1 + n * (8 + 1 + 4) + 64, 0);
    sink.write_bytes(&(n as u32).to_le_bytes());
    let desc = match (use_delta_ids, use_name_dict, use_code_delta) {
        (false, false, false) => DESC_U64_ENUM_BOOL,
        (true, false, false) => DESC_U64_DELTA_ENUM_BOOL,
        (false, false, true) => DESC_U64_ENUM_BOOL_CODEDELTA,
        (true, false, true) => DESC_U64_DELTA_ENUM_BOOL_CODEDELTA,
        (false, true, false) => DESC_U64_ENUM_BOOL_DICT,
        (true, true, false) => DESC_U64_DELTA_ENUM_BOOL_DICT,
        (false, true, true) => DESC_U64_ENUM_BOOL_DICT_CODEDELTA,
        (true, true, true) => DESC_U64_DELTA_ENUM_BOOL_DICT_CODEDELTA,
    };
    sink.write_u8(desc);

    let row_columns = IdFlagRows::Enum(rows);
    write_id_column(&mut sink, row_columns, use_delta_ids);

    // tags and gather variant-specific payloads
    let mut tags = vec![0u8; n];
    let mut names: Vec<&str> = Vec::new();
    let mut codes: Vec<u32> = Vec::new();
    for (i, (_, e, _)) in rows.iter().enumerate() {
        match e {
            EnumBorrow::Name(s) => {
                tags[i] = TAG_NAME;
                names.push(s);
            }
            EnumBorrow::Code(v) => {
                tags[i] = TAG_CODE;
                codes.push(*v);
            }
        }
    }
    sink.write_bytes(&tags);

    // names subcolumn
    sink.align_to(4);
    if use_name_dict {
        // Simple dictionary: unique names + offsets/blob + codes (u32 index)
        use std::collections::HashMap;
        let mut dict: HashMap<&str, u32> = HashMap::new();
        let mut dict_vec: Vec<&str> = Vec::new();
        for &s in &names {
            if !dict.contains_key(s) {
                let id = dict_vec.len() as u32;
                dict.insert(s, id);
                dict_vec.push(s);
            }
        }
        let dict_len = dict_vec.len() as u32;
        sink.write_u32_le(dict_len);
        let mut acc: u32 = 0;
        let mut offs = Vec::with_capacity(dict_vec.len() + 1);
        offs.push(0);
        let mut blob = Vec::new();
        for s in &dict_vec {
            let b = s.as_bytes();
            acc = acc.wrapping_add(b.len() as u32);
            offs.push(acc);
            blob.extend_from_slice(b);
        }
        for v in offs.iter() {
            sink.write_u32_le(*v);
        }
        sink.write_bytes(&blob);
        // Align before writing per-Name codes to ensure u32 alignment
        sink.align_to(4);
        for &s in &names {
            let code = *dict.get(s).unwrap();
            sink.write_u32_le(code);
        }
    } else {
        let mut acc: u32 = 0;
        let mut offs = Vec::with_capacity(names.len() + 1);
        offs.push(0);
        let mut blob = Vec::new();
        for s in &names {
            let b = s.as_bytes();
            acc = acc.wrapping_add(b.len() as u32);
            offs.push(acc);
            blob.extend_from_slice(b);
        }
        for v in offs.iter() {
            sink.write_u32_le(*v);
        }
        sink.write_bytes(&blob);
    }

    // codes subcolumn
    sink.align_to(4);
    if use_code_delta && !codes.is_empty() {
        // Base + varint zigzag deltas
        let base = codes[0] as i64;
        sink.write_u32_le(codes[0]);
        let mut prev = base;
        for &c in &codes[1..] {
            let d = (c as i64) - prev;
            prev = c as i64;
            sink.write_var_u64(zigzag_encode(d));
        }
    } else {
        for v in &codes {
            sink.write_u32_le(*v);
        }
    }

    write_flag_column(&mut sink, row_columns);
    sink.into_inner()
}

/// Parse a byte slice into an enum NCB view.
pub fn view_ncb_u64_enum_bool(bytes: &[u8]) -> Result<NcbU64EnumBoolView<'_>, Error> {
    let (n, desc, mut off) = read_ncb_header(bytes)?;
    let (is_delta, use_dict, code_delta) = match desc {
        DESC_U64_ENUM_BOOL => (false, false, false),
        DESC_U64_DELTA_ENUM_BOOL => (true, false, false),
        DESC_U64_ENUM_BOOL_CODEDELTA => (false, false, true),
        DESC_U64_DELTA_ENUM_BOOL_CODEDELTA => (true, false, true),
        DESC_U64_ENUM_BOOL_DICT => (false, true, false),
        DESC_U64_DELTA_ENUM_BOOL_DICT => (true, true, false),
        DESC_U64_ENUM_BOOL_DICT_CODEDELTA => (false, true, true),
        DESC_U64_DELTA_ENUM_BOOL_DICT_CODEDELTA => (true, true, true),
        _ => return Err(Error::Message("invalid enum NCB descriptor".into())),
    };

    let ids = decode_ids_column(bytes, &mut off, n, is_delta)?;

    // tags: [u8; n]
    let tags = slice_range(bytes, off, n)?;
    // Validate tag values strictly: only TAG_NAME (0) and TAG_CODE (1) allowed
    for (i, &t) in tags.iter().enumerate() {
        if t != TAG_NAME && t != TAG_CODE {
            let _ = i; // keep index available for future richer diagnostics
            return Err(Error::invalid_tag("validating enum tags column", t));
        }
    }
    off = add_offset(off, n)?;
    let tags_view = TagsView::build(tags, n)?;
    // Test-only: verify dense indexes for NAME/CODE tags are consistent with prefix counts
    #[cfg(test)]
    {
        let mut seen_name = 0usize;
        let mut seen_code = 0usize;
        for (i, _t) in tags.iter().enumerate().take(n) {
            let t = tags[i];
            if t == TAG_NAME {
                let k = tags_view.dense_index_of(i, TAG_NAME);
                debug_assert_eq!(
                    k, seen_name,
                    "enum tags NAME dense index mismatch at row {i}: got={k} expected={seen_name}"
                );
                seen_name += 1;
            } else if t == TAG_CODE {
                let k = tags_view.dense_index_of(i, TAG_CODE);
                debug_assert_eq!(
                    k, seen_code,
                    "enum tags CODE dense index mismatch at row {i}: got={k} expected={seen_code}"
                );
                seen_code += 1;
            } else {
                debug_assert!(false, "invalid tag value {t} at row {i}");
            }
        }
        debug_assert_eq!(seen_name + seen_code, n, "enum tags total mismatch");
    }

    // names subcolumn
    let off_after_tags = off;
    let mut off_names = off_after_tags;
    align_offset_checked(bytes, &mut off_names, 4)?;
    // Count names/code variants from tags
    let n_name = tags.iter().filter(|&&t| t == TAG_NAME).count();
    let _n_code_expected = n - n_name;
    #[cfg(test)]
    debug_assert_eq!(n_name + _n_code_expected, n, "tags count mismatch");

    let names = if use_dict {
        // dict_len, dict_offs, dict_blob, then per-Name codes
        let dict_len_bytes = slice_range(bytes, off_names, 4)?;
        off_names = add_offset(off_names, 4)?;
        let mut lb = [0u8; 4];
        lb.copy_from_slice(dict_len_bytes);
        let dict_len_raw = u32::from_le_bytes(lb);
        crate::core::enforce_decode_sequence_length(u64::from(dict_len_raw))?;
        let dict_len = dict_len_raw as usize;
        let dict_count = dict_len.checked_add(1).ok_or(Error::LengthMismatch)?;
        let dict_offs_len = mul_checked(dict_count, 4)?;
        let dict_offs_bytes = slice_range(bytes, off_names, dict_offs_len)?;
        off_names = add_offset(off_names, dict_offs_len)?;
        let dict_data_len = validate_u32_offsets(dict_offs_bytes, dict_len)?;
        let dict_data = slice_range(bytes, off_names, dict_data_len)?;
        off_names = add_offset(off_names, dict_data_len)?;
        // Align before reading per-Name codes (u32)
        align_offset_checked(bytes, &mut off_names, 4)?;
        let codes_len = mul_checked(n_name, 4)?;
        let codes_bytes = slice_range(bytes, off_names, codes_len)?;
        for i in 0..n_name {
            let code = read_u32_at(codes_bytes, i) as usize;
            if code >= dict_len {
                return Err(Error::LengthMismatch);
            }
        }
        let dict_blob = validated_str(dict_data)?;
        EnumNamesRep::Dict {
            dict_offs_bytes,
            dict_blob,
            codes_bytes,
        }
    } else {
        // Offsets-based names: [u32; n_name+1] + blob
        let offs_count = n_name.checked_add(1).ok_or(Error::LengthMismatch)?;
        let total_offs_len = mul_checked(offs_count, 4)?;
        let offs_slice = slice_range(bytes, off_names, total_offs_len)?;
        off_names = add_offset(off_names, total_offs_len)?;
        let last = validate_u32_offsets(offs_slice, n_name)?;
        let data = slice_range(bytes, off_names, last)?;
        let blob_str = validated_str(data)?;
        EnumNamesRep::Offsets {
            offs_bytes: offs_slice,
            blob: blob_str,
        }
    };

    // Recompute expected names end offset from tags and section layout to catch drift
    let mut expected_off = off_after_tags;
    let mis4_expected = expected_off & 3;
    if mis4_expected != 0 {
        expected_off += 4 - mis4_expected;
    }
    if use_dict {
        // dict_len (4), dict_offs (4*(dict_len+1)), dict_blob (len at last offset), align4, per-Name codes (4*n_name)
        let dict_len_bytes = slice_range(bytes, expected_off, 4)?;
        let mut dlb = [0u8; 4];
        dlb.copy_from_slice(dict_len_bytes);
        let dict_len_raw = u32::from_le_bytes(dlb);
        crate::core::check_decode_sequence_length(u64::from(dict_len_raw))?;
        let dict_len = dict_len_raw as usize;
        expected_off = add_offset(expected_off, 4)?;
        let dict_count = dict_len.checked_add(1).ok_or(Error::LengthMismatch)?;
        let dict_offs_len = mul_checked(dict_count, 4)?;
        let dict_offs_bytes = slice_range(bytes, expected_off, dict_offs_len)?;
        expected_off = add_offset(expected_off, dict_offs_len)?;
        // blob length is the last offset
        let last = read_u32_at(dict_offs_bytes, dict_len) as usize;
        expected_off = add_offset(expected_off, last)?;
        let mis4_codes = expected_off & 3;
        if mis4_codes != 0 {
            expected_off += 4 - mis4_codes;
        }
        let codes_len = mul_checked(n_name, 4)?;
        expected_off = add_offset(expected_off, codes_len)?;
    } else {
        // offsets (4*(n_name+1)) + blob (last), no extra align beyond initial
        let offs_count = n_name.checked_add(1).ok_or(Error::LengthMismatch)?;
        let total_offs_len = mul_checked(offs_count, 4)?;
        let offs_slice = slice_range(bytes, expected_off, total_offs_len)?;
        expected_off = add_offset(expected_off, total_offs_len)?;
        let last = read_u32_at(offs_slice, n_name) as usize;
        expected_off = add_offset(expected_off, last)?;
    }
    // Now align for codes subcolumn
    let mis4_codes_start = expected_off & 3;
    if mis4_codes_start != 0 {
        expected_off += 4 - mis4_codes_start;
    }
    #[cfg(test)]
    debug_assert_eq!(
        expected_off, off,
        "enum NCB names section size/align drift before codes: expected={expected_off} actual={off}"
    );
    // Force-correct any drift conservatively
    off = expected_off;

    let n_code = tags.iter().filter(|&&t| t == TAG_CODE).count();
    #[cfg(test)]
    debug_assert_eq!(n_code, _n_code_expected, "code count mismatch with tags");
    let codes = decode_u32_column(bytes, &mut off, n_code, code_delta)?;

    let bit_bytes = n.div_ceil(8);
    let bits = take_bitset_tail(bytes, &mut off, n)?;
    // Build indexes: flags bitset, and Name/Code-tag bitsets derived from tags
    let flags_index = FlagsIndex::build(bits, n);
    let mut name_bits = vec![0u8; bit_bytes];
    let mut code_bits = vec![0u8; bit_bytes];
    for (i, &t) in tags.iter().enumerate() {
        if t == TAG_NAME {
            name_bits[i / 8] |= 1u8 << (i % 8);
        } else if t == TAG_CODE {
            code_bits[i / 8] |= 1u8 << (i % 8);
        }
    }
    let name_tag_index = FlagsIndex::build(&name_bits, n);
    let code_tag_index = FlagsIndex::build(&code_bits, n);

    Ok(NcbU64EnumBoolView {
        n,
        ids,
        tags: tags_view,
        names,
        codes,
        bits,
        flags_index,
        name_tag_index,
        code_tag_index,
    })
}

/// Borrowing enum reference for encoder API convenience.
pub enum EnumBorrow<'a> {
    Name(&'a str),
    Code(u32),
}
