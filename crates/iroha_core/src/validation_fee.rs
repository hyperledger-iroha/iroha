//! Validator-side enforcement for chain-level validation-fee policy.
use crate::{
    smartcontracts::isi::triggers::{
        set::{ExecutableRef, SetReadOnly as _},
        specialized::LoadedActionTrait as _,
        trigger_is_enabled,
    },
    state::{StateReadOnly, StateTransaction, WorldReadOnly},
    tx::TransactionRejectionReason,
};
use core::fmt;
use hex;
use iroha_crypto::Hash;
use iroha_data_model::{
    ValidationFail,
    account::AccountId,
    asset::AssetDefinitionId,
    governance::types::ProposalKind,
    hijiri::{HijiriAccountRiskV1, HijiriParametersV1, Q16},
    isi::{
        InstructionBox, TransferAssetBatch, TransferBox,
        governance::{
            CreateParliamentGovernanceAttemptV1, ProposeValidationFeePayoutLifecycle,
            ProposeValidationFeePolicy, SubmitParliamentLifecycleTransitionV1,
        },
        register::RegisterBox,
        repo::{RepoInstructionBox, RepoIsi, ReverseRepoIsi},
        settlement::{
            DvpIsi, FundFxCorridorEscrow, PvpIsi, RefundFxCorridorEscrow, SetFxCorridorPolicy,
            SettleFxCorridor, SettlementInstructionBox,
        },
    },
    metadata::Metadata,
    prelude::*,
    transaction::{Executable, ExecutableBatchItem, SignedTransaction},
    validation_fee::{
        VALIDATION_FEE_HIJIRI_FEE_QUOTE_HASH_METADATA_KEY,
        VALIDATION_FEE_INSTRUCTION_INDEX_METADATA_KEY, VALIDATION_FEE_POLICY_HASH_METADATA_KEY,
        VALIDATION_FEE_POLICY_VERSION_METADATA_KEY,
        VALIDATION_FEE_TRANSFER_ENTRY_INDEX_METADATA_KEY, ValidationFeeChargingMode,
        ValidationFeeMultisigMarkerV1, ValidationFeeParliamentAuthorizationV1,
        ValidationFeePolicyRegistryEntryV1, ValidationFeePolicyRegistryV1, ValidationFeePolicyV1,
        ValidationFeeTreasuryPayoutBindingV1,
    },
};
use iroha_executor_data_model::isi::multisig::MultisigInstructionBox;
use iroha_primitives::numeric::{Numeric, NumericSpec, Quantity};
use ivm::state_value::{
    MAX_STATE_VALUE_RECORD_BYTES, StateValueAtomV1, StateValueKindV1, StateValueNodeV1,
    StateValueRecordV1, StateValueSchemaV1, state_value_schema_hash_v1,
};
use mv::storage::StorageReadOnly;
use sha2::{Digest as _, Sha256};
const VALIDATION_FEE_TREASURY_PAYOUT_EXEMPTION_CLASS: &str = "TREASURY_PAYOUT";
/// Contract-visible projection of the active lifecycle's consensus-owned fee credit.
///
/// The authoritative balance is keyed by the immutable lifecycle seal. This fixed leaf remains a
/// read-only projection because the v1 payout contract declares it in its state interface.
pub(crate) const VALIDATION_FEE_CREDIT_STATE_LEAF: &str = "AvailableValidationFeeCredit";
pub(crate) const VALIDATION_FEE_CREDIT_ASSET_STATE_LEAF: &str =
    "AvailableValidationFeeAssetDefinitionId";
pub(crate) const VALIDATION_FEE_CREDIT_LIFECYCLE_SEAL_STATE_LEAF: &str =
    "AvailableValidationFeeLifecycleSeal";
const VALIDATION_FEE_CREDIT_LIFECYCLE_STATE_PREFIX: &str = "ValidationFeeCreditLifecycle";
pub(crate) const VALIDATION_FEE_PAYOUT_WRAPPER_ENTRYPOINT_PERMISSION: &str =
    "CanInvokeContractEntrypoint";
pub(crate) const VALIDATION_FEE_POOL_SWAP_ENTRYPOINT: &str = "swap_exact_in_quote_public";

