"""Static contracts for deterministic Android binding generation in release CI."""

from __future__ import annotations

from pathlib import Path
import re


REPO_ROOT = Path(__file__).resolve().parents[2]

FORBIDDEN_PROCESS_CONTROL_PATTERNS = (
    re.compile(
        r"(?m)^[ \t]*(?:command[ \t]+)?"
        r"(?:kill|killall|pkill|renice|timeout)(?:[ \t]|$)"
    ),
    re.compile(r"\b(?:os\.kill|(?:os\.)?killpg)\s*\("),
    re.compile(r"\.\s*(?:terminate|kill)\s*\("),
    re.compile(r"\bstart_new_session\s*="),
    re.compile(r"\b(?:SIG)?(?:STOP|TERM|KILL)\b"),
    re.compile(
        r"\.\s*(?:wait|communicate)\s*\([^)]*\btimeout\s*=",
        re.DOTALL,
    ),
    re.compile(
        r"\bsubprocess\.(?:run|call|check_call|check_output)\s*"
        r"\([^)]*\btimeout\s*=",
        re.DOTALL,
    ),
)


def read(relative: str) -> str:
    """Read one repository contract as UTF-8."""

    return (REPO_ROOT / relative).read_text(encoding="utf-8")


def has_forbidden_process_control(source: str) -> bool:
    """Recognize executable process controls without matching benign prose."""

    return any(pattern.search(source) for pattern in FORBIDDEN_PROCESS_CONTROL_PATTERNS)


def test_android_codegen_gate_uses_two_sealed_isolated_replays() -> None:
    """Binding generation compares two hard-link-free immutable mirrors."""

    gate = read("ci/check_android_codegen.sh")
    assert "deterministic binding generation requires a clean checkout" in gate
    assert "HEAD_COMMIT=" in gate
    assert "GIT_OPTIONAL_LOCKS=0" in gate
    assert "compgen -e" in gate
    assert 'unset "${openapi_git_variable}"' in gate
    for setting in (
        "GIT_NO_LAZY_FETCH=1",
        "GIT_NO_REPLACE_OBJECTS=1",
        "GIT_CONFIG_NOSYSTEM=1",
        "GIT_CONFIG_GLOBAL=/dev/null",
        "GIT_CONFIG_COUNT=2",
        "GIT_CONFIG_KEY_0=core.hooksPath",
        "GIT_CONFIG_VALUE_0=/dev/null",
        "GIT_CONFIG_KEY_1=core.fsmonitor",
        "GIT_CONFIG_VALUE_1=false",
    ):
        assert setting in gate
    assert gate.index("compgen -e") < gate.index("git -C")
    assert gate.count('create_replay_clone "${') == 2
    assert gate.count('run_replay "${') == 2
    assert gate.count("git clone --quiet --local --no-hardlinks --no-checkout") == 1
    assert "worktree" not in gate
    assert "--no-writable-paths" in gate
    assert 'FIRST_TARGET="${RUN_ROOT}/target-first"' in gate
    assert 'SECOND_TARGET="${RUN_ROOT}/target-second"' in gate
    assert 'diff -ru "${FIRST_STAGE}/generated" "${SECOND_STAGE}/generated"' in gate
    assert 'cmp -s "${FIRST_STAGE}/codegen_parity_summary.json"' in gate
    assert "two clean Android binding generations produced different bytes" in gate
    assert "two clean Android parity summaries disagreed" in gate


def test_android_codegen_gate_obeys_shared_cargo_policy() -> None:
    """Every reachable Android generator Cargo call uses the shared policy."""

    gate = read("ci/check_android_codegen.sh")
    proxy = read("scripts/sumeragi_v2_release_cargo_proxy.sh")
    makefile = read("Makefile")
    replay = read("scripts/android_codegen_replay_sorafs_fixture.py")

    assert 'source "${PROCESS_POLICY}"' in gate
    assert "run_cargo run" in gate
    assert "--locked" in gate
    assert "--offline" in gate
    assert "make android-codegen-verify" not in gate
    assert "CARGO_TARGET_DIR:-target" not in gate
    assert 'require_external_cargo_target_dir "${REPO_ROOT}"' in proxy
    assert 'run_cargo "$@"' in proxy
    assert "cargo run --locked -p norito_codegen_exporter" not in makefile
    assert "sumeragi_v2_release_cargo_proxy.sh run --locked --offline" in makefile
    assert '"--locked",\n        "--offline",' in replay
    assert "CARGO_BIN" not in replay
    assert "require_policy_cargo_proxy" in replay

    for relative in (
        "ci/check_android_codegen.sh",
        "scripts/sumeragi_v2_release_cargo_proxy.sh",
        "scripts/android_codegen_replay_sorafs_fixture.py",
        "scripts/android_codegen_docs.py",
        "scripts/check_android_codegen_parity.py",
        "scripts/seal_workspace_source.py",
        "tools/openapi/scripts/provision-openapi-cargo-lock.mjs",
    ):
        assert not has_forbidden_process_control(read(relative)), relative


