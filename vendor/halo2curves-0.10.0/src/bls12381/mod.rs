mod engine;
mod fq;
mod fq12;
mod fq2;
mod fq6;
mod fr;
mod g1;
mod g2;

pub use engine::*;
pub use fq::*;
pub use fq12::*;
pub use fq2::*;
pub use fq6::*;
pub use fr::*;
pub use g1::*;
pub use g2::*;

const BLS_X: [u8; 64] = [
    1, 1, 0, 1, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

const ENDO_PARAMS: EndoParameters = EndoParameters {
    // round(b2/n)
    gamma2: [0x63f6e522f6cfee30u64, 0x7c6becf1e01faadd, 0x01, 0x0],
    // round(-b1/n)
    gamma1: [0x02u64, 0x0, 0x0, 0x0],
    b1: [0x01u64, 0x0, 0x0, 0x0],
    b2: [0x0000000100000000, 0xac45a4010001a402, 0x0, 0x0],
};

use ff::{PrimeField, WithSmallOrderMulGroup};

use crate::arithmetic::{mul_512, sbb, CurveEndo, EndoParameters};
crate::endo!(G1, Fr, ENDO_PARAMS);
crate::endo!(G2, Fr, ENDO_PARAMS);
