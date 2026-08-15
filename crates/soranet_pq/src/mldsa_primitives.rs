//! Safe, portable FIPS 204 polynomial primitives used by the ML-DSA adapter.
//!
//! This module deliberately owns its Rust data structures. Nothing here is passed across an FFI
//! boundary, so changing an upstream C implementation cannot invalidate Rust layout assumptions.
// These conversions and index loops mirror FIPS 204's explicitly fixed-width
// polynomial arithmetic and byte encodings.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::many_single_char_names,
    clippy::needless_range_loop,
    clippy::similar_names,
    clippy::unreadable_literal
)]
use core::array;
use sha3::{
    Shake128, Shake256,
    digest::{ExtendableOutput, Update, XofReader},
};
use zeroize::{Zeroize, ZeroizeOnDrop};
const N: usize = 256;
const Q: i32 = 8_380_417;
const D: u32 = 13;
const QINV: u64 = 58_728_449;
const ZETAS: [i32; N] = [
    0, 25847, -2608894, -518909, 237124, -777960, -876248, 466468, 1826347, 2353451, -359251,
    -2091905, 3119733, -2884855, 3111497, 2680103, 2725464, 1024112, -1079900, 3585928, -549488,
    -1119584, 2619752, -2108549, -2118186, -3859737, -1399561, -3277672, 1757237, -19422, 4010497,
    280005, 2706023, 95776, 3077325, 3530437, -1661693, -3592148, -2537516, 3915439, -3861115,
    -3043716, 3574422, -2867647, 3539968, -300467, 2348700, -539299, -1699267, -1643818, 3505694,
    -3821735, 3507263, -2140649, -1600420, 3699596, 811944, 531354, 954230, 3881043, 3900724,
    -2556880, 2071892, -2797779, -3930395, -1528703, -3677745, -3041255, -1452451, 3475950,
    2176455, -1585221, -1257611, 1939314, -4083598, -1000202, -3190144, -3157330, -3632928, 126922,
    3412210, -983419, 2147896, 2715295, -2967645, -3693493, -411027, -2477047, -671102, -1228525,
    -22981, -1308169, -381987, 1349076, 1852771, -1430430, -3343383, 264944, 508951, 3097992,
    44288, -1100098, 904516, 3958618, -3724342, -8578, 1653064, -3249728, 2389356, -210977, 759969,
    -1316856, 189548, -3553272, 3159746, -1851402, -2409325, -177440, 1315589, 1341330, 1285669,
    -1584928, -812732, -1439742, -3019102, -3881060, -3628969, 3839961, 2091667, 3407706, 2316500,
    3817976, -3342478, 2244091, -2446433, -3562462, 266997, 2434439, -1235728, 3513181, -3520352,
    -3759364, -1197226, -3193378, 900702, 1859098, 909542, 819034, 495491, -1613174, -43260,
    -522500, -655327, -3122442, 2031748, 3207046, -3556995, -525098, -768622, -3595838, 342297,
    286988, -2437823, 4108315, 3437287, -3342277, 1735879, 203044, 2842341, 2691481, -2590150,
    1265009, 4055324, 1247620, 2486353, 1595974, -3767016, 1250494, 2635921, -3548272, -2994039,
    1869119, 1903435, -1050970, -1333058, 1237275, -3318210, -1430225, -451100, 1312455, 3306115,
    -1962642, -1279661, 1917081, -2546312, -1374803, 1500165, 777191, 2235880, 3406031, -542412,
    -2831860, -1671176, -1846953, -2584293, -3724270, 594136, -3776993, -2013608, 2432395, 2454455,
    -164721, 1957272, 3369112, 185531, -1207385, -3183426, 162844, 1616392, 3014001, 810149,
    1652634, -3694233, -1799107, -3038916, 3523897, 3866901, 269760, 2213111, -975884, 1717735,
    472078, -426683, 1723600, -1803090, 1910376, -1667432, -1104333, -260646, -3833893, -2939036,
    -2235985, -420899, -2286327, 183443, -976891, 1612842, -3545687, -554416, 3919660, -48306,
    -1362209, 3937738, 1400424, -846154, 1976782,
];
#[derive(Clone, PartialEq, Eq, Zeroize, ZeroizeOnDrop)]
pub(super) struct Poly {
    pub(super) coeffs: [i32; N],
}
impl Default for Poly {
    fn default() -> Self {
        Self { coeffs: [0; N] }
    }
}
#[derive(Clone, PartialEq, Eq, Zeroize, ZeroizeOnDrop)]
pub(super) struct PolyVec<const M: usize> {
    pub(super) polys: [Poly; M],
}
impl<const M: usize> Default for PolyVec<M> {
    fn default() -> Self {
        Self {
            polys: array::from_fn(|_| Poly::default()),
        }
    }
}
#[inline]
fn montgomery_reduce(a: i64) -> i32 {
    let t = (a as u64).wrapping_mul(QINV) as u32 as i32;
    ((a - i64::from(t) * i64::from(Q)) >> 32) as i32
}
#[inline]
fn reduce32(a: i32) -> i32 {
    let t = (a + (1 << 22)) >> 23;
    a - t * Q
}
#[inline]
fn caddq(a: i32) -> i32 {
    a + ((a >> 31) & Q)
}
fn ntt(poly: &mut Poly) {
    let mut k = 0;
    let mut len = 128;
    while len > 0 {
        let mut start = 0;
        while start < N {
            k += 1;
            let zeta = ZETAS[k];
            for j in start..start + len {
                let t = montgomery_reduce(i64::from(zeta) * i64::from(poly.coeffs[j + len]));
                poly.coeffs[j + len] = poly.coeffs[j] - t;
                poly.coeffs[j] += t;
            }
            start += 2 * len;
        }
        len >>= 1;
    }
}
fn invntt_tomont(poly: &mut Poly) {
    const F: i32 = 41_978;
    let mut k = N;
    let mut len = 1;
    while len < N {
        let mut start = 0;
        while start < N {
            k -= 1;
            let zeta = -ZETAS[k];
            for j in start..start + len {
                let t = poly.coeffs[j];
                poly.coeffs[j] = t + poly.coeffs[j + len];
                poly.coeffs[j + len] = t - poly.coeffs[j + len];
                poly.coeffs[j + len] =
                    montgomery_reduce(i64::from(zeta) * i64::from(poly.coeffs[j + len]));
            }
            start += 2 * len;
        }
        len <<= 1;
    }
    for coeff in &mut poly.coeffs {
        *coeff = montgomery_reduce(i64::from(F) * i64::from(*coeff));
    }
}
fn poly_pointwise_montgomery(a: &Poly, b: &Poly) -> Poly {
    Poly {
        coeffs: array::from_fn(|i| {
            montgomery_reduce(i64::from(a.coeffs[i]) * i64::from(b.coeffs[i]))
        }),
    }
}
fn poly_add(a: &Poly, b: &Poly) -> Poly {
    Poly {
        coeffs: array::from_fn(|i| a.coeffs[i] + b.coeffs[i]),
    }
}
fn poly_sub(a: &Poly, b: &Poly) -> Poly {
    Poly {
        coeffs: array::from_fn(|i| a.coeffs[i] - b.coeffs[i]),
    }
}
fn shake128_reader(seed: &[u8; 32], nonce: u16) -> impl XofReader {
    let mut state = Shake128::default();
    state.update(seed);
    state.update(&nonce.to_le_bytes());
    state.finalize_xof()
}
fn shake256_reader(seed: &[u8], nonce: u16) -> impl XofReader {
    let mut state = Shake256::default();
    state.update(seed);
    state.update(&nonce.to_le_bytes());
    state.finalize_xof()
}
fn poly_uniform(seed: &[u8; 32], nonce: u16) -> Poly {
    let mut reader = shake128_reader(seed, nonce);
    let mut result = Poly::default();
    let mut encoded = [0_u8; 3];
    let mut count = 0;
    while count < N {
        reader.read(&mut encoded);
        let candidate =
            (u32::from(encoded[0]) | (u32::from(encoded[1]) << 8) | (u32::from(encoded[2]) << 16))
                & 0x7f_ffff;
        if candidate < Q as u32 {
            result.coeffs[count] = candidate as i32;
            count += 1;
        }
    }
    result
}
fn poly_uniform_eta(eta: i32, seed: &[u8; 64], nonce: u16) -> Poly {
    debug_assert!(eta == 2 || eta == 4);
    let mut reader = shake256_reader(seed, nonce);
    let mut result = Poly::default();
    let mut count = 0;
    while count < N {
        let mut encoded = [0_u8; 1];
        reader.read(&mut encoded);
        for mut candidate in [encoded[0] & 0x0f, encoded[0] >> 4] {
            let accepted = if eta == 2 {
                if candidate >= 15 {
                    false
                } else {
                    candidate %= 5;
                    true
                }
            } else {
                candidate < 9
            };
            if accepted {
                result.coeffs[count] = eta - i32::from(candidate);
                count += 1;
                if count == N {
                    break;
                }
            }
        }
    }
    result
}
fn poly_uniform_gamma1(gamma1: i32, seed: &[u8; 64], nonce: u16) -> Poly {
    let bits = gamma1_bits(gamma1);
    let mut encoded = vec![0_u8; N * bits / 8];
    shake256_reader(seed, nonce).read(&mut encoded);
    unpack_poly(&encoded, bits, |value| gamma1 - value as i32).0
}
pub(super) fn matrix_expand<const K: usize, const L: usize>(rho: &[u8; 32]) -> [PolyVec<L>; K] {
    array::from_fn(|row| PolyVec {
        polys: array::from_fn(|column| poly_uniform(rho, ((row as u16) << 8) | column as u16)),
    })
}
pub(super) fn vec_uniform_eta<const M: usize>(eta: i32, seed: &[u8; 64], nonce: u16) -> PolyVec<M> {
    PolyVec {
        polys: array::from_fn(|i| poly_uniform_eta(eta, seed, nonce.wrapping_add(i as u16))),
    }
}
pub(super) fn vec_uniform_gamma1<const L: usize>(
    gamma1: i32,
    seed: &[u8; 64],
    nonce: u16,
) -> PolyVec<L> {
    PolyVec {
        polys: array::from_fn(|i| {
            poly_uniform_gamma1(
                gamma1,
                seed,
                (L as u16).wrapping_mul(nonce).wrapping_add(i as u16),
            )
        }),
    }
}
pub(super) fn vec_ntt<const M: usize>(value: &mut PolyVec<M>) {
    for poly in &mut value.polys {
        ntt(poly);
    }
}
pub(super) fn vec_invntt_tomont<const M: usize>(value: &mut PolyVec<M>) {
    for poly in &mut value.polys {
        invntt_tomont(poly);
    }
}
pub(super) fn vec_reduce<const M: usize>(value: &mut PolyVec<M>) {
    for poly in &mut value.polys {
        for coeff in &mut poly.coeffs {
            *coeff = reduce32(*coeff);
        }
    }
}
pub(super) fn vec_caddq<const M: usize>(value: &mut PolyVec<M>) {
    for poly in &mut value.polys {
        for coeff in &mut poly.coeffs {
            *coeff = caddq(*coeff);
        }
    }
}
pub(super) fn vec_add<const M: usize>(a: &PolyVec<M>, b: &PolyVec<M>) -> PolyVec<M> {
    PolyVec {
        polys: array::from_fn(|i| poly_add(&a.polys[i], &b.polys[i])),
    }
}
pub(super) fn vec_sub<const M: usize>(a: &PolyVec<M>, b: &PolyVec<M>) -> PolyVec<M> {
    PolyVec {
        polys: array::from_fn(|i| poly_sub(&a.polys[i], &b.polys[i])),
    }
}
pub(super) fn vec_pointwise<const M: usize>(poly: &Poly, value: &PolyVec<M>) -> PolyVec<M> {
    PolyVec {
        polys: array::from_fn(|i| poly_pointwise_montgomery(poly, &value.polys[i])),
    }
}
pub(super) fn matrix_pointwise<const K: usize, const L: usize>(
    matrix: &[PolyVec<L>; K],
    value: &PolyVec<L>,
) -> PolyVec<K> {
    PolyVec {
        polys: array::from_fn(|row| {
            let mut result = poly_pointwise_montgomery(&matrix[row].polys[0], &value.polys[0]);
            for column in 1..L {
                let product =
                    poly_pointwise_montgomery(&matrix[row].polys[column], &value.polys[column]);
                result = poly_add(&result, &product);
            }
            result
        }),
    }
}
pub(super) fn vec_chknorm<const M: usize>(value: &PolyVec<M>, bound: i32) -> bool {
    if bound > (Q - 1) / 8 || bound <= 0 {
        return true;
    }
    value.polys.iter().any(|poly| {
        poly.coeffs
            .iter()
            .any(|coeff| coeff.unsigned_abs() >= bound as u32)
    })
}
pub(super) fn vec_power2round<const M: usize>(value: &PolyVec<M>) -> (PolyVec<M>, PolyVec<M>) {
    let mut high = PolyVec::default();
    let mut low = PolyVec::default();
    for i in 0..M {
        for j in 0..N {
            let high_coeff = (value.polys[i].coeffs[j] + (1 << (D - 1)) - 1) >> D;
            high.polys[i].coeffs[j] = high_coeff;
            low.polys[i].coeffs[j] = value.polys[i].coeffs[j] - (high_coeff << D);
        }
    }
    (high, low)
}
fn decompose(gamma2: i32, value: i32) -> (i32, i32) {
    let mut high = (value + 127) >> 7;
    if gamma2 == (Q - 1) / 32 {
        high = (high * 1025 + (1 << 21)) >> 22;
        high &= 15;
    } else {
        debug_assert_eq!(gamma2, (Q - 1) / 88);
        high = (high * 11275 + (1 << 23)) >> 24;
        high ^= ((43 - high) >> 31) & high;
    }
    let mut low = value - high * 2 * gamma2;
    low -= (((Q - 1) / 2 - low) >> 31) & Q;
    (high, low)
}
pub(super) fn vec_decompose<const M: usize>(
    gamma2: i32,
    value: &PolyVec<M>,
) -> (PolyVec<M>, PolyVec<M>) {
    let mut high = PolyVec::default();
    let mut low = PolyVec::default();
    for i in 0..M {
        for j in 0..N {
            (high.polys[i].coeffs[j], low.polys[i].coeffs[j]) =
                decompose(gamma2, value.polys[i].coeffs[j]);
        }
    }
    (high, low)
}
pub(super) fn vec_make_hint<const M: usize>(
    gamma2: i32,
    low: &PolyVec<M>,
    high: &PolyVec<M>,
) -> (PolyVec<M>, usize) {
    let mut hint = PolyVec::default();
    let mut count = 0;
    for i in 0..M {
        for j in 0..N {
            let low_coeff = low.polys[i].coeffs[j];
            let high_coeff = high.polys[i].coeffs[j];
            let bit = i32::from(
                low_coeff > gamma2
                    || low_coeff < -gamma2
                    || (low_coeff == -gamma2 && high_coeff != 0),
            );
            hint.polys[i].coeffs[j] = bit;
            count += bit as usize;
        }
    }
    (hint, count)
}
pub(super) fn poly_challenge(tau: usize, seed: &[u8]) -> Poly {
    let mut state = Shake256::default();
    state.update(seed);
    let mut reader = state.finalize_xof();
    let mut sign_bytes = [0_u8; 8];
    reader.read(&mut sign_bytes);
    let mut signs = u64::from_le_bytes(sign_bytes);
    let mut result = Poly::default();
    for i in N - tau..N {
        let selected = loop {
            let mut encoded = [0_u8; 1];
            reader.read(&mut encoded);
            if usize::from(encoded[0]) <= i {
                break usize::from(encoded[0]);
            }
        };
        result.coeffs[i] = result.coeffs[selected];
        result.coeffs[selected] = 1 - 2 * (signs & 1) as i32;
        signs >>= 1;
    }
    result
}
pub(super) fn poly_ntt(value: &mut Poly) {
    ntt(value);
}
fn eta_bits(eta: i32) -> usize {
    if eta == 2 { 3 } else { 4 }
}
fn gamma1_bits(gamma1: i32) -> usize {
    if gamma1 == 1 << 17 { 18 } else { 20 }
}
pub(super) fn eta_packed_bytes(eta: i32) -> usize {
    N * eta_bits(eta) / 8
}
pub(super) fn z_packed_bytes(gamma1: i32) -> usize {
    N * gamma1_bits(gamma1) / 8
}
pub(super) fn w1_packed_bytes(gamma2: i32) -> usize {
    let bits = if gamma2 == (Q - 1) / 88 { 6 } else { 4 };
    N * bits / 8
}
fn pack_poly(output: &mut [u8], poly: &Poly, bits: usize, encode: impl Fn(i32) -> u32) {
    debug_assert_eq!(output.len(), N * bits / 8);
    output.fill(0);
    let mut accumulator = 0_u64;
    let mut accumulator_bits = 0;
    let mut output_index = 0;
    let mask = (1_u64 << bits) - 1;
    for coeff in poly.coeffs {
        accumulator |= (u64::from(encode(coeff)) & mask) << accumulator_bits;
        accumulator_bits += bits;
        while accumulator_bits >= 8 {
            output[output_index] = accumulator as u8;
            output_index += 1;
            accumulator >>= 8;
            accumulator_bits -= 8;
        }
    }
    debug_assert_eq!(output_index, output.len());
    debug_assert_eq!(accumulator_bits, 0);
}
fn unpack_poly(input: &[u8], bits: usize, decode: impl Fn(u32) -> i32) -> (Poly, u32) {
    debug_assert_eq!(input.len(), N * bits / 8);
    let mask = (1_u64 << bits) - 1;
    let mut accumulator = 0_u64;
    let mut accumulator_bits = 0;
    let mut input_index = 0;
    let mut maximum = 0;
    let mut poly = Poly::default();
    for coeff in &mut poly.coeffs {
        while accumulator_bits < bits {
            accumulator |= u64::from(input[input_index]) << accumulator_bits;
            input_index += 1;
            accumulator_bits += 8;
        }
        let encoded = (accumulator & mask) as u32;
        maximum = maximum.max(encoded);
        *coeff = decode(encoded);
        accumulator >>= bits;
        accumulator_bits -= bits;
    }
    (poly, maximum)
}
fn pack_t1(output: &mut [u8], value: &Poly) {
    pack_poly(output, value, 10, |coeff| coeff as u32);
}
fn pack_t0(output: &mut [u8], value: &Poly) {
    pack_poly(output, value, 13, |coeff| ((1 << (D - 1)) - coeff) as u32);
}
fn unpack_t0(input: &[u8]) -> Poly {
    unpack_poly(input, 13, |value| (1 << (D - 1)) - value as i32).0
}
fn pack_eta(output: &mut [u8], eta: i32, value: &Poly) {
    pack_poly(output, value, eta_bits(eta), |coeff| (eta - coeff) as u32);
}
fn unpack_eta(input: &[u8], eta: i32) -> (Poly, bool) {
    let (poly, maximum) = unpack_poly(input, eta_bits(eta), |value| eta - value as i32);
    (poly, maximum <= (2 * eta) as u32)
}
fn pack_z(output: &mut [u8], gamma1: i32, value: &Poly) {
    pack_poly(output, value, gamma1_bits(gamma1), |coeff| {
        (gamma1 - coeff) as u32
    });
}
pub(super) fn pack_w1<const K: usize>(output: &mut [u8], gamma2: i32, value: &PolyVec<K>) {
    let packed_bytes = w1_packed_bytes(gamma2);
    let bits = if gamma2 == (Q - 1) / 88 { 6 } else { 4 };
    debug_assert_eq!(output.len(), K * packed_bytes);
    for i in 0..K {
        pack_poly(
            &mut output[i * packed_bytes..(i + 1) * packed_bytes],
            &value.polys[i],
            bits,
            |coeff| coeff as u32,
        );
    }
}
pub(super) fn pack_pk<const K: usize>(output: &mut [u8], rho: &[u8; 32], t1: &PolyVec<K>) {
    const T1_PACKED_BYTES: usize = N * 10 / 8;
    debug_assert_eq!(output.len(), 32 + K * T1_PACKED_BYTES);
    output[..32].copy_from_slice(rho);
    for i in 0..K {
        let offset = 32 + i * T1_PACKED_BYTES;
        pack_t1(&mut output[offset..offset + T1_PACKED_BYTES], &t1.polys[i]);
    }
}
#[allow(clippy::too_many_arguments)]
pub(super) fn pack_sk<const K: usize, const L: usize>(
    output: &mut [u8],
    eta: i32,
    rho: &[u8; 32],
    tr: &[u8; 64],
    key: &[u8; 32],
    t0: &PolyVec<K>,
    s1: &PolyVec<L>,
    s2: &PolyVec<K>,
) {
    const T0_PACKED_BYTES: usize = N * 13 / 8;
    let eta_bytes = eta_packed_bytes(eta);
    let expected = 2 * 32 + 64 + (L + K) * eta_bytes + K * T0_PACKED_BYTES;
    debug_assert_eq!(output.len(), expected);
    output[..32].copy_from_slice(rho);
    output[32..64].copy_from_slice(key);
    output[64..128].copy_from_slice(tr);
    let mut offset = 128;
    for poly in &s1.polys {
        pack_eta(&mut output[offset..offset + eta_bytes], eta, poly);
        offset += eta_bytes;
    }
    for poly in &s2.polys {
        pack_eta(&mut output[offset..offset + eta_bytes], eta, poly);
        offset += eta_bytes;
    }
    for poly in &t0.polys {
        pack_t0(&mut output[offset..offset + T0_PACKED_BYTES], poly);
        offset += T0_PACKED_BYTES;
    }
    debug_assert_eq!(offset, output.len());
}
#[allow(clippy::too_many_arguments)]
pub(super) fn unpack_sk<const K: usize, const L: usize>(
    input: &[u8],
    eta: i32,
    rho: &mut [u8; 32],
    tr: &mut [u8; 64],
    key: &mut [u8; 32],
    t0: &mut PolyVec<K>,
    s1: &mut PolyVec<L>,
    s2: &mut PolyVec<K>,
) -> bool {
    const T0_PACKED_BYTES: usize = N * 13 / 8;
    let eta_bytes = eta_packed_bytes(eta);
    let expected = 2 * 32 + 64 + (L + K) * eta_bytes + K * T0_PACKED_BYTES;
    debug_assert_eq!(input.len(), expected);
    rho.copy_from_slice(&input[..32]);
    key.copy_from_slice(&input[32..64]);
    tr.copy_from_slice(&input[64..128]);
    let mut offset = 128;
    let mut canonical = true;
    for poly in &mut s1.polys {
        let (decoded, valid) = unpack_eta(&input[offset..offset + eta_bytes], eta);
        *poly = decoded;
        canonical &= valid;
        offset += eta_bytes;
    }
    for poly in &mut s2.polys {
        let (decoded, valid) = unpack_eta(&input[offset..offset + eta_bytes], eta);
        *poly = decoded;
        canonical &= valid;
        offset += eta_bytes;
    }
    for poly in &mut t0.polys {
        *poly = unpack_t0(&input[offset..offset + T0_PACKED_BYTES]);
        offset += T0_PACKED_BYTES;
    }
    debug_assert_eq!(offset, input.len());
    canonical
}
pub(super) fn pack_sig<const K: usize, const L: usize>(
    output: &mut [u8],
    c_tilde: &[u8],
    gamma1: i32,
    omega: usize,
    z: &PolyVec<L>,
    hint: &PolyVec<K>,
) {
    let z_bytes = z_packed_bytes(gamma1);
    debug_assert_eq!(output.len(), c_tilde.len() + L * z_bytes + omega + K);
    output[..c_tilde.len()].copy_from_slice(c_tilde);
    let mut offset = c_tilde.len();
    for poly in &z.polys {
        pack_z(&mut output[offset..offset + z_bytes], gamma1, poly);
        offset += z_bytes;
    }
    output[offset..].fill(0);
    let mut hint_count = 0;
    for i in 0..K {
        for j in 0..N {
            if hint.polys[i].coeffs[j] != 0 {
                debug_assert!(hint_count < omega);
                output[offset + hint_count] = j as u8;
                hint_count += 1;
            }
        }
        output[offset + omega + i] = hint_count as u8;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bit_packing_is_little_endian_and_roundtrips() {
        let poly = Poly {
            coeffs: array::from_fn(|i| (i % 1024) as i32),
        };
        let mut encoded = [0_u8; N * 10 / 8];
        pack_t1(&mut encoded, &poly);
        assert_eq!(&encoded[..5], &[0, 4, 32, 192, 0]);
        let (decoded, maximum) = unpack_poly(&encoded, 10, |value| value as i32);
        assert!(decoded == poly);
        assert_eq!(maximum, 255);
    }
    #[test]
    fn eta_decoding_rejects_unused_encodings() {
        let encoded = [0xff_u8; N * 3 / 8];
        let (_, canonical) = unpack_eta(&encoded, 2);
        assert!(!canonical);
    }
    #[test]
    fn samplers_obey_parameter_bounds() {
        let seed = [0x5a_u8; 64];
        for eta in [2, 4] {
            let poly = poly_uniform_eta(eta, &seed, 7);
            assert!(
                poly.coeffs
                    .iter()
                    .all(|coefficient| (-eta..=eta).contains(coefficient))
            );
        }
        for gamma1 in [1 << 17, 1 << 19] {
            let poly = poly_uniform_gamma1(gamma1, &seed, 11);
            assert!(
                poly.coeffs
                    .iter()
                    .all(|coefficient| (-(gamma1 - 1)..=gamma1).contains(coefficient))
            );
        }
    }
}
