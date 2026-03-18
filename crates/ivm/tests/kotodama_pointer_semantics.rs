//! Kotodama pointer-type semantic restrictions: ensure pointer constructors
//! cannot be used as integers in arithmetic.

use ivm::kotodama::{parser::parse, semantic};

#[test]
fn pointer_cannot_participate_in_arithmetic() {
    let src = r#"
        fn main() {
            let k = name("cursor");
            let a = k + 1; // invalid: Name is not int
        }
    "#;
    let ast = parse(src).expect("parse");
    let err = semantic::analyze(&ast).expect_err("analyze should reject pointer arithmetic");
    assert!(err.message.contains("expects int"));
}
