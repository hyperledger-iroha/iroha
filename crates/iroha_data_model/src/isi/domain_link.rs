//! Account alias binding management instructions.

use super::*;

isi! {
    /// Bind, renew, or clear non-primary aliases for an existing account.
    pub struct SetAccountAliasBinding {
        /// Account whose alias binding should be reconciled.
        pub account: AccountId,
        /// Desired on-chain alias for the account.
        ///
        /// `None` clears every non-primary alias currently bound to the account.
        #[norito(default)]
        pub alias: Option<crate::account::rekey::AccountAlias>,
        /// Optional lease expiry timestamp (unix ms). When provided, the authoritative SNS lease
        /// for the alias is updated before the binding is reconciled.
        #[norito(default)]
        pub lease_expiry_ms: Option<u64>,
    }
}

impl SetAccountAliasBinding {
    /// Stable wire identifier for this instruction.
    pub const WIRE_ID: &'static str = "iroha.account.alias.binding.set";

    /// Create a binding or renewal instruction.
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

    /// Create an instruction that clears all non-primary alias bindings for the account.
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

isi! {
    /// Set, update, renew, or clear the primary alias under which an account is addressed.
    ///
    /// `alias = None` clears the current primary alias.
    pub struct SetPrimaryAccountAlias {
        /// Account whose label should be reconciled.
        pub account: AccountId,
        /// Desired on-chain alias for the account. `None` clears the current primary alias.
        #[norito(default)]
        pub alias: Option<crate::account::rekey::AccountAlias>,
        /// Optional lease expiry timestamp (unix ms). When provided, the authoritative SNS lease
        /// for the alias is updated before the primary alias is reconciled.
        #[norito(default)]
        pub lease_expiry_ms: Option<u64>,
    }
}

impl SetPrimaryAccountAlias {
    /// Stable wire identifier for this instruction.
    pub const WIRE_ID: &'static str = "iroha.account.alias.primary.set";

    /// Create a primary-alias assignment or renewal instruction.
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

    /// Create an instruction that clears the current primary alias.
    #[must_use]
    pub fn clear(account: AccountId) -> Self {
        Self {
            account,
            alias: None,
            lease_expiry_ms: None,
        }
    }
}

impl crate::seal::Instruction for SetPrimaryAccountAlias {}

fn decode_alias_binding_fields(
    bytes: &[u8],
) -> Result<
    (
        AccountId,
        Option<crate::account::rekey::AccountAlias>,
        Option<u64>,
        usize,
    ),
    norito::core::Error,
> {
    let flags =
        norito::core::effective_decode_flags().unwrap_or_else(norito::core::default_encode_flags);
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
    Ok((account, alias, lease_expiry_ms, offset))
}

impl<'a> norito::core::DecodeFromSlice<'a> for SetAccountAliasBinding {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = norito::core::effective_decode_flags()
            .unwrap_or_else(norito::core::default_encode_flags);
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let (account, alias, lease_expiry_ms, used) = decode_alias_binding_fields(bytes)?;
        Ok((
            Self {
                account,
                alias,
                lease_expiry_ms,
            },
            used,
        ))
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for SetPrimaryAccountAlias {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = norito::core::effective_decode_flags()
            .unwrap_or_else(norito::core::default_encode_flags);
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let (account, alias, lease_expiry_ms, used) = decode_alias_binding_fields(bytes)?;
        Ok((
            Self {
                account,
                alias,
                lease_expiry_ms,
            },
            used,
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
        let key_pair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("derive checked domain-link fixture keypair");
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
    fn account_alias_binding_decode_from_slice_roundtrips() {
        let account = account(0xA1);
        assert_slice_roundtrip(SetAccountAliasBinding::bind(
            account.clone(),
            alias(),
            Some(1000),
        ));
        assert_slice_roundtrip(SetAccountAliasBinding::clear(account.clone()));
        assert_slice_roundtrip(SetPrimaryAccountAlias::bind(
            account.clone(),
            alias(),
            Some(2000),
        ));
        assert_slice_roundtrip(SetPrimaryAccountAlias::clear(account));
    }

    #[test]
    fn account_alias_binding_registry_decodes_existing_ids() {
        let registry = crate::isi::InstructionRegistry::new()
            .register_with_id_slice::<SetAccountAliasBinding>("identity::SetAccountAliasBinding")
            .register_with_id_slice::<SetPrimaryAccountAlias>("identity::SetPrimaryAccountAlias");
        let account = account(0xA2);

        assert_registry_decodes(
            &registry,
            "identity::SetAccountAliasBinding",
            SetAccountAliasBinding::bind(account.clone(), alias(), Some(3000)),
        );
        assert_registry_decodes(
            &registry,
            "identity::SetPrimaryAccountAlias",
            SetPrimaryAccountAlias::bind(account, alias(), Some(4000)),
        );
    }
}
