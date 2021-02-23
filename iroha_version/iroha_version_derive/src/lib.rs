extern crate proc_macro;

use crate::proc_macro::TokenStream;
use proc_macro2::Span;
use proc_macro_error::{abort, proc_macro_error, OptionExt, ResultExt};
use quote::quote;
use std::{collections::HashMap, ops::Range};
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input,
    spanned::Spanned,
    AttributeArgs, Error as SynError, Ident, ItemStruct, Lit, LitInt, Meta, NestedMeta,
    Result as SynResult, Token,
};

const VERSION_NUMBER_ARG_NAME: &str = "n";
const VERSIONED_STRUCT_ARG_NAME: &str = "versioned";
const VERSION_FIELD_NAME: &str = "version";
const CONTENT_FIELD_NAME: &str = "content";

/// Used to declare that this struct represents a particular version as a part of the versioned container.
///
/// Adds support for both scale codec and json serialization. To declare only with json support use [`version_with_json`], for scale - [`version_with_scale`].
///
/// ### Arguments:
/// - named `n: u8` - what version this particular struct represents.
/// - named `versioned: String` - to which versioned container to link this struct. Versioned containers are created with [`declare_versioned`].
///
/// ### Examples
/// See [`declare_versioned`].
#[proc_macro_error]
#[proc_macro_attribute]
pub fn version(attr: TokenStream, item: TokenStream) -> TokenStream {
    impl_version(attr, item, true, true)
}

/// See [`version`] for more information.
#[proc_macro_error]
#[proc_macro_attribute]
pub fn version_with_scale(attr: TokenStream, item: TokenStream) -> TokenStream {
    impl_version(attr, item, false, true)
}

/// See [`version`] for more information.
#[proc_macro_error]
#[proc_macro_attribute]
pub fn version_with_json(attr: TokenStream, item: TokenStream) -> TokenStream {
    impl_version(attr, item, true, false)
}

/// Used to generate a versioned container with the given name and given range of supported versions.
///
/// Adds support for both scale codec and json serialization. To declare only with json support use [`declare_versioned_with_json`], for scale - [`declare_versioned_with_scale`].
///
/// ### Arguments
/// 1. positional `versioned_enum_name`
/// 2. positional `supported_version_range`
///
/// ### Example
/// ```
/// use parity_scale_codec::{Decode, Encode};
/// use serde::{Deserialize, Serialize};
/// use iroha_version_derive::{declare_versioned, version};
/// use iroha_version::json::*;
///
/// declare_versioned!(VersionedMessage 1..2);
///
/// #[version(n = 1, versioned = "VersionedMessage")]
/// #[derive(Debug, Clone, Decode, Encode, Serialize, Deserialize)]
/// pub struct Message1;
///
/// let versioned_message: VersionedMessage = Message1.into();
/// let json = versioned_message.to_versioned_json_str().unwrap();
/// let decoded_message = VersionedMessage::from_versioned_json_str(&json).unwrap();
/// match decoded_message {
///    VersionedMessage::V1(message) => {
///        let _message: Message1 = message.into();
///        Ok(())
///    }
///    _ => Err("Unsupported version.".to_string()),
/// }.unwrap();
/// ```
#[proc_macro]
pub fn declare_versioned(input: TokenStream) -> TokenStream {
    let args = parse_macro_input!(input as DeclareVersionedArgs);
    impl_declare_versioned(args, true, true)
}

/// See [`declare_versioned`] for more information.
#[proc_macro]
pub fn declare_versioned_with_scale(input: TokenStream) -> TokenStream {
    let args = parse_macro_input!(input as DeclareVersionedArgs);
    impl_declare_versioned(args, true, false)
}

/// See [`declare_versioned`] for more information.
#[proc_macro]
pub fn declare_versioned_with_json(input: TokenStream) -> TokenStream {
    let args = parse_macro_input!(input as DeclareVersionedArgs);
    impl_declare_versioned(args, false, true)
}

