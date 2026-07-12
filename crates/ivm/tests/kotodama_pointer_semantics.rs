//! Kotodama pointer-type semantic restrictions: ensure pointer constructors
//! cannot be used as integers in arithmetic.

use ivm::kotodama::{compiler::Compiler, i18n::Language};

#[test]
fn pointer_cannot_participate_in_arithmetic() {
    let src = r#"
        seiyaku InvalidPointerArithmetic {
        view fn main() {
            let k = Name::parse("cursor");
            let a = k + 1; // invalid: Name is not int
        }
        }
    "#;
    let error = Compiler::new_with_language(Language::English)
        .compile_source(src)
        .expect_err("compile should reject pointer arithmetic");
    assert!(
        error.contains("error[K2003]")
            && error.contains("Add requires identical numeric operand types")
            && error.contains("implicit conversions are not part of Kotodama V1"),
        "unexpected pointer arithmetic diagnostic: {error}"
    );
}
