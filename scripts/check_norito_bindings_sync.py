#!/usr/bin/env python3
# Copyright 2024 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0
"""
Cross-platform Norito bindings parity check.

This script replaces the previous Bash helper and ensures that changes to the
Rust Norito implementation are mirrored in the Python and Java bindings.  It
is intended to be run from the repository root.
"""

from __future__ import annotations

import glob
import os
import shutil
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable


REPO_ROOT = Path(__file__).resolve().parent.parent


class CheckError(RuntimeError):
    """Raised when one of the parity checks fails."""


def run_command(
    args: Iterable[str | os.PathLike[str]],
    *,
    cwd: Path | None = None,
    env: dict[str, str] | None = None,
    check: bool = True,
    capture_output: bool = False,
) -> subprocess.CompletedProcess[str]:
    """Run a subprocess and return the completed process object."""

    process = subprocess.run(
        [str(arg) for arg in args],
        cwd=str(cwd) if cwd else None,
        env=env,
        check=False,
        capture_output=capture_output,
        text=True,
    )
    if check and process.returncode != 0:
        raise subprocess.CalledProcessError(
            process.returncode, process.args, process.stdout, process.stderr
        )
    return process


def determine_base_ref() -> str:
    """Determine the Git base reference used for comparisons."""

    base_ref = os.environ.get("BASE_REF")
    if base_ref:
        return base_ref

    github_base_ref = os.environ.get("GITHUB_BASE_REF")
    if github_base_ref:
        return f"origin/{github_base_ref}"

    for candidate in ("origin/master", "origin/main"):
        try:
            run_command(
                ["git", "rev-parse", "--verify", candidate],
                check=True,
                capture_output=True,
            )
        except subprocess.CalledProcessError:
            continue
        return candidate

    rev_list = run_command(
        ["git", "rev-list", "--max-parents=0", "HEAD"], capture_output=True
    )
    commits = [line for line in rev_list.stdout.splitlines() if line.strip()]
    if not commits:
        return "HEAD"
    return commits[-1]


def compute_merge_base(base_ref: str) -> str:
    """Return the merge base between HEAD and the supplied base ref."""

    try:
        merge_base_proc = run_command(
            ["git", "merge-base", "HEAD", base_ref], capture_output=True
        )
    except subprocess.CalledProcessError:
        return base_ref

    merge_base = merge_base_proc.stdout.strip()
    return merge_base or base_ref


@dataclass
class PathFlags:
    needs_reference_update: bool = False
    python_updated: bool = False
    java_updated: bool = False


def normalise_path(path: str) -> str:
    """Convert filesystem paths to forward-slash separators."""

    return path.replace("\\", "/")


def update_flags(flags: PathFlags, raw_path: str) -> None:
    """Inspect a path and update bookkeeping flags in-place."""

    path = normalise_path(raw_path)
    if not path:
        return
    if path.startswith("crates/norito/tests/"):
        return
    if path == "crates/norito/src/streaming.rs" or path.startswith(
        "crates/norito/src/streaming/"
    ):
        return
    if path == "crates/norito/build.rs":
        return
    if (
        path.startswith("crates/norito/")
        or path.startswith("crates/norito_derive/")
        or path == "norito.md"
        or path == "docs/source/norito.md"
        or path.startswith("docs/source/norito/")
    ):
        flags.needs_reference_update = True
        return
    if path.startswith("python/norito_py/"):
        flags.python_updated = True
        return
    if path.startswith("java/norito_java/"):
        flags.java_updated = True


def gather_flags(merge_base: str) -> PathFlags:
    """Collect change flags from committed and uncommitted changes."""

    flags = PathFlags()
    diff = run_command(
        ["git", "diff", "--name-only", merge_base, "HEAD"], capture_output=True
    )
    for path in diff.stdout.splitlines():
        update_flags(flags, path)
        if flags.needs_reference_update and flags.python_updated and flags.java_updated:
            return flags

    status = run_command(["git", "status", "--porcelain"], capture_output=True)
    for line in status.stdout.splitlines():
        if len(line) < 4:
            continue
        update_flags(flags, line[3:])
        if flags.needs_reference_update and flags.python_updated and flags.java_updated:
            break

    return flags


