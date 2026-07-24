//! Codegen negative test: returning 14 values should fail with a clear message.

#[test]
fn compile_function_returning_fourteen_values_fails() {
    use ivm::kotodama::compiler::Compiler;
    // A function returning a 14-tuple. Codegen should reject with an error.
    let src = r#"
        seiyaku TooManyReturns {
            view fn h()
                -> (int,int,int,int,int,int,int,int,int,int,int,int,int,int) {
                return (1,2,3,4,5,6,7,8,9,10,11,12,13,14);
            }
        }
    "#;
    let err = Compiler::new()
        .compile_source(src)
        .expect_err("expected compile error for 14-value return");
    assert!(
        err.contains("too many return values"),
        "unexpected error message: {err}"
    );
}

#[test]
fn callmulti_with_fourteen_returns_fails() {
    // Hit the CallMulti codegen guard via test helper without compiling a callee.
    let err = ivm::kotodama::compiler::test_helpers::try_emit_callmulti_guard_only(14)
        .expect_err("expected CallMulti guard error");
    assert!(
        err.contains("too many return values in call"),
        "unexpected error message: {err}"
    );
}

#[test]
fn compile_function_returning_thirteen_values_succeeds() {
    use ivm::kotodama::compiler::Compiler;
    let src = r#"
        seiyaku MaximumReturns {
            view fn h(int a,int b,int c,int d,int e,int f,int g,int eighth,int i,int j,int k,int l,int m)
                -> (int,int,int,int,int,int,int,int,int,int,int,int,int) {
                return (a,b,c,d,e,f,g,eighth,i,j,k,l,m);
            }
        }
    "#;
    Compiler::new()
        .compile_source(src)
        .expect("expected 13-value return to compile");
}

#[test]
fn callmulti_with_thirteen_returns_succeeds() {
    ivm::kotodama::compiler::test_helpers::try_emit_callmulti_guard_only(13)
        .expect("expected 13-value CallMulti to pass guard");
}
