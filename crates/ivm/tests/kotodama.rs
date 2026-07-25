//! Tests for Kotodama parsing, semantics, and compilation.

use std::convert::TryInto;

use iroha_crypto as _;
use iroha_data_model::{
    nexus::{DataSpaceId, LaneId},
    prelude::Quantity,
};
use ivm::{
    ProgramMetadata, axt, encoding, instruction,
    kotodama::{
        ast as kd_ast,
        ast::{BinaryOp, Expr, Function, Item, Statement},
        compiler::{Compiler, CompilerMode, CompilerOptions},
        lexer::{TokenKind, lex},
        parser::parse as parse_source,
        semantic::{Type, analyze},
    },
    syscalls,
};
mod common;

fn parse_meta_offset(code: &[u8]) -> Result<(ProgramMetadata, usize), ivm::VMError> {
    ProgramMetadata::parse(code).map(|parsed| (parsed.metadata, parsed.code_offset))
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write as _;
        let _ = write!(&mut s, "{b:02x}");
    }
    s
}

fn test_compiler() -> Compiler {
    Compiler::new_with_options(CompilerOptions {
        mode: CompilerMode::Test,
        ..CompilerOptions::default()
    })
}

fn select_test_entrypoint(
    vm: &mut ivm::IVM,
    program: &[u8],
    report: &ivm::kotodama::compiler::CompileReport,
    name: &str,
) {
    let parsed = ProgramMetadata::parse(program).expect("parse Kotodama test artifact");
    assert!(
        parsed.contract_interface.is_none(),
        "test-mode artifacts must not embed a deployable CNTR section"
    );
    let implementation = format!("__entrypoint_impl__{name}");
    let function = report
        .budget_report
        .iter()
        .find(|function| function.function_name == implementation)
        .or_else(|| {
            report
                .budget_report
                .iter()
                .find(|function| function.function_name == name)
        })
        .unwrap_or_else(|| panic!("missing test entrypoint `{name}`"));
    let return_offset = program
        .len()
        .checked_sub(parsed.header_len + core::mem::size_of::<u32>())
        .expect("test artifact contains compiler-owned terminal HALT");
    assert_eq!(
        program[program.len() - core::mem::size_of::<u32>()..],
        encoding::wide::encode_halt().to_le_bytes(),
        "test artifact must end with the compiler-owned terminal HALT"
    );
    vm.set_register(
        1,
        u64::try_from(return_offset).expect("test return PC fits u64"),
    );
    let pc =
        u64::try_from(parsed.prefix_len()).expect("program prefix fits u64") + function.pc_start;
    vm.set_program_counter(pc)
        .unwrap_or_else(|error| panic!("select test entrypoint `{name}`: {error:?}"));
}

fn parse(source: &str) -> Result<kd_ast::Program, String> {
    parse_source(source)
}

#[test]
fn create_compiler() {
    let _c = Compiler::new();
}

#[test]
fn compile_stub() {
    let compiler = Compiler::new();
    let result = compiler.compile_source("ADD 1, 2");
    assert!(result.is_err(), "compiler should reject invalid source");
}

#[test]
fn lex_simple_function() {
    let src = "fn add(int a, int b) { let c = a + b; }";
    let tokens = lex(src).expect("lex failed");
    assert!(tokens.iter().any(|t| matches!(t.kind, TokenKind::Fn)));
    assert!(
        tokens
            .iter()
            .any(|t| matches!(t.kind, TokenKind::Ident(ref s) if s == "add"))
    );
}

#[test]
fn parse_simple_add() {
    let src = "module SimpleAdd { fn add(int a, int b) { let c = a + b; } }";
    let prog = parse(src).expect("parse failed");
    let Item::Function(Function {
        name, params, body, ..
    }) = &prog.items[0]
    else {
        panic!("expected function item");
    };
    assert_eq!(name, "add");
    assert_eq!(params.len(), 2);
    assert_eq!(params[0].name, "a");
    assert_eq!(params[1].name, "b");
    match &body.statements[0] {
        Statement::Let { pat, value, .. } => {
            match pat {
                kd_ast::Pattern::Name(n) => assert_eq!(n, "c"),
                _ => panic!("expected name"),
            }
            match value {
                Expr::Binary {
                    op: BinaryOp::Add, ..
                } => {}
                _ => panic!("expected add expr"),
            }
        }
        _ => panic!("unexpected statement"),
    }
}

#[test]
fn lexer_accepts_v1_branded_keywords_in_both_scripts() {
    use ivm::kotodama::lexer::{TokenKind, lex};
    let seiyaku = lex("seiyaku Demo { }").expect("lex seiyaku");
    assert!(matches!(seiyaku[0].kind, TokenKind::Seiyaku));
    let hajimari = lex("hajimari() {}").expect("lex");
    assert!(
        hajimari
            .iter()
            .any(|token| matches!(token.kind, TokenKind::Hajimari))
    );
    for branded in [
        "誓約 Demo { }",
        "始まり() {}",
        "言挙げ fn run() {}",
        "改善() {}",
    ] {
        lex(branded).expect("Japanese branded declaration must be accepted");
    }
}

#[test]
fn parse_and_type_tuples_and_types() {
    let src =
        "module Types { fn t(int x) -> (int, bool) { let (a, b) = (1, true); return (x, true); } }";
    let prog = parse(src).expect("parse");
    let typed = analyze(&prog).expect("type");
    let ivm::kotodama::semantic::TypedItem::Function(f) = &typed.items[0];
    assert_eq!(f.name, "t");
}

#[test]
fn bytes_type_is_accepted_and_roundtrips_through_semantics() {
    let src = "module BytesDemo { fn echo(bytes b) -> bytes { let bytes tmp = b; return tmp; } }";
    let prog = parse(src).expect("parse bytes");
    let typed = analyze(&prog).expect("analyze bytes");
    let ivm::kotodama::semantic::TypedItem::Function(f) = &typed.items[0];
    assert_eq!(f.ret_ty, Some(Type::Bytes));
}

#[test]
fn string_equality_compiles() {
    let src = "seiyaku StringEquality { view fn f() { let _x = \"hi\" == \"hi\"; } }";
    let code = Compiler::new()
        .compile_source(src)
        .expect("string equality should compile");
    assert!(!code.is_empty());
}

#[test]
fn irohaswap_sample_compiles() {
    let src = include_str!("../../kotodama_lang/src/samples/irohaswap.ko");
    let code = test_compiler()
        .compile_source(src)
        .expect("irohaswap sample should compile");
    assert!(!code.is_empty());
}

#[test]
fn prediction_market_demo_compiles() {
    let src = include_str!("../../../demo/prediction_market.ko");
    let code = test_compiler()
        .compile_source(src)
        .expect("prediction market demo should compile");
    assert!(!code.is_empty());
}

#[test]
fn tuple_destructure_and_field_access() {
    // Destructure a tuple literal into (a,b) and sum; also exercise direct field access `(1,2).1`
    let src = "seiyaku TupleDestructure { view fn sum() -> int { let (a,b) = (3,4); let c = (1,2).1; return a + b + c; } }";
    let code = Compiler::new()
        .compile_source(src)
        .expect("compile tuple destructure");
    let mut vm = ivm::IVM::new(u64::MAX);
    vm.load_program(&code).unwrap();
    common::select_kotodama_entrypoint(&mut vm, &code, "sum");
    match vm.run() {
        Ok(_) => {
            eprintln!("tuple_destructure_and_field_access r10={}", vm.register(10));
            assert_eq!(common::decode_i64_register(&vm, 10), 3 + 4 + 2);
        }
        Err(e) => {
            panic!("vm run error: {e:?}");
        }
    }
}

#[test]
fn tuple_var_member_access() {
    // Bind a tuple to a name and use member access on it.
    let src = "seiyaku TupleMember { view fn f() -> int { let t = (5,6); return t.0 + t.1; } }";
    let code = Compiler::new()
        .compile_source(src)
        .expect("compile tuple var member");
    let mut vm = ivm::IVM::new(u64::MAX);
    vm.load_program(&code).unwrap();
    common::select_kotodama_entrypoint(&mut vm, &code, "f");
    match vm.run() {
        Ok(_) => {
            eprintln!("tuple_var_member_access r10={}", vm.register(10));
            assert_eq!(common::decode_i64_register(&vm, 10), 11);
        }
        Err(e) => panic!("vm run error: {e:?}"),
    }
}

#[test]
fn call_function_with_tuple_return() {
    let src = r#"
        seiyaku TupleCall {
            fn pair(int x) -> (int, int) { return (x, x + 1); }
            view fn main() -> int {
                let (a, b) = pair(7);
                return a * b;
            }
        }
    "#;
    let code = Compiler::new()
        .compile_source(src)
        .expect("compile tuple-returning call");
    let mut vm = ivm::IVM::new(u64::MAX);
    vm.load_program(&code).expect("load program");
    common::select_kotodama_entrypoint(&mut vm, &code, "main");
    vm.run().expect("run tuple call");
    assert_eq!(common::decode_i64_register(&vm, 10), 56);
}

#[test]
fn quantity_arithmetic_compiles_without_implicit_conversion() {
    let src = r#"
        seiyaku QuantityArithmetic {
            view fn main() -> quantity {
                let quantity a = 9_000_000_000;
                let decimal factor = 2;
                let quantity b = a * factor;
                let quantity c = b / factor;
                return c;
            }
        }
    "#;
    Compiler::new()
        .compile_source(src)
        .expect("compile quantity arithmetic");
}

#[test]
fn negative_quantity_conversion_is_rejected() {
    let src = r#"
        seiyaku NegativeQuantity {
            fn main() -> quantity {
                let quantity a = -1;
                return a;
            }
        }
    "#;
    let err = Compiler::new()
        .compile_source(src)
        .expect_err("negative quantity literal should fail");
    assert!(err.to_string().contains("E_NEGATIVE_QUANTITY"), "{err}");
}

#[test]
fn fractional_quantity_literal_is_accepted_contextually() {
    let src = r#"
        seiyaku DecimalQuantity {
            view fn main() -> bool {
                let quantity a = 1.50;
                return a == a;
            }
        }
    "#;
    Compiler::new()
        .compile_source(src)
        .expect("fractional quantity literal should compile in quantity context");
}

#[test]
fn decimal_literal_rejects_int_annotation() {
    let prog = parse(
        r#"
        module InvalidDecimalAnnotation {
            fn main() -> int {
                let int a = 1.5;
                return a;
            }
        }
        "#,
    )
    .expect("parse decimal literal");
    let err = analyze(&prog).expect_err("expected decimal literal type error");
    assert_eq!(err.code(), "E_TYPE_ANNOTATION_MISMATCH");
    assert!(
        err.message().contains("expected int, got decimal"),
        "unexpected error message: {}",
        err.message()
    );
}

#[test]
fn implicit_quantity_to_int_conversion_is_rejected() {
    let src = r#"
        seiyaku NoImplicitQuantityCast {
            fn main() -> int {
                let quantity a = 9_000_000_000;
                let int c = a;
                return c;
            }
        }
    "#;
    let error = Compiler::new()
        .compile_source(src)
        .expect_err("implicit quantity-to-int conversion must fail");
    assert!(error.contains("expected int, got quantity"), "{error}");
}

#[test]
fn assert_builtin_obeys_truthiness() {
    let compiler = test_compiler();

    let (pass, _manifest, pass_report) = compiler
        .compile_source_with_manifest_and_report(
            "seiyaku AssertTrue { view fn main() { test::assert(true); } }",
        )
        .expect("compile passing assert");
    let mut vm = ivm::IVM::new(u64::MAX);
    vm.load_program(&pass).expect("load passing assert");
    select_test_entrypoint(&mut vm, &pass, &pass_report, "main");
    vm.run().expect("test::assert(true) should not abort");

    let (fail, _manifest, fail_report) = compiler
        .compile_source_with_manifest_and_report(
            "seiyaku AssertFalse { view fn main() { test::assert(false); } }",
        )
        .expect("compile failing assert");
    let mut vm = ivm::IVM::new(u64::MAX);
    vm.load_program(&fail).expect("load failing assert");
    select_test_entrypoint(&mut vm, &fail, &fail_report, "main");
    let err = vm.run().expect_err("test::assert(false) should abort");
    assert!(matches!(err, ivm::VMError::AssertionFailed));
}

#[test]
fn many_string_literals_load_under_wide_guard() {
    // Exercise pointer literal emission with offsets beyond the wide 8-bit range.
    let mut src = String::from("seiyaku Literals { kotoage fn main() authorize(\"Test\") {");
    for i in 0..32 {
        src.push_str(&format!(" debug::info(\"literal_{i}\");"));
    }
    src.push_str(" return;");
    src.push_str("} }");

    let code = Compiler::new()
        .compile_source(&src)
        .expect("compile program with many literals");
    let mut vm = ivm::IVM::new(u64::MAX);
    vm.load_program(&code)
        .expect("wide guard must accept Kotodama output");
}

