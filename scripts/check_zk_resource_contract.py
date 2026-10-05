#!/usr/bin/env python3
"""Check the unified end-to-end resource contract against the source.

`specs/zk_resource_contract.json` enumerates every resource bound on the path
SDK admission -> Torii -> queue -> VM -> proposer -> block -> RS16 payload ->
transport/frame/sync -> follower validation -> storage. This script is the
drift guard for that file. It fails when:

* an owner symbol is gone, or its defining expression or value differs from
  the recorded one (the expression is read from the Rust source and evaluated
  here; references to other constants resolve through other verified bounds);
* an enforcement site no longer contains the recorded text or contains it a
  different number of times than recorded (`count`, one when absent), or a site
  is only a bare symbol, a constant definition or a function signature;
* a named test has lost its `#[test]` attribute, is `#[ignore]`d or compiled
  out by `#[cfg(any())]`, or its file is no longer part of its crate (no `mod`,
  `include!` or `#[path]` reaches it, or `autotests = false` without a
  `[[test]]` entry);
* a limit constant or committed default in a scanned owner file or directory
  is neither a listed bound nor excluded with a reason (`completeness`), or a
  field of an authoritative typed catalog in the code (`catalogs`: the
  execution-policy digest, the ZK consensus-policy hash, the Nexus
  consensus-policy preimage and the two committed policy structs) is neither
  listed nor excluded, or a catalog binds a field whose name cannot be read;
* a consensus bound names only tests that pin its value or a digest projection
  (`pin_tests`) without the task that owns the missing boundary test;
* a relation between bounds evaluates differently from its recorded
  expectation, under today's values or under a named target profile;
* a bound is classed inconsistently: a validity-affecting bound read from
  node-local configuration must be recorded as a defect with an owning task,
  a consensus bound names a test or the task that owns the test gap, a local
  resource must defer, a local scheduling limit names where it is applied and
  an engineering target is never validity;
* a separately fixed zk-X509 bound differs from the value the delivery plan
  keeps (`FIXED_BOUNDS` below is the independent copy), a fixed validity bound
  is not classed consensus or a prover time or memory bound is;
* a RAM-LFE budget is given a number while one of its inputs is still open,
  or stays open after every input is known, or an open input or a shape bound
  is used by no budget;
* a stage has neither an exact and one-over test nor a task that owns the gap,
  or the deferral and deterministic-rejection behaviors name no test;
* an open item names a task the delivery graph does not contain, or a task
  the graph marks `implemented` still has open items here;
* a reason, note or description cites a bound id this contract does not have.

Expression reading is textual. It finds one definition by name and kind
(`const`, `fn`, `field`, `arm`) after its anchors and evaluates integer
arithmetic only. It is a source regression guard, not a Rust parser or a
substitute for the Rust tests the contract names. A limit is found by its
name (`LIMIT_NAME`): a limit named otherwise is found only once it is listed.

Maintenance. The contract is edited by hand; two modes do the mechanical part:

* `--refresh` rewrites every bound's recorded expression and value from the
  source, recomputes the derived bounds, the closed RAM-LFE budgets and the
  occurrence count of every site that is still present, and removes exclusions
  that match nothing any more. It keeps every classification, site text, test
  and relation as written and prints each count it changed and each exclusion
  it removed. Use it after an owner changed on purpose, then review the diff:
  a changed value may break a relation, which the following check reports.
* `--propose` writes nothing. For every scanned limit or catalog field that is
  neither listed nor excluded it prints a bound skeleton with the owner
  expression, the value, candidate comparison sites and candidate tests; for
  every site whose text is gone it prints the closest lines of the file; and
  it lists stale exclusions. `--propose-for PATH:SYMBOL` prints the same
  skeleton for one constant.

Prerequisites: Python 3.9+ standard library, run from a checkout. No
environment variables are read. Nothing is written without `--refresh`.

Usage:
    python3 scripts/check_zk_resource_contract.py
    python3 scripts/check_zk_resource_contract.py --show      # print every bound
    python3 scripts/check_zk_resource_contract.py --refresh   # rewrite expr, value, counts
    python3 scripts/check_zk_resource_contract.py --propose   # suggest entries, write nothing
"""

from __future__ import annotations

import argparse
import ast
import difflib
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CONTRACT = ROOT / "specs" / "zk_resource_contract.json"
GRAPH = ROOT / "specs" / "zk_delivery_graph.json"
SCHEMA = "iroha.zk_resource_contract.v1"

CLASSES = ("consensus", "local_defer", "local_schedule", "engineering_target")
SOURCES = ("committed", "constant", "node_local")
OWNER_KINDS = ("const", "fn", "field", "arm")
STAGES = (
    "sdk",
    "torii",
    "queue",
    "vm",
    "proposer",
    "block",
    "rs16",
    "transport",
    "sync",
    "follower",
    "storage",
)
GROUPS = (
    "proof",
    "attachment",
    "transaction",
    "block",
    "rs16",
    "transport",
    "sync",
    "storage",
    "queue",
    "guest_memory",
    "vm_handles",
    "decoded_scratch",
    "queued_effects",
    "work_budget",
    "x509",
    "ram_lfe",
)
EXPECTATIONS = ("holds", "violated")
# The parts of the RAM-LFE workload a budget must exist for (delivery plan, "Mandatory
# RAM-LFE and phone claims"): what is registered, submitted, evaluated, proved, verified,
# opened by one opener or a threshold of them, and what a validator holds while checking.
WORKLOAD_COMPONENTS = (
    "program",
    "key_registration",
    "request",
    "evaluation",
    "receipt",
    "verification",
    "opening",
    "threshold_opening",
    "validator_scratch",
    "class_affine",
    "class_bounded",
    "class_refresh",
    "leakage",
    "prover",
)
GIB = 1024 * 1024 * 1024
# Independent copy of the bounds the delivery plan keeps fixed ("X509 transport
# holder custody and time"). The contract cannot relax one by editing itself.
FIXED_BOUNDS = {
    "x509.max_proof_bytes": 9_437_184,
    "transaction.max_tx_bytes": 10_485_760,
    "x509.max_crl_age_seconds": 300,
    "x509.max_presentation_window_seconds": 300,
    "x509.prover_target_seconds": 300,
    "x509.prover_peak_rss_bytes": 12 * GIB,
    "x509.prover_address_space_bytes": 32 * GIB,
}
# Bounds measured on an encoded structure state what framing the length includes.
FRAMED_BOUNDS = (
    "transaction.max_tx_bytes",
    "transaction.max_decompressed_bytes",
    "block.max_block_bytes",
    "proof.privacy_max_proof_bytes_per_action",
    "proof.privacy_max_action_bytes",
    "proof.proof_box_max_encoded_bytes",
    "x509.max_proof_bytes",
    "queued_effects.max_output_bytes",
    "attachment.list_max_canonical_frame_bytes",
)
# Fixed bounds every validator decides on: they are consensus validity, with sites and tests.
FIXED_VALIDITY = (
    "x509.max_proof_bytes",
    "transaction.max_tx_bytes",
    "x509.max_crl_age_seconds",
    "x509.max_presentation_window_seconds",
)
# The fixed zk-X509 time and memory bounds are targets and prover-process limits.
FIXED_NON_VALIDITY = (
    "x509.prover_target_seconds",
    "x509.prover_peak_rss_bytes",
    "x509.prover_address_space_bytes",
)
# Names that mark a constant as a limit for the completeness scan.
LIMIT_NAME = re.compile(
    r"(?:^|_)(?:MAX|MAXIMUM)(?:_|$)|LIMIT|BUDGET|CEILING|OVERHEAD|RESERVE"
    r"|(?:^|_)CAP(?:ACITY)?(?:_|$)"
)
CONST_DEFINITION = re.compile(
    r"(?:^|[{;])[ \t]*(?:pub(?:\([^)]*\))?[ \t]+)?const[ \t]+([A-Z][A-Z0-9_]*)[ \t]*:",
    re.MULTILINE,
)
DEFAULT_FN_DEFINITION = re.compile(
    r"(?:^|[{;}])[ \t]*pub[ \t]+(?:const[ \t]+)?fn[ \t]+([a-z][a-z0-9_]*)[ \t]*\(\s*\)",
    re.MULTILINE,
)
# An inline test module: `#[cfg(test)]` (or `#[cfg(all(test, ...))]`), then any doc
# comments and further attributes, then `mod name {`.
INLINE_TEST_MODULE = re.compile(
    r"^[ \t]*#\[cfg\((?:test|all\(\s*test\b[^\]\n]*\))\)\][ \t]*\n"
    r"(?:[ \t]*(?:///[^\n]*|#\[[^\n]*\])[ \t]*\n)*"
    r"[ \t]*(?:pub(?:\([^)]*\))?[ \t]+)?mod[ \t]+\w+[ \t]*\{",
    re.MULTILINE,
)
# Files a directory scope never scans: test, bench, example and fixture sources.
TEST_FILE = re.compile(
    r"(?:^|/)(?:tests|benches|examples|fixtures)/|_tests?/"
    r"|(?:^|/)(?:tests?|[a-z0-9_]*_tests?|test_[a-z0-9_]*|[a-z0-9_]*fixtures?[a-z0-9_]*)\.rs$"
)
# The typed catalogs every contract checks against the source.
REQUIRED_CATALOGS = (
    "execution_policy_digest_v1",
    "zk_consensus_policy_hash",
    "nexus_consensus_policy_v1",
    "execution_output_policy_v1",
    "fastpq_source_policy_v1",
)
CATALOG_FORMS = ("policy_push", "zk_policy_put", "struct_tree")
TEST_ATTRIBUTE = re.compile(r"#\[\s*(?:tokio\s*::\s*)?test\b")
IGNORE_ATTRIBUTE = re.compile(r"#\[\s*(?:ignore\b|cfg_attr\s*\([^\]]*\bignore\b)")
# `#[cfg(any())]` is never true: the item it marks is not compiled.
COMPILED_OUT = re.compile(r"#\[\s*cfg\s*\(\s*any\s*\(\s*\)\s*\)\s*\]")
PYTHON_SKIP = re.compile(r"@\s*(?:pytest\.mark\.(?:skip|skipif|xfail)|unittest\.skip)")
# A site that is one whole constant or static item, or one function signature.
CONST_ITEM = re.compile(
    r"(?:pub(?:\([^)]*\))?\s+)?(?:const|static)\s+\w+\s*:\s*[^=]+=.*", re.DOTALL
)
FN_SIGNATURE = re.compile(
    r"(?:pub(?:\([^)]*\))?\s+)?(?:(?:const|async|unsafe)\s+)*fn\s+\w+\s*(?:<[^({]*>)?\s*"
    r"\([^{]*(?:\{\s*)?"
)

