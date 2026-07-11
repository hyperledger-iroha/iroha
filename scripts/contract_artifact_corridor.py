#!/usr/bin/env python3
"""Build authenticated, deterministic SCCP EVM and TVM contract artifacts."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import stat
import subprocess
import sys
import tempfile
import unicodedata
import urllib.request
from dataclasses import dataclass
from pathlib import Path
from typing import Callable, Iterable, Mapping, Sequence


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_COMPILER_LOCK = ROOT / "scripts" / "contract_tooling" / "compiler-lock.json"
DEFAULT_ARTIFACT_LOCK = ROOT / "scripts" / "contract_tooling" / "artifact-lock.json"
SOLJSON_RUNNER = ROOT / "scripts" / "contract_soljson_runner.js"
MANIFEST_NAME = "sccp-contract-artifacts-v1.json"
NATIVE_VECTORS_NAME = "native-transfer-event-v1.json"
MANIFEST_SCHEMA = "iroha.sccp.contract-artifacts.v1"
ARTIFACT_LOCK_SCHEMA = "iroha.sccp.contract-artifact-lock.v1"
COMPILER_LOCK_SCHEMA = "iroha.sccp.contract-compiler-lock.v1"
TARGETS = ("evm", "tron")
SOLIDITY_VERSION_PRAGMA = "pragma solidity 0.8.24;"
EXPECTED_COMPILERS = {
    "evm": {
        "identity": "solc-evm-0.8.24+commit.e11b9ed9",
        "reported_version": "0.8.24+commit.e11b9ed9.Emscripten.clang",
        "sha256": "11b054b55273ec55f6ab3f445eb0eb2c83a23fed43d10079d34ac3eabe6ed8b1",
        "url": "https://binaries.soliditylang.org/wasm/soljson-v0.8.24+commit.e11b9ed9.js",
    },
    "tron": {
        "identity": "tron-solc-tvm-0.8.24+commit.7d902c66",
        "reported_version": "0.8.24+commit.7d902c66.Emscripten.clang",
        "sha256": "527b5363b50eee33b9d45a1619ccd3511e6304637867135396969ac93bc67116",
        "url": "https://raw.githubusercontent.com/tronprotocol/solc-bin/main/wasm/soljson-v0.8.24+commit.7d902c66.js",
    },
}
MAX_COMPILER_BYTES = 64 * 1024 * 1024
MAX_SOURCE_BYTES = 2 * 1024 * 1024
MAX_COMPILER_OUTPUT_BYTES = 128 * 1024 * 1024
MAX_DIAGNOSTIC_BYTES = 16 * 1024
HEX_32_RE = re.compile(r"^[0-9a-f]{64}$")
IDENTIFIER_RE = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*$")
SAFE_PATH_SEGMENT_RE = re.compile(r"^[A-Za-z0-9_.-]+$")


class CorridorError(ValueError):
    """A bounded, user-actionable contract-corridor failure."""


@dataclass(frozen=True)
class CompilerSpec:
    """One immutable compiler identity."""

    target: str
    identity: str
    reported_version: str
    url: str
    sha256: str


@dataclass(frozen=True)
class CorridorConfig:
    """Strict compiler, source, and size configuration."""

    compilers: Mapping[str, CompilerSpec]
    settings: Mapping[str, object]
    sources: Mapping[str, tuple[str, ...]]
    size_limits: Mapping[str, Mapping[str, int]]
    tvm_runner: Mapping[str, str]
    canonical_sha256: str


@dataclass
class CompiledTarget:
    """Internal compiler result retained for cross-target alias checks."""

    target: str
    source_paths: tuple[str, ...]
    input_sha256: str
    compiler_sha256: str
    raw_output: Mapping[str, object]
    manifest: Mapping[str, object]


def _reject_constant(value: str) -> object:
    raise CorridorError(f"JSON numeric constant is not allowed: {value}")


def _unique_object(pairs: Sequence[tuple[str, object]]) -> dict[str, object]:
    result: dict[str, object] = {}
    for key, value in pairs:
        if key in result:
            raise CorridorError(f"JSON object contains duplicate key `{key}`")
        result[key] = value
    return result


def parse_json_bytes(payload: bytes, label: str) -> object:
    """Parse bounded UTF-8 JSON while rejecting duplicate keys and nonfinite numbers."""

    if not payload:
        raise CorridorError(f"{label} must not be empty")
    try:
        text = payload.decode("utf-8")
    except UnicodeDecodeError as error:
        raise CorridorError(f"{label} must be UTF-8 JSON") from error
    try:
        return json.loads(
            text,
            object_pairs_hook=_unique_object,
            parse_constant=_reject_constant,
        )
    except (json.JSONDecodeError, CorridorError) as error:
        if isinstance(error, CorridorError):
            raise
        raise CorridorError(f"{label} is malformed JSON") from error


def canonical_json_bytes(value: object) -> bytes:
    """Return the corridor's root-independent canonical JSON encoding."""

    try:
        encoded = json.dumps(
            value,
            ensure_ascii=False,
            allow_nan=False,
            sort_keys=True,
            separators=(",", ":"),
        ).encode("utf-8")
    except (TypeError, ValueError) as error:
        raise CorridorError("value cannot be encoded as canonical JSON") from error
    return encoded


def sha256_hex(payload: bytes) -> str:
    """Return one lowercase SHA-256 digest."""

    return hashlib.sha256(payload).hexdigest()


_KECCAK_ROTATIONS = (
    (0, 36, 3, 41, 18),
    (1, 44, 10, 45, 2),
    (62, 6, 43, 15, 61),
    (28, 55, 25, 21, 56),
    (27, 20, 39, 8, 14),
)
_KECCAK_ROUND_CONSTANTS = (
    0x0000000000000001,
    0x0000000000008082,
    0x800000000000808A,
    0x8000000080008000,
    0x000000000000808B,
    0x0000000080000001,
    0x8000000080008081,
    0x8000000000008009,
    0x000000000000008A,
    0x0000000000000088,
    0x0000000080008009,
    0x000000008000000A,
    0x000000008000808B,
    0x800000000000008B,
    0x8000000000008089,
    0x8000000000008003,
    0x8000000000008002,
    0x8000000000000080,
    0x000000000000800A,
    0x800000008000000A,
    0x8000000080008081,
    0x8000000000008080,
    0x0000000080000001,
    0x8000000080008008,
)
_MASK_64 = (1 << 64) - 1


def _rotate_left_64(value: int, shift: int) -> int:
    if shift == 0:
        return value & _MASK_64
    return ((value << shift) | (value >> (64 - shift))) & _MASK_64


