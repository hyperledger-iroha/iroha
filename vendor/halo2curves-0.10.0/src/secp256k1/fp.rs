#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use core::convert::TryInto;

use halo2derive::impl_field;
use rand_core::RngCore;
use subtle::{Choice, ConditionallySelectable, ConstantTimeEq, CtOption};

impl_field!(
    secp256k1_base,
    Fp,
    modulus = "fffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc2f",
    mul_gen = "3",
    zeta = "7ae96a2b657c07106e64479eac3434e99cf0497512f58995c1396c28719501ee",
    from_uniform = [48, 64],
    endian = "little",
);

crate::extend_field_legendre!(Fp);
crate::impl_binops_calls!(Fp);
crate::impl_binops_additive!(Fp, Fp);
crate::impl_binops_multiplicative!(Fp, Fp);
crate::field_bits!(Fp);
crate::serialize_deserialize_primefield!(Fp);
crate::impl_from_u64!(Fp);
crate::impl_from_bool!(Fp);

#[cfg(test)]
mod test {
    use super::Fp;
    use crate::{
        arith_test, constants_test, from_uniform_bytes_test, legendre_test, serde_test, test,
    };

    constants_test!(Fp);
    arith_test!(Fp);
    legendre_test!(Fp);
    test!(arith, Fp, sqrt_test, 1000);
    serde_test!(Fp PrimeFieldBits);
    from_uniform_bytes_test!(Fp, 1000, L 64, L 48);
}
