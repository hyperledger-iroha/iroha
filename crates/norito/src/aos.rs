//! AoS (ad‑hoc) helpers for adaptive columnar paths
//!
//! This module centralizes the small, headerless Array‑of‑Structs encoders and
//! decoders used by the adaptive helpers in `columnar.rs` when they choose the
//! AoS representation for small inputs. Keeping these routines in one place
//! reduces duplication and ensures consistent error reporting and length/format
//! handling.
//!
//! Format
//! - Bodies start with `[len][ver]` where `len` is encoded using Norito's
//!   length encoding rules (varint under `compact-len`, or fixed u64 otherwise)
//!   and `ver` is a single byte where the low nibble carries the version number
//!   (currently `0x1`) and the high nibble is reserved and must be zero.
//! - After the header, rows are laid out sequentially without padding.
//! - Length prefixes for AoS bodies are determined solely by the compile-time
//!   `compact-len` feature and ignore header flag settings.
//!
//! Notes
//! - These helpers are intentionally `pub` so tests and other crates can use
//!   them for building/inspecting AoS bodies in isolation when needed.

use crate::core::Error;

#[cfg(feature = "compact-len")]
const MAX_VARINT_BYTES: usize = 10;

#[inline]
fn write_len_prefix(buf: &mut Vec<u8>, value: usize) {
    #[cfg(feature = "compact-len")]
    {
        let mut v = value as u64;
        loop {
            let byte = (v & 0x7f) as u8;
            v >>= 7;
            if v == 0 {
                buf.push(byte);
                break;
            } else {
                buf.push(byte | 0x80);
            }
        }
    }
    #[cfg(not(feature = "compact-len"))]
    {
        buf.extend_from_slice(&(value as u64).to_le_bytes());
    }
}

#[inline]
fn read_len_from_slice_fixed(bytes: &[u8]) -> Result<(usize, usize), Error> {
    #[cfg(feature = "compact-len")]
    {
        if bytes.is_empty() {
            return Err(Error::LengthMismatch);
        }
        let mut result = 0u64;
        let mut shift = 0u32;
        for (idx, byte) in bytes.iter().copied().enumerate().take(MAX_VARINT_BYTES) {
            let payload = (byte & 0x7f) as u64;
            result |= payload << shift;
            if byte & 0x80 == 0 {
                let len = usize::try_from(result).map_err(|_| Error::LengthMismatch)?;
                return Ok((len, idx + 1));
            }
            shift += 7;
            if shift >= 64 {
                break;
            }
        }
        Err(Error::LengthMismatch)
    }
    #[cfg(not(feature = "compact-len"))]
    {
        if bytes.len() < 8 {
            return Err(Error::LengthMismatch);
        }
        let mut len_bytes = [0u8; 8];
        len_bytes.copy_from_slice(&bytes[..8]);
        let len =
            usize::try_from(u64::from_le_bytes(len_bytes)).map_err(|_| Error::LengthMismatch)?;
        Ok((len, 8))
    }
}

#[inline]
fn len_prefix_len(value: usize) -> usize {
    #[cfg(feature = "compact-len")]
    {
        let mut v = value as u64;
        let mut len = 1usize;
        while v >= 0x80 {
            v >>= 7;
            len += 1;
        }
        len
    }
    #[cfg(not(feature = "compact-len"))]
    {
        let _ = value;
        8
    }
}

#[inline]
fn take_bytes<'a>(body: &'a [u8], off: &mut usize, len: usize) -> Result<&'a [u8], Error> {
    let end = (*off).checked_add(len).ok_or_else(|| {
        Error::length_mismatch_detail(
            "advancing AoS cursor",
            *off,
            len,
            body.len().saturating_sub(*off),
        )
    })?;
    let slice = body.get(*off..end).ok_or_else(|| {
        Error::length_mismatch_detail(
            "reading AoS slice",
            *off,
            len,
            body.len().saturating_sub(*off),
        )
    })?;
    *off = end;
    Ok(slice)
}

