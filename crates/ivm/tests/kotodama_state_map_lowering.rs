//! Verify Kotodama lowering of `StateMap<i64, i64>` into durable host state.

use std::{collections::HashMap, str::FromStr};

use iroha_crypto::PublicKey;
use iroha_data_model::prelude::Name;
use ivm::{
    CoreHost, IVM, PointerType, encoding, instruction,
    kotodama::{compiler::Compiler as KotodamaCompiler, ir, parser, semantic},
    mock_wsv::{AccountId, MockWorldStateView, WsvHost},
    syscalls,
};
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

fn encoded_state_path(name: &str, key: i64) -> String {
    let key = norito::to_bytes(&key).expect("encode canonical StateMap key");
    format!("{name}/{}", hex::encode(key))
}

fn account(_domain: &str, public_key: &str) -> AccountId {
    let public_key: PublicKey = public_key.parse().expect("public key");
    AccountId::new(public_key)
}

#[test]
fn kotodama_state_map_set_writes_corehost_state() {
    let src = r#"
        seiyaku C {
            state M: StateMap<i64, i64>;
            kotoage fn main() authorize("WriteState") {
                M[1] = 7;
                let _x = M.get(1);
            }
        }
    "#;
    let code = KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile");
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());
    vm.load_program(&code).expect("load");
    common::select_kotodama_entrypoint(&mut vm, &code, "main");
    vm.run().expect("run kotodama");

    // Query CoreHost state via its reversible canonical-Norito key path.
    let path = Name::from_str(&encoded_state_path("M", 1)).expect("valid path");
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
    assert_eq!(common::decode_i64_state_value(tlv.payload), 7);
}

#[test]
fn kotodama_nested_struct_map_roundtrip() {
    let src = r#"
        seiyaku C {
            struct Inner { value: i64; }
            struct Outer { inner: Inner; }
            state state_outer: StateMap<i64, Outer>;
            kotoage fn main() -> i64 authorize("WriteState") {
                state_outer[7] = Outer { inner: Inner { value: 33 } };
                return state_outer.get(7).unwrap_or(Outer { inner: Inner { value: 0 } }).inner.value;
            }
        }
    "#;
    let code = KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile nested map state");
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());
    vm.load_program(&code).expect("load nested map program");
    common::select_kotodama_entrypoint(&mut vm, &code, "main");
    vm.run().expect("execute nested map program");
    assert_eq!(vm.register(10), 33);
}

#[test]
fn kotodama_foreach_map_lowering_uses_compact_loop() {
    let src = r#"
        seiyaku LoopDemo {
            state M: StateMap<i64, i64>;
            view fn main() {
                for (k, v) in M.take(16) {
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
            state M: StateMap<i64, i64>;
            state Mirror: StateMap<i64, i64>;
            kotoage fn main() authorize("WriteState") {
                for (k, v) in M.take(4) {
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
        common::encode_i64_state_value(5),
    )
    .expect("write state index 0");
    wsv.sc_set(
        &encoded_state_path("M", 1),
        common::encode_i64_state_value(9),
    )
    .expect("write state index 1");
    let alice = account(
        "wonderland",
        "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03",
    );
    let host = WsvHost::new_with_subject(wsv, alice.clone(), HashMap::new());
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(host);
    vm.load_program(&code).expect("load loop program");
    common::select_kotodama_entrypoint(&mut vm, &code, "main");
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
        assert_eq!(common::decode_i64_state_value(tlv.payload), expected);
    }
}