def ensure_java_tool(tool: str) -> str | None:
    """Return the path to the requested Java tool, trying common fallbacks.

    macOS ships a stub `/usr/bin/java` that prompts for installation even when Homebrew
    JDKs are available. To avoid tripping over that stub we prefer JAVA_HOME/search
    prefixes before falling back to PATH lookups.
    """

    def prepend_path(directory: Path) -> None:
        os.environ["PATH"] = f"{directory}{os.pathsep}{os.environ.get('PATH', '')}"

    def tool_is_usable(executable: Path) -> bool:
        try:
            run_command([executable, "-version"], capture_output=True)
        except (FileNotFoundError, OSError, subprocess.CalledProcessError):
            return False
        return True

    java_home = os.environ.get("JAVA_HOME")
    if java_home:
        exe_path = Path(java_home) / "bin" / tool
        if exe_path.exists() and tool_is_usable(exe_path):
            prepend_path(exe_path.parent)
            return str(exe_path)

    search_prefixes = [
        "/opt/homebrew/opt/openjdk@21",
        "/usr/local/opt/openjdk@21",
    ]
    search_prefixes.extend(glob.glob("/opt/homebrew/opt/openjdk@*"))
    search_prefixes.extend(glob.glob("/usr/local/opt/openjdk@*"))
    search_prefixes.extend(glob.glob("/Library/Java/JavaVirtualMachines/*/Contents/Home"))

    seen: set[str] = set()
    for prefix in search_prefixes:
        if prefix in seen:
            continue
        seen.add(prefix)
        exe_path = Path(prefix) / "bin" / tool
        if exe_path.exists() and tool_is_usable(exe_path):
            os.environ.setdefault("JAVA_HOME", str(Path(prefix)))
            prepend_path(exe_path.parent)
            return str(exe_path)

    candidate = shutil.which(tool)
    if candidate and tool_is_usable(Path(candidate)):
        return candidate

    if tool == "java":
        try:
            java_home_proc = run_command(
                ["/usr/libexec/java_home"], capture_output=True
            )
        except (FileNotFoundError, subprocess.CalledProcessError):
            java_home_proc = None
        if java_home_proc:
            resolved = java_home_proc.stdout.strip()
            if resolved:
                exe_path = Path(resolved) / "bin" / tool
                if exe_path.exists() and tool_is_usable(exe_path):
                    os.environ.setdefault("JAVA_HOME", resolved)
                    prepend_path(exe_path.parent)
                    return str(exe_path)

    return None


