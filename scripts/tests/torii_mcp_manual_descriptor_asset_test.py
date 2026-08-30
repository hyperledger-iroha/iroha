#!/usr/bin/env python3
"""Seal Torii's versioned static MCP descriptor asset and wrapper inventory."""

from __future__ import annotations

import hashlib
import json
import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SOURCE_PATH = ROOT / "crates/iroha_torii/src/mcp.rs"
ASSET_PATH = ROOT / "crates/iroha_torii/src/mcp/manual_tool_descriptors_v1.json"
EXPECTED_ASSET_LENGTH = 112_201
EXPECTED_ASSET_SHA256 = "ae0938c31351da71ec5e2099b92c942f5a57f5dfaf82018cd1f1dcede755431d"
EXPECTED_SEMANTIC_SHA256 = "11dcb0f72f368587eb73374987ab338686b3d4dfacb19afcfa99ee76fdbeba16"
EXPECTED_HISTORICAL_RUST_PREIMAGE_SHA256 = (
    "1273686f98de21c686573d399d511be7606155b9d09de21869a8c060436242b4"
)
EXPECTED_RETAINED_DIRECT_SHA256 = (
    "af338fada6916a39d324e9c907abebf5137bec028e9d88ad75f91c4ae3d4ffb3"
)
EXPECTED_LOADER_SOURCE_SHA256 = (
    "9757e4567f26eb9b10819d3595ff4c529f3b10124f88e02a09f51d401c67957e"
)
EXPECTED_BLAKE3_BYTES = (
    0xA2, 0xAC, 0x7A, 0x48, 0xD9, 0xFF, 0x12, 0x8C,
    0x4C, 0x36, 0xC8, 0x1C, 0x81, 0x2A, 0x68, 0xEA,
    0xB2, 0x9E, 0x53, 0x2F, 0x0A, 0xFF, 0xE2, 0x3B,
    0xC5, 0x41, 0x2A, 0xD5, 0xC0, 0x80, 0xDD, 0x31,
)
EXPECTED_WRAPPERS = (
    ('iroha_connect_ws_ticket_tool', 'iroha.connect.ws.ticket'),
    ('iroha_connect_session_create_tool', 'iroha.connect.session.create'),
    ('iroha_connect_session_delete_tool', 'iroha.connect.session.delete'),
    (
        "iroha_node_query_projection_checkpoint_plan_tool",
        "iroha.node.query_projection_checkpoint_plan",
    ),
    (
        "iroha_node_query_projection_checkpoint_publish_tool",
        "iroha.node.query_projection_checkpoint_publish",
    ),
    ('iroha_node_query_projection_shard_catalog_tool', 'iroha.node.query_projection_shard_catalog'),
    ('iroha_da_manifests_get_tool', 'iroha.da.manifests.get'),
    ('iroha_runtime_upgrades_activate_tool', 'iroha.runtime.upgrades.activate'),
    ('iroha_runtime_upgrades_cancel_tool', 'iroha.runtime.upgrades.cancel'),
    ('iroha_bridge_finality_proof_tool', 'iroha.bridge.finality.proof'),
    ('iroha_bridge_finality_bundle_tool', 'iroha.bridge.finality.bundle'),
    ('iroha_gov_contract_get_tool', 'iroha.gov.contract.get'),
    ('iroha_aliases_resolve_tool', 'iroha.aliases.resolve'),
    ('iroha_aliases_resolve_index_tool', 'iroha.aliases.resolve_index'),
    ('iroha_aliases_by_account_tool', 'iroha.aliases.by_account'),
    ('iroha_contracts_code_get_tool', 'iroha.contracts.code.get'),
    ('iroha_contracts_code_bytes_get_tool', 'iroha.contracts.code.bytes.get'),
    ('iroha_contracts_call_and_wait_tool', 'iroha.contracts.call_and_wait'),
    ('iroha_contracts_state_get_tool', 'iroha.contracts.state.get'),
    ('iroha_accounts_list_tool', 'iroha.accounts.list'),
    ('iroha_accounts_get_tool', 'iroha.accounts.get'),
    ('iroha_accounts_qr_tool', 'iroha.accounts.qr'),
    ('iroha_accounts_query_tool', 'iroha.accounts.query'),
    ('iroha_accounts_onboard_plan_tool', 'iroha.accounts.onboard.plan'),
    ('iroha_accounts_onboard_prepare_tool', 'iroha.accounts.onboard.prepare'),
    ('iroha_accounts_onboard_submit_tool', 'iroha.accounts.onboard.submit'),
    ('iroha_account_transactions_tool', 'iroha.accounts.transactions'),
    ('iroha_account_history_tool', 'iroha.accounts.history'),
    ('iroha_account_transactions_query_tool', 'iroha.accounts.transactions.query'),
    ('iroha_account_assets_tool', 'iroha.accounts.assets'),
    ('iroha_account_assets_query_tool', 'iroha.accounts.assets.query'),
    ('iroha_account_permissions_tool', 'iroha.accounts.permissions'),
    ('iroha_account_portfolio_tool', 'iroha.accounts.portfolio'),
    ('iroha_domains_list_tool', 'iroha.domains.list'),
    ('iroha_domains_get_tool', 'iroha.domains.get'),
    ('iroha_domains_query_tool', 'iroha.domains.query'),
    ('iroha_subscriptions_plans_list_tool', 'iroha.subscriptions.plans.list'),
    ('iroha_subscriptions_plans_create_tool', 'iroha.subscriptions.plans.create'),
    ('iroha_subscriptions_list_tool', 'iroha.subscriptions.list'),
    ('iroha_subscriptions_create_tool', 'iroha.subscriptions.create'),
    ('iroha_subscriptions_get_tool', 'iroha.subscriptions.get'),
    ('iroha_asset_definitions_tool', 'iroha.assets.definitions'),
    ('iroha_asset_definitions_get_tool', 'iroha.assets.definitions.get'),
    ('iroha_asset_definitions_query_tool', 'iroha.assets.definitions.query'),
    ('iroha_asset_holders_tool', 'iroha.assets.holders'),
    ('iroha_asset_holders_query_tool', 'iroha.assets.holders.query'),
    ('iroha_assets_list_tool', 'iroha.assets.list'),
    ('iroha_assets_get_tool', 'iroha.assets.get'),
    ('iroha_nfts_list_tool', 'iroha.nfts.list'),
    ('iroha_nfts_get_tool', 'iroha.nfts.get'),
    ('iroha_nfts_query_tool', 'iroha.nfts.query'),
    ('iroha_rwas_list_tool', 'iroha.rwas.list'),
    ('iroha_rwas_get_tool', 'iroha.rwas.get'),
    ('iroha_rwas_query_tool', 'iroha.rwas.query'),
    ('iroha_transactions_list_tool', 'iroha.transactions.list'),
    ('iroha_transactions_get_tool', 'iroha.transactions.get'),
    ('iroha_instructions_list_tool', 'iroha.instructions.list'),
    ('iroha_instructions_get_tool', 'iroha.instructions.get'),
    ('iroha_blocks_list_tool', 'iroha.blocks.list'),
    ('iroha_blocks_get_tool', 'iroha.blocks.get'),
    ('iroha_transactions_wait_tool', 'iroha.transactions.wait'),
    ('iroha_transactions_status_tool', 'iroha.transactions.status'),
)
RETAINED_DIRECT_BUILDERS = (
    "iroha_vpn_quotes_create_tool",
    "iroha_vpn_sessions_create_tool",
    "iroha_vpn_sessions_get_tool",
    "iroha_vpn_receipts_submit_tool",
    "iroha_vpn_receipts_list_tool",
    "iroha_gov_proposals_get_tool",
    "iroha_gov_locks_get_tool",
    "iroha_gov_referenda_get_tool",
    "iroha_gov_tally_get_tool",
    "iroha_queries_submit_tool",
    "iroha_transactions_submit_tool",
    "iroha_transactions_submit_and_wait_tool",
)
EXPECTED_RECORD_KEYS = (
    "function",
    "name",
    "effect",
    "description",
    "method",
    "path_template",
    "input_schema",
)


