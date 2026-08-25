"""Static fail-closed contract for native Taira privacy evidence capture."""

from __future__ import annotations

import json
from pathlib import Path
import re
import subprocess


ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = ROOT / ".github" / "workflows" / "capture_taira_privacy_native_evidence.yml"
HOST_CHECK = ROOT / "ci" / "check_taira_privacy_native_host.sh"
HOST_PROBE = ROOT / "ci" / "taira_privacy_native_host_probe.c"
RUNNER = ROOT / "crates" / "iroha_test_network" / "src" / "bin" / "taira_privacy_release_runner.rs"
RUNNER_CARGO = ROOT / "crates" / "iroha_test_network" / "Cargo.toml"
INSTALLER = ROOT / "scripts" / "install_taira_privacy_native_expectations.py"
PROFILE = ROOT / "crates" / "iroha_core" / "src" / "privacy_engines" / "zk_x509" / "profile.rs"
READINESS = PROFILE.parent / "profile" / "readiness_certificates.rs"
ROLLOUT = ROOT / "configs" / "soranexus" / "taira" / "privacy_rollout_plan_v1.json"


def test_capture_corridor_is_registered_and_open_state_is_reported_honestly() -> None:
    for required in (WORKFLOW, HOST_CHECK, HOST_PROBE, RUNNER, INSTALLER):
        assert required.is_file() and required.stat().st_size > 0, required

    cargo = RUNNER_CARGO.read_text(encoding="utf-8")
    assert 'privacy-release-evidence = ["iroha_core/privacy-release-evidence"]' in cargo
    assert 'name = "taira_privacy_release_runner"' in cargo
    assert 'required-features = ["privacy-release-evidence"]' in cargo

    profile = PROFILE.read_text(encoding="utf-8")
    readiness = READINESS.read_text(encoding="utf-8")
    zero_declarations = (
        "ZK_X509_RELEASE_KAT_EXPECTED_PROOF_BYTES_V1: u32 = 0;",
        "ZK_X509_RELEASE_KAT_EXPECTED_PROOF_SHA256_V1: [u8; 32] = [0; 32];",
        "ZK_X509_NATIVE_RELEASE_EXPECTATIONS_NORITO_SHA256_V1: [u8; 32] = [0; 32];",
        "ZK_X509_NATIVE_RELEASE_EXPECTATIONS_JSON_SHA256_V1: [u8; 32] = [0; 32];",
    )
    zero_resource_declarations = (
        "ZK_X509_RESOURCE_POSITIVE_ELAPSED_MILLIS_V1: u64 = 0;",
        "ZK_X509_RESOURCE_POSITIVE_PEAK_RSS_BYTES_V1: u64 = 0;",
        "ZK_X509_RESOURCE_POSITIVE_PEAK_ADDRESS_SPACE_BYTES_V1: u64 = 0;",
        "ZK_X509_RESOURCE_MAXIMUM_ELAPSED_MILLIS_V1: u64 = 0;",
        "ZK_X509_RESOURCE_MAXIMUM_PEAK_RSS_BYTES_V1: u64 = 0;",
        "ZK_X509_RESOURCE_MAXIMUM_PEAK_ADDRESS_SPACE_BYTES_V1: u64 = 0;",
        "ZK_X509_RESOURCE_CERTIFICATE_SHA256_V1: [u8; 32] = [0; 32];",
    )
    open_fields = [declaration in profile for declaration in zero_declarations]
    open_fields.extend(declaration in readiness for declaration in zero_resource_declarations)
    assert all(open_fields) or not any(open_fields), "ZK-X509 capture pins are partially installed"

    rollout = json.loads(ROLLOUT.read_text(encoding="utf-8"))
    entry = next(
        protocol
        for protocol in rollout["protocols"]
        if protocol["label"] == "iroha-zk-x509-stark-p256-v0"
    )
    if all(open_fields):
        assert entry["assurance"] == "unavailable"
        assert entry["missing_evidence"] == [
            "MissingZkX509ProductionKatAndResourcePins"
        ]
        for fixture in (
            "native_release_expectations_v1.norito",
            "native_release_expectations_v1.json",
            "zk_x509_native_resource_v1.norito",
            "zk_x509_native_resource_v1.json",
        ):
            assert not (ROOT / "fixtures" / "privacy" / fixture).exists()


