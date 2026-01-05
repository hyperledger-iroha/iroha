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
//!   do not depend on thread‑local decode flags. Lengths are varint‑encoded when
//!   `compact-len` is enabled, or fixed u64 otherwise.
//! - Tag values are an internal detail: `0x00` = AoS, `0x01` = columnar.

// Shared AoS helpers for ad-hoc small-row layouts
use crate::{
    aos,
    core::{ByteSink, Error, len_u64_to_usize},
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
fn align_offset(off: &mut usize, align: usize) -> Result<(), Error> {
    debug_assert!(align.is_power_of_two());
    let mask = align - 1;
    let mis = *off & mask;
    if mis != 0 {
        *off = add_offset(*off, align - mis)?;
    }
    Ok(())
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
    Ok(prev)
}

fn decode_ids_column<'a>(
    bytes: &'a [u8],
    off: &mut usize,
    n: usize,
    use_delta: bool,
) -> Result<IdsRep<'a>, Error> {
    align_offset(off, 8)?;
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
    // ids
    sink.align_to(8);
    for (id, _, _) in rows {
        sink.write_u64_le(*id);
    }
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
    // flags bitset
    let bit_bytes = rows.len().div_ceil(8);
    let mut bits = vec![0u8; bit_bytes];
    for (i, (_, _, b)) in rows.iter().enumerate() {
        if *b {
            bits[i / 8] |= 1u8 << (i % 8);
        }
    }
    sink.write_bytes(&bits);
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

enum IdsRep<'a> {
    Slice(&'a [u64]),
    Rebuilt(Vec<u64>),
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
        match &self.ids {
            IdsRep::Slice(s) => s[i],
            IdsRep::Rebuilt(v) => v[i],
        }
    }
    /// Access string column. Returns `&str` over the shared blob.
    pub fn name(&self, i: usize) -> Result<&'a str, Error> {
        match &self.names {
            NamesRep::Offsets {
                offs_bytes,
                blob_str,
                ..
            } => {
                let s = read_u32_at(offs_bytes, i) as usize;
                let e = read_u32_at(offs_bytes, i + 1) as usize;
                let len = blob_str.len();
                if s > e || e > len {
                    return Err(Error::LengthMismatch);
                }
                Ok(&blob_str[s..e])
            }
            NamesRep::Dict {
                dict_offs_bytes,
                dict_blob,
                codes_bytes,
                ..
            } => {
                let code = read_u32_at(codes_bytes, i) as usize;
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
                Ok(&dict_blob[s..e])
            }
        }
    }
    /// Access boolean column.
    pub fn flag(&self, i: usize) -> bool {
        let byte = i / 8;
        let bit = i % 8;
        (self.bits[byte] >> bit) & 1 == 1
    }
    /// When ids are stored as a contiguous slice (non-delta), return it for slice-wide projections.
    pub fn ids_slice(&self) -> Option<&'a [u64]> {
        match &self.ids {
            IdsRep::Slice(s) => Some(s),
            _ => None,
        }
    }
}

/// Parse a byte slice into an NCB `(u64, str, bool)` view.
pub fn view_ncb_u64_str_bool(bytes: &[u8]) -> Result<NcbU64StrBoolView<'_>, Error> {
    if bytes.len() < 5 {
        return Err(Error::LengthMismatch);
    }
    let n = u32::from_le_bytes(bytes[0..4].try_into().unwrap()) as usize;
    let desc = bytes[4];
    // Column 1: ids (aligned to 8) or delta-coded
    let mut off = 5usize;
    let ids = decode_ids_column(bytes, &mut off, n, desc == DESC_U64_DELTA_STR_BOOL)?;
    // Column 2: strings
    let names = if desc == DESC_U64_STR_BOOL || desc == DESC_U64_DELTA_STR_BOOL {
        align_offset(&mut off, 4)?;
        let offs_count = n.checked_add(1).ok_or(Error::LengthMismatch)?;
        let offs_len = mul_checked(offs_count, 4)?;
        let offs_bytes = slice_range(bytes, off, offs_len)?;
        off = add_offset(off, offs_len)?;
        let data_len = validate_u32_offsets(offs_bytes, n)?;
        let data = slice_range(bytes, off, data_len)?;
        off = add_offset(off, data_len)?;
        let blob_str = {
            #[cfg(feature = "simdutf8-validate")]
            {
                simdutf8::basic::from_utf8(data).map_err(|_| Error::InvalidUtf8)?
            }
            #[cfg(not(feature = "simdutf8-validate"))]
            {
                std::str::from_utf8(data).map_err(|_| Error::InvalidUtf8)?
            }
        };
        NamesRep::Offsets {
            offs_bytes,
            blob_str,
        }
    } else if desc == DESC_U64_DICT_STR_BOOL {
        align_offset(&mut off, 4)?;
        let dict_len_bytes = slice_range(bytes, off, 4)?;
        let dict_len = read_u32_at(dict_len_bytes, 0) as usize;
        off = add_offset(off, 4)?;
        let dict_count = dict_len.checked_add(1).ok_or(Error::LengthMismatch)?;
        let dict_offs_len = mul_checked(dict_count, 4)?;
        let dict_offs_bytes = slice_range(bytes, off, dict_offs_len)?;
        off = add_offset(off, dict_offs_len)?;
        let dict_data_len = validate_u32_offsets(dict_offs_bytes, dict_len)?;
        let dict_data = slice_range(bytes, off, dict_data_len)?;
        off = add_offset(off, dict_data_len)?;
        // align to 4 for u32 codes
        align_offset(&mut off, 4)?;
        let codes_len = mul_checked(n, 4)?;
        let codes_bytes = slice_range(bytes, off, codes_len)?;
        off = add_offset(off, codes_len)?;
        for i in 0..n {
            let code = read_u32_at(codes_bytes, i) as usize;
            if code >= dict_len {
                return Err(Error::LengthMismatch);
            }
        }
        let dict_blob = {
            #[cfg(feature = "simdutf8-validate")]
            {
                simdutf8::basic::from_utf8(dict_data).map_err(|_| Error::InvalidUtf8)?
            }
            #[cfg(not(feature = "simdutf8-validate"))]
            {
                std::str::from_utf8(dict_data).map_err(|_| Error::InvalidUtf8)?
            }
        };
        NamesRep::Dict {
            dict_offs_bytes,
            dict_blob,
            codes_bytes,
        }
    } else {
        return Err(Error::Message("invalid NCB descriptor".into()));
    };

    // Column 3: bitset ceil(n/8)
    let bit_bytes = n.div_ceil(8);
    let bits = slice_range(bytes, off, bit_bytes)?;

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
        #[cfg(feature = "adaptive-telemetry")]
        let __t0 = std::time::Instant::now();
        let aos = aos::encode_rows_u64_str_bool(rows);
        #[cfg(feature = "adaptive-telemetry")]
        let __aos_ns = __t0.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;

        #[cfg(feature = "adaptive-telemetry")]
        let __t1 = std::time::Instant::now();
        let ncb = encode_ncb_u64_str_bool(rows);
        #[cfg(feature = "adaptive-telemetry")]
        let __ncb_ns = __t1.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;
        let aos_len = aos.len();
        let ncb_len = ncb.len();
        let (tag, mut payload) = if ncb_len < aos_len {
            (ADAPTIVE_TAG_NCB, ncb)
        } else {
            (ADAPTIVE_TAG_AOS, aos)
        };
        telemetry::record_two_pass(tag, aos_len, ncb_len);
        #[cfg(feature = "adaptive-telemetry")]
        telemetry::record_two_pass_times(__aos_ns, __ncb_ns);
        #[cfg(all(feature = "adaptive-telemetry-log", feature = "adaptive-telemetry"))]
        log_two_pass("u64_str_bool", tag, aos_len, ncb_len, __aos_ns, __ncb_ns);
        #[cfg(all(
            feature = "adaptive-telemetry-log",
            not(feature = "adaptive-telemetry")
        ))]
        log_two_pass("u64_str_bool", tag, aos_len, ncb_len, 0, 0);
        let mut out = Vec::with_capacity(1 + payload.len());
        out.push(tag);
        out.append(&mut payload);
        return out;
    }
    let (tag, mut payload) = encode_rows_u64_str_bool_auto(rows);
    telemetry::record_selection_only(tag);
    let mut out = Vec::with_capacity(1 + payload.len());
    out.push(tag);
    out.append(&mut payload);
    out
}

/// Decode an adaptive payload produced by
/// `encode_rows_u64_str_bool_adaptive(rows)` back into owned rows.
pub fn decode_rows_u64_str_bool_adaptive(bytes: &[u8]) -> Result<Vec<(u64, String, bool)>, Error> {
    if bytes.is_empty() {
        return Err(Error::LengthMismatch);
    }
    let tag = bytes[0];
    let body = &bytes[1..];
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

// encode_aos_u64_str_bool/decode moved to `crate::aos`

// Removed unused helper to satisfy clippy's dead_code lint.

// decode_aos_u64_bytes_bool moved to `crate::aos`

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

    sink.align_to(8);
    if use_delta && n > 0 {
        sink.write_u64_le(rows[0].0);
        let mut prev = rows[0].0 as i128;
        for &(id, _, _) in rows.iter().skip(1) {
            let d = (id as i128) - prev;
            prev = id as i128;
            let d64 = if d < i64::MIN as i128 || d > i64::MAX as i128 {
                0
            } else {
                d as i64
            };
            sink.write_var_u64(zigzag_encode(d64));
        }
    } else {
        for (id, _, _) in rows {
            sink.write_u64_le(*id);
        }
    }

    sink.write_bytes(&col_bytes);
    let mut bits = vec![0u8; bit_bytes];
    for (i, &(_, _, b)) in rows.iter().enumerate() {
        if b {
            bits[i / 8] |= 1u8 << (i % 8);
        }
    }
    sink.write_bytes(&bits);
    sink.into_inner()
}

fn should_use_id_delta_opt(rows: &[(u64, Option<&str>, bool)]) -> bool {
    if rows.len() < 2 {
        return false;
    }
    let mut prev = rows[0].0;
    let mut varint_bytes = 0usize;
    for &(id, _, _) in &rows[1..] {
        let d = (id as i128) - (prev as i128);
        if d < i64::MIN as i128 || d > i64::MAX as i128 {
            return false;
        }
        let zz = zigzag_encode(d as i64);
        varint_bytes += varint_len(zz);
        if varint_bytes >= 8usize.saturating_mul(rows.len().saturating_sub(1)) {
            return false;
        }
        prev = id;
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
        match &self.ids {
            IdsRep::Slice(s) => s[i],
            IdsRep::Rebuilt(v) => v[i],
        }
    }
    pub fn name(&self, i: usize) -> Result<Option<&'a str>, Error> {
        self.opt.get(i)
    }
    pub fn flag(&self, i: usize) -> bool {
        let byte = i / 8;
        let bit = i % 8;
        (self.bits[byte] >> bit) & 1 == 1
    }
}

