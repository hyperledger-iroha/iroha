#!/usr/bin/env python3
"""Reject unreviewed panic-recovery boundaries in Torii workers."""

from __future__ import annotations

import difflib
import hashlib
import re
import subprocess
import sys
from pathlib import Path
from typing import NamedTuple

try:
    import tomllib
except ModuleNotFoundError:  # pragma: no cover - Python 3.10 compatibility
    import tomli as tomllib


ROOT = Path(__file__).resolve().parents[1]

# These modules deliberately convert worker failures into request/provider
# errors. A bare blocking task here would run outside the physical worker's
# suppression and would therefore signal process shutdown before its JoinError
# was handled.
NO_BARE_BLOCKING = (
    "crates/iroha_torii/src/lib.rs",
    "crates/iroha_torii/src/lib_pipeline_handlers.rs",
    "crates/iroha_torii/src/routing.rs",
    "crates/iroha_torii/src/routing/signed_query_execution.rs",
    "crates/iroha_torii/src/privacy_issuance_api.rs",
    "crates/iroha_torii/src/da/ingest.rs",
    "crates/iroha_torii/src/da/spool.rs",
    "crates/iroha_torii/src/da/taikai.rs",
    "crates/iroha_torii/src/private_settlement.rs",
    "crates/iroha_torii/src/sorafs/api.rs",
    "crates/iroha_torii/src/sorafs/gateway_compliance_api.rs",
    "crates/iroha_torii/src/sorafs/hedging_billing_api.rs",
    "crates/iroha_torii/src/sorafs/orderbook_runtime.rs",
    "crates/iroha_torii/src/sorafs/por/persistence_randomness.rs",
    "crates/iroha_torii/src/sorafs/reserve_runtime.rs",
    "crates/iroha_torii/src/zk_attachments.rs",
    "crates/iroha_torii/src/zk_prover.rs",
    "crates/iroha_torii/src/sns.rs",
    "crates/iroha_torii/src/parliament_tle_release.rs",
    "crates/iroha_torii/src/webhook.rs",
    "crates/irohad/src/runtime_provider_broker/platform_provider_clients_01.rs",
    "crates/irohad/src/runtime_provider_broker/platform_provider_clients_03.rs",
    "crates/irohad/src/sorafs_provider_ingest_finalized_query.rs",
    "crates/irohad/src/sorafs_provider_ingest_runtime.rs",
    "crates/irohad/src/sorafs_reputation_runtime.rs",
    "crates/irohad/src/sorafs_hedging_billing_runtime.rs",
    "crates/irohad/src/soracloud_runtime.rs",
)
NO_BARE_STD_THREAD = tuple(
    relative
    for relative in NO_BARE_BLOCKING
    if relative.startswith("crates/irohad/")
    and relative != "crates/irohad/src/soracloud_runtime.rs"
)

REQUIRED_SNIPPETS = {
    "crates/iroha_core/src/panic_hook.rs": (
        "pub fn catch_unwind_suppressed",
        "with_hook_suppressed_async",
        "blocking_worker_reuse_does_not_retain_suppression",
    ),
    "crates/irohad/src/panic_recovery.rs": (
        "pub(crate) fn spawn_blocking_recoverable",
        "pub(crate) fn recover_joined",
        "pub(crate) fn spawn_thread_recoverable",
        "raw_join_failure_remains_an_unsuppressed_invariant",
    ),
    "crates/iroha_torii/src/panic_recovery.rs": (
        "pub(crate) fn spawn_blocking_recoverable",
        "pub(crate) fn spawn_joined_recoverable",
        "pub(crate) async fn join_recoverable",
        "ordinary_invariant_panics_remain_unsuppressed",
    ),
    "crates/iroha_torii/src/da/taikai.rs": (
        "read_optional_regular_file",
        "crate::panic_recovery::spawn_blocking_recoverable",
    ),
    "crates/iroha_torii/src/private_settlement.rs": (
        "crate::panic_recovery::spawn_blocking_recoverable",
        "crate::panic_recovery::spawn_joined_recoverable",
    ),
    "crates/iroha_torii/src/sorafs/api.rs": (
        "governance_dag_blocking_response",
        "crate::panic_recovery::spawn_blocking_recoverable",
    ),
    "crates/iroha_torii/src/sorafs/gateway_compliance_api.rs": (
        "crate::panic_recovery::spawn_blocking_recoverable",
        "crate::panic_recovery::join_recoverable",
    ),
    "crates/iroha_torii/src/sorafs/hedging_billing_api.rs": (
        "crate::panic_recovery::spawn_blocking_recoverable",
        "crate::panic_recovery::join_recoverable",
    ),
    "crates/iroha_torii/src/sorafs/orderbook_runtime.rs": (
        "crate::panic_recovery::spawn_blocking_recoverable",
        "crate::panic_recovery::join_recoverable",
    ),
    "crates/iroha_torii/src/sorafs/por/persistence_randomness.rs": (
        "crate::panic_recovery::spawn_blocking_recoverable",
        "crate::panic_recovery::join_recoverable",
    ),
    "crates/iroha_torii/src/sorafs/reserve_runtime.rs": (
        "crate::panic_recovery::spawn_blocking_recoverable",
        "crate::panic_recovery::join_recoverable",
    ),
    "crates/iroha_core/src/executor.rs": (
        "crate::panic_hook::catch_unwind_suppressed",
    ),
    "crates/iroha_core/src/zk.rs": (
        "let pk = crate::panic_hook::catch_unwind_suppressed",
    ),
    "crates/iroha_core/src/zk/kagemusha_accumulation.rs": (
        "crate::panic_hook::catch_unwind_suppressed",
        "recoverable_native_verifier_panic_suppresses_shutdown_hook_scope",
    ),
    "crates/iroha_core/src/zk/kagemusha_recursion_adapter.rs": (
        "crate::panic_hook::catch_unwind_suppressed",
    ),
}

