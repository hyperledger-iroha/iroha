use super::*;

#[test]
fn unknown_variant_attribute_is_rejected() {
    let variant: Variant = syn::parse_quote! {
        #[norito(other)]
        Unknown
    };

    let error = VariantAttr::parse(&variant.attrs).expect_err("unknown key must reject");
    assert_eq!(error.to_string(), "unknown `norito` variant attribute");
}
