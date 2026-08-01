#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]

use core::cmp::Ordering;
use core::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

use zeroize::DefaultIsZeroes;

// ========================================================================
// Fixed-point operations
// ========================================================================

// The FXR type is a fixed-point number, internally represented over 64 bits.
// The integral part is 32 bits, while the fractional part is also 32 bits.
// For an internal 64-bit signed representation x, the represented real
// number is x/2^32.
#[derive(Clone, Copy, Debug, Eq)]
pub(crate) struct FXR(pub(crate) u64);

impl Default for FXR {
    fn default() -> Self {
        Self(0)
    }
}

impl DefaultIsZeroes for FXR {}

impl FXR {
    pub(crate) const ZERO: Self = Self::from_i32(0);

    #[allow(dead_code)]
    pub(crate) const ONE: Self = Self::from_i32(1);

    // Convert a signed 32-bit integer to an FXR value. Since all signed
    // 32-bit integers are representable in the FXR format, no rounding
    // or truncation is applied.
    #[inline(always)]
    pub(crate) const fn from_i32(j: i32) -> Self {
        Self(((j as u32) as u64) << 32)
    }

    // Get an FXR value from its internal 64-bit representation. This is
    // used mostly for fixed constants.
    #[inline(always)]
    pub(crate) const fn from_u64_scaled32(x: u64) -> Self {
        Self(x)
    }

    // Round this value to the nearest integer (half-integers round up).
    // If the represented value is at least 2^31 - 0.5, then the rounding
    // overflows and -2^31 is obtained as a result.
    #[inline(always)]
    pub(crate) const fn round(self) -> i32 {
        let v = self.0.wrapping_add(0x80000000);
        ((v as i64) >> 32) as i32
    }

    // Addition (internally, wraps around at 64 bits).
    #[inline(always)]
    pub(crate) fn set_add(&mut self, other: Self) {
        self.0 = self.0.wrapping_add(other.0);
    }

    // Subtraction (internally, wraps around at 64 bits).
    #[inline(always)]
    pub(crate) fn set_sub(&mut self, other: Self) {
        self.0 = self.0.wrapping_sub(other.0);
    }

    #[allow(dead_code)]
    // Doubling (internally, wraps around at 64 bits).
    #[inline(always)]
    pub(crate) fn set_double(&mut self) {
        self.0 <<= 1;
    }

    #[allow(dead_code)]
    #[inline(always)]
    pub(crate) fn double(self) -> Self {
        let mut r = self;
        r.set_double();
        r
    }

    // Negation (internally, wraps around at 64 bits).
    #[inline(always)]
    pub(crate) fn set_neg(&mut self) {
        self.0 = self.0.wrapping_neg();
    }

    // Absolute value. If the represented value is -2^31, then this
    // overflows (because +2^31 is not representable) and -2^31 is
    // returned.
    #[allow(dead_code)]
    #[inline(always)]
    pub(crate) fn set_abs(&mut self) {
        self.0 = self
            .0
            .wrapping_sub((self.0 << 1) & (((self.0 as i64) >> 63) as u64));
    }

    #[allow(dead_code)]
    #[inline(always)]
    pub(crate) fn abs(self) -> Self {
        let mut r = self;
        r.set_abs();
        r
    }

    // Multiplication (internally, wraps around at 64 bits).
    #[inline(always)]
    pub(crate) fn set_mul(&mut self, other: Self) {
        // We assume here that 64x64->128 multiplications are constant-time
        // on 64-bit x86, ARMv8 and RISC-V. This is not entirely true,
        // because at least on the 64-bit-aware ARM Cortex A53 and A55, the
        // multiplication completes a bit faster when both operands happen
        // to fit on 32 bits. Small leaks are considered less critical for
        // keygen, because each keygen naturally generates a new key pair,
        // uncorrelated with other key pairs, so that small leaks do not
        // accumulate with repreated experiments for breaking a single key.
        // (This argument does not hold if somebody is using this code to
        // repeatedly regenerate the same key pair, deterministically, from
        // a stored seed.)
        #[cfg(any(
            target_arch = "x86_64",
            target_arch = "aarch64",
            target_arch = "arm64ec",
            target_arch = "riscv64"
        ))]
        {
            let z = ((self.0 as i64) as i128) * ((other.0 as i64) as i128);
            self.0 = (z >> 32) as u64;
        }

        #[cfg(not(any(
            target_arch = "x86_64",
            target_arch = "aarch64",
            target_arch = "arm64ec",
            target_arch = "riscv64"
        )))]
        {
            let xl = self.0 as u32;
            let xh = (self.0 >> 32) as i32;
            let yl = other.0 as u32;
            let yh = (other.0 >> 32) as i32;
            let z0 = ((xl as u64) * (yl as u64)) >> 32;
            let z1 = ((xl as i64) * (yh as i64)) as u64;
            let z2 = ((xh as i64) * (yl as i64)) as u64;
            let z3 = (xh.wrapping_mul(yh) as u32 as u64) << 32;
            self.0 = z0.wrapping_add(z1).wrapping_add(z2).wrapping_add(z3);
        }
    }

    // Squaring (internally, wraps around at 64 bits).
    #[inline(always)]
    pub(crate) fn set_sqr(&mut self) {
        let x = *self;
        self.0 = (x * x).0;
    }

    #[inline(always)]
    pub(crate) fn sqr(self) -> Self {
        let mut r = self;
        r.set_sqr();
        r
    }

    // Division by 2^e. Rounding to nearest representable value is
    // applied (rounding up for results which are half-way between two
    // successive representable values). Shift count MUST be less than
    // 64. Shift count MAY be zero (in which case the value is unchanged).
    #[inline(always)]
    pub(crate) fn set_div2e(&mut self, e: u32) {
        let z = self.0.wrapping_add((1u64 << e) >> 1);
        self.0 = ((z as i64) >> e) as u64;
    }

    #[allow(dead_code)]
    #[inline(always)]
    pub(crate) fn div2e(self, e: u32) -> Self {
        let mut r = self;
        r.set_div2e(e);
        r
    }

    // Halving: this is equivalent to div2e(1).
    #[inline(always)]
    pub(crate) fn set_half(&mut self) {
        self.set_div2e(1);
    }

    #[allow(dead_code)]
    #[inline(always)]
    pub(crate) fn half(self) -> Self {
        self.div2e(1)
    }

    // Multiplication by 2^e (internally, wraps around at 64 bits).
    // Shift count MUST be less than 64. Shift count MAY be zero (in which
    // case the value is unchanged).
    #[inline(always)]
    pub(crate) fn set_mul2e(&mut self, e: u32) {
        self.0 <<= e;
    }

    #[allow(dead_code)]
    #[inline(always)]
    pub(crate) fn mul2e(self, e: u32) -> Self {
        let mut r = self;
        r.set_mul2e(e);
        r
    }

    // Inversion. Equivalent to dividing 1 by this value.
    #[allow(dead_code)]
    #[inline(always)]
    pub(crate) fn set_inv(&mut self) {
        self.0 = Self::inner_div(1u64 << 32, self.0);
    }

    #[allow(dead_code)]
    #[inline(always)]
    pub(crate) fn inv(self) -> Self {
        let mut r = self;
        r.set_inv();
        r
    }

    // Division. An internal division algorithm is applied; overflows
    // and similar edge conditions are ignored.
    #[inline(always)]
    pub(crate) fn set_div(&mut self, other: Self) {
        self.0 = Self::inner_div(self.0, other.0);
    }

    // Internal division routine. The steps must be followed exactly in
    // order to obtain reproducible values that match the reference
    // specification, even in case of overflow.
    fn inner_div(x: u64, y: u64) -> u64 {
        // Get absolute values and signs. We ignore the edge condition
        // of an overflow here; we afterwards assume that x and y fit
        // on 64 bits.
        let sx: u64 = ((x as i64) >> 63) as u64;
        let x = (x ^ sx).wrapping_sub(sx);
        let sy: u64 = ((y as i64) >> 63) as u64;
        let y = (y ^ sy).wrapping_sub(sy);

        // Do a bit by by division, assuming that the quotient fits. The
        // numerators starts at x*2, and is shifted one bit at a time.
        let mut q = 0;
        let mut num = x >> 31;
        for i in (33..64).rev() {
            let b = (((num.wrapping_sub(y) as i64) >> 63) + 1) as u64;
            q |= b << i;
            num = num.wrapping_sub(y & b.wrapping_neg());
            num <<= 1;
            num |= (x >> (i - 33)) & 1;
        }
        for i in (0..33).rev() {
            let b = (((num.wrapping_sub(y) as i64) >> 63) + 1) as u64;
            q |= b << i;
            num = num.wrapping_sub(y & b.wrapping_neg());
            num <<= 1;
        }

        // Rounding: if the remainder is at least y/2 (scaled), then we
        // add 2^(-32) to the quotient.
        let b = (((num.wrapping_sub(y) as i64) >> 63) + 1) as u64;
        q = q.wrapping_add(b);

        // Sign management: if the original x and y had different signs,
        // then we must negate the quotient.
        let s = sx ^ sy;
        q = (q ^ s).wrapping_sub(s);
        q
    }
}

impl Ord for FXR {
    #[inline(always)]
    fn cmp(&self, other: &Self) -> Ordering {
        (self.0 as i64).cmp(&(other.0 as i64))
    }
}

impl PartialOrd for FXR {
    #[inline(always)]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for FXR {
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Add<FXR> for FXR {
    type Output = FXR;

    #[inline(always)]
    fn add(self, other: FXR) -> FXR {
        let mut r = self;
        r.set_add(other);
        r
    }
}

impl Add<&FXR> for FXR {
    type Output = FXR;

    #[inline(always)]
    fn add(self, other: &FXR) -> FXR {
        let mut r = self;
        r.set_add(*other);
        r
    }
}

impl Add<FXR> for &FXR {
    type Output = FXR;

    #[inline(always)]
    fn add(self, other: FXR) -> FXR {
        let mut r = *self;
        r.set_add(other);
        r
    }
}

impl Add<&FXR> for &FXR {
    type Output = FXR;

    #[inline(always)]
    fn add(self, other: &FXR) -> FXR {
        let mut r = *self;
        r.set_add(*other);
        r
    }
}

impl AddAssign<FXR> for FXR {
    #[inline(always)]
    fn add_assign(&mut self, other: FXR) {
        self.set_add(other);
    }
}

impl AddAssign<&FXR> for FXR {
    #[inline(always)]
    fn add_assign(&mut self, other: &FXR) {
        self.set_add(*other);
    }
}

impl Div<FXR> for FXR {
    type Output = FXR;

    #[inline(always)]
    fn div(self, other: FXR) -> FXR {
        let mut r = self;
        r.set_div(other);
        r
    }
}

impl Div<&FXR> for FXR {
    type Output = FXR;

    #[inline(always)]
    fn div(self, other: &FXR) -> FXR {
        let mut r = self;
        r.set_div(*other);
        r
    }
}

impl Div<FXR> for &FXR {
    type Output = FXR;

    #[inline(always)]
    fn div(self, other: FXR) -> FXR {
        let mut r = *self;
        r.set_div(other);
        r
    }
}

impl Div<&FXR> for &FXR {
    type Output = FXR;

    #[inline(always)]
    fn div(self, other: &FXR) -> FXR {
        let mut r = *self;
        r.set_div(*other);
        r
    }
}

impl DivAssign<FXR> for FXR {
    #[inline(always)]
    fn div_assign(&mut self, other: FXR) {
        self.set_div(other);
    }
}

impl DivAssign<&FXR> for FXR {
    #[inline(always)]
    fn div_assign(&mut self, other: &FXR) {
        self.set_div(*other);
    }
}

impl Mul<FXR> for FXR {
    type Output = FXR;

    #[inline(always)]
    fn mul(self, other: FXR) -> FXR {
        let mut r = self;
        r.set_mul(other);
        r
    }
}

impl Mul<&FXR> for FXR {
    type Output = FXR;

    #[inline(always)]
    fn mul(self, other: &FXR) -> FXR {
        let mut r = self;
        r.set_mul(*other);
        r
    }
}

impl Mul<FXR> for &FXR {
    type Output = FXR;

    #[inline(always)]
    fn mul(self, other: FXR) -> FXR {
        let mut r = *self;
        r.set_mul(other);
        r
    }
}

impl Mul<&FXR> for &FXR {
    type Output = FXR;

    #[inline(always)]
    fn mul(self, other: &FXR) -> FXR {
        let mut r = *self;
        r.set_mul(*other);
        r
    }
}

impl MulAssign<FXR> for FXR {
    #[inline(always)]
    fn mul_assign(&mut self, other: FXR) {
        self.set_mul(other);
    }
}

impl MulAssign<&FXR> for FXR {
    #[inline(always)]
    fn mul_assign(&mut self, other: &FXR) {
        self.set_mul(*other);
    }
}

impl Neg for FXR {
    type Output = FXR;

    #[inline(always)]
    fn neg(self) -> FXR {
        let mut r = self;
        r.set_neg();
        r
    }
}

impl Neg for &FXR {
    type Output = FXR;

    #[inline(always)]
    fn neg(self) -> FXR {
        let mut r = *self;
        r.set_neg();
        r
    }
}

impl Sub<FXR> for FXR {
    type Output = FXR;

    #[inline(always)]
    fn sub(self, other: FXR) -> FXR {
        let mut r = self;
        r.set_sub(other);
        r
    }
}

impl Sub<&FXR> for FXR {
    type Output = FXR;

    #[inline(always)]
    fn sub(self, other: &FXR) -> FXR {
        let mut r = self;
        r.set_sub(*other);
        r
    }
}

impl Sub<FXR> for &FXR {
    type Output = FXR;

    #[inline(always)]
    fn sub(self, other: FXR) -> FXR {
        let mut r = *self;
        r.set_sub(other);
        r
    }
}

impl Sub<&FXR> for &FXR {
    type Output = FXR;

