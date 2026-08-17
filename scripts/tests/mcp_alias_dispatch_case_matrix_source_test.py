#!/usr/bin/env python3
"""Protect the name-preserving Torii MCP alias-dispatch case matrix."""

from __future__ import annotations

import hashlib
import re
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
MAIN_PATH = REPO_ROOT / "crates/iroha_torii/tests/mcp_endpoints.rs"
EXTENDED_PATH = (
    REPO_ROOT
    / "crates/iroha_torii/tests/mcp_endpoints/extended_tool_dispatch_tests.rs"
)
MAIN_MAX_LINES = 4_189
EXTENDED_MAX_LINES = 721

HELPER_START = "#[derive(Clone, Copy)]\nenum McpAliasDispatchArguments"
HELPER_END = "fn enable_writer_mcp"
HELPER_HASH = "8d9728ba23c408afd1d6d9f7bf1564ae2e0b0ed4cec6bc307178390ad4ec58ed"

# file, name, expectation, request id, tool name, argument row, assertion messages
CASES = (
    (
        "main",
        "mcp_jsonrpc_tools_call_agent_alias_accounts_get_accepts_flat_account_id",
        "error",
        1051,
        "iroha.accounts.get",
        "InvalidAccountId",
        "invalid account id should be marked as MCP tool error for account detail alias",
        "expected invalid account id to be rejected by explorer account detail alias",
    ),
    (
        "main",
        "mcp_jsonrpc_tools_call_agent_alias_accounts_qr_accepts_flat_account_id",
        "error",
        1052,
        "iroha.accounts.qr",
        "InvalidAccountId",
        "invalid account id should be marked as MCP tool error for account QR alias",
        "expected invalid account id to be rejected by explorer account QR alias",
    ),
    (
        "main",
        "mcp_jsonrpc_tools_call_agent_alias_transaction_status_accepts_flat_hash",
        "error",
        1061,
        "iroha.transactions.status",
        "InvalidHash",
        "invalid flat hash should be marked as MCP tool error",
        "tool_execution_error",
    ),
    (
        "main",
        "mcp_jsonrpc_tools_call_agent_alias_transaction_status_accepts_transaction_hash_alias",
        "error",
        10616,
        "iroha.transactions.status",
        "InvalidTransactionHash",
        "invalid transaction_hash alias should be marked as MCP tool error",
        "tool_execution_error",
    ),
    (
        "main",
        "mcp_jsonrpc_tools_call_agent_alias_transactions_get_accepts_flat_hash",
        "error",
        10612,
        "iroha.transactions.get",
        "InvalidHash",
        "invalid hash should be marked as MCP tool error for transaction detail alias",
        "expected invalid transaction hash to be rejected by explorer detail alias",
    ),
    (
        "main",
        "mcp_jsonrpc_tools_call_agent_alias_assets_get_accepts_flat_asset_id",
        "error",
        106152,
        "iroha.assets.get",
        "InvalidAssetId",
        "invalid asset id should be marked as MCP tool error for asset detail alias",
        "expected invalid asset id to be rejected by explorer asset detail alias",
    ),
    (
        "main",
        "mcp_jsonrpc_tools_call_agent_alias_nfts_get_accepts_flat_nft_id",
        "error",
        106154,
        "iroha.nfts.get",
        "InvalidNftId",
        "invalid nft id should be marked as MCP tool error for nft detail alias",
        "expected invalid nft id to be rejected by explorer nft detail alias",
    ),
    (
        "main",
        "mcp_jsonrpc_tools_call_agent_alias_rwas_get_accepts_flat_rwa_id",
        "error",
        106158,
        "iroha.rwas.get",
        "InvalidRwaId",
        "invalid rwa id should be marked as MCP tool error for rwa detail alias",
        "expected invalid rwa id to be rejected by explorer rwa detail alias",
    ),
    (
        "main",
        "mcp_jsonrpc_tools_call_agent_alias_domains_get_accepts_flat_domain_id",
        "error",
        1062221,
        "iroha.domains.get",
        "InvalidDomainId",
        "invalid domain id should be marked as MCP tool error for domain detail alias",
        "expected invalid domain id to be rejected by explorer domain detail alias",
    ),
    (
        "extended",
        "mcp_jsonrpc_tools_call_agent_alias_subscriptions_get_accepts_flat_subscription_id",
        "error",
        1062233,
        "iroha.subscriptions.get",
        "InvalidSubscriptionId",
        "invalid subscription id should be marked as MCP tool error for subscription detail alias",
        "expected invalid subscription id to be rejected by subscription detail alias",
    ),
    (
        "extended",
        "mcp_jsonrpc_tools_call_agent_alias_asset_definitions_get_accepts_flat_definition_id",
        "error",
        1062241,
        "iroha.assets.definitions.get",
        "InvalidDefinitionId",
        "invalid definition id should be marked as MCP tool error for definition detail alias",
        "expected invalid definition id to be rejected by explorer definition detail alias",
    ),
    (
        "main",
        "mcp_jsonrpc_tools_call_agent_alias_transactions_list_accepts_flat_query_fields",
        "success",
        10611,
        "iroha.transactions.list",
        "LimitTwo",
        "transactions list alias with flat query fields should dispatch successfully",
    ),
    (
        "main",
        "mcp_jsonrpc_tools_call_agent_alias_instructions_list_accepts_flat_query_fields",
        "success",
        10613,
        "iroha.instructions.list",
        "PageOne",
        "instructions list alias with flat query fields should dispatch successfully",
    ),
    (
        "main",
        "mcp_jsonrpc_tools_call_agent_alias_assets_list_accepts_flat_query_fields",
        "success",
        106151,
        "iroha.assets.list",
        "PageOne",
        "assets list alias with flat query fields should dispatch successfully",
    ),
    (
        "main",
        "mcp_jsonrpc_tools_call_agent_alias_nfts_list_accepts_flat_query_fields",
        "success",
        106153,
        "iroha.nfts.list",
        "PageOne",
        "nfts list alias with flat query fields should dispatch successfully",
    ),
    (
        "main",
        "mcp_jsonrpc_tools_call_agent_alias_nfts_query_accepts_flat_envelope_fields",
        "success",
        106155,
        "iroha.nfts.query",
        "LimitTwo",
        "nfts query alias with flat envelope fields should dispatch successfully",
    ),
    (
        "main",
        "mcp_jsonrpc_tools_call_agent_alias_rwas_list_accepts_flat_query_fields",
        "success",
        106157,
        "iroha.rwas.list",
        "PageOne",
        "rwas list alias with flat query fields should dispatch successfully",
    ),
    (
        "main",
        "mcp_jsonrpc_tools_call_agent_alias_rwas_query_accepts_flat_envelope_fields",
        "success",
        106159,
        "iroha.rwas.query",
        "LimitTwo",
        "rwas query alias with flat envelope fields should dispatch successfully",
    ),
    (
        "main",
        "mcp_jsonrpc_tools_call_agent_alias_blocks_list_accepts_flat_query_fields",
        "success",
        10616,
        "iroha.blocks.list",
        "PageOne",
        "blocks list alias with flat query fields should dispatch successfully",
    ),
    (
        "main",
        "mcp_jsonrpc_tools_call_agent_alias_domains_query_accepts_flat_envelope_fields",
        "success",
        106223,
        "iroha.domains.query",
        "LimitTwo",
        "domains query alias with flat envelope fields should dispatch successfully",
    ),
)

