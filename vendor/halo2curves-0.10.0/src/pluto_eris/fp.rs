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
    pluto_eris_fp,
    Fp,
    modulus = "24000000000024000130e0000d7f70e4a803ca76f439266f443f9a5cda8a6c7be4a7a5fe8fadffd6a2a7e8c30006b9459ffffcd300000001",
    mul_gen = "a",
    zeta = "480000000000360001c950000d7ee0e4a803c956d01c903d720dc8ad8b38dffaf50c100004c37ffffffe",
    from_uniform = [64, 72, 112],
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

impl ExtField for Fp {
    const NON_RESIDUE: Self = Fp::from_raw([
        0x9ffffcd2fffffffc,
        0xa2a7e8c30006b945,
        0xe4a7a5fe8fadffd6,
        0x443f9a5cda8a6c7b,
        0xa803ca76f439266f,
        0x0130e0000d7f70e4,
        0x2400000000002400,
    ]);
    fn mul_by_nonresidue(&self) -> Self {
        (self.double().double() + self).neg()
    }
    fn frobenius_map(&mut self, _: usize) {}
}

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
    from_uniform_bytes_test!(Fp, 1000, L 64, L 72, L 112);
}
