//! Ensure retired binding-only IVM proof helpers remain outside the CLI grammar.

use super::command;

#[test]
fn binding_only_ivm_proof_commands_are_rejected() {
    for operation in ["derive", "prove", "get", "delete", "derive-pk"] {
        let output = command()
            .args(["app", "zk", "ivm", operation])
            .output()
            .expect("CLI rejects retired proof helper");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(2), "{operation}: {stderr}");
        assert!(stderr.contains("unrecognized subcommand 'ivm'"), "{stderr}");
        assert!(output.stdout.is_empty(), "retired command emitted output");
    }
}
