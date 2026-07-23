//! Chain-authoritative SoraFS reserve, rent, credit, and appeal handlers.

use std::{str::FromStr, sync::OnceLock};

use iroha_data_model::{
    account::AccountId,
    asset::AssetId,
    events::data::sorafs::{
        SorafsGatewayEvent, SorafsReserveLedgerEvent, SorafsReserveLedgerEventKind,
    },
    isi::{
        error::{InstructionExecutionError, InvalidParameterError},
        sorafs::{
            AdvanceSorafsReserveLifecycle, ChargeSorafsReserveRent, DecideSorafsReserveAppeal,
            DecideSorafsReserveMovement, DrawSorafsReserveCredit, RegisterSorafsReserveAccount,
            RepaySorafsReserveCredit, RequestSorafsReserveMovement, SetSorafsReservePolicy,
            SubmitSorafsReserveAppeal,
        },
    },
    name::Name,
    permission::Permission,
    query::{
        error::{FindError, QueryExecutionFail},
        sorafs::prelude::{
            FindSorafsReserveAppealById, FindSorafsReserveMovementById, FindSorafsReservePolicy,
            FindSorafsReserveProviderById,
        },
    },
    sorafs::{
        capacity::ProviderId,
        reserve::{
            RESERVE_MAX_REASON_BYTES_V1, ReserveAppealRecordV1, ReserveAppealStatusV1,
            ReserveAuthorityPolicyRecordV1, ReserveAuthorityPolicyV1, ReserveLifecycleStage,
            ReserveMovementKindV1, ReserveMovementRecordV1, ReserveMovementStatusV1,
            ReserveProviderAccountV1, ReserveTier,
        },
    },
};
use iroha_primitives::json::Json;
use mv::storage::StorageReadOnly;
use norito::{DecodeLimits, decode_from_bytes_with_limits};
use sorafs_manifest::deal::XorQuantity;

use super::*;
use crate::smartcontracts::ValidSingularQuery;
use crate::state::{StateTransaction, WorldReadOnly};

const POLICY_STATE_KEY: &str = "sorafs_reserve_policy_v1";
const PROVIDER_STATE_KEY_PREFIX: &str = "sorafs_reserve_provider_v1_";
const MOVEMENT_STATE_KEY_PREFIX: &str = "sorafs_reserve_movement_v1_";
const APPEAL_STATE_KEY_PREFIX: &str = "sorafs_reserve_appeal_v1_";
const STATE_MAX_BYTES: usize = 2 * 1024 * 1024;
const STATE_LIMITS: DecodeLimits =
    DecodeLimits::new(4_096, STATE_MAX_BYTES, 32_768, STATE_MAX_BYTES * 2, 64);

fn invalid_parameter(message: impl Into<String>) -> InstructionExecutionError {
    InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
        message.into(),
    ))
}

fn corrupt_state(message: impl Into<String>) -> InstructionExecutionError {
    InstructionExecutionError::InvariantViolation(message.into().into())
}

#[allow(clippy::too_many_arguments)]
fn emit_reserve_event(
    state_transaction: &mut StateTransaction<'_, '_>,
    kind: SorafsReserveLedgerEventKind,
    provider_id: Option<ProviderId>,
    operation_id: Option<[u8; 32]>,
    policy_digest: [u8; 32],
    provider_revision: u64,
    authority: &AccountId,
    now_unix: u64,
) {
    state_transaction
        .world
        .emit_events(Some(SorafsGatewayEvent::ReserveLedger(
            SorafsReserveLedgerEvent {
                kind,
                provider_id,
                operation_id,
                policy_digest,
                provider_revision,
                authority: authority.clone(),
                occurred_at_unix_ms: now_unix * 1_000,
            },
        )));
}

fn has_permission(
    state_transaction: &StateTransaction<'_, '_>,
    authority: &AccountId,
    permission: &str,
) -> bool {
    let required = Permission::new(permission.to_owned(), Json::new(()));
    if state_transaction
        .world
        .account_permissions
        .get(authority)
        .is_some_and(|permissions| permissions.iter().any(|candidate| candidate == &required))
    {
        return true;
    }

    state_transaction
        .world
        .account_roles_iter(authority)
        .filter_map(|role_id| state_transaction.world.roles.get(role_id))
        .any(|role| role.permissions().any(|candidate| candidate == &required))
}

fn require_governance(
    state_transaction: &StateTransaction<'_, '_>,
    authority: &AccountId,
) -> Result<(), InstructionExecutionError> {
    if has_permission(state_transaction, authority, "CanSetSorafsReservePolicy") {
        Ok(())
    } else {
        Err(invalid_parameter(
            "CanSetSorafsReservePolicy is required for authoritative reserve governance",
        ))
    }
}

fn policy_key() -> &'static Name {
    static KEY: OnceLock<Name> = OnceLock::new();
    KEY.get_or_init(|| Name::from_str(POLICY_STATE_KEY).expect("static state key is valid"))
}

fn digest_key(prefix: &str, digest: [u8; 32]) -> Name {
    Name::from_str(&format!("{prefix}{}", hex::encode(digest)))
        .expect("static prefix plus lowercase hex is a valid state key")
}

fn provider_key(provider_id: ProviderId) -> Name {
    digest_key(PROVIDER_STATE_KEY_PREFIX, *provider_id.as_bytes())
}

fn movement_key(movement_id: [u8; 32]) -> Name {
    digest_key(MOVEMENT_STATE_KEY_PREFIX, movement_id)
}

fn appeal_key(appeal_id: [u8; 32]) -> Name {
    digest_key(APPEAL_STATE_KEY_PREFIX, appeal_id)
}

fn encode_state<T: norito::core::NoritoSerialize>(
    value: &T,
    label: &str,
) -> Result<Vec<u8>, InstructionExecutionError> {
    norito::to_bytes(value)
        .map_err(|error| corrupt_state(format!("failed to encode {label}: {error}")))
}

fn decode_state<T>(bytes: &[u8], label: &str) -> Result<T, InstructionExecutionError>
where
    for<'de> T: norito::core::NoritoDeserialize<'de> + norito::core::NoritoSerialize,
{
    if bytes.len() > STATE_MAX_BYTES {
        return Err(corrupt_state(format!(
            "{label} state exceeds {STATE_MAX_BYTES} bytes"
        )));
    }
    let value = decode_from_bytes_with_limits::<T>(bytes, STATE_LIMITS)
        .map_err(|error| corrupt_state(format!("failed to decode {label}: {error}")))?;
    if encode_state(&value, label)? != bytes {
        return Err(corrupt_state(format!(
            "{label} state is not exact canonical Norito"
        )));
    }
    Ok(value)
}