class GuardError(AssertionError):
    """The static descriptor asset or its source projection drifted."""


def _strict_object(pairs: list[tuple[str, object]]) -> dict[str, object]:
    result: dict[str, object] = {}
    for key, value in pairs:
        if key in result:
            raise GuardError(f"duplicate JSON object key: {key}")
        result[key] = value
    return result


def _normalized_rust_tokens(source: str) -> bytes:
    """Discard Rust layout and comments while preserving literal bytes."""

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


def _parse_asset(asset_bytes: bytes) -> dict[str, object]:
    if len(asset_bytes) != EXPECTED_ASSET_LENGTH:
        raise GuardError("descriptor asset byte length drifted")
    if _sha256(asset_bytes) != EXPECTED_ASSET_SHA256:
        raise GuardError("descriptor asset byte digest drifted")
    try:
        asset = json.loads(asset_bytes, object_pairs_hook=_strict_object)
    except (GuardError, json.JSONDecodeError) as error:
        raise GuardError(f"descriptor asset is not strict JSON: {error}") from error
    if not isinstance(asset, dict):
        raise GuardError("descriptor asset root is not an object")
    if tuple(asset) != (
        "schema_version",
        "historical_rust_preimage_sha256",
        "descriptors",
    ):
        raise GuardError("descriptor asset root field order drifted")
    if type(asset["schema_version"]) is not int or asset["schema_version"] != 1:
        raise GuardError("descriptor asset version drifted")
    if (
        asset["historical_rust_preimage_sha256"]
        != EXPECTED_HISTORICAL_RUST_PREIMAGE_SHA256
    ):
        raise GuardError("historical Rust preimage digest drifted")
    descriptors = asset["descriptors"]
    if not isinstance(descriptors, list) or len(descriptors) != len(EXPECTED_WRAPPERS):
        raise GuardError("descriptor asset inventory size drifted")
    seen_functions: set[str] = set()
    seen_names: set[str] = set()
    actual_inventory: list[tuple[str, str]] = []
    for index, descriptor in enumerate(descriptors):
        if not isinstance(descriptor, dict) or tuple(descriptor) != EXPECTED_RECORD_KEYS:
            raise GuardError(f"descriptor record {index} fields/order drifted")
        function = descriptor["function"]
        name = descriptor["name"]
        if not isinstance(function, str) or not re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", function):
            raise GuardError(f"descriptor record {index} function is invalid")
        if not isinstance(name, str) or not name:
            raise GuardError(f"descriptor record {index} name is invalid")
        if function in seen_functions or name in seen_names:
            raise GuardError(f"descriptor record {index} duplicates a function or name")
        seen_functions.add(function)
        seen_names.add(name)
        if descriptor["effect"] not in {"read", "build_instruction", "write", "operator"}:
            raise GuardError(f"descriptor record {index} effect is invalid")
        if not isinstance(descriptor["description"], str) or not descriptor["description"]:
            raise GuardError(f"descriptor record {index} description is invalid")
        if descriptor["method"] not in {"GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"}:
            raise GuardError(f"descriptor record {index} method is invalid")
        path = descriptor["path_template"]
        if not isinstance(path, str) or not path.startswith("/"):
            raise GuardError(f"descriptor record {index} path is invalid")
        if not isinstance(descriptor["input_schema"], dict):
            raise GuardError(f"descriptor record {index} schema is not an object")
        actual_inventory.append((function, name))
    if tuple(actual_inventory) != EXPECTED_WRAPPERS:
        raise GuardError("descriptor function/name/order inventory drifted")
    canonical = json.dumps(
        asset,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
    ).encode()
    if _sha256(canonical) != EXPECTED_SEMANTIC_SHA256:
        raise GuardError("descriptor semantic projection drifted")
    return asset


