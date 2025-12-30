use std::fmt::{Display, Formatter};

use darling::{
    FromAttributes, FromDeriveInput, FromField, FromVariant, ast::Style, util::SpannedValue,
};
use manyhow::{Emitter, emit, error_message};
use proc_macro2::{Delimiter, Literal, Span, TokenStream};
use quote::{format_ident, quote};
use syn::{
    Attribute, Field, GenericParam, Ident, Lifetime, LifetimeParam,
    parse::ParseStream,
    spanned::Spanned as _,
    token::{Gt, Lt},
    visit::Visit as _,
};

use crate::{
    attr_parse::{
        derive::DeriveAttrs,
        doc::DocAttrs,
        getset::{GetSetFieldAttrs, GetSetStructAttrs},
        repr::{Repr, ReprKind, ReprPrimitive},
    },
    emitter_ext::EmitterExt,
    utils::{darling_result, parse_single_list_attr_opt},
};

#[derive(Debug)]
enum FfiTypeToken {
    Opaque,
    UnsafeRobust,
    UnsafeNonOwning,
    Local,
}

impl Display for FfiTypeToken {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            FfiTypeToken::Opaque => "#[ffi_type(opaque)]",
            FfiTypeToken::UnsafeRobust => "#[ffi_type(unsafe {robust})]",
            FfiTypeToken::UnsafeNonOwning => "#[ffi_type(unsafe {non_owning})]",
            FfiTypeToken::Local => "#[ffi_type(local)]",
        };
        write!(f, "{text}")
    }
}

#[derive(Debug)]
struct SpannedFfiTypeToken {
    span: Span,
    token: FfiTypeToken,
}

impl syn::parse::Parse for SpannedFfiTypeToken {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let (span, token) = input.step(|cursor| {
            let Some((token, after_token)) = cursor.ident() else {
                return Err(cursor.error("expected ffi type kind"));
            };

            let mut span = token.span();
            let token = token.to_string();
            match token.as_str() {
                "opaque" => Ok(((span, FfiTypeToken::Opaque), after_token)),
                "local" => Ok(((span, FfiTypeToken::Local), after_token)),
                "unsafe" => {
                    let Some((inside_of_group, group_span, after_group)) =
                        after_token.group(Delimiter::Brace)
                    else {
                        return Err(cursor.error("expected `{ ... }` after `unsafe`"));
                    };
                    span = span.join(group_span.span()).unwrap_or(span);

                    let Some((token, after_token)) = inside_of_group.ident() else {
                        return Err(cursor.error("expected ffi type kind"));
                    };
                    if !after_token.eof() {
                        return Err(cursor
                            .error("`unsafe { ... }` should only contain one identifier inside"));
                    }

                    let token = token.to_string();
                    match token.as_str() {
                        "robust" => Ok(((span, FfiTypeToken::UnsafeRobust), after_group)),
                        "non_owning" => Ok(((span, FfiTypeToken::UnsafeNonOwning), after_group)),
                        other => Err(syn::Error::new(
                            token.span(),
                            format!("unknown unsafe ffi type kind: {other}"),
                        )),
                    }
                }
                other => Err(syn::Error::new(
                    span,
                    format!("unknown unsafe ffi type kind: {other}"),
                )),
            }
        })?;

        Ok(Self { span, token })
    }
}

/// This represents an `#[ffi_type(...)]` attribute on a type
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum FfiTypeKindAttribute {
    Opaque,
    UnsafeRobust,
    Local,
}

impl syn::parse::Parse for FfiTypeKindAttribute {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        input.call(SpannedFfiTypeToken::parse).and_then(|token| {
            Ok(match token.token {
                FfiTypeToken::Opaque => FfiTypeKindAttribute::Opaque,
                FfiTypeToken::UnsafeRobust => FfiTypeKindAttribute::UnsafeRobust,
                FfiTypeToken::Local => FfiTypeKindAttribute::Local,
                other => {
                    return Err(syn::Error::new(
                        token.span,
                        format!("`{other}` cannot be used on a type"),
                    ));
                }
            })
        })
    }
}

/// This represents an `#[ffi_type(...)]` attribute on a field
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum FfiTypeKindFieldAttribute {
    UnsafeNonOwning,
}