FORBIDDEN_RECOVERY_SNIPPETS = {
    "crates/iroha_core/src/executor.rs": (
        "std::panic::catch_unwind",
    ),
    "crates/iroha_core/src/zk/kagemusha_accumulation.rs": (
        "std::panic::catch_unwind",
    ),
    "crates/iroha_core/src/zk/kagemusha_recursion_adapter.rs": (
        "std::panic::catch_unwind",
    ),
    "crates/iroha_torii/src/privacy_issuance_api.rs": (
        "catch_unwind(AssertUnwindSafe(operation))",
    ),
    "crates/iroha_torii/src/da/spool.rs": (
        "catch_unwind(AssertUnwindSafe(move || (run)()))",
    ),
}

REVIEWED_TORII_BOUNDARY_INVENTORY = Path(
    "scripts/panic_recovery_boundaries.inventory"
)
AUDITED_SOURCE_PATHS = (
    Path("crates/iroha_torii"),
    Path("crates/build-support"),
    Path("crates/irohad"),
    Path("crates/iroha_core/src/panic_hook.rs"),
)
REQUIRED_AUDITED_SOURCE_PATHS = AUDITED_SOURCE_PATHS[:2]
TORII_BUILD_SCRIPT = Path("crates/build-support/script.rs")
BOUNDARY_IDENTIFIERS = {
    "catch_unwind": frozenset({"catch_unwind"}),
    "spawn_blocking": frozenset({"spawn_blocking", "spawn_blocking_on"}),
    "task_spawn": frozenset(
        {"spawn", "spawn_local", "spawn_local_on", "spawn_on", "spawn_scoped"}
    ),
    # Axum starts each upgraded WebSocket callback in a fresh Tokio task. That
    # task does not inherit Torii's request task-local panic suppression, so
    # every callback must be reviewed like an explicit spawn site.
    "upgrade_task": frozenset({"on_upgrade"}),
}
RAW_STRING_PREFIX = re.compile(r"(?:br|rb|r)(?P<hashes>#{0,255})\"")


class RustToken(NamedTuple):
    """One comment-free Rust lexical token used by the source guard."""

    text: str
    start: int
    end: int