def _extract_direct_builder(source: str, function: str) -> str:
    match = re.search(
        rf"(?m)^fn {re.escape(function)}\(\) -> ToolSpec \{{",
        source,
    )
    if match is None:
        raise GuardError(f"retained direct builder `{function}` is missing or changed signature")
    opening = source.index("{", match.start())
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
                index += 1
                continue
            if char == "{":
                depth += 1
            elif char == "}":
                depth -= 1
                if depth == 0:
                    return source[match.start() : index + 1]
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
    raise GuardError(f"retained direct builder `{function}` is unterminated")


def validate(source: str, asset_bytes: bytes) -> None:
    asset = _parse_asset(asset_bytes)
    loader_start = source.find("const MANUAL_STATIC_TOOL_ASSET_VERSION")
    first_wrapper = source.find("manual_tool! {", loader_start)
    if loader_start < 0 or first_wrapper <= loader_start:
        raise GuardError("static descriptor loader boundaries drifted")
    loader = source[loader_start:first_wrapper]
    if _sha256(_normalized_rust_tokens(loader)) != EXPECTED_LOADER_SOURCE_SHA256:
        raise GuardError("static descriptor loader or wrapper macro drifted")

    digest_match = re.search(
        r"const MANUAL_STATIC_TOOL_ASSET_BLAKE3: \[u8; 32\] = \[(.*?)\];",
        source,
        re.DOTALL,
    )
    if digest_match is None:
        raise GuardError("runtime BLAKE3 seal is missing")
    digest_bytes = tuple(
        int(token, 16) for token in re.findall(r"0x[0-9a-fA-F]{2}", digest_match.group(1))
    )
    if digest_bytes != EXPECTED_BLAKE3_BYTES:
        raise GuardError("runtime BLAKE3 seal drifted")

    wrapper_pattern = re.compile(
        r"\bmanual_tool!\s*(?:"
        r"\(\s*([A-Za-z_][A-Za-z0-9_]*)\s*,\s*"
        r'("(?:\\.|[^"\\])*")\s*\);|'
        r"\{(.*?)\})",
        re.DOTALL,
    )
    group_entry_pattern = re.compile(
        r"([A-Za-z_][A-Za-z0-9_]*)\s*=>\s*"
        r'("(?:\\.|[^"\\])*")\s*;',
        re.DOTALL,
    )
    wrappers_list: list[tuple[str, str]] = []
    for match in wrapper_pattern.finditer(source):
        if match.group(1) is not None:
            wrappers_list.append((match.group(1), json.loads(match.group(2))))
            continue
        group = match.group(3)
        assert group is not None
        entries = tuple(group_entry_pattern.finditer(group))
        cursor = 0
        if not entries:
            raise GuardError("manual wrapper group syntax drifted")
        for entry in entries:
            if group[cursor : entry.start()].strip():
                raise GuardError("manual wrapper group syntax drifted")
            cursor = entry.end()
        if group[cursor:].strip():
            raise GuardError("manual wrapper group syntax drifted")
        wrappers_list.extend(
            (entry.group(1), json.loads(entry.group(2))) for entry in entries
        )
    wrappers = tuple(wrappers_list)
    if wrappers != EXPECTED_WRAPPERS:
        raise GuardError("source wrapper function/name/order inventory drifted")
    asset_inventory = tuple(
        (record["function"], record["name"])
        for record in asset["descriptors"]
    )
    if wrappers != asset_inventory:
        raise GuardError("source wrappers and descriptor asset diverged")
    for function, _ in EXPECTED_WRAPPERS:
        if re.search(rf"(?m)^fn {re.escape(function)}\(", source):
            raise GuardError(f"asset-backed builder `{function}` escaped its wrapper macro")

    catalog_calls = tuple(
        match.group(1)
        for match in re.finditer(
            r"tools\.push\(([A-Za-z_][A-Za-z0-9_]*)\(\)\);",
            source,
        )
        if match.group(1) in {function for function, _ in EXPECTED_WRAPPERS}
    )
    if catalog_calls != tuple(function for function, _ in EXPECTED_WRAPPERS):
        raise GuardError("asset-backed manual tool catalog order drifted")

    retained = b"\0".join(
        _normalized_rust_tokens(_extract_direct_builder(source, function))
        for function in RETAINED_DIRECT_BUILDERS
    )
    if _sha256(retained) != EXPECTED_RETAINED_DIRECT_SHA256:
        raise GuardError("dynamic/postprocessed direct builder preimage drifted")


