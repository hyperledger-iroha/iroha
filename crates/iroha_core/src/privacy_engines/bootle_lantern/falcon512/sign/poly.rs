#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
use super::super::table_assets::read_u64_le;
use super::flr::FLR;
// ========================================================================
// Floating-point polynomials
// ========================================================================
// We consider here polynomials in R[X]/(X^n+1), for n a power of two
// between 4 and 1024. We express n = 2^logn for logn in [2, 10]. For
// each such polynomial:
//
//   - The "normal representation" of f = \sum_{i=0}^{n-1} f_i*X^i is
//     the sequence (f_0, f_1, f_2, ... f_{n-1}) as a slice of size n.
//     Elements are FLR instances.
//
//   - The "FFT representation" consists of n/2 complex numbers; the
//     first n/2 elements are the real parts, and for i = 0 to n/2-1,
//     the element i+n/2 in the slice contains the imaginary part
//     corresponding to element i in the slice. Only n/2 complex numbers
//     are needed for the FFT representation because all polynomials
//     are real (in normal representation), making the FFT representation
//     redundant.
//
//   - If a polynomial is self-adjoint then its FFT representation itself
//     contains only real numbers, and the corresponding slice only
//     has n/2 elements; the remaining n/2 (the imaginary parts of the
//     FFT coefficients) are implicitly zero and are omitted.
// Complex multiplication.
#[allow(dead_code)]
#[inline(always)]
pub(crate) fn flc_mul(x_re: FLR, x_im: FLR, y_re: FLR, y_im: FLR) -> (FLR, FLR) {
    (x_re * y_re - x_im * y_im, x_re * y_im + x_im * y_re)
}
/* unused
// Complex division.
#[inline(always)]
fn flc_div(x_re: FLR, x_im: FLR, y_re: FLR, y_im: FLR) -> (FLR, FLR) {
    let m = FLR::ONE / (y_re.square() + y_im.square());
    let b_re = m * y_re;
    let b_im = m * -y_im;
    flc_mul(x_re, x_im, b_re, b_im)
}
*/
// Convert a polynomial from normal representation to FFT.
pub(crate) fn FFT(logn: u32, f: &mut [FLR]) {
    // First iteration of the FFT algorithm would compute
    // f[j] + i*f[j + n/2] for all j < n/2; since this is exactly our
    // storage format for complex numbers in the FFT representation,
    // that first iteration is a no-op, hence we can start the computation
    // at the second iteration.
    {
        assert!(logn >= 1);
        let n = 1usize << logn;
        let hn = n >> 1;
        let mut t = hn;
        for lm in 1..logn {
            let m = 1 << lm;
            let hm = m >> 1;
            let ht = t >> 1;
            let mut j0 = 0;
            for i in 0..hm {
                let s_re = GM[((m + i) << 1) + 0];
                let s_im = GM[((m + i) << 1) + 1];
                for j in 0..ht {
                    let j1 = j0 + j;
                    let j2 = j1 + ht;
                    let x_re = f[j1];
                    let x_im = f[j1 + hn];
                    let y_re = f[j2];
                    let y_im = f[j2 + hn];
                    let (z_re, z_im) = flc_mul(y_re, y_im, s_re, s_im);
                    f[j1] = x_re + z_re;
                    f[j1 + hn] = x_im + z_im;
                    f[j2] = x_re - z_re;
                    f[j2 + hn] = x_im - z_im;
                }
                j0 += t;
            }
            t = ht;
        }
    }
}
// Convert a polynomial from FFT representation to normal.
pub(crate) fn iFFT(logn: u32, f: &mut [FLR]) {
    // This is the reverse of FFT. We use the fact that if
    // w = exp(i*k*pi/N), then 1/w is the conjugate of w; thus, we can
    // get inverses from the table GM[] itself by simply negating the
    // imaginary part.
    //
    // The last iteration is a no-op (like the first iteration in FFT).
    // Since the last iteration is skipped, we have to perform only
    // a division by n/2 at the end.
    {
        assert!(logn >= 1);
        let n = 1usize << logn;
        let hn = n >> 1;
        let mut t = 1;
        for lm in 1..logn {
            let hm = 1 << (logn - lm);
            let dt = t << 1;
            let mut j0 = 0;
            for i in 0..(hm >> 1) {
                let s_re = GM[((hm + i) << 1) + 0];
                let s_im = -GM[((hm + i) << 1) + 1];
                for j in 0..t {
                    let j1 = j0 + j;
                    let j2 = j1 + t;
                    let x_re = f[j1];
                    let x_im = f[j1 + hn];
                    let y_re = f[j2];
                    let y_im = f[j2 + hn];
                    f[j1] = x_re + y_re;
                    f[j1 + hn] = x_im + y_im;
                    let x_re = x_re - y_re;
                    let x_im = x_im - y_im;
                    let (z_re, z_im) = flc_mul(x_re, x_im, s_re, s_im);
                    f[j2] = z_re;
                    f[j2 + hn] = z_im;
                }
                j0 += dt;
            }
            t = dt;
        }
        // We have logn-1 delayed halvings to perform, i.e. we must divide
        // all returned values by n/2.
        FLR::slice_div2e(&mut f[..n], logn - 1);
    }
}
// Set polynomial d from polynomial f with small coefficients.
pub(crate) fn poly_set_small(logn: u32, d: &mut [FLR], f: &[i8]) {
    {
        for i in 0..(1usize << logn) {
            d[i] = FLR::from_i32(f[i] as i32);
        }
    }
}
// Add polynomial b to polynomial a.
pub(crate) fn poly_add(logn: u32, a: &mut [FLR], b: &[FLR]) {
    {
        for i in 0..(1usize << logn) {
            a[i] += b[i];
        }
    }
}
// Subtract polynomial b from polynomial a.
pub(crate) fn poly_sub(logn: u32, a: &mut [FLR], b: &[FLR]) {
    {
        for i in 0..(1usize << logn) {
            a[i] -= b[i];
        }
    }
}
// Negate polynomial a.
pub(crate) fn poly_neg(logn: u32, a: &mut [FLR]) {
    {
        for i in 0..(1usize << logn) {
            a[i] = -a[i];
        }
    }
}
/* unused
// Replace polynomial a with its Hermitian adjoint adj(a). The polynomial
// must be in FFT representation.
pub(crate) fn poly_adj_fft(logn: u32, a: &mut [FLR]) {
    let n = 1usize << logn;
    for i in (n >> 1)..n {
        a[i] = -a[i];
    }
}
*/
// Multiply polynomial a with polynomial b. The polynomials must be in
// FFT representation.
pub(crate) fn poly_mul_fft(logn: u32, a: &mut [FLR], b: &[FLR]) {
    {
        let hn = 1usize << (logn - 1);
        for i in 0..hn {
            let (re, im) = flc_mul(a[i], a[i + hn], b[i], b[i + hn]);
            a[i] = re;
            a[i + hn] = im;
        }
    }
}
// Multiply polynomial a with the adjoint of polynomial b. The polynomials
// must be in FFT representation.
pub(crate) fn poly_muladj_fft(logn: u32, a: &mut [FLR], b: &[FLR]) {
    {
        let hn = 1usize << (logn - 1);
        for i in 0..hn {
            let (re, im) = flc_mul(a[i], a[i + hn], b[i], -b[i + hn]);
            a[i] = re;
            a[i + hn] = im;
        }
    }
}
// Multiply polynomial a with its own adjoint. The polynomial must be in
// FFT representation. Since the result is a self-adjoint polynomial,
// coefficients n/2 to n-1 are set to zero.
pub(crate) fn poly_mulownadj_fft(logn: u32, a: &mut [FLR]) {
    {
        let hn = 1usize << (logn - 1);
        for i in 0..hn {
            a[i] = a[i].square() + a[i + hn].square();
            a[i + hn] = FLR::ZERO;
        }
    }
}
// Multiply polynomial a with a real constant x.
pub(crate) fn poly_mulconst(logn: u32, a: &mut [FLR], x: FLR) {
    {
        for i in 0..(1usize << logn) {
            a[i] *= x;
        }
    }
}
/* unused
// Divide polynomial a by polynomial b. The polynomials MUST be in FFT
// representation.
pub(crate) fn poly_div_fft(logn: u32, a: &mut [FLR], b: &[FLR]) {
    let hn = 1usize << (logn - 1);
    for i in 0..hn {
        let (re, im) = flc_div(a[i], a[i + hn], b[i], b[i + hn]);
        a[i] = re;
        a[i + hn] = im;
    }
}
*/
/* unused
// Set polynomial d to 1/(f*adj(f) + g*adj(g)). All polynomials are in
// FFT representation. Since the output d is self-adjoint, only its
// first n/2 coefficients are set; the other n/2 coefficients are
// implicitly zero, but need not exist in the destination slice.
pub(crate) fn poly_invnorm2_fft(logn: u32,
    d: &mut [FLR], f: &[FLR], g: &[FLR])
{
    let hn = 1usize << (logn - 1);
    for i in 0..hn {
        let nf = f[i].square() + f[i + hn].square();
        let ng = g[i].square() + g[i + hn].square();
        d[i] = FLR::ONE / (nf + ng);
    }
}
*/
/* unused
// Given polynomial F, G, f and g, set d to F*adj(f) + G*adj(g). All
// polynomials are in FFT representation.
pub(crate) fn poly_add_muladj_fft(logn: u32,
    d: &mut [FLR], F: &[FLR], G: &[FLR], f: &[FLR], g: &[FLR])
{
    let hn = 1usize << (logn - 1);
    for i in 0..hn {
        let (a_re, a_im) = flc_mul(F[i], F[i + hn], f[i], f[i + hn]);
        let (b_re, b_im) = flc_mul(G[i], G[i + hn], g[i], g[i + hn]);
        d[i] = a_re + b_re;
        d[i + hn] = a_im + b_im;
    }
}
*/
/* unused
// Multiply polynomial a by polynomial b, where b is self-adjoint. Only
// the first n/2 coefficients of b are accessed. All polynomials are in
// FFT representation.
pub(crate) fn poly_mul_selfadj_fft(logn: u32, a: &mut [FLR], b: &[FLR]) {
    let hn = 1usize << (logn - 1);
    for i in 0..hn {
        a[i] *= b[i];
        a[i + hn] *= b[i];
    }
}
*/
/* unused
// Divide polynomial a by polynomial b, where b is self-adjoint. Only
// the first n/2 coefficients of b are accessed. All polynomials are in
// FFT representation.
pub(crate) fn poly_div_selfadj_fft(logn: u32, a: &mut [FLR], b: &[FLR]) {
    let hn = 1usize << (logn - 1);
    for i in 0..hn {
        let x = FLR::ONE / b[i];
        a[i] *= x;
        a[i + hn] *= x;
    }
}
*/
// Perform an LDL decomposition of a self-adjoint matrix G. The matrix
// is G = [[g00, g01], [adj(g01), g11]]; g00 and g11 are self-adjoint
// polynomials. The decomposition is G = L*D*adj(L), with:
//    D = [[g00, 0], [0, d11]]
//    L = [[1, 0], [l10, 1]]
// The output polynomials l10 and d11 are written over g01 and g11,
// respectively. Like g11, d11 is self-adjoint and uses only n/2
// coefficients. g00 is unmodified. All polynomials are in FFT
// representation.
pub(crate) fn poly_LDL_fft(logn: u32, g00: &[FLR], g01: &mut [FLR], g11: &mut [FLR]) {
    {
        let hn = 1usize << (logn - 1);
        for i in 0..hn {
            // g00 and g11 are self-adjoint
            let g00_re = g00[i];
            let (g01_re, g01_im) = (g01[i], g01[i + hn]);
            let g11_re = g11[i];
            let inv_g00_re = FLR::ONE / g00_re;
            let (mu_re, mu_im) = (g01_re * inv_g00_re, g01_im * inv_g00_re);
            let zo_re = mu_re * g01_re + mu_im * g01_im;
            g11[i] = g11_re - zo_re;
            g01[i] = mu_re;
            g01[i + hn] = -mu_im;
        }
    }
}
/* unused
// This is identical to poly_LDL_fft() except that the output polynomials
// l10 and d11 are written into separate output buffers instead of
// overwriting the provided g01 and g11.
pub(crate) fn poly_LDLmv_fft(logn: u32,
    d11: &mut [FLR], l10: &mut [FLR], g00: &[FLR], g01: &[FLR], g11: &[FLR])
{
    let hn = 1usize << (logn - 1);
    for i in 0..hn {
        let (g00_re, g00_im) = (g00[i], g00[i + hn]);
        let (g01_re, g01_im) = (g01[i], g01[i + hn]);
        let (g11_re, g11_im) = (g11[i], g11[i + hn]);
        let (mu_re, mu_im) = flc_div(g01_re, g01_im, g00_re, g00_im);
        let (zo_re, zo_im) = flc_mul(mu_re, mu_im, g01_re, -g01_im);
        d11[i] = g11_re - zo_re;
        d11[i + hn] = g11_im - zo_im;
        l10[i] = mu_re;
        l10[i + hn] = -mu_im;
    }
}
*/
// Split operation on a polynomial: for input polynomial f, half-size
// polynomials f0 and f1 (modulo X^(n/2)+1) are such that
// f = f0(x^2) + x*f1(x^2). All polynomials are in FFT representation.
pub(crate) fn poly_split_fft(logn: u32, f0: &mut [FLR], f1: &mut [FLR], f: &[FLR]) {
    // If logn = 1 then the loop is entirely skipped.
    if logn == 1 {
        f0[0] = f[0];
        f1[0] = f[1];
        return;
    }
    {
        let hn = 1usize << (logn - 1);
        let qn = hn >> 1;
        for i in 0..qn {
            let (a_re, a_im) = (f[(i << 1) + 0], f[(i << 1) + 0 + hn]);
            let (b_re, b_im) = (f[(i << 1) + 1], f[(i << 1) + 1 + hn]);
            let (t_re, t_im) = (a_re + b_re, a_im + b_im);
            f0[i] = t_re.half();
            f0[i + qn] = t_im.half();
            let (t_re, t_im) = (a_re - b_re, a_im - b_im);
            let (u_re, u_im) = flc_mul(
                t_re,
                t_im,
                GM[((i + hn) << 1) + 0],
                -GM[((i + hn) << 1) + 1],
            );
            f1[i] = u_re.half();
            f1[i + qn] = u_im.half();
        }
    }
}
// Specialized version of poly_split_fft() when the source polynomial
// is self-adjoint (i.e. all its FFT coefficients are real). On output,
// f0 is self-adjoint, but f1 is not necessarily self-adjoint.
pub(crate) fn poly_split_selfadj_fft(logn: u32, f0: &mut [FLR], f1: &mut [FLR], f: &[FLR]) {
    // If logn = 1 then the loop is entirely skipped.
    if logn == 1 {
        f0[0] = f[0];
        f1[0] = FLR::ZERO;
        return;
    }
    {
        let hn = 1usize << (logn - 1);
        let qn = hn >> 1;
        for i in 0..qn {
            let a_re = f[(i << 1) + 0];
            let b_re = f[(i << 1) + 1];
            let t_re = a_re + b_re;
            f0[i] = t_re.half();
            f0[i + qn] = FLR::ZERO;
            let t_re = (a_re - b_re).half();
            f1[i] = t_re * GM[((i + hn) << 1) + 0];
            f1[i + qn] = t_re * -GM[((i + hn) << 1) + 1];
        }
    }
}
// Merge operation on a polynomial: for input half-size polynomials f0
// and f1 (modulo X^(n/2)+1), compute f = f0(x^2) + x*f1(x^2). All
// polynomials are in FFT representation.
pub(crate) fn poly_merge_fft(logn: u32, f: &mut [FLR], f0: &[FLR], f1: &[FLR]) {
    // If logn = 1 then the loop is entirely skipped.
    if logn == 1 {
        f[0] = f0[0];
        f[1] = f1[0];
        return;
    }
    {
        let hn = 1usize << (logn - 1);
        let qn = hn >> 1;
        for i in 0..qn {
            let (a_re, a_im) = (f0[i], f0[i + qn]);
            let (b_re, b_im) = flc_mul(
                f1[i],
                f1[i + qn],
                GM[((i + hn) << 1) + 0],
                GM[((i + hn) << 1) + 1],
            );
            f[(i << 1) + 0] = a_re + b_re;
            f[(i << 1) + 0 + hn] = a_im + b_im;
            f[(i << 1) + 1] = a_re - b_re;
            f[(i << 1) + 1 + hn] = a_im - b_im;
        }
    }
}
// Table of constants for FFT. For k = 1 to 1023, define j = rev10(k), with
// rev10() being the bit-reversal function over 10 bits. Then:
//   GM[2*k + 0] = cos(k*pi/1024)
//   GM[2*k + 1] = sin(k*pi/1024)
// Here, all values are computed from integer approximations which were
// obtained from Sage, which employs sufficient precision to get an exact
// rounding. Specifically, we round x = cos(j*pi/1024) by looking for the
// integer n such that abs(x)*2^n is in [2^52, 2^53[, and make the value
// as round(x*2^n)/2^n (with FLR::scaled()).
// A test makes sure that the generated FLR constants have exactly the
// expected values.
//
// GM[0] and GM[1] (corresponding to k = 0) are unused and left to zero.
const fn mkflr(bits: u64) -> FLR {
    FLR::from_bits(bits)
}
const SIGN_GM_BYTES: &[u8; 16_384] = include_bytes!("../assets/sign_gm_binary64le_v1.bin");
const fn decode_gm(bytes: &[u8; 16_384]) -> [FLR; 2048] {
    let mut table = [FLR::ZERO; 2048];
    let mut index = 0;
    while index < table.len() {
        table[index] = mkflr(read_u64_le(bytes, index * 8));
        index += 1;
    }
    table
}
pub(crate) const GM: [FLR; 2048] = decode_gm(SIGN_GM_BYTES);
