//! Validator-side enforcement for chain-level validation-fee policy.

use core::fmt;

use hex;
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    ValidationFail,
    account::AccountId,
    asset::AssetDefinitionId,
    block::BlockHeader,
    isi::{
        InstructionBox, TransferAssetBatch, TransferBox,
        register::RegisterBox,
        repo::{RepoInstructionBox, RepoIsi, ReverseRepoIsi},
        settlement::{
            DvpIsi, PvpIsi, SetFxCorridorPolicy, SettleFxCorridor, SettlementInstructionBox,
        },
    },
    metadata::Metadata,
    prelude::*,
    transaction::{Executable, SignedTransaction},
    validation_fee::{
        SignedValidationFeePolicyV1, VALIDATION_FEE_INSTRUCTION_INDEX_METADATA_KEY,
        VALIDATION_FEE_POLICY_HASH_METADATA_KEY, VALIDATION_FEE_POLICY_VERSION_METADATA_KEY,
        VALIDATION_FEE_TRANSFER_ENTRY_INDEX_METADATA_KEY, ValidationFeeChargingMode,
        ValidationFeeGovernanceKeysetV1, ValidationFeeMultisigMarkerV1,
        ValidationFeePolicyRegistryV1, ValidationFeePolicyV1,
    },
};
use iroha_executor_data_model::isi::multisig::MultisigInstructionBox;
use iroha_primitives::numeric::Numeric;
use mv::storage::StorageReadOnly;

use crate::{
    state::{StateTransaction, WorldReadOnly},
    tx::TransactionRejectionReason,
};

const VALIDATION_FEE_TREASURY_PAYOUT_EXEMPTION_CLASS: &str = "TREASURY_PAYOUT";
/// Contract-visible, consensus-owned fee-credit counter.
///
/// The leaf is stored below the immutable contract-address scope used by the IVM host:
/// `sc/{Hash(contract_address_string)}/AvailableValidationFeeMinorUnits`.
pub(crate) const VALIDATION_FEE_CREDIT_STATE_LEAF: &str = "AvailableValidationFeeMinorUnits";
pub(crate) const VALIDATION_FEE_CREDIT_ASSET_STATE_LEAF: &str =
    "AvailableValidationFeeAssetDefinitionId";

