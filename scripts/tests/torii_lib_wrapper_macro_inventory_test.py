#!/usr/bin/env python3
"""Guard Torii's narrow wrapper and route macros against semantic drift."""

from __future__ import annotations

import hashlib
import json
import re
import unittest
from dataclasses import dataclass
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SOURCE_PATH = ROOT / "crates/iroha_torii/src/lib.rs"
ROUTE_CATALOG_SOURCE_PATH = (
    ROOT / "crates/iroha_torii_shared/src/route_catalog.rs"
)
ROUTE_AUTH_METADATA_SCHEMA_VERSION = 1


class GuardError(AssertionError):
    """A generated wrapper or route surface no longer matches its direct preimage."""


@dataclass(frozen=True)
class InvocationGroup:
    """One source-positioned invocation of a wrapper macro."""

    rows: tuple[tuple[str, ...], ...]
    trailing_comma: bool = True


@dataclass(frozen=True)
class WrapperFamily:
    """Exact macro inputs and hashes derived from the direct wrapper preimage."""

    parameters: tuple[str, ...]
    literal_parameters: frozenset[str]
    invocations: tuple[InvocationGroup, ...]
    definition_sha256: str
    expanded_preimage_sha256: str

    @property
    def rows(self) -> tuple[tuple[str, ...], ...]:
        return tuple(row for group in self.invocations for row in group.rows)


