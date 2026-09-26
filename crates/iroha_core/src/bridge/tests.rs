//! Bridge finality and SCCP execution-output regression tests.

use super::*;
use crate::tx::AcceptedTransaction;
use iroha_crypto::{Algorithm, Hash, KeyPair, SignatureOf};
use iroha_data_model::{
    account::AccountId,
    block::{
        BlockSignature, SignedBlock, execution_output::*, output_budget::ExecutionOutputLimits,
    },
    events::{
        time::{TimeEvent, TimeInterval},
        trigger_completed::TriggerCompletedOutcome,
    },
    isi::InstructionBox,
    prelude::TransactionBuilder,
    smart_contract::ContractAddress,
    transaction::{
        DataTriggerSequence, Executable, ExecutionStep, IvmBytecode, IvmProved, SignedTransaction,
        TransactionEntrypoint, TransactionResultInner, executable::ContractInvocation,
    },
    trigger::DataTriggerStep,
};
use iroha_model_base::topology::DataSpaceId;
use norito::codec::DecodeAll as _;
use std::{borrow::Cow, num::NonZeroU64};
#[derive(norito::NoritoSchema, norito::codec::Decode, norito::codec::Encode)]
#[norito_schema(name = "iroha_core::bridge::tests::MutableBridgeBlock")]
struct MutableBridgeBlock {
    signatures: BTreeSet<BlockSignature>,
    payload: iroha_data_model::block::BlockPayload,
    result: Option<iroha_data_model::block::BlockResult>,
}

// Structural, deliberately untrusted decode mirrors the actual three-field
// block payload, without exposing a production mutation API or fixing caches.
fn mutate_bridge_block(
    block: &SignedBlock,
    mutate: impl FnOnce(&mut MutableBridgeBlock),
) -> SignedBlock {
    use norito::codec::Encode;
    let mut encoded = MutableBridgeBlock::decode_all(&mut block.encode().as_slice())
        .expect("decode structural bridge fixture");
    mutate(&mut encoded);
    SignedBlock::decode_all(&mut encoded.encode().as_slice())
        .expect("retain adversarial bridge structure without repairing commitments")
}

