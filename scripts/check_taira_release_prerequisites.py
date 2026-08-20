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
NATIVE_PROTOCOL_PATH = (
    "crates/irohad/src/external_software_signer/taira_authority/protocol.rs"
)
NATIVE_TRANSPORT_PATH = (
    "crates/irohad/src/external_software_signer/taira_authority/transport.rs"
)
NATIVE_SERVICE_PATH = (
    "crates/irohad/src/external_software_signer/taira_authority/service.rs"
)
NATIVE_FEATURE = "taira-authority-bin"
NATIVE_BINARY = "taira_release_authority"
NATIVE_MODULE = "external_software_signer::taira_authority"
FRAME_MAGIC = "IRTAUT01"
INSTALLED_CONTROLLER_PATH = "scripts/seal_taira_release_controllers.py"
INSTALLED_CONTROLLER_STAGE = "installed release controller"
CONTROLLER_BLOCKED_OPERATIONS = ("assemble-boi", "deploy-reset")
CONTROLLER_REQUIRED_MACOS_FILES = (
    "scripts/check_taira_public_v2_24h_soak_evidence.py",
    "scripts/taira_public_soak_authority_contract.py",
)

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

NATIVE_ROLE_VARIANTS = (
    "NativeEvidence",
    "PrivacyProtocolOrigin",
    "PrivacyGovernance",
    "Qualification",
    "DeployIssuance",
    "RolloutObservation",
    "PublicSoakObservation",
    "PublicSoakReplayAdmission",
)

NATIVE_COMMAND_VARIANTS = (
    "Provision",
    "Serve",
    "AssignRun",
    "Authorize",
    "Recover",
    "VerifyReceipt",
    "Status",
    "Rotate",
    "Revoke",
)

NATIVE_ROLE_VALIDATORS = {
    "native-evidence": "validate_native_evidence_v1",
    "privacy-protocol-origin": "validate_privacy_protocol_origin_v1",
    "privacy-governance": "validate_assigned_privacy_governance_request_v1",
    "qualification": "run_qualification_probes",
    "deploy-issuance": "finalize_deployment",
    "rollout-observation": "validate_rollout_observation_subject_v1",
    "public-soak-observation": "issue_public_soak_observation",
    "public-soak-replay-admission": "issue_public_soak_replay_admission",
}

NATIVE_ROLE_VARIANT_BY_LABEL = dict(zip(ROLE_REGISTRY, NATIVE_ROLE_VARIANTS))

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
        "require_native_qualification_isolation",
        "native qualification authority",
        ("qualification",),
        (
            RequiredCall(
                "assemble_qualification_handoff", "authorize", "qualification"
            ),
        ),
    ),
    Prerequisite(
        "scripts/deploy_taira_v21_reset.py",
        "require_deploy_issuance_contracts",
        "deployment issuance authority",
        ("deploy-issuance",),
        (RequiredCall("execute", "authorize", "deploy-issuance"),),
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


def _constant_truth(value: ast.expr) -> bool | None:
    if isinstance(value, ast.Constant) and value.value in (None, True, False, 0, 1):
        return bool(value.value)
    return None


def _live_nodes(
    function: ast.FunctionDef | ast.AsyncFunctionDef,
) -> tuple[ast.AST, ...]:
    """Walk executable nodes while pruning statically dead literal branches."""

    nodes: list[ast.AST] = []

    def visit(node: ast.AST) -> None:
        nodes.append(node)
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef, ast.Lambda)):
            return
        if isinstance(node, ast.If):
            visit(node.test)
            truth = _constant_truth(node.test)
            selected = node.body if truth is True else node.orelse if truth is False else None
            for child in selected if selected is not None else (*node.body, *node.orelse):
                visit(child)
            return
        if isinstance(node, ast.While):
            visit(node.test)
            truth = _constant_truth(node.test)
            selected = node.orelse if truth is False else (*node.body, *node.orelse)
            for child in selected:
                visit(child)
            return
        for child in ast.iter_child_nodes(node):
            visit(child)

    for statement in function.body:
        visit(statement)
    return tuple(nodes)


