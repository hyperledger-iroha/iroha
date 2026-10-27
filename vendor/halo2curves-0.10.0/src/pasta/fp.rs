#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use core::convert::TryInto;
use halo2derive::impl_field;
use rand_core::RngCore;
use subtle::{Choice, ConditionallySelectable, ConstantTimeEq, CtOption};

use crate::{
    extend_field_legendre, field_bits, impl_binops_additive, impl_binops_calls,
    impl_binops_multiplicative, impl_from_bool, impl_from_u64, serialize_deserialize_primefield,
};

// Fp: Pasta base field and Vesta scalar field.
impl_field!(
    pasta_base,
    Fp,
    modulus = "40000000000000000000000000000000224698fc094cf91b992d30ed00000001",
    mul_gen = "5",
    zeta = "12ccca834acdba712caad5dc57aab1b01d1f8bd237ad31491dad5ebdfdfe4ab9",
    from_uniform = [48, 64],
    endian = "little",
);

extend_field_legendre!(Fp);
impl_binops_calls!(Fp);
impl_binops_additive!(Fp, Fp);
impl_binops_multiplicative!(Fp, Fp);
impl_from_u64!(Fp);
impl_from_bool!(Fp);
field_bits!(Fp);
serialize_deserialize_primefield!(Fp);

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