def _workflow() -> str:
    return WORKFLOW.read_text(encoding="utf-8")


def test_workflow_is_manual_native_arm64_only() -> None:
    workflow = _workflow()
    assert "workflow_dispatch:" in workflow
    for automatic_trigger in ("\n  push:", "\n  pull_request:", "\n  schedule:"):
        assert automatic_trigger not in workflow
    for label in (
        "self-hosted",
        "Linux",
        "ARM64",
        "numeric-v1-release-calibration",
        "numeric-v1-slowest-supported-tier",
        "aws-graviton3",
        "c7g-4xlarge",
    ):
        assert f"- {label}" in workflow
    assert re.search(r"(?m)^\s+container:", workflow) is None
    for forbidden_runtime in ("docker ", "podman ", "qemu-", "lima ", "colima "):
        assert forbidden_runtime not in workflow.lower()


def test_every_dispatch_identity_and_budget_is_required_and_validated() -> None:
    workflow = _workflow()
    for input_name in (
        "expected_commit",
        "validator_release_ref",
        "expected_validator_source_tree_sha256",
        "expected_cargo_lock_sha256",
        "expected_exact12_sha256",
        "elapsed_ceiling_ms",
        "peak_rss_ceiling_bytes",
        "address_space_ceiling_bytes",
    ):
        match = re.search(
            rf"(?m)^      {input_name}:\n(?P<body>(?:        [^\n]*\n)+)",
            workflow,
        )
        assert match is not None
        block = match.group("body")
        assert "required: true" in block
        assert "type: string" in block
    assert 'test "$(git rev-parse HEAD)" = "$TAIRA_INPUT_EXPECTED_COMMIT"' in workflow
    assert (
        'test "$observed_source_tree" = "$TAIRA_INPUT_SOURCE_TREE_SHA256"' in workflow
    )
    assert '"$TAIRA_INPUT_LOCK_SHA256" Cargo.lock | sha256sum -c -' in workflow
    assert '"$TAIRA_INPUT_EXACT12_SHA256"' in workflow
    assert "01f6ddf7588f42ae2d7eb0a2f21d44e8e96674cf" in workflow
    assert "commit-date: 2026-02-11" in workflow
    assert "host: aarch64-unknown-linux-gnu" in workflow
    assert "WORKFLOW_SHA" in workflow


def test_reconstructed_dpn_source_is_verified_before_any_build() -> None:
    workflow = _workflow()
    source_start = workflow.index(
        "Authenticate and reconstruct the reviewed DPN source"
    )
    source_end = workflow.index("Allocate a canonical evidence root")
    source_step = workflow[source_start:source_end]
    reconstruct = (
        'python3 -I -S "$release_root/iroha_source_bundle.py" reconstruct'
    )
    verify = 'python3 -I -S "$release_root/iroha_source_bundle.py" verify'
    head_check = 'test "$(git rev-parse HEAD)" = "$TAIRA_INPUT_EXPECTED_COMMIT"'
    assert source_step.count(reconstruct) == 1
    assert source_step.count(verify) == 1
    assert source_step.index(reconstruct) < source_step.index(verify)
    assert source_step.index(verify) < source_step.rindex(head_check)
    for argument in ('--repo "$GITHUB_WORKSPACE"', '--bundle-dir "$source_bundle"'):
        assert source_step.count(argument) == 2


