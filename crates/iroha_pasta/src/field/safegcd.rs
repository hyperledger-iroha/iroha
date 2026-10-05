//! Variable-time modular inversion by the Bernstein-Yang "safegcd" method.
//!
//! This is a port of the variable-time inverse in libsecp256k1
//! (`modinv64_impl.h`, "divsteps" in batches of 62 with signed 62-bit limbs),
//! following Bernstein and Yang, "Fast constant-time gcd computation and
//! modular inversion" (IACR ePrint 2019/266), with Pieter Wuille's
//! variable-time optimisations.
//!
//! **Variable time.** Running time depends on the input value. Use it only on
//! public data (verifier MSMs, generator derivation, folding of public
//! generators). Secret-derived values must use the constant-time Fermat
//! inversion in [`crate::field`].
//!
//! The algorithm is exact integer arithmetic, so its result equals the Fermat
//! inverse bit for bit.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::many_single_char_names,
    clippy::similar_names
)]

/// Mask of the low 62 bits.
const M62: u64 = u64::MAX >> 2;

/// A signed integer as five limbs of 62 bits: `sum v[i] * 2^(62 i)`.
///
/// Limbs 0..=3 are normally in `[0, 2^62)`; limb 4 carries the sign.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Signed62(pub [i64; 5]);

impl Signed62 {
    /// Converts a non-negative 256-bit little-endian integer.
    pub const fn from_limbs(a: &[u64; 4]) -> Self {
        Self([
            (a[0] & M62) as i64,
            (((a[0] >> 62) | (a[1] << 2)) & M62) as i64,
            (((a[1] >> 60) | (a[2] << 4)) & M62) as i64,
            (((a[2] >> 58) | (a[3] << 6)) & M62) as i64,
            (a[3] >> 56) as i64,
        ])
    }

    /// Converts back to a 256-bit integer. Requires a normalised value in
    /// `[0, 2^256)`.
    pub const fn to_limbs(self) -> [u64; 4] {
        let v = self.0;
        [
            (v[0] as u64) | ((v[1] as u64) << 62),
            ((v[1] as u64) >> 2) | ((v[2] as u64) << 60),
            ((v[2] as u64) >> 4) | ((v[3] as u64) << 58),
            ((v[3] as u64) >> 6) | ((v[4] as u64) << 56),
        ]
    }
}

/// Modulus data for the inversion.
#[derive(Clone, Copy, Debug)]
pub struct ModInfo {
    /// The odd modulus in signed-62 form.
    pub modulus: Signed62,
    /// `modulus^-1 mod 2^62`.
    pub modulus_inv62: u64,
}

impl ModInfo {
    /// Builds the inversion data for an odd 256-bit modulus.
    pub const fn new(m: &[u64; 4]) -> Self {
        // Newton iteration: each step doubles the number of correct low bits.
        let m0 = m[0];
        let mut inv = m0; // correct to 3 bits for odd m0
        let mut i = 0;
        while i < 5 {
            inv = inv.wrapping_mul(2u64.wrapping_sub(m0.wrapping_mul(inv)));
            i += 1;
        }
        Self {
            modulus: Signed62::from_limbs(m),
            modulus_inv62: inv & M62,
        }
    }
}

/// The 2x2 transition matrix of 62 divsteps, scaled by `2^62`.
#[derive(Clone, Copy, Debug)]
struct Trans2x2 {
    u: i64,
    v: i64,
    q: i64,
    r: i64,
}

/// Performs 62 divsteps on the low words of `f` and `g` (variable time).
///
/// Returns the new `eta` (`-delta`) and the transition matrix.
fn divsteps_62_var(mut eta: i64, f0: u64, g0: u64) -> (i64, Trans2x2) {
    let (mut u, mut v, mut q, mut r) = (1u64, 0u64, 0u64, 1u64);
    let (mut f, mut g) = (f0, g0);
    let mut i: u32 = 62;
    loop {
        // A sentinel bit bounds the zero count by the remaining steps.
        let zeros = (g | (u64::MAX << i)).trailing_zeros();
        g >>= zeros;
        u <<= zeros;
        v <<= zeros;
        eta -= i64::from(zeros);
        i -= zeros;
        if i == 0 {
            break;
        }
        debug_assert!(f & 1 == 1 && g & 1 == 1);
        let w: u64;
        let m: u64;
        if eta < 0 {
            // Negate eta and replace (f, g) with (g, -f).
            eta = -eta;
            let tmp = f;
            f = g;
            g = tmp.wrapping_neg();
            let tmp = u;
            u = q;
            q = tmp.wrapping_neg();
            let tmp = v;
            v = r;
            r = tmp.wrapping_neg();
            // Cancel up to 6 bits of g, bounded by the remaining steps and eta + 1.
            let limit = core::cmp::min(eta + 1, i64::from(i)) as u32;
            m = (u64::MAX >> (64 - limit)) & 63;
            w = f
                .wrapping_mul(g)
                .wrapping_mul(f.wrapping_mul(f).wrapping_sub(2))
                & m;
        } else {
            // Cancel up to 4 bits of g.
            let limit = core::cmp::min(eta + 1, i64::from(i)) as u32;
            m = (u64::MAX >> (64 - limit)) & 15;
            let w0 = f.wrapping_add((f.wrapping_add(1) & 4) << 1);
            w = w0.wrapping_neg().wrapping_mul(g) & m;
        }
        g = g.wrapping_add(f.wrapping_mul(w));
        q = q.wrapping_add(u.wrapping_mul(w));
        r = r.wrapping_add(v.wrapping_mul(w));
        debug_assert!(g & m == 0);
    }
    (
        eta,
        Trans2x2 {
            u: u as i64,
            v: v as i64,
            q: q as i64,
            r: r as i64,
        },
    )
}

