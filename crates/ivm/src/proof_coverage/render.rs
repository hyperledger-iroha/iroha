//! Deterministic JSON rendering of the proof-coverage inventory.
//!
//! The layout keeps one entry per line so a changed opcode, syscall or fault
//! produces a one-line diff in the tracked artifact. Object keys are sorted by
//! the Norito JSON map, and derived columns (host-state access, metering and
//! gas formula, coverage status) are read from their canonical owners at
//! render time, so the artifact goes stale whenever those owners change.

use norito::json::{Map, Value};

use super::{
    COMPLETE_RELATION, COMPONENTS, Citation, CompletionInputs, DEFAULT_SYSCALL_EXCLUSIONS,
    FAULT_CODE_COVERAGE, HOST_PRIVATE_SYSCALLS, INVENTORY_SCHEMA, INVOCATION_OBLIGATIONS,
    NUMERIC_FAULTS, NumericFaultEntry, OPCODES, OPEN_SEMANTICS, Obligation, POINTER_ABI_FAULTS,
    PRIVACY_HELPERS, PRODUCTION_ADMISSION, PointerAbiFaultEntry, RESERVED_OPCODES, RUN_PHASES,
    SYSCALL_PRIVACY_FUNCTIONS, SYSCALLS, SyscallRelation, TAG_ACCESSORS, TRAP_KINDS, TrapOrigin,
    VM_ERROR_PRODUCER_EXCLUSIONS, VM_ERROR_PRODUCER_SCOPE, VM_ERRORS, completion_blockers,
    components_for_opcode, components_for_trap, opcode_coverage, summary, trap_coverage,
    trap_kind_name,
};
use crate::{SyscallPolicy, host::host_syscall_metering_spec, syscalls};

/// Command that regenerates the tracked artifact.
const REGENERATE: &str =
    "cargo run --locked -p ivm --features dev-tools --bin gen_proof_coverage_inventory -- --write";

/// One top-level member: a scalar value or one entry per line.
enum Section {
    Scalar(Value),
    Rows(Vec<Value>),
}

fn object<const N: usize>(members: [(&str, Value); N]) -> Value {
    let mut map = Map::new();
    for (key, value) in members {
        map.insert(key.to_owned(), value);
    }
    Value::Object(map)
}

fn strings<'a>(values: impl IntoIterator<Item = &'a str>) -> Value {
    Value::Array(values.into_iter().map(Value::from).collect())
}

fn obligations(values: &[Obligation]) -> Value {
    strings(values.iter().map(|obligation| obligation.id()))
}

fn optional(value: Option<&str>) -> Value {
    value.map_or(Value::Null, Value::from)
}

fn compact(value: &Value) -> String {
    norito::json::to_json(value).expect("inventory values serialize as JSON")
}

fn summary_value() -> Value {
    let counts = summary();
    object([
        ("components", Value::from(counts.components)),
        ("numeric_faults", Value::from(counts.numeric_faults)),
        (
            "numeric_faults_complete",
            Value::from(counts.numeric_faults_complete),
        ),
        ("opcodes", Value::from(counts.opcodes)),
        ("opcodes_complete", Value::from(counts.opcodes_complete)),
        (
            "opcodes_component_only",
            Value::from(counts.opcodes_component_only),
        ),
        ("opcodes_uncovered", Value::from(counts.opcodes_uncovered)),
        ("open_semantics", Value::from(counts.open_semantics)),
        ("pointer_abi_faults", Value::from(counts.pointer_abi_faults)),
        (
            "pointer_abi_faults_complete",
            Value::from(counts.pointer_abi_faults_complete),
        ),
        ("syscalls", Value::from(counts.syscalls)),
        ("syscalls_complete", Value::from(counts.syscalls_complete)),
        ("trap_kinds", Value::from(counts.trap_kinds)),
        (
            "trap_kinds_complete",
            Value::from(counts.trap_kinds_complete),
        ),
        ("vm_errors", Value::from(counts.vm_errors)),
    ])
}

fn obligation_rows() -> Vec<Value> {
    Obligation::ALL
        .into_iter()
        .map(|obligation| {
            object([
                ("id", Value::from(obligation.id())),
                ("requirement", Value::from(obligation.requirement())),
            ])
        })
        .collect()
}

fn citations(values: &[Citation]) -> Value {
    Value::Array(
        values
            .iter()
            .map(|citation| {
                object([
                    ("path", Value::from(citation.path)),
                    ("symbol", Value::from(citation.symbol)),
                ])
            })
            .collect(),
    )
}