FAMILIES = {
    "contracts_rollup_event_get_handlers": WrapperFamily(
        parameters=("handler", "routing_handler", "key_hint"),
        literal_parameters=frozenset({"key_hint"}),
        invocations=(
            InvocationGroup(
                rows=(
                    (
                        "handler_contracts_rollups_intents_get",
                        "handle_v1_contracts_rollups_intents_get",
                        "contracts-rollups-intents",
                    ),
                    (
                        "handler_contracts_rollups_vault_positions_get",
                        "handle_v1_contracts_rollups_vault_positions_get",
                        "contracts-rollups-vaults",
                    ),
                    (
                        "handler_contracts_rollups_operators_status_get",
                        "handle_v1_contracts_rollups_operators_status_get",
                        "contracts-rollups-operators",
                    ),
                    (
                        "handler_contracts_rollups_margin_health_get",
                        "handle_v1_contracts_rollups_margin_health_get",
                        "contracts-rollups-margin",
                    ),
                    (
                        "handler_contracts_rollups_rwa_lots_get",
                        "handle_v1_contracts_rollups_rwa_lots_get",
                        "contracts-rollups-rwa",
                    ),
                    (
                        "handler_contracts_rollups_dlmm_hooks_get",
                        "handle_v1_contracts_rollups_dlmm_hooks_get",
                        "contracts-rollups-dlmm-hooks",
                    ),
                )
            ),
        ),
        definition_sha256="d687bb8391a1e7cfd16adec2a0f167b6097b9b0c86a1962e08fe9ffba6bd2477",
        expanded_preimage_sha256="553ff5561e4fa417bfadf417331d78661d90b3b18dbe940a610a5db9991172d9",
    ),
    "subscription_action_handlers": WrapperFamily(
        parameters=("handler", "routing_handler", "access_context"),
        literal_parameters=frozenset({"access_context"}),
        invocations=(
            InvocationGroup(
                rows=(
                    (
                        "handler_subscription_pause",
                        "handle_post_v1_subscription_pause",
                        "v1/subscriptions/{subscription_id}/pause",
                    ),
                    (
                        "handler_subscription_resume",
                        "handle_post_v1_subscription_resume",
                        "v1/subscriptions/{subscription_id}/resume",
                    ),
                    (
                        "handler_subscription_cancel",
                        "handle_post_v1_subscription_cancel",
                        "v1/subscriptions/{subscription_id}/cancel",
                    ),
                    (
                        "handler_subscription_keep",
                        "handle_post_v1_subscription_keep",
                        "v1/subscriptions/{subscription_id}/keep",
                    ),
                )
            ),
            InvocationGroup(
                rows=(
                    (
                        "handler_subscription_charge_now",
                        "handle_post_v1_subscription_charge_now",
                        "v1/subscriptions/{subscription_id}/charge-now",
                    ),
                ),
                trailing_comma=False,
            ),
        ),
        definition_sha256="3b69a1e1c6dc7f56af788a13a1202eeeea56539d294ae06fd9e86e3dbbf421b5",
        expanded_preimage_sha256="dbf99e521129694239c6791f9440608cb67c49b56b8ea2025c0c546522de21dc",
    ),
    "account_recovery_command_handlers": WrapperFamily(
        parameters=("handler", "dto", "metric", "route", "routing_handler"),
        literal_parameters=frozenset({"metric", "route"}),
        invocations=(
            InvocationGroup(
                rows=(
                    (
                        "handler_post_account_recovery_policy_set",
                        "crate::routing::AccountRecoveryPolicySetDto",
                        "account_recovery_policy_set",
                        "v1/accounts/recovery/policy/set",
                        "handle_post_account_recovery_policy_set",
                    ),
                    (
                        "handler_post_account_recovery_propose",
                        "crate::routing::AccountRecoveryProposeDto",
                        "account_recovery_propose",
                        "v1/accounts/recovery/propose",
                        "handle_post_account_recovery_propose",
                    ),
                    (
                        "handler_post_account_recovery_approve",
                        "crate::routing::AccountRecoveryApproveDto",
                        "account_recovery_approve",
                        "v1/accounts/recovery/approve",
                        "handle_post_account_recovery_approve",
                    ),
                    (
                        "handler_post_account_recovery_finalize",
                        "crate::routing::AccountRecoveryFinalizeDto",
                        "account_recovery_finalize",
                        "v1/accounts/recovery/finalize",
                        "handle_post_account_recovery_finalize",
                    ),
                )
            ),
        ),
        definition_sha256="5eb1270d79d4a81f2b47e50bf2d9604f40b55709dbe7044ad8ec73e5cf9b4c35",
        expanded_preimage_sha256="ec2b277f4c7d4925fca1ffa64727b1bc6e423015196d1e1a727a0eeef846acbf",
    ),
    "iso_payment_submission_handlers": WrapperFamily(
        parameters=(
            "handler",
            "message_type",
            "access_context",
            "payload_builder",
        ),
        literal_parameters=frozenset(
            {
                "message_type",
                "access_context",
            }
        ),
        invocations=(
            InvocationGroup(
                rows=(
                    (
                        "handler_iso_pacs008",
                        "pacs.008",
                        "v1/iso20022/pacs008",
                        "build_pacs008_payload",
                    ),
                    (
                        "handler_iso_pacs009",
                        "pacs.009",
                        "v1/iso20022/pacs009",
                        "build_pacs009_payload",
                    ),
                )
            ),
        ),
        definition_sha256="fd5e33624892c8034acbe981fa8080df5ba5e0cb76853ea485bdd76da28019aa",
        expanded_preimage_sha256="99dc2af613287fa81ee6dfd236922a8c0f61e32f6c4f4d98a13a22809c764ea8",
    ),
    "iso_lifecycle_submission_handlers": WrapperFamily(
        parameters=("handler", "message_type", "access_context"),
        literal_parameters=frozenset({"message_type", "access_context"}),
        invocations=(
            InvocationGroup(
                rows=(
                    (
                        "handler_iso_pacs002_submit",
                        "pacs.002",
                        "v1/iso20022/pacs002",
                    ),
                    (
                        "handler_iso_pacs004_submit",
                        "pacs.004",
                        "v1/iso20022/pacs004",
                    ),
                    (
                        "handler_iso_camt056_submit",
                        "camt.056",
                        "v1/iso20022/camt056",
                    ),
                    (
                        "handler_iso_sese023_submit",
                        "sese.023",
                        "v1/iso20022/sese023",
                    ),
                    (
                        "handler_iso_sese024_submit",
                        "sese.024",
                        "v1/iso20022/sese024",
                    ),
                    (
                        "handler_iso_sese025_submit",
                        "sese.025",
                        "v1/iso20022/sese025",
                    ),
                    (
                        "handler_iso_colr012_submit",
                        "colr.012",
                        "v1/iso20022/colr012",
                    ),
                )
            ),
        ),
        definition_sha256="9409d24394be2a627a2e56e48de200716f229a13f89bfc88ea3603def166dc54",
        expanded_preimage_sha256="da365df6755156153cc52f91597e7ce091588bffeab679331ece77e6d39d40be",
    ),
}

