#!/usr/bin/env python3
"""Protect the typed default-executor visitor and permission-provider matrices."""

from __future__ import annotations

import hashlib
import json
import re
import unittest
from collections import Counter
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
DEFAULT_PATH = ROOT / "crates/iroha_executor/src/default/mod.rs"
PERMISSION_PATH = ROOT / "crates/iroha_executor/src/permission.rs"

ORIGINAL_RUST_LINES = 9_641
MAX_DEFAULT_LINES = 6_192
MAX_PERMISSION_LINES = 2_748
MINIMUM_RUST_LINE_SAVING = 701

TARGET_VISITOR_COUNT = 96
TARGET_VISITOR_KINDS = Counter(
    {"execute": 53, "no_op": 23, "orderbook": 12, "reserve": 8}
)
TARGET_VISITOR_SHA256 = (
    "747dcd6ac73f5e5c0f5a93becdf433f3ee5508ba61c6d8c5a166b995147ad758"
)
PUBLIC_VISITOR_ORDER_SHA256 = (
    "181086e92b61b89e67f7cef858845431115f1df2608c0c61b8baff5755137218"
)
MACRO_TOKEN_SHA256 = {
    "declare_execute_visitors": (
        "378f4ad517ab02e01afd099b001a5986b3f770b06958a7b6b78db5e617b62dde"
    ),
    "declare_query_visitors": (
        "1b1aefe607aa3e8c08ea245ddbf19a9deaa706717ce5c6fbbfb1e1f5c6a87479"
    ),
    "impl_validate_grant_revoke_via": (
        "8142ef6231d263f0145ef87597eaf77b4cfd759c2e51c6d5162c44e49dfaf0b2"
    ),
}

EXPECTED_PERMISSION_PROVIDERS = (
    ("query", "CanReadRestrictedDataspace", "OnlyGenesis::from"),
    ("query", "CanReadAllLedgerData", "OnlyGenesis::from"),
    ("executor", "CanUpgradeExecutor", "OnlyGenesis::from"),
    ("smart_contract", "CanRegisterSmartContractCode", "OnlyGenesis::from"),
    ("settlement", "CanManageFxCorridors", "OnlyGenesis::from"),
    ("peer", "CanManagePeers", "OnlyGenesis::from"),
    ("peer", "CanManageLaneRelayEmergency", "OnlyGenesis::from"),
    ("role", "CanManageRoles", "OnlyGenesis::from"),
    ("offline", "CanManageOfflineEscrow", "OnlyGenesis::from"),
    (
        "offline",
        "CanActivateKagemushaRecursiveReleaseV4",
        "OnlyGenesis::from",
    ),
    (
        "offline",
        "CanManageOfflineDeviceAttestationPolicy",
        "OnlyGenesis::from",
    ),
    (
        "asset",
        "CanMintAssetWithDefinition",
        "super::asset_definition::Owner::from",
    ),
    (
        "asset",
        "CanBurnAssetWithDefinition",
        "super::asset_definition::Owner::from",
    ),
    (
        "asset",
        "CanTransferAssetWithDefinition",
        "super::asset_definition::Owner::from",
    ),
    (
        "asset",
        "CanModifyAssetMetadataWithDefinition",
        "super::asset_definition::Owner::from",
    ),
    (
        "asset",
        "CanSetAssetTransferAvailability",
        "super::asset_definition::Owner::from",
    ),
    (
        "asset",
        "CanSetAssetTransferDailyLimit",
        "super::asset_definition::Owner::from",
    ),
    (
        "asset",
        "CanSetAssetHoldingLimit",
        "super::asset_definition::Owner::from",
    ),
    ("asset", "CanTransferAsset", "Owner::from"),
    ("asset_definition", "CanUnregisterAssetDefinition", "Owner::from"),
    (
        "asset_definition",
        "CanModifyAssetDefinitionMetadata",
        "Owner::from",
    ),
    (
        "asset_definition",
        "CanManageAssetDefinitionConfidentialPolicy",
        "Owner::from",
    ),
    ("nft", "CanRegisterNft", "super::domain::Owner::from"),
    ("account", "CanRegisterAccount", "super::domain::Owner::from"),
    ("account", "CanUnregisterAccount", "Owner::from"),
    ("account", "CanModifyAccountMetadata", "Owner::from"),
    ("account", "CanReplaceAccountController", "Owner::from"),
    ("account", "CanReadAccountData", "Owner::from"),
    (
        "trigger",
        "CanRegisterTrigger",
        "super::account::Owner::from",
    ),
    ("trigger", "CanExecuteTrigger", "Owner::from"),
    ("trigger", "CanUnregisterTrigger", "Owner::from"),
    ("trigger", "CanModifyTrigger", "Owner::from"),
    ("trigger", "CanModifyTriggerMetadata", "Owner::from"),
    ("domain", "CanRegisterDomain", "OnlyGenesis::from"),
    ("domain", "CanUnregisterDomain", "Owner::from"),
    ("domain", "CanModifyDomainMetadata", "Owner::from"),
)


