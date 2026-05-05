//! Hidden-program RAM-LFE program-policy instructions.

use super::*;
use crate::ram_lfe::{RamLfeProgramId, RamLfeProgramPolicy};

isi! {
    /// Register a new generic RAM-LFE program policy.
    pub struct RegisterRamLfeProgramPolicy {
        /// Program policy record to register.
        pub policy: RamLfeProgramPolicy,
    }
}

impl crate::seal::Instruction for RegisterRamLfeProgramPolicy {}

isi! {
    /// Activate an existing RAM-LFE program policy.
    pub struct ActivateRamLfeProgramPolicy {
        /// Program policy identifier to activate.
        pub program_id: RamLfeProgramId,
    }
}

impl crate::seal::Instruction for ActivateRamLfeProgramPolicy {}

isi! {
    /// Deactivate an existing RAM-LFE program policy.
    pub struct DeactivateRamLfeProgramPolicy {
        /// Program policy identifier to deactivate.
        pub program_id: RamLfeProgramId,
    }
}

impl crate::seal::Instruction for DeactivateRamLfeProgramPolicy {}