fn now_unix(
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<u64, InstructionExecutionError> {
    let now = state_transaction.block_unix_timestamp_ms() / 1_000;
    if now == 0 {
        return Err(invalid_parameter(
            "authoritative reserve operations require a non-zero block timestamp",
        ));
    }
    Ok(now)
}

fn read_policy(
    world: &impl WorldReadOnly,
) -> Result<Option<ReserveAuthorityPolicyRecordV1>, InstructionExecutionError> {
    let Some(bytes) = world.smart_contract_state().get(policy_key()) else {
        return Ok(None);
    };
    let record: ReserveAuthorityPolicyRecordV1 = decode_state(bytes, "reserve policy")?;
    record
        .policy
        .validate()
        .map_err(|error| corrupt_state(format!("invalid stored reserve policy: {error}")))?;
    let digest = record
        .policy
        .digest()
        .map_err(|error| corrupt_state(format!("failed to digest reserve policy: {error}")))?;
    if digest != record.policy_digest || record.activated_at_unix == 0 {
        return Err(corrupt_state(
            "stored reserve policy digest or activation timestamp is invalid",
        ));
    }
    Ok(Some(record))
}

fn active_policy(
    state_transaction: &StateTransaction<'_, '_>,
    expected_digest: [u8; 32],
) -> Result<(ReserveAuthorityPolicyRecordV1, u64), InstructionExecutionError> {
    let now = now_unix(state_transaction)?;
    let record = read_policy(state_transaction.world())?
        .ok_or_else(|| invalid_parameter("SoraFS reserve policy is not configured"))?;
    if record.policy_digest != expected_digest {
        return Err(invalid_parameter(format!(
            "reserve policy digest mismatch: supplied {}, active {}",
            hex::encode(expected_digest),
            hex::encode(record.policy_digest)
        )));
    }
    if record.activated_at_unix > now {
        return Err(corrupt_state(
            "stored reserve policy activation is later than the current block",
        ));
    }
    Ok((record, now))
}

fn registered_provider_owner(
    world: &impl WorldReadOnly,
    provider_id: ProviderId,
) -> Result<AccountId, InstructionExecutionError> {
    world
        .provider_owners()
        .get(&provider_id)
        .cloned()
        .ok_or_else(|| invalid_parameter(format!("unknown SoraFS provider {provider_id}")))
}

fn credit_cap(
    policy: &ReserveAuthorityPolicyV1,
    terms: &iroha_data_model::sorafs::reserve::ReserveProviderTermsV1,
) -> Result<XorQuantity, InstructionExecutionError> {
    let quote = policy
        .economics
        .quote(
            terms.storage_class,
            terms.capacity_gib,
            terms.duration,
            terms.tier,
            XorQuantity::zero(),
        )
        .map_err(|error| invalid_parameter(format!("invalid reserve account terms: {error}")))?;
    Ok(quote.credit_line_cap.map_or_else(XorQuantity::zero, |cap| {
        XorQuantity::min(&cap, &policy.max_provider_debt)
    }))
}

fn read_provider(
    world: &impl WorldReadOnly,
    provider_id: ProviderId,
) -> Result<Option<ReserveProviderAccountV1>, InstructionExecutionError> {
    let Some(bytes) = world.smart_contract_state().get(&provider_key(provider_id)) else {
        return Ok(None);
    };
    let account: ReserveProviderAccountV1 = decode_state(bytes, "reserve provider account")?;
    if account.terms.provider_id != provider_id
        || account.terms.capacity_gib == 0
        || account.policy_digest == [0; 32]
        || account.revision == 0
        || account.debt_principal > account.credit_cap
        || account.pending_movements > 256
        || account.open_appeals > 16
        || account.interest_accrued_at_unix == 0
        || account.updated_at_unix == 0
    {
        return Err(corrupt_state(
            "stored reserve provider account is inconsistent",
        ));
    }
    Ok(Some(account))
}

fn ensure_apr_rotation_has_no_debt(
    world: &impl WorldReadOnly,
    current: &ReserveAuthorityPolicyV1,
    next: &ReserveAuthorityPolicyV1,
) -> Result<(), InstructionExecutionError> {
    let apr_changed = [ReserveTier::TierA, ReserveTier::TierB, ReserveTier::TierC]
        .into_iter()
        .try_fold(false, |changed, tier| {
            let current_apr = current
                .economics
                .tier_configuration(tier)
                .map_err(|error| corrupt_state(format!("invalid active reserve tier: {error}")))?
                .interest_apr_bps;
            let next_apr = next
                .economics
                .tier_configuration(tier)
                .map_err(|error| invalid_parameter(format!("invalid next reserve tier: {error}")))?
                .interest_apr_bps;
            Ok::<_, InstructionExecutionError>(changed || current_apr != next_apr)
        })?;
    if !apr_changed {
        return Ok(());
    }

    let start = Name::from_str(PROVIDER_STATE_KEY_PREFIX).expect("static provider prefix is valid");
    for (key, payload) in world.smart_contract_state().range(start..) {
        if !key.to_string().starts_with(PROVIDER_STATE_KEY_PREFIX) {
            break;
        }
        let candidate: ReserveProviderAccountV1 =
            decode_state(payload, "reserve provider account")?;
        if provider_key(candidate.terms.provider_id) != *key {
            return Err(corrupt_state(
                "authoritative reserve provider key does not match its account",
            ));
        }
        let account = read_provider(world, candidate.terms.provider_id)?.ok_or_else(|| {
            corrupt_state("authoritative reserve provider disappeared during policy validation")
        })?;
        if !account
            .total_debt()
            .map_err(|error| corrupt_state(format!("invalid provider debt: {error}")))?
            .is_zero()
        {
            return Err(invalid_parameter(
                "reserve APR changes require all provider debt and accrued interest to be settled",
            ));
        }
    }
    Ok(())
}

fn provider_for_policy(
    world: &impl WorldReadOnly,
    provider_id: ProviderId,
    policy: &ReserveAuthorityPolicyRecordV1,
) -> Result<ReserveProviderAccountV1, InstructionExecutionError> {
    let mut account = read_provider(world, provider_id)?
        .ok_or_else(|| invalid_parameter(format!("reserve account for {provider_id} not found")))?;
    let governed_cap = credit_cap(&policy.policy, &account.terms)?;
    account.credit_cap = if account.debt_principal > governed_cap {
        account.debt_principal.clone()
    } else {
        governed_cap
    };
    if account.policy_digest != policy.policy_digest {
        account.policy_digest = policy.policy_digest;
    }
    Ok(account)
}

fn ensure_revision(
    account: &ReserveProviderAccountV1,
    expected: u64,
) -> Result<(), InstructionExecutionError> {
    if account.revision == expected {
        Ok(())
    } else {
        Err(invalid_parameter(format!(
            "reserve provider revision conflict: expected {expected}, current {}",
            account.revision
        )))
    }
}

fn advance_provider_revision(
    account: &mut ReserveProviderAccountV1,
    now: u64,
) -> Result<(), InstructionExecutionError> {
    account.revision = account
        .revision
        .checked_add(1)
        .ok_or_else(|| corrupt_state("reserve provider revision overflow"))?;
    account.updated_at_unix = now;
    Ok(())
}

fn accrue_interest(
    account: &mut ReserveProviderAccountV1,
    policy: &ReserveAuthorityPolicyV1,
    now: u64,
) -> Result<(), InstructionExecutionError> {
    let tier = policy
        .economics
        .tier_configuration(account.terms.tier)
        .map_err(|error| corrupt_state(format!("invalid active reserve tier: {error}")))?;
    account
        .accrue_interest(tier.interest_apr_bps, now)
        .map_err(|error| invalid_parameter(format!("reserve interest accrual failed: {error}")))?;
    Ok(())
}

fn read_movement(
    world: &impl WorldReadOnly,
    movement_id: [u8; 32],
) -> Result<Option<ReserveMovementRecordV1>, InstructionExecutionError> {
    let Some(bytes) = world.smart_contract_state().get(&movement_key(movement_id)) else {
        return Ok(None);
    };
    let record: ReserveMovementRecordV1 = decode_state(bytes, "reserve movement")?;
    let terminal_fields = record.decided_by.is_some()
        && record.decided_at_unix.is_some()
        && record.rationale.is_some();
    if record.movement_id != movement_id
        || record.movement_id == [0; 32]
        || record.amount.is_zero()
        || record.expected_provider_revision == 0
        || record.policy_digest == [0; 32]
        || record.requested_at_unix == 0
        || match record.status {
            ReserveMovementStatusV1::Pending => {
                record.decided_by.is_some()
                    || record.decided_at_unix.is_some()
                    || record.rationale.is_some()
            }
            ReserveMovementStatusV1::Approved | ReserveMovementStatusV1::Rejected => {
                !terminal_fields
            }
        }
    {
        return Err(corrupt_state("stored reserve movement is inconsistent"));
    }
    Ok(Some(record))
}

fn read_appeal(
    world: &impl WorldReadOnly,
    appeal_id: [u8; 32],
) -> Result<Option<ReserveAppealRecordV1>, InstructionExecutionError> {
    let Some(bytes) = world.smart_contract_state().get(&appeal_key(appeal_id)) else {
        return Ok(None);
    };
    let record: ReserveAppealRecordV1 = decode_state(bytes, "reserve appeal")?;
    let terminal_fields = record.decided_by.is_some()
        && record.decided_at_unix.is_some()
        && record.rationale.is_some();
    if record.appeal_id != appeal_id
        || record.appeal_id == [0; 32]
        || record.reason.is_empty()
        || record.reason.len() > RESERVE_MAX_REASON_BYTES_V1
        || record.expected_provider_revision == 0
        || record.submitted_at_unix == 0
        || match record.status {
            ReserveAppealStatusV1::Pending => {
                record.decided_by.is_some()
                    || record.decided_at_unix.is_some()
                    || record.rationale.is_some()
            }
            ReserveAppealStatusV1::Accepted | ReserveAppealStatusV1::Rejected => !terminal_fields,
        }
    {
        return Err(corrupt_state("stored reserve appeal is inconsistent"));
    }
    Ok(Some(record))
}

fn transfer(
    state_transaction: &mut StateTransaction<'_, '_>,
    policy: &ReserveAuthorityPolicyV1,
    source: &AccountId,
    destination: &AccountId,
    amount: &XorQuantity,
) -> Result<(), InstructionExecutionError> {
    super::asset::isi::execute_user_numeric_asset_transfer(
        state_transaction,
        source,
        AssetId::of(policy.asset_definition.clone(), source.clone()),
        destination.clone(),
        amount.clone().into_quantity().into_numeric(),
    )
    .map_err(|error| invalid_parameter(format!("reserve custody transfer failed: {error}")))
}

impl Execute for SetSorafsReservePolicy {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        require_governance(state_transaction, authority)?;
        self.policy
            .validate()
            .map_err(|error| invalid_parameter(format!("invalid reserve policy: {error}")))?;
        state_transaction
            .world
            .account(&self.policy.custody_account)?;
        state_transaction
            .world
            .account(&self.policy.treasury_account)?;
        state_transaction
            .world
            .asset_definition(&self.policy.asset_definition)?;
        let now = now_unix(state_transaction)?;
        let digest = self.policy.digest().map_err(|error| {
            invalid_parameter(format!("failed to digest reserve policy: {error}"))
        })?;
        match read_policy(state_transaction.world())? {
            None => {
                if self.policy.revision != 1 || self.policy.predecessor_policy_digest.is_some() {
                    return Err(invalid_parameter(
                        "first reserve policy must be revision one without a predecessor",
                    ));
                }
            }
            Some(current) => {
                let expected = current
                    .policy
                    .revision
                    .checked_add(1)
                    .ok_or_else(|| corrupt_state("reserve policy revision overflow"))?;
                if self.policy.revision != expected
                    || self.policy.predecessor_policy_digest != Some(current.policy_digest)
                {
                    return Err(invalid_parameter(
                        "reserve policy must exactly extend the active revision and digest",
                    ));
                }
                ensure_apr_rotation_has_no_debt(
                    state_transaction.world(),
                    &current.policy,
                    &self.policy,
                )?;
                if self.policy.asset_definition != current.policy.asset_definition
                    || self.policy.custody_account != current.policy.custody_account
                    || self.policy.treasury_account != current.policy.treasury_account
                {
                    return Err(invalid_parameter(
                        "reserve asset and custody accounts are immutable after activation",
                    ));
                }
            }
        }
        let record = ReserveAuthorityPolicyRecordV1 {
            policy: self.policy,
            policy_digest: digest,
            activated_by: authority.clone(),
            activated_at_unix: now,
        };
        let encoded = encode_state(&record, "reserve policy")?;
        state_transaction
            .world
            .smart_contract_state
            .insert(policy_key().clone(), encoded);
        emit_reserve_event(
            state_transaction,
            SorafsReserveLedgerEventKind::PolicyActivated,
            None,
            None,
            digest,
            0,
            authority,
            now,
        );
        Ok(())
    }
}

