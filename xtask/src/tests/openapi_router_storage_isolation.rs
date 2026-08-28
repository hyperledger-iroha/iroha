const OPENAPI_READ_ONLY_CWD_CHILD: &str = "IROHA_XTASK_OPENAPI_READ_ONLY_CWD_CHILD";

#[test]
fn router_generation_is_side_effect_free_in_a_read_only_current_directory() {
    let cwd = tempdir().expect("read-only OpenAPI generator current directory");
    let cwd_path = cwd
        .path()
        .canonicalize()
        .expect("canonical current directory");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;

        fs::set_permissions(&cwd_path, fs::Permissions::from_mode(0o555))
            .expect("make OpenAPI generator current directory read-only");
    }

    let output = std::process::Command::new(
        std::env::current_exe().expect("locate the xtask test executable"),
    )
    .args([
        "--exact",
        "openapi_tests::router_generation_read_only_current_directory_child",
        "--nocapture",
    ])
    .env(OPENAPI_READ_ONLY_CWD_CHILD, &cwd_path)
    .current_dir(&cwd_path)
    .output()
    .expect("run isolated OpenAPI router generation child");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;

        fs::set_permissions(&cwd_path, fs::Permissions::from_mode(0o700))
            .expect("restore temporary current-directory permissions");
    }

    assert!(
        output.status.success(),
        "isolated OpenAPI router generation failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        fs::read_dir(&cwd_path)
            .expect("inspect OpenAPI generator current directory")
            .next()
            .is_none(),
        "OpenAPI router generation must not create files under its current directory"
    );
}

#[test]
fn router_generation_read_only_current_directory_child() {
    let Some(expected_cwd) = std::env::var_os(OPENAPI_READ_ONLY_CWD_CHILD) else {
        return;
    };
    assert_eq!(
        std::env::current_dir()
            .expect("read child current directory")
            .canonicalize()
            .expect("canonicalize child current directory"),
        PathBuf::from(expected_cwd)
            .canonicalize()
            .expect("canonicalize expected child current directory")
    );

    let spec = try_generate_router_openapi()
        .expect("generate OpenAPI router document from a read-only current directory")
        .expect("OpenAPI router must return its document");
    assert!(
        !spec.is_empty(),
        "OpenAPI router document must not be empty"
    );
}
