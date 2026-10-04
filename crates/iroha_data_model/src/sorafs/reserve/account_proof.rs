//! Exact provider reserve facts at one independently selected certified Global World cut.
//!
//! Registration absence is absence of one provider partition. It grants no transaction,
//! collateral, credit, provider admission or service authority. A present row retains its
//! original policy digest, which can legitimately lag the active policy after rotation.

use super::{
    ReserveAuthorityPolicyRecordV1, ReserveAuthorityPolicyV1, ReserveProviderAccountV1,
    history::{
        ReserveEventJournalHeadV1, ReserveStateV1, STATE_MAX_BYTES, decode_reserve_provider_frame,
        reserve_provider_key,
    },
    proof::verify_selected_policy,
};
use crate::{
    NetworkId,
    account::AccountId,
    sorafs::{
        capacity::{CapacityDeclarationRecord, ProviderId},
        pricing::{PricingScheduleRecord, ProviderCreditRecord},
        stream_token_custody::proof::borrowed,
    },
    sumeragi_finality::{FinalityError, VerifiedSumeragiBlock, WorldStateSnapshotV1},
};
use iroha_crypto::Hash;

/// Finite response-reader bound, never a consensus invalidity verdict.
pub const MAX_RESERVE_ACCOUNT_PROOF_BYTES_V1: usize = 32 * 1024 * 1024;
/// Maximum canonical credit-original bytes accepted by this finite evidence reader.
/// This limit is not a native ledger validity rule.
pub const MAX_RESERVE_ACCOUNT_CREDIT_BYTES_V1: usize = 2 * 1024 * 1024;
/// Finite credit-original decoder limits within the shared cumulative proof allowance.
pub const RESERVE_ACCOUNT_CREDIT_LIMITS_V1: norito::DecodeLimits = norito::DecodeLimits::new(
    4_096,
    MAX_RESERVE_ACCOUNT_CREDIT_BYTES_V1,
    32_768,
    4 * 1024 * 1024,
    64,
);
/// Finite capacity-original response bound, not a native capacity validity rule.
pub const MAX_RESERVE_ACCOUNT_CAPACITY_BYTES_V1: usize = 2 * 1024 * 1024;
/// Capacity records contain a canonical declaration byte vector; its sequence must fit the frame.
pub const RESERVE_ACCOUNT_CAPACITY_LIMITS_V1: norito::DecodeLimits = norito::DecodeLimits::new(
    MAX_RESERVE_ACCOUNT_CAPACITY_BYTES_V1,
    MAX_RESERVE_ACCOUNT_CAPACITY_BYTES_V1,
    4 * 1024 * 1024,
    8 * 1024 * 1024,
    64,
);
/// Finite pricing-original response bound, not a governed pricing validity rule.
pub const MAX_RESERVE_ACCOUNT_PRICING_BYTES_V1: usize = 64 * 1024;
/// Finite pricing decode limits inherited from the same cumulative proof allowance.
pub const RESERVE_ACCOUNT_PRICING_LIMITS_V1: norito::DecodeLimits = norito::DecodeLimits::new(
    4_096,
    MAX_RESERVE_ACCOUNT_PRICING_BYTES_V1,
    32_768,
    256 * 1024,
    64,
);
// Original policy, partition, credit, capacity and pricing are byte vectors; their sequence bound admits the
// declared STATE_MAX_BYTES. The complete World owner separately bounds its entry count.
/// Finite canonical response and verification limits, shared by SDK decode-and-verify owners.
/// Nested operations inherit any tighter caller allowance and debit the same cumulative scope.
pub const RESERVE_ACCOUNT_PROOF_LIMITS_V1: norito::DecodeLimits = norito::DecodeLimits::new(
    MAX_RESERVE_ACCOUNT_PROOF_BYTES_V1,
    MAX_RESERVE_ACCOUNT_PROOF_BYTES_V1,
    MAX_RESERVE_ACCOUNT_PROOF_BYTES_V1,
    128 * 1024 * 1024,
    64,
);

fn invalid(reason: &str) -> FinalityError {
    FinalityError(reason.into())
}
fn map_invalid(error: impl std::fmt::Display) -> FinalityError {
    FinalityError(error.to_string())
}

/// Independently selected caller claims, borrowed for one proof verification.
///
/// This struct has no codec and creates no authority. The certified block is supplied separately;
/// no response field, copied checkpoint or callback may choose these expectations for the caller.
#[derive(Debug)]
pub struct ReserveAccountProofExpectedV1<'a> {
    /// Independently selected human-readable chain label.
    pub chain: &'a str,
    /// Network derived from the independently authenticated original genesis.
    pub network_id: NetworkId,
    /// Selected signer that must equal the active policy operations authority.
    pub operator: &'a AccountId,
    /// Nonzero native provider registry id.
    pub provider_id: ProviderId,
    /// Selected native provider owner; it need not equal the operations authority.
    pub owner: &'a AccountId,
    /// Complete independently retained expected active policy, including all roles and revision.
    pub policy: &'a ReserveAuthorityPolicyV1,
    /// Independently qualified exact native World schema commitment.
    pub schema: Hash,
}

