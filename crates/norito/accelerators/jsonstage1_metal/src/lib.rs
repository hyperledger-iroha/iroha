//! jsonstage1_metal: cdylib exporting JSON Stage-1 structural tape builder via Metal.
//!
//! C ABI: `json_stage1_build_tape(input_ptr, input_len, out_offsets, out_capacity, out_len)`
//! Returns 0 on success, 3 when Metal is unavailable, and non-zero on failure.

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
unsafe extern "C" {
    fn json_stage1_build_tape_metal_impl(
        input_ptr: *const u8,
        input_len: usize,
        out_offsets: *mut u32,
        out_capacity: usize,
        out_len: *mut usize,
    ) -> i32;

    fn norito_crc64_metal_impl(input_ptr: *const u8, input_len: usize, out_crc: *mut u64) -> i32;
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct NoritoSequenceSpan {
    start: usize,
    end: usize,
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
unsafe extern "C" {
    fn norito_sequence_plan_metal_impl(
        input_ptr: *const u8,
        input_len: usize,
        flags: u8,
        layout_kind: u32,
        out_spans: *mut NoritoSequenceSpan,
        out_capacity: usize,
        out_count: *mut usize,
        out_used: *mut usize,
    ) -> i32;
}

#[allow(dead_code)]
const RC_OK: i32 = 0;
const RC_INVALID: i32 = 1;
#[allow(dead_code)]
const RC_NO_SPACE: i32 = 2;
#[allow(dead_code)]
const RC_UNAVAILABLE: i32 = 3;
#[allow(dead_code)]
const RC_BACKEND_ERROR: i32 = 4;
#[allow(dead_code)]
const FLAG_COMPACT_LEN: u8 = 0x02;
#[allow(dead_code)]
const LAYOUT_LENGTH_PREFIXED: u32 = 0;
#[allow(dead_code)]
const LAYOUT_FIXED_OFFSETS: u32 = 1;

/// Build a structural tape (offsets) for the given JSON input.
///
/// This entry point reports Metal availability directly. Scalar fallback is
/// owned by the Norito caller so helper registration cannot confuse CPU work
/// with an accelerated backend.
///
/// # Safety
/// The caller must ensure all pointers are valid for the given lengths and
/// refer to writable/readable memory ranges as appropriate.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn json_stage1_build_tape(
    input_ptr: *const u8,
    input_len: usize,
    out_offsets: *mut u32,
    out_capacity: usize,
    out_len: *mut usize,
) -> i32 {
    if input_ptr.is_null() || out_offsets.is_null() || out_len.is_null() {
        return RC_INVALID;
    }
    if input_len > u32::MAX as usize {
        return RC_INVALID;
    }
    if input_len == 0 {
        unsafe {
            *out_len = 0;
        }
        return RC_OK;
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        unsafe {
            json_stage1_build_tape_metal_impl(
                input_ptr,
                input_len,
                out_offsets,
                out_capacity,
                out_len,
            )
        }
    }

    #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
    {
        let _ = (input_len, out_capacity);
        RC_UNAVAILABLE
    }
}

#[cfg(test)]
fn crc64_raw(bytes: &[u8], init: u64) -> u64 {
    const POLY: u64 = 0xC96C_5795_D787_0F42;
    let mut crc = init;
    for &b in bytes {
        crc ^= b as u64;
        for _ in 0..8 {
            if (crc & 1) != 0 {
                crc = (crc >> 1) ^ POLY;
            } else {
                crc >>= 1;
            }
        }
    }
    crc
}

#[cfg(test)]
fn crc64_cpu(bytes: &[u8]) -> u64 {
    const INIT: u64 = 0xFFFF_FFFF_FFFF_FFFF;
    const XOR_OUT: u64 = 0xFFFF_FFFF_FFFF_FFFF;
    let crc = crc64_raw(bytes, INIT);
    crc ^ XOR_OUT
}

/// Compute CRC64-XZ for the provided buffer using Metal.
///
/// This helper reports backend unavailability or failure directly. The Norito
/// caller owns deterministic SIMD/CPU fallback.
///
/// # Safety
/// The caller must ensure the pointers are valid for the supplied lengths.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn norito_crc64_metal(
    input_ptr: *const u8,
    input_len: usize,
    out_crc: *mut u64,
) -> i32 {
    if input_ptr.is_null() || out_crc.is_null() {
        return RC_INVALID;
    }
    if input_len == 0 {
        unsafe {
            *out_crc = 0;
        }
        return RC_OK;
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        unsafe { norito_crc64_metal_impl(input_ptr, input_len, out_crc) }
    }

    #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
    {
        let _ = input_len;
        RC_UNAVAILABLE
    }
}

