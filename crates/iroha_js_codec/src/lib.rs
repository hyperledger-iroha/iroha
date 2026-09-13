//! Canonical account and instruction codecs shared by JavaScript platform adapters.
//!
//! This crate owns strict JSON admission and native Norito reconstruction. Platform
//! adapters only convert buffers and errors; they do not duplicate ledger codecs.

mod error;
pub use error::{CodecError, CodecErrorKind, CodecResult};
mod archive;
pub use archive::{decode_instruction_archive, encode_instruction_archive};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use iroha_crypto::Hash;
use iroha_data_model::account::Account;
use iroha_data_model::account::AccountId;
use iroha_data_model::account::NewAccount;
use iroha_data_model::account::address::AccountAddress;
use iroha_data_model::account::address::AccountAddressError;
use iroha_data_model::account::address::{ChainDiscriminantGuard, chain_discriminant};
use iroha_data_model::asset::AssetDefinitionAlias;
use iroha_data_model::asset::AssetTransferAvailability;
use iroha_data_model::asset::AssetTransferControlWindow;
use iroha_data_model::asset::AssetTransferLimit;
use iroha_data_model::asset::definition::AssetDefinition;
use iroha_data_model::asset::definition::NewAssetDefinition;
use iroha_data_model::asset::id::AssetDefinitionId;
use iroha_data_model::asset::id::AssetId;
use iroha_data_model::asset::validate_asset_transfer_availability_reason;
use iroha_data_model::domain::Domain;
use iroha_data_model::domain::NewDomain;
use iroha_data_model::escrow::EscrowId;
use iroha_data_model::governance::types::AbiVersion;
use iroha_data_model::governance::types::ContractAbiHash;
use iroha_data_model::governance::types::ContractCodeHash;
use iroha_data_model::isi::Burn;
use iroha_data_model::isi::BurnBox;
use iroha_data_model::isi::CreateKaigi;
use iroha_data_model::isi::CustomInstruction;
use iroha_data_model::isi::EndKaigi;
use iroha_data_model::isi::ExecuteTrigger;
use iroha_data_model::isi::Grant;
use iroha_data_model::isi::GrantBox;
use iroha_data_model::isi::Instruction as InstructionTrait;
use iroha_data_model::isi::InstructionBox;
use iroha_data_model::isi::JoinKaigi;
use iroha_data_model::isi::LeaveKaigi;
use iroha_data_model::isi::Mint;
use iroha_data_model::isi::MintBox;
use iroha_data_model::isi::RecordKaigiUsage;
use iroha_data_model::isi::Register;
use iroha_data_model::isi::RegisterBox;
use iroha_data_model::isi::RegisterKaigiRelay;
use iroha_data_model::isi::RegisterPeerWithPop;
use iroha_data_model::isi::RemoveKeyValue;
use iroha_data_model::isi::ReportKaigiRelayHealth;
use iroha_data_model::isi::SetAssetDefinitionAlias;
use iroha_data_model::isi::SetKaigiRelayManifest;
use iroha_data_model::isi::SetKeyValue;
use iroha_data_model::isi::SetKeyValueBox;
use iroha_data_model::isi::SetParameter;
use iroha_data_model::isi::Transfer;
use iroha_data_model::isi::TransferAssetBatch;
use iroha_data_model::isi::TransferBox;
use iroha_data_model::isi::Unregister;
use iroha_data_model::isi::UnregisterBox;
use iroha_data_model::isi::UnregisterKaigiRelay;
use iroha_data_model::isi::asset_transfer_control::SetAssetTransferAvailability;
use iroha_data_model::isi::asset_transfer_control::SetAssetTransferBlacklist;
use iroha_data_model::isi::asset_transfer_control::SetAssetTransferControl;
use iroha_data_model::isi::escrow::CancelAssetLock;
use iroha_data_model::isi::governance::CastPlainBallot;
use iroha_data_model::isi::governance::CastZkBallot;
use iroha_data_model::isi::governance::ProposeDeployContract;
use iroha_data_model::isi::governance::ProposeValidationFeePolicy;
use iroha_data_model::isi::governance::RegisterCitizen;
use iroha_data_model::isi::ministry::SubmitAgendaProposal;
use iroha_data_model::isi::rwa::ForceTransferRwa;
use iroha_data_model::isi::rwa::FreezeRwa;
use iroha_data_model::isi::rwa::HoldRwa;
use iroha_data_model::isi::rwa::MergeRwas;
use iroha_data_model::isi::rwa::RedeemRwa;
use iroha_data_model::isi::rwa::RegisterRwa;
use iroha_data_model::isi::rwa::ReleaseRwa;
use iroha_data_model::isi::rwa::RwaInstructionBox;
use iroha_data_model::isi::rwa::SetRwaControls;
use iroha_data_model::isi::rwa::TransferRwa;
use iroha_data_model::isi::rwa::UnfreezeRwa;
use iroha_data_model::isi::settlement::DvpIsi;
use iroha_data_model::isi::settlement::FundFxCorridorEscrow;
use iroha_data_model::isi::settlement::PvpIsi;
use iroha_data_model::isi::settlement::RefundFxCorridorEscrow;
use iroha_data_model::isi::settlement::SetFxCorridorPolicy;
use iroha_data_model::isi::settlement::SettleFxCorridor;
use iroha_data_model::isi::settlement::SettlementInstructionBox;
use iroha_data_model::isi::smart_contract_code::ActivateContractInstance;
use iroha_data_model::isi::smart_contract_code::CancelSmartContractCodeUpload;
use iroha_data_model::isi::smart_contract_code::CommitContractDeployment;
use iroha_data_model::isi::smart_contract_code::DeactivateContractInstance;
use iroha_data_model::isi::smart_contract_code::FinalizeSmartContractCodeUpload;
use iroha_data_model::isi::smart_contract_code::RegisterSmartContractBytes;
use iroha_data_model::isi::smart_contract_code::RegisterSmartContractCode;
use iroha_data_model::isi::smart_contract_code::RemoveSmartContractBytes;
use iroha_data_model::isi::smart_contract_code::UploadSmartContractCodeChunk;
use iroha_data_model::isi::social::CancelTwitterEscrow;
use iroha_data_model::isi::social::ClaimTwitterFollowReward;
use iroha_data_model::isi::social::SendToTwitter;
use iroha_data_model::isi::zk::CancelConfidentialPolicyTransition;
use iroha_data_model::isi::zk::CreateElection;
use iroha_data_model::isi::zk::FinalizeElection;
use iroha_data_model::isi::zk::RegisterZkAsset;
use iroha_data_model::isi::zk::ScheduleConfidentialPolicyTransition;
use iroha_data_model::isi::zk::SubmitBallot;
use iroha_data_model::kaigi::KaigiId;
use iroha_data_model::kaigi::KaigiParticipantCommitment;
use iroha_data_model::kaigi::KaigiParticipantNullifier;
use iroha_data_model::kaigi::KaigiRelayHealthStatus;
use iroha_data_model::kaigi::KaigiRelayRegistration;
use iroha_data_model::kaigi::NewKaigi;
use iroha_data_model::kaigi::scalar::KaigiAuthorizationScalarV1;
use iroha_data_model::ministry::AgendaProposalV1;
use iroha_data_model::nft::NewNft;
use iroha_data_model::nft::Nft;
use iroha_data_model::nft::NftId;
use iroha_data_model::oracle::KeyedHash;
use iroha_data_model::parameter::CustomParameter;
use iroha_data_model::parameter::Parameter;
use iroha_data_model::peer::Peer;
use iroha_data_model::permission::Permission;
use iroha_data_model::role::NewRole;
use iroha_data_model::role::Role;
use iroha_data_model::role::RoleId;
use iroha_data_model::rwa::NewRwa;
use iroha_data_model::rwa::RwaControlPolicy;
use iroha_data_model::rwa::RwaId;
use iroha_data_model::rwa::RwaParentRef;
use iroha_data_model::trigger::Trigger;
use iroha_data_model::trigger::TriggerId;
use iroha_data_model::trigger::action::Action;
use iroha_data_model::validation_fee::ValidationFeePolicyV1;
use iroha_model_base::domain::DomainId;
use iroha_model_base::metadata::Metadata;
use iroha_model_base::name::Name;
use iroha_model_base::peer::PeerId;
use iroha_primitives::json::Json;
use iroha_primitives::numeric::Quantity;
use norito::core as norito_core;
use norito::json;
use std::fmt;
use std::panic::AssertUnwindSafe;
use std::panic::catch_unwind;
use std::str::FromStr;

macro_rules! norito_json {
    ({ $($key:literal : $value:expr),+ $(,)? }) => {{
        let mut object = norito::json::Map::new();
        $(object.insert($key.to_string(), norito::json::to_value(&$value)
            .expect("serialize shared canonical instruction JSON"));)*
        norito::json::Value::Object(object)
    }};
}

/// Native admission result for an exact encoded account.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedAccountAddress {
    /// Canonical account controller envelope, without a chain prefix.
    pub canonical_bytes: Vec<u8>,
    /// Chain discriminant decoded from the admitted I105 literal.
    pub network_prefix: u16,
}

/// Native rendering of an admitted account controller.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderedAccountAddress {
    /// Exact lowercase canonical hexadecimal controller envelope.
    pub canonical_hex: String,
    /// Exact I105 literal for the requested chain discriminant.
    pub i105: String,
}

/// Parse exact I105 with native controller admission and an optional network constraint.
pub fn account_address_parse_encoded(
    input: &str,
    expected_prefix: Option<u16>,
) -> CodecResult<ParsedAccountAddress> {
    let address =
        AccountAddress::parse_encoded(input, expected_prefix).map_err(account_address_err)?;
    let network_prefix = match expected_prefix {
        Some(prefix) => prefix,
        None => AccountAddress::i105_discriminant(input).map_err(account_address_err)?,
    };
    let canonical_hex = address.canonical_hex().map_err(account_address_err)?;
    let hex_body = canonical_hex
        .strip_prefix("0x")
        .unwrap_or(canonical_hex.as_str());
    let canonical = hex::decode(hex_body).map_err(|err| CodecError::failure(err.to_string()))?;
    Ok(ParsedAccountAddress {
        canonical_bytes: canonical,
        network_prefix,
    })
}

/// Admit exact canonical controller bytes and render them for the requested network.
pub fn account_address_render(
    bytes: &[u8],
    network_prefix: u16,
) -> CodecResult<RenderedAccountAddress> {
    let address = AccountAddress::from_canonical_bytes(bytes).map_err(account_address_err)?;
    let canonical_hex = address.canonical_hex().map_err(account_address_err)?;
    let i105 = address
        .to_i105_for_discriminant(network_prefix)
        .map_err(account_address_err)?;
    Ok(RenderedAccountAddress {
        canonical_hex,
        i105,
    })
}

/// Admit a JavaScript numeric network prefix without truncation or integer wrapping.
pub fn checked_network_prefix(prefix: f64) -> CodecResult<u16> {
    if !prefix.is_finite() || prefix.fract() != 0.0 || !(0.0..=65535.0).contains(&prefix) {
        return Err(CodecError::new(
            CodecErrorKind::InvalidArgument,
            "network prefix must be an integer between 0 and 65535",
        ));
    }
    Ok(prefix as u16)
}

/// Parse an instruction account operand under the caller's selected network scope.
///
/// Typed native callers must enter `ChainDiscriminantGuard` before calling this helper.
pub fn parse_account_id(input: &str, label: &str) -> CodecResult<AccountId> {
    let parsed = match AccountAddress::parse_encoded(input, Some(chain_discriminant())) {
        Ok(address) => address.to_account_id().map_err(|err| err.to_string()),
        Err(AccountAddressError::UnsupportedAddressFormat) => {
            AccountId::parse_encoded(input).map_err(|err| err.to_string())
        }
        Err(err) => Err(err.to_string()),
    };
    parsed.map_err(|err| {
        CodecError::new(
            CodecErrorKind::InvalidArgument,
            format!("invalid {label}: {err}"),
        )
    })
}

/// Encode the strict JavaScript instruction JSON contract into a public Norito frame.
///
/// `network_prefix` selects account admission and rendering for this operation only.
pub fn encode_instruction_frame(json_payload: &str, network_prefix: u16) -> CodecResult<Vec<u8>> {
    let _network = ChainDiscriminantGuard::enter(network_prefix);
    let instruction = instruction_from_json(json_payload)?;
    let encoded = norito::encode_canonical(&instruction).map_err(codec_error)?;
    Ok(encoded)
}

/// Decode an exact public instruction frame into the strict JavaScript JSON contract.
///
/// Domainless account identities are rendered with the required `network_prefix`.
pub fn decode_instruction_frame(bytes: &[u8], network_prefix: u16) -> CodecResult<String> {
    let _network = ChainDiscriminantGuard::enter(network_prefix);
    let decode = catch_unwind(AssertUnwindSafe(|| {
        let slice = bytes;
        let instruction = decode_instruction_aligned(slice).map_err(codec_error)?;
        let value = instruction_to_json_value(&instruction)?;
        json::to_json(&value).map_err(codec_error)
    }));
    match decode {
        Ok(result) => result,
        Err(payload) => {
            let message = payload
                .downcast_ref::<&str>()
                .copied()
                .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
                .unwrap_or("unknown panic");
            Err(CodecError::new(
                CodecErrorKind::Failure,
                format!("panic during Norito decode: {message}"),
            ))
        }
    }
}

/// Decode the current public instruction frame variants with native canonical validation.
pub fn decode_instruction_aligned(bytes: &[u8]) -> Result<InstructionBox, norito_core::Error> {
    let primary_error = match norito::decode_canonical::<InstructionBox>(bytes) {
        Ok(instruction) => return Ok(instruction),
        Err(error) => error,
    };
    match norito::decode_canonical::<ProposeValidationFeePolicy>(bytes) {
        Ok(instruction) => Ok(instruction.into()),
        Err(_) => Err(primary_error),
    }
}

/// Preserve the canonical account error code and diagnostic for platform adapters.
pub fn account_address_err(err: AccountAddressError) -> CodecError {
    CodecError::new(
        CodecErrorKind::InvalidArgument,
        format!("{}: {err}", err.code_str()),
    )
}

fn codec_error<E: fmt::Display>(error: E) -> CodecError {
    CodecError::new(CodecErrorKind::Failure, error.to_string())
}

fn parse_hash_string(input: &str, context: &str) -> CodecResult<Hash> {
    let trimmed = input.trim();
    if trimmed.starts_with("hash:") {
        return json::from_value(json::Value::String(trimmed.to_owned())).map_err(codec_error);
    }
    if trimmed.len() != Hash::LENGTH * 2
        || !trimmed
            .chars()
            .all(|c| c.is_ascii_hexdigit() || c.is_ascii_whitespace())
    {
        return Err(CodecError::new(
            CodecErrorKind::InvalidArgument,
            format!("{context} must be a 64-character hexadecimal hash literal"),
        ));
    }
    let uppercase = trimmed.to_ascii_uppercase();
    Hash::from_str(&uppercase).map_err(|err| {
        CodecError::new(
            CodecErrorKind::InvalidArgument,
            format!("{context} invalid hash literal: {err}"),
        )
    })
}

/// Parse the hash representation accepted by existing instruction operands.
pub fn parse_hash_value(value: json::Value, context: &str) -> CodecResult<Hash> {
    match value {
        json::Value::String(ref s) => parse_hash_string(s, context),
        other => json::from_value(other).map_err(codec_error),
    }
}

fn parse_canonical_hash_value(value: json::Value, context: &str) -> CodecResult<Hash> {
    json::from_value(value).map_err(|err| {
        CodecError::new(
            CodecErrorKind::InvalidArgument,
            format!("{context} must be a canonical uppercase checksummed hash literal: {err}"),
        )
    })
}

fn parse_optional_hash(value: Option<json::Value>, context: &str) -> CodecResult<Option<Hash>> {
    match value {
        None | Some(json::Value::Null) => Ok(None),
        Some(value) => parse_hash_value(value, context).map(Some),
    }
}

fn parse_optional_string_value(
    value: Option<json::Value>,
    context: &str,
) -> CodecResult<Option<String>> {
    match value {
        None | Some(json::Value::Null) => Ok(None),
        Some(json::Value::String(s)) => Ok(Some(s)),
        Some(other) => parse_string_value(other, context).map(Some),
    }
}

fn parse_keyed_hash(value: json::Value, context: &str) -> CodecResult<KeyedHash> {
    let mut map = match value {
        json::Value::Object(map) => map,
        other => {
            return Err(CodecError::new(
                CodecErrorKind::InvalidArgument,
                format!("{context} must be an object (found {other:?})"),
            ));
        }
    };
    let pepper_id = parse_string_value(
        required_value(&mut map, "pepper_id", context)?,
        &format!("{context}.pepper_id"),
    )?;
    let digest = parse_hash_value(
        required_value(&mut map, "digest", context)?,
        &format!("{context}.digest"),
    )?;
    Ok(KeyedHash { pepper_id, digest })
}

/// Parse an optional Kaigi authorization scalar with native validation.
pub fn parse_optional_kaigi_scalar(
    value: Option<json::Value>,
    context: &str,
) -> CodecResult<Option<KaigiAuthorizationScalarV1>> {
    match value {
        None | Some(json::Value::Null) => Ok(None),
        Some(value) => json::from_value(value).map(Some).map_err(|err| {
            CodecError::new(CodecErrorKind::InvalidArgument, format!("{context}: {err}"))
        }),
    }
}

/// Parse an optional Kaigi participant commitment with native validation.
pub fn parse_optional_commitment(
    value: Option<json::Value>,
    context: &str,
) -> CodecResult<Option<KaigiParticipantCommitment>> {
    match value {
        None | Some(json::Value::Null) => Ok(None),
        Some(value) => json::from_value(value).map(Some).map_err(|err| {
            CodecError::new(
                CodecErrorKind::InvalidArgument,
                format!("{context}.commitment: {err}"),
            )
        }),
    }
}

/// Parse an optional Kaigi participant nullifier with native validation.
pub fn parse_optional_nullifier(
    value: Option<json::Value>,
    context: &str,
) -> CodecResult<Option<KaigiParticipantNullifier>> {
    match value {
        None | Some(json::Value::Null) => Ok(None),
        Some(value) => json::from_value(value).map(Some).map_err(|err| {
            CodecError::new(
                CodecErrorKind::InvalidArgument,
                format!("{context}.nullifier: {err}"),
            )
        }),
    }
}

/// Render an optional Kaigi authorization scalar as its canonical JSON value.
pub fn optional_kaigi_scalar_to_json(value: Option<&KaigiAuthorizationScalarV1>) -> json::Value {
    value.map_or(json::Value::Null, |scalar| {
        json::to_value(scalar).expect("Kaigi scalar serialization")
    })
}

fn optional_hash_to_json(value: Option<&Hash>) -> json::Value {
    value.map_or(json::Value::Null, |hash| {
        json::to_value(hash).expect("hash serialization")
    })
}

/// Render an optional participant commitment as its canonical JSON value.
pub fn optional_commitment_to_json(value: Option<&KaigiParticipantCommitment>) -> json::Value {
    value.map_or(json::Value::Null, |commitment| {
        json::to_value(commitment).expect("Kaigi commitment serialization")
    })
}

/// Render an optional participant nullifier as its canonical JSON value.
pub fn optional_nullifier_to_json(value: Option<&KaigiParticipantNullifier>) -> json::Value {
    value.map_or(json::Value::Null, |nullifier| {
        json::to_value(nullifier).expect("Kaigi nullifier serialization")
    })
}

fn optional_proof_to_json(value: Option<&Vec<u8>>) -> json::Value {
    value.map_or(json::Value::Null, |bytes| {
        json::Value::String(STANDARD.encode(bytes))
    })
}

fn parse_optional_base64(
    value: Option<json::Value>,
    context: &str,
) -> CodecResult<Option<Vec<u8>>> {
    match value {
        None | Some(json::Value::Null) => Ok(None),
        Some(json::Value::String(s)) => STANDARD.decode(s.as_bytes()).map(Some).map_err(|err| {
            CodecError::new(
                CodecErrorKind::InvalidArgument,
                format!("{context} must be a valid base64 string: {err}"),
            )
        }),
        Some(other) => json::from_value(other).map_err(codec_error),
    }
}

fn parse_base64(value: json::Value, context: &str) -> CodecResult<Vec<u8>> {
    match value {
        json::Value::String(s) => STANDARD.decode(s.as_bytes()).map_err(|err| {
            CodecError::new(
                CodecErrorKind::InvalidArgument,
                format!("{context} must be a valid base64 string: {err}"),
            )
        }),
        json::Value::Array(bytes) => {
            let mut buffer = Vec::with_capacity(bytes.len());
            for (index, value) in bytes.into_iter().enumerate() {
                let number = match value {
                    json::Value::Number(n) => n.as_u64().ok_or_else(|| {
                        CodecError::new(
                            CodecErrorKind::InvalidArgument,
                            format!("{context}[{index}] must be an unsigned byte"),
                        )
                    })?,
                    other => {
                        return Err(CodecError::new(
                            CodecErrorKind::InvalidArgument,
                            format!("{context}[{index}] must be an unsigned byte, found {other:?}"),
                        ));
                    }
                };
                if number > 0xFF {
                    return Err(CodecError::new(
                        CodecErrorKind::InvalidArgument,
                        format!("{context}[{index}] must be between 0 and 255"),
                    ));
                }
                buffer.push(u8::try_from(number).expect("validated byte range"));
            }
            Ok(buffer)
        }
        other => Err(CodecError::new(
            CodecErrorKind::InvalidArgument,
            format!("{context} must be a base64 string or byte array (found {other:?})"),
        )),
    }
}

fn required_value(map: &mut json::Map, key: &str, context: &str) -> CodecResult<json::Value> {
    map.remove(key).ok_or_else(|| {
        CodecError::new(
            CodecErrorKind::InvalidArgument,
            format!("{context}.{key} field missing"),
        )
    })
}

fn parse_string_value(value: json::Value, context: &str) -> CodecResult<String> {
    match value {
        json::Value::String(s) => Ok(s),
        other => Err(CodecError::new(
            CodecErrorKind::InvalidArgument,
            format!("{context} must be a string (found {other:?})"),
        )),
    }
}

fn parse_account_id_value(value: json::Value, context: &str) -> CodecResult<AccountId> {
    let literal = parse_string_value(value, context)?;
    parse_account_id(&literal, context)
}

fn parse_rwa_id_value(value: json::Value, context: &str) -> CodecResult<RwaId> {
    let literal = parse_string_value(value, context)?;
    literal.parse().map_err(|err| {
        CodecError::new(
            CodecErrorKind::InvalidArgument,
            format!("invalid RWA id `{literal}`: {err}"),
        )
    })
}