fn checked_keypair() -> KeyPair {
    KeyPair::try_random().expect("bridge fixture key generation should succeed")
}
fn checked_bls_keypair() -> KeyPair {
    KeyPair::try_random_with_algorithm(Algorithm::BlsNormal)
        .expect("bridge BLS fixture key generation should succeed")
}
fn bridge_test_network_id(seed: &[u8]) -> NetworkId {
    NetworkId::from_genesis_hash(iroha_crypto::HashOf::<BlockHeader>::from_untyped_unchecked(
        Hash::new(seed),
    ))
}
#[test]
fn finality_attestation_requires_exact_state_view_tip_hash() {
    let committed_tip = BlockHeader::new(
        NonZeroU64::new(2).expect("non-zero height"),
        None,
        None,
        0,
        0,
    )
    .hash();
    let another_tip = BlockHeader::new(
        NonZeroU64::new(3).expect("non-zero height"),
        Some(committed_tip),
        None,
        0,
        0,
    )
    .hash();
    require_finality_proof_at_committed_tip(committed_tip, committed_tip)
        .expect("exact tip must bind");
    assert!(matches!(
        require_finality_proof_at_committed_tip(committed_tip, another_tip),
        Err(BridgeFinalityAttestationBuildError::FinalityTipMismatch {
            committed_tip_hash,
            proof_block_hash,
        }) if committed_tip_hash == committed_tip && proof_block_hash == another_tip
    ));
}
#[test]
fn finality_attestation_fails_closed_when_requested_tip_races_state_view() {
    require_exact_durable_tip_height(7, 7).expect("same immutable tip height");
    assert!(matches!(
        require_exact_durable_tip_height(7, 8),
        Err(BridgeFinalityAttestationBuildError::HeightIsNotDurableTip {
            requested: 7,
            committed: 8,
        })
    ));
}
#[test]
fn finality_attestation_requires_exact_state_view_genesis_hash() {
    let committed_genesis = BlockHeader::new(
        NonZeroU64::new(1).expect("non-zero height"),
        None,
        None,
        0,
        0,
    )
    .hash();
    let substituted_genesis = BlockHeader::new(
        NonZeroU64::new(1).expect("non-zero height"),
        None,
        None,
        1,
        0,
    )
    .hash();
    require_finality_proof_at_committed_genesis(committed_genesis, committed_genesis)
        .expect("exact genesis must bind");
    assert!(matches!(
        require_finality_proof_at_committed_genesis(
            committed_genesis,
            substituted_genesis,
        ),
        Err(BridgeFinalityAttestationBuildError::GenesisFinalityMismatch {
            committed_genesis_hash,
            proof_block_hash,
        }) if committed_genesis_hash == committed_genesis
            && proof_block_hash == substituted_genesis
    ));
}
fn canonical_test_sccp_payload_bytes(payload: &SccpPayloadV1) -> Vec<u8> {
    iroha_sccp::canonical_sccp_payload_bytes(payload)
        .expect("valid SCCP bridge fixture payload encodes")
}
fn canonical_test_transfer_payload_bytes(payload: &iroha_sccp::TransferPayloadV1) -> Vec<u8> {
    iroha_sccp::canonical_transfer_payload_bytes(payload)
        .expect("valid SCCP transfer fixture payload encodes")
}
#[test]
fn checked_keypair_helpers_preserve_requested_algorithm() {
    assert_eq!(checked_keypair().algorithm(), Algorithm::default());
    assert_eq!(checked_bls_keypair().algorithm(), Algorithm::BlsNormal);
}
#[derive(Clone)]
struct TestSccpFinalityState {
    network_id: NetworkId,
    retained_header: Option<BlockHeader>,
    messages: Vec<ValidatedSccpOutboundMessageProjectionV1>,
    artifact: Option<V2FinalityArtifact>,
    artifact_error: Option<String>,
}
impl BridgeStateReadOnly for TestSccpFinalityState {
    fn bridge_network_id(&self) -> &NetworkId {
        &self.network_id
    }
    fn bridge_verified_v2_finality_artifact(
        &self,
        height: u64,
    ) -> Result<Option<VerifiedV2FinalityArtifact>, String> {
        if let Some(error) = &self.artifact_error {
            return Err(error.clone());
        }
        self.artifact
            .as_ref()
            .filter(|artifact| artifact.height == height)
            .cloned()
            .zip(self.retained_header.clone())
            .map(|(artifact, header)| {
                VerifiedV2FinalityArtifact::verify_for_header(header, artifact)
            })
            .transpose()
            .map_err(|error| error.to_string())
    }
    fn bridge_verified_v2_finality_with_sccp_archive(
        &self,
        height: u64,
    ) -> Result<
        Option<(
            VerifiedV2FinalityArtifact,
            Vec<ValidatedSccpOutboundMessageProjectionV1>,
        )>,
        String,
    > {
        Ok(self
            .bridge_verified_v2_finality_artifact(height)?
            .map(|verified| (verified, self.messages.clone())))
    }
}
fn test_sccp_projections_from_block(
    block: &SignedBlock,
) -> Vec<ValidatedSccpOutboundMessageProjectionV1> {
    collect_sccp_messages_from_signed_block(block)
        .into_iter()
        .enumerate()
        .map(
            |(index, message)| ValidatedSccpOutboundMessageProjectionV1 {
                commitment_index: u32::try_from(index).expect("test SCCP index fits u32"),
                context: message.context,
                payload: message.payload,
                commitment: message.commitment,
            },
        )
        .collect()
}
fn sample_sccp_projection_set(count: u64) -> Vec<ValidatedSccpOutboundMessageProjectionV1> {
    let payloads = (0..count)
        .map(|nonce| {
            canonical_test_sccp_payload_bytes(&sample_transfer_payload(nonce + 1, [0x22; 20]))
        })
        .collect::<Vec<_>>();
    let (block, _) = signed_block_with_sccp_payloads(&payloads, 1);
    test_sccp_projections_from_block(&block)
}
fn projection_root(messages: &[ValidatedSccpOutboundMessageProjectionV1]) -> [u8; 32] {
    iroha_sccp::commitment_merkle_root(
        &messages
            .iter()
            .map(|message| message.commitment.clone())
            .collect::<Vec<_>>(),
    )
    .expect("nonempty test projection has a Merkle root")
}
#[test]
fn finalized_projection_accepts_only_canonical_commitment_order() {
    let messages = sample_sccp_projection_set(3);
    let root = projection_root(&messages);
    let (validated_root, validated) =
        validate_sccp_outbound_projection_against_root(1, Some(root), messages.clone())
            .expect("canonical projection validates")
            .expect("nonempty projection is returned");
    assert_eq!(validated_root, root);
    assert_eq!(validated, messages);
}
#[test]
fn finalized_projection_rejects_reorder_gap_and_coordinated_index_swap() {
    let messages = sample_sccp_projection_set(3);
    let root = projection_root(&messages);
    let mut reordered = messages.clone();
    reordered.reverse();
    let error = validate_sccp_outbound_projection_against_root(1, Some(root), reordered)
        .expect_err("retained indices must reject reordered storage output");
    assert!(error.contains("dense commitment order"), "{error}");
    let mut gap = messages.clone();
    gap[1].commitment_index = 2;
    let error = validate_sccp_outbound_projection_against_root(1, Some(root), gap)
        .expect_err("a commitment-index gap must fail closed");
    assert!(error.contains("expected 1, found 2"), "{error}");
    let mut coordinated_swap = messages;
    coordinated_swap.swap(0, 2);
    for (index, message) in coordinated_swap.iter_mut().enumerate() {
        message.commitment_index = u32::try_from(index).expect("small test index");
    }
    let error = validate_sccp_outbound_projection_against_root(1, Some(root), coordinated_swap)
        .expect_err("rewriting indices cannot rewrite the finalized Merkle order");
    assert!(error.contains("reconstructs root"), "{error}");
}
#[test]
fn finalized_projection_rejects_duplicate_substituted_omitted_and_extra_messages() {
    let messages = sample_sccp_projection_set(3);
    let root = projection_root(&messages);
    let mut duplicate = messages.clone();
    duplicate[1] = duplicate[0].clone();
    duplicate[1].commitment_index = 1;
    let duplicate_root = projection_root(&duplicate);
    let error = validate_sccp_outbound_projection_against_root(1, Some(duplicate_root), duplicate)
        .expect_err("duplicate message identifiers must fail before root acceptance");
    assert!(error.contains("repeats message identifier"), "{error}");
    let mut substituted = messages.clone();
    substituted[1].commitment.message_id[0] ^= 1;
    let error = validate_sccp_outbound_projection_against_root(1, Some(root), substituted)
        .expect_err("payload-independent commitment substitution must fail closed");
    assert!(
        error.contains("substituted context, payload, or commitment"),
        "{error}"
    );
    let mut omitted = messages.clone();
    omitted.pop();
    let error = validate_sccp_outbound_projection_against_root(1, Some(root), omitted)
        .expect_err("omitting a finalized message must change the root");
    assert!(error.contains("reconstructs root"), "{error}");
    let extra = sample_sccp_projection_set(4);
    let error = validate_sccp_outbound_projection_against_root(1, Some(root), extra)
        .expect_err("appending a message must change the finalized root");
    assert!(error.contains("reconstructs root"), "{error}");
}
#[test]
fn finalized_projection_enforces_empty_root_equivalence_and_fixed_bound() {
    assert_eq!(
        validate_sccp_outbound_projection_against_root(8, None, Vec::new())
            .expect("empty rootless block validates"),
        None
    );
    let error = validate_sccp_outbound_projection_against_root(8, Some([0xAA; 32]), Vec::new())
        .expect_err("a rooted finalized header cannot have an empty projection");
    assert!(error.contains("commits a root"), "{error}");
    let one = sample_sccp_projection_set(1);
    let error = validate_sccp_outbound_projection_against_root(8, None, one.clone())
        .expect_err("a rootless finalized header cannot have an outbox record");
    assert!(error.contains("has no commitment root"), "{error}");
    let over_limit =
        vec![
            one[0].clone();
            usize::try_from(iroha_data_model::bridge::SCCP_OUTBOUND_MESSAGES_MAX_PER_BLOCK_V1 + 1,)
                .expect("protocol bound fits usize")
        ];
    let error = validate_sccp_outbound_projection_against_root(8, Some([0xBB; 32]), over_limit)
        .expect_err("the validator must reject before processing an oversized vector");
    assert!(
        error.contains("exceeding the fixed 512-message bound"),
        "{error}"
    );
}
fn sample_transfer_payload(nonce: u64, recipient: [u8; 20]) -> SccpPayloadV1 {
    SccpPayloadV1::Transfer(iroha_sccp::TransferPayloadV1 {
        version: 1,
        source_domain: iroha_sccp::SCCP_DOMAIN_SORA,
        dest_domain: iroha_sccp::SCCP_DOMAIN_ETH,
        nonce,
        route_revision: 1,
        asset_home_domain: iroha_sccp::SCCP_DOMAIN_SORA,
        asset_id_codec: iroha_sccp::SCCP_CODEC_CANONICAL_TEXT,
        asset_id: b"xor".to_vec(),
        amount: 77,
        sender_codec: iroha_sccp::SCCP_CODEC_CANONICAL_TEXT,
        sender: b"sora:bridge".to_vec(),
        recipient_codec: iroha_sccp::SCCP_CODEC_EVM_ADDRESS20,
        recipient: recipient.to_vec(),
        route_id_codec: iroha_sccp::SCCP_CODEC_CANONICAL_TEXT,
        route_id: iroha_sccp::SCCP_TAIRA_ETH_XOR_ROUTE_ID_V1
            .as_bytes()
            .to_vec(),
    })
}
#[test]
fn outbound_context_fixture_maps_every_supported_remote_domain_exactly() {
    use iroha_data_model::bridge::SccpNetworkV1;

    for (domain, expected) in [
        (iroha_sccp::SCCP_DOMAIN_ETH, SccpNetworkV1::EthereumMainnet),
        (iroha_sccp::SCCP_DOMAIN_BSC, SccpNetworkV1::BscMainnet),
        (iroha_sccp::SCCP_DOMAIN_TON, SccpNetworkV1::TonMainnet),
        (iroha_sccp::SCCP_DOMAIN_TRON, SccpNetworkV1::TronMainnet),
    ] {
        assert_eq!(test_sccp_target_network_for_domain(domain), expected);
    }
    assert!(
        std::panic::catch_unwind(|| test_sccp_target_network_for_domain(u32::MAX)).is_err(),
        "unknown domains must not silently inherit an Ethereum test context",
    );
}
fn non_sora_source_transfer_payload(nonce: u64) -> SccpPayloadV1 {
    SccpPayloadV1::Transfer(iroha_sccp::TransferPayloadV1 {
        version: 1,
        source_domain: iroha_sccp::SCCP_DOMAIN_ETH,
        dest_domain: iroha_sccp::SCCP_DOMAIN_SORA,
        nonce,
        route_revision: 1,
        asset_home_domain: iroha_sccp::SCCP_DOMAIN_SORA,
        asset_id_codec: iroha_sccp::SCCP_CODEC_CANONICAL_TEXT,
        asset_id: b"xor".to_vec(),
        amount: 77,
        sender_codec: iroha_sccp::SCCP_CODEC_EVM_ADDRESS20,
        sender: [0x22; 20].to_vec(),
        recipient_codec: iroha_sccp::SCCP_CODEC_CANONICAL_TEXT,
        recipient: b"sora:recipient".to_vec(),
        route_id_codec: iroha_sccp::SCCP_CODEC_CANONICAL_TEXT,
        route_id: iroha_sccp::SCCP_TAIRA_ETH_XOR_ROUTE_ID_V1
            .as_bytes()
            .to_vec(),
    })
}
#[test]
fn durable_outbound_record_retains_and_revalidates_exact_canonical_payload() {
    let payload = sample_transfer_payload(41, [0x31; 20]);
    let payload_bytes = canonical_test_sccp_payload_bytes(&payload);
    let context = test_sccp_outbound_context_for_payload_bytes(&payload_bytes);
    let validated = validate_recorded_sccp_message_payload_bytes(context, &payload_bytes)
        .expect("exact outbound payload validates");
    let record = validated
        .outbound_record(9, 3)
        .expect("validated payload forms a durable record");
    assert_eq!(record.payload_bytes, payload_bytes);
    assert_eq!(
        record.payload_hash,
        iroha_sccp::payload_hash(&payload_bytes)
    );
    let projection = validate_sccp_outbound_message_record_v1(&validated.key, &record)
        .expect("durable record fully revalidates");
    assert_eq!(projection.context, validated.context);
    assert_eq!(projection.commitment_index, 3);
    assert_eq!(projection.payload, validated.payload);
    assert_eq!(projection.commitment, validated.commitment);
    assert!(validated.outbound_record(0, 0).is_none());
    assert!(
        validated
            .outbound_record(
                9,
                iroha_data_model::bridge::SCCP_OUTBOUND_MESSAGES_MAX_PER_BLOCK_V1,
            )
            .is_none()
    );
}
#[test]
fn durable_outbound_record_rejects_payload_malleability_amplification_and_identity_drift() {
    let payload = sample_transfer_payload(42, [0x32; 20]);
    let payload_bytes = canonical_test_sccp_payload_bytes(&payload);
    let context = test_sccp_outbound_context_for_payload_bytes(&payload_bytes);
    let validated = validate_recorded_sccp_message_payload_bytes(context, &payload_bytes)
        .expect("exact outbound payload validates");
    let record = validated
        .outbound_record(10, 0)
        .expect("validated payload forms a durable record");
    let mut malformed = record.clone();
    malformed.payload_bytes[0] ^= 0x7f;
    let mut trailing_alias = record.clone();
    trailing_alias.payload_bytes.push(0);
    let mut oversized = record.clone();
    oversized.payload_bytes =
        vec![0xA5; iroha_data_model::bridge::SCCP_OUTBOUND_MESSAGE_MAX_PAYLOAD_BYTES_V1 + 1];
    let mut wrong_hash = record.clone();
    wrong_hash.payload_hash = [0xA6; 32];
    let wrong_key = SccpOutboundMessageKeyV1 {
        message_id: [0xA7; 32],
        ..validated.key
    };
    let wrong_lane_key = SccpOutboundMessageKeyV1 {
        lane: iroha_data_model::bridge::SccpLaneIdV1 {
            source: iroha_data_model::bridge::SccpNetworkV1::SoraTaira,
            target: iroha_data_model::bridge::SccpNetworkV1::BscMainnet,
        },
        ..validated.key
    };
    let mut aliased_asset_payload = payload;
    let SccpPayloadV1::Transfer(transfer) = &mut aliased_asset_payload;
    transfer.asset_id = b"xor#scope".to_vec();
    let aliased_asset_bytes = canonical_test_sccp_payload_bytes(&aliased_asset_payload);
    let aliased_asset_key = SccpOutboundMessageKeyV1::new(
        context.lane,
        iroha_sccp::sccp_message_id(context.lane, &aliased_asset_payload)
            .expect("scoped-asset payload remains structurally lane-bound"),
    )
    .expect("scoped-asset payload forms a structural key");
    let aliased_asset_record = iroha_data_model::bridge::SccpOutboundPendingMessageRecordV1 {
        destination_binding_hash: context.destination_binding_hash,
        route_configuration_hash: context.route_configuration_hash,
        payload_hash: iroha_sccp::payload_hash(&aliased_asset_bytes),
        payload_bytes: aliased_asset_bytes,
        recorded_at_height: 10,
        commitment_index: 0,
    };
    assert!(aliased_asset_record.is_well_formed_for_key(&aliased_asset_key));
    for (key, hostile) in [
        (validated.key, malformed),
        (validated.key, trailing_alias),
        (validated.key, oversized),
        (validated.key, wrong_hash),
        (wrong_key, record.clone()),
        (wrong_lane_key, record),
        (aliased_asset_key, aliased_asset_record),
    ] {
        assert!(
            validate_sccp_outbound_message_record_v1(&key, &hostile).is_none(),
            "hostile durable evidence unexpectedly validated: {hostile:?}"
        );
    }
}
#[test]
fn outbound_commitment_index_allocation_is_dense_bounded_and_rollback_safe() {
    use mv::storage::{Storage, StorageReadOnly};
    let index_key = |index: u32, id: u32| {
        let mut message_id = [0_u8; 32];
        message_id[..4].copy_from_slice(&id.to_le_bytes());
        iroha_data_model::bridge::SccpOutboundMessageIndexKeyV1 {
            recorded_at_height: 9,
            commitment_index: index,
            lane: iroha_data_model::bridge::SccpLaneIdV1 {
                source: iroha_data_model::bridge::SccpNetworkV1::SoraTaira,
                target: iroha_data_model::bridge::SccpNetworkV1::EthereumMainnet,
            },
            message_id,
        }
    };
    let storage = Storage::new();
    let mut block = storage.block();
    assert_eq!(
        next_sccp_outbound_commitment_index(&block, 9).expect("empty dense index"),
        Some(0)
    );
    {
        let mut transaction = block.transaction();
        transaction.insert(index_key(0, 1), ());
        assert_eq!(
            next_sccp_outbound_commitment_index(&transaction, 9)
                .expect("transaction sees its staged index"),
            Some(1)
        );
    }
    assert!(
        block.is_empty(),
        "dropped transaction must revert its index"
    );
    assert_eq!(
        next_sccp_outbound_commitment_index(&block, 9).expect("reverted index is reusable"),
        Some(0)
    );
    for index in 0..iroha_data_model::bridge::SCCP_OUTBOUND_MESSAGES_MAX_PER_BLOCK_V1 {
        block.insert(index_key(index, index + 1), ());
    }
    assert_eq!(
        next_sccp_outbound_commitment_index(&block, 9).expect("exactly full dense index"),
        None
    );
    block.insert(
        index_key(
            iroha_data_model::bridge::SCCP_OUTBOUND_MESSAGES_MAX_PER_BLOCK_V1,
            513,
        ),
        (),
    );
    assert!(next_sccp_outbound_commitment_index(&block, 9).is_err());
    let gap = Storage::new();
    let mut gap_block = gap.block();
    gap_block.insert(index_key(1, 1), ());
    assert!(next_sccp_outbound_commitment_index(&gap_block, 9).is_err());
    let duplicate = Storage::new();
    let mut duplicate_block = duplicate.block();
    duplicate_block.insert(index_key(0, 1), ());
    duplicate_block.insert(index_key(0, 2), ());
    assert!(next_sccp_outbound_commitment_index(&duplicate_block, 9).is_err());
}
fn signed_transaction_with_executable(executable: Executable) -> SignedTransaction {
    let keypair = checked_keypair();
    let authority = AccountId::new(keypair.public_key().clone());
    TransactionBuilder::new(
        bridge_test_network_id(b"bridge SCCP transaction genesis"),
        authority,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_executable(executable)
    .sign(keypair.private_key())
}
fn accepted_transaction_with_sccp_payload(payload: Vec<u8>) -> AcceptedTransaction<'static> {
    let tx =
        signed_transaction_with_executable(ivm_proved_with_overlay(vec![InstructionBox::from(
            crate::bridge::test_record_sccp_message(payload),
        )]));
    AcceptedTransaction::new_unchecked(Cow::Owned(tx))
}
fn sealed_commitment_entrypoint() -> TransactionEntrypoint {
    let keypair = checked_keypair();
    let network_id = bridge_test_network_id(b"bridge SCCP sealed-index genesis");
    let authority = AccountId::new(keypair.public_key().clone());
    let inner_tx = TransactionBuilder::new(
        network_id,
        authority.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .sign(keypair.private_key());
    let commitment = iroha_data_model::transaction::signed::compute_sealed_transaction_commitment(
        &network_id,
        &inner_tx,
        [0x57; 32],
        5,
    );
    let payload = iroha_data_model::transaction::signed::SealedTransactionCommitmentPayload {
        network_id,
        authority,
        commitment,
        reveal_after_height: 2,
        reveal_deadline_height: 5,
        nonce: None,
    };
    TransactionEntrypoint::SealedCommitment(
        iroha_data_model::transaction::signed::SignedSealedTransactionCommitment::sign(
            payload,
            keypair.private_key(),
        ),
    )
}
fn sealed_sccp_record_entrypoints(payload: Vec<u8>) -> [TransactionEntrypoint; 2] {
    let keypair = checked_keypair();
    let network_id = bridge_test_network_id(b"bridge SCCP sealed-record genesis");
    let authority = AccountId::new(keypair.public_key().clone());
    let signed = TransactionBuilder::new(
        network_id,
        authority.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_executable(ivm_proved_with_overlay(vec![InstructionBox::from(
        crate::bridge::test_record_sccp_message(payload),
    )]))
    .sign(keypair.private_key());
    let salt = [0x58; 32];
    let reveal_deadline_height = 8;
    let commitment = iroha_data_model::transaction::signed::compute_sealed_transaction_commitment(
        &network_id,
        &signed,
        salt,
        reveal_deadline_height,
    );
    let commitment_payload =
        iroha_data_model::transaction::signed::SealedTransactionCommitmentPayload {
            network_id,
            authority,
            commitment,
            reveal_after_height: 4,
            reveal_deadline_height,
            nonce: None,
        };
    let signed_commitment =
        iroha_data_model::transaction::signed::SignedSealedTransactionCommitment::sign(
            commitment_payload,
            keypair.private_key(),
        );
    let reveal = iroha_data_model::transaction::signed::SealedTransactionReveal::new(
        commitment, signed, salt,
    );
    [
        TransactionEntrypoint::SealedCommitment(signed_commitment),
        TransactionEntrypoint::SealedReveal(reveal),
    ]
}
fn ivm_proved_with_overlay(instructions: Vec<InstructionBox>) -> Executable {
    Executable::IvmProved(IvmProved {
        bytecode: IvmBytecode::from_compiled(vec![0x01, 0x02, 0x03]),
        overlay: instructions.into(),
        events_commitment: Hash::new(b"events"),
        gas_policy_commitment: Hash::new(b"gas"),
    })
}
fn replace_finalized_test_block_signature(block: &mut SignedBlock, signer: &KeyPair) {
    let signature = BlockSignature::new(
        0,
        SignatureOf::try_from_hash(signer.private_key(), block.hash())
            .expect("sign finalized test block header"),
    );
    block
        .replace_signatures([signature].into_iter().collect())
        .expect("replace provisional test block signature");
    block
        .signatures()
        .next()
        .expect("finalized test block signature")
        .signature()
        .verify_hash(signer.public_key(), block.hash())
        .expect("finalized test block signature verifies");
}
fn attach_test_outputs(block: &mut SignedBlock, outputs: Vec<ExecutionOutputV1>) {
    let header = block.header();
    let proposal = block.canonical_resultless_proposal();
    block
        .validate_proposal_commitments()
        .expect("fixture proposal commitments");
    block
        .set_execution_outputs(
            outputs,
            // Structural fixture only: no State fragments are executed by this helper.
            0,
            BTreeMap::new(),
            Vec::new(),
            iroha_data_model::nexus::AxtPolicySnapshot::default(),
            BTreeSet::new(),
            Vec::new(),
            &ExecutionOutputLimits {
                max_outputs: 4096,
                max_output_bytes: 16 * 1024 * 1024,
                max_total_output_bytes: 64 * 1024 * 1024,
                max_executed_wire_bytes: 256 * 1024 * 1024,
            },
        )
        .expect("fixture complete typed outputs");
    assert_eq!(block.header(), header);
    assert_eq!(block.canonical_resultless_proposal(), proposal);
    block
        .validate_output_merkle_cache()
        .expect("fixture exact output tree");
}
fn attach_test_network_results(block: &mut SignedBlock, results: Vec<TransactionResultInner>) {
    assert_eq!(results.len(), block.network_entrypoint_count());
    attach_test_outputs(
        block,
        results
            .into_iter()
            .enumerate()
            .map(|(index, result)| {
                ExecutionOutputV1::Network(NetworkExecutionOutputV1 {
                    input_index: u32::try_from(index).expect("fixture index"),
                    result: result.into(),
                    completions: Vec::new(),
                })
            })
            .collect(),
    );
}
fn test_internal_output(pipeline: bool, instructions: Vec<InstructionBox>) -> ExecutionOutputV1 {
    let trigger = TriggerUseV1 {
        trigger_id: if pipeline {
            "bridge_pipeline"
        } else {
            "bridge_time"
        }
        .parse()
        .unwrap(),
        registered_at_height: 0,
        action_hash: Hash::new(if pipeline {
            b"pipeline".as_slice()
        } else {
            b"time".as_slice()
        }),
    };
    let result = TransactionResult::new(Ok(vec![DataTriggerStep {
        id: trigger.trigger_id.clone(),
        instructions: ExecutionStep(instructions.into()),
    }]));
    let completions = vec![InvocationCompletionV1 {
        callback_index: 0,
        trigger_id: trigger.trigger_id.clone(),
        outcome: TriggerCompletedOutcome::Success,
    }];
    if pipeline {
        ExecutionOutputV1::Pipeline(PipelineExecutionOutputV1 {
            invocation: PipelineInvocationV1 {
                event: PipelineEventPositionV1::BlockApproved,
                candidate_index: 0,
                trigger,
            },
            result,
            failure_root: None,
            completions,
        })
    } else {
        ExecutionOutputV1::Time(TimeExecutionOutputV1 {
            invocation: TimeInvocationV1 {
                schedule_index: 0,
                event: TimeEvent {
                    interval: TimeInterval {
                        since_ms: 1,
                        length_ms: 1,
                    },
                },
                trigger,
            },
            result,
            failure_root: None,
            completions,
        })
    }
}
fn signed_block_with_transactions(
    transactions: Vec<SignedTransaction>,
    height: u64,
) -> SignedBlock {
    let keypair = checked_keypair();
    let entry_hashes: Vec<_> = transactions
        .iter()
        .map(SignedTransaction::hash_as_entrypoint)
        .collect();
    let header = BlockHeader::new(
        NonZeroU64::new(height).expect("non-zero height"),
        None,
        iroha_crypto::MerkleTree::root_from_typed_leaves(
            transactions
                .iter()
                .map(SignedTransaction::hash_as_entrypoint),
        ),
        0,
        0,
    );
    let signature = BlockSignature::new(
        0,
        SignatureOf::try_from_hash(keypair.private_key(), header.hash())
            .expect("test block signing should succeed"),
    );
    let mut block = SignedBlock::presigned(signature, header, transactions);
    let results =
        std::iter::repeat_with(|| TransactionResultInner::Ok(DataTriggerSequence::default()))
            .take(entry_hashes.len())
            .collect();
    attach_test_network_results(&mut block, results);
    replace_finalized_test_block_signature(&mut block, &keypair);
    block
}
fn signed_block_without_results(transactions: Vec<SignedTransaction>, height: u64) -> SignedBlock {
    let keypair = checked_keypair();
    let header = BlockHeader::new(
        NonZeroU64::new(height).expect("non-zero height"),
        None,
        iroha_crypto::MerkleTree::root_from_typed_leaves(
            transactions
                .iter()
                .map(SignedTransaction::hash_as_entrypoint),
        ),
        0,
        0,
    );
    let signature = BlockSignature::new(
        0,
        SignatureOf::try_from_hash(keypair.private_key(), header.hash())
            .expect("test block signing should succeed"),
    );
    SignedBlock::presigned(signature, header, transactions)
}
fn signed_block_with_sccp_payloads(
    payloads: &[Vec<u8>],
    height: u64,
) -> (SignedBlock, Vec<SccpPayloadV1>) {
    let keypair = checked_keypair();
    let authority = AccountId::new(keypair.public_key().clone());
    let decoded_payloads: Vec<_> = payloads
        .iter()
        .filter_map(|payload| iroha_sccp::decode_canonical_sccp_payload_bytes(payload))
        .collect();
    let instructions: Vec<InstructionBox> = payloads
        .iter()
        .cloned()
        .map(crate::bridge::test_record_sccp_message)
        .map(InstructionBox::from)
        .collect();
    let tx = TransactionBuilder::new(
        bridge_test_network_id(b"bridge SCCP block transaction genesis"),
        authority,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_executable(ivm_proved_with_overlay(instructions))
    .sign(keypair.private_key());
    let header = BlockHeader::new(
        NonZeroU64::new(height).expect("non-zero height"),
        None,
        iroha_crypto::MerkleTree::root_from_typed_leaves([tx.hash_as_entrypoint()]),
        0,
        0,
    );
    let signature = BlockSignature::new(
        0,
        SignatureOf::try_from_hash(keypair.private_key(), header.hash())
            .expect("test block signing should succeed"),
    );
    let mut block = SignedBlock::presigned(signature, header, vec![tx]);
    attach_test_network_results(
        &mut block,
        vec![TransactionResultInner::Ok(DataTriggerSequence::default())],
    );
    replace_finalized_test_block_signature(&mut block, &keypair);
    (block, decoded_payloads)
}
fn persisted_state_for_exact_sccp_fixture(
    fixture: &iroha_sccp::SccpExactOutboundTestFixtureV1,
) -> (
    iroha_sccp::SccpExactOutboundTestFixtureV1,
    TestSccpFinalityState,
) {
    let provisional_finality =
        iroha_sccp::decode_taira_bridge_finality_proof(&fixture.bundle.finality_proof)
            .expect("exact provisional SCCP finality proof");
    let payload = canonical_test_sccp_payload_bytes(&fixture.bundle.payload);
    let instruction = crate::bridge::test_record_sccp_message(payload);
    assert_eq!(
        instruction.context, fixture.bundle.commitment.context,
        "exact local block instruction must preserve the bundle context"
    );
    let tx =
        signed_transaction_with_executable(ivm_proved_with_overlay(vec![InstructionBox::from(
            instruction,
        )]));
    let block_signer = checked_keypair();
    let template_header = provisional_finality.block_header;
    let mut provisional_header = BlockHeader::new(
        template_header.height(),
        template_header.prev_block_hash(),
        iroha_crypto::MerkleTree::root_from_typed_leaves([tx.hash_as_entrypoint()]),
        u64::try_from(template_header.creation_time().as_millis())
            .expect("fixture creation time fits u64"),
        template_header.view_change_index(),
    );
    provisional_header.set_sccp_commitment_root(template_header.sccp_commitment_root());
    let signature = BlockSignature::new(
        0,
        SignatureOf::try_from_hash(block_signer.private_key(), provisional_header.hash())
            .expect("fixture provisional local block signature"),
    );
    let mut block = SignedBlock::presigned(signature, provisional_header, vec![tx]);
    attach_test_network_results(
        &mut block,
        vec![TransactionResultInner::Ok(DataTriggerSequence::default())],
    );
    assert!(
        provisional_finality
            .finality_artifact
            .validate_for_header(&block.header())
            .is_err(),
        "a pre-finalization artifact must not authenticate the completed local block"
    );
    replace_finalized_test_block_signature(&mut block, &block_signer);
    validate_sccp_commitment_root_for_signed_block(&block)
        .expect("completed local block authenticates its exact SCCP message");
    let fixture = fixture.with_finalized_block(&block, None);
    let finality = iroha_sccp::decode_taira_bridge_finality_proof(&fixture.bundle.finality_proof)
        .expect("exact completed SCCP finality proof");
    assert_eq!(block.header(), finality.block_header);
    assert_eq!(block.hash(), finality.finality_artifact.block_hash);
    assert_eq!(
        fixture.request.public_inputs.finality_block_hash,
        <[u8; 32]>::from(Hash::from(block.hash()))
    );
    finality
        .finality_artifact
        .validate_for_header(&block.header())
        .expect("completed local finality artifact binds the exact block header");
    finality
        .finality_artifact
        .verify()
        .expect("completed local finality artifact is cryptographically valid");
    let messages = test_sccp_projections_from_block(&block);
    let state = TestSccpFinalityState {
        network_id: finality.finality_artifact.height_context.network_id,
        retained_header: Some(block.header()),
        messages,
        artifact: Some(finality.finality_artifact),
        artifact_error: None,
    };
    (fixture, state)
}
#[test]
fn parsed_destination_proof_binds_local_state_before_deriving_call() {
    let fixture = iroha_sccp::sccp_exact_outbound_test_fixture_v1();
    let (fixture, state) = persisted_state_for_exact_sccp_fixture(&fixture);
    iroha_sccp::reset_sccp_destination_proof_work_counters_v1();
    let parsed = iroha_sccp::parse_sccp_destination_proof_v1(&fixture.bridge_proof)
        .expect("exact destination proof parses");
    let trusted_finality =
        verify_sccp_parsed_destination_proof_against_local_state(&state, &parsed)
            .expect("parse-only destination proof must anchor to exact local v2 artifact");
    assert_eq!(
        iroha_sccp::sccp_destination_proof_work_counters_v1(),
        iroha_sccp::SccpDestinationProofWorkCountersV1 {
            artifact_framing_decodes: 1,
            bundle_decodes: 1,
            groth16_pairings: 0,
            bls_verifications: 0,
            bls12381_point_decodes: 0,
        },
        "local authority must be established before proof-controlled cryptography"
    );
    let call = iroha_sccp::verify_parsed_sccp_destination_proof_v1(
        parsed,
        &fixture.route,
        &trusted_finality,
    )
    .expect("locally anchored destination proof verifies against governed route");
    assert_eq!(call.public_inputs(), &fixture.request.public_inputs);
    assert_eq!(
        iroha_sccp::sccp_destination_proof_work_counters_v1(),
        iroha_sccp::SccpDestinationProofWorkCountersV1 {
            artifact_framing_decodes: 1,
            bundle_decodes: 1,
            groth16_pairings: 1,
            bls_verifications: 1,
            bls12381_point_decodes: 0,
        },
        "call derivation must perform exactly one pairing and one finality check"
    );
}
#[test]
fn verified_finality_builder_selects_ton_bls12381_request() {
    let fixture = iroha_sccp::sccp_exact_ton_outbound_test_fixture_v1();
    let finality = iroha_sccp::decode_taira_bridge_finality_proof(&fixture.bundle.finality_proof)
        .expect("exact TON fixture finality proof");
    let verified = VerifiedV2FinalityArtifact::verify_for_header(
        finality.block_header,
        finality.finality_artifact,
    )
    .expect("exact TON fixture finality verifies");
    assert!(
        build_sccp_groth16_bn254_proof_request_from_verified_finality_v1(
            &verified,
            &fixture.bundle,
            &fixture.route,
        )
        .is_none(),
        "TON routes must never be projected into the BN254 request type"
    );
    assert_eq!(
        build_sccp_destination_proof_request_from_verified_finality_v1(
            &verified,
            &fixture.bundle,
            &fixture.route,
        ),
        Some(iroha_sccp::SccpDestinationProofRequestV1::Groth16Bls12381(
            fixture.request,
        ))
    );
}
#[test]
fn verified_finality_derives_epoch_aware_sora_anchor_and_rejects_boundaries() {
    let genesis_fixture = iroha_sccp::sccp_exact_outbound_test_fixture_v1();
    let genesis_finality =
        iroha_sccp::decode_taira_bridge_finality_proof(&genesis_fixture.bundle.finality_proof)
            .expect("exact height-one finality proof");
    let genesis_verified = VerifiedV2FinalityArtifact::verify_for_header(
        genesis_finality.block_header,
        genesis_finality.finality_artifact,
    )
    .expect("exact height-one finality verifies");
    assert_eq!(
        genesis_verified.sccp_sora_finality_anchor_v1(),
        Err(SccpSoraFinalityAnchorBuildError::EpochZero)
    );

    let same_epoch_fixture = genesis_fixture.with_exact_finalized_successor();
    let same_epoch_finality =
        iroha_sccp::decode_taira_bridge_finality_proof(&same_epoch_fixture.bundle.finality_proof)
            .expect("exact same-epoch height-two finality proof");
    let same_epoch_verified = VerifiedV2FinalityArtifact::verify_for_header(
        same_epoch_finality.block_header,
        same_epoch_finality.finality_artifact,
    )
    .expect("exact same-epoch height-two finality verifies");
    assert_eq!(
        same_epoch_verified.sccp_sora_finality_anchor_v1(),
        Err(SccpSoraFinalityAnchorBuildError::EpochZero)
    );

    let fixture = genesis_fixture.with_exact_epoch_one_finalized_successor();
    let finality = iroha_sccp::decode_taira_bridge_finality_proof(&fixture.bundle.finality_proof)
        .expect("exact height-two finality proof");
    let verified = VerifiedV2FinalityArtifact::verify_for_header(
        finality.block_header,
        finality.finality_artifact,
    )
    .expect("exact height-two finality verifies");
    let anchor = verified
        .sccp_sora_finality_anchor_v1()
        .expect("ordinary verified finality derives an SCCP anchor");
    assert_eq!(anchor.epoch, 1);
    assert_eq!(anchor.epoch_end_height, 10);
    assert_ne!(anchor.roster_commitment, [0; 32]);
    assert_eq!(anchor.checkpoint_height, 2);
    assert_eq!(
        anchor.checkpoint_block_hash,
        <[u8; 32]>::from(Hash::from(verified.retained_header().hash()))
    );
    assert_eq!(
        anchor.checkpoint_context_id,
        <[u8; 32]>::from(Hash::from(verified.artifact().context_id().0))
    );
    assert_eq!(
        anchor.checkpoint_finality_artifact_hash,
        <[u8; 32]>::from(Hash::new(norito::codec::Encode::encode(
            verified.artifact(),
        )))
    );
    assert_eq!(
        iroha_data_model::bridge::canonical_sccp_sora_finality_anchor_bytes_v1(anchor)
            .expect("derived anchor has canonical bytes")
            .len(),
        188,
        "canonical epoch-aware SORA finality anchor wire length"
    );

    let assert_internal_boundary_rejected =
        |mutate: fn(&mut V2FinalityArtifact), expected: SccpSoraFinalityAnchorBuildError| {
            let header = verified.retained_header().clone();
            let mut artifact = verified.artifact().clone();
            mutate(&mut artifact);
            let boundary = VerifiedV2FinalityArtifact::from_kura_verified(header, artifact);
            assert_eq!(boundary.sccp_sora_finality_anchor_v1(), Err(expected));
        };
    assert_internal_boundary_rejected(
        |artifact| artifact.height_context.epoch = 0,
        SccpSoraFinalityAnchorBuildError::EpochZero,
    );
    assert_internal_boundary_rejected(
        |artifact| artifact.height_context.parent_commit_qc = None,
        SccpSoraFinalityAnchorBuildError::MissingParentCommitQc,
    );
    assert_internal_boundary_rejected(
        |artifact| artifact.height_context.epoch_end_height = artifact.height - 1,
        SccpSoraFinalityAnchorBuildError::CheckpointAfterEpochEnd,
    );
    assert_internal_boundary_rejected(
        |artifact| {
            artifact.height_context.snapshot_bootstrap = Some(
                iroha_data_model::block::consensus_v2::SnapshotBootstrapAnchor {
                    snapshot_height: artifact.height - 1,
                    snapshot_block_hash: artifact
                        .subject
                        .parent_block_hash
                        .expect("height-two artifact has a parent"),
                    snapshot_block_creation_time_ms: 1,
                    snapshot_state_hash: Hash::new(b"inadmissible SCCP snapshot boundary"),
                },
            );
        },
        SccpSoraFinalityAnchorBuildError::SnapshotBootstrap,
    );
    assert_internal_boundary_rejected(
        |artifact| {
            artifact.validator_set_pops.pop();
        },
        SccpSoraFinalityAnchorBuildError::InvalidAuthenticatedRoster,
    );
}
#[test]
fn sccp_finality_local_state_check_rejects_missing_retained_finality_record() {
    let fixture = iroha_sccp::sccp_exact_outbound_test_fixture_v1();
    let finality = iroha_sccp::decode_taira_bridge_finality_proof(&fixture.bundle.finality_proof)
        .expect("exact fixture finality proof");
    let state = TestSccpFinalityState {
        network_id: finality.finality_artifact.height_context.network_id,
        retained_header: None,
        messages: Vec::new(),
        artifact: None,
        artifact_error: None,
    };
    let err = verify_sccp_finality_proof_against_local_state(&state, &finality)
        .expect_err("unanchored SCCP finality must fail before local crypto");
    assert!(err.contains("artifact for height 1 not found"), "{err}");
}
#[test]
fn finality_builder_never_substitutes_an_adjacent_retained_height() {
    let fixture = iroha_sccp::sccp_exact_outbound_test_fixture_v1();
    let (_, state) = persisted_state_for_exact_sccp_fixture(&fixture);
    assert_eq!(
        build_finality_proof(&state, 2),
        Err(BridgeFinalityError::FinalityArtifactNotFound(2))
    );
    let error = validated_sccp_finalized_messages_at_height(&state, 2)
        .expect_err("an adjacent request must not reuse height-one finality/archive data");
    assert!(error.contains("artifact for height 2 not found"), "{error}");
}
#[test]
fn sccp_local_anchor_rejects_artifact_chain_and_record_substitution() {
    let fixture = iroha_sccp::sccp_exact_outbound_test_fixture_v1();
    let (fixture, base) = persisted_state_for_exact_sccp_fixture(&fixture);
    let finality = iroha_sccp::decode_taira_bridge_finality_proof(&fixture.bundle.finality_proof)
        .expect("exact completed fixture finality proof");
    let assert_rejected = |state: &TestSccpFinalityState, expected: &str| {
        let error = verify_sccp_finality_proof_against_local_state(state, &finality)
            .expect_err("adversarial local substitution must fail");
        assert!(
            error.contains(expected),
            "expected {expected:?}, got {error:?}"
        );
    };
    let mut attack = base.clone();
    attack.network_id =
        NetworkId::from_genesis_hash(iroha_crypto::HashOf::<BlockHeader>::from_untyped_unchecked(
            Hash::new(b"attacker bridge finality network"),
        ));
    assert_rejected(&attack, "network id");
    let mut attack = base.clone();
    attack.artifact = None;
    assert_rejected(
        &attack,
        "Sumeragi-v2 finality artifact for height 1 not found",
    );
    let mut attack = base.clone();
    attack.artifact_error = Some("corrupt sidecar".to_owned());
    assert_rejected(&attack, "corrupt sidecar");
    let mut attack = base.clone();
    attack
        .artifact
        .as_mut()
        .expect("base artifact")
        .commit_qc
        .aggregate_signature[0] ^= 1;
    assert_rejected(
        &attack,
        "invalid Sumeragi-v2 quorum-certificate aggregate signature",
    );
    let mut attack = base.clone();
    attack
        .artifact
        .as_mut()
        .expect("base artifact")
        .validator_set_pops[0][0] ^= 1;
    assert_rejected(
        &attack,
        "invalid Sumeragi-v2 proof of possession at roster index 0",
    );
    let hostile_payload =
        canonical_test_sccp_payload_bytes(&sample_transfer_payload(999, [0x44; 20]));
    let (hostile_block, _) = signed_block_with_sccp_payloads(&[hostile_payload], 1);
    let mut attack = base;
    attack.messages = test_sccp_projections_from_block(&hostile_block);
    assert_rejected(&attack, "reconstructs root");
}
#[test]
fn sccp_commitment_root_is_none_for_empty_messages() {
    assert_eq!(sccp_commitment_root_from_messages(&[]), None);
}
#[test]
fn sccp_commitment_root_matches_direct_merkle_root() {
    let payloads = vec![
        canonical_test_sccp_payload_bytes(&sample_transfer_payload(1, [0x22; 20])),
        canonical_test_sccp_payload_bytes(&sample_transfer_payload(2, [0x22; 20])),
    ];
    let (block, _) = signed_block_with_sccp_payloads(&payloads, 1);
    let messages = collect_sccp_messages_from_signed_block(&block);
    let commitments: Vec<_> = messages
        .iter()
        .map(|message| message.commitment.clone())
        .collect();
    assert_eq!(
        sccp_commitment_root_from_messages(&messages),
        iroha_sccp::commitment_merkle_root(&commitments)
    );
}
#[test]
fn committed_block_rejects_513_self_consistent_outbound_messages() {
    let payloads = (0_u64
        ..=u64::from(iroha_data_model::bridge::SCCP_OUTBOUND_MESSAGES_MAX_PER_BLOCK_V1))
        .map(|nonce| {
            canonical_test_sccp_payload_bytes(&sample_transfer_payload(nonce + 1, [0x22; 20]))
        })
        .collect::<Vec<_>>();
    let (mut block, _) = signed_block_with_sccp_payloads(&payloads, 1);
    let messages = collect_sccp_messages_from_signed_block(&block);
    let root = sccp_commitment_root_from_messages(&messages).expect("nonempty SCCP root");
    let outputs = block.execution_outputs().to_vec();
    block.set_sccp_commitment_root(Some(root));
    attach_test_outputs(&mut block, outputs);
    assert_eq!(
        validate_sccp_commitment_root_for_signed_block(&block),
        Err(SccpCommittedBlockValidationError::TooManyOutboundMessages {
            actual: 513,
            max: 512,
        })
    );
}
#[test]
fn collect_sccp_messages_from_block_without_results_keeps_preexecution_records() {
    let payload = canonical_test_sccp_payload_bytes(&sample_transfer_payload(15, [0x22; 20]));
    let tx =
        signed_transaction_with_executable(ivm_proved_with_overlay(vec![InstructionBox::from(
            crate::bridge::test_record_sccp_message(payload.clone()),
        )]));
    let block = signed_block_without_results(vec![tx], 13);
    assert!(!block.has_results());
    let messages = collect_sccp_messages_from_signed_block(&block);
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].tx_index, 0);
    assert_eq!(
        messages[0].payload,
        iroha_sccp::decode_canonical_sccp_payload_bytes(&payload).expect("payload decodes")
    );
}
#[test]
fn collect_sccp_messages_from_empty_accepted_transactions_is_empty() {
    assert!(collect_sccp_messages_from_accepted_transactions(&[]).is_empty());
}
#[test]
fn collect_sccp_messages_from_plain_instruction_executable() {
    let payload = canonical_test_sccp_payload_bytes(&sample_transfer_payload(12, [0x22; 20]));
    let tx = signed_transaction_with_executable(Executable::Instructions(
        vec![InstructionBox::from(
            crate::bridge::test_record_sccp_message(payload.clone()),
        )]
        .into(),
    ));
    let block = signed_block_with_transactions(vec![tx], 1);
    let messages = collect_sccp_messages_from_signed_block(&block);
    assert_eq!(messages.len(), 1);
    assert_eq!(
        messages[0].payload,
        iroha_sccp::decode_canonical_sccp_payload_bytes(&payload)
            .expect("direct record payload decodes")
    );
}
#[test]
fn collect_sccp_messages_from_block_preserves_payload_order() {
    let payloads = vec![
        canonical_test_sccp_payload_bytes(&sample_transfer_payload(1, [0x22; 20])),
        canonical_test_sccp_payload_bytes(&sample_transfer_payload(2, [0x22; 20])),
    ];
    let (block, decoded_payloads) = signed_block_with_sccp_payloads(&payloads, 1);
    let messages = collect_sccp_messages_from_signed_block(&block);
    assert_eq!(
        messages
            .iter()
            .map(|message| &message.payload)
            .collect::<Vec<_>>(),
        decoded_payloads.iter().collect::<Vec<_>>()
    );
    let commitments: Vec<_> = messages
        .iter()
        .map(|message| message.commitment.clone())
        .collect();
    let root = sccp_commitment_root_from_messages(&messages).expect("commitment root");
    let proof = iroha_sccp::commitment_merkle_proof(&commitments, 1).expect("proof");
    assert_eq!(
        iroha_sccp::merkle_root_from_commitment(&messages[1].commitment, &proof),
        root
    );
}
#[test]
fn committed_replay_extraction_derives_one_outbound_leaf_from_proved_execution() {
    let payload = sample_transfer_payload(301, [0x42; 20]);
    let payload_bytes = canonical_test_sccp_payload_bytes(&payload);
    let (block, _) = signed_block_with_sccp_payloads(&[payload_bytes.clone()], 1);
    let admissions = collect_sccp_replay_admissions_from_finalized_execution(&block, None)
        .expect("finalized proved execution rebuilds");
    assert_eq!(admissions.len(), 1);
    let admission = &admissions[0];
    let transaction = block
        .external_signed_transaction_ref_at(0)
        .expect("fixture contains one signed transaction");
    assert_eq!(
        admission.accumulator_id.boundary,
        SccpReplayBoundaryV1::SoraOutboundLock
    );
    assert_eq!(
        admission.accumulator_id.route_key.lane_id,
        SccpLaneIdV1 {
            source: iroha_data_model::bridge::SccpNetworkV1::EthereumMainnet,
            target: iroha_data_model::bridge::SccpNetworkV1::SoraTaira,
        }
    );
    assert_eq!(
        admission.domain.source_network,
        iroha_data_model::bridge::SccpNetworkV1::SoraTaira
    );
    assert_eq!(
        admission.domain.target_network,
        iroha_data_model::bridge::SccpNetworkV1::EthereumMainnet
    );
    assert_eq!(
        admission.record.replay_id,
        test_sccp_outbound_message_key(&payload).message_id
    );
    assert_eq!(
        admission.record.payload_sha256,
        <[u8; 32]>::from(sha2::Sha256::digest(payload_bytes))
    );
    assert_eq!(admission.record.amount, 77);
    assert_eq!(
        admission.record.principal,
        SccpReplayPrincipalV1::SoraAccount(transaction.authority().clone())
    );
    assert_eq!(
        admission.witness,
        iroha_data_model::bridge::SccpSparseMerkleWitnessV1::empty_shard()
    );
}
#[test]
fn committed_replay_extraction_rejects_multiple_mutations_in_one_transaction() {
    let payloads = [
        canonical_test_sccp_payload_bytes(&sample_transfer_payload(302, [0x42; 20])),
        canonical_test_sccp_payload_bytes(&sample_transfer_payload(303, [0x43; 20])),
    ];
    let (block, _) = signed_block_with_sccp_payloads(&payloads, 1);
    assert_eq!(
        collect_sccp_replay_admissions_from_finalized_execution(&block, None),
        Err(SccpReplayRebuildErrorV1::MultipleMutations)
    );
}
#[test]
fn committed_replay_extraction_ignores_rejected_execution() {
    let payload = canonical_test_sccp_payload_bytes(&sample_transfer_payload(304, [0x44; 20]));
    let (mut block, _) = signed_block_with_sccp_payloads(&[payload], 1);
    attach_test_network_results(
        &mut block,
        vec![TransactionResultInner::Err(
            iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
                iroha_data_model::ValidationFail::NotPermitted(
                    "rejected replay rebuild fixture".to_owned(),
                ),
            ),
        )],
    );
    assert!(
        collect_sccp_replay_admissions_from_finalized_execution(&block, None)
            .expect("rejected execution is structurally valid")
            .is_empty()
    );
}
#[test]
fn commitment_paths_bind_first_middle_last_execution_indices_not_key_order() {
    let candidates = (1..=5)
        .map(|nonce| canonical_test_sccp_payload_bytes(&sample_transfer_payload(nonce, [0x22; 20])))
        .collect::<Vec<_>>();
    let (candidate_block, _) = signed_block_with_sccp_payloads(&candidates, 1);
    let candidate_messages = collect_sccp_messages_from_signed_block(&candidate_block);
    let mut ordered = candidates
        .into_iter()
        .zip(candidate_messages)
        .collect::<Vec<_>>();
    ordered.sort_by(|(_, left), (_, right)| {
        right.commitment.message_id.cmp(&left.commitment.message_id)
    });
    let payloads = ordered
        .into_iter()
        .map(|(payload, _)| payload)
        .collect::<Vec<_>>();
    let (block, _) = signed_block_with_sccp_payloads(&payloads, 1);
    let messages = collect_sccp_messages_from_signed_block(&block);
    assert!(
        messages
            .windows(2)
            .all(|pair| pair[0].commitment.message_id > pair[1].commitment.message_id),
        "fixture execution order must deliberately oppose ascending replay-key order"
    );
    let commitments = messages
        .iter()
        .map(|message| message.commitment.clone())
        .collect::<Vec<_>>();
    let root = iroha_sccp::commitment_merkle_root(&commitments).expect("five-message root");
    for index in [0, 2, 4] {
        let proof = iroha_sccp::commitment_merkle_proof(&commitments, index)
            .expect("first/middle/last path exists");
        assert_eq!(
            iroha_sccp::merkle_root_from_commitment(&messages[index].commitment, &proof),
            root,
            "execution-index path {index} must reconstruct the finalized root"
        );
    }
}
#[test]
fn collect_sccp_messages_rejects_unprefixed_ascii_hex_record_payload_bytes() {
    let expected_payload = sample_transfer_payload(6, [0x22; 20]);
    let payload = canonical_test_sccp_payload_bytes(&expected_payload);
    let encoded_payload = hex::encode(&payload).into_bytes();
    let (block, _) = signed_block_with_sccp_payloads(&[encoded_payload], 4);
    let messages = collect_sccp_messages_from_signed_block(&block);
    assert!(
        messages.is_empty(),
        "ASCII hex payload aliases must not be collected as SCCP records"
    );
}
#[test]
fn collect_sccp_messages_rejects_prefixed_ascii_hex_record_payload_bytes() {
    let expected_payload = sample_transfer_payload(7, [0x22; 20]);
    let payload = canonical_test_sccp_payload_bytes(&expected_payload);
    let encoded_payload = format!("0x{}", hex::encode(&payload)).into_bytes();
    let (block, _) = signed_block_with_sccp_payloads(&[encoded_payload], 4);
    let messages = collect_sccp_messages_from_signed_block(&block);
    assert!(
        messages.is_empty(),
        "prefixed ASCII hex payload aliases must not be collected as SCCP records"
    );
}
#[test]
fn collect_sccp_messages_rejects_ascii_hex_record_payload_aliases() {
    let expected_payload = sample_transfer_payload(8, [0x22; 20]);
    let payload = canonical_test_sccp_payload_bytes(&expected_payload);
    let lowercase_hex = hex::encode(&payload);
    let uppercase_hex = lowercase_hex.to_ascii_uppercase();
    let cases = [
        lowercase_hex.as_bytes().to_vec(),
        format!("0x{lowercase_hex}").into_bytes(),
        uppercase_hex.as_bytes().to_vec(),
        format!("0X{lowercase_hex}").into_bytes(),
        format!(" {lowercase_hex}").into_bytes(),
        format!("{lowercase_hex}\n").into_bytes(),
        format!("{lowercase_hex}0").into_bytes(),
        b"not-hex".to_vec(),
    ];
    for encoded_payload in cases {
        let (block, _) = signed_block_with_sccp_payloads(&[encoded_payload], 5);
        assert!(
            collect_sccp_messages_from_signed_block(&block).is_empty(),
            "SCCP hex record payload aliases must be ignored"
        );
    }
}
#[test]
fn collect_sccp_messages_ignores_ascii_hex_aliases_for_commitment_root() {
    let accepted_payload = sample_transfer_payload(9, [0x22; 20]);
    let accepted_bytes = canonical_test_sccp_payload_bytes(&accepted_payload);
    let rejected_payload = sample_transfer_payload(10, [0x22; 20]);
    let rejected_bytes = canonical_test_sccp_payload_bytes(&rejected_payload);
    let rejected_hex = hex::encode(&rejected_bytes);
    let uppercase_alias = rejected_hex.to_ascii_uppercase().into_bytes();
    let prefixed_alias = format!("0x{rejected_hex}").into_bytes();
    let padded_alias = format!("{rejected_hex}\n").into_bytes();
    let (block, _) = signed_block_with_sccp_payloads(
        &[
            uppercase_alias,
            accepted_bytes,
            prefixed_alias,
            padded_alias,
        ],
        6,
    );
    let messages = collect_sccp_messages_from_signed_block(&block);
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].payload, accepted_payload);
    let expected_commitment = test_sccp_hub_commitment(&accepted_payload);
    assert_eq!(
        sccp_commitment_root_from_messages(&messages),
        iroha_sccp::commitment_merkle_root(&[expected_commitment])
    );
}
#[test]
fn collect_sccp_messages_skips_undecodable_payloads() {
    let payloads = vec![
        canonical_test_sccp_payload_bytes(&sample_transfer_payload(3, [0x22; 20])),
        vec![0xff, 0x00, 0x01],
    ];
    let (block, decoded_payloads) = signed_block_with_sccp_payloads(&payloads, 2);
    let messages = collect_sccp_messages_from_signed_block(&block);
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].payload, decoded_payloads[0]);
}
#[test]
fn collect_sccp_messages_skips_non_sora_origin_payloads() {
    let inbound = SccpPayloadV1::Transfer(iroha_sccp::TransferPayloadV1 {
        version: 1,
        source_domain: iroha_sccp::SCCP_DOMAIN_ETH,
        dest_domain: iroha_sccp::SCCP_DOMAIN_SORA,
        nonce: 11,
        route_revision: 1,
        asset_home_domain: iroha_sccp::SCCP_DOMAIN_ETH,
        asset_id_codec: iroha_sccp::SCCP_CODEC_CANONICAL_TEXT,
        asset_id: b"weth#eth".to_vec(),
        amount: 10,
        sender_codec: iroha_sccp::SCCP_CODEC_EVM_ADDRESS20,
        sender: [0x22; 20].to_vec(),
        recipient_codec: iroha_sccp::SCCP_CODEC_CANONICAL_TEXT,
        recipient: b"alice@universal".to_vec(),
        route_id_codec: iroha_sccp::SCCP_CODEC_CANONICAL_TEXT,
        route_id: b"eth:sora:weth".to_vec(),
    });
    let (block, _) =
        signed_block_with_sccp_payloads(&[canonical_test_sccp_payload_bytes(&inbound)], 2);
    assert!(collect_sccp_messages_from_signed_block(&block).is_empty());
}
#[test]
fn collect_sccp_messages_skips_decodable_but_invalid_payloads() {
    let invalid = sample_transfer_payload(4, [0x22; 20]);
    let SccpPayloadV1::Transfer(mut invalid_transfer) = invalid;
    invalid_transfer.amount = 0;
    let invalid_payload = SccpPayloadV1::Transfer(invalid_transfer);
    assert!(
        iroha_sccp::decode_canonical_sccp_payload_bytes(&canonical_test_sccp_payload_bytes(
            &invalid_payload
        ))
        .is_some()
    );
    assert!(!iroha_sccp::verify_sccp_payload_structure(&invalid_payload));
    let valid_payload = sample_transfer_payload(5, [0x22; 20]);
    let payloads = vec![
        canonical_test_sccp_payload_bytes(&invalid_payload),
        canonical_test_sccp_payload_bytes(&valid_payload),
    ];
    let (block, _) = signed_block_with_sccp_payloads(&payloads, 3);
    let messages = collect_sccp_messages_from_signed_block(&block);
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].payload, valid_payload);
    assert_eq!(messages[0].instruction_index, 1);
}
#[test]
fn collect_sccp_messages_from_plain_ivm_executable_is_empty() {
    let tx = signed_transaction_with_executable(Executable::Ivm(IvmBytecode::from_compiled(vec![
        0x01, 0x02, 0x03,
    ])));
    let block = signed_block_with_transactions(vec![tx], 3);
    assert!(collect_sccp_messages_from_signed_block(&block).is_empty());
}
#[test]
fn collect_sccp_messages_from_contract_call_executable_is_empty() {
    let keypair = checked_keypair();
    let authority = AccountId::new(keypair.public_key().clone());
    let contract_address = ContractAddress::derive(
        &bridge_test_network_id(b"bridge SCCP transaction genesis"),
        &authority,
        0,
        DataSpaceId::UNIVERSAL,
    )
    .expect("derive contract address");
    let tx = signed_transaction_with_executable(Executable::ContractCall(ContractInvocation {
        contract_address,
        expected_code_hash: iroha_crypto::Hash::new(b"bridge-contract-code"),
        entrypoint: "bridge".to_string(),
        arguments: None,
    }));
    let block = signed_block_with_transactions(vec![tx], 4);
    assert!(collect_sccp_messages_from_signed_block(&block).is_empty());
}
#[test]
fn collect_sccp_messages_preserves_instruction_indices_after_skips() {
    let payloads = vec![
        canonical_test_sccp_payload_bytes(&sample_transfer_payload(4, [0x22; 20])),
        vec![0x00, 0x01, 0xff],
        canonical_test_sccp_payload_bytes(&sample_transfer_payload(5, [0x22; 20])),
    ];
    let (block, decoded_payloads) = signed_block_with_sccp_payloads(&payloads, 3);
    let messages = collect_sccp_messages_from_signed_block(&block);
    assert_eq!(messages.len(), 2);
    assert_eq!(
        messages
            .iter()
            .map(|message| (message.tx_index, message.instruction_index))
            .collect::<Vec<_>>(),
        vec![(0, 0), (0, 2)]
    );
    assert_eq!(messages[0].payload, decoded_payloads[0]);
    assert_eq!(messages[1].payload, decoded_payloads[1]);
}
#[test]
fn collect_sccp_messages_preserves_transaction_indices_across_block() {
    let first_payload = canonical_test_sccp_payload_bytes(&sample_transfer_payload(6, [0x22; 20]));
    let second_payload = canonical_test_sccp_payload_bytes(&sample_transfer_payload(7, [0x22; 20]));
    let ignored_tx =
        signed_transaction_with_executable(Executable::Ivm(IvmBytecode::from_compiled(vec![0xAA])));
    let first_tx =
        signed_transaction_with_executable(ivm_proved_with_overlay(vec![InstructionBox::from(
            crate::bridge::test_record_sccp_message(first_payload),
        )]));
    let second_tx =
        signed_transaction_with_executable(ivm_proved_with_overlay(vec![InstructionBox::from(
            crate::bridge::test_record_sccp_message(second_payload),
        )]));
    let block = signed_block_with_transactions(vec![ignored_tx, first_tx, second_tx], 5);
    let messages = collect_sccp_messages_from_signed_block(&block);
    assert_eq!(
        messages
            .iter()
            .map(|message| (message.tx_index, message.instruction_index))
            .collect::<Vec<_>>(),
        vec![(1, 0), (2, 0)]
    );
}
#[test]
fn collect_sccp_messages_from_accepted_transactions_skips_sealed_commitments() {
    let commitment_tx =
        AcceptedTransaction::new_unchecked_entrypoint(Cow::Owned(sealed_commitment_entrypoint()));
    let payload = canonical_test_sccp_payload_bytes(&sample_transfer_payload(6, [0x22; 20]));
    let external_tx =
        signed_transaction_with_executable(ivm_proved_with_overlay(vec![InstructionBox::from(
            crate::bridge::test_record_sccp_message(payload.clone()),
        )]));
    let external_tx = AcceptedTransaction::new_unchecked_entrypoint(Cow::Owned(
        TransactionEntrypoint::External(external_tx),
    ));
    let messages = collect_sccp_messages_from_accepted_transactions(&[commitment_tx, external_tx]);
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].tx_index, 1);
    assert_eq!(messages[0].instruction_index, 0);
    assert_eq!(
        messages[0].payload,
        iroha_sccp::decode_canonical_sccp_payload_bytes(&payload).expect("payload decodes")
    );
}
#[test]
fn collect_sccp_messages_from_accepted_transactions_includes_sealed_reveals() {
    let payload = canonical_test_sccp_payload_bytes(&sample_transfer_payload(7, [0x22; 20]));
    let [commitment, reveal] = sealed_sccp_record_entrypoints(payload.clone());
    let accepted_commitment = AcceptedTransaction::new_unchecked_entrypoint(Cow::Owned(commitment));
    let accepted_reveal = AcceptedTransaction::new_unchecked_entrypoint(Cow::Owned(reveal));
    let messages =
        collect_sccp_messages_from_accepted_transactions(&[accepted_commitment, accepted_reveal]);
    assert_eq!(messages.len(), 1);
    assert_eq!(
        messages[0].tx_index, 1,
        "sealed commitment entrypoints must be counted when preserving canonical indices"
    );
    assert_eq!(messages[0].instruction_index, 0);
    assert_eq!(
        messages[0].payload,
        iroha_sccp::decode_canonical_sccp_payload_bytes(&payload).expect("payload decodes")
    );
}
#[test]
fn collect_sccp_messages_from_accepted_transactions_deduplicates_outbound_keys() {
    let payload = canonical_test_sccp_payload_bytes(&sample_transfer_payload(8, [0x22; 20]));
    let first = accepted_transaction_with_sccp_payload(payload.clone());
    let second = accepted_transaction_with_sccp_payload(payload.clone());
    let messages = collect_sccp_messages_from_accepted_transactions(&[first, second]);
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].tx_index, 0);
    assert_eq!(messages[0].instruction_index, 0);
    assert_eq!(
        messages[0].payload,
        iroha_sccp::decode_canonical_sccp_payload_bytes(&payload).expect("payload decodes")
    );
}
#[test]
fn collect_sccp_messages_from_accepted_transactions_ignores_hex_aliases() {
    let payload = canonical_test_sccp_payload_bytes(&sample_transfer_payload(8, [0x22; 20]));
    let hex_alias = format!("0x{}", hex::encode(&payload)).into_bytes();
    for (first, second, expected_tx_index) in [
        (payload.clone(), hex_alias.clone(), 0),
        (hex_alias.clone(), payload.clone(), 1),
    ] {
        let first = accepted_transaction_with_sccp_payload(first);
        let second = accepted_transaction_with_sccp_payload(second);
        let messages = collect_sccp_messages_from_accepted_transactions(&[first, second]);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].tx_index, expected_tx_index);
        assert_eq!(messages[0].instruction_index, 0);
        assert_eq!(
            messages[0].payload,
            iroha_sccp::decode_canonical_sccp_payload_bytes(&payload).expect("payload decodes")
        );
    }
}
#[test]
fn collect_sccp_messages_from_accepted_transactions_filter_preserves_entry_indices() {
    let skipped_payload =
        canonical_test_sccp_payload_bytes(&sample_transfer_payload(8, [0x22; 20]));
    let included_payload =
        canonical_test_sccp_payload_bytes(&sample_transfer_payload(9, [0x22; 20]));
    let skipped = accepted_transaction_with_sccp_payload(skipped_payload);
    let included = accepted_transaction_with_sccp_payload(included_payload.clone());
    let messages = collect_new_sccp_messages_from_accepted_transactions_where(
        &[skipped, included],
        |tx_index| tx_index == 1,
        |_| false,
    );
    assert_eq!(messages.len(), 1);
    assert_eq!(
        messages[0].tx_index, 1,
        "route filtering must not renumber canonical transaction indices"
    );
    assert_eq!(messages[0].instruction_index, 0);
    assert_eq!(
        messages[0].payload,
        iroha_sccp::decode_canonical_sccp_payload_bytes(&included_payload)
            .expect("payload decodes")
    );
}
#[test]
fn collect_sccp_messages_from_accepted_transactions_skips_empty_outbound_route() {
    let mut payload = sample_transfer_payload(12, [0x22; 20]);
    let SccpPayloadV1::Transfer(transfer) = &mut payload;
    transfer.route_id.clear();
    let accepted =
        accepted_transaction_with_sccp_payload(canonical_test_sccp_payload_bytes(&payload));
    let messages = collect_sccp_messages_from_accepted_transactions(&[accepted]);
    assert!(
        messages.is_empty(),
        "proposal SCCP roots must not include records with empty route identifiers"
    );
}
#[test]
fn collect_sccp_messages_from_accepted_transactions_skips_malformed_outbound_asset_scope() {
    let mut payload = sample_transfer_payload(23, [0x22; 20]);
    let SccpPayloadV1::Transfer(transfer) = &mut payload;
    transfer.asset_id = b"xor#".to_vec();
    transfer.route_id = iroha_sccp::SCCP_TAIRA_ETH_XOR_ROUTE_ID_V1
        .as_bytes()
        .to_vec();
    let accepted =
        accepted_transaction_with_sccp_payload(canonical_test_sccp_payload_bytes(&payload));
    let messages = collect_sccp_messages_from_accepted_transactions(&[accepted]);
    assert!(
        messages.is_empty(),
        "proposal SCCP roots must not include asset-id aliases with empty scopes"
    );
}
#[test]
fn collect_sccp_messages_from_accepted_transactions_skips_scoped_outbound_asset_alias() {
    let mut payload = sample_transfer_payload(25, [0x22; 20]);
    let SccpPayloadV1::Transfer(transfer) = &mut payload;
    transfer.asset_id = b"xor#universal".to_vec();
    transfer.route_id = iroha_sccp::SCCP_TAIRA_ETH_XOR_ROUTE_ID_V1
        .as_bytes()
        .to_vec();
    let accepted =
        accepted_transaction_with_sccp_payload(canonical_test_sccp_payload_bytes(&payload));
    let messages = collect_sccp_messages_from_accepted_transactions(&[accepted]);
    assert!(
        messages.is_empty(),
        "proposal SCCP roots must not include scoped asset-id aliases"
    );
}
#[test]
fn collect_sccp_messages_from_accepted_transactions_deduplicates_same_overlay_key() {
    let payload = canonical_test_sccp_payload_bytes(&sample_transfer_payload(8, [0x22; 20]));
    let tx = signed_transaction_with_executable(ivm_proved_with_overlay(vec![
        InstructionBox::from(crate::bridge::test_record_sccp_message(payload.clone())),
        InstructionBox::from(crate::bridge::test_record_sccp_message(payload.clone())),
    ]));
    let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx));
    let messages = collect_sccp_messages_from_accepted_transactions(&[accepted]);
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].tx_index, 0);
    assert_eq!(messages[0].instruction_index, 0);
    assert_eq!(
        messages[0].payload,
        iroha_sccp::decode_canonical_sccp_payload_bytes(&payload).expect("payload decodes")
    );
}
#[test]
fn collect_new_sccp_messages_from_accepted_transactions_skips_existing_outbox_keys() {
    let payload = sample_transfer_payload(9, [0x22; 20]);
    let key = test_sccp_outbound_message_key(&payload);
    let accepted =
        accepted_transaction_with_sccp_payload(canonical_test_sccp_payload_bytes(&payload));
    let messages = collect_new_sccp_messages_from_accepted_transactions(&[accepted], |candidate| {
        candidate == &key
    });
    assert!(messages.is_empty());
}
#[test]
fn collect_sccp_messages_from_block_deduplicates_successful_duplicate_outbound_keys() {
    let payload = canonical_test_sccp_payload_bytes(&sample_transfer_payload(10, [0x22; 20]));
    let first_tx =
        signed_transaction_with_executable(ivm_proved_with_overlay(vec![InstructionBox::from(
            crate::bridge::test_record_sccp_message(payload.clone()),
        )]));
    let second_tx =
        signed_transaction_with_executable(ivm_proved_with_overlay(vec![InstructionBox::from(
            crate::bridge::test_record_sccp_message(payload.clone()),
        )]));
    let block = signed_block_with_transactions(vec![first_tx, second_tx], 9);
    let messages = collect_sccp_messages_from_signed_block(&block);
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].tx_index, 0);
    assert_eq!(
        messages[0].payload,
        iroha_sccp::decode_canonical_sccp_payload_bytes(&payload).expect("payload decodes")
    );
    let expected_commitment = test_sccp_hub_commitment(&messages[0].payload);
    assert_eq!(
        sccp_commitment_root_from_messages(&messages),
        iroha_sccp::commitment_merkle_root(&[expected_commitment])
    );
}
#[test]
fn collect_sccp_messages_from_block_ignores_hex_aliases() {
    let payload = canonical_test_sccp_payload_bytes(&sample_transfer_payload(11, [0x22; 20]));
    let encoded_payload = format!("0x{}", hex::encode(&payload)).into_bytes();
    let tx = signed_transaction_with_executable(ivm_proved_with_overlay(vec![
        InstructionBox::from(crate::bridge::test_record_sccp_message(encoded_payload)),
        InstructionBox::from(crate::bridge::test_record_sccp_message(payload.clone())),
    ]));
    let block = signed_block_with_transactions(vec![tx], 9);
    let messages = collect_sccp_messages_from_signed_block(&block);
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].tx_index, 0);
    assert_eq!(messages[0].instruction_index, 1);
    assert_eq!(
        messages[0].payload,
        iroha_sccp::decode_canonical_sccp_payload_bytes(&payload).expect("payload decodes")
    );
}
#[test]
fn local_sccp_finality_records_reject_duplicate_successful_outbound_keys() {
    let payload = sample_transfer_payload(12, [0x22; 20]);
    let payload_bytes = canonical_test_sccp_payload_bytes(&payload);
    let first_tx =
        signed_transaction_with_executable(ivm_proved_with_overlay(vec![InstructionBox::from(
            crate::bridge::test_record_sccp_message(payload_bytes.clone()),
        )]));
    let second_tx =
        signed_transaction_with_executable(ivm_proved_with_overlay(vec![InstructionBox::from(
            crate::bridge::test_record_sccp_message(payload_bytes),
        )]));
    let block = signed_block_with_transactions(vec![first_tx, second_tx], 9);
    let messages = collect_sccp_messages_from_signed_block(&block);
    let deduped_root =
        sccp_commitment_root_from_messages(&messages).expect("deduped commitment root");
    let err = validate_local_sccp_records_against_commitment_root(&block, deduped_root)
        .expect_err("duplicate successful SCCP records must reject before root acceptance");
    assert!(err.contains("duplicate outbound message"));
    assert!(err.contains(&hex::encode(
        test_sccp_outbound_message_key(&payload).message_id
    )));
}
#[test]
fn local_sccp_finality_records_reject_hex_alias_payload() {
    let payload = sample_transfer_payload(13, [0x22; 20]);
    let payload_bytes = canonical_test_sccp_payload_bytes(&payload);
    let encoded_payload = format!("0x{}", hex::encode(&payload_bytes)).into_bytes();
    let tx = signed_transaction_with_executable(ivm_proved_with_overlay(vec![
        InstructionBox::from(crate::bridge::test_record_sccp_message(encoded_payload)),
        InstructionBox::from(crate::bridge::test_record_sccp_message(payload_bytes)),
    ]));
    let block = signed_block_with_transactions(vec![tx], 9);
    let messages = collect_sccp_messages_from_signed_block(&block);
    let deduped_root =
        sccp_commitment_root_from_messages(&messages).expect("deduped commitment root");
    let err = validate_local_sccp_records_against_commitment_root(&block, deduped_root)
        .expect_err("SCCP finality local record validation must reject encoded payload aliases");
    assert!(err.contains("invalid outbound SCCP record"));
    assert!(err.contains("tx_index=0"));
    assert!(err.contains("instruction_index=0"));
    assert!(err.contains("payload is invalid"));
}
#[test]
fn validate_sccp_commitment_root_for_signed_block_rejects_resultless_sccp_root() {
    let payload = canonical_test_sccp_payload_bytes(&sample_transfer_payload(14, [0x22; 20]));
    let tx =
        signed_transaction_with_executable(ivm_proved_with_overlay(vec![InstructionBox::from(
            crate::bridge::test_record_sccp_message(payload),
        )]));
    let mut block = signed_block_without_results(vec![tx], 9);
    let messages = collect_sccp_messages_from_signed_block(&block);
    let root = sccp_commitment_root_from_messages(&messages).expect("pre-execution SCCP root");
    block.set_sccp_commitment_root(Some(root));
    let err = validate_sccp_commitment_root_for_signed_block(&block)
        .expect_err("committed SCCP root validation must require committed results");
    assert_eq!(
        err,
        SccpCommittedBlockValidationError::MissingTransactionResults { actual: root }
    );
}
#[test]
fn validate_sccp_commitment_root_for_signed_block_rejects_short_result_vector() {
    let plain_tx = signed_transaction_with_executable(Executable::Instructions(
        Vec::<InstructionBox>::new().into(),
    ));
    let payload = canonical_test_sccp_payload_bytes(&sample_transfer_payload(16, [0x22; 20]));
    let sccp_tx =
        signed_transaction_with_executable(ivm_proved_with_overlay(vec![InstructionBox::from(
            crate::bridge::test_record_sccp_message(payload),
        )]));
    let block = signed_block_with_transactions(vec![plain_tx, sccp_tx], 9);
    let block = mutate_bridge_block(&block, |encoded| {
        let result = encoded.result.as_mut().expect("fixture full result");
        result.outputs.pop();
        result.output_merkle = result
            .outputs
            .iter()
            .map(iroha_crypto::HashOf::new)
            .collect();
    });
    let err = validate_sccp_commitment_root_for_signed_block(&block).expect_err(
        "committed SCCP validation must reject external entrypoints without committed results",
    );
    assert!(matches!(
        err,
        SccpCommittedBlockValidationError::InvalidExecutionOutputs(_)
    ));
    assert_eq!(block.network_entrypoint_count(), 2);
    assert_eq!(block.execution_outputs().len(), 1);
}
#[test]
fn validate_sccp_commitment_root_for_signed_block_rejects_invalid_record_payload() {
    let tx =
        signed_transaction_with_executable(ivm_proved_with_overlay(vec![InstructionBox::from(
            crate::bridge::test_record_sccp_message(b"not a canonical SCCP payload".to_vec()),
        )]));
    let block = signed_block_with_transactions(vec![tx], 9);
    let err = validate_sccp_commitment_root_for_signed_block(&block)
        .expect_err("successful invalid SCCP record payload must reject");
    assert_eq!(
        err,
        SccpCommittedBlockValidationError::InvalidRecordInstruction(
            SccpRecordInstructionValidationError::InvalidPayload {
                tx_index: 0,
                instruction_index: 0,
            }
        )
    );
}
#[test]
fn validate_sccp_commitment_root_for_signed_block_rejects_hex_alias_payload() {
    let payload = canonical_test_sccp_payload_bytes(&sample_transfer_payload(15, [0x22; 20]));
    let tx =
        signed_transaction_with_executable(ivm_proved_with_overlay(vec![InstructionBox::from(
            crate::bridge::test_record_sccp_message(
                format!("0x{}", hex::encode(&payload)).into_bytes(),
            ),
        )]));
    let block = signed_block_with_transactions(vec![tx], 9);
    let err = validate_sccp_commitment_root_for_signed_block(&block)
        .expect_err("successful hex-aliased SCCP record payload must reject");
    assert_eq!(
        err,
        SccpCommittedBlockValidationError::InvalidRecordInstruction(
            SccpRecordInstructionValidationError::InvalidPayload {
                tx_index: 0,
                instruction_index: 0,
            }
        )
    );
}
#[test]
fn validate_sccp_commitment_root_for_signed_block_rejects_bare_transfer_payload() {
    let payload = sample_transfer_payload(20, [0x22; 20]);
    let SccpPayloadV1::Transfer(transfer) = payload;
    let tx =
        signed_transaction_with_executable(ivm_proved_with_overlay(vec![InstructionBox::from(
            crate::bridge::test_record_sccp_message(canonical_test_transfer_payload_bytes(
                &transfer,
            )),
        )]));
    let block = signed_block_with_transactions(vec![tx], 9);
    let err = validate_sccp_commitment_root_for_signed_block(&block)
        .expect_err("successful bare transfer SCCP record payload must reject");
    assert_eq!(
        err,
        SccpCommittedBlockValidationError::InvalidRecordInstruction(
            SccpRecordInstructionValidationError::InvalidPayload {
                tx_index: 0,
                instruction_index: 0,
            }
        )
    );
}
#[test]
fn validate_sccp_commitment_root_for_signed_block_rejects_non_sora_record_payload() {
    let payload = canonical_test_sccp_payload_bytes(&non_sora_source_transfer_payload(18));
    let tx =
        signed_transaction_with_executable(ivm_proved_with_overlay(vec![InstructionBox::from(
            crate::bridge::test_record_sccp_message(payload),
        )]));
    let block = signed_block_with_transactions(vec![tx], 9);
    let err = validate_sccp_commitment_root_for_signed_block(&block)
        .expect_err("successful non-SORA SCCP record payload must reject");
    assert_eq!(
        err,
        SccpCommittedBlockValidationError::InvalidRecordInstruction(
            SccpRecordInstructionValidationError::NonSoraSource {
                tx_index: 0,
                instruction_index: 0,
                source_domain: iroha_sccp::SCCP_DOMAIN_ETH,
            }
        )
    );
}
#[test]
fn validate_sccp_commitment_root_for_signed_block_rejects_empty_outbound_route() {
    let mut payload = sample_transfer_payload(17, [0x22; 20]);
    let SccpPayloadV1::Transfer(transfer) = &mut payload;
    transfer.route_id.clear();
    let payload = canonical_test_sccp_payload_bytes(&payload);
    let tx =
        signed_transaction_with_executable(ivm_proved_with_overlay(vec![InstructionBox::from(
            crate::bridge::test_record_sccp_message(payload),
        )]));
    let block = signed_block_with_transactions(vec![tx], 9);
    let err = validate_sccp_commitment_root_for_signed_block(&block)
        .expect_err("successful empty outbound SCCP route must reject");
    assert_eq!(
        err,
        SccpCommittedBlockValidationError::InvalidRecordInstruction(
            SccpRecordInstructionValidationError::RouteBinding {
                tx_index: 0,
                instruction_index: 0,
                error: SccpOutboundRouteValidationError::EmptyRouteId,
            }
        )
    );
}
#[test]
fn recorded_sccp_route_validation_preserves_typed_errors_before_generic_structure() {
    #[derive(Clone, Copy)]
    enum Case {
        NonTextRoute,
        InvalidRouteUtf8,
        EmptyRoute,
        NonTextAsset,
        InvalidAssetUtf8,
        EmptyAsset,
        InvalidAsset,
        EmptyScope,
        AmbiguousScope,
        ScopedAlias,
        ForeignAssetHome,
    }
    let valid = sample_transfer_payload(170, [0x22; 20]);
    let valid_bytes = canonical_test_sccp_payload_bytes(&valid);
    let context = test_sccp_outbound_context_for_payload_bytes(&valid_bytes);
    for case in [
        Case::NonTextRoute,
        Case::InvalidRouteUtf8,
        Case::EmptyRoute,
        Case::NonTextAsset,
        Case::InvalidAssetUtf8,
        Case::EmptyAsset,
        Case::InvalidAsset,
        Case::EmptyScope,
        Case::AmbiguousScope,
        Case::ScopedAlias,
        Case::ForeignAssetHome,
    ] {
        let mut payload = valid.clone();
        let SccpPayloadV1::Transfer(transfer) = &mut payload;
        let expected = match case {
            Case::NonTextRoute => {
                transfer.route_id_codec = iroha_sccp::SCCP_CODEC_EVM_ADDRESS20;
                SccpOutboundRouteValidationError::NonTextRouteId
            }
            Case::InvalidRouteUtf8 => {
                transfer.route_id = vec![0xFF];
                SccpOutboundRouteValidationError::InvalidRouteIdUtf8
            }
            Case::EmptyRoute => {
                transfer.route_id.clear();
                SccpOutboundRouteValidationError::EmptyRouteId
            }
            Case::NonTextAsset => {
                transfer.asset_id_codec = iroha_sccp::SCCP_CODEC_EVM_ADDRESS20;
                SccpOutboundRouteValidationError::NonTextAssetId
            }
            Case::InvalidAssetUtf8 => {
                transfer.asset_id = vec![0xFF];
                SccpOutboundRouteValidationError::InvalidAssetIdUtf8
            }
            Case::EmptyAsset => {
                transfer.asset_id.clear();
                SccpOutboundRouteValidationError::EmptyAssetKey
            }
            Case::InvalidAsset => {
                transfer.asset_id = b"bad name".to_vec();
                SccpOutboundRouteValidationError::InvalidAssetKey
            }
            Case::EmptyScope => {
                transfer.asset_id = b"xor#".to_vec();
                SccpOutboundRouteValidationError::EmptyAssetScope
            }
            Case::AmbiguousScope => {
                transfer.asset_id = b"xor#universal#shadow".to_vec();
                SccpOutboundRouteValidationError::AmbiguousAssetScope
            }
            Case::ScopedAlias => {
                transfer.asset_id = b"xor#universal".to_vec();
                SccpOutboundRouteValidationError::AssetScopeAlias {
                    asset_key: "xor".to_owned(),
                    scope: "universal".to_owned(),
                }
            }
            Case::ForeignAssetHome => {
                transfer.asset_home_domain = iroha_sccp::SCCP_DOMAIN_ETH;
                SccpOutboundRouteValidationError::InvalidAssetHomeDomain {
                    asset_home_domain: iroha_sccp::SCCP_DOMAIN_ETH,
                    dest_domain: transfer.dest_domain,
                }
            }
        };
        let payload_bytes = canonical_test_sccp_payload_bytes(&payload);
        assert_eq!(
            validate_recorded_sccp_message_payload_bytes(context, &payload_bytes),
            Err(RecordedSccpMessageValidationError::RouteBinding { error: expected })
        );
    }
    let mut structurally_invalid = valid;
    let SccpPayloadV1::Transfer(transfer) = &mut structurally_invalid;
    transfer.amount = 0;
    let payload_bytes = canonical_test_sccp_payload_bytes(&structurally_invalid);
    assert_eq!(
        validate_recorded_sccp_message_payload_bytes(context, &payload_bytes),
        Err(RecordedSccpMessageValidationError::InvalidPayload)
    );
}
#[test]
fn validate_sccp_commitment_root_for_signed_block_rejects_ambiguous_asset_scope() {
    let mut payload = sample_transfer_payload(24, [0x22; 20]);
    let SccpPayloadV1::Transfer(transfer) = &mut payload;
    transfer.asset_id = b"xor#universal#shadow".to_vec();
    let payload = canonical_test_sccp_payload_bytes(&payload);
    let tx =
        signed_transaction_with_executable(ivm_proved_with_overlay(vec![InstructionBox::from(
            crate::bridge::test_record_sccp_message(payload),
        )]));
    let block = signed_block_with_transactions(vec![tx], 9);
    let err = validate_sccp_commitment_root_for_signed_block(&block)
        .expect_err("successful ambiguous outbound SCCP asset scope must reject");
    assert_eq!(
        err,
        SccpCommittedBlockValidationError::InvalidRecordInstruction(
            SccpRecordInstructionValidationError::RouteBinding {
                tx_index: 0,
                instruction_index: 0,
                error: SccpOutboundRouteValidationError::AmbiguousAssetScope,
            }
        )
    );
}
#[test]
fn validate_sccp_commitment_root_for_signed_block_rejects_scoped_asset_alias() {
    let mut payload = sample_transfer_payload(25, [0x22; 20]);
    let SccpPayloadV1::Transfer(transfer) = &mut payload;
    transfer.asset_id = b"xor#universal".to_vec();
    let payload = canonical_test_sccp_payload_bytes(&payload);
    let tx =
        signed_transaction_with_executable(ivm_proved_with_overlay(vec![InstructionBox::from(
            crate::bridge::test_record_sccp_message(payload),
        )]));
    let block = signed_block_with_transactions(vec![tx], 9);
    let err = validate_sccp_commitment_root_for_signed_block(&block)
        .expect_err("successful scoped outbound SCCP asset alias must reject");
    assert_eq!(
        err,
        SccpCommittedBlockValidationError::InvalidRecordInstruction(
            SccpRecordInstructionValidationError::RouteBinding {
                tx_index: 0,
                instruction_index: 0,
                error: SccpOutboundRouteValidationError::AssetScopeAlias {
                    asset_key: "xor".to_owned(),
                    scope: "universal".to_owned(),
                },
            }
        )
    );
}
#[test]
fn validate_sccp_commitment_root_for_signed_block_accepts_direct_record_instruction() {
    let payload = canonical_test_sccp_payload_bytes(&sample_transfer_payload(19, [0x22; 20]));
    let tx = signed_transaction_with_executable(Executable::Instructions(
        vec![InstructionBox::from(
            crate::bridge::test_record_sccp_message(payload),
        )]
        .into(),
    ));
    let mut block = signed_block_with_transactions(vec![tx], 9);
    let messages = collect_sccp_messages_from_signed_block(&block);
    assert_eq!(messages.len(), 1);
    let root = sccp_commitment_root_from_messages(&messages).expect("direct record root");
    let outputs = block.execution_outputs().to_vec();
    block.set_sccp_commitment_root(Some(root));
    attach_test_outputs(&mut block, outputs);
    validate_sccp_commitment_root_for_signed_block(&block)
        .expect("successful direct SCCP record instruction must validate");
}
#[test]
fn local_sccp_finality_records_reject_invalid_record_payload() {
    let tx =
        signed_transaction_with_executable(ivm_proved_with_overlay(vec![InstructionBox::from(
            crate::bridge::test_record_sccp_message(b"not a canonical SCCP payload".to_vec()),
        )]));
    let block = signed_block_with_transactions(vec![tx], 9);
    let err = validate_local_sccp_records_against_commitment_root(&block, [0xAA; 32])
        .expect_err("local SCCP finality validation must reject invalid record payloads");
    assert!(err.contains("invalid outbound SCCP record"));
    assert!(err.contains("tx_index=0"));
    assert!(err.contains("instruction_index=0"));
    assert!(err.contains("payload is invalid"));
}
#[test]
fn local_sccp_finality_records_reject_short_result_vector() {
    let plain_tx = signed_transaction_with_executable(Executable::Instructions(
        Vec::<InstructionBox>::new().into(),
    ));
    let payload = canonical_test_sccp_payload_bytes(&sample_transfer_payload(17, [0x22; 20]));
    let sccp_tx =
        signed_transaction_with_executable(ivm_proved_with_overlay(vec![InstructionBox::from(
            crate::bridge::test_record_sccp_message(payload),
        )]));
    let block = signed_block_with_transactions(vec![plain_tx, sccp_tx], 9);
    let block = mutate_bridge_block(&block, |encoded| {
        let result = encoded.result.as_mut().expect("fixture full result");
        result.outputs.pop();
        result.output_merkle = result
            .outputs
            .iter()
            .map(iroha_crypto::HashOf::new)
            .collect();
    });
    let err = validate_local_sccp_records_against_commitment_root(&block, [0xAA; 32])
        .expect_err("local SCCP finality validation must reject short result vectors");
    assert!(err.contains("invalid execution outputs"));
    assert_eq!(block.network_entrypoint_count(), 2);
    assert_eq!(block.execution_outputs().len(), 1);
}
#[test]
fn local_sccp_finality_records_reject_resultless_matching_root() {
    let payload = canonical_test_sccp_payload_bytes(&sample_transfer_payload(15, [0x22; 20]));
    let tx =
        signed_transaction_with_executable(ivm_proved_with_overlay(vec![InstructionBox::from(
            crate::bridge::test_record_sccp_message(payload),
        )]));
    let block = signed_block_without_results(vec![tx], 9);
    let messages = collect_sccp_messages_from_signed_block(&block);
    let root = sccp_commitment_root_from_messages(&messages).expect("pre-execution SCCP root");
    let err = validate_local_sccp_records_against_commitment_root(&block, root)
        .expect_err("local SCCP finality validation must require committed results");
    assert!(err.contains("missing committed transaction results"));
}
#[test]
fn collect_sccp_messages_from_block_skips_failed_transactions() {
    let first_payload = canonical_test_sccp_payload_bytes(&sample_transfer_payload(10, [0x22; 20]));
    let second_payload =
        canonical_test_sccp_payload_bytes(&sample_transfer_payload(11, [0x22; 20]));
    let first_tx =
        signed_transaction_with_executable(ivm_proved_with_overlay(vec![InstructionBox::from(
            crate::bridge::test_record_sccp_message(first_payload.clone()),
        )]));
    let second_tx =
        signed_transaction_with_executable(ivm_proved_with_overlay(vec![InstructionBox::from(
            crate::bridge::test_record_sccp_message(second_payload),
        )]));
    let mut block = signed_block_with_transactions(vec![first_tx, second_tx], 9);
    attach_test_network_results(
        &mut block,
        vec![
            TransactionResultInner::Ok(DataTriggerSequence::default()),
            TransactionResultInner::Err(
                iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
                    iroha_data_model::ValidationFail::NotPermitted(
                        "failed SCCP transaction fixture".to_owned(),
                    ),
                ),
            ),
        ],
    );
    let messages = collect_sccp_messages_from_signed_block(&block);
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].tx_index, 0);
    assert_eq!(
        messages[0].payload,
        iroha_sccp::decode_canonical_sccp_payload_bytes(&first_payload).expect("payload decodes")
    );
}
#[test]
fn collect_sccp_messages_from_block_skips_failed_network_with_internal_outputs() {
    let payload = canonical_test_sccp_payload_bytes(&sample_transfer_payload(12, [0x22; 20]));
    let tx =
        signed_transaction_with_executable(ivm_proved_with_overlay(vec![InstructionBox::from(
            crate::bridge::test_record_sccp_message(payload),
        )]));
    let mut block = signed_block_with_transactions(vec![tx], 10);
    let outputs = vec![
        ExecutionOutputV1::Network(NetworkExecutionOutputV1 {
            input_index: 0,
            result: TransactionResult::new(Err(
                iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
                    iroha_data_model::ValidationFail::NotPermitted(
                        "failed SCCP transaction fixture".to_owned(),
                    ),
                ),
            )),
            completions: Vec::new(),
        }),
        test_internal_output(true, Vec::new()),
        test_internal_output(false, Vec::new()),
    ];
    attach_test_outputs(&mut block, outputs);
    assert_eq!(block.network_entrypoint_count(), 1);
    assert_eq!(block.execution_outputs().len(), 3);
    assert!(collect_sccp_messages_from_signed_block(&block).is_empty());
}
#[test]
fn collect_sccp_messages_from_block_uses_entrypoint_index_after_sealed_commitment() {
    let payload = canonical_test_sccp_payload_bytes(&sample_transfer_payload(13, [0x22; 20]));
    let tx =
        signed_transaction_with_executable(ivm_proved_with_overlay(vec![InstructionBox::from(
            crate::bridge::test_record_sccp_message(payload),
        )]));
    let sealed_entrypoint = sealed_commitment_entrypoint();
    let mut block = signed_block_with_transactions(vec![tx.clone()], 11);
    block.set_external_entrypoints(vec![sealed_entrypoint, TransactionEntrypoint::External(tx)]);
    attach_test_network_results(
        &mut block,
        vec![
            TransactionResultInner::Ok(DataTriggerSequence::default()),
            TransactionResultInner::Err(
                iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
                    iroha_data_model::ValidationFail::NotPermitted(
                        "failed SCCP transaction fixture".to_owned(),
                    ),
                ),
            ),
        ],
    );
    assert!(collect_sccp_messages_from_signed_block(&block).is_empty());
}
#[test]
fn collect_sccp_messages_from_block_includes_successful_sealed_reveal() {
    let payload = canonical_test_sccp_payload_bytes(&sample_transfer_payload(14, [0x22; 20]));
    let [commitment, reveal] = sealed_sccp_record_entrypoints(payload.clone());
    let mut block = signed_block_with_transactions(Vec::new(), 12);
    block.set_external_entrypoints(vec![commitment, reveal]);
    attach_test_network_results(
        &mut block,
        vec![
            TransactionResultInner::Ok(DataTriggerSequence::default()),
            TransactionResultInner::Ok(DataTriggerSequence::default()),
        ],
    );
    let messages = collect_sccp_messages_from_signed_block(&block);
    assert_eq!(messages.len(), 1);
    assert_eq!(
        messages[0].tx_index, 1,
        "SCCP reveal record must keep the reveal entrypoint index"
    );
    assert_eq!(messages[0].instruction_index, 0);
    assert_eq!(
        messages[0].payload,
        iroha_sccp::decode_canonical_sccp_payload_bytes(&payload).expect("payload decodes")
    );
}
#[test]
fn collect_sccp_messages_from_ivm_proved_overlay() {
    let payload = canonical_test_sccp_payload_bytes(&sample_transfer_payload(7, [0x22; 20]));
    let executable = Executable::IvmProved(IvmProved {
        bytecode: IvmBytecode::from_compiled(vec![0x01, 0x02, 0x03]),
        overlay: vec![InstructionBox::from(
            crate::bridge::test_record_sccp_message(payload.clone()),
        )]
        .into(),
        events_commitment: Hash::new(b"events"),
        gas_policy_commitment: Hash::new(b"gas"),
    });
    let tx = signed_transaction_with_executable(executable);
    let block = signed_block_with_transactions(vec![tx], 4);
    let messages = collect_sccp_messages_from_signed_block(&block);
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].tx_index, 0);
    assert_eq!(messages[0].instruction_index, 0);
    assert_eq!(
        messages[0].payload,
        iroha_sccp::decode_canonical_sccp_payload_bytes(&payload).expect("payload decodes")
    );
}
#[test]
fn sccp_network_projection_keeps_full_internal_suffix_without_input_aliases() {
    let payload = canonical_test_sccp_payload_bytes(&sample_transfer_payload(305, [0x45; 20]));
    let (mut block, _) = signed_block_with_sccp_payloads(&[payload], 2);
    let expected = collect_sccp_messages_from_signed_block(&block);
    let source = block.network_entrypoint_at(0).unwrap().hash();
    let proposal = block.canonical_resultless_proposal();
    let mut outputs = block.execution_outputs().to_vec();
    outputs.push(test_internal_output(true, Vec::new()));
    outputs.push(test_internal_output(false, Vec::new()));
    attach_test_outputs(&mut block, outputs);
    assert_eq!(block.canonical_resultless_proposal(), proposal);
    assert_eq!(block.network_entrypoint_count(), 1);
    assert_eq!(block.execution_outputs().len(), 3);
    assert_eq!(block.network_entrypoint_at(0).unwrap().hash(), source);
    assert_eq!(block.network_output_at(0).unwrap().0, 0);
    assert!(block.network_entrypoint_at(1).is_none());
    assert!(block.network_output_at(1).is_none());
    assert_eq!(collect_sccp_messages_from_signed_block(&block), expected);
    assert_eq!(
        collect_sccp_replay_admissions_from_finalized_execution(&block, None)
            .unwrap()
            .len(),
        1
    );
    let wire = block.canonical_wire().unwrap();
    let decoded = iroha_data_model::block::decode_framed_signed_block(wire.as_framed()).unwrap();
    assert_eq!(decoded, block);
}

