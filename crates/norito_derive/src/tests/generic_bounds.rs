use super::*;

#[test]
fn concrete_recursive_field_does_not_create_a_cyclic_bound() {
    let mut generics = Generics::default();
    let field: syn::Type = syn::parse_quote!(Box<Expr>);

    add_bound(&mut generics, &field, quote!(DemoTrait));

    assert!(generics.where_clause.is_none());
}

#[test]
fn nested_generic_field_keeps_its_required_bound() {
    let mut generics: Generics = syn::parse_quote!(<'a, T, const N: usize>);
    let field: syn::Type = syn::parse_quote!(Cow<'a, [T; N]>);

    add_bound(&mut generics, &field, quote!(DemoTrait));

    let predicates = generics
        .where_clause
        .as_ref()
        .expect("generic field must add a where clause")
        .predicates
        .to_token_stream()
        .to_string();
    assert!(predicates.contains("Cow < 'a , [T ; N] > : DemoTrait"));
}