def mask_rust(source: str) -> str:
    """Blank Rust comments and literals while retaining delimiters and newlines."""
    masked = list(source)
    index = 0
    state = "code"
    block_depth = 0
    raw_hashes = 0
    while index < len(source):
        char = source[index]
        if state == "code":
            if source.startswith("//", index):
                masked[index] = masked[index + 1] = " "
                index += 2
                state = "line_comment"
            elif source.startswith("/*", index):
                masked[index] = masked[index + 1] = " "
                index += 2
                block_depth = 1
                state = "block_comment"
            elif char == '"':
                masked[index] = " "
                index += 1
                state = "string"
            elif char == "'":
                lifetime = (
                    index + 1 < len(source)
                    and (source[index + 1].isalpha() or source[index + 1] == "_")
                    and not (index + 2 < len(source) and source[index + 2] == "'")
                )
                if lifetime:
                    index += 1
                else:
                    masked[index] = " "
                    index += 1
                    state = "character"
            elif char == "r":
                match = re.match(r'r(#+)?"', source[index:])
                if match:
                    opener = match.group(0)
                    raw_hashes = len(match.group(1) or "")
                    for offset in range(index, index + len(opener)):
                        masked[offset] = " "
                    index += len(opener)
                    state = "raw_string"
                else:
                    index += 1
            else:
                index += 1
        elif state == "line_comment":
            if char == "\n":
                state = "code"
            else:
                masked[index] = " "
            index += 1
        elif state == "block_comment":
            if source.startswith("/*", index):
                masked[index] = masked[index + 1] = " "
                index += 2
                block_depth += 1
            elif source.startswith("*/", index):
                masked[index] = masked[index + 1] = " "
                index += 2
                block_depth -= 1
                if block_depth == 0:
                    state = "code"
            else:
                if char != "\n":
                    masked[index] = " "
                index += 1
        elif state in {"string", "character"}:
            if char == "\\":
                masked[index] = " "
                if index + 1 < len(source):
                    masked[index + 1] = " "
                index += 2
            elif (state == "string" and char == '"') or (
                state == "character" and char == "'"
            ):
                masked[index] = " "
                index += 1
                state = "code"
            else:
                if char != "\n":
                    masked[index] = " "
                index += 1
        else:
            terminator = '"' + "#" * raw_hashes
            if source.startswith(terminator, index):
                for offset in range(index, index + len(terminator)):
                    masked[offset] = " "
                index += len(terminator)
                state = "code"
            else:
                if char != "\n":
                    masked[index] = " "
                index += 1
    if state not in {"code", "line_comment"}:
        raise AssertionError(f"unterminated Rust lexical state: {state}")
    return "".join(masked)


def matching_delimiter(
    masked: str, opening: int, left: str = "{", right: str = "}"
) -> int:
    depth = 0
    for index in range(opening, len(masked)):
        depth += (masked[index] == left) - (masked[index] == right)
        if depth == 0:
            return index
    raise AssertionError(f"unclosed {left!r} delimiter at byte {opening}")


