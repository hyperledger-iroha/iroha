#!/usr/bin/env python3
"""Audit the native client paths required by every Taira release authority.

This is deliberately stronger than the old ``raise`` detector.  Readiness
requires the feature-gated native binary, the fixed client and exact eight-role
registry, and a role-correct preflight plus authenticated operation on every
release-critical Python path.
"""

from __future__ import annotations

import argparse
import ast
import json
import re
from collections.abc import Iterable, Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path


NATIVE_CLIENT_PATH = "scripts/taira_authority_client.py"
NATIVE_CARGO_PATH = "crates/irohad/Cargo.toml"
NATIVE_SOURCE_PATH = "crates/irohad/src/bin/taira_release_authority.rs"
NATIVE_FEATURE = "taira-authority-bin"
NATIVE_BINARY = "taira_release_authority"
NATIVE_MODULE = "external_software_signer::taira_authority"
FRAME_MAGIC = "IRTAUT01"

ROLE_REGISTRY = (
    "native-evidence",
    "privacy-protocol-origin",
    "privacy-governance",
    "qualification",
    "deploy-issuance",
    "rollout-observation",
    "public-soak-observation",
    "public-soak-replay-admission",
)

NATIVE_COMMANDS = (
    "provision",
    "serve",
    "assign-run",
    "authorize",
    "recover",
    "verify-receipt",
    "status",
    "rotate",
    "revoke",
)

FIXED_AUTHORITY_ROOTS = (
    "/etc/iroha/taira-authorities/v1",
    "/run/iroha/taira-authorities/v1",
    "/var/lib/iroha/taira-authorities/v1",
    "/private/etc/iroha/taira-authorities/v1",
    "/private/var/run/iroha/taira-authorities/v1",
    "/private/var/db/iroha/taira-authorities/v1",
)

CLIENT_METHOD_COMMANDS = {
    "preflight": "status",
    "authorize": "authorize",
    "verify_receipt": "verify-receipt",
}


@dataclass(frozen=True)
class RequiredCall:
    """One native-client operation reachable from a public entry point."""

    function: str
    method: str
    role: str


@dataclass(frozen=True)
class Prerequisite:
    """One legacy barrier and the authenticated paths which replace it."""

    path: str
    barrier: str
    stage: str
    roles: tuple[str, ...]
    calls: tuple[RequiredCall, ...]


PREREQUISITES = (
    Prerequisite(
        "scripts/taira_release_authority.py",
        "require_independent_native_evidence_authority_provisioned",
        "linux native-evidence authority",
        ("native-evidence",),
        (RequiredCall("build_authority", "authorize", "native-evidence"),),
    ),
    Prerequisite(
        "scripts/taira_privacy_protocol_receipt.py",
        "require_controller_origin_authority_provisioned",
        "privacy protocol origin authority",
        ("privacy-protocol-origin",),
        (
            RequiredCall(
                "validate_evidence_directory",
                "authorize",
                "privacy-protocol-origin",
            ),
        ),
    ),
    Prerequisite(
        "scripts/taira_privacy_governance_authority.py",
        "_require_provisioned_privacy_governance_authority_v1",
        "Exact12 governance authority",
        ("privacy-governance",),
        (
            RequiredCall(
                "request_authenticated_governance_transaction_v1",
                "authorize",
                "privacy-governance",
            ),
            RequiredCall(
                "validate_authenticated_governance_receipt_v1",
                "verify_receipt",
                "privacy-governance",
            ),
        ),
    ),
    Prerequisite(
        "scripts/build_privacy_v1_boi_handoff.py",
        "require_boi_qualification_isolation",
        "native qualification authority",
        ("qualification",),
        (RequiredCall("assemble_boi_handoff", "authorize", "qualification"),),
    ),
    Prerequisite(
        "scripts/deploy_taira_v21_reset.py",
        "require_deploy_issuance_contracts",
        "deployment issuance authority",
        ("deploy-issuance",),
        (RequiredCall("execute", "authorize", "deploy-issuance"),),
    ),
    Prerequisite(
        "scripts/build_taira_public_v2_deploy_handoff.py",
        "require_deploy_native_evidence_authority_provisioned",
        "post-deploy native-evidence authority",
    ),
    Prerequisite(
        "scripts/run_taira_public_v2_24h_soak.py",
        "require_public_soak_runner_authority_provisioned",
        "public 24-hour soak producer authority",
    ),
    Prerequisite(
        "scripts/taira_privacy_rollout_contract.py",
        "require_authenticated_rollout_observation_authority_provisioned",
        "rollout observation authority",
        ("rollout-observation",),
        (RequiredCall("validate_result", "authorize", "rollout-observation"),),
    ),
    Prerequisite(
        "scripts/taira_public_soak_authority_contract.py",
        "require_public_soak_authority_provisioned",
        "public-soak observation and replay authorities",
        ("public-soak-observation", "public-soak-replay-admission"),
        (
            RequiredCall(
                "consume_fresh_public_soak_admission",
                "verify_receipt",
                "public-soak-observation",
            ),
            RequiredCall(
                "consume_fresh_public_soak_admission",
                "authorize",
                "public-soak-replay-admission",
            ),
            RequiredCall(
                "verify_authenticated_public_soak_authority_envelope",
                "verify_receipt",
                "public-soak-observation",
            ),
            RequiredCall(
                "verify_authenticated_public_soak_authority_envelope",
                "verify_receipt",
                "public-soak-replay-admission",
            ),
        ),
    ),
)


