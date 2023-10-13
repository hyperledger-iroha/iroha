pub use iroha_macro_utils::{attr_struct, AttrParser};
use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::{
    parse::{Parse, ParseStream},
    Attribute, GenericArgument, Ident, LitStr, Meta, NestedMeta, PathArguments, Token, Type,
};

/// Keywords used inside `#[view(...)]` and `#[config(...)]`
mod kw {
    // config keywords
    syn::custom_keyword!(serde_as_str);
    syn::custom_keyword!(inner);
    syn::custom_keyword!(env_prefix);
    // view keywords
    syn::custom_keyword!(ignore);
    syn::custom_keyword!(into);
    // builder keywords
    syn::custom_keyword!(parent);
}

/// Structure to parse `#[view(...)]` attributes.
/// [`Inner`] is responsible for parsing attribute arguments.
pub struct View<Inner: Parse>(std::marker::PhantomData<Inner>);

/// Structure to parse `#[config(...)]` attributes.
/// [`Inner`] is responsible for parsing attribute arguments.
struct Config<Inner: Parse>(std::marker::PhantomData<Inner>);

/// Structure to parse `#[builder(...)]` attributes.
/// [`Inner`] is responsible for parsing attribute arguments.
struct Builder<Inner: Parse>(std::marker::PhantomData<Inner>);

impl<Inner: Parse> AttrParser<Inner> for View<Inner> {
    const IDENT: &'static str = "view";
}

impl<Inner: Parse> AttrParser<Inner> for Config<Inner> {
    const IDENT: &'static str = "config";
}

impl<Inner: Parse> AttrParser<Inner> for Builder<Inner> {
    const IDENT: &'static str = "builder";
}

attr_struct! {
    pub struct ViewIgnore {
        _kw: kw::ignore,
    }
}

attr_struct! {
    pub struct ViewFieldType {
        _kw: kw::into,
        _eq: Token![=],
        ty: Type,
    }
}

attr_struct! {
    pub struct ConfigInner {
        _kw: kw::inner,
    }
}

attr_struct! {
    pub struct ConfigAsStr {
        _kw: kw::serde_as_str,
    }
}

attr_struct! {
    pub struct ConfigEnvPrefix {
        _kw: kw::env_prefix,
        _eq: Token![=],
        pub prefix: LitStr,
    }
}

attr_struct! {
    pub struct BuilderParent {
        _kw: kw::parent,
        _eq: Token![=],
        pub parent: Type,
    }
}

impl From<ViewFieldType> for Type {
    fn from(value: ViewFieldType) -> Self {
        value.ty
    }
}

#[derive(Clone)]
pub struct StructField {
    pub ident: Ident,
    pub ty: Type,
    pub vis: syn::Visibility,
    pub attrs: Vec<Attribute>,
    pub env_str: String,
    pub has_inner: bool,
    pub has_option: bool,
    pub has_as_str: bool,
    pub lvalue_read: TokenStream,
    pub lvalue_write: TokenStream,
}

impl StructField {
    fn from_ast(field: syn::Field, env_prefix: &str) -> Self {
        let field_ident = field
            .ident
            .expect("Already checked for named fields at parsing");
        let (lvalue_read, lvalue_write) = gen_lvalue(&field.ty, &field_ident);
        StructField {
            has_inner: field
                .attrs
                .iter()
                .any(|attr| Config::<ConfigInner>::parse(attr).is_ok()),
            has_as_str: field
                .attrs
                .iter()
                .any(|attr| Config::<ConfigAsStr>::parse(attr).is_ok()),
            has_option: is_option_type(&field.ty),
            env_str: env_prefix.to_owned() + &field_ident.to_string().to_uppercase(),
            attrs: field.attrs,
            ident: field_ident,
            ty: field.ty,
            vis: field.vis,
            lvalue_read,
            lvalue_write,
        }
    }
}

impl ToTokens for StructField {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let StructField {
            attrs,
            ty,
            ident,
            vis,
            ..
        } = self;
        let stream = quote! {
            #(#attrs)*
            #vis #ident: #ty
        };
        tokens.extend(stream);
    }
}

/// Parsed struct with named fields used in proc macros of this crate
#[derive(Clone)]
pub struct StructWithFields {
    pub attrs: Vec<Attribute>,
    pub env_prefix: String,
    pub vis: syn::Visibility,
    _struct_token: Token![struct],
    pub ident: Ident,
    pub generics: syn::Generics,
    pub fields: Vec<StructField>,
    _semi_token: Option<Token![;]>,
}

impl Parse for StructWithFields {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let attrs = input.call(Attribute::parse_outer)?;
        let env_prefix = attrs
            .iter()
            .map(Config::<ConfigEnvPrefix>::parse)
            .find_map(Result::ok)
            .map(|pref| pref.prefix.value())
            .unwrap_or_default();
        Ok(Self {
            attrs,
            vis: input.parse()?,
            _struct_token: input.parse()?,
            ident: input.parse()?,
            generics: input.parse()?,
            fields: input
                .parse::<syn::FieldsNamed>()?
                .named
                .into_iter()
                .map(|field| StructField::from_ast(field, &env_prefix))
                .collect(),
            env_prefix,
            _semi_token: input.parse()?,
        })
    }
}