impl syn::parse::Parse for FfiTypeKindFieldAttribute {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        input.call(SpannedFfiTypeToken::parse).and_then(|token| {
            Ok(match token.token {
                FfiTypeToken::UnsafeNonOwning => FfiTypeKindFieldAttribute::UnsafeNonOwning,
                other => {
                    return Err(syn::Error::new(
                        token.span,
                        format!("`{other}` cannot be used on a field"),
                    ));
                }
            })
        })
    }
}

const FFI_TYPE_ATTR: &str = "ffi_type";

pub struct FfiTypeAttr {
    pub kind: Option<FfiTypeKindAttribute>,
}

impl FromAttributes for FfiTypeAttr {
    fn from_attributes(attrs: &[Attribute]) -> darling::Result<Self> {
        parse_single_list_attr_opt(FFI_TYPE_ATTR, attrs).map(|kind| Self { kind })
    }
}

pub struct FfiTypeFieldAttr {
    kind: Option<FfiTypeKindFieldAttribute>,
}

impl FromAttributes for FfiTypeFieldAttr {
    fn from_attributes(attrs: &[Attribute]) -> darling::Result<Self> {
        parse_single_list_attr_opt(FFI_TYPE_ATTR, attrs).map(|kind| Self { kind })
    }
}

pub type FfiTypeData = darling::ast::Data<SpannedValue<FfiTypeVariant>, FfiTypeField>;
pub type FfiTypeFields = darling::ast::Fields<FfiTypeField>;

pub struct FfiTypeInput {
    pub vis: syn::Visibility,
    pub ident: syn::Ident,
    pub generics: syn::Generics,
    pub data: FfiTypeData,
    pub derive_attr: DeriveAttrs,
    pub repr_attr: Repr,
    pub ffi_type_attr: FfiTypeAttr,
    pub getset_attr: GetSetStructAttrs,
    pub span: Span,
    /// The original `DeriveInput` this structure was parsed from
    pub ast: syn::DeriveInput,
}

impl FfiTypeInput {
    pub fn is_opaque(&self) -> bool {
        self.ffi_type_attr.kind == Some(FfiTypeKindAttribute::Opaque)
            || !self.data.is_enum() && self.repr_attr.kind.as_deref().is_none()
    }
}

impl darling::FromDeriveInput for FfiTypeInput {
    fn from_derive_input(input: &syn::DeriveInput) -> darling::Result<Self> {
        let vis = input.vis.clone();
        let ident = input.ident.clone();
        let generics = input.generics.clone();
        let data = darling::ast::Data::try_from(&input.data)?;
        let derive_attr = DeriveAttrs::from_attributes(&input.attrs)?;
        let repr_attr = Repr::from_attributes(&input.attrs)?;
        let ffi_type_attr = FfiTypeAttr::from_attributes(&input.attrs)?;
        let getset_attr = GetSetStructAttrs::from_attributes(&input.attrs)?;
        let span = input.span();

        Ok(FfiTypeInput {
            vis,
            ident,
            generics,
            data,
            derive_attr,
            repr_attr,
            ffi_type_attr,
            getset_attr,
            span,
            ast: input.clone(),
        })
    }
}

#[derive(FromVariant)]
pub struct FfiTypeVariant {
    pub ident: syn::Ident,
    pub discriminant: Option<syn::Expr>,
    pub fields: darling::ast::Fields<FfiTypeField>,
}

pub struct FfiTypeField {
    pub ident: Option<syn::Ident>,
    pub ty: syn::Type,
    pub doc_attrs: DocAttrs,
    pub ffi_type_attr: FfiTypeFieldAttr,
    pub getset_attr: GetSetFieldAttrs,
}

impl FromField for FfiTypeField {
    fn from_field(field: &Field) -> darling::Result<Self> {
        let ident = field.ident.clone();
        let ty = field.ty.clone();
        let doc_attrs = DocAttrs::from_attributes(&field.attrs)?;
        let ffi_type_attr = FfiTypeFieldAttr::from_attributes(&field.attrs)?;
        let getset_attr = GetSetFieldAttrs::from_attributes(&field.attrs)?;
        Ok(Self {
            ident,
            ty,
            doc_attrs,
            ffi_type_attr,
            getset_attr,
        })
    }
}

