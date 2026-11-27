//! Strict platform adapter for the shared canonical contract-multisig constructor.
use crate::{
    CodecError, CodecResult, codec_error, exact_json_object_fields, instruction_to_json_value,
};
use iroha_crypto::Hash;
use iroha_data_model::{
    account::address::{AccountAddress, ChainDiscriminantGuard},
    smart_contract::{ContractAddress, ContractAlias, multisig_call::build_multisig_contract_call},
};
use iroha_primitives::json::Json;
use norito::json;
use std::{num::NonZeroU64, str::FromStr};

/// Produce exact native instruction JSON, metadata and proposal hash.
/// Inputs must come from the caller's independently pinned contract release and
/// exact user intent. This pure codec does not authenticate a ledger projection.
/// The payload is the callback's sole argument source: the trigger callback takes
/// its arguments from the firing `ExecuteTrigger` event, so no argument record is
/// accepted.
///
/// # Errors
///
/// Returns an error when the input exceeds 4 MiB or is not the exact input object,
/// when any account, contract address, alias, code hash or payload is missing or
/// noncanonical, or when the native multisig call cannot be built or rendered.
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
            "code_hash_hex",
            "creation_time_ms",
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
    let code_hash = parse_code_hash_hex(string("code_hash_hex")?)?;
    let attempt_created_at_ms = object
        .get("creation_time_ms")
        .and_then(json::Value::as_u64)
        .and_then(NonZeroU64::new)
        .ok_or_else(|| CodecError::failure("exact positive creation_time_ms required"))?;
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
        &code_hash,
        attempt_created_at_ms,
    )
    .map_err(codec_error)?;
    let instructions = call
        .instructions
        .iter()
        .map(instruction_to_json_value)
        .collect::<CodecResult<Vec<_>>>()?;
    let mut output = json::Map::new();
    output.insert("instructions".to_owned(), json::Value::Array(instructions));
    output.insert(
        "instructions_hash".to_owned(),
        json::Value::String(hex::encode(call.instructions_hash.as_ref())),
    );
    output.insert(
        "metadata".to_owned(),
        json::to_value(&call.metadata).map_err(codec_error)?,
    );
    json::to_json(&json::Value::Object(output)).map_err(codec_error)
}

