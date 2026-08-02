#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${PRIVACY_SDK_GUARD_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
MODE="${1:-}"
PYTHON_BIN="${PRIVACY_SDK_GUARD_PYTHON_BIN:-python3}"

"${PYTHON_BIN}" - "${ROOT_DIR}" "${MODE}" <<'PY'
from __future__ import annotations

import ast
import hashlib
import re
import sys
from dataclasses import dataclass
from pathlib import Path

root = Path(sys.argv[1])
mode = sys.argv[2]

MATRIX_RELATIVE = "fixtures/privacy/exact12_v1.tsv"
MATRIX_BYTES = (root / MATRIX_RELATIVE).read_bytes()
MATRIX_TEXT = MATRIX_BYTES.decode("utf-8", errors="strict")


def matrix_rows(kind: str) -> tuple[tuple[str, ...], ...]:
    rows = []
    for line_number, line in enumerate(MATRIX_TEXT.splitlines(), 1):
        if not line or line.startswith("#"):
            continue
        fields = tuple(line.split("\t"))
        if fields[0] == kind:
            rows.append(fields)
        elif fields[0] not in {
            "matrix-version",
            "registry-sha256",
            "protocol",
            "typed-envelope",
            "retired",
        }:
            raise RuntimeError(f"unknown matrix row {line_number}: {fields[0]}")
    return tuple(rows)


PROTOCOL_ROWS = matrix_rows("protocol")
TYPED_ENVELOPE_ROWS = matrix_rows("typed-envelope")
EXPECTED_IDS = tuple(row[2] for row in PROTOCOL_ROWS)
RETIRED_IDS = tuple(row[1] for row in matrix_rows("retired"))

RETIRED_PUBLIC_SYMBOLS = (
    "privacyCapabilitiesV1",
    "privacyValidateCapabilitiesV1",
    "privacy_capabilities_v1",
    "privacy_validate_capabilities_v1",
    "PRIVACY_CAPABILITY_VALIDATION_STATUS_V1",
    "PRIVACY_NATIVE_ARCHIVE_MAX_BYTES",
    "privacyProofRequestV1",
    "privacyBuildProofV1",
    "privacyVerifyProofV1",
    "privacy_proof_request_v1",
    "privacy_build_proof_v1",
    "privacy_verify_proof_v1",
    "getPrivacyAlgorithmDescriptor",
    "getPrivacyAlgorithmDescriptors",
    "getPrivacyCapabilities",
    "getPrivacyCriteria",
    "buildPrivacyProofEnvelope",
    "iroha_privacy_proof_request_v1",
    "iroha_privacy_build_proof_v1",
    "iroha_privacy_verify_proof_v1",
    "PrivacyProofRequestV1",
    "PrivacyProofResultV1",
    "nativeProofRequest",
    "nativeBuildProof",
    "nativeVerifyProof",
    "buildZkAceTransferAuthorizationV1",
    "buildRegisterZkAceIdentityCommitmentInstruction",
    "buildRotateZkAceIdentityCommitmentInstruction",
    "buildRevokeZkAceIdentityCommitmentInstruction",
    "buildZkAceAuthorizationProofV1",
    "buildZkAceAuthorizedTransferInstruction",
    "ZkAceTransferAuthorizationV1Options",
    "ZkAceTransferAuthorizationV1",
    "RegisterZkAceIdentityCommitmentInstructionInput",
    "RotateZkAceIdentityCommitmentInstructionInput",
    "RevokeZkAceIdentityCommitmentInstructionInput",
    "ZkAcePublicInputsV1Input",
    "ZkAceAuthorizationProofV1Input",
    "ZkAceAuthorizationProofV1",
    "ZkAceWitnessV1Input",
    "ZkAceAuthorizedTransferInstructionInput",
)

DELETED_PYTHON_MODULES = (
    "anonymous_pgc.py",
    "jindo.py",
    "research_adapters.py",
    "silent_threshold.py",
    "sis_hints.py",
    "vega.py",
    "verange.py",
    "zk_ams.py",
    "zk_x509.py",
    "zkat.py",
)


class GuardFailure(RuntimeError):
    pass


def read(relative: str, overrides: dict[str, str]) -> str:
    if relative in overrides:
        return overrides[relative]
    return (root / relative).read_text(encoding="utf-8")


def require(condition: bool, message: str, errors: list[str]) -> None:
    if not condition:
        errors.append(message)


def cargo_feature_values(source: str, name: str) -> tuple[str, ...]:
    match = re.search(
        rf"(?ms)^{re.escape(name)}[ \t]*=[ \t]*\[(.*?)\][ \t]*$",
        source,
    )
    if match is None:
        raise GuardFailure(f"missing Cargo feature {name}")
    payload = match.group(1)
    values = tuple(re.findall(r'"([^"]+)"', payload))
    residue = re.sub(r'"[^"]+"|[,\s#A-Za-z0-9_./-]*', "", payload)
    if residue:
        raise GuardFailure(f"unsupported Cargo feature syntax for {name}")
    return values


def cargo_inline_dependency_features(
    source: str, name: str
) -> tuple[str, ...]:
    match = re.search(
        rf"(?m)^{re.escape(name)}[ \t]*=[ \t]*"
        r"\{[^\r\n]*features[ \t]*=[ \t]*\[([^\]]*)\][^\r\n]*\}[ \t]*$",
        source,
    )
    if match is None:
        raise GuardFailure(f"missing inline Cargo dependency features for {name}")
    return tuple(re.findall(r'"([^"]+)"', match.group(1)))


def check_exact12_feature_boundary(
    overrides: dict[str, str] | None = None,
) -> None:
    overrides = overrides or {}
    errors: list[str] = []
    rust_model = read("crates/iroha_data_model/src/privacy.rs", overrides)
    rust_proofs = read(
        "crates/iroha_data_model/src/privacy/proofs.rs", overrides
    )
    data_model_lib = read("crates/iroha_data_model/src/lib.rs", overrides)
    data_model_block = read(
        "crates/iroha_data_model/src/block/mod.rs", overrides
    )
    data_model_manifest = read(
        "crates/iroha_data_model/Cargo.toml", overrides
    )
    bridge_manifest = read(
        "crates/connect_norito_bridge/Cargo.toml", overrides
    )
    core_manifest = read("crates/iroha_core/Cargo.toml", overrides)

    exact12_conformance_features = cargo_feature_values(
        data_model_manifest, "privacy-exact12-conformance"
    )
    bridge_data_model_features = cargo_inline_dependency_features(
        bridge_manifest, "iroha_data_model"
    )
    core_release_features = cargo_feature_values(
        core_manifest, "privacy-release-evidence"
    )
    exact12_conformance_gate = (
        '#[cfg(any(test, feature = "privacy-exact12-conformance"))]'
    )
    require(
        not exact12_conformance_features,
        "iroha_data_model privacy-exact12-conformance must remain an empty, "
        "exact-surface feature without fixture or randomness edges",
        errors,
    )
    require(
        "privacy-exact12-conformance" in bridge_data_model_features
        and "test-fixtures" not in bridge_data_model_features,
        "connect_norito_bridge must request only the narrow exact-12 "
        "conformance surface, never iroha_data_model/test-fixtures",
        errors,
    )
    require(
        "iroha_data_model/privacy-exact12-conformance"
        in core_release_features
        and "iroha_data_model/test-fixtures" not in core_release_features,
        "privacy release evidence must use the narrow exact-12 conformance "
        "surface instead of general data-model fixtures",
        errors,
    )
    require(
        rust_model.count(exact12_conformance_gate) == 1
        and rust_proofs.count(exact12_conformance_gate) == 1
        and (
            exact12_conformance_gate
            + "\npub use exact12_fixture::{"
        )
        in rust_model
        and (
            exact12_conformance_gate
            + "\nmod exact12_fixture {"
        )
        in rust_proofs,
        "privacy-exact12-conformance must gate exactly the exact-12 exports "
        "and implementation module",
        errors,
    )
    require(
        "privacy-exact12-conformance" not in data_model_lib
        and "privacy-exact12-conformance" not in data_model_block
        and '#[cfg(any(test, feature = "test-fixtures"))]\npub mod testing;'
        in data_model_lib,
        "the exact-12 conformance feature must not expose general testing "
        "modules or block-tampering helpers",
        errors,
    )
    if errors:
        raise GuardFailure("\n".join(f"- {error}" for error in errors))


def literal_assignment(source: str, name: str):
    tree = ast.parse(source)
    for node in tree.body:
        if isinstance(node, (ast.Assign, ast.AnnAssign)):
            target = node.targets[0] if isinstance(node, ast.Assign) else node.target
            if isinstance(target, ast.Name) and target.id == name:
                return ast.literal_eval(node.value)
    raise GuardFailure(f"missing Python assignment {name}")


def js_protocol_ids(source: str) -> tuple[str, ...]:
    match = re.search(
        r"export const PRIVACY_PROTOCOL_IDS_V1 = Object\.freeze\(\[([\s\S]*?)\]\);",
        source,
    )
    if match is None:
        raise GuardFailure("missing JavaScript PRIVACY_PROTOCOL_IDS_V1")
    return tuple(re.findall(r'"([^"]+)"', match.group(1)))


def unique_expected_ids_in_source(source: str) -> tuple[str, ...]:
    matches = re.findall(
        r'"(' + "|".join(re.escape(value) for value in EXPECTED_IDS) + r')"',
        source,
    )
    return tuple(dict.fromkeys(matches))


@dataclass(frozen=True)
class WorkflowStep:
    """One constrained GitHub Actions step parsed without third-party YAML."""

    start_line: int
    end_line: int
    fields: dict[str, str | tuple[tuple[str, str], ...]]


@dataclass(frozen=True)
class WorkflowJob:
    """One top-level workflow job and its semantically parsed steps."""

    name: str
    start_line: int
    end_line: int
    steps: tuple[WorkflowStep, ...]


@dataclass(frozen=True)
class WorkflowDocument:
    """The constrained job/step surface used by the privacy workflow guard."""

    lines: tuple[str, ...]
    trigger_paths: dict[str, tuple[str, ...]]
    jobs: dict[str, WorkflowJob]


def _nested_step_mapping(
    lines: list[str],
    start: int,
    end: int,
    *,
    label: str,
) -> tuple[tuple[str, str], ...]:
    entries: list[tuple[str, str]] = []
    seen: set[str] = set()
    for line_number in range(start, end):
        line = lines[line_number]
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        match = re.fullmatch(
            r"          ([A-Za-z_][A-Za-z0-9_-]*):[ ]+(.+)", line
        )
        if match is None:
            raise GuardFailure(
                f"{label} contains unsupported nested workflow syntax on "
                f"line {line_number + 1}"
            )
        key, value = match.groups()
        if key in seen:
            raise GuardFailure(f"{label} contains duplicate key {key}")
        seen.add(key)
        entries.append((key, value.strip()))
    return tuple(entries)


def _block_scalar(
    lines: list[str], start: int, end: int, *, label: str
) -> str:
    payload: list[str] = []
    for line_number in range(start, end):
        line = lines[line_number]
        if line and not line.startswith("          "):
            raise GuardFailure(
                f"{label} block scalar has invalid indentation on "
                f"line {line_number + 1}"
            )
        payload.append(line[10:] if line else "")
    return "\n".join(payload)


