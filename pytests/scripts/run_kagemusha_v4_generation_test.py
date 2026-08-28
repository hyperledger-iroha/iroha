"""Tests for the guarded Kagemusha V4 candidate-generation launcher."""

from __future__ import annotations

import importlib.util
from dataclasses import replace
import hashlib
import json
import os
from pathlib import Path
import shlex
import stat
import sys

import pytest


ROOT_DIR = Path(__file__).resolve().parents[2]
RUNNER_PATH = ROOT_DIR / "scripts" / "run_kagemusha_v4_generation.py"
SPEC = importlib.util.spec_from_file_location("run_kagemusha_v4_generation", RUNNER_PATH)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


@pytest.fixture(autouse=True)
def _admit_test_output_filesystem(monkeypatch) -> None:
    """Keep tmp_path portable while filesystem-policy tests override explicitly."""

    monkeypatch.setattr(MODULE, "_filesystem_type", lambda _path: "ext4")


def test_runner_disables_local_bytecode_before_importing_guard_modules() -> None:
    source = RUNNER_PATH.read_text(encoding="utf-8")
    disable_offset = source.index("sys.dont_write_bytecode = True")
    local_import_offset = source.index(
        "from scripts.formal import run_sumeragi_v2_tlapm_guard"
    )

    assert disable_offset < local_import_offset


