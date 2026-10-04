//! Native reserve-policy facts under independently selected Global finality and schema.
//!
//! Policy absence is only absence of the exact singleton. It is never proof that the entire
//! reserve namespace is empty, a manager remains eligible to submit, or a runtime may activate.

use super::{
    ReserveAuthorityPolicyRecordV1, ReserveAuthorityPolicyV1,
    history::{ReserveStateV1, STATE_MAX_BYTES, reserve_policy_permission, reserve_state_key},
};
use crate::{
    NetworkId,
    account::AccountId,
    permission::Permissions,
    sorafs::stream_token_custody::proof::borrowed,
    sumeragi_finality::{FinalityError, VerifiedSumeragiBlock, WorldStateSnapshotV1},
};
use iroha_crypto::Hash;

/// Finite aggregate response bound, never a native consensus-invalidity verdict.
pub const MAX_RESERVE_POLICY_PROOF_BYTES_V1: usize = 32 * 1024 * 1024;
/// Maximum direct permission originals admitted by this scoped reader.
pub const MAX_RESERVE_POLICY_MANAGER_PERMISSIONS_V1: usize = 256;
/// Canonical direct permission-row byte bound; no unrelated account row is exposed.
pub const MAX_RESERVE_POLICY_PERMISSION_BYTES_V1: usize = 64 * 1024;

fn invalid(reason: &str) -> FinalityError {
    FinalityError(reason.into())
}
fn map_invalid(error: impl std::fmt::Display) -> FinalityError {
    FinalityError(error.to_string())
}

/// Data-only complete World preimages, direct manager permission and original reserve frame.
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
#[norito_schema(name = "iroha_data_model::sorafs::reserve::proof::ReservePolicyProofV1")]
pub struct ReservePolicyProofV1 {
    /// Complete canonical World hash preimages at the independently selected cut.
    pub world: WorldStateSnapshotV1,
    /// Original direct permission row for the independently selected manager.
    pub manager_permissions: Permissions,
    /// Exact original singleton, or claimed absence requiring complete World authentication.
    #[norito(required)]
    pub current: Option<Vec<u8>>,
}

/// Borrowed encoder for the sole canonical proof layout; originals stay with their native owner.
#[derive(norito::derive::NoritoSerialize, norito::derive::JsonSerialize)]
pub struct ReservePolicyProofRefV1<'a> {
    world: borrowed::Value<'a, WorldStateSnapshotV1>,
    manager_permissions: borrowed::Value<'a, Permissions>,
    current: Option<borrowed::Vec<'a, u8>>,
}
impl norito::NoritoSchema for ReservePolicyProofRefV1<'_> {
    fn nominal_name() -> String {
        <ReservePolicyProofV1 as norito::NoritoSchema>::nominal_name()
    }
    fn frame_name() -> String {
        <ReservePolicyProofV1 as norito::NoritoSchema>::frame_name()
    }
}
impl<'a> ReservePolicyProofRefV1<'a> {
    /// Borrow original public evidence without creating current-state or execution authority.
    #[must_use]
    pub fn new(
        world: &'a WorldStateSnapshotV1,
        manager_permissions: &'a Permissions,
        current: Option<&'a Vec<u8>>,
    ) -> Self {
        Self {
            world: borrowed::Value(world),
            manager_permissions: borrowed::Value(manager_permissions),
            current: current.map(borrowed::Vec),
        }
    }
}

/// Exact policy facts authenticated at one caller-selected certified Global cut.
///
/// This has no public constructor or decoder. Direct permission and entity existence are facts
/// at this cut, not a transaction grant. An absent singleton is not initial namespace eligibility.
#[derive(Debug)]
pub struct VerifiedReservePolicyStateV1 {
    network_id: NetworkId,
    manager: AccountId,
    height: u64,
    context_id: Hash,
    current: Option<ReserveAuthorityPolicyRecordV1>,
}
impl VerifiedReservePolicyStateV1 {
    /// Independently selected and authenticated network.
    #[must_use]
    pub fn network_id(&self) -> NetworkId {
        self.network_id
    }
    /// Exact selected registered account with the direct canonical permission at this cut.
    #[must_use]
    pub fn manager(&self) -> &AccountId {
        &self.manager
    }
    /// Certified height; freshness is enforced independently by the caller.
    #[must_use]
    pub fn height(&self) -> u64 {
        self.height
    }
    /// Exact certified consensus context joining the proof to the independent decision.
    #[must_use]
    pub fn context_id(&self) -> Hash {
        self.context_id
    }
    /// Exact selected current policy, or authenticated singleton absence only.
    #[must_use]
    pub fn current(&self) -> Option<&ReserveAuthorityPolicyRecordV1> {
        self.current.as_ref()
    }
}

