#!/usr/bin/env python3
"""Enforce the Kotodama V1 Criterion regression budget.

The gate compares every current compiler/runtime workload against Criterion's
runner-local ``base`` samples captured from the clean reset anchor. Missing or
malformed samples fail closed. A workload fails when its median is more than
five percent slower, and List sugar has an independent zero-slowdown check
against its manual-loop counterpart. Candidate samples can never be captured
as a portable or self baseline.
"""

from __future__ import annotations

import argparse
import collections
import hashlib
import json
import math
import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Mapping, Sequence


MAX_REGRESSION = 0.05
LIST_SUGAR_MAX_SLOWDOWN = 0.0
# The user-selected reset anchor is the untouched comparison baseline. It
# contains the complete current benchmark inventory and its original lockfile.
BASELINE_SHA = "fc09b635df385d0488067f09baaa92a8d16fa124"
BASELINE_CARGO_LOCK_SHA256 = (
    "0ddb3f3938cf32035371317100674cd1601c3cb41232237f7a7d28b3aeab6222"
)
CANDIDATE_CARGO_LOCK_SHA256 = (
    "71df4943f58ae56f1a6f5286962ed02ae21b5c1940ac8d3bede09dc10dd424d2"
)
EVIDENCE_SCHEMA = "iroha.kotodama.performance.v1"
LIST_SUGAR_BENCHMARK = "kotodama_list_comprehension_runtime_64"
LIST_MANUAL_BENCHMARK = "kotodama_list_manual_runtime_64"
DECIMAL_BENCHMARKS = (
    "kotodama_decimal_add",
    "kotodama_decimal_sub",
    "kotodama_decimal_mul",
    "kotodama_decimal_div_exact",
    "kotodama_decimal_div_round_floor",
    "kotodama_decimal_div_round_ceil",
    "kotodama_decimal_div_round_nearest_even",
)
RUNTIME_PHASE_BENCHMARKS = (
    "kotodama_runtime_phase_prepare_validate_predecode",
    "kotodama_runtime_phase_argument_decode",
    "kotodama_runtime_phase_load_prepared",
    "kotodama_runtime_phase_dirty_reset",
    "kotodama_runtime_phase_execute_prepared",
)
INTERFACE_PHASE_BENCHMARKS = ("kotodama_phase_interface_summary",)
# These identities receive additional source-shape checks as well as the
# ordinary same-runner comparison.
SOURCE_BOUND_BENCHMARKS = (
    DECIMAL_BENCHMARKS
    + INTERFACE_PHASE_BENCHMARKS
    + RUNTIME_PHASE_BENCHMARKS
)
REPOSITORY_ROOT = Path(__file__).resolve().parents[1]
BENCHMARK_SOURCE_PATHS = (
    Path("crates/ivm/benches/bench_kotodama.rs"),
    Path("crates/iroha_core/benches/kotodama_runtime_cache.rs"),
    Path("crates/iroha_core/benches/queries.rs"),
)
BENCHMARK_SOURCES = tuple(
    REPOSITORY_ROOT / relative for relative in BENCHMARK_SOURCE_PATHS
)
IVM_BENCHMARK_SOURCE = BENCHMARK_SOURCES[0]
TYPED_QUERY_BENCHMARK_MARKER = "c.bench_function(family.benchmark_name, |b| {"
TYPED_QUERY_TIMED_BODY_MARKER = "|mut vm| {"
TYPED_QUERY_TIMED_BODY_END = "BatchSize::SmallInput,"
TYPED_QUERY_RAW_RESPONSE_FUNCTION = "fn raw_core_query_response("
TYPED_QUERY_RAW_BYTES_STATEMENT = (
    "let raw_query_response_bytes = TYPED_CORE_QUERY_FAMILIES.map("
)
TYPED_QUERY_RAW_CONTRACT_MARKERS = (
    "QueryRequest::Start(",
    "run_on_snapshot(",
    "let QueryResponse::Iterable(output)",
    "output.batch.len(), QUERY_PAGE_CAPACITY_V1",
    "!output.has_more && output.continue_cursor.is_none()",
    "norito::to_bytes(&raw_core_query_response(",
    ".len(),",
)
TYPED_QUERY_TIMED_CONTRACT_MARKERS = (
    ".syscall(ivm::syscalls::SYSCALL_CORE_QUERY_PAGE, &mut vm)",
    "ivm::list::read_words(&vm, vm.register(10), page_layout)",
    "items.len(), QUERY_PAGE_CAPACITY_V1",
    ".core_query_page_metrics()",
    "metrics.host_queries, 1",
    "metrics.projection_decodes, 1",
    "metrics.leaf_tlv_bytes > 0",
    "metrics.projection_payload_bytes < raw_query_response_bytes",
    "std::hint::black_box((gas, items, vm.register(11), metrics));",
)
PROTECTED_BENCHMARK_PATTERN = re.compile(
    r'"(kotodama_(?:(?:decimal|runtime_phase)_[a-z0-9_]+|phase_interface_summary))"'
)
REPRESENTATIVE_BENCHMARKS = (
    "kotodama_phase_parse",
    "kotodama_phase_resolved_hir",
    "kotodama_phase_semantic",
    *INTERFACE_PHASE_BENCHMARKS,
    "kotodama_phase_typed_effect_hir",
    "kotodama_phase_ir_lower",
    "kotodama_phase_ssa_construct",
    "kotodama_phase_ssa_optimize",
    "kotodama_phase_de_ssa",
    "kotodama_phase_codegen",
    "kotodama_phase_codegen_end_to_end",
    "kotodama_list_semantic_64",
    "kotodama_list_lower_64",
    "kotodama_list_get_64",
    "kotodama_list_try_set_64",
    "kotodama_list_try_push_pop_64",
    "kotodama_list_contains_64",
    LIST_SUGAR_BENCHMARK,
    LIST_MANUAL_BENCHMARK,
    "kotodama_quantity_add",
    "kotodama_quantity_sub",
    "kotodama_quantity_mul_decimal",
    "kotodama_quantity_div_decimal_exact",
    "kotodama_quantity_div_round_floor",
    "kotodama_quantity_div_round_ceil",
    "kotodama_quantity_div_round_nearest_even",
    *DECIMAL_BENCHMARKS,
    "typed_core_query_accounts_page_64",
    "typed_core_query_assets_page_64",
    "typed_core_query_asset_definitions_page_64",
    "typed_core_query_domains_page_64",
    "typed_core_query_nfts_page_64",
    *RUNTIME_PHASE_BENCHMARKS,
    "kotodama_runtime_cold_add",
    "kotodama_runtime_warm_add",
    "kotodama_core_runtime_warm_add",
)

