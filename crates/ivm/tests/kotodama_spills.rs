//! Tests that exercise spilled temporaries and nested calls with spills.

use ivm::{IVM, kotodama::compiler::Compiler as KotodamaCompiler};

#[test]
fn many_locals_force_spills_and_compute() {
    // Create a chain of additions to overflow the allocatable register pool.
    // The allocator reserves 18 regs; this uses ~40 distinct temps.
    let mut body = String::new();
    body.push_str("let a0 = 0;\n");
    for i in 1..40 {
        body.push_str(&format!("let a{} = a{} + 1;\n", i, i - 1));
    }
    body.push_str("return a39;\n");
    let src = format!("fn main() -> int {{\n{body}\n}}");
    let code = KotodamaCompiler::new()
        .compile_source(&src)
        .expect("compile spills");
    let mut vm = IVM::new(u64::MAX);
    vm.load_program(&code).unwrap();
    vm.run().expect("execute spills");
    assert_eq!(vm.register(10), 39);
}

#[test]
fn literal_heavy_set_account_detail_compiles_under_spill_pressure() {
    const COUNT: usize = 256;
    let mut src = String::from("fn main() {\n");
    for i in 0..COUNT {
        src.push_str(&format!(
            "  set_account_detail(authority(), name(\"literal{i}\"), json(\"{{\\\"value\\\":{i}}}\"));\n"
        ));
    }
    src.push_str("}\n");

    KotodamaCompiler::new()
        .compile_source(&src)
        .expect("literal-heavy set_account_detail under spills");
}
