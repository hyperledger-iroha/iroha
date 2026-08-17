#!/usr/bin/env python3
"""Guard the typed telemetry metric catalog against semantic source drift.

The pinned ledger was derived from the pre-compaction ``Metrics::default``
implementation at commit 4da39b000ea89fbedc0d27f41cd5652aa74c85c2.  It covers every catalog row's
construction order, Rust field, Prometheus kind, labels, buckets, registration
state, exported name, and help text.  The guard intentionally uses only the
Python standard library.
"""

from __future__ import annotations

import argparse
import collections
import hashlib
import json
import re
from pathlib import Path

CATALOG = Path("crates/iroha_telemetry/src/metrics/catalog_v2.tsv")
SOURCE = Path("crates/iroha_telemetry/src/metrics.rs")
HEADER = "# iroha-telemetry-metric-catalog-v2"
CATALOG_BYTES = 118_432
CATALOG_SHA256 = "ac46f3540b926a820548f8778c32ae4a14edbac7b87293fd404ed899d0c70b67"
CATALOG_BLAKE3 = "c5e260ec638bceaa33a48883ca6648d40b050be553f722e144f6cbfad1e2b392"
ROWS = 870
REGISTERED = 825
LEDGER_BYTES = 257_634
LEDGER_SHA256 = "6430932e23630d307a05ec418c8c1fab9094c01b5f52535065e048cf845b2d8a"
FACTORY_TOKENS_SHA256 = "04babf52a267d978cd3e4061fa6cec9e1e1e402f664db4137b8685c8fa3c76a1"
POST_CATALOG_TOKENS_SHA256 = "af55c0a26167324571cc34f27eb0c0e8bd0248b4dd4706a0b1023654d5b70644"
METHOD_COUNTS = {
    "float_counter_vec": 6,
    "float_gauge": 11,
    "float_gauge_vec": 34,
    "gauge": 276,
    "gauge_vec": 101,
    "histogram_vec": 2,
    "histogram_vec_with_buckets": 52,
    "histogram_with_buckets": 31,
    "int_counter": 90,
    "int_counter_vec": 232,
    "int_gauge": 12,
    "int_gauge_vec": 23,
}

FACTORY_START = "#[derive(Clone, Copy)]\nstruct MetricSpec {"
FACTORY_END = "#[cfg(test)]\nmod metric_catalog_tests {"
CONSTRUCTION_START = "let mut metrics = MetricFactory::new(&registry, &mut metric_specs);"
CONSTRUCTION_END = "metrics.finish();"
INITIALIZER_START = "        let metrics = Self {"


class GuardError(ValueError):
    """Raised when the bounded Rust grammar used by this guard is violated."""


def _matching_paren(text: str, opening: int) -> int:
    """Return the matching close parenthesis, respecting strings and comments."""
    depth = 0
    quote: str | None = None
    escaped = False
    line_comment = False
    block_depth = 0
    index = opening
    while index < len(text):
        char = text[index]
        pair = text[index : index + 2]
        if line_comment:
            if char == "\n":
                line_comment = False
        elif block_depth:
            if pair == "/*":
                block_depth += 1
                index += 1
            elif pair == "*/":
                block_depth -= 1
                index += 1
        elif quote:
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == quote:
                quote = None
        elif pair == "//":
            line_comment = True
            index += 1
        elif pair == "/*":
            block_depth = 1
            index += 1
        elif char == '"':
            quote = char
        elif char == "(":
            depth += 1
        elif char == ")":
            depth -= 1
            if depth == 0:
                return index
        index += 1
    raise GuardError(f"unclosed parenthesis at byte {opening}")


def _split_top_level(arguments: str) -> list[str]:
    """Split a Rust argument list without splitting nested expressions."""
    parts: list[str] = []
    start = 0
    stack: list[str] = []
    quote: str | None = None
    escaped = False
    line_comment = False
    block_depth = 0
    pairs = {")": "(", "]": "[", "}": "{"}
    index = 0
    while index < len(arguments):
        char = arguments[index]
        pair = arguments[index : index + 2]
        if line_comment:
            if char == "\n":
                line_comment = False
        elif block_depth:
            if pair == "/*":
                block_depth += 1
                index += 1
            elif pair == "*/":
                block_depth -= 1
                index += 1
        elif quote:
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == quote:
                quote = None
        elif pair == "//":
            line_comment = True
            index += 1
        elif pair == "/*":
            block_depth = 1
            index += 1
        elif char == '"':
            quote = char
        elif char in "([{":
            stack.append(char)
        elif char in ")]}" and stack:
            expected = pairs[char]
            if stack[-1] != expected:
                raise GuardError("unbalanced Rust argument delimiters")
            stack.pop()
        elif char == "," and not stack:
            parts.append(arguments[start:index])
            start = index + 1
        index += 1
    parts.append(arguments[start:])
    while parts and not _compact_rust(parts[-1]):
        parts.pop()
    return parts


