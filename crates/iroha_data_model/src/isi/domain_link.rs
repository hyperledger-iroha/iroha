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
    /// Remove an account subject link from a domain without deleting the account identity.
    pub struct UnlinkAccountDomain {
        /// Scoped account identifier whose subject should be unlinked.
        pub account: AccountId,
        /// Domain to unlink the account subject from.
        pub domain: DomainId,
    }
}

impl crate::seal::Instruction for UnlinkAccountDomain {}