fn impl_version(
    attr: TokenStream,
    item: TokenStream,
    with_json: bool,
    with_scale: bool,
) -> TokenStream {
    let args = parse_macro_input!(attr as AttributeArgs);
    let item = parse_macro_input!(item as ItemStruct);
    let args_map: HashMap<_, _> = args
        .into_iter()
        .filter_map(|meta| {
            if let NestedMeta::Meta(Meta::NameValue(name_value)) = meta {
                Some((
                    name_value
                        .path
                        .get_ident()
                        .expect_or_abort("Expected single identifier for attribute argument key.")
                        .to_string(),
                    name_value.lit,
                ))
            } else {
                None
            }
        })
        .collect();
    let version_number = args_map
        .get(VERSION_NUMBER_ARG_NAME)
        .expect_or_abort(&format!(
            "No version number argument with name {} found.",
            VERSION_NUMBER_ARG_NAME
        ));
    let version_number: u32 = if let Lit::Int(number) = version_number {
        number
            .base10_parse()
            .expect_or_abort("Failed to parse version number integer literal.")
    } else {
        abort!(
            version_number.span(),
            "Version number argument should have an integer value."
        )
    };
    let versioned_struct_name = args_map
        .get(VERSIONED_STRUCT_ARG_NAME)
        .expect_or_abort(&format!(
            "No versioned struct name argument with name {} found.",
            VERSIONED_STRUCT_ARG_NAME
        ));
    let versioned_struct_name = if let Lit::Str(name) = versioned_struct_name {
        name.value()
    } else {
        abort!(
            version_number.span(),
            "Versioned struct name argument should have a string value."
        )
    };
    let wrapper_struct_name = Ident::new(
        &format!("{}V{}", versioned_struct_name, version_number),
        Span::call_site(),
    );
    let versioned_struct_name = Ident::new(&versioned_struct_name, Span::call_site());
    let json_derives = if with_json {
        quote!(serde::Serialize, serde::Deserialize,)
    } else {
        quote!()
    };
    let scale_derives = if with_scale {
        quote!(parity_scale_codec::Encode, parity_scale_codec::Decode,)
    } else {
        quote!()
    };
    let struct_name = item.ident.clone();
    quote!(
        #[derive(Debug, Clone, #scale_derives #json_derives)]
        pub struct #wrapper_struct_name (#struct_name);

        impl From<#wrapper_struct_name> for #struct_name {
            fn from(wrapper: #wrapper_struct_name) -> Self {
                let #wrapper_struct_name (inner) = wrapper;
                inner
            }
        }

        impl From<#struct_name> for #wrapper_struct_name {
            fn from(origin: #struct_name) -> Self {
                #wrapper_struct_name (origin)
            }
        }

        impl From<#struct_name> for #versioned_struct_name {
            fn from(origin: #struct_name) -> Self {
                let wrapped: #wrapper_struct_name = origin.into();
                wrapped.into()
            }
        }

        #item
    )
    .into()
}

#[derive(Debug)]
struct DeclareVersionedArgs {
    pub enum_name: Ident,
    pub range: Range<u8>,
}

impl DeclareVersionedArgs {
    pub fn version_idents(&self) -> Vec<Ident> {
        self.range
            .clone()
            .into_iter()
            .map(|i| Ident::new(&format!("V{}", i), Span::call_site()))
            .collect()
    }

    pub fn version_struct_idents(&self) -> Vec<Ident> {
        self.range
            .clone()
            .into_iter()
            .map(|i| Ident::new(&format!("{}V{}", self.enum_name, i), Span::call_site()))
            .collect()
    }

    pub fn version_numbers(&self) -> Vec<u8> {
        self.range.clone().into_iter().collect()
    }

    pub fn version_method_idents(&self) -> Vec<Ident> {
        self.range
            .clone()
            .into_iter()
            .map(|i| Ident::new(&format!("as_v{}", i), Span::call_site()))
            .collect()
    }
}

impl Parse for DeclareVersionedArgs {
    fn parse(input: ParseStream) -> SynResult<Self> {
        let enum_name: Ident = input.parse()?;
        let start_version: LitInt = input.parse()?;
        let start_version: u8 = start_version.base10_parse()?;
        input.parse::<Token![..]>()?;
        let end_version: LitInt = input.parse()?;
        let end_version: u8 = end_version.base10_parse()?;
        if end_version <= start_version {
            return Err(SynError::new(
                Span::call_site(),
                "The end version should be higher then the start version.",
            ));
        }
        Ok(Self {
            enum_name,
            range: start_version..end_version,
        })
    }
}