def _statements_unconditionally_refuse(body: Sequence[ast.stmt]) -> bool:
    """Return whether statements only discard inputs and raise/fail."""

    body = [node for node in body if not isinstance(node, ast.Delete)]
    if len(body) != 1:
        return False
    if isinstance(body[0], ast.Raise):
        return True
    if not isinstance(body[0], ast.Expr) or not isinstance(body[0].value, ast.Call):
        return False
    callee = body[0].value.func
    return isinstance(callee, ast.Name) and callee.id in {"fail", "_fail"}


def is_unconditional_refusal(
    function: ast.FunctionDef | ast.AsyncFunctionDef,
) -> bool:
    """Return whether a function only discards inputs and raises/fails."""

    return _statements_unconditionally_refuse(_body(function))


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


def _static_string_sequence(
    module: ParsedModule,
    value: ast.expr | None,
    seen: frozenset[str] = frozenset(),
) -> tuple[str, ...] | None:
    """Resolve a static string sequence assembled from names and tuple addition."""

    direct = _string_sequence(value)
    if direct is not None:
        return direct
    if isinstance(value, ast.Name) and value.id not in seen:
        return _static_string_sequence(
            module,
            _assignment(module, value.id),
            seen | {value.id},
        )
    if isinstance(value, ast.BinOp) and isinstance(value.op, ast.Add):
        left = _static_string_sequence(module, value.left, seen)
        right = _static_string_sequence(module, value.right, seen)
        if left is not None and right is not None:
            return left + right
    return None


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
            for node in _live_nodes(function)
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
        for node in _live_nodes(definition):
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
        for node in _live_nodes(definition):
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
        for node in _live_nodes(definition)
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


def _rust_tokens(source: str) -> tuple[str, ...]:
    """Lex enough Rust to inspect items without trusting comments or strings."""

    tokens: list[str] = []
    index = 0
    length = len(source)
    while index < length:
        character = source[index]
        if character.isspace():
            index += 1
            continue
        if source.startswith("//", index):
            newline = source.find("\n", index + 2)
            index = length if newline < 0 else newline + 1
            continue
        if source.startswith("/*", index):
            depth = 1
            index += 2
            while index < length and depth:
                if source.startswith("/*", index):
                    depth += 1
                    index += 2
                elif source.startswith("*/", index):
                    depth -= 1
                    index += 2
                else:
                    index += 1
            continue

        raw = re.match(r'(?:br|r)(?P<hashes>#{0,255})"', source[index:])
        if raw is not None and (
            index == 0 or not (source[index - 1].isalnum() or source[index - 1] == "_")
        ):
            terminator = '"' + raw.group("hashes")
            payload_start = index + raw.end()
            payload_end = source.find(terminator, payload_start)
            index = length if payload_end < 0 else payload_end + len(terminator)
            continue
        if character == '"' or (
            character in {"b", "c"} and index + 1 < length and source[index + 1] == '"'
        ):
            index += 2 if character in {"b", "c"} else 1
            while index < length:
                if source[index] == "\\":
                    index += 2
                elif source[index] == '"':
                    index += 1
                    break
                else:
                    index += 1
            continue
        if character.isalpha() or character == "_":
            end = index + 1
            while end < length and (source[end].isalnum() or source[end] == "_"):
                end += 1
            tokens.append(source[index:end])
            index = end
            continue
        if character.isdigit():
            end = index + 1
            while end < length and (source[end].isalnum() or source[end] == "_"):
                end += 1
            tokens.append(source[index:end])
            index = end
            continue
        operator = next(
            (
                candidate
                for candidate in ("::", "=>", "==", "!=", "&&", "||", "->")
                if source.startswith(candidate, index)
            ),
            None,
        )
        if operator is not None:
            tokens.append(operator)
            index += len(operator)
            continue
        tokens.append(character)
        index += 1
    return tuple(tokens)


def _matching_token(
    tokens: Sequence[str], start: int, opening: str, closing: str
) -> int | None:
    if start >= len(tokens) or tokens[start] != opening:
        return None
    depth = 0
    for index in range(start, len(tokens)):
        if tokens[index] == opening:
            depth += 1
        elif tokens[index] == closing:
            depth -= 1
            if depth == 0:
                return index
    return None


