#!/usr/bin/env python3
"""Fail closed on the IVM vector/Metal source compaction.

The guard retains immutable evidence for the reviewed source compaction and
checks the final V1 embedded-kernel, process-owner and quarantine behavior.
Unchanged buffer mappings, CPU algorithms and fallback order remain sealed;
production completion receipts are checked separately from startup self-tests.
It intentionally uses only the Python standard library.
"""

from __future__ import annotations

import hashlib
import re
import subprocess
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SOURCE_PATH = ROOT / "crates/ivm/src/vector.rs"

PREIMAGE_BLOB = "8eb0aad38a0221393f5bae880e80b5bdf2393909"
PREIMAGE_SHA256 = "e716509f6cadc791b5451d6361b492a33379279d58493319dd9c1389fc54c8c5"
PREIMAGE_LINES = 5_179
PREIMAGE_AST_NODES = 57_517
PREIMAGE_NAMED_AST_NODES = 31_361
PREIMAGE_LEAF_TOKENS = 39_726
PREIMAGE_AST_ERRORS = 1

POSTIMAGE_BLOB = "f7f125e6111e13c1f8cdef4654939f68f5ff5f81"
POSTIMAGE_SHA256 = "c330b54b6a7ae3b233737f296ae5dee6bd412f0988d17d72046b62d8954760bc"
POSTIMAGE_LINES = 3_997
# Parser-work counts were measured when the compaction landed; the later
# SHA-256 reference extraction into `sha256_ref.rs` only lowers them further.
POSTIMAGE_AST_NODES = 47_036
POSTIMAGE_NAMED_AST_NODES = 25_218
POSTIMAGE_LEAF_TOKENS = 32_686
POSTIMAGE_AST_ERRORS = 1
MINIMUM_LINE_REDUCTION = 1_000
MINIMUM_AST_NODE_REDUCTION = 10_000

# The historical AST counts above describe POSTIMAGE_BLOB only. They are not
# qualification or compiler-work evidence for the subsequently expanded V1 code.
CURRENT_SHA256 = "b03ad2f8c3a2b14f04c7bb01a24e3dfa1fa61adee67bba88feeb82bdbd4cf2ed"
CURRENT_LINES = 4262
CURRENT_PUBLIC_API_SHA256 = "9234fa228c5872a112cd10b69c1cb2108e83df8e521b616838cee6ce97c486f5"
CURRENT_TEST_SUFFIX_SHA256 = "e52ddbf6e279dce76098500ffe7e54928ddf09ed8de7c19bca4754ae115f1fcc"
CURRENT_INCLUDE_PATHS = ("../metal/v1/ivm_kernels.metallib",)
CURRENT_CFG_COUNT = 179

EXPECTED_CFG_COUNT = 160
EXPECTED_TARGET_FEATURE_COUNT = 3
EXPECTED_MAX_LINE_LENGTH = 264
EXPECTED_MAX_SEMICOLONS_PER_LINE = 3
EXPECTED_PUBLIC_API_SHA256 = (
    "d299bb4e9bedd31abb8cf0d622f1e264a8b064bfa4dc825eede170c30c116817"
)
EXPECTED_INCLUDE_SHA256 = (
    "1552f8955e965b93fd7dfc67b6347eaae367203a6e2af1ba6d0cf463ec4719ea"
)
EXPECTED_TEST_SUFFIX_SHA256 = (
    "cc1e9ceae84cec2e29bd62792f846f0a2ef5a16c9a5a6d0b67854a34cd2e9e47"
)
EXPECTED_INPUT_CALL_SHA256 = (
    "d71014bc72e27cfaa719728a83db7818a84b3d71dbae9005084722e00510365a"
)
EXPECTED_OUTPUT_CALL_SHA256 = (
    "cc1dda21c5a7bf5ac06dbbbd1467a999824dabc56e8144c3d00d92eeaf40b379"
)
EXPECTED_DISPATCH_CALL_SHA256 = (
    "60e1832cf8ad1157c12bb26e6dbe9be200cff59b7317e9bb405414de92604ce4"
)

EXPECTED_TESTS = (
    (
        "completed_metal_work_is_rejected_after_owner_quarantine",
        ('#[cfg(all(target_os = "macos", feature = "metal"))]', "#[test]"),
    ),
    ("fixed_width_operations_skip_gpu_launch", ("#[test]",)),
    (
        "metal_sha256_merkle_helpers_return_none_without_metal_feature",
        ('#[cfg(not(all(target_os = "macos", feature = "metal")))]', "#[test]"),
    ),
    (
        "metal_sha256_leaves_matches_cpu",
        ('#[cfg(all(target_os = "macos", feature = "metal"))]', "#[test]"),
    ),
    (
        "metal_sha256_pairs_reduce_matches_cpu",
        ('#[cfg(all(target_os = "macos", feature = "metal"))]', "#[test]"),
    ),
    (
        "metal_merkle_root_matches_canonical_hash_markers",
        ('#[cfg(all(target_os = "macos", feature = "metal"))]', "#[test]"),
    ),
    (
        "metal_ed25519_batch_matches_cpu",
        ('#[cfg(all(target_os = "macos", feature = "metal"))]', "#[test]"),
    ),
    (
        "test_metal_sha256_compress_returns_true",
        ('#[cfg(target_os = "macos")]', "#[test]"),
    ),
    (
        "metal_vadd32_single_vector_matches_scalar",
        ('#[cfg(all(target_os = "macos", feature = "metal"))]', "#[test]"),
    ),
    (
        "metal_vadd64_single_vector_matches_scalar",
        ('#[cfg(all(target_os = "macos", feature = "metal"))]', "#[test]"),
    ),
    (
        "metal_bitwise_single_vector_matches_scalar",
        ('#[cfg(all(target_os = "macos", feature = "metal"))]', "#[test]"),
    ),
    (
        "metal_aes_batch_matches_scalar",
        ('#[cfg(all(target_os = "macos", feature = "metal"))]', "#[test]"),
    ),
    (
        "metal_aes_rounds_batch_matches_scalar",
        ('#[cfg(all(target_os = "macos", feature = "metal"))]', "#[test]"),
    ),
    (
        "device_selector_prefers_non_headless_perf_device",
        ('#[cfg(all(target_os = "macos", feature = "metal"))]', "#[test]"),
    ),
    (
        "device_selector_falls_back_to_first_non_headless",
        ('#[cfg(all(target_os = "macos", feature = "metal"))]', "#[test]"),
    ),
    (
        "device_selector_handles_all_headless_devices",
        ('#[cfg(all(target_os = "macos", feature = "metal"))]', "#[test]"),
    ),
    (
        "warm_up_metal_is_noop_on_non_metal_targets",
        ('#[cfg(not(all(target_os = "macos", feature = "metal")))]', "#[test]"),
    ),
    (
        "warm_up_metal_reuses_cached_state",
        ('#[cfg(all(target_os = "macos", feature = "metal"))]', "#[test]"),
    ),
)

