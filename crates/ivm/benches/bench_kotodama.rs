//! Benchmarks for Kotodama program parsing and execution in IVM.
use criterion::Criterion;
use ivm::{
    IVM, ProgramMetadata, encoding,
    kotodama::compiler::{Compiler, encode_add},
};

const LITERAL_BENCH_SIZE: usize = 512;

fn kotodama_program() -> Vec<u8> {
    let src = "fn add(a, b) { let c = a + b; }";
    Compiler::new().compile_source(src).expect("compile failed")
}

fn asm_program() -> Vec<u8> {
    let mut prog = ProgramMetadata::default().encode();
    let add = encode_add(3, 10, 11).to_le_bytes();
    prog.extend_from_slice(&add);
    prog.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    prog
}

fn bench_kotodama(c: &mut Criterion) {
    let code = kotodama_program();
    c.bench_function("kotodama_add", |b| {
        b.iter(|| {
            let mut vm = IVM::new(u64::MAX);
            vm.set_register(10, 4);
            vm.set_register(11, 7);
            vm.load_program(&code).unwrap();
            vm.run().unwrap();
            std::hint::black_box(vm.register(3));
        })
    });
}

fn bench_asm(c: &mut Criterion) {
    let code = asm_program();
    c.bench_function("asm_add", |b| {
        b.iter(|| {
            let mut vm = IVM::new(u64::MAX);
            vm.set_register(10, 4);
            vm.set_register(11, 7);
            vm.load_program(&code).unwrap();
            vm.run().unwrap();
            std::hint::black_box(vm.register(3));
        })
    });
}

fn literal_heavy_source(count: usize) -> String {
    let mut src = String::from("fn main() {\n");
    for i in 0..count {
        src.push_str(&format!(
            "  set_account_detail(authority(), name(\"literal{i}\"), json(\"{{\\\"value\\\":{i}}}\"));\n"
        ));
    }
    src.push_str("}\n");
    src
}

fn bench_literal_heavy_compile(c: &mut Criterion) {
    let src = literal_heavy_source(LITERAL_BENCH_SIZE);
    let compiler = Compiler::new();
    c.bench_function("kotodama_literal_heavy_compile", |b| {
        b.iter(|| {
            let bytes = compiler
                .compile_source(&src)
                .expect("literal heavy program compiles");
            std::hint::black_box(bytes);
        })
    });
}

/// Entry point for the benchmark binary.
fn main() {
    // Silence ASCII banner and feature selection in benches.
    ivm::set_banner_enabled(false);
    let mut c = Criterion::default().configure_from_args();
    bench_kotodama(&mut c);
    bench_asm(&mut c);
    bench_literal_heavy_compile(&mut c);
    c.final_summary();
}
