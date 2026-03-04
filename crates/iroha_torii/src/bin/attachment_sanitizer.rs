//! Attachment sanitizer subprocess entrypoint for tests and tooling.

fn main() {
    if let Some(exit_code) = iroha_torii::zk_attachments::sanitizer_process_exit_code_from_env() {
        std::process::exit(exit_code);
    }
    eprintln!("attachment sanitizer must be invoked with IROHA_ATTACHMENT_SANITIZER=1");
    std::process::exit(1);
}