def assert_balanced_rust_delimiters(source: str) -> None:
    masked = mask_rust(source)
    pairs = {"}": "{", "]": "[", ")": "("}
    stack: list[tuple[str, int]] = []
    for index, char in enumerate(masked):
        if char in "{[(":
            stack.append((char, index))
        elif char in pairs:
            if not stack or stack[-1][0] != pairs[char]:
                raise AssertionError(f"mismatched Rust delimiter {char!r} at byte {index}")
            stack.pop()
    if stack:
        char, index = stack[-1]
        raise AssertionError(f"unclosed Rust delimiter {char!r} at byte {index}")


def rust_tokens(source: str) -> list[str]:
    return re.findall(r"[A-Za-z_]\w*|::|->|=>|\.\.|\S", source)


def token_digest(source: str) -> str:
    return hashlib.sha256("\x1f".join(rust_tokens(source)).encode()).hexdigest()


def json_digest(value: object) -> str:
    payload = json.dumps(value, separators=(",", ":"), ensure_ascii=True)
    return hashlib.sha256(payload.encode()).hexdigest()


def macro_block(source: str, name: str) -> str:
    masked = mask_rust(source)
    match = re.search(rf"macro_rules!\s+{name}\s*\{{", masked)
    if not match:
        raise AssertionError(f"missing macro_rules! {name}")
    opening = masked.find("{", match.start(), match.end())
    closing = matching_delimiter(masked, opening)
    return source[match.start() : closing + 1]


def expand_visitor_macros(source: str) -> str:
    """Expand the two typed visitor macros into canonical source for comparison."""
    while True:
        masked = mask_rust(source)
        invocations: list[tuple[int, int, str, int]] = []
        for name in ("declare_execute_visitors", "declare_query_visitors"):
            for match in re.finditer(rf"\b{name}!\s*\{{", masked):
                opening = masked.find("{", match.start(), match.end())
                closing = matching_delimiter(masked, opening)
                invocations.append((match.start(), closing + 1, name, opening))
        if not invocations:
            return source
        start, end, name, opening = max(invocations)
        content = source[opening + 1 : end - 1]
        content_mask = mask_rust(content)
        if name == "declare_execute_visitors":
            cursor = 0
            behavior = "execute"
        else:
            header = re.match(
                r"[ \t\r\n]*(no_op|via[ \t]+(\w+))[ \t\r\n]*;",
                content_mask,
            )
            if not header:
                raise AssertionError("query visitor macro lacks a typed behavior header")
            cursor = header.end()
            behavior = "no_op" if header.group(1) == "no_op" else header.group(2)
        entries = list(
            re.finditer(r"(?m)^[ \t]*(visit_\w+)[ \t]*\(", content_mask)
        )
        if not entries:
            raise AssertionError(f"empty {name} invocation")
        pieces: list[str] = []
        for entry in entries:
            visitor = entry.group(1)
            paren = content_mask.find("(", entry.start(), entry.end())
            paren_end = matching_delimiter(content_mask, paren, "(", ")")
            terminator = paren_end + 1
            while terminator < len(content_mask) and content_mask[terminator].isspace():
                terminator += 1
            if terminator >= len(content_mask) or content_mask[terminator] != ";":
                raise AssertionError(f"unterminated visitor entry: {visitor}")
            query_type = " ".join(content[paren + 1 : paren_end].split())
            pieces.append(content[cursor : entry.start()])
            if behavior == "execute":
                arguments = f"executor: &mut V, isi: &{query_type},"
                body = "execute!(executor, isi);"
            elif behavior == "no_op":
                arguments = f"_executor: &mut V, _query: &{query_type},"
                body = ""
            else:
                arguments = f"executor: &mut V, _query: &{query_type},"
                body = f"{behavior}(executor);"
            pieces.append(
                f"pub fn {visitor}<V: Execute + Visit + ?Sized>({arguments}) "
                f"{{ {body} }}"
            )
            cursor = terminator + 1
        pieces.append(content[cursor:])
        source = source[:start] + "".join(pieces) + source[end:]