pub fn view_ncb_u64_optstr_bool(bytes: &[u8]) -> Result<NcbU64OptStrBoolView<'_>, Error> {
    if bytes.len() < 5 {
        return Err(Error::LengthMismatch);
    }
    let n = u32::from_le_bytes(bytes[0..4].try_into().unwrap()) as usize;
    let desc = bytes[4];
    if desc != DESC_U64_OPTSTR_BOOL && desc != DESC_U64_DELTA_OPTSTR_BOOL {
        return Err(Error::Message("invalid NCB optstr descriptor".into()));
    }
    let mut off = 5usize;
    let ids = decode_ids_column(bytes, &mut off, n, desc == DESC_U64_DELTA_OPTSTR_BOOL)?;
    let opt_start = off;
    let bit_bytes = n.div_ceil(8);
    let end_bits = add_offset(opt_start, bit_bytes)?;
    if end_bits > bytes.len() {
        return Err(Error::LengthMismatch);
    }
    let pres_bits = &bytes[opt_start..end_bits];
    let mut present = 0usize;
    for (i, b) in pres_bits.iter().enumerate() {
        let mut bb = *b;
        let start_bit = i * 8;
        let end_bit = start_bit.checked_add(8).ok_or(Error::LengthMismatch)?;
        if end_bit > n {
            let extra = end_bit - n;
            if extra < 8 {
                let mask = 0xFFu16 >> extra as u16;
                bb &= mask as u8;
            } else {
                bb = 0;
            }
        }
        present += bb.count_ones() as usize;
    }
    let mut p_local = bit_bytes;
    if p_local & 3 != 0 {
        p_local += 4 - (p_local & 3);
    }
    let p = add_offset(opt_start, p_local)?;
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
    let opt = view_opt_str_column(column_bytes, n)?;
    off = end_blob;
    let flag_bytes = n.div_ceil(8);
    let bits = slice_range(bytes, off, flag_bytes)?;
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
    sink.align_to(8);
    if use_delta && n > 0 {
        sink.write_u64_le(rows[0].0);
        let mut prev = rows[0].0 as i128;
        for &(id, _, _) in rows.iter().skip(1) {
            let d = (id as i128) - prev;
            prev = id as i128;
            let d64 = if d < i64::MIN as i128 || d > i64::MAX as i128 {
                0
            } else {
                d as i64
            };
            sink.write_var_u64(zigzag_encode(d64));
        }
    } else {
        for (id, _, _) in rows {
            sink.write_u64_le(*id);
        }
    }
    sink.write_bytes(&col_bytes);
    let mut bits = vec![0u8; bit_bytes];
    for (i, &(_, _, b)) in rows.iter().enumerate() {
        if b {
            bits[i / 8] |= 1u8 << (i % 8);
        }
    }
    sink.write_bytes(&bits);
    sink.into_inner()
}

fn should_use_id_delta_optu32(rows: &[(u64, Option<u32>, bool)]) -> bool {
    if rows.len() < 2 {
        return false;
    }
    let mut prev = rows[0].0;
    let mut varint_bytes = 0usize;
    for &(id, _, _) in &rows[1..] {
        let d = (id as i128) - (prev as i128);
        if d < i64::MIN as i128 || d > i64::MAX as i128 {
            return false;
        }
        let zz = zigzag_encode(d as i64);
        varint_bytes += varint_len(zz);
        if varint_bytes >= 8usize.saturating_mul(rows.len().saturating_sub(1)) {
            return false;
        }
        prev = id;
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
        match &self.ids {
            IdsRep::Slice(s) => s[i],
            IdsRep::Rebuilt(v) => v[i],
        }
    }
    pub fn val(&self, i: usize) -> Option<u32> {
        self.opt.get(i)
    }
    pub fn flag(&self, i: usize) -> bool {
        let byte = i / 8;
        let bit = i % 8;
        (self.bits[byte] >> bit) & 1 == 1
    }
}

pub fn view_ncb_u64_optu32_bool(bytes: &[u8]) -> Result<NcbU64OptU32BoolView<'_>, Error> {
    if bytes.len() < 5 {
        return Err(Error::LengthMismatch);
    }
    let n = u32::from_le_bytes(bytes[0..4].try_into().unwrap()) as usize;
    let desc = bytes[4];
    if desc != DESC_U64_OPTU32_BOOL && desc != DESC_U64_DELTA_OPTU32_BOOL {
        return Err(Error::Message("invalid NCB optu32 descriptor".into()));
    }
    let mut off = 5usize;
    let ids = decode_ids_column(bytes, &mut off, n, desc == DESC_U64_DELTA_OPTU32_BOOL)?;
    let opt_start = off;
    let bit_bytes = n.div_ceil(8);
    let end_bits = add_offset(opt_start, bit_bytes)?;
    if end_bits > bytes.len() {
        return Err(Error::LengthMismatch);
    }
    let pres_bits = &bytes[opt_start..end_bits];
    let mut present = 0usize;
    for (i, b) in pres_bits.iter().enumerate() {
        let mut bb = *b;
        let start_bit = i * 8;
        let end_bit = start_bit.checked_add(8).ok_or(Error::LengthMismatch)?;
        if end_bit > n {
            let extra = end_bit - n;
            if extra < 8 {
                let mask = 0xFFu16 >> extra as u16;
                bb &= mask as u8;
            } else {
                bb = 0;
            }
        }
        present += bb.count_ones() as usize;
    }
    let mut p_local = bit_bytes;
    if p_local & 3 != 0 {
        p_local += 4 - (p_local & 3);
    }
    let p = add_offset(opt_start, p_local)?;
    let need = mul_checked(present, 4)?;
    let opt_end = add_offset(p, need)?;
    if opt_end > bytes.len() {
        return Err(Error::LengthMismatch);
    }
    let column_bytes = &bytes[opt_start..opt_end];
    let opt = view_opt_u32_column(column_bytes, n)?;
    off = opt_end;
    // flags
    let flag_bytes = n.div_ceil(8);
    let bits = slice_range(bytes, off, flag_bytes)?;
    Ok(NcbU64OptU32BoolView { n, ids, opt, bits })
}

/// Adaptive AoS/NCB for `(u64, Option<&str>, bool)`
pub fn encode_rows_u64_optstr_bool_adaptive(rows: &[(u64, Option<&str>, bool)]) -> Vec<u8> {
    let small_n = small_smart_n();
    if rows.len() <= small_n {
        #[cfg(feature = "adaptive-telemetry")]
        let __t0 = std::time::Instant::now();
        let mut ncb = encode_ncb_u64_optstr_bool(rows);
        #[cfg(feature = "adaptive-telemetry")]
        let __ncb_ns = __t0.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;

        #[cfg(feature = "adaptive-telemetry")]
        let __t1 = std::time::Instant::now();
        let aos = aos::encode_rows_u64_optstr_bool(rows);
        #[cfg(feature = "adaptive-telemetry")]
        let __aos_ns = __t1.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;
        let aos_len = aos.len();
        let ncb_len = ncb.len();
        let (tag, mut payload) = if ncb_len < aos_len {
            (ADAPTIVE_TAG_NCB, std::mem::take(&mut ncb))
        } else {
            (ADAPTIVE_TAG_AOS, aos)
        };
        telemetry::record_two_pass(tag, aos_len, ncb_len);
        #[cfg(feature = "adaptive-telemetry")]
        telemetry::record_two_pass_times(__aos_ns, __ncb_ns);
        #[cfg(all(feature = "adaptive-telemetry-log", feature = "adaptive-telemetry"))]
        log_two_pass("u64_optstr_bool", tag, aos_len, ncb_len, __aos_ns, __ncb_ns);
        #[cfg(all(
            feature = "adaptive-telemetry-log",
            not(feature = "adaptive-telemetry")
        ))]
        log_two_pass("u64_optstr_bool", tag, aos_len, ncb_len, 0, 0);
        let mut out = Vec::with_capacity(1 + payload.len());
        out.push(tag);
        out.append(&mut payload);
        return out;
    }
    if should_use_columnar(rows.len()) {
        let mut payload = encode_ncb_u64_optstr_bool(rows);
        let mut out = Vec::with_capacity(1 + payload.len());
        out.push(ADAPTIVE_TAG_NCB);
        out.append(&mut payload);
        telemetry::record_selection_only(ADAPTIVE_TAG_NCB);
        out
    } else {
        let buf = aos::encode_rows_u64_optstr_bool(rows);
        let mut out = Vec::with_capacity(1 + buf.len());
        out.push(ADAPTIVE_TAG_AOS);
        out.extend_from_slice(&buf);
        telemetry::record_selection_only(ADAPTIVE_TAG_AOS);
        out
    }
}

pub fn decode_rows_u64_optstr_bool_adaptive(
    bytes: &[u8],
) -> Result<Vec<(u64, Option<String>, bool)>, Error> {
    if bytes.is_empty() {
        return Err(Error::LengthMismatch);
    }
    let tag = bytes[0];
    let body = &bytes[1..];
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
        #[cfg(feature = "adaptive-telemetry")]
        let __t0 = std::time::Instant::now();
        let mut ncb = encode_ncb_u64_optu32_bool(rows);
        #[cfg(feature = "adaptive-telemetry")]
        let __ncb_ns = __t0.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;

        #[cfg(feature = "adaptive-telemetry")]
        let __t1 = std::time::Instant::now();
        let aos = aos::encode_rows_u64_optu32_bool(rows);
        #[cfg(feature = "adaptive-telemetry")]
        let __aos_ns = __t1.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;
        let aos_len = aos.len();
        let ncb_len = ncb.len();
        let (tag, mut payload) = if ncb_len < aos_len {
            (ADAPTIVE_TAG_NCB, std::mem::take(&mut ncb))
        } else {
            (ADAPTIVE_TAG_AOS, aos)
        };
        telemetry::record_two_pass(tag, aos_len, ncb_len);
        #[cfg(feature = "adaptive-telemetry")]
        telemetry::record_two_pass_times(__aos_ns, __ncb_ns);
        #[cfg(all(feature = "adaptive-telemetry-log", feature = "adaptive-telemetry"))]
        log_two_pass("u64_optu32_bool", tag, aos_len, ncb_len, __aos_ns, __ncb_ns);
        #[cfg(all(
            feature = "adaptive-telemetry-log",
            not(feature = "adaptive-telemetry")
        ))]
        log_two_pass("u64_optu32_bool", tag, aos_len, ncb_len, 0, 0);
        let mut out = Vec::with_capacity(1 + payload.len());
        out.push(tag);
        out.append(&mut payload);
        return out;
    }
    if should_use_columnar(rows.len()) {
        let mut payload = encode_ncb_u64_optu32_bool(rows);
        let mut out = Vec::with_capacity(1 + payload.len());
        out.push(ADAPTIVE_TAG_NCB);
        out.append(&mut payload);
        telemetry::record_selection_only(ADAPTIVE_TAG_NCB);
        out
    } else {
        let buf = aos::encode_rows_u64_optu32_bool(rows);
        let mut out = Vec::with_capacity(1 + buf.len());
        out.push(ADAPTIVE_TAG_AOS);
        out.extend_from_slice(&buf);
        telemetry::record_selection_only(ADAPTIVE_TAG_AOS);
        out
    }
}

