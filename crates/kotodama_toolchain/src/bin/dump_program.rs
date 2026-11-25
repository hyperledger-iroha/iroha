//! Compile, disassemble, and execute a small Kotodama V1 program.
use iroha_primitives::numeric_abi::IntValueV1;
use ivm::{IVM, PointerType, ProgramMetadata};
fn returned_int(vm: &IVM) -> i64 {
    let tlv = vm
        .validate_tlv(vm.public_call_result_word(0).expect("public result word"))
        .unwrap_or_else(|error| {
            panic!("validate returned int in the public result table: {error:?}")
        });
    assert_eq!(tlv.type_id, PointerType::Int);
    IntValueV1::decode_frame(tlv.payload)
        .expect("decode canonical returned int")
        .into_int()
        .try_to_i64()
        .expect("dump program result fits i64")
}
fn main() {
    let src = r"
        seiyaku DumpProgram {
            struct A { int x }
            struct B { A a }
            struct C { B b }
            struct D { C c }
            view fn main() authorize(anyone) -> int {
                let a = A { x: 5 };
                let b = B { a };
                let c = C { b };
                let d = D { c };
                return d.c.b.a.x;
            }
        }
    ";
    let options = kotodama_lang::compiler::CompilerOptions::default();
    let profile = options.target_profile().expect("compiler target");
    let code = kotodama_lang::compiler::Compiler::new_with_options(options)
        .compile_source(src)
        .expect("compile");
    let parsed = ProgramMetadata::parse_for_profile(&code, profile).unwrap();
    let mut memory = ivm::Memory::new();
    memory
        .load_code(&code[parsed.header_len..])
        .expect("validated program fits code region");
    let mut pc = (parsed.code_offset - parsed.header_len) as u64;
    while pc < memory.code_len() {
        let word = ivm::decode(&memory, pc).unwrap();
        println!("pc=0x{pc:04x} word=0x{word:08x}");
        pc += 4;
    }
    let mut vm = IVM::new(u64::MAX);
    vm.load_program_for_profile(&code, profile)
        .expect("load compiled profile");
    vm.select_entrypoint("main")
        .expect("select main entrypoint");
    vm.run().unwrap();
    for reg in 2..10 {
        println!("x{reg}={}", vm.register(reg));
    }
    println!("result={}", returned_int(&vm));
}
