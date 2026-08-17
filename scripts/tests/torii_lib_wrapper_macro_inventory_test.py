#!/usr/bin/env python3
"""Guard Torii's narrow wrapper macros against semantic inventory drift."""

from __future__ import annotations

import hashlib
import json
import re
import unittest
from dataclasses import dataclass
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SOURCE_PATH = ROOT / "crates/iroha_torii/src/lib.rs"


class GuardError(AssertionError):
    """The generated wrapper surface no longer matches its direct preimage."""


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
        definition_sha256="6efd7a13947783182a35ce61e2ebe8794844070d414eb2b45274201f0a2f6539",
        expanded_preimage_sha256="250bef5068184896cfc3553775fe1d7e55670da201c141011f9afa42e2599018",
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
            "message_id_field",
            "missing_message_id",
            "payload_builder",
        ),
        literal_parameters=frozenset(
            {
                "message_type",
                "access_context",
                "message_id_field",
                "missing_message_id",
            }
        ),
        invocations=(
            InvocationGroup(
                rows=(
                    (
                        "handler_iso_pacs008",
                        "pacs.008",
                        "v1/iso20022/pacs008",
                        "MsgId",
                        "missing MsgId field",
                        "build_pacs008_payload",
                    ),
                    (
                        "handler_iso_pacs009",
                        "pacs.009",
                        "v1/iso20022/pacs009",
                        "BizMsgIdr",
                        "missing BizMsgIdr field",
                        "build_pacs009_payload",
                    ),
                )
            ),
        ),
        definition_sha256="7be1ed891831c066458d5530dbb86107df610ef9a180ef4ee2b82e2f3fb3dfbf",
        expanded_preimage_sha256="0a64ec7004bc8f0af4042518e223901c245f4a7a5816e1106383c462282b5094",
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
        definition_sha256="f8d4deedb991e58bd1aff5099767b19b64e72434712d876c89d4c44a3188dac1",
        expanded_preimage_sha256="7a5cd85f41c0a8ccde22bea51f7efca630bbe98aa0e126d2b93dde472a1292f6",
    ),
}


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


def validate_source(source: str) -> None:
    """Validate exact definitions, inputs, expansions, and source ordering."""

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


class ToriiWrapperMacroInventoryTest(unittest.TestCase):
    """Exercise the source guard and representative fail-closed mutations."""

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
                "                axum::extract::ConnectInfo(remote): "
                "axum::extract::ConnectInfo<std::net::SocketAddr>,\n"
                "                AxQuery(params): "
                "AxQuery<crate::routing::ContractEventGetParams>",
                "async fn $handler(\n"
                "                State(app): State<SharedAppState>,\n"
                "                headers: axum::http::HeaderMap,\n"
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
                '(handler_iso_sese025_submit, "sese.025", "v1/iso20022/sese025"),',
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


if __name__ == "__main__":
    unittest.main()