def _keccak_f1600(state: list[int]) -> None:
    for round_constant in _KECCAK_ROUND_CONSTANTS:
        columns = [
            state[x]
            ^ state[x + 5]
            ^ state[x + 10]
            ^ state[x + 15]
            ^ state[x + 20]
            for x in range(5)
        ]
        deltas = [
            columns[(x - 1) % 5] ^ _rotate_left_64(columns[(x + 1) % 5], 1)
            for x in range(5)
        ]
        for y in range(5):
            for x in range(5):
                state[x + 5 * y] ^= deltas[x]

        rotated = [0] * 25
        for y in range(5):
            for x in range(5):
                new_x = y
                new_y = (2 * x + 3 * y) % 5
                rotated[new_x + 5 * new_y] = _rotate_left_64(
                    state[x + 5 * y], _KECCAK_ROTATIONS[x][y]
                )

        for y in range(5):
            row = rotated[5 * y : 5 * y + 5]
            for x in range(5):
                state[x + 5 * y] = row[x] ^ ((~row[(x + 1) % 5]) & row[(x + 2) % 5])
                state[x + 5 * y] &= _MASK_64
        state[0] ^= round_constant


def keccak256(payload: bytes) -> bytes:
    """Return legacy Keccak-256, not the distinct NIST SHA3-256 function."""

    rate = 136
    padded = bytearray(payload)
    padded.append(0x01)
    while len(padded) % rate != rate - 1:
        padded.append(0)
    padded.append(0x80)
    state = [0] * 25
    for offset in range(0, len(padded), rate):
        block = padded[offset : offset + rate]
        for lane in range(rate // 8):
            start = lane * 8
            state[lane] ^= int.from_bytes(block[start : start + 8], "little")
        _keccak_f1600(state)
    output = bytearray()
    while len(output) < 32:
        for lane in range(rate // 8):
            output.extend(state[lane].to_bytes(8, "little"))
            if len(output) >= 32:
                break
        if len(output) < 32:
            _keccak_f1600(state)
    return bytes(output[:32])


def keccak256_hex(payload: bytes) -> str:
    """Return one lowercase Keccak-256 digest."""

    return keccak256(payload).hex()


def _require_object(value: object, label: str) -> Mapping[str, object]:
    if not isinstance(value, dict):
        raise CorridorError(f"{label} must be a JSON object")
    return value


def _require_exact_keys(value: Mapping[str, object], expected: Iterable[str], label: str) -> None:
    expected_set = set(expected)
    actual_set = set(value)
    if actual_set != expected_set:
        raise CorridorError(f"{label} has missing or unknown fields")


def _require_string(value: object, label: str) -> str:
    if not isinstance(value, str) or not value or value != value.strip():
        raise CorridorError(f"{label} must be one nonempty canonical string")
    if any(ord(character) < 0x20 or ord(character) == 0x7F for character in value):
        raise CorridorError(f"{label} must not contain control characters")
    return value


def canonical_source_path(value: object, label: str) -> str:
    """Validate one portable, repository-relative POSIX source path."""

    path = _require_string(value, label)
    if "\\" in path or path.startswith("/") or unicodedata.normalize("NFC", path) != path:
        raise CorridorError(f"{label} must be a normalized repository-relative POSIX path")
    segments = path.split("/")
    if any(
        segment in ("", ".", "..") or not SAFE_PATH_SEGMENT_RE.fullmatch(segment)
        for segment in segments
    ):
        raise CorridorError(f"{label} contains an unsafe or nonportable path segment")
    return path


def _collision_key(path: str) -> str:
    return unicodedata.normalize("NFC", path).casefold()


def load_corridor_config(path: Path = DEFAULT_COMPILER_LOCK) -> CorridorConfig:
    """Load and strictly validate the committed compiler lock."""

    raw = path.read_bytes()
    parsed = _require_object(parse_json_bytes(raw, "compiler lock"), "compiler lock")
    _require_exact_keys(
        parsed,
        ("schema", "compilers", "settings", "sources", "size_limits", "tvm_runner"),
        "compiler lock",
    )
    if parsed["schema"] != COMPILER_LOCK_SCHEMA:
        raise CorridorError("compiler lock schema is unsupported")
    compiler_values = _require_object(parsed["compilers"], "compiler lock compilers")
    source_values = _require_object(parsed["sources"], "compiler lock sources")
    limit_values = _require_object(parsed["size_limits"], "compiler lock size limits")
    if set(compiler_values) != set(TARGETS) or set(source_values) != set(TARGETS):
        raise CorridorError("compiler lock must define distinct EVM and TRON targets")
    if set(limit_values) != set(TARGETS):
        raise CorridorError("compiler lock must define size limits for both targets")

    compilers: dict[str, CompilerSpec] = {}
    sources: dict[str, tuple[str, ...]] = {}
    size_limits: dict[str, Mapping[str, int]] = {}
    for target in TARGETS:
        compiler = _require_object(compiler_values[target], f"{target} compiler")
        _require_exact_keys(
            compiler,
            ("identity", "reported_version", "sha256", "url"),
            f"{target} compiler",
        )
        digest = _require_string(compiler["sha256"], f"{target} compiler sha256")
        if not HEX_32_RE.fullmatch(digest):
            raise CorridorError(f"{target} compiler sha256 must be lowercase hexadecimal")
        url = _require_string(compiler["url"], f"{target} compiler URL")
        if not url.startswith("https://"):
            raise CorridorError(f"{target} compiler URL must use HTTPS")
        compilers[target] = CompilerSpec(
            target=target,
            identity=_require_string(compiler["identity"], f"{target} compiler identity"),
            reported_version=_require_string(
                compiler["reported_version"], f"{target} compiler reported version"
            ),
            url=url,
            sha256=digest,
        )
        expected_compiler = EXPECTED_COMPILERS[target]
        if any(
            getattr(compilers[target], field) != expected_compiler[field]
            for field in ("identity", "reported_version", "url", "sha256")
        ):
            raise CorridorError(
                f"{target} compiler must be the authenticated exact Solidity 0.8.24 release"
            )

        source_list = source_values[target]
        if not isinstance(source_list, list) or not source_list:
            raise CorridorError(f"{target} source map must be one nonempty array")
        paths = tuple(
            canonical_source_path(value, f"{target} source path") for value in source_list
        )
        if paths != tuple(sorted(paths)):
            raise CorridorError(f"{target} source paths must be sorted")
        keys = [_collision_key(value) for value in paths]
        if len(keys) != len(set(keys)):
            raise CorridorError(f"{target} source paths contain a portable path collision")
        sources[target] = paths

        limits = _require_object(limit_values[target], f"{target} size limits")
        _require_exact_keys(
            limits,
            ("creation_bytecode_bytes", "runtime_bytecode_bytes"),
            f"{target} size limits",
        )
        checked_limits: dict[str, int] = {}
        for name, value in limits.items():
            if not isinstance(value, int) or isinstance(value, bool) or value <= 0:
                raise CorridorError(f"{target} {name} limit must be positive")
            checked_limits[name] = value
        size_limits[target] = checked_limits

    if sources["evm"] == sources["tron"]:
        raise CorridorError("EVM and TRON source maps must be distinct")
    if compilers["evm"].sha256 == compilers["tron"].sha256:
        raise CorridorError("EVM and TRON compiler artifacts must not alias")
    settings = _require_object(parsed["settings"], "compiler settings")
    tvm_runner = _require_object(parsed["tvm_runner"], "TVM runner")
    _require_exact_keys(tvm_runner, ("image", "platform"), "TVM runner")
    image = _require_string(tvm_runner["image"], "TVM runner image")
    if not re.fullmatch(r"[a-z0-9._/-]+@sha256:[0-9a-f]{64}", image):
        raise CorridorError("TVM runner image must use an immutable SHA-256 digest")
    platform = _require_string(tvm_runner["platform"], "TVM runner platform")
    return CorridorConfig(
        compilers=compilers,
        settings=settings,
        sources=sources,
        size_limits=size_limits,
        tvm_runner={"image": image, "platform": platform},
        canonical_sha256=sha256_hex(canonical_json_bytes(parsed)),
    )


CompilerFetcher = Callable[[str], bytes]
CompilerRunner = Callable[[Path, CompilerSpec, bytes, str], Mapping[str, object]]


def _network_fetch(url: str) -> bytes:
    request = urllib.request.Request(url, headers={"User-Agent": "iroha-sccp-contract-corridor/1"})
    with urllib.request.urlopen(request, timeout=60) as response:
        content_length = response.headers.get("Content-Length")
        if content_length is not None:
            try:
                length = int(content_length)
            except ValueError as error:
                raise CorridorError("compiler server returned an invalid Content-Length") from error
            if length <= 0 or length > MAX_COMPILER_BYTES:
                raise CorridorError("compiler download exceeds the bounded size policy")
        payload = response.read(MAX_COMPILER_BYTES + 1)
    return payload


def materialize_verified_compiler(
    spec: CompilerSpec,
    destination: Path,
    fetcher: CompilerFetcher = _network_fetch,
) -> Path:
    """Download one compiler and verify its digest before making it executable input."""

    if destination.exists() or destination.is_symlink():
        raise CorridorError("compiler destination collision")
    payload = fetcher(spec.url)
    if not isinstance(payload, bytes) or not payload or len(payload) > MAX_COMPILER_BYTES:
        raise CorridorError("compiler download is empty or exceeds the bounded size policy")
    actual_digest = sha256_hex(payload)
    if actual_digest != spec.sha256:
        raise CorridorError("authenticated compiler SHA-256 digest mismatch")
    destination.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    descriptor = os.open(destination, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    try:
        with os.fdopen(descriptor, "wb") as output:
            output.write(payload)
            output.flush()
            os.fsync(output.fileno())
    except BaseException:
        destination.unlink(missing_ok=True)
        raise
    if destination.is_symlink() or not destination.is_file():
        destination.unlink(missing_ok=True)
        raise CorridorError("verified compiler destination changed during publication")
    if sha256_hex(destination.read_bytes()) != spec.sha256:
        destination.unlink(missing_ok=True)
        raise CorridorError("verified compiler changed before execution")
    return destination


def _mask_solidity_comments_and_strings(source: str, relative: str) -> str:
    """Mask non-code regions while preserving byte offsets for pragma inspection."""

    output = list(source)
    index = 0
    state = "code"
    quote = ""
    while index < len(source):
        character = source[index]
        following = source[index + 1] if index + 1 < len(source) else ""
        if state == "code":
            if character == "/" and following == "/":
                output[index] = output[index + 1] = " "
                index += 2
                state = "line-comment"
                continue
            if character == "/" and following == "*":
                output[index] = output[index + 1] = " "
                index += 2
                state = "block-comment"
                continue
            if character in ("'", '"'):
                quote = character
                output[index] = " "
                index += 1
                state = "string"
                continue
        elif state == "line-comment":
            if character == "\n":
                state = "code"
            else:
                output[index] = " "
            index += 1
            continue
        elif state == "block-comment":
            if character == "*" and following == "/":
                output[index] = output[index + 1] = " "
                index += 2
                state = "code"
                continue
            if character != "\n":
                output[index] = " "
            index += 1
            continue
        else:
            if character == "\\" and following:
                output[index] = " "
                if following != "\n":
                    output[index + 1] = " "
                index += 2
                continue
            output[index] = " " if character != "\n" else "\n"
            index += 1
            if character == quote:
                state = "code"
            continue
        index += 1
    if state in ("block-comment", "string"):
        raise CorridorError(f"contract source contains an unterminated lexical region: {relative}")
    return "".join(output)


def validate_solidity_source_policy(source: str, relative: str) -> None:
    """Require only the exact first-release compiler pragma."""

    masked = _mask_solidity_comments_and_strings(source, relative)
    pragma_tokens = list(re.finditer(r"\bpragma\b", masked))
    directives = list(re.finditer(r"\bpragma\b[^;]*;", masked))
    if len(pragma_tokens) != len(directives):
        raise CorridorError(
            f"contract source contains an incomplete or obfuscated pragma: {relative}"
        )
    rendered = [source[match.start() : match.end()] for match in directives]
    expected = [SOLIDITY_VERSION_PRAGMA]
    if rendered != expected:
        raise CorridorError(
            "contract source must use exactly one literal "
            f"`{SOLIDITY_VERSION_PRAGMA}` with no experimental or additional pragmas: {relative}"
        )


def _stable_file_identity(
    info: os.stat_result,
) -> tuple[int, int, int, int, int, int, int]:
    """Return fields that expose replacement or in-place mutation of an open file."""

    return (
        info.st_dev,
        info.st_ino,
        info.st_mode,
        info.st_nlink,
        info.st_size,
        info.st_mtime_ns,
        info.st_ctime_ns,
    )


def _read_stable_regular_file(path: Path, maximum_bytes: int, label: str) -> bytes:
    """Read one bounded regular file through a no-follow descriptor exactly once."""

    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise CorridorError(f"{label} must be a readable direct regular file") from error
    try:
        before = os.fstat(descriptor)
        if not stat.S_ISREG(before.st_mode):
            raise CorridorError(f"{label} must be a direct regular file")
        if before.st_size <= 0 or before.st_size > maximum_bytes:
            raise CorridorError(f"{label} is empty or exceeds the bounded size policy")
        chunks: list[bytes] = []
        remaining = maximum_bytes + 1
        while remaining > 0:
            chunk = os.read(descriptor, min(1024 * 1024, remaining))
            if not chunk:
                break
            chunks.append(chunk)
            remaining -= len(chunk)
        payload = b"".join(chunks)
        after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    if _stable_file_identity(before) != _stable_file_identity(after):
        raise CorridorError(f"{label} changed while it was being read")
    if len(payload) != before.st_size or not payload or len(payload) > maximum_bytes:
        raise CorridorError(f"{label} changed outside its bounded size policy")
    return payload


def _read_source(repo_root: Path, relative: str) -> bytes:
    root = repo_root.resolve(strict=True)
    candidate = repo_root / relative
    try:
        info = candidate.lstat()
    except FileNotFoundError as error:
        raise CorridorError(f"required contract source is missing: {relative}") from error
    if stat.S_ISLNK(info.st_mode) or not stat.S_ISREG(info.st_mode):
        raise CorridorError(f"contract source must be a direct regular file: {relative}")
    resolved = candidate.resolve(strict=True)
    try:
        resolved.relative_to(root)
    except ValueError as error:
        raise CorridorError(f"contract source escapes the repository root: {relative}") from error
    if info.st_size <= 0 or info.st_size > MAX_SOURCE_BYTES:
        raise CorridorError(f"contract source is empty or exceeds the size bound: {relative}")
    payload = _read_stable_regular_file(
        candidate,
        MAX_SOURCE_BYTES,
        f"contract source {relative}",
    )
    try:
        source = payload.decode("utf-8")
    except UnicodeDecodeError as error:
        raise CorridorError(f"contract source must be UTF-8: {relative}") from error
    validate_solidity_source_policy(source, relative)
    return payload


def standard_json_input(
    repo_root: Path, config: CorridorConfig, target: str
) -> tuple[Mapping[str, object], list[Mapping[str, object]]]:
    """Construct one root-independent standard-json input and source inventory."""

    if target not in TARGETS:
        raise CorridorError("unknown contract compilation target")
    source_map: dict[str, object] = {}
    inventory: list[Mapping[str, object]] = []
    collision_keys: set[str] = set()
    for relative in config.sources[target]:
        key = _collision_key(relative)
        if key in collision_keys:
            raise CorridorError(f"portable contract source path collision: {relative}")
        collision_keys.add(key)
        payload = _read_source(repo_root, relative)
        source_map[relative] = {"content": payload.decode("utf-8")}
        inventory.append(
            {
                "path": relative,
                "byte_length": len(payload),
                "sha256_hex": sha256_hex(payload),
                "keccak256_hex": keccak256_hex(payload),
            }
        )
    return (
        {
            "language": "Solidity",
            "sources": source_map,
            "settings": config.settings,
        },
        inventory,
    )


def run_soljson(
    compiler_path: Path,
    spec: CompilerSpec,
    compiler_input: bytes,
    node_binary: str,
) -> Mapping[str, object]:
    """Execute the authenticated compiler through the bounded Node ABI adapter."""

    with tempfile.TemporaryDirectory(prefix="iroha-sccp-solc-output-") as temporary:
        output_path = Path(temporary) / "output.json"
        error_path = Path(temporary) / "stderr.txt"
        with output_path.open("xb") as stdout, error_path.open("xb") as stderr:
            try:
                result = subprocess.run(
                    [
                        node_binary,
                        str(SOLJSON_RUNNER),
                        str(compiler_path),
                        spec.sha256,
                        spec.reported_version,
                    ],
                    input=compiler_input,
                    stdout=stdout,
                    stderr=stderr,
                    cwd=ROOT,
                    check=False,
                    timeout=240,
                )
            except (OSError, subprocess.TimeoutExpired) as error:
                raise CorridorError("authenticated compiler runner could not complete") from error
        stderr_bytes = error_path.read_bytes()
        if len(stderr_bytes) > MAX_DIAGNOSTIC_BYTES:
            stderr_bytes = stderr_bytes[:MAX_DIAGNOSTIC_BYTES]
        if result.returncode != 0:
            message = stderr_bytes.decode("utf-8", errors="replace").strip()
            raise CorridorError(message or "authenticated compiler runner failed")
        size = output_path.stat().st_size
        if size <= 0 or size > MAX_COMPILER_OUTPUT_BYTES:
            raise CorridorError("authenticated compiler output exceeds the bounded size policy")
        parsed = _require_object(
            parse_json_bytes(output_path.read_bytes(), "authenticated compiler output"),
            "authenticated compiler output",
        )
    _require_exact_keys(parsed, ("compiler_version", "output"), "compiler runner response")
    if parsed["compiler_version"] != spec.reported_version:
        raise CorridorError("compiler runner reported an unexpected version")
    return _require_object(parsed["output"], "standard-json compiler output")


def _safe_diagnostic(entry: Mapping[str, object]) -> str:
    value = entry.get("formattedMessage", entry.get("message", "compiler diagnostic"))
    if not isinstance(value, str):
        return "compiler diagnostic"
    sanitized = "".join(character if character in "\n\t" or ord(character) >= 0x20 else "?" for character in value)
    return sanitized[:2048]


def _reject_compiler_diagnostics(output: Mapping[str, object], target: str) -> None:
    diagnostics = output.get("errors", [])
    if diagnostics is None:
        diagnostics = []
    if not isinstance(diagnostics, list):
        raise CorridorError(f"{target} compiler diagnostics must be an array")
    rejected: list[str] = []
    for value in diagnostics:
        entry = _require_object(value, f"{target} compiler diagnostic")
        severity = entry.get("severity")
        if severity in ("warning", "error"):
            rejected.append(_safe_diagnostic(entry))
        elif severity not in ("info",):
            raise CorridorError(f"{target} compiler returned an unknown diagnostic severity")
    if rejected:
        joined = "\n".join(rejected)
        raise CorridorError(f"{target} compiler emitted a warning or error:\n{joined}")


def _decode_bytecode(value: object, label: str) -> tuple[str, bytes]:
    if not isinstance(value, str) or len(value) % 2 != 0:
        raise CorridorError(f"{label} must be an even-length hexadecimal string")
    if value != value.lower() or not re.fullmatch(r"[0-9a-f]*", value):
        raise CorridorError(f"{label} contains unresolved links or noncanonical hexadecimal")
    return "0x" + value, bytes.fromhex(value)


def _metadata_record(value: object, label: str, compiler_version: str) -> tuple[object, bytes]:
    if not isinstance(value, str):
        raise CorridorError(f"{label} metadata must be JSON text")
    parsed = parse_json_bytes(value.encode("utf-8"), f"{label} metadata")
    metadata = _require_object(parsed, f"{label} metadata")
    compiler = _require_object(metadata.get("compiler"), f"{label} metadata compiler")
    if compiler.get("version") != compiler_version.replace(".Emscripten.clang", ""):
        raise CorridorError(f"{label} metadata compiler identity mismatch")
    canonical = canonical_json_bytes(metadata)
    return metadata, canonical


def _bytecode_record(bytecode: bytes, encoded: str) -> Mapping[str, object]:
    return {
        "hex": encoded,
        "byte_length": len(bytecode),
        "sha256_hex": sha256_hex(bytecode),
        "keccak256_hex": keccak256_hex(bytecode),
    }


def _runtime_immutable_references(
    value: object, runtime_bytes: int, label: str
) -> list[Mapping[str, object]]:
    """Normalize and bound compiler-reported immutable runtime patches."""

    if value is None:
        value = {}
    references = _require_object(value, f"{label} immutable references")
    normalized: list[Mapping[str, object]] = []
    occupied: list[tuple[int, int]] = []
    for ast_id, locations in references.items():
        if not isinstance(ast_id, str) or not re.fullmatch(r"0|[1-9][0-9]*", ast_id):
            raise CorridorError(f"{label} immutable reference AST id is noncanonical")
        if not isinstance(locations, list) or not locations:
            raise CorridorError(f"{label} immutable reference locations must be nonempty")
        for location_value in locations:
            location = _require_object(location_value, f"{label} immutable reference")
            _require_exact_keys(location, ("start", "length"), f"{label} immutable reference")
            start = location["start"]
            length = location["length"]
            if (
                not isinstance(start, int)
                or isinstance(start, bool)
                or start < 0
                or not isinstance(length, int)
                or isinstance(length, bool)
                or length <= 0
                or start > runtime_bytes
                or length > runtime_bytes - start
            ):
                raise CorridorError(f"{label} immutable reference is outside runtime bytecode")
            occupied.append((start, start + length))
            normalized.append({"ast_id": ast_id, "start": start, "length": length})
    occupied.sort()
    if any(previous[1] > current[0] for previous, current in zip(occupied, occupied[1:])):
        raise CorridorError(f"{label} immutable runtime references overlap")
    normalized.sort(key=lambda entry: (entry["start"], entry["length"], int(entry["ast_id"])))
    return normalized


def _validate_normalized_runtime_immutable_references(
    value: object, runtime_bytes: int, label: str
) -> None:
    if not isinstance(value, list):
        raise CorridorError(f"{label} immutable references must be an array")
    rebuilt: dict[str, list[Mapping[str, object]]] = {}
    for entry_value in value:
        entry = _require_object(entry_value, f"{label} immutable reference")
        _require_exact_keys(
            entry, ("ast_id", "start", "length"), f"{label} immutable reference"
        )
        ast_id = entry.get("ast_id")
        rebuilt.setdefault(ast_id if isinstance(ast_id, str) else "", []).append(
            {"start": entry.get("start"), "length": entry.get("length")}
        )
    if value != _runtime_immutable_references(rebuilt, runtime_bytes, label):
        raise CorridorError(f"{label} immutable references are not canonical")


def _build_contract_records(
    output: Mapping[str, object],
    target: str,
    spec: CompilerSpec,
    limits: Mapping[str, int],
    source_paths: Sequence[str],
) -> list[Mapping[str, object]]:
    contracts = _require_object(output.get("contracts"), f"{target} compiler contracts")
    source_set = set(source_paths)
    records: list[Mapping[str, object]] = []
    fqn_collision_keys: set[str] = set()
    for source_path in sorted(contracts):
        canonical = canonical_source_path(source_path, f"{target} compiler source path")
        if canonical not in source_set:
            raise CorridorError(f"{target} compiler emitted an undeclared source path")
        source_contracts = _require_object(
            contracts[source_path], f"{target} contracts for {source_path}"
        )
        for contract_name in sorted(source_contracts):
            if not IDENTIFIER_RE.fullmatch(contract_name):
                raise CorridorError(f"{target} compiler emitted an invalid contract identifier")
            fully_qualified_name = f"{source_path}:{contract_name}"
            collision_key = _collision_key(fully_qualified_name)
            if collision_key in fqn_collision_keys:
                raise CorridorError(f"{target} compiler output contains a contract path collision")
            fqn_collision_keys.add(collision_key)
            artifact = _require_object(
                source_contracts[contract_name], f"{target} artifact {fully_qualified_name}"
            )
            abi = artifact.get("abi")
            if not isinstance(abi, list):
                raise CorridorError(f"{fully_qualified_name} ABI must be an array")
            evm = _require_object(artifact.get("evm"), f"{fully_qualified_name} EVM output")
            creation = _require_object(
                evm.get("bytecode"), f"{fully_qualified_name} creation bytecode"
            )
            runtime = _require_object(
                evm.get("deployedBytecode"), f"{fully_qualified_name} runtime bytecode"
            )
            for link_label, link_value in (
                ("creation", creation.get("linkReferences")),
                ("runtime", runtime.get("linkReferences")),
            ):
                if link_value not in ({}, None):
                    raise CorridorError(
                        f"{fully_qualified_name} has unresolved {link_label} link references"
                    )
            creation_hex, creation_bytes = _decode_bytecode(
                creation.get("object"), f"{fully_qualified_name} creation bytecode"
            )
            runtime_hex, runtime_bytes = _decode_bytecode(
                runtime.get("object"), f"{fully_qualified_name} runtime bytecode"
            )
            immutable_references = _runtime_immutable_references(
                runtime.get("immutableReferences"),
                len(runtime_bytes),
                fully_qualified_name,
            )
            if len(creation_bytes) > limits["creation_bytecode_bytes"]:
                raise CorridorError(f"{fully_qualified_name} creation bytecode exceeds its ceiling")
            if len(runtime_bytes) > limits["runtime_bytecode_bytes"]:
                raise CorridorError(f"{fully_qualified_name} runtime bytecode exceeds its ceiling")
            metadata, metadata_bytes = _metadata_record(
                artifact.get("metadata"), fully_qualified_name, spec.reported_version
            )
            records.append(
                {
                    "fully_qualified_name": fully_qualified_name,
                    "source_path": source_path,
                    "contract_name": contract_name,
                    "abi": abi,
                    "creation_bytecode": _bytecode_record(creation_bytes, creation_hex),
                    "runtime_bytecode": _bytecode_record(runtime_bytes, runtime_hex),
                    "runtime_immutable_references": immutable_references,
                    "metadata": metadata,
                    "metadata_sha256_hex": sha256_hex(metadata_bytes),
                    "metadata_keccak256_hex": keccak256_hex(metadata_bytes),
                }
            )
    if not records:
        raise CorridorError(f"{target} compiler emitted no contract artifacts")
    return records


def compile_target(
    repo_root: Path,
    config: CorridorConfig,
    target: str,
    compiler_path: Path,
    node_binary: str = "node",
    runner: CompilerRunner = run_soljson,
) -> CompiledTarget:
    """Compile and normalize one target through its exact compiler."""

    standard_input, source_inventory = standard_json_input(repo_root, config, target)
    input_bytes = canonical_json_bytes(standard_input)
    output = runner(compiler_path, config.compilers[target], input_bytes, node_binary)
    _reject_compiler_diagnostics(output, target)
    contracts = _build_contract_records(
        output,
        target,
        config.compilers[target],
        config.size_limits[target],
        config.sources[target],
    )
    manifest: Mapping[str, object] = {
        "target": target,
        "compiler": {
            "identity": config.compilers[target].identity,
            "reported_version": config.compilers[target].reported_version,
            "source_url": config.compilers[target].url,
            "soljson_sha256_hex": config.compilers[target].sha256,
        },
        "settings": config.settings,
        "settings_sha256_hex": sha256_hex(canonical_json_bytes(config.settings)),
        "standard_json_input_sha256_hex": sha256_hex(input_bytes),
        "sources": source_inventory,
        "contracts": contracts,
    }
    return CompiledTarget(
        target=target,
        source_paths=config.sources[target],
        input_sha256=sha256_hex(input_bytes),
        compiler_sha256=config.compilers[target].sha256,
        raw_output=output,
        manifest=manifest,
    )


def validate_distinct_targets(evm: CompiledTarget, tron: CompiledTarget) -> None:
    """Reject aliased EVM/TVM inputs, compilers, outputs, and manifests."""

    if evm.target != "evm" or tron.target != "tron":
        raise CorridorError("compiled target roles are reversed or missing")
    evm_contracts = evm.raw_output.get("contracts")
    tron_contracts = tron.raw_output.get("contracts")
    if evm.raw_output is tron.raw_output or evm_contracts is tron_contracts:
        raise CorridorError("EVM and TVM compiler output maps are aliased")
    if evm.source_paths == tron.source_paths or evm.input_sha256 == tron.input_sha256:
        raise CorridorError("EVM and TVM compiler input maps are not distinct")
    if evm.compiler_sha256 == tron.compiler_sha256:
        raise CorridorError("EVM and TVM compiler identities are aliased")
    if canonical_json_bytes(evm_contracts) == canonical_json_bytes(tron_contracts):
        raise CorridorError("EVM and TVM compiler contract outputs are indistinguishable")
    if evm.manifest is tron.manifest or canonical_json_bytes(evm.manifest) == canonical_json_bytes(
        tron.manifest
    ):
        raise CorridorError("EVM and TVM normalized artifact maps are aliased")


def compile_corridor(
    repo_root: Path,
    config: CorridorConfig,
    node_binary: str = "node",
    fetcher: CompilerFetcher = _network_fetch,
    runner: CompilerRunner = run_soljson,
) -> Mapping[str, object]:
    """Compile both targets using separate authenticated compiler processes."""

    with tempfile.TemporaryDirectory(prefix="iroha-sccp-authenticated-compilers-") as temporary:
        temp_root = Path(temporary)
        os.chmod(temp_root, 0o700)
        compiled: dict[str, CompiledTarget] = {}
        for target in TARGETS:
            compiler_path = materialize_verified_compiler(
                config.compilers[target], temp_root / f"{target}-soljson.js", fetcher
            )
            compiled[target] = compile_target(
                repo_root,
                config,
                target,
                compiler_path,
                node_binary,
                runner,
            )
        validate_distinct_targets(compiled["evm"], compiled["tron"])
    return {
        "schema": MANIFEST_SCHEMA,
        "compiler_lock_sha256_hex": config.canonical_sha256,
        "targets": {
            target: compiled[target].manifest for target in TARGETS
        },
    }


def artifact_lock_from_manifest(manifest: Mapping[str, object]) -> Mapping[str, object]:
    """Derive the reviewable size and full-manifest digest lock."""

    targets = _require_object(manifest.get("targets"), "corridor manifest targets")
    locked_targets: dict[str, object] = {}
    for target in TARGETS:
        target_manifest = _require_object(targets.get(target), f"{target} target manifest")
        contracts = target_manifest.get("contracts")
        if not isinstance(contracts, list):
            raise CorridorError(f"{target} target contracts must be an array")
        sizes: dict[str, object] = {}
        for value in contracts:
            contract = _require_object(value, f"{target} contract artifact")
            fqn = _require_string(contract.get("fully_qualified_name"), "contract FQN")
            creation = _require_object(contract.get("creation_bytecode"), f"{fqn} creation")
            runtime = _require_object(contract.get("runtime_bytecode"), f"{fqn} runtime")
            sizes[fqn] = {
                "creation_bytecode_bytes": creation.get("byte_length"),
                "runtime_bytecode_bytes": runtime.get("byte_length"),
            }
        locked_targets[target] = {
            "standard_json_input_sha256_hex": target_manifest.get(
                "standard_json_input_sha256_hex"
            ),
            "contract_sizes": sizes,
        }
    return {
        "schema": ARTIFACT_LOCK_SCHEMA,
        "compiler_lock_sha256_hex": manifest.get("compiler_lock_sha256_hex"),
        "targets": locked_targets,
        "corridor_manifest_sha256_hex": sha256_hex(canonical_json_bytes(manifest)),
    }


def validate_artifact_lock(
    manifest: Mapping[str, object], artifact_lock: Mapping[str, object]
) -> None:
    """Fail closed on compiler input, per-contract size, or artifact digest drift."""

    _require_exact_keys(
        artifact_lock,
        ("schema", "compiler_lock_sha256_hex", "targets", "corridor_manifest_sha256_hex"),
        "artifact lock",
    )
    if artifact_lock["schema"] != ARTIFACT_LOCK_SCHEMA:
        raise CorridorError("artifact lock schema is unsupported")
    if artifact_lock["compiler_lock_sha256_hex"] != manifest.get("compiler_lock_sha256_hex"):
        raise CorridorError("compiler lock digest drift")
    expected_targets = _require_object(artifact_lock["targets"], "artifact lock targets")
    actual_targets = _require_object(manifest.get("targets"), "manifest targets")
    if set(expected_targets) != set(TARGETS) or set(actual_targets) != set(TARGETS):
        raise CorridorError("artifact lock and manifest must contain exact EVM and TRON targets")
    for target in TARGETS:
        expected = _require_object(expected_targets[target], f"{target} artifact lock")
        _require_exact_keys(
            expected,
            ("standard_json_input_sha256_hex", "contract_sizes"),
            f"{target} artifact lock",
        )
        actual = _require_object(actual_targets[target], f"{target} manifest")
        if expected["standard_json_input_sha256_hex"] != actual.get(
            "standard_json_input_sha256_hex"
        ):
            raise CorridorError(f"{target} compiler input/settings digest drift")
        contracts = actual.get("contracts")
        if not isinstance(contracts, list):
            raise CorridorError(f"{target} manifest contracts must be an array")
        actual_sizes: dict[str, object] = {}
        for value in contracts:
            contract = _require_object(value, f"{target} manifest contract")
            fqn = _require_string(contract.get("fully_qualified_name"), "contract FQN")
            creation = _require_object(contract.get("creation_bytecode"), f"{fqn} creation")
            runtime = _require_object(contract.get("runtime_bytecode"), f"{fqn} runtime")
            actual_sizes[fqn] = {
                "creation_bytecode_bytes": creation.get("byte_length"),
                "runtime_bytecode_bytes": runtime.get("byte_length"),
            }
        if expected["contract_sizes"] != actual_sizes:
            raise CorridorError(f"{target} contract bytecode size drift")
    digest = artifact_lock["corridor_manifest_sha256_hex"]
    if not isinstance(digest, str) or not HEX_32_RE.fullmatch(digest):
        raise CorridorError("artifact lock manifest digest is malformed")
    if digest != sha256_hex(canonical_json_bytes(manifest)):
        raise CorridorError("corridor artifact digest drift")


def _validate_hash_record(record: Mapping[str, object], label: str) -> None:
    _require_exact_keys(
        record,
        ("hex", "byte_length", "sha256_hex", "keccak256_hex"),
        label,
    )
    encoded = record.get("hex")
    if not isinstance(encoded, str) or not encoded.startswith("0x"):
        raise CorridorError(f"{label} hex is malformed")
    canonical, payload = _decode_bytecode(encoded[2:], label)
    if canonical != encoded or record.get("byte_length") != len(payload):
        raise CorridorError(f"{label} byte length is inconsistent")
    if record.get("sha256_hex") != sha256_hex(payload):
        raise CorridorError(f"{label} SHA-256 is inconsistent")
    if record.get("keccak256_hex") != keccak256_hex(payload):
        raise CorridorError(f"{label} Keccak-256 is inconsistent")


def validate_manifest_integrity(manifest: Mapping[str, object], config: CorridorConfig) -> None:
    """Recompute every embedded artifact hash before a runtime consumes the manifest."""

    _require_exact_keys(manifest, ("schema", "compiler_lock_sha256_hex", "targets"), "manifest")
    if manifest["schema"] != MANIFEST_SCHEMA:
        raise CorridorError("contract artifact manifest schema is unsupported")
    if manifest["compiler_lock_sha256_hex"] != config.canonical_sha256:
        raise CorridorError("contract artifact manifest compiler lock digest mismatch")
    targets = _require_object(manifest["targets"], "manifest targets")
    if set(targets) != set(TARGETS):
        raise CorridorError("manifest must contain distinct EVM and TRON targets")
    target_contract_digests: dict[str, str] = {}
    for target in TARGETS:
        value = _require_object(targets[target], f"{target} target manifest")
        compiler = _require_object(value.get("compiler"), f"{target} compiler identity")
        if compiler.get("soljson_sha256_hex") != config.compilers[target].sha256:
            raise CorridorError(f"{target} compiler digest mismatch")
        if compiler.get("reported_version") != config.compilers[target].reported_version:
            raise CorridorError(f"{target} compiler version mismatch")
        if value.get("settings_sha256_hex") != sha256_hex(canonical_json_bytes(value.get("settings"))):
            raise CorridorError(f"{target} compiler settings digest mismatch")
        contracts = value.get("contracts")
        if not isinstance(contracts, list) or not contracts:
            raise CorridorError(f"{target} contracts must be a nonempty array")
        seen: set[str] = set()
        for artifact_value in contracts:
            artifact = _require_object(artifact_value, f"{target} contract artifact")
            _require_exact_keys(
                artifact,
                (
                    "fully_qualified_name",
                    "source_path",
                    "contract_name",
                    "abi",
                    "creation_bytecode",
                    "runtime_bytecode",
                    "runtime_immutable_references",
                    "metadata",
                    "metadata_sha256_hex",
                    "metadata_keccak256_hex",
                ),
                f"{target} contract artifact",
            )
            fqn = _require_string(artifact.get("fully_qualified_name"), "contract FQN")
            key = _collision_key(fqn)
            if key in seen:
                raise CorridorError(f"{target} manifest contains a contract path collision")
            seen.add(key)
            _validate_hash_record(
                _require_object(artifact.get("creation_bytecode"), f"{fqn} creation"),
                f"{fqn} creation bytecode",
            )
            _validate_hash_record(
                _require_object(artifact.get("runtime_bytecode"), f"{fqn} runtime"),
                f"{fqn} runtime bytecode",
            )
            runtime_record = _require_object(
                artifact.get("runtime_bytecode"), f"{fqn} runtime"
            )
            runtime_length = runtime_record.get("byte_length")
            if not isinstance(runtime_length, int) or isinstance(runtime_length, bool):
                raise CorridorError(f"{fqn} runtime byte length is malformed")
            _validate_normalized_runtime_immutable_references(
                artifact.get("runtime_immutable_references"), runtime_length, fqn
            )
            metadata = artifact.get("metadata")
            metadata_bytes = canonical_json_bytes(metadata)
            if artifact.get("metadata_sha256_hex") != sha256_hex(metadata_bytes):
                raise CorridorError(f"{fqn} metadata SHA-256 mismatch")
            if artifact.get("metadata_keccak256_hex") != keccak256_hex(metadata_bytes):
                raise CorridorError(f"{fqn} metadata Keccak-256 mismatch")
        target_contract_digests[target] = sha256_hex(canonical_json_bytes(contracts))
    if target_contract_digests["evm"] == target_contract_digests["tron"]:
        raise CorridorError("EVM and TVM artifact maps are aliased")


def validate_manifest_source_inputs(
    manifest: Mapping[str, object], config: CorridorConfig, repo_root: Path
) -> None:
    """Bind a published manifest to the checkout sources consumed by a source smoke."""

    targets = _require_object(manifest.get("targets"), "manifest targets")
    if set(targets) != set(TARGETS):
        raise CorridorError("manifest must contain exact EVM and TRON targets")
    for target in TARGETS:
        target_manifest = _require_object(targets[target], f"{target} target manifest")
        standard_input, source_inventory = standard_json_input(repo_root, config, target)
        current_digest = sha256_hex(canonical_json_bytes(standard_input))
        if target_manifest.get("standard_json_input_sha256_hex") != current_digest:
            raise CorridorError(f"{target} manifest is stale for the current source input")
        if target_manifest.get("sources") != source_inventory:
            raise CorridorError(f"{target} manifest source inventory drift")


def load_artifact_lock(path: Path = DEFAULT_ARTIFACT_LOCK) -> Mapping[str, object]:
    return _require_object(parse_json_bytes(path.read_bytes(), "artifact lock"), "artifact lock")


def load_manifest(path: Path) -> Mapping[str, object]:
    payload = _read_stable_regular_file(
        path,
        MAX_COMPILER_OUTPUT_BYTES,
        "contract artifact manifest",
    )
    return _require_object(parse_json_bytes(payload, "contract artifact manifest"), "manifest")


def snapshot_runtime_inputs(
    manifest_path: Path,
    native_vectors_path: Path,
    output_dir: Path,
) -> tuple[Path, Path]:
    """Publish one private, read-only snapshot of all TVM runtime inputs."""

    if output_dir.exists() or output_dir.is_symlink():
        raise CorridorError("runtime-input snapshot output must not already exist")
    parent = output_dir.parent
    try:
        parent_info = parent.lstat()
    except OSError as error:
        raise CorridorError("runtime-input snapshot parent is unavailable") from error
    if (
        stat.S_ISLNK(parent_info.st_mode)
        or not stat.S_ISDIR(parent_info.st_mode)
        or parent_info.st_uid != os.geteuid()
        or stat.S_IMODE(parent_info.st_mode) & 0o077
    ):
        raise CorridorError("runtime-input snapshot parent must be an owned private directory")

    manifest_payload = _read_stable_regular_file(
        manifest_path,
        MAX_COMPILER_OUTPUT_BYTES,
        "contract artifact manifest",
    )
    vector_payload = _read_stable_regular_file(
        native_vectors_path,
        MAX_SOURCE_BYTES,
        "native transfer vectors",
    )
    # Parse before publication so the locked snapshot never contains malformed input.
    _require_object(
        parse_json_bytes(manifest_payload, "contract artifact manifest"),
        "manifest",
    )
    _require_object(parse_json_bytes(vector_payload, "native transfer vectors"), "vectors")

    os.mkdir(output_dir, 0o700)
    outputs = (
        (output_dir / MANIFEST_NAME, manifest_payload),
        (output_dir / NATIVE_VECTORS_NAME, vector_payload),
    )
    try:
        for destination, payload in outputs:
            descriptor = os.open(
                destination,
                os.O_WRONLY
                | os.O_CREAT
                | os.O_EXCL
                | getattr(os, "O_CLOEXEC", 0)
                | getattr(os, "O_NOFOLLOW", 0),
                0o600,
            )
            with os.fdopen(descriptor, "wb") as output:
                output.write(payload)
                output.flush()
                os.fsync(output.fileno())
            os.chmod(destination, 0o400)
        directory_descriptor = os.open(
            output_dir,
            os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_CLOEXEC", 0),
        )
        try:
            os.fsync(directory_descriptor)
        finally:
            os.close(directory_descriptor)
        os.chmod(output_dir, 0o500)
        for destination, payload in outputs:
            if _read_stable_regular_file(
                destination,
                MAX_COMPILER_OUTPUT_BYTES,
                f"runtime-input snapshot {destination.name}",
            ) != payload:
                raise CorridorError("runtime-input snapshot changed during publication")
        directory_descriptor = os.open(
            output_dir,
            os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_CLOEXEC", 0),
        )
        try:
            os.fsync(directory_descriptor)
        finally:
            os.close(directory_descriptor)
    except BaseException:
        os.chmod(output_dir, 0o700)
        for destination, _payload in outputs:
            destination.unlink(missing_ok=True)
        output_dir.rmdir()
        raise
    return outputs[0][0], outputs[1][0]


def write_canonical_file(path: Path, value: object) -> None:
    """Write one canonical JSON file without following an existing path."""

    if path.exists() or path.is_symlink():
        raise CorridorError(f"refusing output path collision: {path}")
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o644)
    with os.fdopen(descriptor, "wb") as output:
        output.write(canonical_json_bytes(value) + b"\n")
        output.flush()
        os.fsync(output.fileno())


def publish_manifest(output_dir: Path, manifest: Mapping[str, object]) -> Path:
    """Atomically publish one manifest into a new or empty output directory."""

    if output_dir.is_symlink() or (output_dir.exists() and not output_dir.is_dir()):
        raise CorridorError("manifest output directory collides with a non-directory path")
    if output_dir.exists() and any(output_dir.iterdir()):
        raise CorridorError("manifest output directory must be empty")
    parent = output_dir.parent
    parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".sccp-contract-artifacts-", dir=parent) as staging_text:
        staging = Path(staging_text)
        write_canonical_file(staging / MANIFEST_NAME, manifest)
        if output_dir.exists():
            output_dir.rmdir()
        os.replace(staging, output_dir)
    return output_dir / MANIFEST_NAME


def build_and_validate(
    repo_root: Path,
    compiler_lock_path: Path,
    artifact_lock_path: Path,
    node_binary: str,
    fetcher: CompilerFetcher = _network_fetch,
    runner: CompilerRunner = run_soljson,
) -> tuple[Mapping[str, object], CorridorConfig]:
    config = load_corridor_config(compiler_lock_path)
    manifest = compile_corridor(repo_root, config, node_binary, fetcher, runner)
    validate_manifest_integrity(manifest, config)
    validate_artifact_lock(manifest, load_artifact_lock(artifact_lock_path))
    return manifest, config


def _compile_for_lock(args: argparse.Namespace) -> None:
    config = load_corridor_config(args.compiler_lock)
    manifest = compile_corridor(args.repo_root, config, args.node)
    validate_manifest_integrity(manifest, config)
    lock = artifact_lock_from_manifest(manifest)
    write_canonical_file(args.output, lock)
    if args.manifest_output is not None:
        write_canonical_file(args.manifest_output, manifest)
    print(f"wrote reviewed artifact lock: {args.output}")


def _build_command(args: argparse.Namespace) -> None:
    manifest, _ = build_and_validate(
        args.repo_root,
        args.compiler_lock,
        args.artifact_lock,
        args.node,
    )
    output = publish_manifest(args.output_dir, manifest)
    print(f"wrote authenticated SCCP contract manifest: {output}")


def _verify_command(args: argparse.Namespace) -> None:
    config = load_corridor_config(args.compiler_lock)
    manifest = load_manifest(args.manifest)
    validate_manifest_integrity(manifest, config)
    validate_artifact_lock(manifest, load_artifact_lock(args.artifact_lock))
    if args.check_source_inputs:
        validate_manifest_source_inputs(manifest, config, args.repo_root)
    print(f"verified authenticated SCCP contract manifest: {args.manifest}")


def _snapshot_command(args: argparse.Namespace) -> None:
    manifest, vectors = snapshot_runtime_inputs(
        args.manifest,
        args.native_vectors,
        args.output_dir,
    )
    print(f"snapshotted authenticated SCCP contract manifest: {manifest}")
    print(f"snapshotted Rust-generated SCCP native vectors: {vectors}")


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(
        description="Compile SCCP EVM and TVM contracts with authenticated soljson artifacts."
    )
    subcommands = result.add_subparsers(dest="command", required=True)

    build = subcommands.add_parser("build", help="compile, verify, and publish the locked manifest")
    build.add_argument("--repo-root", type=Path, default=ROOT)
    build.add_argument("--compiler-lock", type=Path, default=DEFAULT_COMPILER_LOCK)
    build.add_argument("--artifact-lock", type=Path, default=DEFAULT_ARTIFACT_LOCK)
    build.add_argument("--output-dir", type=Path, required=True)
    build.add_argument("--node", default="node")
    build.set_defaults(handler=_build_command)

    lock = subcommands.add_parser(
        "lock", help="explicitly regenerate the reviewed size/digest lock after contract review"
    )
    lock.add_argument("--repo-root", type=Path, default=ROOT)
    lock.add_argument("--compiler-lock", type=Path, default=DEFAULT_COMPILER_LOCK)
    lock.add_argument("--output", type=Path, required=True)
    lock.add_argument("--manifest-output", type=Path)
    lock.add_argument("--node", default="node")
    lock.set_defaults(handler=_compile_for_lock)

    verify = subcommands.add_parser("verify", help="verify one published manifest and its lock")
    verify.add_argument("--manifest", type=Path, required=True)
    verify.add_argument("--compiler-lock", type=Path, default=DEFAULT_COMPILER_LOCK)
    verify.add_argument("--artifact-lock", type=Path, default=DEFAULT_ARTIFACT_LOCK)
    verify.add_argument("--repo-root", type=Path, default=ROOT)
    verify.add_argument(
        "--check-source-inputs",
        action="store_true",
        help="require the manifest standard-json hashes to match the current checkout",
    )
    verify.set_defaults(handler=_verify_command)

    snapshot = subcommands.add_parser(
        "snapshot",
        help="copy TVM runtime inputs once into a private read-only directory",
    )
    snapshot.add_argument("--manifest", type=Path, required=True)
    snapshot.add_argument("--native-vectors", type=Path, required=True)
    snapshot.add_argument("--output-dir", type=Path, required=True)
    snapshot.set_defaults(handler=_snapshot_command)

    return result


def main(argv: Sequence[str] | None = None) -> int:
    args = parser().parse_args(argv)
    try:
        args.handler(args)
    except (CorridorError, OSError) as error:
        print(f"SCCP contract artifact corridor failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
