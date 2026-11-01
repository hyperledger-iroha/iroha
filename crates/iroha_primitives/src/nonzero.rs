//! Compile-time checked construction of [`NonZero`](core::num::NonZero) integers.

/// Builds a [`NonZero`](core::num::NonZero) integer from a constant expression
/// and rejects zero at compile time.
///
/// The expression is evaluated in an inline `const` block, so it must be a
/// constant. Its integer type comes from a literal suffix or from the
/// surrounding context, as for any integer literal.
///
/// ```
/// use core::num::{NonZeroU32, NonZeroUsize};
///
/// use iroha_primitives::nonzero;
///
/// const LIMIT: NonZeroU32 = nonzero!(16_u32);
/// let batch: NonZeroUsize = nonzero!(4);
/// assert_eq!(LIMIT.get(), 16);
/// assert_eq!(batch.get(), 4);
/// ```
///
/// Zero fails to compile:
///
/// ```compile_fail
/// use core::num::NonZeroU32;
///
/// use iroha_primitives::nonzero;
///
/// const LIMIT: NonZeroU32 = nonzero!(0_u32);
/// ```
#[macro_export]
macro_rules! nonzero {
    ($value:expr $(,)?) => {
        const {
            match ::core::num::NonZero::new($value) {
                ::core::option::Option::Some(value) => value,
                ::core::option::Option::None => {
                    ::core::panic!("nonzero! requires a non-zero value")
                }
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use core::num::{NonZeroI64, NonZeroU8, NonZeroU16, NonZeroU64, NonZeroUsize};

    const SHIFTED: NonZeroU64 = nonzero!(1_u64 << 40);
    static MAXIMUM: NonZeroU16 = nonzero!(u16::MAX);

    #[test]
    fn keeps_suffixed_values() {
        assert_eq!(
            nonzero!(7_u8),
            NonZeroU8::new(7).expect("seven is non-zero")
        );
        let negative: NonZeroI64 = nonzero!(-3_i64);
        assert_eq!(negative.get(), -3);
    }

    #[test]
    fn infers_the_integer_type_from_context() {
        let batch: NonZeroUsize = nonzero!(4);
        assert_eq!(batch.get(), 4);
    }

    #[test]
    fn evaluates_constant_expressions() {
        assert_eq!(SHIFTED.get(), 1 << 40);
        assert_eq!(MAXIMUM.get(), u16::MAX);
        assert_eq!(nonzero!(2_usize.pow(10)).get(), 1024);
        assert_eq!(nonzero!(4 * 1024 * 1024_u64,).get(), 4 * 1024 * 1024);
    }

    #[test]
    fn reads_const_generic_parameters() {
        fn one_more<const N: usize>() -> NonZeroUsize {
            nonzero!(N + 1)
        }
        assert_eq!(one_more::<0>().get(), 1);
        assert_eq!(one_more::<9>().get(), 10);
    }
}
