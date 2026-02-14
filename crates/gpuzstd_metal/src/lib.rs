//! GPU-assisted zstd helper for Norito (Metal).
//!
//! C ABI:
//! - gpu_zstd_compress(src_ptr, src_len, level, dst_ptr, dst_len)
//! - gpu_zstd_decompress(src_ptr, src_len, dst_ptr, dst_len)

#[allow(dead_code)]
mod bitstream;
#[allow(dead_code)]
mod fse;
#[allow(dead_code)]
mod huffman;
#[allow(dead_code)]
mod zstd_frame;

use std::{io::Cursor, ptr, slice};

use crate::zstd_frame::ZstdEncodeError;

const RC_OK: i32 = 0;
const RC_INVALID: i32 = 1;
const RC_NO_SPACE: i32 = 2;
#[cfg_attr(all(target_os = "macos", target_arch = "aarch64"), allow(dead_code))]
const RC_GPU_UNAVAILABLE: i32 = 3;
const RC_ZSTD: i32 = 4;

const CHUNK_SIZE: u32 = 32 * 1024;
const MIN_MATCH: u32 = 3;
const MAX_MATCH: u32 = 64;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub(crate) struct GpuZstdSequence {
    lit_len: u32,
    match_len: u32,
    offset: u32,
    reserved: u32,
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
unsafe extern "C" {
    fn gpuzstd_metal_count_sequences(
        input: *const u8,
        input_len: usize,
        chunk_size: u32,
        min_match: u32,
        max_match: u32,
        out_counts: *mut u32,
        counts_len: u32,
    ) -> i32;

    fn gpuzstd_metal_write_sequences(
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

    #[cfg(test)]
    fn gpuzstd_metal_huff_encode(
        input: *const u8,
        input_len: usize,
        out_bytes: *mut u8,
        out_capacity: usize,
        out_len: *mut usize,
        out_lengths: *mut u8,
        lengths_len: usize,
    ) -> i32;

    #[cfg(test)]
    fn gpuzstd_metal_huff_decode(
        encoded: *const u8,
        encoded_len: usize,
        lengths: *const u8,
        lengths_len: usize,
        out_bytes: *mut u8,
        out_len: usize,
    ) -> i32;

    #[cfg(test)]
    fn gpuzstd_metal_fse_encode(
        symbols: *const u16,
        symbols_len: usize,
        normalized: *const i16,
        normalized_len: usize,
        max_symbol: u32,
        table_log: u32,
        out_bytes: *mut u8,
        out_capacity: usize,
        out_len: *mut usize,
    ) -> i32;

    #[cfg(test)]
    fn gpuzstd_metal_fse_decode(
        encoded: *const u8,
        encoded_len: usize,
        normalized: *const i16,
        normalized_len: usize,
        max_symbol: u32,
        table_log: u32,
        out_symbols: *mut u16,
        out_len: usize,
    ) -> i32;
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
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
        gpuzstd_metal_count_sequences(
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
    let mut gpu_seqs = vec![GpuZstdSequence::default(); seq_len];
    let rc = unsafe {
        gpuzstd_metal_write_sequences(
            input.as_ptr(),
            input.len(),
            CHUNK_SIZE,
            MIN_MATCH,
            MAX_MATCH,
            offsets.as_ptr(),
            offsets.len() as u32,
            gpu_seqs.as_mut_ptr(),
            gpu_seqs.len() as u32,
        )
    };
    if rc != RC_OK {
        return Err(rc);
    }

    let mut consumed: u64 = 0;
    for seq in &gpu_seqs {
        consumed = consumed.saturating_add(seq.lit_len as u64);
        consumed = consumed.saturating_add(seq.match_len as u64);
    }
    if consumed != input.len() as u64 {
        return Err(RC_ZSTD);
    }
    Ok(GpuSequences {
        counts,
        offsets,
        seqs: gpu_seqs,
    })
}

#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
fn gpu_sequences(_input: &[u8]) -> Result<GpuSequences, i32> {
    Err(RC_GPU_UNAVAILABLE)
}

#[derive(Default)]
struct GpuSequences {
    counts: Vec<u32>,
    offsets: Vec<u32>,
    seqs: Vec<GpuZstdSequence>,
}

#[unsafe(no_mangle)]
/// Compress `src` into `dst` using the Metal-assisted zstd path.
///
/// # Safety
/// `src` must point to `src_len` readable bytes. `dst` must point to a writable buffer whose
/// capacity is provided via `*dst_len`. `dst_len` must be non-null and writable.
pub unsafe extern "C" fn gpu_zstd_compress(
    src: *const u8,
    src_len: usize,
    level: i32,
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
            Err(_) => match zstd::encode_all(Cursor::new(src_slice), level) {
                Ok(bytes) => bytes,
                Err(_) => return RC_ZSTD,
            },
        },
        // Keep the ABI stable: CPU fallback guarantees valid standard zstd frames
        // when GPU preprocessing does not produce usable sequences.
        Err(_) => match zstd::encode_all(Cursor::new(src_slice), level) {
            Ok(bytes) => bytes,
            Err(_) => return RC_ZSTD,
        },
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

#[unsafe(no_mangle)]
/// Decompress `src` into `dst` using the Metal-assisted zstd path.
///
/// # Safety
/// `src` must point to `src_len` readable bytes. `dst` must point to a writable buffer whose
/// capacity is provided via `*dst_len`. `dst_len` must be non-null and writable.
pub unsafe extern "C" fn gpu_zstd_decompress(
    src: *const u8,
    src_len: usize,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{fse, huffman};
    use std::time::Instant;

    fn lcg_payload(len: usize, mut seed: u64) -> Vec<u8> {
        let mut out = vec![0u8; len];
        for byte in &mut out {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            *byte = (seed >> 32) as u8;
        }
        out
    }

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

    fn skip_if_unavailable(rc: i32) -> bool {
        if rc == RC_GPU_UNAVAILABLE {
            eprintln!("gpuzstd_metal unavailable; skipping test");
            true
        } else {
            false
        }
    }

    #[test]
    fn gpu_roundtrip_matches_cpu() {
        let payload = b"gpuzstd metal roundtrip";
        let compressed = match try_gpu_compress(payload) {
            Ok(bytes) => bytes,
            Err(rc) => {
                if skip_if_unavailable(rc) {
                    return;
                }
                panic!("gpu compress failed: {rc}");
            }
        };
        let decoded = match try_gpu_decompress(&compressed, payload.len()) {
            Ok(bytes) => bytes,
            Err(rc) => {
                if skip_if_unavailable(rc) {
                    return;
                }
                panic!("gpu decompress failed: {rc}");
            }
        };
        assert_eq!(decoded, payload);
    }

    #[test]
    fn gpu_decode_handles_cpu_zstd() {
        let payload = b"gpuzstd metal cpu decode";
        let cpu_encoded = zstd::encode_all(Cursor::new(payload), 1).expect("cpu encode");
        let decoded = match try_gpu_decompress(&cpu_encoded, payload.len()) {
            Ok(bytes) => bytes,
            Err(rc) => {
                if skip_if_unavailable(rc) {
                    return;
                }
                panic!("gpu decompress failed: {rc}");
            }
        };
        assert_eq!(decoded, payload);
    }

    #[test]
    fn gpu_encode_matches_cpu_zstd() {
        let payload = b"gpuzstd metal encode";
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
    fn gpu_determinism_corpus_roundtrip() {
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
            assert_eq!(decoded_cpu, payload);

            let decoded_gpu = match try_gpu_decompress(&compressed_a, payload.len()) {
                Ok(bytes) => bytes,
                Err(rc) => {
                    if skip_if_unavailable(rc) {
                        return;
                    }
                    panic!("gpu decompress failed: {rc}");
                }
            };
            assert_eq!(decoded_gpu, payload);
        }
    }

    #[test]
    #[ignore]
    fn gpu_vs_cpu_benchmark() {
        let sizes = [4 * 1024, 32 * 1024, 128 * 1024, 1024 * 1024];
        let iterations = [10usize, 6, 4, 2];

        for (&size, &iters) in sizes.iter().zip(iterations.iter()) {
            let payload = lcg_payload(size, 0x1234_5678_9abc_def0 ^ size as u64);
            let cpu_start = Instant::now();
            for _ in 0..iters {
                let _ = zstd::encode_all(Cursor::new(&payload), 1).expect("cpu encode");
            }
            let cpu_elapsed = cpu_start.elapsed();

            let gpu_start = Instant::now();
            let mut last = None;
            for _ in 0..iters {
                let compressed = match try_gpu_compress(&payload) {
                    Ok(bytes) => bytes,
                    Err(rc) => {
                        if skip_if_unavailable(rc) {
                            return;
                        }
                        panic!("gpu compress failed: {rc}");
                    }
                };
                last = Some(compressed);
            }
            let gpu_elapsed = gpu_start.elapsed();

            if let Some(compressed) = last {
                let decoded = zstd::decode_all(Cursor::new(&compressed)).expect("cpu decode");
                assert_eq!(decoded.len(), payload.len());
            }

            let cpu_avg = cpu_elapsed.as_secs_f64() * 1e6 / iters as f64;
            let gpu_avg = gpu_elapsed.as_secs_f64() * 1e6 / iters as f64;
            println!(
                "size={} bytes iters={} cpu_avg_us={:.1} gpu_avg_us={:.1}",
                size, iters, cpu_avg, gpu_avg
            );
        }
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
    fn gpu_compress_does_not_surface_unavailable_rc() {
        let payload = b"gpuzstd availability fallback";
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
        assert_ne!(
            rc, RC_GPU_UNAVAILABLE,
            "compress should fall back to CPU when Metal is temporarily unavailable"
        );
        assert_eq!(rc, RC_OK);
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn gpu_huffman_parity() {
        let payload = b"gpuzstd metal huffman parity payload";
        let (cpu_encoded, cpu_table) = huffman::encode_literals(payload).expect("cpu encode");
        let mut gpu_out = vec![0u8; cpu_encoded.len().saturating_mul(2).saturating_add(16)];
        let mut gpu_len = gpu_out.len();
        let mut gpu_lengths = [0u8; 256];
        let rc = unsafe {
            gpuzstd_metal_huff_encode(
                payload.as_ptr(),
                payload.len(),
                gpu_out.as_mut_ptr(),
                gpu_out.len(),
                &mut gpu_len,
                gpu_lengths.as_mut_ptr(),
                gpu_lengths.len(),
            )
        };
        if rc == RC_GPU_UNAVAILABLE {
            eprintln!("gpuzstd_metal unavailable; skipping test");
            return;
        }
        assert_eq!(rc, RC_OK);
        gpu_out.truncate(gpu_len);
        assert_eq!(gpu_lengths, cpu_table.lengths);
        assert_eq!(gpu_out, cpu_encoded);
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn gpu_huffman_decode_roundtrip() {
        let payload = b"gpuzstd metal huffman decode roundtrip";
        let (cpu_encoded, cpu_table) = huffman::encode_literals(payload).expect("cpu encode");
        let mut gpu_out = vec![0u8; payload.len()];
        let rc = unsafe {
            gpuzstd_metal_huff_decode(
                cpu_encoded.as_ptr(),
                cpu_encoded.len(),
                cpu_table.lengths.as_ptr(),
                cpu_table.lengths.len(),
                gpu_out.as_mut_ptr(),
                gpu_out.len(),
            )
        };
        if rc == RC_GPU_UNAVAILABLE {
            eprintln!("gpuzstd_metal unavailable; skipping test");
            return;
        }
        assert_eq!(rc, RC_OK);
        assert_eq!(gpu_out, payload);
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn gpu_fse_parity() {
        let symbols: Vec<u16> = vec![0, 1, 2, 3, 2, 1, 0, 3, 3, 2, 1];
        let mut counts = vec![0u32; 4];
        for &sym in &symbols {
            counts[sym as usize] += 1;
        }
        let norm = fse::normalize_counts(&counts, 5).expect("normalize");
        let (ct, dt) = fse::build_tables(&norm, 3, 5).expect("tables");
        let cpu_encoded = fse::encode_symbols(&symbols, &ct).expect("cpu encode");

        let mut gpu_out = vec![0u8; cpu_encoded.len().saturating_mul(2).saturating_add(16)];
        let mut gpu_len = gpu_out.len();
        let rc = unsafe {
            gpuzstd_metal_fse_encode(
                symbols.as_ptr(),
                symbols.len(),
                norm.as_ptr(),
                norm.len(),
                3,
                5,
                gpu_out.as_mut_ptr(),
                gpu_out.len(),
                &mut gpu_len,
            )
        };
        if rc == RC_GPU_UNAVAILABLE {
            eprintln!("gpuzstd_metal unavailable; skipping test");
            return;
        }
        assert_eq!(rc, RC_OK);
        gpu_out.truncate(gpu_len);
        assert_eq!(gpu_out, cpu_encoded);

        let mut gpu_symbols = vec![0u16; symbols.len()];
        let rc = unsafe {
            gpuzstd_metal_fse_decode(
                gpu_out.as_ptr(),
                gpu_out.len(),
                norm.as_ptr(),
                norm.len(),
                3,
                5,
                gpu_symbols.as_mut_ptr(),
                gpu_symbols.len(),
            )
        };
        assert_eq!(rc, RC_OK);
        assert_eq!(gpu_symbols, symbols);

        let cpu_decoded = fse::decode_symbols(&cpu_encoded, symbols.len(), &dt).expect("decode");
        assert_eq!(cpu_decoded, symbols);
    }
}
