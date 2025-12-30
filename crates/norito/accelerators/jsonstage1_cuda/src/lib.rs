//! jsonstage1_cuda: cdylib exporting JSON Stage-1 structural tape builder and
//! CRC64 helpers (CUDA-accelerated when compiled with the `cuda-kernel`
//! feature and a working nvcc toolchain).
//!
//! C ABI: `json_stage1_build_tape(input_ptr, input_len, out_offsets, out_capacity, out_len)`
//! Returns 0 on success, non-zero on failure.

#[cfg(crc64_cuda_available)]
extern "C" {
    fn norito_crc64_cuda_impl(input_ptr: *const u8, input_len: usize, out_crc: *mut u64) -> i32;
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

fn crc64_cpu(bytes: &[u8]) -> u64 {
    const POLY: u64 = 0x42F0_E1EB_A9EA_3693;
    let mut crc = 0u64;
    for &b in bytes {
        crc ^= (b as u64) << 56;
        for _ in 0..8 {
            if (crc & 0x8000_0000_0000_0000) != 0 {
                crc = (crc << 1) ^ POLY;
            } else {
                crc <<= 1;
            }
        }
    }
    crc
}

/// Compute CRC64-ECMA using the CUDA helper (CPU fallback for now).
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
    use super::{crc64_cpu, json_stage1_build_tape, norito_crc64_cuda};

    const CRC_123456789: u64 = 0x6C40_DF5F_0B49_7347;
    const CHUNK_SIZE: usize = 16 * 1024;

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

        let mut combined = 0u64;
        for chunk in data.chunks(CHUNK_SIZE) {
            let part = crc64_cpu(chunk);
            combined = crc64_combine(combined, part, chunk.len());
        }

        let full = crc64_cpu(&data);
        assert_eq!(combined, full);
    }

    fn crc64_combine(mut crc1: u64, crc2: u64, len2: usize) -> u64 {
        const POLY: u64 = 0x42F0_E1EB_A9EA_3693;
        if len2 == 0 {
            return crc1;
        }

        let mut odd = [0u64; 64];
        odd[0] = POLY;
        for (n, slot) in odd.iter_mut().enumerate().skip(1) {
            *slot = 1u64 << (63 - n);
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

        let mut even = [0u64; 64];
        gf2_matrix_square(&mut even, &odd);

        let mut len_bits = len2 as u64 * 8;
        while len_bits != 0 {
            gf2_matrix_square(&mut odd, &even);
            if len_bits & 1 != 0 {
                crc1 = gf2_matrix_times(&odd, crc1);
            }
            len_bits >>= 1;
            if len_bits == 0 {
                break;
            }
            gf2_matrix_square(&mut even, &odd);
            if len_bits & 1 != 0 {
                crc1 = gf2_matrix_times(&even, crc1);
            }
            len_bits >>= 1;
        }

        crc1 ^ crc2
    }
}