pub fn derive_ffi_type(emitter: &mut Emitter, input: &syn::DeriveInput) -> TokenStream {
    let Some(mut input) = emitter.handle(darling_result(FfiTypeInput::from_derive_input(input)))
    else {
        return quote!();
    };

    let name = &input.ident;
    if let darling::ast::Data::Enum(variants) = &input.data
        && variants.is_empty()
    {
        emit!(
            emitter,
            input.span,
            "Uninhabited enums are not allowed in FFI"
        );
    }

    // the logic of `is_opaque` is somewhat convoluted and I am not sure if it is even correct
    // there is also `is_opaque_struct`...
    if input.is_opaque() {
        return derive_ffi_type_for_opaque_item(name, &input.generics);
    }
    if input.repr_attr.kind.as_deref() == Some(&ReprKind::Transparent) {
        return derive_ffi_type_for_transparent_item(emitter, &input);
    }

    match &input.data {
        darling::ast::Data::Enum(variants) => {
            if variants.iter().all(|v| v.as_ref().fields.is_empty()) {
                if variants.len() == 1 {
                    // NOTE: one-variant fieldless enums have representation of ()
                    return derive_ffi_type_for_opaque_item(name, &input.generics);
                }
                if let Some(variant) = variants.iter().find(|v| v.as_ref().discriminant.is_some()) {
                    emit!(
                        emitter,
                        &variant.span(),
                        "Fieldless enums with explicit discriminants are prohibited",
                    );
                }

                derive_ffi_type_for_fieldless_enum(
                    emitter,
                    &input.ident,
                    variants,
                    &input.repr_attr,
                )
            } else {
                verify_is_non_owning(emitter, &input.data);
                let local = input.ffi_type_attr.kind == Some(FfiTypeKindAttribute::Local);

                derive_ffi_type_for_data_carrying_enum(
                    emitter,
                    &input.ident,
                    input.generics,
                    variants,
                    local,
                )
            }
        }
        darling::ast::Data::Struct(item) => {
            let ffi_type_impl = derive_ffi_type_for_repr_c(emitter, &input);

            let repr_c_impl = {
                let predicates = &mut input.generics.make_where_clause().predicates;
                let add_bound = |ty| predicates.push(syn::parse_quote! {#ty: iroha_ffi::ReprC});

                if item.style == Style::Unit {
                    emit!(
                        emitter,
                        &input.span,
                        "Unit structs cannot implement `ReprC`"
                    );
                }

                item.fields
                    .iter()
                    .map(|field: &FfiTypeField| &field.ty)
                    .for_each(add_bound);

                derive_unsafe_repr_c(&input.ident, &input.generics)
            };

            quote! {
                #repr_c_impl
                #ffi_type_impl
            }
        }
    }
}

/// Before deriving this trait make sure that all invariants are upheld
fn derive_unsafe_repr_c(name: &Ident, generics: &syn::Generics) -> TokenStream {
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    quote! {
        // SAFETY: Type is robust with #[repr(C)] attribute attached
        unsafe impl #impl_generics iroha_ffi::ReprC for #name #ty_generics #where_clause {}
    }
}

fn derive_ffi_type_for_opaque_item(name: &Ident, generics: &syn::Generics) -> TokenStream {
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    quote! {
        impl #impl_generics iroha_ffi::ir::Ir for #name #ty_generics #where_clause {
            type Type = iroha_ffi::ir::Opaque;
        }

        // SAFETY: Opaque types are never dereferenced and therefore &mut T is considered to be transmutable
        unsafe impl #impl_generics iroha_ffi::ir::InfallibleTransmute for #name #ty_generics #where_clause {}

        impl #impl_generics iroha_ffi::option::Niche<'_> for #name #ty_generics #where_clause {
            const NICHE_VALUE: *mut Self = core::ptr::null_mut();
        }
    }
}