/// Computes `(t * [d, e] + modulus * [md, me]) / 2^62`, keeping `d, e` in range.
fn update_de_62(d: &mut Signed62, e: &mut Signed62, t: &Trans2x2, info: &ModInfo) {
    let dv = d.0;
    let ev = e.0;
    let (u, v, q, r) = (t.u, t.v, t.q, t.r);
    let sd = dv[4] >> 63;
    let se = ev[4] >> 63;
    let mut md = (u & sd).wrapping_add(v & se);
    let mut me = (q & sd).wrapping_add(r & se);
    let mut cd = i128::from(u) * i128::from(dv[0]) + i128::from(v) * i128::from(ev[0]);
    let mut ce = i128::from(q) * i128::from(dv[0]) + i128::from(r) * i128::from(ev[0]);
    // Choose md, me so that the low 62 bits cancel.
    md = md.wrapping_sub(
        (info
            .modulus_inv62
            .wrapping_mul(cd as u64)
            .wrapping_add(md as u64)
            & M62) as i64,
    );
    me = me.wrapping_sub(
        (info
            .modulus_inv62
            .wrapping_mul(ce as u64)
            .wrapping_add(me as u64)
            & M62) as i64,
    );
    let mv = info.modulus.0;
    cd += i128::from(mv[0]) * i128::from(md);
    ce += i128::from(mv[0]) * i128::from(me);
    debug_assert!((cd as u64) & M62 == 0 && (ce as u64) & M62 == 0);
    cd >>= 62;
    ce >>= 62;
    let mut out_d = [0i64; 5];
    let mut out_e = [0i64; 5];
    for i in 1..5 {
        cd += i128::from(u) * i128::from(dv[i]) + i128::from(v) * i128::from(ev[i]);
        ce += i128::from(q) * i128::from(dv[i]) + i128::from(r) * i128::from(ev[i]);
        if mv[i] != 0 {
            cd += i128::from(mv[i]) * i128::from(md);
            ce += i128::from(mv[i]) * i128::from(me);
        }
        out_d[i - 1] = ((cd as u64) & M62) as i64;
        out_e[i - 1] = ((ce as u64) & M62) as i64;
        cd >>= 62;
        ce >>= 62;
    }
    out_d[4] = cd as i64;
    out_e[4] = ce as i64;
    d.0 = out_d;
    e.0 = out_e;
}

/// Computes `t * [f, g] / 2^62` over the first `len` limbs.
fn update_fg_62_var(len: usize, f: &mut Signed62, g: &mut Signed62, t: &Trans2x2) {
    let (u, v, q, r) = (t.u, t.v, t.q, t.r);
    let mut cf = i128::from(u) * i128::from(f.0[0]) + i128::from(v) * i128::from(g.0[0]);
    let mut cg = i128::from(q) * i128::from(f.0[0]) + i128::from(r) * i128::from(g.0[0]);
    debug_assert!((cf as u64) & M62 == 0 && (cg as u64) & M62 == 0);
    cf >>= 62;
    cg >>= 62;
    for i in 1..len {
        let fi = f.0[i];
        let gi = g.0[i];
        cf += i128::from(u) * i128::from(fi) + i128::from(v) * i128::from(gi);
        cg += i128::from(q) * i128::from(fi) + i128::from(r) * i128::from(gi);
        f.0[i - 1] = ((cf as u64) & M62) as i64;
        g.0[i - 1] = ((cg as u64) & M62) as i64;
        cf >>= 62;
        cg >>= 62;
    }
    f.0[len - 1] = cf as i64;
    g.0[len - 1] = cg as i64;
}

/// Brings `r` from `(-2m, m)` to `[0, m)`, negating first when `sign < 0`.
fn normalize_62(r: &mut Signed62, sign: i64, info: &ModInfo) {
    let m62 = M62 as i64;
    let mv = info.modulus.0;
    let mut x = r.0;
    let cond_add = x[4] >> 63;
    for i in 0..5 {
        x[i] = x[i].wrapping_add(mv[i] & cond_add);
    }
    let cond_negate = sign >> 63;
    for limb in &mut x {
        *limb = (*limb ^ cond_negate).wrapping_sub(cond_negate);
    }
    for i in 0..4 {
        x[i + 1] += x[i] >> 62;
        x[i] &= m62;
    }
    let cond_add = x[4] >> 63;
    for i in 0..5 {
        x[i] = x[i].wrapping_add(mv[i] & cond_add);
    }
    for i in 0..4 {
        x[i + 1] += x[i] >> 62;
        x[i] &= m62;
    }
    r.0 = x;
}

