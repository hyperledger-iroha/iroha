//! Validator-side enforcement for chain-level validation-fee policy.

use core::fmt;

use hex;
use iroha_crypto::{Hash, HashOf, blake2::Blake2b512};
use iroha_data_model::{
    ValidationFail,
    account::AccountId,
    asset::AssetDefinitionId,
    block::BlockHeader,
    isi::{
        InstructionBox, TransferAssetBatch, TransferBox,
        governance::{
            ApproveGovernanceProposal, CastParliamentBallot, EnactReferendum, FinalizeReferendum,
            ProposeValidationFeePayoutLifecycle, ProposeValidationFeePolicy,
        },
        register::RegisterBox,
        repo::{RepoInstructionBox, RepoIsi, ReverseRepoIsi},
        settlement::{
            DvpIsi, PvpIsi, SetFxCorridorPolicy, SettleFxCorridor, SettlementInstructionBox,
        },
    },
    metadata::Metadata,
    prelude::*,
    transaction::{Executable, ExecutableBatchItem, SignedTransaction},
    validation_fee::{
        VALIDATION_FEE_INSTRUCTION_INDEX_METADATA_KEY, VALIDATION_FEE_POLICY_HASH_METADATA_KEY,
        VALIDATION_FEE_POLICY_VERSION_METADATA_KEY,
        VALIDATION_FEE_TRANSFER_ENTRY_INDEX_METADATA_KEY, ValidationFeeChargingMode,
        ValidationFeeFinalizationEvidenceV1, ValidationFeeGovernanceVotingModeV1,
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

use crate::{
    smartcontracts::isi::triggers::{
        set::{ExecutableRef, SetReadOnly as _},
        specialized::LoadedActionTrait as _,
        trigger_is_enabled,
    },
    state::{StateTransaction, WorldReadOnly},
    tx::TransactionRejectionReason,
};

const VALIDATION_FEE_TREASURY_PAYOUT_EXEMPTION_CLASS: &str = "TREASURY_PAYOUT";
/// Contract-visible, consensus-owned nominal fee-credit balance.
///
/// The leaf is stored below the immutable contract-address scope used by the IVM host:
/// `sc/{Hash(contract_address_string)}/AvailableValidationFeeCredit`.
pub(crate) const VALIDATION_FEE_CREDIT_STATE_LEAF: &str = "AvailableValidationFeeCredit";
pub(crate) const VALIDATION_FEE_CREDIT_ASSET_STATE_LEAF: &str =
    "AvailableValidationFeeAssetDefinitionId";
pub(crate) const VALIDATION_FEE_PAYOUT_WRAPPER_ENTRYPOINT_PERMISSION: &str =
    "CanInvokeContractEntrypoint";
pub(crate) const VALIDATION_FEE_POOL_SWAP_ENTRYPOINT: &str = "swap_exact_in_quote_public";

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
            let wrapper_sbd_asset = iroha_data_model::asset::AssetId::new(
                binding.sbd_asset_id.clone(),
                binding.treasury_account_id.clone(),
            );
            (transfer.asset == wrapper_sbd_asset).then(|| binding.pool_vault_account_id.clone())
        })
}

/// Exact nominal protocol-fee value validated from a signed transaction payload.
///
/// This is an admission fact, not a balance mutation. Callers persist it only after the signed
/// transaction and all of its data triggers have completed successfully.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ValidationFeeCredit {
    treasury_account_id: AccountId,
    fee_asset_definition_id: AssetDefinitionId,
    asset_scale: u8,
    amount: Quantity,
}

