//! Unified Kotodama V1 developer command.

use std::{
    collections::{BTreeSet, HashMap},
    env,
    io::{BufRead, Write},
    path::{Path, PathBuf},
};

use ivm::kotodama::{
    builtins::{Builtin, BuiltinSurface},
    compiler::CompilerOptions,
    diagnostic::{
        Diagnostic, DiagnosticBundle, DiagnosticPhase, Severity, SourcePosition, SourceSpan,
    },
    driver::{
        BuildDriver, BuildStatus, PublishLayout, PublishMode, SourceBuildRequest,
        atomic_write_if_changed, read_source_file,
    },
    formatter::format_source,
    lexer::{V1_KEYWORDS, V1_OPERATORS},
    semantic::{V1_LIST_MEMBER_NAMES, V1_ROUNDING_PATHS, V1_SOURCE_TYPE_NAMES, V1_SUM_PATHS},
    session::{CompileOutput, CompileRequest, CompilerSession},
    source::{FrontendBudget, MAX_SOURCE_BYTES, SourceFile, SourceId},
};

const USAGE: &str = "\
Kotodama V1 toolchain

Usage:
  koto check [--format human|json|sarif] [--zk] <source.ko>...
  koto build [--profile <name>] [--target-dir <path>] [--out <file.to>]
             [--manifest-out <file.json>] [--max-cycles <count>] [--zk] [--verify]
             <source.ko>...
  koto test [run|coverage|profile|list] [--zk] <options> <source.ko>
  koto fmt [--check] <source.ko>...
  koto doc [--format markdown|json] [--zk] <source.ko>
  koto explain <diagnostic-code>
  koto lsp
";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum KotoCommand {
    Check,
    Build,
    Test,
    Fmt,
    Doc,
    Explain,
    Lsp,
}

const KOTO_COMMAND_INVENTORY: [(&str, KotoCommand); 7] = [
    ("check", KotoCommand::Check),
    ("build", KotoCommand::Build),
    ("test", KotoCommand::Test),
    ("fmt", KotoCommand::Fmt),
    ("doc", KotoCommand::Doc),
    ("explain", KotoCommand::Explain),
    ("lsp", KotoCommand::Lsp),
];

fn koto_command(raw: &str) -> Option<KotoCommand> {
    KOTO_COMMAND_INVENTORY
        .iter()
        .find_map(|(name, command)| (*name == raw).then_some(*command))
}

// JSON can escape one source byte into as many as six ASCII bytes. The wire
// budget admits every canonical 1 MiB source while remaining strictly bounded.
const MAX_LSP_MESSAGE_BYTES: usize = MAX_SOURCE_BYTES * 6 + 256 * 1024;
const MAX_LSP_HEADER_LINE_BYTES: usize = 8 * 1024;
const MAX_LSP_HEADERS: usize = 32;
const MAX_LSP_URI_BYTES: usize = 8 * 1024;
const MAX_LSP_OPEN_DOCUMENTS: usize = 256;
const MAX_LSP_DOCUMENT_BYTES: usize = 64 * MAX_SOURCE_BYTES;

// Contextual syntax and compiler intrinsics do not appear in the lexical
// keyword or public builtin registries, but they are still source-visible V1
// completions. Registered builtins (including receiver methods), sum paths,
// rounding paths, types, and bounded-list members are sourced from their
// canonical compiler tables below.
const V1_CONTEXTUAL_COMPLETIONS: &[(&str, u64)] = &[("json", 14), ("div_round", 2)];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum DiagnosticFormat {
    #[default]
    Human,
    Json,
    Sarif,
}

impl DiagnosticFormat {
    fn parse(raw: &str) -> Result<Self, String> {
        match raw {
            "human" | "text" => Ok(Self::Human),
            "json" => Ok(Self::Json),
            "sarif" => Ok(Self::Sarif),
            _ => Err(format!(
                "unknown diagnostic format `{raw}`; expected human, json, or sarif"
            )),
        }
    }

    fn render(self, diagnostics: &DiagnosticBundle) -> String {
        match self {
            Self::Human => diagnostics.render_human(),
            Self::Json => diagnostics
                .render_json()
                .unwrap_or_else(|error| format!("failed to render diagnostics: {error}")),
            Self::Sarif => diagnostics
                .render_sarif()
                .unwrap_or_else(|error| format!("failed to render diagnostics: {error}")),
        }
    }
}

