from __future__ import annotations

import hashlib
import json
import re
import shutil
from pathlib import Path


SNAPSHOT = Path("/tmp/runtime-provider-broker-live-opening.yO3tqU")
OUTPUT = Path("/tmp/runtime-provider-broker-candidate")
MAIN = Path("runtime_provider_broker.rs")
COMPANION = Path("runtime_provider_broker")

EXTERNAL = {
    "broker_server_preserves_active_listener_without_lock_or_readiness",
    "broker_server_recovers_exact_stale_socket_after_unclean_exit",
    "broker_server_rejects_active_locked_socket_without_unlinking_it",
    "stale_socket_recovery_detects_identity_substitution_before_unlink",
    "orderly_cleanup_quarantines_before_detecting_identity_substitution",
    "broker_endpoint_rejects_socket_hardlink_alias_without_removal",
}

REGISTRY_FIRST_CASES = {
    (MAIN, "decode_policies_are_explicit_and_cover_supported_operation_frames"),
    (
        COMPANION / "api.rs",
        "standalone_registry_retains_exact_network_identity",
    ),
    (
        COMPANION / "api.rs",
        "stock_registry_whitelists_every_frozen_v1_slot",
    ),
    (
        COMPANION / "launcher.rs",
        "empty_catalog_is_rejected_without_backend_discovery",
    ),
    (
        COMPANION / "protocol_primitives.rs",
        "pop_runtime_open_is_public_and_legacy_operation_is_retired",
    ),
}

TEST_RE = re.compile(
    r"(?m)(?P<attrs>(?:^[ \t]*#\[[^\n]*\]\n)+)"
    r"(?P<indent>^[ \t]*)"
    r"(?P<signature>(?:(?:pub(?:\([^)]*\))?|async|const|unsafe)\s+)*"
    r"fn\s+(?P<name>[A-Za-z_][A-Za-z0-9_]*)\s*\(\s*\)\s*"
    r"(?:->\s*[^\{\n]+\s*)?\{)"
)
TEST_ATTR_RE = re.compile(r"#\[(?:(?:tokio|test_log)::)?test(?:\([^]]*\))?\]")
ASSERT_RE = re.compile(
    r"\b(?:assert|assert_eq|assert_ne|debug_assert|debug_assert_eq|debug_assert_ne)\s*!\s*([({\[])"
)


def sha(value: bytes | str) -> str:
    if isinstance(value, str):
        value = value.encode()
    return hashlib.sha256(value).hexdigest()


def skip_quoted(source: str, offset: int) -> int | None:
    raw = re.match(r'(?:br|r)(\#*)"', source[offset:])
    if raw:
        delimiter = '"' + raw.group(1)
        end = source.find(delimiter, offset + len(raw.group(0)))
        if end < 0:
            raise ValueError("unterminated raw string")
        return end + len(delimiter)

    prefix = 2 if source.startswith(('b"', 'c"'), offset) else 1
    if not (source[offset] == '"' or prefix == 2):
        return None
    cursor = offset + prefix
    while cursor < len(source):
        if source[cursor] == "\\":
            cursor += 2
        elif source[cursor] == '"':
            return cursor + 1
        else:
            cursor += 1
    raise ValueError("unterminated string")


def skip_char(source: str, offset: int) -> int | None:
    quote = offset + 1 if source.startswith("b'", offset) else offset
    if quote >= len(source) or source[quote] != "'":
        return None
    cursor = quote + 1
    while cursor < len(source) and source[cursor] != "\n" and cursor - quote <= 16:
        if source[cursor] == "\\":
            cursor += 2
        elif source[cursor] == "'":
            return cursor + 1
        else:
            cursor += 1
    return None


def matching_delimiter(source: str, offset: int) -> int:
    pairs = {"{": "}", "(": ")", "[": "]"}
    opening = source[offset]
    closing = pairs[opening]
    depth = 1
    cursor = offset + 1
    while cursor < len(source):
        if source.startswith("//", cursor):
            end = source.find("\n", cursor + 2)
            cursor = len(source) if end < 0 else end + 1
            continue
        if source.startswith("/*", cursor):
            comment_depth = 1
            cursor += 2
            while comment_depth:
                if source.startswith("/*", cursor):
                    comment_depth += 1
                    cursor += 2
                elif source.startswith("*/", cursor):
                    comment_depth -= 1
                    cursor += 2
                else:
                    cursor += 1
            continue
        quoted_end = skip_quoted(source, cursor)
        if quoted_end is not None:
            cursor = quoted_end
            continue
        char_end = skip_char(source, cursor)
        if char_end is not None:
            cursor = char_end
            continue
        if source[cursor] == opening:
            depth += 1
        elif source[cursor] == closing:
            depth -= 1
            if depth == 0:
                return cursor
        cursor += 1
    raise ValueError(f"unmatched delimiter at {offset}")


