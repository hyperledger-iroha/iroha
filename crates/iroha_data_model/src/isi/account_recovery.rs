//! Native account controller replacement and social recovery instructions.

use super::*;

isi! {
    /// Replace the controller governing an existing account while preserving linked state.
    pub struct ReplaceAccountController {
        /// Canonical account identifier to replace.
        pub account: AccountId,
        /// New controller that should govern the account after replacement.
        pub new_controller: crate::account::AccountController,
    }
}

impl ReplaceAccountController {
    /// Stable wire identifier for this instruction.
    pub const WIRE_ID: &'static str = "iroha.account.controller.replace";
}

impl crate::seal::Instruction for ReplaceAccountController {}

isi! {
    /// Set or replace the alias-keyed recovery policy for an account.
    pub struct SetAccountRecoveryPolicy {
        /// Canonical account identifier whose stable alias policy should be updated.
        pub account: AccountId,
        /// Recovery policy keyed by the account's stable alias.
        pub policy: crate::account::AccountRecoveryPolicy,
    }
}

impl SetAccountRecoveryPolicy {
    /// Stable wire identifier for this instruction.
    pub const WIRE_ID: &'static str = "iroha.account.recovery.policy.set";
}

impl crate::seal::Instruction for SetAccountRecoveryPolicy {}

isi! {
    /// Clear the alias-keyed recovery policy for an account.
    pub struct ClearAccountRecoveryPolicy {
        /// Canonical account identifier whose recovery policy should be cleared.
        pub account: AccountId,
    }
}

impl ClearAccountRecoveryPolicy {
    /// Stable wire identifier for this instruction.
    pub const WIRE_ID: &'static str = "iroha.account.recovery.policy.clear";
}

impl crate::seal::Instruction for ClearAccountRecoveryPolicy {}

isi! {
    /// Propose a controller replacement through the social-recovery workflow.
    pub struct ProposeAccountRecovery {
        /// Stable account alias whose active account should be recovered.
        pub alias: crate::account::AccountAlias,
        /// New controller requested for the alias.
        pub new_controller: crate::account::AccountController,
    }
}

impl ProposeAccountRecovery {
    /// Stable wire identifier for this instruction.
    pub const WIRE_ID: &'static str = "iroha.account.recovery.propose";
}

impl crate::seal::Instruction for ProposeAccountRecovery {}

isi! {
    /// Record a guardian approval for the active recovery request of an alias.
    pub struct ApproveAccountRecovery {
        /// Stable account alias whose pending recovery should receive an approval.
        pub alias: crate::account::AccountAlias,
    }
}

impl ApproveAccountRecovery {
    /// Stable wire identifier for this instruction.
    pub const WIRE_ID: &'static str = "iroha.account.recovery.approve";
}

impl crate::seal::Instruction for ApproveAccountRecovery {}

isi! {
    /// Cancel a pending social-recovery request for an alias.
    pub struct CancelAccountRecovery {
        /// Stable account alias whose pending recovery should be cancelled.
        pub alias: crate::account::AccountAlias,
    }
}

impl CancelAccountRecovery {
    /// Stable wire identifier for this instruction.
    pub const WIRE_ID: &'static str = "iroha.account.recovery.cancel";
}

impl crate::seal::Instruction for CancelAccountRecovery {}

isi! {
    /// Finalize a pending social-recovery request once quorum and timelock are satisfied.
    pub struct FinalizeAccountRecovery {
        /// Stable account alias whose pending recovery should be finalized.
        pub alias: crate::account::AccountAlias,
    }
}

impl FinalizeAccountRecovery {
    /// Stable wire identifier for this instruction.
    pub const WIRE_ID: &'static str = "iroha.account.recovery.finalize";
}

impl crate::seal::Instruction for FinalizeAccountRecovery {}

fn account_recovery_decode_flags() -> u8 {
    norito::core::effective_decode_flags().unwrap_or_else(norito::core::default_encode_flags)
}