EXCLUDED_DIRECT_TESTS = (
    "mcp_jsonrpc_tools_call_agent_alias_contracts_code_get_accepts_hash_shortcut",
    "mcp_jsonrpc_tools_call_agent_alias_contracts_code_bytes_get_accepts_hash_shortcut",
    "mcp_jsonrpc_tools_call_agent_alias_iso20022_status_accepts_message_id_shortcut",
    "mcp_jsonrpc_tools_call_agent_alias_asset_definitions_query_accepts_flat_envelope_fields",
)

HELPER_TOKENS = (
    'norito::json!({"account_id": "not-an-account-id"})',
    'norito::json!({"hash": "not-a-hash"})',
    'norito::json!({"transaction_hash": "not-a-hash"})',
    'norito::json!({"asset_id": "not-an-asset-id"})',
    'norito::json!({"nft_id": "not-an-nft-id"})',
    'norito::json!({"rwa_id": "not-a-rwa-id"})',
    'norito::json!({"domain_id": "not-a-domain-id"})',
    'norito::json!({"subscription_id": "not-a-subscription-id"})',
    'norito::json!({"definition_id": "not-a-definition-id"})',
    'norito::json!({"limit": 2})',
    'norito::json!({"page": 1})',
    "cfg.torii.mcp.enabled = true",
    '"method": "tools/call"',
    '"arguments": { case.arguments.into_json() }',
    "assert_eq!(status, StatusCode::OK)",
    "tool_is_error(&call)",
    ".is_some_and(|status| status >= 400)",
    "Some(200)",
)


