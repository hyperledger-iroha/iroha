//! Shared Metal backing-pool extents used by allocation owners and admission.

/// Exact alignment of a shared Metal backing page.
pub const METAL_PAGE_BYTES: usize = 16 * 1024;
/// Maximum pages retained across every idle pooled allocation.
pub const METAL_POOL_MAX_CACHED_PAGES: usize = 4096;
/// Covers idle pages and oversized retained pages reused for a smaller request.
pub const METAL_POOL_MAX_CACHED_BYTES: usize = METAL_PAGE_BYTES * METAL_POOL_MAX_CACHED_PAGES;

/// One shared cache holds both stage-only and factorized exact-root tables.
pub const METAL_TWIDDLE_CACHE_MAX_ENTRIES: usize = 64;
/// Four radix-256 digit tables cover every public u32 exponent.
pub const METAL_FACTORIZED_TWIDDLE_WORDS: usize = 4 * 256;
/// Largest logical buffer payload; stage-only entries use at most 32 words.
pub const METAL_TWIDDLE_MAX_ENTRY_BYTES: usize = METAL_FACTORIZED_TWIDDLE_WORDS * 8;
/// Full cache plus one public construction owner and one live returned buffer.
/// Physical driver allocation overhead remains outside this logical payload.
pub const METAL_TWIDDLE_PAYLOAD_ALLOWANCE: usize =
    (METAL_TWIDDLE_CACHE_MAX_ENTRIES + 2) * METAL_TWIDDLE_MAX_ENTRY_BYTES;
