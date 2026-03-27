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

fn scan_structural_offsets(mut bytes: &[u8], mut emit: impl FnMut(u32)) -> usize {
    let mut count = 0usize;
    let mut base = 0usize;
    let mut in_str = false;
    while !bytes.is_empty() {
        let c = bytes[0];
        if in_str {
            if c == b'\\' {
                let skip = bytes.len().min(2);
                bytes = &bytes[skip..];
                base += skip;
                continue;
            }
            if c == b'"' {
                emit(base as u32);
                count += 1;
                in_str = false;
            }
            bytes = &bytes[1..];
            base += 1;
            continue;
        }

        match c {
            b'"' => {
                emit(base as u32);
                count += 1;
                in_str = true;
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
        return 1;
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

fn crc64_cpu(bytes: &[u8]) -> u64 {
    const INIT: u64 = 0xFFFF_FFFF_FFFF_FFFF;
    const XOR_OUT: u64 = 0xFFFF_FFFF_FFFF_FFFF;
    let crc = crc64_raw(bytes, INIT);
    crc ^ XOR_OUT
}

/// Compute CRC64-XZ using the CUDA helper (CPU fallback for now).
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
        return 1;
    }
    #[cfg(crc64_cuda_available)]
    unsafe {
        let rc = norito_crc64_cuda_impl(input_ptr, input_len, out_crc);
        if rc == 0 {
            return 0;
        }
    }
    let bytes = unsafe { std::slice::from_raw_parts(input_ptr, input_len) };
    let crc = crc64_cpu(bytes);
    unsafe {
        *out_crc = crc;
    }
    0
}

#[cfg(test)]
mod tests {
    use super::{
        crc64_cpu, crc64_raw, json_stage1_build_tape, norito_crc64_cuda, scan_structural_offsets,
    };

    const CRC_123456789: u64 = 0x995D_C9BB_DF19_39FA;
    const CHUNK_SIZE: usize = 16 * 1024;
    const CRC64_INIT: u64 = 0xFFFF_FFFF_FFFF_FFFF;
    const CRC64_XOR_OUT: u64 = 0xFFFF_FFFF_FFFF_FFFF;

    #[test]
    fn basic_offsets() {
        let s = b"{\"a\":1}";
        let mut out = vec![0u32; 16];
        let mut len = 0usize;
        let rc = unsafe {
            json_stage1_build_tape(s.as_ptr(), s.len(), out.as_mut_ptr(), out.len(), &mut len)
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
            json_stage1_build_tape(s.as_ptr(), s.len(), out.as_mut_ptr(), out.len(), &mut len)
        };
        assert_eq!(rc, 0);
        out.truncate(len);
        assert_eq!(out, vec![0, 1, 3, 4, 5, 10, 11]);
    }

    #[test]
    fn capacity_errors_still_report_required_length() {
        let s = b"{\"a\":1}";
        let mut out = [0u32; 2];
        let mut len = 0usize;
        let rc = unsafe {
            json_stage1_build_tape(s.as_ptr(), s.len(), out.as_mut_ptr(), out.len(), &mut len)
        };
        assert_eq!(rc, 2);
        assert_eq!(len, 5);
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
        assert_eq!(rc, 0);
        assert_eq!(out, CRC_123456789);
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
