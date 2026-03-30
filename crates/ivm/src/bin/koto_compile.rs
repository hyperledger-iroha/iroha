//! Kotodama -> IVM `.to` compiler CLI
//!
//! Minimal CLI for compiling Kotodama `.ko` sources into IVM bytecode with an
//! encoded header. It accepts an input file and optional output path. You can
//! also override header defaults via flags; seiyaku-level `meta {}` inside the
//! source takes precedence when present.
//!
//! Usage:
//!   koto_compile <input.ko> [--out <output.to>] [--manifest-out <manifest.json>] [--abi <u8>] [--vl <u8>]
//!                 [--max-cycles <u64>] [--iter-cap <u8>] [--force-zk] [--force-vector]

use std::{
    env, fs,
    path::{Component, Path, PathBuf},
};

use ivm::kotodama::compiler::{AccessHintDiagnostics, CompileReport};
use ivm::{KotodamaCompiler, ProgramMetadata, kotodama};
use norito::json::{self, Value};

const EXPLAIN_ENTRIES: &[(&str, &str)] = &[
    (
        "E0001",
        "Branch patching produced an offset outside the IVM jump range. Split very large functions or reduce inlining so basic blocks stay within ±1 MiB.",
    ),
    (
        "E0002",
        "The compiler generated a call to a function name that was never defined. Check for typos, visibility modifiers, or conditional compilation that removed the callee.",
    ),
    (
        "E0003",
        "Durable state syscalls require ABI v1. Set `CompilerOptions::abi_version = 1` or add `meta { abi_version: 1 }` inside your `seiyaku` contract.",
    ),
    (
        "E0004",
        "Asset operations expect pointer literals. Use helpers such as `account_id(...)` and `asset_definition(...)`.",
    ),
    (
        "E0005",
        "Kotodama for-loops currently accept only simple `let` or expression initialisers. Move complex setup code before the loop.",
    ),
    (
        "E0006",
        "Kotodama for-loops currently accept only simple `let` or expression step statements. Update the loop variable with an expression (for example `i = i + 1`).",
    ),
];

fn print_explain_and_exit(code: &str) -> ! {
    if let Some((_, message)) = EXPLAIN_ENTRIES
        .iter()
        .find(|(known, _)| known.eq_ignore_ascii_case(code))
    {
        println!("{}: {}", code.to_uppercase(), message);
        std::process::exit(0);
    } else {
        eprintln!("No explanation found for code {code}");
        std::process::exit(1);
    }
}

