use super::*;
fn compact(tokens: TokenStream2) -> String {
    tokens
        .to_string()
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect()
}
#[test]
fn packed_field_bitset_matches_named_and_unnamed_layouts() {
    let named: DeriveInput = syn::parse_quote! {
        struct Named {
            fixed: u32,
            framed: Opaque,
            self_delimiting: Vec<u8>,
        }
    };
    let Data::Struct(named_data) = named.data else {
        unreachable!("test input is a struct");
    };
    assert_eq!(packed_field_bitset(&named_data.fields), vec![0b0000_0010]);
    let unnamed: DeriveInput = syn::parse_quote! {
        struct Unnamed(u32, Opaque, Vec<u8>);
    };
    let Data::Struct(unnamed_data) = unnamed.data else {
        unreachable!("test input is a struct");
    };
    assert_eq!(packed_field_bitset(&unnamed_data.fields), vec![0b0000_0010]);
}
#[test]
fn context_field_paths_delegate_copy_and_context_setup_to_core() {
    let struct_input: DeriveInput = syn::parse_quote! {
        struct Record {
            opaque: Opaque,
        }
    };
    let Data::Struct(struct_data) = &struct_input.data else {
        unreachable!("test input is a struct");
    };
    let struct_expansion = compact(derive_struct_deserialize(
        &struct_input.ident,
        &struct_input.generics,
        &struct_data.fields,
        &struct_input.attrs,
        None,
    ));
    let tuple_input: DeriveInput = syn::parse_quote! {
        struct Tuple(Opaque);
    };
    let Data::Struct(tuple_data) = &tuple_input.data else {
        unreachable!("test input is a struct");
    };
    let tuple_expansion = compact(derive_struct_deserialize(
        &tuple_input.ident,
        &tuple_input.generics,
        &tuple_data.fields,
        &tuple_input.attrs,
        None,
    ));
    let enum_input: DeriveInput = syn::parse_quote! {
        enum Message {
            Tuple(Opaque, u64),
            Named { values: Vec<u32> },
        }
    };
    let Data::Enum(enum_data) = &enum_input.data else {
        unreachable!("test input is an enum");
    };
    let enum_expansion = compact(derive_enum_deserialize(
        &enum_input.ident,
        &enum_input.generics,
        enum_data,
        &enum_input.attrs,
        None,
    ));
    for expansion in [&struct_expansion, &enum_expansion] {
        assert!(
            expansion.contains("norito::core::decode_context_field_"),
            "generated decoder must call the shared context-field helpers"
        );
        assert!(
            !expansion.contains("std::alloc::alloc("),
            "generated decoder must not inline archived-field allocation"
        );
        assert!(
            !expansion.contains("PayloadCtxGuard::enter(tmp_slice)"),
            "generated decoder must not inline archived-field context setup"
        );
    }
    assert!(
        struct_expansion.contains("decode_context_field_fixed_canonical::<Opaque>"),
        "packed framed struct fields must use the exact canonical helper"
    );
    assert!(
        struct_expansion.contains("decode_context_field_canonical::<Opaque>"),
        "ordinary framed struct fields must use the shared canonical helper"
    );
    assert!(
        enum_expansion.contains("decode_context_field_canonical::<Opaque>"),
        "framed tuple-enum fields must use the exact canonical helper"
    );
    for retired_helper in [
        "decode_context_field_canonical_or_archived",
        "decode_context_field_archived::<",
        "decode_context_field_archived_compat",
        "decode_context_field_fixed_archived",
    ] {
        for expansion in [&struct_expansion, &tuple_expansion, &enum_expansion] {
            assert!(
                !expansion.contains(retired_helper),
                "generated decoders must not retry retired field encodings via {retired_helper}"
            );
        }
    }
    assert!(
        !enum_expansion.contains("decode_context_field_flexible"),
        "enum fields must not consume bytes beyond their declared frame"
    );
    assert!(
        enum_expansion.contains("decode_context_field_prefix::<Vec<u32>>"),
        "self-delimiting Vec fields must retain their consumed-prefix path"
    );
    for expansion in [&struct_expansion, &enum_expansion] {
        assert!(
            expansion.contains("finish_context_fields(ptr,offset)"),
            "full-consumption validation must remain shared"
        );
    }
    assert!(
        struct_expansion.contains("payload_range_from_ptr(ptr.wrapping_add(__o),__bitset_len,)"),
        "packed-struct bitsets must use the bounded payload helper; expansion: \
         {struct_expansion}"
    );
    for expansion in [&struct_expansion, &tuple_expansion] {
        assert!(
            expansion.contains("NonCanonicalEncoding"),
            "packed-struct decoders must validate the wire bitset"
        );
        assert!(
            expansion.contains("payload_slice_from_ptr(ptr)"),
            "size headers must be read from the bounded payload slice"
        );
        assert!(
            expansion.contains("read_len_dyn_slice(__size_bytes)"),
            "size headers must use the fallible slice decoder"
        );
        assert!(
            !expansion.contains("try_read_len_ptr_unchecked"),
            "generated size-header loops must not perform pointer reads"
        );
        assert!(
            !expansion.contains("__fallback"),
            "a compact zero length must never be reinterpreted as fixed-width"
        );
    }
    assert!(
        enum_expansion.contains("payload_range_from_ptr(ptr,4)"),
        "enum tags must use the bounded payload helper"
    );
    for struct_only_token in [
        "decode_packed_offsets_slice",
        "__sizes",
        "__bitset",
        "__packed_data_len",
    ] {
        assert!(
            !enum_expansion.contains(struct_only_token),
            "enum codegen must not inherit packed-struct offset/size loops: \
             {struct_only_token}"
        );
    }
}
#[test]
fn binary_default_attributes_do_not_generate_missing_field_fallbacks() {
    let struct_input: DeriveInput = syn::parse_quote! {
        struct Record {
            #[norito(default)]
            count: u32,
            #[norito(default = "custom_default")]
            marker: u64,
        }
    };
    let Data::Struct(struct_data) = &struct_input.data else {
        unreachable!("test input is a struct");
    };
    let struct_expansion = compact(derive_struct_deserialize(
        &struct_input.ident,
        &struct_input.generics,
        &struct_data.fields,
        &struct_input.attrs,
        None,
    ));
    let enum_input: DeriveInput = syn::parse_quote! {
        enum Message {
            Values {
                #[norito(default)]
                count: u32,
                #[norito(default = "custom_default")]
                marker: u64,
            },
        }
    };
    let Data::Enum(enum_data) = &enum_input.data else {
        unreachable!("test input is an enum");
    };
    let enum_expansion = compact(derive_enum_deserialize(
        &enum_input.ident,
        &enum_input.generics,
        enum_data,
        &enum_input.attrs,
        None,
    ));
    for expansion in [&struct_expansion, &enum_expansion] {
        assert!(
            expansion.contains("decode_context_field_canonical::<"),
            "default-annotated binary fields must use the canonical decoder"
        );
        assert!(
            !expansion.contains("custom_default") && !expansion.contains("Default::default"),
            "binary deserializers must not synthesize omitted default-annotated fields"
        );
        assert!(
            !expansion.contains("Err(norito::core::Error::LengthMismatch)=>"),
            "binary length mismatches must remain terminal"
        );
    }
}
#[test]
fn ordinary_struct_fields_use_verified_exact_length_streaming() {
    let input: DeriveInput = syn::parse_quote! {
        struct Envelope {
            named: Vec<u8>,
            other: String,
        }
    };
    let Data::Struct(data) = &input.data else {
        unreachable!();
    };
    let expansion = compact(derive_struct_serialize(
        &input.ident,
        &input.generics,
        &data.fields,
        &input.attrs,
        None,
    ));
    assert_eq!(expansion.matches("write_len_prefixed_exact(").count(), 2);
    assert!(!expansion.contains("write_len_prefixed("));
}
#[test]
fn packed_struct_codegen_counts_then_streams_without_field_payload_buffers() {
    let input: DeriveInput = syn::parse_quote! {
        struct Envelope {
            named: Vec<u8>,
            other: String,
        }
    };
    let Data::Struct(data) = &input.data else {
        unreachable!();
    };
    let expansion = compact(derive_struct_serialize(
        &input.ident,
        &input.generics,
        &data.fields,
        &input.attrs,
        None,
    ));
    assert!(expansion.contains("encoded_payload_len("));
    assert!(expansion.contains("serialize_to_writer_exact("));
    assert!(!expansion.contains("serialize_to_buffer("));
    assert!(!expansion.contains("__field_bufs"));
}
#[test]
fn ordinary_enum_fields_use_verified_exact_length_streaming() {
    let input: DeriveInput = syn::parse_quote! {
        enum Envelope {
            Tuple(Vec<u8>),
            Named { payload: Vec<u8> },
        }
    };
    let Data::Enum(data) = &input.data else {
        unreachable!();
    };
    let expansion = compact(derive_enum_serialize(
        &input.ident,
        &input.generics,
        data,
        &input.attrs,
        None,
    ));
    assert!(expansion.matches("write_len_prefixed_exact(").count() >= 2);
    assert!(!expansion.contains("write_len_prefixed("));
}
#[test]
fn enum_byte_array_lengths_use_the_raw_wire_width() {
    let input: DeriveInput = syn::parse_quote! {
        enum Envelope {
            Tuple([u8; 32]),
            Named { digest: [u8; 32] },
        }
    };
    let Data::Enum(data) = &input.data else {
        unreachable!();
    };
    let expansion = compact(derive_enum_serialize(
        &input.ident,
        &input.generics,
        data,
        &input.attrs,
        None,
    ));
    assert_eq!(
        expansion.matches("core::mem::size_of_val(field0)").count(),
        3,
        "tuple byte arrays must use their raw width in serialization and both length oracles"
    );
    assert_eq!(
        expansion.matches("core::mem::size_of_val(digest)").count(),
        3,
        "named byte arrays must use their raw width in serialization and both length oracles"
    );
    for incorrect in [
        "encoded_len_hint(field0)",
        "encoded_len_exact(field0)",
        "encoded_len_hint(digest)",
        "encoded_len_exact(digest)",
    ] {
        assert!(
            !expansion.contains(incorrect),
            "byte-array length oracle delegated to the generic array codec: {incorrect}"
        );
    }
}