impl ReservePolicyProofV1 {
    /// Decode the sole canonical proof frame with finite transport and cumulative allocation limits.
    /// # Errors
    /// Oversized, malformed, noncanonical or resource-exhausting input.
    pub fn decode_frame(bytes: &[u8]) -> Result<Self, norito::Error> {
        if bytes.is_empty() || bytes.len() > MAX_RESERVE_POLICY_PROOF_BYTES_V1 {
            return Err(norito::Error::Message(
                "reserve policy proof exceeds its bound".into(),
            ));
        }
        norito::decode_canonical_with_limits(
            bytes,
            norito::DecodeLimits::new(
                131_072,
                MAX_RESERVE_POLICY_PROOF_BYTES_V1,
                131_072,
                128 * 1024 * 1024,
                64,
            ),
        )
    }

    /// Authenticate exact policy facts against independently selected scope and Global finality.
    ///
    /// Only the generated manager's direct unit permission is supported by this scoped reader.
    /// Missing/role-only permission is a refusal, not proof that governance authority is absent.
    /// `None` authenticates only the singleton key's absence, never the whole reserve namespace.
    /// The caller separately enforces finality freshness and initial-executor/eligibility prerequisites.
    /// # Errors
    /// Wrong scope/schema, substituted permission/policy/roles, absent selected entities, noncanonical
    /// or concealed state, invalid activation provenance, or exceeded finite reader bounds.
    #[expect(
        clippy::too_many_arguments,
        reason = "independent trust selections remain explicit"
    )]
    pub fn verify(
        &self,
        expected_chain: &str,
        expected_network: NetworkId,
        expected_manager: &AccountId,
        expected_policy: &ReserveAuthorityPolicyV1,
        expected_schema: Hash,
        block: &VerifiedSumeragiBlock,
    ) -> Result<VerifiedReservePolicyStateV1, FinalityError> {
        block.verify_global_scope(expected_network, expected_chain)?;
        expected_policy.validate().map_err(map_invalid)?;
        if block.height() < 2
            || block.commitment().schedule.current.network_id != expected_network
            || self.world.schema_hash != expected_schema
            || norito::canonical_frame_len(expected_manager).map_err(map_invalid)? > STATE_MAX_BYTES
            || norito::canonical_frame_len(expected_policy).map_err(map_invalid)? > STATE_MAX_BYTES
            || self.manager_permissions.len() > MAX_RESERVE_POLICY_MANAGER_PERMISSIONS_V1
            || norito::canonical_frame_len(&self.manager_permissions).map_err(map_invalid)?
                > MAX_RESERVE_POLICY_PERMISSION_BYTES_V1
            || !self
                .manager_permissions
                .contains(&reserve_policy_permission())
            || self
                .current
                .as_ref()
                .is_some_and(|bytes| bytes.is_empty() || bytes.len() > STATE_MAX_BYTES)
            || norito::canonical_frame_len(self).map_err(map_invalid)?
                > MAX_RESERVE_POLICY_PROOF_BYTES_V1
        {
            return Err(invalid(
                "Reserve policy proof differs from independent scope, direct permission or bounds",
            ));
        }
        let world = self.world.authenticate(block)?;
        for account in [
            expected_manager,
            &expected_policy.custody_account,
            &expected_policy.treasury_account,
            &expected_policy.operations_authority,
            &expected_policy.decision_authority,
        ] {
            world.verify_account_key_present(account)?;
        }
        world.verify_asset_definition_key_present(&expected_policy.asset_definition)?;
        world.verify_table_value(
            "world.account_permissions",
            expected_manager,
            &self.manager_permissions,
        )?;
        let current = match &self.current {
            Some(bytes) => {
                let state = verify_selected_policy(&world, expected_policy, bytes)?;
                if state.policy.activated_by != *expected_manager {
                    return Err(invalid(
                        "Reserve policy or activation differs from selected original and certified cut",
                    ));
                }
                Some(state.policy)
            }
            None => {
                world.verify_smart_contract_state_absent(reserve_state_key())?;
                None
            }
        };
        Ok(VerifiedReservePolicyStateV1 {
            network_id: expected_network,
            manager: expected_manager.clone(),
            height: world.height(),
            context_id: world.context_id(),
            current,
        })
    }
}

/// Authenticate the exact selected active singleton against an already authenticated World cut.
/// The manager-specific reader separately checks `activated_by` against its selected manager.
pub(super) fn verify_selected_policy(
    world: &crate::sumeragi_finality::VerifiedWorldStateSnapshotV1,
    expected_policy: &ReserveAuthorityPolicyV1,
    bytes: &Vec<u8>,
) -> Result<ReserveStateV1, FinalityError> {
    if bytes.is_empty() || bytes.len() > STATE_MAX_BYTES {
        return Err(invalid("Reserve policy original exceeds its bound"));
    }
    expected_policy.validate().map_err(map_invalid)?;
    world.verify_table_value("world.smart_contract_state", reserve_state_key(), bytes)?;
    let state = ReserveStateV1::decode_frame(bytes).map_err(map_invalid)?;
    if state.policy.policy != *expected_policy
        || state.policy.policy_digest != expected_policy.digest().map_err(map_invalid)?
        || state.policy.activated_at_unix > world.block_time_ms() / 1_000
        || state.journal_head.last_target_block_height > world.height()
    {
        return Err(invalid(
            "Reserve policy or activation differs from selected original and certified cut",
        ));
    }
    Ok(state)
}

#[cfg(test)]
mod tests;
