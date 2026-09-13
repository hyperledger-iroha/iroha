//! Closed native JSON contracts for NFT sales, contract deployment and replication.
//!
//! This module projects existing ledger types; it grants no execution authority
//! and does not verify stored contract artifacts or replication-order payloads.
//! SoraFS number-only u64 operands must fit the JavaScript safe-integer range
//! in both directions; NFT and deployment decimal-string operands retain u64.

use base64::{Engine as _, engine::general_purpose::STANDARD};
use iroha_data_model::{
    isi::{
        Instruction, InstructionBox,
        nft_market::{BuyNftV1, CancelNftOfferV1, OfferNftV1},
        smart_contract_code::{
            CommitContractDeployment, FinalizeSmartContractCodeUpload, UploadSmartContractCodeChunk,
        },
        sorafs::{CompleteReplicationOrder, ExpireReplicationOrder, IssueReplicationOrder},
    },
    musubi::ArchiveId,
    nft_market::NftSaleOfferV1,
    sorafs::{
        capacity::ProviderId,
        pin_registry::{
            ProviderIngestCompletionAuthorityV1, ProviderIngestCompletionSignerPolicyV1,
            ProviderIngestFinalizedAnchorV1, ReplicationOrderId,
        },
    },
};
use norito::json::{self, JsonDeserialize, JsonSerialize, Value};

use crate::{CodecError, CodecErrorKind, CodecResult, json_u64::MAX_SAFE_INTEGER};

fn invalid(message: impl Into<String>) -> CodecError {
    CodecError::new(CodecErrorKind::InvalidArgument, message)
}

fn object(value: Value, names: &[&str], context: &str) -> CodecResult<json::Map> {
    let Value::Object(fields) = value else {
        return Err(invalid(format!("{context} must be an object")));
    };
    // Missing optional operands are errors too: their null tag is part of the
    // signed contract. Keep the field diagnostic useful to direct SDK callers.
    for name in names {
        if !fields.contains_key(*name) {
            return Err(invalid(format!("{context}: missing field {name}")));
        }
    }
    crate::require_exact_json_fields(&fields, names, context)?;
    Ok(fields)
}

fn take(fields: &mut json::Map, name: &str, context: &str) -> CodecResult<Value> {
    crate::required_value(fields, name, context)
}

fn parse_model<T: JsonDeserialize + JsonSerialize>(value: Value, context: &str) -> CodecResult<T> {
    let parsed: T =
        json::from_value(value.clone()).map_err(|error| invalid(format!("{context}: {error}")))?;
    // Use each native type's single canonical spelling, including checksummed
    // hashes, I105 accounts, NFT ids and contract addresses/aliases.
    if render_model(&parsed)? != value {
        return Err(invalid(format!(
            "{context} must use its canonical native JSON spelling"
        )));
    }
    Ok(parsed)
}

fn render_model<T: JsonSerialize>(value: &T) -> CodecResult<Value> {
    json::to_value(value).map_err(crate::codec_error)
}

fn parse_u64_text(value: Value, context: &str) -> CodecResult<u64> {
    let Value::String(text) = value else {
        return Err(invalid(format!(
            "{context} must be a canonical u64 decimal string"
        )));
    };
    let number = text
        .parse::<u64>()
        .map_err(|error| invalid(format!("{context}: {error}")))?;
    if number.to_string() != text {
        return Err(invalid(format!(
            "{context} must be a canonical u64 decimal string"
        )));
    }
    Ok(number)
}

fn render_u64_text(value: &u64) -> CodecResult<Value> {
    Ok(Value::String(value.to_string()))
}

fn parse_u64_number(value: Value, context: &str) -> CodecResult<u64> {
    let Value::Number(number) = value else {
        return Err(invalid(format!(
            "{context} must be an unsigned JSON integer"
        )));
    };
    number
        .as_u64()
        .filter(|number| *number <= MAX_SAFE_INTEGER)
        .ok_or_else(|| {
            invalid(format!(
                "{context} must be an unsigned JSON integer no greater than {MAX_SAFE_INTEGER}"
            ))
        })
}

fn render_u64_number(value: &u64) -> CodecResult<Value> {
    if *value > MAX_SAFE_INTEGER {
        return Err(invalid(format!(
            "number-only u64 operand exceeds the JavaScript maximum safe integer {MAX_SAFE_INTEGER}"
        )));
    }
    Ok(Value::from(*value))
}

fn parse_u32_number(value: Value, context: &str) -> CodecResult<u32> {
    u32::try_from(parse_u64_number(value, context)?)
        .map_err(|error| invalid(format!("{context}: {error}")))
}

