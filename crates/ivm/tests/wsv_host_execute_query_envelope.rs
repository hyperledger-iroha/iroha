//! JSON envelope queries via SMARTCONTRACT_EXECUTE_QUERY (0xA1).
//! Ensures the mock WSV host parses query envelopes and returns results as INPUT TLVs.

use iroha_crypto::PublicKey;
use iroha_primitives::numeric::Numeric;
use ivm::{
    IVM, Memory, PointerType,
    instruction::wide,
    mock_wsv::{
        AssetDefinitionId, DomainId, MockWorldStateView, PermissionToken, ScopedAccountId, WsvHost,
    },
    syscalls,
};
mod common;

fn make_tlv(type_id: u16, payload: &[u8]) -> Vec<u8> {
    let payload = PointerType::from_u16(type_id)
        .map(|pty| common::payload_for_type(pty, payload))
        .unwrap_or_else(|| payload.to_vec());
    let mut out = Vec::with_capacity(7 + payload.len() + 32);
    out.extend_from_slice(&type_id.to_be_bytes());
    out.push(1);
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(payload.as_ref());
    let h: [u8; 32] = iroha_crypto::Hash::new(payload).into();
    out.extend_from_slice(&h);
    out
}

fn account(domain: &str, public_key: &str) -> ScopedAccountId {
    let domain: DomainId = domain.parse().unwrap();
    let public_key: PublicKey = public_key.parse().unwrap();
    ScopedAccountId::new(domain, public_key)
}

fn run_query(vm: &mut IVM, env: norito::json::Value) -> u64 {
    let body = norito::json::to_vec(&env).expect("env json");
    let tlv = make_tlv(PointerType::Json as u16, &body);
    vm.memory.preload_input(0, &tlv).expect("preload input");
    vm.set_register(10, Memory::INPUT_START);
    // Program: SCALL SMARTCONTRACT_EXECUTE_QUERY; HALT
    let mut code = Vec::new();
    code.extend_from_slice(
        &ivm::encoding::wide::encode_sys(
            wide::system::SCALL,
            syscalls::SYSCALL_SMARTCONTRACT_EXECUTE_QUERY as u8,
        )
        .to_le_bytes(),
    );
    code.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    let mut prog = ivm::ProgramMetadata::default().encode();
    prog.extend_from_slice(&code);
    vm.load_program(&prog).unwrap();
    vm.run().expect("exec query envelope");
    vm.register(10)
}

fn run_instruction(vm: &mut IVM, env: norito::json::Value) {
    let body = norito::json::to_vec(&env).expect("env json");
    let tlv = make_tlv(PointerType::Json as u16, &body);
    vm.memory.preload_input(0, &tlv).expect("preload input");
    vm.set_register(10, Memory::INPUT_START);
    // Program: SCALL SMARTCONTRACT_EXECUTE_INSTRUCTION; HALT
    let mut code = Vec::new();
    code.extend_from_slice(
        &ivm::encoding::wide::encode_sys(
            wide::system::SCALL,
            syscalls::SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION as u8,
        )
        .to_le_bytes(),
    );
    code.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    let mut prog = ivm::ProgramMetadata::default().encode();
    prog.extend_from_slice(&code);
    vm.load_program(&prog).unwrap();
    vm.run().expect("exec instruction envelope");
}

