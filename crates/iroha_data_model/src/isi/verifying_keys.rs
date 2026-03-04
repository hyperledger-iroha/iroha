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

// Seal implementations so these types participate as instructions
impl crate::seal::Instruction for RegisterVerifyingKey {}
impl crate::seal::Instruction for UpdateVerifyingKey {}