fn parse_bytes(value: Value, context: &str) -> CodecResult<Vec<u8>> {
    let Value::String(text) = value else {
        return Err(invalid(format!(
            "{context} must be canonical standard base64"
        )));
    };
    let bytes = STANDARD
        .decode(&text)
        .map_err(|error| invalid(format!("{context}: {error}")))?;
    if STANDARD.encode(&bytes) != text {
        return Err(invalid(format!(
            "{context} must be canonical standard base64"
        )));
    }
    Ok(bytes)
}

fn render_bytes(value: &[u8]) -> CodecResult<Value> {
    Ok(Value::String(STANDARD.encode(value)))
}

fn parse_digest(value: Value, context: &str) -> CodecResult<[u8; 32]> {
    let Value::String(text) = value else {
        return Err(invalid(format!(
            "{context} must be 64 lowercase hexadecimal characters"
        )));
    };
    if text.len() != 64
        || !text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(invalid(format!(
            "{context} must be 64 lowercase hexadecimal characters"
        )));
    }
    let mut bytes = [0; 32];
    hex::decode_to_slice(&text, &mut bytes)
        .map_err(|error| invalid(format!("{context}: {error}")))?;
    Ok(bytes)
}

fn render_digest(value: &[u8; 32]) -> CodecResult<Value> {
    Ok(Value::String(hex::encode(value)))
}

macro_rules! digest_operand {
    ($parse:ident, $render:ident, $ty:ident) => {
        fn $parse(value: Value, context: &str) -> CodecResult<$ty> {
            parse_digest(value, context).map($ty::new)
        }
        fn $render(value: &$ty) -> CodecResult<Value> {
            render_digest(value.as_bytes())
        }
    };
}
digest_operand!(parse_order_id, render_order_id, ReplicationOrderId);
digest_operand!(parse_provider_id, render_provider_id, ProviderId);
digest_operand!(parse_archive_id, render_archive_id, ArchiveId);

macro_rules! optional_operand {
    ($parse:ident, $render:ident, $inner_parse:ident, $inner_render:ident, $ty:ty) => {
        fn $parse(value: Value, context: &str) -> CodecResult<Option<$ty>> {
            match value {
                Value::Null => Ok(None),
                value => $inner_parse(value, context).map(Some),
            }
        }
        fn $render(value: &Option<$ty>) -> CodecResult<Value> {
            value.as_ref().map_or(Ok(Value::Null), $inner_render)
        }
    };
}
optional_operand!(
    parse_optional_account,
    render_optional_account,
    parse_model,
    render_model,
    iroha_data_model::account::AccountId
);
optional_operand!(
    parse_optional_address,
    render_optional_address,
    parse_model,
    render_model,
    iroha_data_model::smart_contract::ContractAddress
);
optional_operand!(
    parse_optional_u64_text,
    render_optional_u64_text,
    parse_u64_text,
    render_u64_text,
    u64
);
optional_operand!(
    parse_optional_digest,
    render_optional_digest,
    parse_digest,
    render_digest,
    [u8; 32]
);
optional_operand!(
    parse_optional_archive_id,
    render_optional_archive_id,
    parse_archive_id,
    render_archive_id,
    ArchiveId
);

// The same explicit field list owns both directions. Native model fields and
// their Rust types remain the source of truth; there are no substitute records.
macro_rules! record_contract {
    ($parse:ident, $render:ident, $ty:ident { $($field:ident: $read:ident => $write:ident),+ $(,)? }) => {
        fn $parse(value: Value, context: &str) -> CodecResult<$ty> {
            let mut fields = object(value, &[$(stringify!($field)),+], context)?;
            Ok($ty {
                $($field: $read(take(&mut fields, stringify!($field), context)?,
                    &format!("{context}.{}", stringify!($field)))?),+
            })
        }
        fn $render(value: &$ty) -> CodecResult<Value> {
            let mut fields = json::Map::new();
            $(fields.insert(stringify!($field).to_owned(), $write(&value.$field)?);)+
            Ok(Value::Object(fields))
        }
    };
}
record_contract!(parse_offer, render_offer, NftSaleOfferV1 {
    network_id: parse_model => render_model,
    offer_id: parse_model => render_model,
    nft_id: parse_model => render_model,
    seller: parse_model => render_model,
    payment_asset: parse_model => render_model,
    price: parse_model => render_model,
    expires_at_height: parse_u64_text => render_u64_text,
    reserved_buyer: parse_optional_account => render_optional_account,
    metadata_hash: parse_model => render_model,
});
record_contract!(parse_policy, render_policy, ProviderIngestCompletionSignerPolicyV1 {
    policy_id: parse_digest => render_digest,
    revision: parse_u64_number => render_u64_number,
    predecessor_digest: parse_optional_digest => render_optional_digest,
    policy_digest: parse_digest => render_digest,
});
record_contract!(parse_authority, render_authority, ProviderIngestCompletionAuthorityV1 {
    provider_owner: parse_model => render_model,
    signer_policy: parse_policy => render_policy,
});
record_contract!(parse_anchor, render_anchor, ProviderIngestFinalizedAnchorV1 {
    height: parse_u64_number => render_u64_number,
    block_hash: parse_digest => render_digest,
});