class GuardError(AssertionError):
    """Raised when the protected MCP dispatch matrix changes."""


def _normalized(source: str) -> str:
    normalized: list[str] = []
    in_string = False
    escaped = False
    for char in source:
        if in_string:
            normalized.append(char)
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == '"':
                in_string = False
        elif char == '"':
            normalized.append(char)
            in_string = True
        elif not char.isspace():
            normalized.append(char)
    if in_string:
        raise GuardError("unterminated string while normalizing MCP alias source")
    return "".join(normalized)


def _normalized_hash(source: str) -> str:
    return hashlib.sha256(_normalized(source).encode()).hexdigest()


def _helper_region(main: str) -> str:
    if main.count(HELPER_START) != 1 or main.count(HELPER_END) != 1:
        raise GuardError("MCP alias helper markers must occur exactly once")
    start = main.index(HELPER_START)
    end = main.index(HELPER_END, start)
    return main[start:end]


def _matching_brace(source: str, opening: int) -> int:
    depth = 0
    state = "code"
    index = opening
    while index < len(source):
        char = source[index]
        following = source[index + 1] if index + 1 < len(source) else ""
        if state == "code":
            if char == "/" and following == "/":
                state = "line"
                index += 2
                continue
            if char == '"':
                state = "string"
            elif char == "{":
                depth += 1
            elif char == "}":
                depth -= 1
                if depth == 0:
                    return index
        elif state == "line":
            if char == "\n":
                state = "code"
        elif char == "\\":
            index += 2
            continue
        elif char == '"':
            state = "code"
        index += 1
    raise GuardError("unterminated MCP alias macro invocation")


def _invocations(source: str) -> tuple[str, ...]:
    found = []
    for match in re.finditer(r"\bmcp_alias_dispatch_test!\s*\{", source):
        opening = match.end() - 1
        found.append(source[match.start() : _matching_brace(source, opening) + 1])
    return tuple(found)


def _expected_invocation(case: tuple[object, ...]) -> str:
    _, name, expectation, request_id, tool_name, arguments, *messages = case
    fields = ",".join(
        [str(request_id), f'"{tool_name}"', str(arguments)]
        + [f'"{message}"' for message in messages]
    )
    return (
        "mcp_alias_dispatch_test!{"
        "#[tokio::test]"
        f"asyncfn{name}=>{expectation}({fields},)"
        "}"
    )


def validate_source(main: str, extended: str) -> None:
    if len(main.splitlines()) > MAIN_MAX_LINES:
        raise GuardError("mcp_endpoints.rs exceeded the frozen source budget")
    if len(extended.splitlines()) > EXTENDED_MAX_LINES:
        raise GuardError("extended MCP dispatch tests exceeded the frozen source budget")
    include = 'include!("mcp_endpoints/extended_tool_dispatch_tests.rs");'
    if main.count(include) != 1:
        raise GuardError("extended MCP dispatch tests must be included exactly once")
    combined = main + "\n" + extended
    invocations = _invocations(combined)
    if len(invocations) != len(CASES):
        raise GuardError(f"expected {len(CASES)} alias rows, found {len(invocations)}")
    error_rows = sum(case[2] == "error" for case in CASES)
    success_rows = sum(case[2] == "success" for case in CASES)
    if (error_rows, success_rows) != (11, 9):
        raise GuardError("the protected matrix must retain 11 error and 9 success rows")
    for case in CASES:
        file_name, name, *_ = case
        source = main if file_name == "main" else extended
        if len(re.findall(rf"\b{re.escape(str(name))}\b", combined)) != 1:
            raise GuardError(f"{name}: expected exactly one source occurrence")
        matching = [row for row in invocations if f"async fn {name}" in row]
        if len(matching) != 1:
            raise GuardError(f"{name}: missing name-preserving macro row")
        if _normalized(matching[0]) != _normalized(_expected_invocation(case)):
            raise GuardError(f"{name}: request or assertion contract changed")
        if matching[0] not in source:
            raise GuardError(f"{name}: moved out of its original included file")
    for name in EXCLUDED_DIRECT_TESTS:
        marker = f"#[tokio::test]\nasync fn {name}"
        if combined.count(marker) != 1:
            raise GuardError(f"{name}: distinct dispatch path must remain bespoke")
    helper = _helper_region(main)
    for token in HELPER_TOKENS:
        if token not in helper:
            raise GuardError(f"MCP alias helper missing semantic token {token!r}")
    observed_hash = _normalized_hash(helper)
    if observed_hash != HELPER_HASH:
        raise GuardError(f"MCP alias helper semantic hash changed: {observed_hash}")