def _rust_tokens(source: str) -> list[RustToken]:
    """Lex enough Rust to distinguish code from comments and literals."""

    tokens: list[RustToken] = []
    cursor = 0
    length = len(source)
    while cursor < length:
        char = source[cursor]
        if char.isspace():
            cursor += 1
            continue
        if source.startswith("//", cursor):
            newline = source.find("\n", cursor + 2)
            cursor = length if newline < 0 else newline + 1
            continue
        if source.startswith("/*", cursor):
            depth = 1
            end = cursor + 2
            while end < length and depth:
                if source.startswith("/*", end):
                    depth += 1
                    end += 2
                elif source.startswith("*/", end):
                    depth -= 1
                    end += 2
                else:
                    end += 1
            cursor = end
            continue

        raw = RAW_STRING_PREFIX.match(source, cursor)
        if raw is not None:
            delimiter = '"' + raw.group("hashes")
            end = source.find(delimiter, raw.end())
            end = length if end < 0 else end + len(delimiter)
            tokens.append(RustToken(source[cursor:end], cursor, end))
            cursor = end
            continue

        literal_prefix = 1 if char in {"b", "c"} and cursor + 1 < length else 0
        quote_at = cursor + literal_prefix
        if quote_at < length and source[quote_at] == '"':
            end = quote_at + 1
            escaped = False
            while end < length:
                current = source[end]
                end += 1
                if escaped:
                    escaped = False
                elif current == "\\":
                    escaped = True
                elif current == '"':
                    break
            tokens.append(RustToken(source[cursor:end], cursor, end))
            cursor = end
            continue

        # A lifetime such as `'a` is an identifier marker, while a character
        # literal has a closing quote. Preserve either as a single token.
        if char == "'" or (char == "b" and cursor + 1 < length and source[cursor + 1] == "'"):
            quote_at = cursor if char == "'" else cursor + 1
            end = quote_at + 1
            closed = False
            if end < length and source[end] == "\\":
                escaped = False
                while end < length and source[end] != "\n":
                    current = source[end]
                    end += 1
                    if escaped:
                        escaped = False
                    elif current == "\\":
                        escaped = True
                    elif current == "'":
                        closed = True
                        break
            elif end + 1 < length and source[end + 1] == "'":
                end += 2
                closed = True
            if closed:
                tokens.append(RustToken(source[cursor:end], cursor, end))
                cursor = end
                continue

        if char == "_" or char.isalpha():
            end = cursor + 1
            while end < length and (source[end] == "_" or source[end].isalnum()):
                end += 1
            tokens.append(RustToken(source[cursor:end], cursor, end))
            cursor = end
            continue

        matched = next(
            (
                punctuation
                for punctuation in ("::", "=>", "->", "..=", "...", "..")
                if source.startswith(punctuation, cursor)
            ),
            char,
        )
        end = cursor + len(matched)
        tokens.append(RustToken(matched, cursor, end))
        cursor = end
    return tokens


def _boundary_aliases(tokens: list[RustToken]) -> dict[str, str]:
    aliases: dict[str, str] = {}
    identifier_kind = {
        identifier: kind
        for kind, identifiers in BOUNDARY_IDENTIFIERS.items()
        for identifier in identifiers
    }
    for index, token in enumerate(tokens):
        kind = identifier_kind.get(token.text)
        if kind is None:
            continue

        # Reject import aliases, including aliases exported for use by another
        # module where the original boundary spelling would otherwise vanish.
        statement_start = index
        while statement_start > 0 and tokens[statement_start - 1].text != ";":
            statement_start -= 1
        if any(
            candidate.text == "use"
            for candidate in tokens[statement_start:index]
        ):
            cursor = index + 1
            while cursor < len(tokens) and tokens[cursor].text not in {";", "{", "}", ","}:
                if tokens[cursor].text == "as" and cursor + 1 < len(tokens):
                    alias = tokens[cursor + 1].text
                    if alias == "_" or alias[0].isalpha():
                        aliases[alias] = kind
                    break
                cursor += 1

        # A local function-item binding hides the call spelling just as
        # effectively as an import alias, for example
        # `let recover = std::panic::catch_unwind;`.
        statement_start = index
        while statement_start > 0 and tokens[statement_start - 1].text not in {";", "{", "}"}:
            statement_start -= 1
        statement_end = index + 1
        while statement_end < len(tokens) and tokens[statement_end].text != ";":
            statement_end += 1
        statement = tokens[statement_start:statement_end]
        equals = next(
            (
                offset
                for offset, candidate in enumerate(statement)
                if candidate.text == "="
            ),
            None,
        )
        binding_kind = next(
            (
                offset
                for offset, candidate in enumerate(statement)
                if candidate.text in {"let", "const", "static"}
            ),
            None,
        )
        if equals is None or binding_kind is None or statement_start + equals >= index:
            continue
        following = tokens[index + 1].text if index + 1 < len(tokens) else ";"
        if following == "(":
            continue
        binding_cursor = binding_kind + 1
        while binding_cursor < equals and statement[binding_cursor].text in {"mut", "ref"}:
            binding_cursor += 1
        if binding_cursor >= equals:
            continue
        alias = statement[binding_cursor].text
        if alias == "_" or not (alias[0].isalpha() or alias[0] == "_"):
            continue
        aliases[alias] = kind
    return aliases


def _is_rust_call(tokens: list[RustToken], identifier_index: int) -> bool:
    """Return whether an identifier is followed by a call expression."""

    cursor = identifier_index + 1
    if cursor + 1 < len(tokens) and tokens[cursor].text == "::" and tokens[cursor + 1].text == "<":
        cursor += 2
        depth = 1
        while cursor < len(tokens) and depth:
            depth += tokens[cursor].text == "<"
            depth -= tokens[cursor].text == ">"
            cursor += 1
    return cursor < len(tokens) and tokens[cursor].text == "("