fn derive_ffi_type_for_transparent_item(
    emitter: &mut Emitter,
    input: &FfiTypeInput,
) -> TokenStream {
    assert_eq!(
        input.repr_attr.kind.as_deref().copied(),
        Some(ReprKind::Transparent)
    );

    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let name = &input.ident;

    // #[repr(transparent)] can only be used on a struct or
    //      single-variant enum that has a single non-zero-sized field (there may be additional zero-sized fields).
    // The effect is that the layout and ABI of the whole struct/enum is guaranteed to be the same as that one field.

    let inner = match &input.data {
        darling::ast::Data::Enum(variants) => {
            if variants.len() != 1 {
                emitter.emit(error_message!(
                    input.span,
                    "`{}` is marked #[repr(transparent)] but has {} variants",
                    name,
                    variants.len()
                ));
                return quote! {};
            }

            let first_variant = emitter.handle(variants.iter().next().ok_or_else(|| {
                error_message!("transparent enum must have exactly one variant, but it has none")
            }));

            let Some(first_variant) = first_variant else {
                return quote! {};
            };

            match select_transparent_inner(
                emitter,
                name,
                first_variant.span(),
                &first_variant.as_ref().fields,
            ) {
                Ok(TransparentInner::Type(inner)) => inner,
                Ok(TransparentInner::AllZeroSized) => {
                    // NOTE: one-variant fieldless enums have representation of ()
                    return derive_ffi_type_for_opaque_item(name, &input.generics);
                }
                Err(()) => return quote! {},
            }
        }
        darling::ast::Data::Struct(item) => {
            match select_transparent_inner(emitter, name, input.span, item) {
                Ok(TransparentInner::Type(inner)) => inner,
                Ok(TransparentInner::AllZeroSized) => {
                    // NOTE: Fieldless structs have representation of ()
                    return derive_ffi_type_for_opaque_item(name, &input.generics);
                }
                Err(()) => return quote! {},
            }
        }
    };

    if input.ffi_type_attr.kind == Some(FfiTypeKindAttribute::UnsafeRobust) {
        return quote! {
            iroha_ffi::ffi_type! {
                // SAFETY: User must make sure the type is robust
                unsafe impl #impl_generics Transparent for #name #ty_generics #where_clause {
                    type Target = #inner;
                }
            }
        };
    }

    quote! {}
}

enum TransparentInner<'a> {
    Type(&'a syn::Type),
    AllZeroSized,
}

fn select_transparent_inner<'a>(
    emitter: &mut Emitter,
    parent: &Ident,
    span: Span,
    fields: &'a darling::ast::Fields<FfiTypeField>,
) -> Result<TransparentInner<'a>, ()> {
    let non_zst: Vec<&FfiTypeField> = fields
        .fields
        .iter()
        .filter(|field| !is_definitely_zero_sized(&field.ty))
        .collect();

    if fields.fields.is_empty() {
        return Ok(TransparentInner::AllZeroSized);
    }

    if non_zst.is_empty() {
        emitter.emit(error_message!(
            span,
            "`{}` is marked #[repr(transparent)] but all fields appear zero-sized",
            parent
        ));
        return Err(());
    }

    if non_zst.len() > 1 {
        emitter.emit(error_message!(
            span,
            "`{}` is marked #[repr(transparent)] but has {} fields that appear non-zero-sized",
            parent,
            non_zst.len()
        ));
        return Err(());
    }

    Ok(TransparentInner::Type(&non_zst[0].ty))
}

fn is_definitely_zero_sized(ty: &syn::Type) -> bool {
    match ty {
        syn::Type::Tuple(tuple) => tuple.elems.is_empty(),
        syn::Type::Path(path) => path.path.segments.last().is_some_and(|segment| {
            segment.ident == "PhantomData" || segment.ident == "PhantomPinned"
        }),
        syn::Type::Array(array) => match &array.len {
            syn::Expr::Lit(expr_lit) => {
                if let syn::Lit::Int(int) = &expr_lit.lit
                    && let Ok(value) = int.base10_parse::<u64>()
                {
                    return value == 0;
                }
                false
            }
            _ => false,
        },
        syn::Type::Paren(inner) => is_definitely_zero_sized(&inner.elem),
        syn::Type::Group(group) => is_definitely_zero_sized(&group.elem),
        _ => false,
    }
}

fn derive_ffi_type_for_fieldless_enum(
    emitter: &mut Emitter,
    enum_name: &Ident,
    variants: &[SpannedValue<FfiTypeVariant>],
    repr: &Repr,
) -> TokenStream {
    let enum_repr_type = get_enum_repr_type(emitter, enum_name, repr, variants.is_empty());
    let variants_len = Literal::usize_unsuffixed(variants.len());

    quote! {
        iroha_ffi::ffi_type! {
            unsafe impl Transparent for #enum_name {
                type Target = #enum_repr_type;

                validation_fn=unsafe {|target: &#enum_repr_type| {
                    (*target as usize) < #variants_len
                }},
                niche_value=<Self as iroha_ffi::FfiType>::ReprC::MAX
            }
        }

        impl iroha_ffi::WrapperTypeOf<#enum_name> for #enum_repr_type {
            type Type = #enum_name;
        }
    }
}

