Construct a matching enum with references in place of enum variant fields

# Example

```
mod model {
    use iroha_data_model_derive::EnumRef;
    use norito::codec::Encode;

    #[derive(EnumRef)]
    #[enum_ref(derive(Encode))]
    pub enum InnerEnum {
        A(u32),
        B(i32),
    }

    #[derive(EnumRef)]
    #[enum_ref(derive(Encode))]
    pub enum OuterEnum {
        A(String),
        #[enum_ref(transparent)]
        B(InnerEnum),
    }
}

/* will produce:
mod model {
    #[derive(Encode)]
    pub(super) enum InnerEnumRef<'a> {
        A(&'a u32),
        B(&'a i32),
    }

    #[derive(Encode)]
    pub(super) enum OuterEnumRef<'a> {
        A(&'a String),
        B(InnerEnumRef<'a>),
    }
}
*/
```