def _bare_blocking_lines(source: str) -> list[int]:
    """Find raw blocking-task calls, including method and `JoinSet` forms."""

    tokens = _rust_tokens(source)
    return [
        source.count("\n", 0, token.start) + 1
        for index, token in enumerate(tokens)
        if token.text in {"spawn_blocking", "spawn_blocking_on"}
        and _is_rust_call(tokens, index)
    ]


def _bare_std_thread_lines(source: str) -> list[int]:
    """Find explicit raw `std::thread` spawn calls in a recoverable module."""

    tokens = _rust_tokens(source)
    failures: list[int] = []
    for index, token in enumerate(tokens):
        if token.text not in {"spawn", "spawn_scoped"} or not _is_rust_call(tokens, index):
            continue
        statement_start = index
        while statement_start > 0 and tokens[statement_start - 1].text not in {";", "{", "}"}:
            statement_start -= 1
        prefix = [candidate.text for candidate in tokens[statement_start:index]]
        rendered = " ".join(prefix)
        if (
            "std :: thread ::" in rendered
            or "thread ::" in rendered
            or "std :: thread :: Builder" in rendered
            or "thread :: Builder" in rendered
        ):
            failures.append(source.count("\n", 0, token.start) + 1)
    return failures


def _source_inventory(path: Path, root: Path) -> tuple[str, dict[str, int]]:
    """Fingerprint one complete source file and summarize boundary spellings."""

    source = path.read_text(encoding="utf-8")
    tokens = _rust_tokens(source)
    identifier_kind = {
        identifier: kind
        for kind, identifiers in BOUNDARY_IDENTIFIERS.items()
        for identifier in identifiers
    }
    counts = {kind: 0 for kind in BOUNDARY_IDENTIFIERS}
    spelling_counts: dict[tuple[str, str], int] = {}
    for token in tokens:
        kind = identifier_kind.get(token.text)
        if kind is None:
            continue
        counts[kind] += 1
        key = (kind, token.text)
        spelling_counts[key] = spelling_counts.get(key, 0) + 1
    relative = path.relative_to(root).as_posix()
    rendered_sites = ",".join(
        f"{kind}:{identifier}={count}"
        for (kind, identifier), count in sorted(spelling_counts.items())
    ) or "none"
    digest = hashlib.sha256(path.read_bytes()).hexdigest()
    return f"{relative}\t{digest}\t{rendered_sites}", counts


def _git_audited_entries(root: Path) -> list[tuple[str, Path]] | None:
    """Return indexed/untracked modes and paths, or ``None`` outside Git."""

    completed = subprocess.run(
        [
            "git",
            "-C",
            str(root),
            "ls-files",
            "--stage",
            "--cached",
            "--others",
            "--exclude-standard",
            "-z",
            "--",
            *(relative.as_posix() for relative in AUDITED_SOURCE_PATHS),
        ],
        check=False,
        capture_output=True,
    )
    if completed.returncode == 0:
        entries: list[tuple[str, Path]] = []
        for raw in completed.stdout.split(b"\0"):
            if not raw:
                continue
            metadata, separator, encoded_path = raw.partition(b"\t")
            if not separator:
                entries.append(("untracked", Path(raw.decode("utf-8"))))
                continue
            fields = metadata.split()
            if not separator or len(fields) != 3:
                continue
            mode = fields[0].decode("ascii")
            stage = fields[2].decode("ascii")
            if stage != "0":
                mode = f"conflict:{stage}:{mode}"
            entries.append(
                (mode, Path(encoded_path.decode("utf-8")))
            )
        return entries
    return None


def torii_audited_files(root: Path) -> list[Path]:
    """Return every repository file under the two audited source roots."""

    git_entries = _git_audited_entries(root)
    if git_entries is not None:
        return sorted(
            path
            for mode, relative in git_entries
            if mode != "160000"
            and (path := root / relative).is_file()
            and not path.is_symlink()
        )

    # Synthetic guard tests are intentionally not Git repositories. In that
    # setting every regular file is part of the candidate source closure.
    paths: list[Path] = []
    for relative_root in AUDITED_SOURCE_PATHS:
        source_root = root / relative_root
        if source_root.is_file() and not source_root.is_symlink():
            paths.append(source_root)
        elif source_root.is_dir():
            paths.extend(
                path
                for path in source_root.rglob("*")
                if path.is_file() and not path.is_symlink()
            )
    return sorted(paths)