# The clean anchor contains the complete current inventory, so every required
# candidate identity has a native baseline sample.
REGRESSION_BENCHMARKS = REPRESENTATIVE_BENCHMARKS
QUANTITY_ROUND_BENCHMARKS = (
    "kotodama_quantity_div_round_floor",
    "kotodama_quantity_div_round_ceil",
    "kotodama_quantity_div_round_nearest_even",
)
DECIMAL_ROUND_BENCHMARKS = DECIMAL_BENCHMARKS[-3:]
TYPED_QUERY_BENCHMARKS = (
    "typed_core_query_accounts_page_64",
    "typed_core_query_assets_page_64",
    "typed_core_query_asset_definitions_page_64",
    "typed_core_query_domains_page_64",
    "typed_core_query_nfts_page_64",
)
STATIC_REGRESSION_BENCHMARKS = tuple(
    name
    for name in REGRESSION_BENCHMARKS
    if name
    not in {
        *DECIMAL_ROUND_BENCHMARKS,
        *QUANTITY_ROUND_BENCHMARKS,
        *TYPED_QUERY_BENCHMARKS,
    }
)
CORE_RUNTIME_WARM_BENCHMARK = "kotodama_core_runtime_warm_add"
CORE_RUNTIME_BASELINE_CHECKOUT = ".checkout_runtime(GAS_LIMIT)"
CORE_RUNTIME_EXPLICIT_MAX_CHECKOUT = (
    ".checkout_runtime(GAS_LIMIT, ivm::Memory::HEAP_MAX_SIZE)"
)

# Filled from the normalized native timed closures at `BASELINE_SHA`.
# Shared Criterion loops deliberately have one closure hash per identity; their
# identity-to-mode/family declaration is prepended before hashing so a mapping
# mutation cannot hide behind an unchanged shared closure.
BASELINE_TIMED_BODY_SHA256: Mapping[str, str] = {
    "kotodama_phase_parse": "1f21f2b13cba3c59d78948343b967ed357760132264301db6f47cdb289625cc9",
    "kotodama_phase_resolved_hir": "5bd4e90133ef1a7b2ee4e08b048c54272ed84d59dbefcf51e15f3317d2eb3185",
    "kotodama_phase_semantic": "fa4e992084b581d1d76425ac014c629594efe384998fe9d838643b0a56131bbf",
    "kotodama_phase_interface_summary": "d50459e751760e7162fd5572928b416097436ef5b276574511fb975d9c3673e7",
    "kotodama_phase_typed_effect_hir": "fa4e992084b581d1d76425ac014c629594efe384998fe9d838643b0a56131bbf",
    "kotodama_phase_ir_lower": "49823939b2bc7afbeca3c3c70b1c25cacfd03b852a11838307ad5fb2efb94333",
    "kotodama_phase_ssa_construct": "6fc353843dd5811ff7df6d0afd5492f220da1ff86cc5bc0de2a56e4f47c76d82",
    "kotodama_phase_ssa_optimize": "a37af101575badaad00ca416d3bfc4313c7787453b1664602f7061ad8d612931",
    "kotodama_phase_de_ssa": "b83dcf4a7eccc91d3bd659a4612174b4decf45915e4c868696c974da7ff47af4",
    "kotodama_phase_codegen": "baf7ec764d7a0b3de7d74f2f5aebf586dff938a6b86fe543143d03aeb8c240de",
    "kotodama_phase_codegen_end_to_end": "e10b492a1820c45cceb4417ec059607653955e8a63bea20b652e5a833795d739",
    "kotodama_list_semantic_64": "f7bb53b11a83d906cfe2c7faa34f6bb83977076d810377ffc5b2fb73134c6573",
    "kotodama_list_lower_64": "6384fb1d5bdd0a597809c7b153ba95f1b9a5aa743c1f40c46b3b43dee32cd44e",
    "kotodama_list_get_64": "f7efc1c532101095b0a80d67d4ff82a38b1de67d1765363c8bfa5710e81660b7",
    "kotodama_list_try_set_64": "772f8320285048c9086ba517c56f3513888bee7d525a235cd112c3750814aacc",
    "kotodama_list_try_push_pop_64": "0922c995c21233249536cc334fb0e761f299e5597e603450133770551f31aee4",
    "kotodama_list_contains_64": "32a48738d4f3a91ff147581318c2e1cc52e5150e3788ad44c79571e2f10c082a",
    "kotodama_list_comprehension_runtime_64": "64a700e36677badbb24a133fc4fc0ea84efc8a40c07fdac8bc830691bbcbbcc0",
    "kotodama_list_manual_runtime_64": "30775489dd3505aceaf7befd7257538f5a0f39f651e2036bebc2770a24e9b39d",
    "kotodama_quantity_add": "1d1ae9909eaa6ef0ff3268579c25f6738a3a324b11d688a453fc88c8d71d9e6a",
    "kotodama_quantity_sub": "b9891496bafd7854c9a112b427115b97933decf2df47c28e56003c03536393b8",
    "kotodama_quantity_mul_decimal": "af006a6f20faae2139f1a5b154d1c69f5bdc2c02904810360028fd63e13249af",
    "kotodama_quantity_div_decimal_exact": "37b47aef001ccd89e3bd4a16b311fdec0301778b8cffc24a9a90625613dfa3f0",
    "kotodama_quantity_div_round_floor": "03555a08de20e9c43e666fdda9fdbf155b9adc3b4f7b2e6940a86ee592fc9bdc",
    "kotodama_quantity_div_round_ceil": "088992b8211a234df39bac9bad5d59dd3204f22d794be49f02ca122d53f4a3da",
    "kotodama_quantity_div_round_nearest_even": "878d48f01a6a45799599936592f9188d7dda8a2155638371bd0be12bd68f83e8",
    "kotodama_decimal_add": "5f80aea629f705895bbf0d9f05f564f2ee9e7fe713e9fa71e945b5dd4fb52cdc",
    "kotodama_decimal_sub": "6d216130785ae68e2b1bb9a890eb19335d8768ceeb13a71f7db59a49ac3a75e8",
    "kotodama_decimal_mul": "e0610a72326267d6ae3aa298d21d90986c9192d96a6a0af96bb8a7ce1c6826f6",
    "kotodama_decimal_div_exact": "215405066b95f53ae8103aa9087532927c9785c3b817a2eafc8cb55cd13c4084",
    "kotodama_decimal_div_round_floor": "21a3ef198b78ad43320faf5feee7eca5bc99683581500d19a56460370b0e2cf5",
    "kotodama_decimal_div_round_ceil": "982a1d87fc28dd5604c511ba3fb430749f3ebd1044090966ed78e85a06785bec",
    "kotodama_decimal_div_round_nearest_even": "f5f8abca81ce3896eea1d2ae64de6d6ec2fda222236ddf6ad6ff4e5f5bb874c2",
    "typed_core_query_accounts_page_64": "7ed21c1ff54a4bedea4a076088757bfd636963f059959b0b54180a5a3eba4b64",
    "typed_core_query_assets_page_64": "c785e42d815962065f34d75861b7b6e192e3754f6ee36128bb6978a9576a9455",
    "typed_core_query_asset_definitions_page_64": "057da321707eac734857d824215d3dd2b98efc8cd3382a4b821d31e76c80040b",
    "typed_core_query_domains_page_64": "942d43e4fefbdf768bd1dc67693ecd7078643f26a021879b2f6cfaba0d83e57b",
    "typed_core_query_nfts_page_64": "7271bd2da13604645ec09add36611290fa93646a432d551337b4cec5b1f28496",
    "kotodama_runtime_phase_prepare_validate_predecode": "3b5096d7b272453fcf8b4bf902c9f7f6516dc0bdb2cdbf9fb4bf57acbb77c903",
    "kotodama_runtime_phase_argument_decode": "a6c94994ede14bd3a5163e5740a51aefef43aeb282335b02a0a721a5cca27ef5",
    "kotodama_runtime_phase_load_prepared": "fd31c6a28d828a5d854793c9b7d56f571d39f33aac88735d359957b6b5c07fef",
    "kotodama_runtime_phase_dirty_reset": "eeac94120c46d1bfc846470f5e1d4ed3f94e2105cc5ecc07051e4ddd8311fd79",
    "kotodama_runtime_phase_execute_prepared": "d7cdaf57b514b5d2012878cfec365a2ceccf0cb0364981112ce7f696f3775030",
    "kotodama_runtime_cold_add": "effde7658da204115d374e68fa2e445962920ee412b56fcb37f97a64ba3ee365",
    "kotodama_runtime_warm_add": "ef0b880de04e2d9bf0ffa528688e2721aab6791f7b776b2f943787f28f212776",
    "kotodama_core_runtime_warm_add": "33bec4effbbaac3bdcf312506b3c031737af9d9dad1d8ad9a2446f9b53bc4fa9",
}


