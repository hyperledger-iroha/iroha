//! Canonical key namespace for the ordinary execution-witness sparse tree.
//!
//! All key producers reserve their first byte here. Distinct enum discriminants
//! make overlapping families a compile error, including fixed synthetic writes
//! and families selected by their first byte. The enum is a key-construction
//! registry, not a separately serialized Norito value.

/// Disjoint first-byte namespaces for execution-witness keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ExecutionWitnessKeyTagV1 {
    /// Account metadata: account identifier and metadata name.
    AccountMetadata = 0xA1,
    /// Domain metadata: domain identifier and metadata name.
    DomainMetadata = 0xA2,
    /// NFT metadata: NFT identifier and metadata name.
    NftMetadata = 0xA3,
    /// Asset-definition metadata: definition identifier and metadata name.
    AssetDefinitionMetadata = 0xA4,
    /// Asset balance by asset identifier.
    AssetBalance = 0xB1,
    /// Asset-definition total supply by definition identifier.
    AssetDefinitionTotalSupply = 0xB2,
    /// Account-to-role binding by account and role identifiers.
    AccountRoleBinding = 0xC1,
    /// Role presence by role identifier.
    Role = 0xC2,
    /// Account permission by account identifier and permission name.
    AccountPermission = 0xC3,
    /// Role permission by role identifier and permission name.
    RolePermission = 0xC4,
    /// Fixed validation-fee policy-registry snapshot.
    ValidationFeePolicy = 0xD4,
    /// Fixed Parliament timed-OVN casting-context snapshot.
    ParliamentTimedOvnCasting = 0xD5,
    /// Kagemusha reserve receipt by its 32-byte operation identifier.
    KagemushaReserveReceipt = 0xD6,
    /// Fixed ordinary FASTPQ source-statement manifest, derived by validator execution.
    FastpqOrdinarySourceStatements = 0xD7,
}

const fn tagged_fixed_key<const N: usize>(
    tag: ExecutionWitnessKeyTagV1,
    mut key: [u8; N],
) -> [u8; N] {
    key[0] = tag as u8;
    key
}

/// Fixed validation-fee policy key committed by every executed block.
pub const VALIDATION_FEE_POLICY_WITNESS_KEY_V1: &[u8] = &tagged_fixed_key(
    ExecutionWitnessKeyTagV1::ValidationFeePolicy,
    *b"\0iroha:validation-fee:policy-registry:v1",
);

/// Fixed Parliament casting-context key committed by every executed block.
pub const PARLIAMENT_TIMED_OVN_CASTING_WITNESS_KEY_V1: &[u8] = &tagged_fixed_key(
    ExecutionWitnessKeyTagV1::ParliamentTimedOvnCasting,
    *b"\0iroha:parliament:timed-ovn:casting-contexts:v1",
);

/// Protected fixed key for the canonical ordinary FASTPQ source-statement manifest.
///
/// Reserving this namespace does not enable compact proof admission. Only the
/// validator-derived complete projection may be inserted under this key.
pub const FASTPQ_ORDINARY_SOURCE_STATEMENTS_WITNESS_KEY_V1: &[u8] = &tagged_fixed_key(
    ExecutionWitnessKeyTagV1::FastpqOrdinarySourceStatements,
    *b"\0iroha:fastpq:ordinary-source-statements:v1",
);

/// Reserved ordinary-write key tag for a finalized Kagemusha operation.
pub const KAGEMUSHA_RESERVE_RECEIPT_WITNESS_KEY_TAG_V1: u8 =
    ExecutionWitnessKeyTagV1::KagemushaReserveReceipt as u8;
/// Exact receipt key length: one tag byte followed by the operation identifier.
pub const KAGEMUSHA_RESERVE_RECEIPT_WITNESS_KEY_BYTES_V1: usize = 33;

/// Derive the sole ordinary-write key for a Kagemusha operation.
#[must_use]
pub const fn kagemusha_reserve_receipt_witness_key_v1(
    operation_id: [u8; 32],
) -> [u8; KAGEMUSHA_RESERVE_RECEIPT_WITNESS_KEY_BYTES_V1] {
    let mut key = [0; KAGEMUSHA_RESERVE_RECEIPT_WITNESS_KEY_BYTES_V1];
    key[0] = KAGEMUSHA_RESERVE_RECEIPT_WITNESS_KEY_TAG_V1;
    let mut index = 0;
    while index < operation_id.len() {
        key[index + 1] = operation_id[index];
        index += 1;
    }
    key
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recorder_and_synthetic_key_families_are_disjoint() {
        use ExecutionWitnessKeyTagV1::*;
        let tags = [
            AccountMetadata,
            DomainMetadata,
            NftMetadata,
            AssetDefinitionMetadata,
            AssetBalance,
            AssetDefinitionTotalSupply,
            AccountRoleBinding,
            Role,
            AccountPermission,
            RolePermission,
            ValidationFeePolicy,
            ParliamentTimedOvnCasting,
            KagemushaReserveReceipt,
            FastpqOrdinarySourceStatements,
        ];
        let distinct = tags
            .into_iter()
            .map(|tag| tag as u8)
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(distinct.len(), tags.len());
        assert_eq!(
            VALIDATION_FEE_POLICY_WITNESS_KEY_V1,
            b"\xd4iroha:validation-fee:policy-registry:v1"
        );
        assert_eq!(
            PARLIAMENT_TIMED_OVN_CASTING_WITNESS_KEY_V1,
            b"\xd5iroha:parliament:timed-ovn:casting-contexts:v1"
        );
        assert_eq!(
            FASTPQ_ORDINARY_SOURCE_STATEMENTS_WITNESS_KEY_V1,
            b"\xd7iroha:fastpq:ordinary-source-statements:v1"
        );
        for operation_id in [[0; 32], [0xD4; 32], [0xD5; 32], [0xFF; 32]] {
            let key = kagemusha_reserve_receipt_witness_key_v1(operation_id);
            assert_eq!(key[0], 0xD6);
            assert_eq!(&key[1..], operation_id.as_slice());
            for fixed in [
                VALIDATION_FEE_POLICY_WITNESS_KEY_V1,
                PARLIAMENT_TIMED_OVN_CASTING_WITNESS_KEY_V1,
                FASTPQ_ORDINARY_SOURCE_STATEMENTS_WITNESS_KEY_V1,
            ] {
                assert_ne!(key[0], fixed[0], "prefix selectors must be disjoint");
            }
        }
    }
}