fn main() {
    if let Err(error) = run(env::args().skip(1).collect()) {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run(mut args: Vec<String>) -> Result<(), String> {
    let Some(command) = args.first().cloned() else {
        print!("{USAGE}");
        return Ok(());
    };
    args.remove(0);
    match koto_command(&command) {
        Some(KotoCommand::Check) => check(args),
        Some(KotoCommand::Build) => build(args),
        Some(KotoCommand::Test) => ivm::koto_test_driver::run_cli(args),
        Some(KotoCommand::Fmt) => format_sources(args),
        Some(KotoCommand::Doc) => document(args),
        Some(KotoCommand::Explain) => explain(args),
        Some(KotoCommand::Lsp) => language_server(),
        None if matches!(command.as_str(), "help" | "--help" | "-h") => {
            print!("{USAGE}");
            Ok(())
        }
        None => Err(format!("unknown command `{command}`\n\n{USAGE}")),
    }
}

fn check(args: Vec<String>) -> Result<(), String> {
    let (format, zk_enabled, inputs) = parse_format_and_inputs(args)?;
    let session = CompilerSession::new(CompilerOptions {
        force_zk: zk_enabled,
        ..CompilerOptions::default()
    });
    let (checked, diagnostics) = check_paths(&session, inputs);
    if format == DiagnosticFormat::Human {
        for path in checked {
            println!("checked {}", path.display());
        }
        if !diagnostics.diagnostics.is_empty() {
            eprintln!("{}", format.render(&diagnostics));
        }
    } else {
        // A batch is one machine-readable document, including the successful
        // empty result. Concatenated JSON arrays or SARIF logs are not valid
        // input for CI consumers.
        println!("{}", format.render(&diagnostics));
    }
    if diagnostics
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == Severity::Error)
    {
        Err("one or more sources failed validation".to_owned())
    } else {
        Ok(())
    }
}

fn check_paths(
    session: &CompilerSession,
    inputs: Vec<PathBuf>,
) -> (Vec<PathBuf>, DiagnosticBundle) {
    let mut checked = Vec::new();
    let mut diagnostics = Vec::new();
    for path in inputs {
        match check_path(session, &path) {
            Ok(bundle) => {
                checked.push(path);
                diagnostics.extend(bundle.diagnostics);
            }
            Err(bundle) => diagnostics.extend(bundle.diagnostics),
        }
    }
    (checked, DiagnosticBundle::new(diagnostics))
}

fn build(args: Vec<String>) -> Result<(), String> {
    let mut profile = String::from("dev");
    let mut target_dir = PathBuf::from("target/kotodama");
    let mut explicit_output = None;
    let mut explicit_manifest_output = None;
    let mut max_cycles = None;
    let mut zk_enabled = false;
    let mut publish_mode = PublishMode::Write;
    let mut inputs = Vec::new();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--profile" => {
                index += 1;
                profile = args
                    .get(index)
                    .ok_or_else(|| "--profile requires a value".to_owned())?
                    .clone();
            }
            "--target-dir" => {
                index += 1;
                target_dir = PathBuf::from(
                    args.get(index)
                        .ok_or_else(|| "--target-dir requires a value".to_owned())?,
                );
            }
            "--out" => {
                index += 1;
                explicit_output = Some(PathBuf::from(
                    args.get(index)
                        .ok_or_else(|| "--out requires a value".to_owned())?,
                ));
            }
            "--manifest-out" => {
                index += 1;
                explicit_manifest_output =
                    Some(PathBuf::from(args.get(index).ok_or_else(|| {
                        "--manifest-out requires a value".to_owned()
                    })?));
            }
            "--max-cycles" => {
                index += 1;
                let raw = args
                    .get(index)
                    .ok_or_else(|| "--max-cycles requires a value".to_owned())?;
                let parsed = raw
                    .parse::<u64>()
                    .map_err(|error| format!("invalid --max-cycles value `{raw}`: {error}"))?;
                if parsed == 0 {
                    return Err("--max-cycles must be greater than zero".to_owned());
                }
                max_cycles = Some(parsed);
            }
            "--zk" => zk_enabled = true,
            "--verify" => publish_mode = PublishMode::Verify,
            flag if flag.starts_with('-') => return Err(format!("unknown build option `{flag}`")),
            path => inputs.push(PathBuf::from(path)),
        }
        index += 1;
    }
    if inputs.is_empty() {
        return Err("build requires at least one .ko source".to_owned());
    }
    if explicit_output.is_some() && inputs.len() != 1 {
        return Err("--out can be used only when building one source".to_owned());
    }
    if explicit_manifest_output.is_some() && inputs.len() != 1 {
        return Err("--manifest-out can be used only when building one source".to_owned());
    }
    let mut compiler_options = CompilerOptions::default();
    if let Some(max_cycles) = max_cycles {
        compiler_options.max_cycles = max_cycles;
    }
    compiler_options.force_zk = zk_enabled;
    let session = CompilerSession::new(compiler_options);
    let driver = BuildDriver::for_current_executable(session).map_err(|error| error.to_string())?;
    let manifest_stdout = explicit_manifest_output.as_deref() == Some(Path::new("-"));
    let mut requests = Vec::with_capacity(inputs.len());
    for input in &inputs {
        let source = read_source_file(input).map_err(|error| error.to_string())?;
        let stem = input
            .file_stem()
            .and_then(|stem| stem.to_str())
            .ok_or_else(|| format!("{} has no UTF-8 file stem", input.display()))?;
        let mut layout = if let Some(output) = explicit_output.as_ref() {
            PublishLayout::for_artifact(output.clone(), None, None)
        } else {
            PublishLayout::standard(&target_dir, &profile, stem, false)
        }
        .map_err(|error| error.to_string())?;
        if let Some(manifest) = explicit_manifest_output
            .as_ref()
            .filter(|path| path.as_path() != Path::new("-"))
        {
            layout.manifest = manifest.clone();
        }
        if manifest_stdout {
            layout = layout.with_sidecar_manifest();
        }
        requests.push(SourceBuildRequest {
            source,
            source_name: input.display().to_string(),
            profile: profile.clone(),
            layout,
            mode: publish_mode,
        });
    }
    let outcomes = driver
        .build_source_batch(requests)
        .map_err(|error| error.to_string())?;
    for outcome in outcomes {
        let notice = match outcome.status {
            BuildStatus::Fresh => "fresh",
            BuildStatus::Built => "built",
        };
        if manifest_stdout {
            eprintln!("{notice} {}", outcome.paths.artifact.display());
            println!(
                "{}",
                norito::json::to_json_pretty(&outcome.manifest)
                    .map_err(|error| format!("render contract manifest: {error}"))?
            );
        } else {
            println!("{notice} {}", outcome.paths.artifact.display());
        }
    }
    Ok(())
}

fn format_sources(args: Vec<String>) -> Result<(), String> {
    let (check_only, inputs) = parse_format_sources_args(args)?;
    let mut changed = false;
    for path in inputs {
        let source = read_source_file(&path).map_err(|error| error.to_string())?;
        let formatted = format_source_text(&source, path.to_str())?;
        if formatted != source {
            changed = true;
            if check_only {
                eprintln!("would format {}", path.display());
            } else {
                atomic_write_if_changed(&path, formatted.as_bytes())
                    .map_err(|error| error.to_string())?;
                println!("formatted {}", path.display());
            }
        }
    }
    if check_only && changed {
        Err("one or more sources require formatting".to_owned())
    } else {
        Ok(())
    }
}

fn parse_format_sources_args(args: Vec<String>) -> Result<(bool, Vec<PathBuf>), String> {
    let mut check_only = false;
    let mut positional_only = false;
    let mut inputs = Vec::new();
    for argument in args {
        match argument.as_str() {
            "--" if !positional_only => positional_only = true,
            "--check" if !positional_only => check_only = true,
            flag if !positional_only && flag.starts_with('-') => {
                return Err(format!("unknown fmt option `{flag}`"));
            }
            _ => inputs.push(PathBuf::from(argument)),
        }
    }
    if inputs.is_empty() {
        return Err("fmt requires at least one .ko source".to_owned());
    }
    Ok((check_only, inputs))
}

fn format_source_text(source: &str, source_name: Option<&str>) -> Result<String, String> {
    let file = SourceFile::new(SourceId(0), source_name.unwrap_or("<source>"), source);
    format_source(&file, FrontendBudget::v1()).map_err(|diagnostics| diagnostics.render_human())
}

fn document(args: Vec<String>) -> Result<(), String> {
    let mut format = "markdown";
    let mut zk_enabled = false;
    let mut input = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--format" => {
                index += 1;
                format = args
                    .get(index)
                    .ok_or_else(|| "doc --format requires a value".to_owned())?;
                if !matches!(format, "markdown" | "json") {
                    return Err(format!(
                        "unknown doc format `{format}`; expected markdown or json"
                    ));
                }
            }
            "--zk" => zk_enabled = true,
            flag if flag.starts_with('-') => {
                return Err(format!("unknown doc option `{flag}`"));
            }
            path if input.is_none() => input = Some(PathBuf::from(path)),
            _ => return Err("doc expects exactly one .ko source".to_owned()),
        }
        index += 1;
    }
    let path = input.ok_or_else(|| "doc expects exactly one .ko source".to_owned())?;
    let session = CompilerSession::new(CompilerOptions {
        force_zk: zk_enabled,
        ..CompilerOptions::default()
    });
    let output = compile_path(&session, &path).map_err(|diagnostics| diagnostics.render_human())?;
    let rendered = if format == "json" {
        norito::json::to_json_pretty(&output.manifest)
            .map_err(|error| format!("render contract interface: {error}"))?
    } else {
        render_contract_documentation(&output.manifest)
    };
    println!("{rendered}");
    Ok(())
}

fn markdown_inline(text: &str) -> String {
    text.replace(['\n', '\r'], " ").replace('`', "\\`")
}

