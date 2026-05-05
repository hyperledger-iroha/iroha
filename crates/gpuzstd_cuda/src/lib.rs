//! CUDA GPU-assisted zstd helper for Norito.
//!
//! This crate exports the same C ABI as `gpuzstd_metal`, but under the
//! `gpuzstd_cuda` artifact name so Unix/Windows Norito builds can load an
//! in-tree CUDA helper directly. The CUDA path performs deterministic
//! match-finding and sequence generation on the GPU, then uses the shared zstd
//! frame encoder for host-side frame assembly. Decode uses the
//! shared frame decoder and falls back to the CPU zstd decoder for frames the
//! in-crate decoder does not yet support.

use std::{io::Cursor, ptr, slice};

use gpuzstd_metal::{GpuZstdSequence, zstd_frame};

const RC_OK: i32 = 0;
const RC_INVALID: i32 = 1;
const RC_NO_SPACE: i32 = 2;
#[cfg_attr(gpuzstd_cuda_available, allow(dead_code))]
const RC_GPU_UNAVAILABLE: i32 = 3;
const RC_ZSTD: i32 = 4;

const CHUNK_SIZE: u32 = 32 * 1024;
#[cfg_attr(not(gpuzstd_cuda_available), allow(dead_code))]
const MIN_MATCH: u32 = 3;
#[cfg_attr(not(gpuzstd_cuda_available), allow(dead_code))]
const MAX_MATCH: u32 = 64;

#[cfg(gpuzstd_cuda_available)]
unsafe extern "C" {
    fn gpuzstd_cuda_count_sequences(
        input: *const u8,
        input_len: usize,
        chunk_size: u32,
        min_match: u32,
        max_match: u32,
        out_counts: *mut u32,
        counts_len: u32,
    ) -> i32;

    fn gpuzstd_cuda_write_sequences(
        input: *const u8,
        input_len: usize,
        chunk_size: u32,
        min_match: u32,
        max_match: u32,
        offsets: *const u32,
        offsets_len: u32,
        out_seqs: *mut GpuZstdSequence,
        seq_capacity: u32,
    ) -> i32;
}

#[derive(Default)]
struct GpuSequences {
    counts: Vec<u32>,
    offsets: Vec<u32>,
    seqs: Vec<GpuZstdSequence>,
}

#[cfg(gpuzstd_cuda_available)]
fn gpu_sequences(input: &[u8]) -> Result<GpuSequences, i32> {
    if input.is_empty() {
        return Ok(GpuSequences::default());
    }
    let chunk_count = input.len().div_ceil(CHUNK_SIZE as usize);
    if chunk_count == 0 {
        return Ok(GpuSequences::default());
    }
    if chunk_count > u32::MAX as usize {
        return Err(RC_ZSTD);
    }

    let mut counts = vec![0u32; chunk_count];
    let rc = unsafe {
        gpuzstd_cuda_count_sequences(
            input.as_ptr(),
            input.len(),
            CHUNK_SIZE,
            MIN_MATCH,
            MAX_MATCH,
            counts.as_mut_ptr(),
            counts.len() as u32,
        )
    };
    if rc != RC_OK {
        return Err(rc);
    }

    let mut offsets = Vec::with_capacity(chunk_count);
    let mut total: u64 = 0;
    for count in &counts {
        offsets.push(total as u32);
        total = total.saturating_add(*count as u64);
    }
    if total == 0 {
        return Ok(GpuSequences {
            counts,
            offsets,
            seqs: Vec::new(),
        });
    }
    if total > u32::MAX as u64 {
        return Err(RC_ZSTD);
    }

    let seq_len = total as usize;
    let mut seqs = vec![GpuZstdSequence::default(); seq_len];
    let rc = unsafe {
        gpuzstd_cuda_write_sequences(
            input.as_ptr(),
            input.len(),
            CHUNK_SIZE,
            MIN_MATCH,
            MAX_MATCH,
            offsets.as_ptr(),
            offsets.len() as u32,
            seqs.as_mut_ptr(),
            seqs.len() as u32,
        )
    };
    if rc != RC_OK {
        return Err(rc);
    }

    let mut consumed: u64 = 0;
    for seq in &seqs {
        consumed = consumed.saturating_add(seq.lit_len as u64);
        consumed = consumed.saturating_add(seq.match_len as u64);
    }
    if consumed != input.len() as u64 {
        return Err(RC_ZSTD);
    }
    Ok(GpuSequences {
        counts,
        offsets,
        seqs,
    })
}

#[cfg(not(gpuzstd_cuda_available))]
fn gpu_sequences(_input: &[u8]) -> Result<GpuSequences, i32> {
    Err(RC_GPU_UNAVAILABLE)
}

