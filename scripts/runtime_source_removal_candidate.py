#!/usr/bin/env python3
"""Stage exact reviewed native-source omissions for the Core-owned retirement rehearsal.

This helper grants no retirement, build, replay, or release authority. The calling Core test
retains its original source handles and all-root receipt join while it uses the isolated tree.
The input census is a build input list; a successful build still has to establish its closure.
"""

from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import shutil
import stat
import sys
import subprocess


class CandidateError(RuntimeError):
    """An exact source preimage or isolated candidate invariant was not satisfied."""


MAX_FILES = 100_000
MAX_FILE_BYTES = 256 * 1024 * 1024
MAX_TOTAL_BYTES = 8 * 1024 * 1024 * 1024
BUILD_ROOTS = {"crates", "vendor", "data_model", "integration_tests", "defaults", "fixtures", ".cargo", "scripts", "tools"}
BUILD_FILES = {
    "Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "rustfmt.toml", "norito.md",
    "generatediso20022.md", "build.rs",
    # Exact public compiler inputs and selected runtime/model regression fixtures.
    # The storage-layout fixture is optional until its genuine first Core capture.
    "artifacts/poseidon/constants.ron",
    "codec/rans/tables/rans_seed0.toml",
    "contracts/evm/sccp/test/sccp_v1_fixtures.test.js",
    "javascript/iroha_js/test/fixtures/race-v1-codec.json",
    "javascript/iroha_js/test/fixtures/retail_fee_codec_v1.json",
    "specs/contract_state_value_proof_v1.md",
    "specs/runtime_baseline_catalog.json",
    "specs/runtime_baseline_instruction_layouts.tsv",
    "specs/runtime_baseline_storage_layouts.tsv",
    "specs/runtime_ironwood_catalog.json",
    "specs/state_table_inventory.json",
    "specs/sumeragi.md",
    "specs/zk_operation_inventory.json",
}
BUILD_SUBTREES = (".oss-scanner/build-features/", "mochi/mochi-core/", "mochi/mochi-integration/",
                  "mochi/mochi-ui-egui/", "python/iroha_python/iroha_python_rs/", "xtask/",
                  "experiments/kagemusha-spartan/")
# Reviewed static compiler programs, source-contract strings and deterministic public
# research vectors. Never widen these exceptions to a directory or filename glob.
REVIEWED_SOURCE_DATA = {
    "crates/kotodama_lang/src/assets/diagnostics_v1/secret_reject_cases_v1.tsv",
    "crates/kotodama_lang/fixtures/koto_v1/secret/001.ko",
    "crates/kotodama_lang/fixtures/koto_v1/secret/002.ko",
    "crates/kotodama_lang/fixtures/koto_v1/secret/003.ko",
    "crates/kotodama_lang/fixtures/koto_v1/secret/004.ko",
    "crates/kotodama_lang/fixtures/koto_v1/secret/005.ko",
    "crates/kotodama_lang/fixtures/koto_v1/secret/006.ko",
    "crates/kotodama_lang/fixtures/koto_v1/secret/007.ko",
    "crates/kotodama_lang/fixtures/koto_v1/secret/008.ko",
    "crates/iroha_zkp_halo2/src/generalized_bulletproof_secret_cleanup_contracts_v1.txt",
    "experiments/kagemusha-spartan/fixtures/credential-source-vectors.txt",
    "experiments/kagemusha-spartan/fixtures/credential-532.txt",
}


def _safe_name(name: str) -> None:
    lowered = name.lower()
    if "sydneycred" in "".join(char for char in lowered if char.isalnum()):
        raise CandidateError(f"forbidden legacy credential name: {name}")
    if name in REVIEWED_SOURCE_DATA:
        return
    code = Path(name).suffix in {".rs", ".py", ".sh", ".cc", ".cpp", ".h", ".proto"}
    leaf = Path(name).name.lower()
    if (not code and any(token in lowered for token in ("private_key", "private-key", "secret", "credential", "onboarding-token"))) or Path(name).suffix in {".pem", ".key", ".p12", ".pfx", ".env"} or leaf == ".env" or leaf in {"id_rsa", "id_ed25519"}:
        raise CandidateError(f"credential-like input is outside this rehearsal: {name}")


def fingerprint(data: bytes, info: os.stat_result) -> dict:
    """Pin original bytes and native identity; equal bytes after replacement are insufficient."""
    return {"sha256": hashlib.sha256(data).hexdigest(), "size": info.st_size,
            "device": info.st_dev, "inode": info.st_ino, "mtime": info.st_mtime_ns,
            "ctime": info.st_ctime_ns, "mode": info.st_mode, "links": info.st_nlink}