def test_candidate_child_environment_rejects_ambient_loader_and_tool_overrides(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    hostile = {
        "PATH": str(tmp_path / "attacker-tools"),
        "LD_PRELOAD": str(tmp_path / "inject.so"),
        "LD_LIBRARY_PATH": str(tmp_path / "libraries"),
        "DYLD_INSERT_LIBRARIES": str(tmp_path / "inject.dylib"),
        "DYLD_LIBRARY_PATH": str(tmp_path / "frameworks"),
        "RUST_LOG": "trace",
        "RAYON_NUM_THREADS": "4096",
        "PYTHONPATH": str(tmp_path / "python"),
    }
    for key, value in hostile.items():
        monkeypatch.setenv(key, value)

    environment = MODULE._candidate_child_environment(tmp_path)

    assert environment == {
        "HOME": "/var/empty",
        "LANG": "C",
        "LC_ALL": "C",
        "PATH": "/usr/bin:/bin",
        "TMPDIR": str(tmp_path),
    }
    assert environment["PATH"] != hostile["PATH"]
    assert not (set(hostile) - {"PATH"}).intersection(environment)


def _fake_prebuilt_generator(tmp_path: Path, mode: str = "success") -> Path:
    """Create a loader-stable shell double for the admitted native generator.

    These integration-style tests exercise the Python supervisor itself. A
    second Homebrew Python process is not part of that contract and can spend
    minutes blocked in dyld on a loaded macOS builder, so the child double uses
    the OS shell and shell builtins instead.
    """

    if mode not in {
        "success",
        "direct_rename",
        "fail_with_residue",
        "malformed_capacity",
        "missing_receipt",
        "post_rename_failure",
        "replace_original",
        "replace_staging_directory",
    }:
        raise ValueError(f"unsupported fake generator mode: {mode}")
    executable = tmp_path / "kagemusha_recursive_spend_v4_bundle"
    helper = tmp_path / f"fake-kagemusha-generator-{mode}.sh"
    helper.write_text(
        f"""#!/bin/sh
set -eu
mode={mode!r}
execution_path=$1
shift
operation=$1
shift
if [ "$operation" = memory-capacity-v1 ]; then
    if [ "$mode" = malformed_capacity ]; then
        printf '%s\n' 'iroha.kagemusha.memory-capacity.v1 malformed'
        exit 0
    fi
    printf '%s\n' 'iroha.kagemusha.memory-capacity.v1 physical=12884901888 ceiling=6442450944 absolute=68719476736 profile=self-physical-footprint-v1 policy=half-effective-physical-cap-absolute-v1'
    exit 0
fi
out_dir=
staging_id=
staging_name=
parent_fd=
source_commit=
source_tree_sha256=
memory_limit_bytes=
while [ "$#" -gt 0 ]; do
    case "$1" in
        --out-dir) out_dir=$2; shift 2 ;;
        --staging-id) staging_id=$2; shift 2 ;;
        --staging-name) staging_name=$2; shift 2 ;;
        --output-parent-fd) parent_fd=$2; shift 2 ;;
        --source-commit) source_commit=$2; shift 2 ;;
        --source-tree-sha256) source_tree_sha256=$2; shift 2 ;;
        --memory-limit-bytes) memory_limit_bytes=$2; shift 2 ;;
        *) shift ;;
    esac
done
case "$parent_fd" in
    ''|*[!0-9]*) exit 92 ;;
esac
[ "$staging_name" = ".kagemusha-v4-staging-${{staging_id}}-work" ] || exit 93
parent_path=/dev/fd/$parent_fd
[ -d "$parent_path" ] || exit 94
output_parent=${{out_dir%/*}}
staging_path="$output_parent/$staging_name"
[ -d "$staging_path" ] || exit 94
case "$memory_limit_bytes" in
    ''|0|*[!0-9]*) exit 91 ;;
esac
parent_observation=$output_parent
printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
    "$operation" "$parent_observation" "$execution_path" "$out_dir" \
    "$staging_id" "$staging_name" "$parent_fd" "$source_commit" \
    "$source_tree_sha256" "$memory_limit_bytes" \
    >> "$staging_path/fd-observations"
if [ "$mode" = fail_with_residue ] && [ "$operation" = generate-candidate ]; then
    printf residue > "$staging_path/large-key.part"
    exit 9
fi
if [ "$mode" = direct_rename ] && [ "$operation" = generate-candidate ]; then
    /bin/mv "$staging_path" "$out_dir"
    exit 0
fi
if [ "$mode" = replace_staging_directory ] && [ "$operation" = generate-candidate ]; then
    /bin/mv "$staging_path" "$output_parent/${{staging_name}}-displaced"
    /bin/mkdir -m 700 "$staging_path"
fi
if [ "$mode" = replace_original ] && [ "$operation" = generate-candidate ]; then
    original={shlex.quote(str(executable))}
    original_parent={shlex.quote(str(executable.parent))}
    /bin/mv "$original" "$original_parent/admitted-original"
    printf '#!/bin/sh\nexit 0\n' > "$original"
    chmod 700 "$original"
    printf yes > "$original_parent/admitted-copy-ran"
fi
if [ "$operation" = generate-candidate ] && [ "$mode" != missing_receipt ]; then
    printf qualification > \
        "$staging_path/recursive-step-two-qualification-v4.norito"
fi
if [ "$operation" = publish-staged-candidate ]; then
    [ -f "$staging_path/recursive-step-two-qualification-v4.norito" ] || exit 95
    /bin/mv "$staging_path" "$out_dir"
    final_path_hex=$(printf '%s' "$out_dir" | /usr/bin/od -An -v -tx1 | /usr/bin/tr -d ' \n')
    if [ "$mode" = post_rename_failure ]; then
        printf '%s\n' "iroha.kagemusha.publication_outcome.v1 status=commit-uncertain final_path_encoding=bytes-hex final_path_hex=$final_path_hex parent_directory_durable=0 parent_sync_error_utf8_hex=73796e746865746963" >&2
        exit 75
    fi
    printf '%s\n' "iroha.kagemusha.publication_outcome.v1 status=committed final_path_encoding=bytes-hex final_path_hex=$final_path_hex parent_directory_durable=1 parent_sync_error_utf8_hex=-"
fi
""",
        encoding="utf-8",
    )
    helper.chmod(0o600)
    executable.write_text(
        "#!/bin/sh\n"
        f"exec /bin/sh {shlex.quote(str(helper))} \"$0\" \"$@\"\n",
        encoding="utf-8",
    )
    executable.chmod(0o700)
    return executable


def _generation_command(executable: Path, out_dir: Path) -> list[str]:
    """Return one complete bundle command with immutable source identity."""

    return [
        str(executable),
        "generate-candidate",
        "--out-dir",
        str(out_dir),
        "--source-commit",
        "0" * 40,
        "--source-tree-sha256",
        "1" * 64,
    ]


def _prepare_guarded_command(
    command: list[str],
) -> tuple[
    list[str],
    MODULE.PinnedOutputParent,
    MODULE.CandidatePublicationContract,
]:
    """Prepare a test command under one fixed finite kernel ceiling."""

    return MODULE._prepare_guarded_command(
        command,
    )


def _sealed_build_report(tmp_path: Path, executable: Path) -> tuple[Path, str]:
    """Write one canonical two-build report authenticating the test generator."""

    generator_bytes = executable.read_bytes()
    generator_sha256 = hashlib.sha256(generator_bytes).hexdigest()
    generator_size = len(generator_bytes)
    common = {
        "authenticated_source_seal_projection_sha256": "1" * 64,
        "build_inputs_sha256": "2" * 64,
        "cargo_binary_sha256": "3" * 64,
        "cargo_semantic_argv": [
            "build", "--release", "--locked", "--offline", "--target",
            "aarch64-apple-darwin", "--target-dir", "<EXTERNAL_TARGET_DIR>",
            "-p", "iroha_core", "--features",
            "iroha_core/dev-tools,iroha_core/kagemusha-candidate-source-seal,iroha_core/kagemusha-candidate-evidence-lab",
            "--bin", MODULE.BUNDLE_EXECUTABLE, "--jobs", "1",
            "--message-format=json-render-diagnostics",
        ],
        "execution_policy_sha256": "4" * 64,
        "normalized_unit_graph_sha256": "5" * 64,
        "reviewed_source_closure_sha256": "6" * 64,
        "runtime_gid": os.getgid(),
        "runtime_uid": os.getuid(),
        "rustc_binary_sha256": "7" * 64,
        "source_commit": "8" * 40,
        "source_date_epoch": 1_786_749_504,
        "source_tree_sha256": "9" * 64,
        "target": "aarch64-apple-darwin",
    }
    builds = []
    for ordinal, source_role, target_role, binary_path in (
        (1, "authenticated-primary-source-snapshot-v1", "fresh-primary-target-v1", str(executable)),
        (2, "authenticated-independent-source-snapshot-v1", "fresh-verification-target-v1", str(executable) + ".verification"),
    ):
        identity = {
            **common,
            "ordinal": ordinal,
            "source_snapshot_role": source_role,
            "target_role": target_role,
        }
        builds.append({
            "identity": identity,
            "identity_sha256": hashlib.sha256(
                MODULE.resource_guard._canonical_json(identity)
            ).hexdigest(),
            "output": {
                "binary_path": binary_path,
                "sha256": generator_sha256,
                "size_bytes": generator_size,
            },
        })
    report = {
        "authenticated_source_seal_projection_sha256": "1" * 64,
        "binary_path": str(executable),
        "binary_sha256": generator_sha256,
        "binary_size_bytes": generator_size,
        "build_profile": "release",
        "builds": builds,
        "byte_equality": {
            "algorithm": "sha256-size-and-final-descriptor-rehash-v1",
            "equal": True,
            "sha256": generator_sha256,
            "size_bytes": generator_size,
        },
        "candidate_generator": {
            "selected_build_ordinal": 1,
            "sha256": generator_sha256,
            "size_bytes": generator_size,
        },
        "minimum_build_physical_memory_bytes": 1,
        "physical_memory_bytes_at_admission": 2,
        "reproducible_build_count": 2,
        "reviewed_cargo_binary_sha256": "3" * 64,
        "reviewed_rustc_binary_sha256": "7" * 64,
        "reviewed_source_closure": {},
        "reviewed_source_closure_descriptor_sha256": "6" * 64,
        "schema": MODULE.SEALED_BUILD_REPORT_SCHEMA,
        "source_commit": "8" * 40,
        "source_date_epoch": 1_786_749_504,
        "source_repo_dirty": False,
        "source_tree_sha256": "9" * 64,
        "target_dir": str(tmp_path / "target"),
        "unit_graph_preflight": {},
        "verification_binary_path": str(executable) + ".verification",
    }
    inner_payload = MODULE.resource_guard._canonical_json(report)
    native_launch = {
        "argument_contract": MODULE.NATIVE_SEALED_BUILDER_ARGUMENT_CONTRACT,
        "argument_sha256": "a" * 64,
        "builder_entrypoint_sha256": "b" * 64,
        "contract": MODULE.NATIVE_SEALED_BUILDER_LAUNCH_CONTRACT,
        "controller_sha256": "c" * 64,
        "environment_contract": MODULE.NATIVE_SEALED_BUILDER_ENVIRONMENT_CONTRACT,
        "environment_sha256": "d" * 64,
        "macos_build": "25A1",
        "os_tcb_contract": MODULE.NATIVE_SEALED_BUILDER_OS_TCB_CONTRACT,
        "os_tcb_sha256": "e" * 64,
        "python_interpreter_sha256": "f" * 64,
        "python_runtime_tree_sha256": "1" * 64,
        "report_publication_contract": (
            MODULE.NATIVE_SEALED_BUILDER_REPORT_PUBLICATION_CONTRACT
        ),
        "runtime_dependency_contract": (
            MODULE.NATIVE_SEALED_BUILDER_RUNTIME_DEPENDENCY_CONTRACT
        ),
    }
    envelope = {
        "builder_report_hex": inner_payload.hex(),
        "builder_report_sha256": hashlib.sha256(inner_payload).hexdigest(),
        "builder_report_size_bytes": len(inner_payload),
        "native_launch": native_launch,
        "schema": MODULE.NATIVE_SEALED_BUILD_REPORT_SCHEMA,
    }
    payload = MODULE.resource_guard._canonical_json(envelope)
    path = tmp_path / "sealed-build-report.json"
    path.write_bytes(payload)
    path.chmod(0o600)
    return path, hashlib.sha256(payload).hexdigest()


def _guarded_args(tmp_path: Path, executable: Path) -> list[str]:
    build_report, build_report_sha256 = _sealed_build_report(tmp_path, executable)
    return [
        "--resource-report",
        str(tmp_path / "resource-report"),
        "--sealed-build-report",
        str(build_report),
        "--sealed-build-report-sha256",
        build_report_sha256,
        "--",
        *_generation_command(executable, tmp_path / "candidate"),
    ]


def test_sealed_build_report_rejects_second_build_substitution(tmp_path: Path) -> None:
    executable = _fake_prebuilt_generator(tmp_path)
    report_path, _digest = _sealed_build_report(tmp_path, executable)
    envelope = json.loads(report_path.read_text(encoding="utf-8"))
    report = json.loads(bytes.fromhex(envelope["builder_report_hex"]))
    report["builds"][1]["output"]["sha256"] = "a" * 64
    inner_payload = MODULE.resource_guard._canonical_json(report)
    envelope["builder_report_hex"] = inner_payload.hex()
    envelope["builder_report_sha256"] = hashlib.sha256(inner_payload).hexdigest()
    envelope["builder_report_size_bytes"] = len(inner_payload)
    payload = MODULE.resource_guard._canonical_json(envelope)
    report_path.write_bytes(payload)

    with pytest.raises(
        MODULE.resource_guard.GuardError, match="independent and equal"
    ):
        MODULE._open_sealed_build_report(
            report_path, hashlib.sha256(payload).hexdigest()
        )


def test_direct_python_v1_build_report_is_not_promotion_admissible(tmp_path: Path) -> None:
    executable = _fake_prebuilt_generator(tmp_path)
    report_path, _digest = _sealed_build_report(tmp_path, executable)
    envelope = json.loads(report_path.read_text(encoding="utf-8"))
    direct_payload = bytes.fromhex(envelope["builder_report_hex"])
    report_path.write_bytes(direct_payload)

    with pytest.raises(
        MODULE.resource_guard.GuardError, match="native-launch envelope"
    ):
        MODULE._open_sealed_build_report(
            report_path, hashlib.sha256(direct_payload).hexdigest()
        )


def test_launcher_rejects_generator_not_named_by_sealed_report(tmp_path: Path) -> None:
    executable = _fake_prebuilt_generator(tmp_path)
    report_path, report_sha256 = _sealed_build_report(tmp_path, executable)
    executable.write_bytes(executable.read_bytes() + b"\n")
    executable.chmod(0o700)

    assert MODULE.main([
        "--resource-report", str(tmp_path / "resource-report"),
        "--sealed-build-report", str(report_path),
        "--sealed-build-report-sha256", report_sha256,
        "--", *_generation_command(executable, tmp_path / "candidate"),
    ]) == 1


def test_effective_limit_is_half_physical_and_cannot_be_raised(monkeypatch) -> None:
    monkeypatch.setattr(MODULE, "_physical_memory_bytes", lambda: 12 * MODULE.BYTES_PER_GIB)

    assert MODULE._effective_memory_limit_bytes(None) == 6 * MODULE.BYTES_PER_GIB
    assert MODULE._effective_memory_limit_bytes(0.125) == 128 * 1024 * 1024
    with pytest.raises(MODULE.resource_guard.GuardError, match="cannot raise"):
        MODULE._effective_memory_limit_bytes(7)
    with pytest.raises(MODULE.resource_guard.GuardError, match="greater than zero"):
        MODULE._effective_memory_limit_bytes(float("nan"))

    monkeypatch.setattr(
        MODULE,
        "_physical_memory_bytes",
        lambda: 64 * MODULE.BYTES_PER_GIB,
    )
    assert (
        MODULE._effective_memory_limit_bytes(None)
        == 32 * MODULE.BYTES_PER_GIB
    )
    with pytest.raises(MODULE.resource_guard.GuardError, match="cannot raise"):
        MODULE._effective_memory_limit_bytes(32.01)

    monkeypatch.setattr(
        MODULE,
        "_physical_memory_bytes",
        lambda: 128 * MODULE.BYTES_PER_GIB,
    )
    assert (
        MODULE._effective_memory_limit_bytes(None)
        == 64 * MODULE.BYTES_PER_GIB
    )
    with pytest.raises(MODULE.resource_guard.GuardError, match="cannot raise"):
        MODULE._effective_memory_limit_bytes(64.01)

    monkeypatch.setattr(MODULE, "_physical_memory_bytes", lambda: 0)
    with pytest.raises(
        MODULE.resource_guard.GuardError,
        match="could not determine installed physical memory",
    ):
        MODULE._effective_memory_limit_bytes(None)


def test_physical_memory_detection_fails_closed(monkeypatch) -> None:
    monkeypatch.setattr(MODULE.sys, "platform", "unsupported-posix")
    monkeypatch.setattr(MODULE.os, "sysconf", lambda _name: None)

    assert MODULE._physical_memory_bytes() == 0


def _memory_capacity_payload(
    *,
    physical: int = 12 * MODULE.BYTES_PER_GIB,
    ceiling: int = 6 * MODULE.BYTES_PER_GIB,
    absolute: int = MODULE.ABSOLUTE_MAX_MEMORY_BYTES,
    profile: str = MODULE.MEMORY_ENFORCEMENT_PROFILE,
    policy: str = MODULE.MEMORY_CAPACITY_POLICY,
) -> bytes:
    """Return one canonical Rust memory-policy query result."""

    return (
        f"{MODULE.MEMORY_CAPACITY_SCHEMA} physical={physical} ceiling={ceiling} "
        f"absolute={absolute} profile={profile} policy={policy}\n"
    ).encode("ascii")


def test_memory_capacity_query_is_exact_and_only_allows_lower_overrides() -> None:
    capacity = MODULE._validate_memory_capacity_outcome(_memory_capacity_payload())

    assert capacity.effective_physical_capacity_bytes == 12 * MODULE.BYTES_PER_GIB
    assert capacity.safety_ceiling_bytes == 6 * MODULE.BYTES_PER_GIB
    assert MODULE._apply_optional_memory_limit_bytes(capacity, None) == (
        6 * MODULE.BYTES_PER_GIB
    )
    assert MODULE._apply_optional_memory_limit_bytes(capacity, 0.125) == (
        128 * 1024 * 1024
    )
    with pytest.raises(MODULE.resource_guard.GuardError, match="cannot raise"):
        MODULE._apply_optional_memory_limit_bytes(capacity, 7)


@pytest.mark.parametrize(
    "payload,error",
    (
        (b"iroha.kagemusha.memory-capacity.v1 malformed\n", "schema"),
        (_memory_capacity_payload(absolute=1), "absolute maximum"),
        (_memory_capacity_payload(profile="rss-only-v0"), "profile"),
        (_memory_capacity_payload(policy="host-only-v0"), "policy"),
        (_memory_capacity_payload(ceiling=13 * MODULE.BYTES_PER_GIB), "bounds"),
    ),
)
def test_memory_capacity_query_rejects_malformed_or_mismatched_results(
    payload: bytes, error: str
) -> None:
    with pytest.raises(MODULE.resource_guard.GuardError, match=error):
        MODULE._validate_memory_capacity_outcome(payload)


def test_memory_capacity_query_detects_executable_path_substitution(
    tmp_path: Path, monkeypatch
) -> None:
    executable = _fake_prebuilt_generator(tmp_path)
    snapshot = MODULE._snapshot_executable(str(executable), MODULE.BUNDLE_EXECUTABLE)
    displaced = tmp_path / "admitted-original"
    capacity = MODULE._validate_memory_capacity_outcome(_memory_capacity_payload())

    def substitute_then_return(*_args, **_kwargs):
        executable.replace(displaced)
        executable.write_text("#!/bin/sh\nexit 0\n", encoding="ascii")
        executable.chmod(0o700)
        return capacity

    monkeypatch.setattr(MODULE, "_run_pinned_bundle_command", substitute_then_return)
    try:
        with pytest.raises(
            MODULE.resource_guard.GuardError, match="executable changed after admission"
        ):
            MODULE._query_generation_memory_capacity(snapshot)
    finally:
        snapshot.close()


def test_malformed_pinned_memory_query_cleans_journal_and_private_copy(
    tmp_path: Path, monkeypatch
) -> None:
    monkeypatch.setattr(
        MODULE.resource_guard, "HEAVY_JOB_LOCK_PATH", tmp_path / "heavy.lock"
    )
    monkeypatch.setattr(MODULE, "LOCK_PATH", tmp_path / "kagemusha.lock")
    executable = _fake_prebuilt_generator(tmp_path, "malformed_capacity")

    assert MODULE.main(_guarded_args(tmp_path, executable)) == 1
    assert not (tmp_path / "candidate").exists()
    assert not (tmp_path / "resource-report").exists()
    assert not list(tmp_path.glob(f"{MODULE.JOURNAL_PREFIX}*"))
    assert not list(tmp_path.glob(f"{MODULE.STAGING_PREFIX}*"))


def test_runner_executes_small_owned_group_and_writes_reports(
    tmp_path: Path, monkeypatch
) -> None:
    monkeypatch.setattr(
        MODULE.resource_guard, "HEAVY_JOB_LOCK_PATH", tmp_path / "heavy.lock"
    )
    monkeypatch.setattr(MODULE, "LOCK_PATH", tmp_path / "kagemusha.lock")
    report_root = tmp_path / "resource-report"

    executable = _fake_prebuilt_generator(tmp_path)
    assert (
        MODULE.main(_guarded_args(tmp_path, executable))
        == 0
    )
    summary = json.loads(
        (report_root / "kagemusha_resource_summary.json").read_text(encoding="utf-8")
    )
    assert summary["exit_reason"] == "completed"
    assert summary["exit_status"] == 0
    assert summary["post_run_cleanup"] == "completed"
    executable_identity = summary["report_context"]["executable_identity"]
    execution = executable_identity["execution"]
    assert execution["method"] in {"darwin_private_fd_copy", "pinned_fd"}
    assert summary["post_run_cleanup_removed"] == (
        1 if execution["method"] == "darwin_private_fd_copy" else 0
    )
    assert summary["post_run_validation"] == "completed"
    assert summary["post_success_finalize"] == "completed"
    assert summary["post_success_finalize_result"] == 1
    assert (tmp_path / "candidate").is_dir()
    assert summary["report_context"]["output_parent"]["canonical_path"] == str(
        tmp_path.resolve()
    )
    observations = (tmp_path / "candidate" / "fd-observations").read_text(
        encoding="ascii"
    ).splitlines()
    assert len(observations) == 2
    execution_paths: list[str] = []
    publication_contract = summary["report_context"]["publication_contract"]
    for operation, observation, expected_out_dir in zip(
        ("generate-candidate", "publish-staged-candidate"),
        observations,
        (
            publication_contract["guarded_out_dir"],
            publication_contract["requested_out_dir"],
        ),
    ):
        fields = observation.split("\t")
        assert fields[:2] == [
            operation,
            summary["report_context"]["output_parent"]["canonical_path"],
        ]
        assert fields[3:] == [
            expected_out_dir,
            publication_contract["staging_id"],
            publication_contract["staging_name"],
            str(publication_contract["output_parent_descriptor"]),
            publication_contract["source_commit"],
            publication_contract["source_tree_sha256"],
            str(summary["memory_limit_bytes"]),
        ]
        assert len(fields) == 10
        execution_paths.append(fields[2])
    assert execution_paths[0] == execution_paths[1]
    execution_path_key = (
        "canonical_path"
        if execution["method"] == "darwin_private_fd_copy"
        else "descriptor_path"
    )
    assert execution_paths[0] == execution[execution_path_key]
    assert not list(tmp_path.glob(f"{MODULE.JOURNAL_PREFIX}*"))
    assert executable_identity["canonical_path"] == str(executable.resolve())
    assert executable_identity["sha256"] == hashlib.sha256(
        executable.read_bytes()
    ).hexdigest()
    assert executable_identity["size_bytes"] == executable.stat().st_size
    assert 0 < summary["memory_limit_bytes"] <= MODULE.ABSOLUTE_MAX_MEMORY_BYTES
    memory_capacity = summary["report_context"]["generation_memory_capacity"]
    assert memory_capacity == {
        "absolute_maximum_bytes": MODULE.ABSOLUTE_MAX_MEMORY_BYTES,
        "effective_physical_capacity_bytes": 12 * MODULE.BYTES_PER_GIB,
        "enforcement_profile": MODULE.MEMORY_ENFORCEMENT_PROFILE,
        "policy": MODULE.MEMORY_CAPACITY_POLICY,
        "safety_ceiling_bytes": 6 * MODULE.BYTES_PER_GIB,
        "schema": MODULE.MEMORY_CAPACITY_SCHEMA,
    }
    assert summary["memory_limit_bytes"] == memory_capacity["safety_ceiling_bytes"]
    assert (
        summary["memory_enforcement_mode"]
        == MODULE.resource_guard.MEMORY_ENFORCEMENT_MAX_RSS_OR_FOOTPRINT
    )
    assert summary["sample_interval_seconds"] == MODULE.SAMPLE_INTERVAL_SECONDS
    assert (
        summary["physical_footprint_interval_seconds"]
        == MODULE.SAMPLE_INTERVAL_SECONDS
    )
    assert (report_root / "kagemusha_resource.jsonl").stat().st_size > 0


def test_runner_does_not_use_the_retired_boolean_supervision_marker() -> None:
    source = RUNNER_PATH.read_text(encoding="utf-8")

    assert "IROHA_KAGEMUSHA_V4_RESOURCE_SUPERVISED" not in source
    assert "RESOURCE_GUARD_" + "AUTH" not in source
    assert "--auth" + "-fd" not in source
    assert "held_lock_descriptors=(heavy_lock, kagemusha_lock)" in source
    assert MODULE.ABSOLUTE_MAX_MEMORY_BYTES == 64 * MODULE.BYTES_PER_GIB
    assert MODULE.SAMPLE_INTERVAL_SECONDS == 0.25
    assert "sample_interval_seconds=SAMPLE_INTERVAL_SECONDS" in source
    assert "physical_footprint_interval_seconds" in source
    assert "MEMORY_ENFORCEMENT_MAX_RSS_OR_FOOTPRINT" in source
    assert MODULE.MEMORY_ENFORCEMENT_PROFILE == "self-physical-footprint-v1"
    assert 'MEMORY_LIMIT_OPTION = "--memory-limit-bytes"' in source


def test_publication_machine_outcomes_are_exact_and_path_bound() -> None:
    final_path = "/tmp/candidate-output"
    final_hex = os.fsencode(final_path).hex()
    committed = (
        "iroha.kagemusha.publication_outcome.v1 status=committed "
        f"final_path_encoding=bytes-hex final_path_hex={final_hex} "
        "parent_directory_durable=1 parent_sync_error_utf8_hex=-\n"
    ).encode("ascii")
    MODULE._validate_publication_outcome(
        committed,
        expected_status="committed",
        expected_final_path=final_path,
    )
    uncertain = (
        "iroha.kagemusha.publication_outcome.v1 status=commit-uncertain "
        f"final_path_encoding=bytes-hex final_path_hex={final_hex} "
        "parent_directory_durable=0 parent_sync_error_utf8_hex=73796e63206661696c6564\n"
    ).encode("ascii")
    MODULE._validate_publication_outcome(
        uncertain,
        expected_status="commit-uncertain",
        expected_final_path=final_path,
    )
    with pytest.raises(MODULE.resource_guard.GuardError, match="wrong final path"):
        MODULE._validate_publication_outcome(
            committed,
            expected_status="committed",
            expected_final_path=f"{final_path}-other",
        )


def test_publication_wrapper_record_is_fixed_size_and_path_bound() -> None:
    final_path = "/tmp/" + "candidate-output-" * 100
    final_hex = os.fsencode(final_path).hex()
    committed = (
        "iroha.kagemusha.publication_outcome.v1 status=committed "
        f"final_path_encoding=bytes-hex final_path_hex={final_hex} "
        "parent_directory_durable=1 parent_sync_error_utf8_hex=-\n"
    ).encode("ascii")

    record = MODULE._publication_control_record(
        0,
        committed,
        b"",
        expected_final_path=final_path,
    )

    assert len(record.encode("ascii")) + 1 <= 256
    assert (
        MODULE._validate_publication_control_record(
            record,
            returncode=0,
            expected_final_path=final_path,
        )
        == "committed"
    )
    with pytest.raises(MODULE.resource_guard.GuardError, match="wrong final path"):
        MODULE._validate_publication_control_record(
            record,
            returncode=0,
            expected_final_path=f"{final_path}-other",
        )


def test_candidate_parser_accepts_one_remainder_command() -> None:
    arguments = [
        "--resource-report",
        "resource-report",
        "--sealed-build-report",
        "sealed-build-report.json",
        "--sealed-build-report-sha256",
        "1" * 64,
        "--",
        MODULE.BUNDLE_EXECUTABLE,
        "generate-candidate",
    ]

    parsed = MODULE._parser().parse_args(arguments)

    assert parsed.command == [
        "--",
        MODULE.BUNDLE_EXECUTABLE,
        "generate-candidate",
    ]


def test_runner_refuses_retired_rss_only_report_mode(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    report = tmp_path / "legacy-report.json"

    assert MODULE.main(["--report", str(report), "--", "/usr/bin/true"]) == 2

    captured = capsys.readouterr()
    assert "--report mode is retired" in captured.err
    assert "RSS-only" in captured.err
    assert "--resource-report" in captured.err
    assert not report.exists()
    source = RUNNER_PATH.read_text(encoding="utf-8")
    assert "run_guarded_command" not in source


def test_runner_requires_prebuilt_generator_and_exact_subcommand(tmp_path: Path) -> None:
    report = tmp_path / "resource-report"
    fake = _fake_prebuilt_generator(tmp_path)
    build_report, build_report_sha256 = _sealed_build_report(tmp_path, fake)
    prefix = [
        "--resource-report", str(report),
        "--sealed-build-report", str(build_report),
        "--sealed-build-report-sha256", build_report_sha256,
        "--",
    ]
    assert MODULE.main(
        [*prefix, "cargo", "run"]
    ) == 1
    assert not report.exists()

    executable = fake
    assert MODULE.main(
        [*prefix,
            str(executable),
            "--help",
            "--out-dir",
            str(tmp_path / "candidate"),
        ]
    ) == 1
    assert not report.exists()


def test_runner_refuses_to_overwrite_resource_evidence(tmp_path: Path) -> None:
    report_root = tmp_path / "resource-report"
    report_root.mkdir()
    executable = _fake_prebuilt_generator(tmp_path)

    assert MODULE.main(_guarded_args(tmp_path, executable)) == 1


def test_runner_refuses_when_shared_heavy_job_lock_is_held(
    tmp_path: Path, monkeypatch
) -> None:
    heavy_lock = tmp_path / "heavy.lock"
    monkeypatch.setattr(MODULE.resource_guard, "HEAVY_JOB_LOCK_PATH", heavy_lock)
    monkeypatch.setattr(MODULE, "LOCK_PATH", tmp_path / "kagemusha.lock")
    executable = _fake_prebuilt_generator(tmp_path)

    with MODULE.resource_guard._host_lock(heavy_lock, description="memory-heavy job"):
        assert MODULE.main(
            _guarded_args(tmp_path, executable)
        ) == MODULE.resource_guard.LOCK_UNAVAILABLE_EXIT_CODE


def test_runner_injects_private_staging_id_and_removes_failure_residue(
    tmp_path: Path, monkeypatch
) -> None:
    monkeypatch.setattr(
        MODULE.resource_guard, "HEAVY_JOB_LOCK_PATH", tmp_path / "heavy.lock"
    )
    monkeypatch.setattr(MODULE, "LOCK_PATH", tmp_path / "kagemusha.lock")
    executable = _fake_prebuilt_generator(tmp_path, "fail_with_residue")

    assert MODULE.main(_guarded_args(tmp_path, executable)) == 9
    assert not list(tmp_path.glob(".kagemusha-v4-staging-*-work"))
    assert not (tmp_path / "candidate").exists()
    summary = json.loads(
        (tmp_path / "resource-report" / "kagemusha_resource_summary.json").read_text(
            encoding="utf-8"
        )
    )
    assert summary["post_run_cleanup"] == "completed"
    execution_method = summary["report_context"]["executable_identity"]["execution"][
        "method"
    ]
    assert execution_method in {"darwin_private_fd_copy", "pinned_fd"}
    assert summary["post_run_cleanup_removed"] == (
        2 if execution_method == "darwin_private_fd_copy" else 1
    )
    assert summary["post_success_finalize"] == "skipped"


@pytest.mark.parametrize("mode", ["missing_receipt", "direct_rename"])
def test_runner_rejects_publication_bypass_without_final_output(
    tmp_path: Path, monkeypatch, mode: str
) -> None:
    monkeypatch.setattr(
        MODULE.resource_guard, "HEAVY_JOB_LOCK_PATH", tmp_path / "heavy.lock"
    )
    monkeypatch.setattr(MODULE, "LOCK_PATH", tmp_path / "kagemusha.lock")
    executable = _fake_prebuilt_generator(tmp_path, mode)

    assert MODULE.main(_guarded_args(tmp_path, executable)) == 1

    summary = json.loads(
        (tmp_path / "resource-report" / "kagemusha_resource_summary.json").read_text(
            encoding="utf-8"
        )
    )
    assert not (tmp_path / "candidate").exists()
    assert not list(tmp_path.glob(f"{MODULE.STAGING_PREFIX}*"))
    assert not list(tmp_path.glob(f"{MODULE.JOURNAL_PREFIX}*"))
    assert summary["post_run_cleanup"] == "completed"
    if mode == "missing_receipt":
        assert summary["post_run_validation"] == "completed"
        assert summary["post_success_finalize"] == "failed"
    else:
        assert summary["post_run_validation"] == "failed"
        assert summary["post_success_finalize"] == "skipped"


def test_runner_rejects_same_name_staging_directory_substitution(
    tmp_path: Path, monkeypatch
) -> None:
    monkeypatch.setattr(
        MODULE.resource_guard, "HEAVY_JOB_LOCK_PATH", tmp_path / "heavy.lock"
    )
    monkeypatch.setattr(MODULE, "LOCK_PATH", tmp_path / "kagemusha.lock")
    executable = _fake_prebuilt_generator(tmp_path, "replace_staging_directory")

    assert MODULE.main(_guarded_args(tmp_path, executable)) == 1

    summary = json.loads(
        (tmp_path / "resource-report" / "kagemusha_resource_summary.json").read_text(
            encoding="utf-8"
        )
    )
    assert summary["exit_reason"] == "post_run_validation_error"
    assert summary["post_run_validation"] == "failed"
    assert summary["post_success_finalize"] == "skipped"
    assert summary["post_run_cleanup"] == "completed"
    assert not (tmp_path / "candidate").exists()
    assert not list(tmp_path.glob(f"{MODULE.STAGING_PREFIX}*"))
    assert not list(tmp_path.glob(f"{MODULE.JOURNAL_PREFIX}*"))


def test_post_rename_publisher_failure_retains_final_and_recovery_journal(
    tmp_path: Path, monkeypatch
) -> None:
    monkeypatch.setattr(
        MODULE.resource_guard, "HEAVY_JOB_LOCK_PATH", tmp_path / "heavy.lock"
    )
    monkeypatch.setattr(MODULE, "LOCK_PATH", tmp_path / "kagemusha.lock")
    executable = _fake_prebuilt_generator(tmp_path, "post_rename_failure")

    assert MODULE.main(_guarded_args(tmp_path, executable)) == 1

    summary = json.loads(
        (tmp_path / "resource-report" / "kagemusha_resource_summary.json").read_text(
            encoding="utf-8"
        )
    )
    assert (tmp_path / "candidate").is_dir()
    journals = list(tmp_path.glob(f"{MODULE.JOURNAL_PREFIX}*"))
    assert len(journals) == 1
    assert summary["post_run_validation"] == "completed"
    assert summary["post_success_finalize"] == "failed"
    assert summary["post_run_cleanup"] == "failed"
    assert summary["exit_reason"] == "post_success_finalize_error"
    assert not list(tmp_path.glob(f"{MODULE.STAGING_PREFIX}*"))


def test_cleanup_staging_is_fd_relative_and_does_not_follow_symlinks(
    tmp_path: Path,
) -> None:
    outside = tmp_path / "outside"
    outside.mkdir()
    sentinel = outside / "retain"
    sentinel.write_bytes(b"outside")
    output_parent = tmp_path / "output"
    output_parent.mkdir()
    command = _generation_command(
        _fake_prebuilt_generator(tmp_path), output_parent / "candidate"
    )
    _guarded, parent, contract = _prepare_guarded_command(command)
    staging_id = contract.staging_id
    residue_name = f"{MODULE.STAGING_PREFIX}{staging_id}-work"
    residue = output_parent / residue_name
    try:
        residue.mkdir(mode=0o700)
        nested = residue / "nested"
        nested.mkdir(mode=0o700)
        (nested / "partial").write_bytes(b"partial")
        (residue / "outside-link").symlink_to(outside, target_is_directory=True)
        nested.chmod(0o500)
        residue.chmod(0o500)
        moved_parent = tmp_path / "moved-output"
        output_parent.rename(moved_parent)
        output_parent.mkdir()
        decoy = output_parent / residue_name
        decoy.mkdir(mode=0o700)

        assert MODULE._cleanup_staging(parent, staging_id) == 1
        assert not (moved_parent / residue_name).exists()
        assert decoy.is_dir()
        assert sentinel.read_bytes() == b"outside"
    finally:
        parent.close()


@pytest.mark.parametrize(
    ("field", "value"),
    [
        ("staging_id", "f" * MODULE.STAGING_ID_HEX_LENGTH),
        (
            "staging_name",
            f"{MODULE.STAGING_PREFIX}{'e' * MODULE.STAGING_ID_HEX_LENGTH}-work",
        ),
    ],
)
def test_publication_contract_rejects_staging_id_or_name_substitution(
    tmp_path: Path, field: str, value: str
) -> None:
    command = _generation_command(
        _fake_prebuilt_generator(tmp_path), tmp_path / "candidate"
    )
    _guarded, parent, contract = _prepare_guarded_command(command)
    altered = replace(contract, **{field: value})
    try:
        with pytest.raises(
            MODULE.resource_guard.GuardError,
            match="child-to-publisher contract changed",
        ):
            altered.validate(parent)
    finally:
        parent.close()


def test_cleanup_never_path_chmods_a_swappable_entry(
    tmp_path: Path, monkeypatch
) -> None:
    outside = tmp_path / "outside"
    outside.mkdir(mode=0o755)
    outside.chmod(0o755)
    output_parent = tmp_path / "output"
    output_parent.mkdir()
    command = _generation_command(
        _fake_prebuilt_generator(tmp_path), output_parent / "candidate"
    )
    _guarded, parent, contract = _prepare_guarded_command(command)
    staging_id = contract.staging_id
    residue_name = f"{MODULE.STAGING_PREFIX}{staging_id}-work"
    residue = output_parent / residue_name
    residue.mkdir(mode=0o700)
    (residue / "partial").write_bytes(b"partial")
    displaced = output_parent / "attacker-displaced-residue"
    real_chmod = os.chmod
    path_chmod_called = False

    def swap_entry_before_chmod(
        path: str, mode: int, *, dir_fd: int | None = None
    ) -> None:
        nonlocal path_chmod_called
        path_chmod_called = True
        residue.rename(displaced)
        residue.symlink_to(outside, target_is_directory=True)
        real_chmod(path, mode, dir_fd=dir_fd)

    monkeypatch.setattr(MODULE.os, "chmod", swap_entry_before_chmod)
    try:
        assert MODULE._cleanup_staging(parent, staging_id) == 1
        assert not path_chmod_called
        assert stat.S_IMODE(outside.stat().st_mode) == 0o755
    finally:
        parent.close()


def test_runner_rejects_caller_supplied_staging_id(tmp_path: Path) -> None:
    executable = _fake_prebuilt_generator(tmp_path)
    arguments = _guarded_args(tmp_path, executable)
    arguments.extend(["--staging-id", "0" * MODULE.STAGING_ID_HEX_LENGTH])

    assert MODULE.main(arguments) == 1
    assert not (tmp_path / "resource-report").exists()


def test_runner_rejects_symlinked_or_writable_executable(tmp_path: Path) -> None:
    executable = _fake_prebuilt_generator(tmp_path)
    symlink_parent = tmp_path / "symlink-parent"
    symlink_parent.mkdir()
    symlink = symlink_parent / MODULE.BUNDLE_EXECUTABLE
    symlink.symlink_to(executable)

    assert MODULE.main(_guarded_args(tmp_path, symlink)) == 1
    assert not (tmp_path / "resource-report").exists()

    executable.chmod(0o722)
    assert MODULE.main(_guarded_args(tmp_path, executable)) == 1
    assert not (tmp_path / "resource-report").exists()


def test_runner_fails_if_executable_changes_during_run(
    tmp_path: Path, monkeypatch
) -> None:
    monkeypatch.setattr(
        MODULE.resource_guard, "HEAVY_JOB_LOCK_PATH", tmp_path / "heavy.lock"
    )
    monkeypatch.setattr(MODULE, "LOCK_PATH", tmp_path / "kagemusha.lock")
    executable = _fake_prebuilt_generator(tmp_path, "replace_original")

    assert MODULE.main(_guarded_args(tmp_path, executable)) == 1
    summary = json.loads(
        (tmp_path / "resource-report" / "kagemusha_resource_summary.json").read_text(
            encoding="utf-8"
        )
    )
    assert summary["exit_reason"] == "post_run_validation_error"
    assert summary["post_run_validation"] == "failed"
    assert summary["post_success_finalize"] == "skipped"
    assert not (tmp_path / "candidate").exists()
    assert (tmp_path / "admitted-copy-ran").read_text(encoding="ascii") == "yes"


def test_stale_journal_recovers_only_its_exact_staging_directory(
    tmp_path: Path,
) -> None:
    command = _generation_command(
        _fake_prebuilt_generator(tmp_path), tmp_path / "candidate"
    )
    _guarded, parent, contract = _prepare_guarded_command(command)
    staging_id = contract.staging_id
    try:
        MODULE._create_run_journal(parent, staging_id)
        residue = tmp_path / f"{MODULE.STAGING_PREFIX}{staging_id}-crash"
        residue.mkdir(mode=0o700)
        (residue / "partial").write_bytes(b"partial")

        assert MODULE._recover_stale_runs(parent) == 1
        assert not residue.exists()
        assert not list(tmp_path.glob(f"{MODULE.JOURNAL_PREFIX}*"))
    finally:
        parent.close()


def test_stale_journal_for_candidate_a_does_not_block_candidate_b(
    tmp_path: Path,
) -> None:
    executable = _fake_prebuilt_generator(tmp_path)
    first_command = _generation_command(executable, tmp_path / "candidate-a")
    second_command = _generation_command(executable, tmp_path / "candidate-b")
    _guarded, first_parent, first_contract = _prepare_guarded_command(
        first_command
    )
    first_id = first_contract.staging_id
    try:
        MODULE._create_run_journal(first_parent, first_id)
        residue = tmp_path / f"{MODULE.STAGING_PREFIX}{first_id}-crash"
        residue.mkdir(mode=0o700)
    finally:
        first_parent.close()

    _guarded, second_parent, _second_contract = _prepare_guarded_command(
        second_command
    )
    try:
        assert MODULE._recover_stale_runs(second_parent) == 1
        assert not residue.exists()
        assert not list(tmp_path.glob(f"{MODULE.JOURNAL_PREFIX}*"))
    finally:
        second_parent.close()


def test_recovery_rejects_unjournaled_or_tampered_residue(tmp_path: Path) -> None:
    command = _generation_command(
        _fake_prebuilt_generator(tmp_path), tmp_path / "candidate"
    )
    _guarded, parent, contract = _prepare_guarded_command(command)
    staging_id = contract.staging_id
    try:
        unjournaled = tmp_path / f"{MODULE.STAGING_PREFIX}{staging_id}-unknown"
        unjournaled.mkdir(mode=0o700)
        with pytest.raises(MODULE.resource_guard.GuardError, match="unjournaled"):
            MODULE._recover_stale_runs(parent)
        unjournaled.rmdir()

        MODULE._create_run_journal(parent, staging_id)
        marker = tmp_path / MODULE._journal_name(staging_id)
        marker.write_text("{}\n", encoding="utf-8")
        marker.chmod(0o600)
        with pytest.raises(MODULE.resource_guard.GuardError, match="output leaf"):
            MODULE._recover_stale_runs(parent)
        assert marker.exists()
    finally:
        parent.close()


def test_journal_write_failure_removes_partial_marker(
    tmp_path: Path, monkeypatch
) -> None:
    command = _generation_command(
        _fake_prebuilt_generator(tmp_path), tmp_path / "candidate"
    )
    _guarded, parent, contract = _prepare_guarded_command(command)
    staging_id = contract.staging_id

    def fail_after_partial_write(descriptor: int, payload: bytes) -> None:
        os.write(descriptor, payload[: max(1, len(payload) // 2)])
        raise MODULE.resource_guard.GuardError("injected journal write failure")

    monkeypatch.setattr(MODULE.resource_guard, "_write_all", fail_after_partial_write)
    try:
        with pytest.raises(
            MODULE.resource_guard.GuardError, match="injected journal write failure"
        ):
            MODULE._create_run_journal(parent, staging_id)
        assert not (tmp_path / MODULE._journal_name(staging_id)).exists()
    finally:
        parent.close()


def test_uncertain_visible_publication_retains_recovery_journal(
    tmp_path: Path,
) -> None:
    command = _generation_command(
        _fake_prebuilt_generator(tmp_path), tmp_path / "candidate"
    )
    _guarded, parent, contract = _prepare_guarded_command(command)
    staging_id = contract.staging_id
    staging: MODULE.PinnedStagingDirectory | None = None
    try:
        MODULE._create_run_journal(parent, staging_id)
        staging = MODULE._create_staging_directory(parent, staging_id)
        (tmp_path / "candidate").mkdir(mode=0o700)

        with pytest.raises(MODULE.resource_guard.GuardError, match="retained"):
            MODULE._cleanup_guarded_run(parent, staging_id)
        assert (tmp_path / MODULE._journal_name(staging_id)).is_file()
    finally:
        if staging is not None:
            staging.close()
        (tmp_path / "candidate").rmdir()
        if (tmp_path / MODULE._journal_name(staging_id)).exists():
            MODULE._remove_run_journal(parent, staging_id)
        parent.close()


def test_crash_recovery_is_scoped_to_the_same_output_parent(tmp_path: Path) -> None:
    first_root = tmp_path / "first"
    second_root = tmp_path / "second"
    first_root.mkdir()
    second_root.mkdir()
    executable = _fake_prebuilt_generator(tmp_path)
    first_command = _generation_command(executable, first_root / "candidate")
    second_command = _generation_command(executable, second_root / "candidate")
    _guarded, first_parent, first_contract = _prepare_guarded_command(
        first_command
    )
    staging_id = first_contract.staging_id
    _guarded, second_parent, _second_contract = _prepare_guarded_command(
        second_command
    )
    try:
        MODULE._create_run_journal(first_parent, staging_id)
        residue = first_root / f"{MODULE.STAGING_PREFIX}{staging_id}-crash"
        residue.mkdir(mode=0o700)

        assert MODULE._recover_stale_runs(second_parent) == 0
        assert residue.is_dir()
        assert MODULE._recover_stale_runs(first_parent) == 1
        assert not residue.exists()
    finally:
        first_parent.close()
        second_parent.close()


def test_pinned_output_parent_detects_path_replacement(tmp_path: Path) -> None:
    output_parent = tmp_path / "output-parent"
    output_parent.mkdir()
    executable = _fake_prebuilt_generator(tmp_path)
    command = _generation_command(executable, output_parent / "candidate")
    _guarded, parent, _contract = _prepare_guarded_command(command)
    moved = tmp_path / "moved-output-parent"
    try:
        output_parent.rename(moved)
        output_parent.mkdir()
        with pytest.raises(MODULE.resource_guard.GuardError, match="identity changed"):
            parent.validate()
        parent.validate(require_path=False)
    finally:
        parent.close()


def test_output_parent_requires_disk_backing_and_free_space(
    tmp_path: Path, monkeypatch
) -> None:
    executable = _fake_prebuilt_generator(tmp_path)
    command = _generation_command(executable, tmp_path / "candidate")

    monkeypatch.setattr(MODULE, "_filesystem_type", lambda _path: "tmpfs")
    with pytest.raises(MODULE.resource_guard.GuardError, match="disk-backed"):
        _prepare_guarded_command(command)
    assert not list(tmp_path.glob(f"{MODULE.STAGING_PREFIX}*"))
    assert not list(tmp_path.glob(f"{MODULE.JOURNAL_PREFIX}*"))

    monkeypatch.setattr(MODULE, "_filesystem_type", lambda _path: "ext4")
    actual_fstatvfs = os.fstatvfs

    class LowSpace:
        f_bavail = 1
        f_frsize = 4096

    monkeypatch.setattr(MODULE.os, "fstatvfs", lambda _descriptor: LowSpace())
    with pytest.raises(MODULE.resource_guard.GuardError, match="16 GiB"):
        _prepare_guarded_command(command)
    monkeypatch.setattr(MODULE.os, "fstatvfs", actual_fstatvfs)


def test_publisher_session_lifeline_prevents_orphaned_completion(
    tmp_path: Path,
) -> None:
    executable = tmp_path / MODULE.BUNDLE_EXECUTABLE
    marker = tmp_path / "publisher-completed"
    executable.write_text(
        "#!/bin/sh\n"
        "/bin/sleep 10\n"
        "printf published > \"$1\"\n",
        encoding="utf-8",
    )
    executable.chmod(0o700)
    snapshot = MODULE._snapshot_executable(str(executable), MODULE.BUNDLE_EXECUTABLE)
    command = _generation_command(executable, tmp_path / "candidate")
    _guarded, parent, contract = _prepare_guarded_command(command)
    staging_id = contract.staging_id
    MODULE._create_run_journal(parent, staging_id)
    MODULE._prepare_execution_copy(parent, snapshot, staging_id)
    session = MODULE._spawn_pinned_guarded_session(
        [snapshot.execution_path(), str(marker)],
        os.environ.copy(),
        (),
        (),
        snapshot,
    )
    try:
        os.close(session.lifeline_writer)
        session.lifeline_writer = -1
        assert session.wrapper.wait(timeout=6) != 0
        assert not marker.exists()
    finally:
        session.close()
        MODULE._release_execution_copy(parent, snapshot)
        MODULE._cleanup_guarded_run(parent, staging_id)
        parent.close()
        snapshot.close()