#[test]
fn query_get_balance_returns_json_tlv() {
    // Setup caller and WSV with a balance for the same account form used by envelope parsing.
    let caller = account(
        "wonderland",
        "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774",
    );
    let caller_value = norito::json::to_value(&caller).expect("serialize caller");
    let caller_literal = caller_value.as_str().expect("caller id string");
    let alice = ScopedAccountId::parse_encoded(caller_literal)
        .map(iroha_data_model::account::ParsedAccountId::into_account_id)
        .expect("canonical account id must parse");
    let rose: AssetDefinitionId = "rose#wonderland".parse().unwrap();
    let wsv = MockWorldStateView::with_balances(&[(
        (alice.clone(), rose.clone()),
        Numeric::from(42_u64),
    )]);
    let host = WsvHost::new_with_subject(
        wsv,
        ivm::mock_wsv::AccountSubjectId::from(&alice.clone()),
        Default::default(),
    );

    let mut vm = IVM::new(u64::MAX);
    vm.set_host(host);

    // Query alice's balance for rose
    let payload = norito::json::object([
        (
            "account_id",
            norito::json::to_value(&alice).expect("serialize account id"),
        ),
        (
            "asset_id",
            norito::json::to_value(&rose).expect("serialize asset id"),
        ),
    ])
    .expect("serialize balance payload");
    let env = norito::json::object([
        (
            "type",
            norito::json::to_value("wsv.get_balance").expect("serialize type"),
        ),
        ("payload", payload),
    ])
    .expect("serialize balance query envelope");
    let p = run_query(&mut vm, env);

    // Validate returned TLV and JSON content
    let tlv = vm.memory.validate_tlv(p).expect("valid output TLV");
    assert_eq!(tlv.type_id as u16, PointerType::Json as u16);
    let v: norito::json::Value = common::json_from_payload(tlv.payload);
    let bal = v.get("balance").and_then(|v| v.as_str()).unwrap();
    let bal: Numeric = bal.parse().expect("parse numeric balance");
    assert_eq!(bal, Numeric::from(42_u64));
}

#[test]
fn query_list_triggers_returns_all() {
    // Seed WSV with some triggers
    let alice = account(
        "wonderland",
        "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774",
    );
    let mut wsv = MockWorldStateView::new();
    wsv.add_account_unchecked(alice.clone());
    wsv.grant_permission(&alice, PermissionToken::ManageTriggers);
    let host = WsvHost::new_with_subject(
        wsv,
        ivm::mock_wsv::AccountSubjectId::from(&alice.clone()),
        Default::default(),
    );

    let mut vm = IVM::new(u64::MAX);
    vm.set_host(host);

    // Create two triggers via instruction envelopes
    let make_trigger_env = |name: &str| {
        let payload = norito::json::object([(
            "name",
            norito::json::to_value(name).expect("serialize trigger name"),
        )])
        .expect("serialize trigger payload");
        norito::json::object([
            (
                "type",
                norito::json::to_value("wsv.create_trigger").expect("serialize type"),
            ),
            ("payload", payload),
        ])
        .expect("serialize trigger envelope")
    };
    run_instruction(&mut vm, make_trigger_env("t1"));
    run_instruction(&mut vm, make_trigger_env("t2"));
    // Disable t2 so the listing reports mixed enabled states.
    let disable_env = norito::json::object([
        (
            "type",
            norito::json::to_value("wsv.set_trigger_enabled").expect("serialize type"),
        ),
        (
            "payload",
            norito::json::object([
                (
                    "name",
                    norito::json::to_value("t2").expect("serialize trigger name"),
                ),
                (
                    "enabled",
                    norito::json::to_value(&false).expect("serialize enabled flag"),
                ),
            ])
            .expect("serialize set trigger payload"),
        ),
    ])
    .expect("serialize disable trigger envelope");
    run_instruction(&mut vm, disable_env);

    let env = norito::json::object([
        (
            "type",
            norito::json::to_value("wsv.list_triggers").expect("serialize type"),
        ),
        (
            "payload",
            norito::json::Value::Object(norito::json::Map::new()),
        ),
    ])
    .expect("serialize list triggers envelope");
    let p = run_query(&mut vm, env);
    let tlv = vm.memory.validate_tlv(p).expect("valid output TLV");
    assert_eq!(tlv.type_id as u16, PointerType::Json as u16);
    let v: norito::json::Value = common::json_from_payload(tlv.payload);
    let arr = v
        .get("triggers")
        .and_then(|v| v.as_array())
        .expect("triggers array");
    // Convert to a map for easier checking
    let mut map = std::collections::BTreeMap::new();
    for it in arr {
        let name = it.get("name").and_then(|v| v.as_str()).unwrap();
        let en = it.get("enabled").and_then(|v| v.as_bool()).unwrap();
        map.insert(name.to_string(), en);
    }
    assert_eq!(map.get("t1"), Some(&true));
    assert_eq!(map.get("t2"), Some(&false));
}

