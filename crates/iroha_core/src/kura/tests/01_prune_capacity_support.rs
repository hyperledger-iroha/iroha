// Original canonical-transition panic isolation and actual DA decode bounds.
fn run_prune_crash_test_in_subprocess() -> bool {
    const CHILD_TEST: &str = "IROHA_KURA_PRUNE_CRASH_CHILD_TEST";
    let current_thread = std::thread::current();
    let test_name = current_thread.name().expect("named prune test thread");
    if let Some(child_test) = std::env::var_os(CHILD_TEST) {
        assert_eq!(child_test, test_name, "run only the selected crash fixture");
        return false;
    }
    let output =
        std::process::Command::new(std::env::current_exe().expect("current Core test executable"))
            .args(["--exact", test_name, "--test-threads=1", "--nocapture"])
            .env(CHILD_TEST, test_name)
            .output()
            .expect("run isolated canonical-prune crash fixture");
    assert!(
        output.status.success(),
        "isolated prune fixture {test_name} failed: {}\n{}\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    true
}

#[test]
fn recovery_control_files_reject_cap_plus_one_before_decode() {
    fn create_sparse(path: &std::path::Path, len: u64) {
        std::fs::File::create(path)
            .expect("create oversized recovery control file")
            .set_len(len)
            .expect("size oversized recovery control file");
    }
    let kura = super::Kura::blank_kura_for_testing();
    {
        let block_store = kura.block_store.lock();
        let rewrite_path = block_store.da_block_rewrite_stage_path();
        create_sparse(&rewrite_path, super::MAX_DA_BLOCK_REWRITE_STAGE_BYTES + 1);
        assert!(
            block_store.read_da_block_rewrite_stage().is_err(),
            "DA rewrite stage must reject cap-plus-one metadata before reading"
        );
        std::fs::remove_file(&rewrite_path).expect("remove oversized DA rewrite stage");
    }
}