#[allow(clippy::too_many_lines)]
fn derive_ffi_type_for_data_carrying_enum(
    emitter: &mut Emitter,
    enum_name: &Ident,
    mut generics: syn::Generics,
    variants: &[SpannedValue<FfiTypeVariant>],
    local: bool,
) -> TokenStream {
    let (repr_c_enum_name, repr_c_enum) =
        gen_data_carrying_repr_c_enum(emitter, enum_name, &generics, variants);

    generics.make_where_clause();
    let lifetime = quote! {'__iroha_ffi_itm};
    let (impl_generics, ty_generics, where_clause) = split_for_impl(&generics);
    let mut store_generics = generics.clone();
    store_generics.params.insert(
        0,
        GenericParam::Lifetime(LifetimeParam::new(Lifetime::new(
            "'__iroha_ffi_itm",
            Span::call_site(),
        ))),
    );
    if store_generics.lt_token.is_none() {
        store_generics.lt_token = Some(Lt::default());
        store_generics.gt_token = Some(Gt::default());
    }
    let (_, store_ty_generics, store_where_clause) = split_for_impl(&store_generics);

    let variant_rust_stores = variants
        .iter()
        .map(|variant| {
            variant_mapper(
                emitter,
                variant,
                || quote! { () },
                |field| {
                    let ty = &field.ty;
                    quote! { <#ty as iroha_ffi::FfiConvert<#lifetime, <#ty as iroha_ffi::FfiType>::ReprC>>::RustStore }
                },
            )
        })
        .collect::<Vec<_>>();

    let variant_ffi_stores = variants
        .iter()
        .map(|variant| {
            variant_mapper(
                emitter,
                variant,
                || quote! { () },
                |field| {
                    let ty = &field.ty;
                    quote! { <#ty as iroha_ffi::FfiConvert<#lifetime, <#ty as iroha_ffi::FfiType>::ReprC>>::FfiStore }
                },
            )
        })
        .collect::<Vec<_>>();

    let variant_store_fields = (0..variants.len())
        .map(|idx| format_ident!("variant_{idx}"))
        .collect::<Vec<_>>();
    let rust_store_ident = format_ident!("__IrohaFfi{}RustStore", enum_name);
    let ffi_store_ident = format_ident!("__IrohaFfi{}FfiStore", enum_name);

    let rust_store_struct_fields = variant_store_fields
        .iter()
        .zip(variant_rust_stores.iter())
        .map(|(field, ty)| quote! { #field: #ty })
        .collect::<Vec<_>>();
    let ffi_store_struct_fields = variant_store_fields
        .iter()
        .zip(variant_ffi_stores.iter())
        .map(|(field, ty)| quote! { #field: #ty })
        .collect::<Vec<_>>();

    let variants_into_ffi = variants
        .iter()
        .enumerate()
        .map(|(i, variant)| {
            let idx = format!("{i}").parse::<TokenStream>().expect("Valid");
            let payload_name = gen_repr_c_enum_payload_name(enum_name);
            let variant_name = &variant.ident;
            let store_field = variant_store_fields[i].clone();

            variant_mapper(
                emitter,
                variant,
                || {
                    quote! { Self::#variant_name => #repr_c_enum_name {
                        tag: #idx, payload: #payload_name {#variant_name: ()}
                    }}
                },
                |_| {
                    quote! {
                        Self::#variant_name(payload) => {
                            let payload = #payload_name {
                                #variant_name: core::mem::ManuallyDrop::new(
                                    iroha_ffi::FfiConvert::into_ffi(payload, &mut store.#store_field)
                                )
                            };

                            #repr_c_enum_name { tag: #idx, payload }
                        }
                    }
                },
            )
        })
        .collect::<Vec<_>>();

    let variants_try_from_ffi = variants.iter().enumerate().map(|(i, variant)| {
        let idx = format!("{i}").parse::<TokenStream>().expect("Valid");
        let variant_name = &variant.ident;
        let store_field = variant_store_fields[i].clone();

        variant_mapper(
            emitter,
            variant,
            || quote! { #idx => Ok(Self::#variant_name) },
            |_| {
                quote! {
                    #idx => {
                        let payload = core::mem::ManuallyDrop::into_inner(
                            source.payload.#variant_name
                        );

                        iroha_ffi::FfiConvert::try_from_ffi(payload, &mut store.#store_field).map(Self::#variant_name)
                    }
                }
            },
        )
    }).collect::<Vec<_>>();

    let non_locality = if local {
        quote! {}
    } else {
        let mut non_local_where_clause = where_clause.unwrap().clone();

        for variant in variants {
            let Some(ty) =
                variant_mapper(emitter, variant, || None, |field| Some(field.ty.clone()))
            else {
                continue;
            };

            non_local_where_clause.predicates.push(
                syn::parse_quote! {#ty: iroha_ffi::repr_c::NonLocal<<#ty as iroha_ffi::ir::Ir>::Type>},
            );
        }

        quote! {
            unsafe impl<#impl_generics> iroha_ffi::repr_c::NonLocal<Self> for #enum_name #ty_generics #non_local_where_clause {}

            impl<#impl_generics> iroha_ffi::repr_c::CWrapperType<Self> for #enum_name #ty_generics #non_local_where_clause {
                type InputType = Self;
                type ReturnType = Self;
            }
            impl<#impl_generics> iroha_ffi::repr_c::COutPtr<Self> for #enum_name #ty_generics #non_local_where_clause {
                type OutPtr = Self::ReprC;
            }
            impl<#impl_generics> iroha_ffi::repr_c::COutPtrWrite<Self> for #enum_name #ty_generics #non_local_where_clause {
                unsafe fn write_out(self, out_ptr: *mut Self::OutPtr) {
                    iroha_ffi::repr_c::write_non_local::<_, Self>(self, out_ptr);
                }
            }
            impl<#impl_generics> iroha_ffi::repr_c::COutPtrRead<Self> for #enum_name #ty_generics #non_local_where_clause {
                unsafe fn try_read_out(out_ptr: Self::OutPtr) -> iroha_ffi::Result<Self> {
                    iroha_ffi::repr_c::read_non_local::<Self, Self>(out_ptr)
                }
            }
        }
    };

    quote! {
        #[doc(hidden)]
        pub struct #rust_store_ident #store_ty_generics #store_where_clause {
            #( #rust_store_struct_fields, )*
        }

        impl #store_ty_generics Default for #rust_store_ident #store_ty_generics #store_where_clause {
            fn default() -> Self {
                Self {
                    #( #variant_store_fields: Default::default(), )*
                }
            }
        }

        #[doc(hidden)]
        pub struct #ffi_store_ident #store_ty_generics #store_where_clause {
            #( #ffi_store_struct_fields, )*
        }

        impl #store_ty_generics Default for #ffi_store_ident #store_ty_generics #store_where_clause {
            fn default() -> Self {
                Self {
                    #( #variant_store_fields: Default::default(), )*
                }
            }
        }

        #repr_c_enum

        // NOTE: Data-carrying enum cannot implement `ReprC` unless it is robust `repr(C)`
        impl<#impl_generics> iroha_ffi::ir::Ir for #enum_name #ty_generics #where_clause {
            type Type = Self;
        }

        impl<#impl_generics> iroha_ffi::repr_c::CType<Self> for #enum_name #ty_generics #where_clause {
            type ReprC = #repr_c_enum_name #ty_generics;
        }
        impl<#lifetime, #impl_generics> iroha_ffi::repr_c::CTypeConvert<#lifetime, Self, #repr_c_enum_name #ty_generics> for #enum_name #ty_generics #where_clause {
            type RustStore = #rust_store_ident #store_ty_generics;
            type FfiStore = #ffi_store_ident #store_ty_generics;

            fn into_repr_c(self, store: &mut Self::RustStore) -> #repr_c_enum_name #ty_generics {
                match self {
                    #(#variants_into_ffi,)*
                }
            }

            unsafe fn try_from_repr_c(source: #repr_c_enum_name #ty_generics, store: &mut Self::FfiStore) -> iroha_ffi::Result<Self> {
                match source.tag {
                    #(#variants_try_from_ffi,)*
                    _ => Err(iroha_ffi::FfiReturn::TrapRepresentation)
                }
            }
        }

        // Enum transmutability can be revisited once robust `repr(C)` variants propagate the trait.
        impl<#impl_generics> iroha_ffi::repr_c::Cloned for #enum_name #ty_generics #where_clause where Self: Clone {}

        #non_locality
    }
}

