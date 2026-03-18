//! Hidden-function-backed identifier policy instructions.

use super::*;
use crate::{
    account::AccountId,
    identifier::{IdentifierPolicy, IdentifierPolicyId, IdentifierResolutionReceipt},
};

isi! {
    /// Register a new identifier policy namespace in the world state.
    pub struct RegisterIdentifierPolicy {
        /// Identifier policy record to register.
        pub policy: IdentifierPolicy,
    }
}

impl crate::seal::Instruction for RegisterIdentifierPolicy {}

isi! {
    /// Activate an existing identifier policy namespace.
    pub struct ActivateIdentifierPolicy {
        /// Policy namespace to activate.
        pub policy_id: IdentifierPolicyId,
    }
}

impl crate::seal::Instruction for ActivateIdentifierPolicy {}

isi! {
    /// Bind a resolver-signed opaque identifier receipt to the UAID attached to an account.
    pub struct ClaimIdentifier {
        /// Account whose UAID should own the receipt-bound opaque identifier.
        pub account: AccountId,
        /// Signed receipt emitted by the configured identifier resolver.
        pub receipt: IdentifierResolutionReceipt,
    }
}

impl crate::seal::Instruction for ClaimIdentifier {}

isi! {
    /// Revoke a previously claimed opaque identifier.
    pub struct RevokeIdentifier {
        /// Policy namespace under which the opaque identifier was claimed.
        pub policy_id: IdentifierPolicyId,
        /// Opaque identifier to revoke.
        pub opaque_id: OpaqueAccountId,
    }
}

impl crate::seal::Instruction for RevokeIdentifier {}
