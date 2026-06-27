//! Validator-side enforcement for chain-level validation-fee policy.

use core::fmt;

use hex;
use iroha_crypto::HashOf;
use iroha_data_model::{
    ValidationFail,
    account::AccountId,
    asset::AssetDefinitionId,
    block::BlockHeader,
    isi::{InstructionBox, TransferAssetBatch, TransferBox},
    metadata::Metadata,
    prelude::*,
    transaction::{Executable, SignedTransaction},
    validation_fee::{
        SignedValidationFeePolicyV1, VALIDATION_FEE_INSTRUCTION_INDEX_METADATA_KEY,
        VALIDATION_FEE_POLICY_HASH_METADATA_KEY, VALIDATION_FEE_POLICY_VERSION_METADATA_KEY,
        VALIDATION_FEE_TRANSFER_ENTRY_INDEX_METADATA_KEY, ValidationFeeChargingMode,
        ValidationFeeGovernanceKeysetV1, ValidationFeePolicyRegistryV1, ValidationFeePolicyV1,
    },
};
use iroha_executor_data_model::isi::multisig::MultisigInstructionBox;
use iroha_primitives::numeric::Numeric;

use crate::{
    state::{StateTransaction, WorldReadOnly},
    tx::TransactionRejectionReason,
};

const VALIDATION_FEE_TREASURY_PAYOUT_EXEMPTION_CLASS: &str = "TREASURY_PAYOUT";

