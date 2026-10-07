//! Witness-only square roots. Every claimed root is separately constrained.
use super::super::{
    extension::Fp2,
    native::{self, Fp},
};

pub(super) fn mul(a: &Fp2, b: &Fp2) -> Fp2 {
    [
        native::sub(&native::mul(&a[0], &b[0]), &native::mul(&a[1], &b[1])),
        native::add(&native::mul(&a[0], &b[1]), &native::mul(&a[1], &b[0])),
    ]
}
fn sqrt_fp(a: &Fp) -> Option<Fp> {
    const EXPONENT: Fp = [
        0xee7fbfffffffeaab,
        0x07aaffffac54ffff,
        0xd9cc34a83dac3d89,
        0xd91dd2e13ce144af,
        0x92c6e9ed90d2eb35,
        0x0680447a8e5ff9a6,
    ];
    let mut x = native::ONE;
    for limb in EXPONENT.iter().rev() {
        for bit in (0..64).rev() {
            x = native::square(&x);
            if (limb >> bit) & 1 == 1 {
                x = native::mul(&x, a);
            }
        }
    }
    (native::square(&x) == *a).then_some(x)
}
/// A root of a quadratic-extension element, if it has one.
pub(super) fn sqrt(a: &Fp2) -> Option<Fp2> {
    if a[1] == native::ZERO {
        if let Some(root) = sqrt_fp(&a[0]) {
            return Some([root, native::ZERO]);
        }
        return sqrt_fp(&native::neg(&a[0])).map(|root| [native::ZERO, root]);
    }
    let norm = native::add(&native::square(&a[0]), &native::square(&a[1]));
    let alpha = sqrt_fp(&norm)?;
    const HALF: Fp = [
        0xdcff7fffffffd556,
        0x0f55ffff58a9ffff,
        0xb39869507b587b12,
        0xb23ba5c279c2895f,
        0x258dd3db21a5d66b,
        0x0d0088f51cbff34d,
    ];
    let first = native::mul(&native::add(&a[0], &alpha), &HALF);
    let x = if let Some(root) = sqrt_fp(&first) {
        root
    } else {
        sqrt_fp(&native::mul(&native::sub(&a[0], &alpha), &HALF))?
    };
    let y = native::mul(&a[1], &native::invert(&native::add(&x, &x)));
    let root = [x, y];
    (mul(&root, &root) == *a).then_some(root)
}