ROUTE_MACRO_DEFINITION_SHA256 = {
    "catalog_route_policy": "60a5d4d3cb782b3609a24dd5b6850fb1511da3f6bb5f6a3029734c13f84ca18e",
    "mount_catalog_route_rows": "3e8928222d7cc7586d5d380b04183132188cc9e4b74f70816a51816d637da23e",
    "mount_local_catalog_route_rows": "74c42676d5766d5d942f9d3dc2d4e7ebbda33330ab1e25be73b355771c57b25d",
}
ROUTE_ROW_COUNT = 541
ROUTE_TUPLE_SHA256 = "91a60004faa27b7a045249ccfc96b7626cc0524c8fb80677796a0bfaa521fb95"


def _normalized_tokens(source: str) -> bytes:
    """Discard layout/comments while preserving string and raw-string bytes."""

    output: list[str] = []
    index = 0
    state = "code"
    block_depth = 0
    raw_hashes = 0
    while index < len(source):
        if state == "code":
            if source.startswith("//", index):
                state = "line_comment"
                index += 2
                continue
            if source.startswith("/*", index):
                state = "block_comment"
                block_depth = 1
                index += 2
                continue
            raw = re.match(r'(?:br|rb|r)(#*)"', source[index:])
            if raw:
                token = raw.group(0)
                output.append(token)
                raw_hashes = len(raw.group(1))
                index += len(token)
                state = "raw_string"
                continue
            if source.startswith('b"', index):
                output.append('b"')
                index += 2
                state = "string"
                continue
            if source[index] == '"':
                output.append('"')
                index += 1
                state = "string"
                continue
            if source[index].isspace():
                index += 1
                continue
            output.append(source[index])
            index += 1
            continue
        if state == "line_comment":
            if source[index] == "\n":
                state = "code"
            index += 1
            continue
        if state == "block_comment":
            if source.startswith("/*", index):
                block_depth += 1
                index += 2
            elif source.startswith("*/", index):
                block_depth -= 1
                index += 2
                if block_depth == 0:
                    state = "code"
            else:
                index += 1
            continue
        if state == "string":
            output.append(source[index])
            if source[index] == "\\" and index + 1 < len(source):
                output.append(source[index + 1])
                index += 2
            elif source[index] == '"':
                state = "code"
                index += 1
            else:
                index += 1
            continue
        raw_end = '"' + "#" * raw_hashes
        if source.startswith(raw_end, index):
            output.append(raw_end)
            index += len(raw_end)
            state = "code"
        else:
            output.append(source[index])
            index += 1
    return "".join(output).encode()


def _sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _macro_definition(source: str, name: str) -> tuple[str, str]:
    start = source.index(f"macro_rules! {name}")
    invocation = source.index(f"{name}!", start)
    definition = source[start:invocation]
    body_start_marker = "\n        $(\n"
    body_end_marker = "\n        )+\n"
    body_start = definition.index(body_start_marker) + len(body_start_marker)
    body_end = definition.index(body_end_marker, body_start)
    return definition, definition[body_start:body_end]


def _render_value(parameter: str, value: str, family: WrapperFamily) -> str:
    if parameter in family.literal_parameters:
        return json.dumps(value, ensure_ascii=True)
    return value


def _expanded_digest(body: str, family: WrapperFamily) -> str:
    expansions = []
    for row in family.rows:
        if len(row) != len(family.parameters):
            raise GuardError("wrapper row width does not match macro parameter width")
        expansion = body
        for parameter, value in zip(family.parameters, row):
            expansion = expansion.replace(
                f"${parameter}", _render_value(parameter, value, family)
            )
        unresolved = re.findall(r"\$[A-Za-z_][A-Za-z0-9_]*", expansion)
        if unresolved:
            raise GuardError(f"unresolved macro parameters: {unresolved}")
        expansions.append(_normalized_tokens(expansion))
    return _sha256(b"\0".join(expansions))