/// Plan Norito binary sequence element spans.
///
/// Returns 0 on success, 1 for invalid input, 2 when `out_capacity` is too
/// small, 3 when no helper backend is available, and 4 for backend failure.
///
/// # Safety
/// The caller must ensure the input and output pointers are valid for the
/// supplied lengths.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn norito_binary_sequence_plan(
    input_ptr: *const u8,
    input_len: usize,
    flags: u8,
    layout_kind: u32,
    out_spans: *mut NoritoSequenceSpan,
    out_capacity: usize,
    out_count: *mut usize,
    out_used: *mut usize,
) -> i32 {
    if input_ptr.is_null() || out_count.is_null() || out_used.is_null() {
        return RC_INVALID;
    }
    if out_capacity > 0 && out_spans.is_null() {
        return RC_INVALID;
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        unsafe {
            norito_sequence_plan_metal_impl(
                input_ptr,
                input_len,
                flags,
                layout_kind,
                out_spans,
                out_capacity,
                out_count,
                out_used,
            )
        }
    }

    #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
    {
        let _ = (input_len, flags, layout_kind, out_spans, out_capacity);
        unsafe {
            *out_count = 0;
            *out_used = 0;
        }
        RC_UNAVAILABLE
    }
}

#[cfg(test)]
#[allow(dead_code)]
enum PlanError {
    Invalid,
    Unavailable,
    Backend,
}

#[cfg(test)]
#[allow(dead_code)]
fn plan_sequence_cpu(
    bytes: &[u8],
    flags: u8,
    layout_kind: u32,
) -> Result<(Vec<NoritoSequenceSpan>, usize), PlanError> {
    // TODO: replace this ABI reference path with Metal kernels once the
    // sequence planner workgroup layout is tuned for large transaction batches.
    let (count, mut offset) = read_seq_len(bytes)?;
    match layout_kind {
        LAYOUT_LENGTH_PREFIXED => {
            let mut spans = Vec::new();
            spans.try_reserve(count).map_err(|_| PlanError::Invalid)?;
            for _ in 0..count {
                let tail = bytes.get(offset..).ok_or(PlanError::Invalid)?;
                let (elem_len, header_len) = read_value_len(tail, flags)?;
                let start = offset.checked_add(header_len).ok_or(PlanError::Invalid)?;
                let end = start.checked_add(elem_len).ok_or(PlanError::Invalid)?;
                if end > bytes.len() {
                    return Err(PlanError::Invalid);
                }
                spans.push(NoritoSequenceSpan { start, end });
                offset = end;
            }
            Ok((spans, offset))
        }
        LAYOUT_FIXED_OFFSETS => {
            let entries = count.checked_add(1).ok_or(PlanError::Invalid)?;
            let table_len = entries.checked_mul(8).ok_or(PlanError::Invalid)?;
            let table_end = offset.checked_add(table_len).ok_or(PlanError::Invalid)?;
            let table = bytes.get(offset..table_end).ok_or(PlanError::Invalid)?;
            if read_u64(table, 0)? != 0 {
                return Err(PlanError::Invalid);
            }
            let data_len =
                usize::try_from(read_u64(table, count)?).map_err(|_| PlanError::Invalid)?;
            let data_start = table_end;
            let data_end = data_start.checked_add(data_len).ok_or(PlanError::Invalid)?;
            if data_end > bytes.len() {
                return Err(PlanError::Invalid);
            }
            let mut spans = Vec::new();
            spans.try_reserve(count).map_err(|_| PlanError::Invalid)?;
            let mut prev = 0usize;
            for idx in 0..count {
                let next =
                    usize::try_from(read_u64(table, idx + 1)?).map_err(|_| PlanError::Invalid)?;
                if next < prev || next > data_len {
                    return Err(PlanError::Invalid);
                }
                spans.push(NoritoSequenceSpan {
                    start: data_start.checked_add(prev).ok_or(PlanError::Invalid)?,
                    end: data_start.checked_add(next).ok_or(PlanError::Invalid)?,
                });
                prev = next;
            }
            if prev != data_len {
                return Err(PlanError::Invalid);
            }
            Ok((spans, data_end))
        }
        _ => Err(PlanError::Unavailable),
    }
}