unsafe fn compress_ffi(
    src: *const u8,
    src_len: usize,
    _level: i32,
    dst: *mut u8,
    dst_len: *mut usize,
) -> i32 {
    if src.is_null() || dst.is_null() || dst_len.is_null() {
        return RC_INVALID;
    }
    let src_slice = unsafe { slice::from_raw_parts(src, src_len) };
    let capacity = unsafe { *dst_len };
    if capacity == 0 {
        return RC_NO_SPACE;
    }
    let encoded = match gpu_sequences(src_slice) {
        Ok(sequences) => match zstd_frame::encode_frame(
            src_slice,
            CHUNK_SIZE as usize,
            &sequences.counts,
            &sequences.offsets,
            &sequences.seqs,
            false,
        ) {
            Ok(bytes) => bytes,
            Err(_) => return RC_ZSTD,
        },
        Err(rc) => return rc,
    };
    if encoded.len() > capacity {
        return RC_NO_SPACE;
    }
    unsafe {
        ptr::copy_nonoverlapping(encoded.as_ptr(), dst, encoded.len());
        *dst_len = encoded.len();
    }
    RC_OK
}

unsafe fn decompress_ffi(src: *const u8, src_len: usize, dst: *mut u8, dst_len: *mut usize) -> i32 {
    if src.is_null() || dst.is_null() || dst_len.is_null() {
        return RC_INVALID;
    }
    let src_slice = unsafe { slice::from_raw_parts(src, src_len) };
    let capacity = unsafe { *dst_len };
    if capacity == 0 {
        return RC_NO_SPACE;
    }
    let decoded = match zstd_frame::decode_frame(src_slice) {
        Ok(bytes) => bytes,
        Err(_) => match zstd::decode_all(Cursor::new(src_slice)) {
            Ok(bytes) => bytes,
            Err(_) => return RC_ZSTD,
        },
    };
    if decoded.len() > capacity {
        return RC_NO_SPACE;
    }
    unsafe {
        ptr::copy_nonoverlapping(decoded.as_ptr(), dst, decoded.len());
        *dst_len = decoded.len();
    }
    RC_OK
}

/// Compress `src` into `dst` using the CUDA helper.
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

