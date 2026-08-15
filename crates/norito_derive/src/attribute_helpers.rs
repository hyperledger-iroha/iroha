fn consume_unknown_meta(meta: syn::meta::ParseNestedMeta) -> SynResult<()> {
    if meta.input.peek(syn::token::Paren) {
        meta.parse_nested_meta(consume_unknown_meta)?
    } else if meta.input.peek(Token![=]) {
        // Parse exactly one attribute value. Parsing a free-form TokenStream here
        // consumes every remaining comma-separated item in the enclosing
        // `#[norito(...)]` list, which can silently hide a later option from a
        // different derive (for example `tag = "kind", schema_name = "stable",
        // deny_unknown_fields`).
        meta.value()?.parse::<syn::Expr>()?;
    }
    Ok(())
}
/// Returns true if the container has `#[norito(decode_from_slice)]` attribute.
fn has_decode_from_slice_attr(attrs: &[Attribute]) -> bool {
    ContainerAttr::parse(attrs)
        .expect("container attributes must be validated before code generation")
        .decode_from_slice
}
fn reuse_archived_alias(attrs: &[Attribute]) -> bool {
    ContainerAttr::parse(attrs)
        .expect("container attributes must be validated before code generation")
        .reuse_archived
}