pub fn decode_rows_u64_optu32_bool_adaptive(
    bytes: &[u8],
) -> Result<Vec<(u64, Option<u32>, bool)>, Error> {
    if bytes.is_empty() {
        return Err(Error::LengthMismatch);
    }
    let tag = bytes[0];
    let body = &bytes[1..];
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
        match &self.ids {
            IdsRep::Slice(s) => s[i],
            IdsRep::Rebuilt(v) => v[i],
        }
    }
    pub fn data(&self, i: usize) -> &'a [u8] {
        let s = read_u32_at(self.offs_bytes, i) as usize;
        let e = read_u32_at(self.offs_bytes, i + 1) as usize;
        &self.blob[s..e]
    }
    pub fn flag(&self, i: usize) -> bool {
        let byte = i / 8;
        let bit = i % 8;
        (self.bits[byte] >> bit) & 1 == 1
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
    sink.align_to(8);
    if use_delta && n > 0 {
        sink.write_u64_le(rows[0].0);
        let mut prev = rows[0].0 as i128;
        for &(id, _, _) in rows.iter().skip(1) {
            let d = (id as i128) - prev;
            prev = id as i128;
            let d64 = if d < i64::MIN as i128 || d > i64::MAX as i128 {
                0
            } else {
                d as i64
            };
            sink.write_var_u64(zigzag_encode(d64));
        }
    } else {
        for (id, _, _) in rows {
            sink.write_u64_le(*id);
        }
    }
    sink.align_to(4);
    for v in &offs {
        sink.write_u32_le(*v);
    }
    sink.write_bytes(&blob);
    let mut bits = vec![0u8; bit_bytes];
    for (i, &(_, _, b)) in rows.iter().enumerate() {
        if b {
            bits[i / 8] |= 1u8 << (i % 8);
        }
    }
    sink.write_bytes(&bits);
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
    let mut prev = rows[0].0;
    let mut varint_bytes = 0usize;
    for &(id, _, _) in &rows[1..] {
        let d = (id as i128) - (prev as i128);
        if d < i64::MIN as i128 || d > i64::MAX as i128 {
            return false;
        }
        let zz = zigzag_encode(d as i64);
        varint_bytes += varint_len(zz);
        if varint_bytes >= 8usize.saturating_mul(rows.len().saturating_sub(1)) {
            return false;
        }
        prev = id;
    }
    true
}

pub fn view_ncb_u64_bytes_bool(bytes: &[u8]) -> Result<NcbU64BytesBoolView<'_>, Error> {
    if bytes.len() < 5 {
        return Err(Error::LengthMismatch);
    }
    let n = u32::from_le_bytes(bytes[0..4].try_into().unwrap()) as usize;
    let desc = bytes[4];
    if desc != DESC_U64_BYTES_BOOL && desc != DESC_U64_DELTA_BYTES_BOOL {
        return Err(Error::Message("invalid NCB bytes descriptor".into()));
    }
    let mut off = 5usize;
    let mis = off & 7;
    if mis != 0 {
        off += 8 - mis;
    }
    let (ids, used_ids) = if desc == DESC_U64_DELTA_BYTES_BOOL {
        if n == 0 {
            (IdsRep::Rebuilt(Vec::new()), 0)
        } else {
            let base_bytes = bytes.get(off..off + 8).ok_or(Error::LengthMismatch)?;
            let mut lb = [0u8; 8];
            lb.copy_from_slice(base_bytes);
            let base = u64::from_le_bytes(lb);
            let mut v = Vec::with_capacity(n);
            v.push(base);
            let mut p = off + 8;
            for _ in 1..n {
                let (vv, used) = read_varint_u64(&bytes[p..])?;
                p += used;
                let d = zigzag_decode(vv) as i128;
                let prev = *v.last().unwrap() as i128;
                let cur = prev + d;
                if cur < 0 || cur > u64::MAX as i128 {
                    return Err(Error::LengthMismatch);
                }
                v.push(cur as u64);
            }
            (IdsRep::Rebuilt(v), p - off)
        }
    } else {
        let ids_len = mul_checked(n, 8)?;
        let ids_bytes = slice_range(bytes, off, ids_len)?;
        let (h, b, t) = unsafe { ids_bytes.align_to::<u64>() };
        if h.is_empty() && t.is_empty() {
            (IdsRep::Slice(b), ids_len)
        } else {
            let mut v = Vec::with_capacity(n);
            for ch in ids_bytes.chunks_exact(8) {
                let mut lb = [0u8; 8];
                lb.copy_from_slice(ch);
                v.push(u64::from_le_bytes(lb));
            }
            (IdsRep::Rebuilt(v), ids_len)
        }
    };
    off = add_offset(off, used_ids)?;
    let mis4 = off & 3;
    if mis4 != 0 {
        off += 4 - mis4;
    }
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
    let bit_bytes = n.div_ceil(8);
    let bits = slice_range(bytes, off, bit_bytes)?;
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
        #[cfg(feature = "adaptive-telemetry")]
        let __t0 = std::time::Instant::now();
        let mut ncb = encode_ncb_u64_bytes_bool(rows);
        #[cfg(feature = "adaptive-telemetry")]
        let __ncb_ns = __t0.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;

        #[cfg(feature = "adaptive-telemetry")]
        let __t1 = std::time::Instant::now();
        let aos = aos::encode_rows_u64_bytes_bool(rows);
        #[cfg(feature = "adaptive-telemetry")]
        let __aos_ns = __t1.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;
        let aos_len = aos.len();
        let ncb_len = ncb.len();
        let (tag, mut payload) = if ncb_len < aos_len {
            (ADAPTIVE_TAG_NCB, std::mem::take(&mut ncb))
        } else {
            (ADAPTIVE_TAG_AOS, aos)
        };
        telemetry::record_two_pass(tag, aos_len, ncb_len);
        #[cfg(feature = "adaptive-telemetry")]
        telemetry::record_two_pass_times(__aos_ns, __ncb_ns);
        #[cfg(all(feature = "adaptive-telemetry-log", feature = "adaptive-telemetry"))]
        log_two_pass("u64_bytes_bool", tag, aos_len, ncb_len, __aos_ns, __ncb_ns);
        #[cfg(all(
            feature = "adaptive-telemetry-log",
            not(feature = "adaptive-telemetry")
        ))]
        log_two_pass("u64_bytes_bool", tag, aos_len, ncb_len, 0, 0);
        let mut out = Vec::with_capacity(1 + payload.len());
        out.push(tag);
        out.append(&mut payload);
        return out;
    }
    if should_use_columnar(rows.len()) {
        let mut payload = encode_ncb_u64_bytes_bool(rows);
        let mut out = Vec::with_capacity(1 + payload.len());
        out.push(ADAPTIVE_TAG_NCB);
        out.append(&mut payload);
        telemetry::record_selection_only(ADAPTIVE_TAG_NCB);
        out
    } else {
        // AoS ad-hoc body via shared helper
        let buf = aos::encode_rows_u64_bytes_bool(rows);
        let mut out = Vec::with_capacity(1 + buf.len());
        out.push(ADAPTIVE_TAG_AOS);
        out.extend_from_slice(&buf);
        telemetry::record_selection_only(ADAPTIVE_TAG_AOS);
        out
    }
}

pub fn decode_rows_u64_bytes_bool_adaptive(
    bytes: &[u8],
) -> Result<Vec<(u64, Vec<u8>, bool)>, Error> {
    if bytes.is_empty() {
        return Err(Error::LengthMismatch);
    }
    let tag = bytes[0];
    let body = &bytes[1..];
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
    rows: Vec<AosStrBoolIdx>,
}

struct AosStrBoolIdx {
    id: u64,
    name_off: usize,
    name_len: usize,
    flag: bool,
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
        let s = r.name_off;
        let e = s + r.name_len;
        let bytes = &self.body[s..e];
        #[cfg(feature = "simdutf8-validate")]
        {
            simdutf8::basic::from_utf8(bytes).map_err(|_| Error::InvalidUtf8)
        }
        #[cfg(not(feature = "simdutf8-validate"))]
        {
            std::str::from_utf8(bytes).map_err(|_| Error::InvalidUtf8)
        }
    }
    pub fn flag(&self, i: usize) -> bool {
        self.rows[i].flag
    }
}

pub fn view_aos_u64_str_bool(body: &[u8]) -> Result<AosU64StrBoolView<'_>, Error> {
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

        let (slen, used) = {
            if off + 8 > body.len() {
                return Err(Error::LengthMismatch);
            }
            let mut lb = [0u8; 8];
            lb.copy_from_slice(&body[off..off + 8]);
            (len_u64_to_usize(u64::from_le_bytes(lb))?, 8)
        };
        off += used;
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
        rows.push(AosStrBoolIdx {
            id,
            name_off: s,
            name_len: slen,
            flag,
        });
    }
    Ok(AosU64StrBoolView { n, body, rows })
}

pub struct AosU64BytesBoolView<'a> {
    n: usize,
    body: &'a [u8],
    rows: Vec<AosBytesBoolIdx>,
}

struct AosBytesBoolIdx {
    id: u64,
    data_off: usize,
    data_len: usize,
    flag: bool,
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

        let (blen, used) = {
            if off + 8 > body.len() {
                return Err(Error::LengthMismatch);
            }
            let mut lb = [0u8; 8];
            lb.copy_from_slice(&body[off..off + 8]);
            (len_u64_to_usize(u64::from_le_bytes(lb))?, 8)
        };
        off += used;
        let s = off;
        let e = s.checked_add(blen).ok_or(Error::LengthMismatch)?;
        if e > body.len() {
            return Err(Error::LengthMismatch);
        }
        off = e;
        if off >= body.len() {
            return Err(Error::LengthMismatch);
        }
        let flag = body[off] != 0;
        off += 1;
        rows.push(AosBytesBoolIdx {
            id,
            data_off: s,
            data_len: blen,
            flag,
        });
    }
    Ok(AosU64BytesBoolView { n, body, rows })
}

// ===== (u64, u32, bool) =====

pub struct NcbU64U32BoolView<'a> {
    n: usize,
    ids: IdsRep<'a>,
    vals: Option<&'a [u32]>,
    vals_rebuilt: Option<Vec<u32>>,
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
        match &self.ids {
            IdsRep::Slice(s) => s[i],
            IdsRep::Rebuilt(v) => v[i],
        }
    }
    pub fn val(&self, i: usize) -> u32 {
        if let Some(s) = self.vals {
            s[i]
        } else {
            self.vals_rebuilt.as_ref().unwrap()[i]
        }
    }
    pub fn flag(&self, i: usize) -> bool {
        let byte = i / 8;
        let bit = i % 8;
        (self.bits[byte] >> bit) & 1 == 1
    }
}

pub fn encode_ncb_u64_u32_bool(
    rows: &[(u64, u32, bool)],
    use_id_delta: bool,
    use_u32_delta: bool,
) -> Vec<u8> {
    let n = rows.len();
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
    sink.align_to(8);
    if use_id_delta && n > 0 {
        sink.write_u64_le(rows[0].0);
        let mut prev = rows[0].0 as i128;
        for &(id, _, _) in rows.iter().skip(1) {
            let d = (id as i128) - prev;
            prev = id as i128;
            let d64 = if d < i64::MIN as i128 || d > i64::MAX as i128 {
                0
            } else {
                d as i64
            };
            sink.write_var_u64(zigzag_encode(d64));
        }
    } else {
        for (id, _, _) in rows {
            sink.write_u64_le(*id);
        }
    }
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
    let mut bits = vec![0u8; bit_bytes];
    for (i, &(_, _, b)) in rows.iter().enumerate() {
        if b {
            bits[i / 8] |= 1u8 << (i % 8);
        }
    }
    sink.write_bytes(&bits);
    sink.into_inner()
}