EXPECTED_DISPATCH_CALLS = (
    ("&queue", "&vadd32", "&[&buf_a,&buf_b,&buf_out]", "1", "1", '"metalself-testvadd32"'),
    ("&queue", "&vadd64", "&[&buf_a,&buf_b,&buf_out]", "1", "1", '"metalself-testvadd64"'),
    ("&queue", "pipeline", "&[&buf_lhs,&buf_rhs,&buf_out]", "1", "1", "label"),
    ("&queue", "&sha256", "&[&buf_state,&buf_block]", "1", "1", '"metalself-testsha256"'),
    ("&queue", "&sha256_leaves", "&[&buf_blocks,&buf_out]", "blocks.len()asNSUInteger", "1", '"metalself-testsha256leaves"'),
    ("&queue", "&aesenc", "&[&buf_s,&buf_k,&buf_out]", "1", "1", '"metalself-testaesenc"'),
    ("&queue", "&aesdec", "&[&buf_s2,&buf_k,&buf_out2]", "1", "1", '"metalself-testaesdec"'),
    ("&queue", "pipeline", "&[&buf_states,&buf_rk,&buf_out]", "states.len()asNSUInteger", "1", "label"),
    ("&queue", "&aesenc_rounds", "&[&buf_states,&buf_rks,&buf_out,&buf_n]", "1", "1", '"metalself-testaesencrounds"'),
    ("&queue", "&keccak", "&[&buf_state]", "1", "1", '"metalself-testkeccak"'),
    ("&queue", "&sha256_pairs", "&[&in_buf,&out_buf]", "pairsasNSUInteger", "1", '"metalself-testsha256pairs"'),
    ("&queue", "pipeline", "&[&buf_sigs,&buf_pks,&buf_hrams,&buf_count,&buf_out]", "sigs.len()asNSUInteger", "pipeline.threadExecutionWidth()", '"metalself-tested25519batch"'),
    ("&ctx.queue", "&ctx.vadd64", "&[&buf_a,&buf_b,&buf_out]", "1", "1", '"metalvadd64"'),
    ("&ctx.queue", "&ctx.vadd32", "&[&buf_a,&buf_b,&buf_out]", "1", "1", '"metalvadd32"'),
    ("&ctx.queue", "pipeline", "&[&buf_a,&buf_b,&buf_out]", "1", "1", '"metalvectorbitpipeline"'),
    ("&ctx.queue", "&ctx.sha256", "&[&buf_state,&buf_block]", "1", "1", '"metalsha256compress"'),
    ("&ctx.queue", "&ctx.sha256_leaves", "&[&buf_blocks,&buf_out]", "nasNSUInteger", "1", '"metalsha256leaves"'),
    ("&ctx.queue", "&ctx.sha256_pairs", "&[&in_buf,&out_buf]", "pairsasNSUInteger", "1", '"metalsha256pairsreduce"'),
    ("&ctx.queue", "pipeline", "&[&buf_sigs,&buf_pks,&buf_hrams,&buf_count,&buf_out]", "nasNSUInteger", "pipeline.threadExecutionWidth().max(1)", '"metaled25519batchverify"'),
    ("&queue", "&pipeline", "&[&buf_sigs,&buf_pks,&buf_hrams,&buf_count,&buf_out]", "nasNSUInteger", "pipeline.threadExecutionWidth().max(1)", '"metaled25519batchverifydirect"'),
    ("&queue", "&pipeline", "&[&buf_sigs,&buf_pks,&buf_hrams,&buf_count,&buf_out]", "nasNSUInteger", "pipeline.threadExecutionWidth().max(1)", '"metaled25519checkbytesdirect"'),
    ("&queue", "&pipeline", "&[&buf_inputs,&buf_count,&buf_out]", "nasNSUInteger", "pipeline.threadExecutionWidth().max(1)", '"metaled25519fieldroundtripdirect"'),
    ("&queue", "&pipeline", "&[&buf_inputs,&buf_count,&buf_status,&buf_out]", "nasNSUInteger", "pipeline.threadExecutionWidth().max(1)", '"metaled25519pointdecompressdirect"'),
    ("&ctx.queue", "&ctx.aesenc", "&[&buf_s,&buf_k,&buf_out]", "1", "1", '"metalaesencround"'),
    ("&ctx.queue", "&ctx.aesdec", "&[&buf_s,&buf_k,&buf_out]", "1", "1", '"metalaesdecround"'),
    ("&ctx.queue", "&ctx.keccak", "&[&buf]", "1", "1", '"metalkeccak_f1600"'),
    ("&ctx.queue", "&ctx.aesenc_batch", "&[&buf_states,&buf_rk,&buf_out]", "states.len()asNSUInteger", "1", '"metalaesencbatch"'),
    ("&ctx.queue", "&ctx.aesdec_batch", "&[&buf_states,&buf_rk,&buf_out]", "states.len()asNSUInteger", "1", '"metalaesdecbatch"'),
    ("&ctx.queue", "&ctx.aesenc_rounds", "&[&buf_states,&buf_rks,&buf_out,&buf_n]", "states.len()asNSUInteger", "1", '"metalaesencroundsbatch"'),
    ("&ctx.queue", "&ctx.aesdec_rounds", "&[&buf_states,&buf_rks,&buf_out,&buf_n]", "states.len()asNSUInteger", "1", '"metalaesdecroundsbatch"'),
)

