//! Unified Kotodama V1 developer command.
//!
//! Kotodama compiles to IVM bytecode (`.to`). `koto` checks, builds, tests, formats, documents and
//! explains Kotodama sources, and serves the language server.
#[path = "koto/editor_lsp.rs"]
mod editor_lsp;
#[path = "koto/explain.rs"]
mod explain;
#[path = "koto/lsp_transport.rs"]
mod lsp_transport;
use clap::{Args, CommandFactory, FromArgMatches, Parser, Subcommand, ValueEnum};
use kotodama_lang::{
    compiler::CompilerOptions,
    diagnostic::{
        Diagnostic, DiagnosticBundle, DiagnosticPhase, Severity, SourcePosition, SourceSpan,
    },
    driver::{
        BuildDriver, BuildError, BuildStatus, LinkedSourceBuildRequest, LoadedSourceProject,
        ProjectSourceKey, PublishLayout, PublishMode, atomic_write_if_changed,
        discover_source_link_request, load_source_project, load_source_project_manifest,
        logical_source_name, project_root_for_source, read_source_file,
    },
    formatter::format_source,
    linker::SourceModuleUnit,
    lint::{LintLevel, LintWarning},
    session::{CompilerSession, LintConfig},
    source::{FrontendBudget, MAX_SOURCE_BYTES, SourceFile, SourceId},
};
#[cfg(test)]
use kotodama_lang::{
    diagnostic::{DiagnosticFix, DiagnosticLabel},
    lexer::{V1_KEYWORDS, V1_OPERATORS},
    session::{CompileOutput, CompileRequest},
    source::TextRange,
};
#[cfg(test)]
use kotodama_surface::{
    builtins::{Builtin, BuiltinSurface},
    source_policy::{V1_LIST_MEMBER_NAMES, V1_ROUNDING_PATHS, V1_SOURCE_TYPE_NAMES, V1_SUM_PATHS},
};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    env,
    io::{BufRead, Write},
    path::{Path, PathBuf},
};
/// Process exit statuses shared with `musubi` (see `crates/musubi/src/output.rs`).
///
/// Scripts can distinguish a misused command line from failing code: usage errors never reuse
/// the status reserved for diagnostics and failing tests.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ExitStatus {
    /// The command completed and every requested check passed.
    Success,
    /// The command line was malformed: unknown option, missing or conflicting arguments.
    Usage,
    /// Compiler diagnostics, formatting drift, or a `--verify` mismatch.
    Failed,
    /// The tests compiled and ran, and at least one failed.
    TestsFailed,
    /// A source, manifest, or output file could not be read or written.
    Io,
    /// The toolchain itself failed; report it with a minimal reproducer.
    Internal,
}
impl ExitStatus {
    /// Stable numeric process exit code.
    const fn code(self) -> i32 {
        match self {
            Self::Success => 0,
            Self::Usage => 2,
            Self::Failed => 8,
            Self::Io => 10,
            Self::TestsFailed => 11,
            Self::Internal => 70,
        }
    }
}
/// Exit-status table appended to `koto --help`.
const EXIT_STATUS_HELP: &str = "\
Exit status:
  0   success
  2   usage error (unknown option, missing or conflicting arguments)
  8   compiler diagnostics, formatting drift, or --verify mismatch
  10  a source, manifest, or output file could not be read or written
  11  the tests ran and at least one failed
  70  internal toolchain error";
