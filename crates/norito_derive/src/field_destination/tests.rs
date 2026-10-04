//! Derived field-walk registration, type shape and unsupported-contract controls.

use super::*;
use syn::{Data, DeriveInput};

fn expansion(input: DeriveInput) -> TokenStream2 {
    let Data::Struct(record) = &input.data else {
        panic!("record fixture")
    };
    derive(
        &input.ident,
        &input.generics,
        &record.fields,
        &input.attrs,
        &ContainerAttr::parse(&input.attrs).unwrap(),
    )
}

#[test]
fn generated_owned_record_calls_one_destination_walk_with_exact_positional_types() {
    let output = expansion(syn::parse_quote! {
        #[norito(decode_fields,decode_from_slice)]
        struct Example { first:u64, fixed:[u8;32], bytes:Vec<u8> }
    });
    let file: syn::File = syn::parse2(output.clone()).expect("complete generated implementations");
    assert_eq!(
        file.items.len(),
        3,
        "record visitor, owned payload, and requested slice implementation"
    );
    let text = output.to_string();
    assert_eq!(text.matches("fn decode_fields").count(), 1);
    assert_eq!(text.matches("framed_field :: <").count(), 2);
    assert_eq!(text.matches("framed_byte_array_field").count(), 1);
    assert!(text.contains("DecodeField < 0usize , u64 >"));
    assert!(text.contains("DecodeField < 1usize , [u8 ; 32] >"));
    assert!(text.contains("DecodeField < 2usize , Vec < u8 > >"));
    assert!(text.contains("DecodeRecordFields < norito :: core :: OwnedFields >"));
    assert!(
        !text.contains("decode_context_field_canonical"),
        "owned decoder must not retain a parallel field parser"
    );
}

#[test]
fn destination_walk_rejects_unimplemented_whole_value_or_shape_contracts() {
    let cases: [(DeriveInput, &str); 5] = [
        (
            syn::parse_quote! { #[norito(decode_fields)] struct Unit; },
            "closed positional record",
        ),
        (
            syn::parse_quote! { #[norito(decode_fields)] struct Generic<T>{value:T} },
            "without generic parameters",
        ),
        (
            syn::parse_quote! { #[norito(decode_fields,validate="Self::validate")] struct Checked {value:u8} },
            "cannot omit a whole-value validation hook",
        ),
        (
            syn::parse_quote! { #[norito(decode_fields)] struct Skipped { #[norito(skip)] value:u8 } },
            "skip and flatten are unsupported",
        ),
        (
            syn::parse_quote! { #[norito(decode_fields)] struct Flat { #[norito(flatten)] value:u8 } },
            "skip and flatten are unsupported",
        ),
    ];
    for (input, message) in cases {
        let tokens = expansion(input).to_string();
        assert!(tokens.contains("compile_error"));
        assert!(tokens.contains(message), "{tokens}");
    }
}

#[test]
fn destination_flag_rejects_duplicate_and_valued_spellings() {
    for (input, expected) in [
        (
            syn::parse_quote! {#[norito(decode_fields,decode_fields)] struct R{}},
            "duplicate decode_fields attribute",
        ),
        (
            syn::parse_quote! {#[norito(decode_fields=true)] struct R{}},
            "decode_fields does not take a value",
        ),
    ] {
        let input: DeriveInput = input;
        assert_eq!(
            ContainerAttr::parse(&input.attrs)
                .err()
                .unwrap()
                .to_string(),
            expected
        );
    }
}

#[test]
fn generated_tuple_uses_sole_positional_walk_and_original_archived_slice_contract() {
    let output = expansion(
        syn::parse_quote! { #[norito(decode_fields,decode_from_slice)] struct Tuple(u32,[u8;4],u64); },
    );
    let file: syn::File = syn::parse2(output.clone()).expect("complete tuple implementations");
    assert_eq!(file.items.len(), 3);
    let text = output.to_string();
    assert_eq!(text.matches("fn decode_fields").count(), 1);
    assert_eq!(text.matches("framed_field :: <").count(), 2);
    assert_eq!(text.matches("framed_byte_array_field").count(), 1);
    assert!(text.contains("DecodeField < 0usize , u32 >"));
    assert!(text.contains("DecodeField < 1usize , [u8 ; 4] >"));
    assert!(text.contains("DecodeField < 2usize , u64 >"));
    assert!(text.contains("Self (__field_0 , __field_1 , __field_2 ,)"));
    assert!(text.contains("Ok ((value , __logical_len))"));
    assert!(!text.contains("decode_context_field_canonical"));
    assert!(!text.contains("decode_prepared_slice_prefix"));
}