def minify(source: str) -> str:
    output: list[str] = []
    pending_space = False
    cursor = 0
    while cursor < len(source):
        if source[cursor].isspace():
            pending_space = True
            cursor += 1
            continue
        if source.startswith(("///", "//!", "/**", "/*!"), cursor):
            raise ValueError("doc comment inside logical case")
        if source.startswith("//", cursor):
            end = source.find("\n", cursor + 2)
            cursor = len(source) if end < 0 else end + 1
            pending_space = True
            continue
        if source.startswith("/*", cursor):
            depth = 1
            cursor += 2
            while depth:
                if source.startswith("/*", cursor):
                    depth += 1
                    cursor += 2
                elif source.startswith("*/", cursor):
                    depth -= 1
                    cursor += 2
                else:
                    cursor += 1
            pending_space = True
            continue
        quoted_end = skip_quoted(source, cursor)
        if quoted_end is not None:
            if pending_space and output:
                output.append(" ")
            output.append(source[cursor:quoted_end])
            pending_space = False
            cursor = quoted_end
            continue
        char_end = skip_char(source, cursor)
        if char_end is not None:
            if pending_space and output:
                output.append(" ")
            output.append(source[cursor:char_end])
            pending_space = False
            cursor = char_end
            continue
        if pending_space and output:
            output.append(" ")
        output.append(source[cursor])
        pending_space = False
        cursor += 1
    return "".join(output).strip()


def assertion_inventory(body: str) -> list[str]:
    rows: list[str] = []
    cursor = 0
    while match := ASSERT_RE.search(body, cursor):
        opening = match.start(1)
        closing = matching_delimiter(body, opening)
        rows.append(minify(body[match.start():closing + 1]))
        cursor = closing + 1
    return rows


def registry(indent: str, aggregates: dict[str, str], test_count: int) -> str:
    i = indent
    j = indent + "    "
    k = indent + "        "
    l = indent + "            "
    return f'''{i}const BROKER_LOGICAL_CASE_INVENTORY_SHA256: &str =
{j}"{aggregates['inventory']}";
{i}const BROKER_LOGICAL_CASE_NAME_INVENTORY_SHA256: &str =
{j}"{aggregates['names']}";
{i}const BROKER_LOGICAL_CASE_ATTR_INVENTORY_SHA256: &str =
{j}"{aggregates['attrs']}";
{i}const BROKER_LOGICAL_CASE_BODY_INVENTORY_SHA256: &str =
{j}"{aggregates['bodies']}";
{i}const BROKER_LOGICAL_CASE_ASSERTION_INVENTORY_SHA256: &str =
{j}"{aggregates['assertions']}";
{i}const BROKER_EXTERNAL_SELECTOR_INVENTORY_SHA256: &str =
{j}"{aggregates['external']}";
{i}const BROKER_OPENING_TEST_COUNT: usize = {test_count};

{i}trait BrokerLogicalCase {{
{j}const OPENING_NAME: &'static str;
{j}const ATTR_SHA256: &'static str;
{j}const BODY_SHA256: &'static str;
{j}const ASSERTION_SHA256: &'static str;
{j}const ASSERTION_COUNT: usize;
{i}}}

{i}macro_rules! broker_logical_case {{
{j}($(#[$attr:meta])* $id:ident, $opening_name:literal, $attr_sha:literal, $body_sha:literal, $assertion_sha:literal, $assertion_count:literal => $body:block) => {{
{k}#[allow(non_camel_case_types)]
{k}enum $id {{}}

{k}impl BrokerLogicalCase for $id {{
{l}const OPENING_NAME: &'static str = $opening_name;
{l}const ATTR_SHA256: &'static str = $attr_sha;
{l}const BODY_SHA256: &'static str = $body_sha;
{l}const ASSERTION_SHA256: &'static str = $assertion_sha;
{l}const ASSERTION_COUNT: usize = $assertion_count;
{k}}}

{k}$(#[$attr])*
{k}fn $id() {{
{l}let _logical_case_inventory = (
{l}    <$id as BrokerLogicalCase>::OPENING_NAME,
{l}    <$id as BrokerLogicalCase>::ATTR_SHA256,
{l}    <$id as BrokerLogicalCase>::BODY_SHA256,
{l}    <$id as BrokerLogicalCase>::ASSERTION_SHA256,
{l}    <$id as BrokerLogicalCase>::ASSERTION_COUNT,
{l}    BROKER_LOGICAL_CASE_INVENTORY_SHA256,
{l}    BROKER_LOGICAL_CASE_NAME_INVENTORY_SHA256,
{l}    BROKER_LOGICAL_CASE_ATTR_INVENTORY_SHA256,
{l}    BROKER_LOGICAL_CASE_BODY_INVENTORY_SHA256,
{l}    BROKER_LOGICAL_CASE_ASSERTION_INVENTORY_SHA256,
{l}    BROKER_EXTERNAL_SELECTOR_INVENTORY_SHA256,
{l}    BROKER_OPENING_TEST_COUNT,
{l});
{l}$body
{k}}}
{j}}};
{i}}}'''


