//! CUDA-named GPU-assisted zstd helper for Norito.
//!
//! This crate exports the same C ABI as `gpuzstd_metal`, but under the
//! `gpuzstd_cuda` artifact name so Unix/Windows Norito builds can load an
//! in-tree `libgpuzstd_cuda` helper directly. The implementation reuses the
//! deterministic shared helper path from `gpuzstd_metal` until dedicated CUDA
//! kernels land.

use gpuzstd_metal::{compress_ffi, decompress_ffi};

/// Compress `src` into `dst` using the shared helper path exposed as the
/// CUDA-named helper artifact.
///
/// # Safety
/// `src` must point to `src_len` readable bytes. `dst` must point to a writable
/// buffer whose capacity is provided via `*dst_len`. `dst_len` must be non-null
/// and writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gpu_zstd_compress(
    src: *const u8,
    src_len: usize,
    level: i32,
    dst: *mut u8,
    dst_len: *mut usize,
) -> i32 {
    unsafe { compress_ffi(src, src_len, level, dst, dst_len) }
}

/// Decompress `src` into `dst` using the shared helper path exposed as the
/// CUDA-named helper artifact.
///
/// # Safety
/// `src` must point to `src_len` readable bytes. `dst` must point to a writable
/// buffer whose capacity is provided via `*dst_len`. `dst_len` must be non-null
/// and writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gpu_zstd_decompress(
    src: *const u8,
    src_len: usize,
    dst: *mut u8,
    dst_len: *mut usize,
) -> i32 {
    unsafe { decompress_ffi(src, src_len, dst, dst_len) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::Cursor, slice};

    const RC_OK: i32 = 0;
    const RC_ZSTD: i32 = 4;

    fn try_gpu_compress(payload: &[u8]) -> Result<Vec<u8>, i32> {
        let mut out = vec![0u8; payload.len().saturating_mul(4).saturating_add(512)];
        let mut out_len = out.len();
        let rc = unsafe {
            gpu_zstd_compress(
                payload.as_ptr(),
                payload.len(),
                1,
                out.as_mut_ptr(),
                &mut out_len,
            )
        };
        if rc != RC_OK {
            return Err(rc);
        }
        out.truncate(out_len);
        Ok(out)
    }

    fn try_gpu_decompress(payload: &[u8], expected_len: usize) -> Result<Vec<u8>, i32> {
        let mut out = vec![0u8; expected_len.saturating_mul(2).saturating_add(256)];
        let mut out_len = out.len();
        let rc = unsafe {
            gpu_zstd_decompress(
                payload.as_ptr(),
                payload.len(),
                out.as_mut_ptr(),
                &mut out_len,
            )
        };
        if rc != RC_OK {
            return Err(rc);
        }
        out.truncate(out_len);
        Ok(out)
    }

    #[test]
    fn gpu_roundtrip_matches_cpu() {
        let payload = b"gpuzstd cuda roundtrip";
        let compressed = try_gpu_compress(payload).expect("gpu compress");
        let decoded = try_gpu_decompress(&compressed, payload.len()).expect("gpu decompress");
        assert_eq!(decoded, payload);
        let cpu_decoded = zstd::decode_all(Cursor::new(&compressed)).expect("cpu decode");
        assert_eq!(cpu_decoded, payload);
    }

    #[test]
    fn gpu_decode_rejects_invalid_frames() {
        let invalid = [0u8, 1, 2, 3, 4, 5];
        let mut out = [0u8; 64];
        let mut out_len = out.len();
        let rc = unsafe {
            gpu_zstd_decompress(
                invalid.as_ptr(),
                invalid.len(),
                out.as_mut_ptr(),
                &mut out_len,
            )
        };
        assert_eq!(rc, RC_ZSTD);
    }

    #[test]
    fn gpu_decode_writes_payload_bytes() {
        let payload = b"gpuzstd cuda payload";
        let encoded = zstd::encode_all(Cursor::new(payload), 1).expect("cpu encode");
        let mut out = vec![0u8; payload.len().saturating_mul(2).saturating_add(256)];
        let mut out_len = out.len();
        let rc = unsafe {
            gpu_zstd_decompress(
                encoded.as_ptr(),
                encoded.len(),
                out.as_mut_ptr(),
                &mut out_len,
            )
        };
        assert_eq!(rc, RC_OK);
        out.truncate(out_len);
        let out_slice = unsafe { slice::from_raw_parts(out.as_ptr(), out.len()) };
        assert_eq!(out_slice, payload);
    }
}
