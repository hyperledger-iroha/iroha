//! Ephemeral `state` lowering tests: maps and nested struct-held maps.

use ivm::{CoreHost, IVM, kotodama::compiler::Compiler as KotodamaCompiler};

#[test]
fn state_map_set_get_roundtrip() {
    // Declare state map and perform set/get within a single run.
    let src = r#"
        seiyaku C {
            state Map<int, int> M;
            fn main() {
                M[1] = 7;
                let x = M[1];
                assert_eq(x, 7);
            }
        }
    "#;
    let compiler = KotodamaCompiler::new();
    let prog = compiler.compile_source(src).expect("compile state map");
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());
    vm.load_program(&prog).expect("load program");
    vm.run().expect("state map roundtrip");
}

#[test]
fn state_struct_with_map_field_roundtrip() {
    // Declare state struct with a map field and use named field access + indexing.
    let src = r#"
        seiyaku C {
            struct S { m: Map<int, int>; }
            state S s;
            fn main() {
                s.m[3] = 9;
                let y = s.m[3];
                assert_eq(y, 9);
            }
        }
    "#;
    let compiler = KotodamaCompiler::new();
    let prog = compiler
        .compile_source(src)
        .expect("compile state struct with map field");
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());
    vm.load_program(&prog).expect("load program");
    vm.run().expect("state struct map roundtrip");
}