class PrerequisiteError(RuntimeError):
    """An exact source contract could not be inspected."""


@dataclass(frozen=True)
class ParsedModule:
    tree: ast.Module
    functions: Mapping[str, ast.FunctionDef | ast.AsyncFunctionDef]
    strings: Mapping[str, str]


def _parse_module(path: Path) -> ParsedModule:
    try:
        tree = ast.parse(path.read_text(encoding="utf-8"), filename=str(path))
    except (OSError, UnicodeError, SyntaxError) as error:
        raise PrerequisiteError(f"cannot parse {path}: {error}") from error
    functions = {
        node.name: node
        for node in tree.body
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef))
    }
    strings: dict[str, str] = {}
    for node in tree.body:
        if not isinstance(node, (ast.Assign, ast.AnnAssign)):
            continue
        targets = node.targets if isinstance(node, ast.Assign) else [node.target]
        if isinstance(node.value, ast.Constant) and isinstance(node.value.value, str):
            for target in targets:
                if isinstance(target, ast.Name):
                    strings[target.id] = node.value.value
    return ParsedModule(tree, functions, strings)


def _body(function: ast.FunctionDef | ast.AsyncFunctionDef) -> list[ast.stmt]:
    body = list(function.body)
    if (
        body
        and isinstance(body[0], ast.Expr)
        and isinstance(body[0].value, ast.Constant)
        and isinstance(body[0].value.value, str)
    ):
        body.pop(0)
    return body


def is_unconditional_refusal(
    function: ast.FunctionDef | ast.AsyncFunctionDef,
) -> bool:
    """Return whether a function only discards inputs and raises/fails."""

    body = [node for node in _body(function) if not isinstance(node, ast.Delete)]
    if len(body) != 1:
        return False
    if isinstance(body[0], ast.Raise):
        return True
    if not isinstance(body[0], ast.Expr) or not isinstance(body[0].value, ast.Call):
        return False
    callee = body[0].value.func
    return isinstance(callee, ast.Name) and callee.id in {"fail", "_fail"}


def _assignment(module: ParsedModule, name: str) -> ast.expr | None:
    for node in module.tree.body:
        targets = node.targets if isinstance(node, ast.Assign) else (
            [node.target] if isinstance(node, ast.AnnAssign) else []
        )
        if any(isinstance(target, ast.Name) and target.id == name for target in targets):
            return node.value
    return None


def _string_sequence(value: ast.expr | None) -> tuple[str, ...] | None:
    if not isinstance(value, (ast.Tuple, ast.List)):
        return None
    if not all(
        isinstance(item, ast.Constant) and isinstance(item.value, str)
        for item in value.elts
    ):
        return None
    return tuple(item.value for item in value.elts)  # type: ignore[misc]


def _registry_roles(module: ParsedModule) -> tuple[str, ...] | None:
    value = _assignment(module, "ROLE_REGISTRY")
    if isinstance(value, ast.Dict) and all(
        isinstance(key, ast.Constant) and isinstance(key.value, str)
        for key in value.keys
    ):
        return tuple(key.value for key in value.keys)  # type: ignore[misc]
    if isinstance(value, ast.DictComp) and any(
        isinstance(node, ast.Name) and node.id == "ROLE_LABELS"
        for node in ast.walk(value)
    ):
        return _string_sequence(_assignment(module, "ROLE_LABELS"))
    return None


