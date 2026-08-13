"""Static guards for fail-closed OpenAPI release generation."""

import re
from pathlib import Path

try:
    import tomllib
except ModuleNotFoundError:  # pragma: no cover - Python 3.10 and older
    import tomli as tomllib


REPO_ROOT = Path(__file__).resolve().parents[2]
XTASK = REPO_ROOT / "xtask" / "src" / "main.rs"
TORII_OPENAPI = REPO_ROOT / "crates" / "iroha_torii" / "src" / "openapi.rs"
OPENAPI_AUTHORITIES = (
    REPO_ROOT / "artifacts" / "openapi" / "torii.json",
    REPO_ROOT / "artifacts" / "openapi" / "versions" / "current" / "torii.json",
    REPO_ROOT / "crates" / "iroha_torii" / "assets" / "openapi" / "torii.json",
)
OPENAPI_GATE = REPO_ROOT / "ci" / "check_openapi_spec.sh"
OPENAPI_GENERATOR_WRAPPER = REPO_ROOT / "ci" / "run_openapi_generator.sh"
RELEASE_PROCESS_POLICY = (
    REPO_ROOT / "scripts" / "sumeragi_v2_release_process_policy.sh"
)
OPENAPI_WORKFLOW = REPO_ROOT / ".github" / "workflows" / "openapi.yml"
OPENAPI_README = REPO_ROOT / "tools" / "openapi" / "README.md"
GENERATED_FILES = REPO_ROOT / "generated-files.toml"
OPENAPI_LOCK_PROVISIONER = (
    REPO_ROOT / "tools" / "openapi" / "scripts" / "provision-openapi-cargo-lock.mjs"
)
OPENAPI_LOCK_PIN = REPO_ROOT / "release" / "openapi-cargo-lock-v1.txt"
OPENAPI_SCRIPTS = (
    REPO_ROOT / "tools" / "openapi" / "scripts" / "sync-openapi.mjs",
    REPO_ROOT / "tools" / "openapi" / "scripts" / "verify-openapi-versions.mjs",
    REPO_ROOT
    / "tools"
    / "openapi"
    / "scripts"
    / "verify-openapi-release-inputs.mjs",
    REPO_ROOT / "tools" / "openapi" / "scripts" / "check-openapi-signatures.mjs",
)

FORBIDDEN_PROCESS_CONTROL_PATTERNS = (
    (
        "shell process-control command",
        re.compile(
            r"(?m)^[ \t]*(?:command[ \t]+)?"
            r"(?:kill|killall|pkill|renice|timeout)(?:[ \t]|$)"
        ),
    ),
    (
        "Python signal API",
        re.compile(r"\b(?:os\.kill|(?:os\.)?killpg)\s*\("),
    ),
    ("Popen terminate/kill API", re.compile(r"\.\s*(?:terminate|kill)\s*\(")),
    ("detached process session", re.compile(r"\bstart_new_session\s*=")),
    ("non-cooperative signal", re.compile(r"\b(?:SIG)?(?:STOP|TERM|KILL)\b")),
    (
        "timed child wait",
        re.compile(
            r"\.\s*(?:wait|communicate)\s*\([^)]*\btimeout\s*=",
            re.DOTALL,
        ),
    ),
    (
        "timed subprocess helper",
        re.compile(
            r"\bsubprocess\.(?:run|call|check_call|check_output)\s*"
            r"\([^)]*\btimeout\s*=",
            re.DOTALL,
        ),
    ),
)


def forbidden_process_control_matches(source: str) -> tuple[str, ...]:
    """Return precise process-control primitives present in production source."""

    return tuple(
        label
        for label, pattern in FORBIDDEN_PROCESS_CONTROL_PATTERNS
        if pattern.search(source)
    )


def test_openapi_generator_has_no_stub_fallback() -> None:
    xtask = XTASK.read_text(encoding="utf-8")
    torii_openapi = TORII_OPENAPI.read_text(encoding="utf-8")

    assert "allow_stub" not in xtask
    assert "build_stub_spec" not in xtask
    assert "pub fn stub_spec" not in torii_openapi
    # The sole occurrence is the negative parser test proving the deleted flag
    # remains rejected. Any production/help occurrence is a regression.
    assert xtask.count('"--allow-stub"') == 1
    assert 'let args = ["xtask", "openapi", "--allow-stub"];' in xtask
    assert "require_release_router_openapi(try_generate_router_openapi())?" in xtask