def preceding_attributes(source: str, position: int) -> list[str]:
    line_start = source.rfind("\n", 0, position) + 1
    cursor = line_start
    reversed_lines: list[str] = []
    while cursor:
        end = cursor - 1
        start = source.rfind("\n", 0, end) + 1
        line = source[start:end].strip()
        if line.startswith("///"):
            reversed_lines.append(line)
            cursor = start
            continue
        if line.endswith("]"):
            block = [line]
            block_cursor = start
            while not block[-1].startswith("#["):
                block_end = block_cursor - 1
                block_start = source.rfind("\n", 0, block_end) + 1
                block.append(source[block_start:block_end].strip())
                block_cursor = block_start
            reversed_lines.extend(block)
            cursor = block_cursor
            continue
        break
    return list(reversed(reversed_lines))


def canonical_tokens(source: str) -> list[str]:
    tokens = rust_tokens(source)
    return [
        token
        for index, token in enumerate(tokens)
        if not (token == "," and index + 1 < len(tokens) and tokens[index + 1] == ")")
    ]


def visitor_records(source: str) -> list[list[object]]:
    masked = mask_rust(source)
    modules: list[tuple[str, int, int]] = []
    for match in re.finditer(r"\bpub\s+mod\s+(\w+)\s*\{", masked):
        opening = masked.find("{", match.start(), match.end())
        modules.append(
            (match.group(1), opening, matching_delimiter(masked, opening))
        )

    def module_at(position: int) -> str:
        containing = [module for module in modules if module[1] < position < module[2]]
        if not containing:
            return "<root>"
        return min(containing, key=lambda module: module[2] - module[1])[0]

    records: list[list[object]] = []
    for match in re.finditer(r"\bpub\s+fn\s+(visit_\w+)\b", masked):
        opening = masked.find("{", match.end())
        if masked.find(";", match.end(), opening) >= 0:
            continue
        closing = matching_delimiter(masked, opening)
        body = canonical_tokens(source[opening + 1 : closing])
        if body and body[-1] == ";":
            body.pop()
        records.append(
            [
                module_at(match.start()),
                match.group(1),
                preceding_attributes(source, match.start()),
                canonical_tokens(source[match.start() : opening]),
                body,
            ]
        )
    return records


def visitor_kind(record: list[object]) -> str | None:
    body = record[-1]
    if body == ["execute", "!", "(", "executor", ",", "isi", ")"]:
        return "execute"
    if not body:
        return "no_op"
    if body == ["visit_orderbook_read", "(", "executor", ")"]:
        return "orderbook"
    if body == ["visit_reserve_read", "(", "executor", ")"]:
        return "reserve"
    return None


def validate_default_source(source: str) -> None:
    if len(source.splitlines()) > MAX_DEFAULT_LINES:
        raise AssertionError("default executor source exceeded its consolidated line ceiling")
    for name in ("declare_execute_visitors", "declare_query_visitors"):
        if token_digest(macro_block(source, name)) != MACRO_TOKEN_SHA256[name]:
            raise AssertionError(f"typed visitor macro changed: {name}")
    direct_targets = [record for record in visitor_records(source) if visitor_kind(record)]
    if direct_targets:
        raise AssertionError("typed visitor skeletons escaped their declaration macros")
    expanded = visitor_records(expand_visitor_macros(source))
    order = [[record[0], record[1]] for record in expanded]
    if json_digest(order) != PUBLIC_VISITOR_ORDER_SHA256:
        raise AssertionError("public visitor name/module order changed")
    targets = [record for record in expanded if visitor_kind(record)]
    if len(targets) != TARGET_VISITOR_COUNT:
        raise AssertionError("typed visitor inventory changed")
    kinds = Counter(visitor_kind(record) for record in targets)
    if kinds != TARGET_VISITOR_KINDS:
        raise AssertionError(f"typed visitor behavior counts changed: {kinds}")
    if json_digest(targets) != TARGET_VISITOR_SHA256:
        raise AssertionError("visitor names, types, attributes, docs, or bodies changed")


