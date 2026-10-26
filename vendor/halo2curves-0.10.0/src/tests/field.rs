#[cfg(test)]
pub(crate) mod arith;

#[cfg(test)]
pub(crate) mod constants;

#[cfg(test)]
#[macro_use]
pub(crate) mod extensions;

#[cfg(test)]
pub(crate) mod legendre;

#[cfg(test)]
pub(crate) mod serde;

#[macro_export]
macro_rules! test {
    ($mod: ident, $field:ident, $test:ident, $size:expr) => {
        #[test]
        fn $test() {
            use rand_core::SeedableRng;
            use rand_xorshift::XorShiftRng;

            use super::*;
            let mut rng = XorShiftRng::from_seed($crate::tests::SEED);
            $crate::tests::field::$mod::$test::<$field>(&mut rng, $size);
        }
    };
    ($mod: ident, $field:ident, $test:ident) => {
        #[test]
        fn $test() {
            use rand_core::SeedableRng;
            use rand_xorshift::XorShiftRng;
            let mut rng = XorShiftRng::from_seed($crate::tests::SEED);
            $crate::tests::field::$mod::$test::<$field>(&mut rng);
        }
    };
}