def test_openapi_static_authorities_are_exact_package_mirrors() -> None:
    authority_bytes = [path.read_bytes() for path in OPENAPI_AUTHORITIES]

    assert authority_bytes[0] == authority_bytes[1] == authority_bytes[2]
    torii_openapi = TORII_OPENAPI.read_text(encoding="utf-8")
    release_gate = OPENAPI_GATE.read_text(encoding="utf-8")
    assert 'include_str!("../assets/openapi/torii.json")' in torii_openapi
    assert "pub(crate) fn compiled_spec() -> &'static Value" in torii_openapi
    assert "pub(crate) fn compiled_spec_json() -> &'static str" in torii_openapi
    assert (
        'PACKAGE_SPEC_PATH="${REPLAY_SOURCE_FIRST}/crates/iroha_torii/assets/openapi/torii.json"'
        in release_gate
    )
    assert 'for authority in "${CURRENT_SPEC_PATH}" "${PACKAGE_SPEC_PATH}"' in release_gate
    assert 'cmp -s "${SPEC_PATH}" "${authority}"' in release_gate


def test_every_openapi_manifest_boundary_validates_release_shape() -> None:
    xtask = XTASK.read_text(encoding="utf-8")

    for function_name in (
        "write_openapi_manifest",
        "write_openapi_manifest_with_signature",
        "write_openapi_manifest_unsigned",
        "write_openapi_manifest_from_bytes",
        "verify_openapi_manifest",
    ):
        start = xtask.index(f"fn {function_name}(")
        body_start = xtask.index("{", start)
        next_function = xtask.find("\nfn ", body_start)
        body = xtask[body_start : None if next_function < 0 else next_function]
        assert "validate_release_openapi_bytes" in body, function_name


def test_openapi_version_and_signature_paths_reject_empty_specs() -> None:
    for path in OPENAPI_SCRIPTS:
        source = path.read_text(encoding="utf-8")
        assert "validateReleaseOpenApiDocumentBytes" in source, path


def test_openapi_generated_owner_has_exact_outputs_and_staging_interfaces() -> None:
    registry = tomllib.loads(GENERATED_FILES.read_text(encoding="utf-8"))
    owners = [
        entry
        for entry in registry["generated"]
        if entry["name"] == "torii-openapi-release-bundle"
    ]
    assert len(owners) == 1
    owner = owners[0]
    assert owner["outputs"] == [
        "artifacts/openapi/torii.json",
        "artifacts/openapi/manifest.json",
        "artifacts/openapi/versions/current/torii.json",
        "artifacts/openapi/versions/current/manifest.json",
        "artifacts/openapi/versions.json",
    ]
    assert "artifacts/openapi/allowed_signers.json" in owner["inputs"]
    assert "artifacts/openapi/allowed_signers.json" not in owner["outputs"]
    assert "bash ci/run_openapi_generator.sh" in owner["generator"]
    assert (
        'IROHA_RELEASE_ARTIFACT_ROOT="${IROHA_OPENAPI_STAGE%/*}"'
        in owner["generator"]
    )
    assert (
        'IROHA_RELEASE_CANCEL_REQUEST_PATH="${IROHA_OPENAPI_STAGE%/*/*}/cancel-request.json"'
        in owner["generator"]
    )
    assert '--output-dir "${IROHA_OPENAPI_STAGE}"' in owner["generator"]
    assert '--output-dir="${IROHA_OPENAPI_STAGE}"' in owner["generator"]
    assert "absolute private /private/tmp <run>/artifacts/<stage> directory" in owner["generator"]
    assert "--reuse-canonical-spec" not in owner["generator"]
    assert "cargo run" not in owner["generator"]
    assert "--lockfile-path" not in owner["generator"]
    assert "unstable-options" not in owner["generator"]
    assert {
        "ci/check_openapi_spec.sh",
        "ci/run_openapi_generator.sh",
        "scripts/seal_workspace_source.py",
        "scripts/sumeragi_v2_release_process_policy.sh",
    } <= set(owner["generator_sources"])
    assert owner["check"] == "bash ci/check_openapi_spec.sh"

    sync_source = OPENAPI_SCRIPTS[0].read_text(encoding="utf-8")
    assert "node:child_process" not in sync_source
    assert "runCargo" not in sync_source
    assert "readOpenApiStableFile(canonicalSpec" in sync_source