macro_rules! impl_decode_one_field {
    ($ty:ident { $field:ident: $field_ty:ty }) => {
        impl<'a> norito::core::DecodeFromSlice<'a> for $ty {
            fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
                let flags = account_recovery_decode_flags();
                if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
                    return super::decode_packed_instruction_payload::<Self>(bytes);
                }

                let mut offset = 0usize;
                let $field = super::decode_aos_canonical_field::<$field_ty>(
                    super::read_aos_field(bytes, &mut offset, flags)?,
                    flags,
                )?;
                if offset != bytes.len() {
                    return Err(norito::core::Error::LengthMismatch);
                }
                norito::core::note_payload_access(bytes, offset);
                Ok((Self { $field }, offset))
            }
        }
    };
}

macro_rules! impl_decode_two_fields {
    ($ty:ident { $first:ident: $first_ty:ty, $second:ident: $second_ty:ty }) => {
        impl<'a> norito::core::DecodeFromSlice<'a> for $ty {
            fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
                let flags = account_recovery_decode_flags();
                if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
                    return super::decode_packed_instruction_payload::<Self>(bytes);
                }

                let mut offset = 0usize;
                let $first = super::decode_aos_canonical_field::<$first_ty>(
                    super::read_aos_field(bytes, &mut offset, flags)?,
                    flags,
                )?;
                let $second = super::decode_aos_canonical_field::<$second_ty>(
                    super::read_aos_field(bytes, &mut offset, flags)?,
                    flags,
                )?;
                if offset != bytes.len() {
                    return Err(norito::core::Error::LengthMismatch);
                }
                norito::core::note_payload_access(bytes, offset);
                Ok((Self { $first, $second }, offset))
            }
        }
    };
}