def _rust_item_body(
    source: str, keyword: str, item_name: str
) -> tuple[str, ...] | None:
    tokens = _rust_tokens(source)
    for index in range(len(tokens) - 1):
        if tokens[index] != keyword or tokens[index + 1] != item_name:
            continue
        try:
            opening = tokens.index("{", index + 2)
        except ValueError:
            return None
        if ";" in tokens[index + 2 : opening]:
            continue
        closing = _matching_token(tokens, opening, "{", "}")
        if closing is not None:
            return tokens[opening + 1 : closing]
    return None


def _split_top_level(tokens: Sequence[str], delimiter: str) -> tuple[tuple[str, ...], ...]:
    pairs = {"(": ")", "[": "]", "{": "}"}
    stack: list[str] = []
    chunks: list[tuple[str, ...]] = []
    start = 0
    for index, token in enumerate(tokens):
        if token in pairs:
            stack.append(pairs[token])
        elif stack and token == stack[-1]:
            stack.pop()
        elif token == delimiter and not stack:
            chunks.append(tuple(tokens[start:index]))
            start = index + 1
    if start < len(tokens):
        chunks.append(tuple(tokens[start:]))
    return tuple(chunk for chunk in chunks if chunk)


def _rust_enum_variants(source: str, enum_name: str) -> tuple[str, ...] | None:
    body = _rust_item_body(source, "enum", enum_name)
    if body is None:
        return None
    variants: list[str] = []
    for declaration in _split_top_level(body, ","):
        variant = next(
            (
                token
                for token in declaration
                if token and token[0].isupper() and token.replace("_", "").isalnum()
            ),
            None,
        )
        if variant is None:
            return None
        variants.append(variant)
    return tuple(variants)


def _contains_tokens(tokens: Sequence[str], expected: Sequence[str]) -> bool:
    width = len(expected)
    return any(
        tuple(tokens[index : index + width]) == tuple(expected)
        for index in range(len(tokens) - width + 1)
    )


def _called_rust_functions(tokens: Sequence[str]) -> set[str]:
    return {
        token
        for index, token in enumerate(tokens[:-1])
        if (token[0].isalpha() or token.startswith("_")) and tokens[index + 1] == "("
    }


def _role_variants(tokens: Sequence[str]) -> set[str]:
    variants: set[str] = set()
    for index in range(len(tokens) - 2):
        if tokens[index : index + 2] == ("TairaAuthorityRoleV1", "::"):
            variants.add(tokens[index + 2])
    return variants


def _top_level_arrow(tokens: Sequence[str]) -> int | None:
    stack: list[str] = []
    pairs = {"(": ")", "[": "]", "{": "}"}
    for index, token in enumerate(tokens):
        if token in pairs:
            stack.append(pairs[token])
        elif stack and token == stack[-1]:
            stack.pop()
        elif token == "=>" and not stack:
            return index
    return None


def _rust_match_arms(
    tokens: Sequence[str],
) -> tuple[tuple[tuple[str, ...], tuple[str, ...]], ...]:
    arms: list[tuple[tuple[str, ...], tuple[str, ...]]] = []
    cursor = 0
    while cursor < len(tokens):
        while cursor < len(tokens) and tokens[cursor] == ",":
            cursor += 1
        arrow_offset = _top_level_arrow(tokens[cursor:])
        if arrow_offset is None:
            break
        arrow = cursor + arrow_offset
        pattern = tuple(tokens[cursor:arrow])
        expression_start = arrow + 1
        if expression_start >= len(tokens):
            break
        if tokens[expression_start] == "{":
            expression_end = _matching_token(tokens, expression_start, "{", "}")
            if expression_end is None:
                break
            expression = tuple(tokens[expression_start : expression_end + 1])
            cursor = expression_end + 1
        else:
            stack: list[str] = []
            pairs = {"(": ")", "[": "]", "{": "}"}
            expression_end = len(tokens)
            for index in range(expression_start, len(tokens)):
                token = tokens[index]
                if token in pairs:
                    stack.append(pairs[token])
                elif stack and token == stack[-1]:
                    stack.pop()
                elif token == "," and not stack:
                    expression_end = index
                    break
            expression = tuple(tokens[expression_start:expression_end])
            cursor = expression_end + 1
        arms.append((pattern, expression))
    return tuple(arms)