def _compact_rust(text: str) -> str:
    """Remove comments and insignificant whitespace outside string literals."""
    output: list[str] = []
    quote: str | None = None
    escaped = False
    line_comment = False
    block_depth = 0
    index = 0
    while index < len(text):
        char = text[index]
        pair = text[index : index + 2]
        if line_comment:
            if char == "\n":
                line_comment = False
        elif block_depth:
            if pair == "/*":
                block_depth += 1
                index += 1
            elif pair == "*/":
                block_depth -= 1
                index += 1
        elif quote:
            output.append(char)
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == quote:
                quote = None
        elif pair == "//":
            line_comment = True
            index += 1
        elif pair == "/*":
            block_depth = 1
            index += 1
        elif char == '"':
            quote = char
            output.append(char)
        elif not char.isspace():
            output.append(char)
        index += 1
    if quote or block_depth:
        raise GuardError("unterminated string or block comment")
    return "".join(output)


def _canonical_expression(text: str) -> str:
    """Canonicalize an expression, including semantically inert trailing commas."""
    value = _compact_rust(text).rstrip(",")
    previous = None
    while previous != value:
        previous = value
        value = re.sub(r",(?=[]})])", "", value)
    return value


def _catalog_rows(raw: bytes) -> tuple[list[tuple[str, str, str, bool]], list[str]]:
    findings: list[str] = []
    if len(raw) != CATALOG_BYTES:
        findings.append(f"catalog bytes: expected {CATALOG_BYTES}, got {len(raw)}")
    digest = hashlib.sha256(raw).hexdigest()
    if digest != CATALOG_SHA256:
        findings.append(f"catalog sha256: expected {CATALOG_SHA256}, got {digest}")
    try:
        lines = raw.decode("utf-8").splitlines()
    except UnicodeDecodeError as error:
        return [], findings + [f"catalog is not UTF-8: {error}"]
    if not lines or lines[0] != HEADER:
        return [], findings + ["catalog header/version changed"]

    rows: list[tuple[str, str, str, bool]] = []
    keys: set[str] = set()
    names: set[str] = set()
    registered = 0
    for ordinal, line in enumerate(lines[1:], 1):
        fields = line.split("\t")
        if len(fields) != 4:
            findings.append(f"catalog row {ordinal} has {len(fields)} fields, expected 4")
            continue
        key, name, help_text, registration = fields
        if not key or not name or not help_text:
            findings.append(f"catalog row {ordinal} has an empty required field")
        if key in keys:
            findings.append(f"duplicate metric key `{key}`")
        if name in names:
            findings.append(f"duplicate metric name `{name}`")
        keys.add(key)
        names.add(name)
        if registration == "registered":
            is_registered = True
            registered += 1
        elif registration == "unregistered":
            is_registered = False
        else:
            findings.append(f"catalog row {ordinal} has invalid registration `{registration}`")
            is_registered = False
        rows.append((key, name, help_text, is_registered))
    if len(rows) != ROWS:
        findings.append(f"catalog rows: expected {ROWS}, got {len(rows)}")
    if registered != REGISTERED:
        findings.append(f"registered rows: expected {REGISTERED}, got {registered}")
    return rows, findings