impl ValidationFeeCredit {
    fn from_policy_minor_units(
        treasury_account_id: AccountId,
        fee_asset_definition_id: AssetDefinitionId,
        asset_scale: u8,
        minor_units: u64,
    ) -> Result<Self, ValidationFeeAdmissionError> {
        Ok(Self {
            treasury_account_id,
            fee_asset_definition_id,
            asset_scale,
            amount: quantity_from_policy_minor_units(minor_units, asset_scale)?,
        })
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
    InvalidPolicyInvariant(&'static str),
    WrongPolicyNetwork {
        expected: String,
        found: String,
    },
    MissingPolicyGenesis,
    WrongPolicyGenesis {
        expected_hash_hex: String,
        found_hash_hex: String,
    },
    PolicyExpired {
        expires_after_height: u64,
        current_height: u64,
    },
    TreasuryPayoutRequiresActiveContractSubject {
        treasury_account_id: String,
    },
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
            Self::InvalidPolicyInvariant(reason) => {
                write!(f, "validation-fee policy is invalid: {reason}")
            }
            Self::WrongPolicyNetwork { expected, found } => write!(
                f,
                "validation-fee policy network mismatch: expected {expected}, found {found}"
            ),
            Self::MissingPolicyGenesis => {
                write!(
                    f,
                    "validation-fee policy cannot be genesis-bound because no committed genesis hash is available"
                )
            }
            Self::WrongPolicyGenesis {
                expected_hash_hex,
                found_hash_hex,
            } => write!(
                f,
                "validation-fee policy genesis mismatch: expected {expected_hash_hex}, found {found_hash_hex}"
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
    amount: Numeric,
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

    let credited_minor_units = enforce_policy_with_credit(tx, &policy).map_err(|err| {
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
    let credit = ValidationFeeCredit::from_policy_minor_units(
        policy_treasury_account_id(&policy).map_err(admission_rejection)?,
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
                .downcast_ref::<CastParliamentBallot>()
                .is_some()
            || instruction
                .as_any()
                .downcast_ref::<ApproveGovernanceProposal>()
                .is_some()
            || instruction
                .as_any()
                .downcast_ref::<FinalizeReferendum>()
                .is_some()
            || instruction
                .as_any()
                .downcast_ref::<EnactReferendum>()
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
/// Deferred executables do not retain transaction-level fee-coordinate metadata, so each
/// execution context must contain one unambiguous authority-paid treasury transfer with the exact
/// aggregate fee. This deliberately makes pre-activation fee-free work fail closed after policy
/// activation.
pub(crate) fn enforce_deferred_instruction_list(
    authority: &AccountId,
    instructions: &[InstructionBox],
    state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<(), TransactionRejectionReason> {
    let Some(policy) = active_policy(state_transaction)? else {
        return Ok(());
    };

    let credited_minor_units =
        enforce_deferred_policy_with_credit(authority, instructions, &policy)
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
        let credit = ValidationFeeCredit::from_policy_minor_units(
            policy_treasury_account_id(&policy).map_err(admission_rejection)?,
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
            scheduled_time_trigger: false,
        }
    }

    /// Bind opaque execution to a trigger event, admitting the payout exemption
    /// only when consensus invoked the contract from a scheduled Time trigger.
    pub(crate) fn from_trigger_event(
        runtime_context: &'a crate::executor::ContractRuntimeExecutionContext,
        code_bytes: &'a [u8],
        event: &iroha_data_model::events::EventBox,
    ) -> Self {
        Self {
            runtime_context,
            code_bytes,
            scheduled_time_trigger: matches!(event, iroha_data_model::events::EventBox::Time(_)),
        }
    }

    #[cfg(test)]
    fn scheduled_time_trigger(
        runtime_context: &'a crate::executor::ContractRuntimeExecutionContext,
        code_bytes: &'a [u8],
    ) -> Self {
        Self {
            runtime_context,
            code_bytes,
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
    let Some(policy) = active_policy(state_transaction)? else {
        return Ok(OpaqueDeferredValidationOutcome::Apply);
    };
    let treasury_payout_binding = verified_opaque_treasury_payout_binding(
        &policy,
        state_transaction,
        runtime_origin.as_ref(),
    )
    .map_err(admission_rejection)?;
    enforce_opaque_deferred_policy(
        instruction_groups,
        &policy,
        treasury_payout_binding
            .as_ref()
            .map(|binding| &binding.treasury_account_id),
    )
    .map_err(admission_rejection)?;

    let fee_asset_definition_id =
        policy_fee_asset_definition_id(&policy).map_err(admission_rejection)?;
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

    if let Some(binding) = treasury_payout_binding.as_ref() {
        if ordered_instructions.is_empty() && instruction_groups.is_empty() {
            return Ok(OpaqueDeferredValidationOutcome::NoOp);
        }
        validate_treasury_payout_effect_plan(instruction_groups, ordered_instructions, binding)
            .map_err(admission_rejection)?;
        let credit = ValidationFeeCredit {
            treasury_account_id: binding.treasury_account_id.clone(),
            fee_asset_definition_id,
            asset_scale: policy.ds_scale,
            amount: binding.batch_sbd.clone(),
        };
        match consume_validation_fee_credit(state_transaction, &credit) {
            Ok(()) => {}
            Err(ValidationFeeAdmissionError::InsufficientCreditBalance { .. }) => {
                return Ok(OpaqueDeferredValidationOutcome::NoOp);
            }
            Err(error) => return Err(admission_rejection(error)),
        }
    }
    Ok(OpaqueDeferredValidationOutcome::Apply)
}

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

    for (authority, instructions) in instruction_groups {
        let allowed_treasury =
            allowed_treasury_payout_authority.filter(|treasury| authority == *treasury);
        reject_opaque_fee_asset_effects(
            authority,
            instructions,
            &fee_asset_definition_id,
            allowed_treasury,
        )?;
    }
    Ok(())
}

fn validate_treasury_payout_effect_plan(
    instruction_groups: &std::collections::BTreeMap<AccountId, Vec<InstructionBox>>,
    ordered_instructions: &[(AccountId, InstructionBox)],
    binding: &ValidationFeeTreasuryPayoutBindingV1,
) -> Result<(), ValidationFeeAdmissionError> {
    let mismatch =
        |reason| ValidationFeeAdmissionError::TreasuryPayoutEffectPlanMismatch { reason };
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
    collect_instruction_asset_transfers(instructions, 0, &binding.sbd_asset_id, &mut collection)?;
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

    let sbd_leg = &collection.transfers[0];
    if sbd_leg.asset_definition_id != binding.sbd_asset_id
        || sbd_leg.source_account_id != binding.treasury_account_id
        || sbd_leg.destination_account_id != binding.pool_vault_account_id
        || sbd_leg.amount != *binding.batch_sbd.as_numeric()
    {
        return Err(mismatch(
            "instruction 0 must be the exact bound SBD treasury-to-vault batch",
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
    let amount_out = Quantity::from_canonical_numeric(xor_return.amount.clone())
        .map_err(|_| mismatch("the XOR output must be a non-negative canonical quantity"))?;
    if amount_out < binding.min_xor_out || amount_out > binding.max_xor_out {
        return Err(mismatch(
            "the XOR output is outside the signed slippage bounds",
        ));
    }

    let recipients = binding.recipients.as_slice();
    let payout0 = amount_out
        .try_mul_decimal(&recipients[0].share)
        .map_err(|_| ValidationFeeAdmissionError::TreasuryPayoutArithmeticFailure)?;
    let payout1 = amount_out
        .try_mul_decimal(&recipients[1].share)
        .map_err(|_| ValidationFeeAdmissionError::TreasuryPayoutArithmeticFailure)?;
    let payout2 = amount_out
        .try_mul_decimal(&recipients[2].share)
        .map_err(|_| ValidationFeeAdmissionError::TreasuryPayoutArithmeticFailure)?;
    let payout3 = amount_out
        .checked_sub(&payout0)
        .and_then(|remaining| remaining.checked_sub(&payout1))
        .and_then(|remaining| remaining.checked_sub(&payout2))
        .map_err(|_| ValidationFeeAdmissionError::TreasuryPayoutArithmeticFailure)?;
    let expected_payouts = [payout0, payout1, payout2, payout3];

    for (offset, (recipient, expected_amount)) in
        recipients.iter().zip(expected_payouts.iter()).enumerate()
    {
        if expected_amount.is_zero() {
            return Err(mismatch("every bound validator payout must be non-zero"));
        }
        let transfer = &collection.transfers[offset + 2];
        if transfer.asset_definition_id != binding.xor_asset_id
            || transfer.source_account_id != binding.treasury_account_id
            || transfer.destination_account_id != recipient.account_id
            || transfer.amount != *expected_amount.as_numeric()
        {
            return Err(mismatch(
                "instructions 2 through 5 must match the ordered validator shares exactly",
            ));
        }
    }
    Ok(())
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
                MultisigInstructionBox::Register(_) | MultisigInstructionBox::Cancel(_) => {}
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
fn enforce_deferred_policy(
    authority: &AccountId,
    instructions: &[InstructionBox],
    policy: &ValidationFeePolicyV1,
) -> Result<(), ValidationFeeAdmissionError> {
    enforce_deferred_policy_with_credit(authority, instructions, policy).map(|_| ())
}

fn enforce_deferred_policy_with_credit(
    authority: &AccountId,
    instructions: &[InstructionBox],
    policy: &ValidationFeePolicyV1,
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
        let marker_fee_coordinate = multisig_marker_coordinate_for_context(
            context_index,
            context,
            policy,
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
    let parameter_id = ValidationFeePolicyRegistryV1::parameter_id();
    let Some(custom) = state_transaction
        .world
        .parameters()
        .custom()
        .get(&parameter_id)
    else {
        return Ok(None);
    };
    let Some(registry) = ValidationFeePolicyRegistryV1::from_custom_parameter(custom) else {
        return Err(admission_rejection(
            ValidationFeeAdmissionError::MalformedPolicyRegistryParameter,
        ));
    };
    registry.validate().map_err(|err| {
        admission_rejection(ValidationFeeAdmissionError::InvalidPolicyRegistry(
            err.to_string(),
        ))
    })?;
    for entry in &registry.registered_policies {
        validate_registry_entry_governance(entry, state_transaction)
            .map_err(admission_rejection)?;
    }

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
    if policy.chain_id != state_transaction.chain_id {
        return Err(admission_rejection(
            ValidationFeeAdmissionError::WrongPolicyNetwork {
                expected: state_transaction.chain_id.to_string(),
                found: policy.chain_id.to_string(),
            },
        ));
    }
    validate_policy_genesis_hash(&policy, state_transaction.block_hashes())
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

fn validate_registry_entry_governance(
    entry: &ValidationFeePolicyRegistryEntryV1,
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<(), ValidationFeeAdmissionError> {
    use iroha_data_model::governance::types::{
        ProposalKind, ValidationFeePayoutLifecycleProposal, ValidationFeePolicyProposal,
    };

    let lifecycle_id = entry
        .payout_lifecycle
        .as_ref()
        .map(|reference| reference.parliament_authorization.proposal_id);
    let policy_kind = ProposalKind::ValidationFeePolicy(ValidationFeePolicyProposal {
        policy: entry.policy.clone(),
        payout_lifecycle_proposal_id: lifecycle_id,
    });
    validate_parliament_authorization(
        &entry.parliament_authorization,
        &policy_kind,
        state_transaction,
    )?;

    match (
        entry.policy.treasury_payout_binding.as_ref(),
        entry.payout_lifecycle.as_ref(),
    ) {
        (None, None) if entry.policy.charging_mode == ValidationFeeChargingMode::Disabled => Ok(()),
        (Some(binding), Some(reference)) => {
            let lifecycle_kind =
                ProposalKind::ValidationFeePayoutLifecycle(ValidationFeePayoutLifecycleProposal {
                    payout_binding: binding.clone(),
                });
            validate_parliament_authorization(
                &reference.parliament_authorization,
                &lifecycle_kind,
                state_transaction,
            )
        }
        _ => Err(ValidationFeeAdmissionError::InvalidPolicyRegistry(
            "enabled policy requires paired payout binding and Parliament lifecycle authorization"
                .to_owned(),
        )),
    }
}

fn validate_parliament_authorization(
    authorization: &ValidationFeeParliamentAuthorizationV1,
    exact_kind: &iroha_data_model::governance::types::ProposalKind,
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<(), ValidationFeeAdmissionError> {
    use iroha_data_model::governance::types::ParliamentBody;

    if let Some(reason) = authorization.invariant_error() {
        return Err(ValidationFeeAdmissionError::InvalidPolicyRegistry(
            reason.to_owned(),
        ));
    }
    let fingerprint = exact_kind.fingerprint();
    if fingerprint != authorization.proposal_fingerprint || authorization.proposal_id != fingerprint
    {
        return Err(ValidationFeeAdmissionError::InvalidPolicyRegistry(
            "stored proposal id/fingerprint does not match the exact typed proposal preimage"
                .to_owned(),
        ));
    }
    let proposal = state_transaction
        .world
        .governance_proposals
        .get(&authorization.proposal_id)
        .ok_or_else(|| {
            ValidationFeeAdmissionError::InvalidPolicyRegistry(
                "authorized governance proposal is missing".to_owned(),
            )
        })?;
    if &proposal.kind != exact_kind
        || proposal.status != crate::state::GovernanceProposalStatus::Enacted
        || proposal.enacted_at_height != Some(authorization.enacted_at_height)
    {
        return Err(ValidationFeeAdmissionError::InvalidPolicyRegistry(
            "authorized governance proposal payload, status, or enactment height differs from the registry"
                .to_owned(),
        ));
    }
    let snapshot = proposal.parliament_snapshot.as_ref().ok_or_else(|| {
        ValidationFeeAdmissionError::InvalidPolicyRegistry(
            "authorized governance proposal has no Parliament snapshot".to_owned(),
        )
    })?;
    let snapshot_bytes = norito::to_bytes(&snapshot.bodies).map_err(|_| {
        ValidationFeeAdmissionError::InvalidPolicyRegistry(
            "Parliament snapshot cannot be encoded".to_owned(),
        )
    })?;
    let digest = Blake2b512::digest(snapshot_bytes);
    let mut computed_roster_root = [0; 32];
    computed_roster_root.copy_from_slice(&digest[..32]);
    if computed_roster_root != snapshot.roster_root
        || snapshot.roster_root != authorization.proposal_time_roster_root
        || snapshot.bodies.selection_epoch != snapshot.selection_epoch
    {
        return Err(ValidationFeeAdmissionError::InvalidPolicyRegistry(
            "proposal-time Parliament roster commitment differs from retained consensus state"
                .to_owned(),
        ));
    }

    const REQUIRED_BODIES: [ParliamentBody; 7] = [
        ParliamentBody::RulesCommittee,
        ParliamentBody::AgendaCouncil,
        ParliamentBody::InterestPanel,
        ParliamentBody::ReviewPanel,
        ParliamentBody::PolicyJury,
        ParliamentBody::OversightCommittee,
        ParliamentBody::FmaCommittee,
    ];
    if REQUIRED_BODIES
        .iter()
        .any(|body| !snapshot.bodies.rosters.contains_key(body))
    {
        return Err(ValidationFeeAdmissionError::InvalidPolicyRegistry(
            "proposal-time Parliament snapshot does not contain all seven required bodies"
                .to_owned(),
        ));
    }
    let referendum_id = hex::encode(authorization.proposal_id);
    let referendum = state_transaction
        .world
        .governance_referenda
        .get(&referendum_id)
        .ok_or_else(|| {
            ValidationFeeAdmissionError::InvalidPolicyRegistry(
                "authorized referendum is missing".to_owned(),
            )
        })?;
    if referendum.h_start != authorization.referendum_window.lower
        || referendum.h_end != authorization.referendum_window.upper
        || referendum.status != crate::state::GovernanceReferendumStatus::Closed
    {
        return Err(ValidationFeeAdmissionError::InvalidPolicyRegistry(
            "authorized referendum window or finalized status differs from retained consensus state"
                .to_owned(),
        ));
    }
    let approvals = state_transaction
        .world
        .governance_stage_approvals
        .get(&referendum_id)
        .ok_or_else(|| {
            ValidationFeeAdmissionError::InvalidPolicyRegistry(
                "authorized Parliament approval records are missing".to_owned(),
            )
        })?;
    if REQUIRED_BODIES.iter().copied().any(|body| {
        !approvals.quorum_met(body, snapshot.selection_epoch)
            || approvals.rejection_quorum_met(body, snapshot.selection_epoch)
    }) {
        return Err(ValidationFeeAdmissionError::InvalidPolicyRegistry(
            "all seven Parliament bodies do not retain affirmative quorum".to_owned(),
        ));
    }
    let finalized = proposal.finalization_evidence.as_ref().ok_or_else(|| {
        ValidationFeeAdmissionError::InvalidPolicyRegistry(
            "authorized proposal finalization evidence is missing".to_owned(),
        )
    })?;
    let typed_finalization = ValidationFeeFinalizationEvidenceV1 {
        referendum_id: finalized.referendum_id,
        finalized_at_height: finalized.finalized_at_height,
        mode: match finalized.mode {
            iroha_data_model::isi::governance::VotingMode::Zk => {
                ValidationFeeGovernanceVotingModeV1::Zk
            }
            iroha_data_model::isi::governance::VotingMode::Plain => {
                ValidationFeeGovernanceVotingModeV1::Plain
            }
        },
        approve: finalized.approve,
        reject: finalized.reject,
        abstain: finalized.abstain,
        min_turnout: finalized.min_turnout,
        approval_threshold_numerator: finalized.approval_threshold_numerator,
        approval_threshold_denominator: finalized.approval_threshold_denominator,
        approved: finalized.approved,
    };
    if finalized.proposal_id != authorization.proposal_id
        || typed_finalization != authorization.finalization
    {
        return Err(ValidationFeeAdmissionError::InvalidPolicyRegistry(
            "finalized referendum evidence differs from the typed registry authorization"
                .to_owned(),
        ));
    }
    Ok(())
}

fn validate_treasury_payout_contract_subject(
    policy: &ValidationFeePolicyV1,
    state_transaction: &StateTransaction<'_, '_>,
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
    let Some(record) = crate::smartcontracts::code::fetch_bound_contract_record(
        state_transaction,
        &binding.contract_address,
    ) else {
        return Err(
            ValidationFeeAdmissionError::TreasuryPayoutRequiresActiveContractSubject {
                treasury_account_id: treasury.to_string(),
            },
        );
    };
    if record.contract_address != binding.contract_address
        || record.contract_subject != treasury
        || binding.treasury_account_id != treasury
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
    let sbd_credit = ValidationFeeCredit {
        treasury_account_id: binding.treasury_account_id.clone(),
        fee_asset_definition_id: binding.sbd_asset_id.clone(),
        asset_scale: policy.ds_scale,
        amount: binding.batch_sbd.clone(),
    };
    validation_fee_credit_asset_spec(state_transaction, &sbd_credit)?;
    let xor_definition = state_transaction
        .world
        .asset_definition(&binding.xor_asset_id)
        .map_err(
            |_| ValidationFeeAdmissionError::TreasuryPayoutRuntimeBindingMismatch {
                reason: "the bound XOR asset definition is missing",
            },
        )?;
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

fn verified_opaque_treasury_payout_binding(
    policy: &ValidationFeePolicyV1,
    state_transaction: &StateTransaction<'_, '_>,
    runtime_origin: Option<&OpaqueDeferredRuntimeOrigin<'_>>,
) -> Result<Option<ValidationFeeTreasuryPayoutBindingV1>, ValidationFeeAdmissionError> {
    if !treasury_payout_exemption_enabled(policy) {
        return Ok(None);
    }
    let binding = policy.treasury_payout_binding.as_ref().ok_or(
        ValidationFeeAdmissionError::TreasuryPayoutRuntimeBindingMismatch {
            reason: "the active Parliament policy has no typed payout binding",
        },
    )?;
    let Some(origin) = runtime_origin else {
        return Ok(None);
    };
    if !origin.scheduled_time_trigger
        || origin.runtime_context.contract_address != binding.contract_address
        || origin.runtime_context.contract_subject != binding.treasury_account_id
        || origin.runtime_context.entrypoint != binding.entrypoint.as_ref()
        || <[u8; 32]>::from(Sha256::digest(origin.code_bytes)) != binding.code_hash
    {
        return Ok(None);
    }
    let Some(record) = crate::smartcontracts::code::fetch_bound_contract_record(
        state_transaction,
        &binding.contract_address,
    ) else {
        return Ok(None);
    };
    if record.contract_address != binding.contract_address
        || record.contract_subject != binding.treasury_account_id
        || <[u8; 32]>::from(Sha256::digest(&record.code_bytes)) != binding.code_hash
        || record.code_bytes.as_slice() != origin.code_bytes
    {
        return Ok(None);
    }
    Ok(Some(binding.clone()))
}

fn validate_policy_genesis_hash(
    policy: &ValidationFeePolicyV1,
    committed_block_hashes: &[HashOf<BlockHeader>],
) -> Result<(), ValidationFeeAdmissionError> {
    let Some(genesis_hash) = committed_block_hashes.first().map(|hash| *hash.as_ref()) else {
        return Err(ValidationFeeAdmissionError::MissingPolicyGenesis);
    };
    if policy.genesis_hash != genesis_hash {
        return Err(ValidationFeeAdmissionError::WrongPolicyGenesis {
            expected_hash_hex: hex::encode(genesis_hash),
            found_hash_hex: hex::encode(policy.genesis_hash),
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

/// Derive the exact IVM durable-state path used for the consensus-owned fee-credit counter.
pub(crate) fn validation_fee_credit_state_key_for_address(
    contract_address: &iroha_data_model::smart_contract::ContractAddress,
) -> Name {
    validation_fee_credit_scoped_state_key_for_address(
        contract_address,
        VALIDATION_FEE_CREDIT_STATE_LEAF,
    )
}

fn validation_fee_credit_asset_state_key_for_address(
    contract_address: &iroha_data_model::smart_contract::ContractAddress,
) -> Name {
    validation_fee_credit_scoped_state_key_for_address(
        contract_address,
        VALIDATION_FEE_CREDIT_ASSET_STATE_LEAF,
    )
}

fn validation_fee_credit_scoped_state_key_for_address(
    contract_address: &iroha_data_model::smart_contract::ContractAddress,
    leaf: &str,
) -> Name {
    let digest = hex::encode(Hash::new(contract_address.to_string().as_bytes()).as_ref());
    format!("sc/{digest}/{leaf}")
        .parse()
        .expect("validation-fee credit path must be a valid Name")
}

/// Return whether a durable-state key is the reserved contract-visible fee-credit leaf.
pub(crate) fn is_validation_fee_credit_state_key(key: &Name) -> bool {
    let Some(rest) = key.as_ref().strip_prefix("sc/") else {
        return false;
    };
    let Some((digest, leaf)) = rest.split_once('/') else {
        return false;
    };
    let reserved_leaf =
        leaf == VALIDATION_FEE_CREDIT_STATE_LEAF || leaf == VALIDATION_FEE_CREDIT_ASSET_STATE_LEAF;
    digest.len() == Hash::LENGTH * 2
        && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
        && reserved_leaf
}

fn validation_fee_credit_state_keys(
    state_transaction: &StateTransaction<'_, '_>,
    treasury: &AccountId,
) -> Result<(Name, Name), ValidationFeeAdmissionError> {
    let Some(record) = crate::smartcontracts::code::fetch_bound_contract_record_by_subject(
        state_transaction,
        treasury,
    ) else {
        return Err(
            ValidationFeeAdmissionError::TreasuryPayoutRequiresActiveContractSubject {
                treasury_account_id: treasury.to_string(),
            },
        );
    };
    Ok((
        validation_fee_credit_state_key_for_address(&record.contract_address),
        validation_fee_credit_asset_state_key_for_address(&record.contract_address),
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
    state_transaction: &StateTransaction<'_, '_>,
    credit: &ValidationFeeCredit,
) -> Result<NumericSpec, ValidationFeeAdmissionError> {
    let expected = NumericSpec::try_fractional(u32::from(credit.asset_scale)).map_err(|_| {
        ValidationFeeAdmissionError::InvalidPolicyInvariant(
            "validation-fee policy asset scale exceeds the numeric domain",
        )
    })?;
    let definition = state_transaction
        .world
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
    let (key, asset_key) =
        validation_fee_credit_state_keys(state_transaction, &credit.treasury_account_id)?;
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
        validation_fee_credit_state_keys(state_transaction, &credit.treasury_account_id)
            .map_err(admission_rejection)?;
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
    state_transaction
        .world
        .smart_contract_state
        .insert(asset_key, asset_bytes);
    state_transaction
        .world
        .smart_contract_state
        .insert(key, bytes);
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
    let (key, _) =
        validation_fee_credit_state_keys(state_transaction, &credit.treasury_account_id)?;
    let bytes = encode_validation_fee_credit_state_value(&remaining).map_err(|_| {
        ValidationFeeAdmissionError::MalformedCreditBalance {
            state_key: key.to_string(),
        }
    })?;
    state_transaction
        .world
        .smart_contract_state
        .insert(key, bytes);
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
fn enforce_policy_with_credit(
    tx: &SignedTransaction,
    policy: &ValidationFeePolicyV1,
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
    if metadata_contains_validation_fee {
        validate_policy_metadata(tx.metadata(), policy)?;
    }
    let fee_coordinate = validation_fee_coordinate(tx.metadata())?;
    let explicit_fee_context_index = fee_coordinate
        .map(|coordinate| {
            resolve_fee_coordinate_context(coordinate, &transfer_collection.transfers)
        })
        .transpose()?;
    let mut requires_policy_metadata = false;
    let mut credited_minor_units = 0_u64;

    for (context_index, context) in transfer_collection.contexts.iter().enumerate() {
        let transaction_fee_coordinate = if explicit_fee_context_index == Some(context_index) {
            fee_coordinate
        } else {
            None
        };
        let marker_fee_coordinate = multisig_marker_coordinate_for_context(
            context_index,
            context,
            policy,
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
        )?;
        requires_policy_metadata |= validated.requires_policy_metadata;
        if context_index == 0 {
            credited_minor_units = validated.credited_minor_units;
        }
    }

    if requires_policy_metadata && !metadata_contains_validation_fee {
        validate_policy_metadata(tx.metadata(), policy)?;
    }
    Ok(credited_minor_units)
}

fn multisig_marker_coordinate_for_context(
    context_index: usize,
    context: &TransferExecutionContext,
    policy: &ValidationFeePolicyV1,
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

        let required_fee_minor_units = required_fee_minor_units(qualifying_transfer_count, policy)?;
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
        let required_fee_minor_units = required_fee_minor_units(qualifying_transfer_count, policy)?;
        return Err(ValidationFeeAdmissionError::MissingFee {
            required_minor_units: required_fee_minor_units,
        });
    }
    if uncoordinated_fee_candidates.len() > 1 {
        return Err(ValidationFeeAdmissionError::DuplicateFeeInstructions {
            count: uncoordinated_fee_candidates.len(),
        });
    }

    let required_fee_minor_units = required_fee_minor_units(qualifying_transfer_count, policy)?;
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
) -> Result<u64, ValidationFeeAdmissionError> {
    let per_transfer_minor_units =
        numeric_to_minor_units(policy.fee.as_numeric(), policy.ds_scale, usize::MAX)
            .map_err(|_| ValidationFeeAdmissionError::RequiredFeeOverflow)?;
    u64::try_from(
        (qualifying_transfer_count as u128)
            .checked_mul(u128::from(per_transfer_minor_units))
            .ok_or(ValidationFeeAdmissionError::RequiredFeeOverflow)?,
    )
    .map_err(|_| ValidationFeeAdmissionError::RequiredFeeOverflow)
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
            RepoInstructionBox::Reverse(isi)
                if repo_touches_fee_asset(
                    isi.cash_leg.asset_definition_id(),
                    isi.collateral_leg.asset_definition_id(),
                    fee_asset_definition_id,
                ) =>
            {
                return Some(ReverseRepoIsi::WIRE_ID);
            }
            RepoInstructionBox::Initiate(_)
            | RepoInstructionBox::Reverse(_)
            | RepoInstructionBox::MarginCall(_) => {}
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

    if let Some(isi) = instruction.as_any().downcast_ref::<ReverseRepoIsi>()
        && repo_touches_fee_asset(
            isi.cash_leg.asset_definition_id(),
            isi.collateral_leg.asset_definition_id(),
            fee_asset_definition_id,
        )
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
            SettlementInstructionBox::Dvp(_)
            | SettlementInstructionBox::Pvp(_)
            | SettlementInstructionBox::SetFxCorridorPolicy(_)
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
            MultisigInstructionBox::Register(_) | MultisigInstructionBox::Cancel(_) => {
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
            // Numeric mint is supply-changing and can also charge an implicit account fee.
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

    // Repo and settlement paths with no policy-DS leg were audited above. Margin calls do not
    // move assets. Any policy-DS leg was already rejected.
    audited_no_ds_effect!(
        RepoInstructionBox,
        RepoIsi,
        ReverseRepoIsi,
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
    // Freezes, blacklists, and limits do not change balances, but applying them to the policy DS
    // would encumber treasury/user holdings outside the signed transfer-and-fee effect.
    reject_fee_asset_transfer_control!(iroha_data_model::isi::SetAssetTransferFreeze);
    reject_fee_asset_transfer_control!(iroha_data_model::isi::SetAssetTransferBlacklist);
    reject_fee_asset_transfer_control!(iroha_data_model::isi::SetAssetTransferControl);

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
        iroha_data_model::isi::SetAssetDefinitionBalancePolicy,
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
        iroha_data_model::isi::escrow::DrawdownAssetLock,
        iroha_data_model::isi::escrow::CancelAssetLock,
        iroha_data_model::isi::escrow::ExpireAssetLock,
        iroha_data_model::isi::escrow::OpenAnonymousAssetEscrow,
        iroha_data_model::isi::escrow::ReleaseAnonymousAssetEscrow,
        iroha_data_model::isi::escrow::CancelAnonymousAssetEscrow,
        iroha_data_model::isi::escrow::ResolveAnonymousEscrowDispute,
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
        iroha_data_model::isi::zk::SubmitZkAceAuthorizedTransfer,
        iroha_data_model::isi::zk::RegisterZkAsset,
        iroha_data_model::isi::zk::RegisterAssetHiddenZkPool,
        iroha_data_model::isi::zk::ScheduleConfidentialPolicyTransition,
        iroha_data_model::isi::zk::CancelConfidentialPolicyTransition,
        iroha_data_model::isi::zk::Shield,
        iroha_data_model::isi::zk::ZkTransfer,
        iroha_data_model::isi::zk::AssetHiddenZkTransfer,
        iroha_data_model::isi::zk::Unshield,
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
                            amount: entry.amount().as_numeric().clone(),
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
                    .expect("explicit asset-transfer disposition must contain a numeric transfer");
                collection.transfers.push(AssetTransferSummary {
                    context_index,
                    instruction_index,
                    entry_index: None,
                    asset_definition_id: transfer.source.definition.clone(),
                    source_account_id: transfer.source.account.clone(),
                    destination_account_id: transfer.destination.clone(),
                    amount: transfer.object.as_numeric().clone(),
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
            let amount_minor_units = numeric_to_minor_units(
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

fn numeric_to_minor_units(
    amount: &Numeric,
    policy_scale: u8,
    instruction_index: usize,
) -> Result<u64, ValidationFeeAdmissionError> {
    let mantissa = amount
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

fn format_entry_index(entry_index: Option<usize>) -> String {
    entry_index.map_or_else(String::new, |entry_index| format!("/{entry_index}"))
}

#[cfg(test)]
mod tests {
    use std::str::FromStr as _;

    use iroha_crypto::{Algorithm, Hash, KeyPair};
    use iroha_data_model::{
        ChainId,
        asset::{AssetDefinitionId, AssetId},
        domain::DomainId,
        events::execute_trigger::ExecuteTriggerEventFilter,
        isi::{
            InstructionBox, Transfer, TransferAssetBatchEntry,
            offline::{RedeemKagemushaRecursiveV4, TopUpKagemushaRecursiveV4},
            repo::RepoMarginCallIsi,
            settlement::{SettlementLeg, SettlementPlan},
        },
        offline::{
            KAGEMUSHA_CONFIDENTIAL_PROOF_BACKEND, KAGEMUSHA_RECURSIVE_SPEND_OPERATION_LIMBS_V4,
            KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4,
            KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_PROOF_ENVELOPE_VERSION_V4,
            KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V4,
            KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_VERSION_V2,
            KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V2,
            KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V4,
            KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4,
            KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4, KagemushaAndroidKeyMintHardwareAssertionV1,
            KagemushaOnlineHardwareAssertionV1, KagemushaPastaCycleParityV1,
            KagemushaPastaCycleProofEnvelopeV4, KagemushaRecursiveSpendArtifactBindingV4,
            KagemushaRecursiveSpendBranchClaimV2, KagemushaRecursiveSpendBundleV4,
            KagemushaRecursiveSpendOperationVectorV4, KagemushaRecursiveSpendProofV4,
            KagemushaRecursiveSpendPublicStatementV4, KagemushaRecursiveSpendRedeemRequestV4,
            KagemushaRecursiveSpendRedeemUnsignedV4, KagemushaRecursiveSpendRedemptionIntentV4,
            KagemushaRecursiveSpendStateBoundaryV2, KagemushaRecursiveSpendTopUpAnchorRefV2,
            KagemushaRecursiveSpendTopUpRequestV4, KagemushaRecursiveSpendTopUpUnsignedV4,
            KagemushaRequestAuthorizationV2, KagemushaScaledAmountV2,
            KagemushaSpendableNoteDescriptorV2, KagemushaUnshieldPublicInputsBindingV2,
            kagemusha_confidential_amount_encoding_v2,
            kagemusha_recursive_spend_verifier_key_id_v4,
        },
        prelude::Register,
        proof::{ProofAttachment, ProofBox, VerifyingKeyId},
        repo::{RepoCashLeg, RepoCollateralLeg, RepoGovernance},
        transaction::{
            Executable, IvmBytecode, IvmProved, TransactionBuilder, executable::ContractInvocation,
        },
        trigger::{
            Trigger,
            action::{Action, Repeats},
        },
        validation_fee::{
            VALIDATION_FEE_INSTRUCTION_INDEX_METADATA_KEY, VALIDATION_FEE_POLICY_HASH_METADATA_KEY,
            VALIDATION_FEE_POLICY_SCHEMA_VERSION, VALIDATION_FEE_POLICY_VERSION_METADATA_KEY,
            VALIDATION_FEE_TRANSFER_ENTRY_INDEX_METADATA_KEY, ValidationFeeFinalizationEvidenceV1,
            ValidationFeeGovernanceVotingModeV1, ValidationFeeGovernanceWindowV1,
            ValidationFeeParliamentAuthorizationV1, ValidationFeePayoutLifecycleReferenceV1,
            ValidationFeePolicyRegistryEntryV1, ValidationFeePolicyRegistryV1,
            ValidationFeeTreasuryPayoutRecipientV1,
        },
    };
    use iroha_executor_data_model::isi::multisig::{MultisigApprove, MultisigPropose};
    use iroha_primitives::json::Json;

    use super::*;

    const TEST_VALIDATION_FEE_ASSET_SCALE: u8 =
        iroha_data_model::validation_fee::VALIDATION_FEE_DS_SCALE;
    const TEST_VALIDATION_FEE_MINOR_UNITS: u64 = 10;

    fn key_pair(seed: u8) -> KeyPair {
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).expect("key pair")
    }

    fn account(seed: u8) -> AccountId {
        let key_pair = key_pair(seed);
        AccountId::new(key_pair.public_key().clone())
    }

    fn asset_definition(name: &str) -> AssetDefinitionId {
        AssetDefinitionId::new(
            DomainId::try_new("fees", "paynet").expect("domain id"),
            Name::from_str(name).expect("asset name"),
        )
    }

    fn fee_asset() -> AssetDefinitionId {
        asset_definition("fee_token")
    }

    fn policy(treasury: &AccountId) -> ValidationFeePolicyV1 {
        ValidationFeePolicyV1 {
            schema_version: VALIDATION_FEE_POLICY_SCHEMA_VERSION,
            chain_id: "generic-testnet".into(),
            genesis_hash: [7; 32],
            policy_version: 1,
            previous_policy_hash: None,
            ds_asset_id: fee_asset(),
            ds_scale: TEST_VALIDATION_FEE_ASSET_SCALE,
            fee: iroha_data_model::validation_fee::initial_validation_fee_amount(),
            treasury_account_id: treasury.clone(),
            charging_mode: ValidationFeeChargingMode::PerQualifyingTransferInstruction,
            effective_from_height: 10,
            expires_after_height: Some(100),
            exemption_classes: Vec::new(),
            treasury_payout_binding: None,
        }
    }

    fn xor_asset() -> AssetDefinitionId {
        asset_definition("xor")
    }

    fn test_contract_address() -> iroha_data_model::smart_contract::ContractAddress {
        iroha_data_model::smart_contract::ContractAddress::derive(
            iroha_config::parameters::defaults::common::chain_discriminant(),
            &account(9),
            42,
            iroha_data_model::nexus::DataSpaceId::UNIVERSAL,
        )
        .expect("test contract address")
    }

    fn treasury_payout_binding(
        contract_address: iroha_data_model::smart_contract::ContractAddress,
        code: &[u8],
    ) -> ValidationFeeTreasuryPayoutBindingV1 {
        let treasury = contract_address.subject_id();
        ValidationFeeTreasuryPayoutBindingV1 {
            contract_address,
            code_hash: <[u8; 32]>::from(Sha256::digest(code)),
            entrypoint: "autonomous_validation_fee_tick"
                .parse()
                .expect("payout entrypoint"),
            treasury_account_id: treasury,
            sbd_asset_id: fee_asset(),
            xor_asset_id: xor_asset(),
            pool_vault_account_id: account(2),
            batch_sbd: iroha_data_model::validation_fee::validation_fee_payout_batch_sbd(),
            min_xor_out: Quantity::from(4_u64),
            max_xor_out: Quantity::from(100_u64),
            recipients: (3..=6)
                .map(|seed| ValidationFeeTreasuryPayoutRecipientV1 {
                    account_id: account(seed),
                    share: "0.25".parse().expect("validator share"),
                })
                .collect(),
        }
    }

    fn policy_with_treasury_payout_lifecycle(
        binding: ValidationFeeTreasuryPayoutBindingV1,
    ) -> ValidationFeePolicyV1 {
        let mut policy = policy(&binding.treasury_account_id);
        policy
            .exemption_classes
            .push(VALIDATION_FEE_TREASURY_PAYOUT_EXEMPTION_CLASS.to_string());
        policy.treasury_payout_binding = Some(binding);
        policy
    }

    fn policy_fee_asset(policy: &ValidationFeePolicyV1) -> AssetDefinitionId {
        policy.ds_asset_id.clone()
    }

    fn successor_policy(previous: &ValidationFeePolicyV1) -> ValidationFeePolicyV1 {
        let mut policy = previous.clone();
        policy.policy_version += 1;
        policy.previous_policy_hash = Some(previous.policy_hash().expect("previous policy hash"));
        policy.effective_from_height += 100;
        policy.expires_after_height = Some(policy.effective_from_height + 100);
        policy
    }

    fn test_parliament_bodies() -> iroha_data_model::governance::types::ParliamentBodies {
        use iroha_data_model::governance::types::{
            ParliamentBodies, ParliamentBody, ParliamentRoster,
        };

        let member = account(250);
        let rosters = [
            ParliamentBody::RulesCommittee,
            ParliamentBody::AgendaCouncil,
            ParliamentBody::InterestPanel,
            ParliamentBody::ReviewPanel,
            ParliamentBody::PolicyJury,
            ParliamentBody::OversightCommittee,
            ParliamentBody::FmaCommittee,
        ]
        .into_iter()
        .map(|body| {
            (
                body,
                ParliamentRoster {
                    body,
                    epoch: 1,
                    members: vec![member.clone()],
                    alternates: Vec::new(),
                    verified: 1,
                    candidate_count: 1,
                    derived_by: Default::default(),
                },
            )
        })
        .collect();
        ParliamentBodies {
            selection_epoch: 1,
            rosters,
        }
    }

    fn test_roster_root() -> [u8; 32] {
        let encoded =
            norito::to_bytes(&test_parliament_bodies()).expect("encode Parliament bodies");
        let digest = Blake2b512::digest(encoded);
        let mut root = [0; 32];
        root.copy_from_slice(&digest[..32]);
        root
    }

    fn test_authorization(proposal_id: [u8; 32]) -> ValidationFeeParliamentAuthorizationV1 {
        ValidationFeeParliamentAuthorizationV1 {
            proposal_id,
            proposal_fingerprint: proposal_id,
            proposal_time_roster_root: test_roster_root(),
            referendum_window: ValidationFeeGovernanceWindowV1 {
                lower: 1,
                upper: 1_000,
            },
            finalization: ValidationFeeFinalizationEvidenceV1 {
                referendum_id: proposal_id,
                finalized_at_height: 8,
                mode: ValidationFeeGovernanceVotingModeV1::Plain,
                approve: 1,
                reject: 0,
                abstain: 0,
                min_turnout: 1,
                approval_threshold_numerator: 1,
                approval_threshold_denominator: 2,
                approved: true,
            },
            enacted_at_height: 9,
        }
    }

    fn policy_registry(policies: &[ValidationFeePolicyV1]) -> ValidationFeePolicyRegistryV1 {
        let registered_policies = policies
            .iter()
            .map(|policy| {
                use iroha_data_model::governance::types::{
                    ProposalKind, ValidationFeePayoutLifecycleProposal, ValidationFeePolicyProposal,
                };

                let payout_lifecycle = policy.treasury_payout_binding.as_ref().map(|binding| {
                    let lifecycle_seal = binding
                        .lifecycle_seal()
                        .expect("derive payout lifecycle seal");
                    let lifecycle_kind = ProposalKind::ValidationFeePayoutLifecycle(
                        ValidationFeePayoutLifecycleProposal {
                            payout_binding: binding.clone(),
                        },
                    );
                    let lifecycle_id = lifecycle_kind.fingerprint();
                    ValidationFeePayoutLifecycleReferenceV1 {
                        lifecycle_seal,
                        parliament_authorization: test_authorization(lifecycle_id),
                    }
                });
                let lifecycle_id = payout_lifecycle
                    .as_ref()
                    .map(|reference| reference.parliament_authorization.proposal_id);
                let kind = ProposalKind::ValidationFeePolicy(ValidationFeePolicyProposal {
                    policy: policy.clone(),
                    payout_lifecycle_proposal_id: lifecycle_id,
                });
                let proposal_id = kind.fingerprint();
                ValidationFeePolicyRegistryEntryV1::from_enactment(
                    policy.clone(),
                    test_authorization(proposal_id),
                    payout_lifecycle,
                )
                .expect("registry entry")
            })
            .collect::<Vec<_>>();
        ValidationFeePolicyRegistryV1 {
            registered_policies,
        }
    }

    fn seed_authorized_proposal(
        kind: iroha_data_model::governance::types::ProposalKind,
        authorization: ValidationFeeParliamentAuthorizationV1,
        state_tx: &mut StateTransaction<'_, '_>,
    ) {
        use iroha_data_model::{
            governance::types::{GovernanceFinalizationEvidence, ParliamentBody},
            isi::governance::VotingMode,
        };

        let proposal_id = authorization.proposal_id;
        let referendum_id = hex::encode(proposal_id);
        let bodies = test_parliament_bodies();
        state_tx.world.governance_proposals.insert(
            proposal_id,
            crate::state::GovernanceProposalRecord {
                proposer: account(250),
                kind,
                created_height: 1,
                status: crate::state::GovernanceProposalStatus::Enacted,
                pipeline: crate::state::GovernancePipeline::default(),
                parliament_snapshot: Some(crate::state::GovernanceParliamentSnapshot {
                    selection_epoch: 1,
                    beacon: [0x44; 32],
                    roster_root: authorization.proposal_time_roster_root,
                    bodies,
                }),
                finalization_evidence: Some(GovernanceFinalizationEvidence {
                    proposal_id,
                    referendum_id: proposal_id,
                    finalized_at_height: authorization.finalization.finalized_at_height,
                    mode: match authorization.finalization.mode {
                        ValidationFeeGovernanceVotingModeV1::Zk => VotingMode::Zk,
                        ValidationFeeGovernanceVotingModeV1::Plain => VotingMode::Plain,
                    },
                    approve: authorization.finalization.approve,
                    reject: authorization.finalization.reject,
                    abstain: authorization.finalization.abstain,
                    min_turnout: authorization.finalization.min_turnout,
                    approval_threshold_numerator: authorization
                        .finalization
                        .approval_threshold_numerator,
                    approval_threshold_denominator: authorization
                        .finalization
                        .approval_threshold_denominator,
                    approved: authorization.finalization.approved,
                }),
                enacted_at_height: Some(authorization.enacted_at_height),
            },
        );
        state_tx.world.governance_referenda.insert(
            referendum_id.clone(),
            crate::state::GovernanceReferendumRecord {
                h_start: authorization.referendum_window.lower,
                h_end: authorization.referendum_window.upper,
                status: crate::state::GovernanceReferendumStatus::Closed,
                mode: match authorization.finalization.mode {
                    ValidationFeeGovernanceVotingModeV1::Zk => {
                        crate::state::GovernanceReferendumMode::Zk
                    }
                    ValidationFeeGovernanceVotingModeV1::Plain => {
                        crate::state::GovernanceReferendumMode::Plain
                    }
                },
            },
        );
        let mut approvals = crate::state::GovernanceStageApprovals::default();
        for body in [
            ParliamentBody::RulesCommittee,
            ParliamentBody::AgendaCouncil,
            ParliamentBody::InterestPanel,
            ParliamentBody::ReviewPanel,
            ParliamentBody::PolicyJury,
            ParliamentBody::OversightCommittee,
            ParliamentBody::FmaCommittee,
        ] {
            approvals
                .ensure_stage(body, 1, 1, 10_000)
                .record(account(250));
        }
        state_tx
            .world
            .governance_stage_approvals
            .insert(referendum_id, approvals);
    }

    fn install_policy_registry_fixture(
        registry: &ValidationFeePolicyRegistryV1,
        state_tx: &mut StateTransaction<'_, '_>,
    ) {
        use iroha_data_model::governance::types::{
            ProposalKind, ValidationFeePayoutLifecycleProposal, ValidationFeePolicyProposal,
        };

        for entry in &registry.registered_policies {
            if let (Some(binding), Some(reference)) = (
                entry.policy.treasury_payout_binding.as_ref(),
                entry.payout_lifecycle.as_ref(),
            ) {
                seed_authorized_proposal(
                    ProposalKind::ValidationFeePayoutLifecycle(
                        ValidationFeePayoutLifecycleProposal {
                            payout_binding: binding.clone(),
                        },
                    ),
                    reference.parliament_authorization,
                    state_tx,
                );
            }
            seed_authorized_proposal(
                ProposalKind::ValidationFeePolicy(ValidationFeePolicyProposal {
                    policy: entry.policy.clone(),
                    payout_lifecycle_proposal_id: entry
                        .payout_lifecycle
                        .as_ref()
                        .map(|reference| reference.parliament_authorization.proposal_id),
                }),
                entry.parliament_authorization,
                state_tx,
            );
        }
        state_tx
            .world
            .parameters
            .get_mut()
            .set_parameter(Parameter::Custom(registry.clone().into_custom_parameter()));
    }

    fn block_hash(bytes: [u8; 32]) -> HashOf<BlockHeader> {
        HashOf::from_untyped_unchecked(Hash::prehashed(bytes))
    }

    fn minimal_bound_contract_artifact() -> (
        Vec<u8>,
        iroha_data_model::smart_contract::manifest::ContractManifest,
    ) {
        let metadata = ivm::ProgramMetadata {
            version_major: 1,
            version_minor: 1,
            mode: 0,
            vector_length: 0,
            max_cycles: 1,
            abi_version: 1,
        };
        let entrypoint = iroha_data_model::smart_contract::manifest::EntrypointDescriptor {
            name: "autonomous_validation_fee_tick".to_owned(),
            kind: iroha_data_model::smart_contract::manifest::EntryPointKind::Kotoage,
            params: Vec::new(),
            argument_schema: None,
            return_type: None,
            return_schema: None,
            permission: Some("CanInvokeValidationFeePayout".to_owned()),
            read_keys: Vec::new(),
            write_keys: Vec::new(),
            access_hints_complete: None,
            access_hints_skipped: Vec::new(),
            triggers: Vec::new(),
        };
        let interface = ivm::EmbeddedContractInterfaceV1 {
            seiyaku_name: "ValidationFeePayout".to_owned(),
            compiler_fingerprint: "validation-fee-bound-contract-test".to_owned(),
            abi_hash: ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1),
            features_bitmap: 0,
            access_set_hints: None,
            kotoba: Vec::new(),
            entrypoints: vec![ivm::EmbeddedEntrypointDescriptor {
                name: entrypoint.name.clone(),
                kind: entrypoint.kind,
                params: entrypoint.params.clone(),
                argument_schema: entrypoint.argument_schema.clone(),
                return_type: entrypoint.return_type.clone(),
                return_schema: entrypoint.return_schema.clone(),
                permission: entrypoint.permission.clone(),
                read_keys: entrypoint.read_keys.clone(),
                write_keys: entrypoint.write_keys.clone(),
                access_hints_complete: entrypoint.access_hints_complete,
                access_hints_skipped: entrypoint.access_hints_skipped.clone(),
                triggers: entrypoint.triggers.clone(),
                entry_pc: 0,
            }],
            error_codes: Vec::new(),
            states: Vec::new(),
        };
        let mut instructions = Vec::new();
        instructions.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
        let mut artifact = metadata.encode();
        artifact.extend_from_slice(&interface.encode_section());
        artifact.extend_from_slice(&instructions);
        let verified =
            ivm::verify_contract_artifact(&artifact).expect("valid bound contract artifact");
        (artifact, verified.manifest)
    }

    fn validation_fee_payout_world(deployer: &AccountId) -> crate::state::World {
        use iroha_data_model::prelude::{Account, AssetDefinition, Domain};

        let contract_domain =
            Domain::new(DomainId::try_new("contracts", "universal").expect("contract domain id"))
                .build(deployer);
        let fee_domain =
            Domain::new(DomainId::try_new("fees", "paynet").expect("fee-asset domain id"))
                .build(deployer);
        let mut accounts = vec![Account::new(deployer.clone()).build(deployer)];
        accounts.extend((2..=7).map(|seed| Account::new(account(seed)).build(deployer)));
        let fee_definition = AssetDefinition::new(
            fee_asset(),
            NumericSpec::fractional(u32::from(TEST_VALIDATION_FEE_ASSET_SCALE)),
        )
        .build(deployer);
        let xor_definition = AssetDefinition::new(
            xor_asset(),
            NumericSpec::fractional(u32::from(TEST_VALIDATION_FEE_ASSET_SCALE)),
        )
        .build(deployer);
        crate::state::World::with(
            [contract_domain, fee_domain],
            accounts,
            [fee_definition, xor_definition],
        )
    }

    fn install_active_bound_validation_fee_policy(
        state_tx: &mut StateTransaction<'_, '_>,
        deployer: &AccountId,
        deployer_key: &KeyPair,
    ) -> ValidationFeePolicyV1 {
        use iroha_data_model::{
            nexus::DataSpaceId, prelude::Account, smart_contract::ContractAddress,
        };

        let deployment_permission: iroha_data_model::permission::Permission =
            iroha_executor_data_model::permission::smart_contract::CanRegisterSmartContractCode
                .into();
        crate::smartcontracts::Execute::execute(
            iroha_data_model::isi::Grant::account_permission(
                deployment_permission,
                deployer.clone(),
            ),
            deployer,
            state_tx,
        )
        .expect("grant contract lifecycle authority");
        let (code, manifest) = minimal_bound_contract_artifact();
        let code_hash =
            crate::smartcontracts::code::register_code_bytes(deployer, code.clone(), state_tx)
                .expect("register contract bytes");
        crate::smartcontracts::code::register_manifest(
            deployer,
            manifest.signed(deployer_key),
            state_tx,
        )
        .expect("register signed contract manifest");
        let contract_address = ContractAddress::derive(
            iroha_config::parameters::defaults::common::chain_discriminant(),
            deployer,
            0,
            DataSpaceId::UNIVERSAL,
        )
        .expect("contract address");
        crate::smartcontracts::code::activate_instance(
            deployer,
            contract_address.clone(),
            code_hash,
            state_tx,
        )
        .expect("activate contract instance");

        let binding = treasury_payout_binding(contract_address, &code);
        crate::smartcontracts::Execute::execute(
            Register::account(Account::new(binding.treasury_account_id.clone())),
            deployer,
            state_tx,
        )
        .expect("register immutable contract subject account");
        let policy = policy_with_treasury_payout_lifecycle(binding);
        install_policy_registry_fixture(&policy_registry(std::slice::from_ref(&policy)), state_tx);
        policy
    }

    fn minor_units(value: u64) -> Numeric {
        Numeric::new(value, u32::from(TEST_VALIDATION_FEE_ASSET_SCALE))
    }

    fn quantity_minor_units(value: u64) -> Quantity {
        Quantity::try_from_numeric(minor_units(value))
            .expect("validation-fee fixture quantity must be non-negative")
    }

    fn transfer(
        from: &AccountId,
        asset_definition: &AssetDefinitionId,
        amount: Numeric,
        to: &AccountId,
    ) -> InstructionBox {
        Transfer::asset_quantity(
            AssetId::new(asset_definition.clone(), from.clone()),
            Quantity::try_from_numeric(amount)
                .expect("validation-fee fixture amount must be non-negative"),
            to.clone(),
        )
        .into()
    }

    fn canonical_treasury_payout_plan(
        binding: &ValidationFeeTreasuryPayoutBindingV1,
        xor_out: Quantity,
    ) -> Vec<InstructionBox> {
        let payout0 = xor_out
            .try_mul_decimal(&binding.recipients[0].share)
            .expect("first exact validator share");
        let payout1 = xor_out
            .try_mul_decimal(&binding.recipients[1].share)
            .expect("second exact validator share");
        let payout2 = xor_out
            .try_mul_decimal(&binding.recipients[2].share)
            .expect("third exact validator share");
        let payout3 = xor_out
            .checked_sub(&payout0)
            .and_then(|remaining| remaining.checked_sub(&payout1))
            .and_then(|remaining| remaining.checked_sub(&payout2))
            .expect("deterministic final-validator remainder");
        let payouts = [payout0, payout1, payout2, payout3];
        let mut instructions = vec![
            transfer(
                &binding.treasury_account_id,
                &binding.sbd_asset_id,
                binding.batch_sbd.as_numeric().clone(),
                &binding.pool_vault_account_id,
            ),
            transfer(
                &binding.pool_vault_account_id,
                &binding.xor_asset_id,
                xor_out.as_numeric().clone(),
                &binding.treasury_account_id,
            ),
        ];
        instructions.extend(
            binding
                .recipients
                .iter()
                .zip(payouts)
                .map(|(recipient, amount)| {
                    transfer(
                        &binding.treasury_account_id,
                        &binding.xor_asset_id,
                        amount.as_numeric().clone(),
                        &recipient.account_id,
                    )
                }),
        );
        instructions
    }

    fn ordered_treasury_payout_plan(
        binding: &ValidationFeeTreasuryPayoutBindingV1,
        instructions: &[InstructionBox],
    ) -> Vec<(AccountId, InstructionBox)> {
        instructions
            .iter()
            .cloned()
            .map(|instruction| (binding.treasury_account_id.clone(), instruction))
            .collect()
    }

    fn assert_treasury_payout_plan_mismatch(
        binding: &ValidationFeeTreasuryPayoutBindingV1,
        groups: &std::collections::BTreeMap<AccountId, Vec<InstructionBox>>,
        ordered: &[(AccountId, InstructionBox)],
    ) {
        assert!(matches!(
            validate_treasury_payout_effect_plan(groups, ordered, binding),
            Err(ValidationFeeAdmissionError::TreasuryPayoutEffectPlanMismatch { .. })
                | Err(ValidationFeeAdmissionError::TreasuryPayoutArithmeticFailure)
        ));
    }

    fn kagemusha_artifact_binding() -> KagemushaRecursiveSpendArtifactBindingV4 {
        KagemushaRecursiveSpendArtifactBindingV4 {
            version: KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4,
            generation: "validation-fee-kagemusha-v4".to_owned(),
            manifest_sha256: [0xA1; 32],
        }
    }

    fn kagemusha_authorization(
        authority: AccountId,
        asset_definition_id: AssetDefinitionId,
        operation_id: [u8; 32],
        payload_digest: [u8; 32],
    ) -> KagemushaRequestAuthorizationV2 {
        KagemushaRequestAuthorizationV2 {
            authority,
            device_id: "validation-fee-kagemusha-device".to_owned(),
            asset_definition_id,
            operation_id,
            issued_at_ms: 1,
            expires_at_ms: 2,
            nonce: [0xA2; 32],
            payload_digest,
            registration_hash: [0xA3; 32],
            hardware_assertion: KagemushaOnlineHardwareAssertionV1::AndroidKeyMint(
                KagemushaAndroidKeyMintHardwareAssertionV1 {
                    signature:
                        iroha_data_model::offline::KagemushaDeviceSignatureV2::from_raw_bytes(
                            &[1; 64],
                        )
                        .expect("canonical low-S fixture signature"),
                },
            ),
        }
    }

    fn kagemusha_top_up_request(
        asset_definition_id: &AssetDefinitionId,
    ) -> KagemushaRecursiveSpendTopUpRequestV4 {
        let payer = account(1);
        let chain_id = ChainId::from("generic-testnet");
        let amount = KagemushaScaledAmountV2::new(500, u32::from(TEST_VALIDATION_FEE_ASSET_SCALE))
            .expect("positive top-up amount");
        let operation_id = [0xA4; 32];
        let mut shield_proof = ProofAttachment::new_ref(
            KAGEMUSHA_CONFIDENTIAL_PROOF_BACKEND.into(),
            ProofBox::new(KAGEMUSHA_CONFIDENTIAL_PROOF_BACKEND.to_owned(), vec![0xA5]),
            VerifyingKeyId::new(
                KAGEMUSHA_CONFIDENTIAL_PROOF_BACKEND,
                "validation-fee-topup-shield",
            ),
        );
        shield_proof.vk_commitment = Some([0xA6; 32]);
        let unsigned = KagemushaRecursiveSpendTopUpUnsignedV4 {
            version: KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4,
            asset: AssetId::new(asset_definition_id.clone(), payer.clone()),
            amount,
            current_note: KagemushaSpendableNoteDescriptorV2 {
                chain_id,
                asset: asset_definition_id.clone(),
                note_commitment: [0xA7; 32],
                spend_nullifier: [0xA8; 32],
                amount,
            },
            shield_evidence: iroha_data_model::offline::KagemushaTopUpShieldEvidenceV2 {
                initial_root: [0xA9; 32],
                finalized_root: [0xAA; 32],
                leaf_index: 0,
                proof: shield_proof,
            },
            artifact_binding: kagemusha_artifact_binding(),
            operation_id,
        };
        let payload_digest = unsigned.digest().expect("valid top-up payload");
        unsigned
            .into_request(kagemusha_authorization(
                payer,
                asset_definition_id.clone(),
                operation_id,
                payload_digest,
            ))
            .expect("valid top-up request")
    }

    fn kagemusha_redeem_request(
        asset_definition_id: &AssetDefinitionId,
    ) -> KagemushaRecursiveSpendRedeemRequestV4 {
        let recipient = account(1);
        let chain_id = ChainId::from("generic-testnet");
        let amount = KagemushaScaledAmountV2::new(500, u32::from(TEST_VALIDATION_FEE_ASSET_SCALE))
            .expect("positive redemption amount");
        let operation_id = [0xB1; 32];
        let topup_anchor_ref = KagemushaRecursiveSpendTopUpAnchorRefV2 {
            topup_operation_id: [0xB2; 32],
            anchor_digest: [0xB3; 32],
        };
        let branch_claim =
            KagemushaRecursiveSpendBranchClaimV2::root(topup_anchor_ref.anchor_digest)
                .expect("canonical root branch claim");
        let note = KagemushaSpendableNoteDescriptorV2 {
            chain_id: chain_id.clone(),
            asset: asset_definition_id.clone(),
            note_commitment: [0xB4; 32],
            spend_nullifier: [0xB5; 32],
            amount,
        };
        let binding = kagemusha_artifact_binding();
        let verifier_key_id = kagemusha_recursive_spend_verifier_key_id_v4(
            KagemushaPastaCycleParityV1::StepEq,
            binding.manifest_sha256,
        );
        let statement = KagemushaRecursiveSpendPublicStatementV4 {
            chain_id: chain_id.clone(),
            asset: asset_definition_id.clone(),
            asset_scale: u32::from(TEST_VALIDATION_FEE_ASSET_SCALE),
            final_root: [0xB6; 32],
            next_zero_leaf_index: 1,
            topup_anchor_refs: vec![topup_anchor_ref.clone()],
            proof_step_count: 1,
            peer_hop_count: 0,
            current_note: note.clone(),
            branch_claims: vec![branch_claim.clone()],
            transition: None,
            artifact_binding: binding.clone(),
            verifier_key_id: verifier_key_id.clone(),
        };
        let public_statement_digest = statement.digest().expect("valid public statement");
        let mut state_limbs = vec![0; KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V2];
        state_limbs[0] = KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_VERSION_V2;
        let mut operation_limbs = [0; KAGEMUSHA_RECURSIVE_SPEND_OPERATION_LIMBS_V4];
        operation_limbs[0] = 1;
        let bundle = KagemushaRecursiveSpendBundleV4 {
            statement,
            operation: KagemushaRecursiveSpendOperationVectorV4 {
                limbs: operation_limbs,
            },
            recursive_proof: KagemushaRecursiveSpendProofV4 {
                verifier_key_id,
                public_statement_digest,
                proof_envelope: KagemushaPastaCycleProofEnvelopeV4 {
                    version: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_PROOF_ENVELOPE_VERSION_V4,
                    proof_backend: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4.to_owned(),
                    transcript_profile: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V4
                        .to_owned(),
                    step_eq_circuit_id: KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4.to_owned(),
                    step_ep_circuit_id: KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V4.to_owned(),
                    artifact_generation: binding.generation.clone(),
                    manifest_sha256: binding.manifest_sha256,
                    step_eq_parameter_generation: "validation-fee-eq-params".to_owned(),
                    step_ep_parameter_generation: "validation-fee-ep-params".to_owned(),
                    step_eq_circuit_params_sha256: [0xB7; 32],
                    step_ep_circuit_params_sha256: [0xB8; 32],
                    step_eq_verifier_key_sha256: [0xB9; 32],
                    step_ep_verifier_key_sha256: [0xBA; 32],
                    state_boundary: KagemushaRecursiveSpendStateBoundaryV2::new(state_limbs)
                        .expect("valid state boundary"),
                    proof: ProofBox::new(
                        KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4.to_owned(),
                        vec![0xBB],
                    ),
                },
            },
        };
        let bundle_digest = bundle.digest().expect("valid recursive bundle");
        let unshield_public_inputs = KagemushaUnshieldPublicInputsBindingV2 {
            input_commitment_0: note.note_commitment,
            input_commitment_1: [0; 32],
            nullifier_0: note.spend_nullifier,
            nullifier_1: [0; 32],
            change_output_commitment: [0; 32],
            root: [0xB6; 32],
            public_amount: kagemusha_confidential_amount_encoding_v2(amount.atomic_units),
            asset_tag: [0xBC; 32],
            chain_tag: [0xBD; 32],
        };
        let redemption = KagemushaRecursiveSpendRedemptionIntentV4 {
            chain_id,
            asset: asset_definition_id.clone(),
            input_note: note,
            parent_branch_claims: vec![branch_claim],
            parent_topup_anchor_refs: vec![topup_anchor_ref],
            parent_proof_step_count: 1,
            parent_peer_hop_count: 0,
            parent_bundle_digest: bundle_digest,
            input_root: [0xB6; 32],
            recipient: recipient.clone(),
            public_amount: amount,
            change_output: None,
            change_artifact_binding: None,
            unshield_public_inputs_digest: unshield_public_inputs
                .digest()
                .expect("valid unshield public inputs"),
            unshield_public_inputs,
            operation_id,
        };
        let mut redeem_proof = ProofAttachment::new_ref(
            KAGEMUSHA_CONFIDENTIAL_PROOF_BACKEND.into(),
            ProofBox::new(KAGEMUSHA_CONFIDENTIAL_PROOF_BACKEND.to_owned(), vec![0xBE]),
            VerifyingKeyId::new(
                KAGEMUSHA_CONFIDENTIAL_PROOF_BACKEND,
                "validation-fee-unshield",
            ),
        );
        redeem_proof.vk_commitment = Some([0xBF; 32]);
        let unsigned = KagemushaRecursiveSpendRedeemUnsignedV4 {
            version: KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4,
            bundle,
            recipient: recipient.clone(),
            amount,
            redeem_proof,
            redemption,
            offline_change: None,
            block_height: 10,
            operation_id,
        };
        let payload_digest = unsigned.digest().expect("valid redemption payload");
        unsigned
            .into_request(kagemusha_authorization(
                recipient,
                asset_definition_id.clone(),
                operation_id,
                payload_digest,
            ))
            .expect("valid redemption request")
    }

    fn repo_initiate(
        agreement: &str,
        initiator: &AccountId,
        counterparty: &AccountId,
        cash_asset: &AssetDefinitionId,
        collateral_asset: &AssetDefinitionId,
    ) -> RepoIsi {
        RepoIsi::new(
            agreement.parse().expect("repo agreement id"),
            initiator.clone(),
            counterparty.clone(),
            None,
            RepoCashLeg {
                asset_definition_id: cash_asset.clone(),
                quantity: Quantity::from(1_u64),
            },
            RepoCollateralLeg::new(collateral_asset.clone(), Quantity::from(1_u64)),
            0,
            1_000,
            RepoGovernance::with_defaults(0, 0),
        )
    }

    fn repo_reverse(
        agreement: &str,
        initiator: &AccountId,
        counterparty: &AccountId,
        cash_asset: &AssetDefinitionId,
        collateral_asset: &AssetDefinitionId,
    ) -> ReverseRepoIsi {
        ReverseRepoIsi::new(
            agreement.parse().expect("repo agreement id"),
            initiator.clone(),
            counterparty.clone(),
            RepoCashLeg {
                asset_definition_id: cash_asset.clone(),
                quantity: Quantity::from(1_u64),
            },
            RepoCollateralLeg::new(collateral_asset.clone(), Quantity::from(1_u64)),
            1_000,
        )
    }

    fn settlement_leg(
        asset_definition_id: &AssetDefinitionId,
        from: &AccountId,
        to: &AccountId,
    ) -> SettlementLeg {
        SettlementLeg::new(asset_definition_id.clone(), 1_u64, from.clone(), to.clone())
    }

    fn tx(
        authority_seed: u8,
        instructions: Vec<InstructionBox>,
        metadata: Metadata,
    ) -> SignedTransaction {
        let key_pair = key_pair(authority_seed);
        let chain: ChainId = "generic-testnet".parse().expect("chain id");
        TransactionBuilder::new(
            chain,
            AccountId::new(key_pair.public_key().clone()),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions(instructions)
        .with_metadata(metadata)
        .sign(key_pair.private_key())
    }

    fn contract_call_tx(authority_seed: u8, metadata: Metadata) -> SignedTransaction {
        let key_pair = key_pair(authority_seed);
        let chain: ChainId = "generic-testnet".parse().expect("chain id");
        TransactionBuilder::new(
            chain,
            AccountId::new(key_pair.public_key().clone()),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_executable(Executable::ContractCall(ContractInvocation {
            contract_address: "tairac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjqddcyq8"
                .parse()
                .expect("contract address"),
            expected_code_hash: iroha_crypto::Hash::new(b"validation-fee-contract-code"),
            entrypoint: "send_transfer".to_owned(),
            arguments: None,
        }))
        .with_metadata(metadata)
        .sign(key_pair.private_key())
    }

    fn ivm_tx(authority_seed: u8, metadata: Metadata) -> SignedTransaction {
        let key_pair = key_pair(authority_seed);
        let chain: ChainId = "generic-testnet".parse().expect("chain id");
        TransactionBuilder::new(
            chain,
            AccountId::new(key_pair.public_key().clone()),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_executable(Executable::Ivm(IvmBytecode::from_compiled(vec![0x00])))
        .with_metadata(metadata)
        .sign(key_pair.private_key())
    }

    fn ivm_proved_tx(
        authority_seed: u8,
        overlay: Vec<InstructionBox>,
        metadata: Metadata,
    ) -> SignedTransaction {
        let key_pair = key_pair(authority_seed);
        let chain: ChainId = "generic-testnet".parse().expect("chain id");
        TransactionBuilder::new(
            chain,
            AccountId::new(key_pair.public_key().clone()),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_executable(Executable::IvmProved(IvmProved {
            bytecode: IvmBytecode::from_compiled(vec![0x00]),
            overlay: overlay.into(),
            events_commitment: Hash::new(b"events"),
            gas_policy_commitment: Hash::new(b"gas-policy"),
        }))
        .with_metadata(metadata)
        .sign(key_pair.private_key())
    }

    fn metadata_for(policy: &ValidationFeePolicyV1) -> Metadata {
        let mut metadata = Metadata::default();
        metadata.insert(
            Name::from_str(VALIDATION_FEE_POLICY_VERSION_METADATA_KEY).expect("metadata key"),
            Json::new(policy.policy_version),
        );
        metadata.insert(
            Name::from_str(VALIDATION_FEE_POLICY_HASH_METADATA_KEY).expect("metadata key"),
            Json::new(hex::encode(policy.policy_hash().expect("policy hash"))),
        );
        metadata
    }

    fn metadata_for_fee_instruction(
        policy: &ValidationFeePolicyV1,
        instruction_index: usize,
    ) -> Metadata {
        let mut metadata = metadata_for(policy);
        metadata.insert(
            Name::from_str(VALIDATION_FEE_INSTRUCTION_INDEX_METADATA_KEY).expect("metadata key"),
            Json::new(u64::try_from(instruction_index).expect("instruction index fits u64")),
        );
        metadata
    }

    fn metadata_for_fee_instruction_coordinate(instruction_index: usize) -> Metadata {
        let mut metadata = Metadata::default();
        metadata.insert(
            Name::from_str(VALIDATION_FEE_INSTRUCTION_INDEX_METADATA_KEY).expect("metadata key"),
            Json::new(u64::try_from(instruction_index).expect("instruction index fits u64")),
        );
        metadata
    }

    fn metadata_for_fee_batch_entry(
        policy: &ValidationFeePolicyV1,
        instruction_index: usize,
        entry_index: usize,
    ) -> Metadata {
        let mut metadata = metadata_for_fee_instruction(policy, instruction_index);
        metadata.insert(
            Name::from_str(VALIDATION_FEE_TRANSFER_ENTRY_INDEX_METADATA_KEY).expect("metadata key"),
            Json::new(u64::try_from(entry_index).expect("entry index fits u64")),
        );
        metadata
    }

    fn with_multisig_fee_marker(
        policy: &ValidationFeePolicyV1,
        mut instructions: Vec<InstructionBox>,
        fee_instruction_index: usize,
        fee_entry_index: Option<usize>,
    ) -> Vec<InstructionBox> {
        instructions.push(
            ValidationFeeMultisigMarkerV1::new(
                policy.policy_version,
                policy.policy_hash().expect("policy hash"),
                u64::try_from(fee_instruction_index).expect("instruction index fits u64"),
                fee_entry_index.map(|index| u64::try_from(index).expect("entry index fits u64")),
            )
            .into_instruction(),
        );
        instructions
    }

    #[test]
    fn newly_dispatchable_native_instruction_fails_until_explicitly_classified() {
        let treasury = account(3);
        let policy = policy(&treasury);
        let instruction: InstructionBox = iroha_data_model::isi::InvalidInstruction::new(
            "future.native.instruction",
            [0xAB; 32],
            "classification coverage sentinel",
        )
        .into();
        let type_name = core::any::type_name::<iroha_data_model::isi::InvalidInstruction>();

        assert_eq!(
            crate::smartcontracts::isi::registered_native_instruction_type_name(&instruction),
            Some(type_name),
            "coverage sentinel must be on the real native dispatch surface"
        );
        assert_eq!(
            native_instruction_ds_effect_disposition(&instruction, &policy_fee_asset(&policy)),
            NativeInstructionDsEffectDisposition::UnclassifiedDispatchable(type_name)
        );
        assert_eq!(
            enforce_policy(&tx(1, vec![instruction], Metadata::default()), &policy),
            Err(ValidationFeeAdmissionError::UnclassifiedNativeInstruction {
                context_index: 0,
                instruction_index: 0,
                registered_type_name: Some(type_name),
            })
        );
    }

    #[test]
    fn custom_instruction_without_effect_disposition_fails_closed() {
        let treasury = account(3);
        let policy = policy(&treasury);
        let instruction: InstructionBox =
            CustomInstruction::new(Json::new("unclassified custom effect")).into();

        assert_eq!(
            native_instruction_ds_effect_disposition(&instruction, &policy_fee_asset(&policy)),
            NativeInstructionDsEffectDisposition::Unknown
        );
        assert_eq!(
            enforce_policy(&tx(1, vec![instruction], Metadata::default()), &policy),
            Err(ValidationFeeAdmissionError::UnclassifiedNativeInstruction {
                context_index: 0,
                instruction_index: 0,
                registered_type_name: None,
            })
        );
    }

    #[test]
    fn active_policy_allows_balance_neutral_permissionless_contract_deployment_steps() {
        use iroha_data_model::{
            isi::smart_contract_code::{
                ActivateContractInstance, CancelSmartContractCodeUpload, CommitContractDeployment,
                FinalizeSmartContractCodeUpload, RegisterSmartContractBytes,
                RegisterSmartContractCode, UploadSmartContractCodeChunk,
            },
            smart_contract::manifest::ContractManifest,
        };

        let treasury = account(3);
        let policy = policy(&treasury);
        let code_hash = Hash::new(b"permissionless-contract-artifact");
        let contract_address: iroha_data_model::smart_contract::ContractAddress =
            "tairac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjqddcyq8"
                .parse()
                .expect("contract address");
        let instructions: Vec<InstructionBox> = vec![
            RegisterSmartContractBytes {
                code_hash,
                code: Vec::new(),
            }
            .into(),
            UploadSmartContractCodeChunk {
                code_hash,
                total_size: 1,
                chunk_index: 0,
                chunk_count: 1,
                chunk: vec![0],
            }
            .into(),
            FinalizeSmartContractCodeUpload {
                code_hash,
                total_size: 1,
                chunk_count: 1,
            }
            .into(),
            CancelSmartContractCodeUpload { code_hash }.into(),
            RegisterSmartContractCode {
                manifest: ContractManifest {
                    seiyaku_name: None,
                    code_hash: Some(code_hash),
                    abi_hash: None,
                    compiler_fingerprint: None,
                    features_bitmap: None,
                    access_set_hints: None,
                    entrypoints: None,
                    states: None,
                    kotoba: None,
                    error_codes: None,
                    provenance: None,
                },
            }
            .into(),
            ActivateContractInstance {
                contract_address: contract_address.clone(),
                code_hash,
            }
            .into(),
            CommitContractDeployment {
                expected_deploy_nonce: 0,
                contract_address,
                code_hash,
                contract_alias: "payments::universal".parse().expect("contract alias"),
                lease_expiry_ms: None,
                expected_previous_contract_address: None,
            }
            .into(),
        ];

        for instruction in &instructions {
            assert_eq!(
                native_instruction_ds_effect_disposition(instruction, &policy_fee_asset(&policy),),
                NativeInstructionDsEffectDisposition::AuditedNoDsEffect,
            );
        }
        assert_eq!(
            enforce_policy(&tx(1, instructions, Metadata::default()), &policy),
            Ok(()),
        );
    }

    #[test]
    fn active_policy_rejects_contract_rebinding_and_artifact_removal_steps() {
        use iroha_data_model::isi::smart_contract_code::{
            CommitContractDeployment, DeactivateContractInstance, RemoveSmartContractBytes,
        };

        let treasury = account(3);
        let policy = policy(&treasury);
        let code_hash = Hash::new(b"immutable-contract-artifact");
        let contract_address: iroha_data_model::smart_contract::ContractAddress =
            "tairac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjqddcyq8"
                .parse()
                .expect("contract address");
        let instructions: Vec<InstructionBox> = vec![
            DeactivateContractInstance {
                contract_address: contract_address.clone(),
                reason: Some("attempted policy-era rebind".to_owned()),
            }
            .into(),
            RemoveSmartContractBytes {
                code_hash,
                reason: Some("attempted policy-era removal".to_owned()),
            }
            .into(),
            CommitContractDeployment {
                expected_deploy_nonce: 1,
                contract_address: contract_address.clone(),
                code_hash,
                contract_alias: "payments::universal".parse().expect("contract alias"),
                lease_expiry_ms: None,
                expected_previous_contract_address: Some(contract_address),
            }
            .into(),
        ];

        for (index, instruction) in instructions.into_iter().enumerate() {
            let instruction_wire_id = match index {
                0 => core::any::type_name::<DeactivateContractInstance>(),
                1 => core::any::type_name::<RemoveSmartContractBytes>(),
                _ => core::any::type_name::<CommitContractDeployment>(),
            };
            assert_eq!(
                enforce_policy(&tx(1, vec![instruction], Metadata::default()), &policy,),
                Err(
                    ValidationFeeAdmissionError::UnsupportedNativeFeeAssetMovement {
                        context_index: 0,
                        instruction_index: 0,
                        instruction_wire_id,
                    },
                ),
            );
        }
    }

    #[test]
    fn numeric_supply_changes_are_disabled_while_policy_is_active() {
        let user = account(1);
        let treasury = account(3);
        let policy = policy(&treasury);
        let mint: InstructionBox =
            Mint::asset_quantity(1_u64, AssetId::new(policy_fee_asset(&policy), user)).into();
        let instruction_wire_id = core::any::type_name::<MintBox>();

        assert_eq!(
            enforce_policy(&tx(1, vec![mint], Metadata::default()), &policy),
            Err(
                ValidationFeeAdmissionError::UnsupportedNativeFeeAssetMovement {
                    context_index: 0,
                    instruction_index: 0,
                    instruction_wire_id,
                }
            )
        );
    }

    #[test]
    fn policy_ds_transfer_controls_cannot_encumber_balances() {
        let treasury = account(3);
        let policy = policy(&treasury);
        let freeze: InstructionBox = iroha_data_model::isi::SetAssetTransferFreeze::new(
            treasury,
            policy_fee_asset(&policy),
            true,
            Some("encumber policy DS".to_owned()),
        )
        .into();
        let instruction_wire_id =
            core::any::type_name::<iroha_data_model::isi::SetAssetTransferFreeze>();

        assert_eq!(
            enforce_policy(&tx(1, vec![freeze], Metadata::default()), &policy),
            Err(
                ValidationFeeAdmissionError::UnsupportedNativeFeeAssetMovement {
                    context_index: 0,
                    instruction_index: 0,
                    instruction_wire_id,
                }
            )
        );
    }

    #[test]
    fn audited_no_ds_effect_instruction_remains_available() {
        let treasury = account(3);
        let policy = policy(&treasury);
        let log: InstructionBox = Log::new(Level::INFO, "audit-only".to_owned()).into();

        enforce_policy(&tx(1, vec![log], Metadata::default()), &policy)
            .expect("audited no-DS-effect instruction should remain available");
    }

    #[test]
    fn active_policy_admits_publicly_bound_kagemusha_fee_asset_conversions() {
        let treasury = account(3);
        let policy = policy(&treasury);
        let fee_asset = policy_fee_asset(&policy);
        let top_up: InstructionBox =
            TopUpKagemushaRecursiveV4::new(kagemusha_top_up_request(&fee_asset)).into();
        let redeem: InstructionBox =
            RedeemKagemushaRecursiveV4::new(kagemusha_redeem_request(&fee_asset)).into();

        for instruction in [top_up, redeem] {
            assert_eq!(
                native_instruction_ds_effect_disposition(&instruction, &fee_asset),
                NativeInstructionDsEffectDisposition::AuditedKagemushaOfflineConversion,
            );
            let collection = collect_asset_transfers(
                &Executable::Instructions(vec![instruction.clone()].into()),
                &account(1),
                &fee_asset,
            )
            .expect("a publicly bound Kagemusha conversion must be classifiable");
            assert!(
                collection.transfers.is_empty(),
                "closed transparent/escrow conversion is not an account-to-account Transfer ISI",
            );
            enforce_policy(&tx(1, vec![instruction], Metadata::default()), &policy)
                .expect("Kagemusha conversion must remain usable for the policy fee asset");
        }
    }

    #[test]
    fn kagemusha_conversion_admission_rejects_redirected_public_bindings() {
        let treasury = account(3);
        let policy = policy(&treasury);
        let fee_asset = policy_fee_asset(&policy);

        let mut top_up = kagemusha_top_up_request(&fee_asset);
        top_up.authorization.authority = account(2);
        assert_eq!(
            enforce_policy(
                &tx(
                    1,
                    vec![TopUpKagemushaRecursiveV4::new(top_up).into()],
                    Metadata::default(),
                ),
                &policy,
            ),
            Err(
                ValidationFeeAdmissionError::InvalidKagemushaOfflineConversion {
                    context_index: 0,
                    instruction_index: 0,
                    instruction_wire_id: core::any::type_name::<TopUpKagemushaRecursiveV4>(),
                },
            ),
        );

        let mut redeem = kagemusha_redeem_request(&fee_asset);
        redeem.recipient = account(2);
        assert_eq!(
            enforce_policy(
                &tx(
                    1,
                    vec![RedeemKagemushaRecursiveV4::new(redeem).into()],
                    Metadata::default(),
                ),
                &policy,
            ),
            Err(
                ValidationFeeAdmissionError::InvalidKagemushaOfflineConversion {
                    context_index: 0,
                    instruction_index: 0,
                    instruction_wire_id: core::any::type_name::<RedeemKagemushaRecursiveV4>(),
                },
            ),
        );
    }

    #[test]
    fn kagemusha_conversion_does_not_exempt_adjacent_fee_asset_transfers() {
        let user = account(1);
        let recipient = account(2);
        let treasury = account(3);
        let policy = policy(&treasury);
        let fee_asset = policy_fee_asset(&policy);
        let top_up: InstructionBox =
            TopUpKagemushaRecursiveV4::new(kagemusha_top_up_request(&fee_asset)).into();
        let redeem: InstructionBox =
            RedeemKagemushaRecursiveV4::new(kagemusha_redeem_request(&fee_asset)).into();
        let principal = transfer(&user, &fee_asset, Numeric::new(1_u64, 0), &recipient);

        for conversion in [top_up, redeem] {
            assert_eq!(
                enforce_policy(
                    &tx(
                        1,
                        vec![conversion.clone(), principal.clone()],
                        Metadata::default(),
                    ),
                    &policy,
                ),
                Err(ValidationFeeAdmissionError::MissingFee {
                    required_minor_units: TEST_VALIDATION_FEE_MINOR_UNITS,
                }),
                "an adjacent ordinary SBD transfer must still pay the exact validation fee",
            );

            let fee = transfer(
                &user,
                &fee_asset,
                minor_units(TEST_VALIDATION_FEE_MINOR_UNITS),
                &treasury,
            );
            enforce_policy(
                &tx(
                    1,
                    vec![conversion, principal.clone(), fee],
                    metadata_for_fee_instruction(&policy, 2),
                ),
                &policy,
            )
            .expect("the ordinary transfer remains admissible with its exact signed fee");
        }
    }

    #[test]
    fn transfer_to_unregistered_account_is_rejected_as_hidden_fee_candidate() {
        let user = account(1);
        let recipient = account(2);
        let treasury = account(3);
        let policy = policy(&treasury);
        let fee_asset = policy_fee_asset(&policy);
        let collection = collect_asset_transfers(
            &Executable::Instructions(
                vec![transfer(
                    &user,
                    &fee_asset,
                    Numeric::new(1_u64, 0),
                    &recipient,
                )]
                .into(),
            ),
            &user,
            &fee_asset,
        )
        .expect("transparent transfer is classifiable");

        assert_eq!(
            reject_potential_implicit_account_admission_fee_with(&collection, |_| false),
            Err(
                ValidationFeeAdmissionError::PotentialImplicitAccountAdmissionFee {
                    context_index: 0,
                    instruction_index: 0,
                    entry_index: None,
                    destination_account_id: recipient.to_string(),
                }
            )
        );
        reject_potential_implicit_account_admission_fee_with(&collection, |_| true)
            .expect("an already registered recipient cannot derive account-admission fees");
    }

    #[test]
    fn active_policy_is_bound_to_committed_genesis_hash() {
        let treasury = account(3);
        let policy = policy(&treasury);

        validate_policy_genesis_hash(&policy, &[block_hash(policy.genesis_hash)])
            .expect("matching genesis hash should validate");

        assert_eq!(
            validate_policy_genesis_hash(&policy, &[]),
            Err(ValidationFeeAdmissionError::MissingPolicyGenesis)
        );

        let wrong_genesis_hash = block_hash([8u8; 32]);
        let wrong_genesis_hash_bytes = *wrong_genesis_hash.as_ref();

        assert_eq!(
            validate_policy_genesis_hash(&policy, &[wrong_genesis_hash]),
            Err(ValidationFeeAdmissionError::WrongPolicyGenesis {
                expected_hash_hex: hex::encode(wrong_genesis_hash_bytes),
                found_hash_hex: hex::encode(policy.genesis_hash),
            })
        );
    }

    #[test]
    fn active_policy_registry_requires_monotonic_chain() {
        let treasury = account(3);
        let first = policy(&treasury);
        let second = successor_policy(&first);
        let registry = policy_registry(&[first.clone(), second.clone()]);

        registry.validate().expect("valid policy chain");

        let mut skipped = registry.clone();
        skipped.registered_policies[1].policy.policy_version = 3;
        assert!(matches!(
            skipped.validate(),
            Err(iroha_data_model::validation_fee::ValidationFeePolicyRegistryError::UnexpectedPolicyVersion {
                expected: 2,
                found: 3,
            })
        ));

        let mut broken_previous = registry.clone();
        broken_previous.registered_policies[1]
            .policy
            .previous_policy_hash = Some([9; 32]);
        assert!(matches!(
            broken_previous.validate(),
            Err(iroha_data_model::validation_fee::ValidationFeePolicyRegistryError::BrokenPreviousPolicyHash {
                policy_version: 2,
            })
        ));
    }

    #[test]
    fn enacted_initial_policy_remains_inactive_until_delayed_effective_height() {
        let mut future = policy_with_treasury_payout_lifecycle(treasury_payout_binding(
            test_contract_address(),
            b"future-policy-payout",
        ));
        future.effective_from_height =
            9 + iroha_data_model::validation_fee::VALIDATION_FEE_POLICY_ACTIVATION_DELAY_BLOCKS;
        future.expires_after_height = future.effective_from_height.checked_add(100);
        let registry = policy_registry(std::slice::from_ref(&future));
        let state = crate::state::State::new_with_chain_for_testing(
            crate::state::World::default(),
            crate::kura::Kura::blank_kura_for_testing(),
            crate::query::store::LiveQueryStore::start_test(),
            "generic-testnet".parse().expect("chain id"),
        );
        let header = iroha_data_model::block::BlockHeader::new(
            std::num::NonZeroU64::new(9).expect("height"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = state.block(header);
        let mut state_tx = block.transaction();
        install_policy_registry_fixture(&registry, &mut state_tx);

        assert!(
            active_policy(&state_tx)
                .expect("future initial policy is valid")
                .is_none(),
            "the mandatory 120,960-block activation delay must not halt pre-activation writes"
        );
    }

    #[test]
    fn active_policy_window_rejects_expired_policy() {
        let treasury = account(3);
        let policy = policy(&treasury);

        assert!(!policy.is_active_at_height(policy.effective_from_height - 1));
        assert!(policy.is_active_at_height(policy.effective_from_height));
        let successor = successor_policy(&policy);
        assert!(!successor.is_active_at_height(successor.effective_from_height.saturating_sub(1)));
        assert!(
            policy.is_active_at_height(policy.expires_after_height.expect("expiry height") - 1)
        );
        assert!(!policy.is_active_at_height(policy.expires_after_height.expect("expiry height")));
    }

    #[test]
    fn active_policy_requires_exact_fee_and_transaction_bound_metadata() {
        let user = account(1);
        let recipient = account(2);
        let treasury = account(3);
        let policy = policy(&treasury);
        let fee_asset = policy_fee_asset(&policy);
        let tx = tx(
            1,
            vec![
                transfer(&user, &fee_asset, Numeric::new(1u64, 0), &recipient),
                transfer(&user, &fee_asset, minor_units(10), &treasury),
            ],
            metadata_for_fee_instruction(&policy, 1),
        );

        enforce_policy(&tx, &policy).expect("valid fee-bearing transaction");
    }

    #[test]
    fn active_policy_rejects_raw_contract_and_ivm_executables_fail_closed() {
        let treasury = account(3);
        let policy = policy(&treasury);
        let contract_call = contract_call_tx(1, metadata_for(&policy));
        let raw_ivm = ivm_tx(1, metadata_for(&policy));

        assert_eq!(
            enforce_policy(&contract_call, &policy),
            Err(ValidationFeeAdmissionError::UnsupportedExecutable)
        );
        assert_eq!(
            enforce_policy(&raw_ivm, &policy),
            Err(ValidationFeeAdmissionError::UnsupportedExecutable)
        );
    }

    #[test]
    fn native_repo_ds_movements_fail_closed_at_top_level() {
        let initiator = account(1);
        let counterparty = account(2);
        let treasury = account(3);
        let policy = policy(&treasury);
        let fee_asset = policy_fee_asset(&policy);
        let xor = asset_definition("xor");

        let blocked = [
            (
                InstructionBox::from(RepoInstructionBox::Initiate(repo_initiate(
                    "repo_ds_cash",
                    &initiator,
                    &counterparty,
                    &fee_asset,
                    &xor,
                ))),
                RepoIsi::WIRE_ID,
            ),
            (
                InstructionBox::from(RepoInstructionBox::Initiate(repo_initiate(
                    "repo_ds_collateral",
                    &initiator,
                    &counterparty,
                    &xor,
                    &fee_asset,
                ))),
                RepoIsi::WIRE_ID,
            ),
            (
                InstructionBox::from(RepoInstructionBox::Reverse(repo_reverse(
                    "reverse_repo_ds_cash",
                    &initiator,
                    &counterparty,
                    &fee_asset,
                    &xor,
                ))),
                ReverseRepoIsi::WIRE_ID,
            ),
            (
                InstructionBox::from(RepoInstructionBox::Reverse(repo_reverse(
                    "reverse_repo_ds_collateral",
                    &initiator,
                    &counterparty,
                    &xor,
                    &fee_asset,
                ))),
                ReverseRepoIsi::WIRE_ID,
            ),
        ];

        for (instruction, instruction_wire_id) in blocked {
            let transaction = tx(1, vec![instruction], Metadata::default());
            assert_eq!(
                enforce_policy(&transaction, &policy),
                Err(
                    ValidationFeeAdmissionError::UnsupportedNativeFeeAssetMovement {
                        context_index: 0,
                        instruction_index: 0,
                        instruction_wire_id,
                    }
                )
            );
        }

        let non_ds = tx(
            1,
            vec![
                RepoInstructionBox::Initiate(repo_initiate(
                    "repo_non_ds",
                    &initiator,
                    &counterparty,
                    &xor,
                    &xor,
                ))
                .into(),
            ],
            Metadata::default(),
        );
        enforce_policy(&non_ds, &policy).expect("non-DS repo remains generic");

        let margin_call = tx(
            1,
            vec![
                RepoInstructionBox::MarginCall(RepoMarginCallIsi::new(
                    "repo_margin_only".parse().expect("repo agreement id"),
                ))
                .into(),
            ],
            Metadata::default(),
        );
        enforce_policy(&margin_call, &policy).expect("repo margin call has no balance effect");
    }

    #[test]
    fn native_settlement_ds_movements_fail_closed_through_wrappers() {
        let initiator = account(1);
        let counterparty = account(2);
        let treasury = account(3);
        let multisig = account(4);
        let policy = policy(&treasury);
        let fee_asset = policy_fee_asset(&policy);
        let xor = asset_definition("xor");

        let dvp = DvpIsi::new(
            "wrapped_ds_dvp".parse().expect("settlement id"),
            settlement_leg(&xor, &initiator, &counterparty),
            settlement_leg(&fee_asset, &counterparty, &initiator),
            SettlementPlan::default(),
        );
        let proved = ivm_proved_tx(
            1,
            vec![SettlementInstructionBox::Dvp(dvp).into()],
            Metadata::default(),
        );
        assert_eq!(
            enforce_policy(&proved, &policy),
            Err(
                ValidationFeeAdmissionError::UnsupportedNativeFeeAssetMovement {
                    context_index: 0,
                    instruction_index: 0,
                    instruction_wire_id: DvpIsi::WIRE_ID,
                }
            )
        );

        let pvp = PvpIsi::new(
            "multisig_ds_pvp".parse().expect("settlement id"),
            settlement_leg(&xor, &multisig, &counterparty),
            settlement_leg(&fee_asset, &counterparty, &multisig),
            SettlementPlan::default(),
        );
        let proposed = tx(
            1,
            vec![
                MultisigPropose::new(
                    multisig,
                    vec![SettlementInstructionBox::Pvp(pvp).into()],
                    None,
                )
                .into(),
            ],
            Metadata::default(),
        );
        assert_eq!(
            enforce_policy(&proposed, &policy),
            Err(
                ValidationFeeAdmissionError::UnsupportedNativeFeeAssetMovement {
                    context_index: 1,
                    instruction_index: 0,
                    instruction_wire_id: PvpIsi::WIRE_ID,
                }
            )
        );

        let non_ds_dvp = DvpIsi::new(
            "wrapped_non_ds_dvp".parse().expect("settlement id"),
            settlement_leg(&xor, &initiator, &counterparty),
            settlement_leg(&xor, &counterparty, &initiator),
            SettlementPlan::default(),
        );
        let non_ds_proved = ivm_proved_tx(
            1,
            vec![SettlementInstructionBox::Dvp(non_ds_dvp).into()],
            Metadata::default(),
        );
        enforce_policy(&non_ds_proved, &policy).expect("non-DS settlement remains generic");
    }

    #[test]
    fn opaque_trigger_artifacts_reject_native_repo_ds_movement() {
        let initiator = account(1);
        let counterparty = account(2);
        let treasury = account(3);
        let policy = policy(&treasury);
        let fee_asset = policy_fee_asset(&policy);
        let xor = asset_definition("xor");
        let trigger_id: iroha_data_model::trigger::TriggerId =
            "opaque_repo_ds_trigger".parse().expect("trigger id");
        let trigger = Trigger::new(
            trigger_id.clone(),
            Action::new(
                vec![InstructionBox::from(RepoInstructionBox::Initiate(
                    repo_initiate(
                        "trigger_repo_ds",
                        &initiator,
                        &counterparty,
                        &fee_asset,
                        &xor,
                    ),
                ))],
                Repeats::Indefinitely,
                initiator.clone(),
                ExecuteTriggerEventFilter::new().for_trigger(trigger_id),
            ),
        );
        let instruction_groups = std::collections::BTreeMap::from([(
            initiator,
            vec![RegisterBox::Trigger(Register::trigger(trigger)).into()],
        )]);

        assert_eq!(
            enforce_opaque_deferred_policy(&instruction_groups, &policy, None),
            Err(
                ValidationFeeAdmissionError::UnsupportedNativeFeeAssetMovement {
                    context_index: 0,
                    instruction_index: 0,
                    instruction_wire_id: RepoIsi::WIRE_ID,
                }
            )
        );
    }

    #[test]
    fn ivm_proved_overlay_requires_exact_validation_fee() {
        let user = account(1);
        let recipient = account(2);
        let treasury = account(3);
        let policy = policy(&treasury);
        let fee_asset = policy_fee_asset(&policy);

        let exact = ivm_proved_tx(
            1,
            vec![
                transfer(&user, &fee_asset, Numeric::new(1u64, 0), &recipient),
                transfer(&user, &fee_asset, minor_units(10), &treasury),
            ],
            metadata_for_fee_instruction(&policy, 1),
        );
        enforce_policy(&exact, &policy).expect("exact proved-IVM overlay fee should validate");

        let missing = ivm_proved_tx(
            1,
            vec![transfer(
                &user,
                &fee_asset,
                Numeric::new(1u64, 0),
                &recipient,
            )],
            metadata_for(&policy),
        );
        assert_eq!(
            enforce_policy(&missing, &policy),
            Err(ValidationFeeAdmissionError::MissingFee {
                required_minor_units: 10,
            })
        );

        for observed_minor_units in [9, 11] {
            let wrong = ivm_proved_tx(
                1,
                vec![
                    transfer(&user, &fee_asset, Numeric::new(1u64, 0), &recipient),
                    transfer(
                        &user,
                        &fee_asset,
                        minor_units(observed_minor_units),
                        &treasury,
                    ),
                ],
                metadata_for_fee_instruction(&policy, 1),
            );
            assert_eq!(
                enforce_policy(&wrong, &policy),
                Err(ValidationFeeAdmissionError::WrongFeeAmount {
                    expected_minor_units: 10,
                    observed_minor_units,
                })
            );
        }
    }

    #[test]
    fn deferred_instruction_list_requires_exact_execution_time_fee() {
        let user = account(1);
        let recipient = account(2);
        let treasury = account(3);
        let policy = policy(&treasury);
        let fee_asset = policy_fee_asset(&policy);
        let principal = || transfer(&user, &fee_asset, Numeric::new(1u64, 0), &recipient);

        assert_eq!(
            enforce_deferred_policy(&user, &[principal()], &policy),
            Err(ValidationFeeAdmissionError::MissingMultisigFeeMarker { context_index: 0 })
        );
        let missing_fee = with_multisig_fee_marker(&policy, vec![principal()], 1, None);
        assert_eq!(
            enforce_deferred_policy(&user, &missing_fee, &policy),
            Err(ValidationFeeAdmissionError::FeeInstructionNotFound {
                instruction_index: 1,
                entry_index: None,
            })
        );
        let exact = with_multisig_fee_marker(
            &policy,
            vec![
                principal(),
                transfer(&user, &fee_asset, minor_units(10), &treasury),
            ],
            1,
            None,
        );
        enforce_deferred_policy(&user, &exact, &policy)
            .expect("deferred principal and exact fee should validate atomically");

        for observed_minor_units in [9, 11] {
            assert_eq!(
                enforce_deferred_policy(
                    &user,
                    &with_multisig_fee_marker(
                        &policy,
                        vec![
                            principal(),
                            transfer(
                                &user,
                                &fee_asset,
                                minor_units(observed_minor_units),
                                &treasury,
                            ),
                        ],
                        1,
                        None,
                    ),
                    &policy
                ),
                Err(ValidationFeeAdmissionError::WrongFeeAmount {
                    expected_minor_units: 10,
                    observed_minor_units,
                })
            );
        }
    }

    #[test]
    fn deferred_multisig_marker_is_unique_policy_bound_and_batch_aware() {
        let user = account(1);
        let recipient = account(2);
        let treasury = account(3);
        let policy = policy(&treasury);
        let fee_asset = policy_fee_asset(&policy);
        let principal = || transfer(&user, &fee_asset, Numeric::new(1u64, 0), &recipient);
        let fee = || transfer(&user, &fee_asset, minor_units(10), &treasury);

        let mut duplicate = with_multisig_fee_marker(&policy, vec![principal(), fee()], 1, None);
        duplicate.push(
            ValidationFeeMultisigMarkerV1::new(
                policy.policy_version,
                policy.policy_hash().expect("policy hash"),
                1,
                None,
            )
            .into_instruction(),
        );
        assert_eq!(
            enforce_deferred_policy(&user, &duplicate, &policy),
            Err(ValidationFeeAdmissionError::DuplicateMultisigFeeMarkers {
                context_index: 0,
                count: 2,
            })
        );

        let malformed: InstructionBox = Log::new(
            Level::TRACE,
            "iroha:validation_fee:multisig:v1:malformed".to_owned(),
        )
        .into();
        assert_eq!(
            enforce_deferred_policy(&user, &[principal(), fee(), malformed], &policy),
            Err(ValidationFeeAdmissionError::MalformedMultisigFeeMarker {
                context_index: 0,
                instruction_index: 2,
            })
        );

        let wrong_version = vec![
            principal(),
            fee(),
            ValidationFeeMultisigMarkerV1::new(
                policy.policy_version + 1,
                policy.policy_hash().expect("policy hash"),
                1,
                None,
            )
            .into_instruction(),
        ];
        assert_eq!(
            enforce_deferred_policy(&user, &wrong_version, &policy),
            Err(
                ValidationFeeAdmissionError::WrongMultisigFeeMarkerPolicyVersion {
                    expected_version: policy.policy_version,
                    observed_version: policy.policy_version + 1,
                }
            )
        );

        let wrong_hash = vec![
            principal(),
            fee(),
            ValidationFeeMultisigMarkerV1::new(policy.policy_version, [0x55; 32], 1, None)
                .into_instruction(),
        ];
        assert_eq!(
            enforce_deferred_policy(&user, &wrong_hash, &policy),
            Err(
                ValidationFeeAdmissionError::WrongMultisigFeeMarkerPolicyHash {
                    expected_hash_hex: hex::encode(policy.policy_hash().expect("policy hash")),
                    observed_hash_hex: hex::encode([0x55; 32]),
                }
            )
        );

        let wrong_coordinate = with_multisig_fee_marker(&policy, vec![principal(), fee()], 0, None);
        assert!(matches!(
            enforce_deferred_policy(&user, &wrong_coordinate, &policy),
            Err(ValidationFeeAdmissionError::WrongFeeBeneficiary { .. })
        ));

        let batch = TransferAssetBatch::new(vec![
            TransferAssetBatchEntry::new(user.clone(), recipient, fee_asset.clone(), 1_u64),
            TransferAssetBatchEntry::new(
                user.clone(),
                treasury.clone(),
                fee_asset.clone(),
                quantity_minor_units(10),
            ),
        ]);
        let batch_with_marker =
            with_multisig_fee_marker(&policy, vec![InstructionBox::from(batch)], 0, Some(1));
        enforce_deferred_policy(&user, &batch_with_marker, &policy)
            .expect("canonical batch-entry marker validates exact deferred fee");

        let unrelated_treasury_inflow = with_multisig_fee_marker(
            &policy,
            vec![transfer(&user, &fee_asset, minor_units(10), &treasury)],
            0,
            None,
        );
        assert_eq!(
            enforce_deferred_policy(&user, &unrelated_treasury_inflow, &policy),
            Err(ValidationFeeAdmissionError::WrongFeeAmount {
                expected_minor_units: 0,
                observed_minor_units: 10,
            })
        );
    }

    #[test]
    fn opaque_deferred_artifacts_reject_fee_asset_but_allow_generic_assets() {
        let user = account(1);
        let recipient = account(2);
        let treasury = account(3);
        let policy = policy(&treasury);
        let fee_asset = policy_fee_asset(&policy);
        let xor = asset_definition("xor");
        let mut instruction_groups = std::collections::BTreeMap::new();
        instruction_groups.insert(
            user.clone(),
            vec![transfer(&user, &xor, Numeric::new(1u64, 0), &recipient)],
        );
        enforce_opaque_deferred_policy(&instruction_groups, &policy, None)
            .expect("opaque non-fee-asset artifacts remain generic");

        instruction_groups.insert(
            user.clone(),
            vec![
                transfer(&user, &fee_asset, Numeric::new(1u64, 0), &recipient),
                transfer(&user, &fee_asset, minor_units(10), &treasury),
            ],
        );
        assert_eq!(
            enforce_opaque_deferred_policy(&instruction_groups, &policy, None),
            Err(
                ValidationFeeAdmissionError::OpaqueDeferredFeeAssetTransfer {
                    execution_account_id: user.to_string(),
                    instruction_index: 0,
                    entry_index: None,
                }
            )
        );

        let trigger_id: iroha_data_model::trigger::TriggerId =
            "opaque_derived_ds_trigger".parse().expect("trigger id");
        let trigger = Trigger::new(
            trigger_id.clone(),
            Action::new(
                vec![
                    transfer(&user, &fee_asset, Numeric::new(1u64, 0), &recipient),
                    transfer(&user, &fee_asset, minor_units(10), &treasury),
                ],
                Repeats::Indefinitely,
                user.clone(),
                ExecuteTriggerEventFilter::new().for_trigger(trigger_id),
            ),
        );
        instruction_groups.insert(
            user.clone(),
            vec![RegisterBox::Trigger(Register::trigger(trigger)).into()],
        );
        assert!(matches!(
            enforce_opaque_deferred_policy(&instruction_groups, &policy, None),
            Err(ValidationFeeAdmissionError::OpaqueDeferredFeeAssetTransfer { .. })
        ));

        let nested_trigger_id: iroha_data_model::trigger::TriggerId =
            "multisig_wrapped_opaque_ds_trigger"
                .parse()
                .expect("trigger id");
        let nested_trigger = Trigger::new(
            nested_trigger_id.clone(),
            Action::new(
                vec![
                    transfer(&user, &fee_asset, Numeric::new(1u64, 0), &recipient),
                    transfer(&user, &fee_asset, minor_units(10), &treasury),
                ],
                Repeats::Indefinitely,
                user.clone(),
                ExecuteTriggerEventFilter::new().for_trigger(nested_trigger_id),
            ),
        );
        let multisig = account(4);
        instruction_groups.insert(
            user.clone(),
            vec![
                MultisigPropose::new(
                    multisig.clone(),
                    vec![RegisterBox::Trigger(Register::trigger(nested_trigger)).into()],
                    None,
                )
                .into(),
            ],
        );
        assert!(matches!(
            enforce_opaque_deferred_policy(&instruction_groups, &policy, None),
            Err(ValidationFeeAdmissionError::OpaqueDeferredFeeAssetTransfer { .. })
        ));

        let proposal_instructions = vec![
            transfer(&multisig, &fee_asset, Numeric::new(1u64, 0), &recipient),
            transfer(&multisig, &fee_asset, minor_units(10), &treasury),
        ];
        let proposal_hash = HashOf::new(&proposal_instructions);
        let approve = MultisigApprove::new(multisig.clone(), proposal_hash);
        let approve_instruction: InstructionBox = approve.clone().into();

        let assert_indirect_approval_rejected = |instructions: Vec<InstructionBox>| {
            let mut visited = std::collections::BTreeSet::new();
            let mut resolver = |candidate: &MultisigApprove| {
                (candidate == &approve).then(|| (multisig.clone(), proposal_instructions.clone()))
            };
            assert!(matches!(
                reject_opaque_deferred_approval_effects_with(
                    &user,
                    &instructions,
                    &fee_asset,
                    &mut visited,
                    0,
                    &mut resolver,
                ),
                Err(ValidationFeeAdmissionError::OpaqueDeferredFeeAssetTransfer { .. })
            ));
        };
        assert_indirect_approval_rejected(vec![approve_instruction.clone()]);
        assert_indirect_approval_rejected(vec![
            MultisigPropose::new(account(5), vec![approve_instruction.clone()], None).into(),
        ]);

        let approval_trigger_id: iroha_data_model::trigger::TriggerId =
            "opaque_multisig_approval_trigger"
                .parse()
                .expect("trigger id");
        let approval_trigger = Trigger::new(
            approval_trigger_id.clone(),
            Action::new(
                vec![approve_instruction],
                Repeats::Indefinitely,
                user.clone(),
                ExecuteTriggerEventFilter::new().for_trigger(approval_trigger_id),
            ),
        );
        assert_indirect_approval_rejected(vec![
            RegisterBox::Trigger(Register::trigger(approval_trigger)).into(),
        ]);
    }

    #[test]
    fn opaque_treasury_payout_exception_is_direct_source_and_authority_bound() {
        let binding = treasury_payout_binding(test_contract_address(), b"bound-pool");
        let treasury = binding.treasury_account_id.clone();
        let recipient = account(2);
        let other = account(7);
        let policy = policy_with_treasury_payout_lifecycle(binding);
        let fee_asset = policy_fee_asset(&policy);

        let direct_payout = std::collections::BTreeMap::from([(
            treasury.clone(),
            vec![transfer(
                &treasury,
                &fee_asset,
                Numeric::new(1_u64, 0),
                &recipient,
            )],
        )]);
        assert_eq!(
            enforce_opaque_deferred_policy(&direct_payout, &policy, None),
            Err(
                ValidationFeeAdmissionError::OpaqueDeferredFeeAssetTransfer {
                    execution_account_id: treasury.to_string(),
                    instruction_index: 0,
                    entry_index: None,
                }
            ),
            "a payout exemption must not apply without a verified runtime origin",
        );
        enforce_opaque_deferred_policy(&direct_payout, &policy, Some(&treasury))
            .expect("a verified contract-subject treasury may make its enacted-lifecycle payout");

        let wrong_authority = std::collections::BTreeMap::from([(
            other.clone(),
            vec![transfer(
                &treasury,
                &fee_asset,
                Numeric::new(1_u64, 0),
                &recipient,
            )],
        )]);
        assert!(matches!(
            enforce_opaque_deferred_policy(&wrong_authority, &policy, Some(&treasury)),
            Err(ValidationFeeAdmissionError::OpaqueDeferredFeeAssetTransfer {
                execution_account_id,
                ..
            }) if execution_account_id == other.to_string()
        ));

        let wrong_source = std::collections::BTreeMap::from([(
            treasury.clone(),
            vec![transfer(
                &other,
                &fee_asset,
                Numeric::new(1_u64, 0),
                &recipient,
            )],
        )]);
        assert!(matches!(
            enforce_opaque_deferred_policy(&wrong_source, &policy, Some(&treasury)),
            Err(ValidationFeeAdmissionError::OpaqueDeferredFeeAssetTransfer { .. })
        ));

        let nested = MultisigPropose::new(
            treasury.clone(),
            vec![transfer(
                &treasury,
                &fee_asset,
                Numeric::new(1_u64, 0),
                &recipient,
            )],
            None,
        );
        let nested_group = std::collections::BTreeMap::from([(
            treasury.clone(),
            vec![InstructionBox::from(nested)],
        )]);
        assert!(matches!(
            enforce_opaque_deferred_policy(&nested_group, &policy, Some(&treasury)),
            Err(ValidationFeeAdmissionError::OpaqueDeferredFeeAssetTransfer { .. })
        ));
    }

    #[test]
    fn treasury_payout_effect_plan_rejects_every_unbound_substitution() {
        let binding = treasury_payout_binding(test_contract_address(), b"bound-pool");
        let treasury = binding.treasury_account_id.clone();
        let canonical = canonical_treasury_payout_plan(&binding, Quantity::from(20_u64));
        let canonical_groups =
            std::collections::BTreeMap::from([(treasury.clone(), canonical.clone())]);
        let canonical_ordered = ordered_treasury_payout_plan(&binding, &canonical);
        validate_treasury_payout_effect_plan(&canonical_groups, &canonical_ordered, &binding)
            .expect("the exact six-transfer plan is accepted");

        let mut missing = canonical.clone();
        missing.pop();
        let missing_groups =
            std::collections::BTreeMap::from([(treasury.clone(), missing.clone())]);
        assert_treasury_payout_plan_mismatch(
            &binding,
            &missing_groups,
            &ordered_treasury_payout_plan(&binding, &missing),
        );

        let mut extra = canonical.clone();
        extra.push(canonical[5].clone());
        let extra_groups = std::collections::BTreeMap::from([(treasury.clone(), extra.clone())]);
        assert_treasury_payout_plan_mismatch(
            &binding,
            &extra_groups,
            &ordered_treasury_payout_plan(&binding, &extra),
        );

        let mut reordered = canonical_ordered.clone();
        reordered.swap(0, 1);
        assert_treasury_payout_plan_mismatch(&binding, &canonical_groups, &reordered);

        let mut wrong_batch = canonical.clone();
        wrong_batch[0] = transfer(
            &treasury,
            &binding.sbd_asset_id,
            Numeric::new(2_u64, 0),
            &binding.pool_vault_account_id,
        );
        let wrong_batch_groups =
            std::collections::BTreeMap::from([(treasury.clone(), wrong_batch.clone())]);
        assert_treasury_payout_plan_mismatch(
            &binding,
            &wrong_batch_groups,
            &ordered_treasury_payout_plan(&binding, &wrong_batch),
        );

        let mut wrong_sbd_asset = canonical.clone();
        wrong_sbd_asset[0] = transfer(
            &treasury,
            &binding.xor_asset_id,
            binding.batch_sbd.as_numeric().clone(),
            &binding.pool_vault_account_id,
        );
        let wrong_sbd_asset_groups =
            std::collections::BTreeMap::from([(treasury.clone(), wrong_sbd_asset.clone())]);
        assert_treasury_payout_plan_mismatch(
            &binding,
            &wrong_sbd_asset_groups,
            &ordered_treasury_payout_plan(&binding, &wrong_sbd_asset),
        );

        let mut wrong_vault = canonical.clone();
        wrong_vault[1] = transfer(
            &account(7),
            &binding.xor_asset_id,
            Numeric::new(20_u64, 0),
            &treasury,
        );
        let wrong_vault_groups =
            std::collections::BTreeMap::from([(treasury.clone(), wrong_vault.clone())]);
        assert_treasury_payout_plan_mismatch(
            &binding,
            &wrong_vault_groups,
            &ordered_treasury_payout_plan(&binding, &wrong_vault),
        );

        for outside_bound in [3_u64, 101_u64] {
            let out_of_bounds =
                canonical_treasury_payout_plan(&binding, Quantity::from(outside_bound));
            let out_of_bounds_groups =
                std::collections::BTreeMap::from([(treasury.clone(), out_of_bounds.clone())]);
            assert_treasury_payout_plan_mismatch(
                &binding,
                &out_of_bounds_groups,
                &ordered_treasury_payout_plan(&binding, &out_of_bounds),
            );
        }

        let mut wrong_validator = canonical.clone();
        wrong_validator[2] = transfer(
            &treasury,
            &binding.xor_asset_id,
            Numeric::new(5_u64, 0),
            &account(7),
        );
        let wrong_validator_groups =
            std::collections::BTreeMap::from([(treasury.clone(), wrong_validator.clone())]);
        assert_treasury_payout_plan_mismatch(
            &binding,
            &wrong_validator_groups,
            &ordered_treasury_payout_plan(&binding, &wrong_validator),
        );

        let mut wrong_final_amount = canonical.clone();
        wrong_final_amount[5] = transfer(
            &treasury,
            &binding.xor_asset_id,
            Numeric::new(4_u64, 0),
            &binding.recipients[3].account_id,
        );
        let wrong_final_groups =
            std::collections::BTreeMap::from([(treasury.clone(), wrong_final_amount.clone())]);
        assert_treasury_payout_plan_mismatch(
            &binding,
            &wrong_final_groups,
            &ordered_treasury_payout_plan(&binding, &wrong_final_amount),
        );

        let mut changed_shares = binding.clone();
        changed_shares.recipients[0].share = "0.20".parse().expect("changed share");
        changed_shares.recipients[1].share = "0.30".parse().expect("changed share");
        assert_treasury_payout_plan_mismatch(
            &changed_shares,
            &canonical_groups,
            &canonical_ordered,
        );

        let other_authority = account(7);
        let wrong_authority_groups =
            std::collections::BTreeMap::from([(other_authority.clone(), canonical.clone())]);
        let wrong_authority_ordered = canonical
            .iter()
            .cloned()
            .map(|instruction| (other_authority.clone(), instruction))
            .collect::<Vec<_>>();
        assert_treasury_payout_plan_mismatch(
            &binding,
            &wrong_authority_groups,
            &wrong_authority_ordered,
        );

        let mut split_groups =
            std::collections::BTreeMap::from([(treasury.clone(), canonical[..5].to_vec())]);
        split_groups.insert(other_authority.clone(), vec![canonical[5].clone()]);
        let mut split_ordered = canonical_ordered;
        split_ordered[5].0 = other_authority;
        assert_treasury_payout_plan_mismatch(&binding, &split_groups, &split_ordered);
    }

    #[test]
    fn opaque_deferred_unresolved_multisig_approval_fails_closed_against_state_mutation() {
        let user = account(1);
        let multisig = account(4);
        let fee_asset = fee_asset();
        let proposal_hash = HashOf::new(&Vec::<InstructionBox>::new());
        let approve = MultisigApprove::new(multisig.clone(), proposal_hash);
        let approve_instruction: InstructionBox = approve.clone().into();
        let mut visited = std::collections::BTreeSet::new();
        let mut resolver =
            |_candidate: &MultisigApprove| -> Option<(AccountId, Vec<InstructionBox>)> { None };

        assert_eq!(
            reject_opaque_deferred_approval_effects_with(
                &user,
                &[approve_instruction],
                &fee_asset,
                &mut visited,
                0,
                &mut resolver,
            ),
            Err(
                ValidationFeeAdmissionError::UnresolvedOpaqueDeferredMultisigApproval {
                    account_id: multisig.to_string(),
                    instructions_hash_hex: hex::encode(approve.instructions_hash.as_ref()),
                }
            )
        );
    }

    #[test]
    fn missing_fee_is_rejected() {
        let user = account(1);
        let recipient = account(2);
        let treasury = account(3);
        let policy = policy(&treasury);
        let fee_asset = policy_fee_asset(&policy);
        let tx = tx(
            1,
            vec![transfer(
                &user,
                &fee_asset,
                Numeric::new(1u64, 0),
                &recipient,
            )],
            metadata_for(&policy),
        );

        assert_eq!(
            enforce_policy(&tx, &policy),
            Err(ValidationFeeAdmissionError::MissingFee {
                required_minor_units: 10
            })
        );
    }

    #[test]
    fn ivm_proved_axt_without_overlay_fee_fails_closed() {
        let treasury = account(3);
        let policy = policy(&treasury);
        let tx = ivm_proved_tx(1, Vec::new(), Metadata::default());

        assert_eq!(
            enforce_policy(&tx, &policy),
            Ok(()),
            "the signed overlay alone cannot observe an AXT-carried DS effect"
        );
        assert_eq!(
            reject_ivm_proved_completed_axt_effects(1),
            Err(ValidationFeeAdmissionError::OpaqueIvmProvedAxtEffects {
                completed_envelopes: 1,
            })
        );
    }

    #[test]
    fn ivm_proved_axt_with_exact_overlay_fee_still_fails_closed() {
        let user = account(1);
        let recipient = account(2);
        let treasury = account(3);
        let policy = policy(&treasury);
        let fee_asset = policy_fee_asset(&policy);
        let tx = ivm_proved_tx(
            1,
            vec![
                transfer(&user, &fee_asset, Numeric::new(1_u64, 0), &recipient),
                transfer(
                    &user,
                    &fee_asset,
                    minor_units(TEST_VALIDATION_FEE_MINOR_UNITS),
                    &treasury,
                ),
            ],
            metadata_for_fee_instruction(&policy, 1),
        );

        assert_eq!(
            enforce_policy(&tx, &policy),
            Ok(()),
            "the explicit signed overlay fee is exact"
        );
        assert_eq!(
            reject_ivm_proved_completed_axt_effects(1),
            Err(ValidationFeeAdmissionError::OpaqueIvmProvedAxtEffects {
                completed_envelopes: 1,
            }),
            "an exact overlay fee cannot cover opaque AXT DS effects"
        );
    }

    #[test]
    fn typed_treasury_payout_policy_cannot_name_a_signable_treasury() {
        let mut policy = policy_with_treasury_payout_lifecycle(treasury_payout_binding(
            test_contract_address(),
            b"bound-pool",
        ));
        policy.treasury_account_id = account(7);
        assert_eq!(
            policy.policy_invariant_error(),
            Some("validation-fee treasury payout contract subject must equal the policy treasury")
        );
    }

    #[test]
    fn treasury_payout_is_exempt_when_enacted_policy_lists_class() {
        use iroha_data_model::{
            block::BlockHeader,
            nexus::DataSpaceId,
            prelude::{Account, AssetDefinition, Domain},
            smart_contract::ContractAddress,
        };
        use nonzero_ext::nonzero;

        let deployer_key = key_pair(55);
        let deployer = AccountId::new(deployer_key.public_key().clone());
        let domain_id = DomainId::try_new("contracts", "universal").expect("domain id");
        let domain = Domain::new(domain_id).build(&deployer);
        let fee_domain =
            Domain::new(DomainId::try_new("fees", "paynet").expect("fee-asset domain id"))
                .build(&deployer);
        let mut accounts = vec![Account::new(deployer.clone()).build(&deployer)];
        accounts.extend((2..=7).map(|seed| Account::new(account(seed)).build(&deployer)));
        let fee_definition = AssetDefinition::new(
            fee_asset(),
            NumericSpec::fractional(u32::from(TEST_VALIDATION_FEE_ASSET_SCALE)),
        )
        .build(&deployer);
        let xor_definition = AssetDefinition::new(
            xor_asset(),
            NumericSpec::fractional(u32::from(TEST_VALIDATION_FEE_ASSET_SCALE)),
        )
        .build(&deployer);
        let world = crate::state::World::with(
            [domain, fee_domain],
            accounts,
            [fee_definition, xor_definition],
        );
        let state = crate::state::State::new_with_chain_for_testing(
            world,
            crate::kura::Kura::blank_kura_for_testing(),
            crate::query::store::LiveQueryStore::start_test(),
            "generic-testnet".parse().expect("chain id"),
        );
        {
            let mut hashes = state.block_hashes.block();
            hashes.push_for_tests(block_hash([7; 32]));
            hashes.commit_for_tests();
        }

        let header = BlockHeader::new(nonzero!(10_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut state_tx = block.transaction();
        let deployment_permission: iroha_data_model::permission::Permission =
            iroha_executor_data_model::permission::smart_contract::CanRegisterSmartContractCode
                .into();
        crate::smartcontracts::Execute::execute(
            iroha_data_model::isi::Grant::account_permission(
                deployment_permission,
                deployer.clone(),
            ),
            &deployer,
            &mut state_tx,
        )
        .expect("grant contract lifecycle authority");
        let (code, manifest) = minimal_bound_contract_artifact();
        let code_hash = crate::smartcontracts::code::register_code_bytes(
            &deployer,
            code.clone(),
            &mut state_tx,
        )
        .expect("register contract bytes");
        crate::smartcontracts::code::register_manifest(
            &deployer,
            manifest.signed(&deployer_key),
            &mut state_tx,
        )
        .expect("register signed contract manifest");
        let contract_address = ContractAddress::derive(
            iroha_config::parameters::defaults::common::chain_discriminant(),
            &deployer,
            0,
            DataSpaceId::UNIVERSAL,
        )
        .expect("contract address");
        crate::smartcontracts::code::activate_instance(
            &deployer,
            contract_address.clone(),
            code_hash,
            &mut state_tx,
        )
        .expect("activate contract instance");

        let binding = treasury_payout_binding(contract_address.clone(), &code);
        let treasury = binding.treasury_account_id.clone();
        crate::smartcontracts::Execute::execute(
            Register::account(Account::new(treasury.clone())),
            &deployer,
            &mut state_tx,
        )
        .expect("register immutable contract subject account");
        let policy = policy_with_treasury_payout_lifecycle(binding.clone());
        let mut wrong_code_binding = binding.clone();
        wrong_code_binding.code_hash[0] ^= 0xff;
        let wrong_code_policy = policy_with_treasury_payout_lifecycle(wrong_code_binding);
        let wrong_code_registry = policy_registry(std::slice::from_ref(&wrong_code_policy));
        install_policy_registry_fixture(&wrong_code_registry, &mut state_tx);
        let wrong_code_error = active_policy(&state_tx)
            .expect_err("the governed binding cannot name another SHA-256 artifact");
        assert!(
            matches!(wrong_code_error, TransactionRejectionReason::Validation(
                ValidationFail::NotPermitted(ref message)
            ) if message.contains("deployed code hash differs from the enacted binding")),
            "unexpected governed code-hash rejection: {wrong_code_error:?}",
        );

        let registry = policy_registry(std::slice::from_ref(&policy));
        install_policy_registry_fixture(&registry, &mut state_tx);

        active_policy(&state_tx)
            .expect("read active policy")
            .expect("policy is active");
        let runtime = crate::executor::ContractRuntimeExecutionContext {
            contract_address: contract_address.clone(),
            contract_subject: treasury.clone(),
            contract_alias: None,
            entrypoint: binding.entrypoint.to_string(),
        };
        let instructions = canonical_treasury_payout_plan(&binding, Quantity::from(20_u64));
        let ordered = ordered_treasury_payout_plan(&binding, &instructions);
        let groups = std::collections::BTreeMap::from([(treasury.clone(), instructions.clone())]);
        for rejected_origin in [
            None,
            Some(crate::executor::ContractRuntimeExecutionContext {
                contract_address: runtime.contract_address.clone(),
                contract_subject: runtime.contract_subject.clone(),
                contract_alias: None,
                entrypoint: "swap_quote_for_base".to_owned(),
            }),
            Some(crate::executor::ContractRuntimeExecutionContext {
                contract_address: test_contract_address(),
                contract_subject: test_contract_address().subject_id(),
                contract_alias: None,
                entrypoint: binding.entrypoint.to_string(),
            }),
        ] {
            let origin = rejected_origin
                .as_ref()
                .map(|context| OpaqueDeferredRuntimeOrigin::new(context, &code));
            let error = enforce_opaque_deferred_instruction_groups(
                &groups,
                &ordered,
                &mut state_tx,
                origin,
            )
            .expect_err(
                "direct execution, a wrong entrypoint, and another address must not use credit",
            );
            assert!(
                matches!(error, TransactionRejectionReason::Validation(
                    ValidationFail::NotPermitted(ref message)
                ) if message.contains("opaque deferred executable derived a policy fee-asset transfer")),
                "unexpected unbound-runtime rejection: {error:?}",
            );
        }
        assert_eq!(
            enforce_opaque_deferred_instruction_groups(
                &std::collections::BTreeMap::new(),
                &[],
                &mut state_tx,
                Some(OpaqueDeferredRuntimeOrigin::scheduled_time_trigger(
                    &runtime, &code,
                )),
            )
            .expect("a bound pool may report no payout when no batch is available"),
            OpaqueDeferredValidationOutcome::NoOp,
        );
        let payout_credit_minor_units =
            numeric_to_minor_units(binding.batch_sbd.as_numeric(), policy.ds_scale, usize::MAX)
                .expect("payout batch must fit the policy minor-unit domain");
        let payout_credit = ValidationFeeCredit::from_policy_minor_units(
            treasury.clone(),
            policy_fee_asset(&policy),
            policy.ds_scale,
            payout_credit_minor_units,
        )
        .expect("convert payout credit into nominal quantity");
        commit_validation_fee_credit(&mut state_tx, Some(&payout_credit))
            .expect("seed consensus validation-fee credit");
        let (_, asset_binding_key) = validation_fee_credit_state_keys(&state_tx, &treasury)
            .expect("resolve treasury credit paths");
        let valid_asset_binding = state_tx
            .world
            .smart_contract_state
            .get(&asset_binding_key)
            .expect("credit commit must bind its fee asset")
            .clone();
        state_tx
            .world
            .smart_contract_state
            .remove(asset_binding_key.clone());
        assert!(matches!(
            read_validation_fee_credit_balance(&state_tx, &payout_credit),
            Err(ValidationFeeAdmissionError::MalformedCreditAssetBinding { .. })
        ));
        state_tx
            .world
            .smart_contract_state
            .insert(asset_binding_key.clone(), vec![0xFF]);
        assert!(matches!(
            read_validation_fee_credit_balance(&state_tx, &payout_credit),
            Err(ValidationFeeAdmissionError::MalformedCreditAssetBinding { .. })
        ));
        state_tx
            .world
            .smart_contract_state
            .insert(asset_binding_key, valid_asset_binding);
        let wrong_asset_credit = ValidationFeeCredit::from_policy_minor_units(
            treasury.clone(),
            asset_definition("unrelated_ds_successor"),
            policy.ds_scale,
            1,
        )
        .expect("convert wrong-asset fixture credit");
        assert!(matches!(
            read_validation_fee_credit_balance(&state_tx, &wrong_asset_credit),
            Err(ValidationFeeAdmissionError::CreditAssetBindingMismatch { .. })
        ));
        let expected_credit_key =
            validation_fee_credit_state_key_for_address(&runtime.contract_address);
        assert_eq!(
            validation_fee_credit_state_keys(&state_tx, &treasury)
                .expect("resolve treasury credit path")
                .0,
            expected_credit_key,
            "native credit must use the same immutable contract-address scope as IVM state"
        );
        assert!(
            expected_credit_key
                .as_ref()
                .ends_with("/AvailableValidationFeeCredit")
        );
        let retired_key: Name = expected_credit_key
            .as_ref()
            .replace(
                "AvailableValidationFeeCredit",
                "AvailableValidationFeeMinorUnits",
            )
            .parse()
            .expect("retired credit path remains a syntactically valid name");
        assert!(
            !is_validation_fee_credit_state_key(&retired_key),
            "the first release must not reserve or decode the retired fixed-width leaf"
        );

        let canonical_credit_state = state_tx
            .world
            .smart_contract_state
            .get(&expected_credit_key)
            .expect("credit commit must write its state value")
            .clone();
        let record: StateValueRecordV1 = norito::decode_from_bytes(&canonical_credit_state)
            .expect("credit must use a state-value record");
        assert_eq!(record.atoms.len(), 1);
        assert_eq!(
            decode_validation_fee_credit_state_value(&canonical_credit_state)
                .expect("decode canonical nominal credit"),
            payout_credit.amount
        );

        state_tx.world.smart_contract_state.insert(
            expected_credit_key.clone(),
            norito::to_bytes(&100_i64).expect("encode retired primitive state value"),
        );
        assert!(matches!(
            read_validation_fee_credit_balance(&state_tx, &payout_credit),
            Err(ValidationFeeAdmissionError::MalformedCreditBalance { .. })
        ));
        state_tx
            .world
            .smart_contract_state
            .remove(expected_credit_key.clone());
        state_tx.world.smart_contract_state.insert(
            retired_key.clone(),
            norito::to_bytes(&100_i64).expect("encode retired fixed-width credit leaf"),
        );
        assert!(matches!(
            read_validation_fee_credit_balance(&state_tx, &payout_credit),
            Err(ValidationFeeAdmissionError::MalformedCreditBalance { .. })
        ));
        state_tx.world.smart_contract_state.remove(retired_key);

        let mut noncanonical_record = canonical_credit_state.clone();
        noncanonical_record.push(0);
        state_tx
            .world
            .smart_contract_state
            .insert(expected_credit_key.clone(), noncanonical_record);
        assert!(matches!(
            read_validation_fee_credit_balance(&state_tx, &payout_credit),
            Err(ValidationFeeAdmissionError::MalformedCreditBalance { .. })
        ));

        let mut wrong_schema_record = record.clone();
        wrong_schema_record.schema_hash[0] ^= 1;
        state_tx.world.smart_contract_state.insert(
            expected_credit_key.clone(),
            norito::to_bytes(&wrong_schema_record).expect("encode wrong-schema state record"),
        );
        assert!(matches!(
            read_validation_fee_credit_balance(&state_tx, &payout_credit),
            Err(ValidationFeeAdmissionError::MalformedCreditBalance { .. })
        ));

        let wrong_scale: Quantity = "0.001".parse().expect("canonical scale-three quantity");
        state_tx.world.smart_contract_state.insert(
            expected_credit_key.clone(),
            encode_validation_fee_credit_state_value(&wrong_scale)
                .expect("encode schema-bound wrong-scale credit"),
        );
        assert!(matches!(
            read_validation_fee_credit_balance(&state_tx, &payout_credit),
            Err(ValidationFeeAdmissionError::CreditAmountOutsideAssetSpec {
                amount,
                allowed_scale: 2,
                ..
            }) if amount == wrong_scale
        ));

        let wrong_policy_scale = ValidationFeeCredit::from_policy_minor_units(
            treasury.clone(),
            policy_fee_asset(&policy),
            policy.ds_scale - 1,
            10,
        )
        .expect("construct mismatched policy-scale fixture");
        state_tx
            .world
            .smart_contract_state
            .insert(expected_credit_key.clone(), canonical_credit_state.clone());
        assert!(matches!(
            read_validation_fee_credit_balance(&state_tx, &wrong_policy_scale),
            Err(
                ValidationFeeAdmissionError::CreditAssetNumericSpecMismatch {
                    expected_scale: 1,
                    observed_scale: Some(2),
                    ..
                }
            )
        ));

        let excessive_debit_minor_units = payout_credit_minor_units
            .checked_mul(2)
            .expect("fixture debit must fit minor-unit domain");
        let excessive_debit = ValidationFeeCredit::from_policy_minor_units(
            treasury.clone(),
            policy_fee_asset(&policy),
            policy.ds_scale,
            excessive_debit_minor_units,
        )
        .expect("construct underflow fixture");
        let excessive_debit_amount = payout_credit
            .amount
            .checked_add(&payout_credit.amount)
            .expect("fixture debit must fit nominal quantity");
        assert!(matches!(
            consume_validation_fee_credit(&mut state_tx, &excessive_debit),
            Err(ValidationFeeAdmissionError::InsufficientCreditBalance {
                available,
                requested,
            }) if available == payout_credit.amount && requested == excessive_debit_amount
        ));

        let wide: Quantity = "18446744073709551616"
            .parse()
            .expect("canonical credit above u64::MAX");
        state_tx.world.smart_contract_state.insert(
            expected_credit_key.clone(),
            encode_validation_fee_credit_state_value(&wide).expect("encode wide nominal credit"),
        );
        let one_tenth_credit = ValidationFeeCredit::from_policy_minor_units(
            treasury.clone(),
            policy_fee_asset(&policy),
            policy.ds_scale,
            TEST_VALIDATION_FEE_MINOR_UNITS,
        )
        .expect("construct one-tenth credit");
        commit_validation_fee_credit(&mut state_tx, Some(&one_tenth_credit))
            .expect("accumulated nominal credit may exceed the u64 policy-scalar domain");
        assert_eq!(
            read_validation_fee_credit_balance(&state_tx, &payout_credit)
                .expect("read accumulated wide credit"),
            "18446744073709551616.1"
                .parse::<Quantity>()
                .expect("canonical accumulated wide credit")
        );

        let mut maximum_bytes = vec![0xff_u8; iroha_primitives::numeric::MAX_MANTISSA_BYTES];
        *maximum_bytes.last_mut().expect("non-empty mantissa") = 0x7f;
        let maximum = Quantity::try_from_numeric(Numeric::new(
            iroha_primitives::bigint::BigInt::from_twos_bytes(&maximum_bytes)
                .expect("maximum signed 512-bit mantissa"),
            0,
        ))
        .expect("maximum non-negative quantity");
        state_tx.world.smart_contract_state.insert(
            expected_credit_key.clone(),
            encode_validation_fee_credit_state_value(&maximum).expect("encode maximum credit"),
        );
        let overflow = commit_validation_fee_credit(&mut state_tx, Some(&one_tenth_credit))
            .expect_err("credit addition must reject Quantity overflow");
        assert!(
            matches!(overflow, TransactionRejectionReason::Validation(
                ValidationFail::NotPermitted(ref message)
            ) if message.contains(&maximum.to_string()) && message.contains("additional 0.1")),
            "overflow diagnostics must retain exact quantities: {overflow:?}"
        );
        assert_eq!(
            read_validation_fee_credit_balance(&state_tx, &payout_credit)
                .expect("failed addition must leave maximum credit unchanged"),
            maximum
        );
        state_tx
            .world
            .smart_contract_state
            .insert(expected_credit_key, canonical_credit_state);
        state_tx.apply();

        {
            let mut failed_signed_transaction = block.transaction();
            let exact_fee_credit = ValidationFeeCredit::from_policy_minor_units(
                treasury.clone(),
                policy_fee_asset(&policy),
                policy.ds_scale,
                TEST_VALIDATION_FEE_MINOR_UNITS,
            )
            .expect("convert exact policy fee into nominal quantity");
            commit_validation_fee_credit(&mut failed_signed_transaction, Some(&exact_fee_credit))
                .expect("stage exact transaction-bound fee credit");
            let staged_credit = payout_credit
                .amount
                .checked_add(&exact_fee_credit.amount)
                .expect("staged fixture credit must fit nominal quantity");
            assert_eq!(
                read_validation_fee_credit_balance(&failed_signed_transaction, &payout_credit,)
                    .expect("read staged fee credit"),
                staged_credit
            );
            // Simulate a later transaction/data-trigger failure: no staged credit is applied.
        }

        {
            let mut failed_trigger_transaction = block.transaction();
            assert_eq!(
                read_validation_fee_credit_balance(&failed_trigger_transaction, &payout_credit)
                    .expect("read credit after failed signed transaction"),
                payout_credit.amount,
                "a failed signed transaction must not create fee credit"
            );
            assert_eq!(
                enforce_opaque_deferred_instruction_groups(
                    &groups,
                    &ordered,
                    &mut failed_trigger_transaction,
                    Some(OpaqueDeferredRuntimeOrigin::scheduled_time_trigger(
                        &runtime, &code,
                    )),
                )
                .expect("matching runtime may stage an exactly credited payout"),
                OpaqueDeferredValidationOutcome::Apply
            );
            assert_eq!(
                read_validation_fee_credit_balance(&failed_trigger_transaction, &payout_credit,)
                    .expect("read staged debit"),
                Quantity::zero(),
                "the validator stages the debit in the trigger subtransaction"
            );
            failed_trigger_transaction
                .world
                .smart_contract_state
                .insert(
                    "ValidationFeeFinalLegRollbackSentinel"
                        .parse()
                        .expect("rollback sentinel key"),
                    vec![1],
                );
            // Simulate failure of the sixth (final validator) transfer: dropping this
            // subtransaction must roll back the pool-state artifact and native credit debit.
        }

        let mut successful_trigger_transaction = block.transaction();
        assert_eq!(
            read_validation_fee_credit_balance(&successful_trigger_transaction, &payout_credit)
                .expect("read rolled-back debit"),
            payout_credit.amount,
            "a failed trigger subtransaction must roll its staged credit debit back"
        );
        assert!(
            successful_trigger_transaction
                .world
                .smart_contract_state
                .get(
                    &"ValidationFeeFinalLegRollbackSentinel"
                        .parse::<Name>()
                        .expect("rollback sentinel key")
                )
                .is_none(),
            "a final-leg failure must roll back staged pool state as well as credit",
        );

        let altered_code = [code.as_slice(), &[0_u8]].concat();
        let error = enforce_opaque_deferred_instruction_groups(
            &groups,
            &ordered,
            &mut successful_trigger_transaction,
            Some(OpaqueDeferredRuntimeOrigin::scheduled_time_trigger(
                &runtime,
                &altered_code,
            )),
        )
        .expect_err("altered runtime code must not receive the payout exception");
        assert!(
            matches!(error, TransactionRejectionReason::Validation(
                ValidationFail::NotPermitted(ref message)
            ) if message.contains("opaque deferred executable derived a policy fee-asset transfer")),
            "unexpected altered-code rejection: {error:?}",
        );

        let wrong_runtime = crate::executor::ContractRuntimeExecutionContext {
            contract_address: contract_address.clone(),
            contract_subject: deployer,
            contract_alias: None,
            entrypoint: binding.entrypoint.to_string(),
        };
        assert!(
            enforce_opaque_deferred_instruction_groups(
                &groups,
                &ordered,
                &mut successful_trigger_transaction,
                Some(OpaqueDeferredRuntimeOrigin::scheduled_time_trigger(
                    &wrong_runtime,
                    &code,
                )),
            )
            .is_err(),
            "a signable runtime authority must not inherit the contract-subject exception"
        );

        assert_eq!(
            enforce_opaque_deferred_instruction_groups(
                &groups,
                &ordered,
                &mut successful_trigger_transaction,
                Some(OpaqueDeferredRuntimeOrigin::scheduled_time_trigger(
                    &runtime, &code,
                )),
            )
            .expect("matching active contract runtime may make its direct treasury payout"),
            OpaqueDeferredValidationOutcome::Apply
        );
        assert_eq!(
            read_validation_fee_credit_balance(&successful_trigger_transaction, &payout_credit)
                .expect("read consumed validation-fee credit"),
            Quantity::zero(),
            "matching payout consumes exactly its policy-minor-unit debit"
        );
        successful_trigger_transaction.apply();

        let mut exhausted_credit_transaction = block.transaction();
        assert_eq!(
            enforce_opaque_deferred_instruction_groups(
                &groups,
                &ordered,
                &mut exhausted_credit_transaction,
                Some(OpaqueDeferredRuntimeOrigin::scheduled_time_trigger(
                    &runtime, &code,
                )),
            )
            .expect("insufficient reserved credit is a legitimate atomic no-op"),
            OpaqueDeferredValidationOutcome::NoOp
        );
        assert_eq!(
            enforce_opaque_deferred_instruction_groups(
                &std::collections::BTreeMap::new(),
                &[],
                &mut exhausted_credit_transaction,
                Some(OpaqueDeferredRuntimeOrigin::scheduled_time_trigger(
                    &runtime, &code,
                )),
            )
            .expect("an empty bound tick is a legitimate no-op"),
            OpaqueDeferredValidationOutcome::NoOp
        );
    }

    #[test]
    fn active_policy_admission_rejects_completed_ivm_proved_axt() {
        use iroha_data_model::block::BlockHeader;
        use nonzero_ext::nonzero;

        let deployer_key = key_pair(55);
        let deployer = AccountId::new(deployer_key.public_key().clone());
        let state = crate::state::State::new_with_chain_for_testing(
            validation_fee_payout_world(&deployer),
            crate::kura::Kura::blank_kura_for_testing(),
            crate::query::store::LiveQueryStore::start_test(),
            "generic-testnet".parse().expect("chain id"),
        );
        {
            let mut hashes = state.block_hashes.block();
            hashes.push_for_tests(block_hash([7; 32]));
            hashes.commit_for_tests();
        }

        let header = BlockHeader::new(nonzero!(10_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut state_tx = block.transaction();
        let policy =
            install_active_bound_validation_fee_policy(&mut state_tx, &deployer, &deployer_key);
        assert_eq!(
            active_policy(&state_tx)
                .expect("active policy lookup succeeds")
                .expect("bound policy is active"),
            policy
        );

        let error = enforce_ivm_proved_completed_axt_admission(1, &state_tx)
            .expect_err("active policy must reject opaque IvmProved AXT effects");
        assert!(
            matches!(error, ValidationFail::NotPermitted(ref message)
                if message.contains("proof-carrying AXT is disabled")
                    && message.contains("not represented in the signed overlay")),
            "unexpected active-policy AXT rejection: {error:?}"
        );
    }

    #[test]
    fn fee_bearing_transaction_requires_signed_fee_instruction_coordinate() {
        let user = account(1);
        let recipient = account(2);
        let treasury = account(3);
        let policy = policy(&treasury);
        let fee_asset = policy_fee_asset(&policy);
        let tx = tx(
            1,
            vec![
                transfer(&user, &fee_asset, Numeric::new(1u64, 0), &recipient),
                transfer(&user, &fee_asset, minor_units(10), &treasury),
            ],
            metadata_for(&policy),
        );

        assert_eq!(
            enforce_policy(&tx, &policy),
            Err(ValidationFeeAdmissionError::MissingFeeInstructionCoordinate)
        );
    }

    #[test]
    fn dangling_fee_batch_entry_coordinate_is_rejected() {
        let user = account(1);
        let recipient = account(2);
        let treasury = account(3);
        let policy = policy(&treasury);
        let fee_asset = policy_fee_asset(&policy);
        let mut metadata = metadata_for(&policy);
        metadata.insert(
            Name::from_str(VALIDATION_FEE_TRANSFER_ENTRY_INDEX_METADATA_KEY).expect("metadata key"),
            Json::new(0u64),
        );
        let tx = tx(
            1,
            vec![
                transfer(&user, &fee_asset, Numeric::new(1u64, 0), &recipient),
                transfer(&user, &fee_asset, minor_units(10), &treasury),
            ],
            metadata,
        );

        assert_eq!(
            enforce_policy(&tx, &policy),
            Err(ValidationFeeAdmissionError::MalformedFeeInstructionMetadata)
        );
    }

    #[test]
    fn non_authority_source_transfer_requires_context_authority_fee() {
        let authority = account(1);
        let recipient = account(2);
        let treasury = account(3);
        let delegated_source = account(4);
        let policy = policy(&treasury);
        let fee_asset = policy_fee_asset(&policy);

        let missing_fee_tx = tx(
            1,
            vec![transfer(
                &delegated_source,
                &fee_asset,
                Numeric::new(1u64, 0),
                &recipient,
            )],
            metadata_for(&policy),
        );
        assert_eq!(
            enforce_policy(&missing_fee_tx, &policy),
            Err(ValidationFeeAdmissionError::MissingFee {
                required_minor_units: 10,
            })
        );

        let exact_fee_tx = tx(
            1,
            vec![
                transfer(
                    &delegated_source,
                    &fee_asset,
                    Numeric::new(1u64, 0),
                    &recipient,
                ),
                transfer(&authority, &fee_asset, minor_units(10), &treasury),
            ],
            metadata_for_fee_instruction(&policy, 1),
        );
        enforce_policy(&exact_fee_tx, &policy)
            .expect("context authority-paid aggregate fee should validate");
    }

    #[test]
    fn unrelated_treasury_inflow_does_not_inflate_transaction_bound_fee_credit() {
        let user = account(1);
        let treasury = account(3);
        let policy = policy(&treasury);
        let fee_asset = policy_fee_asset(&policy);
        let transaction = tx(
            1,
            vec![
                transfer(&user, &fee_asset, Numeric::new(1_u64, 0), &treasury),
                transfer(
                    &user,
                    &fee_asset,
                    minor_units(TEST_VALIDATION_FEE_MINOR_UNITS),
                    &treasury,
                ),
            ],
            metadata_for_fee_instruction(&policy, 1),
        );

        assert_eq!(
            enforce_policy_with_credit(&transaction, &policy)
                .expect("principal inflow plus exact coordinate must validate"),
            TEST_VALIDATION_FEE_MINOR_UNITS,
            "only the exact signed fee coordinate becomes spendable fee credit"
        );
    }

    #[test]
    fn underpayment_and_overpayment_are_rejected() {
        let user = account(1);
        let recipient = account(2);
        let treasury = account(3);
        let policy = policy(&treasury);
        let fee_asset = policy_fee_asset(&policy);

        for (observed, expected_error) in [
            (
                9,
                ValidationFeeAdmissionError::WrongFeeAmount {
                    expected_minor_units: 10,
                    observed_minor_units: 9,
                },
            ),
            (
                11,
                ValidationFeeAdmissionError::WrongFeeAmount {
                    expected_minor_units: 10,
                    observed_minor_units: 11,
                },
            ),
        ] {
            let tx = tx(
                1,
                vec![
                    transfer(&user, &fee_asset, Numeric::new(1u64, 0), &recipient),
                    transfer(&user, &fee_asset, minor_units(observed), &treasury),
                ],
                metadata_for_fee_instruction(&policy, 1),
            );

            assert_eq!(enforce_policy(&tx, &policy), Err(expected_error));
        }
    }

    #[test]
    fn duplicate_fee_instructions_are_rejected_as_ambiguous() {
        let user = account(1);
        let recipient = account(2);
        let treasury = account(3);
        let policy = policy(&treasury);
        let fee_asset = policy_fee_asset(&policy);
        let tx = tx(
            1,
            vec![
                transfer(&user, &fee_asset, Numeric::new(1u64, 0), &recipient),
                transfer(&user, &fee_asset, minor_units(5), &treasury),
                transfer(&user, &fee_asset, minor_units(5), &treasury),
            ],
            metadata_for(&policy),
        );

        assert_eq!(
            enforce_policy(&tx, &policy),
            Err(ValidationFeeAdmissionError::DuplicateFeeInstructions { count: 2 })
        );
    }

    #[test]
    fn signed_fee_coordinate_treats_additional_treasury_transfer_as_qualifying() {
        let user = account(1);
        let recipient = account(2);
        let treasury = account(3);
        let policy = policy(&treasury);
        let fee_asset = policy_fee_asset(&policy);
        let tx = tx(
            1,
            vec![
                transfer(&user, &fee_asset, Numeric::new(1u64, 0), &recipient),
                transfer(&user, &fee_asset, minor_units(10), &treasury),
                transfer(&user, &fee_asset, Numeric::new(1u64, 0), &treasury),
            ],
            metadata_for_fee_instruction(&policy, 1),
        );

        assert_eq!(
            enforce_policy(&tx, &policy),
            Err(ValidationFeeAdmissionError::WrongFeeAmount {
                expected_minor_units: 20,
                observed_minor_units: 10,
            })
        );
    }

    #[test]
    fn wrong_treasury_or_wrong_asset_fee_is_rejected() {
        let user = account(1);
        let recipient = account(2);
        let treasury = account(3);
        let wrong_treasury = account(4);
        let policy = policy(&treasury);
        let fee_asset = policy_fee_asset(&policy);
        let xor = asset_definition("xor");

        let wrong_treasury_tx = tx(
            1,
            vec![
                transfer(&user, &fee_asset, Numeric::new(1u64, 0), &recipient),
                transfer(&user, &fee_asset, minor_units(10), &wrong_treasury),
            ],
            metadata_for_fee_instruction(&policy, 1),
        );
        assert_eq!(
            enforce_policy(&wrong_treasury_tx, &policy),
            Err(ValidationFeeAdmissionError::WrongFeeBeneficiary {
                instruction_index: 1,
                entry_index: None,
                expected_account_id: treasury.to_string(),
                observed_account_id: wrong_treasury.to_string(),
            })
        );

        let wrong_asset_tx = tx(
            1,
            vec![
                transfer(&user, &fee_asset, Numeric::new(1u64, 0), &recipient),
                transfer(&user, &xor, minor_units(10), &treasury),
            ],
            metadata_for_fee_instruction(&policy, 1),
        );
        assert_eq!(
            enforce_policy(&wrong_asset_tx, &policy),
            Err(ValidationFeeAdmissionError::WrongFeeAsset {
                instruction_index: 1,
                entry_index: None,
            })
        );
    }

    #[test]
    fn signed_fee_coordinate_rejects_fee_not_paid_by_transaction_authority() {
        let user = account(1);
        let recipient = account(2);
        let treasury = account(3);
        let sponsor = account(4);
        let policy = policy(&treasury);
        let fee_asset = policy_fee_asset(&policy);
        let tx = tx(
            1,
            vec![
                transfer(&user, &fee_asset, Numeric::new(1u64, 0), &recipient),
                transfer(&sponsor, &fee_asset, minor_units(10), &treasury),
            ],
            metadata_for_fee_instruction(&policy, 1),
        );

        assert_eq!(
            enforce_policy(&tx, &policy),
            Err(ValidationFeeAdmissionError::WrongFeeSource {
                instruction_index: 1,
                entry_index: None,
            })
        );
    }

    #[test]
    fn fee_transfer_is_not_recursively_charged() {
        let user = account(1);
        let recipient = account(2);
        let treasury = account(3);
        let policy = policy(&treasury);
        let fee_asset = policy_fee_asset(&policy);

        let exact_fee_tx = tx(
            1,
            vec![
                transfer(&user, &fee_asset, Numeric::new(1u64, 0), &recipient),
                transfer(&user, &fee_asset, minor_units(10), &treasury),
            ],
            metadata_for_fee_instruction(&policy, 1),
        );
        enforce_policy(&exact_fee_tx, &policy).expect("fee instruction is not recursively charged");

        let recursively_charged_tx = tx(
            1,
            vec![
                transfer(&user, &fee_asset, Numeric::new(1u64, 0), &recipient),
                transfer(&user, &fee_asset, minor_units(20), &treasury),
            ],
            metadata_for_fee_instruction(&policy, 1),
        );
        assert_eq!(
            enforce_policy(&recursively_charged_tx, &policy),
            Err(ValidationFeeAdmissionError::WrongFeeAmount {
                expected_minor_units: 10,
                observed_minor_units: 20,
            })
        );
    }

    #[test]
    fn retail_transfer_to_treasury_requires_separate_signed_fee() {
        let user = account(1);
        let treasury = account(2);
        let policy = policy(&treasury);
        let fee_asset = policy_fee_asset(&policy);
        let tx = tx(
            1,
            vec![
                transfer(&user, &fee_asset, Numeric::new(1u64, 0), &treasury),
                transfer(&user, &fee_asset, minor_units(10), &treasury),
            ],
            metadata_for_fee_instruction(&policy, 1),
        );

        enforce_policy(&tx, &policy)
            .expect("treasury-destination principal requires a separate signed fee instruction");
    }

    #[test]
    fn single_treasury_transfer_cannot_be_signed_as_standalone_fee() {
        let user = account(1);
        let treasury = account(2);
        let policy = policy(&treasury);
        let fee_asset = policy_fee_asset(&policy);
        let tx = tx(
            1,
            vec![transfer(&user, &fee_asset, minor_units(10), &treasury)],
            metadata_for_fee_instruction(&policy, 0),
        );

        assert_eq!(
            enforce_policy(&tx, &policy),
            Err(ValidationFeeAdmissionError::WrongFeeAmount {
                expected_minor_units: 0,
                observed_minor_units: 10,
            })
        );
    }

    #[test]
    fn treasury_payout_requires_enacted_payout_lifecycle() {
        let user = account(1);
        let treasury = account(2);
        let policy = policy(&treasury);
        let fee_asset = policy_fee_asset(&policy);

        let treasury_payout = tx(
            2,
            vec![transfer(
                &treasury,
                &fee_asset,
                Numeric::new(1u64, 0),
                &user,
            )],
            Metadata::default(),
        );
        assert_eq!(
            enforce_policy(&treasury_payout, &policy),
            Err(ValidationFeeAdmissionError::MissingFee {
                required_minor_units: TEST_VALIDATION_FEE_MINOR_UNITS
            })
        );
    }

    #[test]
    fn non_exempt_treasury_payout_is_accepted_with_exact_fee() {
        let user = account(1);
        let treasury = account(2);
        let policy = policy(&treasury);
        let fee_asset = policy_fee_asset(&policy);

        let treasury_payout = tx(
            2,
            vec![
                transfer(&treasury, &fee_asset, Numeric::new(1u64, 0), &user),
                transfer(&treasury, &fee_asset, minor_units(10), &treasury),
            ],
            metadata_for_fee_instruction(&policy, 1),
        );
        enforce_policy(&treasury_payout, &policy)
            .expect("non-exempt treasury payout can pay the exact protocol fee");
    }

    #[test]
    fn ordinary_sbd_transfer_from_bound_treasury_remains_fee_bearing() {
        let user = account(1);
        let binding = treasury_payout_binding(test_contract_address(), b"bound-pool");
        let treasury = binding.treasury_account_id.clone();
        let policy = policy_with_treasury_payout_lifecycle(binding);
        let fee_asset = policy_fee_asset(&policy);

        let treasury_payout = tx(
            2,
            vec![transfer(
                &treasury,
                &fee_asset,
                Numeric::new(1u64, 0),
                &user,
            )],
            Metadata::default(),
        );
        assert_eq!(
            enforce_policy(&treasury_payout, &policy),
            Err(ValidationFeeAdmissionError::MissingFee {
                required_minor_units: TEST_VALIDATION_FEE_MINOR_UNITS,
            }),
            "the exemption is available only to the exact bound opaque runtime plan",
        );
    }

    #[test]
    fn sub_minor_fee_amount_is_rejected() {
        let user = account(1);
        let recipient = account(2);
        let treasury = account(3);
        let policy = policy(&treasury);
        let fee_asset = policy_fee_asset(&policy);
        let tx = tx(
            1,
            vec![
                transfer(&user, &fee_asset, Numeric::new(1u64, 0), &recipient),
                transfer(&user, &fee_asset, Numeric::new(1u64, 5), &treasury),
            ],
            metadata_for_fee_instruction(&policy, 1),
        );

        assert_eq!(
            enforce_policy(&tx, &policy),
            Err(ValidationFeeAdmissionError::NonMinorUnitAmount {
                instruction_index: 1,
                scale: 5,
                policy_scale: TEST_VALIDATION_FEE_ASSET_SCALE
            })
        );
    }

    #[test]
    fn policy_version_metadata_is_required_and_exact() {
        let user = account(1);
        let recipient = account(2);
        let treasury = account(3);
        let policy = policy(&treasury);
        let fee_asset = policy_fee_asset(&policy);
        let instructions = || {
            vec![
                transfer(&user, &fee_asset, Numeric::new(1u64, 0), &recipient),
                transfer(&user, &fee_asset, minor_units(10), &treasury),
            ]
        };

        let missing_metadata_tx = tx(
            1,
            instructions(),
            metadata_for_fee_instruction_coordinate(1),
        );
        assert_eq!(
            enforce_policy(&missing_metadata_tx, &policy),
            Err(ValidationFeeAdmissionError::MissingPolicyVersionMetadata)
        );

        let mut wrong_version = metadata_for_fee_instruction(&policy, 1);
        wrong_version.insert(
            Name::from_str(VALIDATION_FEE_POLICY_VERSION_METADATA_KEY).expect("metadata key"),
            Json::new(policy.policy_version + 1),
        );
        let wrong_version_tx = tx(1, instructions(), wrong_version);
        assert_eq!(
            enforce_policy(&wrong_version_tx, &policy),
            Err(ValidationFeeAdmissionError::WrongPolicyVersionMetadata {
                expected_version: 1,
                observed_version: 2
            })
        );
    }

    #[test]
    fn wrong_policy_hash_metadata_is_rejected() {
        let user = account(1);
        let recipient = account(2);
        let treasury = account(3);
        let policy = policy(&treasury);
        let fee_asset = policy_fee_asset(&policy);
        let mut metadata = metadata_for_fee_instruction(&policy, 1);
        metadata.insert(
            Name::from_str(VALIDATION_FEE_POLICY_HASH_METADATA_KEY).expect("metadata key"),
            Json::new(hex::encode([9u8; 32])),
        );
        let tx = tx(
            1,
            vec![
                transfer(&user, &fee_asset, Numeric::new(1u64, 0), &recipient),
                transfer(&user, &fee_asset, minor_units(10), &treasury),
            ],
            metadata,
        );

        assert!(matches!(
            enforce_policy(&tx, &policy),
            Err(ValidationFeeAdmissionError::WrongPolicyHashMetadata { .. })
        ));
    }

    #[test]
    fn zero_qualifying_transaction_rejects_mismatched_validation_fee_policy_metadata() {
        let user = account(1);
        let recipient = account(2);
        let treasury = account(3);
        let policy = policy(&treasury);
        let non_fee_asset = asset_definition("xor");
        let mut metadata = metadata_for(&policy);
        let observed_hash_hex = hex::encode([9u8; 32]);
        metadata.insert(
            Name::from_str(VALIDATION_FEE_POLICY_HASH_METADATA_KEY).expect("metadata key"),
            Json::new(observed_hash_hex.clone()),
        );
        let tx = tx(
            1,
            vec![transfer(
                &user,
                &non_fee_asset,
                Numeric::new(1u64, 0),
                &recipient,
            )],
            metadata,
        );

        assert_eq!(
            enforce_policy(&tx, &policy),
            Err(ValidationFeeAdmissionError::WrongPolicyHashMetadata {
                expected_hash_hex: hex::encode(policy.policy_hash().expect("policy hash")),
                observed_hash_hex,
            })
        );
    }

    #[test]
    fn zero_qualifying_transaction_with_fee_coordinate_requires_policy_metadata() {
        let user = account(1);
        let recipient = account(2);
        let treasury = account(3);
        let policy = policy(&treasury);
        let non_fee_asset = asset_definition("xor");
        let tx = tx(
            1,
            vec![transfer(
                &user,
                &non_fee_asset,
                Numeric::new(1u64, 0),
                &recipient,
            )],
            metadata_for_fee_instruction_coordinate(0),
        );

        assert_eq!(
            enforce_policy(&tx, &policy),
            Err(ValidationFeeAdmissionError::MissingPolicyVersionMetadata)
        );
    }

    #[test]
    fn zero_qualifying_transaction_rejects_dangling_fee_entry_coordinate() {
        let user = account(1);
        let recipient = account(2);
        let treasury = account(3);
        let policy = policy(&treasury);
        let non_fee_asset = asset_definition("xor");
        let mut metadata = metadata_for(&policy);
        metadata.insert(
            Name::from_str(VALIDATION_FEE_TRANSFER_ENTRY_INDEX_METADATA_KEY).expect("metadata key"),
            Json::new(0u64),
        );
        let tx = tx(
            1,
            vec![transfer(
                &user,
                &non_fee_asset,
                Numeric::new(1u64, 0),
                &recipient,
            )],
            metadata,
        );

        assert_eq!(
            enforce_policy(&tx, &policy),
            Err(ValidationFeeAdmissionError::MalformedFeeInstructionMetadata)
        );
    }

    #[test]
    fn batch_entries_are_charged_per_entry() {
        let user = account(1);
        let recipient_a = account(2);
        let recipient_b = account(3);
        let treasury = account(4);
        let policy = policy(&treasury);
        let fee_asset = policy_fee_asset(&policy);
        let tx = tx(
            1,
            vec![
                TransferAssetBatch::new(vec![
                    TransferAssetBatchEntry::new(
                        user.clone(),
                        recipient_a,
                        fee_asset.clone(),
                        1_u64,
                    ),
                    TransferAssetBatchEntry::new(
                        user.clone(),
                        recipient_b,
                        fee_asset.clone(),
                        1_u64,
                    ),
                    TransferAssetBatchEntry::new(
                        user,
                        treasury,
                        fee_asset,
                        quantity_minor_units(20),
                    ),
                ])
                .into(),
            ],
            metadata_for_fee_batch_entry(&policy, 0, 2),
        );

        assert_eq!(
            enforce_policy_with_credit(&tx, &policy).expect("batch aggregate fee validates"),
            20,
            "a signed batch credits exactly its aggregate protocol fee"
        );
    }

    #[test]
    fn batch_entries_reject_underpayment_and_overpayment() {
        let user = account(1);
        let recipient_a = account(2);
        let recipient_b = account(3);
        let treasury = account(4);
        let policy = policy(&treasury);
        let fee_asset = policy_fee_asset(&policy);

        for (observed, expected_error) in [
            (
                10,
                ValidationFeeAdmissionError::WrongFeeAmount {
                    expected_minor_units: 20,
                    observed_minor_units: 10,
                },
            ),
            (
                30,
                ValidationFeeAdmissionError::WrongFeeAmount {
                    expected_minor_units: 20,
                    observed_minor_units: 30,
                },
            ),
        ] {
            let tx = tx(
                1,
                vec![
                    TransferAssetBatch::new(vec![
                        TransferAssetBatchEntry::new(
                            user.clone(),
                            recipient_a.clone(),
                            fee_asset.clone(),
                            1_u64,
                        ),
                        TransferAssetBatchEntry::new(
                            user.clone(),
                            recipient_b.clone(),
                            fee_asset.clone(),
                            1_u64,
                        ),
                        TransferAssetBatchEntry::new(
                            user.clone(),
                            treasury.clone(),
                            fee_asset.clone(),
                            quantity_minor_units(observed),
                        ),
                    ])
                    .into(),
                ],
                metadata_for_fee_batch_entry(&policy, 0, 2),
            );

            assert_eq!(enforce_policy(&tx, &policy), Err(expected_error));
        }
    }

    #[test]
    fn batch_fee_coordinate_pointing_at_principal_entry_is_rejected() {
        let user = account(1);
        let recipient = account(2);
        let treasury = account(3);
        let policy = policy(&treasury);
        let fee_asset = policy_fee_asset(&policy);
        let tx = tx(
            1,
            vec![
                TransferAssetBatch::new(vec![
                    TransferAssetBatchEntry::new(
                        user.clone(),
                        recipient.clone(),
                        fee_asset.clone(),
                        1_u64,
                    ),
                    TransferAssetBatchEntry::new(
                        user,
                        treasury.clone(),
                        fee_asset,
                        quantity_minor_units(10),
                    ),
                ])
                .into(),
            ],
            metadata_for_fee_batch_entry(&policy, 0, 0),
        );

        assert_eq!(
            enforce_policy(&tx, &policy),
            Err(ValidationFeeAdmissionError::WrongFeeBeneficiary {
                instruction_index: 0,
                entry_index: Some(0),
                expected_account_id: treasury.to_string(),
                observed_account_id: recipient.to_string(),
            })
        );
    }

    #[test]
    fn batch_fee_entry_rejects_wrong_treasury_asset_and_source() {
        let user = account(1);
        let recipient_a = account(2);
        let recipient_b = account(3);
        let treasury = account(4);
        let wrong_treasury = account(5);
        let sponsor = account(6);
        let policy = policy(&treasury);
        let fee_asset = policy_fee_asset(&policy);
        let xor = asset_definition("xor");

        let wrong_treasury_tx = tx(
            1,
            vec![
                TransferAssetBatch::new(vec![
                    TransferAssetBatchEntry::new(
                        user.clone(),
                        recipient_a.clone(),
                        fee_asset.clone(),
                        1_u64,
                    ),
                    TransferAssetBatchEntry::new(
                        user.clone(),
                        recipient_b.clone(),
                        fee_asset.clone(),
                        1_u64,
                    ),
                    TransferAssetBatchEntry::new(
                        user.clone(),
                        wrong_treasury.clone(),
                        fee_asset.clone(),
                        quantity_minor_units(20),
                    ),
                ])
                .into(),
            ],
            metadata_for_fee_batch_entry(&policy, 0, 2),
        );
        assert_eq!(
            enforce_policy(&wrong_treasury_tx, &policy),
            Err(ValidationFeeAdmissionError::WrongFeeBeneficiary {
                instruction_index: 0,
                entry_index: Some(2),
                expected_account_id: treasury.to_string(),
                observed_account_id: wrong_treasury.to_string(),
            })
        );

        let wrong_asset_tx = tx(
            1,
            vec![
                TransferAssetBatch::new(vec![
                    TransferAssetBatchEntry::new(
                        user.clone(),
                        recipient_a.clone(),
                        fee_asset.clone(),
                        1_u64,
                    ),
                    TransferAssetBatchEntry::new(
                        user.clone(),
                        recipient_b.clone(),
                        fee_asset.clone(),
                        1_u64,
                    ),
                    TransferAssetBatchEntry::new(
                        user.clone(),
                        treasury.clone(),
                        xor,
                        quantity_minor_units(20),
                    ),
                ])
                .into(),
            ],
            metadata_for_fee_batch_entry(&policy, 0, 2),
        );
        assert_eq!(
            enforce_policy(&wrong_asset_tx, &policy),
            Err(ValidationFeeAdmissionError::WrongFeeAsset {
                instruction_index: 0,
                entry_index: Some(2),
            })
        );

        let wrong_source_tx = tx(
            1,
            vec![
                TransferAssetBatch::new(vec![
                    TransferAssetBatchEntry::new(
                        user.clone(),
                        recipient_a,
                        fee_asset.clone(),
                        1_u64,
                    ),
                    TransferAssetBatchEntry::new(user, recipient_b, fee_asset.clone(), 1_u64),
                    TransferAssetBatchEntry::new(
                        sponsor,
                        treasury.clone(),
                        fee_asset,
                        quantity_minor_units(20),
                    ),
                ])
                .into(),
            ],
            metadata_for_fee_batch_entry(&policy, 0, 2),
        );
        assert_eq!(
            enforce_policy(&wrong_source_tx, &policy),
            Err(ValidationFeeAdmissionError::WrongFeeSource {
                instruction_index: 0,
                entry_index: Some(2),
            })
        );
    }

    #[test]
    fn multisig_proposal_fee_asset_transfer_requires_context_fee() {
        let user = account(1);
        let recipient = account(2);
        let treasury = account(3);
        let multisig = account(4);
        let policy = policy(&treasury);
        let fee_asset = policy_fee_asset(&policy);
        let proposal = MultisigPropose::new(
            multisig.clone(),
            vec![transfer(
                &multisig,
                &fee_asset,
                Numeric::new(1u64, 0),
                &recipient,
            )],
            None,
        );
        let missing_fee_tx = tx(1, vec![proposal.into()], metadata_for(&policy));

        assert_eq!(
            enforce_policy(&missing_fee_tx, &policy),
            Err(ValidationFeeAdmissionError::MissingMultisigFeeMarker { context_index: 1 })
        );

        let top_level_fee = tx(
            1,
            vec![
                MultisigPropose::new(
                    multisig.clone(),
                    with_multisig_fee_marker(
                        &policy,
                        vec![
                            transfer(&multisig, &fee_asset, Numeric::new(1u64, 0), &recipient),
                            transfer(&multisig, &fee_asset, minor_units(10), &treasury),
                        ],
                        1,
                        None,
                    ),
                    None,
                )
                .into(),
                Log::new(Level::INFO, "outer index spacer".to_owned()).into(),
                transfer(&user, &fee_asset, minor_units(10), &treasury),
            ],
            metadata_for_fee_instruction(&policy, 2),
        );
        assert_eq!(
            enforce_policy(&top_level_fee, &policy),
            Err(ValidationFeeAdmissionError::WrongFeeAmount {
                expected_minor_units: 0,
                observed_minor_units: 10
            })
        );
    }

    #[test]
    fn multisig_proposal_context_fee_validates() {
        let recipient = account(2);
        let treasury = account(3);
        let multisig = account(4);
        let policy = policy(&treasury);
        let fee_asset = policy_fee_asset(&policy);
        let tx = tx(
            1,
            vec![
                MultisigPropose::new(
                    multisig.clone(),
                    with_multisig_fee_marker(
                        &policy,
                        vec![
                            transfer(&multisig, &fee_asset, Numeric::new(1u64, 0), &recipient),
                            transfer(&multisig, &fee_asset, minor_units(10), &treasury),
                        ],
                        1,
                        None,
                    ),
                    None,
                )
                .into(),
            ],
            metadata_for(&policy),
        );

        enforce_policy(&tx, &policy).expect("multisig proposal context fee validates");
    }

    #[test]
    fn multisig_fee_credits_only_when_deferred_instructions_execute() {
        let recipient = account(2);
        let treasury = account(3);
        let multisig = account(4);
        let policy = policy(&treasury);
        let fee_asset = policy_fee_asset(&policy);
        let deferred_instructions = with_multisig_fee_marker(
            &policy,
            vec![
                transfer(&multisig, &fee_asset, Numeric::new(1_u64, 0), &recipient),
                transfer(
                    &multisig,
                    &fee_asset,
                    minor_units(TEST_VALIDATION_FEE_MINOR_UNITS),
                    &treasury,
                ),
            ],
            1,
            None,
        );
        let proposal_transaction = tx(
            1,
            vec![
                MultisigPropose::new(multisig.clone(), deferred_instructions.clone(), None).into(),
            ],
            metadata_for(&policy),
        );

        assert_eq!(
            enforce_policy_with_credit(&proposal_transaction, &policy)
                .expect("signed proposal must validate"),
            0,
            "registering a proposal cannot credit DS that has not moved"
        );
        assert_eq!(
            enforce_deferred_policy_with_credit(&multisig, &deferred_instructions, &policy)
                .expect("executing the stored multisig instructions must validate"),
            TEST_VALIDATION_FEE_MINOR_UNITS,
            "the exact marker-bound fee credits when the proposal actually executes"
        );
    }

    #[test]
    fn multisig_proposal_signed_fee_coordinate_resolves_unique_nested_context() {
        let user = account(1);
        let recipient = account(2);
        let treasury = account(3);
        let multisig = account(4);
        let policy = policy(&treasury);
        let fee_asset = policy_fee_asset(&policy);
        let nested_proposal = || {
            MultisigPropose::new(
                multisig.clone(),
                with_multisig_fee_marker(
                    &policy,
                    vec![
                        transfer(&multisig, &fee_asset, Numeric::new(1u64, 0), &recipient),
                        transfer(&multisig, &fee_asset, minor_units(10), &treasury),
                    ],
                    1,
                    None,
                ),
                None,
            )
        };

        let exact = tx(
            1,
            vec![nested_proposal().into()],
            metadata_for_fee_instruction(&policy, 1),
        );
        enforce_policy(&exact, &policy)
            .expect("signed fee coordinate should resolve the unique nested proposal context");

        let wrong = tx(
            1,
            vec![nested_proposal().into()],
            metadata_for_fee_instruction(&policy, 0),
        );
        assert_eq!(
            enforce_policy(&wrong, &policy),
            Err(ValidationFeeAdmissionError::ConflictingMultisigFeeCoordinate { context_index: 1 })
        );

        let ambiguous = tx(
            1,
            vec![
                nested_proposal().into(),
                transfer(
                    &user,
                    &asset_definition("xor"),
                    Numeric::new(1u64, 0),
                    &recipient,
                ),
            ],
            metadata_for_fee_instruction(&policy, 1),
        );
        assert_eq!(
            enforce_policy(&ambiguous, &policy),
            Err(
                ValidationFeeAdmissionError::AmbiguousFeeInstructionCoordinate {
                    instruction_index: 1,
                    entry_index: None,
                }
            )
        );
    }

    #[test]
    fn nested_fee_coordinate_does_not_implicitly_designate_top_level_treasury_inflow() {
        let user = account(1);
        let recipient = account(2);
        let treasury = account(3);
        let multisig = account(4);
        let policy = policy(&treasury);
        let fee_asset = policy_fee_asset(&policy);
        let nested_proposal = MultisigPropose::new(
            multisig.clone(),
            with_multisig_fee_marker(
                &policy,
                vec![
                    transfer(&multisig, &fee_asset, Numeric::new(1u64, 0), &recipient),
                    transfer(&multisig, &fee_asset, minor_units(10), &treasury),
                ],
                1,
                None,
            ),
            None,
        );
        let tx = tx(
            1,
            vec![
                transfer(&user, &fee_asset, Numeric::new(1u64, 0), &recipient),
                nested_proposal.into(),
                transfer(&user, &fee_asset, minor_units(10), &treasury),
            ],
            metadata_for_fee_instruction(&policy, 1),
        );

        assert_eq!(
            enforce_policy(&tx, &policy),
            Err(ValidationFeeAdmissionError::MissingFeeInstructionCoordinate)
        );
    }

    #[test]
    fn multisig_proposal_context_fee_requires_policy_coordinates() {
        let recipient = account(2);
        let treasury = account(3);
        let multisig = account(4);
        let policy = policy(&treasury);
        let fee_asset = policy_fee_asset(&policy);
        let tx = tx(
            1,
            vec![
                MultisigPropose::new(
                    multisig.clone(),
                    with_multisig_fee_marker(
                        &policy,
                        vec![
                            transfer(&multisig, &fee_asset, Numeric::new(1u64, 0), &recipient),
                            transfer(&multisig, &fee_asset, minor_units(10), &treasury),
                        ],
                        1,
                        None,
                    ),
                    None,
                )
                .into(),
            ],
            Metadata::default(),
        );

        assert_eq!(
            enforce_policy(&tx, &policy),
            Err(ValidationFeeAdmissionError::MissingPolicyVersionMetadata)
        );
    }

    #[test]
    fn multisig_proposal_context_fee_rejects_wrong_amounts() {
        let recipient = account(2);
        let treasury = account(3);
        let multisig = account(4);
        let policy = policy(&treasury);
        let fee_asset = policy_fee_asset(&policy);

        for (observed, expected_error) in [
            (
                9,
                ValidationFeeAdmissionError::WrongFeeAmount {
                    expected_minor_units: 10,
                    observed_minor_units: 9,
                },
            ),
            (
                11,
                ValidationFeeAdmissionError::WrongFeeAmount {
                    expected_minor_units: 10,
                    observed_minor_units: 11,
                },
            ),
        ] {
            let tx = tx(
                1,
                vec![
                    MultisigPropose::new(
                        multisig.clone(),
                        with_multisig_fee_marker(
                            &policy,
                            vec![
                                transfer(&multisig, &fee_asset, Numeric::new(1u64, 0), &recipient),
                                transfer(&multisig, &fee_asset, minor_units(observed), &treasury),
                            ],
                            1,
                            None,
                        ),
                        None,
                    )
                    .into(),
                ],
                metadata_for(&policy),
            );

            assert_eq!(enforce_policy(&tx, &policy), Err(expected_error));
        }
    }

    #[test]
    fn multisig_proposal_context_fee_rejects_wrong_treasury_asset_and_source() {
        let recipient = account(2);
        let treasury = account(3);
        let multisig = account(4);
        let wrong_treasury = account(5);
        let sponsor = account(6);
        let policy = policy(&treasury);
        let fee_asset = policy_fee_asset(&policy);
        let xor = asset_definition("xor");

        let wrong_treasury_tx = tx(
            1,
            vec![
                MultisigPropose::new(
                    multisig.clone(),
                    with_multisig_fee_marker(
                        &policy,
                        vec![
                            transfer(&multisig, &fee_asset, Numeric::new(1u64, 0), &recipient),
                            transfer(&multisig, &fee_asset, minor_units(10), &wrong_treasury),
                        ],
                        1,
                        None,
                    ),
                    None,
                )
                .into(),
            ],
            metadata_for(&policy),
        );
        assert_eq!(
            enforce_policy(&wrong_treasury_tx, &policy),
            Err(ValidationFeeAdmissionError::WrongFeeBeneficiary {
                instruction_index: 1,
                entry_index: None,
                expected_account_id: treasury.to_string(),
                observed_account_id: wrong_treasury.to_string(),
            })
        );

        let wrong_asset_tx = tx(
            1,
            vec![
                MultisigPropose::new(
                    multisig.clone(),
                    with_multisig_fee_marker(
                        &policy,
                        vec![
                            transfer(&multisig, &fee_asset, Numeric::new(1u64, 0), &recipient),
                            transfer(&multisig, &xor, minor_units(10), &treasury),
                        ],
                        1,
                        None,
                    ),
                    None,
                )
                .into(),
            ],
            metadata_for(&policy),
        );
        assert_eq!(
            enforce_policy(&wrong_asset_tx, &policy),
            Err(ValidationFeeAdmissionError::WrongFeeAsset {
                instruction_index: 1,
                entry_index: None,
            })
        );

        let wrong_source_tx = tx(
            1,
            vec![
                MultisigPropose::new(
                    multisig.clone(),
                    with_multisig_fee_marker(
                        &policy,
                        vec![
                            transfer(&multisig, &fee_asset, Numeric::new(1u64, 0), &recipient),
                            transfer(&sponsor, &fee_asset, minor_units(10), &treasury),
                        ],
                        1,
                        None,
                    ),
                    None,
                )
                .into(),
            ],
            metadata_for(&policy),
        );
        assert_eq!(
            enforce_policy(&wrong_source_tx, &policy),
            Err(ValidationFeeAdmissionError::WrongFeeSource {
                instruction_index: 1,
                entry_index: None,
            })
        );
    }

    #[test]
    fn multisig_proposal_batch_entries_are_charged_per_entry() {
        let recipient_a = account(2);
        let recipient_b = account(3);
        let treasury = account(4);
        let multisig = account(5);
        let policy = policy(&treasury);
        let fee_asset = policy_fee_asset(&policy);
        let proposal = MultisigPropose::new(
            multisig.clone(),
            with_multisig_fee_marker(
                &policy,
                vec![
                    TransferAssetBatch::new(vec![
                        TransferAssetBatchEntry::new(
                            multisig.clone(),
                            recipient_a,
                            fee_asset.clone(),
                            1_u64,
                        ),
                        TransferAssetBatchEntry::new(
                            multisig.clone(),
                            recipient_b,
                            fee_asset.clone(),
                            1_u64,
                        ),
                        TransferAssetBatchEntry::new(
                            multisig,
                            treasury,
                            fee_asset,
                            quantity_minor_units(20),
                        ),
                    ])
                    .into(),
                ],
                0,
                Some(2),
            ),
            None,
        );
        let tx = tx(1, vec![proposal.into()], metadata_for(&policy));

        enforce_policy(&tx, &policy).expect("multisig batch aggregate fee validates");
    }

    #[test]
    fn multisig_proposal_batch_entries_reject_underpayment_and_overpayment() {
        let recipient_a = account(2);
        let recipient_b = account(3);
        let treasury = account(4);
        let multisig = account(5);
        let policy = policy(&treasury);
        let fee_asset = policy_fee_asset(&policy);

        for (observed, expected_error) in [
            (
                19,
                ValidationFeeAdmissionError::WrongFeeAmount {
                    expected_minor_units: 20,
                    observed_minor_units: 19,
                },
            ),
            (
                21,
                ValidationFeeAdmissionError::WrongFeeAmount {
                    expected_minor_units: 20,
                    observed_minor_units: 21,
                },
            ),
        ] {
            let proposal = MultisigPropose::new(
                multisig.clone(),
                with_multisig_fee_marker(
                    &policy,
                    vec![
                        TransferAssetBatch::new(vec![
                            TransferAssetBatchEntry::new(
                                multisig.clone(),
                                recipient_a.clone(),
                                fee_asset.clone(),
                                1_u64,
                            ),
                            TransferAssetBatchEntry::new(
                                multisig.clone(),
                                recipient_b.clone(),
                                fee_asset.clone(),
                                1_u64,
                            ),
                            TransferAssetBatchEntry::new(
                                multisig.clone(),
                                treasury.clone(),
                                fee_asset.clone(),
                                quantity_minor_units(observed),
                            ),
                        ])
                        .into(),
                    ],
                    0,
                    Some(2),
                ),
                None,
            );
            let tx = tx(1, vec![proposal.into()], metadata_for(&policy));

            assert_eq!(enforce_policy(&tx, &policy), Err(expected_error));
        }
    }
}