def _parse_workflow_step(
    lines: list[str], start: int, end: int
) -> WorkflowStep:
    first = lines[start]
    if not first.startswith("      -"):
        raise GuardFailure(f"invalid workflow step on line {start + 1}")
    first_payload = first[7:]
    if first_payload.startswith(" "):
        first_payload = first_payload[1:]

    field_rows: list[tuple[int, str]] = []
    if first_payload:
        field_rows.append((start, first_payload))
    for line_number in range(start + 1, end):
        match = re.fullmatch(
            r"        ([A-Za-z_][A-Za-z0-9_-]*):(.*)", lines[line_number]
        )
        if match is not None:
            field_rows.append(
                (line_number, f"{match.group(1)}:{match.group(2)}")
            )
    if not field_rows:
        raise GuardFailure(f"empty workflow step on line {start + 1}")

    fields: dict[str, str | tuple[tuple[str, str], ...]] = {}
    for row_index, (line_number, row) in enumerate(field_rows):
        key, separator, raw_value = row.partition(":")
        if not separator or not re.fullmatch(
            r"[A-Za-z_][A-Za-z0-9_-]*", key
        ):
            raise GuardFailure(
                f"invalid workflow step field on line {line_number + 1}"
            )
        if key in fields:
            raise GuardFailure(
                f"workflow step on line {start + 1} contains duplicate {key}"
            )
        value = raw_value.strip()
        payload_start = line_number + 1
        payload_end = (
            field_rows[row_index + 1][0]
            if row_index + 1 < len(field_rows)
            else end
        )
        if key == "run" and value == "|":
            fields[key] = _block_scalar(
                lines,
                payload_start,
                payload_end,
                label=f"workflow run on line {line_number + 1}",
            )
        elif key in {"env", "with"}:
            if value:
                raise GuardFailure(
                    f"workflow {key} on line {line_number + 1} must be a mapping"
                )
            fields[key] = _nested_step_mapping(
                lines,
                payload_start,
                payload_end,
                label=f"workflow {key} on line {line_number + 1}",
            )
        else:
            for nested_line_number in range(payload_start, payload_end):
                nested = lines[nested_line_number]
                if nested.strip() and not nested.lstrip().startswith("#"):
                    raise GuardFailure(
                        f"workflow scalar {key} has unexpected nested content "
                        f"on line {nested_line_number + 1}"
                    )
            if not value:
                raise GuardFailure(
                    f"workflow scalar {key} is empty on line {line_number + 1}"
                )
            fields[key] = value
    return WorkflowStep(start_line=start, end_line=end, fields=fields)


def _parse_workflow_trigger_paths(
    lines: list[str],
) -> dict[str, tuple[str, ...]]:
    on_rows = [index for index, line in enumerate(lines) if line == "on:"]
    if len(on_rows) != 1:
        raise GuardFailure("privacy workflow must contain exactly one on mapping")
    on_start = on_rows[0] + 1
    on_end = len(lines)
    for line_number in range(on_start, len(lines)):
        if lines[line_number] and not lines[line_number].startswith((" ", "#")):
            on_end = line_number
            break

    event_rows: list[tuple[int, str]] = []
    for line_number in range(on_start, on_end):
        match = re.fullmatch(
            r"  ([A-Za-z_][A-Za-z0-9_-]*):(.*)", lines[line_number]
        )
        if match is not None:
            event_rows.append((line_number, match.group(1)))
    if not event_rows:
        raise GuardFailure("privacy workflow on mapping is empty")
    if len({event for _, event in event_rows}) != len(event_rows):
        raise GuardFailure("privacy workflow contains duplicate trigger events")

    trigger_paths: dict[str, tuple[str, ...]] = {}
    for event_index, (event_start, event) in enumerate(event_rows):
        event_end = (
            event_rows[event_index + 1][0]
            if event_index + 1 < len(event_rows)
            else on_end
        )
        path_rows = [
            line_number
            for line_number in range(event_start + 1, event_end)
            if lines[line_number] == "    paths:"
        ]
        if len(path_rows) > 1:
            raise GuardFailure(
                f"privacy workflow trigger {event} contains duplicate paths mappings"
            )
        if not path_rows:
            trigger_paths[event] = ()
            continue

        paths_start = path_rows[0] + 1
        paths_end = event_end
        for line_number in range(paths_start, event_end):
            if re.fullmatch(
                r"    [A-Za-z_][A-Za-z0-9_-]*:.*", lines[line_number]
            ):
                paths_end = line_number
                break
        paths: list[str] = []
        for line_number in range(paths_start, paths_end):
            line = lines[line_number]
            if not line.strip() or line.lstrip().startswith("#"):
                continue
            match = re.fullmatch(r'      - "([^"]+)"', line)
            if match is None:
                raise GuardFailure(
                    f"privacy workflow trigger {event} paths contains "
                    f"unsupported syntax on line {line_number + 1}"
                )
            paths.append(match.group(1))
        if len(paths) != len(set(paths)):
            raise GuardFailure(
                f"privacy workflow trigger {event} contains duplicate paths"
            )
        trigger_paths[event] = tuple(paths)
    return trigger_paths


def parse_workflow(source: str) -> WorkflowDocument:
    """Parse the exact top-level jobs and step mappings used by this workflow."""

    if "\t" in source:
        raise GuardFailure("privacy workflow must not contain tab indentation")
    lines = source.splitlines()
    jobs_rows = [index for index, line in enumerate(lines) if line == "jobs:"]
    if len(jobs_rows) != 1:
        raise GuardFailure("privacy workflow must contain exactly one jobs mapping")
    jobs_start = jobs_rows[0] + 1
    jobs_end = len(lines)
    for line_number in range(jobs_start, len(lines)):
        if lines[line_number] and not lines[line_number].startswith((" ", "#")):
            jobs_end = line_number
            break

    job_rows: list[tuple[int, str]] = []
    for line_number in range(jobs_start, jobs_end):
        match = re.fullmatch(r"  ([A-Za-z0-9_-]+):", lines[line_number])
        if match is not None:
            job_rows.append((line_number, match.group(1)))
    if not job_rows:
        raise GuardFailure("privacy workflow jobs mapping is empty")
    if len({name for _, name in job_rows}) != len(job_rows):
        raise GuardFailure("privacy workflow contains duplicate job names")

    jobs: dict[str, WorkflowJob] = {}
    for job_index, (job_start, job_name) in enumerate(job_rows):
        job_end = (
            job_rows[job_index + 1][0]
            if job_index + 1 < len(job_rows)
            else jobs_end
        )
        steps_rows = [
            line_number
            for line_number in range(job_start + 1, job_end)
            if lines[line_number] == "    steps:"
        ]
        if len(steps_rows) != 1:
            raise GuardFailure(
                f"workflow job {job_name} must contain exactly one steps list"
            )
        steps_start = steps_rows[0] + 1
        step_rows = [
            line_number
            for line_number in range(steps_start, job_end)
            if lines[line_number].startswith("      -")
        ]
        if not step_rows:
            raise GuardFailure(f"workflow job {job_name} has no steps")
        steps = tuple(
            _parse_workflow_step(
                lines,
                step_start,
                (
                    step_rows[step_index + 1]
                    if step_index + 1 < len(step_rows)
                    else job_end
                ),
            )
            for step_index, step_start in enumerate(step_rows)
        )
        jobs[job_name] = WorkflowJob(
            name=job_name,
            start_line=job_start,
            end_line=job_end,
            steps=steps,
        )
    return WorkflowDocument(
        lines=tuple(lines),
        trigger_paths=_parse_workflow_trigger_paths(lines),
        jobs=jobs,
    )


def _steps_with_field(
    job: WorkflowJob, key: str, value: str
) -> tuple[tuple[int, WorkflowStep], ...]:
    return tuple(
        (index, step)
        for index, step in enumerate(job.steps)
        if step.fields.get(key) == value
    )


def _exact_step(
    step: WorkflowStep,
    expected: dict[str, str | tuple[tuple[str, str], ...]],
) -> bool:
    return step.fields == expected


def _workflow_run_has_cargo_policy(run: str) -> bool:
    executable_lines = "\n".join(
        line for line in run.splitlines() if not line.lstrip().startswith("#")
    )
    return (
        "ci/privacy_sdk_cargo_lockfile.sh" in executable_lines
        or "ci/privacy_sdk_cargo_wrapper.sh" in executable_lines
        or re.search(
            r"(?<![A-Za-z0-9_-])"
            r"(?:[A-Za-z0-9_${}\"./~-]+/)?cargo(?=\s|$)",
            executable_lines,
        )
        is not None
        or re.search(
            r'(?<!\S)"\$\{HOME\}/\.cargo/bin/rustup"'
            r"\s+toolchain\s+install(?=\s|$)",
            executable_lines,
        )
        is not None
    )


NEGATIVE_CONTROL_STEP_NAME = "Privacy SDK guard negative controls"
NEGATIVE_CONTROL_COMMAND_PREFIX = "ci/check_privacy_sdk_guard.sh "
NEGATIVE_CONTROL_COUNT = 232
WORKFLOW_META_NEGATIVE_CONTROLS = frozenset(
    {
        "--negative-control-negative-controls-workflow",
        "--negative-control-negative-controls-comment-workflow",
        "--negative-control-negative-controls-order-workflow",
        "--negative-control-negative-controls-inventory-parity",
    }
)
EXACT12_FEATURE_NEGATIVE_CONTROLS = frozenset(
    {
        "--negative-control-exact12-bridge-test-fixtures",
        "--negative-control-exact12-conformance-rand-edge",
        "--negative-control-exact12-release-evidence-test-fixtures",
    }
)


def _negative_control_modes(step: WorkflowStep) -> tuple[str, ...]:
    run = step.fields.get("run")
    if not isinstance(run, str):
        raise GuardFailure("privacy SDK negative controls step must have one run block")
    modes: list[str] = []
    command_pattern = re.compile(
        re.escape(NEGATIVE_CONTROL_COMMAND_PREFIX)
        + r"(--negative-control-[a-z0-9]+(?:-[a-z0-9]+)*)"
    )
    for line_number, line in enumerate(run.splitlines(), 1):
        match = command_pattern.fullmatch(line)
        if match is None:
            raise GuardFailure(
                "privacy SDK negative controls run block contains a blank, "
                f"commented, or malformed command at entry {line_number}"
            )
        modes.append(match.group(1))
    return tuple(modes)