def _rust_path_literal(token: str) -> str | None:
    """Decode the deliberately narrow path-literal grammar accepted by the guard."""

    quoted = re.fullmatch(r'"(?P<body>[^"\\]*)"', token, re.DOTALL)
    if quoted is not None:
        return quoted.group("body")
    raw = re.fullmatch(
        r'r(?P<hashes>#{0,255})"(?P<body>.*)"(?P=hashes)',
        token,
        re.DOTALL,
    )
    if raw is not None:
        return raw.group("body")
    return None


def _attribute_end(tokens: list[RustToken], start: int) -> int | None:
    """Return the closing bracket for the attribute beginning at ``# [``."""

    if (
        start + 1 >= len(tokens)
        or tokens[start].text != "#"
        or tokens[start + 1].text != "["
    ):
        return None
    depth = 0
    for index in range(start + 1, len(tokens)):
        if tokens[index].text == "[":
            depth += 1
        elif tokens[index].text == "]":
            depth -= 1
            if depth == 0:
                return index
    return None


def _rust_module_directory(path: Path) -> Path:
    """Return the default child-module directory for one Rust source file."""

    if (
        path.name in {"lib.rs", "main.rs", "mod.rs", "build.rs", "script.rs"}
        or path.parent.name in {"tests", "examples", "benches", "bin"}
    ):
        return path.parent
    return path.parent / path.stem


def _inline_module_contexts(tokens: list[RustToken]) -> list[tuple[str, ...]]:
    """Return the inline-module directory stack at each token."""

    inline_openings: dict[int, str] = {}
    for index, token in enumerate(tokens):
        if token.text != "mod" or index + 2 >= len(tokens):
            continue
        name = tokens[index + 1].text
        if (name[0].isalpha() or name[0] == "_") and tokens[index + 2].text == "{":
            inline_openings[index + 2] = name

    contexts: list[tuple[str, ...]] = []
    stack: list[tuple[int, str]] = []
    depth = 0
    for index, token in enumerate(tokens):
        contexts.append(tuple(name for _, name in stack))
        if token.text == "{":
            depth += 1
            if index in inline_openings:
                stack.append((depth, inline_openings[index]))
        elif token.text == "}":
            while stack and stack[-1][0] == depth:
                stack.pop()
            depth = max(0, depth - 1)
    return contexts


def _rust_textual_source_references(
    path: Path, root: Path
) -> tuple[list[Path], list[str]]:
    """Resolve literal includes plus explicit and conventional Rust modules."""

    source = path.read_text(encoding="utf-8")
    tokens = _rust_tokens(source)
    contexts = _inline_module_contexts(tokens)
    # (kind, source token, literal, module-relative base)
    references: list[tuple[str, RustToken, str | None, Path]] = []
    path_overrides: set[int] = set()

    for index, token in enumerate(tokens):
        if (
            token.text == "include"
            and index + 2 < len(tokens)
            and tokens[index + 1].text == "!"
            and tokens[index + 2].text in {"(", "{", "["}
        ):
            closing = {"(": ")", "{": "}", "[": "]"}[tokens[index + 2].text]
            literal = None
            if index + 4 < len(tokens):
                candidate_literal = _rust_path_literal(tokens[index + 3].text)
                close_at = index + 4
                if tokens[close_at].text == ",":
                    close_at += 1
                if (
                    candidate_literal is not None
                    and close_at < len(tokens)
                    and tokens[close_at].text == closing
                ):
                    literal = candidate_literal
            references.append(("include!", token, literal, path.parent))

        if token.text != "#" or index + 1 >= len(tokens) or tokens[index + 1].text != "[":
            continue
        end = _attribute_end(tokens, index)
        if end is None:
            references.append(("#[path]", token, None, path.parent))
            continue
        literal = None
        path_token = token
        for cursor in range(index + 2, end - 1):
            if tokens[cursor].text == "path" and tokens[cursor + 1].text == "=":
                literal = _rust_path_literal(tokens[cursor + 2].text)
                path_token = tokens[cursor]
                break
        if path_token is token:
            continue
        cursor = end + 1
        while cursor < len(tokens) and tokens[cursor].text == "#":
            next_end = _attribute_end(tokens, cursor)
            if next_end is None:
                break
            cursor = next_end + 1
        if cursor < len(tokens) and tokens[cursor].text == "pub":
            cursor += 1
            if cursor < len(tokens) and tokens[cursor].text == "(":
                depth = 1
                cursor += 1
                while cursor < len(tokens) and depth:
                    depth += tokens[cursor].text == "("
                    depth -= tokens[cursor].text == ")"
                    cursor += 1
        if cursor < len(tokens) and tokens[cursor].text == "mod":
            path_overrides.add(cursor)
        # Explicit #[path] values are relative to the containing source file;
        # each enclosing inline module contributes one directory component.
        module_base = path.parent.joinpath(*contexts[index])
        references.append(("#[path]", path_token, literal, module_base))

    for index, token in enumerate(tokens):
        if token.text != "mod" or index in path_overrides or index + 2 >= len(tokens):
            continue
        name = tokens[index + 1].text
        if not (name[0].isalpha() or name[0] == "_") or tokens[index + 2].text != ";":
            continue
        module_base = _rust_module_directory(path).joinpath(*contexts[index])
        candidates = (module_base / f"{name}.rs", module_base / name / "mod.rs")
        existing = [candidate for candidate in candidates if candidate.is_file()]
        if len(existing) == 1:
            references.append(("mod", token, existing[0].name, existing[0].parent))
        elif not existing:
            references.append(("mod", token, None, module_base))
        else:
            relative_source = path.relative_to(root).as_posix()
            line = source.count("\n", 0, token.start) + 1
            rendered = ", ".join(candidate.relative_to(root).as_posix() for candidate in existing)
            return [], [
                f"{relative_source}:{line}: mod {name} has ambiguous source files: {rendered}"
            ]

    audited_roots = tuple((root / relative).resolve() for relative in AUDITED_SOURCE_PATHS)
    resolved: list[Path] = []
    failures: list[str] = []
    relative_source = path.relative_to(root).as_posix()
    for kind, token, literal, base in references:
        line = source.count("\n", 0, token.start) + 1
        if literal is None or not literal or "\0" in literal:
            failures.append(
                f"{relative_source}:{line}: {kind} must name one static local source file"
            )
            continue
        candidate = base / literal
        target = candidate.resolve()
        if not any(target.is_relative_to(source_root) for source_root in audited_roots):
            failures.append(
                f"{relative_source}:{line}: {kind} source path escapes the audited source roots: {literal}"
            )
            continue
        if not candidate.is_file() or candidate.is_symlink():
            failures.append(
                f"{relative_source}:{line}: {kind} source path is missing or not a regular file: {literal}"
            )
            continue
        resolved.append(target)
    return resolved, failures