#[cfg(test)]
#[allow(dead_code)]
fn read_seq_len(bytes: &[u8]) -> Result<(usize, usize), PlanError> {
    let raw = read_u64(bytes, 0)?;
    let len = usize::try_from(raw).map_err(|_| PlanError::Invalid)?;
    Ok((len, 8))
}

#[cfg(test)]
#[allow(dead_code)]
fn read_u64(bytes: &[u8], idx: usize) -> Result<u64, PlanError> {
    let start = idx.checked_mul(8).ok_or(PlanError::Invalid)?;
    let end = start.checked_add(8).ok_or(PlanError::Invalid)?;
    let mut buf = [0u8; 8];
    buf.copy_from_slice(bytes.get(start..end).ok_or(PlanError::Invalid)?);
    Ok(u64::from_le_bytes(buf))
}

#[cfg(test)]
#[allow(dead_code)]
fn read_value_len(bytes: &[u8], flags: u8) -> Result<(usize, usize), PlanError> {
    if (flags & FLAG_COMPACT_LEN) == 0 {
        return read_seq_len(bytes);
    }
    let (value, used) = decode_varint(bytes)?;
    let len = usize::try_from(value).map_err(|_| PlanError::Invalid)?;
    Ok((len, used))
}

#[cfg(test)]
#[allow(dead_code)]
fn decode_varint(bytes: &[u8]) -> Result<(u64, usize), PlanError> {
    let mut result = 0u64;
    let mut shift = 0u32;
    for (idx, byte) in bytes.iter().copied().enumerate().take(10) {
        let payload = (byte & 0x7f) as u64;
        if shift == 63 && payload > 1 {
            return Err(PlanError::Invalid);
        }
        result |= payload << shift;
        if byte & 0x80 == 0 {
            let used = idx + 1;
            if used != varint_len(result) {
                return Err(PlanError::Invalid);
            }
            return Ok((result, used));
        }
        shift += 7;
    }
    Err(PlanError::Invalid)
}

#[cfg(test)]
#[allow(dead_code)]
fn varint_len(mut value: u64) -> usize {
    let mut len = 1usize;
    while value >= 0x80 {
        value >>= 7;
        len += 1;
    }
    len
}

#[cfg(test)]
mod tests {
    use super::{
        NoritoSequenceSpan, crc64_cpu, crc64_raw, json_stage1_build_tape,
        norito_binary_sequence_plan, norito_crc64_metal,
    };

    const CRC64_INIT: u64 = 0xFFFF_FFFF_FFFF_FFFF;
    const CRC64_XOR_OUT: u64 = 0xFFFF_FFFF_FFFF_FFFF;

    fn skip_if_unavailable(rc: i32, helper: &str) -> bool {
        if rc == super::RC_UNAVAILABLE {
            eprintln!("{helper} unavailable; skipping Metal-only assertion");
            true
        } else {
            false
        }
    }

    fn reference_offsets(bytes: &[u8]) -> Vec<u32> {
        let mut offsets = Vec::new();
        let mut in_str = false;
        let mut backslash_run = 0usize;
        for (idx, &byte) in bytes.iter().enumerate() {
            if in_str {
                match byte {
                    b'\\' => {
                        backslash_run = backslash_run.saturating_add(1);
                    }
                    b'"' => {
                        if backslash_run & 1 == 0 {
                            in_str = false;
                            offsets.push(idx as u32);
                        }
                        backslash_run = 0;
                    }
                    _ => {
                        backslash_run = 0;
                    }
                }
            } else {
                match byte {
                    b'"' => {
                        in_str = true;
                        backslash_run = 0;
                        offsets.push(idx as u32);
                    }
                    b'{' | b'}' | b'[' | b']' | b':' | b',' => offsets.push(idx as u32),
                    _ => {}
                }
            }
        }
        offsets
    }