fn invocation_rows() -> Vec<Value> {
    INVOCATION_OBLIGATIONS
        .iter()
        .map(|entry| {
            object([
                ("citations", citations(entry.citations)),
                ("components", strings(entry.components.iter().copied())),
                ("id", Value::from(entry.id)),
                ("obligation", Value::from(entry.obligation.id())),
                ("requirement", Value::from(entry.requirement)),
                ("status", Value::from(entry.status.id())),
            ])
        })
        .collect()
}

fn component_rows() -> Vec<Value> {
    COMPONENTS
        .iter()
        .map(|entry| {
            let opcodes = entry
                .opcodes
                .iter()
                .map(|covered| {
                    let name = super::opcode_entry(covered.opcode).map_or("", |opcode| opcode.name);
                    object([
                        ("name", Value::from(name)),
                        ("restriction", optional(covered.restriction)),
                    ])
                })
                .collect();
            let geometry = entry
                .geometry
                .iter()
                .map(|fact| {
                    object([
                        ("constant", Value::from(fact.constant)),
                        ("name", Value::from(fact.name)),
                        ("path", Value::from(fact.path)),
                        ("value", Value::from(fact.value)),
                    ])
                })
                .collect();
            object([
                ("geometry", Value::Array(geometry)),
                ("id", Value::from(entry.id)),
                ("opcodes", Value::Array(opcodes)),
                (
                    "referenced_opcodes",
                    strings(entry.referenced_opcodes.iter().copied()),
                ),
                ("registered", Value::from(entry.registered)),
                ("restrictions", strings(entry.restrictions.iter().copied())),
                ("sources", strings(entry.sources.iter().copied())),
                ("substrate", obligations(entry.substrate)),
                ("summary", Value::from(entry.summary)),
                (
                    "traps",
                    strings(entry.traps.iter().map(|kind| trap_kind_name(*kind))),
                ),
            ])
        })
        .collect()
}

fn opcode_rows() -> Vec<Value> {
    OPCODES
        .iter()
        .map(|entry| {
            let components = components_for_opcode(entry.opcode)
                .map(|component| {
                    let restriction = component
                        .opcodes
                        .iter()
                        .find(|covered| covered.opcode == entry.opcode)
                        .and_then(|covered| covered.restriction);
                    object([
                        ("id", Value::from(component.id)),
                        ("restriction", optional(restriction)),
                    ])
                })
                .collect();
            object([
                ("components", Value::Array(components)),
                ("coverage", Value::from(opcode_coverage(entry.opcode).id())),
                ("direct_traps", strings(entry.direct_traps.iter().copied())),
                ("effect", Value::from(entry.effect.id())),
                ("family", Value::from(entry.family.id())),
                (
                    "fallible_helpers",
                    strings(entry.fallible_helpers.iter().copied()),
                ),
                ("hex", Value::from(format!("0x{:02X}", entry.opcode))),
                ("module", Value::from(entry.module)),
                ("name", Value::from(entry.name)),
                ("obligations", obligations(entry.obligations)),
                ("opcode", Value::from(u64::from(entry.opcode))),
                ("pc", Value::from(entry.pc.id())),
                ("tag_surface", strings(entry.tag_surface.iter().copied())),
            ])
        })
        .collect()
}

fn reserved_opcode_rows() -> Vec<Value> {
    RESERVED_OPCODES
        .iter()
        .map(|entry| {
            object([
                ("hex", Value::from(format!("0x{:02X}", entry.opcode))),
                ("name", Value::from(entry.name)),
                ("opcode", Value::from(u64::from(entry.opcode))),
                ("outcome", Value::from("InvalidOpcode")),
            ])
        })
        .collect()
}

fn relation_rows() -> Vec<Value> {
    SyscallRelation::ALL
        .into_iter()
        .map(|relation| {
            object([
                (
                    "expected_access",
                    Value::Array(
                        relation
                            .expected_access()
                            .iter()
                            .map(|access| Value::from(format!("{access:?}")))
                            .collect(),
                    ),
                ),
                ("id", Value::from(relation.id())),
                ("obligations", obligations(relation.obligations())),
            ])
        })
        .collect()
}