def torii_rust_source_closure(root: Path) -> tuple[list[Path], list[str]]:
    """Return every textual Rust source reachable inside the audited roots."""

    pending = [path for path in torii_audited_files(root) if path.suffix == ".rs"]

    sources: dict[Path, Path] = {}
    failures: list[str] = []
    while pending:
        path = pending.pop()
        canonical = path.resolve()
        if canonical in sources or path.is_symlink() or not path.is_file():
            continue
        sources[canonical] = path
        try:
            references, reference_failures = _rust_textual_source_references(path, root)
        except UnicodeDecodeError:
            failures.append(
                f"{path.relative_to(root).as_posix()}: textual Rust source must be UTF-8"
            )
            continue
        failures.extend(reference_failures)
        pending.extend(references)
    return sorted(sources.values()), sorted(set(failures))


def torii_boundary_inventory(root: Path) -> tuple[list[str], str, dict[str, int]]:
    """Return the complete Torii and shared-build repository-file closure."""

    records: list[str] = []
    counts = {kind: 0 for kind in BOUNDARY_IDENTIFIERS}
    source_paths, _ = torii_rust_source_closure(root)
    source_canonicals = {path.resolve() for path in source_paths}
    audited_files = {path.resolve(): path for path in torii_audited_files(root)}
    audited_files.update({path.resolve(): path for path in source_paths})
    for canonical, path in sorted(audited_files.items(), key=lambda item: item[1]):
        if canonical in source_canonicals:
            source_record, source_counts = _source_inventory(path, root)
            records.append(source_record)
            for kind, count in source_counts.items():
                counts[kind] += count
            continue
        relative = path.relative_to(root).as_posix()
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        kind = "manifest" if path.name == "Cargo.toml" else "repository-file"
        records.append(f"{relative}\t{digest}\t{kind}")
    records.sort()
    encoded = ("\n".join(records) + ("\n" if records else "")).encode("utf-8")
    return records, hashlib.sha256(encoded).hexdigest(), counts


def _reviewed_inventory(root: Path) -> list[str]:
    path = root / REVIEWED_TORII_BOUNDARY_INVENTORY
    if not path.is_file():
        return []
    return [
        line
        for line in path.read_text(encoding="utf-8").splitlines()
        if line and not line.startswith("#")
    ]