fn impl_declare_versioned(
    args: DeclareVersionedArgs,
    with_scale: bool,
    with_json: bool,
) -> TokenStream {
    let version_idents = args.version_idents();
    let version_struct_idents = args.version_struct_idents();
    let version_numbers = args.version_numbers();
    let version_method_idents = args.version_method_idents();
    let range_end = args.range.end;
    let range_start = args.range.start;
    let enum_name = args.enum_name;
    let scale_impl = if with_scale {
        quote! (
            impl iroha_version::scale::DecodeVersioned for #enum_name {
                fn decode_versioned(input: &[u8]) -> Result<Self, String> {
                    use iroha_version::{Version, UnsupportedVersion, RawVersioned};
                    use parity_scale_codec::Decode;

                    if let Some(version) = input.first() {
                        if Self::supported_versions().contains(version) {
                            let mut input = input.clone();
                            Self::decode(&mut input).map_err(|err| err.to_string())
                        } else {
                            Ok(Self::UnsupportedVersion(UnsupportedVersion::new(*version, RawVersioned::ScaleBytes(input.to_vec()))))
                        }
                    } else {
                        Err("This is not a versioned object. No version information found.".to_string())
                    }
                }
            }

            impl iroha_version::scale::EncodeVersioned for #enum_name {
                fn encode_versioned(&self) -> Result<Vec<u8>, String> {
                    use iroha_version::{UnsupportedVersion, RawVersioned};
                    use parity_scale_codec::Encode;

                    if let Self::UnsupportedVersion(unsupported_version) = self {
                        match &unsupported_version.raw {
                            RawVersioned::ScaleBytes(bytes) => Ok(bytes.clone()),
                            RawVersioned::Json(_) => Err("Can not encode unsupported version from json to scale.".to_string())
                        }
                    } else {
                        Ok(self.encode())
                    }
                }
            }
        )
    } else {
        quote!()
    };
    let scale_derives = if with_scale {
        quote!(parity_scale_codec::Encode, parity_scale_codec::Decode,)
    } else {
        quote!()
    };
    let scale_variant_attributes: Vec<_> = version_numbers
        .iter()
        .map(|version| {
            if with_scale {
                let version = version.to_string();
                quote!(#[codec(index = #version)])
            } else {
                quote!()
            }
        })
        .collect();
    let version_field_name = VERSION_FIELD_NAME;
    let json_impl = if with_json {
        quote!(
            impl<'a> iroha_version::json::DeserializeVersionedJson<'a> for #enum_name {
                fn from_versioned_json_str(input: &str) -> Result<Self, String> {
                    use iroha_version::{Version, UnsupportedVersion, RawVersioned};
                    use std::convert::TryInto;
                    use serde_json::Value;

                    let json: Value = serde_json::from_str(input).map_err(|err| format!("{}", err))?;
                    if let Value::Object(map) = json {
                        if let Some(Value::String(version_number)) = map.get(#version_field_name) {
                            let version: u8 = version_number.parse().map_err(|err| format!("{}", err))?;
                            if Self::supported_versions().contains(&version) {
                                serde_json::from_str(input).map_err(|err| format!("{}", err))
                            } else {
                                Ok(Self::UnsupportedVersion(UnsupportedVersion::new(version, RawVersioned::Json(input.to_string()))))
                            }
                        } else {
                            Err("This is not a versioned object. No version information found.".to_string())
                        }
                    } else {
                        Err("Expected json object.".to_string())
                    }
                }
            }

            impl iroha_version::json::SerializeVersionedJson for #enum_name {
                fn to_versioned_json_str(&self) -> Result<String, String> {
                    use iroha_version::{Version, UnsupportedVersion, RawVersioned};

                    if let Self::UnsupportedVersion(unsupported_version) = self {
                        match &unsupported_version.raw {
                            RawVersioned::ScaleBytes(_) => Err("Can not serialize unsupported version from scale to json.".to_string()),
                            RawVersioned::Json(json) => Ok(json.to_string())
                        }
                    } else {
                        serde_json::to_string(self).map_err(|err| format!("{}", err))
                    }
                }
            }
        )
    } else {
        quote!()
    };
    let json_derives = if with_json {
        quote!(serde::Serialize, serde::Deserialize,)
    } else {
        quote!()
    };
    let content_field_name = CONTENT_FIELD_NAME;
    let json_enum_attribute = if with_json {
        quote!(#[serde(tag = #version_field_name, content = #content_field_name)])
    } else {
        quote!()
    };
    let json_variant_attributes: Vec<_> = version_numbers
        .iter()
        .map(|version| {
            if with_json {
                let version = version.to_string();
                quote!(#[serde(rename = #version)])
            } else {
                quote!()
            }
        })
        .collect();
    quote!(
        #json_enum_attribute
        #[derive(Debug, Clone, iroha_derive::FromVariant, #scale_derives #json_derives)]
        pub enum #enum_name {
            #(#scale_variant_attributes #json_variant_attributes #version_idents (#version_struct_idents)),* ,
            UnsupportedVersion(iroha_version::UnsupportedVersion)
        }

        impl iroha_version::Version for #enum_name {
            fn version(&self) -> u8 {
                use #enum_name::*;
                match self {
                    #(#version_idents (_) => #version_numbers),* ,
                    UnsupportedVersion(unsupported_version) => unsupported_version.version
                }
            }

            fn supported_versions() -> std::ops::Range<u8> {
                #range_start .. #range_end
            }
        }

        impl #enum_name {
            #(
            pub fn #version_method_idents (&self) -> Option<& #version_struct_idents> {
                use #enum_name::*;

                match self {
                    #version_idents (content) => Some(content),
                    _ => None
                }
            }
            )*
        }

        #scale_impl

        #json_impl
    )
    .into()
}