def _reachable(
    module: ParsedModule, root: str
) -> tuple[ast.FunctionDef | ast.AsyncFunctionDef, ...]:
    pending, visited, result = [root], set(), []
    while pending:
        name = pending.pop()
        function = module.functions.get(name)
        if function is None or name in visited:
            continue
        visited.add(name)
        result.append(function)
        pending.extend(
            node.func.id
            for node in ast.walk(function)
            if isinstance(node, ast.Call)
            and isinstance(node.func, ast.Name)
            and node.func.id in module.functions
        )
    return tuple(result)


def _role_argument(call: ast.Call, module: ParsedModule) -> str | None:
    value = call.args[0] if call.args else None
    for keyword in call.keywords:
        if keyword.arg == "role":
            value = keyword.value
    if isinstance(value, ast.Constant) and isinstance(value.value, str):
        return value.value
    if isinstance(value, ast.Name):
        return module.strings.get(value.id)
    return None


def _reaches_client_call(
    module: ParsedModule, *, function: str, method: str, role: str
) -> bool:
    for definition in _reachable(module, function):
        for node in ast.walk(definition):
            if not isinstance(node, ast.Call) or not isinstance(
                node.func, ast.Attribute
            ):
                continue
            if (
                isinstance(node.func.value, ast.Name)
                and node.func.value.id == "taira_authority_client"
                and node.func.attr == method
                and _role_argument(node, module) == role
            ):
                return True
    return False


def _reachable_literals(module: ParsedModule, function: str) -> set[str]:
    literals: set[str] = set()
    referenced: set[str] = set()
    for definition in _reachable(module, function):
        for node in ast.walk(definition):
            if isinstance(node, ast.Constant) and isinstance(node.value, str):
                literals.add(node.value)
            elif isinstance(node, ast.Name):
                referenced.add(node.id)
    literals.update(module.strings[name] for name in referenced & module.strings.keys())
    return literals


def _reaches_transport(module: ParsedModule, function: str) -> bool:
    native_calls = {"connect", "sendmsg", "recvmsg", "run", "Popen"}
    return any(
        isinstance(node.func, ast.Attribute) and node.func.attr in native_calls
        for definition in _reachable(module, function)
        for node in ast.walk(definition)
        if isinstance(node, ast.Call)
    )


def _report(
    kind: str,
    path: str,
    stage: str,
    detail: str,
    function: str | None = None,
    line: int | None = None,
) -> dict[str, object]:
    result: dict[str, object] = {
        "detail": detail,
        "kind": kind,
        "path": path,
        "stage": stage,
    }
    if function is not None:
        result["function"] = function
    if line is not None:
        result["line"] = line
    return result


def _audit_native(repository: Path) -> list[dict[str, object]]:
    stage = "native authority service"
    try:
        cargo = (repository / NATIVE_CARGO_PATH).read_text(encoding="utf-8")
    except (OSError, UnicodeError) as error:
        return [_report("native-service", NATIVE_CARGO_PATH, stage, str(error))]
    reports = []
    if re.search(rf"(?m)^\s*{re.escape(NATIVE_FEATURE)}\s*=", cargo) is None:
        reports.append(
            _report("native-feature", NATIVE_CARGO_PATH, stage, "feature is missing")
        )
    blocks = re.findall(r"(?ms)^\s*\[\[bin\]\]\s*(.*?)(?=^\s*\[\[|\Z)", cargo)
    block = next((item for item in blocks if NATIVE_BINARY in item), "")
    if NATIVE_FEATURE not in block:
        reports.append(
            _report(
                "native-binary",
                NATIVE_CARGO_PATH,
                stage,
                "authority binary is not gated by its required feature",
            )
        )
    try:
        source = (repository / NATIVE_SOURCE_PATH).read_text(encoding="utf-8")
    except (OSError, UnicodeError) as error:
        reports.append(_report("native-service", NATIVE_SOURCE_PATH, stage, str(error)))
        return reports
    for marker in (NATIVE_MODULE, FRAME_MAGIC):
        if marker not in source:
            reports.append(
                _report(
                    "native-service-contract",
                    NATIVE_SOURCE_PATH,
                    stage,
                    f"native source is missing {marker!r}",
                )
            )
    return reports


