//! Consensus-backed Sora Name Service mutation instructions.

use super::*;
use crate::sns::{
    FreezeNameRequestV1, GovernanceHookV1, RegisterNameRequestV1, RenewNameRequestV1, SuffixId,
    TransferNameRequestV1, UpdateControllersRequestV1,
};

fn encode_sns_payload<T: norito::codec::Encode>(payload: T) -> Vec<u8> {
    let encoded = payload.encode();
    drop(payload);
    encoded
}

isi! {
    /// Register a SNS name record through normal transaction consensus.
    pub struct RegisterSnsName {
        /// Norito-encoded [`RegisterNameRequestV1`] to execute in world state.
        pub request: Vec<u8>,
    }
}

impl RegisterSnsName {
    /// Stable wire identifier for this instruction.
    pub const WIRE_ID: &'static str = "iroha.sns.name.register";

    /// Construct a SNS name registration instruction.
    #[must_use]
    pub fn new(request: RegisterNameRequestV1) -> Self {
        Self {
            request: encode_sns_payload(request),
        }
    }
}

impl crate::seal::Instruction for RegisterSnsName {}

isi! {
    /// Renew a SNS name record through normal transaction consensus.
    pub struct RenewSnsName {
        /// Registered suffix identifier.
        pub suffix_id: SuffixId,
        /// Canonical namespace literal.
        pub literal: String,
        /// Norito-encoded [`RenewNameRequestV1`] to execute in world state.
        pub request: Vec<u8>,
    }
}

impl RenewSnsName {
    /// Stable wire identifier for this instruction.
    pub const WIRE_ID: &'static str = "iroha.sns.name.renew";

    /// Construct a SNS name renewal instruction.
    #[must_use]
    pub fn new(
        suffix_id: SuffixId,
        literal: impl Into<String>,
        request: RenewNameRequestV1,
    ) -> Self {
        Self {
            suffix_id,
            literal: literal.into(),
            request: encode_sns_payload(request),
        }
    }
}

impl crate::seal::Instruction for RenewSnsName {}

isi! {
    /// Transfer SNS name ownership through normal transaction consensus.
    pub struct TransferSnsName {
        /// Registered suffix identifier.
        pub suffix_id: SuffixId,
        /// Canonical namespace literal.
        pub literal: String,
        /// Norito-encoded [`TransferNameRequestV1`] to execute in world state.
        pub request: Vec<u8>,
    }
}

impl TransferSnsName {
    /// Stable wire identifier for this instruction.
    pub const WIRE_ID: &'static str = "iroha.sns.name.transfer";

    /// Construct a SNS name transfer instruction.
    #[must_use]
    pub fn new(
        suffix_id: SuffixId,
        literal: impl Into<String>,
        request: TransferNameRequestV1,
    ) -> Self {
        Self {
            suffix_id,
            literal: literal.into(),
            request: encode_sns_payload(request),
        }
    }
}

impl crate::seal::Instruction for TransferSnsName {}

isi! {
    /// Replace SNS name controllers through normal transaction consensus.
    pub struct UpdateSnsNameControllers {
        /// Registered suffix identifier.
        pub suffix_id: SuffixId,
        /// Canonical namespace literal.
        pub literal: String,
        /// Norito-encoded [`UpdateControllersRequestV1`] to execute in world state.
        pub request: Vec<u8>,
    }
}

impl UpdateSnsNameControllers {
    /// Stable wire identifier for this instruction.
    pub const WIRE_ID: &'static str = "iroha.sns.name.controllers.update";

    /// Construct a SNS controller update instruction.
    #[must_use]
    pub fn new(
        suffix_id: SuffixId,
        literal: impl Into<String>,
        request: UpdateControllersRequestV1,
    ) -> Self {
        Self {
            suffix_id,
            literal: literal.into(),
            request: encode_sns_payload(request),
        }
    }
}

impl crate::seal::Instruction for UpdateSnsNameControllers {}

isi! {
    /// Freeze a SNS name through normal transaction consensus.
    pub struct FreezeSnsName {
        /// Registered suffix identifier.
        pub suffix_id: SuffixId,
        /// Canonical namespace literal.
        pub literal: String,
        /// Norito-encoded [`FreezeNameRequestV1`] to execute in world state.
        pub request: Vec<u8>,
    }
}

impl FreezeSnsName {
    /// Stable wire identifier for this instruction.
    pub const WIRE_ID: &'static str = "iroha.sns.name.freeze";

    /// Construct a SNS name freeze instruction.
    #[must_use]
    pub fn new(
        suffix_id: SuffixId,
        literal: impl Into<String>,
        request: FreezeNameRequestV1,
    ) -> Self {
        Self {
            suffix_id,
            literal: literal.into(),
            request: encode_sns_payload(request),
        }
    }
}

impl crate::seal::Instruction for FreezeSnsName {}

isi! {
    /// Unfreeze a SNS name through normal transaction consensus.
    pub struct UnfreezeSnsName {
        /// Registered suffix identifier.
        pub suffix_id: SuffixId,
        /// Canonical namespace literal.
        pub literal: String,
        /// Norito-encoded [`GovernanceHookV1`] authorizing the unfreeze.
        pub governance: Vec<u8>,
    }
}

impl UnfreezeSnsName {
    /// Stable wire identifier for this instruction.
    pub const WIRE_ID: &'static str = "iroha.sns.name.unfreeze";

    /// Construct a SNS name unfreeze instruction.
    #[must_use]
    pub fn new(
        suffix_id: SuffixId,
        literal: impl Into<String>,
        governance: GovernanceHookV1,
    ) -> Self {
        Self {
            suffix_id,
            literal: literal.into(),
            governance: encode_sns_payload(governance),
        }
    }
}

impl crate::seal::Instruction for UnfreezeSnsName {}
