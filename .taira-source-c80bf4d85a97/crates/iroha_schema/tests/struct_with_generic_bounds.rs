//! `IntoSchema` derive tests for generic bounds.
#[derive(iroha_schema::IntoSchema)]
struct Foo<V> {
    _value: Option<V>,
}

#[test]
fn check_generic() {
    use std::{any::TypeId, collections::BTreeMap};

    use Metadata::*;
    use iroha_schema::prelude::*;

    let option_id = TypeId::of::<::std::option::Option<bool>>();
    let expected_struct = Struct(NamedFieldsMeta {
        declarations: vec![Declaration {
            name: "_value".to_owned(),
            ty: option_id,
        }],
    });
    let expected = vec![
        (
            TypeId::of::<bool>(),
            MetaMapEntry {
                type_id: "bool".to_owned(),
                type_name: "bool".to_owned(),
                metadata: Bool,
            },
        ),
        (
            TypeId::of::<::std::option::Option<bool>>(),
            MetaMapEntry {
                type_id: "Option<bool>".to_owned(),
                type_name: "Option<bool>".to_owned(),
                metadata: Option(TypeId::of::<bool>()),
            },
        ),
        (
            TypeId::of::<Foo<bool>>(),
            MetaMapEntry {
                type_id: "Foo<bool>".to_owned(),
                type_name: "Foo<bool>".to_owned(),
                metadata: expected_struct,
            },
        ),
    ]
    .into_iter()
    .collect::<BTreeMap<_, _>>();
    assert_eq!(Foo::<bool>::schema(), expected);
}