class GateError(RuntimeError):
    """Raised when performance evidence is absent or invalid."""


def _benchmark_sources_at(root: Path) -> tuple[Path, ...]:
    """Return the revision-native benchmark source paths under ``root``."""

    return tuple(root / relative for relative in BENCHMARK_SOURCE_PATHS)


def _git_head(root: Path) -> str:
    """Return the exact commit checked out at ``root``."""

    try:
        result = subprocess.run(
            ["git", "-C", str(root), "rev-parse", "--verify", "HEAD^{commit}"],
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
    except OSError as error:
        raise GateError(f"failed to inspect baseline Git revision: {error}") from error
    if result.returncode != 0:
        detail = result.stderr.strip() or f"git exited {result.returncode}"
        raise GateError(f"failed to inspect baseline Git revision: {detail}")
    revision = result.stdout.strip()
    if re.fullmatch(r"[0-9a-f]{40}", revision) is None:
        raise GateError(
            "baseline Git revision must be one lowercase 40-hex commit"
        )
    return revision


def _require_git_paths_clean(root: Path, relative_paths: Sequence[Path]) -> None:
    """Reject tracked changes outside the selected baseline commit."""

    command = [
        "git",
        "-C",
        str(root),
        "status",
        "--porcelain=v1",
        "-z",
        "--untracked-files=all",
    ]
    if relative_paths:
        command.extend(("--", *(relative.as_posix() for relative in relative_paths)))
    try:
        result = subprocess.run(
            command,
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
    except OSError as error:
        raise GateError(
            f"failed to verify baseline benchmark source provenance: {error}"
        ) from error
    if result.returncode == 0 and result.stdout:
        raise GateError(
            "baseline checkout contains tracked or untracked source drift"
        )
    if result.returncode != 0:
        detail = result.stderr.decode("utf-8", errors="replace").strip()
        detail = detail or f"git exited {result.returncode}"
        raise GateError(
            f"failed to verify baseline benchmark source provenance: {detail}"
        )


def _sha256_regular_file(path: Path, label: str) -> str:
    """Hash one required regular, non-symlink provenance file."""

    if path.is_symlink() or not path.is_file():
        raise GateError(f"{label} must be a regular, non-symlink file: {path}")
    digest = hashlib.sha256()
    try:
        with path.open("rb") as source:
            for chunk in iter(lambda: source.read(1024 * 1024), b""):
                digest.update(chunk)
    except OSError as error:
        raise GateError(f"failed to hash {label} {path}: {error}") from error
    return digest.hexdigest()


def validate_baseline_provenance(
    baseline_root: Path,
    candidate_sources: Sequence[Path] = BENCHMARK_SOURCES,
) -> None:
    """Require an exact source and lockfile identity before median comparison.

    The selected baseline commit and its original ``Cargo.lock`` digest are
    policy-pinned; both must match before runner-local timing evidence is read.
    """

    expected_lock = BASELINE_CARGO_LOCK_SHA256
    if expected_lock is None:
        raise GateError(
            "authenticated baseline Cargo.lock provenance is unavailable; "
            "the <=5% comparison is disabled"
        )
    if re.fullmatch(r"[0-9a-f]{64}", expected_lock) is None:
        raise GateError(
            "baseline Cargo.lock policy digest must be lowercase 64-hex"
        )
    try:
        root = baseline_root.resolve(strict=True)
    except OSError as error:
        raise GateError(
            f"failed to resolve baseline checkout {baseline_root}: {error}"
        ) from error
    if not root.is_dir():
        raise GateError(f"baseline checkout is not a directory: {root}")

    revision = _git_head(root)
    if revision != BASELINE_SHA:
        raise GateError(
            "baseline checkout revision mismatch: "
            f"expected {BASELINE_SHA}, got {revision}"
        )
    _require_git_paths_clean(root, ())

    actual_lock = _sha256_regular_file(
        root / "Cargo.lock", "baseline Cargo.lock"
    )
    if actual_lock != expected_lock:
        raise GateError(
            "baseline Cargo.lock digest mismatch: "
            f"expected {expected_lock}, got {actual_lock}"
        )

    validate_revision_inventories(
        base_sources=_benchmark_sources_at(root),
        candidate_sources=candidate_sources,
    )


def validate_candidate_provenance(
    candidate_root: Path, expected_revision: str
) -> tuple[str, str]:
    """Return the exact clean candidate revision and lockfile digest."""

    if re.fullmatch(r"[0-9a-f]{40}", expected_revision) is None:
        raise GateError(
            "expected candidate revision must be one lowercase 40-hex commit"
        )
    if candidate_root.is_symlink():
        raise GateError("candidate checkout must not be a symlink")
    try:
        root = candidate_root.resolve(strict=True)
    except OSError as error:
        raise GateError(
            f"failed to resolve candidate checkout {candidate_root}: {error}"
        ) from error
    if not root.is_dir():
        raise GateError(f"candidate checkout is not a directory: {root}")

    revision = _git_head(root)
    if revision != expected_revision:
        raise GateError(
            "candidate checkout revision mismatch: "
            f"expected {expected_revision}, got {revision}"
        )
    try:
        result = subprocess.run(
            [
                "git",
                "-C",
                str(root),
                "status",
                "--porcelain=v1",
                "-z",
                "--untracked-files=all",
            ],
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
    except OSError as error:
        raise GateError(
            f"failed to verify candidate source provenance: {error}"
        ) from error
    if result.returncode != 0:
        detail = result.stderr.decode("utf-8", errors="replace").strip()
        detail = detail or f"git exited {result.returncode}"
        raise GateError(
            f"failed to verify candidate source provenance: {detail}"
        )
    if result.stdout:
        raise GateError(
            "candidate checkout contains tracked or untracked source drift"
        )
    lock_digest = _sha256_regular_file(
        root / "Cargo.lock", "candidate Cargo.lock"
    )
    if lock_digest != CANDIDATE_CARGO_LOCK_SHA256:
        raise GateError(
            "candidate Cargo.lock digest mismatch: expected "
            f"{CANDIDATE_CARGO_LOCK_SHA256}, got {lock_digest}"
        )
    return revision, lock_digest


def criterion_estimates_sha256(criterion_dir: Path) -> str:
    """Hash every required base/new estimate with its relative path."""

    if criterion_dir.is_symlink() or not criterion_dir.is_dir():
        raise GateError(
            "Criterion directory must be a regular, non-symlink directory: "
            f"{criterion_dir}"
        )
    digest = hashlib.sha256()
    for name in sorted(REGRESSION_BENCHMARKS):
        for sample in ("base", "new"):
            relative_path = Path(name, sample, "estimates.json")
            current = criterion_dir
            for component in relative_path.parts[:-1]:
                current /= component
                if current.is_symlink() or not current.is_dir():
                    raise GateError(
                        "Criterion estimate parent must be a regular, "
                        f"non-symlink directory: {current}"
                    )
            path = criterion_dir / relative_path
            if path.is_symlink() or not path.is_file():
                raise GateError(
                    "Criterion estimate must be a regular, non-symlink file: "
                    f"{path}"
                )
            relative = relative_path.as_posix().encode("utf-8")
            try:
                payload = path.read_bytes()
            except OSError as error:
                raise GateError(
                    f"failed to read Criterion estimate {path}: {error}"
                ) from error
            digest.update(len(relative).to_bytes(8, "big"))
            digest.update(relative)
            digest.update(len(payload).to_bytes(8, "big"))
            digest.update(payload)
    return digest.hexdigest()


def write_evidence_report(
    output: Path,
    *,
    criterion_dir: Path,
    comparisons: Sequence[Comparison],
    threshold: float,
    candidate_revision: str,
    candidate_lock_sha256: str,
) -> None:
    """Create one source- and sample-bound machine-readable gate receipt."""

    parent = output.parent
    if parent.is_symlink() or not parent.is_dir():
        raise GateError(
            f"evidence output parent must be a regular directory: {parent}"
        )
    report = {
        "schema": EVIDENCE_SCHEMA,
        "threshold": threshold,
        "benchmark_count": len(comparisons),
        "criterion_estimates_sha256": criterion_estimates_sha256(criterion_dir),
        "baseline": {
            "revision": BASELINE_SHA,
            "cargo_lock_sha256": BASELINE_CARGO_LOCK_SHA256,
        },
        "candidate": {
            "revision": candidate_revision,
            "cargo_lock_sha256": candidate_lock_sha256,
        },
        "list_sugar": {
            "benchmark": LIST_SUGAR_BENCHMARK,
            "manual_benchmark": LIST_MANUAL_BENCHMARK,
            "maximum_slowdown": LIST_SUGAR_MAX_SLOWDOWN,
        },
        "comparisons": [
            {
                "name": row.name,
                "baseline_ns": row.baseline_ns,
                "measured_ns": row.measured_ns,
                "change": row.change,
            }
            for row in comparisons
        ],
    }
    try:
        with output.open("x", encoding="utf-8", newline="\n") as destination:
            json.dump(report, destination, indent=2, sort_keys=True)
            destination.write("\n")
    except FileExistsError as error:
        raise GateError(f"evidence output already exists: {output}") from error
    except OSError as error:
        raise GateError(
            f"failed to write evidence output {output}: {error}"
        ) from error


_OPEN_DELIMITERS = {"(": ")", "[": "]", "{": "}"}
_CLOSE_DELIMITERS = {value: key for key, value in _OPEN_DELIMITERS.items()}


def _skip_rust_non_code(text: str, index: int) -> int | None:
    """Return the first index after a Rust comment or literal, if one starts here."""

    if text.startswith("//", index):
        newline = text.find("\n", index + 2)
        return len(text) if newline < 0 else newline + 1
    if text.startswith("/*", index):
        depth = 1
        cursor = index + 2
        while cursor < len(text):
            if text.startswith("/*", cursor):
                depth += 1
                cursor += 2
            elif text.startswith("*/", cursor):
                depth -= 1
                cursor += 2
                if depth == 0:
                    return cursor
            else:
                cursor += 1
        return len(text)

    raw_prefix = None
    for prefix in ("br", "r"):
        if text.startswith(prefix, index):
            cursor = index + len(prefix)
            while cursor < len(text) and text[cursor] == "#":
                cursor += 1
            if cursor < len(text) and text[cursor] == '"':
                raw_prefix = text[index:cursor]
                break
    if raw_prefix is not None:
        hashes = raw_prefix.count("#")
        terminator = '"' + ("#" * hashes)
        end = text.find(terminator, index + len(raw_prefix) + 1)
        return len(text) if end < 0 else end + len(terminator)

    if text[index] not in {'"', "'"}:
        return None
    quote = text[index]
    cursor = index + 1
    while cursor < len(text):
        if text[cursor] == "\\":
            cursor += 2
            continue
        if text[cursor] == quote:
            return cursor + 1
        cursor += 1
    return len(text)


def _matching_rust_delimiter(
    text: str, open_index: int, revision: str, context: str
) -> int:
    """Find a balanced Rust delimiter while ignoring comments and literals."""

    opener = text[open_index]
    if opener not in _OPEN_DELIMITERS:
        raise GateError(f"{revision} {context} has no opening delimiter")
    stack = [_OPEN_DELIMITERS[opener]]
    cursor = open_index + 1
    while cursor < len(text):
        skipped = _skip_rust_non_code(text, cursor)
        if skipped is not None:
            cursor = skipped
            continue
        char = text[cursor]
        if char in _OPEN_DELIMITERS:
            stack.append(_OPEN_DELIMITERS[char])
        elif char in _CLOSE_DELIMITERS:
            if not stack or char != stack.pop():
                raise GateError(f"{revision} {context} has mismatched delimiters")
            if not stack:
                return cursor
        cursor += 1
    raise GateError(f"{revision} {context} has an unterminated delimiter")


def _top_level_rust_arguments(
    text: str, open_index: int, close_index: int, revision: str, context: str
) -> list[str]:
    """Split one Rust call's arguments without parsing nested expressions."""

    starts = [open_index + 1]
    stack: list[str] = []
    cursor = open_index + 1
    while cursor < close_index:
        skipped = _skip_rust_non_code(text, cursor)
        if skipped is not None:
            cursor = skipped
            continue
        char = text[cursor]
        if char in _OPEN_DELIMITERS:
            stack.append(_OPEN_DELIMITERS[char])
        elif char in _CLOSE_DELIMITERS:
            if not stack or char != stack.pop():
                raise GateError(f"{revision} {context} has mismatched arguments")
        elif char == "," and not stack:
            starts.append(cursor + 1)
        cursor += 1
    if stack:
        raise GateError(f"{revision} {context} has unterminated arguments")

    boundaries = [start - 1 for start in starts[1:]] + [close_index]
    if len(starts) != len(boundaries):
        raise GateError(f"{revision} {context} has inconsistent argument boundaries")
    return [text[start:end].strip() for start, end in zip(starts, boundaries)]


def _rust_function_from_marker(
    text: str, marker: str, revision: str, context: str
) -> str:
    """Extract one uniquely marked Rust function."""

    if text.count(marker) != 1:
        raise GateError(f"{revision} {context} function must be declared exactly once")
    start = text.index(marker)
    body_open = text.find("{", start + len(marker))
    if body_open < 0:
        raise GateError(f"{revision} {context} function body is missing")
    body_close = _matching_rust_delimiter(
        text, body_open, revision, f"{context} function"
    )
    return " ".join(text[start : body_close + 1].split())


def _rust_statement_from_marker(
    text: str, marker: str, revision: str, context: str
) -> str:
    """Extract one uniquely marked top-level Rust statement."""

    if text.count(marker) != 1:
        raise GateError(f"{revision} {context} statement must be declared exactly once")
    start = text.index(marker)
    stack: list[str] = []
    cursor = start
    while cursor < len(text):
        skipped = _skip_rust_non_code(text, cursor)
        if skipped is not None:
            cursor = skipped
            continue
        char = text[cursor]
        if char in _OPEN_DELIMITERS:
            stack.append(_OPEN_DELIMITERS[char])
        elif char in _CLOSE_DELIMITERS:
            if not stack or char != stack.pop():
                raise GateError(f"{revision} {context} statement is malformed")
        elif char == ";" and not stack:
            return " ".join(text[start : cursor + 1].split())
        cursor += 1
    raise GateError(f"{revision} {context} statement is unterminated")


def _criterion_invocation(
    text: str, marker: str, revision: str, context: str, start: int = 0
) -> str:
    marker_index = text.find(marker, start)
    if marker_index < 0:
        raise GateError(f"{revision} {context} Criterion declaration is missing")
    invocation_open = text.find("(", marker_index + len("c.bench_function"))
    if invocation_open < 0:
        raise GateError(f"{revision} {context} Criterion call is malformed")
    invocation_close = _matching_rust_delimiter(
        text, invocation_open, revision, f"{context} Criterion call"
    )
    return text[marker_index : invocation_close + 1]


def _criterion_timed_closure(invocation: str, revision: str, context: str) -> str:
    batched_marker = "b.iter_batched("
    iter_marker = "b.iter("
    if batched_marker in invocation:
        marker = batched_marker
        argument_index = 1
    elif iter_marker in invocation:
        marker = iter_marker
        argument_index = 0
    else:
        raise GateError(f"{revision} {context} has no Criterion timed iterator")
    if invocation.count(marker) != 1:
        raise GateError(f"{revision} {context} timed iterator must be unique")
    call_start = invocation.index(marker)
    call_open = call_start + len(marker) - 1
    call_close = _matching_rust_delimiter(
        invocation, call_open, revision, f"{context} timed iterator"
    )
    arguments = _top_level_rust_arguments(
        invocation,
        call_open,
        call_close,
        revision,
        f"{context} timed iterator",
    )
    if argument_index >= len(arguments):
        raise GateError(f"{revision} {context} timed closure is missing")
    return " ".join(arguments[argument_index].split())


def _canonicalize_comparable_timed_body(
    name: str, body: str, revision: str
) -> str:
    """Normalize one API-only spelling while preserving workload identity."""

    if name != CORE_RUNTIME_WARM_BENCHMARK:
        return body
    if body.count(".checkout_runtime(") != 1:
        raise GateError(
            f"{revision} {name} must contain exactly one runtime checkout"
        )
    if CORE_RUNTIME_EXPLICIT_MAX_CHECKOUT in body:
        if body.count(CORE_RUNTIME_EXPLICIT_MAX_CHECKOUT) != 1:
            raise GateError(
                f"{revision} {name} explicit max-heap checkout must be unique"
            )
        return body.replace(
            CORE_RUNTIME_EXPLICIT_MAX_CHECKOUT,
            CORE_RUNTIME_BASELINE_CHECKOUT,
            1,
        )
    if CORE_RUNTIME_BASELINE_CHECKOUT in body:
        if body.count(CORE_RUNTIME_BASELINE_CHECKOUT) != 1:
            raise GateError(
                f"{revision} {name} baseline checkout must be unique"
            )
        return body
    raise GateError(
        f"{revision} {name} must use the baseline-equivalent heap limit"
    )


def _quoted_identity_binding(text: str, name: str, revision: str) -> str:
    marker = f'"{name}"'
    if text.count(marker) != 1:
        raise GateError(f"{revision} {name} identity binding must be unique")
    identity = text.index(marker)
    tuple_open = text.rfind("(", 0, identity)
    if tuple_open < 0:
        raise GateError(f"{revision} {name} identity tuple is missing")
    tuple_close = _matching_rust_delimiter(
        text, tuple_open, revision, f"{name} identity tuple"
    )
    if tuple_close < identity:
        raise GateError(f"{revision} {name} identity tuple is malformed")
    return " ".join(text[tuple_open : tuple_close + 1].split())


def _typed_query_identity_binding(text: str, name: str, revision: str) -> str:
    marker = f'"{name}"'
    if text.count(marker) != 1:
        raise GateError(f"{revision} {name} typed-query binding must be unique")
    identity = text.index(marker)
    declaration = text.rfind("TypedCoreQueryFamily {", 0, identity)
    if declaration < 0:
        raise GateError(f"{revision} {name} typed-query declaration is missing")
    declaration_open = text.index("{", declaration)
    declaration_close = _matching_rust_delimiter(
        text, declaration_open, revision, f"{name} typed-query declaration"
    )
    if declaration_close < identity:
        raise GateError(f"{revision} {name} typed-query declaration is malformed")
    return " ".join(text[declaration : declaration_close + 1].split())


def _benchmark_timed_bodies_from_text(text: str, revision: str) -> dict[str, str]:
    """Extract every baseline-comparable native Criterion timed closure."""

    bodies = {}
    for name in STATIC_REGRESSION_BENCHMARKS:
        marker = f'c.bench_function("{name}",'
        if text.count(marker) != 1:
            raise GateError(
                f"{revision} {name} Criterion declaration must be unique"
            )
        invocation = _criterion_invocation(text, marker, revision, name)
        body = _criterion_timed_closure(invocation, revision, name)
        bodies[name] = _canonicalize_comparable_timed_body(
            name, body, revision
        )

    decimal_anchor = text.index(f'"{DECIMAL_ROUND_BENCHMARKS[0]}"')
    decimal_invocation = _criterion_invocation(
        text,
        "c.bench_function(name,",
        revision,
        "rounded Decimal",
        decimal_anchor,
    )
    decimal_body = _criterion_timed_closure(
        decimal_invocation, revision, "rounded Decimal"
    )
    for name in DECIMAL_ROUND_BENCHMARKS:
        binding = _quoted_identity_binding(text, name, revision)
        bodies[name] = f"{binding} => {decimal_body}"

    quantity_anchor = text.index(f'"{QUANTITY_ROUND_BENCHMARKS[0]}"')
    quantity_invocation = _criterion_invocation(
        text,
        "c.bench_function(name,",
        revision,
        "rounded Quantity",
        quantity_anchor,
    )
    quantity_body = _criterion_timed_closure(
        quantity_invocation, revision, "rounded Quantity"
    )
    for name in QUANTITY_ROUND_BENCHMARKS:
        binding = _quoted_identity_binding(text, name, revision)
        bodies[name] = f"{binding} => {quantity_body}"

    typed_invocation = _criterion_invocation(
        text,
        TYPED_QUERY_BENCHMARK_MARKER,
        revision,
        "typed-query family",
    )
    typed_body = _criterion_timed_closure(
        typed_invocation, revision, "typed-query family"
    )
    for name in TYPED_QUERY_BENCHMARKS:
        binding = _typed_query_identity_binding(text, name, revision)
        bodies[name] = f"{binding} => {typed_body}"

    if set(bodies) != set(REGRESSION_BENCHMARKS):
        missing = sorted(set(REGRESSION_BENCHMARKS) - set(bodies))
        extra = sorted(set(bodies) - set(REGRESSION_BENCHMARKS))
        raise GateError(
            f"{revision} timed-body coverage mismatch "
            f"(missing: {', '.join(missing)}; unexpected: {', '.join(extra)})"
        )
    return bodies


def _benchmark_timed_bodies(
    sources: Sequence[Path], revision: str
) -> dict[str, str]:
    return _benchmark_timed_bodies_from_text(_read_sources(sources, revision), revision)


def validate_comparable_timed_bodies(
    sources: Sequence[Path], revision: str
) -> dict[str, str]:
    """Bind every comparable identity to the selected baseline workload."""

    bodies = _benchmark_timed_bodies(sources, revision)
    expected_names = set(REGRESSION_BENCHMARKS)
    if set(BASELINE_TIMED_BODY_SHA256) != expected_names:
        raise GateError("baseline timed-body policy coverage is incomplete")
    failures = []
    for name in REGRESSION_BENCHMARKS:
        actual = hashlib.sha256(bodies[name].encode("utf-8")).hexdigest()
        expected = BASELINE_TIMED_BODY_SHA256[name]
        if actual != expected:
            failures.append(f"{name}: expected {expected}, got {actual}")
    if failures:
        raise GateError(
            f"{revision} comparable timed-body drift from {BASELINE_SHA}:\n  "
            + "\n  ".join(failures)
        )
    return bodies


def _read_sources(sources: Sequence[Path], revision: str) -> str:
    texts = []
    for source in sources:
        try:
            texts.append(source.read_text(encoding="utf-8"))
        except OSError as error:
            raise GateError(
                f"failed to read {revision} benchmark inventory {source}: {error}"
            ) from error
    return "\n".join(texts)


def _identity_counts(text: str, benchmarks: Sequence[str]) -> dict[str, int]:
    return {name: text.count(f'"{name}"') for name in benchmarks}


def _require_identity_counts(
    counts: Mapping[str, int], expected: int, revision: str
) -> None:
    missing = sorted(name for name, count in counts.items() if count < expected)
    duplicated = sorted(name for name, count in counts.items() if count > expected)
    if missing or duplicated:
        details = []
        if missing:
            details.append("missing: " + ", ".join(missing))
        if duplicated:
            details.append("duplicated: " + ", ".join(duplicated))
        raise GateError(
            f"{revision} benchmark inventory mismatch (" + "; ".join(details) + ")"
        )


def _typed_query_timed_body(sources: Sequence[Path], revision: str) -> str:
    """Extract and validate the comparable typed-query Criterion workload."""

    text = _read_sources(sources, revision)
    if text.count(TYPED_QUERY_BENCHMARK_MARKER) != 1:
        raise GateError(
            f"{revision} typed-query family benchmark must be declared exactly once"
        )
    benchmark_start = text.index(TYPED_QUERY_BENCHMARK_MARKER)
    body_end = text.find(TYPED_QUERY_TIMED_BODY_END, benchmark_start)
    if body_end < 0:
        raise GateError(f"{revision} typed-query timed body end is missing")
    body_start = text.find(
        TYPED_QUERY_TIMED_BODY_MARKER, benchmark_start, body_end
    )
    if body_start < 0:
        raise GateError(f"{revision} typed-query timed body is missing")
    body = " ".join(text[body_start:body_end].split())
    cursor = 0
    for marker in TYPED_QUERY_TIMED_CONTRACT_MARKERS:
        offset = body.find(marker, cursor)
        if offset < 0:
            raise GateError(
                f"{revision} typed-query timed contract is missing or reordered: "
                f"{marker}"
            )
        cursor = offset + len(marker)
    return body


def _typed_query_raw_entity_contract(
    sources: Sequence[Path], revision: str
) -> str:
    """Extract the full-entity response used as the projection-size baseline."""

    text = _read_sources(sources, revision)
    contract = " ".join(
        (
            _rust_function_from_marker(
                text,
                TYPED_QUERY_RAW_RESPONSE_FUNCTION,
                revision,
                "typed-query raw response",
            ),
            _rust_statement_from_marker(
                text,
                TYPED_QUERY_RAW_BYTES_STATEMENT,
                revision,
                "typed-query raw response bytes",
            ),
        )
    )
    cursor = 0
    for marker in TYPED_QUERY_RAW_CONTRACT_MARKERS:
        offset = contract.find(marker, cursor)
        if offset < 0:
            raise GateError(
                f"{revision} typed-query raw-entity contract is missing or "
                f"reordered: {marker}"
            )
        cursor = offset + len(marker)
    return contract


def validate_typed_query_raw_entity_contract(
    base_sources: Sequence[Path], candidate_sources: Sequence[Path]
) -> None:
    """Require the comparable projection-size baseline to remain identical."""

    base = _typed_query_raw_entity_contract(base_sources, "base")
    candidate = _typed_query_raw_entity_contract(candidate_sources, "candidate")
    if candidate != base:
        raise GateError(
            "comparable typed-query raw-entity baseline drift; preserve the "
            "exact full-entity QueryResponse workload or rename and reclassify "
            "all five typed query benchmark identities"
        )


def validate_benchmark_policy(
    sources: Sequence[Path] = BENCHMARK_SOURCES,
) -> dict[str, str]:
    """Reject duplicate, incomplete, stale, or misclassified benchmark coverage."""

    representative = set(REPRESENTATIVE_BENCHMARKS)
    comparable = set(REGRESSION_BENCHMARKS)
    if len(representative) != len(REPRESENTATIVE_BENCHMARKS):
        raise GateError("representative benchmark policy contains duplicate identities")
    if len(set(SOURCE_BOUND_BENCHMARKS)) != len(SOURCE_BOUND_BENCHMARKS):
        raise GateError("source-bound benchmark policy contains duplicate identities")
    if comparable != representative:
        raise GateError(
            "comparable benchmark policy must exactly cover every representative "
            "benchmark at the current baseline"
        )

    text = _read_sources(sources, "candidate")
    _require_identity_counts(
        _identity_counts(text, REPRESENTATIVE_BENCHMARKS), 1, "candidate"
    )

    protected_counts = collections.Counter(PROTECTED_BENCHMARK_PATTERN.findall(text))
    source_bound = set(SOURCE_BOUND_BENCHMARKS)
    unexpected = sorted(set(protected_counts) - source_bound)
    stale = sorted(source_bound - set(protected_counts))
    duplicated = sorted(name for name, count in protected_counts.items() if count != 1)
    if unexpected or stale or duplicated:
        details = []
        if unexpected:
            details.append("missing from policy: " + ", ".join(unexpected))
        if stale:
            details.append("not declared by benchmark: " + ", ".join(stale))
        if duplicated:
            details.append("declared more than once: " + ", ".join(duplicated))
        raise GateError(
            "source-bound benchmark coverage drift (" + "; ".join(details) + ")"
        )
    _typed_query_raw_entity_contract(sources, "candidate")
    _typed_query_timed_body(sources, "candidate")
    return validate_comparable_timed_bodies(sources, "candidate")


def validate_revision_inventories(
    base_sources: Sequence[Path],
    candidate_sources: Sequence[Path] = BENCHMARK_SOURCES,
) -> None:
    """Require native base/candidate sources to match the provenance policy."""

    candidate_bodies = validate_benchmark_policy(candidate_sources)
    base_text = _read_sources(base_sources, "base")
    _require_identity_counts(
        _identity_counts(base_text, REGRESSION_BENCHMARKS), 1, "base"
    )
    base_bodies = validate_comparable_timed_bodies(base_sources, "base")
    if candidate_bodies != base_bodies:
        drifted = sorted(
            name
            for name in REGRESSION_BENCHMARKS
            if candidate_bodies[name] != base_bodies[name]
        )
        raise GateError(
            "comparable timed-body equality drift: " + ", ".join(drifted)
        )
    base_timed_body = _typed_query_timed_body(base_sources, "base")
    candidate_timed_body = _typed_query_timed_body(
        candidate_sources, "candidate"
    )
    if candidate_timed_body != base_timed_body:
        raise GateError(
            "comparable typed-query timed body drift; preserve the exact "
            "baseline workload or rename and reclassify all five typed "
            "query benchmark identities"
        )
    validate_typed_query_raw_entity_contract(base_sources, candidate_sources)


@dataclass(frozen=True)
class Comparison:
    """One benchmark comparison, expressed in Criterion nanoseconds."""

    name: str
    baseline_ns: float
    measured_ns: float

    @property
    def change(self) -> float:
        return self.measured_ns / self.baseline_ns - 1.0


def _positive_finite(value: object, context: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise GateError(f"{context} must be a number")
    number = float(value)
    if not math.isfinite(number) or number <= 0.0:
        raise GateError(f"{context} must be finite and positive")
    return number


def read_criterion_median(path: Path) -> float:
    """Read ``median.point_estimate`` from one Criterion estimates file."""

    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except OSError as error:
        raise GateError(f"failed to read Criterion sample {path}: {error}") from error
    except json.JSONDecodeError as error:
        raise GateError(f"invalid Criterion JSON {path}: {error}") from error
    try:
        value = payload["median"]["point_estimate"]
    except (KeyError, TypeError) as error:
        raise GateError(
            f"Criterion sample {path} is missing median.point_estimate"
        ) from error
    return _positive_finite(value, f"Criterion median in {path}")


def read_current_samples(
    criterion_dir: Path, benchmarks: Sequence[str]
) -> dict[str, float]:
    """Read the current ``new`` medians for the selected benchmarks."""

    return {
        name: read_criterion_median(
            criterion_dir / name / "new" / "estimates.json"
        )
        for name in benchmarks
    }


def read_criterion_base(
    criterion_dir: Path, benchmarks: Sequence[str]
) -> dict[str, float]:
    """Read Criterion's previous-run ``base`` medians."""

    return {
        name: read_criterion_median(
            criterion_dir / name / "base" / "estimates.json"
        )
        for name in benchmarks
    }


def compare_samples(
    baseline: Mapping[str, float], measured: Mapping[str, float]
) -> list[Comparison]:
    """Build deterministic comparisons and reject mismatched coverage."""

    if set(baseline) != set(measured):
        missing = sorted(set(baseline) - set(measured))
        extra = sorted(set(measured) - set(baseline))
        details = []
        if missing:
            details.append("missing measured: " + ", ".join(missing))
        if extra:
            details.append("unexpected measured: " + ", ".join(extra))
        raise GateError("benchmark coverage mismatch (" + "; ".join(details) + ")")
    return [
        Comparison(name, baseline[name], measured[name]) for name in sorted(baseline)
    ]


def enforce(comparisons: Sequence[Comparison], threshold: float) -> None:
    """Raise when any benchmark exceeds the configured slowdown budget."""

    if not math.isfinite(threshold) or threshold < 0.0 or threshold > MAX_REGRESSION:
        raise GateError(
            f"threshold must be between 0 and {MAX_REGRESSION:.2f}; "
            "the V1 gate cannot be loosened above five percent"
        )
    # Decimal Criterion values and binary floating point can represent an
    # exact 5% boundary as 5.000000000000004%. Keep the policy inclusive while
    # allowing only machine-rounding noise, not a measurable relaxation.
    failures = [row for row in comparisons if row.change - threshold > 1e-12]
    if failures:
        details = "\n".join(
            f"  {row.name}: {row.change * 100.0:+.2f}% "
            f"({row.baseline_ns:.0f} ns -> {row.measured_ns:.0f} ns)"
            for row in failures
        )
        raise GateError(
            f"Kotodama performance regression exceeds {threshold * 100.0:.2f}%:\n"
            + details
        )


def enforce_list_sugar(samples: Mapping[str, float]) -> None:
    """Require comprehension sugar to be no slower than the manual loop."""
    try:
        sugar = _positive_finite(
            samples[LIST_SUGAR_BENCHMARK], "List comprehension runtime median"
        )
        manual = _positive_finite(
            samples[LIST_MANUAL_BENCHMARK], "manual List runtime median"
        )
    except KeyError as error:
        raise GateError(f"missing List runtime comparison sample {error.args[0]}") from error
    change = sugar / manual - 1.0
    if change - LIST_SUGAR_MAX_SLOWDOWN > 1e-12:
        raise GateError(
            "Kotodama List comprehension sugar exceeds the manual-loop "
            f"baseline by {change * 100.0:+.2f}% "
            f"({manual:.0f} ns -> {sugar:.0f} ns; "
            "the V1 sugar path must be no slower)"
        )


def render(comparisons: Sequence[Comparison]) -> str:
    """Render a compact, deterministic comparison table."""

    rows = ["benchmark | baseline ns | measured ns | change", "---|---:|---:|---:"]
    rows.extend(
        f"{row.name} | {row.baseline_ns:.0f} | {row.measured_ns:.0f} | "
        f"{row.change * 100.0:+.2f}%"
        for row in comparisons
    )
    return "\n".join(rows)


def parse_args(argv: Sequence[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    parser.add_argument(
        "--criterion-dir", type=Path, default=Path("target/criterion")
    )
    parser.add_argument(
        "--baseline-root",
        type=Path,
        required=True,
        help=(
            "exact checkout of the policy-pinned baseline; its Git revision, "
            "native benchmark sources, and authenticated original Cargo.lock "
            "must match before comparison"
        ),
    )
    parser.add_argument("--threshold", type=float, default=MAX_REGRESSION)
    parser.add_argument(
        "--candidate-root",
        type=Path,
        help=(
            "clean candidate checkout to bind into an evidence receipt; "
            "required with --json-output"
        ),
    )
    parser.add_argument(
        "--expected-candidate-commit",
        help=(
            "exact lowercase 40-hex candidate commit; required with "
            "--json-output"
        ),
    )
    parser.add_argument(
        "--json-output",
        type=Path,
        help=(
            "create one machine-readable source/lock/sample-bound receipt; "
            "the path must not already exist"
        ),
    )
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(sys.argv[1:] if argv is None else argv)
    try:
        evidence_values = (
            args.candidate_root,
            args.expected_candidate_commit,
            args.json_output,
        )
        if any(value is not None for value in evidence_values) and not all(
            value is not None for value in evidence_values
        ):
            raise GateError(
                "--candidate-root, --expected-candidate-commit, and "
                "--json-output must be supplied together"
            )
        validate_benchmark_policy()
        validate_baseline_provenance(args.baseline_root)
        current = read_current_samples(
            args.criterion_dir, REPRESENTATIVE_BENCHMARKS
        )
        enforce_list_sugar(current)
        baseline = read_criterion_base(args.criterion_dir, REGRESSION_BENCHMARKS)
        comparisons = compare_samples(
            baseline,
            {name: current[name] for name in REGRESSION_BENCHMARKS},
        )
        print(render(comparisons))
        enforce(comparisons, args.threshold)
        if args.json_output is not None:
            candidate_revision, candidate_lock_sha256 = (
                validate_candidate_provenance(
                    args.candidate_root, args.expected_candidate_commit
                )
            )
            write_evidence_report(
                args.json_output,
                criterion_dir=args.criterion_dir,
                comparisons=comparisons,
                threshold=args.threshold,
                candidate_revision=candidate_revision,
                candidate_lock_sha256=candidate_lock_sha256,
            )
    except GateError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    print(
        "All comparable Kotodama medians are within the 5% V1 budget, every "
        "required workload is present, and List sugar is no slower than its "
        "manual-loop baseline."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