LINE_COMMENT = re.compile(r"//[^\n]*")
INT_LITERAL = re.compile(
    r"\b(0x[0-9a-fA-F_]+|[0-9][0-9_]*)(?:_?(?:u8|u16|u32|u64|u128|usize|i32|i64))?\b"
)
PATH = re.compile(r"[A-Za-z_][A-Za-z_0-9]*(?:::[A-Za-z_][A-Za-z_0-9]*)*")
CAST = re.compile(r"\bas\s+(?:u8|u16|u32|u64|u128|usize|i32|i64)\b")
POW = re.compile(r"(\d+)\s*\.\s*pow\s*\(\s*(\d+)\s*\)")
METHOD_GET = re.compile(r"\.\s*get\s*\(\s*\)")
EXPECT = re.compile(r"\.\s*(?:expect\s*\(\s*\"[^\"]*\"\s*\)|unwrap\s*\(\s*\))")
WRAPPERS = re.compile(
    r"\b(?:nonzero_ext::nonzero!|nonzero!|Bytes|u64::from|u32::from|usize::from"
    r"|NonZeroUsize::new|NonZeroU64::new|NonZeroU32::new)\s*\("
)
EMPTY_CALL = re.compile(r"\(\s*\)")
ID = re.compile(r"[a-z][a-z0-9_]*(?:\.[a-z0-9_]+)+")


class ContractError(Exception):
    """One reportable inconsistency."""


def normalize(text: str) -> str:
    """Drop line comments and collapse whitespace."""
    return " ".join(LINE_COMMENT.sub("", text).split())


def _after_anchor(source: str, owner: dict, where: str) -> str:
    """Return the source after the owner's anchors, applied in order."""
    anchors = owner.get("anchor")
    if anchors is None:
        return source
    if isinstance(anchors, str):
        anchors = [anchors]
    for anchor in anchors:
        position = source.find(anchor)
        if position < 0:
            raise ContractError(f"{where}: anchor {anchor!r} is not in {owner['path']}")
        source = source[position + len(anchor) :]
    return source


def extract_expression(source: str, owner: dict, where: str) -> str:
    """Return the normalized defining expression of `owner` in `source`."""
    kind = owner["kind"]
    symbol = re.escape(owner["symbol"])
    scope = _after_anchor(source, owner, where)
    if kind == "const":
        pattern = rf"\bconst\s+{symbol}\s*:\s*[^=;]+?=\s*(?P<expr>.*?);"
    elif kind == "fn":
        pattern = rf"\bfn\s+{symbol}\s*\([^)]*\)\s*->\s*[^{{;]+\{{(?P<expr>.*?)\}}"
    elif kind == "field":
        pattern = rf"\b{symbol}\s*:\s*(?P<expr>[^,\n}}]+)[,}}\n]"
    elif kind == "arm":
        pattern = rf"{symbol}\s*=>\s*(?P<expr>[^,\n]+),"
    else:
        raise ContractError(f"{where}: unknown owner kind {kind!r}")
    match = re.search(pattern, scope, re.DOTALL)
    if match is None:
        raise ContractError(
            f"{where}: {kind} `{owner['symbol']}` is not defined in {owner['path']}"
        )
    return normalize(match.group("expr"))


