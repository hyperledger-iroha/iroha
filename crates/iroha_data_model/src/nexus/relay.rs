//! Proof-backed cross-lane fee sponsor vault allocations.
//!
//! A verified allocation binds a sponsor program's authoritative source-vault
//! snapshot, spend lease and `FastPQ` proof so sponsored fees can be charged
//! deterministically against the verified amount.

use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use crate::{
    asset::AssetDefinitionId,
    nexus::{AxtFastpqBinding, FeeSponsorProgramId},
};
use iroha_crypto::Hash;
use iroha_model_base::topology::DataSpaceId;
use iroha_primitives::numeric::Quantity;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
/// Prefix for contract-visible verified fee sponsor vault-allocation keys.
pub const VERIFIED_FEE_SPONSOR_VAULT_ALLOCATION_STATE_KEY_PREFIX: &str =
    "pkdeploy_verified_fee_sponsor_vault_allocation";
/// Prefix for cumulative spend recorded against a verified sponsor-vault allocation.
pub const VERIFIED_FEE_SPONSOR_VAULT_ALLOCATION_USAGE_STATE_KEY_PREFIX: &str =
    "pkdeploy_fee_sponsor_vault_allocation_usage";
/// Prefix for cumulative merge-settled spend against a verified allocation.
pub const VERIFIED_FEE_SPONSOR_VAULT_ALLOCATION_SETTLED_USAGE_STATE_KEY_PREFIX: &str =
    "pkdeploy_fee_sponsor_vault_allocation_settled_usage";
const FEE_SPONSOR_VAULT_SOURCE_STATE_ROOT_DOMAIN_V1: &[u8] =
    b"iroha.nexus.fee-sponsor-vault.source-state.v1";
const FEE_SPONSOR_VAULT_ALLOCATION_CLAIM_DOMAIN_V1: &[u8] =
    b"iroha.nexus.fee-sponsor-vault.allocation-claim.v1";
const FEE_SPONSOR_VAULT_POLICY_COMMITMENT_DOMAIN_V1: &[u8] =
    b"nexus-fee-relay:sponsor-vault-policy:v1";
