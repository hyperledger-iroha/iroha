//! Reed-Solomon (16-bit) parity helpers shared across Torii and tooling.

use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
};

const FIELD_ORDER: usize = 1 << 16;
const FIELD_MASK: u32 = 0x1_0000;
const FIELD_POLY: u32 = 0x1_100B; // x^16 + x^12 + x^3 + x + 1
const ORDER_MINUS_ONE: usize = FIELD_ORDER - 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Backend {
    Scalar,
    #[cfg(all(
        feature = "simd-accel",
        any(target_arch = "x86", target_arch = "x86_64")
    ))]
    Avx2,
    #[cfg(all(feature = "simd-accel", target_arch = "aarch64"))]
    Neon,
}

/// Error returned when RS16 parity encoding fails.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rs16Error;

struct FieldTables {
    exp: Vec<u16>,
    log: Vec<u16>,
}

static SIMD_ENABLED: AtomicBool = AtomicBool::new(true);

/// Enable or disable SIMD acceleration for RS16 parity encoding.
pub fn set_simd_enabled(enabled: bool) {
    SIMD_ENABLED.store(enabled, Ordering::SeqCst);
}

/// Returns true when RS16 SIMD acceleration is enabled.
pub fn simd_enabled() -> bool {
    SIMD_ENABLED.load(Ordering::SeqCst)
}

fn tables() -> &'static FieldTables {
    static TABLES: OnceLock<FieldTables> = OnceLock::new();
    TABLES.get_or_init(|| {
        let mut exp = vec![0u16; ORDER_MINUS_ONE * 2];
        let mut log = vec![0u16; FIELD_ORDER];

        let mut value: u32 = 1;
        for (idx, slot) in exp.iter_mut().take(ORDER_MINUS_ONE).enumerate() {
            *slot = value as u16;
            log[value as usize] = idx as u16;

            value <<= 1;
            if (value & FIELD_MASK) != 0 {
                value ^= FIELD_POLY;
            }
            value &= FIELD_MASK - 1;
        }

        let (lower, upper) = exp.split_at_mut(ORDER_MINUS_ONE);
        upper.copy_from_slice(lower);

        FieldTables { exp, log }
    })
}

#[inline]
fn gf_add(a: u16, b: u16) -> u16 {
    a ^ b
}

#[inline]
fn gf_mul(a: u16, b: u16) -> u16 {
    if a == 0 || b == 0 {
        return 0;
    }
    let tables = tables();
    let log_a = tables.log[a as usize] as usize;
    let log_b = tables.log[b as usize] as usize;
    tables.exp[log_a + log_b]
}

#[inline]
fn gf_inv(value: u16) -> Option<u16> {
    if value == 0 {
        return None;
    }
    let tables = tables();
    let log_v = tables.log[value as usize] as usize;
    Some(tables.exp[ORDER_MINUS_ONE - log_v])
}

#[inline]
fn gf_pow(exp: usize) -> u16 {
    let tables = tables();
    tables.exp[exp % ORDER_MINUS_ONE]
}

#[allow(clippy::needless_range_loop)]
fn invert_matrix(mut matrix: Vec<Vec<u16>>) -> Result<Vec<Vec<u16>>, Rs16Error> {
    let size = matrix.len();
    if size == 0 {
        return Ok(Vec::new());
    }
    let width = matrix[0].len();
    if size != width {
        return Err(Rs16Error);
    }
    let mut identity = vec![vec![0u16; size]; size];
    for i in 0..size {
        identity[i][i] = 1;
    }

    for col in 0..size {
        let mut pivot_row = None;
        for row in col..size {
            if matrix[row][col] != 0 {
                pivot_row = Some(row);
                break;
            }
        }
        let pivot_row = pivot_row.ok_or(Rs16Error)?;
        if pivot_row != col {
            matrix.swap(pivot_row, col);
            identity.swap(pivot_row, col);
        }
        let inv = gf_inv(matrix[col][col]).ok_or(Rs16Error)?;
        for j in 0..size {
            matrix[col][j] = gf_mul(matrix[col][j], inv);
            identity[col][j] = gf_mul(identity[col][j], inv);
        }
        for row in 0..size {
            if row == col {
                continue;
            }
            let factor = matrix[row][col];
            if factor == 0 {
                continue;
            }
            for j in 0..size {
                let term = gf_mul(factor, matrix[col][j]);
                matrix[row][j] = gf_add(matrix[row][j], term);
                let term = gf_mul(factor, identity[col][j]);
                identity[row][j] = gf_add(identity[row][j], term);
            }
        }
    }

    Ok(identity)
}

#[derive(Clone)]
struct ParityMatrix {
    rows: Vec<Vec<u16>>,
}