fn should_use_u32_delta(rows: &[(u64, u32, bool)]) -> bool {
    if rows.len() < 2 {
        return false;
    }
    let mut prev = rows[0].1 as i64;
    let mut varint_bytes = 0usize;
    for &(_, v, _) in &rows[1..] {
        let d_i128 = (v as i128) - (prev as i128);
        if d_i128 < i64::MIN as i128 || d_i128 > i64::MAX as i128 {
            return false;
        }
        let zz = zigzag_encode(d_i128 as i64);
        varint_bytes += varint_len(zz);
        if varint_bytes >= 4usize.saturating_mul(rows.len().saturating_sub(1)) {
            return false;
        }
        prev = v as i64;
    }
    true
}

pub fn view_ncb_u64_u32_bool(bytes: &[u8]) -> Result<NcbU64U32BoolView<'_>, Error> {
    if bytes.len() < 5 {
        return Err(Error::LengthMismatch);
    }
    let n = u32::from_le_bytes(bytes[0..4].try_into().unwrap()) as usize;
    let desc = bytes[4];
    if !matches!(
        desc,
        DESC_U64_U32_BOOL
            | DESC_U64_DELTA_U32_BOOL
            | DESC_U64_U32DELTA_BOOL
            | DESC_U64_DELTA_U32DELTA_BOOL
    ) {
        return Err(Error::Message("invalid NCB u64-u32 descriptor".into()));
    }
    let mut off = 5usize;
    let mis = off & 7;
    if mis != 0 {
        off += 8 - mis;
    }
    let (ids, used_ids) = if matches!(desc, DESC_U64_DELTA_U32_BOOL | DESC_U64_DELTA_U32DELTA_BOOL)
    {
        if n == 0 {
            (IdsRep::Rebuilt(Vec::new()), 0)
        } else {
            let base_bytes = bytes.get(off..off + 8).ok_or(Error::LengthMismatch)?;
            let mut lb = [0u8; 8];
            lb.copy_from_slice(base_bytes);
            let base = u64::from_le_bytes(lb);
            let mut v = Vec::with_capacity(n);
            v.push(base);
            let mut p = off + 8;
            for _ in 1..n {
                let (vv, used) = read_varint_u64(&bytes[p..])?;
                p += used;
                let d = zigzag_decode(vv) as i128;
                let prev = *v.last().unwrap() as i128;
                let cur = prev + d;
                if cur < 0 || cur > u64::MAX as i128 {
                    return Err(Error::LengthMismatch);
                }
                v.push(cur as u64);
            }
            (IdsRep::Rebuilt(v), p - off)
        }
    } else {
        let ids_len = mul_checked(n, 8)?;
        let ids_bytes = slice_range(bytes, off, ids_len)?;
        let (h, b, t) = unsafe { ids_bytes.align_to::<u64>() };
        if h.is_empty() && t.is_empty() {
            (IdsRep::Slice(b), ids_len)
        } else {
            let mut v = Vec::with_capacity(n);
            for ch in ids_bytes.chunks_exact(8) {
                let mut lb = [0u8; 8];
                lb.copy_from_slice(ch);
                v.push(u64::from_le_bytes(lb));
            }
            (IdsRep::Rebuilt(v), ids_len)
        }
    };
    off = add_offset(off, used_ids)?;
    // u32 column (align to 4)
    let mis4 = off & 3;
    if mis4 != 0 {
        off += 4 - mis4;
    }
    let (vals, vals_rebuilt, used_vals) =
        if matches!(desc, DESC_U64_U32DELTA_BOOL | DESC_U64_DELTA_U32DELTA_BOOL) {
            if n == 0 {
                (None, Some(Vec::new()), 0)
            } else {
                let base_bytes = bytes.get(off..off + 4).ok_or(Error::LengthMismatch)?;
                let mut lb = [0u8; 4];
                lb.copy_from_slice(base_bytes);
                let base = u32::from_le_bytes(lb) as i64;
                let mut v = Vec::with_capacity(n);
                v.push(base as u32);
                let mut p = off + 4;
                for _ in 1..n {
                    let (vv, used) = read_varint_u64(&bytes[p..])?;
                    p += used;
                    let d = zigzag_decode(vv);
                    let prev = *v.last().unwrap() as i64;
                    let cur_i128 = (prev as i128) + (d as i128);
                    if cur_i128 < 0 || cur_i128 > u32::MAX as i128 {
                        return Err(Error::LengthMismatch);
                    }
                    v.push(cur_i128 as u32);
                }
                (None, Some(v), p - off)
            }
        } else {
            let vals_len = mul_checked(n, 4)?;
            let bytes_u32 = slice_range(bytes, off, vals_len)?;
            let (h, b, t) = unsafe { bytes_u32.align_to::<u32>() };
            if h.is_empty() && t.is_empty() {
                (Some(b), None, vals_len)
            } else {
                let mut v = Vec::with_capacity(n);
                for i in 0..n {
                    v.push(read_u32_at(bytes_u32, i));
                }
                (None, Some(v), vals_len)
            }
        };
    off = add_offset(off, used_vals)?;
    // flags
    let bit_bytes = n.div_ceil(8);
    let bits = slice_range(bytes, off, bit_bytes)?;
    Ok(NcbU64U32BoolView {
        n,
        ids,
        vals,
        vals_rebuilt,
        bits,
    })
}

pub fn encode_rows_u64_u32_bool_adaptive(rows: &[(u64, u32, bool)]) -> Vec<u8> {
    let small_n = small_smart_n();
    if rows.len() <= small_n {
        #[cfg(feature = "adaptive-telemetry")]
        let __t0 = std::time::Instant::now();
        let mut ncb = encode_ncb_u64_u32_bool(
            rows,
            should_use_id_delta_u64_only(rows),
            should_use_u32_delta(rows),
        );
        #[cfg(feature = "adaptive-telemetry")]
        let __ncb_ns = __t0.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;
        #[cfg(feature = "adaptive-telemetry")]
        let (aos, __aos_ns) = {
            let __t1 = std::time::Instant::now();
            let enc = aos::encode_rows_u64_u32_bool(rows);
            let ns = __t1.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;
            (enc, ns)
        };
        #[cfg(not(feature = "adaptive-telemetry"))]
        let aos = aos::encode_rows_u64_u32_bool(rows);
        let aos_len = aos.len();
        let ncb_len = ncb.len();
        let (tag, mut payload) = if ncb_len < aos_len {
            (ADAPTIVE_TAG_NCB, std::mem::take(&mut ncb))
        } else {
            (ADAPTIVE_TAG_AOS, aos)
        };
        telemetry::record_two_pass(tag, aos_len, ncb_len);
        #[cfg(feature = "adaptive-telemetry")]
        telemetry::record_two_pass_times(__aos_ns, __ncb_ns);
        #[cfg(all(feature = "adaptive-telemetry-log", feature = "adaptive-telemetry"))]
        log_two_pass("u64_u32_bool", tag, aos_len, ncb_len, __aos_ns, __ncb_ns);
        #[cfg(all(
            feature = "adaptive-telemetry-log",
            not(feature = "adaptive-telemetry")
        ))]
        log_two_pass("u64_u32_bool", tag, aos_len, ncb_len, 0, 0);
        let mut out = Vec::with_capacity(1 + payload.len());
        out.push(tag);
        out.append(&mut payload);
        return out;
    }
    if should_use_columnar(rows.len()) {
        let use_id_delta = should_use_id_delta_u64_only(rows);
        let use_u32_delta = should_use_u32_delta(rows);
        let mut payload = encode_ncb_u64_u32_bool(rows, use_id_delta, use_u32_delta);
        let mut out = Vec::with_capacity(1 + payload.len());
        out.push(ADAPTIVE_TAG_NCB);
        out.append(&mut payload);
        telemetry::record_selection_only(ADAPTIVE_TAG_NCB);
        out
    } else {
        let buf = aos::encode_rows_u64_u32_bool(rows);
        let mut out = Vec::with_capacity(1 + buf.len());
        out.push(ADAPTIVE_TAG_AOS);
        out.extend_from_slice(&buf);
        telemetry::record_selection_only(ADAPTIVE_TAG_AOS);
        out
    }
}

fn should_use_id_delta_u64_only(rows: &[(u64, u32, bool)]) -> bool {
    if rows.len() < 2 {
        return false;
    }
    let mut prev = rows[0].0;
    let mut varint_bytes = 0usize;
    for &(id, _, _) in &rows[1..] {
        let d = (id as i128) - (prev as i128);
        if d < i64::MIN as i128 || d > i64::MAX as i128 {
            return false;
        }
        let zz = zigzag_encode(d as i64);
        varint_bytes += varint_len(zz);
        if varint_bytes >= 8usize.saturating_mul(rows.len().saturating_sub(1)) {
            return false;
        }
        prev = id;
    }
    true
}

