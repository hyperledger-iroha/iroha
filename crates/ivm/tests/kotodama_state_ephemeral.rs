//! Durable `StateMap` lowering tests, including struct-valued entries.

use ivm::{CoreHost, IVM, kotodama::compiler::Compiler as KotodamaCompiler};
mod common;

#[test]
fn state_map_set_get_roundtrip() {
    // Declare state map and perform set/get within a single run.
    let src = r#"
        seiyaku C {
            state M: StateMap<i64, i64>;
            kotoage fn main() -> i64 authorize("WriteState") {
                M[1] = 7;
                let x = M.get(1).unwrap_or(0);
                return x;
            }
        }
    "#;
    let compiler = KotodamaCompiler::new();
    let prog = compiler.compile_source(src).expect("compile state map");
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());
    vm.load_program(&prog).expect("load program");
    common::select_kotodama_entrypoint(&mut vm, &prog, "main");
    vm.run().expect("state map roundtrip");
    assert_eq!(vm.register(10), 7);
}

#[test]
fn state_map_with_struct_value_roundtrip() {
    // Store and load a struct through a durable state map.
    let src = r#"
        seiyaku C {
            struct S { value: i64; }
            state values: StateMap<i64, S>;
            kotoage fn main() -> i64 authorize("WriteState") {
                values[3] = S { value: 9 };
                let y = values.get(3).unwrap_or(S { value: 0 }).value;
                return y;
            }
        }
    "#;
    let compiler = KotodamaCompiler::new();
    let prog = compiler
        .compile_source(src)
        .expect("compile state map with struct value");
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());
    vm.load_program(&prog).expect("load program");
    common::select_kotodama_entrypoint(&mut vm, &prog, "main");
    vm.run().expect("state map struct roundtrip");
    assert_eq!(vm.register(10), 9);
}
