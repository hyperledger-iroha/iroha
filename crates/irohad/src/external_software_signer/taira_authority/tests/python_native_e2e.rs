#[cfg(target_os = "linux")]
mod linux {
    use std::{
        ffi::OsString,
        fs::{self, OpenOptions},
        io::Write as _,
        os::{
            fd::AsRawFd as _,
            unix::fs::{MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _},
        },
        path::{Path, PathBuf},
        process::{Command, Stdio},
    };

    const NAMESPACE_CHILD_ENV_V1: &str = "IROHA_TAIRA_PYTHON_NATIVE_NAMESPACE_CHILD_V1";
    const ADAPTER_ARG_COUNT_ENV_V1: &str = "IROHA_TAIRA_TEST_ADAPTER_ARG_COUNT_V1";
    const ADAPTER_ARG_PREFIX_ENV_V1: &str = "IROHA_TAIRA_TEST_ADAPTER_ARG_V1_";
    const ADAPTER_OUTPUT_ENV_V1: &str = "IROHA_TAIRA_TEST_ADAPTER_OUTPUT_V1";
    const ADAPTER_TEST_NAME_V1: &str = concat!(
        "external_software_signer::taira_authority::tests::python_native_e2e::linux::",
        "native_cli_adapter_child_v1"
    );
    const NAMESPACE_TEST_NAME_V1: &str = concat!(
        "external_software_signer::taira_authority::tests::python_native_e2e::linux::",
        "python_native_namespace_child_v1"
    );
    const FIXED_VERIFIER_V1: &str = "/usr/libexec/iroha/taira_release_authority";
    const COPIED_TEST_BINARY_V1: &str = "/run/iroha/taira-authority-test-binary-v1";
    const ADAPTER_RESULT_DIRECTORY_V1: &str = "/run/iroha/taira-adapter-results-v1";
    const DRIVER_WORK_DIRECTORY_V1: &str = "/run/iroha/taira-python-native-e2e-v1";
    const DRIVER_SETUP_V1: &str = "/run/iroha/taira-python-native-e2e-setup-v1.json";

    #[allow(unsafe_code)]
    fn replace_stdout(descriptor: i32) -> std::io::Result<()> {
        use std::os::raw::c_int;

        unsafe extern "C" {
            fn dup2(old_descriptor: c_int, new_descriptor: c_int) -> c_int;
        }
        // SAFETY: `descriptor` is owned by the live `File` in the caller and
        // standard output is a valid process descriptor.
        if unsafe { dup2(descriptor, 1) } == -1 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(())
    }

    #[test]
    fn native_cli_adapter_child_v1() {
        let Some(raw_count) = std::env::var_os(ADAPTER_ARG_COUNT_ENV_V1) else {
            return;
        };
        let count = raw_count
            .into_string()
            .expect("adapter argument count is ASCII")
            .parse::<usize>()
            .expect("adapter argument count is canonical");
        let mut arguments = Vec::with_capacity(count + 1);
        arguments.push(OsString::from("taira_release_authority"));
        for index in 0..count {
            arguments.push(
                std::env::var_os(format!("{ADAPTER_ARG_PREFIX_ENV_V1}{index}"))
                    .expect("numbered adapter argument"),
            );
        }
        let output_path = std::env::var_os(ADAPTER_OUTPUT_ENV_V1)
            .map(PathBuf::from)
            .expect("adapter output path");
        let output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(output_path)
            .expect("create isolated adapter output");
        replace_stdout(output.as_raw_fd()).expect("redirect adapter output");
        let status = match super::super::super::transport::run_cli_args_for_test(arguments) {
            Ok(()) => 0,
            Err(message) => {
                eprintln!("{message}");
                1
            }
        };
        std::io::stdout().flush().expect("flush adapter output");
        std::process::exit(status);
    }