class ToriiMcpManualDescriptorAssetTest(unittest.TestCase):
    """Exercise the descriptor seal and representative fail-closed mutations."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.source = SOURCE_PATH.read_text(encoding="utf-8")
        cls.asset = ASSET_PATH.read_bytes()

    def test_current_asset_and_source_match_the_historical_inventory(self) -> None:
        validate(self.source, self.asset)

    def test_prepared_account_schemas_are_closed_and_fully_typed(self) -> None:
        asset = json.loads(self.asset)
        descriptors = {
            descriptor["name"]: descriptor for descriptor in asset["descriptors"]
        }
        names = (
            "iroha.accounts.onboard.prepare",
            "iroha.accounts.onboard.submit",
        )

        def assert_exact_schema(schema: object, path: str) -> None:
            self.assertIsInstance(schema, dict, f"{path} schema must be an object")
            assert isinstance(schema, dict)
            self.assertTrue(schema, f"{path} must not admit arbitrary JSON")
            if schema.get("type") == "object":
                self.assertIs(
                    schema.get("additionalProperties"),
                    False,
                    f"{path} must reject unknown fields",
                )
                properties = schema.get("properties")
                required = schema.get("required")
                self.assertIsInstance(properties, dict, f"{path}.properties")
                self.assertIsInstance(required, list, f"{path}.required")
                assert isinstance(properties, dict)
                assert isinstance(required, list)
                self.assertEqual(
                    set(required),
                    set(properties),
                    f"{path} must require every first-release slot",
                )
                for field, child in properties.items():
                    assert_exact_schema(child, f"{path}.{field}")
            if "items" in schema:
                assert_exact_schema(schema["items"], f"{path}.items")
            for keyword in ("oneOf", "anyOf", "allOf"):
                if keyword not in schema:
                    continue
                branches = schema[keyword]
                self.assertIsInstance(branches, list, f"{path}.{keyword}")
                self.assertTrue(branches, f"{path}.{keyword} must not be empty")
                for index, branch in enumerate(branches):
                    assert_exact_schema(branch, f"{path}.{keyword}[{index}]")

        for name in names:
            schema = descriptors[name]["input_schema"]
            self.assertIs(schema.get("x-iroha-mcp-strict-body"), True)
            body = schema["properties"]["body"]
            assert_exact_schema(body, f"{name}.body")
            fee = body["properties"]["fee_payment"]
            payer_branches = fee.get("oneOf")
            self.assertEqual(
                [branch["properties"]["payer"]["const"] for branch in payer_branches],
                ["authority", "sponsor"],
            )

        for name in names[:2]:
            receipt_body = descriptors[name]["input_schema"]["properties"]["body"][
                "properties"
            ]["receipt"]["properties"]["body"]
            resource = receipt_body["properties"]["resource"]["properties"]
            self.assertEqual(
                resource["intent"]["properties"]["kind"]["const"],
                "account_alias",
            )
            for nullable in (
                resource["quote"],
                resource["instruction_index"],
                receipt_body["properties"]["acquisition"]["properties"][
                    "pricing_class_hint"
                ],
                receipt_body["properties"]["owner_auto_renew_instruction"],
            ):
                self.assertIn("null", {branch.get("type") for branch in nullable["oneOf"]})

    def test_explorer_history_descriptors_use_snapshot_cursor_contract(self) -> None:
        asset = json.loads(self.asset)
        descriptors = {
            descriptor["name"]: descriptor for descriptor in asset["descriptors"]
        }
        expected_query_fields = {
            "iroha.transactions.list": {
                "cursor", "limit", "authority", "block", "status", "asset_id",
            },
            "iroha.instructions.list": {
                "cursor", "limit", "authority", "account", "transaction_hash",
                "transaction_status", "block", "kind", "asset_id",
            },
            "iroha.blocks.list": {"cursor", "limit"},
        }
        for name, query_fields in expected_query_fields.items():
            schema = descriptors[name]["input_schema"]
            self.assertIs(schema.get("additionalProperties"), False, name)
            properties = schema.get("properties")
            self.assertIsInstance(properties, dict, name)
            assert isinstance(properties, dict)
            self.assertEqual(
                set(properties),
                query_fields | {"query", "headers", "accept"},
                name,
            )
            query = properties["query"]
            self.assertIs(query.get("additionalProperties"), False, name)
            self.assertEqual(set(query.get("properties", {})), query_fields, name)
            for owner in (properties, query["properties"]):
                self.assertFalse(
                    {"page", "per_page", "offset"} & set(owner),
                    f"{name} retains offset pagination",
                )
                cursor = owner["cursor"]
                self.assertEqual(cursor.get("pattern"), "^[A-Za-z0-9_-]+$")
                self.assertEqual(cursor.get("minLength"), 1)
                self.assertEqual(cursor.get("maxLength"), 1424)
                limit = owner["limit"]
                self.assertEqual(
                    (limit.get("minimum"), limit.get("maximum"), limit.get("default")),
                    (1, 100, 25),
                )
            self.assertIn("Optional canonical target authentication", descriptors[name]["description"])

    def test_source_mutations_fail_closed(self) -> None:
        mutations = (
            (
                'iroha_connect_ws_ticket_tool => "iroha.connect.ws.ticket";',
                'iroha_connect_ws_ticket_tool => "iroha.connect.session.create";',
            ),
            (
                'effect: descriptor.effect,',
                'effect: ToolEffect::Read,',
            ),
            (
                'include_bytes!("mcp/manual_tool_descriptors_v1.json")',
                'include_bytes!("mcp/manual_tool_descriptors_v2.json")',
            ),
            (
                '"Create a Sora VPN XOR escrow quote.',
                '"Alter a Sora VPN XOR escrow quote.',
            ),
            (
                'tools.push(iroha_connect_ws_ticket_tool());',
                'tools.push(iroha_connect_session_create_tool());',
            ),
        )
        for old, new in mutations:
            with self.subTest(old=old, new=new):
                self.assertIn(old, self.source)
                with self.assertRaises(GuardError):
                    validate(self.source.replace(old, new, 1), self.asset)

    def test_asset_mutations_fail_closed(self) -> None:
        mutations = (
            self.asset[:-1],
            self.asset.replace(b'"schema_version": 1', b'"schema_version": 2', 1),
            self.asset.replace(b'"effect": "read"', b'"effect": "write"', 1),
            self.asset.replace(b'"type": "object"', b'"type": "array" ', 1),
        )
        for mutated in mutations:
            with self.subTest(digest=hashlib.sha256(mutated).hexdigest()):
                with self.assertRaises(GuardError):
                    validate(self.source, mutated)


if __name__ == "__main__":
    unittest.main()
