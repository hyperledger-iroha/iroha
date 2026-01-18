#[test]
fn debug_contains() {
    let src = r#"
        fn f() -> int {
            let m = Map::new();
            m[7] = 111;
            let t = contains(m, 7);
            let f = contains(m, 8);
            return t*2 + f;
        }
    "#;
    let code = ivm::KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile");
    if std::env::var_os("IVM_DEBUG_IR").is_some() {
        let ast = ivm::kotodama::parser::parse(src).expect("parse");
        let typed = ivm::kotodama::semantic::analyze(&ast).expect("analyze");
        let ir_prog = ivm::kotodama::ir::lower(&typed).expect("lower");
        for func in &ir_prog.functions {
            eprintln!("[IR] function {}", func.name);
            for block in &func.blocks {
                eprintln!("  block {}", block.label.0);
                for instr in &block.instrs {
                    eprintln!("    {instr:?}");
                }
                eprintln!("    terminator {:?}", block.terminator);
            }
        }
    }
    let mut vm = ivm::IVM::new(u64::MAX);
    vm.load_program(&code).unwrap();
    vm.run().unwrap_or_else(|e| {
        let map_ptr = vm.register(2);
        panic!("run failed: {e:?}, map_ptr=0x{map_ptr:x}");
    });
    let result = vm.register(10);
    assert_eq!(result, 2);

    // Ensure the map entry persisted: the first key/value pair should match our insert.
    let map_ptr = ivm::Memory::HEAP_START;
    let mut buf = [0u8; 8];
    vm.load_bytes(map_ptr, &mut buf).expect("load key bytes");
    let key = u64::from_le_bytes(buf);
    vm.load_bytes(map_ptr + 8, &mut buf)
        .expect("load value bytes");
    let value = u64::from_le_bytes(buf);
    assert_eq!(key, 7);
    assert_eq!(value, 111);
}
