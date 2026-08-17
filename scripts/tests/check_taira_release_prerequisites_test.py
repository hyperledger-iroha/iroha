"""Tests for the complete Taira authority source-prerequisite audit."""

from __future__ import annotations

import ast
import json
import subprocess
import sys
from pathlib import Path

from scripts import check_taira_release_prerequisites as readiness

REPO_ROOT = Path(__file__).resolve().parents[2]


def _function(source: str) -> ast.FunctionDef:
    node = ast.parse(source).body[0]
    assert isinstance(node, ast.FunctionDef)
    return node


def _write(path: Path, payload: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(payload, encoding="utf-8")


def _complete_fixture(root: Path) -> None:
    roles = "\n".join(
        f'    "{role}": object(),' for role in readiness.ROLE_REGISTRY
    )
    roots = "\n".join(repr(path) for path in readiness.FIXED_AUTHORITY_ROOTS)
    _write(
        root / readiness.NATIVE_CLIENT_PATH,
        f'''import subprocess

ROLE_REGISTRY = {{
{roles}
}}
FIXED_ROOTS = ({roots})
NATIVE_VERIFIER = "/usr/libexec/iroha/taira_release_authority"

def _invoke_native_client(command, role):
    return subprocess.run((NATIVE_VERIFIER, command, "--role", role), check=True)

def preflight(role):
    return _invoke_native_client("status", role)

def authorize(role, request=b"", artifacts=()):
    return _invoke_native_client("authorize", role)

def verify_receipt(role, request=b"", receipt=b"", artifacts=(), historical=False):
    return _invoke_native_client("verify-receipt", role)
''',
    )
    _write(
        root / readiness.NATIVE_CARGO_PATH,
        f'''[features]
{readiness.NATIVE_FEATURE} = []

[[bin]]
name = "{readiness.NATIVE_BINARY}"
path = "src/bin/{readiness.NATIVE_BINARY}.rs"
required-features = ["{readiness.NATIVE_FEATURE}"]
''',
    )
    _write(
        root / readiness.NATIVE_SOURCE_PATH,
        f"fn main() {{ {readiness.NATIVE_MODULE}::run_cli(); }}\n",
    )
    role_variants = "\n".join(
        f"    {variant} = {index},"
        for index, variant in enumerate(readiness.NATIVE_ROLE_VARIANTS, 1)
    )
    role_labels = "\n".join(f'// {role}' for role in readiness.ROLE_REGISTRY)
    _write(
        root / readiness.NATIVE_PROTOCOL_PATH,
        f'''const MAGIC: &str = "{readiness.FRAME_MAGIC}";
pub enum TairaAuthorityRoleV1 {{
{role_variants}
}}
{role_labels}
''',
    )
    command_variants = "\n".join(
        f"    {variant}(Args)," for variant in readiness.NATIVE_COMMAND_VARIANTS
    )
    _write(
        root / readiness.NATIVE_TRANSPORT_PATH,
        f'''enum Command {{
{command_variants}
}}
''',
    )
    _write(
        root / readiness.NATIVE_SERVICE_PATH,
        '''impl Authority {
    fn authorize_json(&self, request: Request) {
        if request.deploy_disposition == Some(DeployDispositionV1::Finalize) {
            self.finalize_deployment();
        }
        if self.role == TairaAuthorityRoleV1::PrivacyGovernance {
            privacy_governance::validate_assigned_privacy_governance_request_v1();
        }
        match self.role {
            TairaAuthorityRoleV1::NativeEvidence => {
                native_evidence::validate_native_evidence_v1();
            }
            TairaAuthorityRoleV1::PrivacyProtocolOrigin => {
                privacy_protocol_origin::validate_privacy_protocol_origin_v1();
            }
            TairaAuthorityRoleV1::Qualification => {
                sandbox::run_qualification_probes();
            }
            TairaAuthorityRoleV1::RolloutObservation => {
                rollout_observation::validate_rollout_observation_subject_v1();
            }
            TairaAuthorityRoleV1::PublicSoakObservation => {
                self.issue_public_soak_observation();
            }
            TairaAuthorityRoleV1::PublicSoakReplayAdmission => {
                self.issue_public_soak_replay_admission();
            }
            _ => {}
        }
    }
}
''',
    )

    for prerequisite in readiness.PREREQUISITES:
        grouped: dict[str, list[readiness.RequiredCall]] = {}
        for required in prerequisite.calls:
            grouped.setdefault(required.function, []).append(required)
        lines = ["from . import taira_authority_client", ""]
        lines.append(f"def {prerequisite.barrier}():")
        for role in prerequisite.roles:
            lines.append(f'    taira_authority_client.preflight("{role}")')
        lines.append("    return None")
        for function, calls in grouped.items():
            lines.extend(("", f"def {function}():", f"    {prerequisite.barrier}()"))
            for required in calls:
                lines.append(
                    f'    taira_authority_client.{required.method}('
                    f'"{required.role}", b"request")'
                )
            lines.append("    return None")
        _write(root / prerequisite.path, "\n".join(lines) + "\n")


def test_unconditional_refusal_detection_is_narrow() -> None:
    """Only a docstring/input discard followed by raise or fail is classified."""

    assert readiness.is_unconditional_refusal(
        _function(
            'def gate(value):\n    """closed"""\n    del value\n    _fail("no")\n'
        )
    )
    assert readiness.is_unconditional_refusal(
        _function('def gate():\n    raise RuntimeError("no")\n')
    )
    assert not readiness.is_unconditional_refusal(
        _function(
            'def gate(role):\n    client.preflight(role)\n    return role\n'
        )
    )


def test_registry_is_the_exact_eight_role_contract() -> None:
    """The source audit names all isolated roles without legacy aliases."""

    assert readiness.ROLE_REGISTRY == (
        "native-evidence",
        "privacy-protocol-origin",
        "privacy-governance",
        "qualification",
        "deploy-issuance",
        "rollout-observation",
        "public-soak-observation",
        "public-soak-replay-admission",
    )
    assert len({role for role in readiness.ROLE_REGISTRY}) == 8


def test_qualification_prerequisite_uses_neutral_call_roots() -> None:
    """The source audit follows the public qualification definitions."""

    prerequisite = readiness.PREREQUISITES[3]
    assert prerequisite.barrier == "require_native_qualification_isolation"
    assert prerequisite.calls == (
        readiness.RequiredCall(
            "assemble_qualification_handoff", "authorize", "qualification"
        ),
    )


def test_complete_native_service_client_and_call_paths_are_ready(
    tmp_path: Path,
) -> None:
    """A complete fixture passes every native and Python call-path check."""

    _complete_fixture(tmp_path)
    assert readiness.unresolved_prerequisites(tmp_path) == []


def test_validator_names_without_authorize_dispatch_never_turn_green(
    tmp_path: Path,
) -> None:
    """Loose names cannot substitute for concrete role-guarded Rust calls."""

    _complete_fixture(tmp_path)
    _write(
        tmp_path / readiness.NATIVE_SERVICE_PATH,
        "\n".join(readiness.NATIVE_ROLE_VALIDATORS.values()) + "\n",
    )
    reports = readiness.unresolved_prerequisites(tmp_path)
    missing = {
        str(report["detail"])
        for report in reports
        if report["kind"] == "native-role-validator"
    }
    assert len(missing) == len(readiness.ROLE_REGISTRY)


def test_authorize_validators_cannot_be_cross_wired_between_roles(
    tmp_path: Path,
) -> None:
    _complete_fixture(tmp_path)
    service = tmp_path / readiness.NATIVE_SERVICE_PATH
    source = service.read_text(encoding="utf-8")
    native_call = "native_evidence::validate_native_evidence_v1();"
    origin_call = (
        "privacy_protocol_origin::validate_privacy_protocol_origin_v1();"
    )
    source = source.replace(native_call, "swapped_validator();", 1)
    source = source.replace(origin_call, native_call, 1)
    source = source.replace("swapped_validator();", origin_call, 1)
    _write(service, source)

    reports = readiness.unresolved_prerequisites(tmp_path)
    missing = {
        str(report["detail"])
        for report in reports
        if report["kind"] == "native-role-validator"
    }
    assert any("native-evidence" in detail for detail in missing)
    assert any("privacy-protocol-origin" in detail for detail in missing)


def test_rust_markers_in_comments_and_strings_are_not_structural_items() -> None:
    source = '''
// enum TairaAuthorityRoleV1 { NativeEvidence, }
const CLAIM: &str = "enum TairaAuthorityRoleV1 { NativeEvidence, }";
/* enum TairaAuthorityRoleV1 { NativeEvidence, } */
'''
    assert readiness._rust_enum_variants(source, "TairaAuthorityRoleV1") is None


def test_removing_a_raise_without_wiring_the_client_never_turns_green(
    tmp_path: Path,
) -> None:
    """A non-refusing no-op remains unresolved when native calls are absent."""

    _complete_fixture(tmp_path)
    prerequisite = readiness.PREREQUISITES[0]
    _write(
        tmp_path / prerequisite.path,
        f'''def {prerequisite.barrier}():
    return None

def {prerequisite.calls[0].function}():
    {prerequisite.barrier}()
    return None
''',
    )
    reports = readiness.unresolved_prerequisites(tmp_path)
    assert any(report["kind"] == "authority-preflight" for report in reports)
    assert any(report["kind"] == "authority-call-path" for report in reports)


def test_statically_dead_client_calls_never_turn_green(tmp_path: Path) -> None:
    """Calls hidden under a literal-false branch are not executable paths."""

    _complete_fixture(tmp_path)
    prerequisite = readiness.PREREQUISITES[0]
    path = tmp_path / prerequisite.path
    source = path.read_text(encoding="utf-8")
    source = source.replace(
        '    taira_authority_client.preflight("native-evidence")',
        '    if False:\n        taira_authority_client.preflight("native-evidence")',
        1,
    )
    _write(path, source)
    reports = readiness.unresolved_prerequisites(tmp_path)
    assert any(report["kind"] == "authority-preflight" for report in reports)


def test_native_role_command_and_validator_registries_are_required(
    tmp_path: Path,
) -> None:
    """A wrapper with marker comments cannot stand in for the native service."""

    _complete_fixture(tmp_path)
    protocol = tmp_path / readiness.NATIVE_PROTOCOL_PATH
    _write(
        protocol,
        protocol.read_text(encoding="utf-8").replace(
            "    RolloutObservation = 6,\n", "", 1
        ),
    )
    transport = tmp_path / readiness.NATIVE_TRANSPORT_PATH
    _write(
        transport,
        transport.read_text(encoding="utf-8").replace(
            "    VerifyReceipt(Args),\n", "", 1
        ),
    )
    service = tmp_path / readiness.NATIVE_SERVICE_PATH
    _write(
        service,
        service.read_text(encoding="utf-8").replace(
            readiness.NATIVE_ROLE_VALIDATORS["privacy-protocol-origin"], "", 1
        ),
    )
    kinds = {report["kind"] for report in readiness.unresolved_prerequisites(tmp_path)}
    assert {
        "native-role-registry",
        "native-command-registry",
        "native-role-validator",
    } <= kinds


def test_missing_role_and_transport_are_both_fail_closed(tmp_path: Path) -> None:
    """Registry completeness and actual native execution are independent gates."""

    _complete_fixture(tmp_path)
    client = tmp_path / readiness.NATIVE_CLIENT_PATH
    source = client.read_text(encoding="utf-8")
    source = source.replace('    "qualification": object(),\n', "")
    source = source.replace(
        '    return subprocess.run((NATIVE_VERIFIER, command, "--role", role), check=True)',
        "    return (command, role)",
    )
    _write(client, source)
    reports = readiness.unresolved_prerequisites(tmp_path)
    assert any(report["kind"] == "role-registry" for report in reports)
    transport = [
        report for report in reports if report["kind"] == "native-client-transport"
    ]
    assert {report["function"] for report in transport} == {
        "preflight",
        "authorize",
        "verify_receipt",
    }


def test_each_operation_and_role_is_checked_individually(tmp_path: Path) -> None:
    """Mutating one expected role at one call site is reported precisely."""

    _complete_fixture(tmp_path)
    prerequisite = readiness.PREREQUISITES[-1]
    path = tmp_path / prerequisite.path
    source = path.read_text(encoding="utf-8")
    source = source.replace(
        'taira_authority_client.authorize('
        '"public-soak-replay-admission", b"request")',
        'taira_authority_client.authorize('
        '"public-soak-observation", b"request")',
        1,
    )
    _write(path, source)
    reports = readiness.unresolved_prerequisites(tmp_path)
    matching = [
        report
        for report in reports
        if report["kind"] == "authority-call-path"
        and report["function"] == "consume_fresh_public_soak_admission"
    ]
    assert len(matching) == 1
    assert "public-soak-replay-admission" in str(matching[0]["detail"])


def test_current_repository_implements_every_release_prerequisite() -> None:
    """The checked-out implementation must report ready, not merely non-refusing."""

    assert readiness.unresolved_prerequisites(REPO_ROOT) == []


def test_cli_is_machine_readable_and_ready() -> None:
    """The release workflow receives exact eight-role readiness JSON."""

    result = subprocess.run(
        [
            sys.executable,
            str(REPO_ROOT / "scripts/check_taira_release_prerequisites.py"),
            "--repository",
            str(REPO_ROOT),
        ],
        check=False,
        capture_output=True,
        text=True,
    )
    payload = json.loads(result.stdout)
    assert result.returncode == 0
    assert payload == {
        "ready": True,
        "role_count": 8,
        "roles": list(readiness.ROLE_REGISTRY),
        "unresolved": [],
    }
