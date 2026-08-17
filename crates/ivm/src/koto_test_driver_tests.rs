// Test body included from the parent module to keep its production source budget bounded.
use super::*;
use iroha_primitives::numeric_abi::IntValueV1;
use std::{
    sync::atomic::{AtomicUsize, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};
static TEMP_DIR_COUNTER: AtomicUsize = AtomicUsize::new(0);
fn decode_i64_word(vm: &IVM, pointer: u64) -> i64 {
    let tlv = vm.validate_tlv(pointer).expect("validate returned int TLV");
    assert_eq!(tlv.type_id, PointerType::Int);
    IntValueV1::decode_frame(tlv.payload)
        .expect("decode returned int frame")
        .into_int()
        .try_to_i64()
        .expect("test result fits i64")
}
fn decode_pointer_state_value(payload: &[u8], kind: StateValueKindV1) -> Vec<u8> {
    let schema = StateValueSchemaV1 {
        nodes: vec![StateValueNodeV1::Leaf(kind)],
    };
    let schema_bytes = norito::encode_canonical(&schema).expect("encode canonical state schema");
    let record: StateValueRecordV1 =
        norito::decode_canonical(payload).expect("decode canonical state record");
    assert_eq!(
        record.schema_hash,
        state_value_schema_hash_v1(&schema_bytes)
    );
    assert!(schema.validate_atoms(&record.atoms));
    let [StateValueAtomV1::Pointer(envelope)] = record.atoms.as_slice() else {
        panic!("state record must contain one pointer atom");
    };
    envelope.clone()
}
fn decode_int_state_value(payload: &[u8]) -> i64 {
    let envelope = decode_pointer_state_value(payload, StateValueKindV1::Int);
    crate::numeric_tlv::decode_int_bytes(&envelope)
        .expect("decode canonical state int")
        .try_to_i64()
        .expect("test state int fits i64")
}
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
    Item::Function(crate::kotodama::ast::Function {
        name: name.to_string(),
        params: Vec::new(),
        ret_ty: None,
        body: crate::kotodama::ast::Block {
            statements: Vec::new(),
            tail: None,
        },
        modifiers: crate::kotodama::ast::FunctionModifiers {
            is_test: true,
            test_fixture: fixture.map(str::to_string),
            ..Default::default()
        },
        location: crate::kotodama::ast::SourceLocation { line: 1, column: 1 },
    })
}
fn compiled_suite_with_fixtures(fixtures: Vec<FixtureDecl>) -> CompiledSuite {
    let target_source = "seiyaku FixtureDemo { fn helper() {} #[test] fn smoke() {} }";
    let target_program = parser::parse(target_source).expect("parse fixture test target");
    let suite = DiscoveredSuite {
        target_path: PathBuf::from("/tmp/fixture_demo.ko"),
        target_source: target_source.to_owned(),
        target_program,
        test_modules: Vec::new(),
        tests: vec![TestCase {
            name: "smoke".to_string(),
            fixture: None,
            line: 1,
        }],
        fixtures: build_fixture_map(&fixtures).expect("build fixture map"),
    };
    compile_suite(&suite, false).expect("compile fixture suite")
}
#[test]
fn pure_unit_test_suite_executes_without_runtime_artifact() {
    let compiled = compiled_suite_with_fixtures(Vec::new());
    assert!(compiled.runtime.is_none());
    assert!(compiled.runtime_entrypoints.is_empty());
    let results = execute_suite(&compiled, TraceMode::Off, 1)
        .expect("execute a suite containing only private helpers and tests");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "smoke");
    assert!(
        results[0].passed,
        "unexpected failure: {:?}",
        results[0].failure
    );
}
#[test]
fn helper_preserves_u64_max_json_int_through_option_match() {
    let temp = TestTempDir::new();
    let target = temp.write(
        "u64_max_option_match.ko",
        include_str!("../fixtures/koto_v1/koto_test_driver_tests/001.ko")
            .strip_suffix('\n')
            .expect("fixture sentinel newline"),
    );
    let suite = discover_suite(&target).expect("discover u64 max regression suite");
    let compiled = compile_suite(&suite, false).expect("compile u64 max regression suite");
    let results =
        execute_suite(&compiled, TraceMode::Off, 1).expect("execute u64 max regression suite");
    assert_eq!(results.len(), 1);
    assert!(
        results[0].passed,
        "unexpected failure: {:?}",
        results[0].failure
    );
}
#[test]
fn compiler_owned_test_return_sentinel_preserves_artifact_verification() {
    let compiled = compiled_suite_with_fixtures(Vec::new());
    let suite_program = compiled.suite.program.artifact();
    assert_eq!(
        compiled.suite.program.code_hash(),
        compiled.suite.report.artifact_hash
    );
    let return_pc = compiled
        .suite
        .program
        .entrypoint_pc(crate::metadata::KOTO_TEST_RETURN_ENTRYPOINT)
        .expect("compiler-owned suite return sentinel");
    let parsed = ProgramMetadata::parse(suite_program).expect("parse compiled suite");
    assert_eq!(
        return_pc,
        u64::try_from(suite_program.len() - parsed.header_len - 4)
            .expect("suite return PC fits u64")
    );
    let mut vm = IVM::new(u64::MAX);
    vm.load_koto_test_prepared(&compiled.suite.program)
        .expect("unmodified compiler-produced test artifact must load");
    let production_error = crate::prepare_contract(compiled.suite.program.shared_artifact())
        .expect_err("production admission must reject the generic IVM 1.0 test harness");
    assert!(
        production_error
            .to_string()
            .contains("expected IVM 1.1 contract artifact"),
        "unexpected production-admission failure: {production_error}"
    );
    let mut post_compile_mutation = suite_program.to_vec();
    post_compile_mutation.extend_from_slice(&crate::encoding::wide::encode_halt().to_le_bytes());
    let post_compile_mutation: Arc<[u8]> = Arc::from(post_compile_mutation);
    let error = crate::contract_artifact::prepare_koto_test_contract(
        Arc::clone(&post_compile_mutation),
        compiled.suite.program.contract_interface().clone(),
    )
    .expect_err("the compiler-owned sidecar must reject post-compile executable mutation");
    assert!(
        error.to_string().contains("must select the terminal HALT"),
        "unexpected mutation failure: {error}"
    );
    let mut mutated_interface = compiled.suite.program.contract_interface().clone();
    let terminal_return = mutated_interface
        .entrypoints
        .iter_mut()
        .find(|entrypoint| entrypoint.name == ivm_abi::metadata::KOTO_TEST_RETURN_ENTRYPOINT)
        .expect("compiled suite exposes its compiler-owned return entrypoint");
    terminal_return.entry_pc = terminal_return
        .entry_pc
        .checked_add(
            u64::try_from(core::mem::size_of::<u32>()).expect("IVM instruction width fits u64"),
        )
        .expect("test return PC remains representable");
    let mutated = crate::contract_artifact::prepare_koto_test_contract(
        post_compile_mutation,
        mutated_interface,
    )
    .expect("a structurally valid generic harness can still be prepared");
    assert_ne!(
        mutated.code_hash(),
        compiled.suite.report.artifact_hash,
        "the compiler report hash must detect every post-compile executable mutation"
    );
}
#[test]
fn parse_args_accepts_supported_subcommands() {
    let options = parse_args(vec![
        "coverage".to_string(),
        "contracts/demo.ko".to_string(),
        "--filter".to_string(),
        "smoke".to_string(),
        "--jobs".to_string(),
        "2".to_string(),
        "--chain-discriminant".to_string(),
        "369".to_string(),
        "--zk".to_string(),
    ])
    .expect("parse args");
    assert_eq!(options.command, Command::Coverage);
    assert_eq!(options.path, PathBuf::from("contracts/demo.ko"));
    assert_eq!(options.filter.as_deref(), Some("smoke"));
    assert_eq!(options.jobs, 2);
    assert_eq!(options.chain_discriminant, 369);
    assert!(options.zk_enabled);
}
#[test]
fn parse_args_rejects_invalid_or_duplicate_chain_discriminants() {
    for invalid in ["", "0", "0369", "+369", "-1", "369x", "65536"] {
        assert!(
            parse_chain_discriminant(invalid).is_err(),
            "accepted invalid discriminant {invalid:?}"
        );
    }
    let duplicate = parse_args(vec![
        "run".to_owned(),
        "--chain-discriminant".to_owned(),
        "369".to_owned(),
        "--chain-discriminant".to_owned(),
        "753".to_owned(),
        "demo.ko".to_owned(),
    ])
    .expect_err("duplicate discriminants must fail closed");
    assert!(duplicate.contains("only once"));
}
#[test]
fn test_runner_uses_exact_taira_chain_discriminant() {
    const TAIRA_RECIPIENT: &str =
        "testﾜヰ8ｽuimdh9FﾂｦUｸﾈbﾕﾆヱMUYｴGｷﾙｹﾐRヱbﾐｷwﾄ6ﾃdDLPQﾋW496uﾙﾜFpﾈtHd4Hﾙﾎ45M1L5";
    let temp = TestTempDir::new();
    let target = temp.write(
        "taira_literal.ko",
        &format!(
            r#"
                seiyaku TairaLiteral {{
                    fn recipient() -> AccountId {{
                        return AccountId::parse("{TAIRA_RECIPIENT}");
                    }}

                    #[test]
                    fn exact_network_literal_roundtrips() {{
                        test::assert(
                            recipient() == AccountId::parse("{TAIRA_RECIPIENT}")
                        );
                    }}
                }}
                "#
        ),
    );
    let suite = discover_suite(&target).expect("discover Taira literal suite");
    let compiled = compile_suite_for_chain(&suite, false, 369)
        .expect("compile exact Taira literal under discriminant 369");
    let results = execute_suite_for_chain(&compiled, TraceMode::Off, 2, 369)
        .expect("execute exact Taira literal suite");
    assert!(results.iter().all(|result| result.passed));
    let mismatch = match compile_suite_for_chain(&suite, false, 753) {
        Ok(_) => panic!("Taira literal must fail under Sora discriminant 753"),
        Err(error) => error,
    };
    assert!(mismatch.contains("ERR_UNEXPECTED_NETWORK_PREFIX"));
}
#[test]
fn zk_test_option_marks_both_test_and_runtime_artifacts() {
    let target_source = "seiyaku ZkTest { hajimari() {} #[test] fn smoke() {} }";
    let target_program = parser::parse(target_source).expect("parse ZK test target");
    let suite = DiscoveredSuite {
        target_path: PathBuf::from("/tmp/zk_test.ko"),
        target_source: target_source.to_owned(),
        target_program,
        test_modules: Vec::new(),
        tests: vec![TestCase {
            name: "smoke".to_owned(),
            fixture: None,
            line: 1,
        }],
        fixtures: HashMap::new(),
    };
    let compiled = compile_suite(&suite, true).expect("compile ZK test suite");
    for artifact in [
        compiled.suite.program.artifact(),
        compiled
            .runtime
            .as_ref()
            .expect("lifecycle target has a runtime artifact")
            .program
            .artifact(),
    ] {
        let metadata = ProgramMetadata::parse(artifact).expect("parse compiled metadata");
        assert_ne!(metadata.metadata.mode & crate::metadata::mode::ZK, 0);
    }
}
#[test]
fn parse_args_rejects_extra_argument_and_missing_path() {
    let err = parse_args(vec!["wat".to_string(), "demo.ko".to_string()])
        .expect_err("extra path should fail");
    assert!(err.contains("unexpected test argument"));
    let err = parse_args(vec!["run".to_string()]).expect_err("missing path should fail");
    assert!(err.contains("usage: koto test"));
}
#[test]
fn filtering_exact_and_seeded_order_are_deterministic() {
    let mut tests = vec![
        TestCase {
            name: "beta".to_owned(),
            fixture: None,
            line: 2,
        },
        TestCase {
            name: "alpha".to_owned(),
            fixture: None,
            line: 1,
        },
        TestCase {
            name: "alphabet".to_owned(),
            fixture: None,
            line: 3,
        },
    ];
    let options = TestOptions {
        command: Command::Run,
        path: PathBuf::from("demo.ko"),
        filter: Some("alpha".to_owned()),
        exact: false,
        jobs: 1,
        seed: 7,
        chain_discriminant: iroha_data_model::account::address::chain_discriminant(),
        zk_enabled: false,
        output: TestOutputFormat::Text,
        output_path: None,
    };
    filter_and_order_tests(&mut tests, &options);
    let first = tests
        .iter()
        .map(|test| test.name.clone())
        .collect::<Vec<_>>();
    let mut repeated = vec![
        TestCase {
            name: "beta".to_owned(),
            fixture: None,
            line: 2,
        },
        TestCase {
            name: "alpha".to_owned(),
            fixture: None,
            line: 1,
        },
        TestCase {
            name: "alphabet".to_owned(),
            fixture: None,
            line: 3,
        },
    ];
    filter_and_order_tests(&mut repeated, &options);
    assert_eq!(
        first,
        repeated
            .iter()
            .map(|test| test.name.clone())
            .collect::<Vec<_>>()
    );
    assert_eq!(first.len(), 2);
}
#[test]
fn structured_request_validation_is_stage_tagged() {
    let mut request = KotoTestRunRequestV1::new("demo.ko", 753);
    request.jobs = 0;
    let error = validate_structured_request(&request).expect_err("zero workers must fail");
    assert_eq!(error.phase, KotoTestRunPhaseV1::Request);
    assert!(error.message.contains("worker count"));
    request.jobs = 1;
    request.exact = true;
    let error = validate_structured_request(&request).expect_err("exact needs a filter");
    assert_eq!(error.phase, KotoTestRunPhaseV1::Request);
    assert!(error.message.contains("requires a filter"));
    request.exact = false;
    request.chain_discriminant = 0;
    let error = validate_structured_request(&request).expect_err("zero chain must fail");
    assert_eq!(error.phase, KotoTestRunPhaseV1::Request);
    assert!(error.message.contains("1..=65535"));
}
#[test]
fn structured_filter_order_is_independent_of_discovery_order() {
    let request = KotoTestRunRequestV1 {
        target: PathBuf::from("demo.ko"),
        filter: Some("case".to_owned()),
        exact: false,
        jobs: 1,
        seed: 17,
        chain_discriminant: 753,
        zk_enabled: false,
    };
    let case = |name: &str, line| TestCase {
        name: name.to_owned(),
        fixture: None,
        line,
    };
    let mut forward = vec![case("case_z", 3), case("ignored", 2), case("case_a", 1)];
    let mut reverse = forward.iter().cloned().rev().collect::<Vec<_>>();
    filter_and_order_structured_tests(&mut forward, &request);
    filter_and_order_structured_tests(&mut reverse, &request);
    assert_eq!(
        forward
            .iter()
            .map(|test| test.name.as_str())
            .collect::<Vec<_>>(),
        reverse
            .iter()
            .map(|test| test.name.as_str())
            .collect::<Vec<_>>()
    );
    assert_eq!(forward.len(), 2);
}
#[test]
fn structured_runner_returns_ordered_logical_outcomes_without_timing() {
    let temp = TestTempDir::new();
    let target = temp.write(
        "structured.ko",
        include_str!("../fixtures/koto_v1/koto_test_driver_tests/002.ko")
            .strip_suffix('\n')
            .expect("fixture sentinel newline"),
    );
    let mut request = KotoTestRunRequestV1::new(&target, 753);
    request.jobs = 2;
    let first = run_tests_structured_v1(&request).expect("run structured suite");
    let repeated = run_tests_structured_v1(&request).expect("repeat structured suite");
    assert_eq!(first, repeated);
    assert_eq!(
        first.target,
        fs::canonicalize(target).expect("canonical target")
    );
    assert_eq!(
        first
            .cases
            .iter()
            .map(|case| case.name.as_str())
            .collect::<Vec<_>>(),
        ["a_fails", "z_passes"]
    );
    assert_eq!(first.passed(), 1);
    assert_eq!(first.failed(), 1);
    assert!(!first.is_success());
    assert!(first.cases[0].failure.is_some());
    assert!(first.cases[1].failure.is_none());
}
#[test]
fn structured_module_graph_executes_exact_dependency_and_ignores_ambient_tests() {
    let temp = TestTempDir::new();
    let target = temp.write(
        "tests/unit.ko",
        include_str!("../fixtures/koto_v1/koto_test_driver_tests/003.ko")
            .strip_suffix('\n')
            .expect("fixture sentinel newline"),
    );
    temp.write(
        "tests/ambient.test.ko",
        include_str!("../fixtures/koto_v1/koto_test_driver_tests/004.ko")
            .strip_suffix('\n')
            .expect("fixture sentinel newline"),
    );
    let dependency = "std/math@1.0.0".to_owned();
    let modules = KotoTestModuleGraphV1 {
        imports: vec![ImportBinding {
            alias: "calc".to_owned(),
            package: dependency.clone(),
        }],
        packages: vec![SourcePackageUnit {
            identity: dependency,
            modules: vec![SourceModuleUnit {
                source_name: "src/lib.ko".to_owned(),
                source: "module Math { fn value() -> int { return 7; } }".to_owned(),
            }],
            exports: BTreeSet::from(["value".to_owned()]),
            imports: Vec::new(),
        }],
    };
    assert_eq!(
        discover_declared_test_names_v1(&target).expect("declared names"),
        ["dependency_is_exact"]
    );
    let report =
        run_tests_structured_with_modules_v1(&KotoTestRunRequestV1::new(&target, 753), &modules)
            .expect("run exact module graph");
    assert_eq!(report.cases.len(), 1);
    assert!(report.is_success());
}
#[test]
fn structured_source_root_is_bound_and_never_reopened_from_the_target_path() {
    let source_name = "tests/in-memory-supplied.ko".to_owned();
    let root = SourceModuleUnit {
        source_name: source_name.clone(),
        source: "seiyaku Supplied { #[test] fn supplied_only() { test::assert(true); } }"
            .to_owned(),
    };
    assert_eq!(
        discover_declared_test_names_source_v1(&root).expect("supplied names"),
        ["supplied_only"]
    );
    let report = run_tests_structured_source_with_modules_v1(
        &KotoTestRunRequestV1::new(&source_name, 753),
        &root,
        &KotoTestModuleGraphV1::default(),
    )
    .expect("run supplied source root");
    assert_eq!(report.target, PathBuf::from(source_name));
    assert_eq!(report.cases.len(), 1);
    assert_eq!(report.cases[0].name, "supplied_only");
    assert!(report.is_success());
    let error = run_tests_structured_source_with_modules_v1(
        &KotoTestRunRequestV1::new("tests/other.ko", 753),
        &root,
        &KotoTestModuleGraphV1::default(),
    )
    .expect_err("request/source substitution must fail");
    assert_eq!(error.phase, KotoTestRunPhaseV1::Request);
    assert!(error.message.contains("must equal"));
    let noncanonical = SourceModuleUnit {
        source_name: "tests/./in-memory-supplied.ko".to_owned(),
        source: root.source.clone(),
    };
    let error = run_tests_structured_source_with_modules_v1(
        &KotoTestRunRequestV1::new(&noncanonical.source_name, 753),
        &noncanonical,
        &KotoTestModuleGraphV1::default(),
    )
    .expect_err("noncanonical source identities must fail before compilation");
    assert_eq!(error.phase, KotoTestRunPhaseV1::Request);
    assert!(error.message.contains("canonical logical spelling"));
}
#[test]
fn machine_reports_preserve_failure_details() {
    let results = vec![TestRunResult {
        name: "rejects_bad_input".to_owned(),
        line: 9,
        elapsed: Duration::from_millis(2),
        passed: false,
        failure: Some("expected rejection".to_owned()),
        trace_pcs: Vec::new(),
        delta_trace: Vec::new(),
    }];
    let json = render_test_json(Path::new("demo.ko"), &results, 42).expect("JSON report");
    let junit = render_test_junit(Path::new("demo.ko"), &results, 42);
    for report in [&json, &junit] {
        assert!(report.contains("rejects_bad_input"));
        assert!(report.contains("expected rejection"));
    }
}
#[test]
fn discover_suite_links_inline_and_matching_standalone_tests() {
    let temp = TestTempDir::new();
    let target = temp.write(
        "contracts/demo.ko",
        include_str!("../fixtures/koto_v1/koto_test_driver_tests/005.ko")
            .strip_suffix('\n')
            .expect("fixture sentinel newline"),
    );
    temp.write(
        "contracts/demo.test.ko",
        include_str!("../fixtures/koto_v1/koto_test_driver_tests/006.ko")
            .strip_suffix('\n')
            .expect("fixture sentinel newline"),
    );
    temp.write("contracts/other.ko", "seiyaku Other { fn other() {} }");
    temp.write(
        "contracts/tests/ignored.test.ko",
        include_str!("../fixtures/koto_v1/koto_test_driver_tests/007.ko")
            .strip_suffix('\n')
            .expect("fixture sentinel newline"),
    );
    let suite = discover_suite(&target).expect("discover suite");
    let mut names = suite
        .tests
        .iter()
        .map(|test| test.name.clone())
        .collect::<Vec<_>>();
    names.sort();
    assert_eq!(names, vec!["inline".to_string(), "standalone".to_string()]);
    let mut public_names = discover_test_names(&target).expect("discover public test names");
    public_names.sort();
    assert_eq!(public_names, names);
}
#[test]
fn discover_suite_from_standalone_input_uses_target_program() {
    let temp = TestTempDir::new();
    temp.write(
        "contracts/demo.ko",
        include_str!("../fixtures/koto_v1/koto_test_driver_tests/008.ko")
            .strip_suffix('\n')
            .expect("fixture sentinel newline"),
    );
    let standalone = temp.write(
        "contracts/demo.test.ko",
        include_str!("../fixtures/koto_v1/koto_test_driver_tests/009.ko")
            .strip_suffix('\n')
            .expect("fixture sentinel newline"),
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
        include_str!("../fixtures/koto_v1/koto_test_driver_tests/010.ko")
            .strip_suffix('\n')
            .expect("fixture sentinel newline"),
    );
    let test_path = temp.write(
            "contracts/contract_flow_demo.test.ko",
            &format!(
                r#"
                module ContractFlowTests {{
                koto_test {{ target: "contract_flow_demo.ko" }}

                fixture actors {{
                    actor("issuer", AccountId::parse("{actor_account}"), "0x{seed_hex}");
                }}

                #[test(fixture="actors")]
                fn drive_contract_flow() {{
                    test::invoke_kotoage_as(actor: "issuer", kotoage: "hajimari", arguments: Json::parse("{{}}"));
                    test::invoke_kotoage_as(actor: "issuer", kotoage: "increment", arguments: Json::parse("{{}}"));
                    test::invoke_kotoage_as(
                        actor: "issuer",
                        kotoage: "remember_caller",
                        arguments: Json::parse("{{}}")
                    );

                    test::expect_reject_as(actor: "issuer", kotoage: "reject_me", arguments: Json::parse("{{}}"));
                }}
                }}
                "#,
                actor_account = actor_account,
                seed_hex = hex::encode(actor_seed),
            ),
        );
    let suite = discover_suite(&test_path).expect("discover suite");
    let compiled = compile_suite(&suite, false).expect("compile suite");
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
    let signature = Ed25519Signature::from_slice(signature_tlv.payload).expect("signature bytes");
    signing_key
        .verifying_key()
        .verify(b"native-flow", &signature)
        .expect("signature verifies");
    put_blob(&mut vm, 10, "issuer");
    put_blob(&mut vm, 11, "hajimari");
    put_json(&mut vm, 12, "{}");
    vm.set_register(13, 0);
    vm.set_register(14, 1);
    host.syscall(TEST_SYSCALL_INVOKE_ENTRYPOINT_AS, &mut vm)
        .expect("invoke hajimari");
    put_blob(&mut vm, 10, "issuer");
    put_blob(&mut vm, 11, "increment");
    put_json(&mut vm, 12, "{}");
    vm.set_register(13, 0);
    vm.set_register(14, 1);
    host.syscall(TEST_SYSCALL_INVOKE_ENTRYPOINT_AS, &mut vm)
        .expect("invoke increment");
    let counter_state = host.inner.wsv.sc_get("counter").expect("counter state");
    assert_eq!(decode_int_state_value(&counter_state), 5);
    put_blob(&mut vm, 10, "issuer");
    put_blob(&mut vm, 11, "remember_caller");
    put_json(&mut vm, 12, "{}");
    vm.set_register(13, 0);
    vm.set_register(14, 1);
    host.syscall(TEST_SYSCALL_INVOKE_ENTRYPOINT_AS, &mut vm)
        .expect("invoke remember_caller");
    let remembered_state = host
        .inner
        .wsv
        .sc_get("last_actor")
        .expect("last_actor state");
    let remembered_account_envelope =
        decode_pointer_state_value(&remembered_state, StateValueKindV1::AccountId);
    let remembered_account_tlv =
        crate::pointer_abi::validate_tlv_bytes(&remembered_account_envelope)
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
    put_blob(&mut vm, 11, "pair");
    put_json(&mut vm, 12, "{}");
    vm.set_register(13, 0b11);
    vm.set_register(14, 2);
    host.syscall(TEST_SYSCALL_INVOKE_ENTRYPOINT_AS, &mut vm)
        .expect("invoke pair");
    assert_eq!(decode_i64_word(&vm, vm.register(10)), 2);
    assert_eq!(decode_i64_word(&vm, vm.register(11)), 3);
    put_blob(&mut vm, 10, "issuer");
    put_blob(&mut vm, 11, "reject_me");
    put_json(&mut vm, 12, "{}");
    vm.set_register(13, 0);
    vm.set_register(14, 1);
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
        include_str!("../fixtures/koto_v1/koto_test_driver_tests/011.ko")
            .strip_suffix('\n')
            .expect("fixture sentinel newline"),
    );
    let test_path = temp.write(
            "contracts/contract_flow_demo.test.ko",
            &format!(
                r#"
                module ContractFlowTests {{
                koto_test {{ target: "contract_flow_demo.ko" }}

                fixture actors {{
                    actor("issuer", AccountId::parse("{actor_account}"), "0x{seed_hex}");
                }}

                #[test(fixture="actors")]
                fn actor_helpers_roundtrip() {{
                    let acct = test::actor_account("issuer");
                    test::assert(acct == AccountId::parse("{actor_account}"));

                    let pk = test::actor_public_key("issuer");
                    let sig = test::actor_sign("issuer", b"native-flow");
                    test::assert(pk != b"");
                    test::assert(sig != b"");
                }}

                #[test(fixture="actors")]
                fn invoke_kotoage_as_runs_the_seiyaku() {{
                    test::invoke_kotoage_as(actor: "issuer", kotoage: "hajimari", arguments: Json::parse("{{}}"));
                    test::invoke_kotoage_as(actor: "issuer", kotoage: "increment", arguments: Json::parse("{{}}"));
                    test::assert(counter == 5);

                    test::invoke_kotoage_as(actor: "issuer", kotoage: "remember_caller", arguments: Json::parse("{{}}"));
                    test::assert(last_actor == AccountId::parse("{actor_account}"));

                    let pair_result = test::invoke_kotoage_as(actor: "issuer", kotoage: "pair", arguments: Json::parse("{{}}"));
                    test::assert_eq(actual: pair_result.0, expected: 2);
                    test::assert_eq(actual: pair_result.1, expected: 3);
                }}

                #[test(fixture="actors")]
                fn expect_reject_as_captures_seiyaku_rejection() {{
                    test::expect_reject_as(actor: "issuer", kotoage: "reject_me", arguments: Json::parse("{{}}"));
                }}

                #[test(fixture="actors")]
                fn expect_reject_as_captures_argument_schema_rejection() {{
                    test::invoke_kotoage_as(actor: "issuer", kotoage: "hajimari", arguments: Json::parse("{{}}"));
                    test::expect_reject_as(actor: "issuer", kotoage: "set_counter", arguments: Json::parse("{{\"value\":\"not-an-int\"}}"));
                    test::expect_reject_as(actor: "issuer", kotoage: "set_counter", arguments: Json::parse("{{}}"));
                    test::expect_reject_as(actor: "issuer", kotoage: "set_counter", arguments: Json::parse("{{\"value\":7,\"unexpected\":true}}"));
                    test::assert(counter == 1);
                }}
                }}
                "#,
                actor_account = actor_account,
                seed_hex = hex::encode(actor_seed),
            ),
        );
    let suite = discover_suite(&test_path).expect("discover suite");
    let compiled = compile_suite(&suite, false).expect("compile suite");
    let runtime = compiled
        .runtime
        .as_ref()
        .expect("standalone contract tests require a runtime artifact");
    assert_eq!(
        compiled.suite.report.artifact_hash,
        crate::metadata::contract_code_hash(compiled.suite.program.artifact()),
        "the standalone test artifact must retain its own hash when a separate runtime artifact is present"
    );
    assert_eq!(
        runtime.report.artifact_hash,
        crate::metadata::contract_code_hash(runtime.program.artifact()),
        "the runtime report must remain bound to the deployable runtime artifact"
    );
    assert_ne!(
        compiled.suite.report.artifact_hash, runtime.report.artifact_hash,
        "test-suite and runtime projections must retain distinct artifact identities"
    );
    let suite_metadata = ProgramMetadata::parse(compiled.suite.program.artifact())
        .expect("parse generic test-suite metadata");
    assert_eq!(
        (
            suite_metadata.metadata.version_major,
            suite_metadata.metadata.version_minor,
        ),
        (1, 0),
        "the compiler-owned test suite must remain a generic IVM 1.0 image"
    );
    let runtime_metadata = ProgramMetadata::parse(runtime.program.artifact())
        .expect("parse deployable runtime metadata");
    assert_eq!(
        (
            runtime_metadata.metadata.version_major,
            runtime_metadata.metadata.version_minor,
        ),
        (1, 1),
        "nested contract calls must use a separately compiled deployable artifact"
    );
    crate::prepare_contract(runtime.program.shared_artifact())
        .expect("the nested runtime artifact must satisfy production admission");
    let production_error = crate::prepare_contract(compiled.suite.program.shared_artifact())
        .expect_err("production admission must reject the generic IVM 1.0 test harness");
    assert!(
        production_error
            .to_string()
            .contains("expected IVM 1.1 contract artifact"),
        "unexpected production-admission failure: {production_error}"
    );
    let results = execute_suite(&compiled, TraceMode::PcOnly, 2).expect("execute suite");
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
fn standalone_test_source_parser_rejects_public_functions() {
    let temp = TestTempDir::new();
    temp.write("demo.ko", "seiyaku Demo { fn helper() {} }");
    let test_file = temp.write(
        "demo.test.ko",
        include_str!("../fixtures/koto_v1/koto_test_driver_tests/012.ko")
            .strip_suffix('\n')
            .expect("fixture sentinel newline"),
    );
    let error = parse_program_file(&test_file)
        .expect_err("a module cannot contain a public seiyaku function");
    assert!(error.contains("module"), "unexpected error: {error}");
}
#[test]
fn finalize_suite_rejects_program_without_tests() {
    let program = Program {
        unit: crate::kotodama::ast::SourceUnit {
            kind: crate::kotodama::ast::SourceUnitKind::Module,
            name: "EmptyTests".to_string(),
        },
        items: vec![Item::Function(crate::kotodama::ast::Function {
            name: "helper".to_string(),
            params: Vec::new(),
            ret_ty: None,
            body: crate::kotodama::ast::Block {
                statements: Vec::new(),
                tail: None,
            },
            modifiers: Default::default(),
            location: crate::kotodama::ast::SourceLocation { line: 1, column: 1 },
        })],
        test_target: None,
        fixtures: Vec::new(),
    };
    let err = finalize_suite(
        PathBuf::from("/tmp/demo.ko"),
        "module EmptyTests { fn helper() {} }".to_owned(),
        program,
        Vec::new(),
    )
    .err()
    .expect("program without tests should fail");
    assert!(err.contains("no #[test] Kotodama functions"));
}
#[test]
fn contract_backed_suite_preserves_runtime_coverage_and_suite_hash() {
    let source = include_str!("../fixtures/koto_v1/koto_test_driver_tests/013.ko")
        .strip_suffix('\n')
        .expect("fixture sentinel newline");
    let program = parser::parse(source).expect("parse program");
    let suite = DiscoveredSuite {
        target_path: PathBuf::from("/tmp/demo.ko"),
        target_source: source.to_owned(),
        target_program: program,
        test_modules: Vec::new(),
        tests: vec![TestCase {
            name: "smoke".to_string(),
            fixture: None,
            line: 6,
        }],
        fixtures: HashMap::new(),
    };
    let compiled = compile_suite(&suite, false).expect("compile suite");
    assert_eq!(compiled.tests.len(), 1);
    let runtime = compiled
        .runtime
        .as_ref()
        .expect("contract-backed suite runtime artifact");
    assert_ne!(
        compiled.suite.report.artifact_hash, runtime.report.artifact_hash,
        "the test-suite and deployable runtime artifacts must retain distinct identities"
    );
    compiled
        .suite
        .program
        .entrypoint_pc(crate::metadata::KOTO_TEST_RETURN_ENTRYPOINT)
        .expect("contract-backed suite must expose its validated return entrypoint");
    let names = compiled
        .coverage_functions
        .iter()
        .map(|function| function.display_name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(names, vec!["run"]);
}
#[test]
fn nested_contract_effects_use_contract_subject_while_context_keeps_invoker() {
    let asset = AssetDefinitionId::derive_from_components(
        DomainId::try_new("effects", "universal").expect("asset domain"),
        "unit".parse().expect("asset name"),
    )
    .canonical_address();
    let target_source = format!(
        r#"
            seiyaku EffectIdentity {{
                error enum EffectError {{ WrongInvoker = 9001, }}

                kotoage fn mint(AccountId destination) authorize("CanInvokeContractEntrypoint") {{
                    require(
                        context::authority() == AccountId::parse("{DEFAULT_CALLER}"),
                        EffectError::WrongInvoker,
                    );
                    ledger::asset::mint(
                        account: destination,
                        asset_definition: AssetDefinitionId::parse("{asset}"),
                        amount: 1,
                    );
                }}
            }}
            "#,
    );
    let test_source = format!(
        r#"
            module EffectIdentityTests {{
                koto_test {{ target: "effect_identity.ko" }}

                fixture missing_subject_grant {{
                    actor("app", AccountId::parse("{DEFAULT_CALLER}"));
                    caller(AccountId::parse("{DEFAULT_CALLER}"));
                    register_asset_definition(AssetDefinitionId::parse("{asset}"));
                    grant_seiyaku_kotoage_permission("app", "mint");
                }}

                fixture app_only_effect_grant {{
                    actor("app", AccountId::parse("{DEFAULT_CALLER}"));
                    caller(AccountId::parse("{DEFAULT_CALLER}"));
                    register_asset_definition(AssetDefinitionId::parse("{asset}"));
                    grant_seiyaku_kotoage_permission("app", "mint");
                    grant_permission("app", "mint_asset:{asset}");
                }}

                fixture seiyaku_subject_effect_grant {{
                    actor("app", AccountId::parse("{DEFAULT_CALLER}"));
                    caller(AccountId::parse("{DEFAULT_CALLER}"));
                    register_asset_definition(AssetDefinitionId::parse("{asset}"));
                    grant_seiyaku_kotoage_permission("app", "mint");
                    grant_seiyaku_effect_permission("mint_asset:{asset}");
                }}

                #[test(fixture = "missing_subject_grant")]
                fn missing_seiyaku_subject_grant_rejects() {{
                    test::expect_reject_as(
                        actor: "app",
                        kotoage: "mint",
                        arguments: Json::parse("{{\"destination\":\"{DEFAULT_CALLER}\"}}"),
                    );
                }}

                #[test(fixture = "app_only_effect_grant")]
                fn application_effect_grant_does_not_authorize_contract() {{
                    test::expect_reject_as(
                        actor: "app",
                        kotoage: "mint",
                        arguments: Json::parse("{{\"destination\":\"{DEFAULT_CALLER}\"}}"),
                    );
                }}

                #[test(fixture = "seiyaku_subject_effect_grant")]
                fn seiyaku_subject_effect_grant_succeeds_with_invoker_context() {{
                    test::invoke_kotoage_as(
                        actor: "app",
                        kotoage: "mint",
                        arguments: Json::parse("{{\"destination\":\"{DEFAULT_CALLER}\"}}"),
                    );
                }}
            }}
            "#,
    );
    let target_program = parser::parse(&target_source).expect("parse effect target");
    let test_program = parser::parse(&test_source).expect("parse effect tests");
    let suite = finalize_suite(
        PathBuf::from("/tmp/effect_identity.ko"),
        target_source,
        target_program,
        vec![DiscoveredTestModule {
            path: PathBuf::from("/tmp/effect_identity.test.ko"),
            source: test_source,
            program: test_program,
        }],
    )
    .expect("build effect identity suite");
    let compiled = compile_suite(&suite, false).expect("compile effect identity suite");
    let results =
        execute_suite(&compiled, TraceMode::Off, 1).expect("execute effect identity suite");
    assert_eq!(results.len(), 3);
    for result in results {
        assert!(
            result.passed,
            "{} failed: {}",
            result.name,
            result
                .failure
                .unwrap_or_else(|| "unknown failure".to_owned()),
        );
    }
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
        None,
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
        None,
        HashMap::new(),
    );
    let mut public_inputs = BTreeMap::new();
    apply_fixture_action(
        &FixtureAction {
            name: "state_set".to_string(),
            args: vec![
                Expr::String("demo/counter".to_string()),
                Expr::IntLiteral(7_i64.into()),
            ],
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
                    name: "Name::parse".to_string(),
                    args: vec![Expr::String("trigger_event_json".to_string())],
                    argument_names: None,
                    implicit_receiver: false,
                },
                Expr::Call {
                    name: "Json::parse".to_string(),
                    args: vec![Expr::String("{\"count\":7}".to_string())],
                    argument_names: None,
                    implicit_receiver: false,
                },
            ],
        },
        &mut host,
        &mut public_inputs,
    )
    .expect("apply public_input");
    let seeded_counter = host.inner.wsv.sc_get("demo/counter").expect("seeded state");
    assert_eq!(decode_int_state_value(&seeded_counter), 7);
    let trigger_name: Name = "trigger_event_json".parse().expect("name");
    let trigger_payload = public_inputs
        .get(&trigger_name)
        .expect("trigger payload present");
    let tlv = crate::pointer_abi::validate_tlv_bytes(trigger_payload).expect("valid tlv");
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
fn build_host_for_fixture_uses_canonical_default_caller() {
    let compiled = compiled_suite_with_fixtures(Vec::new());
    let host = build_host_for_fixture(&compiled, None).expect("build default host");
    assert_eq!(
        host.caller_subject(),
        parse_account_literal(DEFAULT_CALLER).expect("canonical default caller")
    );
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
    let stored = host.inner.wsv.sc_get("demo/value").expect("state value");
    let envelope = decode_pointer_state_value(&stored, StateValueKindV1::String);
    let value = crate::pointer_abi::validate_tlv_bytes(&envelope).expect("string state TLV");
    assert_eq!(value.type_id, PointerType::Blob);
    assert_eq!(value.payload, b"hello");
}
#[test]
fn apply_fixture_action_registers_actor_seed() {
    let caller = parse_account_literal(DEFAULT_CALLER).expect("caller");
    let mut host = KotoTestHost::new(
        WsvHost::new_with_subject(MockWorldStateView::default(), caller, HashMap::new()),
        None,
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
fn fixture_entrypoint_grant_is_address_and_selector_scoped() {
    let caller = parse_account_literal(DEFAULT_CALLER).expect("caller");
    let actor = caller.clone();
    let mut host = KotoTestHost::new(
        WsvHost::new_with_subject(MockWorldStateView::default(), caller, HashMap::new()),
        None,
        HashMap::new(),
    );
    host.register_actor("operator".to_owned(), actor.clone())
        .expect("register fixture actor");
    let mut public_inputs = BTreeMap::new();
    apply_fixture_action(
        &FixtureAction {
            name: "grant_seiyaku_kotoage_permission".to_owned(),
            args: vec![
                Expr::String("operator".to_owned()),
                Expr::String("apply".to_owned()),
            ],
        },
        &mut host,
        &mut public_inputs,
    )
    .expect("grant exact fixture permission");
    let exact = PermissionToken::ContractEntrypoint {
        contract: host.contract_address.clone(),
        entrypoint: "apply".to_owned(),
    };
    let wrong_selector = PermissionToken::ContractEntrypoint {
        contract: host.contract_address.clone(),
        entrypoint: "other".to_owned(),
    };
    assert!(host.inner.wsv.has_permission(&actor, &exact));
    assert!(!host.inner.wsv.has_permission(&actor, &wrong_selector));
    host.inner.wsv.grant_permission(
        &actor,
        PermissionToken::Custom("CanInvokeContractEntrypoint".to_owned()),
    );
    assert!(
        !host.inner.wsv.has_permission(&actor, &wrong_selector),
        "name-only grants must never materialize a scoped entrypoint capability"
    );
}
#[test]
fn fixture_feature_actions_use_seiyaku_and_kotoage_names_only() {
    let caller = parse_account_literal(DEFAULT_CALLER).expect("caller");
    let mut host = KotoTestHost::new(
        WsvHost::new_with_subject(MockWorldStateView::default(), caller, HashMap::new()),
        None,
        HashMap::new(),
    );
    let mut public_inputs = BTreeMap::new();
    for retired in [
        "grant_contract_entrypoint_permission",
        "grant_contract_effect_permission",
        "grant_contract_transfer_effect_permission",
    ] {
        let error = apply_fixture_action(
            &FixtureAction {
                name: retired.to_owned(),
                args: Vec::new(),
            },
            &mut host,
            &mut public_inputs,
        )
        .expect_err("English feature action must not remain compatible");
        assert_eq!(error, format!("unknown fixture action `{retired}`"));
    }
    for (branded, arity) in [
        ("grant_seiyaku_kotoage_permission", 2),
        ("grant_seiyaku_effect_permission", 1),
        ("grant_seiyaku_transfer_effect_permission", 3),
    ] {
        let error = apply_fixture_action(
            &FixtureAction {
                name: branded.to_owned(),
                args: Vec::new(),
            },
            &mut host,
            &mut public_inputs,
        )
        .expect_err("recognized branded action still requires its arguments");
        assert_eq!(
            error,
            format!("fixture action `{branded}` expects {arity} arguments, got 0")
        );
    }
    assert_eq!(
        eval_fixture_account_or_actor(&Expr::Ident("seiyaku_subject".to_owned()), &host)
            .expect("branded subject expression"),
        host.contract_subject()
    );
    assert!(
        eval_fixture_account_or_actor(&Expr::Ident("contract_subject".to_owned()), &host).is_err(),
        "English feature expression must not remain compatible"
    );
}
#[test]
fn fixture_contract_effect_grant_targets_only_the_immutable_contract_subject() {
    let caller = parse_account_literal(DEFAULT_CALLER).expect("caller");
    let mut host = KotoTestHost::new(
        WsvHost::new_with_subject(
            MockWorldStateView::default(),
            caller.clone(),
            HashMap::new(),
        ),
        None,
        HashMap::new(),
    );
    host.register_actor("app".to_owned(), caller.clone())
        .expect("register app actor");
    let asset = AssetDefinitionId::derive_from_components(
        DomainId::try_new("effects", "universal").expect("domain"),
        "unit".parse().expect("asset name"),
    );
    let permission = PermissionToken::MintAsset(asset.clone());
    let mut public_inputs = BTreeMap::new();
    apply_fixture_action(
        &FixtureAction {
            name: "grant_seiyaku_effect_permission".to_owned(),
            args: vec![Expr::String(format!("mint_asset:{asset}"))],
        },
        &mut host,
        &mut public_inputs,
    )
    .expect("grant contract effect permission");
    assert!(
        host.inner
            .wsv
            .has_permission(&host.contract_subject(), &permission)
    );
    assert!(
        !host.inner.wsv.has_permission(&caller, &permission),
        "contract effect grants must never leak onto the invoking application authority"
    );
}
#[test]
fn transfer_control_effects_require_exact_subject_asset_domain_and_dataspace_scope() {
    let controller = parse_account_literal(DEFAULT_CALLER).expect("controller");
    let target = AccountId::new(
        iroha_crypto::KeyPair::from_seed(vec![0x92; 32], iroha_crypto::Algorithm::Ed25519)
            .public_key()
            .clone(),
    );
    let asset = AssetDefinitionId::derive_from_components(
        DomainId::try_new("currency", "sbp").expect("asset domain"),
        "pkr".parse().expect("asset name"),
    );
    let asset_literal = asset.canonical_address();
    let mut host = KotoTestHost::new(
        WsvHost::new_with_subject(
            MockWorldStateView::default(),
            controller.clone(),
            HashMap::new(),
        ),
        None,
        HashMap::new(),
    );
    host.register_actor("controller".to_owned(), controller.clone())
        .expect("register controller");
    host.register_actor("target".to_owned(), target.clone())
        .expect("register target");
    let mut public_inputs = BTreeMap::new();
    apply_fixture_action(
        &FixtureAction {
            name: "register_asset_definition".to_owned(),
            args: vec![Expr::Call {
                name: "AssetDefinitionId::parse".to_owned(),
                args: vec![Expr::String(asset_literal.clone())],
                argument_names: None,
                implicit_receiver: false,
            }],
        },
        &mut host,
        &mut public_inputs,
    )
    .expect("register control asset");
    apply_fixture_action(
        &FixtureAction {
            name: "register_account_alias".to_owned(),
            args: vec![
                Expr::String("target@hbl.sbp".to_owned()),
                Expr::String("target".to_owned()),
                Expr::IntLiteral(10_i64.into()),
            ],
        },
        &mut host,
        &mut public_inputs,
    )
    .expect("register exact target alias scope");
    let availability_permission_expr = |account: &AccountId| Expr::Call {
        name: "Json::parse".to_owned(),
        args: vec![Expr::String(format!(
            r#"{{"type":"CanSetAssetTransferAvailability","account":"{account}","asset_definition":"{asset_literal}"}}"#,
        ))],
        argument_names: None,
        implicit_receiver: false,
    };
    let scoped_permission_expr = |kind: &str, domain: &str| Expr::Call {
        name: "Json::parse".to_owned(),
        args: vec![Expr::String(format!(
            r#"{{"type":"{kind}","asset_definition":"{asset_literal}","account_domain":"{domain}","account_dataspace":10}}"#,
        ))],
        argument_names: None,
        implicit_receiver: false,
    };
    let exact_holding_permission_expr = |account: &AccountId| Expr::Call {
        name: "Json::parse".to_owned(),
        args: vec![Expr::String(format!(
            r#"{{"type":"CanSetAssetHoldingLimit","account":"{account}","asset_definition":"{asset_literal}"}}"#,
        ))],
        argument_names: None,
        implicit_receiver: false,
    };
    apply_fixture_action(
        &FixtureAction {
            name: "grant_permission".to_owned(),
            args: vec![
                Expr::String("controller".to_owned()),
                availability_permission_expr(&target),
            ],
        },
        &mut host,
        &mut public_inputs,
    )
    .expect("grant exact availability permission to app only");
    apply_fixture_action(
        &FixtureAction {
            name: "grant_seiyaku_effect_permission".to_owned(),
            args: vec![availability_permission_expr(&controller)],
        },
        &mut host,
        &mut public_inputs,
    )
    .expect("grant wrong-account availability permission to subject");
    host.inner
        .bind_contract_runtime_context(
            controller.clone(),
            host.contract_address.clone(),
            "apply_availability".to_owned(),
        )
        .expect("bind contract runtime context");
    let call_availability = |host: &mut KotoTestHost| {
        let mut vm = IVM::new(u64::MAX);
        let account = norito::to_bytes(&target).expect("encode target account");
        let asset_bytes = norito::to_bytes(&asset).expect("encode target asset");
        let account_pointer = vm
            .alloc_input_tlv(&make_tlv(PointerType::AccountId, &account))
            .expect("allocate target account");
        let asset_pointer = vm
            .alloc_input_tlv(&make_tlv(PointerType::AssetDefinitionId, &asset_bytes))
            .expect("allocate target asset");
        vm.set_register(10, account_pointer);
        vm.set_register(11, asset_pointer);
        vm.set_register(12, 0);
        vm.set_register(13, 0);
        let reason_layout =
            crate::sum::SumLayoutV1::option(1).expect("availability reason option layout");
        let reason_pointer = crate::sum::allocate_words(&mut vm, reason_layout, 0, &[])
            .expect("allocate absent availability reason");
        vm.set_register(14, reason_pointer);
        host.inner.syscall(
            crate::syscalls::SYSCALL_SET_ASSET_TRANSFER_AVAILABILITY,
            &mut vm,
        )
    };
    assert_eq!(
        call_availability(&mut host),
        Err(crate::VMError::PermissionDenied),
        "an app grant and a wrong-account subject grant must not authorize the effect",
    );
    assert_eq!(
        host.inner.wsv.asset_transfer_availability(&target, &asset),
        None
    );
    apply_fixture_action(
        &FixtureAction {
            name: "grant_seiyaku_effect_permission".to_owned(),
            args: vec![availability_permission_expr(&target)],
        },
        &mut host,
        &mut public_inputs,
    )
    .expect("grant exact availability permission to contract subject");
    call_availability(&mut host).expect("exact contract-subject availability effect succeeds");
    assert_eq!(
        host.inner.wsv.asset_transfer_availability(&target, &asset),
        Some((1, false, false))
    );
    let mut authority_vm = IVM::new(u64::MAX);
    host.inner
        .syscall(crate::syscalls::SYSCALL_SYSVAR_AUTHORITY, &mut authority_vm)
        .expect("read invoker authority inside contract scope");
    let authority_tlv = authority_vm
        .validate_tlv(authority_vm.register(10))
        .expect("authority TLV");
    let observed_authority: AccountId =
        norito::decode_from_bytes(authority_tlv.payload).expect("decode authority");
    assert_eq!(observed_authority, controller);
    apply_fixture_action(
        &FixtureAction {
            name: "grant_seiyaku_effect_permission".to_owned(),
            args: vec![scoped_permission_expr(
                "CanSetAssetTransferDailyLimit",
                "hbl",
            )],
        },
        &mut host,
        &mut public_inputs,
    )
    .expect("grant exact daily-limit permission to contract subject");
    let mut limit_vm = IVM::new(u64::MAX);
    let account = norito::to_bytes(&target).expect("encode limit target account");
    let asset_bytes = norito::to_bytes(&asset).expect("encode limit target asset");
    let account_pointer = limit_vm
        .alloc_input_tlv(&make_tlv(PointerType::AccountId, &account))
        .expect("allocate limit account");
    let asset_pointer = limit_vm
        .alloc_input_tlv(&make_tlv(PointerType::AssetDefinitionId, &asset_bytes))
        .expect("allocate limit asset");
    let cap = Quantity::from(500_u64);
    let cap_payload = QuantityValueV1::new(cap.clone())
        .encode_frame()
        .expect("encode cap quantity frame");
    let cap_pointer = limit_vm
        .alloc_input_tlv(&make_tlv(PointerType::Quantity, &cap_payload))
        .expect("allocate cap quantity");
    let cap_option = crate::sum::allocate_words(
        &mut limit_vm,
        crate::sum::SumLayoutV1::option(1).expect("option layout"),
        1,
        &[cap_pointer],
    )
    .expect("allocate cap option");
    limit_vm.set_register(10, account_pointer);
    limit_vm.set_register(11, asset_pointer);
    limit_vm.set_register(12, cap_option);
    host.inner
        .syscall(
            crate::syscalls::SYSCALL_SET_ASSET_TRANSFER_DAILY_LIMIT,
            &mut limit_vm,
        )
        .expect("exact contract-subject daily limit succeeds");
    assert_eq!(
        host.inner.wsv.asset_transfer_daily_limit(&target, &asset),
        Some(Some(cap.clone()))
    );
    apply_fixture_action(
        &FixtureAction {
            name: "grant_seiyaku_effect_permission".to_owned(),
            args: vec![exact_holding_permission_expr(&target)],
        },
        &mut host,
        &mut public_inputs,
    )
    .expect("grant exact holding-limit permission to contract subject");
    let mut holding_vm = IVM::new(u64::MAX);
    let account_pointer = holding_vm
        .alloc_input_tlv(&make_tlv(PointerType::AccountId, &account))
        .expect("allocate holding-limit account");
    let asset_pointer = holding_vm
        .alloc_input_tlv(&make_tlv(PointerType::AssetDefinitionId, &asset_bytes))
        .expect("allocate holding-limit asset");
    let limit_pointer = holding_vm
        .alloc_input_tlv(&make_tlv(PointerType::Quantity, &cap_payload))
        .expect("allocate holding-limit quantity");
    let limit_option = crate::sum::allocate_words(
        &mut holding_vm,
        crate::sum::SumLayoutV1::option(1).expect("holding option layout"),
        1,
        &[limit_pointer],
    )
    .expect("allocate holding-limit option");
    holding_vm.set_register(10, account_pointer);
    holding_vm.set_register(11, asset_pointer);
    holding_vm.set_register(12, limit_option);
    host.inner
        .syscall(
            crate::syscalls::SYSCALL_SET_ASSET_HOLDING_LIMIT,
            &mut holding_vm,
        )
        .expect("exact contract holding limit succeeds");
    assert_eq!(
        host.inner.wsv.asset_holding_limit(&target, &asset),
        Some(Some(cap))
    );
}
#[test]
fn fixture_account_alias_registration_is_canonical_unique_and_resolvable() {
    let caller = parse_account_literal(DEFAULT_CALLER).expect("caller");
    let other = AccountId::new(
        iroha_crypto::KeyPair::from_seed(vec![0x91; 32], iroha_crypto::Algorithm::Ed25519)
            .public_key()
            .clone(),
    );
    let mut host = KotoTestHost::new(
        WsvHost::new_with_subject(
            MockWorldStateView::default(),
            caller.clone(),
            HashMap::new(),
        ),
        None,
        HashMap::new(),
    );
    host.register_actor("merchant".to_owned(), caller.clone())
        .expect("register merchant actor");
    host.register_actor("other".to_owned(), other)
        .expect("register other actor");
    let mut public_inputs = BTreeMap::new();
    let registration = FixtureAction {
        name: "register_account_alias".to_owned(),
        args: vec![
            Expr::String("merchant@hbl.sbp".to_owned()),
            Expr::String("merchant".to_owned()),
        ],
    };
    apply_fixture_action(&registration, &mut host, &mut public_inputs)
        .expect("register canonical domain-scoped account alias");
    let mut vm = IVM::new(u64::MAX);
    let pointer = vm
        .alloc_input_tlv(&make_tlv(PointerType::Blob, b"merchant@hbl.sbp"))
        .expect("allocate alias argument");
    vm.set_register(10, pointer);
    host.inner
        .syscall(crate::syscalls::SYSCALL_RESOLVE_ACCOUNT_ALIAS, &mut vm)
        .expect("resolve seeded alias");
    let resolved_tlv = vm
        .validate_tlv(vm.register(10))
        .expect("resolved account TLV");
    assert_eq!(resolved_tlv.type_id, PointerType::AccountId);
    let resolved: AccountId =
        norito::decode_from_bytes(resolved_tlv.payload).expect("decode resolved account");
    assert_eq!(resolved, caller);
    let duplicate = apply_fixture_action(&registration, &mut host, &mut public_inputs)
        .expect_err("duplicate alias registration must fail");
    assert!(duplicate.contains("duplicate account alias registration"));
    let conflict = apply_fixture_action(
        &FixtureAction {
            name: "register_account_alias".to_owned(),
            args: vec![
                Expr::String("merchant@hbl.sbp".to_owned()),
                Expr::String("other".to_owned()),
            ],
        },
        &mut host,
        &mut public_inputs,
    )
    .expect_err("conflicting alias registration must fail");
    assert!(conflict.contains("conflicting account alias registration"));
    for alias in [
        "merchant",
        "merchant@@sbp",
        "merchant@",
        "@sbp",
        "merchant@hbl.sbp.extra",
        " merchant@sbp",
        "merchant@sbp ",
    ] {
        let error = apply_fixture_action(
            &FixtureAction {
                name: "register_account_alias".to_owned(),
                args: vec![
                    Expr::String(alias.to_owned()),
                    Expr::String("merchant".to_owned()),
                ],
            },
            &mut host,
            &mut public_inputs,
        )
        .expect_err("noncanonical alias registration must fail");
        assert!(!error.is_empty(), "missing rejection for `{alias}`");
    }
}
#[test]
fn helper_parsers_reject_invalid_numeric_and_mintability() {
    let err = eval_numeric_expr(&Expr::IntLiteral((-1_i64).into()))
        .expect_err("negative quantity should fail");
    assert!(err.contains("negative balances are not allowed"));
    let err = eval_quantity_expr(&Expr::String("-1".to_owned()))
        .expect_err("negative decimal quantity should fail at the nominal boundary");
    assert!(err.contains("balance must be a non-negative quantity"));
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
    let asset = AssetDefinitionId::derive_from_components(domain, "rose".parse().expect("name"));
    let token = parse_permission_token_name(&format!("mint_asset:{asset}"))
        .expect("parse mint asset token");
    assert!(matches!(token, PermissionToken::MintAsset(id) if id == asset));
    let token = parse_permission_token_json(r#"{"type":"custom","name":"demo.permission"}"#)
        .expect("parse custom permission json");
    assert!(matches!(token, PermissionToken::Custom(name) if name == "demo.permission"));
    let err = parse_permission_token_json(r#"{"target":"missing-type"}"#)
        .expect_err("missing type should fail");
    assert!(err.contains("missing `type`"));
    let owner = parse_account_literal(DEFAULT_CALLER).expect("asset owner");
    let bucket = AssetId::with_scope(
        asset.clone(),
        owner,
        AssetBalanceScope::Dataspace(DataSpaceId::new(10)),
    );
    let token = parse_permission_token_json(&format!(
        r#"{{"type":"CanTransferAsset","asset":"{}"}}"#,
        bucket.canonical_literal(),
    ))
    .expect("parse exact transfer bucket permission");
    assert!(matches!(token, PermissionToken::TransferAssetBucket(id) if id == bucket));
    for invalid in [
        format!(
            r#"{{"type":"CanTransferAsset","asset":"{}","asset_definition":"{}"}}"#,
            bucket.canonical_literal(),
            asset.canonical_address(),
        ),
        format!(
            r#"{{"type":"CanTransferAsset","asset":"{}#dataspace:010"}}"#,
            AssetId::new(
                asset.clone(),
                parse_account_literal(DEFAULT_CALLER).expect("owner")
            )
            .canonical_literal(),
        ),
    ] {
        parse_permission_token_json(&invalid)
            .expect_err("ambiguous or non-canonical transfer bucket must fail");
    }
    let availability_account = parse_account_literal(DEFAULT_CALLER).expect("account");
    let availability = parse_permission_token_json(&format!(
            r#"{{"type":"CanSetAssetTransferAvailability","account":"{availability_account}","asset_definition":"{}"}}"#,
            asset.canonical_address(),
        ))
        .expect("parse exact availability permission");
    assert!(matches!(
        availability,
        PermissionToken::SetAssetTransferAvailability {
            account,
            asset_definition,
        } if account == availability_account && asset_definition == asset
    ));
    let daily_limit = parse_permission_token_json(&format!(
            r#"{{"type":"CanSetAssetTransferDailyLimit","asset_definition":"{}","account_domain":"hbl","account_dataspace":10}}"#,
            asset.canonical_address(),
        ))
        .expect("parse scoped daily-limit permission");
    assert!(matches!(
        daily_limit,
        PermissionToken::SetAssetTransferDailyLimit {
            asset_definition,
            account_domain,
            account_dataspace,
        } if asset_definition == asset
            && account_domain.as_ref() == "hbl"
            && account_dataspace == DataSpaceId::new(10)
    ));
    let holding_limit = parse_permission_token_json(&format!(
            r#"{{"type":"CanSetAssetHoldingLimit","account":"{availability_account}","asset_definition":"{}"}}"#,
            asset.canonical_address(),
        ))
        .expect("parse exact holding-limit permission");
    assert!(matches!(
        holding_limit,
        PermissionToken::SetAssetHoldingLimit {
            account,
            asset_definition,
        } if account == availability_account && asset_definition == asset
    ));
    for invalid in [
        format!(
            r#"{{"type":"CanSetAssetTransferAvailability","asset_definition":"{}"}}"#,
            asset.canonical_address(),
        ),
        format!(
            r#"{{"type":"CanSetAssetTransferAvailability","account":"not-an-account","asset_definition":"{}"}}"#,
            asset.canonical_address(),
        ),
        format!(
            r#"{{"type":"CanSetAssetTransferAvailability","account":"{availability_account}","asset_definition":"{}","legacy":true}}"#,
            asset.canonical_address(),
        ),
    ] {
        parse_permission_token_json(&invalid)
            .expect_err("legacy, ambiguous, or extra transfer-control scope must fail");
    }
}
#[test]
fn permission_and_json_helpers_reject_invalid_inputs() {
    let err = parse_permission_token_name("mint_asset:not-an-asset")
        .expect_err("invalid targeted permission should fail");
    assert!(err.contains("invalid asset definition id"));
    let err = eval_json_payload(&[Expr::IntLiteral(7_i64.into())])
        .expect_err("non-string json should fail");
    assert!(err.contains("expects a string payload"));
}
#[test]
fn eval_envelope_expr_encodes_pointer_variants() {
    let account_expr = Expr::Call {
        name: "AccountId::parse".to_string(),
        args: vec![Expr::String(DEFAULT_CALLER.to_string())],
        argument_names: None,
        implicit_receiver: false,
    };
    let account_ptr = eval_envelope_expr(&account_expr).expect("account envelope");
    let account_tlv = crate::pointer_abi::validate_tlv_bytes(&account_ptr).expect("account tlv");
    assert_eq!(account_tlv.type_id, PointerType::AccountId);
    let name_expr = Expr::Call {
        name: "Name::parse".to_string(),
        args: vec![Expr::String("cursor".to_string())],
        argument_names: None,
        implicit_receiver: false,
    };
    let name_ptr = eval_envelope_expr(&name_expr).expect("name envelope");
    let name_tlv = crate::pointer_abi::validate_tlv_bytes(&name_ptr).expect("name tlv");
    assert_eq!(name_tlv.type_id, PointerType::Name);
    let alternate_flags =
        norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
    let _ambient = norito::core::DecodeFlagsGuard::enter(alternate_flags);
    assert_eq!(
        eval_envelope_expr(&account_expr).expect("ambient account envelope"),
        account_ptr
    );
    assert_eq!(
        eval_envelope_expr(&name_expr).expect("ambient name envelope"),
        name_ptr
    );
}
#[test]
fn fixture_evaluators_reject_retired_flat_constructor_aliases() {
    let call = |name: &str, value: &str| Expr::Call {
        name: name.to_owned(),
        args: vec![Expr::String(value.to_owned())],
        argument_names: None,
        implicit_receiver: false,
    };
    assert!(eval_account_expr(&call("account_id", DEFAULT_CALLER)).is_err());
    assert!(eval_domain_expr(&call("domain_id", "wonderland")).is_err());
    assert!(eval_asset_definition_expr(&call("asset_definition", "rose#wonderland")).is_err());
    assert!(eval_name_expr(&call("name", "cursor")).is_err());
    assert!(eval_envelope_expr(&call("json", "{}")).is_err());
    assert!(eval_actor_alias_expr(&call("name", "issuer")).is_err());
    assert!(eval_seed_expr(&call("blob", &format!("0x{}", "00".repeat(32)))).is_err());
}
#[test]
fn render_failure_without_diagnostic_falls_back_to_debug_error() {
    let vm = IVM::new(u64::MAX);
    let rendered = render_failure(&vm, None, &crate::VMError::DecodeError);
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
        unit: crate::kotodama::ast::SourceUnit {
            kind: crate::kotodama::ast::SourceUnitKind::Module,
            name: "DuplicateTests".to_string(),
        },
        items: vec![
            test_function("smoke", None),
            test_function("smoke", Some("seeded")),
        ],
        test_target: None,
        fixtures: Vec::new(),
    };
    let mut names = HashSet::new();
    let mut tests = Vec::new();
    let err = collect_tests_into(&program, &mut names, &mut tests)
        .expect_err("duplicate test names should fail");
    assert!(err.contains("duplicate test function"));
}