def _replace_once(source: str, old: str, new: str) -> str:
    if source.count(old) != 1:
        raise AssertionError(f"mutation preimage must occur once: {old!r}")
    return source.replace(old, new, 1)


def _replace_helper_once(main: str, old: str, new: str) -> str:
    helper = _helper_region(main)
    mutated_helper = _replace_once(helper, old, new)
    return main.replace(helper, mutated_helper, 1)


class McpAliasDispatchCaseMatrixSourceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.main = MAIN_PATH.read_text()
        cls.extended = EXTENDED_PATH.read_text()

    def test_current_source_preserves_alias_matrix(self) -> None:
        validate_source(self.main, self.extended)

    def test_name_mutation_is_rejected(self) -> None:
        name = str(CASES[0][1])
        mutated = _replace_once(self.main, name, f"{name}_mutated")
        with self.assertRaises(GuardError):
            validate_source(mutated, self.extended)

    def test_ordered_attribute_mutation_is_rejected(self) -> None:
        name = str(CASES[1][1])
        old = f"#[tokio::test]\n    async fn {name}"
        mutated = _replace_once(self.main, old, old.replace("tokio::test", "test"))
        with self.assertRaises(GuardError):
            validate_source(mutated, self.extended)

    def test_request_id_mutation_is_rejected(self) -> None:
        old = '1062233,\n        "iroha.subscriptions.get"'
        mutated = _replace_once(self.extended, old, old.replace("1062233", "1062234"))
        with self.assertRaises(GuardError):
            validate_source(self.main, mutated)

    def test_tool_name_mutation_is_rejected(self) -> None:
        old = '1061,\n        "iroha.transactions.status"'
        mutated = _replace_once(self.main, old, old.replace("status", "wait"))
        with self.assertRaises(GuardError):
            validate_source(mutated, self.extended)

    def test_argument_row_mutation_is_rejected(self) -> None:
        name = "mcp_jsonrpc_tools_call_agent_alias_rwas_get_accepts_flat_rwa_id"
        old = f"async fn {name} => error(\n        106158,\n        \"iroha.rwas.get\",\n        InvalidRwaId"
        mutated = _replace_once(self.main, old, old.replace("InvalidRwaId", "InvalidNftId"))
        with self.assertRaises(GuardError):
            validate_source(mutated, self.extended)

    def test_adversarial_literal_mutation_is_rejected(self) -> None:
        mutated = _replace_helper_once(
            self.main,
            'norito::json!({"asset_id": "not-an-asset-id"})',
            'norito::json!({"asset_id": "different-invalid-asset-id"})',
        )
        with self.assertRaises(GuardError):
            validate_source(mutated, self.extended)

    def test_error_category_mutation_is_rejected(self) -> None:
        name = str(CASES[0][1])
        old = f"async fn {name} => error("
        mutated = _replace_once(self.main, old, old.replace("error", "success"))
        with self.assertRaises(GuardError):
            validate_source(mutated, self.extended)

    def test_exact_error_message_mutation_is_rejected(self) -> None:
        old = "expected invalid definition id to be rejected by explorer definition detail alias"
        mutated = _replace_once(self.extended, old, old.replace("rejected", "accepted"))
        with self.assertRaises(GuardError):
            validate_source(self.main, mutated)

    def test_mcp_enablement_mutation_is_rejected(self) -> None:
        mutated = _replace_helper_once(
            self.main,
            "cfg.torii.mcp.enabled = true",
            "cfg.torii.mcp.enabled = false",
        )
        with self.assertRaises(GuardError):
            validate_source(mutated, self.extended)

    def test_router_binding_mutation_is_rejected(self) -> None:
        mutated = _replace_helper_once(
            self.main,
            "let app = build_router(cfg);",
            "let app = build_router(test_utils::mk_minimal_root_cfg());",
        )
        with self.assertRaises(GuardError):
            validate_source(mutated, self.extended)

    def test_response_threshold_mutation_is_rejected(self) -> None:
        mutated = _replace_helper_once(
            self.main,
            ".is_some_and(|status| status >= 400)",
            ".is_some_and(|status| status > 400)",
        )
        with self.assertRaises(GuardError):
            validate_source(mutated, self.extended)


if __name__ == "__main__":
    unittest.main()
