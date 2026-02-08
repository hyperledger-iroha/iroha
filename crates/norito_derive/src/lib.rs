//! Derive macros for the `norito` serialization framework.
//!
//! These macros implement [`NoritoSerialize`] and [`NoritoDeserialize`] for
//! user defined structs. The derive generates an `Archived` type alias and
//! forwards serialization of each field to the corresponding implementation.
//!
//! ```ignore
//! use norito::core::*;
//!
//! #[derive(NoritoSerialize, NoritoDeserialize)]
//! struct Point { x: u32, y: bool }
//!
//! let bytes = to_bytes(&Point { x: 1, y: false }).unwrap();
//! let archived = from_bytes::<Point>(&bytes).unwrap();
//! let decoded = <Point as NoritoDeserialize>::deserialize(archived);
//! assert_eq!(decoded.x, 1);
//! ```

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{
    Attribute, Data, DataEnum, DeriveInput, Fields, Generics, Index, Result as SynResult, Token,
    Variant, parse_macro_input, parse_quote,
};

fn consume_unknown_meta(meta: syn::meta::ParseNestedMeta) -> SynResult<()> {
    if meta.input.peek(syn::token::Paren) {
        meta.parse_nested_meta(consume_unknown_meta)?
    } else if meta.input.peek(Token![=]) {
        meta.value()?.parse::<TokenStream2>()?;
    }
    Ok(())
}

/// Returns true if the container has `#[norito(decode_from_slice)]` attribute.
fn has_decode_from_slice_attr(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| {
        if !attr.path().is_ident("norito") {
            return false;
        }
        let mut found = false;
        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("decode_from_slice") {
                found = true;
            }
            Ok(())
        });
        found
    })
}

fn reuse_archived_alias(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| {
        if !attr.path().is_ident("norito") {
            return false;
        }
        let mut reuse = false;
        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("reuse_archived") {
                reuse = true;
            }
            Ok(())
        });
        reuse
    })
}

// ---- Type classification helpers for packed-struct hybrid layout ----
// Fixed-size types either have a statically known serialized size or are
// special-cased ([u8; N]). Returns Some(byte_len) when known (the value is
// not used arithmetically in all call sites; Some(..) signifies fixed-size).
fn is_fixed_size(ty: &syn::Type) -> Option<usize> {
    match ty {
        syn::Type::Path(tp) => {
            let id = tp
                .path
                .segments
                .last()
                .map(|s| s.ident.to_string())
                .unwrap_or_default();
            match id.as_str() {
                "u8" | "i8" | "bool" => Some(1),
                "u16" | "i16" => Some(2),
                "u32" | "i32" | "f32" => Some(4),
                "u64" | "i64" | "f64" | "usize" | "isize" => Some(8),
                "u128" | "i128" => Some(16),
                "NonZeroU16" => Some(2),
                "NonZeroU32" => Some(4),
                "NonZeroU64" => Some(8),
                "ContractCodeHash" | "ContractAbiHash" | "ProposalId" => Some(36),
                "Dir" | "Role" => Some(4),
                _ => None,
            }
        }
        syn::Type::Array(arr) => {
            if let syn::Type::Path(tp) = &*arr.elem
                && tp.path.is_ident("u8")
            {
                // [u8; N] serialized as raw bytes; treat as fixed-size
                return Some(0);
            }
            None
        }
        _ => None,
    }
}

// Self-delimiting types embed their own length or allow slice-based decoding
// that consumes exactly the number of bytes they need.
fn is_self_delimiting(ty: &syn::Type) -> bool {
    match ty {
        syn::Type::Path(tp) => {
            let id = tp
                .path
                .segments
                .last()
                .map(|s| s.ident.to_string())
                .unwrap_or_default();
            // Conservative rule: only a tight allowlist of well-known
            // primitives/wrappers that are guaranteed to carry their own
            // length headers are considered self‑delimiting at the field
            // boundary. This avoids requiring `DecodeFromSlice` on arbitrary
            // user‑defined types (e.g., `*Id` newtypes/structs) which only
            // implement Norito (de)serialization but not the strict slice API.
            //
            // Collections like Vec/Map/Set/Option/Result are self‑delimiting
            // because they embed their own lengths.
            if matches!(id.as_str(), "String" | "Cow" | "ViewChangeProofPayload") {
                return true;
            }
            // A few specific data-model types are known to be self‑delimiting
            // at their field boundary.
            if matches!(
                id.as_str(),
                "Name"
                    | "Metadata"
                    | "ProofAttachment"
                    | "VerifyingKeyId"
                    | "ConstString"
                    | "PhantomData"
            ) {
                return true;
            }
            if matches!(
                id.as_str(),
                "Vec"
                    | "VecDeque"
                    | "LinkedList"
                    | "BinaryHeap"
                    | "HashMap"
                    | "BTreeMap"
                    | "HashSet"
                    | "BTreeSet"
                    | "Option"
                    | "Result"
            ) {
                return true;
            }
            false
        }
        _ => false,
    }
}

// Treat cryptographic signature wrappers as staged-size (not self-delimiting), but
// enable a precise `DecodeFromSlice` fast path during hybrid packed-struct decode.
fn is_signature_like(ty: &syn::Type) -> bool {
    match ty {
        syn::Type::Path(tp) => tp
            .path
            .segments
            .last()
            .map(|s| {
                let id = s.ident.to_string();
                id == "Signature" || id == "SignatureOf" || id.ends_with("Signature")
            })
            .unwrap_or(false),
        _ => false,
    }
}

// Dynamic wrappers that should always be staged-size under packed-struct
// (i.e., require an explicit size header in the hybrid layout).
fn is_staged_wrapper(ty: &syn::Type) -> bool {
    match ty {
        syn::Type::Path(tp) => tp
            .path
            .segments
            .last()
            .map(|s| {
                let id = s.ident.to_string();
                // Known dynamic wrappers in this workspace
                id == "ConstVec" || id == "ConstString"
            })
            .unwrap_or(false),
        _ => false,
    }
}

// Recognize `Option<..>` and `Result<..>` to enable slice-based enum decoding fast path
fn is_option_or_result(ty: &syn::Type) -> bool {
    match ty {
        syn::Type::Path(tp) => tp
            .path
            .segments
            .last()
            .map(|s| {
                let id = s.ident.to_string();
                id == "Option" || id == "Result"
            })
            .unwrap_or(false),
        _ => false,
    }
}

// Recognize `Vec<..>` to enable slice-based enum packed decode fast path
fn is_vec_type(ty: &syn::Type) -> bool {
    match ty {
        syn::Type::Path(tp) => tp
            .path
            .segments
            .last()
            .map(|s| s.ident == "Vec")
            .unwrap_or(false),
        _ => false,
    }
}

fn is_option_type(ty: &syn::Type) -> bool {
    matches!(
        ty,
        syn::Type::Path(tp)
            if tp
                .path
                .segments
                .last()
                .map(|s| s.ident == "Option")
                .unwrap_or(false)
    )
}

fn option_inner_type(ty: &syn::Type) -> Option<syn::Type> {
    if let syn::Type::Path(tp) = ty
        && let Some(seg) = tp.path.segments.last()
        && seg.ident == "Option"
        && let syn::PathArguments::AngleBracketed(args) = &seg.arguments
    {
        for arg in &args.args {
            if let syn::GenericArgument::Type(inner) = arg {
                return Some(inner.clone());
            }
        }
    }
    None
}

/// Add a trait bound to the generated `where` clause.
fn add_bound(generics: &mut Generics, ty: &syn::Type, bound: TokenStream2) {
    let where_clause = generics.make_where_clause();
    let pred: syn::WherePredicate = parse_quote!(#ty: #bound);
    where_clause.predicates.push(pred);
}

/// Validate `#[norito(...)]` attributes on fields for common misuse cases.
fn validate_field_attrs(fields: &Fields) -> Result<(), syn::Error> {
    match fields {
        Fields::Named(named) => {
            for f in &named.named {
                let attrs = FieldAttr::parse(&f.attrs);
                if let Some(err) = attrs.error {
                    return Err(err);
                }
                if attrs.skip && attrs.default {
                    return Err(syn::Error::new_spanned(
                        f,
                        "conflicting attributes: #[norito(skip)] and #[norito(default)]",
                    ));
                }
                if attrs.flatten {
                    if attrs.rename.is_some() {
                        return Err(syn::Error::new_spanned(
                            f,
                            "#[norito(flatten)] cannot be combined with #[norito(rename = ...)]",
                        ));
                    }
                    if attrs.with.is_some() {
                        return Err(syn::Error::new_spanned(
                            f,
                            "#[norito(flatten)] cannot be combined with #[norito(with = ...)]",
                        ));
                    }
                    if attrs.skip_serializing_if.is_some() {
                        return Err(syn::Error::new_spanned(
                            f,
                            "#[norito(flatten)] cannot be combined with #[norito(skip_serializing_if = ...)]",
                        ));
                    }
                }
            }
        }
        Fields::Unnamed(unnamed) => {
            for f in &unnamed.unnamed {
                let attrs = FieldAttr::parse(&f.attrs);
                if let Some(err) = attrs.error {
                    return Err(err);
                }
                if attrs.rename.is_some() {
                    return Err(syn::Error::new_spanned(
                        f,
                        "#[norito(rename = ...)] is only allowed on named fields",
                    ));
                }
                if attrs.skip && attrs.default {
                    return Err(syn::Error::new_spanned(
                        f,
                        "conflicting attributes: #[norito(skip)] and #[norito(default)]",
                    ));
                }
                if attrs.flatten {
                    return Err(syn::Error::new_spanned(
                        f,
                        "#[norito(flatten)] is only supported on named struct fields",
                    ));
                }
            }
        }
        Fields::Unit => {}
    }
    Ok(())
}

/// Extract a custom discriminant from `#[codec(index = ...)]`.
fn variant_index(variant: &Variant, default: usize) -> u32 {
    for attr in &variant.attrs {
        if attr.path().is_ident("codec") {
            let mut result = None;
            let _ = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("index") {
                    let lit: syn::LitInt = meta.value()?.parse()?;
                    result = Some(lit.base10_parse::<u32>()?);
                }
                Ok(())
            });
            if let Some(v) = result {
                return v;
            }
        }
    }
    default as u32
}

/// Parsed helper attributes for a field.
#[derive(Default, Clone)]
struct FieldAttr {
    /// Optional renamed identifier used during (de)serialization.
    #[allow(dead_code)]
    rename: Option<String>,
    /// Whether the field should be skipped entirely.
    skip: bool,
    /// Use [`Default::default`] or a provided function when the field is missing.
    default: bool,
    /// Optional function path to compute the default value.
    default_fn: Option<syn::Path>,
    /// Optional predicate to skip serialization when it returns true.
    skip_serializing_if: Option<syn::Path>,
    /// Optional helper module providing `serialize`/`deserialize` functions for the field.
    with: Option<syn::Path>,
    /// Whether the field should be flattened into the surrounding map (unsupported).
    flatten: bool,
    /// Force packed-struct layout to emit an explicit size header for this field.
    needs_size: bool,
    /// Deferred parsing error propagated during validation.
    error: Option<syn::Error>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RenameRule {
    Lowercase,
    Uppercase,
    SnakeCase,
    ScreamingSnakeCase,
    KebabCase,
    ScreamingKebabCase,
    CamelCase,
    PascalCase,
}

impl RenameRule {
    fn from_str(lit: &syn::LitStr) -> syn::Result<Self> {
        match lit.value().as_str() {
            "lowercase" => Ok(Self::Lowercase),
            "UPPERCASE" => Ok(Self::Uppercase),
            "snake_case" => Ok(Self::SnakeCase),
            "SCREAMING_SNAKE_CASE" => Ok(Self::ScreamingSnakeCase),
            "kebab-case" => Ok(Self::KebabCase),
            "SCREAMING-KEBAB-CASE" => Ok(Self::ScreamingKebabCase),
            "camelCase" => Ok(Self::CamelCase),
            "PascalCase" => Ok(Self::PascalCase),
            other => Err(syn::Error::new_spanned(
                lit,
                format!("unsupported rename_all value `{other}`"),
            )),
        }
    }

    fn apply(&self, ident: &str) -> String {
        match self {
            Self::Lowercase => ident.to_ascii_lowercase(),
            Self::Uppercase => ident.to_ascii_uppercase(),
            Self::SnakeCase => join_words(words(ident), '_', |w| w),
            Self::ScreamingSnakeCase => join_words(words(ident), '_', ascii_uppercase),
            Self::KebabCase => join_words(words(ident), '-', |w| w),
            Self::ScreamingKebabCase => join_words(words(ident), '-', ascii_uppercase),
            Self::CamelCase => camel_case(words(ident)),
            Self::PascalCase => pascal_case(words(ident)),
        }
    }
}

fn camel_case(words: Vec<String>) -> String {
    let mut iter = words.into_iter();
    let mut result = iter.next().unwrap_or_default();
    for word in iter {
        if let Some((first, rest)) = word.split_first_char() {
            result.push_str(&first.to_uppercase().collect::<String>());
            result.push_str(rest);
        } else {
            result.push_str(&word);
        }
    }
    result
}

fn pascal_case(words: Vec<String>) -> String {
    let mut out = String::new();
    for word in words {
        if let Some((first, rest)) = word.split_first_char() {
            out.push_str(&first.to_uppercase().collect::<String>());
            out.push_str(rest);
        } else {
            out.push_str(&word);
        }
    }
    out
}

fn join_words<F>(words: Vec<String>, separator: char, map: F) -> String
where
    F: FnMut(String) -> String,
{
    let mut iter = words.into_iter().map(map);
    let mut result = iter.next().unwrap_or_default();
    for word in iter {
        result.push(separator);
        result.push_str(&word);
    }
    result
}

fn ascii_uppercase(mut s: String) -> String {
    s.make_ascii_uppercase();
    s
}

trait SplitFirstChar {
    fn split_first_char(&self) -> Option<(char, &str)>;
}

impl SplitFirstChar for String {
    fn split_first_char(&self) -> Option<(char, &str)> {
        let mut chars = self.chars();
        let first = chars.next()?;
        Some((first, chars.as_str()))
    }
}

fn words(ident: &str) -> Vec<String> {
    use core::mem;

    let mut chars = ident.chars().peekable();
    let mut current = String::new();
    let mut result = Vec::new();
    let mut prev_is_lower = false;
    let mut prev_is_upper = false;
    let mut prev_is_digit = false;

    while let Some(ch) = chars.next() {
        if ch == '_' || ch == '-' {
            if !current.is_empty() {
                result.push(mem::take(&mut current));
            }
            prev_is_lower = false;
            prev_is_upper = false;
            prev_is_digit = false;
            continue;
        }

        let is_upper = ch.is_uppercase();
        let is_lower = ch.is_lowercase();
        let is_digit = ch.is_ascii_digit();
        let starts_new_word = is_upper
            && !current.is_empty()
            && (prev_is_lower
                || prev_is_digit
                || (prev_is_upper && chars.peek().is_some_and(|next| next.is_lowercase())));
        if starts_new_word {
            result.push(mem::take(&mut current));
        }

        if is_upper {
            current.extend(ch.to_lowercase());
        } else {
            current.push(ch);
        }

        prev_is_lower = is_lower;
        prev_is_upper = is_upper;
        prev_is_digit = is_digit;
    }

    if !current.is_empty() {
        result.push(current);
    }

    if result.is_empty() {
        result.push(String::new());
    }

    result
}

#[derive(Default)]
struct ContainerAttr {
    rename_all: Option<RenameRule>,
}

impl ContainerAttr {
    fn parse(attrs: &[Attribute]) -> syn::Result<Self> {
        let mut out = ContainerAttr::default();
        for attr in attrs {
            if !attr.path().is_ident("norito") {
                continue;
            }
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("rename_all") {
                    let lit: syn::LitStr = meta.value()?.parse()?;
                    let rule = RenameRule::from_str(&lit)?;
                    if out.rename_all.replace(rule).is_some() {
                        return Err(meta.error("duplicate rename_all attribute"));
                    }
                } else {
                    consume_unknown_meta(meta)?;
                }
                Ok(())
            })?;
        }
        Ok(out)
    }

    fn rename_field(&self, ident: &syn::Ident, attrs: &FieldAttr) -> String {
        if let Some(custom) = &attrs.rename {
            custom.clone()
        } else if let Some(rule) = self.rename_all {
            rule.apply(&ident.to_string())
        } else {
            ident.to_string()
        }
    }

    fn rename_variant(&self, ident: &syn::Ident, attrs: &VariantAttr) -> String {
        if let Some(custom) = &attrs.rename {
            custom.clone()
        } else if let Some(rule) = self.rename_all {
            rule.apply(&ident.to_string())
        } else {
            ident.to_string()
        }
    }
}

impl FieldAttr {
    /// Parse `#[norito(...)]` attributes from a field definition.
    fn parse(attrs: &[syn::Attribute]) -> Self {
        let mut out = FieldAttr::default();
        for attr in attrs {
            if attr.path().is_ident("norito") {
                let res = attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("rename") {
                        let lit: syn::LitStr = meta.value()?.parse()?;
                        out.rename = Some(lit.value());
                    } else if meta.path.is_ident("skip") {
                        out.skip = true;
                    } else if meta.path.is_ident("default") {
                        out.default = true;
                        if !meta.input.is_empty() {
                            let lit: syn::LitStr = meta.value()?.parse()?;
                            match syn::parse_str::<syn::Path>(&lit.value()) {
                                Ok(path) => out.default_fn = Some(path),
                                Err(err) => {
                                    out.error =
                                        Some(meta.error(format!(
                                            "invalid path `{}`: {err}",
                                            lit.value()
                                        )));
                                }
                            }
                        }
                    } else if meta.path.is_ident("skip_serializing_if") {
                        let lit: syn::LitStr = meta.value()?.parse()?;
                        match syn::parse_str::<syn::Path>(&lit.value()) {
                            Ok(path) => out.skip_serializing_if = Some(path),
                            Err(err) => {
                                out.error = Some(
                                    meta.error(format!("invalid path `{}`: {err}", lit.value())),
                                );
                            }
                        }
                    } else if meta.path.is_ident("with") {
                        let lit: syn::LitStr = meta.value()?.parse()?;
                        match syn::parse_str::<syn::Path>(&lit.value()) {
                            Ok(path) => {
                                if out.with.is_some() {
                                    out.error = Some(meta.error("duplicate `with` attribute"));
                                } else {
                                    out.with = Some(path);
                                }
                            }
                            Err(err) => {
                                out.error = Some(
                                    meta.error(format!("invalid path `{}`: {err}", lit.value())),
                                );
                            }
                        }
                    } else if meta.path.is_ident("flatten") {
                        out.flatten = true;
                    } else if meta.path.is_ident("needs_size") {
                        out.needs_size = true;
                    } else {
                        consume_unknown_meta(meta)?;
                    }
                    Ok(())
                });
                if let Err(err) = res {
                    out.error = Some(err);
                }
            }
        }
        out
    }
}

impl FieldAttr {
    fn require_json_serialize_bound(&self, generics: &mut Generics, ty: &syn::Type) {
        if self.with.is_none() {
            add_bound(generics, ty, quote!(norito::json::JsonSerialize));
        }
    }

    fn require_json_deserialize_bound(&self, generics: &mut Generics, ty: &syn::Type) {
        if self.with.is_none() {
            add_bound(generics, ty, quote!(norito::json::JsonDeserialize));
        }
    }

