//! Legacy account-alias binding wire compatibility.
//!
//! The executor routes this shape through the current alias intent classifier,
//! including exact dataspace and PSP-domain isolation checks.
use super::*;
isi! {
    /// Bind or clear non-primary aliases for an existing account.
    pub struct SetAccountAliasBinding {
        /// Account whose alias binding should be reconciled.
        pub account: AccountId,
        /// Desired on-chain alias, or `None` to clear non-primary aliases.
        #[norito(default)]
        pub alias: Option<crate::account::rekey::AccountAlias>,
        /// Legacy lease-expiry carrier.
        ///
        /// The current engine rejects a value here because lease mutation must
        /// use the guarded lease lifecycle API.
        #[norito(default)]
        pub lease_expiry_ms: Option<u64>,
    }
}
impl SetAccountAliasBinding {
    /// Stable legacy wire identifier for this instruction.
    pub const WIRE_ID: &'static str = "iroha.account.alias.binding.set";
    /// Create a binding instruction.
    #[must_use]
    pub fn bind(
        account: AccountId,
        alias: crate::account::rekey::AccountAlias,
        lease_expiry_ms: Option<u64>,
    ) -> Self {
        Self {
            account,
            alias: Some(alias),
            lease_expiry_ms,
        }
    }
    /// Create an instruction that clears every non-primary alias binding.
    #[must_use]
    pub fn clear(account: AccountId) -> Self {
        Self {
            account,
            alias: None,
            lease_expiry_ms: None,
        }
    }
}
impl crate::seal::Instruction for SetAccountAliasBinding {}
impl<'a> norito::core::DecodeFromSlice<'a> for SetAccountAliasBinding {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = norito::core::effective_decode_flags()
            .unwrap_or_else(norito::core::default_encode_flags);
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }
        let mut offset = 0usize;
        let account = super::decode_aos_canonical_field::<AccountId>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let alias = if offset < bytes.len() {
            super::decode_aos_canonical_field::<Option<crate::account::rekey::AccountAlias>>(
                super::read_aos_field(bytes, &mut offset, flags)?,
                flags,
            )?
        } else {
            None
        };
        let lease_expiry_ms = if offset < bytes.len() {
            super::decode_aos_canonical_field::<Option<u64>>(
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
                account,
                alias,
                lease_expiry_ms,
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
            .expect("derive alias-binding fixture keypair");
        AccountId::new(key_pair.public_key().clone())
    }
    #[test]
    fn legacy_alias_binding_roundtrips() {
        let alias = AccountAlias::new(
            "merchant".parse().expect("alias label"),
            Some(AccountAliasDomain::new(
                "banka".parse().expect("alias domain"),
            )),
            DataSpaceId::UNIVERSAL,
        );
        for value in [
            SetAccountAliasBinding::bind(account(0xA1), alias, None),
            SetAccountAliasBinding::clear(account(0xA2)),
        ] {
            let bytes = value.encode();
            let (decoded, used) =
                SetAccountAliasBinding::decode_from_slice(&bytes).expect("decode legacy binding");
            assert_eq!(used, bytes.len());
            assert_eq!(decoded, value);
        }
    }
}