#[test]
fn pointer_constructors_compile() {
    let src = r#"
        seiyaku ConstructorDemo {
            kotoage fn run() authorize("Admin") {
                let alice = AccountId::parse("sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV");
                let bob = AccountId::parse("sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76");
                let asset = AssetDefinitionId::parse("62Fk4FPcMuLvW5QjDGNF2a4jAmjM");
                ledger::account::set_detail(account: context::authority(), key: Name::parse("cursor"), value: Json::parse("{\"query\":\"sc_dummy\",\"cursor\":1}"));
                ledger::asset::transfer(source: alice, destination: bob, asset_definition: asset, amount: 1, dataspace: DataSpaceId::parse("0"));
            }
        }
    "#;
    test_compiler()
        .compile_source(src)
        .expect("compile typed pointer constructors");
}

#[test]
fn public_function_without_authorization_rejected() {
    let src = r#"
        seiyaku PermissionDemo {
            kotoage fn run() {
                ledger::asset::transfer(source: AccountId::parse("sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"), destination: AccountId::parse("sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76"), asset_definition: AssetDefinitionId::parse("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"), amount: 1, dataspace: DataSpaceId::parse("0"));
            }
        }
    "#;
    let err = Compiler::new()
        .compile_source(src)
        .expect_err("missing permission should be rejected");
    assert!(
        err.contains("authorize"),
        "error should mention missing authorization: {err}"
    );
}

#[test]
fn register_peer_requires_permission() {
    let src = r#"
        seiyaku PermissionDemo {
            kotoage fn add() {
                ledger::peer::register(Json::parse("{\"address\":\"127.0.0.1:1337\"}"));
            }
        }
    "#;
    let err = Compiler::new()
        .compile_source(src)
        .expect_err("missing permission should be rejected");
    assert!(
        err.contains("authorize"),
        "error should mention missing authorization for peer registration: {err}"
    );
}

#[test]
fn register_account_requires_permission() {
    let src = r#"
        seiyaku PermissionDemo {
            kotoage fn add() {
                ledger::account::register(AccountId::parse("sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"));
            }
        }
    "#;
    let err = Compiler::new()
        .compile_source(src)
        .expect_err("missing permission should be rejected");
    assert!(
        err.contains("authorize"),
        "error should mention missing authorization for account registration: {err}"
    );
}

#[test]
fn trigger_management_requires_permission() {
    let src = r#"
        seiyaku PermissionDemo {
            kotoage fn add() {
                ledger::trigger::create(Json::parse("{\"id\":\"t1\"}"));
                ledger::trigger::set_enabled(Name::parse("t1"), 1);
            }
        }
    "#;
    let err = Compiler::new()
        .compile_source(src)
        .expect_err("missing permission should be rejected");
    assert!(
        err.contains("authorize"),
        "error should mention missing authorization for trigger operations: {err}"
    );
}

#[test]
fn public_function_with_permission_is_allowed() {
    let src = r#"
        seiyaku PermissionDemo {
            kotoage fn run() authorize("Admin") {
                ledger::asset::transfer(source: AccountId::parse("sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"), destination: AccountId::parse("sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76"), asset_definition: AssetDefinitionId::parse("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"), amount: 1, dataspace: DataSpaceId::parse("0"));
            }
        }
    "#;
    Compiler::new()
        .compile_source(src)
        .expect("permission attribute should allow privileged call");
}

#[test]
fn removed_in_memory_map_type_is_rejected() {
    let src = r#"
        seiyaku Demo {
            state Map<string, int> detail;

            view fn main() {}
        }
    "#;

    let err = Compiler::new()
        .compile_source(src)
        .expect_err("in-memory Map must be rejected");
    assert!(
        err.contains("Map") || err.contains("unknown type"),
        "unexpected error: {err}"
    );
}

#[test]
fn parse_for_each_map_and_builtins() {
    let src = r#"
        seiyaku StateIteration {
            state StateMap<int, int> values;
            view fn f() { for (k, v) in (values) { let seen = v; } }
        }
    "#;
    let err = parse(src).expect_err("bare StateMap iteration must be rejected");
    assert!(
        err.contains("StateMap iteration requires `.take(N)` or `.range(start, end)`"),
        "error hint should mention the canonical bounded helpers: {err}"
    );
}

#[test]
fn c_style_for_loop_is_rejected() {
    let src = "module Loops { fn f() { for var i = 0; i < 3; i = i + 1 { let x = i; } } }";
    parse(src).expect_err("V1 only accepts compiler-proven bounded iterator loops");
}

#[test]
fn literal_range_for_loop_is_bounded() {
    let src = "module Loops { fn f() { for x in range(6) { let y = x; } } }";
    let prog = parse(src).expect("parse should succeed");
    analyze(&prog).expect("a literal range within the V1 bound must type-check");
}

#[test]
fn state_map_take_two_is_bounded() {
    let src = r#"
        seiyaku StateIteration {
            state StateMap<int, int> values;
            fn f() { for (k, v) in values.take(2) { let z = k; } }
        }
    "#;
    let prog = parse(src).expect("parse");
    analyze(&prog).expect("StateMap take(2) is compiler-bounded");
}