impl Execute for RegisterSorafsReserveAccount {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        require_governance(state_transaction, authority)?;
        let (policy, now) = active_policy(state_transaction, self.policy_digest)?;
        if self.terms.capacity_gib == 0 {
            return Err(invalid_parameter(
                "reserve account capacity must be non-zero",
            ));
        }
        let owner = registered_provider_owner(state_transaction.world(), self.terms.provider_id)?;
        if owner.subject_id() != self.terms.provider_account.subject_id() {
            return Err(invalid_parameter(
                "reserve account provider does not match the registry owner",
            ));
        }
        if read_provider(state_transaction.world(), self.terms.provider_id)?.is_some() {
            return Err(invalid_parameter(
                "reserve provider account is already registered",
            ));
        }
        let account = ReserveProviderAccountV1 {
            credit_cap: credit_cap(&policy.policy, &self.terms)?,
            terms: self.terms,
            policy_digest: policy.policy_digest,
            revision: 1,
            reserve_balance: XorQuantity::zero(),
            debt_principal: XorQuantity::zero(),
            accrued_interest: XorQuantity::zero(),
            lifecycle_stage: ReserveLifecycleStage::Warning,
            days_past_due: 0,
            pending_movements: 0,
            open_appeals: 0,
            interest_accrued_at_unix: now,
            updated_at_unix: now,
        };
        let encoded = encode_state(&account, "reserve provider account")?;
        state_transaction
            .world
            .smart_contract_state
            .insert(provider_key(account.terms.provider_id), encoded);
        emit_reserve_event(
            state_transaction,
            SorafsReserveLedgerEventKind::ProviderRegistered,
            Some(account.terms.provider_id),
            None,
            account.policy_digest,
            account.revision,
            authority,
            now,
        );
        Ok(())
    }
}

