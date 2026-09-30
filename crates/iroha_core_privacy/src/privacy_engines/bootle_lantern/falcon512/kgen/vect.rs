#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
use super::fxp::{FXC, FXR, GM_TAB};
// ========================================================================
// Fixed-point vector operations
// ========================================================================
// In FFT representation, we only keep half of the coefficients, because
// all our vectors are real in non-FFT; thus, the FFT representation is
// redundant. For 0 <= k < n/2, f[k] contains the real part, and f[k + n/2]
// the imaginary part of the complex value.
// Convert a (real) vector to its FFT representation.
pub(crate) fn vect_FFT(logn: u32, f: &mut [FXR]) {
    let hn = 1usize << (logn - 1);
    let mut t = hn;
    for lm in 1..logn {
        let m = 1usize << lm;
        let ht = t >> 1;
        let mut j0 = 0;
        for i in 0..(m >> 1) {
            let s = GM_TAB[m + i];
            for j in j0..(j0 + ht) {
                let x = FXC {
                    re: f[j],
                    im: f[j + hn],
                };
                let y = FXC {
                    re: f[j + ht],
                    im: f[j + ht + hn],
                };
                let z = s * y;
                let w1 = x + z;
                f[j] = w1.re;
                f[j + hn] = w1.im;
                let w2 = x - z;
                f[j + ht] = w2.re;
                f[j + ht + hn] = w2.im;
            }
            j0 += t;
        }
        t = ht;
    }
}
// Convert back from FFT representation into a real vector.
//
// Note: in the final outer iteration:
//
//  - f[j] and f[j+hn] are set to the half of a complex number. Fixed point
//    values are held in integers in the [-2^63..+2^63-1] range (in signed
//    interpretation) and halving is done by adding 1 and then an
//    arithmetic right shift, thus with an output necessarily in the
//    [-2^62..+2^62-1] range (addition of 1 to 2^63-1 "wraps around" in that
//    +2^63 is interpreted as -2^63 instead).
//
//  - f[j+ht] and f[j+ht+hn] are set to the product of the half of a
//    complex number, and the complex sqrt(2)-i*sqrt(2). Value sqrt(2)
//    is represented by the fixed point 3037000500. Following the steps
//    in fxc_mul():
//       r1, i1:           -2^62 .. +2^62-1
//       r2:               +3037000500
//       i2:               -3037000500
//       t0:               -3260954456358912000 .. +3260954456358911999
//       t1:               -3260954456358912000 .. +3260954456358912000
//       fxr_add(r1, i1):  -2^63 .. +2^63-2
//       fxr_add(r2, i2):  always zero
//       t2:               always zero
//       fxr_sub(t0, t1):  -6521908912717824000 .. +6521908912717823999
//       fxr_add(t0, t1):  -6521908912717824000 .. +6521908912717823999
//    Thus, the obtained output values must be in the
//    [-6521908912717824000..+6521908912717824000] range.
//
// If the output of vect_iFFT() is then rounded to integers, then the
// maximum range for any output value after rounding is
// [-1518500250..+1518500250], regardless of the contents of the source
// vector. In particular, this is a smaller range than signed 32-bit integers
// in general, and it avoids the troublesome values such as -2^31.
pub(crate) fn vect_iFFT(logn: u32, f: &mut [FXR]) {
    let hn = 1usize << (logn - 1);
    let mut ht = 1;
    for lm in (1..logn).rev() {
        let m = 1usize << lm;
        let t = ht << 1;
        let mut j0 = 0;
        for i in 0..(m >> 1) {
            let s = GM_TAB[m + i].conj();
            for j in j0..(j0 + ht) {
                let x = FXC {
                    re: f[j],
                    im: f[j + hn],
                };
                let y = FXC {
                    re: f[j + ht],
                    im: f[j + ht + hn],
                };
                let z1 = (x + y).half();
                f[j] = z1.re;
                f[j + hn] = z1.im;
                let z2 = s * (x - y).half();
                f[j + ht] = z2.re;
                f[j + ht + hn] = z2.im;
            }
            j0 += t;
        }
        ht = t;
    }
}
// Set vector d to the value of polynomial f.
pub(crate) fn vect_to_fxr(logn: u32, d: &mut [FXR], f: &[i8]) {
    for i in 0..(1usize << logn) {
        d[i] = FXR::from_i32(f[i] as i32);
    }
}
// Add vector b to vector a. This works in both real and FFT representations.
pub(crate) fn vect_add(logn: u32, a: &mut [FXR], b: &[FXR]) {
    for i in 0..(1usize << logn) {
        a[i] += b[i];
    }
}
// Multiply vector a by constant c. This works in both real and FFT
// representations.
pub(crate) fn vect_mul_realconst(logn: u32, a: &mut [FXR], c: FXR) {
    for i in 0..(1usize << logn) {
        a[i] *= c;
    }
}
// Multiply vector a by 2^e. Exponent e should be in the [0,30] range.
pub(crate) fn vect_mul2e(logn: u32, a: &mut [FXR], e: u32) {
    for i in 0..(1usize << logn) {
        a[i].set_mul2e(e);
    }
}
// Multiply vector a by vector b. The vectors must be in FFT representation,
// and the result is in FFT representation.
pub(crate) fn vect_mul_fft(logn: u32, a: &mut [FXR], b: &[FXR]) {
    let hn = 1usize << (logn - 1);
    for i in 0..hn {
        let x = FXC {
            re: a[i],
            im: a[i + hn],
        };
        let y = FXC {
            re: b[i],
            im: b[i + hn],
        };
        let z = x * y;
        a[i] = z.re;
        a[i + hn] = z.im;
    }
}
// Convert a vector into its Hermitian adjoint (in FFT representation).
pub(crate) fn vect_adj_fft(logn: u32, a: &mut [FXR]) {
    for i in (1usize << (logn - 1))..(1usize << logn) {
        a[i].set_neg();
    }
}
// Multiply vector a by the self-adjoint vector b. Both vectors are in FFT
// representation. Since the FFT representation of a self-adjoint vector
// contains only real numbers, the second half of b contains only zeros and
// thus is not accessed (the slice may be half length).
pub(crate) fn vect_mul_selfadj_fft(logn: u32, a: &mut [FXR], b: &[FXR]) {
    let hn = 1usize << (logn - 1);
    for i in 0..hn {
        let c = b[i];
        a[i] *= c;
        a[i + hn] *= c;
    }
}
// Divide vector a by the self-adjoint vector b. Both vectors are in FFT
// representation. Since the FFT representation of a self-adjoint vector
// contains only real numbers, the second half of b contains only zeros and
// thus is not accessed (the slice may be half length).
pub(crate) fn vect_div_selfadj_fft(logn: u32, a: &mut [FXR], b: &[FXR]) {
    let hn = 1usize << (logn - 1);
    for i in 0..hn {
        // We do not compute 1/c separately because that would deviate from
        // the specification, and lose too much precision; we need to
        // perform the two divisions.
        let c = b[i];
        a[i] /= c;
        a[i + hn] /= c;
    }
}
// Compute d = a*adj(a) + b*adj(b). Polynomials are in FFT representation.
// Since d is self-adjoint, it is half-size (only the low half is set, the
// high half is implicitly zero).
pub(crate) fn vect_norm_fft(logn: u32, d: &mut [FXR], a: &[FXR], b: &[FXR]) {
    let hn = 1usize << (logn - 1);
    for i in 0..hn {
        d[i] = a[i].sqr() + a[i + hn].sqr() + b[i].sqr() + b[i + hn].sqr();
    }
}
// Compute d = (2^e)/(a*adj(a) + b*adj(b)). Polynomials are in FFT
// representation. Since d is self-adjoint, it is half-size (only the
// low half is set, the high half is implicitly zero).
pub(crate) fn vect_invnorm_fft(logn: u32, d: &mut [FXR], a: &[FXR], b: &[FXR], e: u32) {
    let hn = 1usize << (logn - 1);
    let r = FXR::from_i32(1i32 << e);
    for i in 0..hn {
        let z1 = a[i].sqr() + a[i + hn].sqr();
        let z2 = b[i].sqr() + b[i + hn].sqr();
        d[i] = r / (z1 + z2);
    }
}
// ========================================================================
