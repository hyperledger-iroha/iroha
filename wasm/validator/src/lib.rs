//! API for *Runtime Validators*.

#![no_std]

extern crate alloc;

use alloc::string::String;

use iroha_wasm::data_model::{prelude::*, validator::Verdict};
pub use iroha_wasm::{self, data_model, ExecuteOnHost};

pub mod prelude {
    //! Contains useful re-exports

    pub use iroha_validator_derive::{entrypoint, Token, ValidateGrantRevoke};
    pub use iroha_wasm::{
        data_model::{prelude::*, validator::Verdict},
        prelude::*,
        Context,
    };

    pub use super::traits::{Token, ValidateGrantRevoke};
    pub use crate::{declare_tokens, deny, pass, pass_if};
}

mod macros {
    //! Contains useful macros

    /// Shortcut for `return Verdict::Pass`.
    #[macro_export]
    macro_rules! pass {
        () => {
            return $crate::iroha_wasm::data_model::validator::Verdict::Pass
        };
    }

    /// Macro to return [`Verdict::Pass`](crate::data_model::validator::Verdict::Pass)
    /// if the expression is `true`.
    ///
    /// # Example
    ///
    /// ```no_run
    /// pass_if!(asset_id.account_id() == authority);
    /// ```
    #[macro_export]
    macro_rules! pass_if {
        ($e:expr) => {
            if $e {
                return $crate::iroha_wasm::data_model::validator::Verdict::Pass;
            }
        };
    }

    /// Shortcut for `return Verdict::Deny(...)`.
    ///
    /// Supports [`format!`](alloc::format) syntax as well as any expression returning [`String`](alloc::string::String).
    ///
    /// # Example
    ///
    /// ```no_run
    /// deny!("Some reason");
    /// deny!("Reason: {}", reason);
    /// deny!("Reason: {reason}");
    /// deny!(get_reason());
    /// ```
    #[macro_export]
    macro_rules! deny {
        ($l:literal $(,)?) => {
            return $crate::iroha_wasm::data_model::validator::Verdict::Deny(
                ::alloc::fmt::format(::core::format_args!($l))
            )
        };
        ($e:expr $(,)?) =>{
            return $crate::iroha_wasm::data_model::validator::Verdict::Deny($e)
        };
        ($fmt:expr, $($arg:tt)*) => {
            return $crate::iroha_wasm::data_model::validator::Verdict::Deny(
                ::alloc::format!($fmt, $($arg)*)
            )
        };
    }

    /// Macro to return [`Verdict::Deny`](crate::data_model::validator::Verdict::Deny)
    /// if the expression is `true`.
    ///
    /// # Example
    ///
    /// ```no_run
    /// deny_if!(asset_id.account_id() != authority, "You have to be an asset owner");
    /// deny_if!(asset_id.account_id() != authority, "You have to be an {} owner", asset_id);
    /// deny_if!(asset_id.account_id() != authority, construct_reason(&asset_id));
    /// ```
    #[macro_export]
    macro_rules! deny_if {
        ($e:expr, $l:literal $(,)?) => {
            if $e {
                deny!($l);
            }
        };
        ($e:expr, $r:expr $(,)?) =>{
            if $e {
                deny!($r);
            }
        };
        ($e:expr, $fmt:expr, $($arg:tt)*) => {
            if $e {
                deny!($fmt, $($arg)*);
            }
        };
    }

    /// Macro to parse literal as a type. Panics if failed.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use iroha_wasm::parse;
    /// use data_model::prelude::*;
    ///
    /// let account_id = parse!("alice@wonderland" as <Account as Identifiable>::Id);
    /// ```
    #[macro_export]
    macro_rules! parse {
        ($l:literal as _) => {
            compile_error!(
                "Don't use `_` as a type in this macro, \
                 otherwise panic message would be less informative"
            )
        };
        ($l:literal as $t:ty) => {
            $crate::iroha_wasm::debug::DebugExpectExt::dbg_expect(
                $l.parse::<$t>(),
                concat!("Failed to parse `", $l, "` as `", stringify!($t), "`"),
            )
        };
    }

