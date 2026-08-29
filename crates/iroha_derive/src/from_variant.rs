use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::{Attribute, Data, DeriveInput, Fields, Meta, Type, spanned::Spanned as _};

const SKIP_FROM_ATTR: &str = "skip_from";
const SKIP_TRY_FROM_ATTR: &str = "skip_try_from";

#[derive(Clone, Copy, Default)]
struct ConversionOptions {
    skip_from: bool,
    skip_try_from: bool,
}

impl ConversionOptions {
    fn parse(attrs: &[Attribute]) -> syn::Result<Self> {
        let mut options = Self::default();
        let mut errors = None;

        for attr in attrs {
            let target = if attr.path().is_ident(SKIP_FROM_ATTR) {
                &mut options.skip_from
            } else if attr.path().is_ident(SKIP_TRY_FROM_ATTR) {
                &mut options.skip_try_from
            } else {
                continue;
            };

            if !matches!(&attr.meta, Meta::Path(_)) {
                combine_error(
                    &mut errors,
                    syn::Error::new(attr.span(), "this attribute does not accept arguments"),
                );
            }
            if *target {
                combine_error(
                    &mut errors,
                    syn::Error::new(attr.span(), "duplicate attribute"),
                );
            }
            *target = true;
        }

        errors.map_or(Ok(options), Err)
    }
}

fn combine_error(errors: &mut Option<syn::Error>, error: syn::Error) {
    if let Some(errors) = errors {
        errors.combine(error);
    } else {
        *errors = Some(error);
    }
}

fn reject_conversion_attrs(attrs: &[Attribute], location: &str, errors: &mut Option<syn::Error>) {
    for attr in attrs {
        let name = if attr.path().is_ident(SKIP_FROM_ATTR) {
            SKIP_FROM_ATTR
        } else if attr.path().is_ident(SKIP_TRY_FROM_ATTR) {
            SKIP_TRY_FROM_ATTR
        } else {
            continue;
        };
        combine_error(
            errors,
            syn::Error::new(
                attr.span(),
                format!(
                    "#[{name}] attribute should be applied to a newtype variant field, not {location}"
                ),
            ),
        );
    }
}

fn generate_from(
    span: Span,
    enum_ty: &syn::Ident,
    variant: &syn::Ident,
    variant_ty: &Type,
    generics: &syn::Generics,
) -> TokenStream {
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    quote_spanned! { span =>
        impl #impl_generics ::core::convert::From<#variant_ty> for #enum_ty #ty_generics #where_clause {
            fn from(origin: #variant_ty) -> Self {
                #enum_ty::#variant(origin)
            }
        }
    }
}

fn generate_try_from(
    span: Span,
    enum_ty: &syn::Ident,
    variant: &syn::Ident,
    variant_ty: &Type,
    generics: &syn::Generics,
    infallible: bool,
) -> TokenStream {
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let extract = if infallible {
        quote! {
            let #enum_ty::#variant(value) = origin;
            ::core::result::Result::Ok(value)
        }
    } else {
        quote! {
            match origin {
                #enum_ty::#variant(value) => ::core::result::Result::Ok(value),
                _ => ::core::result::Result::Err(
                    ::iroha_macro::error::ErrorTryFromEnum::default()
                ),
            }
        }
    };
    quote_spanned! { span =>
        impl #impl_generics ::core::convert::TryFrom<#enum_ty #ty_generics> for #variant_ty #where_clause {
            type Error = ::iroha_macro::error::ErrorTryFromEnum<#enum_ty #ty_generics, Self>;

            fn try_from(origin: #enum_ty #ty_generics) -> ::core::result::Result<Self, Self::Error> {
                #extract
            }
        }
    }
}

pub fn impl_from_variant(input: DeriveInput) -> syn::Result<TokenStream> {
    let DeriveInput {
        ident,
        generics,
        data,
        ..
    } = input;
    let Data::Enum(data) = data else {
        return Err(syn::Error::new(
            ident.span(),
            "FromVariant can only be derived for enums",
        ));
    };

    let infallible = data.variants.len() == 1;
    let mut errors = None;
    let mut implementations = TokenStream::new();

    for variant in data.variants {
        reject_conversion_attrs(&variant.attrs, "the variant", &mut errors);
        let Fields::Unnamed(fields) = &variant.fields else {
            for field in &variant.fields {
                reject_conversion_attrs(&field.attrs, "a non-newtype field", &mut errors);
            }
            continue;
        };
        if fields.unnamed.len() != 1 {
            for field in &fields.unnamed {
                reject_conversion_attrs(&field.attrs, "a non-newtype field", &mut errors);
            }
            continue;
        }

        let Some(field) = fields.unnamed.first() else {
            continue;
        };
        let options = match ConversionOptions::parse(&field.attrs) {
            Ok(options) => options,
            Err(error) => {
                combine_error(&mut errors, error);
                continue;
            }
        };
        let span = variant.span();
        if !options.skip_try_from {
            implementations.extend(generate_try_from(
                span,
                &ident,
                &variant.ident,
                &field.ty,
                &generics,
                infallible,
            ));
        }
        if !options.skip_from {
            implementations.extend(generate_from(
                span,
                &ident,
                &variant.ident,
                &field.ty,
                &generics,
            ));
        }
    }

    errors.map_or_else(|| Ok(implementations), Err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    #[test]
    fn conversion_options_reject_duplicates_and_arguments() {
        let duplicate: syn::Field = parse_quote!(
            #[skip_from]
            #[skip_from]
            String
        );
        let with_arguments: syn::Field = parse_quote!(
            #[skip_try_from(reason)]
            String
        );
        assert!(ConversionOptions::parse(&duplicate.attrs).is_err());
        assert!(ConversionOptions::parse(&with_arguments.attrs).is_err());
    }

    #[test]
    fn derive_rejects_attributes_on_non_newtype_fields() {
        let input: DeriveInput = parse_quote! {
            enum Example {
                Invalid { #[skip_try_from] value: String },
            }
        };
        let error = impl_from_variant(input).expect_err("attribute placement must be rejected");
        assert!(error.to_string().contains("newtype variant field"));
    }
}
