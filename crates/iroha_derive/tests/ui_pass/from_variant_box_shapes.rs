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
    }

    // `From<u8>` must not be derived for the local `Box<u8>` field. The
    // inherent constant only applies when the bound holds and wins over the
    // trait fallback, so this fails to compile if the impl appears.
    trait LacksFromU8 {
        const IMPLEMENTS: bool = false;
    }
    impl<T: ?Sized> LacksFromU8 for T {}
    struct Probe<T: ?Sized>(core::marker::PhantomData<T>);
    #[allow(dead_code)]
    impl<T: ?Sized + From<u8>> Probe<T> {
        const IMPLEMENTS: bool = true;
    }
    const _: () = assert!(!<Probe<Example>>::IMPLEMENTS);
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
