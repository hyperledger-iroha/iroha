// Exact nominal durable-state projection and public call-boundary roundtrips.

#[test]
fn contract_state_nominal_nested_options_preserve_none_and_some_unit() {
    use iroha_data_model::smart_contract::entrypoint::{
        EntrypointArgumentFieldV1, EntrypointArgumentSchemaV1, EntrypointValueAtomV1 as PublicAtom,
        EntrypointValueTypeNodeV1 as Node, EntrypointValueTypeV1,
    };
    use ivm::{EmbeddedStateType as Type, state_value::StateValueAtomV1 as Atom};
    let ty = Type::Option(Box::new(Type::Option(Box::new(Type::Unit))));
    let schema = EntrypointArgumentSchemaV1 {
        fields: vec![EntrypointArgumentFieldV1 {
            name: "value".to_owned(),
            ty: EntrypointValueTypeV1 {
                nodes: vec![Node::Option, Node::Option, Node::Unit],
            },
        }],
    };
    for (atoms, expected_json, expected_atoms) in [
        (
            vec![Atom::Tag(false)],
            "{\"none\":true}",
            vec![PublicAtom::Tag(false)],
        ),
        (
            vec![Atom::Tag(true), Atom::Tag(false)],
            "{\"some\":{\"none\":true}}",
            vec![PublicAtom::Tag(true), PublicAtom::Tag(false)],
        ),
        (
            vec![Atom::Tag(true), Atom::Tag(true), Atom::Unit],
            "{\"some\":{\"some\":null}}",
            vec![
                PublicAtom::Tag(true),
                PublicAtom::Tag(true),
                PublicAtom::Unit,
            ],
        ),
    ] {
        let record = make_state_record(&ty, atoms);
        let projected = decode_contract_state_scalar_json(&record, &ty).unwrap();
        assert_eq!(projected.get(), expected_json);
        let payload =
            IrohaJson::from_raw_json(format!("{{\"value\":{}}}", projected.get())).unwrap();
        assert_eq!(
            ivm::argument_record_from_json(&schema, &payload)
                .unwrap()
                .atoms,
            expected_atoms
        );
    }
    for malformed in [
        "null",
        "{\"some\":null}",
        "{\"none\":false}",
        "{\"none\":true,\"some\":{\"none\":true}}",
    ] {
        let payload = IrohaJson::from_raw_json(format!("{{\"value\":{malformed}}}")).unwrap();
        assert!(ivm::argument_record_from_json(&schema, &payload).is_err());
    }
}

fn nominal_state_cursor_fixture() -> iroha_data_model::smart_contract::state_cursor::StateCursorV1 {
    use iroha_data_model::smart_contract::{
        entrypoint::EntrypointValueKindV1 as Kind, state_cursor::StateCursorV1,
    };
    let suffix = contract_state_stored_map_key_suffix(&ivm::EmbeddedStateType::Bool, "true")
        .expect("canonical bool map key");
    StateCursorV1 {
        instance: "local::projection".to_owned(),
        map: "Balances".parse().unwrap(),
        schema_hash: [7; 32],
        key_type: Kind::Bool,
        last_key: format!("Balances/{suffix}").parse().unwrap(),
    }
}