# None denotes startup self-tests or explicit test-only diagnostic kernels.
# "receipt" is supplied by public wrappers and is None during calibration.
EXPECTED_DISPATCH_RECEIPTS = ('None', 'None', 'None', 'None', 'None', 'None', 'None', 'None', 'None', 'None', 'None', 'None', 'Some(MetalKernel::Add64)', 'Some(MetalKernel::Add32)', 'Some(kernel)', 'Some(MetalKernel::Sha256)', 'receipt', 'receipt', 'receipt', 'None', 'None', 'None', 'None', 'Some(MetalKernel::AesEnc)', 'Some(MetalKernel::AesDec)', 'Some(MetalKernel::Keccak)', 'receipt', 'receipt', 'receipt', 'receipt')

EXPECTED_OUTPUT_CALLS = (
    ("&device", "a.len()*core::mem::size_of::<u32>()"),
    ("&device", "a.len()*core::mem::size_of::<u64>()"),
    ("&device", "lhs.len()*core::mem::size_of::<u32>()"),
    ("&device", "blocks.len()*8*core::mem::size_of::<u32>()"),
    ("&device", "16"),
    ("&device", "16"),
    ("&device", "flat_states.len()"),
    ("&device", "16"),
    ("&device", "16"),
    ("&device", "pairs*8*core::mem::size_of::<u32>()"),
    ("&device", "sigs.len()"),
    ("&ctx.device", "in_a.len()*core::mem::size_of::<u64>()"),
    ("&ctx.device", "a.len()*core::mem::size_of::<u32>()"),
    ("&ctx.device", "a.len()*core::mem::size_of::<u32>()"),
    ("&ctx.device", "n*8*core::mem::size_of::<u32>()"),
    ("&ctx.device", "pairs*8*core::mem::size_of::<u32>()"),
    ("&ctx.device", "n"),
    ("&device", "n"),
    ("&device", "n*32"),
    ("&device", "n*32"),
    ("&device", "n"),
    ("&device", "n*32"),
    ("&ctx.device", "16"),
    ("&ctx.device", "16"),
    ("&ctx.device", "out.len()"),
    ("&ctx.device", "out.len()"),
    ("&ctx.device", "out.len()"),
    ("&ctx.device", "out.len()"),
)

EXPECTED_EXTERNAL_CALL_SITES = {
    "vadd32": (
        "crates/irohad/src/main.rs",
        "crates/ivm/benches/bench_vector.rs",
        "crates/ivm/src/ivm.rs",
        "crates/ivm/tests/acceleration_simd.rs",
        "crates/ivm/tests/crypto_vectors.rs",
        "crates/ivm/tests/cuda.rs",
        "crates/ivm/tests/metal_disable_on_mismatch.rs",
        "crates/ivm/tests/simd_tail_misalignment.rs",
    ),
    "vadd64": (
        "crates/ivm/src/ivm.rs",
        "crates/ivm/tests/cuda.rs",
        "crates/ivm/tests/simd_tail_misalignment.rs",
    ),
    "vand": (
        "crates/ivm/src/ivm.rs",
        "crates/ivm/tests/cuda.rs",
        "crates/ivm/tests/vector_ops.rs",
    ),
    "vxor": (
        "crates/ivm/src/ivm.rs",
        "crates/ivm/tests/cuda.rs",
        "crates/ivm/tests/vector_ops.rs",
    ),
    "vor": (
        "crates/ivm/src/ivm.rs",
        "crates/ivm/tests/cuda.rs",
        "crates/ivm/tests/vector_ops.rs",
    ),
    "vrot32": (
        "crates/ivm/benches/bench_vector.rs",
        "crates/ivm/src/ivm.rs",
        "crates/ivm/tests/simd_tail_misalignment.rs",
    ),
}

EXPECTED_SEAM_COUNTS = {
    "fn_bounds": 2,
    "function_pointers": 1,
    "closures": 119,
    "macro_definitions": 0,
    "action_types": 0,
    "path_relocations": 0,
    "rustfmt_skips": 0,
    "unsafe_functions": 3,
}


class GuardError(AssertionError):
    """The vector/Metal source no longer matches its audited contract."""


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise GuardError(message)


def _sha256(value: object) -> str:
    payload = value if isinstance(value, str) else repr(value)
    return hashlib.sha256(payload.encode()).hexdigest()


def _git_blob(source: str) -> str:
    payload = source.encode()
    return hashlib.sha1(f"blob {len(payload)}\0".encode() + payload).hexdigest()


def _blob(blob: str) -> str:
    try:
        return subprocess.check_output(
            ["git", "cat-file", "blob", blob], cwd=ROOT, text=True, encoding="utf-8"
        )
    except subprocess.CalledProcessError as error:
        raise GuardError(f"authenticated blob {blob} is unavailable") from error


def _read_source() -> str:
    _require(SOURCE_PATH.is_file(), "vector source is missing")
    _require(not SOURCE_PATH.is_symlink(), "vector source must not be a symlink")
    return SOURCE_PATH.read_text(encoding="utf-8")


