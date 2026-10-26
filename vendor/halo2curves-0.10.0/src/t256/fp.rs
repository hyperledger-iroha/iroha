#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use core::convert::TryInto;

use halo2derive::impl_field;
use rand_core::RngCore;
use subtle::{Choice, ConditionallySelectable, ConstantTimeEq, CtOption};

impl_field!(
    t256_base,
    Fp,
    modulus = "ffffffff0000000100000000000000017e72b42b30e7317793135661b1c4b117",
    mul_gen = "6",
    zeta = "b6e848ab29fcd1fa4bdd4681fd4839c7bce2889d6b27546299d130ed370be14d",
    from_uniform = [48, 64],
    endian = "big",
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
