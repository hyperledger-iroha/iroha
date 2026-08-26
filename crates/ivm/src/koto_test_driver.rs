//! In-process Kotodama V1 test runner shared by the unified CLI and SDK tools.
#[cfg(test)]
use crate::ProgramMetadata;
use crate::{
    AccountId, AssetDefinitionId, DomainId, IVM, IVMHost, MockWorldStateView, PermissionToken,
    PointerType, TraceMode, WsvHost,
    kotodama::{
        ast::{Expr, FixtureAction, FixtureDecl, FunctionKind, Item, Program, SourceUnitKind},
        compiler::{CompileReport, CompilerMode, CompilerOptions},
        linker::{
            ImportBinding, MAX_LOGICAL_SOURCE_PATH_BYTES, MAX_MODULE_GRAPH_SOURCE_BYTES,
            ModuleBuildGraph, SourceLinkRequest, SourceModuleUnit, SourcePackageUnit,
        },
        parser,
        session::{CompilerSession, TestCompileOutput, TestSourceUnit},
        source::read_source_file,
    },
};
#[cfg(test)]
use ed25519_dalek::{Signature as Ed25519Signature, Verifier as _};
use ed25519_dalek::{Signer as _, SigningKey};
use iroha_data_model::prelude::{Mintable, Name};
use iroha_data_model::{
    account::address::ChainDiscriminantGuard,
    asset::{AssetBalanceScope, AssetId},
    nexus::DataSpaceId,
    smart_contract::ContractAddress,
};
#[cfg(test)]
use iroha_primitives::numeric_abi::QuantityValueV1;
use iroha_primitives::{
    json::Json,
    numeric::{Numeric, Quantity},
    numeric_abi::DecimalValueV1,
};
use ivm_abi::entrypoint::EntrypointArgumentSchemaV1;
use ivm_abi::state_value::{
    StateValueAtomV1, StateValueKindV1, StateValueNodeV1, StateValueRecordV1, StateValueSchemaV1,
    state_value_schema_hash_v1,
};
use norito::codec::Encode;
use norito::json::{self, Value};
use std::{
    any::Any,
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
    time::{Duration, Instant},
};
const DEFAULT_CALLER: &str = "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV";
const ENTRYPOINT_IMPL_PREFIX: &str = "__entrypoint_impl__";
const TEST_SYSCALL_ACTOR_ACCOUNT: u32 = crate::syscalls::SYSCALL_KOTO_TEST_ACTOR_ACCOUNT;
const TEST_SYSCALL_ACTOR_PUBLIC_KEY: u32 = crate::syscalls::SYSCALL_KOTO_TEST_ACTOR_PUBLIC_KEY;
const TEST_SYSCALL_ACTOR_SIGN: u32 = crate::syscalls::SYSCALL_KOTO_TEST_ACTOR_SIGN;
const TEST_SYSCALL_INVOKE_ENTRYPOINT_AS: u32 =
    crate::syscalls::SYSCALL_KOTO_TEST_INVOKE_ENTRYPOINT_AS;
