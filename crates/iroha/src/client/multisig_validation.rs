//! Canonical multisig proposal intent and exact response/payload validation.

use super::{
    MultisigProposeRequest, MultisigResponse, canonicalize_hex32_literal,
    validate_canonical_standard_base64,
};
use base64::Engine as _;
use eyre::{Result, WrapErr as _, eyre};
use iroha_crypto::HashOf;
use iroha_data_model::validation_fee::{
    VALIDATION_FEE_HIJIRI_FEE_QUOTE_HASH_METADATA_KEY,
    VALIDATION_FEE_INSTRUCTION_INDEX_METADATA_KEY, VALIDATION_FEE_POLICY_HASH_METADATA_KEY,
    VALIDATION_FEE_POLICY_VERSION_METADATA_KEY, VALIDATION_FEE_TRANSFER_ENTRY_INDEX_METADATA_KEY,
    ValidationFeeMultisigMarkerV1,
};
use iroha_data_model::{
    NetworkId, isi::InstructionBox, metadata::Metadata, transaction::TransactionBuilder,
};
use std::time::Duration;

/// Instructions, metadata and their canonical proposal identity validated together.
pub(super) struct ProposalIntent {
    pub(super) instructions: Vec<InstructionBox>,
    pub(super) metadata: Metadata,
    pub(super) hash: HashOf<Vec<InstructionBox>>,
}

/// Parsed validation-fee metadata; raw bytes own the canonical hash values.
struct FeeMetadata {
    policy_version: u64,
    policy_hash_bytes: [u8; 32],
    hijiri_hash_bytes: Option<[u8; 32]>,
    instruction_index: Option<u64>,
    transfer_entry_index: Option<u64>,
}

fn normalized_multisig_request_string(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}
impl FeeMetadata {
    fn from_request(request: &MultisigProposeRequest) -> Result<Option<Self>> {
        let version =
            normalized_multisig_request_string(request.validation_fee_policy_version.as_deref());
        let policy_hash =
            normalized_multisig_request_string(request.validation_fee_policy_hash.as_deref());
        let hijiri_hash = normalized_multisig_request_string(
            request.validation_fee_hijiri_fee_quote_hash.as_deref(),
        );
        let instruction_index =
            normalized_multisig_request_string(request.validation_fee_instruction_index.as_deref());
        let transfer_entry_index = normalized_multisig_request_string(
            request.validation_fee_transfer_entry_index.as_deref(),
        );
        if version.is_some()
            || policy_hash.is_some()
            || hijiri_hash.is_some()
            || instruction_index.is_some()
            || transfer_entry_index.is_some()
        {
            let (Some(version), Some(policy_hash)) = (version, policy_hash) else {
                return Err(eyre!(
                    "multisig validation-fee metadata requires both policy version and hash"
                ));
            };
            let policy_version = version
                .parse::<u64>()
                .wrap_err("multisig validation-fee policy version is not a canonical u64")?;
            let policy_hash =
                canonicalize_hex32_literal(policy_hash, "multisig validation-fee policy hash")?;
            let policy_hash_bytes: [u8; 32] = hex::decode(&policy_hash)
                .wrap_err("decode multisig validation-fee policy hash")?
                .try_into()
                .map_err(|_| eyre!("multisig validation-fee policy hash is not 32 bytes"))?;
            let hijiri_hash = hijiri_hash
                .map(|hash| {
                    canonicalize_hex32_literal(hash, "multisig validation-fee Hijiri quote hash")
                })
                .transpose()?;
            let hijiri_hash_bytes = hijiri_hash
                .as_deref()
                .map(|hash| {
                    hex::decode(hash)
                        .wrap_err("decode multisig validation-fee Hijiri quote hash")?
                        .try_into()
                        .map_err(|_| {
                            eyre!("multisig validation-fee Hijiri quote hash is not 32 bytes")
                        })
                })
                .transpose()?;
            let instruction_index = instruction_index
                .map(|index| {
                    index.parse::<u64>().wrap_err(
                        "multisig validation-fee instruction index is not a canonical u64",
                    )
                })
                .transpose()?;
            let transfer_entry_index = transfer_entry_index
                .map(|index| {
                    index.parse::<u64>().wrap_err(
                        "multisig validation-fee transfer entry index is not a canonical u64",
                    )
                })
                .transpose()?;
            if transfer_entry_index.is_some() && instruction_index.is_none() {
                return Err(eyre!(
                    "multisig validation-fee transfer entry index requires an instruction index"
                ));
            }

            Ok(Some(Self {
                policy_version,
                policy_hash_bytes,
                hijiri_hash_bytes,
                instruction_index,
                transfer_entry_index,
            }))
        } else {
            Ok(None)
        }
    }