    #[inline(always)]
    fn sub(self, other: &FXR) -> FXR {
        let mut r = *self;
        r.set_sub(*other);
        r
    }
}

impl SubAssign<FXR> for FXR {
    #[inline(always)]
    fn sub_assign(&mut self, other: FXR) {
        self.set_sub(other);
    }
}

impl SubAssign<&FXR> for FXR {
    #[inline(always)]
    fn sub_assign(&mut self, other: &FXR) {
        self.set_sub(*other);
    }
}

// A wrapper for a complex number, whose real and imaginary parts both use
// fixed-point (FXR).
#[derive(Clone, Copy, Debug)]
pub(crate) struct FXC {
    pub(crate) re: FXR,
    pub(crate) im: FXR,
}

impl FXC {
    #[inline(always)]
    fn set_add(&mut self, other: &Self) {
        self.re += other.re;
        self.im += other.im;
    }

    #[inline(always)]
    fn set_sub(&mut self, other: &Self) {
        self.re -= other.re;
        self.im -= other.im;
    }

    #[inline(always)]
    fn set_neg(&mut self) {
        self.re.set_neg();
        self.im.set_neg();
    }

    #[inline(always)]
    pub(crate) fn set_half(&mut self) {
        self.re.set_half();
        self.im.set_half();
    }

    #[inline(always)]
    pub(crate) fn half(self) -> Self {
        let mut r = self;
        r.set_half();
        r
    }

    #[inline(always)]
    fn set_mul(&mut self, other: &Self) {
        // We are computing r = (a + i*b)*(c + i*d) with:
        //   z0 = a*c
        //   z1 = b*d
        //   z2 = (a + b)*(c + d)
        //   r = (z0 - z1) + i*(z2 - (z0 + z1))
        // We must follow these formulas to be sure to obtain the same
        // output as the reference code.
        let z0 = self.re * other.re;
        let z1 = self.im * other.im;
        let z2 = (self.re + self.im) * (other.re + other.im);
        self.re = z0 - z1;
        self.im = z2 - (z0 + z1);
    }

    #[inline(always)]
    pub(crate) fn set_conj(&mut self) {
        self.im.set_neg();
    }

    #[inline(always)]
    pub(crate) fn conj(self) -> Self {
        let mut r = self;
        r.set_conj();
        r
    }
}

impl Add<FXC> for FXC {
    type Output = FXC;

    #[inline(always)]
    fn add(self, other: FXC) -> FXC {
        let mut r = self;
        r.set_add(&other);
        r
    }
}

impl Add<&FXC> for FXC {
    type Output = FXC;

    #[inline(always)]
    fn add(self, other: &FXC) -> FXC {
        let mut r = self;
        r.set_add(other);
        r
    }
}

impl Add<FXC> for &FXC {
    type Output = FXC;

    #[inline(always)]
    fn add(self, other: FXC) -> FXC {
        let mut r = *self;
        r.set_add(&other);
        r
    }
}

impl Add<&FXC> for &FXC {
    type Output = FXC;

    #[inline(always)]
    fn add(self, other: &FXC) -> FXC {
        let mut r = *self;
        r.set_add(other);
        r
    }
}

impl AddAssign<FXC> for FXC {
    #[inline(always)]
    fn add_assign(&mut self, other: FXC) {
        self.set_add(&other);
    }
}

impl AddAssign<&FXC> for FXC {
    #[inline(always)]
    fn add_assign(&mut self, other: &FXC) {
        self.set_add(other);
    }
}

impl Mul<FXC> for FXC {
    type Output = FXC;

    #[inline(always)]
    fn mul(self, other: FXC) -> FXC {
        let mut r = self;
        r.set_mul(&other);
        r
    }
}

impl Mul<&FXC> for FXC {
    type Output = FXC;

    #[inline(always)]
    fn mul(self, other: &FXC) -> FXC {
        let mut r = self;
        r.set_mul(other);
        r
    }
}

impl Mul<FXC> for &FXC {
    type Output = FXC;

    #[inline(always)]
    fn mul(self, other: FXC) -> FXC {
        let mut r = *self;
        r.set_mul(&other);
        r
    }
}

impl Mul<&FXC> for &FXC {
    type Output = FXC;

    #[inline(always)]
    fn mul(self, other: &FXC) -> FXC {
        let mut r = *self;
        r.set_mul(other);
        r
    }
}

impl MulAssign<FXC> for FXC {
    #[inline(always)]
    fn mul_assign(&mut self, other: FXC) {
        self.set_mul(&other);
    }
}

impl MulAssign<&FXC> for FXC {
    #[inline(always)]
    fn mul_assign(&mut self, other: &FXC) {
        self.set_mul(other);
    }
}

impl Neg for FXC {
    type Output = FXC;

    #[inline(always)]
    fn neg(self) -> FXC {
        let mut r = self;
        r.set_neg();
        r
    }
}

impl Neg for &FXC {
    type Output = FXC;

    #[inline(always)]
    fn neg(self) -> FXC {
        let mut r = *self;
        r.set_neg();
        r
    }
}

impl Sub<FXC> for FXC {
    type Output = FXC;

    #[inline(always)]
    fn sub(self, other: FXC) -> FXC {
        let mut r = self;
        r.set_sub(&other);
        r
    }
}

impl Sub<&FXC> for FXC {
    type Output = FXC;

    #[inline(always)]
    fn sub(self, other: &FXC) -> FXC {
        let mut r = self;
        r.set_sub(other);
        r
    }
}

impl Sub<FXC> for &FXC {
    type Output = FXC;

    #[inline(always)]
    fn sub(self, other: FXC) -> FXC {
        let mut r = *self;
        r.set_sub(&other);
        r
    }
}

impl Sub<&FXC> for &FXC {
    type Output = FXC;

