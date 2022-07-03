#![allow(clippy::module_name_repetitions)]

//! This module contains permissions related Iroha functionality.

use std::{fmt::Debug, marker::PhantomData};

pub use checks::*;
use error::*;
pub use has_token::*;
use iroha_data_model::prelude::*;
use iroha_macro::FromVariant;
use iroha_schema::IntoSchema;
pub use is_allowed::*;
use parity_scale_codec::{Decode, Encode};
use serde::{Deserialize, Serialize};

use crate::wsv::WorldStateView;

pub mod builder;
mod checks;
pub mod combinators;
mod has_token;
mod is_allowed;
pub mod judge;
pub mod roles;

/// Result type for permission validators
pub type Result<T> = std::result::Result<T, DenialReason>;

/// Operation for which the permission should be checked.
pub trait NeedsPermission: Debug {
    fn required_validator_type(&self) -> ValidatorType;
}

impl NeedsPermission for Instruction {
    fn required_validator_type(&self) -> ValidatorType {
        ValidatorType::Instruction
    }
}

impl NeedsPermission for QueryBox {
    fn required_validator_type(&self) -> ValidatorType {
        ValidatorType::Query
    }
}

// Expression might contain a query, therefore needs to be checked.
impl NeedsPermission for Expression {
    fn required_validator_type(&self) -> ValidatorType {
        ValidatorType::Expression
    }
}

#[derive(Debug, derive_more::From, derive_more::TryInto)]
pub enum NeedsPermissionBox {
    Instruction(Instruction),
    Query(QueryBox),
    Expression(Expression),
}

impl NeedsPermission for NeedsPermissionBox {
    fn required_validator_type(&self) -> ValidatorType {
        match self {
            NeedsPermissionBox::Instruction(_) => ValidatorType::Instruction,
            NeedsPermissionBox::Query(_) => ValidatorType::Query,
            NeedsPermissionBox::Expression(_) => ValidatorType::Expression,
        }
    }
}

/// Type of object validator can check
#[derive(Debug, Copy, Clone, PartialEq, Eq, derive_more::Display, Encode, Decode, IntoSchema)]
pub enum ValidatorType {
    /// [`Instruction`] variant
    Instruction,
    /// [`QueryBox`] variant
    Query,
    /// [`Expression`] variant
    Expression,
}

/// Verdict returned by validators
#[derive(Debug, Clone, PartialEq, Eq, derive_more::Display, Encode, Decode, IntoSchema)]
pub enum ValidatorVerdict {
    /// Instruction is denied
    Deny(DenialReason),
    /// Instruction is skipped cause it is not supported by the validator
    Skip,
    /// Instruction is allowed
    Allow,
}

impl PartialOrd for ValidatorVerdict {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(Ord::cmp(self, other))
    }
}

impl Ord for ValidatorVerdict {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match (self, other) {
            (ValidatorVerdict::Deny(_), ValidatorVerdict::Deny(_)) => std::cmp::Ordering::Equal,
            (ValidatorVerdict::Deny(_), ValidatorVerdict::Skip) => std::cmp::Ordering::Less,
            (ValidatorVerdict::Deny(_), ValidatorVerdict::Allow) => std::cmp::Ordering::Less,
            (ValidatorVerdict::Skip, ValidatorVerdict::Deny(_)) => std::cmp::Ordering::Greater,
            (ValidatorVerdict::Skip, ValidatorVerdict::Skip) => std::cmp::Ordering::Equal,
            (ValidatorVerdict::Skip, ValidatorVerdict::Allow) => std::cmp::Ordering::Less,
            (ValidatorVerdict::Allow, ValidatorVerdict::Deny(_)) => std::cmp::Ordering::Greater,
            (ValidatorVerdict::Allow, ValidatorVerdict::Skip) => std::cmp::Ordering::Greater,
            (ValidatorVerdict::Allow, ValidatorVerdict::Allow) => std::cmp::Ordering::Equal,
        }
    }
}

impl ValidatorVerdict {
    pub fn is_allow(&self) -> bool {
        matches!(self, ValidatorVerdict::Allow)
    }

    pub fn is_deny(&self) -> bool {
        matches!(self, ValidatorVerdict::Deny(_))
    }

    pub fn is_skip(&self) -> bool {
        matches!(self, ValidatorVerdict::Skip)
    }

    pub fn least_permissive(self, other: Self) -> Self {
        std::cmp::min(self, other)
    }

    pub fn least_permissive_with(self, f: impl FnOnce() -> Self) -> Self {
        if let Self::Deny(_) = &self {
            self
        } else {
            self.least_permissive(f())
        }
    }

    pub fn most_permissive(self, other: Self) -> Self {
        std::cmp::max(self, other)
    }

