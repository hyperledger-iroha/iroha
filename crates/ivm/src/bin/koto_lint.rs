//! Kotodama lint, static analysis, and fuzzing CLI.
//!
//! Reads Kotodama source (`.ko`) or bytecode (`.to`) files and runs the
//! selected analysis passes. Lint warnings and analysis findings are reported
//! with stable codes so editors can surface issues inline.

use std::{
    env,
    ffi::{OsStr, OsString},
    fs,
    io::{self, Write},
    path::Path,
};

use ivm::{
    ProgramMetadata,
    kotodama::{
        analysis::{
            AnalysisCategory, AnalysisFinding, AnalysisSeverity,
            bytecode::{self, BytecodeAnalysisError},
            fuzz as fuzz_analysis, source as source_analysis,
        },
        i18n::{self, Message as I18nMessage},
        lint::lint_program,
        parser, semantic,
    },
};
use norito::json::{self, Value};

const EXIT_OK: i32 = 0;
const EXIT_WARNINGS: i32 = 1;
const EXIT_ERROR: i32 = 2;
const EXIT_USAGE: i32 = 64;

const DEFAULT_FUZZ_CASES: usize = 64;
const BYTECODE_FUZZ_RUNS: usize = 4;

fn json_object(entries: Vec<(String, Value)>) -> Value {
    json::object(entries).unwrap_or(Value::Null)
}

fn json_entry<K: Into<String>>(key: K, value: Value) -> (String, Value) {
    (key.into(), value)
}

fn to_value<T: json::JsonSerialize>(value: &T) -> Value {
    json::to_value(value).unwrap_or(Value::Null)
}

fn main() {
    let args: Vec<OsString> = env::args_os().skip(1).collect();
    let mut stdout = io::stdout().lock();
    let mut stderr = io::stderr().lock();
    let exit_code = run(&args, &mut stdout, &mut stderr);
    std::process::exit(exit_code);
}