def _check_cargo_workflow(
    workflow_source: str, errors: list[str]
) -> WorkflowDocument:
    workflow = parse_workflow(workflow_source)
    checkout_action = (
        "actions/checkout@11d5960a326750d5838078e36cf38b85af677262"
    )
    setup_python_action = (
        "actions/setup-python@a26af69be951a213d495a4c3e4e4022e16d87065"
    )
    setup_python_path = "${{ steps.privacy-python.outputs.python-path }}"
    install_run = "\n".join(
        (
            "env -i \\",
            '  HOME="${HOME}" \\',
            '  RUSTUP_DIST_SERVER="https://static.rust-lang.org" \\',
            '  "${HOME}/.cargo/bin/rustup" toolchain install \\',
            '    "1.93.1-x86_64-unknown-linux-gnu" \\',
            "    --profile minimal \\",
            "    --no-self-update",
        )
    )
    native_provision_run = "\n".join(
        (
            "env -u PKG_CONFIG_PATH -u LD_LIBRARY_PATH \\",
            "  ci/privacy_sdk_cargo_lockfile.sh provision-ci \\",
            '  "${GITHUB_WORKSPACE}" \\',
            '  "${RUNNER_TEMP}/iroha-privacy-sdk-cargo" \\',
            '  "${GITHUB_ENV}" \\',
            '  "${GITHUB_PATH}" \\',
            '  "${HOME}/.cargo/bin/rustup" \\',
            '  "1.93.1-x86_64-unknown-linux-gnu"',
        )
    )
    python_provision_run = native_provision_run + (
        ' \\\n  "${{ steps.privacy-python.outputs.python-path }}"'
    )
    native_verify_run = (
        'ci/privacy_sdk_cargo_lockfile.sh verify-ci "${GITHUB_WORKSPACE}"'
    )
    python_verify_run = (
        native_verify_run
        + ' "${{ steps.privacy-python.outputs.python-path }}"'
    )
    checkout_step = {"uses": checkout_action}
    install_step = {
        "name": "Install host-qualified privacy SDK Rust toolchain",
        "shell": "bash",
        "run": install_run,
    }
    setup_python_step = {
        "id": "privacy-python",
        "uses": setup_python_action,
        "with": (
            ("python-version", '"3.12"'),
            ("update-environment", "false"),
        ),
    }
    mobile_python_binding_step = {
        "name": "Bind the canonical mobile Python",
        "env": (("SETUP_PYTHON_PATH", setup_python_path),),
        "run": "\n".join(
            (
                "mobile_python=\"$(\"$SETUP_PYTHON_PATH\" -I -S -c "
                "'import pathlib,sys; "
                "print(pathlib.Path(sys.executable).resolve(strict=True))')\"",
                "[[ \"$(\"$mobile_python\" -I -S -c "
                "'import sys; "
                "print(f\"{sys.version_info.major}.{sys.version_info.minor}\")')\" "
                '== "3.12" ]]',
                'echo "MOBILE_SDK_PYTHON_BINARY=$mobile_python" >> "$GITHUB_ENV"',
            )
        ),
    }
    native_provision_step = {
        "name": "Provision private privacy SDK Cargo lock",
        "shell": "bash",
        "run": native_provision_run,
    }
    python_provision_step = {
        "name": "Provision private privacy SDK Cargo lock",
        "shell": "bash",
        "run": python_provision_run,
    }
    native_initial_verify_step = {
        "name": "Verify privacy SDK Cargo lock isolation",
        "run": native_verify_run,
    }
    python_initial_verify_step = {
        "name": "Verify privacy SDK Cargo lock isolation",
        "run": python_verify_run,
    }
    native_fetch_step = {
        "name": "Prime privacy native Cargo dependencies",
        "env": (("CARGO_NET_OFFLINE", '"false"'),),
        "run": "cargo fetch --locked",
    }
    python_fetch_step = {
        "name": "Prime privacy Python SDK Cargo dependencies",
        "env": (("CARGO_NET_OFFLINE", '"false"'),),
        "run": "cargo fetch --locked",
    }
    native_consumer_step = {
        "name": "Privacy native bridge tests",
        "run": (
            "cargo test -p connect_norito_bridge privacy_ --lib "
            "-- --test-threads=1"
        ),
    }
    python_consumer_step = {
        "name": "Privacy Python SDK tests",
        "run": (
            "env -u PKG_CONFIG_PATH -u LD_LIBRARY_PATH "
            "ci/check_privacy_python_sdk.sh"
        ),
    }
    guard_self_test_step = {
        "name": "Privacy SDK authenticated Cargo lock self-test",
        "run": (
            "env -u PKG_CONFIG_PATH -u LD_LIBRARY_PATH "
            "PRIVACY_SDK_LOCK_TEST_PYTHON_BIN="
            '"${{ steps.privacy-python.outputs.python-path }}" '
            "bash ci/privacy_sdk_cargo_lockfile_test.sh"
        ),
    }
    guard_consumer_step = {
        "name": "Privacy SDK parity and fail-closed guard",
        "run": (
            "env -u PKG_CONFIG_PATH -u LD_LIBRARY_PATH "
            "ci/check_privacy_sdk_guard.sh"
        ),
    }
    native_final_verify_step = {
        "name": "Verify final privacy SDK Cargo lock isolation",
        "if": "always()",
        "run": native_verify_run,
    }
    python_final_verify_step = {
        "name": "Verify final privacy SDK Cargo lock isolation",
        "if": "always()",
        "run": python_verify_run,
    }
    policies = {
        "privacy_native_bridge_tests": {
            "python": False,
            "provision": native_provision_step,
            "initial_verify": native_initial_verify_step,
            "fetch": native_fetch_step,
            "consumer": native_consumer_step,
            "final_verify": native_final_verify_step,
        },
        "privacy_python_sdk_tests": {
            "python": True,
            "provision": python_provision_step,
            "initial_verify": python_initial_verify_step,
            "fetch": python_fetch_step,
            "consumer": python_consumer_step,
            "final_verify": python_final_verify_step,
        },
        "privacy-sdk-guard": {
            "python": True,
            "provision": python_provision_step,
            "initial_verify": python_initial_verify_step,
            "fetch": python_fetch_step,
            "consumer": guard_consumer_step,
            "self_test": guard_self_test_step,
            "final_verify": python_final_verify_step,
        },
    }

    require(
        workflow_source.endswith("\n") and "\r" not in workflow_source,
        "privacy SDK workflow must use canonical LF text ending in LF",
        errors,
    )

    all_steps = tuple(
        (job_name, step_index, step)
        for job_name, job in workflow.jobs.items()
        for step_index, step in enumerate(job.steps)
    )
    setup_python_steps = tuple(
        (job_name, step_index, step)
        for job_name, step_index, step in all_steps
        if isinstance(step.fields.get("uses"), str)
        and step.fields["uses"].startswith("actions/setup-python@")
    )
    require(
        len(setup_python_steps) == 2
        and {job_name for job_name, _, _ in setup_python_steps}
        == {"privacy_python_sdk_tests", "privacy-sdk-guard"}
        and all(
            _exact_step(step, setup_python_step)
            for _, _, step in setup_python_steps
        ),
        "both Python Cargo jobs must use only the exact pinned setup-python "
        "3.12 step with update-environment false and no cache fields",
        errors,
    )
    uses_values = tuple(
        step.fields["uses"]
        for _, _, step in all_steps
        if isinstance(step.fields.get("uses"), str)
    )
    require(
        not any(value.startswith("Swatinem/rust-cache@") for value in uses_values),
        "privacy workflow must not use rust-cache",
        errors,
    )
    require(
        not any(
            value.startswith("actions-rust-lang/setup-rust-toolchain")
            or value.startswith("dtolnay/rust-toolchain")
            for value in uses_values
        ),
        "privacy Cargo jobs must use only the explicitly authenticated rustup installer",
        errors,
    )

    expected_cargo_step_indices: dict[str, set[int]] = {}
    for job_name, policy in policies.items():
        job = workflow.jobs.get(job_name)
        require(
            job is not None,
            f"privacy workflow is missing required Cargo job {job_name}",
            errors,
        )
        if job is None:
            continue

        ordered_indices: list[int] = []
        policy_indices: set[int] = set()
        checkout_matches = tuple(
            (index, step)
            for index, step in enumerate(job.steps)
            if isinstance(step.fields.get("uses"), str)
            and step.fields["uses"].startswith("actions/checkout@")
        )
        require(
            len(checkout_matches) == 1
            and _exact_step(checkout_matches[0][1], checkout_step),
            f"{job_name} must contain exactly one exact pinned checkout step",
            errors,
        )
        if len(checkout_matches) == 1:
            ordered_indices.append(checkout_matches[0][0])

        required_steps = [
            ("toolchain install", install_step),
        ]
        if policy["python"]:
            setup_matches = tuple(
                (index, step)
                for index, step in enumerate(job.steps)
                if isinstance(step.fields.get("uses"), str)
                and step.fields["uses"].startswith("actions/setup-python@")
            )
            require(
                len(setup_matches) == 1
                and _exact_step(setup_matches[0][1], setup_python_step),
                f"{job_name} must contain exactly one exact setup-python step",
                errors,
            )
        else:
            setup_matches = ()
        if job_name == "privacy-sdk-guard":
            required_steps.append(
                ("canonical mobile Python binding", mobile_python_binding_step)
            )
        required_steps.extend(
            (
                ("private Cargo provision", policy["provision"]),
                ("initial Cargo isolation verification", policy["initial_verify"]),
                ("locked Cargo fetch", policy["fetch"]),
            )
        )
        if job_name == "privacy-sdk-guard":
            required_steps.append(
                ("authenticated Cargo lock self-test", policy["self_test"])
            )

        for label, expected_step in required_steps:
            expected_name = expected_step["name"]
            matches = _steps_with_field(job, "name", expected_name)
            require(
                len(matches) == 1
                and _exact_step(matches[0][1], expected_step),
                f"{job_name} must contain exactly one exact {label} step",
                errors,
            )
            if len(matches) == 1:
                ordered_indices.append(matches[0][0])
                policy_indices.add(matches[0][0])

        if policy["python"] and len(setup_matches) == 1:
            ordered_indices.insert(2, setup_matches[0][0])
            policy_indices.add(setup_matches[0][0])

        if job_name == "privacy-sdk-guard":
            negative_matches = _steps_with_field(
                job, "name", NEGATIVE_CONTROL_STEP_NAME
            )
            require(
                len(negative_matches) == 1,
                "privacy-sdk-guard must contain exactly one negative-controls step",
                errors,
            )
            if len(negative_matches) == 1:
                negative_index, negative_step = negative_matches[0]
                require(
                    set(negative_step.fields)
                    == {"name", "run"},
                    "privacy SDK negative-controls step may contain only name and run",
                    errors,
                )
                try:
                    negative_modes = _negative_control_modes(negative_step)
                except GuardFailure as error:
                    errors.append(str(error))
                    negative_modes = ()
                require(
                    len(negative_modes) == NEGATIVE_CONTROL_COUNT,
                    "privacy SDK negative-controls step must contain exactly "
                    f"{NEGATIVE_CONTROL_COUNT} commands",
                    errors,
                )
                require(
                    len(set(negative_modes)) == len(negative_modes),
                    "privacy SDK negative-controls commands must be unique",
                    errors,
                )
                require(
                    WORKFLOW_META_NEGATIVE_CONTROLS.issubset(negative_modes),
                    "privacy SDK negative-controls inventory must include all "
                    "four workflow meta-negative controls",
                    errors,
                )
                ordered_indices.append(negative_index)

        consumer_step = policy["consumer"]
        consumer_matches = _steps_with_field(
            job, "name", consumer_step["name"]
        )
        require(
            len(consumer_matches) == 1
            and _exact_step(consumer_matches[0][1], consumer_step),
            f"{job_name} must contain exactly one exact consumer step",
            errors,
        )
        if len(consumer_matches) == 1:
            ordered_indices.append(consumer_matches[0][0])
            policy_indices.add(consumer_matches[0][0])

        final_step = policy["final_verify"]
        final_matches = _steps_with_field(job, "name", final_step["name"])
        require(
            len(final_matches) == 1
            and _exact_step(final_matches[0][1], final_step),
            f"{job_name} must contain exactly one exact final always() "
            "Cargo isolation verification step",
            errors,
        )
        if len(final_matches) == 1:
            ordered_indices.append(final_matches[0][0])
            policy_indices.add(final_matches[0][0])

        expected_order_length = 11 if job_name == "privacy-sdk-guard" else (
            8 if policy["python"] else 7
        )
        require(
            len(ordered_indices) == expected_order_length
            and all(
                earlier < later
                for earlier, later in zip(
                    ordered_indices, ordered_indices[1:]
                )
            ),
            f"{job_name} must keep checkout, toolchain, setup, mobile Python "
            "binding, provision, verification, fetch, tests, negative controls, "
            "consumer, and final verification in canonical order",
            errors,
        )
        policy_indices.update(
            index for index in ordered_indices if index >= 0
        )
        expected_cargo_step_indices[job_name] = policy_indices

    semantic_python_path_run_count = sum(
        step.fields["run"].count(setup_python_path)
        for _, _, step in all_steps
        if isinstance(step.fields.get("run"), str)
    )
    semantic_python_path_env_count = sum(
        key.count(setup_python_path) + value.count(setup_python_path)
        for _, _, step in all_steps
        for key, value in (
            step.fields["env"]
            if isinstance(step.fields.get("env"), tuple)
            else ()
        )
    )
    semantic_python_path_count = sum(
        value.count(setup_python_path)
        if isinstance(value, str)
        else sum(
            key.count(setup_python_path) + item.count(setup_python_path)
            for key, item in value
        )
        for _, _, step in all_steps
        for value in step.fields.values()
    )
    require(
        semantic_python_path_run_count == 7
        and semantic_python_path_env_count == 1
        and semantic_python_path_count == 8,
        "setup-python output must be threaded into every Python provision, "
        "verification, self-test, and the canonical mobile Python binding",
        errors,
    )

    rogue_cargo_jobs: set[str] = set()
    unexpected_cargo_steps: list[str] = []
    for job_name, step_index, step in all_steps:
        run = step.fields.get("run")
        if not isinstance(run, str) or not _workflow_run_has_cargo_policy(run):
            continue
        if job_name not in policies:
            rogue_cargo_jobs.add(job_name)
        elif step_index not in expected_cargo_step_indices.get(job_name, set()):
            unexpected_cargo_steps.append(f"{job_name}[{step_index}]")
    require(
        not rogue_cargo_jobs,
        "Cargo policy commands may appear only in the three authenticated "
        "privacy Cargo jobs; rogue jobs: "
        + ", ".join(sorted(rogue_cargo_jobs)),
        errors,
    )
    require(
        not unexpected_cargo_steps,
        "privacy Cargo jobs contain unexpected Cargo policy steps: "
        + ", ".join(unexpected_cargo_steps),
        errors,
    )
    return workflow