#[derive(Debug, Clone, PartialEq, Eq)]
enum ValidationFeeAdmissionError {
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
            Self::PolicyHashFailed => write!(f, "validation-fee policy hash failed"),
            Self::UnsupportedExecutable => {
                write!(
                    f,
                    "validation-fee policy only supports instruction-list transactions"
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TransferExecutionContext {
    execution_account_id: AccountId,
    explicit_fee_coordinate_allowed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AssetTransferSummary {
    context_index: usize,
    explicit_fee_coordinate_allowed: bool,
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
    explicit_fee_coordinate_allowed: bool,
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
) -> Result<(), TransactionRejectionReason> {
    let Some(policy) = active_policy(state_transaction)? else {
        return Ok(());
    };

    enforce_policy(tx, &policy).map_err(|err| {
        TransactionRejectionReason::Validation(ValidationFail::NotPermitted(format!(
            "validation-fee admission rejected transaction: {err}"
        )))
    })
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

    Ok(Some(policy))
}

fn policy_is_active_or_unexpired(
    policy: &ValidationFeePolicyV1,
    current_height: u64,
) -> Result<bool, ValidationFeeAdmissionError> {
    if current_height < policy.effective_from_height {
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
    policy.sbd_asset_id.parse().map_err(|_| {
        ValidationFeeAdmissionError::InvalidPolicyInvariant(
            "validation-fee policy SBD asset id is not a ledger asset definition id",
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

fn enforce_policy(
    tx: &SignedTransaction,
    policy: &ValidationFeePolicyV1,
) -> Result<(), ValidationFeeAdmissionError> {
    match policy.charging_mode {
        ValidationFeeChargingMode::PerQualifyingTransferInstruction => {}
    }

    let fee_asset_definition_id = policy_fee_asset_definition_id(policy)?;
    let treasury = policy_treasury_account_id(policy)?;
    let transfer_collection = collect_asset_transfers(tx.instructions(), tx.authority())?;
    let fee_asset_transfers = collect_fee_asset_transfers(
        &transfer_collection.transfers,
        policy,
        &fee_asset_definition_id,
    )?;
    let fee_coordinate = validation_fee_coordinate(tx.metadata())?;
    let mut requires_policy_metadata = false;

    for (context_index, context) in transfer_collection.contexts.iter().enumerate() {
        let context_fee_coordinate = if context.explicit_fee_coordinate_allowed {
            fee_coordinate
        } else {
            None
        };
        requires_policy_metadata |= enforce_context_policy(
            context_index,
            &context.execution_account_id,
            &treasury,
            policy,
            &fee_asset_definition_id,
            &transfer_collection.transfers,
            &fee_asset_transfers,
            context_fee_coordinate,
        )?;
    }

    if requires_policy_metadata {
        validate_policy_metadata(tx.metadata(), policy)?;
    }
    Ok(())
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
) -> Result<bool, ValidationFeeAdmissionError> {
    let mut qualifying_transfer_count = 0usize;
    let mut implicit_fee_transfers = Vec::new();
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
            if &transfer.source_account_id == execution_account_id
                && &transfer.destination_account_id == treasury
            {
                implicit_fee_transfers.push(transfer);
            } else {
                qualifying_transfer_count += 1;
            }
        }

        if qualifying_transfer_count == 0 {
            return Ok(false);
        }
        if !implicit_fee_transfers.is_empty() {
            return Err(ValidationFeeAdmissionError::DuplicateFeeInstructions {
                count: implicit_fee_transfers.len() + 1,
            });
        }

        let required_fee_minor_units = required_fee_minor_units(qualifying_transfer_count, policy)?;
        if fee_transfer.amount_minor_units != required_fee_minor_units {
            return Err(ValidationFeeAdmissionError::WrongFeeAmount {
                expected_minor_units: required_fee_minor_units,
                observed_minor_units: fee_transfer.amount_minor_units,
            });
        }

        return Ok(true);
    }

    for transfer in fee_asset_transfers
        .iter()
        .filter(|transfer| transfer.context_index == context_index)
    {
        if &transfer.source_account_id == treasury && treasury_payout_exemption_enabled {
            continue;
        }
        if &transfer.source_account_id == execution_account_id
            && &transfer.destination_account_id == treasury
        {
            implicit_fee_transfers.push(transfer);
        } else {
            qualifying_transfer_count += 1;
        }
    }

    if qualifying_transfer_count == 0 {
        return Ok(false);
    }

    let required_fee_minor_units = required_fee_minor_units(qualifying_transfer_count, policy)?;

    if implicit_fee_transfers.is_empty() {
        return Err(ValidationFeeAdmissionError::MissingFee {
            required_minor_units: required_fee_minor_units,
        });
    }
    if implicit_fee_transfers.len() > 1 {
        return Err(ValidationFeeAdmissionError::DuplicateFeeInstructions {
            count: implicit_fee_transfers.len(),
        });
    }

    let fee_transfer = &implicit_fee_transfers[0];
    if fee_transfer.amount_minor_units != required_fee_minor_units {
        return Err(ValidationFeeAdmissionError::WrongFeeAmount {
            expected_minor_units: required_fee_minor_units,
            observed_minor_units: fee_transfer.amount_minor_units,
        });
    }

    Ok(true)
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
        transfer.context_index == context_index
            && transfer.explicit_fee_coordinate_allowed
            && fee_coordinate.matches(*transfer)
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
            transfer.context_index == context_index
                && transfer.explicit_fee_coordinate_allowed
                && fee_coordinate.matches(*transfer)
        })
        .ok_or(ValidationFeeAdmissionError::FeeInstructionNotFound {
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

fn collect_asset_transfers(
    executable: &Executable,
    authority: &AccountId,
) -> Result<TransferCollection, ValidationFeeAdmissionError> {
    let Executable::Instructions(instructions) = executable else {
        return Err(ValidationFeeAdmissionError::UnsupportedExecutable);
    };

    let mut collection = TransferCollection {
        contexts: vec![TransferExecutionContext {
            execution_account_id: authority.clone(),
            explicit_fee_coordinate_allowed: true,
        }],
        transfers: Vec::new(),
    };
    collect_instruction_asset_transfers(instructions.as_ref(), 0, &mut collection);
    Ok(collection)
}

fn collect_instruction_asset_transfers(
    instructions: &[InstructionBox],
    context_index: usize,
    collection: &mut TransferCollection,
) {
    let explicit_fee_coordinate_allowed =
        collection.contexts[context_index].explicit_fee_coordinate_allowed;
    for (instruction_index, instruction) in instructions.iter().enumerate() {
        if let Some(batch) = instruction.as_any().downcast_ref::<TransferAssetBatch>() {
            for (entry_index, entry) in batch.entries().iter().enumerate() {
                collection.transfers.push(AssetTransferSummary {
                    context_index,
                    explicit_fee_coordinate_allowed,
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

        if let Some(transfer_box) = instruction.as_any().downcast_ref::<TransferBox>()
            && let TransferBox::Asset(transfer) = transfer_box
        {
            collection.transfers.push(AssetTransferSummary {
                context_index,
                explicit_fee_coordinate_allowed,
                instruction_index,
                entry_index: None,
                asset_definition_id: transfer.source.definition.clone(),
                source_account_id: transfer.source.account.clone(),
                destination_account_id: transfer.destination.clone(),
                amount: transfer.object.clone(),
            });
            continue;
        }

        let Ok(MultisigInstructionBox::Propose(propose)) =
            MultisigInstructionBox::try_from(instruction)
        else {
            continue;
        };
        let nested_context_index = collection.contexts.len();
        collection.contexts.push(TransferExecutionContext {
            execution_account_id: propose.account,
            explicit_fee_coordinate_allowed: false,
        });
        collect_instruction_asset_transfers(
            &propose.instructions,
            nested_context_index,
            collection,
        );
    }
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
                policy.sbd_scale,
                transfer.instruction_index,
            )?;
            Ok(FeeAssetTransferSummary {
                context_index: transfer.context_index,
                explicit_fee_coordinate_allowed: transfer.explicit_fee_coordinate_allowed,
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
        isi::{InstructionBox, Transfer, TransferAssetBatchEntry},
        transaction::TransactionBuilder,
        validation_fee::{
            SignedValidationFeePolicyV1, VALIDATION_FEE_INSTRUCTION_INDEX_METADATA_KEY,
            VALIDATION_FEE_POLICY_HASH_METADATA_KEY, VALIDATION_FEE_POLICY_SCHEMA_VERSION,
            VALIDATION_FEE_POLICY_VERSION_METADATA_KEY,
            VALIDATION_FEE_TRANSFER_ENTRY_INDEX_METADATA_KEY, ValidationFeeGovernanceKeyV1,
            ValidationFeePolicyRegistryEntryV1, ValidationFeePolicyRegistryV1,
            ValidationFeePolicySignatureV1,
        },
    };
    use iroha_executor_data_model::isi::multisig::MultisigPropose;
    use iroha_primitives::json::Json;

    use super::*;

    const TEST_VALIDATION_FEE_ASSET_SCALE: u8 =
        iroha_data_model::validation_fee::VALIDATION_FEE_SBD_SCALE;
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
            sbd_asset_id: fee_asset().to_string(),
            sbd_scale: TEST_VALIDATION_FEE_ASSET_SCALE,
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
        policy.sbd_asset_id.parse().expect("policy SBD asset id")
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
                required_minor_units: 10
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
            metadata_for_fee_instruction(&policy, 1),
        );

        assert_eq!(
            enforce_policy(&tx, &policy),
            Err(ValidationFeeAdmissionError::DuplicateFeeInstructions { count: 2 })
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
        let treasury = account(2);
        let policy = policy(&treasury);
        let fee_asset = policy_fee_asset(&policy);

        let standalone_fee_like_transfer = tx(
            1,
            vec![transfer(&user, &fee_asset, minor_units(10), &treasury)],
            Metadata::default(),
        );
        enforce_policy(&standalone_fee_like_transfer, &policy)
            .expect("standalone treasury transfer is not recursively charged");
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

        let missing_metadata_tx = tx(1, instructions(), Metadata::default());
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

        enforce_policy(&tx, &policy).expect("batch aggregate fee validates");
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
            Err(ValidationFeeAdmissionError::MissingFee {
                required_minor_units: 10
            })
        );

        let top_level_fee = tx(
            1,
            vec![
                MultisigPropose::new(
                    multisig.clone(),
                    vec![transfer(
                        &multisig,
                        &fee_asset,
                        Numeric::new(1u64, 0),
                        &recipient,
                    )],
                    None,
                )
                .into(),
                transfer(&user, &fee_asset, minor_units(10), &treasury),
            ],
            metadata_for_fee_instruction(&policy, 1),
        );
        assert_eq!(
            enforce_policy(&top_level_fee, &policy),
            Err(ValidationFeeAdmissionError::MissingFee {
                required_minor_units: 10
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
                    vec![
                        transfer(&multisig, &fee_asset, Numeric::new(1u64, 0), &recipient),
                        transfer(&multisig, &fee_asset, minor_units(10), &treasury),
                    ],
                    None,
                )
                .into(),
            ],
            metadata_for(&policy),
        );

        enforce_policy(&tx, &policy).expect("multisig proposal context fee validates");
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
                    TransferAssetBatchEntry::new(multisig, treasury, fee_asset, minor_units(20)),
                ])
                .into(),
            ],
            None,
        );
        let tx = tx(1, vec![proposal.into()], metadata_for(&policy));

        enforce_policy(&tx, &policy).expect("multisig batch aggregate fee validates");
    }
}