def test_openapi_cargo_lock_pin_has_one_staging_only_owner() -> None:
    registry = tomllib.loads(GENERATED_FILES.read_text(encoding="utf-8"))
    owners = [
        entry
        for entry in registry["generated"]
        if entry["name"] == "openapi-cargo-lock-pin"
    ]
    assert len(owners) == 1
    owner = owners[0]
    assert owner["outputs"] == ["release/openapi-cargo-lock-v1.txt"]
    assert owner["inputs"] == ["release/openapi-generator-inputs-v1.txt"]
    assert "provision-openapi-cargo-lock.mjs pin" in owner["generator"]
    assert "--source=\"$PWD/Cargo.lock\"" in owner["generator"]
    assert "IROHA_OPENAPI_CARGO_LOCK_PIN_STAGE" in owner["generator"]
    assert "--check=\"$PWD/release/openapi-cargo-lock-v1.txt\"" in owner["check"]

    xtask = XTASK.read_text(encoding="utf-8")
    provisioner = OPENAPI_LOCK_PROVISIONER.read_text(encoding="utf-8")
    pin_fields = dict(
        line.split("=", 1)
        for line in OPENAPI_LOCK_PIN.read_text(encoding="utf-8").splitlines()[1:]
    )
    pinned_size = pin_fields["bytes"]
    pinned_digest = pin_fields["sha256_hex"]
    for source in (xtask, provisioner):
        assert pinned_size not in source.replace("_", "")
        assert pinned_digest not in source
    assert "outside the repository" in provisioner
    assert "assertOpenApiCargoLockSnapshotStable" in provisioner
    assert "absent OpenAPI Cargo.lock requires an explicit existing --source" in provisioner
    for forbidden in (
        "generate-lockfile",
        "--lockfile-path",
        "unstable-options",
        "RUSTC_BOOTSTRAP",
        "cargoExecutable",
        "spawnChecked",
    ):
        assert forbidden not in provisioner
    assert provisioner.count("spawn(") == 1
    assert "const child = spawn('git', arguments_" in provisioner