def _audit_client(repository: Path) -> list[dict[str, object]]:
    stage = "fixed native client"
    path = repository / NATIVE_CLIENT_PATH
    try:
        module = _parse_module(path)
        source = path.read_text(encoding="utf-8")
    except (OSError, UnicodeError, PrerequisiteError) as error:
        return [_report("native-client", NATIVE_CLIENT_PATH, stage, str(error))]
    reports = []
    roles = _registry_roles(module)
    if roles != ROLE_REGISTRY:
        reports.append(
            _report(
                "role-registry",
                NATIVE_CLIENT_PATH,
                stage,
                f"expected exact roles {list(ROLE_REGISTRY)}, found {list(roles or ())}",
            )
        )
    missing_roots = [root for root in FIXED_AUTHORITY_ROOTS if root not in source]
    if missing_roots:
        reports.append(
            _report(
                "fixed-roots",
                NATIVE_CLIENT_PATH,
                stage,
                f"missing fixed roots: {missing_roots}",
            )
        )
    if any(token in source for token in ("os.environ", "os.getenv", "getenv(", "argparse")):
        reports.append(
            _report(
                "client-override",
                NATIVE_CLIENT_PATH,
                stage,
                "fixed client exposes an environment or CLI override",
            )
        )
    for method, command in CLIENT_METHOD_COMMANDS.items():
        function = module.functions.get(method)
        if function is None:
            reports.append(
                _report(
                    "native-client-method", NATIVE_CLIENT_PATH, stage, "missing method", method
                )
            )
            continue
        if is_unconditional_refusal(function) or not _reaches_transport(module, method):
            reports.append(
                _report(
                    "native-client-transport",
                    NATIVE_CLIENT_PATH,
                    stage,
                    "method does not reach native transport",
                    method,
                    function.lineno,
                )
            )
        if command not in _reachable_literals(module, method):
            reports.append(
                _report(
                    "native-client-command",
                    NATIVE_CLIENT_PATH,
                    stage,
                    f"method does not reach {command!r}",
                    method,
                    function.lineno,
                )
            )
    return reports


def unresolved_prerequisites(
    repository: Path,
    prerequisites: Iterable[Prerequisite] = PREREQUISITES,
) -> list[dict[str, object]]:
    """Return every incomplete native, registry, preflight, and operation path."""

    reports = [*_audit_native(repository), *_audit_client(repository)]
    for prerequisite in prerequisites:
        try:
            module = _parse_module(repository / prerequisite.path)
        except PrerequisiteError as error:
            reports.append(
                _report(
                    "python-authority",
                    prerequisite.path,
                    prerequisite.stage,
                    str(error),
                )
            )
            continue
        barrier = module.functions.get(prerequisite.barrier)
        if barrier is None:
            reports.append(
                _report(
                    "python-authority",
                    prerequisite.path,
                    prerequisite.stage,
                    "compatibility barrier is missing",
                    prerequisite.barrier,
                )
            )
            continue
        if is_unconditional_refusal(barrier):
            reports.append(
                _report(
                    "unconditional-refusal",
                    prerequisite.path,
                    prerequisite.stage,
                    "compatibility barrier remains an unconditional refusal",
                    prerequisite.barrier,
                    barrier.lineno,
                )
            )
        for role in prerequisite.roles:
            if not _reaches_client_call(
                module,
                function=prerequisite.barrier,
                method="preflight",
                role=role,
            ):
                reports.append(
                    _report(
                        "authority-preflight",
                        prerequisite.path,
                        prerequisite.stage,
                        f"barrier does not authenticate role {role!r}",
                        prerequisite.barrier,
                        barrier.lineno,
                    )
                )
        for required in prerequisite.calls:
            function = module.functions.get(required.function)
            if function is None or not _reaches_client_call(
                module,
                function=required.function,
                method=required.method,
                role=required.role,
            ):
                reports.append(
                    _report(
                        "authority-call-path",
                        prerequisite.path,
                        prerequisite.stage,
                        f"{required.function} does not reach {required.method} "
                        f"for role {required.role!r}",
                        required.function,
                        function.lineno if function is not None else None,
                    )
                )
    return reports


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--repository",
        type=Path,
        default=Path(__file__).resolve().parents[1],
        help="repository root (defaults to the parent of scripts/)",
    )
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        repository = args.repository.resolve(strict=True)
        unresolved = unresolved_prerequisites(repository)
    except (OSError, PrerequisiteError) as error:
        print(json.dumps({"error": str(error), "ready": False}, sort_keys=True))
        return 2
    report = {
        "ready": not unresolved,
        "role_count": len(ROLE_REGISTRY),
        "roles": list(ROLE_REGISTRY),
        "unresolved": unresolved,
    }
    print(json.dumps(report, sort_keys=True))
    return 0 if not unresolved else 1


if __name__ == "__main__":
    raise SystemExit(main())