def _compact(source: str) -> str:
    return re.sub(r"\s+", "", source)


def _skip_quoted(source: str, start: int) -> int:
    quote_start = start + (1 if source.startswith('b"', start) else 0)
    cursor = quote_start + 1
    while cursor < len(source):
        if source[cursor] == "\\":
            cursor += 2
        elif source[cursor] == '"':
            return cursor + 1
        else:
            cursor += 1
    raise GuardError("unterminated Rust string")


def _matching_delimiter(source: str, opening: int) -> int:
    pairs = {"(": ")", "[": "]", "{": "}"}
    stack: list[str] = []
    cursor = opening
    while cursor < len(source):
        if source.startswith("//", cursor):
            newline = source.find("\n", cursor + 2)
            cursor = len(source) if newline < 0 else newline + 1
            continue
        if source.startswith("/*", cursor):
            depth = 1
            cursor += 2
            while cursor < len(source) and depth:
                if source.startswith("/*", cursor):
                    depth += 1
                    cursor += 2
                elif source.startswith("*/", cursor):
                    depth -= 1
                    cursor += 2
                else:
                    cursor += 1
            _require(depth == 0, "unterminated Rust block comment")
            continue
        if source[cursor] == '"' or source.startswith('b"', cursor):
            cursor = _skip_quoted(source, cursor)
            continue
        character = source[cursor]
        if character in pairs:
            stack.append(character)
        elif character in pairs.values():
            _require(stack and pairs[stack[-1]] == character, "unbalanced Rust delimiter")
            stack.pop()
            if not stack:
                return cursor
        cursor += 1
    raise GuardError("unterminated Rust delimiter")


def _function(source: str, name: str) -> str:
    pattern = re.compile(
        rf"(?m)^[ \t]*(?:(?:pub(?:\([^\n)]*\))?)\s+)?"
        rf"(?:(?:async|const|unsafe)\s+)*fn\s+{re.escape(name)}"
        rf"(?:<[^{{\n]*>)?\s*\("
    )
    matches = tuple(pattern.finditer(source))
    _require(len(matches) == 1, f"expected one function named {name}")
    opening = source.find("{", matches[0].end())
    _require(opening >= 0, f"missing function body for {name}")
    return source[matches[0].start() : _matching_delimiter(source, opening) + 1]


def _header(function: str) -> str:
    return _compact(function[: function.find("{")])


def _split_arguments(arguments: str) -> tuple[str, ...]:
    parts: list[str] = []
    stack: list[str] = []
    pairs = {"(": ")", "[": "]", "{": "}"}
    start = 0
    cursor = 0
    while cursor < len(arguments):
        if arguments[cursor] == '"':
            cursor = _skip_quoted(arguments, cursor)
            continue
        character = arguments[cursor]
        if character in pairs:
            stack.append(character)
        elif character in pairs.values():
            _require(stack and pairs[stack[-1]] == character, "unbalanced call argument")
            stack.pop()
        elif character == "," and not stack:
            parts.append(_compact(arguments[start:cursor]))
            start = cursor + 1
        cursor += 1
    if arguments[start:].strip():
        parts.append(_compact(arguments[start:]))
    return tuple(parts)


def _calls(source: str, name: str) -> tuple[tuple[str, ...], ...]:
    calls: list[tuple[str, ...]] = []
    for match in re.finditer(rf"\b{re.escape(name)}\s*\(", source):
        if re.search(r"fn\s*$", source[max(0, match.start() - 8) : match.start()]):
            continue
        opening = source.find("(", match.start())
        closing = _matching_delimiter(source, opening)
        calls.append(_split_arguments(source[opening + 1 : closing]))
    return tuple(calls)


def _test_inventory(source: str) -> tuple[tuple[str, tuple[str, ...]], ...]:
    lines = source.splitlines()
    inventory: list[tuple[str, tuple[str, ...]]] = []
    for index, line in enumerate(lines):
        if line.strip() != "#[test]":
            continue
        attributes: list[str] = []
        cursor = index
        while cursor >= 0 and lines[cursor].strip().startswith("#["):
            attributes.insert(0, lines[cursor].strip())
            cursor -= 1
        cursor = index + 1
        while cursor < len(lines) and not re.match(r"\s*fn\s+", lines[cursor]):
            cursor += 1
        _require(cursor < len(lines), "test attribute is not followed by a function")
        name = re.search(r"fn\s+(\w+)", lines[cursor])
        _require(name is not None, "test function name is missing")
        inventory.append((name.group(1), tuple(attributes)))
    return tuple(inventory)


def _public_api(source: str) -> tuple[tuple[str, str], ...]:
    inventory: list[tuple[str, str]] = []
    pattern = re.compile(
        r"(?m)^pub(?:\([^\n)]*\))?\s+"
        r"(?:(?:unsafe|const|async)\s+)*fn\s+(\w+)"
    )
    for match in pattern.finditer(source):
        opening = source.find("{", match.end())
        _require(opening >= 0, f"missing public function body for {match.group(1)}")
        inventory.append((match.group(1), _compact(source[match.start() : opening])))
    return tuple(inventory)


def _include_inventory(source: str) -> tuple[str, ...]:
    return tuple(
        re.findall(r'include(?:_str|_bytes)?!\s*\(\s*"([^"]+)"\s*\)', source)
    )


def _require_order(source: str, anchors: tuple[str, ...], label: str) -> None:
    compact = _compact(source)
    cursor = 0
    for anchor in anchors:
        expected = _compact(anchor)
        index = compact.find(expected, cursor)
        _require(index >= 0, f"{label} sequence changed at {anchor!r}")
        cursor = index + len(expected)