#[inline]
fn take_byte(body: &[u8], off: &mut usize) -> Result<u8, Error> {
    let b = *body.get(*off).ok_or_else(|| {
        Error::length_mismatch_detail("reading AoS byte", *off, 1, body.len().saturating_sub(*off))
    })?;
    *off = (*off).checked_add(1).ok_or_else(|| {
        Error::length_mismatch_detail(
            "incrementing AoS cursor",
            *off,
            1,
            body.len().saturating_sub(*off),
        )
    })?;
    Ok(b)
}

#[inline]
fn read_len_prefix(body: &[u8], off: &mut usize) -> Result<usize, Error> {
    let tail = body.get(*off..).ok_or_else(|| {
        Error::length_mismatch_detail(
            "taking AoS length prefix tail",
            *off,
            body.len().saturating_sub(*off),
            body.len().saturating_sub(*off),
        )
    })?;
    let (len, used) = read_len_from_slice_fixed(tail)?;
    *off = (*off).checked_add(used).ok_or_else(|| {
        Error::length_mismatch_detail(
            "advancing AoS length prefix cursor",
            *off,
            used,
            body.len().saturating_sub(*off),
        )
    })?;
    Ok(len)
}

/// Version byte for ad‑hoc AoS bodies emitted by adaptive helpers.
///
/// The low nibble encodes the version of the AoS body layout, starting at 0x1.
/// High nibble is reserved (must be 0 for now).
pub const AOS_FORMAT_VERSION: u8 = 0x1;
const MIN_AOS_ROW_BYTES: usize = 10;

#[inline]
pub fn write_len_and_ver(buf: &mut Vec<u8>, n: usize) {
    // Length prefix according to compact-len feature, followed by a single
    // version byte (currently low nibble = 0x1, high nibble reserved 0).
    write_len_prefix(buf, n);
    buf.push(AOS_FORMAT_VERSION);
}

#[inline]
pub fn read_len_and_ver(body: &[u8]) -> Result<(usize, usize), Error> {
    let mut off = 0usize;
    let n = read_len_prefix(body, &mut off)?;
    let ver = take_byte(body, &mut off)?;
    // Require low nibble to match; high nibble must be zero for now.
    let low = ver & 0x0F;
    let high = ver & 0xF0;
    if low != AOS_FORMAT_VERSION || high != 0 {
        // Report the encountered version byte for diagnostics.
        return Err(Error::UnsupportedVersion {
            found: ver,
            expected: AOS_FORMAT_VERSION,
        });
    }
    let remaining = body.len().saturating_sub(off);
    let max_rows = remaining / MIN_AOS_ROW_BYTES;
    if n > max_rows {
        return Err(Error::LengthMismatch);
    }
    Ok((n, off))
}

// === (u64, u32, bool) ===

/// Encode AoS rows shaped as `(u64, u32, bool)` into an ad-hoc body.
pub fn encode_rows_u64_u32_bool(rows: &[(u64, u32, bool)]) -> Vec<u8> {
    let mut buf = Vec::new();
    write_len_and_ver(&mut buf, rows.len());
    for &(id, val, flag) in rows {
        buf.extend_from_slice(&id.to_le_bytes());
        buf.extend_from_slice(&val.to_le_bytes());
        buf.push(if flag { 1 } else { 0 });
    }
    buf
}

/// Decode AoS rows `(u64, u32, bool)` from an ad-hoc body.
pub fn decode_rows_u64_u32_bool(body: &[u8]) -> Result<Vec<(u64, u32, bool)>, Error> {
    let (n, mut off) = read_len_and_ver(body)?;
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let mut idb = [0u8; 8];
        idb.copy_from_slice(take_bytes(body, &mut off, 8)?);
        let id = u64::from_le_bytes(idb);
        let mut vb = [0u8; 4];
        vb.copy_from_slice(take_bytes(body, &mut off, 4)?);
        let flag = take_byte(body, &mut off)? != 0;
        out.push((id, u32::from_le_bytes(vb), flag));
    }
    Ok(out)
}