def _matching_parenthesis(source: str, opening: int) -> int:
    depth = 0
    index = opening
    state = "code"
    while index < len(source):
        char = source[index]
        if state == "code":
            if source.startswith("//", index):
                state = "line_comment"
                index += 2
                continue
            if source.startswith("/*", index):
                state = "block_comment"
                index += 2
                continue
            if char == '"':
                state = "string"
            elif char == "(":
                depth += 1
            elif char == ")":
                depth -= 1
                if depth == 0:
                    return index
            index += 1
            continue
        if state == "line_comment":
            if char == "\n":
                state = "code"
            index += 1
            continue
        if state == "block_comment":
            if source.startswith("*/", index):
                state = "code"
                index += 2
            else:
                index += 1
            continue
        if char == "\\":
            index += 2
        elif char == '"':
            state = "code"
            index += 1
        else:
            index += 1
    raise GuardError("unterminated macro invocation")


def _macro_invocations(source: str, name: str) -> list[tuple[int, str]]:
    matches = []
    pattern = re.compile(rf"\b{re.escape(name)}!\s*\(")
    for match in pattern.finditer(source):
        opening = source.index("(", match.start())
        closing = _matching_parenthesis(source, opening)
        semicolon = closing + 1
        while semicolon < len(source) and source[semicolon].isspace():
            semicolon += 1
        if semicolon >= len(source) or source[semicolon] != ";":
            raise GuardError(f"{name} invocation must end with a semicolon")
        matches.append((match.start(), source[match.start() : semicolon + 1]))
    return matches


def _expected_invocation(
    name: str, family: WrapperFamily, group: InvocationGroup
) -> bytes:
    rows = []
    for row in group.rows:
        values = [
            _render_value(parameter, value, family)
            for parameter, value in zip(family.parameters, row)
        ]
        rows.append(f"({','.join(values)})")
    suffix = "," if group.trailing_comma else ""
    return _normalized_tokens(f"{name}!({','.join(rows)}{suffix});")


def _matching_delimiter(
    source: str, opening: int, left: str = "(", right: str = ")"
) -> int:
    """Find a balanced delimiter while ignoring comments and string contents."""

    depth = 0
    index = opening
    state = "code"
    block_depth = 0
    while index < len(source):
        char = source[index]
        if state == "code":
            if source.startswith("//", index):
                state = "line_comment"
                index += 2
                continue
            if source.startswith("/*", index):
                state = "block_comment"
                block_depth = 1
                index += 2
                continue
            if char == '"':
                state = "string"
            elif char == left:
                depth += 1
            elif char == right:
                depth -= 1
                if depth == 0:
                    return index
            index += 1
            continue
        if state == "line_comment":
            if char == "\n":
                state = "code"
            index += 1
            continue
        if state == "block_comment":
            if source.startswith("/*", index):
                block_depth += 1
                index += 2
            elif source.startswith("*/", index):
                block_depth -= 1
                index += 2
                if block_depth == 0:
                    state = "code"
            else:
                index += 1
            continue
        if char == "\\":
            index += 2
        elif char == '"':
            state = "code"
            index += 1
        else:
            index += 1
    raise GuardError("unterminated source delimiter")


def _split_top_level(source: str, delimiter: str) -> list[str]:
    """Split on one delimiter outside nested Rust token groups."""

    output: list[str] = []
    start = 0
    stack: list[str] = []
    index = 0
    state = "code"
    block_depth = 0
    pairs = {"(": ")", "[": "]", "{": "}"}
    while index < len(source):
        char = source[index]
        if state == "code":
            if source.startswith("//", index):
                state = "line_comment"
                index += 2
                continue
            if source.startswith("/*", index):
                state = "block_comment"
                block_depth = 1
                index += 2
                continue
            if char == '"':
                state = "string"
            elif char in pairs:
                stack.append(pairs[char])
            elif stack and char == stack[-1]:
                stack.pop()
            elif char == delimiter and not stack:
                output.append(source[start:index].strip())
                start = index + 1
            index += 1
            continue
        if state == "line_comment":
            if char == "\n":
                state = "code"
            index += 1
            continue
        if state == "block_comment":
            if source.startswith("/*", index):
                block_depth += 1
                index += 2
            elif source.startswith("*/", index):
                block_depth -= 1
                index += 2
                if block_depth == 0:
                    state = "code"
            else:
                index += 1
            continue
        if char == "\\":
            index += 2
        elif char == '"':
            state = "code"
            index += 1
        else:
            index += 1
    if source[start:].strip():
        output.append(source[start:].strip())
    return output