impl_decode_two_fields!(ReplaceAccountController {
    account: AccountId,
    new_controller: crate::account::AccountController
});
impl_decode_two_fields!(SetAccountRecoveryPolicy {
    account: AccountId,
    policy: crate::account::AccountRecoveryPolicy
});
impl_decode_one_field!(ClearAccountRecoveryPolicy { account: AccountId });
impl_decode_two_fields!(ProposeAccountRecovery {
    alias: crate::account::AccountAlias,
    new_controller: crate::account::AccountController
});
impl_decode_one_field!(ApproveAccountRecovery {
    alias: crate::account::AccountAlias
});
impl_decode_one_field!(CancelAccountRecovery {
    alias: crate::account::AccountAlias
});
impl_decode_one_field!(FinalizeAccountRecovery {
    alias: crate::account::AccountAlias
});

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;

    use iroha_crypto::{Algorithm, KeyPair, PublicKey};
    use norito::core::DecodeFromSlice;

    use super::*;
    use crate::{
        account::{
            AccountAlias, AccountAliasDomain, AccountController, AccountRecoveryPolicy,
            RecoveryGuardian,
        },
        nexus::DataSpaceId,
    };

    fn public_key(seed: u8) -> PublicKey {
        let key_pair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("derive checked account-recovery ISI fixture keypair");
        key_pair.public_key().clone()
    }

    fn account(seed: u8) -> AccountId {
        AccountId::new(public_key(seed))
    }

    fn alias() -> AccountAlias {
        AccountAlias::new(
            "recoverable".parse().expect("alias label"),
            Some(AccountAliasDomain::new(
                "banka".parse().expect("alias domain"),
            )),
            DataSpaceId::UNIVERSAL,
        )
    }

    fn controller(seed: u8) -> AccountController {
        AccountController::single(public_key(seed))
    }

    fn policy() -> AccountRecoveryPolicy {
        AccountRecoveryPolicy::new(
            vec![RecoveryGuardian::new(account(0xD1), 1)],
            1,
            NonZeroU64::new(100).expect("nonzero timelock"),
        )
        .expect("recovery policy")
    }

    fn assert_slice_roundtrip<T>(value: T)
    where
        T: Clone + PartialEq + core::fmt::Debug + norito::codec::Encode,
        for<'a> T: DecodeFromSlice<'a>,
    {
        let bytes = value.encode();
        let (decoded, used) = T::decode_from_slice(&bytes).expect("decode from slice");
        assert_eq!(used, bytes.len());
        assert_eq!(decoded, value);
    }

    fn assert_registry_decodes<T>(
        registry: &crate::isi::InstructionRegistry,
        wire_id: &'static str,
        value: T,
    ) where
        T: crate::isi::Instruction
            + norito::codec::Encode
            + 'static
            + norito::core::NoritoSerialize,
        for<'de> T: norito::core::NoritoDeserialize<'de>,
    {
        let (payload, flags) = norito::codec::encode_with_header_flags(&value);
        let framed =
            norito::core::frame_bare_with_header_flags::<T>(&payload, flags).expect("frame");
        let decoded = crate::isi::InstructionRegistry::decode(registry, wire_id, &framed)
            .expect("registered")
            .expect("decode");
        assert_eq!(crate::isi::Instruction::dyn_encode(&*decoded), payload);
    }

    #[test]
    fn account_recovery_decode_from_slice_roundtrips() {
        assert_slice_roundtrip(ReplaceAccountController {
            account: account(0xD2),
            new_controller: controller(0xD3),
        });
        assert_slice_roundtrip(SetAccountRecoveryPolicy {
            account: account(0xD4),
            policy: policy(),
        });
        assert_slice_roundtrip(ClearAccountRecoveryPolicy {
            account: account(0xD5),
        });
        assert_slice_roundtrip(ProposeAccountRecovery {
            alias: alias(),
            new_controller: controller(0xD6),
        });
        assert_slice_roundtrip(ApproveAccountRecovery { alias: alias() });
        assert_slice_roundtrip(CancelAccountRecovery { alias: alias() });
        assert_slice_roundtrip(FinalizeAccountRecovery { alias: alias() });
    }

    #[test]
    fn account_recovery_registry_decodes_stable_ids() {
        let registry = crate::isi::InstructionRegistry::new()
            .register_with_id_slice::<ReplaceAccountController>(ReplaceAccountController::WIRE_ID)
            .register_with_id_slice::<SetAccountRecoveryPolicy>(SetAccountRecoveryPolicy::WIRE_ID)
            .register_with_id_slice::<ClearAccountRecoveryPolicy>(
                ClearAccountRecoveryPolicy::WIRE_ID,
            )
            .register_with_id_slice::<ProposeAccountRecovery>(ProposeAccountRecovery::WIRE_ID)
            .register_with_id_slice::<ApproveAccountRecovery>(ApproveAccountRecovery::WIRE_ID)
            .register_with_id_slice::<CancelAccountRecovery>(CancelAccountRecovery::WIRE_ID)
            .register_with_id_slice::<FinalizeAccountRecovery>(FinalizeAccountRecovery::WIRE_ID);

        assert_registry_decodes(
            &registry,
            ReplaceAccountController::WIRE_ID,
            ReplaceAccountController {
                account: account(0xD7),
                new_controller: controller(0xD8),
            },
        );
        assert_registry_decodes(
            &registry,
            SetAccountRecoveryPolicy::WIRE_ID,
            SetAccountRecoveryPolicy {
                account: account(0xD9),
                policy: policy(),
            },
        );
        assert_registry_decodes(
            &registry,
            ClearAccountRecoveryPolicy::WIRE_ID,
            ClearAccountRecoveryPolicy {
                account: account(0xDA),
            },
        );
        assert_registry_decodes(
            &registry,
            ProposeAccountRecovery::WIRE_ID,
            ProposeAccountRecovery {
                alias: alias(),
                new_controller: controller(0xDB),
            },
        );
        assert_registry_decodes(
            &registry,
            ApproveAccountRecovery::WIRE_ID,
            ApproveAccountRecovery { alias: alias() },
        );
        assert_registry_decodes(
            &registry,
            CancelAccountRecovery::WIRE_ID,
            CancelAccountRecovery { alias: alias() },
        );
        assert_registry_decodes(
            &registry,
            FinalizeAccountRecovery::WIRE_ID,
            FinalizeAccountRecovery { alias: alias() },
        );
    }
}