pub fn decode_rows_u64_u32_bool_adaptive(bytes: &[u8]) -> Result<Vec<(u64, u32, bool)>, Error> {
    if bytes.is_empty() {
        return Err(Error::LengthMismatch);
    }
    let tag = bytes[0];
    let body = &bytes[1..];
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
    let mut prev = rows[0].0;
    let mut varint_bytes = 0usize;
    for &(id, _, _, _) in &rows[1..] {
        let d = (id as i128) - (prev as i128);
        if d < i64::MIN as i128 || d > i64::MAX as i128 {
            return false;
        }
        let zz = zigzag_encode(d as i64);
        varint_bytes += varint_len(zz);
        if varint_bytes >= 8usize.saturating_mul(rows.len().saturating_sub(1)) {
            return false;
        }
        prev = id;
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
    let mut prev = rows[0].2 as i64;
    let mut varint_bytes = 0usize;
    for &(_, _, v, _) in &rows[1..] {
        let d_i128 = (v as i128) - (prev as i128);
        if d_i128 < i64::MIN as i128 || d_i128 > i64::MAX as i128 {
            return false;
        }
        let zz = zigzag_encode(d_i128 as i64);
        varint_bytes += varint_len(zz);
        if varint_bytes >= 4usize.saturating_mul(rows.len().saturating_sub(1)) {
            return false;
        }
        prev = v as i64;
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
    let mut prev = rows[0].0;
    let mut varint_bytes = 0usize;
    for &(id, _, _, _) in &rows[1..] {
        let d = (id as i128) - (prev as i128);
        if d < i64::MIN as i128 || d > i64::MAX as i128 {
            return false;
        }
        let zz = zigzag_encode(d as i64);
        varint_bytes += varint_len(zz);
        if varint_bytes >= 8usize.saturating_mul(rows.len().saturating_sub(1)) {
            return false;
        }
        prev = id;
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
    let mut prev = rows[0].2 as i64;
    let mut varint_bytes = 0usize;
    for &(_, _, v, _) in &rows[1..] {
        let d_i128 = (v as i128) - (prev as i128);
        if d_i128 < i64::MIN as i128 || d_i128 > i64::MAX as i128 {
            return false;
        }
        let zz = zigzag_encode(d_i128 as i64);
        varint_bytes += varint_len(zz);
        if varint_bytes >= 4usize.saturating_mul(rows.len().saturating_sub(1)) {
            return false;
        }
        prev = v as i64;
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
    let use_id_delta = match policy.force_id_delta {
        Some(value) => value,
        None => heur_id_delta,
    };
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
    sink.align_to(8);
    if use_id_delta && n > 0 {
        sink.write_u64_le(rows[0].0);
        let mut prev = rows[0].0 as i128;
        for &(id, _, _, _) in rows.iter().skip(1) {
            let d = (id as i128) - prev;
            prev = id as i128;
            let d64 = if d < i64::MIN as i128 || d > i64::MAX as i128 {
                0
            } else {
                d as i64
            };
            sink.write_var_u64(zigzag_encode(d64));
        }
    } else {
        for &(id, _, _, _) in rows {
            sink.write_u64_le(id);
        }
    }
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
    let mut bits = vec![0u8; bit_bytes];
    for (i, &(_, _, _, f)) in rows.iter().enumerate() {
        if f {
            bits[i / 8] |= 1u8 << (i % 8);
        }
    }
    sink.write_bytes(&bits);
    sink.into_inner()
}

enum U32Rep<'a> {
    Slice(&'a [u32]),
    Rebuilt(Vec<u32>),
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
        match &self.ids {
            IdsRep::Slice(s) => s[i],
            IdsRep::Rebuilt(v) => v[i],
        }
    }
    pub fn name(&self, i: usize) -> Result<&'a str, Error> {
        match &self.names {
            NamesRep::Offsets {
                offs_bytes,
                blob_str,
            } => {
                let s = read_u32_at(offs_bytes, i) as usize;
                let e = read_u32_at(offs_bytes, i + 1) as usize;
                let len = blob_str.len();
                if s > e || e > len {
                    return Err(Error::LengthMismatch);
                }
                Ok(&blob_str[s..e])
            }
            NamesRep::Dict {
                dict_offs_bytes,
                dict_blob,
                codes_bytes,
            } => {
                let code = read_u32_at(codes_bytes, i) as usize;
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
                Ok(&dict_blob[s..e])
            }
        }
    }
    pub fn val(&self, i: usize) -> u32 {
        match &self.vals {
            U32Rep::Slice(s) => s[i],
            U32Rep::Rebuilt(v) => v[i],
        }
    }
    pub fn flag(&self, i: usize) -> bool {
        let byte = i / 8;
        let bit = i % 8;
        (self.bits[byte] >> bit) & 1 == 1
    }
}

pub fn view_ncb_u64_str_u32_bool(bytes: &[u8]) -> Result<NcbU64StrU32BoolView<'_>, Error> {
    if bytes.len() < 5 {
        return Err(Error::LengthMismatch);
    }
    let n = u32::from_le_bytes(bytes[0..4].try_into().unwrap()) as usize;
    let desc = bytes[4];
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
    let mut off = 5usize;
    let mis = off & 7;
    if mis != 0 {
        off += 8 - mis;
    }
    let (ids, used_ids) = if id_delta {
        if n == 0 {
            (IdsRep::Rebuilt(Vec::new()), 0)
        } else {
            let base_bytes = bytes.get(off..off + 8).ok_or(Error::LengthMismatch)?;
            let mut lb = [0u8; 8];
            lb.copy_from_slice(base_bytes);
            let base = u64::from_le_bytes(lb);
            let mut v = Vec::with_capacity(n);
            v.push(base);
            let mut p = off + 8;
            for _ in 1..n {
                let (vv, used) = read_varint_u64(&bytes[p..])?;
                p += used;
                let d = zigzag_decode(vv) as i128;
                let prev = *v.last().unwrap() as i128;
                let cur = prev + d;
                if cur < 0 || cur > u64::MAX as i128 {
                    return Err(Error::LengthMismatch);
                }
                v.push(cur as u64);
            }
            (IdsRep::Rebuilt(v), p - off)
        }
    } else {
        let ids_len = mul_checked(n, 8)?;
        let ids_bytes = slice_range(bytes, off, ids_len)?;
        let (h, b, t) = unsafe { ids_bytes.align_to::<u64>() };
        if h.is_empty() && t.is_empty() {
            (IdsRep::Slice(b), ids_len)
        } else {
            let mut v = Vec::with_capacity(n);
            for ch in ids_bytes.chunks_exact(8) {
                let mut lb = [0u8; 8];
                lb.copy_from_slice(ch);
                v.push(u64::from_le_bytes(lb));
            }
            (IdsRep::Rebuilt(v), ids_len)
        }
    };
    off = add_offset(off, used_ids)?;
    // names
    let names = if !is_dict {
        let mis4 = off & 3;
        if mis4 != 0 {
            off += 4 - mis4;
        }
        let offs_count = n.checked_add(1).ok_or(Error::LengthMismatch)?;
        let offs_len = mul_checked(offs_count, 4)?;
        let offs_bytes = slice_range(bytes, off, offs_len)?;
        off = add_offset(off, offs_len)?;
        let data_len = validate_u32_offsets(offs_bytes, n)?;
        let data = slice_range(bytes, off, data_len)?;
        off = add_offset(off, data_len)?;
        let blob_str = {
            #[cfg(feature = "simdutf8-validate")]
            {
                simdutf8::basic::from_utf8(data).map_err(|_| Error::InvalidUtf8)?
            }
            #[cfg(not(feature = "simdutf8-validate"))]
            {
                std::str::from_utf8(data).map_err(|_| Error::InvalidUtf8)?
            }
        };
        NamesRep::Offsets {
            offs_bytes,
            blob_str,
        }
    } else {
        let mis4 = off & 3;
        if mis4 != 0 {
            off += 4 - mis4;
        }
        let dict_len_bytes = slice_range(bytes, off, 4)?;
        let dict_len = read_u32_at(dict_len_bytes, 0) as usize;
        off = add_offset(off, 4)?;
        let dict_count = dict_len.checked_add(1).ok_or(Error::LengthMismatch)?;
        let dict_offs_len = mul_checked(dict_count, 4)?;
        let dict_offs_bytes = slice_range(bytes, off, dict_offs_len)?;
        off = add_offset(off, dict_offs_len)?;
        let dict_data_len = validate_u32_offsets(dict_offs_bytes, dict_len)?;
        let dict_data = slice_range(bytes, off, dict_data_len)?;
        off = add_offset(off, dict_data_len)?;
        let mis4_codes = off & 3;
        if mis4_codes != 0 {
            off += 4 - mis4_codes;
        }
        let codes_len = mul_checked(n, 4)?;
        let codes_bytes = slice_range(bytes, off, codes_len)?;
        off = add_offset(off, codes_len)?;
        let dict_blob = {
            #[cfg(feature = "simdutf8-validate")]
            {
                simdutf8::basic::from_utf8(dict_data).map_err(|_| Error::InvalidUtf8)?
            }
            #[cfg(not(feature = "simdutf8-validate"))]
            {
                std::str::from_utf8(dict_data).map_err(|_| Error::InvalidUtf8)?
            }
        };
        NamesRep::Dict {
            dict_offs_bytes,
            dict_blob,
            codes_bytes,
        }
    };
    // u32 values
    let mis4 = off & 3;
    if mis4 != 0 {
        off += 4 - mis4;
    }
    let (vals, used_vals) = if u32_delta {
        if n == 0 {
            (U32Rep::Rebuilt(Vec::new()), 0)
        } else {
            let mut lb = [0u8; 4];
            lb.copy_from_slice(bytes.get(off..off + 4).ok_or(Error::LengthMismatch)?);
            let base = u32::from_le_bytes(lb);
            let mut v = Vec::with_capacity(n);
            v.push(base);
            let mut p = off + 4;
            for _ in 1..n {
                let (vv, used) = read_varint_u64(&bytes[p..])?;
                p += used;
                let d = zigzag_decode(vv);
                let prev = *v.last().unwrap() as i64;
                // Avoid negation overflow by computing in wider type
                let cur_i128 = (prev as i128) + (d as i128);
                if cur_i128 < 0 || cur_i128 > u32::MAX as i128 {
                    return Err(Error::LengthMismatch);
                }
                v.push(cur_i128 as u32);
            }
            (U32Rep::Rebuilt(v), p - off)
        }
    } else {
        let vals_len = mul_checked(n, 4)?;
        let bytes_u32 = slice_range(bytes, off, vals_len)?;
        let (h, b, t) = unsafe { bytes_u32.align_to::<u32>() };
        if h.is_empty() && t.is_empty() {
            (U32Rep::Slice(b), vals_len)
        } else {
            let mut v = Vec::with_capacity(n);
            for i in 0..n {
                v.push(read_u32_at(bytes_u32, i));
            }
            (U32Rep::Rebuilt(v), vals_len)
        }
    };
    off = add_offset(off, used_vals)?;
    // flags
    let bit_bytes = n.div_ceil(8);
    let bits = slice_range(bytes, off, bit_bytes)?;
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
        #[cfg(feature = "adaptive-telemetry")]
        let __t0 = std::time::Instant::now();
        let mut ncb = encode_ncb_u64_str_u32_bool(rows);
        #[cfg(feature = "adaptive-telemetry")]
        let __ncb_ns = __t0.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;

        #[cfg(feature = "adaptive-telemetry")]
        let __t1 = std::time::Instant::now();
        let aos = aos::encode_rows_u64_str_u32_bool(rows);
        #[cfg(feature = "adaptive-telemetry")]
        let __aos_ns = __t1.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;
        let aos_len = aos.len();
        let ncb_len = ncb.len();
        let (tag, mut payload) = if ncb_len < aos_len {
            (ADAPTIVE_TAG_NCB, std::mem::take(&mut ncb))
        } else {
            (ADAPTIVE_TAG_AOS, aos)
        };
        telemetry::record_two_pass(tag, aos_len, ncb_len);
        #[cfg(feature = "adaptive-telemetry")]
        telemetry::record_two_pass_times(__aos_ns, __ncb_ns);
        #[cfg(all(feature = "adaptive-telemetry-log", feature = "adaptive-telemetry"))]
        log_two_pass(
            "u64_str_u32_bool",
            tag,
            aos_len,
            ncb_len,
            __aos_ns,
            __ncb_ns,
        );
        #[cfg(all(
            feature = "adaptive-telemetry-log",
            not(feature = "adaptive-telemetry")
        ))]
        log_two_pass("u64_str_u32_bool", tag, aos_len, ncb_len, 0, 0);
        let mut out = Vec::with_capacity(1 + payload.len());
        out.push(tag);
        out.append(&mut payload);
        return out;
    }
    if should_use_columnar(rows.len()) {
        let mut payload = encode_ncb_u64_str_u32_bool(rows);
        let mut out = Vec::with_capacity(1 + payload.len());
        out.push(ADAPTIVE_TAG_NCB);
        out.append(&mut payload);
        telemetry::record_selection_only(ADAPTIVE_TAG_NCB);
        out
    } else {
        // AoS ad-hoc body via helper
        let buf = aos::encode_rows_u64_str_u32_bool(rows);
        let mut out = Vec::with_capacity(1 + buf.len());
        out.push(ADAPTIVE_TAG_AOS);
        out.extend_from_slice(&buf);
        telemetry::record_selection_only(ADAPTIVE_TAG_AOS);
        out
    }
}