def test_release_gate_is_clean_pinned_and_replays_complete_bundles_independently() -> None:
    gate = OPENAPI_GATE.read_text(encoding="utf-8")

    assert gate.count(
        "node tools/openapi/scripts/verify-musubi-v1-contract.mjs"
    ) == 1
    assert gate.index("require_clean_checkout\n") < gate.index(
        "node tools/openapi/scripts/verify-musubi-v1-contract.mjs"
    )
    assert "require_clean_checkout" in gate
    assert "EXPECTED_GENERATOR_COMMIT" not in gate
    assert gate.count(
        "node tools/openapi/scripts/verify-openapi-release-inputs.mjs"
    ) == 2
    assert gate.count(
        "python3 scripts/check_sorafs_release_version_map.py"
    ) == 2
    assert gate.count('"${REPLAY_SOURCE_FIRST}"') >= 3
    assert gate.count('"${REPLAY_SOURCE_SECOND}"') >= 3
    assert '"${REPLAY_CARGO_TARGET_DIR_FIRST}"' in gate
    assert '"${REPLAY_CARGO_TARGET_DIR_SECOND}"' in gate
    assert '"${REPLAY_GENERATED_FIRST}"' in gate
    assert '"${REPLAY_GENERATED_SECOND}"' in gate
    assert 'create_replay_source "${REPLAY_SOURCE_FIRST}"' in gate
    assert 'create_replay_source "${REPLAY_SOURCE_SECOND}"' in gate
    assert (
        'REPLAY_COMMIT="$(git -C "${REPO_ROOT}" rev-parse --verify '
        '"HEAD^{commit}")"'
    ) in gate
    assert (
        'REPLAY_TREE="$(git -C "${REPO_ROOT}" rev-parse --verify '
        '"${REPLAY_COMMIT}^{tree}")"'
    ) in gate
    assert "git clone --quiet --local --no-hardlinks --no-checkout" in gate
    assert 'checkout --quiet --detach "${REPLAY_COMMIT}"' in gate
    assert "worktree add" not in gate
    assert "worktree remove" not in gate
    assert "rm -rf" not in gate
    assert "const sourcePath = await realpath(lockSourceArgument);" in gate
    assert "provisionOpenApiCargoLock," in gate
    assert "const summary = await provisionOpenApiCargoLock({" in gate
    assert "repoRoot: replaySourceRoot," in gate
    assert "summary.status !== 'installed'" in gate
    assert "summary.source !== 'operator'" in gate
    assert 'summary.path !== \'Cargo.lock\'' in gate
    assert '"${REPO_ROOT}/Cargo.lock"' in gate
    assert 'cp "${REPO_ROOT}/Cargo.lock"' not in gate
    assert (
        'OPENAPI_RUN_ROOT="$(mktemp -d '
        '/private/tmp/iroha-openapi-check.XXXXXX)"'
    ) in gate
    assert (
        'REPLAY_CARGO_TARGET_DIR_FIRST="${OPENAPI_RUN_ROOT}/target-first"'
        in gate
    )
    assert (
        'REPLAY_CARGO_TARGET_DIR_SECOND="${OPENAPI_RUN_ROOT}/target-second"'
        in gate
    )
    assert 'CARGO_TARGET_DIR="${target_root}"' in gate
    assert "require_external_private_directory" in gate
    assert "require_external_release_artifact_root" in gate
    assert "require_release_artifact_directory" in gate
    assert 'allowedSignersFile: join(outputDir, \'allowed_signers.json\')' not in gate
    assert 'allowedSignersFile,' in gate
    assert '"${ALLOWED_SIGNERS_PATH}"' in gate
    assert 'cp -R "${REPLAY_BASELINE}/." "${output_dir}/"' in gate
    assert '--output-root "${generated_dir}"' in gate
    assert "--unsigned-manifest" in gate
    assert '--seal --root "${source_root}" --no-writable-paths' in gate
    assert '--verify --root "${source_root}" --no-writable-paths' in gate
    assert 'actual_tree="$(GIT_OPTIONAL_LOCKS=0 git' in gate
    assert '"${actual_tree}" != "${REPLAY_TREE}"' in gate
    assert "const {syncOpenApi} = await import(syncModule)" in gate
    assert "requireSigned: false" in gate
    assert "is not clean and unsigned" in gate
    artifact_block = gate.split("GENERATED_RELEASE_ARTIFACTS=(\n", 1)[1].split(
        "\n)", 1
    )[0]
    assert [
        line.strip().removeprefix('"').removesuffix('"')
        for line in artifact_block.splitlines()
        if line.strip()
    ] == [
        "torii.json",
        "manifest.json",
        "versions/current/torii.json",
        "versions/current/manifest.json",
        "versions.json",
    ]
    assert 'diff -u "${first}" "${second}"' in gate
    assert (
        'diff -ru "${REPLAY_BUNDLE_FIRST}" "${REPLAY_BUNDLE_SECOND}"'
        in gate
    )
    assert 'diff -u "${MANIFEST_PATH}" "${CURRENT_MANIFEST_PATH}"' in gate
    assert (
        'diff -u "${RELEASE_INPUT_SUMMARY_FIRST}" '
        '"${RELEASE_INPUT_SUMMARY_SECOND}"'
        in gate
    )
    assert (
        'diff -u "${VERSION_MAP_SUMMARY_FIRST}" '
        '"${VERSION_MAP_SUMMARY_SECOND}"'
        in gate
    )
    assert "source-identity.json" in gate
    assert '"candidate_commit": commit' in gate
    assert '"candidate_tree": tree' in gate
    assert "VERSION_VERIFY_POLICY_ARGS" not in gate