fn syscall_rows() -> Vec<Value> {
    SYSCALLS
        .iter()
        .map(|entry| {
            let access = syscalls::registered_syscall_access(entry.number)
                .map_or(Value::Null, |access| Value::from(format!("{access:?}")));
            let spec = host_syscall_metering_spec(SyscallPolicy::AbiV1, entry.number);
            let metering = spec.map_or(Value::Null, |spec| {
                Value::from(format!("{:?}", spec.metering))
            });
            let formula = spec.map_or(Value::Null, |spec| {
                Value::from(format!("{:?}", spec.formula))
            });
            let contract_bound = syscalls::GENERIC_PROGRAM_DENIED_SYSCALLS_V1
                .binary_search(&entry.number)
                .is_ok();
            object([
                ("access", access),
                ("contract_bound", Value::from(contract_bound)),
                ("coverage", Value::from(entry.status().id())),
                ("gas_formula", formula),
                ("hex", Value::from(format!("0x{:06X}", entry.number))),
                ("metering", metering),
                ("name", Value::from(entry.name)),
                ("number", Value::from(entry.number)),
                ("relation", Value::from(entry.relation.id())),
                ("result", Value::from(entry.result.id())),
                ("statement", Value::from(entry.statement.id())),
                ("trap", Value::from(entry.trap.id())),
            ])
        })
        .collect()
}

fn host_private_rows() -> Vec<Value> {
    HOST_PRIVATE_SYSCALLS
        .iter()
        .map(|entry| {
            object([
                ("hex", Value::from(format!("0x{:08X}", entry.number))),
                ("name", Value::from(entry.name)),
                ("number", Value::from(entry.number)),
                ("outcome", Value::from("UnknownSyscall")),
            ])
        })
        .collect()
}

fn origin_rows() -> Vec<Value> {
    TrapOrigin::ALL
        .into_iter()
        .map(|origin| {
            object([
                ("id", Value::from(origin.id())),
                ("in_invocation", origin_in_invocation(origin)),
                ("obligations", obligations(origin.obligations())),
            ])
        })
        .collect()
}

fn trap_kind_rows() -> Vec<Value> {
    TRAP_KINDS
        .iter()
        .map(|entry| {
            object([
                (
                    "components",
                    strings(components_for_trap(entry.kind).map(|component| component.id)),
                ),
                ("coverage", Value::from(trap_coverage(entry.kind).id())),
                ("name", Value::from(entry.name())),
                ("obligations", obligations(&entry.obligations())),
                (
                    "origins",
                    strings(entry.origins().iter().map(|origin| origin.id())),
                ),
                (
                    "vm_errors",
                    strings(entry.vm_errors().map(|error| error.variant)),
                ),
            ])
        })
        .collect()
}

fn vm_error_rows() -> Vec<Value> {
    VM_ERRORS
        .iter()
        .map(|entry| {
            let producers = entry
                .producers()
                .iter()
                .map(|producer| {
                    object([
                        ("origin", Value::from(producer.origin.id())),
                        ("path", Value::from(producer.path)),
                    ])
                })
                .collect();
            object([
                (
                    "origins",
                    strings(entry.origins().iter().map(|origin| origin.id())),
                ),
                ("producers", Value::Array(producers)),
                ("trap_kind", optional(entry.trap_kind.map(trap_kind_name))),
                ("variant", Value::from(entry.variant)),
            ])
        })
        .collect()
}

fn run_phase_rows() -> Vec<Value> {
    RUN_PHASES
        .iter()
        .map(|phase| {
            object([
                ("direct_traps", strings(phase.direct_traps.iter().copied())),
                (
                    "fallible_helpers",
                    strings(phase.fallible_helpers.iter().copied()),
                ),
                ("function", Value::from(phase.function)),
                ("id", Value::from(phase.id)),
                ("obligations", obligations(phase.obligations)),
                ("path", Value::from(phase.path)),
                (
                    "result_sources",
                    strings(phase.result_sources.iter().copied()),
                ),
                ("summary", Value::from(phase.summary)),
            ])
        })
        .collect()
}

fn privacy_tag_surface_value() -> Value {
    object([
        ("accessors", strings(TAG_ACCESSORS.iter().copied())),
        ("helpers", strings(PRIVACY_HELPERS.iter().copied())),
        (
            "syscall_functions",
            strings(SYSCALL_PRIVACY_FUNCTIONS.iter().copied()),
        ),
    ])
}

fn producer_scope_value() -> Value {
    object([
        (
            "exclusions",
            strings(VM_ERROR_PRODUCER_EXCLUSIONS.iter().copied()),
        ),
        ("roots", strings(VM_ERROR_PRODUCER_SCOPE.iter().copied())),
    ])
}

fn origin_in_invocation(origin: TrapOrigin) -> Value {
    Value::from(origin.in_invocation())
}