fn domain_separated_hash(domain: &[u8], payload: &[u8]) -> Hash {
    let domain_len = u64::try_from(domain.len())
        .expect("protocol-defined digest domains fit in u64")
        .to_le_bytes();
    Hash::new_from_chunks(&[&domain_len, domain, payload])
}
/// Proof-backed cross-lane spend allocation for one sponsor-program vault asset.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::nexus::relay::VerifiedFeeSponsorVaultAllocation")]
pub struct VerifiedFeeSponsorVaultAllocation {
    /// Exact sponsor program authorized to consume the allocation.
    pub program_id: FeeSponsorProgramId,
    /// Immutable program revision bound by the source proof.
    pub program_revision: u64,
    /// Canonical fee asset allocated by the source vault.
    pub asset_definition_id: AssetDefinitionId,
    /// Maximum amount authorized by this spend lease.
    pub verified_allocation: Quantity,
    /// Source dataspace that owns the authoritative program vault.
    pub source_dataspace_id: DataSpaceId,
    /// Monotonic source consensus height bound by the proof.
    pub source_height: u64,
    /// Source state root that commits the vault allocation and counters.
    pub source_state_root: Hash,
    /// Consensus height after which the allocation cannot admit new charges.
    pub expires_at_height: u64,
    /// Globally unique proof-bound spend lease identifier.
    pub lease_id: Hash,
    /// Deterministic hash of the proof payload used during registration.
    pub proof_payload_hash: Hash,
    /// `FastPQ` statement digest verified during registration.
    pub fastpq_statement_digest: [u8; 32],
    /// Deterministic digest of the embedded `FastPQ` proof payload.
    pub fastpq_proof_digest: Hash,
    /// Block height where the balance proof was verified and persisted.
    pub verified_at_height: u64,
    /// Manifest root enforced during registration.
    pub manifest_root: [u8; 32],
    /// FASTPQ binding that admission consumes on-ledger.
    pub fastpq_binding: AxtFastpqBinding,
}
/// Canonical source-ledger claim authorized by a sponsor-vault spend lease.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::nexus::relay::FeeSponsorVaultAllocationClaim")]
pub struct FeeSponsorVaultAllocationClaim {
    /// Exact sponsor program authorized to spend the allocation.
    pub program_id: FeeSponsorProgramId,
    /// Immutable program revision bound by the source proof.
    pub program_revision: u64,
    /// Canonical fee asset allocated by the source vault.
    pub asset_definition_id: AssetDefinitionId,
    /// Maximum amount authorized by this spend lease.
    pub verified_allocation: Quantity,
    /// Dataspace containing the authoritative source vault.
    pub source_dataspace_id: DataSpaceId,
    /// Monotonic source consensus height bound by the proof.
    pub source_height: u64,
    /// Source state root committing the vault and budget state.
    pub source_state_root: Hash,
    /// Consensus height after which the spend lease expires.
    pub expires_at_height: u64,
    /// Globally unique proof-bound spend lease identifier.
    pub lease_id: Hash,
}
#[derive(Clone, Debug, Encode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::nexus::relay::FeeSponsorVaultSourceStateCommitment")]
struct FeeSponsorVaultSourceStateCommitment {
    version: u8,
    program_id: FeeSponsorProgramId,
    program_revision: u64,
    asset_definition_id: AssetDefinitionId,
    vault_balance: Quantity,
    source_dataspace_id: DataSpaceId,
    source_height: u64,
}
/// Commit the exact source-vault snapshot from which a relay allocation was derived.
///
/// Registration recomputes this commitment from authoritative world state. This
/// prevents a valid proof over a self-declared amount from allocating more than
/// the isolated program vault actually contains.
#[must_use]
pub fn fee_sponsor_vault_source_state_root(
    program_id: &FeeSponsorProgramId,
    program_revision: u64,
    asset_definition_id: &AssetDefinitionId,
    vault_balance: &Quantity,
    source_dataspace_id: DataSpaceId,
    source_height: u64,
) -> Hash {
    let commitment = FeeSponsorVaultSourceStateCommitment {
        version: 1,
        program_id: program_id.clone(),
        program_revision,
        asset_definition_id: asset_definition_id.clone(),
        vault_balance: vault_balance.clone(),
        source_dataspace_id,
        source_height,
    };
    domain_separated_hash(
        FEE_SPONSOR_VAULT_SOURCE_STATE_ROOT_DOMAIN_V1,
        &commitment.encode(),
    )
}
/// Compute the canonical claim digest for a verified sponsor-vault allocation proof.
#[must_use]
pub fn fee_sponsor_vault_allocation_claim_digest(
    allocation: &FeeSponsorVaultAllocationClaim,
) -> Hash {
    domain_separated_hash(
        FEE_SPONSOR_VAULT_ALLOCATION_CLAIM_DOMAIN_V1,
        &allocation.encode(),
    )
}
/// Commit the exact manifest policy authorized for a sponsor-vault allocation proof.
///
/// The framing intentionally matches the Nexus fee-relay worker transcript:
/// domain, one NUL separator, the little-endian manifest length, then the
/// manifest root. Producers and consensus consumers must use this helper so a
/// proof cannot advertise an owner-selected policy commitment.
#[must_use]
pub fn fee_sponsor_vault_policy_commitment(manifest_root: &[u8; 32]) -> Hash {
    let separator = [0_u8];
    let manifest_len = u64::try_from(manifest_root.len())
        .expect("fixed manifest root length fits in u64")
        .to_le_bytes();
    Hash::new_from_chunks(&[
        FEE_SPONSOR_VAULT_POLICY_COMMITMENT_DOMAIN_V1,
        &separator,
        &manifest_len,
        manifest_root,
    ])
}
impl VerifiedFeeSponsorVaultAllocation {
    /// Construct a verified vault allocation from canonical verified inputs.
    #[must_use]
    #[expect(
        clippy::too_many_arguments,
        reason = "the constructor mirrors the canonical verified record fields"
    )]
    pub fn new(
        program_id: FeeSponsorProgramId,
        program_revision: u64,
        asset_definition_id: AssetDefinitionId,
        verified_allocation: Quantity,
        source_dataspace_id: DataSpaceId,
        source_height: u64,
        source_state_root: Hash,
        expires_at_height: u64,
        lease_id: Hash,
        proof_payload_hash: Hash,
        fastpq_statement_digest: [u8; 32],
        fastpq_proof_digest: Hash,
        verified_at_height: u64,
        manifest_root: [u8; 32],
        fastpq_binding: AxtFastpqBinding,
    ) -> Self {
        Self {
            program_id,
            program_revision,
            asset_definition_id,
            verified_allocation,
            source_dataspace_id,
            source_height,
            source_state_root,
            expires_at_height,
            lease_id,
            proof_payload_hash,
            fastpq_statement_digest,
            fastpq_proof_digest,
            verified_at_height,
            manifest_root,
            fastpq_binding,
        }
    }
    /// Return the canonical contract-state key for this exact spend lease.
    #[must_use]
    pub fn state_key_for(
        program_id: &FeeSponsorProgramId,
        asset_definition_id: &AssetDefinitionId,
        lease_id: &Hash,
    ) -> String {
        let material = format!("{program_id}|{asset_definition_id}|{lease_id}");
        let suffix = Hash::new(material.as_bytes());
        format!(
            "{VERIFIED_FEE_SPONSOR_VAULT_ALLOCATION_STATE_KEY_PREFIX}_{}",
            hex::encode(suffix.as_ref())
        )
    }
    /// Return the canonical state key for cumulative spend against one proof-bound lease.
    #[must_use]
    pub fn usage_state_key_for(lease_id: &Hash) -> String {
        format!(
            "{VERIFIED_FEE_SPONSOR_VAULT_ALLOCATION_USAGE_STATE_KEY_PREFIX}_{}",
            hex::encode(lease_id.as_ref())
        )
    }
    /// Return the canonical state key for cumulative merge-settled spend on one lease.
    #[must_use]
    pub fn settled_usage_state_key_for(lease_id: &Hash) -> String {
        format!(
            "{VERIFIED_FEE_SPONSOR_VAULT_ALLOCATION_SETTLED_USAGE_STATE_KEY_PREFIX}_{}",
            hex::encode(lease_id.as_ref())
        )
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::AccountId;
    use iroha_crypto::{Hash, KeyPair};
    use iroha_primitives::numeric::Quantity;
    fn checked_account_id() -> AccountId {
        AccountId::new(
            KeyPair::try_random()
                .expect("generate checked Nexus relay fixture keypair")
                .public_key()
                .clone(),
        )
    }
    fn test_fastpq_binding(source_dsid: DataSpaceId, effect_type: &str) -> AxtFastpqBinding {
        AxtFastpqBinding {
            parameter: "fastpq-state-transition-stark-v1".to_owned(),
            source_dsid: source_dsid.as_u64(),
            source_dataspace: format!("ds-{}", source_dsid.as_u64()),
            source_receipt_id: "relay-receipt".to_owned(),
            source_tx_commitment: "11".repeat(32),
            claim_type: effect_type.to_owned(),
            claim_digest: "22".repeat(32),
            witness_commitment: "33".repeat(32),
            policy_commitment: "44".repeat(32),
            verified_effect_type: effect_type.to_owned(),
            corridor: "relay".to_owned(),
            verifier_id: "fastpq".to_owned(),
            verifier_version: "v1".to_owned(),
            target_dsids: vec![source_dsid.as_u64()],
            effect_binding: None,
            remote_spend_intent_commitments: Vec::new(),
        }
    }
    #[test]
    fn verified_fee_sponsor_allocation_json_is_exact_and_requires_statement_digest() {
        let program_id = FeeSponsorProgramId::new(
            checked_account_id(),
            "retail".parse().expect("program name"),
        );
        let asset_definition_id = "66owaQmAQMuHxPzxUN3bqZ6FJfDa"
            .parse()
            .expect("canonical asset definition id");
        let record = VerifiedFeeSponsorVaultAllocation::new(
            program_id,
            3,
            asset_definition_id,
            Quantity::from(10_u32),
            DataSpaceId::new(2),
            40,
            Hash::new(b"fee-sponsor-source-state"),
            100,
            Hash::new(b"fee-sponsor-lease"),
            Hash::new(b"fee-sponsor-proof-payload"),
            [0x55; 32],
            Hash::new(b"fee-sponsor-proof-digest"),
            41,
            [0x66; 32],
            test_fastpq_binding(DataSpaceId::new(2), "fee_sponsor_vault_allocation"),
        );
        let mut missing =
            norito::json::to_value(&record).expect("serialize verified sponsor allocation");
        missing
            .as_object_mut()
            .expect("verified sponsor allocation JSON object")
            .remove("fastpq_statement_digest");
        assert!(
            norito::json::from_value::<VerifiedFeeSponsorVaultAllocation>(missing).is_err(),
            "the first-release sponsor allocation must require its statement digest"
        );

        let mut unknown =
            norito::json::to_value(&record).expect("serialize verified sponsor allocation");
        unknown
            .as_object_mut()
            .expect("verified sponsor allocation JSON object")
            .insert("pre_release_field".to_owned(), norito::json::Value::Null);
        assert!(
            norito::json::from_value::<VerifiedFeeSponsorVaultAllocation>(unknown).is_err(),
            "the first-release sponsor allocation must reject unknown fields"
        );
    }

    #[test]
    fn fee_sponsor_vault_claim_digest_binds_program_asset_amount_and_lease() {
        let claim = FeeSponsorVaultAllocationClaim {
            program_id: FeeSponsorProgramId::new(
                checked_account_id(),
                "retail".parse().expect("program name"),
            ),
            program_revision: 3,
            asset_definition_id: "66owaQmAQMuHxPzxUN3bqZ6FJfDa"
                .parse()
                .expect("canonical asset definition id"),
            verified_allocation: Quantity::from(10_u32),
            source_dataspace_id: DataSpaceId::new(2),
            source_height: 40,
            source_state_root: Hash::new(b"source-state"),
            expires_at_height: 100,
            lease_id: Hash::new(b"lease-a"),
        };
        let original = fee_sponsor_vault_allocation_claim_digest(&claim);
        let ambient = {
            let alternate_flags =
                norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
            let _ambient = norito::core::DecodeFlagsGuard::enter(alternate_flags);
            fee_sponsor_vault_allocation_claim_digest(&claim)
        };
        assert_eq!(
            ambient, original,
            "the sponsor allocation proof preimage must ignore ambient Norito layout"
        );
        let mut changed = claim.clone();
        changed.verified_allocation = Quantity::from(11_u32);
        assert_ne!(
            original,
            fee_sponsor_vault_allocation_claim_digest(&changed)
        );
        changed = claim.clone();
        changed.lease_id = Hash::new(b"lease-b");
        assert_ne!(
            original,
            fee_sponsor_vault_allocation_claim_digest(&changed)
        );
        changed = claim;
        changed.program_revision = 4;
        assert_ne!(
            original,
            fee_sponsor_vault_allocation_claim_digest(&changed)
        );
    }
    #[test]
    fn fee_sponsor_vault_policy_commitment_is_pinned_and_manifest_bound() {
        let manifest_root = [0x63; 32];
        let commitment = fee_sponsor_vault_policy_commitment(&manifest_root);
        assert_eq!(
            hex::encode(commitment.as_ref()),
            "ba7798eeac2858b268b812321a7f45acdb651b921b47eeae55ea41177f0dc85b"
        );
        assert_ne!(commitment, fee_sponsor_vault_policy_commitment(&[0x64; 32]));
    }
    #[test]
    fn fee_sponsor_vault_source_root_binds_authoritative_snapshot() {
        let program_id = FeeSponsorProgramId::new(
            checked_account_id(),
            "retail".parse().expect("program name"),
        );
        let asset_definition_id = "66owaQmAQMuHxPzxUN3bqZ6FJfDa"
            .parse()
            .expect("canonical asset definition id");
        let original = fee_sponsor_vault_source_state_root(
            &program_id,
            3,
            &asset_definition_id,
            &Quantity::from(10_u32),
            DataSpaceId::new(2),
            40,
        );
        let ambient = {
            let alternate_flags =
                norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
            let _ambient = norito::core::DecodeFlagsGuard::enter(alternate_flags);
            fee_sponsor_vault_source_state_root(
                &program_id,
                3,
                &asset_definition_id,
                &Quantity::from(10_u32),
                DataSpaceId::new(2),
                40,
            )
        };
        assert_eq!(
            ambient, original,
            "the sponsor source-state commitment must ignore ambient Norito layout"
        );
        assert_ne!(
            original,
            fee_sponsor_vault_source_state_root(
                &program_id,
                3,
                &asset_definition_id,
                &Quantity::from(11_u32),
                DataSpaceId::new(2),
                40,
            )
        );
        assert_ne!(
            original,
            fee_sponsor_vault_source_state_root(
                &program_id,
                3,
                &asset_definition_id,
                &Quantity::from(10_u32),
                DataSpaceId::new(3),
                40,
            )
        );
    }
    #[test]
    fn fee_sponsor_vault_allocation_usage_keys_are_lease_bound_and_disjoint() {
        let first = Hash::new(b"fee-sponsor-lease-a");
        let second = Hash::new(b"fee-sponsor-lease-b");
        let executed = VerifiedFeeSponsorVaultAllocation::usage_state_key_for(&first);
        let settled = VerifiedFeeSponsorVaultAllocation::settled_usage_state_key_for(&first);
        assert!(executed.starts_with(VERIFIED_FEE_SPONSOR_VAULT_ALLOCATION_USAGE_STATE_KEY_PREFIX));
        assert!(
            settled
                .starts_with(VERIFIED_FEE_SPONSOR_VAULT_ALLOCATION_SETTLED_USAGE_STATE_KEY_PREFIX)
        );
        assert_ne!(executed, settled);
        assert_ne!(
            executed,
            VerifiedFeeSponsorVaultAllocation::usage_state_key_for(&second)
        );
        assert!(!executed.starts_with(VERIFIED_FEE_SPONSOR_VAULT_ALLOCATION_STATE_KEY_PREFIX));
        assert!(!settled.starts_with(VERIFIED_FEE_SPONSOR_VAULT_ALLOCATION_STATE_KEY_PREFIX));
    }
}

#[cfg(test)]
mod captured_relay_schema_tests;