def _route_macro_definition(source: str, name: str) -> str:
    try:
        start = source.index(f"macro_rules! {name}")
        opening = source.index("{", start)
    except ValueError as error:
        raise GuardError(f"{name} definition is missing") from error
    closing = _matching_delimiter(source, opening, "{", "}")
    return source[start : closing + 1]


def _cfg_target_end(source: str, target: int) -> int:
    """Return the end of the item, block, or statement governed by a cfg."""

    stack: list[str] = []
    index = target
    state = "code"
    block_depth = 0
    pairs = {"(": ")", "[": "]"}
    while index < len(source):
        char = source[index]
        if state == "code":
            if source.startswith("//", index):
                state = "line_comment"
                index += 2
                continue
            if source.startswith("/*", index):
                state = "block_comment"
                block_depth = 1
                index += 2
                continue
            if char == '"':
                state = "string"
            elif char in pairs:
                stack.append(pairs[char])
            elif stack and char == stack[-1]:
                stack.pop()
            elif not stack and char == "{":
                return _matching_delimiter(source, index, "{", "}") + 1
            elif not stack and char == ";":
                return index + 1
            index += 1
            continue
        if state == "line_comment":
            if char == "\n":
                state = "code"
            index += 1
            continue
        if state == "block_comment":
            if source.startswith("/*", index):
                block_depth += 1
                index += 2
            elif source.startswith("*/", index):
                block_depth -= 1
                index += 2
                if block_depth == 0:
                    state = "code"
            else:
                index += 1
            continue
        if char == "\\":
            index += 2
        elif char == '"':
            state = "code"
            index += 1
        else:
            index += 1
    raise GuardError("cfg attribute has no governed source target")


def _route_cfg_ranges(
    source: str, corridor_start: int, corridor_end: int
) -> list[tuple[int, int, str]]:
    ranges = []
    pattern = re.compile(r"#\[cfg\s*\(")
    for match in pattern.finditer(source, corridor_start, corridor_end):
        opening = source.index("(", match.start())
        closing = _matching_delimiter(source, opening)
        expression = "".join(source[opening + 1 : closing].split())
        cursor = source.find("]", closing) + 1
        while True:
            while cursor < len(source) and source[cursor].isspace():
                cursor += 1
            if not source.startswith("#[", cursor):
                break
            cursor = _matching_delimiter(source, cursor + 1, "[", "]") + 1
        ranges.append((cursor, _cfg_target_end(source, cursor), expression))
    return ranges


def _route_cfg_at(position: int, ranges: list[tuple[int, int, str]]) -> str:
    active = [item for item in ranges if item[0] <= position < item[1]]
    active.sort(key=lambda item: item[0])
    return "&".join(expression for _, _, expression in active) or "always"


