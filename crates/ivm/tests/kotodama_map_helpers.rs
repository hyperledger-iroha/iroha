//! Focused coverage for the stable map-helper surface.

use std::{collections::HashMap, str::FromStr};

use ivm::{
    IVM, PointerType,
    kotodama::{
        compiler::Compiler,
        ir::{self, Instr, Terminator},
        parser::parse,
        semantic::analyze,
    },
    mock_wsv::{AccountId, MockWorldStateView, WsvHost},
    validate_tlv_bytes,
};

#[test]
fn get_or_default_ephemeral() {
    let src = r#"
        fn f() -> int {
            let m = Map::new();
            m[7] = 111;
            let a = get_or_default(m, 7, 5);
            let b = get_or_default(m, 8, 9);
            return a*2 + b;
        }
    "#;
    let code = Compiler::new().compile_source(src).expect("compile");
    let mut vm = IVM::new(u64::MAX);
    vm.load_program(&code).unwrap();
    vm.run().expect("execute");
    assert_eq!(vm.register(10), 111 * 2 + 9);
}

#[test]
fn get_or_default_durable() {
    let src = r#"
        state Map<int,int> m;
        fn f() -> int {
            m[7] = 111;
            let a = get_or_default(m, 7, 5);
            let b = get_or_default(m, 8, 9);
            return a*2 + b;
        }
    "#;
    let code = Compiler::new().compile_source(src).expect("compile");
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(ivm::CoreHost::new());
    vm.load_program(&code).unwrap();
    vm.run().expect("execute");
    assert_eq!(vm.register(10), 111 * 2 + 9);
}

#[test]
fn ir_lower_ensure_ephemeral() {
    let src = "fn f(m: Map<int,int>, k: int) -> int { return m.ensure(k); }";
    let prog = parse(src).expect("parse ensure");
    let typed = analyze(&prog).expect("analyze ensure");
    let lowered = ir::lower(&typed).expect("lower");
    let f = &lowered.functions[0];
    let mut saw_pair = false;
    let mut saw_set = false;
    let mut saw_branch = false;
    for bb in &f.blocks {
        for ins in &bb.instrs {
            match ins {
                Instr::MapLoadPair { .. } => saw_pair = true,
                Instr::MapSet { .. } => saw_set = true,
                _ => {}
            }
        }
        if matches!(bb.terminator, Terminator::Branch { .. }) {
            saw_branch = true;
        }
    }
    assert!(saw_pair && saw_set && saw_branch);
}

#[test]
fn semantic_ensure_pointer_requires_explicit_default() {
    let src = "fn f(m: Map<int, Name>) { let _ = m.ensure(1); }";
    let prog = parse(src).expect("parse pointer map without default");
    let err = analyze(&prog).expect_err("pointer-valued ensure should require default");
    assert!(
        err.message
            .contains("requires an explicit default for pointer-valued maps")
    );
}

#[test]
fn semantic_ensure_non_int_requires_explicit_default() {
    let src = "fn f(m: Map<int, bool>) { let _ = m.ensure(1); }";
    let prog = parse(src).expect("parse bool map without default");
    let err = analyze(&prog).expect_err("non-int map should require explicit default");
    assert!(
        err.message
            .contains("auto-default is only available for Map<*,int>")
    );
}

#[test]
fn ir_lower_ensure_pointer_variants_use_pointer_syscalls() {
    let cases = [
        ("Name", r#"name("alias")"#),
        (
            "AccountId",
            r#"account_id("sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB")"#,
        ),
        (
            "AssetDefinitionId",
            r#"asset_definition("62Fk4FPcMuLvW5QjDGNF2a4jAmjM")"#,
        ),
        ("DomainId", r#"domain("wonderland")"#),
        ("NftId", r#"nft_id("rose:uuid:0123$wonderland")"#),
    ];
    for (ty, ctor) in cases {
        let src = format!(
            r#"
        seiyaku C {{
            state S: Map<int, {ty}>;
            fn hajimari() -> {ty} {{
                return S.ensure(7, {ctor});
            }}
        }}
        "#
        );
        let prog = parse(&src).expect("parse pointer durable map");
        let typed = analyze(&prog).expect("analyze pointer durable map");
        let lowered = ir::lower(&typed).expect("lower");
        let func = lowered
            .functions
            .iter()
            .find(|f| f.name == "hajimari")
            .expect("hajimari lowered");
        let mut saw_pointer_to = false;
        let mut saw_pointer_from = false;
        for bb in &func.blocks {
            for ins in &bb.instrs {
                match ins {
                    Instr::PointerToNorito { .. } => saw_pointer_to = true,
                    Instr::PointerFromNorito { .. } => saw_pointer_from = true,
                    _ => {}
                }
            }
        }
        assert!(
            saw_pointer_to,
            "durable else branch should encode pointer defaults for {ty}"
        );
        assert!(
            saw_pointer_from,
            "durable then branch should decode stored pointer for {ty}"
        );
    }
}

#[test]
fn runtime_durable_ensure_state_map() {
    let src = r#"
        seiyaku C {
            state S: Map<int,int>;
            fn hajimari() {
                let x = S.ensure(7);
                assert(x == 0);
                let y = S.ensure(7);
                assert(y == 0);
            }
        }
    "#;
    let code = Compiler::new()
        .compile_source(src)
        .expect("compile durable ensure");
    let mut vm = IVM::new(u64::MAX);
    vm.load_program(&code).expect("load");
    let wsv = MockWorldStateView::new();
    let alice: AccountId = AccountId::new(
        "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
            .parse()
            .expect("public key"),
    );
    let host = WsvHost::new_with_subject(wsv, alice, HashMap::new());
    vm.set_host(host);
    vm.run().expect("exec");

    let host_ref = vm.host_mut_any().unwrap();
    let host = host_ref.downcast_ref::<WsvHost>().unwrap();
    let base = iroha_data_model::prelude::Name::from_str("S").expect("valid Name literal");
    let expected_path = format!("{}/{}", base.as_ref(), 7);
    let mut val = host.wsv.sc_get(&expected_path);
    if val.is_none() {
        let namespaced_path = format!("{}\0\0\0\0\0\0\0{}", char::from(0x01), expected_path);
        val = host.wsv.sc_get(&namespaced_path);
    }
    let val = val.expect("durable state entry should exist");
    let tlv = validate_tlv_bytes(&val).expect("state entry should use NoritoBytes TLV");
    assert_eq!(tlv.type_id, PointerType::NoritoBytes);
    let stored: i64 =
        norito::decode_from_bytes(tlv.payload).expect("durable int value should be Norito i64");
    assert_eq!(stored, 0);
}
