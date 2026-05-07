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

impl<'a> norito::core::DecodeFromSlice<'a> for AcquireAccountAliasLease {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = norito::core::effective_decode_flags()
            .unwrap_or_else(norito::core::default_encode_flags);
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let mut offset = 0usize;
        let alias = super::decode_aos_canonical_field::<crate::account::rekey::AccountAlias>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let owner = super::decode_aos_canonical_field::<AccountId>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let payer = super::decode_aos_canonical_field::<AccountId>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let term_years = super::decode_aos_canonical_field::<u8>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let pricing_class_hint = if offset < bytes.len() {
            super::decode_aos_canonical_field::<Option<u8>>(
                super::read_aos_field(bytes, &mut offset, flags)?,
                flags,
            )?
        } else {
            None
        };
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((
            Self {
                alias,
                owner,
                payer,
                term_years,
                pricing_class_hint,
            },
            offset,
        ))
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for RenewAccountAliasLease {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = norito::core::effective_decode_flags()
            .unwrap_or_else(norito::core::default_encode_flags);
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let mut offset = 0usize;
        let alias = super::decode_aos_canonical_field::<crate::account::rekey::AccountAlias>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let payer = super::decode_aos_canonical_field::<AccountId>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let term_years = super::decode_aos_canonical_field::<u8>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((
            Self {
                alias,
                payer,
                term_years,
            },
            offset,
        ))
    }
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, KeyPair};
    use norito::core::DecodeFromSlice;

    use super::*;
    use crate::{
        account::rekey::{AccountAlias, AccountAliasDomain},
        nexus::DataSpaceId,
    };

    fn account(seed: u8) -> AccountId {
        let key_pair = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
        AccountId::new(key_pair.public_key().clone())
    }

    fn alias() -> AccountAlias {
        AccountAlias::new(
            "merchant".parse().expect("alias label"),
            Some(AccountAliasDomain::new(
                "banka".parse().expect("alias domain"),
            )),
            DataSpaceId::UNIVERSAL,
        )
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

    #[test]
    fn account_alias_lease_decode_from_slice_roundtrips() {
        let owner = account(0xB1);
        let payer = account(0xB2);
        assert_slice_roundtrip(AcquireAccountAliasLease::new(
            alias(),
            owner,
            payer.clone(),
            3,
            Some(2),
        ));
        assert_slice_roundtrip(RenewAccountAliasLease::new(alias(), payer, 1));
    }

    #[test]
    fn account_alias_lease_registry_decodes_type_names() {
        let registry = crate::isi::InstructionRegistry::new()
            .register_slice::<AcquireAccountAliasLease>()
            .register_slice::<RenewAccountAliasLease>();
        let owner = account(0xB3);
        let payer = account(0xB4);

        let acquire = AcquireAccountAliasLease::new(alias(), owner, payer.clone(), 2, None);
        let (payload, flags) = norito::codec::encode_with_header_flags(&acquire);
        let framed =
            norito::core::frame_bare_with_header_flags::<AcquireAccountAliasLease>(&payload, flags)
                .expect("frame acquire");
        let decoded = crate::isi::InstructionRegistry::decode(
            &registry,
            std::any::type_name::<AcquireAccountAliasLease>(),
            &framed,
        )
        .expect("registered acquire")
        .expect("decode acquire");
        assert_eq!(crate::isi::Instruction::dyn_encode(&*decoded), payload);

        let renew = RenewAccountAliasLease::new(alias(), payer, 1);
        let (payload, flags) = norito::codec::encode_with_header_flags(&renew);
        let framed =
            norito::core::frame_bare_with_header_flags::<RenewAccountAliasLease>(&payload, flags)
                .expect("frame renew");
        let decoded = crate::isi::InstructionRegistry::decode(
            &registry,
            std::any::type_name::<RenewAccountAliasLease>(),
            &framed,
        )
        .expect("registered renew")
        .expect("decode renew");
        assert_eq!(crate::isi::Instruction::dyn_encode(&*decoded), payload);
    }
}