def test_source_authorization_uses_only_protected_environment_policy() -> None:
    workflow = _workflow()
    assert "environment: taira-privacy-native-release" in workflow
    protected = (
        "TAIRA_PRIVACY_IROHA_SIGNER_PRINCIPAL",
        "TAIRA_PRIVACY_IROHA_SIGNER_PUBLIC_KEY",
        "TAIRA_PRIVACY_IROHA_SIGNER_FINGERPRINT",
        "TAIRA_PRIVACY_DPN_SIGNER_PRINCIPAL",
        "TAIRA_PRIVACY_DPN_SIGNER_PUBLIC_KEY",
        "TAIRA_PRIVACY_DPN_SIGNER_FINGERPRINT",
        "TAIRA_PRIVACY_RUST_SYSROOT_TREE_SHA256",
    )
    dispatch = workflow[
        workflow.index("workflow_dispatch:") : workflow.index("concurrency:")
    ]
    for variable in protected:
        assert f"${{{{ vars.{variable} }}}}" in workflow
        assert variable not in dispatch
    assert "TAIRA_APPROVED_IROHA_SIGNER_PUBLIC_KEY" in workflow
    assert "TAIRA_APPROVED_DPN_SIGNER_PUBLIC_KEY" in workflow
    assert "approved SSH public key blob is truncated" in workflow
    assert 'namespaces="git"' in workflow


def test_both_source_commits_are_ssh_authenticated_before_repo_code() -> None:
    workflow = _workflow()
    checkout = workflow.index("actions/checkout@")
    source = workflow.index("Authenticate and reconstruct the reviewed DPN source")
    reconstruct = workflow.index(
        'python3 -I -S "$release_root/iroha_source_bundle.py" reconstruct'
    )
    first_repo_gate = (
        workflow.index("scripts/check", checkout)
        if "scripts/check" in workflow[checkout:]
        else reconstruct
    )
    assert checkout < source < reconstruct
    assert source < first_repo_gate or first_repo_gate == reconstruct
    source_block = workflow[source:workflow.index("Allocate a canonical evidence root")]
    assert source_block.count("verify_signed_commit \\") == 2
    assert "verify-commit --raw" in source_block
    assert "gpg.format=ssh" in source_block
    assert "gpg.minTrustLevel=fully" in source_block
    assert "GIT_CONFIG_NOSYSTEM=1" in source_block
    assert "GIT_CONFIG_GLOBAL=/dev/null" in source_block
    assert "GIT_NO_REPLACE_OBJECTS=1" in source_block
    assert "--format=%G?%x00%GF%x00%GP%x00%GS%x00" in source_block
    assert "SSH signature metadata is not the approved identity" in source_block
    assert "-----BEGIN SSH SIGNATURE-----" in source_block


def test_dpn_release_bytes_come_only_from_the_authenticated_git_object() -> None:
    workflow = _workflow()
    source = workflow[
        workflow.index("Authenticate and reconstruct the reviewed DPN source") :
        workflow.index("Allocate a canonical evidence root")
    ]
    assert "raw.githubusercontent.com" not in source
    assert "curl " not in source
    assert "https://github.com/soramitsu/dpn-api-rust.git" in source
    fetch = source.index("fetch \\")
    verify = source.rindex("verify_signed_commit \\")
    extract = source.index('"ls-tree",')
    reconstruct = source.index("iroha_source_bundle.py\" reconstruct")
    assert fetch < verify < extract < reconstruct
    for required in (
        '"--full-tree"',
        "rb\"(100644|100755) blob ([0-9a-f]{40})\\t\"",
        '"cat-file", "-s"',
        '"cat-file", "blob"',
        "os.O_EXCL",
        "os.O_NOFOLLOW",
        "os.fsync(descriptor)",
        "remote remove origin",
    ):
        assert required in source