def _check_workflow_trigger_paths(
    workflow: WorkflowDocument,
    required_paths: tuple[str, ...],
    errors: list[str],
) -> None:
    guarded_events = tuple(
        event
        for event in ("pull_request", "push")
        if event in workflow.trigger_paths
    )
    require(
        "pull_request" in guarded_events
        and bool(workflow.trigger_paths["pull_request"]),
        "privacy workflow must define a non-empty pull_request paths inventory",
        errors,
    )
    for required_path in required_paths:
        require(
            bool(guarded_events)
            and all(
                required_path in workflow.trigger_paths[event]
                for event in guarded_events
            ),
            "privacy workflow pull_request/push paths must include "
            f"{required_path}",
            errors,
        )


def _mutate_workflow_meta_negative_control(source: str, mode: str) -> str:
    if mode not in WORKFLOW_META_NEGATIVE_CONTROLS:
        raise GuardFailure(f"unsupported workflow meta-negative control: {mode}")
    workflow = parse_workflow(source)
    guard_job = workflow.jobs.get("privacy-sdk-guard")
    if guard_job is None:
        raise GuardFailure("workflow meta-negative control cannot find guard job")
    negative_matches = _steps_with_field(
        guard_job, "name", NEGATIVE_CONTROL_STEP_NAME
    )
    if len(negative_matches) != 1:
        raise GuardFailure(
            "workflow meta-negative control requires one negative-controls step"
        )
    _, negative_step = negative_matches[0]
    lines = source.splitlines(keepends=True)

    if mode == "--negative-control-negative-controls-workflow":
        del lines[negative_step.start_line : negative_step.end_line]
    elif mode == "--negative-control-negative-controls-comment-workflow":
        command = "          " + NEGATIVE_CONTROL_COMMAND_PREFIX + mode
        matches = tuple(
            line_number
            for line_number in range(
                negative_step.start_line, negative_step.end_line
            )
            if lines[line_number].rstrip("\r\n") == command
        )
        if len(matches) != 1:
            raise GuardFailure(
                "workflow meta-negative control cannot find comment target"
            )
        line_number = matches[0]
        lines[line_number] = lines[line_number].replace(
            "          ci/", "          # ci/", 1
        )
    elif mode == "--negative-control-negative-controls-order-workflow":
        consumer_matches = _steps_with_field(
            guard_job,
            "name",
            "Privacy SDK parity and fail-closed guard",
        )
        if len(consumer_matches) != 1:
            raise GuardFailure(
                "workflow meta-negative control cannot find guard consumer"
            )
        _, consumer_step = consumer_matches[0]
        if negative_step.end_line != consumer_step.start_line:
            raise GuardFailure(
                "workflow meta-negative control requires adjacent negative "
                "controls and guard consumer steps"
            )
        block = lines[negative_step.start_line : negative_step.end_line]
        del lines[negative_step.start_line : negative_step.end_line]
        adjusted_consumer_end = consumer_step.end_line - len(block)
        lines[adjusted_consumer_end:adjusted_consumer_end] = block
    else:
        command = "          " + NEGATIVE_CONTROL_COMMAND_PREFIX + mode
        matches = tuple(
            line_number
            for line_number in range(
                negative_step.start_line, negative_step.end_line
            )
            if lines[line_number].rstrip("\r\n") == command
        )
        if len(matches) != 1:
            raise GuardFailure(
                "workflow meta-negative control cannot find inventory target"
            )
        line_number = matches[0]
        lines.insert(line_number + 1, lines[line_number])

    mutated = "".join(lines)
    if mutated == source:
        raise GuardFailure(f"workflow meta-negative control made no change: {mode}")
    return mutated