#[test]
fn sccp_projection_rejects_complete_body_corruption_before_any_message() {
    let payload = canonical_test_sccp_payload_bytes(&sample_transfer_payload(306, [0x46; 20]));
    let (mut original, _) = signed_block_with_sccp_payloads(&[payload], 2);
    let mut outputs = original.execution_outputs().to_vec();
    outputs.push(test_internal_output(true, Vec::new()));
    outputs.push(test_internal_output(false, Vec::new()));
    attach_test_outputs(&mut original, outputs);
    for mutation in 0..5 {
        let block = mutate_bridge_block(&original, |encoded| {
            let result = encoded.result.as_mut().unwrap();
            match mutation {
                0 => {
                    // Correct row count, altered cached leaves.
                    result.output_merkle = result
                        .outputs
                        .iter()
                        .rev()
                        .map(iroha_crypto::HashOf::new)
                        .collect();
                }
                1 => {
                    let ExecutionOutputV1::Network(row) = &mut result.outputs[0] else {
                        unreachable!()
                    };
                    row.input_index = 1;
                    result.output_merkle = result
                        .outputs
                        .iter()
                        .map(iroha_crypto::HashOf::new)
                        .collect();
                }
                2 => {
                    // Internal success cannot stand in for the missing Network row.
                    result.outputs.remove(0);
                    result.output_merkle = result
                        .outputs
                        .iter()
                        .map(iroha_crypto::HashOf::new)
                        .collect();
                }
                3 => {
                    let ExecutionOutputV1::Time(row) = &mut result.outputs[2] else {
                        unreachable!()
                    };
                    row.invocation.trigger.registered_at_height = 2;
                    result.output_merkle = result
                        .outputs
                        .iter()
                        .map(iroha_crypto::HashOf::new)
                        .collect();
                }
                4 => {
                    encoded.payload.header.set_execution_context_hash(Some(
                        iroha_crypto::HashOf::from_untyped_unchecked(Hash::new(
                            b"foreign bridge context",
                        )),
                    ));
                }
                _ => unreachable!(),
            }
        });
        assert!(
            matches!(
                validate_sccp_commitment_root_for_signed_block(&block),
                Err(SccpCommittedBlockValidationError::InvalidExecutionOutputs(
                    _
                ))
            ),
            "mutation {mutation}"
        );
        assert!(
            collect_sccp_messages_from_signed_block(&block).is_empty(),
            "mutation {mutation} must not leak the first valid candidate"
        );
        assert_eq!(
            collect_sccp_replay_admissions_from_finalized_execution(&block, None),
            Err(SccpReplayRebuildErrorV1::MalformedBlock)
        );
    }
}

