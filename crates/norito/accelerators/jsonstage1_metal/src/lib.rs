//! jsonstage1_metal: cdylib exporting JSON Stage‑1 structural tape builder via Metal.
//!
//! C ABI: `json_stage1_build_tape(input_ptr, input_len, out_offsets, out_capacity, out_len)`
//! Returns 0 on success, non-zero on failure.

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
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    unsafe {
        let rc = json_stage1_build_tape_metal_impl(
            input_ptr,
            input_len,
            out_offsets,
            out_capacity,
            out_len,
        );
        if rc == 0 {
            return rc;
        }
    }
    // CPU fallback (used when Metal is unavailable or on non-mac targets)
    if input_ptr.is_null() || out_offsets.is_null() || out_len.is_null() {
        return 1;
    }
    let bytes = unsafe { std::slice::from_raw_parts(input_ptr, input_len) };
    let mut offs = Vec::<u32>::with_capacity(1024);
    let mut i = 0usize;
    let mut in_str = false;
    while i < bytes.len() {
        let c = bytes[i];
        if in_str {
            if c == b'\\' {
                i = i.saturating_add(2);
                continue;
            }
            if c == b'"' {
                in_str = false;
                offs.push(i as u32);
                i += 1;
                continue;
            }
            i += 1;
        } else {
            match c {
                b'"' => {
                    in_str = true;
                    offs.push(i as u32);
                    i += 1;
                }
                b'{' | b'}' | b'[' | b']' | b':' | b',' => {
                    offs.push(i as u32);
                    i += 1;
                }
                _ => i += 1,
            }
        }
    }
    let need = offs.len();
    unsafe {
        *out_len = need;
    }
    if need > out_capacity {
        return 2;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(offs.as_ptr(), out_offsets, need);
    }
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

/// Compute CRC64-XZ for the provided buffer using Metal when available,
/// falling back to a portable CPU implementation otherwise.
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
        return 1;
    }
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    unsafe {
        let rc = norito_crc64_metal_impl(input_ptr, input_len, out_crc);
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
    use super::{crc64_cpu, crc64_raw, json_stage1_build_tape, norito_crc64_metal};

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
    fn crc64_round_trip() {
        let data = b"123456789";
        let mut out = 0u64;
        let rc = unsafe { norito_crc64_metal(data.as_ptr(), data.len(), &mut out) };
        assert_eq!(rc, 0);
        assert_eq!(out, 0x995D_C9BB_DF19_39FA);
    }

    #[test]
    fn crc64_large_matches_cpu() {
        let data = vec![0xAAu8; 48 * 1024];
        let mut out = 0u64;
        let rc = unsafe { norito_crc64_metal(data.as_ptr(), data.len(), &mut out) };
        assert_eq!(rc, 0);
        assert_eq!(out, crc64_cpu(&data));
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