    #[test]
    fn basic_offsets() {
        let s = b"{\"a\":1}";
        let mut out = vec![0u32; 16];
        let mut len = 0usize;
        let rc = unsafe {
            json_stage1_build_tape(s.as_ptr(), s.len(), out.as_mut_ptr(), out.len(), &mut len)
        };
        if skip_if_unavailable(rc, "jsonstage1_metal") {
            return;
        }
        assert_eq!(rc, super::RC_OK);
        out.truncate(len);
        assert_eq!(out, reference_offsets(s));
    }

    #[test]
    fn stage1_capacity_reports_required_len_when_available() {
        let s = br#"{"capacity":[1,2,3],"quoted":"a\"b"}"#;
        let expected = reference_offsets(s);
        let mut out = [0u32; 2];
        let mut len = 0usize;
        let rc = unsafe {
            json_stage1_build_tape(s.as_ptr(), s.len(), out.as_mut_ptr(), out.len(), &mut len)
        };
        if skip_if_unavailable(rc, "jsonstage1_metal") {
            return;
        }
        assert_eq!(rc, super::RC_NO_SPACE);
        assert_eq!(len, expected.len());
    }

    #[test]
    fn crc64_round_trip() {
        let data = b"123456789";
        let mut out = 0u64;
        let rc = unsafe { norito_crc64_metal(data.as_ptr(), data.len(), &mut out) };
        if skip_if_unavailable(rc, "jsonstage1_metal CRC64") {
            return;
        }
        assert_eq!(rc, super::RC_OK);
        assert_eq!(out, 0x995D_C9BB_DF19_39FA);
    }

    #[test]
    fn crc64_large_matches_cpu() {
        let data = vec![0xAAu8; 48 * 1024];
        let mut out = 0u64;
        let rc = unsafe { norito_crc64_metal(data.as_ptr(), data.len(), &mut out) };
        if skip_if_unavailable(rc, "jsonstage1_metal CRC64") {
            return;
        }
        assert_eq!(rc, super::RC_OK);
        assert_eq!(out, crc64_cpu(&data));
    }

    #[test]
    fn public_ffi_rejects_null_pointers() {
        let s = b"{\"a\":1}";
        let mut out = [0u32; 8];
        let mut len = 0usize;
        let mut crc = 0u64;

        let rc = unsafe {
            json_stage1_build_tape(
                std::ptr::null(),
                s.len(),
                out.as_mut_ptr(),
                out.len(),
                &mut len,
            )
        };
        assert_eq!(rc, super::RC_INVALID);

        let rc = unsafe {
            json_stage1_build_tape(s.as_ptr(), s.len(), std::ptr::null_mut(), 0, &mut len)
        };
        assert_eq!(rc, super::RC_INVALID);

        let rc = unsafe {
            json_stage1_build_tape(
                s.as_ptr(),
                s.len(),
                out.as_mut_ptr(),
                out.len(),
                std::ptr::null_mut(),
            )
        };
        assert_eq!(rc, super::RC_INVALID);

        let rc = unsafe { norito_crc64_metal(std::ptr::null(), s.len(), &mut crc) };
        assert_eq!(rc, super::RC_INVALID);

        let rc = unsafe { norito_crc64_metal(s.as_ptr(), s.len(), std::ptr::null_mut()) };
        assert_eq!(rc, super::RC_INVALID);
    }