/// Admit exact lower-case 32-byte hexadecimal as a native code hash.
fn parse_code_hash_hex(code: &str) -> CodecResult<Hash> {
    if code.len() != 64
        || !code
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    {
        return Err(CodecError::failure(
            "code_hash_hex requires exact lower-case 32-byte hexadecimal",
        ));
    }
    Hash::from_str(&code.to_ascii_uppercase()).map_err(codec_error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{account::AccountId, id::NetworkId};
    use iroha_model_base::topology::DataSpaceId;

    #[test]
    fn contract_call_json_contains_exact_native_instructions_and_hash() {
        let key = KeyPair::try_from_seed(vec![7; 32], Algorithm::Ed25519).expect("fixture key");
        let account = AccountId::new(key.public_key().clone());
        let network: NetworkId =
            "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0"
                .parse()
                .expect("network");
        let address = ContractAddress::derive(&network, &account, 7, DataSpaceId::new(9))
            .expect("contract address");
        let alias: ContractAlias = "reviewed_artifact::universal".parse().expect("alias");
        let code_hash = Hash::new(b"reviewed-artifact");
        let payload = Json::new(norito::json!({"proposal_id": "case-1"}));
        let account_literal = account
            .to_i105_for_discriminant(369)
            .expect("account address");
        let input = norito::json!({
            "multisig_account_id": account_literal,
            "contract_address": (address.to_string()),
            "contract_alias": (alias.to_string()),
            "entrypoint": "finalize_mint_request",
            "payload": {"proposal_id": "case-1"},
            "code_hash_hex": (hex::encode(code_hash.as_ref())),
            "creation_time_ms": 1,
        });
        let input = json::to_json(&input).expect("input JSON");
        let result = build_canonical_multisig_contract_call_json(&input, 369)
            .expect("canonical multisig call");
        let value: json::Value = json::from_json(&result).expect("output JSON");
        let expected = build_multisig_contract_call(
            &account,
            &address,
            &alias,
            "finalize_mint_request",
            &payload,
            &code_hash,
            NonZeroU64::new(1).unwrap(),
        )
        .expect("expected call");
        let expected_instructions = expected
            .instructions
            .iter()
            .map(instruction_to_json_value)
            .collect::<CodecResult<Vec<_>>>()
            .expect("native instructions");
        assert_eq!(
            value["instructions"],
            json::Value::Array(expected_instructions)
        );
        assert_eq!(
            value["instructions_hash"].as_str(),
            Some(hex::encode(expected.instructions_hash.as_ref()).as_str()),
        );
        assert_eq!(
            value["metadata"],
            json::to_value(&expected.metadata).expect("native metadata"),
        );
        let mut refreshed: json::Value = json::from_json(&input).unwrap();
        refreshed
            .as_object_mut()
            .unwrap()
            .insert("creation_time_ms".into(), json::Value::from(2_u64));
        let retry: json::Value = json::from_json(
            &build_canonical_multisig_contract_call_json(&json::to_json(&refreshed).unwrap(), 369)
                .unwrap(),
        )
        .unwrap();
        assert_ne!(value["instructions_hash"], retry["instructions_hash"]);
        for invalid in [
            json::Value::from(0_u64),
            json::Value::Null,
            json::Value::String("1".into()),
        ] {
            refreshed
                .as_object_mut()
                .unwrap()
                .insert("creation_time_ms".into(), invalid);
            assert!(
                build_canonical_multisig_contract_call_json(
                    &json::to_json(&refreshed).unwrap(),
                    369
                )
                .is_err()
            );
        }
    }

    #[test]
    fn code_hash_hex_admits_only_exact_lower_case_digests() {
        let code_hash = Hash::new(b"reviewed-artifact");
        let literal = hex::encode(code_hash.as_ref());
        assert_eq!(parse_code_hash_hex(&literal).expect("digest"), code_hash);
        let upper = literal.to_ascii_uppercase();
        assert_ne!(upper, literal, "fixture digest must contain hex letters");
        for invalid in [upper, literal[..62].to_owned(), format!("{literal}00")] {
            assert!(parse_code_hash_hex(&invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn fixed_argument_record_input_is_refused() {
        let key = KeyPair::try_from_seed(vec![7; 32], Algorithm::Ed25519).expect("fixture key");
        let account = AccountId::new(key.public_key().clone());
        let network: NetworkId =
            "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0"
                .parse()
                .expect("network");
        let address = ContractAddress::derive(&network, &account, 7, DataSpaceId::new(9))
            .expect("contract address");
        let mut input = norito::json!({
            "multisig_account_id": (account
                .to_i105_for_discriminant(369)
                .expect("account address")),
            "contract_address": (address.to_string()),
            "contract_alias": "reviewed_artifact::universal",
            "entrypoint": "finalize_mint_request",
            "payload": {"proposal_id": "case-1"},
            "code_hash_hex": (hex::encode(Hash::new(b"reviewed-artifact").as_ref())),
            "creation_time_ms": 1,
        });
        build_canonical_multisig_contract_call_json(&json::to_json(&input).unwrap(), 369)
            .expect("payload-only input builds");
        for record in [json::Value::Null, json::Value::String("0a0b".to_owned())] {
            input
                .as_object_mut()
                .unwrap()
                .insert("arguments_hex".into(), record);
            assert!(
                build_canonical_multisig_contract_call_json(&json::to_json(&input).unwrap(), 369)
                    .is_err(),
                "trigger callbacks take their arguments from the firing event"
            );
        }
    }
}
