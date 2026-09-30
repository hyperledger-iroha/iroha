// Included in the real stream-token broker fixture. These tests qualify transport binding only;
// successful signed transaction decoding does not claim native execution or finalized authority.

fn native_check_fixture() -> (
    iroha_data_model::isi::sorafs::MutateSorafsStreamTokenAuthority,
    KeyPair,
) {
    use iroha_data_model::{
        account::AccountId,
        block::consensus::HeightContextId,
        sorafs::{capacity::ProviderId, stream_token_authority::*},
    };
    let observer = KeyPair::try_from_seed(vec![0xD3; 32], Algorithm::Ed25519).unwrap();
    let operator = KeyPair::try_from_seed(vec![0xD4; 32], Algorithm::Ed25519).unwrap();
    let receipt =
        stream_token_signer_test_support::receipt(&token_body().signing_payload_bytes().unwrap());
    let reviewed = StreamTokenReviewedV1 {
        request: receipt.request,
        intent: receipt.intent,
    };
    let check = StreamTokenCheckV1 {
        challenge: [0xD5; 32],
        expected_operator: AccountId::new(operator.public_key().clone()),
        expected_observer: AccountId::new(observer.public_key().clone()),
        floor: StreamTokenFinalityFloorV1 {
            height: 10,
            block_hash: [0xD6; 32],
            context_id: HeightContextId(iroha_crypto::HashOf::from_untyped_unchecked(
                iroha_crypto::Hash::prehashed([0xD7; 32]),
            )),
        },
        reviewed,
        phase: StreamTokenCheckPhaseV1::Current(reviewed.intent.previous_audit),
    };
    (
        iroha_data_model::isi::sorafs::MutateSorafsStreamTokenAuthority {
            request: StreamTokenAuthorityRequestV1 {
                network_id: *network_id().as_bytes(),
                provider_id: ProviderId::new([0x22; 32]),
                expected_control_revision: 1,
                expected_control_digest: reviewed.request.original_custody.control_state_digest,
                action: StreamTokenAuthorityActionV1::Check(check),
            },
        },
        observer,
    )
}

#[test]
fn native_check_transport_rejects_mutations_foreign_scope_and_substituted_signed_result() {
    use iroha_data_model::{
        account::AccountId,
        isi::InstructionBox,
        sorafs::stream_token_authority::StreamTokenAuthorityActionV1,
        transaction::{FeePaymentIntent, TransactionBuilder},
    };
    let binding = token_signer_binding();
    let (instruction, observer) = native_check_fixture();
    let payload = encode_canonical(&instruction, 16 * 1024).unwrap();
    assert_eq!(
        decode_stream_token_check_request(&binding, &payload).unwrap(),
        instruction
    );
    let sign = |instruction: iroha_data_model::isi::sorafs::MutateSorafsStreamTokenAuthority| {
        TransactionBuilder::new(
            network_id(),
            AccountId::new(observer.public_key().clone()),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([InstructionBox::from(instruction)])
        .try_sign(observer.private_key())
        .unwrap()
    };
    let signed = sign(instruction.clone());
    let result = encode_canonical(&signed, 32 * 1024).unwrap();
    assert_eq!(
        decode_stream_token_check_result(&binding, &payload, &result).unwrap(),
        signed
    );
    let mut substituted = instruction.clone();
    let StreamTokenAuthorityActionV1::Check(check) = &mut substituted.request.action else {
        unreachable!()
    };
    check.challenge[0] ^= 1;
    let substituted = encode_canonical(&sign(substituted), 32 * 1024).unwrap();
    assert!(decode_stream_token_check_result(&binding, &payload, &substituted).is_err());
    let mut foreign = instruction.clone();
    foreign.request.network_id[0] ^= 1;
    assert!(
        decode_stream_token_check_request(
            &binding,
            &encode_canonical(&foreign, 16 * 1024).unwrap()
        )
        .is_err()
    );
    let mut reserve = instruction;
    let StreamTokenAuthorityActionV1::Check(check) = &reserve.request.action else {
        unreachable!()
    };
    reserve.request.action = StreamTokenAuthorityActionV1::Reserve(check.reviewed);
    assert!(
        decode_stream_token_check_request(
            &binding,
            &encode_canonical(&reserve, 16 * 1024).unwrap()
        )
        .is_err()
    );
}