_BINARY = {
    ast.Add: lambda a, b: a + b,
    ast.Sub: lambda a, b: a - b,
    ast.Mult: lambda a, b: a * b,
    ast.FloorDiv: lambda a, b: a // b,
    ast.Mod: lambda a, b: a % b,
    ast.LShift: lambda a, b: a << b,
    ast.RShift: lambda a, b: a >> b,
    ast.Pow: lambda a, b: a**b,
}
_COMPARE = {
    ast.LtE: lambda a, b: a <= b,
    ast.Lt: lambda a, b: a < b,
    ast.GtE: lambda a, b: a >= b,
    ast.Gt: lambda a, b: a > b,
    ast.Eq: lambda a, b: a == b,
    ast.NotEq: lambda a, b: a != b,
}
_FUNCTIONS = {
    "min": min,
    "max": max,
    "ceil_div": lambda a, b: -(-a // b),
}


def _evaluate(node: ast.AST, where: str):
    if isinstance(node, ast.Expression):
        return _evaluate(node.body, where)
    if isinstance(node, ast.Constant) and type(node.value) is int:
        return node.value
    if isinstance(node, ast.BinOp) and type(node.op) in _BINARY:
        return _BINARY[type(node.op)](
            _evaluate(node.left, where), _evaluate(node.right, where)
        )
    if isinstance(node, ast.UnaryOp) and isinstance(node.op, ast.USub):
        return -_evaluate(node.operand, where)
    if isinstance(node, ast.BoolOp):
        values = [_evaluate(value, where) for value in node.values]
        return all(values) if isinstance(node.op, ast.And) else any(values)
    if isinstance(node, ast.Compare):
        left = _evaluate(node.left, where)
        for operator, right_node in zip(node.ops, node.comparators):
            right = _evaluate(right_node, where)
            if type(operator) not in _COMPARE or not _COMPARE[type(operator)](left, right):
                if type(operator) not in _COMPARE:
                    raise ContractError(f"{where}: unsupported comparison")
                return False
            left = right
        return True
    if (
        isinstance(node, ast.Call)
        and isinstance(node.func, ast.Name)
        and node.func.id in _FUNCTIONS
        and not node.keywords
    ):
        return _FUNCTIONS[node.func.id](*[_evaluate(arg, where) for arg in node.args])
    raise ContractError(f"{where}: unsupported expression element {ast.dump(node)[:60]}")


def safe_eval(text: str, where: str):
    """Evaluate integer arithmetic, comparisons, `and`/`or`, min, max, ceil_div."""
    try:
        tree = ast.parse(text, mode="eval")
    except SyntaxError as error:
        raise ContractError(f"{where}: cannot parse {text!r}: {error.msg}") from None
    return _evaluate(tree, where)


def _literal(text: str) -> int:
    digits = text.replace("_", "")
    return int(digits, 16) if digits.lower().startswith("0x") else int(digits, 10)


def evaluate_rust(expression: str, refs: dict, values: dict, where: str) -> int:
    """Evaluate a Rust integer constant expression.

    `refs` maps a referenced path (as written, or its last segment) to the id
    of the bound that owns it; `values` holds the verified value of each id.
    """
    text = CAST.sub("", expression)
    text = EXPECT.sub("", text)
    text = METHOD_GET.sub("", text)
    text = WRAPPERS.sub("(", text)
    text = EMPTY_CALL.sub("", text)
    text = INT_LITERAL.sub(lambda match: str(_literal(match.group(1))), text)
    text = POW.sub(r"(\1 ** \2)", text)

    def resolve(match: re.Match) -> str:
        path = match.group(0)
        target = refs.get(path, refs.get(path.rsplit("::", 1)[-1]))
        if target is None:
            raise ContractError(
                f"{where}: `{path}` in `{expression}` has no entry in `refs`"
            )
        if target not in values:
            raise ContractError(f"{where}: `{path}` refers to unverified bound {target}")
        return str(values[target])

    text = PATH.sub(resolve, text)
    text = text.replace("/", "//")
    if not re.fullmatch(r"[0-9\s+\-*/()<>]*", text) or not text.strip():
        raise ContractError(f"{where}: `{expression}` is not integer arithmetic")
    value = safe_eval(text, where)
    if type(value) is not int:
        raise ContractError(f"{where}: `{expression}` is not an integer")
    return value


def evaluate_relation(expression: str, values: dict, where: str):
    """Evaluate a relation or formula over bound ids."""

    def resolve(match: re.Match) -> str:
        identifier = match.group(0)
        if identifier not in values:
            raise ContractError(f"{where}: unknown or open bound `{identifier}`")
        return str(values[identifier])

    return safe_eval(ID.sub(resolve, expression), where)


_SOURCES: dict = {}


def _read(root: Path, relative: str, where: str) -> str:
    """Read one source file; a file is read once per size and modification time."""
    path = root / relative
    if not path.is_file():
        raise ContractError(f"{where}: {relative} does not exist")
    stat = path.stat()
    key = (str(path), stat.st_mtime_ns, stat.st_size)
    if key not in _SOURCES:
        _SOURCES[key] = path.read_text(encoding="utf-8")
    return _SOURCES[key]


_NORMALIZED: dict = {}


def _normalized(text: str) -> str:
    """`normalize` of a whole source file, computed once."""
    key = (len(text), hash(text))
    if key not in _NORMALIZED:
        _NORMALIZED[key] = normalize(text)
    return _NORMALIZED[key]


def _require(record: dict, keys: tuple, where: str) -> None:
    missing = [key for key in keys if key not in record]
    if missing:
        raise ContractError(f"{where}: missing {', '.join(missing)}")


def _definition_only(site: dict, owner: dict | None = None) -> bool:
    """Whether `site` names no comparison or application of a bound.

    A bare symbol, one constant or static item (the owner's or any other, such as an
    alias) and one function signature prove nothing about enforcement: removing the
    real comparison would leave them in place. A constant item that is a compile-time
    assertion does compare and is accepted. With `owner`, the owner's own definition
    in its file is rejected in every form the checker reads it.
    """
    text = normalize(site["contains"])
    if PATH.fullmatch(text.strip(" ,;.")):
        return True
    if (
        CONST_ITEM.fullmatch(text)
        and ";" not in text.rstrip("; ")
        and "assert" not in text
    ):
        return True
    if FN_SIGNATURE.fullmatch(text):
        return True
    if owner is None or site["path"] != owner["path"]:
        return False
    symbol = re.escape(owner["symbol"])
    return bool(
        re.search(rf"\bconst\s+{symbol}\s*:", text)
        or re.search(rf"\bfn\s+{symbol}\s*\(\s*\)", text)
        or re.fullmatch(rf"{symbol}\s*(?::|=>).*", text)
    )


_TEST_PROBLEMS: dict = {}


def test_problem(
    source: str,
    path: str,
    name: str,
    macro: str | None = None,
    macro_source: str | None = None,
) -> str | None:
    """Return why `name` is not a test that runs in `source`, or None when it is.

    `macro` names a `macro_rules!` test generator (for example `state_test`), for a test
    written as `state_test! { sync name ... }` or `world_test!(name { ... })`. The
    generator is defined in the same file, or in the file whose text is `macro_source`
    when the test file is an `include!` of it.
    """
    key = (
        len(source),
        hash(source),
        path.endswith(".py"),
        name,
        macro,
        None if macro_source is None else hash(macro_source),
    )
    if key not in _TEST_PROBLEMS:
        _TEST_PROBLEMS[key] = (
            _test_problem(source, path, name)
            if macro is None
            else _macro_test_problem(source, name, macro, macro_source)
        )
    return _TEST_PROBLEMS[key]


def _macro_test_problem(
    source: str, name: str, macro: str, macro_source: str | None = None
) -> str | None:
    """A test generated by a `macro_rules!` whose arms carry `#[test]`."""
    generator = re.escape(macro)
    defining = source if macro_source is None else macro_source
    definition = re.search(rf"\bmacro_rules!\s*{generator}\s*\{{", defining)
    if definition is None:
        return f"names the test macro `{macro}!`, which is not defined for"
    end = _block_end(defining, definition.end() - 1)
    body = defining[definition.end() : end if end > 0 else len(defining)]
    if not TEST_ATTRIBUTE.search(body):
        return f"names `{macro}!`, which generates no `#[test]` in"
    if IGNORE_ATTRIBUTE.search(body):
        return f"names `{macro}!`, which generates an `#[ignore]`d test in"
    invocation = rf"\b{generator}!\s*[\{{(]\s*(?:[a-z_]+\s+)?{re.escape(name)}\b"
    if not re.search(invocation, source):
        return "is not in"
    return None


def _named_test_problem(root: Path, test: dict, where: str) -> str | None:
    """`test_problem` for one test record of the contract."""
    macro_source = (
        _read(root, test["macro_path"], where) if test.get("macro_path") else None
    )
    return test_problem(
        _read(root, test["path"], where),
        test["path"],
        test["name"],
        test.get("macro"),
        macro_source,
    )


def _test_problem(source: str, path: str, name: str) -> str | None:
    escaped = re.escape(name)
    if path.endswith(".py"):
        match = re.search(rf"^([ \t]*)def\s+{escaped}\s*\(", source, re.MULTILINE)
        if match is None:
            return "is not in"
        lines = source[: match.start()].splitlines()
        while lines and lines[-1].strip().startswith("@"):
            if PYTHON_SKIP.search(lines.pop()):
                return "is skipped in"
        return None
    found = False
    for match in re.finditer(rf"\bfn\s+{escaped}\s*\(", source):
        found = True
        # The item header: everything after the previous item or block ends.
        head = LINE_COMMENT.sub("", source[max(0, match.start() - 6000) : match.start()])
        head = head[max(head.rfind("}"), head.rfind(";"), head.rfind("{")) + 1 :]
        if not TEST_ATTRIBUTE.search(head):
            continue
        if IGNORE_ATTRIBUTE.search(head):
            return "is `#[ignore]`d in"
        if COMPILED_OUT.search(head):
            return "is compiled out by `#[cfg(any())]` in"
        return None
    return "has no `#[test]` attribute in" if found else "is not in"


_REACHABLE: dict = {}
_REFERENCES: dict = {}
_MANIFESTS: dict = {}
_CODE: dict = {}
_KNOWN_REFERRERS: dict = {}
_DECLARES: dict = {}


def _code(file: Path) -> str:
    """The text of `file` without line comments; read once per size and modification time."""
    stat = file.stat()
    key = (str(file), stat.st_mtime_ns, stat.st_size)
    if key not in _CODE:
        _CODE[key] = LINE_COMMENT.sub("", file.read_text(encoding="utf-8"))
    return _CODE[key]


def _crate_of(root: Path, file: Path) -> Path | None:
    """The nearest directory at or below `root` that holds the `Cargo.toml` of `file`."""
    directory = file.parent
    while True:
        key = str(directory)
        if key not in _MANIFESTS:
            _MANIFESTS[key] = (directory / "Cargo.toml").is_file()
        if _MANIFESTS[key]:
            return directory
        if directory == root or root not in directory.parents:
            return None
        directory = directory.parent


def _crate_references(crate: Path) -> dict:
    """Every `include!("..")` and `#[path = ".."]` in the Rust files of `crate`.

    Maps the resolved target to the files that name it. A reference marked
    `#[cfg(any())]` does not count. Built at most once per run.
    """
    key = str(crate)
    if key not in _REFERENCES:
        found: dict = {}
        for file in sorted(crate.rglob("*.rs")):
            if "target" in file.relative_to(crate).parts:
                continue
            raw = file.read_text(encoding="utf-8")
            if "include!" not in raw and "path" not in raw:
                continue
            text = LINE_COMMENT.sub("", raw)
            for match in re.finditer(
                r"(?:include!\s*\(\s*|#\[\s*path\s*=\s*)\"([^\"]+\.rs)\"", text
            ):
                if COMPILED_OUT.search(text[max(0, match.start() - 160) : match.start()]):
                    continue
                target = (file.parent / match.group(1)).resolve()
                found.setdefault(str(target), []).append(file)
        _REFERENCES[key] = found
    return _REFERENCES[key]


def _referrers_by_path(crate: Path, file: Path) -> list:
    """The files of `crate` that `include!` or `#[path]` the file.

    The answer of an earlier run is reused while every file that gave it is unchanged;
    otherwise the crate is read again, so a removed reference is always noticed.
    """
    key = str(file.resolve())
    known = _KNOWN_REFERRERS.get(key)
    if known and all(
        referrer.is_file() and referrer.stat().st_mtime_ns == modified
        for referrer, modified in known
    ):
        return [referrer for referrer, _ in known]
    referrers = _crate_references(crate).get(key, [])
    if referrers:
        _KNOWN_REFERRERS[key] = [(referrer, referrer.stat().st_mtime_ns) for referrer in referrers]
    else:
        _KNOWN_REFERRERS.pop(key, None)
    return referrers


def _declares_module(file: Path, name: str) -> bool:
    """Whether `file` declares `mod name;` (or an inline `mod name {`) that is compiled."""
    if not file.is_file():
        return False
    stat = file.stat()
    key = (str(file), stat.st_mtime_ns, stat.st_size, name)
    if key not in _DECLARES:
        text = _code(file)
        _DECLARES[key] = False
        for match in re.finditer(rf"\bmod\s+{re.escape(name)}\s*[;{{]", text):
            head = text[max(0, match.start() - 200) : match.start()]
            head = head[max(head.rfind(";"), head.rfind("}"), head.rfind("{")) + 1 :]
            # `#[path = ".."] mod name;` names another file; `_referrers_by_path` reads it.
            if not COMPILED_OUT.search(head) and not re.search(r"#\[\s*path\b", head):
                _DECLARES[key] = True
                break
    return _DECLARES[key]


def _reaches_crate_root(crate: Path, file: Path, seen: frozenset) -> bool:
    """Whether a chain of `mod`, `include!` and `#[path]` joins `file` to a crate target.

    Targets are `src/lib.rs`, `src/main.rs`, `src/bin/*`, a path the manifest names, and
    a top-level file of `tests/`, `benches/` or `examples/` unless the manifest turns
    automatic discovery of that directory off.
    """
    key = str(file)
    if key in _REACHABLE:
        return _REACHABLE[key]
    if key in seen:
        return False
    seen = seen | {key}
    parts = file.relative_to(crate).parts
    manifest = (crate / "Cargo.toml").read_text(encoding="utf-8")
    automatic = {"tests": "autotests", "benches": "autobenches", "examples": "autoexamples"}
    if parts[0] == "src" and (
        (len(parts) == 2 and parts[1] in ("lib.rs", "main.rs"))
        or (len(parts) == 3 and parts[1] == "bin")
    ):
        result = True
    elif f'"{"/".join(parts)}"' in manifest:
        result = True
    elif (
        parts[0] in automatic
        and (len(parts) == 2 or (len(parts) == 3 and parts[2] == "main.rs"))
        and not re.search(rf"^\s*{automatic[parts[0]]}\s*=\s*false", manifest, re.MULTILINE)
    ):
        result = True
    else:
        referrers: list = []
        module, directory = file.stem, file.parent
        if file.name == "mod.rs":
            module, directory = directory.name, directory.parent
        # `mod name;` in the parent module file, or in an ancestor for a nested inline module.
        ancestor = directory
        for _ in range(4):
            if crate not in (ancestor, *ancestor.parents):
                break
            for parent in (
                ancestor / "mod.rs",
                ancestor.parent / f"{ancestor.name}.rs",
                ancestor / "lib.rs",
                ancestor / "main.rs",
            ):
                if parent != file and _declares_module(parent, module):
                    referrers.append(parent)
            ancestor = ancestor.parent
        result = any(_reaches_crate_root(crate, referrer, seen) for referrer in referrers)
        if not result:
            # Otherwise an `include!` or a `#[path]` somewhere in the crate names the file.
            result = any(
                referrer != file and _reaches_crate_root(crate, referrer, seen)
                for referrer in _referrers_by_path(crate, file)
            )
    _REACHABLE[key] = result
    return result


def unreachable_test_file(root: Path, relative: str) -> bool:
    """Whether the Rust test file is in a crate and no module path of that crate reaches it.

    A file without a crate manifest above it (a scratch source) is not judged.
    """
    if not relative.endswith(".rs"):
        return False
    file = root / relative
    crate = _crate_of(root, file)
    if crate is None:
        return False
    return not _reaches_crate_root(crate, file, frozenset())


def _check_sites(
    root: Path,
    record: dict,
    where: str,
    problems: list,
    owner: dict | None = None,
    refresh: bool = False,
    notes: list | None = None,
) -> None:
    """Check the enforcement sites, tests and observations of one record.

    A site records how many times its text occurs in its file (`count`, one when
    absent): removing one of two identical comparisons is drift. With `refresh` the
    count of a site that is still present is rewritten and reported in `notes`.
    """
    for site in record.get("enforcement", []):
        try:
            _require(site, ("stage", "path", "contains"), where)
            if site["stage"] not in STAGES:
                raise ContractError(f"{where}: unknown stage {site['stage']!r}")
            source = _normalized(_read(root, site["path"], where))
            occurrences = source.count(normalize(site["contains"]))
            if occurrences == 0:
                raise ContractError(
                    f"{where}: {site['path']} no longer contains `{site['contains']}`"
                )
            if _definition_only(site, owner):
                raise ContractError(
                    f"{where}: the site `{site['contains']}` is only a symbol, a "
                    "definition or a function signature; name where the bound is "
                    "compared or applied"
                )
            recorded = site.get("count", 1)
            if type(recorded) is not int or recorded < 1 or ("count" in site and recorded == 1):
                raise ContractError(
                    f"{where}: `count` of the site `{site['contains']}` is an integer "
                    "above one; a site that occurs once records none"
                )
            if occurrences != recorded:
                if not refresh:
                    raise ContractError(
                        f"{where}: {site['path']} contains `{site['contains']}` "
                        f"{occurrences} time(s); the contract records {recorded}"
                    )
                if occurrences == 1:
                    del site["count"]
                else:
                    site["count"] = occurrences
                if notes is not None:
                    notes.append(
                        f"{where}: site `{site['contains']}` in {site['path']} now occurs "
                        f"{occurrences} time(s), was {recorded}; confirm the change was meant"
                    )
        except ContractError as error:
            problems.append(str(error))
    for test in record.get("tests", []):
        try:
            _require(test, ("path", "name"), where)
            reason = _named_test_problem(root, test, where)
            if reason is not None:
                raise ContractError(f"{where}: test `{test['name']}` {reason} {test['path']}")
            if unreachable_test_file(root, test["path"]):
                raise ContractError(
                    f"{where}: test `{test['name']}` is in {test['path']}, which no "
                    "`mod`, `include!`, `#[path]` or test target of its crate reaches"
                )
        except ContractError as error:
            problems.append(str(error))
    for observation in record.get("observations", []):
        try:
            _require(observation, ("path", "contains", "what"), where)
            source = _normalized(_read(root, observation["path"], where))
            if normalize(observation["contains"]) not in source:
                raise ContractError(
                    f"{where}: {observation['path']} no longer contains "
                    f"`{observation['contains']}`"
                )
        except ContractError as error:
            problems.append(str(error))


def verify_bounds(root: Path, contract: dict, problems: list, refresh: bool = False) -> dict:
    """Verify every bound against its owner; return the verified id -> value map.

    With `refresh`, the recorded expression and value are replaced by what the
    source defines instead of being compared with it.
    """
    values: dict = {}
    identifiers = [bound.get("id", "<bound without id>") for bound in contract["bounds"]]
    for identifier in sorted(set(identifiers)):
        if identifiers.count(identifier) > 1:
            problems.append(f"{identifier}: duplicate bound id")
    known = set(identifiers)
    failed: set = set()
    pending = list(contract["bounds"])
    # A bound may reference a bound listed later: resolve until nothing is left.
    while pending:
        waiting = []
        for bound in pending:
            where = bound.get("id", "<bound without id>")
            refs = bound.get("refs", {})
            unresolved = [target for target in refs.values() if target not in values]
            if unresolved and all(
                target in known and target not in failed for target in unresolved
            ):
                waiting.append(bound)
                continue
            try:
                for target in unresolved:
                    raise ContractError(
                        f"{where}: `refs` names {target}, which is "
                        + ("not verified" if target in known else "not a bound")
                    )
                required = ("id", "group", "what", "unit", "source", "class", "owner")
                _require(bound, required if refresh else required + ("value",), where)
                owner = bound["owner"]
                owner_keys = ("path", "symbol", "kind")
                _require(owner, owner_keys if refresh else owner_keys + ("expr",), where)
                expression = extract_expression(
                    _read(root, owner["path"], where), owner, where
                )
                if refresh:
                    owner["expr"] = expression
                elif expression != normalize(owner["expr"]):
                    raise ContractError(
                        f"{where}: `{owner['symbol']}` in {owner['path']} is now "
                        f"`{expression}`; the contract records `{owner['expr']}`"
                    )
                value = evaluate_rust(expression, refs, values, where)
                if refresh:
                    bound["value"] = value
                elif value != bound["value"]:
                    raise ContractError(
                        f"{where}: `{owner['symbol']}` evaluates to {value}; "
                        f"the contract records {bound['value']}"
                    )
                values[where] = value
            except ContractError as error:
                problems.append(str(error))
                failed.add(where)
        if len(waiting) == len(pending):
            for bound in waiting:
                problems.append(f"{bound['id']}: `refs` form a cycle")
            break
        pending = waiting
    return values


def pin_tests(contract: dict) -> set:
    """The `(path, name)` of every test recorded as a pin of a value or projection."""
    return {(test.get("path"), test.get("name")) for test in contract.get("pin_tests", [])}


def check_pin_tests(root: Path, contract: dict, problems: list) -> None:
    """Each pin test exists, states what it pins and is named by a bound."""
    named = {
        (test.get("path"), test.get("name"))
        for bound in contract["bounds"]
        for test in bound.get("tests", [])
    }
    seen = set()
    for test in contract.get("pin_tests", []):
        where = f"pin_tests {test.get('name', '<without name>')}"
        try:
            _require(test, ("path", "name", "pins", "what"), where)
            if test["pins"] not in ("value", "projection"):
                raise ContractError(f"{where}: `pins` is `value` or `projection`")
            key = (test["path"], test["name"])
            if key in seen:
                raise ContractError(f"{where}: listed twice")
            seen.add(key)
            reason = _named_test_problem(root, test, where)
            if reason is not None:
                raise ContractError(f"{where}: {reason} {test['path']}")
            if key not in named:
                raise ContractError(f"{where}: no bound names this test")
        except ContractError as error:
            problems.append(str(error))


def check_classification(contract: dict, tasks: dict, problems: list) -> None:
    groups = set()
    pins = pin_tests(contract)
    relations = {relation.get("id"): relation for relation in contract.get("relations", [])}
    for bound in contract["bounds"]:
        where = bound.get("id", "<bound without id>")
        groups.add(bound.get("group"))
        if bound.get("group") not in GROUPS:
            problems.append(f"{where}: unknown group {bound.get('group')!r}")
        if bound.get("class") not in CLASSES:
            problems.append(f"{where}: unknown class {bound.get('class')!r}")
        if bound.get("source") not in SOURCES:
            problems.append(f"{where}: unknown source {bound.get('source')!r}")
        if bound.get("owner", {}).get("kind") not in OWNER_KINDS:
            problems.append(f"{where}: unknown owner kind")
        defect = bound.get("defect")
        if bound.get("class") == "consensus":
            if bound.get("source") == "node_local" and defect is None:
                problems.append(
                    f"{where}: a validity-affecting bound read from node-local "
                    "configuration must be recorded as a defect with an owner task"
                )
            if bound.get("source") == "committed" and not bound.get("committed_as"):
                problems.append(f"{where}: a committed bound names `committed_as`")
            if "unenforced" in bound:
                # A value that is bound into consensus state but compared nowhere yet.
                if bound.get("enforcement"):
                    problems.append(
                        f"{where}: a bound with enforcement sites is not `unenforced`"
                    )
                _check_open_owner(bound["unenforced"], tasks, f"{where}: unenforced", problems)
            elif not bound.get("enforcement"):
                problems.append(f"{where}: a consensus bound lists its enforcement sites")
            # Every consensus bound is backed by a test of its boundary, or by the task
            # that owns the gap. A test that only pins the value or shows that the value
            # changes a digest projection (`pin_tests`) is not a test of the boundary.
            tests = bound.get("tests") or []
            only_pins = bool(tests) and all(
                (test.get("path"), test.get("name")) in pins for test in tests
            )
            if "test_gap" in bound:
                if tests and not only_pins:
                    problems.append(
                        f"{where}: a bound with a test of its boundary records no `test_gap`"
                    )
                _check_open_owner(bound["test_gap"], tasks, f"{where}: test_gap", problems)
            elif not tests:
                problems.append(
                    f"{where}: a consensus bound names a test of the bound, or the task "
                    "that owns the gap in `test_gap`"
                )
            elif only_pins and "unenforced" not in bound and not bound.get("value_pin"):
                problems.append(
                    f"{where}: its tests only pin the value or a digest projection "
                    "(`pin_tests`); name a test of the boundary, the task that owns it in "
                    "`test_gap`, or state in `value_pin` why a pin is the right test"
                )
            if "value_pin" in bound and ("test_gap" in bound or not only_pins):
                problems.append(
                    f"{where}: `value_pin` is for a bound whose every test is a pin and "
                    "that records no `test_gap`"
                )
        if bound.get("class") == "local_defer":
            if bound.get("on_exhaustion") == "defer":
                if not bound.get("tests"):
                    problems.append(
                        f"{where}: a deferring local resource names the test of its deferral"
                    )
            elif bound.get("on_exhaustion") == "local_refusal":
                # A local refusal is acceptable only where a relation shows it cannot
                # refuse something the committed bounds admit (or records that gap).
                guard = relations.get(bound.get("guarded_by"))
                if guard is None:
                    problems.append(
                        f"{where}: a locally refusing bound names its relation in `guarded_by`"
                    )
            elif bound.get("on_exhaustion") == "prover_failure":
                # A limit on a proving process: nothing has been submitted yet.
                if not bound.get("note"):
                    problems.append(
                        f"{where}: a prover-side limit states in `note` why no validator "
                        "decides on it"
                    )
                if not bound.get("enforcement"):
                    problems.append(
                        f"{where}: a prover-side limit names where the prover applies it"
                    )
            else:
                problems.append(
                    f"{where}: a local resource records `on_exhaustion` as "
                    "`defer`, `local_refusal` or `prover_failure`"
                )
            if bound.get("validity") is not False:
                problems.append(f"{where}: a local resource records `validity: false`")
        if bound.get("class") == "local_schedule":
            # A time budget a node plans or waits within. Exceeding it retries or
            # re-plans one local attempt; it is neither a validity rule nor a target.
            if not bound.get("enforcement"):
                problems.append(
                    f"{where}: a local scheduling limit names where it is applied"
                )
            if bound.get("on_exceed") not in ("retry", "replan"):
                problems.append(
                    f"{where}: a local scheduling limit records `on_exceed` as "
                    "`retry` or `replan`"
                )
            if not bound.get("tests"):
                problems.append(f"{where}: a local scheduling limit names its tests")
            if bound.get("validity") is not False:
                problems.append(
                    f"{where}: a local scheduling limit records `validity: false`"
                )
        if bound.get("class") == "engineering_target":
            if bound.get("enforcement"):
                problems.append(f"{where}: an engineering target has no enforcement site")
            if bound.get("validity") is not False:
                problems.append(f"{where}: an engineering target records `validity: false`")
        if defect is not None:
            _check_open_owner(defect, tasks, f"{where}: defect", problems)
        if "inclusive" in bound and type(bound["inclusive"]) is not bool:
            problems.append(f"{where}: `inclusive` is a boolean")
    for declared in contract.get("declared", []):
        groups.add(declared.get("group"))
    for group in GROUPS:
        if group not in groups:
            problems.append(f"group `{group}` has no bound or declared item")
    stages = {
        site["stage"]
        for bound in contract["bounds"]
        for site in bound.get("enforcement", [])
        if "stage" in site
    }
    for stage in STAGES:
        if stage not in stages:
            problems.append(f"stage `{stage}` has no enforcement site")
    classes = {bound.get("class") for bound in contract["bounds"]}
    for name in CLASSES:
        if name not in contract.get("classes", {}):
            problems.append(f"class `{name}` is not described in `classes`")
        if name not in classes:
            problems.append(f"class `{name}` has no bound")


def _check_open_owner(record: dict, tasks: dict, where: str, problems: list) -> None:
    owner = record.get("owner")
    if owner not in tasks:
        problems.append(f"{where}: owner `{owner}` is not a task of the delivery graph")
    elif tasks[owner] == "implemented":
        problems.append(
            f"{where}: task {owner} is recorded implemented; reconcile this open item"
        )
    if not record.get("summary"):
        problems.append(f"{where}: an open item states what is open in `summary`")


def check_fixed(contract: dict, values: dict, problems: list) -> None:
    by_id = {bound.get("id"): bound for bound in contract["bounds"]}
    for identifier, expected in FIXED_BOUNDS.items():
        bound = by_id.get(identifier)
        if bound is None:
            problems.append(f"{identifier}: the fixed bound is missing from the contract")
            continue
        if bound.get("fixed") is not True:
            problems.append(f"{identifier}: the bound is recorded `fixed: true`")
        if bound.get("value") != expected or values.get(identifier, expected) != expected:
            problems.append(
                f"{identifier}: the delivery plan fixes {expected}; "
                f"the contract records {bound.get('value')}"
            )
    for identifier in FRAMED_BOUNDS:
        if not by_id.get(identifier, {}).get("framing"):
            problems.append(
                f"{identifier}: states in `framing` which encoding overhead the length includes"
            )
    pins = pin_tests(contract)
    for identifier in FIXED_VALIDITY:
        bound = by_id.get(identifier, {})
        if (
            bound.get("class") != "consensus"
            or not bound.get("enforcement")
            or not bound.get("tests")
        ):
            problems.append(
                f"{identifier}: a fixed validity bound is classed consensus with its "
                "enforcement sites and tests"
            )
        elif all((test.get("path"), test.get("name")) in pins for test in bound["tests"]):
            problems.append(
                f"{identifier}: a fixed validity bound names a test of its boundary, "
                "not only a pin of its value"
            )
    for identifier in FIXED_NON_VALIDITY:
        bound = by_id.get(identifier, {})
        if bound.get("class") == "consensus":
            problems.append(
                f"{identifier}: a prover time or memory bound is never consensus validity"
            )


def _block_end(source: str, open_index: int) -> int:
    """Index just past the block whose `{` is at `open_index`, or -1 when it is not closed.

    String literals and line comments are skipped, so a brace inside them does not count.
    """
    depth = 0
    index = open_index
    length = len(source)
    while index < length:
        char = source[index]
        if char == '"':
            index += 1
            while index < length and source[index] != '"':
                index += 2 if source[index] == "\\" else 1
        elif char == "/" and source.startswith("//", index):
            newline = source.find("\n", index)
            index = length if newline < 0 else newline
        elif char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
            if depth == 0:
                return index + 1
        index += 1
    return -1


def _module_block(source: str, anchors: list, where: str) -> str:
    """Return the brace-delimited block of a nested module path (each anchor ends with `{`)."""
    for anchor in anchors[:-1]:
        position = source.find(anchor)
        if position < 0:
            raise ContractError(f"{where}: module `{anchor}` is not in the file")
        source = source[position + len(anchor) :]
    anchor = anchors[-1]
    start = source.find(anchor)
    if start < 0:
        raise ContractError(f"{where}: module `{anchor}` is not in the file")
    end = _block_end(source, start + len(anchor) - 1)
    if end < 0:
        raise ContractError(f"{where}: module `{anchor}` is not closed")
    return source[start:end]


def _without_inline_tests(source: str) -> str:
    """Remove every inline `#[cfg(test)] mod name { ... }` block."""
    kept = []
    position = 0
    for match in INLINE_TEST_MODULE.finditer(source):
        if match.start() < position:
            continue
        end = _block_end(source, match.end() - 1)
        kept.append(source[position : match.start()])
        position = len(source) if end < 0 else end
    kept.append(source[position:])
    return "".join(kept)


def scope_symbols(source: str, scope: dict, where: str) -> list:
    """Return the limit symbols a completeness scope defines, in source order.

    A `const` scope finds every constant whose name marks a limit (`LIMIT_NAME`, or the
    scope's own `matching` pattern, which must come with a `why`). A `default_fn` scope
    finds every zero-argument default of a committed-parameter module. Inline test
    modules are not scanned.
    """
    key = (len(source), hash(source), json.dumps(scope, sort_keys=True))
    if key not in _SCOPE_SYMBOLS:
        _SCOPE_SYMBOLS[key] = _scope_symbols(source, scope, where)
    return list(_SCOPE_SYMBOLS[key])


_SCOPE_SYMBOLS: dict = {}


def _scope_symbols(source: str, scope: dict, where: str) -> list:
    source = _without_inline_tests(source)
    if scope.get("module"):
        source = _module_block(source, scope["module"], where)
    kind = scope.get("kind")
    if kind == "const":
        pattern = LIMIT_NAME
        if "matching" in scope:
            if not scope.get("why"):
                raise ContractError(
                    f"{where}: a scope with its own `matching` pattern states `why`"
                )
            pattern = re.compile(scope["matching"])
        names = [name for name in CONST_DEFINITION.findall(source) if pattern.search(name)]
    elif kind == "default_fn":
        names = DEFAULT_FN_DEFINITION.findall(source)
    else:
        raise ContractError(f"{where}: unknown scope kind {kind!r}")
    return list(dict.fromkeys(names))


def _owner_anchors(owner: dict) -> list:
    anchors = owner.get("anchor")
    if anchors is None:
        return []
    return [anchors] if isinstance(anchors, str) else list(anchors)


def _scope_key(scope: dict) -> tuple:
    return (scope.get("path"), tuple(scope.get("module") or ()))


def expand_scopes(root: Path, section: dict, problems: list) -> list:
    """Return every concrete scope: each file scope, and one per file of each `tree`.

    A `tree` scope scans every Rust source below a directory except test, bench,
    example and fixture files and the paths it lists under `skip` with a reason, so a
    new file of the directory is scanned too. A file that also has its own scope
    without a module keeps that scope.
    """
    scopes = []
    explicit = {
        scope.get("path") for scope in section["scopes"] if "path" in scope and not scope.get("module")
    }
    trees = set()
    for scope in section["scopes"]:
        if "tree" not in scope:
            scopes.append(scope)
            continue
        where = f"completeness tree {scope['tree']}"
        directory = root / scope["tree"]
        if scope.get("kind") != "const" or scope.get("module"):
            problems.append(f"{where}: a tree scope scans constants and names no module")
            continue
        if not directory.is_dir():
            problems.append(f"{where}: the directory does not exist")
            continue
        if scope["tree"] in trees:
            problems.append(f"{where}: the tree is listed twice")
            continue
        trees.add(scope["tree"])
        skipped = []
        for skip in scope.get("skip", []):
            if not skip.get("why"):
                problems.append(f"{where}: a skipped path states `why`")
            if not (root / skip.get("path", "")).exists() or not str(
                skip.get("path", "")
            ).startswith(scope["tree"] + "/"):
                problems.append(f"{where}: skipped path {skip.get('path')!r} is not below the tree")
            skipped.append(skip.get("path", ""))
        extra = {key: scope[key] for key in ("matching", "why") if key in scope}
        for file in sorted(directory.rglob("*.rs")):
            relative = file.relative_to(root).as_posix()
            inner = relative[len(scope["tree"]) + 1 :]
            if TEST_FILE.search(inner) or relative in explicit:
                continue
            if any(relative == path or relative.startswith(path + "/") for path in skipped):
                continue
            scopes.append({"path": relative, "kind": "const", "tree": scope["tree"], **extra})
    return scopes


def _exclusion_entries(section: dict, problems: list) -> list:
    """Flatten the exclusion groups into `(holder, path, module, symbol)` entries.

    A group names one file (`path`, optional `module`, `symbols`) or, under one shared
    reason, several (`files`, each with `path` and `symbols`).
    """
    entries = []
    for group in section.get("excluded", []):
        holders = group["files"] if "files" in group else [group]
        where = f"completeness exclusion in {holders[0].get('path') if holders else None}"
        if not group.get("reason"):
            problems.append(f"{where}: an exclusion states its reason")
        if "files" in group and ("path" in group or "symbols" in group):
            problems.append(f"{where}: a group names `path` and `symbols`, or `files`")
        for holder in holders:
            if not holder.get("path") or not holder.get("symbols"):
                problems.append(f"{where}: an exclusion names its file and symbols")
            for symbol in holder.get("symbols", []):
                entries.append((holder, holder.get("path"), tuple(holder.get("module") or ()), symbol))
    return entries


def check_completeness(
    root: Path,
    contract: dict,
    problems: list,
    refresh: bool = False,
    notes: list | None = None,
    unlisted: list | None = None,
) -> None:
    """Every limit an owner file defines is a listed bound or excluded with a reason.

    With `refresh`, an exclusion that matches no scanned limit is removed and reported
    in `notes` instead of failing. `unlisted` receives `(scope, symbol)` of every limit
    that is neither listed nor excluded, for `--propose`.
    """
    section = contract.get("completeness")
    if not isinstance(section, dict) or not section.get("scopes"):
        problems.append("completeness: the contract lists the owner files it scans")
        return
    owners = [bound.get("owner", {}) for bound in contract["bounds"]]

    def inside(scope: dict, owner: dict) -> bool:
        module = list(scope.get("module") or ())
        return owner.get("path") == scope["path"] and _owner_anchors(owner)[: len(module)] == module

    def listed(scope: dict, symbol: str) -> bool:
        kind = "fn" if scope["kind"] == "default_fn" else "const"
        return any(
            owner.get("symbol") == symbol and owner.get("kind") == kind and inside(scope, owner)
            for owner in owners
        )

    entries = _exclusion_entries(section, problems)
    excluded: dict = {}
    for holder, path, module, symbol in entries:
        key = ((path, module), symbol)
        if key in excluded:
            problems.append(f"completeness exclusion in {path}: `{symbol}` is excluded twice")
        excluded[key] = False
    scopes = expand_scopes(root, section, problems)
    seen = set()
    for scope in scopes:
        where = f"completeness {scope.get('path')}"
        if scope.get("module"):
            where += f" `{' '.join(scope['module'])}`"
        try:
            _require(scope, ("path", "kind"), where)
            if (_scope_key(scope), scope["kind"]) in seen:
                raise ContractError(f"{where}: the scope is listed twice")
            seen.add((_scope_key(scope), scope["kind"]))
            symbols = scope_symbols(_read(root, scope["path"], where), scope, where)
            for symbol in symbols:
                key = (_scope_key(scope), symbol)
                if listed(scope, symbol):
                    if key in excluded:
                        problems.append(f"{where}: `{symbol}` is both listed and excluded")
                        excluded[key] = True
                    continue
                if key in excluded:
                    excluded[key] = True
                    continue
                problems.append(
                    f"{where}: `{symbol}` is neither a listed bound nor excluded with a reason"
                )
                if unlisted is not None:
                    unlisted.append((scope, symbol))
        except ContractError as error:
            problems.append(str(error))
    for holder, path, module, symbol in entries:
        if excluded.get(((path, module), symbol)):
            continue
        text = (
            f"completeness: the exclusion of `{symbol}` in {path}"
            + (f" `{' '.join(module)}`" if module else "")
            + " matches no scanned limit"
        )
        if refresh:
            holder["symbols"] = [name for name in holder["symbols"] if name != symbol]
            if notes is not None:
                notes.append(text + "; removed")
        else:
            problems.append(text)
    if refresh:
        for group in section.get("excluded", []):
            if "files" in group:
                group["files"] = [holder for holder in group["files"] if holder.get("symbols")]
        section["excluded"] = [
            group
            for group in section.get("excluded", [])
            if (group.get("files") if "files" in group else group.get("symbols"))
        ]
    # Listing a bound extends the scan: a constant owner lies inside a scanned scope.
    for bound in contract["bounds"]:
        owner = bound.get("owner", {})
        if owner.get("kind") != "const":
            continue
        if not any(scope.get("kind") == "const" and inside(scope, owner) for scope in scopes):
            problems.append(
                f"{bound.get('id')}: its owner in {owner.get('path')} is outside every "
                "completeness scope"
            )


def _group_end(source: str, open_index: int, opening: str, closing: str) -> int:
    """Index just past the group opened at `open_index`, or -1; string literals are skipped."""
    depth = 0
    index = open_index
    length = len(source)
    while index < length:
        char = source[index]
        if char == '"':
            index += 1
            while index < length and source[index] != '"':
                index += 2 if source[index] == "\\" else 1
        elif char == opening:
            depth += 1
        elif char == closing:
            depth -= 1
            if depth == 0:
                return index + 1
        index += 1
    return -1


def _function_body(source: str, function: str, where: str, path: str) -> str:
    """The body of `fn function(..) {..}` without line comments."""
    start = source.find(f"fn {function}(")
    if start < 0:
        raise ContractError(f"{where}: `{function}` is not in {path}")
    # The body opens at the first `{` after the parameter list.
    parameters = _group_end(source, source.index("(", start), "(", ")")
    body = source.find("{", parameters) if parameters > 0 else -1
    end = _block_end(source, body) if body >= 0 else -1
    if end < 0:
        raise ContractError(f"{where}: `{function}` is not closed")
    return LINE_COMMENT.sub("", source[body:end])


def _policy_push_fields(body: str, where: str) -> list:
    """Field names of every `policy.push(name, ..)` call, in source order.

    A name is a string literal, or the first element of the tuples of the literal
    table a `for (name, ..) in [..] { .. }` loop around the call iterates. Any other
    call fails: a field the checker cannot name is a field it cannot classify.
    """
    fields = []
    for call in re.finditer(r"policy\s*\.\s*push\s*\(\s*", body):
        rest = body[call.end() :]
        literal = re.match(r"\"([^\"]+)\"", rest)
        if literal:
            fields.append(literal.group(1))
            continue
        unreadable = ContractError(
            f"{where}: a `policy.push(` call binds a field whose name the checker cannot "
            f"read (`{' '.join(rest[:48].split())}`)"
        )
        variable = re.match(r"([a-z_][a-z0-9_]*)\s*,", rest)
        if variable is None:
            raise unreadable
        loops = list(
            re.finditer(
                rf"\bfor\s*\(\s*{variable.group(1)}\s*,[^)]*\)\s*in\s*\[", body[: call.start()]
            )
        )
        if not loops:
            raise unreadable
        table_open = loops[-1].end() - 1
        table_end = _group_end(body, table_open, "[", "]")
        block = re.match(r"\s*\{", body[table_end:]) if table_end > 0 else None
        if block is None or _block_end(body, table_end + block.end() - 1) < call.start():
            raise unreadable
        table = body[table_open + 1 : table_end - 1]
        index = 0
        names = []
        while True:
            opening = table.find("(", index)
            if opening < 0:
                break
            closing = _group_end(table, opening, "(", ")")
            entry = re.match(r"\(\s*\"([^\"]+)\"\s*,", table[opening:closing]) if closing > 0 else None
            if entry is None:
                raise unreadable
            names.append(entry.group(1))
            index = closing
        if not names:
            raise unreadable
        fields.extend(names)
    return fields


def _zk_policy_put_fields(body: str, where: str) -> list:
    """Field names of every `zk_policy_put_*(&mut h, "name", ..)` call, in source order.

    The domain separator (`zk_policy_put_bytes(&mut h, b"..")`) is not a field; any
    other call without a literal name fails.
    """
    fields = []
    for call in re.finditer(r"\bzk_policy_put_(\w+)\s*\(\s*&mut\s+h\s*,\s*", body):
        rest = body[call.end() :]
        literal = re.match(r"\"([^\"]+)\"", rest)
        if literal:
            fields.append(literal.group(1))
        elif not (call.group(1) == "bytes" and re.match(r"b\"[^\"]*\"\s*\)", rest)):
            raise ContractError(
                f"{where}: a `zk_policy_put_{call.group(1)}(` call binds a field whose "
                f"name the checker cannot read (`{' '.join(rest[:48].split())}`)"
            )
    return fields


def _split_top_level(text: str) -> list:
    """Split `text` at commas outside every `()`, `[]`, `<>` and `{}` group."""
    parts = []
    depth = 0
    start = 0
    for index, char in enumerate(text):
        if char in "([<{":
            depth += 1
        elif char in ")]>}":
            depth -= 1
        elif char == "," and depth == 0:
            parts.append(text[start:index])
            start = index + 1
    parts.append(text[start:])
    return parts


def struct_field_tree(source: str, name: str, where: str, seen: tuple = ()) -> list:
    """Dotted leaf field paths of `struct name { .. }`, in declaration order.

    A field whose type is another struct of the same file (also inside `Vec<..>` or
    `Option<..>`) contributes that struct's fields below its own name.
    """
    definition = re.search(rf"\bstruct\s+{re.escape(name)}\s*\{{", source)
    if definition is None:
        raise ContractError(f"{where}: struct `{name}` is not defined in the file")
    end = _block_end(source, definition.end() - 1)
    if end < 0:
        raise ContractError(f"{where}: struct `{name}` is not closed")
    body = LINE_COMMENT.sub("", source[definition.end() : end - 1])
    body = re.sub(r"#\[[^\]]*\]", "", body)
    fields = []
    for part in _split_top_level(body):
        field = re.fullmatch(
            r"\s*(?:pub(?:\([^)]*\))?\s+)?([a-z_][a-z0-9_]*)\s*:\s*(.+?)\s*", part, re.DOTALL
        )
        if field is None:
            if part.strip():
                raise ContractError(f"{where}: cannot read a field of `{name}`: `{part.strip()[:40]}`")
            continue
        inner = field.group(2)
        while True:
            wrapped = re.fullmatch(r"(?:Vec|Option)\s*<\s*(.+?)\s*>", inner, re.DOTALL)
            if wrapped is None:
                break
            inner = wrapped.group(1)
        nested = (
            re.fullmatch(r"[A-Z]\w*", inner)
            and inner not in seen
            and inner != name
            and re.search(rf"\bstruct\s+{inner}\s*\{{", source)
        )
        if nested:
            fields.extend(
                f"{field.group(1)}.{leaf}"
                for leaf in struct_field_tree(source, inner, where, (*seen, name))
            )
        else:
            fields.append(field.group(1))
    return fields


_CATALOG_FIELDS: dict = {}


def catalog_fields(source: str, catalog: dict, where: str) -> list:
    """Return the field names an authoritative typed catalog binds, in source order.

    `policy_push` and `zk_policy_put` read the calls of the named function;
    `struct_tree` reads the fields of the named struct and of the structs it nests,
    and, when a `function` is named, checks that the function builds that struct.
    """
    reader = {key: catalog.get(key) for key in ("field_form", "function", "struct", "path")}
    key = (len(source), hash(source), json.dumps(reader, sort_keys=True), where)
    if key not in _CATALOG_FIELDS:
        _CATALOG_FIELDS[key] = _catalog_fields(source, catalog, where)
    return list(_CATALOG_FIELDS[key])


def _catalog_fields(source: str, catalog: dict, where: str) -> list:
    form = catalog.get("field_form")
    if form not in CATALOG_FORMS:
        raise ContractError(f"{where}: unknown field form {form!r}")
    if form == "struct_tree":
        _require(catalog, ("struct",), where)
        if "function" in catalog:
            body = _function_body(source, catalog["function"], where, catalog["path"])
            if not re.search(rf"\b{re.escape(catalog['struct'])}\s*\{{", body):
                raise ContractError(
                    f"{where}: `{catalog['function']}` no longer builds `{catalog['struct']}`"
                )
        return struct_field_tree(source, catalog["struct"], where)
    _require(catalog, ("function",), where)
    body = _function_body(source, catalog["function"], where, catalog["path"])
    if form == "policy_push":
        return _policy_push_fields(body, where)
    return _zk_policy_put_fields(body, where)


def check_catalogs(
    root: Path, contract: dict, tasks: dict, problems: list, unlisted: list | None = None
) -> None:
    """Every field of an authoritative typed catalog is a listed bound or excluded.

    A field recorded under `digests` is the digest of another catalog of this contract,
    which must exist: the fields behind the digest are classified there. `unlisted`
    receives `(catalog, field)` of every field that is not recorded, for `--propose`.
    """
    by_id = {bound.get("id"): bound for bound in contract["bounds"]}
    catalog_ids = [catalog.get("id") for catalog in contract.get("catalogs", [])]
    claimed: dict = {}
    for catalog in contract.get("catalogs", []):
        where = f"catalog {catalog.get('id', '<without id>')}"
        try:
            _require(catalog, ("id", "path", "field_form", "what", "fields", "excluded"), where)
            if catalog_ids.count(catalog["id"]) > 1:
                raise ContractError(f"{where}: duplicate catalog id")
            name = catalog.get("function") or catalog.get("struct")
            observed = catalog_fields(_read(root, catalog["path"], where), catalog, where)
            if len(set(observed)) != len(observed):
                raise ContractError(f"{where}: the source binds a field twice")
            excluded = []
            for group in catalog["excluded"]:
                if not group.get("reason"):
                    raise ContractError(f"{where}: an exclusion states its reason")
                excluded.extend(group.get("fields", []))
            digests = catalog.get("digests", {})
            for field, target in digests.items():
                if target not in catalog_ids or target == catalog["id"]:
                    problems.append(
                        f"{where}: `{field}` is recorded as the digest of `{target}`, "
                        "which is not another catalog of this contract"
                    )
            recorded = list(catalog["fields"]) + excluded + list(digests)
            for field in sorted(set(recorded)):
                if recorded.count(field) > 1:
                    problems.append(f"{where}: `{field}` is recorded twice")
            for field in observed:
                if field not in recorded:
                    problems.append(
                        f"{where}: field `{field}` of `{name}` is neither "
                        "listed as a bound nor excluded with a reason"
                    )
                    if unlisted is not None:
                        unlisted.append((catalog, field))
            for field in recorded:
                if field not in observed:
                    problems.append(f"{where}: `{field}` is not a field of `{name}`")
            for field, identifier in catalog["fields"].items():
                bound = by_id.get(identifier)
                if bound is None:
                    problems.append(f"{where}: `{field}` names unknown bound `{identifier}`")
                    continue
                if identifier in claimed:
                    problems.append(f"{identifier}: two catalog fields name this bound")
                claimed[identifier] = catalog["id"]
                if bound.get("catalog") != {"id": catalog["id"], "field": field}:
                    problems.append(
                        f"{identifier}: records `catalog` as {catalog['id']} / {field}"
                    )
                if catalog.get("source") and bound.get("source") != catalog["source"]:
                    problems.append(
                        f"{identifier}: a field of {catalog['id']} has source "
                        f"`{catalog['source']}`"
                    )
                if catalog.get("source") == "node_local" and bound.get("class") != "consensus":
                    problems.append(
                        f"{identifier}: {catalog['id']} binds validity-affecting node "
                        "configuration; the bound is classed consensus with its defect"
                    )
        except ContractError as error:
            problems.append(str(error))
    for required in REQUIRED_CATALOGS:
        if required not in catalog_ids:
            problems.append(f"catalogs: `{required}` is checked against the source")
    for bound in contract["bounds"]:
        if "catalog" in bound and claimed.get(bound.get("id")) != bound["catalog"].get("id"):
            problems.append(
                f"{bound.get('id')}: its `catalog` entry is not a field of that catalog"
            )


def check_relations(contract: dict, values: dict, tasks: dict, problems: list) -> None:
    profiles = {"today": {}}
    for name, profile in contract.get("profiles", {}).items():
        overrides = profile.get("overrides", {})
        for identifier in overrides:
            if identifier not in values:
                problems.append(f"profile {name}: unknown bound `{identifier}`")
        profiles[name] = overrides
        owner = profile.get("owner")
        if owner not in tasks:
            problems.append(f"profile {name}: owner `{owner}` is not a delivery-graph task")
    seen = set()
    for relation in contract.get("relations", []):
        where = f"relation {relation.get('id', '<without id>')}"
        try:
            _require(relation, ("id", "expr", "why", "expect"), where)
            if relation["id"] in seen:
                raise ContractError(f"{where}: duplicate relation id")
            seen.add(relation["id"])
            if "today" not in relation["expect"]:
                raise ContractError(f"{where}: `expect` records `today`")
            for profile, expected in relation["expect"].items():
                if profile not in profiles:
                    raise ContractError(f"{where}: unknown profile `{profile}`")
                if expected not in EXPECTATIONS:
                    raise ContractError(f"{where}: unknown expectation {expected!r}")
                result = evaluate_relation(
                    relation["expr"], {**values, **profiles[profile]}, where
                )
                if type(result) is not bool:
                    raise ContractError(f"{where}: the expression is not a comparison")
                actual = "holds" if result else "violated"
                if actual != expected:
                    raise ContractError(
                        f"{where}: {actual} under `{profile}`; the contract records {expected}"
                    )
            # A violated relation names the task that must make it hold. A relation
            # that holds may still carry an open item, for example an unmeasured margin.
            if "violated" in relation["expect"].values() or "open" in relation:
                _check_open_owner(relation.get("open", {}), tasks, where, problems)
        except ContractError as error:
            problems.append(str(error))


def check_declared(contract: dict, tasks: dict, problems: list) -> None:
    for declared in contract.get("declared", []):
        where = f"declared {declared.get('id', '<without id>')}"
        try:
            _require(declared, ("id", "group", "what", "status", "requires"), where)
            if declared["group"] not in GROUPS:
                raise ContractError(f"{where}: unknown group {declared['group']!r}")
            if declared["status"] != "not_implemented":
                raise ContractError(
                    f"{where}: an implemented item is a bound with an owner, not `declared`"
                )
            if not declared["requires"]:
                raise ContractError(f"{where}: a declared item lists what it must satisfy")
            _check_open_owner(declared.get("open", {}), tasks, where, problems)
        except ContractError as error:
            problems.append(str(error))


def check_budgets(
    contract: dict, values: dict, tasks: dict, problems: list, refresh: bool = False
) -> None:
    section = contract.get("ram_lfe")
    if not isinstance(section, dict) or not section.get("budgets"):
        problems.append("ram_lfe: the contract derives the RAM-LFE budgets")
        return
    # Closed budgets join `values`, so a relation can name one.
    known = values
    open_inputs = {}
    for item in section.get("open_inputs", []):
        where = f"ram_lfe input {item.get('id', '<without id>')}"
        if item.get("value") is not None:
            problems.append(f"{where}: a known input is a bound, not an open input")
        _check_open_owner(item, tasks, where, problems)
        open_inputs[item.get("id")] = item
    for budget in section["budgets"]:
        where = f"ram_lfe budget {budget.get('id', '<without id>')}"
        try:
            _require(budget, ("id", "what", "unit", "class", "formula", "inputs", "value"), where)
            if budget["class"] not in CLASSES:
                raise ContractError(f"{where}: unknown class {budget['class']!r}")
            used = set(ID.findall(budget["formula"]))
            if used != set(budget["inputs"]):
                raise ContractError(
                    f"{where}: `inputs` must be exactly the ids the formula uses: "
                    f"{sorted(used)}"
                )
            missing = [name for name in used if name not in known and name not in open_inputs]
            if missing:
                raise ContractError(f"{where}: unknown inputs {sorted(missing)}")
            still_open = sorted(name for name in used if name in open_inputs)
            # The formula must be well formed even while inputs are open.
            probe = {**known, **{name: 1 for name in open_inputs}}
            if type(evaluate_relation(budget["formula"], probe, where)) is not int:
                raise ContractError(f"{where}: the formula is not an integer expression")
            if still_open:
                if budget["value"] is not None:
                    raise ContractError(
                        f"{where}: {still_open} are open, so the value must be null; "
                        "no number is invented"
                    )
            else:
                computed = evaluate_relation(budget["formula"], known, where)
                if refresh:
                    budget["value"] = computed
                if budget["value"] != computed:
                    raise ContractError(
                        f"{where}: every input is known and the formula gives "
                        f"{computed}; the contract records {budget['value']}"
                    )
                known[budget["id"]] = computed
        except ContractError as error:
            problems.append(str(error))
    # The derivation covers the full workload: no declared input or shape is left unused.
    used_anywhere = set()
    for budget in section["budgets"]:
        used_anywhere.update(ID.findall(budget.get("formula", "")))
    for identifier in open_inputs:
        if identifier not in used_anywhere:
            problems.append(
                f"ram_lfe input {identifier}: an open input is used by a budget formula"
            )
    for bound in contract["bounds"]:
        identifier = bound.get("id", "")
        if identifier.startswith("ram_lfe.shape_") and identifier not in used_anywhere:
            problems.append(
                f"{identifier}: a RAM-LFE shape bound is used by a budget formula"
            )
    for component in WORKLOAD_COMPONENTS:
        if not any(budget.get("component") == component for budget in section["budgets"]):
            problems.append(
                f"ram_lfe: the workload component `{component}` has a budget"
            )
    for budget in section["budgets"]:
        if budget.get("component") not in WORKLOAD_COMPONENTS:
            problems.append(
                f"ram_lfe budget {budget.get('id')}: names its workload `component`"
            )


def check_derived(
    root: Path,
    contract: dict,
    values: dict,
    problems: list,
    refresh: bool = False,
    notes: list | None = None,
) -> None:
    """Recompute every derived bound from the verified bounds; add it to `values`."""
    for derived in contract.get("derived", []):
        where = f"derived {derived.get('id', '<without id>')}"
        try:
            keys = ("id", "group", "what", "unit", "class", "formula", "value")
            _require(derived, keys + ("enforcement", "tests"), where)
            if derived["group"] not in GROUPS:
                raise ContractError(f"{where}: unknown group {derived['group']!r}")
            if derived["class"] not in CLASSES:
                raise ContractError(f"{where}: unknown class {derived['class']!r}")
            if derived["id"] in values:
                raise ContractError(f"{where}: the id is already a bound")
            computed = evaluate_relation(derived["formula"], values, where)
            if type(computed) is not int:
                raise ContractError(f"{where}: the formula is not an integer expression")
            if refresh:
                derived["value"] = computed
            if computed != derived["value"]:
                raise ContractError(
                    f"{where}: the formula gives {computed}; the contract records "
                    f"{derived['value']}"
                )
            if not derived["enforcement"] or not derived["tests"]:
                raise ContractError(f"{where}: names its enforcement sites and tests")
            values[derived["id"]] = computed
        except ContractError as error:
            problems.append(str(error))
        _check_sites(root, derived, where, problems, None, refresh, notes)


BEHAVIORS = (
    "missing_local_resource_defers",
    "malformed_carried_evidence_rejects_deterministically",
)
# Stages a behavior's tests must reach: each test names the stage it exercises.
BEHAVIOR_STAGES = {
    "missing_local_resource_defers": ("queue", "proposer", "follower"),
    "malformed_carried_evidence_rejects_deterministically": ("sdk", "queue", "follower"),
}


def check_stage_tests(
    root: Path,
    contract: dict,
    tasks: dict,
    problems: list,
    refresh: bool = False,
    notes: list | None = None,
) -> None:
    """Every stage names its exact and one-over tests, or the task that owns the gap."""
    covered = set()
    for entry in contract.get("stage_tests", []):
        where = f"stage_tests {entry.get('stage')}"
        if entry.get("stage") not in STAGES:
            problems.append(f"{where}: unknown stage")
            continue
        covered.add(entry["stage"])
        _check_sites(root, entry, where, problems, None, refresh, notes)
        if not entry.get("claim"):
            problems.append(f"{where}: states its claim")
        if "open" in entry:
            _check_open_owner(entry["open"], tasks, where, problems)
        elif not entry.get("tests"):
            problems.append(f"{where}: names a test or the task that owns the gap")
    for stage in STAGES:
        if stage not in covered:
            problems.append(f"stage_tests: stage `{stage}` is missing")
    behaviors = {entry.get("id"): entry for entry in contract.get("behaviors", [])}
    for identifier in BEHAVIORS:
        entry = behaviors.get(identifier)
        if entry is None or not entry.get("tests"):
            problems.append(f"behaviors: `{identifier}` names the tests that show it")
            continue
        _check_sites(root, entry, f"behaviors {identifier}", problems, None, refresh, notes)
        if "open" in entry:
            _check_open_owner(entry["open"], tasks, f"behaviors {identifier}", problems)
        reached = {test.get("stage") for test in entry["tests"]}
        for stage in BEHAVIOR_STAGES[identifier]:
            if stage not in reached:
                problems.append(
                    f"behaviors: `{identifier}` names a test at the `{stage}` stage"
                )


# Keys whose text is source code, a path or a name: not prose that cites bounds.
_UNCITED_KEYS = frozenset(
    (
        "contains",
        "expr",
        "formula",
        "path",
        "symbol",
        "anchor",
        "committed_as",
        "macro",
        "macro_path",
        "name",
        "field",
        "function",
        "struct",
        "matching",
    )
)
_CITED_ID = re.compile(
    r"(?<![/:\w.])(?:" + "|".join(GROUPS) + r")\.[a-z][a-z0-9_]*(?![\w(])"
)


def check_references(contract: dict, problems: list) -> None:
    """Every bound id a reason, note or description cites is an id of this contract.

    A reason such as "bounded by guest_memory.heap_max_bytes" is checked text: a
    renamed or removed bound must not leave a reason that cites nothing. A group
    followed by `.*` (`rs16.*`) cites the group and is not an id.
    """
    known = {bound.get("id") for bound in contract["bounds"]}
    for key in ("derived", "declared"):
        known.update(item.get("id") for item in contract.get(key, []))
    lfe = contract.get("ram_lfe") or {}
    known.update(item.get("id") for item in lfe.get("budgets", []))
    known.update(item.get("id") for item in lfe.get("open_inputs", []))
    unknown: dict = {}

    def walk(value, trail: str) -> None:
        if isinstance(value, dict):
            for key, item in value.items():
                if key not in _UNCITED_KEYS:
                    walk(item, f"{trail}/{key}")
        elif isinstance(value, list):
            for item in value:
                walk(item, trail)
        elif isinstance(value, str):
            for match in _CITED_ID.finditer(value):
                cited = match.group(0)
                if cited not in known and not cited.endswith(".rs"):
                    unknown.setdefault(cited, trail)

    walk(contract, "")
    for cited, trail in sorted(unknown.items()):
        problems.append(f"{trail.lstrip('/')}: the text cites `{cited}`, which is not a bound")


def load_tasks(graph_path: Path) -> dict:
    graph = json.loads(graph_path.read_text(encoding="utf-8"))
    return {task["id"]: task.get("status", "planned") for task in graph["tasks"]}


def check(
    root: Path = ROOT,
    contract: dict | None = None,
    tasks: dict | None = None,
    refresh: bool = False,
    notes: list | None = None,
    unlisted: dict | None = None,
) -> list:
    """Return every problem found; an empty list means the contract matches the source.

    With `refresh`, `contract` is updated in place before the remaining checks run: the
    expression and value the source defines for every bound, the occurrence count of
    every site that is still present, and the exclusions that still match a limit.
    `notes` receives one line for each count changed and each exclusion removed.
    `unlisted` receives the limits (`limits`) and catalog fields (`fields`) that are
    neither listed nor excluded, for `--propose`.
    """
    problems: list = []
    # A file may have changed since the last run: the verdicts are per run.
    _REACHABLE.clear()
    _REFERENCES.clear()
    if contract is None:
        contract = json.loads((root / CONTRACT.relative_to(ROOT)).read_text(encoding="utf-8"))
    if tasks is None:
        tasks = load_tasks(root / GRAPH.relative_to(ROOT))
    if contract.get("schema") != SCHEMA:
        return [f"schema is {contract.get('schema')!r}, expected {SCHEMA!r}"]
    if contract.get("task") not in tasks:
        problems.append("`task` names the delivery-graph task that owns the contract")
    if not isinstance(contract.get("bounds"), list) or not contract["bounds"]:
        return problems + ["the contract lists its bounds"]
    limits = None if unlisted is None else unlisted.setdefault("limits", [])
    fields = None if unlisted is None else unlisted.setdefault("fields", [])
    values = verify_bounds(root, contract, problems, refresh)
    for bound in contract["bounds"]:
        _check_sites(
            root,
            bound,
            bound.get("id", "<bound without id>"),
            problems,
            bound.get("owner"),
            refresh,
            notes,
        )
    for declared in contract.get("declared", []):
        _check_sites(
            root, declared, f"declared {declared.get('id')}", problems, None, refresh, notes
        )
    for relation in contract.get("relations", []):
        _check_sites(
            root, relation, f"relation {relation.get('id')}", problems, None, refresh, notes
        )
    check_pin_tests(root, contract, problems)
    check_classification(contract, tasks, problems)
    check_fixed(contract, values, problems)
    check_completeness(root, contract, problems, refresh, notes, limits)
    check_catalogs(root, contract, tasks, problems, fields)
    check_derived(root, contract, values, problems, refresh, notes)
    check_budgets(contract, values, tasks, problems, refresh)
    check_relations(contract, values, tasks, problems)
    check_declared(contract, tasks, problems)
    check_stage_tests(root, contract, tasks, problems, refresh, notes)
    check_references(contract, problems)
    return problems


def render(contract: dict) -> str:
    """The canonical text of the contract file."""
    return json.dumps(contract, indent=2, ensure_ascii=True) + "\n"


# What marks a line as comparing or applying a limit, for `--propose`.
APPLICATION = re.compile(
    r"[<>]=?|[=!]=|\.\s*(?:min|max|clamp|contains|saturating_sub|checked_sub)\s*\("
    r"|\b(?:try_reserve\w*|with_capacity|to_bytes_bounded|take)\s*\("
)


def _crate_files(root: Path, relative: str, search_in: list) -> list:
    """Rust sources of the crate that owns `relative` and of every `search_in` directory."""
    file = root / relative
    crate = _crate_of(root, file)
    directories = [crate if crate is not None else file.parent]
    directories.extend(root / directory for directory in search_in)
    files = []
    for directory in directories:
        for path in sorted(directory.rglob("*.rs")):
            if "target" not in path.relative_to(root).parts and path not in files:
                files.append(path)
    return files


def propose_entry(
    root: Path,
    scope: dict,
    symbol: str,
    values: dict,
    search_in: list,
    words: tuple = (),
    limit: int = 6,
) -> dict:
    """A bound skeleton for one unlisted limit: its owner, value, candidate sites and tests.

    Sites are lines that mention the symbol (or one of `words`, for example the
    configuration field a default is read through) next to a comparison or an applying
    call; tests are active tests whose body mentions it. Both are searched in the
    owner's crate and in `search_in`. They are suggestions to review, not evidence.
    """
    where = f"{scope['path']}:{symbol}"
    kind = "fn" if scope.get("kind") == "default_fn" else "const"
    owner: dict = {"path": scope["path"], "symbol": symbol, "kind": kind}
    if scope.get("module"):
        owner["anchor"] = list(scope["module"])
    entry: dict = {
        "id": f"<group>.{symbol.lower()}",
        "group": "<group>",
        "what": "<what the bound limits>",
        "unit": "<bytes|count|...>",
        "value": None,
        "source": "committed" if kind == "fn" else "constant",
        "class": "<consensus|local_defer|local_schedule|engineering_target>",
        "owner": owner,
    }
    try:
        owner["expr"] = extract_expression(_read(root, scope["path"], where), owner, where)
        # Resolve names through the bounds already listed for the same file.
        entry["value"] = evaluate_rust(owner["expr"], {}, values, where)
    except ContractError as error:
        entry["value_note"] = f"not evaluated: {error}; add `refs` for the constants it names"
    names = "|".join(re.escape(name) for name in (symbol, *words))
    word = re.compile(rf"\b(?:{names})\b")
    definition = re.compile(rf"\b(?:const|fn)\s+{re.escape(symbol)}\b")
    sites: list = []
    tests: list = []
    for path in _crate_files(root, scope["path"], search_in):
        text = path.read_text(encoding="utf-8")
        if not word.search(text):
            continue
        relative = path.relative_to(root).as_posix()
        production = _without_inline_tests(text)
        if not TEST_FILE.search(relative):
            for line in production.splitlines():
                code = LINE_COMMENT.sub("", line).strip()
                if (
                    word.search(code)
                    and not definition.search(code)
                    and APPLICATION.search(code.replace("->", "").replace("=>", ""))
                    and len(sites) < limit
                ):
                    site = {"stage": "<stage>", "path": relative, "contains": code}
                    count = _normalized(text).count(normalize(code))
                    if count > 1:
                        site["count"] = count
                    if site not in sites:
                        sites.append(site)
        for match in re.finditer(r"\bfn\s+(\w+)\s*\(", text):
            head = LINE_COMMENT.sub("", text[max(0, match.start() - 400) : match.start()])
            head = head[max(head.rfind("}"), head.rfind(";"), head.rfind("{")) + 1 :]
            if not TEST_ATTRIBUTE.search(head) or IGNORE_ATTRIBUTE.search(head):
                continue
            opening = text.find("{", match.end())
            end = _block_end(text, opening) if opening >= 0 else -1
            if end > 0 and word.search(text[opening:end]) and len(tests) < limit:
                tests.append({"path": relative, "name": match.group(1)})
    entry["candidate_sites"] = sites
    entry["candidate_tests"] = tests
    return entry


def propose(
    root: Path,
    contract: dict,
    tasks: dict,
    search_in: list,
    only: str | None = None,
    module: tuple = (),
    words: tuple = (),
) -> list:
    """Lines of suggestions for everything the check reports as unclassified or stale.

    Nothing is written. `only` (`PATH:SYMBOL`) asks for the skeleton of one constant
    whether or not it is listed; `module` names the nested modules that hold it and
    `words` further identifiers to search sites and tests for.
    """
    working = json.loads(json.dumps(contract))
    values: dict = {}
    verify_bounds(root, working, [], False)
    for bound in working["bounds"]:
        if "value" in bound:
            values[bound["id"]] = bound["value"]
    lines: list = []
    if only is not None:
        path, _, symbol = only.rpartition(":")
        scope = {"path": path, "kind": "const", "module": list(module)}
        entry = propose_entry(root, scope, symbol, values, search_in, words)
        return [json.dumps(entry, indent=2)]
    unlisted: dict = {}
    notes: list = []
    check(root, working, tasks, True, notes, unlisted)
    for scope, symbol in unlisted.get("limits", []):
        lines.append(f"# unlisted limit `{symbol}` in {scope['path']}: list it or exclude it")
        lines.append(json.dumps(propose_entry(root, scope, symbol, values, search_in), indent=2))
    for catalog, field in unlisted.get("fields", []):
        lines.append(
            f"# catalog {catalog['id']}: field `{field}` is not recorded; map it to a bound "
            "in `fields` or add it to an `excluded` group with the reason"
        )
    for note in notes:
        lines.append(f"# --refresh would change: {note}")
    records = [(bound.get("id"), bound) for bound in contract["bounds"]]
    for key in ("declared", "relations", "derived", "stage_tests", "behaviors"):
        records.extend((f"{key} {item.get('id', item.get('stage'))}", item) for item in contract.get(key, []))
    for where, record in records:
        for site in record.get("enforcement", []):
            try:
                source = _read(root, site["path"], str(where))
            except ContractError:
                lines.append(f"# {where}: {site['path']} does not exist")
                continue
            if normalize(site["contains"]) in _normalized(source):
                continue
            candidates = sorted({normalize(line) for line in source.splitlines()} - {""})
            close = difflib.get_close_matches(normalize(site["contains"]), candidates, 3, 0.5)
            lines.append(f"# {where}: site `{site['contains']}` is gone from {site['path']}")
            lines.extend(f"#   closest: {line}" for line in close)
    return lines


def main(argv: list | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    parser.add_argument("--show", action="store_true", help="print every bound")
    parser.add_argument(
        "--refresh",
        action="store_true",
        help="rewrite each bound's expression and value and each site's occurrence count "
        "from the source and drop exclusions that match nothing, keeping every "
        "classification; then check the result",
    )
    parser.add_argument(
        "--propose",
        action="store_true",
        help="print a bound skeleton with candidate sites and tests for every unlisted "
        "limit, the closest lines for every site that is gone and what --refresh would "
        "change; write nothing",
    )
    parser.add_argument(
        "--propose-for",
        metavar="PATH:SYMBOL",
        help="print the bound skeleton of one constant; write nothing",
    )
    parser.add_argument(
        "--search-in",
        action="append",
        default=[],
        metavar="DIR",
        help="also search this directory for candidate sites and tests (repeatable; "
        "the owner's crate is always searched)",
    )
    parser.add_argument(
        "--module",
        action="append",
        default=[],
        metavar="ANCHOR",
        help="with --propose-for: a module that holds the constant, outermost first, "
        "written as in the source, for example 'pub mod nexus {' (repeatable)",
    )
    parser.add_argument(
        "--word",
        action="append",
        default=[],
        metavar="NAME",
        help="with --propose-for: another identifier to search sites and tests for, "
        "for example the configuration field a default is read through (repeatable)",
    )
    arguments = parser.parse_args(argv)
    contract = json.loads(CONTRACT.read_text(encoding="utf-8"))
    if arguments.propose or arguments.propose_for:
        lines = propose(
            ROOT,
            contract,
            load_tasks(GRAPH),
            arguments.search_in,
            arguments.propose_for,
            tuple(arguments.module),
            tuple(arguments.word),
        )
        print("\n".join(lines) if lines else "nothing to propose: the contract matches the source")
        return 0
    before = render(contract)
    notes: list = []
    problems = check(contract=contract, refresh=arguments.refresh, notes=notes)
    if arguments.refresh:
        for note in notes:
            print(f"refresh: {note}")
        after = render(contract)
        if after != before:
            CONTRACT.write_text(after, encoding="utf-8")
            print(f"{CONTRACT.relative_to(ROOT)}: rewritten from the source; review the diff")
        else:
            print(f"{CONTRACT.relative_to(ROOT)}: already matches the source")
    if arguments.show:
        for bound in contract["bounds"]:
            print(
                f"{bound['id']:<52} {bound['value']:>14} {bound['unit']:<12} "
                f"{bound['source']:<10} {bound['class']}"
            )
    if problems:
        for problem in problems:
            print(f"error: {problem}", file=sys.stderr)
        print(f"{len(problems)} problem(s) in {CONTRACT.relative_to(ROOT)}", file=sys.stderr)
        return 1
    scopes = expand_scopes(ROOT, contract["completeness"], [])
    print(
        f"{CONTRACT.relative_to(ROOT)}: {len(contract['bounds'])} bounds, "
        f"{len(contract.get('derived', []))} derived, "
        f"{len(contract.get('relations', []))} relations, "
        f"{len(contract.get('declared', []))} declared items, "
        f"{sum(len(catalog['fields']) for catalog in contract.get('catalogs', []))} "
        f"catalog fields and {len(scopes)} scanned scopes match the source"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