def _route_semantics(policy: str, arguments: list[str]) -> tuple[str, str, str]:
    method = policy.rsplit("_", 1)[-1].upper()
    if method not in {"GET", "POST", "DELETE", "ANY"}:
        raise GuardError(f"unknown route method policy: {policy}")

    def require(width: int) -> None:
        if len(arguments) != width:
            raise GuardError(f"{policy} row width drifted")

    if policy.startswith("limited_canonical_account_"):
        require(4)
        limit = f"max({arguments[2]});auth({arguments[3]})"
        auth = f"canonical-account({arguments[1]}.clone())"
    elif policy.startswith("layered_canonical_account_"):
        require(4)
        limit = f"layer({arguments[2]}.clone());auth({arguments[3]})"
        auth = f"canonical-account({arguments[1]}.clone())"
    elif policy.startswith("canonical_account_proof_"):
        if policy == "canonical_account_proof_get":
            require(2)
            proof_limit = "0"
        elif policy == "canonical_account_proof_post":
            require(3)
            proof_limit = arguments[2]
        else:
            raise GuardError(f"unknown canonical proof route policy: {policy}")
        limit = f"proof({proof_limit})"
        auth = f"canonical-account-proof({arguments[1]}.clone())"
    elif policy.startswith("canonical_account_"):
        require(3)
        limit = f"auth({arguments[2]})"
        auth = f"canonical-account({arguments[1]}.clone())"
    elif policy.startswith("limited_operator_"):
        require(3)
        limit = f"max({arguments[2]})"
        auth = f"operator({arguments[1]}.clone())"
    elif policy.startswith("operator_") and policy != "operator_credential_post":
        require(2)
        limit = "none"
        auth = f"operator({arguments[1]}.clone())"
    elif policy.startswith("limited_hardened_canonical_signature_"):
        require(2)
        if policy != "limited_hardened_canonical_signature_get":
            raise GuardError(f"unknown hardened route policy: {policy}")
        limit = f"max({arguments[1]});harden-reputation"
        auth = "handler:CanonicalAccountSignature"
    elif policy.startswith("limited_"):
        require(2)
        limit = f"max({arguments[1]})"
        stem = policy[len("limited_") :].rsplit("_", 1)[0]
        try:
            auth = {
                "public": "public",
                "unauthenticated": "unauthenticated",
                "canonical_signature": "handler:CanonicalAccountSignature",
                "optional_canonical_signature":
                    "handler:OptionalCanonicalAccountSignature",
                "canonical_signed": "handler:CanonicalSignedBody",
                "protocol_handshake": "handler:ProtocolHandshake",
            }[stem]
        except KeyError as error:
            raise GuardError(f"unknown limited route policy: {policy}") from error
    elif policy.startswith("layered_"):
        require(2)
        limit = f"layer({arguments[1]}.clone())"
        stem = policy[len("layered_") :].rsplit("_", 1)[0]
        try:
            auth = {
                "public": "public",
                "canonical_signature": "handler:CanonicalAccountSignature",
                "canonical_signed": "handler:CanonicalSignedBody",
            }[stem]
        except KeyError as error:
            raise GuardError(f"unknown layered route policy: {policy}") from error
    else:
        require(1)
        limit = "none"
        stem = policy.rsplit("_", 1)[0]
        try:
            auth = {
                "public": "public",
                "unauthenticated": "unauthenticated",
                "canonical_signature": "handler:CanonicalAccountSignature",
                "optional_canonical_signature":
                    "handler:OptionalCanonicalAccountSignature",
                "canonical_signed": "handler:CanonicalSignedBody",
                "protocol_handshake": "handler:ProtocolHandshake",
                "operator_credential": "handler:OperatorCredentialExchange",
                "onboarding": "onboarding",
            }[stem]
        except KeyError as error:
            raise GuardError(f"unknown direct route policy: {policy}") from error
    return method, limit, auth


def _route_table_rows(source: str) -> list[tuple[str, str, str, str, str, str]]:
    try:
        corridor_start = source.index(
            '    #[cfg(feature = "telemetry")]\n'
            "    #[allow(clippy::unused_self)]\n"
            "    fn add_telemetry_routes"
        )
        corridor_end = source.index(
            "    fn add_runtime_governance_routes", corridor_start
        )
    except ValueError as error:
        raise GuardError("Torii route-builder corridor markers drifted") from error
    cfg_ranges = _route_cfg_ranges(source, corridor_start, corridor_end)
    positioned_rows = []
    for name, local_catalog in (
        ("mount_catalog_route_rows", False),
        ("mount_local_catalog_route_rows", True),
    ):
        pattern = re.compile(rf"\b{re.escape(name)}!\s*\(")
        for match in pattern.finditer(source, corridor_start, corridor_end):
            opening = source.index("(", match.start())
            closing = _matching_delimiter(source, opening)
            semicolon = closing + 1
            while semicolon < len(source) and source[semicolon].isspace():
                semicolon += 1
            if semicolon >= len(source) or source[semicolon] != ";":
                raise GuardError(f"{name} invocation must end with a semicolon")
            parts = _split_top_level(source[opening + 1 : closing], ";")
            header = _split_top_level(parts[0], ",") if parts else []
            if len(header) != 2 or header[0] != "builder":
                raise GuardError(f"{name} header drifted")
            catalog = header[1]
            cfg = _route_cfg_at(match.start(), cfg_ranges)
            for row in parts[1:]:
                row_match = re.fullmatch(
                    r"([A-Z0-9_]+)\s*=>\s*([a-z0-9_]+)\s*(\(.*\))",
                    row,
                    re.S,
                )
                if not row_match:
                    raise GuardError(f"{name} row syntax drifted")
                descriptor, policy, argument_group = row_match.groups()
                arguments = _split_top_level(argument_group[1:-1], ",")
                if not arguments:
                    raise GuardError(f"{name} row has no handler")
                method, limit, auth = _route_semantics(policy, arguments)
                module = "runtime_governance" if local_catalog else catalog
                positioned_rows.append(
                    (
                        match.start(),
                        (
                            cfg,
                            method,
                            f"route_catalog::{module}::{descriptor}",
                            arguments[0],
                            limit,
                            auth,
                        ),
                    )
                )
    positioned_rows.sort(key=lambda item: item[0])
    return [row for _, row in positioned_rows]