/// Decompress `src` into `dst` using the CUDA helper.
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
    use std::io::Cursor;

    fn lcg_payload(len: usize, mut seed: u64) -> Vec<u8> {
        let mut out = vec![0u8; len];
        for byte in &mut out {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            *byte = (seed >> 32) as u8;
        }
        out
    }

    fn try_gpu_compress(payload: &[u8]) -> Result<Vec<u8>, i32> {
        try_gpu_compress_level(payload, 1)
    }

    fn try_gpu_compress_level(payload: &[u8], level: i32) -> Result<Vec<u8>, i32> {
        let mut out = vec![0u8; payload.len().saturating_mul(4).saturating_add(512)];
        let mut out_len = out.len();
        let rc = unsafe {
            gpu_zstd_compress(
                payload.as_ptr(),
                payload.len(),
                level,
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

    fn skip_if_unavailable(rc: i32) -> bool {
        if rc == RC_GPU_UNAVAILABLE {
            if std::env::var_os("GPUZSTD_CUDA_REQUIRE").is_some() {
                panic!(
                    "gpuzstd_cuda unavailable while GPUZSTD_CUDA_REQUIRE is set; build with nvcc and run on a CUDA host"
                );
            }
            eprintln!("gpuzstd_cuda unavailable; skipping CUDA-only assertion");
            true
        } else {
            false
        }
    }

    fn assert_payload_eq(actual: &[u8], expected: &[u8], context: &str) {
        if actual == expected {
            return;
        }
        let mismatch = actual
            .iter()
            .zip(expected)
            .position(|(actual, expected)| actual != expected);
        panic!(
            "{context}: decoded payload mismatch: actual_len={} expected_len={} first_mismatch={mismatch:?}",
            actual.len(),
            expected.len()
        );
    }

    #[test]
    fn ffi_rejects_null_pointers() {
        let payload = b"gpuzstd ffi boundary";
        let mut out = [0u8; 64];
        let mut out_len = out.len();

        let rc = unsafe {
            gpu_zstd_compress(
                std::ptr::null(),
                payload.len(),
                1,
                out.as_mut_ptr(),
                &mut out_len,
            )
        };
        assert_eq!(rc, RC_INVALID);

        let rc = unsafe {
            gpu_zstd_compress(
                payload.as_ptr(),
                payload.len(),
                1,
                std::ptr::null_mut(),
                &mut out_len,
            )
        };
        assert_eq!(rc, RC_INVALID);

        let rc = unsafe {
            gpu_zstd_decompress(
                payload.as_ptr(),
                payload.len(),
                out.as_mut_ptr(),
                std::ptr::null_mut(),
            )
        };
        assert_eq!(rc, RC_INVALID);
    }

    #[test]
    fn ffi_reports_no_space_before_cuda_work_for_zero_capacity() {
        let payload = b"gpuzstd ffi boundary";
        let mut out = [0u8; 1];
        let mut out_len = 0usize;

        let rc = unsafe {
            gpu_zstd_compress(
                payload.as_ptr(),
                payload.len(),
                1,
                out.as_mut_ptr(),
                &mut out_len,
            )
        };
        assert_eq!(rc, RC_NO_SPACE);

        let encoded = zstd::encode_all(Cursor::new(payload), 1).expect("cpu encode");
        let rc = unsafe {
            gpu_zstd_decompress(
                encoded.as_ptr(),
                encoded.len(),
                out.as_mut_ptr(),
                &mut out_len,
            )
        };
        assert_eq!(rc, RC_NO_SPACE);
    }

    #[test]
    fn gpu_decode_accepts_exact_output_capacity() {
        let payload = b"gpuzstd exact output capacity";
        let encoded = zstd::encode_all(Cursor::new(payload), 1).expect("cpu encode");
        let mut out = vec![0u8; payload.len()];
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
        assert_eq!(out, payload);
    }

    #[test]
    fn gpu_compress_reports_no_space_for_short_output_buffer_when_available() {
        let payload = b"gpuzstd cuda no-space branch gpuzstd cuda no-space branch";
        let mut out = [0u8; 4];
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
        if skip_if_unavailable(rc) {
            return;
        }
        assert_eq!(rc, RC_NO_SPACE);
    }

    #[test]
    fn gpu_compress_empty_payload_roundtrips_when_available() {
        let payload: &[u8] = b"";
        let mut encoded = vec![0u8; 64];
        let mut encoded_len = encoded.len();
        let rc = unsafe {
            gpu_zstd_compress(
                payload.as_ptr(),
                payload.len(),
                1,
                encoded.as_mut_ptr(),
                &mut encoded_len,
            )
        };
        if skip_if_unavailable(rc) {
            return;
        }
        assert_eq!(rc, RC_OK);
        encoded.truncate(encoded_len);
        assert!(
            zstd::decode_all(Cursor::new(&encoded))
                .expect("cpu decode")
                .is_empty()
        );

        let mut decoded = [0u8; 1];
        let mut decoded_len = decoded.len();
        let rc = unsafe {
            gpu_zstd_decompress(
                encoded.as_ptr(),
                encoded.len(),
                decoded.as_mut_ptr(),
                &mut decoded_len,
            )
        };
        assert_eq!(rc, RC_OK);
        assert_eq!(decoded_len, 0);
    }

    #[test]
    fn gpu_compress_accepts_exact_output_capacity_when_available() {
        let payload = b"gpuzstd exact compression capacity gpuzstd exact compression capacity";
        let encoded = match try_gpu_compress(payload) {
            Ok(bytes) => bytes,
            Err(rc) => {
                if skip_if_unavailable(rc) {
                    return;
                }
                panic!("gpu compress failed: {rc}");
            }
        };

        let mut exact = vec![0u8; encoded.len()];
        let mut exact_len = exact.len();
        let rc = unsafe {
            gpu_zstd_compress(
                payload.as_ptr(),
                payload.len(),
                1,
                exact.as_mut_ptr(),
                &mut exact_len,
            )
        };
        assert_eq!(rc, RC_OK);
        exact.truncate(exact_len);
        assert_eq!(exact, encoded);
    }

    #[test]
    fn gpu_decode_empty_source_reports_zstd_error() {
        let input: &[u8] = b"";
        let mut out = [0u8; 8];
        let mut out_len = out.len();
        let rc = unsafe {
            gpu_zstd_decompress(input.as_ptr(), input.len(), out.as_mut_ptr(), &mut out_len)
        };
        assert_eq!(rc, RC_ZSTD);
    }

    #[test]
    fn gpu_compress_roundtrips_when_cuda_is_available() {
        let payload = b"gpuzstd cuda roundtrip gpuzstd cuda roundtrip";
        let compressed = match try_gpu_compress(payload) {
            Ok(bytes) => bytes,
            Err(rc) => {
                if skip_if_unavailable(rc) {
                    return;
                }
                panic!("gpu compress failed: {rc}");
            }
        };
        let decoded = zstd::decode_all(Cursor::new(&compressed)).expect("cpu decode");
        assert_eq!(decoded, payload);
    }

    #[test]
    fn gpu_decode_accepts_standard_cpu_frames() {
        let payload = b"gpuzstd cuda roundtrip";
        let compressed = zstd::encode_all(Cursor::new(payload), 1).expect("cpu encode");
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
    fn gpu_decode_reports_no_space_for_short_output_buffer() {
        let payload = b"gpuzstd cuda payload";
        let encoded = zstd::encode_all(Cursor::new(payload), 1).expect("cpu encode");
        let mut out = [0u8; 4];
        let mut out_len = out.len();
        let rc = unsafe {
            gpu_zstd_decompress(
                encoded.as_ptr(),
                encoded.len(),
                out.as_mut_ptr(),
                &mut out_len,
            )
        };
        assert_eq!(rc, RC_NO_SPACE);
    }

    #[test]
    fn cuda_determinism_corpus_roundtrip_when_available() {
        let corpus = [
            b"gpuzstd verification corpus".as_slice(),
            &[0u8; 1][..],
            &[0x5a; 64][..],
            &[0xa5; 1023][..],
            &[0x33; 1024][..],
        ];
        let random_1 = lcg_payload(257, 0x1234_5678_9abc_def0);
        let random_2 = lcg_payload(4096, 0xfeed_beef_cafe_f00d);

        for payload in corpus
            .into_iter()
            .chain([random_1.as_slice(), random_2.as_slice()])
        {
            let compressed_a = match try_gpu_compress(payload) {
                Ok(bytes) => bytes,
                Err(rc) => {
                    if skip_if_unavailable(rc) {
                        return;
                    }
                    panic!("gpu compress failed: {rc}");
                }
            };
            let compressed_b = match try_gpu_compress(payload) {
                Ok(bytes) => bytes,
                Err(rc) => {
                    if skip_if_unavailable(rc) {
                        return;
                    }
                    panic!("gpu compress failed: {rc}");
                }
            };
            assert_eq!(compressed_a, compressed_b);

            let decoded_cpu = zstd::decode_all(Cursor::new(&compressed_a)).expect("cpu decode");
            assert_payload_eq(&decoded_cpu, payload, "cpu zstd decode");

            let decoded_gpu =
                try_gpu_decompress(&compressed_a, payload.len()).expect("gpu helper frame decode");
            assert_payload_eq(&decoded_gpu, payload, "helper zstd decode");
        }
    }

    #[test]
    fn cuda_large_corpus_validation_when_available() {
        let mut patterned = Vec::with_capacity(512 * 1024);
        for idx in 0..(512 * 1024) {
            patterned.push(match idx % 17 {
                0..=7 => b'A',
                8..=11 => b'B',
                12..=14 => (idx as u8).wrapping_mul(31),
                _ => b'\n',
            });
        }
        let repeated = vec![0x5au8; 256 * 1024];
        let randomish = lcg_payload(384 * 1024, 0x9e37_79b9_7f4a_7c15);

        for (case, payload) in [
            ("patterned", patterned),
            ("repeated", repeated),
            ("randomish", randomish),
        ] {
            let compressed_a = match try_gpu_compress_level(&payload, 3) {
                Ok(bytes) => bytes,
                Err(rc) => {
                    if skip_if_unavailable(rc) {
                        return;
                    }
                    panic!("gpu compress failed: {rc}");
                }
            };
            let compressed_b = try_gpu_compress_level(&payload, 3).expect("second gpu encode");
            assert_eq!(
                compressed_a, compressed_b,
                "CUDA zstd output must be deterministic for identical input"
            );

            let cpu_decoded = zstd::decode_all(Cursor::new(&compressed_a)).expect("cpu decode");
            assert_payload_eq(&cpu_decoded, &payload, case);

            let helper_decoded =
                try_gpu_decompress(&compressed_a, payload.len()).expect("helper decode");
            assert_payload_eq(&helper_decoded, &payload, case);
        }
    }

    #[test]
    fn cuda_required_env_fails_if_helper_is_not_accelerated() {
        if std::env::var_os("GPUZSTD_CUDA_REQUIRE").is_none() {
            return;
        }
        let payload = b"required cuda validation payload required cuda validation payload";
        let compressed = try_gpu_compress(payload)
            .expect("GPUZSTD_CUDA_REQUIRE requires CUDA compression to succeed");
        let decoded = zstd::decode_all(Cursor::new(&compressed)).expect("cpu decode");
        assert_eq!(decoded, payload);
    }
}
