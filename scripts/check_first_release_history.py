#!/usr/bin/env python3
"""Check the first-release history and cutover inventory against the source tree.

The inventory is `specs/first_release_history_cutover.json`; its contract is
`specs/first_release_history_cutover.md`. The check reads source text and the
pinned history fixture. It builds nothing, runs no test, decodes no stored
history and changes no file. It fails when:

- a history-bearing wire type admits a version other than its single listed one,
  or a `Version` implementation is not listed;
- a listed replay, restart or rejection test no longer exists, is ignored, is
  conditionally compiled, or names no package and target;
- a blocked test target's anchor no longer matches, so the entry is stale;
- the pinned history fixture differs from its manifest or from the digest the
  inventory records, or its generator or replay test is gone;
- a history loader defines or imports a compatibility-named item that is neither
  a listed rejection helper nor a classified definition;
- a retired decoder or shim identifier reappears;
- a helper kept only for tests is referenced from production source;
- a retired store artifact refusal no longer exists, is no longer called from
  the Strict open path, refuses other names than the inventory lists, or a
  retired-artifact refusal exists that the inventory does not list;
- an incompatible-change surface lacks a concrete regeneration target with a
  generator or checking test that mentions it, or the contract table and the
  inventory list different surfaces or cutovers;
- a probe or custody anchor of the obsolete-history evidence is gone;
- an open finding no longer matches the source, so the inventory is stale.

Limits. The loader check is name-based: it sees definitions and `use ... as`
imports whose name says compatibility. It does not see a fallback written
inside an existing function, a version `match` outside the `Version` trait, or
a neutral name. The test check reads attributes on the test function only, not
on enclosing modules. Whether a retired artifact is still refused at run time,
and whether a history written by an earlier build still replays, is established
by the listed Rust tests, which read this inventory and the pinned fixture.

`--report` prints every open finding as `path:line` at the current source.
`--test-commands` prints one `cargo test` command per package and target that
runs exactly the listed tests; blocked targets are printed as comments.

Requires Python 3.10+ and no third-party module. `--root` selects another
checkout (default: the repository containing this script).
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import sys

INVENTORY = "specs/first_release_history_cutover.json"
SCHEMA = "iroha.first_release_history_cutover.v1"
CUTOVERS = ("fresh_genesis", "contract_redeployment", "fresh_genesis_when_stored")
ARTIFACT_LOCATIONS = {
    "blocks_root": "blocks_root.join(retired)",
    "store_root": "store_root.join(retired)",
}
ARTIFACT_KINDS = ("file", "directory")
SKIPPED_DIRECTORIES = frozenset(("target", "node_modules", ".git"))
TEST_PATH = re.compile(
    r"(?:^|/)(?:tests|benches|examples|fuzz)(?:/|$)"
    r"|_tests(?:/|\.rs$)"
    r"|(?:^|/)tests\.rs$"
)
TEST_ATTRIBUTE = re.compile(
    r"#\[\s*(?:cfg\s*\(\s*(?:test\b|all\s*\(\s*test\b|any\s*\(\s*test\b)"
    r"|(?:\w+::)*test\b)"
)
INNER_TEST_ATTRIBUTE = re.compile(r"^\s*#!\[\s*cfg\s*\(\s*test\s*\)\s*\]", re.MULTILINE)
IS_TEST_ATTRIBUTE = re.compile(r"#\[\s*(?:\w+::)*test\b")
IGNORE_ATTRIBUTE = re.compile(r"#\[\s*(?:ignore\b|cfg_attr\b[^\]]*\bignore\b)")
CFG_ATTRIBUTE = re.compile(r"#\[\s*cfg\s*\((?P<predicate>.*)\)\s*\]\Z", re.DOTALL)
FUNCTION_QUALIFIER = re.compile(
    r"(?:pub(?:\s*\([^)]*\))?|async|const|unsafe|extern)\s*\Z"
)
VERSION_IMPL = re.compile(
    r"\bimpl\b(?:\s*<[^{;]*?>)?\s*(?:\w+::)*Version\s+for\s+(?P<type>\w+)[^{;]*\{"
)
VERSION_IMPL_BYTES = re.compile(rb"\bVersion\s+for\s")
VERSION_LITERAL = re.compile(r"\bfn\s+version\s*\(\s*&self\s*\)\s*->\s*u8\s*\{\s*(\d+)\s*\}")
SUPPORTED_RANGE = re.compile(
    r"\bfn\s+supported_versions\s*\(\s*\)\s*->\s*[\w:]*Range\s*<\s*u8\s*>\s*\{"
    r"\s*(\d+)\s*\.\.\s*(\d+)\s*\}"
)
LAYOUT_WORD = r"layout|format|wire|header|schema|version|codec|encoding|decod|payload"
LAYOUT_WORD_UPPER = r"LAYOUT|FORMAT|WIRE|HEADER|SCHEMA|VERSION|CODEC|ENCODING|PAYLOAD"
LAYOUT_WORD_CAMEL = r"Layout|Format|Wire|Header|Schema|Version|Codec|Encoding|Payload|Block"
COMPATIBILITY_NAME = (
    r"[Ll]egacy|LEGACY|[Cc]ompat(?!ibl)|COMPAT(?!IBL)|[Mm]igrat|MIGRAT"
    r"|PreRelease|pre_release|PRE_RELEASE"
    r"|[Ss]him|SHIM|[Dd]eprecated|DEPRECATED|_v0\b|V0\b"
    r"|fallback_(?:decod|layout|format|wire)|(?:decod|layout|format|wire)\w*_fallback"
    # An earlier layout named as such: `decode_old_layout`, `PreviousHeader`, `PRIOR_FORMAT`.
    r"|(?<![a-z])(?:old|previous|prior)_(?:" + LAYOUT_WORD + r")"
    r"|(?:decode|decoder|parse|read|load|" + LAYOUT_WORD + r")_(?:old|previous|prior)(?![a-z])"
    r"|(?<![A-Z])(?:OLD|PREVIOUS|PRIOR)_(?:" + LAYOUT_WORD_UPPER + r")"
    r"|(?<![a-z])(?:Old|Previous|Prior)(?:" + LAYOUT_WORD_CAMEL + r")"
    # A second name for a layout: `wire_alias`, `alias_layout`, `LayoutAlias`.
    r"|(?:layout|format|wire|decoder|schema|version|compat)_alias(?![a-z])"
    r"|(?<![a-z])alias_(?:layout|format|wire|decoder|schema|version)"
    r"|(?:Layout|Format|Wire|Decoder|Schema|Version)Alias"
    r"|Alias(?:Layout|Format|Wire|Decoder|Schema|Version)"
)
COMPATIBILITY_DEFINITION = re.compile(
    r"\b(?:fn|struct|enum|type|const|static|mod|trait)\s+"
    r"(?P<name>\w*(?:" + COMPATIBILITY_NAME + r")\w*)"
)
COMPATIBILITY_WORD = re.compile(r"\w*(?:" + COMPATIBILITY_NAME + r")\w*\Z")
USE_STATEMENT = re.compile(r"\buse\b[^;]*;")
USE_ALIAS = re.compile(r"\bas\s+(?P<name>\w+)")
RETIRED_REFUSAL = re.compile(r"\bfn\s+(?P<name>(?:reject|ensure_no)_retired_\w+)")
INLINE_RETIRED_ARRAY = re.compile(r"\bfor\s+retired\s+in\s*\[")
SURFACE_ROW = re.compile(
    r"^\|\s*`(?P<id>[a-z_]+)`\s*\|[^|]*\|\s*`(?P<cutover>[a-z_]+)`[^|]*\|", re.MULTILINE
)
SURFACE_ID = re.compile(r"[a-z][a-z_]*\Z")
TEST_TARGET = re.compile(r"(?:lib|test:[A-Za-z0-9_]+)\Z")
CHAR_LITERAL = re.compile(r"'(?:\\(?:u\{[0-9a-fA-F_]{1,8}\}|x[0-9a-fA-F]{2}|.)|[^\\'\n])'")
LITERAL_START = re.compile(r"//|/\*|\"|'|\b[bc]?r#*\"")
BLOCK_COMMENT_EDGE = re.compile(r"/\*|\*/")
STRING_END = re.compile(r'(?:[^"\\]|\\.)*"', re.DOTALL)
NOT_NEWLINE = re.compile(r"[^\n]")
BRACE = re.compile(r"[{}]")
BRACKET = re.compile(r"[\[\]]")


def literal_spans(text: str) -> list[tuple[str, int, int, str]]:
    """Comments and literals of Rust source as `(kind, start, end, content)`.

    `kind` is `comment`, `string` or `char`; `content` is a string literal's text between its
    quotes (escapes are kept as written) and empty otherwise.
    """
    spans: list[tuple[str, int, int, str]] = []
    index = 0
    length = len(text)
    while True:
        found = LITERAL_START.search(text, index)
        if found is None:
            return spans
        start, token = found.start(), found.group()
        if token == "//":
            end = text.find("\n", start)
            end = length if end < 0 else end
            spans.append(("comment", start, end, ""))
        elif token == "/*":
            depth, end = 1, start + 2
            while depth:
                edge = BLOCK_COMMENT_EDGE.search(text, end)
                if edge is None:
                    end = length
                    break
                depth += 1 if edge.group() == "/*" else -1
                end = edge.end()
            spans.append(("comment", start, end, ""))
        elif token == '"':
            closing = STRING_END.match(text, start + 1)
            end = length if closing is None else closing.end()
            spans.append(("string", start, end, text[start + 1 : max(start + 1, end - 1)]))
        elif token == "'":
            literal = CHAR_LITERAL.match(text, start)
            if literal is None:
                # A lifetime or loop label, not a literal.
                index = start + 1
                continue
            end = literal.end()
            spans.append(("char", start, end, ""))
        else:
            closing = '"' + "#" * token.count("#")
            close_at = text.find(closing, found.end())
            end = length if close_at < 0 else close_at + len(closing)
            content_end = length if close_at < 0 else close_at
            spans.append(("string", start, end, text[found.end() : content_end]))
        index = end


def blank_rust_literals(text: str) -> str:
    """Blank comments and string/char literal contents, keeping length and newlines."""
    out: list[str] = []
    index = 0
    for _, start, end, _ in literal_spans(text):
        out.append(text[index:start])
        out.append(NOT_NEWLINE.sub(" ", text[start:end]))
        index = end
    out.append(text[index:])
    return "".join(out)


def matching_close(blanked: str, opening: int, pair: re.Pattern[str] = BRACE) -> int:
    """Return the index after the delimiter matching `blanked[opening]`, or the text length."""
    depth = 0
    for found in pair.finditer(blanked, opening):
        depth += 1 if found.group() in "{[" else -1
        if depth == 0:
            return found.end()
    return len(blanked)


def item_start(blanked: str, position: int) -> int:
    """Skip whitespace and attributes from `position` to the item they apply to."""
    while True:
        while position < len(blanked) and blanked[position].isspace():
            position += 1
        if not blanked.startswith("#[", position):
            return position
        position = matching_close(blanked, position + 1, BRACKET)


def item_end(blanked: str, start: int) -> int:
    """Return the index after the item starting at `start`: its block, or its `;`."""
    nesting = 0
    for index in range(start, len(blanked)):
        char = blanked[index]
        if char in "([":
            nesting += 1
        elif char in ")]":
            nesting -= 1
        elif char == "{":
            return matching_close(blanked, index)
        elif char == ";" and nesting <= 0:
            return index + 1
    return len(blanked)


def test_spans(blanked: str) -> list[tuple[int, int]]:
    """Character spans of items compiled only for tests (`cfg(test)` items and test functions)."""
    if INNER_TEST_ATTRIBUTE.search(blanked):
        return [(0, len(blanked))]
    spans: list[tuple[int, int]] = []
    covered = 0
    for attribute in TEST_ATTRIBUTE.finditer(blanked):
        if attribute.start() < covered:
            continue
        covered = item_end(blanked, item_start(blanked, attribute.start()))
        spans.append((attribute.start(), covered))
    return spans


def in_spans(position: int, spans: list[tuple[int, int]]) -> bool:
    """Whether `position` lies inside one of `spans`."""
    return any(start <= position < end for start, end in spans)


def line_of(text: str, position: int) -> int:
    """One-based line number of a character position."""
    return text.count("\n", 0, position) + 1


def is_test_path(relative: str) -> bool:
    """Whether a repository-relative Rust path is test, bench, example or fuzz source."""
    return TEST_PATH.search(relative) is not None


def rust_sources(root: Path, relative_roots: list[str]) -> list[Path]:
    """Every `.rs` file below the given files, directories or glob patterns, sorted."""
    found: set[Path] = set()
    for relative in relative_roots:
        if any(char in relative for char in "*?["):
            candidates = sorted(root.glob(relative))
        else:
            candidates = [root / relative]
        for candidate in candidates:
            if candidate.is_file():
                if candidate.suffix == ".rs":
                    found.add(candidate)
                continue
            for parent, directories, names in os.walk(candidate):
                directories[:] = sorted(set(directories) - SKIPPED_DIRECTORIES)
                found.update(Path(parent) / name for name in names if name.endswith(".rs"))
    return sorted(found)


def read(path: Path) -> str | None:
    """Read UTF-8 source, or `None` when the file is missing or unreadable."""
    try:
        return path.read_text(encoding="utf-8")
    except (OSError, UnicodeError):
        return None


class Source:
    """One Rust source file: its text, the text with literals blanked, and its literals."""

    def __init__(self, text: str) -> None:
        self.text = text
        self.literals = literal_spans(text)
        self.blanked = blank_rust_literals(text)
        self.tests = test_spans(self.blanked)

    def function(self, name: str) -> tuple[int, int] | None:
        """Span of the first production function `name`, from `fn` to the end of its body."""
        pattern = re.compile(r"\bfn\s+" + re.escape(name) + r"\s*(?:<[^>(]*>)?\s*\(")
        for found in pattern.finditer(self.blanked):
            if not in_spans(found.start(), self.tests):
                return found.start(), item_end(self.blanked, found.start())
        return None

    def strings(self, start: int, end: int) -> list[str]:
        """Contents of the string literals inside `[start, end)`."""
        return [
            content
            for kind, begin, finish, content in self.literals
            if kind == "string" and start <= begin and finish <= end
        ]


class Sources:
    """Rust sources of one checkout, lexed once."""

    def __init__(self, root: Path) -> None:
        self.root = root
        self.cache: dict[str, Source | None] = {}

    def get(self, relative: str) -> Source | None:
        """The lexed source at a repository-relative path, or `None` when unreadable."""
        if relative not in self.cache:
            text = read(self.root / relative)
            self.cache[relative] = None if text is None else Source(text)
        return self.cache[relative]


def load_inventory(root: Path) -> tuple[dict, list[str]]:
    """Load the inventory and report structural defects."""
    path = root / INVENTORY
    try:
        inventory = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        return {}, [f"{INVENTORY}: unreadable inventory: {error}"]
    errors: list[str] = []
    if not isinstance(inventory, dict) or inventory.get("schema") != SCHEMA:
        return {}, [f"{INVENTORY}: schema must be {SCHEMA}"]
    required = {
        "contract": str,
        "history_wire": list,
        "test_only_version_impls": list,
        "replay_tests": list,
        "obsolete_layout_rejection_tests": list,
        "blocked_test_targets": list,
        "pinned_history": dict,
        "loader_paths": list,
        "loader_rejection_helpers": list,
        "loader_classified_definitions": list,
        "test_only_references": list,
        "retired_identifier_roots": list,
        "retired_identifiers": list,
        "retired_store_artifacts": list,
        "incompatible_change_surfaces": list,
        "obsolete_history_evidence": dict,
        "open_findings": list,
    }
    for key, kind in required.items():
        if not isinstance(inventory.get(key), kind):
            errors.append(f"{INVENTORY}: `{key}` must be a {kind.__name__}")
    if errors:
        return {}, errors
    for key in (
        "history_wire",
        "replay_tests",
        "retired_identifiers",
        "loader_paths",
        "retired_store_artifacts",
        "incompatible_change_surfaces",
    ):
        if not inventory[key]:
            errors.append(f"{INVENTORY}: `{key}` must not be empty")
    if not (root / inventory["contract"]).is_file():
        errors.append(f"{INVENTORY}: contract {inventory['contract']} is missing")
    return inventory, errors


def version_impl_errors(
    relative: str,
    text: str,
    listed: dict[tuple[str, str], dict],
    test_only: set[tuple[str, str]],
    seen: set[tuple[str, str]],
) -> list[str]:
    """Check the `Version` implementations of one source file against the inventory."""
    errors: list[str] = []
    blanked = blank_rust_literals(text)
    for found in VERSION_IMPL.finditer(blanked):
        key = (relative, found.group("type"))
        seen.add(key)
        where = f"{relative}:{line_of(blanked, found.start())}"
        if key in test_only:
            continue
        entry = listed.get(key)
        if entry is None:
            errors.append(
                f"{where}: `Version` implementation for {key[1]} is not listed in "
                f"{INVENTORY}; a history-bearing wire type has exactly one version"
            )
            continue
        body = blanked[found.end() - 1 : matching_close(blanked, found.end() - 1)]
        version = VERSION_LITERAL.search(body)
        supported = SUPPORTED_RANGE.search(body)
        expected = entry["version"]
        if version is None or int(version.group(1)) != expected:
            errors.append(f"{where}: {key[1]}::version must be the literal {expected}")
        if supported is None or (int(supported.group(1)), int(supported.group(2))) != (
            expected,
            expected + 1,
        ):
            errors.append(
                f"{where}: {key[1]}::supported_versions must be exactly "
                f"{expected}..{expected + 1}; no second layout or version dispatch"
            )
    return errors


def reference_pattern(needle: str) -> str:
    """Regular expression matching a Rust path or cast written with any spacing."""
    words = [
        r"\s*::\s*".join(re.escape(piece) for piece in word.split("::"))
        for word in needle.split()
    ]
    return r"(?<!\w)" + r"\s+".join(words) + r"(?!\w)"


def test_only_reference_errors(
    relative: str, text: str, references: list[tuple[dict, re.Pattern[str]]]
) -> list[str]:
    """Production references in one source file to helpers kept only for tests."""
    if is_test_path(relative):
        return []
    errors: list[str] = []
    blanked = blank_rust_literals(text)
    spans = test_spans(blanked)
    for entry, pattern in references:
        for found in pattern.finditer(blanked):
            if in_spans(found.start(), spans):
                continue
            if re.search(r"\bfn\s+\Z", blanked[: found.start()]):
                continue
            errors.append(
                f"{relative}:{line_of(blanked, found.start())}: production source references "
                f"`{entry['needle']}`, which is kept only for tests ({entry['finding']})"
            )
    return errors


def check_sources(root: Path, inventory: dict) -> list[str]:
    """One pass over the scanned roots.

    Every `Version` implementation must be listed, and a history-bearing type admits exactly its
    one listed version. No retired decoder or shim identifier may appear anywhere. A helper kept
    only for tests has no reference in production source.
    """
    identifiers = [entry["identifier"] for entry in inventory["retired_identifiers"]]
    errors = [
        f"{INVENTORY}: retired identifier `{identifier}` is not a plain identifier"
        for identifier in identifiers
        if not re.fullmatch(r"\w+", identifier)
    ]
    findings = {entry["id"] for entry in inventory["open_findings"]}
    references: list[tuple[dict, re.Pattern[str]]] = []
    for entry in inventory["test_only_references"]:
        if entry["finding"] not in findings:
            errors.append(
                f"{INVENTORY}: test-only reference `{entry['needle']}` names the unknown "
                f"finding {entry['finding']}; when the helper is gone, delete the entry"
            )
        references.append((entry, re.compile(reference_pattern(entry["needle"]))))
    if errors:
        return errors
    needles = [identifier.encode() for identifier in identifiers]
    retired = re.compile(rb"(?<![A-Za-z0-9])(?:" + b"|".join(needles) + rb")")
    # Every word of a reference must occur before the file is lexed for it.
    prefilters = [
        [piece.encode() for piece in re.split(r"\s+|::", entry["needle"]) if piece]
        for entry, _ in references
    ]
    listed = {(entry["path"], entry["type"]): entry for entry in inventory["history_wire"]}
    test_only = {(entry["path"], entry["type"]) for entry in inventory["test_only_version_impls"]}
    seen: set[tuple[str, str]] = set()
    for relative in inventory["retired_identifier_roots"]:
        if not (root / relative).is_dir():
            errors.append(f"{relative}: scan root is missing")
    for path in rust_sources(root, inventory["retired_identifier_roots"]):
        relative = path.relative_to(root).as_posix()
        try:
            data = path.read_bytes()
        except OSError:
            errors.append(f"{relative}: unreadable source")
            continue
        if any(needle in data for needle in needles):
            for found in retired.finditer(data):
                line = data.count(b"\n", 0, found.start()) + 1
                errors.append(
                    f"{relative}:{line}: retired first-release identifier "
                    f"`{found.group().decode()}` reappeared"
                )
        has_version = b"Version" in data and VERSION_IMPL_BYTES.search(data)
        has_reference = any(all(piece in data for piece in pieces) for pieces in prefilters)
        if not has_version and not has_reference:
            continue
        try:
            text = data.decode("utf-8")
        except UnicodeError:
            errors.append(f"{relative}: unreadable source")
            continue
        if has_version:
            errors += version_impl_errors(relative, text, listed, test_only, seen)
        if has_reference:
            errors += test_only_reference_errors(relative, text, references)
    for key in sorted((set(listed) | test_only) - seen):
        errors.append(f"{key[0]}: listed `Version` implementation for {key[1]} does not exist")
    return errors


def function_attributes(blanked: str, fn_start: int) -> list[str]:
    """The attributes applied to the function whose `fn` keyword is at `fn_start`."""
    position = fn_start
    while True:
        head = blanked[:position].rstrip()
        qualifier = FUNCTION_QUALIFIER.search(head)
        if qualifier is None or qualifier.start() == qualifier.end():
            break
        position = qualifier.start()
    attributes: list[str] = []
    while True:
        head = blanked[:position].rstrip()
        if not head.endswith("]"):
            return attributes
        depth = 0
        opening = -1
        for index in range(len(head) - 1, -1, -1):
            if head[index] == "]":
                depth += 1
            elif head[index] == "[":
                depth -= 1
                if depth == 0:
                    opening = index
                    break
        if opening < 1 or head[opening - 1] != "#":
            return attributes
        attributes.append(head[opening - 1 :])
        position = opening - 1


def check_tests_exist(sources: Sources, entries: list[dict], kind: str) -> list[str]:
    """Each listed test function exists, is a test, always compiles and is not ignored."""
    errors: list[str] = []
    for entry in entries:
        relative, name = entry["path"], entry["test"]
        if not isinstance(entry.get("package"), str) or not TEST_TARGET.match(
            str(entry.get("target", ""))
        ):
            errors.append(
                f"{INVENTORY}: {kind} test `{name}` must name its `package` and its `target` "
                "(`lib` or `test:<name>`)"
            )
        source = sources.get(relative)
        if source is None:
            errors.append(f"{relative}: {kind} test source is missing")
            continue
        declared = None
        for found in re.finditer(r"\bfn\s+" + re.escape(name) + r"\s*\(", source.blanked):
            attributes = function_attributes(source.blanked, found.start())
            if any(IS_TEST_ATTRIBUTE.match(attribute) for attribute in attributes):
                declared = attributes
                break
        if declared is None:
            errors.append(f"{relative}: {kind} test `{name}` no longer exists")
            continue
        if any(IGNORE_ATTRIBUTE.match(attribute) for attribute in declared):
            errors.append(f"{relative}: {kind} test `{name}` is ignored")
        for attribute in declared:
            condition = CFG_ATTRIBUTE.match(attribute)
            if condition and condition.group("predicate").strip() != "test":
                errors.append(
                    f"{relative}: {kind} test `{name}` is conditionally compiled "
                    f"(`{' '.join(attribute.split())}`)"
                )
    return errors


def listed_tests(inventory: dict) -> list[dict]:
    """Every replay and rejection test of the inventory."""
    return inventory["replay_tests"] + inventory["obsolete_layout_rejection_tests"]


def check_blocked_targets(root: Path, inventory: dict) -> list[str]:
    """A blocked test target still fails to build for the listed reason."""
    errors: list[str] = []
    targets = {(entry.get("package"), entry.get("target")) for entry in listed_tests(inventory)}
    for entry in inventory["blocked_test_targets"]:
        key = (entry["package"], entry["target"])
        label = f"{key[0]} ({key[1]})"
        if key not in targets:
            errors.append(f"{INVENTORY}: blocked test target {label} has no listed test")
        if not entry["owner"].startswith("outside-zk-plan:"):
            tasks = plan_tasks(root)
            if tasks is not None and entry["owner"] not in tasks:
                errors.append(f"{INVENTORY}: blocked test target {label} names an unknown owner")
        if not entry["anchors"]:
            errors.append(f"{INVENTORY}: blocked test target {label} has no anchor")
        for anchor in entry["anchors"]:
            if check_anchor(root, anchor["path"], anchor["contains"]) is None:
                errors.append(
                    f"{anchor['path']}: blocked test target {label} no longer matches the "
                    "source; delete the entry, run its listed tests and record the result"
                )
    return errors


def test_commands(inventory: dict) -> list[str]:
    """One `cargo test` command per package and target that runs exactly the listed tests."""
    blocked = {
        (entry["package"], entry["target"]): entry["reason"]
        for entry in inventory["blocked_test_targets"]
    }
    groups: dict[tuple[str, str], list[str]] = {}
    for entry in listed_tests(inventory):
        groups.setdefault((entry["package"], entry["target"]), []).append(entry["test"])
    lines: list[str] = []
    for (package, target), names in sorted(groups.items()):
        selector = "--lib" if target == "lib" else f"--test {target.split(':', 1)[1]}"
        command = f"cargo test -p {package} {selector} -- {' '.join(sorted(set(names)))}"
        if (package, target) in blocked:
            lines.append(f"# blocked: {blocked[(package, target)]}")
            lines.append(f"# {command}")
        else:
            lines.append(command)
    return lines


def check_pinned_history(root: Path, sources: Sources, inventory: dict) -> list[str]:
    """The pinned history fixture is intact and is the one the inventory records."""
    pinned = inventory["pinned_history"]
    relative = pinned["manifest"]
    try:
        manifest = json.loads((root / relative).read_text(encoding="utf-8"))
        blocks = manifest["blocks"]
        recorded = manifest["history_sha256"]
        form = manifest["format"]
    except (OSError, UnicodeError, json.JSONDecodeError, KeyError, TypeError) as error:
        return [f"{relative}: unreadable pinned history manifest: {error}"]
    errors: list[str] = []
    if form != pinned["format"]:
        errors.append(f"{relative}: pinned history format must be {pinned['format']}")
    if not isinstance(blocks, list) or len(blocks) < 2:
        return errors + [f"{relative}: a pinned history is a genesis and at least one block"]
    history = hashlib.sha256()
    directory = (root / relative).parent
    for index, block in enumerate(blocks):
        name = str(block.get("file", ""))
        where = f"{directory.relative_to(root).as_posix()}/{name}"
        if block.get("height") != index + 1 or "/" in name or not name:
            errors.append(f"{relative}: pinned block {index + 1} is out of order or misnamed")
            continue
        try:
            frame = (directory / name).read_bytes()
        except OSError:
            errors.append(f"{where}: pinned frame is missing")
            continue
        if hashlib.sha256(frame).hexdigest() != block.get("sha256") or len(frame) != block.get(
            "bytes"
        ):
            errors.append(f"{where}: pinned frame differs from the digest its manifest records")
        history.update(frame)
    if not errors and history.hexdigest() != recorded:
        errors.append(f"{relative}: `history_sha256` differs from the pinned frames")
    if recorded != pinned["history_sha256"]:
        errors.append(
            f"{relative}: pinned history {recorded} is not the one {INVENTORY} records "
            f"({pinned['history_sha256']}); a regenerated history is a declared cutover: "
            "record its digest in the inventory in the same change"
        )
    generator = pinned["generator"]
    source = sources.get(generator["path"])
    if source is None or not re.search(
        r"\bfn\s+" + re.escape(generator["test"]) + r"\s*\(", source.blanked
    ):
        errors.append(f"{generator['path']}: pinned history generator `{generator['test']}` is gone")
    replay = pinned["replay_test"]
    if not any(
        (entry["path"], entry["test"]) == (replay["path"], replay["test"])
        for entry in inventory["replay_tests"]
    ):
        errors.append(
            f"{INVENTORY}: pinned history replay test `{replay['test']}` is not a listed replay test"
        )
    return errors


def check_loader_definitions(root: Path, sources: Sources, inventory: dict) -> list[str]:
    """History loaders define and import no unlisted compatibility-named item.

    Rejection helpers and classified definitions are the only exceptions, and every
    `reject_retired_*`/`ensure_no_retired_*` function is a listed refusal or helper.
    """
    errors: list[str] = []
    helpers = {(entry["path"], entry["name"]) for entry in inventory["loader_rejection_helpers"]}
    classified = {
        (entry["path"], entry["name"]) for entry in inventory["loader_classified_definitions"]
    }
    refusals = {
        (entry["path"], entry["refusal"])
        for entry in inventory["retired_store_artifacts"]
        if entry.get("kind") == "function"
    }
    allowed = helpers | classified
    seen: set[tuple[str, str]] = set()
    for relative in inventory["loader_paths"]:
        if not rust_sources(root, [relative]):
            errors.append(f"{relative}: listed history loader path has no Rust source")
    for path in rust_sources(root, inventory["loader_paths"]):
        relative = path.relative_to(root).as_posix()
        if is_test_path(relative):
            continue
        source = sources.get(relative)
        if source is None:
            errors.append(f"{relative}: unreadable history loader source")
            continue
        blanked, spans = source.blanked, source.tests
        for found in COMPATIBILITY_DEFINITION.finditer(blanked):
            if in_spans(found.start(), spans):
                continue
            key = (relative, found.group("name"))
            seen.add(key)
            if key not in allowed:
                errors.append(
                    f"{relative}:{line_of(blanked, found.start())}: history loader defines "
                    f"`{key[1]}`; old layouts are refused, never decoded, migrated or aliased"
                )
        for statement in USE_STATEMENT.finditer(blanked):
            if in_spans(statement.start(), spans):
                continue
            for alias in USE_ALIAS.finditer(statement.group()):
                name = alias.group("name")
                if COMPATIBILITY_WORD.match(name) and (relative, name) not in allowed:
                    position = statement.start() + alias.start()
                    errors.append(
                        f"{relative}:{line_of(blanked, position)}: history loader imports an "
                        f"item as `{name}`; a replaced layout keeps no compatibility alias"
                    )
        for found in RETIRED_REFUSAL.finditer(blanked):
            if in_spans(found.start(), spans):
                continue
            key = (relative, found.group("name"))
            seen.add(key)
            if key not in helpers and key not in refusals:
                errors.append(
                    f"{relative}:{line_of(blanked, found.start())}: retired-artifact refusal "
                    f"`{key[1]}` is not listed under `retired_store_artifacts` or "
                    "`loader_rejection_helpers`"
                )
    for key in sorted(helpers - seen):
        errors.append(f"{key[0]}: listed rejection helper `{key[1]}` no longer exists")
    for key in sorted(classified - seen):
        errors.append(f"{key[0]}: classified definition `{key[1]}` no longer exists; delete it")
    return errors


def check_anchor(root: Path, relative: str, anchor: str) -> int | None:
    """One-based line of the first `anchor` occurrence in a file, or `None`."""
    text = read(root / relative)
    if text is None:
        return None
    position = text.find(anchor)
    return None if position < 0 else line_of(text, position)


def inline_retired_arrays(source: Source, start: int, end: int) -> list[tuple[list[str], str]]:
    """Each `for retired in [..] {..}` of a span: its name literals and its loop body."""
    arrays: list[tuple[list[str], str]] = []
    for found in INLINE_RETIRED_ARRAY.finditer(source.blanked, start, end):
        opening = found.end() - 1
        closing = matching_close(source.blanked, opening, BRACKET)
        body_end = item_end(source.blanked, closing)
        arrays.append((source.strings(opening, closing), source.text[closing:body_end]))
    return arrays


def refusal_function_errors(sources: Sources, entry: dict, source: Source) -> list[str]:
    """One named refusal function refuses exactly its listed names and is still called."""
    name, relative = entry["refusal"], entry["path"]
    span = source.function(name)
    if span is None:
        return [f"{relative}: retired-artifact refusal `{name}` no longer exists"]
    errors: list[str] = []
    body = source.blanked[span[0] : span[1]]
    found = source.strings(*span)
    listed = entry["literals"] + entry["messages"]
    for literal in sorted(set(listed) - set(found)):
        errors.append(
            f"{relative}: retired-artifact refusal `{name}` no longer names `{literal}`; the "
            "artifact is no longer refused by name"
        )
    for literal in sorted(set(found) - set(listed)):
        errors.append(
            f"{relative}: retired-artifact refusal `{name}` names `{literal}`, which "
            f"{INVENTORY} does not list"
        )
    if entry["error"] not in body and not any(entry["error"] in literal for literal in found):
        errors.append(
            f"{relative}: retired-artifact refusal `{name}` no longer fails with `{entry['error']}`"
        )
    for constant in entry["constants"]:
        owner = sources.get(constant["path"])
        defined = owner is not None and re.search(
            r"\bconst\s+"
            + re.escape(constant["name"])
            + r"\s*:\s*&\s*(?:'static\s+)?str\s*=\s*\""
            + re.escape(constant["value"])
            + r"\"\s*;",
            owner.text,
        )
        if not defined:
            errors.append(
                f"{constant['path']}: retired artifact name `{constant['value']}` is no longer "
                f"the constant `{constant['name']}`"
            )
        user = constant.get("via", name)
        user_span = source.function(user)
        if user_span is None or not re.search(
            r"\b" + re.escape(constant["name"]) + r"\b", source.blanked[user_span[0] : user_span[1]]
        ):
            errors.append(
                f"{relative}: `{user}` no longer uses `{constant['name']}`; "
                f"`{constant['value']}` is no longer refused by name"
            )
        if user != name and not re.search(r"\b" + re.escape(user) + r"\s*\(", body):
            errors.append(f"{relative}: retired-artifact refusal `{name}` no longer calls `{user}`")
    if not entry["called_from"]:
        errors.append(f"{INVENTORY}: retired-artifact refusal `{name}` lists no call site")
    for site in entry["called_from"]:
        caller = sources.get(site["path"])
        caller_span = None if caller is None else caller.function(site["function"])
        if caller_span is None or not re.search(
            r"\b(?:Self|Kura)\s*::\s*" + re.escape(name) + r"\s*\(",
            caller.blanked[caller_span[0] : caller_span[1]],
        ):
            errors.append(
                f"{site['path']}: `{site['function']}` no longer calls the retired-artifact "
                f"refusal `{name}`"
            )
    return errors


def check_retired_store_artifacts(root: Path, sources: Sources, inventory: dict) -> list[str]:
    """Retired store artifacts are refused by the listed functions and arrays, and by no other."""
    errors: list[str] = []
    inline_listed: dict[str, int] = {}
    for entry in inventory["retired_store_artifacts"]:
        name, relative = entry["refusal"], entry["path"]
        if entry["location"] not in ARTIFACT_LOCATIONS:
            errors.append(f"{INVENTORY}: retired-artifact refusal `{name}` has an unknown location")
        if not entry["artifacts"]:
            errors.append(f"{INVENTORY}: retired-artifact refusal `{name}` lists no artifact")
        for artifact in entry["artifacts"]:
            plant = str(artifact.get("plant", ""))
            if (
                not artifact.get("name")
                or not plant
                or plant.startswith("/")
                or ".." in plant.split("/")
                or artifact.get("kind") not in ARTIFACT_KINDS
            ):
                errors.append(
                    f"{INVENTORY}: retired artifact of `{name}` needs a `name`, a relative "
                    "`plant` path and a `kind` of file or directory"
                )
        source = sources.get(relative)
        if source is None:
            errors.append(f"{relative}: retired-artifact refusal source is missing")
            continue
        if entry["kind"] == "function":
            errors += refusal_function_errors(sources, entry, source)
            continue
        if entry["kind"] != "inline_array":
            errors.append(f"{INVENTORY}: retired-artifact refusal `{name}` has an unknown kind")
            continue
        inline_listed[relative] = inline_listed.get(relative, 0) + 1
        span = source.function(name)
        arrays = [] if span is None else inline_retired_arrays(source, *span)
        if entry["index"] >= len(arrays):
            errors.append(
                f"{relative}: `{name}` no longer holds retired-artifact array {entry['index']}"
            )
            continue
        names, loop = arrays[entry["index"]]
        if sorted(names) != sorted(entry["literals"]):
            errors.append(
                f"{relative}: retired-artifact array {entry['index']} of `{name}` refuses "
                f"{sorted(names)}, {INVENTORY} lists {sorted(entry['literals'])}"
            )
        if ARTIFACT_LOCATIONS.get(entry["location"], "\0") not in loop or entry["error"] not in loop:
            errors.append(
                f"{relative}: retired-artifact array {entry['index']} of `{name}` no longer "
                f"refuses under {entry['location']} with `{entry['error']}`"
            )
    for path in rust_sources(root, inventory["loader_paths"]):
        relative = path.relative_to(root).as_posix()
        source = None if is_test_path(relative) else sources.get(relative)
        if source is None:
            continue
        found = sum(
            1
            for array in INLINE_RETIRED_ARRAY.finditer(source.blanked)
            if not in_spans(array.start(), source.tests)
        )
        if found != inline_listed.get(relative, 0):
            errors.append(
                f"{relative}: {found} inline retired-artifact arrays, {INVENTORY} lists "
                f"{inline_listed.get(relative, 0)}"
            )
    return errors


def regeneration_errors(root: Path, surface: str, target: dict) -> list[str]:
    """One regeneration target names a concrete artifact and what produces or checks it."""
    artifact = str(target.get("artifact", ""))
    path = root / artifact
    if not artifact or not path.exists():
        return [f"{artifact}: regeneration target of `{surface}` is missing"]
    if path.is_dir() and "/" not in artifact.strip("/"):
        return [
            f"{artifact}: regeneration target of `{surface}` is a whole top-level directory; "
            "name the pinned artifact"
        ]
    if target.get("document"):
        if not path.is_file():
            return [f"{artifact}: document regeneration target of `{surface}` must be a file"]
        return []
    owners = [target[key] for key in ("generator", "checked_by") if target.get(key)]
    mention = target.get("mention")
    if not owners or not mention:
        return [
            f"{artifact}: regeneration target of `{surface}` needs a `generator` or `checked_by` "
            "source and the `mention` that ties it to the artifact"
        ]
    errors: list[str] = []
    for owner in owners:
        text = read(root / owner)
        if text is None:
            errors.append(f"{owner}: generator or check of `{artifact}` is missing")
        elif mention not in text:
            errors.append(f"{owner}: no longer mentions `{mention}`; it does not cover `{artifact}`")
    return errors


def check_surfaces(root: Path, inventory: dict) -> list[str]:
    """Incompatible-change surfaces are concrete, and the contract table lists the same ones."""
    errors: list[str] = []
    listed: dict[str, str] = {}
    for entry in inventory["incompatible_change_surfaces"]:
        identifier = str(entry.get("id", ""))
        surface = entry["surface"]
        if not SURFACE_ID.match(identifier) or identifier in listed:
            errors.append(f"{INVENTORY}: surface `{surface}` needs a unique lower-case `id`")
        listed[identifier] = entry["cutover"]
        if entry["cutover"] not in CUTOVERS:
            errors.append(f"{INVENTORY}: surface `{surface}` has unknown cutover")
        if not entry["anchors"]:
            errors.append(f"{INVENTORY}: surface `{surface}` has no source anchor")
        for anchor in entry["anchors"]:
            if check_anchor(root, anchor["path"], anchor["contains"]) is None:
                errors.append(
                    f"{anchor['path']}: surface `{surface}` anchor `{anchor['contains']}` "
                    "is missing; update the inventory with the moved owner"
                )
        if not entry["regenerate"]:
            errors.append(f"{INVENTORY}: surface `{surface}` lists nothing to regenerate")
        for target in entry["regenerate"]:
            errors += regeneration_errors(root, surface, target)
    contract = read(root / inventory["contract"]) or ""
    table = {row.group("id"): row.group("cutover") for row in SURFACE_ROW.finditer(contract)}
    for identifier in sorted(set(listed) - set(table)):
        errors.append(
            f"{inventory['contract']}: surface `{identifier}` of {INVENTORY} is missing from the "
            "contract table"
        )
    for identifier in sorted(set(table) - set(listed)):
        errors.append(
            f"{inventory['contract']}: contract table surface `{identifier}` is not in {INVENTORY}"
        )
    for identifier in sorted(set(table) & set(listed)):
        if table[identifier] != listed[identifier]:
            errors.append(
                f"{inventory['contract']}: surface `{identifier}` cutover is "
                f"`{table[identifier]}` in the contract and `{listed[identifier]}` in {INVENTORY}"
            )
    return errors


def check_evidence(root: Path, inventory: dict) -> list[str]:
    """The obsolete-history evidence tool, its test and the mechanisms it relies on exist."""
    evidence = inventory["obsolete_history_evidence"]
    errors: list[str] = []
    tool = read(root / evidence["tool"])
    if tool is None:
        errors.append(f"{evidence['tool']}: obsolete-history evidence tool is missing")
    elif evidence["manifest_schema"] not in tool:
        errors.append(
            f"{evidence['tool']}: no longer writes the `{evidence['manifest_schema']}` manifest"
        )
    if not (root / evidence["tool_test"]).is_file():
        errors.append(f"{evidence['tool_test']}: obsolete-history evidence test is missing")
    for anchor in evidence["anchors"]:
        if check_anchor(root, anchor["path"], anchor["contains"]) is None:
            errors.append(
                f"{anchor['path']}: obsolete-history evidence anchor `{anchor['contains']}` is "
                "missing; the contract's diagnostic path changed"
            )
    return errors


def open_findings(root: Path, inventory: dict) -> tuple[list[str], list[str]]:
    """Return `(report lines, errors)` for findings owned by other tasks."""
    report: list[str] = []
    errors: list[str] = []
    identifiers: set[str] = set()
    tasks = plan_tasks(root)
    for entry in inventory["open_findings"]:
        identifier, owner = entry["id"], entry["owner"]
        if identifier in identifiers:
            errors.append(f"{INVENTORY}: duplicate open finding {identifier}")
        identifiers.add(identifier)
        if not owner.startswith("outside-zk-plan:") and tasks is not None and owner not in tasks:
            errors.append(f"{INVENTORY}: open finding {identifier} names unknown task {owner}")
        line = check_anchor(root, entry["path"], entry["anchor"])
        if line is None:
            errors.append(
                f"{entry['path']}: open finding {identifier} no longer matches the source; "
                "delete it, or list what was removed under `retired_identifiers`"
            )
            continue
        report.append(f"{entry['path']}:{line}: {identifier} [{owner}] {entry['summary']}")
    return report, errors


def plan_tasks(root: Path) -> set[str] | None:
    """Task identifiers of the delivery graph, or `None` when the graph is unavailable."""
    try:
        graph = json.loads((root / "specs/zk_delivery_graph.json").read_text(encoding="utf-8"))
        return {task["id"] for task in graph["tasks"]}
    except (OSError, UnicodeError, json.JSONDecodeError, KeyError, TypeError):
        return None


def check(root: Path) -> tuple[str, ...]:
    """Check the inventory against `root` and return every violation."""
    inventory, errors = load_inventory(root)
    if not inventory:
        return tuple(errors)
    sources = Sources(root)
    errors += check_sources(root, inventory)
    errors += check_tests_exist(sources, inventory["replay_tests"], "replay")
    errors += check_tests_exist(
        sources, inventory["obsolete_layout_rejection_tests"], "obsolete-layout rejection"
    )
    errors += check_blocked_targets(root, inventory)
    errors += check_pinned_history(root, sources, inventory)
    errors += check_loader_definitions(root, sources, inventory)
    errors += check_retired_store_artifacts(root, sources, inventory)
    errors += check_surfaces(root, inventory)
    errors += check_evidence(root, inventory)
    errors += open_findings(root, inventory)[1]
    return tuple(sorted(set(errors)))


def main() -> int:
    """Check the requested tree; optionally print the open findings or the test commands."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--report", action="store_true", help="print open findings as path:line")
    parser.add_argument(
        "--test-commands",
        action="store_true",
        help="print the cargo commands that run exactly the listed tests",
    )
    args = parser.parse_args()
    root = args.root.resolve()
    errors = check(root)
    if args.report or args.test_commands:
        inventory, _ = load_inventory(root)
        if inventory and args.report:
            print("\n".join(open_findings(root, inventory)[0]))
        if inventory and args.test_commands:
            print("\n".join(test_commands(inventory)))
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    if not args.test_commands:
        print("first-release history and cutover inventory matches the source")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