    fn run_checked(command: &mut Command, label: &str) -> std::process::Output {
        let output = command.output().unwrap_or_else(|error| {
            panic!("cannot launch {label}: {error}");
        });
        assert!(
            output.status.success(),
            "{label} failed: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        output
    }

    fn mount_tmpfs(target: &str) {
        let _ = run_checked(
            Command::new("/usr/bin/mount").args([
                "-t",
                "tmpfs",
                "-o",
                "mode=0755,nosuid,nodev",
                "tmpfs",
                target,
            ]),
            &format!("mount isolated {target}"),
        );
    }

    fn prepare_root_owned_installation_chain(root: &str) {
        let root = Path::new(root);
        fs::create_dir_all(root).expect("create fixed authority installation root");
        let mut current = Some(root);
        while let Some(path) = current {
            if matches!(path.to_str(), Some("/etc" | "/run" | "/var/lib")) {
                break;
            }
            fs::set_permissions(path, fs::Permissions::from_mode(0o755))
                .expect("make fixed authority installation root traversable");
            current = path.parent();
        }
    }

    fn install_adapter(test_binary: &Path) {
        fs::create_dir_all("/usr/libexec/iroha").expect("create isolated verifier directory");
        fs::set_permissions("/usr/libexec/iroha", fs::Permissions::from_mode(0o755))
            .expect("make isolated verifier directory traversable");
        fs::create_dir_all("/run/iroha").expect("create isolated runtime parent");
        fs::set_permissions("/run/iroha", fs::Permissions::from_mode(0o755))
            .expect("make isolated runtime parent traversable");
        fs::copy(test_binary, COPIED_TEST_BINARY_V1).expect("copy isolated test binary");
        fs::set_permissions(COPIED_TEST_BINARY_V1, fs::Permissions::from_mode(0o555))
            .expect("make copied test binary executable");
        fs::create_dir_all(ADAPTER_RESULT_DIRECTORY_V1).expect("create adapter result directory");
        fs::set_permissions(
            ADAPTER_RESULT_DIRECTORY_V1,
            fs::Permissions::from_mode(0o1777),
        )
        .expect("make adapter result directory role-accessible");

        let script = format!(
            "#!/bin/sh\n\
             count=$#\n\
             export {ADAPTER_ARG_COUNT_ENV_V1}=\"$count\"\n\
             index=0\n\
             while [ \"$#\" -gt 0 ]; do\n\
               export \"{ADAPTER_ARG_PREFIX_ENV_V1}${{index}}=$1\"\n\
               index=$((index + 1))\n\
               shift\n\
             done\n\
             output=\"{ADAPTER_RESULT_DIRECTORY_V1}/output.$$\"\n\
             error=\"{ADAPTER_RESULT_DIRECTORY_V1}/error.$$\"\n\
             export {ADAPTER_OUTPUT_ENV_V1}=\"$output\"\n\
             if \"{COPIED_TEST_BINARY_V1}\" --exact \"{ADAPTER_TEST_NAME_V1}\" --nocapture \
                 >/dev/null 2>\"$error\"; then\n\
               status=0\n\
             else\n\
               status=$?\n\
             fi\n\
             if [ -f \"$output\" ]; then cat \"$output\"; fi\n\
             if [ \"$status\" -ne 0 ] && [ -f \"$error\" ]; then cat \"$error\" >&2; fi\n\
             rm -f \"$output\" \"$error\"\n\
             exit \"$status\"\n"
        );
        fs::write(FIXED_VERIFIER_V1, script).expect("write fixed verifier adapter");
        fs::set_permissions(FIXED_VERIFIER_V1, fs::Permissions::from_mode(0o555))
            .expect("make fixed verifier adapter executable");
        let metadata =
            fs::symlink_metadata(FIXED_VERIFIER_V1).expect("fixed verifier adapter identity");
        assert!(metadata.is_file());
        assert_eq!(metadata.nlink(), 1);
        assert_eq!(metadata.uid(), 0);
        assert_eq!(metadata.mode() & 0o777, 0o555);
    }

    fn install_driver_setup() {
        let roles = [
            ("native-evidence", 61_000, 61_001, 61_002, "11"),
            ("privacy-protocol-origin", 61_003, 61_004, 61_005, "22"),
            ("privacy-governance", 61_006, 61_007, 61_008, "33"),
            ("qualification", 0, 61_010, 61_011, "44"),
            ("deploy-issuance", 61_012, 61_013, 61_014, "55"),
            ("rollout-observation", 61_015, 61_016, 61_017, "66"),
            ("public-soak-observation", 61_018, 61_019, 61_020, "77"),
            ("public-soak-replay-admission", 61_021, 61_022, 61_023, "88"),
        ];
        let mut role_rows = Vec::with_capacity(roles.len());
        for (role, service_uid, client_uid, administrator_uid, policy_byte) in roles {
            role_rows.push(format!(
                "\"{role}\":{{\"administrator_uid\":{administrator_uid},\"client_uid\":{client_uid},\"policy_sha256\":\"{}\",\"service_uid\":{service_uid}}}",
                policy_byte.repeat(32)
            ));
        }
        let setup = format!(
            "{{\"clocks_unix_millis\":{{\"deploy-issuance\":1900000000000,\"native-evidence\":1900000000000,\"privacy-governance\":1800000000001,\"privacy-protocol-origin\":1900000000000,\"public-soak-observation\":1800000001000,\"public-soak-replay-admission\":1800000003000,\"qualification\":1900000000000,\"rollout-observation\":1900000000000}},\"governance_retained_private_key_hex\":\"ccf31d85e3b32a4bea59987ce0c78e3b8e2db93881468ab2435fe45d5c9dcd53\",\"native_assignment\":{{\"controller_digest\":\"{}\",\"controller_host_id\":\"controller-host-v1\",\"controller_installation_id\":\"controller-installation-v1\",\"run_nonce\":\"{}\"}},\"roles\":{{{}}},\"schema\":\"iroha.taira.python-native-authority-e2e-setup.v1\"}}\n",
            "99".repeat(32),
            "aa".repeat(32),
            role_rows.join(",")
        );
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o400)
            .open(DRIVER_SETUP_V1)
            .expect("create root-owned driver setup");
        file.write_all(setup.as_bytes())
            .and_then(|()| file.sync_all())
            .expect("persist root-owned driver setup");
        fs::create_dir(DRIVER_WORK_DIRECTORY_V1).expect("create empty driver work directory");
        fs::set_permissions(DRIVER_WORK_DIRECTORY_V1, fs::Permissions::from_mode(0o700))
            .expect("protect driver work directory");
    }