    fn append(self, proposal_instructions: &mut Vec<InstructionBox>, metadata: &mut Metadata) {
        let Self {
            policy_version,
            policy_hash_bytes,
            hijiri_hash_bytes,
            instruction_index,
            transfer_entry_index,
        } = self;
        metadata.insert(
            VALIDATION_FEE_POLICY_VERSION_METADATA_KEY
                .parse()
                .expect("static validation-fee policy-version metadata key"),
            iroha_primitives::json::Json::new(policy_version),
        );
        metadata.insert(
            VALIDATION_FEE_POLICY_HASH_METADATA_KEY
                .parse()
                .expect("static validation-fee policy-hash metadata key"),
            iroha_primitives::json::Json::new(hex::encode(policy_hash_bytes)),
        );
        if let Some(hijiri_hash) = hijiri_hash_bytes {
            metadata.insert(
                VALIDATION_FEE_HIJIRI_FEE_QUOTE_HASH_METADATA_KEY
                    .parse()
                    .expect("static Hijiri quote-hash metadata key"),
                iroha_primitives::json::Json::new(hex::encode(hijiri_hash)),
            );
        }
        if let Some(instruction_index) = instruction_index {
            metadata.insert(
                VALIDATION_FEE_INSTRUCTION_INDEX_METADATA_KEY
                    .parse()
                    .expect("static validation-fee instruction-index metadata key"),
                iroha_primitives::json::Json::new(instruction_index),
            );
            proposal_instructions.push(
                ValidationFeeMultisigMarkerV1::new(
                    policy_version,
                    policy_hash_bytes,
                    hijiri_hash_bytes,
                    instruction_index,
                    transfer_entry_index,
                )
                .into_instruction(),
            );
        }
        if let Some(transfer_entry_index) = transfer_entry_index {
            metadata.insert(
                VALIDATION_FEE_TRANSFER_ENTRY_INDEX_METADATA_KEY
                    .parse()
                    .expect("static validation-fee transfer-entry-index metadata key"),
                iroha_primitives::json::Json::new(transfer_entry_index),
            );
        }
    }
}

pub(super) fn canonical_propose_intent(request: &MultisigProposeRequest) -> Result<ProposalIntent> {
    let mut proposal_instructions = request.instructions.clone();
    if proposal_instructions.iter().any(|instruction| {
        !matches!(
            ValidationFeeMultisigMarkerV1::parse_instruction(instruction),
            Ok(None)
        )
    }) {
        return Err(eyre!(
            "multisig propose request instructions must not contain a validation-fee marker"
        ));
    }

    let mut metadata = Metadata::default();
    if let Some(memo) = normalized_multisig_request_string(request.memo.as_deref()) {
        metadata.insert(
            "memo".parse().expect("static metadata key `memo`"),
            iroha_primitives::json::Json::new(memo.to_owned()),
        );
    }

    if let Some(fee) = FeeMetadata::from_request(request)? {
        fee.append(&mut proposal_instructions, &mut metadata);
    }

    let proposal_hash = HashOf::new(&proposal_instructions);
    Ok(ProposalIntent {
        instructions: proposal_instructions,
        metadata,
        hash: proposal_hash,
    })
}
fn decode_unsigned_payload(
    response: &MultisigResponse,
    request: &MultisigProposeRequest,
    transaction_payload_b64: &str,
    signing_message_b64: &str,
) -> Result<TransactionBuilder> {
    const MAX_TRANSACTION_PAYLOAD_BYTES: usize = 16 * 1024 * 1024;
    validate_canonical_standard_base64(
        transaction_payload_b64,
        MAX_TRANSACTION_PAYLOAD_BYTES,
        "multisig response.transaction_payload_b64",
    )?;
    validate_canonical_standard_base64(
        signing_message_b64,
        64,
        "multisig response.signing_message_b64",
    )?;
    let transaction_payload = base64::engine::general_purpose::STANDARD
        .decode(transaction_payload_b64)
        .wrap_err("decode multisig response transaction payload")?;
    let builder = TransactionBuilder::decode_payload(&transaction_payload)
        .wrap_err("decode canonical multisig response transaction payload")?;
    let signing_message = base64::engine::general_purpose::STANDARD
        .decode(signing_message_b64)
        .wrap_err("decode multisig response signing message")?;
    if signing_message.as_slice() != builder.payload_hash_bytes().as_slice() {
        return Err(eyre!(
            "multisig response signing message does not match the transaction payload"
        ));
    }
    if builder.payload().fee_payment != response.fee_payment {
        return Err(eyre!(
            "multisig response fee_payment does not match the transaction payload"
        ));
    }
    if builder.payload().authority() != &request.signer_account_id {
        return Err(eyre!(
            "multisig response transaction authority does not match the requested signer"
        ));
    }
    if response.creation_time_ms != Some(builder.payload().creation_time_ms) {
        return Err(eyre!(
            "multisig response creation_time_ms does not match the transaction payload"
        ));
    }
    Ok(builder)
}

