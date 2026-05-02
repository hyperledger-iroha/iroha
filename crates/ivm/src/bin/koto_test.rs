use std::{
    any::Any,
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    env, fs,
    path::{Path, PathBuf},
    str::FromStr,
    time::{Duration, Instant},
};

#[cfg(test)]
use ed25519_dalek::{Signature as Ed25519Signature, Verifier as _};
use ed25519_dalek::{Signer as _, SigningKey};
use iroha_data_model::prelude::{Mintable, Name};
use iroha_primitives::{json::Json, numeric::Numeric};
use ivm::{
    AccountId, AssetDefinitionId, DomainId, IVM, IVMHost, MockWorldStateView, PermissionToken,
    PointerType, ProgramMetadata, TraceMode, WsvHost,
    kotodama::{
        ast::{Expr, FixtureAction, FixtureDecl, FunctionKind, FunctionVisibility, Item, Program},
        compiler::{CompileReport, CompilerMode, CompilerOptions},
        parser,
    },
};
use norito::codec::Encode;
use norito::json::{self, Value};

const DEFAULT_CALLER: &str =
    "ed0120AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
const ENTRYPOINT_IMPL_PREFIX: &str = "__entrypoint_impl__";
const TEST_SYSCALL_ACTOR_ACCOUNT: u32 = ivm::syscalls::SYSCALL_KOTO_TEST_ACTOR_ACCOUNT;
const TEST_SYSCALL_ACTOR_PUBLIC_KEY: u32 = ivm::syscalls::SYSCALL_KOTO_TEST_ACTOR_PUBLIC_KEY;
const TEST_SYSCALL_ACTOR_SIGN: u32 = ivm::syscalls::SYSCALL_KOTO_TEST_ACTOR_SIGN;
const TEST_SYSCALL_INVOKE_ENTRYPOINT_AS: u32 =
    ivm::syscalls::SYSCALL_KOTO_TEST_INVOKE_ENTRYPOINT_AS;
const TEST_SYSCALL_EXPECT_REJECT_AS: u32 = ivm::syscalls::SYSCALL_KOTO_TEST_EXPECT_REJECT_AS;

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
}

#[derive(Clone, Debug)]
struct TestCase {
    name: String,
    fixture: Option<String>,
    line: usize,
}

struct DiscoveredSuite {
    target_path: PathBuf,
    target_program: Program,
    merged_program: Program,
    tests: Vec<TestCase>,
    fixtures: HashMap<String, FixtureDecl>,
}

struct CompiledSuite {
    code: Vec<u8>,
    runtime_code: Vec<u8>,
    report: CompileReport,
    pc_base: u64,
    runtime_entrypoints: HashMap<String, u64>,
    tests: Vec<CompiledTestCase>,
    fixtures: HashMap<String, FixtureDecl>,
    coverage_functions: Vec<CoverageFunction>,
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
    delta_trace: Vec<ivm::zk::DeltaEntry>,
}

struct KotoTestHost {
    inner: WsvHost,
    actors: HashMap<String, FixtureActor>,
    base_public_inputs: BTreeMap<Name, Vec<u8>>,
    entrypoints: HashMap<String, u64>,
    program: Vec<u8>,
    last_test_error: Option<String>,
    supplemental_trace_pcs: Vec<u64>,
    supplemental_delta_trace: Vec<ivm::zk::DeltaEntry>,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let (command, path) = parse_args(env::args().skip(1).collect())?;
    let suite = discover_suite(&path)?;
    let compiled = compile_suite(&suite)?;
    let trace_mode = match command {
        Command::Run => TraceMode::Off,
        Command::Coverage => TraceMode::PcOnly,
        Command::Profile => TraceMode::DeltaRegisters,
    };
    let results = execute_suite(&compiled, trace_mode)?;
    print_run_summary(&suite.target_path, &results);
    if command == Command::Coverage {
        print_coverage_report(&compiled, &results);
    }
    if command == Command::Profile {
        print_profile_report(&compiled, &results)?;
    }
    if results.iter().any(|result| !result.passed) {
        return Err("one or more Kotodama tests failed".to_string());
    }
    Ok(())
}

fn parse_args(args: Vec<String>) -> Result<(Command, PathBuf), String> {
    if args.len() != 2 {
        return Err(
            "usage: koto_test <run|coverage|profile> <path/to/program.ko|path/to/test.ko>"
                .to_string(),
        );
    }
    let command = match args[0].as_str() {
        "run" => Command::Run,
        "coverage" => Command::Coverage,
        "profile" => Command::Profile,
        other => {
            return Err(format!(
                "unknown subcommand `{other}`; expected one of: run, coverage, profile"
            ));
        }
    };
    Ok((command, PathBuf::from(&args[1])))
}

fn discover_suite(path: &Path) -> Result<DiscoveredSuite, String> {
    let input_path = fs::canonicalize(path)
        .map_err(|err| format!("failed to resolve {}: {err}", path.display()))?;
    let input_program = parse_program_file(&input_path)?;
    if input_program.test_target.is_some() {
        discover_suite_from_standalone_test(&input_path, input_program)
    } else {
        discover_suite_from_target(&input_path, input_program)
    }
}

fn discover_suite_from_target(
    path: &Path,
    target_program: Program,
) -> Result<DiscoveredSuite, String> {
    let mut merged_program = target_program.clone();
    let standalone_tests = discover_standalone_tests_for_target(path)?;
    for (test_path, test_program) in standalone_tests {
        validate_standalone_test_program(&test_path, path, &test_program)?;
        merged_program.items.extend(test_program.items.into_iter());
        merged_program
            .fixtures
            .extend(test_program.fixtures.into_iter());
    }
    merged_program.test_target = None;
    finalize_suite(path.to_path_buf(), target_program, merged_program)
}

fn discover_suite_from_standalone_test(
    test_path: &Path,
    test_program: Program,
) -> Result<DiscoveredSuite, String> {
    let target_decl = test_program.test_target.as_ref().ok_or_else(|| {
        format!(
            "{} is missing a koto_test target declaration",
            test_path.display()
        )
    })?;
    let target_path = resolve_target_path(test_path, &target_decl.target)?;
    let target_program = parse_program_file(&target_path)?;
    validate_standalone_test_program(test_path, &target_path, &test_program)?;

    let mut merged_program = target_program.clone();
    merged_program.items.extend(test_program.items);
    merged_program.fixtures.extend(test_program.fixtures);
    merged_program.test_target = None;
    finalize_suite(target_path, target_program, merged_program)
}

fn finalize_suite(
    target_path: PathBuf,
    target_program: Program,
    merged_program: Program,
) -> Result<DiscoveredSuite, String> {
    let tests = collect_tests(&merged_program)?;
    if tests.is_empty() {
        return Err(format!(
            "no #[test] Kotodama functions were found for {}",
            target_path.display()
        ));
    }
    let fixtures = build_fixture_map(&merged_program.fixtures)?;
    Ok(DiscoveredSuite {
        target_path,
        target_program,
        merged_program,
        tests,
        fixtures,
    })
}

