use manyhow::{Emitter, emit};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{
    Attribute, Data, DeriveInput, Expr, ExprLit, Field, Fields, Lit, parse_quote, spanned::Spanned,
};

struct FieldInfo {
    ident: syn::Ident,
    ty: syn::Type,
    attrs: Vec<Attribute>,
    skip: bool,
    default: Option<Expr>,
    init: Option<Expr>,
}

fn parse_field(emitter: &mut Emitter, field: &Field) -> FieldInfo {
    let ident = field.ident.clone().expect("named fields");
    let ty = field.ty.clone();
    let attrs: Vec<Attribute> = field
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("doc"))
        .cloned()
        .collect();
    let mut skip = false;
    let mut default = None;
    let mut init = None;

    for attr in &field.attrs {
        if !attr.path().is_ident("registrable_builder") {
            continue;
        }
        if let Err(e) = parse_registrable_attr(attr, &mut skip, &mut default, &mut init) {
            emit!(emitter, attr.span(), "{}", e);
        }
    }

    if skip && init.is_none() {
        emit!(
            emitter,
            field.span(),
            "`skip` fields must specify `init` expression"
        );
    }

    FieldInfo {
        ident,
        ty,
        attrs,
        skip,
        default,
        init,
    }
}

fn parse_registrable_attr(
    attr: &Attribute,
    skip: &mut bool,
    default: &mut Option<Expr>,
    init: &mut Option<Expr>,
) -> syn::Result<()> {
    attr.parse_nested_meta(|nested| {
        if nested.path.is_ident("skip") {
            *skip = true;
            Ok(())
        } else if nested.path.is_ident("default") {
            let expr: Expr = nested.value()?.parse()?;
            let expr = normalize_value_expr(expr)?;
            *default = Some(expr);
            Ok(())
        } else if nested.path.is_ident("init") {
            let expr: Expr = nested.value()?.parse()?;
            *init = Some(normalize_value_expr(expr)?);
            Ok(())
        } else {
            Err(nested.error("Unsupported attribute option"))
        }
    })
}

fn normalize_value_expr(expr: Expr) -> syn::Result<Expr> {
    if let Expr::Lit(ExprLit {
        lit: Lit::Str(lit), ..
    }) = expr
    {
        syn::parse_str(&lit.value())
    } else {
        Ok(expr)
    }
}