fn run_lint_pass(src: &str) -> Result<Vec<kotodama::lint::LintWarning>, String> {
    let program = kotodama::parser::parse(src).map_err(|e| format!("lint parse error: {e}"))?;
    Ok(kotodama::lint::lint_program(&program))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DiagnosticFormat {
    Text,
    Json,
}

fn json_entry<K: Into<String>>(key: K, value: Value) -> (String, Value) {
    (key.into(), value)
}

fn json_object(entries: Vec<(String, Value)>) -> Value {
    json::object(entries).unwrap_or(Value::Null)
}

fn serialize_access_hint_diagnostics(diag: &AccessHintDiagnostics) -> Value {
    json_object(vec![
        json_entry("state_wildcards", Value::from(diag.state_wildcards as u64)),
        json_entry("isi_wildcards", Value::from(diag.isi_wildcards as u64)),
    ])
}

fn serialize_source_map(report: &CompileReport) -> Value {
    Value::Array(
        report
            .source_map
            .iter()
            .map(|entry| {
                json_object(vec![
                    json_entry("function_name", Value::from(entry.function_name.clone())),
                    json_entry("pc_start", Value::from(entry.pc_start)),
                    json_entry("pc_end", Value::from(entry.pc_end)),
                    json_entry(
                        "source_path",
                        entry
                            .source
                            .source_path
                            .clone()
                            .map_or(Value::Null, Value::from),
                    ),
                    json_entry("line", Value::from(entry.source.line as u64)),
                    json_entry("column", Value::from(entry.source.column as u64)),
                ])
            })
            .collect(),
    )
}

fn serialize_budget_report(report: &CompileReport) -> Value {
    Value::Array(
        report
            .budget_report
            .iter()
            .map(|entry| {
                json_object(vec![
                    json_entry("function_name", Value::from(entry.function_name.clone())),
                    json_entry("pc_start", Value::from(entry.pc_start)),
                    json_entry("pc_end", Value::from(entry.pc_end)),
                    json_entry("bytecode_bytes", Value::from(entry.bytecode_bytes as u64)),
                    json_entry("bytecode_words", Value::from(entry.bytecode_words as u64)),
                    json_entry("frame_bytes", Value::from(entry.frame_bytes as u64)),
                    json_entry("jump_span_words", Value::from(entry.jump_span_words as u64)),
                    json_entry("jump_range_risk", Value::from(entry.jump_range_risk)),
                    json_entry(
                        "source_path",
                        entry
                            .source
                            .as_ref()
                            .and_then(|source| source.source_path.clone())
                            .map_or(Value::Null, Value::from),
                    ),
                    json_entry(
                        "line",
                        entry
                            .source
                            .as_ref()
                            .map_or(Value::Null, |source| Value::from(source.line as u64)),
                    ),
                    json_entry(
                        "column",
                        entry
                            .source
                            .as_ref()
                            .map_or(Value::Null, |source| Value::from(source.column as u64)),
                    ),
                ])
            })
            .collect(),
    )
}

fn write_report_output(path: &str, value: &Value) -> Result<(), String> {
    let rendered = json::to_string_pretty(value).map_err(|e| format!("report serialize: {e}"))?;
    if path == "-" {
        println!("{rendered}");
    } else {
        fs::write(path, &rendered).map_err(|e| format!("failed to write {path}: {e}"))?;
    }
    Ok(())
}

fn normalize_debug_source_name(input: &str) -> Option<String> {
    let path = Path::new(input);
    if path.as_os_str().is_empty() {
        return None;
    }
    let mut cleaned = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !cleaned.pop() {
                    cleaned.push(component.as_os_str());
                }
            }
            other => cleaned.push(other.as_os_str()),
        }
    }
    let rendered = if cleaned.as_os_str().is_empty() {
        path.to_path_buf()
    } else {
        cleaned
    };
    Some(rendered.display().to_string())
}

fn print_access_hint_diagnostics(diag: &AccessHintDiagnostics, format: DiagnosticFormat) {
    if diag.is_empty() {
        return;
    }
    if format == DiagnosticFormat::Json {
        let rendered = json::to_string_pretty(&serialize_access_hint_diagnostics(diag))
            .unwrap_or_else(|_| "{}".to_owned());
        eprintln!("{rendered}");
        return;
    }
    if diag.state_wildcards > 0 {
        eprintln!(
            "access-hint: {} state accesses used dynamic keys; entrypoint hints may be conservative",
            diag.state_wildcards
        );
    }
    if diag.isi_wildcards > 0 {
        eprintln!(
            "access-hint: {} ISI instructions used opaque or non-literal targets; entrypoint hints may be conservative",
            diag.isi_wildcards
        );
    }
}

fn manifest_entrypoints_are_globally_wildcarded(
    manifest: &iroha_data_model::smart_contract::manifest::ContractManifest,
) -> bool {
    manifest.entrypoints.as_ref().is_some_and(|entrypoints| {
        !entrypoints.is_empty()
            && entrypoints.iter().all(|entrypoint| {
                entrypoint.read_keys.iter().any(|key| key == "*")
                    && entrypoint.write_keys.iter().any(|key| key == "*")
            })
    })
}

/// Serialize a manifest to pretty JSON using Norito’s JSON helpers.
/// The JSON is inspection-only; deploy flows derive manifests directly from the artifact.
fn manifest_to_json(
    manifest: &iroha_data_model::smart_contract::manifest::ContractManifest,
) -> Result<String, String> {
    norito::json::to_json_pretty(manifest).map_err(|e| format!("manifest json serialize: {e}"))
}