fn fault_row(tag: u64, name: &str, classes: &[Obligation]) -> Value {
    object([
        ("coverage", Value::from(FAULT_CODE_COVERAGE.id())),
        ("name", Value::from(name)),
        ("obligations", obligations(classes)),
        ("tag", Value::from(tag)),
    ])
}

fn open_semantic_rows() -> Vec<Value> {
    OPEN_SEMANTICS
        .iter()
        .map(|entry| {
            object([
                ("evidence", citations(entry.evidence)),
                ("id", Value::from(entry.id)),
                ("reason", Value::from(entry.reason)),
                ("subject", Value::from(entry.subject)),
            ])
        })
        .collect()
}

/// Render the complete inventory as deterministic JSON text.
///
/// The output ends with one newline and is byte-for-byte the content of the
/// tracked artifact at [`super::INVENTORY_ARTIFACT_PATH`].
#[must_use]
pub fn render_inventory_json() -> String {
    let numeric_faults = NUMERIC_FAULTS
        .iter()
        .map(|entry| {
            fault_row(
                entry.fault.tag(),
                entry.name,
                NumericFaultEntry::OBLIGATIONS,
            )
        })
        .collect();
    let pointer_faults = POINTER_ABI_FAULTS
        .iter()
        .map(|entry| {
            fault_row(
                entry.fault.tag(),
                entry.name,
                PointerAbiFaultEntry::OBLIGATIONS,
            )
        })
        .collect();
    let blockers = completion_blockers(&CompletionInputs::current());
    let sections = [
        ("schema", Section::Scalar(Value::from(INVENTORY_SCHEMA))),
        ("regenerate", Section::Scalar(Value::from(REGENERATE))),
        (
            "complete_relation",
            Section::Scalar(optional(COMPLETE_RELATION)),
        ),
        (
            "production_admission",
            Section::Scalar(Value::from(PRODUCTION_ADMISSION)),
        ),
        (
            "completion_open",
            Section::Scalar(Value::from(!blockers.is_empty())),
        ),
        (
            "completion_blockers",
            Section::Scalar(strings(blockers.iter().map(|blocker| blocker.id()))),
        ),
        (
            "default_syscall_exclusions",
            Section::Scalar(Value::Array(
                DEFAULT_SYSCALL_EXCLUSIONS
                    .iter()
                    .map(|number| Value::from(*number))
                    .collect(),
            )),
        ),
        ("summary", Section::Scalar(summary_value())),
        ("obligations", Section::Rows(obligation_rows())),
        ("invocation_obligations", Section::Rows(invocation_rows())),
        ("open_semantics", Section::Rows(open_semantic_rows())),
        ("components", Section::Rows(component_rows())),
        ("run_phases", Section::Rows(run_phase_rows())),
        (
            "privacy_tag_surface",
            Section::Scalar(privacy_tag_surface_value()),
        ),
        ("opcodes", Section::Rows(opcode_rows())),
        ("reserved_opcodes", Section::Rows(reserved_opcode_rows())),
        ("syscall_relations", Section::Rows(relation_rows())),
        ("syscalls", Section::Rows(syscall_rows())),
        ("host_private_syscalls", Section::Rows(host_private_rows())),
        ("trap_origins", Section::Rows(origin_rows())),
        ("trap_kinds", Section::Rows(trap_kind_rows())),
        (
            "vm_error_producer_scope",
            Section::Scalar(producer_scope_value()),
        ),
        ("vm_errors", Section::Rows(vm_error_rows())),
        ("numeric_faults", Section::Rows(numeric_faults)),
        ("pointer_abi_faults", Section::Rows(pointer_faults)),
    ];
    let mut out = String::from("{\n");
    let last = sections.len() - 1;
    for (index, (key, section)) in sections.into_iter().enumerate() {
        out.push_str("  ");
        out.push_str(&compact(&Value::from(key)));
        out.push_str(": ");
        match section {
            Section::Scalar(value) => out.push_str(&compact(&value)),
            Section::Rows(rows) if rows.is_empty() => out.push_str("[]"),
            Section::Rows(rows) => {
                out.push_str("[\n");
                let final_row = rows.len() - 1;
                for (row_index, row) in rows.iter().enumerate() {
                    out.push_str("    ");
                    out.push_str(&compact(row));
                    out.push_str(if row_index == final_row { "\n" } else { ",\n" });
                }
                out.push_str("  ]");
            }
        }
        out.push_str(if index == last { "\n" } else { ",\n" });
    }
    out.push_str("}\n");
    out
}