fn validate_unsigned_instructions(
    response: &MultisigResponse,
    request: &MultisigProposeRequest,
    network_id: NetworkId,
    intent: ProposalIntent,
    builder: &TransactionBuilder,
) -> Result<()> {
    let ProposalIntent {
        instructions: proposal_instructions,
        metadata: expected_metadata,
        hash: proposal_hash,
    } = intent;
    let creation_time_ms = response
        .creation_time_ms
        .expect("creation time was matched to the decoded payload above");
    let propose_instruction = InstructionBox::from(
        iroha_executor_data_model::isi::multisig::MultisigPropose::new(
            response.resolved_multisig_account_id.clone(),
            proposal_instructions,
            None,
        ),
    );
    let approve_instruction = InstructionBox::from(
        iroha_executor_data_model::isi::multisig::MultisigApprove::new(
            response.resolved_multisig_account_id.clone(),
            proposal_hash,
        ),
    );
    let mut expected_builder = TransactionBuilder::new(
        network_id,
        request.signer_account_id.clone(),
        response.fee_payment.clone(),
    );
    expected_builder.set_creation_time(Duration::from_millis(creation_time_ms));
    let expected_builder = expected_builder.with_metadata(expected_metadata);
    let propose_only = expected_builder
        .clone()
        .with_instructions(core::iter::once(propose_instruction.clone()));
    let propose_and_approve =
        expected_builder.with_instructions([propose_instruction, approve_instruction]);
    if builder.payload() != propose_only.payload()
        && builder.payload() != propose_and_approve.payload()
    {
        return Err(eyre!(
            "multisig response transaction payload does not match the exact requested executable and metadata"
        ));
    }
    if response.tx_hash_hex.is_some() || response.executed_tx_hash_hex.is_some() {
        return Err(eyre!(
            "unsubmitted multisig response must not contain transaction hashes"
        ));
    }
    Ok(())
}