fn parity_matrix(data_shards: usize, parity_shards: usize) -> Result<Arc<ParityMatrix>, Rs16Error> {
    static CACHE: OnceLock<Mutex<HashMap<(usize, usize), Arc<ParityMatrix>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(entry) = cache
        .lock()
        .expect("mutex poisoned")
        .get(&(data_shards, parity_shards))
    {
        return Ok(entry.clone());
    }

    let total = data_shards + parity_shards;
    let mut vandermonde = vec![vec![0u16; data_shards]; total];
    for (row_idx, row) in vandermonde.iter_mut().enumerate() {
        for (col_idx, value) in row.iter_mut().enumerate() {
            *value = if row_idx == 0 || col_idx == 0 {
                1
            } else {
                gf_pow(row_idx * col_idx)
            };
        }
    }

    let data_block = invert_matrix(vandermonde[..data_shards].to_vec())?;
    let mut parity_rows = vec![vec![0u16; data_shards]; parity_shards];
    for (parity_idx, parity_row) in parity_rows.iter_mut().enumerate() {
        let source = &vandermonde[data_shards + parity_idx];
        for (col_idx, slot) in parity_row.iter_mut().enumerate() {
            let mut acc = 0u16;
            for (data_idx, src_val) in source.iter().take(data_shards).enumerate() {
                let term = gf_mul(*src_val, data_block[data_idx][col_idx]);
                acc = gf_add(acc, term);
            }
            *slot = acc;
        }
    }

    let matrix = Arc::new(ParityMatrix { rows: parity_rows });
    cache
        .lock()
        .expect("mutex poisoned")
        .insert((data_shards, parity_shards), matrix.clone());
    Ok(matrix)
}

fn choose_backend() -> Backend {
    if !simd_enabled() {
        return Backend::Scalar;
    }
    #[cfg(all(
        feature = "simd-accel",
        any(target_arch = "x86", target_arch = "x86_64")
    ))]
    if std::arch::is_x86_feature_detected!("avx2") {
        return Backend::Avx2;
    }
    #[cfg(all(feature = "simd-accel", target_arch = "aarch64"))]
    if std::arch::is_aarch64_feature_detected!("neon") {
        return Backend::Neon;
    }
    Backend::Scalar
}

/// Encode RS16 parity for the provided data symbols and parity shard count.
pub fn encode_parity(
    data_symbols: &[Vec<u16>],
    parity_count: usize,
) -> Result<Vec<Vec<u16>>, Rs16Error> {
    encode_parity_with_backend(data_symbols, parity_count, choose_backend())
}

/// Convert a payload slice into RS16 symbols using the configured chunk size.
pub fn symbols_from_payload(
    chunk_size: usize,
    payload: &[u8],
    offset: usize,
    length: usize,
) -> Result<Vec<u16>, Rs16Error> {
    if chunk_size < 2 || length > chunk_size {
        return Err(Rs16Error);
    }
    let end = offset.checked_add(length).ok_or(Rs16Error)?;
    if end > payload.len() {
        return Err(Rs16Error);
    }
    let symbol_count = chunk_size / 2;
    Ok(symbols_from_chunk(symbol_count, &payload[offset..end]))
}

/// Convert chunk bytes into RS16 symbols padded to the requested symbol count.
pub fn symbols_from_chunk(symbol_count: usize, chunk: &[u8]) -> Vec<u16> {
    let mut symbols = vec![0u16; symbol_count];
    let mut cursor = 0usize;
    for slot in &mut symbols {
        if cursor >= chunk.len() {
            break;
        }
        let mut pair = [0u8; 2];
        let remaining = 2.min(chunk.len() - cursor);
        pair[..remaining].copy_from_slice(&chunk[cursor..cursor + remaining]);
        *slot = u16::from_le_bytes(pair);
        cursor += remaining;
    }
    symbols
}

/// Compute the parity chunk offset for a given stripe/parity index.
pub fn parity_offset(
    total_size: u64,
    stripe_index: usize,
    parity_index: usize,
    parity_shards: usize,
    chunk_size: u32,
) -> Option<u64> {
    let stride = u64::from(chunk_size);
    let slot = u64::try_from(stripe_index)
        .ok()?
        .checked_mul(parity_shards as u64)?
        .checked_add(parity_index as u64)?;
    total_size.checked_add(slot.checked_mul(stride)?)
}