/// Original owner, active singleton, optional partition/credit/capacity, and governed pricing.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    norito::Encode,
    norito::Decode,
    iroha_schema::IntoSchema,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields, no_fast_from_json)]
#[norito_schema(name = "iroha_data_model::sorafs::reserve::account_proof::ReserveAccountProofV1")]
pub struct ReserveAccountProofV1 {
    /// Complete canonical World hash preimages, with the independently selected native schema.
    pub world: WorldStateSnapshotV1,
    /// Exact original native provider-owner row.
    pub owner: AccountId,
    /// Exact original active reserve singleton, including activation and event-head data.
    pub policy: Vec<u8>,
    /// Exact original provider partition; absence requires complete World authentication.
    #[norito(required)]
    pub current: Option<Vec<u8>>,
    /// Canonical typed native credit row; absence requires the exact complete World key to be absent.
    #[norito(required)]
    pub credit: Option<Vec<u8>>,
    /// Canonical native capacity row; None proves only this provider key is absent.
    #[norito(required)]
    pub capacity: Option<Vec<u8>>,
    /// Canonical original governed pricing cell, required even when provider rows are absent.
    pub pricing: Vec<u8>,
}

/// Borrowed encoder of the sole canonical response layout; source owners retain their values.
#[derive(norito::derive::NoritoSerialize, norito::derive::JsonSerialize)]
pub struct ReserveAccountProofRefV1<'a> {
    world: borrowed::Value<'a, WorldStateSnapshotV1>,
    owner: borrowed::Value<'a, AccountId>,
    policy: borrowed::Vec<'a, u8>,
    current: Option<borrowed::Vec<'a, u8>>,
    credit: Option<borrowed::Vec<'a, u8>>,
    capacity: Option<borrowed::Vec<'a, u8>>,
    pricing: borrowed::Vec<'a, u8>,
}
impl norito::NoritoSchema for ReserveAccountProofRefV1<'_> {
    fn nominal_name() -> String {
        <ReserveAccountProofV1 as norito::NoritoSchema>::nominal_name()
    }
    fn frame_name() -> String {
        <ReserveAccountProofV1 as norito::NoritoSchema>::frame_name()
    }
}
impl<'a> ReserveAccountProofRefV1<'a> {
    /// Borrow exact original public evidence; no signing or native State authority is created.
    #[must_use]
    pub fn new(
        world: &'a WorldStateSnapshotV1,
        owner: &'a AccountId,
        policy: &'a Vec<u8>,
        current: Option<&'a Vec<u8>>,
        credit: Option<&'a Vec<u8>>,
        capacity: Option<&'a Vec<u8>>,
        pricing: &'a Vec<u8>,
    ) -> Self {
        Self {
            world: borrowed::Value(world),
            owner: borrowed::Value(owner),
            policy: borrowed::Vec(policy),
            current: current.map(borrowed::Vec),
            credit: credit.map(borrowed::Vec),
            capacity: capacity.map(borrowed::Vec),
            pricing: borrowed::Vec(pricing),
        }
    }
}

