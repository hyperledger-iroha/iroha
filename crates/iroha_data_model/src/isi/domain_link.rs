//! Account subject and domain link management instructions.

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
    /// Bind an additional stable on-chain alias to an existing account.
    pub struct BindAccountAlias {
        /// Account whose alias binding should be reconciled.
        pub account: AccountId,
        /// Desired on-chain alias for the account.
        pub label: crate::account::rekey::AccountLabel,
    }
}

impl crate::seal::Instruction for BindAccountAlias {}

isi! {
    /// Set or update the stable on-chain label under which an account is addressed.
    pub struct SetAccountLabel {
        /// Account whose label should be reconciled.
        pub account: AccountId,
        /// Desired on-chain label for the account.
        pub label: crate::account::rekey::AccountLabel,
    }
}

impl crate::seal::Instruction for SetAccountLabel {}

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