def run_python_parity_checks() -> None:
    """Execute Norito Python binding parity checks."""

    print("[norito] Running Python binding parity checks...", file=sys.stderr)
    sys.path.insert(0, str(REPO_ROOT / "python" / "norito_py" / "src"))
    try:
        import norito  # type: ignore[import-not-found]
        from norito import (  # type: ignore[import-not-found]
            EnumCode,
            EnumName,
            SchemaDescriptor,
            decode_rows_u64_bytes_bool_adaptive,
            decode_rows_u64_enum_bool_adaptive,
            decode_rows_u64_optstr_bool_adaptive,
            decode_rows_u64_optu32_bool_adaptive,
            decode_rows_u64_str_bool_adaptive,
            encode_ncb_u64_bytes_bool,
            encode_ncb_u64_enum_bool,
            encode_ncb_u64_optstr_bool,
            encode_ncb_u64_optu32_bool,
            encode_ncb_u64_str_bool,
            encode_rows_u64_bytes_bool_adaptive,
            encode_rows_u64_enum_bool_adaptive,
            encode_rows_u64_optstr_bool_adaptive,
            encode_rows_u64_optu32_bool_adaptive,
            encode_rows_u64_str_bool_adaptive,
        )
        from norito import streaming  # type: ignore[import-not-found]
    except Exception as exc:  # pragma: no cover - import failure path
        raise CheckError(
            "Failed to import python Norito package; run pip install -r scripts/requirements.txt."
        ) from exc
    finally:
        sys.path.pop(0)

    structural = {
        "Sample": {
            "Struct": [
                {"name": "id", "type": "u64"},
                {"name": "name", "type": "String"},
                {"name": "flag", "type": "bool"},
            ]
        },
        "String": "String",
        "bool": "bool",
        "u64": {"Int": "FixedWidth"},
    }
    expected_hash = bytes.fromhex("0107fec5fc24ac0d0107fec5fc24ac0d")
    actual_hash = SchemaDescriptor.structural(structural).hash_bytes()
    if actual_hash != expected_hash:
        raise CheckError("Python structural schema hash mismatch")

    rows = [(1, "alice", True), (2, "bob", False), (3, "charlie", True)]
    expected_columnar = (
        "03000000530000000100000000000000020200000000000005000000080000000f000000"
        "616c696365626f62636861726c696505"
    )
    expected_adaptive = (
        "000301010000000000000005616c69636501020000000000000003626f62000300000000000000"
        "07636861726c696501"
    )
    columnar = encode_ncb_u64_str_bool(rows)
    adaptive = encode_rows_u64_str_bool_adaptive(rows)
    if columnar.hex() != expected_columnar:
        raise CheckError("Python columnar encoding diverged")
    if adaptive.hex() != expected_adaptive:
        raise CheckError("Python adaptive encoding diverged")
    decoded = decode_rows_u64_str_bool_adaptive(adaptive)
    if decoded != rows:
        raise CheckError("Python adaptive decode failed roundtrip")

    opt_rows = [(1, "a", False), (2, None, True), (3, "bc", False)]
    expected_opt_columnar = (
        "030000005b000000010000000000000002020500000000000000010000000300000061626302"
    )
    expected_opt_adaptive = (
        "0003010100000000000000010161000200000000000000000103000000000000000102626300"
    )
    columnar = encode_ncb_u64_optstr_bool(opt_rows)
    adaptive = encode_rows_u64_optstr_bool_adaptive(opt_rows)
    if columnar.hex() != expected_opt_columnar:
        raise CheckError("Python optstr columnar encoding diverged")
    if adaptive.hex() != expected_opt_adaptive:
        raise CheckError("Python optstr adaptive encoding diverged")
    if decode_rows_u64_optstr_bool_adaptive(adaptive) != opt_rows:
        raise CheckError("Python optstr adaptive decode failed roundtrip")

    optu32_rows = [(1, 7, False), (2, None, True), (3, 9, False)]
    expected_optu32_columnar = "030000005c0000000100000000000000020205000000070000000900000002"
    expected_optu32_adaptive = "01030000005c0000000100000000000000020205000000070000000900000002"
    columnar = encode_ncb_u64_optu32_bool(optu32_rows)
    adaptive = encode_rows_u64_optu32_bool_adaptive(optu32_rows)
    if columnar.hex() != expected_optu32_columnar:
        raise CheckError("Python optu32 columnar encoding diverged")
    if adaptive.hex() != expected_optu32_adaptive:
        raise CheckError("Python optu32 adaptive encoding diverged")
    if decode_rows_u64_optu32_bool_adaptive(adaptive) != optu32_rows:
        raise CheckError("Python optu32 adaptive decode failed roundtrip")

    byte_rows = [(1, b"abc", True), (2, b"\x00\xff", False)]
    expected_bytes_columnar = (
        "020000005400000001000000000000000200000000000000030000000500000061626300ff01"
    )
    expected_bytes_adaptive = "0002010100000000000000036162630102000000000000000200ff00"
    columnar = encode_ncb_u64_bytes_bool(byte_rows)
    adaptive = encode_rows_u64_bytes_bool_adaptive(byte_rows)
    if columnar.hex() != expected_bytes_columnar:
        raise CheckError("Python bytes columnar encoding diverged")
    if adaptive.hex() != expected_bytes_adaptive:
        raise CheckError("Python bytes adaptive encoding diverged")
    if decode_rows_u64_bytes_bool_adaptive(adaptive) != byte_rows:
        raise CheckError("Python bytes adaptive decode failed roundtrip")

    enum_rows = [
        (10, EnumName("x"), False),
        (12, EnumName("y"), True),
        (14, EnumCode(7), False),
        (15, EnumCode(8), True),
    ]
    expected_enum_columnar = (
        "04000000670000000a0000000000000004040200000101000000000001000000020000007879000007000000020a"
    )
    expected_enum_adaptive = (
        "0104000000670000000a0000000000000004040200000101000000000001000000020000007879000007000000020a"
    )
    columnar = encode_ncb_u64_enum_bool(enum_rows)
    adaptive = encode_rows_u64_enum_bool_adaptive(enum_rows)
    if columnar.hex() != expected_enum_columnar:
        raise CheckError("Python enum columnar encoding diverged")
    if adaptive.hex() != expected_enum_adaptive:
        raise CheckError("Python enum adaptive encoding diverged")
    if decode_rows_u64_enum_bool_adaptive(adaptive) != enum_rows:
        raise CheckError("Python enum adaptive decode failed roundtrip")

    # Streaming manifest/control-frame sanity check mirrors Rust codec behaviour.
    manifest = streaming.ManifestV1(
        stream_id=bytes(range(32)),
        protocol_version=1,
        segment_number=7,
        published_at=123456,
        profile=streaming.ProfileId(0),
        da_endpoint="nsc://example",
        chunk_root=bytes(range(32, 64)),
        content_key_id=9,
        nonce_salt=bytes(range(64, 96)),
        chunk_descriptors=[
            streaming.ChunkDescriptor(
                chunk_id=0,
                offset=0,
                length=128,
                commitment=bytes(range(96, 128)),
                parity=False,
            )
        ],
        transport_capabilities_hash=bytes(range(128, 160)),
        encryption_suite=streaming.X25519ChaCha20Poly1305Suite(bytes([0xAA] * 32)),
        fec_suite=streaming.FecScheme.RS12_10,
        privacy_routes=[],
        neural_bundle=None,
        public_metadata=streaming.StreamMetadata(
            title="demo",
            description=None,
            access_policy_id=None,
            tags=["test"],
        ),
        capabilities=streaming.CapabilityFlags(0),
        signature=bytes([0xBB] * 64),
    )
    control_frame = streaming.ControlFrame(
        streaming.ControlFrameVariant.MANIFEST_ANNOUNCE,
        streaming.ManifestAnnounceFrame(manifest),
    )
    schema = SchemaDescriptor("iroha.nsc.ControlFrame")
    adapter = streaming.control_frame_adapter()
    encoded = norito.encode(control_frame, schema, adapter)
    decoded_frame = norito.decode(encoded, adapter, schema=schema)
    if decoded_frame != control_frame:
        raise CheckError("Python streaming control frame parity mismatch")


