//! Test-only attribution of exact instruction-emission byte ranges.
//!
//! This observer does not change IR, lowering decisions, fixups, or metadata.
//! It attributes split reloads to their consuming IR instruction; prologues,
//! terminators, and any relaxation bytes remain an explicit unassigned remainder.

use std::{cell::RefCell, collections::BTreeMap};

use crate::ir::Instr;

#[derive(Default)]
struct Counts {
    instructions: usize,
    bytes: usize,
}
type Report = BTreeMap<(String, String), Counts>;
thread_local! {
    static REPORT: RefCell<Option<Report>> = const { RefCell::new(None) };
}

pub(super) struct Observation {
    function: String,
    instruction: String,
    start: usize,
}

pub(super) fn advance(
    previous: &mut Option<Observation>,
    function: &str,
    instruction: &Instr,
    current: usize,
) {
    finish(previous.take(), current);
    if REPORT.with(|report| report.borrow().is_some()) {
        // The complete debug value is never emitted; only the enum variant is
        // retained. Profiling is enabled solely around the public DLMM fixture.
        let name = format!("{instruction:?}")
            .split([' ', '{', '('])
            .next()
            .unwrap()
            .to_owned();
        *previous = Some(Observation {
            function: function.into(),
            instruction: name,
            start: current,
        });
    }
}

pub(super) fn finish(observation: Option<Observation>, current: usize) {
    let Some(observation) = observation else {
        return;
    };
    REPORT.with(|report| {
        let mut report = report.borrow_mut();
        let report = report.as_mut().expect("active scoped emission observation");
        let count = report
            .entry((observation.function, observation.instruction))
            .or_default();
        count.instructions += 1;
        count.bytes += current.checked_sub(observation.start).unwrap();
    });
}

fn observe<T>(body: impl FnOnce() -> T) -> (T, Report) {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            REPORT.with(|report| {
                report.borrow_mut().take();
            });
        }
    }
    REPORT.with(|report| {
        let mut report = report.borrow_mut();
        assert!(
            report.is_none(),
            "one emission observer per compiler worker"
        );
        *report = Some(Report::new());
    });
    let reset = Reset;
    let value = body();
    let report = REPORT.with(|report| report.borrow_mut().take().unwrap());
    drop(reset);
    (value, report)
}