// === (u64, bytes/str, bool) ===

/// Encode AoS rows shaped as `(u64, &[u8], bool)` into an ad-hoc body.
pub fn encode_rows_u64_bytes_bool(rows: &[(u64, &[u8], bool)]) -> Vec<u8> {
    let mut buf = Vec::new();
    write_len_and_ver(&mut buf, rows.len());
    for &(id, bs, flag) in rows {
        buf.extend_from_slice(&id.to_le_bytes());
        write_len_prefix(&mut buf, bs.len());
        buf.extend_from_slice(bs);
        buf.push(if flag { 1 } else { 0 });
    }
    buf
}

/// Decode AoS rows `(u64, Vec<u8>, bool)` from an ad-hoc body.
pub fn decode_rows_u64_bytes_bool(body: &[u8]) -> Result<Vec<(u64, Vec<u8>, bool)>, Error> {
    let (n, mut off) = read_len_and_ver(body)?;
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let mut idb = [0u8; 8];
        idb.copy_from_slice(take_bytes(body, &mut off, 8)?);
        let id = u64::from_le_bytes(idb);
        let blen = read_len_prefix(body, &mut off)?;
        let bytes = take_bytes(body, &mut off, blen)?.to_vec();
        let flag = take_byte(body, &mut off)? != 0;
        out.push((id, bytes, flag));
    }
    Ok(out)
}

/// Encode AoS rows shaped as `(u64, &str, bool)` into an ad-hoc body.
pub fn encode_rows_u64_str_bool(rows: &[(u64, &str, bool)]) -> Vec<u8> {
    // Reuse the bytes version by converting to bytes on the fly.
    let borrowed: Vec<(u64, &[u8], bool)> = rows
        .iter()
        .map(|(id, s, b)| (*id, s.as_bytes(), *b))
        .collect();
    encode_rows_u64_bytes_bool(&borrowed)
}

/// Decode AoS rows `(u64, String, bool)` from an ad-hoc body.
pub fn decode_rows_u64_str_bool(body: &[u8]) -> Result<Vec<(u64, String, bool)>, Error> {
    let tmp = decode_rows_u64_bytes_bool(body)?;
    let mut out = Vec::with_capacity(tmp.len());
    for (id, bytes, flag) in tmp {
        let s = std::str::from_utf8(&bytes)
            .map_err(|_| Error::InvalidUtf8)?
            .to_string();
        out.push((id, s, flag));
    }
    Ok(out)
}

// === (u64, bytes/str, u32, bool) ===

/// Encode AoS rows shaped as `(u64, &[u8], u32, bool)` into an ad-hoc body.
pub fn encode_rows_u64_bytes_u32_bool(rows: &[(u64, &[u8], u32, bool)]) -> Vec<u8> {
    let mut buf = Vec::new();
    write_len_and_ver(&mut buf, rows.len());
    for &(id, bs, v, flag) in rows {
        buf.extend_from_slice(&id.to_le_bytes());
        write_len_prefix(&mut buf, bs.len());
        buf.extend_from_slice(bs);
        buf.extend_from_slice(&v.to_le_bytes());
        buf.push(if flag { 1 } else { 0 });
    }
    buf
}

