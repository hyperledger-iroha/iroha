//! Schema metadata test for tuple structs and unnamed field handling.

use std::{any::TypeId, collections::BTreeMap};

use iroha_schema::prelude::*;
use norito::{Decode, Encode};

#[derive(IntoSchema, Encode, Decode)]
struct Command(String, Vec<String>, #[codec(skip)] bool);

#[test]
fn unnamed() {
    use Metadata::*;

    let expected = vec![
        (
            TypeId::of::<::std::string::String>(),
            MetaMapEntry {
                type_id: "String".to_owned(),
                type_name: "String".to_owned(),
                metadata: String,
            },
        ),
        (
            TypeId::of::<::std::vec::Vec<::std::string::String>>(),
            MetaMapEntry {
                type_id: "Vec<String>".to_owned(),
                type_name: "Vec<String>".to_owned(),
                metadata: Vec(VecMeta {
                    ty: TypeId::of::<::std::string::String>(),
                }),
            },
        ),
        (
            TypeId::of::<Command>(),
            MetaMapEntry {
                type_id: "Command".to_owned(),
                type_name: "Command".to_owned(),
                metadata: Tuple(UnnamedFieldsMeta {
                    types: vec![
                        TypeId::of::<::std::string::String>(),
                        TypeId::of::<::std::vec::Vec<::std::string::String>>(),
                    ],
                }),
            },
        ),
    ]
    .into_iter()
    .collect::<BTreeMap<_, _>>();
    assert_eq!(Command::schema(), expected);
}
