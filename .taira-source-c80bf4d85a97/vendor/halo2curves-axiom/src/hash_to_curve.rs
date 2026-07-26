#![allow(clippy::op_ref)]

use ff::{Field, FromUniformBytes, PrimeField};
use pasta_curves::arithmetic::CurveExt;
use static_assertions::const_assert;
use subtle::{Choice, ConditionallySelectable, ConstantTimeEq, CtOption};

use crate::ff_ext::Legendre;

/// Hashes over a message and writes the output to all of `buf`.
/// Modified from https://github.com/zcash/pasta_curves/blob/7e3fc6a4919f6462a32b79dd226cb2587b7961eb/src/hashtocurve.rs#L11.
fn hash_to_field<F: FromUniformBytes<64>>(
    method: &str,
    curve_id: &str,
    domain_prefix: &str,
    message: &[u8],
    buf: &mut [F; 2],
) {
    assert!(domain_prefix.len() < 256);
    assert!((18 + method.len() + curve_id.len() + domain_prefix.len()) < 256);

    // Assume that the field size is 32 bytes and k is 256, where k is defined in
    // <https://www.ietf.org/archive/id/draft-irtf-cfrg-hash-to-curve-10.html#name-security-considerations-3>.
    const CHUNKLEN: usize = 64;
    const_assert!(CHUNKLEN * 2 < 256);

    // Input block size of BLAKE2b.
    const R_IN_BYTES: usize = 128;

    let personal = [0u8; 16];
    let empty_hasher = blake2b_simd::Params::new()
        .hash_length(CHUNKLEN)
        .personal(&personal)
        .to_state();

    let b_0 = empty_hasher
        .clone()
        .update(&[0; R_IN_BYTES])
        .update(message)
        .update(&[0, (CHUNKLEN * 2) as u8, 0])
        .update(domain_prefix.as_bytes())
        .update(b"-")
        .update(curve_id.as_bytes())
        .update(b"_XMD:BLAKE2b_")
        .update(method.as_bytes())
        .update(b"_RO_")
        .update(&[(18 + method.len() + curve_id.len() + domain_prefix.len()) as u8])
        .finalize();

    let b_1 = empty_hasher
        .clone()
        .update(b_0.as_array())
        .update(&[1])
        .update(domain_prefix.as_bytes())
        .update(b"-")
        .update(curve_id.as_bytes())
        .update(b"_XMD:BLAKE2b_")
        .update(method.as_bytes())
        .update(b"_RO_")
        .update(&[(18 + method.len() + curve_id.len() + domain_prefix.len()) as u8])
        .finalize();

    let b_2 = {
        let mut empty_hasher = empty_hasher;
        for (l, r) in b_0.as_array().iter().zip(b_1.as_array().iter()) {
            empty_hasher.update(&[*l ^ *r]);
        }
        empty_hasher
            .update(&[2])
            .update(domain_prefix.as_bytes())
            .update(b"-")
            .update(curve_id.as_bytes())
            .update(b"_XMD:BLAKE2b_")
            .update(method.as_bytes())
            .update(b"_RO_")
            .update(&[(18 + method.len() + curve_id.len() + domain_prefix.len()) as u8])
            .finalize()
    };

    for (big, buf) in [b_1, b_2].iter().zip(buf.iter_mut()) {
        let mut little = [0u8; CHUNKLEN];
        little.copy_from_slice(big.as_array());
        little.reverse();
        *buf = F::from_uniform_bytes(&little);
    }
}