def capture(root: Path) -> dict:
    """Capture only explicit native build roots, refusing unknown or credential-like inputs.

    Git supplies names only. No untracked root file or client credential store is opened.
    Missing compiler inputs must fail the subsequent isolated build, never widen this census.
    """
    result = subprocess.run(["git", "-C", str(root), "ls-files", "--cached", "--others", "--exclude-standard", "-z"], check=True, capture_output=True)
    names = set()
    for raw in result.stdout.split(b"\0"):
        if not raw:
            continue
        name = raw.decode("utf-8")
        path = PurePosixPath(name)
        if name in BUILD_FILES or path.parts[0] in BUILD_ROOTS or name.startswith(BUILD_SUBTREES) or name == "specs/kotodama_grammar.md":
            relative(name)
            try:
                _safe_name(name)
            except CandidateError:
                continue
            # Known secret-policy source filenames must be explicitly reviewed into an
            # allowlist before use; filename similarity never authorizes reading secrets.
            names.add(name)
    if not names or len(names) > MAX_FILES:
        raise CandidateError("invalid build input census")
    captured = {}
    total = 0
    for name in sorted(names):
        data, metadata = _file(root, name)
        total += len(data)
        if total > MAX_TOTAL_BYTES:
            raise CandidateError("source exceeds total staging byte limit")
        captured[name] = fingerprint(data, metadata)
    return captured


def relative(value: str) -> Path:
    """Accept a single canonical relative source name."""
    path = PurePosixPath(value)
    if (
        not value
        or not path.parts
        or path.is_absolute()
        or "\\" in value
        or ":" in value
        or str(path) != value
        or any(part in (".", "..", ".git", "target") for part in path.parts)
        or any(ord(char) < 32 for char in value)
    ):
        raise CandidateError(f"noncanonical source path: {value!r}")
    return Path(*path.parts)


def _file(root: Path, name: str) -> tuple[bytes, os.stat_result]:
    _safe_name(name)
    path = root / relative(name)
    for ancestor in reversed(path.parents):
        if ancestor == root or root in ancestor.parents:
            if stat.S_ISLNK(ancestor.lstat().st_mode):
                raise CandidateError(f"source ancestor is a link: {name}")
    before = path.lstat()
    if not stat.S_ISREG(before.st_mode) or before.st_nlink != 1:
        raise CandidateError(f"source is not an independent regular file: {name}")
    if before.st_size > MAX_FILE_BYTES:
        raise CandidateError(f"source exceeds staging byte limit: {name}")
    with path.open("rb") as stream:
        opened = os.fstat(stream.fileno())
        if (opened.st_dev, opened.st_ino) != (before.st_dev, before.st_ino):
            raise CandidateError(f"source was replaced while opening: {name}")
        data = stream.read(MAX_FILE_BYTES + 1)
        if len(data) > MAX_FILE_BYTES:
            raise CandidateError(f"source exceeds staging byte limit while reading: {name}")
        after = os.fstat(stream.fileno())
    current = path.lstat()
    fields = lambda info: (
        info.st_dev, info.st_ino, info.st_size, info.st_mtime_ns, info.st_ctime_ns,
        info.st_mode, info.st_nlink,
    )
    if fields(before) != fields(after) or fields(before) != fields(current):
        raise CandidateError(f"source changed while reading: {name}")
    return data, before