fn run(args: &[OsString], stdout: &mut impl Write, stderr: &mut impl Write) -> i32 {
    let language = i18n::detect_language();
    let parsed = match parse_args(args) {
        Ok(parsed) => parsed,
        Err(err) => {
            let _ = writeln!(stderr, "{err}");
            return EXIT_USAGE;
        }
    };

    if parsed.show_help {
        print_usage(language, stdout);
        return EXIT_OK;
    }

    if parsed.inputs.is_empty() {
        print_usage(language, stderr);
        return EXIT_USAGE;
    }

    let mut exit_code = EXIT_OK;
    let mut json_reports: Vec<Value> = Vec::new();
    for raw_path in parsed.inputs {
        let path = Path::new(&raw_path);
        match detect_input_kind(path) {
            InputKind::Source => match analyze_source(path, &parsed.options, language) {
                Ok(result) => {
                    if parsed.options.output_format == OutputFormat::Text {
                        emit_source_text(path, &result, language, stdout);
                    } else {
                        json_reports.push(serialize_source_result(path, &result));
                    }
                    if result.has_warning() && exit_code == EXIT_OK {
                        exit_code = EXIT_WARNINGS;
                    }
                }
                Err(err_msg) => {
                    let _ = writeln!(stderr, "{err_msg}");
                    if parsed.options.output_format == OutputFormat::Json {
                        json_reports.push(serialize_error(path, "source", &err_msg));
                    }
                    exit_code = EXIT_ERROR;
                }
            },
            InputKind::Bytecode => {
                if parsed.options.run_lint {
                    let _ = writeln!(
                        stderr,
                        "{}: lint is only available for Kotodama source; skipping",
                        path.display()
                    );
                }
                match analyze_bytecode_file(path, &parsed.options) {
                    Ok(result) => {
                        if parsed.options.output_format == OutputFormat::Text {
                            emit_bytecode_text(path, &result, stdout);
                        } else {
                            json_reports.push(serialize_bytecode_result(path, &result));
                        }
                        if result.has_warning() && exit_code == EXIT_OK {
                            exit_code = EXIT_WARNINGS;
                        }
                    }
                    Err(err_msg) => {
                        let _ = writeln!(stderr, "{err_msg}");
                        if parsed.options.output_format == OutputFormat::Json {
                            json_reports.push(serialize_error(path, "bytecode", &err_msg));
                        }
                        exit_code = EXIT_ERROR;
                    }
                }
            }
        }
    }

    if parsed.options.output_format == OutputFormat::Json {
        let root = json_object(vec![json_entry("files", Value::Array(json_reports))]);
        match json::to_string_pretty(&root) {
            Ok(rendered) => {
                let _ = writeln!(stdout, "{rendered}");
            }
            Err(err) => {
                let _ = writeln!(stderr, "failed to encode json report: {err}");
                exit_code = EXIT_ERROR;
            }
        }
    }

    exit_code
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
enum OutputFormat {
    #[default]
    Text,
    Json,
}

#[derive(Default)]
struct Options {
    run_lint: bool,
    run_static: bool,
    run_fuzz: bool,
    output_format: OutputFormat,
}

struct ParsedArgs {
    options: Options,
    inputs: Vec<OsString>,
    show_help: bool,
}

#[derive(Clone)]
struct LintOutput {
    code: String,
    message: String,
}

struct SourceAnalysisResult {
    lint: Option<Vec<LintOutput>>,
    static_findings: Option<Vec<AnalysisFinding>>,
    fuzz_report: Option<fuzz_analysis::FuzzReport>,
}

impl SourceAnalysisResult {
    fn has_warning(&self) -> bool {
        let lint_warn = self
            .lint
            .as_ref()
            .is_some_and(|warnings| !warnings.is_empty());
        let static_warn = self.static_findings.as_ref().is_some_and(|findings| {
            findings
                .iter()
                .any(|f| f.severity == AnalysisSeverity::Warning)
        });
        let fuzz_warn = self.fuzz_report.as_ref().is_some_and(|report| {
            report
                .findings
                .iter()
                .any(|f| f.severity == AnalysisSeverity::Warning)
        });
        lint_warn || static_warn || fuzz_warn
    }
}

struct BytecodeAnalysisResult {
    metadata: Option<ProgramMetadata>,
    static_findings: Option<Vec<AnalysisFinding>>,
    fuzz_report: Option<bytecode::BytecodeFuzzReport>,
}

impl BytecodeAnalysisResult {
    fn has_warning(&self) -> bool {
        let static_warn = self.static_findings.as_ref().is_some_and(|findings| {
            findings
                .iter()
                .any(|f| f.severity == AnalysisSeverity::Warning)
        });
        let fuzz_warn = self.fuzz_report.as_ref().is_some_and(|report| {
            report
                .findings
                .iter()
                .any(|f| f.severity == AnalysisSeverity::Warning)
        });
        static_warn || fuzz_warn
    }
}

enum InputKind {
    Source,
    Bytecode,
}

fn parse_args(args: &[OsString]) -> Result<ParsedArgs, String> {
    let mut opts = Options {
        run_lint: true,
        run_static: false,
        run_fuzz: false,
        output_format: OutputFormat::Text,
    };
    let mut inputs = Vec::new();
    let mut show_help = false;

    for arg in args {
        if arg == "--help" || arg == "-h" {
            show_help = true;
            continue;
        }
        if arg == "--lint-only" {
            opts.run_lint = true;
            opts.run_static = false;
            opts.run_fuzz = false;
            continue;
        }
        if arg == "--lint" {
            opts.run_lint = true;
            continue;
        }
        if arg == "--no-lint" {
            opts.run_lint = false;
            continue;
        }
        if arg == "--static" {
            opts.run_static = true;
            continue;
        }
        if arg == "--fuzz" {
            opts.run_fuzz = true;
            continue;
        }
        if arg == "--all" {
            opts.run_lint = true;
            opts.run_static = true;
            opts.run_fuzz = true;
            continue;
        }
        if arg == "--json" {
            opts.output_format = OutputFormat::Json;
            continue;
        }
        if arg == "--text" {
            opts.output_format = OutputFormat::Text;
            continue;
        }
        if let Some(format) = arg.to_str().and_then(|s| s.strip_prefix("--format=")) {
            match format {
                "json" => opts.output_format = OutputFormat::Json,
                "text" => opts.output_format = OutputFormat::Text,
                other => return Err(format!("unknown format `{other}`")),
            }
            continue;
        }
        if arg.to_string_lossy().starts_with('-') {
            return Err(format!("unknown option `{}`", arg.to_string_lossy()));
        }
        inputs.push(arg.clone());
    }

    if !opts.run_lint && !opts.run_static && !opts.run_fuzz {
        opts.run_lint = true;
    }

    Ok(ParsedArgs {
        options: opts,
        inputs,
        show_help,
    })
}

fn detect_input_kind(path: &Path) -> InputKind {
    match path.extension().and_then(OsStr::to_str) {
        Some(ext) if ext.eq_ignore_ascii_case("to") => InputKind::Bytecode,
        _ => InputKind::Source,
    }
}

fn analyze_source(
    path: &Path,
    opts: &Options,
    language: i18n::Language,
) -> Result<SourceAnalysisResult, String> {
    let contents = fs::read_to_string(path).map_err(|err| {
        let path_buf = path.display().to_string();
        let err_str = err.to_string();
        i18n::translate(language, I18nMessage::ReadFile(&path_buf, &err_str))
    })?;

    let program = parser::parse(&contents).map_err(|err| {
        let translated = i18n::translate(language, I18nMessage::ParserError(&err));
        format!("{}: {}", path.display(), translated)
    })?;

    let lint = if opts.run_lint {
        let warnings = lint_program(&program);
        let outputs = warnings
            .into_iter()
            .map(|warning| LintOutput {
                code: warning.code.to_string(),
                message: warning.localized_message(language),
            })
            .collect();
        Some(outputs)
    } else {
        None
    };

    let typed_program = if opts.run_static || opts.run_fuzz {
        Some(semantic::analyze(&program).map_err(|err| {
            format!(
                "{}: static analysis failed: {}",
                path.display(),
                err.message
            )
        })?)
    } else {
        None
    };

    let static_findings = if opts.run_static {
        let typed = typed_program
            .as_ref()
            .expect("typed program must be available for static analysis");
        Some(source_analysis::run_static_analysis(&program, typed))
    } else {
        None
    };

    let fuzz_report = if opts.run_fuzz {
        let typed = typed_program
            .as_ref()
            .expect("typed program must be available for fuzzing");
        Some(fuzz_analysis::run_fuzz(&program, typed, DEFAULT_FUZZ_CASES))
    } else {
        None
    };

    Ok(SourceAnalysisResult {
        lint,
        static_findings,
        fuzz_report,
    })
}

fn analyze_bytecode_file(path: &Path, opts: &Options) -> Result<BytecodeAnalysisResult, String> {
    let bytes =
        fs::read(path).map_err(|err| format!("{}: failed to read: {err}", path.display()))?;

    let mut metadata: Option<ProgramMetadata> = None;

    let static_findings = if opts.run_static {
        match bytecode::analyze_bytecode(&bytes) {
            Ok(analysis) => {
                metadata = Some(analysis.metadata.clone());
                Some(analysis.findings)
            }
            Err(err) => {
                return Err(format!(
                    "{}: failed to analyse bytecode: {}",
                    path.display(),
                    display_bytecode_error(err)
                ));
            }
        }
    } else {
        None
    };

    let fuzz_report = if opts.run_fuzz {
        match bytecode::run_bytecode_fuzz(&bytes, BYTECODE_FUZZ_RUNS) {
            Ok(report) => {
                if metadata.is_none() {
                    metadata = ProgramMetadata::parse(&bytes)
                        .ok()
                        .map(|parsed| parsed.metadata);
                }
                Some(report)
            }
            Err(err) => {
                return Err(format!(
                    "{}: failed to run bytecode fuzz harness: {}",
                    path.display(),
                    display_bytecode_error(err)
                ));
            }
        }
    } else {
        None
    };

    Ok(BytecodeAnalysisResult {
        metadata,
        static_findings,
        fuzz_report,
    })
}

fn emit_source_text(
    path: &Path,
    result: &SourceAnalysisResult,
    language: i18n::Language,
    stdout: &mut impl Write,
) {
    if let Some(lints) = &result.lint {
        if lints.is_empty() {
            let ok_msg = i18n::translate(language, I18nMessage::LintOk);
            let _ = writeln!(stdout, "{}: lint: {}", path.display(), ok_msg);
        } else {
            for warning in lints {
                let _ = writeln!(
                    stdout,
                    "{}: lint: {}: {}",
                    path.display(),
                    warning.code,
                    warning.message
                );
            }
        }
    }

    if let Some(findings) = &result.static_findings {
        emit_findings(path, stdout, findings);
    }

    if let Some(report) = &result.fuzz_report {
        emit_findings(path, stdout, &report.findings);
        let summary = if report.cases_executed > 0 {
            format!(
                "executed {} cases across {} functions",
                report.cases_executed, report.functions_covered
            )
        } else {
            "no compatible functions for harness".to_string()
        };
        let _ = writeln!(
            stdout,
            "{}: {}: info: fuzz-summary: {}",
            path.display(),
            AnalysisCategory::Fuzz,
            summary
        );
    }
}

fn emit_bytecode_text(path: &Path, result: &BytecodeAnalysisResult, stdout: &mut impl Write) {
    if let Some(findings) = &result.static_findings {
        emit_findings(path, stdout, findings);
    }

    let should_print_header = match &result.static_findings {
        Some(findings) => findings.is_empty(),
        None => result.metadata.is_some(),
    };
    if let Some(meta) = result.metadata.as_ref().filter(|_| should_print_header) {
        let _ = writeln!(
            stdout,
            "{}: {}: info: bytecode-header: abi_version={} max_cycles={} vector_length_hint={}",
            path.display(),
            AnalysisCategory::BytecodeStatic,
            meta.abi_version,
            meta.max_cycles,
            meta.vector_length
        );
    }

    if let Some(report) = &result.fuzz_report {
        emit_findings(path, stdout, &report.findings);
        let _ = writeln!(
            stdout,
            "{}: {}: info: fuzz-summary: executed {} runs ({} failures)",
            path.display(),
            AnalysisCategory::BytecodeFuzz,
            report.runs,
            report.failures
        );
    }
}

fn serialize_source_result(path: &Path, result: &SourceAnalysisResult) -> Value {
    let static_val = result
        .static_findings
        .as_ref()
        .map(|findings| json_object(vec![json_entry("findings", findings_to_json(findings))]))
        .unwrap_or(Value::Null);

    let fuzz_val = result
        .fuzz_report
        .as_ref()
        .map(|report| {
            json_object(vec![
                json_entry("findings", findings_to_json(&report.findings)),
                json_entry("cases_executed", to_value(&report.cases_executed)),
                json_entry("functions_covered", to_value(&report.functions_covered)),
            ])
        })
        .unwrap_or(Value::Null);

    let path_str = path.display().to_string();
    let kind_str = "source".to_string();
    json_object(vec![
        json_entry("path", to_value(&path_str)),
        json_entry("kind", to_value(&kind_str)),
        json_entry("lint", lint_to_json(&result.lint)),
        json_entry("static", static_val),
        json_entry("fuzz", fuzz_val),
    ])
}

fn serialize_bytecode_result(path: &Path, result: &BytecodeAnalysisResult) -> Value {
    let metadata_val = result
        .metadata
        .as_ref()
        .map(|meta| {
            json_object(vec![
                json_entry("abi_version", to_value(&meta.abi_version)),
                json_entry("mode", to_value(&meta.mode)),
                json_entry("vector_length_hint", to_value(&meta.vector_length)),
                json_entry("max_cycles", to_value(&meta.max_cycles)),
            ])
        })
        .unwrap_or(Value::Null);

    let static_val = result
        .static_findings
        .as_ref()
        .map(|findings| json_object(vec![json_entry("findings", findings_to_json(findings))]))
        .unwrap_or(Value::Null);

    let fuzz_val = result
        .fuzz_report
        .as_ref()
        .map(|report| {
            json_object(vec![
                json_entry("findings", findings_to_json(&report.findings)),
                json_entry("runs", to_value(&report.runs)),
                json_entry("failures", to_value(&report.failures)),
            ])
        })
        .unwrap_or(Value::Null);

    let path_str = path.display().to_string();
    let kind_str = "bytecode".to_string();
    json_object(vec![
        json_entry("path", to_value(&path_str)),
        json_entry("kind", to_value(&kind_str)),
        json_entry("metadata", metadata_val),
        json_entry("static", static_val),
        json_entry("fuzz", fuzz_val),
    ])
}

fn serialize_error(path: &Path, kind: &str, message: &str) -> Value {
    let path_str = path.display().to_string();
    let kind_str = kind.to_string();
    let message_str = message.to_string();
    json_object(vec![
        json_entry("path", to_value(&path_str)),
        json_entry("kind", to_value(&kind_str)),
        json_entry("error", to_value(&message_str)),
    ])
}

fn lint_to_json(lint: &Option<Vec<LintOutput>>) -> Value {
    match lint {
        None => Value::Null,
        Some(warnings) => {
            let warnings_json: Vec<Value> = warnings
                .iter()
                .map(|w| {
                    json_object(vec![
                        json_entry("code", to_value(&w.code)),
                        json_entry("message", to_value(&w.message)),
                    ])
                })
                .collect();
            let status = if warnings.is_empty() {
                "ok"
            } else {
                "warnings"
            };
            let status_str = status.to_string();
            json_object(vec![
                json_entry("status", to_value(&status_str)),
                json_entry("warnings", Value::Array(warnings_json)),
            ])
        }
    }
}

fn findings_to_json(findings: &[AnalysisFinding]) -> Value {
    Value::Array(
        findings
            .iter()
            .map(|f| {
                json_object(vec![
                    json_entry("category", to_value(&f.category.to_string())),
                    json_entry("severity", to_value(&f.severity.to_string())),
                    json_entry("code", to_value(&f.code)),
                    json_entry("message", to_value(&f.message)),
                ])
            })
            .collect(),
    )
}

fn emit_findings(path: &Path, stdout: &mut impl Write, findings: &[AnalysisFinding]) -> bool {
    let mut warned = false;
    for finding in findings {
        if finding.severity == AnalysisSeverity::Warning {
            warned = true;
        }
        let _ = writeln!(
            stdout,
            "{}: {}: {}: {}: {}",
            path.display(),
            finding.category,
            finding.severity,
            finding.code,
            finding.message
        );
    }
    warned
}

fn print_usage(language: i18n::Language, out: &mut impl Write) {
    let _ = writeln!(out, "{}", i18n::translate(language, I18nMessage::LintUsage));
    let _ = writeln!(
        out,
        "{}",
        i18n::translate(language, I18nMessage::LintUsageHelp)
    );
    let _ = writeln!(
        out,
        "Options: [--lint] [--no-lint] [--static] [--fuzz] [--all] [--lint-only] [--json] [--text] [--format=<text|json>]"
    );
}

fn display_bytecode_error(err: BytecodeAnalysisError) -> String {
    err.to_string()
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        sync::{
            Mutex,
            atomic::{AtomicUsize, Ordering},
        },
    };

    use norito::json::Value;

    use super::*;

    static TEST_FILE_COUNTER: AtomicUsize = AtomicUsize::new(0);
    static LANG_LOCK: Mutex<()> = Mutex::new(());

    fn write_temp_file(ext: &str, contents: &[u8]) -> PathBuf {
        let mut path = std::env::temp_dir();
        let unique = TEST_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
        path.push(format!(
            "koto_lint_test_{}_{}.{}",
            std::process::id(),
            unique,
            ext
        ));
        fs::write(&path, contents).expect("write temp file");
        path
    }

    fn with_lang_en<F: FnOnce()>(f: F) {
        let _guard = LANG_LOCK.lock().expect("lang lock poisoned");
        let previous = env::var("LANG").ok();
        unsafe {
            env::set_var("LANG", "en_US.UTF-8");
        }
        f();
        unsafe {
            if let Some(prev) = previous {
                env::set_var("LANG", prev);
            } else {
                env::remove_var("LANG");
            }
        }
    }

    #[test]
    fn run_returns_usage_without_args() {
        with_lang_en(|| {
            let mut out = Vec::new();
            let mut err = Vec::new();
            let exit = run(&[], &mut out, &mut err);
            assert_eq!(exit, EXIT_USAGE);
            let stderr = String::from_utf8(err).expect("stderr must be UTF-8");
            assert!(stderr.contains("Usage: koto_lint"));
            assert!(out.is_empty());
        });
    }

    #[cfg(unix)]
    #[test]
    fn run_accepts_non_utf8_path() {
        use std::os::unix::ffi::OsStringExt;
        with_lang_en(|| {
            let file_name = OsString::from_vec(vec![b'f', b'o', b'o', 0xff, b'.', b'k', b'o']);
            let mut out = Vec::new();
            let mut err = Vec::new();
            let args = vec![file_name];
            let exit = run(&args, &mut out, &mut err);

            assert_eq!(exit, EXIT_ERROR);
            assert!(out.is_empty());
            let stderr = String::from_utf8_lossy(&err);
            assert!(
                stderr.contains("failed to read"),
                "expected localized read error message, got {stderr:?}"
            );
        });
    }

    #[test]
    fn run_reports_unused_state_warning() {
        with_lang_en(|| {
            let path = write_temp_file(
                "ko",
                br#"
                state Map<int,int> unused;
                fn main() {}
            "#,
            );
            let mut out = Vec::new();
            let mut err = Vec::new();
            let args = vec![path.clone().into_os_string()];
            let exit = run(&args, &mut out, &mut err);

            let _ = fs::remove_file(&path);

            assert_eq!(exit, EXIT_WARNINGS);
            let stdout = String::from_utf8(out).expect("stdout must be utf-8");
            assert!(
                stdout.contains("unused-state"),
                "expected unused-state warning, got {stdout:?}"
            );
            assert!(
                err.is_empty(),
                "stderr should be empty when only warnings are emitted"
            );
        });
    }

    #[test]
    fn run_reports_parse_errors() {
        with_lang_en(|| {
            let path = write_temp_file("ko", b"this is not valid kotodama");
            let mut out = Vec::new();
            let mut err = Vec::new();
            let args = vec![path.clone().into_os_string()];
            let exit = run(&args, &mut out, &mut err);

            let _ = fs::remove_file(&path);

            assert_eq!(exit, EXIT_ERROR);
            assert!(
                out.is_empty(),
                "stdout should be empty when parsing fails, got {out:?}"
            );
            let stderr = String::from_utf8(err).expect("stderr must be utf-8");
            assert!(
                stderr.contains("parser error"),
                "expected parser error, got {stderr:?}"
            );
        });
    }

    #[test]
    fn run_outputs_json_report() {
        with_lang_en(|| {
            let path = write_temp_file(
                "ko",
                br#"
                state Map<int,int> data;
                fn main() {}
            "#,
            );
            let mut out = Vec::new();
            let mut err = Vec::new();
            let args = vec![OsString::from("--json"), path.clone().into_os_string()];
            let exit = run(&args, &mut out, &mut err);
            let _ = fs::remove_file(&path);

            assert_eq!(exit, EXIT_WARNINGS);
            assert!(err.is_empty(), "stderr should be empty: {err:?}");
            let stdout = String::from_utf8(out).expect("stdout utf8");
            let json_value: Value = norito::json::from_str(&stdout).expect("parse json output");
            let files = json_value
                .as_object()
                .and_then(|obj| obj.get("files"))
                .and_then(|v| v.as_array())
                .expect("files array");
            assert!(!files.is_empty(), "expected at least one file report");
        });
    }
}