    #[test]
    fn python_native_namespace_child_v1() {
        if std::env::var_os(NAMESPACE_CHILD_ENV_V1).is_none() {
            return;
        }
        let _ = run_checked(
            Command::new("/usr/bin/mount").args(["--make-rprivate", "/"]),
            "make mount namespace private",
        );
        // Keep the host loader and mount metadata available until every other
        // isolated tree is mounted; `/etc` is replaced last.
        for target in ["/run", "/var/lib", "/usr/libexec", "/etc"] {
            mount_tmpfs(target);
        }
        for root in [
            "/etc/iroha/taira-authorities/v1",
            "/run/iroha/taira-authorities/v1",
            "/var/lib/iroha/taira-authorities/v1",
        ] {
            prepare_root_owned_installation_chain(root);
        }

        let current_executable = std::env::current_exe().expect("current test executable");
        install_adapter(&current_executable);
        install_driver_setup();
        let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("workspace root");
        let driver = repository.join("scripts/tests/taira_python_native_authority_e2e_driver.py");
        assert!(driver.is_file(), "Python/native E2E driver is installed");
        let output = run_checked(
            Command::new("/usr/bin/python3")
                .arg(&driver)
                .arg("--repo-root")
                .arg(repository)
                .arg("--work-root")
                .arg(DRIVER_WORK_DIRECTORY_V1)
                .arg("--setup")
                .arg(DRIVER_SETUP_V1)
                .env("PYTHONPATH", repository)
                .env("LANG", "C")
                .env("LC_ALL", "C")
                .stdin(Stdio::null()),
            "Python-to-native authority driver",
        );
        assert!(
            String::from_utf8_lossy(&output.stdout)
                .contains("\"qualification_barrier\":\"authenticated-fixed-service\""),
            "driver must authenticate the former qualification barrier"
        );
    }

    #[test]
    fn seven_former_authority_barriers_use_fixed_native_installations_v1() {
        if rustix::process::geteuid().as_raw() != 0 {
            eprintln!("SKIP: Python/native authority namespace test requires Linux root");
            return;
        }
        for required in ["/usr/bin/mount", "/usr/bin/python3", "/usr/libexec"] {
            if !Path::new(required).exists() {
                eprintln!("SKIP: Python/native authority namespace test requires {required}");
                return;
            }
        }
        let unshare = ["/usr/bin/unshare", "/bin/unshare"]
            .into_iter()
            .find(|path| Path::new(path).is_file());
        let Some(unshare) = unshare else {
            eprintln!("SKIP: Python/native authority namespace test requires util-linux unshare");
            return;
        };
        let current_executable = std::env::current_exe().expect("current test executable");
        let output = Command::new(unshare)
            .args(["--mount", "--fork", "--kill-child"])
            .arg(current_executable)
            .args(["--exact", NAMESPACE_TEST_NAME_V1, "--nocapture"])
            .env(NAMESPACE_CHILD_ENV_V1, "1")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .expect("launch private mount namespace");
        if !output.status.success()
            && String::from_utf8_lossy(&output.stderr).contains("Operation not permitted")
        {
            eprintln!(
                "SKIP: Python/native authority namespace test requires mount-namespace capability"
            );
            return;
        }
        assert!(
            output.status.success(),
            "Python/native namespace child failed: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
    }
}

#[cfg(not(target_os = "linux"))]
#[test]
fn seven_former_authority_barriers_use_fixed_native_installations_v1() {
    eprintln!("SKIP: Python/native authority namespace test requires Linux");
}