#[test]
fn canonical_dlmm_lowering_attribution_preserves_exact_artifact_and_reports_complete_costs() {
    use crate::{
        compiler::CompilerOptions,
        metadata::ProgramMetadata,
        session::{CompileRequest, CompilerSession},
    };
    let source = include_str!("../../../iroha_core/src/validation_fee/fixtures/dlmm_pool.ko");
    let (plain, profiled, report) = crate::session::run_with_compiler_stack(|| {
        let compile = || {
            CompilerSession::new(CompilerOptions::default())
                .build(CompileRequest {
                    source,
                    source_name: Some("dlmm_pool.ko"),
                })
                .expect("canonical validated compiler pipeline")
        };
        let plain = compile();
        let (profiled, report) = observe(compile);
        (plain, profiled, report)
    })
    .unwrap();
    assert_eq!(plain.artifact, profiled.artifact);
    assert_eq!(plain.manifest, profiled.manifest);
    assert_eq!(plain.contract_interface, profiled.contract_interface);
    let parsed = ProgramMetadata::parse(&plain.artifact).unwrap();
    let code_bytes = plain.artifact.len() - parsed.code_offset;
    // Measure the same compiled artifact; standalone field encodings below are
    // diagnostic attribution only and are not additive section byte counts.
    use norito::Encode as _;
    let interface = parsed.contract_interface.as_ref().expect("production CNTR");
    let contract_bytes = interface.encode_section();
    assert_eq!(
        &plain.artifact[parsed.header_len..parsed.header_len + contract_bytes.len()],
        contract_bytes.as_slice()
    );
    assert!(
        parsed.contract_debug.is_none(),
        "default production has no DBG1"
    );
    let literal = parsed.literal_section.expect("canonical DLMM literals");
    assert_eq!(literal.start, parsed.header_len + contract_bytes.len());
    assert_eq!(literal.code_offset, parsed.code_offset);
    let literal_header_bytes = literal.entries_start - literal.start;
    let literal_index_bytes = literal.data_start - literal.entries_start;
    let literal_data_bytes = literal.data_end - literal.data_start;
    let literal_padding_bytes = literal.code_offset - literal.data_end;
    assert_eq!(literal_index_bytes, literal.count * 8);
    assert_eq!(
        parsed.header_len
            + contract_bytes.len()
            + literal_header_bytes
            + literal_index_bytes
            + literal_data_bytes
            + literal_padding_bytes
            + code_bytes,
        plain.artifact.len()
    );
    eprintln!(
        "DLMM artifact sections header_bytes={} cntr_bytes={} literal_count={} literal_header_bytes={literal_header_bytes} literal_index_bytes={literal_index_bytes} literal_data_bytes={literal_data_bytes} literal_padding_bytes={literal_padding_bytes} code_offset={} code_bytes={code_bytes}",
        parsed.header_len,
        contract_bytes.len(),
        literal.count,
        parsed.code_offset
    );
    macro_rules! field_cost {
        ($($field:ident),+ $(,)?) => {$(
            eprintln!("DLMM cntr field={} standalone_payload_bytes={}", stringify!($field), interface.$field.encode().len());
        )+};
    }
    field_cost!(
        seiyaku_name,
        compiler_fingerprint,
        abi_hash,
        features_bitmap,
        access_set_hints,
        kotoba,
        entrypoints,
        callables,
        states,
        error_types,
        error_messages,
    );
    for entrypoint in &interface.entrypoints {
        eprintln!(
            "DLMM cntr entrypoint={} standalone_payload_bytes={} read_keys={} read_keys_payload_bytes={} write_keys={} write_keys_payload_bytes={} argument_schema_payload_bytes={} return_schema_payload_bytes={}",
            entrypoint.name,
            entrypoint.encode().len(),
            entrypoint.read_keys.len(),
            entrypoint.read_keys.encode().len(),
            entrypoint.write_keys.len(),
            entrypoint.write_keys.encode().len(),
            entrypoint.argument_schema.encode().len(),
            entrypoint.return_schema.encode().len(),
        );
    }
    let directory = ivm_abi::metadata::LiteralDirectory::validate(
        &plain.artifact,
        parsed.header_len,
        parsed.literal_section,
        ivm_abi::SyscallPolicy::AbiV1,
    )
    .expect("same canonical artifact literal directory");
    let mut literal_types = BTreeMap::<u16, Counts>::new();
    let mut unique_payloads = std::collections::BTreeSet::new();
    let mut duplicate_payload_bytes = 0;
    let mut duplicate_count = 0;
    for literal in directory.iter() {
        let (kind, payload, bytes) = match literal {
            ivm_abi::metadata::ValidatedLiteral::Pointer {
                type_id, payload, ..
            } => (type_id as u16, payload.to_vec(), 39 + payload.len()),
            ivm_abi::metadata::ValidatedLiteral::I64(bits) => (0, bits.to_le_bytes().to_vec(), 8),
        };
        let count = literal_types.entry(kind).or_default();
        count.instructions += 1;
        count.bytes += bytes;
        if !unique_payloads.insert((kind, payload)) {
            duplicate_count += 1;
            duplicate_payload_bytes += bytes;
        }
    }
    assert_eq!(directory.len(), literal.count);
    assert_eq!(
        literal_types
            .values()
            .map(|count| count.bytes)
            .sum::<usize>(),
        literal_data_bytes
    );
    for (kind, count) in literal_types {
        eprintln!(
            "DLMM literal type_id={kind} count={} data_bytes={}",
            count.instructions, count.bytes
        );
    }
    eprintln!(
        "DLMM literal duplicate_count={duplicate_count} duplicate_data_bytes={duplicate_payload_bytes}"
    );
    let assigned_bytes = report.values().map(|count| count.bytes).sum::<usize>();
    assert!(!report.is_empty());
    assert!(
        assigned_bytes < code_bytes,
        "prologue/terminator costs remain explicit"
    );
    let mut totals = BTreeMap::<String, Counts>::new();
    for ((function, instruction), count) in report {
        eprintln!(
            "DLMM emission function={function} instruction={instruction} count={} bytes={}",
            count.instructions, count.bytes
        );
        let total = totals.entry(instruction).or_default();
        total.instructions += count.instructions;
        total.bytes += count.bytes;
    }
    for (instruction, count) in totals {
        eprintln!(
            "DLMM emission total instruction={instruction} count={} bytes={}",
            count.instructions, count.bytes
        );
    }
    eprintln!(
        "DLMM emission artifact_bytes={} code_bytes={code_bytes} assigned_bytes={assigned_bytes} prologue_terminator_relaxation_bytes={} artifact_hash={}",
        plain.artifact.len(),
        code_bytes - assigned_bytes,
        plain.report.artifact_hash
    );
}

#[test]
fn emission_observer_retires_on_unwind_without_affecting_following_compilation() {
    let failed = std::panic::catch_unwind(|| observe(|| panic!("diagnostic observer unwind")));
    assert!(failed.is_err());
    let ((), report) = observe(|| ());
    assert!(report.is_empty());
}