def test_rust_toolchain_is_content_authenticated_and_rechecked() -> None:
    workflow = _workflow()
    setup = workflow.index("actions-rust-lang/setup-rust-toolchain@")
    authenticate = workflow.index("Authenticate the installed Rust toolchain tree")
    first_cargo = workflow.index("cargo test", authenticate)
    final_build = workflow.index("Verify feature separation and build fresh final binaries")
    second_seal = workflow.index("rust-toolchain-tree-after-builds-v1.json")
    assert setup < authenticate < first_cargo < final_build < second_seal
    stable_hasher = (
        '"$TAIRA_DPN_RELEASE_ROOT/authentication/hash_taira_rust_toolchain.py"'
    )
    assert workflow.count(stable_hasher) == 2
    assert (
        '"$TAIRA_INPUT_EXPECTED_COMMIT:scripts/hash_taira_rust_toolchain.py"'
        in workflow
    )
    assert '"$observed_toolchain_sha" != \\' in workflow
    assert '"$TAIRA_APPROVED_RUST_SYSROOT_TREE_SHA256"' in workflow
    assert 'echo "RUSTC=$rust_sysroot/bin/rustc"' in workflow
    assert 'echo "CARGO=$rust_sysroot/bin/cargo"' in workflow
    assert 'test "$final_toolchain_sha" = "$TAIRA_RUST_TOOLCHAIN_TREE_SHA256"' in workflow


def test_workflow_uses_only_commit_pinned_actions_and_read_permission() -> None:
    workflow = _workflow()
    uses = re.findall(r"(?m)^\s+-?\s*uses:\s*([^@\s]+)@([^\s]+)", workflow)
    assert uses
    for action, revision in uses:
        assert re.fullmatch(r"[0-9a-f]{40}", revision), (action, revision)
    assert "permissions:\n  contents: read" in workflow
    assert "persist-credentials: false" in workflow


def test_zero_pin_capture_and_fresh_final_rebuild_are_ordered() -> None:
    workflow = _workflow()
    pre_capture_gates = workflow.index(
        "Run fail-fast installer, soundness, and deterministic KAT gates"
    )
    bootstrap = workflow.index("Build and attest the zero-pin static capture runner")
    capture = workflow.index(
        "Capture, validate, hash, and install the native fixture set"
    )
    installer = workflow.index("scripts/install_taira_privacy_native_expectations.py")
    final_identity = workflow.index("TAIRA_FINAL_SOURCE_SHA256")
    final_build = workflow.index(
        "Verify feature separation and build fresh final binaries"
    )
    generate = workflow.index(
        "Generate, verify, and independently reverify the final bundle"
    )
    upload = workflow.index("Upload the non-publishing native evidence archive")
    assert (
        pre_capture_gates
        < bootstrap
        < capture
        < installer
        < final_identity
        < final_build
        < generate
        < upload
    )
    assert "taira-privacy-bootstrap-target." in workflow
    assert "taira-privacy-final-target." in workflow
    assert "provenance/bootstrap-runner.elf" in workflow
    assert workflow.count("-C target-feature=+crt-static") >= 4
    assert workflow.count("INTERP") >= 4
    assert workflow.count("NEEDED") >= 4
    assert workflow.count("RWE") >= 2
    assert "cargo update" not in workflow
    assert "cargo generate-lockfile" not in workflow
    assert workflow.count("--locked") >= 6


def test_pre_capture_gates_are_ordered_auditable_and_resource_bounded() -> None:
    workflow = _workflow()
    gates = workflow.index(
        "Run fail-fast installer, soundness, and deterministic KAT gates"
    )
    bootstrap = workflow.index("Build and attest the zero-pin static capture runner")
    block = workflow[gates:bootstrap]

    installer = block.index(
        "pytests/scripts/install_taira_privacy_native_expectations_test.py"
    )
    soundness = block.index(
        "privacy_engines::zk_x509::profile::readiness_certificates::tests::"
        "installed_soundness_pin_matches_the_current_compiled_profile"
    )
    kat = block.index(
        "privacy_release_evidence::zk_x509::release_kat_tests::"
        "positive_release_stage_is_the_sole_kat_producer"
    )
    assert installer < soundness < kat

    for required in (
        "PYTHONDONTWRITEBYTECODE=1",
        "-p no:cacheprovider",
        "--ignored",
        "--exact",
        "--nocapture",
        "--test-threads=1",
        'CARGO_TARGET_DIR="$bootstrap_target"',
        'TAIRA_BOOTSTRAP_SOURCE_SHA256',
        '"$TAIRA_INPUT_LOCK_SHA256" Cargo.lock | sha256sum -c -',
    ):
        assert required in block
    for bounded_setting in (
        "CARGO_BUILD_JOBS=1",
        "RAYON_NUM_THREADS=4",
        "RUST_MIN_STACK=8388608",
    ):
        assert block.count(bounded_setting) >= 4