/// Exact facts authenticated at one caller-selected Global decision, with no public decoder.
///
/// The active policy and the provider's last-projected policy digest remain separate facts.
/// A caller must independently enforce freshness and join an original successful transaction
/// carrier before asserting that its own registration completed. Balances here do not prove
/// aggregate custody backing, usable collateral, credit availability or runtime readiness.
#[derive(Debug)]
pub struct VerifiedReserveAccountStateV1 {
    network_id: NetworkId,
    operator: AccountId,
    provider_id: ProviderId,
    owner: AccountId,
    height: u64,
    context_id: Hash,
    block_time_ms: u64,
    policy: ReserveStateV1,
    current: Option<ReserveProviderAccountV1>,
    credit: Option<ProviderCreditRecord>,
    capacity: Option<CapacityDeclarationRecord>,
    pricing: PricingScheduleRecord,
}
impl VerifiedReserveAccountStateV1 {
    /// Independently selected authenticated network.
    #[must_use]
    pub fn network_id(&self) -> NetworkId {
        self.network_id
    }
    /// Selected registered operator equal to the active policy's operations authority.
    #[must_use]
    pub fn operator(&self) -> &AccountId {
        &self.operator
    }
    /// Selected provider registry identifier.
    #[must_use]
    pub fn provider_id(&self) -> ProviderId {
        self.provider_id
    }
    /// Exact selected registered native provider owner.
    #[must_use]
    pub fn owner(&self) -> &AccountId {
        &self.owner
    }
    /// Certified height; this does not enforce wall-clock freshness.
    #[must_use]
    pub fn height(&self) -> u64 {
        self.height
    }
    /// Exact certified consensus context.
    #[must_use]
    pub fn context_id(&self) -> Hash {
        self.context_id
    }
    /// Certified block timestamp, used for an independent original-carrier join.
    #[must_use]
    pub fn block_time_ms(&self) -> u64 {
        self.block_time_ms
    }
    /// Exact selected active policy and its committed activation provenance.
    #[must_use]
    pub fn policy(&self) -> &ReserveAuthorityPolicyRecordV1 {
        &self.policy.policy
    }
    /// Committed event-head data only; the event originals are required for journal continuity.
    #[must_use]
    pub fn journal_head(&self) -> &ReserveEventJournalHeadV1 {
        &self.policy.journal_head
    }
    /// Complete unchanged provider partition, or absence of this provider's exact physical key.
    #[must_use]
    pub fn current(&self) -> Option<&ReserveProviderAccountV1> {
        self.current.as_ref()
    }
    /// Exact committed credit projection, or proven absence of this provider's native credit key.
    /// These fields grant no collateral, capacity, spending or service authority.
    #[must_use]
    pub fn credit(&self) -> Option<&ProviderCreditRecord> {
        self.credit.as_ref()
    }
    /// Exact committed capacity declaration, including original bytes and validity times.
    /// This does not establish active capacity, current backing or provider admission.
    #[must_use]
    pub fn capacity(&self) -> Option<&CapacityDeclarationRecord> {
        self.capacity.as_ref()
    }
    /// Exact governed pricing cell at this cut. Existing arithmetic consumers own validation.
    /// This read neither chooses funding amounts nor freezes pricing for a later transaction.
    #[must_use]
    pub fn pricing(&self) -> &PricingScheduleRecord {
        &self.pricing
    }
}

impl ReserveAccountProofV1 {
    /// Decode the sole canonical frame under finite inherited cumulative allocation limits.
    /// # Errors
    /// Oversized, malformed, noncanonical or resource-exhausting input.
    pub fn decode_frame(bytes: &[u8]) -> Result<Self, norito::Error> {
        if bytes.is_empty() || bytes.len() > MAX_RESERVE_ACCOUNT_PROOF_BYTES_V1 {
            return Err(norito::Error::Message(
                "reserve account proof exceeds its bound".into(),
            ));
        }
        norito::decode_canonical_with_limits(bytes, RESERVE_ACCOUNT_PROOF_LIMITS_V1)
    }