def _authorize_role_dispatches(source: str) -> Mapping[str, set[str]]:
    """Return calls protected by concrete role/disposition branches in authorize."""

    body = _rust_item_body(source, "fn", "authorize_json")
    if body is None:
        return {}
    dispatches: dict[str, set[str]] = {}

    # Calls in `match self.role` arms are associated with the exact arm variants.
    for index, token in enumerate(body):
        if token != "match":
            continue
        try:
            opening = body.index("{", index + 1)
        except ValueError:
            continue
        if not _contains_tokens(body[index + 1 : opening], ("self", ".", "role")):
            continue
        closing = _matching_token(body, opening, "{", "}")
        if closing is None:
            continue
        for pattern, expression in _rust_match_arms(body[opening + 1 : closing]):
            calls = _called_rust_functions(expression)
            for variant in _role_variants(pattern):
                dispatches.setdefault(variant, set()).update(calls)

    # Role-specific `if` blocks cover governance and replay pre-validation.
    # The deployment finalizer is guarded by its typed disposition, whose parser
    # accepts that field only for the deployment role.
    for index, token in enumerate(body):
        if token != "if":
            continue
        opening = None
        stack: list[str] = []
        pairs = {"(": ")", "[": "]"}
        for candidate in range(index + 1, len(body)):
            current = body[candidate]
            if current in pairs:
                stack.append(pairs[current])
            elif stack and current == stack[-1]:
                stack.pop()
            elif current == "{" and not stack:
                opening = candidate
                break
        if opening is None:
            continue
        closing = _matching_token(body, opening, "{", "}")
        if closing is None:
            continue
        condition = body[index + 1 : opening]
        calls = _called_rust_functions(body[opening + 1 : closing])
        if _contains_tokens(condition, ("self", ".", "role")):
            for variant in _role_variants(condition):
                if not _contains_tokens(
                    condition,
                    (
                        "self",
                        ".",
                        "role",
                        "==",
                        "TairaAuthorityRoleV1",
                        "::",
                        variant,
                    ),
                ):
                    continue
                dispatches.setdefault(variant, set()).update(calls)
        if _contains_tokens(condition, ("DeployDispositionV1", "::", "Finalize")):
            dispatches.setdefault("DeployIssuance", set()).update(calls)
    return dispatches


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
    for marker in (NATIVE_MODULE, "run_cli()"):
        if marker not in source:
            reports.append(
                _report(
                    "native-service-contract",
                    NATIVE_SOURCE_PATH,
                    stage,
                    f"native source is missing {marker!r}",
                )
            )
    try:
        protocol = (repository / NATIVE_PROTOCOL_PATH).read_text(encoding="utf-8")
        transport = (repository / NATIVE_TRANSPORT_PATH).read_text(encoding="utf-8")
        service = (repository / NATIVE_SERVICE_PATH).read_text(encoding="utf-8")
    except (OSError, UnicodeError) as error:
        reports.append(_report("native-service", NATIVE_PROTOCOL_PATH, stage, str(error)))
        return reports
    variants = _rust_enum_variants(protocol, "TairaAuthorityRoleV1")
    if variants != NATIVE_ROLE_VARIANTS:
        reports.append(
            _report(
                "native-role-registry",
                NATIVE_PROTOCOL_PATH,
                stage,
                f"expected exact native roles {list(ROLE_REGISTRY)}",
            )
        )
    if FRAME_MAGIC not in protocol:
        reports.append(
            _report(
                "native-frame",
                NATIVE_PROTOCOL_PATH,
                stage,
                f"native protocol is missing {FRAME_MAGIC!r}",
            )
        )
    commands = _rust_enum_variants(transport, "Command")
    if commands is None or any(command not in commands for command in NATIVE_COMMAND_VARIANTS):
        reports.append(
            _report(
                "native-command-registry",
                NATIVE_TRANSPORT_PATH,
                stage,
                f"required commands are missing: {list(NATIVE_COMMANDS)}",
            )
        )
    dispatches = _authorize_role_dispatches(service)
    for role, validator in NATIVE_ROLE_VALIDATORS.items():
        variant = NATIVE_ROLE_VARIANT_BY_LABEL[role]
        if validator not in dispatches.get(variant, set()):
            reports.append(
                _report(
                    "native-role-validator",
                    NATIVE_SERVICE_PATH,
                    stage,
                    f"authorize_json does not dispatch role {role!r} to {validator}",
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


def _is_args_string_equality(
    expression: ast.expr, attribute: str, expected: str
) -> bool:
    if not (
        isinstance(expression, ast.Compare)
        and len(expression.ops) == 1
        and isinstance(expression.ops[0], ast.Eq)
        and len(expression.comparators) == 1
    ):
        return False
    left, right = expression.left, expression.comparators[0]
    return any(
        isinstance(candidate, ast.Attribute)
        and isinstance(candidate.value, ast.Name)
        and candidate.value.id == "args"
        and candidate.attr == attribute
        and isinstance(value, ast.Constant)
        and value.value == expected
        for candidate, value in ((left, right), (right, left))
    )


def _and_terms(expression: ast.expr) -> tuple[ast.expr, ...]:
    if isinstance(expression, ast.BoolOp) and isinstance(expression.op, ast.And):
        return tuple(
            term for value in expression.values for term in _and_terms(value)
        )
    return (expression,)


def _is_run_operation_guard(expression: ast.expr, operation: str) -> bool:
    terms = _and_terms(expression)
    return any(
        _is_args_string_equality(term, "command", "run") for term in terms
    ) and any(
        _is_args_string_equality(term, "operation", operation) for term in terms
    )


def _audit_sealed_controller(repository: Path) -> list[dict[str, object]]:
    path = repository / INSTALLED_CONTROLLER_PATH
    try:
        module = _parse_module(path)
    except PrerequisiteError as error:
        return [
            _report(
                "installed-controller",
                INSTALLED_CONTROLLER_PATH,
                INSTALLED_CONTROLLER_STAGE,
                str(error),
            )
        ]

    reports: list[dict[str, object]] = []
    main = module.functions.get("main")
    if main is None:
        reports.append(
            _report(
                "installed-controller",
                INSTALLED_CONTROLLER_PATH,
                INSTALLED_CONTROLLER_STAGE,
                "installed controller main is missing",
                "main",
            )
        )
    else:
        for operation in CONTROLLER_BLOCKED_OPERATIONS:
            refusals = [
                node
                for node in _live_nodes(main)
                if isinstance(node, ast.If)
                and _is_run_operation_guard(node.test, operation)
                and _statements_unconditionally_refuse(node.body)
            ]
            for refusal in refusals:
                reports.append(
                    _report(
                        "controller-unconditional-refusal",
                        INSTALLED_CONTROLLER_PATH,
                        INSTALLED_CONTROLLER_STAGE,
                        f"main refuses operation {operation!r} before attestation",
                        "main",
                        refusal.lineno,
                    )
                )

    boi_dispatch = module.functions.get("_dispatch_boi_composite")
    if boi_dispatch is None:
        reports.append(
            _report(
                "installed-controller",
                INSTALLED_CONTROLLER_PATH,
                INSTALLED_CONTROLLER_STAGE,
                "BOI composite dispatcher is missing",
                "_dispatch_boi_composite",
            )
        )
    elif is_unconditional_refusal(boi_dispatch):
        reports.append(
            _report(
                "controller-unconditional-refusal",
                INSTALLED_CONTROLLER_PATH,
                INSTALLED_CONTROLLER_STAGE,
                "BOI composite dispatcher remains an unconditional refusal",
                "_dispatch_boi_composite",
                boi_dispatch.lineno,
            )
        )

    macos_files = _static_string_sequence(module, _assignment(module, "MACOS_FILES"))
    if macos_files is None:
        reports.append(
            _report(
                "controller-source-closure",
                INSTALLED_CONTROLLER_PATH,
                INSTALLED_CONTROLLER_STAGE,
                "MACOS_FILES is not a statically auditable string sequence",
            )
        )
    else:
        for required in CONTROLLER_REQUIRED_MACOS_FILES:
            if required not in macos_files:
                reports.append(
                    _report(
                        "controller-source-closure",
                        INSTALLED_CONTROLLER_PATH,
                        INSTALLED_CONTROLLER_STAGE,
                        f"MACOS_FILES omits required dependency {required!r}",
                    )
                )
    return reports


def unresolved_prerequisites(
    repository: Path,
    prerequisites: Iterable[Prerequisite] = PREREQUISITES,
) -> list[dict[str, object]]:
    """Return every incomplete native, registry, preflight, and operation path."""

    reports = [
        *_audit_native(repository),
        *_audit_client(repository),
        *_audit_sealed_controller(repository),
    ]
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