def test_capture_installs_and_pins_the_complete_x509_resource_certificate() -> None:
    workflow = _workflow()
    capture_block = workflow[
        workflow.index(
            "Capture, validate, hash, and install the native fixture set"
        ) : workflow.index("Verify feature separation and build fresh final binaries")
    ]
    for option in (
        "--x509-resource-host-metadata",
        "--x509-resource-norito-out",
        "--x509-resource-json-out",
        "--native-verifier",
        "--native-verifier-sha256",
        "--exact12-matrix",
        "--captured-x509-resource-norito",
        "--captured-x509-resource-json",
        "--authenticated-iroha-source-commit",
        "--authenticated-iroha-signer-principal",
        "--authenticated-iroha-signer-fingerprint",
        "--authenticated-iroha-allowed-signers-sha256",
        "--authenticated-validator-source-commit",
        "--authenticated-validator-signer-principal",
        "--authenticated-validator-signer-fingerprint",
        "--authenticated-validator-allowed-signers-sha256",
        "--authenticated-validator-source-tree-sha256",
        "--authenticated-bootstrap-source-tree-sha256",
        "--authenticated-cargo-lock-sha256",
        "--authenticated-rust-toolchain-tree-sha256",
    ):
        assert option in capture_block
    assert 'TAIRA_BOOTSTRAP_RUNNER_SHA256=$bootstrap_runner_sha' in workflow
    installer = capture_block.index(
        "python3 -I -S scripts/install_taira_privacy_native_expectations.py"
    )
    installed_compare = capture_block.index(
        'cmp "$captured_norito"'
    )
    validation_arguments = capture_block[installer:installed_compare]
    assert '--native-verifier "$TAIRA_BOOTSTRAP_RUNNER"' in validation_arguments
    assert (
        '--native-verifier-sha256 "$TAIRA_BOOTSTRAP_RUNNER_SHA256"'
        in validation_arguments
    )
    assert (
        '--exact12-matrix "$GITHUB_WORKSPACE/fixtures/privacy/exact12_v1.tsv"'
        in validation_arguments
    )
    for fixture in (
        "fixtures/privacy/native_release_expectations_v1.norito",
        "fixtures/privacy/native_release_expectations_v1.json",
        "fixtures/privacy/zk_x509_native_resource_v1.norito",
        "fixtures/privacy/zk_x509_native_resource_v1.json",
    ):
        assert fixture in workflow
    for pin in (
        "ZK_X509_RELEASE_KAT_EXPECTED_PROOF_BYTES_V1",
        "ZK_X509_RELEASE_KAT_EXPECTED_PROOF_SHA256_V1",
        "ZK_X509_NATIVE_RELEASE_EXPECTATIONS_NORITO_SHA256_V1",
        "ZK_X509_NATIVE_RELEASE_EXPECTATIONS_JSON_SHA256_V1",
        "ZK_X509_RESOURCE_POSITIVE_ELAPSED_MILLIS_V1",
        "ZK_X509_RESOURCE_POSITIVE_PEAK_RSS_BYTES_V1",
        "ZK_X509_RESOURCE_POSITIVE_PEAK_ADDRESS_SPACE_BYTES_V1",
        "ZK_X509_RESOURCE_MAXIMUM_ELAPSED_MILLIS_V1",
        "ZK_X509_RESOURCE_MAXIMUM_PEAK_RSS_BYTES_V1",
        "ZK_X509_RESOURCE_MAXIMUM_PEAK_ADDRESS_SPACE_BYTES_V1",
        "ZK_X509_RESOURCE_CERTIFICATE_SHA256_V1",
    ):
        assert pin in workflow
    assert "native-capture-pins.patch" in capture_block
    assert "zk-x509-readiness-certificates.before.rs" in workflow
    assert "zk-x509-readiness-certificates.after.rs" in capture_block