impl Execute for RequestSorafsReserveMovement {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        let (policy, now) = active_policy(state_transaction, self.policy_digest)?;
        if self.movement_id == [0; 32] || self.amount.is_zero() {
            return Err(invalid_parameter(
                "reserve movement id and amount must be non-zero",
            ));
        }
        if read_movement(state_transaction.world(), self.movement_id)?.is_some() {
            return Err(invalid_parameter("reserve movement id is already recorded"));
        }
        let mut account =
            provider_for_policy(state_transaction.world(), self.provider_id, &policy)?;
        if account.terms.provider_account.subject_id() != authority.subject_id() {
            return Err(invalid_parameter(
                "reserve movement authority is not the provider account",
            ));
        }
        ensure_revision(&account, self.expected_provider_revision)?;
        if account.pending_movements >= policy.policy.max_pending_movements_per_provider {
            return Err(invalid_parameter(
                "reserve provider reached the pending-movement ceiling",
            ));
        }
        account.pending_movements = account
            .pending_movements
            .checked_add(1)
            .ok_or_else(|| corrupt_state("reserve pending-movement counter overflow"))?;
        advance_provider_revision(&mut account, now)?;
        let record = ReserveMovementRecordV1 {
            movement_id: self.movement_id,
            provider_id: self.provider_id,
            kind: self.kind,
            amount: self.amount,
            requested_by: authority.clone(),
            expected_provider_revision: self.expected_provider_revision,
            policy_digest: policy.policy_digest,
            status: ReserveMovementStatusV1::Pending,
            requested_at_unix: now,
            decided_by: None,
            decided_at_unix: None,
            rationale: None,
        };
        let encoded_account = encode_state(&account, "reserve provider account")?;
        let encoded_record = encode_state(&record, "reserve movement")?;
        state_transaction
            .world
            .smart_contract_state
            .insert(provider_key(self.provider_id), encoded_account);
        state_transaction
            .world
            .smart_contract_state
            .insert(movement_key(self.movement_id), encoded_record);
        emit_reserve_event(
            state_transaction,
            SorafsReserveLedgerEventKind::MovementRequested,
            Some(self.provider_id),
            Some(self.movement_id),
            policy.policy_digest,
            account.revision,
            authority,
            now,
        );
        Ok(())
    }
}

impl Execute for DecideSorafsReserveMovement {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        require_governance(state_transaction, authority)?;
        if self.rationale.is_empty() || self.rationale.len() > RESERVE_MAX_REASON_BYTES_V1 {
            return Err(invalid_parameter(
                "reserve movement rationale is empty or oversized",
            ));
        }
        let mut movement = read_movement(state_transaction.world(), self.movement_id)?
            .ok_or_else(|| invalid_parameter("reserve movement not found"))?;
        if movement.status != ReserveMovementStatusV1::Pending {
            return Err(invalid_parameter("reserve movement is already decided"));
        }
        let policy = read_policy(state_transaction.world())?
            .ok_or_else(|| invalid_parameter("SoraFS reserve policy is not configured"))?;
        let now = now_unix(state_transaction)?;
        let mut account =
            provider_for_policy(state_transaction.world(), movement.provider_id, &policy)?;
        accrue_interest(&mut account, &policy.policy, now)?;
        account.pending_movements = account
            .pending_movements
            .checked_sub(1)
            .ok_or_else(|| corrupt_state("reserve pending-movement counter underflow"))?;

        if self.approve {
            match movement.kind {
                ReserveMovementKindV1::TopUp => {
                    account.reserve_balance = account
                        .reserve_balance
                        .checked_add(&movement.amount)
                        .map_err(|error| {
                            invalid_parameter(format!("reserve balance overflow: {error}"))
                        })?;
                }
                ReserveMovementKindV1::Withdrawal => {
                    if !account
                        .total_debt()
                        .map_err(|error| {
                            invalid_parameter(format!("invalid provider debt: {error}"))
                        })?
                        .is_zero()
                    {
                        return Err(invalid_parameter(
                            "reserve withdrawal is forbidden while provider debt is outstanding",
                        ));
                    }
                    let remaining = account
                        .reserve_balance
                        .checked_sub(&movement.amount)
                        .map_err(|_| {
                            invalid_parameter("reserve withdrawal exceeds provider partition")
                        })?;
                    let quote = policy
                        .policy
                        .economics
                        .quote(
                            account.terms.storage_class,
                            account.terms.capacity_gib,
                            account.terms.duration,
                            account.terms.tier,
                            remaining.clone(),
                        )
                        .map_err(|error| {
                            invalid_parameter(format!("reserve withdrawal quote failed: {error}"))
                        })?;
                    if !quote
                        .ledger_projection()
                        .map_err(|error| {
                            invalid_parameter(format!(
                                "reserve withdrawal projection failed: {error}"
                            ))
                        })?
                        .meets_underwriting
                    {
                        return Err(invalid_parameter(
                            "reserve withdrawal would breach underwriting",
                        ));
                    }
                    account.reserve_balance = remaining;
                }
            }
        }
        advance_provider_revision(&mut account, now)?;
        movement.status = if self.approve {
            ReserveMovementStatusV1::Approved
        } else {
            ReserveMovementStatusV1::Rejected
        };
        movement.decided_by = Some(authority.clone());
        movement.decided_at_unix = Some(now);
        movement.rationale = Some(self.rationale);
        let encoded_account = encode_state(&account, "reserve provider account")?;
        let encoded_movement = encode_state(&movement, "reserve movement")?;

