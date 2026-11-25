//! Kotodama tuple-return demo: compile a function returning pointer-backed integers.
use iroha_primitives::numeric_abi::IntValueV1;
use ivm::{IVM, PointerType};
use kotodama_lang::compiler::{Compiler, CompilerOptions};
fn returned_int(vm: &IVM, word_index: usize) -> i64 {
    let pointer = vm
        .public_call_result_word(word_index)
        .expect("read public completed result-table word");
    let tlv = vm
        .validate_tlv(pointer)
        .unwrap_or_else(|error| panic!("validate returned int at word {word_index}: {error:?}"));
    assert_eq!(tlv.type_id, PointerType::Int);
    IntValueV1::decode_frame(tlv.payload)
        .expect("decode canonical returned int")
        .into_int()
        .try_to_i64()
        .expect("demo result fits i64")
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1) Inline Kotodama source: a main that returns a pair (a+1, b+1).
    let src = r#"
        seiyaku TupleReturnDemo {
            view fn main() authorize(anyone) -> (int, int) {
                let int a = 3;
                let int b = 5;
                let t = (a + 1, b + 1);
                let x = t.0;
                let y = t.1;
                return (x, y);
            }
        }
    "#;
    // 2) Compile to IVM bytecode
    let options = CompilerOptions::default();
    let profile = options.target_profile()?;
    let compiler = Compiler::new_with_options(options);
    let code = compiler.compile_source(src).expect("compile tuple return");
    let contract = ivm::prepare_contract_for_profile(code.into(), profile)?;
    let mut vm = IVM::new(1_000_000);
    vm.load_prepared(&contract)?;
    vm.select_entrypoint("main")?;
    vm.run().expect("run");
    // 4) Read canonical `IntValueV1` values from the completed result table.
    assert_eq!(vm.call_result_word_count()?, 2);
    let out0 = returned_int(&vm, 0);
    let out1 = returned_int(&vm, 1);
    println!("tuple return -> ({out0} , {out1})");
    assert_eq!(out0, 4);
    assert_eq!(out1, 6);
    Ok(())
}