fn encode_parity_with_backend(
    data_symbols: &[Vec<u16>],
    parity_count: usize,
    backend: Backend,
) -> Result<Vec<Vec<u16>>, Rs16Error> {
    if data_symbols.is_empty() || parity_count == 0 {
        return Ok(Vec::new());
    }
    let symbol_count = data_symbols[0].len();
    if data_symbols.iter().any(|row| row.len() != symbol_count) {
        return Err(Rs16Error);
    }
    let matrix = parity_matrix(data_symbols.len(), parity_count)?;
    let mut parity = vec![vec![0u16; symbol_count]; parity_count];

    for (row_idx, coeffs) in matrix.rows.iter().enumerate() {
        let row = &mut parity[row_idx];
        for (data_idx, coef) in coeffs.iter().enumerate() {
            if *coef == 0 {
                continue;
            }
            let data_row = &data_symbols[data_idx];
            mul_add_row(*coef, data_row, row, backend);
        }
    }

    Ok(parity)
}

#[allow(unsafe_code)]
fn mul_add_row(coef: u16, data_row: &[u16], out: &mut [u16], backend: Backend) {
    match backend {
        Backend::Scalar => mul_add_row_scalar(coef, data_row, out),
        #[cfg(all(
            feature = "simd-accel",
            any(target_arch = "x86", target_arch = "x86_64")
        ))]
        Backend::Avx2 => {
            // SAFETY: Backend::Avx2 is only selected when AVX2 is available.
            unsafe {
                avx2::mul_add_row_avx2(coef, data_row, out);
            }
        }
        #[cfg(all(feature = "simd-accel", target_arch = "aarch64"))]
        Backend::Neon => {
            // SAFETY: Backend::Neon is only selected when NEON is available.
            unsafe {
                neon::mul_add_row_neon(coef, data_row, out);
            }
        }
    }
}

fn mul_add_row_scalar(coef: u16, data_row: &[u16], out: &mut [u16]) {
    debug_assert_eq!(data_row.len(), out.len());
    if coef == 0 {
        return;
    }
    if coef == 1 {
        for (slot, symbol) in out.iter_mut().zip(data_row.iter()) {
            *slot ^= symbol;
        }
        return;
    }
    for (slot, symbol) in out.iter_mut().zip(data_row.iter()) {
        let term = gf_mul(coef, *symbol);
        *slot ^= term;
    }
}

#[cfg(all(
    feature = "simd-accel",
    any(target_arch = "x86", target_arch = "x86_64")
))]
mod avx2 {
    #[cfg(target_arch = "x86")]
    use std::arch::x86 as arch;
    #[cfg(target_arch = "x86_64")]
    use std::arch::x86_64 as arch;

    use super::gf_mul;

    /// AVX2 path that vectorizes the XOR accumulation; gf_mul remains scalar.
    // TODO: Vectorize gf_mul to avoid per-symbol scalar multiplication.
    #[allow(unsafe_code)]
    #[target_feature(enable = "avx2")]
    pub(super) unsafe fn mul_add_row_avx2(coef: u16, data_row: &[u16], out: &mut [u16]) {
        // SAFETY: Caller ensures AVX2 is available and slices are equal length.
        unsafe {
            debug_assert_eq!(data_row.len(), out.len());
            if coef == 0 {
                return;
            }
            let len = data_row.len();
            let mut idx = 0usize;
            if coef == 1 {
                while idx + 16 <= len {
                    let data_vec = arch::_mm256_loadu_si256(
                        data_row.as_ptr().add(idx) as *const arch::__m256i
                    );
                    let out_vec =
                        arch::_mm256_loadu_si256(out.as_ptr().add(idx) as *const arch::__m256i);
                    let merged = arch::_mm256_xor_si256(out_vec, data_vec);
                    arch::_mm256_storeu_si256(
                        out.as_mut_ptr().add(idx) as *mut arch::__m256i,
                        merged,
                    );
                    idx += 16;
                }
                for (slot, symbol) in out.iter_mut().zip(data_row.iter()).skip(idx) {
                    *slot ^= symbol;
                }
                return;
            }

            let mut terms = [0u16; 16];
            while idx + 16 <= len {
                for (offset, term) in terms.iter_mut().enumerate() {
                    *term = gf_mul(coef, data_row[idx + offset]);
                }
                let term_vec = arch::_mm256_loadu_si256(terms.as_ptr() as *const arch::__m256i);
                let out_vec =
                    arch::_mm256_loadu_si256(out.as_ptr().add(idx) as *const arch::__m256i);
                let merged = arch::_mm256_xor_si256(out_vec, term_vec);
                arch::_mm256_storeu_si256(out.as_mut_ptr().add(idx) as *mut arch::__m256i, merged);
                idx += 16;
            }
            for (slot, symbol) in out.iter_mut().zip(data_row.iter()).skip(idx) {
                *slot ^= gf_mul(coef, *symbol);
            }
        }
    }
}

#[cfg(all(feature = "simd-accel", target_arch = "aarch64"))]
mod neon {
    use std::arch::aarch64 as arch;