fn derive_ffi_type_for_repr_c(emitter: &mut Emitter, input: &FfiTypeInput) -> TokenStream {
    verify_is_non_owning(emitter, &input.data);
    if input.repr_attr.kind.as_deref().copied() != Some(ReprKind::C) {
        let span = input
            .repr_attr
            .kind
            .map_or_else(Span::call_site, |kind| kind.span());
        emit!(
            emitter,
            span,
            "To make an FFI type robust you must annotate the type definition with `#[repr(C)]`. \
             If that is not possible, add `#[ffi_type(opaque)]` to the same item as this derive to mark it opaque"
        );
    }

    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let name = &input.ident;

    quote! {
        iroha_ffi::ffi_type! {
            impl #impl_generics Robust for #name #ty_generics #where_clause {}
        }
    }
}

fn gen_data_carrying_repr_c_enum(
    emitter: &mut Emitter,
    enum_name: &Ident,
    generics: &syn::Generics,
    variants: &[SpannedValue<FfiTypeVariant>],
) -> (Ident, TokenStream) {
    let (payload_name, payload) =
        gen_data_carrying_enum_payload(emitter, enum_name, generics, variants);
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let doc = format!(" [`ReprC`] equivalent of [`{enum_name}`]");
    let enum_tag_type = gen_enum_tag_type(variants);
    let repr_c_enum_name = gen_repr_c_enum_name(enum_name);

    let repr_c_enum = quote! {
        #payload

        #[repr(C)]
        #[doc = #doc]
        #[derive(Clone)]
        #[allow(non_camel_case_types)]
        pub struct #repr_c_enum_name #impl_generics #where_clause {
            tag: #enum_tag_type, payload: #payload_name #ty_generics,
        }

        impl #impl_generics Copy for #repr_c_enum_name #ty_generics where #payload_name #ty_generics: Copy {}
        unsafe impl #impl_generics iroha_ffi::ReprC for #repr_c_enum_name #ty_generics #where_clause {}
    };

    (repr_c_enum_name, repr_c_enum)
}