def torii_boundary_alias_failures(root: Path) -> list[str]:
    """Reject aliases that could hide boundary use in another Rust module."""

    failures: list[str] = []
    source_paths, _ = torii_rust_source_closure(root)
    for path in source_paths:
        aliases = _boundary_aliases(_rust_tokens(path.read_text(encoding="utf-8")))
        for alias, kind in sorted(aliases.items()):
            relative = path.relative_to(root).as_posix()
            failures.append(
                f"{relative}: {kind} boundary alias {alias!r} is forbidden; "
                "use the audited spelling so cross-module calls remain visible"
            )
    return failures


def torii_source_path_failures(root: Path) -> list[str]:
    """Reject source-closure indirection, escapes, and Git submodules."""

    failures: list[str] = []
    source_paths, rust_source_failures = torii_rust_source_closure(root)
    failures.extend(rust_source_failures)
    git_entries = _git_audited_entries(root)
    if git_entries is not None:
        for mode, relative in git_entries:
            if mode == "160000":
                failures.append(
                    f"{relative.as_posix()}: gitlink/submodule is forbidden in the audited source closure"
                )
            elif mode.startswith("conflict:"):
                stage = mode.split(":", 2)[1]
                failures.append(
                    f"{relative.as_posix()}: unresolved Git index stage {stage} is forbidden in the audited source closure"
                )
    for relative_root in AUDITED_SOURCE_PATHS:
        source_root = root / relative_root
        if not source_root.exists():
            if relative_root in REQUIRED_AUDITED_SOURCE_PATHS:
                failures.append(f"missing audited source root {relative_root.as_posix()}")
            continue
        candidates = [source_root]
        if source_root.is_dir():
            candidates.extend(source_root.rglob("*"))
        for path in candidates:
            if path.is_symlink():
                failures.append(
                    f"{path.relative_to(root).as_posix()}: symlink is forbidden in the audited source closure"
                )

    audited_roots = tuple((root / relative).resolve() for relative in AUDITED_SOURCE_PATHS)
    audited_files = {path.resolve() for path in torii_audited_files(root)}
    if git_entries is not None:
        for source_path in source_paths:
            if source_path.resolve() not in audited_files:
                failures.append(
                    f"{source_path.relative_to(root).as_posix()}: textual module source is "
                    "outside the sealed repository-file inventory"
                )
    manifests = [
        path for path in torii_audited_files(root) if path.name == "Cargo.toml"
    ]
    for manifest in manifests:
        relative_manifest = manifest.relative_to(root).as_posix()
        try:
            data = tomllib.loads(manifest.read_text(encoding="utf-8"))
        except (OSError, UnicodeDecodeError, tomllib.TOMLDecodeError) as error:
            failures.append(f"{relative_manifest}: cannot validate Cargo targets: {error}")
            continue

        target_paths: list[tuple[str, object]] = []
        package = data.get("package", {})
        if isinstance(package, dict) and "build" in package:
            build = package["build"]
            if isinstance(build, str):
                target_paths.append(("package build script", build))
            elif build is not False:
                failures.append(
                    f"{relative_manifest}: package build must be false or one static path"
                )

        library = data.get("lib")
        if isinstance(library, dict) and "path" in library:
            target_paths.append(("lib target", library["path"]))
        for section in ("bin", "example", "test", "bench"):
            entries = data.get(section, [])
            if not isinstance(entries, list):
                continue
            for index, entry in enumerate(entries):
                if isinstance(entry, dict) and "path" in entry:
                    target_paths.append(
                        (f"{section} target #{index + 1}", entry["path"])
                    )

        for label, raw_path in target_paths:
            if not isinstance(raw_path, str) or not raw_path or "\0" in raw_path:
                failures.append(
                    f"{relative_manifest}: {label} must name one static local source file"
                )
                continue
            candidate = manifest.parent / raw_path
            target = candidate.resolve()
            if not any(target.is_relative_to(source_root) for source_root in audited_roots):
                failures.append(
                    f"{relative_manifest}: {label} escapes the audited source roots: {raw_path}"
                )
                continue
            if not candidate.is_file() or candidate.is_symlink():
                failures.append(
                    f"{relative_manifest}: {label} is missing or not a regular sealed file: {raw_path}"
                )
                continue
            if target not in audited_files:
                failures.append(
                    f"{relative_manifest}: {label} is outside the sealed repository-file inventory: {raw_path}"
                )

    manifest = root / "crates/iroha_torii/Cargo.toml"
    if manifest.is_file():
        try:
            data = tomllib.loads(manifest.read_text(encoding="utf-8"))
            package = data.get("package", {})
            build = package.get("build") if isinstance(package, dict) else None
        except (OSError, UnicodeDecodeError, tomllib.TOMLDecodeError):
            build = None
        expected = (root / TORII_BUILD_SCRIPT).resolve()
        if not isinstance(build, str):
            failures.append("crates/iroha_torii/Cargo.toml: missing explicit build script")
        elif (manifest.parent / build).resolve() != expected:
            failures.append(
                "crates/iroha_torii/Cargo.toml: build script escaped the audited "
                f"source closure (expected {TORII_BUILD_SCRIPT.as_posix()})"
            )
    return failures


