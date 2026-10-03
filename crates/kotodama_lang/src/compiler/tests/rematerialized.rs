//! Same-compiler artifact and rejection controls for literal-home omission.

use crate::{
    compiler::CompilerOptions,
    instruction,
    ir::{DataRefKind, Instr, Temp},
    metadata::ProgramMetadata,
    regalloc::with_literal_homes,
    session::{CompileOutput, CompileRequest, CompilerSession},
};

fn compile(source: &str, retain_homes: bool) -> CompileOutput {
    crate::session::run_with_compiler_stack(|| {
        let build = || {
            CompilerSession::new(CompilerOptions::default())
                .build(CompileRequest {
                    source,
                    source_name: Some("literal_homes.ko"),
                })
                .expect("compile through the canonical validated source pipeline")
        };
        if retain_homes {
            with_literal_homes(build)
        } else {
            build()
        }
    })
    .expect("canonical compiler worker")
}

fn assert_same_semantic_metadata(baseline: &CompileOutput, optimized: &CompileOutput) {
    let mut before = baseline.manifest.clone();
    let mut after = optimized.manifest.clone();
    before.code_hash = None;
    after.code_hash = None;
    assert_eq!(
        before, after,
        "public schemas, authorities, exports, errors, state and hints remain exact"
    );
    let callables = |output: &CompileOutput| {
        output
            .report
            .budget_report
            .iter()
            .map(|function| {
                let callable = output
                    .contract_interface
                    .callables
                    .iter()
                    .find(|callable| callable.entry_pc == function.pc_start)
                    .expect("authenticated function descriptor");
                assert!(callable.validate());
                assert_eq!(callable.frame_bytes, function.frame_bytes);
                (
                    function.function_name.clone(),
                    (callable.arguments.clone(), callable.results.clone()),
                )
            })
            .collect::<std::collections::BTreeMap<_, _>>()
    };
    assert_eq!(
        callables(baseline),
        callables(optimized),
        "not one executable or callable schema is omitted"
    );
    let normalize = |output: &CompileOutput| {
        let mut interface = output.contract_interface.clone();
        for entrypoint in &mut interface.entrypoints {
            entrypoint.entry_pc = 0;
        }
        interface.callables.clear(); // Compared by function identity above; only PC/frame geometry can differ.
        interface
    };
    assert_eq!(normalize(baseline), normalize(optimized));
    let before = ProgramMetadata::parse(&baseline.artifact).expect("baseline metadata");
    let after = ProgramMetadata::parse(&optimized.artifact).expect("optimized metadata");
    let header = |metadata: &ProgramMetadata| {
        (
            metadata.version_major,
            metadata.version_minor,
            metadata.mode,
            metadata.vector_length,
            metadata.max_cycles,
            metadata.abi_version,
        )
    };
    assert_eq!(
        header(&before.metadata),
        header(&after.metadata),
        "no ABI, gas or execution-mode policy changes"
    );
}

#[test]
fn rematerialized_literals_reduce_emitted_memory_traffic_and_authenticated_frames() {
    let source = include_str!("../fixtures/v1/literal_homes.ko");
    let baseline = compile(source, true);
    let optimized = compile(source, false);
    assert_same_semantic_metadata(&baseline, &optimized);
    let memory_words = |output: &CompileOutput| {
        let metadata = ProgramMetadata::parse(&output.artifact).unwrap();
        output.artifact[metadata.code_offset..]
            .chunks_exact(4)
            .filter(|word| {
                matches!(
                    instruction::wide::opcode(u32::from_le_bytes((*word).try_into().unwrap())),
                    instruction::wide::memory::LOAD64 | instruction::wide::memory::STORE64
                )
            })
            .count()
    };
    let frame = |output: &CompileOutput| {
        output
            .report
            .budget_report
            .iter()
            .find(|function| function.function_name == "run")
            .expect("run report")
            .frame_bytes
    };
    assert!(optimized.artifact.len() < baseline.artifact.len());
    assert!(
        memory_words(&optimized) < memory_words(&baseline),
        "actual LOAD64/STORE64 words must decrease, not just a sidecar estimate"
    );
    assert!(frame(&optimized) < frame(&baseline));
    assert_eq!(compile(source, false).artifact, optimized.artifact);
    eprintln!(
        "literal-home pressure: baseline_bytes={} optimized_bytes={} baseline_memory_words={} optimized_memory_words={} baseline_run_frame={} optimized_run_frame={}",
        baseline.artifact.len(),
        optimized.artifact.len(),
        memory_words(&baseline),
        memory_words(&optimized),
        frame(&baseline),
        frame(&optimized)
    );
}

#[test]
fn rematerialized_unused_literals_preserve_canonical_codegen_errors() {
    for (kind, invalid) in [
        (DataRefKind::Int, "bad-int"),
        (DataRefKind::Decimal, "bad-decimal"),
        (DataRefKind::Quantity, "bad-quantity"),
    ] {
        let emit = || {
            crate::session::run_with_compiler_stack(|| {
                super::compile_with_injected_ir(vec![Instr::DataRef {
                    dest: Temp(777),
                    kind,
                    value: invalid.into(),
                }])
            })
            .unwrap()
        };
        let baseline = crate::session::run_with_compiler_stack(|| {
            with_literal_homes(|| {
                super::compile_with_injected_ir(vec![Instr::DataRef {
                    dest: Temp(777),
                    kind,
                    value: invalid.into(),
                }])
            })
        })
        .unwrap()
        .expect_err("unused invalid literal rejected by original literal inventory");
        let optimized =
            emit().expect_err("unused invalid literal remains rejected after home omission");
        assert_eq!(optimized, baseline);
        assert!(optimized.contains(invalid), "{optimized}");
    }
}

#[test]
fn canonical_dlmm_rematerialization_measures_same_compiler_artifacts_and_metadata() {
    let source = include_str!("../../../../iroha_core/src/validation_fee/fixtures/dlmm_pool.ko");
    let baseline = compile(source, true);
    let optimized = compile(source, false);
    assert_same_semantic_metadata(&baseline, &optimized);
    assert!(
        optimized.artifact.len() < baseline.artifact.len(),
        "the current canonical pool must actually shrink"
    );
    let before = ProgramMetadata::parse(&baseline.artifact).unwrap();
    let after = ProgramMetadata::parse(&optimized.artifact).unwrap();
    eprintln!(
        "canonical DLMM same-compiler literal homes: baseline_bytes={} optimized_bytes={} saved_bytes={} baseline_code_bytes={} optimized_code_bytes={} baseline_hash={} optimized_hash={}",
        baseline.artifact.len(),
        optimized.artifact.len(),
        baseline.artifact.len() - optimized.artifact.len(),
        baseline.artifact.len() - before.code_offset,
        optimized.artifact.len() - after.code_offset,
        baseline.report.artifact_hash,
        optimized.report.artifact_hash
    );
    for (before, after) in baseline
        .report
        .budget_report
        .iter()
        .zip(&optimized.report.budget_report)
    {
        assert_eq!(before.function_name, after.function_name);
        eprintln!(
            "literal-home function={} baseline_words={} optimized_words={} baseline_frame={} optimized_frame={}",
            before.function_name,
            before.bytecode_words,
            after.bytecode_words,
            before.frame_bytes,
            after.frame_bytes
        );
    }
    // This measured compiler gate does not replace actual pool-effect and
    // precision runtime qualification or the still-required default 4M gate.
}