#[test]
fn sccp_callback_record_requires_applied_outbox_authority() {
    let payload = canonical_test_sccp_payload_bytes(&sample_transfer_payload(307, [0x47; 20]));
    for origin in 0..3 {
        let transactions = if origin == 0 {
            vec![signed_transaction_with_executable(
                Executable::Instructions(Vec::<InstructionBox>::new().into()),
            )]
        } else {
            Vec::new()
        };
        let mut block = signed_block_with_transactions(transactions, 2);
        let internal = test_internal_output(
            origin == 1,
            vec![crate::bridge::test_record_sccp_message(payload.clone()).into()],
        );
        let output = if origin == 0 {
            ExecutionOutputV1::Network(NetworkExecutionOutputV1 {
                input_index: 0,
                result: internal.result().clone(),
                completions: Vec::new(),
            })
        } else {
            internal
        };
        attach_test_outputs(&mut block, vec![output]);
        assert_eq!(block.network_entrypoint_count(), usize::from(origin == 0));
        assert!(collect_sccp_messages_from_signed_block(&block).is_empty());
        assert!(
            matches!(validate_sccp_commitment_root_for_signed_block(&block), Err(SccpCommittedBlockValidationError::InvalidExecutionOutputs(reason)) if reason.contains("applied-outbox"))
        );
        // Full typed internal trace is examined without inventing a signed authority.
        assert_eq!(
            collect_sccp_replay_admissions_from_finalized_execution(&block, None),
            Err(SccpReplayRebuildErrorV1::MalformedAdmission)
        );
    }
}