    #[test]
    fn public_ffi_handles_empty_inputs_without_device_work() {
        let input: &[u8] = b"";
        let mut offsets = [123u32; 1];
        let mut len = usize::MAX;
        let rc = unsafe {
            json_stage1_build_tape(
                input.as_ptr(),
                input.len(),
                offsets.as_mut_ptr(),
                offsets.len(),
                &mut len,
            )
        };
        assert_eq!(rc, super::RC_OK);
        assert_eq!(len, 0);

        let mut crc = u64::MAX;
        let rc = unsafe { norito_crc64_metal(input.as_ptr(), input.len(), &mut crc) };
        assert_eq!(rc, super::RC_OK);
        assert_eq!(crc, 0);
    }

    #[test]
    fn public_stage1_rejects_lengths_outside_offset_abi() {
        let input = [0u8; 1];
        let mut offsets = [0u32; 1];
        let mut len = 0usize;
        let rc = unsafe {
            json_stage1_build_tape(
                input.as_ptr(),
                u32::MAX as usize + 1,
                offsets.as_mut_ptr(),
                offsets.len(),
                &mut len,
            )
        };
        assert_eq!(rc, super::RC_INVALID);
    }

    #[test]
    fn binary_sequence_plan_length_prefixed_compact() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&2u64.to_le_bytes());
        bytes.push(1);
        bytes.push(b'a');
        bytes.extend_from_slice(&[0x82, 0x01]);
        bytes.extend(std::iter::repeat_n(0x55, 130));

        let mut spans = vec![NoritoSequenceSpan { start: 0, end: 0 }; 2];
        let mut count = 0usize;
        let mut used = 0usize;
        let rc = unsafe {
            norito_binary_sequence_plan(
                bytes.as_ptr(),
                bytes.len(),
                super::FLAG_COMPACT_LEN,
                super::LAYOUT_LENGTH_PREFIXED,
                spans.as_mut_ptr(),
                spans.len(),
                &mut count,
                &mut used,
            )
        };
        if skip_if_unavailable(rc, "jsonstage1_metal sequence planner") {
            return;
        }
        assert_eq!(rc, super::RC_OK);
        spans.truncate(count);
        assert_eq!(spans[0].start, 9);
        assert_eq!(spans[0].end, 10);
        assert_eq!(spans[1].start, 12);
        assert_eq!(spans[1].end, 142);
        assert_eq!(used, bytes.len());
    }

    #[test]
    fn crc64_chunked_matches_full_crc() {
        let data = vec![0x7Bu8; 64 * 1024 + 17];
        let mut combined = CRC64_INIT;
        for chunk in data.chunks(16 * 1024) {
            let part = crc64_raw(chunk, 0);
            combined = crc64_combine_raw(combined, part, chunk.len());
        }
        assert_eq!(combined ^ CRC64_XOR_OUT, crc64_cpu(&data));
    }

    fn crc64_combine_raw(crc1: u64, crc2: u64, len2: usize) -> u64 {
        let shifted = crc64_shift(crc1, len2);
        shifted ^ crc2
    }

    fn crc64_shift(mut crc1: u64, len2: usize) -> u64 {
        const POLY: u64 = 0xC96C_5795_D787_0F42;
        if len2 == 0 {
            return crc1;
        }

        let mut mat = [0u64; 64];
        let mut square = [0u64; 64];
        let mut row = 1u64;
        mat[0] = POLY;
        for slot in mat.iter_mut().skip(1) {
            *slot = row;
            row <<= 1;
        }

        fn gf2_matrix_times(mat: &[u64; 64], mut vec: u64) -> u64 {
            let mut sum = 0;
            let mut idx = 0;
            while vec != 0 {
                if vec & 1 == 1 {
                    sum ^= mat[idx];
                }
                vec >>= 1;
                idx += 1;
            }
            sum
        }

        fn gf2_matrix_square(square: &mut [u64; 64], mat: &[u64; 64]) {
            for n in 0..64 {
                square[n] = gf2_matrix_times(mat, mat[n]);
            }
        }

        let mut len_bits = len2 as u64 * 8;
        while len_bits != 0 {
            if len_bits & 1 != 0 {
                crc1 = gf2_matrix_times(&mat, crc1);
            }
            gf2_matrix_square(&mut square, &mat);
            mat = square;
            len_bits >>= 1;
        }

        crc1
    }
}
