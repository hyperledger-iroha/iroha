//! Strict platform adapter for the shared canonical contract-multisig constructor.
use crate::{
    CodecError, CodecResult, codec_error, exact_json_object_fields, instruction_to_json_value,
};
use iroha_crypto::Hash;
use iroha_data_model::{
    account::address::{AccountAddress, ChainDiscriminantGuard},
    smart_contract::{ContractAddress, ContractAlias, multisig_call::build_multisig_contract_call},
    transaction::executable::ContractArgumentRecord,
};
use iroha_primitives::json::Json;
use norito::json;
use std::str::FromStr;

/// Produce exact native instruction JSON, metadata and proposal hash.
/// Inputs must come from the caller's independently pinned contract release and
/// exact user intent. This pure codec does not authenticate a ledger projection.
pub fn build_canonical_multisig_contract_call_json(
    input: &str,
    network_prefix: u16,
) -> CodecResult<String> {
    if input.len() > 4 * 1024 * 1024 {
        return Err(CodecError::failure("contract preparation exceeds 4 MiB"));
    }
    let _network = ChainDiscriminantGuard::enter(network_prefix);
    let value: json::Value = json::parse_value(input).map_err(codec_error)?;
    exact_json_object_fields(
        &value,
        &[
            "multisig_account_id",
            "contract_address",
            "contract_alias",
            "entrypoint",
            "payload",
            "arguments_hex",
            "code_hash_hex",
        ],
        "contract multisig input",
    )?;
    let object = value
        .as_object()
        .ok_or_else(|| CodecError::failure("object required"))?;
    let string = |key: &str| -> CodecResult<&str> {
        object
            .get(key)
            .and_then(json::Value::as_str)
            .filter(|s| !s.is_empty() && s.trim() == *s)
            .ok_or_else(|| CodecError::failure(format!("exact {key} string required")))
    };
    let account_literal = string("multisig_account_id")?;
    let account = AccountAddress::from_i105_for_discriminant(account_literal, Some(network_prefix))
        .map_err(codec_error)?
        .to_account_id()
        .map_err(codec_error)?;
    if account.to_string() != account_literal {
        return Err(CodecError::failure("noncanonical multisig account"));
    }
    let address_literal = string("contract_address")?;
    let address = ContractAddress::from_str(address_literal).map_err(codec_error)?;
    if address.to_string() != address_literal {
        return Err(CodecError::failure("noncanonical contract address"));
    }
    let alias_literal = string("contract_alias")?;
    let alias = ContractAlias::from_str(alias_literal).map_err(codec_error)?;
    if alias.to_string() != alias_literal {
        return Err(CodecError::failure("noncanonical contract alias"));
    }
    let code = string("code_hash_hex")?;
    if code.len() != 64
        || !code
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    {
        return Err(CodecError::failure(
            "code_hash_hex requires exact lower-case 32-byte hexadecimal",
        ));
    }
    let code_hash = Hash::from_str(&code.to_ascii_uppercase()).map_err(codec_error)?;
    let arguments = match object.get("arguments_hex") {
        Some(json::Value::Null) => None,
        Some(json::Value::String(literal))
            if literal.len() <= 2 * 1024 * 1024
                && literal.len() % 2 == 0
                && literal
                    .bytes()
                    .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)) =>
        {
            Some(
                ContractArgumentRecord::try_new(hex::decode(literal).map_err(codec_error)?)
                    .map_err(codec_error)?,
            )
        }
        _ => {
            return Err(CodecError::failure(
                "arguments_hex requires bounded exact native argument-record bytes or null",
            ));
        }
    };
    let payload = match object.get("payload") {
        Some(value @ json::Value::Object(_)) => Json::new(value.clone()),
        _ => {
            return Err(CodecError::failure("payload must be an exact object"));
        }
    };
    let call = build_multisig_contract_call(
        &account,
        &address,
        &alias,
        string("entrypoint")?,
        &payload,
        arguments,
        &code_hash,
    )
    .map_err(codec_error)?;
    let instructions = call
        .instructions
        .iter()
        .map(instruction_to_json_value)
        .collect::<CodecResult<Vec<_>>>()?;
    json::to_json(&norito::json!({
        "instructions": instructions,
        "instructions_hash": hex::encode(call.instructions_hash.as_ref()),
        "metadata": call.metadata,
    }))
    .map_err(codec_error)
}