def _factory_calls(source: str) -> tuple[list[tuple[str, str, str, str, str]], list[str]]:
    findings: list[str] = []
    if source.count(CONSTRUCTION_START) != 1 or source.count(CONSTRUCTION_END) != 1:
        return [], ["typed factory construction boundary changed"]
    construction = source.split(CONSTRUCTION_START, 1)[1].split(CONSTRUCTION_END, 1)[0]
    calls: list[tuple[str, str, str, str, str]] = []
    pattern = re.compile(r"\bmetrics\s*\.\s*([a-z_]+)\s*\(")
    scalar_methods = {"float_gauge", "gauge", "int_counter", "int_gauge"}
    vector_methods = {
        "float_counter_vec",
        "float_gauge_vec",
        "gauge_vec",
        "histogram_vec",
        "int_counter_vec",
        "int_gauge_vec",
    }
    try:
        for match in pattern.finditer(construction):
            method = match.group(1)
            opening = construction.find("(", match.start())
            closing = _matching_paren(construction, opening)
            arguments = _split_top_level(construction[opening + 1 : closing])
            if not arguments:
                raise GuardError(f"factory method `{method}` has no key")
            key_match = re.fullmatch(r'\s*"([^"\\]+)"\s*', arguments[0])
            if not key_match:
                raise GuardError(f"factory method `{method}` has a non-literal key")
            key = key_match.group(1)
            statement_start = construction.rfind("\n        let ", 0, match.start()) + 1
            statement_end = construction.find(";\n", match.start())
            if statement_start <= 0 or statement_end < 0:
                raise GuardError(f"factory call `{key}` is not a bounded let statement")
            statement = construction[statement_start : statement_end + 1]
            variable_match = re.match(r"\s*let\s+(?:mut\s+)?([A-Za-z_]\w*)", statement)
            if not variable_match:
                raise GuardError(f"factory call `{key}` has no simple binding")
            variable = variable_match.group(1)
            labels = ""
            buckets = ""
            if method == "histogram_vec_with_buckets":
                if len(arguments) != 3:
                    raise GuardError(f"factory call `{key}` has the wrong arity")
                buckets = _canonical_expression(arguments[1])
                labels = _canonical_expression(arguments[2])
            elif method == "histogram_with_buckets":
                if len(arguments) != 2:
                    raise GuardError(f"factory call `{key}` has the wrong arity")
                buckets = _canonical_expression(arguments[1])
            elif method in vector_methods:
                if len(arguments) != 2:
                    raise GuardError(f"factory call `{key}` has the wrong arity")
                labels = _canonical_expression(arguments[1])
            elif method in scalar_methods:
                if len(arguments) != 1:
                    raise GuardError(f"factory call `{key}` has the wrong arity")
            else:
                raise GuardError(f"factory call `{key}` uses unknown method `{method}`")
            calls.append((key, variable, method, labels, buckets))
    except GuardError as error:
        findings.append(str(error))
    return calls, findings


def _semantic_ledger(
    rows: list[tuple[str, str, str, bool]],
    calls: list[tuple[str, str, str, str, str]],
) -> bytes:
    if len(rows) != len(calls):
        raise GuardError(f"catalog/source row count differs: {len(rows)} != {len(calls)}")
    records: list[str] = []
    for ordinal, (row, call) in enumerate(zip(rows, calls), 1):
        key, name, help_text, registered = row
        source_key, variable, method, labels, buckets = call
        if key != source_key:
            raise GuardError(
                f"catalog/source order differs at row {ordinal}: `{key}` != `{source_key}`"
            )
        record = {
            "buckets": buckets,
            "help": help_text,
            "key": key,
            "labels": labels,
            "method": method,
            "name": name,
            "ordinal": ordinal,
            "registered": registered,
            "variable": variable,
        }
        records.append(json.dumps(record, ensure_ascii=False, separators=(",", ":"), sort_keys=True))
    return ("\n".join(records) + "\n").encode("utf-8")