    use super::gf_mul;

    /// NEON path that vectorizes the XOR accumulation; gf_mul remains scalar.
    // TODO: Vectorize gf_mul to avoid per-symbol scalar multiplication.
    #[allow(unsafe_code)]
    #[target_feature(enable = "neon")]
    pub(super) unsafe fn mul_add_row_neon(coef: u16, data_row: &[u16], out: &mut [u16]) {
        // SAFETY: Caller ensures NEON is available and slices are equal length.
        unsafe {
            debug_assert_eq!(data_row.len(), out.len());
            if coef == 0 {
                return;
            }
            let len = data_row.len();
            let mut idx = 0usize;
            if coef == 1 {
                while idx + 8 <= len {
                    let data_vec = arch::vld1q_u16(data_row.as_ptr().add(idx));
                    let out_vec = arch::vld1q_u16(out.as_ptr().add(idx));
                    let merged = arch::veorq_u16(out_vec, data_vec);
                    arch::vst1q_u16(out.as_mut_ptr().add(idx), merged);
                    idx += 8;
                }
                for (slot, symbol) in out.iter_mut().zip(data_row.iter()).skip(idx) {
                    *slot ^= symbol;
                }
                return;
            }

            let mut terms = [0u16; 8];
            while idx + 8 <= len {
                for (offset, term) in terms.iter_mut().enumerate() {
                    *term = gf_mul(coef, data_row[idx + offset]);
                }
                let term_vec = arch::vld1q_u16(terms.as_ptr());
                let out_vec = arch::vld1q_u16(out.as_ptr().add(idx));
                let merged = arch::veorq_u16(out_vec, term_vec);
                arch::vst1q_u16(out.as_mut_ptr().add(idx), merged);
                idx += 8;
            }
            for (slot, symbol) in out.iter_mut().zip(data_row.iter()).skip(idx) {
                *slot ^= gf_mul(coef, *symbol);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_symbols(rows: usize, symbols: usize) -> Vec<Vec<u16>> {
        let mut data = Vec::with_capacity(rows);
        for row in 0..rows {
            let mut symbols_row = Vec::with_capacity(symbols);
            for idx in 0..symbols {
                symbols_row.push(((row as u16) << 8) ^ (idx as u16));
            }
            data.push(symbols_row);
        }
        data
    }

    #[test]
    fn parity_shape_is_stable() {
        let data = sample_symbols(4, 64);
        let parity = encode_parity(&data, 3).expect("parity");
        assert_eq!(parity.len(), 3);
        assert!(parity.iter().all(|row| row.len() == 64));
    }

    #[test]
    fn simd_toggle_roundtrip() {
        let _guard = SIMULATED_GUARD
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap();
        let prev = simd_enabled();
        set_simd_enabled(!prev);
        assert_eq!(simd_enabled(), !prev);
        set_simd_enabled(prev);
    }

    #[test]
    fn simd_backends_match_scalar() {
        let data = sample_symbols(6, 128);
        let scalar = encode_parity_with_backend(&data, 4, Backend::Scalar).expect("scalar");
        #[cfg(all(
            feature = "simd-accel",
            any(target_arch = "x86", target_arch = "x86_64")
        ))]
        if std::arch::is_x86_feature_detected!("avx2") {
            let avx = encode_parity_with_backend(&data, 4, Backend::Avx2).expect("avx2");
            assert_eq!(scalar, avx);
        }
        #[cfg(all(feature = "simd-accel", target_arch = "aarch64"))]
        if std::arch::is_aarch64_feature_detected!("neon") {
            let neon = encode_parity_with_backend(&data, 4, Backend::Neon).expect("neon");
            assert_eq!(scalar, neon);
        }
    }

    static SIMULATED_GUARD: OnceLock<Mutex<()>> = OnceLock::new();

    #[test]
    fn symbols_from_chunk_packs_pairs() {
        let chunk = [0x01u8, 0x02, 0x03];
        let symbols = symbols_from_chunk(2, &chunk);
        assert_eq!(symbols, vec![0x0201, 0x0003]);
    }

    #[test]
    fn symbols_from_payload_bounds_check() {
        let payload = [0u8; 4];
        let ok = symbols_from_payload(4, &payload, 0, 4).expect("symbols");
        assert_eq!(ok.len(), 2);
        assert!(symbols_from_payload(2, &payload, 0, 4).is_err());
        assert!(symbols_from_payload(4, &payload, 3, 2).is_err());
    }

    #[test]
    fn parity_offset_matches_expected() {
        let offset = parity_offset(100, 2, 1, 3, 16).expect("offset");
        assert_eq!(offset, 100 + (2 * 3 + 1) as u64 * 16);
    }
}