/// Decode AoS rows `(u64, Vec<u8>, u32, bool)` from an ad-hoc body.
#[allow(clippy::type_complexity)]
pub fn decode_rows_u64_bytes_u32_bool(
    body: &[u8],
) -> Result<Vec<(u64, Vec<u8>, u32, bool)>, Error> {
    let (n, mut off) = read_len_and_ver(body)?;
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let mut idb = [0u8; 8];
        idb.copy_from_slice(take_bytes(body, &mut off, 8)?);
        let id = u64::from_le_bytes(idb);
        let blen = read_len_prefix(body, &mut off)?;
        let bytes = take_bytes(body, &mut off, blen)?.to_vec();
        let mut vb = [0u8; 4];
        vb.copy_from_slice(take_bytes(body, &mut off, 4)?);
        let flag = take_byte(body, &mut off)? != 0;
        out.push((id, bytes, u32::from_le_bytes(vb), flag));
    }
    Ok(out)
}

/// Encode AoS rows shaped as `(u64, &str, u32, bool)` into an ad-hoc body.
pub fn encode_rows_u64_str_u32_bool(rows: &[(u64, &str, u32, bool)]) -> Vec<u8> {
    let borrowed: Vec<(u64, &[u8], u32, bool)> = rows
        .iter()
        .map(|(id, s, v, b)| (*id, s.as_bytes(), *v, *b))
        .collect();
    encode_rows_u64_bytes_u32_bool(&borrowed)
}

/// Decode AoS rows `(u64, String, u32, bool)` from an ad-hoc body.
pub fn decode_rows_u64_str_u32_bool(body: &[u8]) -> Result<Vec<(u64, String, u32, bool)>, Error> {
    let tmp = decode_rows_u64_bytes_u32_bool(body)?;
    let mut out = Vec::with_capacity(tmp.len());
    for (id, bytes, v, flag) in tmp {
        let s = std::str::from_utf8(&bytes)
            .map_err(|_| Error::InvalidUtf8)?
            .to_string();
        out.push((id, s, v, flag));
    }
    Ok(out)
}

// === (u64, Option<&str>, bool) and (u64, Option<u32>, bool) ===

/// Encode AoS rows `(u64, Option<&str>, bool)` using the standard ad‑hoc body.
pub fn encode_rows_u64_optstr_bool(rows: &[(u64, Option<&str>, bool)]) -> Vec<u8> {
    let mut buf = Vec::new();
    write_len_and_ver(&mut buf, rows.len());
    for &(id, opt, flag) in rows {
        buf.extend_from_slice(&id.to_le_bytes());
        match opt {
            None => buf.push(0u8),
            Some(s) => {
                buf.push(1u8);
                write_len_prefix(&mut buf, s.len());
                buf.extend_from_slice(s.as_bytes());
            }
        }
        buf.push(if flag { 1 } else { 0 });
    }
    buf
}

/// Decode AoS rows `(u64, Option<String>, bool)` from the ad‑hoc body.
pub fn decode_rows_u64_optstr_bool(body: &[u8]) -> Result<Vec<(u64, Option<String>, bool)>, Error> {
    let (n, mut off) = read_len_and_ver(body)?;
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let mut idb = [0u8; 8];
        idb.copy_from_slice(take_bytes(body, &mut off, 8)?);
        let id = u64::from_le_bytes(idb);
        let tag = take_byte(body, &mut off)?;
        let opt = if tag == 0 {
            None
        } else {
            let slen = read_len_prefix(body, &mut off)?;
            let bytes = take_bytes(body, &mut off, slen)?;
            let s = std::str::from_utf8(bytes)
                .map_err(|_| Error::InvalidUtf8)?
                .to_string();
            Some(s)
        };
        let flag = take_byte(body, &mut off)? != 0;
        out.push((id, opt, flag));
    }
    Ok(out)
}

/// Encode AoS rows `(u64, Option<u32>, bool)` using the standard ad‑hoc body.
pub fn encode_rows_u64_optu32_bool(rows: &[(u64, Option<u32>, bool)]) -> Vec<u8> {
    let mut buf = Vec::new();
    write_len_and_ver(&mut buf, rows.len());
    for &(id, opt, flag) in rows {
        buf.extend_from_slice(&id.to_le_bytes());
        match opt {
            None => buf.push(0u8),
            Some(v) => {
                buf.push(1u8);
                buf.extend_from_slice(&v.to_le_bytes());
            }
        }
        buf.push(if flag { 1 } else { 0 });
    }
    buf
}

