"""Security contract for the protected Kagemusha V4 promotion workflow."""

import hashlib
from pathlib import Path
import subprocess
import tempfile

import pytest


ROOT = Path(__file__).resolve().parents[2]
PROMOTION_WORKFLOW = ROOT / ".github/workflows/promote_kagemusha_v4.yml"
TAIRA_WORKFLOW = ROOT / ".github/workflows/publish_taira_validator.yml"
READINESS_GATE = ROOT / "ci/check_kagemusha_production_readiness.sh"
READINESS_SOURCE_CONTRACT = (
    ROOT / "ci/check_kagemusha_production_readiness_source_contract.py"
)


def test_production_job_is_distinct_from_the_untrusted_controller_build() -> None:
    """Production inputs must never enter the job that builds handoff bytes."""

    source = PROMOTION_WORKFLOW.read_text(encoding="utf-8")
    build, promotion = source.split("  production-promotion:\n", 1)
    assert "environment:" not in build
    assert "kagemusha-untrusted-build" in build
    assert "environment: kagemusha-v4-production" in promotion
    assert "needs: controller-build" in promotion
    assert "kagemusha-production" in promotion
    assert "cancel-in-progress: false" in source


def test_production_job_installs_and_qualifies_the_exact_controller_image() -> None:
    """The protected job must byte-pin, root-install, and attack its controller."""

    source = PROMOTION_WORKFLOW.read_text(encoding="utf-8")
    promotion = source.split("  production-promotion:\n", 1)[1]
    install = promotion.index(
        "/usr/bin/install -o root -g wheel -m 0555 \"$controller_source\""
    )
    publish = promotion.index(
        "/bin/mv -f -- \"$controller_install_candidate\" "
        '"$KAGEMUSHA_AUTHENTICATED_TOOL_CONTROLLER_BIN"'
    )
    post_digest = promotion.index(
        'shasum -a 256 "$KAGEMUSHA_AUTHENTICATED_TOOL_CONTROLLER_BIN"',
        publish,
    )
    qualify = promotion.index(
        '"$KAGEMUSHA_AUTHENTICATED_TOOL_CONTROLLER_BIN" qualify-host-v1',
        post_digest,
    )
    gate_snapshot = promotion.index(
        'gate_snapshot="$gate_launch_dir/check_kagemusha_production_readiness.sh"',
        qualify,
    )
    gate = promotion.index("exec /bin/bash /dev/fd/10 promotion", gate_snapshot)
    assert install < publish < post_digest < qualify < gate
    assert gate_snapshot < gate
    assert 'exec 8<"$gate_snapshot"' in promotion[gate_snapshot:gate]
    assert 'exec 9<"$gate_snapshot"' in promotion[gate_snapshot:gate]
    assert 'exec 10<"$gate_snapshot"' in promotion[gate_snapshot:gate]
    assert "KAGEMUSHA_PRODUCTION_READINESS_GATE_EXECUTION_FD=10" in promotion
    assert "/usr/bin/shasum -a 256 <&9" in promotion[gate_snapshot:gate]
    assert 'test "$observed_sha256" = "$expected_sha256"' in promotion[gate_snapshot:gate]
    assert 'controller_uid" != 0' in promotion
    assert 'controller_gid" != 0' in promotion
    assert 'controller_mode" != 555' in promotion
    assert "controller handoff inventory is not exact" in promotion
    assert "require_root_custodied_directory_chain" in promotion
    assert "/bin/ls -lde" in promotion
    assert "/usr/bin/xattr" in promotion
    assert "/usr/bin/python3" not in promotion