def check(overrides: dict[str, str] | None = None) -> None:
    overrides = overrides or {}
    errors: list[str] = []
    check_exact12_feature_boundary(overrides)

    version_rows = matrix_rows("matrix-version")
    registry_rows = matrix_rows("registry-sha256")
    require(
        MATRIX_TEXT.endswith("\n")
        and "\r" not in MATRIX_TEXT
        and all(MATRIX_TEXT.split("\n")[:-1]),
        "exact12 matrix must use non-empty canonical LF lines and end with LF",
        errors,
    )
    require(
        version_rows == (("matrix-version", "1"),),
        "exact12 matrix must declare only version 1",
        errors,
    )
    require(
        len(PROTOCOL_ROWS) == 12
        and all(
            len(row) == 5 and row[1] == str(index)
            for index, row in enumerate(PROTOCOL_ROWS)
        )
        and len(set(EXPECTED_IDS)) == 12,
        "exact12 matrix must contain exactly 12 unique indexed protocol routes",
        errors,
    )
    registry_preimage = "".join(f"{protocol_id}\n" for protocol_id in EXPECTED_IDS)
    registry_digest = hashlib.sha256(registry_preimage.encode("utf-8")).hexdigest()
    require(
        registry_rows == (("registry-sha256", registry_digest),),
        "exact12 matrix registry digest does not bind its ordered protocol rows",
        errors,
    )
    require(
        len(TYPED_ENVELOPE_ROWS) == 12
        and all(len(row) == 6 for row in TYPED_ENVELOPE_ROWS)
        and tuple(row[1:4] for row in TYPED_ENVELOPE_ROWS)
        == tuple(row[2:5] for row in PROTOCOL_ROWS)
        and all(
            re.fullmatch(r"[0-9a-f]{64}", digest) is not None
            and digest != "0" * 64
            for row in TYPED_ENVELOPE_ROWS
            for digest in row[4:]
        ),
        "exact12 matrix must bind non-zero typed envelopes for all 12 canonical routes",
        errors,
    )
    require(
        len(RETIRED_IDS) == len(set(RETIRED_IDS))
        and all(protocol_id not in EXPECTED_IDS for protocol_id in RETIRED_IDS),
        "exact12 matrix retired IDs must be unique and outside the registry",
        errors,
    )

    js_source = read("javascript/iroha_js/src/privacyCapabilities.js", overrides)
    py_catalog = read(
        "python/iroha_python/src/iroha_python/privacy_catalog.py", overrides
    )
    rust_model = read("crates/iroha_data_model/src/privacy.rs", overrides)
    rust_protocol = read(
        "crates/iroha_data_model/src/privacy/protocol.rs", overrides
    )
    js_crypto = read("javascript/iroha_js/src/crypto.js", overrides)
    py_crypto = read(
        "python/iroha_python/src/iroha_python/crypto.py", overrides
    )
    py_native = read(
        "python/iroha_python/iroha_python_rs/src/lib.rs", overrides
    )

    for relative, source, markers in (
        (
            "crates/iroha_data_model/src/privacy.rs",
            rust_model,
            (
                "TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1: u32 = 9 * 1024 * 1024",
                "TAIRA_PRIVACY_MAX_ACTION_BYTES_V1: u32 = 9 * 1024 * 1024",
                "TAIRA_PRIVACY_MAX_BYTES_PER_TRANSACTION_V1: u32 = 9 * 1024 * 1024",
                "TAIRA_PRIVACY_MAX_BYTES_PER_BLOCK_V1: u32 = 18 * 1024 * 1024",
            ),
        ),
        (
            "javascript/iroha_js/src/privacyCapabilities.js",
            js_source,
            (
                "max_proof_bytes_per_action: 9 * 1024 * 1024",
                "max_action_bytes: 9 * 1024 * 1024",
                "max_privacy_bytes_per_transaction: 9 * 1024 * 1024",
                "max_privacy_bytes_per_block: 18 * 1024 * 1024",
            ),
        ),
        (
            "python/iroha_python/src/iroha_python/privacy_catalog.py",
            py_catalog,
            (
                '"max_proof_bytes_per_action": 9 * 1024 * 1024',
                '"max_action_bytes": 9 * 1024 * 1024',
                '"max_privacy_bytes_per_transaction": 9 * 1024 * 1024',
                '"max_privacy_bytes_per_block": 18 * 1024 * 1024',
            ),
        ),
    ):
        require(
            all(marker in source for marker in markers),
            f"{relative} must pin the first-release 9 MiB action/transaction and 18 MiB block privacy ceilings",
            errors,
        )

    require(
        re.search(
            r"PRIVACY_REQUIRED_BRIDGE_ABI_VERSION\s*=\s*21\s*;",
            js_crypto,
        )
        is not None
        and "abiVersion === PRIVACY_REQUIRED_BRIDGE_ABI_VERSION" in js_crypto
        and "abiVersion >= PRIVACY_REQUIRED_BRIDGE_ABI_VERSION" not in js_crypto,
        "JavaScript privacy bridge must require exact first-release ABI 21",
        errors,
    )
    require(
        literal_assignment(py_crypto, "PRIVACY_REQUIRED_BRIDGE_ABI_VERSION") == 21
        and "version == PRIVACY_REQUIRED_BRIDGE_ABI_VERSION" in py_crypto
        and "version >= PRIVACY_REQUIRED_BRIDGE_ABI_VERSION" not in py_crypto,
        "Python privacy bridge must require exact first-release ABI 21",
        errors,
    )
    require(
        "fn privacy_bridge_abi_version_py() -> u32" in py_native
        and "PRIVACY_BRIDGE_ABI_VERSION_V1" in py_native,
        "Python native privacy bridge must report first-release ABI 21",
        errors,
    )
    for relative, marker in (
        (
            "java/iroha_android/src/main/java/org/hyperledger/iroha/android/privacy/PrivacyNativeBridge.java",
            "REQUIRED_BRIDGE_ABI_VERSION = 21",
        ),
        (
            "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/privacy/PrivacyNativeBridge.kt",
            "REQUIRED_BRIDGE_ABI_VERSION: Int = 21",
        ),
        (
            "IrohaSwift/Sources/IrohaSwift/PrivacyNativeBridge.swift",
            "requiredBridgeABIVersion: UInt32 = 21",
        ),
        (
            "csharp/src/Hyperledger.Iroha.Sdk/Privacy/PrivacyNative.cs",
            "RequiredBridgeAbiVersion = 21",
        ),
    ):
        require(
            marker in read(relative, overrides),
            f"{relative} must require exact first-release ABI 21",
            errors,
        )

    require(
        js_protocol_ids(js_source) == EXPECTED_IDS,
        "JavaScript source capability registry must contain the exact 12 IDs in order",
        errors,
    )
    require(
        tuple(literal_assignment(py_catalog, "PRIVACY_PROTOCOL_IDS_V1"))
        == EXPECTED_IDS,
        "Python capability registry must contain the exact 12 IDs in order",
        errors,
    )
    require(
        rust_model.count("pub const COUNT: usize = 12;") == 1,
        "Rust PrivacyProtocolIdV1::COUNT must remain exactly 12",
        errors,
    )
    positions = [rust_model.find(f'"{protocol_id}"') for protocol_id in EXPECTED_IDS]
    require(
        all(position >= 0 for position in positions)
        and positions == sorted(positions),
        "Rust canonical privacy labels must include the exact IDs in order",
        errors,
    )

    for marker in (
        "objectWithExactKeys",
        "PRIVACY_CAPABILITY_SNAPSHOT_VERSION_V1",
        "PRIVACY_PROTOCOL_IDS_V1.length",
        "Object.freeze",
    ):
        require(marker in js_source, f"JavaScript strict parser lost {marker}", errors)
    for marker in (
        "_exact_object",
        "reject_duplicate_pairs",
        "PRIVACY_CAPABILITY_SNAPSHOT_MAX_JSON_BYTES_V1",
        "type(value) is not int",
    ):
        require(marker in py_catalog, f"Python strict parser lost {marker}", errors)

    public_files = (
        "javascript/iroha_js/src/index.js",
        "javascript/iroha_js/src/crypto.js",
        "javascript/iroha_js/src/crypto.browser.js",
        "javascript/iroha_js/index.d.ts",
        "python/iroha_python/src/iroha_python/__init__.py",
        "python/iroha_python/src/iroha_python/crypto.py",
        "crates/iroha_js_host/src/lib.rs",
        "python/iroha_python/iroha_python_rs/src/lib.rs",
    )
    for relative in public_files:
        source = read(relative, overrides)
        for symbol in RETIRED_PUBLIC_SYMBOLS:
            require(
                re.search(rf"\b{re.escape(symbol)}\b", source) is None,
                f"{relative} must not expose retired generic symbol {symbol}",
                errors,
            )

    mobile_capability_files = (
        (
            "java/iroha_android/src/main/java/org/hyperledger/iroha/android/privacy/PrivacyNativeBridge.java",
            "java/iroha_android/src/main/java/org/hyperledger/iroha/android/privacy/PrivacyProtocolIdV1.java",
        ),
        (
            "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/privacy/PrivacyNativeBridge.kt",
            "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/privacy/PrivacyIdsV1.kt",
        ),
        (
            "IrohaSwift/Sources/IrohaSwift/PrivacyNativeBridge.swift",
            "IrohaSwift/Sources/IrohaSwift/PrivacyNativeBridge.swift",
        ),
        (
            "csharp/src/Hyperledger.Iroha.Sdk/Privacy/PrivacyNative.cs",
            "csharp/src/Hyperledger.Iroha.Sdk/Privacy/PrivacyNative.cs",
        ),
    )
    for relative, registry_relative in mobile_capability_files:
        source = read(relative, overrides)
        registry_source = read(registry_relative, overrides)
        require(
            unique_expected_ids_in_source(registry_source) == EXPECTED_IDS,
            f"{registry_relative} must expose the exact 12 canonical IDs in order",
            errors,
        )
        for symbol in RETIRED_PUBLIC_SYMBOLS:
            require(
                re.search(rf"\b{re.escape(symbol)}\b", source) is None,
                f"{relative} must not expose retired generic symbol {symbol}",
                errors,
            )
        for protocol_id in RETIRED_IDS:
            require(
                protocol_id not in source and protocol_id not in registry_source,
                f"{relative} and its closed registry must not accept retired ID {protocol_id}",
                errors,
            )
        for marker in (
            "compiled",
            "catalog",
            "ProtocolIdV1",
            "unknown",
        ):
            require(
                marker.lower() in source.lower(),
                f"{relative} lost closed local catalog marker {marker}",
                errors,
            )

    backend_registry_sources = (
        "javascript/iroha_js/src/toriiClient.js",
        "python/iroha_python/src/iroha_python/_privacy_backends.py",
        "java/iroha_android/src/main/java/org/hyperledger/iroha/android/model/zk/VerifyingKeyBackendTag.java",
        "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/core/model/zk/VerifyingKeyBackendTag.kt",
        "IrohaSwift/Sources/IrohaSwift/VerifyingKeyBackendTag.swift",
        "csharp/src/Hyperledger.Iroha.Sdk/Zk/VerifyingKeyBackendTag.cs",
    )
    retired_alias_markers = (
        "PendingCatalogBackendAliases",
        "pendingCatalogBackendAliases",
        "pendingCompactLabels",
        "PENDING_CATALOG_BACKEND_ALIASES",
        "_PENDING_PRODUCTION_BACKEND_ALIASES",
        "isPendingProductionBackend",
        "is_pending_production_backend_label",
    )
    for relative in backend_registry_sources:
        source = read(relative, overrides)
        require(
            all(marker not in source for marker in retired_alias_markers),
            f"{relative} must not expose or normalize a pending backend-alias category",
            errors,
        )
        for protocol_id in RETIRED_IDS:
            require(
                protocol_id not in source,
                f"{relative} must treat retired protocol ID {protocol_id} as unsupported",
                errors,
            )

    backend_registry_tests = (
        "javascript/iroha_js/test/openVerifyEnvelope.test.js",
        "python/iroha_python/tests/privacy_backend_labels_test.py",
        "java/iroha_android/src/test/java/org/hyperledger/iroha/android/model/instructions/VerifyingKeyInstructionUtilsTests.java",
        "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/core/model/zk/VerifyingKeyBackendTagTest.kt",
        "IrohaSwift/Tests/IrohaSwiftTests/VerifyingKeyBackendTagTests.swift",
        "csharp/tests/Hyperledger.Iroha.Sdk.Tests/VerifyingKeyBackendTagTests.cs",
    )
    for relative in backend_registry_tests:
        source = read(relative, overrides)
        require(
            "sis-hints-anoncred-pq-v0" in source and "sis-with-hints" in source,
            f"{relative} must retain both explicit retired SIS alias rejection cases",
            errors,
        )

    validator_markers = (
        (
            "javascript/iroha_js/src/crypto.js",
            "privacyValidateCompiledProfileCatalogV1",
        ),
        (
            "python/iroha_python/src/iroha_python/crypto.py",
            "privacy_validate_compiled_profile_catalog_v1",
        ),
        (
            "java/iroha_android/src/main/java/org/hyperledger/iroha/android/privacy/PrivacyNativeBridge.java",
            "nativeValidateCompiledProfileCatalog",
        ),
        (
            "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/privacy/PrivacyNativeBridge.kt",
            "nativeValidateCompiledProfileCatalog",
        ),
        (
            "IrohaSwift/Sources/IrohaSwift/NativeBridge.swift",
            "iroha_privacy_validate_compiled_profile_catalog_v1",
        ),
        (
            "csharp/src/Hyperledger.Iroha.Sdk/Privacy/PrivacyNative.cs",
            "iroha_privacy_validate_compiled_profile_catalog_v1",
        ),
    )
    for relative, marker in validator_markers:
        source = read(relative, overrides)
        require(
            marker in source,
            f"{relative} must call the shared Rust typed compiled-catalog validator",
            errors,
        )
        require(
            "0x50" not in source
            and "CAPABILITY_SCHEMA_BYTE" not in source
            and "capabilitySchemaByte" not in source
            and "SchemaByte = 0x50" not in source,
            f"{relative} must not retain the fabricated repeated-byte schema gate",
            errors,
        )

    require(
        "pub fn validate_privacy_capability_archive_v1" in rust_protocol
        and "decode_canonical_with_limits::<PrivacyCapabilitySnapshotV1>"
        in rust_protocol
        and "PRIVACY_CAPABILITY_ARCHIVE_MAX_BYTES_V1: usize = 256 * 1024"
        in rust_protocol
        and "snapshot.validate().is_err()" in rust_protocol,
        "Rust data model must own the bounded canonical typed capability validator",
        errors,
    )
    require(
        "pub fn validate_privacy_compiled_profile_catalog_archive_v1" in rust_protocol
        and "decode_canonical_with_limits::<PrivacyCompiledProfileCatalogV1>"
        in rust_protocol
        and "PRIVACY_COMPILED_PROFILE_CATALOG_ARCHIVE_MAX_BYTES_V1: usize = 256 * 1024"
        in rust_protocol
        and "catalog.validate().is_err()" in rust_protocol,
        "Rust data model must own the bounded canonical typed compiled-catalog validator",
        errors,
    )
    for relative in (
        "crates/iroha_js_host/src/lib.rs",
        "python/iroha_python/iroha_python_rs/src/lib.rs",
    ):
        source = read(relative, overrides)
        require(
            "compiled_privacy_profile_catalog_v1" in source
            and "validate_local_privacy_compiled_profile_catalog_archive_v1" in source
            and "PrivacyCompiledProfileCatalogV1" in source,
            f"{relative} must expose and exactly validate only the local compiled catalog",
            errors,
        )
        require(
            "committed_privacy_capability_snapshot_v1" not in source,
            f"{relative} must not synthesize a live capability snapshot from local metadata",
            errors,
        )
        require(
            "PRIVACY_CAPABILITIES_RESULT_SCHEMA_BYTE" not in source
            and "privacy_patch_archive_schema_hash" not in source
            and "privacy_patch_archive_repeated_schema_byte" not in source,
            f"{relative} must not rewrite the canonical Norito schema hash",
            errors,
        )

    connect_bridge = read("crates/connect_norito_bridge/src/lib.rs", overrides)
    require(
        "compiled_privacy_profile_catalog_v1" in connect_bridge
        and "validate_local_privacy_compiled_profile_catalog_archive_v1" in connect_bridge
        and "PrivacyCompiledProfileCatalogV1" in connect_bridge
        and "PRIVACY_COMPILED_PROFILE_CATALOG_ARCHIVE_MAX_BYTES_V1" in connect_bridge,
        "connect bridge must expose only the bounded exact local compiled-profile catalog",
        errors,
    )
    require(
        "iroha_privacy_capabilities_v1" not in connect_bridge
        and "iroha_privacy_validate_capabilities_v1" not in connect_bridge,
        "connect bridge must not expose a local archive as authoritative capabilities",
        errors,
    )

    capability_only_native_files = (
        "crates/connect_norito_bridge/src/lib.rs",
        "crates/connect_norito_bridge/include/connect_norito_bridge.h",
        "IrohaSwift/Sources/IrohaSwift/NativeBridge.swift",
        "java/iroha_android/src/main/java/org/hyperledger/iroha/android/privacy/PrivacyConfidentialWitness.java",
        "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/privacy/PrivacyConfidentialWitness.kt",
        "IrohaSwift/Sources/IrohaSwift/PrivacyConfidentialWitness.swift",
        "csharp/src/Hyperledger.Iroha.Sdk/Privacy/PrivacyNative.cs",
    )
    for relative in capability_only_native_files:
        source = read(relative, overrides)
        for symbol in RETIRED_PUBLIC_SYMBOLS:
            require(
                re.search(rf"\b{re.escape(symbol)}\b", source) is None,
                f"{relative} retains retired generic privacy route {symbol}",
                errors,
            )

    swift_native_bridge = read(
        "IrohaSwift/Sources/IrohaSwift/NativeBridge.swift", overrides
    )
    require(
        "privacyFreeFn ?? freeFn" not in swift_native_bridge,
        "Swift privacy buffers must never fall back to connect_norito_free",
        errors,
    )
    require(
        "&& privacyFreeFn != nil" in swift_native_bridge
        and "let privacyFreeFn else" in swift_native_bridge,
        "Swift privacy availability and archive consumers must require the dedicated zeroizing free",
        errors,
    )
    require(
        "loadedBridgeAbiVersion == PrivacyNativeBridge.requiredBridgeABIVersion"
        in swift_native_bridge,
        "Swift privacy availability must require exact first-release ABI 21",
        errors,
    )

    c_header = read(
        "crates/connect_norito_bridge/include/connect_norito_bridge.h", overrides
    )
    require(
        set(re.findall(r"\b(iroha_privacy_[a-z0-9_]+)\s*\(", c_header))
        == {
            "iroha_privacy_compiled_profile_catalog_v1",
            "iroha_privacy_validate_compiled_profile_catalog_v1",
            "iroha_privacy_exact12_fixture_bundle_v1",
            "iroha_privacy_validate_exact12_fixture_bundle_v1",
            "iroha_privacy_free_buffer",
        },
        "C privacy ABI must contain only local compiled-profile catalog, exact-12 conformance, typed validators, and zeroizing free",
        errors,
    )
    cli_root = root / "crates/iroha_cli"
    if cli_root.exists():
        cli_source = "\n".join(
            path.read_text(encoding="utf-8")
            for path in cli_root.rglob("*.rs")
        )
        for symbol in RETIRED_PUBLIC_SYMBOLS:
            require(
                re.search(rf"\b{re.escape(symbol)}\b", cli_source) is None,
                f"Rust CLI must fail closed instead of exposing {symbol}",
                errors,
            )

    for relative in (
        "javascript/iroha_js/src/privacyCapabilities.js",
        "python/iroha_python/src/iroha_python/privacy_catalog.py",
    ):
        source = read(relative, overrides)
        for protocol_id in RETIRED_IDS:
            require(
                protocol_id not in source,
                f"{relative} must not accept retired ID {protocol_id}",
                errors,
            )

    for relative in (
        "crates/iroha_js_host/src/lib.rs",
        "python/iroha_python/iroha_python_rs/src/lib.rs",
    ):
        source = read(relative, overrides)
        for marker in ("PrivacyCompiledProfileCatalogV1", "PrivacyProtocolIdV1::ALL"):
            require(marker in source, f"{relative} lost local catalog marker {marker}", errors)
        for marker in (
            "struct PrivacyAlgorithmEntry",
            "struct PrivacyCapabilitiesV1",
            "PRIVACY_ALGORITHM_ENTRIES",
        ):
            require(marker not in source, f"{relative} retains legacy catalog {marker}", errors)

    for marker in ("PrivacyCompiledProfileCatalogV1", "PrivacyProtocolIdV1::ALL"):
        require(
            marker in connect_bridge,
            f"crates/connect_norito_bridge/src/lib.rs lost local catalog marker {marker}",
            errors,
        )

    require(
        "mod privacy_production;" not in read(
            "crates/connect_norito_bridge/src/lib.rs", overrides
        ),
        "connect bridge must not compile the retired generic production dispatcher",
        errors,
    )
    require(
        not (root / "crates/connect_norito_bridge/src/privacy_production.rs").exists(),
        "retired connect privacy_production.rs must remain deleted",
        errors,
    )
    require(
        not (root / "javascript/iroha_js/src/privacyAlgorithms.js").exists(),
        "retired JavaScript editorial privacy catalog must remain deleted",
        errors,
    )
    for module in DELETED_PYTHON_MODULES:
        require(
            not (
                root / "python/iroha_python/src/iroha_python" / module
            ).exists(),
            f"retired Python module {module} must remain deleted",
            errors,
        )

    js_tests = read("javascript/iroha_js/test/privacyCatalogParity.test.js", overrides)
    py_tests = read("python/iroha_python/tests/privacy_catalog_test.py", overrides)
    matrix_consumers = (
        "crates/iroha_data_model/src/privacy.rs",
        "javascript/iroha_js/test/privacyCatalogParity.test.js",
        "python/iroha_python/tests/privacy_catalog_test.py",
        "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/privacy/PrivacyNativeBridgeTest.kt",
        "java/iroha_android/src/test/java/org/hyperledger/iroha/android/privacy/PrivacyNativeBridgeTest.java",
        "IrohaSwift/Tests/IrohaSwiftTests/PrivacyNativeBridgeTests.swift",
        "csharp/tests/Hyperledger.Iroha.Sdk.Tests/PrivacyNativeTests.cs",
    )
    for relative in matrix_consumers:
        require(
            "exact12_v1.tsv" in read(relative, overrides),
            f"{relative} must consume the shared exact12 matrix",
            errors,
        )
    for marker in ("unknown fields", "aliases", "canonical 12"):
        require(
            marker.lower() in js_tests.lower() or marker.lower() in py_tests.lower(),
            f"strict SDK tests must retain {marker} coverage",
            errors,
        )
    require(
        "duplicate" in py_tests.lower() and "NaN" in py_tests,
        "Python tests must retain duplicate-key and non-finite-number rejection",
        errors,
    )

    workflow_source = read(
        ".github/workflows/pr_privacy_sdk_guard.yml", overrides
    )
    lock_helper_source = read("ci/privacy_sdk_cargo_lockfile.sh", overrides)
    cargo_wrapper_source = read("ci/privacy_sdk_cargo_wrapper.sh", overrides)
    python_sdk_guard_source = read("ci/check_privacy_python_sdk.sh", overrides)
    python_wheel_verifier_source = read(
        "ci/verify_privacy_python_wheel.py", overrides
    )
    python_conftest_source = read(
        "python/iroha_python/tests/conftest.py", overrides
    )
    python_import_fallback_source = read(
        "python/iroha_python/tests/package_import_fallback_test.py", overrides
    )
    python_pyproject_source = read("python/iroha_python/pyproject.toml", overrides)
    workflow = _check_cargo_workflow(workflow_source, errors)
    required_workflow_paths = (
        ".gitignore",
        ".cargo/config",
        ".cargo/config.toml",
        "**/.cargo/config",
        "**/.cargo/config.toml",
        "Cargo.toml",
        "**/Cargo.toml",
        "Cargo.lock",
        "**/Cargo.lock",
        "rust-toolchain",
        "rust-toolchain.toml",
        "**/rust-toolchain",
        "**/rust-toolchain.toml",
        "crates/**",
        "vendor/**",
        "crates/iroha_crypto/**",
        "crates/iroha_data_model/**",
        "crates/iroha_primitives/**",
        "crates/iroha_schema/**",
        "crates/iroha_version/**",
        "crates/iroha_torii_shared/**",
        "crates/norito/**",
        "crates/ivm/**",
        "crates/sorafs_manifest/**",
        "crates/iroha_config/**",
        "crates/iroha_core/**",
        "crates/iroha_zkp_halo2/**",
        "crates/zk_ace_prover/**",
        "crates/sorafs_car/**",
        "crates/sorafs_chunker/**",
        "crates/sorafs_orchestrator/**",
        "ci/verify_privacy_python_wheel.py",
        "python/iroha_python/pyproject.toml",
        "python/iroha_python/iroha_python_rs/build.rs",
        "python/iroha_python/iroha_python_rs/src/**",
        "python/iroha_python/requirements-ci.lock",
        "python/iroha_python/tests/conftest.py",
        "python/iroha_python/tests/package_import_fallback_test.py",
        "python/iroha_python/src/**",
        "python/iroha_python/src/**/*.py",
        "python/iroha_python/src/**/*.so",
        "python/iroha_python/src/**/*.dylib",
        "python/iroha_python/src/**/*.pyd",
        "python/norito_py/**",
        "python/norito_py/pyproject.toml",
        "python/norito_py/src/**/*.py",
        "python/iroha_torii_client/**",
        "python/iroha_torii_client/pyproject.toml",
        "python/iroha_torii_client/**/*.py",
    )
    _check_workflow_trigger_paths(workflow, required_workflow_paths, errors)
    lock_helper_executable_source = "\n".join(
        line
        for line in lock_helper_source.splitlines()
        if not line.lstrip().startswith("#")
    )
    github_path_write_block = "\n".join(
        (
            "  printf '%s\\n' \"${toolchain_bin_directory}\" \\",
            '    >>"${github_path_path}" || return 1',
            "  printf '%s\\n' \"${cargo_wrapper_directory}\" \\",
            '    >>"${github_path_path}" || return 1',
        )
    )
    require(
        'RUSTC_BOOTSTRAP=1 \\' in lock_helper_source
        and '"${real_cargo}" -Z unstable-options generate-lockfile'
        in lock_helper_source
        and 'CARGO_HOME="${private_cargo_home}"' in lock_helper_source
        and "CARGO_NET_OFFLINE=false" in lock_helper_source
        and '--lockfile-path "${lock_path}"' in lock_helper_source
        and "privacy_sdk_validate_repository_cargo_configuration"
        in lock_helper_source
        and "privacy_sdk_prepare_private_cargo_home" in lock_helper_source
        and "CARGO_ENCODED_RUSTDOCFLAGS" in lock_helper_source
        and "IROHA_PRIVACY_AUTHENTICATED_CARGO_CONFIG_SEAL"
        in lock_helper_source
        and "IROHA_PRIVACY_AUTHENTICATED_CARGO_HOME" in lock_helper_source
        and "IROHA_PRIVACY_AUTHENTICATED_WORKSPACE_CARGO_LOCK_STATE=absent"
        in lock_helper_source
        and '"CARGO",' in lock_helper_source
        and "privacy SDK CI Cargo selector must be absent before wrapper selection"
        in lock_helper_source
        and "privacy SDK CI deterministic Cargo environment changed"
        in lock_helper_source
        and lock_helper_executable_source.count(github_path_write_block) == 1
        and lock_helper_executable_source.count(
            "printf '%s\\n' \"${toolchain_bin_directory}\""
        )
        == 1
        and lock_helper_executable_source.count(
            "printf '%s\\n' \"${cargo_wrapper_directory}\""
        )
        == 1,
        "privacy SDK CI helper must generate only the authenticated external lock and install toolchain/wrapper PATH ordering",
        errors,
    )
    require(
        "run_real_cargo_and_verify_locks" in cargo_wrapper_source
        and cargo_wrapper_source.count("assert_authenticated_cargo_lock_state")
        >= 3
        and "assert_authenticated_cargo_configuration" in cargo_wrapper_source
        and "IROHA_PRIVACY_AUTHENTICATED_WORKSPACE_CARGO_LOCK_STATE"
        in cargo_wrapper_source,
        "privacy SDK Cargo wrapper must verify selected and workspace locks before and after Cargo",
        errors,
    )
    require(
        "assert_no_cargo_policy_environment" in cargo_wrapper_source
        and "CARGO_ALIAS_*" in cargo_wrapper_source
        and "CARGO_HOST_*" in cargo_wrapper_source
        and "CARGO_UNSTABLE_*" in cargo_wrapper_source
        and "RUSTC_WORKSPACE_WRAPPER" in cargo_wrapper_source
        and "RUSTUP_*" in cargo_wrapper_source
        and "rejected caller-supplied rustc-side compiler arguments"
        in cargo_wrapper_source
        and "configuration search path" in cargo_wrapper_source
        and "cargo_child_environment" in cargo_wrapper_source
        and 'cargo_child_environment+=("CARGO=${CARGO_WRAPPER_ENTRY_PATH}")'
        in cargo_wrapper_source
        and "IROHA_PRIVACY_*|IROHA_JS_CARGO_LOCKFILE_PATH"
        in cargo_wrapper_source
        and "command_index=-1" in cargo_wrapper_source
        and "cargo_argument_limit=" in cargo_wrapper_source
        and "rejected --config because" in cargo_wrapper_source
        and "is_cargo_jobs_option" in cargo_wrapper_source
        and "assert_authenticated_jobs_option" in cargo_wrapper_source
        and "assert_authenticated_target_dir_option" in cargo_wrapper_source
        and "assert_reinforcing_offline_option" in cargo_wrapper_source
        and "metadata|rustc|build|check|test|fetch)"
        in cargo_wrapper_source
        and "--artifact-dir|--artifact-dir=*|--out-dir|--out-dir=*|--root|--root=*"
        in cargo_wrapper_source
        and "artifact or installation output override"
        in cargo_wrapper_source
        and "--crate-type|--crate-type=*" in cargo_wrapper_source
        and "Cargo-side compiler output override" in cargo_wrapper_source
        and "incremental policy must remain disabled" in cargo_wrapper_source
        and "encoded rustflags are not authenticated" in cargo_wrapper_source
        and "assert_authenticated_maturin_darwin_rustc_policy"
        in cargo_wrapper_source
        and "assert_authenticated_python_command_environment"
        in cargo_wrapper_source
        and "native Cargo invocation must not inherit the Cargo selector environment"
        in cargo_wrapper_source
        and "Maturin metadata must select the authenticated Cargo wrapper"
        in cargo_wrapper_source
        and "Maturin rustc must not inherit the Cargo selector environment"
        in cargo_wrapper_source
        and "assert_authenticated_maturin_arguments" in cargo_wrapper_source
        and "Maturin Cargo arguments do not match pinned 1.14.1"
        in cargo_wrapper_source
        and "IROHA_PRIVACY_AUTHENTICATED_PYTHON_BUILD_POLICY"
        in cargo_wrapper_source
        and "native Cargo invocation rejected an unauthenticated Python-build surface"
        in cargo_wrapper_source
        and "CARGO_ENCODED_RUSTDOCFLAGS" in cargo_wrapper_source
        and "requires an authenticated Cargo home" in cargo_wrapper_source
        and "requires an authenticated Cargo config path" in cargo_wrapper_source
        and "requires an authenticated Cargo config seal" in cargo_wrapper_source
        and "home must not contain Cargo configuration" in cargo_wrapper_source
        and "privacy_sdk_assert_directory_state" in cargo_wrapper_source,
        "privacy SDK Cargo wrapper must reject alias/config/policy bypasses and authenticate its private Cargo home",
        errors,
    )
    require(
        "resolve_python_312_bin" in python_sdk_guard_source
        and "IROHA_PRIVACY_LOCKFILE_PYTHON_BIN" in python_sdk_guard_source
        and "requirements-ci.lock" in python_sdk_guard_source
        and "--require-hashes" in python_sdk_guard_source
        and "--only-binary=:all:" in python_sdk_guard_source
        and "--force-reinstall" in python_sdk_guard_source
        and "'pytest>=8.0'" not in python_sdk_guard_source
        and "'requests>=2.31'" not in python_sdk_guard_source,
        "privacy Python SDK gate must use authenticated Python 3.12 and freshly reinstall the hash-pinned dependency lock",
        errors,
    )
    require(
        "PRIVACY_PYTHON_SDK_VENV is forbidden" in python_sdk_guard_source
        and "Python/root overrides require explicit test mode"
        in python_sdk_guard_source
        and "PRIVACY_PYTHON_SDK_TEST_MODE" in python_sdk_guard_source
        and "PRIVACY_PYTHON_SDK_TEST_VENV" in python_sdk_guard_source
        and "assert_no_python_startup_injection" in python_sdk_guard_source
        and "capture_venv_distribution_names" in python_sdk_guard_source
        and "assert_expected_venv_distributions" in python_sdk_guard_source
        and "PYTEST_DISABLE_PLUGIN_AUTOLOAD=1" in python_sdk_guard_source
        and '"PIP_"' in python_sdk_guard_source
        and "PIP_CONFIG_FILE=/dev/null" in python_sdk_guard_source
        and "--isolated" in python_sdk_guard_source
        and "--no-cache-dir" in python_sdk_guard_source
        and '"CARGO",' in python_sdk_guard_source
        and 'name == "CARGO_INCREMENTAL" and os.environ[name] == "0"'
        in python_sdk_guard_source
        and 'name == "CARGO_ENCODED_RUSTFLAGS" and os.environ[name] == ""'
        in python_sdk_guard_source
        and "configure_private_cargo_home" in python_sdk_guard_source
        and 'privacy_sdk_assert_ci_cargo_lock_state "${ROOT_DIR}" "${PYTHON_BIN}"'
        in python_sdk_guard_source
        and "exactly for metadata then rustc" in python_sdk_guard_source
        and "inherited authenticated Cargo home does not match CARGO_HOME"
        in python_sdk_guard_source
        and "validate_repository_cargo_configuration" in python_sdk_guard_source
        and "IROHA_PRIVACY_AUTHENTICATED_CARGO_HOME"
        in python_sdk_guard_source
        and "IROHA_PRIVACY_AUTHENTICATED_CARGO_CONFIG_SEAL"
        in python_sdk_guard_source
        and "IROHA_PRIVACY_AUTHENTICATED_PYO3_PYTHON"
        in python_sdk_guard_source,
        "privacy Python SDK gate must use a fresh production venv with sealed distributions, startup isolation, and a clean authenticated Cargo home",
        errors,
    )
    require(
        "IROHA_PYTHON_SKIP_RUNTIME_LINK=1" in python_sdk_guard_source
        and 'PYO3_PYTHON="${VENV_DIR}/bin/python"' in python_sdk_guard_source
        and "CARGO_BUILD_JOBS=1" in python_sdk_guard_source
        and "CARGO_NET_OFFLINE=true" in python_sdk_guard_source
        and 'CARGO_TARGET_DIR="${PRIVATE_CARGO_TARGET_DIR}"'
        in python_sdk_guard_source
        and "--locked" in python_sdk_guard_source
        and "--offline" in python_sdk_guard_source
        and "--jobs 1" in python_sdk_guard_source
        and '--target-dir "${PRIVATE_CARGO_TARGET_DIR}"'
        in python_sdk_guard_source
        and "-I -m maturin build" in python_sdk_guard_source
        and "-m maturin develop" not in python_sdk_guard_source
        and '--out "${PRIVATE_WHEEL_DIR}"' in python_sdk_guard_source,
        "privacy Python SDK native build must produce a wheel offline, single-job, runtime-unlinked, and private-targeted",
        errors,
    )
    require(
        '"${VENV_DIR}/bin/python" -I -B -m pip'
        in python_sdk_guard_source
        and "--no-compile" in python_sdk_guard_source
        and "--no-deps" in python_sdk_guard_source
        and "--no-index" in python_sdk_guard_source
        and "resolve_private_wheel" in python_sdk_guard_source
        and "privacy_python_sdk_file_seal" in python_sdk_guard_source
        and "verify_installed_wheel" in python_sdk_guard_source
        and "verify_privacy_python_wheel.py" in python_sdk_guard_source
        and python_sdk_guard_source.count(
            '"${VENV_DIR}/bin/python" -I -B \\'
        )
        == 2
        and '"${VENV_DIR}/bin/python" -I -B -m pytest -q \\'
        in python_sdk_guard_source
        and '"${ROOT_DIR}/python/norito_py/src"' in python_sdk_guard_source
        and '"${ROOT_DIR}/python/iroha_torii_client"'
        in python_sdk_guard_source
        and "IROHA_PRIVACY_AUTHENTICATED_WHEEL_SEAL"
        in python_sdk_guard_source
        and '"${WHEEL_SEAL}"' in python_sdk_guard_source
        and "preflight_private_wheel" in python_sdk_guard_source
        and "preflight_wheel" in python_wheel_verifier_source
        and 'sys.argv[1] == "--preflight"' in python_wheel_verifier_source
        and "expected_wheel_seal" in python_wheel_verifier_source
        and "_canonical_zip_member_name" in python_wheel_verifier_source
        and "_assert_contiguous_local_records" in python_wheel_verifier_source
        and "data descriptor does not exactly cover"
        in python_wheel_verifier_source
        and "authenticate_dependency_roots" in python_wheel_verifier_source
        and "_capture_dependency_tree" in python_wheel_verifier_source
        and "a bytecode or native loader alias"
        in python_wheel_verifier_source
        and "sys.dont_write_bytecode = True"
        in python_wheel_verifier_source
        and "return basename.casefold().endswith(NATIVE_FILE_ENDINGS)"
        in python_wheel_verifier_source
        and "MAX_TOTAL_UNCOMPRESSED_BYTES" in python_wheel_verifier_source
        and "DIST_INFO_REQUIRED_FILES" in python_wheel_verifier_source
        and "scripts, data roots, or other packages"
        in python_wheel_verifier_source
        and "reject_preseeded_modules" in python_wheel_verifier_source
        and "ExtensionFileLoader" in python_wheel_verifier_source
        and "loader_state" in python_wheel_verifier_source
        and "verify_installed_files" in python_wheel_verifier_source
        and "package_members" in python_wheel_verifier_source
        and "dist_info_members" in python_wheel_verifier_source
        and "_assert_unique_distribution_origin"
        in python_wheel_verifier_source
        and "_assert_record_payload" in python_wheel_verifier_source
        and "PIP_GENERATED_DIST_INFO_FILES"
        in python_wheel_verifier_source
        and "bytecode cache directory" in python_wheel_verifier_source
        and "iroha_python/__init__.py" in python_wheel_verifier_source
        and "iroha_python._crypto" in python_wheel_verifier_source
        and "Python.framework" in python_wheel_verifier_source
        and "libpython" in python_wheel_verifier_source
        and '[str(otool), "-L", str(native_path)]'
        in python_wheel_verifier_source,
        "privacy Python SDK gate must install exactly one sealed wheel and verify its package, native bytes, and Darwin links",
        errors,
    )
    require(
        "checkout_native_artifact_state" in python_sdk_guard_source
        and "assert_checkout_native_artifacts_unchanged"
        in python_sdk_guard_source
        and 'native_endings = (".so", ".dylib", ".pyd")'
        in python_sdk_guard_source
        and "path.name.casefold().endswith(native_endings)"
        in python_sdk_guard_source
        and "native artifact suffix must be lowercase"
        in python_sdk_guard_source,
        "privacy Python SDK gate must seal every checkout native extension before and after the build",
        errors,
    )
    require(
        all(
            marker in python_pyproject_source
            for marker in (
                '"src/**/*.so"',
                '"src/**/*.dylib"',
                '"src/**/*.pyd"',
                '"src/**/__pycache__/**"',
                '"src/**/*.pyc"',
                '"src/**/*.pyo"',
            )
        ),
        "privacy Python wheel policy must exclude checkout-native and bytecode artifacts from mixed-project inputs",
        errors,
    )
    for rejected_environment_name in (
        "CARGO_BUILD_TARGET",
        "CARGO_BUILD_RUSTC",
        "CARGO_BUILD_RUSTC_WRAPPER",
        "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER",
        "CARGO_BUILD_RUSTFLAGS",
        "CARGO_BUILD_RUSTDOC",
        "CARGO_BUILD_RUSTDOCFLAGS",
        "CARGO_BUILD_",
        "CARGO_ALIAS_",
        "CARGO_HTTP_",
        "CARGO_HOST_",
        "CARGO_NET_",
        "CARGO_REGISTRIES_",
        "CARGO_REGISTRY_",
        "CARGO_SOURCE_",
        "CARGO_TARGET_",
        "CARGO_UNSTABLE_",
        "RUSTFLAGS",
        "RUSTC",
        "RUSTC_WRAPPER",
        "RUSTC_WORKSPACE_WRAPPER",
        "RUSTDOCFLAGS",
        "RUSTUP_TOOLCHAIN",
        "CARGO_ENCODED_RUSTFLAGS",
        "CARGO_ENCODED_RUSTDOCFLAGS",
        "PYO3_CONFIG_FILE",
        "PYO3_",
        "PYTHON_SYS_EXECUTABLE",
        "PYTHONOPTIMIZE",
        '"PYTHON"',
        "IROHA_PYTHON_RUNTIME_PATH",
        "CARGO_PROFILE_",
    ):
        require(
            rejected_environment_name in python_sdk_guard_source,
            f"privacy Python SDK gate must reject ambient {rejected_environment_name}",
            errors,
        )
    require(
        "IROHA_PYTHON_TEST_INSTALLED_PACKAGE=1" in python_sdk_guard_source
        and "IROHA_PYTHON_TEST_INSTALLED_PACKAGE" in python_conftest_source
        and "site.getsitepackages()" in python_conftest_source
        and "sysconfig.get_paths()" in python_conftest_source
        and "iroha_python._crypto" in python_conftest_source
        and "PathFinder.find_spec" in python_conftest_source
        and "ExtensionFileLoader" in python_conftest_source
        and "loader_state" in python_conftest_source
        and 'env.get("IROHA_PYTHON_TEST_INSTALLED_PACKAGE") != "1"'
        in python_import_fallback_source
        and 'PYTHONPATH="${ROOT_DIR}/python/norito_py/src:${ROOT_DIR}/python"'
        in python_sdk_guard_source,
        "privacy Python SDK tests must import the installed wheel from private venv site-packages without checkout source fallback",
        errors,
    )
    require(
        "must not select the workspace Cargo.lock" in lock_helper_source
        and "before.st_nlink != 1" in lock_helper_source,
        "privacy SDK lock selection must admit only singly linked non-workspace locks",
        errors,
    )

    if errors:
        raise GuardFailure("\n".join(f"- {error}" for error in errors))