fn retained_enacted_validation_fee_proposals<'a>(
    state_transaction: &'a StateTransaction<'_, '_>,
) -> impl Iterator<Item = ([u8; 32], &'a ProposalKind)> + 'a {
    let world = &state_transaction.world;
    world
        .validation_fee_proposal_index
        .iter()
        .filter_map(move |((_, proposal_id), ())| {
            let proposal = world.governance_proposals.get(proposal_id)?;
            (proposal.status == crate::state::GovernanceProposalStatus::Enacted)
                .then_some((*proposal_id, &proposal.kind))
        })
}

/// Return the retained enacted validation-fee proposal that pins an account.
///
/// The retained proposal kinds, rather than the currently active policy projection, are the
/// append-only authorization source. Consequently this guard also covers the mandatory activation
/// delay before the first enabled policy can make account unregistration fail closed at admission.
pub(crate) fn retained_enacted_validation_fee_account_reference(
    state_transaction: &StateTransaction<'_, '_>,
    account_id: &AccountId,
) -> Option<([u8; 32], &'static str)> {
    retained_enacted_validation_fee_proposals(state_transaction).find_map(
        |(proposal_id, proposal_kind)| {
            let payout_reference = |binding: &iroha_data_model::validation_fee::ValidationFeeTreasuryPayoutBindingV1| {
                if &binding.treasury_account_id == account_id {
                    Some("payout treasury")
                } else if &binding.pool_vault_account_id == account_id {
                    Some("payout pool vault")
                } else if binding
                    .recipients
                    .iter()
                    .any(|recipient| &recipient.account_id == account_id)
                {
                    Some("payout recipient")
                } else {
                    None
                }
            };
            let reference_kind = match proposal_kind {
                ProposalKind::ValidationFeePolicy(payload) => {
                    if &payload.policy.treasury_account_id == account_id {
                        Some("policy treasury")
                    } else {
                        payload
                            .policy
                            .treasury_payout_binding
                            .as_ref()
                            .and_then(payout_reference)
                    }
                }
                ProposalKind::ValidationFeePayoutLifecycle(payload) => {
                    payout_reference(&payload.payout_binding)
                }
                _ => None,
            }?;
            Some((proposal_id, reference_kind))
        },
    )
}

fn retained_enacted_validation_fee_asset_reference_matching(
    state_transaction: &StateTransaction<'_, '_>,
    mut matches: impl FnMut(&AssetDefinitionId) -> bool,
) -> Option<([u8; 32], &'static str, AssetDefinitionId)> {
    retained_enacted_validation_fee_proposals(state_transaction).find_map(
        |(proposal_id, proposal_kind)| {
            let matched = match proposal_kind {
                ProposalKind::ValidationFeePolicy(payload) => {
                    if matches(&payload.policy.ds_asset_id) {
                        Some((
                            "policy DS asset definition",
                            payload.policy.ds_asset_id.clone(),
                        ))
                    } else if let Some(binding) = payload.policy.treasury_payout_binding.as_ref() {
                        if matches(&binding.ds_asset_id) {
                            Some(("payout DS asset definition", binding.ds_asset_id.clone()))
                        } else if matches(&binding.xor_asset_id) {
                            Some(("payout XOR asset definition", binding.xor_asset_id.clone()))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                ProposalKind::ValidationFeePayoutLifecycle(payload) => {
                    let binding = &payload.payout_binding;
                    if matches(&binding.ds_asset_id) {
                        Some(("payout DS asset definition", binding.ds_asset_id.clone()))
                    } else if matches(&binding.xor_asset_id) {
                        Some(("payout XOR asset definition", binding.xor_asset_id.clone()))
                    } else {
                        None
                    }
                }
                _ => None,
            }?;
            Some((proposal_id, matched.0, matched.1))
        },
    )
}

/// Return the retained enacted validation-fee proposal that pins an asset definition.
///
/// Every enacted policy DS and payout-lifecycle DS/XOR definition remains referenced even before
/// activation and after its balance drains.
pub(crate) fn retained_enacted_validation_fee_asset_reference(
    state_transaction: &StateTransaction<'_, '_>,
    asset_definition_id: &AssetDefinitionId,
) -> Option<([u8; 32], &'static str)> {
    retained_enacted_validation_fee_asset_reference_matching(state_transaction, |candidate| {
        candidate == asset_definition_id
    })
    .map(|(proposal_id, reference_kind, _)| (proposal_id, reference_kind))
}

/// Return one retained enacted validation-fee reference to any definition in `candidates`.
///
/// The typed proposal index is traversed once, so containing-domain teardown is `O(P + D)` rather
/// than rescanning `P` retained validation-fee proposals for each of its `D` definitions.
pub(crate) fn retained_enacted_validation_fee_asset_reference_in(
    state_transaction: &StateTransaction<'_, '_>,
    candidates: &std::collections::BTreeSet<AssetDefinitionId>,
) -> Option<([u8; 32], &'static str, AssetDefinitionId)> {
    retained_enacted_validation_fee_asset_reference_matching(state_transaction, |candidate| {
        candidates.contains(candidate)
    })
}

fn enacted_payout_binding_for_contract<'a>(
    state_transaction: &'a StateTransaction<'_, '_>,
    contract_address: &iroha_data_model::smart_contract::ContractAddress,
) -> Option<&'a ValidationFeeTreasuryPayoutBindingV1> {
    state_transaction
        .world
        .governance_proposals
        .iter()
        .filter(|(_, proposal)| proposal.status == crate::state::GovernanceProposalStatus::Enacted)
        .find_map(|(_, proposal)| {
            let iroha_data_model::governance::types::ProposalKind::ValidationFeePayoutLifecycle(
                lifecycle,
            ) = &proposal.kind
            else {
                return None;
            };
            (&lifecycle.payout_binding.contract_address == contract_address)
                .then_some(&lifecycle.payout_binding)
        })
}
/// Whether an enacted payout lifecycle pins this active contract address.
pub(crate) fn is_enacted_validation_fee_payout_contract(
    state_transaction: &StateTransaction<'_, '_>,
    contract_address: &iroha_data_model::smart_contract::ContractAddress,
) -> bool {
    enacted_payout_binding_for_contract(state_transaction, contract_address).is_some()
}
/// Whether an enacted payout lifecycle pins this exact scheduled trigger.
pub(crate) fn is_enacted_validation_fee_payout_trigger(
    state_transaction: &StateTransaction<'_, '_>,
    trigger_id: &iroha_data_model::trigger::TriggerId,
) -> bool {
    let Some(action) = state_transaction
        .world
        .triggers
        .time_triggers()
        .get(trigger_id)
    else {
        return false;
    };
    let ExecutableRef::ContractCall(invocation) = action.executable() else {
        return false;
    };
    let Some(binding) =
        enacted_payout_binding_for_contract(state_transaction, &invocation.contract_address)
    else {
        return false;
    };
    action.authority() == &binding.treasury_account_id
        && invocation.entrypoint == binding.entrypoint.as_ref()
        && trigger_is_enabled(action.metadata())
}
/// Whether a new scheduled invocation would duplicate an enacted payout path.
pub(crate) fn is_enacted_validation_fee_payout_invocation(
    state_transaction: &StateTransaction<'_, '_>,
    invocation: &iroha_data_model::transaction::executable::ContractInvocation,
) -> bool {
    let Some(binding) =
        enacted_payout_binding_for_contract(state_transaction, &invocation.contract_address)
    else {
        return false;
    };
    let Some(active_code_hash) = state_transaction
        .world
        .contract_instances
        .get(&invocation.contract_address)
        .copied()
    else {
        return false;
    };
    invocation.expected_code_hash == active_code_hash
        && invocation.entrypoint == binding.entrypoint.as_ref()
        && invocation.arguments.is_none()
}
fn trigger_id_from_permission(
    permission: &iroha_data_model::permission::Permission,
) -> Option<iroha_data_model::trigger::TriggerId> {
    iroha_executor_data_model::permission::trigger::CanUnregisterTrigger::try_from(permission)
        .map(|token| token.trigger)
        .or_else(|_| {
            iroha_executor_data_model::permission::trigger::CanModifyTrigger::try_from(permission)
                .map(|token| token.trigger)
        })
        .or_else(|_| {
            iroha_executor_data_model::permission::trigger::CanExecuteTrigger::try_from(permission)
                .map(|token| token.trigger)
        })
        .or_else(|_| {
            iroha_executor_data_model::permission::trigger::CanModifyTriggerMetadata::try_from(
                permission,
            )
            .map(|token| token.trigger)
        })
        .ok()
}
/// Whether a permission would delegate control of an enacted payout trigger.
pub(crate) fn permission_targets_enacted_validation_fee_payout_trigger(
    state_transaction: &StateTransaction<'_, '_>,
    permission: &iroha_data_model::permission::Permission,
) -> bool {
    trigger_id_from_permission(permission).is_some_and(|trigger_id| {
        is_enacted_validation_fee_payout_trigger(state_transaction, &trigger_id)
    })
}
/// Return the sole direct account allowed to hold one of an enacted payout
/// lifecycle's exact wrapper, pool-selector, or typed asset-effect tokens.
pub(crate) fn enacted_validation_fee_payout_runtime_permission_owner(
    state_transaction: &StateTransaction<'_, '_>,
    permission: &iroha_data_model::permission::Permission,
) -> Option<AccountId> {
    if let Ok(scoped) =
        iroha_executor_data_model::permission::smart_contract::CanInvokeContractEntrypoint::try_from(
            permission,
        )
    {
        return state_transaction
            .world
            .governance_proposals
            .iter()
            .filter(|(_, proposal)| {
                proposal.status == crate::state::GovernanceProposalStatus::Enacted
            })
            .find_map(|(_, proposal)| {
                let iroha_data_model::governance::types::ProposalKind::ValidationFeePayoutLifecycle(
                    lifecycle,
                ) = &proposal.kind
                else {
                    return None;
                };
                let binding = &lifecycle.payout_binding;
                let wrapper_selector = scoped.contract == binding.contract_address
                    && scoped.entrypoint == binding.entrypoint.as_ref();
                let pool_selector = scoped.contract.subject_id() == binding.pool_vault_account_id
                    && scoped.entrypoint == VALIDATION_FEE_POOL_SWAP_ENTRYPOINT;
                (wrapper_selector || pool_selector).then(|| binding.treasury_account_id.clone())
            });
    }
    let transfer =
        iroha_executor_data_model::permission::asset::CanTransferAsset::try_from(permission)
            .ok()?;
    state_transaction
        .world
        .governance_proposals
        .iter()
        .filter(|(_, proposal)| proposal.status == crate::state::GovernanceProposalStatus::Enacted)
        .find_map(|(_, proposal)| {
            let iroha_data_model::governance::types::ProposalKind::ValidationFeePayoutLifecycle(
                lifecycle,
            ) = &proposal.kind
            else {
                return None;
            };
            let binding = &lifecycle.payout_binding;
            let wrapper_ds_asset = iroha_data_model::asset::AssetId::new(
                binding.ds_asset_id.clone(),
                binding.treasury_account_id.clone(),
            );
            (transfer.asset == wrapper_ds_asset).then(|| binding.pool_vault_account_id.clone())
        })
}
/// Exact nominal protocol-fee value validated from a signed transaction payload.
///
/// This is an admission fact, not a balance mutation. Callers persist it only after the signed
/// transaction and all of its data triggers have completed successfully.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ValidationFeeCredit {
    treasury_account_id: AccountId,
    lifecycle_seal: [u8; 32],
    fee_asset_definition_id: AssetDefinitionId,
    asset_scale: u8,
    amount: Quantity,
}
impl ValidationFeeCredit {
    fn from_policy_minor_units(
        treasury_account_id: AccountId,
        lifecycle_seal: [u8; 32],
        fee_asset_definition_id: AssetDefinitionId,
        asset_scale: u8,
        minor_units: u64,
    ) -> Result<Self, ValidationFeeAdmissionError> {
        if lifecycle_seal == [0; 32] {
            return Err(ValidationFeeAdmissionError::InvalidPayoutLifecycleSeal);
        }
        Ok(Self {
            treasury_account_id,
            lifecycle_seal,
            fee_asset_definition_id,
            asset_scale,
            amount: quantity_from_policy_minor_units(minor_units, asset_scale)?,
        })
    }

    fn with_amount(&self, amount: Quantity) -> Self {
        Self {
            treasury_account_id: self.treasury_account_id.clone(),
            lifecycle_seal: self.lifecycle_seal,
            fee_asset_definition_id: self.fee_asset_definition_id.clone(),
            asset_scale: self.asset_scale,
            amount,
        }
    }
}
fn quantity_from_policy_minor_units(
    minor_units: u64,
    asset_scale: u8,
) -> Result<Quantity, ValidationFeeAdmissionError> {
    // V1 policy arithmetic is explicitly a u64 minor-unit protocol. Convert once at that
    // boundary; the consensus-owned durable balance remains a nominal Quantity thereafter.
    let value = Numeric::try_new(minor_units, u32::from(asset_scale)).map_err(|_| {
        ValidationFeeAdmissionError::InvalidPolicyInvariant(
            "validation-fee policy minor-unit scalar is outside the nominal Quantity domain",
        )
    })?;
    Quantity::from_canonical_numeric(value).map_err(|_| {
        ValidationFeeAdmissionError::InvalidPolicyInvariant(
            "validation-fee policy minor-unit scalar is negative",
        )
    })
}
#[derive(Debug, Clone, PartialEq, Eq)]
enum ValidationFeeAdmissionError {
    MalformedPolicyRegistryParameter,
    InvalidPolicyRegistry(String),
    MalformedHijiriParameters,
    InvalidHijiriParameters(String),
    InvalidPolicyInvariant(&'static str),
    WrongPolicyNetwork {
        expected: String,
        found: String,
    },
    PolicyExpired {
        expires_after_height: u64,
        current_height: u64,
    },
    TreasuryPayoutRequiresActiveContractSubject {
        treasury_account_id: String,
    },
    InvalidPayoutLifecycleSeal,
    MalformedCreditBalance {
        state_key: String,
    },
    MalformedCreditAssetBinding {
        state_key: String,
    },
    CreditAssetBindingMismatch {
        expected_asset_definition_id: String,
        observed_asset_definition_id: String,
    },
    MissingCreditAssetDefinition {
        asset_definition_id: String,
    },
    CreditAssetNumericSpecMismatch {
        asset_definition_id: String,
        expected_scale: u8,
        observed_scale: Option<u32>,
    },
    CreditAmountOutsideAssetSpec {
        asset_definition_id: String,
        amount: Quantity,
        allowed_scale: u8,
    },
    CreditBalanceOverflow {
        current: Quantity,
        additional: Quantity,
    },
    InsufficientCreditBalance {
        available: Quantity,
        requested: Quantity,
    },
    PolicyHashFailed,
    UnsupportedExecutable,
    NonMinorUnitAmount {
        instruction_index: usize,
        scale: u32,
        policy_scale: u8,
    },
    AmountTooLarge {
        instruction_index: usize,
    },
    RequiredFeeOverflow,
    MissingFee {
        required_minor_units: u64,
    },
    MissingFeeInstructionCoordinate,
    DuplicateFeeInstructions {
        count: usize,
    },
    WrongFeeAmount {
        expected_minor_units: u64,
        observed_minor_units: u64,
    },
    FeeInstructionNotFound {
        instruction_index: usize,
        entry_index: Option<usize>,
    },
    AmbiguousFeeInstructionCoordinate {
        instruction_index: usize,
        entry_index: Option<usize>,
    },
    OpaqueDeferredFeeAssetTransfer {
        execution_account_id: String,
        instruction_index: usize,
        entry_index: Option<usize>,
    },
    TreasuryPayoutRuntimeBindingMismatch {
        reason: &'static str,
    },
    AmbiguousTreasuryPayoutRuntimeIdentity {
        first_policy_hash_hex: String,
        second_policy_hash_hex: String,
    },
    TreasuryPayoutEffectPlanMismatch {
        reason: &'static str,
    },
    TreasuryPayoutArithmeticFailure,
    UnsupportedNativeFeeAssetMovement {
        context_index: usize,
        instruction_index: usize,
        instruction_wire_id: &'static str,
    },
    InvalidKagemushaOfflineConversion {
        context_index: usize,
        instruction_index: usize,
        instruction_wire_id: &'static str,
    },
    UnclassifiedNativeInstruction {
        context_index: usize,
        instruction_index: usize,
        registered_type_name: Option<&'static str>,
    },
    PotentialImplicitAccountAdmissionFee {
        context_index: usize,
        instruction_index: usize,
        entry_index: Option<usize>,
        destination_account_id: String,
    },
    UnresolvedOpaqueDeferredMultisigApproval {
        account_id: String,
        instructions_hash_hex: String,
    },
    OpaqueDeferredProposalDepthExceeded,
    OpaqueIvmProvedAxtEffects {
        completed_envelopes: usize,
    },
    MalformedMultisigFeeMarker {
        context_index: usize,
        instruction_index: usize,
    },
    MissingMultisigFeeMarker {
        context_index: usize,
    },
    DuplicateMultisigFeeMarkers {
        context_index: usize,
        count: usize,
    },
    UnexpectedMultisigFeeMarker {
        context_index: usize,
    },
    WrongMultisigFeeMarkerPolicyVersion {
        expected_version: u64,
        observed_version: u64,
    },
    WrongMultisigFeeMarkerPolicyHash {
        expected_hash_hex: String,
        observed_hash_hex: String,
    },
    WrongMultisigFeeMarkerHijiriFeeQuoteHash {
        expected_hash_hex: Option<String>,
        observed_hash_hex: Option<String>,
    },
    ConflictingMultisigFeeCoordinate {
        context_index: usize,
    },
    WrongFeeSource {
        instruction_index: usize,
        entry_index: Option<usize>,
    },
    WrongFeeAsset {
        instruction_index: usize,
        entry_index: Option<usize>,
    },
    WrongFeeBeneficiary {
        instruction_index: usize,
        entry_index: Option<usize>,
        expected_account_id: String,
        observed_account_id: String,
    },
    MalformedFeeInstructionMetadata,
    MissingPolicyVersionMetadata,
    MalformedPolicyVersionMetadata,
    WrongPolicyVersionMetadata {
        expected_version: u64,
        observed_version: u64,
    },
    MissingPolicyHashMetadata,
    MalformedPolicyHashMetadata,
    WrongPolicyHashMetadata {
        expected_hash_hex: String,
        observed_hash_hex: String,
    },
    MissingHijiriFeeQuoteHashMetadata,
    UnexpectedHijiriFeeQuoteHashMetadata,
    MalformedHijiriFeeQuoteHashMetadata,
    WrongHijiriFeeQuoteHashMetadata {
        expected_hash_hex: String,
        observed_hash_hex: String,
    },
}
impl fmt::Display for ValidationFeeAdmissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MalformedPolicyRegistryParameter => {
                write!(f, "validation-fee policy registry parameter is malformed")
            }
            Self::InvalidPolicyRegistry(reason) => {
                write!(f, "validation-fee policy registry is invalid: {reason}")
            }
            Self::MalformedHijiriParameters => {
                write!(f, "Hijiri parameter is malformed")
            }
            Self::InvalidHijiriParameters(reason) => {
                write!(f, "Hijiri parameter is invalid: {reason}")
            }
            Self::InvalidPolicyInvariant(reason) => {
                write!(f, "validation-fee policy is invalid: {reason}")
            }
            Self::WrongPolicyNetwork { expected, found } => write!(
                f,
                "validation-fee policy network mismatch: expected {expected}, found {found}"
            ),
            Self::PolicyExpired {
                expires_after_height,
                current_height,
            } => write!(
                f,
                "validation-fee policy expired at height {expires_after_height}; current height is {current_height}"
            ),
            Self::TreasuryPayoutRequiresActiveContractSubject {
                treasury_account_id,
            } => write!(
                f,
                "Parliament-enacted TREASURY_PAYOUT requires treasury {treasury_account_id} to be an active immutable non-signable contract subject in world state"
            ),
            Self::InvalidPayoutLifecycleSeal => write!(
                f,
                "validation-fee payout lifecycle seal is zero or cannot be derived"
            ),
            Self::MalformedCreditBalance { state_key } => write!(
                f,
                "validation-fee credit balance at reserved state key `{state_key}` is malformed"
            ),
            Self::MalformedCreditAssetBinding { state_key } => write!(
                f,
                "validation-fee credit asset binding at reserved state key `{state_key}` is malformed"
            ),
            Self::CreditAssetBindingMismatch {
                expected_asset_definition_id,
                observed_asset_definition_id,
            } => write!(
                f,
                "validation-fee credit belongs to asset {observed_asset_definition_id}, not active policy asset {expected_asset_definition_id}"
            ),
            Self::MissingCreditAssetDefinition {
                asset_definition_id,
            } => write!(
                f,
                "validation-fee credit asset definition {asset_definition_id} is missing"
            ),
            Self::CreditAssetNumericSpecMismatch {
                asset_definition_id,
                expected_scale,
                observed_scale,
            } => write!(
                f,
                "validation-fee credit asset {asset_definition_id} must have numeric scale {expected_scale}, found {}",
                observed_scale
                    .map_or_else(|| "unconstrained".to_owned(), |scale| scale.to_string())
            ),
            Self::CreditAmountOutsideAssetSpec {
                asset_definition_id,
                amount,
                allowed_scale,
            } => write!(
                f,
                "validation-fee credit amount {amount} does not satisfy asset {asset_definition_id} scale {allowed_scale}"
            ),
            Self::CreditBalanceOverflow {
                current,
                additional,
            } => write!(
                f,
                "validation-fee credit balance overflow: current {current}, additional {additional}"
            ),
            Self::InsufficientCreditBalance {
                available,
                requested,
            } => write!(
                f,
                "TREASURY_PAYOUT exceeds validation-fee credit balance: available {available}, requested {requested}"
            ),
            Self::PolicyHashFailed => write!(f, "validation-fee policy hash failed"),
            Self::UnsupportedExecutable => {
                write!(
                    f,
                    "validation-fee policy rejects raw contract and IVM executables; use an instruction list or a proof-bound IVM overlay"
                )
            }
            Self::NonMinorUnitAmount {
                instruction_index,
                scale,
                policy_scale,
            } => write!(
                f,
                "fee-asset transfer instruction {instruction_index} uses scale {scale}, above policy scale {policy_scale}"
            ),
            Self::AmountTooLarge { instruction_index } => {
                write!(
                    f,
                    "fee-asset transfer instruction {instruction_index} amount exceeds supported minor-unit range"
                )
            }
            Self::RequiredFeeOverflow => write!(f, "validation-fee calculation overflowed"),
            Self::MissingFee {
                required_minor_units,
            } => write!(
                f,
                "missing validation-fee transfer of {required_minor_units} minor units"
            ),
            Self::MissingFeeInstructionCoordinate => {
                write!(
                    f,
                    "missing signed validation-fee instruction coordinate metadata"
                )
            }
            Self::DuplicateFeeInstructions { count } => {
                write!(f, "ambiguous validation-fee transfer count {count}")
            }
            Self::WrongFeeAmount {
                expected_minor_units,
                observed_minor_units,
            } => write!(
                f,
                "wrong validation-fee amount: expected {expected_minor_units} minor units, observed {observed_minor_units}"
            ),
            Self::FeeInstructionNotFound {
                instruction_index,
                entry_index,
            } => write!(
                f,
                "signed validation-fee instruction coordinate {instruction_index}{} does not reference a transfer",
                format_entry_index(*entry_index)
            ),
            Self::AmbiguousFeeInstructionCoordinate {
                instruction_index,
                entry_index,
            } => write!(
                f,
                "signed validation-fee instruction coordinate {instruction_index}{} matches multiple transfer contexts",
                format_entry_index(*entry_index)
            ),
            Self::OpaqueDeferredFeeAssetTransfer {
                execution_account_id,
                instruction_index,
                entry_index,
            } => write!(
                f,
                "opaque deferred executable derived a policy fee-asset transfer at {instruction_index}{} for execution authority {execution_account_id}; concrete principal and fee effects must be signed instructions",
                format_entry_index(*entry_index)
            ),
            Self::TreasuryPayoutRuntimeBindingMismatch { reason } => write!(
                f,
                "TREASURY_PAYOUT runtime does not match the enacted lifecycle binding: {reason}"
            ),
            Self::AmbiguousTreasuryPayoutRuntimeIdentity {
                first_policy_hash_hex,
                second_policy_hash_hex,
            } => write!(
                f,
                "TREASURY_PAYOUT scheduled runtime matches multiple retained lifecycle identities from policies {first_policy_hash_hex} and {second_policy_hash_hex}"
            ),
            Self::TreasuryPayoutEffectPlanMismatch { reason } => write!(
                f,
                "TREASURY_PAYOUT effect plan does not match the enacted lifecycle binding: {reason}"
            ),
            Self::TreasuryPayoutArithmeticFailure => write!(
                f,
                "TREASURY_PAYOUT output/share arithmetic is not exactly representable"
            ),
            Self::UnsupportedNativeFeeAssetMovement {
                context_index,
                instruction_index,
                instruction_wire_id,
            } => write!(
                f,
                "native instruction `{instruction_wire_id}` at instruction {instruction_index} in execution context {context_index} can move the policy DS outside an explicit asset transfer; this path is disabled while the validation-fee policy is active"
            ),
            Self::InvalidKagemushaOfflineConversion {
                context_index,
                instruction_index,
                instruction_wire_id,
            } => write!(
                f,
                "Kagemusha offline-cash conversion `{instruction_wire_id}` at instruction {instruction_index} in execution context {context_index} does not have a valid payer/recipient-signed public effect binding"
            ),
            Self::UnclassifiedNativeInstruction {
                context_index,
                instruction_index,
                registered_type_name,
            } => {
                if let Some(registered_type_name) = registered_type_name {
                    write!(
                        f,
                        "native instruction `{registered_type_name}` at instruction {instruction_index} in execution context {context_index} has no audited validation-fee DS effect disposition; it is disabled while the validation-fee policy is active"
                    )
                } else {
                    write!(
                        f,
                        "custom or unregistered instruction at instruction {instruction_index} in execution context {context_index} has no audited validation-fee DS effect disposition; it is disabled while the validation-fee policy is active"
                    )
                }
            }
            Self::PotentialImplicitAccountAdmissionFee {
                context_index,
                instruction_index,
                entry_index,
                destination_account_id,
            } => write!(
                f,
                "asset transfer at instruction {instruction_index}{} in execution context {context_index} targets unregistered account {destination_account_id}; implicit account admission can derive an unsigned DS fee and is disabled while the validation-fee policy is active",
                format_entry_index(*entry_index)
            ),
            Self::UnresolvedOpaqueDeferredMultisigApproval {
                account_id,
                instructions_hash_hex,
            } => write!(
                f,
                "opaque deferred executable contains a multisig approval that cannot be resolved before execution for account {account_id} and instructions hash {instructions_hash_hex}"
            ),
            Self::OpaqueDeferredProposalDepthExceeded => write!(
                f,
                "opaque deferred validation-fee proposal graph exceeds the maximum traversal depth"
            ),
            Self::OpaqueIvmProvedAxtEffects {
                completed_envelopes,
            } => write!(
                f,
                "IvmProved replay completed {completed_envelopes} AXT envelope(s), but those effects are not represented in the signed overlay; proof-carrying AXT is disabled while the validation-fee policy is active"
            ),
            Self::MalformedMultisigFeeMarker {
                context_index,
                instruction_index,
            } => write!(
                f,
                "multisig validation-fee marker is malformed in context {context_index} at instruction {instruction_index}"
            ),
            Self::MissingMultisigFeeMarker { context_index } => write!(
                f,
                "fee-bearing multisig context {context_index} is missing its signed validation-fee marker"
            ),
            Self::DuplicateMultisigFeeMarkers {
                context_index,
                count,
            } => write!(
                f,
                "fee-bearing multisig context {context_index} contains {count} signed validation-fee markers; exactly one is required"
            ),
            Self::UnexpectedMultisigFeeMarker { context_index } => write!(
                f,
                "multisig context {context_index} contains a validation-fee marker without a fee-asset effect"
            ),
            Self::WrongMultisigFeeMarkerPolicyVersion {
                expected_version,
                observed_version,
            } => write!(
                f,
                "wrong multisig validation-fee marker policy version: expected {expected_version}, observed {observed_version}"
            ),
            Self::WrongMultisigFeeMarkerPolicyHash {
                expected_hash_hex,
                observed_hash_hex,
            } => write!(
                f,
                "wrong multisig validation-fee marker policy hash: expected {expected_hash_hex}, observed {observed_hash_hex}"
            ),
            Self::WrongMultisigFeeMarkerHijiriFeeQuoteHash {
                expected_hash_hex,
                observed_hash_hex,
            } => write!(
                f,
                "wrong multisig validation-fee marker Hijiri quote hash: expected {}, observed {}",
                expected_hash_hex.as_deref().unwrap_or("-"),
                observed_hash_hex.as_deref().unwrap_or("-")
            ),
            Self::ConflictingMultisigFeeCoordinate { context_index } => write!(
                f,
                "transaction metadata and signed multisig validation-fee marker disagree in context {context_index}"
            ),
            Self::WrongFeeSource {
                instruction_index,
                entry_index,
            } => write!(
                f,
                "signed validation-fee instruction coordinate {instruction_index}{} is not paid by the transaction authority",
                format_entry_index(*entry_index)
            ),
            Self::WrongFeeAsset {
                instruction_index,
                entry_index,
            } => write!(
                f,
                "signed validation-fee instruction coordinate {instruction_index}{} does not pay the policy fee asset",
                format_entry_index(*entry_index)
            ),
            Self::WrongFeeBeneficiary {
                instruction_index,
                entry_index,
                expected_account_id,
                observed_account_id,
            } => write!(
                f,
                "signed validation-fee instruction coordinate {instruction_index}{} pays wrong beneficiary: expected {expected_account_id}, observed {observed_account_id}",
                format_entry_index(*entry_index)
            ),
            Self::MalformedFeeInstructionMetadata => {
                write!(f, "signed validation-fee instruction metadata is malformed")
            }
            Self::MissingPolicyVersionMetadata => {
                write!(f, "missing signed validation-fee policy version metadata")
            }
            Self::MalformedPolicyVersionMetadata => {
                write!(
                    f,
                    "signed validation-fee policy version metadata is malformed"
                )
            }
            Self::WrongPolicyVersionMetadata {
                expected_version,
                observed_version,
            } => write!(
                f,
                "wrong signed validation-fee policy version: expected {expected_version}, observed {observed_version}"
            ),
            Self::MissingPolicyHashMetadata => {
                write!(f, "missing signed validation-fee policy hash metadata")
            }
            Self::MalformedPolicyHashMetadata => {
                write!(f, "signed validation-fee policy hash metadata is malformed")
            }
            Self::WrongPolicyHashMetadata {
                expected_hash_hex,
                observed_hash_hex,
            } => write!(
                f,
                "wrong signed validation-fee policy hash: expected {expected_hash_hex}, observed {observed_hash_hex}"
            ),
            Self::MissingHijiriFeeQuoteHashMetadata => {
                write!(
                    f,
                    "missing signed validation-fee Hijiri quote hash metadata"
                )
            }
            Self::UnexpectedHijiriFeeQuoteHashMetadata => write!(
                f,
                "signed validation-fee Hijiri quote hash metadata is present while Hijiri pricing is inactive"
            ),
            Self::MalformedHijiriFeeQuoteHashMetadata => write!(
                f,
                "signed validation-fee Hijiri quote hash metadata is malformed"
            ),
            Self::WrongHijiriFeeQuoteHashMetadata {
                expected_hash_hex,
                observed_hash_hex,
            } => write!(
                f,
                "wrong signed validation-fee Hijiri quote hash: expected {expected_hash_hex}, observed {observed_hash_hex}"
            ),
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FeeInstructionCoordinate {
    instruction_index: usize,
    entry_index: Option<usize>,
}
impl FeeInstructionCoordinate {
    fn matches<T: TransferLocation>(&self, transfer: &T) -> bool {
        self.instruction_index == transfer.instruction_index()
            && self.entry_index == transfer.entry_index()
    }
}
trait TransferLocation {
    fn instruction_index(&self) -> usize;
    fn entry_index(&self) -> Option<usize>;
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct TransferCollection {
    contexts: Vec<TransferExecutionContext>,
    transfers: Vec<AssetTransferSummary>,
    multisig_fee_markers: Vec<MultisigFeeMarkerSummary>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct TransferExecutionContext {
    execution_account_id: AccountId,
    requires_multisig_fee_marker: bool,
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct MultisigFeeMarkerSummary {
    context_index: usize,
    marker_instruction_index: usize,
    marker: ValidationFeeMultisigMarkerV1,
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct AssetTransferSummary {
    context_index: usize,
    instruction_index: usize,
    entry_index: Option<usize>,
    asset_definition_id: AssetDefinitionId,
    source_account_id: AccountId,
    destination_account_id: AccountId,
    amount: Quantity,
}
impl TransferLocation for AssetTransferSummary {
    fn instruction_index(&self) -> usize {
        self.instruction_index
    }
    fn entry_index(&self) -> Option<usize> {
        self.entry_index
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct FeeAssetTransferSummary {
    context_index: usize,
    instruction_index: usize,
    entry_index: Option<usize>,
    source_account_id: AccountId,
    destination_account_id: AccountId,
    amount_minor_units: u64,
}
impl TransferLocation for FeeAssetTransferSummary {
    fn instruction_index(&self) -> usize {
        self.instruction_index
    }
    fn entry_index(&self) -> Option<usize> {
        self.entry_index
    }
}
pub(crate) fn enforce_validation_fee_admission(
    tx: &SignedTransaction,
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<Option<ValidationFeeCredit>, TransactionRejectionReason> {
    if is_validation_fee_control_plane_transaction(tx) {
        return Ok(None);
    }
    let Some(policy) = active_policy(state_transaction)? else {
        return Ok(None);
    };
    let hijiri = active_hijiri_parameters(state_transaction)?;
    let resolve_account_risk =
        |account_id: &AccountId| active_hijiri_account_risk(state_transaction, account_id);
    let credited_minor_units =
        enforce_policy_with_credit_and_hijiri(tx, &policy, hijiri.as_ref(), &resolve_account_risk)
            .map_err(|err| {
                TransactionRejectionReason::Validation(ValidationFail::NotPermitted(format!(
                    "validation-fee admission rejected transaction: {err}"
                )))
            })?;
    let fee_asset_definition_id =
        policy_fee_asset_definition_id(&policy).map_err(admission_rejection)?;
    let collection =
        collect_asset_transfers(tx.instructions(), tx.authority(), &fee_asset_definition_id)
            .map_err(admission_rejection)?;
    reject_potential_implicit_account_admission_fee_with(&collection, |account| {
        state_transaction.world.account(account).is_ok()
    })
    .map_err(admission_rejection)?;
    if credited_minor_units == 0 || !treasury_payout_exemption_enabled(&policy) {
        return Ok(None);
    }
    let lifecycle_seal = policy_payout_lifecycle_seal(&policy).map_err(admission_rejection)?;
    let credit = ValidationFeeCredit::from_policy_minor_units(
        policy_treasury_account_id(&policy).map_err(admission_rejection)?,
        lifecycle_seal,
        fee_asset_definition_id,
        policy.ds_scale,
        credited_minor_units,
    )
    .map_err(admission_rejection)?;
    ensure_validation_fee_credit_capacity(state_transaction, &credit)
        .map_err(admission_rejection)?;
    Ok(Some(credit))
}
fn is_validation_fee_control_plane_transaction(tx: &SignedTransaction) -> bool {
    fn is_control_plane_instruction(instruction: &InstructionBox) -> bool {
        instruction
            .as_any()
            .downcast_ref::<ProposeValidationFeePolicy>()
            .is_some()
            || instruction
                .as_any()
                .downcast_ref::<ProposeValidationFeePayoutLifecycle>()
                .is_some()
            || instruction
                .as_any()
                .downcast_ref::<CreateParliamentGovernanceAttemptV1>()
                .is_some()
            || instruction
                .as_any()
                .downcast_ref::<SubmitParliamentLifecycleTransitionV1>()
                .is_some()
    }
    match tx.instructions() {
        Executable::Instructions(instructions) => {
            !instructions.is_empty() && instructions.iter().all(is_control_plane_instruction)
        }
        Executable::Batch(items) => {
            !items.is_empty()
                && items.iter().all(|item| match item {
                    ExecutableBatchItem::Instruction(instruction) => {
                        is_control_plane_instruction(instruction)
                    }
                    ExecutableBatchItem::ContractCall(_) => false,
                })
        }
        Executable::Ivm(_) | Executable::IvmProved(_) | Executable::ContractCall(_) => false,
    }
}
/// Reject proof-carrying AXT completions while an active validation-fee policy cannot classify
/// their DS effects from the signed transaction payload.
///
/// An exact fee in `IvmProved.overlay` is insufficient: the completed AXT envelope may move
/// additional DS not represented by that overlay. This remains fail-closed until signed AXT
/// effects are modeled directly.
pub(crate) fn enforce_ivm_proved_completed_axt_admission(
    completed_envelopes: usize,
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<(), ValidationFail> {
    if completed_envelopes == 0 {
        return Ok(());
    }
    let policy = active_policy(state_transaction).map_err(|rejection| match rejection {
        TransactionRejectionReason::Validation(fail) => fail,
        other => ValidationFail::NotPermitted(format!(
            "validation-fee policy resolution failed during IvmProved AXT admission: {other:?}"
        )),
    })?;
    if policy.is_none() {
        return Ok(());
    }
    let error = reject_ivm_proved_completed_axt_effects(completed_envelopes)
        .expect_err("non-zero completed AXT count must fail closed under active policy");
    let rejection = admission_rejection(error);
    match rejection {
        TransactionRejectionReason::Validation(fail) => Err(fail),
        _ => unreachable!("validation-fee admission rejection must be a validation failure"),
    }
}
fn reject_ivm_proved_completed_axt_effects(
    completed_envelopes: usize,
) -> Result<(), ValidationFeeAdmissionError> {
    if completed_envelopes == 0 {
        return Ok(());
    }
    Err(ValidationFeeAdmissionError::OpaqueIvmProvedAxtEffects {
        completed_envelopes,
    })
}
/// Re-evaluate a stored or derived instruction list against the policy active at execution time.
///
/// Deferred executables do not retain transaction-level fee-coordinate metadata, so each execution
/// context must contain one unambiguous authority-paid treasury transfer with the exact aggregate
/// fee. This deliberately makes pre-activation fee-free work fail closed after policy activation.
pub(crate) fn enforce_deferred_instruction_list(
    authority: &AccountId,
    instructions: &[InstructionBox],
    state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<(), TransactionRejectionReason> {
    let Some(policy) = active_policy(state_transaction)? else {
        return Ok(());
    };
    let hijiri = active_hijiri_parameters(state_transaction)?;
    let resolve_account_risk =
        |account_id: &AccountId| active_hijiri_account_risk(state_transaction, account_id);
    let credited_minor_units = enforce_deferred_policy_with_credit_and_hijiri(
        authority,
        instructions,
        &policy,
        hijiri.as_ref(),
        &resolve_account_risk,
    )
    .map_err(admission_rejection)?;
    let fee_asset_definition_id =
        policy_fee_asset_definition_id(&policy).map_err(admission_rejection)?;
    let mut collection = TransferCollection {
        contexts: vec![TransferExecutionContext {
            execution_account_id: authority.clone(),
            requires_multisig_fee_marker: true,
        }],
        transfers: Vec::new(),
        multisig_fee_markers: Vec::new(),
    };
    collect_instruction_asset_transfers(instructions, 0, &fee_asset_definition_id, &mut collection)
        .map_err(admission_rejection)?;
    reject_potential_implicit_account_admission_fee_with(&collection, |account| {
        state_transaction.world.account(account).is_ok()
    })
    .map_err(admission_rejection)?;
    if credited_minor_units > 0 && treasury_payout_exemption_enabled(&policy) {
        let lifecycle_seal = policy_payout_lifecycle_seal(&policy).map_err(admission_rejection)?;
        let credit = ValidationFeeCredit::from_policy_minor_units(
            policy_treasury_account_id(&policy).map_err(admission_rejection)?,
            lifecycle_seal,
            fee_asset_definition_id,
            policy.ds_scale,
            credited_minor_units,
        )
        .map_err(admission_rejection)?;
        ensure_validation_fee_credit_capacity(state_transaction, &credit)
            .map_err(admission_rejection)?;
        commit_validation_fee_credit(state_transaction, Some(&credit))?;
    }
    Ok(())
}
/// Reject policy fee-asset effects derived by opaque deferred VM/contract execution.
///
/// Trigger registration signs the opaque executable, not the concrete event/state-derived
/// instruction artifacts. Consequently, permitting even an exactly balanced derived DS transfer
/// would violate the requirement that both principal and fee are covered by the user signature.
pub(crate) struct OpaqueDeferredRuntimeOrigin<'a> {
    runtime_context: &'a crate::executor::ContractRuntimeExecutionContext,
    code_bytes: &'a [u8],
    trigger_id: Option<&'a iroha_data_model::trigger::TriggerId>,
    scheduled_time_trigger: bool,
}
/// Whether validated opaque effects should be atomically applied or discarded
/// as the bound payout's legitimate empty/insufficient-credit no-op.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OpaqueDeferredValidationOutcome {
    Apply,
    NoOp,
}
impl<'a> OpaqueDeferredRuntimeOrigin<'a> {
    pub(crate) fn new(
        runtime_context: &'a crate::executor::ContractRuntimeExecutionContext,
        code_bytes: &'a [u8],
    ) -> Self {
        Self {
            runtime_context,
            code_bytes,
            trigger_id: None,
            scheduled_time_trigger: false,
        }
    }
    /// Bind opaque execution to a trigger event, admitting the payout exemption
    /// only when consensus invoked the contract from a scheduled Time trigger.
    pub(crate) fn from_trigger_event(
        runtime_context: &'a crate::executor::ContractRuntimeExecutionContext,
        code_bytes: &'a [u8],
        event: &iroha_data_model::events::EventBox,
        trigger_id: &'a iroha_data_model::trigger::TriggerId,
    ) -> Self {
        Self {
            runtime_context,
            code_bytes,
            trigger_id: Some(trigger_id),
            scheduled_time_trigger: matches!(event, iroha_data_model::events::EventBox::Time(_)),
        }
    }
    #[cfg(test)]
    fn scheduled_time_trigger(
        runtime_context: &'a crate::executor::ContractRuntimeExecutionContext,
        code_bytes: &'a [u8],
        trigger_id: &'a iroha_data_model::trigger::TriggerId,
    ) -> Self {
        Self {
            runtime_context,
            code_bytes,
            trigger_id: Some(trigger_id),
            scheduled_time_trigger: true,
        }
    }
}
pub(crate) fn enforce_opaque_deferred_instruction_groups(
    instruction_groups: &std::collections::BTreeMap<AccountId, Vec<InstructionBox>>,
    ordered_instructions: &[(AccountId, InstructionBox)],
    state_transaction: &mut StateTransaction<'_, '_>,
    runtime_origin: Option<OpaqueDeferredRuntimeOrigin<'_>>,
) -> Result<OpaqueDeferredValidationOutcome, TransactionRejectionReason> {
    let registry = validated_policy_registry(state_transaction)?;
    let active_policy =
        active_policy_from_validated_registry(registry.as_ref(), state_transaction)?;
    let payout_lifecycle = resolve_retained_payout_lifecycle(
        registry.as_ref(),
        state_transaction,
        runtime_origin.as_ref(),
    )
    .map_err(admission_rejection)?;
    let (fee_asset_definition_id, treasury_payout_authority) =
        if let Some(lifecycle) = payout_lifecycle.as_ref() {
            (
                lifecycle.binding.ds_asset_id.clone(),
                Some(&lifecycle.binding.treasury_account_id),
            )
        } else if let Some(policy) = active_policy.as_ref() {
            (
                policy_fee_asset_definition_id(policy).map_err(admission_rejection)?,
                None,
            )
        } else {
            return Ok(OpaqueDeferredValidationOutcome::Apply);
        };
    enforce_opaque_deferred_fee_asset_policy(
        instruction_groups,
        &fee_asset_definition_id,
        treasury_payout_authority,
    )
    .map_err(admission_rejection)?;
    let mut visited_proposals = std::collections::BTreeSet::new();
    let mut resolve = |approve: &iroha_executor_data_model::isi::multisig::MultisigApprove| {
        crate::smartcontracts::isi::multisig::live_proposal_instructions_for_approval(
            state_transaction,
            approve,
        )
    };
    for (authority, instructions) in instruction_groups {
        let mut collection = TransferCollection {
            contexts: vec![TransferExecutionContext {
                execution_account_id: authority.clone(),
                requires_multisig_fee_marker: false,
            }],
            transfers: Vec::new(),
            multisig_fee_markers: Vec::new(),
        };
        collect_instruction_asset_transfers(
            instructions,
            0,
            &fee_asset_definition_id,
            &mut collection,
        )
        .map_err(admission_rejection)?;
        reject_potential_implicit_account_admission_fee_with(&collection, |account| {
            state_transaction.world.account(account).is_ok()
        })
        .map_err(admission_rejection)?;
        reject_opaque_deferred_approval_effects_with(
            authority,
            instructions,
            &fee_asset_definition_id,
            &mut visited_proposals,
            0,
            &mut resolve,
        )
        .map_err(admission_rejection)?;
    }
    if let Some(lifecycle) = payout_lifecycle.as_ref() {
        let binding = &lifecycle.binding;
        if ordered_instructions.is_empty() && instruction_groups.is_empty() {
            return Ok(OpaqueDeferredValidationOutcome::NoOp);
        }
        let batch_credit = ValidationFeeCredit {
            treasury_account_id: binding.treasury_account_id.clone(),
            lifecycle_seal: lifecycle.lifecycle_seal,
            fee_asset_definition_id,
            asset_scale: lifecycle.ds_scale,
            amount: binding.batch_ds.clone(),
        };
        let available = read_validation_fee_credit_balance(state_transaction, &batch_credit)
            .map_err(admission_rejection)?;
        let xor_scale = validation_fee_payout_xor_scale(state_transaction, binding)
            .map_err(admission_rejection)?;
        let Some(terms) =
            validation_fee_payout_terms(&available, binding, lifecycle.ds_scale, xor_scale)
                .map_err(admission_rejection)?
        else {
            return Ok(OpaqueDeferredValidationOutcome::NoOp);
        };
        if !validate_treasury_payout_effect_plan(
            instruction_groups,
            ordered_instructions,
            binding,
            &terms,
        )
        .map_err(admission_rejection)?
        {
            return Ok(OpaqueDeferredValidationOutcome::NoOp);
        }
        let debit = batch_credit.with_amount(terms.debit_ds);
        consume_validation_fee_credit(state_transaction, &debit).map_err(admission_rejection)?;
    }
    Ok(OpaqueDeferredValidationOutcome::Apply)
}
#[cfg(test)]
fn enforce_opaque_deferred_policy(
    instruction_groups: &std::collections::BTreeMap<AccountId, Vec<InstructionBox>>,
    policy: &ValidationFeePolicyV1,
    treasury_payout_authority: Option<&AccountId>,
) -> Result<(), ValidationFeeAdmissionError> {
    let fee_asset_definition_id = policy_fee_asset_definition_id(policy)?;
    let policy_treasury = policy_treasury_account_id(policy)?;
    let allowed_treasury_payout_authority = treasury_payout_authority.filter(|authority| {
        treasury_payout_exemption_enabled(policy) && **authority == policy_treasury
    });
    enforce_opaque_deferred_fee_asset_policy(
        instruction_groups,
        &fee_asset_definition_id,
        allowed_treasury_payout_authority,
    )
}
fn enforce_opaque_deferred_fee_asset_policy(
    instruction_groups: &std::collections::BTreeMap<AccountId, Vec<InstructionBox>>,
    fee_asset_definition_id: &AssetDefinitionId,
    allowed_treasury_payout_authority: Option<&AccountId>,
) -> Result<(), ValidationFeeAdmissionError> {
    for (authority, instructions) in instruction_groups {
        let allowed_treasury =
            allowed_treasury_payout_authority.filter(|treasury| authority == *treasury);
        reject_opaque_fee_asset_effects(
            authority,
            instructions,
            fee_asset_definition_id,
            allowed_treasury,
        )?;
    }
    Ok(())
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct ValidationFeePayoutTerms {
    debit_ds: Quantity,
    min_xor_out: Quantity,
    max_xor_out: Quantity,
    xor_scale: u32,
}
fn validation_fee_payout_xor_scale(
    state_transaction: &StateTransaction<'_, '_>,
    binding: &ValidationFeeTreasuryPayoutBindingV1,
) -> Result<u32, ValidationFeeAdmissionError> {
    let definition = state_transaction
        .world
        .asset_definition(&binding.xor_asset_id)
        .map_err(
            |_| ValidationFeeAdmissionError::TreasuryPayoutRuntimeBindingMismatch {
                reason: "the bound XOR asset definition is missing",
            },
        )?;
    definition.spec().scale().ok_or(
        ValidationFeeAdmissionError::TreasuryPayoutRuntimeBindingMismatch {
            reason: "the bound XOR asset must have a fixed minor-unit scale",
        },
    )
}
fn validation_fee_payout_terms(
    seal_credit: &Quantity,
    binding: &ValidationFeeTreasuryPayoutBindingV1,
    ds_scale: u8,
    xor_scale: u32,
) -> Result<Option<ValidationFeePayoutTerms>, ValidationFeeAdmissionError> {
    let debit_ds = if seal_credit < &binding.batch_ds {
        seal_credit.clone()
    } else {
        binding.batch_ds.clone()
    };
    if debit_ds.is_zero() {
        return Ok(None);
    }
    let batch_minor = quantity_to_minor_units_u128(&binding.batch_ds, u32::from(ds_scale))?;
    let debit_minor = quantity_to_minor_units_u128(&debit_ds, u32::from(ds_scale))?;
    if batch_minor == 0 || debit_minor == 0 || debit_minor > batch_minor {
        return Err(ValidationFeeAdmissionError::TreasuryPayoutArithmeticFailure);
    }
    let min_minor = quantity_to_minor_units_u128(&binding.min_xor_out, xor_scale)?;
    let max_minor = quantity_to_minor_units_u128(&binding.max_xor_out, xor_scale)?;
    let scaled_min_numerator = min_minor
        .checked_mul(debit_minor)
        .ok_or(ValidationFeeAdmissionError::TreasuryPayoutArithmeticFailure)?;
    let scaled_max_numerator = max_minor
        .checked_mul(debit_minor)
        .ok_or(ValidationFeeAdmissionError::TreasuryPayoutArithmeticFailure)?;
    let min_scaled_minor = checked_ceil_ratio(scaled_min_numerator, batch_minor)?;
    let max_scaled_minor = scaled_max_numerator / batch_minor;
    if min_scaled_minor > max_scaled_minor || max_scaled_minor == 0 {
        return Ok(None);
    }
    Ok(Some(ValidationFeePayoutTerms {
        debit_ds,
        min_xor_out: quantity_from_minor_units_u128(min_scaled_minor, xor_scale)?,
        max_xor_out: quantity_from_minor_units_u128(max_scaled_minor, xor_scale)?,
        xor_scale,
    }))
}
fn checked_ceil_ratio(
    numerator: u128,
    denominator: u128,
) -> Result<u128, ValidationFeeAdmissionError> {
    if denominator == 0 {
        return Err(ValidationFeeAdmissionError::TreasuryPayoutArithmeticFailure);
    }
    let quotient = numerator / denominator;
    if numerator % denominator == 0 {
        Ok(quotient)
    } else {
        quotient
            .checked_add(1)
            .ok_or(ValidationFeeAdmissionError::TreasuryPayoutArithmeticFailure)
    }
}
fn quantity_to_minor_units_u128(
    amount: &Quantity,
    asset_scale: u32,
) -> Result<u128, ValidationFeeAdmissionError> {
    let amount_scale = amount.scale();
    if amount_scale > asset_scale {
        return Err(ValidationFeeAdmissionError::TreasuryPayoutArithmeticFailure);
    }
    let mantissa = amount
        .as_numeric()
        .try_mantissa_u128()
        .ok_or(ValidationFeeAdmissionError::TreasuryPayoutArithmeticFailure)?;
    mantissa
        .checked_mul(pow10(asset_scale - amount_scale)?)
        .ok_or(ValidationFeeAdmissionError::TreasuryPayoutArithmeticFailure)
}
fn quantity_from_minor_units_u128(
    minor_units: u128,
    asset_scale: u32,
) -> Result<Quantity, ValidationFeeAdmissionError> {
    let numeric = Numeric::try_new(minor_units, asset_scale)
        .map_err(|_| ValidationFeeAdmissionError::TreasuryPayoutArithmeticFailure)?;
    Quantity::from_canonical_numeric(numeric)
        .map_err(|_| ValidationFeeAdmissionError::TreasuryPayoutArithmeticFailure)
}
fn canonical_validation_fee_payouts(
    binding: &ValidationFeeTreasuryPayoutBindingV1,
    amount_out: &Quantity,
    xor_scale: u32,
) -> Result<Option<Vec<(AccountId, Quantity)>>, ValidationFeeAdmissionError> {
    let amount_out_minor = quantity_to_minor_units_u128(amount_out, xor_scale)?;
    let recipient_count = u128::try_from(binding.recipients.len())
        .map_err(|_| ValidationFeeAdmissionError::TreasuryPayoutArithmeticFailure)?;
    if recipient_count == 0 || amount_out_minor < recipient_count {
        return Ok(None);
    }
    let quotient = amount_out_minor / recipient_count;
    let remainder = amount_out_minor % recipient_count;
    let mut recipients = binding.recipients.iter().collect::<Vec<_>>();
    recipients.sort_by(|lhs, rhs| lhs.account_id.cmp(&rhs.account_id));
    recipients
        .into_iter()
        .enumerate()
        .map(|(index, recipient)| {
            let index = u128::try_from(index)
                .map_err(|_| ValidationFeeAdmissionError::TreasuryPayoutArithmeticFailure)?;
            let extra = u128::from(index < remainder);
            Ok((
                recipient.account_id.clone(),
                quantity_from_minor_units_u128(quotient + extra, xor_scale)?,
            ))
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Some)
}
fn validate_treasury_payout_effect_plan(
    instruction_groups: &std::collections::BTreeMap<AccountId, Vec<InstructionBox>>,
    ordered_instructions: &[(AccountId, InstructionBox)],
    binding: &ValidationFeeTreasuryPayoutBindingV1,
    terms: &ValidationFeePayoutTerms,
) -> Result<bool, ValidationFeeAdmissionError> {
    let mismatch =
        |reason| ValidationFeeAdmissionError::TreasuryPayoutEffectPlanMismatch { reason };
    if binding.invariant_error().is_some() {
        return Err(mismatch("the enacted payout binding is invalid"));
    }
    if instruction_groups.len() != 1 {
        return Err(mismatch(
            "the runtime must emit exactly one authority group",
        ));
    }
    let Some(instructions) = instruction_groups.get(&binding.treasury_account_id) else {
        return Err(mismatch(
            "the sole authority group must be the bound treasury",
        ));
    };
    if instructions.len() != 6 || ordered_instructions.len() != 6 {
        return Err(mismatch(
            "the atomic payout must contain exactly six instructions",
        ));
    }
    if ordered_instructions
        .iter()
        .any(|(authority, _)| authority != &binding.treasury_account_id)
    {
        return Err(mismatch(
            "every ordered effect must retain the bound treasury runtime authority",
        ));
    }
    if !ordered_instructions
        .iter()
        .map(|(_, instruction)| instruction)
        .eq(instructions.iter())
    {
        return Err(mismatch(
            "grouped effects do not preserve the canonical global queue order",
        ));
    }
    let mut collection = TransferCollection {
        contexts: vec![TransferExecutionContext {
            execution_account_id: binding.treasury_account_id.clone(),
            requires_multisig_fee_marker: false,
        }],
        transfers: Vec::new(),
        multisig_fee_markers: Vec::new(),
    };
    collect_instruction_asset_transfers(instructions, 0, &binding.ds_asset_id, &mut collection)?;
    if collection.transfers.len() != 6 {
        return Err(mismatch(
            "every instruction must be one direct non-batched asset transfer",
        ));
    }
    for (index, transfer) in collection.transfers.iter().enumerate() {
        if transfer.context_index != 0
            || transfer.instruction_index != index
            || transfer.entry_index.is_some()
        {
            return Err(mismatch(
                "transfers must retain the canonical direct instruction order",
            ));
        }
    }
    let ds_leg = &collection.transfers[0];
    if ds_leg.asset_definition_id != binding.ds_asset_id
        || ds_leg.source_account_id != binding.treasury_account_id
        || ds_leg.destination_account_id != binding.pool_vault_account_id
        || ds_leg.amount != terms.debit_ds
    {
        return Err(mismatch(
            "instruction 0 must be the exact bound DS treasury-to-vault batch",
        ));
    }
    let xor_return = &collection.transfers[1];
    if xor_return.asset_definition_id != binding.xor_asset_id
        || xor_return.source_account_id != binding.pool_vault_account_id
        || xor_return.destination_account_id != binding.treasury_account_id
    {
        return Err(mismatch(
            "instruction 1 must return XOR from the bound vault to the treasury",
        ));
    }
    let amount_out = xor_return.amount.clone();
    if amount_out.is_zero()
        || amount_out < terms.min_xor_out
        || amount_out > terms.max_xor_out
        || quantity_to_minor_units_u128(&amount_out, terms.xor_scale).is_err()
    {
        return Ok(false);
    }
    let Some(expected_payouts) =
        canonical_validation_fee_payouts(binding, &amount_out, terms.xor_scale)?
    else {
        return Ok(false);
    };
    for (offset, (recipient_account_id, expected_amount)) in expected_payouts.iter().enumerate() {
        if expected_amount.is_zero() {
            return Err(mismatch("every bound validator payout must be non-zero"));
        }
        let transfer = &collection.transfers[offset + 2];
        if transfer.asset_definition_id != binding.xor_asset_id
            || transfer.source_account_id != binding.treasury_account_id
            || transfer.destination_account_id != *recipient_account_id
            || &transfer.amount != expected_amount
        {
            return Err(mismatch(
                "instructions 2 through 5 must match the ordered validator shares exactly",
            ));
        }
    }
    Ok(true)
}
fn reject_opaque_fee_asset_effects(
    authority: &AccountId,
    instructions: &[InstructionBox],
    fee_asset_definition_id: &AssetDefinitionId,
    allowed_direct_treasury_source: Option<&AccountId>,
) -> Result<(), ValidationFeeAdmissionError> {
    let mut transfer_collection = TransferCollection {
        contexts: vec![TransferExecutionContext {
            execution_account_id: authority.clone(),
            requires_multisig_fee_marker: false,
        }],
        transfers: Vec::new(),
        multisig_fee_markers: Vec::new(),
    };
    collect_instruction_asset_transfers(
        instructions,
        0,
        fee_asset_definition_id,
        &mut transfer_collection,
    )?;
    if let Some(transfer) = transfer_collection.transfers.iter().find(|transfer| {
        if transfer.asset_definition_id != *fee_asset_definition_id {
            return false;
        }
        let allowed = allowed_direct_treasury_source.is_some_and(|treasury| {
            transfer.context_index == 0
                && transfer.source_account_id == *treasury
                && transfer_collection.contexts[transfer.context_index].execution_account_id
                    == *treasury
        });
        !allowed
    }) {
        let execution_account_id = transfer_collection.contexts[transfer.context_index]
            .execution_account_id
            .to_string();
        return Err(
            ValidationFeeAdmissionError::OpaqueDeferredFeeAssetTransfer {
                execution_account_id,
                instruction_index: transfer.instruction_index,
                entry_index: transfer.entry_index,
            },
        );
    }
    for instruction in instructions {
        if let Ok(MultisigInstructionBox::Propose(propose)) =
            MultisigInstructionBox::try_from(instruction)
        {
            reject_opaque_fee_asset_effects(
                &propose.account,
                &propose.instructions,
                fee_asset_definition_id,
                None,
            )?;
        }
        let Some(RegisterBox::Trigger(register)) =
            instruction.as_any().downcast_ref::<RegisterBox>()
        else {
            continue;
        };
        let action = register.object.action();
        match action.executable() {
            Executable::Instructions(instructions) => reject_opaque_fee_asset_effects(
                action.authority(),
                instructions,
                fee_asset_definition_id,
                None,
            )?,
            Executable::IvmProved(proved) => reject_opaque_fee_asset_effects(
                action.authority(),
                &proved.overlay,
                fee_asset_definition_id,
                None,
            )?,
            Executable::Batch(items) => {
                let nested_instructions = items
                    .iter()
                    .filter_map(|item| match item {
                        ExecutableBatchItem::Instruction(instruction) => Some(instruction.clone()),
                        ExecutableBatchItem::ContractCall(_) => None,
                    })
                    .collect::<Vec<_>>();
                reject_opaque_fee_asset_effects(
                    action.authority(),
                    &nested_instructions,
                    fee_asset_definition_id,
                    None,
                )?;
            }
            Executable::ContractCall(_) | Executable::Ivm(_) => {}
        }
    }
    Ok(())
}
const MAX_OPAQUE_DEFERRED_PROPOSAL_DEPTH: usize = 64;
fn reject_opaque_deferred_approval_effects_with<F>(
    _authority: &AccountId,
    instructions: &[InstructionBox],
    fee_asset_definition_id: &AssetDefinitionId,
    visited_proposals: &mut std::collections::BTreeSet<String>,
    depth: usize,
    resolve: &mut F,
) -> Result<(), ValidationFeeAdmissionError>
where
    F: FnMut(
        &iroha_executor_data_model::isi::multisig::MultisigApprove,
    ) -> Option<(AccountId, Vec<InstructionBox>)>,
{
    if depth > MAX_OPAQUE_DEFERRED_PROPOSAL_DEPTH {
        return Err(ValidationFeeAdmissionError::OpaqueDeferredProposalDepthExceeded);
    }
    for instruction in instructions {
        if let Ok(multisig) = MultisigInstructionBox::try_from(instruction) {
            match multisig {
                MultisigInstructionBox::Propose(propose) => {
                    reject_opaque_deferred_approval_effects_with(
                        &propose.account,
                        &propose.instructions,
                        fee_asset_definition_id,
                        visited_proposals,
                        depth + 1,
                        resolve,
                    )?;
                }
                MultisigInstructionBox::Approve(approve) => {
                    let Some((proposal_authority, proposal_instructions)) = resolve(&approve)
                    else {
                        return Err(
                            ValidationFeeAdmissionError::UnresolvedOpaqueDeferredMultisigApproval {
                                account_id: approve.account.to_string(),
                                instructions_hash_hex: hex::encode(
                                    approve.instructions_hash.as_ref(),
                                ),
                            },
                        );
                    };
                    let proposal_key = format!(
                        "{}:{}",
                        proposal_authority,
                        hex::encode(approve.instructions_hash.as_ref())
                    );
                    if !visited_proposals.insert(proposal_key) {
                        continue;
                    }
                    reject_opaque_fee_asset_effects(
                        &proposal_authority,
                        &proposal_instructions,
                        fee_asset_definition_id,
                        None,
                    )?;
                    reject_opaque_deferred_approval_effects_with(
                        &proposal_authority,
                        &proposal_instructions,
                        fee_asset_definition_id,
                        visited_proposals,
                        depth + 1,
                        resolve,
                    )?;
                }
                MultisigInstructionBox::Register(_)
                | MultisigInstructionBox::Cancel(_)
                | MultisigInstructionBox::InvalidateOutstanding(_) => {}
            }
        }
        let Some(RegisterBox::Trigger(register)) =
            instruction.as_any().downcast_ref::<RegisterBox>()
        else {
            continue;
        };
        let action = register.object.action();
        match action.executable() {
            Executable::Instructions(instructions) => {
                reject_opaque_deferred_approval_effects_with(
                    action.authority(),
                    instructions,
                    fee_asset_definition_id,
                    visited_proposals,
                    depth + 1,
                    resolve,
                )?;
            }
            Executable::IvmProved(proved) => {
                reject_opaque_deferred_approval_effects_with(
                    action.authority(),
                    &proved.overlay,
                    fee_asset_definition_id,
                    visited_proposals,
                    depth + 1,
                    resolve,
                )?;
            }
            Executable::Batch(items) => {
                let nested_instructions = items
                    .iter()
                    .filter_map(|item| match item {
                        ExecutableBatchItem::Instruction(instruction) => Some(instruction.clone()),
                        ExecutableBatchItem::ContractCall(_) => None,
                    })
                    .collect::<Vec<_>>();
                reject_opaque_deferred_approval_effects_with(
                    action.authority(),
                    &nested_instructions,
                    fee_asset_definition_id,
                    visited_proposals,
                    depth + 1,
                    resolve,
                )?;
            }
            Executable::ContractCall(_) | Executable::Ivm(_) => {}
        }
    }
    Ok(())
}
#[cfg(test)]
fn no_hijiri_account_risk(
    _account_id: &AccountId,
) -> Result<Option<HijiriAccountRiskV1>, ValidationFeeAdmissionError> {
    Ok(None)
}

#[cfg(test)]
fn enforce_deferred_policy(
    authority: &AccountId,
    instructions: &[InstructionBox],
    policy: &ValidationFeePolicyV1,
) -> Result<(), ValidationFeeAdmissionError> {
    enforce_deferred_policy_with_credit(authority, instructions, policy).map(|_| ())
}
#[cfg(test)]
fn enforce_deferred_policy_with_credit(
    authority: &AccountId,
    instructions: &[InstructionBox],
    policy: &ValidationFeePolicyV1,
) -> Result<u64, ValidationFeeAdmissionError> {
    enforce_deferred_policy_with_credit_and_hijiri(
        authority,
        instructions,
        policy,
        None,
        &no_hijiri_account_risk,
    )
}
fn enforce_deferred_policy_with_credit_and_hijiri(
    authority: &AccountId,
    instructions: &[InstructionBox],
    policy: &ValidationFeePolicyV1,
    hijiri: Option<&HijiriParametersV1>,
    resolve_account_risk: &dyn Fn(
        &AccountId,
    ) -> Result<
        Option<HijiriAccountRiskV1>,
        ValidationFeeAdmissionError,
    >,
) -> Result<u64, ValidationFeeAdmissionError> {
    let fee_asset_definition_id = policy_fee_asset_definition_id(policy)?;
    let treasury = policy_treasury_account_id(policy)?;
    let mut transfer_collection = TransferCollection {
        contexts: vec![TransferExecutionContext {
            execution_account_id: authority.clone(),
            requires_multisig_fee_marker: true,
        }],
        transfers: Vec::new(),
        multisig_fee_markers: Vec::new(),
    };
    collect_instruction_asset_transfers(
        instructions,
        0,
        &fee_asset_definition_id,
        &mut transfer_collection,
    )?;
    let fee_asset_transfers = collect_fee_asset_transfers(
        &transfer_collection.transfers,
        policy,
        &fee_asset_definition_id,
    )?;
    let mut credited_minor_units = 0_u64;
    for (context_index, context) in transfer_collection.contexts.iter().enumerate() {
        let resolved_hijiri =
            resolve_hijiri_fee(hijiri, &context.execution_account_id, resolve_account_risk)?;
        let marker_fee_coordinate = multisig_marker_coordinate_for_context(
            context_index,
            context,
            policy,
            resolved_hijiri.map(|resolved| resolved.quote_hash),
            &transfer_collection.multisig_fee_markers,
            &fee_asset_transfers,
        )?;
        let validated = enforce_context_policy(
            context_index,
            &context.execution_account_id,
            &treasury,
            policy,
            &fee_asset_definition_id,
            &transfer_collection.transfers,
            &fee_asset_transfers,
            marker_fee_coordinate,
            false,
            resolved_hijiri.map(|resolved| resolved.multiplier),
        )?;
        if context_index == 0 {
            credited_minor_units = validated.credited_minor_units;
        }
    }
    Ok(credited_minor_units)
}
fn active_policy(
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<Option<ValidationFeePolicyV1>, TransactionRejectionReason> {
    let registry = validated_policy_registry(state_transaction)?;
    active_policy_from_validated_registry(registry.as_ref(), state_transaction)
}

fn active_hijiri_parameters(
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<Option<HijiriParametersV1>, TransactionRejectionReason> {
    let parameter_id = HijiriParametersV1::parameter_id();
    let Some(custom) = state_transaction
        .world
        .parameters()
        .custom()
        .get(&parameter_id)
    else {
        return Ok(None);
    };
    match HijiriParametersV1::from_custom_parameter(custom) {
        Ok(Some(parameters)) => Ok(Some(parameters)),
        Ok(None) => Err(admission_rejection(
            ValidationFeeAdmissionError::MalformedHijiriParameters,
        )),
        Err(iroha_data_model::hijiri::HijiriParametersError::MalformedPayload) => Err(
            admission_rejection(ValidationFeeAdmissionError::MalformedHijiriParameters),
        ),
        Err(error) => Err(admission_rejection(
            ValidationFeeAdmissionError::InvalidHijiriParameters(error.to_string()),
        )),
    }
}

fn active_hijiri_account_risk(
    state_transaction: &StateTransaction<'_, '_>,
    account_id: &AccountId,
) -> Result<Option<HijiriAccountRiskV1>, ValidationFeeAdmissionError> {
    let parameter_id = HijiriAccountRiskV1::parameter_id_for(account_id)
        .map_err(|error| ValidationFeeAdmissionError::InvalidHijiriParameters(error.to_string()))?;
    let Some(custom) = state_transaction
        .world
        .parameters()
        .custom()
        .get(&parameter_id)
    else {
        return Ok(None);
    };
    HijiriAccountRiskV1::from_custom_parameter(custom)
        .map_err(|error| ValidationFeeAdmissionError::InvalidHijiriParameters(error.to_string()))?
        .ok_or_else(|| {
            ValidationFeeAdmissionError::InvalidHijiriParameters(
                "account-risk record changed its reserved identity".to_owned(),
            )
        })
        .map(Some)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ResolvedHijiriFee {
    multiplier: Q16,
    quote_hash: [u8; 32],
}

fn resolve_hijiri_fee(
    parameters: Option<&HijiriParametersV1>,
    account_id: &AccountId,
    resolve_account_risk: &dyn Fn(
        &AccountId,
    ) -> Result<
        Option<HijiriAccountRiskV1>,
        ValidationFeeAdmissionError,
    >,
) -> Result<Option<ResolvedHijiriFee>, ValidationFeeAdmissionError> {
    let Some(parameters) = parameters else {
        return Ok(None);
    };
    let account_risk = resolve_account_risk(account_id)?;
    let multiplier = parameters
        .multiplier_for(account_id, account_risk.as_ref())
        .map_err(|error| ValidationFeeAdmissionError::InvalidHijiriParameters(error.to_string()))?;
    let quote_hash = parameters
        .fee_quote_hash(account_id, account_risk.as_ref())
        .map_err(|error| ValidationFeeAdmissionError::InvalidHijiriParameters(error.to_string()))?;
    Ok(Some(ResolvedHijiriFee {
        multiplier,
        quote_hash,
    }))
}

fn validated_policy_registry(
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<Option<ValidationFeePolicyRegistryV1>, TransactionRejectionReason> {
    validated_policy_registry_in_world(&state_transaction.world).map_err(admission_rejection)
}

fn validated_policy_registry_in_world<W: WorldReadOnly + ?Sized>(
    world: &W,
) -> Result<Option<ValidationFeePolicyRegistryV1>, ValidationFeeAdmissionError> {
    let parameter_id = ValidationFeePolicyRegistryV1::parameter_id();
    let Some(custom) = world.parameters().custom().get(&parameter_id) else {
        return Ok(None);
    };
    let Some(registry) = ValidationFeePolicyRegistryV1::from_custom_parameter(custom) else {
        return Err(ValidationFeeAdmissionError::MalformedPolicyRegistryParameter);
    };
    registry
        .validate()
        .map_err(|err| ValidationFeeAdmissionError::InvalidPolicyRegistry(err.to_string()))?;
    for entry in &registry.registered_policies {
        validate_registry_entry_governance(entry, world)?;
    }
    Ok(Some(registry))
}

/// Validate the protected validation-fee registry against every retained governance store.
///
/// Snapshot restore calls this before exposing the reconstructed world so an intrinsically valid
/// registry cannot strand admission behind missing or mismatched proposal and Parliament records.
pub(crate) fn validate_persisted_policy_registry_governance_v1<W: WorldReadOnly + ?Sized>(
    world: &W,
) -> Result<(), String> {
    validated_policy_registry_in_world(world)
        .map(drop)
        .map_err(|error| error.to_string())
}

/// Validate the restored registry against the exact network and active payout runtime.
///
/// This runs only after snapshot construction provides a complete [`StateReadOnly`] view, but
/// before that state is published or allowed to recover durable journals.
pub(crate) fn validate_persisted_policy_registry_runtime_v1(
    state: &impl StateReadOnly,
    restored_height: u64,
) -> Result<(), String> {
    let Some(registry) =
        validated_policy_registry_in_world(state.world()).map_err(|error| error.to_string())?
    else {
        return Ok(());
    };
    for entry in &registry.registered_policies {
        validate_policy_network_id(&entry.policy, state.network_id())
            .map_err(|error| error.to_string())?;
    }
    let Some(entry) = registry.effective_entry_at_height(restored_height) else {
        return Ok(());
    };
    if entry.policy.charging_mode == ValidationFeeChargingMode::Disabled {
        return Ok(());
    }
    validate_treasury_payout_contract_subject(&entry.policy, state)
        .map_err(|error| error.to_string())
}

fn active_policy_from_validated_registry(
    registry: Option<&ValidationFeePolicyRegistryV1>,
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<Option<ValidationFeePolicyV1>, TransactionRejectionReason> {
    let Some(registry) = registry else {
        return Ok(None);
    };
    let current_height = state_transaction.block_height();
    let Some(entry) = registry.scheduled_entry_at_height(current_height) else {
        // The initial Parliament policy is deliberately enacted well before
        // activation so downstreams can pin its finalized proof. Until that
        // first effective height arrives there is no active validation fee.
        return Ok(None);
    };
    let policy = entry.policy.clone();
    if let Some(reason) = policy.policy_invariant_error() {
        return Err(admission_rejection(
            ValidationFeeAdmissionError::InvalidPolicyInvariant(reason),
        ));
    }
    validate_policy_network_id(&policy, &state_transaction.network_id)
        .map_err(admission_rejection)?;
    if let Some(expires_after_height) = policy.expires_after_height {
        if current_height >= expires_after_height {
            return Err(admission_rejection(
                ValidationFeeAdmissionError::PolicyExpired {
                    expires_after_height,
                    current_height,
                },
            ));
        }
    }
    if policy.charging_mode == ValidationFeeChargingMode::Disabled {
        return Ok(None);
    }
    validate_treasury_payout_contract_subject(&policy, state_transaction)
        .map_err(admission_rejection)?;
    Ok(Some(policy))
}
fn validate_registry_entry_governance<W: WorldReadOnly + ?Sized>(
    entry: &ValidationFeePolicyRegistryEntryV1,
    world: &W,
) -> Result<(), ValidationFeeAdmissionError> {
    use iroha_data_model::governance::types::{
        ProposalKind, ValidationFeePayoutLifecycleProposal, ValidationFeePolicyProposal,
    };
    let lifecycle_id = entry
        .payout_lifecycle
        .as_ref()
        .map(|reference| reference.parliament_authorization.proposal_fingerprint);
    let policy_kind = ProposalKind::ValidationFeePolicy(ValidationFeePolicyProposal {
        proposal_operator: entry.parliament_authorization.proposal_operator.clone(),
        policy: entry.policy.clone(),
        payout_lifecycle_proposal_id: lifecycle_id,
    });
    validate_parliament_authorization(&entry.parliament_authorization, &policy_kind, world)?;
    match (
        entry.policy.treasury_payout_binding.as_ref(),
        entry.payout_lifecycle.as_ref(),
    ) {
        (None, None) => Ok(()),
        (Some(binding), Some(reference)) => {
            let lifecycle_kind =
                ProposalKind::ValidationFeePayoutLifecycle(ValidationFeePayoutLifecycleProposal {
                    proposal_operator: reference.parliament_authorization.proposal_operator.clone(),
                    payout_binding: binding.clone(),
                });
            validate_parliament_authorization(
                &reference.parliament_authorization,
                &lifecycle_kind,
                world,
            )
        }
        _ => Err(ValidationFeeAdmissionError::InvalidPolicyRegistry(
            "enabled policy requires paired payout binding and Parliament lifecycle authorization"
                .to_owned(),
        )),
    }
}
fn validate_parliament_authorization<W: WorldReadOnly + ?Sized>(
    authorization: &ValidationFeeParliamentAuthorizationV1,
    exact_kind: &iroha_data_model::governance::types::ProposalKind,
    world: &W,
) -> Result<(), ValidationFeeAdmissionError> {
    if let Some(reason) = authorization.invariant_error() {
        return Err(ValidationFeeAdmissionError::InvalidPolicyRegistry(
            reason.to_owned(),
        ));
    }
    let fingerprint = exact_kind.fingerprint();
    if fingerprint != authorization.proposal_fingerprint {
        return Err(ValidationFeeAdmissionError::InvalidPolicyRegistry(
            "stored proposal fingerprint does not match the exact typed proposal preimage"
                .to_owned(),
        ));
    }
    let exact_operator = match exact_kind {
        iroha_data_model::governance::types::ProposalKind::ValidationFeePolicy(payload) => {
            &payload.proposal_operator
        }
        iroha_data_model::governance::types::ProposalKind::ValidationFeePayoutLifecycle(
            payload,
        ) => &payload.proposal_operator,
        _ => {
            return Err(ValidationFeeAdmissionError::InvalidPolicyRegistry(
                "validation-fee authorization received a non-validation-fee proposal".to_owned(),
            ));
        }
    };
    if exact_operator != &authorization.proposal_operator {
        return Err(ValidationFeeAdmissionError::InvalidPolicyRegistry(
            "stored proposal operator does not match the exact typed proposal preimage".to_owned(),
        ));
    }
    let certificate = &authorization.governance_certificate;
    let governed_subject = exact_kind.governed_subject_id_v1().map_err(|_| {
        ValidationFeeAdmissionError::InvalidPolicyRegistry(
            "failed to derive the exact validation-fee governed subject".to_owned(),
        )
    })?;
    let certificate_subject = match certificate.expected_head {
        iroha_data_model::governance::types::GovernanceExpectedHeadV1::Absent(head) => {
            head.subject_id
        }
        iroha_data_model::governance::types::GovernanceExpectedHeadV1::Present(head) => {
            head.subject_id
        }
    };
    if certificate.effect_preimage_hash != exact_kind.effect_preimage_hash_v1()
        || certificate_subject != governed_subject
    {
        return Err(ValidationFeeAdmissionError::InvalidPolicyRegistry(
            "stored Parliament certificate is not bound to the exact governed effect and subject"
                .to_owned(),
        ));
    }
    let proposal = world
        .governance_proposals()
        .get(&authorization.proposal_fingerprint)
        .ok_or_else(|| {
            ValidationFeeAdmissionError::InvalidPolicyRegistry(
                "authorized governance proposal is missing".to_owned(),
            )
        })?;
    if &proposal.kind != exact_kind
        || proposal.proposer != authorization.proposal_operator
        || proposal.status != crate::state::GovernanceProposalStatus::Enacted
    {
        return Err(ValidationFeeAdmissionError::InvalidPolicyRegistry(
            "authorized governance proposal payload or status differs from the registry".to_owned(),
        ));
    }
    let attempt = world
        .parliament_attempts()
        .get(&certificate.governance_attempt_id)
        .ok_or_else(|| {
            ValidationFeeAdmissionError::InvalidPolicyRegistry(
                "authorized Parliament attempt is missing".to_owned(),
            )
        })?;
    attempt.validate().map_err(|error| {
        ValidationFeeAdmissionError::InvalidPolicyRegistry(format!(
            "authorized Parliament attempt is invalid: {error}"
        ))
    })?;
    attempt
        .validate_proposal_bindings_v1(exact_kind)
        .map_err(|error| {
            ValidationFeeAdmissionError::InvalidPolicyRegistry(format!(
                "authorized Parliament attempt proposal bindings are invalid: {error}"
            ))
        })?;
    if attempt.proposal_content_id() != certificate.proposal_content_id
        || attempt.attempt().status
            != iroha_data_model::governance::types::GovernanceAttemptStatusV1::Enacted
        || attempt.terminal_height() != Some(authorization.enacted_at_height)
        || attempt.certificate() != Some(certificate)
    {
        return Err(ValidationFeeAdmissionError::InvalidPolicyRegistry(
            "authorized Parliament attempt does not retain the exact enacted certificate"
                .to_owned(),
        ));
    }
    Ok(())
}
fn validate_treasury_payout_contract_subject(
    policy: &ValidationFeePolicyV1,
    state: &impl StateReadOnly,
) -> Result<(), ValidationFeeAdmissionError> {
    if !treasury_payout_exemption_enabled(policy) {
        return Ok(());
    }
    let treasury = policy_treasury_account_id(policy)?;
    let binding = policy.treasury_payout_binding.as_ref().ok_or(
        ValidationFeeAdmissionError::TreasuryPayoutRuntimeBindingMismatch {
            reason: "the active Parliament policy has no typed payout binding",
        },
    )?;
    if binding.treasury_account_id != treasury {
        return Err(
            ValidationFeeAdmissionError::TreasuryPayoutRuntimeBindingMismatch {
                reason: "the payout binding treasury differs from the policy treasury",
            },
        );
    }
    validate_treasury_payout_binding_contract_subject(binding, policy.ds_scale, state)
}

fn validate_treasury_payout_binding_contract_subject(
    binding: &ValidationFeeTreasuryPayoutBindingV1,
    ds_scale: u8,
    state: &impl StateReadOnly,
) -> Result<(), ValidationFeeAdmissionError> {
    let Some(record) =
        crate::smartcontracts::code::fetch_bound_contract_record(state, &binding.contract_address)
    else {
        return Err(
            ValidationFeeAdmissionError::TreasuryPayoutRequiresActiveContractSubject {
                treasury_account_id: binding.treasury_account_id.to_string(),
            },
        );
    };
    if record.contract_address != binding.contract_address
        || record.contract_subject != binding.treasury_account_id
    {
        return Err(
            ValidationFeeAdmissionError::TreasuryPayoutRuntimeBindingMismatch {
                reason: "contract address or immutable subject differs from the enacted binding",
            },
        );
    }
    if <[u8; 32]>::from(Sha256::digest(&record.code_bytes)) != binding.code_hash {
        return Err(
            ValidationFeeAdmissionError::TreasuryPayoutRuntimeBindingMismatch {
                reason: "deployed code hash differs from the enacted binding",
            },
        );
    }
    let entrypoint = binding.entrypoint.as_ref();
    if !record
        .manifest
        .entrypoints
        .as_ref()
        .is_some_and(|entrypoints| entrypoints.iter().any(|item| item.name == entrypoint))
    {
        return Err(
            ValidationFeeAdmissionError::TreasuryPayoutRuntimeBindingMismatch {
                reason: "the enacted entrypoint is absent from the deployed manifest",
            },
        );
    }
    let lifecycle_seal = binding
        .lifecycle_seal()
        .map_err(|_| ValidationFeeAdmissionError::InvalidPayoutLifecycleSeal)?;
    if lifecycle_seal == [0; 32] {
        return Err(ValidationFeeAdmissionError::InvalidPayoutLifecycleSeal);
    }
    let ds_credit = ValidationFeeCredit {
        treasury_account_id: binding.treasury_account_id.clone(),
        lifecycle_seal,
        fee_asset_definition_id: binding.ds_asset_id.clone(),
        asset_scale: ds_scale,
        amount: binding.batch_ds.clone(),
    };
    validation_fee_credit_asset_spec(state, &ds_credit)?;
    let xor_definition = state
        .world()
        .asset_definition(&binding.xor_asset_id)
        .map_err(
            |_| ValidationFeeAdmissionError::TreasuryPayoutRuntimeBindingMismatch {
                reason: "the bound XOR asset definition is missing",
            },
        )?;
    if xor_definition.spec().scale().is_none() {
        return Err(
            ValidationFeeAdmissionError::TreasuryPayoutRuntimeBindingMismatch {
                reason: "the bound XOR asset must have a fixed minor-unit scale",
            },
        );
    }
    if xor_definition
        .spec()
        .check(binding.min_xor_out.as_numeric())
        .is_err()
        || xor_definition
            .spec()
            .check(binding.max_xor_out.as_numeric())
            .is_err()
    {
        return Err(
            ValidationFeeAdmissionError::TreasuryPayoutRuntimeBindingMismatch {
                reason: "the enacted XOR bounds do not satisfy the bound asset numeric specification",
            },
        );
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResolvedPayoutLifecycle {
    lifecycle_seal: [u8; 32],
    binding: ValidationFeeTreasuryPayoutBindingV1,
    ds_scale: u8,
    policy_hash: [u8; 32],
}

fn scheduled_trigger_claims_payout_binding(
    binding: &ValidationFeeTreasuryPayoutBindingV1,
    state_transaction: &StateTransaction<'_, '_>,
    origin: &OpaqueDeferredRuntimeOrigin<'_>,
) -> bool {
    let Some(trigger_id) = origin.trigger_id else {
        return false;
    };
    let Some(action) = state_transaction
        .world
        .triggers
        .time_triggers()
        .get(trigger_id)
    else {
        return false;
    };
    let ExecutableRef::ContractCall(invocation) = action.executable() else {
        return false;
    };
    let Some(active_code_hash) = state_transaction
        .world
        .contract_instances
        .get(&binding.contract_address)
    else {
        return false;
    };
    origin.scheduled_time_trigger
        && trigger_is_enabled(action.metadata())
        && action.authority() == &binding.treasury_account_id
        && invocation.contract_address == binding.contract_address
        && invocation.expected_code_hash == *active_code_hash
        && invocation.entrypoint == binding.entrypoint.as_ref()
        && invocation.arguments.is_none()
}

fn runtime_origin_matches_payout_binding(
    binding: &ValidationFeeTreasuryPayoutBindingV1,
    state_transaction: &StateTransaction<'_, '_>,
    origin: &OpaqueDeferredRuntimeOrigin<'_>,
) -> bool {
    if !runtime_origin_claims_payout_binding(binding, origin) {
        return false;
    }
    crate::smartcontracts::code::fetch_bound_contract_record(
        state_transaction,
        &binding.contract_address,
    )
    .is_some_and(|record| {
        record.contract_address == binding.contract_address
            && record.contract_subject == binding.treasury_account_id
            && <[u8; 32]>::from(Sha256::digest(&record.code_bytes)) == binding.code_hash
            && record.code_bytes.as_slice() == origin.code_bytes
    })
}

fn runtime_origin_claims_payout_binding(
    binding: &ValidationFeeTreasuryPayoutBindingV1,
    origin: &OpaqueDeferredRuntimeOrigin<'_>,
) -> bool {
    !(origin.runtime_context.contract_address != binding.contract_address
        || origin.runtime_context.contract_subject != binding.treasury_account_id
        || origin.runtime_context.entrypoint != binding.entrypoint.as_ref()
        || <[u8; 32]>::from(Sha256::digest(origin.code_bytes)) != binding.code_hash)
}

fn resolve_retained_payout_lifecycle(
    registry: Option<&ValidationFeePolicyRegistryV1>,
    state_transaction: &StateTransaction<'_, '_>,
    runtime_origin: Option<&OpaqueDeferredRuntimeOrigin<'_>>,
) -> Result<Option<ResolvedPayoutLifecycle>, ValidationFeeAdmissionError> {
    let (Some(registry), Some(origin)) = (registry, runtime_origin) else {
        return Ok(None);
    };
    let mut retained_identity_claimed = false;
    let mut resolved: Option<ResolvedPayoutLifecycle> = None;
    let current_height = state_transaction.block_height();
    for entry in &registry.registered_policies {
        // An enacted policy scheduled for a later height has never owned a fee-credit lifecycle
        // at the current state boundary. It must neither authorize its runtime early nor make an
        // already-effective predecessor ambiguous before the scheduled cutover.
        if entry.policy.effective_from_height > current_height {
            continue;
        }
        let (Some(binding), Some(reference)) = (
            entry.policy.treasury_payout_binding.as_ref(),
            entry.payout_lifecycle.as_ref(),
        ) else {
            continue;
        };
        let trigger_matches =
            scheduled_trigger_claims_payout_binding(binding, state_transaction, origin);
        let runtime_claims = runtime_origin_claims_payout_binding(binding, origin);
        if !trigger_matches && !runtime_claims {
            continue;
        }
        retained_identity_claimed = true;
        if !trigger_matches
            || !runtime_origin_matches_payout_binding(binding, state_transaction, origin)
        {
            continue;
        }
        validate_treasury_payout_binding_contract_subject(
            binding,
            entry.policy.ds_scale,
            state_transaction,
        )?;
        let candidate = ResolvedPayoutLifecycle {
            lifecycle_seal: reference.lifecycle_seal,
            binding: binding.clone(),
            ds_scale: entry.policy.ds_scale,
            policy_hash: entry.policy_hash,
        };
        if let Some(existing) = resolved.as_ref() {
            if existing.lifecycle_seal == candidate.lifecycle_seal
                && existing.binding == candidate.binding
                && existing.ds_scale == candidate.ds_scale
            {
                continue;
            }
            return Err(
                ValidationFeeAdmissionError::AmbiguousTreasuryPayoutRuntimeIdentity {
                    first_policy_hash_hex: hex::encode(existing.policy_hash),
                    second_policy_hash_hex: hex::encode(candidate.policy_hash),
                },
            );
        }
        resolved = Some(candidate);
    }
    if retained_identity_claimed && resolved.is_none() {
        return Err(
            ValidationFeeAdmissionError::TreasuryPayoutRuntimeBindingMismatch {
                reason: "the scheduled trigger or executed runtime identity matches a retained payout lifecycle but the pair is not exact",
            },
        );
    }
    Ok(resolved)
}
fn validate_policy_network_id(
    policy: &ValidationFeePolicyV1,
    expected_network_id: &iroha_data_model::NetworkId,
) -> Result<(), ValidationFeeAdmissionError> {
    if &policy.network_id != expected_network_id {
        return Err(ValidationFeeAdmissionError::WrongPolicyNetwork {
            expected: expected_network_id.to_string(),
            found: policy.network_id.to_string(),
        });
    }
    Ok(())
}
fn admission_rejection(error: ValidationFeeAdmissionError) -> TransactionRejectionReason {
    TransactionRejectionReason::Validation(ValidationFail::NotPermitted(format!(
        "validation-fee admission rejected transaction: {error}"
    )))
}
fn policy_fee_asset_definition_id(
    policy: &ValidationFeePolicyV1,
) -> Result<AssetDefinitionId, ValidationFeeAdmissionError> {
    Ok(policy.ds_asset_id.clone())
}
fn policy_treasury_account_id(
    policy: &ValidationFeePolicyV1,
) -> Result<AccountId, ValidationFeeAdmissionError> {
    Ok(policy.treasury_account_id.clone())
}
fn policy_payout_lifecycle_seal(
    policy: &ValidationFeePolicyV1,
) -> Result<[u8; 32], ValidationFeeAdmissionError> {
    let binding = policy.treasury_payout_binding.as_ref().ok_or(
        ValidationFeeAdmissionError::TreasuryPayoutRuntimeBindingMismatch {
            reason: "the active Parliament policy has no typed payout binding",
        },
    )?;
    let seal = binding
        .lifecycle_seal()
        .map_err(|_| ValidationFeeAdmissionError::InvalidPayoutLifecycleSeal)?;
    if seal == [0; 32] {
        return Err(ValidationFeeAdmissionError::InvalidPayoutLifecycleSeal);
    }
    Ok(seal)
}
/// Derive the exact IVM durable-state path used for the consensus-owned fee-credit counter.
pub(crate) fn validation_fee_credit_state_key_for_address(
    contract_address: &iroha_data_model::smart_contract::ContractAddress,
) -> StatePath {
    validation_fee_credit_scoped_state_key_for_address(
        contract_address,
        VALIDATION_FEE_CREDIT_STATE_LEAF,
    )
}
fn validation_fee_credit_asset_state_key_for_address(
    contract_address: &iroha_data_model::smart_contract::ContractAddress,
) -> StatePath {
    validation_fee_credit_scoped_state_key_for_address(
        contract_address,
        VALIDATION_FEE_CREDIT_ASSET_STATE_LEAF,
    )
}
fn validation_fee_credit_lifecycle_seal_state_key_for_address(
    contract_address: &iroha_data_model::smart_contract::ContractAddress,
) -> StatePath {
    validation_fee_credit_scoped_state_key_for_address(
        contract_address,
        VALIDATION_FEE_CREDIT_LIFECYCLE_SEAL_STATE_LEAF,
    )
}
fn validation_fee_credit_lifecycle_state_key_for_address(
    contract_address: &iroha_data_model::smart_contract::ContractAddress,
    lifecycle_seal: [u8; 32],
    leaf: &str,
) -> StatePath {
    let digest = hex::encode(Hash::new(contract_address.to_string().as_bytes()).as_ref());
    let lifecycle_seal = hex::encode(lifecycle_seal);
    format!("sc/{digest}/{VALIDATION_FEE_CREDIT_LIFECYCLE_STATE_PREFIX}/{lifecycle_seal}/{leaf}")
        .parse()
        .expect("validation-fee lifecycle credit path must be a valid StatePath")
}
fn validation_fee_credit_scoped_state_key_for_address(
    contract_address: &iroha_data_model::smart_contract::ContractAddress,
    leaf: &str,
) -> StatePath {
    let digest = hex::encode(Hash::new(contract_address.to_string().as_bytes()).as_ref());
    format!("sc/{digest}/{leaf}")
        .parse()
        .expect("validation-fee credit path must be a valid StatePath")
}
/// Return whether a durable-state key is the reserved contract-visible fee-credit leaf.
pub(crate) fn is_validation_fee_credit_state_key(key: &StatePath) -> bool {
    let Some(rest) = key.as_ref().strip_prefix("sc/") else {
        return false;
    };
    let mut segments = rest.split('/');
    let Some(digest) = segments.next() else {
        return false;
    };
    if digest.len() != Hash::LENGTH * 2 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return false;
    }
    let Some(second) = segments.next() else {
        return false;
    };
    let is_reserved_leaf = |leaf: &str| {
        leaf == VALIDATION_FEE_CREDIT_STATE_LEAF
            || leaf == VALIDATION_FEE_CREDIT_ASSET_STATE_LEAF
            || leaf == VALIDATION_FEE_CREDIT_LIFECYCLE_SEAL_STATE_LEAF
    };
    if is_reserved_leaf(second) {
        return segments.next().is_none();
    }
    if second != VALIDATION_FEE_CREDIT_LIFECYCLE_STATE_PREFIX {
        return false;
    }
    let Some(seal) = segments.next() else {
        return false;
    };
    let Some(leaf) = segments.next() else {
        return false;
    };
    seal.len() == Hash::LENGTH * 2
        && seal.bytes().all(|byte| byte.is_ascii_hexdigit())
        && (leaf == VALIDATION_FEE_CREDIT_STATE_LEAF
            || leaf == VALIDATION_FEE_CREDIT_ASSET_STATE_LEAF)
        && segments.next().is_none()
}
fn validation_fee_credit_state_keys(
    state_transaction: &StateTransaction<'_, '_>,
    credit: &ValidationFeeCredit,
) -> Result<(StatePath, StatePath), ValidationFeeAdmissionError> {
    let Some(record) = crate::smartcontracts::code::fetch_bound_contract_record_by_subject(
        state_transaction,
        &credit.treasury_account_id,
    ) else {
        return Err(
            ValidationFeeAdmissionError::TreasuryPayoutRequiresActiveContractSubject {
                treasury_account_id: credit.treasury_account_id.to_string(),
            },
        );
    };
    Ok((
        validation_fee_credit_lifecycle_state_key_for_address(
            &record.contract_address,
            credit.lifecycle_seal,
            VALIDATION_FEE_CREDIT_STATE_LEAF,
        ),
        validation_fee_credit_lifecycle_state_key_for_address(
            &record.contract_address,
            credit.lifecycle_seal,
            VALIDATION_FEE_CREDIT_ASSET_STATE_LEAF,
        ),
    ))
}
fn validation_fee_credit_projection_state_keys(
    state_transaction: &StateTransaction<'_, '_>,
    credit: &ValidationFeeCredit,
) -> Result<(StatePath, StatePath, StatePath), ValidationFeeAdmissionError> {
    let Some(record) = crate::smartcontracts::code::fetch_bound_contract_record_by_subject(
        state_transaction,
        &credit.treasury_account_id,
    ) else {
        return Err(
            ValidationFeeAdmissionError::TreasuryPayoutRequiresActiveContractSubject {
                treasury_account_id: credit.treasury_account_id.to_string(),
            },
        );
    };
    Ok((
        validation_fee_credit_state_key_for_address(&record.contract_address),
        validation_fee_credit_asset_state_key_for_address(&record.contract_address),
        validation_fee_credit_lifecycle_seal_state_key_for_address(&record.contract_address),
    ))
}
fn validation_fee_credit_state_schema() -> StateValueSchemaV1 {
    StateValueSchemaV1 {
        nodes: vec![StateValueNodeV1::Leaf(StateValueKindV1::Quantity)],
    }
}
/// Encode the canonical schema-bound durable value for the validation-fee credit leaf.
///
/// # Errors
///
/// Returns an IVM encoding error if the quantity or schema-bound record cannot be encoded.
pub(crate) fn encode_validation_fee_credit_state_value(
    value: &Quantity,
) -> Result<Vec<u8>, ivm::VMError> {
    let schema_payload = norito::to_bytes(&validation_fee_credit_state_schema())
        .map_err(|_| ivm::VMError::NoritoInvalid)?;
    let envelope = ivm::numeric_tlv::encode_quantity(value)?;
    norito::to_bytes(&StateValueRecordV1 {
        schema_hash: state_value_schema_hash_v1(&schema_payload),
        atoms: vec![StateValueAtomV1::Pointer(envelope)],
    })
    .map_err(|_| ivm::VMError::NoritoInvalid)
}
fn decode_validation_fee_credit_state_value(bytes: &[u8]) -> Option<Quantity> {
    if bytes.len() > MAX_STATE_VALUE_RECORD_BYTES {
        return None;
    }
    let record: StateValueRecordV1 = norito::decode_from_bytes(bytes).ok()?;
    if norito::to_bytes(&record).ok()? != bytes {
        return None;
    }
    let schema_payload = norito::to_bytes(&validation_fee_credit_state_schema()).ok()?;
    if record.schema_hash != state_value_schema_hash_v1(&schema_payload) {
        return None;
    }
    let [StateValueAtomV1::Pointer(envelope)] = record.atoms.as_slice() else {
        return None;
    };
    ivm::numeric_tlv::decode_quantity_bytes(envelope).ok()
}
fn validation_fee_credit_asset_spec(
    state: &impl StateReadOnly,
    credit: &ValidationFeeCredit,
) -> Result<NumericSpec, ValidationFeeAdmissionError> {
    let expected = NumericSpec::try_fractional(u32::from(credit.asset_scale)).map_err(|_| {
        ValidationFeeAdmissionError::InvalidPolicyInvariant(
            "validation-fee policy asset scale exceeds the numeric domain",
        )
    })?;
    let definition = state
        .world()
        .asset_definition(&credit.fee_asset_definition_id)
        .map_err(
            |_| ValidationFeeAdmissionError::MissingCreditAssetDefinition {
                asset_definition_id: credit.fee_asset_definition_id.to_string(),
            },
        )?;
    let observed = definition.spec();
    if observed != expected {
        return Err(
            ValidationFeeAdmissionError::CreditAssetNumericSpecMismatch {
                asset_definition_id: credit.fee_asset_definition_id.to_string(),
                expected_scale: credit.asset_scale,
                observed_scale: observed.scale(),
            },
        );
    }
    if expected.check(credit.amount.as_numeric()).is_err() {
        return Err(ValidationFeeAdmissionError::CreditAmountOutsideAssetSpec {
            asset_definition_id: credit.fee_asset_definition_id.to_string(),
            amount: credit.amount.clone(),
            allowed_scale: credit.asset_scale,
        });
    }
    Ok(expected)
}
fn read_validation_fee_credit_balance(
    state_transaction: &StateTransaction<'_, '_>,
    credit: &ValidationFeeCredit,
) -> Result<Quantity, ValidationFeeAdmissionError> {
    let (key, asset_key) = validation_fee_credit_state_keys(state_transaction, credit)?;
    let value_bytes = state_transaction.world.smart_contract_state.get(&key);
    let binding_bytes = state_transaction.world.smart_contract_state.get(&asset_key);
    let (Some(value_bytes), Some(binding_bytes)) = (value_bytes, binding_bytes) else {
        if value_bytes.is_some() || binding_bytes.is_some() {
            return Err(if value_bytes.is_some() {
                ValidationFeeAdmissionError::MalformedCreditAssetBinding {
                    state_key: asset_key.to_string(),
                }
            } else {
                ValidationFeeAdmissionError::MalformedCreditBalance {
                    state_key: key.to_string(),
                }
            });
        }
        validation_fee_credit_asset_spec(state_transaction, credit)?;
        return Ok(Quantity::zero());
    };
    let value = decode_validation_fee_credit_state_value(value_bytes).ok_or_else(|| {
        ValidationFeeAdmissionError::MalformedCreditBalance {
            state_key: key.to_string(),
        }
    })?;
    let bound_asset =
        norito::decode_from_bytes::<AssetDefinitionId>(binding_bytes).map_err(|_| {
            ValidationFeeAdmissionError::MalformedCreditAssetBinding {
                state_key: asset_key.to_string(),
            }
        })?;
    if norito::to_bytes(&bound_asset)
        .map_err(
            |_| ValidationFeeAdmissionError::MalformedCreditAssetBinding {
                state_key: asset_key.to_string(),
            },
        )?
        .as_slice()
        != binding_bytes.as_slice()
    {
        return Err(ValidationFeeAdmissionError::MalformedCreditAssetBinding {
            state_key: asset_key.to_string(),
        });
    }
    if bound_asset != credit.fee_asset_definition_id {
        return Err(ValidationFeeAdmissionError::CreditAssetBindingMismatch {
            expected_asset_definition_id: credit.fee_asset_definition_id.to_string(),
            observed_asset_definition_id: bound_asset.to_string(),
        });
    }
    let spec = validation_fee_credit_asset_spec(state_transaction, credit)?;
    if spec.check(value.as_numeric()).is_err() {
        return Err(ValidationFeeAdmissionError::CreditAmountOutsideAssetSpec {
            asset_definition_id: credit.fee_asset_definition_id.to_string(),
            amount: value,
            allowed_scale: credit.asset_scale,
        });
    }
    Ok(value)
}
fn ensure_validation_fee_credit_capacity(
    state_transaction: &StateTransaction<'_, '_>,
    credit: &ValidationFeeCredit,
) -> Result<(), ValidationFeeAdmissionError> {
    let current = read_validation_fee_credit_balance(state_transaction, credit)?;
    current.checked_add(&credit.amount).map_err(|_| {
        ValidationFeeAdmissionError::CreditBalanceOverflow {
            current,
            additional: credit.amount.clone(),
        }
    })?;
    Ok(())
}
/// Persist a previously validated fee credit in the current state transaction.
///
/// The caller must invoke this only after the corresponding signed transaction and its data
/// triggers have succeeded. Dropping the state transaction rolls this mutation back.
/// First-release lifecycle-scoped balance and asset leaves remain in consensus state after their
/// balance reaches zero; no retirement transition exists from which Core could derive inactivity
/// and an exact reference count.
pub(crate) fn commit_validation_fee_credit(
    state_transaction: &mut StateTransaction<'_, '_>,
    credit: Option<&ValidationFeeCredit>,
) -> Result<(), TransactionRejectionReason> {
    let Some(credit) = credit else {
        return Ok(());
    };
    let current = read_validation_fee_credit_balance(state_transaction, credit)
        .map_err(admission_rejection)?;
    let next = current.checked_add(&credit.amount).map_err(|_| {
        admission_rejection(ValidationFeeAdmissionError::CreditBalanceOverflow {
            current,
            additional: credit.amount.clone(),
        })
    })?;
    let (key, asset_key) =
        validation_fee_credit_state_keys(state_transaction, credit).map_err(admission_rejection)?;
    let bytes = encode_validation_fee_credit_state_value(&next).map_err(|_| {
        admission_rejection(ValidationFeeAdmissionError::MalformedCreditBalance {
            state_key: key.to_string(),
        })
    })?;
    let asset_bytes = norito::to_bytes(&credit.fee_asset_definition_id).map_err(|_| {
        admission_rejection(ValidationFeeAdmissionError::MalformedCreditAssetBinding {
            state_key: asset_key.to_string(),
        })
    })?;
    let (projection_key, projection_asset_key, projection_seal_key) =
        validation_fee_credit_projection_state_keys(state_transaction, credit)
            .map_err(admission_rejection)?;
    let seal_bytes = norito::to_bytes(&credit.lifecycle_seal).map_err(|_| {
        admission_rejection(ValidationFeeAdmissionError::InvalidPayoutLifecycleSeal)
    })?;
    state_transaction
        .world
        .smart_contract_state
        .insert(asset_key, asset_bytes.clone());
    state_transaction
        .world
        .smart_contract_state
        .insert(key, bytes.clone());
    state_transaction
        .world
        .smart_contract_state
        .insert(projection_asset_key, asset_bytes);
    state_transaction
        .world
        .smart_contract_state
        .insert(projection_seal_key, seal_bytes);
    state_transaction
        .world
        .smart_contract_state
        .insert(projection_key, bytes);
    Ok(())
}
fn consume_validation_fee_credit(
    state_transaction: &mut StateTransaction<'_, '_>,
    credit: &ValidationFeeCredit,
) -> Result<(), ValidationFeeAdmissionError> {
    // This validator-side mutation is the single authoritative debit. The contract may read the
    // scoped counter to avoid queueing an unaffordable batch, but guest STATE_SET/DEL are denied
    // for this leaf so contract code cannot double-debit, replenish, or forge credit.
    if credit.amount.is_zero() {
        return Ok(());
    }
    let available = read_validation_fee_credit_balance(state_transaction, credit)?;
    let remaining = available.checked_sub(&credit.amount).map_err(|_| {
        ValidationFeeAdmissionError::InsufficientCreditBalance {
            available,
            requested: credit.amount.clone(),
        }
    })?;
    let (key, _) = validation_fee_credit_state_keys(state_transaction, credit)?;
    let bytes = encode_validation_fee_credit_state_value(&remaining).map_err(|_| {
        ValidationFeeAdmissionError::MalformedCreditBalance {
            state_key: key.to_string(),
        }
    })?;
    let (projection_key, projection_asset_key, projection_seal_key) =
        validation_fee_credit_projection_state_keys(state_transaction, credit)?;
    let asset_bytes = norito::to_bytes(&credit.fee_asset_definition_id).map_err(|_| {
        ValidationFeeAdmissionError::MalformedCreditAssetBinding {
            state_key: projection_asset_key.to_string(),
        }
    })?;
    let seal_bytes = norito::to_bytes(&credit.lifecycle_seal)
        .map_err(|_| ValidationFeeAdmissionError::InvalidPayoutLifecycleSeal)?;
    state_transaction
        .world
        .smart_contract_state
        .insert(key, bytes.clone());
    state_transaction
        .world
        .smart_contract_state
        .insert(projection_asset_key, asset_bytes);
    state_transaction
        .world
        .smart_contract_state
        .insert(projection_seal_key, seal_bytes);
    state_transaction
        .world
        .smart_contract_state
        .insert(projection_key, bytes);
    Ok(())
}
#[cfg(test)]
fn enforce_policy(
    tx: &SignedTransaction,
    policy: &ValidationFeePolicyV1,
) -> Result<(), ValidationFeeAdmissionError> {
    enforce_policy_with_credit(tx, policy).map(|_| ())
}
/// Validate all signed fee coordinates and return only the credit whose transfer executes in the
/// top-level transaction context. Nested multisig proposal contents are signed but deferred, so
/// they are deliberately excluded until [`enforce_deferred_policy_with_credit`] runs them.
#[cfg(test)]
fn enforce_policy_with_credit(
    tx: &SignedTransaction,
    policy: &ValidationFeePolicyV1,
) -> Result<u64, ValidationFeeAdmissionError> {
    enforce_policy_with_credit_and_hijiri(tx, policy, None, &no_hijiri_account_risk)
}
fn enforce_policy_with_credit_and_hijiri(
    tx: &SignedTransaction,
    policy: &ValidationFeePolicyV1,
    hijiri: Option<&HijiriParametersV1>,
    resolve_account_risk: &dyn Fn(
        &AccountId,
    ) -> Result<
        Option<HijiriAccountRiskV1>,
        ValidationFeeAdmissionError,
    >,
) -> Result<u64, ValidationFeeAdmissionError> {
    match policy.charging_mode {
        ValidationFeeChargingMode::Disabled => return Ok(0),
        ValidationFeeChargingMode::PerQualifyingTransferInstruction => {}
    }
    let fee_asset_definition_id = policy_fee_asset_definition_id(policy)?;
    let treasury = policy_treasury_account_id(policy)?;
    let transfer_collection =
        collect_asset_transfers(tx.instructions(), tx.authority(), &fee_asset_definition_id)?;
    let fee_asset_transfers = collect_fee_asset_transfers(
        &transfer_collection.transfers,
        policy,
        &fee_asset_definition_id,
    )?;
    let metadata_contains_validation_fee = has_validation_fee_metadata(tx.metadata());
    let fee_coordinate = validation_fee_coordinate(tx.metadata())?;
    let explicit_fee_context_index = fee_coordinate
        .map(|coordinate| {
            resolve_fee_coordinate_context(coordinate, &transfer_collection.transfers)
        })
        .transpose()?;
    // The signed metadata describes the explicitly coordinated fee context. For ordinary
    // transactions that is context zero; a multisig proposal can instead coordinate a nested
    // execution account whose account-bound Hijiri quote hash necessarily differs from its signer.
    let metadata_fee_context_index = explicit_fee_context_index.unwrap_or(0);
    let mut requires_policy_metadata = false;
    let mut credited_minor_units = 0_u64;
    let mut metadata_hijiri_quote_hash = None;
    for (context_index, context) in transfer_collection.contexts.iter().enumerate() {
        let resolved_hijiri =
            resolve_hijiri_fee(hijiri, &context.execution_account_id, resolve_account_risk)?;
        if context_index == metadata_fee_context_index {
            metadata_hijiri_quote_hash = resolved_hijiri.map(|resolved| resolved.quote_hash);
            if metadata_contains_validation_fee {
                validate_policy_metadata(tx.metadata(), policy, metadata_hijiri_quote_hash)?;
            }
        }
        let transaction_fee_coordinate = if explicit_fee_context_index == Some(context_index) {
            fee_coordinate
        } else {
            None
        };
        let marker_fee_coordinate = multisig_marker_coordinate_for_context(
            context_index,
            context,
            policy,
            resolved_hijiri.map(|resolved| resolved.quote_hash),
            &transfer_collection.multisig_fee_markers,
            &fee_asset_transfers,
        )?;
        if transaction_fee_coordinate.is_some()
            && marker_fee_coordinate.is_some()
            && transaction_fee_coordinate != marker_fee_coordinate
        {
            return Err(
                ValidationFeeAdmissionError::ConflictingMultisigFeeCoordinate { context_index },
            );
        }
        let context_fee_coordinate = marker_fee_coordinate.or(transaction_fee_coordinate);
        let validated = enforce_context_policy(
            context_index,
            &context.execution_account_id,
            &treasury,
            policy,
            &fee_asset_definition_id,
            &transfer_collection.transfers,
            &fee_asset_transfers,
            context_fee_coordinate,
            false,
            resolved_hijiri.map(|resolved| resolved.multiplier),
        )?;
        requires_policy_metadata |= validated.requires_policy_metadata;
        if context_index == 0 {
            credited_minor_units = validated.credited_minor_units;
        }
    }
    if requires_policy_metadata && !metadata_contains_validation_fee {
        validate_policy_metadata(tx.metadata(), policy, metadata_hijiri_quote_hash)?;
    }
    Ok(credited_minor_units)
}
fn multisig_marker_coordinate_for_context(
    context_index: usize,
    context: &TransferExecutionContext,
    policy: &ValidationFeePolicyV1,
    expected_hijiri_fee_quote_hash: Option<[u8; 32]>,
    markers: &[MultisigFeeMarkerSummary],
    fee_asset_transfers: &[FeeAssetTransferSummary],
) -> Result<Option<FeeInstructionCoordinate>, ValidationFeeAdmissionError> {
    let context_markers: Vec<_> = markers
        .iter()
        .filter(|marker| marker.context_index == context_index)
        .collect();
    if !context.requires_multisig_fee_marker {
        if context_markers.is_empty() {
            return Ok(None);
        }
        return Err(ValidationFeeAdmissionError::UnexpectedMultisigFeeMarker { context_index });
    }
    let has_fee_asset_effect = fee_asset_transfers
        .iter()
        .any(|transfer| transfer.context_index == context_index);
    if !has_fee_asset_effect {
        if context_markers.is_empty() {
            return Ok(None);
        }
        return Err(ValidationFeeAdmissionError::UnexpectedMultisigFeeMarker { context_index });
    }
    if context_markers.is_empty() {
        return Err(ValidationFeeAdmissionError::MissingMultisigFeeMarker { context_index });
    }
    if context_markers.len() != 1 {
        return Err(ValidationFeeAdmissionError::DuplicateMultisigFeeMarkers {
            context_index,
            count: context_markers.len(),
        });
    }
    let marker = context_markers[0].marker;
    if marker.policy_version != policy.policy_version {
        return Err(
            ValidationFeeAdmissionError::WrongMultisigFeeMarkerPolicyVersion {
                expected_version: policy.policy_version,
                observed_version: marker.policy_version,
            },
        );
    }
    let policy_hash = policy
        .policy_hash()
        .map_err(|_| ValidationFeeAdmissionError::PolicyHashFailed)?;
    if marker.policy_hash != policy_hash {
        return Err(
            ValidationFeeAdmissionError::WrongMultisigFeeMarkerPolicyHash {
                expected_hash_hex: hex::encode(policy_hash),
                observed_hash_hex: hex::encode(marker.policy_hash),
            },
        );
    }
    if marker.hijiri_fee_quote_hash != expected_hijiri_fee_quote_hash {
        return Err(
            ValidationFeeAdmissionError::WrongMultisigFeeMarkerHijiriFeeQuoteHash {
                expected_hash_hex: expected_hijiri_fee_quote_hash.map(hex::encode),
                observed_hash_hex: marker.hijiri_fee_quote_hash.map(hex::encode),
            },
        );
    }
    let instruction_index = usize::try_from(marker.instruction_index).map_err(|_| {
        ValidationFeeAdmissionError::MalformedMultisigFeeMarker {
            context_index,
            instruction_index: context_markers[0].marker_instruction_index,
        }
    })?;
    let entry_index = marker
        .transfer_entry_index
        .map(|entry_index| {
            usize::try_from(entry_index).map_err(|_| {
                ValidationFeeAdmissionError::MalformedMultisigFeeMarker {
                    context_index,
                    instruction_index: context_markers[0].marker_instruction_index,
                }
            })
        })
        .transpose()?;
    Ok(Some(FeeInstructionCoordinate {
        instruction_index,
        entry_index,
    }))
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ValidatedContextFee {
    requires_policy_metadata: bool,
    credited_minor_units: u64,
}
impl ValidatedContextFee {
    const NONE: Self = Self {
        requires_policy_metadata: false,
        credited_minor_units: 0,
    };
}
fn enforce_context_policy(
    context_index: usize,
    execution_account_id: &AccountId,
    treasury: &AccountId,
    policy: &ValidationFeePolicyV1,
    fee_asset_definition_id: &AssetDefinitionId,
    transfers: &[AssetTransferSummary],
    fee_asset_transfers: &[FeeAssetTransferSummary],
    fee_coordinate: Option<FeeInstructionCoordinate>,
    allow_implicit_context_fee: bool,
    hijiri_multiplier: Option<Q16>,
) -> Result<ValidatedContextFee, ValidationFeeAdmissionError> {
    let mut qualifying_transfer_count = 0usize;
    let mut uncoordinated_fee_candidates = Vec::new();
    if let Some(fee_coordinate) = fee_coordinate {
        let fee_transfer = validate_explicit_fee_coordinate(
            fee_coordinate,
            context_index,
            transfers,
            fee_asset_transfers,
            execution_account_id,
            treasury,
            fee_asset_definition_id,
        )?;
        for transfer in fee_asset_transfers
            .iter()
            .filter(|transfer| transfer.context_index == context_index)
        {
            if fee_coordinate.matches(transfer) {
                continue;
            }
            qualifying_transfer_count += 1;
        }
        if qualifying_transfer_count == 0 {
            if fee_transfer.amount_minor_units != 0 {
                return Err(ValidationFeeAdmissionError::WrongFeeAmount {
                    expected_minor_units: 0,
                    observed_minor_units: fee_transfer.amount_minor_units,
                });
            }
            return Ok(ValidatedContextFee::NONE);
        }
        let required_fee_minor_units =
            required_fee_minor_units(qualifying_transfer_count, policy, hijiri_multiplier)?;
        if fee_transfer.amount_minor_units != required_fee_minor_units {
            return Err(ValidationFeeAdmissionError::WrongFeeAmount {
                expected_minor_units: required_fee_minor_units,
                observed_minor_units: fee_transfer.amount_minor_units,
            });
        }
        return Ok(ValidatedContextFee {
            requires_policy_metadata: true,
            credited_minor_units: required_fee_minor_units,
        });
    }
    for transfer in fee_asset_transfers
        .iter()
        .filter(|transfer| transfer.context_index == context_index)
    {
        if allow_implicit_context_fee
            && &transfer.source_account_id == execution_account_id
            && &transfer.destination_account_id == treasury
        {
            uncoordinated_fee_candidates.push(transfer);
        } else {
            qualifying_transfer_count += 1;
        }
    }
    if qualifying_transfer_count == 0 {
        if uncoordinated_fee_candidates.is_empty() {
            return Ok(ValidatedContextFee::NONE);
        }
        if uncoordinated_fee_candidates.len() > 1 {
            return Err(ValidationFeeAdmissionError::DuplicateFeeInstructions {
                count: uncoordinated_fee_candidates.len(),
            });
        }
        return Err(ValidationFeeAdmissionError::MissingFeeInstructionCoordinate);
    }
    if !allow_implicit_context_fee {
        let uncoordinated_fee_candidate_count = fee_asset_transfers
            .iter()
            .filter(|transfer| transfer.context_index == context_index)
            .filter(|transfer| {
                &transfer.source_account_id == execution_account_id
                    && &transfer.destination_account_id == treasury
            })
            .count();
        if uncoordinated_fee_candidate_count > 1 {
            return Err(ValidationFeeAdmissionError::DuplicateFeeInstructions {
                count: uncoordinated_fee_candidate_count,
            });
        }
        if uncoordinated_fee_candidate_count == 1 {
            return Err(ValidationFeeAdmissionError::MissingFeeInstructionCoordinate);
        }
    }
    if uncoordinated_fee_candidates.is_empty() {
        let required_fee_minor_units =
            required_fee_minor_units(qualifying_transfer_count, policy, hijiri_multiplier)?;
        return Err(ValidationFeeAdmissionError::MissingFee {
            required_minor_units: required_fee_minor_units,
        });
    }
    if uncoordinated_fee_candidates.len() > 1 {
        return Err(ValidationFeeAdmissionError::DuplicateFeeInstructions {
            count: uncoordinated_fee_candidates.len(),
        });
    }
    let required_fee_minor_units =
        required_fee_minor_units(qualifying_transfer_count, policy, hijiri_multiplier)?;
    let fee_transfer = &uncoordinated_fee_candidates[0];
    if fee_transfer.amount_minor_units != required_fee_minor_units {
        return Err(ValidationFeeAdmissionError::WrongFeeAmount {
            expected_minor_units: required_fee_minor_units,
            observed_minor_units: fee_transfer.amount_minor_units,
        });
    }
    Ok(ValidatedContextFee {
        requires_policy_metadata: true,
        credited_minor_units: required_fee_minor_units,
    })
}
fn treasury_payout_exemption_enabled(policy: &ValidationFeePolicyV1) -> bool {
    policy
        .exemption_classes
        .iter()
        .any(|class| class == VALIDATION_FEE_TREASURY_PAYOUT_EXEMPTION_CLASS)
}
fn validate_explicit_fee_coordinate<'a>(
    fee_coordinate: FeeInstructionCoordinate,
    context_index: usize,
    transfers: &'a [AssetTransferSummary],
    fee_asset_transfers: &'a [FeeAssetTransferSummary],
    execution_account_id: &AccountId,
    treasury: &AccountId,
    fee_asset_definition_id: &AssetDefinitionId,
) -> Result<&'a FeeAssetTransferSummary, ValidationFeeAdmissionError> {
    let Some(raw_fee_transfer) = transfers.iter().find(|transfer| {
        transfer.context_index == context_index && fee_coordinate.matches(*transfer)
    }) else {
        return Err(ValidationFeeAdmissionError::FeeInstructionNotFound {
            instruction_index: fee_coordinate.instruction_index,
            entry_index: fee_coordinate.entry_index,
        });
    };
    if &raw_fee_transfer.source_account_id != execution_account_id {
        return Err(ValidationFeeAdmissionError::WrongFeeSource {
            instruction_index: fee_coordinate.instruction_index,
            entry_index: fee_coordinate.entry_index,
        });
    }
    if &raw_fee_transfer.asset_definition_id != fee_asset_definition_id {
        return Err(ValidationFeeAdmissionError::WrongFeeAsset {
            instruction_index: fee_coordinate.instruction_index,
            entry_index: fee_coordinate.entry_index,
        });
    }
    if &raw_fee_transfer.destination_account_id != treasury {
        return Err(ValidationFeeAdmissionError::WrongFeeBeneficiary {
            instruction_index: fee_coordinate.instruction_index,
            entry_index: fee_coordinate.entry_index,
            expected_account_id: treasury.to_string(),
            observed_account_id: raw_fee_transfer.destination_account_id.to_string(),
        });
    }
    fee_asset_transfers
        .iter()
        .find(|transfer| {
            transfer.context_index == context_index && fee_coordinate.matches(*transfer)
        })
        .ok_or(ValidationFeeAdmissionError::FeeInstructionNotFound {
            instruction_index: fee_coordinate.instruction_index,
            entry_index: fee_coordinate.entry_index,
        })
}
fn resolve_fee_coordinate_context(
    fee_coordinate: FeeInstructionCoordinate,
    transfers: &[AssetTransferSummary],
) -> Result<usize, ValidationFeeAdmissionError> {
    let mut matched_context_index = None;
    for transfer in transfers
        .iter()
        .filter(|transfer| fee_coordinate.matches(*transfer))
    {
        match matched_context_index {
            None => matched_context_index = Some(transfer.context_index),
            Some(context_index) if context_index == transfer.context_index => {}
            Some(_) => {
                return Err(
                    ValidationFeeAdmissionError::AmbiguousFeeInstructionCoordinate {
                        instruction_index: fee_coordinate.instruction_index,
                        entry_index: fee_coordinate.entry_index,
                    },
                );
            }
        }
    }
    matched_context_index.ok_or(ValidationFeeAdmissionError::FeeInstructionNotFound {
        instruction_index: fee_coordinate.instruction_index,
        entry_index: fee_coordinate.entry_index,
    })
}
fn required_fee_minor_units(
    qualifying_transfer_count: usize,
    policy: &ValidationFeePolicyV1,
    hijiri_multiplier: Option<Q16>,
) -> Result<u64, ValidationFeeAdmissionError> {
    let base_per_transfer_minor_units =
        quantity_to_minor_units(&policy.fee, policy.ds_scale, usize::MAX)
            .map_err(|_| ValidationFeeAdmissionError::RequiredFeeOverflow)?;
    let aggregate_base_minor_units = u64::try_from(
        (qualifying_transfer_count as u128)
            .checked_mul(u128::from(base_per_transfer_minor_units))
            .ok_or(ValidationFeeAdmissionError::RequiredFeeOverflow)?,
    )
    .map_err(|_| ValidationFeeAdmissionError::RequiredFeeOverflow)?;
    hijiri_multiplier
        .map_or(Some(aggregate_base_minor_units), |multiplier| {
            multiplier.checked_mul_u64_ceil(aggregate_base_minor_units)
        })
        .ok_or(ValidationFeeAdmissionError::RequiredFeeOverflow)
}
fn validation_fee_coordinate(
    metadata: &Metadata,
) -> Result<Option<FeeInstructionCoordinate>, ValidationFeeAdmissionError> {
    let Some(instruction_index_value) = metadata.get(VALIDATION_FEE_INSTRUCTION_INDEX_METADATA_KEY)
    else {
        if metadata
            .get(VALIDATION_FEE_TRANSFER_ENTRY_INDEX_METADATA_KEY)
            .is_some()
        {
            return Err(ValidationFeeAdmissionError::MalformedFeeInstructionMetadata);
        }
        return Ok(None);
    };
    let instruction_index = instruction_index_value
        .try_into_any_norito::<u64>()
        .map_err(|_| ValidationFeeAdmissionError::MalformedFeeInstructionMetadata)?;
    let instruction_index = usize::try_from(instruction_index)
        .map_err(|_| ValidationFeeAdmissionError::MalformedFeeInstructionMetadata)?;
    let entry_index = metadata
        .get(VALIDATION_FEE_TRANSFER_ENTRY_INDEX_METADATA_KEY)
        .map(|value| {
            value
                .try_into_any_norito::<u64>()
                .map_err(|_| ValidationFeeAdmissionError::MalformedFeeInstructionMetadata)
                .and_then(|entry_index| {
                    usize::try_from(entry_index)
                        .map_err(|_| ValidationFeeAdmissionError::MalformedFeeInstructionMetadata)
                })
        })
        .transpose()?;
    Ok(Some(FeeInstructionCoordinate {
        instruction_index,
        entry_index,
    }))
}
fn validate_policy_metadata(
    metadata: &Metadata,
    policy: &ValidationFeePolicyV1,
    expected_hijiri_fee_quote_hash: Option<[u8; 32]>,
) -> Result<(), ValidationFeeAdmissionError> {
    let observed_version = metadata
        .get(VALIDATION_FEE_POLICY_VERSION_METADATA_KEY)
        .ok_or(ValidationFeeAdmissionError::MissingPolicyVersionMetadata)?
        .try_into_any_norito::<u64>()
        .map_err(|_| ValidationFeeAdmissionError::MalformedPolicyVersionMetadata)?;
    if observed_version != policy.policy_version {
        return Err(ValidationFeeAdmissionError::WrongPolicyVersionMetadata {
            expected_version: policy.policy_version,
            observed_version,
        });
    }
    let observed_hash_hex = metadata
        .get(VALIDATION_FEE_POLICY_HASH_METADATA_KEY)
        .ok_or(ValidationFeeAdmissionError::MissingPolicyHashMetadata)?
        .try_into_any_norito::<String>()
        .map_err(|_| ValidationFeeAdmissionError::MalformedPolicyHashMetadata)?;
    let observed_hash = decode_hash_hex(&observed_hash_hex)
        .ok_or(ValidationFeeAdmissionError::MalformedPolicyHashMetadata)?;
    let policy_hash = policy
        .policy_hash()
        .map_err(|_| ValidationFeeAdmissionError::PolicyHashFailed)?;
    if observed_hash != policy_hash {
        return Err(ValidationFeeAdmissionError::WrongPolicyHashMetadata {
            expected_hash_hex: hex::encode(policy_hash),
            observed_hash_hex,
        });
    }
    let observed_hijiri_hash = metadata.get(VALIDATION_FEE_HIJIRI_FEE_QUOTE_HASH_METADATA_KEY);
    match (expected_hijiri_fee_quote_hash, observed_hijiri_hash) {
        (None, None) => {}
        (None, Some(_)) => {
            return Err(ValidationFeeAdmissionError::UnexpectedHijiriFeeQuoteHashMetadata);
        }
        (Some(_), None) => {
            return Err(ValidationFeeAdmissionError::MissingHijiriFeeQuoteHashMetadata);
        }
        (Some(expected), Some(observed)) => {
            let observed_hash_hex = observed
                .try_into_any_norito::<String>()
                .map_err(|_| ValidationFeeAdmissionError::MalformedHijiriFeeQuoteHashMetadata)?;
            let observed_hash = decode_canonical_hash_hex(&observed_hash_hex)
                .ok_or(ValidationFeeAdmissionError::MalformedHijiriFeeQuoteHashMetadata)?;
            if observed_hash != expected {
                return Err(
                    ValidationFeeAdmissionError::WrongHijiriFeeQuoteHashMetadata {
                        expected_hash_hex: hex::encode(expected),
                        observed_hash_hex,
                    },
                );
            }
        }
    }
    Ok(())
}
fn has_validation_fee_metadata(metadata: &Metadata) -> bool {
    metadata
        .get(VALIDATION_FEE_POLICY_VERSION_METADATA_KEY)
        .is_some()
        || metadata
            .get(VALIDATION_FEE_POLICY_HASH_METADATA_KEY)
            .is_some()
        || metadata
            .get(VALIDATION_FEE_HIJIRI_FEE_QUOTE_HASH_METADATA_KEY)
            .is_some()
        || metadata
            .get(VALIDATION_FEE_INSTRUCTION_INDEX_METADATA_KEY)
            .is_some()
        || metadata
            .get(VALIDATION_FEE_TRANSFER_ENTRY_INDEX_METADATA_KEY)
            .is_some()
}
/// Return whether a signed transaction advertises validation-fee policy/coordinate metadata.
/// Such transactions require consensus credit post-processing and must not use detached batch
/// merge paths that commit effects before admission facts.
pub(crate) fn transaction_has_validation_fee_metadata(tx: &SignedTransaction) -> bool {
    has_validation_fee_metadata(tx.metadata())
}
fn collect_asset_transfers(
    executable: &Executable,
    authority: &AccountId,
    fee_asset_definition_id: &AssetDefinitionId,
) -> Result<TransferCollection, ValidationFeeAdmissionError> {
    let batch_instructions;
    let instructions = match executable {
        Executable::Instructions(instructions) => instructions.as_ref(),
        // The overlay is part of the signed transaction payload and is bound to the bytecode by
        // the proved-IVM attachment. Proof verification still runs before the overlay executes.
        Executable::IvmProved(proved) => proved.overlay.as_ref(),
        Executable::Batch(items) => {
            if items
                .iter()
                .any(|item| matches!(item, ExecutableBatchItem::ContractCall(_)))
            {
                return Err(ValidationFeeAdmissionError::UnsupportedExecutable);
            }
            batch_instructions = items
                .iter()
                .filter_map(|item| match item {
                    ExecutableBatchItem::Instruction(instruction) => Some(instruction.clone()),
                    ExecutableBatchItem::ContractCall(_) => None,
                })
                .collect::<Vec<_>>();
            &batch_instructions
        }
        Executable::ContractCall(_) | Executable::Ivm(_) => {
            return Err(ValidationFeeAdmissionError::UnsupportedExecutable);
        }
    };
    let mut collection = TransferCollection {
        contexts: vec![TransferExecutionContext {
            execution_account_id: authority.clone(),
            requires_multisig_fee_marker: false,
        }],
        transfers: Vec::new(),
        multisig_fee_markers: Vec::new(),
    };
    collect_instruction_asset_transfers(instructions, 0, fee_asset_definition_id, &mut collection)?;
    Ok(collection)
}
fn repo_touches_fee_asset(
    cash_asset_definition_id: &AssetDefinitionId,
    collateral_asset_definition_id: &AssetDefinitionId,
    fee_asset_definition_id: &AssetDefinitionId,
) -> bool {
    cash_asset_definition_id == fee_asset_definition_id
        || collateral_asset_definition_id == fee_asset_definition_id
}
fn native_fee_asset_movement_wire_id(
    instruction: &InstructionBox,
    fee_asset_definition_id: &AssetDefinitionId,
) -> Option<&'static str> {
    if let Some(repo) = instruction.as_any().downcast_ref::<RepoInstructionBox>() {
        match repo {
            RepoInstructionBox::Initiate(isi)
                if repo_touches_fee_asset(
                    isi.cash_leg.asset_definition_id(),
                    isi.collateral_leg.asset_definition_id(),
                    fee_asset_definition_id,
                ) =>
            {
                return Some(RepoIsi::WIRE_ID);
            }
            // Reversal terms are loaded from the immutable on-chain agreement rather than
            // committed in the signed instruction. Without a signed effect plan, admission
            // cannot prove that the policy asset is untouched, so this path must fail closed.
            RepoInstructionBox::Reverse(_) => {
                return Some(ReverseRepoIsi::WIRE_ID);
            }
            RepoInstructionBox::Initiate(_) | RepoInstructionBox::MarginCall(_) => {}
        }
    }
    if let Some(isi) = instruction.as_any().downcast_ref::<RepoIsi>()
        && repo_touches_fee_asset(
            isi.cash_leg.asset_definition_id(),
            isi.collateral_leg.asset_definition_id(),
            fee_asset_definition_id,
        )
    {
        return Some(RepoIsi::WIRE_ID);
    }
    if instruction
        .as_any()
        .downcast_ref::<ReverseRepoIsi>()
        .is_some()
    {
        return Some(ReverseRepoIsi::WIRE_ID);
    }
    if let Some(settlement) = instruction
        .as_any()
        .downcast_ref::<SettlementInstructionBox>()
    {
        match settlement {
            SettlementInstructionBox::Dvp(isi)
                if isi.delivery_leg.asset_definition_id() == fee_asset_definition_id
                    || isi.payment_leg.asset_definition_id() == fee_asset_definition_id =>
            {
                return Some(DvpIsi::WIRE_ID);
            }
            SettlementInstructionBox::Pvp(isi)
                if isi.primary_leg.asset_definition_id() == fee_asset_definition_id
                    || isi.counter_leg.asset_definition_id() == fee_asset_definition_id =>
            {
                return Some(PvpIsi::WIRE_ID);
            }
            SettlementInstructionBox::SettleFxCorridor(isi)
                if isi.source_asset_definition_id == *fee_asset_definition_id
                    || isi.destination_asset_definition_id == *fee_asset_definition_id =>
            {
                return Some(SettleFxCorridor::WIRE_ID);
            }
            SettlementInstructionBox::FundFxCorridorEscrow(isi)
                if isi.destination_asset_definition_id == *fee_asset_definition_id =>
            {
                return Some(FundFxCorridorEscrow::WIRE_ID);
            }
            SettlementInstructionBox::RefundFxCorridorEscrow(isi)
                if isi.destination_asset_definition_id == *fee_asset_definition_id =>
            {
                return Some(RefundFxCorridorEscrow::WIRE_ID);
            }
            SettlementInstructionBox::Dvp(_)
            | SettlementInstructionBox::Pvp(_)
            | SettlementInstructionBox::SetFxCorridorPolicy(_)
            | SettlementInstructionBox::FundFxCorridorEscrow(_)
            | SettlementInstructionBox::RefundFxCorridorEscrow(_)
            | SettlementInstructionBox::SettleFxCorridor(_) => {}
        }
    }
    if let Some(isi) = instruction.as_any().downcast_ref::<DvpIsi>()
        && (isi.delivery_leg.asset_definition_id() == fee_asset_definition_id
            || isi.payment_leg.asset_definition_id() == fee_asset_definition_id)
    {
        return Some(DvpIsi::WIRE_ID);
    }
    if let Some(isi) = instruction.as_any().downcast_ref::<PvpIsi>()
        && (isi.primary_leg.asset_definition_id() == fee_asset_definition_id
            || isi.counter_leg.asset_definition_id() == fee_asset_definition_id)
    {
        return Some(PvpIsi::WIRE_ID);
    }
    if let Some(isi) = instruction.as_any().downcast_ref::<SettleFxCorridor>()
        && (isi.source_asset_definition_id == *fee_asset_definition_id
            || isi.destination_asset_definition_id == *fee_asset_definition_id)
    {
        return Some(SettleFxCorridor::WIRE_ID);
    }
    None
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NativeInstructionDsEffectDisposition {
    /// A transparent transfer whose signed source, destination, asset, and amount are collected.
    ExplicitAssetTransfer,
    /// A multisig proposal whose signed nested instructions are collected recursively.
    RecursiveMultisigProposal,
    /// A payer/recipient-signed conversion between a transparent balance and protocol escrow.
    AuditedKagemushaOfflineConversion,
    /// Deferred execution is guarded again when the stored trigger/proposal is materialized.
    GuardedDeferredEffect,
    /// Audited not to change numeric asset balances or supply.
    AuditedNoDsEffect,
    /// Known to be able to move, mint, burn, delete, lock, or derive an DS effect.
    RejectKnownDsCapable(&'static str),
    /// Present in the actual native dispatch table, but not yet classified by this guard.
    UnclassifiedDispatchable(&'static str),
    /// Custom or otherwise absent from the native dispatch table.
    Unknown,
}
fn native_instruction_ds_effect_disposition(
    instruction: &InstructionBox,
    fee_asset_definition_id: &AssetDefinitionId,
) -> NativeInstructionDsEffectDisposition {
    macro_rules! reject_known {
        ($($ty:ty),+ $(,)?) => {
            $(
                if instruction.as_any().downcast_ref::<$ty>().is_some() {
                    return NativeInstructionDsEffectDisposition::RejectKnownDsCapable(
                        core::any::type_name::<$ty>(),
                    );
                }
            )+
        };
    }
    macro_rules! audited_no_ds_effect {
        ($($ty:ty),+ $(,)?) => {
            $(
                if instruction.as_any().downcast_ref::<$ty>().is_some() {
                    return NativeInstructionDsEffectDisposition::AuditedNoDsEffect;
                }
            )+
        };
    }
    if instruction
        .as_any()
        .downcast_ref::<TransferAssetBatch>()
        .is_some()
        || instruction
            .as_any()
            .downcast_ref::<Transfer<Asset, Quantity, Account>>()
            .is_some()
    {
        return NativeInstructionDsEffectDisposition::ExplicitAssetTransfer;
    }
    if let Some(transfer) = instruction.as_any().downcast_ref::<TransferBox>() {
        return match transfer {
            TransferBox::Asset(_) => NativeInstructionDsEffectDisposition::ExplicitAssetTransfer,
            TransferBox::Domain(_) | TransferBox::AssetDefinition(_) => {
                NativeInstructionDsEffectDisposition::AuditedNoDsEffect
            }
            // NFT receipt can implicitly create an account and charge an account-admission fee.
            TransferBox::Nft(_) => {
                NativeInstructionDsEffectDisposition::RejectKnownDsCapable(core::any::type_name::<
                    TransferBox,
                >())
            }
        };
    }
    if let Ok(multisig) = MultisigInstructionBox::try_from(instruction) {
        return match multisig {
            MultisigInstructionBox::Propose(_) => {
                NativeInstructionDsEffectDisposition::RecursiveMultisigProposal
            }
            MultisigInstructionBox::Approve(_) => {
                NativeInstructionDsEffectDisposition::GuardedDeferredEffect
            }
            MultisigInstructionBox::Register(_)
            | MultisigInstructionBox::Cancel(_)
            | MultisigInstructionBox::InvalidateOutstanding(_) => {
                NativeInstructionDsEffectDisposition::AuditedNoDsEffect
            }
        };
    }
    if let Some(register) = instruction.as_any().downcast_ref::<RegisterBox>() {
        return match register {
            // Trigger artifacts are traversed by the opaque-deferred guards and checked again
            // immediately before their concrete instruction groups execute.
            RegisterBox::Trigger(_) => NativeInstructionDsEffectDisposition::GuardedDeferredEffect,
            RegisterBox::Peer(_)
            | RegisterBox::Domain(_)
            | RegisterBox::Account(_)
            | RegisterBox::AssetDefinition(_)
            | RegisterBox::Nft(_)
            | RegisterBox::Role(_) => NativeInstructionDsEffectDisposition::AuditedNoDsEffect,
        };
    }
    if let Some(unregister) = instruction.as_any().downcast_ref::<UnregisterBox>() {
        return match unregister {
            // Removing these containers can delete policy-DS balances or total supply without a
            // transparent transfer/burn instruction.
            UnregisterBox::Domain(_)
            | UnregisterBox::Account(_)
            | UnregisterBox::AssetDefinition(_) => {
                NativeInstructionDsEffectDisposition::RejectKnownDsCapable(core::any::type_name::<
                    UnregisterBox,
                >())
            }
            UnregisterBox::Peer(_)
            | UnregisterBox::Nft(_)
            | UnregisterBox::Role(_)
            | UnregisterBox::Trigger(_) => NativeInstructionDsEffectDisposition::AuditedNoDsEffect,
        };
    }
    if let Some(mint) = instruction.as_any().downcast_ref::<MintBox>() {
        return match mint {
            // Quantity mint is supply-changing and can also charge an implicit account fee.
            MintBox::Asset(_) => {
                NativeInstructionDsEffectDisposition::RejectKnownDsCapable(core::any::type_name::<
                    MintBox,
                >())
            }
            MintBox::TriggerRepetitions(_) => {
                NativeInstructionDsEffectDisposition::AuditedNoDsEffect
            }
        };
    }
    if let Some(burn) = instruction.as_any().downcast_ref::<BurnBox>() {
        return match burn {
            BurnBox::Asset(_) => {
                NativeInstructionDsEffectDisposition::RejectKnownDsCapable(core::any::type_name::<
                    BurnBox,
                >())
            }
            BurnBox::TriggerRepetitions(_) => {
                NativeInstructionDsEffectDisposition::AuditedNoDsEffect
            }
        };
    }
    reject_known!(Mint<Quantity, Asset>, Burn<Quantity, Asset>);
    if let Some(instruction_wire_id) =
        native_fee_asset_movement_wire_id(instruction, fee_asset_definition_id)
    {
        return NativeInstructionDsEffectDisposition::RejectKnownDsCapable(instruction_wire_id);
    }
    // Kagemusha does not expose an arbitrary transparent account-to-account transfer. A top-up
    // can only debit the payer authenticated inside the request and reserve the exact amount in
    // protocol escrow; a redemption can only debit provenance-bound protocol escrow and credit
    // the recipient authenticated inside the request. The offline peer-to-peer value transition
    // is proof-bound and does not execute a ledger transfer instruction. Keep this distinct from
    // `AuditedNoDsEffect`: the operations do change transparent balances, but they are closed
    // custody conversions rather than qualifying `Transfer<Asset, Quantity, Account>` principal
    // instructions under `PerQualifyingTransferInstruction`.
    if let Some(top_up) = instruction
        .as_any()
        .downcast_ref::<iroha_data_model::isi::offline::TopUpKagemushaRecursiveV4>(
    ) {
        return if top_up.request.asset.definition() == fee_asset_definition_id {
            NativeInstructionDsEffectDisposition::AuditedKagemushaOfflineConversion
        } else {
            NativeInstructionDsEffectDisposition::AuditedNoDsEffect
        };
    }
    if let Some(redeem) = instruction
        .as_any()
        .downcast_ref::<iroha_data_model::isi::offline::RedeemKagemushaRecursiveV4>(
    ) {
        return if &redeem.request.bundle.statement.asset == fee_asset_definition_id {
            NativeInstructionDsEffectDisposition::AuditedKagemushaOfflineConversion
        } else {
            NativeInstructionDsEffectDisposition::AuditedNoDsEffect
        };
    }
    // Repo initiation and settlement paths with signed non-policy-DS legs were audited above.
    // Reverse-repo settlement is always rejected because its legs are state-derived. Margin calls
    // do not move assets. Any signed policy-DS leg was already rejected.
    audited_no_ds_effect!(
        RepoInstructionBox,
        RepoIsi,
        iroha_data_model::isi::repo::RepoMarginCallIsi,
        SettlementInstructionBox,
        DvpIsi,
        PvpIsi,
        SetFxCorridorPolicy,
        SettleFxCorridor,
    );
    macro_rules! reject_fee_asset_transfer_control {
        ($ty:ty) => {
            if let Some(control) = instruction.as_any().downcast_ref::<$ty>() {
                return if control.asset_definition_id == *fee_asset_definition_id {
                    NativeInstructionDsEffectDisposition::RejectKnownDsCapable(
                        core::any::type_name::<$ty>(),
                    )
                } else {
                    NativeInstructionDsEffectDisposition::AuditedNoDsEffect
                };
            }
        };
    }
    // Availability, blacklists, and limits do not change balances, but applying them to the policy DS
    // would encumber treasury/user holdings outside the signed transfer-and-fee effect.
    reject_fee_asset_transfer_control!(iroha_data_model::isi::SetAssetTransferAvailability);
    reject_fee_asset_transfer_control!(iroha_data_model::isi::SetAssetTransferBlacklist);
    reject_fee_asset_transfer_control!(iroha_data_model::isi::SetAssetTransferControl);
    reject_fee_asset_transfer_control!(iroha_data_model::isi::SetAssetHoldingLimit);
    // These families have native, state-derived balance/supply/custody effects that cannot be
    // represented faithfully as a signed transparent transfer coordinate. Keep them disabled
    // until they have an effect-plan representation covered by the user signature.
    if let Some(commit) = instruction
        .as_any()
        .downcast_ref::<iroha_data_model::isi::smart_contract_code::CommitContractDeployment>(
    ) {
        return if commit.expected_previous_contract_address().is_some() {
            // Rotation deactivates the exact prior target and therefore has the same policy-era
            // rebinding risk as an explicit DeactivateContractInstance.
            NativeInstructionDsEffectDisposition::RejectKnownDsCapable(core::any::type_name::<
                iroha_data_model::isi::smart_contract_code::CommitContractDeployment,
            >())
        } else {
            NativeInstructionDsEffectDisposition::AuditedNoDsEffect
        };
    }
    reject_known!(
        // Active-policy deployments are intentionally one-way: registration and first
        // activation are balance-neutral (and allowed below), while removal/deactivation
        // would make a contract subject re-bindable and could redirect assets sent to that
        // subject after its code identity was reviewed.
        iroha_data_model::isi::smart_contract_code::DeactivateContractInstance,
        iroha_data_model::isi::smart_contract_code::RemoveSmartContractBytes,
        iroha_data_model::isi::rwa::RwaInstructionBox,
        iroha_data_model::isi::rwa::RegisterRwa,
        iroha_data_model::isi::rwa::TransferRwa,
        iroha_data_model::isi::rwa::MergeRwas,
        iroha_data_model::isi::rwa::RedeemRwa,
        iroha_data_model::isi::rwa::FreezeRwa,
        iroha_data_model::isi::rwa::UnfreezeRwa,
        iroha_data_model::isi::rwa::HoldRwa,
        iroha_data_model::isi::rwa::ReleaseRwa,
        iroha_data_model::isi::rwa::ForceTransferRwa,
        iroha_data_model::isi::rwa::SetRwaControls,
        iroha_data_model::isi::sorafs::RegisterPinManifest,
        iroha_data_model::isi::account_recovery::ReplaceAccountController,
        iroha_data_model::isi::account_recovery::FinalizeAccountRecovery,
        iroha_data_model::isi::alias_setup::EnsureAlias,
        iroha_data_model::isi::alias_setup::RenewAliasLease,
        iroha_data_model::isi::social::ClaimTwitterFollowReward,
        iroha_data_model::isi::social::SendToTwitter,
        iroha_data_model::isi::social::CancelTwitterEscrow,
        iroha_data_model::isi::escrow::OpenAssetEscrow,
        iroha_data_model::isi::escrow::ReleaseAssetEscrow,
        iroha_data_model::isi::escrow::CancelAssetEscrow,
        iroha_data_model::isi::escrow::ResolveEscrowDispute,
        iroha_data_model::isi::escrow::OpenAssetLock,
        iroha_data_model::isi::escrow::OpenConditionalEscrow,
        iroha_data_model::isi::escrow::AttestEscrowCondition,
        iroha_data_model::isi::escrow::ExpireConditionalEscrow,
        iroha_data_model::isi::escrow::DrawdownAssetLock,
        iroha_data_model::isi::escrow::CancelAssetLock,
        iroha_data_model::isi::escrow::ExpireAssetLock,
        iroha_data_model::isi::vpn::OpenVpnLeaseEscrow,
        iroha_data_model::isi::vpn::SettleVpnLease,
        iroha_data_model::isi::vpn::RefundExpiredVpnLease,
        iroha_data_model::isi::oracle::SubmitOracleObservation,
        iroha_data_model::isi::oracle::AggregateOracleFeed,
        iroha_data_model::isi::oracle::OpenOracleDispute,
        iroha_data_model::isi::oracle::ResolveOracleDispute,
        iroha_data_model::isi::staking::RegisterPublicLaneValidator,
        iroha_data_model::isi::staking::BondPublicLaneStake,
        iroha_data_model::isi::staking::SchedulePublicLaneUnbond,
        iroha_data_model::isi::staking::FinalizePublicLaneUnbond,
        iroha_data_model::isi::staking::SlashPublicLaneValidator,
        iroha_data_model::isi::staking::RecordPublicLaneRewards,
        iroha_data_model::isi::staking::ClaimPublicLaneRewards,
        iroha_data_model::isi::privacy::SubmitPrivacyProofV1,
        iroha_data_model::isi::zk::RegisterZkAsset,
        iroha_data_model::isi::zk::ScheduleConfidentialPolicyTransition,
        iroha_data_model::isi::zk::CancelConfidentialPolicyTransition,
        iroha_data_model::isi::governance::CastZkBallot,
        iroha_data_model::isi::governance::CastPlainBallot,
        iroha_data_model::isi::governance::RecordCitizenServiceOutcome,
        iroha_data_model::isi::governance::RegisterCitizen,
        iroha_data_model::isi::governance::UnregisterCitizen,
        iroha_data_model::isi::governance::SlashGovernanceLock,
        iroha_data_model::isi::governance::RestituteGovernanceLock,
    );
    // Narrow allowlist of native families whose handlers only mutate metadata, permissions,
    // control-plane records, or deferred-execution bookkeeping.
    audited_no_ds_effect!(
        iroha_data_model::isi::register::RegisterPeerWithPop,
        // The exact-roster QC is the complete authority for this control-plane
        // operation. Applying it changes only threshold-key session records.
        iroha_data_model::isi::consensus_keys::ApplyThresholdKeyLifecycleCertificateV1,
        // Parliament attempt admission and lifecycle transitions mutate only
        // their typed governance records. Core independently revalidates their
        // authority and consensus proofs before applying those state changes.
        iroha_data_model::isi::governance::CreateParliamentGovernanceAttemptV1,
        iroha_data_model::isi::governance::SubmitParliamentLifecycleTransitionV1,
        // These lifecycle steps only register content-addressed artifacts or create an
        // initially absent address -> code-hash binding. The executor rejects activation
        // over an address already bound to a different hash. Deactivation/removal remain
        // rejected above while the validation-fee policy is active, so ordinary users can
        // deploy permissionless contracts without creating a policy-era rebind path.
        iroha_data_model::isi::smart_contract_code::RegisterSmartContractCode,
        iroha_data_model::isi::smart_contract_code::RegisterSmartContractBytes,
        iroha_data_model::isi::smart_contract_code::UploadSmartContractCodeChunk,
        iroha_data_model::isi::smart_contract_code::FinalizeSmartContractCodeUpload,
        iroha_data_model::isi::smart_contract_code::CancelSmartContractCodeUpload,
        iroha_data_model::isi::smart_contract_code::ActivateContractInstance,
        // Privacy governance and bootstrap instructions affect only typed
        // privacy state and rollback-safe privacy budgets. Proof admission is
        // classified above because ZK-ACE can authorize a transparent transfer.
        iroha_data_model::isi::privacy::RegisterPrivacyProtocolActivationV1,
        iroha_data_model::isi::privacy::TransitionPrivacyProtocolLifecycleV1,
        iroha_data_model::isi::privacy::PublishPrivacyRootV1,
        iroha_data_model::isi::privacy::BootstrapPrivacyOrchardPoolV1,
        iroha_data_model::isi::privacy::BootstrapPrivacyProofManagedPoolV1,
        iroha_data_model::isi::privacy::BootstrapPrivacyPgcAccountsV1,
        iroha_data_model::isi::privacy::BootstrapPrivacyZkAmsRegistryV1,
        iroha_data_model::isi::privacy::RegisterPrivacyZkAcePolicyV1,
        iroha_data_model::isi::privacy::RotatePrivacyZkAcePolicyV1,
        iroha_data_model::isi::privacy::RevokePrivacyZkAcePolicyV1,
        // Kaigi only mutates domain metadata and emits diagnostic summaries;
        // its billing fields do not move assets or change supply.
        iroha_data_model::isi::kaigi::CreateKaigi,
        iroha_data_model::isi::kaigi::JoinKaigi,
        iroha_data_model::isi::kaigi::LeaveKaigi,
        iroha_data_model::isi::kaigi::EndKaigi,
        iroha_data_model::isi::kaigi::RecordKaigiUsage,
        iroha_data_model::isi::kaigi::SetKaigiRelayManifest,
        iroha_data_model::isi::kaigi::RegisterKaigiRelay,
        iroha_data_model::isi::kaigi::UnregisterKaigiRelay,
        iroha_data_model::isi::kaigi::ReportKaigiRelayHealth,
        SetKeyValueBox,
        RemoveKeyValueBox,
        iroha_data_model::isi::SetAssetKeyValue,
        iroha_data_model::isi::RemoveAssetKeyValue,
        iroha_data_model::isi::AddSignatory,
        iroha_data_model::isi::RemoveSignatory,
        iroha_data_model::isi::SetAccountQuorum,
        iroha_data_model::isi::alias_setup::ConfigureAliasAutoRenew,
        iroha_data_model::isi::alias_setup::RebindAccountAlias,
        iroha_data_model::isi::alias_setup::CompareAndSetPrimaryAccountAlias,
        GrantBox,
        RevokeBox,
        ExecuteTrigger,
        SetParameter,
        Upgrade,
        Log,
        iroha_data_model::isi::SetAssetDefinitionAlias,
        SetKeyValue<Trigger>,
    );
    match crate::smartcontracts::isi::registered_native_instruction_type_name(instruction) {
        Some(type_name) => {
            NativeInstructionDsEffectDisposition::UnclassifiedDispatchable(type_name)
        }
        None => NativeInstructionDsEffectDisposition::Unknown,
    }
}
fn collect_instruction_asset_transfers(
    instructions: &[InstructionBox],
    context_index: usize,
    fee_asset_definition_id: &AssetDefinitionId,
    collection: &mut TransferCollection,
) -> Result<(), ValidationFeeAdmissionError> {
    for (instruction_index, instruction) in instructions.iter().enumerate() {
        match ValidationFeeMultisigMarkerV1::parse_instruction(instruction) {
            Ok(Some(marker)) => {
                collection
                    .multisig_fee_markers
                    .push(MultisigFeeMarkerSummary {
                        context_index,
                        marker_instruction_index: instruction_index,
                        marker,
                    });
                continue;
            }
            Ok(None) => {}
            Err(_) => {
                return Err(ValidationFeeAdmissionError::MalformedMultisigFeeMarker {
                    context_index,
                    instruction_index,
                });
            }
        }
        match native_instruction_ds_effect_disposition(instruction, fee_asset_definition_id) {
            NativeInstructionDsEffectDisposition::ExplicitAssetTransfer => {
                if let Some(batch) = instruction.as_any().downcast_ref::<TransferAssetBatch>() {
                    for (entry_index, entry) in batch.entries().iter().enumerate() {
                        collection.transfers.push(AssetTransferSummary {
                            context_index,
                            instruction_index,
                            entry_index: Some(entry_index),
                            asset_definition_id: entry.asset_definition().clone(),
                            source_account_id: entry.from().clone(),
                            destination_account_id: entry.to().clone(),
                            amount: entry.amount().clone(),
                        });
                    }
                    continue;
                }
                let transfer = instruction
                    .as_any()
                    .downcast_ref::<Transfer<Asset, Quantity, Account>>()
                    .or_else(|| {
                        instruction
                            .as_any()
                            .downcast_ref::<TransferBox>()
                            .and_then(|transfer| match transfer {
                                TransferBox::Asset(transfer) => Some(transfer),
                                TransferBox::Domain(_)
                                | TransferBox::AssetDefinition(_)
                                | TransferBox::Nft(_) => None,
                            })
                    })
                    .expect("explicit asset-transfer disposition must contain a quantity transfer");
                collection.transfers.push(AssetTransferSummary {
                    context_index,
                    instruction_index,
                    entry_index: None,
                    asset_definition_id: transfer.source.definition.clone(),
                    source_account_id: transfer.source.account.clone(),
                    destination_account_id: transfer.destination.clone(),
                    amount: transfer.object.clone(),
                });
            }
            NativeInstructionDsEffectDisposition::RecursiveMultisigProposal => {
                let Ok(MultisigInstructionBox::Propose(propose)) =
                    MultisigInstructionBox::try_from(instruction)
                else {
                    unreachable!("recursive multisig disposition must contain a proposal");
                };
                let nested_context_index = collection.contexts.len();
                collection.contexts.push(TransferExecutionContext {
                    execution_account_id: propose.account,
                    requires_multisig_fee_marker: true,
                });
                collect_instruction_asset_transfers(
                    &propose.instructions,
                    nested_context_index,
                    fee_asset_definition_id,
                    collection,
                )?;
            }
            NativeInstructionDsEffectDisposition::AuditedKagemushaOfflineConversion => {
                let (instruction_wire_id, valid_public_binding) = if let Some(top_up) = instruction
                    .as_any()
                    .downcast_ref::<iroha_data_model::isi::offline::TopUpKagemushaRecursiveV4>()
                {
                    (
                        core::any::type_name::<
                            iroha_data_model::isi::offline::TopUpKagemushaRecursiveV4,
                        >(),
                        top_up.request.validate_public_binding().is_ok(),
                    )
                } else if let Some(redeem) =
                    instruction
                        .as_any()
                        .downcast_ref::<iroha_data_model::isi::offline::RedeemKagemushaRecursiveV4>(
                        )
                {
                    (
                        core::any::type_name::<
                            iroha_data_model::isi::offline::RedeemKagemushaRecursiveV4,
                        >(),
                        redeem.request.validate_public_binding().is_ok(),
                    )
                } else {
                    unreachable!(
                        "audited Kagemusha conversion disposition must contain a V4 top-up or redemption"
                    );
                };
                if !valid_public_binding {
                    return Err(
                        ValidationFeeAdmissionError::InvalidKagemushaOfflineConversion {
                            context_index,
                            instruction_index,
                            instruction_wire_id,
                        },
                    );
                }
            }
            NativeInstructionDsEffectDisposition::GuardedDeferredEffect
            | NativeInstructionDsEffectDisposition::AuditedNoDsEffect => {}
            NativeInstructionDsEffectDisposition::RejectKnownDsCapable(instruction_wire_id) => {
                return Err(
                    ValidationFeeAdmissionError::UnsupportedNativeFeeAssetMovement {
                        context_index,
                        instruction_index,
                        instruction_wire_id,
                    },
                );
            }
            NativeInstructionDsEffectDisposition::UnclassifiedDispatchable(
                registered_type_name,
            ) => {
                return Err(ValidationFeeAdmissionError::UnclassifiedNativeInstruction {
                    context_index,
                    instruction_index,
                    registered_type_name: Some(registered_type_name),
                });
            }
            NativeInstructionDsEffectDisposition::Unknown => {
                return Err(ValidationFeeAdmissionError::UnclassifiedNativeInstruction {
                    context_index,
                    instruction_index,
                    registered_type_name: None,
                });
            }
        }
    }
    Ok(())
}
fn reject_potential_implicit_account_admission_fee_with<F>(
    collection: &TransferCollection,
    mut account_exists: F,
) -> Result<(), ValidationFeeAdmissionError>
where
    F: FnMut(&AccountId) -> bool,
{
    let Some(transfer) = collection
        .transfers
        .iter()
        .find(|transfer| !account_exists(&transfer.destination_account_id))
    else {
        return Ok(());
    };
    Err(
        ValidationFeeAdmissionError::PotentialImplicitAccountAdmissionFee {
            context_index: transfer.context_index,
            instruction_index: transfer.instruction_index,
            entry_index: transfer.entry_index,
            destination_account_id: transfer.destination_account_id.to_string(),
        },
    )
}
fn collect_fee_asset_transfers(
    transfers: &[AssetTransferSummary],
    policy: &ValidationFeePolicyV1,
    fee_asset_definition_id: &AssetDefinitionId,
) -> Result<Vec<FeeAssetTransferSummary>, ValidationFeeAdmissionError> {
    transfers
        .iter()
        .filter(|transfer| &transfer.asset_definition_id == fee_asset_definition_id)
        .map(|transfer| {
            let amount_minor_units = quantity_to_minor_units(
                &transfer.amount,
                policy.ds_scale,
                transfer.instruction_index,
            )?;
            Ok(FeeAssetTransferSummary {
                context_index: transfer.context_index,
                instruction_index: transfer.instruction_index,
                entry_index: transfer.entry_index,
                source_account_id: transfer.source_account_id.clone(),
                destination_account_id: transfer.destination_account_id.clone(),
                amount_minor_units,
            })
        })
        .collect()
}
fn quantity_to_minor_units(
    amount: &Quantity,
    policy_scale: u8,
    instruction_index: usize,
) -> Result<u64, ValidationFeeAdmissionError> {
    let mantissa = amount
        .as_numeric()
        .try_mantissa_u128()
        .ok_or(ValidationFeeAdmissionError::AmountTooLarge { instruction_index })?;
    let amount_scale = amount.scale();
    let policy_scale = u32::from(policy_scale);
    if amount_scale > policy_scale {
        return Err(ValidationFeeAdmissionError::NonMinorUnitAmount {
            instruction_index,
            scale: amount_scale,
            policy_scale: policy_scale as u8,
        });
    }
    let scaled = mantissa
        .checked_mul(pow10(policy_scale - amount_scale)?)
        .ok_or(ValidationFeeAdmissionError::AmountTooLarge { instruction_index })?;
    u64::try_from(scaled)
        .map_err(|_| ValidationFeeAdmissionError::AmountTooLarge { instruction_index })
}
fn pow10(exponent: u32) -> Result<u128, ValidationFeeAdmissionError> {
    let mut value = 1u128;
    for _ in 0..exponent {
        value = value
            .checked_mul(10)
            .ok_or(ValidationFeeAdmissionError::RequiredFeeOverflow)?;
    }
    Ok(value)
}
fn decode_hash_hex(value: &str) -> Option<[u8; 32]> {
    let bytes = hex::decode(value).ok()?;
    bytes.try_into().ok()
}
fn decode_canonical_hash_hex(value: &str) -> Option<[u8; 32]> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return None;
    }
    decode_hash_hex(value)
}
fn format_entry_index(entry_index: Option<usize>) -> String {
    entry_index.map_or_else(String::new, |entry_index| format!("/{entry_index}"))
}
#[cfg(test)]
pub(crate) mod tests {
    include!("validation_fee/support_tests.rs");
    include!("validation_fee/admission_tests.rs");
    include!("validation_fee/runtime_tests.rs");
    include!("validation_fee/multisig_batch_tests.rs");
}
