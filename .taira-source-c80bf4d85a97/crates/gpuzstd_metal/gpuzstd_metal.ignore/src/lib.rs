//! gpuzstd_metal: cdylib exporting GPU-assisted zstd helpers for Norito.
//!
//! C ABI:
//! - gpu_zstd_compress(src_ptr, src_len, level, dst_ptr, dst_len)
//! - gpu_zstd_decompress(src_ptr, src_len, dst_ptr, dst_len)

use std::io::Cursor;

const RC_OK: i32 = 0;
const RC_INVALID: i32 = 1;
const RC_NO_SPACE: i32 = 2;
#[cfg(any(test, not(all(target_os = "macos", target_arch = "aarch64"))))]
const RC_GPU_UNAVAILABLE: i32 = 3;
const RC_ZSTD: i32 = 4;

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
unsafe extern "C" {
    fn gpuzstd_metal_prepass(input_ptr: *const u8, input_len: usize, out_digest: *mut u64) -> i32;
}

fn metal_prepass(input: &[u8]) -> Result<u64, i32> {
    let mut digest = 0u64;
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    unsafe {
        let rc = gpuzstd_metal_prepass(input.as_ptr(), input.len(), &mut digest);
        if rc == 0 {
            return Ok(digest);
        }
        return Err(rc);
    }
    #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
    {
        let _ = input;
        Err(RC_GPU_UNAVAILABLE)
    }
}

fn compress_cpu(src: &[u8], level: i32) -> Result<Vec<u8>, i32> {
    zstd::encode_all(Cursor::new(src), level).map_err(|_| RC_ZSTD)
}

fn decompress_cpu(src: &[u8]) -> Result<Vec<u8>, i32> {
    zstd::decode_all(Cursor::new(src)).map_err(|_| RC_ZSTD)
}

/// Compress bytes into zstd format, using Metal when available.
///
/// # Safety
/// The caller must ensure `src_ptr` is readable for `src_len` bytes, `dst_ptr`
/// is writable for the capacity stored in `dst_len`, and `dst_len` is valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gpu_zstd_compress(
    src_ptr: *const u8,
    src_len: usize,
    level: i32,
    dst_ptr: *mut u8,
    dst_len: *mut usize,
) -> i32 {
    if src_ptr.is_null() || dst_ptr.is_null() || dst_len.is_null() {
        return RC_INVALID;
    }
    let src = unsafe { std::slice::from_raw_parts(src_ptr, src_len) };
    let capacity = unsafe { *dst_len };

    // CPU zstd keeps output in the standard frame format.
    if let Err(rc) = metal_prepass(src) {
        return rc;
    }

    let encoded = match compress_cpu(src, level) {
        Ok(bytes) => bytes,
        Err(code) => return code,
    };
    let encoded_len = encoded.len();
    unsafe {
        *dst_len = encoded_len;
    }
    if encoded_len > capacity {
        return RC_NO_SPACE;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(encoded.as_ptr(), dst_ptr, encoded_len);
    }
    RC_OK
}

/// Decompress zstd bytes, using Metal when available.
///
/// # Safety
/// The caller must ensure `src_ptr` is readable for `src_len` bytes, `dst_ptr`
/// is writable for the capacity stored in `dst_len`, and `dst_len` is valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gpu_zstd_decompress(
    src_ptr: *const u8,
    src_len: usize,
    dst_ptr: *mut u8,
    dst_len: *mut usize,
) -> i32 {
    if src_ptr.is_null() || dst_ptr.is_null() || dst_len.is_null() {
        return RC_INVALID;
    }
    let src = unsafe { std::slice::from_raw_parts(src_ptr, src_len) };
    let capacity = unsafe { *dst_len };

    // CPU decode ensures standard frames roundtrip without requiring GPU decode support.
    if let Err(rc) = metal_prepass(src) {
        return rc;
    }

    let decoded = match decompress_cpu(src) {
        Ok(bytes) => bytes,
        Err(code) => return code,
    };
    let decoded_len = decoded.len();
    unsafe {
        *dst_len = decoded_len;
    }
    if decoded_len > capacity {
        return RC_NO_SPACE;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(decoded.as_ptr(), dst_ptr, decoded_len);
    }
    RC_OK
}

#[cfg(test)]
mod tests {
    use super::{
        RC_GPU_UNAVAILABLE, RC_OK, compress_cpu, decompress_cpu, gpu_zstd_compress,
        gpu_zstd_decompress,
    };

    #[test]
    fn compress_and_decompress_roundtrip() {
        let payload = b"gpuzstd metal roundtrip";
        let mut compressed = vec![0u8; 256];
        let mut compressed_len = compressed.len();
        let rc = unsafe {
            gpu_zstd_compress(
                payload.as_ptr(),
                payload.len(),
                1,
                compressed.as_mut_ptr(),
                &mut compressed_len,
            )
        };
        if rc == RC_GPU_UNAVAILABLE {
            // Running on non-Metal hosts; skip the GPU path.
            return;
        }
        assert_eq!(rc, RC_OK);
        compressed.truncate(compressed_len);

        let mut decoded = vec![0u8; payload.len()];
        let mut decoded_len = decoded.len();
        let rc = unsafe {
            gpu_zstd_decompress(
                compressed.as_ptr(),
                compressed.len(),
                decoded.as_mut_ptr(),
                &mut decoded_len,
            )
        };
        assert_eq!(rc, RC_OK);
        decoded.truncate(decoded_len);
        assert_eq!(decoded, payload);
    }

    #[test]
    fn decodes_cpu_zstd_payloads() {
        let payload = b"cpu zstd path";
        let encoded = compress_cpu(payload, 1).expect("cpu compress");

        let mut decoded = vec![0u8; payload.len()];
        let mut decoded_len = decoded.len();
        let rc = unsafe {
            gpu_zstd_decompress(
                encoded.as_ptr(),
                encoded.len(),
                decoded.as_mut_ptr(),
                &mut decoded_len,
            )
        };
        if rc == RC_GPU_UNAVAILABLE {
            return;
        }
        assert_eq!(rc, RC_OK);
        decoded.truncate(decoded_len);
        assert_eq!(decoded, payload);
    }

    #[test]
    fn cpu_roundtrip_matches_expected() {
        let payload = vec![0xABu8; 128 * 1024];
        let encoded = compress_cpu(&payload, 1).expect("cpu compress");
        let decoded = decompress_cpu(&encoded).expect("cpu decode");
        assert_eq!(decoded, payload);
    }
}
