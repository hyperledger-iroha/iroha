//! Module [`validator_entrypoint`](crate::validator_entrypoint) macro implementation

use super::*;

mod export {
    pub const VALIDATOR_VALIDATE_TRANSACTION: &str = "_iroha_validator_validate_transaction";
    pub const VALIDATOR_VALIDATE_INSTRUCTION: &str = "_iroha_validator_validate_instruction";
    pub const VALIDATOR_VALIDATE_QUERY: &str = "_iroha_validator_validate_query";
    pub const VALIDATOR_MIGRATE: &str = "_iroha_validator_migrate";
}

mod import {
    pub const GET_VALIDATE_TRANSACTION_PAYLOAD: &str = "get_validate_transaction_payload";
    pub const GET_VALIDATE_INSTRUCTION_PAYLOAD: &str = "get_validate_instruction_payload";
    pub const GET_VALIDATE_QUERY_PAYLOAD: &str = "get_validate_query_payload";
}

/// [`validator_entrypoint`](crate::validator_entrypoint()) macro implementation
#[allow(clippy::needless_pass_by_value)]
pub fn impl_entrypoint(attr: TokenStream, item: TokenStream) -> TokenStream {
    let fn_item = parse_macro_input!(item as syn::ItemFn);

    assert!(
        attr.is_empty(),
        "`#[entrypoint]` macro for Validator entrypoints accepts no attributes"
    );

    macro_rules! match_entrypoints {
        (validate: {
            $($user_entrypoint_name:ident =>
                $generated_entrypoint_name:ident ($query_validating_object_fn_name:ident)),* $(,)?
        }
        other: {
            $($other_user_entrypoint_name:ident => $branch:block),* $(,)?
        }) => {
            match &fn_item.sig.ident {
                $(fn_name if fn_name == stringify!($user_entrypoint_name) => {
                    impl_validate_entrypoint(
                        fn_item,
                        stringify!($user_entrypoint_name),
                        export::$generated_entrypoint_name,
                        import::$query_validating_object_fn_name,
                    )
                })*
                $(fn_name if fn_name == stringify!($other_user_entrypoint_name) => $branch),*
                _ => panic!(
                    "Validator entrypoint name must be one of: {:?}",
                    [
                        $(stringify!($user_entrypoint_name),)*
                        $(stringify!($other_user_entrypoint_name),)*
                    ]
                ),
            }
        };
    }

    match_entrypoints! {
        validate: {
            validate_transaction => VALIDATOR_VALIDATE_TRANSACTION(GET_VALIDATE_TRANSACTION_PAYLOAD),
            validate_instruction => VALIDATOR_VALIDATE_INSTRUCTION(GET_VALIDATE_INSTRUCTION_PAYLOAD),
            validate_query => VALIDATOR_VALIDATE_QUERY(GET_VALIDATE_QUERY_PAYLOAD),
        }
        other: {
            migrate => { impl_migrate_entrypoint(fn_item) }
        }
    }
}

fn impl_validate_entrypoint(
    fn_item: syn::ItemFn,
    user_entrypoint_name: &'static str,
    generated_entrypoint_name: &'static str,
    get_validation_payload_fn_name: &'static str,
) -> TokenStream {
    let syn::ItemFn {
        attrs,
        vis,
        sig,
        mut block,
    } = fn_item;
    let fn_name = &sig.ident;

    assert!(
        matches!(sig.output, syn::ReturnType::Type(_, _)),
        "Validator `{user_entrypoint_name}` entrypoint must have `Result` return type"
    );

    block.stmts.insert(
        0,
        parse_quote!(
            use ::iroha_validator::smart_contract::{ExecuteOnHost as _, QueryHost as _};
        ),
    );

    let generated_entrypoint_ident: syn::Ident = syn::parse_str(generated_entrypoint_name)
        .expect("Provided entrypoint name to generate is not a valid Ident, this is a bug");

    let get_validation_payload_fn_ident: syn::Ident =
        syn::parse_str(get_validation_payload_fn_name).expect(
            "Provided function name to query validating object is not a valid Ident, this is a bug",
        );

    quote! {
        /// Validator `validate` entrypoint
        ///
        /// # Memory safety
        ///
        /// This function transfers the ownership of allocated
        /// [`Result`](::iroha_validator::data_model::validator::Result)
        #[no_mangle]
        #[doc(hidden)]
        unsafe extern "C" fn #generated_entrypoint_ident() -> *const u8 {
            let payload = ::iroha_validator::#get_validation_payload_fn_ident();
            let verdict: ::iroha_validator::data_model::validator::Result =
                #fn_name(payload.authority, payload.to_validate, payload.block_height);
            let bytes_box = ::core::mem::ManuallyDrop::new(::iroha_validator::utils::encode_with_length_prefix(&verdict));

            bytes_box.as_ptr()
        }

        // NOTE: Host objects are always passed by value to wasm
        #[allow(clippy::needless_pass_by_value)]
        #(#attrs)*
        #vis #sig
        #block
    }
    .into()
}

fn impl_migrate_entrypoint(fn_item: syn::ItemFn) -> TokenStream {
    let syn::ItemFn {
        attrs,
        vis,
        sig,
        block,
    } = fn_item;
    let fn_name = &sig.ident;

    assert!(
        matches!(sig.output, syn::ReturnType::Type(_, _)),
        "Validator `migrate()` entrypoint must have `MigrationResult` return type"
    );

    let migrate_fn_name =
        syn::Ident::new(export::VALIDATOR_MIGRATE, proc_macro2::Span::call_site());

    quote! {
        /// Validator `permission_token_schema` entrypoint
        ///
        /// # Memory safety
        ///
        /// This function transfers the ownership of allocated [`Vec`](alloc::vec::Vec).
        #[no_mangle]
        #[doc(hidden)]
        unsafe extern "C" fn #migrate_fn_name() -> *const u8 {
            let payload = ::iroha_validator::get_migrate_payload();
            let res: ::iroha_validator::data_model::validator::MigrationResult = #fn_name(payload.block_height);
            let bytes = ::core::mem::ManuallyDrop::new(::iroha_validator::utils::encode_with_length_prefix(&res));

            ::core::mem::ManuallyDrop::new(bytes).as_ptr()
        }

        // NOTE: False positive
        #[allow(clippy::unnecessary_wraps)]
        #(#attrs)*
        #vis #sig
        #block
    }
    .into()
}
