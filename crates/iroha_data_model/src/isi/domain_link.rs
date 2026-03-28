//! Account subject, alias binding, and domain link management instructions.

use super::*;

isi! {
    /// Link an account subject to a domain without requiring account registration in that domain.
    pub struct LinkAccountDomain {
        /// Scoped account identifier whose subject should be linked.
        pub account: AccountId,
        /// Domain to link the account subject to.
        pub domain: DomainId,
    }
}

impl crate::seal::Instruction for LinkAccountDomain {}

isi! {
    /// Bind, renew, or clear non-primary aliases for an existing account.
    pub struct SetAccountAliasBinding {
        /// Account whose alias binding should be reconciled.
        pub account: AccountId,
        /// Desired on-chain alias for the account.
        ///
        /// `None` clears every non-primary alias currently bound to the account.
        #[norito(default)]
        pub alias: Option<crate::account::rekey::AccountAlias>,
        /// Optional lease expiry timestamp (unix ms). When provided, the authoritative SNS lease
        /// for the alias is updated before the binding is reconciled.
        #[norito(default)]
        pub lease_expiry_ms: Option<u64>,
    }
}

impl SetAccountAliasBinding {
    /// Stable wire identifier for this instruction.
    pub const WIRE_ID: &'static str = "iroha.account.alias.binding.set";

    /// Create a binding or renewal instruction.
    #[must_use]
    pub fn bind(
        account: AccountId,
        alias: crate::account::rekey::AccountAlias,
        lease_expiry_ms: Option<u64>,
    ) -> Self {
        Self {
            account,
            alias: Some(alias),
            lease_expiry_ms,
        }
    }

    /// Create an instruction that clears all non-primary alias bindings for the account.
    #[must_use]
    pub fn clear(account: AccountId) -> Self {
        Self {
            account,
            alias: None,
            lease_expiry_ms: None,
        }
    }
}

impl crate::seal::Instruction for SetAccountAliasBinding {}

isi! {
    /// Set, update, renew, or clear the primary alias under which an account is addressed.
    ///
    /// `alias = None` clears the current primary alias.
    pub struct SetPrimaryAccountAlias {
        /// Account whose label should be reconciled.
        pub account: AccountId,
        /// Desired on-chain alias for the account. `None` clears the current primary alias.
        #[norito(default)]
        pub alias: Option<crate::account::rekey::AccountAlias>,
        /// Optional lease expiry timestamp (unix ms). When provided, the authoritative SNS lease
        /// for the alias is updated before the primary alias is reconciled.
        #[norito(default)]
        pub lease_expiry_ms: Option<u64>,
    }
}

impl SetPrimaryAccountAlias {
    /// Stable wire identifier for this instruction.
    pub const WIRE_ID: &'static str = "iroha.account.alias.primary.set";

    /// Create a primary-alias assignment or renewal instruction.
    #[must_use]
    pub fn bind(
        account: AccountId,
        alias: crate::account::rekey::AccountAlias,
        lease_expiry_ms: Option<u64>,
    ) -> Self {
        Self {
            account,
            alias: Some(alias),
            lease_expiry_ms,
        }
    }

    /// Create an instruction that clears the current primary alias.
    #[must_use]
    pub fn clear(account: AccountId) -> Self {
        Self {
            account,
            alias: None,
            lease_expiry_ms: None,
        }
    }
}

impl crate::seal::Instruction for SetPrimaryAccountAlias {}

isi! {
    /// Remove an account subject link from a domain without deleting the account identity.
    pub struct UnlinkAccountDomain {
        /// Scoped account identifier whose subject should be unlinked.
        pub account: AccountId,
        /// Domain to unlink the account subject from.
        pub domain: DomainId,
    }
}

impl crate::seal::Instruction for UnlinkAccountDomain {}