/// Returns `x^-1 mod m` for `0 < x < m` (variable time). Returns zero for zero.
pub fn invert_var(x: &[u64; 4], info: &ModInfo) -> [u64; 4] {
    let mut d = Signed62([0; 5]);
    let mut e = Signed62([1, 0, 0, 0, 0]);
    let mut f = info.modulus;
    let mut g = Signed62::from_limbs(x);
    let mut len = 5usize;
    let mut eta: i64 = -1;
    loop {
        let (new_eta, t) = divsteps_62_var(eta, f.0[0] as u64, g.0[0] as u64);
        eta = new_eta;
        update_de_62(&mut d, &mut e, &t, info);
        update_fg_62_var(len, &mut f, &mut g, &t);
        if g.0[0] == 0 && g.0[1..len].iter().all(|&limb| limb == 0) {
            break;
        }
        let fn_ = f.0[len - 1];
        let gn = g.0[len - 1];
        let mut cond = ((len as i64) - 2) >> 63;
        cond |= fn_ ^ (fn_ >> 63);
        cond |= gn ^ (gn >> 63);
        if cond == 0 {
            f.0[len - 2] |= ((fn_ as u64) << 62) as i64;
            g.0[len - 2] |= ((gn as u64) << 62) as i64;
            len -= 1;
        }
    }
    normalize_62(&mut d, f.0[len - 1], info);
    d.to_limbs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signed62_round_trip() {
        for a in [
            [0u64; 4],
            [1, 2, 3, 4],
            [u64::MAX, u64::MAX, u64::MAX, u64::MAX >> 1],
            crate::field::fp::MODULUS.m,
        ] {
            assert_eq!(Signed62::from_limbs(&a).to_limbs(), a);
        }
    }

    #[test]
    fn modinfo_inverse_is_correct() {
        for m in [crate::field::fp::MODULUS.m, crate::field::fq::MODULUS.m] {
            let info = ModInfo::new(&m);
            assert_eq!(m[0].wrapping_mul(info.modulus_inv62) & M62, 1);
        }
    }

    #[test]
    fn divsteps_matrix_scales_inputs() {
        // u * f0 + v * g0 == f << 62 (mod 2^64) holds for the returned matrix:
        // checked indirectly through update_fg producing zero low bits.
        let (eta, t) = divsteps_62_var(-1, 0x992d_30ed_0000_0001, 12345);
        assert!(eta.abs() < 1000);
        let cf = i128::from(t.u) * 0x992d_30ed_0000_0001_i128 + i128::from(t.v) * 12345;
        assert_eq!((cf as u64) & M62, 0);
    }

    #[test]
    fn small_inverses() {
        let m = crate::field::fp::MODULUS.m;
        let info = ModInfo::new(&m);
        // 2^-1 mod p = (p + 1) / 2.
        let inv2 = invert_var(&[2, 0, 0, 0], &info);
        assert_eq!(
            inv2,
            [0xcc96_9876_8000_0001, 0x1123_4c7e_04a6_7c8d, 0, 1 << 61]
        );
        assert_eq!(invert_var(&[1, 0, 0, 0], &info), [1, 0, 0, 0]);
        assert_eq!(invert_var(&[0, 0, 0, 0], &info), [0, 0, 0, 0]);
    }

    #[test]
    fn update_and_normalize_preserve_value_mod_m() {
        let m = crate::field::fq::MODULUS.m;
        let info = ModInfo::new(&m);
        let mut r = Signed62([-1, 0, 0, 0, 0]);
        normalize_62(&mut r, 1, &info);
        let mut expected = m;
        expected[0] -= 1;
        assert_eq!(r.to_limbs(), expected);
        let mut d = Signed62([0; 5]);
        let mut e = Signed62([1, 0, 0, 0, 0]);
        let t = Trans2x2 {
            u: 1 << 62,
            v: 0,
            q: 0,
            r: 1 << 62,
        };
        update_de_62(&mut d, &mut e, &t, &info);
        assert_eq!(d.0, [0; 5]);
        assert_eq!(e.0, [1, 0, 0, 0, 0]);
        let mut f = Signed62::from_limbs(&[5, 0, 0, 0]);
        let mut g = Signed62::from_limbs(&[7, 0, 0, 0]);
        update_fg_62_var(5, &mut f, &mut g, &t);
        assert_eq!(f.to_limbs(), [5, 0, 0, 0]);
        assert_eq!(g.to_limbs(), [7, 0, 0, 0]);
    }
}
