//! Baseline v1 capture: frozen descriptors, artifact corpus and execution oracle of the pre-C10
//! IVM. This program is compiled inside an export of the anchor commit by
//! `scripts/ivm_baseline_v1/capture.py`; it is not part of the live workspace.
//!
//! Usage: `baseline_v1_capture --anchor-root <export> --capture-dir <this directory>
//! --anchor-commit <sha> --out <fixture directory>`.

mod anchor_core_admission;
mod asm_cases;
mod descriptors;
mod koto_cases;
mod oracle;

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use ivm::TraceMode;
use norito::json::{Map, Value};

use crate::oracle::{Case, n, obj, s, sha256_hex};

fn argument(name: &str) -> String {
    let mut arguments = std::env::args().skip(1);
    while let Some(flag) = arguments.next() {
        if flag == name {
            return arguments
                .next()
                .unwrap_or_else(|| panic!("{name} needs a value"));
        }
    }
    panic!("missing {name}");
}

fn write(path: &Path, bytes: &[u8]) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap_or_else(|error| panic!("{}: {error}", parent.display()));
    }
    fs::write(path, bytes).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
}

fn pretty(value: &Value) -> Vec<u8> {
    let mut text = norito::json::to_string_pretty(value).expect("Norito JSON document");
    text.push('\n');
    text.into_bytes()
}

fn member<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    path.iter()
        .try_fold(value, |value, key| value.as_object()?.get(*key))
}

fn bump(counts: &mut BTreeMap<String, u64>, key: impl Into<String>) {
    *counts.entry(key.into()).or_default() += 1;
}

fn counts_json(counts: &BTreeMap<String, u64>) -> Value {
    Value::Object(
        counts
            .iter()
            .map(|(key, count)| (key.clone(), n(*count)))
            .collect(),
    )
}

/// Fields of a run that a PC trace must not change.
const TRACE_INVARIANT: &[&[&str]] = &[
    &["result"],
    &["not_run"],
    &["pc"],
    &["cycles"],
    &["gas_remaining"],
    &["registers"],
    &["private_registers"],
    &["register_root"],
    &["output"],
    &["return"],
    &["syscalls"],
    &["syscall_trace"],
    &["summary", "final_memory_root"],
    &["summary", "gas_used"],
];

