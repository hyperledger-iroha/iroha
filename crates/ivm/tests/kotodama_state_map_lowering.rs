//! Verify Kotodama lowering of `state Map<int,int>` writes into durable host state.

use std::{collections::HashMap, str::FromStr};

use iroha_data_model::prelude::Name;
use ivm::{
    CoreHost, IVM, PointerType, encoding, instruction,
    kotodama::{compiler::Compiler as KotodamaCompiler, ir, parser, semantic},
    mock_wsv::{AccountId, MockWorldStateView, WsvHost},
    syscalls,
};
use norito::{decode_from_bytes, to_bytes};
mod common;

fn make_tlv(pty: PointerType, payload: &[u8]) -> Vec<u8> {
    let payload = common::payload_for_type(pty, payload);
    let mut v = Vec::with_capacity(7 + payload.len() + 32);
    v.extend_from_slice(&(pty as u16).to_be_bytes());
    v.push(1);
    v.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    v.extend_from_slice(payload.as_ref());
    let h: [u8; 32] = iroha_crypto::Hash::new(payload).into();
    v.extend_from_slice(&h);
    v
}

fn encoded_state_path(name: &str, key: impl std::fmt::Display) -> String {
    format!("{name}/{}", key)
}

#[test]
fn kotodama_state_map_set_writes_corehost_state() {
    let src = r#"
        seiyaku C {
            state Map<int, int> M;
            fn main() {
                M[1] = 7; // should lower to host::STATE_SET("M/1", Norito(i64=7)) plus ephemeral.
                let _x = M[1];
            }
        }
    "#;
    let code = KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile");
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());
    vm.load_program(&code).expect("load");
    vm.run().expect("run kotodama");

    // Query CoreHost state via STATE_GET for namespaced path "M/1" and expect payload "7"
    let path = Name::from_str("M/1").expect("valid path");
    let path_tlv = make_tlv(PointerType::Name, path.as_ref().as_bytes());
    let p_path = vm.alloc_input_tlv(&path_tlv).expect("alloc path");
    let mut get_prog_bytes = Vec::new();
    let scall = encoding::wide::encode_sys(
        instruction::wide::system::SCALL,
        syscalls::SYSCALL_STATE_GET as u8,
    );
    get_prog_bytes.extend_from_slice(&scall.to_le_bytes());
    get_prog_bytes.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    let get_prog = common::assemble(&get_prog_bytes);
    vm.set_register(10, p_path);
    vm.load_program(&get_prog).expect("load get");
    vm.run().expect("state get");
    let p_out = vm.register(10);
    let tlv = vm.memory.validate_tlv(p_out).expect("validate out");
    assert_eq!(tlv.type_id, PointerType::NoritoBytes);
    let stored: i64 = decode_from_bytes(tlv.payload).expect("decode stored int");
    assert_eq!(stored, 7);
}

#[test]
fn kotodama_nested_struct_map_roundtrip() {
    let src = r#"
        seiyaku C {
            struct Inner { map: Map<int, int>; }
            struct Outer { inner: Inner; }
            state Outer state_outer;
            fn main() -> int {
                state_outer.inner.map[7] = 33;
                return state_outer.inner.map[7];
            }
        }
    "#;
    let code = KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile nested map state");
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());
    vm.load_program(&code).expect("load nested map program");
    vm.run().expect("execute nested map program");
    assert_eq!(vm.register(10), 33);
}

#[test]
fn kotodama_foreach_map_lowering_uses_compact_loop() {
    let src = r#"
        seiyaku LoopDemo {
            state Map<int, int> M;
            fn main() {
                for (k, v) in M #[bounded(16)] {
                    let _tmp = k + v;
                }
            }
        }
    "#;

    let program = parser::parse(src).expect("parse loop demo");
    let typed = semantic::analyze(&program).expect("semantic analysis");
    let ir_prog = ir::lower(&typed).expect("lower");
    let main_fn = ir_prog
        .functions
        .iter()
        .find(|f| f.name == "main")
        .expect("lowered main");

    let mut state_gets = 0usize;
    let mut map_load_pairs = 0usize;
    for bb in &main_fn.blocks {
        for instr in &bb.instrs {
            match instr {
                ir::Instr::StateGet { .. } => state_gets += 1,
                ir::Instr::MapLoadPair { .. } => map_load_pairs += 1,
                _ => {}
            }
        }
    }

    assert_eq!(
        map_load_pairs, 0,
        "state map lowering should avoid MapLoadPair unrolling"
    );
    assert!(
        state_gets >= 1,
        "state map iteration should fetch from durable state"
    );
}

#[test]
fn kotodama_foreach_reads_durable_state_map_entries() {
    let src = r#"
        seiyaku LoopDemo {
            state Map<int, int> M;
            state Map<int, int> Mirror;
            fn main() {
                for (k, v) in M #[bounded(4)] {
                    Mirror[k] = v;
                }
            }
        }
    "#;

    let code = KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile durable loop");
    let mut wsv = MockWorldStateView::new();
    wsv.sc_set(
        &encoded_state_path("M", 0),
        to_bytes(&5_i64).expect("encode state value 5"),
    )
    .expect("write state index 0");
    wsv.sc_set(
        &encoded_state_path("M", 1),
        to_bytes(&9_i64).expect("encode state value 9"),
    )
    .expect("write state index 1");
    let alice: AccountId =
        "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland"
            .parse()
            .expect("parse account id");
    let host = WsvHost::new(wsv, alice, HashMap::new(), HashMap::new());
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(host);
    vm.load_program(&code).expect("load loop program");
    vm.run().expect("execute loop program");
    // Read back the mirrored entries written inside the loop.
    let mut get_prog_bytes = Vec::new();
    let scall = encoding::wide::encode_sys(
        instruction::wide::system::SCALL,
        syscalls::SYSCALL_STATE_GET as u8,
    );
    get_prog_bytes.extend_from_slice(&scall.to_le_bytes());
    get_prog_bytes.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    let get_prog = common::assemble(&get_prog_bytes);
    for (path, expected) in [
        (encoded_state_path("Mirror", 0), 5_i64),
        (encoded_state_path("Mirror", 1), 9_i64),
    ] {
        let path_tlv = make_tlv(PointerType::Name, path.as_bytes());
        let p_path = vm.alloc_input_tlv(&path_tlv).expect("alloc path");
        vm.set_register(10, p_path);
        vm.load_program(&get_prog).expect("load get");
        vm.run().expect("state get");
        let out = vm.register(10);
        let tlv = vm.memory.validate_tlv(out).expect("validate out");
        assert_eq!(tlv.type_id, PointerType::NoritoBytes);
        let stored: i64 = decode_from_bytes(tlv.payload).expect("decode mirrored int");
        assert_eq!(stored, expected);
    }
}