macro_rules! instruction_contracts {
    ($($ty:ident { $($field:ident: $read:ident => $write:ident),+ $(,)? }),+ $(,)?) => {
        pub(super) fn is_lifecycle_instruction(instruction: &InstructionBox) -> bool {
            let instruction: &dyn Instruction = &**instruction;
            let value = instruction.as_any();
            $(value.is::<$ty>())||+
        }

        pub(super) fn from_json(value: &Value) -> Option<CodecResult<InstructionBox>> {
            let Value::Object(envelope) = value else { return None; };
            $(if let Some(payload) = envelope.get(stringify!($ty)) {
                return Some((|| {
                    crate::require_exact_json_fields(envelope, &[stringify!($ty)], "instruction envelope")?;
                    let context = stringify!($ty);
                    let mut fields = object(payload.clone(), &[$(stringify!($field)),+], context)?;
                    Ok($ty {
                        $($field: $read(take(&mut fields, stringify!($field), context)?,
                            &format!("{context}.{}", stringify!($field)))?),+
                    }.into())
                })());
            })+
            None
        }

        pub(super) fn to_json(instruction: &InstructionBox) -> Option<CodecResult<Value>> {
            let instruction: &dyn Instruction = &**instruction;
            let value = instruction.as_any();
            $(if let Some(value) = value.downcast_ref::<$ty>() {
                return Some((|| {
                    let mut fields = json::Map::new();
                    $(fields.insert(stringify!($field).to_owned(), $write(&value.$field)?);)+
                    let mut envelope = json::Map::new();
                    envelope.insert(stringify!($ty).to_owned(), Value::Object(fields));
                    Ok(Value::Object(envelope))
                })());
            })+
            None
        }
    };
}
instruction_contracts! {
    OfferNftV1 {
        offer_id: parse_model => render_model,
        nft_id: parse_model => render_model,
        payment_asset: parse_model => render_model,
        price: parse_model => render_model,
        expires_at_height: parse_u64_text => render_u64_text,
        reserved_buyer: parse_optional_account => render_optional_account,
    },
    BuyNftV1 { offer: parse_offer => render_offer },
    CancelNftOfferV1 {
        offer_id: parse_model => render_model,
        expected_offer_hash: parse_model => render_model,
    },
    UploadSmartContractCodeChunk {
        code_hash: parse_model => render_model,
        total_size: parse_u64_text => render_u64_text,
        chunk_index: parse_u32_number => render_model,
        chunk_count: parse_u32_number => render_model,
        chunk: parse_bytes => render_bytes,
    },
    FinalizeSmartContractCodeUpload {
        code_hash: parse_model => render_model,
        total_size: parse_u64_text => render_u64_text,
        chunk_count: parse_u32_number => render_model,
    },
    CommitContractDeployment {
        expected_deploy_nonce: parse_u64_text => render_u64_text,
        contract_address: parse_model => render_model,
        code_hash: parse_model => render_model,
        contract_alias: parse_model => render_model,
        lease_expiry_ms: parse_optional_u64_text => render_optional_u64_text,
        expected_previous_contract_address: parse_optional_address => render_optional_address,
    },
    IssueReplicationOrder {
        order_id: parse_order_id => render_order_id,
        order_payload: parse_bytes => render_bytes,
        issued_epoch: parse_u64_number => render_u64_number,
        deadline_epoch: parse_u64_number => render_u64_number,
        musubi_archive: parse_optional_archive_id => render_optional_archive_id,
    },
    CompleteReplicationOrder {
        order_id: parse_order_id => render_order_id,
        provider_id: parse_provider_id => render_provider_id,
        completion_epoch: parse_u64_number => render_u64_number,
        expected_authority: parse_authority => render_authority,
        expected_assignment_revision: parse_u64_number => render_u64_number,
        finalized_anchor: parse_anchor => render_anchor,
    },
    ExpireReplicationOrder {
        order_id: parse_order_id => render_order_id,
        expiration_epoch: parse_u64_number => render_u64_number,
    },
}

#[cfg(test)]
#[path = "lifecycle_instruction_tests.rs"]
mod tests;
