//! Shared Metal backing-pool extents used by allocation owners and admission.

/// Exact alignment of a shared Metal backing page.
pub const METAL_PAGE_BYTES: usize = 16 * 1024;
/// Maximum pages retained across every idle pooled allocation.
pub const METAL_POOL_MAX_CACHED_PAGES: usize = 4096;
/// Covers idle pages and oversized retained pages reused for a smaller request.
pub const METAL_POOL_MAX_CACHED_BYTES: usize = METAL_PAGE_BYTES * METAL_POOL_MAX_CACHED_PAGES;