pub fn decode_rows_u64_str_u32_bool_adaptive(
    bytes: &[u8],
) -> Result<Vec<(u64, String, u32, bool)>, Error> {
    if bytes.is_empty() {
        return Err(Error::LengthMismatch);
    }
    let tag = bytes[0];
    let body = &bytes[1..];
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
        match &self.ids {
            IdsRep::Slice(s) => s[i],
            IdsRep::Rebuilt(v) => v[i],
        }
    }
    pub fn data(&self, i: usize) -> &'a [u8] {
        let s = read_u32_at(self.offs_bytes, i) as usize;
        let e = read_u32_at(self.offs_bytes, i + 1) as usize;
        &self.blob[s..e]
    }
    pub fn val(&self, i: usize) -> u32 {
        match &self.vals {
            U32Rep::Slice(s) => s[i],
            U32Rep::Rebuilt(v) => v[i],
        }
    }
    pub fn flag(&self, i: usize) -> bool {
        let byte = i / 8;
        let bit = i % 8;
        (self.bits[byte] >> bit) & 1 == 1
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
    sink.align_to(8);
    if use_id_delta && n > 0 {
        sink.write_u64_le(rows[0].0);
        let mut prev = rows[0].0 as i128;
        for &(id, _, _, _) in rows.iter().skip(1) {
            let d = (id as i128) - prev;
            prev = id as i128;
            let d64 = if d < i64::MIN as i128 || d > i64::MAX as i128 {
                0
            } else {
                d as i64
            };
            sink.write_var_u64(zigzag_encode(d64));
        }
    } else {
        for &(id, _, _, _) in rows {
            sink.write_u64_le(id);
        }
    }
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
    let mut bits = vec![0u8; bit_bytes];
    for (i, &(_, _, _, f)) in rows.iter().enumerate() {
        if f {
            bits[i / 8] |= 1u8 << (i % 8);
        }
    }
    sink.write_bytes(&bits);
    sink.into_inner()
}

pub fn view_ncb_u64_bytes_u32_bool(bytes: &[u8]) -> Result<NcbU64BytesU32BoolView<'_>, Error> {
    if bytes.len() < 5 {
        return Err(Error::LengthMismatch);
    }
    let n = u32::from_le_bytes(bytes[0..4].try_into().unwrap()) as usize;
    let desc = bytes[4];
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
    let mut off = 5usize;
    let mis = off & 7;
    if mis != 0 {
        off += 8 - mis;
    }
    let (ids, used_ids) = if id_delta {
        if n == 0 {
            (IdsRep::Rebuilt(Vec::new()), 0)
        } else {
            let base_bytes = bytes.get(off..off + 8).ok_or(Error::LengthMismatch)?;
            let mut lb = [0u8; 8];
            lb.copy_from_slice(base_bytes);
            let base = u64::from_le_bytes(lb);
            let mut v = Vec::with_capacity(n);
            v.push(base);
            let mut p = off + 8;
            for _ in 1..n {
                let (vv, used) = read_varint_u64(&bytes[p..])?;
                p += used;
                let d = zigzag_decode(vv) as i128;
                let prev = *v.last().unwrap() as i128;
                let cur = prev + d;
                if cur < 0 || cur > u64::MAX as i128 {
                    return Err(Error::LengthMismatch);
                }
                v.push(cur as u64);
            }
            (IdsRep::Rebuilt(v), p - off)
        }
    } else {
        let ids_len = mul_checked(n, 8)?;
        let ids_bytes = slice_range(bytes, off, ids_len)?;
        let (h, b, t) = unsafe { ids_bytes.align_to::<u64>() };
        if h.is_empty() && t.is_empty() {
            (IdsRep::Slice(b), ids_len)
        } else {
            let mut v = Vec::with_capacity(n);
            for ch in ids_bytes.chunks_exact(8) {
                let mut lb = [0u8; 8];
                lb.copy_from_slice(ch);
                v.push(u64::from_le_bytes(lb));
            }
            (IdsRep::Rebuilt(v), ids_len)
        }
    };
    off = add_offset(off, used_ids)?;
    let mis4 = off & 3;
    if mis4 != 0 {
        off += 4 - mis4;
    }
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
    let mis4v = off & 3;
    if mis4v != 0 {
        off += 4 - mis4v;
    }
    let (vals, used_vals) = if u32_delta {
        if n == 0 {
            (U32Rep::Rebuilt(Vec::new()), 0)
        } else {
            let mut lb = [0u8; 4];
            lb.copy_from_slice(bytes.get(off..off + 4).ok_or(Error::LengthMismatch)?);
            let base = u32::from_le_bytes(lb);
            let mut v = Vec::with_capacity(n);
            v.push(base);
            let mut p = off + 4;
            for _ in 1..n {
                let (vv, used) = read_varint_u64(&bytes[p..])?;
                p += used;
                let d = zigzag_decode(vv);
                let prev = *v.last().unwrap() as i64;
                // Avoid negation/overflow by computing in wider type and
                // validating bounds before casting to u32
                let cur_i128 = (prev as i128) + (d as i128);
                if cur_i128 < 0 || cur_i128 > u32::MAX as i128 {
                    return Err(Error::LengthMismatch);
                }
                v.push(cur_i128 as u32);
            }
            (U32Rep::Rebuilt(v), p - off)
        }
    } else {
        let vals_len = mul_checked(n, 4)?;
        let bytes_u32 = slice_range(bytes, off, vals_len)?;
        let (h, b, t) = unsafe { bytes_u32.align_to::<u32>() };
        if h.is_empty() && t.is_empty() {
            (U32Rep::Slice(b), vals_len)
        } else {
            let mut v = Vec::with_capacity(n);
            for i in 0..n {
                v.push(read_u32_at(bytes_u32, i));
            }
            (U32Rep::Rebuilt(v), vals_len)
        }
    };
    off = add_offset(off, used_vals)?;
    let bit_bytes = n.div_ceil(8);
    let bits = slice_range(bytes, off, bit_bytes)?;
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
        #[cfg(feature = "adaptive-telemetry")]
        let __t0 = std::time::Instant::now();
        let mut ncb = encode_ncb_u64_bytes_u32_bool(rows);
        #[cfg(feature = "adaptive-telemetry")]
        let __ncb_ns = __t0.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;
        // AoS ad-hoc body via shared helper
        #[cfg(feature = "adaptive-telemetry")]
        let __t1 = std::time::Instant::now();
        let aos = aos::encode_rows_u64_bytes_u32_bool(rows);
        #[cfg(feature = "adaptive-telemetry")]
        let __aos_ns = __t1.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;
        let aos_len = aos.len();
        let ncb_len = ncb.len();
        let (tag, mut payload) = if ncb.len() < aos_len {
            (ADAPTIVE_TAG_NCB, std::mem::take(&mut ncb))
        } else {
            (ADAPTIVE_TAG_AOS, aos)
        };
        telemetry::record_two_pass(tag, aos_len, ncb_len);
        #[cfg(feature = "adaptive-telemetry")]
        telemetry::record_two_pass_times(__aos_ns, __ncb_ns);
        #[cfg(all(feature = "adaptive-telemetry-log", feature = "adaptive-telemetry"))]
        log_two_pass(
            "u64_bytes_u32_bool",
            tag,
            aos_len,
            ncb_len,
            __aos_ns,
            __ncb_ns,
        );
        #[cfg(all(
            feature = "adaptive-telemetry-log",
            not(feature = "adaptive-telemetry")
        ))]
        log_two_pass("u64_bytes_u32_bool", tag, aos_len, ncb_len, 0, 0);
        let mut out = Vec::with_capacity(1 + payload.len());
        out.push(tag);
        out.append(&mut payload);
        return out;
    }
    if should_use_columnar(rows.len()) {
        let mut payload = encode_ncb_u64_bytes_u32_bool(rows);
        let mut out = Vec::with_capacity(1 + payload.len());
        out.push(ADAPTIVE_TAG_NCB);
        out.append(&mut payload);
        telemetry::record_selection_only(ADAPTIVE_TAG_NCB);
        out
    } else {
        // AoS ad-hoc body via shared helper
        let buf = aos::encode_rows_u64_bytes_u32_bool(rows);
        let mut out = Vec::with_capacity(1 + buf.len());
        out.push(ADAPTIVE_TAG_AOS);
        out.extend_from_slice(&buf);
        telemetry::record_selection_only(ADAPTIVE_TAG_AOS);
        out
    }
}

#[allow(clippy::type_complexity)]
pub fn decode_rows_u64_bytes_u32_bool_adaptive(
    bytes: &[u8],
) -> Result<Vec<(u64, Vec<u8>, u32, bool)>, Error> {
    if bytes.is_empty() {
        return Err(Error::LengthMismatch);
    }
    let tag = bytes[0];
    let body = &bytes[1..];
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
    rows: Vec<AosStrU32Idx>,
}

struct AosStrU32Idx {
    id: u64,
    name_off: usize,
    name_len: usize,
    val: u32,
    flag: bool,
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
        let s = r.name_off;
        let e = s + r.name_len;
        let bytes = &self.body[s..e];
        #[cfg(feature = "simdutf8-validate")]
        {
            simdutf8::basic::from_utf8(bytes).map_err(|_| Error::InvalidUtf8)
        }
        #[cfg(not(feature = "simdutf8-validate"))]
        {
            std::str::from_utf8(bytes).map_err(|_| Error::InvalidUtf8)
        }
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

        let (slen, used) = {
            if off + 8 > body.len() {
                return Err(Error::LengthMismatch);
            }
            let mut lb = [0u8; 8];
            lb.copy_from_slice(&body[off..off + 8]);
            (len_u64_to_usize(u64::from_le_bytes(lb))?, 8)
        };
        off += used;
        let s = off;
        let e = s.checked_add(slen).ok_or(Error::LengthMismatch)?;
        if e > body.len() {
            return Err(Error::LengthMismatch);
        }
        off = e;
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
        rows.push(AosStrU32Idx {
            id,
            name_off: s,
            name_len: slen,
            val: u32::from_le_bytes(vb),
            flag,
        });
    }
    Ok(AosU64StrU32BoolView { n, body, rows })
}

/// Borrowed view over an AoS ad-hoc body for rows shaped as `(u64, &[u8], u32, bool)`.
pub struct AosU64BytesU32BoolView<'a> {
    n: usize,
    body: &'a [u8],
    rows: Vec<AosBytesU32Idx>,
}

struct AosBytesU32Idx {
    id: u64,
    data_off: usize,
    data_len: usize,
    val: u32,
    flag: bool,
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

