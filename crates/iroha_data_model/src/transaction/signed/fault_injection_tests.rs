//! Feature-gated signed-transaction fault-injection tests.

use super::*;
use crate::{Level, isi::Log};

fn checked_random_keypair() -> iroha_crypto::KeyPair {
    iroha_crypto::KeyPair::try_random().expect("test fixture random key generation should succeed")
}

fn sample_account() -> (NetworkId, AccountId, iroha_crypto::KeyPair) {
    let network_id = test_network_id(0x2D);
    let keypair = checked_random_keypair();
    let account_id = AccountId::new(keypair.public_key().clone());
    (network_id, account_id, keypair)
}

fn overlay_entries(tx: &SignedTransaction) -> Vec<String> {
    SignedTransaction::fault_injection_overlay(&tx.payload.metadata).unwrap_or_default()
}

#[test]
fn injects_into_ivm_bytecode_and_records_trailer() {
    let (network_id, account_id, keypair) = sample_account();
    let mut tx = TransactionBuilder::new(
        network_id,
        account_id.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_bytecode(IvmBytecode::from_compiled(vec![0xAA, 0xBB, 0xCC]))
    .sign(keypair.private_key());

    let original_hash = tx.hash();
    let original_bytes = match tx.instructions() {
        Executable::Ivm(bytecode) => bytecode.as_ref().to_vec(),
        _ => panic!("expected bytecode payload"),
    };

    let injected: InstructionBox = Log {
        level: Level::INFO,
        msg: "fault injected".into(),
    }
    .into();
    let expected = injected.clone();

    tx.inject_instructions([injected]);

    assert_ne!(tx.hash(), original_hash, "hash must change after injection");

    let patched_bytes = match tx.instructions() {
        Executable::Ivm(bytecode) => bytecode.as_ref().to_vec(),
        _ => panic!("expected bytecode payload"),
    };
    assert_eq!(
        patched_bytes, original_bytes,
        "fault injection must not mutate the Kotodama bytecode payload"
    );

    let overlay = overlay_entries(&tx);
    assert_eq!(overlay.len(), 1);
    let expected_b64 =
        BASE64_STANDARD.encode(norito::to_bytes(&expected).expect("encode overlay payload"));
    assert_eq!(overlay[0], expected_b64);
}

#[test]
fn repeated_injection_appends_trailer_instructions() {
    let (network_id, account_id, keypair) = sample_account();
    let mut tx = TransactionBuilder::new(
        network_id,
        account_id,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_bytecode(IvmBytecode::from_compiled(vec![0x01, 0x02, 0x03, 0x04]))
    .sign(keypair.private_key());

    let first: InstructionBox = Log {
        level: Level::WARN,
        msg: "first fault".into(),
    }
    .into();
    let second: InstructionBox = Log {
        level: Level::ERROR,
        msg: "second fault".into(),
    }
    .into();

    tx.inject_instructions([first.clone()]);
    tx.inject_instructions([second.clone()]);

    let bytes = match tx.instructions() {
        Executable::Ivm(bytecode) => bytecode.as_ref().to_vec(),
        _ => panic!("expected bytecode payload"),
    };
    assert_eq!(
        bytes,
        vec![0x01, 0x02, 0x03, 0x04],
        "fault injection must leave bytecode untouched"
    );
    let overlay = overlay_entries(&tx);
    assert_eq!(overlay.len(), 2, "overlay should preserve both batches");
    assert_eq!(
        overlay[0],
        BASE64_STANDARD.encode(norito::to_bytes(&first).expect("encode first overlay instruction"))
    );
    assert_eq!(
        overlay[1],
        BASE64_STANDARD
            .encode(norito::to_bytes(&second).expect("encode second overlay instruction"))
    );
}