    pub fn most_permissive_with(self, f: impl FnOnce() -> Self) -> Self {
        if let Self::Allow = &self {
            self
        } else {
            self.most_permissive(f())
        }
    }
}

impl From<Result<()>> for ValidatorVerdict {
    fn from(result: Result<()>) -> Self {
        match result {
            Ok(_) => ValidatorVerdict::Allow,
            Err(reason) => ValidatorVerdict::Deny(reason),
        }
    }
}

pub mod error {
    //! Contains errors structures

    use std::{convert::Infallible, str::FromStr};

    use super::{Decode, Encode, IntoSchema, ValidatorType};
    use crate::smartcontracts::Mismatch;

    /// Wrong validator expectation error
    ///
    /// I.e. used when user tries to validate [`QueryBox`](super::QueryBox) with
    /// [`IsAllowedBoxed`](super::IsAllowedBoxed) containing
    /// [`IsAllowedBoxed::Instruction`](super::IsAllowedBoxed::Instruction) variant
    pub type ValidatorTypeMismatch = Mismatch<ValidatorType>;

    /// Reason for prohibiting the execution of the particular instruction.
    #[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Decode, Encode, IntoSchema)]
    #[allow(variant_size_differences)]
    pub enum DenialReason {
        /// [`ValidatorTypeMismatch`] variant
        #[error("Wrong validator type: {0}")]
        ValidatorTypeMismatch(#[from] ValidatorTypeMismatch),
        /// Variant for custom error
        #[error("{0}")]
        Custom(String),
        /// Variant used when at least one [`Validator`](super::IsAllowed) should be provided
        #[error("No validators provided")]
        NoValidatorsProvided,
    }

    impl From<String> for DenialReason {
        fn from(s: String) -> Self {
            Self::Custom(s)
        }
    }

    impl FromStr for DenialReason {
        type Err = Infallible;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            Ok(Self::Custom(s.to_owned()))
        }
    }
}

pub mod prelude {
    //! Exports common types for permissions.

    pub use super::{
        builder::Validator as ValidatorBuilder,
        error::DenialReason,
        judge::{AllowAll, Judge, OperationJudgeBoxed},
        roles::{IsGrantAllowed, IsGrantAllowedBoxed, IsRevokeAllowed, IsRevokeAllowedBoxed},
        HasTokenBoxed, IsAllowed, IsAllowedBoxed, ValidatorVerdict,
    };
}

#[cfg(test)]
mod tests {
    #![allow(clippy::restriction)]

    use std::{collections::BTreeSet, str::FromStr as _};

    use iroha_data_model::{expression::prelude::*, isi::*};

    use super::{builder::Validator as ValidatorBuilder, judge::DenyAll, prelude::*, *};
    use crate::wsv::World;

    #[derive(Debug, Clone, Serialize)]
    struct DenyBurn;

    impl GetValidatorType for DenyBurn {
        fn get_validator_type(&self) -> ValidatorType {
            ValidatorType::Instruction
        }
    }

    impl IsAllowed for DenyBurn {
        type Operation = Instruction;

        fn check(
            &self,
            _authority: &AccountId,
            instruction: &Instruction,
            _wsv: &WorldStateView,
        ) -> ValidatorVerdict {
            match instruction {
                Instruction::Burn(_) => {
                    ValidatorVerdict::Deny("Denying sequence isi.".to_owned().into())
                }
                _ => ValidatorVerdict::Skip,
            }
        }
    }

    #[derive(Debug, Clone, Serialize)]
    struct DenyAlice;

    impl GetValidatorType for DenyAlice {
        fn get_validator_type(&self) -> ValidatorType {
            ValidatorType::Instruction
        }
    }

    impl IsAllowed for DenyAlice {
        type Operation = Instruction;

        fn check(
            &self,
            authority: &AccountId,
            _instruction: &Instruction,
            _wsv: &WorldStateView,
        ) -> ValidatorVerdict {
            if authority.name.as_ref() == "alice" {
                ValidatorVerdict::Deny("Alice account is denied.".to_owned().into())
            } else {
                ValidatorVerdict::Skip
            }
        }
    }

    #[derive(Debug, Clone, Serialize)]
    struct GrantedToken;

    // TODO: ADD some Revoke tests.

    impl HasToken for GrantedToken {
        fn token(
            &self,
            _authority: &AccountId,
            _instruction: &Instruction,
            _wsv: &WorldStateView,
        ) -> std::result::Result<PermissionToken, String> {
            Ok(PermissionToken::new(
                Name::from_str("token").expect("Valid"),
            ))
        }
    }

    fn asset_id(
        asset_name: &str,
        asset_domain: &str,
        account_name: &str,
        account_domain: &str,
    ) -> IdBox {
        IdBox::AssetId(AssetId::new(
            AssetDefinitionId::new(
                asset_name.parse().expect("Valid"),
                asset_domain.parse().expect("Valid"),
            ),
            AccountId::new(
                account_name.parse().expect("Valid"),
                account_domain.parse().expect("Valid"),
            ),
        ))
    }

