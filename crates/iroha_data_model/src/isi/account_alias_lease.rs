//! Canonical paid account-alias lease instructions.

use super::*;

isi! {
    /// Acquire a finite SNS lease for an account alias.
    pub struct AcquireAccountAliasLease {
        /// Alias literal to lease.
        pub alias: crate::account::rekey::AccountAlias,
        /// Account that will own the leased alias.
        pub owner: AccountId,
        /// Account that pays the alias lease cost.
        pub payer: AccountId,
        /// Lease term in years.
        pub term_years: u8,
        /// Optional pricing tier hint.
        #[norito(default)]
        pub pricing_class_hint: Option<u8>,
    }
}

impl AcquireAccountAliasLease {
    /// Stable wire identifier for this instruction.
    pub const WIRE_ID: &'static str = "iroha.account.alias.lease.acquire";

    /// Construct a new paid alias lease-acquisition instruction.
    #[must_use]
    pub fn new(
        alias: crate::account::rekey::AccountAlias,
        owner: AccountId,
        payer: AccountId,
        term_years: u8,
        pricing_class_hint: Option<u8>,
    ) -> Self {
        Self {
            alias,
            owner,
            payer,
            term_years,
            pricing_class_hint,
        }
    }
}

impl crate::seal::Instruction for AcquireAccountAliasLease {}

isi! {
    /// Renew an existing finite SNS lease for an account alias.
    pub struct RenewAccountAliasLease {
        /// Alias literal to renew.
        pub alias: crate::account::rekey::AccountAlias,
        /// Account that pays the renewal cost.
        pub payer: AccountId,
        /// Renewal term in years.
        pub term_years: u8,
    }
}

impl RenewAccountAliasLease {
    /// Stable wire identifier for this instruction.
    pub const WIRE_ID: &'static str = "iroha.account.alias.lease.renew";

    /// Construct a new paid alias lease-renewal instruction.
    #[must_use]
    pub fn new(
        alias: crate::account::rekey::AccountAlias,
        payer: AccountId,
        term_years: u8,
    ) -> Self {
        Self {
            alias,
            payer,
            term_years,
        }
    }
}

impl crate::seal::Instruction for RenewAccountAliasLease {}