def run_java_parity_checks() -> None:
    """Execute Norito Java binding parity checks."""

    if os.environ.get("NORITO_JAVA_SKIP_TESTS") == "1":
        print(
            "[norito-java] Skipping JVM parity tests (NORITO_JAVA_SKIP_TESTS=1).",
            file=sys.stderr,
        )
        return

    print("[norito] Running Java binding parity checks...", file=sys.stderr)

    javac_path = ensure_java_tool("javac")
    if not javac_path:
        if java_checks_are_strict():
            raise CheckError("javac not found; install JDK 21+ to run tests")
        print(
            "[norito-java] javac not found; skipping JVM parity checks outside strict mode.",
            file=sys.stderr,
        )
        return

    java_path = ensure_java_tool("java")
    if not java_path:
        if java_checks_are_strict():
            raise CheckError("java runtime not found; install JDK 21+ or set JAVA_HOME")
        print(
            "[norito-java] java runtime not found; skipping JVM parity checks outside strict mode.",
            file=sys.stderr,
        )
        return

    root = REPO_ROOT / "java" / "norito_java"
    build_root = REPO_ROOT / "target" / "norito_java"
    build_root.mkdir(parents=True, exist_ok=True)
    keep_build = os.environ.get("NORITO_JAVA_KEEP_BUILD") == "1"

    temp_dir = Path(tempfile.mkdtemp(prefix="classes_", dir=build_root))
    classes_dir = temp_dir / "classes"
    classes_dir.mkdir(parents=True, exist_ok=True)

    main_sources = [str(path) for path in (root / "src" / "main" / "java").rglob("*.java")]
    test_sources = [str(path) for path in (root / "src" / "test" / "java").rglob("*.java")]

    javac_flags = ["-d", str(classes_dir)]
    try:
        run_command(
            [javac_path, "--release", "21", "-version"],
            check=True,
            capture_output=True,
        )
    except subprocess.CalledProcessError:
        pass
    else:
        javac_flags = ["--release", "21", "-d", str(classes_dir)]

    try:
        run_command([javac_path, *javac_flags, *main_sources, *test_sources], cwd=root)
        run_command(
            [java_path, "-ea", "-cp", str(classes_dir), "org.hyperledger.iroha.norito.NoritoTests"],
            cwd=root,
        )
    finally:
        if keep_build:
            print(
                f"[norito-java] Preserving compiled classes under {classes_dir}",
                file=sys.stderr,
            )
        else:
            shutil.rmtree(temp_dir, ignore_errors=True)