fn gen_data_carrying_enum_payload(
    emitter: &mut Emitter,
    enum_name: &Ident,
    generics: &syn::Generics,
    variants: &[SpannedValue<FfiTypeVariant>],
) -> (Ident, TokenStream) {
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let field_names = variants.iter().map(|variant| &variant.ident);
    let payload_name = gen_repr_c_enum_payload_name(enum_name);
    let doc = format!(" [`ReprC`] equivalent of [`{enum_name}`]");

    let field_tys = variants
        .iter()
        .map(|variant| {
            variant_mapper(
                emitter,
                variant,
                || quote! {()},
                |field| {
                    let field_ty = &field.ty;
                    quote! {core::mem::ManuallyDrop<<#field_ty as iroha_ffi::FfiType>::ReprC>}
                },
            )
        })
        .collect::<Vec<_>>();

    let payload = quote! {
        #[repr(C)]
        #[doc = #doc]
        #[derive(Clone)]
        #[allow(non_snake_case, non_camel_case_types)]
        pub union #payload_name #impl_generics #where_clause {
            #(#field_names: #field_tys),*
        }

        impl #impl_generics Copy for #payload_name #ty_generics where #( #field_tys: Copy ),* {}
        unsafe impl #impl_generics iroha_ffi::ReprC for #payload_name #ty_generics #where_clause {}
    };

    (payload_name, payload)
}

fn variant_mapper<T: Sized, F0: FnOnce() -> T, F1: FnOnce(&FfiTypeField) -> T>(
    emitter: &mut Emitter,
    variant: &SpannedValue<FfiTypeVariant>,
    unit_mapper: F0,
    field_mapper: F1,
) -> T {
    match &variant.as_ref().fields.style {
        Style::Tuple if variant.as_ref().fields.fields.len() == 1 => {
            field_mapper(&variant.as_ref().fields.fields[0])
        }
        Style::Tuple => {
            emit!(
                emitter,
                variant.span(),
                "Only unit or single unnamed field variants supported"
            );
            unit_mapper()
        }
        Style::Struct => {
            emit!(
                emitter,
                variant.span(),
                "Only unit or single unnamed field variants supported"
            );
            unit_mapper()
        }
        Style::Unit => unit_mapper(),
    }
}

fn gen_repr_c_enum_name(enum_name: &Ident) -> Ident {
    Ident::new(&format!("__iroha_ffi__ReprC{enum_name}"), Span::call_site())
}