#[test]
fn contract_state_nominal_products_roundtrip_through_public_argument_json() {
    use iroha_data_model::smart_contract::entrypoint::{
        EntrypointArgumentFieldV1, EntrypointArgumentSchemaV1, EntrypointListTypeNodeV1,
        EntrypointStructTypeNodeV1, EntrypointValueAtomV1 as PublicAtom,
        EntrypointValueTypeNodeV1 as Node, EntrypointValueTypeV1,
    };
    use ivm::state_value::StateValueAtomV1 as Atom;
    use ivm::{EmbeddedStateFieldDescriptor as Field, EmbeddedStateType as Type};
    let error = ivm::error_types::list_error_type();
    let name = "std/math@1.0.0::Math::Receipt";
    let fields = vec!["completed".to_owned(), "attempts".to_owned()];
    let ty = Type::Struct {
        name: name.to_owned(),
        fields: vec![
            Field {
                name: fields[0].clone(),
                ty: Type::Unit,
            },
            Field {
                name: fields[1].clone(),
                ty: Type::List {
                    capacity: 4,
                    element: Box::new(Type::Result {
                        ok: Box::new(Type::Unit),
                        err: Box::new(Type::Error(error.clone())),
                    }),
                },
            },
        ],
    };
    let record = make_state_record(
        &ty,
        vec![
            Atom::Unit,
            Atom::List(vec![
                vec![Atom::Tag(true), Atom::Unit],
                vec![Atom::Tag(false), Atom::ErrorCode(1)],
                vec![Atom::Tag(false), Atom::ErrorCode(2)],
            ]),
        ],
    );
    let projected = decode_contract_state_scalar_json(&record, &ty).expect("project exact product");
    assert_eq!(
        projected.get(),
        "{\"attempts\":[{\"ok\":null},{\"err\":\"IndexOutOfBounds\"},{\"err\":\"CapacityExceeded\"}],\"completed\":null}"
    );
    let schema = EntrypointArgumentSchemaV1 {
        fields: vec![EntrypointArgumentFieldV1 {
            name: "receipt".to_owned(),
            ty: EntrypointValueTypeV1 {
                nodes: vec![
                    Node::Struct(EntrypointStructTypeNodeV1 {
                        name: name.to_owned(),
                        fields,
                    }),
                    Node::Unit,
                    Node::List(EntrypointListTypeNodeV1 { capacity: 4 }),
                    Node::Result,
                    Node::Unit,
                    Node::Error(error),
                ],
            },
        }],
    };
    let payload = IrohaJson::from_raw_json(format!("{{\"receipt\":{}}}", projected.get())).unwrap();
    let arguments = ivm::argument_record_from_json(&schema, &payload)
        .expect("projected state is valid public call JSON");
    assert_eq!(
        arguments.atoms,
        vec![
            PublicAtom::Unit,
            PublicAtom::List(3),
            PublicAtom::Tag(true),
            PublicAtom::Unit,
            PublicAtom::Tag(false),
            PublicAtom::ErrorCode(1),
            PublicAtom::Tag(false),
            PublicAtom::ErrorCode(2),
        ]
    );
    for malformed in [
        "{\"receipt\":{\"attempts\":[],\"completed\":0}}",
        "{\"receipt\":{\"attempts\":[{\"err\":1}],\"completed\":null}}",
        "{\"receipt\":{\"attempts\":[{\"err\":\"Other::IndexOutOfBounds\"}],\"completed\":null}}",
    ] {
        let payload = IrohaJson::from_raw_json(malformed.to_owned()).unwrap();
        assert!(ivm::argument_record_from_json(&schema, &payload).is_err());
    }
}

#[test]
fn contract_state_nominal_identity_schema_and_codes_are_exact() {
    use ivm::{EmbeddedStateType as Type, state_value::StateValueAtomV1 as Atom};
    let descriptor = ivm::error_types::list_error_type();
    let ty = Type::Error(descriptor.clone());
    let record = make_state_record(&ty, vec![Atom::ErrorCode(1)]);
    assert_eq!(
        decode_contract_state_scalar_json(&record, &ty)
            .unwrap()
            .get(),
        "\"IndexOutOfBounds\""
    );
    let mut wrong_identity = descriptor.clone();
    wrong_identity.identity = "other::ListError".to_owned();
    let mut wrong_schema = descriptor;
    wrong_schema.variants[0].name = "DifferentVariant".to_owned();
    for wrong in [
        Type::Error(wrong_identity),
        Type::Error(wrong_schema),
        Type::Unit,
    ] {
        assert!(decode_contract_state_scalar_json(&record, &wrong).is_err());
    }
    let zero_code = ivm::state_value::StateValueRecordV1 {
        schema_hash: [0; 32],
        atoms: vec![Atom::ErrorCode(0)],
    };
    assert!(
        norito::to_bytes(&zero_code).is_err(),
        "zero error codes have no canonical record"
    );
    for atom in [Atom::ErrorCode(99), Atom::Bool(true), Atom::Unit] {
        let record = make_unchecked_state_record(&ty, vec![atom]);
        assert!(decode_contract_state_scalar_json(&record, &ty).is_err());
    }
    for atom in [Atom::Bool(false), Atom::Bool(true), Atom::ErrorCode(1)] {
        let record = make_unchecked_state_record(&Type::Unit, vec![atom]);
        assert!(decode_contract_state_scalar_json(&record, &Type::Unit).is_err());
    }
    let unit = make_state_record(&Type::Unit, vec![Atom::Unit]);
    let flags = norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
    let _alternate = norito::core::DecodeFlagsGuard::enter(flags);
    assert_eq!(
        decode_contract_state_scalar_json(&unit, &Type::Unit)
            .unwrap()
            .get(),
        "null"
    );
    assert_eq!(
        decode_contract_state_scalar_json(&record, &ty)
            .unwrap()
            .get(),
        "\"IndexOutOfBounds\""
    );
}