        let (blen, used) = {
            if off + 8 > body.len() {
                return Err(Error::LengthMismatch);
            }
            let mut lb = [0u8; 8];
            lb.copy_from_slice(&body[off..off + 8]);
            (len_u64_to_usize(u64::from_le_bytes(lb))?, 8)
        };
        off += used;
        let s = off;
        let e = s.checked_add(blen).ok_or(Error::LengthMismatch)?;
        if e > body.len() {
            return Err(Error::LengthMismatch);
        }
        off = e;
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
        rows.push(AosBytesU32Idx {
            id,
            data_off: s,
            data_len: blen,
            val: u32::from_le_bytes(vb),
            flag,
        });
    }
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
        #[cfg(feature = "simdutf8-validate")]
        {
            Ok(Some(
                simdutf8::basic::from_utf8(bytes).map_err(|_| Error::InvalidUtf8)?,
            ))
        }
        #[cfg(not(feature = "simdutf8-validate"))]
        {
            Ok(Some(
                std::str::from_utf8(bytes).map_err(|_| Error::InvalidUtf8)?,
            ))
        }
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
            let (slen, used) = {
                if off + 8 > body.len() {
                    return Err(Error::LengthMismatch);
                }
                let mut lb = [0u8; 8];
                lb.copy_from_slice(&body[off..off + 8]);
                (len_u64_to_usize(u64::from_le_bytes(lb))?, 8)
            };
            off += used;
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
                #[cfg(feature = "simdutf8-validate")]
                {
                    Ok(AosEnumRef::Name(
                        simdutf8::basic::from_utf8(s).map_err(|_| Error::InvalidUtf8)?,
                    ))
                }
                #[cfg(not(feature = "simdutf8-validate"))]
                {
                    Ok(AosEnumRef::Name(
                        std::str::from_utf8(s).map_err(|_| Error::InvalidUtf8)?,
                    ))
                }
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

    let (n, used) = {
        if body.len() < 8 {
            return Err(Error::LengthMismatch);
        }
        let mut lb = [0u8; 8];
        lb.copy_from_slice(&body[..8]);
        (len_u64_to_usize(u64::from_le_bytes(lb))?, 8)
    };
    off += used;
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
            let (slen, used) = {
                if off + 8 > body.len() {
                    return Err(Error::LengthMismatch);
                }
                let mut lb = [0u8; 8];
                lb.copy_from_slice(&body[off..off + 8]);
                (len_u64_to_usize(u64::from_le_bytes(lb))?, 8)
            };
            off += used;
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
        #[cfg(feature = "adaptive-telemetry")]
        let __t0 = std::time::Instant::now();
        let mut ncb = encode_ncb_u64_enum_bool(rows, use_delta_ids, use_name_dict, use_code_delta);
        #[cfg(feature = "adaptive-telemetry")]
        let __ncb_ns = __t0.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;

        #[cfg(feature = "adaptive-telemetry")]
        let __t1 = std::time::Instant::now();
        let aos = aos::encode_rows_u64_enum_bool(rows);
        #[cfg(feature = "adaptive-telemetry")]
        let __aos_ns = __t1.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;
        let aos_len = aos.len();
        let ncb_len = ncb.len();
        let (tag, mut payload) = if ncb_len < aos_len {
            (ADAPTIVE_ENUM_TAG_NCB, std::mem::take(&mut ncb))
        } else {
            (ADAPTIVE_ENUM_TAG_AOS, aos)
        };
        telemetry::record_two_pass(tag, aos_len, ncb_len);
        #[cfg(feature = "adaptive-telemetry")]
        telemetry::record_two_pass_times(__aos_ns, __ncb_ns);
        #[cfg(all(feature = "adaptive-telemetry-log", feature = "adaptive-telemetry"))]
        log_two_pass("u64_enum_bool", tag, aos_len, ncb_len, __aos_ns, __ncb_ns);
        #[cfg(all(
            feature = "adaptive-telemetry-log",
            not(feature = "adaptive-telemetry")
        ))]
        log_two_pass("u64_enum_bool", tag, aos_len, ncb_len, 0, 0);
        let mut out = Vec::with_capacity(1 + payload.len());
        out.push(tag);
        out.append(&mut payload);
        return out;
    }
    if should_use_columnar(rows.len()) {
        let use_delta_ids = should_use_id_delta_enum(rows);
        let use_name_dict = should_use_name_dict_enum(rows);
        let use_code_delta = should_use_code_delta_enum(rows);
        let mut payload =
            encode_ncb_u64_enum_bool(rows, use_delta_ids, use_name_dict, use_code_delta);
        let mut out = Vec::with_capacity(1 + payload.len());
        out.push(ADAPTIVE_ENUM_TAG_NCB);
        out.append(&mut payload);
        out
    } else {
        let buf = aos::encode_rows_u64_enum_bool(rows);
        let mut out = Vec::with_capacity(1 + buf.len());
        out.push(ADAPTIVE_ENUM_TAG_AOS);
        out.extend_from_slice(&buf);
        out
    }
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
    if bytes.is_empty() {
        return Err(Error::LengthMismatch);
    }
    let tag = bytes[0];
    let body = &bytes[1..];
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

    #[test]
    fn adaptive_large_input_tags_ncb() {
        // Build rows just over the threshold so auto path is used
        let t = crate::core::heuristics::get().aos_ncb_small_n;
        let n = t.saturating_add(1);
        let mut rows: Vec<(u64, &str, bool)> = Vec::with_capacity(n);
        // Use small, distinct strings; content heuristics may vary inside NCB but the tag is driven by size
        let names: Vec<String> = (0..n).map(|i| format!("n{i}")).collect();
        for (i, name) in names.iter().enumerate() {
            rows.push((i as u64, name.as_str(), i % 2 == 0));
        }
        let bytes = encode_rows_u64_str_bool_adaptive(&rows);
        assert!(!bytes.is_empty());
        match bytes[0] {
            ADAPTIVE_TAG_NCB => {
                assert!(
                    should_use_columnar(n),
                    "encoder picked NCB but heuristic rejected columnar"
                );
            }
            ADAPTIVE_TAG_AOS => {
                assert!(
                    !should_use_columnar(n),
                    "encoder picked AoS while heuristic expected columnar"
                );
            }
            other => panic!("unexpected adaptive tag: {other}"),
        }
        // And it should roundtrip
        let decoded = decode_rows_u64_str_bool_adaptive(&bytes).expect("decode");
        let expected: Vec<(u64, String, bool)> = rows
            .iter()
            .map(|(id, s, b)| (*id, (*s).to_string(), *b))
            .collect();
        assert_eq!(decoded, expected);
    }
}