pub(super) fn validate_response(
    response: &MultisigResponse,
    request: &MultisigProposeRequest,
    network_id: NetworkId,
) -> Result<()> {
    if !response.ok {
        return Err(eyre!("multisig response.ok must be true"));
    }
    if request
        .multisig_account_id
        .as_ref()
        .is_some_and(|expected| expected != &response.resolved_multisig_account_id)
    {
        return Err(eyre!(
            "multisig response resolved account does not match the requested account"
        ));
    }
    if !request
        .fee_payment
        .has_same_payer_and_gas_bound(&response.fee_payment)
    {
        return Err(eyre!(
            "multisig response fee_payment changed the requested payer, sponsor revision, or gas bound"
        ));
    }
    response
        .fee_payment
        .validate()
        .map_err(|error| eyre!("multisig response fee_payment is invalid: {error}"))?;
    if request
        .creation_time_ms
        .is_some_and(|expected| response.creation_time_ms != Some(expected))
    {
        return Err(eyre!(
            "multisig response creation_time_ms is not bound to the request"
        ));
    }
    for (field, value) in [
        (
            "multisig response.instructions_hash",
            &response.instructions_hash,
        ),
        ("multisig response.tx_hash_hex", &response.tx_hash_hex),
        (
            "multisig response.executed_tx_hash_hex",
            &response.executed_tx_hash_hex,
        ),
    ] {
        if let Some(value) = value {
            canonicalize_hex32_literal(value, field)?;
        }
    }
    if response.proposal_id.is_none()
        || response.proposal_id.as_ref() != response.instructions_hash.as_ref()
    {
        return Err(eyre!(
            "multisig propose response proposal_id and instructions_hash must be the same canonical proposal hash"
        ));
    }
    let intent = canonical_propose_intent(request)?;
    let expected_proposal_id = hex::encode(intent.hash.as_ref());
    if response.proposal_id.as_deref() != Some(expected_proposal_id.as_str()) {
        return Err(eyre!(
            "multisig response proposal hash does not match the exact requested instructions and validation-fee marker"
        ));
    }
    match (
        response.submitted,
        response.transaction_payload_b64.as_deref(),
        response.signing_message_b64.as_deref(),
    ) {
        (true, None, None) => {
            if response.tx_hash_hex.is_none() {
                return Err(eyre!(
                    "submitted multisig response must contain tx_hash_hex"
                ));
            }
        }
        (false, Some(transaction_payload_b64), Some(signing_message_b64)) => {
            let builder = decode_unsigned_payload(
                response,
                request,
                transaction_payload_b64,
                signing_message_b64,
            )?;
            validate_unsigned_instructions(response, request, network_id, intent, &builder)?;
        }
        _ => {
            return Err(eyre!(
                "multisig response must contain either a submitted transaction hash or an exact transaction-payload/signing-message pair"
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_data_model::{prelude::*, transaction::FeePaymentIntent};
    use iroha_primitives::json::Json;

    fn request() -> MultisigProposeRequest {
        MultisigProposeRequest {
            multisig_account_id: Some(iroha_test_samples::ALICE_ID.clone()),
            multisig_account_alias: None,
            signer_account_id: iroha_test_samples::BOB_ID.clone(),
            public_key_hex: None,
            signature_b64: None,
            creation_time_ms: Some(123),
            fee_payment: FeePaymentIntent::authority(Vec::new(), None),
            memo: Some("  approved intent  ".to_owned()),
            validation_fee_policy_version: Some(" 42 ".to_owned()),
            validation_fee_policy_hash: Some("AB".repeat(32)),
            validation_fee_hijiri_fee_quote_hash: Some("CE".repeat(32)),
            instructions: vec![Log::new(Level::INFO, "exact proposal".to_owned()).into()],
            validation_fee_instruction_index: Some("3".to_owned()),
            validation_fee_transfer_entry_index: Some("2".to_owned()),
        }
    }

    #[test]
    fn fee_metadata_preserves_exact_marker_hashes_and_projection() {
        let request = request();
        let intent = canonical_propose_intent(&request).unwrap();
        let mut expected_instructions = request.instructions.clone();
        expected_instructions.push(
            ValidationFeeMultisigMarkerV1::new(42, [0xab; 32], Some([0xce; 32]), 3, Some(2))
                .into_instruction(),
        );
        let mut metadata = Metadata::default();
        for (key, value) in [
            ("memo", Json::new("approved intent")),
            (
                VALIDATION_FEE_POLICY_VERSION_METADATA_KEY,
                Json::new(42_u64),
            ),
            (
                VALIDATION_FEE_POLICY_HASH_METADATA_KEY,
                Json::new("ab".repeat(32)),
            ),
            (
                VALIDATION_FEE_HIJIRI_FEE_QUOTE_HASH_METADATA_KEY,
                Json::new("ce".repeat(32)),
            ),
            (
                VALIDATION_FEE_INSTRUCTION_INDEX_METADATA_KEY,
                Json::new(3_u64),
            ),
            (
                VALIDATION_FEE_TRANSFER_ENTRY_INDEX_METADATA_KEY,
                Json::new(2_u64),
            ),
        ] {
            metadata.insert(key.parse().unwrap(), value);
        }
        assert_eq!(intent.instructions, expected_instructions);
        assert_eq!(intent.metadata, metadata);
        assert_eq!(intent.hash, HashOf::new(&expected_instructions));
        let mut altered = request;
        altered.validation_fee_instruction_index = Some("4".to_owned());
        assert_ne!(
            canonical_propose_intent(&altered).unwrap().hash,
            intent.hash
        );
    }

    #[test]
    fn incomplete_fee_binding_and_embedded_markers_reject() {
        for mutation in [
            "version",
            "policy_hash",
            "instruction_index",
            "malformed_hash",
        ] {
            let mut request = request();
            match mutation {
                "version" => request.validation_fee_policy_version = None,
                "policy_hash" => request.validation_fee_policy_hash = None,
                "instruction_index" => request.validation_fee_instruction_index = None,
                "malformed_hash" => {
                    request.validation_fee_hijiri_fee_quote_hash = Some("ab".to_owned())
                }
                _ => unreachable!(),
            }
            assert!(
                canonical_propose_intent(&request).is_err(),
                "accepted {mutation}"
            );
        }
        let mut request = request();
        request.instructions.push(
            ValidationFeeMultisigMarkerV1::new(42, [0xab; 32], None, 3, None).into_instruction(),
        );
        assert!(canonical_propose_intent(&request).is_err());
    }
}