if mode:
    if not mode.startswith("--negative-control-"):
        raise SystemExit(f"unknown mode: {mode}")
    try:
        if mode in EXACT12_FEATURE_NEGATIVE_CONTROLS:
            check_exact12_feature_boundary()
        else:
            check()
    except (GuardFailure, SyntaxError, ValueError) as error:
        raise SystemExit(
            "negative control requires a valid canonical baseline:\n" + str(error)
        ) from error
    overrides: dict[str, str] = {}
    if mode in WORKFLOW_META_NEGATIVE_CONTROLS:
        path = ".github/workflows/pr_privacy_sdk_guard.yml"
        overrides[path] = _mutate_workflow_meta_negative_control(
            read(path, {}), mode
        )
    elif mode in {
        "--negative-control-cargo-lock-native-workflow",
        "--negative-control-cargo-lock-python-workflow",
        "--negative-control-cargo-lock-guard-workflow",
    }:
        path = ".github/workflows/pr_privacy_sdk_guard.yml"
        job = {
            "--negative-control-cargo-lock-native-workflow": "privacy_native_bridge_tests",
            "--negative-control-cargo-lock-python-workflow": "privacy_python_sdk_tests",
            "--negative-control-cargo-lock-guard-workflow": "privacy-sdk-guard",
        }[mode]
        source = read(path, {})
        match = re.search(
            rf"(?ms)^  {re.escape(job)}:\n(.*?)(?=^  [A-Za-z0-9_-]+:\n|\Z)",
            source,
        )
        if match is None:
            raise SystemExit(f"negative control cannot find workflow job: {job}")
        block = match.group(0).replace("provision-ci", "bypassed-provision", 1)
        overrides[path] = source[: match.start()] + block + source[match.end() :]
    elif mode == "--negative-control-cargo-lock-helper-generation":
        path = "ci/privacy_sdk_cargo_lockfile.sh"
        overrides[path] = read(path, {}).replace(
            "generate-lockfile", "generate-workspace-lockfile", 1
        )
    elif mode == "--negative-control-cargo-config-workflow-path":
        path = ".github/workflows/pr_privacy_sdk_guard.yml"
        overrides[path] = read(path, {}).replace(
            '      - "**/.cargo/config"\n', "", 1
        )
    elif mode == "--negative-control-native-crates-workflow-path":
        path = ".github/workflows/pr_privacy_sdk_guard.yml"
        overrides[path] = read(path, {}).replace(
            '      - "crates/**"\n', "", 1
        )
    elif mode == "--negative-control-rust-toolchain-workflow-path":
        path = ".github/workflows/pr_privacy_sdk_guard.yml"
        overrides[path] = read(path, {}).replace(
            '      - "**/rust-toolchain"\n', "", 1
        )
    elif mode == "--negative-control-cargo-lock-workflow-path":
        path = ".github/workflows/pr_privacy_sdk_guard.yml"
        overrides[path] = read(path, {}).replace(
            '      - "**/Cargo.lock"\n', "", 1
        )
    elif mode == "--negative-control-cargo-manifest-workflow-path":
        path = ".github/workflows/pr_privacy_sdk_guard.yml"
        overrides[path] = read(path, {}).replace(
            '      - "**/Cargo.toml"\n', "", 1
        )
    elif mode == "--negative-control-vendor-workflow-path":
        path = ".github/workflows/pr_privacy_sdk_guard.yml"
        overrides[path] = read(path, {}).replace(
            '      - "vendor/**"\n', "", 1
        )
    elif mode == "--negative-control-rust-toolchain-install-workflow":
        path = ".github/workflows/pr_privacy_sdk_guard.yml"
        overrides[path] = read(path, {}).replace(
            '            RUSTUP_DIST_SERVER="https://static.rust-lang.org" \\\n',
            "",
            1,
        )
    elif mode == "--negative-control-cargo-fetch-workflow":
        path = ".github/workflows/pr_privacy_sdk_guard.yml"
        overrides[path] = read(path, {}).replace(
            '          CARGO_NET_OFFLINE: "false"\n',
            '          CARGO_NET_OFFLINE: "true"\n',
            1,
        )
    elif mode == "--negative-control-python-setup-cache-workflow":
        path = ".github/workflows/pr_privacy_sdk_guard.yml"
        overrides[path] = read(path, {}).replace(
            '          update-environment: false\n',
            '          update-environment: false\n          cache: pip\n',
            1,
        )
    elif mode == "--negative-control-python-path-threading-workflow":
        path = ".github/workflows/pr_privacy_sdk_guard.yml"
        overrides[path] = read(path, {}).replace(
            "          SETUP_PYTHON_PATH: "
            "${{ steps.privacy-python.outputs.python-path }}\n",
            "          SETUP_PYTHON_PATH: python3\n",
            1,
        )
    elif mode == "--negative-control-js-privacy-abi-drift":
        path = "javascript/iroha_js/src/crypto.js"
        overrides[path] = read(path, {}).replace(
            "PRIVACY_REQUIRED_BRIDGE_ABI_VERSION = 21",
            "PRIVACY_REQUIRED_BRIDGE_ABI_VERSION = 20",
            1,
        )
    elif mode == "--negative-control-python-privacy-abi-drift":
        path = "python/iroha_python/src/iroha_python/crypto.py"
        overrides[path] = read(path, {}).replace(
            "PRIVACY_REQUIRED_BRIDGE_ABI_VERSION: Final[int] = 21",
            "PRIVACY_REQUIRED_BRIDGE_ABI_VERSION: Final[int] = 22",
            1,
        )
    elif mode == "--negative-control-canonical-backend-alias-rejection-coverage":
        path = "python/iroha_python/src/iroha_python/_privacy_backends.py"
        overrides[path] = read(path, {}) + (
            '\n_PENDING_PRODUCTION_BACKEND_ALIASES = {"siswithhints"}\n'
        )
    elif mode == "--negative-control-exact12-bridge-test-fixtures":
        path = "crates/connect_norito_bridge/Cargo.toml"
        overrides[path] = read(path, {}).replace(
            '"privacy-exact12-conformance"',
            '"test-fixtures"',
            1,
        )
    elif mode == "--negative-control-exact12-conformance-rand-edge":
        path = "crates/iroha_data_model/Cargo.toml"
        overrides[path] = read(path, {}).replace(
            "privacy-exact12-conformance = []",
            'privacy-exact12-conformance = ["iroha_crypto/rand"]',
            1,
        )
    elif mode == "--negative-control-exact12-release-evidence-test-fixtures":
        path = "crates/iroha_core/Cargo.toml"
        overrides[path] = read(path, {}).replace(
            '"iroha_data_model/privacy-exact12-conformance"',
            '"iroha_data_model/test-fixtures"',
            1,
        )
    else:
        selector = sum(mode.encode("utf-8")) % 4
        if selector == 0:
            path = "javascript/iroha_js/src/privacyCapabilities.js"
            overrides[path] = read(path, {}).replace(
                '"iroha-zk-ams-v1"', '"zk-ams-recursive-admission-v0"', 1
            )
        elif selector == 1:
            path = "python/iroha_python/src/iroha_python/privacy_catalog.py"
            overrides[path] = read(path, {}).replace(
                '"iroha-zk-x509-stark-p256-v0",',
                '"iroha-jindo-polynomial-commitment-v0",',
            )
        elif selector == 2:
            path = "javascript/iroha_js/src/index.js"
            overrides[path] = read(path, {}) + "\nexport const privacyBuildProofV1 = null;\n"
        else:
            path = "crates/iroha_js_host/src/lib.rs"
            overrides[path] = read(path, {}).replace("PrivacyProtocolIdV1::ALL", "[]")
    try:
        if mode in EXACT12_FEATURE_NEGATIVE_CONTROLS:
            check_exact12_feature_boundary(overrides)
        else:
            check(overrides)
    except (GuardFailure, SyntaxError, ValueError):
        print(f"negative control rejected canonical privacy SDK drift: {mode}")
        raise SystemExit(0)
    raise SystemExit(f"negative control was not detected: {mode}")