def provider_entries(source: str) -> tuple[tuple[str, str, str], ...]:
    masked = mask_rust(source)
    modules: list[tuple[str, int, int]] = []
    for match in re.finditer(r"\b(?:pub\s+)?mod\s+(\w+)\s*\{", masked):
        opening = masked.find("{", match.start(), match.end())
        modules.append(
            (match.group(1), opening, matching_delimiter(masked, opening))
        )

    def module_at(position: int) -> str:
        containing = [module for module in modules if module[1] < position < module[2]]
        if not containing:
            return "<root>"
        return min(containing, key=lambda module: module[2] - module[1])[0]

    entries: list[tuple[str, str, str]] = []
    for match in re.finditer(r"\bimpl_validate_grant_revoke_via!\s*\(", masked):
        opening = masked.find("(", match.start(), match.end())
        closing = matching_delimiter(masked, opening, "(", ")")
        content = source[opening + 1 : closing]
        provider, permission_list = content.split("=>", 1)
        provider = " ".join(provider.split())
        entries.extend(
            (module_at(match.start()), " ".join(permission.split()), provider)
            for permission in permission_list.split(",")
            if permission.strip()
        )
    return tuple(entries)


def validate_permission_source(source: str) -> None:
    if len(source.splitlines()) > MAX_PERMISSION_LINES:
        raise AssertionError("permission source exceeded its consolidated line ceiling")
    name = "impl_validate_grant_revoke_via"
    if token_digest(macro_block(source, name)) != MACRO_TOKEN_SHA256[name]:
        raise AssertionError("grant/revoke provider macro changed")
    if provider_entries(source) != EXPECTED_PERMISSION_PROVIDERS:
        raise AssertionError("grant/revoke permission/provider order changed")
    if "impl_genesis_only_offline_permission" in source:
        raise AssertionError("retired genesis-only provider macro returned")
    if "impl_asset_definition_control_permission" in source:
        raise AssertionError("retired asset-definition provider macro returned")


class ExecutorVisitorDelegationSourceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.default_source = DEFAULT_PATH.read_text(encoding="utf-8")
        cls.permission_source = PERMISSION_PATH.read_text(encoding="utf-8")

    def test_default_visitor_expansion_contract(self) -> None:
        validate_default_source(self.default_source)

    def test_permission_provider_contract(self) -> None:
        validate_permission_source(self.permission_source)

    def test_rust_line_saving_is_retained(self) -> None:
        current = len(self.default_source.splitlines()) + len(
            self.permission_source.splitlines()
        )
        self.assertGreaterEqual(ORIGINAL_RUST_LINES - current, MINIMUM_RUST_LINE_SAVING)

    def test_rust_delimiters_are_balanced(self) -> None:
        assert_balanced_rust_delimiters(self.default_source)
        assert_balanced_rust_delimiters(self.permission_source)

    def test_mutating_visitor_type_is_rejected(self) -> None:
        mutated = self.default_source.replace(
            "visit_register_provider_owner(RegisterProviderOwner);",
            "visit_register_provider_owner(UnregisterProviderOwner);",
            1,
        )
        with self.assertRaises(AssertionError):
            validate_default_source(mutated)

    def test_mutating_ordered_expect_attribute_is_rejected(self) -> None:
        mutated = self.default_source.replace(
            "the generated Visit dispatch ABI passes every query operation by shared reference",
            "mutated Visit dispatch ABI reason",
            1,
        )
        with self.assertRaises(AssertionError):
            validate_default_source(mutated)

    def test_mutating_query_helper_is_rejected(self) -> None:
        mutated = self.default_source.replace(
            "via visit_orderbook_read;", "via visit_reserve_read;", 1
        )
        with self.assertRaises(AssertionError):
            validate_default_source(mutated)

    def test_mutating_permission_provider_is_rejected(self) -> None:
        mutated = self.permission_source.replace(
            "impl_validate_grant_revoke_via!(OnlyGenesis::from => CanManageRoles);",
            "impl_validate_grant_revoke_via!(Owner::from => CanManageRoles);",
            1,
        )
        with self.assertRaises(AssertionError):
            validate_permission_source(mutated)

    def test_mutating_provider_body_is_rejected(self) -> None:
        mutated = self.permission_source.replace(
            "$provider(self).validate(authority, host, context)",
            "$provider(self).validate(authority, context, host)",
            1,
        )
        with self.assertRaises(AssertionError):
            validate_permission_source(mutated)


if __name__ == "__main__":
    unittest.main()
