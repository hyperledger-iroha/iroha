#!/usr/bin/env python3
"""Reject repository-owned Wasm/WASI runtimes, artifacts and build support.

Requires Python 3.10+ and Git. Checks tracked and nonignored untracked files in
the current checkout, including unstaged edits. It never changes files and has
no environment override. Upstream vendored source is not an Iroha build target.
Only the named upstream source roots and exact reviewed evidence bytes are
exempt from text checks; artifacts are checked everywhere in the Git-owned boundary.
This static declaration check does not resolve dynamically constructed programs.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys


SELF = "scripts/check_ivm_only.py"
TEST = "pytests/scripts/ivm_only_guard_test.py"
EVIDENCE = "scripts/ivm_policy_evidence.json"
REJECTION_ASSERTIONS = {
    "crates/iroha_plonk/tests/source_identity.rs":
        'assert!(!supported_target("wasm32", "unknown", "32", "little"));',
}
UPSTREAM_VENDOR_ROOTS = {
    "vendor/find_cuda_helper",
    "vendor/vega-prover",
}
JS_SUFFIXES = {".js", ".mjs", ".cjs", ".ts", ".tsx", ".jsx"}
BUILD_SUFFIXES = JS_SUFFIXES | {
    ".rs", ".toml", ".json", ".yml", ".yaml", ".sh", ".py",
    ".kts", ".gradle", ".kt", ".nix", ".cmake", ".c", ".h", ".cpp",
    ".hpp", ".cc", ".cxx", ".java", ".swift", ".cs", ".fs", ".fsx",
    ".ps1", ".bash", ".zsh", ".bat", ".cmd", ".dockerfile", ".go",
}
BUILD_FILENAMES = {"Makefile", "Dockerfile", "CMakeLists.txt"}
TARGET = re.compile(r"\b(?:wasm32|wasm64)(?:-[\w-]+)?\b|\bwasm_js\b")
TARGET_FAMILY = re.compile(r'''\btarget_family\s*=\s*["']wasm["']''')
TOOL = re.compile(r"\b(?:wasm[-_]bindgen|wasm-pack|wasmtime|wasmer|wasmi|wasmparser|wasm-encoder)\b")
WASI = re.compile(r'target_os\s*=\s*[\"\']wasi[\"\']|\ballow_wasi\s*:|^\s*WasiLite\s*[,={]')
CONFIG_WASI = re.compile(r'(?m)^\s*allow_wasi\s*=|[\"\']allow_wasi[\"\']\s*:')
NO_STD = re.compile(r"#!\s*\[\s*(?:no_std\b|cfg_attr\s*\([^\]]*\bno_std\b)")
JS_RUNTIME = re.compile(
    r"\bWebAssembly\s*(?:\.|\[)"
    r"|\b(?:globalThis|global|window|self)\s*\.\s*WebAssembly\b"
    r"|\brequire\s*\(\s*[\"'](?:@[^/]+/)?(?:wasm|wasi)"
    r"|\b(?:from\s*|require\s*\(\s*|import\s*\(?\s*)[\"'](?:node:)?wasi[\"']"
)
GRADLE_TARGET = re.compile(r"\bwasm(?:Js|Wasi)?\s*[({]")
GO_BUILD = re.compile(
    r"^\s*//\s*(?:go:build|\+build)\s+.*\b(?:wasm|wasm32|wasm64|wasi|wasip1|wasip2)\b"
)
GO_TARGET_FILE = re.compile(r"_(?:wasm|wasm32|wasm64|wasi|wasip1|wasip2)(?:_test)?\.go$")


def repository_paths(root: Path) -> list[Path]:
    """Enumerate the current Git-owned source boundary without a base ref."""
    output = subprocess.run(
        ["git", "-C", str(root), "ls-files", "-z", "--cached", "--others", "--exclude-standard"],
        check=True, stdout=subprocess.PIPE,
    ).stdout
    return sorted({Path(name.decode("utf-8")) for name in output.split(b"\0") if name})


def reviewed_evidence(root: Path) -> dict[str, str]:
    """Load exact reviewed JSON evidence identities, never a directory exemption."""
    path = root / EVIDENCE
    if not path.exists():
        return {}
    manifest = json.loads(path.read_text(encoding="utf-8"))
    if (
        not isinstance(manifest, dict)
        or set(manifest) != {"version", "records"}
        or type(manifest["version"]) is not int
        or manifest["version"] != 1
        or not isinstance(manifest["records"], list)
    ):
        raise ValueError("expected version 1 evidence registry with a records array")
    identities = {}
    for record in manifest["records"]:
        if not isinstance(record, dict) or set(record) != {"path", "sha256", "kind"}:
            raise ValueError("invalid evidence record fields")
        name, digest, kind = record["path"], record["sha256"], record["kind"]
        if (
            not isinstance(name, str)
            or not re.fullmatch(r"docs/validation/(?:[A-Za-z0-9_.-]+/)*[A-Za-z0-9_.-]+\.json", name)
            or any(part in {".", ".."} for part in name.split("/"))
            or name in identities
            or not isinstance(digest, str)
            or not re.fullmatch(r"[0-9a-f]{64}", digest)
            or kind not in ("source-digests", "cargo-metadata", "compiler-version")
        ):
            raise ValueError("invalid, duplicate or out-of-bound evidence identity")
        identities[name] = digest
    return identities


def check_path(root: Path, relative: Path, evidence: dict[str, str] | None = None) -> list[str]:
    """Return concrete forbidden artifacts or executable/build declarations."""
    name = relative.as_posix()
    path = root / relative
    if not path.is_file():
        return []
    suffix = relative.suffix.lower()
    if GO_TARGET_FILE.search(relative.name):
        return [f"{name}: Go WebAssembly/WASI build targets are prohibited"]
    if suffix in {".wasm", ".wat", ".wast"}:
        return [f"{name}: WebAssembly artifacts are prohibited; IVM bytecode uses .to"]
    with path.open("rb") as stream:
        if stream.read(4) == b"\x00asm":
            return [f"{name}: WebAssembly binary magic is prohibited"]
    # These historical reports contain dependency source inventories and tool
    # version output, not build instructions. Any changed bytes lose the exemption.
    if evidence and name in evidence:
        if hashlib.sha256(path.read_bytes()).hexdigest() == evidence[name]:
            return []
    # Exempt source text only. A vendored binary remains a
    # prohibited artifact, and an arbitrary nested directory is not upstream.
    if name in {SELF, TEST}:
        return []
    if any(name.startswith(prefix + "/") for prefix in UPSTREAM_VENDOR_ROOTS):
        return []
    if suffix not in BUILD_SUFFIXES and relative.name not in BUILD_FILENAMES:
        return []
    # Lockfiles record optional upstream targets too. Their corresponding
    # first-party package manifests are always checked.
    if relative.name in {"package-lock.json", "pnpm-lock.yaml"}:
        return []
    rules = [TARGET, TARGET_FAMILY, TOOL, WASI]
    if suffix in {".toml", ".json"}:
        rules.append(CONFIG_WASI)
    if suffix in JS_SUFFIXES:
        rules.append(JS_RUNTIME)
    if suffix in {".kts", ".gradle"}:
        rules.append(GRADLE_TARGET)
    lines = path.read_text(encoding="utf-8").splitlines()
    code_lines = [
        "" if line.lstrip().startswith(("//", "# ", "///", "//!", "* ")) else line
        for line in lines
    ]
    findings = []
    found_lines = set()
    for number, line in enumerate(code_lines, 1):
        # A single known rejection assertion is data for the native-target
        # validator. Neither the test file nor general target strings are exempt.
        if line.strip() == REJECTION_ASSERTIONS.get(name):
            continue
        if any(rule.search(line) for rule in rules):
            found_lines.add(number)
    if suffix == ".go":
        # Go selects build targets through comments. Check these before the
        # generic comment exclusion can erase the active build constraint.
        for number, line in enumerate(lines, 1):
            if GO_BUILD.search(line):
                found_lines.add(number)
    if suffix == ".rs":
        # Rust attributes may span lines, including cfg_attr's condition and
        # no_std argument. Preserve newlines when skipping documentation.
        code = "\n".join(code_lines)
        for rule in (NO_STD, TARGET_FAMILY):
            for match in rule.finditer(code):
                found_lines.add(code.count("\n", 0, match.start()) + 1)
    for number in sorted(found_lines):
        findings.append(
            f"{name}:{number}: forbidden non-IVM build/runtime declaration: {lines[number - 1].strip()}"
        )
    return findings


def main(argv: list[str] | None = None) -> int:
    """Run the full current-tree policy check and print actionable findings."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    args = parser.parse_args(argv)
    root = args.root.resolve()
    try:
        evidence = reviewed_evidence(root)
    except (OSError, ValueError) as error:
        print(f"IVM-only guard failed: {EVIDENCE}: {error}", file=sys.stderr)
        return 1
    findings = [finding for path in repository_paths(root) for finding in check_path(root, path, evidence)]
    if findings:
        print("IVM-only guard failed:", file=sys.stderr)
        print("\n".join(findings), file=sys.stderr)
        return 1
    print("IVM-only guard passed: no prohibited artifacts or static build/runtime declarations found.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