def check_contents(catalog_raw: bytes, source: str) -> list[str]:
    """Return deterministic findings for the supplied catalog and Rust source."""
    rows, findings = _catalog_rows(catalog_raw)
    calls, source_findings = _factory_calls(source)
    findings.extend(source_findings)

    consumer = 'include_str!("metrics/catalog_v2.tsv")'
    if source.count(consumer) != 1:
        findings.append(f"catalog consumer count: expected 1, got {source.count(consumer)}")
    if "catalog_v1.tsv" in source:
        findings.append("obsolete catalog_v1 consumer remains")
    expected_literals = (
        "const METRIC_CATALOG_V2_ROWS: usize = 870;",
        "const METRIC_CATALOG_V2_REGISTERED: usize = 825;",
        "const METRIC_CATALOG_V2_BYTES: usize = 118_432;",
        CATALOG_BLAKE3,
    )
    for literal in expected_literals:
        if source.count(literal) != 1:
            findings.append(f"Rust catalog invariant changed or duplicated: {literal}")

    if rows and calls:
        if len(calls) != ROWS:
            findings.append(f"factory calls: expected {ROWS}, got {len(calls)}")
        method_counts = dict(sorted(collections.Counter(call[2] for call in calls).items()))
        if method_counts != METHOD_COUNTS:
            findings.append(
                f"factory method inventory changed: expected {METHOD_COUNTS}, got {method_counts}"
            )
        try:
            ledger = _semantic_ledger(rows, calls)
        except (GuardError, ValueError) as error:
            findings.append(str(error))
        else:
            if len(ledger) != LEDGER_BYTES:
                findings.append(f"semantic ledger bytes: expected {LEDGER_BYTES}, got {len(ledger)}")
            ledger_digest = hashlib.sha256(ledger).hexdigest()
            if ledger_digest != LEDGER_SHA256:
                findings.append(
                    f"semantic ledger sha256: expected {LEDGER_SHA256}, got {ledger_digest}"
                )

    if re.search(r"metric_specs\s*\.\s*(?:opts|histogram_opts)\s*\(", source):
        findings.append("direct metric option construction bypasses the typed factory")
    construction = ""
    if source.count(CONSTRUCTION_START) == 1 and source.count(CONSTRUCTION_END) == 1:
        construction = source.split(CONSTRUCTION_START, 1)[1].split(CONSTRUCTION_END, 1)[0]
    if "register_guarded(" in construction or "register!(" in construction:
        findings.append("duplicated metric registration remains in the construction pass")

    if source.count(FACTORY_START) != 1 or source.count(FACTORY_END) != 1:
        findings.append("typed factory implementation boundary changed")
    else:
        factory = source.split(FACTORY_START, 1)[1].split(FACTORY_END, 1)[0]
        factory_tokens = _compact_rust(FACTORY_START + factory).encode("utf-8")
        digest = hashlib.sha256(factory_tokens).hexdigest()
        if digest != FACTORY_TOKENS_SHA256:
            findings.append(
                f"typed factory source sha256: expected {FACTORY_TOKENS_SHA256}, got {digest}"
            )

    if source.count(CONSTRUCTION_END) == 1 and source.count(INITIALIZER_START) == 1:
        post_catalog = source.split(CONSTRUCTION_END, 1)[1].split(INITIALIZER_START, 1)[0]
        digest = hashlib.sha256(_compact_rust(post_catalog).encode("utf-8")).hexdigest()
        if digest != POST_CATALOG_TOKENS_SHA256:
            findings.append(
                "post-catalog direct metric source sha256: "
                f"expected {POST_CATALOG_TOKENS_SHA256}, got {digest}"
            )
    else:
        findings.append("post-catalog direct metric boundary changed")
    return findings


def check(root: Path) -> list[str]:
    """Return deterministic findings for a repository root."""
    catalog_path = root / CATALOG
    source_path = root / SOURCE
    if not catalog_path.is_file():
        return [f"missing catalog: {CATALOG}"]
    if not source_path.is_file():
        return [f"missing source: {SOURCE}"]
    try:
        return check_contents(catalog_path.read_bytes(), source_path.read_text(encoding="utf-8"))
    except (GuardError, OSError, UnicodeError, ValueError) as error:
        return [f"guard parse failed: {error}"]


def self_test(root: Path) -> list[str]:
    """Exercise independent catalog, call-site, and factory-source mutations in memory."""
    catalog = (root / CATALOG).read_bytes()
    source = (root / SOURCE).read_text(encoding="utf-8")
    failures: list[str] = []
    baseline = check_contents(catalog, source)
    if baseline:
        return ["self-test baseline failed: " + "; ".join(baseline)]

    mutations = [
        (
            "registration",
            catalog.replace(b"\tregistered\n", b"\tunregistered\n", 1),
            source,
        ),
        ("catalog-name", catalog.replace(b"txs\ttxs", b"txs\ttxz", 1), source),
        (
            "factory-kind",
            catalog,
            source.replace('metrics.int_counter_vec("txs"', 'metrics.gauge_vec("txs"', 1),
        ),
        (
            "factory-label",
            catalog,
            source.replace(
                'metrics.int_counter_vec("txs", &["type"])',
                'metrics.int_counter_vec("txs", &["kind"])',
                1,
            ),
        ),
        ("bucket", catalog, source.replace("-10.0,", "-11.0,", 1)),
        ("factory-body", catalog, source.replace("if registered {", "if !registered {", 1)),
        ("second-consumer", catalog, source + '\nconst EXTRA: &str = include_str!("metrics/catalog_v2.tsv");\n'),
    ]
    for label, mutated_catalog, mutated_source in mutations:
        if mutated_catalog == catalog and mutated_source == source:
            failures.append(f"self-test mutation `{label}` did not alter its input")
        elif not check_contents(mutated_catalog, mutated_source):
            failures.append(f"self-test mutation `{label}` was not rejected")
    return failures


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    root = args.root.resolve()
    findings = self_test(root) if args.self_test else check(root)
    if findings:
        for finding in findings:
            print(f"telemetry metric catalog v2 guard: {finding}")
        return 1
    suffix = "; mutation self-test passed" if args.self_test else ""
    print(
        "telemetry metric catalog v2 guard: "
        f"{ROWS} rows, {REGISTERED} registered, ledger_sha256={LEDGER_SHA256}{suffix}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
