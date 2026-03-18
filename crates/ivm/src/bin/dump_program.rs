fn main() {
    let src = r#"
        struct A { x: int; }
        struct B { a: A; }
        struct C { b: B; }
        struct D { c: C; }
        fn f() -> int {
            let a = A(5);
            let b = B(a);
            let c = C(b);
            let d = D(c);
            return d.c.b.a.x;
        }
    "#;
    let code = ivm::KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile");
    let parsed = ivm::ProgramMetadata::parse(&code).unwrap();
    let mut memory = ivm::Memory::new((code.len() - parsed.header_len) as u64);
    memory.load_code(&code[parsed.header_len..]);
    let mut pc = (parsed.code_offset - parsed.header_len) as u64;
    while pc < memory.code_len() {
        let (word, len) = ivm::decode(&memory, pc).unwrap();
        println!("pc=0x{pc:04x} word=0x{word:08x} len={len}");
        pc += len as u64;
    }
    let mut vm = ivm::IVM::new(u64::MAX);
    vm.load_program(&code).unwrap();
    vm.run().unwrap();
    for reg in 2..=10 {
        println!("x{reg}={}", vm.register(reg));
    }
}
