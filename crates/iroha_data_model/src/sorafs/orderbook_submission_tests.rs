use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, PrivateKey};
use norito::core::{DecodeFlagsGuard, header_flags};
#[rustfmt::skip]
use sorafs_manifest::{OrderCancelReasonV1, OrderSideV1, OrderTierV1, OrderbookOrderCancelFieldsV1, OrderbookOrderRequestFieldsV1, OrderbookSettlementReceiptFieldsV1, XorQuantity, build_signed_orderbook_order_cancel_bytes_ed25519_v1, build_signed_orderbook_order_request_bytes_ed25519_v1, build_signed_orderbook_settlement_receipt_bytes_ed25519_v1, decode_order_request_v1};
use super::*;
#[rustfmt::skip]
use crate::{account::{AccountAddress, AccountId, address::ChainDiscriminantGuard}, block::BlockHeader, isi::InstructionBox, transaction::{FeePaymentIntent, IvmBytecode, TransactionBuilder, TransactionSubmissionReceiptPayload}};
type Error = SorafsOrderbookSubmissionValidationError;
type Route = SorafsOrderbookSubmissionRouteV1;
const NETWORK_SEED: u8 = 0x71;
const DISCRIMINANT: u16 = 369;
#[rustfmt::skip]
fn keypair(seed: u8) -> KeyPair { KeyPair::from_private_key(PrivateKey::from_bytes(Algorithm::Ed25519, &[seed; 32]).unwrap()).unwrap() }
#[rustfmt::skip]
fn network(seed: u8) -> NetworkId { NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([seed; Hash::LENGTH]))) }
#[rustfmt::skip]
fn amount(micro: u128) -> XorQuantity { XorQuantity::try_from_micro(micro).unwrap() }
#[rustfmt::skip]
fn owner(seed: u8) -> Vec<u8> { AccountAddress::from_account_id(&AccountId::new(keypair(seed).public_key().clone())).unwrap().to_i105_for_discriminant(DISCRIMINANT).unwrap().into_bytes() }
#[rustfmt::skip]
fn order_instruction(owner_account: Vec<u8>, seed: u8) -> InstructionBox {
    SubmitSorafsOrderbookOrder::new(build_signed_orderbook_order_request_bytes_ed25519_v1(OrderbookOrderRequestFieldsV1 {
        side: OrderSideV1::Bid, tier: OrderTierV1::Hot, price_per_gib: amount(10), quantity_gib: 2,
        remaining_gib: 2, owner_account, provider_id: None, expiry_unix: 10, nonce: 1,
        maker_fee_bps: 1, taker_fee_bps: 2,
    }, &[seed; 32]).unwrap(), [0xA5; 32]).into()
}
#[rustfmt::skip]
fn instruction(route: Route, seed: u8) -> InstructionBox {
    match route {
        Route::SubmitOrder => order_instruction(owner(seed), seed),
        Route::CancelOrder => CancelSorafsOrderbookOrder::new(build_signed_orderbook_order_cancel_bytes_ed25519_v1(OrderbookOrderCancelFieldsV1 {
            order_id: [0x41; 32], owner_account: owner(seed), reason: OrderCancelReasonV1::OwnerRequested, nonce: 2,
        }, &[seed; 32]).unwrap(), [0xA5; 32]).into(),
        Route::RecordReceipt => RecordSorafsOrderbookSettlementReceipt::new(build_signed_orderbook_settlement_receipt_bytes_ed25519_v1(OrderbookSettlementReceiptFieldsV1 {
            receipt_id: [0x51; 32], channel_id: [0x52; 32], trade_id: [0x53; 32], range_start: 0,
            range_end: 1, chunk_hash: [0x54; 32], bytes_delivered: 1, xor_debited: amount(2),
            provider_credit: amount(1), fee_amount: amount(1), issued_at_unix: 9,
        }, &[seed; 32]).unwrap(), [0xA5; 32]).into(),
    }
}
#[rustfmt::skip]
fn signed(instructions: Vec<InstructionBox>, seed: u8) -> SignedTransaction {
    let keys = keypair(seed);
    TransactionBuilder::new(network(NETWORK_SEED), AccountId::new(keys.public_key().clone()), FeePaymentIntent::authority(Vec::new(), None)).with_instructions(instructions).sign(keys.private_key())
}
#[rustfmt::skip]
fn transaction(route: Route, seed: u8) -> SignedTransaction { signed(vec![instruction(route, seed)], seed) }
#[rustfmt::skip]
fn ivm(size: usize, seed: u8) -> SignedTransaction {
    let keys = keypair(seed);
    TransactionBuilder::new(network(NETWORK_SEED), AccountId::new(keys.public_key().clone()), FeePaymentIntent::authority(Vec::new(), None)).with_bytecode(IvmBytecode::from_compiled(vec![0xA5; size])).sign(keys.private_key())
}
#[rustfmt::skip]
fn inspect(transaction: &SignedTransaction, route: Route) -> Result<ValidatedSorafsOrderbookSubmissionV1, Error> { inspect_sorafs_orderbook_submission_for_discriminant_v1(&transaction.encode_wire_v1().unwrap(), route, &network(NETWORK_SEED), DISCRIMINANT) }
macro_rules! reject {
    ($transaction:expr, $route:expr, $error:expr) => {
        assert_eq!(inspect(&$transaction, $route), Err($error))
    };
}
macro_rules! reject_wire {
    ($bytes:expr, $route:expr, $network:expr, $error:expr) => {
        assert_eq!(
            inspect_sorafs_orderbook_submission_for_discriminant_v1(
                $bytes,
                $route,
                &network($network),
                DISCRIMINANT
            ),
            Err($error)
        )
    };
}
#[test]
#[allow(deprecated)]
#[rustfmt::skip]
fn all_routes_validate_and_derive_equal_authoritative_identities() {
    assert_eq!(parse_sorafs_orderbook_decimal_u64_v1("0", "value"), Ok(0)); for value in [" 1", "01", "+1"] { assert!(parse_sorafs_orderbook_decimal_u64_v1(value, "value").is_err()); }
    let _chain = ChainDiscriminantGuard::enter(DISCRIMINANT);
    for route in [Route::SubmitOrder, Route::CancelOrder, Route::RecordReceipt] {
        let transaction = transaction(route, 0x21); let validated = inspect(&transaction, route).unwrap(); assert_eq!(inspect_sorafs_orderbook_submission_v1(&transaction.encode_wire_v1().unwrap(), route, &network(NETWORK_SEED)).unwrap(), validated.identity);
        assert_eq!(validated.identity.tx_hash.as_ref(), validated.identity.entrypoint_hash.as_ref());
        assert_eq!(validated.identity.tx_hash, validated.identity.signed_transaction_hash);
    }
}
#[test]
#[rustfmt::skip]
fn transaction_wire_network_route_signature_and_shape_fail_closed() {
    let transaction = transaction(Route::SubmitOrder, 0x22); let canonical = transaction.encode_wire_v1().unwrap();
    reject_wire!(&[], Route::SubmitOrder, NETWORK_SEED, Error::EmptyTransaction);
    reject_wire!(&vec![0; ORDERBOOK_TRANSACTION_MAX_CANONICAL_BYTES_V1 + 1], Route::SubmitOrder, NETWORK_SEED, Error::TransactionTooLarge);
    let mut trailing = canonical.clone(); trailing.push(0);
    reject_wire!(&trailing, Route::SubmitOrder, NETWORK_SEED, Error::InvalidTransactionEncoding);
    reject!(transaction.clone(), Route::CancelOrder, Error::RouteMismatch);
    reject_wire!(&canonical, Route::SubmitOrder, 0x72, Error::NetworkMismatch);
    reject!(transaction.with_authority(AccountId::new(keypair(0x23).public_key().clone())), Route::SubmitOrder, Error::InvalidTransactionSignature);
    reject!(signed(Vec::new(), 0x24), Route::SubmitOrder, Error::NonSingletonInstruction);
    let item = instruction(Route::SubmitOrder, 0x24);
    reject!(signed(vec![item.clone(), item], 0x24), Route::SubmitOrder, Error::NonSingletonInstruction);
    reject!(ivm(1, 0x24), Route::SubmitOrder, Error::NonInstructionExecutable);
}
#[test]
#[rustfmt::skip]
fn embedded_signature_owner_and_discriminant_fail_closed() {
    let item = instruction(Route::SubmitOrder, 0x24);
    let mut order = decode_order_request_v1(&item.as_any().downcast_ref::<SubmitSorafsOrderbookOrder>().unwrap().order_payload).unwrap();
    order.signature.signature[0] ^= 1;
    let bad = SubmitSorafsOrderbookOrder::new(norito::to_bytes(&order).unwrap(), [0xA5; 32]);
    reject!(signed(vec![bad.into()], 0x24), Route::SubmitOrder, Error::InvalidEmbeddedPayload);
    reject!(signed(vec![instruction(Route::SubmitOrder, 0x25)], 0x24), Route::SubmitOrder, Error::EmbeddedPayloadAuthorityMismatch);

    let seed = 0x28; let foreign = signed(vec![order_instruction(owner(seed), seed)], seed);
    let _conflicting = ChainDiscriminantGuard::enter(753);
    inspect(&foreign, Route::SubmitOrder).unwrap();
    assert_eq!(inspect_sorafs_orderbook_submission_for_discriminant_v1(&foreign.encode_wire_v1().unwrap(), Route::SubmitOrder, &network(NETWORK_SEED), 753), Err(Error::InvalidEmbeddedPayload));
    let mut padded = owner(seed); padded.push(b' '); let padded = signed(vec![order_instruction(padded, seed)], seed);
    assert_eq!(inspect_sorafs_orderbook_submission_for_discriminant_v1(&padded.encode_wire_v1().unwrap(), Route::SubmitOrder, &network(NETWORK_SEED), DISCRIMINANT), Err(Error::InvalidEmbeddedPayload));

    let canonical = AccountAddress::from_account_id(&AccountId::new(keypair(seed).public_key().clone())).unwrap();
    let mut alternate = hex::decode(canonical.canonical_hex().unwrap().trim_start_matches("0x")).unwrap(); alternate[0] ^= 0b0010_0000;
    let alternate = AccountAddress::from_canonical_bytes(&alternate).unwrap().to_i105_for_discriminant(DISCRIMINANT).unwrap().into_bytes();
    let alternate = signed(vec![order_instruction(alternate, seed)], seed);
    assert_eq!(inspect_sorafs_orderbook_submission_for_discriminant_v1(&alternate.encode_wire_v1().unwrap(), Route::SubmitOrder, &network(NETWORK_SEED), DISCRIMINANT), Err(Error::InvalidEmbeddedPayload));
}
#[test]
#[rustfmt::skip]
fn alternate_layout_and_framed_overhead_are_rejected() {
    let transaction = transaction(Route::SubmitOrder, 0x26);
    let flags = norito::core::default_encode_flags() ^ header_flags::COMPACT_LEN;
    let alternate = DecodeFlagsGuard::enter(flags); let mut wire = vec![1];
    norito::core::serialize_to_buffer(&transaction, &mut wire).unwrap();
    assert_ne!(wire, transaction.encode_wire_v1().unwrap());
    validate_sorafs_orderbook_submission_transaction_v1(&transaction, Route::SubmitOrder, &network(NETWORK_SEED), DISCRIMINANT).unwrap();
    reject_wire!(&wire, Route::SubmitOrder, NETWORK_SEED, Error::InvalidTransactionEncoding); drop(alternate);

    let (mut low, mut high) = (0, ORDERBOOK_TRANSACTION_MAX_CANONICAL_BYTES_V1);
    while low < high { let mid = low + (high - low).div_ceil(2); if ivm(mid, 0x26).encode_wire_v1().unwrap().len() <= ORDERBOOK_TRANSACTION_MAX_CANONICAL_BYTES_V1 { low = mid } else { high = mid - 1 } }
    let near_cap = ivm(low, 0x26);
    assert!(near_cap.encode_wire_v1().unwrap().len() <= ORDERBOOK_TRANSACTION_MAX_CANONICAL_BYTES_V1);
    assert!(norito::to_bytes(&near_cap).unwrap().len() > ORDERBOOK_TRANSACTION_MAX_CANONICAL_BYTES_V1);
    reject!(near_cap, Route::SubmitOrder, Error::TransactionTooLarge);
}
#[rustfmt::skip]
fn receipt_fixture() -> (SorafsOrderbookSubmissionIdentityV1, KeyPair, TransactionSubmissionReceipt) {
    let identity = inspect(&transaction(Route::SubmitOrder, 0x27), Route::SubmitOrder).unwrap().identity; let signer = keypair(0x61);
    let receipt = TransactionSubmissionReceipt::try_sign(TransactionSubmissionReceiptPayload {
        tx_hash: identity.tx_hash, entrypoint_hash: identity.entrypoint_hash,
        signed_transaction_hash: Some(identity.signed_transaction_hash), submitted_at_ms: 7,
        submitted_at_height: 8, signer: signer.public_key().clone(),
    }, &signer).unwrap();
    (identity, signer, receipt)
}
macro_rules! reject_receipt {
    ($wire:expr, $identity:expr, $signer:expr, $error:expr) => {
        assert_eq!(
            decode_and_verify_sorafs_orderbook_submission_receipt_v1($wire, $identity, $signer),
            Err($error)
        )
    };
}
#[test]
#[rustfmt::skip]
fn receipt_is_exact_signed_pinned_bounded_and_binds_every_identity() {
    let (identity, signer, receipt) = receipt_fixture(); let wire = norito::to_bytes(&receipt).unwrap();
    assert_eq!(decode_and_verify_sorafs_orderbook_submission_receipt_v1(&wire, &identity, signer.public_key()).unwrap(), receipt);
    assert_eq!(parse_sorafs_orderbook_submission_identity_v1(&identity.tx_hash.to_string(), &identity.entrypoint_hash.to_string(), &identity.signed_transaction_hash.to_string()), Some(identity));
    assert_eq!(parse_sorafs_orderbook_receipt_signer_v1(&signer.public_key().to_string()), Some(signer.public_key().clone()));
    reject_receipt!(&[], &identity, signer.public_key(), Error::EmptyReceipt);
    reject_receipt!(&vec![0; ORDERBOOK_SUBMISSION_RECEIPT_MAX_CANONICAL_BYTES_V1 + 1], &identity, signer.public_key(), Error::ReceiptTooLarge);
    reject_receipt!(&wire, &identity, keypair(0x62).public_key(), Error::ReceiptSignerMismatch);
    for index in 0_u8..3 {
        let mut altered = identity; let other = Hash::prehashed([0x80 + index; Hash::LENGTH]);
        let error = match index { 0 => { altered.tx_hash = HashOf::from_untyped_unchecked(other); Error::ReceiptTransactionHashMismatch }, 1 => { altered.entrypoint_hash = HashOf::from_untyped_unchecked(other); Error::ReceiptEntrypointHashMismatch }, _ => { altered.signed_transaction_hash = HashOf::from_untyped_unchecked(other); Error::ReceiptSignedTransactionHashMismatch } };
        reject_receipt!(&wire, &altered, signer.public_key(), error);
    }
    let mut tampered = receipt.clone(); tampered.payload.submitted_at_ms += 1;
    reject_receipt!(&norito::to_bytes(&tampered).unwrap(), &identity, signer.public_key(), Error::InvalidReceiptSignature);
    let flags = norito::core::default_encode_flags() ^ header_flags::COMPACT_LEN; let _alternate = DecodeFlagsGuard::enter(flags); let mut bare = Vec::new();
    norito::core::serialize_to_buffer(&receipt, &mut bare).unwrap();
    let alternate = norito::core::frame_bare_with_header_flags::<TransactionSubmissionReceipt>(&bare, flags).unwrap();
    reject_receipt!(&alternate, &identity, signer.public_key(), Error::NonCanonicalReceipt);
}
