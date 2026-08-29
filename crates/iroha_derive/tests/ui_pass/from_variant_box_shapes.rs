//! Verifies that `Box`-named field types are treated as exact types.

mod no_arguments {
    struct Box;

    #[derive(iroha_derive::FromVariant)]
    enum Example {
        Value(Box),
    }
}

mod one_argument {
    struct Box<T>(T);

    impl<T> Box<T> {
        fn new(value: T) -> Self {
            Self(value)
        }
    }

    #[derive(iroha_derive::FromVariant)]
    enum Example {
        Value(Box<u8>),
    }

    fn accepts_exact_field_type() {
        let _: Example = Box::new(1_u8).into();
        assert!(impls::impls!(Example: !From<u8>));
    }
}

mod two_arguments {
    struct Box<A, B>(core::marker::PhantomData<(A, B)>);

    #[derive(iroha_derive::FromVariant)]
    enum Example {
        Value(Box<u8, u16>),
    }
}

mod dynamically_sized {
    trait Value {}

    #[derive(iroha_derive::FromVariant)]
    enum Example {
        Value(Box<dyn Value>),
    }
}

fn main() {}
