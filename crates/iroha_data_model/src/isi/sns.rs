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

fn sns_decode_flags() -> u8 {
    norito::core::effective_decode_flags().unwrap_or_else(norito::core::default_encode_flags)
}

fn decode_sns_request_payload(bytes: &[u8]) -> Result<(Vec<u8>, usize), norito::core::Error> {
    let flags = sns_decode_flags();
    let mut offset = 0usize;
    let request = super::decode_aos_slice_field::<Vec<u8>>(
        super::read_aos_field(bytes, &mut offset, flags)?,
        flags,
    )?;
    if offset != bytes.len() {
        return Err(norito::core::Error::LengthMismatch);
    }
    norito::core::note_payload_access(bytes, offset);
    Ok((request, offset))
}

fn decode_sns_named_payload(
    bytes: &[u8],
) -> Result<(SuffixId, String, Vec<u8>, usize), norito::core::Error> {
    let flags = sns_decode_flags();
    let mut offset = 0usize;
    let suffix_id = super::decode_aos_slice_field::<SuffixId>(
        super::read_aos_field(bytes, &mut offset, flags)?,
        flags,
    )?;
    let literal = super::decode_aos_slice_field::<String>(
        super::read_aos_field(bytes, &mut offset, flags)?,
        flags,
    )?;
    let request = super::decode_aos_slice_field::<Vec<u8>>(
        super::read_aos_field(bytes, &mut offset, flags)?,
        flags,
    )?;
    if offset != bytes.len() {
        return Err(norito::core::Error::LengthMismatch);
    }
    norito::core::note_payload_access(bytes, offset);
    Ok((suffix_id, literal, request, offset))
}

impl<'a> norito::core::DecodeFromSlice<'a> for RegisterSnsName {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = sns_decode_flags();
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let (request, used) = decode_sns_request_payload(bytes)?;
        Ok((Self { request }, used))
    }
}

macro_rules! impl_named_sns_decode {
    ($ty:ident { $bytes_field:ident }) => {
        impl<'a> norito::core::DecodeFromSlice<'a> for $ty {
            fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
                let flags = sns_decode_flags();
                if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
                    return super::decode_packed_instruction_payload::<Self>(bytes);
                }

                let (suffix_id, literal, $bytes_field, used) = decode_sns_named_payload(bytes)?;
                Ok((
                    Self {
                        suffix_id,
                        literal,
                        $bytes_field,
                    },
                    used,
                ))
            }
        }
    };
}

impl_named_sns_decode!(RenewSnsName { request });
impl_named_sns_decode!(TransferSnsName { request });
impl_named_sns_decode!(UpdateSnsNameControllers { request });
impl_named_sns_decode!(FreezeSnsName { request });
impl_named_sns_decode!(UnfreezeSnsName { governance });

#[cfg(test)]
mod tests {
    use norito::core::DecodeFromSlice;

    use super::*;

    #[test]
    fn sns_isi_decode_from_slice_roundtrips() {
        let register = RegisterSnsName {
            request: vec![0x01, 0x02],
        };
        let renew = RenewSnsName {
            suffix_id: 0x1001,
            literal: "alice".to_owned(),
            request: vec![0x03],
        };
        let transfer = TransferSnsName {
            suffix_id: 0x1001,
            literal: "alice".to_owned(),
            request: vec![0x04],
        };
        let update = UpdateSnsNameControllers {
            suffix_id: 0x1001,
            literal: "alice".to_owned(),
            request: vec![0x05],
        };
        let freeze = FreezeSnsName {
            suffix_id: 0x1001,
            literal: "alice".to_owned(),
            request: vec![0x06],
        };
        let unfreeze = UnfreezeSnsName {
            suffix_id: 0x1001,
            literal: "alice".to_owned(),
            governance: vec![0x07],
        };

        macro_rules! assert_decode {
            ($value:expr, $ty:ty) => {{
                let value: $ty = $value;
                let bytes = value.encode();
                let (decoded, used) =
                    <$ty as DecodeFromSlice>::decode_from_slice(&bytes).expect("decode SNS ISI");
                assert_eq!(used, bytes.len());
                assert_eq!(decoded, value);
            }};
        }

        assert_decode!(register, RegisterSnsName);
        assert_decode!(renew, RenewSnsName);
        assert_decode!(transfer, TransferSnsName);
        assert_decode!(update, UpdateSnsNameControllers);
        assert_decode!(freeze, FreezeSnsName);
        assert_decode!(unfreeze, UnfreezeSnsName);
    }

    #[test]
    fn sns_registry_slice_decodes_type_names() {
        let registry = crate::isi::InstructionRegistry::new()
            .register_slice::<RegisterSnsName>()
            .register_slice::<RenewSnsName>()
            .register_slice::<TransferSnsName>()
            .register_slice::<UpdateSnsNameControllers>()
            .register_slice::<FreezeSnsName>()
            .register_slice::<UnfreezeSnsName>();

        macro_rules! assert_registry_decode {
            ($value:expr, $ty:ty) => {{
                let value: $ty = $value;
                let (payload, flags) = norito::codec::encode_with_header_flags(&value);
                let framed = norito::core::frame_bare_with_header_flags::<$ty>(&payload, flags)
                    .expect("frame SNS ISI");
                let decoded = crate::isi::InstructionRegistry::decode(
                    &registry,
                    std::any::type_name::<$ty>(),
                    &framed,
                )
                .expect("registered SNS ISI")
                .expect("decode SNS ISI");
                assert_eq!(crate::isi::Instruction::dyn_encode(&*decoded), payload);
            }};
        }

        assert_registry_decode!(
            RegisterSnsName {
                request: vec![0x01],
            },
            RegisterSnsName
        );
        assert_registry_decode!(
            RenewSnsName {
                suffix_id: 0x1001,
                literal: "alice".to_owned(),
                request: vec![0x02],
            },
            RenewSnsName
        );
        assert_registry_decode!(
            TransferSnsName {
                suffix_id: 0x1001,
                literal: "alice".to_owned(),
                request: vec![0x03],
            },
            TransferSnsName
        );
        assert_registry_decode!(
            UpdateSnsNameControllers {
                suffix_id: 0x1001,
                literal: "alice".to_owned(),
                request: vec![0x04],
            },
            UpdateSnsNameControllers
        );
        assert_registry_decode!(
            FreezeSnsName {
                suffix_id: 0x1001,
                literal: "alice".to_owned(),
                request: vec![0x05],
            },
            FreezeSnsName
        );
        assert_registry_decode!(
            UnfreezeSnsName {
                suffix_id: 0x1001,
                literal: "alice".to_owned(),
                governance: vec![0x06],
            },
            UnfreezeSnsName
        );
    }
}
