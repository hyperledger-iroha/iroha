use ff::PrimeField;
use num_bigint::BigUint;
use num_traits::Num;
use std::borrow::Cow;

pub mod curve;
pub mod field;

pub(crate) fn fe_from_str<F: PrimeField>(string: impl AsRef<str>) -> F {
    let string = string.as_ref();
    let oct = if let Some(hex) = string.strip_prefix("0x") {
        Cow::Owned(BigUint::from_str_radix(hex, 16).unwrap().to_string())
    } else {
        Cow::Borrowed(string)
    };
    F::from_str_vartime(&oct).unwrap()
}