def java_checks_are_strict() -> bool:
    """Return whether missing Java tooling should fail parity checks."""

    if os.environ.get("NORITO_JAVA_STRICT") == "1":
        return True

    ci_value = os.environ.get("CI", "").strip().lower()
    return ci_value in {"1", "true", "yes", "on"}


def main() -> int:
    os.chdir(REPO_ROOT)

    merge_base = compute_merge_base(determine_base_ref())
    flags = gather_flags(merge_base)

    if flags.needs_reference_update and (not flags.python_updated or not flags.java_updated):
        if not flags.python_updated:
            print(
                "Detected changes to Norito sources without updates under python/norito_py.",
                file=sys.stderr,
            )
        if not flags.java_updated:
            print(
                "Detected changes to Norito sources without updates under java/norito_java.",
                file=sys.stderr,
            )
        print(
            "Please update the language bindings to reflect the latest Norito changes.",
            file=sys.stderr,
        )
        return 1

    try:
        if flags.python_updated or flags.needs_reference_update:
            run_python_parity_checks()
        if flags.java_updated or flags.needs_reference_update:
            run_java_parity_checks()
    except CheckError as error:
        print(error, file=sys.stderr)
        message_lower = str(error).lower()
        if "python" in message_lower:
            print(
                "Python Norito parity checks failed. Run "
                "'python3 -m unittest discover -s python/norito_py/tests' to investigate.",
                file=sys.stderr,
            )
        elif "java" in message_lower:
            print(
                "Java Norito parity checks failed. Ensure a JDK is installed and rerun "
                "java/norito_java/run_tests.sh to investigate.",
                file=sys.stderr,
            )
        return 1
    except subprocess.CalledProcessError as error:
        print(error.stderr or error.stdout, file=sys.stderr)
        if "javac" in error.cmd[0]:
            print(
                "Java Norito parity checks failed. Ensure a JDK is installed and rerun "
                "java/norito_java/run_tests.sh to investigate.",
                file=sys.stderr,
            )
        else:
            print(
                "Python Norito parity checks failed. Run "
                "'python3 -m unittest discover -s python/norito_py/tests' to investigate.",
                file=sys.stderr,
            )
        return 1

    return 0


if __name__ == "__main__":
    sys.exit(main())