    #[inline(always)]
    fn sub(self, other: &FXC) -> FXC {
        let mut r = *self;
        r.set_sub(other);
        r
    }
}

impl SubAssign<FXC> for FXC {
    #[inline(always)]
    fn sub_assign(&mut self, other: FXC) {
        self.set_sub(&other);
    }
}

impl SubAssign<&FXC> for FXC {
    #[inline(always)]
    fn sub_assign(&mut self, other: &FXC) {
        self.set_sub(other);
    }
}

// FFT constants:
//   n = 2^1024
//   w = exp(i*pi/n) (a primitive 2n-th root of 1)
//   w_j = w^(2*j+1) for j = 0..n-1 (n-th roots of -1)
//   rev() = bit reversal over 10 bits
//   GM_TAB[rev(j)] contains w_j
pub(crate) const GM_TAB: [FXC; 1024] = [
    FXC {
        re: FXR(4294967296),
        im: FXR(0),
    },
    FXC {
        re: FXR(0),
        im: FXR(4294967296),
    },
    FXC {
        re: FXR(3037000500),
        im: FXR(3037000500),
    },
    FXC {
        re: FXR(18446744070672551116),
        im: FXR(3037000500),
    },
    FXC {
        re: FXR(3968032378),
        im: FXR(1643612827),
    },
    FXC {
        re: FXR(18446744072065938789),
        im: FXR(3968032378),
    },
    FXC {
        re: FXR(1643612827),
        im: FXR(3968032378),
    },
    FXC {
        re: FXR(18446744069741519238),
        im: FXR(1643612827),
    },
    FXC {
        re: FXR(4212440704),
        im: FXR(837906553),
    },
    FXC {
        re: FXR(18446744072871645063),
        im: FXR(4212440704),
    },
    FXC {
        re: FXR(2386155981),
        im: FXR(3571134792),
    },
    FXC {
        re: FXR(18446744070138416824),
        im: FXR(2386155981),
    },
    FXC {
        re: FXR(3571134792),
        im: FXR(2386155981),
    },
    FXC {
        re: FXR(18446744071323395635),
        im: FXR(3571134792),
    },
    FXC {
        re: FXR(837906553),
        im: FXR(4212440704),
    },
    FXC {
        re: FXR(18446744069497110912),
        im: FXR(837906553),
    },
    FXC {
        re: FXR(4274285855),
        im: FXR(420980412),
    },
    FXC {
        re: FXR(18446744073288571204),
        im: FXR(4274285855),
    },
    FXC {
        re: FXR(2724698408),
        im: FXR(3320054617),
    },
    FXC {
        re: FXR(18446744070389496999),
        im: FXR(2724698408),
    },
    FXC {
        re: FXR(3787822988),
        im: FXR(2024633568),
    },
    FXC {
        re: FXR(18446744071684918048),
        im: FXR(3787822988),
    },
    FXC {
        re: FXR(1246763195),
        im: FXR(4110027446),
    },
    FXC {
        re: FXR(18446744069599524170),
        im: FXR(1246763195),
    },
    FXC {
        re: FXR(4110027446),
        im: FXR(1246763195),
    },
    FXC {
        re: FXR(18446744072462788421),
        im: FXR(4110027446),
    },
    FXC {
        re: FXR(2024633568),
        im: FXR(3787822988),
    },
    FXC {
        re: FXR(18446744069921728628),
        im: FXR(2024633568),
    },
    FXC {
        re: FXR(3320054617),
        im: FXR(2724698408),
    },
    FXC {
        re: FXR(18446744070984853208),
        im: FXR(3320054617),
    },
    FXC {
        re: FXR(420980412),
        im: FXR(4274285855),
    },
    FXC {
        re: FXR(18446744069435265761),
        im: FXR(420980412),
    },
    FXC {
        re: FXR(4289793820),
        im: FXR(210744057),
    },
    FXC {
        re: FXR(18446744073498807559),
        im: FXR(4289793820),
    },
    FXC {
        re: FXR(2884323748),
        im: FXR(3182360851),
    },
    FXC {
        re: FXR(18446744070527190765),
        im: FXR(2884323748),
    },
    FXC {
        re: FXR(3882604450),
        im: FXR(1836335144),
    },
    FXC {
        re: FXR(18446744071873216472),
        im: FXR(3882604450),
    },
    FXC {
        re: FXR(1446930903),
        im: FXR(4043900968),
    },
    FXC {
        re: FXR(18446744069665650648),
        im: FXR(1446930903),
    },
    FXC {
        re: FXR(4166252509),
        im: FXR(1043591926),
    },
    FXC {
        re: FXR(18446744072665959690),
        im: FXR(4166252509),
    },
    FXC {
        re: FXR(2208054473),
        im: FXR(3683916329),
    },
    FXC {
        re: FXR(18446744070025635287),
        im: FXR(2208054473),
    },
    FXC {
        re: FXR(3449750080),
        im: FXR(2558509031),
    },
    FXC {
        re: FXR(18446744071151042585),
        im: FXR(3449750080),
    },
    FXC {
        re: FXR(630202589),
        im: FXR(4248480760),
    },
    FXC {
        re: FXR(18446744069461070856),
        im: FXR(630202589),
    },
    FXC {
        re: FXR(4248480760),
        im: FXR(630202589),
    },
    FXC {
        re: FXR(18446744073079349027),
        im: FXR(4248480760),
    },
    FXC {
        re: FXR(2558509031),
        im: FXR(3449750080),
    },
    FXC {
        re: FXR(18446744070259801536),
        im: FXR(2558509031),
    },
    FXC {
        re: FXR(3683916329),
        im: FXR(2208054473),
    },
    FXC {
        re: FXR(18446744071501497143),
        im: FXR(3683916329),
    },
    FXC {
        re: FXR(1043591926),
        im: FXR(4166252509),
    },
    FXC {
        re: FXR(18446744069543299107),
        im: FXR(1043591926),
    },
    FXC {
        re: FXR(4043900968),
        im: FXR(1446930903),
    },
    FXC {
        re: FXR(18446744072262620713),
        im: FXR(4043900968),
    },
    FXC {
        re: FXR(1836335144),
        im: FXR(3882604450),
    },
    FXC {
        re: FXR(18446744069826947166),
        im: FXR(1836335144),
    },
    FXC {
        re: FXR(3182360851),
        im: FXR(2884323748),
    },
    FXC {
        re: FXR(18446744070825227868),
        im: FXR(3182360851),
    },
    FXC {
        re: FXR(210744057),
        im: FXR(4289793820),
    },
    FXC {
        re: FXR(18446744069419757796),
        im: FXR(210744057),
    },
    FXC {
        re: FXR(4293673732),
        im: FXR(105403774),
    },
    FXC {
        re: FXR(18446744073604147842),
        im: FXR(4293673732),
    },
    FXC {
        re: FXR(2961554089),
        im: FXR(3110617535),
    },
    FXC {
        re: FXR(18446744070598934081),
        im: FXR(2961554089),
    },
    FXC {
        re: FXR(3926501002),
        im: FXR(1740498191),
    },
    FXC {
        re: FXR(18446744071969053425),
        im: FXR(3926501002),
    },
    FXC {
        re: FXR(1545737412),
        im: FXR(4007173558),
    },
    FXC {
        re: FXR(18446744069702378058),
        im: FXR(1545737412),
    },
    FXC {
        re: FXR(4190608739),
        im: FXR(941032661),
    },
    FXC {
        re: FXR(18446744072768518955),
        im: FXR(4190608739),
    },
    FXC {
        re: FXR(2297797281),
        im: FXR(3628618433),
    },
    FXC {
        re: FXR(18446744070080933183),
        im: FXR(2297797281),
    },
    FXC {
        re: FXR(3511500034),
        im: FXR(2473077351),
    },
    FXC {
        re: FXR(18446744071236474265),
        im: FXR(3511500034),
    },
    FXC {
        re: FXR(734275721),
        im: FXR(4231735252),
    },
    FXC {
        re: FXR(18446744069477816364),
        im: FXR(734275721),
    },
    FXC {
        re: FXR(4262667143),
        im: FXR(525749847),
    },
    FXC {
        re: FXR(18446744073183801769),
        im: FXR(4262667143),
    },
    FXC {
        re: FXR(2642399561),
        im: FXR(3385922125),
    },
    FXC {
        re: FXR(18446744070323629491),
        im: FXR(2642399561),
    },
    FXC {
        re: FXR(3736995171),
        im: FXR(2116981616),
    },
    FXC {
        re: FXR(18446744071592570000),
        im: FXR(3736995171),
    },
    FXC {
        re: FXR(1145522571),
        im: FXR(4139386683),
    },
    FXC {
        re: FXR(18446744069570164933),
        im: FXR(1145522571),
    },
    FXC {
        re: FXR(4078192482),
        im: FXR(1347252816),
    },
    FXC {
        re: FXR(18446744072362298800),
        im: FXR(4078192482),
    },
    FXC {
        re: FXR(1931065957),
        im: FXR(3836369162),
    },
    FXC {
        re: FXR(18446744069873182454),
        im: FXR(1931065957),
    },
    FXC {
        re: FXR(3252187232),
        im: FXR(2805355999),
    },
    FXC {
        re: FXR(18446744070904195617),
        im: FXR(3252187232),
    },
    FXC {
        re: FXR(315957395),
        im: FXR(4283329896),
    },
    FXC {
        re: FXR(18446744069426221720),
        im: FXR(315957395),
    },
    FXC {
        re: FXR(4283329896),
        im: FXR(315957395),
    },
    FXC {
        re: FXR(18446744073393594221),
        im: FXR(4283329896),
    },
    FXC {
        re: FXR(2805355999),
        im: FXR(3252187232),
    },
    FXC {
        re: FXR(18446744070457364384),
        im: FXR(2805355999),
    },
    FXC {
        re: FXR(3836369162),
        im: FXR(1931065957),
    },
    FXC {
        re: FXR(18446744071778485659),
        im: FXR(3836369162),
    },
    FXC {
        re: FXR(1347252816),
        im: FXR(4078192482),
    },
    FXC {
        re: FXR(18446744069631359134),
        im: FXR(1347252816),
    },
    FXC {
        re: FXR(4139386683),
        im: FXR(1145522571),
    },
    FXC {
        re: FXR(18446744072564029045),
        im: FXR(4139386683),
    },
    FXC {
        re: FXR(2116981616),
        im: FXR(3736995171),
    },
    FXC {
        re: FXR(18446744069972556445),
        im: FXR(2116981616),
    },
    FXC {
        re: FXR(3385922125),
        im: FXR(2642399561),
    },
    FXC {
        re: FXR(18446744071067152055),
        im: FXR(3385922125),
    },
    FXC {
        re: FXR(525749847),
        im: FXR(4262667143),
    },
    FXC {
        re: FXR(18446744069446884473),
        im: FXR(525749847),
    },
    FXC {
        re: FXR(4231735252),
        im: FXR(734275721),
    },
    FXC {
        re: FXR(18446744072975275895),
        im: FXR(4231735252),
    },
    FXC {
        re: FXR(2473077351),
        im: FXR(3511500034),
    },
    FXC {
        re: FXR(18446744070198051582),
        im: FXR(2473077351),
    },
    FXC {
        re: FXR(3628618433),
        im: FXR(2297797281),
    },
    FXC {
        re: FXR(18446744071411754335),
        im: FXR(3628618433),
    },
    FXC {
        re: FXR(941032661),
        im: FXR(4190608739),
    },
    FXC {
        re: FXR(18446744069518942877),
        im: FXR(941032661),
    },
    FXC {
        re: FXR(4007173558),
        im: FXR(1545737412),
    },
    FXC {
        re: FXR(18446744072163814204),
        im: FXR(4007173558),
    },
    FXC {
        re: FXR(1740498191),
        im: FXR(3926501002),
    },
    FXC {
        re: FXR(18446744069783050614),
        im: FXR(1740498191),
    },
    FXC {
        re: FXR(3110617535),
        im: FXR(2961554089),
    },
    FXC {
        re: FXR(18446744070747997527),
        im: FXR(3110617535),
    },
    FXC {
        re: FXR(105403774),
        im: FXR(4293673732),
    },
    FXC {
        re: FXR(18446744069415877884),
        im: FXR(105403774),
    },
    FXC {
        re: FXR(4294643893),
        im: FXR(52705856),
    },
    FXC {
        re: FXR(18446744073656845760),
        im: FXR(4294643893),
    },
    FXC {
        re: FXR(2999503152),
        im: FXR(3074040487),
    },
    FXC {
        re: FXR(18446744070635511129),
        im: FXR(2999503152),
    },
    FXC {
        re: FXR(3947563934),
        im: FXR(1692182927),
    },
    FXC {
        re: FXR(18446744072017368689),
        im: FXR(3947563934),
    },
    FXC {
        re: FXR(1594795204),
        im: FXR(3987903250),
    },
    FXC {
        re: FXR(18446744069721648366),
        im: FXR(1594795204),
    },
    FXC {
        re: FXR(4201841112),
        im: FXR(889536587),
    },
    FXC {
        re: FXR(18446744072820015029),
        im: FXR(4201841112),
    },
    FXC {
        re: FXR(2342152991),
        im: FXR(3600147697),
    },
    FXC {
        re: FXR(18446744070109403919),
        im: FXR(2342152991),
    },
    FXC {
        re: FXR(3541584088),
        im: FXR(2429799626),
    },
    FXC {
        re: FXR(18446744071279751990),
        im: FXR(3541584088),
    },
    FXC {
        re: FXR(786150333),
        im: FXR(4222405917),
    },
    FXC {
        re: FXR(18446744069487145699),
        im: FXR(786150333),
    },
    FXC {
        re: FXR(4268797931),
        im: FXR(473400776),
    },
    FXC {
        re: FXR(18446744073236150840),
        im: FXR(4268797931),
    },
    FXC {
        re: FXR(2683751066),
        im: FXR(3353240863),
    },
    FXC {
        re: FXR(18446744070356310753),
        im: FXR(2683751066),
    },
    FXC {
        re: FXR(3762692404),
        im: FXR(2070963532),
    },
    FXC {
        re: FXR(18446744071638588084),
        im: FXR(3762692404),
    },
    FXC {
        re: FXR(1196232957),
        im: FXR(4125017671),
    },
    FXC {
        re: FXR(18446744069584533945),
        im: FXR(1196232957),
    },
    FXC {
        re: FXR(4094418266),
        im: FXR(1297105676),
    },
    FXC {
        re: FXR(18446744072412445940),
        im: FXR(4094418266),
    },
    FXC {
        re: FXR(1977998702),
        im: FXR(3812383140),
    },
    FXC {
        re: FXR(18446744069897168476),
        im: FXR(1977998702),
    },
    FXC {
        re: FXR(3286368382),
        im: FXR(2765235421),
    },
    FXC {
        re: FXR(18446744070944316195),
        im: FXR(3286368382),
    },
    FXC {
        re: FXR(368496651),
        im: FXR(4279130086),
    },
    FXC {
        re: FXR(18446744069430421530),
        im: FXR(368496651),
    },
    FXC {
        re: FXR(4286884652),
        im: FXR(263370557),
    },
    FXC {
        re: FXR(18446744073446181059),
        im: FXR(4286884652),
    },
    FXC {
        re: FXR(2845054101),
        im: FXR(3217516315),
    },
    FXC {
        re: FXR(18446744070492035301),
        im: FXR(2845054101),
    },
    FXC {
        re: FXR(3859777440),
        im: FXR(1883842400),
    },
    FXC {
        re: FXR(18446744071825709216),
        im: FXR(3859777440),
    },
    FXC {
        re: FXR(1397197066),
        im: FXR(4061352537),
    },
    FXC {
        re: FXR(18446744069648199079),
        im: FXR(1397197066),
    },
    FXC {
        re: FXR(4153132319),
        im: FXR(1094639673),
    },
    FXC {
        re: FXR(18446744072614911943),
        im: FXR(4153132319),
    },
    FXC {
        re: FXR(2162680890),
        im: FXR(3710735162),
    },
    FXC {
        re: FXR(18446744069998816454),
        im: FXR(2162680890),
    },
    FXC {
        re: FXR(3418093478),
        im: FXR(2600650120),
    },
    FXC {
        re: FXR(18446744071108901496),
        im: FXR(3418093478),
    },
    FXC {
        re: FXR(578019742),
        im: FXR(4255894413),
    },
    FXC {
        re: FXR(18446744069453657203),
        im: FXR(578019742),
    },
    FXC {
        re: FXR(4240427302),
        im: FXR(682290530),
    },
    FXC {
        re: FXR(18446744073027261086),
        im: FXR(4240427302),
    },
    FXC {
        re: FXR(2515982640),
        im: FXR(3480887161),
    },
    FXC {
        re: FXR(18446744070228664455),
        im: FXR(2515982640),
    },
    FXC {
        re: FXR(3656542712),
        im: FXR(2253095531),
    },
    FXC {
        re: FXR(18446744071456456085),
        im: FXR(3656542712),
    },
    FXC {
        re: FXR(992387019),
        im: FXR(4178745276),
    },
    FXC {
        re: FXR(18446744069530806340),
        im: FXR(992387019),
    },
    FXC {
        re: FXR(4025840401),
        im: FXR(1496446837),
    },
    FXC {
        re: FXR(18446744072213104779),
        im: FXR(4025840401),
    },
    FXC {
        re: FXR(1788551342),
        im: FXR(3904846754),
    },
    FXC {
        re: FXR(18446744069804704862),
        im: FXR(1788551342),
    },
    FXC {
        re: FXR(3146726136),
        im: FXR(2923159027),
    },
    FXC {
        re: FXR(18446744070786392589),
        im: FXR(3146726136),
    },
    FXC {
        re: FXR(158085819),
        im: FXR(4292056960),
    },
    FXC {
        re: FXR(18446744069417494656),
        im: FXR(158085819),
    },
    FXC {
        re: FXR(4292056960),
        im: FXR(158085819),
    },
    FXC {
        re: FXR(18446744073551465797),
        im: FXR(4292056960),
    },
    FXC {
        re: FXR(2923159027),
        im: FXR(3146726136),
    },
    FXC {
        re: FXR(18446744070562825480),
        im: FXR(2923159027),
    },
    FXC {
        re: FXR(3904846754),
        im: FXR(1788551342),
    },
    FXC {
        re: FXR(18446744071921000274),
        im: FXR(3904846754),
    },
    FXC {
        re: FXR(1496446837),
        im: FXR(4025840401),
    },
    FXC {
        re: FXR(18446744069683711215),
        im: FXR(1496446837),
    },
    FXC {
        re: FXR(4178745276),
        im: FXR(992387019),
    },
    FXC {
        re: FXR(18446744072717164597),
        im: FXR(4178745276),
    },
    FXC {
        re: FXR(2253095531),
        im: FXR(3656542712),
    },
    FXC {
        re: FXR(18446744070053008904),
        im: FXR(2253095531),
    },
    FXC {
        re: FXR(3480887161),
        im: FXR(2515982640),
    },
    FXC {
        re: FXR(18446744071193568976),
        im: FXR(3480887161),
    },
    FXC {
        re: FXR(682290530),
        im: FXR(4240427302),
    },
    FXC {
        re: FXR(18446744069469124314),
        im: FXR(682290530),
    },
    FXC {
        re: FXR(4255894413),
        im: FXR(578019742),
    },
    FXC {
        re: FXR(18446744073131531874),
        im: FXR(4255894413),
    },
    FXC {
        re: FXR(2600650120),
        im: FXR(3418093478),
    },
    FXC {
        re: FXR(18446744070291458138),
        im: FXR(2600650120),
    },
    FXC {
        re: FXR(3710735162),
        im: FXR(2162680890),
    },
    FXC {
        re: FXR(18446744071546870726),
        im: FXR(3710735162),
    },
    FXC {
        re: FXR(1094639673),
        im: FXR(4153132319),
    },
    FXC {
        re: FXR(18446744069556419297),
        im: FXR(1094639673),
    },
    FXC {
        re: FXR(4061352537),
        im: FXR(1397197066),
    },
    FXC {
        re: FXR(18446744072312354550),
        im: FXR(4061352537),
    },
    FXC {
        re: FXR(1883842400),
        im: FXR(3859777440),
    },
    FXC {
        re: FXR(18446744069849774176),
        im: FXR(1883842400),
    },
    FXC {
        re: FXR(3217516315),
        im: FXR(2845054101),
    },
    FXC {
        re: FXR(18446744070864497515),
        im: FXR(3217516315),
    },
    FXC {
        re: FXR(263370557),
        im: FXR(4286884652),
    },
    FXC {
        re: FXR(18446744069422666964),
        im: FXR(263370557),
    },
    FXC {
        re: FXR(4279130086),
        im: FXR(368496651),
    },
    FXC {
        re: FXR(18446744073341054965),
        im: FXR(4279130086),
    },
    FXC {
        re: FXR(2765235421),
        im: FXR(3286368382),
    },
    FXC {
        re: FXR(18446744070423183234),
        im: FXR(2765235421),
    },
    FXC {
        re: FXR(3812383140),
        im: FXR(1977998702),
    },
    FXC {
        re: FXR(18446744071731552914),
        im: FXR(3812383140),
    },
    FXC {
        re: FXR(1297105676),
        im: FXR(4094418266),
    },
    FXC {
        re: FXR(18446744069615133350),
        im: FXR(1297105676),
    },
    FXC {
        re: FXR(4125017671),
        im: FXR(1196232957),
    },
    FXC {
        re: FXR(18446744072513318659),
        im: FXR(4125017671),
    },
    FXC {
        re: FXR(2070963532),
        im: FXR(3762692404),
    },
    FXC {
        re: FXR(18446744069946859212),
        im: FXR(2070963532),
    },
    FXC {
        re: FXR(3353240863),
        im: FXR(2683751066),
    },
    FXC {
        re: FXR(18446744071025800550),
        im: FXR(3353240863),
    },
    FXC {
        re: FXR(473400776),
        im: FXR(4268797931),
    },
    FXC {
        re: FXR(18446744069440753685),
        im: FXR(473400776),
    },
    FXC {
        re: FXR(4222405917),
        im: FXR(786150333),
    },
    FXC {
        re: FXR(18446744072923401283),
        im: FXR(4222405917),
    },
    FXC {
        re: FXR(2429799626),
        im: FXR(3541584088),
    },
    FXC {
        re: FXR(18446744070167967528),
        im: FXR(2429799626),
    },
    FXC {
        re: FXR(3600147697),
        im: FXR(2342152991),
    },
    FXC {
        re: FXR(18446744071367398625),
        im: FXR(3600147697),
    },
    FXC {
        re: FXR(889536587),
        im: FXR(4201841112),
    },
    FXC {
        re: FXR(18446744069507710504),
        im: FXR(889536587),
    },
    FXC {
        re: FXR(3987903250),
        im: FXR(1594795204),
    },
    FXC {
        re: FXR(18446744072114756412),
        im: FXR(3987903250),
    },
    FXC {
        re: FXR(1692182927),
        im: FXR(3947563934),
    },
    FXC {
        re: FXR(18446744069761987682),
        im: FXR(1692182927),
    },
    FXC {
        re: FXR(3074040487),
        im: FXR(2999503152),
    },
    FXC {
        re: FXR(18446744070710048464),
        im: FXR(3074040487),
    },
    FXC {
        re: FXR(52705856),
        im: FXR(4294643893),
    },
    FXC {
        re: FXR(18446744069414907723),
        im: FXR(52705856),
    },
    FXC {
        re: FXR(4294886444),
        im: FXR(26353424),
    },
    FXC {
        re: FXR(18446744073683198192),
        im: FXR(4294886444),
    },
    FXC {
        re: FXR(3018308645),
        im: FXR(3055578014),
    },
    FXC {
        re: FXR(18446744070653973602),
        im: FXR(3018308645),
    },
    FXC {
        re: FXR(3957872662),
        im: FXR(1667929275),
    },
    FXC {
        re: FXR(18446744072041622341),
        im: FXR(3957872662),
    },
    FXC {
        re: FXR(1619234497),
        im: FXR(3978042699),
    },
    FXC {
        re: FXR(18446744069731508917),
        im: FXR(1619234497),
    },
    FXC {
        re: FXR(4207220108),
        im: FXR(863737830),
    },
    FXC {
        re: FXR(18446744072845813786),
        im: FXR(4207220108),
    },
    FXC {
        re: FXR(2364198992),
        im: FXR(3585708745),
    },
    FXC {
        re: FXR(18446744070123842871),
        im: FXR(2364198992),
    },
    FXC {
        re: FXR(3556426389),
        im: FXR(2408023134),
    },
    FXC {
        re: FXR(18446744071301528482),
        im: FXR(3556426389),
    },
    FXC {
        re: FXR(812043729),
        im: FXR(4217502704),
    },
    FXC {
        re: FXR(18446744069492048912),
        im: FXR(812043729),
    },
    FXC {
        re: FXR(4271622305),
        im: FXR(447199012),
    },
    FXC {
        re: FXR(18446744073262352604),
        im: FXR(4271622305),
    },
    FXC {
        re: FXR(2704275644),
        im: FXR(3336710553),
    },
    FXC {
        re: FXR(18446744070372841063),
        im: FXR(2704275644),
    },
    FXC {
        re: FXR(3775328765),
        im: FXR(2047837100),
    },
    FXC {
        re: FXR(18446744071661714516),
        im: FXR(3775328765),
    },
    FXC {
        re: FXR(1221521071),
        im: FXR(4117600071),
    },
    FXC {
        re: FXR(18446744069591951545),
        im: FXR(1221521071),
    },
    FXC {
        re: FXR(4102300081),
        im: FXR(1271958380),
    },
    FXC {
        re: FXR(18446744072437593236),
        im: FXR(4102300081),
    },
    FXC {
        re: FXR(2001353810),
        im: FXR(3800174601),
    },
    FXC {
        re: FXR(18446744069909377015),
        im: FXR(2001353810),
    },
    FXC {
        re: FXR(3303273682),
        im: FXR(2745018589),
    },
    FXC {
        re: FXR(18446744070964533027),
        im: FXR(3303273682),
    },
    FXC {
        re: FXR(394745962),
        im: FXR(4276788480),
    },
    FXC {
        re: FXR(18446744069432763136),
        im: FXR(394745962),
    },
    FXC {
        re: FXR(4288419964),
        im: FXR(237061769),
    },
    FXC {
        re: FXR(18446744073472489847),
        im: FXR(4288419964),
    },
    FXC {
        re: FXR(2864742853),
        im: FXR(3199998822),
    },
    FXC {
        re: FXR(18446744070509552794),
        im: FXR(2864742853),
    },
    FXC {
        re: FXR(3871263820),
        im: FXR(1860123788),
    },
    FXC {
        re: FXR(18446744071849427828),
        im: FXR(3871263820),
    },
    FXC {
        re: FXR(1422090755),
        im: FXR(4052703044),
    },
    FXC {
        re: FXR(18446744069656848572),
        im: FXR(1422090755),
    },
    FXC {
        re: FXR(4159770720),
        im: FXR(1069135926),
    },
    FXC {
        re: FXR(18446744072640415690),
        im: FXR(4159770720),
    },
    FXC {
        re: FXR(2185408821),
        im: FXR(3697395348),
    },
    FXC {
        re: FXR(18446744070012156268),
        im: FXR(2185408821),
    },
    FXC {
        re: FXR(3433986423),
        im: FXR(2579628136),
    },
    FXC {
        re: FXR(18446744071129923480),
        im: FXR(3433986423),
    },
    FXC {
        re: FXR(604122538),
        im: FXR(4252267634),
    },
    FXC {
        re: FXR(18446744069457283982),
        im: FXR(604122538),
    },
    FXC {
        re: FXR(4244533933),
        im: FXR(656258914),
    },
    FXC {
        re: FXR(18446744073053292702),
        im: FXR(4244533933),
    },
    FXC {
        re: FXR(2537293599),
        im: FXR(3465383855),
    },
    FXC {
        re: FXR(18446744070244167761),
        im: FXR(2537293599),
    },
    FXC {
        re: FXR(3670298613),
        im: FXR(2230616993),
    },
    FXC {
        re: FXR(18446744071478934623),
        im: FXR(3670298613),
    },
    FXC {
        re: FXR(1018008636),
        im: FXR(4172577440),
    },
    FXC {
        re: FXR(18446744069536974176),
        im: FXR(1018008636),
    },
    FXC {
        re: FXR(4034946641),
        im: FXR(1471716574),
    },
    FXC {
        re: FXR(18446744072237835042),
        im: FXR(4034946641),
    },
    FXC {
        re: FXR(1812477362),
        im: FXR(3893798902),
    },
    FXC {
        re: FXR(18446744069815752714),
        im: FXR(1812477362),
    },
    FXC {
        re: FXR(3164603066),
        im: FXR(2903796051),
    },
    FXC {
        re: FXR(18446744070805755565),
        im: FXR(3164603066),
    },
    FXC {
        re: FXR(184418409),
        im: FXR(4291006167),
    },
    FXC {
        re: FXR(18446744069418545449),
        im: FXR(184418409),
    },
    FXC {
        re: FXR(4292946160),
        im: FXR(131747276),
    },
    FXC {
        re: FXR(18446744073577804340),
        im: FXR(4292946160),
    },
    FXC {
        re: FXR(2942411948),
        im: FXR(3128730733),
    },
    FXC {
        re: FXR(18446744070580820883),
        im: FXR(2942411948),
    },
    FXC {
        re: FXR(3915747591),
        im: FXR(1764557983),
    },
    FXC {
        re: FXR(18446744071944993633),
        im: FXR(3915747591),
    },
    FXC {
        re: FXR(1521120759),
        im: FXR(4016582591),
    },
    FXC {
        re: FXR(18446744069692969025),
        im: FXR(1521120759),
    },
    FXC {
        re: FXR(4184755784),
        im: FXR(966728038),
    },
    FXC {
        re: FXR(18446744072742823578),
        im: FXR(4184755784),
    },
    FXC {
        re: FXR(2275489241),
        im: FXR(3642649144),
    },
    FXC {
        re: FXR(18446744070066902472),
        im: FXR(2275489241),
    },
    FXC {
        re: FXR(3496259414),
        im: FXR(2494576955),
    },
    FXC {
        re: FXR(18446744071214974661),
        im: FXR(3496259414),
    },
    FXC {
        re: FXR(708296459),
        im: FXR(4236161021),
    },
    FXC {
        re: FXR(18446744069473390595),
        im: FXR(708296459),
    },
    FXC {
        re: FXR(4259360959),
        im: FXR(551895183),
    },
    FXC {
        re: FXR(18446744073157656433),
        im: FXR(4259360959),
    },
    FXC {
        re: FXR(2621574191),
        im: FXR(3402071844),
    },
    FXC {
        re: FXR(18446744070307479772),
        im: FXR(2621574191),
    },
    FXC {
        re: FXR(3723935269),
        im: FXR(2139871536),
    },
    FXC {
        re: FXR(18446744071569680080),
        im: FXR(3723935269),
    },
    FXC {
        re: FXR(1120102207),
        im: FXR(4146337555),
    },
    FXC {
        re: FXR(18446744069563214061),
        im: FXR(1120102207),
    },
    FXC {
        re: FXR(4069849124),
        im: FXR(1372250773),
    },
    FXC {
        re: FXR(18446744072337300843),
        im: FXR(4069849124),
    },
    FXC {
        re: FXR(1907490086),
        im: FXR(3848145741),
    },
    FXC {
        re: FXR(18446744069861405875),
        im: FXR(1907490086),
    },
    FXC {
        re: FXR(3234912670),
        im: FXR(2825258235),
    },
    FXC {
        re: FXR(18446744070884293381),
        im: FXR(3234912670),
    },
    FXC {
        re: FXR(289669429),
        im: FXR(4285187942),
    },
    FXC {
        re: FXR(18446744069424363674),
        im: FXR(289669429),
    },
    FXC {
        re: FXR(4281310585),
        im: FXR(342233465),
    },
    FXC {
        re: FXR(18446744073367318151),
        im: FXR(4281310585),
    },
    FXC {
        re: FXR(2785348143),
        im: FXR(3269339351),
    },
    FXC {
        re: FXR(18446744070440212265),
        im: FXR(2785348143),
    },
    FXC {
        re: FXR(3824448145),
        im: FXR(1954569124),
    },
    FXC {
        re: FXR(18446744071754982492),
        im: FXR(3824448145),
    },
    FXC {
        re: FXR(1322204136),
        im: FXR(4086382299),
    },
    FXC {
        re: FXR(18446744069623169317),
        im: FXR(1322204136),
    },
    FXC {
        re: FXR(4132279966),
        im: FXR(1170899806),
    },
    FXC {
        re: FXR(18446744072538651810),
        im: FXR(4132279966),
    },
    FXC {
        re: FXR(2094011993),
        im: FXR(3749914379),
    },
    FXC {
        re: FXR(18446744069959637237),
        im: FXR(2094011993),
    },
    FXC {
        re: FXR(3369644927),
        im: FXR(2663125446),
    },
    FXC {
        re: FXR(18446744071046426170),
        im: FXR(3369644927),
    },
    FXC {
        re: FXR(499584716),
        im: FXR(4265812840),
    },
    FXC {
        re: FXR(18446744069443738776),
        im: FXR(499584716),
    },
    FXC {
        re: FXR(4227150159),
        im: FXR(760227338),
    },
    FXC {
        re: FXR(18446744072949324278),
        im: FXR(4227150159),
    },
    FXC {
        re: FXR(2451484637),
        im: FXR(3526608449),
    },
    FXC {
        re: FXR(18446744070182943167),
        im: FXR(2451484637),
    },
    FXC {
        re: FXR(3614451106),
        im: FXR(2320018810),
    },
    FXC {
        re: FXR(18446744071389532806),
        im: FXR(3614451106),
    },
    FXC {
        re: FXR(915301854),
        im: FXR(4196303920),
    },
    FXC {
        re: FXR(18446744069513247696),
        im: FXR(915301854),
    },
    FXC {
        re: FXR(3997613658),
        im: FXR(1570295869),
    },
    FXC {
        re: FXR(18446744072139255747),
        im: FXR(3997613658),
    },
    FXC {
        re: FXR(1716372869),
        im: FXR(3937106583),
    },
    FXC {
        re: FXR(18446744069772445033),
        im: FXR(1716372869),
    },
    FXC {
        re: FXR(3092387225),
        im: FXR(2980584729),
    },
    FXC {
        re: FXR(18446744070728966887),
        im: FXR(3092387225),
    },
    FXC {
        re: FXR(79056303),
        im: FXR(4294239650),
    },
    FXC {
        re: FXR(18446744069415311966),
        im: FXR(79056303),
    },
    FXC {
        re: FXR(4294239650),
        im: FXR(79056303),
    },
    FXC {
        re: FXR(18446744073630495313),
        im: FXR(4294239650),
    },
    FXC {
        re: FXR(2980584729),
        im: FXR(3092387225),
    },
    FXC {
        re: FXR(18446744070617164391),
        im: FXR(2980584729),
    },
    FXC {
        re: FXR(3937106583),
        im: FXR(1716372869),
    },
    FXC {
        re: FXR(18446744071993178747),
        im: FXR(3937106583),
    },
    FXC {
        re: FXR(1570295869),
        im: FXR(3997613658),
    },
    FXC {
        re: FXR(18446744069711937958),
        im: FXR(1570295869),
    },
    FXC {
        re: FXR(4196303920),
        im: FXR(915301854),
    },
    FXC {
        re: FXR(18446744072794249762),
        im: FXR(4196303920),
    },
    FXC {
        re: FXR(2320018810),
        im: FXR(3614451106),
    },
    FXC {
        re: FXR(18446744070095100510),
        im: FXR(2320018810),
    },
    FXC {
        re: FXR(3526608449),
        im: FXR(2451484637),
    },
    FXC {
        re: FXR(18446744071258066979),
        im: FXR(3526608449),
    },
    FXC {
        re: FXR(760227338),
        im: FXR(4227150159),
    },
    FXC {
        re: FXR(18446744069482401457),
        im: FXR(760227338),
    },
    FXC {
        re: FXR(4265812840),
        im: FXR(499584716),
    },
    FXC {
        re: FXR(18446744073209966900),
        im: FXR(4265812840),
    },
    FXC {
        re: FXR(2663125446),
        im: FXR(3369644927),
    },
    FXC {
        re: FXR(18446744070339906689),
        im: FXR(2663125446),
    },
    FXC {
        re: FXR(3749914379),
        im: FXR(2094011993),
    },
    FXC {
        re: FXR(18446744071615539623),
        im: FXR(3749914379),
    },
    FXC {
        re: FXR(1170899806),
        im: FXR(4132279966),
    },
    FXC {
        re: FXR(18446744069577271650),
        im: FXR(1170899806),
    },
    FXC {
        re: FXR(4086382299),
        im: FXR(1322204136),
    },
    FXC {
        re: FXR(18446744072387347480),
        im: FXR(4086382299),
    },
    FXC {
        re: FXR(1954569124),
        im: FXR(3824448145),
    },
    FXC {
        re: FXR(18446744069885103471),
        im: FXR(1954569124),
    },
    FXC {
        re: FXR(3269339351),
        im: FXR(2785348143),
    },
    FXC {
        re: FXR(18446744070924203473),
        im: FXR(3269339351),
    },
    FXC {
        re: FXR(342233465),
        im: FXR(4281310585),
    },
    FXC {
        re: FXR(18446744069428241031),
        im: FXR(342233465),
    },
    FXC {
        re: FXR(4285187942),
        im: FXR(289669429),
    },
    FXC {
        re: FXR(18446744073419882187),
        im: FXR(4285187942),
    },
    FXC {
        re: FXR(2825258235),
        im: FXR(3234912670),
    },
    FXC {
        re: FXR(18446744070474638946),
        im: FXR(2825258235),
    },
    FXC {
        re: FXR(3848145741),
        im: FXR(1907490086),
    },
    FXC {
        re: FXR(18446744071802061530),
        im: FXR(3848145741),
    },
    FXC {
        re: FXR(1372250773),
        im: FXR(4069849124),
    },
    FXC {
        re: FXR(18446744069639702492),
        im: FXR(1372250773),
    },
    FXC {
        re: FXR(4146337555),
        im: FXR(1120102207),
    },
    FXC {
        re: FXR(18446744072589449409),
        im: FXR(4146337555),
    },
    FXC {
        re: FXR(2139871536),
        im: FXR(3723935269),
    },
    FXC {
        re: FXR(18446744069985616347),
        im: FXR(2139871536),
    },
    FXC {
        re: FXR(3402071844),
        im: FXR(2621574191),
    },
    FXC {
        re: FXR(18446744071087977425),
        im: FXR(3402071844),
    },
    FXC {
        re: FXR(551895183),
        im: FXR(4259360959),
    },
    FXC {
        re: FXR(18446744069450190657),
        im: FXR(551895183),
    },
    FXC {
        re: FXR(4236161021),
        im: FXR(708296459),
    },
    FXC {
        re: FXR(18446744073001255157),
        im: FXR(4236161021),
    },
    FXC {
        re: FXR(2494576955),
        im: FXR(3496259414),
    },
    FXC {
        re: FXR(18446744070213292202),
        im: FXR(2494576955),
    },
    FXC {
        re: FXR(3642649144),
        im: FXR(2275489241),
    },
    FXC {
        re: FXR(18446744071434062375),
        im: FXR(3642649144),
    },
    FXC {
        re: FXR(966728038),
        im: FXR(4184755784),
    },
    FXC {
        re: FXR(18446744069524795832),
        im: FXR(966728038),
    },
    FXC {
        re: FXR(4016582591),
        im: FXR(1521120759),
    },
    FXC {
        re: FXR(18446744072188430857),
        im: FXR(4016582591),
    },
    FXC {
        re: FXR(1764557983),
        im: FXR(3915747591),
    },
    FXC {
        re: FXR(18446744069793804025),
        im: FXR(1764557983),
    },
    FXC {
        re: FXR(3128730733),
        im: FXR(2942411948),
    },
    FXC {
        re: FXR(18446744070767139668),
        im: FXR(3128730733),
    },
    FXC {
        re: FXR(131747276),
        im: FXR(4292946160),
    },
    FXC {
        re: FXR(18446744069416605456),
        im: FXR(131747276),
    },
    FXC {
        re: FXR(4291006167),
        im: FXR(184418409),
    },
    FXC {
        re: FXR(18446744073525133207),
        im: FXR(4291006167),
    },
    FXC {
        re: FXR(2903796051),
        im: FXR(3164603066),
    },
    FXC {
        re: FXR(18446744070544948550),
        im: FXR(2903796051),
    },
    FXC {
        re: FXR(3893798902),
        im: FXR(1812477362),
    },
    FXC {
        re: FXR(18446744071897074254),
        im: FXR(3893798902),
    },
    FXC {
        re: FXR(1471716574),
        im: FXR(4034946641),
    },
    FXC {
        re: FXR(18446744069674604975),
        im: FXR(1471716574),
    },
    FXC {
        re: FXR(4172577440),
        im: FXR(1018008636),
    },
    FXC {
        re: FXR(18446744072691542980),
        im: FXR(4172577440),
    },
    FXC {
        re: FXR(2230616993),
        im: FXR(3670298613),
    },
    FXC {
        re: FXR(18446744070039253003),
        im: FXR(2230616993),
    },
    FXC {
        re: FXR(3465383855),
        im: FXR(2537293599),
    },
    FXC {
        re: FXR(18446744071172258017),
        im: FXR(3465383855),
    },
    FXC {
        re: FXR(656258914),
        im: FXR(4244533933),
    },
    FXC {
        re: FXR(18446744069465017683),
        im: FXR(656258914),
    },
    FXC {
        re: FXR(4252267634),
        im: FXR(604122538),
    },
    FXC {
        re: FXR(18446744073105429078),
        im: FXR(4252267634),
    },
    FXC {
        re: FXR(2579628136),
        im: FXR(3433986423),
    },
    FXC {
        re: FXR(18446744070275565193),
        im: FXR(2579628136),
    },
    FXC {
        re: FXR(3697395348),
        im: FXR(2185408821),
    },
    FXC {
        re: FXR(18446744071524142795),
        im: FXR(3697395348),
    },
    FXC {
        re: FXR(1069135926),
        im: FXR(4159770720),
    },
    FXC {
        re: FXR(18446744069549780896),
        im: FXR(1069135926),
    },
    FXC {
        re: FXR(4052703044),
        im: FXR(1422090755),
    },
    FXC {
        re: FXR(18446744072287460861),
        im: FXR(4052703044),
    },
    FXC {
        re: FXR(1860123788),
        im: FXR(3871263820),
    },
    FXC {
        re: FXR(18446744069838287796),
        im: FXR(1860123788),
    },
    FXC {
        re: FXR(3199998822),
        im: FXR(2864742853),
    },
    FXC {
        re: FXR(18446744070844808763),
        im: FXR(3199998822),
    },
    FXC {
        re: FXR(237061769),
        im: FXR(4288419964),
    },
    FXC {
        re: FXR(18446744069421131652),
        im: FXR(237061769),
    },
    FXC {
        re: FXR(4276788480),
        im: FXR(394745962),
    },
    FXC {
        re: FXR(18446744073314805654),
        im: FXR(4276788480),
    },
    FXC {
        re: FXR(2745018589),
        im: FXR(3303273682),
    },
    FXC {
        re: FXR(18446744070406277934),
        im: FXR(2745018589),
    },
    FXC {
        re: FXR(3800174601),
        im: FXR(2001353810),
    },
    FXC {
        re: FXR(18446744071708197806),
        im: FXR(3800174601),
    },
    FXC {
        re: FXR(1271958380),
        im: FXR(4102300081),
    },
    FXC {
        re: FXR(18446744069607251535),
        im: FXR(1271958380),
    },
    FXC {
        re: FXR(4117600071),
        im: FXR(1221521071),
    },
    FXC {
        re: FXR(18446744072488030545),
        im: FXR(4117600071),
    },
    FXC {
        re: FXR(2047837100),
        im: FXR(3775328765),
    },
    FXC {
        re: FXR(18446744069934222851),
        im: FXR(2047837100),
    },
    FXC {
        re: FXR(3336710553),
        im: FXR(2704275644),
    },
    FXC {
        re: FXR(18446744071005275972),
        im: FXR(3336710553),
    },
    FXC {
        re: FXR(447199012),
        im: FXR(4271622305),
    },
    FXC {
        re: FXR(18446744069437929311),
        im: FXR(447199012),
    },
    FXC {
        re: FXR(4217502704),
        im: FXR(812043729),
    },
    FXC {
        re: FXR(18446744072897507887),
        im: FXR(4217502704),
    },
    FXC {
        re: FXR(2408023134),
        im: FXR(3556426389),
    },
    FXC {
        re: FXR(18446744070153125227),
        im: FXR(2408023134),
    },
    FXC {
        re: FXR(3585708745),
        im: FXR(2364198992),
    },
    FXC {
        re: FXR(18446744071345352624),
        im: FXR(3585708745),
    },
    FXC {
        re: FXR(863737830),
        im: FXR(4207220108),
    },
    FXC {
        re: FXR(18446744069502331508),
        im: FXR(863737830),
    },
    FXC {
        re: FXR(3978042699),
        im: FXR(1619234497),
    },
    FXC {
        re: FXR(18446744072090317119),
        im: FXR(3978042699),
    },
    FXC {
        re: FXR(1667929275),
        im: FXR(3957872662),
    },
    FXC {
        re: FXR(18446744069751678954),
        im: FXR(1667929275),
    },
    FXC {
        re: FXR(3055578014),
        im: FXR(3018308645),
    },
    FXC {
        re: FXR(18446744070691242971),
        im: FXR(3055578014),
    },
    FXC {
        re: FXR(26353424),
        im: FXR(4294886444),
    },
    FXC {
        re: FXR(18446744069414665172),
        im: FXR(26353424),
    },
    FXC {
        re: FXR(4294947083),
        im: FXR(13176774),
    },
    FXC {
        re: FXR(18446744073696374842),
        im: FXR(4294947083),
    },
    FXC {
        re: FXR(3027668821),
        im: FXR(3046303593),
    },
    FXC {
        re: FXR(18446744070663248023),
        im: FXR(3027668821),
    },
    FXC {
        re: FXR(3962971170),
        im: FXR(1655778843),
    },
    FXC {
        re: FXR(18446744072053772773),
        im: FXR(3962971170),
    },
    FXC {
        re: FXR(1631431340),
        im: FXR(3973056236),
    },
    FXC {
        re: FXR(18446744069736495380),
        im: FXR(1631431340),
    },
    FXC {
        re: FXR(4209850218),
        im: FXR(850826195),
    },
    FXC {
        re: FXR(18446744072858725421),
        im: FXR(4209850218),
    },
    FXC {
        re: FXR(2375188665),
        im: FXR(3578438609),
    },
    FXC {
        re: FXR(18446744070131113007),
        im: FXR(2375188665),
    },
    FXC {
        re: FXR(3563797363),
        im: FXR(2397100839),
    },
    FXC {
        re: FXR(18446744071312450777),
        im: FXR(3563797363),
    },
    FXC {
        re: FXR(824979024),
        im: FXR(4214991540),
    },
    FXC {
        re: FXR(18446744069494560076),
        im: FXR(824979024),
    },
    FXC {
        re: FXR(4272974189),
        im: FXR(434091755),
    },
    FXC {
        re: FXR(18446744073275459861),
        im: FXR(4272974189),
    },
    FXC {
        re: FXR(2714499801),
        im: FXR(3328398249),
    },
    FXC {
        re: FXR(18446744070381153367),
        im: FXR(2714499801),
    },
    FXC {
        re: FXR(3781593674),
        im: FXR(2036244917),
    },
    FXC {
        re: FXR(18446744071673306699),
        im: FXR(3781593674),
    },
    FXC {
        re: FXR(1234147941),
        im: FXR(4113833119),
    },
    FXC {
        re: FXR(18446744069595718497),
        im: FXR(1234147941),
    },
    FXC {
        re: FXR(4106183088),
        im: FXR(1259366714),
    },
    FXC {
        re: FXR(18446744072450184902),
        im: FXR(4106183088),
    },
    FXC {
        re: FXR(2013003163),
        im: FXR(3794016650),
    },
    FXC {
        re: FXR(18446744069915534966),
        im: FXR(2013003163),
    },
    FXC {
        re: FXR(3311679735),
        im: FXR(2734871369),
    },
    FXC {
        re: FXR(18446744070974680247),
        im: FXR(3311679735),
    },
    FXC {
        re: FXR(407865107),
        im: FXR(4275557289),
    },
    FXC {
        re: FXR(18446744069433994327),
        im: FXR(407865107),
    },
    FXC {
        re: FXR(4289127078),
        im: FXR(223903967),
    },
    FXC {
        re: FXR(18446744073485647649),
        im: FXR(4289127078),
    },
    FXC {
        re: FXR(2874546829),
        im: FXR(3191194855),
    },
    FXC {
        re: FXR(18446744070518356761),
        im: FXR(2874546829),
    },
    FXC {
        re: FXR(3876952381),
        im: FXR(1848238164),
    },
    FXC {
        re: FXR(18446744071861313452),
        im: FXR(3876952381),
    },
    FXC {
        re: FXR(1434517580),
        im: FXR(4048321058),
    },
    FXC {
        re: FXR(18446744069661230558),
        im: FXR(1434517580),
    },
    FXC {
        re: FXR(4163031206),
        im: FXR(1056368897),
    },
    FXC {
        re: FXR(18446744072653182719),
        im: FXR(4163031206),
    },
    FXC {
        re: FXR(2196741986),
        im: FXR(3690673207),
    },
    FXC {
        re: FXR(18446744070018878409),
        im: FXR(2196741986),
    },
    FXC {
        re: FXR(3441884449),
        im: FXR(2569080674),
    },
    FXC {
        re: FXR(18446744071140470942),
        im: FXR(3441884449),
    },
    FXC {
        re: FXR(617165468),
        im: FXR(4250394200),
    },
    FXC {
        re: FXR(18446744069459157416),
        im: FXR(617165468),
    },
    FXC {
        re: FXR(4246527332),
        im: FXR(643233779),
    },
    FXC {
        re: FXR(18446744073066317837),
        im: FXR(4246527332),
    },
    FXC {
        re: FXR(2547913306),
        im: FXR(3457583240),
    },
    FXC {
        re: FXR(18446744070251968376),
        im: FXR(2547913306),
    },
    FXC {
        re: FXR(3677124776),
        im: FXR(2219346178),
    },
    FXC {
        re: FXR(18446744071490205438),
        im: FXR(3677124776),
    },
    FXC {
        re: FXR(1030805132),
        im: FXR(4169434596),
    },
    FXC {
        re: FXR(18446744069540117020),
        im: FXR(1030805132),
    },
    FXC {
        re: FXR(4039442815),
        im: FXR(1459330606),
    },
    FXC {
        re: FXR(18446744072250221010),
        im: FXR(4039442815),
    },
    FXC {
        re: FXR(1824414839),
        im: FXR(3888219974),
    },
    FXC {
        re: FXR(18446744069821331642),
        im: FXR(1824414839),
    },
    FXC {
        re: FXR(3173496894),
        im: FXR(2894073520),
    },
    FXC {
        re: FXR(18446744070815478096),
        im: FXR(3173496894),
    },
    FXC {
        re: FXR(197582163),
        im: FXR(4290420185),
    },
    FXC {
        re: FXR(18446744069419131431),
        im: FXR(197582163),
    },
    FXC {
        re: FXR(4293330151),
        im: FXR(118576083),
    },
    FXC {
        re: FXR(18446744073590975533),
        im: FXR(4293330151),
    },
    FXC {
        re: FXR(2951996911),
        im: FXR(3119688816),
    },
    FXC {
        re: FXR(18446744070589862800),
        im: FXR(2951996911),
    },
    FXC {
        re: FXR(3921142750),
        im: FXR(1752536335),
    },
    FXC {
        re: FXR(18446744071957015281),
        im: FXR(3921142750),
    },
    FXC {
        re: FXR(1533436302),
        im: FXR(4011896955),
    },
    FXC {
        re: FXR(18446744069697654661),
        im: FXR(1533436302),
    },
    FXC {
        re: FXR(4187701970),
        im: FXR(953884839),
    },
    FXC {
        re: FXR(18446744072755666777),
        im: FXR(4187701970),
    },
    FXC {
        re: FXR(2286654023),
        im: FXR(3635650898),
    },
    FXC {
        re: FXR(18446744070073900718),
        im: FXR(2286654023),
    },
    FXC {
        re: FXR(3503896214),
        im: FXR(2483838842),
    },
    FXC {
        re: FXR(18446744071225712774),
        im: FXR(3503896214),
    },
    FXC {
        re: FXR(721289485),
        im: FXR(4233968062),
    },
    FXC {
        re: FXR(18446744069475583554),
        im: FXR(721289485),
    },
    FXC {
        re: FXR(4261034104),
        im: FXR(538825051),
    },
    FXC {
        re: FXR(18446744073170726565),
        im: FXR(4261034104),
    },
    FXC {
        re: FXR(2631999263),
        im: FXR(3394012957),
    },
    FXC {
        re: FXR(18446744070315538659),
        im: FXR(2631999263),
    },
    FXC {
        re: FXR(3730482776),
        im: FXR(2128436593),
    },
    FXC {
        re: FXR(18446744071581115023),
        im: FXR(3730482776),
    },
    FXC {
        re: FXR(1132817720),
        im: FXR(4142881616),
    },
    FXC {
        re: FXR(18446744069566670000),
        im: FXR(1132817720),
    },
    FXC {
        re: FXR(4074039976),
        im: FXR(1359758194),
    },
    FXC {
        re: FXR(18446744072349793422),
        im: FXR(4074039976),
    },
    FXC {
        re: FXR(1919287054),
        im: FXR(3842275534),
    },
    FXC {
        re: FXR(18446744069867276082),
        im: FXR(1919287054),
    },
    FXC {
        re: FXR(3243565216),
        im: FXR(2815320366),
    },
    FXC {
        re: FXR(18446744070894231250),
        im: FXR(3243565216),
    },
    FXC {
        re: FXR(302814837),
        im: FXR(4284279082),
    },
    FXC {
        re: FXR(18446744069425272534),
        im: FXR(302814837),
    },
    FXC {
        re: FXR(4282340394),
        im: FXR(329096979),
    },
    FXC {
        re: FXR(18446744073380454637),
        im: FXR(4282340394),
    },
    FXC {
        re: FXR(2795365227),
        im: FXR(3260778637),
    },
    FXC {
        re: FXR(18446744070448772979),
        im: FXR(2795365227),
    },
    FXC {
        re: FXR(3830426680),
        im: FXR(1942826684),
    },
    FXC {
        re: FXR(18446744071766724932),
        im: FXR(3830426680),
    },
    FXC {
        re: FXR(1334734758),
        im: FXR(4082306603),
    },
    FXC {
        re: FXR(18446744069627245013),
        im: FXR(1334734758),
    },
    FXC {
        re: FXR(4135852789),
        im: FXR(1158216639),
    },
    FXC {
        re: FXR(18446744072551334977),
        im: FXR(4135852789),
    },
    FXC {
        re: FXR(2105506713),
        im: FXR(3743472393),
    },
    FXC {
        re: FXR(18446744069966079223),
        im: FXR(2105506713),
    },
    FXC {
        re: FXR(3377799422),
        im: FXR(2652774988),
    },
    FXC {
        re: FXR(18446744071056776628),
        im: FXR(3377799422),
    },
    FXC {
        re: FXR(512669694),
        im: FXR(4264260060),
    },
    FXC {
        re: FXR(18446744069445291556),
        im: FXR(512669694),
    },
    FXC {
        re: FXR(4229462610),
        im: FXR(747255046),
    },
    FXC {
        re: FXR(18446744072962296570),
        im: FXR(4229462610),
    },
    FXC {
        re: FXR(2462292582),
        im: FXR(3519070803),
    },
    FXC {
        re: FXR(18446744070190480813),
        im: FXR(2462292582),
    },
    FXC {
        re: FXR(3621551813),
        im: FXR(2308918911),
    },
    FXC {
        re: FXR(18446744071400632705),
        im: FXR(3621551813),
    },
    FXC {
        re: FXR(928171626),
        im: FXR(4193476065),
    },
    FXC {
        re: FXR(18446744069516075551),
        im: FXR(928171626),
    },
    FXC {
        re: FXR(4002412444),
        im: FXR(1558023973),
    },
    FXC {
        re: FXR(18446744072151527643),
        im: FXR(4002412444),
    },
    FXC {
        re: FXR(1728443664),
        im: FXR(3931822297),
    },
    FXC {
        re: FXR(18446744069777729319),
        im: FXR(1728443664),
    },
    FXC {
        re: FXR(3101516976),
        im: FXR(2971083391),
    },
    FXC {
        re: FXR(18446744070738468225),
        im: FXR(3101516976),
    },
    FXC {
        re: FXR(92230472),
        im: FXR(4293976900),
    },
    FXC {
        re: FXR(18446744069415574716),
        im: FXR(92230472),
    },
    FXC {
        re: FXR(4294461982),
        im: FXR(65881389),
    },
    FXC {
        re: FXR(18446744073643670227),
        im: FXR(4294461982),
    },
    FXC {
        re: FXR(2990058012),
        im: FXR(3083228366),
    },
    FXC {
        re: FXR(18446744070626323250),
        im: FXR(2990058012),
    },
    FXC {
        re: FXR(3942353812),
        im: FXR(1704285919),
    },
    FXC {
        re: FXR(18446744072005265697),
        im: FXR(3942353812),
    },
    FXC {
        re: FXR(1582552984),
        im: FXR(3992777245),
    },
    FXC {
        re: FXR(18446744069716774371),
        im: FXR(1582552984),
    },
    FXC {
        re: FXR(4199092278),
        im: FXR(902423468),
    },
    FXC {
        re: FXR(18446744072807128148),
        im: FXR(4199092278),
    },
    FXC {
        re: FXR(2331096871),
        im: FXR(3607316378),
    },
    FXC {
        re: FXR(18446744070102235238),
        im: FXR(2331096871),
    },
    FXC {
        re: FXR(3534112901),
        im: FXR(2440653617),
    },
    FXC {
        re: FXR(18446744071268897999),
        im: FXR(3534112901),
    },
    FXC {
        re: FXR(773192474),
        im: FXR(4224797921),
    },
    FXC {
        re: FXR(18446744069484753695),
        im: FXR(773192474),
    },
    FXC {
        re: FXR(4267325469),
        im: FXR(486495035),
    },
    FXC {
        re: FXR(18446744073223056581),
        im: FXR(4267325469),
    },
    FXC {
        re: FXR(2673450838),
        im: FXR(3361458715),
    },
    FXC {
        re: FXR(18446744070348092901),
        im: FXR(2673450838),
    },
    FXC {
        re: FXR(3756321069),
        im: FXR(2082497563),
    },
    FXC {
        re: FXR(18446744071627054053),
        im: FXR(3756321069),
    },
    FXC {
        re: FXR(1183571952),
        im: FXR(4128668249),
    },
    FXC {
        re: FXR(18446744069580883367),
        im: FXR(1183571952),
    },
    FXC {
        re: FXR(4090419533),
        im: FXR(1309661069),
    },
    FXC {
        re: FXR(18446744072399890547),
        im: FXR(4090419533),
    },
    FXC {
        re: FXR(1966293167),
        im: FXR(3818433613),
    },
    FXC {
        re: FXR(18446744069891118003),
        im: FXR(1966293167),
    },
    FXC {
        re: FXR(3277869293),
        im: FXR(2775304843),
    },
    FXC {
        re: FXR(18446744070934246773),
        im: FXR(3277869293),
    },
    FXC {
        re: FXR(355366730),
        im: FXR(4280240479),
    },
    FXC {
        re: FXR(18446744069429311137),
        im: FXR(355366730),
    },
    FXC {
        re: FXR(4286056468),
        im: FXR(276521294),
    },
    FXC {
        re: FXR(18446744073433030322),
        im: FXR(4286056468),
    },
    FXC {
        re: FXR(2835169511),
        im: FXR(3226229675),
    },
    FXC {
        re: FXR(18446744070483321941),
        im: FXR(2835169511),
    },
    FXC {
        re: FXR(3853979728),
        im: FXR(1895675165),
    },
    FXC {
        re: FXR(18446744071813876451),
        im: FXR(3853979728),
    },
    FXC {
        re: FXR(1384730436),
        im: FXR(4065619964),
    },
    FXC {
        re: FXR(18446744069643931652),
        im: FXR(1384730436),
    },
    FXC {
        re: FXR(4149754467),
        im: FXR(1107376152),
    },
    FXC {
        re: FXR(18446744072602175464),
        im: FXR(4149754467),
    },
    FXC {
        re: FXR(2151286337),
        im: FXR(3717352710),
    },
    FXC {
        re: FXR(18446744069992198906),
        im: FXR(2151286337),
    },
    FXC {
        re: FXR(3410098710),
        im: FXR(2611124444),
    },
    FXC {
        re: FXR(18446744071098427172),
        im: FXR(3410098710),
    },
    FXC {
        re: FXR(564960121),
        im: FXR(4257647723),
    },
    FXC {
        re: FXR(18446744069451903893),
        im: FXR(564960121),
    },
    FXC {
        re: FXR(4238314108),
        im: FXR(695296767),
    },
    FXC {
        re: FXR(18446744073014254849),
        im: FXR(4238314108),
    },
    FXC {
        re: FXR(2505291588),
        im: FXR(3488589706),
    },
    FXC {
        re: FXR(18446744070220961910),
        im: FXR(2505291588),
    },
    FXC {
        re: FXR(3649613104),
        im: FXR(2264303042),
    },
    FXC {
        re: FXR(18446744071445248574),
        im: FXR(3649613104),
    },
    FXC {
        re: FXR(979562138),
        im: FXR(4181770210),
    },
    FXC {
        re: FXR(18446744069527781406),
        im: FXR(979562138),
    },
    FXC {
        re: FXR(4021230421),
        im: FXR(1508790899),
    },
    FXC {
        re: FXR(18446744072200760717),
        im: FXR(4021230421),
    },
    FXC {
        re: FXR(1776563023),
        im: FXR(3910315575),
    },
    FXC {
        re: FXR(18446744069799236041),
        im: FXR(1776563023),
    },
    FXC {
        re: FXR(3137743202),
        im: FXR(2932799290),
    },
    FXC {
        re: FXR(18446744070776752326),
        im: FXR(3137743202),
    },
    FXC {
        re: FXR(144917230),
        im: FXR(4292521761),
    },
    FXC {
        re: FXR(18446744069417029855),
        im: FXR(144917230),
    },
    FXC {
        re: FXR(4291551760),
        im: FXR(171252920),
    },
    FXC {
        re: FXR(18446744073538298696),
        im: FXR(4291551760),
    },
    FXC {
        re: FXR(2913491250),
        im: FXR(3155679453),
    },
    FXC {
        re: FXR(18446744070553872163),
        im: FXR(2913491250),
    },
    FXC {
        re: FXR(3899341179),
        im: FXR(1800522825),
    },
    FXC {
        re: FXR(18446744071909028791),
        im: FXR(3899341179),
    },
    FXC {
        re: FXR(1484088690),
        im: FXR(4030412489),
    },
    FXC {
        re: FXR(18446744069679139127),
        im: FXR(1484088690),
    },
    FXC {
        re: FXR(4175681009),
        im: FXR(1005202558),
    },
    FXC {
        re: FXR(18446744072704349058),
        im: FXR(4175681009),
    },
    FXC {
        re: FXR(2241866812),
        im: FXR(3663437903),
    },
    FXC {
        re: FXR(18446744070046113713),
        im: FXR(2241866812),
    },
    FXC {
        re: FXR(3473151854),
        im: FXR(2526650010),
    },
    FXC {
        re: FXR(18446744071182901606),
        im: FXR(3473151854),
    },
    FXC {
        re: FXR(669277872),
        im: FXR(4242500584),
    },
    FXC {
        re: FXR(18446744069467051032),
        im: FXR(669277872),
    },
    FXC {
        re: FXR(4254101044),
        im: FXR(591073921),
    },
    FXC {
        re: FXR(18446744073118477695),
        im: FXR(4254101044),
    },
    FXC {
        re: FXR(2590151318),
        im: FXR(3426056074),
    },
    FXC {
        re: FXR(18446744070283495542),
        im: FXR(2590151318),
    },
    FXC {
        re: FXR(3704082687),
        im: FXR(2174055087),
    },
    FXC {
        re: FXR(18446744071535496529),
        im: FXR(3704082687),
    },
    FXC {
        re: FXR(1081892891),
        im: FXR(4156471081),
    },
    FXC {
        re: FXR(18446744069553080535),
        im: FXR(1081892891),
    },
    FXC {
        re: FXR(4057046884),
        im: FXR(1409650544),
    },
    FXC {
        re: FXR(18446744072299901072),
        im: FXR(4057046884),
    },
    FXC {
        re: FXR(1871991904),
        im: FXR(3865538822),
    },
    FXC {
        re: FXR(18446744069844012794),
        im: FXR(1871991904),
    },
    FXC {
        re: FXR(3208772670),
        im: FXR(2854911913),
    },
    FXC {
        re: FXR(18446744070854639703),
        im: FXR(3208772670),
    },
    FXC {
        re: FXR(250217341),
        im: FXR(4287672487),
    },
    FXC {
        re: FXR(18446744069421879129),
        im: FXR(250217341),
    },
    FXC {
        re: FXR(4277979416),
        im: FXR(381623102),
    },
    FXC {
        re: FXR(18446744073327928514),
        im: FXR(4277979416),
    },
    FXC {
        re: FXR(2755139971),
        im: FXR(3294836538),
    },
    FXC {
        re: FXR(18446744070414715078),
        im: FXR(2755139971),
    },
    FXC {
        re: FXR(3806296784),
        im: FXR(1989685620),
    },
    FXC {
        re: FXR(18446744071719865996),
        im: FXR(3806296784),
    },
    FXC {
        re: FXR(1284538073),
        im: FXR(4098378461),
    },
    FXC {
        re: FXR(18446744069611173155),
        im: FXR(1284538073),
    },
    FXC {
        re: FXR(4121328267),
        im: FXR(1208882703),
    },
    FXC {
        re: FXR(18446744072500668913),
        im: FXR(4121328267),
    },
    FXC {
        re: FXR(2059410008),
        im: FXR(3769028322),
    },
    FXC {
        re: FXR(18446744069940523294),
        im: FXR(2059410008),
    },
    FXC {
        re: FXR(3344991450),
        im: FXR(2694026034),
    },
    FXC {
        re: FXR(18446744071015525582),
        im: FXR(3344991450),
    },
    FXC {
        re: FXR(460302060),
        im: FXR(4270230215),
    },
    FXC {
        re: FXR(18446744069439321401),
        im: FXR(460302060),
    },
    FXC {
        re: FXR(4219974170),
        im: FXR(799100792),
    },
    FXC {
        re: FXR(18446744072910450824),
        im: FXR(4219974170),
    },
    FXC {
        re: FXR(2418922764),
        im: FXR(3549021941),
    },
    FXC {
        re: FXR(18446744070160529675),
        im: FXR(2418922764),
    },
    FXC {
        re: FXR(3592945130),
        im: FXR(2353187066),
    },
    FXC {
        re: FXR(18446744071356364550),
        im: FXR(3592945130),
    },
    FXC {
        re: FXR(876641334),
        im: FXR(4204550397),
    },
    FXC {
        re: FXR(18446744069505001219),
        im: FXR(876641334),
    },
    FXC {
        re: FXR(3982991719),
        im: FXR(1607022414),
    },
    FXC {
        re: FXR(18446744072102529202),
        im: FXR(3982991719),
    },
    FXC {
        re: FXR(1680064008),
        im: FXR(3952736900),
    },
    FXC {
        re: FXR(18446744069756814716),
        im: FXR(1680064008),
    },
    FXC {
        re: FXR(3064823674),
        im: FXR(3008920059),
    },
    FXC {
        re: FXR(18446744070700631557),
        im: FXR(3064823674),
    },
    FXC {
        re: FXR(39529826),
        im: FXR(4294785381),
    },
    FXC {
        re: FXR(18446744069414766235),
        im: FXR(39529826),
    },
    FXC {
        re: FXR(4294785381),
        im: FXR(39529826),
    },
    FXC {
        re: FXR(18446744073670021790),
        im: FXR(4294785381),
    },
    FXC {
        re: FXR(3008920059),
        im: FXR(3064823674),
    },
    FXC {
        re: FXR(18446744070644727942),
        im: FXR(3008920059),
    },
    FXC {
        re: FXR(3952736900),
        im: FXR(1680064008),
    },
    FXC {
        re: FXR(18446744072029487608),
        im: FXR(3952736900),
    },
    FXC {
        re: FXR(1607022414),
        im: FXR(3982991719),
    },
    FXC {
        re: FXR(18446744069726559897),
        im: FXR(1607022414),
    },
    FXC {
        re: FXR(4204550397),
        im: FXR(876641334),
    },
    FXC {
        re: FXR(18446744072832910282),
        im: FXR(4204550397),
    },
    FXC {
        re: FXR(2353187066),
        im: FXR(3592945130),
    },
    FXC {
        re: FXR(18446744070116606486),
        im: FXR(2353187066),
    },
    FXC {
        re: FXR(3549021941),
        im: FXR(2418922764),
    },
    FXC {
        re: FXR(18446744071290628852),
        im: FXR(3549021941),
    },
    FXC {
        re: FXR(799100792),
        im: FXR(4219974170),
    },
    FXC {
        re: FXR(18446744069489577446),
        im: FXR(799100792),
    },
    FXC {
        re: FXR(4270230215),
        im: FXR(460302060),
    },
    FXC {
        re: FXR(18446744073249249556),
        im: FXR(4270230215),
    },
    FXC {
        re: FXR(2694026034),
        im: FXR(3344991450),
    },
    FXC {
        re: FXR(18446744070364560166),
        im: FXR(2694026034),
    },
    FXC {
        re: FXR(3769028322),
        im: FXR(2059410008),
    },
    FXC {
        re: FXR(18446744071650141608),
        im: FXR(3769028322),
    },
    FXC {
        re: FXR(1208882703),
        im: FXR(4121328267),
    },
    FXC {
        re: FXR(18446744069588223349),
        im: FXR(1208882703),
    },
    FXC {
        re: FXR(4098378461),
        im: FXR(1284538073),
    },
    FXC {
        re: FXR(18446744072425013543),
        im: FXR(4098378461),
    },
    FXC {
        re: FXR(1989685620),
        im: FXR(3806296784),
    },
    FXC {
        re: FXR(18446744069903254832),
        im: FXR(1989685620),
    },
    FXC {
        re: FXR(3294836538),
        im: FXR(2755139971),
    },
    FXC {
        re: FXR(18446744070954411645),
        im: FXR(3294836538),
    },
    FXC {
        re: FXR(381623102),
        im: FXR(4277979416),
    },
    FXC {
        re: FXR(18446744069431572200),
        im: FXR(381623102),
    },
    FXC {
        re: FXR(4287672487),
        im: FXR(250217341),
    },
    FXC {
        re: FXR(18446744073459334275),
        im: FXR(4287672487),
    },
    FXC {
        re: FXR(2854911913),
        im: FXR(3208772670),
    },
    FXC {
        re: FXR(18446744070500778946),
        im: FXR(2854911913),
    },
    FXC {
        re: FXR(3865538822),
        im: FXR(1871991904),
    },
    FXC {
        re: FXR(18446744071837559712),
        im: FXR(3865538822),
    },
    FXC {
        re: FXR(1409650544),
        im: FXR(4057046884),
    },
    FXC {
        re: FXR(18446744069652504732),
        im: FXR(1409650544),
    },
    FXC {
        re: FXR(4156471081),
        im: FXR(1081892891),
    },
    FXC {
        re: FXR(18446744072627658725),
        im: FXR(4156471081),
    },
    FXC {
        re: FXR(2174055087),
        im: FXR(3704082687),
    },
    FXC {
        re: FXR(18446744070005468929),
        im: FXR(2174055087),
    },
    FXC {
        re: FXR(3426056074),
        im: FXR(2590151318),
    },
    FXC {
        re: FXR(18446744071119400298),
        im: FXR(3426056074),
    },
    FXC {
        re: FXR(591073921),
        im: FXR(4254101044),
    },
    FXC {
        re: FXR(18446744069455450572),
        im: FXR(591073921),
    },
    FXC {
        re: FXR(4242500584),
        im: FXR(669277872),
    },
    FXC {
        re: FXR(18446744073040273744),
        im: FXR(4242500584),
    },
    FXC {
        re: FXR(2526650010),
        im: FXR(3473151854),
    },
    FXC {
        re: FXR(18446744070236399762),
        im: FXR(2526650010),
    },
    FXC {
        re: FXR(3663437903),
        im: FXR(2241866812),
    },
    FXC {
        re: FXR(18446744071467684804),
        im: FXR(3663437903),
    },
    FXC {
        re: FXR(1005202558),
        im: FXR(4175681009),
    },
    FXC {
        re: FXR(18446744069533870607),
        im: FXR(1005202558),
    },
    FXC {
        re: FXR(4030412489),
        im: FXR(1484088690),
    },
    FXC {
        re: FXR(18446744072225462926),
        im: FXR(4030412489),
    },
    FXC {
        re: FXR(1800522825),
        im: FXR(3899341179),
    },
    FXC {
        re: FXR(18446744069810210437),
        im: FXR(1800522825),
    },
    FXC {
        re: FXR(3155679453),
        im: FXR(2913491250),
    },
    FXC {
        re: FXR(18446744070796060366),
        im: FXR(3155679453),
    },
    FXC {
        re: FXR(171252920),
        im: FXR(4291551760),
    },
    FXC {
        re: FXR(18446744069417999856),
        im: FXR(171252920),
    },
    FXC {
        re: FXR(4292521761),
        im: FXR(144917230),
    },
    FXC {
        re: FXR(18446744073564634386),
        im: FXR(4292521761),
    },
    FXC {
        re: FXR(2932799290),
        im: FXR(3137743202),
    },
    FXC {
        re: FXR(18446744070571808414),
        im: FXR(2932799290),
    },
    FXC {
        re: FXR(3910315575),
        im: FXR(1776563023),
    },
    FXC {
        re: FXR(18446744071932988593),
        im: FXR(3910315575),
    },
    FXC {
        re: FXR(1508790899),
        im: FXR(4021230421),
    },
    FXC {
        re: FXR(18446744069688321195),
        im: FXR(1508790899),
    },
    FXC {
        re: FXR(4181770210),
        im: FXR(979562138),
    },
    FXC {
        re: FXR(18446744072729989478),
        im: FXR(4181770210),
    },
    FXC {
        re: FXR(2264303042),
        im: FXR(3649613104),
    },
    FXC {
        re: FXR(18446744070059938512),
        im: FXR(2264303042),
    },
    FXC {
        re: FXR(3488589706),
        im: FXR(2505291588),
    },
    FXC {
        re: FXR(18446744071204260028),
        im: FXR(3488589706),
    },
    FXC {
        re: FXR(695296767),
        im: FXR(4238314108),
    },
    FXC {
        re: FXR(18446744069471237508),
        im: FXR(695296767),
    },
    FXC {
        re: FXR(4257647723),
        im: FXR(564960121),
    },
    FXC {
        re: FXR(18446744073144591495),
        im: FXR(4257647723),
    },
    FXC {
        re: FXR(2611124444),
        im: FXR(3410098710),
    },
    FXC {
        re: FXR(18446744070299452906),
        im: FXR(2611124444),
    },
    FXC {
        re: FXR(3717352710),
        im: FXR(2151286337),
    },
    FXC {
        re: FXR(18446744071558265279),
        im: FXR(3717352710),
    },
    FXC {
        re: FXR(1107376152),
        im: FXR(4149754467),
    },
    FXC {
        re: FXR(18446744069559797149),
        im: FXR(1107376152),
    },
    FXC {
        re: FXR(4065619964),
        im: FXR(1384730436),
    },
    FXC {
        re: FXR(18446744072324821180),
        im: FXR(4065619964),
    },
    FXC {
        re: FXR(1895675165),
        im: FXR(3853979728),
    },
    FXC {
        re: FXR(18446744069855571888),
        im: FXR(1895675165),
    },
    FXC {
        re: FXR(3226229675),
        im: FXR(2835169511),
    },
    FXC {
        re: FXR(18446744070874382105),
        im: FXR(3226229675),
    },
    FXC {
        re: FXR(276521294),
        im: FXR(4286056468),
    },
    FXC {
        re: FXR(18446744069423495148),
        im: FXR(276521294),
    },
    FXC {
        re: FXR(4280240479),
        im: FXR(355366730),
    },
    FXC {
        re: FXR(18446744073354184886),
        im: FXR(4280240479),
    },
    FXC {
        re: FXR(2775304843),
        im: FXR(3277869293),
    },
    FXC {
        re: FXR(18446744070431682323),
        im: FXR(2775304843),
    },
    FXC {
        re: FXR(3818433613),
        im: FXR(1966293167),
    },
    FXC {
        re: FXR(18446744071743258449),
        im: FXR(3818433613),
    },
    FXC {
        re: FXR(1309661069),
        im: FXR(4090419533),
    },
    FXC {
        re: FXR(18446744069619132083),
        im: FXR(1309661069),
    },
    FXC {
        re: FXR(4128668249),
        im: FXR(1183571952),
    },
    FXC {
        re: FXR(18446744072525979664),
        im: FXR(4128668249),
    },
    FXC {
        re: FXR(2082497563),
        im: FXR(3756321069),
    },
    FXC {
        re: FXR(18446744069953230547),
        im: FXR(2082497563),
    },
    FXC {
        re: FXR(3361458715),
        im: FXR(2673450838),
    },
    FXC {
        re: FXR(18446744071036100778),
        im: FXR(3361458715),
    },
    FXC {
        re: FXR(486495035),
        im: FXR(4267325469),
    },
    FXC {
        re: FXR(18446744069442226147),
        im: FXR(486495035),
    },
    FXC {
        re: FXR(4224797921),
        im: FXR(773192474),
    },
    FXC {
        re: FXR(18446744072936359142),
        im: FXR(4224797921),
    },
    FXC {
        re: FXR(2440653617),
        im: FXR(3534112901),
    },
    FXC {
        re: FXR(18446744070175438715),
        im: FXR(2440653617),
    },
    FXC {
        re: FXR(3607316378),
        im: FXR(2331096871),
    },
    FXC {
        re: FXR(18446744071378454745),
        im: FXR(3607316378),
    },
    FXC {
        re: FXR(902423468),
        im: FXR(4199092278),
    },
    FXC {
        re: FXR(18446744069510459338),
        im: FXR(902423468),
    },
    FXC {
        re: FXR(3992777245),
        im: FXR(1582552984),
    },
    FXC {
        re: FXR(18446744072126998632),
        im: FXR(3992777245),
    },
    FXC {
        re: FXR(1704285919),
        im: FXR(3942353812),
    },
    FXC {
        re: FXR(18446744069767197804),
        im: FXR(1704285919),
    },
    FXC {
        re: FXR(3083228366),
        im: FXR(2990058012),
    },
    FXC {
        re: FXR(18446744070719493604),
        im: FXR(3083228366),
    },
    FXC {
        re: FXR(65881389),
        im: FXR(4294461982),
    },
    FXC {
        re: FXR(18446744069415089634),
        im: FXR(65881389),
    },
    FXC {
        re: FXR(4293976900),
        im: FXR(92230472),
    },
    FXC {
        re: FXR(18446744073617321144),
        im: FXR(4293976900),
    },
    FXC {
        re: FXR(2971083391),
        im: FXR(3101516976),
    },
    FXC {
        re: FXR(18446744070608034640),
        im: FXR(2971083391),
    },
    FXC {
        re: FXR(3931822297),
        im: FXR(1728443664),
    },
    FXC {
        re: FXR(18446744071981107952),
        im: FXR(3931822297),
    },
    FXC {
        re: FXR(1558023973),
        im: FXR(4002412444),
    },
    FXC {
        re: FXR(18446744069707139172),
        im: FXR(1558023973),
    },
    FXC {
        re: FXR(4193476065),
        im: FXR(928171626),
    },
    FXC {
        re: FXR(18446744072781379990),
        im: FXR(4193476065),
    },
    FXC {
        re: FXR(2308918911),
        im: FXR(3621551813),
    },
    FXC {
        re: FXR(18446744070087999803),
        im: FXR(2308918911),
    },
    FXC {
        re: FXR(3519070803),
        im: FXR(2462292582),
    },
    FXC {
        re: FXR(18446744071247259034),
        im: FXR(3519070803),
    },
    FXC {
        re: FXR(747255046),
        im: FXR(4229462610),
    },
    FXC {
        re: FXR(18446744069480089006),
        im: FXR(747255046),
    },
    FXC {
        re: FXR(4264260060),
        im: FXR(512669694),
    },
    FXC {
        re: FXR(18446744073196881922),
        im: FXR(4264260060),
    },
    FXC {
        re: FXR(2652774988),
        im: FXR(3377799422),
    },
    FXC {
        re: FXR(18446744070331752194),
        im: FXR(2652774988),
    },
    FXC {
        re: FXR(3743472393),
        im: FXR(2105506713),
    },
    FXC {
        re: FXR(18446744071604044903),
        im: FXR(3743472393),
    },
    FXC {
        re: FXR(1158216639),
        im: FXR(4135852789),
    },
    FXC {
        re: FXR(18446744069573698827),
        im: FXR(1158216639),
    },
    FXC {
        re: FXR(4082306603),
        im: FXR(1334734758),
    },
    FXC {
        re: FXR(18446744072374816858),
        im: FXR(4082306603),
    },
    FXC {
        re: FXR(1942826684),
        im: FXR(3830426680),
    },
    FXC {
        re: FXR(18446744069879124936),
        im: FXR(1942826684),
    },
    FXC {
        re: FXR(3260778637),
        im: FXR(2795365227),
    },
    FXC {
        re: FXR(18446744070914186389),
        im: FXR(3260778637),
    },
    FXC {
        re: FXR(329096979),
        im: FXR(4282340394),
    },
    FXC {
        re: FXR(18446744069427211222),
        im: FXR(329096979),
    },
    FXC {
        re: FXR(4284279082),
        im: FXR(302814837),
    },
    FXC {
        re: FXR(18446744073406736779),
        im: FXR(4284279082),
    },
    FXC {
        re: FXR(2815320366),
        im: FXR(3243565216),
    },
    FXC {
        re: FXR(18446744070465986400),
        im: FXR(2815320366),
    },
    FXC {
        re: FXR(3842275534),
        im: FXR(1919287054),
    },
    FXC {
        re: FXR(18446744071790264562),
        im: FXR(3842275534),
    },
    FXC {
        re: FXR(1359758194),
        im: FXR(4074039976),
    },
    FXC {
        re: FXR(18446744069635511640),
        im: FXR(1359758194),
    },
    FXC {
        re: FXR(4142881616),
        im: FXR(1132817720),
    },
    FXC {
        re: FXR(18446744072576733896),
        im: FXR(4142881616),
    },
    FXC {
        re: FXR(2128436593),
        im: FXR(3730482776),
    },
    FXC {
        re: FXR(18446744069979068840),
        im: FXR(2128436593),
    },
    FXC {
        re: FXR(3394012957),
        im: FXR(2631999263),
    },
    FXC {
        re: FXR(18446744071077552353),
        im: FXR(3394012957),
    },
    FXC {
        re: FXR(538825051),
        im: FXR(4261034104),
    },
    FXC {
        re: FXR(18446744069448517512),
        im: FXR(538825051),
    },
    FXC {
        re: FXR(4233968062),
        im: FXR(721289485),
    },
    FXC {
        re: FXR(18446744072988262131),
        im: FXR(4233968062),
    },
    FXC {
        re: FXR(2483838842),
        im: FXR(3503896214),
    },
    FXC {
        re: FXR(18446744070205655402),
        im: FXR(2483838842),
    },
    FXC {
        re: FXR(3635650898),
        im: FXR(2286654023),
    },
    FXC {
        re: FXR(18446744071422897593),
        im: FXR(3635650898),
    },
    FXC {
        re: FXR(953884839),
        im: FXR(4187701970),
    },
    FXC {
        re: FXR(18446744069521849646),
        im: FXR(953884839),
    },
    FXC {
        re: FXR(4011896955),
        im: FXR(1533436302),
    },
    FXC {
        re: FXR(18446744072176115314),
        im: FXR(4011896955),
    },
    FXC {
        re: FXR(1752536335),
        im: FXR(3921142750),
    },
    FXC {
        re: FXR(18446744069788408866),
        im: FXR(1752536335),
    },
    FXC {
        re: FXR(3119688816),
        im: FXR(2951996911),
    },
    FXC {
        re: FXR(18446744070757554705),
        im: FXR(3119688816),
    },
    FXC {
        re: FXR(118576083),
        im: FXR(4293330151),
    },
    FXC {
        re: FXR(18446744069416221465),
        im: FXR(118576083),
    },
    FXC {
        re: FXR(4290420185),
        im: FXR(197582163),
    },
    FXC {
        re: FXR(18446744073511969453),
        im: FXR(4290420185),
    },
    FXC {
        re: FXR(2894073520),
        im: FXR(3173496894),
    },
    FXC {
        re: FXR(18446744070536054722),
        im: FXR(2894073520),
    },
    FXC {
        re: FXR(3888219974),
        im: FXR(1824414839),
    },
    FXC {
        re: FXR(18446744071885136777),
        im: FXR(3888219974),
    },
    FXC {
        re: FXR(1459330606),
        im: FXR(4039442815),
    },
    FXC {
        re: FXR(18446744069670108801),
        im: FXR(1459330606),
    },
    FXC {
        re: FXR(4169434596),
        im: FXR(1030805132),
    },
    FXC {
        re: FXR(18446744072678746484),
        im: FXR(4169434596),
    },
    FXC {
        re: FXR(2219346178),
        im: FXR(3677124776),
    },
    FXC {
        re: FXR(18446744070032426840),
        im: FXR(2219346178),
    },
    FXC {
        re: FXR(3457583240),
        im: FXR(2547913306),
    },
    FXC {
        re: FXR(18446744071161638310),
        im: FXR(3457583240),
    },
    FXC {
        re: FXR(643233779),
        im: FXR(4246527332),
    },
    FXC {
        re: FXR(18446744069463024284),
        im: FXR(643233779),
    },
    FXC {
        re: FXR(4250394200),
        im: FXR(617165468),
    },
    FXC {
        re: FXR(18446744073092386148),
        im: FXR(4250394200),
    },
    FXC {
        re: FXR(2569080674),
        im: FXR(3441884449),
    },
    FXC {
        re: FXR(18446744070267667167),
        im: FXR(2569080674),
    },
    FXC {
        re: FXR(3690673207),
        im: FXR(2196741986),
    },
    FXC {
        re: FXR(18446744071512809630),
        im: FXR(3690673207),
    },
    FXC {
        re: FXR(1056368897),
        im: FXR(4163031206),
    },
    FXC {
        re: FXR(18446744069546520410),
        im: FXR(1056368897),
    },
    FXC {
        re: FXR(4048321058),
        im: FXR(1434517580),
    },
    FXC {
        re: FXR(18446744072275034036),
        im: FXR(4048321058),
    },
    FXC {
        re: FXR(1848238164),
        im: FXR(3876952381),
    },
    FXC {
        re: FXR(18446744069832599235),
        im: FXR(1848238164),
    },
    FXC {
        re: FXR(3191194855),
        im: FXR(2874546829),
    },
    FXC {
        re: FXR(18446744070835004787),
        im: FXR(3191194855),
    },
    FXC {
        re: FXR(223903967),
        im: FXR(4289127078),
    },
    FXC {
        re: FXR(18446744069420424538),
        im: FXR(223903967),
    },
    FXC {
        re: FXR(4275557289),
        im: FXR(407865107),
    },
    FXC {
        re: FXR(18446744073301686509),
        im: FXR(4275557289),
    },
    FXC {
        re: FXR(2734871369),
        im: FXR(3311679735),
    },
    FXC {
        re: FXR(18446744070397871881),
        im: FXR(2734871369),
    },
    FXC {
        re: FXR(3794016650),
        im: FXR(2013003163),
    },
    FXC {
        re: FXR(18446744071696548453),
        im: FXR(3794016650),
    },
    FXC {
        re: FXR(1259366714),
        im: FXR(4106183088),
    },
    FXC {
        re: FXR(18446744069603368528),
        im: FXR(1259366714),
    },
    FXC {
        re: FXR(4113833119),
        im: FXR(1234147941),
    },
    FXC {
        re: FXR(18446744072475403675),
        im: FXR(4113833119),
    },
    FXC {
        re: FXR(2036244917),
        im: FXR(3781593674),
    },
    FXC {
        re: FXR(18446744069927957942),
        im: FXR(2036244917),
    },
    FXC {
        re: FXR(3328398249),
        im: FXR(2714499801),
    },
    FXC {
        re: FXR(18446744070995051815),
        im: FXR(3328398249),
    },
    FXC {
        re: FXR(434091755),
        im: FXR(4272974189),
    },
    FXC {
        re: FXR(18446744069436577427),
        im: FXR(434091755),
    },
    FXC {
        re: FXR(4214991540),
        im: FXR(824979024),
    },
    FXC {
        re: FXR(18446744072884572592),
        im: FXR(4214991540),
    },
    FXC {
        re: FXR(2397100839),
        im: FXR(3563797363),
    },
    FXC {
        re: FXR(18446744070145754253),
        im: FXR(2397100839),
    },
    FXC {
        re: FXR(3578438609),
        im: FXR(2375188665),
    },
    FXC {
        re: FXR(18446744071334362951),
        im: FXR(3578438609),
    },
    FXC {
        re: FXR(850826195),
        im: FXR(4209850218),
    },
    FXC {
        re: FXR(18446744069499701398),
        im: FXR(850826195),
    },
    FXC {
        re: FXR(3973056236),
        im: FXR(1631431340),
    },
    FXC {
        re: FXR(18446744072078120276),
        im: FXR(3973056236),
    },
    FXC {
        re: FXR(1655778843),
        im: FXR(3962971170),
    },
    FXC {
        re: FXR(18446744069746580446),
        im: FXR(1655778843),
    },
    FXC {
        re: FXR(3046303593),
        im: FXR(3027668821),
    },
    FXC {
        re: FXR(18446744070681882795),
        im: FXR(3046303593),
    },
    FXC {
        re: FXR(13176774),
        im: FXR(4294947083),
    },
    FXC {
        re: FXR(18446744069414604533),
        im: FXR(13176774),
    },
];

// ========================================================================
