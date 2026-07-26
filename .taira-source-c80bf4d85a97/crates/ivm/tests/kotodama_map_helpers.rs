//! Focused coverage for the stable map-helper surface.

use std::{collections::HashMap, str::FromStr};

use ivm::{
    IVM,
    kotodama::{
        compiler::Compiler,
        ir::{self, Instr, Terminator},
        parser::parse,
        semantic::analyze,
    },
    mock_wsv::{AccountId, MockWorldStateView, WsvHost},
};
mod common;

#[test]
fn ephemeral_map_constructor_is_rejected() {
    let src = r#"
        seiyaku LegacyMap {
          fn main() -> int {
            let m = Map::new();
            return 0;
          }
        }
    "#;
    let err = Compiler::new()
        .compile_source(src)
        .expect_err("ephemeral Map::new must be rejected");
    assert!(
        err.contains("error[K2002]") && err.contains("unknown function or builtin `Map::new`"),
        "unexpected error: {err}"
    );
}

#[test]
fn get_or_state_map() {
    let src = r#"
        seiyaku StateMapHelpers {
          state StateMap<int, int> m;

          kotoage fn main() -> int authorize("WriteState") {
              m[7] = 111;
              let a = m.get_or(key: 7, default: 5);
              let b = m.get_or(key: 8, default: 9);
              return a * 2 + b;
          }
        }
    "#;
    let code = Compiler::new().compile_source(src).expect("compile");
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(ivm::CoreHost::new());
    vm.load_program(&code).unwrap();
    common::select_kotodama_entrypoint(&mut vm, &code, "main");
    vm.run().expect("execute");
    assert_eq!(common::decode_i64_register(&vm, 10), 111 * 2 + 9);
}

#[test]
fn ir_lower_ensure_state_map() {
    let src = r#"
        seiyaku EnsureLowering {
          state StateMap<int, int> m;
          kotoage fn f(int k) -> int authorize("WriteState") { return m.ensure(key: k); }
        }
    "#;
    let prog = parse(src).expect("parse ensure");
    let typed = analyze(&prog).expect("analyze ensure");
    let lowered = ir::lower(&typed).expect("lower");
    let f = &lowered.functions[0];
    let mut saw_get = false;
    let mut saw_set = false;
    let mut saw_branch = false;
    for bb in &f.blocks {
        for ins in &bb.instrs {
            match ins {
                Instr::StateGet { .. } => saw_get = true,
                Instr::StateSet { .. } => saw_set = true,
                _ => {}
            }
        }
        if matches!(bb.terminator, Terminator::Branch { .. }) {
            saw_branch = true;
        }
    }
    assert!(saw_get && saw_set && saw_branch);
}

#[test]
fn semantic_ensure_pointer_requires_explicit_default() {
    let src = r#"
        seiyaku EnsurePointer {
          state StateMap<int, Name> m;
          fn f() { let _ = m.ensure(1); }
        }
    "#;
    let prog = parse(src).expect("parse pointer map without default");
    let err = analyze(&prog).expect_err("pointer-valued ensure should require default");
    assert!(
        err.message()
            .contains("requires an explicit default for pointer-valued maps")
    );
}

#[test]
fn semantic_ensure_non_int_requires_explicit_default() {
    let src = r#"
        seiyaku EnsureBool {
          state StateMap<int, bool> m;
          fn f() { let _ = m.ensure(1); }
        }
    "#;
    let prog = parse(src).expect("parse bool map without default");
    let err = analyze(&prog).expect_err("non-int map should require explicit default");
    assert!(err.message().contains("auto-default is only available"));
}

#[test]
fn ir_lower_ensure_pointer_variants_use_pointer_syscalls() {
    let cases = [
        ("Name", r#"Name::parse("alias")"#),
        (
            "AccountId",
            r#"AccountId::parse("sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV")"#,
        ),
        (
            "AssetDefinitionId",
            r#"AssetDefinitionId::parse("62Fk4FPcMuLvW5QjDGNF2a4jAmjM")"#,
        ),
        ("DomainId", r#"DomainId::parse("wonderland.universal")"#),
        ("NftId", r#"NftId::parse("n0$wonderland.universal")"#),
    ];
    for (ty, ctor) in cases {
        let src = format!(
            r#"
        seiyaku C {{
            state StateMap<int, {ty}> S;
            kotoage fn main() -> {ty} authorize("WriteState") {{
                return S.ensure(key: 7, default: {ctor});
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
            .find(|f| f.name == "main")
            .expect("main lowered");
        let entry = func
            .blocks
            .iter()
            .find_map(|block| match block.terminator {
                Terminator::Branch {
                    then_bb, else_bb, ..
                } => Some((then_bb, else_bb)),
                _ => None,
            })
            .expect("durable ensure must branch on state presence");
        let present = func
            .blocks
            .iter()
            .find(|block| block.label == entry.0)
            .expect("present-value branch");
        let absent = func
            .blocks
            .iter()
            .find(|block| block.label == entry.1)
            .expect("absent-value branch");
        let decodes_state = |instruction: &Instr| {
            matches!(
                instruction,
                Instr::DirectHelperSyscall { syscall, .. }
                    if *syscall == ivm::syscalls::SYSCALL_STATE_VALUE_DECODE
            )
        };
        let encodes_state =
            |instruction: &Instr| matches!(instruction, Instr::StateValueEncode { .. });
        assert!(
            present.instrs.iter().any(decodes_state),
            "present branch must decode the active stored {ty} value"
        );
        assert!(
            present
                .instrs
                .iter()
                .all(|instruction| !encodes_state(instruction)),
            "present branch must not materialize the inactive default {ty} value"
        );
        assert!(
            absent.instrs.iter().any(encodes_state),
            "absent branch must encode the active default {ty} value"
        );
        assert!(
            absent
                .instrs
                .iter()
                .all(|instruction| !decodes_state(instruction)),
            "absent branch must not decode an inactive stored {ty} value"
        );
        let mut saw_state_set = false;
        for bb in &func.blocks {
            for ins in &bb.instrs {
                if matches!(ins, Instr::StateSet { .. }) {
                    saw_state_set = true;
                }
            }
        }
        assert!(
            saw_state_set,
            "absent branch must persist the schema-bound {ty} default"
        );
    }
}

#[test]
fn runtime_durable_ensure_state_map() {
    let src = r#"
        seiyaku C {
            state StateMap<int, int> S;
            kotoage fn main() -> int authorize("WriteState") {
                let x = S.ensure(key: 7);
                let y = S.ensure(key: 7);
                return x + y;
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
    common::select_kotodama_entrypoint(&mut vm, &code, "main");
    vm.run().expect("exec");
    assert_eq!(common::decode_i64_register(&vm, 10), 0);

    let host_ref = vm.host_mut_any().unwrap();
    let host = host_ref.downcast_ref::<WsvHost>().unwrap();
    let base = iroha_data_model::prelude::Name::from_str("S").expect("valid Name literal");
    let key = ivm::numeric_tlv::encode_int(&iroha_primitives::bigint::BigInt::from_i128(7))
        .expect("encode canonical pointer-backed StateMap key");
    let expected_path = format!("{}/{}", base.as_ref(), hex::encode(key));
    let mut val = host.wsv.sc_get(&expected_path);
    if val.is_none() {
        let namespaced_path = format!("{}\0\0\0\0\0\0\0{}", char::from(0x01), expected_path);
        val = host.wsv.sc_get(&namespaced_path);
    }
    let val = val.expect("durable state entry should exist");
    assert_eq!(common::decode_int_state_value(&val), 0);
}
