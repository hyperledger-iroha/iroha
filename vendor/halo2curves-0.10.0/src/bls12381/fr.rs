#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use core::convert::TryInto;

use halo2derive::impl_field;
use rand_core::RngCore;
use subtle::{Choice, ConditionallySelectable, ConstantTimeEq, CtOption};

impl_field!(
    bls12381_scalar,
    Fr,
    modulus = "73eda753299d7d483339d80809a1d80553bda402fffe5bfeffffffff00000001",
    mul_gen = "7",
    zeta = "ac45a4010001a40200000000ffffffff",
    from_uniform = [64],
    endian = "little",
);

crate::extend_field_legendre!(Fr);
crate::impl_binops_calls!(Fr);
crate::impl_binops_additive!(Fr, Fr);
crate::impl_binops_multiplicative!(Fr, Fr);
crate::field_bits!(Fr);
crate::serialize_deserialize_primefield!(Fr);
crate::impl_from_u64!(Fr);
crate::impl_from_bool!(Fr);

#[cfg(test)]
mod test {
    use super::Fr;
    use crate::{
        arith_test, constants_test, from_uniform_bytes_test, legendre_test, serde_test, test,
    };

    constants_test!(Fr);

    arith_test!(Fr);
    legendre_test!(Fr);
    test!(arith, Fr, sqrt_test, 1000);

    serde_test!(Fr PrimeFieldBits);
    from_uniform_bytes_test!(Fr, 1000, L 64);
}