/// Decode AoS rows `(u64, Option<u32>, bool)` from the ad‑hoc body.
pub fn decode_rows_u64_optu32_bool(body: &[u8]) -> Result<Vec<(u64, Option<u32>, bool)>, Error> {
    let (n, mut off) = read_len_and_ver(body)?;
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let mut idb = [0u8; 8];
        idb.copy_from_slice(take_bytes(body, &mut off, 8)?);
        let id = u64::from_le_bytes(idb);
        let tag = take_byte(body, &mut off)?;
        let opt = if tag == 0 {
            None
        } else {
            let mut vb = [0u8; 4];
            vb.copy_from_slice(take_bytes(body, &mut off, 4)?);
            Some(u32::from_le_bytes(vb))
        };
        let flag = take_byte(body, &mut off)? != 0;
        out.push((id, opt, flag));
    }
    Ok(out)
}

// === (u64, enum(Name(String)|Code(u32)), bool) ===

use crate::columnar::{EnumBorrow, RowEnumOwned};

/// Encode AoS rows `(u64, enum{Name|Code}, bool)` using the historical ad-hoc body.
///
/// Note: enum AoS keeps a minimal header without the version byte to match
/// golden vectors and existing tests.
pub fn encode_rows_u64_enum_bool(rows: &[(u64, EnumBorrow<'_>, bool)]) -> Vec<u8> {
    let mut buf = Vec::new();
    // Length prefix only (no version byte) for enum AoS
    write_len_prefix(&mut buf, rows.len());
    for (id, en, flag) in rows.iter() {
        buf.extend_from_slice(&id.to_le_bytes());
        match en {
            EnumBorrow::Name(s) => {
                buf.push(0u8);
                write_len_prefix(&mut buf, s.len());
                buf.extend_from_slice(s.as_bytes());
            }
            EnumBorrow::Code(v) => {
                buf.push(1u8);
                buf.extend_from_slice(&v.to_le_bytes());
            }
        }
        buf.push(if *flag { 1 } else { 0 });
    }
    buf
}

/// Decode AoS rows `(u64, RowEnumOwned, bool)` from the ad-hoc body.
pub fn decode_rows_u64_enum_bool(body: &[u8]) -> Result<Vec<(u64, RowEnumOwned, bool)>, Error> {
    // Read length without version byte for enum AoS
    let mut off = 0usize;
    let n = read_len_prefix(body, &mut off)?;
    let prefix_len = len_prefix_len(0);
    let name_min = 8usize + 1 + prefix_len + 1;
    let code_min = 8usize + 1 + 4 + 1;
    let min_row = name_min.min(code_min);
    let remaining = body.len().saturating_sub(off);
    let max_rows = remaining / min_row;
    if n > max_rows {
        return Err(Error::LengthMismatch);
    }
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let mut idb = [0u8; 8];
        idb.copy_from_slice(take_bytes(body, &mut off, 8)?);
        let id = u64::from_le_bytes(idb);
        let tag = take_byte(body, &mut off)?;
        let en = if tag == 0 {
            let slen = read_len_prefix(body, &mut off)?;
            let bytes = take_bytes(body, &mut off, slen)?;
            let name = std::str::from_utf8(bytes)
                .map_err(|_| Error::InvalidUtf8)?
                .to_string();
            RowEnumOwned::Name(name)
        } else if tag == 1 {
            let mut vb = [0u8; 4];
            vb.copy_from_slice(take_bytes(body, &mut off, 4)?);
            RowEnumOwned::Code(u32::from_le_bytes(vb))
        } else {
            return Err(Error::invalid_tag("decoding AoS enum discriminant", tag));
        };
        let flag = take_byte(body, &mut off)? != 0;
        out.push((id, en, flag));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aos_bytes_bool_roundtrip() {
        let rows: Vec<(u64, Vec<u8>, bool)> = vec![
            (1, b"abc".to_vec(), true),
            (2, b"".to_vec(), false),
            (3, vec![0, 1, 2, 3, 4], true),
        ];
        let borrowed: Vec<(u64, &[u8], bool)> = rows
            .iter()
            .map(|(id, v, b)| (*id, v.as_slice(), *b))
            .collect();
        let enc = encode_rows_u64_bytes_bool(&borrowed);
        let dec = decode_rows_u64_bytes_bool(&enc).expect("decode");
        assert_eq!(rows, dec);
    }

    #[test]
    fn aos_str_bool_roundtrip() {
        let rows_owned: Vec<(u64, String, bool)> = vec![
            (1, "alpha".into(), true),
            (2, "".into(), false),
            (3, "βeta".into(), true),
        ];
        let borrowed: Vec<(u64, &str, bool)> = rows_owned
            .iter()
            .map(|(id, s, b)| (*id, s.as_str(), *b))
            .collect();
        let enc = encode_rows_u64_str_bool(&borrowed);
        let dec = decode_rows_u64_str_bool(&enc).expect("decode");
        assert_eq!(rows_owned, dec);
    }

    #[test]
    fn aos_u32_bool_roundtrip() {
        let rows: Vec<(u64, u32, bool)> = vec![(1, 5, true), (2, 17, false), (9, 0, true)];
        let enc = encode_rows_u64_u32_bool(&rows);
        let dec = decode_rows_u64_u32_bool(&enc).expect("decode");
        assert_eq!(rows, dec);
    }

    #[test]
    fn aos_str_u32_bool_roundtrip() {
        let rows_owned: Vec<(u64, String, u32, bool)> =
            vec![(10, "name".into(), 5, true), (11, "x".into(), 0, false)];
        let borrowed: Vec<(u64, &str, u32, bool)> = rows_owned
            .iter()
            .map(|(id, s, v, b)| (*id, s.as_str(), *v, *b))
            .collect();
        let enc = encode_rows_u64_str_u32_bool(&borrowed);
        let dec = decode_rows_u64_str_u32_bool(&enc).expect("decode");
        assert_eq!(rows_owned, dec);
    }

    #[test]
    fn aos_opt_str_bool_roundtrip() {
        let rows: Vec<(u64, Option<&str>, bool)> = vec![(1, Some("x"), true), (2, None, false)];
        let bytes = encode_rows_u64_optstr_bool(&rows);
        let decoded = decode_rows_u64_optstr_bool(&bytes).expect("decode");
        let expected: Vec<(u64, Option<String>, bool)> =
            vec![(1, Some("x".to_string()), true), (2, None, false)];
        assert_eq!(decoded, expected);
    }

    #[test]
    fn aos_opt_u32_bool_roundtrip() {
        let rows: Vec<(u64, Option<u32>, bool)> = vec![(10, Some(7), true), (11, None, false)];
        let bytes = encode_rows_u64_optu32_bool(&rows);
        let decoded = decode_rows_u64_optu32_bool(&bytes).expect("decode");
        assert_eq!(decoded, vec![(10, Some(7), true), (11, None, false)]);
    }

    #[test]
    fn aos_header_rejects_excessive_row_count() {
        let mut body = Vec::new();
        write_len_and_ver(&mut body, 1);
        let result = read_len_and_ver(&body);
        assert!(matches!(result, Err(Error::LengthMismatch)));
    }

    #[test]
    fn aos_enum_rejects_excessive_row_count() {
        let mut body = Vec::new();
        write_len_prefix(&mut body, 1);
        let result = decode_rows_u64_enum_bool(&body);
        assert!(matches!(result, Err(Error::LengthMismatch)));
    }
}