fn gen_repr_c_enum_payload_name(enum_name: &Ident) -> Ident {
    Ident::new(
        &format!("__iroha_ffi__{enum_name}Payload"),
        Span::call_site(),
    )
}

// NOTE: Except for the raw pointers there should be no other type
// that is at the same time Robust and also transfers ownership
/// Verifies for each pointer type found inside the `FfiTypeData` that it is marked as non-owning
fn verify_is_non_owning(emitter: &mut Emitter, data: &FfiTypeData) {
    struct PtrVisitor<'a> {
        emitter: &'a mut Emitter,
    }
    impl syn::visit::Visit<'_> for PtrVisitor<'_> {
        fn visit_type_ptr(&mut self, node: &syn::TypePtr) {
            emit!(
                self.emitter,
                node,
                "Raw pointer found. If the pointer doesn't own the data, attach `#[ffi_type(unsafe {{non_owning}})` to the field. Otherwise, mark the entire type as opaque with `#[ffi_type(opaque)]`"
            );
        }
    }

    fn visit_field(ptr_visitor: &mut PtrVisitor, field: &FfiTypeField) {
        if field.ffi_type_attr.kind == Some(FfiTypeKindFieldAttribute::UnsafeNonOwning) {
            return;
        }
        ptr_visitor.visit_type(&field.ty);
    }

    let mut ptr_visitor = PtrVisitor { emitter };
    match data {
        FfiTypeData::Enum(variants) => {
            for variant in variants {
                for field in variant.as_ref().fields.iter() {
                    visit_field(&mut ptr_visitor, field);
                }
            }
        }
        FfiTypeData::Struct(fields) => {
            for field in fields.iter() {
                visit_field(&mut ptr_visitor, field);
            }
        }
    }
}

fn get_enum_repr_type(
    emitter: &mut Emitter,
    enum_name: &Ident,
    repr: &Repr,
    is_empty: bool,
) -> syn::Type {
    let Some(kind) = repr.kind else {
        // empty enums are not allowed to have a `#[repr]` attribute
        // it's an error to use an `#[derive(FfiType)]` on them
        // but we still want to generate a reasonable error message, so we check for it here
        if !is_empty {
            emit!(
                emitter,
                enum_name,
                "Enum representation is not specified. Try adding `#[repr(u32)]` or similar"
            );
        }
        return syn::parse_quote! {u32};
    };

    let ReprKind::Primitive(primitive) = &*kind else {
        emit!(
            emitter,
            &kind.span(),
            "Enum should have a primitive representation (like `#[repr(u32)]`)"
        );
        return syn::parse_quote! {u32};
    };

    match primitive {
        ReprPrimitive::U8 => syn::parse_quote! {u8},
        ReprPrimitive::U16 => syn::parse_quote! {u16},
        ReprPrimitive::U32 => syn::parse_quote! {u32},
        ReprPrimitive::U64 => syn::parse_quote! {u64},
        ReprPrimitive::I8 => syn::parse_quote! {i8},
        ReprPrimitive::I16 => syn::parse_quote! {i16},
        ReprPrimitive::I32 => syn::parse_quote! {i32},

        _ => {
            emit!(
                emitter,
                &kind.span(),
                "Enum representation is not supported"
            );
            syn::parse_quote! {u32}
        }
    }
}

fn gen_enum_tag_type(variants: &[SpannedValue<FfiTypeVariant>]) -> TokenStream {
    const U8_MAX: usize = u8::MAX as usize;
    const U16_MAX: usize = u16::MAX as usize;
    const U32_MAX: usize = u32::MAX as usize;

    // NOTE: Arms are matched in the order of declaration
    #[allow(clippy::match_overlapping_arm)]
    match variants.len() {
        0..=U8_MAX => quote! {u8},
        0..=U16_MAX => quote! {u16},
        0..=U32_MAX => quote! {u32},
        _ => {
            // I don't think ANYONE will ever see this error lol
            unreachable!("Come get your easter egg!");
        }
    }
}

fn split_for_impl(
    generics: &syn::Generics,
) -> (
    syn::punctuated::Punctuated<syn::GenericParam, syn::Token![,]>,
    syn::TypeGenerics<'_>,
    Option<&syn::WhereClause>,
) {
    let impl_generics = generics.params.clone();
    let (_, ty_generics, where_clause) = generics.split_for_impl();
    (impl_generics, ty_generics, where_clause)
}