fn parse_program_file(path: &Path) -> Result<Program, String> {
    let src = fs::read_to_string(path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    parser::parse(&src).map_err(|err| format!("{}: {err}", path.display()))
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
) -> Result<Vec<(PathBuf, Program)>, String> {
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
        let program = parse_program_file(&test_path)?;
        if let Some(test_target) = &program.test_target {
            let resolved = resolve_target_path(&test_path, &test_target.target)?;
            if resolved == target_path {
                discovered.push((test_path, program));
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
    if program.contract_meta.is_some() {
        return Err(format!(
            "{} must not declare seiyaku/meta blocks in standalone test mode",
            test_path.display()
        ));
    }
    for item in &program.items {
        match item {
            Item::Function(func) => {
                if func.modifiers.visibility != FunctionVisibility::Internal
                    || matches!(
                        func.modifiers.kind,
                        FunctionKind::Hajimari | FunctionKind::Kaizen | FunctionKind::View
                    )
                {
                    return Err(format!(
                        "{} contains a non-local function `{}`; standalone test files may only define internal helpers and #[test] functions",
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
            Item::Struct(_) | Item::Const(_) | Item::Kotoba(_) => {}
        }
    }
    Ok(())
}

fn collect_tests(program: &Program) -> Result<Vec<TestCase>, String> {
    let mut tests = Vec::new();
    let mut names = HashSet::new();
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
    Ok(tests)
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

fn compile_suite(suite: &DiscoveredSuite) -> Result<CompiledSuite, String> {
    let test_opts = CompilerOptions {
        mode: CompilerMode::Test,
        debug_source_name: Some(suite.target_path.display().to_string()),
        ..CompilerOptions::default()
    };
    let test_compiler = ivm::KotodamaCompiler::new_with_options(test_opts);
    let (code, _manifest, test_report) =
        test_compiler.compile_program_with_manifest_and_report(&suite.merged_program)?;
    let runtime_opts = CompilerOptions {
        debug_source_name: Some(suite.target_path.display().to_string()),
        ..CompilerOptions::default()
    };
    let runtime_compiler = ivm::KotodamaCompiler::new_with_options(runtime_opts);
    let (runtime_code, _runtime_manifest, runtime_report) =
        runtime_compiler.compile_program_with_manifest_and_report(&suite.target_program)?;
    let metadata = ProgramMetadata::parse(&code)
        .map_err(|err| format!("failed to parse compiled program metadata: {err:?}"))?;
    let test_pc_base = metadata.literal_prefix_len() as u64;
    let runtime_metadata = ProgramMetadata::parse(&runtime_code)
        .map_err(|err| format!("failed to parse runtime program metadata: {err:?}"))?;
    let runtime_pc_base = runtime_metadata.literal_prefix_len() as u64;
    let runtime_entrypoints = runtime_metadata
        .contract_interface
        .as_ref()
        .map(|interface| {
            interface
                .entrypoints
                .iter()
                .map(|entry| {
                    (
                        entry.name.clone(),
                        runtime_pc_base.saturating_add(entry.entry_pc),
                    )
                })
                .collect::<HashMap<_, _>>()
        })
        .unwrap_or_default();

    let mut test_pcs = HashMap::new();
    for entry in &test_report.source_map {
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

    let coverage_functions =
        build_coverage_functions(&suite.target_program, &runtime_report, runtime_pc_base);

    Ok(CompiledSuite {
        code,
        runtime_code,
        report: runtime_report,
        pc_base: runtime_pc_base,
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
        .source_map
        .iter()
        .filter_map(|entry| entry.function_name.strip_prefix(ENTRYPOINT_IMPL_PREFIX))
        .map(ToOwned::to_owned)
        .collect::<HashSet<_>>();

    let mut functions = report
        .source_map
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
                line: entry.source.line,
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

fn execute_suite(
    compiled: &CompiledSuite,
    trace_mode: TraceMode,
) -> Result<Vec<TestRunResult>, String> {
    let parsed = ProgramMetadata::parse(&compiled.code)
        .map_err(|err| format!("failed to parse compiled suite metadata: {err:?}"))?;
    let suite_return_pc = (compiled.code.len().saturating_sub(parsed.header_len)) as u64;
    let suite_program = with_return_halt(&compiled.code);
    let mut results = Vec::with_capacity(compiled.tests.len());
    for test in &compiled.tests {
        let mut host = build_host_for_fixture(compiled, test.fixture.as_deref())?;
        let mut vm = IVM::new(u64::MAX);
        vm.load_program(&suite_program)
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

        results.push(TestRunResult {
            name: test.name.clone(),
            line: test.line,
            elapsed,
            passed,
            failure,
            trace_pcs,
            delta_trace,
        });
    }
    Ok(results)
}

fn with_return_halt(program: &[u8]) -> Vec<u8> {
    let mut with_halt = program.to_vec();
    with_halt.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    with_halt
}

fn build_host_for_fixture(
    compiled: &CompiledSuite,
    fixture_name: Option<&str>,
) -> Result<KotoTestHost, String> {
    let caller = parse_account_literal(DEFAULT_CALLER)?;
    let base_host =
        WsvHost::new_with_subject(MockWorldStateView::default(), caller, HashMap::new());
    let mut host = KotoTestHost::new(
        base_host,
        compiled.runtime_code.clone(),
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
            let amount = eval_numeric_expr(&action.args[2])?;
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
            let value = eval_envelope_expr(&action.args[1])?;
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
    supplemental_delta_trace: Vec<ivm::zk::DeltaEntry>,
}

impl KotoTestHost {
    fn new(inner: WsvHost, program: Vec<u8>, entrypoints: HashMap<String, u64>) -> Self {
        Self {
            inner,
            actors: HashMap::new(),
            base_public_inputs: BTreeMap::new(),
            entrypoints,
            program,
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

    fn supplemental_delta_trace(&self) -> &[ivm::zk::DeltaEntry] {
        &self.supplemental_delta_trace
    }

    fn clear_test_error(&mut self) {
        self.last_test_error = None;
    }

    fn restore_public_inputs(&mut self) {
        self.inner
            .set_public_inputs(self.base_public_inputs.clone());
    }

    fn fail_test<T>(&mut self, message: impl Into<String>) -> Result<T, ivm::VMError> {
        self.last_test_error = Some(message.into());
        Err(ivm::VMError::AssertionFailed)
    }

    fn decode_alias_arg(vm: &IVM, reg: usize, label: &str) -> Result<String, ivm::VMError> {
        let ptr = vm.register(reg);
        if ptr == 0 {
            return Err(ivm::VMError::NoritoInvalid);
        }
        let tlv = vm.validate_tlv(ptr)?;
        match tlv.type_id {
            PointerType::Blob | PointerType::NoritoBytes => {
                String::from_utf8(tlv.payload.to_vec()).map_err(|_| ivm::VMError::DecodeError)
            }
            PointerType::Name => {
                let name: Name = norito::decode_from_bytes(tlv.payload)
                    .map_err(|_| ivm::VMError::DecodeError)?;
                Ok(name.as_ref().to_string())
            }
            _ => {
                let _ = label;
                Err(ivm::VMError::NoritoInvalid)
            }
        }
    }

    fn decode_json_arg(vm: &IVM, reg: usize) -> Result<Json, ivm::VMError> {
        let ptr = vm.register(reg);
        if ptr == 0 {
            return Err(ivm::VMError::NoritoInvalid);
        }
        let tlv = vm.validate_tlv(ptr)?;
        match tlv.type_id {
            PointerType::Json | PointerType::NoritoBytes | PointerType::Blob => {
                norito::decode_from_bytes(tlv.payload).map_err(|_| ivm::VMError::DecodeError)
            }
            _ => Err(ivm::VMError::NoritoInvalid),
        }
    }

    fn decode_bytes_arg(vm: &IVM, reg: usize) -> Result<Vec<u8>, ivm::VMError> {
        let ptr = vm.register(reg);
        if ptr == 0 {
            return Err(ivm::VMError::NoritoInvalid);
        }
        let tlv = vm.validate_tlv(ptr)?;
        match tlv.type_id {
            PointerType::Blob | PointerType::NoritoBytes => Ok(tlv.payload.to_vec()),
            _ => Err(ivm::VMError::NoritoInvalid),
        }
    }

    fn alloc_pointer_result(
        vm: &mut IVM,
        pointer_type: PointerType,
        payload: &[u8],
    ) -> Result<u64, ivm::VMError> {
        let tlv = make_tlv(pointer_type, payload);
        vm.alloc_input_tlv(&tlv)
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
        err: &ivm::VMError,
    ) -> String {
        format!(
            "actor `{actor_alias}` calling `{entrypoint}` with payload {:?} failed: {}",
            payload,
            render_failure(nested_vm, None, err)
        )
    }

    fn invoke_entrypoint(
        &mut self,
        vm: &mut IVM,
        expect_reject: bool,
    ) -> Result<u64, ivm::VMError> {
        self.clear_test_error();
        let actor_alias =
            Self::decode_alias_arg(vm, 10, "actor").map_err(|_| ivm::VMError::NoritoInvalid)?;
        let entrypoint = Self::decode_alias_arg(vm, 11, "entrypoint")
            .map_err(|_| ivm::VMError::NoritoInvalid)?;
        let payload = Self::decode_json_arg(vm, 12)?;
        let returns_pointer = vm.register(13) != 0;
        let actor = match self.actors.get(&actor_alias).cloned() {
            Some(actor) => actor,
            None => {
                return self.fail_test(format!(
                    "unknown actor `{actor_alias}` while calling `{entrypoint}`"
                ));
            }
        };
        let entry_pc = match self.entrypoints.get(&entrypoint).copied() {
            Some(pc) => pc,
            None => {
                return self.fail_test(format!(
                    "unknown runtime entrypoint `{entrypoint}` for actor `{actor_alias}`"
                ));
            }
        };

        let rollback = self
            .inner
            .checkpoint()
            .ok_or(ivm::VMError::HostUnavailable)?;
        let previous_caller = self.inner.caller_subject();

        self.inner.set_caller_subject(actor.account.clone());
        let trigger_name: Name = "trigger_event_json"
            .parse()
            .map_err(|_| ivm::VMError::DecodeError)?;
        let mut nested_inputs = self.base_public_inputs.clone();
        let encoded_payload = norito::to_bytes(&payload).map_err(|_| ivm::VMError::DecodeError)?;
        nested_inputs.insert(trigger_name, make_tlv(PointerType::Json, &encoded_payload));
        self.inner.set_public_inputs(nested_inputs);

        let mut nested_vm = IVM::new(u64::MAX);
        nested_vm.reset();
        let clear = [0u8; 7 + iroha_crypto::Hash::LENGTH];
        nested_vm
            .memory
            .preload_input(0, &clear)
            .map_err(|_| ivm::VMError::DecodeError)?;
        let runtime_program = with_return_halt(&self.program);
        nested_vm
            .load_program(&runtime_program)
            .map_err(|_| ivm::VMError::DecodeError)?;
        nested_vm.set_program_counter(entry_pc)?;
        nested_vm.set_register(1, nested_vm.memory.code_len().saturating_sub(4));
        nested_vm.set_trace_mode(vm.trace_mode());
        nested_vm.set_max_cycles(0);
        let nested_outcome = nested_vm.run_with_host(&mut self.inner);
        self.record_nested_trace(&nested_vm);

        match nested_outcome {
            Ok(()) if expect_reject => {
                let _ = self.inner.restore(rollback.as_ref());
                self.fail_test(format!(
                    "expected actor `{actor_alias}` calling `{entrypoint}` with payload {:?} to reject, but it succeeded",
                    payload
                ))
            }
            Ok(()) => {
                self.inner.set_caller_subject(previous_caller);
                self.restore_public_inputs();
                let value = nested_vm.register(10);
                if returns_pointer && value != 0 {
                    let tlv = nested_vm.clone_tlv(value)?;
                    let ptr = vm.alloc_input_tlv(&tlv)?;
                    vm.set_register(10, ptr);
                } else {
                    vm.set_register(10, value);
                }
                Ok(0)
            }
            Err(err) if expect_reject => {
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
    fn syscall(&mut self, number: u32, vm: &mut IVM) -> Result<u64, ivm::VMError> {
        match number {
            TEST_SYSCALL_ACTOR_ACCOUNT => {
                self.clear_test_error();
                let alias = Self::decode_alias_arg(vm, 10, "actor")?;
                let Some(actor) = self.actors.get(&alias) else {
                    return self.fail_test(format!("unknown actor `{alias}`"));
                };
                let payload =
                    norito::to_bytes(&actor.account).map_err(|_| ivm::VMError::NoritoInvalid)?;
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

    fn allows_syscall(&mut self, policy: ivm::SyscallPolicy, number: u32) -> bool {
        ivm::syscalls::is_koto_test_syscall(number)
            || ivm::syscalls::is_syscall_allowed(policy, number)
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

    fn begin_tx(&mut self, declared: &ivm::parallel::StateAccessSet) -> Result<(), ivm::VMError> {
        self.inner.begin_tx(declared)
    }

    fn finish_tx(&mut self) -> Result<ivm::host::AccessLog, ivm::VMError> {
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
        Expr::Call { name, args } if name == "name" => {
            if args.len() != 1 {
                return Err("`name` expects exactly one argument".to_string());
            }
            eval_string_expr(&args[0])
        }
        other => Err(format!("expected actor alias expression, got {other:?}")),
    }
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
        Expr::String(raw) | Expr::Ident(raw) | Expr::Decimal(raw) => decode_hex_or_raw_bytes(raw)?,
        Expr::Call { name, args } if matches!(name.as_str(), "blob" | "norito_bytes") => {
            if args.len() != 1 {
                return Err(format!("`{name}` expects exactly one argument"));
            }
            let raw = eval_string_expr(&args[0])?;
            decode_hex_or_raw_bytes(&raw)?
        }
        other => return Err(format!("expected 32-byte seed blob, got {other:?}")),
    };
    <[u8; 32]>::try_from(bytes.as_slice())
        .map_err(|_| format!("actor seed must be exactly 32 bytes, got {}", bytes.len()))
}

fn eval_account_expr(expr: &Expr) -> Result<AccountId, String> {
    match expr {
        Expr::String(raw) | Expr::Ident(raw) => parse_account_literal(raw),
        Expr::Call { name, args } if matches!(name.as_str(), "account" | "account_id") => {
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
        Expr::Call { name, args } if matches!(name.as_str(), "domain" | "domain_id") => {
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
        Expr::Call { name, args } if name == "asset_definition" => {
            if args.len() != 1 {
                return Err("`asset_definition` expects exactly one argument".to_string());
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
        Expr::Call { name, args } if name == "name" => {
            if args.len() != 1 {
                return Err("`name` expects exactly one argument".to_string());
            }
            let raw = eval_string_expr(&args[0])?;
            Name::from_str(&raw).map_err(|_| format!("invalid name `{raw}`"))
        }
        other => Err(format!("expected name expression, got {other:?}")),
    }
}

fn eval_string_expr(expr: &Expr) -> Result<String, String> {
    match expr {
        Expr::String(raw) | Expr::Ident(raw) | Expr::Decimal(raw) => Ok(raw.clone()),
        Expr::Number(value) => Ok(value.to_string()),
        Expr::Bool(value) => Ok(value.to_string()),
        other => Err(format!("expected string-like expression, got {other:?}")),
    }
}

fn eval_numeric_expr(expr: &Expr) -> Result<Numeric, String> {
    match expr {
        Expr::Number(value) if *value >= 0 => Ok(Numeric::from(*value as u64)),
        Expr::Decimal(raw) | Expr::String(raw) => raw
            .parse::<Numeric>()
            .map_err(|_| format!("invalid numeric value `{raw}`")),
        Expr::Number(value) => Err(format!("negative balances are not allowed: {value}")),
        other => Err(format!("expected numeric expression, got {other:?}")),
    }
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
        Expr::Call { name, args } if name == "json" => {
            if args.len() != 1 {
                return Err("`json` expects exactly one argument".to_string());
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
        Expr::Number(value) => Ok(value.to_string().into_bytes()),
        Expr::Decimal(raw) | Expr::Ident(raw) => Ok(raw.as_bytes().to_vec()),
        Expr::Bool(value) => Ok(value.to_string().into_bytes()),
        Expr::Call { name, args } if name == "json" => Ok(eval_json_payload(args)?.into_bytes()),
        other => Err(format!("unsupported account detail value `{other:?}`")),
    }
}

fn eval_envelope_expr(expr: &Expr) -> Result<Vec<u8>, String> {
    match expr {
        Expr::Bool(value) => make_norito_envelope(value),
        Expr::Number(value) => make_norito_envelope(value),
        Expr::Decimal(raw) | Expr::String(raw) | Expr::Ident(raw) => make_norito_envelope(raw),
        Expr::Bytes(bytes) => Ok(make_tlv(PointerType::Blob, bytes)),
        Expr::Call { name, args } if name == "json" => {
            let payload = eval_json_payload(args)?;
            Ok(make_tlv(PointerType::Json, payload.as_bytes()))
        }
        Expr::Call { name, args } if matches!(name.as_str(), "account" | "account_id") => {
            if args.len() != 1 {
                return Err(format!("`{name}` expects exactly one argument"));
            }
            let account = eval_account_expr(expr)?;
            let bytes = norito::to_bytes(&account)
                .map_err(|err| format!("failed to encode account id: {err}"))?;
            Ok(make_tlv(PointerType::AccountId, &bytes))
        }
        Expr::Call { name, args } if name == "asset_definition" => {
            if args.len() != 1 {
                return Err("`asset_definition` expects exactly one argument".to_string());
            }
            let asset = eval_asset_definition_expr(expr)?;
            let bytes = norito::to_bytes(&asset)
                .map_err(|err| format!("failed to encode asset definition id: {err}"))?;
            Ok(make_tlv(PointerType::AssetDefinitionId, &bytes))
        }
        Expr::Call { name, args } if matches!(name.as_str(), "domain" | "domain_id") => {
            if args.len() != 1 {
                return Err(format!("`{name}` expects exactly one argument"));
            }
            let domain = eval_domain_expr(expr)?;
            let bytes = norito::to_bytes(&domain)
                .map_err(|err| format!("failed to encode domain id: {err}"))?;
            Ok(make_tlv(PointerType::DomainId, &bytes))
        }
        Expr::Call { name, args } if name == "name" => {
            if args.len() != 1 {
                return Err("`name` expects exactly one argument".to_string());
            }
            let name = eval_name_expr(expr)?;
            let bytes =
                norito::to_bytes(&name).map_err(|err| format!("failed to encode name: {err}"))?;
            Ok(make_tlv(PointerType::Name, &bytes))
        }
        other => Err(format!("unsupported fixture value expression `{other:?}`")),
    }
}

fn eval_json_payload(args: &[Expr]) -> Result<String, String> {
    if args.len() != 1 {
        return Err("`json` expects exactly one argument".to_string());
    }
    match &args[0] {
        Expr::String(raw) => Ok(raw.clone()),
        other => Err(format!("`json` expects a string payload, got {other:?}")),
    }
}

fn make_norito_envelope<T: Encode>(value: &T) -> Result<Vec<u8>, String> {
    let bytes =
        norito::to_bytes(value).map_err(|err| format!("failed to encode Norito value: {err}"))?;
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
        .map(iroha_data_model::account::ParsedAccountId::into_account_id)
        .or_else(|_| {
            raw.parse::<iroha_data_model::smart_contract::ContractAddress>()
                .map(|address| address.subject_id())
        })
        .or_else(|_| raw.parse::<iroha_crypto::PublicKey>().map(AccountId::new))
        .map_err(|_| format!("invalid account id `{raw}`"))
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
    if let Some(rest) = raw.strip_prefix("shield:") {
        return Ok(PermissionToken::Shield(
            AssetDefinitionId::parse_address_literal(rest)
                .map_err(|_| format!("invalid asset definition id `{rest}`"))?,
        ));
    }
    if let Some(rest) = raw.strip_prefix("unshield:") {
        return Ok(PermissionToken::Unshield(
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
        "shield" => Ok(PermissionToken::Shield(
            AssetDefinitionId::parse_address_literal(target("target")?)
                .map_err(|_| "invalid `target` asset definition id".to_string())?,
        )),
        "unshield" => Ok(PermissionToken::Unshield(
            AssetDefinitionId::parse_address_literal(target("target")?)
                .map_err(|_| "invalid `target` asset definition id".to_string())?,
        )),
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
        "manage_roles" => Ok(PermissionToken::ManageRoles),
        "manage_permissions" => Ok(PermissionToken::ManagePermissions),
        "manage_triggers" => Ok(PermissionToken::ManageTriggers),
        "manage_peers" => Ok(PermissionToken::ManagePeers),
        "custom" => Ok(PermissionToken::Custom(target("name")?.to_string())),
        other => Err(format!("unsupported permission type `{other}`")),
    }
}

fn render_failure(vm: &IVM, extra_detail: Option<&str>, err: &ivm::VMError) -> String {
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
    for result in results {
        for (cycle, entry) in result.delta_trace.iter().enumerate() {
            let source = compiled.report.source_map.iter().find(|map_entry| {
                let start = compiled.pc_base.saturating_add(map_entry.pc_start);
                let end = compiled.pc_base.saturating_add(map_entry.pc_end);
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
    use super::*;
    use std::{
        sync::atomic::{AtomicUsize, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    static TEMP_DIR_COUNTER: AtomicUsize = AtomicUsize::new(0);

    struct TestTempDir {
        path: PathBuf,
    }

    impl TestTempDir {
        fn new() -> Self {
            let nonce = TEMP_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "koto_test_bin_{}_{}_{}",
                std::process::id(),
                timestamp,
                nonce
            ));
            fs::create_dir_all(&path).expect("create temp dir");
            Self { path }
        }

        fn write(&self, relative: &str, contents: &str) -> PathBuf {
            let path = self.path.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("create parent dir");
            }
            fs::write(&path, contents).expect("write temp file");
            path
        }
    }

    impl Drop for TestTempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn test_function(name: &str, fixture: Option<&str>) -> Item {
        Item::Function(ivm::kotodama::ast::Function {
            name: name.to_string(),
            params: Vec::new(),
            ret_ty: None,
            body: ivm::kotodama::ast::Block {
                statements: Vec::new(),
            },
            modifiers: ivm::kotodama::ast::FunctionModifiers {
                is_test: true,
                test_fixture: fixture.map(str::to_string),
                ..Default::default()
            },
            location: ivm::kotodama::ast::SourceLocation { line: 1, column: 1 },
        })
    }

    fn compiled_suite_with_fixtures(fixtures: Vec<FixtureDecl>) -> CompiledSuite {
        let target_program = Program {
            items: vec![Item::Function(ivm::kotodama::ast::Function {
                name: "helper".to_string(),
                params: Vec::new(),
                ret_ty: None,
                body: ivm::kotodama::ast::Block {
                    statements: Vec::new(),
                },
                modifiers: Default::default(),
                location: ivm::kotodama::ast::SourceLocation { line: 1, column: 1 },
            })],
            contract_meta: None,
            test_target: None,
            fixtures: fixtures.clone(),
        };
        let mut merged_program = target_program.clone();
        merged_program.items.push(test_function("smoke", None));
        let suite = DiscoveredSuite {
            target_path: PathBuf::from("/tmp/fixture_demo.ko"),
            target_program,
            merged_program,
            tests: vec![TestCase {
                name: "smoke".to_string(),
                fixture: None,
                line: 1,
            }],
            fixtures: build_fixture_map(&fixtures).expect("build fixture map"),
        };
        compile_suite(&suite).expect("compile fixture suite")
    }

    #[test]
    fn parse_args_accepts_supported_subcommands() {
        let (command, path) = parse_args(vec![
            "coverage".to_string(),
            "contracts/demo.ko".to_string(),
        ])
        .expect("parse args");
        assert_eq!(command, Command::Coverage);
        assert_eq!(path, PathBuf::from("contracts/demo.ko"));
    }

    #[test]
    fn parse_args_rejects_unknown_subcommand_and_missing_path() {
        let err = parse_args(vec!["wat".to_string(), "demo.ko".to_string()])
            .expect_err("unknown command should fail");
        assert!(err.contains("unknown subcommand"));

        let err = parse_args(vec!["run".to_string()]).expect_err("missing path should fail");
        assert!(err.contains("usage: koto_test"));
    }

    #[test]
    fn discover_suite_merges_inline_and_matching_standalone_tests() {
        let temp = TestTempDir::new();
        let target = temp.write(
            "contracts/demo.ko",
            r#"
            fn increment(x: int) -> int { return x + 1; }

            #[test]
            fn inline() {
              assert_eq(increment(1), 2);
            }
            "#,
        );
        temp.write(
            "contracts/demo.test.ko",
            r#"
            koto_test { target: "demo.ko" }

            #[test]
            fn standalone() {
              assert_eq(increment(2), 3);
            }
            "#,
        );
        temp.write("contracts/other.ko", "fn other() {}");
        temp.write(
            "contracts/tests/ignored.test.ko",
            r#"
            koto_test { target: "../other.ko" }

            #[test]
            fn ignored() {}
            "#,
        );

        let suite = discover_suite(&target).expect("discover suite");
        let mut names = suite
            .tests
            .iter()
            .map(|test| test.name.clone())
            .collect::<Vec<_>>();
        names.sort();
        assert_eq!(names, vec!["inline".to_string(), "standalone".to_string()]);
    }

    #[test]
    fn discover_suite_from_standalone_input_uses_target_program() {
        let temp = TestTempDir::new();
        temp.write(
            "contracts/demo.ko",
            r#"
            fn increment(x: int) -> int { return x + 1; }
            "#,
        );
        let standalone = temp.write(
            "contracts/demo.test.ko",
            r#"
            koto_test { target: "demo.ko" }

            #[test]
            fn smoke() {
              assert_eq(increment(2), 3);
            }
            "#,
        );

        let suite = discover_suite(&standalone).expect("discover suite from standalone input");
        assert_eq!(
            suite.target_path.file_name().and_then(|name| name.to_str()),
            Some("demo.ko")
        );
        assert_eq!(suite.tests.len(), 1);
        assert_eq!(suite.tests[0].name, "smoke");
    }

    #[test]
    fn execute_suite_supports_native_contract_flow_helpers() {
        let temp = TestTempDir::new();
        let actor_seed = [9_u8; 32];
        let signing_key = SigningKey::from_bytes(&actor_seed);
        let actor_public_key = iroha_crypto::PublicKey::from_bytes(
            iroha_crypto::Algorithm::Ed25519,
            signing_key.verifying_key().as_bytes(),
        )
        .expect("public key");
        let actor_account = AccountId::new(actor_public_key)
            .canonical_i105()
            .expect("canonical actor account");
        temp.write(
            "contracts/contract_flow_demo.ko",
            r#"
            seiyaku Demo {
                state int counter;
                state AccountId last_actor;

                hajimari() {
                    counter = 1;
                }

                #[access(read="*", write="*")]
                kotoage fn increment() {
                    counter = counter + 4;
                }

                #[access(read="*", write="*")]
                kotoage fn remember_caller() {
                    last_actor = authority();
                }

                #[access(read="*", write="*")]
                kotoage fn reject_me() {
                    assert_eq(1, 2);
                }
            }
            "#,
        );
        let test_path = temp.write(
            "contracts/contract_flow_demo.test.ko",
            &format!(
                r#"
                koto_test {{ target: "contract_flow_demo.ko" }}

                fixture actors {{
                    actor("issuer", account_id("{actor_account}"), "0x{seed_hex}");
                }}

                #[test(fixture="actors")]
                fn drive_contract_flow() {{
                    invoke_entrypoint_as("issuer", "hajimari", json("{{}}"));
                    invoke_entrypoint_as("issuer", "increment", json("{{}}"));
                    invoke_entrypoint_as(
                        "issuer",
                        "remember_caller",
                        json("{{}}")
                    );

                    expect_reject_as("issuer", "reject_me", json("{{}}"));
                }}
                "#,
                actor_account = actor_account,
                seed_hex = hex::encode(actor_seed),
            ),
        );

        let suite = discover_suite(&test_path).expect("discover suite");
        let compiled = compile_suite(&suite).expect("compile suite");
        let mut host = build_host_for_fixture(&compiled, Some("actors")).expect("build host");
        let mut vm = IVM::new(u64::MAX);
        vm.set_trace_mode(TraceMode::PcOnly);

        let put_blob = |vm: &mut IVM, reg: usize, value: &str| {
            let ptr = vm
                .alloc_input_tlv(&make_tlv(PointerType::Blob, value.as_bytes()))
                .expect("blob tlv");
            vm.set_register(reg, ptr);
        };
        let put_json = |vm: &mut IVM, reg: usize, raw: &str| {
            let json = Json::from_str_norito(raw).expect("json payload");
            let bytes = norito::to_bytes(&json).expect("json norito");
            let ptr = vm
                .alloc_input_tlv(&make_tlv(PointerType::Json, &bytes))
                .expect("json tlv");
            vm.set_register(reg, ptr);
        };

        put_blob(&mut vm, 10, "issuer");
        host.syscall(TEST_SYSCALL_ACTOR_ACCOUNT, &mut vm)
            .expect("actor account syscall");
        let actor_tlv = vm
            .validate_input_tlv(vm.register(10))
            .expect("actor account tlv");
        assert_eq!(actor_tlv.type_id, PointerType::AccountId);
        let decoded_actor: AccountId =
            norito::decode_from_bytes(actor_tlv.payload).expect("decode actor account");
        assert_eq!(
            decoded_actor
                .canonical_i105()
                .expect("canonical decoded actor"),
            actor_account
        );

        put_blob(&mut vm, 10, "issuer");
        host.syscall(TEST_SYSCALL_ACTOR_PUBLIC_KEY, &mut vm)
            .expect("actor public key syscall");
        let public_key_tlv = vm
            .validate_input_tlv(vm.register(10))
            .expect("public key tlv");
        assert_eq!(public_key_tlv.type_id, PointerType::Blob);
        assert_eq!(
            public_key_tlv.payload,
            signing_key.verifying_key().as_bytes()
        );

        put_blob(&mut vm, 10, "issuer");
        let message_ptr = vm
            .alloc_input_tlv(&make_tlv(PointerType::Blob, b"native-flow"))
            .expect("message tlv");
        vm.set_register(11, message_ptr);
        host.syscall(TEST_SYSCALL_ACTOR_SIGN, &mut vm)
            .expect("actor sign syscall");
        let signature_tlv = vm
            .validate_input_tlv(vm.register(10))
            .expect("signature tlv");
        assert_eq!(signature_tlv.type_id, PointerType::Blob);
        let signature =
            Ed25519Signature::from_slice(signature_tlv.payload).expect("signature bytes");
        signing_key
            .verifying_key()
            .verify(b"native-flow", &signature)
            .expect("signature verifies");

        put_blob(&mut vm, 10, "issuer");
        put_blob(&mut vm, 11, "hajimari");
        put_json(&mut vm, 12, "{}");
        vm.set_register(13, 0);
        host.syscall(TEST_SYSCALL_INVOKE_ENTRYPOINT_AS, &mut vm)
            .expect("invoke hajimari");

        put_blob(&mut vm, 10, "issuer");
        put_blob(&mut vm, 11, "increment");
        put_json(&mut vm, 12, "{}");
        vm.set_register(13, 0);
        host.syscall(TEST_SYSCALL_INVOKE_ENTRYPOINT_AS, &mut vm)
            .expect("invoke increment");
        let counter_state = host.inner.wsv.sc_get("counter").expect("counter state");
        let counter_tlv =
            ivm::pointer_abi::validate_tlv_bytes(&counter_state).expect("counter state tlv");
        assert_eq!(counter_tlv.type_id, PointerType::NoritoBytes);
        let counter: i64 =
            norito::decode_from_bytes(counter_tlv.payload).expect("decode counter value");
        assert_eq!(counter, 5);

        put_blob(&mut vm, 10, "issuer");
        put_blob(&mut vm, 11, "remember_caller");
        put_json(&mut vm, 12, "{}");
        vm.set_register(13, 0);
        host.syscall(TEST_SYSCALL_INVOKE_ENTRYPOINT_AS, &mut vm)
            .expect("invoke remember_caller");
        let remembered_state = host
            .inner
            .wsv
            .sc_get("last_actor")
            .expect("last_actor state");
        let remembered_tlv =
            ivm::pointer_abi::validate_tlv_bytes(&remembered_state).expect("remembered state tlv");
        assert_eq!(remembered_tlv.type_id, PointerType::NoritoBytes);
        let remembered_account_tlv = ivm::pointer_abi::validate_tlv_bytes(remembered_tlv.payload)
            .expect("remembered account tlv");
        assert_eq!(remembered_account_tlv.type_id, PointerType::AccountId);
        let remembered: AccountId = norito::decode_from_bytes(remembered_account_tlv.payload)
            .expect("decode remembered account");
        assert_eq!(
            remembered
                .canonical_i105()
                .expect("canonical remembered account"),
            actor_account
        );

        put_blob(&mut vm, 10, "issuer");
        put_blob(&mut vm, 11, "reject_me");
        put_json(&mut vm, 12, "{}");
        vm.set_register(13, 0);
        host.syscall(TEST_SYSCALL_EXPECT_REJECT_AS, &mut vm)
            .expect("expect reject");

        assert!(
            !host.supplemental_trace_pcs().is_empty(),
            "expected coverage trace from nested entrypoint execution"
        );
    }

    #[test]
    fn execute_suite_runs_compiled_contract_flow_helpers_from_standalone_test() {
        let temp = TestTempDir::new();
        let actor_seed = [9_u8; 32];
        let signing_key = SigningKey::from_bytes(&actor_seed);
        let actor_public_key = iroha_crypto::PublicKey::from_bytes(
            iroha_crypto::Algorithm::Ed25519,
            signing_key.verifying_key().as_bytes(),
        )
        .expect("public key");
        let actor_account = AccountId::new(actor_public_key)
            .canonical_i105()
            .expect("canonical actor account");

        temp.write(
            "contracts/contract_flow_demo.ko",
            r#"
            seiyaku Demo {
                state int counter;
                state AccountId last_actor;

                hajimari() {
                    counter = 1;
                }

                #[access(read="*", write="*")]
                kotoage fn increment() {
                    counter = counter + 4;
                }

                #[access(read="*", write="*")]
                kotoage fn remember_caller() {
                    last_actor = authority();
                }

                #[access(read="*", write="*")]
                kotoage fn reject_me() {
                    assert_eq(1, 2);
                }
            }
            "#,
        );
        let test_path = temp.write(
            "contracts/contract_flow_demo.test.ko",
            &format!(
                r#"
                koto_test {{ target: "contract_flow_demo.ko" }}

                fixture actors {{
                    actor("issuer", account_id("{actor_account}"), "0x{seed_hex}");
                }}

                #[test(fixture="actors")]
                fn actor_helpers_roundtrip() {{
                    let acct = actor_account("issuer");
                    assert(acct == account_id("{actor_account}"));

                    let pk = actor_public_key("issuer");
                    let sig = actor_sign("issuer", b"native-flow");
                    assert(tlv_len(pk) > 0);
                    assert(tlv_len(sig) > 0);
                }}

                #[test(fixture="actors")]
                fn invoke_entrypoint_as_runs_the_contract() {{
                    invoke_entrypoint_as("issuer", "hajimari", json("{{}}"));
                    invoke_entrypoint_as("issuer", "increment", json("{{}}"));
                    assert(counter == 5);

                    invoke_entrypoint_as("issuer", "remember_caller", json("{{}}"));
                    assert(last_actor == account_id("{actor_account}"));
                }}

                #[test(fixture="actors")]
                fn expect_reject_as_captures_contract_rejection() {{
                    expect_reject_as("issuer", "reject_me", json("{{}}"));
                }}
                "#,
                actor_account = actor_account,
                seed_hex = hex::encode(actor_seed),
            ),
        );

        let suite = discover_suite(&test_path).expect("discover suite");
        let compiled = compile_suite(&suite).expect("compile suite");
        let results = execute_suite(&compiled, TraceMode::PcOnly).expect("execute suite");
        let failures = results
            .iter()
            .filter(|result| !result.passed)
            .map(|result| {
                format!(
                    "{}: {}",
                    result.name,
                    result.failure.as_deref().unwrap_or("missing failure")
                )
            })
            .collect::<Vec<_>>();
        assert!(
            failures.is_empty(),
            "compiled standalone tests should pass: {}",
            failures.join("; ")
        );
        assert!(
            results
                .iter()
                .any(|result| !result.trace_pcs.is_empty() || !result.delta_trace.is_empty()),
            "expected compiled helpers to emit execution traces"
        );
    }

    #[test]
    fn validate_standalone_test_program_rejects_public_functions() {
        let temp = TestTempDir::new();
        let target = fs::canonicalize(temp.write("demo.ko", "fn helper() {}")).expect("target");
        let test_file = temp.write(
            "demo.test.ko",
            r#"
            koto_test { target: "demo.ko" }

            kotoage fn not_local() {}
            "#,
        );
        let program = parse_program_file(&test_file).expect("parse standalone test");
        let err = validate_standalone_test_program(&test_file, &target, &program)
            .expect_err("public function should fail validation");
        assert!(err.contains("non-local function"));
    }

    #[test]
    fn finalize_suite_rejects_program_without_tests() {
        let program = Program {
            items: vec![Item::Function(ivm::kotodama::ast::Function {
                name: "helper".to_string(),
                params: Vec::new(),
                ret_ty: None,
                body: ivm::kotodama::ast::Block {
                    statements: Vec::new(),
                },
                modifiers: Default::default(),
                location: ivm::kotodama::ast::SourceLocation { line: 1, column: 1 },
            })],
            contract_meta: None,
            test_target: None,
            fixtures: Vec::new(),
        };
        let err = finalize_suite(PathBuf::from("/tmp/demo.ko"), program.clone(), program)
            .err()
            .expect("program without tests should fail");
        assert!(err.contains("no #[test] Kotodama functions"));
    }

    #[test]
    fn compile_suite_excludes_test_functions_from_coverage() {
        let program = parser::parse(
            r#"
            seiyaku Demo {
                #[access(read="*", write="*")]
                kotoage fn run(count: int) -> int { return count + 1; }

                #[test]
                fn smoke() {
                    let next = invoke_entrypoint("run", json("{\"count\": 7}"));
                    assert_eq(next, 8);
                }
            }
            "#,
        )
        .expect("parse program");
        let suite = DiscoveredSuite {
            target_path: PathBuf::from("/tmp/demo.ko"),
            target_program: program.clone(),
            merged_program: program,
            tests: vec![TestCase {
                name: "smoke".to_string(),
                fixture: None,
                line: 6,
            }],
            fixtures: HashMap::new(),
        };

        let compiled = compile_suite(&suite).expect("compile suite");
        assert_eq!(compiled.tests.len(), 1);
        let names = compiled
            .coverage_functions
            .iter()
            .map(|function| function.display_name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["run"]);
    }

    #[test]
    fn build_fixture_map_rejects_duplicate_names() {
        let fixtures = vec![
            FixtureDecl {
                name: "seeded".to_string(),
                actions: Vec::new(),
            },
            FixtureDecl {
                name: "seeded".to_string(),
                actions: Vec::new(),
            },
        ];
        let err = build_fixture_map(&fixtures).expect_err("duplicate fixtures should fail");
        assert!(err.contains("duplicate fixture"));
    }

    #[test]
    fn apply_fixture_action_rejects_unknown_action() {
        let caller = parse_account_literal(DEFAULT_CALLER).expect("caller");
        let mut host = KotoTestHost::new(
            WsvHost::new_with_subject(MockWorldStateView::default(), caller, HashMap::new()),
            Vec::new(),
            HashMap::new(),
        );
        let mut public_inputs = BTreeMap::new();
        let err = apply_fixture_action(
            &FixtureAction {
                name: "wat".to_string(),
                args: Vec::new(),
            },
            &mut host,
            &mut public_inputs,
        )
        .expect_err("unknown fixture action should fail");
        assert!(err.contains("unknown fixture action"));
    }

    #[test]
    fn apply_fixture_action_populates_state_and_public_inputs() {
        let caller = parse_account_literal(DEFAULT_CALLER).expect("caller");
        let mut host = KotoTestHost::new(
            WsvHost::new_with_subject(MockWorldStateView::default(), caller, HashMap::new()),
            Vec::new(),
            HashMap::new(),
        );
        let mut public_inputs = BTreeMap::new();

        apply_fixture_action(
            &FixtureAction {
                name: "state_set".to_string(),
                args: vec![Expr::String("demo/counter".to_string()), Expr::Number(7)],
            },
            &mut host,
            &mut public_inputs,
        )
        .expect("apply state_set");
        apply_fixture_action(
            &FixtureAction {
                name: "public_input".to_string(),
                args: vec![
                    Expr::Call {
                        name: "name".to_string(),
                        args: vec![Expr::String("trigger_event_json".to_string())],
                    },
                    Expr::Call {
                        name: "json".to_string(),
                        args: vec![Expr::String("{\"count\":7}".to_string())],
                    },
                ],
            },
            &mut host,
            &mut public_inputs,
        )
        .expect("apply public_input");

        assert_eq!(
            host.inner.wsv.sc_get("demo/counter").expect("seeded state"),
            make_norito_envelope(&7i64).expect("encoded value"),
        );
        let trigger_name: Name = "trigger_event_json".parse().expect("name");
        let trigger_payload = public_inputs
            .get(&trigger_name)
            .expect("trigger payload present");
        let tlv = ivm::pointer_abi::validate_tlv_bytes(trigger_payload).expect("valid tlv");
        assert_eq!(tlv.type_id, PointerType::Json);
    }

    #[test]
    fn build_host_for_fixture_rejects_unknown_fixture() {
        let compiled = compiled_suite_with_fixtures(Vec::new());
        let err = build_host_for_fixture(&compiled, Some("missing"))
            .err()
            .expect("unknown fixture should fail");
        assert!(err.contains("unknown fixture"));
    }

    #[test]
    fn build_host_for_fixture_applies_bound_caller() {
        let fixture = FixtureDecl {
            name: "seeded".to_string(),
            actions: vec![
                FixtureAction {
                    name: "caller".to_string(),
                    args: vec![Expr::String(DEFAULT_CALLER.to_string())],
                },
                FixtureAction {
                    name: "state_set".to_string(),
                    args: vec![
                        Expr::String("demo/value".to_string()),
                        Expr::String("hello".to_string()),
                    ],
                },
            ],
        };
        let compiled = compiled_suite_with_fixtures(vec![fixture]);
        let host = build_host_for_fixture(&compiled, Some("seeded")).expect("build host");
        assert_eq!(
            host.caller_subject(),
            parse_account_literal(DEFAULT_CALLER).expect("caller")
        );
        assert_eq!(
            host.inner.wsv.sc_get("demo/value").expect("state value"),
            make_norito_envelope(&"hello").expect("encoded string"),
        );
    }

    #[test]
    fn apply_fixture_action_registers_actor_seed() {
        let caller = parse_account_literal(DEFAULT_CALLER).expect("caller");
        let mut host = KotoTestHost::new(
            WsvHost::new_with_subject(MockWorldStateView::default(), caller, HashMap::new()),
            Vec::new(),
            HashMap::new(),
        );
        let mut public_inputs = BTreeMap::new();
        let actor_seed = [7_u8; 32];
        let signing_key = SigningKey::from_bytes(&actor_seed);
        let actor_account = iroha_crypto::PublicKey::from_bytes(
            iroha_crypto::Algorithm::Ed25519,
            signing_key.verifying_key().as_bytes(),
        )
        .expect("public key")
        .to_string();
        apply_fixture_action(
            &FixtureAction {
                name: "actor".to_string(),
                args: vec![
                    Expr::String("seller".to_string()),
                    Expr::String(actor_account.clone()),
                    Expr::String(format!("0x{}", hex::encode(actor_seed))),
                ],
            },
            &mut host,
            &mut public_inputs,
        )
        .expect("register actor");

        assert!(public_inputs.is_empty());
        assert_eq!(
            host.actor_account("seller").expect("actor account"),
            parse_account_literal(&actor_account).expect("parsed actor account")
        );
        assert_eq!(
            host.actors["seller"].seed.expect("stored actor seed"),
            actor_seed
        );
    }

    #[test]
    fn helper_parsers_reject_invalid_numeric_and_mintability() {
        let err = eval_numeric_expr(&Expr::Number(-1)).expect_err("negative amount should fail");
        assert!(err.contains("negative balances are not allowed"));

        let err = eval_mintable_expr(&Expr::String("sometimes".to_string()))
            .expect_err("invalid mintability should fail");
        assert!(err.contains("unsupported mintability"));

        let err = expect_arg_count(
            &FixtureAction {
                name: "caller".to_string(),
                args: Vec::new(),
            },
            1,
        )
        .expect_err("wrong arg count should fail");
        assert!(err.contains("expects 1 arguments"));
    }

    #[test]
    fn parse_permission_helpers_cover_targeted_and_json_forms() {
        let domain = DomainId::try_new("wonderland", "universal").expect("domain");
        let asset = AssetDefinitionId::new(domain, "rose".parse().expect("name"));
        let token = parse_permission_token_name(&format!("mint_asset:{asset}"))
            .expect("parse mint asset token");
        assert!(matches!(token, PermissionToken::MintAsset(id) if id == asset));

        let token = parse_permission_token_json(r#"{"type":"custom","name":"demo.permission"}"#)
            .expect("parse custom permission json");
        assert!(matches!(token, PermissionToken::Custom(name) if name == "demo.permission"));

        let err = parse_permission_token_json(r#"{"target":"missing-type"}"#)
            .expect_err("missing type should fail");
        assert!(err.contains("missing `type`"));
    }

    #[test]
    fn permission_and_json_helpers_reject_invalid_inputs() {
        let err = parse_permission_token_name("mint_asset:not-an-asset")
            .expect_err("invalid targeted permission should fail");
        assert!(err.contains("invalid asset definition id"));

        let err = eval_json_payload(&[Expr::Number(7)]).expect_err("non-string json should fail");
        assert!(err.contains("expects a string payload"));
    }

    #[test]
    fn eval_envelope_expr_encodes_pointer_variants() {
        let account_ptr = eval_envelope_expr(&Expr::Call {
            name: "account_id".to_string(),
            args: vec![Expr::String(DEFAULT_CALLER.to_string())],
        })
        .expect("account envelope");
        let account_tlv = ivm::pointer_abi::validate_tlv_bytes(&account_ptr).expect("account tlv");
        assert_eq!(account_tlv.type_id, PointerType::AccountId);

        let name_ptr = eval_envelope_expr(&Expr::Call {
            name: "name".to_string(),
            args: vec![Expr::String("cursor".to_string())],
        })
        .expect("name envelope");
        let name_tlv = ivm::pointer_abi::validate_tlv_bytes(&name_ptr).expect("name tlv");
        assert_eq!(name_tlv.type_id, PointerType::Name);
    }

    #[test]
    fn render_failure_without_diagnostic_falls_back_to_debug_error() {
        let vm = IVM::new(u64::MAX);
        let rendered = render_failure(&vm, None, &ivm::VMError::DecodeError);
        assert!(rendered.contains("DecodeError"));
    }

    #[test]
    fn coverage_helper_functions_handle_internal_and_boundary_cases() {
        assert_eq!(
            normalize_user_function_name("__entrypoint_impl__run"),
            Some("run")
        );
        assert_eq!(normalize_user_function_name("__lowered_internal"), None);
        assert_eq!(normalize_user_function_name("run"), Some("run"));

        let function = CoverageFunction {
            display_name: "run".to_string(),
            line: 3,
            pc_start: 10,
            pc_end: 20,
        };
        let executed = HashSet::from([9_u64, 10, 19, 20]);
        assert!(function_hit(&function, &executed));
        assert_eq!(percentage(0, 0), 100.0);
        assert_eq!(percentage(1, 4), 25.0);
    }

    #[test]
    fn collect_tests_rejects_duplicate_test_names() {
        let program = Program {
            items: vec![
                test_function("smoke", None),
                test_function("smoke", Some("seeded")),
            ],
            contract_meta: None,
            test_target: None,
            fixtures: Vec::new(),
        };
        let err = collect_tests(&program).expect_err("duplicate test names should fail");
        assert!(err.contains("duplicate test function"));
    }
}
