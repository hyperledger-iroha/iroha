//! Compile-test exercising the `FromVariant` derive attributes.
use impls::impls;
struct Variant1;
struct Variant2;
struct Variant3;
struct Variant4;
#[allow(unused)]
#[derive(iroha_derive::FromVariant)]
enum Enum {
    Variant1(Box<Variant1>),
    Variant2(#[skip_from] Box<Variant2>),
    Variant3(#[skip_try_from] Box<Variant3>),
    Variant4(
        #[skip_from]
        #[skip_try_from]
        Box<Variant4>,
    ),
}
macro_rules! check_variant {
    ($variant:ty, $skip_from:expr, $skip_try_from:expr) => {
        if $skip_from {
            assert!(impls!(Enum: !From<$variant>), "Enum implements From<{}>, but #[skip_from] was specified", stringify!($variant));
        } else {
            assert!(impls!(Enum: From<$variant>), "Enum does not implement From<{}>, but #[skip_from] was not specified", stringify!($variant));
        }
        if $skip_try_from {
            assert!(impls!($variant: !TryFrom<Enum>), "{} implements TryFrom<Enum>, but #[skip_try_from] was specified", stringify!($variant));
        } else {
            assert!(impls!($variant: TryFrom<Enum>), "{} does not implement TryFrom<Enum>, but #[skip_try_from] was not specified", stringify!($variant));
        }
    };
}
#[test]
fn conversion_controls_disable_only_the_selected_implementation() {
    check_variant!(Box<Variant1>, false, false);
    check_variant!(Box<Variant2>, true, false);
    check_variant!(Box<Variant3>, false, true);
    check_variant!(Box<Variant4>, true, true);
}
