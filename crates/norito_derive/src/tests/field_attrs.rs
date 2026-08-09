//! Field-attribute parser and validation tests.

use super::*;

#[test]
fn needs_size_attribute_is_parsed() {
    let field: syn::Field = syn::parse_quote! {
        #[norito(needs_size)]
        demo: u32
    };
    let attrs = FieldAttr::parse(&field.attrs).expect("valid field attribute");
    assert!(attrs.needs_size);
}

#[test]
fn required_attribute_is_parsed_and_duplicate_is_rejected() {
    let field: syn::Field = syn::parse_quote! {
        #[norito(required)]
        demo: Option<u32>
    };
    let attrs = FieldAttr::parse(&field.attrs).expect("valid required attribute");
    assert!(attrs.required);

    let duplicate: syn::Field = syn::parse_quote! {
        #[norito(required, required)]
        demo: Option<u32>
    };
    let error = FieldAttr::parse(&duplicate.attrs).expect_err("duplicate required must reject");
    assert_eq!(error.to_string(), "duplicate `required` attribute");
}

#[test]
fn required_attribute_rejects_value_and_incompatible_uses() {
    let valued: syn::Field = syn::parse_quote! {
        #[norito(required = true)]
        demo: Option<u32>
    };
    let error = FieldAttr::parse(&valued.attrs).expect_err("valued required must reject");
    assert_eq!(error.to_string(), "`required` does not take a value");

    let cases: Vec<(syn::Field, &str)> = vec![
        (
            syn::parse_quote!(#[norito(required)] demo: u32),
            "#[norito(required)] can only be used on Option fields",
        ),
        (
            syn::parse_quote!(#[norito(required, default)] demo: Option<u32>),
            "#[norito(required)] cannot be combined with #[norito(default)]",
        ),
        (
            syn::parse_quote!(#[norito(required, skip)] demo: Option<u32>),
            "#[norito(required)] cannot be combined with #[norito(skip)]",
        ),
        (
            syn::parse_quote!(#[norito(required, flatten)] demo: Option<u32>),
            "#[norito(required)] cannot be combined with #[norito(flatten)]",
        ),
        (
            syn::parse_quote!(#[norito(required, skip_serializing_if = "Option::is_none")] demo: Option<u32>),
            "#[norito(required)] cannot be combined with #[norito(skip_serializing_if = ...)]",
        ),
    ];
    for (field, expected) in cases {
        let attrs = FieldAttr::parse(&field.attrs).expect("attributes parse");
        let error = validate_required_attr(&field, &attrs, true).expect_err("misuse must reject");
        assert_eq!(error.to_string(), expected);
    }
}

#[test]
fn required_attribute_is_rejected_on_tuple_fields() {
    let field: syn::Field = syn::parse_quote!(#[norito(required)] Option<u32>);
    let attrs = FieldAttr::parse(&field.attrs).expect("attributes parse");
    let error = validate_required_attr(&field, &attrs, false).expect_err("tuple use must reject");
    assert_eq!(
        error.to_string(),
        "#[norito(required)] is only supported on named fields"
    );
}

#[test]
fn malformed_and_unknown_field_attributes_are_rejected() {
    let malformed: syn::Field = syn::parse_quote! {
        #[norito(with = "bad::")]
        malformed: u32
    };
    assert!(FieldAttr::parse(&malformed.attrs).is_err());

    let unknown: syn::Field = syn::parse_quote! {
        #[norito(transparant)]
        unknown: u32
    };
    let error = FieldAttr::parse(&unknown.attrs).expect_err("unknown key must reject");
    assert_eq!(error.to_string(), "unknown `norito` field attribute");
}

#[test]
fn malformed_enum_field_attribute_is_rejected_before_codegen() {
    let input: DeriveInput = syn::parse_quote! {
        enum Demo {
            Value {
                #[norito(with = "bad::")]
                value: u32,
            },
        }
    };

    validate_data_field_attrs(&input.data)
        .expect_err("enum fields must use the same validation as struct fields");
}