def _seam_counts(source: str) -> dict[str, int]:
    patterns = {
        "fn_bounds": r"\bFn(?:Once|Mut)?\b",
        "function_pointers": r"(?::|=|->)\s*fn\s*\(",
        "closures": r"\|[^|\n]*\|",
        "macro_definitions": r"macro_rules!",
        "action_types": r"\b(?:Action|Step|Body|Assertion)\w*\b",
        "path_relocations": r"#\s*\[\s*path\s*=",
        "rustfmt_skips": r"rustfmt::skip",
        "unsafe_functions": r"\bunsafe\s+fn\b",
    }
    return {name: len(re.findall(pattern, source)) for name, pattern in patterns.items()}


def _external_call_sites() -> dict[str, tuple[str, ...]]:
    paths = subprocess.check_output(
        ["git", "ls-files", "*.rs"], cwd=ROOT, text=True, encoding="utf-8"
    ).splitlines()
    contents: list[tuple[str, str]] = []
    for relative in paths:
        if relative == "crates/ivm/src/vector.rs":
            continue
        path = ROOT / relative
        if path.is_file():
            contents.append((relative, path.read_text(encoding="utf-8")))
    inventory: dict[str, tuple[str, ...]] = {}
    for symbol in EXPECTED_EXTERNAL_CALL_SITES:
        pattern = re.compile(rf"\b{re.escape(symbol)}\s*\(")
        inventory[symbol] = tuple(path for path, text in contents if pattern.search(text))
    return inventory