/// Short version printed by `koto -V`.
const KOTO_VERSION: &str = env!("CARGO_PKG_VERSION");
/// Long version printed by `koto --version`: compiler identity, bytecode target, and ABI hash.
fn koto_long_version() -> &'static str {
    static LONG_VERSION: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    LONG_VERSION.get_or_init(|| {
        let abi_hash = ivm_abi::syscalls::compute_abi_hash(ivm_abi::SyscallPolicy::AbiV1);
        format!(
            "{KOTO_VERSION}\ncompiler: kotodama_lang/{KOTO_VERSION}\ntarget: IVM 1.1 bytecode (.to), ABI v1\nabi_hash: {}",
            hex_lower(&abi_hash)
        )
    })
}
fn hex_lower(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut out, byte| {
        let _ = write!(out, "{byte:02x}");
        out
    })
}
/// Kotodama V1 toolchain: check, build, test, format, document, and explain seiyaku compiled to
/// IVM bytecode.
#[derive(Parser, Debug)]
#[command(
    name = "koto",
    version = KOTO_VERSION,
    about = "Kotodama V1 toolchain: check, build, test, format, and document seiyaku compiled to IVM bytecode",
    after_help = EXIT_STATUS_HELP,
    propagate_version = true
)]
struct Cli {
    #[command(subcommand)]
    command: KotoCommand,
}
#[derive(Subcommand, Debug)]
enum KotoCommand {
    /// Type-check sources and report diagnostics without writing artifacts.
    Check(CheckArgs),
    /// Compile seiyaku into IVM `.to` artifacts and their interface manifests.
    Build(BuildArgs),
    /// Discover and run `#[test]` functions against the seiyaku under test.
    Test(TestArgs),
    /// Format sources in place, or report unformatted files with --check.
    Fmt(FmtArgs),
    /// Render the public interface of a seiyaku as Markdown or JSON.
    Doc(DocArgs),
    /// Explain a diagnostic code, lint name, or branded keyword.
    Explain(ExplainArgs),
    /// Serve the Kotodama language server over stdio.
    Lsp(LspArgs),
}
/// Explicit source-graph selection shared by every compiling subcommand.
#[derive(Args, Debug, Default, Clone)]
struct SourceSelection {
    /// Compile the exact locked graph declared by this project manifest instead of positional
    /// sources.
    #[arg(long, value_name = "kotodama.project.json")]
    project: Option<PathBuf>,
    /// Directory that logical source names are relative to (default: the source's directory;
    /// for a `koto_test` module, the nearest directory containing both it and its target).
    #[arg(long, value_name = "DIR", conflicts_with = "project")]
    source_root: Option<PathBuf>,
}
/// Compiler capabilities shared by every compiling subcommand.
#[derive(Args, Debug, Clone, Copy)]
struct CompileCapabilities {
    /// Account-address chain discriminant (1..=65535) of the network the sources target:
    /// `AccountId` literals must be encoded for it, and `koto test` derives fixture actor
    /// accounts from it. Pass the target network's value when its literals use another prefix.
    #[arg(
        long,
        value_name = "N",
        value_parser = parse_chain_discriminant,
        default_value_t = iroha_data_model::account::address::chain_discriminant()
    )]
    chain_discriminant: u16,
    /// Enable ZK seiyaku compilation (`Secret<T>` and proof/commitment operations).
    #[arg(long)]
    zk: bool,
}
impl CompileCapabilities {
    fn chain_discriminant(self) -> u16 {
        self.chain_discriminant
    }
}
#[derive(Args, Debug)]
struct CheckArgs {
    /// Diagnostic output format.
    #[arg(long, value_enum, default_value_t)]
    format: DiagnosticFormat,
    #[command(flatten)]
    lints: LintArgs,
    #[command(flatten)]
    capabilities: CompileCapabilities,
    #[command(flatten)]
    selection: SourceSelection,
    /// Sources to check. Each file is an independent root unless --project is given; a
    /// `koto_test` module is checked in test mode against its target.
    #[arg(
        value_name = "SOURCE",
        required_unless_present = "project",
        conflicts_with = "project"
    )]
    sources: Vec<PathBuf>,
}
/// Lint levels selected on the command line, layered over the project manifest's `lints`.
#[derive(Args, Debug, Default, Clone)]
struct LintArgs {
    /// Fail the check when any lint would warn (every `warn` lint becomes `deny`).
    #[arg(long)]
    deny_warnings: bool,
    /// Do not report the lint with this name, such as `unused-local` (repeatable).
    #[arg(long = "allow", value_name = "LINT")]
    allow: Vec<String>,
    /// Report the lint with this name as a warning (repeatable).
    #[arg(long = "warn", value_name = "LINT")]
    warn: Vec<String>,
    /// Report the lint with this name as an error that fails the check (repeatable).
    #[arg(long = "deny", value_name = "LINT")]
    deny: Vec<String>,
}
impl LintArgs {
    /// Validate the flags: every name must be a registered lint, and one lint gets one level.
    fn config(&self) -> Result<LintConfig, KotoError> {
        let mut config = LintConfig::new();
        let mut selected = BTreeMap::<&str, LintLevel>::new();
        for (names, level) in [
            (&self.allow, LintLevel::Allow),
            (&self.warn, LintLevel::Warn),
            (&self.deny, LintLevel::Deny),
        ] {
            for name in names {
                if let Some(previous) = selected.insert(name, level)
                    && previous != level
                {
                    return Err(KotoError::Usage(format!(
                        "lint `{name}` is given both `--{}` and `--{}`",
                        previous.as_str(),
                        level.as_str()
                    )));
                }
                config
                    .set_level(name, level)
                    .map_err(|unknown| KotoError::Usage(unknown.to_string()))?;
            }
        }
        config.set_deny_warnings(self.deny_warnings);
        Ok(config)
    }
}
/// Apply the effective lint level to one finding: `None` when it is allowed, otherwise the
/// finding with its severity (`deny` reports an error that fails the check).
fn leveled_lint(config: &LintConfig, warning: LintWarning) -> Option<LintWarning> {
    match config.level(warning.code) {
        LintLevel::Allow => None,
        level => Some(warning.with_level(level)),
    }
}
#[derive(Args, Debug)]
struct BuildArgs {
    /// Diagnostic output format.
    #[arg(long, value_enum, default_value_t)]
    format: DiagnosticFormat,
    /// Build profile; selects the output directory `<target-dir>/<profile>`.
    #[arg(long, value_name = "NAME", default_value = "dev")]
    profile: String,
    /// Root directory for build outputs.
    #[arg(long, value_name = "DIR", default_value = "target/kotodama")]
    target_dir: PathBuf,
    /// Write the `.to` artifact to this exact path (one source only).
    #[arg(long, value_name = "FILE.to")]
    out: Option<PathBuf>,
    /// Write the manifest to this path, or `-` for stdout (one source only).
    #[arg(long, value_name = "FILE.json")]
    manifest_out: Option<PathBuf>,
    /// Cycle ceiling recorded in the artifact header; must not exceed node admission policy.
    #[arg(long, value_name = "COUNT", value_parser = clap::value_parser!(u64).range(1..))]
    max_cycles: Option<u64>,
    /// Verify that existing outputs match a fresh build without writing anything.
    #[arg(long)]
    verify: bool,
    #[command(flatten)]
    capabilities: CompileCapabilities,
    #[command(flatten)]
    selection: SourceSelection,
    /// Seiyaku sources to build. Each produces its own artifact unless --project is given.
    #[arg(
        value_name = "SOURCE",
        required_unless_present = "project",
        conflicts_with = "project"
    )]
    sources: Vec<PathBuf>,
}
/// `koto test` with an optional action; a bare `koto test <SOURCE>` runs the suite.
#[derive(Args, Debug)]
#[command(args_conflicts_with_subcommands = true, subcommand_negates_reqs = true)]
struct TestArgs {
    #[command(subcommand)]
    action: Option<TestAction>,
    #[command(flatten)]
    run: TestRunArgs,
}
#[derive(Subcommand, Debug)]
enum TestAction {
    /// Compile and run the selected tests (the default action).
    Run(TestRunArgs),
    /// List the discovered tests without compiling them.
    List(TestListArgs),
    /// Run the selected tests and report which seiyaku functions executed.
    Coverage(TestCoverageArgs),
    /// Run the selected tests and print a per-instruction execution trace.
    Trace(TestTraceArgs),
}
/// Suite discovery and selection shared by every `koto test` action.
#[derive(Args, Debug, Clone)]
struct TestSuiteArgs {
    /// Run only tests whose name contains this text (or equals it with --exact).
    #[arg(long, value_name = "TEXT")]
    filter: Option<String>,
    /// Require --filter to match the complete test name.
    #[arg(long, requires = "filter")]
    exact: bool,
    /// Deterministic ordering seed; 0 runs tests in name order.
    #[arg(long, value_name = "N", default_value_t = 0)]
    seed: u64,
    #[command(flatten)]
    capabilities: CompileCapabilities,
    #[command(flatten)]
    selection: SourceSelection,
    /// A seiyaku with inline tests, or a `*.test.ko` module declaring `koto_test { target: ... }`.
    #[arg(value_name = "SOURCE", required_unless_present = "project")]
    source: Option<PathBuf>,
}
#[derive(Args, Debug, Clone)]
struct TestRunArgs {
    /// Report format written to stdout.
    #[arg(long, value_enum, default_value_t)]
    format: TestRunFormat,
    /// Also write a JUnit XML report to this file.
    #[arg(long, value_name = "FILE.xml")]
    junit: Option<PathBuf>,
    /// Print a per-kotoage gas table (calls, min, mean, max) after the results.
    #[arg(long)]
    gas_report: bool,
    /// Number of tests executed in parallel.
    #[arg(long, short = 'j', value_name = "N", default_value_t = 1, value_parser = clap::value_parser!(u64).range(1..=256))]
    jobs: u64,
    #[command(flatten)]
    suite: TestSuiteArgs,
}
#[derive(Args, Debug, Clone)]
struct TestListArgs {
    /// Listing format.
    #[arg(long, value_enum, default_value_t)]
    format: TestListFormat,
    #[command(flatten)]
    suite: TestSuiteArgs,
}
#[derive(Args, Debug, Clone)]
struct TestCoverageArgs {
    /// Number of tests executed in parallel.
    #[arg(long, short = 'j', value_name = "N", default_value_t = 1, value_parser = clap::value_parser!(u64).range(1..=256))]
    jobs: u64,
    #[command(flatten)]
    suite: TestSuiteArgs,
}
#[derive(Args, Debug, Clone)]
struct TestTraceArgs {
    /// Trace format: readable steps, or one JSON object per executed instruction.
    #[arg(long, value_enum, default_value_t)]
    format: TestListFormat,
    #[command(flatten)]
    suite: TestSuiteArgs,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
enum TestRunFormat {
    #[default]
    Human,
    Json,
    Junit,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
enum TestListFormat {
    #[default]
    Human,
    Json,
}
#[derive(Args, Debug)]
struct FmtArgs {
    /// Report files that need formatting without rewriting them (exit status 8 if any).
    #[arg(long)]
    check: bool,
    /// Files or directories; directories are searched recursively for `*.ko` files, skipping
    /// hidden and `target` directories. Defaults to the current directory.
    #[arg(value_name = "PATH")]
    paths: Vec<PathBuf>,
}
#[derive(Args, Debug)]
struct DocArgs {
    /// Documentation format.
    #[arg(long, value_enum, default_value_t)]
    format: DocFormat,
    #[command(flatten)]
    capabilities: CompileCapabilities,
    #[command(flatten)]
    selection: SourceSelection,
    /// Seiyaku source to document.
    #[arg(
        value_name = "SOURCE",
        required_unless_present = "project",
        conflicts_with = "project"
    )]
    source: Option<PathBuf>,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
enum DocFormat {
    #[default]
    Markdown,
    Json,
}
#[derive(Args, Debug)]
struct ExplainArgs {
    /// Diagnostic code (`K2003`, `E_TEST_ONLY_PRODUCTION`), lint name (`unused-parameter`), or
    /// branded keyword in either spelling (`kotoage`, `言挙げ`).
    #[arg(value_name = "TOPIC", required_unless_present = "list")]
    topic: Option<String>,
    /// List every diagnostic code, lint name, and branded keyword.
    #[arg(long, conflicts_with = "topic")]
    list: bool,
    /// Output format; `markdown` renders a reference page with one anchor per code.
    #[arg(long, value_enum, default_value_t)]
    format: explain::ExplainFormat,
}
#[derive(Args, Debug)]
struct LspArgs {
    /// Enable ZK seiyaku compilation (`Secret<T>` and proof/commitment operations).
    #[arg(long)]
    zk: bool,
    #[command(flatten)]
    selection: SourceSelection,
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
#[cfg(test)]
const V1_CONTEXTUAL_COMPLETIONS: &[(&str, u64)] = &[("json", 14), ("div_round", 2)];
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
enum DiagnosticFormat {
    #[default]
    Human,
    Json,
    Sarif,
}
impl DiagnosticFormat {
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
/// A failed `koto` command and the exit status it selects.
#[derive(Debug)]
enum KotoError {
    /// Malformed command line detected after parsing (for example a conflicting combination).
    Usage(String),
    /// A file could not be read or written.
    Io(String),
    /// Code was rejected or tests failed; the details were already rendered or are in the text.
    Failed(String),
    /// Structured compiler diagnostics rendered in the requested format.
    Diagnostics {
        format: DiagnosticFormat,
        diagnostics: DiagnosticBundle,
    },
    /// The toolchain itself failed.
    Internal(String),
    /// Failing tests whose report was already printed.
    TestsFailed,
    /// A complete rendered failure report (for example compiler diagnostics), printed as is.
    Rendered(String),
}
impl KotoError {
    fn exit_status(&self) -> ExitStatus {
        match self {
            Self::Usage(_) => ExitStatus::Usage,
            Self::Io(_) => ExitStatus::Io,
            Self::Failed(_) | Self::Diagnostics { .. } | Self::Rendered(_) => ExitStatus::Failed,
            Self::TestsFailed => ExitStatus::TestsFailed,
            Self::Internal(_) => ExitStatus::Internal,
        }
    }
}
impl std::fmt::Display for KotoError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Usage(message)
            | Self::Io(message)
            | Self::Failed(message)
            | Self::Internal(message) => formatter.write_str(message),
            Self::Diagnostics {
                format,
                diagnostics,
            } => formatter.write_str(&format.render(diagnostics)),
            Self::TestsFailed => Ok(()),
            Self::Rendered(report) => formatter.write_str(report),
        }
    }
}
fn build_error(format: DiagnosticFormat, error: BuildError) -> KotoError {
    match error {
        BuildError::Io { .. } => KotoError::Io(error.to_string()),
        BuildError::InvalidProfile(_)
        | BuildError::InvalidStem(_)
        | BuildError::InvalidPath { .. }
        | BuildError::OutputCollision { .. } => KotoError::Usage(error.to_string()),
        BuildError::Render(_) | BuildError::Internal(_) => KotoError::Internal(error.to_string()),
        error => match error.into_diagnostics() {
            Ok(diagnostics) => KotoError::Diagnostics {
                format,
                diagnostics,
            },
            Err(error) => KotoError::Failed(error.to_string()),
        },
    }
}
impl From<kotodama_toolchain::koto_test_driver::KotoTestCliError> for KotoError {
    fn from(error: kotodama_toolchain::koto_test_driver::KotoTestCliError) -> Self {
        use kotodama_toolchain::koto_test_driver::KotoTestCliErrorKind as Kind;
        match error.kind {
            Kind::Usage => Self::Usage(error.message),
            Kind::Io => Self::Io(error.message),
            Kind::Compile => Self::Rendered(error.message),
            Kind::TestsFailed => Self::TestsFailed,
            Kind::Internal => Self::Internal(error.message),
        }
    }
}
fn main() {
    std::process::exit(run_process(env::args_os()).code());
}
/// Parse a full process argument vector, run the command, and report its exit status.
fn run_process<I, T>(args: I) -> ExitStatus
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    let command = Cli::command().long_version(koto_long_version());
    let cli = match command.try_get_matches_from(args) {
        Ok(matches) => match Cli::from_arg_matches(&matches) {
            Ok(cli) => cli,
            Err(error) => return report_clap_error(&error),
        },
        Err(error) => return report_clap_error(&error),
    };
    match run(cli) {
        Ok(()) => ExitStatus::Success,
        Err(error) => {
            let status = error.exit_status();
            match error {
                KotoError::Diagnostics {
                    format,
                    diagnostics,
                } => eprintln!("{}", format.render(&diagnostics)),
                // Failing tests already printed their report.
                KotoError::TestsFailed => {}
                KotoError::Rendered(report) => eprintln!("{report}"),
                other => eprintln!("error: {other}"),
            }
            status
        }
    }
}
fn report_clap_error(error: &clap::Error) -> ExitStatus {
    use clap::error::ErrorKind;
    let _ = error.print();
    match error.kind() {
        ErrorKind::DisplayHelp
        | ErrorKind::DisplayVersion
        | ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand => {
            if error.kind() == ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand {
                ExitStatus::Usage
            } else {
                ExitStatus::Success
            }
        }
        _ => ExitStatus::Usage,
    }
}
fn run(cli: Cli) -> Result<(), KotoError> {
    match cli.command {
        KotoCommand::Check(args) => check(args),
        KotoCommand::Build(args) => build(args),
        KotoCommand::Test(args) => run_tests(args),
        KotoCommand::Fmt(args) => format_sources(args),
        KotoCommand::Doc(args) => document(args),
        KotoCommand::Explain(args) => explain::run(&args.topic, args.list, args.format),
        KotoCommand::Lsp(args) => language_server(args),
    }
}
/// Translate the parsed `koto test` command line into the runner's option record.
fn test_cli_options(args: TestArgs) -> kotodama_toolchain::koto_test_driver::KotoTestCliOptions {
    use kotodama_toolchain::koto_test_driver::{
        KotoTestAction, KotoTestCliOptions, KotoTestReportFormat,
    };
    let suite_options = |action: KotoTestAction, suite: TestSuiteArgs| {
        let TestSuiteArgs {
            filter,
            exact,
            seed,
            capabilities,
            selection,
            source,
        } = suite;
        let mut options = KotoTestCliOptions::new(action, capabilities.chain_discriminant());
        options.source = source;
        options.source_root = selection.source_root;
        options.project = selection.project;
        options.filter = filter;
        options.exact = exact;
        options.seed = seed;
        options.zk_enabled = capabilities.zk;
        options
    };
    let run_options = |args: TestRunArgs| {
        let mut options = suite_options(KotoTestAction::Run, args.suite);
        options.format = match args.format {
            TestRunFormat::Human => KotoTestReportFormat::Human,
            TestRunFormat::Json => KotoTestReportFormat::Json,
            TestRunFormat::Junit => KotoTestReportFormat::Junit,
        };
        options.junit = args.junit;
        options.gas_report = args.gas_report;
        options.jobs = usize::try_from(args.jobs).unwrap_or(1);
        options
    };
    let list_format = |format: TestListFormat| match format {
        TestListFormat::Human => KotoTestReportFormat::Human,
        TestListFormat::Json => KotoTestReportFormat::Json,
    };
    match args.action {
        None => run_options(args.run),
        Some(TestAction::Run(run)) => run_options(run),
        Some(TestAction::List(list)) => {
            let mut options = suite_options(KotoTestAction::List, list.suite);
            options.format = list_format(list.format);
            options
        }
        Some(TestAction::Coverage(coverage)) => {
            let mut options = suite_options(KotoTestAction::Coverage, coverage.suite);
            options.jobs = usize::try_from(coverage.jobs).unwrap_or(1);
            options
        }
        Some(TestAction::Trace(trace)) => {
            let mut options = suite_options(KotoTestAction::Trace, trace.suite);
            options.format = list_format(trace.format);
            options
        }
    }
}
fn run_tests(args: TestArgs) -> Result<(), KotoError> {
    kotodama_toolchain::koto_test_driver::run_cli(test_cli_options(args)).map_err(KotoError::from)
}
fn source_root_for_input(input: &Path, explicit: Option<&Path>) -> Result<PathBuf, String> {
    if let Some(root) = explicit {
        return root
            .canonicalize()
            .map_err(|error| format!("resolve source root `{}`: {error}", root.display()));
    }
    input
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .canonicalize()
        .map_err(|error| format!("resolve source root: {error}"))
}
fn check(args: CheckArgs) -> Result<(), KotoError> {
    let CheckArgs {
        format,
        lints,
        capabilities,
        selection,
        sources,
    } = args;
    let lint_flags = lints.config()?;
    // A source that cannot be opened is an I/O failure (exit status 10), as in every other
    // subcommand, not a diagnostic about Kotodama code.
    if let Some((path, error)) = sources.iter().find_map(|path| {
        std::fs::File::open(path)
            .err()
            .map(|error| (path.as_path(), error))
    }) {
        return Err(KotoError::Io(format!(
            "cannot read `{}`: {error}",
            path.display()
        )));
    }
    let chain_discriminant = capabilities.chain_discriminant();
    let session = CompilerSession::new(CompilerOptions {
        force_zk: capabilities.zk,
        chain_discriminant,
        ..CompilerOptions::default()
    });
    let driver = BuildDriver::new(session, "koto-check");
    let (test_modules, sources): (Vec<_>, Vec<_>) = sources
        .into_iter()
        .partition(|path| is_test_module_path(path));
    let (mut checked, mut diagnostics) = match selection.project {
        Some(manifest) => check_locked_project(&driver, &manifest, &lint_flags),
        None if sources.is_empty() => (Vec::new(), DiagnosticBundle::new(Vec::new())),
        None => check_project_paths_with_root(
            &driver,
            sources,
            selection.source_root.as_deref(),
            &lint_flags,
        ),
    };
    let mut test_targets = Vec::new();
    for module in test_modules {
        match kotodama_toolchain::koto_test_driver::check_test_module_v1(
            &module,
            selection.source_root.as_deref(),
            chain_discriminant,
            capabilities.zk,
        ) {
            Ok(target) => {
                test_targets.push((module.clone(), target));
                checked.push(module);
            }
            Err(kotodama_toolchain::koto_test_driver::KotoTestCheckErrorV1::Diagnostics(
                bundle,
            )) => diagnostics.diagnostics.extend(bundle.diagnostics),
            Err(kotodama_toolchain::koto_test_driver::KotoTestCheckErrorV1::Other(error)) => {
                return Err(KotoError::from(error));
            }
        }
    }
    if format == DiagnosticFormat::Human {
        for path in &checked {
            match test_targets.iter().find(|(module, _)| module == path) {
                Some((_, target)) => println!(
                    "checked {} (test module for {}; run `koto test {}`)",
                    display_path(path),
                    display_path(target),
                    display_path(path)
                ),
                None => println!("checked {}", display_path(path)),
            }
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
        Err(KotoError::Failed(
            "one or more sources failed validation".to_owned(),
        ))
    } else {
        Ok(())
    }
}
/// Display a path relative to the working directory when it lies inside it.
fn display_path(path: &Path) -> String {
    let relative = std::env::current_dir().ok().and_then(|cwd| {
        let cwd = cwd.canonicalize().unwrap_or(cwd);
        let absolute = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        absolute.strip_prefix(&cwd).ok().map(Path::to_path_buf)
    });
    relative
        .filter(|relative| !relative.as_os_str().is_empty())
        .unwrap_or_else(|| path.to_path_buf())
        .display()
        .to_string()
}
/// Whether `path` parses as a standalone test module (`module` with `koto_test { target: ... }`).
///
/// Unreadable or unparsable files are not test modules here; the ordinary check path reports
/// their diagnostics.
fn is_test_module_path(path: &Path) -> bool {
    read_source_file(path).is_ok_and(|source| {
        kotodama_lang::parser::parse(&source).is_ok_and(|program| program.test_target.is_some())
    })
}
/// Check the exact graph of a project manifest with its `lints` levels, overridden by `lint_flags`.
fn check_locked_project(
    driver: &BuildDriver,
    manifest: &Path,
    lint_flags: &LintConfig,
) -> (Vec<PathBuf>, DiagnosticBundle) {
    let loaded = match load_source_project_manifest(manifest) {
        Ok(loaded) => loaded,
        Err(error) => {
            let diagnostics = error.into_diagnostics().unwrap_or_else(|error| {
                DiagnosticBundle::single(Diagnostic::error(
                    "K0000",
                    DiagnosticPhase::Lex,
                    error.to_string(),
                    None,
                ))
            });
            return (Vec::new(), diagnostics);
        }
    };
    check_loaded_project(driver, loaded, lint_flags)
}
fn check_loaded_project(
    driver: &BuildDriver,
    loaded: LoadedSourceProject,
    lint_flags: &LintConfig,
) -> (Vec<PathBuf>, DiagnosticBundle) {
    let lint_config = loaded.lints.merged_with(lint_flags);
    let source_paths = loaded.source_paths;
    let checked_graph = if kotodama_lang::parser::parse(&loaded.graph.root.source)
        .is_ok_and(|program| program.unit.kind == kotodama_lang::ast::SourceUnitKind::Module)
        && loaded.graph.imports.is_empty()
        && loaded.graph.packages.is_empty()
    {
        driver.check_module_sources(loaded.graph.root, loaded.graph.sources)
    } else {
        driver.check_project(loaded.graph)
    };
    match checked_graph {
        Ok(warnings) => {
            let checked = source_paths
                .values()
                .cloned()
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            let diagnostics = warnings
                .into_iter()
                .filter_map(|warning| {
                    let lint = leveled_lint(&lint_config, warning.warning)?;
                    let key = ProjectSourceKey {
                        package_identity: warning.package_identity.clone(),
                        source_name: warning.source_name.clone(),
                    };
                    let path = source_paths
                        .get(&key)
                        .map_or_else(|| Path::new(&warning.source_name), PathBuf::as_path);
                    Some(lint.to_diagnostic(
                        &display_path(path),
                        warning.package_identity.as_deref(),
                        kotodama_lang::i18n::detect_language(),
                    ))
                })
                .collect();
            (checked, DiagnosticBundle::new(diagnostics))
        }
        Err(error) => {
            let mut bundle = error.into_diagnostics().unwrap_or_else(|error| {
                DiagnosticBundle::single(Diagnostic::error(
                    "K0000",
                    DiagnosticPhase::Lex,
                    error.to_string(),
                    None,
                ))
            });
            for diagnostic in &mut bundle.diagnostics {
                remap_locked_project_diagnostic_sources(diagnostic, &source_paths);
            }
            (Vec::new(), bundle)
        }
    }
}
#[cfg(test)]
fn check_project_paths(
    driver: &BuildDriver,
    inputs: Vec<PathBuf>,
) -> (Vec<PathBuf>, DiagnosticBundle) {
    check_project_paths_with_root(driver, inputs, None, &LintConfig::default())
}
fn check_project_paths_with_root(
    driver: &BuildDriver,
    inputs: Vec<PathBuf>,
    explicit_root: Option<&Path>,
    lint_config: &LintConfig,
) -> (Vec<PathBuf>, DiagnosticBundle) {
    if let [input] = inputs.as_slice() {
        let root = source_root_for_input(input, explicit_root);
        let loaded = root
            .clone()
            .map_err(|error| BuildError::InvalidPath {
                path: input.clone(),
                message: error,
            })
            .and_then(|root| load_source_project(input, &root, &BTreeMap::new()));
        match loaded {
            Ok(loaded) => {
                return check_loaded_project(driver, loaded, lint_config);
            }
            Err(error) => {
                let mut diagnostics = error.into_diagnostics().unwrap_or_else(|error| {
                    DiagnosticBundle::single(Diagnostic::error(
                        "K0000",
                        DiagnosticPhase::Lex,
                        error.to_string(),
                        None,
                    ))
                });
                // Loading parses the source and its companions; name them by the same
                // working-directory-relative paths as semantic diagnostics.
                if let Ok(root) = root {
                    for diagnostic in &mut diagnostics.diagnostics {
                        remap_rooted_diagnostic_sources(diagnostic, &root);
                    }
                }
                return (Vec::new(), diagnostics);
            }
        }
    }
    let preferred_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut checked = Vec::new();
    let mut diagnostics = Vec::new();
    let mut sources = Vec::new();
    let mut source_paths = HashMap::<String, String>::new();
    let mut project_inputs = Vec::new();
    for path in inputs {
        let source = match read_source_file(&path) {
            Ok(source) => source,
            Err(error) => {
                diagnostics.push(Diagnostic::error(
                    "K0000",
                    DiagnosticPhase::Lex,
                    format!("failed to read source `{}`: {error}", path.display()),
                    None,
                ));
                continue;
            }
        };
        let project_root = match project_root_for_source(&path, &preferred_root) {
            Ok(root) => root,
            Err(error) => {
                diagnostics.push(Diagnostic::error(
                    "K0000",
                    DiagnosticPhase::Lex,
                    error.to_string(),
                    None,
                ));
                continue;
            }
        };
        let canonical_path = match path.canonicalize() {
            Ok(path) => path,
            Err(error) => {
                diagnostics.push(Diagnostic::error(
                    "K0000",
                    DiagnosticPhase::Lex,
                    format!(
                        "failed to canonicalize source `{}` after reading it: {error}",
                        path.display()
                    ),
                    None,
                ));
                continue;
            }
        };
        let source_name = match logical_source_name(&canonical_path, &project_root) {
            Ok(source_name) => source_name,
            Err(error) => {
                diagnostics.push(Diagnostic::error(
                    "K0000",
                    DiagnosticPhase::Lex,
                    error.to_string(),
                    None,
                ));
                continue;
            }
        };
        let display_path = path.display().to_string();
        if let Some(first) = source_paths.get(&source_name) {
            diagnostics.push(Diagnostic::error(
                "K0000",
                DiagnosticPhase::Lex,
                format!(
                    "explicit sources `{first}` and `{display_path}` have the same logical project path `{source_name}`"
                ),
                None,
            ));
            continue;
        }
        source_paths.insert(source_name.clone(), display_path);
        sources.push(SourceModuleUnit {
            source_name,
            source,
        });
        project_inputs.push(path);
    }
    if !sources.is_empty() {
        match driver.check_explicit_sources(sources) {
            Ok(warnings) => {
                checked.extend(project_inputs);
                diagnostics.extend(warnings.into_iter().filter_map(|warning| {
                    let lint = leveled_lint(lint_config, warning.warning)?;
                    let path = source_paths
                        .get(&warning.source_name)
                        .map(String::as_str)
                        .unwrap_or(warning.source_name.as_str());
                    Some(lint_diagnostic(lint, Path::new(path)))
                }));
            }
            Err(error) => match error.into_diagnostics() {
                Ok(mut bundle) => {
                    for diagnostic in &mut bundle.diagnostics {
                        remap_project_diagnostic_sources(diagnostic, &source_paths);
                    }
                    diagnostics.extend(bundle.diagnostics);
                }
                Err(error) => diagnostics.push(Diagnostic::error(
                    "K0000",
                    DiagnosticPhase::Lex,
                    error.to_string(),
                    None,
                )),
            },
        }
    }
    (checked, DiagnosticBundle::new(diagnostics))
}
#[cfg(test)]
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
fn build(args: BuildArgs) -> Result<(), KotoError> {
    let BuildArgs {
        format: diagnostic_format,
        profile,
        target_dir,
        out: explicit_output,
        manifest_out: explicit_manifest_output,
        max_cycles,
        verify,
        capabilities,
        selection,
        sources: inputs,
    } = args;
    let publish_mode = if verify {
        PublishMode::Verify
    } else {
        PublishMode::Write
    };
    let project_manifest = selection.project;
    let source_root = selection.source_root;
    let build_count = if project_manifest.is_some() {
        1
    } else {
        inputs.len()
    };
    if explicit_output.is_some() && build_count != 1 {
        return Err(KotoError::Usage(
            "--out can be used only when building one source".to_owned(),
        ));
    }
    if explicit_manifest_output.is_some() && build_count != 1 {
        return Err(KotoError::Usage(
            "--manifest-out can be used only when building one source".to_owned(),
        ));
    }
    let mut compiler_options = CompilerOptions::default();
    if let Some(max_cycles) = max_cycles {
        compiler_options.max_cycles = max_cycles;
    }
    compiler_options.chain_discriminant = capabilities.chain_discriminant();
    compiler_options.force_zk = capabilities.zk;
    let session = CompilerSession::new(compiler_options);
    let driver = BuildDriver::for_current_executable(session)
        .map_err(|error| build_error(diagnostic_format, error))?;
    let manifest_stdout = explicit_manifest_output.as_deref() == Some(Path::new("-"));
    let projects = if let Some(manifest) = project_manifest.as_ref() {
        let loaded = load_source_project_manifest(manifest)
            .map_err(|error| build_error(diagnostic_format, error))?;
        let source_name = loaded.graph.root.source_name.clone();
        let stem = Path::new(&source_name)
            .file_stem()
            .and_then(|stem| stem.to_str())
            .ok_or_else(|| KotoError::Usage(format!("{source_name} has no UTF-8 file stem")))?
            .to_owned();
        vec![(stem, source_name, loaded.graph)]
    } else {
        let mut projects = Vec::with_capacity(inputs.len());
        for input in &inputs {
            let stem = input
                .file_stem()
                .and_then(|stem| stem.to_str())
                .ok_or_else(|| {
                    KotoError::Usage(format!("{} has no UTF-8 file stem", input.display()))
                })?
                .to_owned();
            let project_root =
                source_root_for_input(input, source_root.as_deref()).map_err(KotoError::Io)?;
            let graph = discover_source_link_request(input, &project_root, Vec::new(), Vec::new())
                .map_err(|error| build_error(diagnostic_format, error))?;
            let source_name = graph.root.source_name.clone();
            projects.push((stem, source_name, graph));
        }
        projects
    };
    let mut requests = Vec::with_capacity(projects.len());
    for (stem, source_name, graph) in projects {
        let mut layout = if let Some(output) = explicit_output.as_ref() {
            PublishLayout::for_artifact(output.clone(), None, None)
        } else {
            PublishLayout::standard(&target_dir, &profile, &stem, false)
        }
        .map_err(|error| build_error(diagnostic_format, error))?;
        if let Some(manifest) = explicit_manifest_output
            .as_ref()
            .filter(|path| path.as_path() != Path::new("-"))
        {
            layout.manifest = manifest.clone();
        }
        if manifest_stdout {
            layout = layout.with_sidecar_manifest();
        }
        requests.push(LinkedSourceBuildRequest {
            graph,
            source_name,
            profile: profile.clone(),
            layout,
            mode: publish_mode,
        });
    }
    let outcomes = driver
        .build_project_batch(requests)
        .map_err(|error| build_error(diagnostic_format, error))?;
    for outcome in outcomes {
        let notice = match outcome.status {
            BuildStatus::Fresh => "fresh",
            BuildStatus::Built => "built",
        };
        if manifest_stdout {
            eprintln!("{notice} {}", outcome.paths.artifact.display());
            println!(
                "{}",
                norito::json::to_json_pretty(&outcome.manifest).map_err(|error| {
                    KotoError::Internal(format!("render contract manifest: {error}"))
                })?
            );
        } else {
            println!("{notice} {}", outcome.paths.artifact.display());
        }
    }
    Ok(())
}
fn format_sources(args: FmtArgs) -> Result<(), KotoError> {
    let FmtArgs { check, paths } = args;
    let paths = if paths.is_empty() {
        vec![PathBuf::from(".")]
    } else {
        paths
    };
    let inputs = collect_format_inputs(&paths)?;
    if inputs.is_empty() {
        return Err(KotoError::Usage(format!(
            "no `*.ko` sources found under {}",
            paths
                .iter()
                .map(|path| format!("`{}`", path.display()))
                .collect::<Vec<_>>()
                .join(", ")
        )));
    }
    let mut changed = false;
    let mut invalid = false;
    for path in inputs {
        let source = read_source_file(&path).map_err(|error| KotoError::Io(error.to_string()))?;
        let formatted = match format_source_text(&source, path.to_str()) {
            Ok(formatted) => formatted,
            Err(diagnostics) => {
                // Keep formatting the remaining files; one invalid file must not hide drift in
                // the others.
                eprintln!("{diagnostics}");
                invalid = true;
                continue;
            }
        };
        if formatted != source {
            changed = true;
            if check {
                println!("would format {}", display_path(&path));
            } else {
                atomic_write_if_changed(&path, formatted.as_bytes())
                    .map_err(|error| KotoError::Io(error.to_string()))?;
                println!("formatted {}", display_path(&path));
            }
        }
    }
    if invalid {
        Err(KotoError::Failed(
            "one or more sources are not valid Kotodama and were left unchanged".to_owned(),
        ))
    } else if check && changed {
        Err(KotoError::Failed(
            "one or more sources require formatting; run `koto fmt` to rewrite them".to_owned(),
        ))
    } else {
        Ok(())
    }
}
/// Expand `koto fmt` operands: files are kept as given and directories are searched recursively
/// for `*.ko` sources in sorted order, skipping hidden directories, `target`, and symbolic links.
fn collect_format_inputs(paths: &[PathBuf]) -> Result<Vec<PathBuf>, KotoError> {
    let mut inputs = BTreeSet::new();
    for path in paths {
        let metadata = std::fs::metadata(path)
            .map_err(|error| KotoError::Io(format!("read `{}`: {error}", path.display())))?;
        if metadata.is_dir() {
            collect_ko_sources(path, &mut inputs)?;
        } else {
            inputs.insert(path.clone());
        }
    }
    Ok(inputs.into_iter().collect())
}
fn collect_ko_sources(directory: &Path, out: &mut BTreeSet<PathBuf>) -> Result<(), KotoError> {
    let entries = std::fs::read_dir(directory)
        .map_err(|error| KotoError::Io(format!("read `{}`: {error}", directory.display())))?;
    for entry in entries {
        let entry = entry
            .map_err(|error| KotoError::Io(format!("read `{}`: {error}", directory.display())))?;
        let file_type = entry
            .file_type()
            .map_err(|error| KotoError::Io(format!("read `{}`: {error}", directory.display())))?;
        let path = entry.path();
        let hidden_or_output = entry
            .file_name()
            .to_str()
            .is_none_or(|name| name.starts_with('.') || name == "target");
        if file_type.is_dir() {
            if !hidden_or_output {
                collect_ko_sources(&path, out)?;
            }
        } else if file_type.is_file() && path.extension().is_some_and(|extension| extension == "ko")
        {
            out.insert(path);
        }
    }
    Ok(())
}
fn format_source_text(source: &str, source_name: Option<&str>) -> Result<String, String> {
    let file = SourceFile::new(SourceId(0), source_name.unwrap_or("<source>"), source);
    format_source(&file, FrontendBudget::v1()).map_err(|diagnostics| diagnostics.render_human())
}
fn document(args: DocArgs) -> Result<(), KotoError> {
    let DocArgs {
        format,
        capabilities,
        selection,
        source,
    } = args;
    let session = CompilerSession::new(CompilerOptions {
        force_zk: capabilities.zk,
        chain_discriminant: capabilities.chain_discriminant(),
        ..CompilerOptions::default()
    });
    let graph = if let Some(manifest) = selection.project {
        load_source_project_manifest(&manifest)
            .map_err(|error| build_error(DiagnosticFormat::Human, error))?
            .graph
    } else {
        let path = source
            .ok_or_else(|| KotoError::Usage("doc expects a .ko source or --project".to_owned()))?;
        let project_root = source_root_for_input(&path, selection.source_root.as_deref())
            .map_err(KotoError::Io)?;
        discover_source_link_request(&path, &project_root, Vec::new(), Vec::new())
            .map_err(|error| build_error(DiagnosticFormat::Human, error))?
    };
    let source_name = graph.root.source_name.clone();
    let root_source = graph.root.source.clone();
    let analysis = kotodama_lang::editor::EditorSnapshot::project(&graph, capabilities.zk);
    let source_id = analysis
        .sources()
        .find(|source| source.name() == source_name)
        .map(SourceFile::id)
        .ok_or_else(|| {
            KotoError::Internal(
                "documentation source is absent from the explicit project graph".to_owned(),
            )
        })?;
    let source_signatures = analysis.unit_declaration_signatures(source_id);
    let driver = BuildDriver::new(session, "koto-doc");
    let output = driver
        .compile_project(graph, &source_name)
        .map_err(|error| build_error(DiagnosticFormat::Human, error))?;
    let rendered = match format {
        DocFormat::Json => norito::json::to_json_pretty(
            &contract_documentation_json(&output.manifest, &source_signatures)
                .map_err(KotoError::Internal)?,
        )
        .map_err(|error| KotoError::Internal(format!("render contract interface: {error}")))?,
        DocFormat::Markdown => render_contract_documentation(
            &output.manifest,
            &DocumentationContext::new(&source_signatures, Some(&root_source)),
        ),
    };
    println!("{rendered}");
    Ok(())
}
fn markdown_inline(text: &str) -> String {
    text.replace(['\n', '\r'], " ").replace('`', "\\`")
}
fn contract_documentation_json(
    manifest: &iroha_data_model::smart_contract::manifest::ContractManifest,
    signatures: &[kotodama_lang::editor::EditorSignature],
) -> Result<norito::json::Value, String> {
    use norito::json::{Value, object};
    let signatures = signatures
        .iter()
        .map(|signature| {
            let parameters = signature
                .parameters
                .iter()
                .map(|parameter| {
                    object([
                        ("name", Value::from(parameter.name.clone())),
                        ("type", Value::from(parameter.ty.clone())),
                        (
                            "call_mode",
                            Value::from(if parameter.named {
                                "named"
                            } else {
                                "positional"
                            }),
                        ),
                    ])
                })
                .collect::<Result<Vec<_>, _>>()?;
            object([
                ("name", Value::from(signature.name.clone())),
                ("parameters", Value::Array(parameters)),
                ("return_type", Value::from(signature.return_type.clone())),
            ])
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    object([
        (
            "manifest",
            norito::json::to_value(manifest).map_err(|error| error.to_string())?,
        ),
        ("source_signatures", Value::Array(signatures)),
    ])
    .map_err(|error| error.to_string())
}
/// Inputs to `koto doc` beyond the manifest: the source-form signatures and the keyword spelling
/// written at each declaration.
struct DocumentationContext<'a> {
    signatures: &'a [kotodama_lang::editor::EditorSignature],
    spellings: BTreeMap<String, String>,
}
impl<'a> DocumentationContext<'a> {
    fn new(signatures: &'a [kotodama_lang::editor::EditorSignature], source: Option<&str>) -> Self {
        Self {
            signatures,
            spellings: source.map(declared_keyword_spellings).unwrap_or_default(),
        }
    }
}
/// Map each kotoage, hajimari and kaizen declaration to the keyword spelling written in source,
/// so generated documentation echoes `言挙げ` where the author wrote `言挙げ`.
fn declared_keyword_spellings(source: &str) -> BTreeMap<String, String> {
    use kotodama_lang::lexer::TokenKind;
    let Ok(tokens) = kotodama_lang::lexer::lex(source) else {
        return BTreeMap::new();
    };
    let spelling = |token: &kotodama_lang::lexer::Token| {
        source
            .get(token.range.start as usize..token.range.end as usize)
            .map(ToOwned::to_owned)
    };
    let mut spellings = BTreeMap::new();
    for (index, token) in tokens.iter().enumerate() {
        match token.kind {
            TokenKind::Kotoage => {
                if let (Some(fn_token), Some(name_token)) =
                    (tokens.get(index + 1), tokens.get(index + 2))
                    && fn_token.kind == TokenKind::Fn
                    && let TokenKind::Ident(name) = &name_token.kind
                    && let Some(written) = spelling(token)
                {
                    spellings.insert(name.clone(), written);
                }
            }
            TokenKind::Hajimari | TokenKind::Kaizen => {
                let entry = if token.kind == TokenKind::Hajimari {
                    "hajimari"
                } else {
                    "kaizen"
                };
                if let Some(written) = spelling(token) {
                    spellings.insert(entry.to_owned(), written);
                }
            }
            _ => {}
        }
    }
    spellings
}
/// Canonical JSON example for one argument schema: the exact object `koto test`, Torii and the
/// CLIs accept for this entrypoint. Numbers that would lose precision in JSON (`int`, `decimal`,
/// `quantity`) are decimal strings.
fn argument_example(
    schema: &iroha_data_model::smart_contract::entrypoint::EntrypointArgumentSchemaV1,
) -> String {
    let fields = schema
        .fields
        .iter()
        .map(|field| {
            let mut index = 0;
            format!(
                "{}: {}",
                norito::json::to_string(&norito::json::Value::from(field.name.clone()))
                    .unwrap_or_else(|_| format!("\"{}\"", field.name)),
                value_example(&field.ty.nodes, &mut index)
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("{{{fields}}}")
}
fn value_example(
    nodes: &[iroha_data_model::smart_contract::entrypoint::EntrypointValueTypeNodeV1],
    index: &mut usize,
) -> String {
    use iroha_data_model::smart_contract::entrypoint::{
        EntrypointValueKindV1 as Kind, EntrypointValueTypeNodeV1 as Node,
    };
    let Some(node) = nodes.get(*index) else {
        return "null".to_owned();
    };
    *index += 1;
    match node {
        Node::Struct(structure) => {
            let fields = structure
                .fields
                .iter()
                .map(|field| format!("\"{field}\": {}", value_example(nodes, index)))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{{{fields}}}")
        }
        Node::Tuple(arity) => {
            let items = (0..*arity)
                .map(|_| value_example(nodes, index))
                .collect::<Vec<_>>()
                .join(", ");
            format!("[{items}]")
        }
        Node::Option => {
            skip_value(nodes, index);
            "{\"none\": true}".to_owned()
        }
        Node::Result => {
            let ok = value_example(nodes, index);
            skip_value(nodes, index);
            format!("{{\"ok\": {ok}}}")
        }
        Node::List(_) => {
            skip_value(nodes, index);
            "[]".to_owned()
        }
        Node::Unit => "null".to_owned(),
        Node::Error(error) => error.variants.first().map_or_else(
            || "null".to_owned(),
            |variant| format!("\"{}\"", variant.name),
        ),
        Node::StateCursor(_) => "\"0x…\"".to_owned(),
        Node::Leaf(kind) => match kind {
            Kind::Int | Kind::Quantity => "\"0\"",
            Kind::Decimal => "\"0.0\"",
            Kind::Bool => "false",
            Kind::String => "\"\"",
            Kind::Json => "{}",
            Kind::Name => "\"name\"",
            Kind::AccountId => "\"<account id>\"",
            Kind::AssetDefinitionId => "\"<asset definition id>\"",
            Kind::AssetId => "\"<asset id>\"",
            Kind::DomainId => "\"<domain>.<dataspace>\"",
            Kind::NftId => "\"<nft id>\"",
            Kind::DataSpaceId => "0",
            Kind::Blob => "\"0x\"",
        }
        .to_owned(),
    }
}
/// Encoding rules `koto doc` states for one argument schema, each only when a parameter uses it:
/// numbers that would lose precision in JSON are decimal strings, and options are tagged objects.
fn argument_encoding_notes(
    schema: &iroha_data_model::smart_contract::entrypoint::EntrypointArgumentSchemaV1,
) -> String {
    use iroha_data_model::smart_contract::entrypoint::{
        EntrypointValueKindV1 as Kind, EntrypointValueTypeNodeV1 as Node,
    };
    let nodes = || schema.fields.iter().flat_map(|field| &field.ty.nodes);
    let mut notes = String::new();
    if nodes().any(|node| matches!(node, Node::Leaf(Kind::Int | Kind::Decimal | Kind::Quantity))) {
        notes.push_str(" `int`, `decimal` and `quantity` values are canonical decimal strings.");
    }
    if nodes().any(|node| matches!(node, Node::Option)) {
        notes.push_str(" Options are `{\"some\": value}` or `{\"none\": true}`.");
    }
    notes
}
/// Advance `index` past one complete value subtree.
fn skip_value(
    nodes: &[iroha_data_model::smart_contract::entrypoint::EntrypointValueTypeNodeV1],
    index: &mut usize,
) {
    let _ = value_example(nodes, index);
}
/// Whether an error identity is owned by the compiler rather than declared by the seiyaku.
fn is_compiler_owned_error(identity: &str) -> bool {
    identity.starts_with("kotodama::")
}
fn render_contract_documentation(
    manifest: &iroha_data_model::smart_contract::manifest::ContractManifest,
    context: &DocumentationContext<'_>,
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
    let entrypoints = manifest.entrypoints.as_deref().unwrap_or_default();
    let keyword = |romaji: &str| {
        kotodama_lang::glossary::by_spelling(romaji).map_or_else(
            || format!("`{romaji}`"),
            |keyword| format!("`{}` / `{}`", keyword.romaji, keyword.kanji),
        )
    };
    let sections = [
        (
            EntryPointKind::Kotoage,
            format!("{} (authorized public mutations)", keyword("kotoage")),
        ),
        (EntryPointKind::View, "Views (read-only calls)".to_owned()),
        (
            EntryPointKind::Hajimari,
            format!("Lifecycle: {}", keyword("hajimari")),
        ),
        (
            EntryPointKind::Kaizen,
            format!("Lifecycle: {}", keyword("kaizen")),
        ),
    ];
    for (kind, title) in sections {
        let declared = entrypoints
            .iter()
            .filter(|entrypoint| entrypoint.kind == kind)
            .collect::<Vec<_>>();
        if declared.is_empty() {
            continue;
        }
        let _ = writeln!(output, "\n## {title}");
        for entrypoint in declared {
            render_entrypoint_documentation(&mut output, entrypoint, context);
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
    let error_types = manifest.error_types.as_deref().unwrap_or_default();
    let render_errors = |output: &mut String| {
        for error in error_types
            .iter()
            .filter(|error| !is_compiler_owned_error(&error.identity))
        {
            for variant in &error.variants {
                let _ = writeln!(
                    output,
                    "\n- `{}::{}` = `{}`",
                    markdown_inline(&error.identity),
                    markdown_inline(&variant.name),
                    variant.code
                );
            }
        }
    };
    if error_types
        .iter()
        .any(|error| !is_compiler_owned_error(&error.identity))
    {
        output.push_str("\n## Seiyaku errors\n");
        render_errors(&mut output);
    }
    let compiler_owned = error_types
        .iter()
        .filter(|error| is_compiler_owned_error(&error.identity))
        .collect::<Vec<_>>();
    if !compiler_owned.is_empty() {
        output.push_str(
            "\n## Runtime errors\n\nCompiler-owned errors that checked list and numeric operations can raise in any seiyaku:\n",
        );
        for error in compiler_owned {
            let _ = writeln!(
                output,
                "\n- `{}`: {}",
                markdown_inline(&error.identity),
                error
                    .variants
                    .iter()
                    .map(|variant| format!(
                        "`{}` ({})",
                        markdown_inline(&variant.name),
                        variant.code
                    ))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
    }
    output
}
fn render_entrypoint_documentation(
    output: &mut String,
    entrypoint: &iroha_data_model::smart_contract::manifest::EntrypointDescriptor,
    context: &DocumentationContext<'_>,
) {
    use iroha_data_model::smart_contract::manifest::EntryPointKind;
    use std::fmt::Write as _;
    let written = context.spellings.get(&entrypoint.name).map(String::as_str);
    let _ = writeln!(
        output,
        "\n### `{}`\n",
        markdown_inline(&entrypoint_declaration(entrypoint, context, written))
    );
    let declaration = match entrypoint.kind {
        EntryPointKind::Kotoage => format!(
            "Declared with `{}`: an authorized call that may change durable state and the ledger.",
            written.unwrap_or("kotoage")
        ),
        EntryPointKind::View => "Declared with `view`: a read-only call.".to_owned(),
        EntryPointKind::Hajimari => format!(
            "Declared with `{}`: the one-shot activation hook. Until it runs, every other call and view is rejected.",
            written.unwrap_or("hajimari")
        ),
        EntryPointKind::Kaizen => format!(
            "Declared with `{}`: the migration hook run once after this seiyaku's code is replaced in place.",
            written.unwrap_or("kaizen")
        ),
    };
    let _ = writeln!(output, "{declaration}");
    match entrypoint.permission.as_deref() {
        Some(permission) => {
            let _ = writeln!(output, "Authorization: `{}`", markdown_inline(permission));
        }
        None if matches!(
            entrypoint.kind,
            EntryPointKind::Hajimari | EntryPointKind::Kaizen
        ) =>
        {
            output.push_str(
                "Authorization: the runtime `CanInvokeContractEntrypoint` lifecycle permission\n",
            );
        }
        None => output.push_str("Authorization: public\n"),
    }
    match entrypoint.argument_schema.as_ref() {
        Some(schema) => {
            let _ = writeln!(
                output,
                "Arguments: a JSON object keyed by parameter name, for example `{}`.{}",
                markdown_inline(&argument_example(schema)),
                argument_encoding_notes(schema)
            );
        }
        None if !entrypoint.params.is_empty() => {
            output.push_str("Arguments: a JSON object keyed by parameter name; `int`, `decimal` and `quantity` values are canonical decimal strings.\n");
        }
        None => output.push_str("Arguments: none (send `{}`).\n"),
    }
    let complete = entrypoint.access_hints_complete == Some(true)
        && entrypoint.access_hints_skipped.is_empty();
    if complete {
        output.push_str("Access analysis: complete; the scheduler can run this call in parallel with calls that touch other keys.\n");
    } else {
        output.push_str("Access analysis: conservative; the compiler could not list every key this call touches, so the scheduler serializes it with other calls to this seiyaku. This affects throughput, not correctness.\n");
    }
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
/// Source-syntax declaration header of an entrypoint, as `koto doc` headings show it.
///
/// Uses the editor snapshot's declaration (which keeps the keyword spelling written at the
/// declaration site) and otherwise renders the manifest descriptor through the shared
/// [`kotodama_lang::signature_render`] printer.
fn entrypoint_declaration(
    entrypoint: &iroha_data_model::smart_contract::manifest::EntrypointDescriptor,
    context: &DocumentationContext<'_>,
    written: Option<&str>,
) -> String {
    use iroha_data_model::smart_contract::manifest::EntryPointKind;
    use kotodama_lang::{
        ast::FunctionKind,
        signature_render::{RenderParameter, SourceDeclaration, source_declaration},
    };
    if let Some(declaration) = context
        .signatures
        .iter()
        .find(|signature| signature.name == entrypoint.name)
        .map(|signature| signature.declaration.as_str())
        .filter(|declaration| !declaration.is_empty())
    {
        return declaration.to_owned();
    }
    let parameters = entrypoint
        .params
        .iter()
        .map(|parameter| RenderParameter {
            name: &parameter.name,
            ty: &parameter.type_name,
            named: true,
        })
        .collect::<Vec<_>>();
    source_declaration(&SourceDeclaration {
        kind: match entrypoint.kind {
            EntryPointKind::Kotoage => FunctionKind::Kotoage,
            EntryPointKind::View => FunctionKind::View,
            EntryPointKind::Hajimari => FunctionKind::Hajimari,
            EntryPointKind::Kaizen => FunctionKind::Kaizen,
        },
        keyword: written,
        name: &entrypoint.name,
        parameters: &parameters,
        return_type: entrypoint.return_type.as_deref().unwrap_or("()"),
        permission: entrypoint.permission.as_deref(),
        is_test: false,
        fixture: None,
    })
}
fn parse_chain_discriminant(raw: &str) -> Result<u16, String> {
    if raw.is_empty()
        || (raw.len() > 1 && raw.starts_with('0'))
        || !raw.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(format!(
            "invalid --chain-discriminant value `{raw}`: expected a decimal integer in 1..=65535"
        ));
    }
    let value = raw.parse::<u16>().map_err(|_| {
        format!(
            "invalid --chain-discriminant value `{raw}`: expected a decimal integer in 1..=65535"
        )
    })?;
    if value == 0 {
        return Err("--chain-discriminant must be in 1..=65535".to_owned());
    }
    Ok(value)
}
#[cfg(test)]
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
#[cfg(test)]
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
fn lint_diagnostic(warning: kotodama_lang::lint::LintWarning, path: &Path) -> Diagnostic {
    warning.to_diagnostic(
        &path.display().to_string(),
        None,
        kotodama_lang::i18n::detect_language(),
    )
}
/// Serve the language server over stdio.
///
/// An unreadable project manifest is an I/O failure and an invalid one reports its diagnostics,
/// as in `koto check`; a broken transport stream is an I/O failure.
fn language_server(args: LspArgs) -> Result<(), KotoError> {
    let LspArgs { zk, selection } = args;
    let zk_enabled = zk;
    let source_root = selection.source_root;
    let project_manifest = selection.project;
    let project = project_manifest
        .as_deref()
        .map(load_source_project_manifest)
        .transpose()
        .map_err(|error| build_error(DiagnosticFormat::Human, error))?;
    let inbox = lsp_transport::Inbox::new();
    let reader = inbox.clone();
    let _reader = std::thread::Builder::new()
        .name("koto-lsp-input".to_owned())
        .spawn(move || reader.read_from(&mut std::io::stdin().lock()))
        .map_err(|error| KotoError::Internal(format!("start LSP input reader: {error}")))?;
    let stdout = std::io::stdout();
    let result = language_server_dispatch(
        &inbox,
        &mut stdout.lock(),
        project_manifest.as_deref(),
        project,
        zk_enabled,
        source_root.as_deref(),
    );
    inbox.close();
    result.map_err(KotoError::Io)
}

fn language_server_dispatch(
    inbox: &lsp_transport::Inbox,
    transport_output: &mut impl Write,
    project_manifest: Option<&Path>,
    mut project: Option<LoadedSourceProject>,
    zk_enabled: bool,
    source_root: Option<&Path>,
) -> Result<(), String> {
    let mut documents = HashMap::<String, String>::new();
    let mut versions = HashMap::<String, i64>::new();
    let mut published_diagnostic_uris = BTreeSet::new();
    let mut editor_cache = HashMap::<String, editor_lsp::Workspace>::new();
    let session = CompilerSession::new(CompilerOptions {
        force_zk: zk_enabled,
        ..CompilerOptions::default()
    });
    let driver = BuildDriver::new(session, "koto-lsp");
    while let Some(pending) = inbox.next()? {
        if inbox.reject_before_analysis(&pending, transport_output)? {
            continue;
        }
        let message = &pending.message;
        // Keep the compiler and immutable semantic workspace on this dispatcher thread.
        // The input thread can invalidate work while this operation is being analyzed.
        let mut output = Vec::new();
        let mut next_diagnostic_uris = None;
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
                    let version = message
                        .pointer("/params/textDocument/version")
                        .and_then(norito::json::Value::as_i64);
                    if let Some(version) = version
                        && versions
                            .get(uri)
                            .is_some_and(|previous| *previous >= version)
                    {
                        continue;
                    }
                    editor_cache.clear();
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
                    if documents.contains_key(uri) {
                        if let Some(version) = version {
                            versions.insert(uri.to_owned(), version);
                        }
                    } else {
                        versions.remove(uri);
                    }
                    if project_manifest.is_none() && source_root.is_some() {
                        project = lsp_local_source_project_with_root(&documents, None, source_root);
                    }
                    if inbox.is_current(&pending) {
                        next_diagnostic_uris = Some(publish_lsp_project_diagnostics(
                            &mut output,
                            &driver,
                            &documents,
                            project.as_ref(),
                            &versions,
                            &published_diagnostic_uris,
                            zk_enabled,
                        )?);
                    }
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
                    let version = message
                        .pointer("/params/textDocument/version")
                        .and_then(norito::json::Value::as_i64);
                    if let Some(version) = version
                        && versions
                            .get(uri)
                            .is_some_and(|previous| *previous >= version)
                    {
                        continue;
                    }
                    editor_cache.clear();
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
                    if documents.contains_key(uri) {
                        if let Some(version) = version {
                            versions.insert(uri.to_owned(), version);
                        }
                    } else {
                        versions.remove(uri);
                    }
                    if project_manifest.is_none() && source_root.is_some() {
                        project = lsp_local_source_project_with_root(&documents, None, source_root);
                    }
                    if inbox.is_current(&pending) {
                        next_diagnostic_uris = Some(publish_lsp_project_diagnostics(
                            &mut output,
                            &driver,
                            &documents,
                            project.as_ref(),
                            &versions,
                            &published_diagnostic_uris,
                            zk_enabled,
                        )?);
                    }
                }
            }
            Some("textDocument/didClose") => {
                if let Some(uri) = message
                    .pointer("/params/textDocument/uri")
                    .and_then(norito::json::Value::as_str)
                {
                    documents.remove(uri);
                    versions.remove(uri);
                    editor_cache.clear();
                    if project_manifest.is_none() && source_root.is_some() {
                        project = lsp_local_source_project_with_root(&documents, None, source_root);
                    }
                    if inbox.is_current(&pending) {
                        next_diagnostic_uris = Some(publish_lsp_project_diagnostics(
                            &mut output,
                            &driver,
                            &documents,
                            project.as_ref(),
                            &versions,
                            &published_diagnostic_uris,
                            zk_enabled,
                        )?);
                    }
                }
            }
            Some(
                method @ ("textDocument/completion"
                | "textDocument/hover"
                | "textDocument/signatureHelp"
                | "textDocument/definition"
                | "textDocument/references"
                | "textDocument/documentHighlight"
                | "textDocument/documentSymbol"
                | "textDocument/foldingRange"
                | "textDocument/semanticTokens/full"
                | "textDocument/codeLens"
                | "textDocument/prepareRename"
                | "textDocument/rename"),
            ) => {
                let uri = message
                    .pointer("/params/textDocument/uri")
                    .and_then(norito::json::Value::as_str)
                    .unwrap_or("");
                // A locked graph may span every open URI. Retain one bounded graph snapshot,
                // rather than duplicating the full graph once for each queried document.
                editor_cache.retain(|key, _| key == uri);
                let workspace = editor_cache.entry(uri.to_owned()).or_insert_with(|| {
                    editor_lsp::Workspace::new(&documents, project.as_ref(), uri, zk_enabled)
                        .with_versions(&versions)
                });
                match workspace.response(method, message) {
                    Ok(result) => write_lsp_response(&mut output, id, result)?,
                    Err(error) => write_lsp_error(&mut output, id, -32602, &error)?,
                }
            }
            Some("workspace/didChangeWatchedFiles" | "textDocument/didSave") => {
                editor_cache.clear();
                if let Some(path) = project_manifest {
                    match load_source_project_manifest(path) {
                        Ok(loaded) => project = Some(loaded),
                        Err(error) => {
                            project = None;
                            publish_lsp_notification(
                                &mut output,
                                "window/showMessage",
                                json_object(vec![
                                    ("type", 1_u64.into()),
                                    (
                                        "message",
                                        format!("Kotodama project reload failed: {error}").into(),
                                    ),
                                ]),
                            )?;
                        }
                    }
                }
                if project_manifest.is_none() && source_root.is_some() {
                    project = lsp_local_source_project_with_root(&documents, None, source_root);
                }
                if inbox.is_current(&pending) {
                    next_diagnostic_uris = Some(publish_lsp_project_diagnostics(
                        &mut output,
                        &driver,
                        &documents,
                        project.as_ref(),
                        &versions,
                        &published_diagnostic_uris,
                        zk_enabled,
                    )?);
                }
            }
            Some("workspace/symbol") => {
                let query = message
                    .pointer("/params/query")
                    .and_then(norito::json::Value::as_str)
                    .unwrap_or("");
                let symbols = editor_lsp::workspace_symbol_response(
                    &documents,
                    project.as_ref(),
                    zk_enabled,
                    query,
                );
                write_lsp_response(&mut output, id, symbols)?;
            }
            Some("textDocument/codeAction") => {
                let actions = message
                    .pointer("/params/textDocument/uri")
                    .and_then(norito::json::Value::as_str)
                    .and_then(|uri| {
                        documents.get(uri).map(|_| {
                            lsp_project_code_action_items(
                                &driver,
                                &documents,
                                project.as_ref(),
                                uri,
                                message.pointer("/params/range"),
                                zk_enabled,
                            )
                        })
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
        if inbox.complete(&pending, transport_output, &output)?
            && let Some(uris) = next_diagnostic_uris
        {
            published_diagnostic_uris = uris;
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
#[cfg(test)]
fn read_lsp_message(input: &mut impl BufRead) -> Result<Option<norito::json::Value>, String> {
    read_lsp_message_frame(input).map(|frame| frame.map(|(message, _)| message))
}
fn read_lsp_message_frame(
    input: &mut impl BufRead,
) -> Result<Option<(norito::json::Value, usize)>, String> {
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
        .map(|message| Some((message, length)))
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
fn collect_lsp_project_diagnostics(
    driver: &BuildDriver,
    documents: &HashMap<String, String>,
) -> HashMap<String, DiagnosticBundle> {
    // Standalone test modules are checked in compiler test mode against their targets.
    let mut ordered = documents
        .iter()
        .filter(|(uri, source)| !editor_lsp::is_test_module(uri, source))
        .collect::<Vec<_>>();
    ordered.sort_by(|(left, _), (right, _)| left.cmp(right));
    let mut logical_to_uri = HashMap::new();
    let sources = ordered
        .iter()
        .enumerate()
        .map(|(index, (uri, source))| {
            let logical = format!("open/{index:04}.ko");
            logical_to_uri.insert(logical.clone(), (*uri).clone());
            SourceModuleUnit {
                source_name: logical,
                source: (*source).clone(),
            }
        })
        .collect::<Vec<_>>();
    let mut grouped = ordered
        .iter()
        .map(|(uri, _)| ((*uri).clone(), Vec::new()))
        .collect::<HashMap<_, Vec<Diagnostic>>>();
    match driver.check_lsp_open_sources(sources) {
        Ok(warnings) => {
            for warning in warnings {
                let Some(uri) = logical_to_uri.get(&warning.source_name) else {
                    continue;
                };
                grouped
                    .entry(uri.clone())
                    .or_default()
                    .push(lint_diagnostic(warning.warning, Path::new(uri)));
            }
        }
        Err(error) => {
            let mut diagnostics = match error.into_diagnostics() {
                Ok(bundle) => bundle.diagnostics,
                Err(error) => vec![Diagnostic::error(
                    "K0000",
                    DiagnosticPhase::Lex,
                    error.to_string(),
                    None,
                )],
            };
            for diagnostic in &mut diagnostics {
                remap_project_diagnostic_sources(diagnostic, &logical_to_uri);
            }
            let fallback = ordered.first().map(|(uri, _)| (*uri).clone());
            for diagnostic in diagnostics {
                let owner = diagnostic
                    .primary_span
                    .as_ref()
                    .and_then(|span| span.source.clone())
                    .or_else(|| fallback.clone());
                if let Some(owner) = owner {
                    grouped.entry(owner).or_default().push(diagnostic);
                }
            }
        }
    }
    grouped
        .into_iter()
        .map(|(uri, diagnostics)| (uri, DiagnosticBundle::new(diagnostics)))
        .collect()
}
fn collect_lsp_workspace_diagnostics(
    driver: &BuildDriver,
    documents: &HashMap<String, String>,
    project: Option<&LoadedSourceProject>,
) -> HashMap<String, DiagnosticBundle> {
    // Without an explicit project every open seiyaku roots its own local graph, exactly as
    // navigation analyzes it, so unrelated seiyaku in one directory are never one check.
    let local_projects = if project.is_none() {
        lsp_local_source_projects_with_root(documents, None, None, usize::MAX)
    } else {
        Vec::new()
    };
    let projects = project
        .into_iter()
        .chain(&local_projects)
        .collect::<Vec<_>>();
    if projects.is_empty() {
        return collect_lsp_project_diagnostics(driver, documents);
    }
    let mut grouped = documents
        .keys()
        .cloned()
        .map(|uri| (uri, Vec::new()))
        .collect::<HashMap<_, Vec<Diagnostic>>>();
    let mut covered = HashSet::new();
    for project in projects {
        let (diagnostics, project_documents) =
            collect_lsp_graph_diagnostics(driver, documents, project);
        for (uri, bundle) in diagnostics {
            let published = grouped.entry(uri).or_default();
            for diagnostic in bundle.diagnostics {
                // A module imported by several open roots reports each of its errors once.
                if !published.contains(&diagnostic) {
                    published.push(diagnostic);
                }
            }
        }
        covered.extend(project_documents);
    }
    let loose_documents = documents
        .iter()
        .filter(|(uri, _)| !covered.contains(*uri))
        .map(|(uri, source)| (uri.clone(), source.clone()))
        .collect::<HashMap<_, _>>();
    for (uri, bundle) in collect_lsp_project_diagnostics(driver, &loose_documents) {
        grouped.entry(uri).or_default().extend(bundle.diagnostics);
    }
    grouped
        .into_iter()
        .map(|(uri, diagnostics)| (uri, DiagnosticBundle::new(diagnostics)))
        .collect()
}
/// Diagnostics of one project graph with the open documents overlaid, and the open documents
/// that graph accounts for. A graph whose sources cannot be loaded reports only that loading
/// error and accounts for every open document, as a failed `koto check` stops there.
fn collect_lsp_graph_diagnostics(
    driver: &BuildDriver,
    documents: &HashMap<String, String>,
    project: &LoadedSourceProject,
) -> (HashMap<String, DiagnosticBundle>, HashSet<String>) {
    let (graph, source_uris, project_documents, _) =
        match lsp_project_with_open_overlays(project, documents) {
            Ok(overlaid) => overlaid,
            Err(error) => {
                return (
                    lsp_source_loading_diagnostics(error, project, documents),
                    documents.keys().cloned().collect(),
                );
            }
        };
    let mut grouped = HashMap::<String, Vec<Diagnostic>>::new();
    match driver.check_project(graph) {
        Ok(warnings) => {
            for warning in warnings {
                let key = ProjectSourceKey {
                    package_identity: warning.package_identity.clone(),
                    source_name: warning.source_name,
                };
                let Some(uri) = source_uris.get(&key) else {
                    continue;
                };
                // The editor reports the project manifest's lint levels, as `koto check` does.
                let Some(lint) = leveled_lint(&project.lints, warning.warning) else {
                    continue;
                };
                let diagnostic = lint.to_diagnostic(
                    uri,
                    warning.package_identity.as_deref(),
                    kotodama_lang::i18n::detect_language(),
                );
                grouped.entry(uri.clone()).or_default().push(diagnostic);
            }
        }
        Err(error) => {
            let diagnostics = error.into_diagnostics().unwrap_or_else(|error| {
                DiagnosticBundle::single(Diagnostic::error(
                    "K0000",
                    DiagnosticPhase::Lex,
                    error.to_string(),
                    None,
                ))
            });
            let fallback = source_uris.values().next().cloned();
            for mut diagnostic in diagnostics.diagnostics {
                let owner = diagnostic.primary_span.as_ref().and_then(|span| {
                    let key = ProjectSourceKey {
                        package_identity: span.package_identity.clone(),
                        source_name: span.source.clone()?,
                    };
                    source_uris.get(&key).cloned()
                });
                if owner.is_none() {
                    if let Some(span) = diagnostic.primary_span.take() {
                        diagnostic.notes.push(format!(
                            "locked project error originates in {}{}",
                            span.package_identity
                                .as_deref()
                                .map_or(String::new(), |package| format!("{package}::")),
                            span.source.as_deref().unwrap_or("<source>")
                        ));
                    }
                    // Edits for a source that is not open here cannot be applied.
                    diagnostic.fix = None;
                    diagnostic.alternative_fixes.clear();
                }
                remap_lsp_locked_project_diagnostic(&mut diagnostic, &source_uris);
                if let Some(uri) = owner.or_else(|| fallback.clone()) {
                    grouped.entry(uri).or_default().push(diagnostic);
                }
            }
        }
    }
    (
        grouped
            .into_iter()
            .map(|(uri, diagnostics)| (uri, DiagnosticBundle::new(diagnostics)))
            .collect(),
        project_documents,
    )
}
fn lsp_source_loading_diagnostics(
    error: BuildError,
    project: &LoadedSourceProject,
    documents: &HashMap<String, String>,
) -> HashMap<String, DiagnosticBundle> {
    let bundle = error.into_diagnostics().unwrap_or_else(|error| {
        DiagnosticBundle::single(Diagnostic::error(
            "E_SOURCE_NOT_FOUND",
            DiagnosticPhase::Resolve,
            error.to_string(),
            None,
        ))
    });
    let root_key = ProjectSourceKey {
        package_identity: None,
        source_name: project.graph.root.source_name.clone(),
    };
    let root_path = project.source_paths.get(&root_key);
    let source_root = project
        .manifest
        .as_ref()
        .and_then(|manifest| manifest.path().parent().map(Path::to_path_buf))
        .or_else(|| {
            let mut path = root_path?.clone();
            for _ in project.graph.root.source_name.split('/') {
                path.pop();
            }
            Some(path)
        });
    let mut source_uris = project
        .source_paths
        .iter()
        .filter_map(|(key, path)| lsp_path_file_uri(path).map(|uri| (key.clone(), uri)))
        .collect::<BTreeMap<_, _>>();
    for diagnostic in &bundle.diagnostics {
        for span in diagnostic
            .primary_span
            .iter()
            .chain(diagnostic.labels.iter().map(|label| &label.span))
        {
            if let (Some(root), Some(name)) = (&source_root, &span.source) {
                let path = root.join(name);
                if let Some(uri) = lsp_path_file_uri(&path) {
                    source_uris
                        .entry(ProjectSourceKey {
                            package_identity: span.package_identity.clone(),
                            source_name: name.clone(),
                        })
                        .or_insert(uri);
                }
            }
        }
    }
    let fallback = root_path.and_then(|path| lsp_path_file_uri(path));
    let mut grouped = documents
        .keys()
        .map(|uri| (uri.clone(), Vec::new()))
        .collect::<HashMap<_, _>>();
    for mut diagnostic in bundle.diagnostics {
        remap_lsp_locked_project_diagnostic(&mut diagnostic, &source_uris);
        if let Some(uri) = diagnostic
            .primary_span
            .as_ref()
            .and_then(|span| span.source.clone())
            .or_else(|| fallback.clone())
        {
            grouped.entry(uri).or_default().push(diagnostic);
        }
    }
    grouped
        .into_iter()
        .map(|(uri, diagnostics)| (uri, DiagnosticBundle::new(diagnostics)))
        .collect()
}
fn lsp_local_source_project(
    documents: &HashMap<String, String>,
    requested_uri: Option<&str>,
) -> Option<LoadedSourceProject> {
    lsp_local_source_project_with_root(documents, requested_uri, None)
}
fn lsp_local_source_project_with_root(
    documents: &HashMap<String, String>,
    requested_uri: Option<&str>,
    source_root: Option<&Path>,
) -> Option<LoadedSourceProject> {
    lsp_local_source_projects_with_root(documents, requested_uri, source_root, 1).pop()
}
/// Local graphs rooted at the open seiyaku documents, in path order, at most `limit` of them.
/// Each root reads its declared `include`/`import` closure relative to its own directory (or
/// `source_root`); with `requested_uri`, only graphs containing that document are returned.
fn lsp_local_source_projects_with_root(
    documents: &HashMap<String, String>,
    requested_uri: Option<&str>,
    source_root: Option<&Path>,
    limit: usize,
) -> Vec<LoadedSourceProject> {
    let overlays = documents
        .iter()
        .filter_map(|(uri, source)| lsp_file_uri_path(uri).map(|path| (path, source.clone())))
        .collect::<BTreeMap<_, _>>();
    let requested = requested_uri.and_then(lsp_file_uri_path);
    let mut ordered = overlays.iter().collect::<Vec<_>>();
    ordered.sort_by(|(left, _), (right, _)| left.cmp(right));
    let mut projects = Vec::new();
    for (path, source) in ordered {
        if projects.len() >= limit {
            break;
        }
        if !kotodama_lang::parser::parse(source)
            .is_ok_and(|program| program.unit.kind == kotodama_lang::ast::SourceUnitKind::Seiyaku)
        {
            continue;
        }
        let Some(root) = source_root.or_else(|| path.parent()) else {
            continue;
        };
        // Retain the root even while its declared closure is incomplete. Overlay loading below
        // reports the exact dependency error instead of reclassifying this contract as loose.
        let project = load_source_project(path, root, &overlays).unwrap_or_else(|_| {
            let source_name = logical_source_name(path, root)
                .unwrap_or_else(|_| path.to_string_lossy().into_owned());
            LoadedSourceProject {
                graph: kotodama_lang::linker::SourceLinkRequest {
                    root: SourceModuleUnit {
                        source_name: source_name.clone(),
                        source: source.clone(),
                    },
                    sources: Vec::new(),
                    imports: Vec::new(),
                    packages: Vec::new(),
                },
                source_paths: BTreeMap::from([(
                    ProjectSourceKey {
                        package_identity: None,
                        source_name,
                    },
                    path.clone(),
                )]),
                manifest: None,
                lints: LintConfig::default(),
            }
        });
        if requested
            .as_ref()
            .is_none_or(|requested| project.source_paths.values().any(|path| path == requested))
        {
            projects.push(project);
        }
    }
    projects
}
/// Project link graph with the open editor documents overlaid: the link request, the
/// document URI of each project source, the open document URIs owned by the project, and
/// the effective project manifest.
type LspOverlaidProject = (
    kotodama_lang::linker::SourceLinkRequest,
    BTreeMap<ProjectSourceKey, String>,
    HashSet<String>,
    Option<kotodama_lang::driver::ProjectManifestSource>,
);
fn lsp_project_with_open_overlays(
    project: &LoadedSourceProject,
    documents: &HashMap<String, String>,
) -> Result<LspOverlaidProject, BuildError> {
    let overlays = documents
        .iter()
        .filter_map(|(uri, source)| lsp_file_uri_path(uri).map(|path| (path, source.clone())))
        .collect::<BTreeMap<_, _>>();
    let manifest_overlay = project.manifest.as_ref().and_then(|manifest| {
        documents
            .iter()
            .find(|(uri, _)| lsp_file_uri_path(uri).as_deref() == Some(manifest.path()))
    });
    let reloaded = project
        .manifest
        .as_ref()
        .map(|manifest| {
            kotodama_lang::driver::load_source_project_manifest_with_text_and_overlays(
                manifest.path(),
                manifest_overlay.map_or(manifest.text(), |(_, text)| text.as_str()),
                &overlays,
            )
        })
        .transpose()?;
    let project = reloaded.as_ref().unwrap_or(project);
    let mut graph = project.graph.clone();
    let mut source_uris = project
        .source_paths
        .iter()
        .filter_map(|(key, path)| lsp_path_file_uri(path).map(|uri| (key.clone(), uri)))
        .collect::<BTreeMap<_, _>>();
    let mut overlaid = BTreeSet::new();
    let mut project_documents = HashSet::new();
    if let Some((uri, _)) = manifest_overlay {
        project_documents.insert(uri.clone());
    }
    let mut ordered = documents.iter().collect::<Vec<_>>();
    ordered.sort_by(|(left, _), (right, _)| left.cmp(right));
    for (uri, source) in ordered {
        let Some(path) = lsp_file_uri_path(uri) else {
            continue;
        };
        let Some((key, _)) = project
            .source_paths
            .iter()
            .find(|(_, project_path)| *project_path == &path)
        else {
            continue;
        };
        if !overlaid.insert(key.clone()) {
            continue;
        }
        if replace_project_source(&mut graph, key, source) {
            source_uris.insert(key.clone(), uri.clone());
            project_documents.insert(uri.clone());
        }
    }
    let source_root = project
        .manifest
        .as_ref()
        .and_then(|manifest| manifest.path().parent().map(Path::to_path_buf))
        .or_else(|| {
            let key = ProjectSourceKey {
                package_identity: None,
                source_name: graph.root.source_name.clone(),
            };
            let mut root = project.source_paths.get(&key)?.clone();
            for _ in graph.root.source_name.split('/') {
                root.pop();
            }
            Some(root)
        })
        .ok_or_else(|| BuildError::InvalidPath {
            path: PathBuf::from(&graph.root.source_name),
            message: "project root has no physical source path".into(),
        })?;
    graph.sources = kotodama_lang::driver::load_source_companions(
        std::slice::from_ref(&graph.root),
        &source_root,
        &overlays,
    )?;
    for package in &mut graph.packages {
        package.sources = kotodama_lang::driver::load_source_package_companions(
            &package.modules,
            &source_root,
            &overlays,
            &package.identity,
        )
        .map_err(|error| match error.into_diagnostics() {
            Ok(mut bundle) => {
                for diagnostic in &mut bundle.diagnostics {
                    for span in diagnostic
                        .primary_span
                        .iter_mut()
                        .chain(diagnostic.labels.iter_mut().map(|label| &mut label.span))
                    {
                        span.package_identity = Some(package.identity.clone());
                    }
                    for fix in diagnostic
                        .fix
                        .iter_mut()
                        .chain(&mut diagnostic.alternative_fixes)
                    {
                        fix.span.package_identity = Some(package.identity.clone());
                    }
                }
                BuildError::Compile(bundle)
            }
            Err(error) => error,
        })?;
    }
    for (owner, source) in
        graph
            .sources
            .iter()
            .map(|source| (None, source))
            .chain(graph.packages.iter().flat_map(|package| {
                package
                    .sources
                    .iter()
                    .map(move |source| (Some(package.identity.clone()), source))
            }))
    {
        let path = source_root.join(&source.source_name);
        let uri = lsp_path_file_uri(&path).ok_or_else(|| BuildError::InvalidPath {
            path,
            message: "source URI requires a UTF-8 path".into(),
        })?;
        if documents.contains_key(&uri) {
            project_documents.insert(uri.clone());
        }
        source_uris.insert(
            ProjectSourceKey {
                package_identity: owner,
                source_name: source.source_name.clone(),
            },
            uri,
        );
    }
    Ok((
        graph,
        source_uris,
        project_documents,
        project.manifest.clone(),
    ))
}
fn lsp_path_file_uri(path: &Path) -> Option<String> {
    let text = path.to_str()?;
    let mut uri = String::from("file://");
    if !text.starts_with('/') {
        uri.push('/');
    }
    for byte in text.as_bytes() {
        if byte.is_ascii_alphanumeric() || matches!(*byte, b'/' | b'-' | b'_' | b'.' | b'~' | b':')
        {
            uri.push(char::from(*byte));
        } else {
            use std::fmt::Write as _;
            let _ = write!(uri, "%{byte:02X}");
        }
    }
    Some(uri)
}
fn replace_project_source(
    graph: &mut kotodama_lang::linker::SourceLinkRequest,
    key: &ProjectSourceKey,
    source: &str,
) -> bool {
    match &key.package_identity {
        None if graph.root.source_name == key.source_name => {
            graph.root.source = source.to_owned();
            true
        }
        None => graph
            .sources
            .iter_mut()
            .find(|unit| unit.source_name == key.source_name)
            .is_some_and(|unit| {
                unit.source = source.to_owned();
                true
            }),
        Some(package_identity) => graph
            .packages
            .iter_mut()
            .find(|package| &package.identity == package_identity)
            .and_then(|package| {
                package
                    .modules
                    .iter_mut()
                    .chain(package.sources.iter_mut())
                    .find(|module| module.source_name == key.source_name)
            })
            .is_some_and(|module| {
                module.source = source.to_owned();
                true
            }),
    }
}
fn lsp_file_uri_path(uri: &str) -> Option<PathBuf> {
    let encoded = uri
        .strip_prefix("file://localhost")
        .or_else(|| uri.strip_prefix("file://"))?;
    if !encoded.starts_with('/') {
        // A non-empty authority names a remote host. Kotodama project sources
        // are canonical local files, so such a URI cannot own an overlay.
        return None;
    }
    let bytes = encoded.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let high = decode_hex_digit(*bytes.get(index + 1)?)?;
            let low = decode_hex_digit(*bytes.get(index + 2)?)?;
            decoded.push((high << 4) | low);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    let decoded = String::from_utf8(decoded).ok()?;
    #[cfg(windows)]
    let decoded = decoded
        .strip_prefix('/')
        .filter(|path| path.as_bytes().get(1) == Some(&b':'))
        .unwrap_or(&decoded);
    let path = PathBuf::from(decoded);
    path.canonicalize().ok().or_else(|| {
        let mut normalized = PathBuf::new();
        for component in path.components() {
            match component {
                std::path::Component::ParentDir => {
                    if !normalized.pop() {
                        return None;
                    }
                }
                std::path::Component::CurDir => {}
                component => normalized.push(component.as_os_str()),
            }
        }
        normalized.is_absolute().then_some(normalized)
    })
}
fn decode_hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}
fn remap_lsp_locked_project_diagnostic(
    diagnostic: &mut Diagnostic,
    source_uris: &BTreeMap<ProjectSourceKey, String>,
) {
    let remap = |span: &mut SourceSpan| {
        let Some(source_name) = span.source.as_ref() else {
            return;
        };
        let key = ProjectSourceKey {
            package_identity: span.package_identity.clone(),
            source_name: source_name.clone(),
        };
        if let Some(uri) = source_uris.get(&key) {
            span.source = Some(uri.clone());
        }
    };
    if let Some(span) = &mut diagnostic.primary_span {
        remap(span);
    }
    for label in &mut diagnostic.labels {
        remap(&mut label.span);
    }
    for fix in diagnostic
        .fix
        .iter_mut()
        .chain(&mut diagnostic.alternative_fixes)
    {
        remap(&mut fix.span);
    }
}
fn remap_project_diagnostic_sources(
    diagnostic: &mut Diagnostic,
    logical_to_uri: &HashMap<String, String>,
) {
    let remap = |span: &mut SourceSpan| {
        if let Some(uri) = span
            .source
            .as_ref()
            .and_then(|source| logical_to_uri.get(source))
        {
            span.source = Some(uri.clone());
        }
    };
    if let Some(span) = &mut diagnostic.primary_span {
        remap(span);
    }
    for label in &mut diagnostic.labels {
        remap(&mut label.span);
    }
    for fix in diagnostic
        .fix
        .iter_mut()
        .chain(&mut diagnostic.alternative_fixes)
    {
        remap(&mut fix.span);
    }
}
fn remap_locked_project_diagnostic_sources(
    diagnostic: &mut Diagnostic,
    source_paths: &BTreeMap<ProjectSourceKey, PathBuf>,
) {
    let remap = |span: &mut SourceSpan| {
        let Some(source_name) = span.source.as_ref() else {
            return;
        };
        let key = ProjectSourceKey {
            package_identity: span.package_identity.clone(),
            source_name: source_name.clone(),
        };
        if let Some(path) = source_paths.get(&key) {
            span.source = Some(display_path(path));
        }
    };
    if let Some(span) = &mut diagnostic.primary_span {
        remap(span);
    }
    for label in &mut diagnostic.labels {
        remap(&mut label.span);
    }
    for fix in diagnostic
        .fix
        .iter_mut()
        .chain(&mut diagnostic.alternative_fixes)
    {
        remap(&mut fix.span);
    }
}
/// Name root-local sources of a diagnostic (logical paths below `root`) by their
/// working-directory-relative paths, as semantic diagnostics of the same files are named.
fn remap_rooted_diagnostic_sources(diagnostic: &mut Diagnostic, root: &Path) {
    let remap = |span: &mut SourceSpan| {
        if span.package_identity.is_some() {
            return;
        }
        let Some(source_name) = span.source.as_deref() else {
            return;
        };
        let path = root.join(source_name);
        if path.is_file() {
            span.source = Some(display_path(&path));
        }
    };
    if let Some(span) = &mut diagnostic.primary_span {
        remap(span);
    }
    for label in &mut diagnostic.labels {
        remap(&mut label.span);
    }
    for fix in diagnostic
        .fix
        .iter_mut()
        .chain(&mut diagnostic.alternative_fixes)
    {
        remap(&mut fix.span);
    }
}
fn publish_lsp_project_diagnostics(
    output: &mut impl Write,
    driver: &BuildDriver,
    documents: &HashMap<String, String>,
    project: Option<&LoadedSourceProject>,
    versions: &HashMap<String, i64>,
    previously_published: &BTreeSet<String>,
    zk_enabled: bool,
) -> Result<BTreeSet<String>, String> {
    let mut diagnostics = collect_lsp_workspace_diagnostics(driver, documents, project);
    editor_lsp::apply_test_module_diagnostics(&mut diagnostics, documents, zk_enabled);
    let current_uris = documents
        .keys()
        .chain(diagnostics.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    for uri in current_uris.union(previously_published) {
        let source = documents.get(uri).map_or("", String::as_str);
        let values = diagnostics
            .get(uri)
            .into_iter()
            .flat_map(|bundle| bundle.diagnostics.iter())
            .map(|diagnostic| lsp_diagnostic_value(diagnostic, source))
            .collect();
        let mut params = vec![
            ("uri", norito::json::Value::from(uri.as_str())),
            ("diagnostics", norito::json::Value::Array(values)),
        ];
        if let Some(version) = versions.get(uri) {
            params.push(("version", (*version).into()));
        }
        publish_lsp_notification(
            output,
            "textDocument/publishDiagnostics",
            json_object(params),
        )?;
    }
    Ok(current_uris)
}
fn lsp_initialize_result() -> norito::json::Value {
    json_object(vec![
        (
            "serverInfo",
            json_object(vec![
                ("name", "koto".into()),
                ("version", env!("CARGO_PKG_VERSION").into()),
            ]),
        ),
        ("capabilities", lsp_capabilities()),
    ])
}
fn lsp_capabilities() -> norito::json::Value {
    json_object(vec![
        ("textDocumentSync", norito::json::Value::from(1_u64)),
        ("documentSymbolProvider", true.into()),
        ("workspaceSymbolProvider", true.into()),
        ("documentHighlightProvider", true.into()),
        ("foldingRangeProvider", true.into()),
        (
            "semanticTokensProvider",
            json_object(vec![
                ("legend", editor_lsp::semantic_tokens_legend()),
                ("full", true.into()),
            ]),
        ),
        (
            "codeLensProvider",
            json_object(vec![("resolveProvider", false.into())]),
        ),
        (
            "completionProvider",
            json_object(vec![
                ("resolveProvider", norito::json::Value::from(false)),
                (
                    "triggerCharacters",
                    norito::json::Value::Array(vec![".".into(), ":".into()]),
                ),
            ]),
        ),
        (
            "documentFormattingProvider",
            norito::json::Value::from(true),
        ),
        (
            "codeActionProvider",
            json_object(vec![(
                "codeActionKinds",
                norito::json::Value::Array(vec!["quickfix".into()]),
            )]),
        ),
        ("hoverProvider", true.into()),
        ("definitionProvider", true.into()),
        ("referencesProvider", true.into()),
        (
            "renameProvider",
            json_object(vec![("prepareProvider", true.into())]),
        ),
        (
            "signatureHelpProvider",
            json_object(vec![(
                "triggerCharacters",
                norito::json::Value::Array(vec!["(".into(), ",".into(), ":".into()]),
            )]),
        ),
        ("positionEncoding", "utf-16".into()),
    ])
}
#[cfg(test)]
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
#[cfg(test)]
fn lsp_diagnostics(session: &CompilerSession, uri: &str, source: &str) -> Vec<norito::json::Value> {
    collect_lsp_diagnostics(session, uri, source)
        .diagnostics
        .iter()
        .map(|diagnostic| lsp_diagnostic_value(diagnostic, source))
        .collect()
}
/// Whether a note is a terminal source excerpt: text lines followed by a caret underline.
fn is_rendered_source_excerpt(note: &str) -> bool {
    note.contains('\n')
        && note.lines().last().is_some_and(|underline| {
            underline.contains('^')
                && underline
                    .chars()
                    .all(|character| matches!(character, '^' | '~' | '-' | ' ' | '\t'))
        })
}
fn lsp_diagnostic_value(diagnostic: &Diagnostic, source: &str) -> norito::json::Value {
    let source = diagnostic
        .primary_source
        .as_ref()
        .map_or(source, |file| file.text());
    let range = diagnostic.primary_span.as_ref().map_or_else(
        || lsp_range(0, 0, 0, 1),
        |span| lsp_source_span_range(source, span),
    );
    let mut message = diagnostic.message.clone();
    // Editors draw the primary range themselves; terminal source excerpts are never repeated.
    for note in diagnostic
        .notes
        .iter()
        .filter(|note| !is_rendered_source_excerpt(note))
    {
        message.push_str("\n\nnote: ");
        message.push_str(note);
    }
    if let Some(help) = &diagnostic.help {
        message.push_str("\n\nhelp: ");
        message.push_str(help);
    }
    let related = diagnostic
        .labels
        .iter()
        .enumerate()
        .filter_map(|(index, label)| {
            let uri = label.span.source.as_deref()?;
            let label_source = diagnostic
                .label_sources
                .get(index)
                .and_then(Option::as_ref)
                .map_or("", |file| file.text());
            Some(json_object(vec![
                (
                    "location",
                    json_object(vec![
                        ("uri", norito::json::Value::from(uri)),
                        ("range", lsp_source_span_range(label_source, &label.span)),
                    ]),
                ),
                ("message", norito::json::Value::from(label.message.clone())),
            ]))
        })
        .collect::<Vec<_>>();
    json_object(vec![
        ("range", range),
        ("code", norito::json::Value::from(diagnostic.code.clone())),
        (
            "severity",
            norito::json::Value::from(match diagnostic.severity {
                kotodama_lang::diagnostic::Severity::Error => 1_u64,
                kotodama_lang::diagnostic::Severity::Warning => 2_u64,
            }),
        ),
        ("source", norito::json::Value::from("kotodama")),
        ("relatedInformation", norito::json::Value::Array(related)),
        (
            "codeDescription",
            json_object(vec![(
                "href",
                norito::json::Value::from(explain::documentation_url(&diagnostic.code)),
            )]),
        ),
        ("message", norito::json::Value::from(message)),
    ])
}
#[cfg(test)]
fn lsp_code_action_items(
    session: &CompilerSession,
    uri: &str,
    source: &str,
) -> norito::json::Value {
    lsp_code_actions_from_bundle(
        collect_lsp_diagnostics(session, uri, source),
        uri,
        source,
        None,
    )
}
fn lsp_project_code_action_items(
    driver: &BuildDriver,
    documents: &HashMap<String, String>,
    project: Option<&LoadedSourceProject>,
    uri: &str,
    range: Option<&norito::json::Value>,
    zk_enabled: bool,
) -> norito::json::Value {
    let source = documents.get(uri).map_or("", String::as_str);
    let mut diagnostics = collect_lsp_workspace_diagnostics(driver, documents, project);
    editor_lsp::apply_test_module_diagnostics(&mut diagnostics, documents, zk_enabled);
    let bundle = diagnostics
        .remove(uri)
        .unwrap_or_else(|| DiagnosticBundle::new(Vec::new()));
    lsp_code_actions_from_bundle(
        bundle,
        uri,
        source,
        range.and_then(|range| lsp_byte_range(source, range)),
    )
}
/// Byte range of an LSP UTF-16 range in `source`.
fn lsp_byte_range(
    source: &str,
    range: &norito::json::Value,
) -> Option<kotodama_lang::source::TextRange> {
    let offset = |position: &str| -> Option<u32> {
        let line = usize::try_from(range.pointer(&format!("/{position}/line"))?.as_u64()?).ok()?;
        let character =
            usize::try_from(range.pointer(&format!("/{position}/character"))?.as_u64()?).ok()?;
        let start = source
            .split_inclusive('\n')
            .take(line)
            .map(str::len)
            .sum::<usize>();
        let text = source.get(start..)?.split('\n').next()?;
        let mut utf16 = 0;
        for (byte, ch) in text.char_indices() {
            if utf16 >= character {
                return u32::try_from(start + byte).ok();
            }
            utf16 += ch.len_utf16();
        }
        u32::try_from(start + text.len()).ok()
    };
    let (start, end) = (offset("start")?, offset("end")?);
    (start <= end).then(|| kotodama_lang::source::TextRange::new(start, end))
}
/// Short code-action title naming exactly what the edit does.
fn lsp_fix_title(
    source: &str,
    fix: &kotodama_lang::diagnostic::DiagnosticFix,
    code: &str,
) -> String {
    let replaced = fix
        .span
        .byte_range
        .and_then(|range| source.get(range.start as usize..range.end as usize))
        .unwrap_or_default();
    let short = |text: &str| !text.contains('\n') && text.chars().count() <= 40;
    match (
        replaced.trim().is_empty(),
        fix.replacement.trim().is_empty(),
    ) {
        (false, true) if short(replaced) => format!("Remove `{}`", replaced.trim()),
        (true, false) if short(&fix.replacement) => {
            format!("Insert `{}`", fix.replacement.trim())
        }
        (false, false) if short(replaced) && short(&fix.replacement) => format!(
            "Replace `{}` with `{}`",
            replaced.trim(),
            fix.replacement.trim()
        ),
        _ => format!("Apply the suggested {code} fix"),
    }
}
fn lsp_code_actions_from_bundle(
    bundle: DiagnosticBundle,
    uri: &str,
    source: &str,
    range: Option<kotodama_lang::source::TextRange>,
) -> norito::json::Value {
    let mut actions = Vec::new();
    for diagnostic in bundle.diagnostics {
        // Offer only fixes for diagnostics that touch the requested range.
        if let Some(requested) = range
            && diagnostic
                .primary_span
                .as_ref()
                .and_then(|span| span.byte_range)
                .is_some_and(|span| span.end < requested.start || requested.end < span.start)
        {
            continue;
        }
        let fixes = diagnostic
            .fix
            .iter()
            .map(|fix| (fix, true))
            .chain(diagnostic.alternative_fixes.iter().map(|fix| (fix, false)))
            .collect::<Vec<_>>();
        for (fix, preferred) in fixes {
            let Some(byte_range) = fix.span.byte_range else {
                continue;
            };
            let (Ok(start), Ok(end)) = (
                usize::try_from(byte_range.start),
                usize::try_from(byte_range.end),
            ) else {
                continue;
            };
            if start > end
                || end > source.len()
                || !source.is_char_boundary(start)
                || !source.is_char_boundary(end)
            {
                continue;
            }
            let edit = json_object(vec![
                ("range", lsp_text_range(source, byte_range)),
                (
                    "newText",
                    norito::json::Value::from(fix.replacement.clone()),
                ),
            ]);
            let Ok(changes) =
                norito::json::object([(uri.to_owned(), norito::json::Value::Array(vec![edit]))])
            else {
                continue;
            };
            actions.push(json_object(vec![
                (
                    "title",
                    norito::json::Value::from(lsp_fix_title(source, fix, &diagnostic.code)),
                ),
                ("kind", norito::json::Value::from("quickfix")),
                ("isPreferred", norito::json::Value::from(preferred)),
                (
                    "diagnostics",
                    norito::json::Value::Array(vec![lsp_diagnostic_value(&diagnostic, source)]),
                ),
                ("edit", json_object(vec![("changes", changes)])),
            ]));
        }
    }
    norito::json::Value::Array(actions)
}
fn lsp_source_span_range(source: &str, span: &SourceSpan) -> norito::json::Value {
    span.byte_range
        .filter(|range| range.end as usize <= source.len() && !source.is_empty())
        .map_or_else(
            || {
                let (start_line, start_character) = lsp_source_position(source, &span.start);
                let (end_line, end_character) = lsp_source_position(source, &span.end);
                lsp_range(start_line, start_character, end_line, end_character)
            },
            |range| lsp_text_range(source, range),
        )
}
fn lsp_source_position(source: &str, position: &SourcePosition) -> (u64, u64) {
    let line = position.line.saturating_sub(1);
    let column = position.column.saturating_sub(1);
    let character = source
        .split('\n')
        .nth(line)
        .filter(|_| !source.is_empty())
        .map_or(column, |text| {
            text.chars().take(column).map(char::len_utf16).sum()
        });
    (line as u64, character as u64)
}
fn lsp_text_range(source: &str, range: kotodama_lang::source::TextRange) -> norito::json::Value {
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
#[cfg(test)]
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
    for (builtin, spec) in Builtin::registry() {
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
    /// Parse a complete `koto` argument vector the way `main` does.
    fn parse_cli(args: &[&str]) -> Result<Cli, clap::Error> {
        let command = Cli::command().long_version(koto_long_version());
        let matches =
            command.try_get_matches_from(std::iter::once("koto").chain(args.iter().copied()))?;
        Cli::from_arg_matches(&matches)
    }
    #[test]
    fn command_inventory_is_exact_and_retired_names_stay_rejected() {
        let inventory = Cli::command()
            .get_subcommands()
            .map(|command| command.get_name().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(
            inventory,
            ["check", "build", "test", "fmt", "doc", "explain", "lsp"]
        );
        for retired in ["compile", "lint", "koto_compile", "koto_lint", "koto_test"] {
            let error = parse_cli(&[retired, "x.ko"]).expect_err("retired command");
            assert_eq!(
                error.kind(),
                clap::error::ErrorKind::InvalidSubcommand,
                "{retired}"
            );
        }
        Cli::command().debug_assert();
    }
    #[test]
    fn every_subcommand_has_help_and_the_binary_reports_its_version() {
        for command in ["check", "build", "test", "fmt", "doc", "explain", "lsp"] {
            for flag in ["--help", "-h"] {
                let error = parse_cli(&[command, flag]).expect_err("help is reported through clap");
                assert_eq!(
                    error.kind(),
                    clap::error::ErrorKind::DisplayHelp,
                    "{command} {flag}"
                );
                assert_eq!(report_status_for(&error), ExitStatus::Success);
            }
        }
        for flag in ["--version", "-V"] {
            let error = parse_cli(&[flag]).expect_err("version is reported through clap");
            assert_eq!(error.kind(), clap::error::ErrorKind::DisplayVersion);
        }
        let long = koto_long_version();
        assert!(long.contains("kotodama_lang/"));
        assert!(long.contains("IVM 1.1 bytecode"));
        assert!(long.lines().any(|line| {
            line.strip_prefix("abi_hash: ")
                .is_some_and(|hash| hash.len() == 64)
        }));
        let help = Cli::command().render_long_help().to_string();
        for status in [
            "0   success",
            "2   usage error",
            "8   compiler diagnostics",
            "10  ",
            "11  the tests ran",
            "70  internal",
        ] {
            assert!(
                help.contains(status),
                "help omits exit status `{status}`:\n{help}"
            );
        }
    }
    /// Exit status `main` selects for a clap parse outcome, without printing.
    fn report_status_for(error: &clap::Error) -> ExitStatus {
        use clap::error::ErrorKind;
        match error.kind() {
            ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => ExitStatus::Success,
            _ => ExitStatus::Usage,
        }
    }
    #[test]
    fn unknown_options_report_the_subcommand_usage_with_the_usage_status() {
        let error = parse_cli(&["check", "--frobnicate", "a.ko"]).expect_err("unknown flag");
        assert_eq!(error.kind(), clap::error::ErrorKind::UnknownArgument);
        assert_eq!(report_status_for(&error), ExitStatus::Usage);
        assert!(error.to_string().contains("koto check"), "{error}");
        let error = parse_cli(&["build"]).expect_err("missing source");
        assert_eq!(report_status_for(&error), ExitStatus::Usage);
        assert_eq!(ExitStatus::Usage.code(), 2);
        assert_eq!(ExitStatus::Failed.code(), 8);
        assert_eq!(ExitStatus::Io.code(), 10);
        assert_eq!(ExitStatus::TestsFailed.code(), 11);
        assert_eq!(ExitStatus::Internal.code(), 70);
        assert_eq!(KotoError::Io("x".to_owned()).exit_status(), ExitStatus::Io);
        assert_eq!(
            KotoError::TestsFailed.exit_status(),
            ExitStatus::TestsFailed
        );
        assert_eq!(
            KotoError::from(kotodama_toolchain::koto_test_driver::KotoTestCliError {
                kind: kotodama_toolchain::koto_test_driver::KotoTestCliErrorKind::TestsFailed,
                message: String::new(),
            })
            .exit_status(),
            ExitStatus::TestsFailed
        );
        assert_eq!(
            KotoError::Failed("drift".to_owned()).exit_status(),
            ExitStatus::Failed
        );
    }
    #[test]
    fn test_options_take_explicit_report_paths_and_default_to_run() {
        use kotodama_toolchain::koto_test_driver::{KotoTestAction, KotoTestReportFormat};
        let Cli {
            command: KotoCommand::Test(args),
        } = parse_cli(&["test", "--junit", "report.xml", "tests/vault.test.ko"]).expect("parse")
        else {
            panic!("expected test command");
        };
        let options = test_cli_options(args);
        assert_eq!(options.action, KotoTestAction::Run);
        assert_eq!(options.junit, Some(PathBuf::from("report.xml")));
        assert_eq!(options.source, Some(PathBuf::from("tests/vault.test.ko")));
        assert_eq!(options.format, KotoTestReportFormat::Human);
        let error = parse_cli(&["test", "--junit", "tests/vault.test.ko"])
            .expect_err("--junit consumes its value, so the source is missing");
        assert_eq!(
            error.kind(),
            clap::error::ErrorKind::MissingRequiredArgument
        );
        for (action, expected) in [
            ("run", KotoTestAction::Run),
            ("list", KotoTestAction::List),
            ("coverage", KotoTestAction::Coverage),
            ("trace", KotoTestAction::Trace),
        ] {
            let Cli {
                command: KotoCommand::Test(args),
            } = parse_cli(&["test", action, "--filter", "quote", "--exact", "x.test.ko"])
                .expect("parse action")
            else {
                panic!("expected test command");
            };
            let options = test_cli_options(args);
            assert_eq!(options.action, expected);
            assert_eq!(options.filter.as_deref(), Some("quote"));
            assert!(options.exact);
        }
        let error =
            parse_cli(&["test", "--exact", "x.test.ko"]).expect_err("--exact needs --filter");
        assert_eq!(
            error.kind(),
            clap::error::ErrorKind::MissingRequiredArgument
        );
        let error = parse_cli(&["test", "run", "--format", "text", "x.test.ko"])
            .expect_err("retired format spelling");
        assert_eq!(error.kind(), clap::error::ErrorKind::InvalidValue);
        let error = parse_cli(&["test", "--json", "x.test.ko"]).expect_err("retired --json flag");
        assert_eq!(error.kind(), clap::error::ErrorKind::UnknownArgument);
        let error =
            parse_cli(&["test", "profile", "x.test.ko"]).expect_err("`profile` is now `trace`");
        assert_ne!(error.kind(), clap::error::ErrorKind::DisplayHelp);
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
        let source = r#"
                    seiyaku Vault {
                        error enum VaultError { Empty = 7 }
                        state int balance;
                        始まり() { balance = 0; }
                        kaizen() {}
                        言挙げ fn deposit(int amount) authorize("CanDeposit") {
                            require(amount > 0, VaultError::Empty);
                            balance = balance + amount;
                        }
                        view fn read() -> int { return balance; }
                    }
                "#;
        let output = CompilerSession::default()
            .build(CompileRequest {
                source,
                source_name: Some("vault.ko"),
            })
            .expect("compile documentation fixture");
        let markdown = render_contract_documentation(
            &output.manifest,
            &DocumentationContext::new(&[], Some(source)),
        );
        for expected in [
            "# Vault",
            "## `kotoage` / `言挙げ` (authorized public mutations)",
            "### `言挙げ fn deposit(int amount) authorize(\"CanDeposit\")`",
            "### `始まり()`",
            "### `kaizen()`",
            "### `view fn read() -> int`",
            "Declared with `言挙げ`: an authorized call",
            "## Views (read-only calls)",
            "Declared with `view`: a read-only call.",
            "Arguments: none (send `{}`).",
            "## Lifecycle: `hajimari` / `始まり`",
            "Declared with `始まり`: the one-shot activation hook",
            "Declared with `kaizen`: the migration hook",
            "Authorization: `CanDeposit`",
            "for example `{\"amount\": \"0\"}`",
            "`int`, `decimal` and `quantity` values are canonical decimal strings",
            "## Durable state",
            "`int` `balance`",
            "## Seiyaku errors",
            "`Vault::VaultError::Empty` = `7`",
        ] {
            assert!(
                markdown.contains(expected),
                "generated documentation omitted {expected:?}:\n{markdown}"
            );
        }
        for absent in [
            "deposit(amount: int)",
            "deposit(int amount) -> ()",
            "External arguments use a JSON record",
        ] {
            assert!(
                !markdown.contains(absent),
                "unexpected {absent:?}:\n{markdown}"
            );
        }
        let runtime = markdown
            .find("## Runtime errors")
            .expect("runtime error appendix");
        assert!(
            markdown
                .find("kotodama::")
                .is_some_and(|first| first > runtime),
            "compiler-owned errors stay in the appendix:\n{markdown}"
        );
    }
    #[test]
    fn documentation_states_only_the_encodings_a_record_uses() {
        let source = "seiyaku Notes { view fn greet(string who) -> string { return who; } view fn pick(Option<int> limit) -> int { return 1; } }";
        let output = CompilerSession::default()
            .build(CompileRequest {
                source,
                source_name: Some("notes.ko"),
            })
            .expect("compile encoding documentation fixture");
        let schema = |name: &str| {
            output
                .manifest
                .entrypoints
                .as_deref()
                .unwrap_or_default()
                .iter()
                .find(|entrypoint| entrypoint.name == name)
                .and_then(|entrypoint| entrypoint.argument_schema.clone())
                .expect("argument schema")
        };
        assert_eq!(argument_encoding_notes(&schema("greet")), "");
        assert_eq!(
            argument_encoding_notes(&schema("pick")),
            " `int`, `decimal` and `quantity` values are canonical decimal strings. Options are `{\"some\": value}` or `{\"none\": true}`."
        );
        let markdown = render_contract_documentation(
            &output.manifest,
            &DocumentationContext::new(&[], Some(source)),
        );
        assert!(
            markdown.contains("for example `{\"who\": \"\"}`.\n"),
            "{markdown}"
        );
    }
    #[test]
    fn documentation_groups_compiler_owned_errors_separately() {
        let source = "seiyaku Rates { view fn ratio(decimal left, decimal right) -> decimal { return left / right; } }";
        let output = CompilerSession::default()
            .build(CompileRequest {
                source,
                source_name: Some("rates.ko"),
            })
            .expect("compile numeric documentation fixture");
        let markdown = render_contract_documentation(
            &output.manifest,
            &DocumentationContext::new(&[], Some(source)),
        );
        if markdown.contains("kotodama::") {
            let runtime = markdown
                .find("## Runtime errors")
                .expect("runtime error heading");
            let first = markdown.find("kotodama::").expect("compiler-owned error");
            assert!(first > runtime, "{markdown}");
        }
        assert!(!markdown.contains("## Seiyaku errors"), "{markdown}");
        assert!(
            !markdown.contains("## `kotoage`"),
            "no kotoage section without kotoage:\n{markdown}"
        );
        assert!(
            markdown.contains("for example `{\"left\": \"0.0\", \"right\": \"0.0\"}`"),
            "{markdown}"
        );
    }
    #[test]
    fn source_documentation_preserves_call_modes_and_named_external_records() {
        use kotodama_lang::editor::EditorSnapshot;
        for (declaration, mode) in [("int amount", "named"), ("int _ amount", "positional")] {
            let source =
                format!("seiyaku Labels {{ view fn echo({declaration}) -> int {{ amount }} }}");
            let snapshot = EditorSnapshot::single("labels.ko", &source, false);
            let signatures = snapshot.declaration_signatures(SourceId(0));
            let output = CompilerSession::default()
                .build(CompileRequest {
                    source: &source,
                    source_name: Some("labels.ko"),
                })
                .expect("compile source documentation fixture");
            let markdown = render_contract_documentation(
                &output.manifest,
                &DocumentationContext::new(&signatures, Some(&source)),
            );
            assert!(
                markdown.contains(&format!("### `view fn echo({declaration}) -> int`")),
                "{markdown}"
            );
            assert!(markdown.contains("`{\"amount\": \"0\"}`"), "{markdown}");
            let json = contract_documentation_json(&output.manifest, &signatures).unwrap();
            let echo = json
                .get("source_signatures")
                .and_then(norito::json::Value::as_array)
                .unwrap()
                .iter()
                .find(|value| {
                    value.get("name").and_then(norito::json::Value::as_str) == Some("echo")
                })
                .unwrap();
            let parameter = &echo
                .get("parameters")
                .and_then(norito::json::Value::as_array)
                .unwrap()[0];
            assert_eq!(
                parameter
                    .get("call_mode")
                    .and_then(norito::json::Value::as_str),
                Some(mode)
            );
            assert_eq!(
                output.manifest.entrypoints.as_ref().unwrap()[0].params[0].name,
                "amount"
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
            branded, "誓約 Demo {\n    言挙げ fn run() authorize(\"Run\") {}\n}\n",
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
        let Cli {
            command: KotoCommand::Check(options),
        } = parse_cli(&[
            "check",
            "--format",
            "sarif",
            "--chain-discriminant",
            "369",
            "--zk",
            "--project",
            "kotodama.project.json",
        ])
        .expect("parse check options")
        else {
            panic!("expected check");
        };
        assert_eq!(options.format, DiagnosticFormat::Sarif);
        assert!(options.capabilities.zk);
        assert_eq!(options.capabilities.chain_discriminant(), 369);
        assert_eq!(
            options.selection.project,
            Some(PathBuf::from("kotodama.project.json"))
        );
        assert!(options.sources.is_empty());
        let error = parse_cli(&["check", "--project", "p.json", "a.ko"])
            .expect_err("--project excludes positional sources");
        assert_eq!(error.kind(), clap::error::ErrorKind::ArgumentConflict);
        let error = parse_cli(&["check", "--project", "p.json", "--source-root", "."])
            .expect_err("--project excludes --source-root");
        assert_eq!(error.kind(), clap::error::ErrorKind::ArgumentConflict);
        let error = parse_cli(&["check", "--format", "text", "a.ko"]).expect_err("retired alias");
        assert_eq!(error.kind(), clap::error::ErrorKind::InvalidValue);
    }
    #[test]
    fn chain_discriminant_option_is_strict_and_nonzero() {
        assert_eq!(parse_chain_discriminant("369").expect("Taira value"), 369);
        assert_eq!(
            parse_chain_discriminant("65535").expect("maximum u16 value"),
            u16::MAX
        );
        for invalid in ["", "0", "0369", "+369", "-1", "369x", "65536"] {
            assert!(
                parse_chain_discriminant(invalid).is_err(),
                "accepted invalid discriminant {invalid:?}"
            );
        }
        let duplicate = parse_cli(&[
            "check",
            "--chain-discriminant",
            "369",
            "--chain-discriminant",
            "753",
            "contract.ko",
        ])
        .expect_err("duplicate option must fail closed");
        assert_eq!(duplicate.kind(), clap::error::ErrorKind::ArgumentConflict);
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
    fn build_errors_preserve_identical_canonical_fields_in_every_renderer() {
        let primary = SourceSpan {
            package_identity: None,
            source: Some("modules/app.ko".to_owned()),
            start: SourcePosition { line: 3, column: 9 },
            end: SourcePosition {
                line: 3,
                column: 22,
            },
            byte_range: Some(TextRange::new(41, 54)),
        };
        let related = SourceSpan {
            package_identity: None,
            source: Some("modules/math.ko".to_owned()),
            start: SourcePosition {
                line: 1,
                column: 18,
            },
            end: SourcePosition {
                line: 1,
                column: 24,
            },
            byte_range: Some(TextRange::new(17, 23)),
        };
        let mut diagnostic = Diagnostic::error(
            "E_UNEXPORTED_SYMBOL",
            DiagnosticPhase::Resolve,
            "source `modules/app.ko` cannot call unexported symbol `math::hidden`",
            Some(primary.clone()),
        );
        diagnostic.labels.push(DiagnosticLabel {
            span: related,
            message: "the private declaration is here".to_owned(),
        });
        diagnostic
            .notes
            .push("imports are explicit in V1".to_owned());
        diagnostic.fix = Some(DiagnosticFix {
            span: primary,
            replacement: "math::visible".to_owned(),
        });
        let diagnostics = DiagnosticBundle::single(diagnostic.clone());
        let human = build_error(
            DiagnosticFormat::Human,
            BuildError::Compile(diagnostics.clone()),
        )
        .to_string();
        for expected in [
            "error[E_UNEXPORTED_SYMBOL] resolve",
            "--> modules/app.ko:3:9",
            "= label: modules/math.ko:1:18: the private declaration is here",
            "imports are explicit in V1",
            "= help:",
            "= fix:",
            "math::visible",
        ] {
            assert!(
                human.contains(expected),
                "human diagnostics omitted {expected:?}"
            );
        }
        let json: norito::json::Value = norito::json::from_str(
            &build_error(
                DiagnosticFormat::Json,
                BuildError::Compile(diagnostics.clone()),
            )
            .to_string(),
        )
        .expect("build JSON diagnostics");
        let sarif: norito::json::Value = norito::json::from_str(
            &build_error(DiagnosticFormat::Sarif, BuildError::Compile(diagnostics)).to_string(),
        )
        .expect("build SARIF diagnostics");
        let canonical = diagnostic.to_json_value();
        assert_eq!(
            json.as_array().and_then(|items| items.first()),
            Some(&canonical)
        );
        assert_eq!(
            sarif.pointer("/runs/0/results/0/properties/kotodama"),
            Some(&canonical),
        );
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
            "seiyaku Lint { fn helper(int unused) -> int { return 1; } view fn value() -> int { return helper(unused: 0); } }",
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
            kotodama_lang::diagnostic::diagnostic_explanation("K5003").is_some(),
            "every unified lint code must work with koto explain",
        );
        std::fs::remove_dir_all(root).expect("remove lint check root");
    }
    #[test]
    fn lint_flags_validate_names_and_select_one_level_per_lint() {
        let flags = LintArgs {
            deny_warnings: false,
            allow: vec!["unused-local".to_owned()],
            warn: Vec::new(),
            deny: vec!["dead-store".to_owned()],
        };
        let config = flags.config().expect("known lints");
        assert_eq!(config.level("unused-local"), LintLevel::Allow);
        assert_eq!(config.level("dead-store"), LintLevel::Deny);
        assert_eq!(config.level("unused-state"), LintLevel::Warn);
        let conflicting = LintArgs {
            deny: vec!["unused-local".to_owned()],
            ..flags.clone()
        };
        assert!(matches!(
            conflicting.config(),
            Err(KotoError::Usage(message)) if message.contains("both `--allow` and `--deny`")
        ));
        let unknown = LintArgs {
            allow: vec!["unused-locl".to_owned()],
            ..LintArgs::default()
        };
        assert!(matches!(
            unknown.config(),
            Err(KotoError::Usage(message)) if message.contains("did you mean `unused-local`?")
        ));
        let denying = LintArgs {
            deny_warnings: true,
            ..LintArgs::default()
        }
        .config()
        .expect("deny-warnings");
        let warnings = CompilerSession::default()
            .check_with_lints(CompileRequest {
                source: "seiyaku L { view fn one() -> int { let unused = 1; return 1; } }",
                source_name: Some("l.ko"),
            })
            .expect("lints do not fail the check");
        let unused = warnings
            .into_iter()
            .find(|warning| warning.code == "unused-local")
            .expect("unused local");
        let denied = leveled_lint(&denying, unused.clone()).expect("denied lints are reported");
        assert_eq!(denied.severity, kotodama_lang::lint::LintSeverity::Error);
        assert!(
            leveled_lint(&config, unused).is_none(),
            "allowed lints are dropped"
        );
    }
    #[test]
    fn rooted_diagnostics_name_existing_sources_relative_to_the_working_directory() {
        let cwd = std::env::current_dir().expect("working directory");
        let root = cwd.join("src");
        let span = |source: &str| SourceSpan {
            package_identity: None,
            source: Some(source.to_owned()),
            start: SourcePosition { line: 1, column: 1 },
            end: SourcePosition { line: 1, column: 2 },
            byte_range: None,
        };
        let mut diagnostic = Diagnostic::error(
            "K1001",
            DiagnosticPhase::Parse,
            "message",
            Some(span("lib.rs")),
        );
        diagnostic.alternative_fixes.push(DiagnosticFix {
            span: span("missing.ko"),
            replacement: String::new(),
        });
        remap_rooted_diagnostic_sources(&mut diagnostic, &root);
        assert_eq!(
            diagnostic
                .primary_span
                .and_then(|span| span.source)
                .as_deref(),
            Some("src/lib.rs")
        );
        assert_eq!(
            diagnostic.alternative_fixes[0].span.source.as_deref(),
            Some("missing.ko"),
            "names that are not files below the root stay logical"
        );
    }
    #[test]
    fn unified_check_links_only_the_explicit_locked_project_graph() {
        let root = std::env::temp_dir().join(format!(
            "koto-check-project-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).expect("create project check root");
        let app = root.join("app.ko");
        let module = root.join("math.ko");
        let project = root.join("kotodama.project.json");
        std::fs::write(
            &app,
            "seiyaku App { view fn run() -> int { return Math::value(unused: 1); } }",
        )
        .expect("write project root");
        std::fs::write(
            &module,
            "module Math { export fn value(int unused) -> int { return 7; } }",
        )
        .expect("write project module");
        std::fs::write(
            &project,
            r#"{
                "version": 1,
                "root": "app.ko",
                "imports": [{"alias": "Math", "package": "example/math@1.0.0"}],
                "packages": [{
                    "identity": "example/math@1.0.0",
                    "modules": ["math.ko"],
                    "exports": ["value"],
                    "imports": []
                }]
            }"#,
        )
        .expect("write explicit project manifest");
        let driver = BuildDriver::new(CompilerSession::default(), "koto-check-test");
        let (checked, diagnostics) =
            check_locked_project(&driver, &project, &LintConfig::default());
        let canonical_app = app.canonicalize().expect("canonical app path");
        let canonical_module = module.canonicalize().expect("canonical module path");
        assert_eq!(
            checked,
            vec![canonical_app.clone(), canonical_module.clone()]
        );
        assert!(
            diagnostics
                .diagnostics
                .iter()
                .all(|diagnostic| { diagnostic.severity != Severity::Error })
        );
        let warning = diagnostics
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == "K5003")
            .expect("module lint warning");
        assert_eq!(
            warning
                .primary_span
                .as_ref()
                .and_then(|span| span.source.as_deref()),
            canonical_module.to_str()
        );
        assert_eq!(
            warning
                .primary_span
                .as_ref()
                .and_then(|span| span.package_identity.as_deref()),
            Some("example/math@1.0.0")
        );
        let (checked, positional) = check_project_paths(&driver, vec![app.clone(), module.clone()]);
        assert!(checked.is_empty());
        assert!(
            positional
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "E_PROJECT_MANIFEST_REQUIRED")
        );
        std::fs::write(
            &app,
            "seiyaku App { view fn run() -> int { return Missing::value(); } }",
        )
        .expect("write unknown module call");
        let (checked, diagnostics) =
            check_locked_project(&driver, &project, &LintConfig::default());
        assert!(checked.is_empty());
        let error = diagnostics
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == "E_UNKNOWN_IMPORT_ALIAS")
            .expect("unknown module alias diagnostic");
        assert_eq!(
            error
                .primary_span
                .as_ref()
                .and_then(|span| span.source.as_deref()),
            canonical_app.to_str()
        );
        std::fs::remove_dir_all(root).expect("remove project check root");
    }
    #[test]
    fn unified_check_rejects_multiple_explicit_roots_with_physical_spans() {
        let root = std::env::temp_dir().join(format!(
            "koto-check-roots-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).expect("create multiple-root check directory");
        let first = root.join("a.ko");
        let second = root.join("b.ko");
        std::fs::write(&first, "seiyaku A { view fn value() -> int { return 1; } }")
            .expect("write first root");
        std::fs::write(
            &second,
            "seiyaku B { view fn value() -> int { return 2; } }",
        )
        .expect("write second root");
        let driver = BuildDriver::new(CompilerSession::default(), "koto-check-test");
        let (checked, diagnostics) =
            check_project_paths(&driver, vec![second.clone(), first.clone()]);
        assert!(checked.is_empty());
        let diagnostic = diagnostics
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == "E_MULTIPLE_SEIYAKU_ROOTS")
            .expect("multiple-root diagnostic");
        assert_eq!(
            diagnostic
                .primary_span
                .as_ref()
                .and_then(|span| span.source.as_deref()),
            first.to_str()
        );
        assert_eq!(diagnostic.labels.len(), 1);
        assert_eq!(diagnostic.labels[0].span.source.as_deref(), second.to_str());
        std::fs::remove_dir_all(root).expect("remove multiple-root check directory");
    }
    #[test]
    fn formatter_options_fail_closed_and_allow_dash_paths_after_separator() {
        let error = parse_cli(&["fmt", "--write", "demo.ko"])
            .expect_err("unknown formatter flags must not become file paths");
        assert_eq!(error.kind(), clap::error::ErrorKind::UnknownArgument);
        let error = parse_cli(&["fmt", "--check", "--check", "demo.ko"])
            .expect_err("duplicate formatter options must fail closed");
        assert_eq!(error.kind(), clap::error::ErrorKind::ArgumentConflict);
        let Cli {
            command: KotoCommand::Fmt(args),
        } = parse_cli(&["fmt", "--check", "--", "--literal-name.ko"])
            .expect("separator permits a leading-dash file name")
        else {
            panic!("expected fmt");
        };
        assert!(args.check);
        assert_eq!(args.paths, vec![PathBuf::from("--literal-name.ko")]);
    }
    #[test]
    fn formatter_directories_expand_recursively_to_sorted_sources() {
        let root = std::env::temp_dir().join(format!(
            "koto-fmt-dir-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos()
        ));
        for directory in ["contracts", "tests/nested", "target/kotodama", ".hidden"] {
            std::fs::create_dir_all(root.join(directory)).expect("create fmt tree");
        }
        for file in [
            "contracts/b.ko",
            "contracts/a.ko",
            "tests/nested/c.test.ko",
            "target/kotodama/generated.ko",
            ".hidden/skip.ko",
            "contracts/readme.md",
        ] {
            std::fs::write(root.join(file), "seiyaku Demo {}\n").expect("write fmt input");
        }
        let inputs = collect_format_inputs(std::slice::from_ref(&root)).expect("expand directory");
        let relative = inputs
            .iter()
            .map(|path| path.strip_prefix(&root).expect("inside root").to_path_buf())
            .collect::<Vec<_>>();
        assert_eq!(
            relative,
            [
                PathBuf::from("contracts/a.ko"),
                PathBuf::from("contracts/b.ko"),
                PathBuf::from("tests/nested/c.test.ko"),
            ]
        );
        let explicit = root.join("target/kotodama/generated.ko");
        assert_eq!(
            collect_format_inputs(std::slice::from_ref(&explicit)).expect("explicit file"),
            vec![explicit]
        );
        assert!(matches!(
            collect_format_inputs(&[root.join("missing")]),
            Err(KotoError::Io(_))
        ));
        std::fs::remove_dir_all(root).expect("remove fmt tree");
    }
    fn build_args(args: &[&str]) -> BuildArgs {
        let mut full = vec!["build"];
        full.extend_from_slice(args);
        match parse_cli(&full).expect("parse build arguments").command {
            KotoCommand::Build(args) => args,
            other => panic!("expected build, got {other:?}"),
        }
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
            "module Math { export fn add(int left, int right) -> int { return left + right; } }",
        )
        .expect("write module source");
        let error = build(build_args(&[
            "--target-dir",
            &target.display().to_string(),
            &source.display().to_string(),
        ]))
        .expect_err("standalone module build must fail");
        assert!(
            error.to_string().contains("E_ROOT_MUST_BE_SEIYAKU"),
            "unexpected error: {error}"
        );
        assert!(!target.join("dev/math.to").exists());
        let session = CompilerSession::default();
        check_path(&session, &source).expect("module remains valid for koto check");
        std::fs::remove_dir_all(root).expect("remove module test root");
    }
    #[test]
    fn build_project_uses_the_same_explicit_locked_graph_as_check() {
        let root = std::env::temp_dir().join(format!(
            "koto-project-build-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos()
        ));
        let target = root.join("target/kotodama");
        std::fs::create_dir_all(root.join("contracts")).expect("create contract directory");
        std::fs::create_dir_all(root.join("modules")).expect("create module directory");
        std::fs::write(
            root.join("contracts/app.ko"),
            "seiyaku App { view fn run() -> int { return Math::value(); } }",
        )
        .expect("write root source");
        std::fs::write(
            root.join("modules/math.ko"),
            "module Math { export fn value() -> int { return 7; } }",
        )
        .expect("write module source");
        let project = root.join("kotodama.project.json");
        std::fs::write(
            &project,
            r#"{
                "version": 1,
                "root": "contracts/app.ko",
                "imports": [{"alias": "Math", "package": "example/math@1.0.0"}],
                "packages": [{
                    "identity": "example/math@1.0.0",
                    "modules": ["modules/math.ko"],
                    "exports": ["value"],
                    "imports": []
                }]
            }"#,
        )
        .expect("write project manifest");
        build(build_args(&[
            "--target-dir",
            &target.display().to_string(),
            "--project",
            &project.display().to_string(),
        ]))
        .expect("build exact project graph");
        assert!(target.join("dev/app.to").is_file());
        let malformed = std::fs::read_to_string(&project)
            .expect("read project manifest")
            .replace("\"exports\": [\"value\"]", "\"exports\": []");
        std::fs::write(&project, malformed).expect("remove exact export");
        let error = build(build_args(&[
            "--target-dir",
            &target.display().to_string(),
            "--project",
            &project.display().to_string(),
        ]))
        .expect_err("build must reject an undeclared export");
        assert!(error.to_string().contains("E_UNEXPORTED_SYMBOL"), "{error}");
        std::fs::remove_dir_all(root).expect("remove project build root");
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
                .pointer("/capabilities/codeActionProvider/codeActionKinds/0")
                .and_then(norito::json::Value::as_str),
            Some("quickfix"),
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
            "get_bytes_hex",
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
        let indexed = "seiyaku C { fn write() { var List<int, 2> values = [1]; values[0] = 2; } }";
        let indexed_actions = lsp_code_action_items(&session, uri, indexed);
        let indexed_action = indexed_actions
            .as_array()
            .expect("code action array")
            .iter()
            .find(|action| {
                action
                    .pointer("/diagnostics/0/code")
                    .and_then(norito::json::Value::as_str)
                    == Some("E_LIST_UNSAFE_INDEX")
            })
            .expect("checked-list quick fix");
        assert_eq!(
            indexed_action
                .pointer("/kind")
                .and_then(norito::json::Value::as_str),
            Some("quickfix")
        );
        let indexed_edit = indexed_action
            .pointer("/edit/changes")
            .and_then(|changes| changes.get(uri))
            .and_then(norito::json::Value::as_array)
            .and_then(|edits| edits.first())
            .expect("checked-list workspace edit");
        assert_eq!(
            indexed_edit
                .get("newText")
                .and_then(norito::json::Value::as_str),
            Some("values.set(index: 0, value: 2);")
        );
        let start = indexed_edit
            .pointer("/range/start/character")
            .and_then(norito::json::Value::as_u64)
            .expect("indexed edit start") as usize;
        let end = indexed_edit
            .pointer("/range/end/character")
            .and_then(norito::json::Value::as_u64)
            .expect("indexed edit end") as usize;
        assert_eq!(&indexed[start..end], "values[0] = 2;");
        let unresolved = "seiyaku C { fn f() { target(1, second: 2); } }";
        let unresolved_diagnostics = collect_lsp_diagnostics(&session, uri, unresolved);
        assert!(unresolved_diagnostics.diagnostics.iter().any(|diagnostic| {
            diagnostic.severity == Severity::Error && diagnostic.fix.is_none()
        }));
        let unresolved_actions = lsp_code_action_items(&session, uri, unresolved);
        assert!(
            unresolved_actions
                .as_array()
                .expect("code action array")
                .is_empty(),
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
            "module Math { export fn value() -> int { return 1; } }",
        );
        assert!(
            module.is_empty(),
            "valid reusable modules must not receive deployable-only K4003: {module:?}",
        );
        let invalid = lsp_diagnostics(
            &session,
            "file:///workspace/broken.ko",
            "module Broken { export fn value( -> int { return 1; } }",
        );
        assert!(!invalid.is_empty());
    }
    #[test]
    fn lsp_open_documents_never_infer_cross_file_graph_authority() {
        let driver = BuildDriver::new(CompilerSession::default(), "lsp-test");
        let app_uri = "file:///workspace/app.ko";
        let module_uri = "file:///workspace/math.ko";
        let documents = HashMap::from([
            (
                app_uri.to_owned(),
                "seiyaku App { view fn run() -> int { return Math::value(); } }".to_owned(),
            ),
            (
                module_uri.to_owned(),
                "module Math { export fn value() -> int { return 1; } }".to_owned(),
            ),
        ]);
        let diagnostics = collect_lsp_project_diagnostics(&driver, &documents);
        let diagnostic = diagnostics[app_uri]
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == "E_PROJECT_MANIFEST_REQUIRED")
            .expect("open root and module must require explicit graph authority");
        let span = diagnostic.primary_span.as_ref().expect("exact call span");
        assert_eq!(span.source.as_deref(), Some(app_uri));
        assert!(
            diagnostic
                .help
                .as_deref()
                .is_some_and(|help| help.contains("--project"))
        );
        assert!(diagnostics[module_uri].diagnostics.is_empty());
    }
    #[test]
    fn lsp_project_uses_open_overlays_on_the_exact_locked_graph() {
        let root = std::env::temp_dir().join(format!(
            "koto-lsp-project-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).expect("create LSP project root");
        let app = root.join("app.ko");
        let module = root.join("math.ko");
        let manifest = root.join("kotodama.project.json");
        std::fs::write(
            &app,
            "seiyaku App { view fn run() -> int { return Math::value(); } }",
        )
        .expect("write valid project root");
        std::fs::write(
            &module,
            "module Math { export fn value() -> int { return 7; } }",
        )
        .expect("write project module");
        std::fs::write(
            &manifest,
            r#"{
                "version": 1,
                "root": "app.ko",
                "imports": [{"alias": "Math", "package": "example/math@1.0.0"}],
                "packages": [{
                    "identity": "example/math@1.0.0",
                    "modules": ["math.ko"],
                    "exports": ["value"],
                    "imports": []
                }]
            }"#,
        )
        .expect("write exact LSP project manifest");
        let project = load_source_project_manifest(&manifest).expect("load exact LSP project");
        let app_uri = format!(
            "file://{}",
            app.canonicalize().expect("canonical app path").display()
        );
        let overlay = "seiyaku App { view fn run() -> int { return Math::missing(); } }".to_owned();
        let documents = HashMap::from([(app_uri.clone(), overlay.clone())]);
        let driver = BuildDriver::new(CompilerSession::default(), "lsp-project-test");
        let diagnostics = collect_lsp_workspace_diagnostics(&driver, &documents, Some(&project));
        let diagnostic = diagnostics[&app_uri]
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == "E_UNEXPORTED_SYMBOL")
            .expect("open root overlay is checked against the locked package export set");
        let span = diagnostic
            .primary_span
            .as_ref()
            .expect("exact overlay span");
        assert_eq!(span.source.as_deref(), Some(app_uri.as_str()));
        assert!(span.package_identity.is_none());
        let range = span.byte_range.expect("overlay byte range");
        let start = usize::try_from(range.start).expect("range start fits usize");
        let end = usize::try_from(range.end).expect("range end fits usize");
        assert_eq!(&overlay[start..end], "Math::missing");
        assert!(
            diagnostics[&app_uri]
                .diagnostics
                .iter()
                .all(|diagnostic| diagnostic.code != "E_PROJECT_MANIFEST_REQUIRED"),
            "an explicit LSP project must provide graph authority"
        );
        // Unopened dependencies stay in the semantic graph and retain their exact text.
        let documents = HashMap::new();
        let workspace = editor_lsp::Workspace::new(&documents, Some(&project), &app_uri, false);
        let root_source = &project.graph.root.source;
        let request = norito::json!({"params": {"textDocument": {"uri": (app_uri.clone())}, "position": {"line": 0, "character": (root_source.find("value()").unwrap())}, "context": {"includeDeclaration": true}, "newName": "renamed"}});
        let module_uri = lsp_path_file_uri(&module.canonicalize().unwrap()).unwrap();
        let definition = workspace
            .response("textDocument/definition", &request)
            .unwrap();
        assert_eq!(
            definition
                .pointer("/uri")
                .and_then(norito::json::Value::as_str),
            Some(module_uri.as_str())
        );
        let references = workspace
            .response("textDocument/references", &request)
            .unwrap();
        assert_eq!(references.as_array().unwrap().len(), 2);
        let renamed = workspace.response("textDocument/rename", &request).unwrap();
        assert_eq!(
            renamed
                .pointer("/documentChanges")
                .unwrap()
                .as_array()
                .unwrap()
                .len(),
            3,
            "owned exports rename the root reference, declaration, and exact manifest token together"
        );
        // Unopened source changes are reloaded from disk under the same locked manifest.
        let invalid_source =
            "module Math { /* 金庫😀 */ export fn value() -> int { return missing; } }";
        std::fs::write(&module, invalid_source).expect("write unopened dependency error");
        let diagnostics = collect_lsp_workspace_diagnostics(&driver, &documents, Some(&project));
        let diagnostic = diagnostics[&module_uri]
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == "K2002")
            .expect("unopened dependency error");
        assert_eq!(
            diagnostic.primary_source.as_ref().unwrap().text(),
            invalid_source
        );
        let rendered = lsp_diagnostic_value(diagnostic, "");
        let expected_character = invalid_source[..invalid_source.find("missing").unwrap()]
            .encode_utf16()
            .count() as u64;
        assert_eq!(
            rendered
                .pointer("/range/start/character")
                .and_then(norito::json::Value::as_u64),
            Some(expected_character)
        );
        std::fs::write(
            &module,
            "module Math { /* 金庫😀 */ export fn value() -> int { 7 } fn helper(int unused) -> int { 1 } }",
        )
        .expect("write unopened dependency lint");
        let diagnostics = collect_lsp_workspace_diagnostics(&driver, &documents, Some(&project));
        let warning = diagnostics[&module_uri]
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == "K5003")
            .expect("unopened dependency lint");
        assert_eq!(
            warning.primary_source.as_ref().unwrap().package_identity(),
            Some("example/math@1.0.0")
        );
        let captured = warning.primary_source.as_ref().unwrap().text();
        let range = warning.primary_span.as_ref().unwrap().byte_range.unwrap();
        assert_eq!(
            &captured[range.start as usize..range.end as usize],
            "unused"
        );
        assert_eq!(
            lsp_diagnostic_value(warning, "")
                .pointer("/range/start/character")
                .and_then(norito::json::Value::as_u64),
            Some(captured[..range.start as usize].encode_utf16().count() as u64)
        );
        std::fs::remove_dir_all(root).expect("remove LSP project root");
    }
    #[test]
    fn lsp_diagnostics_project_scalar_columns_and_captured_related_sources_to_utf16() {
        let source = SourceFile::new(SourceId(0), "file:///日本語.ko", "金庫😀x\n次😀y");
        let primary = SourceSpan {
            package_identity: None,
            source: Some(source.name().to_owned()),
            start: SourcePosition { line: 1, column: 4 },
            end: SourcePosition { line: 1, column: 5 },
            byte_range: None,
        };
        let mut diagnostic = Diagnostic::error(
            "K2002",
            DiagnosticPhase::Resolve,
            "unresolved x",
            Some(primary),
        );
        diagnostic.labels.push(DiagnosticLabel {
            span: SourceSpan {
                package_identity: None,
                source: Some(source.name().to_owned()),
                start: SourcePosition { line: 2, column: 3 },
                end: SourcePosition { line: 2, column: 4 },
                byte_range: None,
            },
            message: "関連する定義 y".to_owned(),
        });
        diagnostic
            .notes
            .push("この値は現在のスコープにありません。".to_owned());
        diagnostic.help = Some("使う前に値を宣言してください。".to_owned());
        diagnostic.capture_source(&source);
        let rendered = lsp_diagnostic_value(&diagnostic, "the editor has a newer buffer");
        assert_eq!(rendered.pointer("/range"), Some(&lsp_range(0, 4, 0, 5)));
        assert_eq!(
            rendered.pointer("/relatedInformation/0/location/range"),
            Some(&lsp_range(1, 3, 1, 4))
        );
        assert_eq!(
            rendered
                .pointer("/message")
                .and_then(norito::json::Value::as_str),
            Some(
                "unresolved x\n\nnote: この値は現在のスコープにありません。\n\nhelp: 使う前に値を宣言してください。"
            )
        );
        assert_eq!(
            rendered
                .pointer("/relatedInformation/0/message")
                .and_then(norito::json::Value::as_str),
            Some("関連する定義 y")
        );
        assert_eq!(
            lsp_source_position("", &SourcePosition { line: 3, column: 9 }),
            (2, 8)
        );
    }
    #[test]
    fn lsp_options_require_one_explicit_project_value() {
        let Cli {
            command: KotoCommand::Lsp(args),
        } = parse_cli(&["lsp", "--zk", "--project", "kotodama.project.json"])
            .expect("parse exact LSP project")
        else {
            panic!("expected lsp");
        };
        assert!(args.zk);
        assert_eq!(
            args.selection.project,
            Some(PathBuf::from("kotodama.project.json"))
        );
        assert_eq!(
            parse_cli(&["lsp", "--project"])
                .expect_err("missing project path")
                .kind(),
            clap::error::ErrorKind::InvalidValue
        );
        assert_eq!(
            parse_cli(&["lsp", "--project", "a.json", "--project", "b.json"])
                .expect_err("duplicate project path")
                .kind(),
            clap::error::ErrorKind::ArgumentConflict
        );
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