@pytest.mark.skipif(
    not Path("/usr/bin/shasum").is_file(),
    reason="protected macOS launcher requires /usr/bin/shasum",
)
def test_gate_hash_and_execution_use_independent_authenticated_descriptors() -> None:
    """A pathname replacement cannot substitute bytes after all FDs are open."""

    with tempfile.TemporaryDirectory() as temporary:
        root = Path(temporary)
        gate = root / "gate.sh"
        hostile = root / "hostile.sh"
        gate.write_text(
            "#!/bin/bash\n"
            "set -euo pipefail\n"
            'test "${BASH_SOURCE[0]}" = /dev/fd/10\n'
            'observed="$(/usr/bin/shasum -a 256 /dev/fd/8 | '
            "/usr/bin/awk '{print $1}')\"\n"
            'test "$observed" = "$1"\n'
            "echo trusted-gate\n",
            encoding="utf-8",
        )
        hostile.write_text("#!/bin/bash\necho hostile-gate\n", encoding="utf-8")
        digest = hashlib.sha256(gate.read_bytes()).hexdigest()
        launcher = r'''
set -euo pipefail
gate="$1"
hostile="$2"
expected="$3"
exec 8<"$gate"
exec 9<"$gate"
exec 10<"$gate"
observed="$(/usr/bin/shasum -a 256 <&9 | /usr/bin/awk '{print $1}')"
exec 9<&-
test "$observed" = "$expected"
/bin/mv "$gate" "$gate.authenticated"
/bin/mv "$hostile" "$gate"
exec /bin/bash /dev/fd/10 "$expected"
'''
        completed = subprocess.run(
            ["/bin/bash", "-c", launcher, "descriptor-launch", str(gate), str(hostile), digest],
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
            close_fds=True,
            text=True,
        )
        assert completed.returncode == 0, completed.stderr
        assert completed.stdout == "trusted-gate\n"


def test_promotion_forwards_every_required_external_pin_under_env_clear() -> None:
    """Missing operator trust records must make the protected gate fail closed."""

    source = PROMOTION_WORKFLOW.read_text(encoding="utf-8")
    promotion = source.split("  production-promotion:\n", 1)[1]
    required = (
        "KAGEMUSHA_PRODUCTION_READINESS_GATE_SHA256",
        "KAGEMUSHA_PRODUCTION_READINESS_PYTHON",
        "KAGEMUSHA_PRODUCTION_READINESS_PYTHON_SHA256",
        "KAGEMUSHA_PRODUCTION_READINESS_PYTHON_RUNTIME_ROOT",
        "KAGEMUSHA_PRODUCTION_READINESS_PYTHON_RUNTIME_TREE_SHA256",
        "KAGEMUSHA_V4_RELEASE_POLICY_PATH",
        "KAGEMUSHA_V4_ARTIFACT_ROOT",
        "KAGEMUSHA_V4_KAGAMI_BIN",
        "KAGEMUSHA_V4_KAGAMI_SHA256",
        "KAGEMUSHA_AUTHENTICATED_TOOL_CONTROLLER_BIN",
        "KAGEMUSHA_AUTHENTICATED_TOOL_CONTROLLER_SHA256",
        "KAGEMUSHA_BUILD_REVIEWED_SOURCE_CLOSURE",
        "KAGEMUSHA_BUILD_REVIEWED_SOURCE_CLOSURE_SHA256",
        "KAGEMUSHA_PRODUCTION_SOURCE_SSH_ALLOWED_SIGNERS_PATH",
        "KAGEMUSHA_PRODUCTION_SOURCE_SSH_ALLOWED_SIGNERS_SHA256",
        "KAGEMUSHA_PRODUCTION_SOURCE_SSH_REVOCATION_PATH",
        "KAGEMUSHA_PRODUCTION_SOURCE_SSH_REVOCATION_SHA256",
        "KAGEMUSHA_BUILD_AUTHENTICATED_SOURCE_SEAL_PROJECTION",
        "KAGEMUSHA_BUILD_AUTHENTICATED_SOURCE_SEAL_PROJECTION_SHA256",
        "KAGEMUSHA_BUILD_SOURCE_SEAL_AUTHORIZATION",
        "KAGEMUSHA_BUILD_SOURCE_SEAL_AUTHORIZATION_SHA256",
        "KAGEMUSHA_BUILD_SOURCE_SEAL_CONTROLLER_SIGNATURE",
        "KAGEMUSHA_BUILD_SOURCE_SEAL_CONTROLLER_SIGNATURE_SHA256",
        "KAGEMUSHA_BUILD_SOURCE_SEAL_CONTROLLER_ALLOWED_SIGNERS",
        "KAGEMUSHA_BUILD_SOURCE_SEAL_CONTROLLER_ALLOWED_SIGNERS_SHA256",
        "KAGEMUSHA_BUILD_SOURCE_SEAL_CONTROLLER_REVOCATION",
        "KAGEMUSHA_BUILD_SOURCE_SEAL_CONTROLLER_REVOCATION_SHA256",
        "KAGEMUSHA_BUILD_SOURCE_SEAL_EXECUTION_POLICY",
        "KAGEMUSHA_BUILD_SOURCE_SEAL_EXECUTION_POLICY_SHA256",
        "KAGEMUSHA_BUILD_SOURCE_SEAL_RAW_UNIT_GRAPH",
        "KAGEMUSHA_BUILD_SOURCE_SEAL_RAW_UNIT_GRAPH_SHA256",
        "KAGEMUSHA_BUILD_SOURCE_SEAL_NORMALIZED_UNIT_GRAPH",
        "KAGEMUSHA_BUILD_SOURCE_SEAL_NORMALIZED_UNIT_GRAPH_SHA256",
        "KAGEMUSHA_IOS_DEVICE_EVIDENCE_ROOT",
        "KAGEMUSHA_IOS_DEVICE_EVIDENCE_TRUSTED_KEY_ID",
        "KAGEMUSHA_IOS_DEVICE_EVIDENCE_TRUSTED_PUBLIC_KEY",
        "KAGEMUSHA_IOS_DEVICE_EVIDENCE_PRODUCTION_POLICY",
        "KAGEMUSHA_IOS_DEVICE_EVIDENCE_FRESHNESS_TRUSTED_KEY_ID",
        "KAGEMUSHA_IOS_DEVICE_EVIDENCE_FRESHNESS_TRUSTED_PUBLIC_KEY",
    )
    for name in required:
        assert f"{name}: ${{{{ vars.{name} }}}}" in promotion
        assert f'{name}="${name}"' in promotion
    assert "sudo -n /usr/bin/env -i" in promotion
    assert "KAGEMUSHA_PRODUCTION_READINESS_ROOT" not in promotion


