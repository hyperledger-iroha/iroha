//! Tests for Kotodama parsing, semantics, and compilation.

use std::convert::TryInto;

use iroha_crypto as _;
use iroha_data_model::nexus::{DataSpaceId, LaneId};
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
    let src = "fn add(a: i64, b: i64) { let c = a + b; }";
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
    let src = "module SimpleAdd { fn add(a: i64, b: i64) { let c = a + b; } }";
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
    let initializer = lex("hajimari() {}").expect("lex");
    assert!(
        initializer
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
    let src = "module Types { fn t(x: i64) -> (i64, bool) { let (a, b): (i64, bool) = (1, true); return (x, true); } }";
    let prog = parse(src).expect("parse");
    let typed = analyze(&prog).expect("type");
    let ivm::kotodama::semantic::TypedItem::Function(f) = &typed.items[0];
    assert_eq!(f.name, "t");
}

#[test]
fn bytes_type_is_accepted_and_roundtrips_through_semantics() {
    let src = "module BytesDemo { fn echo(b: bytes) -> bytes { let tmp: bytes = b; return tmp; } }";
    let prog = parse(src).expect("parse bytes");
    let typed = analyze(&prog).expect("analyze bytes");
    let ivm::kotodama::semantic::TypedItem::Function(f) = &typed.items[0];
    assert_eq!(f.ret_ty, Some(Type::Bytes));
}

#[test]
fn string_equality_compiles() {
    let src = "module StringEquality { fn f() { let _x = \"hi\" == \"hi\"; } }";
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
    let src = "seiyaku TupleDestructure { fn sum() -> i64 { let (a,b) = (3,4); let c = (1,2).1; return a + b + c; } }";
    let code = Compiler::new()
        .compile_source(src)
        .expect("compile tuple destructure");
    let mut vm = ivm::IVM::new(u64::MAX);
    vm.load_program(&code).unwrap();
    match vm.run() {
        Ok(_) => {
            eprintln!("tuple_destructure_and_field_access r10={}", vm.register(10));
            assert_eq!(vm.register(10), 3 + 4 + 2);
        }
        Err(e) => {
            panic!("vm run error: {e:?}");
        }
    }
}

#[test]
fn tuple_var_member_access() {
    // Bind a tuple to a name and use member access on it.
    let src = "seiyaku TupleMember { fn f() -> i64 { let t = (5,6); return t.0 + t.1; } }";
    let code = Compiler::new()
        .compile_source(src)
        .expect("compile tuple var member");
    let mut vm = ivm::IVM::new(u64::MAX);
    vm.load_program(&code).unwrap();
    match vm.run() {
        Ok(_) => {
            eprintln!("tuple_var_member_access r10={}", vm.register(10));
            assert_eq!(vm.register(10), 11);
        }
        Err(e) => panic!("vm run error: {e:?}"),
    }
}

