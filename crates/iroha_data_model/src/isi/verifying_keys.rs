use super::*;
use crate::proof::{VerifyingKeyId, VerifyingKeyRecord};

isi! {
    /// Register a new verifying key record into the WSV.
    pub struct RegisterVerifyingKey {
        /// Identifier of the verifying key (backend + name).
        pub id: VerifyingKeyId,
        /// Verifying key record with version and commitment (and optional inline key).
        pub record: VerifyingKeyRecord,
    }
}

isi! {
    /// Update an existing verifying key record (monotonic version).
    pub struct UpdateVerifyingKey {
        /// Identifier of the verifying key to update.
        pub id: VerifyingKeyId,
        /// New record with strictly greater version.
        pub record: VerifyingKeyRecord,
    }
}

isi! {
    /// Deprecate an existing verifying key (disallow future updates).
    pub struct DeprecateVerifyingKey {
        /// Identifier of the verifying key to deprecate.
        pub id: VerifyingKeyId,
    }
}

// Seal implementations so these types participate as instructions
impl crate::seal::Instruction for RegisterVerifyingKey {}
impl crate::seal::Instruction for UpdateVerifyingKey {}
impl crate::seal::Instruction for DeprecateVerifyingKey {}
