//! Compile-fail coverage for removed first-class durable-state parameters.

use ivm::kotodama::compiler::Compiler as KotodamaCompiler;

fn assert_state_parameter_rejected(source: &str) {
    let error = KotodamaCompiler::new()
        .compile_source(source)
        .expect_err("V1 must reject first-class state handles");
    assert!(
        error.contains("state handles are not first-class parameters"),
        "unexpected diagnostic: {error}"
    );
}

#[test]
fn scalar_state_parameter_is_rejected() {
    assert_state_parameter_rejected(
        r#"
        module RemovedScalarStateParameter {
            fn read_counter(state int value) -> int { return value; }
        }
        "#,
    );
}

#[test]
fn state_map_parameter_is_rejected() {
    assert_state_parameter_rejected(
        r#"
        module RemovedMapStateParameter {
            fn read_value(state StateMap<int, int> values, int key) -> int {
                return values.get(key).unwrap_or(0);
            }
        }
        "#,
    );
}