#[test]
fn call_function_with_tuple_return() {
    let src = r#"
        seiyaku TupleCall {
            fn pair(x: i64) -> (i64, i64) { return (x, x + 1); }
            fn main() -> i64 {
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
    vm.run().expect("run tuple call");
    assert_eq!(vm.register(10), 56);
}

#[test]
fn amount_arithmetic_compiles_without_implicit_conversion() {
    let src = r#"
        seiyaku AmountArithmetic {
            fn main() -> Amount {
                let a: Amount = Amount::from_i64(9_000_000_000);
                let b: Amount = a * a;
                let c: Amount = b / a;
                return c;
            }
        }
    "#;
    Compiler::new()
        .compile_source(src)
        .expect("compile Amount arithmetic");
}

#[test]
fn negative_amount_conversion_is_rejected() {
    let src = r#"
        seiyaku NegativeAmount {
            fn main() -> Amount {
                let a: Amount = Amount::from_i64(-1);
                return a;
            }
        }
    "#;
    let err = Compiler::new()
        .compile_source(src)
        .expect_err("negative alias literal should fail");
    assert!(err.to_string().contains("cannot convert a negative i64"));
}

#[test]
fn fractional_amount_literal_is_rejected() {
    let src = r#"
        seiyaku DecimalAmount {
            fn main() -> bool {
                let a: Amount = 1.50;
                return a == a;
            }
        }
    "#;
    let err = Compiler::new()
        .compile_source(src)
        .expect_err("fractional Amount literal should fail");
    assert!(
        !err.is_empty(),
        "fractional Amount rejection needs a diagnostic"
    );
}

#[test]
fn decimal_literal_rejects_int_annotation() {
    let prog = parse(
        r#"
        module InvalidDecimalAnnotation {
            fn main() -> i64 {
                let a: i64 = 1.5;
                return a;
            }
        }
        "#,
    )
    .expect("parse decimal literal");
    let err = analyze(&prog).expect_err("expected decimal literal type error");
    assert!(
        err.message.contains("scale=0"),
        "unexpected error message: {}",
        err.message
    );
}

#[test]
fn implicit_amount_to_i64_conversion_is_rejected() {
    let src = r#"
        seiyaku NoImplicitAmountCast {
            fn main() -> i64 {
                let a: Amount = Amount::from_i64(9_000_000_000);
                let b: Amount = a * a;
                let c: i64 = b;
                return c;
            }
        }
    "#;
    let error = Compiler::new()
        .compile_source(src)
        .expect_err("implicit Amount-to-i64 conversion must fail");
    assert!(error.contains("expected i64, got Amount"), "{error}");
}

#[test]
fn assert_builtin_obeys_truthiness() {
    let compiler = test_compiler();

    let pass = compiler
        .compile_source("seiyaku AssertTrue { fn main() { test::assert(true); } }")
        .expect("compile passing assert");
    let mut vm = ivm::IVM::new(u64::MAX);
    vm.load_program(&pass).expect("load passing assert");
    vm.run().expect("test::assert(true) should not abort");

    let fail = compiler
        .compile_source("seiyaku AssertFalse { fn main() { test::assert(false); } }")
        .expect("compile failing assert");
    let mut vm = ivm::IVM::new(u64::MAX);
    vm.load_program(&fail).expect("load failing assert");
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
                let alice = AccountId::parse("sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB");
                let bob = AccountId::parse("sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76");
                let asset = AssetDefinitionId::parse("62Fk4FPcMuLvW5QjDGNF2a4jAmjM");
                ledger::account::set_detail(context::authority(), Name::parse("cursor"), Json::parse("{\"query\":\"sc_dummy\",\"cursor\":1}"));
                ledger::asset::transfer(alice, bob, asset, Amount::from_i64(1), DataSpaceId::parse("0"));
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
                ledger::asset::transfer(
                    AccountId::parse("sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB"),
                    AccountId::parse("sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76"),
                    AssetDefinitionId::parse("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
                    Amount::from_i64(1),
                    DataSpaceId::parse("0")
                );
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
                ledger::account::register(AccountId::parse("sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB"));
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
                ledger::asset::transfer(
                    AccountId::parse("sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB"),
                    AccountId::parse("sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76"),
                    AssetDefinitionId::parse("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
                    Amount::from_i64(1),
                    DataSpaceId::parse("0")
                );
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
            state detail: Map<string, i64>;

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
            state values: StateMap<i64, i64>;
            fn f() { for (k, v) in values { debug::info("kv"); } }
        }
    "#;
    let prog = parse(src).expect("parse");
    // Bare map iteration is rejected by the semantic phase; callers must bound it.
    let err = analyze(&prog).expect_err("expected unbounded iteration error");
    assert!(
        err.message.contains(".take"),
        "error hint should mention the canonical bounded helper: {}",
        err.message
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
            state values: StateMap<i64, i64>;
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
            state values: StateMap<i64, i64>;
            fn f() { for (k, v) in values #[bounded(1)] { let z = k; } }
        }
    "#;
    parse(src).expect_err("legacy bounded attributes are not V1 syntax");
}

#[test]
fn parse_and_type_bounded_map_take_one_ok() {
    let src = r#"
        seiyaku StateIteration {
            state values: StateMap<i64, i64>;
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
            kotoage fn verify(p: bytes) authorize("ZkVerifier") {
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
                ledger::account::set_detail(context::authority(), Name::parse("k"), Json::parse("1"));
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
    let src = r#"seiyaku Time { view fn f() -> i64 { return context::current_time_ms(); } }"#;
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
    let src = r#"seiyaku Height { view fn f() -> i64 { return context::block_height(); } }"#;
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
          kotoage fn f() authorize("Test") {
            let block_time = context::block_time_ms();
            let chain = context::chain_id();
            let contract = context::contract_address();
            let name = context::entrypoint();
            debug::info(block_time);
            debug::info(chain);
            debug::info(contract);
            debug::info(name);
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
        err.message.contains("chain_id expects no arguments"),
        "unexpected error: {}",
        err.message
    );
}

#[test]
fn raw_query_and_authority_sysvar_helpers_are_not_source_apis() {
    let src = r#"
        seiyaku RawQuery {
          view fn query(payload: bytes) -> bytes {
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
        err.message.contains("query_execute_norito"),
        "unexpected error: {}",
        err.message
    );

    let prog =
        parse(r#"module InvalidAuthority { fn f() { let _caller = sysvar_authority(1); } }"#)
            .unwrap();
    let err = analyze(&prog).expect_err("expected sysvar arity error");
    assert!(
        err.message.contains("sysvar_authority"),
        "unexpected error: {}",
        err.message
    );
}

#[test]
fn compile_emits_typed_query_get_helpers() {
    let src = r#"
      seiyaku Queries {
        view fn read() -> bytes {
            let account = ledger::query::account(context::authority());
            let definition = ledger::query::asset_definition(AssetDefinitionId::parse("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"));
            let domain = ledger::query::domain(DomainId::parse("wonderland.universal"));
            let nft = ledger::query::nft(NftId::parse("n0$wonderland.universal"));
            let parameter = ledger::query::parameter(Name::parse("block.max_transactions"));
            let manifest = ledger::query::contract_manifest(b"hash");
            let instance = ledger::query::contract_instance(Name::parse("router::universal"));
            return instance;
        }
      }
    "#;
    let code = test_compiler().compile_source(src).expect("compile");
    let (_, off) = parse_meta_offset(&code).unwrap();
    let code_region = &code[off..];
    for (name, syscall) in [
        ("QUERY_GET_ACCOUNT", syscalls::SYSCALL_QUERY_GET_ACCOUNT),
        (
            "QUERY_GET_ASSET_DEFINITION",
            syscalls::SYSCALL_QUERY_GET_ASSET_DEFINITION,
        ),
        ("QUERY_GET_DOMAIN", syscalls::SYSCALL_QUERY_GET_DOMAIN),
        ("QUERY_GET_NFT", syscalls::SYSCALL_QUERY_GET_NFT),
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

    let account = AccountId::parse_encoded("sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB")
        .map(ParsedAccountId::into_account_id)
        .expect("parse account literal");
    let asset_definition = AssetDefinitionId::parse_address_literal("62Fk4FPcMuLvW5QjDGNF2a4jAmjM")
        .expect("parse asset definition");
    let asset = AssetId::of(asset_definition.clone(), account.clone());
    let src = format!(
        r#"
        seiyaku QueryHints {{
          view fn read() -> bytes {{
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
        err.message.contains("ledger::query::account"),
        "unexpected error: {}",
        err.message
    );

    let prog = parse(
        r#"module InvalidQuery { fn f() { let _instance = ledger::query::contract_instance(1); } }"#,
    )
    .unwrap();
    let err = analyze(&prog).expect_err("expected contract instance query key type error");
    assert!(
        err.message.contains("ledger::query::contract_instance"),
        "unexpected error: {}",
        err.message
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
        err.message
            .contains("crypto::vrf::epoch_seed expects (bytes) VrfEpochSeedRequest"),
        "unexpected error: {}",
        err.message
    );
}

#[test]
fn compile_emits_state_introspection_helpers() {
    let src = r#"
        seiyaku StateIntrospection {
        fn f() {
            let prefix = Name::parse("Orders");
            let keys = state::keys(prefix, 0, 2);
            let present = state::contains(prefix);
            let len = state::len(prefix);
            let count = state::count(prefix);
            debug::info(codec::tlv_len(keys));
            if present {
                debug::info(len);
            }
            debug::info(count);
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
        r#"seiyaku C { fn f() { let _keys = state::keys(Name::parse("Orders"), 0, b"bad"); } }"#,
    )
    .unwrap();
    let err = analyze(&prog).expect_err("expected state_keys type error");
    assert!(
        err.message
            .contains("state::keys expects (Name, i64 offset, i64 limit)"),
        "unexpected error: {}",
        err.message
    );
}

#[test]
fn compile_emits_extended_hash_syscalls() {
    let src = r#"
        seiyaku HashFunctions {
        fn f(payload: bytes) {
            let b = crypto::blake2b256(payload);
            let k = crypto::keccak256(payload);
            let i = crypto::iroha_hash(payload);
            debug::info(codec::tlv_len(b));
            debug::info(codec::tlv_len(k));
            debug::info(codec::tlv_len(i));
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
        err.message.contains("crypto::keccak256 expects (bytes)"),
        "unexpected error: {}",
        err.message
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
            state values: StateMap<i64, i64>;
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
            state values: StateMap<i64, i64>;
            fn f() { for (key, value) in values.take(1) { values[0] = 1; } }
        }
    "#;
    let prog = parse(src).expect("parse");
    let err = analyze(&prog).expect_err("should reject mutation during iteration");
    assert!(err.message.contains("E_ITER_MUTATION"));
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
    let src = "module Arithmetic { fn add(a: i64, b: i64) { let c = a + b; } }";
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
    assert!(err.message.contains("expects i64 operands"));
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
    let src = "seiyaku Add { fn add(a: i64, b: i64) -> i64 { return a + b; } }";
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
    vm.set_register(10, 4);
    vm.set_register(11, 7);
    vm.load_program(&code).unwrap();
    vm.run().expect("execution failed");
    assert_eq!(vm.register(10), 11);
}

#[test]
fn state_allocations_do_not_clobber_params() {
    let src = r#"
        seiyaku StateAllocation {
            state values: StateMap<i64, i64>;
            fn id(x: i64) -> i64 { return x; }
        }
    "#;
    let code = Compiler::new().compile_source(src).expect("compile failed");
    let mut vm = ivm::IVM::new(u64::MAX);
    vm.set_register(10, 42);
    vm.load_program(&code).unwrap();
    vm.run().expect("execution failed");
    assert_eq!(vm.register(10), 42);
}

#[test]
fn compile_builtin_create_nfts_and_set_detail() {
    let src = "seiyaku CanonicalHostCalls { kotoage fn main() authorize(\"Admin\") { ledger::nft::create_for_all_users(); ledger::account::set_detail(context::authority(), Name::parse(\"cursor\"), Json::parse(\"{\\\"query\\\":\\\"sc_dummy\\\",\\\"cursor\\\":1}\")); } }";
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
            let aid = "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB";
            let key = "cursor";
            let val = "{\"query\":\"sc_dummy\",\"cursor\":1}";
            ledger::account::set_detail(AccountId::parse(aid), Name::parse(key), Json::parse(val));
            ledger::domain::transfer(context::authority(), DomainId::parse(did), AccountId::parse(aid));
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
        r#"seiyaku C { fn f(value: bytes) { let _x = AccountId::parse(value); } }"#,
        r#"seiyaku C { fn f(value: bytes) { let _x = Json::parse(value); } }"#,
        r#"seiyaku C { fn f(value: bytes) { let _x = Name::parse(value); } }"#,
        r#"seiyaku C { fn f(value: Name) { let _x = Name::parse(value); } }"#,
        r#"seiyaku C { fn f(value: string) { let _x = value.account_id(); } }"#,
        r#"seiyaku C { fn f(value: string) { let _x = value.name(); } }"#,
        r#"seiyaku C { fn f(value: string) { let _x = value.json(); } }"#,
        r#"seiyaku C { fn f(value: bytes) { let _x = value.blob(); } }"#,
        r#"seiyaku C { fn f(value: bytes) { let _x = value.norito_bytes(); } }"#,
        r#"seiyaku C { fn f() { let _x = blob("raw"); } }"#,
        r#"seiyaku C { fn f() { let _x = norito_bytes("raw"); } }"#,
    ] {
        let error = Compiler::new()
            .compile_source(source)
            .expect_err("non-canonical pointer conversion must be rejected");
        assert!(
            error.contains("expects string")
                || error.contains("method aliases were removed")
                || error.contains("compiler-internal"),
            "unexpected error: {error}"
        );
    }
}

#[test]
fn triple_nested_struct_field_access() {
    // Deeply nested struct fields: d.c.b.a.x
    let src = r#"
        seiyaku NestedStructFields {
        struct A { x: i64; }
        struct B { a: A; }
        struct C { b: B; }
        struct D { c: C; }
        fn f() -> i64 {
            let a = A(5);
            let b = B(a);
            let c = C(b);
            let d = D(c);
            return d.c.b.a.x;
        }
        }
    "#;
    let code = ivm::KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile triple nested access");
    let mut vm = ivm::IVM::new(u64::MAX);
    vm.load_program(&code).unwrap();
    vm.run().expect("execute");
    assert_eq!(vm.register(10), 5);
}

#[test]
fn triple_nested_struct_field_mixed_named_numeric_access() {
    // Mixed access: d.c.0.a.x where D { c: (B, i64) }
    let src = r#"
        seiyaku MixedStructTupleFields {
        struct A { x: i64; }
        struct B { a: A; }
        struct D { c: (B, i64); }
        fn f() -> i64 {
            let a = A(7);
            let b = B(a);
            let d = D((b, 99));
            return d.c.0.a.x;
        }
        }
    "#;
    let code = ivm::KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile mixed named/numeric access");
    let mut vm = ivm::IVM::new(u64::MAX);
    vm.load_program(&code).unwrap();
    vm.run().expect("execute");
    assert_eq!(vm.register(10), 7);
}

#[test]
fn invalid_numeric_on_struct_reports_error() {
    let src = r#"
        module InvalidStructIndex {
        struct A { x: i64; }
        fn f() { let a = A(1); let v = a.0; }
        }
    "#;
    let prog = parse(src).expect("parse ok");
    let err = analyze(&prog).expect_err("expected error");
    assert!(err.message.contains("unknown field '0' on struct A"));
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
    assert!(err.message.contains("unknown field 'a' on tuple"));
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
    assert!(err.message.contains("tuple index 3 out of bounds"));
}

#[test]
fn tuple_index_on_non_tuple_reports_type() {
    let src = r#"
        module InvalidStructTupleIndex {
        struct A { x: i64; }
        fn f() { let s = A(1); let v = s.0; }
        }
    "#;
    let prog = parse(src).expect("parse ok");
    let err = analyze(&prog).expect_err("expected error");
    assert!(
        err.message
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
    assert!(err.message.contains("tuple index on non-tuple type i64"));
}

#[test]
fn unknown_field_on_struct_reports_available_fields() {
    let src = r#"
        module UnknownStructField {
        struct A { x: i64; y: i64; }
        fn f() { let a = A(1,2); let v = a.z; }
        }
    "#;
    let prog = parse(src).expect("parse ok");
    let err = analyze(&prog).expect_err("expected error");
    assert!(
        err.message
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
    assert!(err.message.contains("unknown field 'foo' on type i64"));
}

#[test]
fn invalid_indexing_on_non_map_reports_error() {
    let src = r#"
        module InvalidStructIndexing {
        struct A { x: i64; }
        fn f() { let a = A(1); let v = a[0]; }
        }
    "#;
    let prog = parse(src).expect("parse ok");
    let err = analyze(&prog).expect_err("expected error");
    assert!(err.message.contains("indexing not supported on this type"));
}

#[test]
fn method_call_sugar_receiver_and_arg() {
    // a.method(b) sugar: receiver prepended as first arg
    let src = r#"
        seiyaku MethodCallSugar {
        fn add(x: i64, y: i64) -> i64 { return x + y; }
        fn main() -> i64 { return (5).add(7); }
        }
    "#;
    let code = ivm::KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile method sugar");
    let mut vm = ivm::IVM::new(u64::MAX);
    vm.load_program(&code).unwrap();
    vm.run().expect("execute");
    assert_eq!(vm.register(10), 12);
}

#[test]
fn semantic_type_enforcement_for_typed_syscalls() {
    use ivm::kotodama::parser::parse;
    // Wrong types should fail
    let bad = parse(
        "module InvalidMint { fn f() { ledger::asset::mint(Name::parse(\"x\"), AssetDefinitionId::parse(\"62Fk4FPcMuLvW5QjDGNF2a4jAmjM\"), Amount::from_i64(1)); } }",
    )
    .unwrap();
    assert!(analyze(&bad).is_err());
    let bad2 = parse("module InvalidDetail { fn f() { ledger::account::set_detail(AccountId::parse(\"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB\"), Json::parse(\"1\"), Name::parse(\"k\")); } }").unwrap();
    assert!(analyze(&bad2).is_err());
}

#[test]
fn range_end_less_than_start_rejected() {
    let src = r#"
        seiyaku InvalidStateRange {
            state values: StateMap<i64, i64>;
            fn f() { for (key, value) in values.range(5, 2) { let seen = value; } }
        }
    "#;
    let prog = parse(src).expect("parse");
    let err = analyze(&prog).expect_err("expected end<start rejection");
    assert!(err.message.contains("end >= start"));
}

#[test]
fn range_non_integer_args_rejected() {
    let src = r#"
        seiyaku InvalidStateRangeTypes {
            state values: StateMap<i64, i64>;
            fn f() { for (key, value) in values.range("a", "b") { let seen = value; } }
        }
    "#;
    let prog = parse(src).expect("parse");
    let err = analyze(&prog).expect_err("expected non-integer rejection");
    assert!(err.message.contains("range(start, end)"));
}

#[test]
fn dynamic_state_map_take_is_rejected() {
    let src = r#"
        seiyaku DynamicTake {
        state values: StateMap<i64, i64>;
        fn bounded_take_sum(n: i64) -> i64 {
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
        state values: StateMap<i64, i64>;
        fn bounded_range_sum(start: i64, end: i64) -> i64 {
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
    let src = "seiyaku NftCalls { kotoage fn main() authorize(\"ManageNfts\") { ledger::nft::mint(NftId::parse(\"n0$wonderland.universal\"), AccountId::parse(\"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB\")); ledger::nft::transfer(AccountId::parse(\"sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB\"), NftId::parse(\"n0$wonderland.universal\"), AccountId::parse(\"sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76\")); } }";
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
    let src = "seiyaku Modulo { fn r(a: i64, b: i64) -> i64 { return a % b; } }";
    let code = Compiler::new().compile_source(src).expect("compile modulo");
    let mut vm = ivm::IVM::new(u64::MAX);
    vm.load_program(&code).unwrap();
    vm.set_register(10, 17); // a
    vm.set_register(11, 5); // b
    vm.run().expect("execute");
    assert_eq!(vm.register(10), 17 % 5);
}

#[test]
fn compiler_owns_first_release_abi_metadata() {
    let code = Compiler::new()
        .compile_source("seiyaku FixedAbi { view fn f() -> i64 { return 3; } }")
        .expect("compile");
    let (meta, _off) = parse_meta_offset(&code).unwrap();
    assert_eq!(meta.abi_version, 1);
    assert_eq!(meta.vector_length, 0);
}

#[test]
fn compile_emits_manifest_hashes() {
    use ivm::{SyscallPolicy, syscalls::compute_abi_hash};
    let src = "module ManifestHash { fn f() { let x = 1 + 2; } }";
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
        .compile_source_with_manifest("seiyaku LiteralAlpha { fn f() { debug::info(\"alpha\"); } }")
        .expect("compile alpha");
    let (_, manifest_b) = compiler
        .compile_source_with_manifest("seiyaku LiteralBeta { fn f() { debug::info(\"beta\"); } }")
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
            state counter: i64;

            hajimari() {
                counter = 0;
            }

            kotoage fn run() authorize("Admin") {
                test::assert(true);
                let current = counter;
                let _digest = crypto::poseidon2(current, 1);
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
    assert!(matches!(entrypoints[0].kind, EntryPointKind::Init));
    assert_eq!(entrypoints[0].permission, None);
    assert_eq!(entrypoints[0].read_keys, Vec::<String>::new());
    assert_eq!(entrypoints[0].write_keys, vec!["state:counter"]);

    assert_eq!(entrypoints[1].name, "run");
    assert!(matches!(entrypoints[1].kind, EntryPointKind::Public));
    assert_eq!(entrypoints[1].permission.as_deref(), Some("Admin"));
    assert_eq!(entrypoints[1].read_keys, vec!["state:counter"]);
    assert_eq!(entrypoints[1].write_keys, Vec::<String>::new());
    const FEATURE_ZK: u64 = 1 << 0;
    assert_eq!(manifest.features_bitmap, Some(FEATURE_ZK));
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
            let acc = AccountId::parse("sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB");
            let asset = AssetDefinitionId::parse("62Fk4FPcMuLvW5QjDGNF2a4jAmjM");
            ledger::asset::mint(acc, asset, Amount::from_i64(1));
            ledger::asset::burn(acc, asset, Amount::from_i64(1));
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
        AccountId::parse_encoded("sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB")
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
                AccountId::parse("sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB"),
                DomainId::parse("wonderland.universal"),
                context::authority()
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
fn production_manifest_rejects_parameter_dependent_isi_access() {
    let src = r#"
        seiyaku Test {
            kotoage fn move(from: AccountId, to: AccountId, asset: AssetDefinitionId, amount: Amount, space: DataSpaceId) authorize("Admin") {
                ledger::asset::transfer(from, to, asset, amount, space);
            }
        }
    "#;
    let err = Compiler::new()
        .compile_source_with_manifest(src)
        .expect_err("production compile must reject incomplete parameter-dependent ISI access");
    assert!(
        err.contains("E_ACCESS_INCOMPLETE"),
        "unexpected error: {err}"
    );
}

#[test]
fn kotoba_block_emits_manifest_translations() {
    let src = r#"
        seiyaku C {
            messages {
                "E0001": { en: "Invalid assets", ja: "無効な資産" }
                hint: { en: "Check inputs" }
            }
            view fn main() {}
        }
    "#;
    let (_code, manifest) = Compiler::new()
        .compile_source_with_manifest(src)
        .expect("compile manifest with kotoba");
    let entries = manifest.kotoba.expect("kotoba entries should be present");
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].msg_id, "E0001");
    assert_eq!(entries[0].translations.len(), 2);
    assert_eq!(entries[1].msg_id, "hint");
    assert_eq!(entries[1].translations[0].lang, "en");
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
            ledger::asset::transfer(
              AccountId::parse("sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB"),
              AccountId::parse("sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76"),
              AssetDefinitionId::parse("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
              Amount::from_i64(1),
              DataSpaceId::parse("0")
            );
          }
        }
    "#;
    let prog = parse(src).expect("parse ledger::asset::transfer");
    analyze(&prog).expect("analyze ledger::asset::transfer");

    let src2 = "module RemovedMap { fn make() -> i64 { return std::map::new(); } }";
    let prog2 = parse(src2).expect("parse std::map::new");
    analyze(&prog2).expect_err("in-memory map constructors are removed from V1");
}

#[test]
fn indirect_sensitive_calls_require_permission() {
    let src = r#"
        seiyaku Permission {
          fn helper() {
            ledger::asset::transfer(
              AccountId::parse("sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB"),
              AccountId::parse("sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76"),
              AssetDefinitionId::parse("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
              Amount::from_i64(1),
              DataSpaceId::parse("0")
            );
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
            state counter: i64;

            hajimari() {
                counter = 0;
            }

            kotoage fn bump(times: i64) authorize("Admin") {
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
    let src =
        "module Ternary { fn f(a: i64, b: i64) -> i64 { let x = (1 < 2) ? a : b; return x; } }";
    let prog = parse(src).expect("parse ternary");
    let typed = analyze(&prog).expect("type ternary");
    let ivm::kotodama::semantic::TypedItem::Function(f) = &typed.items[0];
    assert_eq!(f.name, "f");
}

#[test]
fn ternary_min_types() {
    let src = "module Ternary { fn min(a: i64, b: i64) -> i64 { return (a < b) ? a : b; } }";
    let typed = analyze(&parse(src).expect("parse ternary")).expect("type ternary");
    assert!(typed.items.iter().any(|item| {
        matches!(item, ivm::kotodama::semantic::TypedItem::Function(function) if function.name == "min")
    }));
}

#[test]
fn nested_ternary_types() {
    let src = "module Ternary { fn f(a: i64, b: i64, c: i64) -> i64 { return (a < b) ? ((b < c) ? b : c) : a; } }";
    analyze(&parse(src).expect("parse nested ternary")).expect("type nested ternary");
}

#[test]
fn build_options_control_header_and_source_meta_is_unavailable() {
    let src = r#"
seiyaku MyC {
  hajimari() {
    test::assert(true);
    let _digest = crypto::poseidon2(1, 2);
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
    assert_ne!(meta.mode & ivm::ivm_mode::ZK, 0);
    assert_eq!(meta.mode & ivm::ivm_mode::VECTOR, 0);
}

#[test]
fn removed_in_memory_map_indexing_is_rejected() {
    let src = "module RemovedMap { fn f(m: Map<i64, i64>, k: i64) -> i64 { return m[k]; } }";
    let error = Compiler::new()
        .compile_source(src)
        .expect_err("in-memory Map must not compile in V1");
    assert!(error.contains("Map"), "unexpected error: {error}");
}

#[test]
fn branch_lowering_uses_compact_bne_and_one_relaxed_transfer() {
    let src = r#"
seiyaku Branches {
    view fn branch(b: bool) -> i64 {
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
        .find(|entry| entry.function_name == "branch")
        .expect("branch budget report");
    let words = code[metadata.code_offset + branch.pc_start as usize
        ..metadata.code_offset + branch.pc_end as usize]
        .chunks_exact(4)
        .map(|chunk| u32::from_le_bytes(chunk.try_into().expect("word chunk")))
        .collect::<Vec<_>>();

    let bne_index = words
        .iter()
        .position(|word| ((word >> 24) as u8) == instruction::wide::control::BNE)
        .expect("expected BNE in lowered branch");

    let bne_word = words[bne_index];
    let imm = (bne_word & 0xFF) as u8 as i8;
    assert_eq!(
        imm, 2,
        "BNE should skip the one-word else transfer and land on the one-word then transfer"
    );

    let relaxed_else_transfer = words[bne_index + 1];
    let then_block_index = bne_index + imm as usize;
    assert!(
        words.len() > then_block_index,
        "BNE target should land directly in the then block"
    );
    assert_eq!(
        (relaxed_else_transfer >> 24) as u8,
        instruction::wide::control::JAL,
        "fallthrough path must jump to the else block"
    );
    assert_ne!(
        (words[then_block_index] >> 24) as u8,
        instruction::wide::control::JAL,
        "taken path must fall through into the then block without a second jump"
    );
}

#[test]
fn compile_poseidon2_and_assert_eq() {
    // poseidon2 computation
    let src = "module Poseidon { fn f(a: i64, b: i64) { let h = crypto::poseidon2(a, b); } }";
    let code = Compiler::new().compile_source(src).expect("compile failed");
    assert!(!code.is_empty());
    let (meta, _) = parse_meta_offset(&code).unwrap();
    assert_ne!(meta.mode & 0x01, 0, "poseidon2 should enable ZK mode");

    // assert_eq succeeds without enabling ZK mode
    let src = "module Assertions { fn g(a: i64, b: i64) { test::assert_eq(a, b); } }";
    let code = Compiler::new().compile_source(src).expect("compile failed");

    let (meta, _) = parse_meta_offset(&code).unwrap();
    assert_eq!(meta.mode & 0x01, 0);

    let mut vm = ivm::IVM::new(u64::MAX);
    vm.set_register(10, 1);
    vm.set_register(11, 1);
    vm.load_program(&code).unwrap();
    vm.run().expect("assert_eq failed");

    // failing case
    let mut vm2 = ivm::IVM::new(u64::MAX);
    vm2.set_register(10, 1);
    vm2.set_register(11, 2);
    vm2.load_program(&code).unwrap();
    let res = vm2.run();
    assert!(matches!(res, Err(ivm::VMError::AssertionFailed)));
}

#[test]
fn compile_pubkgen_and_valcom() {
    let src = "module Commitments { fn f(a: i64, b: i64) -> (i64, i64) { let p = crypto::pubkgen(a); let c = crypto::valcom(a, b); return (p, c); } }";
    let code = Compiler::new().compile_source(src).expect("compile failed");

    let (meta, _) = parse_meta_offset(&code).unwrap();
    assert!(meta.mode & 0x01 != 0);

    let mut vm = ivm::IVM::new(u64::MAX);
    vm.set_register(10, 9);
    vm.set_register(11, 4);
    vm.load_program(&code).unwrap();
    vm.run().expect("execution failed");

    let expected_pubk = ivm::field::mul(9, 2);
    assert_eq!(vm.register(10), expected_pubk);
    let expected_commit = ivm::pedersen_commit_truncated(9, 4);
    assert_eq!(vm.register(11), expected_commit);
}

#[test]
fn pubkgen_valcom_spills_are_handled() {
    use ivm::kotodama::ir::Instr;
    use ivm::kotodama::regalloc;

    let build_src = |count: usize| {
        let mut src = String::from("module CommitmentSpills {\nfn main(a: i64, b: i64) -> i64 {\n");
        for i in 0..count {
            let value = (i + 1) as i64;
            src.push_str(&format!("  let v{i} = {value};\n"));
        }
        src.push_str("  let c = crypto::valcom(a, b);\n");
        src.push_str("  let p = crypto::pubkgen(c);\n");
        src.push_str("  var sum = 0;\n");
        for i in 0..count {
            src.push_str(&format!("  sum = sum + v{i};\n"));
        }
        src.push_str("  return sum + p + c;\n}\n}\n");
        src
    };

    let mut chosen = None;
    let mut count = 20usize;
    while count <= 80 {
        let src = build_src(count);
        let prog = parse(&src).expect("parse valcom spill");
        let typed = analyze(&prog).expect("analyze valcom spill");
        let ir = ivm::kotodama::ir::lower(&typed).expect("lower");
        let func = ir
            .functions
            .iter()
            .find(|f| f.name == "main")
            .expect("main lowered");
        let alloc = regalloc::allocate(func);
        let is_spilled = |t: &ivm::kotodama::ir::Temp| alloc.stack.contains_key(t);
        let mut saw_spill = false;
        for bb in &func.blocks {
            for ins in &bb.instrs {
                match ins {
                    Instr::Valcom { dest, value, blind } => {
                        if is_spilled(dest) || is_spilled(value) || is_spilled(blind) {
                            saw_spill = true;
                        }
                    }
                    Instr::Pubkgen { dest, src } => {
                        if is_spilled(dest) || is_spilled(src) {
                            saw_spill = true;
                        }
                    }
                    _ => {}
                }
            }
        }
        if saw_spill {
            chosen = Some(src);
            break;
        }
        count += 4;
    }

    let src = chosen.expect("expected valcom/pubkgen spill; adjust pressure if needed");
    Compiler::new()
        .compile_source(&src)
        .expect("compile valcom/pubkgen spill");
}

#[test]
fn typed_json_access_spills_are_handled() {
    use ivm::kotodama::ir::Instr;
    use ivm::kotodama::regalloc;

    let build_src = |count: usize| {
        let mut src = String::from("module JsonSpills {\nfn main(j: Json) -> i64 {\n");
        for i in 0..count {
            let value = (i + 1) as i64;
            src.push_str(&format!("  let v{i} = {value};\n"));
        }
        src.push_str("  var sum = 0;\n");
        for i in 0..count {
            src.push_str(&format!("  sum = sum + v{i};\n"));
        }
        src.push_str("  let val = json::get_i64(j, Name::parse(\"value\"));\n");
        src.push_str("  return sum + val;\n}\n}\n");
        src
    };

    let mut chosen = None;
    let mut count = 20usize;
    while count <= 80 {
        let src = build_src(count);
        let prog = parse(&src).expect("parse typed Json spill");
        let typed = analyze(&prog).expect("analyze typed Json spill");
        let ir = ivm::kotodama::ir::lower(&typed).expect("lower");
        let func = ir
            .functions
            .iter()
            .find(|f| f.name == "main")
            .expect("main lowered");
        let alloc = regalloc::allocate(func);
        let is_spilled = |t: &ivm::kotodama::ir::Temp| alloc.stack.contains_key(t);
        let mut saw_spill = false;
        for bb in &func.blocks {
            for ins in &bb.instrs {
                match ins {
                    Instr::JsonGetInt { dest, json, key } => {
                        if is_spilled(dest) || is_spilled(json) || is_spilled(key) {
                            saw_spill = true;
                        }
                    }
                    _ => {}
                }
            }
        }
        if saw_spill {
            chosen = Some(src);
            break;
        }
        count += 4;
    }

    let src = chosen.expect("expected typed Json access spill; adjust pressure if needed");
    Compiler::new()
        .compile_source(&src)
        .expect("compile typed Json spill");
}

#[test]
fn raw_json_codec_aliases_are_rejected() {
    let src = r#"
        module RemovedCodecAliases {
            fn main(payload: bytes) -> Json {
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
module PoseidonForms {
    fn main() -> (i64, i64) {
        let pair = crypto::poseidon2(123456789, 987654321);
        let sextet = crypto::poseidon6(3, 5, 8, 13, 21, 34);
        return (pair, sextet);
    }
}
"#;
    let code = Compiler::new()
        .compile_source(src)
        .expect("compile both Poseidon arities");
    let parsed = ProgramMetadata::parse(&code).expect("parse metadata");
    assert_ne!(parsed.metadata.mode & ivm::ivm_mode::ZK, 0);
    let poseidon6_word = code[parsed.code_offset..]
        .chunks_exact(4)
        .map(|chunk| u32::from_le_bytes(chunk.try_into().unwrap()))
        .find(|word| instruction::wide::opcode(*word) == instruction::wide::crypto::POSEIDON6)
        .expect("compiled code contains POSEIDON6");
    let (_, input_base) = encoding::wide::decode_poseidon6(poseidon6_word)
        .expect("compiler emits canonical POSEIDON6 register window");
    assert_eq!(input_base, 10);

    let mut vm = ivm::IVM::new(u64::MAX);
    vm.load_program(&code).expect("load Poseidon program");
    vm.run().expect("run Poseidon program");
    assert_eq!(vm.register(10), ivm::poseidon2(123456789, 987654321));
    assert_eq!(vm.register(11), ivm::poseidon6([3, 5, 8, 13, 21, 34]));
}

#[test]
fn unbounded_state_map_iteration_is_rejected() {
    let src = r#"
        seiyaku UnboundedStateIteration {
            state values: StateMap<i64, i64>;
            fn f() { for (key, value) in values { let seen = value; } }
        }
    "#;
    let err = Compiler::new().compile_source(src).unwrap_err();
    assert!(err.contains("E_UNBOUNDED_ITERATION"));
}

#[test]
fn unbounded_state_map_iteration_cannot_infer_a_limit() {
    let src = r#"
        seiyaku MissingStateIterationLimit {
            state values: StateMap<i64, i64>;
            fn sum() { for (key, value) in values { let seen = key; } }
        }
    "#;
    let err = Compiler::new().compile_source(src).unwrap_err();
    assert!(err.contains("E_UNBOUNDED_ITERATION"));
}

#[test]
fn map_new_is_rejected_in_v1() {
    let src = "module RemovedMap { fn make() -> i64 { return Map::new(); } }";
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
    assert!(meta.mode & 0x01 != 0); // uses ZK builtins
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

    use ivm::kotodama::ir::Instr;
    let path = Path::new("tests/data/amm.ko");
    let src = std::fs::read_to_string(path).expect("read failed");
    let prog = parse(&src).expect("parse failed");
    let typed = analyze(&prog).expect("semantic analysis failed");
    let ir = ivm::kotodama::ir::lower(&typed).expect("lower");
    let instrs = &ir.functions[0].blocks[0].instrs;
    assert!(instrs.iter().any(|i| matches!(
        i,
        Instr::Binary {
            op: BinaryOp::Mul,
            ..
        }
    )));
    assert!(instrs.iter().any(|i| matches!(
        i,
        Instr::Binary {
            op: BinaryOp::Div,
            ..
        }
    )));
}

#[test]
fn parse_dai_clone() {
    use std::path::Path;

    use ivm::kotodama::ir::Instr;
    let path = Path::new("tests/data/dai.ko");
    let src = std::fs::read_to_string(path).expect("read failed");
    let prog = parse(&src).expect("parse failed");
    let typed = analyze(&prog).expect("semantic analysis failed");
    let ir = ivm::kotodama::ir::lower(&typed).expect("lower");
    let mut has_add = false;
    let mut has_sub = false;
    for block in &ir.functions[0].blocks {
        for instr in &block.instrs {
            match instr {
                Instr::Binary {
                    op: BinaryOp::Add, ..
                } => has_add = true,
                Instr::Binary {
                    op: BinaryOp::Sub, ..
                } => has_sub = true,
                _ => {}
            }
        }
    }
    assert!(has_add && has_sub);
}

#[test]
fn parse_mint_asset_builtin() {
    use ivm::kotodama::ir::Instr;
    let src = "module MintHelpers { fn f(a: AccountId, b: AssetDefinitionId, c: Amount) { ledger::asset::mint(a, b, c); } }";
    let prog = parse(src).expect("parse failed");
    let typed = analyze(&prog).expect("semantic analysis failed");
    let ir = ivm::kotodama::ir::lower(&typed).expect("lower");
    let instrs = &ir.functions[0].blocks[0].instrs;
    assert!(instrs.iter().any(|i| matches!(i, Instr::MintAsset { .. })));
}

#[test]
fn parse_transfer_asset_builtin() {
    use ivm::kotodama::ir::Instr;
    let src = "module TransferHelpers { fn f(a: AccountId, b: AccountId, c: AssetDefinitionId, d: Amount, e: DataSpaceId) { ledger::asset::transfer(a, b, c, d, e); } }";
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
    let src = "module BatchHelpers { fn f(a: AccountId, b: AccountId, c: AssetDefinitionId, d: Amount) { ledger::asset::transfer_batch((a, b, c, d), (b, a, c, d)); } }";
    let prog = parse(src).expect("parse failed");
    let typed = analyze(&prog).expect("semantic analysis failed");
    let ir = ivm::kotodama::ir::lower(&typed).expect("lower");
    let instrs = &ir.functions[0].blocks[0].instrs;
    assert!(
        !instrs
            .iter()
            .any(|i| matches!(i, Instr::TransferBatchBegin | Instr::TransferBatchEnd)),
        "high-level transfer_batch must not emit batch boundary syscalls"
    );
    let transfer_count = instrs
        .iter()
        .filter(|i| matches!(i, Instr::TransferBatchAsset { .. }))
        .count();
    assert_eq!(
        transfer_count, 2,
        "expected two transfer calls inside batch"
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
    let src = "module InvalidBatch { fn f(a: AccountId) { ledger::asset::transfer_batch(a); } }";
    let prog = parse(src).expect("parse failed");
    assert!(
        analyze(&prog).is_err(),
        "non-tuple transfer_batch entries must be rejected"
    );
}

#[test]
fn parse_burn_asset_builtin() {
    use ivm::kotodama::ir::Instr;
    let src = "module BurnHelpers { fn f(a: AccountId, b: AssetDefinitionId, c: Amount) { ledger::asset::burn(a, b, c); } }";
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
                ledger::asset::register(
                    AssetDefinitionId::parse("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
                    "X",
                    1,
                    0
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
                    AssetDefinitionId::parse("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
                    "X",
                    1,
                    AccountId::parse("sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB"),
                    0
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
            fn f() { ledger::asset::register("x", "X", 1, 0); }
        }
    "#;
    let prog = parse(src).expect("parse failed");
    let err = analyze(&prog).expect_err("bare asset names should be rejected");
    assert!(
        err.message.contains("AssetDefinitionId"),
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
    let src = "module UnaryOps { fn f(a: i64, b: bool) { let c = -a; let d = !b; } }";
    Compiler::new()
        .compile_source(src)
        .expect("compile unary ops");
}

#[test]
fn in_memory_map_methods_are_rejected() {
    let src = r#"
        module RemovedMapMethods {
            fn f(m: Map<i64, i64>) {
                let a = m.contains(1);
                let b = m.ensure(2);
            }
        }
    "#;
    let prog = parse(src).expect("parse map methods");
    let error = analyze(&prog).expect_err("in-memory Map methods must be rejected");
    assert!(error.message.contains("Map"), "unexpected error: {error:?}");
}

#[test]
fn ir_lower_contains_method_state_map() {
    use ivm::kotodama::ir::Instr;
    let src = r#"
        seiyaku StateContains {
            state m: StateMap<i64, i64>;
            view fn f(k: i64) -> bool { return m.contains(k); }
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
            fn g(m: Map<i64, i64>) -> i64 { return keys_take2(m, 0, 1); }
        }
    "#;
    let prog = parse(src).expect("parse keys_take2");
    let error = analyze(&prog).expect_err("ephemeral map helpers must be rejected");
    assert!(error.message.contains("Map"), "unexpected error: {error:?}");
}

#[test]
fn ephemeral_keys_values_take2_helper_is_rejected() {
    let src = r#"
        module RemovedMapEntries {
            fn f(m: Map<i64, i64>) {
                let t = std::map::keys_values_take2(m, 0, 1);
                let a = t.0;
                let b = t.1;
            }
        }
    "#;
    let prog = parse(src).expect("parse keys_values_take2");
    let error = analyze(&prog).expect_err("ephemeral map helpers must be rejected");
    assert!(error.message.contains("Map"), "unexpected error: {error:?}");
}

#[test]
fn ir_tuple_pack_and_get_general() {
    use ivm::kotodama::ir::Instr;
    let src = r#"
        module Tuples {
            fn f(a: i64, b: i64) {
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
        module VrfVerification {
            fn main(input: bytes, public_key: bytes, proof: bytes, batch: bytes) {
                let _out = crypto::vrf::verify(input, public_key, proof, 2);
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
        module RemovedPointerCodec {
            fn main(value: bytes) {
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
            account: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB".to_string(),
            origin_dsid: Some(dsid),
        },
        budget: axt::HandleBudget {
            remaining: 10,
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
            axt::use_asset_handle(handle, norito_bytes("intent"), proof);
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
fn semantic_return_mismatch_unit() {
    let src = "module ReturnMismatch { fn f() -> () { return 1; } }";
    let prog = parse(src).expect("parse");
    let err = analyze(&prog).unwrap_err();
    assert!(err.message.contains("return type mismatch"));
}