fn print_usage() {
    eprintln!("Usage:");
    eprintln!("  koto_compile <input.ko> [--out <output.to>] [--manifest-out <manifest.json>] \\");
    eprintln!(
        "               [--abi <u8>] [--vl <u8>] [--max-cycles <u64>] [--iter-cap <u8>] [--force-zk] [--force-vector] [--emit-source-map <path>] [--emit-budget-report <path>] [--diagnostic-format <text|json>] [--strip-debug] [--no-lint] [--deny-lint-warnings]"
    );
    eprintln!("  koto_compile --explain <code>");
    eprintln!("\nNotes:");
    eprintln!("  - First release supports only ABI 1. Use --abi 1.");
    eprintln!("  - Use '--manifest-out -' to print the inspection manifest JSON to stdout.");
    eprintln!(
        "  - Lint runs by default; pass --no-lint to skip or --deny-lint-warnings to fail on lint output."
    );
}

fn validate_abi_version(version: u8) -> Result<(), String> {
    if version == 1 {
        return Ok(());
    }
    Err(format!(
        "error: --abi={version} is not supported in the first release (expected 1)."
    ))
}

fn main() {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() {
        print_usage();
        std::process::exit(2);
    }

    let mut input: Option<String> = None;
    let mut out: Option<String> = None;
    let mut abi_version: Option<u8> = None;
    let mut manifest_out: Option<String> = None;
    let mut vector_length: Option<u8> = None;
    let mut max_cycles: Option<u64> = None;
    let mut iter_cap: Option<u8> = None;
    let mut force_zk = false;
    let mut force_vector = false;
    let mut lint_enabled = true;
    let mut lint_deny_warnings = false;
    let mut diagnostic_format = DiagnosticFormat::Text;
    let mut emit_source_map: Option<String> = None;
    let mut emit_budget_report: Option<String> = None;
    let mut strip_debug = false;
    let mut explain: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                if i + 1 >= args.len() {
                    eprintln!("--out expects a path");
                    std::process::exit(2);
                }
                out = Some(args[i + 1].clone());
                i += 2;
            }
            "--abi" => {
                if i + 1 >= args.len() {
                    eprintln!("--abi expects a value");
                    std::process::exit(2);
                }
                abi_version = args[i + 1].parse::<u8>().ok();
                i += 2;
            }
            "--manifest-out" => {
                if i + 1 >= args.len() {
                    eprintln!("--manifest-out expects a path");
                    std::process::exit(2);
                }
                manifest_out = Some(args[i + 1].clone());
                i += 2;
            }
            "--vl" => {
                if i + 1 >= args.len() {
                    eprintln!("--vl expects a value");
                    std::process::exit(2);
                }
                vector_length = args[i + 1].parse::<u8>().ok();
                i += 2;
            }
            "--max-cycles" => {
                if i + 1 >= args.len() {
                    eprintln!("--max-cycles expects a value");
                    std::process::exit(2);
                }
                max_cycles = args[i + 1].parse::<u64>().ok();
                i += 2;
            }
            "--iter-cap" => {
                if i + 1 >= args.len() {
                    eprintln!("--iter-cap expects a value");
                    std::process::exit(2);
                }
                iter_cap = args[i + 1].parse::<u8>().ok();
                i += 2;
            }
            "--explain" => {
                if i + 1 >= args.len() {
                    eprintln!("--explain expects an error code");
                    std::process::exit(2);
                }
                explain = Some(args[i + 1].clone());
                i += 2;
            }
            "--emit-source-map" => {
                if i + 1 >= args.len() {
                    eprintln!("--emit-source-map expects a path");
                    std::process::exit(2);
                }
                emit_source_map = Some(args[i + 1].clone());
                i += 2;
            }
            "--emit-budget-report" => {
                if i + 1 >= args.len() {
                    eprintln!("--emit-budget-report expects a path");
                    std::process::exit(2);
                }
                emit_budget_report = Some(args[i + 1].clone());
                i += 2;
            }
            "--diagnostic-format" => {
                if i + 1 >= args.len() {
                    eprintln!("--diagnostic-format expects one of: text, json");
                    std::process::exit(2);
                }
                diagnostic_format = match args[i + 1].as_str() {
                    "text" => DiagnosticFormat::Text,
                    "json" => DiagnosticFormat::Json,
                    other => {
                        eprintln!("unsupported diagnostic format: {other}");
                        std::process::exit(2);
                    }
                };
                i += 2;
            }
            "--strip-debug" => {
                strip_debug = true;
                i += 1;
            }
            "--force-zk" => {
                force_zk = true;
                i += 1;
            }
            "--force-vector" => {
                force_vector = true;
                i += 1;
            }
            "--lint" => {
                lint_enabled = true;
                i += 1;
            }
            "--no-lint" => {
                lint_enabled = false;
                i += 1;
            }
            "--deny-lint-warnings" => {
                lint_enabled = true;
                lint_deny_warnings = true;
                i += 1;
            }
            "--lint-format" => {
                if i + 1 >= args.len() {
                    eprintln!("--lint-format expects one of: human, json");
                    std::process::exit(2);
                }
                let fmt = args[i + 1].as_str();
                if fmt != "human" {
                    eprintln!("--lint-format currently supports only 'human' output");
                    std::process::exit(2);
                }
                i += 2;
            }
            other if other.starts_with('-') => {
                eprintln!("Unknown flag: {other}");
                print_usage();
                std::process::exit(2);
            }
            path => {
                if let Some(existing) = input.as_deref() {
                    eprintln!(
                        "Multiple input files specified: '{}' and '{}'",
                        existing, path
                    );
                    std::process::exit(2);
                }
                input = Some(path.to_string());
                i += 1;
            }
        }
    }

    if let Some(code) = explain {
        print_explain_and_exit(&code);
    }

    let input_path = match input {
        Some(p) => p,
        None => {
            print_usage();
            std::process::exit(2);
        }
    };

    // Build compiler options; contract meta in source will override these where set.
    let mut opts = ivm::kotodama::compiler::CompilerOptions::default();
    if let Some(v) = abi_version {
        if let Err(err) = validate_abi_version(v) {
            eprintln!("{err}");
            std::process::exit(2);
        }
        opts.abi_version = v;
    }
    if let Some(vl) = vector_length {
        opts.vector_length = vl;
    }
    if let Some(mc) = max_cycles {
        opts.max_cycles = mc;
    }
    if let Some(ic) = iter_cap {
        opts.dynamic_iter_cap = ic;
    }
    opts.force_zk = force_zk;
    opts.force_vector = force_vector;
    opts.emit_debug = !strip_debug;
    opts.debug_source_name = normalize_debug_source_name(&input_path);

    let source = match std::fs::read_to_string(&input_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("read {}: {}", &input_path, e);
            std::process::exit(1);
        }
    };
    if lint_enabled {
        let lang = kotodama::i18n::detect_language();
        match run_lint_pass(&source) {
            Ok(warnings) => {
                if !warnings.is_empty() {
                    for w in &warnings {
                        eprintln!("lint: {}", w.localized_message(lang));
                    }
                    if lint_deny_warnings {
                        eprintln!("lint: warnings treated as errors (--deny-lint-warnings)");
                        std::process::exit(1);
                    }
                }
            }
            Err(err) => {
                eprintln!("{err}");
                if lint_deny_warnings {
                    std::process::exit(1);
                }
            }
        }
    }
    let compiler = KotodamaCompiler::new_with_options(opts);
    let (code, manifest, report) = match compiler.compile_source_with_manifest_and_report(&source) {
        Ok(result) => result,
        Err(e) => {
            eprintln!("compile error: {e}");
            std::process::exit(1);
        }
    };
    if !manifest_entrypoints_are_globally_wildcarded(&manifest) {
        print_access_hint_diagnostics(&report.access_hint_diagnostics, diagnostic_format);
    }
    let manifest_opt = manifest_out.is_some().then_some(manifest);

    // Pick output path
    let output_path = match out {
        Some(p) => PathBuf::from(p),
        None => {
            let mut p = PathBuf::from(&input_path);
            p.set_extension("to");
            p
        }
    };

    if let Err(e) = fs::write(&output_path, &code) {
        eprintln!("failed to write {}: {e}", output_path.display());
        std::process::exit(1);
    }

    // Optionally write manifest JSON (or print to stdout when path is '-')
    if let (Some(path), Some(mf)) = (manifest_out, manifest_opt.as_ref()) {
        match manifest_to_json(mf) {
            Ok(json) => {
                if path == "-" {
                    println!("{json}");
                } else {
                    if let Err(e) = fs::write(&path, &json) {
                        eprintln!("failed to write {path}: {e}");
                        std::process::exit(1);
                    }
                    println!("Wrote manifest {path}");
                }
            }
            Err(e) => {
                eprintln!("manifest serialize error: {e}");
                std::process::exit(1);
            }
        }
    }

    if let Some(path) = emit_source_map.as_deref()
        && let Err(err) = write_report_output(path, &serialize_source_map(&report))
    {
        eprintln!("{err}");
        std::process::exit(1);
    }
    if let Some(path) = emit_budget_report.as_deref()
        && let Err(err) = write_report_output(path, &serialize_budget_report(&report))
    {
        eprintln!("{err}");
        std::process::exit(1);
    }

    // Print a short header summary to stdout
    if let Ok(parsed) = ProgramMetadata::parse(&code) {
        let meta = parsed.metadata;
        println!(
            "Wrote {} (abi={}, mode=0x{:02x}, vl={}, max_cycles={}, debug={})",
            output_path.display(),
            meta.abi_version,
            meta.mode,
            meta.vector_length,
            meta.max_cycles,
            parsed.contract_debug.is_some()
        );
    } else {
        println!("Wrote {}", output_path.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_v1_denied() {
        let err = validate_abi_version(2).expect_err("abi 2 should be rejected");
        assert!(
            err.contains("expected 1"),
            "error should mention expected ABI 1: {err}"
        );
        assert!(validate_abi_version(1).is_ok());
    }

    #[test]
    fn manifest_json_serialization_has_keys() {
        let m = iroha_data_model::smart_contract::manifest::ContractManifest {
            code_hash: None,
            abi_hash: None,
            compiler_fingerprint: Some("test".to_string()),
            features_bitmap: Some(0),
            access_set_hints: None,
            entrypoints: None,
            kotoba: None,
            provenance: None,
        };
        let s = manifest_to_json(&m).expect("json");
        assert!(s.contains("compiler_fingerprint"));
    }

    #[test]
    fn lint_runner_reports_unused_state() {
        let src = "seiyaku S { state int a; fn f() { } }";
        let warnings = run_lint_pass(src).expect("lint");
        assert!(
            !warnings.is_empty(),
            "expected unused-state lint to be emitted"
        );
    }

    #[test]
    fn wildcarded_entrypoints_suppress_access_hint_output() {
        let manifest = iroha_data_model::smart_contract::manifest::ContractManifest {
            code_hash: None,
            abi_hash: None,
            compiler_fingerprint: None,
            features_bitmap: None,
            access_set_hints: None,
            entrypoints: Some(vec![
                iroha_data_model::smart_contract::manifest::EntrypointDescriptor {
                    name: "run".to_string(),
                    kind: iroha_data_model::smart_contract::manifest::EntryPointKind::Public,
                    params: Vec::new(),
                    return_type: None,
                    permission: None,
                    read_keys: vec!["*".to_string()],
                    write_keys: vec!["*".to_string()],
                    access_hints_complete: Some(true),
                    access_hints_skipped: Vec::new(),
                    triggers: Vec::new(),
                },
            ]),
            kotoba: None,
            provenance: None,
        };
        assert!(manifest_entrypoints_are_globally_wildcarded(&manifest));
    }
}
