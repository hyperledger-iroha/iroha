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
fn archived_field_paths_delegate_copy_and_context_setup_to_core() {
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
        struct_expansion.contains("decode_context_field_archived_compat::<Opaque>"),
        "the legacy retry path must remain delegated to the shared helper"
    );
    assert!(
        struct_expansion.contains("decode_context_field_canonical::<Opaque>"),
        "ordinary framed struct fields must use the shared canonical helper"
    );
    assert!(
        enum_expansion.contains("decode_context_field_flexible::<Opaque>"),
        "tuple-enum compatibility decoding must use the shared flexible helper"
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
        struct_expansion.contains("payload_range_from_ptr(ptr.wrapping_add(__o),__bitset_len)"),
        "packed-struct bitsets must use the bounded payload helper"
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
}