    fn serializer_call(&self, value: TokenStream2, out: TokenStream2) -> TokenStream2 {
        if let Some(path) = &self.with {
            quote! { #path::serialize(#value, #out); }
        } else {
            quote! { norito::json::JsonSerialize::json_serialize(#value, #out); }
        }
    }

    fn deserializer_call(&self, ty: &syn::Type, parser: TokenStream2) -> TokenStream2 {
        if let Some(path) = &self.with {
            quote! { #path::deserialize(#parser)? }
        } else {
            quote! { <#ty as norito::json::JsonDeserialize>::json_deserialize(#parser)? }
        }
    }

    fn deserialize_from_value(&self, ty: &syn::Type, value: TokenStream2) -> TokenStream2 {
        let call = self.deserializer_call(ty, quote!(&mut __parser));
        quote! {{
            let __json = norito::json::to_json(&#value)?;
            let mut __parser = norito::json::Parser::new(&__json);
            #call
        }}
    }
}

#[cfg(test)]
mod field_attr_tests {
    use super::*;

    #[test]
    fn needs_size_attribute_is_parsed() {
        let field: syn::Field = syn::parse_quote! {
            #[norito(needs_size)]
            demo: u32
        };
        let attrs = FieldAttr::parse(&field.attrs);
        assert!(attrs.needs_size);
    }
}

#[cfg(test)]
mod self_delimiting_tests {
    use super::*;

    #[test]
    fn allowlisted_types_are_self_delimiting() {
        let ok_types: Vec<syn::Type> = vec![
            syn::parse_quote!(String),
            syn::parse_quote!(Cow<'static, str>),
            syn::parse_quote!(ViewChangeProofPayload),
            syn::parse_quote!(Name),
            syn::parse_quote!(Metadata),
            syn::parse_quote!(ProofAttachment),
            syn::parse_quote!(VerifyingKeyId),
            syn::parse_quote!(ConstString),
            syn::parse_quote!(PhantomData<u8>),
            syn::parse_quote!(Vec<u8>),
            syn::parse_quote!(VecDeque<u8>),
            syn::parse_quote!(LinkedList<u8>),
            syn::parse_quote!(BinaryHeap<u8>),
            syn::parse_quote!(HashMap<String, u8>),
            syn::parse_quote!(BTreeMap<String, u8>),
            syn::parse_quote!(HashSet<u8>),
            syn::parse_quote!(BTreeSet<u8>),
            syn::parse_quote!(Option<u32>),
            syn::parse_quote!(Result<u8, u8>),
        ];

        for ty in ok_types {
            assert!(is_self_delimiting(&ty), "expected self-delimiting: {ty:?}");
        }
    }

    #[test]
    fn non_allowlisted_types_are_not_self_delimiting() {
        let bad_types: Vec<syn::Type> = vec![
            syn::parse_quote!(u32),
            syn::parse_quote!(Foo),
            syn::parse_quote!(ConstVec<u8>),
        ];

        for ty in bad_types {
            assert!(
                !is_self_delimiting(&ty),
                "unexpected self-delimiting: {ty:?}"
            );
        }
    }
}

#[derive(Default)]
struct EnumAttr {
    tag: Option<String>,
    content: Option<String>,
    error: Option<syn::Error>,
}

impl EnumAttr {
    fn parse(attrs: &[Attribute]) -> syn::Result<Self> {
        let mut out = EnumAttr::default();
        for attr in attrs {
            if !attr.path().is_ident("norito") {
                continue;
            }
            let res = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("tag") {
                    let lit: syn::LitStr = meta.value()?.parse()?;
                    if out.tag.is_some() {
                        return Err(meta.error("duplicate `tag` attribute"));
                    }
                    out.tag = Some(lit.value());
                } else if meta.path.is_ident("content") {
                    let lit: syn::LitStr = meta.value()?.parse()?;
                    if out.content.is_some() {
                        return Err(meta.error("duplicate `content` attribute"));
                    }
                    out.content = Some(lit.value());
                } else {
                    consume_unknown_meta(meta)?;
                }
                Ok(())
            });
            if let Err(err) = res {
                out.error = Some(err);
            }
        }
        if let Some(err) = out.error {
            Err(err)
        } else {
            Ok(out)
        }
    }
}

#[derive(Default)]
struct VariantAttr {
    rename: Option<String>,
}

impl VariantAttr {
    fn parse(attrs: &[Attribute]) -> syn::Result<Self> {
        let mut out = VariantAttr::default();
        for attr in attrs {
            if !attr.path().is_ident("norito") {
                continue;
            }
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("rename") {
                    let lit: syn::LitStr = meta.value()?.parse()?;
                    out.rename = Some(lit.value());
                } else {
                    consume_unknown_meta(meta)?;
                }
                Ok(())
            })?;
        }
        Ok(out)
    }
}

/// Generate `NoritoSerialize` implementation for a struct.
///
/// Each field is serialized in definition order and the resulting
/// implementation is bounded by that field's own `NoritoSerialize` trait.
fn derive_struct_serialize(
    ident: &syn::Ident,
    generics: &Generics,
    fields: &Fields,
    container_attrs: &[Attribute],
) -> TokenStream2 {
    let mut r#gen = generics.clone();
    let serialize_calls: Vec<_> = match fields {
        Fields::Named(named) => named
            .named
            .iter()
            .filter_map(|f| {
                let attrs = FieldAttr::parse(&f.attrs);
                if attrs.skip {
                    return None;
                }
                let name = f.ident.as_ref().unwrap();
                add_bound(&mut r#gen, &f.ty, quote!(norito::core::NoritoSerialize));
                // AoS fast path for [u8; N]: write raw N bytes inside an outer field length
                let is_u8_arr = matches!(&f.ty, syn::Type::Array(arr) if matches!(&*arr.elem, syn::Type::Path(tp) if tp.path.is_ident("u8")));
                if is_u8_arr {
                    Some(quote! {
                        let __len_bytes = core::mem::size_of_val(&self.#name);
                        #[cfg(feature = "compact-len")]
                        { norito::core::write_len(&mut writer, __len_bytes as u64)?; }
                        #[cfg(not(feature = "compact-len"))]
                        { writer.write_u64::<norito::core::LittleEndian>(__len_bytes as u64)?; }
                        writer.write_all(&self.#name)?;
                    })
                } else {
                    Some(quote! {
                        norito::core::write_len_prefixed(
                            &mut writer,
                            &self.#name,
                            &mut __norito_tmp,
                        )?;
                    })
                }
            })
            .collect(),
        Fields::Unnamed(unnamed) => unnamed
            .unnamed
            .iter()
            .enumerate()
            .filter_map(|(i, f)| {
                let attrs = FieldAttr::parse(&f.attrs);
                if attrs.skip {
                    return None;
                }
                let idx = Index::from(i);
                add_bound(&mut r#gen, &f.ty, quote!(norito::core::NoritoSerialize));
                // AoS fast path for [u8; N] unnamed tuple fields
                let is_u8_arr = matches!(&f.ty, syn::Type::Array(arr) if matches!(&*arr.elem, syn::Type::Path(tp) if tp.path.is_ident("u8")));
                if is_u8_arr {
                    Some(quote! {
                        let __len_bytes = core::mem::size_of_val(&self.#idx);
                        #[cfg(feature = "compact-len")]
                        { norito::core::write_len(&mut writer, __len_bytes as u64)?; }
                        #[cfg(not(feature = "compact-len"))]
                        { writer.write_u64::<norito::core::LittleEndian>(__len_bytes as u64)?; }
                        writer.write_all(&self.#idx)?;
                    })
                } else {
                    Some(quote! {
                        norito::core::write_len_prefixed(
                            &mut writer,
                            &self.#idx,
                            &mut __norito_tmp,
                        )?;
                    })
                }
            })
            .collect(),
        Fields::Unit => Vec::new(),
    };

    // Field expressions for packed-struct path and specialized serializers
    let mut packed_field_exprs: Vec<TokenStream2> = Vec::new();
    let mut packed_field_ser_calls: Vec<TokenStream2> = Vec::new();
    let mut packed_field_stage_stmts: Vec<TokenStream2> = Vec::new();
    match fields {
        Fields::Named(named) => {
            for f in &named.named {
                let attrs = FieldAttr::parse(&f.attrs);
                if attrs.skip {
                    continue;
                }
                let name = f.ident.as_ref().unwrap();
                let ty = &f.ty;
                let is_u8_array = matches!(&f.ty,
                    syn::Type::Array(arr) if matches!(&*arr.elem, syn::Type::Path(tp) if tp.path.is_ident("u8"))
                );
                packed_field_exprs.push(quote! { &self.#name });
                if is_u8_array {
                    packed_field_ser_calls.push(quote! { writer.write_all(&self.#name)?; });
                    packed_field_stage_stmts
                        .push(quote! { __field_bufs.push((&self.#name).to_vec()); });
                } else if is_self_delimiting(&f.ty) {
                    // Self-delimiting fields already carry their own length header inside
                    // their serialized payload. In the hybrid packed-struct layout we must
                    // NOT add an extra prefix here; the decoder expects the field's own
                    // header at the start of the field data.
                    packed_field_ser_calls.push(quote! {
                        norito::core::NoritoSerialize::serialize(&self.#name, &mut writer)?;
                    });
                    packed_field_stage_stmts.push(quote! {
                        let mut __b: ::std::vec::Vec<u8> = ::std::vec::Vec::new();
                        norito::core::NoritoSerialize::serialize(&self.#name, &mut __b)?;
                        #[cfg(debug_assertions)]
                        if norito::debug_trace_enabled() {
                            eprintln!(
                                "stage {}::{} packed bytes len={} ty={}",
                                stringify!(#ident),
                                stringify!(#name),
                                __b.len(),
                                core::any::type_name::<#ty>(),
                            );
                        }
                        __field_bufs.push(__b);
                    });
                } else {
                    // Signature-like proofs and other variable-length fields share the staging path to
                    // compute accurate offsets before writing the packed payload.
                    packed_field_ser_calls.push(quote! { norito::core::NoritoSerialize::serialize(&self.#name, &mut writer)?; });
                    packed_field_stage_stmts.push(quote! {
                        let mut __b: ::std::vec::Vec<u8> = ::std::vec::Vec::new();
                        norito::core::NoritoSerialize::serialize(&self.#name, &mut __b)?;
                        #[cfg(debug_assertions)]
                        if norito::debug_trace_enabled() {
                            eprintln!(
                                "stage {}::{} packed bytes len={} ty={}",
                                stringify!(#ident),
                                stringify!(#name),
                                __b.len(),
                                core::any::type_name::<#ty>(),
                            );
                        }
                        __field_bufs.push(__b);
                    });
                }
            }
        }
        Fields::Unnamed(unnamed) => {
            for (i, f) in unnamed.unnamed.iter().enumerate() {
                let attrs = FieldAttr::parse(&f.attrs);
                if attrs.skip {
                    continue;
                }
                let idx = Index::from(i);
                let ty = &f.ty;
                let is_u8_array = matches!(&f.ty,
                    syn::Type::Array(arr) if matches!(&*arr.elem, syn::Type::Path(tp) if tp.path.is_ident("u8"))
                );
                packed_field_exprs.push(quote! { &self.#idx });
                if is_u8_array {
                    packed_field_ser_calls.push(quote! { writer.write_all(&self.#idx)?; });
                    packed_field_stage_stmts
                        .push(quote! { __field_bufs.push((&self.#idx).to_vec()); });
                } else if is_self_delimiting(&f.ty) {
                    // Do not add an extra prefix for self-delimiting fields; write only the
                    // field's serialized bytes.
                    packed_field_ser_calls.push(quote! {
                        norito::core::NoritoSerialize::serialize(&self.#idx, &mut writer)?;
                    });
                    packed_field_stage_stmts.push(quote! {
                        let mut __b: ::std::vec::Vec<u8> = ::std::vec::Vec::new();
                        norito::core::NoritoSerialize::serialize(&self.#idx, &mut __b)?;
                        #[cfg(debug_assertions)]
                        if norito::debug_trace_enabled() {
                            eprintln!(
                                "stage {}::{} packed bytes len={} ty={}",
                                stringify!(#ident),
                                stringify!(#idx),
                                __b.len(),
                                core::any::type_name::<#ty>(),
                            );
                        }
                        __field_bufs.push(__b);
                    });
                } else {
                    packed_field_ser_calls.push(quote! { norito::core::NoritoSerialize::serialize(&self.#idx, &mut writer)?; });
                    packed_field_stage_stmts.push(quote! {
                        let mut __b: ::std::vec::Vec<u8> = ::std::vec::Vec::new();
                        norito::core::NoritoSerialize::serialize(&self.#idx, &mut __b)?;
                        #[cfg(debug_assertions)]
                        if norito::debug_trace_enabled() {
                            eprintln!(
                                "stage {}::{} packed bytes len={} ty={}",
                                stringify!(#ident),
                                stringify!(#idx),
                                __b.len(),
                                core::any::type_name::<#ty>(),
                            );
                        }
                        __field_bufs.push(__b);
                    });
                }
            }
        }
        Fields::Unit => {}
    }
    let packed_field_count: usize = packed_field_exprs.len();

    // Classification helpers for packed-struct hybrid layout
    fn is_fixed_size(ty: &syn::Type) -> Option<usize> {
        match ty {
            syn::Type::Path(tp) => {
                let id = tp
                    .path
                    .segments
                    .last()
                    .map(|s| s.ident.to_string())
                    .unwrap_or_default();
                match id.as_str() {
                    "u8" | "i8" | "bool" => Some(1),
                    "u16" | "i16" => Some(2),
                    "u32" | "i32" | "f32" => Some(4),
                    "u64" | "i64" | "f64" | "usize" | "isize" => Some(8),
                    "u128" | "i128" => Some(16),
                    "NonZeroU16" => Some(2),
                    "NonZeroU32" => Some(4),
                    "NonZeroU64" => Some(8),
                    "ContractCodeHash" | "ContractAbiHash" | "ProposalId" => Some(36),
                    // Common small enums in Connect wire types; encoded as 4-byte tag
                    "Dir" | "Role" => Some(4),
                    _ => None,
                }
            }
            syn::Type::Array(arr) => {
                if let syn::Type::Path(tp) = &*arr.elem
                    && tp.path.is_ident("u8")
                {
                    // [u8; N] is fixed-size N bytes
                    // We can't compute to usize here easily; treat as fixed-size marker
                    return Some(0); // marker; handled specially downstream
                }
                None
            }
            _ => None,
        }
    }

    fn is_self_delimiting(ty: &syn::Type) -> bool {
        match ty {
            syn::Type::Path(tp) => {
                let id = tp
                    .path
                    .segments
                    .last()
                    .map(|s| s.ident.to_string())
                    .unwrap_or_default();
                // Keep in sync with the top-level is_self_delimiting allowlist.
                if matches!(id.as_str(), "String" | "Cow" | "ViewChangeProofPayload") {
                    return true;
                }
                if matches!(
                    id.as_str(),
                    "Name"
                        | "Metadata"
                        | "ProofAttachment"
                        | "VerifyingKeyId"
                        | "ConstString"
                        | "PhantomData"
                ) {
                    return true;
                }
                if matches!(
                    id.as_str(),
                    "Vec"
                        | "VecDeque"
                        | "LinkedList"
                        | "BinaryHeap"
                        | "HashMap"
                        | "BTreeMap"
                        | "HashSet"
                        | "BTreeSet"
                        | "Option"
                        | "Result"
                ) {
                    return true;
                }
                false
            }
            _ => false,
        }
    }

    // Build encoded_len_hint body: sum 8-byte prefixes plus field hints.
    let len_hint_body: TokenStream2 = match fields {
        Fields::Named(named) => {
            let mut parts: Vec<TokenStream2> = Vec::new();
            for f in &named.named {
                let attrs = FieldAttr::parse(&f.attrs);
                if attrs.skip {
                    continue;
                }
                let name = f.ident.as_ref().unwrap();
                parts.push(quote! {
                    let __h = norito::core::NoritoSerialize::encoded_len_hint(&self.#name)?;
                    __sum = __sum.saturating_add(8 + __h);
                });
            }
            quote! { #(#parts)* }
        }
        Fields::Unnamed(unnamed) => {
            let mut parts: Vec<TokenStream2> = Vec::new();
            for (i, f) in unnamed.unnamed.iter().enumerate() {
                let attrs = FieldAttr::parse(&f.attrs);
                if attrs.skip {
                    continue;
                }
                let idx = Index::from(i);
                parts.push(quote! {
                    let __h = norito::core::NoritoSerialize::encoded_len_hint(&self.#idx)?;
                    __sum = __sum.saturating_add(8 + __h);
                });
            }
            quote! { #(#parts)* }
        }
        Fields::Unit => quote! {},
    };

    // Build encoded_len_exact body depending on layout:
    // - packed-struct hybrid: bitset bytes + sum(exact) for self-delim or fixed +
    //   sum(prefix_len(exact)+exact) for needs-size fields
    // - compat: sum(prefix_len + exact) per field (compact lengths when enabled)
    let len_exact_body: TokenStream2 = match fields {
        Fields::Named(named) => {
            let mut parts: Vec<TokenStream2> = Vec::new();
            let mut count = 0usize;
            for f in &named.named {
                let attrs = FieldAttr::parse(&f.attrs);
                if attrs.skip {
                    continue;
                }
                count += 1;
                let name = f.ident.as_ref().unwrap();
                let needs_size = !(is_self_delimiting(&f.ty) || is_fixed_size(&f.ty).is_some());
                let part = if needs_size {
                    quote! {
                        let __e = norito::core::NoritoSerialize::encoded_len_exact(&self.#name)?;
                        let __prefix_len = norito::core::len_prefix_len(__e);
                        __sum = __sum.saturating_add(__prefix_len + __e);
                    }
                } else {
                    quote! {
                        let __e = norito::core::NoritoSerialize::encoded_len_exact(&self.#name)?;
                        #[cfg(feature = "packed-struct")]
                        { __sum = __sum.saturating_add(__e); }
                        #[cfg(not(feature = "packed-struct"))]
                        { __sum = __sum.saturating_add(norito::core::len_prefix_len(__e) + __e); }
                    }
                };
                parts.push(part);
            }
            let bitset_len: usize = count.div_ceil(8).max(1);
            quote! {
                #[cfg(feature = "packed-struct")]
                { __sum = __sum.saturating_add(#bitset_len); }
                #(#parts)*
            }
        }
        Fields::Unnamed(unnamed) => {
            let mut parts: Vec<TokenStream2> = Vec::new();
            let mut count = 0usize;
            for (i, f) in unnamed.unnamed.iter().enumerate() {
                let attrs = FieldAttr::parse(&f.attrs);
                if attrs.skip {
                    continue;
                }
                count += 1;
                let idx = Index::from(i);
                let needs_size = !(is_self_delimiting(&f.ty) || is_fixed_size(&f.ty).is_some());
                let part = if needs_size {
                    quote! {
                        let __e = norito::core::NoritoSerialize::encoded_len_exact(&self.#idx)?;
                        let __prefix_len = norito::core::len_prefix_len(__e);
                        __sum = __sum.saturating_add(__prefix_len + __e);
                    }
                } else {
                    quote! {
                        let __e = norito::core::NoritoSerialize::encoded_len_exact(&self.#idx)?;
                        #[cfg(feature = "packed-struct")]
                        { __sum = __sum.saturating_add(__e); }
                        #[cfg(not(feature = "packed-struct"))]
                        { __sum = __sum.saturating_add(norito::core::len_prefix_len(__e) + __e); }
                    }
                };
                parts.push(part);
            }
            let bitset_len: usize = count.div_ceil(8).max(1);
            quote! {
                #[cfg(feature = "packed-struct")]
                { __sum = __sum.saturating_add(#bitset_len); }
                #(#parts)*
            }
        }
        Fields::Unit => quote! {},
    };

    let archived = format_ident!("Archived{}", ident);
    let params = generics
        .params
        .iter()
        .map(|param| match param {
            syn::GenericParam::Type(ty) => {
                let ident = &ty.ident;
                quote! { #ident }
            }
            syn::GenericParam::Lifetime(lt) => {
                let lt = &lt.lifetime;
                quote! { #lt }
            }
            syn::GenericParam::Const(c) => {
                let ident = &c.ident;
                quote! { #ident }
            }
        })
        .collect::<Vec<_>>();
    let alias_generics = if params.is_empty() {
        quote! {}
    } else {
        quote! { < #( #params ),* > }
    };
    match fields {
        Fields::Named(named) => {
            for f in &named.named {
                let attrs = FieldAttr::parse(&f.attrs);
                if attrs.skip {
                    continue;
                }
                if is_signature_like(&f.ty) || is_staged_wrapper(&f.ty) {
                    add_bound(&mut r#gen, &f.ty, quote!(norito::codec::Decode));
                }
            }
        }
        Fields::Unnamed(unnamed) => {
            for f in &unnamed.unnamed {
                let attrs = FieldAttr::parse(&f.attrs);
                if attrs.skip {
                    continue;
                }
                if is_signature_like(&f.ty) || is_staged_wrapper(&f.ty) {
                    add_bound(&mut r#gen, &f.ty, quote!(norito::codec::Decode));
                }
            }
        }
        Fields::Unit => {}
    }
    let (impl_generics, ty_generics, where_clause) = r#gen.split_for_impl();
    let has_signature_like_field: bool = match fields {
        Fields::Named(named) => named
            .named
            .iter()
            .filter(|f| {
                let attrs = FieldAttr::parse(&f.attrs);
                !(attrs.skip)
            })
            .any(|f| is_signature_like(&f.ty)),
        Fields::Unnamed(unnamed) => unnamed
            .unnamed
            .iter()
            .filter(|f| {
                let attrs = FieldAttr::parse(&f.attrs);
                !(attrs.skip)
            })
            .any(|f| is_signature_like(&f.ty)),
        Fields::Unit => false,
    };
    let field_bitset_enabled = if has_signature_like_field {
        quote! { false }
    } else {
        quote! { norito::core::use_field_bitset() }
    };
    let field_bitset_enabled_encode = field_bitset_enabled;
    // Build bitset and size write tokens for packed-struct hybrid
    let (bitset_bytes, write_sizes_code, all_needs_false) = {
        let mut needs: ::std::vec::Vec<bool> = ::std::vec::Vec::new();
        match fields {
            Fields::Named(named) => {
                for f in &named.named {
                    let attrs = FieldAttr::parse(&f.attrs);
                    if attrs.skip {
                        continue;
                    }
                    let need = if attrs.needs_size {
                        true
                    } else {
                        is_staged_wrapper(&f.ty)
                            || !(is_self_delimiting(&f.ty) || is_fixed_size(&f.ty).is_some())
                    };
                    needs.push(need);
                }
            }
            Fields::Unnamed(unnamed) => {
                for f in &unnamed.unnamed {
                    let attrs = FieldAttr::parse(&f.attrs);
                    if attrs.skip {
                        continue;
                    }
                    let need = if attrs.needs_size {
                        true
                    } else {
                        is_staged_wrapper(&f.ty)
                            || !(is_self_delimiting(&f.ty) || is_fixed_size(&f.ty).is_some())
                    };
                    needs.push(need);
                }
            }
            Fields::Unit => {}
        }
        let mut bytes: ::std::vec::Vec<u8> = ::std::vec::Vec::new();
        let mut cur: u8 = 0;
        let mut bit: u8 = 0;
        for &need in needs.iter() {
            if need {
                cur |= 1u8 << bit;
            }
            bit += 1;
            if bit == 8 {
                bytes.push(cur);
                cur = 0;
                bit = 0;
            }
        }
        if bit != 0 {
            bytes.push(cur);
        }
        let bitset_lit = quote! { [ #( #bytes ),* ] };
        // write sizes code: coalesce varint sizes into a single buffer
        let mut stmts: ::std::vec::Vec<TokenStream2> = ::std::vec::Vec::new();
        // Build: let mut __sizes_hdr = Vec::with_capacity(needs_count * 2);
        let needs_count = needs.iter().filter(|&&n| n).count();
        stmts.push(quote! { let mut __sizes_hdr: ::std::vec::Vec<u8> = ::std::vec::Vec::with_capacity(#needs_count * 2); });
        for (i, need) in needs.into_iter().enumerate() {
            if need {
                stmts.push(quote! {
                    let __field_len = __field_bufs[#i].len();
                    norito::core::write_len_header_to_vec(&mut __sizes_hdr, __field_len as u64);
                });
            }
        }
        // Write once
        stmts.push(quote! { writer.write_all(&__sizes_hdr)?; });
        let sizes_code = quote! { #(#stmts)* };
        let all_needs_false = bytes.is_empty() || bytes.iter().all(|b| *b == 0);
        (bitset_lit, sizes_code, all_needs_false)
    };

    let alias_decl = if reuse_archived_alias(container_attrs) {
        quote! {}
    } else {
        let archived_doc = format!(
            "Archived Norito representation of `{}` generated by `#[derive(NoritoSerialize)]`.",
            ident
        );
        quote! {
            #[doc = #archived_doc]
            pub type #archived #alias_generics = norito::core::Archived<#ident #ty_generics>;
        }
    };
    quote! {
        #alias_decl
        impl #impl_generics norito::core::NoritoSerialize for #ident #ty_generics #where_clause {
            #[inline]
            fn schema_hash() -> [u8; 16] {
                #[cfg(feature = "schema-structural")]
                { norito::core::schema_hash_structural::<Self>() }
                #[cfg(not(feature = "schema-structural"))]
                { norito::core::type_name_schema_hash::<Self>() }
            }
            #[inline]
            fn encoded_len_hint(&self) -> Option<usize> {
                let mut __sum: usize = 0;
                #len_hint_body
                Some(__sum)
            }
            #[inline]
            fn encoded_len_exact(&self) -> Option<usize> {
                let mut __sum: usize = 0;
                #len_exact_body
                Some(__sum)
            }
            fn serialize<W: std::io::Write>(&self, mut writer: W) -> ::core::result::Result<(), norito::core::Error> {
                use norito::core::WriteBytesExt;
                if norito::core::use_packed_struct() {
                    if #field_bitset_enabled_encode {
                        norito::core::mark_field_bitset_used_if_encoding();
                        // Hybrid packed-struct: write bitset + optional staged sizes before payload.
                        if #all_needs_false {
                            writer.write_all(&#bitset_bytes)?;
                            // No sizes to emit
                            #( #packed_field_ser_calls )*
                            Ok(())
                        } else {
                            // Stage field payloads to compute sizes, then flush.
                            let mut __field_bufs: ::std::vec::Vec<::std::vec::Vec<u8>> = ::std::vec::Vec::with_capacity(#packed_field_count);
                            #(
                                { #packed_field_stage_stmts }
                            )*
                            writer.write_all(&#bitset_bytes)?;
                            {
                                #write_sizes_code
                            }
                            for __b in __field_bufs { writer.write_all(&__b)?; }
                            Ok(())
                        }
                    } else {
                        // Compat packed-struct: emit per-field lengths (or offsets) followed by payload data.
                        let mut __field_bufs: ::std::vec::Vec<::std::vec::Vec<u8>> = ::std::vec::Vec::with_capacity(#packed_field_count);
                        #(
                            { #packed_field_stage_stmts }
                        )*
                        let mut __acc: usize = 0;
                        writer.write_u64::<norito::core::LittleEndian>(0)?;
                        for __buf in &__field_bufs {
                            __acc = __acc.saturating_add(__buf.len());
                            writer.write_u64::<norito::core::LittleEndian>(__acc as u64)?;
                        }
                        for __buf in __field_bufs { writer.write_all(&__buf)?; }
                        Ok(())
                    }
                } else {
                    // Single-pass per-field into stack-backed buffer to avoid extra
                    // allocations and a second encode pass.
                    let mut __norito_tmp: norito::core::DeriveSmallBuf = norito::core::DeriveSmallBuf::new();
                    #(#serialize_calls)*
                    Ok(())
                }
            }
        }
    }
}

/// Generate `NoritoDeserialize` implementation for a struct.
///
/// The produced code casts the archived bytes back to `Self` and
/// recursively calls `NoritoDeserialize` on each field.
fn derive_struct_deserialize(
    ident: &syn::Ident,
    generics: &Generics,
    fields: &Fields,
    container_attrs: &[Attribute],
) -> TokenStream2 {
    let mut r#gen = generics.clone();
    // Helper: detect [u8; N] arrays for a fast path
    fn u8_array_len(ty: &syn::Type) -> Option<syn::Expr> {
        if let syn::Type::Array(arr) = ty
            && let syn::Type::Path(tp) = &*arr.elem
            && tp.path.is_ident("u8")
        {
            return Some(arr.len.clone());
        }
        None
    }
    let deserialize_fields: Vec<_> = match fields {
        Fields::Named(named) => named
            .named
            .iter()
            .map(|f| {
                let attrs = FieldAttr::parse(&f.attrs);
                let name = f.ident.as_ref().unwrap();
                let ty = &f.ty;
                if attrs.skip {
                    add_bound(&mut r#gen, ty, quote!(Default));
                    return quote! { #name: Default::default() };
                }

                let fallback_expr = if let Some(path) = attrs.default_fn.as_ref() {
                    Some(quote! { (#path)() })
                } else if attrs.default {
                    add_bound(&mut r#gen, ty, quote!(Default));
                    Some(quote! { Default::default() })
                } else {
                    None
                };

                if let Some(len_expr) = u8_array_len(ty) {
                    let decode_expr = quote! {
                        (|| -> ::core::result::Result<_, norito::core::Error> {
                            let (base, total) = norito::core::payload_ctx().ok_or(norito::core::Error::MissingPayloadContext)?;
                            let start = (ptr as usize).saturating_sub(base);
                            let payload = unsafe { std::slice::from_raw_parts(base as *const u8, total) };
                            if start + offset >= payload.len() {
                                return Err(norito::core::Error::LengthMismatch);
                            }
                            let (field_len, hdr) =
                                norito::core::read_len_dyn_slice(&payload[start + offset..])?;
                            let expected_len: usize = #len_expr;
                            if field_len != expected_len {
                                return Err(norito::core::Error::LengthMismatch);
                            }
                            let data_start = start + offset + hdr;
                            let data_end = data_start
                                .checked_add(field_len)
                                .ok_or(norito::core::Error::LengthMismatch)?;
                            if data_end > payload.len() {
                                return Err(norito::core::Error::LengthMismatch);
                            }
                            let mut value = [0u8; #len_expr];
                            value.copy_from_slice(&payload[data_start..data_end]);
                            offset += hdr + field_len;
                            Ok(value)
                        })()
                    };
                    if let Some(fallback) = fallback_expr {
                        quote! {
                            #name: {
                                match #decode_expr {
                                    Ok(value) => value,
                                    Err(norito::core::Error::LengthMismatch) => { #fallback }
                                    Err(err) => return Err(err),
                                }
                            }
                        }
                    } else {
                        quote! {
                            #name: {
                                match #decode_expr {
                                    Ok(value) => value,
                                    Err(err) => return Err(err),
                                }
                            }
                        }
                    }
                } else {
                    add_bound(&mut r#gen, ty, quote!(for<'__d> norito::core::NoritoDeserialize<'__d>));
                    add_bound(&mut r#gen, ty, quote!(norito::core::NoritoSerialize));
                    let decode_expr = quote! {
                        (|| -> ::core::result::Result<#ty, norito::core::Error> {
                            let (base, total) = norito::core::payload_ctx().ok_or(norito::core::Error::MissingPayloadContext)?;
                            let start = (ptr as usize).saturating_sub(base);
                            let payload = unsafe { std::slice::from_raw_parts(base as *const u8, total) };
                            if start + offset >= payload.len() {
                                return Err(norito::core::Error::LengthMismatch);
                            }
                            let (field_len, hdr) =
                                norito::core::read_len_dyn_slice(&payload[start + offset..])?;
                            let data_start = offset
                                .checked_add(hdr)
                                .and_then(|s| start.checked_add(s))
                                .ok_or(norito::core::Error::LengthMismatch)?;
                            let data_end = data_start
                                .checked_add(field_len)
                                .ok_or(norito::core::Error::LengthMismatch)?;
                            let field_data = payload
                                .get(data_start..data_end)
                                .ok_or(norito::core::Error::LengthMismatch)?;
                            let (value, consumed) =
                                norito::core::decode_field_canonical::<#ty>(field_data)?;
                            #[cfg(feature = "compact-len")]
                            {
                                offset += hdr + consumed;
                            }
                            #[cfg(not(feature = "compact-len"))]
                            {
                                offset += hdr + consumed;
                            }
                            Ok(value)
                        })()
                    };
                    if let Some(fallback) = fallback_expr {
                        quote! {
                            #name: {
                                match #decode_expr {
                                    Ok(value) => value,
                                    Err(norito::core::Error::LengthMismatch) => { #fallback }
                                    Err(err) => return Err(err),
                                }
                            }
                        }
                    } else {
                        quote! {
                            #name: {
                                match #decode_expr {
                                    Ok(value) => value,
                                    Err(err) => return Err(err),
                                }
                            }
                        }
                    }
                }
            })
            .collect(),
        Fields::Unnamed(unnamed) => {
            unnamed
                .unnamed
                .iter()
                .enumerate()
                .map(|(i, f)| {
                    let attrs = FieldAttr::parse(&f.attrs);
                    let idx_var = format_ident!("field{}", i);
                    let ty = &f.ty;
                    if attrs.skip {
                        add_bound(&mut r#gen, ty, quote!(Default));
                        return quote! {
                            let #idx_var = Default::default();
                        };
                    }

                    let fallback_expr = if let Some(path) = attrs.default_fn.as_ref() {
                        Some(quote! { (#path)() })
                    } else if attrs.default {
                        add_bound(&mut r#gen, ty, quote!(Default));
                        Some(quote! { Default::default() })
                    } else {
                        None
                    };

                    if let Some(len_expr) = u8_array_len(ty) {
                        let decode_expr = quote! {
                            (|| -> ::core::result::Result<_, norito::core::Error> {
                                let (base, total) = if let Some(ctx) = norito::core::payload_ctx() {
                                    ctx
                                } else {
                                    return Err(norito::core::Error::MissingPayloadContext);
                                };
                                let start = (ptr as usize).saturating_sub(base);
                                let payload = unsafe { std::slice::from_raw_parts(base as *const u8, total) };
                                if start + offset >= payload.len() {
                                    return Err(norito::core::Error::LengthMismatch);
                                }
                                let (field_len, hdr) =
                                    norito::core::read_len_dyn_slice(&payload[start + offset..])?;
                                let expected_len: usize = #len_expr;
                                if field_len != expected_len {
                                    return Err(norito::core::Error::LengthMismatch);
                                }
                                let data_start = start + offset + hdr;
                                let data_end = data_start
                                    .checked_add(field_len)
                                    .ok_or(norito::core::Error::LengthMismatch)?;
                                if data_end > payload.len() {
                                    return Err(norito::core::Error::LengthMismatch);
                                }
                                let mut value = [0u8; #len_expr];
                                value.copy_from_slice(&payload[data_start..data_end]);
                                offset += hdr + field_len;
                                Ok(value)
                            })()
                        };
                        if let Some(fallback) = fallback_expr {
                            quote! {
                                let #idx_var = match #decode_expr {
                                    Ok(value) => value,
                                    Err(norito::core::Error::LengthMismatch) => { #fallback },
                                    Err(err) => return Err(err),
                                };
                            }
                        } else {
                            quote! {
                                let #idx_var = match #decode_expr {
                                    Ok(value) => value,
                                    Err(err) => return Err(err),
                                };
                            }
                        }
                    } else {
                        add_bound(&mut r#gen, ty, quote!(for<'__d> norito::core::NoritoDeserialize<'__d>));
                        add_bound(&mut r#gen, ty, quote!(norito::core::NoritoSerialize));
                        let decode_expr = quote! {
                            (|| -> ::core::result::Result<#ty, norito::core::Error> {
                                let (base, total) = if let Some(ctx) = norito::core::payload_ctx() {
                                    ctx
                                } else {
                                    return Err(norito::core::Error::MissingPayloadContext);
                                };
                                let start = (ptr as usize).saturating_sub(base);
                                let payload = unsafe { std::slice::from_raw_parts(base as *const u8, total) };
                                if start + offset >= payload.len() {
                                    return Err(norito::core::Error::LengthMismatch);
                                }
                                let (field_len, hdr) =
                                    norito::core::read_len_dyn_slice(&payload[start + offset..])?;
                                let data_start = offset
                                    .checked_add(hdr)
                                    .and_then(|s| start.checked_add(s))
                                    .ok_or(norito::core::Error::LengthMismatch)?;
                                let data_end = data_start
                                    .checked_add(field_len)
                                    .ok_or(norito::core::Error::LengthMismatch)?;
                                let field_data = payload
                                    .get(data_start..data_end)
                                    .ok_or(norito::core::Error::LengthMismatch)?;
                                let (value, consumed) =
                                    norito::core::decode_field_canonical::<#ty>(field_data)?;
                                #[cfg(feature = "compact-len")]
                                {
                                    offset += hdr + consumed;
                                }
                                #[cfg(not(feature = "compact-len"))]
                                {
                                    offset += hdr + consumed;
                                }
                                Ok(value)
                            })()
                        };
                        if let Some(fallback) = fallback_expr {
                            quote! {
                                let #idx_var = match #decode_expr {
                                    Ok(value) => value,
                                    Err(norito::core::Error::LengthMismatch) => { #fallback },
                                    Err(err) => return Err(err),
                                };
                            }
                        } else {
                            quote! {
                                let #idx_var = match #decode_expr {
                                    Ok(value) => value,
                                    Err(err) => return Err(err),
                                };
                            }
                        }
                    }
                })
                .collect()
        }
        Fields::Unit => Vec::new(),
    };

    let mut impl_gen = r#gen.clone();
    impl_gen.params.insert(0, syn::parse_quote!('de));
    let (impl_generics, _, where_clause) = impl_gen.split_for_impl();
    let (_, ty_generics, _) = r#gen.split_for_impl();

    let has_signature_like_field: bool = match fields {
        Fields::Named(named) => named
            .named
            .iter()
            .filter(|f| {
                let attrs = FieldAttr::parse(&f.attrs);
                !(attrs.skip)
            })
            .any(|f| is_signature_like(&f.ty)),
        Fields::Unnamed(unnamed) => unnamed
            .unnamed
            .iter()
            .filter(|f| {
                let attrs = FieldAttr::parse(&f.attrs);
                !(attrs.skip)
            })
            .any(|f| is_signature_like(&f.ty)),
        Fields::Unit => false,
    };
    let field_bitset_enabled_decode = if has_signature_like_field {
        quote! { false }
    } else {
        quote! { norito::core::use_field_bitset() }
    };
    let field_bitset_enabled_decode_named = field_bitset_enabled_decode.clone();
    let field_bitset_enabled_decode_unnamed = field_bitset_enabled_decode;

    match fields {
        Fields::Named(_) => {
            // Count non-skipped fields used in packed-struct layout
            let packed_named_count: usize = match fields {
                Fields::Named(named) => named
                    .named
                    .iter()
                    .filter(|f| {
                        let a = FieldAttr::parse(&f.attrs);
                        !a.skip
                    })
                    .count(),
                _ => 0,
            };
            // Build packed-struct named field initializers (compat offsets-based)
            let packed_named_inits: Vec<TokenStream2> = match fields {
                Fields::Named(named) => named
                    .named
                    .iter()
                    .map(|f| {
                        let attrs = FieldAttr::parse(&f.attrs);
                        let name = f.ident.as_ref().unwrap();
                        let ty = &f.ty;
                        if attrs.skip {
                            quote! { #name: Default::default() }
                        } else if let Some(path) = attrs.default_fn.as_ref() {
                            quote! { #name: (#path)() }
                        } else if attrs.default {
                            quote! { #name: Default::default() }
                        } else {
                            quote! {
                                #name: {
                                    let __start = __offs[__i];
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
                                    unsafe {
                                        let field_data = std::slice::from_raw_parts(data_base.add(__start), __len);
                                        let (v, consumed) = norito::core::decode_field_canonical::<#ty>(field_data)?;
                                        debug_assert_eq!(consumed, __len);
                                        v
                                    }
                                }
                            }
                        }
                    })
                    .collect(),
                _ => Vec::new(),
            };
            // Precompute bit positions for non-skipped fields (named)
            let named_bit_positions: Vec<Option<usize>> = match fields {
                Fields::Named(named) => {
                    let mut pos = 0usize;
                    named
                        .named
                        .iter()
                        .map(|f| {
                            let attrs = FieldAttr::parse(&f.attrs);
                            if attrs.skip {
                                None
                            } else {
                                let p = pos;
                                pos += 1;
                                Some(p)
                            }
                        })
                        .collect()
                }
                _ => Vec::new(),
            };
            // Determine if any named field needs an explicit size (non self-delimiting, non fixed, not skipped/default)
            let _named_any_needs_size: bool = match fields {
                Fields::Named(named) => {
                    let mut any = false;
                    for f in &named.named {
                        let attrs = FieldAttr::parse(&f.attrs);
                        if attrs.skip {
                            continue;
                        }
                        let need = if attrs.needs_size {
                            true
                        } else {
                            is_staged_wrapper(&f.ty)
                                || !(is_self_delimiting(&f.ty) || is_fixed_size(&f.ty).is_some())
                        };
                        if need {
                            any = true;
                            break;
                        }
                    }
                    any
                }
                _ => false,
            };
            // Build packed-struct named field initializers (hybrid bitset-based sequential decode)
            let packed_named_inits_hybrid: Vec<TokenStream2> = match fields {
                Fields::Named(named) => named
                    .named
                    .iter()
                    .enumerate()
                    .map(|(i, f)| {
                        let attrs = FieldAttr::parse(&f.attrs);
                        let name = f.ident.as_ref().unwrap();
                        let ty = &f.ty;
                        let fixed_size = is_fixed_size(ty);
                        let compat_decode_named = if is_option_type(ty) {
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
                                let ptr2 = unsafe { data_base.add(__data_off) };
                                let remaining = total_rem
                                    .checked_sub(__data_off)
                                    .ok_or(norito::core::Error::LengthMismatch)?;
                                let slice = unsafe { std::slice::from_raw_parts(ptr2, remaining) };
                                #[cfg(debug_assertions)]
                                if norito::debug_trace_enabled() {
                                    let preview_len = core::cmp::min(slice.len(), 16);
                                    eprintln!(
                                        "packed decode {}::{} compat slice_len={} preview={:?}",
                                        stringify!(#ident),
                                        stringify!(#name),
                                        slice.len(),
                                        &slice[..preview_len]
                                    );
                                }
                                match norito::core::decode_field_canonical::<#ty>(slice) {
                                    Ok((compat, used)) => {
                                        __data_off += used;
                                        compat
                                    }
                                    Err(err) => return Err(err),
                                }
                            }
                        };
                        let compat_decode_named_len_mismatch = compat_decode_named.clone();
                        if attrs.skip {
                            quote! { #name: Default::default() }
                        } else if let Some(len_expr) = u8_array_len(ty) {
                            quote!{
                                #name: {
                                    let mut __arr: [u8; #len_expr] = [0; #len_expr];
                                    let __slice = unsafe { ::std::slice::from_raw_parts(data_base.add(__data_off), #len_expr) };
                                    __arr.copy_from_slice(__slice);
                                    __data_off += #len_expr;
                                    __arr
                                }
                            }
                        } else if is_self_delimiting(ty) {
                            quote!{
                                #name: {
                                    // Read the inner self-delimiting payload length header directly at pointer
                                    let (field_len, hdr) = match unsafe {
                                        norito::core::try_read_len_ptr_unchecked(data_base.add(__data_off))
                                    } {
                                        Ok(res) => res,
                                        Err(err) => return Err(err),
                                    };
                                    let data_ptr = unsafe { data_base.add(__data_off) };
                                    let total_len = hdr + field_len;
                                    #[cfg(debug_assertions)]
                                    if norito::debug_trace_enabled() {
                                        let preview_len = core::cmp::min(total_len, 16);
                                        let preview = unsafe {
                                            ::std::slice::from_raw_parts(data_ptr, preview_len)
                                        };
                                        eprintln!(
                                            "packed decode {}::{} self_delim hdr={} field_len={} preview={:?}",
                                            stringify!(#ident),
                                            stringify!(#name),
                                            hdr,
                                            field_len,
                                            preview
                                        );
                                    }
                                    __data_off += total_len;
                                    let field_data = unsafe { std::slice::from_raw_parts(data_ptr, total_len) };
                                    #[cfg(debug_assertions)]
                                    if norito::debug_trace_enabled() {
                                        let preview_len = core::cmp::min(total_len, 16);
                                        eprintln!(
                                            "packed decode {}::{} copied payload preview={:?}",
                                            stringify!(#ident),
                                            stringify!(#name),
                                            &field_data[..preview_len]
                                        );
                                    }
                                    let (value, _) = norito::core::decode_field_canonical::<#ty>(field_data)?;
                                    value
                                }
                            }
                        } else if let Some(fixed_len) = fixed_size {
                            let fixed_len_lit = fixed_len;
                            quote!{
                                #name: {
                                    // Copy exactly the fixed-size payload and deserialize
                                    let __fsz: usize = #fixed_len_lit;
                                    let ptr2 = unsafe { data_base.add(__data_off) };
                                    __data_off += __fsz;
                                    let field_data = unsafe { std::slice::from_raw_parts(ptr2, __fsz) };
                                    let (v, consumed) = norito::core::decode_field_canonical::<#ty>(field_data)?;
                                    debug_assert_eq!(consumed, __fsz);
                                    v
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
                                        let ptr2 = unsafe { data_base.add(__data_off) };
                                        __data_off += __len;
                                        unsafe {
                                            let layout = std::alloc::Layout::from_size_align(__len.max(1), core::mem::align_of::<norito::core::Archived<#ty>>()).unwrap();
                                            let tmp_ptr = std::alloc::alloc(layout);
                                            if tmp_ptr.is_null() {
                                                std::alloc::handle_alloc_error(layout);
                                            }
                                            core::ptr::copy(ptr2, tmp_ptr, __len);
                                            let tmp_slice = std::slice::from_raw_parts(tmp_ptr as *const u8, __len);
                                            let _payload_guard = norito::core::PayloadCtxGuard::enter(tmp_slice);
                                            let v_res: ::core::result::Result<#ty, norito::core::Error> = {
                                                // Try direct archived decode first
                                                match <#ty as norito::core::NoritoDeserialize>::try_deserialize(&*(tmp_ptr as *const norito::core::Archived<#ty>)) {
                                                    Ok(val) => Ok(val),
                                                    Err(err) => {
                                                        #[cfg(feature = "compact-len")]
                                                        {
                                                            let fallback = Err(err);
                                                            let compat = (|| {
                                                                if let Ok((inner_len, inner_hdr)) = norito::core::read_len_from_slice(tmp_slice) {
                                                                    if inner_hdr + inner_len == tmp_slice.len() {
                                                                        // Strip outer varint and decode inner archived bytes.
                                                                        let layout2 = std::alloc::Layout::from_size_align(inner_len.max(1), core::mem::align_of::<norito::core::Archived<#ty>>()).unwrap();
                                                                        let tmp2 = std::alloc::alloc(layout2);
                                                                        if tmp2.is_null() {
                                                                            std::alloc::handle_alloc_error(layout2);
                                                                        }
                                                                        core::ptr::copy(tmp_slice.as_ptr().add(inner_hdr), tmp2, inner_len);
                                                                        let inner_slice = std::slice::from_raw_parts(tmp2 as *const u8, inner_len);
                                                                        let _g2 = norito::core::PayloadCtxGuard::enter(inner_slice);
                                                                        let inner_res = <#ty as norito::core::NoritoDeserialize>::try_deserialize(&*(tmp2 as *const norito::core::Archived<#ty>));
                                                                        std::alloc::dealloc(tmp2, layout2);
                                                                        return Some(inner_res);
                                                                    }
                                                                }
                                                                None
                                                            })();
                                                            compat.unwrap_or(fallback)
                                                        }
                                                        #[cfg(not(feature = "compact-len"))]
                                                        {
                                                            Err(err)
                                                        }
                                                    }
                                                }
                                            };
                                            std::alloc::dealloc(tmp_ptr, layout);
                                            v_res?
                                        }
                                    } else {
                                        #compat_decode_named_len_mismatch
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
                                        let ptr2 = unsafe { data_base.add(__data_off) };
                                        __data_off += __len;
                                        unsafe {
                                            let layout = std::alloc::Layout::from_size_align(__len.max(1), core::mem::align_of::<norito::core::Archived<#ty>>()).unwrap();
                                            let tmp_ptr = std::alloc::alloc(layout);
                                            if tmp_ptr.is_null() {
                                                std::alloc::handle_alloc_error(layout);
                                            }
                                            core::ptr::copy(ptr2, tmp_ptr, __len);
                                            let tmp_slice = std::slice::from_raw_parts(tmp_ptr as *const u8, __len);
                                            let _payload_guard = norito::core::PayloadCtxGuard::enter(tmp_slice);
                                            let v_res: ::core::result::Result<#ty, norito::core::Error> = match <#ty as norito::core::NoritoDeserialize>::try_deserialize(&*(tmp_ptr as *const norito::core::Archived<#ty>)) {
                                                Ok(val) => Ok(val),
                                                Err(err) => {
                                                    #[cfg(feature = "compact-len")]
                                                    {
                                                        let fallback = Err(err);
                                                        let compat = (|| {
                                                            if let Ok((inner_len, inner_hdr)) = norito::core::read_len_from_slice(tmp_slice) {
                                                                if inner_hdr + inner_len == tmp_slice.len() {
                                                                    let layout2 = std::alloc::Layout::from_size_align(inner_len.max(1), core::mem::align_of::<norito::core::Archived<#ty>>()).unwrap();
                                                                    let tmp2 = std::alloc::alloc(layout2);
                                                                    if tmp2.is_null() {
                                                                        std::alloc::handle_alloc_error(layout2);
                                                                    }
                                                                    core::ptr::copy(tmp_slice.as_ptr().add(inner_hdr), tmp2, inner_len);
                                                                    let inner_slice = std::slice::from_raw_parts(tmp2 as *const u8, inner_len);
                                                                    let _g2 = norito::core::PayloadCtxGuard::enter(inner_slice);
                                                                    let inner_res = <#ty as norito::core::NoritoDeserialize>::try_deserialize(&*(tmp2 as *const norito::core::Archived<#ty>));
                                                                    std::alloc::dealloc(tmp2, layout2);
                                                                    return Some(inner_res);
                                                                }
                                                            }
                                                            None
                                                        })();
                                                        compat.unwrap_or(fallback)
                                                    }
                                                    #[cfg(not(feature = "compact-len"))]
                                                    {
                                                        Err(err)
                                                    }
                                                }
                                            };
                                            std::alloc::dealloc(tmp_ptr, layout);
                                            v_res?
                                        }
                                    } else { #compat_decode_named }
                                }
                            }
                        }
                    })
                    .collect(),
                _ => Vec::new(),
            };
            // Build per-field size-read statements for fields that require explicit sizes
            let read_sizes_stmts_named: Vec<TokenStream2> = match fields {
                Fields::Named(named) => named
                    .named
                    .iter()
                    .enumerate()
                    .filter_map(|(i, f)| {
                        let attrs = FieldAttr::parse(&f.attrs);
                        if attrs.skip { return None; }
                        let name = f.ident.as_ref().unwrap();
                        let bp_val: usize = named_bit_positions[i].expect("bitpos");
                        Some(quote!{
                            if (((*__bitset.get(#bp_val / 8).unwrap_or(&0)) >> ((#bp_val % 8) as u8)) & 1) != 0 {
                                match unsafe { norito::core::try_read_len_ptr_unchecked(ptr.add(__o)) } {
                                    Ok((sz, hdr)) => {
                                        __o += hdr;
                                        #[cfg(debug_assertions)]
                                        if norito::debug_trace_enabled() {
                                            eprintln!(
                                                "packed decode {}::{} size header={}",
                                                stringify!(#ident),
                                                stringify!(#name),
                                                sz
                                            );
                                        }
                                        __sizes.push(sz);
                                    }
                                    Err(err) => {
                                        #[cfg(debug_assertions)]
                                        if norito::debug_trace_enabled() {
                                            eprintln!(
                                                "packed decode {}::{} missing size header: {err:?}",
                                                stringify!(#ident),
                                                stringify!(#name)
                                            );
                                        }
                                        return Err(err);
                                    }
                                }
                            }
                        })
                    })
                    .collect(),
                _ => Vec::new(),
            };
            let __decode_from_slice_impl = if has_decode_from_slice_attr(container_attrs) {
                let (impl_generics2, ty_generics2, where_clause2) = r#gen.split_for_impl();
                quote! {
                    impl<'a> #impl_generics2 norito::core::DecodeFromSlice<'a> for #ident #ty_generics2 #where_clause2 {
                        #[inline]
                        fn decode_from_slice(bytes: &'a [u8]) -> ::core::result::Result<(Self, usize), norito::core::Error> {
                            let __logical_len = bytes.len();
                            let __min_size = ::core::mem::size_of::<norito::core::Archived<Self>>();
                            if __min_size > 0 && __logical_len == 0 {
                                return Err(norito::core::Error::LengthMismatch);
                            }
                            let __decode_bytes: ::std::borrow::Cow<'a, [u8]> =
                                if __min_size > 0 && __logical_len < __min_size {
                                    let mut __pad = ::std::vec::Vec::with_capacity(__min_size);
                                    __pad.extend_from_slice(bytes);
                                    __pad.resize(__min_size, 0);
                                    ::std::borrow::Cow::Owned(__pad)
                                } else {
                                    ::std::borrow::Cow::Borrowed(bytes)
                                };
                            let __archived = norito::core::archived_from_slice::<Self>(__decode_bytes.as_ref())?;
                            let __archived_bytes = __archived.bytes();
                            let _pg = norito::core::PayloadCtxGuard::enter_with_len(__archived_bytes, __logical_len);
                            let value = <Self as norito::core::NoritoDeserialize>::try_deserialize(__archived.archived())?;
                            Ok((value, __logical_len))
                        }
                    }
                }
            } else {
                quote! {}
            };

            quote! {
                impl #impl_generics norito::core::NoritoDeserialize<'de> for #ident #ty_generics #where_clause {
                    #[inline]
                    fn schema_hash() -> [u8; 16] {
                        #[cfg(feature = "schema-structural")]
                        { norito::core::schema_hash_structural::<Self>() }
                        #[cfg(not(feature = "schema-structural"))]
                        { norito::core::type_name_schema_hash::<Self>() }
                    }
                    fn deserialize(archived: &'de norito::core::Archived<Self>) -> Self {
                        match Self::try_deserialize(archived) {
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
                        let __value = if norito::core::use_packed_struct() {
                            let mut __o = 0usize;
                            let __count: usize = #packed_named_count;
                            // Hybrid packed-struct is indicated by the field-bitset flag.
                            // Packed-struct sizes follow COMPACT_LEN; packed-seq offsets
                            // are fixed-width in v1.
                            if #field_bitset_enabled_decode_named {
                                // Hybrid: read bitset, then sizes for needed fields; decode sequentially.
                                let __bitset_len: usize = (#packed_named_count).div_ceil(8);
                                let mut __bitset = ::std::vec::Vec::with_capacity(__bitset_len);
                                unsafe {
                                    __bitset.extend_from_slice(::std::slice::from_raw_parts(ptr.add(__o), __bitset_len));
                                }
                                if norito::debug_trace_enabled() {
                                    eprintln!(
                                        "decode struct {} bitset bytes={:?}",
                                        stringify!(#ident),
                                        __bitset
                                    );
                                }
                                __o += __bitset_len;
                                // Read sizes for fields that require explicit sizes
                                let mut __sizes: ::std::vec::Vec<usize> = ::std::vec::Vec::new();
                                // Read sizes in field order for those that require explicit size
                                { #(#read_sizes_stmts_named)* }
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
                                Self { #(#packed_named_inits_hybrid),* }
                            } else {
                                // Compat: read offsets or sizes into offsets array
                                let mut __offs: ::std::vec::Vec<usize> = ::std::vec::Vec::new();
                                let mut __packed_data_len: usize = 0;
                                let mut __packed_tail_len: usize = 0;
                                if __count == 0 {
                                    __offs.push(0);
                                } else {
                                    let (ctx_base, ctx_total) = if let Some(ctx) = norito::core::payload_ctx() {
                                        ctx
                                    } else {
                                        return Err(norito::core::Error::MissingPayloadContext);
                                    };
                                    let struct_start =
                                        (ptr as usize).saturating_sub(ctx_base);
                                    let payload_bytes = unsafe {
                                        ::std::slice::from_raw_parts(
                                            ctx_base as *const u8,
                                            ctx_total,
                                        )
                                    };
                                    let offsets_slice = payload_bytes
                                        .get(struct_start + __o..)
                                        .ok_or(norito::core::Error::LengthMismatch)?;
                                    let (computed_offs, used_offs, data_len, tail_len) =
                                        norito::core::decode_packed_offsets_slice(
                                            offsets_slice,
                                            __count,
                                        )?;
                                    __packed_data_len = data_len;
                                    __packed_tail_len = tail_len;
                                    __offs = computed_offs;
                                    __o += used_offs;
                                    #[cfg(debug_assertions)]
                                    if norito::debug_trace_enabled() {
                                        eprintln!(
                                            "decode struct {} offsets {:?} used={} count={} data_len={} tail_len={}",
                                            stringify!(#ident),
                                            __offs,
                                            used_offs,
                                            __count,
                                            __packed_data_len,
                                            __packed_tail_len
                                        );
                                    }
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
                                __value
                            }
                        } else {
                            let mut offset = 0usize;
                            let __value = Self { #(#deserialize_fields),* };
                            if let Some((__base, __total)) = norito::core::payload_ctx() {
                                if __total != 0 {
                                    let __ptr_us = ptr as usize;
                                    if __ptr_us < __base {
                                        return Err(norito::core::Error::LengthMismatch);
                                    }
                                    let __start = __ptr_us - __base;
                                    if __start > __total {
                                        return Err(norito::core::Error::LengthMismatch);
                                    }
                                    let __payload = unsafe {
                                        std::slice::from_raw_parts(__base as *const u8, __total)
                                    };
                                    let __remaining = __payload
                                        .get(__start..)
                                        .ok_or(norito::core::Error::LengthMismatch)?;
                                    if offset != __remaining.len() {
                                        return Err(norito::core::Error::LengthMismatch);
                                    }
                                    norito::core::note_payload_access(__remaining, offset);
                                } else if offset != 0 {
                                    return Err(norito::core::Error::LengthMismatch);
                                }
                            } else {
                                return Err(norito::core::Error::MissingPayloadContext);
                            }
                            __value
                        };
                        Ok(__value)
                    }
                }
                #__decode_from_slice_impl
            }
        }
        Fields::Unnamed(unnamed) => {
            let vars: Vec<_> = (0..unnamed.unnamed.len())
                .map(|i| format_ident!("field{}", i))
                .collect();
            let packed_unnamed_count: usize = match fields {
                Fields::Unnamed(unnamed) => unnamed
                    .unnamed
                    .iter()
                    .filter(|f| {
                        let a = FieldAttr::parse(&f.attrs);
                        !a.skip
                    })
                    .count(),
                _ => 0,
            };
            // Build packed-struct unnamed field statements (compat offsets-based)
            let packed_unnamed_stmts: Vec<TokenStream2> = match fields {
                Fields::Unnamed(unnamed) => unnamed
                    .unnamed
                    .iter()
                    .enumerate()
                    .map(|(i, f)| {
                        let attrs = FieldAttr::parse(&f.attrs);
                        let idx_var = format_ident!("field{}", i);
                        let ty = &f.ty;
                        if attrs.skip {
                            quote! { let #idx_var = Default::default(); }
                        } else if let Some(path) = attrs.default_fn.as_ref() {
                            quote! { let #idx_var = (#path)(); }
                        } else if attrs.default {
                            quote! { let #idx_var = Default::default(); }
                        } else {
                            quote! {
                                let #idx_var = {
                                    let __start = __offs[__i];
                                    let __end = __offs[__i + 1];
                                    __i += 1;
                                    let __len = __end - __start;
                                    unsafe {
                                        let layout = std::alloc::Layout::from_size_align(__len.max(1), core::mem::align_of::<norito::core::Archived<#ty>>()).unwrap();
                                        let tmp_ptr = std::alloc::alloc(layout);
                                        if tmp_ptr.is_null() {
                                            std::alloc::handle_alloc_error(layout);
                                        }
                                        core::ptr::copy(data_base.add(__start), tmp_ptr, __len);
                                        let tmp_slice = std::slice::from_raw_parts(tmp_ptr as *const u8, __len);
                                        let _g = norito::core::PayloadCtxGuard::enter(tmp_slice);
                                        let v_res = <#ty as norito::core::NoritoDeserialize>::try_deserialize(&*(tmp_ptr as *const norito::core::Archived<#ty>));
                                        std::alloc::dealloc(tmp_ptr, layout);
                                        v_res?
                                    }
                                };
                            }
                        }
                    })
                    .collect(),
                _ => Vec::new(),
            };
            // Determine if any unnamed field needs an explicit size
            let _unnamed_any_needs_size: bool = match fields {
                Fields::Unnamed(unnamed) => {
                    let mut any = false;
                    for f in &unnamed.unnamed {
                        let attrs = FieldAttr::parse(&f.attrs);
                        if attrs.skip {
                            continue;
                        }
                        let need = if attrs.needs_size {
                            true
                        } else {
                            is_staged_wrapper(&f.ty)
                                || !(is_self_delimiting(&f.ty) || is_fixed_size(&f.ty).is_some())
                        };
                        if need {
                            any = true;
                            break;
                        }
                    }
                    any
                }
                _ => false,
            };
            // Precompute bit positions for non-skipped fields (unnamed)
            let unnamed_bit_positions: Vec<Option<usize>> = match fields {
                Fields::Unnamed(unnamed) => {
                    let mut pos = 0usize;
                    unnamed
                        .unnamed
                        .iter()
                        .map(|f| {
                            let attrs = FieldAttr::parse(&f.attrs);
                            if attrs.skip {
                                None
                            } else {
                                let p = pos;
                                pos += 1;
                                Some(p)
                            }
                        })
                        .collect()
                }
                _ => Vec::new(),
            };
            // Build packed-struct unnamed field statements (hybrid bitset-based)
            let packed_unnamed_stmts_hybrid: Vec<TokenStream2> = match fields {
                Fields::Unnamed(unnamed) => unnamed
                    .unnamed
                    .iter()
                    .enumerate()
                    .map(|(i, f)| {
                        let attrs = FieldAttr::parse(&f.attrs);
                        let idx_var = format_ident!("field{}", i);
                        let ty = &f.ty;
                        let fixed_size = is_fixed_size(ty);
                        if attrs.skip {
                            quote! { let #idx_var = Default::default(); }
                        } else if let Some(path) = attrs.default_fn.as_ref() {
                            quote! { let #idx_var = (#path)(); }
                        } else if attrs.default {
                            quote! { let #idx_var = Default::default(); }
                        } else if let Some(len_expr) = u8_array_len(ty) {
                            quote! {
                                let #idx_var = {
                                    let mut __arr: [u8; #len_expr] = [0; #len_expr];
                                    let __slice = unsafe { ::std::slice::from_raw_parts(data_base.add(__data_off), #len_expr) };
                                    __arr.copy_from_slice(__slice);
                                    __data_off += #len_expr;
                                    __arr
                                };
                            }
                        } else if is_self_delimiting(ty) {
                            quote! {
                                let #idx_var = {
                                    let (field_len, hdr) = match unsafe {
                                        norito::core::try_read_len_ptr_unchecked(data_base.add(__data_off))
                                    } {
                                        Ok(res) => res,
                                        Err(err) => return Err(err),
                                    };
                                    let data_ptr = unsafe { data_base.add(__data_off) };
                                    let total_len = hdr + field_len;
                                    __data_off += total_len;
                                    unsafe {
                                        let layout = std::alloc::Layout::from_size_align(total_len.max(1), core::mem::align_of::<norito::core::Archived<#ty>>()).unwrap();
                                        let tmp_ptr = std::alloc::alloc(layout);
                                        if tmp_ptr.is_null() {
                                            std::alloc::handle_alloc_error(layout);
                                        }
                                        core::ptr::copy(data_ptr, tmp_ptr, total_len);
                                        let tmp_slice = std::slice::from_raw_parts(tmp_ptr as *const u8, total_len);
                                        let _g = norito::core::PayloadCtxGuard::enter(tmp_slice);
                                        let v_res = <#ty as norito::core::NoritoDeserialize>::try_deserialize(&*(tmp_ptr as *const norito::core::Archived<#ty>));
                                        std::alloc::dealloc(tmp_ptr, layout);
                                        v_res?
                                    }
                                };
                            }
                        } else if let Some(fixed_len) = fixed_size {
                            let fixed_len_lit = fixed_len;
                            quote! {
                                let #idx_var = {
                                    let __fsz: usize = #fixed_len_lit;
                                    let ptr2 = unsafe { data_base.add(__data_off) };
                                    __data_off += __fsz;
                                    unsafe {
                                        let layout = std::alloc::Layout::from_size_align(__fsz.max(1), core::mem::align_of::<norito::core::Archived<#ty>>()).unwrap();
                                        let tmp_ptr = std::alloc::alloc(layout);
                                        if tmp_ptr.is_null() {
                                            std::alloc::handle_alloc_error(layout);
                                        }
                                        core::ptr::copy(ptr2, tmp_ptr, __fsz);
                                        let tmp_slice = std::slice::from_raw_parts(tmp_ptr as *const u8, __fsz);
                                        let _g = norito::core::PayloadCtxGuard::enter(tmp_slice);
                                        let v_res = <#ty as norito::core::NoritoDeserialize>::try_deserialize(&*(tmp_ptr as *const norito::core::Archived<#ty>));
                                        std::alloc::dealloc(tmp_ptr, layout);
                                        v_res?
                                    }
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
                                        let ptr2 = unsafe { data_base.add(__data_off) };
                                        __data_off += __len;
                                        unsafe {
                                            let layout = std::alloc::Layout::from_size_align(__len.max(1), core::mem::align_of::<norito::core::Archived<#ty>>()).unwrap();
                                            let tmp_ptr = std::alloc::alloc(layout);
                                            if tmp_ptr.is_null() {
                                                std::alloc::handle_alloc_error(layout);
                                            }
                                            core::ptr::copy(ptr2, tmp_ptr, __len);
                                            let tmp_slice = std::slice::from_raw_parts(tmp_ptr as *const u8, __len);
                                            let (v, _used) = norito::core::decode_field_canonical::<#ty>(tmp_slice)?;
                                            std::alloc::dealloc(tmp_ptr, layout);
                                            v
                                        }
                                    } else {
                                        let ptr2 = unsafe { data_base.add(__data_off) };
                                        let remaining = total_rem
                                            .checked_sub(__data_off)
                                            .ok_or(norito::core::Error::LengthMismatch)?;
                                        let slice = unsafe { std::slice::from_raw_parts(ptr2, remaining) };
                                        let (value, used) = norito::core::decode_field_canonical::<#ty>(slice)?;
                                        __data_off += used;
                                        value
                                    }
                                };
                            }
                        }
                    })
                    .collect(),
                _ => Vec::new(),
            };
            let read_sizes_stmts_unnamed: Vec<TokenStream2> = match fields {
                Fields::Unnamed(unnamed) => unnamed
                    .unnamed
                    .iter()
                    .enumerate()
                    .filter_map(|(i, f)| {
                        let attrs = FieldAttr::parse(&f.attrs);
                        if attrs.skip { return None; }
                        let bp_val: usize = unnamed_bit_positions[i].expect("ubitpos");
                        Some(quote!{
                            if (((*__bitset.get(#bp_val / 8).unwrap_or(&0)) >> ((#bp_val % 8) as u8)) & 1) != 0 {
                                let (mut __sz, mut __hdr) = match unsafe { norito::core::try_read_len_ptr_unchecked(ptr.add(__o)) } {
                                    Ok(res) => res,
                                    Err(err) => return Err(err),
                                };
                                if __sz == 0 && __hdr == 1 {
                                    let mut __fallback = [0u8; 8];
                                    unsafe {
                                        __fallback.copy_from_slice(::std::slice::from_raw_parts(ptr.add(__o), 8));
                                    }
                                    let __len64 = u64::from_le_bytes(__fallback) as usize;
                                    if __len64 != 0 {
                                        __sz = __len64;
                                        __hdr = 8;
                                    }
                                }
                                __o += __hdr;
                                __sizes.push(__sz);
                            }
                        })
                    })
                    .collect(),
                _ => Vec::new(),
            };
            let __decode_from_slice_impl = if has_decode_from_slice_attr(container_attrs) {
                let (impl_generics2, ty_generics2, where_clause2) = r#gen.split_for_impl();
                quote! {
                    impl<'a> #impl_generics2 norito::core::DecodeFromSlice<'a> for #ident #ty_generics2 #where_clause2 {
                        #[inline]
                        fn decode_from_slice(bytes: &'a [u8]) -> ::core::result::Result<(Self, usize), norito::core::Error> {
                            let __logical_len = bytes.len();
                            let __min_size = ::core::mem::size_of::<norito::core::Archived<Self>>();
                            if __min_size > 0 && __logical_len == 0 {
                                return Err(norito::core::Error::LengthMismatch);
                            }
                            let __decode_bytes: ::std::borrow::Cow<'a, [u8]> =
                                if __min_size > 0 && __logical_len < __min_size {
                                    let mut __pad = ::std::vec::Vec::with_capacity(__min_size);
                                    __pad.extend_from_slice(bytes);
                                    __pad.resize(__min_size, 0);
                                    ::std::borrow::Cow::Owned(__pad)
                                } else {
                                    ::std::borrow::Cow::Borrowed(bytes)
                                };
                            let __archived = norito::core::archived_from_slice::<Self>(__decode_bytes.as_ref())?;
                            let __archived_bytes = __archived.bytes();
                            let _pg = norito::core::PayloadCtxGuard::enter_with_len(__archived_bytes, __logical_len);
                            let value = <Self as norito::core::NoritoDeserialize>::try_deserialize(__archived.archived())?;
                            Ok((value, __logical_len))
                        }
                    }
                }
            } else {
                quote! {}
            };
            quote! {
                impl #impl_generics norito::core::NoritoDeserialize<'de> for #ident #ty_generics #where_clause {
                    #[inline]
                    fn schema_hash() -> [u8; 16] {
                        #[cfg(feature = "schema-structural")]
                        { norito::core::schema_hash_structural::<Self>() }
                        #[cfg(not(feature = "schema-structural"))]
                        { norito::core::type_name_schema_hash::<Self>() }
                    }
                    fn deserialize(archived: &'de norito::core::Archived<Self>) -> Self {
                        match Self::try_deserialize(archived) {
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
                                let __bitset_len: usize = __count.div_ceil(8);
                                let mut __bitset = ::std::vec::Vec::with_capacity(__bitset_len);
                                unsafe {
                                    __bitset.extend_from_slice(::std::slice::from_raw_parts(ptr.add(__o), __bitset_len));
                                }
                                __o += __bitset_len;
                                // Read sizes for variable-length fields that are present
                                let mut __sizes: ::std::vec::Vec<usize> = ::std::vec::Vec::new();
                                { #(#read_sizes_stmts_unnamed)* }
                                // Decode payload sequentially
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
                                #(#packed_unnamed_stmts_hybrid)*
                                Self( #(#vars),* )
                            } else {
                                let mut __offs: ::std::vec::Vec<usize> = ::std::vec::Vec::new();
                                let mut __packed_data_len: usize = 0;
                                let mut __packed_tail_len: usize = 0;
                                if __count == 0 {
                                    __offs.push(0);
                                } else {
                                    let (ctx_base, ctx_total) = if let Some(ctx) = norito::core::payload_ctx() {
                                        ctx
                                    } else {
                                        return Err(norito::core::Error::MissingPayloadContext);
                                    };
                                    let struct_start =
                                        (ptr as usize).saturating_sub(ctx_base);
                                    let payload_bytes = unsafe {
                                        ::std::slice::from_raw_parts(
                                            ctx_base as *const u8,
                                            ctx_total,
                                        )
                                    };
                                    let offsets_slice = payload_bytes
                                        .get(struct_start + __o..)
                                        .ok_or(norito::core::Error::LengthMismatch)?;
                                    let (computed_offs, used_offs, data_len, tail_len) =
                                        norito::core::decode_packed_offsets_slice(
                                            offsets_slice,
                                            __count,
                                        )?;
                                    __packed_data_len = data_len;
                                    __packed_tail_len = tail_len;
                                    __offs = computed_offs;
                                    __o += used_offs;
                                    #[cfg(debug_assertions)]
                                    if norito::debug_trace_enabled() {
                                        eprintln!(
                                            "decode struct {} offsets {:?} used={} count={} data_len={} tail_len={}",
                                            stringify!(#ident),
                                            __offs,
                                            used_offs,
                                            __count,
                                            __packed_data_len,
                                            __packed_tail_len
                                        );
                                    }
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
                                Self( #(#vars),* )
                            }
                        } else {
                            let mut offset = 0usize;
                            #(#deserialize_fields)*
                            let __value = Self( #(#vars),* );
                            if let Some((__base, __total)) = norito::core::payload_ctx() {
                                if __total != 0 {
                                    let __ptr_us = ptr as usize;
                                    if __ptr_us < __base {
                                        return Err(norito::core::Error::LengthMismatch);
                                    }
                                    let __start = __ptr_us - __base;
                                    if __start > __total {
                                        return Err(norito::core::Error::LengthMismatch);
                                    }
                                    let __payload = unsafe {
                                        std::slice::from_raw_parts(__base as *const u8, __total)
                                    };
                                    let __remaining = __payload
                                        .get(__start..)
                                        .ok_or(norito::core::Error::LengthMismatch)?;
                                    if offset != __remaining.len() {
                                        return Err(norito::core::Error::LengthMismatch);
                                    }
                                    norito::core::note_payload_access(__remaining, offset);
                                } else if offset != 0 {
                                    return Err(norito::core::Error::LengthMismatch);
                                }
                            } else {
                                return Err(norito::core::Error::MissingPayloadContext);
                            }
                            __value
                        };
                        Ok(__value)
                    }
                }
                #__decode_from_slice_impl
            }
        }
        Fields::Unit => {
            let __decode_from_slice_impl = if has_decode_from_slice_attr(container_attrs) {
                let (impl_generics2, ty_generics2, where_clause2) = r#gen.split_for_impl();
                quote! {
                    impl<'a> #impl_generics2 norito::core::DecodeFromSlice<'a> for #ident #ty_generics2 #where_clause2 {
                        #[inline]
                        fn decode_from_slice(bytes: &'a [u8]) -> ::core::result::Result<(Self, usize), norito::core::Error> {
                            let __logical_len = bytes.len();
                            let __min_size = ::core::mem::size_of::<norito::core::Archived<Self>>();
                            if __min_size > 0 && __logical_len == 0 {
                                return Err(norito::core::Error::LengthMismatch);
                            }
                            let __decode_bytes: ::std::borrow::Cow<'a, [u8]> =
                                if __min_size > 0 && __logical_len < __min_size {
                                    let mut __pad = ::std::vec::Vec::with_capacity(__min_size);
                                    __pad.extend_from_slice(bytes);
                                    __pad.resize(__min_size, 0);
                                    ::std::borrow::Cow::Owned(__pad)
                                } else {
                                    ::std::borrow::Cow::Borrowed(bytes)
                                };
                            let __archived = norito::core::archived_from_slice::<Self>(__decode_bytes.as_ref())?;
                            let __archived_bytes = __archived.bytes();
                            let _pg = norito::core::PayloadCtxGuard::enter_with_len(__archived_bytes, __logical_len);
                            let value = <Self as norito::core::NoritoDeserialize>::try_deserialize(__archived.archived())?;
                            Ok((value, __logical_len))
                        }
                    }
                }
            } else {
                quote! {}
            };
            quote! {
                impl #impl_generics norito::core::NoritoDeserialize<'de> for #ident #ty_generics #where_clause {
                    #[inline]
                    fn schema_hash() -> [u8; 16] {
                        #[cfg(feature = "schema-structural")]
                        { norito::core::schema_hash_structural::<Self>() }
                        #[cfg(not(feature = "schema-structural"))]
                        { norito::core::type_name_schema_hash::<Self>() }
                    }
                    fn deserialize(_archived: &'de norito::core::Archived<Self>) -> Self {
                        Self
                    }
                }
                #__decode_from_slice_impl
            }
        }
    }
}

/// Generate `NoritoSerialize` implementation for an enum.
///
/// Each variant is preceded by a `u32` discriminant followed by its fields.
fn derive_enum_serialize(
    ident: &syn::Ident,
    generics: &Generics,
    data: &DataEnum,
    container_attrs: &[Attribute],
) -> TokenStream2 {
    let mut r#gen = generics.clone();
    let mut arms = Vec::new();
    let mut hint_arms = Vec::new();
    let mut exact_arms = Vec::new();

    for (idx, variant) in data.variants.iter().enumerate() {
        let v_ident = &variant.ident;
        let disc = variant_index(variant, idx);
        match &variant.fields {
            Fields::Unit => {
                arms.push(quote! {
                    Self::#v_ident => {
                        norito::core::NoritoSerialize::serialize(&(#disc as u32), &mut writer)?;
                    }
                });
                hint_arms.push(quote! { Self::#v_ident => Some(4) });
                exact_arms.push(quote! { Self::#v_ident => Some(4) });
            }
            Fields::Unnamed(fields) => {
                let bindings: Vec<_> = (0..fields.unnamed.len())
                    .map(|i| format_ident!("field{}", i))
                    .collect();
                let serialize_calls = fields
                    .unnamed
                    .iter()
                    .zip(bindings.iter())
                    .filter_map(|(f, b)| {
                        let attrs = FieldAttr::parse(&f.attrs);
                        if attrs.skip {
                            return None;
                        }
                        add_bound(&mut r#gen, &f.ty, quote!(norito::core::NoritoSerialize));
                        let is_sd = is_self_delimiting(&f.ty);
                        let is_fixed = is_fixed_size(&f.ty).is_some();
                        let is_u8_array = matches!(&f.ty, syn::Type::Array(arr) if matches!(&*arr.elem, syn::Type::Path(tp) if tp.path.is_ident("u8")));
                        let ser = if is_sd || is_fixed {
                            if is_u8_array {
                                quote! {
                                    if __norito_packed {
                                        writer.write_all(&#b[..])?;
                                    } else {
                                        let __len_bytes = core::mem::size_of_val(#b);
                                        #[cfg(feature = "compact-len")]
                                        { norito::core::write_len(&mut writer, __len_bytes as u64)?; }
                                        #[cfg(not(feature = "compact-len"))]
                                        { writer.write_u64::<norito::core::LittleEndian>(__len_bytes as u64)?; }
                                        writer.write_all(&#b[..])?;
                                    }
                                }
                            } else {
                                quote! {
                                    if __norito_packed {
                                        norito::core::NoritoSerialize::serialize(#b, &mut writer)?;
                                    } else {
                                        norito::core::write_len_prefixed(
                                            &mut writer,
                                            #b,
                                            &mut __norito_tmp,
                                        )?;
                                    }
                                }
                            }
                        } else {
                            quote! {
                                // Non self-delimiting, non-fixed types keep outer length framing even in packed builds
                                norito::core::write_len_prefixed(
                                    &mut writer,
                                    #b,
                                    &mut __norito_tmp,
                                )?;
                            }
                        };
                        Some(ser)
                    });

                // Serialization calls without per-field outer length for needs-size fields
                let _serialize_calls_nohdr =
                    fields
                        .unnamed
                        .iter()
                        .zip(bindings.iter())
                        .filter_map(|(f, b)| {
                            let attrs = FieldAttr::parse(&f.attrs);
                            if attrs.skip {
                                return None;
                            }
                            let is_sd = is_self_delimiting(&f.ty);
                            let is_fixed = is_fixed_size(&f.ty).is_some();
                            let ser = if is_sd || is_fixed {
                                quote! {
                                    if __norito_packed {
                                        norito::core::NoritoSerialize::serialize(#b, &mut writer)?;
                                    } else {
                                        norito::core::write_len_prefixed(
                                            &mut writer,
                                            #b,
                                            &mut __norito_tmp,
                                        )?;
                                    }
                                }
                            } else {
                                quote! {
                                    norito::core::write_len_prefixed(
                                        &mut writer,
                                        #b,
                                        &mut __norito_tmp,
                                    )?;
                                }
                            };
                            Some(ser)
                        });
                arms.push(quote! {
                    Self::#v_ident(#(#bindings),*) => {
                        let __norito_packed = norito::core::use_packed_struct();
                        norito::core::NoritoSerialize::serialize(&(#disc as u32), &mut writer)?;
                        let mut __norito_tmp: norito::core::DeriveSmallBuf = norito::core::DeriveSmallBuf::new();
                        #(#serialize_calls)*
                    }
                });
                let mut adds = Vec::new();
                for b in &bindings {
                    adds.push(quote! { __sum = __sum.saturating_add(8 + norito::core::NoritoSerialize::encoded_len_hint(#b)?); });
                }
                hint_arms.push(quote! {
                    Self::#v_ident(#(#bindings),*) => { let mut __sum: usize = 4; #(#adds)* Some(__sum) }
                });

                // exact arms
                let mut exact_adds = Vec::new();
                for (i, _b) in bindings.iter().enumerate() {
                    let b = &bindings[i];
                    let ty = &fields.unnamed[i].ty;
                    let is_sd = is_self_delimiting(ty);
                    let is_fixed = is_fixed_size(ty).is_some();
                    let add = if is_sd || is_fixed {
                        quote! {
                            let __e = norito::core::NoritoSerialize::encoded_len_exact(#b)?;
                            if norito::core::use_packed_struct() {
                                __sum = __sum.saturating_add(__e);
                            } else {
                                __sum = __sum.saturating_add(norito::core::len_prefix_len(__e) + __e);
                            }
                        }
                    } else {
                        quote! {
                            let __e = norito::core::NoritoSerialize::encoded_len_exact(#b)?;
                            let __prefix_len = norito::core::len_prefix_len(__e);
                            __sum = __sum.saturating_add(__prefix_len + __e);
                        }
                    };
                    exact_adds.push(add);
                }
                exact_arms.push(quote! {
                    Self::#v_ident( #( #bindings ),* ) => {
                        let mut __sum: usize = 4;
                        #(#exact_adds)*
                        Some(__sum)
                    }
                });
            }
            Fields::Named(fields) => {
                let names: Vec<_> = fields
                    .named
                    .iter()
                    .map(|f| f.ident.as_ref().unwrap())
                    .collect();
                let serialize_calls = fields.named.iter().filter_map(|f| {
                    let attrs = FieldAttr::parse(&f.attrs);
                    if attrs.skip {
                        return None;
                    }
                    let name = f.ident.as_ref().unwrap();
                    add_bound(&mut r#gen, &f.ty, quote!(norito::core::NoritoSerialize));
                    let is_sd = is_self_delimiting(&f.ty);
                    let is_fixed = is_fixed_size(&f.ty).is_some();
                    let is_u8_array = matches!(&f.ty, syn::Type::Array(arr) if matches!(&*arr.elem, syn::Type::Path(tp) if tp.path.is_ident("u8")));
                    let ser = if is_sd || is_fixed {
                        if is_u8_array {
                            quote! {
                                if __norito_packed {
                                    writer.write_all(&#name[..])?;
                                } else {
                                    let __len_bytes = core::mem::size_of_val(#name);
                                    #[cfg(feature = "compact-len")]
                                    { norito::core::write_len(&mut writer, __len_bytes as u64)?; }
                                    #[cfg(not(feature = "compact-len"))]
                                    { writer.write_u64::<norito::core::LittleEndian>(__len_bytes as u64)?; }
                                    writer.write_all(&#name[..])?;
                                }
                            }
                        } else {
                            quote! {
                                if __norito_packed {
                                    norito::core::NoritoSerialize::serialize(#name, &mut writer)?;
                                } else {
                                    norito::core::write_len_prefixed(
                                        &mut writer,
                                        #name,
                                        &mut __norito_tmp,
                                    )?;
                                }
                            }
                        }
                    } else {
                        // Non self-delimiting, non-fixed: always write an outer length header
                        // for named enum fields (both in packed and non-packed modes).
                        quote! {
                            norito::core::write_len_prefixed(
                                &mut writer,
                                #name,
                                &mut __norito_tmp,
                            )?;
                        }
                    };
                    Some(ser)
                });
                arms.push(quote! {
                    Self::#v_ident { #(#names),* } => {
                        let __norito_packed = norito::core::use_packed_struct();
                        norito::core::NoritoSerialize::serialize(&(#disc as u32), &mut writer)?;
                        let mut __norito_tmp: norito::core::DeriveSmallBuf = norito::core::DeriveSmallBuf::new();
                        #(#serialize_calls)*
                    }
                });
                let mut adds = Vec::new();
                for name in &names {
                    adds.push(quote! { __sum = __sum.saturating_add(8 + norito::core::NoritoSerialize::encoded_len_hint(#name)?); });
                }
                hint_arms.push(quote! {
                    Self::#v_ident { #(#names),* } => { let mut __sum: usize = 4; #(#adds)* Some(__sum) }
                });

                // exact arm
                let mut exact_adds = Vec::new();
                for (i, name) in names.iter().enumerate() {
                    let ty = &fields.named[i].ty;
                    let is_sd = is_self_delimiting(ty);
                    let is_fixed = is_fixed_size(ty).is_some();
                    let add = if is_sd || is_fixed {
                        quote! {
                            let __e = norito::core::NoritoSerialize::encoded_len_exact(#name)?;
                            if norito::core::use_packed_struct() {
                                __sum = __sum.saturating_add(__e);
                            } else {
                                __sum = __sum.saturating_add(norito::core::len_prefix_len(__e) + __e);
                            }
                        }
                    } else {
                        quote! {
                            let __e = norito::core::NoritoSerialize::encoded_len_exact(#name)?;
                            let __prefix_len = norito::core::len_prefix_len(__e);
                            __sum = __sum.saturating_add(__prefix_len + __e);
                        }
                    };
                    exact_adds.push(add);
                }
                exact_arms.push(quote! {
                    Self::#v_ident { #( #names ),* } => { let mut __sum: usize = 4; #(#exact_adds)* Some(__sum) }
                });
            }
        }
    }

    let (impl_generics, ty_generics, where_clause) = r#gen.split_for_impl();
    let params = r#gen
        .params
        .iter()
        .map(|param| match param {
            syn::GenericParam::Type(ty) => {
                let ident = &ty.ident;
                quote! { #ident }
            }
            syn::GenericParam::Lifetime(lt) => {
                let lt = &lt.lifetime;
                quote! { #lt }
            }
            syn::GenericParam::Const(c) => {
                let ident = &c.ident;
                quote! { #ident }
            }
        })
        .collect::<Vec<_>>();
    let alias_generics = if params.is_empty() {
        quote! {}
    } else {
        quote! { < #( #params ),* > }
    };
    let archived = format_ident!("Archived{}", ident);
    let alias_decl = if reuse_archived_alias(container_attrs) {
        quote! {}
    } else {
        let archived_doc = format!(
            "Archived Norito representation of `{}` generated by `#[derive(NoritoSerialize)]`.",
            ident
        );
        quote! {
            #[doc = #archived_doc]
            pub type #archived #alias_generics = norito::core::Archived<#ident #ty_generics>;
        }
    };
    quote! {
        #alias_decl
        impl #impl_generics norito::core::NoritoSerialize for #ident #ty_generics #where_clause {
            #[inline]
            fn schema_hash() -> [u8; 16] {
                #[cfg(feature = "schema-structural")]
                { norito::core::schema_hash_structural::<Self>() }
                #[cfg(not(feature = "schema-structural"))]
                { norito::core::type_name_schema_hash::<Self>() }
            }
            #[inline]
            fn encoded_len_hint(&self) -> Option<usize> {
                match self { #( #hint_arms ),* }
            }
            #[inline]
            fn encoded_len_exact(&self) -> Option<usize> {
                match self { #( #exact_arms ),* }
            }
            fn serialize<W: std::io::Write>(&self, mut writer: W) -> ::core::result::Result<(), norito::core::Error> {
                use norito::core::WriteBytesExt;
                match self {
                    #(#arms),*
                }
                Ok(())
            }
        }
    }
}

/// Generate `NoritoDeserialize` implementation for an enum.
fn derive_enum_deserialize(
    ident: &syn::Ident,
    generics: &Generics,
    data: &DataEnum,
    container_attrs: &[Attribute],
) -> TokenStream2 {
    let mut r#gen = generics.clone();
    let mut arms = Vec::new();

    // Helper to detect [u8; N] array length for specialized AoS path
    fn u8_array_len(ty: &syn::Type) -> Option<syn::Expr> {
        if let syn::Type::Array(arr) = ty
            && let syn::Type::Path(tp) = &*arr.elem
            && tp.path.is_ident("u8")
        {
            return Some(arr.len.clone());
        }
        None
    }

    for (idx, variant) in data.variants.iter().enumerate() {
        let v_ident = &variant.ident;
        let disc = variant_index(variant, idx);
        match &variant.fields {
            Fields::Unit => arms.push(quote! {
                #disc => {
                    let offset = 4usize;
                    let __payload = norito::core::payload_slice_from_ptr(ptr)?;
                    if offset != __payload.len() {
                        return Err(norito::core::Error::LengthMismatch);
                    }
                    norito::core::note_payload_access(__payload, offset);
                    Self::#v_ident
                }
            }),
            Fields::Unnamed(fields) => {
                let deser_stmts: Vec<TokenStream2> = fields
                    .unnamed
                    .iter()
                    .enumerate()
                    .map(|(i, f)| {
                        let attrs = FieldAttr::parse(&f.attrs);
                        let ty = &f.ty;
                        let idx_var = format_ident!("field{}", i);
                        if attrs.skip {
                            add_bound(&mut r#gen, ty, quote!(Default));
                            quote! {
                                let #idx_var = Default::default();
                            }
                        } else if let Some(path) = attrs.default_fn.as_ref() {
                            quote! {
                                let #idx_var = (#path)();
                            }
                        } else if attrs.default {
                            add_bound(&mut r#gen, ty, quote!(Default));
                            quote! {
                                let #idx_var = Default::default();
                            }
                        } else {
                            add_bound(&mut r#gen, ty, quote!(for<'__d> norito::core::NoritoDeserialize<'__d>));
                            add_bound(&mut r#gen, ty, quote!(norito::core::NoritoSerialize));
                            let is_sd = is_self_delimiting(&f.ty);
                            let fixed_size = is_fixed_size(&f.ty);
                            let is_fixed = fixed_size.is_some();
                            if is_sd || is_fixed {
                                if let Some(len_expr) = u8_array_len(ty) {
                                    quote! {
                                        let #idx_var = {
                                            if norito::core::use_packed_struct() {
                                                let __slice = unsafe { std::slice::from_raw_parts(ptr.add(offset), #len_expr as usize) };
                                                let mut __arr: [u8; #len_expr] = [0; #len_expr];
                                                __arr.copy_from_slice(__slice);
                                                offset += #len_expr as usize;
                                                __arr
                                            } else {
                                                let (field_len, hdr) = match unsafe { norito::core::try_read_len_ptr_unchecked(ptr.add(offset)) } {
                                                    Ok(res) => res,
                                                    Err(err) => return Err(err),
                                                };
                                                let data_ptr = unsafe { ptr.add(offset + hdr) };
                                                let field_data = unsafe { std::slice::from_raw_parts(data_ptr, field_len) };
                                                let (value, consumed) = norito::core::decode_field_canonical::<#ty>(field_data)?;
                                                #[cfg(feature = "compact-len")]
                                                { offset += hdr + consumed; }
                                                #[cfg(not(feature = "compact-len"))]
                                                { offset += hdr + consumed; }
                                                value
                                            }
                                        };
                                    }
                                } else {
                                    // Distinguish self-delimiting vs fixed-size for packed enums.
                                    if is_sd {
                                        quote! {
                                            let #idx_var = {
                                                if norito::core::use_packed_struct() {
                                                    // Self-delimiting: inner header present; read it and decode exact window.
                                                    let (field_len, hdr) = match unsafe { norito::core::try_read_len_ptr_unchecked(ptr.add(offset)) } {
                                                        Ok(res) => res,
                                                        Err(err) => return Err(err),
                                                    };
                                                    let data_ptr = unsafe { ptr.add(offset) };
                                                    let layout = std::alloc::Layout::from_size_align(hdr + field_len, core::mem::align_of::<norito::core::Archived<#ty>>()).unwrap();
                                                    let tmp_ptr = unsafe { std::alloc::alloc(layout) };
                                                    if tmp_ptr.is_null() {
                                                        std::alloc::handle_alloc_error(layout);
                                                    }
                                                    unsafe { std::ptr::copy(data_ptr, tmp_ptr, hdr + field_len); }
                                                    let tmp_slice = unsafe { std::slice::from_raw_parts(tmp_ptr as *const u8, hdr + field_len) };
                                                    let _g = norito::core::PayloadCtxGuard::enter(tmp_slice);
                                                    let val_res = <#ty as norito::core::NoritoDeserialize>::try_deserialize(unsafe { &*(tmp_ptr as *const norito::core::Archived<#ty>) });
                                                    unsafe { std::alloc::dealloc(tmp_ptr, layout); }
                                                    let val = val_res?;
                                                    offset += hdr + field_len;
                                                    val
                                                } else {
                                                    // AoS: length-prefixed outer framing (pointer-based)
                                                    let (field_len, hdr) = match unsafe { norito::core::try_read_len_ptr_unchecked(ptr.add(offset)) } {
                                                        Ok(res) => res,
                                                        Err(err) => return Err(err),
                                                    };
                                                    let data_ptr = unsafe { ptr.add(offset + hdr) };
                                                    let layout = std::alloc::Layout::from_size_align(field_len.max(1), core::mem::align_of::<norito::core::Archived<#ty>>()).unwrap();
                                                    let tmp_ptr = unsafe { std::alloc::alloc(layout) };
                                                    if tmp_ptr.is_null() {
                                                        std::alloc::handle_alloc_error(layout);
                                                    }
                                                    unsafe { std::ptr::copy(data_ptr, tmp_ptr, field_len); }
                                                    let tmp_slice = unsafe { std::slice::from_raw_parts(tmp_ptr as *const u8, field_len) };
                                                    let _g = norito::core::PayloadCtxGuard::enter(tmp_slice);
                                                    let value_res = <#ty as norito::core::NoritoDeserialize>::try_deserialize(unsafe { &*(tmp_ptr as *const norito::core::Archived<#ty>) });
                                                    unsafe { std::alloc::dealloc(tmp_ptr, layout); }
                                                    let value = value_res?;
                                                    #[cfg(feature = "compact-len")]
                                                    { offset += hdr + field_len; }
                                                    #[cfg(not(feature = "compact-len"))]
                                                    { offset += hdr + field_len; }
                                                    value
                                                }
                                            };
                                        }
                                    } else {
                                    // Fixed-size (non [u8;N]) unnamed variant field
                                    let fixed_len_lit = fixed_size.expect("fixed-size field");
                                    quote! {
                                            let #idx_var = {
                                                if norito::core::use_packed_struct() {
                                                    let __fsz: usize = #fixed_len_lit;
                                                    let data = unsafe { std::slice::from_raw_parts(ptr.add(offset), __fsz) };
                                                    let layout = std::alloc::Layout::from_size_align(__fsz.max(1), core::mem::align_of::<norito::core::Archived<#ty>>()).unwrap();
                                                    let tmp_ptr = unsafe { std::alloc::alloc(layout) };
                                                    if tmp_ptr.is_null() {
                                                        std::alloc::handle_alloc_error(layout);
                                                    }
                                                    unsafe { std::ptr::copy(data.as_ptr(), tmp_ptr, __fsz); }
                                                    let tmp_slice = unsafe { std::slice::from_raw_parts(tmp_ptr as *const u8, __fsz) };
                                                    let _g = norito::core::PayloadCtxGuard::enter(tmp_slice);
                                                    let value_res = <#ty as norito::core::NoritoDeserialize>::try_deserialize(unsafe { &*(tmp_ptr as *const norito::core::Archived<#ty>) });
                                                    unsafe { std::alloc::dealloc(tmp_ptr, layout); }
                                                    let value = value_res?;
                                                    offset += __fsz;
                                                    value
                                                } else {
                                                    // AoS outer length + payload (pointer-based)
                                                    let (field_len, hdr) = match unsafe { norito::core::try_read_len_ptr_unchecked(ptr.add(offset)) } {
                                                        Ok(res) => res,
                                                        Err(err) => return Err(err),
                                                    };
                                                    let data_ptr = unsafe { ptr.add(offset + hdr) };
                                                    let layout = std::alloc::Layout::from_size_align(field_len.max(1), core::mem::align_of::<norito::core::Archived<#ty>>()).unwrap();
                                                    let tmp_ptr = unsafe { std::alloc::alloc(layout) };
                                                    if tmp_ptr.is_null() {
                                                        std::alloc::handle_alloc_error(layout);
                                                    }
                                                    unsafe { std::ptr::copy(data_ptr, tmp_ptr, field_len); }
                                                    let tmp_slice = unsafe { std::slice::from_raw_parts(tmp_ptr as *const u8, field_len) };
                                                    let _g = norito::core::PayloadCtxGuard::enter(tmp_slice);
                                                    let value_res = <#ty as norito::core::NoritoDeserialize>::try_deserialize(unsafe { &*(tmp_ptr as *const norito::core::Archived<#ty>) });
                                                    unsafe { std::alloc::dealloc(tmp_ptr, layout); }
                                                    let value = value_res?;
                                                    #[cfg(feature = "compact-len")]
                                                    { offset += hdr + field_len; }
                                                    #[cfg(not(feature = "compact-len"))]
                                                    { offset += hdr + field_len; }
                                                    value
                                                }
                                            };
                                        }
                                    }
                                }
                            } else {
                                let is_opt_res = is_option_or_result(ty);
                                if is_opt_res {
                                    quote! {
                                        let #idx_var = {
                                            // Unnamed variant Option/Result: read outer size, then decode bounded window
                                            let (field_len, hdr) = match unsafe { norito::core::try_read_len_ptr_unchecked(ptr.add(offset)) } {
                                                Ok(res) => res,
                                                Err(err) => return Err(err),
                                            };
                                            let raw = unsafe { std::slice::from_raw_parts(ptr.add(offset + hdr), field_len) };
                                            let (val, used) = norito::core::decode_field_canonical::<#ty>(raw)?;
                                            debug_assert_eq!(used, field_len);
                                            offset += hdr + field_len;
                                            val
                                        };
                                    }
                                } else if is_vec_type(ty) {
                                    quote! {
                                        let #idx_var = {
                                            if norito::core::use_packed_struct() {
                                                // Unnamed variant Vec: read outer size, then decode bounded window
                                                let (field_len, hdr) = match unsafe { norito::core::try_read_len_ptr_unchecked(ptr.add(offset)) } {
                                                    Ok(res) => res,
                                                    Err(err) => return Err(err),
                                                };
                                                match norito::core::decode_field_canonical::<#ty>(unsafe { std::slice::from_raw_parts(ptr.add(offset + hdr), field_len) }) {
                                                    Ok((val, used)) => { debug_assert_eq!(used, field_len); offset += hdr + used; val }
                                                    Err(_e) => {
                                                        // Archived fallback
                                                        let data_ptr = unsafe { ptr.add(offset + hdr) };
                                                        let layout = std::alloc::Layout::from_size_align(field_len.max(1), core::mem::align_of::<norito::core::Archived<#ty>>()).unwrap();
                                                        let tmp_ptr = unsafe { std::alloc::alloc(layout) };
                                                        if tmp_ptr.is_null() {
                                                            std::alloc::handle_alloc_error(layout);
                                                        }
                                                        unsafe { std::ptr::copy(data_ptr, tmp_ptr, field_len); }
                                                        let tmp_slice = unsafe { std::slice::from_raw_parts(tmp_ptr as *const u8, field_len) };
                                                        let _g = norito::core::PayloadCtxGuard::enter(tmp_slice);
                                                        let val_res = <#ty as norito::core::NoritoDeserialize>::try_deserialize(unsafe { &*(tmp_ptr as *const norito::core::Archived<#ty>) });
                                                        unsafe { std::alloc::dealloc(tmp_ptr, layout); }
                                                        let val = val_res?;
                                                        offset += hdr + field_len;
                                                        val
                                                    }
                                                }
                                            } else {
                                                let (field_len, hdr) = match unsafe { norito::core::try_read_len_ptr_unchecked(ptr.add(offset)) } {
                                                    Ok(res) => res,
                                                    Err(err) => return Err(err),
                                                };
                                                let data_ptr = unsafe { ptr.add(offset + hdr) };
                                                let layout = std::alloc::Layout::from_size_align(field_len.max(1), core::mem::align_of::<norito::core::Archived<#ty>>()).unwrap();
                                                let tmp_ptr = unsafe { std::alloc::alloc(layout) };
                                                if tmp_ptr.is_null() {
                                                    std::alloc::handle_alloc_error(layout);
                                                }
                                                unsafe { std::ptr::copy(data_ptr, tmp_ptr, field_len); }
                                                let tmp_slice = unsafe { std::slice::from_raw_parts(tmp_ptr as *const u8, field_len) };
                                                let _g = norito::core::PayloadCtxGuard::enter(tmp_slice);
                                                let val_res = <#ty as norito::core::NoritoDeserialize>::try_deserialize(unsafe { &*(tmp_ptr as *const norito::core::Archived<#ty>) });
                                                unsafe { std::alloc::dealloc(tmp_ptr, layout); }
                                                let val = val_res?;
                                                #[cfg(feature = "compact-len")]
                                                { offset += hdr + field_len; }
                                                #[cfg(not(feature = "compact-len"))]
                                                { offset += hdr + field_len; }
                                                val
                                            }
                                        };
                                    }
                                } else {
                                    quote! {
                                        let #idx_var = {
                                            let (field_len, hdr) = match unsafe { norito::core::try_read_len_ptr_unchecked(ptr.add(offset)) } {
                                                Ok(res) => res,
                                                Err(err) => return Err(err),
                                            };
                                            #[cfg(debug_assertions)]
                                            if norito::debug_trace_enabled() {
                                                eprintln!(
                                                    "decode enum {}::{} field{} size header={} hdr={}",
                                                    stringify!(#ident),
                                                    stringify!(#v_ident),
                                                    #i,
                                                    field_len,
                                                    hdr
                                                );
                                            }
                                            let data_ptr = unsafe { ptr.add(offset + hdr) };
                                            let mut __used = field_len;
                                            let mut __decoded: ::core::option::Option<#ty> = ::core::option::Option::None;
                                            {
                                                let (ctx_base, ctx_total) = if let Some(ctx) = norito::core::payload_ctx() {
                                                    ctx
                                                } else {
                                                    return Err(norito::core::Error::MissingPayloadContext);
                                                };
                                                let struct_start = (ptr as usize).saturating_sub(ctx_base);
                                                let field_start = struct_start
                                                    .checked_add(offset)
                                                    .and_then(|v| v.checked_add(hdr))
                                                    .ok_or(norito::core::Error::LengthMismatch)?;
                                                if field_start <= ctx_total {
                                                    let available = ctx_total - field_start;
                                                    #[cfg(debug_assertions)]
                                                    if norito::debug_trace_enabled() {
                                                        eprintln!(
                                                            "decode enum {}::{} field{} canonical attempt header={} ctx_total={} available={}",
                                                            stringify!(#ident),
                                                            stringify!(#v_ident),
                                                            #i,
                                                            field_len,
                                                            ctx_total,
                                                            available
                                                        );
                                                    }
                                                    let full_slice = unsafe {
                                                        std::slice::from_raw_parts(data_ptr, available)
                                                    };
                                                    match norito::core::decode_field_canonical::<#ty>(full_slice) {
                                                        Ok((val, used)) => {
                                                            __used = used;
                                                            __decoded = ::core::option::Option::Some(val);
                                                            #[cfg(debug_assertions)]
                                                            if norito::debug_trace_enabled() {
                                                                eprintln!(
                                                                    "decode enum {}::{} field{} canonical used={}",
                                                                    stringify!(#ident),
                                                                    stringify!(#v_ident),
                                                                    #i,
                                                                    used
                                                                );
                                                            }
                                                        }
                                                        Err(err) => {
                                                            #[cfg(debug_assertions)]
                                                            if norito::debug_trace_enabled() {
                                                                eprintln!(
                                                                    "decode enum {}::{} field{} canonical failed: {err:?}",
                                                                    stringify!(#ident),
                                                                    stringify!(#v_ident),
                                                                    #i
                                                                );
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                            let (val, used_total) = if let ::core::option::Option::Some(value) = __decoded {
                                                #[cfg(debug_assertions)]
                                                if norito::debug_trace_enabled() && __used != field_len {
                                                    eprintln!(
                                                        "decode enum {}::{} field{} canonical len mismatch header={} used={}",
                                                        stringify!(#ident),
                                                        stringify!(#v_ident),
                                                        #i,
                                                        field_len,
                                                        __used
                                                    );
                                                }
                                                (value, __used)
                                            } else {
                                                let layout = std::alloc::Layout::from_size_align(field_len.max(1), core::mem::align_of::<norito::core::Archived<#ty>>()).unwrap();
                                                let tmp_ptr = unsafe { std::alloc::alloc(layout) };
                                                if tmp_ptr.is_null() {
                                                    std::alloc::handle_alloc_error(layout);
                                                }
                                                unsafe { std::ptr::copy(data_ptr, tmp_ptr, field_len); }
                                                let tmp_slice = unsafe { std::slice::from_raw_parts(tmp_ptr as *const u8, field_len) };
                                                #[cfg(debug_assertions)]
                                                if norito::debug_trace_enabled() {
                                                    let preview_len = core::cmp::min(tmp_slice.len(), 16);
                                                    eprintln!(
                                                        "decode enum {}::{} field{} payload preview {:?}",
                                                        stringify!(#ident),
                                                        stringify!(#v_ident),
                                                        #i,
                                                        &tmp_slice[..preview_len]
                                                    );
                                                }
                                                let _g = norito::core::PayloadCtxGuard::enter(tmp_slice);
                                                let value_res = <#ty as norito::core::NoritoDeserialize>::try_deserialize(unsafe { &*(tmp_ptr as *const norito::core::Archived<#ty>) });
                                                unsafe { std::alloc::dealloc(tmp_ptr, layout); }
                                                let value = value_res?;
                                                (value, field_len)
                                            };
                                            offset += hdr + used_total;
                                            val
                                        };
                                    }
                                }
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
                        let __payload = norito::core::payload_slice_from_ptr(ptr)?;
                        if offset != __payload.len() {
                            return Err(norito::core::Error::LengthMismatch);
                        }
                        norito::core::note_payload_access(__payload, offset);
                        __value
                    }
                });
            }
            Fields::Named(fields) => {
                let deser_stmts: Vec<TokenStream2> = fields
                    .named
                    .iter()
                    .map(|f| {
                        let attrs = FieldAttr::parse(&f.attrs);
                        let name = f.ident.as_ref().unwrap();
                        let ty = &f.ty;
                        if attrs.skip {
                            add_bound(&mut r#gen, ty, quote!(Default));
                            quote! {
                                let #name = Default::default();
                            }
                        } else if let Some(path) = attrs.default_fn.as_ref() {
                            quote! {
                                let #name = (#path)();
                            }
                        } else if attrs.default {
                            add_bound(&mut r#gen, ty, quote!(Default));
                            quote! {
                                let #name = Default::default();
                            }
                        } else {
                            add_bound(&mut r#gen, ty, quote!(for<'__d> norito::core::NoritoDeserialize<'__d>));
                            add_bound(&mut r#gen, ty, quote!(norito::core::NoritoSerialize));
                            let is_sd = is_self_delimiting(&f.ty);
                            let fixed_size = is_fixed_size(&f.ty);
                            let is_fixed = fixed_size.is_some();
                            if is_sd || is_fixed {
                                if let Some(len_expr) = u8_array_len(ty) {
                                    quote! {
                                        let #name = {
                                            if norito::core::use_packed_struct() {
                                                let __slice = unsafe { std::slice::from_raw_parts(ptr.add(offset), #len_expr as usize) };
                                                let mut __arr: [u8; #len_expr] = [0; #len_expr];
                                                __arr.copy_from_slice(__slice);
                                                offset += #len_expr as usize;
                                                __arr
                                            } else {
                                                let (field_len, hdr) = match unsafe { norito::core::try_read_len_ptr_unchecked(ptr.add(offset)) } {
                                                    Ok(res) => res,
                                                    Err(err) => return Err(err),
                                                };
                                                let data_ptr = unsafe { ptr.add(offset + hdr) };
                                                let layout = std::alloc::Layout::from_size_align(field_len.max(1), core::mem::align_of::<norito::core::Archived<#ty>>()).unwrap();
                                                let tmp_ptr = unsafe { std::alloc::alloc(layout) };
                                                if tmp_ptr.is_null() {
                                                    std::alloc::handle_alloc_error(layout);
                                                }
                                                unsafe { std::ptr::copy(data_ptr, tmp_ptr, field_len); }
                                                let tmp_slice = unsafe { std::slice::from_raw_parts(tmp_ptr as *const u8, field_len) };
                                                let _g = norito::core::PayloadCtxGuard::enter(tmp_slice);
                                                let value_res = <#ty as norito::core::NoritoDeserialize>::try_deserialize(unsafe { &*(tmp_ptr as *const norito::core::Archived<#ty>) });
                                                unsafe { std::alloc::dealloc(tmp_ptr, layout); }
                                                let value = value_res?;
                                                #[cfg(feature = "compact-len")]
                                                { offset += hdr + field_len; }
                                                #[cfg(not(feature = "compact-len"))]
                                                { offset += hdr + field_len; }
                                                value
                                            }
                                        };
                                    }
                                } else if is_sd {
                                    quote! {
                                        let #name = {
                                            if norito::core::use_packed_struct() {
                                                // Packed: inner header lives at the current offset.
                                                let (field_len, hdr) = match unsafe { norito::core::try_read_len_ptr_unchecked(ptr.add(offset)) } {
                                                    Ok(res) => res,
                                                    Err(err) => return Err(err),
                                                };
                                                let total = hdr + field_len;
                                                let data_ptr = unsafe { ptr.add(offset) };
                                                let layout = std::alloc::Layout::from_size_align(total.max(1), core::mem::align_of::<norito::core::Archived<#ty>>()).unwrap();
                                                let tmp_ptr = unsafe { std::alloc::alloc(layout) };
                                                if tmp_ptr.is_null() {
                                                    std::alloc::handle_alloc_error(layout);
                                                }
                                                unsafe { std::ptr::copy(data_ptr, tmp_ptr, total); }
                                                let tmp_slice = unsafe { std::slice::from_raw_parts(tmp_ptr as *const u8, total) };
                                                let _g = norito::core::PayloadCtxGuard::enter(tmp_slice);
                                                let value_res = <#ty as norito::core::NoritoDeserialize>::try_deserialize(unsafe { &*(tmp_ptr as *const norito::core::Archived<#ty>) });
                                                unsafe { std::alloc::dealloc(tmp_ptr, layout); }
                                                let value = value_res?;
                                                offset += total;
                                                value
                                            } else {
                                                // AoS: outer field length wraps the self-delimiting payload.
                                                let (field_len, hdr) = match unsafe { norito::core::try_read_len_ptr_unchecked(ptr.add(offset)) } {
                                                    Ok(res) => res,
                                                    Err(err) => return Err(err),
                                                };
                                                let data_ptr = unsafe { ptr.add(offset + hdr) };
                                                let layout = std::alloc::Layout::from_size_align(field_len.max(1), core::mem::align_of::<norito::core::Archived<#ty>>()).unwrap();
                                                let tmp_ptr = unsafe { std::alloc::alloc(layout) };
                                                if tmp_ptr.is_null() {
                                                    std::alloc::handle_alloc_error(layout);
                                                }
                                                unsafe { std::ptr::copy(data_ptr, tmp_ptr, field_len); }
                                                let tmp_slice = unsafe { std::slice::from_raw_parts(tmp_ptr as *const u8, field_len) };
                                                let _g = norito::core::PayloadCtxGuard::enter(tmp_slice);
                                                let value_res = <#ty as norito::core::NoritoDeserialize>::try_deserialize(unsafe { &*(tmp_ptr as *const norito::core::Archived<#ty>) });
                                                unsafe { std::alloc::dealloc(tmp_ptr, layout); }
                                                let value = value_res?;
                                                #[cfg(feature = "compact-len")]
                                                { offset += hdr + field_len; }
                                                #[cfg(not(feature = "compact-len"))]
                                                { offset += hdr + field_len; }
                                                value
                                            }
                                        };
                                    }
                                } else { // fixed-size, non-[u8;N]
                                    let fixed_len_lit = fixed_size.expect("fixed-size field");
                                    quote! {
                                        let #name = {
                                            if norito::core::use_packed_struct() {
                                                let __fsz: usize = #fixed_len_lit;
                                                let data = unsafe { std::slice::from_raw_parts(ptr.add(offset), __fsz) };
                                                let layout = std::alloc::Layout::from_size_align(__fsz.max(1), core::mem::align_of::<norito::core::Archived<#ty>>()).unwrap();
                                                let tmp_ptr = unsafe { std::alloc::alloc(layout) };
                                                if tmp_ptr.is_null() {
                                                    std::alloc::handle_alloc_error(layout);
                                                }
                                                unsafe { std::ptr::copy(data.as_ptr(), tmp_ptr, __fsz); }
                                                let tmp_slice = unsafe { std::slice::from_raw_parts(tmp_ptr as *const u8, __fsz) };
                                                let _g = norito::core::PayloadCtxGuard::enter(tmp_slice);
                                                let value_res = <#ty as norito::core::NoritoDeserialize>::try_deserialize(unsafe { &*(tmp_ptr as *const norito::core::Archived<#ty>) });
                                                unsafe { std::alloc::dealloc(tmp_ptr, layout); }
                                                let value = value_res?;
                                                offset += __fsz;
                                                value
                                            } else {
                                                let (field_len, hdr) = match unsafe { norito::core::try_read_len_ptr_unchecked(ptr.add(offset)) } {
                                                    Ok(res) => res,
                                                    Err(err) => return Err(err),
                                                };
                                                let data_ptr = unsafe { ptr.add(offset + hdr) };
                                                let layout = std::alloc::Layout::from_size_align(field_len.max(1), core::mem::align_of::<norito::core::Archived<#ty>>()).unwrap();
                                                let tmp_ptr = unsafe { std::alloc::alloc(layout) };
                                                if tmp_ptr.is_null() {
                                                    std::alloc::handle_alloc_error(layout);
                                                }
                                                unsafe { std::ptr::copy(data_ptr, tmp_ptr, field_len); }
                                                let tmp_slice = unsafe { std::slice::from_raw_parts(tmp_ptr as *const u8, field_len) };
                                                let _g = norito::core::PayloadCtxGuard::enter(tmp_slice);
                                                let value_res = <#ty as norito::core::NoritoDeserialize>::try_deserialize(unsafe { &*(tmp_ptr as *const norito::core::Archived<#ty>) });
                                                unsafe { std::alloc::dealloc(tmp_ptr, layout); }
                                                let value = value_res?;
                                                #[cfg(feature = "compact-len")]
                                                { offset += hdr + field_len; }
                                                #[cfg(not(feature = "compact-len"))]
                                                { offset += hdr + field_len; }
                                                value
                                            }
                                        };
                                    }
                                }
                            } else {
                                let is_opt_res = is_option_or_result(ty);
                                if is_opt_res {
                                    quote! {
                                        let #name = {
                                            // Option/Result fields in enum named-variants carry an outer [len] header.
                                            let (field_len, hdr) = match unsafe { norito::core::try_read_len_ptr_unchecked(ptr.add(offset)) } {
                                                Ok(res) => res,
                                                Err(err) => return Err(err),
                                            };
                                            let sub = unsafe { std::slice::from_raw_parts(ptr.add(offset + hdr), field_len) };
                                            let (value, used) = norito::core::decode_field_canonical::<#ty>(sub)?;
                                            debug_assert_eq!(used, field_len);
                                            offset += hdr + field_len;
                                            value
                                        };
                                    }
                                } else if is_vec_type(ty) {
                                    quote! {
                                        let #name = {
                                            // Vec in enum named-variant: outer size present; use pointer-bounded window
                                            let (field_len, hdr) = match unsafe { norito::core::try_read_len_ptr_unchecked(ptr.add(offset)) } {
                                                Ok(res) => res,
                                                Err(err) => return Err(err),
                                            };
                                            match norito::core::decode_field_canonical::<#ty>(unsafe { std::slice::from_raw_parts(ptr.add(offset + hdr), field_len) }) {
                                                Ok((value, used)) => { debug_assert_eq!(used, field_len); offset += hdr + used; value }
                                                Err(_e) => {
                                                    // Archived fallback
                                                    let data_ptr = unsafe { ptr.add(offset + hdr) };
                                                    let layout = std::alloc::Layout::from_size_align(field_len.max(1), core::mem::align_of::<norito::core::Archived<#ty>>()).unwrap();
                                                    let tmp_ptr = unsafe { std::alloc::alloc(layout) };
                                                    if tmp_ptr.is_null() {
                                                        std::alloc::handle_alloc_error(layout);
                                                    }
                                                    unsafe { std::ptr::copy(data_ptr, tmp_ptr, field_len); }
                                                    let tmp_slice = unsafe { std::slice::from_raw_parts(tmp_ptr as *const u8, field_len) };
                                                    let _g = norito::core::PayloadCtxGuard::enter(tmp_slice);
                                                    let value_res = <#ty as norito::core::NoritoDeserialize>::try_deserialize(unsafe { &*(tmp_ptr as *const norito::core::Archived<#ty>) });
                                                    unsafe { std::alloc::dealloc(tmp_ptr, layout); }
                                                    let value = value_res?;
                                                    offset += hdr + field_len;
                                                    value
                                                }
                                            }
                                        };
                                    }
                                } else {
                                    quote! {
                                        let #name = {
                                            // Enum named-variants do not use a hybrid FIELD_BITSET header.
                                            // Read the outer field length at the current pointer and decode a bounded window.
                                            let (field_len, hdr) = match unsafe { norito::core::try_read_len_ptr_unchecked(ptr.add(offset)) } {
                                                Ok(res) => res,
                                                Err(err) => return Err(err),
                                            };
                                            let data_ptr = unsafe { ptr.add(offset + hdr) };
                                            let layout = std::alloc::Layout::from_size_align(field_len.max(1), core::mem::align_of::<norito::core::Archived<#ty>>()).unwrap();
                                            let tmp_ptr = unsafe { std::alloc::alloc(layout) };
                                            if tmp_ptr.is_null() {
                                                std::alloc::handle_alloc_error(layout);
                                            }
                                            unsafe { std::ptr::copy(data_ptr, tmp_ptr, field_len); }
                                            let tmp_slice = unsafe { std::slice::from_raw_parts(tmp_ptr as *const u8, field_len) };
                                            let _g = norito::core::PayloadCtxGuard::enter(tmp_slice);
                                            let value_res = <#ty as norito::core::NoritoDeserialize>::try_deserialize(unsafe { &*(tmp_ptr as *const norito::core::Archived<#ty>) });
                                            unsafe { std::alloc::dealloc(tmp_ptr, layout); }
                                            let value = value_res?;
                                            #[cfg(feature = "compact-len")]
                                            { offset += hdr + field_len; }
                                            #[cfg(not(feature = "compact-len"))]
                                            { offset += hdr + field_len; }
                                            value
                                        };
                                    }
                                }
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
                        let __payload = norito::core::payload_slice_from_ptr(ptr)?;
                        if offset != __payload.len() {
                            return Err(norito::core::Error::LengthMismatch);
                        }
                        norito::core::note_payload_access(__payload, offset);
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
    let __decode_from_slice_impl = if has_decode_from_slice_attr(container_attrs) {
        let (impl_generics2, ty_generics2, where_clause2) = r#gen.split_for_impl();
        quote! {
            impl<'a> #impl_generics2 norito::core::DecodeFromSlice<'a> for #ident #ty_generics2 #where_clause2 {
                #[inline]
                fn decode_from_slice(bytes: &'a [u8]) -> ::core::result::Result<(Self, usize), norito::core::Error> {
                    let __logical_len = bytes.len();
                    let __min_size = ::core::mem::size_of::<norito::core::Archived<Self>>();
                    if __min_size > 0 && __logical_len == 0 {
                        return Err(norito::core::Error::LengthMismatch);
                    }
                    let __decode_bytes: ::std::borrow::Cow<'a, [u8]> =
                        if __min_size > 0 && __logical_len < __min_size {
                            let mut __pad = ::std::vec::Vec::with_capacity(__min_size);
                            __pad.extend_from_slice(bytes);
                            __pad.resize(__min_size, 0);
                            ::std::borrow::Cow::Owned(__pad)
                        } else {
                            ::std::borrow::Cow::Borrowed(bytes)
                        };
                    let __archived = norito::core::archived_from_slice::<Self>(__decode_bytes.as_ref())?;
                    let __archived_bytes = __archived.bytes();
                    let _pg = norito::core::PayloadCtxGuard::enter_with_len(__archived_bytes, __logical_len);
                    let value = <Self as norito::core::NoritoDeserialize>::try_deserialize(__archived.archived())?;
                    Ok((value, __logical_len))
                }
            }
        }
    } else {
        quote! {}
    };
    quote! {
        impl #impl_generics norito::core::NoritoDeserialize<'de> for #ident #ty_generics #where_clause {
            fn deserialize(archived: &'de norito::core::Archived<Self>) -> Self {
                match Self::try_deserialize(archived) {
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
                // Read tag without assuming alignment
                let mut __tag_bytes = [0u8; 4];
                unsafe { __tag_bytes.copy_from_slice(std::slice::from_raw_parts(ptr, 4)); }
                let tag = u32::from_le_bytes(__tag_bytes);
                let value = match tag {
                    #(#arms,)*
                    _ => {
                        return Err(norito::core::Error::Message(
                            "invalid enum discriminant".into(),
                        ))
                    }
                };
                Ok(value)
            }
        }
        #__decode_from_slice_impl
    }
}

#[proc_macro_derive(NoritoSerialize, attributes(codec, norito))]
/// Entry point for the `#[derive(NoritoSerialize)]` macro.
pub fn derive_norito_serialize(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match &input.data {
        Data::Struct(data) => match validate_field_attrs(&data.fields) {
            Ok(()) => {
                derive_struct_serialize(&input.ident, &input.generics, &data.fields, &input.attrs)
                    .into()
            }
            Err(e) => e.to_compile_error().into(),
        },
        Data::Enum(data) => {
            derive_enum_serialize(&input.ident, &input.generics, data, &input.attrs).into()
        }
        _ => syn::Error::new_spanned(
            &input.ident,
            "NoritoSerialize only supports structs and enums",
        )
        .to_compile_error()
        .into(),
    }
}

#[proc_macro_derive(NoritoDeserialize, attributes(codec, norito))]
/// Entry point for the `#[derive(NoritoDeserialize)]` macro.
pub fn derive_norito_deserialize(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match &input.data {
        Data::Struct(data) => match validate_field_attrs(&data.fields) {
            Ok(()) => {
                derive_struct_deserialize(&input.ident, &input.generics, &data.fields, &input.attrs)
                    .into()
            }
            Err(e) => e.to_compile_error().into(),
        },
        Data::Enum(data) => {
            derive_enum_deserialize(&input.ident, &input.generics, data, &input.attrs).into()
        }
        _ => syn::Error::new_spanned(
            &input.ident,
            "NoritoDeserialize only supports structs and enums",
        )
        .to_compile_error()
        .into(),
    }
}

// ===== FastJson (prototype) =====

fn derive_fast_json_struct_flatten(
    ident: &syn::Ident,
    generics: &Generics,
    named: &syn::FieldsNamed,
    container_attrs: &ContainerAttr,
) -> syn::Result<TokenStream2> {
    let mut r#gen = generics.clone();
    let mut init_stmts = Vec::new();
    let mut parse_stmts = Vec::new();
    let mut flatten_stmts = Vec::new();
    let mut finals = Vec::new();

    for field in named.named.iter() {
        let attrs = FieldAttr::parse(&field.attrs);
        if let Some(err) = attrs.error {
            return Err(err);
        }
        let field_ident = field.ident.as_ref().unwrap();
        let ty = &field.ty;
        if attrs.skip {
            if attrs.default || attrs.default_fn.is_some() {
                return Err(syn::Error::new_spanned(
                    field,
                    "#[norito(skip)] cannot be combined with #[norito(default)]",
                ));
            }
            add_bound(&mut r#gen, &field.ty, quote!(::core::default::Default));
            finals.push(quote! { #field_ident: ::core::default::Default::default() });
            continue;
        }

        attrs.require_json_deserialize_bound(&mut r#gen, ty);
        if attrs.flatten {
            attrs.require_json_serialize_bound(&mut r#gen, ty);
        }

        let var_ident = format_ident!("__norito_field_{}", field_ident);
        init_stmts.push(quote! {
            let mut #var_ident: ::core::option::Option<#ty> = ::core::option::Option::None;
        });

        if attrs.flatten {
            let parse_expr = attrs
                .deserialize_from_value(ty, quote!(norito::json::Value::Object(__map.clone())));
            flatten_stmts.push(quote! {
                let parsed = #parse_expr;
                let __used = norito::json::to_value(&parsed)?;
                if let norito::json::Value::Object(__used_map) = __used {
                    for __key in __used_map.keys() {
                        __map.remove(__key);
                    }
                } else {
                    return Err(norito::Error::Message(
                        "#[norito(flatten)] field must deserialize to an object".into(),
                    ));
                }
                #var_ident = ::core::option::Option::Some(parsed);
            });
        } else {
            let key = container_attrs.rename_field(field_ident, &attrs);
            let key_lit = syn::LitStr::new(&key, proc_macro2::Span::call_site());
            let parse_expr = attrs.deserialize_from_value(ty, quote!(value));
            parse_stmts.push(quote! {
                if let ::core::option::Option::Some(value) = __map.remove(#key_lit) {
                    let parsed = #parse_expr;
                    #var_ident = ::core::option::Option::Some(parsed);
                }
            });
        }

        let missing_key = container_attrs.rename_field(field_ident, &attrs);
        let missing_msg = syn::LitStr::new(
            &format!("missing field `{missing_key}`"),
            proc_macro2::Span::call_site(),
        );
        let default_expr = if let Some(path) = &attrs.default_fn {
            Some(quote! { (#path)() })
        } else if attrs.default {
            add_bound(&mut r#gen, &field.ty, quote!(::core::default::Default));
            Some(quote! { ::core::default::Default::default() })
        } else {
            None
        };
        if is_option_type(&field.ty) {
            if let Some(expr) = default_expr {
                finals.push(quote! { #field_ident: #var_ident.unwrap_or_else(|| #expr) });
            } else {
                finals.push(quote! {
                    #field_ident: #var_ident.unwrap_or(::core::option::Option::None)
                });
            }
        } else if let Some(expr) = default_expr {
            finals.push(quote! { #field_ident: #var_ident.unwrap_or_else(|| #expr) });
        } else {
            finals.push(quote! {
                #field_ident: #var_ident
                    .ok_or_else(|| norito::Error::Message(#missing_msg.into()))?
            });
        }
    }

    let (impl_generics, ty_generics, where_clause) = r#gen.split_for_impl();
    Ok(quote! {
        impl<'a> #impl_generics norito::json::FastFromJson<'a> for #ident #ty_generics #where_clause {
            fn parse<'arena>(
                w: &mut norito::json::TapeWalker<'a>,
                _arena: &'arena mut norito::json::Arena,
            ) -> ::core::result::Result<Self, norito::Error> {
                let mut parser = norito::json::Parser::new_at(w.input(), w.raw_pos());
                let value = norito::json::Value::json_deserialize(&mut parser)?;
                w.sync_to_raw(parser.position());
                let mut __map = match value {
                    norito::json::Value::Object(map) => map,
                    _ => {
                        return Err(norito::Error::Message("expected JSON object".into()));
                    }
                };
                #(#init_stmts)*
                #(#parse_stmts)*
                #(#flatten_stmts)*
                Ok(Self { #(#finals),* })
            }
        }
    })
}

#[proc_macro_derive(FastJson, attributes(norito))]
pub fn derive_fast_json(input: TokenStream) -> TokenStream {
    let DeriveInput {
        ident,
        data,
        attrs,
        generics,
        ..
    } = parse_macro_input!(input);
    let container_attrs = match ContainerAttr::parse(&attrs) {
        Ok(attrs) => attrs,
        Err(err) => return err.to_compile_error().into(),
    };
    let body = match data {
        Data::Struct(ds) => {
            if let Err(e) = validate_field_attrs(&ds.fields) {
                return e.to_compile_error().into();
            }
            match ds.fields {
                Fields::Named(named) => {
                    if named
                        .named
                        .iter()
                        .any(|f| FieldAttr::parse(&f.attrs).flatten)
                    {
                        match derive_fast_json_struct_flatten(
                            &ident,
                            &generics,
                            &named,
                            &container_attrs,
                        ) {
                            Ok(tokens) => tokens,
                            Err(err) => return err.to_compile_error().into(),
                        }
                    } else {
                        // Hashed-key dispatch using TapeWalker::read_key_hash with
                        // last_key() collision guard. This avoids building a temporary
                        // String for the key and speeds up large object decoding.

                        let mut inits = Vec::new();
                        let mut cases = Vec::new();
                        let mut finals = Vec::new();

                        for (idx, f) in named.named.iter().enumerate() {
                            let name = f.ident.as_ref().unwrap();
                            let attrs = FieldAttr::parse(&f.attrs);
                            let key = container_attrs.rename_field(name, &attrs);
                            let key_lit = syn::LitStr::new(&key, proc_macro2::Span::call_site());
                            // Precompute 64-bit key hash at compile-time to match TapeWalker
                            let key_hash_expr: syn::Expr =
                                syn::parse_quote! { norito::json::key_hash_const(#key_lit) };

                            inits.push(quote! { let mut #name = None; });

                            let bitmask: u128 = 1u128 << idx;
                            let parse_body = match &f.ty {
                                syn::Type::Path(tp)
                                    if tp.path.segments.last().map(|s| s.ident.to_string())
                                        == Some("String".into()) =>
                                {
                                    quote! {
                                        // Inline string parse via TapeWalker
                                        let sref = w.parse_string_ref_inline(arena)?;
                                        if (__seen & #bitmask) != 0 { return Err(norito::Error::Message(concat!("duplicate field `", #key_lit, "`").into())); }
                                        __seen |= #bitmask;
                                        #name = Some(sref.to_string());
                                    }
                                }
                                syn::Type::Reference(rf) => {
                                    let is_str = matches!(&*rf.elem, syn::Type::Path(tp2) if tp2.path.is_ident("str"));
                                    if is_str {
                                        quote! {
                                            let sref = w.parse_string_ref_inline(arena)?;
                                            let v: &str = match sref { norito::json::StrRef::Borrowed(s) => s, norito::json::StrRef::Owned(s) => s };
                                            if (__seen & #bitmask) != 0 { return Err(norito::Error::Message(concat!("duplicate field `", #key_lit, "`").into())); }
                                            __seen |= #bitmask;
                                            #name = Some(v);
                                        }
                                    } else {
                                        quote! {
                                            let s_in = w.input();
                                            let mut p = norito::json::Parser::new_at(s_in, w.raw_pos());
                                            let v: #rf = <#rf as norito::json::JsonDeserialize>::json_deserialize(&mut p)?;
                                            w.sync_to_raw(p.position());
                                            if (__seen & #bitmask) != 0 { return Err(norito::Error::Message(concat!("duplicate field `", #key_lit, "`").into())); }
                                            __seen |= #bitmask;
                                            #name = Some(v);
                                        }
                                    }
                                }
                                syn::Type::Path(tp)
                                    if tp.path.segments.last().map(|s| s.ident.to_string())
                                        == Some("bool".into()) =>
                                {
                                    quote! {
                                        let v = w.parse_bool_inline()?;
                                        if (__seen & #bitmask) != 0 { return Err(norito::Error::Message(concat!("duplicate field `", #key_lit, "`").into())); }
                                        __seen |= #bitmask;
                                        #name = Some(v);
                                    }
                                }
                                syn::Type::Path(tp)
                                    if tp.path.segments.last().map(|s| s.ident.to_string())
                                        == Some("u64".into()) =>
                                {
                                    quote! {
                                        let v = w.parse_u64_inline()?;
                                        if (__seen & #bitmask) != 0 { return Err(norito::Error::Message(concat!("duplicate field `", #key_lit, "`").into())); }
                                        __seen |= #bitmask;
                                        #name = Some(v);
                                    }
                                }
                                syn::Type::Path(tp)
                                    if tp.path.segments.last().map(|s| s.ident.to_string())
                                        == Some("u32".into()) =>
                                {
                                    quote! {
                                        let v64 = w.parse_u64_inline()?;
                                        let v = u32::try_from(v64).map_err(|_| norito::Error::Message("u32 overflow".into()))?;
                                        if (__seen & #bitmask) != 0 { return Err(norito::Error::Message(concat!("duplicate field `", #key_lit, "`").into())); }
                                        __seen |= #bitmask;
                                        #name = Some(v);
                                    }
                                }
                                // Inline Vec<T> fast path using TapeWalker
                                syn::Type::Path(tp)
                                    if tp.path.segments.last().map(|s| s.ident.to_string())
                                        == Some("Vec".into()) =>
                                {
                                    // Extract inner type T from Vec<T>
                                    let inner_ty = match &tp.path.segments.last().unwrap().arguments
                                    {
                                        syn::PathArguments::AngleBracketed(ab) => {
                                            ab.args.first().cloned()
                                        }
                                        _ => None,
                                    };
                                    if let Some(syn::GenericArgument::Type(it)) = inner_ty {
                                        // Generate specialized loops for a few primitives; else defer to FastFromJson
                                        let is_u64 = matches!(&it, syn::Type::Path(p) if p.path.segments.last().unwrap().ident == "u64");
                                        let is_u32 = matches!(&it, syn::Type::Path(p) if p.path.segments.last().unwrap().ident == "u32");
                                        let is_u128 = matches!(&it, syn::Type::Path(p) if p.path.segments.last().unwrap().ident == "u128");
                                        let is_i64 = matches!(&it, syn::Type::Path(p) if p.path.segments.last().unwrap().ident == "i64");
                                        let is_f64 = matches!(&it, syn::Type::Path(p) if p.path.segments.last().unwrap().ident == "f64");
                                        let is_bool = matches!(&it, syn::Type::Path(p) if p.path.segments.last().unwrap().ident == "bool");
                                        let is_string = matches!(&it, syn::Type::Path(p) if p.path.segments.last().unwrap().ident == "String");
                                        if is_u64 {
                                            quote! {
                                                // Expect array start
                                                w.skip_ws();
                                                if let Some((off, b'[')) = w.peek_struct() { let _ = w.next_struct(); w.sync_to_raw(off + 1); } else { return Err(norito::Error::Message("expected array".into())); }
                                                let mut tmp: ::std::vec::Vec<u64> = ::std::vec::Vec::new();
                                                loop {
                                                    w.skip_ws();
                                                    let __raw = w.raw_pos();
                                                    let __bytes = w.input().as_bytes();
                                                    if __raw < __bytes.len() && __bytes[__raw] == b']' {
                                                        if let Some((off, ch)) = w.peek_struct() {
                                                            if ch == b']' && off == __raw { let _ = w.next_struct(); w.sync_to_raw(off + 1); } else { w.sync_to_raw(__raw + 1); }
                                                        } else { w.sync_to_raw(__raw + 1); }
                                                        break;
                                                    }
                                                    let v = w.parse_u64_inline()?; tmp.push(v);
                                                    let _ = w.consume_comma_if_present()?;
                                                }
                                                if (__seen & #bitmask) != 0 { return Err(norito::Error::Message(concat!("duplicate field `", #key_lit, "`").into())); }
                                                __seen |= #bitmask; #name = Some(tmp);
                                            }
                                        } else if is_u32 {
                                            quote! {
                                                w.skip_ws();
                                                if let Some((off, b'[')) = w.peek_struct() { let _ = w.next_struct(); w.sync_to_raw(off + 1); } else { return Err(norito::Error::Message("expected array".into())); }
                                                let mut tmp: ::std::vec::Vec<u32> = ::std::vec::Vec::new();
                                                loop {
                                                    w.skip_ws();
                                                    let __raw = w.raw_pos();
                                                    let __bytes = w.input().as_bytes();
                                                    if __raw < __bytes.len() && __bytes[__raw] == b']' {
                                                        if let Some((off, ch)) = w.peek_struct() {
                                                            if ch == b']' && off == __raw { let _ = w.next_struct(); w.sync_to_raw(off + 1); } else { w.sync_to_raw(__raw + 1); }
                                                        } else { w.sync_to_raw(__raw + 1); }
                                                        break;
                                                    }
                                                    let v64 = w.parse_u64_inline()?; let v = u32::try_from(v64).map_err(|_| norito::Error::Message("u32 overflow".into()))?; tmp.push(v);
                                                    let _ = w.consume_comma_if_present()?;
                                                }
                                                if (__seen & #bitmask) != 0 { return Err(norito::Error::Message(concat!("duplicate field `", #key_lit, "`").into())); }
                                                __seen |= #bitmask; #name = Some(tmp);
                                            }
                                        } else if is_u128 {
                                            quote! {
                                                w.skip_ws();
                                                if let Some((off, b'[')) = w.peek_struct() { let _ = w.next_struct(); w.sync_to_raw(off + 1); } else { return Err(norito::Error::Message("expected array".into())); }
                                                let mut tmp: ::std::vec::Vec<u128> = ::std::vec::Vec::new();
                                                loop {
                                                    w.skip_ws();
                                                    let __raw = w.raw_pos();
                                                    let __bytes = w.input().as_bytes();
                                                    if __raw < __bytes.len() && __bytes[__raw] == b']' {
                                                        if let Some((off, ch)) = w.peek_struct() {
                                                            if ch == b']' && off == __raw { let _ = w.next_struct(); w.sync_to_raw(off + 1); } else { w.sync_to_raw(__raw + 1); }
                                                        } else { w.sync_to_raw(__raw + 1); }
                                                        break;
                                                    }
                                                    let v = w.parse_u128_inline()?; tmp.push(v);
                                                    let _ = w.consume_comma_if_present()?;
                                                }
                                                if (__seen & #bitmask) != 0 { return Err(norito::Error::Message(concat!("duplicate field `", #key_lit, "`").into())); }
                                                __seen |= #bitmask; #name = Some(tmp);
                                            }
                                        } else if is_i64 {
                                            quote! {
                                                w.skip_ws();
                                                if let Some((off, b'[')) = w.peek_struct() { let _ = w.next_struct(); w.sync_to_raw(off + 1); } else { return Err(norito::Error::Message("expected array".into())); }
                                                let mut tmp: ::std::vec::Vec<i64> = ::std::vec::Vec::new();
                                                loop {
                                                    w.skip_ws();
                                                    let __raw = w.raw_pos();
                                                    let __bytes = w.input().as_bytes();
                                                    if __raw < __bytes.len() && __bytes[__raw] == b']' {
                                                        if let Some((off, ch)) = w.peek_struct() {
                                                            if ch == b']' && off == __raw { let _ = w.next_struct(); w.sync_to_raw(off + 1); } else { w.sync_to_raw(__raw + 1); }
                                                        } else { w.sync_to_raw(__raw + 1); }
                                                        break;
                                                    }
                                                    let v = w.parse_i64_inline()?; tmp.push(v);
                                                    let _ = w.consume_comma_if_present()?;
                                                }
                                                if (__seen & #bitmask) != 0 { return Err(norito::Error::Message(concat!("duplicate field `", #key_lit, "`").into())); }
                                                __seen |= #bitmask; #name = Some(tmp);
                                            }
                                        } else if is_f64 {
                                            quote! {
                                                w.skip_ws();
                                                if let Some((off, b'[')) = w.peek_struct() { let _ = w.next_struct(); w.sync_to_raw(off + 1); } else { return Err(norito::Error::Message("expected array".into())); }
                                                let mut tmp: ::std::vec::Vec<f64> = ::std::vec::Vec::new();
                                                loop {
                                                    w.skip_ws();
                                                    let __raw = w.raw_pos();
                                                    let __bytes = w.input().as_bytes();
                                                    if __raw < __bytes.len() && __bytes[__raw] == b']' {
                                                        if let Some((off, ch)) = w.peek_struct() {
                                                            if ch == b']' && off == __raw { let _ = w.next_struct(); w.sync_to_raw(off + 1); } else { w.sync_to_raw(__raw + 1); }
                                                        } else { w.sync_to_raw(__raw + 1); }
                                                        break;
                                                    }
                                                    let v = w.parse_f64_inline()?; tmp.push(v);
                                                    let _ = w.consume_comma_if_present()?;
                                                }
                                                if (__seen & #bitmask) != 0 { return Err(norito::Error::Message(concat!("duplicate field `", #key_lit, "`").into())); }
                                                __seen |= #bitmask; #name = Some(tmp);
                                            }
                                        } else if is_bool {
                                            quote! {
                                                w.skip_ws();
                                                if let Some((off, b'[')) = w.peek_struct() { let _ = w.next_struct(); w.sync_to_raw(off + 1); } else { return Err(norito::Error::Message("expected array".into())); }
                                                let mut tmp: ::std::vec::Vec<bool> = ::std::vec::Vec::new();
                                                loop {
                                                    w.skip_ws();
                                                    let __raw = w.raw_pos();
                                                    let __bytes = w.input().as_bytes();
                                                    if __raw < __bytes.len() && __bytes[__raw] == b']' {
                                                        if let Some((off, ch)) = w.peek_struct() {
                                                            if ch == b']' && off == __raw { let _ = w.next_struct(); w.sync_to_raw(off + 1); } else { w.sync_to_raw(__raw + 1); }
                                                        } else { w.sync_to_raw(__raw + 1); }
                                                        break;
                                                    }
                                                    let v = w.parse_bool_inline()?; tmp.push(v);
                                                    let _ = w.consume_comma_if_present()?;
                                                }
                                                if (__seen & #bitmask) != 0 { return Err(norito::Error::Message(concat!("duplicate field `", #key_lit, "`").into())); }
                                                __seen |= #bitmask; #name = Some(tmp);
                                            }
                                        } else if is_string {
                                            quote! {
                                                w.skip_ws();
                                                if let Some((off, b'[')) = w.peek_struct() { let _ = w.next_struct(); w.sync_to_raw(off + 1); } else { return Err(norito::Error::Message("expected array".into())); }
                                                let mut tmp: ::std::vec::Vec<String> = ::std::vec::Vec::new();
                                                loop {
                                                    w.skip_ws();
                                                    let __raw = w.raw_pos();
                                                    let __bytes = w.input().as_bytes();
                                                    if __raw < __bytes.len() && __bytes[__raw] == b']' {
                                                        if let Some((off, ch)) = w.peek_struct() {
                                                            if ch == b']' && off == __raw { let _ = w.next_struct(); w.sync_to_raw(off + 1); } else { w.sync_to_raw(__raw + 1); }
                                                        } else { w.sync_to_raw(__raw + 1); }
                                                        break;
                                                    }
                                                    let sref = w.parse_string_ref_inline(arena)?; tmp.push(sref.to_string());
                                                    let _ = w.consume_comma_if_present()?;
                                                }
                                                if (__seen & #bitmask) != 0 { return Err(norito::Error::Message(concat!("duplicate field `", #key_lit, "`").into())); }
                                                __seen |= #bitmask; #name = Some(tmp);
                                            }
                                        } else {
                                            // Assume FastFromJson for inner type
                                            quote! {
                                                w.skip_ws();
                                                if let Some((off, b'[')) = w.peek_struct() { let _ = w.next_struct(); w.sync_to_raw(off + 1); } else { return Err(norito::Error::Message("expected array".into())); }
                                                let mut tmp: ::std::vec::Vec<#it> = ::std::vec::Vec::new();
                                                loop {
                                                    w.skip_ws();
                                                    let __raw = w.raw_pos();
                                                    let __bytes = w.input().as_bytes();
                                                    if __raw < __bytes.len() && __bytes[__raw] == b']' {
                                                        if let Some((off, ch)) = w.peek_struct() {
                                                            if ch == b']' && off == __raw { let _ = w.next_struct(); w.sync_to_raw(off + 1); } else { w.sync_to_raw(__raw + 1); }
                                                        } else { w.sync_to_raw(__raw + 1); }
                                                        break;
                                                    }
                                                    let v = <#it as norito::json::FastFromJson>::parse(w, arena)?; tmp.push(v);
                                                    let _ = w.consume_comma_if_present()?;
                                                }
                                                if (__seen & #bitmask) != 0 { return Err(norito::Error::Message(concat!("duplicate field `", #key_lit, "`").into())); }
                                                __seen |= #bitmask; #name = Some(tmp);
                                            }
                                        }
                                    } else {
                                        // Fallback to Parser when Vec<T> cannot be analyzed
                                        quote! {
                                            let s_in = w.input();
                                            let mut p = norito::json::Parser::new_at(s_in, w.raw_pos());
                                            let v: #tp = <#tp as norito::json::JsonDeserialize>::json_deserialize(&mut p)?;
                                            w.sync_to_raw(p.position());
                                            if (__seen & #bitmask) != 0 { return Err(norito::Error::Message(concat!("duplicate field `", #key_lit, "`").into())); }
                                            __seen |= #bitmask;
                                            #name = Some(v);
                                        }
                                    }
                                }
                                syn::Type::Path(tp)
                                    if tp.path.segments.last().map(|s| s.ident.to_string())
                                        == Some("StrRef".into()) =>
                                {
                                    quote! {
                                        let sref = w.parse_string_ref_inline(arena)?;
                                        if (__seen & #bitmask) != 0 { return Err(norito::Error::Message(concat!("duplicate field `", #key_lit, "`").into())); }
                                        __seen |= #bitmask;
                                        #name = Some(sref);
                                    }
                                }
                                // Special-case Option<primitive> to avoid Parser
                                syn::Type::Path(tp)
                                    if tp.path.segments.last().map(|s| s.ident.to_string())
                                        == Some("Option".into()) =>
                                {
                                    // Extract inner type for a few primitives we fast-path
                                    let inner = match &tp.path.segments.last().unwrap().arguments {
                                        syn::PathArguments::AngleBracketed(ab) => {
                                            if ab.args.len() == 1 {
                                                Some(ab.args.first().unwrap())
                                            } else {
                                                None
                                            }
                                        }
                                        _ => None,
                                    };
                                    if let Some(syn::GenericArgument::Type(inner_ty)) = inner {
                                        if let syn::Type::Path(itp) = inner_ty {
                                            let id =
                                                itp.path.segments.last().unwrap().ident.to_string();
                                            if id == "u64" {
                                                quote! {
                                                    // null => None; else parse u64
                                                    w.skip_ws();
                                                    let bytes = w.input().as_bytes();
                                                    if w.raw_pos() + 4 <= bytes.len() && &bytes[w.raw_pos()..w.raw_pos()+4] == b"null" {
                                                        w.sync_to_raw(w.raw_pos() + 4);
                                                        if (__seen & #bitmask) != 0 { return Err(norito::Error::Message(concat!("duplicate field `", #key_lit, "`").into())); }
                                                        __seen |= #bitmask;
                                                        #name = Some(None);
                                                    } else {
                                                        let v = w.parse_u64_inline()?;
                                                        if (__seen & #bitmask) != 0 { return Err(norito::Error::Message(concat!("duplicate field `", #key_lit, "`").into())); }
                                                        __seen |= #bitmask;
                                                        #name = Some(Some(v));
                                                    }
                                                }
                                            } else if id == "u32" {
                                                quote! {
                                                    w.skip_ws();
                                                    let bytes = w.input().as_bytes();
                                                    if w.raw_pos() + 4 <= bytes.len() && &bytes[w.raw_pos()..w.raw_pos()+4] == b"null" {
                                                        w.sync_to_raw(w.raw_pos() + 4);
                                                        if (__seen & #bitmask) != 0 { return Err(norito::Error::Message(concat!("duplicate field `", #key_lit, "`").into())); }
                                                        __seen |= #bitmask;
                                                        #name = Some(None);
                                                    } else {
                                                        let v64 = w.parse_u64_inline()?;
                                                        let v = u32::try_from(v64).map_err(|_| norito::Error::Message("u32 overflow".into()))?;
                                                        if (__seen & #bitmask) != 0 { return Err(norito::Error::Message(concat!("duplicate field `", #key_lit, "`").into())); }
                                                        __seen |= #bitmask;
                                                        #name = Some(Some(v));
                                                    }
                                                }
                                            } else if id == "u128" {
                                                quote! {
                                                    w.skip_ws();
                                                    let bytes = w.input().as_bytes();
                                                    if w.raw_pos() + 4 <= bytes.len() && &bytes[w.raw_pos()..w.raw_pos()+4] == b"null" {
                                                        w.sync_to_raw(w.raw_pos() + 4);
                                                        if (__seen & #bitmask) != 0 { return Err(norito::Error::Message(concat!("duplicate field `", #key_lit, "`").into())); }
                                                        __seen |= #bitmask;
                                                        #name = Some(None);
                                                    } else {
                                                        let v = w.parse_u128_inline()?;
                                                        if (__seen & #bitmask) != 0 { return Err(norito::Error::Message(concat!("duplicate field `", #key_lit, "`").into())); }
                                                        __seen |= #bitmask;
                                                        #name = Some(Some(v));
                                                    }
                                                }
                                            } else if id == "i64" {
                                                quote! {
                                                    w.skip_ws();
                                                    let bytes = w.input().as_bytes();
                                                    if w.raw_pos() + 4 <= bytes.len() && &bytes[w.raw_pos()..w.raw_pos()+4] == b"null" {
                                                        w.sync_to_raw(w.raw_pos() + 4);
                                                        if (__seen & #bitmask) != 0 { return Err(norito::Error::Message(concat!("duplicate field `", #key_lit, "`").into())); }
                                                        __seen |= #bitmask;
                                                        #name = Some(None);
                                                    } else {
                                                        let v = w.parse_i64_inline()?;
                                                        if (__seen & #bitmask) != 0 { return Err(norito::Error::Message(concat!("duplicate field `", #key_lit, "`").into())); }
                                                        __seen |= #bitmask;
                                                        #name = Some(Some(v));
                                                    }
                                                }
                                            } else if id == "f64" {
                                                quote! {
                                                    w.skip_ws();
                                                    let bytes = w.input().as_bytes();
                                                    if w.raw_pos() + 4 <= bytes.len() && &bytes[w.raw_pos()..w.raw_pos()+4] == b"null" {
                                                        w.sync_to_raw(w.raw_pos() + 4);
                                                        if (__seen & #bitmask) != 0 { return Err(norito::Error::Message(concat!("duplicate field `", #key_lit, "`").into())); }
                                                        __seen |= #bitmask;
                                                        #name = Some(None);
                                                    } else {
                                                        let v = w.parse_f64_inline()?;
                                                        if (__seen & #bitmask) != 0 { return Err(norito::Error::Message(concat!("duplicate field `", #key_lit, "`").into())); }
                                                        __seen |= #bitmask;
                                                        #name = Some(Some(v));
                                                    }
                                                }
                                            } else if id == "bool" {
                                                quote! {
                                                    w.skip_ws();
                                                    let bytes = w.input().as_bytes();
                                                    if w.raw_pos() + 4 <= bytes.len() && &bytes[w.raw_pos()..w.raw_pos()+4] == b"null" {
                                                        w.sync_to_raw(w.raw_pos() + 4);
                                                        if (__seen & #bitmask) != 0 { return Err(norito::Error::Message(concat!("duplicate field `", #key_lit, "`").into())); }
                                                        __seen |= #bitmask;
                                                        #name = Some(None);
                                                    } else {
                                                        let v = w.parse_bool_inline()?;
                                                        if (__seen & #bitmask) != 0 { return Err(norito::Error::Message(concat!("duplicate field `", #key_lit, "`").into())); }
                                                        __seen |= #bitmask;
                                                        #name = Some(Some(v));
                                                    }
                                                }
                                            } else if id == "String" {
                                                quote! {
                                                    w.skip_ws();
                                                    let bytes = w.input().as_bytes();
                                                    if w.raw_pos() + 4 <= bytes.len() && &bytes[w.raw_pos()..w.raw_pos()+4] == b"null" {
                                                        w.sync_to_raw(w.raw_pos() + 4);
                                                        if (__seen & #bitmask) != 0 { return Err(norito::Error::Message(concat!("duplicate field `", #key_lit, "`").into())); }
                                                        __seen |= #bitmask;
                                                        #name = Some(None);
                                                    } else {
                                                        let sref = w.parse_string_ref_inline(arena)?;
                                                        if (__seen & #bitmask) != 0 { return Err(norito::Error::Message(concat!("duplicate field `", #key_lit, "`").into())); }
                                                        __seen |= #bitmask;
                                                        #name = Some(Some(sref.to_string()));
                                                    }
                                                }
                                            } else {
                                                // Fallback generic Option<T> via Parser
                                                quote! {
                                                    let s_in = w.input();
                                                    let mut p = norito::json::Parser::new_at(s_in, w.raw_pos());
                                                    let v: #tp = <#tp as norito::json::JsonDeserialize>::json_deserialize(&mut p)?;
                                                    w.sync_to_raw(p.position());
                                                    if (__seen & #bitmask) != 0 { return Err(norito::Error::Message(concat!("duplicate field `", #key_lit, "`").into())); }
                                                    __seen |= #bitmask;
                                                    #name = Some(v);
                                                }
                                            }
                                        } else {
                                            // Inner type isn't a Path; fallback to generic parser for the Option field
                                            quote! {
                                                let s_in = w.input();
                                                let mut p = norito::json::Parser::new_at(s_in, w.raw_pos());
                                                let v: #tp = <#tp as norito::json::JsonDeserialize>::json_deserialize(&mut p)?;
                                                w.sync_to_raw(p.position());
                                                if (__seen & #bitmask) != 0 { return Err(norito::Error::Message(concat!("duplicate field `", #key_lit, "`").into())); }
                                                __seen |= #bitmask;
                                                #name = Some(v);
                                            }
                                        }
                                    } else {
                                        // No inner type found; fallback to generic parser
                                        quote! {
                                            let s_in = w.input();
                                            let mut p = norito::json::Parser::new_at(s_in, w.raw_pos());
                                            let v: #tp = <#tp as norito::json::JsonDeserialize>::json_deserialize(&mut p)?;
                                            w.sync_to_raw(p.position());
                                            if (__seen & #bitmask) != 0 { return Err(norito::Error::Message(concat!("duplicate field `", #key_lit, "`").into())); }
                                            __seen |= #bitmask;
                                            #name = Some(v);
                                        }
                                    }
                                }
                                _ => {
                                    let ty = &f.ty;
                                    quote! {
                                        let s_in = w.input();
                                        let mut p = norito::json::Parser::new_at(s_in, w.raw_pos());
                                        let v: #ty = <#ty as norito::json::JsonDeserialize>::json_deserialize(&mut p)?;
                                        w.sync_to_raw(p.position());
                                        if (__seen & #bitmask) != 0 { return Err(norito::Error::Message(concat!("duplicate field `", #key_lit, "`").into())); }
                                        __seen |= #bitmask;
                                        #name = Some(v);
                                    }
                                }
                            };

                            let default_expr = if let Some(path) = &attrs.default_fn {
                                quote! { #path() }
                            } else {
                                quote! { ::core::default::Default::default() }
                            };

                            cases.push(quote! {
                                x if x == #key_hash_expr => {
                                    if w.last_key() == #key_lit {
                                        #parse_body
                                    } else {
                                        // Hash collision: skip this value deterministically
                                        w.skip_value()?;
                                    }
                                }
                            });
                            // Required vs optional: for Option<T> fields, absence should map to None
                            let is_option = matches!(&f.ty, syn::Type::Path(tp) if tp.path.segments.last().map(|s| s.ident == "Option").unwrap_or(false));
                            if attrs.default {
                                finals
                                    .push(quote! { #name: #name.unwrap_or_else(|| #default_expr) });
                            } else if is_option {
                                finals.push(quote! { #name: #name.unwrap_or(None) });
                            } else {
                                finals.push(quote! { #name: #name.ok_or_else(|| norito::Error::Message(format!("missing field `{}`", stringify!(#name))))? });
                            }
                        }

                        quote! {
                            impl<'a> norito::json::FastFromJson<'a> for #ident {
                                fn parse<'arena>(w: &mut norito::json::TapeWalker<'a>, arena: &'arena mut norito::json::Arena) -> ::core::result::Result<Self, norito::Error> {
                                    w.expect_object_start()?;
                                    #(#inits)*
                                    let mut __seen: u128 = 0;
                                    while !w.peek_object_end()? {
                                        // Read hashed key via TapeWalker, then dispatch.
                                        let __kh = w.read_key_hash()?;
                                        w.expect_colon_resync()?;
                                        match __kh {
                                            #(#cases),*,
                                            _ => {
                                                // Unknown key: skip its value
                                                w.skip_value()?;
                                            }
                                        }
                                        let _ = w.consume_comma_if_present()?;
                                    }
                                    w.expect_object_end()?;
                                    Ok(Self { #(#finals),* })
                                }
                            }
                        }
                    }
                }
                _ => quote! { compile_error!("FastJson only supports named structs"); },
            }
        }
        Data::Enum(de) => {
            let enum_attr = match EnumAttr::parse(&attrs) {
                Ok(attr) => attr,
                Err(err) => return err.to_compile_error().into(),
            };
            let tag = match enum_attr.tag {
                Some(t) => t,
                None => {
                    return syn::Error::new_spanned(
                        &ident,
                        "FastJson enum support currently requires #[norito(tag = ...)]",
                    )
                    .to_compile_error()
                    .into();
                }
            };
            let content = match enum_attr.content {
                Some(c) => c,
                None => {
                    return syn::Error::new_spanned(
                        &ident,
                        "FastJson enum support currently requires #[norito(content = ...)]",
                    )
                    .to_compile_error()
                    .into();
                }
            };

            let tag_lit = syn::LitStr::new(&tag, proc_macro2::Span::call_site());
            let content_lit = syn::LitStr::new(&content, proc_macro2::Span::call_site());
            let tag_hash_expr: syn::Expr =
                syn::parse_quote! { norito::json::key_hash_const(#tag_lit) };
            let content_hash_expr: syn::Expr =
                syn::parse_quote! { norito::json::key_hash_const(#content_lit) };

            let mut tag_match_arms = Vec::new();
            let mut parse_arms = Vec::new();

            for (idx, variant) in de.variants.iter().enumerate() {
                if let Err(err) = VariantAttr::parse(&variant.attrs) {
                    return err.to_compile_error().into();
                }
                let idx_lit = syn::LitInt::new(&idx.to_string(), proc_macro2::Span::call_site());
                let v_ident = &variant.ident;
                let v_attr = VariantAttr::parse(&variant.attrs).expect("validated above");
                let variant_name = container_attrs.rename_variant(v_ident, &v_attr);
                let variant_lit = syn::LitStr::new(&variant_name, proc_macro2::Span::call_site());

                tag_match_arms.push(quote! { #variant_lit => #idx_lit as u8, });

                match &variant.fields {
                    Fields::Unit => {
                        parse_arms.push(quote! {
                            #idx_lit => {
                                let mut __parser = norito::json::Parser::new(__norito_content_slice);
                                __parser.parse_null()?;
                                __parser.skip_ws();
                                if !__parser.eof() {
                                    Err(norito::Error::Message(format!(
                                        "unexpected content for unit variant `{}`",
                                        #variant_lit
                                    ).into()))
                                } else {
                                    Ok(Self::#v_ident)
                                }
                            }
                        });
                    }
                    Fields::Unnamed(fields) => {
                        for field in fields.unnamed.iter() {
                            let attrs = FieldAttr::parse(&field.attrs);
                            if let Some(err) = attrs.error {
                                return err.to_compile_error().into();
                            }
                            if attrs.skip {
                                return syn::Error::new_spanned(
                                    field,
                                    "#[norito(skip)] is not supported on enum tuple variants",
                                )
                                .to_compile_error()
                                .into();
                            }
                            if attrs.flatten {
                                return syn::Error::new_spanned(
                                    field,
                                    "#[norito(flatten)] is not supported on enum tuple variants",
                                )
                                .to_compile_error()
                                .into();
                            }
                        }

                        if fields.unnamed.len() == 1 {
                            let field = &fields.unnamed[0];
                            let attrs = FieldAttr::parse(&field.attrs);
                            let ty = &field.ty;
                            let deserialize_call =
                                attrs.deserializer_call(ty, quote!(&mut __parser));
                            parse_arms.push(quote! {
                                #idx_lit => {
                                    let mut __parser = norito::json::Parser::new(__norito_content_slice);
                                    let value = #deserialize_call;
                                    __parser.skip_ws();
                                    if !__parser.eof() {
                                        Err(norito::Error::Message(format!(
                                            "unexpected trailing data for variant `{}`",
                                            #variant_lit
                                        ).into()))
                                    } else {
                                        Ok(Self::#v_ident(value))
                                    }
                                }
                            });
                        } else {
                            let mut inits = Vec::new();
                            let mut match_tokens = Vec::new();
                            let mut finals = Vec::new();
                            for (tuple_idx, field) in fields.unnamed.iter().enumerate() {
                                let attrs = FieldAttr::parse(&field.attrs);
                                let ty = &field.ty;
                                let binding = format_ident!("__norito_variant_{tuple_idx}");
                                inits.push(quote! {
                                    let mut #binding: ::core::option::Option<#ty> = ::core::option::Option::None;
                                });
                                let missing_text = format!(
                                    "missing tuple index {tuple_idx} for variant `{variant_name}`"
                                );
                                let missing_msg =
                                    syn::LitStr::new(&missing_text, proc_macro2::Span::call_site());
                                let deserialize_call =
                                    attrs.deserializer_call(ty, quote!(&mut __parser));
                                let tuple_idx_lit = syn::LitInt::new(
                                    &tuple_idx.to_string(),
                                    proc_macro2::Span::call_site(),
                                );
                                match_tokens.push(quote! {
                                    #tuple_idx_lit => {
                                        let value = #deserialize_call;
                                        #binding = ::core::option::Option::Some(value);
                                    }
                                });
                                finals.push(quote! {
                                    #binding.ok_or_else(|| norito::Error::Message(#missing_msg.into()))?
                                });
                            }
                            let constructor = quote! { Self::#v_ident( #( #finals ),* ) };
                            parse_arms.push(quote! {
                                #idx_lit => {
                                    let mut __parser = norito::json::Parser::new(__norito_content_slice);
                                    __parser.skip_ws();
                                    __parser.expect(b'[')?;
                                    __parser.skip_ws();
                                    #(#inits)*
                                    let mut __idx = 0usize;
                                    if !__parser.try_consume_char(b']')? {
                                        let mut __first = true;
                                        loop {
                                            if !__first {
                                                __parser.expect(b',')?;
                                            }
                                            __first = false;
                                            match __idx {
                                                #( #match_tokens ),*,
                                                _ => {
                                                    __parser.skip_value()?;
                                                    return Err(norito::Error::Message(format!(
                                                        "too many elements for variant `{}`",
                                                        #variant_lit
                                                    ).into()));
                                                }
                                            }
                                            __idx += 1;
                                            __parser.skip_ws();
                                            if __parser.try_consume_char(b']')? {
                                                break;
                                            }
                                        }
                                    }
                                    __parser.skip_ws();
                                    if !__parser.eof() {
                                        return Err(norito::Error::Message(format!(
                                            "unexpected trailing data for variant `{}`",
                                            #variant_lit
                                        ).into()));
                                    }
                                    Ok(#constructor)
                                }
                            });
                        }
                    }
                    Fields::Named(fields) => {
                        for field in fields.named.iter() {
                            let attrs = FieldAttr::parse(&field.attrs);
                            if let Some(err) = attrs.error {
                                return err.to_compile_error().into();
                            }
                            if attrs.skip {
                                return syn::Error::new_spanned(
                                    field,
                                    "#[norito(skip)] is not supported on enum struct variants",
                                )
                                .to_compile_error()
                                .into();
                            }
                            if attrs.flatten {
                                return syn::Error::new_spanned(
                                    field,
                                    "#[norito(flatten)] is not supported on enum struct variants",
                                )
                                .to_compile_error()
                                .into();
                            }
                        }

                        let mut inits = Vec::new();
                        let mut match_tokens = Vec::new();
                        let mut finals = Vec::new();
                        for field in fields.named.iter() {
                            let attrs = FieldAttr::parse(&field.attrs);
                            let field_ident = field.ident.as_ref().unwrap();
                            let key = container_attrs.rename_field(field_ident, &attrs);
                            let key_lit = syn::LitStr::new(&key, proc_macro2::Span::call_site());
                            let var_ident = format_ident!("__norito_variant_field_{}", field_ident);
                            let ty = &field.ty;
                            inits.push(quote! { let mut #var_ident: ::core::option::Option<#ty> = ::core::option::Option::None; });
                            let duplicate_text =
                                format!("duplicate field `{key}` in variant `{variant_name}`");
                            let duplicate_msg =
                                syn::LitStr::new(&duplicate_text, proc_macro2::Span::call_site());
                            match_tokens.push(quote! {
                                #key_lit => {
                                    if #var_ident.is_some() {
                                        return Err(norito::Error::Message(#duplicate_msg.into()));
                                    }
                                    let value = <#ty as norito::json::JsonDeserialize>::json_deserialize(&mut __parser)?;
                                    #var_ident = ::core::option::Option::Some(value);
                                }
                            });
                            let missing_text =
                                format!("missing field `{key}` in variant `{variant_name}`");
                            let missing_msg =
                                syn::LitStr::new(&missing_text, proc_macro2::Span::call_site());
                            finals.push(quote! {
                                #field_ident: #var_ident.ok_or_else(|| norito::Error::Message(#missing_msg.into()))?
                            });
                        }
                        parse_arms.push(quote! {
                            #idx_lit => {
                                let mut __parser = norito::json::Parser::new(__norito_content_slice);
                                __parser.skip_ws();
                                if !__parser.try_consume_char(b'{')? {
                                    return Err(norito::Error::Message(format!(
                                        "expected object for variant `{}`",
                                        #variant_lit
                                    ).into()));
                                }
                                #(#inits)*
                                __parser.skip_ws();
                                if !__parser.try_consume_char(b'}')? {
                                    loop {
                                        __parser.skip_ws();
                                        let key = __parser.parse_string()?;
                                        __parser.expect(b':')?;
                                        match key.as_str() {
                                            #( #match_tokens ),*,
                                            _ => {
                                                __parser.skip_value()?;
                                            }
                                        }
                                        __parser.skip_ws();
                                        if __parser.try_consume_char(b'}')? {
                                            break;
                                        }
                                        __parser.expect(b',')?;
                                    }
                                }
                                __parser.skip_ws();
                                if !__parser.eof() {
                                    return Err(norito::Error::Message(format!(
                                        "unexpected trailing data for variant `{}`",
                                        #variant_lit
                                    ).into()));
                                }
                                Ok(Self::#v_ident { #( #finals ),* })
                            }
                        });
                    }
                }
            }

            let unknown_tag_msg = syn::LitStr::new(
                &format!("unknown variant `{{}}` for {ident}"),
                proc_macro2::Span::call_site(),
            );
            let duplicate_tag_msg =
                syn::LitStr::new("duplicate tag field", proc_macro2::Span::call_site());
            let duplicate_content_msg =
                syn::LitStr::new("duplicate content field", proc_macro2::Span::call_site());
            let missing_tag_msg =
                syn::LitStr::new("missing tag field", proc_macro2::Span::call_site());
            let missing_content_msg =
                syn::LitStr::new("missing content field", proc_macro2::Span::call_site());

            let parse_match = quote! {
                match __idx_local {
                    #(#parse_arms),*,
                    _ => Err(norito::Error::Message("invalid enum variant index".into())),
                }
            };

            quote! {
                impl<'a> norito::json::FastFromJson<'a> for #ident {
                    fn parse<'arena>(
                        w: &mut norito::json::TapeWalker<'a>,
                        arena: &'arena mut norito::json::Arena,
                    ) -> ::core::result::Result<Self, norito::Error> {
                        let __input = w.input();
                        w.expect_object_start()?;
                        let mut __variant_idx: ::core::option::Option<u8> = ::core::option::Option::None;
                        let mut __content_slice: ::core::option::Option<&str> = ::core::option::Option::None;
                        let mut __result: ::core::option::Option<Self> = ::core::option::Option::None;

                        while !w.peek_object_end()? {
                            let __kh = w.read_key_hash()?;
                            w.expect_colon_resync()?;
                            if __kh == #tag_hash_expr {
                                if __variant_idx.is_some() {
                                    return Err(norito::Error::Message(#duplicate_tag_msg.into()));
                                }
                                let __tag_ref = w.parse_string_ref_inline(arena)?;
                                let __tag_str: &str = match __tag_ref {
                                    norito::json::StrRef::Borrowed(s) => s,
                                    norito::json::StrRef::Owned(s) => s,
                                };
                                let __idx = match __tag_str {
                                    #(#tag_match_arms)*
                                    other => {
                                        return Err(norito::Error::Message(format!(#unknown_tag_msg, other).into()));
                                    }
                                };
                                __variant_idx = Some(__idx);
                                if let Some(__slice) = __content_slice.take() {
                                    let __norito_content_slice = __slice;
                                    let __value = {
                                        let __idx_local = __idx;
                                        #parse_match
                                    }?;
                                    __result = Some(__value);
                                }
                            } else if __kh == #content_hash_expr {
                                if __content_slice.is_some() || __result.is_some() {
                                    return Err(norito::Error::Message(#duplicate_content_msg.into()));
                                }
                                let __start = w.raw_pos();
                                let mut __parser = norito::json::Parser::new_at(__input, __start);
                                __parser.skip_value()?;
                                let __end = __parser.position();
                                w.sync_to_raw(__end);
                                let __slice = &__input[__start..__end];
                                if let Some(__idx) = __variant_idx {
                                    let __norito_content_slice = __slice;
                                    let __value = {
                                        let __idx_local = __idx;
                                        #parse_match
                                    }?;
                                    __result = Some(__value);
                                } else {
                                    __content_slice = Some(__slice);
                                }
                            } else {
                                w.skip_value()?;
                            }
                            let _ = w.consume_comma_if_present()?;
                        }
                        w.expect_object_end()?;

                        if let Some(value) = __result {
                            return Ok(value);
                        }

                        let __idx = __variant_idx.ok_or_else(|| norito::Error::Message(#missing_tag_msg.into()))?;
                        let __slice = __content_slice.ok_or_else(|| norito::Error::Message(#missing_content_msg.into()))?;
                        let __norito_content_slice = __slice;
                        let __idx_local = __idx;
                        #parse_match
                    }
                }
            }
        }
        _ => quote! { compile_error!("FastJson only supports structs and enums"); },
    };
    body.into()
}

#[proc_macro_derive(FastJsonWrite, attributes(norito))]
pub fn derive_fast_json_write(input: TokenStream) -> TokenStream {
    let DeriveInput {
        ident,
        data,
        generics,
        attrs,
        ..
    } = parse_macro_input!(input as DeriveInput);
    let container_attrs = match ContainerAttr::parse(&attrs) {
        Ok(attrs) => attrs,
        Err(err) => return err.to_compile_error().into(),
    };
    match data {
        Data::Struct(ds) => {
            if let Err(e) = validate_field_attrs(&ds.fields) {
                return e.to_compile_error().into();
            }
            match ds.fields {
                Fields::Named(named) => {
                    let mut r#gen = generics.clone();
                    let mut writers = Vec::new();
                    for f in named.named.iter() {
                        let attrs = FieldAttr::parse(&f.attrs);
                        if attrs.skip {
                            continue;
                        }
                        attrs.require_json_serialize_bound(&mut r#gen, &f.ty);
                        let fname = f.ident.as_ref().unwrap();
                        let key = container_attrs.rename_field(fname, &attrs);
                        let key_lit = syn::LitStr::new(&key, proc_macro2::Span::call_site());
                        let serialize_call =
                            attrs.serializer_call(quote!(&self.#fname), quote!(out));
                        let flatten_tokens = quote! {
                            let __value = norito::json::to_value(&self.#fname)
                                .expect("flatten field must serialize to JSON value");
                            if let norito::json::Value::Object(__map) = __value {
                                for (__key, __val) in __map.into_iter() {
                                    if !__norito_first {
                                        out.push(',');
                                    } else {
                                        __norito_first = false;
                                    }
                                    out.push('"');
                                    out.push_str(&__key);
                                    out.push_str("\":");
                                    norito::json::JsonSerialize::json_serialize(&__val, out);
                                }
                            } else {
                                panic!("#[norito(flatten)] field must serialize to an object");
                            }
                        };
                        let render = if attrs.flatten {
                            flatten_tokens
                        } else if let Some(predicate) = &attrs.skip_serializing_if {
                            quote! {
                                if !(#predicate)(&self.#fname) {
                                    if !__norito_first {
                                        out.push(',');
                                    } else {
                                        __norito_first = false;
                                    }
                                    out.push('"');
                                    out.push_str(#key_lit);
                                    out.push_str("\":");
                                    #serialize_call
                                }
                            }
                        } else {
                            quote! {
                                if !__norito_first {
                                    out.push(',');
                                } else {
                                    __norito_first = false;
                                }
                                out.push('"');
                                out.push_str(#key_lit);
                                out.push_str("\":");
                                #serialize_call
                            }
                        };
                        writers.push(render);
                    }
                    let (impl_generics, ty_generics, where_clause) = r#gen.split_for_impl();
                    quote! {
                        impl #impl_generics norito::json::FastJsonWrite for #ident #ty_generics #where_clause {
                            fn write_json(&self, out: &mut String) {
                                out.push('{');
                                let mut __norito_first = true;
                                #(#writers)*
                                out.push('}');
                            }
                        }
                    }
                    .into()
                }
                Fields::Unnamed(unnamed) => {
                    let mut r#gen = generics.clone();
                    let mut writers = Vec::new();
                    for (idx, f) in unnamed.unnamed.iter().enumerate() {
                        let attrs = FieldAttr::parse(&f.attrs);
                        if attrs.skip {
                            continue;
                        }
                        attrs.require_json_serialize_bound(&mut r#gen, &f.ty);
                        let index = Index::from(idx);
                        let serialize_call =
                            attrs.serializer_call(quote!(&self.#index), quote!(out));
                        if let Some(predicate) = &attrs.skip_serializing_if {
                            writers.push(quote! {
                                if !(#predicate)(&self.#index) {
                                    if !__norito_first {
                                        out.push(',');
                                    } else {
                                        __norito_first = false;
                                    }
                                    #serialize_call
                                }
                            });
                        } else {
                            writers.push(quote! {
                                if !__norito_first {
                                    out.push(',');
                                } else {
                                    __norito_first = false;
                                }
                                #serialize_call
                            });
                        }
                    }
                    let (impl_generics, ty_generics, where_clause) = r#gen.split_for_impl();
                    quote! {
                        impl #impl_generics norito::json::FastJsonWrite for #ident #ty_generics #where_clause {
                            fn write_json(&self, out: &mut String) {
                                out.push('[');
                                let mut __norito_first = true;
                                #(#writers)*
                                out.push(']');
                            }
                        }
                    }
                    .into()
                }
                Fields::Unit => {
                    let r#gen = generics.clone();
                    let (impl_generics, ty_generics, where_clause) = r#gen.split_for_impl();
                    quote! {
                        impl #impl_generics norito::json::FastJsonWrite for #ident #ty_generics #where_clause {
                            fn write_json(&self, out: &mut String) {
                                out.push_str("null");
                            }
                        }
                    }
                    .into()
                }
            }
        }
        Data::Enum(de) => {
            let enum_attr = match EnumAttr::parse(&attrs) {
                Ok(attr) => attr,
                Err(err) => return err.to_compile_error().into(),
            };
            let tag = match enum_attr.tag {
                Some(t) => t,
                None => {
                    return syn::Error::new_spanned(
                        &ident,
                        "enum JsonSerialize requires #[norito(tag = ...)]",
                    )
                    .to_compile_error()
                    .into();
                }
            };
            let content = match enum_attr.content {
                Some(c) => c,
                None => {
                    return syn::Error::new_spanned(
                        &ident,
                        "enum JsonSerialize requires #[norito(content = ...)]",
                    )
                    .to_compile_error()
                    .into();
                }
            };
            let tag_lit = syn::LitStr::new(&tag, proc_macro2::Span::call_site());
            let content_lit = syn::LitStr::new(&content, proc_macro2::Span::call_site());
            let mut r#gen = generics.clone();
            let mut arms = Vec::new();
            for variant in de.variants.iter() {
                let v_ident = &variant.ident;
                let v_attr = match VariantAttr::parse(&variant.attrs) {
                    Ok(attr) => attr,
                    Err(err) => return err.to_compile_error().into(),
                };
                let variant_name = container_attrs.rename_variant(v_ident, &v_attr);
                let variant_lit = syn::LitStr::new(&variant_name, proc_macro2::Span::call_site());
                match &variant.fields {
                    Fields::Unit => {
                        arms.push(quote! {
                            Self::#v_ident => {
                                out.push('{');
                                out.push('"');
                                out.push_str(#tag_lit);
                                out.push_str("\":");
                                norito::json::write_json_string(#variant_lit, out);
                                out.push(',');
                                out.push('"');
                                out.push_str(#content_lit);
                                out.push_str("\":null");
                                out.push('}');
                            }
                        });
                    }
                    Fields::Unnamed(fields) => {
                        if fields.unnamed.is_empty() {
                            arms.push(quote! {
                                Self::#v_ident => {
                                    out.push('{');
                                    out.push('"');
                                    out.push_str(#tag_lit);
                                    out.push_str("\":");
                                    norito::json::write_json_string(#variant_lit, out);
                                    out.push(',');
                                    out.push('"');
                                    out.push_str(#content_lit);
                                    out.push_str("\":null");
                                    out.push('}');
                                }
                            });
                            continue;
                        }
                        if fields.unnamed.len() == 1 {
                            let attrs = FieldAttr::parse(&fields.unnamed[0].attrs);
                            if attrs.skip {
                                return syn::Error::new_spanned(
                                    &fields.unnamed[0],
                                    "#[norito(skip)] is not supported on enum tuple variants",
                                )
                                .to_compile_error()
                                .into();
                            }
                            let ty = &fields.unnamed[0].ty;
                            attrs.require_json_serialize_bound(&mut r#gen, ty);
                            let binding = format_ident!("__norito_field");
                            let serialize_call =
                                attrs.serializer_call(quote!(#binding), quote!(out));
                            arms.push(quote! {
                                Self::#v_ident(#binding) => {
                                    out.push('{');
                                    out.push('"');
                                    out.push_str(#tag_lit);
                                    out.push_str("\":");
                                    norito::json::write_json_string(#variant_lit, out);
                                    out.push(',');
                                    out.push('"');
                                    out.push_str(#content_lit);
                                    out.push_str("\":");
                                    #serialize_call
                                    out.push('}');
                                }
                            });
                        } else {
                            let mut bindings = Vec::new();
                            let mut serializers = Vec::new();
                            for (idx, field) in fields.unnamed.iter().enumerate() {
                                let attrs = FieldAttr::parse(&field.attrs);
                                if attrs.skip {
                                    return syn::Error::new_spanned(
                                        field,
                                        "#[norito(skip)] is not supported on enum tuple variants",
                                    )
                                    .to_compile_error()
                                    .into();
                                }
                                attrs.require_json_serialize_bound(&mut r#gen, &field.ty);
                                let binding = format_ident!("__norito_v{idx}");
                                let serialize_call =
                                    attrs.serializer_call(quote!(#binding), quote!(out));
                                serializers.push(quote! {
                                    if !__norito_first {
                                        out.push(',');
                                    } else {
                                        __norito_first = false;
                                    }
                                    #serialize_call
                                });
                                bindings.push(binding);
                            }
                            let ref_bindings: Vec<_> = bindings.iter().collect();
                            arms.push(quote! {
                                Self::#v_ident( #( #ref_bindings ),* ) => {
                                    out.push('{');
                                    out.push('"');
                                    out.push_str(#tag_lit);
                                    out.push_str("\":");
                                    norito::json::write_json_string(#variant_lit, out);
                                    out.push(',');
                                    out.push('"');
                                    out.push_str(#content_lit);
                                    out.push_str("\":[");
                                    let mut __norito_first = true;
                                    #(#serializers)*
                                    out.push(']');
                                    out.push('}');
                                }
                            });
                        }
                    }
                    Fields::Named(fields) => {
                        let mut field_writers = Vec::new();
                        let mut ref_idents = Vec::new();
                        for field in fields.named.iter() {
                            let attrs = FieldAttr::parse(&field.attrs);
                            if attrs.skip {
                                return syn::Error::new_spanned(
                                    field,
                                    "#[norito(skip)] is not supported on enum struct variants",
                                )
                                .to_compile_error()
                                .into();
                            }
                            attrs.require_json_serialize_bound(&mut r#gen, &field.ty);
                            let fname = field.ident.as_ref().unwrap();
                            ref_idents.push(fname.clone());
                            let key = attrs.rename.clone().unwrap_or_else(|| fname.to_string());
                            let key_lit = syn::LitStr::new(&key, proc_macro2::Span::call_site());
                            let serialize_call = attrs.serializer_call(quote!(#fname), quote!(out));
                            field_writers.push(quote! {
                                if !__norito_first_inner {
                                    out.push(',');
                                } else {
                                    __norito_first_inner = false;
                                }
                                out.push('"');
                                out.push_str(#key_lit);
                                out.push_str("\":");
                                #serialize_call
                            });
                        }
                        arms.push(quote! {
                            Self::#v_ident { #( #ref_idents ),* } => {
                                out.push('{');
                                out.push('"');
                                out.push_str(#tag_lit);
                                out.push_str("\":");
                                norito::json::write_json_string(#variant_lit, out);
                                out.push(',');
                                out.push('"');
                                out.push_str(#content_lit);
                                out.push_str("\":{");
                                let mut __norito_first_inner = true;
                                #(#field_writers)*
                                out.push('}');
                                out.push('}');
                            }
                        });
                    }
                }
            }
            let (impl_generics, ty_generics, where_clause) = r#gen.split_for_impl();
            quote! {
                impl #impl_generics norito::json::FastJsonWrite for #ident #ty_generics #where_clause {
                    fn write_json(&self, out: &mut String) {
                        match self {
                            #( #arms ),*
                        }
                    }
                }
            }
            .into()
        }
        _ => syn::Error::new_spanned(ident, "FastJsonWrite only supports structs and enums")
            .to_compile_error()
            .into(),
    }
}

#[proc_macro_derive(JsonSerialize, attributes(norito))]
pub fn derive_json_serialize(input: TokenStream) -> TokenStream {
    derive_fast_json_write(input)
}

fn derive_struct_json_deserialize(
    ident: &syn::Ident,
    generics: &Generics,
    data: &syn::DataStruct,
    container_attrs: &ContainerAttr,
) -> syn::Result<TokenStream2> {
    validate_field_attrs(&data.fields)?;
    let mut r#gen = generics.clone();
    match &data.fields {
        Fields::Named(named) => {
            if named
                .named
                .iter()
                .any(|f| FieldAttr::parse(&f.attrs).flatten)
            {
                return derive_struct_json_deserialize_flatten(
                    ident,
                    generics,
                    named,
                    container_attrs,
                );
            }
            let mut inits = Vec::new();
            let mut arms = Vec::new();
            let mut finals = Vec::new();
            for f in named.named.iter() {
                let attrs = FieldAttr::parse(&f.attrs);
                let field_ident = f.ident.as_ref().unwrap();
                let key = container_attrs.rename_field(field_ident, &attrs);
                let key_lit = syn::LitStr::new(&key, proc_macro2::Span::call_site());
                let var_ident = format_ident!("__norito_field_{}", field_ident);
                if attrs.skip {
                    if attrs.default || attrs.default_fn.is_some() {
                        return Err(syn::Error::new_spanned(
                            f,
                            "#[norito(skip)] cannot be combined with #[norito(default)]",
                        ));
                    }
                    // Skipped fields are filled from Default
                    add_bound(&mut r#gen, &f.ty, quote!(::core::default::Default));
                    finals.push(quote! { #field_ident: ::core::default::Default::default() });
                    continue;
                }
                attrs.require_json_deserialize_bound(&mut r#gen, &f.ty);
                let ty = &f.ty;
                inits.push(quote! { let mut #var_ident: ::core::option::Option<#ty> = ::core::option::Option::None; });
                let duplicate_msg = syn::LitStr::new(
                    &format!("duplicate field `{key}`"),
                    proc_macro2::Span::call_site(),
                );
                let deserialize_call = attrs.deserializer_call(ty, quote!(parser));
                arms.push(quote! {
                    #key_lit => {
                        if #var_ident.is_some() {
                            return Err(norito::json::Error::Message(#duplicate_msg.into()));
                        }
                        let value = #deserialize_call;
                        #var_ident = ::core::option::Option::Some(value);
                    }
                });
                let missing_msg = syn::LitStr::new(
                    &format!("missing field `{key}`"),
                    proc_macro2::Span::call_site(),
                );
                let default_expr = if let Some(path) = &attrs.default_fn {
                    Some(quote! { (#path)() })
                } else if attrs.default {
                    add_bound(&mut r#gen, &f.ty, quote!(::core::default::Default));
                    Some(quote! { ::core::default::Default::default() })
                } else {
                    None
                };
                if is_option_type(&f.ty) {
                    if let Some(expr) = default_expr {
                        finals.push(quote! { #field_ident: #var_ident.unwrap_or_else(|| #expr) });
                    } else {
                        finals.push(quote! { #field_ident: #var_ident.unwrap_or(::core::option::Option::None) });
                    }
                } else if let Some(expr) = default_expr {
                    finals.push(quote! { #field_ident: #var_ident.unwrap_or_else(|| #expr) });
                } else {
                    finals.push(quote! {
                        #field_ident: #var_ident.ok_or_else(|| norito::json::Error::Message(#missing_msg.into()))?
                    });
                }
            }
            let (impl_generics, ty_generics, where_clause) = r#gen.split_for_impl();
            let result = quote! {
                impl #impl_generics norito::json::JsonDeserialize for #ident #ty_generics #where_clause {
                    #[allow(clippy::useless_let_if_seq)]
                    fn json_deserialize(parser: &mut norito::json::Parser<'_>) -> ::core::result::Result<Self, norito::json::Error> {
                        parser.skip_ws();
                        parser.expect(b'{')?;
                        parser.skip_ws();
                        #(#inits)*
                        if !parser.try_consume_char(b'}')? {
                            loop {
                                parser.skip_ws();
                                let key = parser.parse_string()?;
                                parser.expect(b':')?;
                                match key.as_str() {
                                    #( #arms ),*,
                                    _ => {
                                        parser.skip_value()?;
                                    }
                                }
                                parser.skip_ws();
                                if parser.try_consume_char(b',')? {
                                    continue;
                                }
                                parser.expect(b'}')?;
                                break;
                            }
                        }
                        Ok(Self { #( #finals ),* })
                    }
                }
            };
            Ok(result)
        }
        Fields::Unnamed(unnamed) => {
            let mut inits = Vec::new();
            let mut match_arms = Vec::new();
            let mut finals = Vec::new();
            let mut gen_local = r#gen.clone();
            for (idx, f) in unnamed.unnamed.iter().enumerate() {
                let attrs = FieldAttr::parse(&f.attrs);
                let var_ident = format_ident!("__norito_tuple_{idx}");
                if attrs.skip {
                    add_bound(&mut gen_local, &f.ty, quote!(::core::default::Default));
                    finals.push(quote! { ::core::default::Default::default() });
                    match_arms.push(quote! {
                        #idx => {
                            parser.skip_value()?;
                        }
                    });
                    continue;
                }
                attrs.require_json_deserialize_bound(&mut gen_local, &f.ty);
                let ty = &f.ty;
                inits.push(quote! { let mut #var_ident: ::core::option::Option<#ty> = ::core::option::Option::None; });
                let deserialize_call = attrs.deserializer_call(ty, quote!(parser));
                match_arms.push(quote! {
                    #idx => {
                        let value = #deserialize_call;
                        #var_ident = ::core::option::Option::Some(value);
                    }
                });
                let missing_msg = syn::LitStr::new(
                    &format!("missing tuple index {idx}"),
                    proc_macro2::Span::call_site(),
                );
                let default_expr = if let Some(path) = &attrs.default_fn {
                    Some(quote! { (#path)() })
                } else if attrs.default {
                    add_bound(&mut gen_local, &f.ty, quote!(::core::default::Default));
                    Some(quote! { ::core::default::Default::default() })
                } else {
                    None
                };
                if is_option_type(&f.ty) {
                    if let Some(expr) = default_expr {
                        finals.push(quote! { #var_ident.unwrap_or_else(|| #expr) });
                    } else {
                        finals.push(quote! { #var_ident.unwrap_or(::core::option::Option::None) });
                    }
                } else if let Some(expr) = default_expr {
                    finals.push(quote! { #var_ident.unwrap_or_else(|| #expr) });
                } else {
                    finals.push(quote! {
                        #var_ident.ok_or_else(|| norito::json::Error::Message(#missing_msg.into()))?
                    });
                }
            }
            let (impl_generics, ty_generics, where_clause) = gen_local.split_for_impl();
            let len = unnamed.unnamed.len();
            let result = quote! {
                impl #impl_generics norito::json::JsonDeserialize for #ident #ty_generics #where_clause {
                    #[allow(clippy::useless_let_if_seq)]
                    fn json_deserialize(parser: &mut norito::json::Parser<'_>) -> ::core::result::Result<Self, norito::json::Error> {
                        parser.skip_ws();
                        parser.expect(b'[')?;
                        parser.skip_ws();
                        #(#inits)*
                        let mut __norito_index: usize = 0;
                        if !parser.try_consume_char(b']')? {
                            let mut __first = true;
                            loop {
                                if !__first {
                                    parser.expect(b',')?;
                                }
                                __first = false;
                                match __norito_index {
                                    #( #match_arms ),*,
                                    _ => {
                                        parser.skip_value()?;
                                        return Err(norito::json::Error::Message(format!("too many elements for tuple struct `{}`", stringify!(#ident)).into()));
                                    }
                                }
                                __norito_index += 1;
                                parser.skip_ws();
                                if parser.try_consume_char(b']')? {
                                    break;
                                }
                            }
                        }
                        if __norito_index < #len {
                            // Missing trailing elements will be handled by final unwrap/default logic
                        }
                        Ok(Self( #( #finals ),* ))
                    }
                }
            };
            Ok(result)
        }
        Fields::Unit => {
            let (impl_generics, ty_generics, where_clause) = r#gen.split_for_impl();
            Ok(quote! {
                impl #impl_generics norito::json::JsonDeserialize for #ident #ty_generics #where_clause {
                    #[allow(clippy::useless_let_if_seq)]
                    fn json_deserialize(parser: &mut norito::json::Parser<'_>) -> ::core::result::Result<Self, norito::json::Error> {
                        parser.parse_null()?;
                        Ok(Self)
                    }
                }
            })
        }
    }
}

fn derive_struct_json_deserialize_flatten(
    ident: &syn::Ident,
    generics: &Generics,
    named: &syn::FieldsNamed,
    container_attrs: &ContainerAttr,
) -> syn::Result<TokenStream2> {
    let mut r#gen = generics.clone();
    let mut init_stmts = Vec::new();
    let mut parse_stmts = Vec::new();
    let mut flatten_stmts = Vec::new();
    let mut finals = Vec::new();

    for field in named.named.iter() {
        let attrs = FieldAttr::parse(&field.attrs);
        let field_ident = field.ident.as_ref().unwrap();
        let ty = &field.ty;
        if attrs.skip {
            if attrs.default || attrs.default_fn.is_some() {
                return Err(syn::Error::new_spanned(
                    field,
                    "#[norito(skip)] cannot be combined with #[norito(default)]",
                ));
            }
            add_bound(&mut r#gen, &field.ty, quote!(::core::default::Default));
            finals.push(quote! { #field_ident: ::core::default::Default::default() });
            continue;
        }

        attrs.require_json_deserialize_bound(&mut r#gen, ty);
        let var_ident = format_ident!("__norito_field_{}", field_ident);
        init_stmts.push(quote! {
            let mut #var_ident: ::core::option::Option<#ty> = ::core::option::Option::None;
        });

        if attrs.flatten {
            let parse_expr = attrs
                .deserialize_from_value(ty, quote!(norito::json::Value::Object(__map.clone())));
            flatten_stmts.push(quote! {
                let parsed = #parse_expr;
                let __used = norito::json::to_value(&parsed)?;
                if let norito::json::Value::Object(__used_map) = __used {
                    for __key in __used_map.keys() {
                        __map.remove(__key);
                    }
                } else {
                    return Err(norito::json::Error::Message(
                        "#[norito(flatten)] field must deserialize to an object".into(),
                    ));
                }
                #var_ident = ::core::option::Option::Some(parsed);
            });
        } else {
            let key = container_attrs.rename_field(field_ident, &attrs);
            let key_lit = syn::LitStr::new(&key, proc_macro2::Span::call_site());
            let parse_expr = attrs.deserialize_from_value(ty, quote!(value));
            parse_stmts.push(quote! {
                if let ::core::option::Option::Some(value) = __map.remove(#key_lit) {
                    let parsed = #parse_expr;
                    #var_ident = ::core::option::Option::Some(parsed);
                }
            });
        }

        let missing_key = container_attrs.rename_field(field_ident, &attrs);
        let missing_msg = syn::LitStr::new(
            &format!("missing field `{missing_key}`"),
            proc_macro2::Span::call_site(),
        );
        let default_expr = if let Some(path) = &attrs.default_fn {
            Some(quote! { (#path)() })
        } else if attrs.default {
            add_bound(&mut r#gen, &field.ty, quote!(::core::default::Default));
            Some(quote! { ::core::default::Default::default() })
        } else {
            None
        };
        if is_option_type(&field.ty) {
            if let Some(expr) = default_expr {
                finals.push(quote! { #field_ident: #var_ident.unwrap_or_else(|| #expr) });
            } else {
                finals.push(quote! {
                    #field_ident: #var_ident.unwrap_or(::core::option::Option::None)
                });
            }
        } else if let Some(expr) = default_expr {
            finals.push(quote! { #field_ident: #var_ident.unwrap_or_else(|| #expr) });
        } else {
            finals.push(quote! {
                #field_ident: #var_ident.ok_or_else(|| norito::json::Error::Message(#missing_msg.into()))?
            });
        }
    }

    let (impl_generics, ty_generics, where_clause) = r#gen.split_for_impl();
    let result = quote! {
        impl #impl_generics norito::json::JsonDeserialize for #ident #ty_generics #where_clause {
            #[allow(clippy::useless_let_if_seq)]
            fn json_deserialize(parser: &mut norito::json::Parser<'_>) -> ::core::result::Result<Self, norito::json::Error> {
                let value = norito::json::Value::json_deserialize(parser)?;
                let mut __map = match value {
                    norito::json::Value::Object(map) => map,
                    _ => {
                        return Err(norito::json::Error::Message(
                            "expected JSON object".into(),
                        ));
                    }
                };
                #(#init_stmts)*
                #(#parse_stmts)*
                #(#flatten_stmts)*
                Ok(Self { #(#finals),* })
            }
        }
    };
    Ok(result)
}

fn derive_enum_json_deserialize(
    ident: &syn::Ident,
    generics: &Generics,
    data: &syn::DataEnum,
    attrs: &[Attribute],
    container_attrs: &ContainerAttr,
) -> syn::Result<TokenStream2> {
    let enum_attr = EnumAttr::parse(attrs)?;
    let tag = enum_attr.tag.ok_or_else(|| {
        syn::Error::new_spanned(ident, "enum JsonDeserialize requires #[norito(tag = ...)]")
    })?;
    let content = enum_attr.content.ok_or_else(|| {
        syn::Error::new_spanned(
            ident,
            "enum JsonDeserialize requires #[norito(content = ...)]",
        )
    })?;
    let tag_lit = syn::LitStr::new(&tag, proc_macro2::Span::call_site());
    let content_lit = syn::LitStr::new(&content, proc_macro2::Span::call_site());

    let mut r#gen = generics.clone();
    let mut arms = Vec::new();

    for variant in data.variants.iter() {
        let v_ident = &variant.ident;
        let v_attr = VariantAttr::parse(&variant.attrs)?;
        let variant_name = container_attrs.rename_variant(v_ident, &v_attr);
        let variant_lit = syn::LitStr::new(&variant_name, proc_macro2::Span::call_site());
        match &variant.fields {
            Fields::Unit => {
                arms.push(quote! {
                    #variant_lit => {
                        let mut __parser = norito::json::Parser::new(__norito_content_str);
                        __parser.parse_null()?;
                        __parser.skip_ws();
                        if !__parser.eof() {
                            return Err(norito::json::Error::Message(
                                format!("unexpected content for unit variant `{}`", #variant_lit).into(),
                            ));
                        }
                        Ok(Self::#v_ident)
                    }
                });
            }
            Fields::Unnamed(fields) => {
                let count = fields.unnamed.len();
                if count == 1 {
                    let field = &fields.unnamed[0];
                    let attrs = FieldAttr::parse(&field.attrs);
                    if attrs.skip {
                        return Err(syn::Error::new_spanned(
                            field,
                            "#[norito(skip)] is not supported on enum tuple variants",
                        ));
                    }
                    attrs.require_json_deserialize_bound(&mut r#gen, &field.ty);
                    let ty = &field.ty;
                    let deserialize_call = attrs.deserializer_call(ty, quote!(&mut __parser));
                    arms.push(quote! {
                        #variant_lit => {
                            let mut __parser = norito::json::Parser::new(__norito_content_str);
                            let value = #deserialize_call;
                            __parser.skip_ws();
                            if !__parser.eof() {
                                return Err(norito::json::Error::Message(format!(
                                    "unexpected trailing data for variant `{}`", #variant_lit
                                ).into()));
                            }
                            Ok(Self::#v_ident(value))
                        }
                    });
                } else {
                    let mut gen_local = r#gen.clone();
                    let mut inits = Vec::new();
                    let mut match_tokens = Vec::new();
                    let mut finals = Vec::new();
                    for (idx, field) in fields.unnamed.iter().enumerate() {
                        let attrs = FieldAttr::parse(&field.attrs);
                        if attrs.skip {
                            return Err(syn::Error::new_spanned(
                                field,
                                "#[norito(skip)] is not supported on enum tuple variants",
                            ));
                        }
                        let ty = &field.ty;
                        attrs.require_json_deserialize_bound(&mut gen_local, ty);
                        let binding = format_ident!("__norito_variant_{idx}");
                        inits.push(quote! {
                            let mut #binding: ::core::option::Option<#ty> = ::core::option::Option::None;
                        });
                        let missing_text =
                            format!("missing tuple index {idx} for variant `{variant_name}`");
                        let missing_msg =
                            syn::LitStr::new(&missing_text, proc_macro2::Span::call_site());
                        let deserialize_call = attrs.deserializer_call(ty, quote!(&mut __parser));
                        match_tokens.push(quote! {
                            #idx => {
                                let value = #deserialize_call;
                                #binding = ::core::option::Option::Some(value);
                            }
                        });
                        finals.push(quote! {
                            #binding.ok_or_else(|| norito::json::Error::Message(#missing_msg.into()))?
                        });
                    }
                    let constructor = quote! { Self::#v_ident( #( #finals ),* ) };
                    arms.push(quote! {
                        #variant_lit => {
                            let mut __parser = norito::json::Parser::new(__norito_content_str);
                            __parser.skip_ws();
                            __parser.expect(b'[')?;
                            __parser.skip_ws();
                            #(#inits)*
                            let mut __idx = 0usize;
                            if !__parser.try_consume_char(b']')? {
                                let mut __first = true;
                                loop {
                                    if !__first {
                                        __parser.expect(b',')?;
                                    }
                                    __first = false;
                                    match __idx {
                                        #( #match_tokens ),*,
                                        _ => {
                                            __parser.skip_value()?;
                                            return Err(norito::json::Error::Message(format!(
                                                "too many elements for variant `{}`", #variant_lit
                                            ).into()));
                                        }
                                    }
                                    __idx += 1;
                                    __parser.skip_ws();
                                    if __parser.try_consume_char(b']')? {
                                        break;
                                    }
                                }
                            }
                            __parser.skip_ws();
                            if !__parser.eof() {
                                return Err(norito::json::Error::Message(format!(
                                    "unexpected trailing data for variant `{}`", #variant_lit
                                ).into()));
                            }
                            Ok(#constructor)
                        }
                    });
                    r#gen = gen_local;
                }
            }
            Fields::Named(fields) => {
                let mut inits = Vec::new();
                let mut match_tokens = Vec::new();
                let mut finals = Vec::new();
                let mut gen_local = r#gen.clone();
                for field in fields.named.iter() {
                    let attrs = FieldAttr::parse(&field.attrs);
                    if attrs.skip {
                        return Err(syn::Error::new_spanned(
                            field,
                            "#[norito(skip)] is not supported on enum struct variants",
                        ));
                    }
                    let field_ident = field.ident.as_ref().unwrap();
                    let key = container_attrs.rename_field(field_ident, &attrs);
                    let key_lit = syn::LitStr::new(&key, proc_macro2::Span::call_site());
                    add_bound(
                        &mut gen_local,
                        &field.ty,
                        quote!(norito::json::JsonDeserialize),
                    );
                    let var_ident = format_ident!("__norito_variant_field_{}", field_ident);
                    let ty = &field.ty;
                    inits.push(quote! { let mut #var_ident: ::core::option::Option<#ty> = ::core::option::Option::None; });
                    let duplicate_text =
                        format!("duplicate field `{key}` in variant `{variant_name}`");
                    let duplicate_msg =
                        syn::LitStr::new(&duplicate_text, proc_macro2::Span::call_site());
                    match_tokens.push(quote! {
                        #key_lit => {
                            if #var_ident.is_some() {
                                return Err(norito::json::Error::Message(#duplicate_msg.into()));
                            }
                            let value = <#ty as norito::json::JsonDeserialize>::json_deserialize(&mut __parser)?;
                            #var_ident = ::core::option::Option::Some(value);
                        }
                    });
                    let missing_text = format!("missing field `{key}` in variant `{variant_name}`");
                    let missing_msg =
                        syn::LitStr::new(&missing_text, proc_macro2::Span::call_site());
                    finals.push(quote! {
                        #field_ident: #var_ident.ok_or_else(|| norito::json::Error::Message(#missing_msg.into()))?
                    });
                }
                arms.push(quote! {
                    #variant_lit => {
                        let mut __parser = norito::json::Parser::new(__norito_content_str);
                        __parser.skip_ws();
                        if !__parser.try_consume_char(b'{')? {
                            return Err(norito::json::Error::Message(format!(
                                "expected object for variant `{}`", #variant_lit
                            ).into()));
                        }
                        #(#inits)*
                        __parser.skip_ws();
                        if !__parser.try_consume_char(b'}')? {
                            loop {
                                __parser.skip_ws();
                                let key = __parser.parse_string()?;
                                __parser.expect(b':')?;
                                match key.as_str() {
                                    #( #match_tokens ),*,
                                    _ => {
                                        __parser.skip_value()?;
                                    }
                                }
                                __parser.skip_ws();
                                if __parser.try_consume_char(b',')? {
                                    continue;
                                }
                                __parser.expect(b'}')?;
                                break;
                            }
                        }
                        __parser.skip_ws();
                        if !__parser.eof() {
                            return Err(norito::json::Error::Message(format!(
                                "unexpected trailing data for variant `{}`", #variant_lit
                            ).into()));
                        }
                        Ok(Self::#v_ident { #( #finals ),* })
                    }
                });
                r#gen = gen_local;
            }
        }
    }

    let (impl_generics, ty_generics, where_clause) = r#gen.split_for_impl();
    let result = quote! {
        impl #impl_generics norito::json::JsonDeserialize for #ident #ty_generics #where_clause {
            #[allow(clippy::useless_let_if_seq)]
            fn json_deserialize(parser: &mut norito::json::Parser<'_>) -> ::core::result::Result<Self, norito::json::Error> {
                parser.skip_ws();
                parser.expect(b'{')?;
                parser.skip_ws();
                let mut __norito_tag: ::core::option::Option<String> = ::core::option::Option::None;
                let mut __norito_raw: ::core::option::Option<String> = ::core::option::Option::None;
                if !parser.try_consume_char(b'}')? {
                    loop {
                        parser.skip_ws();
                        let key = parser.parse_string()?;
                        parser.expect(b':')?;
                        if key == #tag_lit {
                            let value = parser.parse_string()?;
                            __norito_tag = ::core::option::Option::Some(value);
                        } else if key == #content_lit {
                            let raw = norito::json::RawValue::json_deserialize(parser)?;
                            __norito_raw = ::core::option::Option::Some(raw.into_string());
                        } else {
                            parser.skip_value()?;
                        }
                        parser.skip_ws();
                        if parser.try_consume_char(b',')? {
                            continue;
                        }
                        parser.expect(b'}')?;
                        break;
                    }
                }
                let tag = __norito_tag.ok_or_else(|| norito::json::Error::Message(
                    format!("missing `{}` field", #tag_lit).into(),
                ))?;
                let content_buf;
                if let Some(raw) = __norito_raw {
                    content_buf = raw;
                } else {
                    content_buf = "null".to_owned();
                }
                let __norito_content_str = content_buf.as_str();
                match tag.as_str() {
                    #( #arms ),*,
                    other => Err(norito::json::Error::Message(
                        format!("unknown variant `{}`", other).into(),
                    )),
                }
            }
        }
    };
    Ok(result)
}
fn derive_fast_from_json_fallback(input: &DeriveInput) -> TokenStream2 {
    let ident = &input.ident;
    let generics = input.generics.clone();
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    quote! {
        impl<'a> #impl_generics norito::json::FastFromJson<'a> for #ident #ty_generics #where_clause {
            fn parse<'arena>(
                w: &mut norito::json::TapeWalker<'a>,
                _arena: &'arena mut norito::json::Arena,
            ) -> ::core::result::Result<Self, norito::Error> {
                let input = w.input();
                let mut parser = norito::json::Parser::new_at(input, w.raw_pos());
                let value = <Self as norito::json::JsonDeserialize>::json_deserialize(&mut parser)
                    .map_err(norito::Error::from)?;
                w.sync_to_raw(parser.position());
                Ok(value)
            }
        }
    }
}

fn has_no_fast_from_json_attr(attrs: &[syn::Attribute]) -> bool {
    let mut flag = false;
    for attr in attrs {
        if attr.path().is_ident("norito") {
            let _ = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("no_fast_from_json") {
                    flag = true;
                }
                Ok(())
            });
        }
    }
    flag
}

#[proc_macro_derive(JsonDeserialize, attributes(norito))]
pub fn derive_json_deserialize(input: TokenStream) -> TokenStream {
    let parsed = parse_macro_input!(input as DeriveInput);
    let container_attrs = match ContainerAttr::parse(&parsed.attrs) {
        Ok(attrs) => attrs,
        Err(err) => return err.to_compile_error().into(),
    };
    let fast_impl = if has_no_fast_from_json_attr(&parsed.attrs) {
        None
    } else {
        Some(derive_fast_from_json_fallback(&parsed))
    };
    let deserialize_impl = match &parsed.data {
        Data::Struct(data) => {
            match derive_struct_json_deserialize(
                &parsed.ident,
                &parsed.generics,
                data,
                &container_attrs,
            ) {
                Ok(ts) => ts,
                Err(e) => return e.to_compile_error().into(),
            }
        }
        Data::Enum(data) => {
            match derive_enum_json_deserialize(
                &parsed.ident,
                &parsed.generics,
                data,
                &parsed.attrs,
                &container_attrs,
            ) {
                Ok(ts) => ts,
                Err(e) => return e.to_compile_error().into(),
            }
        }
        _ => {
            return syn::Error::new_spanned(
                &parsed.ident,
                "JsonDeserialize only supports structs and enums",
            )
            .to_compile_error()
            .into();
        }
    };
    let mut tokens = TokenStream2::new();
    if let Some(fallback) = fast_impl {
        tokens.extend(fallback);
    }
    tokens.extend(deserialize_impl);
    tokens.into()
}

#[proc_macro_derive(Encode, attributes(codec, norito))]
/// Derive `norito::codec::Encode` for structs.
pub fn derive_encode(input: TokenStream) -> TokenStream {
    derive_norito_serialize(input)
}

#[proc_macro_derive(Decode, attributes(codec, norito))]
/// Derive `norito::codec::Decode` for structs.
pub fn derive_decode(input: TokenStream) -> TokenStream {
    derive_norito_deserialize(input)
}