def _validate_route_tables(source: str) -> None:
    for name, expected in ROUTE_MACRO_DEFINITION_SHA256.items():
        definition = _route_macro_definition(source, name)
        if _sha256(_normalized_tokens(definition)) != expected:
            raise GuardError(f"{name} definition drifted")
    rows = _route_table_rows(source)
    if len(rows) != ROUTE_ROW_COUNT:
        raise GuardError("Torii route row count drifted")
    catalog_source = ROUTE_CATALOG_SOURCE_PATH.read_text(encoding="utf-8")
    schema_match = re.search(
        r"pub const ROUTE_AUTH_METADATA_SCHEMA_VERSION_V1:\s*u16\s*=\s*(\d+)\s*;",
        catalog_source,
    )
    if schema_match is None:
        raise GuardError("route-auth metadata schema version is missing")
    schema_version = int(schema_match.group(1))
    if schema_version != ROUTE_AUTH_METADATA_SCHEMA_VERSION:
        raise GuardError("route-auth metadata schema version drifted")
    digest = _sha256(
        json.dumps(
            {
                "route_auth_metadata_schema_version": schema_version,
                "routes": rows,
            },
            separators=(",", ":"),
            ensure_ascii=True,
        ).encode()
    )
    if digest != ROUTE_TUPLE_SHA256:
        raise GuardError(
            "versioned ordered Torii (cfg, method, descriptor, handler, limit, auth) inventory drifted"
        )


def validate_source(source: str) -> None:
    """Validate exact wrapper/route definitions, expansions, and source ordering."""

    wrapper_names: list[str] = []
    positions: dict[str, list[int]] = {}
    for name, family in FAMILIES.items():
        definition, body = _macro_definition(source, name)
        actual_definition = _sha256(_normalized_tokens(definition))
        if actual_definition != family.definition_sha256:
            raise GuardError(f"{name} definition drifted")
        actual_expansion = _expanded_digest(body, family)
        if actual_expansion != family.expanded_preimage_sha256:
            raise GuardError(f"{name} no longer reconstructs its direct preimage")
        invocations = _macro_invocations(source, name)
        if len(invocations) != len(family.invocations):
            raise GuardError(f"{name} invocation count drifted")
        positions[name] = [position for position, _ in invocations]
        for (_, actual), expected in zip(invocations, family.invocations):
            if _normalized_tokens(actual) != _expected_invocation(name, family, expected):
                raise GuardError(f"{name} invocation inventory drifted")
        wrapper_names.extend(row[0] for row in family.rows)

    if len(wrapper_names) != len(set(wrapper_names)):
        raise GuardError("wrapper names must be globally unique")
    for wrapper in wrapper_names:
        if re.search(rf"\basync\s+fn\s+{re.escape(wrapper)}\b", source):
            raise GuardError(f"{wrapper} escaped its guarded macro family")

    usage = source.index("async fn handler_subscription_usage(")
    subscription_positions = positions["subscription_action_handlers"]
    if not subscription_positions[0] < usage < subscription_positions[1]:
        raise GuardError("subscription usage/action logical order drifted")
    if not (
        positions["account_recovery_command_handlers"][0]
        < source.index("async fn handler_post_account_recovery_status(")
    ):
        raise GuardError("account recovery command/status order drifted")
    if not (
        positions["iso_payment_submission_handlers"][0]
        < source.index("async fn handler_iso_lifecycle_submit(")
        < positions["iso_lifecycle_submission_handlers"][0]
        < source.index("async fn handler_iso_status(")
    ):
        raise GuardError("ISO submission/status logical order drifted")
    _validate_route_tables(source)