impl ToTokens for StructWithFields {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let StructWithFields {
            attrs,
            vis,
            ident,
            generics,
            fields,
            ..
        } = self;
        let stream = quote! {
            #(#attrs)*
            #vis struct #ident #generics {
                #(#fields),*
            }
        };
        tokens.extend(stream);
    }
}

/// Remove attributes with ident [`attr_ident`] from attributes
pub fn remove_attr(attrs: &mut Vec<Attribute>, attr_ident: &str) {
    attrs.retain(|attr| !attr.path.is_ident(attr_ident));
}

pub fn extract_field_idents(fields: &[StructField]) -> Vec<&Ident> {
    fields.iter().map(|field| &field.ident).collect::<Vec<_>>()
}

pub fn extract_field_types(fields: &[StructField]) -> Vec<Type> {
    fields
        .iter()
        .map(|field| field.ty.clone())
        .collect::<Vec<_>>()
}

pub fn get_type_argument<'tl>(s: &str, ty: &'tl Type) -> Option<&'tl GenericArgument> {
    let Type::Path(path) = ty else {
        return None;
    };
    let segments = &path.path.segments;
    if segments.len() != 1 || segments[0].ident != s {
        return None;
    }

    if let PathArguments::AngleBracketed(bracketed_arguments) = &segments[0].arguments {
        if bracketed_arguments.args.len() == 1 {
            return Some(&bracketed_arguments.args[0]);
        }
    }
    None
}

pub fn get_inner_type<'tl>(outer_ty_ident: &str, ty: &'tl Type) -> &'tl Type {
    #[allow(clippy::shadow_unrelated)]
    get_type_argument(outer_ty_ident, ty)
        .and_then(|ty| {
            if let GenericArgument::Type(r#type) = ty {
                Some(r#type)
            } else {
                None
            }
        })
        .unwrap_or(ty)
}

pub fn is_arc_rwlock(ty: &Type) -> bool {
    let dearced_ty = get_inner_type("Arc", ty);
    get_type_argument("RwLock", dearced_ty).is_some()
}

/// Check if the provided type is of the form [`Option<..>`]
pub fn is_option_type(ty: &Type) -> bool {
    get_type_argument("Option", ty).is_some()
}

/// Remove attributes with ident [`attr_ident`] from struct attributes and field attributes
pub fn remove_attr_from_struct(ast: &mut StructWithFields, attr_ident: &str) {
    let StructWithFields { attrs, fields, .. } = ast;
    for field in fields {
        remove_attr(&mut field.attrs, attr_ident);
    }
    remove_attr(attrs, attr_ident);
}

/// Keep only derive attributes passed as a second argument in struct attributes and field attributes
pub fn keep_derive_attr(ast: &mut StructWithFields, kept_attrs: &[&str]) {
    ast.attrs
        .iter_mut()
        .filter(|attr| attr.path.is_ident("derive"))
        .for_each(|attr| {
            let meta = attr
                .parse_meta()
                .expect("derive macro must be in one of the meta forms");
            if let Meta::List(list) = meta {
                let items: Vec<NestedMeta> = list
                    .nested
                    .into_iter()
                    .filter(|nested| {
                        if let NestedMeta::Meta(Meta::Path(path)) = nested {
                            return kept_attrs.iter().any(|kept_attr| path.is_ident(kept_attr));
                        }
                        // Non-nested all kept by default
                        true
                    })
                    .collect();
                *attr = syn::parse_quote!(
                    #[derive(#(#items),*)]
                );
            }
        });
}

/// Keep only attributes passed as a second argument in struct attributes and field attributes
pub fn keep_attrs_in_struct(ast: &mut StructWithFields, kept_attrs: &[&str]) {
    let StructWithFields { attrs, fields, .. } = ast;
    for field in fields {
        field.attrs.retain(|attr| {
            kept_attrs
                .iter()
                .any(|kept_attr| attr.path.is_ident(kept_attr))
        });
    }
    attrs.retain(|attr| {
        kept_attrs
            .iter()
            .any(|kept_attr| attr.path.is_ident(kept_attr))
    });
}

/// Generate lvalue forms for a struct field, taking [`Arc<RwLock<..>>`] types
/// into account as well. Returns a 2-tuple of read and write forms.
pub fn gen_lvalue(field_ty: &Type, field_ident: &Ident) -> (TokenStream, TokenStream) {
    let is_lvalue = is_arc_rwlock(field_ty);

    let lvalue_read = if is_lvalue {
        quote! { self.#field_ident.read().await }
    } else {
        quote! { self.#field_ident }
    };

    let lvalue_write = if is_lvalue {
        quote! { self.#field_ident.write().await }
    } else {
        quote! { self.#field_ident }
    };

    (lvalue_read, lvalue_write)
}

/// Check if [`StructWithFields`] has `#[builder(parent = ..)]`
pub fn get_parent_ty(ast: &StructWithFields) -> Type {
    ast.attrs
        .iter()
        .find_map(|attr| Builder::<BuilderParent>::parse(attr).ok())
        .map(|builder| builder.parent)
        .expect("Should not be called on structs with no `#[builder(..)]` attribute")
}