#[test]
fn removed_bounded_attribute_is_rejected() {
    let src = r#"
        seiyaku StateIteration {
            state StateMap<int, int> values;
            fn f() { for (k, v) in values #[bounded(1)] { let z = k; } }
        }
    "#;
    parse(src).expect_err("legacy bounded attributes are not V1 syntax");
}

#[test]
fn parse_and_type_bounded_map_take_one_ok() {
    let src = r#"
        seiyaku StateIteration {
            state StateMap<int, int> values;
            fn f() { for (k, v) in values.take(1) { let z = k; } }
        }
    "#;
    let prog = parse(src).expect("parse");
    let typed = analyze(&prog).expect("analyze");
    let ivm::kotodama::semantic::TypedItem::Function(func) = &typed.items[0];
    assert!(!func.body.statements.is_empty());
}

#[test]
fn compile_domain_literal_emits_tlv_domainid() {
    let src = r#"
        seiyaku DomainLiteral {
            view fn get() -> DomainId {
                return DomainId::parse("wonderland.universal");
            }
        }
    "#;
    let compiler = Compiler::new();
    let bytes = compiler.compile_source(src).expect("compile ok");
    assert!(
        bytes.windows(2).any(|w| w == [0x00, 0x08]),
        "expected DomainId TLV type (0x0008) in compiled artifact"
    );
}

#[test]
fn compile_register_domain_emits_syscall_0x10() {
    use ivm::encoding;
    let src = r#"
        seiyaku RegisterDomain {
            kotoage fn register() authorize("Admin") {
                ledger::domain::register(DomainId::parse("wonderland.universal"));
            }
        }
    "#;
    let compiler = Compiler::new();
    let bytes = compiler.compile_source(src).expect("compile ok");
    // Expected sys encoding for SCALL with imm8=0x10 (SYSCALL_REGISTER_DOMAIN)
    let word = encoding::wide::encode_sys(instruction::wide::system::SCALL, 0x10);
    let needle = word.to_le_bytes();
    assert!(
        bytes.windows(4).any(|w| w == needle),
        "expected SCALL imm8=0x10 (register_domain) in compiled bytecode"
    );
}

#[test]
fn compile_zk_verify_batch_emits_syscall_0x68() {
    // Ensure the kotodama intrinsic lowers to SCALL 0x68
    let src = r#"
        seiyaku VerifyBatch {
            kotoage fn verify(bytes p) authorize("ZkVerifier") {
                crypto::zk::verify_batch(p);
            }
        }
    "#;
    let compiler = Compiler::new();
    let bytes = compiler.compile_source(src).expect("compile ok");
    let word = encoding::wide::encode_sys(instruction::wide::system::SCALL, 0x68);
    let needle = word.to_le_bytes();
    assert!(
        bytes.windows(4).any(|w| w == needle),
        "expected SCALL imm8=0x68 (zk_verify_batch) in compiled bytecode"
    );
}

#[test]
fn compile_blob_literal_emits_tlv_blob() {
    // Ensure a bytes literal emits the canonical bytes TLV (wire type 0x0006).
    let src = r#"
        seiyaku BlobLiteral {
            view fn get() -> bytes {
                return b"hello";
            }
        }
    "#;
    let bytes = Compiler::new().compile_source(src).expect("compile ok");
    assert!(
        bytes.windows(2).any(|w| w == [0x00, 0x06]),
        "expected bytes TLV type (0x0006) in compiled artifact"
    );
}

#[test]
fn semantic_typed_pointers_and_authority() {
    let src = r#"
        seiyaku TypedPointers {
            kotoage fn f() authorize("Admin") {
                ledger::account::set_detail(account: context::authority(), key: Name::parse("k"), value: Json::parse("1"));
            }
        }
    "#;
    let prog = parse(src).expect("parse");
    let res = analyze(&prog);
    assert!(
        res.is_ok(),
        "semantics should accept typed pointers + authority"
    );
}

#[test]
fn compile_emits_get_authority_syscall() {
    let src = r#"seiyaku Authority { view fn f() -> AccountId { return context::authority(); } }"#;
    let code = Compiler::new().compile_source(src).expect("compile");
    let (_, off) = parse_meta_offset(&code).unwrap();
    let mut words = Vec::new();
    let mut i = off;
    while i + 4 <= code.len() {
        words.push(u32::from_le_bytes(code[i..i + 4].try_into().unwrap()));
        i += 4;
    }
    let scall = instruction::wide::system::SCALL;
    let want = encoding::wide::encode_sys(scall, syscalls::SYSCALL_GET_AUTHORITY as u8);
    assert!(words.contains(&want), "GET_AUTHORITY syscall not found");
}

#[test]
fn compile_emits_current_time_syscall() {
    let src = r#"seiyaku Time { view fn f() -> int { return context::current_time_ms(); } }"#;
    let code = Compiler::new().compile_source(src).expect("compile");
    let (_, off) = parse_meta_offset(&code).unwrap();
    let mut words = Vec::new();
    let mut i = off;
    while i + 4 <= code.len() {
        words.push(u32::from_le_bytes(code[i..i + 4].try_into().unwrap()));
        i += 4;
    }
    let scall = instruction::wide::system::SCALL;
    let want = encoding::wide::encode_sys(scall, syscalls::SYSCALL_CURRENT_TIME_MS as u8);
    assert!(words.contains(&want), "CURRENT_TIME_MS syscall not found");
}

#[test]
fn compile_emits_block_height_syscall() {
    let src = r#"seiyaku Height { view fn f() -> int { return context::block_height(); } }"#;
    let code = Compiler::new().compile_source(src).expect("compile");
    let (_, off) = parse_meta_offset(&code).unwrap();
    let code_region = &code[off..];
    let want = encoding::wide::encode_syscallx(syscalls::SYSCALL_SYSVAR_BLOCK_HEIGHT).to_le_bytes();
    assert!(
        code_region.windows(want.len()).any(|window| window == want),
        "SYSVAR_BLOCK_HEIGHT syscall not found"
    );
}

#[test]
fn compile_emits_extended_sysvar_helpers() {
    let src = r#"
        seiyaku Context {
          view fn f() -> (int, bytes, bytes, bytes) {
            let block_time = context::block_time_ms();
            let chain = context::chain_id();
            let contract_address = context::seiyaku_address();
            let name = context::kotoage();
            return (block_time, chain, contract_address, name);
          }
        }
    "#;
    let code = Compiler::new().compile_source(src).expect("compile");
    let (_, off) = parse_meta_offset(&code).unwrap();
    let code_region = &code[off..];
    for (name, syscall) in [
        (
            "SYSVAR_BLOCK_TIME_MS",
            syscalls::SYSCALL_SYSVAR_BLOCK_TIME_MS,
        ),
        ("SYSVAR_CHAIN_ID", syscalls::SYSCALL_SYSVAR_CHAIN_ID),
        (
            "SYSVAR_CONTRACT_ADDRESS",
            syscalls::SYSCALL_SYSVAR_CONTRACT_ADDRESS,
        ),
        ("SYSVAR_ENTRYPOINT", syscalls::SYSCALL_SYSVAR_ENTRYPOINT),
    ] {
        let want = encoding::wide::encode_syscallx(syscall).to_le_bytes();
        assert!(
            code_region.windows(want.len()).any(|window| window == want),
            "{name} syscall not found"
        );
    }
}

#[test]
fn semantic_rejects_extended_sysvar_helper_args() {
    let prog = parse(r#"module InvalidContext { fn f() { let _chain = context::chain_id(1); } }"#)
        .unwrap();
    let err = analyze(&prog).expect_err("expected sysvar arity error");
    assert!(
        err.message().contains("chain_id expects no arguments"),
        "unexpected error: {}",
        err.message()
    );
}

#[test]
fn raw_query_and_authority_sysvar_helpers_are_not_source_apis() {
    let src = r#"
        seiyaku RawQuery {
          view fn query(bytes payload) -> bytes {
            return query_execute_norito(payload);
          }
        }
    "#;
    let error = test_compiler()
        .compile_source(src)
        .expect_err("raw query bridge must be rejected");
    assert!(error.contains("query_execute_norito"));

    let raw_authority =
        r#"seiyaku RawAuthority { view fn caller() -> AccountId { return sysvar_authority(); } }"#;
    let error = test_compiler()
        .compile_source(raw_authority)
        .expect_err("direct sysvar helper must be rejected");
    assert!(error.contains("sysvar_authority"));
}

#[test]
fn semantic_rejects_extended_query_and_authority_sysvar_helper_args() {
    let prog =
        parse(r#"module InvalidQuery { fn f() { let _response = query_execute_norito(1); } }"#)
            .unwrap();
    let err = analyze(&prog).expect_err("expected query payload type error");
    assert!(
        err.message().contains("query_execute_norito"),
        "unexpected error: {}",
        err.message()
    );

    let prog =
        parse(r#"module InvalidAuthority { fn f() { let _caller = sysvar_authority(1); } }"#)
            .unwrap();
    let err = analyze(&prog).expect_err("expected sysvar arity error");
    assert!(
        err.message().contains("sysvar_authority"),
        "unexpected error: {}",
        err.message()
    );
}

#[test]
fn compile_emits_core_query_get_helpers() {
    let src = r#"
      seiyaku Queries {
        view fn read() -> bytes {
            let account = ledger::query::account(context::authority());
            let definition = ledger::query::asset_definition(AssetDefinitionId::parse("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"));
            let domain = ledger::query::domain(DomainId::parse("wonderland.universal"));
            let nft = ledger::query::nft(NftId::parse("n0$wonderland.universal"));
            let parameter = ledger::query::parameter(Name::parse("block.max_transactions"));
            let manifest = ledger::query::seiyaku_manifest(b"hash");
            let instance = ledger::query::seiyaku_instance(Name::parse("router::universal"));
            return instance;
        }
      }
    "#;
    let code = test_compiler().compile_source(src).expect("compile");
    let (_, off) = parse_meta_offset(&code).unwrap();
    let code_region = &code[off..];
    for (name, syscall) in [
        ("CORE_QUERY_GET(account)", syscalls::SYSCALL_CORE_QUERY_GET),
        (
            "CORE_QUERY_GET(asset_definition)",
            syscalls::SYSCALL_CORE_QUERY_GET,
        ),
        ("CORE_QUERY_GET(domain)", syscalls::SYSCALL_CORE_QUERY_GET),
        ("CORE_QUERY_GET(nft)", syscalls::SYSCALL_CORE_QUERY_GET),
        ("QUERY_GET_PARAMETER", syscalls::SYSCALL_QUERY_GET_PARAMETER),
        (
            "QUERY_GET_CONTRACT_MANIFEST",
            syscalls::SYSCALL_QUERY_GET_CONTRACT_MANIFEST,
        ),
        (
            "QUERY_GET_CONTRACT_INSTANCE",
            syscalls::SYSCALL_QUERY_GET_CONTRACT_INSTANCE,
        ),
    ] {
        let want = encoding::wide::encode_syscallx(syscall).to_le_bytes();
        assert!(
            code_region.windows(want.len()).any(|window| window == want),
            "{name} syscall not found"
        );
    }
}

#[test]
fn manifest_includes_exact_access_hints_for_static_typed_query_get_helpers() {
    use iroha_data_model::{
        account::{AccountId, ParsedAccountId},
        asset::id::{AssetDefinitionId, AssetId},
    };

    let account = AccountId::parse_encoded("sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV")
        .map(ParsedAccountId::into_account_id)
        .expect("parse account literal");
    let asset_definition = AssetDefinitionId::parse_address_literal("62Fk4FPcMuLvW5QjDGNF2a4jAmjM")
        .expect("parse asset definition");
    let asset = AssetId::of(asset_definition.clone(), account.clone());
    let src = format!(
        r#"
        seiyaku QueryHints {{
          view fn read() -> Option<NftView> {{
            let account = ledger::query::account(context::authority());
            let asset = ledger::query::asset(AssetId::parse("{asset}"));
            let definition = ledger::query::asset_definition(AssetDefinitionId::parse("{asset_definition}"));
            let domain = ledger::query::domain(DomainId::parse("wonderland.universal"));
            let nft = ledger::query::nft(NftId::parse("n0$wonderland.universal"));
            return nft;
          }}
        }}
    "#
    );
    let (_code, manifest) = Compiler::new()
        .compile_source_with_manifest(&src)
        .expect("compile manifest with typed query hints");
    let entrypoints = manifest.entrypoints.expect("entrypoints must be present");
    let read = entrypoints
        .iter()
        .find(|entry| entry.name == "read")
        .expect("read entrypoint");
    assert_eq!(read.access_hints_complete, Some(true));
    assert!(read.access_hints_skipped.is_empty());
    assert!(read.write_keys.is_empty());
    assert!(read.read_keys.contains(&"account:$authority".to_string()));
    assert!(read.read_keys.contains(&format!("account:{account}")));
    assert!(read.read_keys.contains(&format!("asset:{asset}")));
    assert!(
        read.read_keys
            .contains(&format!("asset_def:{asset_definition}"))
    );
    assert!(
        read.read_keys
            .contains(&"domain:wonderland.universal".to_string())
    );
    assert!(read.read_keys.contains(&"nft".to_string()));
    assert!(
        read.read_keys
            .contains(&"nft:n0$wonderland.universal".to_string())
    );
    assert!(!read.read_keys.contains(&"*".to_string()));
}

#[test]
fn semantic_rejects_typed_query_get_helper_args() {
    let prog =
        parse(r#"module InvalidQuery { fn f() { let _account = ledger::query::account(1); } }"#)
            .unwrap();
    let err = analyze(&prog).expect_err("expected account query key type error");
    assert!(
        err.message().contains("ledger::query::account"),
        "unexpected error: {}",
        err.message()
    );

    let prog = parse(
        r#"module InvalidQuery { fn f() { let _instance = ledger::query::seiyaku_instance(1); } }"#,
    )
    .unwrap();
    let err = analyze(&prog).expect_err("expected contract instance query key type error");
    assert!(
        err.message().contains("ledger::query::seiyaku_instance"),
        "unexpected error: {}",
        err.message()
    );
}

#[test]
fn compile_emits_zk_vrf_read_helpers() {
    let src = r#"
        seiyaku ReadHelpers {
          view fn read() -> bytes {
            let roots = crypto::zk::roots(b"roots");
            let tally = ledger::governance::tally(b"tally");
            let seed = crypto::vrf::epoch_seed(b"seed");
            return seed;
          }
        }
    "#;
    let code = test_compiler().compile_source(src).expect("compile");
    let (_, off) = parse_meta_offset(&code).unwrap();
    let code_region = &code[off..];
    for (name, syscall) in [
        ("ZK_ROOTS_GET", syscalls::SYSCALL_ZK_ROOTS_GET),
        ("ZK_VOTE_GET_TALLY", syscalls::SYSCALL_ZK_VOTE_GET_TALLY),
        ("VRF_EPOCH_SEED", syscalls::SYSCALL_VRF_EPOCH_SEED),
    ] {
        let want = encoding::wide::encode_sys(instruction::wide::system::SCALL, syscall as u8)
            .to_le_bytes();
        assert!(
            code_region.windows(want.len()).any(|window| window == want),
            "{name} syscall not found"
        );
    }
}

#[test]
fn manifest_includes_exact_access_hints_for_static_zk_read_requests() {
    use iroha_data_model::asset::id::AssetDefinitionId;
    use ivm::zk_verify::{RootsGetRequest, VoteGetTallyRequest};

    let asset_definition = AssetDefinitionId::parse_address_literal("62Fk4FPcMuLvW5QjDGNF2a4jAmjM")
        .expect("parse asset definition");
    let roots_payload = norito::to_bytes(&RootsGetRequest {
        asset_id: asset_definition.to_string(),
        max: 4,
    })
    .expect("encode roots request");
    let tally_payload = norito::to_bytes(&VoteGetTallyRequest {
        election_id: "election-1".to_string(),
    })
    .expect("encode tally request");
    let roots_literal = roots_payload
        .iter()
        .map(|byte| format!("\\x{byte:02x}"))
        .collect::<String>();
    let tally_literal = tally_payload
        .iter()
        .map(|byte| format!("\\x{byte:02x}"))
        .collect::<String>();
    let src = format!(
        r#"
        seiyaku StaticReadHints {{
          view fn read() -> bytes {{
            let roots = crypto::zk::roots(b"{}");
            let tally = ledger::governance::tally(b"{}");
            return tally;
          }}
        }}
    "#,
        roots_literal, tally_literal
    );
    let (_code, manifest) = Compiler::new()
        .compile_source_with_manifest(&src)
        .expect("compile manifest with ZK read hints");
    let entrypoints = manifest.entrypoints.expect("entrypoints must be present");
    let read = entrypoints
        .iter()
        .find(|entry| entry.name == "read")
        .expect("read entrypoint");
    assert_eq!(read.access_hints_complete, Some(true));
    assert!(read.access_hints_skipped.is_empty());
    assert!(read.write_keys.is_empty());
    assert!(
        read.read_keys
            .contains(&format!("zk_asset:{asset_definition}"))
    );
    assert!(
        read.read_keys
            .contains(&"zk:election:election-1:tally".to_string())
    );
    assert!(!read.read_keys.contains(&"*".to_string()));
}

#[test]
fn semantic_rejects_zk_vrf_read_helper_args() {
    let prog =
        parse(r#"module InvalidVrfRequest { fn f() { let _seed = crypto::vrf::epoch_seed(1); } }"#)
            .unwrap();
    let err = analyze(&prog).expect_err("expected vrf seed payload type error");
    assert!(
        err.message().contains(
            "crypto::vrf::epoch_seed expects (bytes) pointer to NoritoBytes VrfEpochSeedRequest"
        ),
        "unexpected error: {}",
        err.message()
    );
}

#[test]
fn compile_emits_state_introspection_helpers() {
    let src = r#"
        seiyaku StateIntrospection {
        view fn f() -> (bytes, bool, int, int) {
            let prefix = Name::parse("Orders");
            let keys = state::keys(path: prefix, offset: 0, limit: 2);
            let present = state::contains(prefix);
            let len = state::len(prefix);
            let count = state::count(prefix);
            return (keys, present, len, count);
        }
        }
    "#;
    let code = Compiler::new().compile_source(src).expect("compile");
    let (_, off) = parse_meta_offset(&code).unwrap();
    let code_region = &code[off..];
    for (name, syscall) in [
        ("STATE_KEYS", syscalls::SYSCALL_STATE_KEYS),
        ("STATE_HAS", syscalls::SYSCALL_STATE_HAS),
        ("STATE_LEN", syscalls::SYSCALL_STATE_LEN),
        ("STATE_COUNT", syscalls::SYSCALL_STATE_COUNT),
    ] {
        let want = encoding::wide::encode_syscallx(syscall).to_le_bytes();
        assert!(
            code_region.windows(want.len()).any(|window| window == want),
            "{name} syscall not found"
        );
    }
}

#[test]
fn semantic_rejects_state_introspection_helper_args() {
    let prog = parse(
        r#"seiyaku C { fn f() { let _keys = state::keys(path: Name::parse("Orders"), offset: 0, limit: b"bad"); } }"#,
    )
    .unwrap();
    let err = analyze(&prog).expect_err("expected state_keys type error");
    assert!(
        err.message()
            .contains("state::keys expects (Name, int offset, int limit)"),
        "unexpected error: {}",
        err.message()
    );
}

#[test]
fn compile_emits_extended_hash_syscalls() {
    let src = r#"
        seiyaku HashFunctions {
        view fn f(bytes payload) -> Json {
            let b = crypto::blake2b256(payload);
            let k = crypto::keccak256(payload);
            let i = crypto::iroha_hash(payload);
            return json { blake2b: b, keccak: k, iroha: i };
        }
        }
    "#;
    let code = Compiler::new().compile_source(src).expect("compile");
    let (_, off) = parse_meta_offset(&code).unwrap();
    let mut words = Vec::new();
    let mut i = off;
    while i + 4 <= code.len() {
        words.push(u32::from_le_bytes(code[i..i + 4].try_into().unwrap()));
        i += 4;
    }
    let scall = instruction::wide::system::SCALL;
    for (name, syscall) in [
        ("BLAKE2B256_HASH", syscalls::SYSCALL_BLAKE2B256_HASH),
        ("KECCAK256_HASH", syscalls::SYSCALL_KECCAK256_HASH),
        ("IROHA_HASH", syscalls::SYSCALL_IROHA_HASH),
    ] {
        let want = encoding::wide::encode_sys(scall, syscall as u8);
        assert!(words.contains(&want), "{name} syscall not found");
    }
}

#[test]
fn semantic_rejects_extended_hash_non_bytes_arg() {
    let prog =
        parse(r#"module InvalidHash { fn f() { let digest = crypto::keccak256(1); } }"#).unwrap();
    let err = analyze(&prog).expect_err("expected type error");
    assert!(
        err.message().contains("crypto::keccak256 expects (bytes)"),
        "unexpected error: {}",
        err.message()
    );
}

#[test]
fn compile_emits_resolve_account_alias_syscall() {
    let src = r#"seiyaku ResolveAlias { view fn f() { let a = ledger::account::resolve_alias("banking@centralbank"); } }"#;
    let code = Compiler::new().compile_source(src).expect("compile");
    let (_, off) = parse_meta_offset(&code).unwrap();
    let mut words = Vec::new();
    let mut i = off;
    while i + 4 <= code.len() {
        words.push(u32::from_le_bytes(code[i..i + 4].try_into().unwrap()));
        i += 4;
    }
    let scall = instruction::wide::system::SCALL;
    let want = encoding::wide::encode_sys(scall, syscalls::SYSCALL_RESOLVE_ACCOUNT_ALIAS as u8);
    assert!(
        words.contains(&want),
        "RESOLVE_ACCOUNT_ALIAS syscall not found"
    );
}

#[test]
fn parse_and_type_bounded_map_take_one() {
    let src = r#"
        seiyaku BoundedStateIteration {
            state StateMap<int, int> values;
            fn f() { for (key, value) in values.take(1) { let seen = key; } }
        }
    "#;
    let prog = parse(src).expect("parse");
    let typed = analyze(&prog).expect("analyze");
    let ivm::kotodama::semantic::TypedItem::Function(func) = &typed.items[0];
    assert!(!func.body.statements.is_empty());
}

#[test]
fn for_each_map_mutation_is_rejected() {
    // Mutation of the iterated map inside the loop must be rejected.
    let src = r#"
        seiyaku MutatingStateIteration {
            state StateMap<int, int> values;
            fn f() { for (key, value) in values.take(1) { values[0] = 1; } }
        }
    "#;
    let prog = parse(src).expect("parse");
    let err = analyze(&prog).expect_err("should reject mutation during iteration");
    assert_eq!(err.code(), "E_ITER_MUTATION");
}

#[test]
fn parse_error() {
    let src = "module Broken { fn bad("; // incomplete
    let err = parse(src).unwrap_err();
    assert!(err.contains("identifier"));
    assert!(err.contains("module Broken { fn bad("));
}

#[test]
fn semantic_simple_add() {
    let src = "module Arithmetic { fn add(int a, int b) { let c = a + b; } }";
    let prog = parse(src).expect("parse failed");
    let typed = analyze(&prog).expect("semantic analysis failed");
    let ivm::kotodama::semantic::TypedItem::Function(func) = &typed.items[0];
    if let ivm::kotodama::semantic::TypedStatement::Let { name, value } = &func.body.statements[0] {
        assert_eq!(name, "c");
        assert_eq!(value.ty, Type::Int);
    } else {
        panic!("expected let statement");
    }
}

#[test]
fn semantic_type_error() {
    let src = "module InvalidArithmetic { fn bad() { let a = 1 + \"hi\"; } }";
    let prog = parse(src).expect("parse failed");
    let err = analyze(&prog).unwrap_err();
    assert!(
        err.message()
            .contains("operator Add is not defined for int and string"),
        "{}",
        err.message()
    );
}

#[test]
fn encode_helpers() {
    use ivm::kotodama::compiler::{encode_add, encode_addi};

    let add = encode_add(3, 1, 2);
    assert_eq!(add, 0x0103_0102);

    let addi = encode_addi(1, 1, 7).expect("encode addi");
    assert_eq!(addi, 0x2001_0107);
}

#[test]
fn compile_and_run_add() {
    let src = "seiyaku Add { fn add(int a, int b) -> int { return a + b; } view fn main() -> int { return add(a: 4, b: 7); } }";
    let compiler = Compiler::new();
    let code = compiler.compile_source(src).expect("compile failed");
    let (meta, off) = parse_meta_offset(&code).unwrap();
    assert_eq!(meta.mode, 0);
    assert_eq!(meta.version_minor, 1);
    assert!(
        off > 17,
        "self-describing artifacts must prefix code with CNTR"
    );

    let mut vm = ivm::IVM::new(u64::MAX);
    // Decode trace left disabled by default; first-words dump is printed above.
    vm.load_program(&code).unwrap();
    common::select_kotodama_entrypoint(&mut vm, &code, "main");
    vm.run().expect("execution failed");
    assert_eq!(common::decode_i64_register(&vm, 10), 11);
}

#[test]
fn state_allocations_do_not_clobber_params() {
    let src = r#"
        seiyaku StateAllocation {
            state StateMap<int, int> values;
            fn id(int x) -> int { return x; }
            view fn main() -> int { return id(42); }
        }
    "#;
    let code = Compiler::new().compile_source(src).expect("compile failed");
    let mut vm = ivm::IVM::new(u64::MAX);
    vm.load_program(&code).unwrap();
    common::select_kotodama_entrypoint(&mut vm, &code, "main");
    vm.run().expect("execution failed");
    assert_eq!(common::decode_i64_register(&vm, 10), 42);
}

#[test]
fn compile_builtin_create_nfts_and_set_detail() {
    let src = "seiyaku CanonicalHostCalls { kotoage fn main() authorize(\"Admin\") { ledger::nft::create_for_all_users(); ledger::account::set_detail(account: context::authority(), key: Name::parse(\"cursor\"), value: Json::parse(\"{\\\"query\\\":\\\"sc_dummy\\\",\\\"cursor\\\":1}\")); } }";
    let code = test_compiler().compile_source(src).expect("compile failed");
    // Sanity: code contains at least three syscalls (order preserved)
    // Byte-pattern search for SCALL encodings (LE): [imm8, 0x00, 0x00, 0x60]
    let (_meta, off) = parse_meta_offset(&code).unwrap();
    let code_bytes = &code[off..];
    // Expect SCALL encodings for our helper syscalls present
    let scall = instruction::wide::system::SCALL;
    let _want = [
        encoding::wide::encode_sys(scall, syscalls::SYSCALL_CREATE_NFTS_FOR_ALL_USERS as u8),
        encoding::wide::encode_sys(scall, syscalls::SYSCALL_SET_ACCOUNT_DETAIL as u8),
    ];
    let imm_create = syscalls::SYSCALL_CREATE_NFTS_FOR_ALL_USERS as u8;
    let imm_detail = syscalls::SYSCALL_SET_ACCOUNT_DETAIL as u8;
    let pat = |imm: u8| [imm, 0x00, 0x00, instruction::wide::system::SCALL];
    let has = |imm: u8| code_bytes.windows(4).any(|w| w == pat(imm));
    assert!(has(imm_create) && has(imm_detail));
}

#[test]
fn statement_call_sugar_is_rejected() {
    for src in [
        "seiyaku C { fn f() { call helper(); } fn helper() {} }",
        "seiyaku C { fn f() { call 1; } }",
    ] {
        let err = parse(src).expect_err("statement-level call sugar must be rejected");
        assert!(err.contains("call"), "unexpected parse error: {err}");
    }
}

#[test]
fn pointer_constructors_accept_string_variables() {
    // Use variables bound to string literals; constructors should work
    let src = r#"
        seiyaku PointerConstructors {
          kotoage fn main() authorize("PointerConstructors") {
            let did = "wonderland.universal";
            let aid = "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV";
            let key = "cursor";
            let val = "{\"query\":\"sc_dummy\",\"cursor\":1}";
            ledger::account::set_detail(account: AccountId::parse(aid), key: Name::parse(key), value: Json::parse(val));
            ledger::domain::transfer(source: context::authority(), domain: DomainId::parse(did), destination: AccountId::parse(aid));
          }
        }
    "#;
    let code = ivm::KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile pointer from vars");
    // Expect SCALLs present for set detail and transfer domain
    let (_m, off) = parse_meta_offset(&code).unwrap();
    let bytes = &code[off..];
    let scall = instruction::wide::system::SCALL;
    let has = |imm: u8| {
        bytes
            .windows(4)
            .any(|w| w == encoding::wide::encode_sys(scall, imm).to_le_bytes())
    };
    assert!(
        has(syscalls::SYSCALL_SET_ACCOUNT_DETAIL as u8)
            && has(syscalls::SYSCALL_TRANSFER_DOMAIN as u8),
        "expected set detail and transfer domain syscalls"
    );
}

#[test]
fn pointer_constructors_reject_implicit_conversions_and_method_aliases() {
    for source in [
        r#"seiyaku C { fn f(bytes value) { let _x = AccountId::parse(value); } }"#,
        r#"seiyaku C { fn f(bytes value) { let _x = Json::parse(value); } }"#,
        r#"seiyaku C { fn f(bytes value) { let _x = Name::parse(value); } }"#,
        r#"seiyaku C { fn f(Name value) { let _x = Name::parse(value); } }"#,
        r#"seiyaku C { fn f(string value) { let _x = value.account_id(); } }"#,
        r#"seiyaku C { fn f(string value) { let _x = value.name(); } }"#,
        r#"seiyaku C { fn f(string value) { let _x = value.json(); } }"#,
        r#"seiyaku C { fn f(bytes value) { let _x = value.blob(); } }"#,
        r#"seiyaku C { fn f(bytes value) { let _x = value.norito_bytes(); } }"#,
        r#"seiyaku C { fn f() { let _x = blob("raw"); } }"#,
        r#"seiyaku C { fn f() { let _x = norito_bytes("raw"); } }"#,
    ] {
        let error = Compiler::new()
            .compile_source(source)
            .expect_err("non-canonical pointer conversion must be rejected");
        assert!(
            error.contains("expects string")
                || error.contains("method aliases were removed")
                || error.contains("compiler-internal")
                || error.contains("unknown function or builtin"),
            "unexpected error: {error}"
        );
    }
}

#[test]
fn triple_nested_struct_field_access() {
    // Deeply nested struct fields: d.c.b.a.x
    let src = r#"
        seiyaku NestedStructFields {
        struct A { int x }
        struct B { A a }
        struct C { B b }
        struct D { C c }
        view fn f() -> int {
            let a = A { x: 5 };
            let b = B { a };
            let c = C { b };
            let d = D { c };
            return d.c.b.a.x;
        }
        }
    "#;
    let code = ivm::KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile triple nested access");
    let mut vm = ivm::IVM::new(u64::MAX);
    vm.load_program(&code).unwrap();
    common::select_kotodama_entrypoint(&mut vm, &code, "f");
    vm.run().expect("execute");
    assert_eq!(common::decode_i64_register(&vm, 10), 5);
}

#[test]
fn triple_nested_struct_field_mixed_named_numeric_access() {
    // Mixed access: d.c.0.a.x where D { (B, int) c }
    let src = r#"
        seiyaku MixedStructTupleFields {
        struct A { int x }
        struct B { A a }
        struct D { (B, int) c }
        view fn f() -> int {
            let a = A { x: 7 };
            let b = B { a };
            let d = D { c: (b, 99) };
            return d.c.0.a.x;
        }
        }
    "#;
    let code = ivm::KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile mixed named/numeric access");
    let mut vm = ivm::IVM::new(u64::MAX);
    vm.load_program(&code).unwrap();
    common::select_kotodama_entrypoint(&mut vm, &code, "f");
    vm.run().expect("execute");
    assert_eq!(common::decode_i64_register(&vm, 10), 7);
}

#[test]
fn invalid_numeric_on_struct_reports_error() {
    let src = r#"
        module InvalidStructIndex {
        struct A { int x }
        fn f() { let a = A { x: 1 }; let v = a.0; }
        }
    "#;
    let prog = parse(src).expect("parse ok");
    let err = analyze(&prog).expect_err("expected error");
    assert!(err.message().contains("unknown field '0' on struct A"));
}

#[test]
fn invalid_named_on_tuple_reports_error() {
    let src = r#"
        module InvalidTupleField {
        fn f() { let t = (1,2); let v = t.a; }
        }
    "#;
    let prog = parse(src).expect("parse ok");
    let err = analyze(&prog).expect_err("expected error");
    assert!(err.message().contains("unknown field 'a' on tuple"));
}

#[test]
fn invalid_numeric_tuple_index_reports_error() {
    let src = r#"
        module InvalidTupleIndex {
        fn f() { let t = (1,2); let v = t.3; }
        }
    "#;
    let prog = parse(src).expect("parse ok");
    let err = analyze(&prog).expect_err("expected error");
    assert!(err.message().contains("tuple index 3 out of bounds"));
}

#[test]
fn tuple_index_on_non_tuple_reports_type() {
    let src = r#"
        module InvalidStructTupleIndex {
        struct A { int x }
        fn f() { let s = A { x: 1 }; let v = s.0; }
        }
    "#;
    let prog = parse(src).expect("parse ok");
    let err = analyze(&prog).expect_err("expected error");
    assert!(
        err.message()
            .contains("tuple index on non-tuple type struct A")
    );
}

#[test]
fn tuple_index_on_non_tuple_int_reports_type() {
    let src = r#"
        module InvalidScalarTupleIndex {
        fn f() { let n = 1; let v = n.0; }
        }
    "#;
    let prog = parse(src).expect("parse ok");
    let err = analyze(&prog).expect_err("expected error");
    assert!(err.message().contains("tuple index on non-tuple type int"));
}

#[test]
fn unknown_field_on_struct_reports_available_fields() {
    let src = r#"
        module UnknownStructField {
        struct A { int x, int y }
        fn f() { let a = A { x: 1, y: 2 }; let v = a.z; }
        }
    "#;
    let prog = parse(src).expect("parse ok");
    let err = analyze(&prog).expect_err("expected error");
    assert!(
        err.message()
            .contains("unknown field 'z' on struct A (available: x, y)")
    );
}

#[test]
fn invalid_named_on_non_struct_reports_error() {
    let src = r#"
        module InvalidScalarField {
        fn f() { let n = 1; let v = n.foo; }
        }
    "#;
    let prog = parse(src).expect("parse ok");
    let err = analyze(&prog).expect_err("expected error");
    assert!(err.message().contains("unknown field 'foo' on type int"));
}

#[test]
fn invalid_indexing_on_non_map_reports_error() {
    let src = r#"
        module InvalidStructIndexing {
        struct A { int x }
        fn f() { let a = A { x: 1 }; let v = a[0]; }
        }
    "#;
    let prog = parse(src).expect("parse ok");
    let err = analyze(&prog).expect_err("expected error");
    assert!(
        err.message()
            .contains("indexing not supported on this type")
    );
}

#[test]
fn method_call_sugar_receiver_and_arg() {
    // a.method(b) sugar: receiver prepended as first arg
    let src = r#"
        seiyaku MethodCallSugar {
        fn add(int x, int y) -> int { return x + y; }
        view fn main() -> int { return (5).add(7); }
        }
    "#;
    let code = ivm::KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile method sugar");
    let mut vm = ivm::IVM::new(u64::MAX);
    vm.load_program(&code).unwrap();
    common::select_kotodama_entrypoint(&mut vm, &code, "main");
    vm.run().expect("execute");
    assert_eq!(common::decode_i64_register(&vm, 10), 12);
}

#[test]
fn semantic_type_enforcement_for_typed_syscalls() {
    use ivm::kotodama::parser::parse;
    // Wrong types should fail
    let bad = parse(
        "module InvalidMint { fn f() { ledger::asset::mint(account: Name::parse(\"x\"), asset_definition: AssetDefinitionId::parse(\"62Fk4FPcMuLvW5QjDGNF2a4jAmjM\"), amount: 1); } }",
    )
    .unwrap();
    assert!(analyze(&bad).is_err());
    let bad2 = parse("module InvalidDetail { fn f() { ledger::account::set_detail(account: AccountId::parse(\"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV\"), key: Json::parse(\"1\"), value: Name::parse(\"k\")); } }").unwrap();
    assert!(analyze(&bad2).is_err());
}

#[test]
fn range_end_less_than_start_rejected() {
    let src = r#"
        seiyaku InvalidStateRange {
            state StateMap<int, int> values;
            fn f() { for (key, value) in values.range(5, 2) { let seen = value; } }
        }
    "#;
    let prog = parse(src).expect("parse");
    let err = analyze(&prog).expect_err("expected end<start rejection");
    assert!(err.message().contains("end >= start"));
}

#[test]
fn range_non_integer_args_rejected() {
    let src = r#"
        seiyaku InvalidStateRangeTypes {
            state StateMap<int, int> values;
            fn f() { for (key, value) in values.range("a", "b") { let seen = value; } }
        }
    "#;
    let prog = parse(src).expect("parse");
    let err = analyze(&prog).expect_err("expected non-integer rejection");
    assert!(err.message().contains("range(start, end)"));
}

#[test]
fn dynamic_state_map_take_is_rejected() {
    let src = r#"
        seiyaku DynamicTake {
        state StateMap<int, int> values;
        fn bounded_take_sum(int n) -> int {
            var acc = 0;
            for (key, value) in values.take(n) { acc = acc + value; }
            return acc;
        }
        }
    "#;
    let err = ivm::KotodamaCompiler::new()
        .compile_source(src)
        .expect_err("dynamic state-map bounds must be rejected");
    assert!(
        err.contains("E_UNBOUNDED_ITERATION") && err.contains("literal"),
        "unexpected error: {err}"
    );
}

#[test]
fn dynamic_state_map_range_is_rejected() {
    let src = r#"
        seiyaku DynamicRange {
        state StateMap<int, int> values;
        fn bounded_range_sum(int start, int end) -> int {
            var acc = 0;
            for (key, value) in values.range(start, end) { acc = acc + value; }
            return acc;
        }
        }
    "#;
    let err = ivm::KotodamaCompiler::new()
        .compile_source(src)
        .expect_err("dynamic state-map bounds must be rejected");
    assert!(
        err.contains("E_UNBOUNDED_ITERATION") && err.contains("literal"),
        "unexpected error: {err}"
    );
}

#[test]
fn compile_typed_nft_syscalls() {
    let src = "seiyaku NftCalls { kotoage fn main() authorize(\"ManageNfts\") { ledger::nft::mint(nft: NftId::parse(\"n0$wonderland.universal\"), owner: AccountId::parse(\"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV\")); ledger::nft::transfer(source: AccountId::parse(\"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV\"), nft: NftId::parse(\"n0$wonderland.universal\"), destination: AccountId::parse(\"sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76\")); } }";
    let code = Compiler::new()
        .compile_source(src)
        .expect("compile typed NFT");
    let (_meta, off) = parse_meta_offset(&code).unwrap();
    let code_bytes = &code[off..];
    let pat = |imm: u8| [imm, 0x00, 0x00, instruction::wide::system::SCALL];
    let has = |imm: u8| code_bytes.windows(4).any(|w| w == pat(imm));
    assert!(has(syscalls::SYSCALL_NFT_MINT_ASSET as u8));
    assert!(has(syscalls::SYSCALL_NFT_TRANSFER_ASSET as u8));
}

#[test]
fn compile_and_run_modulo() {
    // Return a % b
    let src = "seiyaku Modulo { view fn main() -> int { return 17 % 5; } }";
    let code = Compiler::new().compile_source(src).expect("compile modulo");
    let mut vm = ivm::IVM::new(u64::MAX);
    vm.load_program(&code).unwrap();
    common::select_kotodama_entrypoint(&mut vm, &code, "main");
    vm.run().expect("execute");
    assert_eq!(common::decode_i64_register(&vm, 10), 17 % 5);
}

#[test]
fn compiler_owns_first_release_abi_metadata() {
    let code = Compiler::new()
        .compile_source("seiyaku FixedAbi { view fn f() -> int { return 3; } }")
        .expect("compile");
    let (meta, _off) = parse_meta_offset(&code).unwrap();
    assert_eq!(meta.abi_version, 1);
    assert_eq!(meta.vector_length, 0);
}

#[test]
fn compile_emits_manifest_hashes() {
    use ivm::{SyscallPolicy, syscalls::compute_abi_hash};
    let src = "seiyaku ManifestHash { view fn f() { let x = 1 + 2; } }";
    let (code, manifest) = Compiler::new()
        .compile_source_with_manifest(src)
        .expect("compile with manifest");
    let parsed = ProgramMetadata::parse(&code).expect("parse header");
    let meta = parsed.metadata;
    let expected_code_hash = ivm::contract_code_hash(&code);
    assert_eq!(manifest.code_hash, Some(expected_code_hash));
    let policy = match meta.abi_version {
        1 => SyscallPolicy::AbiV1,
        _ => unreachable!("compiler emits ABI v1 only"),
    };
    let expected_abi = iroha_crypto::Hash::prehashed(compute_abi_hash(policy));
    assert_eq!(manifest.abi_hash, Some(expected_abi));
}

#[test]
fn manifest_code_hash_reflects_literals() {
    let compiler = Compiler::new();
    let (_, manifest_a) = compiler
        .compile_source_with_manifest(
            "seiyaku LiteralAlpha { view fn f() -> string { return \"alpha\"; } }",
        )
        .expect("compile alpha");
    let (_, manifest_b) = compiler
        .compile_source_with_manifest(
            "seiyaku LiteralBeta { view fn f() -> string { return \"beta\"; } }",
        )
        .expect("compile beta");
    let hash_a = manifest_a.code_hash.expect("alpha code hash");
    let hash_b = manifest_b.code_hash.expect("beta code hash");
    assert_ne!(hash_a, hash_b, "literals must influence code_hash");
}

#[test]
fn manifest_includes_entrypoints_and_features() {
    use iroha_data_model::smart_contract::manifest::EntryPointKind;

    let src = r#"
        seiyaku Demo {
            state int counter;

            hajimari() {
                counter = 0;
            }

            kotoage fn run() authorize("Admin") {
                let current = counter;
                let next = current + 1;
                if current > 0 {
                    debug::info("counter tick");
                } else {
                    debug::info("counter fresh");
                }
            }
        }
    "#;
    let (code, manifest) = Compiler::new()
        .compile_source_with_manifest(src)
        .expect("compile manifest with entrypoints");
    let parsed = ProgramMetadata::parse(&code).expect("parse compiled artifact");
    assert_eq!(parsed.metadata.version_minor, 1);
    let contract_interface = parsed
        .contract_interface
        .expect("compiled contract must embed a CNTR section");
    assert_eq!(contract_interface.entrypoints.len(), 2);
    assert_eq!(contract_interface.entrypoints[0].name, "hajimari");
    assert_eq!(contract_interface.entrypoints[1].name, "run");
    let entrypoints = manifest.entrypoints.expect("entrypoints must be present");
    assert_eq!(entrypoints.len(), 2);
    assert_eq!(entrypoints[0].name, "hajimari");
    assert!(matches!(entrypoints[0].kind, EntryPointKind::Hajimari));
    assert_eq!(entrypoints[0].permission, None);
    assert_eq!(entrypoints[0].read_keys, Vec::<String>::new());
    assert_eq!(entrypoints[0].write_keys, vec!["state:counter"]);

    assert_eq!(entrypoints[1].name, "run");
    assert!(matches!(entrypoints[1].kind, EntryPointKind::Kotoage));
    assert_eq!(entrypoints[1].permission.as_deref(), Some("Admin"));
    assert_eq!(entrypoints[1].read_keys, vec!["state:counter"]);
    assert_eq!(entrypoints[1].write_keys, Vec::<String>::new());
    assert_eq!(manifest.features_bitmap, Some(0));
}

#[test]
fn manifest_includes_trigger_descriptors() {
    use iroha_data_model::{events::EventFilterBox, trigger::action::Repeats};

    let src = r#"
        seiyaku Demo {
            kotoage fn run() authorize("Trigger") {}

            trigger wake -> run {
                on time pre_commit;
                repeats 2;
                metadata { tag: "alpha"; }
            }
        }
    "#;
    let (_code, manifest) = Compiler::new()
        .compile_source_with_manifest(src)
        .expect("compile manifest with triggers");
    let entrypoints = manifest.entrypoints.expect("entrypoints must be present");
    let run = entrypoints
        .iter()
        .find(|entry| entry.name == "run")
        .expect("run entrypoint");
    assert_eq!(run.triggers.len(), 1);
    let trigger = &run.triggers[0];
    assert_eq!(trigger.id.to_string(), "wake");
    assert!(matches!(trigger.filter, EventFilterBox::Time(_)));
    assert_eq!(trigger.repeats, Repeats::Exactly(2));
    assert_eq!(trigger.callback.entrypoint, "run");
}

#[test]
fn manifest_includes_isi_access_hints_for_static_targets() {
    use iroha_data_model::{
        account::AccountId,
        asset::id::{AssetDefinitionId, AssetId},
    };

    let asset_literal = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";
    let src = r#"
        seiyaku StaticAssetAccess {
        kotoage fn main() authorize("MintAndBurn") {
            let acc = AccountId::parse("sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV");
            let asset = AssetDefinitionId::parse("62Fk4FPcMuLvW5QjDGNF2a4jAmjM");
            ledger::asset::mint(account: acc, asset_definition: asset, amount: 1);
            ledger::asset::burn(account: acc, asset_definition: asset, amount: 1);
        }
        }
    "#;
    let (_code, manifest) = Compiler::new()
        .compile_source_with_manifest(src)
        .expect("compile manifest with ISI hints");
    let hints = manifest
        .access_set_hints
        .expect("access_set_hints must be present");
    let account: AccountId =
        AccountId::parse_encoded("sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV")
            .map(iroha_data_model::account::ParsedAccountId::into_account_id)
            .expect("parse encoded account literal");
    let asset_def =
        AssetDefinitionId::parse_address_literal(asset_literal).expect("parse canonical asset");
    assert!(asset_def.is_opaque_canonical());
    let asset_id = AssetId::of(asset_def.clone(), account.clone());

    assert!(hints.read_keys.contains(&format!("account:{account}")));
    assert!(hints.read_keys.contains(&format!("asset_def:{asset_def}")));
    assert!(hints.read_keys.contains(&format!("asset:{asset_id}")));
    assert!(hints.write_keys.contains(&format!("asset_def:{asset_def}")));
    assert!(hints.write_keys.contains(&format!("asset:{asset_id}")));
    assert!(
        !hints.read_keys.iter().any(|key| key.starts_with("domain:")),
        "opaque canonical asset definitions should not synthesize domain hints",
    );

    let entrypoints = manifest.entrypoints.expect("entrypoints must be present");
    let main = entrypoints
        .iter()
        .find(|entry| entry.name == "main")
        .expect("main entrypoint");
    assert!(main.read_keys.contains(&format!("account:{account}")));
    assert!(main.read_keys.contains(&format!("asset_def:{asset_def}")));
    assert!(main.read_keys.contains(&format!("asset:{asset_id}")));
    assert!(main.write_keys.contains(&format!("asset_def:{asset_def}")));
    assert!(main.write_keys.contains(&format!("asset:{asset_id}")));
    assert!(
        !main.read_keys.iter().any(|key| key.starts_with("domain:")),
        "opaque canonical asset definitions should not synthesize domain hints",
    );
}

#[test]
fn production_manifest_accepts_authority_placeholder_isi_access() {
    let src = r#"
        seiyaku AuthorityDomainTransfer {
        kotoage fn main() authorize("TransferDomain") {
            ledger::domain::transfer(
                source: AccountId::parse("sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"),
                domain: DomainId::parse("wonderland.universal"),
                destination: context::authority(),
            );
        }
        }
    "#;
    let (_code, manifest) = Compiler::new()
        .compile_source_with_manifest(src)
        .expect("production compile must accept authority placeholder access");
    let hints = manifest
        .access_set_hints
        .expect("access_set_hints must be present");
    assert!(hints.read_keys.contains(&"account:$authority".to_string()));
    assert!(
        hints
            .read_keys
            .contains(&"domain:wonderland.universal".to_string())
    );
    assert!(
        hints
            .write_keys
            .contains(&"domain:wonderland.universal".to_string())
    );
    assert!(!hints.read_keys.contains(&"*".to_string()));
    assert!(!hints.write_keys.contains(&"*".to_string()));

    let entrypoints = manifest.entrypoints.expect("entrypoints must be present");
    let main = entrypoints
        .iter()
        .find(|entry| entry.name == "main")
        .expect("main entrypoint");
    assert_eq!(main.access_hints_complete, Some(true));
    assert!(main.read_keys.contains(&"account:$authority".to_string()));
    assert!(
        main.read_keys
            .contains(&"domain:wonderland.universal".to_string())
    );
    assert!(
        main.write_keys
            .contains(&"domain:wonderland.universal".to_string())
    );
}

#[test]
fn production_manifest_accepts_parameter_dependent_isi_access_with_coarse_hints() {
    let src = r#"
        seiyaku Test {
            kotoage fn move(AccountId from, AccountId to, AssetDefinitionId asset, quantity amount, DataSpaceId space) authorize("Admin") {
                ledger::asset::transfer(source: from, destination: to, asset_definition: asset, amount: amount, dataspace: space);
            }
        }
    "#;
    let (_code, manifest) = Compiler::new()
        .compile_source_with_manifest(src)
        .expect("parameter-dependent asset transfers have bounded coarse hints");
    let hints = manifest
        .access_set_hints
        .expect("production manifest must include access hints");
    for key in ["account:*", "asset:*", "asset_def:*"] {
        assert!(hints.read_keys.iter().any(|actual| actual == key));
    }
    for key in ["asset:*", "asset_def:*"] {
        assert!(hints.write_keys.iter().any(|actual| actual == key));
    }
    assert!(!hints.read_keys.iter().any(|key| key == "*"));
    assert!(!hints.write_keys.iter().any(|key| key == "*"));

    let entrypoints = manifest.entrypoints.expect("entrypoints must be present");
    let move_entry = entrypoints
        .iter()
        .find(|entry| entry.name == "move")
        .expect("move entrypoint");
    assert_eq!(move_entry.access_hints_complete, Some(true));
    assert!(move_entry.access_hints_skipped.is_empty());
    for key in ["account:*", "asset:*", "asset_def:*"] {
        assert!(move_entry.read_keys.iter().any(|actual| actual == key));
    }
    for key in ["asset:*", "asset_def:*"] {
        assert!(move_entry.write_keys.iter().any(|actual| actual == key));
    }
}

#[test]
fn source_localization_blocks_are_rejected() {
    for spelling in ["messages", "kotoba"] {
        let src = format!(
            r#"
            seiyaku C {{
                {spelling} {{
                    "E0001": {{ en: "Invalid assets", ja: "無効な資産" }}
                }}
                view fn main() {{}}
            }}
            "#
        );
        let error = Compiler::new()
            .compile_source_with_manifest(&src)
            .expect_err("source localization tables are not part of V1");
        assert!(error.contains("source-unit item"), "{error}");
    }
}

#[test]
fn lexer_block_comments_and_number_literals() {
    // Block comments and hex/binary/underscored numbers
    let src = r#"
        module Literals {
          fn f() {
            /* comment */
            let a = 0x10;
            let b = 0b1010;
            let c = 10_000;
          }
        }
    "#;
    let prog = parse(src).expect("parse with comments and literals");
    let _typed = analyze(&prog).expect("analyze literals");
}

#[test]
fn compound_assignments_typecheck() {
    // x +=, -=, *=, /=, %= should typecheck and rebind SSA name
    let src = r#"module Compound { fn f() { var x = 1; x += 2; x *= 3; x /= 2; x %= 2; } }"#;
    let prog = parse(src).expect("parse compound assigns");
    let _typed = analyze(&prog).expect("analyze compound assigns");
}

#[test]
fn canonical_host_calls_typecheck_and_removed_map_does_not() {
    let src = r#"
        seiyaku Transfer {
          kotoage fn f() authorize("Admin") {
            ledger::asset::transfer(source: AccountId::parse("sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"), destination: AccountId::parse("sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76"), asset_definition: AssetDefinitionId::parse("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"), amount: 1, dataspace: DataSpaceId::parse("0"));
          }
        }
    "#;
    let prog = parse(src).expect("parse ledger::asset::transfer");
    analyze(&prog).expect("analyze ledger::asset::transfer");

    let src2 = "module RemovedMap { fn make() -> int { return std::map::new(); } }";
    let prog2 = parse(src2).expect("parse std::map::new");
    analyze(&prog2).expect_err("in-memory map constructors are removed from V1");
}

#[test]
fn indirect_sensitive_calls_require_permission() {
    let src = r#"
        seiyaku Permission {
          fn helper() {
            ledger::asset::transfer(source: AccountId::parse("sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"), destination: AccountId::parse("sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76"), asset_definition: AssetDefinitionId::parse("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"), amount: 1, dataspace: DataSpaceId::parse("0"));
        }

        kotoage fn public_entry() {
            helper();
        }
        }
    "#;
    let err = Compiler::new().compile_source(src).unwrap_err();
    assert!(
        err.contains("authorize"),
        "expected authorization error, got {err}"
    );
}

#[test]
fn while_loops_are_rejected_in_v1() {
    let src = r#"
        seiyaku Counter {
            state int counter;

            hajimari() {
                counter = 0;
            }

            kotoage fn bump(int times) authorize("Admin") {
                var i = 0;
                while i < times {
                    counter = counter + 1;
                    i = i + 1;
                }
            }
        }
    "#;
    let error = Compiler::new()
        .compile_source(src)
        .expect_err("while loops must be rejected");
    assert!(error.contains("while"), "unexpected error: {error}");
}

#[test]
fn ternary_parses_and_types() {
    let src = "module Ternary { fn f(int a, int b) -> int { let x = (1 < 2) ? a : b; return x; } }";
    let prog = parse(src).expect("parse ternary");
    let typed = analyze(&prog).expect("type ternary");
    let ivm::kotodama::semantic::TypedItem::Function(f) = &typed.items[0];
    assert_eq!(f.name, "f");
}

#[test]
fn ternary_min_types() {
    let src = "module Ternary { fn choose_min(int a, int b) -> int { return (a < b) ? a : b; } }";
    let typed = analyze(&parse(src).expect("parse ternary")).expect("type ternary");
    assert!(typed.items.iter().any(|item| {
        matches!(item, ivm::kotodama::semantic::TypedItem::Function(function) if function.name == "choose_min")
    }));
}

#[test]
fn nested_ternary_types() {
    let src =
        "module Ternary { fn f(int a, int b, int c) -> int { return a < b ? b < c ? b : c : a; } }";
    analyze(&parse(src).expect("parse nested ternary")).expect("type nested ternary");
}

#[test]
fn build_options_control_header_and_source_meta_is_unavailable() {
    let src = r#"
seiyaku MyC {
  hajimari() {
    let _digest = crypto::iroha_hash(b"build-options");
    let a = 1;
  }
}
"#;
    let code = Compiler::new_with_options(ivm::kotodama::compiler::CompilerOptions {
        max_cycles: 1234,
        ..Default::default()
    })
    .compile_source(src)
    .expect("compile build-selected metadata");
    let (meta, _off) = parse_meta_offset(&code).unwrap();
    assert_eq!(meta.abi_version, 1);
    assert_eq!(meta.vector_length, 0);
    assert_eq!(meta.max_cycles, 1234);
    assert_eq!(meta.mode & ivm::ivm_mode::ZK, 0);
    assert_eq!(meta.mode & ivm::ivm_mode::VECTOR, 0);
}

#[test]
fn removed_in_memory_map_indexing_is_rejected() {
    let src = "module RemovedMap { fn f(Map<int, int> m, int k) -> int { return m[k]; } }";
    let error = Compiler::new()
        .compile_source(src)
        .expect_err("in-memory Map must not compile in V1");
    assert!(error.contains("Map"), "unexpected error: {error}");
}

#[test]
fn branch_lowering_uses_compact_conditional_and_one_relaxed_transfer() {
    let src = r#"
seiyaku Branches {
    view fn branch(bool b) -> int {
        if b {
            return 1;
        } else {
            return 2;
        }
    }
}
"#;
    let (code, _manifest, report) = Compiler::new()
        .compile_source_with_manifest_and_report(src)
        .expect("compile simple branch");
    let metadata = ProgramMetadata::parse(&code).expect("parse metadata");
    let branch = report
        .budget_report
        .iter()
        .find(|entry| entry.function_name == "__entrypoint_impl__branch")
        .expect("branch budget report");
    let words = code[metadata.code_offset + branch.pc_start as usize
        ..metadata.code_offset + branch.pc_end as usize]
        .chunks_exact(4)
        .map(|chunk| u32::from_le_bytes(chunk.try_into().expect("word chunk")))
        .collect::<Vec<_>>();

    let branch_index = words
        .iter()
        .position(|word| {
            matches!(
                instruction::wide::opcode(*word),
                instruction::wide::control::BEQ | instruction::wide::control::BNE
            )
        })
        .expect("expected compact conditional branch");

    let branch_word = words[branch_index];
    let imm = instruction::wide::imm8(branch_word);
    assert_eq!(
        imm, 2,
        "BNE should skip the one-word else transfer and land on the one-word then transfer"
    );

    let relaxed_else_transfer = words[branch_index + 1];
    let then_block_index =
        branch_index + usize::try_from(imm).expect("positive branch displacement");
    assert!(
        words.len() > then_block_index,
        "BNE target should land directly in the then block"
    );
    assert!(
        matches!(
            instruction::wide::opcode(relaxed_else_transfer),
            instruction::wide::control::JAL | instruction::wide::control::JMP
        ),
        "fallthrough path must transfer to the non-adjacent block"
    );
    assert!(
        !matches!(
            instruction::wide::opcode(words[then_block_index]),
            instruction::wide::control::JAL | instruction::wide::control::JMP
        ),
        "taken path must fall through into the adjacent block without a second jump"
    );
}

#[test]
fn compile_poseidon2_and_assert_eq() {
    // Truncated scalar proof gadgets are internal VM operations.
    let src =
        "module Poseidon { fn f(int a, int b) { let h = crypto::poseidon2(left: a, right: b); } }";
    let error = Compiler::new()
        .compile_source(src)
        .expect_err("truncated Poseidon must not be a source API");
    assert!(error.contains("crypto::poseidon2"));

    // assert_eq succeeds without enabling ZK mode
    let src = "seiyaku Assertions { view fn pass() { test::assert_eq(actual: 1, expected: 1); } view fn fail() { test::assert_eq(actual: 1, expected: 2); } }";
    let (code, _manifest, report) = test_compiler()
        .compile_source_with_manifest_and_report(src)
        .expect("compile failed");

    let (meta, _) = parse_meta_offset(&code).unwrap();
    assert_eq!(meta.mode & 0x01, 0);

    let mut vm = ivm::IVM::new(u64::MAX);
    vm.load_program(&code).unwrap();
    select_test_entrypoint(&mut vm, &code, &report, "pass");
    vm.run().expect("assert_eq failed");

    // failing case
    let mut vm2 = ivm::IVM::new(u64::MAX);
    vm2.load_program(&code).unwrap();
    select_test_entrypoint(&mut vm2, &code, &report, "fail");
    let res = vm2.run();
    assert!(matches!(res, Err(ivm::VMError::AssertionFailed)));
}

#[test]
fn compile_pubkgen_and_valcom() {
    let src = "seiyaku Commitments { view fn main() -> (int, int) { let p = crypto::pubkgen(9); let c = crypto::valcom(left: 9, right: 4); return (p, c); } }";
    let error = Compiler::new()
        .compile_source(src)
        .expect_err("toy public-key and public scalar commitment APIs must fail closed");
    assert!(error.contains("crypto::pubkgen"));
}

#[test]
fn public_scalar_valcom_is_rejected_even_without_pubkgen() {
    let src = "module Commitment { fn main(int a, int b) -> int { return crypto::valcom(left: a, right: b); } }";
    let error = Compiler::new_with_options(CompilerOptions {
        force_zk: true,
        ..CompilerOptions::default()
    })
    .compile_source(src)
    .expect_err("public valcom operands must not select the truncated opcode");
    assert!(error.contains("Secret<int|decimal|quantity>"));
}

#[test]
fn typed_json_access_spills_are_handled() {
    use ivm::kotodama::ir::Instr;
    use ivm::kotodama::regalloc;

    std::thread::Builder::new()
        .name("typed_json_access_spills".to_owned())
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            let build_src = |count: usize| {
                let mut src =
                    String::from("seiyaku JsonSpills {\nview fn main(Json j) -> int {\n");
                for i in 0..count {
                    let value = (i + 1) as i64;
                    src.push_str(&format!("  let v{i} = {value};\n"));
                }
                src.push_str("  let val = match j.get_int(Name::parse(\"value\")) { Option::some(value) => value, Option::none => 0 };\n");
                src.push_str("  let sum = ");
                for i in 0..count {
                    if i > 0 {
                        src.push_str(" + ");
                    }
                    src.push_str(&format!("v{i}"));
                }
                src.push_str(" + val;\n  return sum;\n}\n}\n");
                src
            };

            let src = build_src(32);
            let prog = parse(&src).expect("parse typed Json spill");
            let typed = analyze(&prog).expect("analyze typed Json spill");
            let ir = ivm::kotodama::ir::lower(&typed).expect("lower");
            let func = ir
                .functions
                .iter()
                .find(|f| f.name == "__entrypoint_impl__main")
                .expect("main implementation lowered");
            assert!(func.blocks.iter().any(|block| {
                block.instrs.iter().any(|instruction| {
                    matches!(
                        instruction,
                        Instr::JsonGetNumeric {
                            kind: ivm::kotodama::ir::WideNumericKind::Int,
                            ..
                        }
                    )
                })
            }));
            let alloc = regalloc::allocate(func);
            assert!(
                !alloc.stack.is_empty(),
                "fixture must compile typed Json access in a spilled stack frame"
            );
            Compiler::new()
                .compile_source(&src)
                .expect("compile typed Json spill");
        })
        .expect("spawn typed Json spill test")
        .join()
        .expect("typed Json spill test thread");
}

#[test]
fn raw_json_codec_aliases_are_rejected() {
    let src = r#"
        seiyaku RemovedCodecAliases {
            view fn main(bytes payload) -> Json {
                return decode_json(payload);
            }
        }
    "#;
    let error = Compiler::new()
        .compile_source(src)
        .expect_err("raw JSON codec aliases are not part of Kotodama V1");
    assert!(
        error.contains("decode_json") || error.contains("unknown"),
        "unexpected error: {error}"
    );
}

#[test]
fn compile_and_run_poseidon_register_forms() {
    let src = r#"
seiyaku PoseidonForms {
    view fn main() -> (int, int) {
        let pair = crypto::poseidon2(left: 123456789, right: 987654321);
        let sextet = crypto::poseidon6(a: 3, b: 5, c: 8, d: 13, e: 21, f: 34);
        return (pair, sextet);
    }
}
"#;
    let error = Compiler::new()
        .compile_source(src)
        .expect_err("truncated Poseidon register forms must remain VM-internal");
    assert!(error.contains("crypto::poseidon2") || error.contains("crypto::poseidon6"));
}

#[test]
fn unbounded_state_map_iteration_is_rejected() {
    let src = r#"
        seiyaku UnboundedStateIteration {
            state StateMap<int, int> values;
            view fn f() { for (key, value) in (values) { let seen = value; } }
        }
    "#;
    let err = Compiler::new().compile_source(src).unwrap_err();
    assert!(
        err.contains("StateMap iteration requires `.take(N)` or `.range(start, end)`"),
        "{err}"
    );
}

#[test]
fn unbounded_state_map_iteration_cannot_infer_a_limit() {
    let src = r#"
        seiyaku MissingStateIterationLimit {
            state StateMap<int, int> values;
            view fn sum() { for (key, value) in (values) { let seen = key; } }
        }
    "#;
    let err = Compiler::new().compile_source(src).unwrap_err();
    assert!(
        err.contains("StateMap iteration requires `.take(N)` or `.range(start, end)`"),
        "{err}"
    );
}

#[test]
fn map_new_is_rejected_in_v1() {
    let src = "module RemovedMap { fn make() -> int { return Map::new(); } }";
    let error = Compiler::new()
        .compile_source(src)
        .expect_err("in-memory Map allocation must not compile in V1");
    assert!(error.contains("Map"), "unexpected error: {error}");
}

#[test]
fn compile_from_file() {
    let path = std::path::Path::new("tests/data/add.ko");
    let code = Compiler::new().compile_file(path).expect("compile failed");
    assert!(!code.is_empty());
}

#[test]
fn compile_complex_program() {
    let path = std::path::Path::new("tests/data/complex.ko");
    let code = Compiler::new().compile_file(path).expect("compile failed");
    let (meta, _) = parse_meta_offset(&code).unwrap();
    assert_eq!(meta.mode & ivm::ivm_mode::ZK, 0);
}

#[test]
fn parse_control_flow() {
    let path = std::path::Path::new("tests/data/control.ko");
    let src = std::fs::read_to_string(path).expect("read failed");
    let prog = parse(&src).expect("parse failed");
    let typed = analyze(&prog).expect("semantic analysis failed");
    let ir = ivm::kotodama::ir::lower(&typed).expect("lower");
    assert!(ir.functions[0].blocks.len() > 2);
}

#[test]
fn parse_amm_dex() {
    use std::path::Path;

    use ivm::kotodama::ir::{Instr, WideNumericKind};
    let path = Path::new("tests/data/amm.ko");
    let src = std::fs::read_to_string(path).expect("read failed");
    let prog = parse(&src).expect("parse failed");
    let typed = analyze(&prog).expect("semantic analysis failed");
    let ir = ivm::kotodama::ir::lower(&typed).expect("lower");
    let mut has_mul = false;
    let mut has_div = false;
    for function in &ir.functions {
        for block in &function.blocks {
            for instruction in &block.instrs {
                match instruction {
                    Instr::NumericBinary {
                        op: BinaryOp::Mul,
                        left_kind: WideNumericKind::Int,
                        right_kind: WideNumericKind::Int,
                        result_kind: WideNumericKind::Int,
                        ..
                    } => has_mul = true,
                    Instr::NumericBinary {
                        op: BinaryOp::Div,
                        left_kind: WideNumericKind::Int,
                        right_kind: WideNumericKind::Int,
                        result_kind: WideNumericKind::Int,
                        ..
                    } => has_div = true,
                    _ => {}
                }
            }
        }
    }
    assert!(has_mul && has_div);
}

#[test]
fn parse_dai_clone() {
    use std::path::Path;

    use ivm::kotodama::ir::{Instr, WideNumericKind};
    let path = Path::new("tests/data/dai.ko");
    let src = std::fs::read_to_string(path).expect("read failed");
    let prog = parse(&src).expect("parse failed");
    let typed = analyze(&prog).expect("semantic analysis failed");
    let ir = ivm::kotodama::ir::lower(&typed).expect("lower");
    let mut has_add = false;
    let mut has_sub = false;
    for function in &ir.functions {
        for block in &function.blocks {
            for instruction in &block.instrs {
                match instruction {
                    Instr::NumericBinary {
                        op: BinaryOp::Add,
                        left_kind: WideNumericKind::Quantity,
                        right_kind: WideNumericKind::Quantity,
                        result_kind: WideNumericKind::Quantity,
                        ..
                    } => has_add = true,
                    Instr::NumericBinary {
                        op: BinaryOp::Sub,
                        left_kind: WideNumericKind::Quantity,
                        right_kind: WideNumericKind::Quantity,
                        result_kind: WideNumericKind::Quantity,
                        ..
                    } => has_sub = true,
                    _ => {}
                }
            }
        }
    }
    assert!(has_add && has_sub);
}

#[test]
fn parse_mint_asset_builtin() {
    use ivm::kotodama::ir::Instr;
    let src = "module MintHelpers { fn f(AccountId a, AssetDefinitionId b, quantity c) { ledger::asset::mint(account: a, asset_definition: b, amount: c); } }";
    let prog = parse(src).expect("parse failed");
    let typed = analyze(&prog).expect("semantic analysis failed");
    let ir = ivm::kotodama::ir::lower(&typed).expect("lower");
    let instrs = &ir.functions[0].blocks[0].instrs;
    assert!(instrs.iter().any(|i| matches!(i, Instr::MintAsset { .. })));
}

#[test]
fn parse_transfer_asset_builtin() {
    use ivm::kotodama::ir::Instr;
    let src = "module TransferHelpers { fn f(AccountId a, AccountId b, AssetDefinitionId c, quantity d, DataSpaceId e) { ledger::asset::transfer(source: a, destination: b, asset_definition: c, amount: d, dataspace: e); } }";
    let prog = parse(src).expect("parse failed");
    let typed = analyze(&prog).expect("semantic analysis failed");
    let ir = ivm::kotodama::ir::lower(&typed).expect("lower");
    let instrs = &ir.functions[0].blocks[0].instrs;
    assert!(
        instrs
            .iter()
            .any(|i| matches!(i, Instr::TransferAsset { .. }))
    );
}

#[test]
fn parse_transfer_batch_builtin() {
    use ivm::kotodama::ir::Instr;
    let src = "module BatchHelpers { fn f(AccountId a, AccountId b, AssetDefinitionId c, quantity d) { ledger::asset::transfer_batch((a, b, c, d), (b, a, c, d)); } }";
    let prog = parse(src).expect("parse failed");
    let typed = analyze(&prog).expect("semantic analysis failed");
    let ir = ivm::kotodama::ir::lower(&typed).expect("lower");
    let instrs = &ir.functions[0].blocks[0].instrs;
    assert_eq!(
        instrs
            .iter()
            .filter(|i| matches!(i, Instr::TransferBatchBegin))
            .count(),
        1,
        "one high-level transfer_batch call must open one atomic batch"
    );
    let transfer_count = instrs
        .iter()
        .filter(|i| matches!(i, Instr::TransferBatchAsset { .. }))
        .count();
    assert_eq!(
        transfer_count, 2,
        "expected two transfer calls inside batch"
    );
    assert_eq!(
        instrs
            .iter()
            .filter(|i| matches!(i, Instr::TransferBatchEnd))
            .count(),
        1,
        "one high-level transfer_batch call must close one atomic batch"
    );
}

#[test]
fn transfer_batch_requires_entries() {
    let src = "module EmptyBatch { fn f() { ledger::asset::transfer_batch(); } }";
    let prog = parse(src).expect("parse failed");
    assert!(
        analyze(&prog).is_err(),
        "an empty transfer_batch call must be rejected"
    );
}

#[test]
fn transfer_batch_requires_tuple_entries() {
    let src = "module InvalidBatch { fn f(AccountId a) { ledger::asset::transfer_batch(a); } }";
    let prog = parse(src).expect("parse failed");
    assert!(
        analyze(&prog).is_err(),
        "non-tuple transfer_batch entries must be rejected"
    );
}

#[test]
fn parse_burn_asset_builtin() {
    use ivm::kotodama::ir::Instr;
    let src = "module BurnHelpers { fn f(AccountId a, AssetDefinitionId b, quantity c) { ledger::asset::burn(account: a, asset_definition: b, amount: c); } }";
    let prog = parse(src).expect("parse failed");
    let typed = analyze(&prog).expect("semantic analysis failed");
    let ir = ivm::kotodama::ir::lower(&typed).expect("lower");
    let instrs = &ir.functions[0].blocks[0].instrs;
    assert!(instrs.iter().any(|i| matches!(i, Instr::BurnAsset { .. })));
}

#[test]
fn parse_register_asset_builtin() {
    use ivm::kotodama::ir::Instr;
    let src = r#"
        module RegisterAssetHelpers {
            fn f() {
                ledger::asset::register(asset_definition: AssetDefinitionId::parse("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"), name: "X", scale: 1, mintable: 0);
            }
        }
    "#;
    let prog = parse(src).expect("parse failed");
    let typed = analyze(&prog).expect("semantic analysis failed");
    let ir = ivm::kotodama::ir::lower(&typed).expect("lower");
    let instrs = &ir.functions[0].blocks[0].instrs;
    assert!(
        instrs
            .iter()
            .any(|i| matches!(i, Instr::RegisterAsset { .. }))
    );
}

#[test]
fn parse_create_new_asset_builtin() {
    use ivm::kotodama::ir::Instr;
    let src = r#"
        module CreateAssetHelpers {
            fn f() {
                ledger::asset::create(
                    asset_definition: AssetDefinitionId::parse("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
                    name: "X",
                    scale: 1,
                    owner: AccountId::parse("sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"),
                    mintable: 0
                );
            }
        }
    "#;
    let prog = parse(src).expect("parse failed");
    let typed = analyze(&prog).expect("semantic analysis failed");
    let ir = ivm::kotodama::ir::lower(&typed).expect("lower");
    let instrs = &ir.functions[0].blocks[0].instrs;
    assert!(
        instrs
            .iter()
            .any(|i| matches!(i, Instr::CreateNewAsset { .. }))
    );
}

#[test]
fn parse_register_asset_rejects_bare_name_literal() {
    let src = r#"
        module InvalidAssetRegistration {
            fn f() { ledger::asset::register(asset_definition: "x", name: "X", scale: 1, mintable: 0); }
        }
    "#;
    let prog = parse(src).expect("parse failed");
    let err = analyze(&prog).expect_err("bare asset names should be rejected");
    assert!(
        err.message().contains("AssetDefinitionId"),
        "unexpected semantic error: {err:?}"
    );
}

#[test]
fn parse_mfc_example() {
    use std::path::Path;

    use ivm::kotodama::ir::{Instr, Terminator};
    let path = Path::new("tests/data/mfc.ko");
    let src = std::fs::read_to_string(path).expect("read failed");
    let prog = parse(&src).expect("parse failed");
    let typed = analyze(&prog).expect("semantic analysis failed");
    let ir = ivm::kotodama::ir::lower(&typed).expect("lower");
    let mut has_transfer = false;
    let mut has_branch = false;
    for block in &ir.functions[1].blocks {
        for instr in &block.instrs {
            if matches!(instr, Instr::TransferAsset { .. }) {
                has_transfer = true;
            }
        }
        if matches!(block.terminator, Terminator::Branch { .. }) {
            has_branch = true;
        }
    }
    let mut has_mint = false;
    let mut has_register = false;
    for instr in &ir.functions[2].blocks[0].instrs {
        if matches!(instr, Instr::MintAsset { .. }) {
            has_mint = true;
        }
        if matches!(instr, Instr::RegisterAsset { .. }) {
            has_register = true;
        }
    }
    assert!(has_transfer && has_branch && has_mint && has_register);
}

#[test]
fn compile_kotodama_samples_supported() {
    use std::path::Path;
    let compiler = test_compiler();
    let samples_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../kotodama_lang/src/samples");
    // Compile a curated subset of samples supported by the current compiler
    let files = [
        "asset_ops.ko",
        "mint_rose_trigger.ko",
        "query_assets_and_save_cursor.ko",
        "smart_contract_can_filter_queries.ko",
        "threshold_escrow.ko",
    ];
    for file in files {
        let src = std::fs::read_to_string(samples_dir.join(file)).expect("read failed");
        compiler
            .compile_source(&src)
            .unwrap_or_else(|e| panic!("compile failed for {file}: {e}"));
    }
}

#[test]
fn compile_unary_ops() {
    let src = "seiyaku UnaryOps { view fn f(int a, bool b) { let c = -a; let d = !b; } }";
    Compiler::new()
        .compile_source(src)
        .expect("compile unary ops");
}

#[test]
fn in_memory_map_methods_are_rejected() {
    let src = r#"
        module RemovedMapMethods {
            fn f(Map<int, int> m) {
                let a = m.contains(1);
                let b = m.ensure(2);
            }
        }
    "#;
    let prog = parse(src).expect("parse map methods");
    let error = analyze(&prog).expect_err("in-memory Map methods must be rejected");
    assert!(
        error.message().contains("Map"),
        "unexpected error: {error:?}"
    );
}

#[test]
fn ir_lower_contains_method_state_map() {
    use ivm::kotodama::ir::Instr;
    let src = r#"
        seiyaku StateContains {
            state StateMap<int, int> m;
            view fn f(int k) -> bool { return m.contains(k); }
        }
    "#;
    let prog = parse(src).expect("parse contains");
    let typed = analyze(&prog).expect("analyze contains");
    let ir = ivm::kotodama::ir::lower(&typed).expect("lower");
    let f = &ir.functions[0];
    let mut saw_state_get = false;
    let mut saw_ne = false;
    for block in &f.blocks {
        for ins in &block.instrs {
            match ins {
                Instr::StateGet { .. } => saw_state_get = true,
                Instr::Binary { op, .. } if *op == ivm::kotodama::ast::BinaryOp::Ne => {
                    saw_ne = true;
                }
                _ => {}
            }
        }
    }
    assert!(saw_state_get && saw_ne);
}

#[test]
fn ephemeral_keys_take2_helper_is_rejected() {
    let src = r#"
        module RemovedMapIteration {
            fn g(Map<int, int> m) -> int { return keys_take2(m, 0, 1); }
        }
    "#;
    let prog = parse(src).expect("parse keys_take2");
    let error = analyze(&prog).expect_err("ephemeral map helpers must be rejected");
    assert!(
        error.message().contains("Map"),
        "unexpected error: {error:?}"
    );
}

#[test]
fn ephemeral_keys_values_take2_helper_is_rejected() {
    let src = r#"
        module RemovedMapEntries {
            fn f(Map<int, int> m) {
                let t = std::map::keys_values_take2(m, 0, 1);
                let a = t.0;
                let b = t.1;
            }
        }
    "#;
    let prog = parse(src).expect("parse keys_values_take2");
    let error = analyze(&prog).expect_err("ephemeral map helpers must be rejected");
    assert!(
        error.message().contains("Map"),
        "unexpected error: {error:?}"
    );
}

#[test]
fn ir_tuple_pack_and_get_general() {
    use ivm::kotodama::ir::Instr;
    let src = r#"
        module Tuples {
            fn f(int a, int b) {
                let t = (a, b);
                let x = t.0;
                let y = t.1;
            }
        }
    "#;
    let prog = parse(src).expect("parse tuple pack/get");
    let typed = analyze(&prog).expect("analyze tuple pack/get");
    let ir = ivm::kotodama::ir::lower(&typed).expect("lower");
    let f = &ir.functions[0];
    let mut saw_pack = false;
    let mut saw_get = false;
    for block in &f.blocks {
        for ins in &block.instrs {
            match ins {
                Instr::TuplePack { .. } => saw_pack = true,
                Instr::TupleGet { .. } => saw_get = true,
                _ => {}
            }
        }
    }
    let flattened = typed.items.iter().any(|item| {
        let ivm::kotodama::semantic::TypedItem::Function(func) = item;
        func.body.statements.iter().any(|stmt| {
            if let ivm::kotodama::semantic::TypedStatement::Let { name, value } = stmt {
                name.contains('#')
                    && matches!(value.expr, ivm::kotodama::semantic::ExprKind::Ident(_))
            } else {
                false
            }
        })
    });
    assert!(saw_pack, "tuple literals should lower to TuplePack");
    assert!(
        saw_get || flattened,
        "tuple field access should lower via TupleGet or reuse flattened bindings"
    );
}

#[test]
fn typed_vrf_syscalls_are_present() {
    let src = r#"
        seiyaku VrfVerification {
            view fn main(bytes input, bytes public_key, bytes proof, bytes batch) {
                let _out = crypto::vrf::verify(request: input);
                let _batch = crypto::vrf::verify_batch(batch);
            }
        }
    "#;
    let code = ivm::KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile vrf intrinsic");
    let (_meta, off) = parse_meta_offset(&code).unwrap();
    let bytes = &code[off..];
    let scall = instruction::wide::system::SCALL;
    let has = |imm: u8| {
        bytes
            .windows(4)
            .any(|w| w == encoding::wide::encode_sys(scall, imm).to_le_bytes())
    };
    assert!(has(syscalls::SYSCALL_VRF_VERIFY as u8));
    assert!(has(syscalls::SYSCALL_VRF_VERIFY_BATCH as u8));
}

#[test]
fn raw_pointer_codec_alias_is_rejected() {
    let src = r#"
        seiyaku RemovedPointerCodec {
            view fn main(bytes value) {
                let _encoded = pointer_to_norito(value);
            }
        }
    "#;
    let error = Compiler::new()
        .compile_source(src)
        .expect_err("raw pointer codec aliases are not part of Kotodama V1");
    assert!(
        error.contains("pointer_to_norito") || error.contains("unknown"),
        "unexpected error: {error}"
    );
}

#[test]
fn raw_axt_intrinsics_are_rejected() {
    use norito::to_bytes;

    let dsid = DataSpaceId::new(7);
    let desc = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![axt::AxtTouchSpec {
            dsid,
            read: vec![],
            write: vec![],
        }],
    };
    let handle = axt::AssetHandle {
        scope: vec!["transfer".to_string()],
        subject: axt::HandleSubject {
            account: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV".to_string(),
            origin_dsid: Some(dsid),
        },
        budget: axt::HandleBudget {
            remaining: Quantity::from(10_u64),
            per_use: None,
        },
        handle_era: 1,
        sub_nonce: 2,
        group_binding: axt::GroupBinding {
            composability_group_id: vec![1, 2, 3],
            epoch_id: 3,
        },
        target_lane: LaneId::new(1),
        axt_binding: vec![0; 32],
        manifest_view_root: vec![0; 32],
        expiry_slot: 42,
        max_clock_skew_ms: Some(5),
    };
    let proof = axt::ProofBlob {
        payload: vec![1, 2, 3, 4],
        expiry_slot: None,
    };
    let desc_hex = hex(&to_bytes(&desc).expect("encode descriptor"));
    let handle_hex = hex(&to_bytes(&handle).expect("encode handle"));
    let proof_hex = hex(&to_bytes(&proof).expect("encode proof"));
    let src = format!(
        r#"
        seiyaku RemovedAxtSurface {{
          kotoage fn main() authorize("UseAxt") {{
            let ds = DataSpaceId::parse("{dsid}");
            let desc = axt_descriptor("0x{desc_hex}");
            let handle = asset_handle("0x{handle_hex}");
            let proof = proof_blob("0x{proof_hex}");
            axt::begin(desc);
            axt::touch(ds, norito_bytes("manifest"));
            axt::verify_proof(ds, proof);
            axt::use_asset_handle(handle: handle, operation: norito_bytes("intent"), proof: proof);
            axt::commit();
          }}
        }}
    "#
    );
    let error = test_compiler()
        .compile_source(&src)
        .expect_err("raw AXT pointer construction is not part of Kotodama V1");
    assert!(
        error.contains("axt_descriptor")
            || error.contains("AssetHandle")
            || error.contains("raw pointer")
            || error.contains("unknown"),
        "unexpected error: {error}"
    );
}

#[test]
fn semantic_return_value_without_declared_type_is_rejected() {
    let src = "module ReturnMismatch { fn f() { return 1; } }";
    let prog = parse(src).expect("parse");
    let err = analyze(&prog).unwrap_err();
    assert_eq!(err.code(), "K2003");
    assert!(err.message().contains("declared return type"));
}