    #[test]
    pub fn multiple_validators_combined() {
        let permissions_validator = ValidatorBuilder::with_validator(DenyBurn)
            .with_validator(DenyAlice)
            .no_denies()
            .build();
        let instruction_burn: Instruction =
            BurnBox::new(Value::U32(10), asset_id("xor", "test", "alice", "test")).into();
        let instruction_fail = Instruction::Fail(FailBox {
            message: "fail message".to_owned(),
        });
        let account_bob = <Account as Identifiable>::Id::from_str("bob@test").expect("Valid");
        let account_alice = <Account as Identifiable>::Id::from_str("alice@test").expect("Valid");
        let wsv = WorldStateView::new(World::new());
        assert!(permissions_validator
            .judge(&account_bob, &instruction_burn, &wsv)
            .is_err());
        assert!(permissions_validator
            .judge(&account_alice, &instruction_fail, &wsv)
            .is_err());
        assert!(permissions_validator
            .judge(&account_alice, &instruction_burn, &wsv)
            .is_err());
        assert!(permissions_validator
            .judge(&account_bob, &instruction_fail, &wsv)
            .is_ok());
    }

    #[test]
    pub fn recursive_validator() {
        let permissions_validator = ValidatorBuilder::with_recursive_validator(DenyBurn)
            .no_denies()
            .build();
        let instruction_burn: Instruction =
            BurnBox::new(Value::U32(10), asset_id("xor", "test", "alice", "test")).into();
        let instruction_fail = Instruction::Fail(FailBox {
            message: "fail message".to_owned(),
        });
        let nested_instruction_sequence =
            Instruction::If(If::new(true, instruction_burn.clone()).into());
        let account_alice = <Account as Identifiable>::Id::from_str("alice@test").expect("Valid");
        let wsv = WorldStateView::new(World::new());
        assert!(permissions_validator
            .judge(&account_alice, &instruction_fail, &wsv)
            .is_ok());
        assert!(permissions_validator
            .judge(&account_alice, &instruction_burn, &wsv)
            .is_err());
        assert!(permissions_validator
            .judge(&account_alice, &nested_instruction_sequence, &wsv)
            .is_err());
    }

    #[test]
    pub fn granted_permission() -> eyre::Result<()> {
        let alice_id = <Account as Identifiable>::Id::from_str("alice@test")?;
        let bob_id = <Account as Identifiable>::Id::from_str("bob@test")?;
        let alice_xor_id = <Asset as Identifiable>::Id::new(
            AssetDefinitionId::from_str("xor#test").expect("Valid"),
            AccountId::from_str("alice@test").expect("Valid"),
        );
        let instruction_burn: Instruction = BurnBox::new(Value::U32(10), alice_xor_id).into();
        let mut domain = Domain::new(DomainId::from_str("test").expect("Valid")).build();
        let mut bob_account = Account::new(bob_id.clone(), []).build();
        assert!(bob_account.add_permission(PermissionToken::new(
            Name::from_str("token").expect("Valid")
        )));
        assert!(domain.add_account(bob_account).is_none());
        let wsv = WorldStateView::new(World::with([domain], BTreeSet::new()));
        let validator: HasTokenBoxed = Box::new(GrantedToken);
        assert!(validator
            .check(&alice_id, &instruction_burn, &wsv)
            .is_deny());
        assert!(validator.check(&bob_id, &instruction_burn, &wsv).is_allow());
        Ok(())
    }

    #[test]
    pub fn check_query_permissions_nested() {
        let instruction: Instruction = Pair::new(
            TransferBox::new(
                asset_id("btc", "crypto", "seller", "company"),
                Expression::Add(Add::new(
                    Expression::Query(
                        FindAssetQuantityById::new(AssetId::new(
                            AssetDefinitionId::from_str("btc2eth_rate#exchange").expect("Valid"),
                            AccountId::from_str("dex@exchange").expect("Valid"),
                        ))
                        .into(),
                    ),
                    10_u32,
                )),
                asset_id("btc", "crypto", "buyer", "company"),
            ),
            TransferBox::new(
                asset_id("eth", "crypto", "buyer", "company"),
                15_u32,
                asset_id("eth", "crypto", "seller", "company"),
            ),
        )
        .into();
        let wsv = WorldStateView::new(World::new());
        let alice_id = <Account as Identifiable>::Id::from_str("alice@test").expect("Valid");
        let judge = ValidatorBuilder::with_validator(DenyAll::new())
            .no_denies()
            .build();
        assert!(check_query_in_instruction(&alice_id, &instruction, &wsv, &judge).is_err())
    }
}