    /// Authenticate exact owner, policy, partition/credit/capacity facts and required pricing.
    ///
    /// Every trust input is independent of the response. Only the common Global finality and
    /// complete World owners verify consensus or inclusion; there is no second verifier.
    /// Native registration, installed-executor policy and fee eligibility remain separate.
    /// # Errors
    /// Wrong scope/schema/operator/owner/policy, absent selected entities, concealed or malformed
    /// originals, future provenance, or finite reader refusal.
    pub fn verify(
        &self,
        expected: &ReserveAccountProofExpectedV1<'_>,
        block: &VerifiedSumeragiBlock,
    ) -> Result<VerifiedReserveAccountStateV1, FinalityError> {
        let expected_chain = expected.chain;
        let expected_network = expected.network_id;
        let expected_operator = expected.operator;
        let expected_provider = expected.provider_id;
        let expected_owner = expected.owner;
        let expected_policy = expected.policy;
        let expected_schema = expected.schema;
        block.verify_global_scope(expected_network, expected_chain)?;
        if block.height() < 2
            || block.commitment().schedule.current.network_id != expected_network
            || self.world.schema_hash != expected_schema
            || expected_provider.as_bytes() == &[0; 32]
            || self.owner != *expected_owner
            || expected_policy.operations_authority != *expected_operator
            || self.policy.is_empty()
            || self.policy.len() > STATE_MAX_BYTES
            || self
                .current
                .as_ref()
                .is_some_and(|bytes| bytes.is_empty() || bytes.len() > STATE_MAX_BYTES)
            || self.credit.as_ref().is_some_and(|bytes| {
                bytes.is_empty() || bytes.len() > MAX_RESERVE_ACCOUNT_CREDIT_BYTES_V1
            })
            || self.capacity.as_ref().is_some_and(|bytes| {
                bytes.is_empty() || bytes.len() > MAX_RESERVE_ACCOUNT_CAPACITY_BYTES_V1
            })
            || self.pricing.is_empty()
            || self.pricing.len() > MAX_RESERVE_ACCOUNT_PRICING_BYTES_V1
            || norito::canonical_frame_len(expected_operator).map_err(map_invalid)?
                > STATE_MAX_BYTES
            || norito::canonical_frame_len(expected_owner).map_err(map_invalid)? > STATE_MAX_BYTES
            || norito::canonical_frame_len(expected_policy).map_err(map_invalid)? > STATE_MAX_BYTES
            || norito::canonical_frame_len(self).map_err(map_invalid)?
                > MAX_RESERVE_ACCOUNT_PROOF_BYTES_V1
        {
            return Err(invalid(
                "Reserve account proof differs from independent scope or bounds",
            ));
        }
        // All nested originals share one finite scope. Inherited tighter limits and original
        // refusals remain authoritative; each nested decode must not receive a fresh allowance.
        norito::core::with_decode_limits_scope(RESERVE_ACCOUNT_PROOF_LIMITS_V1, || {
            let world = self.world.authenticate(block)?;
            world.verify_table_value(
                "world.provider_owners",
                &expected_provider,
                expected_owner,
            )?;
            let policy = verify_selected_policy(&world, expected_policy, &self.policy)?;
            for account in [
                expected_operator,
                expected_owner,
                &expected_policy.custody_account,
                &expected_policy.treasury_account,
                &expected_policy.decision_authority,
            ] {
                world.verify_account_key_present(account)?;
            }
            world.verify_asset_definition_key_present(&expected_policy.asset_definition)?;
            let key = reserve_provider_key(expected_provider);
            let current = if let Some(bytes) = &self.current {
                world.verify_table_value("world.smart_contract_state", &key, bytes)?;
                let account =
                    decode_reserve_provider_frame(bytes, expected_provider).map_err(map_invalid)?;
                if account.terms.provider_account != *expected_owner
                    || account.updated_at_unix > world.block_time_ms() / 1_000
                {
                    return Err(invalid(
                        "Reserve provider owner or timestamp differs from certified cut",
                    ));
                }
                // Do not normalize policy_digest or recompute a credit cap. Native updates
                // perform that projection lazily; this reader must preserve original bytes.
                Some(account)
            } else {
                world.verify_smart_contract_state_absent(&key)?;
                None
            };
            let credit = if let Some(bytes) = &self.credit {
                // The complete World is already authenticated. Decode exactly once; the
                // native table stores the typed record, not this response's byte vector.
                let record: ProviderCreditRecord =
                    norito::decode_canonical_with_limits(bytes, RESERVE_ACCOUNT_CREDIT_LIMITS_V1)
                        .map_err(map_invalid)?;
                if record.provider_id != expected_provider {
                    return Err(invalid("Reserve credit provider differs from selected key"));
                }
                world.verify_table_value(
                    "world.provider_credit_ledger",
                    &expected_provider,
                    &record,
                )?;
                // Preserve all native projection values, even when settlement/telemetry has
                // not caught up. No copied Metadata graph or additional policy is imposed.
                Some(record)
            } else {
                world.verify_provider_credit_absent(&expected_provider)?;
                None
            };
            let capacity = if let Some(bytes) = &self.capacity {
                let record: CapacityDeclarationRecord =
                    norito::decode_canonical_with_limits(bytes, RESERVE_ACCOUNT_CAPACITY_LIMITS_V1)
                        .map_err(map_invalid)?;
                if record.provider_id != expected_provider {
                    return Err(invalid(
                        "Reserve capacity provider differs from selected key",
                    ));
                }
                world.verify_table_value(
                    "world.capacity_declarations",
                    &expected_provider,
                    &record,
                )?;
                // Exact committed facts: native execution owns payload semantics, current
                // backing and active allocations. Expired or future rows remain unchanged.
                Some(record)
            } else {
                world.verify_capacity_declaration_absent(&expected_provider)?;
                None
            };
            let pricing: PricingScheduleRecord = norito::decode_canonical_with_limits(
                &self.pricing,
                RESERVE_ACCOUNT_PRICING_LIMITS_V1,
            )
            .map_err(map_invalid)?;
            world.verify_cell_value("world.sorafs_pricing", &pricing)?;
            // No hidden eligibility rule: existing pricing arithmetic validates the schedule
            // when a caller actually selects economics. This proof authenticates its original.
            Ok(VerifiedReserveAccountStateV1 {
                network_id: expected_network,
                operator: expected_operator.clone(),
                provider_id: expected_provider,
                owner: expected_owner.clone(),
                height: world.height(),
                context_id: world.context_id(),
                block_time_ms: world.block_time_ms(),
                policy,
                current,
                credit,
                capacity,
                pricing,
            })
        })
    }
}

#[cfg(test)]
mod tests;