def test_final_evidence_is_reverified_from_copied_bundle_before_upload() -> None:
    workflow = _workflow()
    final_step = workflow.index(
        "Generate, verify, and independently reverify the final bundle"
    )
    package_step = workflow.index("Package complete provenance")
    block = workflow[final_step:package_step]
    assert '"$TAIRA_FINAL_RUNNER" generate' in block
    assert '"$TAIRA_FINAL_RUNNER" verify' in block
    assert '"$TAIRA_EVIDENCE_ROOT/bin/taira_privacy_release_runner" verify' in block
    assert (
        block.count("python3 -I -S scripts/compute_workspace_source_manifest.py") >= 2
    )
    assert block.count('"$TAIRA_INPUT_LOCK_SHA256" Cargo.lock | sha256sum -c -') >= 2
    assert block.count("--x509-resource-norito") == 2
    assert block.count("--x509-resource-json") == 2
    assert "zk-x509-resource-v1.norito" in block
    assert "zk-x509-resource-v1.json" in block
    assert '"published": False' in workflow
    assert '"deployed": False' in workflow


def test_packaged_archive_is_safelist_audited_before_hash_and_upload() -> None:
    workflow = _workflow()
    package_start = workflow.index("Package complete provenance")
    package_end = workflow.index("Upload the non-publishing native evidence archive")
    package = workflow[package_start:package_end]
    tar_creation = '-czf "$bundle"'
    audit = "python3 -I -S scripts/audit_taira_privacy_native_archive.py"
    archive_hash = 'bundle_sha="$(sha256sum "$bundle"'
    assert package.count(audit) == 1
    assert package.index(tar_creation) < package.index(audit) < package.index(archive_hash)
    assert '--archive "$bundle"' in package
    assert '--staged-root "$TAIRA_EVIDENCE_ROOT"' in package


def test_packaged_provenance_binds_source_and_toolchain_authentication() -> None:
    workflow = _workflow()
    package = workflow[
        workflow.index("Package complete provenance") :
        workflow.index("Upload the non-publishing native evidence archive")
    ]
    for evidence in (
        "source-authentication.json",
        "iroha-allowed-signers",
        "iroha-commit.raw",
        "iroha-signature-metadata.bin",
        "iroha-verify-commit.log",
        "dpn-allowed-signers",
        "dpn-commit.raw",
        "dpn-signature-metadata.bin",
        "dpn-verify-commit.log",
        "ssh-revocation",
        "hash_taira_rust_toolchain.py",
        "rust-toolchain-tree-v1.json",
        "rust-toolchain-tree-after-builds-v1.json",
    ):
        assert evidence in workflow
    assert '"authenticated_source_origins": authenticated_origins' in package
    assert '"rust_toolchain_tree_sha256": required_sha(' in package
    assert '"expectation_installation_manifest_sha256": digest(' in package
    assert "authenticated source origins diverged before packaging" in package
    assert "expectation installation diverged from authenticated origins" in package


def test_capture_workflow_cannot_publish_or_deploy() -> None:
    workflow = _workflow().lower()
    for forbidden in (
        "cargo publish",
        "docker push",
        "buildx --push",
        "gh release",
        "kubectl ",
        "helm ",
        "terraform ",
        "scp ",
        "rsync ",
    ):
        assert forbidden not in workflow
    assert re.search(r"(?m)^\s+ssh(?:\s|$)", workflow) is None
    assert "actions/upload-artifact@" in workflow
    assert '"published": false' in workflow
    assert '"deployed": false' in workflow