def closed_torii_boundary_inventory_failures(
    root: Path,
    expected_records: list[str] | None = None,
    observed_inventory: tuple[list[str], str, dict[str, int]] | None = None,
) -> list[str]:
    """Return drift from the reviewed complete Torii-source inventory."""

    if observed_inventory is None:
        observed_inventory = torii_boundary_inventory(root)
    observed_records, observed_digest, observed_counts = observed_inventory
    if expected_records is None:
        expected_records = _reviewed_inventory(root)
    expected_encoded = (
        "\n".join(expected_records) + ("\n" if expected_records else "")
    ).encode("utf-8")
    expected_digest = hashlib.sha256(expected_encoded).hexdigest()
    expected_counts = {kind: 0 for kind in BOUNDARY_IDENTIFIERS}
    for record in expected_records:
        sites = record.rsplit("\t", 1)[-1]
        for site in sites.split(","):
            if not site or "=" not in site or ":" not in site:
                continue
            kind = site.split(":", 1)[0]
            if kind in expected_counts:
                expected_counts[kind] += int(site.rsplit("=", 1)[1])

    failures: list[str] = []
    for token in sorted(BOUNDARY_IDENTIFIERS):
        expected_count = expected_counts.get(token, 0)
        observed_count = observed_counts[token]
        if observed_count != expected_count:
            failures.append(
                f"Torii {token} site count drifted "
                f"(expected {expected_count}, found {observed_count})"
            )
    if observed_digest != expected_digest:
        failures.append(
            "Torii recovery-boundary source inventory drifted "
            f"(expected {expected_digest}, found {observed_digest}); "
            "review --print-inventory output before updating the digest"
        )
        diff = list(
            difflib.unified_diff(
                expected_records,
                observed_records,
                fromfile=REVIEWED_TORII_BOUNDARY_INVENTORY.as_posix(),
                tofile="observed Torii boundary inventory",
                lineterm="",
            )
        )
        failures.extend(diff[:80])
        if len(diff) > 80:
            failures.append(f"... inventory diff truncated ({len(diff) - 80} more lines)")
    return failures


def main() -> int:
    observed_inventory = None
    if "--print-inventory" in sys.argv[1:]:
        observed_inventory = torii_boundary_inventory(ROOT)
        records, digest, counts = observed_inventory
        for record in records:
            print(record)
        print(f"sha256\t{digest}")
        for token in sorted(counts):
            print(f"count\t{token}\t{counts[token]}")
    failures = closed_torii_boundary_inventory_failures(
        ROOT, observed_inventory=observed_inventory
    )
    failures.extend(torii_source_path_failures(ROOT))
    failures.extend(torii_boundary_alias_failures(ROOT))
    for relative in NO_BARE_BLOCKING:
        source = (ROOT / relative).read_text(encoding="utf-8")
        lines = _bare_blocking_lines(source)
        if lines:
            rendered_lines = ", ".join(str(line) for line in lines)
            failures.append(f"{relative}: bare spawn_blocking at line(s) {rendered_lines}")
    for relative in NO_BARE_STD_THREAD:
        source = (ROOT / relative).read_text(encoding="utf-8")
        lines = _bare_std_thread_lines(source)
        if lines:
            rendered_lines = ", ".join(str(line) for line in lines)
            failures.append(f"{relative}: bare std::thread spawn at line(s) {rendered_lines}")

    for relative, snippets in REQUIRED_SNIPPETS.items():
        source = (ROOT / relative).read_text(encoding="utf-8")
        for snippet in snippets:
            if snippet not in source:
                failures.append(f"{relative}: missing audited recovery marker {snippet!r}")

    for relative, snippets in FORBIDDEN_RECOVERY_SNIPPETS.items():
        source = (ROOT / relative).read_text(encoding="utf-8")
        for snippet in snippets:
            if snippet in source:
                failures.append(f"{relative}: unreviewed bare recovery boundary {snippet!r}")

    if failures:
        print("panic recovery boundary guard failed:", file=sys.stderr)
        for failure in failures:
            print(f"- {failure}", file=sys.stderr)
        return 1
    print("panic recovery boundary guard passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