def _validate_source(
    source: str, *, authenticate: bool, check_external_calls: bool = False
) -> None:
    preimage = _blob(PREIMAGE_BLOB)
    _require(_git_blob(preimage) == PREIMAGE_BLOB, "preimage blob identity changed")
    _require(_sha256(preimage) == PREIMAGE_SHA256, "preimage SHA-256 changed")
    _require(len(preimage.splitlines()) == PREIMAGE_LINES, "preimage line count changed")
    _require(
        PREIMAGE_LINES - POSTIMAGE_LINES >= MINIMUM_LINE_REDUCTION,
        "recorded line reduction no longer clears the gate",
    )
    _require(
        PREIMAGE_AST_NODES - POSTIMAGE_AST_NODES >= MINIMUM_AST_NODE_REDUCTION,
        "recorded AST-node reduction no longer clears the gate",
    )
    _require(
        POSTIMAGE_NAMED_AST_NODES < PREIMAGE_NAMED_AST_NODES
        and POSTIMAGE_LEAF_TOKENS < PREIMAGE_LEAF_TOKENS,
        "recorded parser/compiler-work evidence is not reductive",
    )
    _require(PREIMAGE_AST_ERRORS == POSTIMAGE_AST_ERRORS == 1, "AST error ledger changed")

    compacted = _blob(POSTIMAGE_BLOB)
    _require(_git_blob(compacted) == POSTIMAGE_BLOB, "compaction blob identity changed")
    _require(_sha256(compacted) == POSTIMAGE_SHA256, "compaction SHA-256 changed")
    _require(len(compacted.splitlines()) == POSTIMAGE_LINES, "compaction line count changed")
    _require(
        _sha256(_public_api(compacted)) == EXPECTED_PUBLIC_API_SHA256,
        "historical compaction API changed",
    )
    _require(
        _sha256(_include_inventory(compacted)) == EXPECTED_INCLUDE_SHA256,
        "historical compaction includes changed",
    )
    historical_test_marker = '#[cfg(test)]\nmod tests'
    _require(
        _sha256(compacted[compacted.index(historical_test_marker) :])
        == EXPECTED_TEST_SUFFIX_SHA256,
        "historical compaction test evidence changed",
    )
    if authenticate:
        _require(len(source.splitlines()) == CURRENT_LINES, "current source line count changed")
        _require(_sha256(source) == CURRENT_SHA256, "current V1 source SHA-256 changed")
    _require(
        source.count('unsafe extern "C" {') == POSTIMAGE_AST_ERRORS,
        "known tree-sitter error sentinel changed",
    )
    _require(
        max(map(len, source.splitlines())) <= EXPECTED_MAX_LINE_LENGTH,
        "source line packing exceeded the audited maximum",
    )
    _require(
        max(line.count(";") for line in source.splitlines())
        <= EXPECTED_MAX_SEMICOLONS_PER_LINE,
        "multiple statements were packed onto a line",
    )
    _require(
        len(re.findall(r"(?m)^\s*#\[cfg\(", source)) == CURRENT_CFG_COUNT,
        "cfg inventory changed",
    )
    _require(
        len(re.findall(r"(?m)^\s*#\[target_feature\(", source))
        == EXPECTED_TARGET_FEATURE_COUNT,
        "target-feature inventory changed",
    )
    _require(_test_inventory(source) == EXPECTED_TESTS, "test IDs/attrs/order changed")
    test_marker = '#[cfg(test)]\nmod tests'
    _require(test_marker in source, "test module marker is missing")
    _require(
        _sha256(source[source.index(test_marker) :]) == CURRENT_TEST_SUFFIX_SHA256,
        "direct test module changed",
    )
    _require(_sha256(_public_api(source)) == CURRENT_PUBLIC_API_SHA256, "V1 API changed")
    retained_api = tuple(
        entry for entry in _public_api(preimage) if entry[0] != "bit_pipe_compile_count"
    )
    added_api_names = {
        "metal_merkle_prefer_gpu", "metal_batch_prefer_gpu",
        "metal_merkle_root", "gpu_launch_eligible",
    }
    _require(
        tuple(entry for entry in _public_api(source) if entry[0] not in added_api_names)
        == retained_api,
        "retained vector API changed",
    )
    _require(
        _include_inventory(source) == CURRENT_INCLUDE_PATHS,
        "embedded artifact include inventory changed",
    )
    _require(_seam_counts(source) == EXPECTED_SEAM_COUNTS, "forbidden seam inventory changed")

    expected_headers = {
        "metal_input_buffer": (
            "fnmetal_input_buffer<T:MetalBufferElement>(device:"
            "&ProtocolObject<dynMTLDevice>,values:&[T],byte_len:usize,)->"
            "Option<Retained<ProtocolObject<dynMTLBuffer>>>"
        ),
        "metal_output_buffer": (
            "fnmetal_output_buffer(device:&ProtocolObject<dynMTLDevice>,byte_len:usize,)->"
            "Option<Retained<ProtocolObject<dynMTLBuffer>>>"
        ),
        "metal_dispatch": (
            "fnmetal_dispatch(queue:&ProtocolObject<dynMTLCommandQueue>,pipeline:"
            "&ProtocolObject<dynMTLComputePipelineState>,buffers:"
            "&[&ProtocolObject<dynMTLBuffer>],grid_width:NSUInteger,"
            "threadgroup_width:NSUInteger,context:&str,receipt:Option<MetalKernel>,)->Option<()>"
        ),
    }
    for name, expected in expected_headers.items():
        _require(_header(_function(source, name)) == expected, f"{name} signature changed")

    marker_surface = """#[cfg(all(target_os = "macos", feature = "metal"))]
trait MetalBufferElement: Copy {}
#[cfg(all(target_os = "macos", feature = "metal"))]
impl MetalBufferElement for u8 {}
#[cfg(all(target_os = "macos", feature = "metal"))]
impl MetalBufferElement for u32 {}
#[cfg(all(target_os = "macos", feature = "metal"))]
impl MetalBufferElement for u64 {}"""
    _require(source.count(marker_surface) == 1, "typed Metal marker surface changed")
    _require(
        tuple(
            re.findall(
                r"(?m)^impl MetalBufferElement for ([^\s{]+) \{\}$",
                source,
            )
        )
        == ("u8", "u32", "u64"),
        "typed Metal marker implementation set changed",
    )

    input_buffer = _function(source, "metal_input_buffer")
    _require_order(
        input_buffer,
        (
            "debug_assert!(byte_len <= core::mem::size_of_val(values));",
            "NonNull::new_unchecked(values.as_ptr() as *mut core::ffi::c_void)",
            "byte_len,",
            "MTLResourceOptions::CPUCacheModeDefaultCache,",
        ),
        "typed input buffer",
    )
    input_calls = _calls(source, "metal_input_buffer")
    _require(len(input_calls) == 65, "typed input-buffer call count changed")
    _require(_sha256(input_calls) == EXPECTED_INPUT_CALL_SHA256, "input call map changed")
    _require(
        input_calls.count(("&device", "&cur[..]", "cur.len()")) == 1,
        "self-test full cur buffer changed",
    )
    _require(
        input_calls.count(("&ctx.device", "&cur[..]", "pair_len")) == 1,
        "true pair_len prefix buffer changed",
    )

    output_calls = _calls(source, "metal_output_buffer")
    _require(len(output_calls) == 28, "output-buffer call count changed")
    _require(
        output_calls == EXPECTED_OUTPUT_CALLS,
        "output-buffer device receiver/length map changed",
    )
    _require(_sha256(output_calls) == EXPECTED_OUTPUT_CALL_SHA256, "output call map changed")
    dispatch_calls = _calls(source, "metal_dispatch")
    _require(all(len(row) == 7 for row in dispatch_calls), "Metal dispatch arity changed")
    dispatch_mapping = tuple(row[:-1] for row in dispatch_calls)
    _require(dispatch_mapping == EXPECTED_DISPATCH_CALLS, "ordered Metal dispatch rows changed")
    _require(
        _sha256(dispatch_mapping) == EXPECTED_DISPATCH_CALL_SHA256,
        "Metal dispatch row digest changed",
    )
    _require(
        tuple(row[-1] for row in dispatch_calls) == EXPECTED_DISPATCH_RECEIPTS,
        "Metal accepted-completion receipt map changed",
    )
    dispatch = _function(source, "metal_dispatch")
    _require(
        "    encoder.endEncoding();\n    command_buffer.commit();" in dispatch,
        "Metal dispatch statements were packed onto one line",
    )
    _require_order(
        dispatch,
        (
            "if !metal_runtime_allowed() { return None; }",
            "let Some(command_buffer) = queue.commandBuffer() else {",
            'record_metal_disable(format!("{context} could not create a command buffer"));',
            "return None;",
            "let Some(encoder) = command_buffer.computeCommandEncoder() else {",
            'record_metal_disable(format!("{context} could not create a compute encoder"));',
            "return None;",
            "encoder.setComputePipelineState(pipeline);",
            "for (index, buffer) in buffers.iter().copied().enumerate()",
            "encoder.setBuffer_offset_atIndex(Some(buffer), 0, index);",
            "encoder.dispatchThreads_threadsPerThreadgroup",
            "width: grid_width,",
            "width: threadgroup_width,",
            "encoder.endEncoding();",
            "command_buffer.commit();",
            "let completed = finalize_command_buffer(&command_buffer, context);",
            "let accepted = metal_dispatch_result_allowed(completed, metal_runtime_allowed());",
            "metal_receipts::record_completion(receipt, accepted);",
            "accepted.then_some(())",
        ),
        "Metal encode/commit/wait",
    )

    constructor = _function(source, "new")
    _require_order(
        constructor,
        (
            "let cmd2 = queue.commandBuffer()?;",
            "let dec = cmd2.computeCommandEncoder()?;",
            "dec.setComputePipelineState(&aesdec_rounds);",
            "metal_input_buffer(&device, &enc2[..], 16)?;",
            "metal_output_buffer(&device, 16)?;",
            "dec.setBuffer_offset_atIndex(Some(&buf_states2), 0, 0);",
            "dec.setBuffer_offset_atIndex(Some(&buf_rks), 0, 1);",
            "dec.setBuffer_offset_atIndex(Some(&buf_out2), 0, 2);",
            "dec.setBuffer_offset_atIndex(Some(&buf_n), 0, 3);",
            """let grid = MTLSize {
                        width: 1,
                        height: 1,
                        depth: 1,
                    };""",
            """let threads = MTLSize {
                        width: 1,
                        height: 1,
                        depth: 1,
                    };""",
            "dec.dispatchThreads_threadsPerThreadgroup(grid, threads);",
            "dec.endEncoding();",
            "cmd2.commit();",
            'finalize_command_buffer(&cmd2, "metal self-test aesdec rounds")',
        ),
        "inline AESDEC encoder-before-buffer self-test",
    )

    for name in ("vadd32_slice", "vadd64_slice", "vand_slice", "vxor_slice", "vor_slice", "vrot32_slice"):
        _require(_function(source, name) == _function(preimage, name), f"{name} changed")
    fallback_orders = {
        "vadd32": ("metal_vadd32(a, b)", "crate::cuda::vadd32_cuda", "vadd32_slice"),
        "vadd64": ("metal_vadd64(a, b)", "crate::cuda::vadd64_cuda", "vadd64_slice"),
        "vand": ("metal_vand(a, b)", "crate::cuda::vand_cuda", "vand_slice"),
        "vxor": ("metal_vxor(a, b)", "crate::cuda::vxor_cuda", "vxor_slice"),
        "vor": ("metal_vor(a, b)", "crate::cuda::vor_cuda", "vor_slice"),
        "vrot32": ("vrot32_slice",),
    }
    for name, anchors in fallback_orders.items():
        function = _function(source, name)
        _require_order(function, anchors, f"{name} fallback")
        _require("target_arch" not in function, f"{name} reintroduced duplicate arch bodies")
    auto_orders = {
        "vadd32_auto": ("assert_eq!(a.len(), lanes);", "assert_eq!(b.len(), lanes);", "vadd32_dyn"),
        "vadd64_auto": ("assert_eq!(a.len(), lanes);", "assert_eq!(b.len(), lanes);", "vadd64_dyn"),
        "vand_auto": ("assert_eq!(a.len(), lanes);", "assert_eq!(b.len(), lanes);", "vand_dyn"),
        "vxor_auto": ("assert_eq!(a.len(), lanes);", "assert_eq!(b.len(), lanes);", "vxor_dyn"),
        "vor_auto": ("assert_eq!(a.len(), lanes);", "assert_eq!(b.len(), lanes);", "vor_dyn"),
        "vrot32_auto": ("assert_eq!(a.len(), lanes);", "vrot32_dyn"),
    }
    for name, anchors in auto_orders.items():
        _require_order(_function(source, name), anchors, f"{name} assertion/fallback")

    _validate_v1_behavior(source)

    if check_external_calls:
        _require(_external_call_sites() == EXPECTED_EXTERNAL_CALL_SITES, "external call-site set changed")