// Implementation of <https://datatracker.ietf.org/doc/html/rfc9380#name-simplified-swu-method>
#[allow(clippy::too_many_arguments)]
pub(crate) fn simple_svdw_map_to_curve<C>(u: C::Base, z: C::Base) -> C
where
    C: CurveExt,
{
    let zero = C::Base::ZERO;
    let one = C::Base::ONE;
    let a = C::a();
    let b = C::b();

    //1.  tv1 = u^2
    let tv1 = u.square();
    //2.  tv1 = Z * tv1
    let tv1 = z * tv1;
    //3.  tv2 = tv1^2
    let tv2 = tv1.square();
    //4.  tv2 = tv2 + tv1
    let tv2 = tv2 + tv1;
    //5.  tv3 = tv2 + 1
    let tv3 = tv2 + one;
    //6.  tv3 = B * tv3
    let tv3 = b * tv3;
    //7.  tv4 = CMOV(Z, -tv2, tv2 != 0) # tv4 = z if tv2 is 0 else tv4 = -tv2
    let tv2_is_not_zero = !tv2.ct_eq(&zero);
    let tv4 = C::Base::conditional_select(&z, &-tv2, tv2_is_not_zero);
    //8.  tv4 = A * tv4
    let tv4 = a * tv4;
    //9.  tv2 = tv3^2
    let tv2 = tv3.square();
    //10. tv6 = tv4^2
    let tv6 = tv4.square();
    //11. tv5 = A * tv6
    let tv5 = a * tv6;
    //12. tv2 = tv2 + tv5
    let tv2 = tv2 + tv5;
    //13. tv2 = tv2 * tv3
    let tv2 = tv2 * tv3;
    //14. tv6 = tv6 * tv4
    let tv6 = tv6 * tv4;
    //15. tv5 = B * tv6
    let tv5 = b * tv6;
    //16. tv2 = tv2 + tv5
    let tv2 = tv2 + tv5;
    //17.   x = tv1 * tv3
    let x = tv1 * tv3;
    //18. (is_gx1_square, y1) = sqrt_ratio(tv2, tv6)
    let (is_gx1_square, y1) = sqrt_ratio(&tv2, &tv6, &z);
    //19.   y = tv1 * u
    let y = tv1 * u;
    //20.   y = y * y1
    let y = y * y1;
    //21.   x = CMOV(x, tv3, is_gx1_square)
    let x = C::Base::conditional_select(&x, &tv3, is_gx1_square);
    //22.   y = CMOV(y, y1, is_gx1_square)
    let y = C::Base::conditional_select(&y, &y1, is_gx1_square);
    //23.  e1 = sgn0(u) == sgn0(y)
    let e1 = u.is_odd().ct_eq(&y.is_odd());
    //24.   y = CMOV(-y, y, e1) # Select correct sign of y
    let y = C::Base::conditional_select(&-y, &y, e1);
    //25.   x = x / tv4
    let x = x * tv4.invert().unwrap();
    //26. return (x, y)
    C::new_jacobian(x, y, one).unwrap()
}