#[test]
fn sccp_typed_output_substitution_cannot_use_original_executed_finality() {
    use iroha_data_model::block::proofs::{TrustedBlockProofAnchor, TrustedBlockProofAnchorError};
    let fixture = iroha_sccp::sccp_exact_outbound_test_fixture_v1();
    let block = fixture.finalized_block.block();
    let artifact = &fixture.finalized_block.proof().finality_artifact;
    // The exact fixture constructs/authenticates this target; this is not an untrusted
    // transport artifact promoted to an independent production trust pin.
    let context = artifact.context_id();
    let source = block.network_entrypoint_at(0).unwrap().hash();
    TrustedBlockProofAnchor::from_untrusted_finality_artifact(block, artifact, context, &source)
        .expect("real BLS CommitQC binds the complete original output wire");
    let mut changed = block.clone();
    let mut outputs = changed.execution_outputs().to_vec();
    outputs[0] = ExecutionOutputV1::network_output_limit_rejection(0);
    attach_test_outputs(&mut changed, outputs);
    assert_eq!(changed.header(), block.header());
    assert_eq!(changed.hash(), block.hash());
    assert_eq!(
        changed.canonical_resultless_proposal(),
        block.canonical_resultless_proposal()
    );
    assert_ne!(
        changed.canonical_wire().unwrap().as_framed(),
        block.canonical_wire().unwrap().as_framed()
    );
    changed.validate_output_merkle_cache().unwrap();
    assert!(matches!(
        TrustedBlockProofAnchor::from_untrusted_finality_artifact(
            &changed, artifact, context, &source
        ),
        Err(TrustedBlockProofAnchorError::ExecutedBlockWireMismatch)
    ));
    assert!(matches!(
        validate_sccp_commitment_root_for_signed_block(&changed),
        Err(SccpCommittedBlockValidationError::CommitmentRootMismatch { .. })
    ));
}