const TEST_SYSCALL_EXPECT_REJECT_AS: u32 = crate::syscalls::SYSCALL_KOTO_TEST_EXPECT_REJECT_AS;
const TEST_MAX_RETURN_VALUES: usize = 13;
#[derive(Clone)]
struct FixtureActor {
    account: AccountId,
    seed: Option<[u8; 32]>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Command {
    Run,
    Coverage,
    Profile,
    List,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum TestOutputFormat {
    #[default]
    Text,
    Json,
    Junit,
}
#[derive(Clone, Debug)]
struct TestOptions {
    command: Command,
    path: PathBuf,
    filter: Option<String>,
    exact: bool,
    jobs: usize,
    seed: u64,
    chain_discriminant: u16,
    zk_enabled: bool,
    output: TestOutputFormat,
    output_path: Option<PathBuf>,
}
#[derive(Clone, Debug)]
struct TestCase {
    name: String,
    fixture: Option<String>,
    line: usize,
}
struct DiscoveredSuite {
    target_path: PathBuf,
    target_source: String,
    target_program: Program,
    test_modules: Vec<DiscoveredTestModule>,
    tests: Vec<TestCase>,
    fixtures: HashMap<String, FixtureDecl>,
}
struct DiscoveredTestModule {
    path: PathBuf,
    source: String,
    program: Program,
}
struct CompiledArtifact {
    program: crate::PreparedContract,
    report: CompileReport,
    pc_base: u64,
}
struct CompiledSuite {
    suite: CompiledArtifact,
    runtime: Option<CompiledArtifact>,
    runtime_entrypoints: HashMap<String, RuntimeEntrypoint>,
    tests: Vec<CompiledTestCase>,
    fixtures: HashMap<String, FixtureDecl>,
    coverage_functions: Vec<CoverageFunction>,
}
impl CompiledSuite {
    fn profile_artifact(&self) -> &CompiledArtifact {
        self.runtime.as_ref().unwrap_or(&self.suite)
    }
}
#[derive(Clone)]
struct RuntimeEntrypoint {
    pc: u64,
    argument_schema: Option<EntrypointArgumentSchemaV1>,
    permission: Option<String>,
}
#[derive(Clone)]
struct CompiledTestCase {
    name: String,
    fixture: Option<String>,
    line: usize,
    pc: u64,
}
#[derive(Clone)]
struct CoverageFunction {
    display_name: String,
    line: u32,
    pc_start: u64,
    pc_end: u64,
}
struct TestRunResult {
    name: String,
    line: usize,
    elapsed: Duration,
    passed: bool,
    failure: Option<String>,
    trace_pcs: Vec<u64>,
    delta_trace: Vec<crate::zk::DeltaEntry>,
}
/// One deterministic request for the non-printing Kotodama V1 test runner.
///
/// The runner never writes to stdout or stderr. Frontends retain sole ownership
/// of rendering, which lets them produce one structured output document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KotoTestRunRequestV1 {
    /// Target contract or standalone test module to discover and run.
    pub target: PathBuf,
    /// Optional test-name substring (or exact name when [`Self::exact`] is set).
    pub filter: Option<String>,
    /// Require [`Self::filter`] to match the complete test name.
    pub exact: bool,
    /// Maximum number of independent test workers.
    pub jobs: usize,
    /// Deterministic ordering seed; zero requests canonical name order.
    pub seed: u64,
    /// Account-address chain discriminant used while compiling and executing.
    pub chain_discriminant: u16,
    /// Enable the Kotodama ZK compilation surface.
    pub zk_enabled: bool,
}
impl KotoTestRunRequestV1 {
    /// Construct a canonical single-worker request for `target`.
    #[must_use]
    pub fn new(target: impl Into<PathBuf>, chain_discriminant: u16) -> Self {
        Self {
            target: target.into(),
            filter: None,
            exact: false,
            jobs: 1,
            seed: 0,
            chain_discriminant,
            zk_enabled: false,
        }
    }
}
/// Exact external module graph visible to one declared Kotodama test root.
///
/// Import aliases are parent-local and every package identity must be the exact
/// immutable identity selected by the caller's lock graph. Filesystem module
/// discovery is disabled when this graph is used.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct KotoTestModuleGraphV1 {
    /// Direct aliases visible to the declared test root.
    pub imports: Vec<ImportBinding>,
    /// Complete locked package graph, including transitive modules.
    pub packages: Vec<SourcePackageUnit>,
}
/// Stable stage at which a structured Kotodama test request failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KotoTestRunPhaseV1 {
    /// Request fields were inconsistent or outside their V1 bounds.
    Request,
    /// Target and standalone test discovery failed.
    Discovery,
    /// The canonical Kotodama compiler rejected the discovered suite.
    Compilation,
    /// VM preparation or test execution failed before a case outcome existed.
    Execution,
}
/// Structured failure returned by the non-printing Kotodama test runner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KotoTestRunErrorV1 {
    /// Stable failure stage.
    pub phase: KotoTestRunPhaseV1,
    /// Human-readable diagnostic detail suitable for a frontend error record.
    pub message: String,
}
impl KotoTestRunErrorV1 {
    fn new(phase: KotoTestRunPhaseV1, message: impl Into<String>) -> Self {
        Self {
            phase,
            message: message.into(),
        }
    }
}
impl std::fmt::Display for KotoTestRunErrorV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}
impl std::error::Error for KotoTestRunErrorV1 {}
/// Deterministic logical outcome of one Kotodama test case.
///
/// Wall-clock timing and VM trace data are intentionally absent: neither is a
/// consensus-stable test result, and frontends can measure an entire invocation
/// separately when operational timing is useful.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KotoTestCaseOutcomeV1 {
    /// Unique test function name.
    pub name: String,
    /// One-based source line of the test declaration.
    pub line: u32,
    /// Whether VM execution completed successfully.
    pub passed: bool,
    /// Stable rendered VM diagnostic for a failed case.
    pub failure: Option<String>,
}
/// One complete, deterministically ordered Kotodama test report.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KotoTestRunReportV1 {
    /// Canonical target path selected during discovery.
    pub target: PathBuf,
    /// Ordering seed copied from the request.
    pub seed: u64,
    /// Case outcomes in deterministic execution order.
    pub cases: Vec<KotoTestCaseOutcomeV1>,
}
impl KotoTestRunReportV1 {
    /// Return the number of successful cases.
    #[must_use]
    pub fn passed(&self) -> usize {
        self.cases.iter().filter(|case| case.passed).count()
    }
    /// Return the number of failed cases.
    #[must_use]
    pub fn failed(&self) -> usize {
        self.cases.len().saturating_sub(self.passed())
    }
    /// Return whether every selected case passed.
    #[must_use]
    pub fn is_success(&self) -> bool {
        self.failed() == 0
    }
}
/// Discover, compile, and run one Kotodama V1 test suite without writing output.
///
/// Case failures are successful report values with `passed = false`. Only an
/// invalid request or a discovery/compiler/runner infrastructure failure is
/// returned as [`KotoTestRunErrorV1`]. Case order is independent of filesystem
/// enumeration and worker scheduling.
///
/// # Errors
///
/// Returns a structured stage-tagged error when request validation, discovery,
/// compilation, or VM execution setup fails.
pub fn run_tests_structured_v1(
    request: &KotoTestRunRequestV1,
) -> Result<KotoTestRunReportV1, KotoTestRunErrorV1> {
    validate_structured_request(request)?;
    let suite = discover_suite(&request.target)
        .map_err(|error| KotoTestRunErrorV1::new(KotoTestRunPhaseV1::Discovery, error))?;
    run_discovered_suite_structured(request, suite, None)
}
/// Run one explicitly declared test root against an exact locked module graph.
///
/// Unlike [`run_tests_structured_v1`], this entry point performs no sibling or
/// recursive test discovery. The supplied root, aliases, and immutable package
/// modules are the complete compilation authority.
///
/// # Errors
///
/// Returns a structured stage-tagged error when request validation, isolated
/// root discovery, exact typed linking, compilation, or VM execution fails.
pub fn run_tests_structured_with_modules_v1(
    request: &KotoTestRunRequestV1,
    modules: &KotoTestModuleGraphV1,
) -> Result<KotoTestRunReportV1, KotoTestRunErrorV1> {
    validate_structured_request(request)?;
    let suite = discover_declared_suite(&request.target)
        .map_err(|error| KotoTestRunErrorV1::new(KotoTestRunPhaseV1::Discovery, error))?;
    run_discovered_suite_structured(request, suite, Some(modules))
}
/// Run one caller-supplied test root against an exact locked module graph.
///
/// This is the structured source-set boundary for package managers and other authenticated callers.
/// The source text is never reopened from [`KotoTestRunRequestV1::target`], and no sibling or
/// recursive filesystem discovery occurs. The request target must exactly equal the source unit's
/// portable diagnostic name.
///
/// # Errors
///
/// Returns a structured stage-tagged error when the request/source binding is
/// invalid, the supplied root cannot be parsed as a direct test root, exact
/// typed linking fails, or VM execution setup fails.
pub fn run_tests_structured_source_with_modules_v1(
    request: &KotoTestRunRequestV1,
    root: &SourceModuleUnit,
    modules: &KotoTestModuleGraphV1,
) -> Result<KotoTestRunReportV1, KotoTestRunErrorV1> {
    validate_structured_request(request)?;
    validate_structured_source_request(request, root)?;
    let suite = discover_declared_suite_from_source(root)
        .map_err(|error| KotoTestRunErrorV1::new(KotoTestRunPhaseV1::Discovery, error))?;
    run_discovered_suite_structured(request, suite, Some(modules))
}
/// Discover test names from one explicitly declared root without ambient files.
///
/// # Errors
///
/// Returns an error when the path cannot be resolved or read, the source is not
/// a direct test root, parsing fails, or no `#[test]` function is declared.
pub fn discover_declared_test_names_v1(path: &Path) -> Result<Vec<String>, String> {
    let suite = discover_declared_suite(path)?;
    Ok(suite.tests.into_iter().map(|test| test.name).collect())
}
/// Discover test names from one explicitly supplied direct root.
///
/// No path is opened and no ambient source is discovered. The source name is
/// retained only as a bounded portable diagnostic identity.
///
/// # Errors
///
/// Returns an error when the source identity or text exceeds the typed-module
/// graph bounds, parsing fails, the root is indirect, or no test is declared.
pub fn discover_declared_test_names_source_v1(
    root: &SourceModuleUnit,
) -> Result<Vec<String>, String> {
    validate_structured_source(root)?;
    let suite = discover_declared_suite_from_source(root)?;
    Ok(suite.tests.into_iter().map(|test| test.name).collect())
}
fn run_discovered_suite_structured(
    request: &KotoTestRunRequestV1,
    mut suite: DiscoveredSuite,
    modules: Option<&KotoTestModuleGraphV1>,
) -> Result<KotoTestRunReportV1, KotoTestRunErrorV1> {
    filter_and_order_structured_tests(&mut suite.tests, request);
    if suite.tests.is_empty() {
        return Err(KotoTestRunErrorV1::new(
            KotoTestRunPhaseV1::Discovery,
            "no Kotodama tests matched the requested filter",
        ));
    }
    let compiled = match modules {
        Some(modules) => compile_suite_with_modules_for_chain(
            &suite,
            modules,
            request.zk_enabled,
            request.chain_discriminant,
        ),
        None => compile_suite_for_chain(&suite, request.zk_enabled, request.chain_discriminant),
    }
    .map_err(|error| KotoTestRunErrorV1::new(KotoTestRunPhaseV1::Compilation, error))?;
    let results = execute_suite_for_chain(
        &compiled,
        TraceMode::Off,
        request.jobs,
        request.chain_discriminant,
    )
    .map_err(|error| KotoTestRunErrorV1::new(KotoTestRunPhaseV1::Execution, error))?;
    let cases = results
        .into_iter()
        .map(|result| {
            let line = u32::try_from(result.line).map_err(|_| {
                KotoTestRunErrorV1::new(
                    KotoTestRunPhaseV1::Execution,
                    format!("test `{}` source line exceeds the V1 bound", result.name),
                )
            })?;
            Ok(KotoTestCaseOutcomeV1 {
                name: result.name,
                line,
                passed: result.passed,
                failure: result.failure,
            })
        })
        .collect::<Result<Vec<_>, KotoTestRunErrorV1>>()?;
    Ok(KotoTestRunReportV1 {
        target: suite.target_path,
        seed: request.seed,
        cases,
    })
}
fn validate_structured_request(request: &KotoTestRunRequestV1) -> Result<(), KotoTestRunErrorV1> {
    if request.jobs == 0 {
        return Err(KotoTestRunErrorV1::new(
            KotoTestRunPhaseV1::Request,
            "Kotodama test worker count must be greater than zero",
        ));
    }
    if request.chain_discriminant == 0 {
        return Err(KotoTestRunErrorV1::new(
            KotoTestRunPhaseV1::Request,
            "Kotodama test chain discriminant must be in 1..=65535",
        ));
    }
    if request.exact && request.filter.is_none() {
        return Err(KotoTestRunErrorV1::new(
            KotoTestRunPhaseV1::Request,
            "exact Kotodama test selection requires a filter",
        ));
    }
    Ok(())
}
fn validate_structured_source_request(
    request: &KotoTestRunRequestV1,
    root: &SourceModuleUnit,
) -> Result<(), KotoTestRunErrorV1> {
    validate_structured_source(root)
        .map_err(|error| KotoTestRunErrorV1::new(KotoTestRunPhaseV1::Request, error))?;
    if request.target.as_path() != Path::new(&root.source_name) {
        return Err(KotoTestRunErrorV1::new(
            KotoTestRunPhaseV1::Request,
            "Kotodama test request target must equal the supplied source name",
        ));
    }
    Ok(())
}
fn validate_structured_source(root: &SourceModuleUnit) -> Result<(), String> {
    if root.source_name.is_empty()
        || root.source_name.len() > MAX_LOGICAL_SOURCE_PATH_BYTES
        || root.source.is_empty()
        || root.source.len() > MAX_MODULE_GRAPH_SOURCE_BYTES
    {
        return Err(format!(
            "supplied Kotodama test root must have a nonempty bounded source name and at most {MAX_MODULE_GRAPH_SOURCE_BYTES} UTF-8 source bytes"
        ));
    }
    ModuleBuildGraph::fingerprint(&SourceLinkRequest {
        root: root.clone(),
        imports: Vec::new(),
        packages: Vec::new(),
    })
    .map_err(|error| error.to_string())?;
    if root.source_name.contains('\\')
        || root
            .source_name
            .split('/')
            .any(|component| component.is_empty() || matches!(component, "." | ".."))
    {
        return Err(
            "supplied Kotodama test root source name must use its canonical logical spelling"
                .to_owned(),
        );
    }
    Ok(())
}
fn filter_and_order_structured_tests(tests: &mut Vec<TestCase>, request: &KotoTestRunRequestV1) {
    if let Some(filter) = request.filter.as_deref() {
        tests.retain(|test| {
            if request.exact {
                test.name == filter
            } else {
                test.name.contains(filter)
            }
        });
    }
    tests.sort_by(|left, right| {
        if request.seed == 0 {
            left.name
                .cmp(&right.name)
                .then_with(|| left.line.cmp(&right.line))
        } else {
            seeded_test_key(request.seed, &left.name)
                .cmp(&seeded_test_key(request.seed, &right.name))
                .then_with(|| left.name.cmp(&right.name))
                .then_with(|| left.line.cmp(&right.line))
        }
    });
}
struct KotoTestHost {
    inner: WsvHost,
    actors: HashMap<String, FixtureActor>,
    base_public_inputs: BTreeMap<Name, Vec<u8>>,
    entrypoints: HashMap<String, RuntimeEntrypoint>,
    program: Option<crate::PreparedContract>,
    contract_address: ContractAddress,
    last_test_error: Option<String>,
    supplemental_trace_pcs: Vec<u64>,
    supplemental_delta_trace: Vec<crate::zk::DeltaEntry>,
}
/// Run the VM-backed Kotodama test harness for the unified `koto test` command.
pub fn run_cli(args: Vec<String>) -> Result<(), String> {
    let options = parse_args(args)?;
    let mut suite = discover_suite(&options.path)?;
    filter_and_order_tests(&mut suite.tests, &options);
    if options.command == Command::List {
        return print_test_list(&suite, options.output);
    }
    if suite.tests.is_empty() {
        return Err("no Kotodama tests matched the requested filter".to_owned());
    }
    let compiled = compile_suite_for_chain(&suite, options.zk_enabled, options.chain_discriminant)?;
    let trace_mode = match options.command {
        Command::Run => TraceMode::Off,
        Command::Coverage => TraceMode::PcOnly,
        Command::Profile => TraceMode::DeltaRegisters,
        Command::List => unreachable!("list exits before compilation"),
    };
    if options.output != TestOutputFormat::Text && options.command != Command::Run {
        return Err("JSON and JUnit output are currently available for `koto test run`".to_owned());
    }
    let results = execute_suite_for_chain(
        &compiled,
        trace_mode,
        options.jobs,
        options.chain_discriminant,
    )?;
    emit_test_results(
        &suite.target_path,
        &results,
        options.output,
        options.output_path.as_deref(),
        options.seed,
    )?;
    if options.command == Command::Coverage {
        print_coverage_report(&compiled, &results);
    }
    if options.command == Command::Profile {
        print_profile_report(&compiled, &results)?;
    }
    if results.iter().any(|result| !result.passed) {
        return Err("one or more Kotodama tests failed".to_string());
    }
    Ok(())
}
/// Discover the Kotodama test names contributed by one target or standalone test source.
///
/// Developer frontends use this before dispatching filtered runs so a filter
/// that matches a test in one file does not fail early on an unrelated file.
pub fn discover_test_names(path: &Path) -> Result<Vec<String>, String> {
    let suite = discover_suite(path)?;
    Ok(suite.tests.into_iter().map(|test| test.name).collect())
}
fn parse_args(args: Vec<String>) -> Result<TestOptions, String> {
    let mut command = Command::Run;
    let mut path = None;
    let mut filter = None;
    let mut exact = false;
    let mut jobs = 1_usize;
    let mut seed = 0_u64;
    let mut chain_discriminant = None;
    let mut zk_enabled = false;
    let mut output = TestOutputFormat::Text;
    let mut output_path = None;
    let mut index = 0;
    if let Some(first) = args.first() {
        command = match first.as_str() {
            "run" => Command::Run,
            "coverage" => Command::Coverage,
            "profile" => Command::Profile,
            "list" => Command::List,
            _ => Command::Run,
        };
        if matches!(first.as_str(), "run" | "coverage" | "profile" | "list") {
            index += 1;
        }
    }
    while index < args.len() {
        match args[index].as_str() {
            "--list" => command = Command::List,
            "--filter" => {
                index += 1;
                filter = Some(
                    args.get(index)
                        .ok_or_else(|| "--filter requires a value".to_owned())?
                        .clone(),
                );
            }
            "--exact" => exact = true,
            "--jobs" | "-j" => {
                index += 1;
                jobs = args
                    .get(index)
                    .ok_or_else(|| "--jobs requires a value".to_owned())?
                    .parse()
                    .map_err(|_| "--jobs must be a positive integer".to_owned())?;
                if jobs == 0 {
                    return Err("--jobs must be greater than zero".to_owned());
                }
            }
            "--seed" => {
                index += 1;
                seed = args
                    .get(index)
                    .ok_or_else(|| "--seed requires a value".to_owned())?
                    .parse()
                    .map_err(|_| "--seed must be an unsigned integer".to_owned())?;
            }
            "--chain-discriminant" => {
                index += 1;
                let raw = args
                    .get(index)
                    .ok_or_else(|| "--chain-discriminant requires a value".to_owned())?;
                let parsed = parse_chain_discriminant(raw)?;
                if chain_discriminant.replace(parsed).is_some() {
                    return Err("--chain-discriminant may be supplied only once".to_owned());
                }
            }
            "--zk" => zk_enabled = true,
            "--json" => output = TestOutputFormat::Json,
            "--junit" => {
                output = TestOutputFormat::Junit;
                if args
                    .get(index + 1)
                    .is_some_and(|next| !next.starts_with('-'))
                {
                    index += 1;
                    output_path = Some(PathBuf::from(&args[index]));
                }
            }
            "--format" => {
                index += 1;
                output = match args
                    .get(index)
                    .ok_or_else(|| "--format requires a value".to_owned())?
                    .as_str()
                {
                    "text" | "human" => TestOutputFormat::Text,
                    "json" => TestOutputFormat::Json,
                    "junit" => TestOutputFormat::Junit,
                    other => return Err(format!("unknown test output format `{other}`")),
                };
            }
            flag if flag.starts_with('-') => return Err(format!("unknown test option `{flag}`")),
            raw_path if path.is_none() => path = Some(PathBuf::from(raw_path)),
            extra => return Err(format!("unexpected test argument `{extra}`")),
        }
        index += 1;
    }
    if exact && filter.is_none() {
        return Err("--exact requires --filter".to_owned());
    }
    let path = path.ok_or_else(|| {
        "usage: koto test [run|coverage|profile|list] [--zk] [options] <program.ko|test.ko>"
            .to_owned()
    })?;
    Ok(TestOptions {
        command,
        path,
        filter,
        exact,
        jobs,
        seed,
        chain_discriminant: chain_discriminant
            .unwrap_or_else(iroha_data_model::account::address::chain_discriminant),
        zk_enabled,
        output,
        output_path,
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
fn filter_and_order_tests(tests: &mut Vec<TestCase>, options: &TestOptions) {
    if let Some(filter) = options.filter.as_deref() {
        tests.retain(|test| {
            if options.exact {
                test.name == filter
            } else {
                test.name.contains(filter)
            }
        });
    }
    if options.seed != 0 {
        tests.sort_by_key(|test| seeded_test_key(options.seed, &test.name));
    }
}
fn seeded_test_key(seed: u64, name: &str) -> u64 {
    name.bytes()
        .fold(seed ^ 0xcbf2_9ce4_8422_2325, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
        })
}
fn discover_suite(path: &Path) -> Result<DiscoveredSuite, String> {
    let input_path = fs::canonicalize(path)
        .map_err(|err| format!("failed to resolve {}: {err}", path.display()))?;
    let (input_source, input_program) = parse_program_file(&input_path)?;
    if input_program.test_target.is_some() {
        discover_suite_from_standalone_test(&input_path, input_source, input_program)
    } else {
        discover_suite_from_target(&input_path, input_source, input_program)
    }
}
fn discover_declared_suite(path: &Path) -> Result<DiscoveredSuite, String> {
    let input_path = fs::canonicalize(path)
        .map_err(|err| format!("failed to resolve {}: {err}", path.display()))?;
    let (source, program) = parse_program_file(&input_path)?;
    if program.test_target.is_some() {
        return Err(format!(
            "{} is an indirect koto_test module; exact module graphs require a directly declared test root",
            input_path.display()
        ));
    }
    finalize_suite(input_path, source, program, Vec::new())
}
fn discover_declared_suite_from_source(root: &SourceModuleUnit) -> Result<DiscoveredSuite, String> {
    validate_structured_source(root)?;
    let program =
        parser::parse(&root.source).map_err(|error| format!("{}: {error}", root.source_name))?;
    if program.test_target.is_some() {
        return Err(format!(
            "{} is an indirect koto_test module; exact module graphs require a directly declared test root",
            root.source_name
        ));
    }
    finalize_suite(
        PathBuf::from(&root.source_name),
        root.source.clone(),
        program,
        Vec::new(),
    )
}
fn discover_suite_from_target(
    path: &Path,
    target_source: String,
    target_program: Program,
) -> Result<DiscoveredSuite, String> {
    let standalone_tests = discover_standalone_tests_for_target(path)?;
    for test in &standalone_tests {
        validate_standalone_test_program(&test.path, path, &test.program)?;
    }
    finalize_suite(
        path.to_path_buf(),
        target_source,
        target_program,
        standalone_tests,
    )
}
fn discover_suite_from_standalone_test(
    test_path: &Path,
    test_source: String,
    test_program: Program,
) -> Result<DiscoveredSuite, String> {
    let target_decl = test_program.test_target.as_ref().ok_or_else(|| {
        format!(
            "{} is missing a koto_test target declaration",
            test_path.display()
        )
    })?;
    let target_path = resolve_target_path(test_path, &target_decl.target)?;
    let (target_source, target_program) = parse_program_file(&target_path)?;
    validate_standalone_test_program(test_path, &target_path, &test_program)?;
    finalize_suite(
        target_path,
        target_source,
        target_program,
        vec![DiscoveredTestModule {
            path: test_path.to_path_buf(),
            source: test_source,
            program: test_program,
        }],
    )
}
fn finalize_suite(
    target_path: PathBuf,
    target_source: String,
    target_program: Program,
    test_modules: Vec<DiscoveredTestModule>,
) -> Result<DiscoveredSuite, String> {
    let mut tests = Vec::new();
    let mut test_names = HashSet::new();
    collect_tests_into(&target_program, &mut test_names, &mut tests)?;
    for module in &test_modules {
        collect_tests_into(&module.program, &mut test_names, &mut tests)?;
    }
    if tests.is_empty() {
        return Err(format!(
            "no #[test] Kotodama functions were found for {}",
            target_path.display()
        ));
    }
    let fixtures = build_fixture_map(
        &target_program
            .fixtures
            .iter()
            .chain(
                test_modules
                    .iter()
                    .flat_map(|module| module.program.fixtures.iter()),
            )
            .cloned()
            .collect::<Vec<_>>(),
    )?;
    Ok(DiscoveredSuite {
        target_path,
        target_source,
        target_program,
        test_modules,
        tests,
        fixtures,
    })
}
fn parse_program_file(path: &Path) -> Result<(String, Program), String> {
    let src = read_source_file(path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    let program = parser::parse(&src).map_err(|err| format!("{}: {err}", path.display()))?;
    Ok((src, program))
}
fn resolve_target_path(test_file: &Path, raw_target: &str) -> Result<PathBuf, String> {
    let parent = test_file
        .parent()
        .ok_or_else(|| format!("{} has no parent directory", test_file.display()))?;
    let candidate = parent.join(raw_target);
    fs::canonicalize(&candidate).map_err(|err| {
        format!(
            "failed to resolve target `{raw_target}` from {}: {err}",
            test_file.display()
        )
    })
}
fn discover_standalone_tests_for_target(
    target_path: &Path,
) -> Result<Vec<DiscoveredTestModule>, String> {
    let base_dir = target_path
        .parent()
        .ok_or_else(|| format!("{} has no parent directory", target_path.display()))?;
    let mut paths = BTreeSet::new();
    for entry in fs::read_dir(base_dir)
        .map_err(|err| format!("failed to read {}: {err}", base_dir.display()))?
    {
        let entry = entry.map_err(|err| format!("failed to read directory entry: {err}"))?;
        let path = entry.path();
        if path == target_path {
            continue;
        }
        if path.extension().and_then(|ext| ext.to_str()) != Some("ko") {
            continue;
        }
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(".test.ko"))
        {
            paths.insert(
                fs::canonicalize(&path)
                    .map_err(|err| format!("failed to resolve {}: {err}", path.display()))?,
            );
        }
    }
    let tests_dir = base_dir.join("tests");
    if tests_dir.exists() {
        collect_ko_files(&tests_dir, &mut paths)?;
    }
    let mut discovered = Vec::new();
    for test_path in paths {
        let (source, program) = parse_program_file(&test_path)?;
        if let Some(test_target) = &program.test_target {
            let resolved = resolve_target_path(&test_path, &test_target.target)?;
            if resolved == target_path {
                discovered.push(DiscoveredTestModule {
                    path: test_path,
                    source,
                    program,
                });
            }
        }
    }
    Ok(discovered)
}
fn collect_ko_files(dir: &Path, out: &mut BTreeSet<PathBuf>) -> Result<(), String> {
    for entry in
        fs::read_dir(dir).map_err(|err| format!("failed to read {}: {err}", dir.display()))?
    {
        let entry = entry.map_err(|err| format!("failed to read directory entry: {err}"))?;
        let path = entry.path();
        if path.is_dir() {
            collect_ko_files(&path, out)?;
            continue;
        }
        if path.extension().and_then(|ext| ext.to_str()) != Some("ko") {
            continue;
        }
        out.insert(
            fs::canonicalize(&path)
                .map_err(|err| format!("failed to resolve {}: {err}", path.display()))?,
        );
    }
    Ok(())
}
fn validate_standalone_test_program(
    test_path: &Path,
    target_path: &Path,
    program: &Program,
) -> Result<(), String> {
    let target_decl = program.test_target.as_ref().ok_or_else(|| {
        format!(
            "{} must include `koto_test {{ target: \"...\" }}`",
            test_path.display()
        )
    })?;
    let resolved_target = resolve_target_path(test_path, &target_decl.target)?;
    if resolved_target != target_path {
        return Err(format!(
            "{} targets {}, expected {}",
            test_path.display(),
            resolved_target.display(),
            target_path.display()
        ));
    }
    if program.unit.kind != SourceUnitKind::Module {
        return Err(format!(
            "{} must declare a non-deployable module in standalone test mode",
            test_path.display()
        ));
    }
    for item in &program.items {
        match item {
            Item::Function(func) => {
                if func.modifiers.kind != FunctionKind::Private {
                    return Err(format!(
                        "{} contains a non-local function `{}`; standalone test files may only define private helpers and #[test] functions",
                        test_path.display(),
                        func.name
                    ));
                }
            }
            Item::State(_) | Item::Trigger(_) => {
                return Err(format!(
                    "{} may not declare durable state or triggers",
                    test_path.display()
                ));
            }
            Item::Struct(_) | Item::ErrorEnum(_) | Item::Const(_) => {}
        }
    }
    Ok(())
}
fn collect_tests_into(
    program: &Program,
    names: &mut HashSet<String>,
    tests: &mut Vec<TestCase>,
) -> Result<(), String> {
    for item in &program.items {
        let Item::Function(func) = item else {
            continue;
        };
        if !func.modifiers.is_test {
            continue;
        }
        if !names.insert(func.name.clone()) {
            return Err(format!("duplicate test function `{}`", func.name));
        }
        tests.push(TestCase {
            name: func.name.clone(),
            fixture: func.modifiers.test_fixture.clone(),
            line: func.location.line,
        });
    }
    Ok(())
}
fn build_fixture_map(fixtures: &[FixtureDecl]) -> Result<HashMap<String, FixtureDecl>, String> {
    let mut map = HashMap::new();
    for fixture in fixtures {
        if map.contains_key(&fixture.name) {
            return Err(format!("duplicate fixture `{}`", fixture.name));
        }
        map.insert(fixture.name.clone(), fixture.clone());
    }
    Ok(map)
}
#[cfg(test)]
fn compile_suite(suite: &DiscoveredSuite, zk_enabled: bool) -> Result<CompiledSuite, String> {
    compile_suite_for_chain(
        suite,
        zk_enabled,
        iroha_data_model::account::address::chain_discriminant(),
    )
}
fn compile_suite_for_chain(
    suite: &DiscoveredSuite,
    zk_enabled: bool,
    chain_discriminant: u16,
) -> Result<CompiledSuite, String> {
    let source_name = suite.target_path.display().to_string();
    let test_opts = CompilerOptions {
        force_zk: zk_enabled,
        chain_discriminant,
        mode: CompilerMode::Test,
        ..CompilerOptions::default()
    };
    let target = TestSourceUnit {
        source_name,
        source: suite.target_source.clone(),
    };
    let test_modules = suite
        .test_modules
        .iter()
        .map(|module| TestSourceUnit {
            source_name: module.path.display().to_string(),
            source: module.source.clone(),
        })
        .collect::<Vec<_>>();
    let outputs = CompilerSession::new(test_opts)
        .build_test_sources(&target, &test_modules)
        .map_err(|diagnostics| diagnostics.render_human())?;
    prepare_compiled_suite(suite, outputs)
}
fn compile_suite_with_modules_for_chain(
    suite: &DiscoveredSuite,
    modules: &KotoTestModuleGraphV1,
    zk_enabled: bool,
    chain_discriminant: u16,
) -> Result<CompiledSuite, String> {
    if !suite.test_modules.is_empty() {
        return Err(
            "exact module-graph test compilation accepts one directly declared root".to_owned(),
        );
    }
    let source_name = if suite.target_path.is_absolute() {
        let project_root = suite.target_path.parent().ok_or_else(|| {
            format!(
                "Kotodama test target `{}` has no project parent directory",
                suite.target_path.display()
            )
        })?;
        crate::kotodama::driver::logical_source_name(&suite.target_path, project_root)
            .map_err(|error| error.to_string())?
    } else {
        suite.target_path.display().to_string()
    };
    let outputs = ModuleBuildGraph::default()
        .build_test_project(
            SourceLinkRequest {
                root: SourceModuleUnit {
                    source_name: source_name.clone(),
                    source: suite.target_source.clone(),
                },
                imports: modules.imports.clone(),
                packages: modules.packages.clone(),
            },
            CompilerOptions {
                force_zk: zk_enabled,
                chain_discriminant,
                mode: CompilerMode::Test,
                ..CompilerOptions::default()
            },
            &source_name,
        )
        .map_err(|diagnostics| diagnostics.render_human())?;
    prepare_compiled_suite(suite, outputs)
}
fn prepare_compiled_suite(
    suite: &DiscoveredSuite,
    outputs: TestCompileOutput,
) -> Result<CompiledSuite, String> {
    let test_output = outputs.suite;
    let test_contract_interface = test_output.contract_interface().clone();
    let test_report = test_output.report;
    let suite_program = crate::contract_artifact::prepare_koto_test_contract(
        Arc::from(test_output.artifact),
        test_contract_interface,
    )
    .map_err(|err| format!("failed to prepare compiled Kotodama test suite: {err}"))?;
    if suite_program.code_hash() != test_report.artifact_hash {
        return Err(format!(
            "compiled suite artifact hash mismatch: expected {}, got {}",
            test_report.artifact_hash,
            suite_program.code_hash()
        ));
    }
    let test_pc_base = suite_program.instruction_entry_pc();
    let suite_artifact = CompiledArtifact {
        program: suite_program,
        report: test_report,
        pc_base: test_pc_base,
    };
    let (runtime, runtime_entrypoints) = if let Some(runtime_output) = outputs.runtime {
        let runtime_report = runtime_output.report;
        let runtime_program = crate::prepare_contract(Arc::from(runtime_output.artifact))
            .map_err(|err| format!("failed to prepare compiled runtime contract: {err}"))?;
        if runtime_program.code_hash() != runtime_report.artifact_hash {
            return Err(format!(
                "compiled runtime artifact hash mismatch: expected {}, got {}",
                runtime_report.artifact_hash,
                runtime_program.code_hash()
            ));
        }
        let runtime_pc_base = runtime_program.instruction_entry_pc();
        let runtime_entrypoints = runtime_program
            .contract_interface()
            .entrypoints
            .iter()
            .map(|entry| {
                let pc = runtime_program
                    .entrypoint_pc(&entry.name)
                    .expect("prepared runtime indexes every validated entrypoint");
                (
                    entry.name.clone(),
                    RuntimeEntrypoint {
                        pc,
                        argument_schema: entry.argument_schema.clone(),
                        permission: entry.permission.clone(),
                    },
                )
            })
            .collect::<HashMap<_, _>>();
        (
            Some(CompiledArtifact {
                program: runtime_program,
                report: runtime_report,
                pc_base: runtime_pc_base,
            }),
            runtime_entrypoints,
        )
    } else {
        (None, HashMap::new())
    };
    let mut test_pcs = HashMap::new();
    for entry in &suite_artifact.report.budget_report {
        test_pcs
            .entry(entry.function_name.clone())
            .or_insert(test_pc_base.saturating_add(entry.pc_start));
    }
    let tests = suite
        .tests
        .iter()
        .map(|test| {
            let pc = test_pcs
                .get(&test.name)
                .copied()
                .ok_or_else(|| format!("missing debug info for test `{}`", test.name))?;
            Ok(CompiledTestCase {
                name: test.name.clone(),
                fixture: test.fixture.clone(),
                line: test.line,
                pc,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let profile_artifact = runtime.as_ref().unwrap_or(&suite_artifact);
    let coverage_functions = build_coverage_functions(
        &suite.target_program,
        &profile_artifact.report,
        profile_artifact.pc_base,
    );
    Ok(CompiledSuite {
        suite: suite_artifact,
        runtime,
        runtime_entrypoints,
        tests,
        fixtures: suite.fixtures.clone(),
        coverage_functions,
    })
}
fn build_coverage_functions(
    program: &Program,
    report: &CompileReport,
    pc_base: u64,
) -> Vec<CoverageFunction> {
    let test_names = program
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Function(func) if func.modifiers.is_test => Some(func.name.clone()),
            _ => None,
        })
        .collect::<HashSet<_>>();
    let implementation_bases = report
        .budget_report
        .iter()
        .filter_map(|entry| entry.function_name.strip_prefix(ENTRYPOINT_IMPL_PREFIX))
        .map(ToOwned::to_owned)
        .collect::<HashSet<_>>();
    let mut functions = report
        .budget_report
        .iter()
        .filter_map(|entry| {
            let display_name = normalize_user_function_name(&entry.function_name)?;
            if test_names.contains(display_name) {
                return None;
            }
            if implementation_bases.contains(&entry.function_name) {
                return None;
            }
            Some(CoverageFunction {
                display_name: display_name.to_string(),
                line: entry.source.as_ref().map_or(0, |source| source.line),
                pc_start: pc_base.saturating_add(entry.pc_start),
                pc_end: pc_base.saturating_add(entry.pc_end),
            })
        })
        .collect::<Vec<_>>();
    functions.sort_by_key(|function| (function.line, function.display_name.clone()));
    functions
}
fn normalize_user_function_name(name: &str) -> Option<&str> {
    if let Some(base) = name.strip_prefix(ENTRYPOINT_IMPL_PREFIX) {
        return Some(base);
    }
    if name.starts_with("__") {
        return None;
    }
    Some(name)
}
#[cfg(test)]
fn execute_suite(
    compiled: &CompiledSuite,
    trace_mode: TraceMode,
    jobs: usize,
) -> Result<Vec<TestRunResult>, String> {
    execute_suite_for_chain(
        compiled,
        trace_mode,
        jobs,
        iroha_data_model::account::address::chain_discriminant(),
    )
}
fn execute_suite_for_chain(
    compiled: &CompiledSuite,
    trace_mode: TraceMode,
    jobs: usize,
    chain_discriminant: u16,
) -> Result<Vec<TestRunResult>, String> {
    let suite_return_pc = compiled
        .suite
        .program
        .entrypoint_pc(crate::metadata::KOTO_TEST_RETURN_ENTRYPOINT)
        .ok_or_else(|| "compiled suite is missing its validated return entrypoint".to_owned())?;
    let worker_count = jobs.min(compiled.tests.len().max(1));
    if worker_count == 1 {
        let _chain_discriminant = ChainDiscriminantGuard::enter(chain_discriminant);
        return compiled
            .tests
            .iter()
            .map(|test| execute_test(compiled, test, trace_mode, suite_return_pc))
            .collect();
    }
    let joined = std::thread::scope(|scope| {
        let mut workers = Vec::with_capacity(worker_count);
        for worker in 0..worker_count {
            workers.push(scope.spawn(move || {
                let _chain_discriminant = ChainDiscriminantGuard::enter(chain_discriminant);
                compiled
                    .tests
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| index % worker_count == worker)
                    .map(|(index, test)| {
                        execute_test(compiled, test, trace_mode, suite_return_pc)
                            .map(|result| (index, result))
                    })
                    .collect::<Result<Vec<_>, String>>()
            }));
        }
        workers
            .into_iter()
            .map(|worker| {
                worker
                    .join()
                    .map_err(|_| "Kotodama test worker panicked".to_owned())?
            })
            .collect::<Result<Vec<_>, String>>()
    })?;
    let mut indexed = joined.into_iter().flatten().collect::<Vec<_>>();
    indexed.sort_by_key(|(index, _)| *index);
    Ok(indexed.into_iter().map(|(_, result)| result).collect())
}
fn execute_test(
    compiled: &CompiledSuite,
    test: &CompiledTestCase,
    trace_mode: TraceMode,
    suite_return_pc: u64,
) -> Result<TestRunResult, String> {
    let mut host = build_host_for_fixture(compiled, test.fixture.as_deref())?;
    let mut vm = IVM::new(u64::MAX);
    vm.load_koto_test_prepared(&compiled.suite.program)
        .map_err(|err| format!("failed to load compiled suite: {err:?}"))?;
    vm.set_register(1, suite_return_pc);
    vm.set_program_counter(test.pc)
        .map_err(|err| format!("failed to jump to test `{}`: {err:?}", test.name))?;
    vm.set_trace_mode(trace_mode);
    let started = Instant::now();
    let outcome = vm.run_with_host(&mut host);
    let elapsed = started.elapsed();
    let passed = outcome.is_ok();
    let failure = outcome
        .err()
        .map(|err| render_failure(&vm, host.last_test_error(), &err));
    let mut trace_pcs = vm.trace_pcs().to_vec();
    trace_pcs.extend_from_slice(host.supplemental_trace_pcs());
    let mut delta_trace = vm.delta_register_trace().to_vec();
    delta_trace.extend_from_slice(host.supplemental_delta_trace());
    Ok(TestRunResult {
        name: test.name.clone(),
        line: test.line,
        elapsed,
        passed,
        failure,
        trace_pcs,
        delta_trace,
    })
}
fn build_host_for_fixture(
    compiled: &CompiledSuite,
    fixture_name: Option<&str>,
) -> Result<KotoTestHost, String> {
    let caller = default_caller_account()?;
    let base_host =
        WsvHost::new_with_subject(MockWorldStateView::default(), caller, HashMap::new());
    let mut host = KotoTestHost::new(
        base_host,
        compiled
            .runtime
            .as_ref()
            .map(|artifact| artifact.program.clone()),
        compiled.runtime_entrypoints.clone(),
    );
    let mut public_inputs = BTreeMap::new();
    if let Some(name) = fixture_name {
        let fixture = compiled
            .fixtures
            .get(name)
            .ok_or_else(|| format!("unknown fixture `{name}`"))?;
        for action in &fixture.actions {
            apply_fixture_action(action, &mut host, &mut public_inputs)?;
        }
    }
    host.base_public_inputs = public_inputs.clone();
    host.inner_mut().set_public_inputs(public_inputs);
    Ok(host)
}
fn apply_fixture_action(
    action: &FixtureAction,
    host: &mut KotoTestHost,
    public_inputs: &mut BTreeMap<Name, Vec<u8>>,
) -> Result<(), String> {
    match action.name.as_str() {
        "actor" => {
            if !(2..=3).contains(&action.args.len()) {
                return Err("fixture action `actor` expects 2 or 3 arguments".to_string());
            }
            let alias = eval_actor_alias_expr(&action.args[0])?;
            let account = eval_account_expr(&action.args[1])?;
            let seed = if action.args.len() == 3 {
                Some(eval_seed_expr(&action.args[2])?)
            } else {
                None
            };
            host.register_actor(alias.clone(), account)?;
            if let Some(seed) = seed {
                host.set_actor_seed(&alias, seed)?;
            }
            Ok(())
        }
        "caller" => {
            expect_arg_count(action, 1)?;
            host.set_caller_subject(eval_account_expr(&action.args[0])?);
            Ok(())
        }
        "register_account" => {
            expect_arg_count(action, 1)?;
            let account = eval_account_expr(&action.args[0])?;
            host.inner_mut().wsv.add_account_unchecked(account);
            Ok(())
        }
        "grant_permission" => {
            if action.args.len() == 1 {
                let permission = eval_permission_expr(&action.args[0])?;
                let caller = host.caller_subject();
                host.inner_mut().wsv.grant_permission(&caller, permission);
                return Ok(());
            }
            if action.args.len() == 2 {
                let alias = eval_actor_alias_expr(&action.args[0])?;
                let permission = eval_permission_expr(&action.args[1])?;
                let account = host
                    .actor_account(&alias)
                    .ok_or_else(|| format!("unknown actor `{alias}`"))?;
                host.inner_mut().wsv.grant_permission(&account, permission);
                return Ok(());
            }
            Err("fixture action `grant_permission` expects 1 or 2 arguments".to_string())
        }
        "grant_seiyaku_kotoage_permission" => {
            expect_arg_count(action, 2)?;
            let alias = eval_actor_alias_expr(&action.args[0])?;
            let entrypoint = eval_string_expr(&action.args[1])?;
            if entrypoint.is_empty() || entrypoint.trim() != entrypoint {
                return Err(
                    "fixture seiyaku kotoage permission requires a non-empty canonical selector"
                        .to_owned(),
                );
            }
            let account = host
                .actor_account(&alias)
                .ok_or_else(|| format!("unknown actor `{alias}`"))?;
            let permission = PermissionToken::ContractEntrypoint {
                contract: host.contract_address.clone(),
                entrypoint,
            };
            host.inner_mut().wsv.grant_permission(&account, permission);
            Ok(())
        }
        "grant_seiyaku_effect_permission" => {
            expect_arg_count(action, 1)?;
            let permission = eval_permission_expr(&action.args[0])?;
            let contract_subject = host.contract_subject();
            host.inner_mut()
                .wsv
                .grant_permission(&contract_subject, permission);
            Ok(())
        }
        "grant_seiyaku_transfer_effect_permission" => {
            expect_arg_count(action, 3)?;
            let source = eval_fixture_account_or_actor(&action.args[0], host)?;
            let asset_definition = eval_asset_definition_expr(&action.args[1])?;
            let dataspace = DataSpaceId::new(eval_u64_expr(&action.args[2])?);
            let permission = PermissionToken::TransferAssetBucket(AssetId::with_scope(
                asset_definition,
                source,
                AssetBalanceScope::Dataspace(dataspace),
            ));
            let contract_subject = host.contract_subject();
            host.inner_mut()
                .wsv
                .grant_permission(&contract_subject, permission);
            Ok(())
        }
        "register_account_alias" => {
            if !(2..=3).contains(&action.args.len()) {
                return Err(
                    "fixture action `register_account_alias` expects 2 or 3 arguments".to_owned(),
                );
            }
            let alias = eval_string_expr(&action.args[0])?;
            let account = eval_fixture_account_or_actor(&action.args[1], host)?;
            let dataspace = action
                .args
                .get(2)
                .map(eval_u64_expr)
                .transpose()?
                .map(DataSpaceId::new);
            host.inner_mut()
                .register_account_alias_with_dataspace(alias, account, dataspace)
        }
        "register_domain" => {
            expect_arg_count(action, 1)?;
            let domain = eval_domain_expr(&action.args[0])?;
            let caller = host.caller_subject();
            let inner = host.inner_mut();
            inner
                .wsv
                .grant_permission(&caller, PermissionToken::RegisterDomain);
            if inner.wsv.register_domain(&caller, domain.clone()) {
                return Ok(());
            }
            Err(format!("failed to register domain `{domain}`"))
        }
        "register_asset_definition" => {
            if !(1..=2).contains(&action.args.len()) {
                return Err(format!(
                    "fixture action `{}` expects 1 or 2 arguments",
                    action.name
                ));
            }
            let asset = eval_asset_definition_expr(&action.args[0])?;
            let mintable = if action.args.len() == 2 {
                eval_mintable_expr(&action.args[1])?
            } else {
                Mintable::Infinitely
            };
            let caller = host.caller_subject();
            let inner = host.inner_mut();
            inner
                .wsv
                .grant_permission(&caller, PermissionToken::RegisterAssetDefinition);
            let _ = inner
                .wsv
                .register_asset_definition(&caller, asset.clone(), mintable);
            Ok(())
        }
        "set_balance" => {
            expect_arg_count(action, 3)?;
            let account = eval_account_expr(&action.args[0])?;
            let asset = eval_asset_definition_expr(&action.args[1])?;
            let amount = eval_quantity_expr(&action.args[2])?;
            let caller = host.caller_subject();
            let inner = host.inner_mut();
            inner.wsv.add_account_unchecked(account.clone());
            inner
                .wsv
                .grant_permission(&caller, PermissionToken::RegisterAssetDefinition);
            let _ =
                inner
                    .wsv
                    .register_asset_definition(&caller, asset.clone(), Mintable::Infinitely);
            inner
                .wsv
                .grant_permission(&caller, PermissionToken::MintAsset(asset.clone()));
            if inner
                .wsv
                .mint(&caller, account.clone(), asset.clone(), amount.clone())
            {
                return Ok(());
            }
            Err(format!(
                "failed to set balance `{amount}` for `{account}` on `{asset}`"
            ))
        }
        "set_account_detail" => {
            expect_arg_count(action, 3)?;
            let account = eval_account_expr(&action.args[0])?;
            let key = eval_string_expr(&action.args[1])?;
            let value = eval_detail_bytes(&action.args[2])?;
            let caller = host.caller_subject();
            let inner = host.inner_mut();
            inner.wsv.add_account_unchecked(account.clone());
            if caller != account {
                inner
                    .wsv
                    .grant_permission(&caller, PermissionToken::SetAccountDetail(account.clone()));
            }
            if inner.wsv.set_account_detail(&caller, &account, &key, value) {
                return Ok(());
            }
            Err(format!(
                "failed to set account detail `{key}` for `{account}`"
            ))
        }
        "state_set" => {
            expect_arg_count(action, 2)?;
            let path = eval_string_expr(&action.args[0])?;
            let value = eval_state_payload_expr(&action.args[1])?;
            host.inner_mut()
                .wsv
                .sc_set(&path, value)
                .map_err(|err| format!("failed to seed state `{path}`: {err:?}"))
        }
        "public_input" => {
            expect_arg_count(action, 2)?;
            let name = eval_name_expr(&action.args[0])?;
            let value = eval_envelope_expr(&action.args[1])?;
            public_inputs.insert(name, value);
            Ok(())
        }
        other => Err(format!("unknown fixture action `{other}`")),
    }
}
struct KotoTestHostSnapshot {
    inner: Box<dyn Any + Send>,
    actors: HashMap<String, FixtureActor>,
    last_test_error: Option<String>,
    supplemental_trace_pcs: Vec<u64>,
    supplemental_delta_trace: Vec<crate::zk::DeltaEntry>,
}
impl KotoTestHost {
    fn new(
        inner: WsvHost,
        program: Option<crate::PreparedContract>,
        entrypoints: HashMap<String, RuntimeEntrypoint>,
    ) -> Self {
        let contract_address = ContractAddress::derive(
            &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
                .parse()
                .expect("canonical test network id"),
            &inner.caller_subject(),
            0,
            DataSpaceId::UNIVERSAL,
        )
        .expect("Kotodama test contract address derivation must be deterministic");
        let mut inner = inner;
        inner
            .wsv
            .add_account_unchecked(contract_address.subject_id());
        Self {
            inner,
            actors: HashMap::new(),
            base_public_inputs: BTreeMap::new(),
            entrypoints,
            program,
            contract_address,
            last_test_error: None,
            supplemental_trace_pcs: Vec::new(),
            supplemental_delta_trace: Vec::new(),
        }
    }
    fn inner_mut(&mut self) -> &mut WsvHost {
        &mut self.inner
    }
    fn caller_subject(&self) -> AccountId {
        self.inner.caller_subject()
    }
    fn set_caller_subject(&mut self, caller: AccountId) {
        self.inner.set_caller_subject(caller);
    }
    fn contract_subject(&self) -> AccountId {
        self.contract_address.subject_id()
    }
    fn actor_account(&self, alias: &str) -> Option<AccountId> {
        self.actors.get(alias).map(|actor| actor.account.clone())
    }
    fn register_actor(&mut self, alias: String, account: AccountId) -> Result<(), String> {
        if self.actors.contains_key(&alias) {
            return Err(format!("duplicate actor `{alias}`"));
        }
        self.inner.wsv.add_account_unchecked(account.clone());
        self.actors.insert(
            alias,
            FixtureActor {
                account,
                seed: None,
            },
        );
        Ok(())
    }
    fn set_actor_seed(&mut self, alias: &str, seed: [u8; 32]) -> Result<(), String> {
        let signing_key = SigningKey::from_bytes(&seed);
        let public_key = iroha_crypto::PublicKey::from_bytes(
            iroha_crypto::Algorithm::Ed25519,
            signing_key.verifying_key().as_bytes(),
        )
        .map_err(|err| format!("failed to derive Ed25519 public key for actor `{alias}`: {err}"))?;
        let derived_account = AccountId::new(public_key);
        let actor = self
            .actors
            .get_mut(alias)
            .ok_or_else(|| format!("unknown actor `{alias}`"))?;
        if actor.account != derived_account {
            return Err(format!(
                "actor `{alias}` seed derives `{derived_account}` but fixture bound `{}`",
                actor.account
            ));
        }
        actor.seed = Some(seed);
        Ok(())
    }
    fn last_test_error(&self) -> Option<&str> {
        self.last_test_error.as_deref()
    }
    fn supplemental_trace_pcs(&self) -> &[u64] {
        &self.supplemental_trace_pcs
    }
    fn supplemental_delta_trace(&self) -> &[crate::zk::DeltaEntry] {
        &self.supplemental_delta_trace
    }
    fn clear_test_error(&mut self) {
        self.last_test_error = None;
    }
    fn restore_public_inputs(&mut self) {
        self.inner
            .set_public_inputs(self.base_public_inputs.clone());
    }
    fn fail_test<T>(&mut self, message: impl Into<String>) -> Result<T, crate::VMError> {
        self.last_test_error = Some(message.into());
        Err(crate::VMError::AssertionFailed)
    }
    fn decode_alias_arg(vm: &IVM, reg: usize, label: &str) -> Result<String, crate::VMError> {
        let ptr = vm.register(reg);
        if ptr == 0 {
            return Err(crate::VMError::NoritoInvalid);
        }
        let tlv = vm.validate_tlv(ptr)?;
        match tlv.type_id {
            PointerType::Blob | PointerType::NoritoBytes => {
                String::from_utf8(tlv.payload.to_vec()).map_err(|_| crate::VMError::DecodeError)
            }
            PointerType::Name => {
                let name: Name = norito::decode_canonical(tlv.payload)
                    .map_err(|_| crate::VMError::DecodeError)?;
                Ok(name.as_ref().to_string())
            }
            _ => {
                let _ = label;
                Err(crate::VMError::NoritoInvalid)
            }
        }
    }
    fn decode_json_arg(vm: &IVM, reg: usize) -> Result<Json, crate::VMError> {
        let ptr = vm.register(reg);
        if ptr == 0 {
            return Err(crate::VMError::NoritoInvalid);
        }
        let tlv = vm.validate_tlv(ptr)?;
        match tlv.type_id {
            PointerType::Json | PointerType::NoritoBytes | PointerType::Blob => {
                norito::decode_canonical(tlv.payload).map_err(|_| crate::VMError::DecodeError)
            }
            _ => Err(crate::VMError::NoritoInvalid),
        }
    }
    fn decode_bytes_arg(vm: &IVM, reg: usize) -> Result<Vec<u8>, crate::VMError> {
        let ptr = vm.register(reg);
        if ptr == 0 {
            return Err(crate::VMError::NoritoInvalid);
        }
        let tlv = vm.validate_tlv(ptr)?;
        match tlv.type_id {
            PointerType::Blob | PointerType::NoritoBytes => Ok(tlv.payload.to_vec()),
            _ => Err(crate::VMError::NoritoInvalid),
        }
    }
    fn alloc_pointer_result(
        vm: &mut IVM,
        pointer_type: PointerType,
        payload: &[u8],
    ) -> Result<u64, crate::VMError> {
        let tlv = make_tlv(pointer_type, payload);
        vm.alloc_host_tlv(&tlv)
    }
    fn record_nested_trace(&mut self, nested_vm: &IVM) {
        self.supplemental_trace_pcs
            .extend_from_slice(nested_vm.trace_pcs());
        self.supplemental_delta_trace
            .extend_from_slice(nested_vm.delta_register_trace());
    }
    fn nested_failure_message(
        actor_alias: &str,
        entrypoint: &str,
        payload: &Json,
        nested_vm: &IVM,
        err: &crate::VMError,
    ) -> String {
        format!(
            "actor `{actor_alias}` calling kotoage `{entrypoint}` with payload {:?} failed: {}",
            payload,
            render_failure(nested_vm, None, err)
        )
    }
    fn invoke_entrypoint(
        &mut self,
        vm: &mut IVM,
        expect_reject: bool,
    ) -> Result<u64, crate::VMError> {
        self.clear_test_error();
        let actor_alias =
            Self::decode_alias_arg(vm, 10, "actor").map_err(|_| crate::VMError::NoritoInvalid)?;
        let entrypoint =
            Self::decode_alias_arg(vm, 11, "kotoage").map_err(|_| crate::VMError::NoritoInvalid)?;
        let payload = Self::decode_json_arg(vm, 12)?;
        let return_pointer_mask = if expect_reject { 0 } else { vm.register(13) };
        let return_arity = if expect_reject {
            1
        } else {
            match vm.register(14) {
                0 => 1,
                raw => usize::try_from(raw).unwrap_or(TEST_MAX_RETURN_VALUES + 1),
            }
        };
        if return_arity == 0 || return_arity > TEST_MAX_RETURN_VALUES {
            return self.fail_test(format!(
                "actor `{actor_alias}` calling kotoage `{entrypoint}` requested unsupported return arity {return_arity}"
            ));
        }
        let actor = match self.actors.get(&actor_alias).cloned() {
            Some(actor) => actor,
            None => {
                return self.fail_test(format!(
                    "unknown actor `{actor_alias}` while calling kotoage `{entrypoint}`"
                ));
            }
        };
        let runtime_entrypoint = match self.entrypoints.get(&entrypoint).cloned() {
            Some(entrypoint) => entrypoint,
            None => {
                return self.fail_test(format!(
                    "unknown runtime public or lifecycle target `{entrypoint}` for actor `{actor_alias}`"
                ));
            }
        };
        if runtime_entrypoint.permission.as_deref() == Some("CanInvokeContractEntrypoint") {
            let permission = PermissionToken::ContractEntrypoint {
                contract: self.contract_address.clone(),
                entrypoint: entrypoint.clone(),
            };
            if !self.inner.wsv.has_permission(&actor.account, &permission) {
                if expect_reject {
                    vm.set_register(10, 0);
                    return Ok(0);
                }
                return self.fail_test(format!(
                    "actor `{actor_alias}` lacks exact CanInvokeContractEntrypoint permission for kotoage `{entrypoint}`"
                ));
            }
        }
        let Some(program) = self.program.as_ref() else {
            return self.fail_test(format!(
                "runtime public or lifecycle target `{entrypoint}` has no compiled runtime artifact"
            ));
        };
        let mut nested_inputs = self.base_public_inputs.clone();
        if let Some(schema) = runtime_entrypoint.argument_schema.as_ref() {
            let trigger_name: Name = "trigger_event_json"
                .parse()
                .map_err(|_| crate::VMError::DecodeError)?;
            let encoded_payload = match crate::encode_argument_record_from_json(schema, &payload) {
                Ok(encoded_payload) => encoded_payload,
                Err(crate::VMError::DecodeError | crate::VMError::NoritoInvalid)
                    if expect_reject =>
                {
                    vm.set_register(10, 0);
                    return Ok(0);
                }
                Err(err) => {
                    return self.fail_test(format!(
                        "actor `{actor_alias}` calling kotoage `{entrypoint}` supplied arguments that do not match the kotoage schema: {err:?}"
                    ));
                }
            };
            nested_inputs.insert(
                trigger_name,
                make_tlv(PointerType::NoritoBytes, &encoded_payload),
            );
        }
        let mut nested_vm = IVM::new(u64::MAX);
        nested_vm.reset();
        let clear = [0u8; 7 + iroha_crypto::Hash::LENGTH];
        nested_vm
            .memory
            .preload_input(0, &clear)
            .map_err(|_| crate::VMError::DecodeError)?;
        nested_vm
            .load_prepared(program)
            .map_err(|_| crate::VMError::DecodeError)?;
        nested_vm.set_program_counter(runtime_entrypoint.pc)?;
        nested_vm.set_trace_mode(vm.trace_mode());
        nested_vm.set_max_cycles(0);
        let rollback = self
            .inner
            .checkpoint()
            .ok_or(crate::VMError::HostUnavailable)?;
        let previous_caller = self.inner.caller_subject();
        if let Err(message) = self.inner.bind_contract_runtime_context(
            actor.account.clone(),
            self.contract_address.clone(),
            entrypoint.clone(),
        ) {
            return self.fail_test(message);
        }
        self.inner.set_public_inputs(nested_inputs);
        let nested_outcome = nested_vm.run_with_host(&mut self.inner);
        self.record_nested_trace(&nested_vm);
        match nested_outcome {
            Ok(()) if expect_reject => {
                let _ = self.inner.restore(rollback.as_ref());
                self.fail_test(format!(
                    "expected actor `{actor_alias}` calling kotoage `{entrypoint}` with payload {:?} to reject, but it succeeded",
                    payload
                ))
            }
            Ok(()) => {
                self.inner.clear_contract_runtime_context(previous_caller);
                self.restore_public_inputs();
                for idx in 0..return_arity {
                    let value = nested_vm.register(10 + idx);
                    let out_reg = 10 + idx;
                    if ((return_pointer_mask >> idx) & 1) != 0 && value != 0 {
                        let tlv = nested_vm.clone_tlv(value)?;
                        let ptr = vm.alloc_host_tlv(&tlv)?;
                        vm.set_register(out_reg, ptr);
                    } else {
                        vm.set_register(out_reg, value);
                    }
                }
                Ok(0)
            }
            Err(_err) if expect_reject => {
                let _ = self.inner.restore(rollback.as_ref());
                vm.set_register(10, 0);
                Ok(0)
            }
            Err(err) => {
                let _ = self.inner.restore(rollback.as_ref());
                self.fail_test(Self::nested_failure_message(
                    &actor_alias,
                    &entrypoint,
                    &payload,
                    &nested_vm,
                    &err,
                ))
            }
        }
    }
}
impl IVMHost for KotoTestHost {
    fn prepare_syscall(&self, number: u32, vm: &IVM) -> Result<u64, crate::VMError> {
        if crate::syscalls::is_koto_test_syscall(number) {
            Ok(0)
        } else {
            self.inner.prepare_syscall(number, vm)
        }
    }
    fn syscall(&mut self, number: u32, vm: &mut IVM) -> Result<u64, crate::VMError> {
        match number {
            TEST_SYSCALL_ACTOR_ACCOUNT => {
                self.clear_test_error();
                let alias = Self::decode_alias_arg(vm, 10, "actor")?;
                let Some(actor) = self.actors.get(&alias) else {
                    return self.fail_test(format!("unknown actor `{alias}`"));
                };
                let payload = norito::encode_canonical(&actor.account)
                    .map_err(|_| crate::VMError::NoritoInvalid)?;
                let ptr = Self::alloc_pointer_result(vm, PointerType::AccountId, &payload)?;
                vm.set_register(10, ptr);
                Ok(0)
            }
            TEST_SYSCALL_ACTOR_PUBLIC_KEY => {
                self.clear_test_error();
                let alias = Self::decode_alias_arg(vm, 10, "actor")?;
                let Some(actor) = self.actors.get(&alias) else {
                    return self.fail_test(format!("unknown actor `{alias}`"));
                };
                let Some(seed) = actor.seed else {
                    return self.fail_test(format!(
                        "actor `{alias}` does not have a deterministic signing seed"
                    ));
                };
                let signing_key = SigningKey::from_bytes(&seed);
                let ptr = Self::alloc_pointer_result(
                    vm,
                    PointerType::Blob,
                    signing_key.verifying_key().as_bytes(),
                )?;
                vm.set_register(10, ptr);
                Ok(0)
            }
            TEST_SYSCALL_ACTOR_SIGN => {
                self.clear_test_error();
                let alias = Self::decode_alias_arg(vm, 10, "actor")?;
                let Some(actor) = self.actors.get(&alias) else {
                    return self.fail_test(format!("unknown actor `{alias}`"));
                };
                let Some(seed) = actor.seed else {
                    return self.fail_test(format!(
                        "actor `{alias}` does not have a deterministic signing seed"
                    ));
                };
                let message = Self::decode_bytes_arg(vm, 11)?;
                let signing_key = SigningKey::from_bytes(&seed);
                let signature = signing_key.sign(&message);
                let ptr = Self::alloc_pointer_result(vm, PointerType::Blob, &signature.to_bytes())?;
                vm.set_register(10, ptr);
                Ok(0)
            }
            TEST_SYSCALL_INVOKE_ENTRYPOINT_AS => self.invoke_entrypoint(vm, false),
            TEST_SYSCALL_EXPECT_REJECT_AS => self.invoke_entrypoint(vm, true),
            _ => self.inner.syscall(number, vm),
        }
    }
    fn allows_syscall(&self, policy: crate::SyscallPolicy, number: u32) -> bool {
        crate::syscalls::is_koto_test_syscall(number)
            || crate::syscalls::is_syscall_allowed(policy, number)
    }
    fn as_any(&mut self) -> &mut dyn Any
    where
        Self: 'static,
    {
        self
    }
    fn supports_concurrent_blocks(&self) -> bool {
        self.inner.supports_concurrent_blocks()
    }
    fn begin_tx(
        &mut self,
        declared: &crate::parallel::StateAccessSet,
    ) -> Result<(), crate::VMError> {
        self.inner.begin_tx(declared)
    }
    fn finish_tx(&mut self) -> Result<crate::host::AccessLog, crate::VMError> {
        self.inner.finish_tx()
    }
    fn checkpoint(&self) -> Option<Box<dyn Any + Send>> {
        let inner = self.inner.checkpoint()?;
        Some(Box::new(KotoTestHostSnapshot {
            inner,
            actors: self.actors.clone(),
            last_test_error: self.last_test_error.clone(),
            supplemental_trace_pcs: self.supplemental_trace_pcs.clone(),
            supplemental_delta_trace: self.supplemental_delta_trace.clone(),
        }))
    }
    fn restore(&mut self, snapshot: &dyn Any) -> bool {
        let Some(snapshot) = snapshot.downcast_ref::<KotoTestHostSnapshot>() else {
            return false;
        };
        if !self.inner.restore(snapshot.inner.as_ref()) {
            return false;
        }
        self.actors = snapshot.actors.clone();
        self.last_test_error = snapshot.last_test_error.clone();
        self.supplemental_trace_pcs = snapshot.supplemental_trace_pcs.clone();
        self.supplemental_delta_trace = snapshot.supplemental_delta_trace.clone();
        true
    }
    fn access_logging_supported(&self) -> bool {
        self.inner.access_logging_supported()
    }
}
fn expect_arg_count(action: &FixtureAction, expected: usize) -> Result<(), String> {
    if action.args.len() == expected {
        return Ok(());
    }
    Err(format!(
        "fixture action `{}` expects {} arguments, got {}",
        action.name,
        expected,
        action.args.len()
    ))
}
fn eval_actor_alias_expr(expr: &Expr) -> Result<String, String> {
    match expr {
        Expr::String(raw) | Expr::Ident(raw) => Ok(raw.clone()),
        other => Err(format!("expected actor alias expression, got {other:?}")),
    }
}
fn eval_fixture_account_or_actor(expr: &Expr, host: &KotoTestHost) -> Result<AccountId, String> {
    if matches!(expr, Expr::String(raw) | Expr::Ident(raw) if raw == "seiyaku_subject") {
        return Ok(host.contract_subject());
    }
    if let Expr::String(raw) | Expr::Ident(raw) = expr
        && let Some(account) = host.actor_account(raw)
    {
        return Ok(account);
    }
    eval_account_expr(expr)
}
fn decode_hex_or_raw_bytes(raw: &str) -> Result<Vec<u8>, String> {
    if let Some(hex) = raw.strip_prefix("0x") {
        if hex.len() % 2 != 0 {
            return Err(format!(
                "invalid hex literal `{raw}`: expected even-length hex digits"
            ));
        }
        let mut out = Vec::with_capacity(hex.len() / 2);
        for chunk in hex.as_bytes().chunks(2) {
            let byte_str = std::str::from_utf8(chunk)
                .map_err(|err| format!("invalid hex literal `{raw}`: {err}"))?;
            let byte = u8::from_str_radix(byte_str, 16)
                .map_err(|err| format!("invalid hex literal `{raw}`: {err}"))?;
            out.push(byte);
        }
        return Ok(out);
    }
    Ok(raw.as_bytes().to_vec())
}
fn eval_seed_expr(expr: &Expr) -> Result<[u8; 32], String> {
    let bytes = match expr {
        Expr::Bytes(bytes) => bytes.clone(),
        Expr::String(raw) | Expr::Ident(raw) => decode_hex_or_raw_bytes(raw)?,
        other => return Err(format!("expected 32-byte seed blob, got {other:?}")),
    };
    <[u8; 32]>::try_from(bytes.as_slice())
        .map_err(|_| format!("actor seed must be exactly 32 bytes, got {}", bytes.len()))
}
fn eval_account_expr(expr: &Expr) -> Result<AccountId, String> {
    match expr {
        Expr::String(raw) | Expr::Ident(raw) => parse_account_literal(raw),
        Expr::Call { name, args, .. } if name == "AccountId::parse" => {
            if args.len() != 1 {
                return Err(format!("`{name}` expects exactly one argument"));
            }
            let raw = eval_string_expr(&args[0])?;
            parse_account_literal(&raw)
        }
        other => Err(format!("expected account expression, got {other:?}")),
    }
}
fn eval_domain_expr(expr: &Expr) -> Result<DomainId, String> {
    match expr {
        Expr::String(raw) | Expr::Ident(raw) => parse_domain_literal(raw),
        Expr::Call { name, args, .. } if name == "DomainId::parse" => {
            if args.len() != 1 {
                return Err(format!("`{name}` expects exactly one argument"));
            }
            let raw = eval_string_expr(&args[0])?;
            parse_domain_literal(&raw)
        }
        other => Err(format!("expected domain expression, got {other:?}")),
    }
}
fn eval_asset_definition_expr(expr: &Expr) -> Result<AssetDefinitionId, String> {
    match expr {
        Expr::String(raw) | Expr::Ident(raw) => AssetDefinitionId::parse_address_literal(raw)
            .map_err(|_| format!("invalid asset definition id `{raw}`")),
        Expr::Call { name, args, .. } if name == "AssetDefinitionId::parse" => {
            if args.len() != 1 {
                return Err("`AssetDefinitionId::parse` expects exactly one argument".to_string());
            }
            let raw = eval_string_expr(&args[0])?;
            AssetDefinitionId::parse_address_literal(&raw)
                .map_err(|_| format!("invalid asset definition id `{raw}`"))
        }
        other => Err(format!(
            "expected asset definition expression, got {other:?}"
        )),
    }
}
fn eval_name_expr(expr: &Expr) -> Result<Name, String> {
    match expr {
        Expr::String(raw) | Expr::Ident(raw) => {
            Name::from_str(raw).map_err(|_| format!("invalid name `{raw}`"))
        }
        Expr::Call { name, args, .. } if name == "Name::parse" => {
            if args.len() != 1 {
                return Err("`Name::parse` expects exactly one argument".to_string());
            }
            let raw = eval_string_expr(&args[0])?;
            Name::from_str(&raw).map_err(|_| format!("invalid name `{raw}`"))
        }
        other => Err(format!("expected name expression, got {other:?}")),
    }
}
fn eval_string_expr(expr: &Expr) -> Result<String, String> {
    match expr {
        Expr::String(raw) | Expr::Ident(raw) | Expr::DecimalLiteral(raw) => Ok(raw.clone()),
        Expr::IntLiteral(value) => Ok(value.to_string()),
        Expr::Bool(value) => Ok(value.to_string()),
        other => Err(format!("expected string-like expression, got {other:?}")),
    }
}
fn eval_numeric_expr(expr: &Expr) -> Result<Numeric, String> {
    match expr {
        Expr::IntLiteral(value) if !value.is_negative() => Ok(Numeric::new(value.clone(), 0)),
        Expr::DecimalLiteral(raw) | Expr::String(raw) => raw
            .replace('_', "")
            .parse::<Numeric>()
            .map_err(|_| format!("invalid numeric value `{raw}`")),
        Expr::IntLiteral(value) => Err(format!("negative balances are not allowed: {value}")),
        other => Err(format!("expected numeric expression, got {other:?}")),
    }
}
fn eval_quantity_expr(expr: &Expr) -> Result<Quantity, String> {
    let numeric = eval_numeric_expr(expr)?;
    Quantity::try_from_numeric(numeric)
        .map_err(|error| format!("balance must be a non-negative quantity: {error}"))
}
fn eval_u64_expr(expr: &Expr) -> Result<u64, String> {
    let raw = eval_string_expr(expr)?;
    let value = raw
        .parse::<u64>()
        .map_err(|_| format!("expected canonical unsigned integer, got `{raw}`"))?;
    if value.to_string() != raw {
        return Err(format!(
            "unsigned integer is not canonically encoded: `{raw}`"
        ));
    }
    Ok(value)
}
fn eval_mintable_expr(expr: &Expr) -> Result<Mintable, String> {
    let raw = eval_string_expr(expr)?.to_ascii_lowercase();
    match raw.as_str() {
        "+" | "infinite" | "infinitely" => Ok(Mintable::Infinitely),
        "=" | "once" => Ok(Mintable::Once),
        "-" | "not" | "never" => Ok(Mintable::Not),
        other => Err(format!("unsupported mintability `{other}`")),
    }
}
fn eval_permission_expr(expr: &Expr) -> Result<PermissionToken, String> {
    match expr {
        Expr::String(raw) | Expr::Ident(raw) => parse_permission_token_name(raw),
        Expr::Call { name, args, .. } if name == "Json::parse" => {
            if args.len() != 1 {
                return Err("`Json::parse` expects exactly one argument".to_string());
            }
            let payload = eval_json_payload(args)?;
            parse_permission_token_json(&payload)
        }
        other => Err(format!("expected permission expression, got {other:?}")),
    }
}
fn eval_detail_bytes(expr: &Expr) -> Result<Vec<u8>, String> {
    match expr {
        Expr::String(raw) => Ok(raw.as_bytes().to_vec()),
        Expr::IntLiteral(value) => Ok(value.to_string().into_bytes()),
        Expr::DecimalLiteral(raw) | Expr::Ident(raw) => Ok(raw.as_bytes().to_vec()),
        Expr::Bool(value) => Ok(value.to_string().into_bytes()),
        Expr::Call { name, args, .. } if name == "Json::parse" => {
            Ok(eval_json_payload(args)?.into_bytes())
        }
        other => Err(format!("unsupported account detail value `{other:?}`")),
    }
}
fn eval_state_payload_expr(expr: &Expr) -> Result<Vec<u8>, String> {
    let (kind, atom) = match expr {
        Expr::Bool(value) => (StateValueKindV1::Bool, StateValueAtomV1::Bool(*value)),
        Expr::IntLiteral(value) => (
            StateValueKindV1::Int,
            StateValueAtomV1::Pointer(
                crate::numeric_tlv::encode_int(value)
                    .map_err(|error| format!("invalid int state fixture: {error:?}"))?,
            ),
        ),
        Expr::DecimalLiteral(raw) => {
            let value = raw
                .replace('_', "")
                .parse::<Numeric>()
                .map_err(|_| format!("invalid decimal state fixture `{raw}`"))?;
            let value = DecimalValueV1::try_from_numeric(value)
                .map_err(|error| format!("invalid decimal state fixture: {error:?}"))?;
            (
                StateValueKindV1::Decimal,
                StateValueAtomV1::Pointer(
                    crate::numeric_tlv::encode_decimal(value.as_numeric())
                        .map_err(|error| format!("invalid decimal state fixture: {error:?}"))?,
                ),
            )
        }
        Expr::String(raw) | Expr::Ident(raw) => (
            StateValueKindV1::String,
            StateValueAtomV1::Pointer(make_tlv(PointerType::Blob, raw.as_bytes())),
        ),
        Expr::Bytes(bytes) => (
            StateValueKindV1::Bytes,
            StateValueAtomV1::Pointer(make_tlv(PointerType::Blob, bytes)),
        ),
        Expr::Call { name, .. } if name == "Json::parse" => (
            StateValueKindV1::Json,
            StateValueAtomV1::Pointer(eval_envelope_expr(expr)?),
        ),
        Expr::Call { name, .. } if name == "AccountId::parse" => (
            StateValueKindV1::AccountId,
            StateValueAtomV1::Pointer(eval_envelope_expr(expr)?),
        ),
        Expr::Call { name, .. } if name == "AssetDefinitionId::parse" => (
            StateValueKindV1::AssetDefinitionId,
            StateValueAtomV1::Pointer(eval_envelope_expr(expr)?),
        ),
        Expr::Call { name, .. } if name == "DomainId::parse" => (
            StateValueKindV1::DomainId,
            StateValueAtomV1::Pointer(eval_envelope_expr(expr)?),
        ),
        Expr::Call { name, .. } if name == "Name::parse" => (
            StateValueKindV1::Name,
            StateValueAtomV1::Pointer(eval_envelope_expr(expr)?),
        ),
        other => return Err(format!("unsupported state fixture value `{other:?}`")),
    };
    encode_state_leaf(kind, atom)
}
fn encode_state_leaf(kind: StateValueKindV1, atom: StateValueAtomV1) -> Result<Vec<u8>, String> {
    let schema = StateValueSchemaV1 {
        nodes: vec![StateValueNodeV1::Leaf(kind)],
    };
    if !schema.validate_atoms(std::slice::from_ref(&atom)) {
        return Err(format!("invalid {kind:?} state fixture atom"));
    }
    let schema_bytes = norito::encode_canonical(&schema)
        .map_err(|error| format!("failed to encode state fixture schema: {error}"))?;
    norito::encode_canonical(&StateValueRecordV1 {
        schema_hash: state_value_schema_hash_v1(&schema_bytes),
        atoms: vec![atom],
    })
    .map_err(|error| format!("failed to encode state fixture record: {error}"))
}
fn eval_envelope_expr(expr: &Expr) -> Result<Vec<u8>, String> {
    match expr {
        Expr::Bool(value) => make_norito_envelope(value),
        Expr::IntLiteral(value) => crate::numeric_tlv::encode_int(value)
            .map_err(|error| format!("invalid int fixture value: {error:?}")),
        Expr::DecimalLiteral(raw) => {
            let value = raw
                .replace('_', "")
                .parse::<Numeric>()
                .map_err(|_| format!("invalid decimal fixture value `{raw}`"))?;
            crate::numeric_tlv::encode_decimal(&value)
                .map_err(|error| format!("invalid decimal fixture value: {error:?}"))
        }
        Expr::String(raw) | Expr::Ident(raw) => Ok(make_tlv(PointerType::Blob, raw.as_bytes())),
        Expr::Bytes(bytes) => Ok(make_tlv(PointerType::Blob, bytes)),
        Expr::Call { name, args, .. } if name == "Json::parse" => {
            let payload = eval_json_payload(args)?;
            let value = Json::from_str_norito(&payload)
                .map_err(|error| format!("invalid JSON fixture value: {error}"))?;
            let encoded = norito::encode_canonical(&value)
                .map_err(|error| format!("failed to encode JSON fixture value: {error}"))?;
            Ok(make_tlv(PointerType::Json, &encoded))
        }
        Expr::Call { name, args, .. } if name == "AccountId::parse" => {
            if args.len() != 1 {
                return Err(format!("`{name}` expects exactly one argument"));
            }
            let account = eval_account_expr(expr)?;
            let bytes = norito::encode_canonical(&account)
                .map_err(|err| format!("failed to encode account id: {err}"))?;
            Ok(make_tlv(PointerType::AccountId, &bytes))
        }
        Expr::Call { name, args, .. } if name == "AssetDefinitionId::parse" => {
            if args.len() != 1 {
                return Err("`AssetDefinitionId::parse` expects exactly one argument".to_string());
            }
            let asset = eval_asset_definition_expr(expr)?;
            let bytes = norito::encode_canonical(&asset)
                .map_err(|err| format!("failed to encode asset definition id: {err}"))?;
            Ok(make_tlv(PointerType::AssetDefinitionId, &bytes))
        }
        Expr::Call { name, args, .. } if name == "DomainId::parse" => {
            if args.len() != 1 {
                return Err(format!("`{name}` expects exactly one argument"));
            }
            let domain = eval_domain_expr(expr)?;
            let bytes = norito::encode_canonical(&domain)
                .map_err(|err| format!("failed to encode domain id: {err}"))?;
            Ok(make_tlv(PointerType::DomainId, &bytes))
        }
        Expr::Call { name, args, .. } if name == "Name::parse" => {
            if args.len() != 1 {
                return Err("`Name::parse` expects exactly one argument".to_string());
            }
            let name = eval_name_expr(expr)?;
            let bytes = norito::encode_canonical(&name)
                .map_err(|err| format!("failed to encode name: {err}"))?;
            Ok(make_tlv(PointerType::Name, &bytes))
        }
        other => Err(format!("unsupported fixture value expression `{other:?}`")),
    }
}
fn eval_json_payload(args: &[Expr]) -> Result<String, String> {
    if args.len() != 1 {
        return Err("`Json::parse` expects exactly one argument".to_string());
    }
    match &args[0] {
        Expr::String(raw) => Ok(raw.clone()),
        other => Err(format!(
            "`Json::parse` expects a string payload, got {other:?}"
        )),
    }
}
fn make_norito_envelope<T: Encode>(value: &T) -> Result<Vec<u8>, String> {
    let bytes = norito::encode_canonical(value)
        .map_err(|err| format!("failed to encode canonical Norito value: {err}"))?;
    Ok(make_tlv(PointerType::NoritoBytes, &bytes))
}
fn make_tlv(pointer_type: PointerType, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(7 + payload.len() + iroha_crypto::Hash::LENGTH);
    out.extend_from_slice(&(pointer_type as u16).to_be_bytes());
    out.push(1);
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(payload);
    let hash: [u8; 32] = iroha_crypto::Hash::new(payload).into();
    out.extend_from_slice(&hash);
    out
}
fn parse_account_literal(raw: &str) -> Result<AccountId, String> {
    AccountId::parse_encoded(raw)
        .or_else(|_| {
            raw.parse::<iroha_data_model::smart_contract::ContractAddress>()
                .map(|address| address.subject_id())
        })
        .or_else(|_| raw.parse::<iroha_crypto::PublicKey>().map(AccountId::new))
        .map_err(|_| format!("invalid account id `{raw}`"))
}
fn default_caller_account() -> Result<AccountId, String> {
    let _chain_discriminant = ChainDiscriminantGuard::enter(753);
    parse_account_literal(DEFAULT_CALLER)
}
fn parse_domain_literal(raw: &str) -> Result<DomainId, String> {
    if raw.contains('.') {
        return DomainId::parse_fully_qualified(raw)
            .map_err(|_| format!("invalid domain id `{raw}`"));
    }
    DomainId::try_new(raw, "universal").map_err(|_| format!("invalid domain id `{raw}`"))
}
fn parse_permission_token_name(raw: &str) -> Result<PermissionToken, String> {
    if raw == "register_domain" {
        return Ok(PermissionToken::RegisterDomain);
    }
    if raw == "register_account" {
        return Ok(PermissionToken::RegisterAccount);
    }
    if raw == "register_asset_definition" {
        return Ok(PermissionToken::RegisterAssetDefinition);
    }
    if let Some(rest) = raw.strip_prefix("read_assets:") {
        return Ok(PermissionToken::ReadAccountAssets(parse_account_literal(
            rest,
        )?));
    }
    if let Some(rest) = raw.strip_prefix("add_signatory:") {
        return Ok(PermissionToken::AddSignatory(parse_account_literal(rest)?));
    }
    if let Some(rest) = raw.strip_prefix("remove_signatory:") {
        return Ok(PermissionToken::RemoveSignatory(parse_account_literal(
            rest,
        )?));
    }
    if let Some(rest) = raw.strip_prefix("set_account_quorum:") {
        return Ok(PermissionToken::SetAccountQuorum(parse_account_literal(
            rest,
        )?));
    }
    if let Some(rest) = raw.strip_prefix("set_account_detail:") {
        return Ok(PermissionToken::SetAccountDetail(parse_account_literal(
            rest,
        )?));
    }
    if let Some(rest) = raw.strip_prefix("register_zk_asset:") {
        return Ok(PermissionToken::RegisterZkAsset(
            AssetDefinitionId::parse_address_literal(rest)
                .map_err(|_| format!("invalid asset definition id `{rest}`"))?,
        ));
    }
    if let Some(rest) = raw.strip_prefix("mint_asset:") {
        return Ok(PermissionToken::MintAsset(
            AssetDefinitionId::parse_address_literal(rest)
                .map_err(|_| format!("invalid asset definition id `{rest}`"))?,
        ));
    }
    if let Some(rest) = raw.strip_prefix("burn_asset:") {
        return Ok(PermissionToken::BurnAsset(
            AssetDefinitionId::parse_address_literal(rest)
                .map_err(|_| format!("invalid asset definition id `{rest}`"))?,
        ));
    }
    if let Some(rest) = raw.strip_prefix("transfer_asset:") {
        return Ok(PermissionToken::TransferAsset(
            AssetDefinitionId::parse_address_literal(rest)
                .map_err(|_| format!("invalid asset definition id `{rest}`"))?,
        ));
    }
    match raw {
        "manage_roles" => Ok(PermissionToken::ManageRoles),
        "manage_permissions" => Ok(PermissionToken::ManagePermissions),
        "manage_triggers" => Ok(PermissionToken::ManageTriggers),
        "manage_peers" => Ok(PermissionToken::ManagePeers),
        _ if !raw.is_empty() => Ok(PermissionToken::Custom(raw.to_string())),
        _ => Err("permission name must not be empty".to_string()),
    }
}
fn parse_permission_token_json(raw: &str) -> Result<PermissionToken, String> {
    let value: Value =
        json::from_str(raw).map_err(|err| format!("invalid permission json: {err}"))?;
    let map = value
        .as_object()
        .ok_or_else(|| "permission json must be an object".to_string())?;
    let kind = map
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| "permission json is missing `type`".to_string())?;
    let target = |field: &str| -> Result<&str, String> {
        map.get(field)
            .and_then(Value::as_str)
            .ok_or_else(|| format!("permission json is missing `{field}`"))
    };
    let transfer_control_scope = || -> Result<(AssetDefinitionId, Name, DataSpaceId), String> {
        if map.len() != 4
            || !map.contains_key("type")
            || !map.contains_key("asset_definition")
            || !map.contains_key("account_domain")
            || !map.contains_key("account_dataspace")
        {
            return Err(
                "transfer-control permission json requires exactly type, asset_definition, account_domain, and account_dataspace"
                    .to_owned(),
            );
        }
        let asset_literal = target("asset_definition")?;
        let asset_definition = AssetDefinitionId::parse_address_literal(asset_literal)
            .map_err(|_| "invalid `asset_definition` canonical id".to_owned())?;
        let domain_literal = target("account_domain")?;
        let account_domain = Name::from_str(domain_literal)
            .map_err(|_| "invalid `account_domain` canonical Name".to_owned())?;
        if account_domain.as_ref() != domain_literal {
            return Err("`account_domain` is not canonically encoded".to_owned());
        }
        let account_dataspace = map
            .get("account_dataspace")
            .and_then(Value::as_u64)
            .map(DataSpaceId::new)
            .ok_or_else(|| "`account_dataspace` must be an unsigned integer".to_owned())?;
        Ok((asset_definition, account_domain, account_dataspace))
    };
    let exact_transfer_bucket = || -> Result<AssetId, String> {
        if map.len() != 2 || !map.contains_key("type") || !map.contains_key("asset") {
            return Err(
                "CanTransferAsset permission json requires exactly type and asset".to_owned(),
            );
        }
        let literal = target("asset")?;
        let asset = AssetId::parse_literal(literal)
            .map_err(|_| "invalid canonical `asset` balance-bucket id".to_owned())?;
        if asset.canonical_literal() != literal {
            return Err("`asset` balance-bucket id is not canonically encoded".to_owned());
        }
        Ok(asset)
    };
    let exact_account_asset_target =
        |permission: &str| -> Result<(AccountId, AssetDefinitionId), String> {
            if map.len() != 3
                || !map.contains_key("type")
                || !map.contains_key("account")
                || !map.contains_key("asset_definition")
            {
                return Err(format!(
                    "{permission} permission json requires exactly type, account, and asset_definition"
                ));
            }
            let account = parse_account_literal(target("account")?)?;
            let asset_definition =
                AssetDefinitionId::parse_address_literal(target("asset_definition")?)
                    .map_err(|_| "invalid `asset_definition` canonical id".to_owned())?;
            Ok((account, asset_definition))
        };
    match kind {
        "register_domain" => Ok(PermissionToken::RegisterDomain),
        "register_account" => Ok(PermissionToken::RegisterAccount),
        "register_asset_definition" => Ok(PermissionToken::RegisterAssetDefinition),
        "register_zk_asset" => Ok(PermissionToken::RegisterZkAsset(
            AssetDefinitionId::parse_address_literal(target("target")?)
                .map_err(|_| "invalid `target` asset definition id".to_string())?,
        )),
        "read_assets" => Ok(PermissionToken::ReadAccountAssets(parse_account_literal(
            target("target")?,
        )?)),
        "add_signatory" => Ok(PermissionToken::AddSignatory(parse_account_literal(
            target("target")?,
        )?)),
        "remove_signatory" => Ok(PermissionToken::RemoveSignatory(parse_account_literal(
            target("target")?,
        )?)),
        "set_account_quorum" => Ok(PermissionToken::SetAccountQuorum(parse_account_literal(
            target("target")?,
        )?)),
        "set_account_detail" => Ok(PermissionToken::SetAccountDetail(parse_account_literal(
            target("target")?,
        )?)),
        "mint_asset" => Ok(PermissionToken::MintAsset(
            AssetDefinitionId::parse_address_literal(target("target")?)
                .map_err(|_| "invalid `target` asset definition id".to_string())?,
        )),
        "burn_asset" => Ok(PermissionToken::BurnAsset(
            AssetDefinitionId::parse_address_literal(target("target")?)
                .map_err(|_| "invalid `target` asset definition id".to_string())?,
        )),
        "transfer_asset" => Ok(PermissionToken::TransferAsset(
            AssetDefinitionId::parse_address_literal(target("target")?)
                .map_err(|_| "invalid `target` asset definition id".to_string())?,
        )),
        "CanTransferAsset" => Ok(PermissionToken::TransferAssetBucket(
            exact_transfer_bucket()?
        )),
        "CanSetAssetTransferAvailability" => {
            let (account, asset_definition) =
                exact_account_asset_target("CanSetAssetTransferAvailability")?;
            Ok(PermissionToken::SetAssetTransferAvailability {
                account,
                asset_definition,
            })
        }
        "CanSetAssetTransferDailyLimit" => {
            let (asset_definition, account_domain, account_dataspace) = transfer_control_scope()?;
            Ok(PermissionToken::SetAssetTransferDailyLimit {
                asset_definition,
                account_domain,
                account_dataspace,
            })
        }
        "CanSetAssetHoldingLimit" => {
            let (account, asset_definition) =
                exact_account_asset_target("CanSetAssetHoldingLimit")?;
            Ok(PermissionToken::SetAssetHoldingLimit {
                account,
                asset_definition,
            })
        }
        "manage_roles" => Ok(PermissionToken::ManageRoles),
        "manage_permissions" => Ok(PermissionToken::ManagePermissions),
        "manage_triggers" => Ok(PermissionToken::ManageTriggers),
        "manage_peers" => Ok(PermissionToken::ManagePeers),
        "custom" => Ok(PermissionToken::Custom(target("name")?.to_string())),
        other => Err(format!("unsupported permission type `{other}`")),
    }
}
fn render_failure(vm: &IVM, extra_detail: Option<&str>, err: &crate::VMError) -> String {
    let mut message = format!("{err:?}");
    if let Some(diag) = vm.last_diagnostic() {
        if let Some(source) = &diag.source {
            if let Some(function) = &source.function {
                message.push_str(&format!(" at {function}"));
            }
            if let Some(line) = source.line {
                message.push_str(&format!(":{}:{}", line, source.column.unwrap_or(0)));
            }
        } else {
            message.push_str(&format!(" at pc {}", diag.pc));
        }
        if !diag.message.is_empty() && diag.message != message {
            message.push_str(&format!(" ({})", diag.message));
        }
    }
    if let Some(extra_detail) = extra_detail
        && !extra_detail.is_empty()
    {
        message.push_str(&format!(" [{extra_detail}]"));
    }
    message
}
fn print_run_summary(target_path: &Path, results: &[TestRunResult]) {
    println!("koto_test target: {}", target_path.display());
    for result in results {
        let status = if result.passed { "ok" } else { "FAILED" };
        println!(
            "{status:>6}  {}:{}  {}  ({:.2?})",
            target_path.display(),
            result.line,
            result.name,
            result.elapsed
        );
        if let Some(failure) = &result.failure {
            println!("       {failure}");
        }
    }
    let passed = results.iter().filter(|result| result.passed).count();
    println!(
        "\nresult: {}. {} passed; {} failed",
        if passed == results.len() {
            "ok"
        } else {
            "FAILED"
        },
        passed,
        results.len().saturating_sub(passed)
    );
}
fn print_test_list(suite: &DiscoveredSuite, format: TestOutputFormat) -> Result<(), String> {
    match format {
        TestOutputFormat::Text => {
            for test in &suite.tests {
                println!(
                    "{}:{}: {}",
                    suite.target_path.display(),
                    test.line,
                    test.name
                );
            }
        }
        TestOutputFormat::Json => {
            let tests = suite
                .tests
                .iter()
                .map(|test| {
                    json::object(vec![
                        ("name".to_owned(), Value::from(test.name.clone())),
                        ("line".to_owned(), Value::from(test.line as u64)),
                        (
                            "fixture".to_owned(),
                            test.fixture.clone().map_or(Value::Null, Value::from),
                        ),
                    ])
                    .unwrap_or(Value::Null)
                })
                .collect();
            let value = json::object(vec![
                (
                    "target".to_owned(),
                    Value::from(suite.target_path.display().to_string()),
                ),
                ("tests".to_owned(), Value::Array(tests)),
            ])
            .map_err(|error| format!("build test-list JSON: {error}"))?;
            println!(
                "{}",
                json::to_string_pretty(&value)
                    .map_err(|error| format!("serialize test-list JSON: {error}"))?
            );
        }
        TestOutputFormat::Junit => {
            return Err("JUnit output is not meaningful for `koto test list`".to_owned());
        }
    }
    Ok(())
}
fn emit_test_results(
    target_path: &Path,
    results: &[TestRunResult],
    format: TestOutputFormat,
    output_path: Option<&Path>,
    seed: u64,
) -> Result<(), String> {
    let rendered = match format {
        TestOutputFormat::Text => {
            print_run_summary(target_path, results);
            return Ok(());
        }
        TestOutputFormat::Json => render_test_json(target_path, results, seed)?,
        TestOutputFormat::Junit => render_test_junit(target_path, results, seed),
    };
    if let Some(path) = output_path {
        fs::write(path, rendered)
            .map_err(|error| format!("write test report {}: {error}", path.display()))?;
    } else {
        println!("{rendered}");
    }
    Ok(())
}
fn render_test_json(
    target_path: &Path,
    results: &[TestRunResult],
    seed: u64,
) -> Result<String, String> {
    let passed = results.iter().filter(|result| result.passed).count();
    let tests = results
        .iter()
        .map(|result| {
            json::object(vec![
                ("name".to_owned(), Value::from(result.name.clone())),
                ("line".to_owned(), Value::from(result.line as u64)),
                ("passed".to_owned(), Value::from(result.passed)),
                (
                    "duration_ns".to_owned(),
                    Value::from(u64::try_from(result.elapsed.as_nanos()).unwrap_or(u64::MAX)),
                ),
                (
                    "failure".to_owned(),
                    result.failure.clone().map_or(Value::Null, Value::from),
                ),
            ])
            .unwrap_or(Value::Null)
        })
        .collect();
    let value = json::object(vec![
        (
            "target".to_owned(),
            Value::from(target_path.display().to_string()),
        ),
        ("seed".to_owned(), Value::from(seed)),
        ("passed".to_owned(), Value::from(passed as u64)),
        (
            "failed".to_owned(),
            Value::from(results.len().saturating_sub(passed) as u64),
        ),
        ("tests".to_owned(), Value::Array(tests)),
    ])
    .map_err(|error| format!("build test JSON: {error}"))?;
    json::to_string_pretty(&value).map_err(|error| format!("serialize test JSON: {error}"))
}
fn render_test_junit(target_path: &Path, results: &[TestRunResult], seed: u64) -> String {
    use std::fmt::Write as _;
    let failed = results.iter().filter(|result| !result.passed).count();
    let duration = results
        .iter()
        .map(|result| result.elapsed.as_secs_f64())
        .sum::<f64>();
    let mut output = String::new();
    let _ = writeln!(output, "<?xml version=\"1.0\" encoding=\"UTF-8\"?>");
    let _ = writeln!(
        output,
        "<testsuite name=\"{}\" tests=\"{}\" failures=\"{}\" time=\"{duration:.9}\" seed=\"{seed}\">",
        escape_xml(&target_path.display().to_string()),
        results.len(),
        failed
    );
    for result in results {
        let _ = writeln!(
            output,
            "  <testcase name=\"{}\" classname=\"{}\" line=\"{}\" time=\"{:.9}\">",
            escape_xml(&result.name),
            escape_xml(&target_path.display().to_string()),
            result.line,
            result.elapsed.as_secs_f64()
        );
        if let Some(failure) = &result.failure {
            let _ = writeln!(
                output,
                "    <failure message=\"Kotodama test failed\">{}</failure>",
                escape_xml(failure)
            );
        }
        let _ = writeln!(output, "  </testcase>");
    }
    output.push_str("</testsuite>\n");
    output
}
fn escape_xml(raw: &str) -> String {
    let mut escaped = String::with_capacity(raw.len());
    for character in raw.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            other => escaped.push(other),
        }
    }
    escaped
}
fn print_coverage_report(compiled: &CompiledSuite, results: &[TestRunResult]) {
    let mut executed_pcs = HashSet::new();
    for result in results {
        executed_pcs.extend(result.trace_pcs.iter().copied());
    }
    let total_functions = compiled.coverage_functions.len();
    let covered_functions = compiled
        .coverage_functions
        .iter()
        .filter(|function| function_hit(function, &executed_pcs))
        .count();
    let total_bytes = compiled
        .coverage_functions
        .iter()
        .map(|function| function.pc_end.saturating_sub(function.pc_start))
        .sum::<u64>();
    let covered_bytes = compiled
        .coverage_functions
        .iter()
        .filter(|function| function_hit(function, &executed_pcs))
        .map(|function| function.pc_end.saturating_sub(function.pc_start))
        .sum::<u64>();
    let function_pct = percentage(covered_functions as u64, total_functions as u64);
    let byte_pct = percentage(covered_bytes, total_bytes);
    println!(
        "\ncoverage: {covered_functions}/{total_functions} functions ({function_pct:.1}%), {covered_bytes}/{total_bytes} bytecode-bytes ({byte_pct:.1}%)"
    );
    println!("covered  line  function");
    for function in &compiled.coverage_functions {
        let covered = if function_hit(function, &executed_pcs) {
            "yes"
        } else {
            "no "
        };
        println!(
            "{covered:>7}  {:>4}  {}",
            function.line, function.display_name
        );
    }
}
fn function_hit(function: &CoverageFunction, executed_pcs: &HashSet<u64>) -> bool {
    executed_pcs
        .iter()
        .any(|pc| function.pc_start <= *pc && *pc < function.pc_end)
}
fn percentage(numerator: u64, denominator: u64) -> f64 {
    if denominator == 0 {
        100.0
    } else {
        (numerator as f64 / denominator as f64) * 100.0
    }
}
fn print_profile_report(compiled: &CompiledSuite, results: &[TestRunResult]) -> Result<(), String> {
    println!("\nprofile:");
    let profile_artifact = compiled.profile_artifact();
    for result in results {
        for (cycle, entry) in result.delta_trace.iter().enumerate() {
            let source = profile_artifact.report.source_map.iter().find(|map_entry| {
                let start = profile_artifact.pc_base.saturating_add(map_entry.pc_start);
                let end = profile_artifact.pc_base.saturating_add(map_entry.pc_end);
                start <= entry.pc && entry.pc < end
            });
            let value = json::object(vec![
                ("test".to_string(), Value::from(result.name.clone())),
                ("cycle".to_string(), Value::from(cycle as u64)),
                ("pc".to_string(), Value::from(entry.pc)),
                (
                    "function".to_string(),
                    source
                        .and_then(|map_entry| {
                            normalize_user_function_name(&map_entry.function_name)
                        })
                        .map_or(Value::Null, |name| Value::from(name.to_string())),
                ),
                (
                    "line".to_string(),
                    source.map_or(Value::Null, |map_entry| {
                        Value::from(map_entry.source.line as u64)
                    }),
                ),
                (
                    "changed_registers".to_string(),
                    Value::Array(
                        entry
                            .changes
                            .iter()
                            .map(|(register, value, tag)| {
                                json::object(vec![
                                    ("register".to_string(), Value::from(*register as u64)),
                                    ("value".to_string(), Value::from(*value)),
                                    ("tag".to_string(), Value::from(*tag)),
                                ])
                                .unwrap_or(Value::Null)
                            })
                            .collect(),
                    ),
                ),
            ])
            .map_err(|err| format!("failed to build profile record: {err}"))?;
            let line = json::to_string(&value)
                .map_err(|err| format!("failed to serialize profile record: {err}"))?;
            println!("{line}");
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    include!("koto_test_driver_tests.rs");
}