def main() -> None:
    paths = [MAIN, *sorted((SNAPSHOT / COMPANION).rglob("*.rs"))]
    paths = [path.relative_to(SNAPSHOT) if path.is_absolute() else path for path in paths]
    sources = {path: (SNAPSHOT / path).read_text() for path in paths}
    records: list[dict[str, object]] = []
    for path in paths:
        source = sources[path]
        for match in TEST_RE.finditer(source):
            attrs = match.group("attrs")
            if not TEST_ATTR_RE.search(attrs):
                continue
            body_start = match.end("signature") - 1
            body_end = matching_delimiter(source, body_start)
            body = source[body_start:body_end + 1]
            assertions = assertion_inventory(body)
            records.append(
                {
                    "path": path,
                    "name": match.group("name"),
                    "start": match.start("attrs"),
                    "end": body_end + 1,
                    "indent": match.group("indent"),
                    "attrs": attrs,
                    "body": body,
                    "attrs_min": minify(attrs),
                    "body_min": minify(body),
                    "attr_sha": sha(minify(attrs)),
                    "body_sha": sha(minify(body)),
                    "assertion_sha": sha(json.dumps(assertions, separators=(",", ":"))),
                    "assertion_count": len(assertions),
                }
            )
    assert len(records) == 169, len(records)
    assert len({str(row["name"]) for row in records}) == 169
    assert EXTERNAL <= {str(row["name"]) for row in records}

    internal = [row for row in records if row["name"] not in EXTERNAL]
    for index, row in enumerate(internal, 1):
        row["id"] = f"broker_case_{index:04d}"
    assert len(internal) == 163

    cases = [
        [
            row["id"],
            row["name"],
            str(row["path"]),
            row["attr_sha"],
            row["body_sha"],
            row["assertion_sha"],
            row["assertion_count"],
        ]
        for row in internal
    ]
    aggregates = {
        "inventory": sha(json.dumps(cases, separators=(",", ":"))),
        "names": sha(json.dumps([row[1] for row in cases], separators=(",", ":"))),
        "attrs": sha(json.dumps([row[3] for row in cases], separators=(",", ":"))),
        "bodies": sha(json.dumps([row[4] for row in cases], separators=(",", ":"))),
        "assertions": sha(
            json.dumps([[row[5], row[6]] for row in cases], separators=(",", ":"))
        ),
        "external": sha(json.dumps(sorted(EXTERNAL), separators=(",", ":"))),
    }

    by_path: dict[Path, list[dict[str, object]]] = {}
    for row in internal:
        by_path.setdefault(row["path"], []).append(row)

    OUTPUT.mkdir(parents=True, exist_ok=True)
    for path, source in sources.items():
        candidate = source
        for row in sorted(by_path.get(path, []), key=lambda item: int(item["start"]), reverse=True):
            prefix = ""
            if (path, row["name"]) in REGISTRY_FIRST_CASES:
                prefix = registry(str(row["indent"]), aggregates, len(records)) + "\n\n"
            invocation = (
                f'{row["indent"]}broker_logical_case!('
                f'{row["attrs_min"]} {row["id"]}, '
                f'{json.dumps(row["name"])}, '
                f'"{row["attr_sha"]}", "{row["body_sha"]}", '
                f'"{row["assertion_sha"]}", {row["assertion_count"]} => '
                f'{row["body_min"]});'
            )
            candidate = candidate[: int(row["start"])] + prefix + invocation + candidate[int(row["end"]):]
        destination = OUTPUT / path
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_text(candidate)

    inventory = {
        "schema": 1,
        "opening_family_rust_lines": sum(source.count("\n") for source in sources.values()),
        "opening_hashes": {str(path): sha(source) for path, source in sources.items()},
        "aggregates": aggregates,
        "external": sorted(EXTERNAL),
        "cases": cases,
    }
    inventory_path = Path("/tmp/runtime-provider-broker-logical-inventory.json")
    inventory_path.write_text(json.dumps(inventory, indent=2, sort_keys=True) + "\n")
    print(f"tests={len(records)} internal={len(internal)} external={len(EXTERNAL)}")
    print(f"internal_assertions={sum(int(row['assertion_count']) for row in internal)}")
    for key, value in aggregates.items():
        print(f"{key}_sha256={value}")
    print(f"inventory_artifact_sha256={sha(inventory_path.read_bytes())}")


if __name__ == "__main__":
    main()
