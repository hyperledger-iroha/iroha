//! Iroha client CLI executable.
fn main() -> std::process::ExitCode {
    iroha_cli::main_entry(iroha_core::compiled_build_metadata!())
}