#[test]
fn contract_state_nominal_page_cursor_roundtrips_without_erasing_key_kind() {
    use iroha_data_model::smart_contract::entrypoint::{
        EntrypointArgumentFieldV1, EntrypointArgumentSchemaV1, EntrypointListTypeNodeV1,
        EntrypointStructTypeNodeV1, EntrypointValueAtomV1 as PublicAtom,
        EntrypointValueKindV1 as Kind, EntrypointValueTypeNodeV1 as Node, EntrypointValueTypeV1,
    };
    use ivm::state_value::StateValueAtomV1 as Atom;
    use ivm::{EmbeddedStateFieldDescriptor as Field, EmbeddedStateType as Type};
    let frame = nominal_state_cursor_fixture().encode_frame().unwrap();
    let envelope = make_tlv(PointerType::NoritoBytes, &frame);
    let ty = Type::Struct {
        name: "StatePage".to_owned(),
        fields: vec![
            Field {
                name: "items".to_owned(),
                ty: Type::List {
                    capacity: 2,
                    element: Box::new(Type::Tuple(vec![Type::Bool, Type::Unit])),
                },
            },
            Field {
                name: "next".to_owned(),
                ty: Type::Option(Box::new(Type::StateCursor(Kind::Bool))),
            },
        ],
    };
    let record = make_state_record(
        &ty,
        vec![
            Atom::List(vec![vec![Atom::Bool(true), Atom::Unit]]),
            Atom::Tag(true),
            Atom::Pointer(envelope.clone()),
        ],
    );
    let projected = decode_contract_state_scalar_json(&record, &ty).unwrap();
    assert_eq!(
        projected.get(),
        &format!(
            "{{\"items\":[[true,null]],\"next\":{{\"some\":\"0x{}\"}}}}",
            hex::encode(&frame)
        )
    );
    let schema = EntrypointArgumentSchemaV1 {
        fields: vec![EntrypointArgumentFieldV1 {
            name: "page".to_owned(),
            ty: EntrypointValueTypeV1 {
                nodes: vec![
                    Node::Struct(EntrypointStructTypeNodeV1 {
                        name: "StatePage".to_owned(),
                        fields: vec!["items".to_owned(), "next".to_owned()],
                    }),
                    Node::List(EntrypointListTypeNodeV1 { capacity: 2 }),
                    Node::Tuple(2),
                    Node::Leaf(Kind::Bool),
                    Node::Unit,
                    Node::Option,
                    Node::StateCursor(Kind::Bool),
                ],
            },
        }],
    };
    let payload = IrohaJson::from_raw_json(format!("{{\"page\":{}}}", projected.get())).unwrap();
    let arguments = ivm::argument_record_from_json(&schema, &payload).unwrap();
    assert_eq!(
        arguments.atoms,
        vec![
            PublicAtom::List(1),
            PublicAtom::Bool(true),
            PublicAtom::Unit,
            PublicAtom::Tag(true),
            PublicAtom::Pointer(envelope),
        ]
    );
    let mut malformed = nominal_state_cursor_fixture();
    malformed.key_type = Kind::Int;
    let malformed = malformed.encode_frame().unwrap();
    let payload = IrohaJson::from_raw_json(format!(
        "{{\"page\":{{\"items\":[],\"next\":{{\"some\":\"0x{}\"}}}}}}",
        hex::encode(malformed)
    ))
    .unwrap();
    assert!(ivm::argument_record_from_json(&schema, &payload).is_err());
}

#[test]
fn contract_state_nominal_cursor_rejects_wrong_pointer_key_and_malformed_frames() {
    use iroha_data_model::smart_contract::entrypoint::EntrypointValueKindV1 as Kind;
    use ivm::{EmbeddedStateType as Type, state_value::StateValueAtomV1 as Atom};
    let ty = Type::StateCursor(Kind::Bool);
    let cursor = nominal_state_cursor_fixture();
    let frame = cursor.encode_frame().unwrap();
    let valid = make_state_record(
        &ty,
        vec![Atom::Pointer(make_tlv(PointerType::NoritoBytes, &frame))],
    );
    assert!(decode_contract_state_scalar_json(&valid, &ty).is_ok());
    let mut wrong_key = cursor.clone();
    wrong_key.key_type = Kind::Int;
    let mut wrong_map = cursor.clone();
    wrong_map.last_key = "Other/00".parse().unwrap();
    let mut empty_instance = cursor;
    empty_instance.instance.clear();
    let mut trailing = frame.clone();
    trailing.push(0);
    for (pointer, malformed) in [
        (PointerType::Blob, frame),
        (PointerType::NoritoBytes, wrong_key.encode_frame().unwrap()),
        (
            PointerType::NoritoBytes,
            norito::encode_canonical(&wrong_map).unwrap(),
        ),
        (
            PointerType::NoritoBytes,
            norito::encode_canonical(&empty_instance).unwrap(),
        ),
        (PointerType::NoritoBytes, trailing),
    ] {
        let record =
            make_unchecked_state_record(&ty, vec![Atom::Pointer(make_tlv(pointer, &malformed))]);
        assert!(decode_contract_state_scalar_json(&record, &ty).is_err());
    }
}