#[allow(clippy::too_many_lines)]
pub fn impl_registrable_builder(emitter: &mut Emitter, input: &DeriveInput) -> TokenStream {
    let name = &input.ident;
    let builder_name = format_ident!("New{}", name);

    let mut item_attrs: Vec<Attribute> = input
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("doc"))
        .cloned()
        .collect();
    add_doc_if_missing(
        &mut item_attrs,
        format!("Builder for `{name}` created by `RegistrableBuilder`."),
    );

    // Extract named fields; emit helpful diagnostics on mismatch.
    let fields_named = if let Data::Struct(data) = &input.data {
        if let Fields::Named(named) = &data.fields {
            named
        } else {
            emit!(
                emitter,
                input,
                "RegistrableBuilder can only be used with structs with named fields"
            );
            return quote!();
        }
    } else {
        emit!(
            emitter,
            input,
            "RegistrableBuilder can only be used with structs"
        );
        return quote!();
    };

    let infos: Vec<FieldInfo> = fields_named
        .named
        .iter()
        .map(|f| parse_field(emitter, f))
        .collect();

    let builder_fields = infos
        .iter()
        .filter(|f| !f.skip)
        .map(|f| {
            let ident = &f.ident;
            let ty = &f.ty;
            let mut attrs = f.attrs.clone();
            if f.default.is_some() {
                attrs.push(parse_quote! {
                    #[cfg_attr(feature = "json", norito(default))]
                });
            }
            add_doc_if_missing(
                &mut attrs,
                format!("Builder field for `{name}` initial `{ident}` value."),
            );
            quote! { #(#attrs)* pub #ident: #ty }
        })
        .collect::<Vec<_>>();

    let new_params = infos
        .iter()
        .filter(|f| !f.skip && f.default.is_none())
        .map(|f| {
            let ident = &f.ident;
            let ty = &f.ty;
            quote! { #ident: #ty }
        })
        .collect::<Vec<_>>();

    let new_inits = infos
        .iter()
        .filter(|f| !f.skip)
        .map(|f| {
            let ident = &f.ident;
            f.default
                .as_ref()
                .map_or_else(|| quote! { #ident }, |expr| quote! { #ident: #expr })
        })
        .collect::<Vec<_>>();

    let with_methods = infos
        .iter()
        .filter(|f| !f.skip)
        .map(|f| {
            let ident = &f.ident;
            let method_ident = format_ident!("with_{}", ident);
            let ty = &f.ty;
            quote! {
                #[doc = concat!("Set `", stringify!(#ident), "` on the builder.")]
                #[inline]
                #[must_use]
                pub fn #method_ident(mut self, #ident: #ty) -> Self {
                    self.#ident = #ident;
                    self
                }
            }
        })
        .collect::<Vec<_>>();

    let build_fields = infos
        .iter()
        .map(|f| {
            let ident = &f.ident;
            if f.skip {
                let init = f.init.as_ref().expect("init guaranteed");
                quote! { #ident: #init }
            } else {
                quote! { #ident: self.#ident }
            }
        })
        .collect::<Vec<_>>();

    let json_fields: Vec<_> = infos.iter().filter(|f| !f.skip).collect();
    let builder_fields_idents = json_fields
        .iter()
        .map(|f| f.ident.clone())
        .collect::<Vec<_>>();
    let builder_fields_tys = json_fields.iter().map(|f| f.ty.clone()).collect::<Vec<_>>();
    let builder_json_inits = json_fields
        .iter()
        .map(|f| {
            let ident = &f.ident;
            f.default.as_ref().map_or_else(
                || quote! { #ident.ok_or_else(|| MapVisitor::missing_field(stringify!(#ident)))? },
                |default| quote! { #ident.unwrap_or_else(|| #default) },
            )
        })
        .collect::<Vec<_>>();

    quote! {
        #[derive(Debug, Clone, IdEqOrdHash, Decode, Encode, IntoSchema)]
        #[cfg_attr(
            feature = "json",
            derive(crate::DeriveJsonSerialize, crate::DeriveFastJson)
        )]
        // If JsonDeserialize is ever added back as a derive, keep FastFromJson single-sourced
        // from DeriveFastJson by suppressing its fallback emission.
        #[cfg_attr(feature = "json", norito(no_fast_from_json))]
        #(#item_attrs)*
        pub struct #builder_name {
            #( #builder_fields, )*
        }

        #[cfg(feature = "json")]
        impl norito::json::JsonDeserialize for #builder_name {
            fn json_deserialize(
                parser: &mut norito::json::Parser<'_>,
            ) -> Result<Self, norito::json::Error> {
                use norito::json::{Error, KeyRef, MapVisitor};

                let mut visitor = MapVisitor::new(parser)?;
                #( let mut #builder_fields_idents: Option<#builder_fields_tys> = None; )*

                while let Some(key) = visitor.next_key()? {
                    match key {
                        #(
                            KeyRef::Borrowed(name) if name == stringify!(#builder_fields_idents) => {
                                if #builder_fields_idents.is_some() {
                                    return Err(MapVisitor::duplicate_field(name));
                                }
                                #builder_fields_idents = Some(visitor.parse_value()?);
                            }
                        )*
                        KeyRef::Owned(name) => return Err(Error::unknown_field(name)),
                        KeyRef::Borrowed(name) => return Err(Error::unknown_field(name.to_owned())),
                    }
                }

                visitor.finish()?;

                Ok(Self {
                    #(
                        #builder_fields_idents: #builder_json_inits,
                    )*
                })
            }
        }

        impl core::fmt::Display for #builder_name {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                core::fmt::Debug::fmt(self, f)
            }
        }

        impl #builder_name {
            #[doc = concat!(
                "Create a new `",
                stringify!(#builder_name),
                "` for `",
                stringify!(#name),
                "`."
            )]
            #[inline]
            pub fn new( #( #new_params ),* ) -> Self {
                Self { #( #new_inits, )* }
            }
            #( #with_methods )*
        }

        impl Registered for #name {
            type With = #builder_name;
        }

        impl Registrable for #builder_name {
            type Target = #name;
            #[inline]
            #[doc = concat!("Build a `", stringify!(#name), "` from this builder.")]
            fn build(self, authority: &crate::account::AccountId) -> Self::Target {
                Self::Target {
                    #( #build_fields, )*
                }
            }
        }
    }
}

fn add_doc_if_missing(attrs: &mut Vec<syn::Attribute>, default: impl AsRef<str>) {
    if attrs.iter().any(|attr| attr.path().is_ident("doc")) {
        return;
    }
    let doc = default.as_ref();
    attrs.push(syn::parse_quote!(#[doc = #doc]));
}