#[test]
fn query_list_domains_for_subject_returns_sorted_domains() {
    let beta: DomainId = "beta".parse().unwrap();
    let alpha: DomainId = "alpha".parse().unwrap();
    let admin = account(
        "beta",
        "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774",
    );
    let linked_beta = account(
        "beta",
        "ed01201509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4",
    );
    let linked_alpha = account(
        "alpha",
        "ed01201509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4",
    );

    let mut wsv = MockWorldStateView::new();
    wsv.add_account_unchecked(admin.clone());
    wsv.grant_permission(&admin, PermissionToken::RegisterDomain);
    wsv.grant_permission(&admin, PermissionToken::RegisterAccount);
    assert!(wsv.register_account(&admin, linked_beta.clone()));
    assert!(wsv.register_domain(&admin, alpha.clone()));
    assert!(wsv.register_account(&admin, linked_alpha.clone()));

    let host = WsvHost::new_with_subject(
        wsv,
        ivm::mock_wsv::AccountSubjectId::from(&admin),
        Default::default(),
    );
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(host);

    let payload = norito::json::object([(
        "account_id",
        norito::json::to_value(&linked_beta).expect("serialize account id"),
    )])
    .expect("serialize payload");
    let env = norito::json::object([
        (
            "type",
            norito::json::to_value("wsv.list_domains_for_subject").expect("serialize type"),
        ),
        ("payload", payload),
    ])
    .expect("serialize envelope");

    let p = run_query(&mut vm, env);
    let tlv = vm.memory.validate_tlv(p).expect("valid output TLV");
    assert_eq!(tlv.type_id as u16, PointerType::Json as u16);
    let v: norito::json::Value = common::json_from_payload(tlv.payload);
    let domains = v
        .get("domains")
        .and_then(|v| v.as_array())
        .expect("domains array")
        .iter()
        .map(|v| v.as_str().expect("domain string").to_string())
        .collect::<Vec<_>>();
    assert_eq!(domains, vec![alpha.to_string(), beta.to_string()]);
}

#[test]
fn query_list_accounts_for_domain_returns_linked_subjects() {
    let wonderland: DomainId = "wonderland".parse().unwrap();
    let other: DomainId = "other".parse().unwrap();
    let admin = account(
        "wonderland",
        "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774",
    );
    let alice_wonderland = account(
        "wonderland",
        "ed01201509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4",
    );
    let alice_other = account(
        "other",
        "ed01201509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4",
    );
    let bob_wonderland = account(
        "wonderland",
        "ed012026DB3C0E3D6A4C53E2CD59000B2D5F9ECB41D4EDD5E0C83F9F1B40D0F0A5BF42",
    );

    let mut wsv = MockWorldStateView::new();
    wsv.add_account_unchecked(admin.clone());
    wsv.grant_permission(&admin, PermissionToken::RegisterDomain);
    wsv.grant_permission(&admin, PermissionToken::RegisterAccount);
    assert!(wsv.register_account(&admin, alice_wonderland.clone()));
    assert!(wsv.register_account(&admin, bob_wonderland.clone()));
    assert!(wsv.register_domain(&admin, other.clone()));
    assert!(wsv.register_account(&admin, alice_other.clone()));

    let host = WsvHost::new_with_subject(
        wsv,
        ivm::mock_wsv::AccountSubjectId::from(&admin),
        Default::default(),
    );
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(host);

    let payload = norito::json::object([(
        "domain_id",
        norito::json::to_value(&wonderland.to_string()).expect("serialize domain"),
    )])
    .expect("serialize payload");
    let env = norito::json::object([
        (
            "type",
            norito::json::to_value("wsv.list_accounts_for_domain").expect("serialize type"),
        ),
        ("payload", payload),
    ])
    .expect("serialize envelope");

    let p = run_query(&mut vm, env);
    let tlv = vm.memory.validate_tlv(p).expect("valid output TLV");
    assert_eq!(tlv.type_id as u16, PointerType::Json as u16);
    let v: norito::json::Value = common::json_from_payload(tlv.payload);
    let expected_domain = wonderland.to_string();
    assert_eq!(
        v.get("domain_id").and_then(|v| v.as_str()),
        Some(expected_domain.as_str())
    );
    let account_ids = v
        .get("account_ids")
        .and_then(|v| v.as_array())
        .expect("account_ids array")
        .iter()
        .map(|value| value.as_str().expect("account id string").to_string())
        .collect::<std::collections::BTreeSet<_>>();
    let expected = [
        admin.to_string(),
        alice_wonderland.to_string(),
        bob_wonderland.to_string(),
    ]
    .into_iter()
    .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(account_ids, expected);
}