#[inline]
fn should_use_id_delta_enum(rows: &[(u64, EnumBorrow<'_>, bool)]) -> bool {
    if rows.len() < 2 {
        return false;
    }
    let mut prev = rows[0].0;
    let mut varint_bytes: usize = 0;
    for &(id, _, _) in &rows[1..] {
        let d = (id as i128) - (prev as i128);
        if d < i64::MIN as i128 || d > i64::MAX as i128 {
            return false;
        }
        let zz = zigzag_encode(d as i64);
        varint_bytes += varint_len(zz);
        if varint_bytes >= 8usize.saturating_mul(rows.len().saturating_sub(1)) {
            return false;
        }
        prev = id;
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
        value |= ((b & 0x7F) as u64) << shift;
        if (b & 0x80) == 0 {
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
            } => {
                let count = offs_bytes.len() / 4;
                if *i + 1 >= count {
                    return None;
                }
                let s = read_u32_at(offs_bytes, *i) as usize;
                let e = read_u32_at(offs_bytes, *i + 1) as usize;
                *i += 1;
                Some(&blob[s..e])
            }
            NamesIterator::Dict {
                dict_offs_bytes,
                dict_blob,
                codes_bytes,
                i,
            } => {
                let n = codes_bytes.len() / 4;
                if *i >= n {
                    return None;
                }
                let code = read_u32_at(codes_bytes, *i) as usize;
                let s = read_u32_at(dict_offs_bytes, code) as usize;
                let e = read_u32_at(dict_offs_bytes, code + 1) as usize;
                *i += 1;
                Some(&dict_blob[s..e])
            }
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
    // IDs
    sink.align_to(8);
    for (id, _, _) in rows {
        sink.write_u64_le(*id);
    }
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
    // Flags bitset
    let bit_bytes = (n as usize).div_ceil(8);
    let mut bits = vec![0u8; bit_bytes];
    for (i, (_, _, b)) in rows.iter().enumerate() {
        if *b {
            bits[i / 8] |= 1u8 << (i % 8);
        }
    }
    sink.write_bytes(&bits);
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
    let mut ok = true;
    for &(id, _, _) in &rows[1..] {
        let d_i128 = (id as i128) - (prev as i128);
        if d_i128 < i64::MIN as i128 || d_i128 > i64::MAX as i128 {
            ok = false;
            break;
        }
        let d = d_i128 as i64;
        let zz = zigzag_encode(d);
        deltas.push(zz);
        varint_bytes += varint_len(zz);
        prev = id;
    }
    // Check if delta coding saves space vs 8*(n-1)
    if !ok || varint_bytes >= 8usize.saturating_mul(rows.len().saturating_sub(1)) {
        return encode_ncb_u64_str_bool(rows);
    }
    let n = rows.len() as u32;
    let mut sink = ByteSink::with_headroom(4 + 1 + rows.len() * (8 + 1 + 4) + 16, 0);
    sink.write_bytes(&n.to_le_bytes());
    sink.write_u8(DESC_U64_DELTA_STR_BOOL);
    // ids: base + varint deltas
    sink.align_to(8);
    sink.write_u64_le(rows[0].0);
    for &zz in &deltas {
        {
            let mut vv = zz;
            while vv >= 0x80 {
                sink.write_u8((vv as u8) | 0x80);
                vv >>= 7;
            }
            sink.write_u8(vv as u8);
        }
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
    // flags
    let bit_bytes = rows.len().div_ceil(8);
    let mut bits = vec![0u8; bit_bytes];
    for (i, &(_, _, b)) in rows.iter().enumerate() {
        if b {
            bits[i / 8] |= 1u8 << (i % 8);
        }
    }
    sink.write_bytes(&bits);
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
    {
        let mut vv = v;
        while vv >= 0x80 {
            buf.push((vv as u8) | 0x80);
            vv >>= 7;
        }
        buf.push(vv as u8);
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
    let mut prev = rows[0].0;
    let mut varint_bytes: usize = 0;
    for &(id, _, _) in &rows[1..] {
        let d = (id as i128) - (prev as i128);
        if d < i64::MIN as i128 || d > i64::MAX as i128 {
            return false;
        }
        let zz = zigzag_encode(d as i64);
        varint_bytes += varint_len(zz);
        if varint_bytes >= 8usize.saturating_mul(rows.len().saturating_sub(1)) {
            return false;
        }
        prev = id;
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
    let bit_bytes = n_rows.div_ceil(8);
    if bytes.len() < bit_bytes {
        return Err(Error::LengthMismatch);
    }
    let pres_bits = &bytes[..bit_bytes];
    // Count present entries from the bitset
    let mut present = 0usize;
    for (i, b) in pres_bits.iter().enumerate() {
        // Mask out bits beyond n_rows in the last byte
        let mut bb = *b;
        let start_bit = i * 8;
        let end_bit = start_bit.checked_add(8).ok_or(Error::LengthMismatch)?;
        if end_bit > n_rows {
            let extra = end_bit - n_rows;
            if extra < 8 {
                let mask = 0xFFu16 >> extra as u16;
                bb &= mask as u8;
            } else {
                bb = 0;
            }
        }
        present += bb.count_ones() as usize;
    }
    let mut off = bit_bytes;
    let mis4 = off & 3;
    if mis4 != 0 {
        off += 4 - mis4;
    }
    // Offsets table (present+1) followed by blob
    let offs_count = present.checked_add(1).ok_or(Error::LengthMismatch)?;
    let offs_len_bytes = mul_checked(offs_count, 4)?;
    let offs_bytes = slice_range(bytes, off, offs_len_bytes)?;
    off = add_offset(off, offs_len_bytes)?;
    let last = validate_u32_offsets(offs_bytes, present)?;
    let blob = slice_range(bytes, off, last)?;
    let blob_str = {
        #[cfg(feature = "simdutf8-validate")]
        {
            simdutf8::basic::from_utf8(blob).map_err(|_| Error::InvalidUtf8)?
        }
        #[cfg(not(feature = "simdutf8-validate"))]
        {
            std::str::from_utf8(blob).map_err(|_| Error::InvalidUtf8)?
        }
    };
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
    let bit_bytes = n_rows.div_ceil(8);
    if bytes.len() < bit_bytes {
        return Err(Error::LengthMismatch);
    }
    let pres_bits = &bytes[..bit_bytes];
    // Count present
    let mut present = 0usize;
    for (i, b) in pres_bits.iter().enumerate() {
        let mut bb = *b;
        let start_bit = i * 8;
        let end_bit = start_bit.checked_add(8).ok_or(Error::LengthMismatch)?;
        if end_bit > n_rows {
            let extra = end_bit - n_rows;
            if extra < 8 {
                let mask = 0xFFu16 >> extra as u16;
                bb &= mask as u8;
            } else {
                bb = 0;
            }
        }
        present += bb.count_ones() as usize;
    }
    let mut off = bit_bytes;
    let mis4 = off & 3;
    if mis4 != 0 {
        off += 4 - mis4;
    }
    let vals_len = mul_checked(present, 4)?;
    let vals_bytes = slice_range(bytes, off, vals_len)?;
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
    codes: CodeRep<'a>,
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

enum CodeRep<'a> {
    Slice(&'a [u32]),
    Rebuilt(Vec<u32>),
}

impl<'a> NcbU64EnumBoolView<'a> {
    /// Number of rows.
    pub fn len(&self) -> usize {
        self.n
    }
    /// Access id column.
    pub fn id(&self, i: usize) -> u64 {
        match &self.ids {
            IdsRep::Slice(s) => s[i],
            IdsRep::Rebuilt(v) => v[i],
        }
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
                let v = match &self.codes {
                    CodeRep::Slice(s) => s[k],
                    CodeRep::Rebuilt(v) => v[k],
                };
                Ok(ColEnumRef::Code(v))
            }
            other => Err(Error::invalid_tag("projecting enum payload variant", other)),
        }
    }
    /// Access boolean flag column.
    pub fn flag(&self, i: usize) -> bool {
        let byte = i / 8;
        let bit = i % 8;
        (self.bits[byte] >> bit) & 1 == 1
    }
    /// Project raw tags slice for projection-only scans.
    pub fn tags_slice(&self) -> &'a [u8] {
        self.tags.tags
    }
    /// Project raw ids when available as a slice (None if delta-coded).
    pub fn ids_slice(&self) -> Option<&'a [u64]> {
        if let IdsRep::Slice(s) = &self.ids {
            Some(s)
        } else {
            None
        }
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
            CodeRep::Slice(s) => s.len(),
            CodeRep::Rebuilt(v) => v.len(),
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
            CodeRep::Slice(s) => s.get(k).copied().ok_or(Error::LengthMismatch),
            CodeRep::Rebuilt(v) => v.get(k).copied().ok_or(Error::LengthMismatch),
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
            CodeRep::Slice(s) => CodesDenseIter::Slice { slice: s, i: 0 },
            CodeRep::Rebuilt(v) => CodesDenseIter::Vec { vec: v, i: 0 },
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
            } => {
                let count = offs_bytes.len() / 4;
                if *i + 1 >= count {
                    return None;
                }
                let s = read_u32_at(offs_bytes, *i) as usize;
                let e = read_u32_at(offs_bytes, *i + 1) as usize;
                *i += 1;
                Some(&blob[s..e])
            }
            NamesDenseIter::Dict {
                dict_offs_bytes,
                dict_blob,
                codes_bytes,
                i,
            } => {
                let n = codes_bytes.len() / 4;
                if *i >= n {
                    return None;
                }
                let code = read_u32_at(codes_bytes, *i) as usize;
                let s = read_u32_at(dict_offs_bytes, code) as usize;
                let e = read_u32_at(dict_offs_bytes, code + 1) as usize;
                *i += 1;
                Some(&dict_blob[s..e])
            }
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
                CodeRep::Slice(s) => s.get(k).copied(),
                CodeRep::Rebuilt(v) => v.get(k).copied(),
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
            CodeRep::Slice(s) => s.get(k).copied(),
            CodeRep::Rebuilt(v) => v.get(k).copied(),
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

    // ids
    sink.align_to(8);
    if use_delta_ids && n >= 2 {
        sink.write_u64_le(rows[0].0);
        let mut prev = rows[0].0 as i128;
        for &(id, _, _) in &rows[1..] {
            let d = (id as i128) - prev;
            prev = id as i128;
            let d64 = if d < i64::MIN as i128 || d > i64::MAX as i128 {
                0
            } else {
                d as i64
            };
            let zz = zigzag_encode(d64);

            {
                let mut vv = zz;
                while vv >= 0x80 {
                    sink.write_u8(((vv as u8) | 0x80) as u8);
                    vv >>= 7;
                }
                sink.write_u8(vv as u8);
            }
        }
    } else {
        for (id, _, _) in rows {
            sink.write_u64_le(*id);
        }
    }

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
            let zz = zigzag_encode(d);

            {
                let mut vv = zz;
                while vv >= 0x80 {
                    sink.write_u8(((vv as u8) | 0x80) as u8);
                    vv >>= 7;
                }
                sink.write_u8(vv as u8);
            }
        }
    } else {
        for v in &codes {
            sink.write_u32_le(*v);
        }
    }

    // flags
    let bit_bytes = n.div_ceil(8);
    let mut bits = vec![0u8; bit_bytes];
    for (i, &(_, _, b)) in rows.iter().enumerate() {
        if b {
            bits[i / 8] |= 1u8 << (i % 8);
        }
    }
    sink.write_bytes(&bits);
    sink.into_inner()
}

/// Parse a byte slice into an enum NCB view.
pub fn view_ncb_u64_enum_bool(bytes: &[u8]) -> Result<NcbU64EnumBoolView<'_>, Error> {
    if bytes.len() < 5 {
        return Err(Error::LengthMismatch);
    }
    let n = u32::from_le_bytes(bytes[0..4].try_into().unwrap()) as usize;
    let desc = bytes[4];
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

    // ids (aligned 8)
    let mut off = 5usize;
    let mis = off & 7;
    if mis != 0 {
        off += 8 - mis;
    }
    let (ids, used_ids) = if is_delta {
        if n == 0 {
            (IdsRep::Rebuilt(Vec::new()), 0)
        } else {
            let base_bytes = bytes.get(off..off + 8).ok_or(Error::LengthMismatch)?;
            let mut lb = [0u8; 8];
            lb.copy_from_slice(base_bytes);
            let base = u64::from_le_bytes(lb);
            let mut vec = Vec::with_capacity(n);
            vec.push(base);
            let mut p = off + 8;
            for _ in 1..n {
                let (v, used) = read_varint_u64(&bytes[p..])?;
                p += used;
                let d = zigzag_decode(v) as i128;
                let prev = *vec.last().unwrap() as i128;
                let curr = prev + d;
                if curr < 0 || curr > u64::MAX as i128 {
                    return Err(Error::LengthMismatch);
                }
                vec.push(curr as u64);
            }
            (IdsRep::Rebuilt(vec), p - off)
        }
    } else {
        let ids_len = mul_checked(n, 8)?;
        let ids_bytes = slice_range(bytes, off, ids_len)?;
        let (head, body, tail) = unsafe { ids_bytes.align_to::<u64>() };
        if head.is_empty() && tail.is_empty() {
            (IdsRep::Slice(body), ids_len)
        } else {
            let mut v = Vec::with_capacity(n);
            for chunk in ids_bytes.chunks_exact(8) {
                let mut lb = [0u8; 8];
                lb.copy_from_slice(chunk);
                v.push(u64::from_le_bytes(lb));
            }
            (IdsRep::Rebuilt(v), ids_len)
        }
    };
    off = add_offset(off, used_ids)?;

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
    let mis4 = off_names & 3;
    if mis4 != 0 {
        off_names += 4 - mis4;
    }
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
        let dict_len = u32::from_le_bytes(lb) as usize;
        let dict_count = dict_len.checked_add(1).ok_or(Error::LengthMismatch)?;
        let dict_offs_len = mul_checked(dict_count, 4)?;
        let dict_offs_bytes = slice_range(bytes, off_names, dict_offs_len)?;
        off_names = add_offset(off_names, dict_offs_len)?;
        let dict_data_len = validate_u32_offsets(dict_offs_bytes, dict_len)?;
        let dict_data = slice_range(bytes, off_names, dict_data_len)?;
        off_names = add_offset(off_names, dict_data_len)?;
        // Align before reading per-Name codes (u32)
        let mis4_codes = off_names & 3;
        if mis4_codes != 0 {
            off_names += 4 - mis4_codes;
        }
        let codes_len = mul_checked(n_name, 4)?;
        let codes_bytes = slice_range(bytes, off_names, codes_len)?;
        for i in 0..n_name {
            let code = read_u32_at(codes_bytes, i) as usize;
            if code >= dict_len {
                return Err(Error::LengthMismatch);
            }
        }
        let dict_blob = {
            #[cfg(feature = "simdutf8-validate")]
            {
                simdutf8::basic::from_utf8(dict_data).map_err(|_| Error::InvalidUtf8)?
            }
            #[cfg(not(feature = "simdutf8-validate"))]
            {
                std::str::from_utf8(dict_data).map_err(|_| Error::InvalidUtf8)?
            }
        };
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
        let blob_str = {
            #[cfg(feature = "simdutf8-validate")]
            {
                simdutf8::basic::from_utf8(data).map_err(|_| Error::InvalidUtf8)?
            }
            #[cfg(not(feature = "simdutf8-validate"))]
            {
                std::str::from_utf8(data).map_err(|_| Error::InvalidUtf8)?
            }
        };
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
        let dict_len = u32::from_le_bytes(dlb) as usize;
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

    // codes subcolumn (aligned 4)
    let mis4 = off & 3;
    if mis4 != 0 {
        off += 4 - mis4;
    }
    let n_code = tags.iter().filter(|&&t| t == TAG_CODE).count();
    #[cfg(test)]
    debug_assert_eq!(n_code, _n_code_expected, "code count mismatch with tags");
    let codes = if code_delta {
        if n_code == 0 {
            CodeRep::Rebuilt(Vec::new())
        } else {
            // base u32 + (n_code-1) varint zigzag deltas
            let base_bytes = bytes.get(off..off + 4).ok_or(Error::LengthMismatch)?;
            off += 4;
            let mut lb = [0u8; 4];
            lb.copy_from_slice(base_bytes);
            let base = u32::from_le_bytes(lb) as i64;
            let mut vec = Vec::with_capacity(n_code);
            vec.push(base as u32);
            let mut p = off;
            for _ in 1..n_code {
                let (v, used) = read_varint_u64(&bytes[p..])?;
                p += used;
                let d = zigzag_decode(v);
                let prev = *vec.last().unwrap() as i64;
                // Perform addition in wider type and validate u32 bounds to avoid wraparound
                let curr_i128 = (prev as i128) + (d as i128);
                if curr_i128 < 0 || curr_i128 > u32::MAX as i128 {
                    return Err(Error::LengthMismatch);
                }
                vec.push(curr_i128 as u32);
            }
            off = p;
            CodeRep::Rebuilt(vec)
        }
    } else {
        let codes_len = mul_checked(n_code, 4)?;
        let codes_bytes = slice_range(bytes, off, codes_len)?;
        off = add_offset(off, codes_len)?;
        let (hc, codes_slice, tc) = unsafe { codes_bytes.align_to::<u32>() };
        if hc.is_empty() && tc.is_empty() {
            CodeRep::Slice(codes_slice)
        } else {
            let mut v = Vec::with_capacity(n_code);
            for i in 0..n_code {
                v.push(read_u32_at(codes_bytes, i));
            }
            CodeRep::Rebuilt(v)
        }
    };

    // flags bitset
    let bit_bytes = n.div_ceil(8);
    let bits = bytes
        .get(off..off + bit_bytes)
        .ok_or(Error::LengthMismatch)?;
    #[cfg(test)]
    debug_assert_eq!(
        off + bit_bytes,
        bytes.len(),
        "unexpected trailing bytes in enum NCB view"
    );
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