/// Render an instruction account operand as its canonical I105 literal.
pub fn account_id_to_canonical_i105(account_id: &AccountId) -> CodecResult<String> {
    account_id.canonical_i105().map_err(|err| {
        CodecError::new(
            CodecErrorKind::InvalidArgument,
            format!("failed to encode account id as canonical I105: {err}"),
        )
    })
}

/// Parse and validate the complete list of RWA parent references.
pub fn parse_rwa_parent_refs_value(
    value: json::Value,
    context: &str,
) -> CodecResult<Vec<RwaParentRef>> {
    let json::Value::Array(entries) = value else {
        return Err(CodecError::new(
            CodecErrorKind::InvalidArgument,
            format!("{context} must be an array"),
        ));
    };
    let mut parents = Vec::with_capacity(entries.len());
    for (index, entry) in entries.into_iter().enumerate() {
        let entry_context = format!("{context}[{index}]");
        let json::Value::Object(mut fields) = entry else {
            return Err(CodecError::new(
                CodecErrorKind::InvalidArgument,
                format!("{entry_context} must be an object"),
            ));
        };
        let rwa = parse_rwa_id_value(
            required_value(&mut fields, "rwa", &entry_context)?,
            &format!("{entry_context}.rwa"),
        )?;
        let quantity: Quantity =
            json::from_value(required_value(&mut fields, "quantity", &entry_context)?)
                .map_err(codec_error)?;
        parents.push(RwaParentRef::new(rwa, quantity));
    }
    Ok(parents)
}

/// Render RWA parent references without changing their JSON contract.
pub fn rwa_parent_refs_to_json(parents: &[RwaParentRef]) -> json::Value {
    json::Value::Array(
        parents
            .iter()
            .map(|parent| {
                norito_json!({
                    "rwa": parent.rwa().to_string(),
                    "quantity": parent.quantity(),
                })
            })
            .collect(),
    )
}

fn rwa_status_to_json(status: Option<&Name>) -> json::Value {
    status.map_or(json::Value::Null, |status| {
        json::Value::String(status.to_string())
    })
}

fn rwa_control_policy_to_json(policy: &RwaControlPolicy) -> CodecResult<json::Value> {
    let controller_accounts = policy
        .controller_accounts()
        .iter()
        .map(account_id_to_canonical_i105)
        .collect::<CodecResult<Vec<_>>>()?;
    let mut payload = json::Map::new();
    payload.insert(
        "controller_accounts".to_owned(),
        json::to_value(&controller_accounts).map_err(codec_error)?,
    );
    payload.insert(
        "controller_roles".to_owned(),
        json::to_value(
            &policy
                .controller_roles()
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
        )
        .map_err(codec_error)?,
    );
    payload.insert(
        "freeze_enabled".to_owned(),
        json::Value::Bool(*policy.freeze_enabled()),
    );
    payload.insert(
        "hold_enabled".to_owned(),
        json::Value::Bool(*policy.hold_enabled()),
    );
    payload.insert(
        "force_transfer_enabled".to_owned(),
        json::Value::Bool(*policy.force_transfer_enabled()),
    );
    payload.insert(
        "redeem_enabled".to_owned(),
        json::Value::Bool(*policy.redeem_enabled()),
    );
    Ok(json::Value::Object(payload))
}

/// Render a new RWA, including its typed control policy and parent references.
pub fn new_rwa_to_json(rwa: &NewRwa) -> CodecResult<json::Value> {
    Ok(norito_json!({
        "domain": rwa.domain(),
        "quantity": rwa.quantity(),
        "spec": rwa.spec(),
        "primary_reference": rwa.primary_reference(),
        "status": rwa_status_to_json(rwa.status().as_ref()),
        "metadata": rwa.metadata(),
        "parents": rwa_parent_refs_to_json(rwa.parents()),
        "controls": rwa_control_policy_to_json(rwa.controls())?,
    }))
}

fn normalize_zk_ballot_public_inputs_json(raw: &str, context: &str) -> CodecResult<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(CodecError::new(
            CodecErrorKind::InvalidArgument,
            format!("{context} must be valid JSON"),
        ));
    }
    let mut value: json::Value = json::from_str(trimmed).map_err(|err| {
        CodecError::new(
            CodecErrorKind::InvalidArgument,
            format!("{context} must be valid JSON: {err}"),
        )
    })?;
    normalize_zk_ballot_public_inputs(&mut value, context)?;
    json::to_string(&value).map_err(|err| {
        CodecError::new(
            CodecErrorKind::InvalidArgument,
            format!("{context} must be valid JSON: {err}"),
        )
    })
}

/// Validate and normalize the account, amount, and hash inputs of a ZK ballot.
pub fn normalize_zk_ballot_public_inputs(
    value: &mut json::Value,
    context: &str,
) -> CodecResult<()> {
    let map = match value {
        json::Value::Object(map) => map,
        other => {
            return Err(CodecError::new(
                CodecErrorKind::InvalidArgument,
                format!("{context} must be a JSON object (found {other:?})"),
            ));
        }
    };
    reject_zk_public_input_key(map, "durationBlocks", "duration_blocks", context)?;
    reject_zk_public_input_key(map, "root_hint_hex", "root_hint", context)?;
    reject_zk_public_input_key(map, "rootHintHex", "root_hint", context)?;
    reject_zk_public_input_key(map, "rootHint", "root_hint", context)?;
    reject_zk_public_input_key(map, "nullifier_hex", "nullifier", context)?;
    reject_zk_public_input_key(map, "nullifierHex", "nullifier", context)?;
    canonicalize_hex32_public_input(map, "root_hint", "root_hint", context)?;
    canonicalize_hex32_public_input(map, "nullifier", "nullifier", context)?;
    let has_owner = zk_hint_present(map, "owner");
    let has_amount = zk_hint_present(map, "amount");
    let has_duration = zk_hint_present(map, "duration_blocks");
    let any = has_owner || has_amount || has_duration;
    if any && !(has_owner && has_amount && has_duration) {
        return Err(CodecError::new(
            CodecErrorKind::InvalidArgument,
            format!(
                "{context} must include owner, amount, and duration_blocks when providing lock hints"
            ),
        ));
    }
    ensure_zk_public_input_owner_canonical(map, context)?;
    ensure_zk_public_input_amount_canonical(map, context)?;
    Ok(())
}

fn reject_zk_public_input_key(
    map: &json::Map,
    key: &str,
    canonical: &str,
    context: &str,
) -> CodecResult<()> {
    if map.contains_key(key) {
        return Err(CodecError::new(
            CodecErrorKind::InvalidArgument,
            format!("{context} must use {canonical} (unsupported key {key})"),
        ));
    }
    Ok(())
}

fn ensure_zk_public_input_owner_canonical(map: &json::Map, context: &str) -> CodecResult<()> {
    let Some(value) = map.get("owner") else {
        return Ok(());
    };
    if matches!(value, json::Value::Null) {
        return Ok(());
    }
    let owner = value.as_str().ok_or_else(|| {
        CodecError::new(
            CodecErrorKind::InvalidArgument,
            format!("{context}.owner must be a canonical I105 account id"),
        )
    })?;
    let canonical = AccountId::parse_encoded(owner)
        .map(|account| account.to_string())
        .map_err(|_| {
            CodecError::new(
                CodecErrorKind::InvalidArgument,
                format!("{context}.owner must be a canonical I105 account id"),
            )
        })?;
    if canonical != owner {
        return Err(CodecError::new(
            CodecErrorKind::InvalidArgument,
            format!("{context}.owner must use canonical I105 account id form"),
        ));
    }
    Ok(())
}

fn ensure_zk_public_input_amount_canonical(map: &json::Map, context: &str) -> CodecResult<()> {
    let Some(value) = map.get("amount") else {
        return Ok(());
    };
    if matches!(value, json::Value::Null) {
        return Ok(());
    }
    let amount = value.as_str().ok_or_else(|| {
        CodecError::new(
            CodecErrorKind::InvalidArgument,
            format!("{context}.amount must be a canonical Quantity string"),
        )
    })?;
    parse_canonical_quantity_text(amount, &format!("{context}.amount")).map(|_| ())
}

fn canonicalize_hex32_public_input(
    map: &mut json::Map,
    key: &str,
    label: &str,
    context: &str,
) -> CodecResult<()> {
    let Some(value) = map.get_mut(key) else {
        return Ok(());
    };
    if matches!(value, json::Value::Null) {
        return Ok(());
    }
    let raw = value.as_str().ok_or_else(|| {
        CodecError::new(
            CodecErrorKind::InvalidArgument,
            format!("{context}.{label} must be 32-byte hex"),
        )
    })?;
    let canonical = canonicalize_hex32_value(raw).ok_or_else(|| {
        CodecError::new(
            CodecErrorKind::InvalidArgument,
            format!("{context}.{label} must be 32-byte hex"),
        )
    })?;
    *value = json::Value::String(canonical);
    Ok(())
}

fn canonicalize_hex32_value(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    let without_scheme = if let Some((scheme, rest)) = trimmed.split_once(':') {
        if scheme.is_empty() || scheme.eq_ignore_ascii_case("blake2b32") {
            rest
        } else {
            return None;
        }
    } else {
        trimmed
    };
    let body = without_scheme.trim();
    let body = body
        .strip_prefix("0x")
        .or_else(|| body.strip_prefix("0X"))
        .unwrap_or(body)
        .trim();
    if body.len() != 64 || !body.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    Some(body.to_ascii_lowercase())
}

fn zk_hint_present(map: &json::Map, key: &str) -> bool {
    map.get(key)
        .is_some_and(|value| !matches!(value, json::Value::Null))
}

fn parse_u64_value(value: json::Value, context: &str) -> CodecResult<u64> {
    match value {
        json::Value::Number(number) => number.as_u64().ok_or_else(|| {
            CodecError::new(
                CodecErrorKind::InvalidArgument,
                format!("{context} must be an unsigned integer"),
            )
        }),
        json::Value::String(s) => s.parse::<u64>().map_err(|err| {
            CodecError::new(
                CodecErrorKind::InvalidArgument,
                format!("{context} must be an unsigned integer string: {err}"),
            )
        }),
        other => Err(CodecError::new(
            CodecErrorKind::InvalidArgument,
            format!("{context} must be an unsigned integer (found {other:?})"),
        )),
    }
}

fn remove_case_insensitive(map: &mut json::Map, key: &str) -> Option<json::Value> {
    map.remove(key)
        .or_else(|| map.remove(&key.to_ascii_lowercase()))
        .or_else(|| map.remove(&key.to_ascii_uppercase()))
}

fn parse_u8_value(value: json::Value, context: &str) -> CodecResult<u8> {
    let parsed = parse_u64_value(value, context)?;
    u8::try_from(parsed).map_err(|_| {
        CodecError::new(
            CodecErrorKind::InvalidArgument,
            format!("{context} must fit into u8"),
        )
    })
}

fn parse_canonical_quantity_text(source: &str, context: &str) -> CodecResult<Quantity> {
    let quantity = Quantity::from_str(source).map_err(|err| {
        CodecError::new(
            CodecErrorKind::InvalidArgument,
            format!("{context} must be canonical non-negative Quantity text: {err}"),
        )
    })?;
    if quantity.to_string() != source {
        return Err(CodecError::new(
            CodecErrorKind::InvalidArgument,
            format!("{context} must use canonical Quantity text"),
        ));
    }
    Ok(quantity)
}

fn parse_canonical_quantity_value(value: json::Value, context: &str) -> CodecResult<Quantity> {
    let json::Value::String(source) = value else {
        return Err(CodecError::new(
            CodecErrorKind::InvalidArgument,
            format!("{context} must be a canonical Quantity string"),
        ));
    };
    parse_canonical_quantity_text(&source, context)
}