def test_python_runtime_is_a_full_preimport_tree_closure() -> None:
    """Interpreter-only pins cannot authenticate imported standard-library code."""

    gate = (ROOT / "ci/check_kagemusha_production_readiness.sh").read_text(
        encoding="utf-8"
    )
    shell = gate.split("<<'PY'", 1)[0]
    tree_check = shell.index("promotion_root_tree_sha256()")
    version_probe = shell.index("sys.version_info >= (3, 10)")
    interpreter = shell.index('"${PYTHON_BIN}" -I -S - "${ROOT_DIR}"')
    assert tree_check < version_probe < interpreter
    assert "/private/var/db/iroha-kagemusha-python-runtime-v1" in shell
    assert '"${PYTHON_BIN}" != "${PYTHON_RUNTIME_ROOT}/bin/python3"' in shell
    assert "/usr/bin/find -s" in shell
    assert "-print0" in shell
    assert "has unbound extended attributes" in shell
    assert "contains a symbolic link" in shell
    assert "contains a group/world-writable entry" in shell
    assert "differs from its trusted tree SHA-256" in shell
    assert "changed before interpreter execution" in shell
    assert "changed during the production gate" in gate
    embedded = gate.split("<<'PY'\n", 1)[1].rsplit("\nPY\n", 1)[0]
    assert "trusted_python_runtime_root" in embedded
    assert "promotion Python import path escapes the sealed runtime tree" in embedded
    assert "imported promotion Python module" in embedded
    assert "MACOS_LIBC.flistxattr" in embedded