def _validate_v1_behavior(source: str) -> None:
    """Check source-level V1 ownership, receipt and public geometry contracts."""

    for retired in ("newLibraryWithSource_options_error", "bit_pipe_compile_count", "static METAL_STATE: std::cell::RefCell", "metal_acceleration_speed"):
        _require(retired not in source, f"retired Metal path returned: {retired}")
    _require_order(
        _function(source, "bundled_metal_library"),
        (
            "if sha2::Sha256::digest(METAL_KERNELS).as_slice() != METAL_KERNELS_SHA256 {",
            'record_metal_disable("embedded Metal library digest mismatch");',
            "return None;",
            "dispatch2::DispatchData::from_static_bytes(METAL_KERNELS)",
            "device.newLibraryWithData_error(&data).ok()",
        ),
        "authenticated embedded Metal library",
    )
    _require(
        "static METAL_STATE: metal_owner::ProcessOwner<MetalState> = metal_owner::ProcessOwner::new();" in source,
        "Metal lost its process owner",
    )
    _require_order(
        _function(source, "with_metal_state"),
        ("if !metal_runtime_allowed()", "return None;", "METAL_STATE.acquire(MetalState::new)?", "Some(f(&state))"),
        "Metal owner acquisition",
    )
    _require(
        _compact(_function(source, "metal_dispatch_result_allowed"))
        == "fnmetal_dispatch_result_allowed(completed:bool,backend_allowed:bool)->bool{completed&&backend_allowed}",
        "quarantined or failed Metal result may be published",
    )
    _require(
        "const MIN_GPU_TRANSFER_BYTES: usize = 4 * 1024;" in source,
        "public transfer floor changed",
    )
    _require(
        _compact(_function(source, "gpu_launch_eligible"))
        == "pub(crate)fngpu_launch_eligible(transfer_bytes:usize)->bool{transfer_bytes>=MIN_GPU_TRANSFER_BYTES}",
        "GPU selection no longer depends only on public transfer geometry",
    )
    for name in ("vadd32", "vadd64", "vand", "vxor", "vor"):
        function = _function(source, name)
        _require(
            function.count("gpu_launch_eligible(FIXED_VECTOR_TRANSFER_BYTES)") == 2,
            f"{name} lost a public workload gate",
        )
    _require(
        _function(source, "sha256_compress").count("gpu_launch_eligible(SHA256_BLOCK_TRANSFER_BYTES)") == 2,
        "single-block SHA lost a public workload gate",
    )
    for name in ("metal_aesenc_rounds_batch_with_receipt", "metal_aesdec_rounds_batch_with_receipt"):
        _require(
            "let nrounds = u32::try_from(round_keys.len()).ok()?;" in _function(source, name),
            f"{name} silently narrows the public round count",
        )
    merkle = _function(source, "metal_sha256_pairs_reduce_with_receipt")
    _require(merkle.count("if canonical_merkle {") == 3, "canonical Merkle marker sites changed")
    _require_order(
        merkle,
        (
            "if digests.len() == 1", "root[31] |= 1;", "return Some(root);",
            "while count > 1", "for node in cur.chunks_exact_mut(32)", "node[31] |= 1;",
            "metal_dispatch(", "root.copy_from_slice(&cur[..32]);", "root[31] |= 1;", "Some(root)",
        ),
        "canonical Merkle hash markers",
    )


