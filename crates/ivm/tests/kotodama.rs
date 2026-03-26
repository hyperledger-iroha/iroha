//! Tests for Kotodama parsing, semantics, and compilation.

use std::{convert::TryInto, str::FromStr};

use iroha_crypto as _;
use iroha_data_model::nexus::{DataSpaceId, LaneId};
use ivm::{
    ProgramMetadata, axt, encoding, instruction,
    kotodama::{
        ast as kd_ast,
        ast::{BinaryOp, Expr, Function, Item, Statement},
        compiler::{Compiler, CompilerOptions},
        lexer::{TokenKind, lex},
        parser::parse,
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
    let src = "fn add(a, b) { let c = a + b; }";
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
    let src = "fn add(a, b) { let c = a + b; }";
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
fn lexer_alias_keywords_equivalence() {
    use ivm::kotodama::lexer::{TokenKind, lex};
    let toks_a = lex("seiyaku { }").expect("lex");
    let toks_b = lex("誓約 { }").expect("lex");
    assert!(matches!(toks_a[0].kind, TokenKind::Seiyaku));
    assert!(matches!(toks_b[0].kind, TokenKind::Seiyaku));
    let toks_c = lex("hajimari() {}").expect("lex");
    let toks_d = lex("始まり() {}").expect("lex");
    assert!(toks_c.iter().any(|t| matches!(t.kind, TokenKind::Hajimari)));
    assert!(toks_d.iter().any(|t| matches!(t.kind, TokenKind::Hajimari)));
}

#[test]
fn parse_and_type_tuples_and_types() {
    let src = "fn t(Map<AccountId, fixed_u128> balances, x: int) -> (int, bool) { let (a,b): (int, bool) = (1, true); return (x, true); }";
    let prog = parse(src).expect("parse");
    let typed = analyze(&prog).expect("type");
    let ivm::kotodama::semantic::TypedItem::Function(f) = &typed.items[0];
    assert_eq!(f.name, "t");
}

#[test]
fn bytes_type_is_accepted_and_roundtrips_through_semantics() {
    let src = "fn echo(b: bytes) -> bytes { let tmp: bytes = b; return tmp; }";
    let prog = parse(src).expect("parse bytes");
    let typed = analyze(&prog).expect("analyze bytes");
    let ivm::kotodama::semantic::TypedItem::Function(f) = &typed.items[0];
    assert_eq!(f.ret_ty, Some(Type::Bytes));
}

#[test]
fn string_equality_compiles() {
    let src = "fn f() { let _x = \"hi\" == \"hi\"; }";
    let code = Compiler::new()
        .compile_source(src)
        .expect("string equality should compile");
    assert!(!code.is_empty());
}

#[test]
fn irohaswap_sample_compiles() {
    let src = include_str!("../../kotodama_lang/src/samples/irohaswap.ko");
    let code = Compiler::new()
        .compile_source(src)
        .expect("irohaswap sample should compile");
    assert!(!code.is_empty());
}

#[test]
fn prediction_market_demo_compiles() {
    let src = include_str!("../../../demo/prediction_market.ko");
    let code = Compiler::new()
        .compile_source(src)
        .expect("prediction market demo should compile");
    assert!(!code.is_empty());
}

#[test]
fn tuple_destructure_and_field_access() {
    // Destructure a tuple literal into (a,b) and sum; also exercise direct field access `(1,2).1`
    let src = "fn sum() -> int { let (a,b) = (3,4); let c = (1,2).1; return a + b + c; }";
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
    let src = "fn f() -> int { let t = (5,6); return t.0 + t.1; }";
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
        fn pair(x: int) -> (int, int) { return (x, x + 1); }
        fn main() -> int {
            let (a, b) = pair(7);
            return a * b;
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
fn numeric_alias_u128_arithmetic_roundtrip() {
    let src = r#"
        fn main() -> int {
            let a: Amount = 9_000_000_000;
            let b: Amount = a * a;
            let c: Amount = b / a;
            return c;
        }
    "#;
    let code = Compiler::new()
        .compile_source(src)
        .expect("compile numeric alias arithmetic");
    let mut vm = ivm::IVM::new(u64::MAX);
    vm.load_program(&code).expect("load program");
    vm.run().expect("run numeric alias program");
    assert_eq!(vm.register(10), 9_000_000_000);
}

#[test]
fn numeric_alias_negative_literal_rejected() {
    let src = r#"
        fn main() -> int {
            let a: Amount = -1;
            return a;
        }
    "#;
    let err = Compiler::new()
        .compile_source(src)
        .expect_err("negative alias literal should fail");
    assert!(err.to_string().contains("unsigned"));
}

#[test]
fn numeric_alias_decimal_literal_rejected() {
    let src = r#"
        fn main() -> bool {
            let a: Amount = 1.50;
            return a == 1;
        }
    "#;
    let err = Compiler::new()
        .compile_source(src)
        .expect_err("decimal alias literal should fail");
    assert!(err.to_string().contains("scale=0"));
}

#[test]
fn decimal_literal_rejects_int_annotation() {
    let prog = parse(
        r#"
        fn main() -> int {
            let a: int = 1.5;
            return a;
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
fn numeric_alias_to_int_overflow_rejected() {
    let src = r#"
        fn main() -> int {
            let a: Amount = 9_000_000_000;
            let b: Amount = a * a;
            let c: int = b;
            return c;
        }
    "#;
    let code = Compiler::new()
        .compile_source(src)
        .expect("compile numeric alias overflow");
    let mut vm = ivm::IVM::new(u64::MAX);
    vm.load_program(&code).expect("load program");
    let err = vm.run().expect_err("expected overflow cast failure");
    assert!(matches!(err, ivm::VMError::AssertionFailed));
}

#[test]
fn many_string_literals_load_under_wide_guard() {
    // Exercise pointer literal emission with offsets beyond the wide 8-bit range.
    let mut src = String::from("fn main() {");
    for i in 0..32 {
        src.push_str(&format!(" info(\"literal_{i}\");"));
    }
    src.push_str(" return;");
    src.push('}');

    let code = Compiler::new()
        .compile_source(&src)
        .expect("compile program with many literals");
    let mut vm = ivm::IVM::new(u64::MAX);
    vm.load_program(&code)
        .expect("wide guard must accept Kotodama output");
}

#[test]
fn prelude_macros_compile() {
    let src = r#"
        seiyaku MacroDemo {
            kotoage fn run() permission(Admin) {
                let alice = account!("6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn");
                let bob = account!("6cmzPVPX4Vs6C1nbbQ7UD7Q6AWKJFC12abs4kZtXEE9SsFf6QRpp8rU");
                let asset = asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM");
                set_account_detail(authority(), name!("cursor"), json!{ query: "sc_dummy", cursor: 1 });
                transfer_asset(alice, bob, asset, 1);
            }
        }
    "#;
    Compiler::new()
        .compile_source(src)
        .expect("compile macros from the Kotodama prelude");
}

#[test]
fn public_function_without_permission_rejected() {
    let src = r#"
        seiyaku PermissionDemo {
            kotoage fn run() {
                transfer_asset(
                    account!("6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn"),
                    account!("6cmzPVPX4Vs6C1nbbQ7UD7Q6AWKJFC12abs4kZtXEE9SsFf6QRpp8rU"),
                    asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
                    1
                );
            }
        }
    "#;
    let err = Compiler::new()
        .compile_source(src)
        .expect_err("missing permission should be rejected");
    assert!(
        err.contains("permission"),
        "error should mention missing permission: {err}"
    );
}

#[test]
fn register_peer_requires_permission() {
    let src = r#"
        seiyaku PermissionDemo {
            kotoage fn add() {
                register_peer(json!{ address: "127.0.0.1:1337" });
            }
        }
    "#;
    let err = Compiler::new()
        .compile_source(src)
        .expect_err("missing permission should be rejected");
    assert!(
        err.contains("permission"),
        "error should mention missing permission for register_peer: {err}"
    );
}

#[test]
fn register_account_requires_permission() {
    let src = r#"
        seiyaku PermissionDemo {
            kotoage fn add() {
                register_account(account!("6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn"));
            }
        }
    "#;
    let err = Compiler::new()
        .compile_source(src)
        .expect_err("missing permission should be rejected");
    assert!(
        err.contains("permission"),
        "error should mention missing permission for register_account: {err}"
    );
}

#[test]
fn trigger_management_requires_permission() {
    let src = r#"
        seiyaku PermissionDemo {
            kotoage fn add() {
                create_trigger(json!{ id: "t1" });
                set_trigger_enabled(name!("t1"), 1);
            }
        }
    "#;
    let err = Compiler::new()
        .compile_source(src)
        .expect_err("missing permission should be rejected");
    assert!(
        err.contains("permission"),
        "error should mention missing permission for trigger ops: {err}"
    );
}

#[test]
fn public_function_with_permission_is_allowed() {
    let src = r#"
        seiyaku PermissionDemo {
            kotoage fn run() permission(Admin) {
                transfer_asset(
                    account!("6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn"),
                    account!("6cmzPVPX4Vs6C1nbbQ7UD7Q6AWKJFC12abs4kZtXEE9SsFf6QRpp8rU"),
                    asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
                    1
                );
            }
        }
    "#;
    Compiler::new()
        .compile_source(src)
        .expect("permission attribute should allow privileged call");
}

#[test]
fn on_chain_profile_rejects_string_key_maps() {
    let src = r#"
        seiyaku Demo {
            state Map<string, int> detail;

            fn main() {
                return;
            }
        }
    "#;

    let err = Compiler::new()
        .compile_source(src)
        .expect_err("expected on-chain profile error for Map<string, int>");
    assert!(
        err.contains("on-chain profile")
            && (err.contains("Map<string, int>") || err.contains("key type `string`")),
        "unexpected error: {err}"
    );

    let opts = CompilerOptions {
        enforce_on_chain_profile: false,
        ..CompilerOptions::default()
    };
    let relaxed = Compiler::new_with_options(opts);
    relaxed
        .compile_source(src)
        .expect("compilation should succeed when on-chain profile disabled");
}

#[test]
fn parse_for_each_map_and_builtins() {
    let src = r#"fn f(m: Map<int, int>) { for (k, v) in m { info("kv"); } }"#;
    let prog = parse(src).expect("parse");
    // Unbounded map iteration is currently rejected by the semantic phase.
    let err = analyze(&prog).expect_err("expected unbounded iteration error");
    assert!(
        err.message.contains("#[bounded") || err.message.contains(".take"),
        "error hint should mention bounded helpers: {}",
        err.message
    );
}

#[test]
fn parse_for_loop() {
    let src = "fn f() { for let i = 0; i < 3; i++ { let x = i; } }";
    let prog = parse(src).expect("parse failed");
    let Item::Function(Function { body, .. }) = &prog.items[0] else {
        panic!("expected function item");
    };
    match &body.statements[0] {
        Statement::For { line: _, .. } => {}
        _ => panic!("expected for"),
    }
}

#[test]
fn parse_range_for_loop_rejected() {
    // Grammar accepts 'for x in expr' but current semantics reject unbounded iteration
    let src = "fn f() { for x in range(6) { let y = x; } }";
    let prog = parse(src).expect("parse should succeed");
    assert!(analyze(&prog).is_err());
}

#[test]
fn parse_and_type_bounded_map_take_two_rejected_for_ephemeral() {
    let src = r#"fn f(m: Map<int, int>) { for (k, v) in m.take(2) { let z = k; } }"#;
    let prog = parse(src).expect("parse");
    let err = analyze(&prog).expect_err("ephemeral map take(2) should be rejected");
    assert!(
        err.message.contains("E_MAP_BOUNDS"),
        "error should mention map bounds: {}",
        err.message
    );
}

#[test]
fn bounded_attribute_accepts_literal() {
    let src = r#"fn f(m: Map<int, int>) { for (k, v) in m #[bounded(1)] { let z = k; } }"#;
    let prog = parse(src).expect("parse");
    analyze(&prog).expect("analyze with #[bounded]");
}

#[test]
fn bounded_attribute_rejects_ephemeral_overflow() {
    let src = r#"fn f(m: Map<int, int>) { for (k, v) in m #[bounded(2)] { let z = k; } }"#;
    let prog = parse(src).expect("parse");
    let err = analyze(&prog).expect_err("ephemeral bounded >1 should error");
    assert!(
        err.message.contains("E_MAP_BOUNDS"),
        "error should mention map bounds: {}",
        err.message
    );
}

#[test]
fn parse_and_type_bounded_map_take_one_ok() {
    let src = r#"fn f(m: Map<int, int>) { for (k, v) in m.take(1) { let z = k; } }"#;
    let prog = parse(src).expect("parse");
    let typed = analyze(&prog).expect("analyze");
    let ivm::kotodama::semantic::TypedItem::Function(func) = &typed.items[0];
    assert!(!func.body.statements.is_empty());
}

#[test]
fn compile_domain_literal_emits_tlv_domainid() {
    let src = r#"
        fn hajimari() {
            let d = domain("wonderland");
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
        fn hajimari() {
            register_domain(domain("wonderland"));
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
        fn hajimari() {
            // norito_bytes("..") is a typed constructor producing a Blob TLV
            let p = norito_bytes("00");
            zk_verify_batch(p);
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
    // Ensure `blob("...")` emits a TLV with type 0x0006 (Blob)
    let src = r#"
        fn hajimari() {
            let b = blob("hello");
        }
    "#;
    let bytes = Compiler::new().compile_source(src).expect("compile ok");
    assert!(
        bytes.windows(2).any(|w| w == [0x00, 0x06]),
        "expected Blob TLV type (0x0006) in compiled artifact"
    );
}

#[test]
fn semantic_typed_pointers_and_authority() {
    // set_account_detail(authority(), name("k"), json("1")) should type-check
    let src = r#"fn f() { set_account_detail(authority(), name("k"), json("1")); }"#;
    let prog = parse(src).expect("parse");
    let res = analyze(&prog);
    assert!(
        res.is_ok(),
        "semantics should accept typed pointers + authority"
    );
}

#[test]
fn compile_emits_get_authority_syscall() {
    // Compile a program that uses authority() and ensure SCALL GET_AUTHORITY is present
    let src = r#"fn f() { let a = authority(); }"#;
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
    let src = r#"fn f() { let now = current_time_ms(); }"#;
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
fn compile_emits_resolve_account_alias_syscall() {
    let src = r#"fn f() { let a = resolve_account_alias("banking@sbp"); }"#;
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
    let src = r#"fn f(m: Map<int, int>) { for (k, v) in m.take(1) { let z = k; } }"#;
    let prog = parse(src).expect("parse");
    let typed = analyze(&prog).expect("analyze");
    let ivm::kotodama::semantic::TypedItem::Function(func) = &typed.items[0];
    assert!(!func.body.statements.is_empty());
}

#[test]
fn for_each_map_mutation_is_rejected() {
    // Mutation of the iterated map inside the loop must be rejected.
    let src = r#"fn f(m: Map<int, int>) { for (k, v) in m.take(1) { m[0] = 1; } }"#;
    let prog = parse(src).expect("parse");
    let err = analyze(&prog).expect_err("should reject mutation during iteration");
    assert!(err.message.contains("E_ITER_MUTATION"));
}

#[test]
fn parse_error() {
    let src = "fn bad("; // incomplete
    let err = parse(src).unwrap_err();
    assert!(err.contains("identifier"));
    assert!(err.contains("fn bad("));
}

#[test]
fn semantic_simple_add() {
    let src = "fn add(a, b) { let c = a + b; }";
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
    let src = "fn bad() { let a = 1 + \"hi\"; }";
    let prog = parse(src).expect("parse failed");
    let err = analyze(&prog).unwrap_err();
    assert!(err.message.contains("expects int operands"));
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
    let src = "fn add(a: int, b: int) -> int { return a + b; }";
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
        state Map<int,int> m;
        fn id(x: int) -> int { return x; }
    "#;
    let code = Compiler::new().compile_source(src).expect("compile failed");
    let mut vm = ivm::IVM::new(u64::MAX);
    vm.set_register(10, 42);
    vm.load_program(&code).unwrap();
    vm.run().expect("execution failed");
    assert_eq!(vm.register(10), 42);
}

#[test]
fn compile_builtin_create_nfts_and_depth_and_set_detail() {
    let src = "fn main() { create_nfts_for_all_users(); set_execution_depth(111); set_account_detail(authority(), name(\"cursor\"), json(\"{\\\"query\\\":\\\"sc_dummy\\\",\\\"cursor\\\":1}\")); }";
    let code = Compiler::new().compile_source(src).expect("compile failed");
    // Sanity: code contains at least three syscalls (order preserved)
    // Byte-pattern search for SCALL encodings (LE): [imm8, 0x00, 0x00, 0x60]
    let (_meta, off) = parse_meta_offset(&code).unwrap();
    let code_bytes = &code[off..];
    // Expect SCALL encodings for our helper syscalls present
    let scall = instruction::wide::system::SCALL;
    let _want = [
        encoding::wide::encode_sys(scall, syscalls::SYSCALL_CREATE_NFTS_FOR_ALL_USERS as u8),
        encoding::wide::encode_sys(
            scall,
            syscalls::SYSCALL_SET_SMARTCONTRACT_EXECUTION_DEPTH as u8,
        ),
        encoding::wide::encode_sys(scall, syscalls::SYSCALL_SET_ACCOUNT_DETAIL as u8),
    ];
    let imm_create = syscalls::SYSCALL_CREATE_NFTS_FOR_ALL_USERS as u8;
    let imm_depth = syscalls::SYSCALL_SET_SMARTCONTRACT_EXECUTION_DEPTH as u8;
    let imm_detail = syscalls::SYSCALL_SET_ACCOUNT_DETAIL as u8;
    let pat = |imm: u8| [imm, 0x00, 0x00, instruction::wide::system::SCALL];
    let has = |imm: u8| code_bytes.windows(4).any(|w| w == pat(imm));
    assert!(has(imm_create) && has(imm_depth) && has(imm_detail));
}

#[test]
fn call_sugar_parses_and_compiles_equivalently() {
    // Without `call` sugar
    let src_plain = "fn f() { set_account_detail(authority(), name(\"k\"), json(\"1\")); }";
    // With `call` sugar
    let src_call = "fn f() { call set_account_detail(authority(), name(\"k\"), json(\"1\")); }";
    let code_plain = Compiler::new()
        .compile_source(src_plain)
        .expect("compile plain");
    let code_call = Compiler::new()
        .compile_source(src_call)
        .expect("compile call sugar");
    assert_eq!(
        code_plain, code_call,
        "call sugar should not change codegen"
    );
}

#[test]
fn call_sugar_misuse_reports_location() {
    let src = "fn f() { call 1; }";
    let err = parse(src).expect_err("expected parse error");
    assert!(err.contains("call expects a function call"));
}

#[test]
fn pointer_constructors_accept_string_variables() {
    // Use variables bound to string literals; constructors should work
    let src = r#"
        fn main() {
            let did = "wonderland";
            let aid = "6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn";
            let key = "cursor";
            let val = "{\"query\":\"sc_dummy\",\"cursor\":1}";
            set_account_detail(account_id(aid), name(key), json(val));
            transfer_domain(authority(), domain(did), account_id(aid));
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
fn json_constructor_accepts_norito_bytes_pointer() {
    let src = r#"
        fn main() {
            let jb = norito_bytes("{\"k\":1}");
            let j = json(jb);
            // Use j to ensure it flows through as Json pointer
            let did = "wonderland";
            let aid = "6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn";
            set_account_detail(account_id(aid), name("cursor"), j);
        }
    "#;
    let code = ivm::KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile json from norito_bytes");
    let (_m, off) = parse_meta_offset(&code).unwrap();
    let bytes = &code[off..];
    let scall = instruction::wide::system::SCALL;
    let has = |imm: u8| {
        bytes
            .windows(4)
            .any(|w| w == encoding::wide::encode_sys(scall, imm).to_le_bytes())
    };
    assert!(
        has(syscalls::SYSCALL_JSON_DECODE as u8) && has(syscalls::SYSCALL_SET_ACCOUNT_DETAIL as u8),
        "expected JSON_DECODE and SET_ACCOUNT_DETAIL syscalls"
    );
}

#[test]
fn method_sugar_name_on_variable() {
    let src = r#"
        fn main() {
            let aid = "6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn";
            let key = "cursor";
            let val = "{\"x\":1}";
            set_account_detail(aid.account_id(), key.name(), val.json());
        }
    "#;
    let code = ivm::KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile method sugar name/json");
    let (_m, off) = parse_meta_offset(&code).unwrap();
    let bytes = &code[off..];
    let scall = instruction::wide::system::SCALL;
    let has = |imm: u8| {
        bytes
            .windows(4)
            .any(|w| w == encoding::wide::encode_sys(scall, imm).to_le_bytes())
    };
    assert!(has(syscalls::SYSCALL_SET_ACCOUNT_DETAIL as u8));
}

#[test]
fn method_sugar_json_on_norito_bytes_variable() {
    let src = r#"
        fn main() {
            let nb = "{\"k\":1}".norito_bytes();
            let j = nb.json();
            let aid = "6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn";
            set_account_detail(aid.account_id(), "cursor".name(), j);
        }
    "#;
    let code = ivm::KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile method sugar json on blob");
    let (_m, off) = parse_meta_offset(&code).unwrap();
    let bytes = &code[off..];
    let scall = instruction::wide::system::SCALL;
    let has = |imm: u8| {
        bytes
            .windows(4)
            .any(|w| w == encoding::wide::encode_sys(scall, imm).to_le_bytes())
    };
    assert!(
        has(syscalls::SYSCALL_JSON_DECODE as u8) && has(syscalls::SYSCALL_SET_ACCOUNT_DETAIL as u8),
        "expected JSON_DECODE and SET_ACCOUNT_DETAIL syscalls"
    );
}

#[test]
fn name_constructor_accepts_norito_bytes_pointer() {
    let src = r#"
        fn main() {
            let nb = norito_bytes("domain_name");
            let nm = name(nb);
            let aid = "6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn";
            transfer_domain(authority(), nm, aid.account_id());
        }
    "#;
    let code = ivm::KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile name from norito bytes");
    let (_m, off) = parse_meta_offset(&code).unwrap();
    let bytes = &code[off..];
    let scall = instruction::wide::system::SCALL;
    let has = |imm: u8| {
        bytes
            .windows(4)
            .any(|w| w == encoding::wide::encode_sys(scall, imm).to_le_bytes())
    };
    assert!(
        has(syscalls::SYSCALL_NAME_DECODE as u8) && has(syscalls::SYSCALL_TRANSFER_DOMAIN as u8),
        "expected NAME_DECODE and TRANSFER_DOMAIN syscalls"
    );
}

#[test]
fn method_sugar_name_on_norito_bytes_variable() {
    let src = r#"
        fn main() {
            let nb = "wonderland".norito_bytes();
            let nm = nb.name();
            let aid = "6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn";
            transfer_domain(authority(), nm, aid.account_id());
        }
    "#;
    let code = ivm::KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile name method on norito bytes");
    let (_m, off) = parse_meta_offset(&code).unwrap();
    let bytes = &code[off..];
    let scall = instruction::wide::system::SCALL;
    let has = |imm: u8| {
        bytes
            .windows(4)
            .any(|w| w == encoding::wide::encode_sys(scall, imm).to_le_bytes())
    };
    assert!(
        has(syscalls::SYSCALL_NAME_DECODE as u8) && has(syscalls::SYSCALL_TRANSFER_DOMAIN as u8),
        "expected NAME_DECODE and TRANSFER_DOMAIN syscalls"
    );
}

#[test]
fn name_constructor_rejects_wrong_type_json() {
    let src = r#"
        fn f() { let j = json("{} "); let n = name(j); }
    "#;
    let prog = parse(src).expect("parse ok");
    let err = analyze(&prog).expect_err("expected type error");
    assert!(
        err.message
            .contains("name expects string, Name, or Blob (NoritoBytes)")
    );
}

#[test]
fn json_constructor_rejects_wrong_type_name() {
    let src = r#"
        fn f() { let nm = name("k"); let j = json(nm); }
    "#;
    let prog = parse(src).expect("parse ok");
    let err = analyze(&prog).expect_err("expected type error");
    assert!(
        err.message
            .contains("json expects string, Json, or Blob (NoritoBytes)")
    );
}

#[test]
fn blob_constructor_rejects_wrong_type_json() {
    let src = r#"
        fn f() { let j = json("{} "); let b = blob(j); }
    "#;
    let prog = parse(src).expect("parse ok");
    let err = analyze(&prog).expect_err("expected type error");
    assert!(
        err.message.contains("blob expects string or Blob")
            || err.message.contains("norito_bytes expects string or Blob")
    );
}

#[test]
fn account_id_constructor_accepts_blob() {
    let src = r#"
        fn main() {
            let nb = norito_bytes("bad");
            let a = account_id(nb);
            info("ok");
        }
    "#;
    let code = Compiler::new()
        .compile_source(src)
        .expect("account_id should accept NoritoBytes");
    let (_m, off) = parse_meta_offset(&code).unwrap();
    let bytes = &code[off..];
    let scall = instruction::wide::system::SCALL;
    let has = |imm: u8| {
        bytes
            .windows(4)
            .any(|w| w == encoding::wide::encode_sys(scall, imm).to_le_bytes())
    };
    assert!(has(syscalls::SYSCALL_POINTER_FROM_NORITO as u8));
}

#[test]
fn name_pass_through_from_name_pointer() {
    let src = r#"
        fn main() {
            let nm = name("wonderland");
            let nm2 = name(nm);
            let aid = "6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn";
            transfer_domain(authority(), nm2, aid.account_id());
        }
    "#;
    let code = ivm::KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile");
    let (_m, off) = parse_meta_offset(&code).unwrap();
    let bytes = &code[off..];
    let scall = instruction::wide::system::SCALL;
    let has = |imm: u8| {
        bytes
            .windows(4)
            .any(|w| w == encoding::wide::encode_sys(scall, imm).to_le_bytes())
    };
    // Should not need NAME_DECODE or JSON_DECODE for pass-through Name pointer
    assert!(has(syscalls::SYSCALL_TRANSFER_DOMAIN as u8));
    assert!(!has(syscalls::SYSCALL_NAME_DECODE as u8));
    assert!(!has(syscalls::SYSCALL_JSON_DECODE as u8));
}

#[test]
fn contains_ephemeral_true_and_false() {
    let src = r#"
        fn f() -> int {
            let m = Map::new();
            m[7] = 111;
            let t = contains(m, 7);
            let f = contains(m, 8);
            // return t*2 + f to disambiguate (expect 2*1+0=2)
            return t*2 + f;
        }
    "#;
    let code = ivm::KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile");
    let mut vm = ivm::IVM::new(u64::MAX);
    vm.load_program(&code).unwrap();
    vm.run().expect("execute");
    assert_eq!(vm.register(10), 2);
}

#[test]
fn contains_durable_true_then_false() {
    let src = r#"
        state Map<int,int> m;
        fn f() -> int {
            m[7] = 1;
            let t = contains(m, 7);
            let f = contains(m, 8);
            return t*2 + f;
        }
    "#;
    let code = ivm::KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile durable contains");
    let mut vm = ivm::IVM::new(u64::MAX);
    vm.set_host(ivm::CoreHost::new());
    vm.set_register(31, 0x0031_0000);
    vm.load_program(&code).unwrap();
    vm.run().expect("execute");
    assert_eq!(vm.register(10), 2);
}

#[test]
fn method_sugar_contains_ephemeral() {
    let src = r#"
        fn f() -> int {
            let m = Map::new();
            m[1] = 2;
            let t = m.contains(1);
            let f = m.contains(2);
            return t*2 + f;
        }
    "#;
    let code = ivm::KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile");
    let mut vm = ivm::IVM::new(u64::MAX);
    vm.load_program(&code).unwrap();
    vm.run().expect("execute");
    assert_eq!(vm.register(10), 2);
}

#[test]
fn contains_rejects_wrong_types() {
    let src = r#"
        fn f() { let m = Map::new(); let x = contains(7, m); }
    "#;
    let prog = parse(src).expect("parse ok");
    let err = analyze(&prog).expect_err("expected type error");
    assert!(
        err.message
            .contains("contains expects Map<K,V> as first arg")
    );
}

#[test]
fn get_or_default_ephemeral() {
    let src = r#"
        fn f() -> int {
            let m = Map::new();
            m[7] = 111;
            let a = get_or_default(m, 7, 5);
            let b = get_or_default(m, 8, 9);
            return a*2 + b;
        }
    "#;
    let code = ivm::KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile");
    let mut vm = ivm::IVM::new(u64::MAX);
    vm.load_program(&code).unwrap();
    match vm.run() {
        Ok(()) => {
            assert_eq!(vm.register(10), 111 * 2 + 9);
        }
        Err(err) => {
            eprintln!("run err: {:?} pc={}", err, vm.pc());
            panic!("execute");
        }
    }
}

#[test]
fn get_or_default_durable() {
    let src = r#"
        state Map<int,int> m;
        fn f() -> int {
            m[7] = 111;
            let a = get_or_default(m, 7, 5);
            let b = get_or_default(m, 8, 9);
            return a*2 + b;
        }
    "#;
    let code = ivm::KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile");
    let mut vm = ivm::IVM::new(u64::MAX);
    vm.set_host(ivm::CoreHost::new());
    vm.load_program(&code).unwrap();
    vm.run().expect("execute");
    assert_eq!(vm.register(10), 111 * 2 + 9);
}

#[test]
fn blob_method_pass_through_compiles() {
    let src = r#"
        fn main() -> int {
            let nb = norito_bytes("abc");
            let bb = nb.blob();
            return 0;
        }
    "#;
    let code = ivm::KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile");
    let (_m, off) = parse_meta_offset(&code).unwrap();
    let bytes = &code[off..];
    // Should not need NAME/JSON decode; just a simple program with HALT
    let scall = instruction::wide::system::SCALL;
    let has = |imm: u8| {
        bytes
            .windows(4)
            .any(|w| w == encoding::wide::encode_sys(scall, imm).to_le_bytes())
    };
    assert!(!has(syscalls::SYSCALL_NAME_DECODE as u8));
    assert!(!has(syscalls::SYSCALL_JSON_DECODE as u8));
}

#[test]
fn norito_bytes_constructor_rejects_json_pointer() {
    let src = r#"
        fn f() { let j = json("{} "); let nb = norito_bytes(j); }
    "#;
    let prog = parse(src).expect("parse ok");
    let err = analyze(&prog).expect_err("expected type error");
    assert!(err.message.contains("norito_bytes expects string or Blob"));
}

#[test]
fn nested_struct_field_access_and_map_indexing() {
    // Define nested structs and access map nested inside
    let src = r#"
        struct Inner { map: Map<int, int>; }
        struct Outer { inner: Inner; }
        fn f() -> int {
            let m = Map::new();
            m[7] = 111;
            let i = Inner(m);
            let o = Outer(i);
            return o.inner.map[7];
        }
    "#;
    let code = ivm::KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile nested struct and map indexing");
    let mut vm = ivm::IVM::new(u64::MAX);
    vm.load_program(&code).unwrap();
    // The program allocates the map internally; nothing to set up beyond run
    vm.run().expect("execute");
    // Expect 111 in r10 from nested access
    assert_eq!(vm.register(10), 111);
}

#[test]
fn triple_nested_struct_field_access() {
    // Deeply nested struct fields: d.c.b.a.x
    let src = r#"
        struct A { x: int; }
        struct B { a: A; }
        struct C { b: B; }
        struct D { c: C; }
        fn f() -> int {
            let a = A(5);
            let b = B(a);
            let c = C(b);
            let d = D(c);
            return d.c.b.a.x;
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
    // Mixed access: d.c.0.a.x where D { c: (B, int) }
    let src = r#"
        struct A { x: int; }
        struct B { a: A; }
        struct D { c: (B, int); }
        fn f() -> int {
            let a = A(7);
            let b = B(a);
            let d = D((b, 99));
            return d.c.0.a.x;
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
fn durable_nested_struct_map_path_compiles_to_state_get() {
    use ivm::{encoding, instruction, syscalls};
    // State-backed nested structs: ensure s.inner.map[7] compiles to BUILD_PATH + STATE_GET
    let src = r#"
        struct Inner { map: Map<int, int>; }
        struct Outer { inner: Inner; }
        state Outer s;
        fn f() -> int { return s.inner.map[7]; }
    "#;
    let code = ivm::KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile durable nested map path");
    let (_meta, off) = parse_meta_offset(&code).unwrap();
    let bytes = &code[off..];
    let scall = instruction::wide::system::SCALL;
    let has = |imm: u8| {
        bytes
            .windows(4)
            .any(|w| w == encoding::wide::encode_sys(scall, imm).to_le_bytes())
    };
    assert!(
        has(syscalls::SYSCALL_BUILD_PATH_MAP_KEY as u8)
            && has(syscalls::SYSCALL_STATE_GET as u8)
            && has(syscalls::SYSCALL_DECODE_INT as u8),
        "expected BUILD_PATH_MAP_KEY + STATE_GET + DECODE_INT in bytecode"
    );
}

#[test]
fn invalid_numeric_on_struct_reports_error() {
    let src = r#"
        struct A { x: int; }
        fn f() { let a = A(1); let v = a.0; }
    "#;
    let prog = parse(src).expect("parse ok");
    let err = analyze(&prog).expect_err("expected error");
    assert!(err.message.contains("unknown field '0' on struct A"));
}

#[test]
fn invalid_named_on_tuple_reports_error() {
    let src = r#"
        fn f() { let t = (1,2); let v = t.a; }
    "#;
    let prog = parse(src).expect("parse ok");
    let err = analyze(&prog).expect_err("expected error");
    assert!(err.message.contains("unknown field 'a' on tuple"));
}

#[test]
fn invalid_numeric_tuple_index_reports_error() {
    let src = r#"
        fn f() { let t = (1,2); let v = t.3; }
    "#;
    let prog = parse(src).expect("parse ok");
    let err = analyze(&prog).expect_err("expected error");
    assert!(err.message.contains("tuple index 3 out of bounds"));
}

#[test]
fn tuple_index_on_non_tuple_reports_type() {
    let src = r#"
        struct A { x: int; }
        fn f() { let s = A(1); let v = s.0; }
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
        fn f() { let n = 1; let v = n.0; }
    "#;
    let prog = parse(src).expect("parse ok");
    let err = analyze(&prog).expect_err("expected error");
    assert!(err.message.contains("tuple index on non-tuple type int"));
}

#[test]
fn unknown_field_on_struct_reports_available_fields() {
    let src = r#"
        struct A { x: int; y: int; }
        fn f() { let a = A(1,2); let v = a.z; }
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
        fn f() { let n = 1; let v = n.foo; }
    "#;
    let prog = parse(src).expect("parse ok");
    let err = analyze(&prog).expect_err("expected error");
    assert!(err.message.contains("unknown field 'foo' on type int"));
}

#[test]
fn invalid_indexing_on_non_map_reports_error() {
    let src = r#"
        struct A { x: int; }
        fn f() { let a = A(1); let v = a[0]; }
    "#;
    let prog = parse(src).expect("parse ok");
    let err = analyze(&prog).expect_err("expected error");
    assert!(err.message.contains("indexing not supported on this type"));
}

#[test]
fn method_call_sugar_receiver_and_arg() {
    // a.method(b) sugar: receiver prepended as first arg
    let src = r#"
        fn add(x: int, y: int) -> int { return x + y; }
        fn main() -> int { return (5).add(7); }
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
fn ir_bounded_map_take_n_sets_loop_bounds() {
    use ivm::kotodama::ir::{self, Instr, Temp};
    let src = r#"fn f(m: Map<int, int>) { for (k, v) in m.take(1) { let z = v; } }"#;
    let prog = parse(src).expect("parse");
    let typed = analyze(&prog).expect("analyze");
    let ir_prog = ir::lower(&typed).expect("lower");
    let func = ir_prog
        .functions
        .iter()
        .find(|f| f.name == "f")
        .expect("function f present");
    let mut consts = std::collections::HashMap::<Temp, i64>::new();
    for instr in func.blocks.iter().flat_map(|bb| &bb.instrs) {
        if let Instr::Const { dest, value } = instr {
            consts.insert(*dest, *value);
        }
    }
    let mut saw_start_limit = false;
    for instr in func.blocks.iter().flat_map(|bb| &bb.instrs) {
        if let Instr::Binary {
            op: ivm::kotodama::ast::BinaryOp::Lt,
            left,
            right,
            ..
        } = instr
            && consts.get(left) == Some(&0)
            && consts.get(right) == Some(&1)
        {
            saw_start_limit = true;
            break;
        }
    }
    assert!(
        saw_start_limit,
        "loop should compare index 0 against limit 1; const map: {consts:?}"
    );
    let mut saw_key = false;
    let mut saw_val = false;
    for instr in func.blocks.iter().flat_map(|bb| &bb.instrs) {
        if let Instr::Load64Imm { imm, .. } = instr {
            if *imm == 0 {
                saw_key = true;
            } else if *imm == 8 {
                saw_val = true;
            }
        }
    }
    assert!(
        saw_key && saw_val,
        "bounded loop should load key/value pairs"
    );
}

#[test]
fn ir_bounded_map_range_start_is_honored() {
    use ivm::kotodama::ir::{self, Instr, Temp};
    let src = r#"fn f(m: Map<int, int>) { for (k, v) in m.range(0, 1) { let z = v; } }"#;
    let prog = parse(src).expect("parse");
    let typed = analyze(&prog).expect("analyze");
    let ir_prog = ir::lower(&typed).expect("lower");
    let func = ir_prog
        .functions
        .iter()
        .find(|f| f.name == "f")
        .expect("function f present");
    let mut consts = std::collections::HashMap::<Temp, i64>::new();
    for instr in func.blocks.iter().flat_map(|bb| &bb.instrs) {
        if let Instr::Const { dest, value } = instr {
            consts.insert(*dest, *value);
        }
    }
    let mut honored = false;
    for instr in func.blocks.iter().flat_map(|bb| &bb.instrs) {
        if let Instr::Binary {
            op: ivm::kotodama::ast::BinaryOp::Lt,
            left,
            right,
            ..
        } = instr
            && consts.get(left) == Some(&0)
            && consts.get(right) == Some(&1)
        {
            honored = true;
            break;
        }
    }
    assert!(
        honored,
        "expected comparison between start 0 and end 1; const map: {consts:?}"
    );
}
#[test]
fn semantic_type_enforcement_for_typed_syscalls() {
    use ivm::kotodama::parser::parse;
    // Wrong types should fail
    let bad =
        parse("fn f() { mint_asset(name(\"x\"), asset_definition(\"62Fk4FPcMuLvW5QjDGNF2a4jAmjM\"), 1); }")
            .unwrap();
    assert!(analyze(&bad).is_err());
    let bad2 = parse("fn f() { set_account_detail(account_id(\"6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn\"), json(\"1\"), name(\"k\")); }").unwrap();
    assert!(analyze(&bad2).is_err());
}

#[test]
fn range_end_less_than_start_rejected() {
    let src = r#"fn f(m: Map<int, int>) { for (k, v) in m.range(5, 2) { let z = v; } }"#;
    let prog = parse(src).expect("parse");
    let err = analyze(&prog).expect_err("expected end<start rejection");
    assert!(err.message.contains("end >= start"));
}

#[test]
fn range_non_integer_args_rejected() {
    let src = r#"fn f(m: Map<int, int>) { for (k, v) in m.range("a", "b") { let z = v; } }"#;
    let prog = parse(src).expect("parse");
    let err = analyze(&prog).expect_err("expected non-integer rejection");
    assert!(err.message.contains("range(start, end)"));
}

#[cfg(feature = "kotodama_dynamic_bounds")]
#[test]
fn dynamic_take_n1_executes_once() {
    // Program: sum first n values from a minimal map
    let src = r#"
        fn bounded_take_sum(m: Map<int, int>, n: int) -> int {
            let acc = 0;
            for (k, v) in m.take(n) { acc = acc + v; }
            return acc;
        }
    "#;
    let code = ivm::KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile dyn take");
    let mut vm = ivm::IVM::new(u64::MAX);
    vm.load_program(&code).unwrap();
    // Allocate two buckets (32 bytes): (k0=7,v0=111), (k1=8,v1=222)
    let base = vm.alloc_heap(32).expect("alloc");
    vm.store_u64(base + 0, 7).expect("store k0");
    vm.store_u64(base + 8, 111).expect("store v0");
    vm.store_u64(base + 16, 8).expect("store k1");
    vm.store_u64(base + 24, 222).expect("store v1");
    // Args: m=base, n=1
    vm.set_register(10, base);
    vm.set_register(11, 1);
    vm.run().expect("execute");
    // Expect only first bucket counted: acc == 111
    assert_eq!(vm.register(10), 111);
}

#[cfg(feature = "kotodama_dynamic_bounds")]
#[test]
fn dynamic_range_start1_end2_executes_second_only() {
    // Program: sum entries in range [start, end)
    let src = r#"
        fn bounded_range_sum(m: Map<int, int>, start: int, end: int) -> int {
            let acc = 0;
            for (k, v) in m.range(start, end) { acc = acc + v; }
            return acc;
        }
    "#;
    let code = ivm::KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile dyn range");
    let mut vm = ivm::IVM::new(u64::MAX);
    vm.load_program(&code).unwrap();
    // Allocate two buckets (32 bytes): (k0=7,v0=111), (k1=8,v1=222)
    let base = vm.alloc_heap(32).expect("alloc");
    vm.store_u64(base + 0, 7).expect("store k0");
    vm.store_u64(base + 8, 111).expect("store v0");
    vm.store_u64(base + 16, 8).expect("store k1");
    vm.store_u64(base + 24, 222).expect("store v1");
    // Args: m=base, start=1, end=2  => n = 1, only index 1 bucket counted
    vm.set_register(10, base);
    vm.set_register(11, 1);
    vm.set_register(12, 2);
    vm.run().expect("execute");
    // Expect only second bucket counted: acc == 222
    assert_eq!(vm.register(10), 222);
}

#[test]
fn compile_typed_nft_syscalls() {
    let src = "fn main() { nft_mint_asset(nft_id(\"n0$wonderland\"), account_id(\"6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn\")); nft_transfer_asset(account_id(\"6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn\"), nft_id(\"n0$wonderland\"), account_id(\"6cmzPVPX4Vs6C1nbbQ7UD7Q6AWKJFC12abs4kZtXEE9SsFf6QRpp8rU\")); }";
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
fn compile_and_run_map_get_match_and_mismatch() {
    // Program that returns m[k]
    let src = "fn get(m: Map<int, int>, k: int) -> int { let v = m[k]; return v; }";
    let code = Compiler::new().compile_source(src).expect("compile failed");

    // Case 1: key matches -> returns stored value
    let mut vm = ivm::IVM::new(u64::MAX);
    vm.load_program(&code).unwrap();
    // Decode trace left disabled. Allocate after loading program so memory isn't overwritten.
    let base = vm.alloc_heap(16).expect("alloc");
    vm.store_u64(base, 7).expect("store key");
    vm.store_u64(base + 8, 4242).expect("store val");
    vm.set_register(10, base); // m
    vm.set_register(11, 7); // k
    vm.run().expect("execution failed");
    // Return value must equal stored value when keys match
    assert_eq!(vm.register(10), 4242);

    // Case 2: key mismatch -> returns 0
    let mut vm2 = ivm::IVM::new(u64::MAX);
    vm2.load_program(&code).unwrap();
    let base2 = vm2.alloc_heap(16).expect("alloc");
    vm2.store_u64(base2, 7).expect("store key");
    vm2.store_u64(base2 + 8, 4242).expect("store val");
    vm2.set_register(10, base2); // m
    vm2.set_register(11, 8); // k (mismatch)
    vm2.run().expect("execution failed");
    // Return value must be 0 when keys mismatch
    assert_eq!(vm2.register(10), 0);
}

#[test]
fn compile_and_run_map_set() {
    // Program that sets m[k] = v on the minimal 2-word map layout
    let src = "fn set(m: Map<int, int>, k: int, v: int) { m[k] = v; }";
    let code = Compiler::new().compile_source(src).expect("compile failed");

    let mut vm = ivm::IVM::new(u64::MAX);
    // Decode trace left disabled.
    let base = vm.alloc_heap(16).expect("alloc");
    // Initialize with key=7, value=0
    vm.store_u64(base, 7).expect("store key");
    vm.store_u64(base + 8, 0).expect("store val");

    // Call: m=base, k=7, v=1234
    vm.set_register(10, base);
    vm.set_register(11, 7);
    vm.set_register(12, 1234);
    vm.load_program(&code).unwrap();
    vm.run().expect("execution failed");

    // Value at offset 8 should be updated
    // Verify the 64-bit value at offset 8 was updated
    let v64 = vm.memory.load_u64(base + 8).unwrap();
    assert_eq!(v64, 1234);
}

#[test]
fn map_get_handles_spills() {
    use ivm::kotodama::ir::Instr;
    use ivm::kotodama::regalloc;

    let count = 20usize;
    let mut src = String::from("fn main(m: Map<int,int>, k: int) -> int {\n");
    for i in 0..count {
        let value = (i + 1) as i64;
        src.push_str(&format!("  let a{i} = {value};\n"));
    }
    src.push_str("  let v = m[k];\n");
    src.push_str("  let sum = ");
    for i in 0..count {
        if i > 0 {
            src.push_str(" + ");
        }
        src.push_str(&format!("a{i}"));
    }
    src.push_str(";\n");
    src.push_str("  let hit = std::map::has(m, k);\n");
    src.push_str("  return sum + v + hit;\n}\n");

    let prog = parse(&src).expect("parse spill map-get");
    let typed = analyze(&prog).expect("analyze spill map-get");
    let ir = ivm::kotodama::ir::lower(&typed).expect("lower");
    let func = ir
        .functions
        .iter()
        .find(|f| f.name == "main")
        .expect("main lowered");
    let alloc = regalloc::allocate(func);
    let is_spilled = |t: &ivm::kotodama::ir::Temp| alloc.stack.contains_key(t);
    let mut saw_map_get_spill = false;
    for bb in &func.blocks {
        for ins in &bb.instrs {
            if let Instr::MapGet { dest, map, key } = ins
                && (is_spilled(dest) || is_spilled(map) || is_spilled(key))
            {
                saw_map_get_spill = true;
            }
        }
    }
    assert!(
        saw_map_get_spill,
        "expected MapGet operand spill under register pressure"
    );

    let code = Compiler::new()
        .compile_source(&src)
        .expect("compile spill map-get");
    let mut vm = ivm::IVM::new(u64::MAX);
    vm.load_program(&code).unwrap();
    let base = vm.alloc_heap(16).expect("alloc map");
    vm.store_u64(base, 7).expect("store key");
    vm.store_u64(base + 8, 100).expect("store val");
    vm.set_register(10, base);
    vm.set_register(11, 7);
    vm.run().expect("execute spill map-get");
    let expected = (count as u64 * (count as u64 + 1) / 2) + 100 + 1;
    assert_eq!(vm.register(10), expected);
}

#[test]
fn keys_take2_load64imm_executes() {
    use ivm::kotodama::ir::Instr;

    let count = 20usize;
    let mut src = String::from("fn main(m: Map<int,int>) -> int {\n");
    for i in 0..count {
        let value = (i + 1) as i64;
        src.push_str(&format!("  let a{i} = {value};\n"));
    }
    src.push_str("  let pick = std::map::keys_take2(m, 0, 0);\n");
    src.push_str("  let sum = 0;\n");
    for i in 0..count {
        src.push_str(&format!("  sum = sum + a{i};\n"));
    }
    src.push_str("  return sum + pick;\n}\n");

    let prog = parse(&src).expect("parse load64");
    let typed = analyze(&prog).expect("analyze load64");
    let ir = ivm::kotodama::ir::lower(&typed).expect("lower");
    let func = ir
        .functions
        .iter()
        .find(|f| f.name == "main")
        .expect("main lowered");
    let mut saw_load64imm = false;
    for bb in &func.blocks {
        for ins in &bb.instrs {
            if let Instr::Load64Imm { .. } = ins {
                saw_load64imm = true;
                break;
            }
        }
    }
    assert!(saw_load64imm, "expected Load64Imm in keys_take2 lowering");

    let code = Compiler::new()
        .compile_source(&src)
        .expect("compile load64");
    let mut vm = ivm::IVM::new(u64::MAX);
    vm.load_program(&code).unwrap();
    let base = vm.alloc_heap(16).expect("alloc map");
    vm.store_u64(base, 7).expect("store key");
    vm.store_u64(base + 8, 100).expect("store val");
    vm.set_register(10, base);
    vm.run().expect("execute load64");
    let expected = (count as u64 * (count as u64 + 1) / 2) + 7;
    assert_eq!(vm.register(10), expected);
}

#[test]
fn map_load_pair_handles_spilled_map_base() {
    use ivm::kotodama::ir::Instr;
    use ivm::kotodama::regalloc;

    let count = 20usize;
    let mut src = String::from("fn main(m: Map<int,int>) -> int {\n");
    for i in 0..count {
        let value = (i + 1) as i64;
        src.push_str(&format!("  let a{i} = {value};\n"));
    }
    src.push_str("  let sum = ");
    for i in 0..count {
        if i > 0 {
            src.push_str(" + ");
        }
        src.push_str(&format!("a{i}"));
    }
    src.push_str(";\n");
    src.push_str("  let hit = std::map::has(m, 7);\n");
    src.push_str("  return sum + hit;\n}\n");

    let prog = parse(&src).expect("parse spill map-load");
    let typed = analyze(&prog).expect("analyze spill map-load");
    let ir = ivm::kotodama::ir::lower(&typed).expect("lower");
    let func = ir
        .functions
        .iter()
        .find(|f| f.name == "main")
        .expect("main lowered");
    let alloc = regalloc::allocate(func);
    let is_spilled = |t: &ivm::kotodama::ir::Temp| alloc.stack.contains_key(t);
    let mut saw_map_load_pair_spill = false;
    for bb in &func.blocks {
        for ins in &bb.instrs {
            if let Instr::MapLoadPair {
                dest_key,
                dest_val,
                map,
                ..
            } = ins
                && (is_spilled(map) || is_spilled(dest_key) || is_spilled(dest_val))
            {
                saw_map_load_pair_spill = true;
            }
        }
    }
    assert!(
        saw_map_load_pair_spill,
        "expected MapLoadPair operand spill under register pressure"
    );

    let code = Compiler::new()
        .compile_source(&src)
        .expect("compile spill map-load");
    let mut vm = ivm::IVM::new(u64::MAX);
    vm.load_program(&code).unwrap();
    let base = vm.alloc_heap(16).expect("alloc map");
    vm.store_u64(base, 7).expect("store key");
    vm.store_u64(base + 8, 1).expect("store val");
    vm.set_register(10, base);
    vm.run().expect("execute spill map-load");
    let expected = (count as u64 * (count as u64 + 1) / 2) + 1;
    assert_eq!(vm.register(10), expected);
}

#[test]
fn compile_and_run_modulo() {
    // Return a % b
    let src = "fn r(a, b) -> int { return a % b; }";
    let code = Compiler::new().compile_source(src).expect("compile modulo");
    let mut vm = ivm::IVM::new(u64::MAX);
    vm.load_program(&code).unwrap();
    vm.set_register(10, 17); // a
    vm.set_register(11, 5); // b
    vm.run().expect("execute");
    assert_eq!(vm.register(10), 17 % 5);
}

#[test]
fn compiler_abi_version_plumbing() {
    let opts = CompilerOptions {
        abi_version: 1,
        ..Default::default()
    };
    let code = Compiler::new_with_options(opts)
        .compile_source("fn f() { let x = 1 + 2; }")
        .expect("compile");
    let (meta, _off) = parse_meta_offset(&code).unwrap();
    assert_eq!(meta.abi_version, 1);
}

#[test]
fn compile_emits_manifest_hashes() {
    use ivm::{SyscallPolicy, syscalls::compute_abi_hash};
    let src = "fn f() { let x = 1 + 2; }";
    let (code, manifest) = Compiler::new()
        .compile_source_with_manifest(src)
        .expect("compile with manifest");
    let parsed = ProgramMetadata::parse(&code).expect("parse header");
    let meta = parsed.metadata;
    let expected_code_hash = iroha_crypto::Hash::new(&code[parsed.header_len..]);
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
        .compile_source_with_manifest("fn f() { info(\"alpha\"); }")
        .expect("compile alpha");
    let (_, manifest_b) = compiler
        .compile_source_with_manifest("fn f() { info(\"beta\"); }")
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
            meta { features: ["zk", "simd"]; }
            state int counter;

            hajimari() {
                counter = 0;
            }

            kotoage fn run() permission(Admin) {
                setvl(8);
                assert(true);
                let current = counter;
                if current > 0 {
                    info("counter tick");
                } else {
                    info("counter fresh");
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
    assert!(matches!(entrypoints[1].kind, EntryPointKind::Public));
    assert_eq!(entrypoints[1].permission.as_deref(), Some("Admin"));
    assert_eq!(entrypoints[1].read_keys, vec!["state:counter"]);
    assert_eq!(entrypoints[1].write_keys, Vec::<String>::new());
    const FEATURE_ZK: u64 = 1 << 0;
    const FEATURE_VECTOR: u64 = 1 << 1;
    assert_eq!(manifest.features_bitmap, Some(FEATURE_ZK | FEATURE_VECTOR));
}

#[test]
fn manifest_includes_trigger_descriptors() {
    use iroha_data_model::{events::EventFilterBox, trigger::action::Repeats};

    let src = r#"
        seiyaku Demo {
            kotoage fn run() {}

            register_trigger wake {
                call run;
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
        domain::DomainId,
    };

    let src = r#"
        fn main() {
            let acc = account_id("6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn");
            let asset = asset_definition("62Fk4FPcMuLvW5QjDGNF2a4jAmjM");
            mint_asset(acc, asset, 1);
            burn_asset(acc, asset, 1);
        }
    "#;
    let (_code, manifest) = Compiler::new()
        .compile_source_with_manifest(src)
        .expect("compile manifest with ISI hints");
    let hints = manifest
        .access_set_hints
        .expect("access_set_hints must be present");
    let account: AccountId =
        AccountId::parse_encoded("6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn")
            .map(iroha_data_model::account::ParsedAccountId::into_account_id)
            .expect("parse encoded account literal");
    let asset_def: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        "wonderland".parse().unwrap(),
        "rose".parse().unwrap(),
    );
    let domain: DomainId = "wonderland".parse().expect("domain");
    let asset_id = AssetId::of(asset_def.clone(), account.clone());

    assert!(hints.read_keys.contains(&format!("account:{account}")));
    assert!(hints.read_keys.contains(&format!("domain:{domain}")));
    assert!(hints.read_keys.contains(&format!("asset_def:{asset_def}")));
    assert!(hints.read_keys.contains(&format!("asset:{asset_id}")));
    assert!(hints.write_keys.contains(&format!("asset_def:{asset_def}")));
    assert!(hints.write_keys.contains(&format!("asset:{asset_id}")));

    let entrypoints = manifest.entrypoints.expect("entrypoints must be present");
    let main = entrypoints
        .iter()
        .find(|entry| entry.name == "main")
        .expect("main entrypoint");
    assert!(main.read_keys.contains(&format!("account:{account}")));
    assert!(main.read_keys.contains(&format!("domain:{domain}")));
    assert!(main.read_keys.contains(&format!("asset_def:{asset_def}")));
    assert!(main.read_keys.contains(&format!("asset:{asset_id}")));
    assert!(main.write_keys.contains(&format!("asset_def:{asset_def}")));
    assert!(main.write_keys.contains(&format!("asset:{asset_id}")));
}

#[test]
fn manifest_emits_wildcard_hints_when_isi_targets_are_opaque() {
    let src = r#"
        fn main() {
            transfer_domain(
                account_id("6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn"),
                domain("wonderland"),
                authority()
            );
        }
    "#;
    let (_code, manifest) = Compiler::new()
        .compile_source_with_manifest(src)
        .expect("compile manifest with opaque ISI");
    let hints = manifest
        .access_set_hints
        .expect("opaque ISI targets should emit wildcard hints");
    assert_eq!(hints.read_keys, vec!["*".to_string()]);
    assert_eq!(hints.write_keys, vec!["*".to_string()]);
    let entrypoints = manifest.entrypoints.expect("entrypoints must be present");
    let main = entrypoints
        .iter()
        .find(|entry| entry.name == "main")
        .expect("main entrypoint");
    assert_eq!(main.read_keys, vec!["*".to_string()]);
    assert_eq!(main.write_keys, vec!["*".to_string()]);
}

#[test]
fn kotoba_block_emits_manifest_translations() {
    let src = r#"
        seiyaku C {
            kotoba {
                "E0001": { en: "Invalid assets", ja: "無効な資産" }
                hint: { en: "Check inputs" }
            }
            kotoage fn main() {}
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
        fn f() {
            /* comment */
            let a = 0x10;
            let b = 0b1010;
            let c = 10_000;
        }
    "#;
    let prog = parse(src).expect("parse with comments and literals");
    let _typed = analyze(&prog).expect("analyze literals");
}

#[test]
fn compound_assignments_typecheck() {
    // x +=, -=, *=, /=, %= should typecheck and rebind SSA name
    let src = r#"fn f() { let x = 1; x += 2; x *= 3; x /= 2; x %= 2; }"#;
    let prog = parse(src).expect("parse compound assigns");
    let _typed = analyze(&prog).expect("analyze compound assigns");
}

#[test]
fn namespaced_host_calls_and_std_map_new_parse_and_type() {
    // host::transfer_asset should resolve to transfer_asset builtin
    let src = r#"
        fn f() {
            host::transfer_asset(
              account_id("6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn"),
              account_id("6cmzPVPX4Vs6C1nbbQ7UD7Q6AWKJFC12abs4kZtXEE9SsFf6QRpp8rU"),
              asset_definition("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
              1
            );
        }
    "#;
    let prog = parse(src).expect("parse host::transfer_asset");
    analyze(&prog).expect("analyze host::transfer_asset");

    // std::map::new should normalize to Map::new
    let src2 = "fn make() -> int { return std::map::new(); }";
    let prog2 = parse(src2).expect("parse std::map::new");
    analyze(&prog2).expect("analyze std::map::new");
}

#[test]
fn indirect_sensitive_calls_require_permission() {
    let src = r#"
        fn helper() {
            transfer_asset(
              account_id("6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn"),
              account_id("6cmzPVPX4Vs6C1nbbQ7UD7Q6AWKJFC12abs4kZtXEE9SsFf6QRpp8rU"),
              asset_definition("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
              1
            );
        }

        kotoage fn public_entry() {
            helper();
        }
    "#;
    let err = Compiler::new().compile_source(src).unwrap_err();
    assert!(
        err.contains("permission"),
        "expected permission error, got {err}"
    );
}

#[test]
fn while_assignments_propagate_state() {
    let src = r#"
        seiyaku Counter {
            state int counter;

            hajimari() {
                counter = 0;
            }

            kotoage fn bump(times: int) {
                let i = 0;
                while i < times {
                    counter = counter + 1;
                    i = i + 1;
                }
            }
        }
    "#;
    Compiler::new()
        .compile_source(src)
        .expect("while loop assignments should compile");
}

#[test]
fn ternary_parses_and_types() {
    // Ternary as expression; only parse and typecheck (IR lowering TBD)
    let src = "fn f(a: int, b: int) -> int { let x = (1 < 2) ? a : b; return x; }";
    let prog = parse(src).expect("parse ternary");
    let typed = analyze(&prog).expect("type ternary");
    let ivm::kotodama::semantic::TypedItem::Function(f) = &typed.items[0];
    assert_eq!(f.name, "f");
}

#[test]
fn compile_and_run_ternary_min() {
    // Return the min of two ints using a ternary expression
    let src = "fn min(a: int, b: int) -> int { return (a < b) ? a : b; }";
    let code = Compiler::new()
        .compile_source(src)
        .expect("compile ternary");
    let mut vm = ivm::IVM::new(u64::MAX);
    vm.load_program(&code).unwrap();
    // Case 1: a < b
    vm.set_register(10, 3); // a
    vm.set_register(11, 5); // b
    vm.run().expect("exec");
    assert_eq!(vm.register(10), 3);

    // Case 2: a >= b
    let mut vm2 = ivm::IVM::new(u64::MAX);
    vm2.load_program(&code).unwrap();
    vm2.set_register(10, 7); // a
    vm2.set_register(11, 4); // b
    vm2.run().expect("exec");
    assert_eq!(vm2.register(10), 4);
}

#[test]
fn compile_and_run_nested_ternary() {
    // Return: a<b ? (b<c ? b : c) : a
    let src = "fn f(a: int, b: int, c: int) -> int { return (a < b) ? ((b < c) ? b : c) : a; }";
    let code = Compiler::new()
        .compile_source(src)
        .expect("compile nested ternary");
    let mut vm = ivm::IVM::new(u64::MAX);
    vm.load_program(&code).unwrap();
    vm.set_register(10, 1); // a
    vm.set_register(11, 3); // b
    vm.set_register(12, 2); // c
    vm.run().expect("exec");
    assert_eq!(vm.register(10), 2);
}

#[test]
fn seiyaku_meta_controls_header() {
    let src = r#"
seiyaku MyC {
  meta {
    abi_version: 1;
    vector_length: 8;
    max_cycles: 1234;
    zk: true;
    vector: true;
  }
  hajimari() {
    setvl(8);
    assert(true);
    let a = 1;
  }
}
"#;
    let code = Compiler::new()
        .compile_source(src)
        .expect("compile seiyaku meta");
    let (meta, _off) = parse_meta_offset(&code).unwrap();
    assert_eq!(meta.abi_version, 1);
    assert_eq!(meta.vector_length, 8);
    assert_eq!(meta.max_cycles, 1234);
    assert_ne!(meta.mode & ivm::ivm_mode::ZK, 0);
    assert_ne!(meta.mode & ivm::ivm_mode::VECTOR, 0);
}

#[test]
fn compile_map_get_then_branch_control_flow() {
    // Program performs a map get and then conditionally returns either the value or 0
    let src = "fn f(m: Map<int, int>, k: int, b: bool) -> int { let v = m[k]; if b { return v; } else { return 0; } }";
    let code = Compiler::new().compile_source(src).expect("compile failed");

    // Prepare a simple 2-word map with key=5, value=777
    let mut vm = ivm::IVM::new(u64::MAX);
    vm.load_program(&code).unwrap();
    let base = vm.alloc_heap(16).expect("alloc");
    vm.store_u64(base, 5).expect("store key");
    vm.store_u64(base + 8, 777).expect("store val");

    // Case 1: branch taken (b = true) -> returns v
    vm.set_register(10, base); // m
    vm.set_register(11, 5); // k
    vm.set_register(12, 1); // b = true
    vm.run().expect("execution failed");
    assert_eq!(vm.register(10), 777);

    // Case 2: branch not taken (b = false) -> returns 0
    let mut vm2 = ivm::IVM::new(u64::MAX);
    vm2.load_program(&code).unwrap();
    let base2 = vm2.alloc_heap(16).expect("alloc");
    vm2.store_u64(base2, 5).expect("store key");
    vm2.store_u64(base2 + 8, 777).expect("store val");
    vm2.set_register(10, base2);
    vm2.set_register(11, 5);
    vm2.set_register(12, 0); // b = false
    vm2.run().expect("execution failed");
    assert_eq!(vm2.register(10), 0);
}

#[test]
fn branch_lowering_uses_short_bne_and_dual_jal() {
    let src = r#"
fn branch(b: bool) -> int {
    if b {
        return 1;
    } else {
        return 2;
    }
}
"#;
    let code = Compiler::new()
        .compile_source(src)
        .expect("compile simple branch");
    let (_meta, code_offset) = parse_meta_offset(&code).expect("parse metadata");

    let mut words: Vec<u32> = Vec::new();
    for chunk in code[code_offset..].chunks_exact(4) {
        let word = u32::from_le_bytes(chunk.try_into().expect("word chunk"));
        words.push(word);
        if word == encoding::wide::encode_halt() {
            break;
        }
    }

    let bne_index = words
        .iter()
        .position(|word| ((word >> 24) as u8) == instruction::wide::control::BNE)
        .expect("expected BNE in lowered branch");

    assert!(
        words.len() > bne_index + 2,
        "BNE should be followed by two JAL instructions"
    );

    let bne_word = words[bne_index];
    let imm = (bne_word & 0xFF) as u8 as i8;
    assert_eq!(
        imm, 2,
        "BNE should skip the immediate JAL with offset 2 to support wide immediates"
    );

    let jal_else = words[bne_index + 1];
    let jal_then = words[bne_index + 2];
    assert_eq!(
        (jal_else >> 24) as u8,
        instruction::wide::control::JAL,
        "fallthrough path must jump to the else block"
    );
    assert_eq!(
        (jal_then >> 24) as u8,
        instruction::wide::control::JAL,
        "taken path must jump to the then block"
    );
}

#[test]
fn compile_poseidon2_and_assert_eq() {
    // poseidon2 computation
    let src = "fn f(a, b) { let h = poseidon2(a, b); }";
    let code = Compiler::new().compile_source(src).expect("compile failed");
    assert!(!code.is_empty());

    // assert_eq succeeds
    let src = "fn g(a, b) { assert_eq(a, b); }";
    let code = Compiler::new().compile_source(src).expect("compile failed");

    let (meta, _) = parse_meta_offset(&code).unwrap();
    assert!(meta.mode & 0x01 != 0);

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
    let src = "fn f(a: int, b: int) -> (int, int) { let p = pubkgen(a); let c = valcom(a, b); return (p, c); }";
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
        let mut src = String::from("fn main(a: int, b: int) -> int {\n");
        for i in 0..count {
            let value = (i + 1) as i64;
            src.push_str(&format!("  let v{i} = {value};\n"));
        }
        src.push_str("  let c = valcom(a, b);\n");
        src.push_str("  let p = pubkgen(c);\n");
        src.push_str("  let sum = 0;\n");
        for i in 0..count {
            src.push_str(&format!("  sum = sum + v{i};\n"));
        }
        src.push_str("  return sum + p + c;\n}\n");
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
fn json_encode_decode_spills_are_handled() {
    use ivm::kotodama::ir::Instr;
    use ivm::kotodama::regalloc;

    let build_src = |count: usize| {
        let mut src = String::from("fn main(b: bytes) -> int {\n");
        for i in 0..count {
            let value = (i + 1) as i64;
            src.push_str(&format!("  let v{i} = {value};\n"));
        }
        src.push_str("  let j = decode_json(b);\n");
        src.push_str("  let sum = 0;\n");
        for i in 0..count {
            src.push_str(&format!("  sum = sum + v{i};\n"));
        }
        src.push_str("  let encoded = encode_json(j);\n");
        src.push_str("  let val = decode_int(encoded);\n");
        src.push_str("  return sum + val;\n}\n");
        src
    };

    let mut chosen = None;
    let mut count = 20usize;
    while count <= 80 {
        let src = build_src(count);
        let prog = parse(&src).expect("parse json spill");
        let typed = analyze(&prog).expect("analyze json spill");
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
                    Instr::JsonDecode { dest, blob } => {
                        if is_spilled(dest) || is_spilled(blob) {
                            saw_spill = true;
                        }
                    }
                    Instr::JsonEncode { dest, json } => {
                        if is_spilled(dest) || is_spilled(json) {
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

    let src = chosen.expect("expected json encode/decode spill; adjust pressure if needed");
    Compiler::new()
        .compile_source(&src)
        .expect("compile json spill");
}

#[test]
fn compile_poseidon6_error() {
    let src = "fn g(a,b,c,d,e,f) { let h = poseidon6(a,b,c,d,e,f); }";
    let err = Compiler::new().compile_source(src).unwrap_err();
    assert!(err.contains("POSEIDON6 not supported"));
}

#[test]
fn compile_and_run_foreach_map_single_iteration() {
    // Executes body once, binding (k,v) and returning v
    let src = r#"fn f(m: Map<int, int>) -> int { for (k, v) in m { return v; } return 0; }"#;
    let err = Compiler::new().compile_source(src).unwrap_err();
    assert!(err.contains("E_UNBOUNDED_ITERATION"));
}

#[test]
fn compile_and_run_foreach_map_two_buckets_sum() {
    // Sum values from two buckets deterministically
    let src = r#"fn sum(m: Map<int, int>) -> int { let acc = 0; for (k, v) in m { acc = acc + v; } return acc; }"#;
    let err = Compiler::new().compile_source(src).unwrap_err();
    assert!(err.contains("E_UNBOUNDED_ITERATION"));
}

#[test]
fn map_new_allocates() {
    // Expect non-zero pointer returned by Map::new()
    let src = "fn make() -> int { return Map::new(); }";
    let code = Compiler::new().compile_source(src).expect("compile failed");
    let (meta, _off) = parse_meta_offset(&code).unwrap();
    assert_eq!(meta.mode & 0x01, 0); // no ZK flag required

    // Debug: dump first 8 instruction words after header
    let (_, off) = parse_meta_offset(&code).unwrap();
    let mut words = Vec::new();
    for i in 0..8 {
        let start = off + i * 4;
        if start + 4 <= code.len() {
            let w = u32::from_le_bytes(code[start..start + 4].try_into().unwrap());
            words.push(w);
        }
    }
    eprintln!(
        "Map::new first words: {:?}",
        words
            .iter()
            .map(|w| format!("0x{w:08x}"))
            .collect::<Vec<_>>()
    );

    let mut vm = ivm::IVM::new(u64::MAX);
    vm.load_program(&code).unwrap();
    vm.run().expect("execution failed");
    let map_ptr = vm.register(10);
    assert_ne!(map_ptr, 0);
    let mut buf = [0u8; 8];
    vm.load_bytes(map_ptr, &mut buf)
        .expect("load Map::new key bytes");
    let key = u64::from_le_bytes(buf);
    vm.load_bytes(map_ptr + 8, &mut buf)
        .expect("load Map::new value bytes");
    let value = u64::from_le_bytes(buf);
    assert_eq!(key, 0);
    assert_eq!(value, 0);
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
    let src = "fn f(a: AccountId, b: AssetDefinitionId, c: int){ mint_asset(a,b,c); }";
    let prog = parse(src).expect("parse failed");
    let typed = analyze(&prog).expect("semantic analysis failed");
    let ir = ivm::kotodama::ir::lower(&typed).expect("lower");
    let instrs = &ir.functions[0].blocks[0].instrs;
    assert!(instrs.iter().any(|i| matches!(i, Instr::MintAsset { .. })));
}

#[test]
fn parse_transfer_asset_builtin() {
    use ivm::kotodama::ir::Instr;
    let src = "fn f(a: AccountId, b: AccountId, c: AssetDefinitionId, d: int){ transfer_asset(a,b,c,d); }";
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
    let src = "fn f(a: AccountId, b: AccountId, c: AssetDefinitionId, d: int){ transfer_batch((a,b,c,d), (b,a,c,d)); }";
    let prog = parse(src).expect("parse failed");
    let typed = analyze(&prog).expect("semantic analysis failed");
    let ir = ivm::kotodama::ir::lower(&typed).expect("lower");
    let instrs = &ir.functions[0].blocks[0].instrs;
    let begin_idx = instrs
        .iter()
        .position(|i| matches!(i, Instr::TransferBatchBegin))
        .expect("begin not emitted");
    let end_idx = instrs
        .iter()
        .position(|i| matches!(i, Instr::TransferBatchEnd))
        .expect("end not emitted");
    assert!(
        begin_idx < end_idx,
        "batch begin must precede batch end (begin={begin_idx}, end={end_idx})"
    );
    let transfer_count = instrs
        .iter()
        .filter(|i| matches!(i, Instr::TransferAsset { .. }))
        .count();
    assert_eq!(
        transfer_count, 2,
        "expected two transfer calls inside batch"
    );
}

#[test]
fn transfer_batch_requires_entries() {
    let src = "fn f(){ transfer_batch(); }";
    let prog = parse(src).expect("parse failed");
    assert!(
        analyze(&prog).is_err(),
        "an empty transfer_batch call must be rejected"
    );
}

#[test]
fn transfer_batch_requires_tuple_entries() {
    let src = "fn f(a: AccountId){ transfer_batch(a); }";
    let prog = parse(src).expect("parse failed");
    assert!(
        analyze(&prog).is_err(),
        "non-tuple transfer_batch entries must be rejected"
    );
}

#[test]
fn parse_burn_asset_builtin() {
    use ivm::kotodama::ir::Instr;
    let src = "fn f(a: AccountId, b: AssetDefinitionId, c: int){ burn_asset(a,b,c); }";
    let prog = parse(src).expect("parse failed");
    let typed = analyze(&prog).expect("semantic analysis failed");
    let ir = ivm::kotodama::ir::lower(&typed).expect("lower");
    let instrs = &ir.functions[0].blocks[0].instrs;
    assert!(instrs.iter().any(|i| matches!(i, Instr::BurnAsset { .. })));
}

#[test]
fn parse_register_asset_builtin() {
    use ivm::kotodama::ir::Instr;
    let src = "fn f(){ register_asset(\"x\", \"X\", 1, 0); }";
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
    let src = "fn f(){ create_new_asset(\"x\", \"X\", 1, 2, 0); }";
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
    let compiler = Compiler::new();
    let samples_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../kotodama_lang/src/samples");
    // Compile a curated subset of samples supported by the current compiler
    let files = [
        "asset_ops.ko",
        "mint_rose_trigger.ko",
        "query_assets_and_save_cursor.ko",
        "smart_contract_can_filter_queries.ko",
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
    let src = "fn f(a: int, b: bool) { let c = -a; let d = !b; }";
    Compiler::new()
        .compile_source(src)
        .expect("compile unary ops");
}

#[test]
fn std_map_aliases_and_iter_helpers_parse_and_type() {
    // Ensure std::map::* aliases normalize and typecheck
    let src = r#"
        fn f(m: Map<int,int>) {
            let a = std::map::has(m, 1);
            let b = std::map::get_or_insert_default(m, 2);
            let c = std::map::keys_take2(m, 0, 1);
            let d = std::map::values_take2(m, 0, 0);
        }
    "#;
    let prog = parse(src).expect("parse std::map aliases");
    analyze(&prog).expect("analyze std::map aliases");
}

#[test]
fn ir_lower_has_alias_ephemeral() {
    // has(m,k) should lower like contains(m,k) on non-state param maps: MapLoadPair + Eq
    use ivm::kotodama::ir::Instr;
    let src = "fn f(m: Map<int,int>, k: int) { let x = has(m,k); }";
    let prog = parse(src).expect("parse has");
    let typed = analyze(&prog).expect("analyze has");
    let ir = ivm::kotodama::ir::lower(&typed).expect("lower");
    let f = &ir.functions[0];
    let mut saw_load_pair = false;
    let mut saw_eq = false;
    for ins in &f.blocks[0].instrs {
        match ins {
            Instr::MapLoadPair { .. } => saw_load_pair = true,
            Instr::Binary { op, .. } if *op == ivm::kotodama::ast::BinaryOp::Eq => saw_eq = true,
            _ => {}
        }
    }
    assert!(saw_load_pair && saw_eq);
}

#[test]
fn ir_lower_get_or_insert_default_ephemeral() {
    // get_or_insert_default on non-state param map should emit MapLoadPair, Branch, MapSet
    use ivm::kotodama::ir::{Instr, Terminator};
    let src = "fn f(m: Map<int,int>, k: int) -> int { return get_or_insert_default(m, k); }";
    let prog = parse(src).expect("parse get_or_insert_default");
    let typed = analyze(&prog).expect("analyze get_or_insert_default");
    let ir = ivm::kotodama::ir::lower(&typed).expect("lower");
    let f = &ir.functions[0];
    let mut saw_pair = false;
    let mut saw_set = false;
    let mut saw_branch = false;
    for bb in &f.blocks {
        for ins in &bb.instrs {
            match ins {
                Instr::MapLoadPair { .. } => saw_pair = true,
                Instr::MapSet { .. } => saw_set = true,
                _ => {}
            }
        }
        if matches!(bb.terminator, Terminator::Branch { .. }) {
            saw_branch = true;
        }
    }
    assert!(saw_pair && saw_set && saw_branch);
}

#[test]
fn semantic_get_or_insert_default_pointer_requires_explicit_default() {
    let src = "fn f(m: Map<int, Name>) { let _ = get_or_insert_default(m, 1); }";
    let prog = parse(src).expect("parse pointer map without default");
    let err =
        analyze(&prog).expect_err("pointer-valued get_or_insert_default should require default");
    assert!(
        err.message
            .contains("requires an explicit default for pointer-valued maps")
    );
}

#[test]
fn semantic_get_or_insert_default_non_int_requires_explicit_default() {
    let src = "fn f(m: Map<int, bool>) { let _ = get_or_insert_default(m, 1); }";
    let prog = parse(src).expect("parse bool map without default");
    let err = analyze(&prog).expect_err("non-int map should require explicit default");
    assert!(
        err.message
            .contains("auto-default is only available for Map<*,int>")
    );
}

#[test]
fn ir_lower_get_or_insert_default_pointer_variants_use_pointer_syscalls() {
    use ivm::kotodama::ir::Instr;
    let cases = [
        ("Name", r#"name("alias")"#),
        (
            "AccountId",
            r#"account_id("6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn")"#,
        ),
        (
            "AssetDefinitionId",
            r#"asset_definition("62Fk4FPcMuLvW5QjDGNF2a4jAmjM")"#,
        ),
        ("DomainId", r#"domain("wonderland")"#),
        ("NftId", r#"nft_id("rose:uuid:0123$wonderland")"#),
    ];
    for (ty, ctor) in cases {
        let src = format!(
            r#"
        seiyaku C {{
            state S: Map<int, {ty}>;
            fn hajimari() -> {ty} {{
                return get_or_insert_default(S, 7, {ctor});
            }}
        }}
        "#
        );
        let prog = parse(&src).expect("parse pointer durable map");
        let typed = analyze(&prog).expect("analyze pointer durable map");
        let ir = ivm::kotodama::ir::lower(&typed).expect("lower");
        let func = ir
            .functions
            .iter()
            .find(|f| f.name == "hajimari")
            .expect("hajimari lowered");
        let mut saw_pointer_to = false;
        let mut saw_pointer_from = false;
        for bb in &func.blocks {
            for ins in &bb.instrs {
                match ins {
                    Instr::PointerToNorito { .. } => saw_pointer_to = true,
                    Instr::PointerFromNorito { .. } => saw_pointer_from = true,
                    _ => {}
                }
            }
        }
        assert!(
            saw_pointer_to,
            "durable else branch should encode pointer defaults for {ty}"
        );
        assert!(
            saw_pointer_from,
            "durable then branch should decode stored pointer for {ty}"
        );
    }
}

#[test]
fn ir_lower_keys_values_take2_ephemeral() {
    // keys_take2/values_take2 on non-state map param should compute dynamic addr and load key/value
    use ivm::kotodama::ir::Instr;
    let src = r#"
        fn g(m: Map<int,int>) -> int { return keys_take2(m, 0, 1); }
    "#;
    let prog = parse(src).expect("parse keys_take2");
    let typed = analyze(&prog).expect("analyze keys_take2");
    let ir = ivm::kotodama::ir::lower(&typed).expect("lower");
    let f = &ir.functions[0];
    let mut saw_load0 = false;
    let mut saw_load8 = false;
    for ins in &f.blocks[0].instrs {
        if let Instr::Load64Imm { imm, .. } = ins {
            if *imm == 0 {
                saw_load0 = true;
            }
            if *imm == 8 {
                saw_load8 = true;
            }
        }
    }
    assert!(saw_load0 && saw_load8);
}

#[test]
fn ir_lower_keys_values_take2_tuple_access() {
    // keys_values_take2 returns a tuple; accessing .0/.1 should lower to LOAD64s
    use ivm::kotodama::ir::Instr;
    let src = r#"
        fn f(m: Map<int,int>) { let t = std::map::keys_values_take2(m, 0, 1); let a = t.0; let b = t.1; }
    "#;
    let prog = parse(src).expect("parse keys_values_take2");
    let typed = analyze(&prog).expect("analyze keys_values_take2");
    let ir = ivm::kotodama::ir::lower(&typed).expect("lower");
    let f = &ir.functions[0];
    let mut saw0 = false;
    let mut saw8 = false;
    for ins in &f.blocks[0].instrs {
        if let Instr::Load64Imm { imm, .. } = ins {
            if *imm == 0 {
                saw0 = true;
            }
            if *imm == 8 {
                saw8 = true;
            }
        }
    }
    assert!(saw0 && saw8);
}

#[test]
fn ir_tuple_pack_and_get_general() {
    use ivm::kotodama::ir::Instr;
    let src = r#"
        fn f(a: int, b: int) {
            let t = (a, b);
            let x = t.0;
            let y = t.1;
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
fn runtime_durable_get_or_insert_default_state_map() {
    // Durable path: Map<int,int> declared in state; first call inserts 0; second returns 0 without inserting again.
    use std::collections::HashMap;

    use ivm::{
        IVM, PointerType,
        kotodama::compiler::Compiler,
        mock_wsv::{MockWorldStateView, ScopedAccountId, WsvHost},
        validate_tlv_bytes,
    };
    let src = r#"
        seiyaku C {
            state S: Map<int,int>;
            fn hajimari() {
                let x = get_or_insert_default(S, 7);
                assert(x == 0);
                let y = get_or_insert_default(S, 7);
                assert(y == 0);
            }
        }
    "#;
    let code = Compiler::new()
        .compile_source(src)
        .expect("compile durable gid");
    let mut vm = IVM::new(u64::MAX);
    vm.load_program(&code).expect("load");
    let wsv = MockWorldStateView::new();
    let alice: ScopedAccountId = ScopedAccountId::new(
        "wonderland".parse().expect("domain id"),
        "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
            .parse()
            .expect("public key"),
    );
    let host =
        WsvHost::new_with_subject(wsv, ivm::mock_wsv::AccountId::from(&alice), HashMap::new());
    vm.set_host(host);
    // Run hajimari
    vm.run().expect("exec");
    // Verify durable state was set at path "S/7"
    let host_ref = vm.host_mut_any().unwrap();
    let host = host_ref.downcast_ref::<WsvHost>().unwrap();
    let base = iroha_data_model::prelude::Name::from_str("S").expect("valid Name literal");
    let expected_path = format!("{}/{}", base.as_ref(), 7);
    let mut val = host.wsv.sc_get(&expected_path);
    if val.is_none() {
        // Durable state maps produced by std::map lowerings namespace entries with a sentinel
        // prefix: 0x01 followed by seven zero bytes, then the "<base>/<key>" path.
        let namespaced_path = format!("{}\0\0\0\0\0\0\0{}", char::from(0x01), expected_path);
        val = host.wsv.sc_get(&namespaced_path);
    }
    let val = val.expect("durable state entry should exist");
    let tlv = validate_tlv_bytes(&val).expect("state entry should use NoritoBytes TLV");
    assert_eq!(tlv.type_id, PointerType::NoritoBytes);
    let stored: i64 =
        norito::decode_from_bytes(tlv.payload).expect("durable int value should be Norito i64");
    assert_eq!(stored, 0);
}

#[test]
fn vrf_and_pointer_syscalls_present() {
    let src = r#"
        fn main() {
            let input = blob("msg");
            let pk = blob("pk");
            let proof = blob("proof");
            let out = vrf_verify(input, pk, proof, 2);
            let _batch = vrf_verify_batch(norito_bytes("batch"));
            let _encoded = pointer_to_norito(out);
            let _schema = schema_info(name("example.schema"));
            let _handle = asset_handle(norito_bytes("0x00"));
            let _ds = dataspace_id(norito_bytes("0x00"));
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
    assert!(has(syscalls::SYSCALL_POINTER_TO_NORITO as u8));
    assert!(has(syscalls::SYSCALL_POINTER_FROM_NORITO as u8));
    assert!(has(syscalls::SYSCALL_SCHEMA_INFO as u8));
}

#[test]
fn axt_intrinsics_lower_to_syscalls() {
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
            account: "6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn".to_string(),
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
        fn main() {{
            let ds = dataspace_id("{dsid}");
            let desc = axt_descriptor("0x{desc_hex}");
            let handle = asset_handle("0x{handle_hex}");
            let proof = proof_blob("0x{proof_hex}");
            axt_begin(desc);
            axt_touch(ds, norito_bytes("manifest"));
            verify_ds_proof(ds, proof);
            use_asset_handle(handle, norito_bytes("intent"), proof);
            axt_commit();
        }}
    "#
    );
    let code = ivm::KotodamaCompiler::new()
        .compile_source(&src)
        .expect("compile axt intrinsics");
    let (_meta, off) = parse_meta_offset(&code).unwrap();
    let bytes = &code[off..];
    let scall = instruction::wide::system::SCALL;
    let has = |imm: u8| {
        bytes
            .windows(4)
            .any(|w| w == encoding::wide::encode_sys(scall, imm).to_le_bytes())
    };
    assert!(has(syscalls::SYSCALL_AXT_BEGIN as u8));
    assert!(has(syscalls::SYSCALL_AXT_TOUCH as u8));
    assert!(has(syscalls::SYSCALL_VERIFY_DS_PROOF as u8));
    assert!(has(syscalls::SYSCALL_USE_ASSET_HANDLE as u8));
    assert!(has(syscalls::SYSCALL_AXT_COMMIT as u8));
}

#[test]
fn semantic_return_mismatch_unit() {
    let src = "fn f() -> () { return 1; }";
    let prog = parse(src).expect("parse");
    let err = analyze(&prog).unwrap_err();
    assert!(err.message.contains("return type mismatch"));
}
