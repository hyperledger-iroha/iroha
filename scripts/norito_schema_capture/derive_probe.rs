//! Test-only compiler probes for an isolated Norito identity capture snapshot.
//!
//! Shipping derives never load this module. A prepared snapshot appends the
//! generated test beside an existing codec implementation; codec tokens remain
//! unchanged. Generic and unregistered local items still require explicit review.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{DeriveInput, ext::IdentExt as _};

/// Existing codec direction whose real schema hash will be inspected.
#[derive(Clone, Copy)]
pub(crate) enum Direction {
    /// Inspect the already generated serializer.
    Serialize,
    /// Inspect the already generated deserializer.
    Deserialize,
}

/// Generate a harness probe without constructing or changing a wire value.
pub(crate) fn probe(
    input: &DeriveInput,
    direction: Direction,
    root_override: Option<&str>,
) -> TokenStream {
    let ident = &input.ident;
    let bare_ident = ident.unraw();
    let direction_name = match direction {
        Direction::Serialize => "serialize",
        Direction::Deserialize => "deserialize",
    };
    let function = format_ident!("__norito_schema_capture_{}_{}", direction_name, bare_ident);
    let body = if input.generics.params.is_empty() {
        let hash = match direction {
            Direction::Serialize => {
                quote!(<#ident as norito::core::NoritoSerialize>::schema_hash())
            }
            Direction::Deserialize => {
                quote!(<#ident as norito::core::NoritoDeserialize<'static>>::schema_hash())
            }
        };
        let root = root_override.map_or_else(
            || quote!(::std::any::type_name::<#ident>()),
            |name| quote!(#name),
        );
        let explicit_root = root_override.is_some();
        let declared_identity = input
            .attrs
            .iter()
            .any(|attribute| attribute.path().is_ident("norito_schema"));
        let check_declaration = declared_identity.then(|| {
            quote! {
                // During declaration preparation, the explicit contract must
                // match this independently generated, still-active codec.
                ::std::assert_eq!(<#ident as norito::NoritoSchema>::nominal_name(), nominal);
                ::std::assert_eq!(norito::schema::identity::frame_hash::<#ident>(), actual_hash);
            }
        });
        quote! {
            let nominal = ::std::any::type_name::<#ident>();
            let root_hint = #root;
            let actual_hash = #hash;
            // The actual codec hash is evidence even if a structural or other
            // projection does not match the name hint. Such records stay in
            // the review queue; capture must not invent a matching name.
            let root_matches = actual_hash == norito::core::schema_hash_for_name(root_hint);
            #check_declaration
            ::std::println!(
                "NORITO_SCHEMA_CAPTURE_V1\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                #direction_name, hex(nominal.as_bytes()), hex(root_hint.as_bytes()),
                hex(&actual_hash), hex(::std::file!().as_bytes()), ::std::line!(),
                ::std::column!(), hex(::std::stringify!(#ident).as_bytes()),
                hex(::std::module_path!().as_bytes()), #explicit_root, root_matches,
            );
        }
    } else {
        quote! {
            ::std::println!(
                "NORITO_SCHEMA_CAPTURE_SKIP_V1\t{}\t{}\t{}\t{}\t{}\t{}\tgeneric",
                #direction_name, hex(::std::stringify!(#ident).as_bytes()),
                hex(::std::file!().as_bytes()), ::std::line!(), ::std::column!(),
                hex(::std::module_path!().as_bytes()),
            );
        }
    };
    quote! {
        // Function-local tests are not registered by Rust. Their absent records
        // remain explicit source-inventory gaps, never covered captures.
        #[allow(non_snake_case, dead_code)]
        #[cfg(test)]
        #[test]
        #[doc = "Capture one compiler-resolved codec identity in an isolated source snapshot."]
        fn #function() {
            fn hex(bytes: &[u8]) -> ::std::string::String {
                use ::std::fmt::Write as _;
                let mut result = ::std::string::String::with_capacity(bytes.len() * 2);
                for byte in bytes {
                    ::std::write!(&mut result, "{byte:02x}").expect("string formatting");
                }
                result
            }
            #body
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn generated(source: &str, direction: Direction, root: Option<&str>) -> syn::ItemFn {
        let input = syn::parse_str::<DeriveInput>(source).expect("input declaration");
        syn::parse2(probe(&input, direction, root)).expect("one valid test function")
    }

    #[test]
    fn private_nongeneric_type_has_a_real_serializer_probe() {
        let function = generated("struct Private(u64);", Direction::Serialize, None);
        assert_eq!(
            function.sig.ident,
            "__norito_schema_capture_serialize_Private"
        );
        let text = quote!(#function).to_string();
        assert!(text.contains("type_name :: < Private >"));
        assert!(text.contains("NoritoSerialize > :: schema_hash"));
        assert!(text.contains("NORITO_SCHEMA_CAPTURE_V1"));
        assert!(!text.contains("SKIP_V1"));
        assert!(
            function
                .attrs
                .iter()
                .any(|attribute| attribute.path().is_ident("test"))
        );
        assert!(
            function
                .attrs
                .iter()
                .any(|attribute| attribute.path().is_ident("cfg"))
        );
    }

    #[test]
    fn decoder_probe_does_not_require_a_serializer() {
        let function = generated("enum ReadOnly { Ready }", Direction::Deserialize, None);
        assert_eq!(
            function.sig.ident,
            "__norito_schema_capture_deserialize_ReadOnly"
        );
        let text = quote!(#function).to_string();
        assert!(text.contains("NoritoDeserialize < 'static >"));
        assert!(!text.contains("NoritoSerialize"));
    }

    #[test]
    fn explicit_root_remains_distinct_from_actual_nominal_name() {
        let function = generated(
            "struct Renamed;",
            Direction::Serialize,
            Some("protocol::Fixed"),
        );
        let text = quote!(#function).to_string();
        assert!(text.contains("type_name :: < Renamed >"));
        assert!(text.contains("\"protocol::Fixed\""));
        assert!(text.contains("root_matches"));
        assert!(text.contains("schema_hash_for_name"));
    }

    #[test]
    fn declared_identity_is_checked_against_the_existing_codec() {
        let declared = generated(
            "#[norito_schema(name = \"protocol::Declared\")] struct Declared;",
            Direction::Serialize,
            None,
        );
        let text = quote!(#declared).to_string();
        assert!(text.contains("NoritoSchema > :: nominal_name"));
        assert!(text.contains("frame_hash :: < Declared >"));
        let ordinary = generated("struct Ordinary;", Direction::Serialize, None);
        assert!(!quote!(#ordinary).to_string().contains("NoritoSchema"));
    }

    #[test]
    fn generic_slots_are_reported_without_invented_instantiations() {
        for source in [
            "struct Generic<T>(T);",
            "struct Borrowed<'a>(&'a str);",
            "struct Const<const N: usize>([u8; N]);",
            "struct Defaulted<T = u8>(T);",
        ] {
            let function = generated(source, Direction::Serialize, None);
            let text = quote!(#function).to_string();
            assert!(text.contains("NORITO_SCHEMA_CAPTURE_SKIP_V1"));
            assert!(!text.contains("type_name"));
            assert!(!text.contains("schema_hash"));
        }
    }

    #[test]
    fn direction_and_case_preserve_distinct_harness_names() {
        let upper = generated("struct Value;", Direction::Serialize, None);
        let lower = generated("struct value;", Direction::Serialize, None);
        let decode = generated("struct Value;", Direction::Deserialize, None);
        assert_ne!(upper.sig.ident, lower.sig.ident);
        assert_ne!(upper.sig.ident, decode.sig.ident);
    }

    #[test]
    fn raw_identifiers_form_valid_probe_functions() {
        let function = generated("struct r#type;", Direction::Serialize, None);
        assert_eq!(function.sig.ident, "__norito_schema_capture_serialize_type");
        assert!(
            quote!(#function)
                .to_string()
                .contains("type_name :: < r#type >")
        );
    }

    #[test]
    fn real_compiler_runs_private_macro_and_generic_probes() {
        use std::{fs, path::PathBuf, process::Command, time::SystemTime};

        struct Scratch(PathBuf);
        impl Drop for Scratch {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
        let nonce = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("current clock")
            .as_nanos();
        let scratch = Scratch(std::env::temp_dir().join(format!(
            "norito-capture-compiler-{}-{nonce}",
            std::process::id()
        )));
        fs::create_dir(&scratch.0).expect("new compiler fixture directory");
        let private_encode = generated(
            "struct Hidden;",
            Direction::Serialize,
            Some("declared::Root"),
        );
        let private_decode = generated("struct Hidden;", Direction::Deserialize, None);
        let macro_probe = generated("struct Generated;", Direction::Serialize, None);
        let generic_probe = generated("struct Generic<T>(T);", Direction::Serialize, None);
        let raw_probe = generated("struct r#type;", Direction::Serialize, None);
        let local_probe = generated("struct Local;", Direction::Serialize, None);
        let unmatched_probe = generated("struct Unmatched;", Direction::Serialize, None);
        let source = quote! {
            #![deny(warnings)]
            extern crate self as norito;
            pub mod core {
                pub trait NoritoSerialize { fn schema_hash() -> [u8; 16]; }
                pub trait NoritoDeserialize<'a> { fn schema_hash() -> [u8; 16]; }
                pub fn schema_hash_for_name(_: &str) -> [u8; 16] { [7; 16] }
            }
            mod private {
                struct Hidden;
                impl crate::core::NoritoSerialize for Hidden { fn schema_hash() -> [u8; 16] { [7; 16] } }
                impl<'a> crate::core::NoritoDeserialize<'a> for Hidden { fn schema_hash() -> [u8; 16] { [7; 16] } }
                #private_encode
                #private_decode
            }
            macro_rules! make {
                () => {
                    struct Generated;
                    impl crate::core::NoritoSerialize for Generated { fn schema_hash() -> [u8; 16] { [7; 16] } }
                    #macro_probe
                };
            }
            make!();
            pub struct Generic<T>(pub T);
            #generic_probe
            #[allow(non_camel_case_types)]
            struct r#type;
            impl crate::core::NoritoSerialize for r#type { fn schema_hash() -> [u8; 16] { [7; 16] } }
            #raw_probe
            struct Unmatched;
            impl crate::core::NoritoSerialize for Unmatched { fn schema_hash() -> [u8; 16] { [9; 16] } }
            #unmatched_probe
            // This fixture explicitly exercises Rust's unregistered local
            // tests. Ordinary capture sources retain their own lint policy.
            #[allow(unnameable_test_items)]
            fn local_container() {
                struct Local;
                impl crate::core::NoritoSerialize for Local { fn schema_hash() -> [u8; 16] { [7; 16] } }
                #local_probe
                let _ = Local;
            }
            #[test]
            fn exercise_local_container() { local_container(); }
        };
        let input = scratch.0.join("fixture.rs");
        let executable = scratch.0.join("fixture-tests");
        fs::write(&input, source.to_string()).expect("write compiler fixture");
        let compiled = Command::new("rustc")
            .args(["--edition=2024", "--crate-name=capture_fixture", "--test"])
            .arg(&input)
            .arg("-o")
            .arg(&executable)
            .output()
            .expect("run workspace compiler");
        assert!(
            compiled.status.success(),
            "{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let output = Command::new(&executable)
            .args([
                "__norito_schema_capture_",
                "--nocapture",
                "--test-threads=1",
            ])
            .output()
            .expect("run compiler-generated probes");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).expect("UTF-8 capture output");
        assert!(stdout.contains("6 passed; 0 failed; 0 ignored"), "{stdout}");
        assert_eq!(stdout.matches("NORITO_SCHEMA_CAPTURE_V1\t").count(), 5);
        assert_eq!(stdout.matches("NORITO_SCHEMA_CAPTURE_SKIP_V1\t").count(), 1);
        assert!(
            stdout.contains("\tfalse\tfalse"),
            "unmatched projection was hidden: {stdout}"
        );
        assert!(!stdout.contains("__norito_schema_capture_serialize_Local"));
    }
}
