//! Native bounded asset-transfer capability instructions.

use super::*;

isi! {
    /// Register an exact, bounded, non-delegable asset-transfer capability.
    #[norito(decode_from_slice)]
    pub struct RegisterAssetTransferCapabilityV1 {
        /// Caller-computed id, checked against the immutable intent by Core.
        pub capability_id: crate::asset_transfer_capability::AssetTransferCapabilityIdV1,
        /// Complete immutable capability intent.
        pub intent: crate::asset_transfer_capability::AssetTransferCapabilityIntentV1,
    }
}

impl RegisterAssetTransferCapabilityV1 {
    /// Stable wire identifier.
    pub const WIRE_ID: &'static str = "iroha.asset_transfer_capability.register.v1";

    /// Construct a registration with the canonical id derived from `intent`.
    #[must_use]
    pub fn new(intent: crate::asset_transfer_capability::AssetTransferCapabilityIntentV1) -> Self {
        let capability_id = intent.id();
        Self {
            capability_id,
            intent,
        }
    }
}

impl crate::seal::Instruction for RegisterAssetTransferCapabilityV1 {}

isi! {
    /// Revoke an active capability using remaining-use compare-and-set.
    #[norito(decode_from_slice)]
    pub struct RevokeAssetTransferCapabilityV1 {
        /// Capability to revoke.
        pub capability_id: crate::asset_transfer_capability::AssetTransferCapabilityIdV1,
        /// Exact remaining-use value observed by the caller.
        pub expected_remaining_uses: u32,
    }
}

impl RevokeAssetTransferCapabilityV1 {
    /// Stable wire identifier.
    pub const WIRE_ID: &'static str = "iroha.asset_transfer_capability.revoke.v1";

    /// Construct a compare-and-set revocation.
    #[must_use]
    pub const fn new(
        capability_id: crate::asset_transfer_capability::AssetTransferCapabilityIdV1,
        expected_remaining_uses: u32,
    ) -> Self {
        Self {
            capability_id,
            expected_remaining_uses,
        }
    }
}

impl crate::seal::Instruction for RevokeAssetTransferCapabilityV1 {}

isi! {
    /// Atomically execute and consume one use of an asset-transfer capability.
    #[norito(decode_from_slice)]
    pub struct ExecuteAssetTransferCapabilityV1 {
        /// Capability to consume.
        pub capability_id: crate::asset_transfer_capability::AssetTransferCapabilityIdV1,
        /// Exact source repeated to prevent caller-side rebinding.
        pub source: crate::asset::AssetId,
        /// Exact destination repeated to prevent caller-side rebinding.
        pub destination: crate::account::AccountId,
        /// Exact per-use amount repeated to prevent caller-side rebinding.
        pub amount: iroha_primitives::numeric::Quantity,
        /// Exact evidence digest repeated for audit and replay safety.
        pub evidence_digest: iroha_crypto::Hash,
        /// Exact remaining-use value observed by the caller.
        pub expected_remaining_uses: u32,
    }
}

impl ExecuteAssetTransferCapabilityV1 {
    /// Stable wire identifier.
    pub const WIRE_ID: &'static str = "iroha.asset_transfer_capability.execute.v1";

    /// Construct an exact compare-and-set execution from a ledger record.
    #[must_use]
    pub fn from_record(
        record: &crate::asset_transfer_capability::AssetTransferCapabilityV1,
    ) -> Self {
        Self {
            capability_id: record.id,
            source: record.source.clone(),
            destination: record.destination.clone(),
            amount: record.amount_per_use.clone(),
            evidence_digest: record.evidence_digest,
            expected_remaining_uses: record.remaining_uses,
        }
    }
}

impl crate::seal::Instruction for ExecuteAssetTransferCapabilityV1 {}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, KeyPair};
    use norito::core::DecodeFromSlice;

    use super::*;
    use crate::{
        asset::{AssetBalanceScope, AssetDefinitionId},
        asset_transfer_capability::AssetTransferCapabilityIntentV1,
        domain::DomainId,
    };

    fn account(seed: u8) -> AccountId {
        AccountId::new(
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
                .expect("fixture key")
                .public_key()
                .clone(),
        )
    }

    fn intent() -> AssetTransferCapabilityIntentV1 {
        let grantor = account(1);
        AssetTransferCapabilityIntentV1 {
            grantor: grantor.clone(),
            delegate: account(2),
            source: AssetId::with_scope(
                AssetDefinitionId::new(
                    DomainId::try_new("cbdc", "universal").expect("domain"),
                    "ils".parse().expect("name"),
                ),
                grantor,
                AssetBalanceScope::Dataspace(crate::nexus::DataSpaceId::UNIVERSAL),
            ),
            destination: account(3),
            amount_per_use: Quantity::from(5_u32),
            evidence_digest: Hash::new(b"evidence"),
            valid_from_ms: 1,
            expires_at_ms: 2,
            initial_uses: 1,
            contract_scope: None,
            nonce: 9,
        }
    }

    fn assert_roundtrip<T>(value: T)
    where
        T: Clone + PartialEq + core::fmt::Debug + norito::codec::Encode,
        for<'a> T: DecodeFromSlice<'a>,
    {
        let encoded = value.encode();
        let (decoded, used) = T::decode_from_slice(&encoded).expect("decode");
        assert_eq!(used, encoded.len());
        assert_eq!(decoded, value);
    }

    #[test]
    fn capability_instructions_roundtrip_from_slice() {
        let register = RegisterAssetTransferCapabilityV1::new(intent());
        let record = crate::asset_transfer_capability::AssetTransferCapabilityV1::from_intent(
            register.intent.clone(),
            1,
        );
        assert_roundtrip(register);
        assert_roundtrip(RevokeAssetTransferCapabilityV1::new(record.id, 1));
        assert_roundtrip(ExecuteAssetTransferCapabilityV1::from_record(&record));
    }
}