        if self.approve {
            match movement.kind {
                ReserveMovementKindV1::TopUp => transfer(
                    state_transaction,
                    &policy.policy,
                    &account.terms.provider_account,
                    &policy.policy.custody_account,
                    &movement.amount,
                )?,
                ReserveMovementKindV1::Withdrawal => transfer(
                    state_transaction,
                    &policy.policy,
                    &policy.policy.custody_account,
                    &account.terms.provider_account,
                    &movement.amount,
                )?,
            }
        }
        state_transaction
            .world
            .smart_contract_state
            .insert(provider_key(movement.provider_id), encoded_account);
        state_transaction
            .world
            .smart_contract_state
            .insert(movement_key(movement.movement_id), encoded_movement);
        emit_reserve_event(
            state_transaction,
            if movement.status == ReserveMovementStatusV1::Approved {
                SorafsReserveLedgerEventKind::MovementApproved
            } else {
                SorafsReserveLedgerEventKind::MovementRejected
            },
            Some(movement.provider_id),
            Some(movement.movement_id),
            policy.policy_digest,
            account.revision,
            authority,
            now,
        );
        Ok(())
    }
}

impl Execute for ChargeSorafsReserveRent {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        require_governance(state_transaction, authority)?;
        if !(1..=12).contains(&self.billing_periods) {
            return Err(invalid_parameter(
                "reserve rent billing periods must be in 1..=12",
            ));
        }
        let (policy, now) = active_policy(state_transaction, self.policy_digest)?;
        let mut account =
            provider_for_policy(state_transaction.world(), self.provider_id, &policy)?;
        ensure_revision(&account, self.expected_provider_revision)?;
        accrue_interest(&mut account, &policy.policy, now)?;
        let quote = policy
            .policy
            .economics
            .quote(
                account.terms.storage_class,
                account.terms.capacity_gib,
                account.terms.duration,
                account.terms.tier,
                account.reserve_balance.clone(),
            )
            .map_err(|error| invalid_parameter(format!("reserve rent quote failed: {error}")))?;
        let rent = quote
            .effective_rent
            .checked_mul_u64(u64::from(self.billing_periods))
            .map_err(|error| invalid_parameter(format!("reserve rent overflow: {error}")))?;
        if rent.is_zero() {
            return Err(invalid_parameter(
                "deterministic reserve rent charge is zero",
            ));
        }
        account.days_past_due = 0;
        account.lifecycle_stage = if quote
            .ledger_projection()
            .map_err(|error| invalid_parameter(format!("reserve projection failed: {error}")))?
            .needs_top_up_alert
        {
            ReserveLifecycleStage::Warning
        } else {
            ReserveLifecycleStage::Active
        };
        advance_provider_revision(&mut account, now)?;
        let encoded = encode_state(&account, "reserve provider account")?;
        transfer(
            state_transaction,
            &policy.policy,
            &account.terms.provider_account,
            &policy.policy.treasury_account,
            &rent,
        )?;
        state_transaction
            .world
            .smart_contract_state
            .insert(provider_key(self.provider_id), encoded);
        emit_reserve_event(
            state_transaction,
            SorafsReserveLedgerEventKind::RentCharged,
            Some(self.provider_id),
            None,
            policy.policy_digest,
            account.revision,
            authority,
            now,
        );
        Ok(())
    }
}

impl Execute for AdvanceSorafsReserveLifecycle {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        require_governance(state_transaction, authority)?;
        let (policy, now) = active_policy(state_transaction, self.policy_digest)?;
        let mut account =
            provider_for_policy(state_transaction.world(), self.provider_id, &policy)?;
        ensure_revision(&account, self.expected_provider_revision)?;
        accrue_interest(&mut account, &policy.policy, now)?;
        let quote = policy
            .policy
            .economics
            .quote(
                account.terms.storage_class,
                account.terms.capacity_gib,
                account.terms.duration,
                account.terms.tier,
                account.reserve_balance.clone(),
            )
            .map_err(|error| {
                invalid_parameter(format!("reserve lifecycle quote failed: {error}"))
            })?;
        let projection = quote
            .lifecycle_projection(
                self.days_past_due,
                policy.policy.grace_period_days,
                policy.policy.default_after_days,
            )
            .map_err(|error| {
                invalid_parameter(format!("reserve lifecycle projection failed: {error}"))
            })?;
        account.lifecycle_stage = projection.stage;
        account.days_past_due = self.days_past_due;
        advance_provider_revision(&mut account, now)?;
        let encoded = encode_state(&account, "reserve provider account")?;
        state_transaction
            .world
            .smart_contract_state
            .insert(provider_key(self.provider_id), encoded);
        emit_reserve_event(
            state_transaction,
            SorafsReserveLedgerEventKind::LifecycleAdvanced,
            Some(self.provider_id),
            None,
            policy.policy_digest,
            account.revision,
            authority,
            now,
        );
        Ok(())
    }
}

impl Execute for DrawSorafsReserveCredit {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        require_governance(state_transaction, authority)?;
        if self.amount.is_zero() {
            return Err(invalid_parameter("reserve credit draw must be non-zero"));
        }
        let (policy, now) = active_policy(state_transaction, self.policy_digest)?;
        let mut account =
            provider_for_policy(state_transaction.world(), self.provider_id, &policy)?;
        ensure_revision(&account, self.expected_provider_revision)?;
        accrue_interest(&mut account, &policy.policy, now)?;
        if self.amount
            > account
                .available_credit()
                .map_err(|error| invalid_parameter(format!("invalid provider credit: {error}")))?
        {
            return Err(invalid_parameter(
                "reserve credit draw exceeds the provider credit cap",
            ));
        }
        account.debt_principal = account
            .debt_principal
            .checked_add(&self.amount)
            .map_err(|error| invalid_parameter(format!("reserve debt overflow: {error}")))?;
        account.reserve_balance = account
            .reserve_balance
            .checked_add(&self.amount)
            .map_err(|error| invalid_parameter(format!("reserve balance overflow: {error}")))?;
        advance_provider_revision(&mut account, now)?;
        let encoded = encode_state(&account, "reserve provider account")?;
        transfer(
            state_transaction,
            &policy.policy,
            &policy.policy.treasury_account,
            &policy.policy.custody_account,
            &self.amount,
        )?;
        state_transaction
            .world
            .smart_contract_state
            .insert(provider_key(self.provider_id), encoded);
        emit_reserve_event(
            state_transaction,
            SorafsReserveLedgerEventKind::CreditDrawn,
            Some(self.provider_id),
            None,
            policy.policy_digest,
            account.revision,
            authority,
            now,
        );
        Ok(())
    }
}