class IvmVectorMetalCompactionSourceTest(unittest.TestCase):
    """Authenticate the compacted vector/Metal implementation and mutations."""

    def setUp(self) -> None:
        self.source = _read_source()

    def _rejected(self, old: str, new: str, *, count: int = 1) -> None:
        _validate_source(self.source, authenticate=False)
        self.assertEqual(self.source.count(old), count)
        self.assertNotEqual(old, new)
        mutated = self.source.replace(old, new, 1)
        with self.assertRaises(GuardError):
            _validate_source(mutated, authenticate=False)

    def test_repository_contract(self) -> None:
        _validate_source(self.source, authenticate=True, check_external_calls=True)

    def test_rejects_test_identity_drift(self) -> None:
        self._rejected(
            "fn fixed_width_operations_skip_gpu_launch()",
            "fn fixed_width_operations_skip_gpu_launch_changed()",
        )

    def test_rejects_input_bound_or_prefix_drift(self) -> None:
        self._rejected(
            "debug_assert!(byte_len <= core::mem::size_of_val(values));",
            "debug_assert!(byte_len < core::mem::size_of_val(values));",
        )
        self._rejected(
            "metal_input_buffer(&ctx.device, &cur[..], pair_len)?",
            "metal_input_buffer(&ctx.device, &cur[..], cur.len())?",
        )
        self._rejected(
            """impl MetalBufferElement for u64 {}

#[cfg(all(target_os = "macos", feature = "metal"))]
fn metal_input_buffer""",
            """impl MetalBufferElement for u64 {}
impl MetalBufferElement for PaddedMetalWord {}
#[cfg(all(target_os = "macos", feature = "metal"))]
fn metal_input_buffer""",
        )

    def test_rejects_dispatch_buffer_or_inline_order_drift(self) -> None:
        self._rejected(
            "metal_output_buffer(&ctx.device, n)?",
            "metal_output_buffer(&device, n)?",
        )
        self._rejected(
            "&[&buf_a, &buf_b, &buf_out]",
            "&[&buf_b, &buf_a, &buf_out]",
            count=5,
        )
        self._rejected(
            "let cmd2 = queue.commandBuffer()?;\n                    let dec = cmd2.computeCommandEncoder()?;",
            "let dec = cmd2.computeCommandEncoder()?;\n                    let cmd2 = queue.commandBuffer()?;",
        )
        self._rejected(
            """dec.setBuffer_offset_atIndex(Some(&buf_n), 0, 3);
                    }
                    let grid = MTLSize {
                        width: 1,""",
            """dec.setBuffer_offset_atIndex(Some(&buf_n), 0, 3);
                    }
                    let aesdec_grid = MTLSize {
                        width: 1,""",
        )

    def test_rejects_fallback_order_drift(self) -> None:
        self._rejected("crate::cuda::vadd32_cuda", "crate::cuda::vadd32_reordered")

    def test_rejects_new_callback_or_macro_seams(self) -> None:
        self._rejected(
            "debug_assert!(byte_len <= core::mem::size_of_val(values));",
            "let callback: fn() = warm_up_metal;",
        )
        self._rejected(
            "debug_assert!(byte_len <= core::mem::size_of_val(values));",
            "macro_rules! packed_body { () => {} }",
        )

    def test_rejects_relocation_or_cfg_drift(self) -> None:
        self._rejected(
            "debug_assert!(byte_len <= core::mem::size_of_val(values));",
            'let _ = include_str!("relocated.rs");',
        )
        self._rejected('#[target_feature(enable = "sha2")]', "#[cfg(any())]")

    def test_rejects_quarantine_or_receipt_drift(self) -> None:
        self._rejected("completed && backend_allowed", "completed || backend_allowed")
        self._rejected(
            "metal_receipts::record_completion(receipt, accepted);",
            "metal_receipts::record_completion(receipt, completed);",
        )
        self._rejected("Some(MetalKernel::Add64)", "None")

    def test_rejects_public_workload_gate_drift(self) -> None:
        self._rejected(
            "if gpu_launch_eligible(FIXED_VECTOR_TRANSFER_BYTES)",
            "if true",
            count=10,
        )
        self._rejected("transfer_bytes >= MIN_GPU_TRANSFER_BYTES", "transfer_bytes < MIN_GPU_TRANSFER_BYTES")

    def test_rejects_embedded_library_or_owner_drift(self) -> None:
        self._rejected("METAL_STATE.acquire(MetalState::new)?", "MetalState::new()?")
        self._rejected("!= METAL_KERNELS_SHA256", "== METAL_KERNELS_SHA256")

    def test_rejects_merkle_marker_or_round_bound_drift(self) -> None:
        self._rejected("node[31] |= 1;", "node[31] |= 0;")
        self._rejected("u32::try_from(round_keys.len()).ok()?", "round_keys.len() as u32", count=2)

    def test_rejects_line_minification(self) -> None:
        self._rejected(
            "    encoder.endEncoding();\n    command_buffer.commit();",
            "    encoder.endEncoding(); command_buffer.commit();",
        )


if __name__ == "__main__":
    unittest.main()