class ToriiWrapperMacroInventoryTest(unittest.TestCase):
    """Exercise both source inventories and representative fail-closed mutations."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.source = SOURCE_PATH.read_text(encoding="utf-8")

    def test_current_source_reconstructs_exact_preimage(self) -> None:
        validate_source(self.source)

    def test_signature_dispatch_inventory_and_order_mutations_fail(self) -> None:
        mutations = (
            (
                "async fn $handler(\n"
                "                State(app): State<SharedAppState>,\n"
                "                headers: axum::http::HeaderMap,\n"
                "                method: axum::http::Method,\n"
                "                uri: axum::http::Uri,\n"
                "                axum::extract::ConnectInfo(remote): "
                "axum::extract::ConnectInfo<std::net::SocketAddr>,\n"
                "                AxQuery(params): "
                "AxQuery<crate::routing::ContractEventGetParams>",
                "async fn $handler(\n"
                "                State(app): State<SharedAppState>,\n"
                "                headers: axum::http::HeaderMap,\n"
                "                method: axum::http::Method,\n"
                "                uri: axum::http::Uri,\n"
                "                axum::extract::ConnectInfo(remote): "
                "axum::extract::ConnectInfo<std::net::SocketAddr>,\n"
                "                AxQuery(params): "
                "AxQuery<crate::routing::TraderRollupAccountParams>",
            ),
            (
                "handle_post_v1_subscription_pause",
                "handle_post_v1_subscription_resume",
            ),
            (
                '"v1/accounts/recovery/approve"',
                '"v1/accounts/recovery/finalize"',
            ),
            (
                "runtime.mark_queued(&msg_id);",
                "runtime.mark_queued(&msg_id); runtime.mark_queued(&msg_id);",
            ),
            (
                "(\n"
                "        handler_iso_sese025_submit,\n"
                '        "sese.025",\n'
                '        "v1/iso20022/sese025"\n'
                "    ),",
                "",
            ),
            ("$message_type:literal", "$message_type:expr"),
        )
        for old, new in mutations:
            with self.subTest(old=old, new=new):
                self.assertIn(old, self.source)
                mutated = self.source.replace(old, new, 1)
                with self.assertRaises(GuardError):
                    validate_source(mutated)

    def test_route_policy_inventory_and_cfg_mutations_fail(self) -> None:
        mutations = (
            (
                "STATUS => unauthenticated_get(handler_status_root);\n"
                "            STATUS_TAIL => unauthenticated_get(handler_status_tail);",
                "STATUS_TAIL => unauthenticated_get(handler_status_tail);\n"
                "            STATUS => unauthenticated_get(handler_status_root);",
            ),
            (
                "TRANSACTION => limited_canonical_signed_post("
                "handler_post_transaction, body_limit);",
                "TRANSACTION => limited_canonical_signed_post("
                "handler_post_transaction, iso_body_limit);",
            ),
            (
                '#[cfg(feature = "telemetry")]\n'
                "    #[allow(clippy::unused_self)]\n"
                "    fn add_telemetry_routes",
                '#[cfg(feature = "app_api")]\n'
                "    #[allow(clippy::unused_self)]\n"
                "    fn add_telemetry_routes",
            ),
            (
                "catalog_get($handler).authenticated_operator($state.clone())",
                "catalog_get($handler).authenticated_operator($state)",
            ),
        )
        for old, new in mutations:
            with self.subTest(old=old, new=new):
                self.assertIn(old, self.source)
                mutated = self.source.replace(old, new, 1)
                with self.assertRaises(GuardError):
                    validate_source(mutated)


if __name__ == "__main__":
    unittest.main()