/// Require exactly the named JSON fields, rejecting missing and unknown fields.
pub fn require_exact_json_fields(
    fields: &json::Map,
    expected: &[&str],
    context: &str,
) -> CodecResult<()> {
    let missing = expected
        .iter()
        .copied()
        .filter(|key| !fields.contains_key(*key))
        .collect::<Vec<_>>();
    let unexpected = fields
        .keys()
        .filter(|key| !expected.contains(&key.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    if missing.is_empty() && unexpected.is_empty() {
        return Ok(());
    }
    Err(CodecError::new(
        CodecErrorKind::InvalidArgument,
        format!(
            "{context} must contain exactly [{}]; missing [{}], unexpected [{}]",
            expected.join(", "),
            missing.join(", "),
            unexpected.join(", ")
        ),
    ))
}

/// Parse the complete, strict validation fee policy JSON contract.
pub fn validation_fee_policy_from_json_value(
    value: json::Value,
) -> CodecResult<ValidationFeePolicyV1> {
    const POLICY_FIELDS: &[&str] = &[
        "schema_version",
        "network_id",
        "policy_version",
        "previous_policy_hash",
        "ds_asset_id",
        "ds_scale",
        "fee",
        "treasury_account_id",
        "charging_mode",
        "effective_from_height",
        "expires_after_height",
        "exemption_classes",
        "treasury_payout_binding",
    ];
    const PAYOUT_BINDING_FIELDS: &[&str] = &[
        "contract_address",
        "code_hash",
        "entrypoint",
        "treasury_account_id",
        "ds_asset_id",
        "xor_asset_id",
        "pool_vault_account_id",
        "batch_ds",
        "min_xor_out",
        "max_xor_out",
        "recipients",
    ];
    const RECIPIENT_FIELDS: &[&str] = &["account_id", "share"];
    let json::Value::Object(fields) = &value else {
        return Err(CodecError::new(
            CodecErrorKind::InvalidArgument,
            "validation-fee policy must be an object",
        ));
    };
    require_exact_json_fields(fields, POLICY_FIELDS, "validation-fee policy")?;
    if let Some(binding) = fields.get("treasury_payout_binding")
        && !binding.is_null()
    {
        let json::Value::Object(binding_fields) = binding else {
            return Err(CodecError::new(
                CodecErrorKind::InvalidArgument,
                "validation-fee policy.treasury_payout_binding must be an object or null",
            ));
        };
        require_exact_json_fields(
            binding_fields,
            PAYOUT_BINDING_FIELDS,
            "validation-fee policy.treasury_payout_binding",
        )?;
        let Some(json::Value::Array(recipients)) = binding_fields.get("recipients") else {
            return Err(CodecError::new(
                CodecErrorKind::InvalidArgument,
                "validation-fee policy.treasury_payout_binding.recipients must be an array",
            ));
        };
        for (index, recipient) in recipients.iter().enumerate() {
            let json::Value::Object(recipient_fields) = recipient else {
                return Err(CodecError::new(
                    CodecErrorKind::InvalidArgument,
                    format!(
                        "validation-fee policy.treasury_payout_binding.recipients[{index}] must be an object"
                    ),
                ));
            };
            require_exact_json_fields(
                recipient_fields,
                RECIPIENT_FIELDS,
                &format!("validation-fee policy.treasury_payout_binding.recipients[{index}]"),
            )?;
        }
    }
    json::from_value(value).map_err(codec_error)
}

/// Validate fee policy invariants and the required payout lifecycle binding.
pub fn validate_validation_fee_policy_proposal(
    policy: &ValidationFeePolicyV1,
    payout_lifecycle_proposal_id: Option<&[u8; 32]>,
) -> CodecResult<()> {
    if let Some(reason) = policy.policy_invariant_error() {
        return Err(CodecError::new(
            CodecErrorKind::InvalidArgument,
            format!("invalid validation-fee policy: {reason}"),
        ));
    }
    match (
        policy.treasury_payout_binding.as_ref(),
        payout_lifecycle_proposal_id,
    ) {
        (None, None) => Ok(()),
        (Some(_), Some(id)) if *id != [0; 32] => Ok(()),
        (Some(_), _) => Err(CodecError::new(
            CodecErrorKind::InvalidArgument,
            "payout-enabled validation-fee policy requires a non-zero lifecycle proposal id",
        )),
        (None, Some(_)) => Err(CodecError::new(
            CodecErrorKind::InvalidArgument,
            "validation-fee policy without a payout binding cannot select a lifecycle proposal",
        )),
    }
}

fn validation_fee_policy_instruction_from_json(value: json::Value) -> CodecResult<InstructionBox> {
    const INSTRUCTION_FIELDS: &[&str] = &["policy", "payout_lifecycle_proposal_id"];
    let json::Value::Object(mut fields) = value else {
        return Err(CodecError::new(
            CodecErrorKind::InvalidArgument,
            "ProposeValidationFeePolicy must be an object",
        ));
    };
    require_exact_json_fields(&fields, INSTRUCTION_FIELDS, "ProposeValidationFeePolicy")?;
    let policy = validation_fee_policy_from_json_value(required_value(
        &mut fields,
        "policy",
        "ProposeValidationFeePolicy",
    )?)?;
    let payout_lifecycle_proposal_id = match required_value(
        &mut fields,
        "payout_lifecycle_proposal_id",
        "ProposeValidationFeePolicy",
    )? {
        json::Value::Null => None,
        value => Some(json::from_value::<[u8; 32]>(value).map_err(codec_error)?),
    };
    validate_validation_fee_policy_proposal(&policy, payout_lifecycle_proposal_id.as_ref())?;
    Ok(ProposeValidationFeePolicy {
        policy,
        payout_lifecycle_proposal_id,
    }
    .into())
}

/// Parse instruction JSON through the strict native instruction adapter.
pub fn instruction_from_json(payload: &str) -> CodecResult<InstructionBox> {
    let value: json::Value = json::from_json(payload).map_err(codec_error)?;
    value_to_instruction(value)
}

fn transfer_asset_batch_from_json(value: json::Value) -> CodecResult<InstructionBox> {
    let json::Value::Object(mut fields) = value else {
        return Err(CodecError::new(
            CodecErrorKind::InvalidArgument,
            "TransferAssetBatch payload must be an object",
        ));
    };
    let entries_value = required_value(&mut fields, "entries", "TransferAssetBatch")?;
    if !fields.is_empty() {
        return Err(CodecError::new(
            CodecErrorKind::InvalidArgument,
            format!(
                "TransferAssetBatch contains unexpected field(s): {}",
                fields.keys().cloned().collect::<Vec<_>>().join(", ")
            ),
        ));
    }
    let json::Value::Array(entry_values) = entries_value else {
        return Err(CodecError::new(
            CodecErrorKind::InvalidArgument,
            "TransferAssetBatch.entries must be an array",
        ));
    };
    let mut entries = Vec::with_capacity(entry_values.len());
    for (index, entry_value) in entry_values.into_iter().enumerate() {
        let context = format!("TransferAssetBatch.entries[{index}]");
        let json::Value::Object(mut entry_fields) = entry_value else {
            return Err(CodecError::new(
                CodecErrorKind::InvalidArgument,
                format!("{context} must be an object"),
            ));
        };
        let from = parse_account_id_value(
            required_value(&mut entry_fields, "from", &context)?,
            &format!("{context}.from"),
        )?;
        let to = parse_account_id_value(
            required_value(&mut entry_fields, "to", &context)?,
            &format!("{context}.to"),
        )?;
        let asset_definition_literal = parse_string_value(
            required_value(&mut entry_fields, "asset_definition", &context)?,
            &format!("{context}.asset_definition"),
        )?;
        let asset_definition = AssetDefinitionId::parse_address_literal(&asset_definition_literal)
            .map_err(|err| {
                CodecError::new(
                    CodecErrorKind::InvalidArgument,
                    format!("invalid {context}.asset_definition: {err}"),
                )
            })?;
        let amount: Quantity =
            json::from_value(required_value(&mut entry_fields, "amount", &context)?)
                .map_err(codec_error)?;
        if !entry_fields.is_empty() {
            return Err(CodecError::new(
                CodecErrorKind::InvalidArgument,
                format!(
                    "{context} contains unexpected field(s): {}",
                    entry_fields.keys().cloned().collect::<Vec<_>>().join(", ")
                ),
            ));
        }
        entries.push(iroha_data_model::isi::TransferAssetBatchEntry::new(
            from,
            to,
            asset_definition,
            amount,
        ));
    }
    Ok(InstructionBox::from(TransferAssetBatch::new(entries)))
}

fn validate_governance_selector_payload(
    payload: Option<&json::Value>,
    field: &str,
    context: &str,
) -> CodecResult<()> {
    let Some(json::Value::Object(fields)) = payload else {
        return Ok(());
    };
    let Some(selector) = fields.get(field) else {
        return Ok(());
    };
    let json::Value::String(selector) = selector else {
        return Err(CodecError::new(
            CodecErrorKind::InvalidArgument,
            format!(
                "{context}.{field} must be 1-128 RFC 3986 unreserved ASCII characters and must not start with a dot"
            ),
        ));
    };
    if !iroha_data_model::governance::is_valid_governance_selector_v1(selector) {
        return Err(CodecError::new(
            CodecErrorKind::InvalidArgument,
            format!(
                "{context}.{field} must be 1-128 RFC 3986 unreserved ASCII characters and must not start with a dot"
            ),
        ));
    }
    Ok(())
}

/// Validate the exact selector fields of governance instruction payloads.
pub fn validate_governance_instruction_selectors(value: &json::Value) -> CodecResult<()> {
    let json::Value::Object(instruction) = value else {
        return Ok(());
    };
    for (variant, field) in [
        ("CastZkBallot", "election_id"),
        ("CastPlainBallot", "referendum_id"),
    ] {
        validate_governance_selector_payload(instruction.get(variant), field, variant)?;
    }
    for zk_key in ["zk", "Zk", "ZK"] {
        let Some(json::Value::Object(zk)) = instruction.get(zk_key) else {
            continue;
        };
        for variant in ["CreateElection", "SubmitBallot", "FinalizeElection"] {
            validate_governance_selector_payload(
                zk.get(variant),
                "election_id",
                &format!("{zk_key}.{variant}"),
            )?;
        }
    }
    Ok(())
}

// One explicit bidirectional catalog owns the browser families with native JSON
// derives. Do not fall back to InstructionBox JSON: that owner emits base64,
// which loses the structured intent inspected by browser signing consumers.
macro_rules! typed_browser_instruction_catalog {
    ($apply:ident) => {
        $apply! {
            OpenGameSessionV1 => iroha_data_model::isi::game::OpenGameSessionV1,
            JoinGameSessionV1 => iroha_data_model::isi::game::JoinGameSessionV1,
            StartGameSessionV1 => iroha_data_model::isi::game::StartGameSessionV1,
            CommitGameCheckpointV1 => iroha_data_model::isi::game::CommitGameCheckpointV1,
            ChallengeGameSessionV1 => iroha_data_model::isi::game::ChallengeGameSessionV1,
            CommitGameInputsV1 => iroha_data_model::isi::game::CommitGameInputsV1,
            RevealGameInputsV1 => iroha_data_model::isi::game::RevealGameInputsV1,
            AdvanceGameDeadlineV1 => iroha_data_model::isi::game::AdvanceGameDeadlineV1,
            SettleGameSessionV1 => iroha_data_model::isi::game::SettleGameSessionV1,
            ExpireGameSessionV1 => iroha_data_model::isi::game::ExpireGameSessionV1,
            ClaimGamePayoutV1 => iroha_data_model::isi::game::ClaimGamePayoutV1,
            StakeGameItemV1 => iroha_data_model::isi::game::StakeGameItemV1,
            RegisterExecutionProofProfileV1 => iroha_data_model::isi::game::RegisterExecutionProofProfileV1,
            VerifyExecutionProofV1 => iroha_data_model::isi::game::VerifyExecutionProofV1,
            OfferNftV1 => iroha_data_model::isi::nft_market::OfferNftV1,
            BuyNftV1 => iroha_data_model::isi::nft_market::BuyNftV1,
            CancelNftOfferV1 => iroha_data_model::isi::nft_market::CancelNftOfferV1,
            TopUpKagemushaV1 => iroha_data_model::isi::kagemusha_v1::TopUpKagemushaV1,
        }
    };
}

fn instruction_envelope(name: &str, payload: json::Value) -> json::Value {
    let mut outer = json::Map::new();
    outer.insert(name.to_owned(), payload);
    json::Value::Object(outer)
}

fn strict_typed_instruction<T>(payload: &json::Value, name: &str) -> CodecResult<T>
where
    T: json::JsonDeserialize + json::JsonSerialize,
{
    let instruction: T = json::from_value(payload.clone()).map_err(|error| {
        CodecError::new(CodecErrorKind::InvalidArgument, format!("{name}: {error}"))
    })?;
    if json::to_value(&instruction).map_err(codec_error)? != *payload {
        return Err(CodecError::new(
            CodecErrorKind::InvalidArgument,
            format!("{name} must contain exactly its canonical typed JSON fields and values"),
        ));
    }
    Ok(instruction)
}

macro_rules! define_typed_browser_instruction_json {
    ($($name:ident => $ty:ty,)*) => {
        fn typed_browser_instruction_from_json(
            value: &json::Value,
        ) -> Option<CodecResult<InstructionBox>> {
            let json::Value::Object(fields) = value else { return None; };
            $(if let Some(payload) = fields.get(stringify!($name)) {
                return Some((|| {
                    exact_json_object_fields(value, &[stringify!($name)], "instruction envelope")?;
                    let instruction: $ty = strict_typed_instruction(payload, stringify!($name))?;
                    Ok(Box::new(instruction).into_instruction_box())
                })());
            })*
            None
        }

        fn typed_browser_instruction_to_json(
            instruction: &InstructionBox,
        ) -> Option<CodecResult<json::Value>> {
            let instruction_ref: &dyn InstructionTrait = &**instruction;
            $(if let Some(typed) = instruction_ref.as_any().downcast_ref::<$ty>() {
                return Some((|| {
                    let payload = json::to_value(typed).map_err(codec_error)?;
                    let reconstructed: $ty = strict_typed_instruction(&payload, stringify!($name))?;
                    if norito::encode_canonical(typed).map_err(codec_error)?
                        != norito::encode_canonical(&reconstructed).map_err(codec_error)?
                    {
                        return Err(CodecError::failure("typed instruction JSON changes canonical Norito bytes"));
                    }
                    Ok(instruction_envelope(stringify!($name), payload))
                })());
            })*
            None
        }
    };
}
typed_browser_instruction_catalog!(define_typed_browser_instruction_json);

fn deployment_instruction_from_json(value: &json::Value) -> Option<CodecResult<InstructionBox>> {
    let json::Value::Object(outer) = value else {
        return None;
    };
    let name = [
        "UploadSmartContractCodeChunk",
        "FinalizeSmartContractCodeUpload",
        "CancelSmartContractCodeUpload",
        "RegisterSmartContractCode",
        "CommitContractDeployment",
    ]
    .into_iter()
    .find(|name| outer.contains_key(*name))?;
    Some((|| {
        exact_json_object_fields(value, &[name], "instruction envelope")?;
        let payload = &outer[name];
        let expected: &[&str] = match name {
            "UploadSmartContractCodeChunk" => &[
                "code_hash",
                "total_size",
                "chunk_index",
                "chunk_count",
                "chunk",
            ],
            "FinalizeSmartContractCodeUpload" => &["code_hash", "total_size", "chunk_count"],
            "CancelSmartContractCodeUpload" => &["code_hash"],
            "RegisterSmartContractCode" => &["manifest"],
            "CommitContractDeployment" => &[
                "expected_deploy_nonce",
                "contract_address",
                "code_hash",
                "contract_alias",
                "lease_expiry_ms",
                "expected_previous_contract_address",
            ],
            _ => unreachable!("explicit deployment catalog"),
        };
        exact_json_object_fields(payload, expected, name)?;
        let field = |key: &str| payload.get(key).expect("exact fields checked").clone();
        if name == "RegisterSmartContractCode" {
            let manifest = json::from_value(field("manifest")).map_err(codec_error)?;
            return Ok(Box::new(RegisterSmartContractCode { manifest }).into_instruction_box());
        }
        let code_hash = parse_hash_value(field("code_hash"), &format!("{name}.code_hash"))?;
        let unsigned = |key: &str| {
            let value = field(key);
            let parsed = parse_u64_value(value.clone(), &format!("{name}.{key}"))?;
            if matches!(&value, json::Value::String(literal) if *literal != parsed.to_string()) {
                return Err(CodecError::new(
                    CodecErrorKind::InvalidArgument,
                    format!("{name}.{key} must be a canonical unsigned integer"),
                ));
            }
            Ok(parsed)
        };
        let u32_field = |key: &str| {
            u32::try_from(unsigned(key)?).map_err(|_| {
                CodecError::new(
                    CodecErrorKind::InvalidArgument,
                    format!("{name}.{key} must fit u32"),
                )
            })
        };
        Ok(match name {
            "UploadSmartContractCodeChunk" => Box::new(UploadSmartContractCodeChunk {
                code_hash,
                total_size: unsigned("total_size")?,
                chunk_index: u32_field("chunk_index")?,
                chunk_count: u32_field("chunk_count")?,
                chunk: parse_base64(field("chunk"), "UploadSmartContractCodeChunk.chunk")?,
            })
            .into_instruction_box(),
            "FinalizeSmartContractCodeUpload" => Box::new(FinalizeSmartContractCodeUpload {
                code_hash,
                total_size: unsigned("total_size")?,
                chunk_count: u32_field("chunk_count")?,
            })
            .into_instruction_box(),
            "CancelSmartContractCodeUpload" => {
                Box::new(CancelSmartContractCodeUpload { code_hash }).into_instruction_box()
            }
            "CommitContractDeployment" => Box::new(CommitContractDeployment {
                expected_deploy_nonce: unsigned("expected_deploy_nonce")?,
                contract_address: json::from_value(field("contract_address"))
                    .map_err(codec_error)?,
                code_hash,
                contract_alias: json::from_value(field("contract_alias")).map_err(codec_error)?,
                lease_expiry_ms: if field("lease_expiry_ms").is_null() {
                    None
                } else {
                    Some(unsigned("lease_expiry_ms")?)
                },
                expected_previous_contract_address: json::from_value(field(
                    "expected_previous_contract_address",
                ))
                .map_err(codec_error)?,
            })
            .into_instruction_box(),
            _ => unreachable!("explicit deployment catalog"),
        })
    })())
}

/// Admit a JSON instruction value with the existing explicit variant checks.
pub fn value_to_instruction(value: json::Value) -> CodecResult<InstructionBox> {
    if let Some(result) = typed_browser_instruction_from_json(&value)
        .or_else(|| deployment_instruction_from_json(&value))
    {
        return result;
    }
    validate_governance_instruction_selectors(&value)?;
    // These instructions carry release-critical JSON contracts. The generic
    // `InstructionBox` decoder may accept data-model defaults and unknown
    // fields, so route these envelopes through explicit strict parsers.
    let requires_explicit_parser = matches!(
        &value,
        json::Value::Object(map)
            if map.contains_key("Register")
                || map.contains_key("Settlement")
                || map.contains_key("CancelAssetLock")
                || map.contains_key("SetAssetTransferAvailability")
                || map.contains_key("SetAssetTransferBlacklist")
                || map.contains_key("SetAssetTransferControl")
                || map.contains_key("ProposeValidationFeePolicy")
    );
    if !requires_explicit_parser {
        if let Ok(instruction) = json::from_value::<InstructionBox>(value.clone()) {
            return Ok(instruction);
        }
    }
    match value {
        json::Value::Object(mut map) => {
            if let Some(payload) = map.remove("SetAssetTransferAvailability") {
                if !map.is_empty() {
                    return Err(CodecError::new(
                        CodecErrorKind::InvalidArgument,
                        format!(
                            "SetAssetTransferAvailability instruction envelope contains unexpected field(s): {}",
                            map.keys().cloned().collect::<Vec<_>>().join(", ")
                        ),
                    ));
                }
                let json::Value::Object(mut fields) = payload else {
                    return Err(CodecError::new(
                        CodecErrorKind::InvalidArgument,
                        "SetAssetTransferAvailability must be an object",
                    ));
                };
                let account_id = parse_account_id_value(
                    required_value(&mut fields, "account_id", "SetAssetTransferAvailability")?,
                    "SetAssetTransferAvailability.account_id",
                )?;
                let asset_definition_literal = parse_string_value(
                    required_value(
                        &mut fields,
                        "asset_definition_id",
                        "SetAssetTransferAvailability",
                    )?,
                    "SetAssetTransferAvailability.asset_definition_id",
                )?;
                let asset_definition_id = AssetDefinitionId::parse_address_literal(
                    &asset_definition_literal,
                )
                .map_err(|error| {
                    CodecError::new(
                        CodecErrorKind::InvalidArgument,
                        format!(
                            "invalid SetAssetTransferAvailability.asset_definition_id: {error}"
                        ),
                    )
                })?;
                let expected_revision = parse_u64_value(
                    required_value(
                        &mut fields,
                        "expected_revision",
                        "SetAssetTransferAvailability",
                    )?,
                    "SetAssetTransferAvailability.expected_revision",
                )?;
                let parse_availability =
                    |value: json::Value, context: &str| -> CodecResult<AssetTransferAvailability> {
                        match value {
                            json::Value::String(value) if value == "Enabled" => {
                                Ok(AssetTransferAvailability::Enabled)
                            }
                            json::Value::String(value) if value == "Disabled" => {
                                Ok(AssetTransferAvailability::Disabled)
                            }
                            other => Err(CodecError::new(
                                CodecErrorKind::InvalidArgument,
                                format!(
                                    "{context} must be exactly \"Enabled\" or \"Disabled\" (found {other:?})"
                                ),
                            )),
                        }
                    };
                let incoming = parse_availability(
                    required_value(&mut fields, "incoming", "SetAssetTransferAvailability")?,
                    "SetAssetTransferAvailability.incoming",
                )?;
                let outgoing = parse_availability(
                    required_value(&mut fields, "outgoing", "SetAssetTransferAvailability")?,
                    "SetAssetTransferAvailability.outgoing",
                )?;
                let reason = match fields.remove("reason") {
                    None | Some(json::Value::Null) => None,
                    Some(json::Value::String(value))
                        if !value.is_empty() && value.trim() == value =>
                    {
                        Some(value)
                    }
                    Some(other) => {
                        return Err(CodecError::new(
                            CodecErrorKind::InvalidArgument,
                            format!(
                                "SetAssetTransferAvailability.reason must be non-empty exact text or null (found {other:?})"
                            ),
                        ));
                    }
                };
                validate_asset_transfer_availability_reason(reason.as_deref()).map_err(
                    |error| CodecError::new(CodecErrorKind::InvalidArgument, error.to_string()),
                )?;
                if !fields.is_empty() {
                    return Err(CodecError::new(
                        CodecErrorKind::InvalidArgument,
                        format!(
                            "SetAssetTransferAvailability contains unexpected field(s): {}",
                            fields.keys().cloned().collect::<Vec<_>>().join(", ")
                        ),
                    ));
                }
                return Ok(SetAssetTransferAvailability::new(
                    account_id,
                    asset_definition_id,
                    expected_revision,
                    incoming,
                    outgoing,
                    reason,
                )
                .into());
            }
            if let Some(payload) = map.remove("SetAssetTransferBlacklist") {
                if !map.is_empty() {
                    return Err(CodecError::new(
                        CodecErrorKind::InvalidArgument,
                        format!(
                            "SetAssetTransferBlacklist instruction envelope contains unexpected field(s): {}",
                            map.keys().cloned().collect::<Vec<_>>().join(", ")
                        ),
                    ));
                }
                let json::Value::Object(mut fields) = payload else {
                    return Err(CodecError::new(
                        CodecErrorKind::InvalidArgument,
                        "SetAssetTransferBlacklist must be an object",
                    ));
                };
                require_exact_json_fields(
                    &fields,
                    &["account_id", "asset_definition_id", "blacklisted"],
                    "SetAssetTransferBlacklist",
                )?;
                let account_id = parse_account_id_value(
                    required_value(&mut fields, "account_id", "SetAssetTransferBlacklist")?,
                    "SetAssetTransferBlacklist.account_id",
                )?;
                let asset_definition_literal = parse_string_value(
                    required_value(
                        &mut fields,
                        "asset_definition_id",
                        "SetAssetTransferBlacklist",
                    )?,
                    "SetAssetTransferBlacklist.asset_definition_id",
                )?;
                let asset_definition_id = AssetDefinitionId::parse_address_literal(
                    &asset_definition_literal,
                )
                .map_err(|error| {
                    CodecError::new(
                        CodecErrorKind::InvalidArgument,
                        format!("invalid SetAssetTransferBlacklist.asset_definition_id: {error}"),
                    )
                })?;
                let blacklisted = match required_value(
                    &mut fields,
                    "blacklisted",
                    "SetAssetTransferBlacklist",
                )? {
                    json::Value::Bool(value) => value,
                    other => {
                        return Err(CodecError::new(
                            CodecErrorKind::InvalidArgument,
                            format!(
                                "SetAssetTransferBlacklist.blacklisted must be a boolean (found {other:?})"
                            ),
                        ));
                    }
                };
                return Ok(SetAssetTransferBlacklist::new(
                    account_id,
                    asset_definition_id,
                    blacklisted,
                )
                .into());
            }
            if let Some(payload) = map.remove("SetAssetTransferControl") {
                if !map.is_empty() {
                    return Err(CodecError::new(
                        CodecErrorKind::InvalidArgument,
                        format!(
                            "SetAssetTransferControl instruction envelope contains unexpected field(s): {}",
                            map.keys().cloned().collect::<Vec<_>>().join(", ")
                        ),
                    ));
                }
                let json::Value::Object(mut fields) = payload else {
                    return Err(CodecError::new(
                        CodecErrorKind::InvalidArgument,
                        "SetAssetTransferControl must be an object",
                    ));
                };
                require_exact_json_fields(
                    &fields,
                    &["account_id", "asset_definition_id", "limits"],
                    "SetAssetTransferControl",
                )?;
                let account_id = parse_account_id_value(
                    required_value(&mut fields, "account_id", "SetAssetTransferControl")?,
                    "SetAssetTransferControl.account_id",
                )?;
                let asset_definition_literal = parse_string_value(
                    required_value(
                        &mut fields,
                        "asset_definition_id",
                        "SetAssetTransferControl",
                    )?,
                    "SetAssetTransferControl.asset_definition_id",
                )?;
                let asset_definition_id = AssetDefinitionId::parse_address_literal(
                    &asset_definition_literal,
                )
                .map_err(|error| {
                    CodecError::new(
                        CodecErrorKind::InvalidArgument,
                        format!("invalid SetAssetTransferControl.asset_definition_id: {error}"),
                    )
                })?;
                let limits_value =
                    required_value(&mut fields, "limits", "SetAssetTransferControl")?;
                let json::Value::Array(limit_values) = limits_value else {
                    return Err(CodecError::new(
                        CodecErrorKind::InvalidArgument,
                        "SetAssetTransferControl.limits must be an array",
                    ));
                };
                let mut limits = Vec::with_capacity(limit_values.len());
                for (index, value) in limit_values.into_iter().enumerate() {
                    let context = format!("SetAssetTransferControl.limits[{index}]");
                    let json::Value::Object(mut limit_fields) = value else {
                        return Err(CodecError::new(
                            CodecErrorKind::InvalidArgument,
                            format!("{context} must be an object"),
                        ));
                    };
                    require_exact_json_fields(&limit_fields, &["window", "cap_amount"], &context)?;
                    let window = match required_value(&mut limit_fields, "window", &context)? {
                        json::Value::String(value) if value == "Day" => {
                            AssetTransferControlWindow::Day
                        }
                        json::Value::String(value) if value == "Week" => {
                            AssetTransferControlWindow::Week
                        }
                        json::Value::String(value) if value == "Month" => {
                            AssetTransferControlWindow::Month
                        }
                        other => {
                            return Err(CodecError::new(
                                CodecErrorKind::InvalidArgument,
                                format!(
                                    "{context}.window must be exactly \"Day\", \"Week\", or \"Month\" (found {other:?})"
                                ),
                            ));
                        }
                    };
                    if limits
                        .iter()
                        .any(|limit: &AssetTransferLimit| limit.window == window)
                    {
                        return Err(CodecError::new(
                            CodecErrorKind::InvalidArgument,
                            format!("{context}.window duplicates {window}"),
                        ));
                    }
                    let cap_amount =
                        match required_value(&mut limit_fields, "cap_amount", &context)? {
                            json::Value::Null => None,
                            value => Some(parse_canonical_quantity_value(
                                value,
                                &format!("{context}.cap_amount"),
                            )?),
                        };
                    limits.push(AssetTransferLimit { window, cap_amount });
                }
                return Ok(
                    SetAssetTransferControl::new(account_id, asset_definition_id, limits).into(),
                );
            }
            if let Some(payload) = map.remove("DeploySoracloudService") {
                if !map.is_empty() {
                    return Err(CodecError::new(
                        CodecErrorKind::InvalidArgument,
                        format!(
                            "DeploySoracloudService instruction envelope contains unexpected field(s): {}",
                            map.keys().cloned().collect::<Vec<_>>().join(", ")
                        ),
                    ));
                }
                let instruction: iroha_data_model::isi::soracloud::DeploySoracloudService =
                    json::from_value(payload).map_err(codec_error)?;
                return Ok(instruction.into());
            }
            if let Some(payload) = map.remove("DeploySoracloudAgentApartment") {
                if !map.is_empty() {
                    return Err(CodecError::new(
                        CodecErrorKind::InvalidArgument,
                        format!(
                            "DeploySoracloudAgentApartment instruction envelope contains unexpected field(s): {}",
                            map.keys().cloned().collect::<Vec<_>>().join(", ")
                        ),
                    ));
                }
                let instruction: iroha_data_model::isi::soracloud::DeploySoracloudAgentApartment =
                    json::from_value(payload).map_err(codec_error)?;
                return Ok(instruction.into());
            }
            if let Some(payload) = map.remove("JoinSoracloudHfSharedLease") {
                if !map.is_empty() {
                    return Err(CodecError::new(
                        CodecErrorKind::InvalidArgument,
                        format!(
                            "JoinSoracloudHfSharedLease instruction envelope contains unexpected field(s): {}",
                            map.keys().cloned().collect::<Vec<_>>().join(", ")
                        ),
                    ));
                }
                let instruction: iroha_data_model::isi::soracloud::JoinSoracloudHfSharedLease =
                    json::from_value(payload).map_err(codec_error)?;
                return Ok(instruction.into());
            }
            if let Some(proposal_value) = map.remove("ProposeValidationFeePolicy") {
                if !map.is_empty() {
                    return Err(CodecError::new(
                        CodecErrorKind::InvalidArgument,
                        format!(
                            "ProposeValidationFeePolicy instruction envelope contains unexpected field(s): {}",
                            map.keys().cloned().collect::<Vec<_>>().join(", ")
                        ),
                    ));
                }
                return validation_fee_policy_instruction_from_json(proposal_value);
            }
            if let Some(settlement_value) = map.remove("Settlement") {
                if !map.is_empty() {
                    return Err(CodecError::new(
                        CodecErrorKind::InvalidArgument,
                        format!(
                            "Settlement instruction envelope contains unexpected field(s): {}",
                            map.keys().cloned().collect::<Vec<_>>().join(", ")
                        ),
                    ));
                }
                return settlement_instruction_from_json(settlement_value);
            }
            if let Some(batch_value) = map.remove("TransferAssetBatch") {
                return transfer_asset_batch_from_json(batch_value);
            }
            if let Some(cancel_value) = map.remove("CancelAssetLock") {
                if !map.is_empty() {
                    return Err(CodecError::new(
                        CodecErrorKind::InvalidArgument,
                        format!(
                            "CancelAssetLock instruction envelope contains unexpected field(s): {}",
                            map.keys().cloned().collect::<Vec<_>>().join(", ")
                        ),
                    ));
                }
                exact_json_object_fields(
                    &cancel_value,
                    &["escrow_id", "expected_remaining_amount"],
                    "CancelAssetLock",
                )?;
                let json::Value::Object(mut fields) = cancel_value else {
                    unreachable!("exact_json_object_fields accepted an object");
                };
                let escrow_id = EscrowId::new(parse_canonical_hash_value(
                    required_value(&mut fields, "escrow_id", "CancelAssetLock")?,
                    "CancelAssetLock.escrow_id",
                )?);
                let expected_remaining_amount = parse_canonical_quantity_value(
                    required_value(&mut fields, "expected_remaining_amount", "CancelAssetLock")?,
                    "CancelAssetLock.expected_remaining_amount",
                )?;
                if expected_remaining_amount.is_zero() {
                    return Err(CodecError::new(
                        CodecErrorKind::InvalidArgument,
                        "CancelAssetLock.expected_remaining_amount must be positive",
                    ));
                }
                return Ok(CancelAssetLock::new(escrow_id, expected_remaining_amount).into());
            }
            if let Some(register_value) = map.remove("Register") {
                if !map.is_empty() {
                    return Err(CodecError::new(
                        CodecErrorKind::InvalidArgument,
                        format!(
                            "Register instruction envelope contains unexpected field(s): {}",
                            map.keys().cloned().collect::<Vec<_>>().join(", ")
                        ),
                    ));
                }
                let json::Value::Object(mut register_map) = register_value else {
                    return Err(CodecError::new(
                        CodecErrorKind::InvalidArgument,
                        "Register instruction must be an object containing exactly one variant",
                    ));
                };
                if register_map.len() != 1 {
                    return Err(CodecError::new(
                        CodecErrorKind::InvalidArgument,
                        "Register instruction must contain exactly one variant",
                    ));
                }
                if let Some(domain_value) = register_map.remove("Domain") {
                    let new_domain: NewDomain =
                        json::from_value(domain_value).map_err(codec_error)?;
                    let register_box = RegisterBox::Domain(Register::<Domain>::domain(new_domain));
                    return Ok(InstructionBox::from(register_box));
                }
                if let Some(account_value) = register_map.remove("Account") {
                    let new_account: NewAccount =
                        json::from_value(account_value).map_err(codec_error)?;
                    let register_box =
                        RegisterBox::Account(Register::<Account>::account(new_account));
                    return Ok(InstructionBox::from(register_box));
                }
                if let Some(asset_value) = register_map.remove("AssetDefinition") {
                    let new_asset: NewAssetDefinition =
                        json::from_value(asset_value).map_err(codec_error)?;
                    let register_box = RegisterBox::AssetDefinition(
                        Register::<AssetDefinition>::asset_definition(new_asset),
                    );
                    return Ok(InstructionBox::from(register_box));
                }
                if let Some(nft_value) = register_map.remove("Nft") {
                    let new_nft: NewNft = json::from_value(nft_value).map_err(codec_error)?;
                    let register_box = RegisterBox::Nft(Register::<Nft>::nft(new_nft));
                    return Ok(InstructionBox::from(register_box));
                }
                if let Some(role_value) = register_map.remove("Role") {
                    let new_role: NewRole = json::from_value(role_value).map_err(codec_error)?;
                    let register_box = RegisterBox::Role(Register::<Role>::role(new_role));
                    return Ok(InstructionBox::from(register_box));
                }
                if let Some(trigger_value) = register_map.remove("Trigger") {
                    let json::Value::Object(mut trigger_fields) = trigger_value else {
                        return Err(CodecError::new(
                            CodecErrorKind::InvalidArgument,
                            "Register.Trigger must be an object with exact id and action fields",
                        ));
                    };
                    let trigger_id: TriggerId = json::from_value(required_value(
                        &mut trigger_fields,
                        "id",
                        "Register.Trigger",
                    )?)
                    .map_err(codec_error)?;
                    let action: Action = json::from_value(required_value(
                        &mut trigger_fields,
                        "action",
                        "Register.Trigger",
                    )?)
                    .map_err(codec_error)?;
                    if !trigger_fields.is_empty() {
                        return Err(CodecError::new(
                            CodecErrorKind::InvalidArgument,
                            format!(
                                "Register.Trigger contains unexpected field(s): {}",
                                trigger_fields
                                    .keys()
                                    .cloned()
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            ),
                        ));
                    }
                    let trigger = Trigger::new(trigger_id, action);
                    let register_box = RegisterBox::Trigger(Register::<Trigger>::trigger(trigger));
                    return Ok(InstructionBox::from(register_box));
                }
                if let Some(peer_value) = register_map.remove("Peer") {
                    let peer_registration: RegisterPeerWithPop =
                        json::from_value(peer_value).map_err(codec_error)?;
                    let register_box = RegisterBox::Peer(peer_registration);
                    return Ok(InstructionBox::from(register_box));
                }
                return Err(CodecError::new(
                    CodecErrorKind::InvalidArgument,
                    "unsupported Register instruction variant",
                ));
            }
            if let Some(parameter_value) = remove_case_insensitive(&mut map, "SetParameter") {
                let parameter = json::from_value::<Parameter>(parameter_value.clone())
                    .or_else(|_| {
                        json::from_value::<CustomParameter>(parameter_value).map(Parameter::Custom)
                    })
                    .map_err(codec_error)?;
                return Ok(InstructionBox::from(SetParameter::new(parameter)));
            }
            if let Some(json::Value::Object(mut mint_map)) = map.remove("Mint") {
                if let Some(json::Value::Object(mut asset_fields)) = mint_map.remove("Asset") {
                    let quantity_value = asset_fields.remove("object").ok_or_else(|| {
                        CodecError::new(
                            CodecErrorKind::InvalidArgument,
                            "Mint.Asset.object field missing",
                        )
                    })?;
                    let destination_value =
                        asset_fields.remove("destination").ok_or_else(|| {
                            CodecError::new(
                                CodecErrorKind::InvalidArgument,
                                "Mint.Asset.destination field missing",
                            )
                        })?;
                    let quantity: Quantity =
                        json::from_value(quantity_value).map_err(codec_error)?;
                    let destination: AssetId =
                        json::from_value(destination_value).map_err(codec_error)?;
                    let mint = Mint::asset_quantity(quantity, destination);
                    return Ok(InstructionBox::from(MintBox::Asset(mint)));
                }
                if let Some(json::Value::Object(mut trigger_fields)) =
                    mint_map.remove("TriggerRepetitions")
                {
                    let repetitions_value = trigger_fields.remove("object").ok_or_else(|| {
                        CodecError::new(
                            CodecErrorKind::InvalidArgument,
                            "Mint.TriggerRepetitions.object field missing",
                        )
                    })?;
                    let destination_value =
                        trigger_fields.remove("destination").ok_or_else(|| {
                            CodecError::new(
                                CodecErrorKind::InvalidArgument,
                                "Mint.TriggerRepetitions.destination field missing",
                            )
                        })?;
                    let repetitions: u32 =
                        json::from_value(repetitions_value).map_err(codec_error)?;
                    let trigger_id: TriggerId =
                        json::from_value(destination_value).map_err(codec_error)?;
                    let mint = Mint::trigger_repetitions(repetitions, trigger_id);
                    return Ok(InstructionBox::from(MintBox::TriggerRepetitions(mint)));
                }
                return Err(CodecError::new(
                    CodecErrorKind::InvalidArgument,
                    "unsupported Mint instruction variant; expected keys: Asset or TriggerRepetitions",
                ));
            }
            if let Some(json::Value::Object(mut unregister_map)) = map.remove("Unregister") {
                if let Some(peer_value) = unregister_map.remove("Peer") {
                    let peer_id: PeerId = json::from_value(peer_value).map_err(codec_error)?;
                    let unregister_box = UnregisterBox::Peer(Unregister::<Peer>::peer(peer_id));
                    return Ok(InstructionBox::from(unregister_box));
                }
                if let Some(domain_value) = unregister_map.remove("Domain") {
                    let domain_id: DomainId =
                        json::from_value(domain_value).map_err(codec_error)?;
                    let unregister_box =
                        UnregisterBox::Domain(Unregister::<Domain>::domain(domain_id));
                    return Ok(InstructionBox::from(unregister_box));
                }
                if let Some(account_value) = unregister_map.remove("Account") {
                    let account_id = parse_account_id_value(account_value, "Unregister.Account")?;
                    let unregister_box =
                        UnregisterBox::Account(Unregister::<Account>::account(account_id));
                    return Ok(InstructionBox::from(unregister_box));
                }
                if let Some(asset_value) = unregister_map.remove("AssetDefinition") {
                    let definition_id: AssetDefinitionId =
                        json::from_value(asset_value).map_err(codec_error)?;
                    let unregister_box =
                        UnregisterBox::AssetDefinition(
                            Unregister::<AssetDefinition>::asset_definition(definition_id),
                        );
                    return Ok(InstructionBox::from(unregister_box));
                }
                if let Some(nft_value) = unregister_map.remove("Nft") {
                    let nft_id: NftId = json::from_value(nft_value).map_err(codec_error)?;
                    let unregister_box = UnregisterBox::Nft(Unregister::<Nft>::nft(nft_id));
                    return Ok(InstructionBox::from(unregister_box));
                }
                if let Some(role_value) = unregister_map.remove("Role") {
                    let role_id: RoleId = json::from_value(role_value).map_err(codec_error)?;
                    let unregister_box = UnregisterBox::Role(Unregister::<Role>::role(role_id));
                    return Ok(InstructionBox::from(unregister_box));
                }
                if let Some(trigger_value) = unregister_map.remove("Trigger") {
                    let trigger_id: TriggerId =
                        json::from_value(trigger_value).map_err(codec_error)?;
                    let unregister_box =
                        UnregisterBox::Trigger(Unregister::<Trigger>::trigger(trigger_id));
                    return Ok(InstructionBox::from(unregister_box));
                }
                return Err(CodecError::new(
                    CodecErrorKind::InvalidArgument,
                    "unsupported Unregister instruction variant; expected keys: Peer, Domain, Account, AssetDefinition, Nft, Role, Trigger",
                ));
            }
            if let Some(json::Value::Object(mut burn_map)) = map.remove("Burn") {
                if let Some(json::Value::Object(mut asset_fields)) = burn_map.remove("Asset") {
                    let quantity_value = asset_fields.remove("object").ok_or_else(|| {
                        CodecError::new(
                            CodecErrorKind::InvalidArgument,
                            "Burn.Asset.object field missing",
                        )
                    })?;
                    let destination_value =
                        asset_fields.remove("destination").ok_or_else(|| {
                            CodecError::new(
                                CodecErrorKind::InvalidArgument,
                                "Burn.Asset.destination field missing",
                            )
                        })?;
                    let quantity: Quantity =
                        json::from_value(quantity_value).map_err(codec_error)?;
                    let asset_id: AssetId =
                        json::from_value(destination_value).map_err(codec_error)?;
                    let burn = Burn::asset_quantity(quantity, asset_id);
                    return Ok(InstructionBox::from(BurnBox::Asset(burn)));
                }
                if let Some(json::Value::Object(mut trigger_fields)) =
                    burn_map.remove("TriggerRepetitions")
                {
                    let repetitions_value = trigger_fields.remove("object").ok_or_else(|| {
                        CodecError::new(
                            CodecErrorKind::InvalidArgument,
                            "Burn.TriggerRepetitions.object field missing",
                        )
                    })?;
                    let destination_value =
                        trigger_fields.remove("destination").ok_or_else(|| {
                            CodecError::new(
                                CodecErrorKind::InvalidArgument,
                                "Burn.TriggerRepetitions.destination field missing",
                            )
                        })?;
                    let repetitions: u32 =
                        json::from_value(repetitions_value).map_err(codec_error)?;
                    let trigger_id: TriggerId =
                        json::from_value(destination_value).map_err(codec_error)?;
                    let burn = Burn::trigger_repetitions(repetitions, trigger_id);
                    return Ok(InstructionBox::from(BurnBox::TriggerRepetitions(burn)));
                }
                return Err(CodecError::new(
                    CodecErrorKind::InvalidArgument,
                    "unsupported Burn instruction variant; expected keys: Asset or TriggerRepetitions",
                ));
            }
            if let Some(json::Value::Object(mut execute_fields)) = map.remove("ExecuteTrigger") {
                let trigger: TriggerId = json::from_value(required_value(
                    &mut execute_fields,
                    "trigger",
                    "ExecuteTrigger",
                )?)
                .map_err(codec_error)?;
                let args = execute_fields
                    .remove("args")
                    .map(Json::from)
                    .unwrap_or_default();
                return Ok(InstructionBox::from(ExecuteTrigger { trigger, args }));
            }
            if let Some(json::Value::Object(mut transfer_map)) = map.remove("Transfer") {
                if let Some(json::Value::Object(mut asset_fields)) = transfer_map.remove("Asset") {
                    let source_value = asset_fields.remove("source").ok_or_else(|| {
                        CodecError::new(
                            CodecErrorKind::InvalidArgument,
                            "Transfer.Asset.source field missing",
                        )
                    })?;
                    let quantity_value = asset_fields.remove("object").ok_or_else(|| {
                        CodecError::new(
                            CodecErrorKind::InvalidArgument,
                            "Transfer.Asset.object field missing",
                        )
                    })?;
                    let destination_value =
                        asset_fields.remove("destination").ok_or_else(|| {
                            CodecError::new(
                                CodecErrorKind::InvalidArgument,
                                "Transfer.Asset.destination field missing",
                            )
                        })?;
                    let source: AssetId = json::from_value(source_value).map_err(codec_error)?;
                    let quantity: Quantity =
                        json::from_value(quantity_value).map_err(codec_error)?;
                    let destination =
                        parse_account_id_value(destination_value, "Transfer.Asset.destination")?;
                    let transfer = Transfer::asset_quantity(source, quantity, destination);
                    return Ok(InstructionBox::from(TransferBox::Asset(transfer)));
                }
                if let Some(json::Value::Object(mut domain_fields)) = transfer_map.remove("Domain")
                {
                    let source_value = domain_fields.remove("source").ok_or_else(|| {
                        CodecError::new(
                            CodecErrorKind::InvalidArgument,
                            "Transfer.Domain.source field missing",
                        )
                    })?;
                    let object_value = domain_fields.remove("object").ok_or_else(|| {
                        CodecError::new(
                            CodecErrorKind::InvalidArgument,
                            "Transfer.Domain.object field missing",
                        )
                    })?;
                    let destination_value =
                        domain_fields.remove("destination").ok_or_else(|| {
                            CodecError::new(
                                CodecErrorKind::InvalidArgument,
                                "Transfer.Domain.destination field missing",
                            )
                        })?;
                    let source = parse_account_id_value(source_value, "Transfer.Domain.source")?;
                    let domain_id: DomainId =
                        json::from_value(object_value).map_err(codec_error)?;
                    let destination =
                        parse_account_id_value(destination_value, "Transfer.Domain.destination")?;
                    let transfer = Transfer::domain(source, domain_id, destination);
                    return Ok(InstructionBox::from(TransferBox::Domain(transfer)));
                }
                if let Some(json::Value::Object(mut definition_fields)) =
                    transfer_map.remove("AssetDefinition")
                {
                    let source_value = definition_fields.remove("source").ok_or_else(|| {
                        CodecError::new(
                            CodecErrorKind::InvalidArgument,
                            "Transfer.AssetDefinition.source field missing",
                        )
                    })?;
                    let object_value = definition_fields.remove("object").ok_or_else(|| {
                        CodecError::new(
                            CodecErrorKind::InvalidArgument,
                            "Transfer.AssetDefinition.object field missing",
                        )
                    })?;
                    let destination_value =
                        definition_fields.remove("destination").ok_or_else(|| {
                            CodecError::new(
                                CodecErrorKind::InvalidArgument,
                                "Transfer.AssetDefinition.destination field missing",
                            )
                        })?;
                    let source =
                        parse_account_id_value(source_value, "Transfer.AssetDefinition.source")?;
                    let definition: AssetDefinitionId =
                        json::from_value(object_value).map_err(codec_error)?;
                    let destination = parse_account_id_value(
                        destination_value,
                        "Transfer.AssetDefinition.destination",
                    )?;
                    let transfer = Transfer::asset_definition(source, definition, destination);
                    return Ok(InstructionBox::from(TransferBox::AssetDefinition(transfer)));
                }
                if let Some(json::Value::Object(mut nft_fields)) = transfer_map.remove("Nft") {
                    let source_value = nft_fields.remove("source").ok_or_else(|| {
                        CodecError::new(
                            CodecErrorKind::InvalidArgument,
                            "Transfer.Nft.source field missing",
                        )
                    })?;
                    let object_value = nft_fields.remove("object").ok_or_else(|| {
                        CodecError::new(
                            CodecErrorKind::InvalidArgument,
                            "Transfer.Nft.object field missing",
                        )
                    })?;
                    let destination_value = nft_fields.remove("destination").ok_or_else(|| {
                        CodecError::new(
                            CodecErrorKind::InvalidArgument,
                            "Transfer.Nft.destination field missing",
                        )
                    })?;
                    let source = parse_account_id_value(source_value, "Transfer.Nft.source")?;
                    let nft_id: NftId = json::from_value(object_value).map_err(codec_error)?;
                    let destination =
                        parse_account_id_value(destination_value, "Transfer.Nft.destination")?;
                    let transfer = Transfer::nft(source, nft_id, destination);
                    return Ok(InstructionBox::from(TransferBox::Nft(transfer)));
                }
                return Err(CodecError::new(
                    CodecErrorKind::InvalidArgument,
                    "unsupported Transfer instruction variant; expected keys: Asset, Domain, AssetDefinition, or Nft",
                ));
            }
            if let Some(json::Value::Object(mut grant_map)) = map.remove("Grant") {
                if let Some(json::Value::Object(mut fields)) = grant_map.remove("Permission") {
                    let object_value = required_value(&mut fields, "object", "Grant.Permission")?;
                    let destination = parse_account_id_value(
                        required_value(&mut fields, "destination", "Grant.Permission")?,
                        "Grant.Permission.destination",
                    )?;
                    if !fields.is_empty() {
                        return Err(CodecError::new(
                            CodecErrorKind::InvalidArgument,
                            format!(
                                "Grant.Permission contains unsupported fields: {}",
                                fields.keys().cloned().collect::<Vec<_>>().join(",")
                            ),
                        ));
                    }
                    let mut permission_fields = match object_value {
                        json::Value::Object(map) => map,
                        other => {
                            return Err(CodecError::new(
                                CodecErrorKind::InvalidArgument,
                                format!(
                                    "Grant.Permission.object must be an object (found {other:?})"
                                ),
                            ));
                        }
                    };
                    permission_fields
                        .entry("payload".to_owned())
                        .or_insert(json::Value::Null);
                    let permission: Permission =
                        json::from_value(json::Value::Object(permission_fields))
                            .map_err(codec_error)?;
                    let grant = Grant::account_permission(permission, destination);
                    return Ok(InstructionBox::from(GrantBox::Permission(grant)));
                }
                return Err(CodecError::new(
                    CodecErrorKind::InvalidArgument,
                    "unsupported Grant instruction variant; expected key: Permission",
                ));
            }
            if let Some(json::Value::Object(mut fields)) = map.remove("SetAssetDefinitionAlias") {
                let asset_definition_id: AssetDefinitionId = parse_string_value(
                    required_value(
                        &mut fields,
                        "asset_definition_id",
                        "SetAssetDefinitionAlias",
                    )?,
                    "SetAssetDefinitionAlias.asset_definition_id",
                )?
                .parse()
                .map_err(|err| {
                    CodecError::new(
                        CodecErrorKind::InvalidArgument,
                        format!(
                            "invalid SetAssetDefinitionAlias.asset_definition_id literal: {err}"
                        ),
                    )
                })?;
                let alias = parse_optional_string_value(
                    fields.remove("alias"),
                    "SetAssetDefinitionAlias.alias",
                )?
                .map(|literal| {
                    literal.parse::<AssetDefinitionAlias>().map_err(|err| {
                        CodecError::new(
                            CodecErrorKind::InvalidArgument,
                            format!("invalid SetAssetDefinitionAlias.alias literal: {err}"),
                        )
                    })
                })
                .transpose()?;
                let lease_expiry_ms = match fields.remove("lease_expiry_ms") {
                    None | Some(json::Value::Null) => None,
                    Some(value) => Some(parse_u64_value(
                        value,
                        "SetAssetDefinitionAlias.lease_expiry_ms",
                    )?),
                };
                if !fields.is_empty() {
                    return Err(CodecError::new(
                        CodecErrorKind::InvalidArgument,
                        format!(
                            "SetAssetDefinitionAlias contains unsupported fields: {}",
                            fields.keys().cloned().collect::<Vec<_>>().join(",")
                        ),
                    ));
                }
                let instruction = match alias {
                    Some(alias) => {
                        SetAssetDefinitionAlias::bind(asset_definition_id, alias, lease_expiry_ms)
                    }
                    None => SetAssetDefinitionAlias::clear(asset_definition_id),
                };
                return Ok(InstructionBox::from(instruction));
            }
            if let Some(json::Value::Object(mut fields)) = map.remove("RegisterRwa") {
                let rwa_value = required_value(&mut fields, "rwa", "RegisterRwa")?;
                let json::Value::Object(mut fields) = rwa_value else {
                    return Err(CodecError::new(
                        CodecErrorKind::InvalidArgument,
                        "RegisterRwa.rwa must be an object",
                    ));
                };
                let domain: DomainId =
                    json::from_value(required_value(&mut fields, "domain", "RegisterRwa.rwa")?)
                        .map_err(codec_error)?;
                let quantity: Quantity =
                    json::from_value(required_value(&mut fields, "quantity", "RegisterRwa.rwa")?)
                        .map_err(codec_error)?;
                let spec =
                    json::from_value(required_value(&mut fields, "spec", "RegisterRwa.rwa")?)
                        .map_err(codec_error)?;
                let primary_reference = parse_string_value(
                    required_value(&mut fields, "primary_reference", "RegisterRwa.rwa")?,
                    "RegisterRwa.rwa.primary_reference",
                )?;
                let status: Option<Name> =
                    fields
                        .remove("status")
                        .map_or(Ok(None), |value| match value {
                            json::Value::Null => Ok(None),
                            other => json::from_value(other).map_err(codec_error),
                        })?;
                let metadata = fields
                    .remove("metadata")
                    .map_or(Ok(Metadata::default()), |value| {
                        json::from_value(value).map_err(codec_error)
                    })?;
                let parents = fields.remove("parents").map_or(Ok(Vec::new()), |value| {
                    parse_rwa_parent_refs_value(value, "RegisterRwa.rwa.parents")
                })?;
                let controls = fields
                    .remove("controls")
                    .map_or(Ok(RwaControlPolicy::default()), |value| {
                        json::from_value(value).map_err(codec_error)
                    })?;
                let register = RegisterRwa {
                    rwa: NewRwa::new(
                        domain,
                        quantity,
                        spec,
                        primary_reference,
                        status,
                        metadata,
                        parents,
                        controls,
                    ),
                };
                return Ok(InstructionBox::from(RwaInstructionBox::from(register)));
            }
            if let Some(json::Value::Object(mut fields)) = map.remove("TransferRwa") {
                let source = parse_account_id_value(
                    required_value(&mut fields, "source", "TransferRwa")?,
                    "TransferRwa.source",
                )?;
                let rwa = parse_rwa_id_value(
                    required_value(&mut fields, "rwa", "TransferRwa")?,
                    "TransferRwa.rwa",
                )?;
                let quantity: Quantity =
                    json::from_value(required_value(&mut fields, "quantity", "TransferRwa")?)
                        .map_err(codec_error)?;
                let destination = parse_account_id_value(
                    required_value(&mut fields, "destination", "TransferRwa")?,
                    "TransferRwa.destination",
                )?;
                return Ok(InstructionBox::from(RwaInstructionBox::from(TransferRwa {
                    source,
                    rwa,
                    quantity,
                    destination,
                })));
            }
            if let Some(json::Value::Object(mut fields)) = map.remove("MergeRwas") {
                let parents = parse_rwa_parent_refs_value(
                    required_value(&mut fields, "parents", "MergeRwas")?,
                    "MergeRwas.parents",
                )?;
                let primary_reference = parse_string_value(
                    required_value(&mut fields, "primary_reference", "MergeRwas")?,
                    "MergeRwas.primary_reference",
                )?;
                let status: Option<Name> =
                    fields
                        .remove("status")
                        .map_or(Ok(None), |value| match value {
                            json::Value::Null => Ok(None),
                            other => json::from_value(other).map_err(codec_error),
                        })?;
                let metadata = fields
                    .remove("metadata")
                    .map_or(Ok(Metadata::default()), |value| {
                        json::from_value(value).map_err(codec_error)
                    })?;
                return Ok(InstructionBox::from(RwaInstructionBox::from(MergeRwas {
                    parents,
                    primary_reference,
                    status,
                    metadata,
                })));
            }
            if let Some(json::Value::Object(mut fields)) = map.remove("RedeemRwa") {
                let rwa = parse_rwa_id_value(
                    required_value(&mut fields, "rwa", "RedeemRwa")?,
                    "RedeemRwa.rwa",
                )?;
                let quantity: Quantity =
                    json::from_value(required_value(&mut fields, "quantity", "RedeemRwa")?)
                        .map_err(codec_error)?;
                return Ok(InstructionBox::from(RwaInstructionBox::from(RedeemRwa {
                    rwa,
                    quantity,
                })));
            }
            if let Some(json::Value::Object(mut fields)) = map.remove("FreezeRwa") {
                let rwa = parse_rwa_id_value(
                    required_value(&mut fields, "rwa", "FreezeRwa")?,
                    "FreezeRwa.rwa",
                )?;
                return Ok(InstructionBox::from(RwaInstructionBox::from(FreezeRwa {
                    rwa,
                })));
            }
            if let Some(json::Value::Object(mut fields)) = map.remove("UnfreezeRwa") {
                let rwa = parse_rwa_id_value(
                    required_value(&mut fields, "rwa", "UnfreezeRwa")?,
                    "UnfreezeRwa.rwa",
                )?;
                return Ok(InstructionBox::from(RwaInstructionBox::from(UnfreezeRwa {
                    rwa,
                })));
            }
            if let Some(json::Value::Object(mut fields)) = map.remove("HoldRwa") {
                let rwa = parse_rwa_id_value(
                    required_value(&mut fields, "rwa", "HoldRwa")?,
                    "HoldRwa.rwa",
                )?;
                let quantity: Quantity =
                    json::from_value(required_value(&mut fields, "quantity", "HoldRwa")?)
                        .map_err(codec_error)?;
                return Ok(InstructionBox::from(RwaInstructionBox::from(HoldRwa {
                    rwa,
                    quantity,
                })));
            }
            if let Some(json::Value::Object(mut fields)) = map.remove("ReleaseRwa") {
                let rwa = parse_rwa_id_value(
                    required_value(&mut fields, "rwa", "ReleaseRwa")?,
                    "ReleaseRwa.rwa",
                )?;
                let quantity: Quantity =
                    json::from_value(required_value(&mut fields, "quantity", "ReleaseRwa")?)
                        .map_err(codec_error)?;
                return Ok(InstructionBox::from(RwaInstructionBox::from(ReleaseRwa {
                    rwa,
                    quantity,
                })));
            }
            if let Some(json::Value::Object(mut fields)) = map.remove("ForceTransferRwa") {
                let rwa = parse_rwa_id_value(
                    required_value(&mut fields, "rwa", "ForceTransferRwa")?,
                    "ForceTransferRwa.rwa",
                )?;
                let quantity: Quantity =
                    json::from_value(required_value(&mut fields, "quantity", "ForceTransferRwa")?)
                        .map_err(codec_error)?;
                let destination = parse_account_id_value(
                    required_value(&mut fields, "destination", "ForceTransferRwa")?,
                    "ForceTransferRwa.destination",
                )?;
                return Ok(InstructionBox::from(RwaInstructionBox::from(
                    ForceTransferRwa {
                        rwa,
                        quantity,
                        destination,
                    },
                )));
            }
            if let Some(json::Value::Object(mut fields)) = map.remove("SetRwaControls") {
                let rwa = parse_rwa_id_value(
                    required_value(&mut fields, "rwa", "SetRwaControls")?,
                    "SetRwaControls.rwa",
                )?;
                let controls: RwaControlPolicy =
                    json::from_value(required_value(&mut fields, "controls", "SetRwaControls")?)
                        .map_err(codec_error)?;
                return Ok(InstructionBox::from(RwaInstructionBox::from(
                    SetRwaControls { rwa, controls },
                )));
            }
            if let Some(json::Value::Object(mut fields)) = map.remove("SetRwaKeyValue") {
                let rwa = parse_rwa_id_value(
                    required_value(&mut fields, "rwa", "SetRwaKeyValue")?,
                    "SetRwaKeyValue.rwa",
                )?;
                let key: Name =
                    json::from_value(required_value(&mut fields, "key", "SetRwaKeyValue")?)
                        .map_err(codec_error)?;
                let value: Json =
                    json::from_value(required_value(&mut fields, "value", "SetRwaKeyValue")?)
                        .map_err(codec_error)?;
                return Ok(InstructionBox::from(RwaInstructionBox::from(
                    SetKeyValue::rwa(rwa, key, value),
                )));
            }
            if let Some(json::Value::Object(mut variants)) = map.remove("SetKeyValue") {
                if let Some(json::Value::Object(mut fields)) = variants.remove("Account") {
                    let account = parse_account_id_value(
                        required_value(&mut fields, "object", "SetKeyValue.Account")?,
                        "SetKeyValue.Account.object",
                    )?;
                    let key: Name = json::from_value(required_value(
                        &mut fields,
                        "key",
                        "SetKeyValue.Account",
                    )?)
                    .map_err(codec_error)?;
                    let value: Json = json::from_value(required_value(
                        &mut fields,
                        "value",
                        "SetKeyValue.Account",
                    )?)
                    .map_err(codec_error)?;
                    if !fields.is_empty() {
                        return Err(CodecError::new(
                            CodecErrorKind::InvalidArgument,
                            format!(
                                "SetKeyValue.Account contains unsupported fields: {}",
                                fields.keys().cloned().collect::<Vec<_>>().join(", ")
                            ),
                        ));
                    }
                    if !variants.is_empty() {
                        return Err(CodecError::new(
                            CodecErrorKind::InvalidArgument,
                            "SetKeyValue must contain exactly one variant",
                        ));
                    }
                    return Ok(InstructionBox::from(SetKeyValue::account(
                        account, key, value,
                    )));
                }
                if let Some(json::Value::Object(mut fields)) = variants.remove("Nft") {
                    let nft_id: NftId =
                        json::from_value(required_value(&mut fields, "object", "SetKeyValue.Nft")?)
                            .map_err(codec_error)?;
                    let key: Name =
                        json::from_value(required_value(&mut fields, "key", "SetKeyValue.Nft")?)
                            .map_err(codec_error)?;
                    let value: Json =
                        json::from_value(required_value(&mut fields, "value", "SetKeyValue.Nft")?)
                            .map_err(codec_error)?;
                    if !fields.is_empty() {
                        return Err(CodecError::new(
                            CodecErrorKind::InvalidArgument,
                            format!(
                                "SetKeyValue.Nft contains unsupported fields: {}",
                                fields.keys().cloned().collect::<Vec<_>>().join(", ")
                            ),
                        ));
                    }
                    if !variants.is_empty() {
                        return Err(CodecError::new(
                            CodecErrorKind::InvalidArgument,
                            "SetKeyValue must contain exactly one variant",
                        ));
                    }
                    return Ok(InstructionBox::from(SetKeyValue::nft(nft_id, key, value)));
                }
                return Err(CodecError::new(
                    CodecErrorKind::InvalidArgument,
                    "SetKeyValue currently supports the Account and Nft variants",
                ));
            }
            if let Some(json::Value::Object(mut fields)) = map.remove("RemoveRwaKeyValue") {
                let rwa = parse_rwa_id_value(
                    required_value(&mut fields, "rwa", "RemoveRwaKeyValue")?,
                    "RemoveRwaKeyValue.rwa",
                )?;
                let key: Name =
                    json::from_value(required_value(&mut fields, "key", "RemoveRwaKeyValue")?)
                        .map_err(codec_error)?;
                return Ok(InstructionBox::from(RwaInstructionBox::from(
                    RemoveKeyValue::rwa(rwa, key),
                )));
            }
            if let Some(json::Value::Object(mut kaigi_map)) = map.remove("Kaigi") {
                if let Some(json::Value::Object(mut create_fields)) =
                    kaigi_map.remove("CreateKaigi")
                {
                    let call_value = create_fields.remove("call").ok_or_else(|| {
                        CodecError::new(
                            CodecErrorKind::InvalidArgument,
                            "CreateKaigi.call field missing",
                        )
                    })?;
                    let call: NewKaigi = json::from_value(call_value).map_err(|err| {
                        CodecError::new(
                            CodecErrorKind::InvalidArgument,
                            format!("CreateKaigi.call parse error: {err}"),
                        )
                    })?;
                    let commitment = parse_optional_commitment(
                        create_fields.remove("commitment"),
                        "CreateKaigi",
                    )?;
                    let nullifier =
                        parse_optional_nullifier(create_fields.remove("nullifier"), "CreateKaigi")?;
                    let roster_root = parse_optional_hash(
                        create_fields.remove("roster_root"),
                        "CreateKaigi.roster_root",
                    )?;
                    let proof =
                        parse_optional_base64(create_fields.remove("proof"), "CreateKaigi.proof")?;
                    let instruction = CreateKaigi {
                        call,
                        commitment,
                        nullifier,
                        roster_root,
                        proof,
                    };
                    return Ok(Box::new(instruction).into_instruction_box());
                }
                if let Some(json::Value::Object(mut join_fields)) = kaigi_map.remove("JoinKaigi") {
                    let call_id_value = join_fields.remove("call_id").ok_or_else(|| {
                        CodecError::new(
                            CodecErrorKind::InvalidArgument,
                            "JoinKaigi.call_id field missing",
                        )
                    })?;
                    let call_id: KaigiId = json::from_value(call_id_value).map_err(codec_error)?;
                    let participant_value = join_fields.remove("participant").ok_or_else(|| {
                        CodecError::new(
                            CodecErrorKind::InvalidArgument,
                            "JoinKaigi.participant field missing",
                        )
                    })?;
                    let participant =
                        parse_account_id_value(participant_value, "JoinKaigi.participant")?;
                    let commitment =
                        parse_optional_commitment(join_fields.remove("commitment"), "JoinKaigi")?;
                    let nullifier =
                        parse_optional_nullifier(join_fields.remove("nullifier"), "JoinKaigi")?;
                    let roster_root = parse_optional_hash(
                        join_fields.remove("roster_root"),
                        "JoinKaigi.roster_root",
                    )?;
                    let proof =
                        parse_optional_base64(join_fields.remove("proof"), "JoinKaigi.proof")?;
                    let join = JoinKaigi {
                        call_id,
                        participant,
                        commitment,
                        nullifier,
                        roster_root,
                        proof,
                    };
                    return Ok(Box::new(join).into_instruction_box());
                }
                if let Some(json::Value::Object(mut leave_fields)) = kaigi_map.remove("LeaveKaigi")
                {
                    let call_id_value = leave_fields.remove("call_id").ok_or_else(|| {
                        CodecError::new(
                            CodecErrorKind::InvalidArgument,
                            "LeaveKaigi.call_id field missing",
                        )
                    })?;
                    let call_id: KaigiId = json::from_value(call_id_value).map_err(codec_error)?;
                    let participant_value =
                        leave_fields.remove("participant").ok_or_else(|| {
                            CodecError::new(
                                CodecErrorKind::InvalidArgument,
                                "LeaveKaigi.participant field missing",
                            )
                        })?;
                    let participant =
                        parse_account_id_value(participant_value, "LeaveKaigi.participant")?;
                    let commitment =
                        parse_optional_commitment(leave_fields.remove("commitment"), "LeaveKaigi")?;
                    let nullifier =
                        parse_optional_nullifier(leave_fields.remove("nullifier"), "LeaveKaigi")?;
                    let roster_root = parse_optional_hash(
                        leave_fields.remove("roster_root"),
                        "LeaveKaigi.roster_root",
                    )?;
                    let proof =
                        parse_optional_base64(leave_fields.remove("proof"), "LeaveKaigi.proof")?;
                    let leave = LeaveKaigi {
                        call_id,
                        participant,
                        commitment,
                        nullifier,
                        roster_root,
                        proof,
                    };
                    return Ok(Box::new(leave).into_instruction_box());
                }
                if let Some(json::Value::Object(mut end_fields)) = kaigi_map.remove("EndKaigi") {
                    let call_id_value = end_fields.remove("call_id").ok_or_else(|| {
                        CodecError::new(
                            CodecErrorKind::InvalidArgument,
                            "EndKaigi.call_id field missing",
                        )
                    })?;
                    let call_id: KaigiId = json::from_value(call_id_value).map_err(codec_error)?;
                    let ended_at = match end_fields.remove("ended_at_ms") {
                        None | Some(json::Value::Null) => None,
                        Some(value) => Some(json::from_value(value).map_err(codec_error)?),
                    };
                    let commitment =
                        parse_optional_commitment(end_fields.remove("commitment"), "EndKaigi")?;
                    let nullifier =
                        parse_optional_nullifier(end_fields.remove("nullifier"), "EndKaigi")?;
                    let roster_root = parse_optional_hash(
                        end_fields.remove("roster_root"),
                        "EndKaigi.roster_root",
                    )?;
                    let proof =
                        parse_optional_base64(end_fields.remove("proof"), "EndKaigi.proof")?;
                    let end = EndKaigi {
                        call_id,
                        ended_at_ms: ended_at,
                        commitment,
                        nullifier,
                        roster_root,
                        proof,
                    };
                    return Ok(Box::new(end).into_instruction_box());
                }
                if let Some(json::Value::Object(mut usage_fields)) =
                    kaigi_map.remove("RecordKaigiUsage")
                {
                    let call_id_value = usage_fields.remove("call_id").ok_or_else(|| {
                        CodecError::new(
                            CodecErrorKind::InvalidArgument,
                            "RecordKaigiUsage.call_id field missing",
                        )
                    })?;
                    let call_id: KaigiId = json::from_value(call_id_value).map_err(codec_error)?;
                    let duration_value = usage_fields.remove("duration_ms").ok_or_else(|| {
                        CodecError::new(
                            CodecErrorKind::InvalidArgument,
                            "RecordKaigiUsage.duration_ms field missing",
                        )
                    })?;
                    let duration_ms: u64 = json::from_value(duration_value).map_err(codec_error)?;
                    let billed_gas = usage_fields
                        .remove("billed_gas")
                        .map(|value| json::from_value(value).map_err(codec_error))
                        .transpose()?
                        .unwrap_or_default();
                    let usage_commitment = parse_optional_kaigi_scalar(
                        usage_fields.remove("usage_commitment"),
                        "RecordKaigiUsage.usage_commitment",
                    )?;
                    let proof = parse_optional_base64(
                        usage_fields.remove("proof"),
                        "RecordKaigiUsage.proof",
                    )?;
                    let usage = RecordKaigiUsage {
                        call_id,
                        duration_ms,
                        billed_gas,
                        usage_commitment,
                        proof,
                    };
                    return Ok(Box::new(usage).into_instruction_box());
                }
                if let Some(json::Value::Object(mut manifest_fields)) =
                    kaigi_map.remove("SetKaigiRelayManifest")
                {
                    let call_id_value = manifest_fields.remove("call_id").ok_or_else(|| {
                        CodecError::new(
                            CodecErrorKind::InvalidArgument,
                            "SetKaigiRelayManifest.call_id field missing",
                        )
                    })?;
                    let call_id: KaigiId = json::from_value(call_id_value).map_err(codec_error)?;
                    let relay_manifest =
                        manifest_fields
                            .remove("relay_manifest")
                            .map_or(Ok(None), |value| match value {
                                json::Value::Null => Ok(None),
                                other => json::from_value(other).map(Some).map_err(codec_error),
                            })?;
                    let manifest = SetKaigiRelayManifest {
                        call_id,
                        relay_manifest,
                    };
                    return Ok(Box::new(manifest).into_instruction_box());
                }
                if let Some(json::Value::Object(mut register_fields)) =
                    kaigi_map.remove("RegisterKaigiRelay")
                {
                    let relay_value = register_fields.remove("relay").ok_or_else(|| {
                        CodecError::new(
                            CodecErrorKind::InvalidArgument,
                            "RegisterKaigiRelay.relay field missing",
                        )
                    })?;
                    let relay: KaigiRelayRegistration =
                        json::from_value(relay_value).map_err(codec_error)?;
                    let registration = RegisterKaigiRelay { relay };
                    return Ok(Box::new(registration).into_instruction_box());
                }
                if let Some(json::Value::Object(mut unregister_fields)) =
                    kaigi_map.remove("UnregisterKaigiRelay")
                {
                    let relay_id_value = unregister_fields.remove("relay_id").ok_or_else(|| {
                        CodecError::new(
                            CodecErrorKind::InvalidArgument,
                            "UnregisterKaigiRelay.relay_id field missing",
                        )
                    })?;
                    let relay_id =
                        parse_account_id_value(relay_id_value, "UnregisterKaigiRelay.relay_id")?;
                    return Ok(Box::new(UnregisterKaigiRelay { relay_id }).into_instruction_box());
                }
                if let Some(json::Value::Object(mut health_fields)) =
                    kaigi_map.remove("ReportKaigiRelayHealth")
                {
                    let call_id_value = health_fields.remove("call_id").ok_or_else(|| {
                        CodecError::new(
                            CodecErrorKind::InvalidArgument,
                            "ReportKaigiRelayHealth.call_id field missing",
                        )
                    })?;
                    let call_id: KaigiId = json::from_value(call_id_value).map_err(codec_error)?;
                    let relay_id_value = health_fields.remove("relay_id").ok_or_else(|| {
                        CodecError::new(
                            CodecErrorKind::InvalidArgument,
                            "ReportKaigiRelayHealth.relay_id field missing",
                        )
                    })?;
                    let relay_id =
                        parse_account_id_value(relay_id_value, "ReportKaigiRelayHealth.relay_id")?;
                    let status_value = health_fields.remove("status").ok_or_else(|| {
                        CodecError::new(
                            CodecErrorKind::InvalidArgument,
                            "ReportKaigiRelayHealth.status field missing",
                        )
                    })?;
                    let status: KaigiRelayHealthStatus =
                        json::from_value(status_value).map_err(codec_error)?;
                    let reported_at_ms = health_fields
                        .remove("reported_at_ms")
                        .map_or(Ok(0_u64), |value| {
                            json::from_value(value).map_err(codec_error)
                        })?;
                    let notes =
                        health_fields
                            .remove("notes")
                            .map_or(Ok(None), |value| match value {
                                json::Value::Null => Ok(None),
                                other => json::from_value(other).map(Some).map_err(codec_error),
                            })?;
                    let report = ReportKaigiRelayHealth {
                        call_id,
                        relay_id,
                        status,
                        reported_at_ms,
                        notes,
                    };
                    return Ok(Box::new(report).into_instruction_box());
                }
                return Err(CodecError::new(
                    CodecErrorKind::InvalidArgument,
                    "unsupported Kaigi instruction variant; see iroha_data_model::isi::kaigi for supported set",
                ));
            }
            if let Some(json::Value::Object(mut fields)) = map.remove("ProposeDeployContract") {
                let contract_address: iroha_data_model::smart_contract::ContractAddress =
                    parse_string_value(
                        required_value(&mut fields, "contract_address", "ProposeDeployContract")?,
                        "ProposeDeployContract.contract_address",
                    )?
                    .parse()
                    .map_err(|err| {
                        CodecError::new(
                            CodecErrorKind::InvalidArgument,
                            format!(
                                "invalid ProposeDeployContract.contract_address literal: {err}"
                            ),
                        )
                    })?;
                let code_hash: ContractCodeHash = json::from_value(required_value(
                    &mut fields,
                    "code_hash",
                    "ProposeDeployContract",
                )?)
                .map_err(codec_error)?;
                let abi_hash: ContractAbiHash = json::from_value(required_value(
                    &mut fields,
                    "abi_hash",
                    "ProposeDeployContract",
                )?)
                .map_err(codec_error)?;
                let abi_version: AbiVersion = json::from_value(required_value(
                    &mut fields,
                    "abi_version",
                    "ProposeDeployContract",
                )?)
                .map_err(codec_error)?;
                let manifest_provenance = match fields.remove("manifest_provenance") {
                    None | Some(json::Value::Null) => None,
                    Some(value) => Some(json::from_value(value).map_err(codec_error)?),
                };
                if !fields.is_empty() {
                    return Err(CodecError::new(
                        CodecErrorKind::InvalidArgument,
                        format!(
                            "ProposeDeployContract contains unexpected field(s): {}",
                            fields.keys().cloned().collect::<Vec<_>>().join(", ")
                        ),
                    ));
                }
                let instruction = ProposeDeployContract {
                    contract_address,
                    code_hash,
                    abi_hash,
                    abi_version,
                    manifest_provenance,
                };
                return Ok(Box::new(instruction).into_instruction_box());
            }
            if let Some(json::Value::Object(mut fields)) = map.remove("CastZkBallot") {
                let election_id = parse_string_value(
                    required_value(&mut fields, "election_id", "CastZkBallot")?,
                    "CastZkBallot.election_id",
                )?;
                let proof_b64 = parse_string_value(
                    required_value(&mut fields, "proof_b64", "CastZkBallot")?,
                    "CastZkBallot.proof_b64",
                )?;
                let public_inputs_json = parse_string_value(
                    required_value(&mut fields, "public_inputs_json", "CastZkBallot")?,
                    "CastZkBallot.public_inputs_json",
                )?;
                let public_inputs_json = normalize_zk_ballot_public_inputs_json(
                    public_inputs_json.as_str(),
                    "CastZkBallot.public_inputs_json",
                )?;
                let ballot = CastZkBallot {
                    election_id,
                    proof_b64,
                    public_inputs_json,
                };
                return Ok(Box::new(ballot).into_instruction_box());
            }
            if let Some(json::Value::Object(mut fields)) = map.remove("CastPlainBallot") {
                let referendum_id = parse_string_value(
                    required_value(&mut fields, "referendum_id", "CastPlainBallot")?,
                    "CastPlainBallot.referendum_id",
                )?;
                let owner_value = required_value(&mut fields, "owner", "CastPlainBallot")?;
                let owner = parse_account_id_value(owner_value, "CastPlainBallot.owner")?;
                let amount = parse_canonical_quantity_value(
                    required_value(&mut fields, "amount", "CastPlainBallot")?,
                    "CastPlainBallot.amount",
                )?;
                let duration_blocks = parse_u64_value(
                    required_value(&mut fields, "duration_blocks", "CastPlainBallot")?,
                    "CastPlainBallot.duration_blocks",
                )?;
                let direction = parse_u8_value(
                    required_value(&mut fields, "direction", "CastPlainBallot")?,
                    "CastPlainBallot.direction",
                )?;
                let ballot = CastPlainBallot {
                    referendum_id,
                    owner,
                    amount,
                    duration_blocks,
                    direction,
                };
                return Ok(Box::new(ballot).into_instruction_box());
            }
            if let Some(json::Value::Object(mut fields)) = map.remove("RegisterCitizen") {
                let owner_value = required_value(&mut fields, "owner", "RegisterCitizen")?;
                let owner = parse_account_id_value(owner_value, "RegisterCitizen.owner")?;
                let amount = parse_canonical_quantity_value(
                    required_value(&mut fields, "amount", "RegisterCitizen")?,
                    "RegisterCitizen.amount",
                )?;
                let instruction = RegisterCitizen { owner, amount };
                return Ok(Box::new(instruction).into_instruction_box());
            }
            if let Some(json::Value::Object(mut fields)) = map.remove("SubmitAgendaProposal") {
                let proposal: AgendaProposalV1 = json::from_value(required_value(
                    &mut fields,
                    "proposal",
                    "SubmitAgendaProposal",
                )?)
                .map_err(codec_error)?;
                let instruction = SubmitAgendaProposal { proposal };
                return Ok(Box::new(instruction).into_instruction_box());
            }
            if let Some(json::Value::Object(mut fields)) = map.remove("RegisterSmartContractBytes")
            {
                let code_hash_value =
                    required_value(&mut fields, "code_hash", "RegisterSmartContractBytes")?;
                let code_hash =
                    parse_hash_value(code_hash_value, "RegisterSmartContractBytes.code_hash")?;
                let code_value = required_value(&mut fields, "code", "RegisterSmartContractBytes")?;
                let code = parse_base64(code_value, "RegisterSmartContractBytes.code")?;
                let instruction = RegisterSmartContractBytes { code_hash, code };
                return Ok(Box::new(instruction).into_instruction_box());
            }
            if let Some(json::Value::Object(mut fields)) = map.remove("RemoveSmartContractBytes") {
                let code_hash_value =
                    required_value(&mut fields, "code_hash", "RemoveSmartContractBytes")?;
                let code_hash =
                    parse_hash_value(code_hash_value, "RemoveSmartContractBytes.code_hash")?;
                let reason = parse_optional_string_value(
                    fields.remove("reason"),
                    "RemoveSmartContractBytes.reason",
                )?;
                let instruction = RemoveSmartContractBytes { code_hash, reason };
                return Ok(Box::new(instruction).into_instruction_box());
            }
            if let Some(json::Value::Object(mut fields)) = map.remove("ActivateContractInstance") {
                let contract_address: iroha_data_model::smart_contract::ContractAddress =
                    parse_string_value(
                        required_value(
                            &mut fields,
                            "contract_address",
                            "ActivateContractInstance",
                        )?,
                        "ActivateContractInstance.contract_address",
                    )?
                    .parse()
                    .map_err(|err| {
                        CodecError::new(
                            CodecErrorKind::InvalidArgument,
                            format!(
                                "invalid ActivateContractInstance.contract_address literal: {err}"
                            ),
                        )
                    })?;
                let expected_revision = parse_u64_value(
                    required_value(&mut fields, "expected_revision", "ActivateContractInstance")?,
                    "ActivateContractInstance.expected_revision",
                )?;
                let code_hash_value =
                    required_value(&mut fields, "code_hash", "ActivateContractInstance")?;
                let code_hash =
                    parse_hash_value(code_hash_value, "ActivateContractInstance.code_hash")?;
                let instruction = ActivateContractInstance {
                    contract_address,
                    expected_revision,
                    code_hash,
                };
                return Ok(Box::new(instruction).into_instruction_box());
            }
            if let Some(json::Value::Object(mut fields)) = map.remove("DeactivateContractInstance")
            {
                let contract_address: iroha_data_model::smart_contract::ContractAddress =
                    parse_string_value(
                        required_value(
                            &mut fields,
                            "contract_address",
                            "DeactivateContractInstance",
                        )?,
                        "DeactivateContractInstance.contract_address",
                    )?
                    .parse()
                    .map_err(|err| {
                        CodecError::new(
                            CodecErrorKind::InvalidArgument,
                            format!(
                                "invalid DeactivateContractInstance.contract_address literal: {err}"
                            ),
                        )
                    })?;
                let expected_revision = parse_u64_value(
                    required_value(
                        &mut fields,
                        "expected_revision",
                        "DeactivateContractInstance",
                    )?,
                    "DeactivateContractInstance.expected_revision",
                )?;
                let reason = parse_optional_string_value(
                    fields.remove("reason"),
                    "DeactivateContractInstance.reason",
                )?;
                let instruction = DeactivateContractInstance {
                    contract_address,
                    expected_revision,
                    reason,
                };
                return Ok(Box::new(instruction).into_instruction_box());
            }
            if let Some(value) = map.remove("ClaimTwitterFollowReward") {
                let mut fields = match value {
                    json::Value::Object(map) => map,
                    other => {
                        return Err(CodecError::new(
                            CodecErrorKind::InvalidArgument,
                            format!(
                                "ClaimTwitterFollowReward payload must be an object (found {other:?})"
                            ),
                        ));
                    }
                };
                let binding_hash = parse_keyed_hash(
                    required_value(&mut fields, "binding_hash", "ClaimTwitterFollowReward")?,
                    "ClaimTwitterFollowReward.binding_hash",
                )?;
                let instruction = ClaimTwitterFollowReward { binding_hash };
                return Ok(Box::new(instruction).into_instruction_box());
            }
            if let Some(value) = map.remove("SendToTwitter") {
                let mut fields = match value {
                    json::Value::Object(map) => map,
                    other => {
                        return Err(CodecError::new(
                            CodecErrorKind::InvalidArgument,
                            format!("SendToTwitter payload must be an object (found {other:?})"),
                        ));
                    }
                };
                let binding_hash = parse_keyed_hash(
                    required_value(&mut fields, "binding_hash", "SendToTwitter")?,
                    "SendToTwitter.binding_hash",
                )?;
                let amount: Quantity =
                    json::from_value(required_value(&mut fields, "amount", "SendToTwitter")?)
                        .map_err(codec_error)?;
                let instruction = SendToTwitter {
                    binding_hash,
                    amount,
                };
                return Ok(Box::new(instruction).into_instruction_box());
            }
            if let Some(value) = map.remove("CancelTwitterEscrow") {
                let mut fields = match value {
                    json::Value::Object(map) => map,
                    other => {
                        return Err(CodecError::new(
                            CodecErrorKind::InvalidArgument,
                            format!(
                                "CancelTwitterEscrow payload must be an object (found {other:?})"
                            ),
                        ));
                    }
                };
                let binding_hash = parse_keyed_hash(
                    required_value(&mut fields, "binding_hash", "CancelTwitterEscrow")?,
                    "CancelTwitterEscrow.binding_hash",
                )?;
                let instruction = CancelTwitterEscrow { binding_hash };
                return Ok(Box::new(instruction).into_instruction_box());
            }
            if let Some(custom_value) = remove_case_insensitive(&mut map, "Custom") {
                let mut custom_map = match custom_value {
                    json::Value::Object(map) => map,
                    other => {
                        return Err(CodecError::new(
                            CodecErrorKind::InvalidArgument,
                            format!(
                                "Custom instruction payload must be an object (found {other:?})"
                            ),
                        ));
                    }
                };
                let payload =
                    remove_case_insensitive(&mut custom_map, "payload").ok_or_else(|| {
                        CodecError::new(
                            CodecErrorKind::InvalidArgument,
                            "Custom.payload field missing",
                        )
                    })?;
                return Ok(InstructionBox::from(CustomInstruction::new(payload)));
            }
            if let Some(multisig_value) = remove_case_insensitive(&mut map, "Multisig") {
                let multisig_map = match multisig_value {
                    json::Value::Object(map) => map,
                    other => {
                        return Err(CodecError::new(
                            CodecErrorKind::InvalidArgument,
                            format!(
                                "Multisig instruction payload must be an object (found {other:?})"
                            ),
                        ));
                    }
                };
                return Ok(InstructionBox::from(CustomInstruction::new(
                    json::Value::Object(multisig_map),
                )));
            }
            if let Some(propose_value) = remove_case_insensitive(&mut map, "MultisigPropose") {
                let propose_fields = match propose_value {
                    json::Value::Object(map) => map,
                    other => {
                        return Err(CodecError::new(
                            CodecErrorKind::InvalidArgument,
                            format!("MultisigPropose payload must be an object (found {other:?})"),
                        ));
                    }
                };
                let mut payload = json::Map::new();
                payload.insert("Propose".to_owned(), json::Value::Object(propose_fields));
                return Ok(InstructionBox::from(CustomInstruction::new(
                    json::Value::Object(payload),
                )));
            }
            if let Some(approve_value) = remove_case_insensitive(&mut map, "MultisigApprove") {
                let approve_fields = match approve_value {
                    json::Value::Object(map) => map,
                    other => {
                        return Err(CodecError::new(
                            CodecErrorKind::InvalidArgument,
                            format!("MultisigApprove payload must be an object (found {other:?})"),
                        ));
                    }
                };
                let mut payload = json::Map::new();
                payload.insert("Approve".to_owned(), json::Value::Object(approve_fields));
                return Ok(InstructionBox::from(CustomInstruction::new(
                    json::Value::Object(payload),
                )));
            }
            if let Some(cancel_value) = remove_case_insensitive(&mut map, "MultisigCancel") {
                let cancel_fields = match cancel_value {
                    json::Value::Object(map) => map,
                    other => {
                        return Err(CodecError::new(
                            CodecErrorKind::InvalidArgument,
                            format!("MultisigCancel payload must be an object (found {other:?})"),
                        ));
                    }
                };
                let mut payload = json::Map::new();
                payload.insert("Cancel".to_owned(), json::Value::Object(cancel_fields));
                return Ok(InstructionBox::from(CustomInstruction::new(
                    json::Value::Object(payload),
                )));
            }
            if let Some(register_value) = remove_case_insensitive(&mut map, "MultisigRegister") {
                let register_fields = match register_value {
                    json::Value::Object(map) => map,
                    other => {
                        return Err(CodecError::new(
                            CodecErrorKind::InvalidArgument,
                            format!("MultisigRegister payload must be an object (found {other:?})"),
                        ));
                    }
                };
                let mut payload = json::Map::new();
                payload.insert("Register".to_owned(), json::Value::Object(register_fields));
                return Ok(InstructionBox::from(CustomInstruction::new(
                    json::Value::Object(payload),
                )));
            }
            if let Some(zk_value) = remove_case_insensitive(&mut map, "Zk") {
                let mut zk_map = match zk_value {
                    json::Value::Object(map) => map,
                    other => {
                        return Err(CodecError::new(
                            CodecErrorKind::InvalidArgument,
                            format!("Zk instruction payload must be an object (found {other:?})"),
                        ));
                    }
                };
                if let Some(payload) = zk_map.remove("RegisterZkAsset") {
                    let instruction: RegisterZkAsset =
                        json::from_value(payload).map_err(codec_error)?;
                    return Ok(Box::new(instruction).into_instruction_box());
                }
                if let Some(payload) = zk_map.remove("ScheduleConfidentialPolicyTransition") {
                    let instruction: ScheduleConfidentialPolicyTransition =
                        json::from_value(payload).map_err(codec_error)?;
                    return Ok(Box::new(instruction).into_instruction_box());
                }
                if let Some(payload) = zk_map.remove("CancelConfidentialPolicyTransition") {
                    let instruction: CancelConfidentialPolicyTransition =
                        json::from_value(payload).map_err(codec_error)?;
                    return Ok(Box::new(instruction).into_instruction_box());
                }
                if let Some(payload) = zk_map.remove("CreateElection") {
                    let instruction: CreateElection =
                        json::from_value(payload).map_err(codec_error)?;
                    return Ok(Box::new(instruction).into_instruction_box());
                }
                if let Some(payload) = zk_map.remove("SubmitBallot") {
                    let instruction: SubmitBallot =
                        json::from_value(payload).map_err(codec_error)?;
                    return Ok(Box::new(instruction).into_instruction_box());
                }
                if let Some(payload) = zk_map.remove("FinalizeElection") {
                    let instruction: FinalizeElection =
                        json::from_value(payload).map_err(codec_error)?;
                    return Ok(Box::new(instruction).into_instruction_box());
                }
                return Err(CodecError::new(
                    CodecErrorKind::InvalidArgument,
                    "unsupported zk instruction variant",
                ));
            }
            Err(CodecError::new(
                CodecErrorKind::InvalidArgument,
                "unsupported instruction; refer to Iroha data model instructions for supported variants",
            ))
        }
        _ => Err(CodecError::new(
            CodecErrorKind::InvalidArgument,
            "instruction JSON must be an object",
        )),
    }
}

fn settlement_instruction_from_json(value: json::Value) -> CodecResult<InstructionBox> {
    let json::Value::Object(mut variants) = value else {
        return Err(CodecError::new(
            CodecErrorKind::InvalidArgument,
            "Settlement instruction must be an object containing exactly one variant",
        ));
    };
    if variants.len() != 1 {
        return Err(CodecError::new(
            CodecErrorKind::InvalidArgument,
            "Settlement instruction must contain exactly one supported settlement variant",
        ));
    }
    let (variant, payload) = variants
        .pop_first()
        .expect("length checked settlement variant");
    let instruction = match variant.as_str() {
        "Dvp" => {
            SettlementInstructionBox::Dvp(json::from_value::<DvpIsi>(payload).map_err(codec_error)?)
        }
        "Pvp" => {
            SettlementInstructionBox::Pvp(json::from_value::<PvpIsi>(payload).map_err(codec_error)?)
        }
        "SetFxCorridorPolicy" => {
            let set = json::from_value::<SetFxCorridorPolicy>(payload).map_err(codec_error)?;
            if let Some(error) = set.policy.invariant_error() {
                return Err(CodecError::new(CodecErrorKind::InvalidArgument, error));
            }
            SettlementInstructionBox::SetFxCorridorPolicy(set)
        }
        "FundFxCorridorEscrow" => {
            let fund = json::from_value::<FundFxCorridorEscrow>(payload).map_err(codec_error)?;
            if fund.amount.is_zero() {
                return Err(CodecError::new(
                    CodecErrorKind::InvalidArgument,
                    "FundFxCorridorEscrow.amount must be positive",
                ));
            }
            SettlementInstructionBox::FundFxCorridorEscrow(fund)
        }
        "RefundFxCorridorEscrow" => {
            let refund =
                json::from_value::<RefundFxCorridorEscrow>(payload).map_err(codec_error)?;
            if refund.amount.is_zero() {
                return Err(CodecError::new(
                    CodecErrorKind::InvalidArgument,
                    "RefundFxCorridorEscrow.amount must be positive",
                ));
            }
            SettlementInstructionBox::RefundFxCorridorEscrow(refund)
        }
        "SettleFxCorridor" => {
            exact_json_object_fields(
                &payload,
                &[
                    "policy_id",
                    "expected_policy_revision",
                    "source_asset_definition_id",
                    "destination_asset_definition_id",
                    "settlement_id",
                    "recipient",
                    "source_amount",
                    "expected_destination_amount",
                    "oracle_evidence",
                ],
                "SettleFxCorridor",
            )?;
            let settle = json::from_value::<SettleFxCorridor>(payload).map_err(codec_error)?;
            if settle.source_amount.is_zero() {
                return Err(CodecError::new(
                    CodecErrorKind::InvalidArgument,
                    "SettleFxCorridor.source_amount must be positive",
                ));
            }
            SettlementInstructionBox::SettleFxCorridor(settle)
        }
        _ => {
            return Err(CodecError::new(
                CodecErrorKind::InvalidArgument,
                format!("unsupported Settlement instruction variant `{variant}`"),
            ));
        }
    };
    Ok(InstructionBox::from(instruction))
}

fn exact_json_object_fields(
    value: &json::Value,
    expected: &[&str],
    context: &str,
) -> CodecResult<()> {
    let json::Value::Object(fields) = value else {
        return Err(CodecError::new(
            CodecErrorKind::InvalidArgument,
            format!("{context} payload must be an object"),
        ));
    };
    let unknown = fields
        .keys()
        .filter(|key| !expected.contains(&key.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    if !unknown.is_empty() {
        return Err(CodecError::new(
            CodecErrorKind::InvalidArgument,
            format!(
                "{context} contains unexpected field(s): {}",
                unknown.join(", ")
            ),
        ));
    }
    let missing = expected
        .iter()
        .copied()
        .filter(|key| !fields.contains_key(*key))
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(CodecError::new(
            CodecErrorKind::InvalidArgument,
            format!("{context} is missing field(s): {}", missing.join(", ")),
        ));
    }
    Ok(())
}

/// Render a typed instruction through its canonical JavaScript JSON contract.
pub fn instruction_to_json_value(instruction: &InstructionBox) -> CodecResult<json::Value> {
    let value = instruction_to_json_value_inner(instruction)?;
    if let Some(reconstructed) = deployment_instruction_from_json(&value) {
        if norito::encode_canonical(&reconstructed?).map_err(codec_error)?
            != norito::encode_canonical(instruction).map_err(codec_error)?
        {
            return Err(CodecError::failure(
                "deployment instruction JSON changes canonical Norito bytes",
            ));
        }
    }
    Ok(value)
}

fn instruction_to_json_value_inner(instruction: &InstructionBox) -> CodecResult<json::Value> {
    if let Some(result) = typed_browser_instruction_to_json(instruction) {
        return result;
    }
    let instruction_ref: &dyn InstructionTrait = &**instruction;
    if let Some(limit) = instruction_ref
        .as_any()
        .downcast_ref::<iroha_data_model::isi::asset_transfer_control::SetAssetHoldingLimit>(
    ) {
        return Ok(norito::json!({
            "name": "SetAssetHoldingLimit",
            "params": {
                "account_id": (account_id_to_canonical_i105(&limit.account_id)?),
                "asset_definition_id": (limit.asset_definition_id.to_string()),
                "holding_limit": (limit.holding_limit.as_ref().map(|value| value.to_string())),
            },
        }));
    }
    if let Some(availability) = instruction_ref
        .as_any()
        .downcast_ref::<SetAssetTransferAvailability>()
    {
        let mut inner = json::Map::new();
        inner.insert(
            "account_id".to_owned(),
            json::to_value(&availability.account_id).map_err(codec_error)?,
        );
        inner.insert(
            "asset_definition_id".to_owned(),
            json::to_value(&availability.asset_definition_id).map_err(codec_error)?,
        );
        inner.insert(
            "expected_revision".to_owned(),
            json::Value::String(availability.expected_revision.to_string()),
        );
        inner.insert(
            "incoming".to_owned(),
            json::Value::String(
                match availability.incoming {
                    AssetTransferAvailability::Enabled => "Enabled",
                    AssetTransferAvailability::Disabled => "Disabled",
                }
                .to_owned(),
            ),
        );
        inner.insert(
            "outgoing".to_owned(),
            json::Value::String(
                match availability.outgoing {
                    AssetTransferAvailability::Enabled => "Enabled",
                    AssetTransferAvailability::Disabled => "Disabled",
                }
                .to_owned(),
            ),
        );
        inner.insert(
            "reason".to_owned(),
            availability
                .reason
                .as_ref()
                .map_or(json::Value::Null, |value| {
                    json::Value::String(value.clone())
                }),
        );
        let mut outer = json::Map::new();
        outer.insert(
            "SetAssetTransferAvailability".to_owned(),
            json::Value::Object(inner),
        );
        return Ok(json::Value::Object(outer));
    }
    if let Some(blacklist) = instruction_ref
        .as_any()
        .downcast_ref::<SetAssetTransferBlacklist>()
    {
        let mut inner = json::Map::new();
        inner.insert(
            "account_id".to_owned(),
            json::to_value(&blacklist.account_id).map_err(codec_error)?,
        );
        inner.insert(
            "asset_definition_id".to_owned(),
            json::to_value(&blacklist.asset_definition_id).map_err(codec_error)?,
        );
        inner.insert(
            "blacklisted".to_owned(),
            json::Value::Bool(blacklist.blacklisted),
        );
        let mut outer = json::Map::new();
        outer.insert(
            "SetAssetTransferBlacklist".to_owned(),
            json::Value::Object(inner),
        );
        return Ok(json::Value::Object(outer));
    }
    if let Some(control) = instruction_ref
        .as_any()
        .downcast_ref::<SetAssetTransferControl>()
    {
        let mut inner = json::Map::new();
        inner.insert(
            "account_id".to_owned(),
            json::to_value(&control.account_id).map_err(codec_error)?,
        );
        inner.insert(
            "asset_definition_id".to_owned(),
            json::to_value(&control.asset_definition_id).map_err(codec_error)?,
        );
        inner.insert(
            "limits".to_owned(),
            json::Value::Array(
                control
                    .limits
                    .iter()
                    .map(|limit| {
                        let mut fields = json::Map::new();
                        fields.insert(
                            "window".to_owned(),
                            json::Value::String(
                                match limit.window {
                                    AssetTransferControlWindow::Day => "Day",
                                    AssetTransferControlWindow::Week => "Week",
                                    AssetTransferControlWindow::Month => "Month",
                                }
                                .to_owned(),
                            ),
                        );
                        fields.insert(
                            "cap_amount".to_owned(),
                            limit
                                .cap_amount
                                .as_ref()
                                .map_or(json::Value::Null, |value| {
                                    json::Value::String(value.to_string())
                                }),
                        );
                        json::Value::Object(fields)
                    })
                    .collect(),
            ),
        );
        let mut outer = json::Map::new();
        outer.insert(
            "SetAssetTransferControl".to_owned(),
            json::Value::Object(inner),
        );
        return Ok(json::Value::Object(outer));
    }
    if let Some(cancel) = instruction_ref.as_any().downcast_ref::<CancelAssetLock>() {
        if cancel.expected_remaining_amount.is_zero() {
            return Err(CodecError::new(
                CodecErrorKind::InvalidArgument,
                "CancelAssetLock.expected_remaining_amount must be positive",
            ));
        }
        let mut inner = json::Map::new();
        inner.insert(
            "escrow_id".to_owned(),
            json::to_value(&cancel.escrow_id).map_err(codec_error)?,
        );
        inner.insert(
            "expected_remaining_amount".to_owned(),
            json::Value::String(cancel.expected_remaining_amount.to_string()),
        );
        let mut outer = json::Map::new();
        outer.insert("CancelAssetLock".to_owned(), json::Value::Object(inner));
        return Ok(json::Value::Object(outer));
    }
    if let Some(deploy) = instruction_ref
        .as_any()
        .downcast_ref::<iroha_data_model::isi::soracloud::DeploySoracloudService>(
    ) {
        let mut outer = json::Map::new();
        outer.insert(
            "DeploySoracloudService".to_owned(),
            json::to_value(deploy).map_err(codec_error)?,
        );
        return Ok(json::Value::Object(outer));
    }
    if let Some(deploy) = instruction_ref
        .as_any()
        .downcast_ref::<iroha_data_model::isi::soracloud::DeploySoracloudAgentApartment>(
    ) {
        let mut outer = json::Map::new();
        outer.insert(
            "DeploySoracloudAgentApartment".to_owned(),
            json::to_value(deploy).map_err(codec_error)?,
        );
        return Ok(json::Value::Object(outer));
    }
    if let Some(join) = instruction_ref
        .as_any()
        .downcast_ref::<iroha_data_model::isi::soracloud::JoinSoracloudHfSharedLease>(
    ) {
        let mut outer = json::Map::new();
        outer.insert(
            "JoinSoracloudHfSharedLease".to_owned(),
            json::to_value(join).map_err(codec_error)?,
        );
        return Ok(json::Value::Object(outer));
    }
    if let Some(settlement) = instruction_ref
        .as_any()
        .downcast_ref::<SettlementInstructionBox>()
    {
        let (variant, payload) = match settlement {
            SettlementInstructionBox::Dvp(value) => {
                ("Dvp", json::to_value(value).map_err(codec_error)?)
            }
            SettlementInstructionBox::Pvp(value) => {
                ("Pvp", json::to_value(value).map_err(codec_error)?)
            }
            SettlementInstructionBox::SetFxCorridorPolicy(value) => (
                "SetFxCorridorPolicy",
                json::to_value(value).map_err(codec_error)?,
            ),
            SettlementInstructionBox::FundFxCorridorEscrow(value) => (
                "FundFxCorridorEscrow",
                json::to_value(value).map_err(codec_error)?,
            ),
            SettlementInstructionBox::RefundFxCorridorEscrow(value) => (
                "RefundFxCorridorEscrow",
                json::to_value(value).map_err(codec_error)?,
            ),
            SettlementInstructionBox::SettleFxCorridor(value) => (
                "SettleFxCorridor",
                json::to_value(value).map_err(codec_error)?,
            ),
        };
        let mut variants = json::Map::new();
        variants.insert(variant.to_owned(), payload);
        let mut outer = json::Map::new();
        outer.insert("Settlement".to_owned(), json::Value::Object(variants));
        return Ok(json::Value::Object(outer));
    }
    if let Some(register_box) = instruction_ref.as_any().downcast_ref::<RegisterBox>() {
        let mut register_map = json::Map::new();
        match register_box {
            RegisterBox::Domain(register) => {
                let inner = json::to_value(register.object()).map_err(codec_error)?;
                register_map.insert("Domain".to_owned(), inner);
            }
            RegisterBox::Account(register) => {
                let inner = json::to_value(register.object()).map_err(codec_error)?;
                register_map.insert("Account".to_owned(), inner);
            }
            RegisterBox::AssetDefinition(register) => {
                let inner = json::to_value(register.object()).map_err(codec_error)?;
                register_map.insert("AssetDefinition".to_owned(), inner);
            }
            RegisterBox::Nft(register) => {
                let inner = json::to_value(register.object()).map_err(codec_error)?;
                register_map.insert("Nft".to_owned(), inner);
            }
            RegisterBox::Role(register) => {
                let inner = json::to_value(register.object()).map_err(codec_error)?;
                register_map.insert("Role".to_owned(), inner);
            }
            RegisterBox::Trigger(register) => {
                let trigger = register.object();
                let mut inner = json::Map::new();
                inner.insert(
                    "id".to_owned(),
                    json::to_value(trigger.id()).map_err(codec_error)?,
                );
                inner.insert(
                    "action".to_owned(),
                    json::to_value(trigger.action()).map_err(codec_error)?,
                );
                register_map.insert("Trigger".to_owned(), json::Value::Object(inner));
            }
            RegisterBox::Peer(register) => {
                let inner = json::to_value(register).map_err(codec_error)?;
                register_map.insert("Peer".to_owned(), inner);
            }
        }
        if !register_map.is_empty() {
            let mut outer = json::Map::new();
            outer.insert("Register".to_owned(), json::Value::Object(register_map));
            return Ok(json::Value::Object(outer));
        }
    }
    if let Some(unregister_box) = instruction_ref.as_any().downcast_ref::<UnregisterBox>() {
        let mut unregister_map = json::Map::new();
        match unregister_box {
            UnregisterBox::Peer(unregister) => {
                let inner = json::to_value(&unregister.object).map_err(codec_error)?;
                unregister_map.insert("Peer".to_owned(), inner);
            }
            UnregisterBox::Domain(unregister) => {
                let inner = json::to_value(&unregister.object).map_err(codec_error)?;
                unregister_map.insert("Domain".to_owned(), inner);
            }
            UnregisterBox::Account(unregister) => {
                let inner = json::to_value(&unregister.object).map_err(codec_error)?;
                unregister_map.insert("Account".to_owned(), inner);
            }
            UnregisterBox::AssetDefinition(unregister) => {
                let inner = json::to_value(&unregister.object).map_err(codec_error)?;
                unregister_map.insert("AssetDefinition".to_owned(), inner);
            }
            UnregisterBox::Nft(unregister) => {
                let inner = json::to_value(&unregister.object).map_err(codec_error)?;
                unregister_map.insert("Nft".to_owned(), inner);
            }
            UnregisterBox::Role(unregister) => {
                let inner = json::to_value(&unregister.object).map_err(codec_error)?;
                unregister_map.insert("Role".to_owned(), inner);
            }
            UnregisterBox::Trigger(unregister) => {
                let inner = json::to_value(&unregister.object).map_err(codec_error)?;
                unregister_map.insert("Trigger".to_owned(), inner);
            }
        }
        if !unregister_map.is_empty() {
            let mut outer = json::Map::new();
            outer.insert("Unregister".to_owned(), json::Value::Object(unregister_map));
            return Ok(json::Value::Object(outer));
        }
    }
    if let Some(mint_box) = instruction_ref.as_any().downcast_ref::<MintBox>() {
        let mut mint_map = json::Map::new();
        if let MintBox::Asset(mint) = mint_box {
            let mut asset_fields = json::Map::new();
            let object = json::to_value(mint.object()).map_err(codec_error)?;
            let destination = json::Value::String(mint.destination().canonical_literal());
            asset_fields.insert("object".to_owned(), object);
            asset_fields.insert("destination".to_owned(), destination);
            mint_map.insert("Asset".to_owned(), json::Value::Object(asset_fields));
        }
        if let MintBox::TriggerRepetitions(mint) = mint_box {
            let mut trigger_fields = json::Map::new();
            let repetitions = json::to_value(mint.object()).map_err(codec_error)?;
            let destination = json::to_value(mint.destination()).map_err(codec_error)?;
            trigger_fields.insert("object".to_owned(), repetitions);
            trigger_fields.insert("destination".to_owned(), destination);
            mint_map.insert(
                "TriggerRepetitions".to_owned(),
                json::Value::Object(trigger_fields),
            );
        }
        if !mint_map.is_empty() {
            let mut outer = json::Map::new();
            outer.insert("Mint".to_owned(), json::Value::Object(mint_map));
            return Ok(json::Value::Object(outer));
        }
    }
    if let Some(transfer_box) = instruction_ref.as_any().downcast_ref::<TransferBox>() {
        let mut transfer_map = json::Map::new();
        if let TransferBox::Asset(transfer) = transfer_box {
            let mut asset_fields = json::Map::new();
            let source = json::Value::String(transfer.source().canonical_literal());
            let quantity = json::to_value(transfer.object()).map_err(codec_error)?;
            let destination = json::to_value(transfer.destination()).map_err(codec_error)?;
            asset_fields.insert("source".to_owned(), source);
            asset_fields.insert("object".to_owned(), quantity);
            asset_fields.insert("destination".to_owned(), destination);
            transfer_map.insert("Asset".to_owned(), json::Value::Object(asset_fields));
        }
        if let TransferBox::Domain(transfer) = transfer_box {
            let mut domain_fields = json::Map::new();
            let source = json::to_value(transfer.source()).map_err(codec_error)?;
            let object = json::to_value(transfer.object()).map_err(codec_error)?;
            let destination = json::to_value(transfer.destination()).map_err(codec_error)?;
            domain_fields.insert("source".to_owned(), source);
            domain_fields.insert("object".to_owned(), object);
            domain_fields.insert("destination".to_owned(), destination);
            transfer_map.insert("Domain".to_owned(), json::Value::Object(domain_fields));
        }
        if let TransferBox::AssetDefinition(transfer) = transfer_box {
            let mut definition_fields = json::Map::new();
            let source = json::to_value(transfer.source()).map_err(codec_error)?;
            let object = json::to_value(transfer.object()).map_err(codec_error)?;
            let destination = json::to_value(transfer.destination()).map_err(codec_error)?;
            definition_fields.insert("source".to_owned(), source);
            definition_fields.insert("object".to_owned(), object);
            definition_fields.insert("destination".to_owned(), destination);
            transfer_map.insert(
                "AssetDefinition".to_owned(),
                json::Value::Object(definition_fields),
            );
        }
        if let TransferBox::Nft(transfer) = transfer_box {
            let mut nft_fields = json::Map::new();
            let source = json::to_value(transfer.source()).map_err(codec_error)?;
            let object = json::to_value(transfer.object()).map_err(codec_error)?;
            let destination = json::to_value(transfer.destination()).map_err(codec_error)?;
            nft_fields.insert("source".to_owned(), source);
            nft_fields.insert("object".to_owned(), object);
            nft_fields.insert("destination".to_owned(), destination);
            transfer_map.insert("Nft".to_owned(), json::Value::Object(nft_fields));
        }
        if !transfer_map.is_empty() {
            let mut outer = json::Map::new();
            outer.insert("Transfer".to_owned(), json::Value::Object(transfer_map));
            return Ok(json::Value::Object(outer));
        }
    }
    if let Some(batch) = instruction_ref
        .as_any()
        .downcast_ref::<TransferAssetBatch>()
    {
        let mut outer = json::Map::new();
        outer.insert(
            "TransferAssetBatch".to_owned(),
            json::to_value(batch).map_err(codec_error)?,
        );
        return Ok(json::Value::Object(outer));
    }
    if let Some(burn_box) = instruction_ref.as_any().downcast_ref::<BurnBox>() {
        let mut burn_map = json::Map::new();
        if let BurnBox::Asset(burn) = burn_box {
            let mut asset_fields = json::Map::new();
            let object = json::to_value(burn.object()).map_err(codec_error)?;
            let destination = json::Value::String(burn.destination().canonical_literal());
            asset_fields.insert("object".to_owned(), object);
            asset_fields.insert("destination".to_owned(), destination);
            burn_map.insert("Asset".to_owned(), json::Value::Object(asset_fields));
        }
        if let BurnBox::TriggerRepetitions(burn) = burn_box {
            let mut trigger_fields = json::Map::new();
            let repetitions = json::to_value(burn.object()).map_err(codec_error)?;
            let destination = json::to_value(burn.destination()).map_err(codec_error)?;
            trigger_fields.insert("object".to_owned(), repetitions);
            trigger_fields.insert("destination".to_owned(), destination);
            burn_map.insert(
                "TriggerRepetitions".to_owned(),
                json::Value::Object(trigger_fields),
            );
        }
        if !burn_map.is_empty() {
            let mut outer = json::Map::new();
            outer.insert("Burn".to_owned(), json::Value::Object(burn_map));
            return Ok(json::Value::Object(outer));
        }
    }
    if let Some(grant_box) = instruction_ref.as_any().downcast_ref::<GrantBox>() {
        if let GrantBox::Permission(grant) = grant_box {
            let mut fields = json::Map::new();
            fields.insert(
                "object".to_owned(),
                json::to_value(grant.object()).map_err(codec_error)?,
            );
            fields.insert(
                "destination".to_owned(),
                json::to_value(grant.destination()).map_err(codec_error)?,
            );
            let mut grant_map = json::Map::new();
            grant_map.insert("Permission".to_owned(), json::Value::Object(fields));
            let mut outer = json::Map::new();
            outer.insert("Grant".to_owned(), json::Value::Object(grant_map));
            return Ok(json::Value::Object(outer));
        }
    }
    if let Some(set_key_value) = instruction_ref.as_any().downcast_ref::<SetKeyValueBox>() {
        if let SetKeyValueBox::Account(set) = set_key_value {
            let mut fields = json::Map::new();
            fields.insert(
                "object".to_owned(),
                json::Value::String(account_id_to_canonical_i105(set.object())?),
            );
            fields.insert(
                "key".to_owned(),
                json::to_value(set.key()).map_err(codec_error)?,
            );
            fields.insert(
                "value".to_owned(),
                json::to_value(set.value()).map_err(codec_error)?,
            );
            let mut variants = json::Map::new();
            variants.insert("Account".to_owned(), json::Value::Object(fields));
            let mut outer = json::Map::new();
            outer.insert("SetKeyValue".to_owned(), json::Value::Object(variants));
            return Ok(json::Value::Object(outer));
        }
        if let SetKeyValueBox::Nft(set) = set_key_value {
            let mut fields = json::Map::new();
            fields.insert(
                "object".to_owned(),
                json::to_value(set.object()).map_err(codec_error)?,
            );
            fields.insert(
                "key".to_owned(),
                json::to_value(set.key()).map_err(codec_error)?,
            );
            fields.insert(
                "value".to_owned(),
                json::to_value(set.value()).map_err(codec_error)?,
            );
            let mut variants = json::Map::new();
            variants.insert("Nft".to_owned(), json::Value::Object(fields));
            let mut outer = json::Map::new();
            outer.insert("SetKeyValue".to_owned(), json::Value::Object(variants));
            return Ok(json::Value::Object(outer));
        }
    }
    if let Some(alias) = instruction_ref
        .as_any()
        .downcast_ref::<SetAssetDefinitionAlias>()
    {
        let mut fields = json::Map::new();
        fields.insert(
            "asset_definition_id".to_owned(),
            json::Value::String(alias.asset_definition_id().to_string()),
        );
        fields.insert(
            "alias".to_owned(),
            alias.alias().as_ref().map_or(json::Value::Null, |value| {
                json::Value::String(value.to_string())
            }),
        );
        fields.insert(
            "lease_expiry_ms".to_owned(),
            alias
                .lease_expiry_ms()
                .as_ref()
                .map_or(json::Value::Null, |value| {
                    json::Value::Number(json::Number::from(*value))
                }),
        );
        let mut outer = json::Map::new();
        outer.insert(
            "SetAssetDefinitionAlias".to_owned(),
            json::Value::Object(fields),
        );
        return Ok(json::Value::Object(outer));
    }
    if let Some(execute_trigger) = instruction_ref.as_any().downcast_ref::<ExecuteTrigger>() {
        let mut payload = json::Map::new();
        payload.insert(
            "trigger".to_owned(),
            json::to_value(execute_trigger.trigger()).map_err(codec_error)?,
        );
        let args = json::parse_value(execute_trigger.args().get()).map_err(|error| {
            CodecError::new(
                CodecErrorKind::InvalidArgument,
                format!("ExecuteTrigger.args is not valid JSON: {error}"),
            )
        })?;
        payload.insert("args".to_owned(), args);
        let mut outer = json::Map::new();
        outer.insert("ExecuteTrigger".to_owned(), json::Value::Object(payload));
        return Ok(json::Value::Object(outer));
    }
    if let Some(rwa_box) = instruction_ref.as_any().downcast_ref::<RwaInstructionBox>() {
        let (label, payload) = match rwa_box {
            RwaInstructionBox::Register(register) => (
                "RegisterRwa",
                norito_json!({ "rwa": new_rwa_to_json(register.rwa())? }),
            ),
            RwaInstructionBox::Transfer(transfer) => (
                "TransferRwa",
                norito_json!({
                    "source": account_id_to_canonical_i105(transfer.source())?,
                    "rwa": transfer.rwa().to_string(),
                    "quantity": transfer.quantity(),
                    "destination": account_id_to_canonical_i105(transfer.destination())?,
                }),
            ),
            RwaInstructionBox::Merge(merge) => {
                let mut payload = json::Map::new();
                payload.insert(
                    "parents".to_owned(),
                    rwa_parent_refs_to_json(merge.parents()),
                );
                payload.insert(
                    "primary_reference".to_owned(),
                    json::Value::String(merge.primary_reference().clone()),
                );
                payload.insert(
                    "status".to_owned(),
                    rwa_status_to_json(merge.status().as_ref()),
                );
                payload.insert(
                    "metadata".to_owned(),
                    json::to_value(merge.metadata()).map_err(codec_error)?,
                );
                ("MergeRwas", json::Value::Object(payload))
            }
            RwaInstructionBox::Redeem(redeem) => (
                "RedeemRwa",
                norito_json!({
                    "rwa": redeem.rwa().to_string(),
                    "quantity": redeem.quantity(),
                }),
            ),
            RwaInstructionBox::Freeze(freeze) => (
                "FreezeRwa",
                norito_json!({ "rwa": freeze.rwa().to_string() }),
            ),
            RwaInstructionBox::Unfreeze(unfreeze) => (
                "UnfreezeRwa",
                norito_json!({ "rwa": unfreeze.rwa().to_string() }),
            ),
            RwaInstructionBox::Hold(hold) => (
                "HoldRwa",
                norito_json!({
                    "rwa": hold.rwa().to_string(),
                    "quantity": hold.quantity(),
                }),
            ),
            RwaInstructionBox::Release(release) => (
                "ReleaseRwa",
                norito_json!({
                    "rwa": release.rwa().to_string(),
                    "quantity": release.quantity(),
                }),
            ),
            RwaInstructionBox::ForceTransfer(force_transfer) => (
                "ForceTransferRwa",
                norito_json!({
                    "rwa": force_transfer.rwa().to_string(),
                    "quantity": force_transfer.quantity(),
                    "destination": account_id_to_canonical_i105(force_transfer.destination())?,
                }),
            ),
            RwaInstructionBox::SetControls(set_controls) => (
                "SetRwaControls",
                norito_json!({
                    "rwa": set_controls.rwa().to_string(),
                    "controls": rwa_control_policy_to_json(set_controls.controls())?,
                }),
            ),
            RwaInstructionBox::SetKeyValue(set) => (
                "SetRwaKeyValue",
                norito_json!({
                    "rwa": set.object().to_string(),
                    "key": set.key().clone(),
                    "value": json::to_value(set.value()).map_err(codec_error)?,
                }),
            ),
            RwaInstructionBox::RemoveKeyValue(remove) => (
                "RemoveRwaKeyValue",
                norito_json!({
                    "rwa": remove.object().to_string(),
                    "key": remove.key().clone(),
                }),
            ),
        };
        let mut outer = json::Map::new();
        outer.insert(label.to_owned(), payload);
        return Ok(json::Value::Object(outer));
    }
    if let Some(custom_instruction) = instruction_ref.as_any().downcast_ref::<CustomInstruction>() {
        let payload_json =
            json::parse_value(custom_instruction.payload.get()).map_err(|error| {
                CodecError::new(
                    CodecErrorKind::InvalidArgument,
                    format!("Custom.payload is not valid JSON: {error}"),
                )
            })?;
        return Ok(custom_json_value(payload_json));
    }
    if let Some(register) = instruction_ref.as_any().downcast_ref::<RegisterRwa>() {
        let mut outer = json::Map::new();
        outer.insert(
            "RegisterRwa".to_owned(),
            norito_json!({ "rwa": new_rwa_to_json(register.rwa())? }),
        );
        return Ok(json::Value::Object(outer));
    }
    if let Some(transfer) = instruction_ref.as_any().downcast_ref::<TransferRwa>() {
        let mut outer = json::Map::new();
        outer.insert(
            "TransferRwa".to_owned(),
            norito_json!({
                "source": account_id_to_canonical_i105(transfer.source())?,
                "rwa": transfer.rwa().to_string(),
                "quantity": transfer.quantity(),
                "destination": account_id_to_canonical_i105(transfer.destination())?,
            }),
        );
        return Ok(json::Value::Object(outer));
    }
    if let Some(merge) = instruction_ref.as_any().downcast_ref::<MergeRwas>() {
        let mut payload = json::Map::new();
        payload.insert(
            "parents".to_owned(),
            rwa_parent_refs_to_json(merge.parents()),
        );
        payload.insert(
            "primary_reference".to_owned(),
            json::Value::String(merge.primary_reference().clone()),
        );
        payload.insert(
            "status".to_owned(),
            rwa_status_to_json(merge.status().as_ref()),
        );
        payload.insert(
            "metadata".to_owned(),
            json::to_value(merge.metadata()).map_err(codec_error)?,
        );
        let mut outer = json::Map::new();
        outer.insert("MergeRwas".to_owned(), json::Value::Object(payload));
        return Ok(json::Value::Object(outer));
    }
    if let Some(redeem) = instruction_ref.as_any().downcast_ref::<RedeemRwa>() {
        let mut outer = json::Map::new();
        outer.insert(
            "RedeemRwa".to_owned(),
            norito_json!({
                "rwa": redeem.rwa().to_string(),
                "quantity": redeem.quantity(),
            }),
        );
        return Ok(json::Value::Object(outer));
    }
    if let Some(freeze) = instruction_ref.as_any().downcast_ref::<FreezeRwa>() {
        let mut outer = json::Map::new();
        outer.insert(
            "FreezeRwa".to_owned(),
            norito_json!({ "rwa": freeze.rwa().to_string() }),
        );
        return Ok(json::Value::Object(outer));
    }
    if let Some(unfreeze) = instruction_ref.as_any().downcast_ref::<UnfreezeRwa>() {
        let mut outer = json::Map::new();
        outer.insert(
            "UnfreezeRwa".to_owned(),
            norito_json!({ "rwa": unfreeze.rwa().to_string() }),
        );
        return Ok(json::Value::Object(outer));
    }
    if let Some(hold) = instruction_ref.as_any().downcast_ref::<HoldRwa>() {
        let mut outer = json::Map::new();
        outer.insert(
            "HoldRwa".to_owned(),
            norito_json!({
                "rwa": hold.rwa().to_string(),
                "quantity": hold.quantity(),
            }),
        );
        return Ok(json::Value::Object(outer));
    }
    if let Some(release) = instruction_ref.as_any().downcast_ref::<ReleaseRwa>() {
        let mut outer = json::Map::new();
        outer.insert(
            "ReleaseRwa".to_owned(),
            norito_json!({
                "rwa": release.rwa().to_string(),
                "quantity": release.quantity(),
            }),
        );
        return Ok(json::Value::Object(outer));
    }
    if let Some(force_transfer) = instruction_ref.as_any().downcast_ref::<ForceTransferRwa>() {
        let mut outer = json::Map::new();
        outer.insert(
            "ForceTransferRwa".to_owned(),
            norito_json!({
                "rwa": force_transfer.rwa().to_string(),
                "quantity": force_transfer.quantity(),
                "destination": account_id_to_canonical_i105(force_transfer.destination())?,
            }),
        );
        return Ok(json::Value::Object(outer));
    }
    if let Some(set_controls) = instruction_ref.as_any().downcast_ref::<SetRwaControls>() {
        let mut outer = json::Map::new();
        outer.insert(
            "SetRwaControls".to_owned(),
            norito_json!({
                "rwa": set_controls.rwa().to_string(),
                "controls": rwa_control_policy_to_json(set_controls.controls())?,
            }),
        );
        return Ok(json::Value::Object(outer));
    }
    if let Some(submit) = instruction_ref
        .as_any()
        .downcast_ref::<SubmitAgendaProposal>()
    {
        let mut outer = json::Map::new();
        outer.insert(
            "SubmitAgendaProposal".to_owned(),
            norito_json!({
                "proposal": submit.proposal,
            }),
        );
        return Ok(json::Value::Object(outer));
    }
    if let Some(propose) = instruction_ref
        .as_any()
        .downcast_ref::<ProposeValidationFeePolicy>()
    {
        let mut inner = json::Map::new();
        inner.insert(
            "policy".to_owned(),
            json::to_value(&propose.policy).map_err(codec_error)?,
        );
        inner.insert(
            "payout_lifecycle_proposal_id".to_owned(),
            json::to_value(&propose.payout_lifecycle_proposal_id).map_err(codec_error)?,
        );
        let mut outer = json::Map::new();
        outer.insert(
            "ProposeValidationFeePolicy".to_owned(),
            json::Value::Object(inner),
        );
        return Ok(json::Value::Object(outer));
    }
    if let Some(propose) = instruction_ref
        .as_any()
        .downcast_ref::<ProposeDeployContract>()
    {
        let mut inner = json::Map::new();
        inner.insert(
            "contract_address".to_owned(),
            json::Value::String(propose.contract_address.to_string()),
        );
        inner.insert(
            "code_hash".to_owned(),
            json::to_value(&propose.code_hash).map_err(codec_error)?,
        );
        inner.insert(
            "abi_hash".to_owned(),
            json::to_value(&propose.abi_hash).map_err(codec_error)?,
        );
        inner.insert(
            "abi_version".to_owned(),
            json::to_value(&propose.abi_version).map_err(codec_error)?,
        );
        if let Some(manifest_provenance) = &propose.manifest_provenance {
            inner.insert(
                "manifest_provenance".to_owned(),
                json::to_value(manifest_provenance).map_err(codec_error)?,
            );
        }
        let mut outer = json::Map::new();
        outer.insert(
            "ProposeDeployContract".to_owned(),
            json::Value::Object(inner),
        );
        return Ok(json::Value::Object(outer));
    }
    if let Some(ballot) = instruction_ref.as_any().downcast_ref::<CastZkBallot>() {
        let mut inner = json::Map::new();
        inner.insert(
            "election_id".to_owned(),
            json::Value::String(ballot.election_id.clone()),
        );
        inner.insert(
            "proof_b64".to_owned(),
            json::Value::String(ballot.proof_b64.clone()),
        );
        inner.insert(
            "public_inputs_json".to_owned(),
            json::Value::String(ballot.public_inputs_json.clone()),
        );
        let mut outer = json::Map::new();
        outer.insert("CastZkBallot".to_owned(), json::Value::Object(inner));
        return Ok(json::Value::Object(outer));
    }
    if let Some(ballot) = instruction_ref.as_any().downcast_ref::<CastPlainBallot>() {
        let mut inner = json::Map::new();
        inner.insert(
            "referendum_id".to_owned(),
            json::Value::String(ballot.referendum_id.clone()),
        );
        inner.insert(
            "owner".to_owned(),
            json::to_value(&ballot.owner).map_err(codec_error)?,
        );
        inner.insert(
            "amount".to_owned(),
            json::Value::String(ballot.amount.to_string()),
        );
        inner.insert(
            "duration_blocks".to_owned(),
            json::to_value(&ballot.duration_blocks).map_err(codec_error)?,
        );
        inner.insert(
            "direction".to_owned(),
            json::to_value(&ballot.direction).map_err(codec_error)?,
        );
        let mut outer = json::Map::new();
        outer.insert("CastPlainBallot".to_owned(), json::Value::Object(inner));
        return Ok(json::Value::Object(outer));
    }
    if let Some(citizen) = instruction_ref.as_any().downcast_ref::<RegisterCitizen>() {
        let mut inner = json::Map::new();
        inner.insert(
            "owner".to_owned(),
            json::to_value(&citizen.owner).map_err(codec_error)?,
        );
        inner.insert(
            "amount".to_owned(),
            json::Value::String(citizen.amount.to_string()),
        );
        let mut outer = json::Map::new();
        outer.insert("RegisterCitizen".to_owned(), json::Value::Object(inner));
        return Ok(json::Value::Object(outer));
    }
    if let Some(register) = instruction_ref.as_any().downcast_ref::<RegisterZkAsset>() {
        return Ok(zk_json_value(
            "RegisterZkAsset",
            json::to_value(register).map_err(codec_error)?,
        ));
    }
    if let Some(transition) = instruction_ref
        .as_any()
        .downcast_ref::<ScheduleConfidentialPolicyTransition>()
    {
        return Ok(zk_json_value(
            "ScheduleConfidentialPolicyTransition",
            json::to_value(transition).map_err(codec_error)?,
        ));
    }
    if let Some(cancel) = instruction_ref
        .as_any()
        .downcast_ref::<CancelConfidentialPolicyTransition>()
    {
        return Ok(zk_json_value(
            "CancelConfidentialPolicyTransition",
            json::to_value(cancel).map_err(codec_error)?,
        ));
    }
    if let Some(create) = instruction_ref.as_any().downcast_ref::<CreateElection>() {
        return Ok(zk_json_value(
            "CreateElection",
            json::to_value(create).map_err(codec_error)?,
        ));
    }
    if let Some(submit) = instruction_ref.as_any().downcast_ref::<SubmitBallot>() {
        return Ok(zk_json_value(
            "SubmitBallot",
            json::to_value(submit).map_err(codec_error)?,
        ));
    }
    if let Some(finalize) = instruction_ref.as_any().downcast_ref::<FinalizeElection>() {
        return Ok(zk_json_value(
            "FinalizeElection",
            json::to_value(finalize).map_err(codec_error)?,
        ));
    }
    if let Some(parameter) = instruction_ref.as_any().downcast_ref::<SetParameter>() {
        return Ok(instruction_envelope(
            "SetParameter",
            json::to_value(&parameter.0).map_err(codec_error)?,
        ));
    }
    if let Some(upload) = instruction_ref
        .as_any()
        .downcast_ref::<UploadSmartContractCodeChunk>()
    {
        return Ok(instruction_envelope(
            "UploadSmartContractCodeChunk",
            norito::json!({
                "code_hash": (json::to_value(&upload.code_hash).map_err(codec_error)?),
                "total_size": (upload.total_size.to_string()),
                "chunk_index": (upload.chunk_index),
                "chunk_count": (upload.chunk_count),
                "chunk": (STANDARD.encode(&upload.chunk)),
            }),
        ));
    }
    if let Some(finalize) = instruction_ref
        .as_any()
        .downcast_ref::<FinalizeSmartContractCodeUpload>()
    {
        return Ok(instruction_envelope(
            "FinalizeSmartContractCodeUpload",
            norito::json!({
                "code_hash": (json::to_value(&finalize.code_hash).map_err(codec_error)?),
                "total_size": (finalize.total_size.to_string()),
                "chunk_count": (finalize.chunk_count),
            }),
        ));
    }
    if let Some(cancel) = instruction_ref
        .as_any()
        .downcast_ref::<CancelSmartContractCodeUpload>()
    {
        return Ok(instruction_envelope(
            "CancelSmartContractCodeUpload",
            norito::json!({
                "code_hash": (json::to_value(&cancel.code_hash).map_err(codec_error)?),
            }),
        ));
    }
    if let Some(commit) = instruction_ref
        .as_any()
        .downcast_ref::<CommitContractDeployment>()
    {
        return Ok(instruction_envelope(
            "CommitContractDeployment",
            norito::json!({
                "expected_deploy_nonce": (commit.expected_deploy_nonce.to_string()),
                "contract_address": (commit.contract_address.to_string()),
                "code_hash": (json::to_value(&commit.code_hash).map_err(codec_error)?),
                "contract_alias": (commit.contract_alias.to_string()),
                "lease_expiry_ms": (commit.lease_expiry_ms.map(|value| value.to_string())),
                "expected_previous_contract_address": (commit.expected_previous_contract_address.as_ref().map(|value| value.to_string())),
            }),
        ));
    }
    if let Some(register_code) = instruction_ref
        .as_any()
        .downcast_ref::<RegisterSmartContractCode>()
    {
        let manifest_value = json::to_value(&register_code.manifest).map_err(codec_error)?;
        let mut inner = json::Map::new();
        inner.insert("manifest".to_owned(), manifest_value);
        let mut outer = json::Map::new();
        outer.insert(
            "RegisterSmartContractCode".to_owned(),
            json::Value::Object(inner),
        );
        return Ok(json::Value::Object(outer));
    }
    if let Some(register_bytes) = instruction_ref
        .as_any()
        .downcast_ref::<RegisterSmartContractBytes>()
    {
        let mut inner = json::Map::new();
        inner.insert(
            "code_hash".to_owned(),
            json::to_value(&register_bytes.code_hash).map_err(codec_error)?,
        );
        inner.insert(
            "code".to_owned(),
            json::Value::String(STANDARD.encode(&register_bytes.code)),
        );
        let mut outer = json::Map::new();
        outer.insert(
            "RegisterSmartContractBytes".to_owned(),
            json::Value::Object(inner),
        );
        return Ok(json::Value::Object(outer));
    }
    if let Some(remove_bytes) = instruction_ref
        .as_any()
        .downcast_ref::<RemoveSmartContractBytes>()
    {
        let mut inner = json::Map::new();
        inner.insert(
            "code_hash".to_owned(),
            json::to_value(&remove_bytes.code_hash).map_err(codec_error)?,
        );
        if let Some(reason) = &remove_bytes.reason {
            inner.insert("reason".to_owned(), json::Value::String(reason.clone()));
        }
        let mut outer = json::Map::new();
        outer.insert(
            "RemoveSmartContractBytes".to_owned(),
            json::Value::Object(inner),
        );
        return Ok(json::Value::Object(outer));
    }
    if let Some(activate) = instruction_ref
        .as_any()
        .downcast_ref::<ActivateContractInstance>()
    {
        let mut inner = json::Map::new();
        inner.insert(
            "contract_address".to_owned(),
            json::Value::String(activate.contract_address.to_string()),
        );
        inner.insert(
            "expected_revision".to_owned(),
            json::Value::String(activate.expected_revision.to_string()),
        );
        inner.insert(
            "code_hash".to_owned(),
            json::to_value(&activate.code_hash).map_err(codec_error)?,
        );
        let mut outer = json::Map::new();
        outer.insert(
            "ActivateContractInstance".to_owned(),
            json::Value::Object(inner),
        );
        return Ok(json::Value::Object(outer));
    }
    if let Some(deactivate) = instruction_ref
        .as_any()
        .downcast_ref::<DeactivateContractInstance>()
    {
        let mut inner = json::Map::new();
        inner.insert(
            "contract_address".to_owned(),
            json::Value::String(deactivate.contract_address.to_string()),
        );
        inner.insert(
            "expected_revision".to_owned(),
            json::Value::String(deactivate.expected_revision.to_string()),
        );
        if let Some(reason) = &deactivate.reason {
            inner.insert("reason".to_owned(), json::Value::String(reason.clone()));
        }
        let mut outer = json::Map::new();
        outer.insert(
            "DeactivateContractInstance".to_owned(),
            json::Value::Object(inner),
        );
        return Ok(json::Value::Object(outer));
    }
    if let Some(claim) = instruction_ref
        .as_any()
        .downcast_ref::<ClaimTwitterFollowReward>()
    {
        let mut inner = json::Map::new();
        inner.insert(
            "binding_hash".to_owned(),
            json::to_value(&claim.binding_hash).map_err(codec_error)?,
        );
        let mut outer = json::Map::new();
        outer.insert(
            "ClaimTwitterFollowReward".to_owned(),
            json::Value::Object(inner),
        );
        return Ok(json::Value::Object(outer));
    }
    if let Some(send) = instruction_ref.as_any().downcast_ref::<SendToTwitter>() {
        let mut inner = json::Map::new();
        inner.insert(
            "binding_hash".to_owned(),
            json::to_value(&send.binding_hash).map_err(codec_error)?,
        );
        inner.insert(
            "amount".to_owned(),
            json::to_value(&send.amount).map_err(codec_error)?,
        );
        let mut outer = json::Map::new();
        outer.insert("SendToTwitter".to_owned(), json::Value::Object(inner));
        return Ok(json::Value::Object(outer));
    }
    if let Some(cancel) = instruction_ref
        .as_any()
        .downcast_ref::<CancelTwitterEscrow>()
    {
        let mut inner = json::Map::new();
        inner.insert(
            "binding_hash".to_owned(),
            json::to_value(&cancel.binding_hash).map_err(codec_error)?,
        );
        let mut outer = json::Map::new();
        outer.insert("CancelTwitterEscrow".to_owned(), json::Value::Object(inner));
        return Ok(json::Value::Object(outer));
    }
    if let Some(create) = instruction_ref.as_any().downcast_ref::<CreateKaigi>() {
        let mut payload = json::Map::new();
        payload.insert(
            "call".to_owned(),
            json::to_value(create.call()).map_err(codec_error)?,
        );
        payload.insert(
            "commitment".to_owned(),
            optional_commitment_to_json(create.commitment().as_ref()),
        );
        payload.insert(
            "nullifier".to_owned(),
            optional_nullifier_to_json(create.nullifier().as_ref()),
        );
        payload.insert(
            "roster_root".to_owned(),
            optional_hash_to_json(create.roster_root().as_ref()),
        );
        payload.insert(
            "proof".to_owned(),
            optional_proof_to_json(create.proof().as_ref()),
        );
        return Ok(kaigi_json_value(
            "CreateKaigi",
            json::Value::Object(payload),
        ));
    }
    if let Some(join) = instruction_ref.as_any().downcast_ref::<JoinKaigi>() {
        let mut payload = json::Map::new();
        payload.insert(
            "call_id".to_owned(),
            json::to_value(join.call_id()).map_err(codec_error)?,
        );
        payload.insert(
            "participant".to_owned(),
            json::to_value(join.participant()).map_err(codec_error)?,
        );
        payload.insert(
            "commitment".to_owned(),
            optional_commitment_to_json(join.commitment().as_ref()),
        );
        payload.insert(
            "nullifier".to_owned(),
            optional_nullifier_to_json(join.nullifier().as_ref()),
        );
        payload.insert(
            "roster_root".to_owned(),
            optional_hash_to_json(join.roster_root().as_ref()),
        );
        payload.insert(
            "proof".to_owned(),
            optional_proof_to_json(join.proof().as_ref()),
        );
        return Ok(kaigi_json_value("JoinKaigi", json::Value::Object(payload)));
    }
    if let Some(leave) = instruction_ref.as_any().downcast_ref::<LeaveKaigi>() {
        let mut payload = json::Map::new();
        payload.insert(
            "call_id".to_owned(),
            json::to_value(leave.call_id()).map_err(codec_error)?,
        );
        payload.insert(
            "participant".to_owned(),
            json::to_value(leave.participant()).map_err(codec_error)?,
        );
        payload.insert(
            "commitment".to_owned(),
            optional_commitment_to_json(leave.commitment().as_ref()),
        );
        payload.insert(
            "nullifier".to_owned(),
            optional_nullifier_to_json(leave.nullifier().as_ref()),
        );
        payload.insert(
            "roster_root".to_owned(),
            optional_hash_to_json(leave.roster_root().as_ref()),
        );
        payload.insert(
            "proof".to_owned(),
            optional_proof_to_json(leave.proof().as_ref()),
        );
        return Ok(kaigi_json_value("LeaveKaigi", json::Value::Object(payload)));
    }
    if let Some(end) = instruction_ref.as_any().downcast_ref::<EndKaigi>() {
        let mut payload = json::Map::new();
        payload.insert(
            "call_id".to_owned(),
            json::to_value(end.call_id()).map_err(codec_error)?,
        );
        payload.insert(
            "ended_at_ms".to_owned(),
            json::to_value(end.ended_at_ms()).map_err(codec_error)?,
        );
        payload.insert(
            "commitment".to_owned(),
            optional_commitment_to_json(end.commitment().as_ref()),
        );
        payload.insert(
            "nullifier".to_owned(),
            optional_nullifier_to_json(end.nullifier().as_ref()),
        );
        payload.insert(
            "roster_root".to_owned(),
            optional_hash_to_json(end.roster_root().as_ref()),
        );
        payload.insert(
            "proof".to_owned(),
            optional_proof_to_json(end.proof().as_ref()),
        );
        return Ok(kaigi_json_value("EndKaigi", json::Value::Object(payload)));
    }
    if let Some(usage) = instruction_ref.as_any().downcast_ref::<RecordKaigiUsage>() {
        let mut payload = json::Map::new();
        payload.insert(
            "call_id".to_owned(),
            json::to_value(usage.call_id()).map_err(codec_error)?,
        );
        payload.insert(
            "duration_ms".to_owned(),
            json::to_value(usage.duration_ms()).map_err(codec_error)?,
        );
        payload.insert(
            "billed_gas".to_owned(),
            json::to_value(usage.billed_gas()).map_err(codec_error)?,
        );
        payload.insert(
            "usage_commitment".to_owned(),
            optional_kaigi_scalar_to_json(usage.usage_commitment().as_ref()),
        );
        payload.insert(
            "proof".to_owned(),
            optional_proof_to_json(usage.proof().as_ref()),
        );
        return Ok(kaigi_json_value(
            "RecordKaigiUsage",
            json::Value::Object(payload),
        ));
    }
    if let Some(health) = instruction_ref
        .as_any()
        .downcast_ref::<ReportKaigiRelayHealth>()
    {
        let mut payload = json::Map::new();
        payload.insert(
            "call_id".to_owned(),
            json::to_value(&health.call_id).map_err(codec_error)?,
        );
        payload.insert(
            "relay_id".to_owned(),
            json::to_value(&health.relay_id).map_err(codec_error)?,
        );
        payload.insert(
            "status".to_owned(),
            json::to_value(&health.status).map_err(codec_error)?,
        );
        payload.insert(
            "reported_at_ms".to_owned(),
            json::Value::Number(health.reported_at_ms.into()),
        );
        payload.insert(
            "notes".to_owned(),
            health
                .notes
                .as_ref()
                .map_or(json::Value::Null, |s| json::Value::String(s.clone())),
        );
        return Ok(kaigi_json_value(
            "ReportKaigiRelayHealth",
            json::Value::Object(payload),
        ));
    }
    if let Some(manifest) = instruction_ref
        .as_any()
        .downcast_ref::<SetKaigiRelayManifest>()
    {
        let mut payload = json::Map::new();
        payload.insert(
            "call_id".to_owned(),
            json::to_value(manifest.call_id()).map_err(codec_error)?,
        );
        let relay_manifest = manifest.relay_manifest().clone();
        payload.insert(
            "relay_manifest".to_owned(),
            json::to_value(&relay_manifest).map_err(codec_error)?,
        );
        return Ok(kaigi_json_value(
            "SetKaigiRelayManifest",
            json::Value::Object(payload),
        ));
    }
    if let Some(registration) = instruction_ref
        .as_any()
        .downcast_ref::<RegisterKaigiRelay>()
    {
        let mut payload = json::Map::new();
        payload.insert(
            "relay".to_owned(),
            json::to_value(registration.relay()).map_err(codec_error)?,
        );
        return Ok(kaigi_json_value(
            "RegisterKaigiRelay",
            json::Value::Object(payload),
        ));
    }
    if let Some(unregistration) = instruction_ref
        .as_any()
        .downcast_ref::<UnregisterKaigiRelay>()
    {
        let mut payload = json::Map::new();
        payload.insert(
            "relay_id".to_owned(),
            json::to_value(unregistration.relay_id()).map_err(codec_error)?,
        );
        return Ok(kaigi_json_value(
            "UnregisterKaigiRelay",
            json::Value::Object(payload),
        ));
    }
    Err(CodecError::new(
        CodecErrorKind::Failure,
        "unsupported instruction variant; JSON conversion is not yet implemented for this instruction",
    ))
}

fn kaigi_json_value(tag: &str, payload: json::Value) -> json::Value {
    let mut variant = json::Map::new();
    variant.insert(tag.to_owned(), payload);
    let mut outer = json::Map::new();
    outer.insert("Kaigi".to_owned(), json::Value::Object(variant));
    json::Value::Object(outer)
}

/// Wrap an opaque custom payload in the canonical Custom instruction envelope.
pub fn custom_json_value(payload: json::Value) -> json::Value {
    let mut custom = json::Map::new();
    custom.insert("payload".to_owned(), payload);
    let mut outer = json::Map::new();
    outer.insert("Custom".to_owned(), json::Value::Object(custom));
    json::Value::Object(outer)
}

fn zk_json_value(tag: &str, payload: json::Value) -> json::Value {
    let mut variant = json::Map::new();
    variant.insert(tag.to_owned(), payload);
    let mut outer = json::Map::new();
    outer.insert("zk".to_owned(), json::Value::Object(variant));
    json::Value::Object(outer)
}

#[cfg(test)]
mod tests;