impl Execute for RepaySorafsReserveCredit {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        if self.amount.is_zero() {
            return Err(invalid_parameter(
                "reserve credit repayment must be non-zero",
            ));
        }
        let (policy, now) = active_policy(state_transaction, self.policy_digest)?;
        let mut account =
            provider_for_policy(state_transaction.world(), self.provider_id, &policy)?;
        if account.terms.provider_account.subject_id() != authority.subject_id() {
            return Err(invalid_parameter(
                "reserve credit repayment authority is not the provider account",
            ));
        }
        ensure_revision(&account, self.expected_provider_revision)?;
        accrue_interest(&mut account, &policy.policy, now)?;
        let total_debt = account
            .total_debt()
            .map_err(|error| invalid_parameter(format!("invalid reserve debt: {error}")))?;
        if self.amount > total_debt {
            return Err(invalid_parameter(
                "reserve credit repayment exceeds total debt",
            ));
        }
        let interest_payment = XorQuantity::min(&self.amount, &account.accrued_interest);
        account.accrued_interest = account
            .accrued_interest
            .checked_sub(&interest_payment)
            .map_err(|error| corrupt_state(format!("reserve interest underflow: {error}")))?;
        let principal_payment = self
            .amount
            .checked_sub(&interest_payment)
            .map_err(|error| corrupt_state(format!("reserve repayment underflow: {error}")))?;
        account.debt_principal = account
            .debt_principal
            .checked_sub(&principal_payment)
            .map_err(|error| corrupt_state(format!("reserve principal underflow: {error}")))?;
        let governed_cap = credit_cap(&policy.policy, &account.terms)?;
        account.credit_cap = if account.debt_principal > governed_cap {
            account.debt_principal.clone()
        } else {
            governed_cap
        };
        advance_provider_revision(&mut account, now)?;
        let encoded = encode_state(&account, "reserve provider account")?;
        transfer(
            state_transaction,
            &policy.policy,
            &account.terms.provider_account,
            &policy.policy.treasury_account,
            &self.amount,
        )?;
        state_transaction
            .world
            .smart_contract_state
            .insert(provider_key(self.provider_id), encoded);
        emit_reserve_event(
            state_transaction,
            SorafsReserveLedgerEventKind::CreditRepaid,
            Some(self.provider_id),
            None,
            policy.policy_digest,
            account.revision,
            authority,
            now,
        );
        Ok(())
    }
}

impl Execute for SubmitSorafsReserveAppeal {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        let (policy, now) = active_policy(state_transaction, self.policy_digest)?;
        if self.appeal_id == [0; 32]
            || self.reason.is_empty()
            || self.reason.len() > RESERVE_MAX_REASON_BYTES_V1
            || self.evidence_digest == Some([0; 32])
        {
            return Err(invalid_parameter(
                "reserve appeal id, reason, or evidence digest is invalid",
            ));
        }
        if read_appeal(state_transaction.world(), self.appeal_id)?.is_some() {
            return Err(invalid_parameter("reserve appeal id is already recorded"));
        }
        let mut account =
            provider_for_policy(state_transaction.world(), self.provider_id, &policy)?;
        if account.terms.provider_account.subject_id() != authority.subject_id() {
            return Err(invalid_parameter(
                "reserve appeal authority is not the provider account",
            ));
        }
        ensure_revision(&account, self.expected_provider_revision)?;
        if account.open_appeals >= policy.policy.max_open_appeals_per_provider {
            return Err(invalid_parameter(
                "reserve provider reached the open-appeal ceiling",
            ));
        }
        account.open_appeals = account
            .open_appeals
            .checked_add(1)
            .ok_or_else(|| corrupt_state("reserve open-appeal counter overflow"))?;
        advance_provider_revision(&mut account, now)?;
        let appeal = ReserveAppealRecordV1 {
            appeal_id: self.appeal_id,
            provider_id: self.provider_id,
            submitted_by: authority.clone(),
            requested_stage: self.requested_stage,
            reason: self.reason,
            evidence_digest: self.evidence_digest,
            expected_provider_revision: self.expected_provider_revision,
            status: ReserveAppealStatusV1::Pending,
            submitted_at_unix: now,
            decided_by: None,
            decided_at_unix: None,
            rationale: None,
        };
        let encoded_account = encode_state(&account, "reserve provider account")?;
        let encoded_appeal = encode_state(&appeal, "reserve appeal")?;
        state_transaction
            .world
            .smart_contract_state
            .insert(provider_key(self.provider_id), encoded_account);
        state_transaction
            .world
            .smart_contract_state
            .insert(appeal_key(self.appeal_id), encoded_appeal);
        emit_reserve_event(
            state_transaction,
            SorafsReserveLedgerEventKind::AppealSubmitted,
            Some(self.provider_id),
            Some(self.appeal_id),
            policy.policy_digest,
            account.revision,
            authority,
            now,
        );
        Ok(())
    }
}

impl Execute for DecideSorafsReserveAppeal {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        require_governance(state_transaction, authority)?;
        if self.rationale.is_empty() || self.rationale.len() > RESERVE_MAX_REASON_BYTES_V1 {
            return Err(invalid_parameter(
                "reserve appeal rationale is empty or oversized",
            ));
        }
        let mut appeal = read_appeal(state_transaction.world(), self.appeal_id)?
            .ok_or_else(|| invalid_parameter("reserve appeal not found"))?;
        if appeal.status != ReserveAppealStatusV1::Pending {
            return Err(invalid_parameter("reserve appeal is already decided"));
        }
        let policy = read_policy(state_transaction.world())?
            .ok_or_else(|| invalid_parameter("SoraFS reserve policy is not configured"))?;
        let now = now_unix(state_transaction)?;
        let mut account =
            provider_for_policy(state_transaction.world(), appeal.provider_id, &policy)?;
        account.open_appeals = account
            .open_appeals
            .checked_sub(1)
            .ok_or_else(|| corrupt_state("reserve open-appeal counter underflow"))?;
        if self.accept {
            account.lifecycle_stage = appeal.requested_stage;
        }
        advance_provider_revision(&mut account, now)?;
        appeal.status = if self.accept {
            ReserveAppealStatusV1::Accepted
        } else {
            ReserveAppealStatusV1::Rejected
        };
        appeal.decided_by = Some(authority.clone());
        appeal.decided_at_unix = Some(now);
        appeal.rationale = Some(self.rationale);
        let encoded_account = encode_state(&account, "reserve provider account")?;
        let encoded_appeal = encode_state(&appeal, "reserve appeal")?;
        state_transaction
            .world
            .smart_contract_state
            .insert(provider_key(appeal.provider_id), encoded_account);
        state_transaction
            .world
            .smart_contract_state
            .insert(appeal_key(appeal.appeal_id), encoded_appeal);
        emit_reserve_event(
            state_transaction,
            if appeal.status == ReserveAppealStatusV1::Accepted {
                SorafsReserveLedgerEventKind::AppealAccepted
            } else {
                SorafsReserveLedgerEventKind::AppealRejected
            },
            Some(appeal.provider_id),
            Some(appeal.appeal_id),
            policy.policy_digest,
            account.revision,
            authority,
            now,
        );
        Ok(())
    }
}

