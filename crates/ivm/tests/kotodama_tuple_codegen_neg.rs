//! Codegen negative test: returning 9 values should fail with a clear message.

#[test]
fn compile_function_returning_nine_values_fails() {
    use ivm::kotodama::compiler::Compiler;
    // A function returning a 9-tuple. Codegen should reject with an error.
    let src = r#"
        fn h(a:int,b:int,c:int,d:int,e:int,f:int,g:int,h:int,i:int)
            -> (int,int,int,int,int,int,int,int,int) {
            return (a,b,c,d,e,f,g,h,i);
        }
    "#;
    let err = Compiler::new()
        .compile_source(src)
        .expect_err("expected compile error for 9-value return");
    assert!(
        err.contains("too many return values"),
        "unexpected error message: {err}"
    );
}

#[test]
fn callmulti_with_nine_returns_fails() {
    // Hit the CallMulti codegen guard via test helper without compiling a callee.
    let err = ivm::kotodama::compiler::test_helpers::try_emit_callmulti_guard_only(9)
        .expect_err("expected CallMulti guard error");
    assert!(
        err.contains("too many return values in call"),
        "unexpected error message: {err}"
    );
}

#[test]
fn compile_function_returning_eight_values_succeeds() {
    use ivm::kotodama::compiler::Compiler;
    let src = r#"
        fn h(a:int,b:int,c:int,d:int,e:int,f:int,g:int,h:int)
            -> (int,int,int,int,int,int,int,int) {
            return (a,b,c,d,e,f,g,h);
        }
    "#;
    Compiler::new()
        .compile_source(src)
        .expect("expected 8-value return to compile");
}

#[test]
fn callmulti_with_eight_returns_succeeds() {
    ivm::kotodama::compiler::test_helpers::try_emit_callmulti_guard_only(8)
        .expect("expected 8-value CallMulti to pass guard");
}