def test_host_guard_covers_native_kernel_and_resource_primitives() -> None:
    host_check = HOST_CHECK.read_text(encoding="utf-8")
    native_probe = HOST_PROBE.read_text(encoding="utf-8")
    complete_guard = host_check + "\n" + native_probe
    for required in (
        'uname -s)" != "Linux"',
        'uname -m)" != "aarch64"',
        "kernel >= 6.3",
        "c7g.4xlarge",
        "Neoverse-V1",
        "/proc/self/status",
        "/proc/self/task",
        "/proc/sys/vm/memfd_noexec",
        'os.sysconf("SC_NPROCESSORS_CONF")',
        'os.sysconf("SC_NPROCESSORS_ONLN")',
        "os.sched_getaffinity",
        "cpuset.cpus.effective",
        "cpu.max",
        "memory.max",
        "memory.current",
        "12 * 1024 * 1024 * 1024",
        "32 * 1024 * 1024 * 1024",
        "8 * 1024 * 1024",
        "RLIMIT_NOFILE",
        "RLIMIT_FSIZE",
        "RLIMIT_CPU",
        "RLIMIT_NPROC",
        "Landlock ABI",
        "landlock_restrict_self",
        "enter deny-all Landlock domain",
        "openat2",
        "MFD_EXEC",
        "F_SEAL_EXEC",
        "seccomp TSYNC",
        "WORKER_THREAD_COUNT = 5",
        "writable executable segment",
        "-static",
    ):
        assert required in complete_guard
    assert "containerized execution is forbidden" in host_check
    syntax = subprocess.run(
        ["bash", "-n", str(HOST_CHECK)],
        check=False,
        capture_output=True,
        text=True,
    )
    assert syntax.returncode == 0, syntax.stderr
    help_result = subprocess.run(
        ["bash", str(HOST_CHECK), "--help"],
        check=False,
        capture_output=True,
        text=True,
    )
    assert help_result.returncode == 0, help_result.stderr
    assert "--metadata-out" in help_result.stdout
    assert "--x509-environment-out" in help_result.stdout


def test_host_guard_rejects_invalid_output_contract_before_host_probing(
    tmp_path: Path,
) -> None:
    canonical = tmp_path.resolve()
    metadata = canonical / "metadata.json"
    environment = canonical / "environment.json"
    metadata.write_text("existing", encoding="utf-8")
    cases = (
        [],
        ["--metadata-out", "relative.json"],
        ["--metadata-out", str(canonical / "new.json")],
        [
            "--metadata-out",
            str(metadata),
            "--x509-environment-out",
            str(environment),
        ],
        [
            "--metadata-out",
            str(canonical / "same.json"),
            "--x509-environment-out",
            str(canonical / "same.json"),
        ],
        ["--unknown", "value"],
    )
    for arguments in cases:
        result = subprocess.run(
            ["bash", str(HOST_CHECK), *arguments],
            check=False,
            capture_output=True,
            text=True,
        )
        assert result.returncode != 0, arguments


def test_host_guard_emits_exact_typed_x509_environment_create_new() -> None:
    host_check = HOST_CHECK.read_text(encoding="utf-8")
    for fragment in (
        '"operating_system": "linux"',
        '"architecture": "aarch64"',
        '"endianness": "little"',
        '"kernel_minimum_major": 6',
        '"kernel_minimum_minor": 3',
        '"rustc_release": "1.93.1"',
        '"rustc_host": "aarch64-unknown-linux-gnu"',
        '"rustc_commit_hash": "01f6ddf7588f42ae2d7eb0a2f21d44e8e96674cf"',
        '"rustc_commit_date": "2026-02-11"',
        '"instance_type": instance_type',
        '"cpu_model": cpu_model',
        "os.O_EXCL",
        "os.O_NOFOLLOW",
    ):
        assert fragment in host_check
    assert (
        'echo "TAIRA_X509_RESOURCE_ENVIRONMENT=$x509_environment" >>"$GITHUB_ENV"'
        in _workflow()
    )