def test_android_codegen_gate_owns_authenticated_release_channels() -> None:
    """Standalone runs create both channels; inherited runs require both."""

    gate = read("ci/check_android_codegen.sh")
    assert (
        'if [[ -z "${IROHA_RELEASE_ARTIFACT_ROOT:-}" \\\n'
        '  && -z "${IROHA_RELEASE_CANCEL_REQUEST_PATH:-}" ]]; then'
    ) in gate
    assert (
        'elif [[ -z "${IROHA_RELEASE_ARTIFACT_ROOT:-}" \\\n'
        '  || -z "${IROHA_RELEASE_CANCEL_REQUEST_PATH:-}" ]]; then'
    ) in gate
    assert (
        "IROHA_RELEASE_ARTIFACT_ROOT and "
        "IROHA_RELEASE_CANCEL_REQUEST_PATH must be supplied together"
    ) in gate
    assert 'require_external_release_artifact_root "${ROOT_DIR}"' in gate
    assert '"release cancellation marker parent"' in gate
    assert gate.count("require_disjoint_release_roots") == 2
    assert gate.rindex('require_disjoint_release_roots "${ROOT_DIR}"') < gate.index(
        'release_gate_boundary "android-codegen:channels-ready"'
    )
    assert 'release_gate_boundary "android-codegen:channels-ready"' in gate
    assert "[android-codegen] authenticated artifact root" in gate
    assert "[android-codegen] cooperative cancellation marker" in gate
    assert (
        'EVIDENCE_DIR="$(mktemp -d '
        '"${IROHA_RELEASE_ARTIFACT_ROOT}/android-codegen.XXXXXX")"'
    ) in gate
    assert 'require_release_artifact_directory "${EVIDENCE_DIR}"' in gate
    assert 'LOG_PATH="${EVIDENCE_DIR}/android-codegen.log"' in gate

    before = gate.index(
        'release_gate_boundary "android-codegen:before-completion-publication"'
    )
    receipt = gate.index('"${EVIDENCE_DIR}/COMPLETED.json"')
    after = gate.index(
        'release_gate_boundary "android-codegen:after-completion-publication"'
    )
    assert before < receipt < after


def test_android_process_control_scan_catches_reachable_mutations() -> None:
    """Whitespace and API spelling changes cannot bypass the focused scan."""

    mutations = (
        'kill "$child_pid"',
        'pkill -TERM cargo',
        'renice 10 "$child_pid"',
        'timeout 60 command cargo',
        "os.kill(child_pid, 9)",
        "os.killpg(group_id, 9)",
        "killpg(group_id, 9)",
        "child.terminate()",
        "child.kill()",
        "subprocess.Popen(command, start_new_session=True)",
        "child.wait(timeout = 60)",
        "child.communicate(\n    timeout=60)",
        "subprocess.run(command,\n    timeout=60)",
        "signal.SIGSTOP",
        "signal.SIGTERM",
        "signal.SIGKILL",
    )
    for mutation in mutations:
        assert has_forbidden_process_control(f"safe_prefix\n{mutation}\n"), mutation

    assert not has_forbidden_process_control(
        "# Cancellation is cooperative and checked only between commands.\n"
    )


def test_android_codegen_gate_fails_on_missing_or_extra_outputs() -> None:
    """Every tracked output and the mandatory parity/hash artifacts fail closed."""

    gate = read("ci/check_android_codegen.sh")
    assert "status --porcelain=v1 --untracked-files=all" in gate
    assert "immutable replay source identity changed" in gate
    assert '"${generated_root}/codegen_hash_tree.json"' in gate
    assert '"${generated_root}/codegen_manifest_metadata.json"' in gate
    assert '"${summary_path}"' in gate
    assert 'if [[ ! -f "${required}" || -L "${required}" ]]' in gate
    assert 'if [[ -f "${HASH_TREE_SOURCE}" ]]' not in gate
    assert 'diff -ru "${ROOT_DIR}/${DOCS_REL}" "${FIRST_STAGE}/generated"' in gate
    assert 'cmp -s "${ROOT_DIR}/${SUMMARY_REL}"' in gate


def test_android_codegen_archive_has_canonical_reproducible_metadata() -> None:
    """The staged archive must not inherit timestamps, owners, modes, or links."""

    gate = read("ci/check_android_codegen.sh")
    for marker in (
        'gzip.GzipFile(filename="", mode="wb", fileobj=raw, mtime=0)',
        "format=tarfile.PAX_FORMAT",
        "info.uid = 0",
        "info.gid = 0",
        'info.uname = ""',
        'info.gname = ""',
        "info.mtime = 0",
        "Android generated-doc archive must not contain symlinks",
    ):
        assert marker in gate
    assert gate.count('build_deterministic_archive "${') == 2
    assert 'cmp -s "${FIRST_ARCHIVE}" "${SECOND_ARCHIVE}"' in gate
    assert "two clean Android documentation archives produced different bytes" in gate
    assert "tar -czf" not in gate
    assert "rm -rf" not in gate
    assert "worktree remove" not in gate
    assert 'EVIDENCE_DIR="$(mktemp -d "${IROHA_RELEASE_ARTIFACT_ROOT}/' in gate


def test_openapi_workflow_executes_binding_replay_after_openapi_replay() -> None:
    """The canonical release-input workflow must actually execute both gates."""

    workflow = read(".github/workflows/openapi.yml")
    openapi = workflow.index("run: bash ci/check_openapi_spec.sh")
    bindings = workflow.index("run: bash ci/check_android_codegen.sh")
    assert openapi < bindings
    assert "timeout-minutes:" not in workflow
    assert "cancel-in-progress: false" in workflow
    assert "cancel-in-progress: true" not in workflow


def test_android_codegen_owner_doc_describes_authenticated_completion() -> None:
    owner_doc = read("specs/sdk/android/manifest_codegen_parity.md")

    assert "all-or-none" in owner_doc
    assert "authenticated artifact/cancellation channel pair" in owner_doc
    assert "both" in owner_doc
    assert "final `COMPLETED.json` receipt" in owner_doc
    assert "never interrupt an in-flight child" in owner_doc