#[allow(clippy::type_complexity)]
pub(crate) fn simple_svdw_hash_to_curve<'a, C>(
    curve_id: &'static str,
    domain_prefix: &'a str,
    z: C::Base,
) -> Box<dyn Fn(&[u8]) -> C + 'a>
where
    C: CurveExt,
    C::Base: FromUniformBytes<64>,
{
    Box::new(move |message| {
        let mut us = [C::Base::ZERO; 2];
        hash_to_field("SSWU", curve_id, domain_prefix, message, &mut us);

        let [q0, q1]: [C; 2] = us.map(|u| simple_svdw_map_to_curve(u, z));

        let r = q0 + &q1;
        debug_assert!(bool::from(r.is_on_curve()));
        r
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn svdw_map_to_curve<C>(
    u: C::Base,
    c1: C::Base,
    c2: C::Base,
    c3: C::Base,
    c4: C::Base,
    z: C::Base,
) -> C
where
    C: CurveExt,
    C::Base: Legendre,
{
    let one = C::Base::ONE;
    let a = C::a();
    let b = C::b();

    // 1. tv1 = u^2
    let tv1 = u.square();
    // 2. tv1 = tv1 * c1
    let tv1 = tv1 * c1;
    // 3. tv2 = 1 + tv1
    let tv2 = one + tv1;
    // 4. tv1 = 1 - tv1
    let tv1 = one - tv1;
    // 5. tv3 = tv1 * tv2
    let tv3 = tv1 * tv2;
    // 6. tv3 = inv0(tv3)
    let tv3 = tv3.invert().unwrap_or(C::Base::ZERO);
    // 7. tv4 = u * tv1
    let tv4 = u * tv1;
    // 8. tv4 = tv4 * tv3
    let tv4 = tv4 * tv3;
    // 9. tv4 = tv4 * c3
    let tv4 = tv4 * c3;
    // 10. x1 = c2 - tv4
    let x1 = c2 - tv4;
    // 11. gx1 = x1^2
    let gx1 = x1.square();
    // 12. gx1 = gx1 + A
    let gx1 = gx1 + a;
    // 13. gx1 = gx1 * x1
    let gx1 = gx1 * x1;
    // 14. gx1 = gx1 + B
    let gx1 = gx1 + b;
    // 15. e1 = is_square(gx1)
    let e1 = !gx1.ct_quadratic_non_residue();
    // 16. x2 = c2 + tv4
    let x2 = c2 + tv4;
    // 17. gx2 = x2^2
    let gx2 = x2.square();
    // 18. gx2 = gx2 + A
    let gx2 = gx2 + a;
    // 19. gx2 = gx2 * x2
    let gx2 = gx2 * x2;
    // 20. gx2 = gx2 + B
    let gx2 = gx2 + b;
    // 21. e2 = is_square(gx2) AND NOT e1    # Avoid short-circuit logic ops
    let e2 = !gx2.ct_quadratic_non_residue() & (!e1);
    // 22. x3 = tv2^2
    let x3 = tv2.square();
    // 23. x3 = x3 * tv3
    let x3 = x3 * tv3;
    // 24. x3 = x3^2
    let x3 = x3.square();
    // 25. x3 = x3 * c4
    let x3 = x3 * c4;
    // 26. x3 = x3 + Z
    let x3 = x3 + z;
    // 27. x = CMOV(x3, x1, e1)    # x = x1 if gx1 is square, else x = x3
    let x = C::Base::conditional_select(&x3, &x1, e1);
    // 28. x = CMOV(x, x2, e2)    # x = x2 if gx2 is square and gx1 is not
    let x = C::Base::conditional_select(&x, &x2, e2);
    // 29. gx = x^2
    let gx = x.square();
    // 30. gx = gx + A
    let gx = gx + a;
    // 31. gx = gx * x
    let gx = gx * x;
    // 32. gx = gx + B
    let gx = gx + b;
    // 33. y = sqrt(gx)
    let y = gx.sqrt().unwrap();
    // 34. e3 = sgn0(u) == sgn0(y)
    let e3 = u.is_odd().ct_eq(&y.is_odd());
    // 35. y = CMOV(-y, y, e3)    # Select correct sign of y
    let y = C::Base::conditional_select(&-y, &y, e3);
    // 36. return (x, y)
    C::new_jacobian(x, y, one).unwrap()
}

// Implement https://datatracker.ietf.org/doc/html/rfc9380#name-sqrt_ratio-for-any-field
// Copied from ff sqrt_ratio_generic substituting F::ROOT_OF_UNITY for input Z
fn sqrt_ratio<F: PrimeField>(num: &F, div: &F, z: &F) -> (Choice, F) {
    // General implementation:
    //
    // a = num * inv0(div)
    //   = {    0    if div is zero
    //     { num/div otherwise
    //
    // b = z * a
    //   = {      0      if div is zero
    //     { z*num/div otherwise

    // Since z is non-square, a and b are either both zero (and both square), or
    // only one of them is square. We can therefore choose the square root to return
    // based on whether a is square, but for the boolean output we need to handle the
    // num != 0 && div == 0 case specifically.

    let a = div.invert().unwrap_or(F::ZERO) * num;
    let b = a * z;
    let sqrt_a = a.sqrt();
    let sqrt_b = b.sqrt();

    let num_is_zero = num.is_zero();
    let div_is_zero = div.is_zero();
    let is_square = sqrt_a.is_some();
    let is_nonsquare = sqrt_b.is_some();
    assert!(bool::from(
        num_is_zero | div_is_zero | (is_square ^ is_nonsquare)
    ));

    (
        is_square & (num_is_zero | !div_is_zero),
        CtOption::conditional_select(&sqrt_b, &sqrt_a, is_square).unwrap(),
    )
}

/// Implementation of https://www.ietf.org/archive/id/draft-irtf-cfrg-hash-to-curve-10.html#section-6.6.1
#[allow(clippy::type_complexity)]
pub(crate) fn svdw_hash_to_curve<'a, C>(
    curve_id: &'static str,
    domain_prefix: &'a str,
    z: C::Base,
) -> Box<dyn Fn(&[u8]) -> C + 'a>
where
    C: CurveExt,
    C::Base: FromUniformBytes<64> + Legendre,
{
    let [c1, c2, c3, c4] = svdw_precomputed_constants::<C>(z);

    Box::new(move |message| {
        let mut us = [C::Base::ZERO; 2];
        hash_to_field("SVDW", curve_id, domain_prefix, message, &mut us);

        let [q0, q1]: [C; 2] = us.map(|u| svdw_map_to_curve(u, c1, c2, c3, c4, z));

        let r = q0 + &q1;
        debug_assert!(bool::from(r.is_on_curve()));
        r
    })
}

pub(crate) fn svdw_precomputed_constants<C: CurveExt>(z: C::Base) -> [C::Base; 4] {
    let a = C::a();
    let b = C::b();
    let one = C::Base::ONE;
    let three = one + one + one;
    let four = three + one;
    let tmp = three * z.square() + four * a;

    // 1. c1 = g(Z)
    let c1 = (z.square() + a) * z + b;
    // 2. c2 = -Z / 2
    let c2 = -z * C::Base::TWO_INV;
    // 3. c3 = sqrt(-g(Z) * (3 * Z^2 + 4 * A))    # sgn0(c3) MUST equal 0
    let c3 = {
        let c3 = (-c1 * tmp).sqrt().unwrap();
        C::Base::conditional_select(&c3, &-c3, c3.is_odd())
    };
    // 4. c4 = -4 * g(Z) / (3 * Z^2 + 4 * A)
    let c4 = -four * c1 * tmp.invert().unwrap();

    [c1, c2, c3, c4]
}
