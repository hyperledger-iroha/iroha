//! Basic tests for compress_auto behavior under workspace feature flags.
//!
//! In this workspace, `norito` is built with `default-features = false` and
//! without the `compression` feature. Ensure `compress_auto` degrades
//! gracefully and returns `Compression::None` even when payload size would
//! normally trigger compression.

#[test]
fn compress_auto_is_noop_without_compression_feature() {
    use norito::{Compression, core::heuristics::compress_auto};

    let payload = vec![0u8; 8 * 1024];
    let (algo, out) = compress_auto(payload.clone()).expect("compress_auto should not fail");
    // Always reference output to avoid unused variable warnings under cfg variations
    let _ = &out;

    // With `compression` disabled, we must not attempt to call into zstd and
    // should return the original payload unchanged. With `compression` enabled,
    // the selector should choose zstd for a sufficiently large payload.
    #[cfg(not(feature = "compression"))]
    {
        assert_eq!(algo, Compression::None);
        assert_eq!(out, payload);
    }
    #[cfg(feature = "compression")]
    {
        assert_eq!(algo, Compression::Zstd);
        // Do not assert on size; zstd may occasionally expand very small or
        // random payloads. The important part is that it did not error and
        // chose the zstd path.
    }
}