    /// Declare token types of current module. Use it with a full path to the token.
    ///
    /// Used to iterate over token types to validate `Grant` and `Revoke` instructions.
    ///
    ///
    /// TODO: Replace with procedural macro. Example:
    /// ```
    /// #[tokens(path = "crate::current_module")]
    /// mod tokens {
    ///     #[derive(Token, ...)]
    ///     pub struct MyToken;
    /// }
    /// ```
    #[macro_export]
    macro_rules! declare_tokens {
        ($($token_ty:ty),+ $(,)?) => {
            macro_rules! map_tokens {
                ($callback:ident) => {$(
                    $callback!($token_ty)
                );+}
            }

            pub(crate) use map_tokens;
        }
    }

    #[cfg(test)]
    mod tests {
        //! Tests in this modules can't be doc-tests because of `compile_error!` on native target
        //! and `webassembly-test-runner` on wasm target.

        use webassembly_test::webassembly_test;

        use crate::{alloc::borrow::ToOwned as _, data_model::validator::Verdict, deny};

        #[webassembly_test]
        fn test_deny() {
            let a = || deny!("Some reason");
            assert_eq!(a(), Verdict::Deny("Some reason".to_owned()));

            let get_reason = || "Reason from expression".to_owned();
            let b = || deny!(get_reason());
            assert_eq!(b(), Verdict::Deny("Reason from expression".to_owned()));

            let mes = "Format message";
            let c = || deny!("Reason: {}", mes);
            assert_eq!(c(), Verdict::Deny("Reason: Format message".to_owned()));

            let mes = "Advanced format message";
            let d = || deny!("Reason: {mes}");
            assert_eq!(
                d(),
                Verdict::Deny("Reason: Advanced format message".to_owned())
            );
        }

        #[webassembly_test]
        fn test_deny_if() {
            let a = || {
                deny_if!(true, "Some reason");
                unreachable!()
            };
            assert_eq!(a(), Verdict::Deny("Some reason".to_owned()));

            let get_reason = || "Reason from expression".to_owned();
            let b = || {
                deny_if!(true, get_reason());
                unreachable!()
            };
            assert_eq!(b(), Verdict::Deny("Reason from expression".to_owned()));

            let mes = "Format message";
            let c = || {
                deny_if!(true, "Reason: {}", mes);
                unreachable!()
            };
            assert_eq!(c(), Verdict::Deny("Reason: Format message".to_owned()));

            let mes = "Advanced format message";
            let d = || {
                deny_if!(true, "Reason: {mes}");
                unreachable!()
            };
            assert_eq!(
                d(),
                Verdict::Deny("Reason: Advanced format message".to_owned())
            );
        }
    }
}

/// Error type for `TryFrom<PermissionToken>` implementations.
#[derive(Debug, Clone)]
pub enum PermissionTokenConversionError {
    /// Unexpected token id.
    Id(PermissionTokenId),
    /// Missing parameter.
    Param(&'static str),
    // TODO: Improve this error
    /// Unexpected parameter value.
    Value(String),
}

pub mod traits {
    //! Contains traits related to validators

    use super::*;

    /// [`Token`] trait is used to check if the token is owned by the account.
    pub trait Token:
        TryFrom<PermissionToken, Error = PermissionTokenConversionError> + ValidateGrantRevoke
    {
        /// Get definition id of this token
        fn definition_id() -> PermissionTokenId;

        /// Check if token is owned by the account using evaluation on host.
        ///
        /// Basically it's a wrapper around [`DoesAccountHavePermissionToken`] query.
        fn is_owned_by(&self, account_id: &<Account as Identifiable>::Id) -> bool;
    }

    /// Trait that should be implemented for all permission tokens.
    /// Provides a function to check validity of [`Grant`] and [`Revoke`]
    /// instructions containing implementing token.
    pub trait ValidateGrantRevoke {
        /// Validate [`Grant`] instruction for this token.
        fn validate_grant(&self, authority: &<Account as Identifiable>::Id) -> Verdict;

        /// Validate [`Revoke`] instruction for this token.
        fn validate_revoke(&self, authority: &<Account as Identifiable>::Id) -> Verdict;
    }
}

pub mod pass_conditions {
    //! Contains some common pass conditions used in [`ValidateGrantRevoke`](crate::data_model::validator::prelude::ValidateGrantRevoke)

    use super::*;

    /// Predicate-like trait used for pass conditions to identify if [`Grant`] or [`Revoke`] should be allowed.
    pub trait PassCondition {
        fn validate(&self, authority: &<Account as Identifiable>::Id) -> Verdict;
    }

