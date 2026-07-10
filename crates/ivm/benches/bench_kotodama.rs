//! Benchmarks for phase-separated Kotodama compilation and execution in IVM.
use std::{collections::BTreeMap, sync::Arc};

use criterion::Criterion;
use iroha_crypto::Hash;
use iroha_data_model::prelude::Name;
use iroha_primitives::json::Json;
use ivm::{
    IVM, ProgramMetadata, encoding,
    host::DefaultHost,
    kotodama::{
        compiler::{Compiler, encode_add},
        ir, parser,
        semantic::SemanticContext,
    },
    pointer_abi::PointerType,
};

const LITERAL_BENCH_SIZE: usize = 512;

fn kotodama_program() -> Vec<u8> {
    let src = "seiyaku Add { view fn add(a: i64, b: i64) -> i64 { return a + b; } }";
    Compiler::new().compile_source(src).expect("compile failed")
}

fn entrypoint_pc(program: &[u8], name: &str) -> u64 {
    let parsed = ProgramMetadata::parse(program).expect("parse Kotodama metadata");
    let entrypoint = parsed
        .contract_interface
        .as_ref()
        .expect("Kotodama artifact has CNTR")
        .entrypoints
        .iter()
        .find(|entrypoint| entrypoint.name == name)
        .expect("benchmark entrypoint exists");
    u64::try_from(parsed.prefix_len()).expect("prefix fits u64") + entrypoint.entry_pc
}

fn tlv(pointer_type: PointerType, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(7 + payload.len() + Hash::LENGTH);
    out.extend_from_slice(&(pointer_type as u16).to_be_bytes());
    out.push(1);
    out.extend_from_slice(
        &u32::try_from(payload.len())
            .expect("benchmark payload fits u32")
            .to_be_bytes(),
    );
    out.extend_from_slice(payload);
    out.extend_from_slice(Hash::new(payload).as_ref());
    out
}

fn add_argument_host(program: &[u8]) -> DefaultHost {
    let parsed = ProgramMetadata::parse(program).expect("parse Kotodama metadata");
    let schema = parsed
        .contract_interface
        .as_ref()
        .expect("Kotodama artifact has CNTR")
        .entrypoints
        .iter()
        .find(|entrypoint| entrypoint.name == "add")
        .and_then(|entrypoint| entrypoint.argument_schema.as_ref())
        .expect("benchmark entrypoint has an argument schema");
    let payload = Json::from(norito::json!({"a": 4, "b": 7}));
    let payload = ivm::encode_argument_record_from_json(schema, &payload)
        .expect("encode canonical benchmark argument record");
    let key: Name = "trigger_event_json"
        .parse()
        .expect("public input key is a Name");
    DefaultHost::new().with_public_inputs(BTreeMap::from([(
        key,
        tlv(PointerType::NoritoBytes, &payload),
    )]))
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
    let pc = entrypoint_pc(&code, "add");
    let host = add_argument_host(&code);
    let prepared =
        ivm::prepare_contract(Arc::from(code.clone())).expect("prepare benchmark contract once");
    assert_eq!(prepared.entrypoint_pc("add"), Some(pc));

    // Keep the benchmark honest: CNTR artifacts deliberately halt until an
    // entrypoint is selected, so validate the measured path before sampling.
    let mut verification_vm = IVM::new(u64::MAX);
    verification_vm
        .load_prepared(&prepared)
        .expect("load prepared benchmark artifact");
    verification_vm
        .set_program_counter(pc)
        .expect("select benchmark entrypoint");
    verification_vm.set_host(host.clone());
    verification_vm.run().expect("execute benchmark add");
    assert_eq!(verification_vm.register(10), 11);

    c.bench_function("kotodama_runtime_cold_add", |b| {
        b.iter(|| {
            let mut vm = IVM::new(u64::MAX);
            vm.load_program(&code).unwrap();
            vm.set_program_counter(pc).unwrap();
            vm.set_host(host.clone());
            vm.run().unwrap();
            std::hint::black_box(vm.register(10));
        })
    });

    let mut vm = IVM::new(u64::MAX);
    vm.load_prepared(&prepared)
        .expect("load warm prepared benchmark artifact");
    vm.set_program_counter(pc)
        .expect("select warm benchmark entrypoint");
    let template = vm.runtime_template();
    c.bench_function("kotodama_runtime_warm_add", |b| {
        b.iter(|| {
            vm.reset_from_runtime_template(&template);
            vm.set_host(host.clone());
            vm.run().unwrap();
            std::hint::black_box(vm.register(10));
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
    let mut src = String::from("seiyaku Literals {\n  kotoage fn main() authorize(\"Bench\") {\n");
    for i in 0..count {
        src.push_str(&format!(
            "    ledger::account::set_detail(context::authority(), Name::parse(\"literal{i}\"), Json::parse(\"{{\\\"value\\\":{i}}}\"));\n"
        ));
    }
    src.push_str("  }\n}\n");
    src
}

fn bench_compiler_phases(c: &mut Criterion) {
    let source = literal_heavy_source(64);
    c.bench_function("kotodama_phase_parse", |b| {
        b.iter(|| std::hint::black_box(parser::parse(&source).expect("parse benchmark source")))
    });

    let parsed = parser::parse(&source).expect("parse benchmark source");
    c.bench_function("kotodama_phase_semantic", |b| {
        b.iter(|| {
            let context = SemanticContext::new();
            std::hint::black_box(context.analyze(&parsed).expect("analyze benchmark source"))
        })
    });

    let semantic_context = SemanticContext::new();
    let typed = semantic_context
        .analyze(&parsed)
        .expect("analyze benchmark source");
    c.bench_function("kotodama_phase_ir_lower", |b| {
        b.iter(|| std::hint::black_box(ir::lower(&typed).expect("lower benchmark source")))
    });
}

fn bench_literal_heavy_compile(c: &mut Criterion) {
    let src = literal_heavy_source(LITERAL_BENCH_SIZE);
    let compiler = Compiler::new();
    c.bench_function("kotodama_phase_codegen_end_to_end", |b| {
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
    bench_compiler_phases(&mut c);
    bench_literal_heavy_compile(&mut c);
    c.final_summary();
}