fn render_contract_documentation(
    manifest: &iroha_data_model::smart_contract::manifest::ContractManifest,
) -> String {
    use iroha_data_model::smart_contract::manifest::EntryPointKind;
    use std::fmt::Write as _;

    let seiyaku_name = manifest.seiyaku_name.as_deref().unwrap_or("Seiyaku");
    let mut output = format!("# {}\n", markdown_inline(seiyaku_name));
    if let Some(code_hash) = manifest.code_hash.as_ref() {
        let _ = writeln!(output, "\nCanonical artifact: `{code_hash}`");
    }
    if let Some(abi_hash) = manifest.abi_hash.as_ref() {
        let _ = writeln!(output, "ABI V1: `{abi_hash}`");
    }

    output.push_str("\n## Entrypoints\n");
    for entrypoint in manifest.entrypoints.as_deref().unwrap_or_default() {
        let parameters = entrypoint
            .params
            .iter()
            .map(|parameter| {
                format!(
                    "{} {}",
                    markdown_inline(&parameter.type_name),
                    markdown_inline(&parameter.name)
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        let return_type = entrypoint
            .return_type
            .as_deref()
            .map_or(String::new(), |ty| format!(" -> {}", markdown_inline(ty)));
        let _ = writeln!(
            output,
            "\n### `{}({}){}`",
            markdown_inline(&entrypoint.name),
            parameters,
            return_type
        );
        let kind = match entrypoint.kind {
            EntryPointKind::Kotoage => "`kotoage`/`言挙げ`",
            EntryPointKind::View => "`view`",
            EntryPointKind::Hajimari => "`hajimari`/`始まり`",
            EntryPointKind::Kaizen => "`kaizen`/`改善`",
        };
        let _ = writeln!(output, "\nKind: {kind}");
        match entrypoint.permission.as_deref() {
            Some(permission) => {
                let _ = writeln!(output, "Authorization: `{}`", markdown_inline(permission));
            }
            None if matches!(
                entrypoint.kind,
                EntryPointKind::Hajimari | EntryPointKind::Kaizen
            ) =>
            {
                output.push_str("Authorization: runtime-defined lifecycle policy\n");
            }
            None => output.push_str("Authorization: public\n"),
        }
        let access_status = if entrypoint.access_hints_complete == Some(true)
            && entrypoint.access_hints_skipped.is_empty()
        {
            "complete compiler derivation"
        } else {
            "conservative serialization required"
        };
        let _ = writeln!(output, "Access analysis: {access_status}");
        if !entrypoint.read_keys.is_empty() {
            let _ = writeln!(
                output,
                "Reads: {}",
                entrypoint
                    .read_keys
                    .iter()
                    .map(|key| format!("`{}`", markdown_inline(key)))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        if !entrypoint.write_keys.is_empty() {
            let _ = writeln!(
                output,
                "Writes: {}",
                entrypoint
                    .write_keys
                    .iter()
                    .map(|key| format!("`{}`", markdown_inline(key)))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        for reason in &entrypoint.access_hints_skipped {
            let _ = writeln!(output, "Access note: {}", markdown_inline(reason));
        }
    }

    if let Some(states) = manifest.states.as_deref()
        && !states.is_empty()
    {
        output.push_str("\n## Durable state\n");
        for state in states {
            let _ = writeln!(
                output,
                "\n- `{}` `{}`",
                markdown_inline(&state.type_name),
                markdown_inline(&state.name)
            );
        }
    }
    if let Some(error_codes) = manifest.error_codes.as_deref()
        && !error_codes.is_empty()
    {
        output.push_str("\n## Seiyaku errors\n");
        for error in error_codes {
            let _ = writeln!(
                output,
                "\n- `{}::{}` = `{}`",
                markdown_inline(&error.namespace),
                markdown_inline(&error.name),
                error.code
            );
        }
    }
    output
}

fn explain(args: Vec<String>) -> Result<(), String> {
    if args.len() != 1 {
        return Err("explain expects one diagnostic code".to_owned());
    }
    let code = args[0].to_ascii_uppercase();
    let explanation = ivm::kotodama::diagnostic::diagnostic_explanation(&code)
        .ok_or_else(|| format!("no explanation is registered for `{code}`"))?;
    println!(
        "{} [{}]: {}\nhelp: {}",
        explanation.code,
        explanation.phase.as_str(),
        explanation.summary,
        explanation.help
    );
    Ok(())
}

fn parse_format_and_inputs(
    args: Vec<String>,
) -> Result<(DiagnosticFormat, bool, Vec<PathBuf>), String> {
    let mut format = DiagnosticFormat::Human;
    let mut zk_enabled = false;
    let mut inputs = Vec::new();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--format" => {
                index += 1;
                format = DiagnosticFormat::parse(
                    args.get(index)
                        .ok_or_else(|| "--format requires a value".to_owned())?,
                )?;
            }
            "--zk" => zk_enabled = true,
            flag if flag.starts_with('-') => return Err(format!("unknown option `{flag}`")),
            path => inputs.push(PathBuf::from(path)),
        }
        index += 1;
    }
    if inputs.is_empty() {
        return Err("check requires at least one .ko source".to_owned());
    }
    Ok((format, zk_enabled, inputs))
}

fn compile_path(session: &CompilerSession, path: &Path) -> Result<CompileOutput, DiagnosticBundle> {
    let source = read_source_file(path).map_err(|error| {
        DiagnosticBundle::single(Diagnostic::error(
            "K0000",
            DiagnosticPhase::Lex,
            format!("failed to read source `{}`: {error}", path.display()),
            None,
        ))
    })?;
    session.build(CompileRequest {
        source: &source,
        source_name: path.to_str(),
    })
}

fn check_path(
    session: &CompilerSession,
    path: &Path,
) -> Result<DiagnosticBundle, DiagnosticBundle> {
    let source = read_source_file(path).map_err(|error| {
        DiagnosticBundle::single(Diagnostic::error(
            "K0000",
            DiagnosticPhase::Lex,
            format!("failed to read source `{}`: {error}", path.display()),
            None,
        ))
    })?;
    let warnings = session.check_with_lints(CompileRequest {
        source: &source,
        source_name: path.to_str(),
    })?;
    Ok(DiagnosticBundle::new(
        warnings
            .into_iter()
            .map(|warning| lint_diagnostic(warning, path))
            .collect(),
    ))
}

fn lint_diagnostic(warning: ivm::kotodama::lint::LintWarning, path: &Path) -> Diagnostic {
    let code = warning.diagnostic_code();
    let (line, column) = warning
        .source
        .as_ref()
        .map_or((1, 1), |span| (span.line.max(1), span.column.max(1)));
    let position = SourcePosition { line, column };
    let span = SourceSpan {
        source: Some(path.display().to_string()),
        start: position,
        end: position,
        byte_range: None,
    };
    let mut diagnostic = Diagnostic::warning(
        code,
        DiagnosticPhase::Semantic,
        warning.localized_message(ivm::kotodama::i18n::detect_language()),
        Some(span),
    );
    diagnostic.notes.push(format!(
        "lint `{}` in category `{}`",
        warning.code,
        warning.category.as_str()
    ));
    diagnostic
}

fn language_server() -> Result<(), String> {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut input = stdin.lock();
    let mut output = stdout.lock();
    let mut documents = HashMap::<String, String>::new();
    let session = CompilerSession::default();

    while let Some(message) = read_lsp_message(&mut input)? {
        let method = message
            .get("method")
            .and_then(norito::json::Value::as_str)
            .map(ToOwned::to_owned);
        let id = message.get("id").cloned();
        match method.as_deref() {
            Some("initialize") => {
                write_lsp_response(&mut output, id, lsp_initialize_result())?;
            }
            Some("shutdown") => {
                write_lsp_response(&mut output, id, norito::json::Value::Null)?;
            }
            Some("exit") => return Ok(()),
            Some("textDocument/didOpen") => {
                if let (Some(uri), Some(text)) = (
                    message
                        .pointer("/params/textDocument/uri")
                        .and_then(norito::json::Value::as_str),
                    message
                        .pointer("/params/textDocument/text")
                        .and_then(norito::json::Value::as_str),
                ) {
                    if let Err(message) = store_lsp_document(&mut documents, uri, text) {
                        publish_lsp_notification(
                            &mut output,
                            "window/showMessage",
                            json_object(vec![
                                ("type", norito::json::Value::from(1_u64)),
                                ("message", norito::json::Value::from(message)),
                            ]),
                        )?;
                    }
                    publish_lsp_diagnostics(&mut output, &session, uri, text)?;
                }
            }
            Some("textDocument/didChange") => {
                if let (Some(uri), Some(text)) = (
                    message
                        .pointer("/params/textDocument/uri")
                        .and_then(norito::json::Value::as_str),
                    message
                        .pointer("/params/contentChanges/0/text")
                        .and_then(norito::json::Value::as_str),
                ) {
                    if let Err(message) = store_lsp_document(&mut documents, uri, text) {
                        publish_lsp_notification(
                            &mut output,
                            "window/showMessage",
                            json_object(vec![
                                ("type", norito::json::Value::from(1_u64)),
                                ("message", norito::json::Value::from(message)),
                            ]),
                        )?;
                    }
                    publish_lsp_diagnostics(&mut output, &session, uri, text)?;
                }
            }
            Some("textDocument/didClose") => {
                if let Some(uri) = message
                    .pointer("/params/textDocument/uri")
                    .and_then(norito::json::Value::as_str)
                {
                    documents.remove(uri);
                    publish_lsp_notification(
                        &mut output,
                        "textDocument/publishDiagnostics",
                        json_object(vec![
                            ("uri", norito::json::Value::from(uri)),
                            ("diagnostics", norito::json::Value::Array(Vec::new())),
                        ]),
                    )?;
                }
            }
            Some("textDocument/completion") => {
                write_lsp_response(&mut output, id, lsp_completion_items())?;
            }
            Some("textDocument/codeAction") => {
                let actions = message
                    .pointer("/params/textDocument/uri")
                    .and_then(norito::json::Value::as_str)
                    .and_then(|uri| {
                        documents
                            .get(uri)
                            .map(|source| lsp_code_action_items(&session, uri, source))
                    })
                    .unwrap_or_else(|| norito::json::Value::Array(Vec::new()));
                write_lsp_response(&mut output, id, actions)?;
            }
            Some("textDocument/formatting") => {
                let edits = message
                    .pointer("/params/textDocument/uri")
                    .and_then(norito::json::Value::as_str)
                    .and_then(|uri| documents.get(uri))
                    .map_or_else(Vec::new, |source| {
                        let Ok(formatted) = format_source_text(source, None) else {
                            return Vec::new();
                        };
                        if formatted == source.as_str() {
                            Vec::new()
                        } else {
                            vec![json_object(vec![
                                (
                                    "range",
                                    json_object(vec![
                                        ("start", lsp_position(0_u64, 0_u64)),
                                        ("end", lsp_position(u32::MAX, 0_u64)),
                                    ]),
                                ),
                                ("newText", norito::json::Value::from(formatted)),
                            ])]
                        }
                    });
                write_lsp_response(&mut output, id, norito::json::Value::Array(edits))?;
            }
            Some(_) if id.is_some() => {
                write_lsp_error(&mut output, id, -32601, "method not found")?;
            }
            Some(_) | None => {}
        }
    }
    Ok(())
}

fn store_lsp_document(
    documents: &mut HashMap<String, String>,
    uri: &str,
    source: &str,
) -> Result<(), String> {
    if uri.len() > MAX_LSP_URI_BYTES {
        return Err(format!(
            "Kotodama document URI exceeds the {MAX_LSP_URI_BYTES}-byte language-server limit"
        ));
    }
    if source.len() > MAX_SOURCE_BYTES {
        documents.remove(uri);
        return Err(format!(
            "Kotodama document `{uri}` exceeds the {MAX_SOURCE_BYTES}-byte V1 source limit"
        ));
    }
    let previous_bytes = documents.get(uri).map_or(0, String::len);
    let total_bytes = documents
        .values()
        .fold(0_usize, |total, value| total.saturating_add(value.len()))
        .saturating_sub(previous_bytes)
        .saturating_add(source.len());
    let document_count = documents
        .len()
        .saturating_add(usize::from(!documents.contains_key(uri)));
    if document_count > MAX_LSP_OPEN_DOCUMENTS || total_bytes > MAX_LSP_DOCUMENT_BYTES {
        documents.remove(uri);
        return Err(format!(
            "Kotodama language server workspace limit reached ({MAX_LSP_OPEN_DOCUMENTS} documents/{MAX_LSP_DOCUMENT_BYTES} bytes); close unused documents"
        ));
    }
    documents.insert(uri.to_owned(), source.to_owned());
    Ok(())
}

fn read_bounded_lsp_header_line(
    input: &mut impl BufRead,
    line: &mut Vec<u8>,
) -> Result<usize, String> {
    line.clear();
    loop {
        let (consumed, terminated) = {
            let available = input
                .fill_buf()
                .map_err(|error| format!("read LSP header: {error}"))?;
            if available.is_empty() {
                return Ok(line.len());
            }
            let terminated_at = available.iter().position(|byte| *byte == b'\n');
            let consumed = terminated_at.map_or(available.len(), |index| index + 1);
            if line.len().saturating_add(consumed) > MAX_LSP_HEADER_LINE_BYTES {
                return Err(format!(
                    "LSP header line exceeds the {MAX_LSP_HEADER_LINE_BYTES}-byte limit"
                ));
            }
            line.extend_from_slice(&available[..consumed]);
            (consumed, terminated_at.is_some())
        };
        input.consume(consumed);
        if terminated {
            return Ok(line.len());
        }
    }
}

fn read_lsp_message(input: &mut impl BufRead) -> Result<Option<norito::json::Value>, String> {
    let mut content_length = None;
    let mut line = Vec::new();
    for _ in 0..MAX_LSP_HEADERS {
        let read = read_bounded_lsp_header_line(input, &mut line)?;
        if read == 0 {
            return if content_length.is_none() {
                Ok(None)
            } else {
                Err("unexpected EOF before the LSP header terminator".to_owned())
            };
        }
        let line =
            std::str::from_utf8(&line).map_err(|_| "LSP headers must be valid UTF-8".to_owned())?;
        let header = line.trim_end_matches(['\r', '\n']);
        if header.is_empty() {
            break;
        }
        let (name, raw) = header
            .split_once(':')
            .ok_or_else(|| "malformed LSP header; expected `name: value`".to_owned())?;
        if name.eq_ignore_ascii_case("Content-Length") {
            if content_length.is_some() {
                return Err("duplicate LSP Content-Length header".to_owned());
            }
            content_length = Some(
                raw.trim()
                    .parse::<usize>()
                    .map_err(|_| "invalid LSP Content-Length".to_owned())?,
            );
        }
    }
    if !line.ends_with(b"\n") || !line.iter().all(|byte| matches!(byte, b'\r' | b'\n')) {
        return Err(format!(
            "LSP request exceeds the {MAX_LSP_HEADERS}-header limit"
        ));
    }
    let length = content_length.ok_or_else(|| "missing LSP Content-Length".to_owned())?;
    if length > MAX_LSP_MESSAGE_BYTES {
        return Err(format!(
            "LSP message exceeds the {MAX_LSP_MESSAGE_BYTES}-byte limit"
        ));
    }
    let mut body = vec![0_u8; length];
    input
        .read_exact(&mut body)
        .map_err(|error| format!("read LSP message: {error}"))?;
    norito::json::from_slice(&body)
        .map(Some)
        .map_err(|error| format!("decode LSP JSON: {error}"))
}

fn write_lsp_message(output: &mut impl Write, message: &norito::json::Value) -> Result<(), String> {
    let body =
        norito::json::to_string(message).map_err(|error| format!("encode LSP JSON: {error}"))?;
    write!(output, "Content-Length: {}\r\n\r\n{body}", body.len())
        .map_err(|error| format!("write LSP message: {error}"))?;
    output
        .flush()
        .map_err(|error| format!("flush LSP message: {error}"))
}

fn write_lsp_response(
    output: &mut impl Write,
    id: Option<norito::json::Value>,
    result: norito::json::Value,
) -> Result<(), String> {
    write_lsp_message(
        output,
        &json_object(vec![
            ("jsonrpc", norito::json::Value::from("2.0")),
            ("id", id.unwrap_or(norito::json::Value::Null)),
            ("result", result),
        ]),
    )
}

fn write_lsp_error(
    output: &mut impl Write,
    id: Option<norito::json::Value>,
    code: i64,
    message: &str,
) -> Result<(), String> {
    write_lsp_message(
        output,
        &json_object(vec![
            ("jsonrpc", norito::json::Value::from("2.0")),
            ("id", id.unwrap_or(norito::json::Value::Null)),
            (
                "error",
                json_object(vec![
                    ("code", norito::json::Value::from(code)),
                    ("message", norito::json::Value::from(message)),
                ]),
            ),
        ]),
    )
}

fn publish_lsp_notification(
    output: &mut impl Write,
    method: &str,
    params: norito::json::Value,
) -> Result<(), String> {
    write_lsp_message(
        output,
        &json_object(vec![
            ("jsonrpc", norito::json::Value::from("2.0")),
            ("method", norito::json::Value::from(method)),
            ("params", params),
        ]),
    )
}

fn publish_lsp_diagnostics(
    output: &mut impl Write,
    session: &CompilerSession,
    uri: &str,
    source: &str,
) -> Result<(), String> {
    let diagnostics = lsp_diagnostics(session, uri, source);
    publish_lsp_notification(
        output,
        "textDocument/publishDiagnostics",
        json_object(vec![
            ("uri", norito::json::Value::from(uri)),
            ("diagnostics", norito::json::Value::Array(diagnostics)),
        ]),
    )
}

fn lsp_initialize_result() -> norito::json::Value {
    json_object(vec![(
        "capabilities",
        json_object(vec![
            ("textDocumentSync", norito::json::Value::from(1_u64)),
            (
                "completionProvider",
                json_object(vec![("resolveProvider", norito::json::Value::from(false))]),
            ),
            (
                "documentFormattingProvider",
                norito::json::Value::from(true),
            ),
            ("codeActionProvider", norito::json::Value::from(true)),
        ]),
    )])
}

fn collect_lsp_diagnostics(session: &CompilerSession, uri: &str, source: &str) -> DiagnosticBundle {
    // LSP validates reusable modules as well as deployable contracts. Calling
    // `build` here would add the artifact-only K4003 error to every valid
    // module document and perform unnecessary code generation while typing.
    match session.check_with_lints(CompileRequest {
        source,
        source_name: Some(uri),
    }) {
        Ok(warnings) => DiagnosticBundle::new(
            warnings
                .into_iter()
                .map(|warning| lint_diagnostic(warning, Path::new(uri)))
                .collect(),
        ),
        Err(bundle) => bundle,
    }
}

fn lsp_diagnostics(session: &CompilerSession, uri: &str, source: &str) -> Vec<norito::json::Value> {
    collect_lsp_diagnostics(session, uri, source)
        .diagnostics
        .iter()
        .map(|diagnostic| lsp_diagnostic_value(diagnostic, source))
        .collect()
}

fn lsp_diagnostic_value(diagnostic: &Diagnostic, source: &str) -> norito::json::Value {
    let range = diagnostic.primary_span.as_ref().map_or_else(
        || lsp_range(0, 0, 0, 1),
        |span| lsp_source_span_range(source, span),
    );
    json_object(vec![
        ("range", range),
        ("code", norito::json::Value::from(diagnostic.code.clone())),
        (
            "severity",
            norito::json::Value::from(match diagnostic.severity {
                ivm::kotodama::diagnostic::Severity::Error => 1_u64,
                ivm::kotodama::diagnostic::Severity::Warning => 2_u64,
            }),
        ),
        ("source", norito::json::Value::from("kotodama")),
        (
            "message",
            norito::json::Value::from(diagnostic.message.clone()),
        ),
    ])
}

fn lsp_code_action_items(
    session: &CompilerSession,
    uri: &str,
    source: &str,
) -> norito::json::Value {
    let actions = collect_lsp_diagnostics(session, uri, source)
        .diagnostics
        .into_iter()
        .filter_map(|diagnostic| {
            let fix = diagnostic.fix.as_ref()?;
            let byte_range = fix.span.byte_range?;
            let start = usize::try_from(byte_range.start).ok()?;
            let end = usize::try_from(byte_range.end).ok()?;
            if start > end
                || end > source.len()
                || !source.is_char_boundary(start)
                || !source.is_char_boundary(end)
            {
                return None;
            }
            let edit = json_object(vec![
                ("range", lsp_text_range(source, byte_range)),
                (
                    "newText",
                    norito::json::Value::from(fix.replacement.clone()),
                ),
            ]);
            let changes =
                norito::json::object([(uri.to_owned(), norito::json::Value::Array(vec![edit]))])
                    .ok()?;
            Some(json_object(vec![
                (
                    "title",
                    norito::json::Value::from(format!(
                        "Fix {}: {}",
                        diagnostic.code, diagnostic.message
                    )),
                ),
                ("kind", norito::json::Value::from("quickfix")),
                ("isPreferred", norito::json::Value::from(true)),
                (
                    "diagnostics",
                    norito::json::Value::Array(vec![lsp_diagnostic_value(&diagnostic, source)]),
                ),
                ("edit", json_object(vec![("changes", changes)])),
            ]))
        })
        .collect();
    norito::json::Value::Array(actions)
}

fn lsp_source_span_range(source: &str, span: &SourceSpan) -> norito::json::Value {
    span.byte_range.map_or_else(
        || {
            lsp_range(
                span.start.line.saturating_sub(1) as u64,
                span.start.column.saturating_sub(1) as u64,
                span.end.line.saturating_sub(1) as u64,
                span.end.column.saturating_sub(1) as u64,
            )
        },
        |range| lsp_text_range(source, range),
    )
}

fn lsp_text_range(source: &str, range: ivm::kotodama::source::TextRange) -> norito::json::Value {
    let (start_line, start_character) = lsp_offset_position(source, range.start);
    let (end_line, end_character) = lsp_offset_position(source, range.end);
    lsp_range(start_line, start_character, end_line, end_character)
}

fn lsp_offset_position(source: &str, offset: u32) -> (u64, u64) {
    let offset = usize::try_from(offset)
        .unwrap_or(source.len())
        .min(source.len());
    let offset = if source.is_char_boundary(offset) {
        offset
    } else {
        let mut boundary = offset;
        while !source.is_char_boundary(boundary) {
            boundary = boundary.saturating_sub(1);
        }
        boundary
    };
    let prefix = &source[..offset];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() as u64;
    let line_start = prefix.rfind('\n').map_or(0, |index| index + 1);
    let character = prefix[line_start..].encode_utf16().count() as u64;
    (line, character)
}

fn lsp_range(
    start_line: u64,
    start_character: u64,
    end_line: u64,
    end_character: u64,
) -> norito::json::Value {
    json_object(vec![
        ("start", lsp_position(start_line, start_character)),
        ("end", lsp_position(end_line, end_character)),
    ])
}

fn lsp_completion_items() -> norito::json::Value {
    let mut labels = BTreeSet::new();
    let mut items = Vec::new();
    let mut push = |label: &'static str, kind: u64| {
        if labels.insert(label) {
            items.push(json_object(vec![
                ("label", norito::json::Value::from(label)),
                ("kind", norito::json::Value::from(kind)),
            ]));
        }
    };
    for &keyword in V1_KEYWORDS {
        push(keyword, 14);
    }
    for &operator in V1_OPERATORS {
        push(operator, 24);
    }
    for &ty in V1_SOURCE_TYPE_NAMES {
        push(ty, 7);
    }
    for &path in V1_SUM_PATHS {
        push(path, 3);
    }
    for &path in V1_ROUNDING_PATHS {
        push(path, 20);
    }
    for &member in V1_LIST_MEMBER_NAMES {
        push(member, 2);
    }
    for &(label, kind) in V1_CONTEXTUAL_COMPLETIONS {
        push(label, kind);
    }
    for &builtin in Builtin::ALL {
        let spec = builtin.spec();
        match spec.surface {
            BuiltinSurface::Function => push(spec.name, 3),
            BuiltinSurface::MethodOnly => push(builtin.name(), 2),
            BuiltinSurface::FunctionOrMethod => {
                push(spec.name, 3);
                push(builtin.name(), 2);
            }
            BuiltinSurface::CompilerInternal => continue,
        }
    }
    norito::json::Value::Array(items)
}

fn lsp_position(line: impl Into<u64>, character: impl Into<u64>) -> norito::json::Value {
    json_object(vec![
        ("line", norito::json::Value::from(line.into())),
        ("character", norito::json::Value::from(character.into())),
    ])
}

fn json_object(entries: Vec<(&str, norito::json::Value)>) -> norito::json::Value {
    norito::json::object(
        entries
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value)),
    )
    .unwrap_or(norito::json::Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_inventory_is_exact_and_retired_names_stay_rejected() {
        let inventory = KOTO_COMMAND_INVENTORY
            .iter()
            .map(|(name, _)| *name)
            .collect::<Vec<_>>();
        assert_eq!(
            inventory,
            ["check", "build", "test", "fmt", "doc", "explain", "lsp"]
        );

        let usage_inventory = USAGE
            .lines()
            .filter_map(|line| line.strip_prefix("  koto "))
            .filter_map(|line| line.split_ascii_whitespace().next())
            .collect::<Vec<_>>();
        assert_eq!(usage_inventory, inventory);

        for retired in ["compile", "lint", "koto_compile", "koto_lint", "koto_test"] {
            assert_eq!(koto_command(retired), None, "retired command `{retired}`");
        }
    }

    #[test]
    fn layout_normalization_is_idempotent() {
        let formatted =
            format_source_text("seiyaku Demo {   \n}\n\n", None).expect("format valid source");
        assert_eq!(formatted, "seiyaku Demo {}\n");
        assert_eq!(
            format_source_text(&formatted, None).expect("reformat valid source"),
            formatted
        );
    }

    #[test]
    fn contract_documentation_is_stable_markdown_from_the_manifest() {
        let output = CompilerSession::default()
            .build(CompileRequest {
                source: r#"
                    seiyaku Vault {
                        error enum VaultError { Empty = 7 }
                        state int balance;
                        hajimari() { balance = 0; }
                        kotoage fn deposit(int amount) authorize("CanDeposit") {
                            require(amount > 0, VaultError::Empty);
                            balance = balance + amount;
                        }
                        view fn read() -> int { return balance; }
                    }
                "#,
                source_name: Some("vault.ko"),
            })
            .expect("compile documentation fixture");
        let markdown = render_contract_documentation(&output.manifest);
        for expected in [
            "# Vault",
            "### `deposit(int amount)`",
            "Kind: `kotoage`/`言挙げ`",
            "Authorization: `CanDeposit`",
            "## Durable state",
            "`int` `balance`",
            "## Seiyaku errors",
            "`VaultError::Empty` = `7`",
        ] {
            assert!(
                markdown.contains(expected),
                "generated documentation omitted {expected:?}:\n{markdown}"
            );
        }
    }

    #[test]
    fn formatter_validation_uses_lossless_v1_syntax() {
        format_source_text(
            "seiyaku Demo { view fn value() -> int { return 1; } }",
            Some("valid.ko"),
        )
        .expect("valid syntax");
        let branded = format_source_text(
            "誓約 Demo { 言挙げ fn run() authorize(\"Run\") {} }",
            Some("branded.ko"),
        )
        .expect("branded Japanese keywords are valid V1 syntax");
        assert_eq!(
            branded, "誓約 Demo { 言挙げ fn run() authorize(\"Run\") {} }\n",
            "formatting must preserve the selected branded script",
        );
        let error = format_source_text(
            "seiyaku Démo { view fn value() -> int { return ; } }",
            Some("invalid.ko"),
        )
        .expect_err("invalid source must not be formatted");
        assert!(error.contains("K0100"));
        assert!(error.contains("invalid.ko"));
    }

    #[test]
    fn check_options_select_zk_policy_without_source_metadata() {
        let (format, zk_enabled, inputs) = parse_format_and_inputs(vec![
            "--format".to_owned(),
            "sarif".to_owned(),
            "--zk".to_owned(),
            "proof.ko".to_owned(),
        ])
        .expect("parse check options");
        assert_eq!(format, DiagnosticFormat::Sarif);
        assert!(zk_enabled);
        assert_eq!(inputs, vec![PathBuf::from("proof.ko")]);
    }

    #[test]
    fn unreadable_sources_emit_native_structured_diagnostics() {
        let missing = std::env::temp_dir().join(format!(
            "koto-missing-source-{}-{}.ko",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos()
        ));
        let session = CompilerSession::default();

        for diagnostics in [
            compile_path(&session, &missing).expect_err("missing build source"),
            check_path(&session, &missing).expect_err("missing check source"),
        ] {
            let diagnostic = diagnostics
                .diagnostics
                .first()
                .expect("one read diagnostic");
            assert_eq!(diagnostic.code, "K0000");
            assert_eq!(diagnostic.phase, DiagnosticPhase::Lex);
            assert!(diagnostic.primary_span.is_none());
            assert!(diagnostic.message.contains(&missing.display().to_string()));
            assert!(!diagnostic.message.starts_with("K0000:"));
        }
    }

    #[test]
    fn check_batch_has_one_equivalent_json_and_sarif_diagnostic_set() {
        let root = std::env::temp_dir().join(format!(
            "koto-check-batch-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).expect("create check batch root");
        let first = root.join("first.ko");
        let second = root.join("second.ko");
        std::fs::write(&first, "seiyaku First { € }").expect("write first invalid source");
        std::fs::write(&second, "seiyaku Second { £ }").expect("write second invalid source");

        let (checked, diagnostics) = check_paths(
            &CompilerSession::default(),
            vec![first.clone(), second.clone()],
        );
        assert!(checked.is_empty());
        assert!(diagnostics.diagnostics.len() >= 2);
        let sources = diagnostics
            .diagnostics
            .iter()
            .filter_map(|diagnostic| {
                diagnostic
                    .primary_span
                    .as_ref()
                    .and_then(|span| span.source.as_deref())
            })
            .collect::<std::collections::BTreeSet<_>>();
        assert!(sources.contains(first.to_str().expect("UTF-8 first path")));
        assert!(sources.contains(second.to_str().expect("UTF-8 second path")));

        let json: norito::json::Value =
            norito::json::from_str(&DiagnosticFormat::Json.render(&diagnostics))
                .expect("batch JSON is one document");
        let sarif: norito::json::Value =
            norito::json::from_str(&DiagnosticFormat::Sarif.render(&diagnostics))
                .expect("batch SARIF is one document");
        assert_eq!(
            json.as_array().map(Vec::len),
            sarif
                .pointer("/runs/0/results")
                .and_then(norito::json::Value::as_array)
                .map(Vec::len),
        );
        std::fs::remove_dir_all(root).expect("remove check batch root");
    }

    #[test]
    fn unified_check_surfaces_lints_as_non_fatal_structured_warnings() {
        let root = std::env::temp_dir().join(format!(
            "koto-check-lint-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).expect("create lint check root");
        let source = root.join("lint.ko");
        std::fs::write(
            &source,
            "seiyaku Lint { view fn value(int unused) -> int { return 1; } }",
        )
        .expect("write lint source");

        let warnings = check_path(&CompilerSession::default(), &source)
            .expect("lint warning must not fail semantic checking");
        let warning = warnings
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == "K5003")
            .expect("unused parameter warning");
        assert_eq!(warning.severity, Severity::Warning);
        assert!(warning.primary_span.is_some());
        assert!(
            ivm::kotodama::diagnostic::diagnostic_explanation("K5003").is_some(),
            "every unified lint code must work with koto explain",
        );
        std::fs::remove_dir_all(root).expect("remove lint check root");
    }

    #[test]
    fn formatter_options_fail_closed_and_allow_dash_paths_after_separator() {
        let error = parse_format_sources_args(vec!["--write".to_owned(), "demo.ko".to_owned()])
            .expect_err("unknown formatter flags must not become file paths");
        assert!(error.contains("unknown fmt option"));

        let (check_only, inputs) = parse_format_sources_args(vec![
            "--check".to_owned(),
            "--".to_owned(),
            "--literal-name.ko".to_owned(),
        ])
        .expect("separator permits a leading-dash file name");
        assert!(check_only);
        assert_eq!(inputs, vec![PathBuf::from("--literal-name.ko")]);
    }

    #[test]
    fn build_rejects_standalone_module_without_publishing_artifact() {
        let root = std::env::temp_dir().join(format!(
            "koto-module-build-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos()
        ));
        let source = root.join("math.ko");
        let target = root.join("target/kotodama");
        std::fs::create_dir_all(&root).expect("create module test root");
        std::fs::write(
            &source,
            "module Math { fn add(int left, int right) -> int { return left + right; } }",
        )
        .expect("write module source");
        let error = build(vec![
            "--target-dir".to_owned(),
            target.display().to_string(),
            source.display().to_string(),
        ])
        .expect_err("standalone module build must fail");
        assert!(error.contains("K4003"), "unexpected error: {error}");
        assert!(!target.join("dev/math.to").exists());

        let session = CompilerSession::default();
        check_path(&session, &source).expect("module remains valid for koto check");
        std::fs::remove_dir_all(root).expect("remove module test root");
    }

    #[test]
    fn lsp_framing_and_completion_use_canonical_syntax_tables() {
        let body = br#"{"jsonrpc":"2.0","id":1,"method":"initialize"}"#;
        let framed = format!(
            "Content-Length: {}\r\n\r\n{}",
            body.len(),
            std::str::from_utf8(body).expect("JSON is UTF-8")
        );
        let mut input = std::io::Cursor::new(framed.into_bytes());
        let message = read_lsp_message(&mut input)
            .expect("read LSP frame")
            .expect("one message");
        assert_eq!(
            message.get("method").and_then(norito::json::Value::as_str),
            Some("initialize")
        );
        assert_eq!(
            lsp_initialize_result()
                .pointer("/capabilities/codeActionProvider")
                .and_then(norito::json::Value::as_bool),
            Some(true),
        );

        let completions = lsp_completion_items();
        let labels = completions
            .as_array()
            .expect("completion array")
            .iter()
            .filter_map(|item| item.get("label").and_then(norito::json::Value::as_str))
            .collect::<Vec<_>>();
        let completion_kind = |label: &str| {
            completions
                .as_array()
                .expect("completion array")
                .iter()
                .find(|item| item.get("label").and_then(norito::json::Value::as_str) == Some(label))
                .and_then(|item| item.get("kind"))
                .and_then(norito::json::Value::as_u64)
        };
        assert!(labels.contains(&"seiyaku"));
        assert!(labels.contains(&"kotoage"));
        assert!(labels.contains(&"hajimari"));
        assert!(labels.contains(&"kaizen"));
        assert!(labels.contains(&"誓約"));
        assert!(labels.contains(&"言挙げ"));
        assert!(labels.contains(&"始まり"));
        assert!(labels.contains(&"改善"));
        assert!(labels.contains(&"&&"));
        assert_eq!(completion_kind("json"), Some(14));
        assert_eq!(completion_kind("div_round"), Some(2));
        for current in V1_SUM_PATHS
            .iter()
            .chain(V1_ROUNDING_PATHS)
            .chain(V1_LIST_MEMBER_NAMES)
            .chain(V1_CONTEXTUAL_COMPLETIONS.iter().map(|(label, _)| label))
        {
            assert!(
                labels.contains(current),
                "missing canonical V1 completion `{current}`"
            );
        }
        for current in [
            "json",
            "int",
            "decimal",
            "quantity",
            "List",
            "AccountView",
            "AssetDefinitionView",
            "QueryPage",
            "Option::some",
            "Result::err",
            "Rounding::nearest_even",
            "div_round",
            "try_push",
            "enumerate",
            "get_int",
            "get_decimal",
            "get_quantity",
            "get_json",
            "get_name",
            "get_account_id",
            "get_asset_definition_id",
            "get_nft_id",
            "get_blob_hex",
            "ledger::query::account",
            "ledger::query::asset",
            "ledger::query::asset_definition",
            "ledger::query::domain",
            "ledger::query::nft",
            "ledger::query::accounts",
            "ledger::query::assets",
            "ledger::query::asset_definitions",
            "ledger::query::domains",
            "ledger::query::nfts",
        ] {
            assert!(
                labels.contains(&current),
                "missing V1 completion `{current}`"
            );
        }
        assert_eq!(
            labels.iter().copied().collect::<BTreeSet<_>>().len(),
            labels.len(),
            "completion labels must be stable and duplicate-free",
        );
        for retired in [
            "contract",
            "entry",
            "init",
            "upgrade",
            "json!",
            "option::some",
            "option::none",
            "result::ok",
            "result::err",
            "Amount",
            "get_amount",
            "get_numeric",
            "json_get_int",
            "json_get_numeric",
        ] {
            assert!(!labels.contains(&retired));
        }
    }

    #[test]
    fn lsp_quick_fixes_are_exact_current_document_workspace_edits() {
        let session = CompilerSession::default();
        let uri = "file:///workspace/fixes.ko";

        let mixed =
            "seiyaku C { fn target(int first, int second) {} fn f() { target(1, second: 2); } }";
        let mixed_actions = lsp_code_action_items(&session, uri, mixed);
        let mixed_action = mixed_actions
            .as_array()
            .expect("code action array")
            .iter()
            .find(|action| {
                action
                    .pointer("/diagnostics/0/code")
                    .and_then(norito::json::Value::as_str)
                    == Some("E_MIXED_CALL_ARGUMENTS")
            })
            .expect("mixed-call quick fix");
        assert_eq!(
            mixed_action
                .pointer("/kind")
                .and_then(norito::json::Value::as_str),
            Some("quickfix")
        );
        let mixed_edit = mixed_action
            .pointer("/edit/changes")
            .and_then(|changes| changes.get(uri))
            .and_then(norito::json::Value::as_array)
            .and_then(|edits| edits.first())
            .expect("mixed-call workspace edit");
        assert_eq!(
            mixed_edit
                .get("newText")
                .and_then(norito::json::Value::as_str),
            Some("first: 1")
        );
        let start = mixed_edit
            .pointer("/range/start/character")
            .and_then(norito::json::Value::as_u64)
            .expect("mixed edit start") as usize;
        let end = mixed_edit
            .pointer("/range/end/character")
            .and_then(norito::json::Value::as_u64)
            .expect("mixed edit end") as usize;
        assert_eq!(&mixed[start..end], "1");

        let unresolved = "seiyaku C { fn f() { target(1, second: 2); } }";
        let unresolved_diagnostics = collect_lsp_diagnostics(&session, uri, unresolved);
        assert!(unresolved_diagnostics.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "E_MIXED_CALL_ARGUMENTS" && diagnostic.fix.is_none()
        }));
        let unresolved_actions = lsp_code_action_items(&session, uri, unresolved);
        assert!(
            unresolved_actions
                .as_array()
                .expect("code action array")
                .iter()
                .all(|action| {
                    action
                        .pointer("/diagnostics/0/code")
                        .and_then(norito::json::Value::as_str)
                        != Some("E_MIXED_CALL_ARGUMENTS")
                }),
            "an unresolved call must not receive a guessed parameter-name edit"
        );

        let positional =
            "seiyaku C { struct Pair { int left, int right } fn f() { let pair = Pair(1, 2); } }";
        let positional_actions = lsp_code_action_items(&session, uri, positional);
        let positional_action = positional_actions
            .as_array()
            .expect("code action array")
            .iter()
            .find(|action| {
                action
                    .pointer("/diagnostics/0/code")
                    .and_then(norito::json::Value::as_str)
                    == Some("E_POSITIONAL_STRUCT")
            })
            .expect("positional-struct quick fix");
        let positional_edit = positional_action
            .pointer("/edit/changes")
            .and_then(|changes| changes.get(uri))
            .and_then(norito::json::Value::as_array)
            .and_then(|edits| edits.first())
            .expect("positional-struct workspace edit");
        assert_eq!(
            positional_edit
                .get("newText")
                .and_then(norito::json::Value::as_str),
            Some("Pair { left: 1, right: 2, }")
        );
    }

    #[test]
    fn lsp_check_accepts_reusable_modules_without_artifact_codegen() {
        let session = CompilerSession::default();
        let module = lsp_diagnostics(
            &session,
            "file:///workspace/math.ko",
            "module Math { fn value() -> int { return 1; } }",
        );
        assert!(
            module.is_empty(),
            "valid reusable modules must not receive deployable-only K4003: {module:?}",
        );

        let invalid = lsp_diagnostics(
            &session,
            "file:///workspace/broken.ko",
            "module Broken { fn value( -> int { return 1; } }",
        );
        assert!(!invalid.is_empty());
    }

    #[test]
    fn lsp_document_store_is_bounded_and_removes_rejected_updates() {
        let mut documents = HashMap::new();
        for index in 0..MAX_LSP_OPEN_DOCUMENTS {
            store_lsp_document(
                &mut documents,
                &format!("file:///workspace/{index}.ko"),
                "module M {}",
            )
            .expect("document below count limit");
        }
        let error = store_lsp_document(
            &mut documents,
            "file:///workspace/overflow.ko",
            "module Overflow {}",
        )
        .expect_err("document count must be bounded");
        assert!(error.contains("workspace limit"));
        assert!(!documents.contains_key("file:///workspace/overflow.ko"));

        let huge_uri = format!("file:///{}", "u".repeat(MAX_LSP_URI_BYTES));
        let error = store_lsp_document(&mut documents, &huge_uri, "module Uri {}")
            .expect_err("document URI must be bounded");
        assert!(error.contains("document URI exceeds"));

        let existing = "file:///workspace/0.ko";
        let oversized = "x".repeat(MAX_SOURCE_BYTES + 1);
        let error = store_lsp_document(&mut documents, existing, &oversized)
            .expect_err("oversized changed document must fail");
        assert!(error.contains("V1 source limit"));
        assert!(
            !documents.contains_key(existing),
            "a rejected update must not leave stale source available to formatting",
        );
    }

    #[test]
    fn lsp_framing_rejects_oversized_and_ambiguous_inputs_before_allocation() {
        let oversized = format!("Content-Length: {}\r\n\r\n", MAX_LSP_MESSAGE_BYTES + 1);
        let error = read_lsp_message(&mut std::io::Cursor::new(oversized.into_bytes()))
            .expect_err("oversized LSP frame must fail");
        assert!(error.contains("exceeds"), "unexpected error: {error}");

        let duplicate = b"Content-Length: 2\r\nContent-Length: 2\r\n\r\n{}";
        let error = read_lsp_message(&mut std::io::Cursor::new(duplicate))
            .expect_err("duplicate length must fail");
        assert!(error.contains("duplicate"), "unexpected error: {error}");

        let mixed_case_duplicate = b"content-length: 2\r\nCONTENT-LENGTH: 2\r\n\r\n{}";
        let error = read_lsp_message(&mut std::io::Cursor::new(mixed_case_duplicate))
            .expect_err("header names are case-insensitive");
        assert!(error.contains("duplicate"), "unexpected error: {error}");

        let lowercase = b"content-length: 2\r\n\r\n{}";
        read_lsp_message(&mut std::io::Cursor::new(lowercase))
            .expect("lowercase header is valid")
            .expect("one lowercase-header message");

        let malformed = b"Content-Length 2\r\n\r\n{}";
        let error = read_lsp_message(&mut std::io::Cursor::new(malformed))
            .expect_err("malformed header must fail closed");
        assert!(error.contains("malformed"), "unexpected error: {error}");

        let long_header = format!("{}\n", "x".repeat(MAX_LSP_HEADER_LINE_BYTES + 1));
        let error = read_lsp_message(&mut std::io::Cursor::new(long_header.into_bytes()))
            .expect_err("oversized header line must fail");
        assert!(
            error.contains("header line exceeds"),
            "unexpected error: {error}"
        );
    }
}