try:
    check()
except (GuardFailure, SyntaxError, ValueError) as error:
    print("privacy SDK canonical cutover guard failed:", file=sys.stderr)
    print(error, file=sys.stderr)
    raise SystemExit(1)

print("privacy SDK canonical cutover guard passed")
PY

if [[ -n "${MODE}" || "${PRIVACY_SDK_GUARD_SKIP_RUNTIME:-0}" == "1" ]]; then
  exit 0
fi

# Runtime SDK checks resolve Rust dependencies only through one explicitly
# selected authenticated non-workspace lock. Normalize the shared selection
# once and pass the same canonical path to every SDK-specific guard.
# shellcheck source=ci/privacy_sdk_cargo_lockfile.sh
source "${ROOT_DIR}/ci/privacy_sdk_cargo_lockfile.sh"
PRIVACY_SDK_CARGO_LOCKFILE="$(
  privacy_sdk_resolve_cargo_lockfile "${ROOT_DIR}" "${PYTHON_BIN}"
)"
export IROHA_PRIVACY_CARGO_LOCKFILE_PATH="${PRIVACY_SDK_CARGO_LOCKFILE}"
export IROHA_JS_CARGO_LOCKFILE_PATH="${PRIVACY_SDK_CARGO_LOCKFILE}"

PRIVACY_SDK_CARGO_LOCK_SEAL="$(
  privacy_sdk_file_seal "${PRIVACY_SDK_CARGO_LOCKFILE}" "${PYTHON_BIN}"
)"
PRIVACY_SDK_WORKSPACE_LOCK="${ROOT_DIR}/Cargo.lock"
PRIVACY_SDK_WORKSPACE_LOCK_STATE="$(
  privacy_sdk_capture_optional_file_state \
    "${PRIVACY_SDK_WORKSPACE_LOCK}" \
    "workspace Cargo.lock" \
    "${PYTHON_BIN}"
)"

assert_privacy_sdk_guard_lock_state() {
  local status=0
  privacy_sdk_assert_file_seal \
    "${PRIVACY_SDK_CARGO_LOCKFILE}" \
    "${PRIVACY_SDK_CARGO_LOCK_SEAL}" \
    "selected authenticated Cargo.lock" \
    "${PYTHON_BIN}" || status=1
  privacy_sdk_assert_optional_file_state \
    "${PRIVACY_SDK_WORKSPACE_LOCK}" \
    "${PRIVACY_SDK_WORKSPACE_LOCK_STATE}" \
    "workspace Cargo.lock" \
    "${PYTHON_BIN}" || status=1
  return "${status}"
}

cleanup_privacy_sdk_guard_lock_state() {
  local status=$?
  trap - EXIT HUP INT TERM
  if ! assert_privacy_sdk_guard_lock_state; then
    status=1
  fi
  exit "${status}"
}
trap cleanup_privacy_sdk_guard_lock_state EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM

bash "${ROOT_DIR}/ci/check_privacy_js_sdk.sh"
assert_privacy_sdk_guard_lock_state
bash "${ROOT_DIR}/ci/check_privacy_python_sdk.sh"
assert_privacy_sdk_guard_lock_state