/// Exact protocol-fee value validated from a signed transaction payload.
///
/// This is an admission fact, not a balance mutation. Callers persist it only after the signed
/// transaction and all of its data triggers have completed successfully.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ValidationFeeCredit {
    treasury_account_id: AccountId,
    fee_asset_definition_id: AssetDefinitionId,
    minor_units: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ValidationFeeAdmissionError {
    MissingPolicyParameter,
    MalformedPolicyParameter,
    MissingGovernanceKeysetParameter,
    MalformedGovernanceKeysetParameter,
    InvalidPolicySignature(String),
    MissingPolicyRegistryParameter,
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
    FutureSuccessorPolicy {
        policy_version: u64,
        effective_from_height: u64,
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
    CreditBalanceOverflow {
        current_minor_units: u64,
        additional_minor_units: u64,
    },
    InsufficientCreditBalance {
        available_minor_units: u64,
        requested_minor_units: u64,
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
    UnsupportedNativeFeeAssetMovement {
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
            Self::MissingPolicyParameter => {
                write!(f, "validation-fee signed policy parameter is missing")
            }
            Self::MalformedPolicyParameter => {
                write!(f, "validation-fee policy parameter is malformed")
            }
            Self::MissingGovernanceKeysetParameter => {
                write!(f, "validation-fee governance keyset parameter is missing")
            }
            Self::MalformedGovernanceKeysetParameter => {
                write!(f, "validation-fee governance keyset parameter is malformed")
            }
            Self::InvalidPolicySignature(reason) => {
                write!(
                    f,
                    "validation-fee policy signature verification failed: {reason}"
                )
            }
            Self::MissingPolicyRegistryParameter => {
                write!(f, "validation-fee policy registry parameter is missing")
            }
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
            Self::FutureSuccessorPolicy {
                policy_version,
                effective_from_height,
                current_height,
            } => write!(
                f,
                "active validation-fee successor policy version {policy_version} is not effective until height {effective_from_height}; current height is {current_height}"
            ),
            Self::TreasuryPayoutRequiresActiveContractSubject {
                treasury_account_id,
            } => write!(
                f,
                "signed TREASURY_PAYOUT requires treasury {treasury_account_id} to be an active immutable non-signable contract subject in world state"
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
            Self::CreditBalanceOverflow {
                current_minor_units,
                additional_minor_units,
            } => write!(
                f,
                "validation-fee credit balance overflow: current {current_minor_units} minor units, additional {additional_minor_units} minor units"
            ),
            Self::InsufficientCreditBalance {
                available_minor_units,
                requested_minor_units,
            } => write!(
                f,
                "TREASURY_PAYOUT exceeds validation-fee credit balance: available {available_minor_units} minor units, requested {requested_minor_units} minor units"
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
            Self::UnsupportedNativeFeeAssetMovement {
                context_index,
                instruction_index,
                instruction_wire_id,
            } => write!(
                f,
                "native instruction `{instruction_wire_id}` at instruction {instruction_index} in execution context {context_index} can move the policy DS outside an explicit asset transfer; this path is disabled while the validation-fee policy is active"
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
    let credit = ValidationFeeCredit {
        treasury_account_id: policy_treasury_account_id(&policy).map_err(admission_rejection)?,
        fee_asset_definition_id,
        minor_units: credited_minor_units,
    };
    ensure_validation_fee_credit_capacity(state_transaction, &credit)
        .map_err(admission_rejection)?;
    Ok(Some(credit))
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
        let credit = ValidationFeeCredit {
            treasury_account_id: policy_treasury_account_id(&policy)
                .map_err(admission_rejection)?,
            fee_asset_definition_id,
            minor_units: credited_minor_units,
        };
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
}

impl<'a> OpaqueDeferredRuntimeOrigin<'a> {
    pub(crate) fn new(
        runtime_context: &'a crate::executor::ContractRuntimeExecutionContext,
        code_bytes: &'a [u8],
    ) -> Self {
        Self {
            runtime_context,
            code_bytes,
        }
    }
}

pub(crate) fn enforce_opaque_deferred_instruction_groups(
    instruction_groups: &std::collections::BTreeMap<AccountId, Vec<InstructionBox>>,
    state_transaction: &mut StateTransaction<'_, '_>,
    runtime_origin: Option<OpaqueDeferredRuntimeOrigin<'_>>,
) -> Result<(), TransactionRejectionReason> {
    let Some(policy) = active_policy(state_transaction)? else {
        return Ok(());
    };
    let treasury_payout_authority = verified_opaque_treasury_payout_authority(
        &policy,
        state_transaction,
        runtime_origin.as_ref(),
    )
    .map_err(admission_rejection)?;
    enforce_opaque_deferred_policy(
        instruction_groups,
        &policy,
        treasury_payout_authority.as_ref(),
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

    let payout_minor_units = opaque_treasury_payout_debit_minor_units(
        instruction_groups,
        &policy,
        treasury_payout_authority.as_ref(),
    )
    .map_err(admission_rejection)?;
    if payout_minor_units > 0 {
        let treasury_account_id = treasury_payout_authority.ok_or_else(|| {
            admission_rejection(ValidationFeeAdmissionError::InsufficientCreditBalance {
                available_minor_units: 0,
                requested_minor_units: payout_minor_units,
            })
        })?;
        let credit = ValidationFeeCredit {
            treasury_account_id,
            fee_asset_definition_id,
            minor_units: payout_minor_units,
        };
        consume_validation_fee_credit(state_transaction, &credit).map_err(admission_rejection)?;
    }
    Ok(())
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

fn opaque_treasury_payout_debit_minor_units(
    instruction_groups: &std::collections::BTreeMap<AccountId, Vec<InstructionBox>>,
    policy: &ValidationFeePolicyV1,
    treasury_payout_authority: Option<&AccountId>,
) -> Result<u64, ValidationFeeAdmissionError> {
    let Some(treasury) = treasury_payout_authority else {
        return Ok(0);
    };
    let fee_asset_definition_id = policy_fee_asset_definition_id(policy)?;
    let Some(instructions) = instruction_groups.get(treasury) else {
        return Ok(0);
    };
    let mut collection = TransferCollection {
        contexts: vec![TransferExecutionContext {
            execution_account_id: treasury.clone(),
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
    )?;
    let fee_asset_transfers =
        collect_fee_asset_transfers(&collection.transfers, policy, &fee_asset_definition_id)?;
    fee_asset_transfers
        .iter()
        .filter(|transfer| transfer.context_index == 0 && transfer.source_account_id == *treasury)
        .try_fold(0_u64, |total, transfer| {
            total
                .checked_add(transfer.amount_minor_units)
                .ok_or(ValidationFeeAdmissionError::RequiredFeeOverflow)
        })
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
        let nested_instructions = match action.executable() {
            Executable::Instructions(instructions) => instructions.as_ref(),
            Executable::IvmProved(proved) => proved.overlay.as_ref(),
            Executable::ContractCall(_) | Executable::Ivm(_) => continue,
        };
        reject_opaque_fee_asset_effects(
            action.authority(),
            nested_instructions,
            fee_asset_definition_id,
            None,
        )?;
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
        let nested_instructions = match action.executable() {
            Executable::Instructions(instructions) => instructions.as_ref(),
            Executable::IvmProved(proved) => proved.overlay.as_ref(),
            Executable::ContractCall(_) | Executable::Ivm(_) => continue,
        };
        reject_opaque_deferred_approval_effects_with(
            action.authority(),
            nested_instructions,
            fee_asset_definition_id,
            visited_proposals,
            depth + 1,
            resolve,
        )?;
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
            &treasury,
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
    let parameter_id = ValidationFeePolicyV1::parameter_id();
    let Some(custom) = state_transaction
        .world
        .parameters()
        .custom()
        .get(&parameter_id)
    else {
        let custom_parameters = state_transaction.world.parameters().custom();
        let keyset_present = custom_parameters
            .get(&ValidationFeeGovernanceKeysetV1::parameter_id())
            .is_some();
        let registry_present = custom_parameters
            .get(&ValidationFeePolicyRegistryV1::parameter_id())
            .is_some();
        if keyset_present || registry_present {
            return Err(admission_rejection(
                ValidationFeeAdmissionError::MissingPolicyParameter,
            ));
        }
        return Ok(None);
    };
    let Some(signed_policy) = SignedValidationFeePolicyV1::from_custom_parameter(custom) else {
        return Err(admission_rejection(
            ValidationFeeAdmissionError::MalformedPolicyParameter,
        ));
    };
    let keyset = active_governance_keyset(state_transaction)?;
    let policy = verify_signed_policy(signed_policy, &keyset).map_err(admission_rejection)?;
    let registry = active_policy_registry(state_transaction)?;
    verify_policy_registry(&policy, &registry).map_err(admission_rejection)?;
    let expected_network = state_transaction.chain_id.to_string();
    if policy.network_id != expected_network {
        return Err(admission_rejection(
            ValidationFeeAdmissionError::WrongPolicyNetwork {
                expected: expected_network,
                found: policy.network_id.clone(),
            },
        ));
    }
    validate_policy_genesis_hash(&policy, state_transaction.block_hashes())
        .map_err(admission_rejection)?;
    if !policy_is_active_or_unexpired(&policy, state_transaction.block_height())
        .map_err(admission_rejection)?
    {
        return Ok(None);
    }
    validate_treasury_payout_contract_subject(&policy, state_transaction)
        .map_err(admission_rejection)?;

    Ok(Some(policy))
}

fn validate_treasury_payout_contract_subject(
    policy: &ValidationFeePolicyV1,
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<(), ValidationFeeAdmissionError> {
    if !treasury_payout_exemption_enabled(policy) {
        return Ok(());
    }
    let treasury = policy_treasury_account_id(policy)?;
    let Some(record) = crate::smartcontracts::code::fetch_bound_contract_record_by_subject(
        state_transaction,
        &treasury,
    ) else {
        return Err(
            ValidationFeeAdmissionError::TreasuryPayoutRequiresActiveContractSubject {
                treasury_account_id: treasury.to_string(),
            },
        );
    };
    if record.contract_subject != treasury {
        return Err(
            ValidationFeeAdmissionError::TreasuryPayoutRequiresActiveContractSubject {
                treasury_account_id: treasury.to_string(),
            },
        );
    }
    Ok(())
}

fn verified_opaque_treasury_payout_authority(
    policy: &ValidationFeePolicyV1,
    state_transaction: &StateTransaction<'_, '_>,
    runtime_origin: Option<&OpaqueDeferredRuntimeOrigin<'_>>,
) -> Result<Option<AccountId>, ValidationFeeAdmissionError> {
    if !treasury_payout_exemption_enabled(policy) {
        return Ok(None);
    }
    let treasury = policy_treasury_account_id(policy)?;
    let Some(origin) = runtime_origin else {
        return Ok(None);
    };
    if origin.runtime_context.contract_subject != treasury {
        return Ok(None);
    }
    let Some(record) = crate::smartcontracts::code::fetch_bound_contract_record(
        state_transaction,
        &origin.runtime_context.contract_address,
    ) else {
        return Ok(None);
    };
    if record.contract_subject != treasury || record.code_bytes.as_slice() != origin.code_bytes {
        return Ok(None);
    }
    Ok(Some(treasury))
}

fn policy_is_active_or_unexpired(
    policy: &ValidationFeePolicyV1,
    current_height: u64,
) -> Result<bool, ValidationFeeAdmissionError> {
    if current_height < policy.effective_from_height {
        if policy.policy_version > 1 {
            return Err(ValidationFeeAdmissionError::FutureSuccessorPolicy {
                policy_version: policy.policy_version,
                effective_from_height: policy.effective_from_height,
                current_height,
            });
        }
        return Ok(false);
    }
    if let Some(expires_after_height) = policy.expires_after_height {
        if current_height >= expires_after_height {
            return Err(ValidationFeeAdmissionError::PolicyExpired {
                expires_after_height,
                current_height,
            });
        }
    }
    Ok(true)
}

fn verify_signed_policy(
    signed_policy: SignedValidationFeePolicyV1,
    keyset: &ValidationFeeGovernanceKeysetV1,
) -> Result<ValidationFeePolicyV1, ValidationFeeAdmissionError> {
    signed_policy
        .verify_against_keyset(keyset)
        .map_err(|err| ValidationFeeAdmissionError::InvalidPolicySignature(err.to_string()))?;
    let policy = signed_policy.policy;
    if let Some(reason) = policy.policy_invariant_error() {
        return Err(ValidationFeeAdmissionError::InvalidPolicyInvariant(reason));
    }

    Ok(policy)
}

fn verify_policy_registry(
    policy: &ValidationFeePolicyV1,
    registry: &ValidationFeePolicyRegistryV1,
) -> Result<(), ValidationFeeAdmissionError> {
    registry
        .validate_active_policy(policy)
        .map_err(|err| ValidationFeeAdmissionError::InvalidPolicyRegistry(err.to_string()))
}

fn active_governance_keyset(
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<ValidationFeeGovernanceKeysetV1, TransactionRejectionReason> {
    let parameter_id = ValidationFeeGovernanceKeysetV1::parameter_id();
    let Some(custom) = state_transaction
        .world
        .parameters()
        .custom()
        .get(&parameter_id)
    else {
        return Err(admission_rejection(
            ValidationFeeAdmissionError::MissingGovernanceKeysetParameter,
        ));
    };
    ValidationFeeGovernanceKeysetV1::from_custom_parameter(custom).ok_or_else(|| {
        admission_rejection(ValidationFeeAdmissionError::MalformedGovernanceKeysetParameter)
    })
}

fn active_policy_registry(
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<ValidationFeePolicyRegistryV1, TransactionRejectionReason> {
    let parameter_id = ValidationFeePolicyRegistryV1::parameter_id();
    let Some(custom) = state_transaction
        .world
        .parameters()
        .custom()
        .get(&parameter_id)
    else {
        return Err(admission_rejection(
            ValidationFeeAdmissionError::MissingPolicyRegistryParameter,
        ));
    };
    ValidationFeePolicyRegistryV1::from_custom_parameter(custom).ok_or_else(|| {
        admission_rejection(ValidationFeeAdmissionError::MalformedPolicyRegistryParameter)
    })
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
    policy.ds_asset_id.parse().map_err(|_| {
        ValidationFeeAdmissionError::InvalidPolicyInvariant(
            "validation-fee policy DS asset id is not a ledger asset definition id",
        )
    })
}

fn policy_treasury_account_id(
    policy: &ValidationFeePolicyV1,
) -> Result<AccountId, ValidationFeeAdmissionError> {
    AccountId::parse_encoded(policy.treasury_account_id.as_str())
        .map(iroha_data_model::account::ParsedAccountId::into_account_id)
        .map_err(|_| {
            ValidationFeeAdmissionError::InvalidPolicyInvariant(
                "validation-fee policy treasury account id is not a ledger account id",
            )
        })
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

fn read_validation_fee_credit_balance(
    state_transaction: &StateTransaction<'_, '_>,
    credit: &ValidationFeeCredit,
) -> Result<u64, ValidationFeeAdmissionError> {
    let (key, asset_key) =
        validation_fee_credit_state_keys(state_transaction, &credit.treasury_account_id)?;
    let Some(bytes) = state_transaction.world.smart_contract_state.get(&key) else {
        return Ok(0);
    };
    let value = norito::decode_from_bytes::<i64>(bytes).map_err(|_| {
        ValidationFeeAdmissionError::MalformedCreditBalance {
            state_key: key.to_string(),
        }
    })?;
    let value =
        u64::try_from(value).map_err(|_| ValidationFeeAdmissionError::MalformedCreditBalance {
            state_key: key.to_string(),
        })?;
    if value == 0 {
        return Ok(0);
    }
    let binding_bytes = state_transaction
        .world
        .smart_contract_state
        .get(&asset_key)
        .ok_or_else(
            || ValidationFeeAdmissionError::MalformedCreditAssetBinding {
                state_key: asset_key.to_string(),
            },
        )?;
    let bound_asset =
        norito::decode_from_bytes::<AssetDefinitionId>(binding_bytes).map_err(|_| {
            ValidationFeeAdmissionError::MalformedCreditAssetBinding {
                state_key: asset_key.to_string(),
            }
        })?;
    if bound_asset != credit.fee_asset_definition_id {
        return Err(ValidationFeeAdmissionError::CreditAssetBindingMismatch {
            expected_asset_definition_id: credit.fee_asset_definition_id.to_string(),
            observed_asset_definition_id: bound_asset.to_string(),
        });
    }
    Ok(value)
}

fn ensure_validation_fee_credit_capacity(
    state_transaction: &StateTransaction<'_, '_>,
    credit: &ValidationFeeCredit,
) -> Result<(), ValidationFeeAdmissionError> {
    let current_minor_units = read_validation_fee_credit_balance(state_transaction, credit)?;
    let next = current_minor_units.checked_add(credit.minor_units).ok_or(
        ValidationFeeAdmissionError::CreditBalanceOverflow {
            current_minor_units,
            additional_minor_units: credit.minor_units,
        },
    )?;
    if next > i64::MAX as u64 {
        return Err(ValidationFeeAdmissionError::CreditBalanceOverflow {
            current_minor_units,
            additional_minor_units: credit.minor_units,
        });
    }
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
    let current_minor_units = read_validation_fee_credit_balance(state_transaction, credit)
        .map_err(admission_rejection)?;
    let next = current_minor_units
        .checked_add(credit.minor_units)
        .ok_or_else(|| {
            admission_rejection(ValidationFeeAdmissionError::CreditBalanceOverflow {
                current_minor_units,
                additional_minor_units: credit.minor_units,
            })
        })?;
    let next = i64::try_from(next).map_err(|_| {
        admission_rejection(ValidationFeeAdmissionError::CreditBalanceOverflow {
            current_minor_units,
            additional_minor_units: credit.minor_units,
        })
    })?;
    let (key, asset_key) =
        validation_fee_credit_state_keys(state_transaction, &credit.treasury_account_id)
            .map_err(admission_rejection)?;
    let bytes = norito::to_bytes(&next).map_err(|_| {
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
    if credit.minor_units == 0 {
        return Ok(());
    }
    let available_minor_units = read_validation_fee_credit_balance(state_transaction, credit)?;
    let remaining_minor_units = available_minor_units
        .checked_sub(credit.minor_units)
        .ok_or(ValidationFeeAdmissionError::InsufficientCreditBalance {
            available_minor_units,
            requested_minor_units: credit.minor_units,
        })?;
    let (key, _) =
        validation_fee_credit_state_keys(state_transaction, &credit.treasury_account_id)?;
    let remaining = i64::try_from(remaining_minor_units).map_err(|_| {
        ValidationFeeAdmissionError::MalformedCreditBalance {
            state_key: key.to_string(),
        }
    })?;
    let bytes = norito::to_bytes(&remaining).map_err(|_| {
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
            &treasury,
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
    treasury: &AccountId,
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

    let treasury_payout_exemption_enabled = treasury_payout_exemption_enabled(policy);
    let has_fee_asset_effect = fee_asset_transfers
        .iter()
        .filter(|transfer| transfer.context_index == context_index)
        .any(|transfer| {
            !(&transfer.source_account_id == treasury && treasury_payout_exemption_enabled)
        });
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
    let treasury_payout_exemption_enabled = treasury_payout_exemption_enabled(policy);

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
            if &transfer.source_account_id == treasury && treasury_payout_exemption_enabled {
                continue;
            }
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
        if &transfer.source_account_id == treasury && treasury_payout_exemption_enabled {
            continue;
        }
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
                !(&transfer.source_account_id == treasury && treasury_payout_exemption_enabled)
            })
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
    u64::try_from(
        (qualifying_transfer_count as u128)
            .checked_mul(u128::from(policy.fee_minor_units))
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
    let instructions = match executable {
        Executable::Instructions(instructions) => instructions.as_ref(),
        // The overlay is part of the signed transaction payload and is bound to the bytecode by
        // the proved-IVM attachment. Proof verification still runs before the overlay executes.
        Executable::IvmProved(proved) => proved.overlay.as_ref(),
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
            .downcast_ref::<Transfer<Asset, Numeric, Account>>()
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
    reject_known!(Mint<Numeric, Asset>, Burn<Numeric, Asset>);

    if let Some(instruction_wire_id) =
        native_fee_asset_movement_wire_id(instruction, fee_asset_definition_id)
    {
        return NativeInstructionDsEffectDisposition::RejectKnownDsCapable(instruction_wire_id);
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
        iroha_data_model::isi::account_alias_lease::AcquireAccountAliasLease,
        iroha_data_model::isi::account_alias_lease::RenewAccountAliasLease,
        iroha_data_model::isi::sns::RegisterSnsName,
        iroha_data_model::isi::sns::RenewSnsName,
        iroha_data_model::isi::offline::TopUpKagemushaRecursiveV2,
        iroha_data_model::isi::offline::RedeemKagemushaRecursiveV2,
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
        iroha_data_model::isi::smart_contract_code::ActivateContractInstance,
        SetKeyValueBox,
        RemoveKeyValueBox,
        iroha_data_model::isi::SetAssetKeyValue,
        iroha_data_model::isi::RemoveAssetKeyValue,
        iroha_data_model::isi::AddSignatory,
        iroha_data_model::isi::RemoveSignatory,
        iroha_data_model::isi::SetAccountQuorum,
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
                    .downcast_ref::<Transfer<Asset, Numeric, Account>>()
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

    use iroha_crypto::{Algorithm, Hash, KeyPair, SignatureOf};
    use iroha_data_model::{
        ChainId,
        asset::{AssetDefinitionId, AssetId},
        domain::DomainId,
        events::execute_trigger::ExecuteTriggerEventFilter,
        isi::{
            InstructionBox, Transfer, TransferAssetBatchEntry,
            repo::RepoMarginCallIsi,
            settlement::{SettlementLeg, SettlementPlan},
        },
        prelude::Register,
        repo::{RepoCashLeg, RepoCollateralLeg, RepoGovernance},
        transaction::{
            Executable, IvmBytecode, IvmProved, TransactionBuilder, executable::ContractInvocation,
        },
        trigger::{
            Trigger,
            action::{Action, Repeats},
        },
        validation_fee::{
            SignedValidationFeePolicyV1, VALIDATION_FEE_INSTRUCTION_INDEX_METADATA_KEY,
            VALIDATION_FEE_POLICY_HASH_METADATA_KEY, VALIDATION_FEE_POLICY_SCHEMA_VERSION,
            VALIDATION_FEE_POLICY_VERSION_METADATA_KEY,
            VALIDATION_FEE_TRANSFER_ENTRY_INDEX_METADATA_KEY, ValidationFeeGovernanceKeyV1,
            ValidationFeePolicyRegistryEntryV1, ValidationFeePolicyRegistryV1,
            ValidationFeePolicySignatureV1,
        },
    };
    use iroha_executor_data_model::isi::multisig::{MultisigApprove, MultisigPropose};
    use iroha_primitives::json::Json;

    use super::*;

    const TEST_VALIDATION_FEE_ASSET_SCALE: u8 =
        iroha_data_model::validation_fee::VALIDATION_FEE_DS_SCALE;
    const TEST_VALIDATION_FEE_MINOR_UNITS: u64 =
        iroha_data_model::validation_fee::VALIDATION_FEE_INITIAL_MINOR_UNITS;

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
            network_id: "generic-testnet".to_string(),
            genesis_hash: [7; 32],
            policy_version: 1,
            previous_policy_hash: None,
            ds_asset_id: fee_asset().to_string(),
            ds_scale: TEST_VALIDATION_FEE_ASSET_SCALE,
            fee_minor_units: TEST_VALIDATION_FEE_MINOR_UNITS,
            treasury_account_id: treasury.to_string(),
            charging_mode: ValidationFeeChargingMode::PerQualifyingTransferInstruction,
            effective_from_height: 10,
            expires_after_height: Some(100),
            governance_keyset_id: "validation-fee-governance-v1".to_string(),
            exemption_classes: Vec::new(),
        }
    }

    fn policy_with_treasury_payout_exemption(treasury: &AccountId) -> ValidationFeePolicyV1 {
        let mut policy = policy(treasury);
        policy
            .exemption_classes
            .push(VALIDATION_FEE_TREASURY_PAYOUT_EXEMPTION_CLASS.to_string());
        policy
    }

    fn policy_fee_asset(policy: &ValidationFeePolicyV1) -> AssetDefinitionId {
        policy.ds_asset_id.parse().expect("policy DS asset id")
    }

    fn governance_keyset(
        key_pairs: &[&KeyPair],
        threshold: u16,
    ) -> ValidationFeeGovernanceKeysetV1 {
        ValidationFeeGovernanceKeysetV1 {
            keyset_id: "validation-fee-governance-v1".to_string(),
            threshold,
            keys: key_pairs
                .iter()
                .map(|key_pair| ValidationFeeGovernanceKeyV1 {
                    public_key: key_pair.public_key().clone(),
                    weight: 1,
                })
                .collect(),
        }
    }

    fn signed_policy(
        policy: ValidationFeePolicyV1,
        key_pairs: &[&KeyPair],
    ) -> SignedValidationFeePolicyV1 {
        SignedValidationFeePolicyV1 {
            signatures: key_pairs
                .iter()
                .map(|key_pair| ValidationFeePolicySignatureV1 {
                    public_key: key_pair.public_key().clone(),
                    signature: SignatureOf::try_new(
                        key_pair.private_key(),
                        &policy.signing_payload(),
                    )
                    .expect("policy signature"),
                })
                .collect(),
            policy,
        }
    }

    fn successor_policy(previous: &ValidationFeePolicyV1) -> ValidationFeePolicyV1 {
        let mut policy = previous.clone();
        policy.policy_version += 1;
        policy.previous_policy_hash = Some(previous.policy_hash().expect("previous policy hash"));
        policy.effective_from_height += 100;
        policy.expires_after_height = Some(policy.effective_from_height + 100);
        policy
    }

    fn policy_registry(policies: &[ValidationFeePolicyV1]) -> ValidationFeePolicyRegistryV1 {
        let registered_policies = policies
            .iter()
            .map(|policy| {
                ValidationFeePolicyRegistryEntryV1::from_policy(policy).expect("registry entry")
            })
            .collect::<Vec<_>>();
        let active = registered_policies.last().expect("at least one policy");
        ValidationFeePolicyRegistryV1 {
            active_policy_hash: active.policy_hash,
            active_policy_version: active.policy_version,
            registered_policies,
        }
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
            name: "payout".to_owned(),
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

    fn minor_units(value: u64) -> Numeric {
        Numeric::new(value, u32::from(TEST_VALIDATION_FEE_ASSET_SCALE))
    }

    fn transfer(
        from: &AccountId,
        asset_definition: &AssetDefinitionId,
        amount: Numeric,
        to: &AccountId,
    ) -> InstructionBox {
        Transfer::asset_numeric(
            AssetId::new(asset_definition.clone(), from.clone()),
            amount,
            to.clone(),
        )
        .into()
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
                quantity: Numeric::new(1_u64, 0),
            },
            RepoCollateralLeg::new(collateral_asset.clone(), Numeric::new(1_u64, 0)),
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
                quantity: Numeric::new(1_u64, 0),
            },
            RepoCollateralLeg::new(collateral_asset.clone(), Numeric::new(1_u64, 0)),
            1_000,
        )
    }

    fn settlement_leg(
        asset_definition_id: &AssetDefinitionId,
        from: &AccountId,
        to: &AccountId,
    ) -> SettlementLeg {
        SettlementLeg::new(
            asset_definition_id.clone(),
            Numeric::new(1_u64, 0),
            from.clone(),
            to.clone(),
        )
    }

    fn tx(
        authority_seed: u8,
        instructions: Vec<InstructionBox>,
        metadata: Metadata,
    ) -> SignedTransaction {
        let key_pair = key_pair(authority_seed);
        let chain: ChainId = "generic-testnet".parse().expect("chain id");
        TransactionBuilder::new(chain, AccountId::new(key_pair.public_key().clone()))
            .with_instructions(instructions)
            .with_metadata(metadata)
            .sign(key_pair.private_key())
    }

    fn contract_call_tx(authority_seed: u8, metadata: Metadata) -> SignedTransaction {
        let key_pair = key_pair(authority_seed);
        let chain: ChainId = "generic-testnet".parse().expect("chain id");
        TransactionBuilder::new(chain, AccountId::new(key_pair.public_key().clone()))
            .with_executable(Executable::ContractCall(ContractInvocation {
                contract_address: "tairac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjqddcyq8"
                    .parse()
                    .expect("contract address"),
                entrypoint: "send_transfer".to_owned(),
                arguments: None,
            }))
            .with_metadata(metadata)
            .sign(key_pair.private_key())
    }

    fn ivm_tx(authority_seed: u8, metadata: Metadata) -> SignedTransaction {
        let key_pair = key_pair(authority_seed);
        let chain: ChainId = "generic-testnet".parse().expect("chain id");
        TransactionBuilder::new(chain, AccountId::new(key_pair.public_key().clone()))
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
        TransactionBuilder::new(chain, AccountId::new(key_pair.public_key().clone()))
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
                ActivateContractInstance, RegisterSmartContractBytes, RegisterSmartContractCode,
            },
            smart_contract::manifest::ContractManifest,
        };

        let treasury = account(3);
        let policy = policy(&treasury);
        let code_hash = Hash::new(b"permissionless-contract-artifact");
        let contract_address = "tairac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjqddcyq8"
            .parse()
            .expect("contract address");
        let instructions: Vec<InstructionBox> = vec![
            RegisterSmartContractBytes {
                code_hash,
                code: Vec::new(),
            }
            .into(),
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
                contract_address,
                code_hash,
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
            DeactivateContractInstance, RemoveSmartContractBytes,
        };

        let treasury = account(3);
        let policy = policy(&treasury);
        let code_hash = Hash::new(b"immutable-contract-artifact");
        let contract_address = "tairac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjqddcyq8"
            .parse()
            .expect("contract address");
        let instructions: Vec<InstructionBox> = vec![
            DeactivateContractInstance {
                contract_address,
                reason: Some("attempted policy-era rebind".to_owned()),
            }
            .into(),
            RemoveSmartContractBytes {
                code_hash,
                reason: Some("attempted policy-era removal".to_owned()),
            }
            .into(),
        ];

        for (index, instruction) in instructions.into_iter().enumerate() {
            let instruction_wire_id = if index == 0 {
                core::any::type_name::<DeactivateContractInstance>()
            } else {
                core::any::type_name::<RemoveSmartContractBytes>()
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
        let mint: InstructionBox = Mint::asset_numeric(
            Numeric::new(1_u64, 0),
            AssetId::new(policy_fee_asset(&policy), user),
        )
        .into();
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
    fn active_policy_requires_valid_threshold_signed_policy() {
        let first = key_pair(21);
        let second = key_pair(22);
        let treasury = account(3);
        let policy = policy(&treasury);
        let keyset = governance_keyset(&[&first, &second], 2);

        assert_eq!(
            verify_signed_policy(signed_policy(policy.clone(), &[&first, &second]), &keyset),
            Ok(policy.clone())
        );

        assert!(matches!(
            verify_signed_policy(signed_policy(policy.clone(), &[&first]), &keyset),
            Err(ValidationFeeAdmissionError::InvalidPolicySignature(reason))
                if reason.contains("do not meet threshold")
        ));

        let mut invalid_policy = policy;
        invalid_policy.fee_minor_units = 0;
        assert_eq!(
            verify_signed_policy(signed_policy(invalid_policy, &[&first, &second]), &keyset),
            Err(ValidationFeeAdmissionError::InvalidPolicyInvariant(
                "validation-fee policy amount must be 10 minor units"
            ))
        );
    }

    #[test]
    fn active_policy_registry_requires_monotonic_chain() {
        let treasury = account(3);
        let first = policy(&treasury);
        let second = successor_policy(&first);
        let registry = policy_registry(&[first.clone(), second.clone()]);

        verify_policy_registry(&second, &registry).expect("valid policy chain");

        let mut skipped = registry.clone();
        skipped.registered_policies[1].policy_version = 3;
        assert!(matches!(
            verify_policy_registry(&second, &skipped),
            Err(ValidationFeeAdmissionError::InvalidPolicyRegistry(reason))
                if reason.contains("expected 2, found 3")
        ));

        let mut broken_previous = registry.clone();
        broken_previous.registered_policies[1].previous_policy_hash = Some([9; 32]);
        assert!(matches!(
            verify_policy_registry(&second, &broken_previous),
            Err(ValidationFeeAdmissionError::InvalidPolicyRegistry(reason))
                if reason.contains("previous hash is broken")
        ));

        let rollback = policy_registry(&[first]);
        assert!(matches!(
            verify_policy_registry(&second, &rollback),
            Err(ValidationFeeAdmissionError::InvalidPolicyRegistry(reason))
                if reason.contains("active version mismatch")
        ));
    }

    #[test]
    fn active_policy_window_rejects_expired_policy() {
        let treasury = account(3);
        let policy = policy(&treasury);

        assert_eq!(
            policy_is_active_or_unexpired(&policy, policy.effective_from_height - 1),
            Ok(false)
        );
        assert_eq!(
            policy_is_active_or_unexpired(&policy, policy.effective_from_height),
            Ok(true)
        );
        let successor = successor_policy(&policy);
        assert_eq!(
            policy_is_active_or_unexpired(
                &successor,
                successor.effective_from_height.saturating_sub(1)
            ),
            Err(ValidationFeeAdmissionError::FutureSuccessorPolicy {
                policy_version: successor.policy_version,
                effective_from_height: successor.effective_from_height,
                current_height: successor.effective_from_height.saturating_sub(1),
            })
        );
        assert_eq!(
            policy_is_active_or_unexpired(
                &policy,
                policy.expires_after_height.expect("expiry height") - 1
            ),
            Ok(true)
        );
        assert_eq!(
            policy_is_active_or_unexpired(
                &policy,
                policy.expires_after_height.expect("expiry height")
            ),
            Err(ValidationFeeAdmissionError::PolicyExpired {
                expires_after_height: policy.expires_after_height.expect("expiry height"),
                current_height: policy.expires_after_height.expect("expiry height"),
            })
        );
    }

    #[test]
    fn active_policy_requires_exact_fee_and_signed_metadata() {
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
            TransferAssetBatchEntry::new(
                user.clone(),
                recipient,
                fee_asset.clone(),
                Numeric::new(1u64, 0),
            ),
            TransferAssetBatchEntry::new(
                user.clone(),
                treasury.clone(),
                fee_asset.clone(),
                minor_units(10),
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
        let treasury = account(1);
        let recipient = account(2);
        let other = account(3);
        let policy = policy_with_treasury_payout_exemption(&treasury);
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
            .expect("a verified contract-subject treasury may make its direct signed-class payout");

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
    fn active_treasury_payout_policy_rejects_a_signable_treasury_account() {
        use iroha_data_model::{block::BlockHeader, isi::SetParameter, parameter::Parameter};
        use nonzero_ext::nonzero;

        let treasury = account(3);
        let policy = policy_with_treasury_payout_exemption(&treasury);
        let governance = key_pair(44);
        let keyset = governance_keyset(&[&governance], 1);
        let registry = policy_registry(std::slice::from_ref(&policy));
        let signed = signed_policy(policy, &[&governance]);
        let state = crate::state::State::new_with_chain_for_testing(
            crate::state::World::default(),
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
        for custom in [
            keyset.into_custom_parameter(),
            registry.into_custom_parameter(),
            signed.into_custom_parameter(),
        ] {
            crate::smartcontracts::Execute::execute(
                SetParameter::new(Parameter::Custom(custom)),
                &treasury,
                &mut state_tx,
            )
            .expect("install active validation-fee policy");
        }

        let error = active_policy(&state_tx)
            .expect_err("a signable account cannot back the signed treasury-payout class");
        assert!(
            matches!(error, TransactionRejectionReason::Validation(
                ValidationFail::NotPermitted(ref message)
            ) if message.contains("TREASURY_PAYOUT")
                && message.contains("active immutable non-signable contract subject")),
            "unexpected signable-treasury rejection: {error:?}",
        );
    }

    #[test]
    fn active_treasury_payout_policy_accepts_only_matching_bound_contract_runtime() {
        use iroha_data_model::{
            block::BlockHeader,
            isi::SetParameter,
            nexus::DataSpaceId,
            parameter::Parameter,
            prelude::{Account, Domain},
            smart_contract::ContractAddress,
        };
        use nonzero_ext::nonzero;

        let deployer_key = key_pair(55);
        let deployer = AccountId::new(deployer_key.public_key().clone());
        let domain_id = DomainId::try_new("contracts", "universal").expect("domain id");
        let domain = Domain::new(domain_id).build(&deployer);
        let deployer_account = Account::new(deployer.clone()).build(&deployer);
        let world = crate::state::World::with(
            [domain],
            [deployer_account],
            core::iter::empty::<iroha_data_model::asset::AssetDefinition>(),
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

        let treasury = contract_address.subject_id();
        let recipient = deployer.clone();
        let policy = policy_with_treasury_payout_exemption(&treasury);
        let governance = key_pair(44);
        let keyset = governance_keyset(&[&governance], 1);
        let registry = policy_registry(std::slice::from_ref(&policy));
        let signed = signed_policy(policy.clone(), &[&governance]);
        for custom in [
            keyset.into_custom_parameter(),
            registry.into_custom_parameter(),
            signed.into_custom_parameter(),
        ] {
            crate::smartcontracts::Execute::execute(
                SetParameter::new(Parameter::Custom(custom)),
                &deployer,
                &mut state_tx,
            )
            .expect("install active validation-fee policy");
        }

        active_policy(&state_tx)
            .expect("read active policy")
            .expect("policy is active");
        let runtime = crate::executor::ContractRuntimeExecutionContext {
            contract_address: contract_address.clone(),
            contract_subject: treasury.clone(),
            contract_alias: None,
            entrypoint: "payout".to_owned(),
        };
        let groups = std::collections::BTreeMap::from([(
            treasury.clone(),
            vec![transfer(
                &treasury,
                &policy_fee_asset(&policy),
                minor_units(100),
                &recipient,
            )],
        )]);
        let payout_credit = ValidationFeeCredit {
            treasury_account_id: treasury.clone(),
            fee_asset_definition_id: policy_fee_asset(&policy),
            minor_units: 100,
        };
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
        let wrong_asset_credit = ValidationFeeCredit {
            treasury_account_id: treasury.clone(),
            fee_asset_definition_id: asset_definition("unrelated_ds_successor"),
            minor_units: 1,
        };
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
        state_tx.apply();

        {
            let mut failed_signed_transaction = block.transaction();
            let exact_fee_credit = ValidationFeeCredit {
                treasury_account_id: treasury.clone(),
                fee_asset_definition_id: policy_fee_asset(&policy),
                minor_units: TEST_VALIDATION_FEE_MINOR_UNITS,
            };
            commit_validation_fee_credit(&mut failed_signed_transaction, Some(&exact_fee_credit))
                .expect("stage exact signed fee credit");
            assert_eq!(
                read_validation_fee_credit_balance(&failed_signed_transaction, &payout_credit,)
                    .expect("read staged fee credit"),
                110
            );
            // Simulate a later transaction/data-trigger failure: no staged credit is applied.
        }

        {
            let mut failed_trigger_transaction = block.transaction();
            assert_eq!(
                read_validation_fee_credit_balance(&failed_trigger_transaction, &payout_credit)
                    .expect("read credit after failed signed transaction"),
                100,
                "a failed signed transaction must not create fee credit"
            );
            enforce_opaque_deferred_instruction_groups(
                &groups,
                &mut failed_trigger_transaction,
                Some(OpaqueDeferredRuntimeOrigin::new(&runtime, &code)),
            )
            .expect("matching runtime may stage an exactly credited payout");
            assert_eq!(
                read_validation_fee_credit_balance(&failed_trigger_transaction, &payout_credit,)
                    .expect("read staged debit"),
                0,
                "the validator stages the debit in the trigger subtransaction"
            );
            // Simulate a later swap/instruction failure by dropping the trigger transaction.
        }

        let mut successful_trigger_transaction = block.transaction();
        assert_eq!(
            read_validation_fee_credit_balance(&successful_trigger_transaction, &payout_credit)
                .expect("read rolled-back debit"),
            100,
            "a failed trigger subtransaction must roll its staged credit debit back"
        );

        let altered_code = [code.as_slice(), &[0_u8]].concat();
        let error = enforce_opaque_deferred_instruction_groups(
            &groups,
            &mut successful_trigger_transaction,
            Some(OpaqueDeferredRuntimeOrigin::new(&runtime, &altered_code)),
        )
        .expect_err("altered runtime code must not receive the payout exception");
        assert!(
            matches!(error, TransactionRejectionReason::Validation(
                ValidationFail::NotPermitted(ref message)
            ) if message.contains("opaque deferred executable derived a policy fee-asset transfer")),
            "unexpected altered-code rejection: {error:?}",
        );

        let wrong_runtime = crate::executor::ContractRuntimeExecutionContext {
            contract_address,
            contract_subject: deployer,
            contract_alias: None,
            entrypoint: "payout".to_owned(),
        };
        assert!(
            enforce_opaque_deferred_instruction_groups(
                &groups,
                &mut successful_trigger_transaction,
                Some(OpaqueDeferredRuntimeOrigin::new(&wrong_runtime, &code)),
            )
            .is_err(),
            "a signable runtime authority must not inherit the contract-subject exception"
        );

        enforce_opaque_deferred_instruction_groups(
            &groups,
            &mut successful_trigger_transaction,
            Some(OpaqueDeferredRuntimeOrigin::new(&runtime, &code)),
        )
        .expect("matching active contract runtime may make its direct treasury payout");
        assert_eq!(
            read_validation_fee_credit_balance(&successful_trigger_transaction, &payout_credit)
                .expect("read consumed validation-fee credit"),
            0,
            "matching payout consumes exactly its policy-minor-unit debit"
        );
        successful_trigger_transaction.apply();

        let mut exhausted_credit_transaction = block.transaction();
        let error = enforce_opaque_deferred_instruction_groups(
            &groups,
            &mut exhausted_credit_transaction,
            Some(OpaqueDeferredRuntimeOrigin::new(&runtime, &code)),
        )
        .expect_err("treasury asset holdings without fee credit must not fund another payout");
        assert!(
            matches!(error, TransactionRejectionReason::Validation(
                ValidationFail::NotPermitted(ref message)
            ) if message.contains("exceeds validation-fee credit balance")
                && message.contains("available 0")
                && message.contains("requested 100")),
            "unexpected exhausted-credit rejection: {error:?}"
        );
    }

    #[test]
    fn active_policy_admission_rejects_completed_ivm_proved_axt() {
        use iroha_data_model::{block::BlockHeader, isi::SetParameter, parameter::Parameter};
        use nonzero_ext::nonzero;

        let treasury = account(3);
        let policy = policy(&treasury);
        let governance = key_pair(44);
        let keyset = governance_keyset(&[&governance], 1);
        let registry = policy_registry(std::slice::from_ref(&policy));
        let signed = signed_policy(policy, &[&governance]);
        let state = crate::state::State::new_with_chain_for_testing(
            crate::state::World::default(),
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
        for custom in [
            keyset.into_custom_parameter(),
            registry.into_custom_parameter(),
            signed.into_custom_parameter(),
        ] {
            crate::smartcontracts::Execute::execute(
                SetParameter::new(Parameter::Custom(custom)),
                &treasury,
                &mut state_tx,
            )
            .expect("install active validation-fee policy");
        }

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
    fn unrelated_treasury_inflow_does_not_inflate_signed_fee_credit() {
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
    fn treasury_payout_requires_signed_exemption() {
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
    fn treasury_payout_is_exempt_when_signed_policy_lists_class() {
        let user = account(1);
        let treasury = account(2);
        let policy = policy_with_treasury_payout_exemption(&treasury);
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
        enforce_policy(&treasury_payout, &policy).expect("treasury payout class is signed");
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
    fn signed_policy_version_metadata_is_required_and_exact() {
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
                        Numeric::new(1u64, 0),
                    ),
                    TransferAssetBatchEntry::new(
                        user.clone(),
                        recipient_b,
                        fee_asset.clone(),
                        Numeric::new(1u64, 0),
                    ),
                    TransferAssetBatchEntry::new(user, treasury, fee_asset, minor_units(20)),
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
                            Numeric::new(1u64, 0),
                        ),
                        TransferAssetBatchEntry::new(
                            user.clone(),
                            recipient_b.clone(),
                            fee_asset.clone(),
                            Numeric::new(1u64, 0),
                        ),
                        TransferAssetBatchEntry::new(
                            user.clone(),
                            treasury.clone(),
                            fee_asset.clone(),
                            minor_units(observed),
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
                        Numeric::new(1u64, 0),
                    ),
                    TransferAssetBatchEntry::new(
                        user,
                        treasury.clone(),
                        fee_asset,
                        minor_units(10),
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
                        Numeric::new(1u64, 0),
                    ),
                    TransferAssetBatchEntry::new(
                        user.clone(),
                        recipient_b.clone(),
                        fee_asset.clone(),
                        Numeric::new(1u64, 0),
                    ),
                    TransferAssetBatchEntry::new(
                        user.clone(),
                        wrong_treasury.clone(),
                        fee_asset.clone(),
                        minor_units(20),
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
                        Numeric::new(1u64, 0),
                    ),
                    TransferAssetBatchEntry::new(
                        user.clone(),
                        recipient_b.clone(),
                        fee_asset.clone(),
                        Numeric::new(1u64, 0),
                    ),
                    TransferAssetBatchEntry::new(
                        user.clone(),
                        treasury.clone(),
                        xor,
                        minor_units(20),
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
                        Numeric::new(1u64, 0),
                    ),
                    TransferAssetBatchEntry::new(
                        user,
                        recipient_b,
                        fee_asset.clone(),
                        Numeric::new(1u64, 0),
                    ),
                    TransferAssetBatchEntry::new(
                        sponsor,
                        treasury.clone(),
                        fee_asset,
                        minor_units(20),
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
    fn multisig_proposal_context_fee_requires_signed_policy_metadata() {
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
                            Numeric::new(1u64, 0),
                        ),
                        TransferAssetBatchEntry::new(
                            multisig.clone(),
                            recipient_b,
                            fee_asset.clone(),
                            Numeric::new(1u64, 0),
                        ),
                        TransferAssetBatchEntry::new(
                            multisig,
                            treasury,
                            fee_asset,
                            minor_units(20),
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
                                Numeric::new(1u64, 0),
                            ),
                            TransferAssetBatchEntry::new(
                                multisig.clone(),
                                recipient_b.clone(),
                                fee_asset.clone(),
                                Numeric::new(1u64, 0),
                            ),
                            TransferAssetBatchEntry::new(
                                multisig.clone(),
                                treasury.clone(),
                                fee_asset.clone(),
                                minor_units(observed),
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
