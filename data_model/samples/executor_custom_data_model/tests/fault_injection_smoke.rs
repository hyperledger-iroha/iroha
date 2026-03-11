use std::str::FromStr;

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use iroha_data_model::{Level, prelude::*};

const OVERLAY_KEY: &str = "fault_injection_overlay";

fn overlay_entries(tx: &SignedTransaction) -> Vec<String> {
    let key = Name::from_str(OVERLAY_KEY).expect("valid metadata key");
    tx.metadata()
        .get(&key)
        .cloned()
        .and_then(|value| value.try_into_any_norito::<Vec<String>>().ok())
        .unwrap_or_default()
}

#[test]
fn kotodama_bytecode_fault_injection_smoke() {
    let chain: ChainId = "fi-smoke-chain".parse().unwrap();
    let keypair = iroha_crypto::KeyPair::random();
    let account_id = AccountId::new(keypair.public_key().clone());

    let mut tx = TransactionBuilder::new(chain, account_id)
        .with_bytecode(IvmBytecode::from_compiled(vec![0xAA, 0xBB, 0xCC]))
        .sign(keypair.private_key());

    let original = match tx.instructions() {
        Executable::Ivm(bytecode) => bytecode.as_ref().to_vec(),
        _ => panic!("expected IVM bytecode payload"),
    };

    let first: InstructionBox = Log::new(Level::INFO, "first fault probe".into()).into();
    let second: InstructionBox = Log::new(Level::WARN, "second fault probe".into()).into();

    tx.inject_instructions([first.clone()]);
    tx.inject_instructions([second.clone()]);

    let mutated = match tx.instructions() {
        Executable::Ivm(bytecode) => bytecode.as_ref().to_vec(),
        _ => panic!("expected IVM bytecode payload"),
    };
    assert_eq!(
        mutated, original,
        "fault injection must not mutate Kotodama bytecode"
    );

    let overlay = overlay_entries(&tx);
    assert_eq!(
        overlay,
        vec![
            BASE64_STANDARD
                .encode(norito::to_bytes(&first).expect("encode first overlay instruction")),
            BASE64_STANDARD
                .encode(norito::to_bytes(&second).expect("encode second overlay instruction"))
        ],
        "overlay must retain encoded instruction payloads"
    );
}
