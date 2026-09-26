//! Canonical platform-independent construction of one exact contract multisig call.
//!
//! Callers must independently authenticate the contract code/schema and approval
//! authority. Construction is not proof of deployment, authorization or finality.
use crate::{
    account::AccountId,
    events::execute_trigger::ExecuteTriggerEventFilter,
    isi::{ExecuteTrigger, InstructionBox, Register},
    smart_contract::{ContractAddress, ContractAlias},
    transaction::{
        Executable,
        executable::{ContractArgumentRecord, ContractInvocation},
    },
    trigger::{
        Trigger, TriggerId,
        action::{Action, Repeats},
    },
};
use iroha_crypto::{Hash, HashOf};
use iroha_model_base::{metadata::Metadata, name::Name};
use iroha_primitives::json::Json;
use std::str::FromStr;

/// Native call material to be incorporated into the caller's complete signed transaction.
pub struct CanonicalMultisigContractCall {
    /// Exactly `RegisterTrigger` followed by `ExecuteTrigger`.
    pub instructions: Vec<InstructionBox>,
    /// Hash of the exact native instruction vector, including target, code and arguments.
    pub instructions_hash: HashOf<Vec<InstructionBox>>,
    /// Reserved metadata on the constructed trigger action.
    pub metadata: Metadata,
}

/// Construct trigger metadata without application-specific policy or event claims.
pub fn contract_call_metadata(
    address: &ContractAddress,
    code_hash: &Hash,
    alias: &ContractAlias,
    entrypoint: &str,
    payload: &Json,
) -> Metadata {
    let mut metadata = Metadata::default();
    for (key, value) in [
        ("contract_address", Json::new(address.to_string())),
        ("contract_code_hash", Json::new(code_hash.to_string())),
    ] {
        metadata.insert(Name::from_str(key).expect("static metadata key"), value);
    }
    metadata.insert(
        Name::from_str("contract_alias").expect("static metadata key"),
        Json::new(alias.to_string()),
    );
    metadata.insert(
        Name::from_str("contract_entrypoint").expect("static metadata key"),
        Json::new(entrypoint.to_owned()),
    );
    metadata.insert(
        Name::from_str("contract_payload").expect("static metadata key"),
        payload.clone(),
    );
    metadata
}

/// Derive the sole current trigger identifier from typed, canonical native material.
/// Delimiter-separated text and serialization-error placeholders are not accepted.
///
/// # Errors
/// Returns an error if the typed material cannot be canonically encoded or the
/// derived trigger name is invalid.
pub fn derive_multisig_contract_call_trigger_id(
    authority: &AccountId,
    address: &ContractAddress,
    entrypoint: &str,
    payload: &Json,
    code_hash: &Hash,
) -> Result<TriggerId, String> {
    let material = (
        authority.clone(),
        address.clone(),
        entrypoint.to_owned(),
        payload.clone(),
        *code_hash,
    );
    let wire = norito::encode_canonical(&material).map_err(|error| error.to_string())?;
    let hash = Hash::new_from_chunks(&[b"iroha:multisig-contract-trigger:v1\0", &wire]);
    let name = Name::from_str(&format!("msig_cc_{}", hex::encode(hash.as_ref())))
        .map_err(|error| error.to_string())?;
    Ok(TriggerId::new(name))
}

