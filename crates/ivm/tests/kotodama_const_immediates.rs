//! Kotodama integer constant lowering regression tests.

use ivm::{
    CoreHost, IVM, ProgramMetadata, encoding, instruction,
    kotodama::compiler::Compiler as KotodamaCompiler,
};
mod common;

#[test]
fn compile_large_positive_constant_executes() {
    let src = r#"
        seiyaku LargePositiveConstant {
            view fn main() -> i64 {
                let x = 123_456_789_012i64;
                return x;
            }
        }
    "#;
    let code = KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile large positive constant");
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());
    vm.load_program(&code)
        .expect("load large positive constant program");
    common::select_kotodama_entrypoint(&mut vm, &code, "main");
    vm.run().expect("run large positive constant program");
    assert_eq!(vm.register(10) as i64, 123_456_789_012);
}

#[test]
fn compile_large_negative_constant_executes() {
    let src = r#"
        seiyaku LargeNegativeConstant {
            view fn main() -> i64 {
                let x = -987_654_321_098i64;
                return x;
            }
        }
    "#;
    let code = KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile large negative constant");
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());
    vm.load_program(&code)
        .expect("load large negative constant program");
    common::select_kotodama_entrypoint(&mut vm, &code, "main");
    vm.run().expect("run large negative constant program");
    assert_eq!(vm.register(10) as i64, -987_654_321_098);
}

#[test]
fn signed_boundary_constants_are_one_word_deduplicated_and_deterministic() {
    let source = r#"
        seiyaku IndexedConstants {
            view fn minimum() -> i64 { return -9223372036854775808i64; }
            view fn maximum() -> i64 { return 9223372036854775807i64; }
            view fn maximum_again() -> i64 { return 9223372036854775807i64; }
        }
    "#;
    let compiler = KotodamaCompiler::new();
    let artifact = compiler
        .compile_source(source)
        .expect("compile signed boundaries");
    assert_eq!(
        artifact,
        compiler
            .compile_source(source)
            .expect("repeat signed-boundary compilation")
    );

    let metadata = ProgramMetadata::parse(&artifact).expect("parse signed-boundary artifact");
    let section = metadata.literal_section.expect("indexed scalar table");
    assert_eq!(
        section.count, 2,
        "MIN and deduplicated MAX must occupy exactly two scalar entries"
    );
    let words = artifact[metadata.code_offset..]
        .chunks_exact(4)
        .map(|word| u32::from_le_bytes(word.try_into().expect("instruction word")))
        .collect::<Vec<_>>();
    let loads = words
        .iter()
        .copied()
        .filter(|word| instruction::wide::opcode(*word) == instruction::wide::memory::LDI64)
        .collect::<Vec<_>>();
    assert_eq!(loads.len(), 3, "each ordinary scalar load must be one word");
    assert!(
        words
            .iter()
            .all(|word| { instruction::wide::opcode(*word) != instruction::wide::arithmetic::SLL })
    );
    let mut indices = loads
        .iter()
        .map(|word| encoding::wide::decode_literal(*word).2)
        .collect::<Vec<_>>();
    indices.sort_unstable();
    assert!(
        indices[0] == indices[1] || indices[1] == indices[2],
        "the repeated MAX literal must share one table index: {indices:?}"
    );

    for (entrypoint, expected) in [
        ("minimum", i64::MIN),
        ("maximum", i64::MAX),
        ("maximum_again", i64::MAX),
    ] {
        let mut vm = IVM::new(u64::MAX);
        vm.load_program(&artifact).expect("load boundary artifact");
        common::select_kotodama_entrypoint(&mut vm, &artifact, entrypoint);
        vm.run().expect("run boundary entrypoint");
        assert_eq!(vm.register(10) as i64, expected);
    }
}
