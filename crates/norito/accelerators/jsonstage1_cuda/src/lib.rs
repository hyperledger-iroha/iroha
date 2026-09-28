//! jsonstage1_cuda: cdylib exporting JSON Stage-1 structural tape builder and
//! CRC64 helpers (CUDA-accelerated when compiled with the `cuda-kernel`
//! feature and a working nvcc toolchain).
//!
//! C ABI: `json_stage1_build_tape(input_ptr, input_len, out_offsets, out_capacity, out_len)`
//! Returns 0 on success, non-zero on failure.
#[cfg(crc64_cuda_available)]
unsafe extern "C" {
    fn norito_crc64_cuda_impl(input_ptr: *const u8, input_len: usize, out_crc: *mut u64) -> i32;
}
#[cfg(jsonstage1_cuda_available)]
unsafe extern "C" {
    fn json_stage1_build_tape_cuda_impl(
        input_ptr: *const u8,
        input_len: usize,
        out_offsets: *mut u32,
        out_capacity: usize,
        out_len: *mut usize,
    ) -> i32;
}
#[allow(dead_code)]
const RC_OK: i32 = 0;
const RC_INVALID: i32 = 1;
#[allow(dead_code)]
const RC_NO_SPACE: i32 = 2;
#[cfg_attr(all(jsonstage1_cuda_available, crc64_cuda_available), allow(dead_code))]
const RC_GPU_UNAVAILABLE: i32 = 3;
#[allow(dead_code)]
const RC_BACKEND_ERROR: i32 = 4;
const FLAG_COMPACT_LEN: u8 = 0x02;
#[repr(C)]
#[derive(Clone, Copy)]
pub struct NoritoSequenceSpan {
    start: usize,
    end: usize,
}
#[cfg(jsonstage1_cuda_available)]
unsafe extern "C" {
    fn norito_sequence_plan_cuda_impl(
        input_ptr: *const u8,
        input_len: usize,
        flags: u8,
        out_spans: *mut NoritoSequenceSpan,
        out_capacity: usize,
        out_count: *mut usize,
        out_used: *mut usize,
    ) -> i32;
}
#[cfg_attr(any(not(test), jsonstage1_cuda_available), allow(dead_code))]
fn scan_structural_offsets(mut bytes: &[u8], mut emit: impl FnMut(u32)) -> usize {
    let mut count = 0usize;
    let mut base = 0usize;
    let mut in_str = false;
    let mut backslash_run = 0usize;
    while !bytes.is_empty() {
        let c = bytes[0];
        if in_str {
            if c == b'\\' {
                backslash_run += 1;
                bytes = &bytes[1..];
                base += 1;
                continue;
            }
            if c == b'"' {
                let escaped = (backslash_run & 1) != 0;
                backslash_run = 0;
                if !escaped {
                    emit(base as u32);
                    count += 1;
                    in_str = false;
                }
                bytes = &bytes[1..];
                base += 1;
                continue;
            }
            backslash_run = 0;
            bytes = &bytes[1..];
            base += 1;
            continue;
        }
        match c {
            b'"' => {
                emit(base as u32);
                count += 1;
                in_str = true;
                backslash_run = 0;
            }
            b'{' | b'}' | b'[' | b']' | b':' | b',' => {
                emit(base as u32);
                count += 1;
            }
            _ => {}
        }
        bytes = &bytes[1..];
        base += 1;
    }
    count
}
/// Build a structural tape (offsets) for the given JSON input.
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
    #[cfg(jsonstage1_cuda_available)]
    unsafe {
        json_stage1_build_tape_cuda_impl(input_ptr, input_len, out_offsets, out_capacity, out_len)
    }
    #[cfg(not(jsonstage1_cuda_available))]
    {
        let _ = input_len;
        let _ = out_capacity;
        RC_GPU_UNAVAILABLE
    }
}
#[cfg_attr(any(not(test), jsonstage1_cuda_available), allow(dead_code))]
unsafe fn json_stage1_build_tape_cpu(
    input_ptr: *const u8,
    input_len: usize,
    out_offsets: *mut u32,
    out_capacity: usize,
    out_len: *mut usize,
) -> i32 {
    if input_ptr.is_null() || out_offsets.is_null() || out_len.is_null() {
        return RC_INVALID;
    }
    let bytes = unsafe { std::slice::from_raw_parts(input_ptr, input_len) };
    let need = scan_structural_offsets(bytes, |_| {});
    unsafe {
        *out_len = need;
    }
    if need > out_capacity {
        return 2;
    }
    let out = unsafe { std::slice::from_raw_parts_mut(out_offsets, need) };
    let mut written = 0usize;
    scan_structural_offsets(bytes, |offset| {
        out[written] = offset;
        written += 1;
    });
    debug_assert_eq!(written, need);
    0
}
#[cfg_attr(any(not(test), crc64_cuda_available), allow(dead_code))]
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
#[cfg_attr(any(not(test), crc64_cuda_available), allow(dead_code))]
fn crc64_cpu(bytes: &[u8]) -> u64 {
    const INIT: u64 = 0xFFFF_FFFF_FFFF_FFFF;
    const XOR_OUT: u64 = 0xFFFF_FFFF_FFFF_FFFF;
    let crc = crc64_raw(bytes, INIT);
    crc ^ XOR_OUT
}
/// Compute CRC64-XZ using the CUDA helper.
///
/// # Safety
/// The caller must ensure the input and output pointers are valid for the
/// given lengths.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn norito_crc64_cuda(
    input_ptr: *const u8,
    input_len: usize,
    out_crc: *mut u64,
) -> i32 {
    if input_ptr.is_null() || out_crc.is_null() {
        return RC_INVALID;
    }
    #[cfg(crc64_cuda_available)]
    unsafe {
        norito_crc64_cuda_impl(input_ptr, input_len, out_crc)
    }
    #[cfg(not(crc64_cuda_available))]
    {
        let _ = input_len;
        RC_GPU_UNAVAILABLE
    }
}
/// Plan the element spans of a length-prefixed Norito binary sequence through
/// the helper ABI.
///
/// The input is `[u64 count][len][payload]...`, where each element length is a
/// fixed-width `u64` or, when `flags` carries `COMPACT_LEN` (`0x02`), a
/// canonical varint. Any other flag bit is rejected as invalid input.
///
/// Returns 0 on success, 1 for invalid input, 2 when `out_capacity` is too
/// small, 3 when no CUDA backend is available, and 4 for backend failure.
///
/// # Safety
/// The caller must ensure all pointers are valid for their supplied lengths.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn norito_length_prefixed_sequence_plan(
    input_ptr: *const u8,
    input_len: usize,
    flags: u8,
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
    if flags & !FLAG_COMPACT_LEN != 0 {
        return RC_INVALID;
    }
    #[cfg(jsonstage1_cuda_available)]
    {
        unsafe {
            norito_sequence_plan_cuda_impl(
                input_ptr,
                input_len,
                flags,
                out_spans,
                out_capacity,
                out_count,
                out_used,
            )
        }
    }
    #[cfg(not(jsonstage1_cuda_available))]
    {
        let _ = (input_len, out_spans, out_capacity);
        unsafe {
            *out_count = 0;
            *out_used = 0;
        }
        RC_GPU_UNAVAILABLE
    }
}
#[cfg(test)]
#[allow(dead_code)]
#[derive(Debug, PartialEq, Eq)]
enum PlanError {
    Invalid,
}
#[cfg(test)]
#[allow(dead_code)]
fn plan_sequence_cpu(
    bytes: &[u8],
    flags: u8,
) -> Result<(Vec<NoritoSequenceSpan>, usize), PlanError> {
    // Keep this host implementation as the deterministic reference used by
    // CUDA parity and malformed-input tests.
    let (count, mut offset) = read_seq_len(bytes)?;
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
        FLAG_COMPACT_LEN, NoritoSequenceSpan, PlanError, RC_GPU_UNAVAILABLE, RC_INVALID,
        RC_NO_SPACE, crc64_cpu, crc64_raw, json_stage1_build_tape, json_stage1_build_tape_cpu,
        norito_crc64_cuda, norito_length_prefixed_sequence_plan, plan_sequence_cpu,
        scan_structural_offsets,
    };
    const CRC_123456789: u64 = 0x995D_C9BB_DF19_39FA;
    const CHUNK_SIZE: usize = 16 * 1024;
    const CRC64_INIT: u64 = 0xFFFF_FFFF_FFFF_FFFF;
    const CRC64_XOR_OUT: u64 = 0xFFFF_FFFF_FFFF_FFFF;
    fn cuda_required() -> bool {
        std::env::var_os("JSONSTAGE1_CUDA_REQUIRE").is_some()
    }
    fn skip_if_unavailable(rc: i32, helper: &str) -> bool {
        if rc == RC_GPU_UNAVAILABLE {
            if cuda_required() {
                panic!(
                    "{helper} unavailable while JSONSTAGE1_CUDA_REQUIRE is set; build with nvcc and run on a CUDA host"
                );
            }
            eprintln!("{helper} unavailable; skipping CUDA-only assertion");
            true
        } else {
            false
        }
    }
    fn reference_offsets(input: &[u8]) -> Vec<u32> {
        let mut expected = Vec::new();
        scan_structural_offsets(input, |offset| expected.push(offset));
        expected
    }
    fn lcg_payload(len: usize, mut seed: u64) -> Vec<u8> {
        let mut out = Vec::with_capacity(len);
        for _ in 0..len {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            out.push((seed >> 32) as u8);
        }
        out
    }
    fn compact_sequence(elements: &[&[u8]]) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&(elements.len() as u64).to_le_bytes());
        for element in elements {
            let mut len = element.len() as u64;
            while len >= 0x80 {
                bytes.push((len as u8 & 0x7f) | 0x80);
                len >>= 7;
            }
            bytes.push(len as u8);
            bytes.extend_from_slice(element);
        }
        bytes
    }
    fn fixed_width_sequence(elements: &[&[u8]]) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&(elements.len() as u64).to_le_bytes());
        for element in elements {
            bytes.extend_from_slice(&(element.len() as u64).to_le_bytes());
            bytes.extend_from_slice(element);
        }
        bytes
    }
    #[test]
    fn basic_offsets() {
        let s = b"{\"a\":1}";
        let mut out = vec![0u32; 16];
        let mut len = 0usize;
        let rc = unsafe {
            json_stage1_build_tape_cpu(s.as_ptr(), s.len(), out.as_mut_ptr(), out.len(), &mut len)
        };
        assert_eq!(rc, 0);
        out.truncate(len);
        assert_eq!(out, vec![0, 1, 3, 4, 6]);
    }
    #[test]
    fn escaped_quotes_keep_string_state_aligned() {
        let s = b"{\"a\":\"b\\\"c\"}";
        let mut out = vec![0u32; 16];
        let mut len = 0usize;
        let rc = unsafe {
            json_stage1_build_tape_cpu(s.as_ptr(), s.len(), out.as_mut_ptr(), out.len(), &mut len)
        };
        assert_eq!(rc, 0);
        out.truncate(len);
        assert_eq!(out, vec![0, 1, 3, 4, 5, 10, 11]);
    }
    #[test]
    fn even_backslashes_do_not_escape_quote() {
        let s = br#"{"a":"b\\" ,"c":1}"#;
        let mut out = vec![0u32; 32];
        let mut len = 0usize;
        let rc = unsafe {
            json_stage1_build_tape_cpu(s.as_ptr(), s.len(), out.as_mut_ptr(), out.len(), &mut len)
        };
        assert_eq!(rc, 0);
        out.truncate(len);
        assert_eq!(out, vec![0, 1, 3, 4, 5, 9, 11, 12, 14, 15, 17]);
    }
    #[test]
    fn backslashes_outside_strings_do_not_escape_quotes() {
        let s = br#"{\"a\":1}"#;
        let mut out = vec![0u32; 16];
        let mut len = 0usize;
        let rc = unsafe {
            json_stage1_build_tape_cpu(s.as_ptr(), s.len(), out.as_mut_ptr(), out.len(), &mut len)
        };
        assert_eq!(rc, 0);
        out.truncate(len);
        assert_eq!(out, vec![0, 2]);
    }
    #[test]
    fn capacity_errors_still_report_required_length() {
        let s = b"{\"a\":1}";
        let mut out = [0u32; 2];
        let mut len = 0usize;
        let rc = unsafe {
            json_stage1_build_tape_cpu(s.as_ptr(), s.len(), out.as_mut_ptr(), out.len(), &mut len)
        };
        assert_eq!(rc, 2);
        assert_eq!(len, 5);
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
        assert_eq!(rc, RC_INVALID);
        let rc = unsafe {
            json_stage1_build_tape(s.as_ptr(), s.len(), std::ptr::null_mut(), 0, &mut len)
        };
        assert_eq!(rc, RC_INVALID);
        let rc = unsafe {
            json_stage1_build_tape(
                s.as_ptr(),
                s.len(),
                out.as_mut_ptr(),
                out.len(),
                std::ptr::null_mut(),
            )
        };
        assert_eq!(rc, RC_INVALID);
        let rc = unsafe { norito_crc64_cuda(std::ptr::null(), s.len(), &mut crc) };
        assert_eq!(rc, RC_INVALID);
        let rc = unsafe { norito_crc64_cuda(s.as_ptr(), s.len(), std::ptr::null_mut()) };
        assert_eq!(rc, RC_INVALID);
    }
    #[test]
    fn public_ffi_rejects_sequence_planner_null_control_pointers() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&0u64.to_le_bytes());
        let mut spans = [NoritoSequenceSpan { start: 0, end: 0 }; 1];
        let mut count = 0usize;
        let mut used = 0usize;
        let rc = unsafe {
            norito_length_prefixed_sequence_plan(
                std::ptr::null(),
                bytes.len(),
                0,
                spans.as_mut_ptr(),
                spans.len(),
                &mut count,
                &mut used,
            )
        };
        assert_eq!(rc, RC_INVALID);
        let rc = unsafe {
            norito_length_prefixed_sequence_plan(
                bytes.as_ptr(),
                bytes.len(),
                0,
                spans.as_mut_ptr(),
                spans.len(),
                std::ptr::null_mut(),
                &mut used,
            )
        };
        assert_eq!(rc, RC_INVALID);
        let rc = unsafe {
            norito_length_prefixed_sequence_plan(
                bytes.as_ptr(),
                bytes.len(),
                0,
                spans.as_mut_ptr(),
                spans.len(),
                &mut count,
                std::ptr::null_mut(),
            )
        };
        assert_eq!(rc, RC_INVALID);
        let rc = unsafe {
            norito_length_prefixed_sequence_plan(
                bytes.as_ptr(),
                bytes.len(),
                0,
                std::ptr::null_mut(),
                1,
                &mut count,
                &mut used,
            )
        };
        assert_eq!(rc, RC_INVALID);
    }
    #[test]
    fn public_ffi_rejects_non_length_prefix_flags() {
        let bytes = compact_sequence(&[b"a"]);
        for flags in (0..=u8::MAX).filter(|flags| *flags & !FLAG_COMPACT_LEN != 0) {
            let mut spans = [NoritoSequenceSpan { start: 0, end: 0 }; 1];
            let mut count = usize::MAX;
            let mut used = usize::MAX;
            let rc = unsafe {
                norito_length_prefixed_sequence_plan(
                    bytes.as_ptr(),
                    bytes.len(),
                    flags,
                    spans.as_mut_ptr(),
                    spans.len(),
                    &mut count,
                    &mut used,
                )
            };
            assert_eq!(rc, RC_INVALID, "flags {flags:#04x} must be rejected");
            assert_eq!((count, used), (usize::MAX, usize::MAX));
        }
    }
    #[test]
    fn cpu_stage1_empty_input_reports_zero_offsets() {
        let s = b"";
        let mut out = [0u32; 1];
        let mut len = usize::MAX;
        let rc = unsafe {
            json_stage1_build_tape_cpu(s.as_ptr(), s.len(), out.as_mut_ptr(), out.len(), &mut len)
        };
        assert_eq!(rc, 0);
        assert_eq!(len, 0);
    }
    #[test]
    fn reference_sequence_plan_splits_both_length_prefix_widths() {
        let compact = compact_sequence(&[b"a", b"bc", b"def"]);
        let (spans, used) = plan_sequence_cpu(&compact, FLAG_COMPACT_LEN).expect("compact plan");
        let spans: Vec<_> = spans.iter().map(|span| (span.start, span.end)).collect();
        assert_eq!(spans, vec![(9, 10), (11, 13), (14, 17)]);
        assert_eq!(used, compact.len());
        let fixed = fixed_width_sequence(&[b"a", b"bc"]);
        let (spans, used) = plan_sequence_cpu(&fixed, 0).expect("fixed-width plan");
        let spans: Vec<_> = spans.iter().map(|span| (span.start, span.end)).collect();
        assert_eq!(spans, vec![(16, 17), (25, 27)]);
        assert_eq!(used, fixed.len());
        let (spans, used) = plan_sequence_cpu(&0u64.to_le_bytes(), 0).expect("empty plan");
        assert!(spans.is_empty());
        assert_eq!(used, 8);
    }
    #[test]
    fn cuda_sequence_plan_matches_reference_when_available() {
        let elements: Vec<Vec<u8>> = (0..300u32)
            .map(|idx| lcg_payload((idx % 131) as usize, u64::from(idx)))
            .collect();
        let element_refs: Vec<&[u8]> = elements.iter().map(Vec::as_slice).collect();
        let large = compact_sequence(&element_refs);
        let cases = [
            (compact_sequence(&[b"a", b"bc", b"def"]), FLAG_COMPACT_LEN),
            (fixed_width_sequence(&[b"a", b"bc", b"def"]), 0),
            (large, FLAG_COMPACT_LEN),
        ];
        for (bytes, flags) in cases {
            let (expected, expected_used) =
                plan_sequence_cpu(&bytes, flags).expect("reference plan");
            let mut spans = vec![NoritoSequenceSpan { start: 0, end: 0 }; expected.len()];
            let mut count = 0usize;
            let mut used = 0usize;
            let rc = unsafe {
                norito_length_prefixed_sequence_plan(
                    bytes.as_ptr(),
                    bytes.len(),
                    flags,
                    spans.as_mut_ptr(),
                    spans.len(),
                    &mut count,
                    &mut used,
                )
            };
            if skip_if_unavailable(rc, "jsonstage1_cuda sequence planner") {
                return;
            }
            assert_eq!(rc, super::RC_OK);
            assert_eq!(count, expected.len());
            assert_eq!(used, expected_used);
            for (actual, expected) in spans.iter().zip(&expected) {
                assert_eq!((actual.start, actual.end), (expected.start, expected.end));
            }
        }
    }
    #[test]
    fn cuda_sequence_plan_capacity_reports_required_count_when_available() {
        let bytes = compact_sequence(&[b"a", b"bc", b"def"]);
        let mut spans = vec![
            NoritoSequenceSpan {
                start: usize::MAX,
                end: usize::MAX
            };
            2
        ];
        let mut count = 0usize;
        let mut used = usize::MAX;
        let rc = unsafe {
            norito_length_prefixed_sequence_plan(
                bytes.as_ptr(),
                bytes.len(),
                FLAG_COMPACT_LEN,
                spans.as_mut_ptr(),
                spans.len(),
                &mut count,
                &mut used,
            )
        };
        if skip_if_unavailable(rc, "jsonstage1_cuda sequence planner") {
            return;
        }
        assert_eq!(rc, RC_NO_SPACE);
        assert_eq!(count, 3);
        assert_eq!(used, 0);
        assert!(
            spans
                .iter()
                .all(|span| span.start == usize::MAX && span.end == usize::MAX),
            "capacity rejection must not write partial sequence spans"
        );
    }
    #[test]
    fn reference_sequence_plan_rejects_adversarial_shapes() {
        let mut overflowing_length = Vec::new();
        overflowing_length.extend_from_slice(&1u64.to_le_bytes());
        overflowing_length.extend_from_slice(&u64::MAX.to_le_bytes());
        assert!(matches!(
            plan_sequence_cpu(&overflowing_length, 0),
            Err(PlanError::Invalid)
        ));
        let mut overlong_varint = Vec::new();
        overlong_varint.extend_from_slice(&1u64.to_le_bytes());
        overlong_varint.extend_from_slice(&[0xFF; 10]);
        overlong_varint.push(0x01);
        assert!(matches!(
            plan_sequence_cpu(&overlong_varint, FLAG_COMPACT_LEN),
            Err(PlanError::Invalid)
        ));
        let mut truncated_length_prefixed = Vec::new();
        truncated_length_prefixed.extend_from_slice(&2u64.to_le_bytes());
        truncated_length_prefixed.extend_from_slice(&1u64.to_le_bytes());
        truncated_length_prefixed.push(b'a');
        truncated_length_prefixed.extend_from_slice(&4u64.to_le_bytes());
        truncated_length_prefixed.push(b'b');
        assert!(matches!(
            plan_sequence_cpu(&truncated_length_prefixed, 0),
            Err(PlanError::Invalid)
        ));
        let mut non_canonical_compact = Vec::new();
        non_canonical_compact.extend_from_slice(&1u64.to_le_bytes());
        non_canonical_compact.extend_from_slice(&[0x80, 0x00]);
        assert!(matches!(
            plan_sequence_cpu(&non_canonical_compact, FLAG_COMPACT_LEN),
            Err(PlanError::Invalid)
        ));
    }
    #[test]
    fn cuda_sequence_plan_invalid_length_prefix_do_not_publish_partial_spans_when_available() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&2u64.to_le_bytes());
        bytes.extend_from_slice(&1u64.to_le_bytes());
        bytes.push(b'a');
        bytes.extend_from_slice(&4u64.to_le_bytes());
        bytes.push(b'b');
        let sentinel = NoritoSequenceSpan {
            start: usize::MAX,
            end: usize::MAX,
        };
        let mut spans = vec![sentinel; 2];
        let mut count = 0usize;
        let mut used = usize::MAX;
        let rc = unsafe {
            norito_length_prefixed_sequence_plan(
                bytes.as_ptr(),
                bytes.len(),
                0,
                spans.as_mut_ptr(),
                spans.len(),
                &mut count,
                &mut used,
            )
        };
        if skip_if_unavailable(rc, "jsonstage1_cuda sequence planner") {
            return;
        }
        assert_eq!(rc, RC_INVALID);
        assert_eq!(count, 2);
        assert_eq!(used, 0);
        assert!(
            spans
                .iter()
                .all(|span| span.start == sentinel.start && span.end == sentinel.end),
            "invalid length-prefixed plans must not expose partially written CUDA spans"
        );
    }
    #[test]
    fn cuda_sequence_plan_rejects_impossible_length_prefixed_count_when_available() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&100u64.to_le_bytes());
        let sentinel = NoritoSequenceSpan {
            start: usize::MAX,
            end: usize::MAX,
        };
        let mut spans = vec![sentinel; 100];
        let mut count = 0usize;
        let mut used = usize::MAX;
        let rc = unsafe {
            norito_length_prefixed_sequence_plan(
                bytes.as_ptr(),
                bytes.len(),
                FLAG_COMPACT_LEN,
                spans.as_mut_ptr(),
                spans.len(),
                &mut count,
                &mut used,
            )
        };
        if skip_if_unavailable(rc, "jsonstage1_cuda sequence planner") {
            return;
        }
        assert_eq!(rc, RC_INVALID);
        assert_eq!(count, 100);
        assert_eq!(used, 0);
        assert!(
            spans
                .iter()
                .all(|span| span.start == sentinel.start && span.end == sentinel.end),
            "impossible declared counts must fail before publishing CUDA spans"
        );
    }
    #[test]
    fn cpu_stage1_exact_capacity_writes_offsets() {
        let s = br#"{"exact":[1,2,3]}"#;
        let expected = reference_offsets(s);
        let mut out = vec![0u32; expected.len()];
        let mut len = 0usize;
        let rc = unsafe {
            json_stage1_build_tape_cpu(s.as_ptr(), s.len(), out.as_mut_ptr(), out.len(), &mut len)
        };
        assert_eq!(rc, 0);
        out.truncate(len);
        assert_eq!(out, expected);
    }
    #[test]
    fn cpu_stage1_zero_capacity_reports_required_length() {
        let s = br#"{"zero":[1,2,3]}"#;
        let expected = reference_offsets(s);
        let mut out = [0u32; 1];
        let mut len = 0usize;
        let rc = unsafe {
            json_stage1_build_tape_cpu(s.as_ptr(), s.len(), out.as_mut_ptr(), 0, &mut len)
        };
        assert_eq!(rc, 2);
        assert_eq!(len, expected.len());
    }
    #[test]
    fn crc64_cpu_empty_matches_xz_identity() {
        assert_eq!(crc64_cpu(b""), 0);
    }
    #[test]
    fn public_cuda_entrypoints_handle_empty_inputs_without_device_work() {
        let s = b"";
        let mut offsets = [123u32; 1];
        let mut len = usize::MAX;
        let rc = unsafe {
            json_stage1_build_tape(
                s.as_ptr(),
                s.len(),
                offsets.as_mut_ptr(),
                offsets.len(),
                &mut len,
            )
        };
        if skip_if_unavailable(rc, "jsonstage1_cuda") {
            return;
        }
        assert_eq!(rc, 0);
        assert_eq!(len, 0);
        let mut crc = u64::MAX;
        let rc = unsafe { norito_crc64_cuda(s.as_ptr(), s.len(), &mut crc) };
        if skip_if_unavailable(rc, "jsonstage1_cuda CRC64") {
            return;
        }
        assert_eq!(rc, 0);
        assert_eq!(crc, 0);
    }
    #[test]
    fn cuda_stage1_zero_capacity_reports_required_len_when_available() {
        let s = br#"{"zero":[1,2,3],"quoted":"a\"b"}"#;
        let expected = reference_offsets(s);
        let mut out = [0u32; 1];
        let mut len = 0usize;
        let rc =
            unsafe { json_stage1_build_tape(s.as_ptr(), s.len(), out.as_mut_ptr(), 0, &mut len) };
        if skip_if_unavailable(rc, "jsonstage1_cuda") {
            return;
        }
        assert_eq!(rc, 2);
        assert_eq!(len, expected.len());
    }
    #[test]
    fn cuda_stage1_matches_reference_when_available() {
        let s = br#"{"left":[1,2],"right":{"quoted":"a\"b"}}"#;
        let expected = reference_offsets(s);
        let mut out = vec![0u32; expected.len() + 8];
        let mut len = 0usize;
        let rc = unsafe {
            json_stage1_build_tape(s.as_ptr(), s.len(), out.as_mut_ptr(), out.len(), &mut len)
        };
        if skip_if_unavailable(rc, "jsonstage1_cuda") {
            return;
        }
        assert_eq!(rc, 0);
        out.truncate(len);
        assert_eq!(out, expected);
    }
    #[test]
    fn cuda_stage1_exact_capacity_matches_reference_when_available() {
        let s = br#"{"exact":[1,2,3],"quoted":"a\"b"}"#;
        let expected = reference_offsets(s);
        let mut out = vec![0u32; expected.len()];
        let mut len = 0usize;
        let rc = unsafe {
            json_stage1_build_tape(s.as_ptr(), s.len(), out.as_mut_ptr(), out.len(), &mut len)
        };
        if skip_if_unavailable(rc, "jsonstage1_cuda") {
            return;
        }
        assert_eq!(rc, 0);
        out.truncate(len);
        assert_eq!(out, expected);
    }
    #[test]
    fn cuda_stage1_corpus_matches_reference_when_available() {
        let mut docs = vec![
            br#"{"a":1,"b":[true,false,null],"c":"plain"}"#.to_vec(),
            br#"{"escaped":"quote: \" slash: \\ pair: \\\\"}"#.to_vec(),
            br#"[{"nested":{"x":[1,2,3]}},{"empty":{}},{"arr":[]}]"#.to_vec(),
            br#"{\"invalid_but_scalar_defined\":1}"#.to_vec(),
        ];
        let mut boundary = String::from("{\"pad\":\"");
        boundary.push_str(&"a".repeat(31));
        boundary.push_str("\\\\\\\"");
        boundary.push_str("\",\"next\":[1,2,3],\"tail\":\"");
        boundary.push_str(&"z".repeat(65));
        boundary.push_str("\"}");
        docs.push(boundary.into_bytes());
        let mut large = String::from("{\"rows\":[");
        for idx in 0..2048 {
            if idx != 0 {
                large.push(',');
            }
            large.push_str("{\"id\":");
            large.push_str(&idx.to_string());
            large.push_str(",\"name\":\"row\\\\\\\"");
            large.push_str(&(idx % 17).to_string());
            large.push_str("\",\"values\":[1,2,3]}");
        }
        large.push_str("]}");
        docs.push(large.into_bytes());
        for doc in docs {
            let expected = reference_offsets(&doc);
            let mut out = vec![0u32; expected.len() + 16];
            let mut len = 0usize;
            let rc = unsafe {
                json_stage1_build_tape(
                    doc.as_ptr(),
                    doc.len(),
                    out.as_mut_ptr(),
                    out.len(),
                    &mut len,
                )
            };
            if skip_if_unavailable(rc, "jsonstage1_cuda") {
                return;
            }
            assert_eq!(rc, 0);
            out.truncate(len);
            assert_eq!(out, expected);
        }
    }
    #[test]
    fn cuda_stage1_capacity_reports_required_len_when_available() {
        let s = br#"{"capacity":[1,2,3],"quoted":"a\"b"}"#;
        let expected = reference_offsets(s);
        let mut out = [0u32; 2];
        let mut len = 0usize;
        let rc = unsafe {
            json_stage1_build_tape(s.as_ptr(), s.len(), out.as_mut_ptr(), out.len(), &mut len)
        };
        if skip_if_unavailable(rc, "jsonstage1_cuda") {
            return;
        }
        assert_eq!(rc, 2);
        assert_eq!(len, expected.len());
    }
    #[test]
    fn scanner_counts_match_written_offsets() {
        let s = br#"{"left":[1,2],"right":{"quoted":"a\"b"}}"#;
        let mut offsets = Vec::new();
        let count = scan_structural_offsets(s, |offset| offsets.push(offset));
        assert_eq!(count, offsets.len());
        assert!(!offsets.is_empty());
    }
    #[test]
    fn crc64_matches_reference() {
        let data = b"123456789";
        let mut out = 0u64;
        let rc = unsafe { norito_crc64_cuda(data.as_ptr(), data.len(), &mut out) };
        if skip_if_unavailable(rc, "jsonstage1_cuda CRC64") {
            return;
        }
        assert_eq!(rc, 0);
        assert_eq!(out, CRC_123456789);
    }
    #[test]
    fn cuda_crc64_large_payload_matches_cpu_when_available() {
        let data = lcg_payload(2 * CHUNK_SIZE + 333, 0xa5a5_0123_dead_beef);
        let mut out = 0u64;
        let rc = unsafe { norito_crc64_cuda(data.as_ptr(), data.len(), &mut out) };
        if skip_if_unavailable(rc, "jsonstage1_cuda CRC64") {
            return;
        }
        assert_eq!(rc, 0);
        assert_eq!(out, crc64_cpu(&data));
    }
    #[test]
    fn cuda_required_env_fails_if_helpers_are_not_accelerated() {
        if !cuda_required() {
            return;
        }
        let s = br#"{"required":"cuda","array":[1,2,3],"quoted":"a\"b"}"#;
        let expected = reference_offsets(s);
        let mut out = vec![0u32; expected.len() + 4];
        let mut len = 0usize;
        let rc = unsafe {
            json_stage1_build_tape(s.as_ptr(), s.len(), out.as_mut_ptr(), out.len(), &mut len)
        };
        assert_eq!(rc, 0, "JSONSTAGE1_CUDA_REQUIRE requires Stage-1 CUDA");
        out.truncate(len);
        assert_eq!(out, expected);
        let mut crc = 0u64;
        let rc = unsafe { norito_crc64_cuda(s.as_ptr(), s.len(), &mut crc) };
        assert_eq!(rc, 0, "JSONSTAGE1_CUDA_REQUIRE requires CUDA CRC64");
        assert_eq!(crc, crc64_cpu(s));
    }
    #[test]
    fn chunked_combine_matches_full_crc() {
        let data = (0u32..(CHUNK_SIZE as u32 + 3_333))
            .flat_map(|v| v.to_le_bytes())
            .collect::<Vec<u8>>();
        let mut combined = CRC64_INIT;
        for chunk in data.chunks(CHUNK_SIZE) {
            let part = crc64_raw(chunk, 0);
            combined = crc64_combine_raw(combined, part, chunk.len());
        }
        let full = crc64_cpu(&data);
        assert_eq!(combined ^ CRC64_XOR_OUT, full);
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