fn main() {
    let anchor_root = PathBuf::from(argument("--anchor-root"));
    let capture_dir = PathBuf::from(argument("--capture-dir"));
    let anchor_commit = argument("--anchor-commit");
    let out = PathBuf::from(argument("--out"));

    let frozen = descriptors::capture(&out, &anchor_root, &anchor_commit);
    write(
        &out.join("descriptors/frozen_values.json"),
        &pretty(&frozen),
    );

    let sources = koto_cases::discover(&anchor_root, &capture_dir.join("kotodama"));
    let (compiled, rejected) = koto_cases::compile(&sources);
    let mut cases: Vec<Case> = asm_cases::build();
    cases.extend(koto_cases::build(
        &compiled,
        &capture_dir.join("kotodama/runs.json"),
    ));

    let mut ids = BTreeSet::new();
    let mut artifacts = BTreeMap::<String, Vec<u8>>::new();
    for case in &cases {
        assert!(ids.insert(case.id.clone()), "duplicate case id {}", case.id);
        let known = artifacts
            .entry(case.artifact.clone())
            .or_insert_with(|| case.program.clone());
        assert_eq!(
            *known, case.program,
            "{} disagrees about the bytes of {}",
            case.id, case.artifact
        );
    }
    for (path, bytes) in &artifacts {
        write(&out.join(path), bytes);
    }
    let compiled_ids: BTreeSet<&str> = compiled.iter().map(|entry| entry.id.as_str()).collect();
    let rejections: BTreeMap<&str, &str> = rejected
        .iter()
        .map(|entry| (entry.id.as_str(), entry.error.as_str()))
        .collect();
    let mut source_index = Vec::new();
    for source in &sources {
        let path = format!("sources/kotodama/{}.ko.txt", source.id);
        write(&out.join(&path), source.text.as_bytes());
        let mut entry = Map::new();
        entry.insert("id".to_owned(), s(source.id.clone()));
        entry.insert("origin".to_owned(), s(source.origin.clone()));
        entry.insert("source".to_owned(), s(path));
        entry.insert(
            "source_sha256".to_owned(),
            s(sha256_hex(source.text.as_bytes())),
        );
        if compiled_ids.contains(source.id.as_str()) {
            let artifact = format!("artifacts/kotodama/{}.to.bin", source.id);
            entry.insert(
                "artifact_sha256".to_owned(),
                s(sha256_hex(&artifacts[&artifact])),
            );
            entry.insert("artifact".to_owned(), s(artifact));
        } else {
            entry.insert(
                "compile_error".to_owned(),
                s(rejections[source.id.as_str()]),
            );
        }
        // Sources also compiled with the ZK mode bit forced.
        let zk_id = format!("zk/{}", source.id);
        let zk_artifact = format!("artifacts/kotodama/{zk_id}.to.bin");
        if let Some(bytes) = artifacts.get(&zk_artifact) {
            entry.insert("zk_artifact_sha256".to_owned(), s(sha256_hex(bytes)));
            entry.insert("zk_artifact".to_owned(), s(zk_artifact));
        } else if let Some(error) = rejections.get(zk_id.as_str()) {
            entry.insert("zk_compile_error".to_owned(), s(*error));
        }
        source_index.push(Value::Object(entry));
    }

    let mut lines = String::new();
    let mut executed = BTreeMap::<u8, u64>::new();
    let mut by_kind = BTreeMap::<String, u64>::new();
    let mut by_host = BTreeMap::<String, u64>::new();
    let mut vm_admission = BTreeMap::<String, u64>::new();
    let mut core_admission = BTreeMap::<String, u64>::new();
    let mut results = BTreeMap::<String, u64>::new();
    let mut fault_kinds = BTreeMap::<String, u64>::new();
    let mut syscalls = BTreeMap::<String, u64>::new();
    let mut covers = BTreeMap::<String, u64>::new();
    let mut problems = Vec::new();
    for case in &cases {
        let first = oracle::run_case(case, TraceMode::Off);
        let second = oracle::run_case(case, TraceMode::Off);
        if first.run != second.run {
            problems.push(format!("{}: two identical runs disagree", case.id));
        }
        if first.panicked {
            problems.push(format!("{}: the anchor panicked", case.id));
        }
        let traced = if case.traced {
            oracle::run_case(case, TraceMode::PcOnly)
        } else {
            oracle::Outcome {
                run: Value::Null,
                trace: None,
                opcodes: BTreeMap::new(),
                panicked: false,
            }
        };
        let diverged: Vec<Value> = TRACE_INVARIANT
            .iter()
            .filter(|path| case.traced && member(&first.run, path) != member(&traced.run, path))
            .map(|path| s(path.join(".")))
            .collect();
        if !diverged.is_empty() {
            problems.push(format!("{}: PC tracing changed {diverged:?}", case.id));
        }
        for (opcode, count) in &traced.opcodes {
            *executed.entry(*opcode).or_default() += count;
        }
        let setup = oracle::setup_json(case);
        let admission = oracle::admission_json(case);
        bump(
            &mut by_kind,
            case.id.split('/').next().unwrap_or_default().to_owned(),
        );
        bump(
            &mut by_host,
            member(&setup, &["host"])
                .and_then(Value::as_str)
                .unwrap_or_default(),
        );
        bump(
            &mut vm_admission,
            member(&admission, &["vm", "error", "debug"])
                .and_then(Value::as_str)
                .map_or("Ok", |debug| {
                    debug
                        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                        .next()
                        .unwrap_or_default()
                }),
        );
        bump(
            &mut core_admission,
            member(&admission, &["core", "error", "variant"])
                .and_then(Value::as_str)
                .unwrap_or("Ok"),
        );
        let result = if let Some(stage) = member(&first.run, &["not_run"]).and_then(Value::as_str) {
            format!("not run ({})", stage.split(' ').next().unwrap_or_default())
        } else if member(&first.run, &["panic"]).is_some() {
            "panic".to_owned()
        } else if member(&first.run, &["result", "ok"]).and_then(Value::as_bool) == Some(true) {
            "Ok".to_owned()
        } else {
            member(&first.run, &["result", "unmetered_debug"])
                .and_then(Value::as_str)
                .unwrap_or_default()
                .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                .next()
                .unwrap_or_default()
                .to_owned()
        };
        bump(&mut results, result);
        if let Some(kind) = member(&first.run, &["result", "fault_kind"])
            .or_else(|| member(&first.run, &["fault_kind"]))
            .and_then(Value::as_object)
        {
            let variant = kind
                .get("variant")
                .and_then(Value::as_str)
                .unwrap_or_default();
            // `Numeric` and `PointerAbi` carry a nested fault code.
            let detail = kind
                .get("json")
                .and_then(|json| member(json, &["value", "kind"]))
                .and_then(Value::as_str);
            bump(
                &mut fault_kinds,
                detail.map_or_else(
                    || variant.to_owned(),
                    |detail| format!("{variant}({detail})"),
                ),
            );
        }
        if let Some(calls) = member(&first.run, &["syscalls"]).and_then(Value::as_array) {
            for call in calls {
                if member(call, &["event"]).and_then(Value::as_str) == Some("call") {
                    bump(
                        &mut syscalls,
                        member(call, &["name"]).and_then(Value::as_str).map_or_else(
                            || {
                                format!(
                                    "#{}",
                                    member(call, &["number"])
                                        .and_then(Value::as_u64)
                                        .unwrap_or_default()
                                )
                            },
                            str::to_owned,
                        ),
                    );
                }
            }
        }
        for tag in &case.covers {
            bump(&mut covers, tag.clone());
        }
        let mut record = Map::new();
        record.insert("id".to_owned(), s(case.id.clone()));
        record.insert("artifact".to_owned(), s(case.artifact.clone()));
        record.insert("artifact_sha256".to_owned(), s(sha256_hex(&case.program)));
        record.insert(
            "covers".to_owned(),
            Value::Array(case.covers.iter().cloned().map(s).collect()),
        );
        record.insert("setup".to_owned(), setup);
        record.insert("admission".to_owned(), admission);
        record.insert("run".to_owned(), first.run);
        record.insert("trace".to_owned(), traced.trace.unwrap_or(Value::Null));
        if !case.traced {
            record.insert(
                "trace_note".to_owned(),
                s("not traced: the program observes its own trace logs"),
            );
        }
        if !diverged.is_empty() {
            record.insert("trace_divergence".to_owned(), Value::Array(diverged));
        }
        lines.push_str(
            &norito::json::to_string(&Value::Object(record)).expect("oracle record JSON"),
        );
        lines.push('\n');
    }
    write(&out.join("oracle/oracle.jsonl"), lines.as_bytes());

    let executed_json: Vec<Value> = descriptors::OPCODE_NAMES
        .iter()
        .filter(|(opcode, _, _)| ivm::instruction::wide::is_valid_opcode(*opcode))
        .map(|(opcode, name, family)| {
            obj([
                ("opcode", s(format!("0x{opcode:02x}"))),
                ("name", s(*name)),
                ("family", s(*family)),
                ("executed", n(executed.get(opcode).copied().unwrap_or(0))),
            ])
        })
        .collect();
    let never: Vec<Value> = descriptors::OPCODE_NAMES
        .iter()
        .filter(|(opcode, _, _)| {
            ivm::instruction::wide::is_valid_opcode(*opcode) && !executed.contains_key(opcode)
        })
        .map(|(_, name, _)| s(*name))
        .collect();
    let corpus = obj([
        ("anchor_commit", s(anchor_commit)),
        (
            "vm_construction",
            s("IVM::deterministic_builder(gas_limit).suppress_startup_banner().build()"),
        ),
        (
            "run_recipe",
            s(
                "build the VM; load_program(artifact) (wsv_contract: install_contract_fixture + load_prepared); for each prelude entrypoint: select_entrypoint, run_with_host, reset, set_gas_limit; apply setup.steps in order; run_with_host(recording host), or run_with_host_and_cycle_budget with one VmCycleBudget shared by the prelude and the run when setup.cycle_budget is set",
            ),
        ),
        (
            "capture_host",
            obj([
                ("os", s(std::env::consts::OS)),
                ("arch", s(std::env::consts::ARCH)),
            ]),
        ),
        ("cases", n(cases.len() as u64)),
        ("artifacts", n(artifacts.len() as u64)),
        ("cases_by_kind", counts_json(&by_kind)),
        ("cases_by_host", counts_json(&by_host)),
        ("vm_admission", counts_json(&vm_admission)),
        ("core_admission", counts_json(&core_admission)),
        ("run_results", counts_json(&results)),
        ("fault_kinds", counts_json(&fault_kinds)),
        ("syscalls_dispatched", counts_json(&syscalls)),
        ("cover_tags", counts_json(&covers)),
        ("executed_opcodes", Value::Array(executed_json)),
        ("valid_opcodes_never_executed", Value::Array(never)),
        ("kotodama_sources", Value::Array(source_index)),
        (
            "problems",
            Value::Array(problems.iter().cloned().map(s).collect()),
        ),
    ]);
    write(&out.join("oracle/corpus.json"), &pretty(&corpus));

    println!(
        "captured {} cases over {} artifacts ({} Kotodama sources, {} rejected by the compiler)",
        cases.len(),
        artifacts.len(),
        sources.len(),
        rejected.len()
    );
    if !problems.is_empty() {
        for problem in &problems {
            eprintln!("problem: {problem}");
        }
        std::process::exit(2);
    }
}
