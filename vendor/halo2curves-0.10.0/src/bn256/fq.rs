#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use core::convert::TryInto;
use halo2derive::impl_field;
use rand_core::RngCore;
use subtle::{Choice, ConditionallySelectable, ConstantTimeEq, CtOption};

use crate::ff_ext::ExtField;

impl_field!(
    bn256_base,
    Fq,
    modulus = "30644e72e131a029b85045b68181585d97816a916871ca8d3c208c16d87cfd47",
    mul_gen = "3",
    zeta = "30644e72e131a0295e6dd9e7e0acccb0c28f069fbb966e3de4bd44e5607cfd48",
    from_uniform = [64, 48],
    endian = "little",
);

crate::extend_field_legendre!(Fq);
crate::impl_binops_calls!(Fq);
crate::impl_binops_additive!(Fq, Fq);
crate::impl_binops_multiplicative!(Fq, Fq);
crate::field_bits!(Fq);
crate::serialize_deserialize_primefield!(Fq);
crate::impl_from_u64!(Fq);
crate::impl_from_bool!(Fq);

use ff::Field;
const NEGATIVE_ONE: Fq = Fq::ZERO.sub_const(&Fq::ONE);
impl ExtField for Fq {
    const NON_RESIDUE: Self = NEGATIVE_ONE;
    fn mul_by_nonresidue(&self) -> Self {
        self.neg()
    }
    fn frobenius_map(&mut self, _: usize) {}
}

#[cfg(test)]
mod test {
    use super::Fq;
    use crate::{
        arith_test, constants_test, from_uniform_bytes_test, legendre_test, serde_test, test,
    };

    constants_test!(Fq);

    arith_test!(Fq);
    legendre_test!(Fq);
    test!(arith, Fq, sqrt_test, 1000);

    serde_test!(Fq PrimeFieldBits);
    from_uniform_bytes_test!(Fq, 1000, L 64, L 48);
}