def test_source_contract_provider_executes_only_authenticated_exact_bytes() -> None:
    """Promotion must never reopen the static provider after authenticating it."""

    gate = READINESS_GATE.read_text(encoding="utf-8")
    promotion = gate.rsplit("def promotion_errors()", 1)[1].split(
        "source_contract_errors: list[str] = []", 1
    )[0]
    provider_auth = promotion.split(
        'label = f"reviewed readiness source-contract provider', 1
    )[1].split("if self_test:", 1)[0]
    custody = provider_auth.index("require_production_root_custody(descriptor, label)")
    descriptor_read = provider_auth.index("source_contract_bytes = read_pinned_descriptor(")
    closure_auth = provider_auth.index(
        "authenticate_reviewed_source_file(\n"
        "            READINESS_SOURCE_CONTRACT,"
    )
    retain = provider_auth.index(
        "authenticated_readiness_source_contract_bytes = source_contract_bytes"
    )
    assert custody < descriptor_read < closure_auth < retain

    dispatch = gate.rsplit("source_contract_errors: list[str] = []", 1)[1].split(
        "errors = source_contract_errors", 1
    )[0]
    promotion_branch, candidate_branch = dispatch.split("\nelse:\n", 1)
    assert "read_bytes" not in promotion_branch
    assert (
        "source_contract_bytes = authenticated_readiness_source_contract_bytes"
        in promotion_branch
    )
    assert dispatch.count("(root / READINESS_SOURCE_CONTRACT).read_bytes()") == 1
    assert "source_contract_bytes = (root / READINESS_SOURCE_CONTRACT).read_bytes()" in candidate_branch
    assert "code = compile(\n            source_contract_bytes," in dispatch
    assert "exec(code, source_contract_context, source_contract_context)" in dispatch
    assert READINESS_SOURCE_CONTRACT.stat().st_size <= 128 * 1024


def test_promotion_gate_reconstructs_the_projection_from_every_signed_input() -> None:
    """A pinned hand-authored projection must not bypass signed graph reconstruction."""

    gate = (ROOT / "ci/check_kagemusha_production_readiness.sh").read_text(
        encoding="utf-8"
    )
    promotion = gate.rsplit("def promotion_errors()", 1)[1]
    source_identity = promotion.index("source_identity = parsed_identity")
    snapshot = promotion.index("snapshot_private_python_package(")
    verify = promotion.index("run_kagemusha_source_projection_snapshot.py")
    artifact_scan = promotion.index("for path in artifact_root.iterdir()")
    assert source_identity < snapshot < verify < artifact_scan
    assert '"scripts/profile_cargo_build.py"' in gate
    assert '"scripts/build_kagemusha_v4_candidate_bundle.py"' in gate
    assert (
        '"-I",\n                "-S",\n'
        '                str(trusted_source_projection_launcher)'
        in promotion
    )
    assert "str(producer_package_root)" in promotion[verify:artifact_scan]
    for argument in (
        "--authorization",
        "--controller-signature",
        "--controller-allowed-signers",
        "--controller-revocation",
        "--execution-policy",
        "--raw-unit-graph",
        "--unit-graph",
        "--projection",
    ):
        assert argument in promotion
    assert "source-projection reconstruction receipt differs from the signed inputs" in promotion
    assert "manifest build tools differ from the authenticated execution policy" in promotion


def test_both_protected_taira_macos_hosts_requalify_before_controller_use() -> None:
    """A build-host result cannot substitute for qualification on protected hosts."""

    source = TAIRA_WORKFLOW.read_text(encoding="utf-8")
    qualification = source.split("  macos-secret-free-qualification:\n", 1)[1].split(
        "  macos-candidate-authority:\n", 1
    )[0]
    deploy = source.split("  macos-deploy:\n", 1)[1].split(
        "  macos-publish:\n", 1
    )[0]
    for job in (qualification, deploy):
        install = job.index("/usr/bin/install -o root -g wheel -m 0555")
        identity = job.index("installed authenticated-tool controller identity is invalid")
        qualify = job.index("qualify-host-v1")
        protected_use = job.index("prepare-reset --")
        assert install < identity < qualify < protected_use
        assert "authenticated-tool-controller: macOS host qualification passed" in job
