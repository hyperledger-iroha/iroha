//! Binary payload reconstruction and typed frame decoder generation.

use super::*;

fn decode_from_archived_body() -> TokenStream2 {
    quote! {
        let value = <Self as norito::core::DeserializePayload>::try_deserialize(
            __archived,
        )?;
        Ok((value, __logical_len))
    }
}

fn sequential_deserialize_value(
    field: &StructField<'_>,
    generics: &mut Generics,
) -> Option<TokenStream2> {
    let ty = &field.field.ty;
    if field.attrs.skip {
        add_bound(generics, ty, quote!(Default));
        return None;
    }
    // Binary V1 fields are positional and mandatory. `default` remains a JSON
    // input policy and must never synthesize an omitted binary field.
    let decode = if field.attrs.flatten {
        add_bound(
            generics,
            ty,
            quote!(for<'__d> norito::core::DecodeFromSlice<'__d>),
        );
        quote! {
            (|| -> ::core::result::Result<#ty, norito::core::Error> {
                let (base, total) = norito::core::payload_ctx()
                    .ok_or(norito::core::Error::MissingPayloadContext)?;
                let start = (ptr as usize).saturating_sub(base);
                let payload = unsafe {
                    std::slice::from_raw_parts(base as *const u8, total)
                };
                let field_data = payload
                    .get(start + offset..)
                    .ok_or(norito::core::Error::LengthMismatch)?;
                let _flatten_guard = norito::core::SequentialOverrideGuard::enter();
                let (value, consumed) =
                    <#ty as norito::core::DecodeFromSlice>::decode_from_slice(field_data)?;
                offset += consumed;
                Ok(value)
            })()
        }
    } else if let Some(length) = u8_array_len(ty) {
        quote! {
            norito::core::decode_context_framed_byte_array::<{ #length }>(ptr, &mut offset)
        }
    } else {
        add_bound(
            generics,
            ty,
            quote!(for<'__d> norito::core::DeserializePayload<'__d>),
        );
        add_bound(generics, ty, quote!(norito::core::SerializePayload));
        quote! {
            norito::core::decode_context_field_canonical::<#ty>(ptr, &mut offset)
        }
    };
    Some(quote! { (#decode)? })
}

/// Generate payload reconstruction and, when requested, the typed frame contract.
pub(super) fn derive_struct_deserialize(
    ident: &syn::Ident,
    generics: &Generics,
    fields: &Fields,
    container_attrs: &[Attribute],
    schema_name: Option<&str>,
    framed: bool,
) -> TokenStream2 {
    let validation = match ContainerAttr::parse(container_attrs) {
        Ok(attrs) => attrs.validate,
        Err(error) => return error.to_compile_error(),
    };
    let validated_value = decode_validation::value(validation.as_ref(), quote!(__value));
    let mut r#gen = generics.clone();
    let parsed_fields = struct_fields(fields);
    let has_flatten_fields = struct_has_flatten(&parsed_fields);
    let deserialize_fields = match fields {
        Fields::Named(_) => parsed_fields
            .iter()
            .map(|field| {
                let member = &field.member;
                sequential_deserialize_value(field, &mut r#gen).map_or_else(
                    || quote! { #member: Default::default() },
                    |value| quote! { #member: { #value } },
                )
            })
            .collect::<Vec<_>>(),
        Fields::Unnamed(_) => parsed_fields
            .iter()
            .enumerate()
            .map(|(index, field)| {
                let binding = format_ident!("field{}", index);
                sequential_deserialize_value(field, &mut r#gen).map_or_else(
                    || quote! { let #binding = Default::default(); },
                    |value| quote! { let #binding = #value; },
                )
            })
            .collect(),
        Fields::Unit => Vec::new(),
    };

    let mut impl_gen = r#gen.clone();
    impl_gen.params.insert(0, syn::parse_quote!('de));
    let (impl_generics, _, where_clause) = impl_gen.split_for_impl();
    let (_, ty_generics, _) = r#gen.split_for_impl();
    let frame_impl = if framed {
        let schema_hash_body = schema_hash_body(schema_name);
        quote! {
            impl #impl_generics norito::core::NoritoDeserialize<'de> for #ident #ty_generics #where_clause {
                #[inline]
                fn schema_hash() -> [u8; 16] { #schema_hash_body }
            }
        }
    } else {
        TokenStream2::new()
    };

    let field_bitset_enabled_decode = if struct_has_signature_like(&parsed_fields) {
        quote! { false }
    } else {
        quote! { norito::core::use_field_bitset() }
    };
    let field_bitset_enabled_decode_named = field_bitset_enabled_decode.clone();
    let field_bitset_enabled_decode_unnamed = field_bitset_enabled_decode;
    let expected_field_bitset = packed_field_bitset_from(&parsed_fields);
    let expected_field_bitset = quote! { [ #( #expected_field_bitset ),* ] };

    match fields {
        Fields::Named(_) => {
            let packed_named_count = active_struct_fields(&parsed_fields).count();
            // Build packed-struct named field initializers for the offset-table layout.
            let packed_named_inits: Vec<TokenStream2> = match fields {
                Fields::Named(named) => named
                    .named
                    .iter()
                    .map(|f| {
                        let attrs = FieldAttr::parse_validated(&f.attrs);
                        let name = f.ident.as_ref().unwrap();
                        let ty = &f.ty;
                        if attrs.skip {
                            quote! { #name: Default::default() }
                        } else {
                            quote! {
                                #name: {
                                    let mut __start = __offs[__i];
                                    let __end = __offs[__i + 1];
                                    __i += 1;
                                    let __len = __end - __start;
                                    #[cfg(debug_assertions)]
                                    if norito::debug_trace_enabled() {
                                        eprintln!(
                                            "packed decode {}::{} start={} end={} len={} ty={}",
                                            stringify!(#ident),
                                            stringify!(#name),
                                            __start,
                                            __end,
                                            __len,
                                            core::any::type_name::<#ty>(),
                                        );
                                    }
                                    norito::core::decode_context_field_fixed_canonical::<#ty>(
                                        data_base,
                                        &mut __start,
                                        __len,
                                    )?
                                }
                            }
                        }
                    })
                    .collect(),
                _ => Vec::new(),
            };
            let named_bit_positions = packed_bit_positions(&parsed_fields);
            // Build packed-struct named field initializers (hybrid bitset-based sequential decode)
            let packed_named_inits_hybrid: Vec<TokenStream2> = match fields {
                Fields::Named(named) => named
                    .named
                    .iter()
                    .enumerate()
                    .map(|(i, f)| {
                        let attrs = FieldAttr::parse_validated(&f.attrs);
                        let name = f.ident.as_ref().unwrap();
                        let ty = &f.ty;
                        let fixed_size = is_fixed_size(ty);
                        let sequential_decode_named = if is_option_type(ty) {
                            let inner_ty = option_inner_type(ty).expect("Option inner type");
                            quote! {
                                let ptr2 = unsafe { data_base.add(__data_off) };
                                let remaining = total_rem
                                    .checked_sub(__data_off)
                                    .ok_or(norito::core::Error::LengthMismatch)?;
                                let slice = unsafe { std::slice::from_raw_parts(ptr2, remaining) };
                                if slice.len() < 4 {
                                    return Err(norito::core::Error::LengthMismatch);
                                }
                                let tag = u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]);
                                match tag {
                                    0 => {
                                        __data_off += 4;
                                        Default::default()
                                    }
                                    1 => {
                                        let value_slice = &slice[4..];
                                        let (inner, used) = match norito::core::decode_field_canonical::<#inner_ty>(value_slice) {
                                            Ok(res) => res,
                                            Err(err) => return Err(err),
                                        };
                                        __data_off += 4 + used;
                                        Some(inner)
                                    }
                                    other => {
                                        return Err(norito::core::Error::invalid_tag(
                                            "Option::try_deserialize",
                                            (other & 0xFF) as u8,
                                        ));
                                    }
                                }
                            }
                        } else {
                            quote! {
                                norito::core::decode_context_field_prefix::<#ty>(
                                    data_base,
                                    &mut __data_off,
                                )?
                            }
                        };
                        let sequential_decode_named_without_size = sequential_decode_named.clone();
                        if attrs.skip {
                            quote! { #name: Default::default() }
                        } else if let Some(len_expr) = u8_array_len(ty) {
                            quote!{
                                #name: {
                                    norito::core::decode_context_byte_array::<{ #len_expr }>(
                                        data_base,
                                        &mut __data_off,
                                    )?
                                }
                            }
                        } else if is_self_delimiting(ty) {
                            quote!{
                                #name: {
                                    norito::core::decode_context_field_prefix::<#ty>(
                                        data_base,
                                        &mut __data_off,
                                    )?
                                }
                            }
                        } else if let Some(fixed_len) = fixed_size {
                            let fixed_len_lit = fixed_len;
                            quote!{
                                #name: {
                                    norito::core::decode_context_field_fixed_canonical::<#ty>(
                                        data_base,
                                        &mut __data_off,
                                        #fixed_len_lit,
                                    )?
                                }
                            }
                        } else if is_signature_like(ty) {
                            let __bitpos_val: usize = named_bit_positions[i].expect("bitpos");
                            quote!{
                                #name: {
                                    let __need = (((*__bitset.get(#__bitpos_val / 8).unwrap_or(&0)) >> (((#__bitpos_val % 8) as u8)) ) & 1) != 0;
                                    if __need {
                                        let __len = *__sizes
                                            .get(__sz_i)
                                            .ok_or(norito::core::Error::LengthMismatch)?;
                                        if norito::debug_trace_enabled() {
                                            eprintln!("packed signature decode len={}", __len);
                                        }
                                        __sz_i += 1;
                                        norito::core::decode_context_field_fixed_canonical::<#ty>(
                                            data_base,
                                            &mut __data_off,
                                            __len,
                                        )?
                                    } else {
                                        #sequential_decode_named_without_size
                                    }
                                }
                            }
                        } else {
                            let __bitpos_val: usize = named_bit_positions[i].expect("bitpos");
                            quote!{
                                #name: {
                                    let __need = (((*__bitset.get(#__bitpos_val / 8).unwrap_or(&0)) >> (((#__bitpos_val % 8) as u8)) ) & 1) != 0;
                                    if __need {
                                        let __len = *__sizes
                                            .get(__sz_i)
                                            .ok_or(norito::core::Error::LengthMismatch)?;
                                        __sz_i += 1;
                                        norito::core::decode_context_field_fixed_canonical::<#ty>(
                                            data_base,
                                            &mut __data_off,
                                            __len,
                                        )?
                                    } else { #sequential_decode_named }
                                }
                            }
                        }
                    })
                    .collect(),
                _ => Vec::new(),
            };
            let validated_slice_value =
                decode_validation::binding(validation.as_ref(), &format_ident!("value"));
            let __decode_from_slice_impl = slice_decode::derive(
                ident,
                &r#gen,
                container_attrs,
                quote! {
                    let ptr = __archived as *const _ as *const u8;
                    let mut offset = 0usize;
                    let value = Self { #(#deserialize_fields),* };
                    #validated_slice_value
                    Ok((value, offset))
                },
            );

            quote! {
                #frame_impl
                impl #impl_generics norito::core::DeserializePayload<'de> for #ident #ty_generics #where_clause {
                    fn deserialize(archived: &'de norito::core::Archived<Self>) -> Self {
                        match <Self as norito::core::DeserializePayload<'de>>::try_deserialize(archived) {
                            Ok(value) => value,
                            Err(err) => panic!(
                                concat!(
                                    "norito: fallible deserialize failed for ",
                                    stringify!(#ident),
                                    ": {:?}"
                                ),
                                err
                            ),
                        }
                    }
                    fn try_deserialize(archived: &'de norito::core::Archived<Self>) -> ::core::result::Result<Self, norito::core::Error> {
                        let ptr = archived as *const _ as *const u8;
                        if norito::debug_trace_enabled() {
                            if let Some((__base, __total)) = norito::core::payload_ctx() {
                                eprintln!(
                                    "decode struct {} ptr_off={} total={}",
                                    stringify!(#ident),
                                    (ptr as usize).saturating_sub(__base),
                                    __total
                                );
                            }
                            if let Some((__base_dbg, __total_dbg)) = norito::core::payload_ctx() {
                                let __start_dbg = (ptr as usize).saturating_sub(__base_dbg);
                                let __payload_dbg = unsafe {
                                    std::slice::from_raw_parts(__base_dbg as *const u8, __total_dbg)
                                };
                                let __available_dbg = __payload_dbg.len().saturating_sub(__start_dbg);
                                let __preview_dbg = __available_dbg.min(32);
                                let __view_dbg =
                                    &__payload_dbg[__start_dbg..__start_dbg + __preview_dbg];
                                eprintln!(
                                    "decode struct {} payload preview {:?}",
                                    stringify!(#ident),
                                    __view_dbg
                                );
                            }
                        }
                        let __value = if !#has_flatten_fields && norito::core::use_packed_struct() {
                            let mut __o = 0usize;
                            let __count: usize = #packed_named_count;
                            // Hybrid packed-struct is indicated by the field-bitset flag.
                            // Packed-struct sizes follow COMPACT_LEN; packed-seq offsets
                            // are fixed-width in v1.
                            if #field_bitset_enabled_decode_named {
                                // Hybrid: read bitset, then sizes for needed fields; decode sequentially.
                                let __expected_bitset: &[u8] = &#expected_field_bitset;
                                let (__bitset, __sizes, __header_len) =
                                    norito::core::decode_context_packed_header(
                                        ptr,
                                        __count,
                                        __expected_bitset,
                                    )?;
                                if norito::debug_trace_enabled() {
                                    eprintln!(
                                        "decode struct {} bitset bytes={:?}",
                                        stringify!(#ident),
                                        __bitset
                                    );
                                }
                                __o = __header_len;
                                let data_base = unsafe { ptr.add(__o) };
                                let (base, total) = if let Some(ctx) = norito::core::payload_ctx() {
                                    ctx
                                } else {
                                    return Err(norito::core::Error::MissingPayloadContext);
                                };
                                let base_off = (data_base as usize).saturating_sub(base);
                                let total_rem = total.saturating_sub(base_off);
                                let mut __data_off = 0usize;
                                let mut __sz_i = 0usize;
                                // Initialize fields in order
                                let __value = Self { #(#packed_named_inits_hybrid),* };
                                __o = __o.checked_add(__data_off)
                                    .ok_or(norito::core::Error::LengthMismatch)?;
                                norito::core::finish_context_fields(ptr, __o)?;
                                __value
                            } else {
                                // Read the advertised offset-table layout.
                                let (
                                    __offs,
                                    __used_offs,
                                    __packed_data_len,
                                    __packed_tail_len,
                                ) = norito::core::decode_context_packed_offsets(ptr, __count)?;
                                __o = __o
                                    .checked_add(__used_offs)
                                    .ok_or(norito::core::Error::LengthMismatch)?;
                                #[cfg(debug_assertions)]
                                if norito::debug_trace_enabled() {
                                    eprintln!(
                                        "decode struct {} offsets {:?} used={} count={} data_len={} tail_len={}",
                                        stringify!(#ident),
                                        __offs,
                                        __used_offs,
                                        __count,
                                        __packed_data_len,
                                        __packed_tail_len
                                    );
                                }
                                let data_base = unsafe { ptr.add(__o) };
                                let __packed_data_len_local = __packed_data_len;
                                let __packed_tail_len_local = __packed_tail_len;
                                let mut __i = 0usize;
                                let __value = Self { #(#packed_named_inits),* };
                                __o = __o
                                    .checked_add(__packed_data_len_local)
                                    .and_then(|v| v.checked_add(__packed_tail_len_local))
                                    .ok_or(norito::core::Error::LengthMismatch)?;
                                norito::core::finish_context_fields(ptr, __o)?;
                                __value
                            }
                        } else {
                            let mut offset = 0usize;
                            let __value = Self { #(#deserialize_fields),* };
                            norito::core::finish_context_fields(ptr, offset)?;
                            __value
                        };
                        #validated_value
                    }
                }
                #__decode_from_slice_impl
            }
        }
        Fields::Unnamed(unnamed) => {
            let vars: Vec<_> = (0..unnamed.unnamed.len())
                .map(|i| format_ident!("field{}", i))
                .collect();
            let packed_unnamed_count = active_struct_fields(&parsed_fields).count();
            // Build packed-struct unnamed field statements for the offset-table layout.
            let packed_unnamed_stmts: Vec<TokenStream2> = match fields {
                Fields::Unnamed(unnamed) => unnamed
                    .unnamed
                    .iter()
                    .enumerate()
                    .map(|(i, f)| {
                        let attrs = FieldAttr::parse_validated(&f.attrs);
                        let idx_var = format_ident!("field{}", i);
                        let ty = &f.ty;
                        if attrs.skip {
                            quote! { let #idx_var = Default::default(); }
                        } else {
                            quote! {
                                let #idx_var = {
                                    let mut __start = __offs[__i];
                                    let __end = __offs[__i + 1];
                                    __i += 1;
                                    let __len = __end - __start;
                                    norito::core::decode_context_field_fixed_canonical::<#ty>(
                                        data_base,
                                        &mut __start,
                                        __len,
                                    )?
                                };
                            }
                        }
                    })
                    .collect(),
                _ => Vec::new(),
            };
            let unnamed_bit_positions = packed_bit_positions(&parsed_fields);
            // Build packed-struct unnamed field statements (hybrid bitset-based)
            let packed_unnamed_stmts_hybrid: Vec<TokenStream2> = match fields {
                Fields::Unnamed(unnamed) => unnamed
                    .unnamed
                    .iter()
                    .enumerate()
                    .map(|(i, f)| {
                        let attrs = FieldAttr::parse_validated(&f.attrs);
                        let idx_var = format_ident!("field{}", i);
                        let ty = &f.ty;
                        let fixed_size = is_fixed_size(ty);
                        if attrs.skip {
                            quote! { let #idx_var = Default::default(); }
                        } else if let Some(len_expr) = u8_array_len(ty) {
                            quote! {
                                let #idx_var = {
                                    norito::core::decode_context_byte_array::<{ #len_expr }>(
                                        data_base,
                                        &mut __data_off,
                                    )?
                                };
                            }
                        } else if is_self_delimiting(ty) {
                            quote! {
                                let #idx_var = {
                                    norito::core::decode_context_field_prefix::<#ty>(
                                        data_base,
                                        &mut __data_off,
                                    )?
                                };
                            }
                        } else if let Some(fixed_len) = fixed_size {
                            let fixed_len_lit = fixed_len;
                            quote! {
                                let #idx_var = {
                                    norito::core::decode_context_field_fixed_canonical::<#ty>(
                                        data_base,
                                        &mut __data_off,
                                        #fixed_len_lit,
                                    )?
                                };
                            }
                        } else {
                            let __ubitpos_val: usize = unnamed_bit_positions[i].expect("ubitpos");
                            quote!{
                                let #idx_var = {
                                    let __need = (((*__bitset.get(#__ubitpos_val / 8).unwrap_or(&0)) >> (((#__ubitpos_val % 8) as u8)) ) & 1) != 0;
                                    if __need {
                                        let __len = *__sizes
                                            .get(__sz_i)
                                            .ok_or(norito::core::Error::LengthMismatch)?;
                                        __sz_i += 1;
                                        norito::core::decode_context_field_fixed_canonical::<#ty>(
                                            data_base,
                                            &mut __data_off,
                                            __len,
                                        )?
                                    } else {
                                        norito::core::decode_context_field_prefix::<#ty>(
                                            data_base,
                                            &mut __data_off,
                                        )?
                                    }
                                };
                            }
                        }
                    })
                    .collect(),
                _ => Vec::new(),
            };
            let __decode_from_slice_impl =
                slice_decode::derive(ident, &r#gen, container_attrs, decode_from_archived_body());
            quote! {
                #frame_impl
                impl #impl_generics norito::core::DeserializePayload<'de> for #ident #ty_generics #where_clause {
                    fn deserialize(archived: &'de norito::core::Archived<Self>) -> Self {
                        match <Self as norito::core::DeserializePayload<'de>>::try_deserialize(archived) {
                            Ok(value) => value,
                            Err(err) => panic!(
                                concat!(
                                    "norito: fallible deserialize failed for ",
                                    stringify!(#ident),
                                    ": {:?}"
                                ),
                                err
                            ),
                        }
                    }
                    fn try_deserialize(archived: &'de norito::core::Archived<Self>) -> ::core::result::Result<Self, norito::core::Error> {
                        let ptr = archived as *const _ as *const u8;
                        let __value = if norito::core::use_packed_struct() {
                            let mut __o = 0usize;
                            let __count: usize = #packed_unnamed_count;
                            // Hybrid packed-struct is signaled by FIELD_BITSET; packed-seq
                            // offset flags are reserved and unused for structs.
                            if #field_bitset_enabled_decode_unnamed {
                                // Read the presence bitset for unnamed fields (hybrid decoding)
                                let __expected_bitset: &[u8] = &#expected_field_bitset;
                                let (__bitset, __sizes, __header_len) =
                                    norito::core::decode_context_packed_header(
                                        ptr,
                                        __count,
                                        __expected_bitset,
                                    )?;
                                __o = __header_len;
                                // Decode payload sequentially
                                let data_base = unsafe { ptr.add(__o) };
                                let mut __data_off = 0usize;
                                let mut __sz_i = 0usize;
                                #(#packed_unnamed_stmts_hybrid)*
                                __o = __o.checked_add(__data_off)
                                    .ok_or(norito::core::Error::LengthMismatch)?;
                                norito::core::finish_context_fields(ptr, __o)?;
                                Self( #(#vars),* )
                            } else {
                                let (
                                    __offs,
                                    __used_offs,
                                    __packed_data_len,
                                    __packed_tail_len,
                                ) = norito::core::decode_context_packed_offsets(ptr, __count)?;
                                __o = __o
                                    .checked_add(__used_offs)
                                    .ok_or(norito::core::Error::LengthMismatch)?;
                                #[cfg(debug_assertions)]
                                if norito::debug_trace_enabled() {
                                    eprintln!(
                                        "decode struct {} offsets {:?} used={} count={} data_len={} tail_len={}",
                                        stringify!(#ident),
                                        __offs,
                                        __used_offs,
                                        __count,
                                        __packed_data_len,
                                        __packed_tail_len
                                    );
                                }
                                let data_base = unsafe { ptr.add(__o) };
                                let __packed_data_len_local = __packed_data_len;
                                let __packed_tail_len_local = __packed_tail_len;
                                let mut __i = 0usize;
                                #(#packed_unnamed_stmts)*
                                __o = __o
                                    .checked_add(__packed_data_len_local)
                                    .and_then(|v| v.checked_add(__packed_tail_len_local))
                                    .ok_or(norito::core::Error::LengthMismatch)?;
                                norito::core::finish_context_fields(ptr, __o)?;
                                Self( #(#vars),* )
                            }
                        } else {
                            let mut offset = 0usize;
                            #(#deserialize_fields)*
                            let __value = Self( #(#vars),* );
                            norito::core::finish_context_fields(ptr, offset)?;
                            __value
                        };
                        #validated_value
                    }
                }
                #__decode_from_slice_impl
            }
        }
        Fields::Unit => {
            let unit_methods = decode_validation::unit_methods(ident, validation.as_ref());
            let __decode_from_slice_impl =
                slice_decode::derive(ident, &r#gen, container_attrs, decode_from_archived_body());
            quote! {
                #frame_impl
                impl #impl_generics norito::core::DeserializePayload<'de> for #ident #ty_generics #where_clause {
                    #unit_methods
                }
                #__decode_from_slice_impl
            }
        }
    }
}

/// Generate enum reconstruction with positional fields and an optional frame contract.
pub(super) fn derive_enum_deserialize(
    ident: &syn::Ident,
    generics: &Generics,
    data: &DataEnum,
    container_attrs: &[Attribute],
    schema_name: Option<&str>,
    framed: bool,
) -> TokenStream2 {
    let mut r#gen = generics.clone();
    let validation = match ContainerAttr::parse(container_attrs) {
        Ok(attrs) => attrs.validate,
        Err(error) => return error.to_compile_error(),
    };
    let validated_value = decode_validation::value(validation.as_ref(), quote!(value));
    let mut arms = Vec::new();
    let discriminants = match enum_variant_indices(data) {
        Ok(discriminants) => discriminants,
        Err(error) => return error.to_compile_error(),
    };

    for (variant, disc) in data.variants.iter().zip(discriminants) {
        let v_ident = &variant.ident;
        match &variant.fields {
            Fields::Unit => arms.push(quote! {
                #disc => {
                    let offset = 4usize;
                    norito::core::finish_context_fields(ptr, offset)?;
                    Self::#v_ident
                }
            }),
            Fields::Unnamed(fields) => {
                let deser_stmts: Vec<TokenStream2> = fields
                    .unnamed
                    .iter()
                    .enumerate()
                    .map(|(i, f)| {
                        let attrs = FieldAttr::parse_validated(&f.attrs);
                        let ty = &f.ty;
                        let idx_var = format_ident!("field{}", i);
                        if attrs.skip {
                            add_bound(&mut r#gen, ty, quote!(Default));
                            quote! {
                                let #idx_var = Default::default();
                            }
                        } else {
                            add_bound(&mut r#gen, ty, quote!(for<'__d> norito::core::DeserializePayload<'__d>));
                            add_bound(&mut r#gen, ty, quote!(norito::core::SerializePayload));
                            let is_sd = is_self_delimiting(&f.ty);
                            let fixed_size = is_fixed_size(&f.ty);
                            let is_fixed = fixed_size.is_some();
                            let decode = if is_sd || is_fixed {
                                if let Some(len_expr) = u8_array_len(ty) {
                                    quote! {
                                        if norito::core::use_packed_struct() {
                                            norito::core::decode_context_byte_array::<{ #len_expr }>(
                                                ptr,
                                                &mut offset,
                                            )
                                        } else {
                                            norito::core::decode_context_field_canonical::<#ty>(
                                                ptr,
                                                &mut offset,
                                            )
                                        }
                                    }
                                } else {
                                    // Distinguish self-delimiting vs fixed-size for packed enums.
                                    if is_sd {
                                        quote! {
                                            if norito::core::use_packed_struct() {
                                                norito::core::decode_context_field_prefix::<#ty>(
                                                    ptr,
                                                    &mut offset,
                                                )
                                            } else {
                                                norito::core::decode_context_field_canonical::<#ty>(
                                                    ptr,
                                                    &mut offset,
                                                )
                                            }
                                        }
                                    } else {
                                        // Fixed-size (non [u8;N]) unnamed variant field
                                        let fixed_len_lit = fixed_size.expect("fixed-size field");
                                        quote! {
                                            if norito::core::use_packed_struct() {
                                                norito::core::decode_context_field_fixed_canonical::<#ty>(
                                                    ptr,
                                                    &mut offset,
                                                    #fixed_len_lit,
                                                )
                                            } else {
                                                norito::core::decode_context_field_canonical::<#ty>(
                                                    ptr,
                                                    &mut offset,
                                                )
                                            }
                                        }
                                    }
                                }
                            } else {
                                let is_opt_res = is_option_or_result(ty);
                                if is_opt_res {
                                    quote! {
                                        norito::core::decode_context_field_canonical::<#ty>(
                                            ptr,
                                            &mut offset,
                                        )
                                    }
                                } else if is_vec_type(ty) {
                                    quote! {
                                        if norito::core::use_packed_struct() {
                                            norito::core::decode_context_field_canonical::<#ty>(
                                                ptr,
                                                &mut offset,
                                            )
                                        } else {
                                            norito::core::decode_context_field_canonical::<#ty>(
                                                ptr,
                                                &mut offset,
                                            )
                                        }
                                    }
                                } else {
                                    quote! {
                                        norito::core::decode_context_field_canonical::<#ty>(
                                            ptr,
                                            &mut offset,
                                        )
                                    }
                                }
                            };
                            let value = quote! { (#decode)? };
                            quote! {
                                let #idx_var = #value;
                            }
                        }
                    })
                    .collect();
                let vars: Vec<_> = (0..fields.unnamed.len())
                    .map(|i| format_ident!("field{}", i))
                    .collect();
                arms.push(quote! {
                    #disc => {
                        let mut offset = 4usize;
                        #(#deser_stmts)*
                        let __value = Self::#v_ident(#(#vars),*);
                        norito::core::finish_context_fields(ptr, offset)?;
                        __value
                    }
                });
            }
            Fields::Named(fields) => {
                let deser_stmts: Vec<TokenStream2> = fields
                    .named
                    .iter()
                    .map(|f| {
                        let attrs = FieldAttr::parse_validated(&f.attrs);
                        let name = f.ident.as_ref().unwrap();
                        let ty = &f.ty;
                        if attrs.skip {
                            add_bound(&mut r#gen, ty, quote!(Default));
                            quote! {
                                let #name = Default::default();
                            }
                        } else {
                            add_bound(&mut r#gen, ty, quote!(for<'__d> norito::core::DeserializePayload<'__d>));
                            add_bound(&mut r#gen, ty, quote!(norito::core::SerializePayload));
                            let is_sd = is_self_delimiting(&f.ty);
                            let fixed_size = is_fixed_size(&f.ty);
                            let is_fixed = fixed_size.is_some();
                            let decode = if is_sd || is_fixed {
                                if let Some(len_expr) = u8_array_len(ty) {
                                    quote! {
                                        if norito::core::use_packed_struct() {
                                            norito::core::decode_context_byte_array::<{ #len_expr }>(
                                                ptr,
                                                &mut offset,
                                            )
                                        } else {
                                            norito::core::decode_context_field_canonical::<#ty>(
                                                ptr,
                                                &mut offset,
                                            )
                                        }
                                    }
                                } else if is_sd {
                                    quote! {
                                        if norito::core::use_packed_struct() {
                                            norito::core::decode_context_field_prefix::<#ty>(
                                                ptr,
                                                &mut offset,
                                            )
                                        } else {
                                            norito::core::decode_context_field_canonical::<#ty>(
                                                ptr,
                                                &mut offset,
                                            )
                                        }
                                    }
                                } else { // fixed-size, non-[u8;N]
                                    let fixed_len_lit = fixed_size.expect("fixed-size field");
                                    quote! {
                                        if norito::core::use_packed_struct() {
                                            norito::core::decode_context_field_fixed_canonical::<#ty>(
                                                ptr,
                                                &mut offset,
                                                #fixed_len_lit,
                                            )
                                        } else {
                                            norito::core::decode_context_field_canonical::<#ty>(
                                                ptr,
                                                &mut offset,
                                            )
                                        }
                                    }
                                }
                            } else {
                                quote! {
                                    norito::core::decode_context_field_canonical::<#ty>(
                                        ptr,
                                        &mut offset,
                                    )
                                }
                            };
                            let value = quote! { (#decode)? };
                            quote! {
                                let #name = #value;
                            }
                        }
                    })
                    .collect();
                let names: Vec<_> = fields
                    .named
                    .iter()
                    .map(|f| f.ident.as_ref().unwrap())
                    .collect();
                arms.push(quote! {
                    #disc => {
                        let mut offset = 4usize;

                        #(#deser_stmts)*

                        let __value = Self::#v_ident { #(#names),* };
                        norito::core::finish_context_fields(ptr, offset)?;
                        __value
                    }
                });
            }
        }
    }

    let mut impl_gen = r#gen.clone();
    impl_gen.params.insert(0, syn::parse_quote!('de));
    let (impl_generics, _, where_clause) = impl_gen.split_for_impl();
    let (_, ty_generics, _) = r#gen.split_for_impl();
    let frame_impl = if framed {
        let schema_hash_body = schema_hash_body(schema_name);
        quote! {
            impl #impl_generics norito::core::NoritoDeserialize<'de> for #ident #ty_generics #where_clause {
                #[inline]
                fn schema_hash() -> [u8; 16] { #schema_hash_body }
            }
        }
    } else {
        TokenStream2::new()
    };

    let __decode_from_slice_impl =
        slice_decode::derive(ident, &r#gen, container_attrs, decode_from_archived_body());
    quote! {
        #frame_impl
        impl #impl_generics norito::core::DeserializePayload<'de> for #ident #ty_generics #where_clause {

            fn deserialize(archived: &'de norito::core::Archived<Self>) -> Self {
                match <Self as norito::core::DeserializePayload<'de>>::try_deserialize(archived) {
                    Ok(value) => value,
                    Err(err) => panic!(
                        concat!(
                            "norito: fallible deserialize failed for ",
                            stringify!(#ident),
                            ": {:?}"
                        ),
                        err
                    ),
                }
            }

            fn try_deserialize(archived: &'de norito::core::Archived<Self>) -> ::core::result::Result<Self, norito::core::Error> {
                let ptr = archived as *const _ as *const u8;
                // Read the tag through the active, length-bounded payload
                // context. Constructing a raw slice here would read beyond a
                // truncated archive before the decoder could return an error.
                let mut __tag_bytes = [0u8; 4];
                __tag_bytes.copy_from_slice(norito::core::payload_range_from_ptr(ptr, 4)?);
                let tag = u32::from_le_bytes(__tag_bytes);
                let value = match tag {
                    #(#arms,)*
                    _ => {
                        return Err(norito::core::Error::Message(
                            "invalid enum discriminant".into(),
                        ))
                    }
                };
                #validated_value
            }
        }
        #__decode_from_slice_impl
    }
}