/// Construct the exact current proposal from independently reviewed typed inputs.
/// No ledger I/O, signature, alias resolution, or application authority inference occurs.
///
/// # Errors
/// Rejects an empty, overlong, or whitespace-padded entrypoint, a payload that is
/// not a JSON object, or a failure to derive the trigger ID or construct its action.
pub fn build_multisig_contract_call(
    authority: &AccountId,
    address: &ContractAddress,
    alias: &ContractAlias,
    entrypoint: &str,
    payload: &Json,
    arguments: Option<ContractArgumentRecord>,
    code_hash: &Hash,
) -> Result<CanonicalMultisigContractCall, String> {
    if entrypoint.is_empty() || entrypoint.len() > 128 || entrypoint.trim() != entrypoint {
        return Err("exact bounded contract entrypoint required".to_owned());
    }
    if !matches!(
        payload.try_into_any_norito::<norito::json::Value>(),
        Ok(norito::json::Value::Object(_))
    ) {
        return Err("exact contract object payload required".to_owned());
    }
    let trigger_id = derive_multisig_contract_call_trigger_id(
        authority, address, entrypoint, payload, code_hash,
    )?;
    let metadata = contract_call_metadata(address, code_hash, alias, entrypoint, payload);
    let action = Action::new(
        Executable::ContractCall(ContractInvocation {
            contract_address: address.clone(),
            expected_code_hash: *code_hash,
            entrypoint: entrypoint.to_owned(),
            arguments,
        }),
        Repeats::Exactly(1),
        authority.clone(),
        ExecuteTriggerEventFilter::new().for_trigger(trigger_id.clone()),
    )
    .map_err(|error| error.to_string())?
    .with_metadata(metadata.clone());
    let execute = ExecuteTrigger::new(trigger_id.clone()).with_args(payload.clone());
    let instructions = vec![
        InstructionBox::from(Register::trigger(Trigger::new(trigger_id, action))),
        InstructionBox::from(execute),
    ];
    Ok(CanonicalMultisigContractCall {
        instructions_hash: HashOf::new(&instructions),
        instructions,
        metadata,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::NetworkId;
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_model_base::topology::DataSpaceId;
    fn fixture() -> (AccountId, ContractAddress, ContractAlias, Hash) {
        let key = KeyPair::try_from_seed(vec![7; 32], Algorithm::Ed25519).expect("fixture key");
        let owner = AccountId::new(key.public_key().clone());
        let network: NetworkId =
            "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0"
                .parse()
                .expect("network");
        let address =
            ContractAddress::derive(&network, &owner, 7, DataSpaceId::new(9)).expect("address");
        let alias = "reviewed_artifact::universal".parse().expect("alias");
        (owner, address, alias, Hash::new(b"reviewed-artifact"))
    }
    #[test]
    fn exact_call_binds_arguments_target_code_and_instruction_order() {
        let (owner, address, alias, code) = fixture();
        let payload = Json::new(norito::json!({"proposal_id":"case-1", "finalized_at_ms":12}));
        let build = |args, code| {
            build_multisig_contract_call(
                &owner,
                &address,
                &alias,
                "finalize_mint_request",
                &payload,
                Some(ContractArgumentRecord::try_new(args).expect("args")),
                code,
            )
            .expect("call")
        };
        let original = build(vec![1, 2, 3], &code);
        assert_ne!(
            original.instructions_hash,
            build(vec![1, 2, 4], &code).instructions_hash
        );
        assert_ne!(
            original.instructions_hash,
            build(vec![1, 2, 3], &Hash::new(b"different-artifact")).instructions_hash
        );
        let mut reordered = original.instructions.clone();
        reordered.reverse();
        assert_ne!(original.instructions_hash, HashOf::new(&reordered));
        assert_eq!(original.instructions.len(), 2);
        assert_eq!(
            original.metadata,
            contract_call_metadata(&address, &code, &alias, "finalize_mint_request", &payload)
        );
        assert!(
            build_multisig_contract_call(
                &owner,
                &address,
                &alias,
                "finalize_mint_request",
                &Json::new("scalar"),
                None,
                &code,
            )
            .is_err()
        );
    }
    #[test]
    fn entrypoint_and_payload_cannot_collide_through_text_delimiters() {
        let (owner, address, alias, code) = fixture();
        let first = derive_multisig_contract_call_trigger_id(
            &owner,
            &address,
            "a|b",
            &Json::new(norito::json!({"value":"c"})),
            &code,
        )
        .expect("first");
        let second = derive_multisig_contract_call_trigger_id(
            &owner,
            &address,
            "a",
            &Json::new(norito::json!({"value":"b|c"})),
            &code,
        )
        .expect("second");
        assert_ne!(first, second);
        assert!(
            build_multisig_contract_call(
                &owner,
                &address,
                &alias,
                " ",
                &Json::new(norito::json!({})),
                None,
                &code,
            )
            .is_err()
        );
    }
}
