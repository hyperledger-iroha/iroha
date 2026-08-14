//! Legacy account-alias lease acquisition wire compatibility.
//!
//! New callers should use [`super::alias_setup::EnsureAlias`]. This instruction
//! remains registered so BOI participants built against the pre-declarative
//! alias API can submit the same lease request into the current SNS engine.
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
    /// Stable legacy wire identifier for this instruction.
    pub const WIRE_ID: &'static str = "iroha.account.alias.lease.acquire";
    /// Construct a paid alias lease-acquisition instruction.
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
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        account::rekey::{AccountAlias, AccountAliasDomain},
        nexus::DataSpaceId,
    };
    use iroha_crypto::{Algorithm, KeyPair};
    use norito::core::DecodeFromSlice;
    fn account(seed: u8) -> AccountId {
        let key_pair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("derive account-alias fixture keypair");
        AccountId::new(key_pair.public_key().clone())
    }
    #[test]
    fn legacy_acquire_alias_roundtrips() {
        let alias = AccountAlias::new(
            "merchant".parse().expect("alias label"),
            Some(AccountAliasDomain::new(
                "banka".parse().expect("alias domain"),
            )),
            DataSpaceId::UNIVERSAL,
        );
        let value = AcquireAccountAliasLease::new(alias, account(0xB1), account(0xB2), 3, Some(2));
        let bytes = value.encode();
        let (decoded, used) =
            AcquireAccountAliasLease::decode_from_slice(&bytes).expect("decode legacy acquisition");
        assert_eq!(used, bytes.len());
        assert_eq!(decoded, value);
    }
}