fn query_failure(error: impl core::fmt::Display) -> QueryExecutionFail {
    QueryExecutionFail::Conversion(error.to_string())
}

impl ValidSingularQuery for FindSorafsReservePolicy {
    fn execute(
        &self,
        state_ro: &impl crate::state::StateReadOnly,
    ) -> Result<ReserveAuthorityPolicyRecordV1, QueryExecutionFail> {
        read_policy(state_ro.world())
            .map_err(query_failure)?
            .ok_or_else(|| QueryExecutionFail::Find(FindError::SorafsReservePolicy))
    }
}

impl ValidSingularQuery for FindSorafsReserveProviderById {
    fn execute(
        &self,
        state_ro: &impl crate::state::StateReadOnly,
    ) -> Result<ReserveProviderAccountV1, QueryExecutionFail> {
        read_provider(state_ro.world(), self.provider_id)
            .map_err(query_failure)?
            .ok_or_else(|| {
                QueryExecutionFail::Find(FindError::SorafsReserveProvider(self.provider_id))
            })
    }
}

impl ValidSingularQuery for FindSorafsReserveMovementById {
    fn execute(
        &self,
        state_ro: &impl crate::state::StateReadOnly,
    ) -> Result<ReserveMovementRecordV1, QueryExecutionFail> {
        read_movement(state_ro.world(), self.movement_id)
            .map_err(query_failure)?
            .ok_or_else(|| {
                QueryExecutionFail::Find(FindError::SorafsReserveMovement(self.movement_id))
            })
    }
}