def test_openapi_cargo_and_owner_surfaces_obey_release_process_policy() -> None:
    gate = OPENAPI_GATE.read_text(encoding="utf-8")
    wrapper = OPENAPI_GENERATOR_WRAPPER.read_text(encoding="utf-8")
    policy = RELEASE_PROCESS_POLICY.read_text(encoding="utf-8")
    workflow = OPENAPI_WORKFLOW.read_text(encoding="utf-8")
    readme = OPENAPI_README.read_text(encoding="utf-8")

    for source in (gate, wrapper):
        assert 'source "${PROCESS_POLICY}"' in source
        assert "run_cargo run" in source
        assert "--locked" in source
        assert "--offline" in source
        assert re.search(r"(?<!_)\bcargo\s+run\b", source) is None
        assert "--jobs" not in source
        assert " -j" not in source
        assert "--lockfile-path" not in source
        assert "unstable-options" not in source

    process_surfaces = (
        OPENAPI_GATE,
        OPENAPI_GENERATOR_WRAPPER,
        REPO_ROOT / "scripts" / "seal_workspace_source.py",
        REPO_ROOT / "scripts" / "check_sorafs_release_version_map.py",
        OPENAPI_LOCK_PROVISIONER,
        REPO_ROOT / "tools" / "openapi" / "scripts" / "sync-openapi.mjs",
        REPO_ROOT
        / "tools"
        / "openapi"
        / "scripts"
        / "verify-musubi-v1-contract.mjs",
        *OPENAPI_SCRIPTS[1:],
    )
    for path in process_surfaces:
        source = path.read_text(encoding="utf-8")
        assert forbidden_process_control_matches(source) == (), path

    channel_pair = re.compile(
        r'if \[\[ -z "\$\{IROHA_RELEASE_ARTIFACT_ROOT:-\}" \\\n'
        r'  && -z "\$\{IROHA_RELEASE_CANCEL_REQUEST_PATH:-\}" \]\]; then'
        r'[\s\S]*?elif \[\[ -z "\$\{IROHA_RELEASE_ARTIFACT_ROOT:-\}" \\\n'
        r'  \|\| -z "\$\{IROHA_RELEASE_CANCEL_REQUEST_PATH:-\}" \]\]; then'
    )
    for source in (gate, wrapper):
        assert channel_pair.search(source)
        assert (
            "IROHA_RELEASE_ARTIFACT_ROOT and "
            "IROHA_RELEASE_CANCEL_REQUEST_PATH must be supplied together"
        ) in source
        assert '"release cancellation marker parent"' in source
        assert "require_external_release_artifact_root" in source
        assert "require_disjoint_release_roots" in source
        assert (
            "export IROHA_RELEASE_ARTIFACT_ROOT "
            "IROHA_RELEASE_CANCEL_REQUEST_PATH"
        ) in source

    assert gate.count("require_disjoint_release_roots") == 2
    assert wrapper.count("require_disjoint_release_roots") == 1
    assert (
        'IROHA_RELEASE_CANCEL_REQUEST_PATH="${IROHA_RELEASE_ARTIFACT_ROOT%/*}/cancel-request.json"'
        in wrapper
    )

    assert 'release_gate_boundary "openapi:channels-ready"' in gate
    assert "OpenAPI authenticated artifact root" in gate
    assert "OpenAPI cooperative cancellation marker" in gate
    assert (
        'OPENAPI_EVIDENCE_DIR="$(mktemp -d '
        '"${IROHA_RELEASE_ARTIFACT_ROOT}/openapi-check.XXXXXX")"'
    ) in gate
    gate_before = gate.index('release_gate_boundary "openapi:before-completion-publication"')
    gate_receipt = gate.index('"${OPENAPI_EVIDENCE_DIR}/source-identity.json"')
    gate_after = gate.index('release_gate_boundary "openapi:after-completion-publication"')
    assert gate_before < gate_receipt < gate_after

    assert 'release_gate_boundary "openapi-generator:channels-ready"' in wrapper
    assert "OpenAPI generator authenticated artifact root" in wrapper
    assert "OpenAPI generator cooperative cancellation marker" in wrapper
    assert 'require_release_artifact_directory "${OUTPUT_DIR}"' in wrapper
    assert 'require_release_artifact_directory "${SIGNING_PAYLOAD%/*}"' in wrapper
    assert (
        'mktemp -d "${IROHA_RELEASE_ARTIFACT_ROOT}/'
        'openapi-generator-evidence.XXXXXX"'
    ) in wrapper
    wrapper_before = wrapper.index(
        'release_gate_boundary "openapi-generator:before-completion-publication"'
    )
    wrapper_receipt = wrapper.index(
        '"${OPENAPI_GENERATOR_EVIDENCE_DIR}/source-identity.json"'
    )
    wrapper_after = wrapper.index(
        'release_gate_boundary "openapi-generator:after-completion-publication"'
    )
    assert wrapper_before < wrapper_receipt < wrapper_after

    assert policy.count("acquire_invocation_cargo_lock() {") == 1
    assert policy.count("release_invocation_cargo_lock() {") == 1
    assert 'lock_path="${artifact_root}/.sumeragi-v2-cargo.lock"' in policy
    assert "lock.mkdir(mode=0o700)" in policy
    assert "wait_for_external_cargo" not in policy
    assert "ps -" not in policy
    assert "pgrep" not in policy
    assert "/proc/" not in policy
    assert "process_snapshot" not in policy
    scoped_cargo_start = policy.index("_run_cargo_with_scoped_lock() {")
    scoped_cargo_end = policy.index(
        "\nrequire_external_private_directory() {", scoped_cargo_start
    )
    scoped_cargo = policy[scoped_cargo_start:scoped_cargo_end]
    assert scoped_cargo.count("_require_cargo_configuration_unchanged") == 3
    assert 'if "$IROHA_RELEASE_CARGO_BIN" "$@"; then' in scoped_cargo
    assert "acquire_invocation_cargo_lock || return $?" in scoped_cargo
    assert "release_invocation_cargo_lock || return $?" in scoped_cargo
    assert (
        '( _run_cargo_with_scoped_lock "$label" "${pinned_arguments[@]}" )'
        in policy
    )
    assert 'pinned_arguments=("$subcommand" -j1)' in policy
    assert 'pinned_arguments+=("$@")' in policy
    assert "local status" not in policy
    assert "locked_count != 1 || offline_count != 1" in policy

    assert "cancel-in-progress: false" in workflow
    assert "cancel-in-progress: true" not in workflow
    assert "timeout-minutes:" not in workflow
    assert "CARGO_TARGET_DIR:" not in workflow
    assert "Swatinem/rust-cache" not in workflow
    assert workflow.count("provision-openapi-cargo-lock.mjs pin") == 1
    assert "provision-openapi-cargo-lock.mjs provision" not in workflow
    assert workflow.count('--source="${repo_root}/Cargo.lock"') == 1
    assert workflow.count(
        '--check="${repo_root}/release/openapi-cargo-lock-v1.txt"'
    ) == 1

    metadata_start = workflow.index("  metadata:")
    canonical_start = workflow.index("  canonical-spec:")
    assert metadata_start < canonical_start
    metadata_job = workflow[metadata_start:canonical_start]
    canonical_job = workflow[canonical_start:]
    for root_lock_dependency in (
        "Cargo.lock",
        "release/openapi-cargo-lock-v1.txt",
        "provision-openapi-cargo-lock.mjs",
    ):
        assert root_lock_dependency not in metadata_job
    for rust_generation_surface in (
        r"\bcargo(?:\s|\+)",
        r"\brust(?:c|fmt|up)\b",
        r"\bxtask\b",
        r"generate-lockfile",
        r"check_openapi_spec\.sh",
        r"run_openapi_generator\.sh",
    ):
        assert re.search(rust_generation_surface, metadata_job, re.IGNORECASE) is None
    assert "verify-openapi-release-inputs.mjs" in metadata_job
    assert "verify-openapi-versions.mjs --allow-unsigned" in metadata_job
    assert "check-openapi-signatures.mjs" in metadata_job
    assert "provision-openapi-cargo-lock.mjs pin" in canonical_job
    assert '--source="${repo_root}/Cargo.lock"' in canonical_job
    assert (
        '--check="${repo_root}/release/openapi-cargo-lock-v1.txt"'
        in canonical_job
    )
    assert canonical_job.index(
        "provision-openapi-cargo-lock.mjs pin"
    ) < canonical_job.index("bash ci/check_openapi_spec.sh")

    assert "bash ci/run_openapi_generator.sh" in readme
    assert "mktemp -d /private/tmp/iroha-openapi-refresh.XXXXXX" in readme
    assert 'OPENAPI_ARTIFACT_ROOT="${OPENAPI_RUN_ROOT}/artifacts"' in readme
    assert 'OPENAPI_STAGE="${OPENAPI_ARTIFACT_ROOT}/openapi"' in readme
    assert 'export IROHA_RELEASE_ARTIFACT_ROOT="${OPENAPI_ARTIFACT_ROOT}"' in readme
    assert (
        'export IROHA_RELEASE_CANCEL_REQUEST_PATH="${OPENAPI_RUN_ROOT}/cancel-request.json"'
        in readme
    )
    assert "must provide both" in readme
    assert "cargo run" not in readme
    assert "--lockfile-path" not in readme
    assert "unstable-options" not in readme


def test_openapi_process_control_scan_catches_reachable_mutations() -> None:
    mutations = (
        'kill "$child_pid"',
        'pkill -STOP cargo',
        'renice 10 "$child_pid"',
        'timeout 30 command cargo',
        "os.kill(child_pid, 9)",
        "os.killpg(group_id, 9)",
        "killpg(group_id, 9)",
        "child.terminate()",
        "child.kill()",
        "subprocess.Popen(command, start_new_session=True)",
        "child.wait(timeout = 30)",
        "child.communicate(\n    timeout=30)",
        "subprocess.run(command,\n    timeout=30)",
        "signal.SIGSTOP",
        "signal.SIGTERM",
        "signal.SIGKILL",
    )
    for mutation in mutations:
        assert forbidden_process_control_matches(f"safe_prefix\n{mutation}\n"), mutation

    benign_prose = (
        "# Cooperative cancellation is observed only at a gate boundary.\n"
        'printf "completed without interrupting an in-flight child"\n'
    )
    assert forbidden_process_control_matches(benign_prose) == ()