def stage(root: Path, output: Path, request: dict) -> None:
    """Copy the exact input census and apply only the independently reviewed hook edits.

    The destination must be new and outside the original source tree. Original files are
    never opened for writing. Any failure removes only the newly created destination.
    """
    root = root.resolve(strict=True)
    output = output.absolute()
    if output.exists() or output.is_symlink():
        raise CandidateError("candidate output already exists")
    parent = output.parent.resolve(strict=True)
    if parent == root or root in parent.parents:
        raise CandidateError("candidate must be outside the original source tree")
    output = parent / output.name
    names = request["inputs"]
    if not names or len(names) > MAX_FILES or names != sorted(set(names)):
        raise CandidateError("build inputs must be sorted and unique")
    for name in names:
        relative(name)
    pinned = request["pinned"]
    if set(pinned) != set(names):
        raise CandidateError("every build input needs its original identity and byte pin")
    reviewed = request["reviewed"]
    omitted = request["omit"]
    edits = request["edits"]
    if not omitted or omitted != sorted(set(omitted)):
        raise CandidateError("omissions must be sorted and unique")
    affected = set(omitted) | {edit["path"] for edit in edits}
    if set(reviewed) != affected or not affected <= set(names):
        raise CandidateError("every omission and hook edit needs an exact reviewed input")
    if set(omitted) & {edit["path"] for edit in edits}:
        raise CandidateError("a source cannot be both omitted and edited")
    for name, digest in reviewed.items():
        data, _ = _file(root, name)
        if hashlib.sha256(data).hexdigest() != digest:
            raise CandidateError(f"source differs from reviewed preimage: {name}")
    output.mkdir(mode=0o700)
    try:
        observed = {}
        total = 0
        for name in names:
            data, metadata = _file(root, name)
            if fingerprint(data, metadata) != pinned[name]:
                raise CandidateError(f"original build input changed: {name}")
            if name in reviewed and hashlib.sha256(data).hexdigest() != reviewed[name]:
                raise CandidateError(f"source differs from reviewed preimage during copy: {name}")
            total += len(data)
            if total > MAX_TOTAL_BYTES:
                raise CandidateError("source exceeds total staging byte limit")
            observed[name] = hashlib.sha256(data).digest()
            if name in omitted:
                continue
            destination = output / relative(name)
            destination.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
            with destination.open("xb") as stream:
                stream.write(data)
            destination.chmod(stat.S_IMODE(metadata.st_mode))
        for edit in edits:
            destination = output / relative(edit["path"])
            data = destination.read_bytes()
            before, after = edit["before"].encode(), edit["after"].encode()
            if not before or data.count(before) != 1:
                raise CandidateError(f"reviewed hook is absent or ambiguous: {edit['path']}")
            destination.write_bytes(data.replace(before, after, 1))
        for name, expected in observed.items():
            data, metadata = _file(root, name)
            if hashlib.sha256(data).digest() != expected or fingerprint(data, metadata) != pinned[name]:
                raise CandidateError(f"original source changed during staging: {name}")
        for name in omitted:
            if (output / relative(name)).exists():
                raise CandidateError(f"omitted source remains: {name}")
    except BaseException:
        shutil.rmtree(output)
        raise


def verify(root: Path, output: Path, request: dict) -> None:
    """Recheck the exact isolated source delta around the real build and replay process."""
    expected = set(request["inputs"]) - set(request["omit"])
    observed = set()
    for directory, directories, files in os.walk(output, followlinks=False):
        for name in directories:
            if (Path(directory) / name).is_symlink():
                raise CandidateError("candidate directory became a link")
        observed.update((Path(directory) / name).relative_to(output).as_posix() for name in files)
    if expected != observed:
        raise CandidateError("candidate input membership changed")
    edits = {}
    for edit in request["edits"]:
        edits.setdefault(edit["path"], []).append(edit)
    for name in request["inputs"]:
        data, metadata = _file(root, name)
        if fingerprint(data, metadata) != request["pinned"][name]:
            raise CandidateError(f"original build input changed: {name}")
        if name in request["reviewed"] and hashlib.sha256(data).hexdigest() != request["reviewed"][name]:
            raise CandidateError(f"source differs from reviewed preimage: {name}")
        if name in request["omit"]:
            continue
        for edit in edits.get(name, []):
            before = edit["before"].encode()
            if data.count(before) != 1:
                raise CandidateError(f"reviewed hook is absent or ambiguous: {name}")
            data = data.replace(before, edit["after"].encode(), 1)
        actual, _ = _file(output, name)
        if data != actual:
            raise CandidateError(f"candidate source changed: {name}")


def main() -> None:
    """Consume the Core owner's explicit stage request; emit no qualification report."""
    if len(sys.argv) == 4 and sys.argv[1] == "capture":
        with Path(sys.argv[3]).open("x") as output:
            json.dump(capture(Path(sys.argv[2])), output, sort_keys=True)
        return
    if len(sys.argv) != 5 or sys.argv[1] not in ("stage", "verify"):
        raise SystemExit("usage: runtime_source_removal_candidate.py capture SOURCE PINS_JSON | stage|verify SOURCE TREE REQUEST_JSON")
    request = json.loads(Path(sys.argv[4]).read_text())
    (stage if sys.argv[1] == "stage" else verify)(Path(sys.argv[2]), Path(sys.argv[3]), request)


if __name__ == "__main__":
    main()