    pub mod derive_conversions {
        //! Module with derive macros to generate conversion from custom strongly-typed token
        //! to some pass condition to successfully derive [`ValidateGrantRevoke`](iroha_validator_derive::ValidateGrantRevoke)

        pub mod asset {
            //! Module with derives related to asset tokens

            pub use iroha_validator_derive::RefIntoAssetOwner as Owner;
        }

        pub mod asset_definition {
            //! Module with derives related to asset definition tokens

            pub use iroha_validator_derive::RefIntoAssetDefinitionOwner as Owner;
        }

        pub mod account {
            //! Module with derives related to account tokens

            pub use iroha_validator_derive::RefIntoAccountOwner as Owner;
        }
    }

    pub mod asset {
        //! Module with pass conditions for assets-related to tokens

        use super::*;

        /// Pass condition that checks if `authority` is the owner of `asset_id`.
        #[derive(Debug, Clone)]
        pub struct Owner<'asset> {
            pub asset_id: &'asset <Asset as Identifiable>::Id,
        }

        impl PassCondition for Owner<'_> {
            fn validate(&self, authority: &<Account as Identifiable>::Id) -> Verdict {
                pass_if!(self.asset_id.account_id() == authority);
                deny!("Can't give permission to access asset owned by another account")
            }
        }
    }

    pub mod asset_definition {
        //! Module with pass conditions for asset definitions related to tokens

        use super::*;

        /// Pass condition that checks if `authority` is the owner of `asset_definition_id`.
        #[derive(Debug, Clone)]
        pub struct Owner<'asset_definition> {
            pub asset_definition_id: &'asset_definition <AssetDefinition as Identifiable>::Id,
        }

        impl PassCondition for Owner<'_> {
            fn validate(&self, authority: &<Account as Identifiable>::Id) -> Verdict {
                pass_if!(utils::is_asset_definition_owner(
                    self.asset_definition_id,
                    authority
                ));
                deny!("Can't give permission to access asset definition owned by another account")
            }
        }
    }

    pub mod account {
        //! Module with pass conditions for assets-related to tokens

        use super::*;

        /// Pass condition that checks if `authority` is the owner of `account_id`.
        #[derive(Debug, Clone)]
        pub struct Owner<'asset> {
            pub account_id: &'asset <Account as Identifiable>::Id,
        }

        impl PassCondition for Owner<'_> {
            fn validate(&self, authority: &<Account as Identifiable>::Id) -> Verdict {
                pass_if!(self.account_id == authority);
                deny!("Can't give permission to access another account")
            }
        }
    }

    /// Pass condition that always passes.
    #[derive(Debug, Default, Copy, Clone)]
    pub struct AlwaysPass;

    impl PassCondition for AlwaysPass {
        fn validate(&self, _: &<Account as Identifiable>::Id) -> Verdict {
            pass!()
        }
    }

    impl<T: traits::Token> From<&T> for AlwaysPass {
        fn from(_: &T) -> Self {
            Self::default()
        }
    }

    /// Pass condition that allows operation only in genesis.
    ///
    /// In other words it always denies the operation, because runtime validators are not used
    /// in genesis validation.
    #[derive(Debug, Default, Copy, Clone)]
    pub struct OnlyGenesis;

    impl PassCondition for OnlyGenesis {
        fn validate(&self, _: &<Account as Identifiable>::Id) -> Verdict {
            deny!("This operation is always denied and only allowed inside the genesis block")
        }
    }

    impl<T: traits::Token> From<&T> for OnlyGenesis {
        fn from(_: &T) -> Self {
            Self::default()
        }
    }
}

pub mod utils {
    //! Contains some utils for validators

    use super::*;

    /// Check if `authority` is the owner of `asset_definition_id`.
    ///
    /// Wrapper around [`IsAssetDefinitionOwner`](crate::data_model::prelude::IsAssetDefinitionOwner) query.
    pub fn is_asset_definition_owner(
        asset_definition_id: &<AssetDefinition as Identifiable>::Id,
        authority: &<Account as Identifiable>::Id,
    ) -> bool {
        use iroha_wasm::{debug::DebugExpectExt as _, ExecuteOnHost as _};

        QueryBox::from(IsAssetDefinitionOwner::new(
            asset_definition_id.clone(),
            authority.clone(),
        ))
        .execute()
        .try_into()
        .dbg_expect("Failed to convert `IsAssetDefinitionOwner` query result into `bool`")
    }
}