impl ValidSingularQuery for FindSorafsReserveAppealById {
    fn execute(
        &self,
        state_ro: &impl crate::state::StateReadOnly,
    ) -> Result<ReserveAppealRecordV1, QueryExecutionFail> {
        read_appeal(state_ro.world(), self.appeal_id)
            .map_err(query_failure)?
            .ok_or_else(|| QueryExecutionFail::Find(FindError::SorafsReserveAppeal(self.appeal_id)))
    }
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, Hash, KeyPair, PrivateKey};
    use iroha_data_model::{
        account::{Account, AccountId},
        asset::{Asset, AssetDefinition, AssetDefinitionId, AssetId},
        block::BlockHeader,
        domain::{Domain, DomainId},
        permission::{Permission, Permissions},
        sorafs::{
            pin_registry::StorageClass,
            reserve::{
                RESERVE_AUTHORITY_POLICY_VERSION_V1, ReserveDuration, ReservePolicyV1,
                ReserveProviderTermsV1, ReserveTier,
            },
        },
    };
    use iroha_primitives::{
        json::Json,
        numeric::{Numeric, Quantity},
    };
    use nonzero_ext::nonzero;

    use super::*;
    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World},
    };

    const NOW: u64 = 20_000;
    const PROVIDER_ID: ProviderId = ProviderId::new([0x61; 32]);

    fn keypair(seed: u8) -> KeyPair {
        let private = PrivateKey::from_bytes(Algorithm::Ed25519, &[seed; 32])
            .expect("valid deterministic Ed25519 seed");
        KeyPair::from_private_key(private).expect("derive deterministic keypair")
    }

    fn account(keypair: &KeyPair) -> AccountId {
        AccountId::new(keypair.public_key().clone())
    }

    fn asset_definition() -> AssetDefinitionId {
        AssetDefinitionId::new(
            DomainId::try_new("reserve", "universal").expect("reserve domain"),
            "xor".parse().expect("reserve asset"),
        )
    }

    fn quantity_micro(micro: u128) -> Quantity {
        Quantity::try_from_numeric(Numeric::new(micro, 6)).expect("micro-XOR fixture")
    }

    fn xor_micro(micro: u128) -> XorQuantity {
        XorQuantity::try_from_micro(micro).expect("micro-XOR reserve fixture")
    }

    fn policy(
        revision: u64,
        predecessor_policy_digest: Option<[u8; 32]>,
        custody_account: AccountId,
        treasury_account: AccountId,
    ) -> ReserveAuthorityPolicyV1 {
        ReserveAuthorityPolicyV1 {
            version: RESERVE_AUTHORITY_POLICY_VERSION_V1,
            revision,
            predecessor_policy_digest,
            economics: ReservePolicyV1::default(),
            asset_definition: asset_definition(),
            custody_account,
            treasury_account,
            grace_period_days: 7,
            default_after_days: 30,
            max_provider_debt: xor_micro(1_000_000_000),
            max_pending_movements_per_provider: 4,
            max_open_appeals_per_provider: 2,
        }
    }

    fn state_fixture(
        governance: &AccountId,
        provider: &AccountId,
        custody: &AccountId,
        treasury: &AccountId,
    ) -> State {
        let definition_id = asset_definition();
        let domain = Domain::new(definition_id.domain().clone()).build(governance);
        let definition = AssetDefinition::numeric(definition_id.clone())
            .with_name("XOR".to_owned())
            .build(governance);
        let provider_asset = Asset::new(
            AssetId::of(definition_id.clone(), provider.clone()),
            quantity_micro(100_000_000),
        );
        let treasury_asset = Asset::new(
            AssetId::of(definition_id, treasury.clone()),
            quantity_micro(100_000_000),
        );
        let mut world = World::with_assets(
            [domain],
            [
                Account::new(governance.clone()).build(governance),
                Account::new(provider.clone()).build(provider),
                Account::new(custody.clone()).build(custody),
                Account::new(treasury.clone()).build(treasury),
            ],
            [definition],
            [provider_asset, treasury_asset],
            [],
        );
        let mut permissions = Permissions::new();
        permissions.insert(Permission::new(
            "CanSetSorafsReservePolicy".to_owned(),
            Json::new(()),
        ));
        world
            .account_permissions
            .insert(governance.clone(), permissions);
        world.provider_owners.insert(PROVIDER_ID, provider.clone());
        State::new_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        )
    }

    fn terms(provider_account: AccountId) -> ReserveProviderTermsV1 {
        ReserveProviderTermsV1 {
            provider_id: PROVIDER_ID,
            provider_account,
            tier: ReserveTier::TierA,
            storage_class: StorageClass::Hot,
            duration: ReserveDuration::Monthly,
            capacity_gib: 10,
        }
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn pending_operations_survive_concurrency_and_policy_rotation() {
        let governance = account(&keypair(0x71));
        let provider = account(&keypair(0x72));
        let custody = account(&keypair(0x73));
        let treasury = account(&keypair(0x74));
        let state = state_fixture(&governance, &provider, &custody, &treasury);
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, NOW * 1_000, 0);
        let mut block = state.block(header);
        let mut stx = block.transaction();
        stx.tx_call_hash = Some(Hash::prehashed([0x91; Hash::LENGTH]));

        let first = policy(1, None, custody.clone(), treasury.clone());
        let first_digest = first.digest().expect("first policy digest");
        assert!(
            SetSorafsReservePolicy::new(first.clone())
                .execute(&provider, &mut stx)
                .is_err(),
            "provider cannot activate reserve governance policy"
        );
        SetSorafsReservePolicy::new(first)
            .execute(&governance, &mut stx)
            .expect("activate first policy");
        RegisterSorafsReserveAccount::new(terms(provider.clone()), first_digest)
            .execute(&governance, &mut stx)
            .expect("register reserve account");
        stx.world.provider_owners.remove(PROVIDER_ID);
        assert!(
            read_provider(stx.world(), PROVIDER_ID)
                .expect("read provider after registry withdrawal")
                .is_some(),
            "registered reserve state remains authoritative after provider registry withdrawal"
        );

        for (id, revision) in [(0x81, 1), (0x82, 2)] {
            RequestSorafsReserveMovement::new(
                [id; 32],
                PROVIDER_ID,
                ReserveMovementKindV1::TopUp,
                xor_micro(10_000_000),
                revision,
                first_digest,
            )
            .execute(&provider, &mut stx)
            .expect("request concurrent top-up");
        }
        let pending = read_provider(stx.world(), PROVIDER_ID)
            .expect("read provider")
            .expect("provider");
        assert_eq!((pending.revision, pending.pending_movements), (3, 2));
        assert!(
            RequestSorafsReserveMovement::new(
                [0x83; 32],
                PROVIDER_ID,
                ReserveMovementKindV1::TopUp,
                xor_micro(1_000_000),
                1,
                first_digest,
            )
            .execute(&provider, &mut stx)
            .is_err(),
            "stale provider revision must fail closed"
        );
        assert_eq!(
            read_provider(stx.world(), PROVIDER_ID)
                .expect("read provider")
                .expect("provider"),
            pending
        );
        for id in [0x81, 0x82] {
            DecideSorafsReserveMovement::new([id; 32], true, "approved".to_owned())
                .execute(&governance, &mut stx)
                .expect("decide concurrent top-up");
        }

        RequestSorafsReserveMovement::new(
            [0x84; 32],
            PROVIDER_ID,
            ReserveMovementKindV1::TopUp,
            xor_micro(10_000_000),
            5,
            first_digest,
        )
        .execute(&provider, &mut stx)
        .expect("request before policy rotation");
        let second = policy(2, Some(first_digest), custody.clone(), treasury.clone());
        let second_digest = second.digest().expect("second policy digest");
        SetSorafsReservePolicy::new(second)
            .execute(&governance, &mut stx)
            .expect("rotate reserve policy");
        DecideSorafsReserveMovement::new([0x84; 32], true, "approved after rotation".to_owned())
            .execute(&governance, &mut stx)
            .expect("pending movement remains decidable after rotation");

        for (id, revision) in [(0x91_u8, 7), (0x92, 8)] {
            SubmitSorafsReserveAppeal::new(
                [id; 32],
                PROVIDER_ID,
                revision,
                ReserveLifecycleStage::Warning,
                "review lifecycle evidence".to_owned(),
                Some([id.wrapping_add(1); 32]),
                second_digest,
            )
            .execute(&provider, &mut stx)
            .expect("submit concurrent appeal");
        }
        for id in [0x91, 0x92] {
            DecideSorafsReserveAppeal::new([id; 32], false, "not substantiated".to_owned())
                .execute(&governance, &mut stx)
                .expect("decide concurrent appeal");
        }

        let before_cap_reduction = FindSorafsReserveProviderById::new(PROVIDER_ID)
            .execute(&stx)
            .expect("query provider");
        assert_eq!(before_cap_reduction.revision, 11);
        assert_eq!(before_cap_reduction.policy_digest, second_digest);
        assert_eq!(before_cap_reduction.reserve_balance, xor_micro(30_000_000));
        assert_eq!(before_cap_reduction.open_appeals, 0);
        assert_eq!(
            FindSorafsReserveAppealById::new([0x91; 32])
                .execute(&stx)
                .expect("query appeal")
                .status,
            ReserveAppealStatusV1::Rejected
        );

        DrawSorafsReserveCredit::new(PROVIDER_ID, 11, xor_micro(10_000_000), second_digest)
            .execute(&governance, &mut stx)
            .expect("draw credit before cap reduction");
        let mut unsafe_apr_change =
            policy(3, Some(second_digest), custody.clone(), treasury.clone());
        unsafe_apr_change
            .economics
            .tiers
            .iter_mut()
            .find(|tier| tier.tier == ReserveTier::TierA)
            .expect("tier A fixture")
            .interest_apr_bps += 1;
        assert!(
            SetSorafsReservePolicy::new(unsafe_apr_change)
                .execute(&governance, &mut stx)
                .is_err(),
            "APR rotation with outstanding debt must fail rather than reprice it retroactively"
        );
        assert_eq!(
            read_policy(stx.world())
                .expect("read policy")
                .expect("active policy")
                .policy_digest,
            second_digest
        );
        let mut third = policy(3, Some(second_digest), custody.clone(), treasury.clone());
        third.max_provider_debt = xor_micro(1_000_000);
        let third_digest = third.digest().expect("third policy digest");
        SetSorafsReservePolicy::new(third)
            .execute(&governance, &mut stx)
            .expect("reduce credit cap below grandfathered principal");
        RepaySorafsReserveCredit::new(PROVIDER_ID, 12, xor_micro(10_000_000), third_digest)
            .execute(&provider, &mut stx)
            .expect("cap reduction must not brick repayment");
        let final_account = FindSorafsReserveProviderById::new(PROVIDER_ID)
            .execute(&stx)
            .expect("query final provider");
        assert_eq!(final_account.revision, 13);
        assert_eq!(final_account.policy_digest, third_digest);
        assert_eq!(final_account.reserve_balance, xor_micro(40_000_000));
        assert_eq!(final_account.debt_principal, XorQuantity::zero());
        assert_eq!(final_account.credit_cap, xor_micro(1_000_000));

        let provider_balance = stx
            .world
            .assets
            .get(&AssetId::of(asset_definition(), provider))
            .expect("provider asset")
            .as_ref()
            .clone();
        let custody_balance = stx
            .world
            .assets
            .get(&AssetId::of(asset_definition(), custody))
            .expect("custody asset")
            .as_ref()
            .clone();
        assert_eq!(provider_balance, quantity_micro(60_000_000));
        assert_eq!(custody_balance, quantity_micro(40_000_000));
    }
}
