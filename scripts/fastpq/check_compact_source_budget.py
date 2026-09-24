#!/usr/bin/env python3
"""Check FASTPQ's source-owned wire geometry; this does not qualify proofs."""

from __future__ import annotations

import json
import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SOURCES = {
    "profile": "crates/fastpq_prover/src/backend/compact_protocol/profile.rs",
    "row": "crates/fastpq_prover/src/backend/compact_protocol/shared_openings/row_values.rs",
    "shared": "crates/fastpq_prover/src/backend/compact_protocol/shared_openings.rs",
    "producer": "crates/fastpq_prover/src/backend/compact_quantity_producer.rs",
    "fp4": "crates/fastpq_prover/src/field.rs",
    "digest": "crates/fastpq_isi/src/poseidon_digest384.rs",
    "deep": "crates/fastpq_prover/src/backend/deep_proof.rs",
    "backend": "crates/fastpq_prover/src/backend.rs",
    "axt": "crates/fastpq_prover/src/axt_binding.rs",
    "retained": "crates/fastpq_prover/src/backend/compact_quantity_diagnostic.rs",
}


def load_sources(root: Path = ROOT) -> dict[str, str]:
    """Load the exact files whose geometry and admission rules are screened."""
    return {name: (root / path).read_text() for name, path in SOURCES.items()}


def number(source: str, pattern: str, label: str) -> int:
    """Read one decimal Rust literal, refusing ambiguous or absent bindings."""
    matches = re.findall(pattern, source, flags=re.MULTILINE)
    if len(matches) != 1:
        raise ValueError(f"expected one {label}; found {len(matches)}")
    return int(matches[0].replace("_", ""))


def require(source: str, pattern: str, label: str) -> None:
    """Reject source layouts for which this byte model is no longer justified."""
    if re.search(pattern, source, flags=re.MULTILINE) is None:
        raise ValueError(f"unrecognized {label}; update the budget model")


def field(payload: int) -> int:
    """Canonical Norito field bytes: unsigned base-128 length plus payload."""
    if payload < 0:
        raise ValueError("negative payload")
    prefix = 1
    remaining = payload
    while remaining >= 128:
        remaining >>= 7
        prefix += 1
    return prefix + payload


def vector(count: int, element_payload: int) -> int:
    """A Norito Vec with an eight-byte count and framed elements."""
    return 8 + count * field(element_payload)


def record(*payloads: int) -> int:
    """A Norito struct containing ordered framed fields."""
    return sum(map(field, payloads))


def frontier(leaves: int, opened: int) -> int:
    """Maximum minimal binary frontier for any `opened` distinct leaves."""
    if not leaves or leaves & (leaves - 1) or not 0 < opened <= leaves:
        raise ValueError("invalid Merkle opening geometry")
    return sum(min(opened, 1 << level) for level in range(leaves.bit_length() - 1)) - opened + 1


def budget(sources: dict[str, str]) -> dict[str, object]:
    """Derive a conservative current-frame bound and read candidate boundaries."""
    profile = sources["profile"].split("#[derive", 1)[0]
    query_count = number(profile, r"const QUERY_COUNT: usize = ([\d_]+);", "query count")
    trace_rows = number(profile, r"geometry\.schema\.trace_rows != ([\d_]+)", "trace rows")
    width = number(profile, r"geometry\.schema\.width != ([\d_]+)", "committed width")
    constraints = number(profile, r"geometry\.schema\.constraints != ([\d_]+)", "constraints")
    lde_rows = number(profile, r"geometry\.lde_rows != ([\d_]+)", "LDE rows")
    arity = number(profile, r"FASTPQ_FINAL_V1\.fri\.arity != ([\d_]+)", "FRI arity")
    folds = number(sources["profile"], r"folds: ([\d_]+),", "FRI folds")
    terminal = number(sources["profile"], r"terminal_values: ([\d_]+),", "terminal size")
    row_width = number(sources["row"], r"struct RowValues\(\[u64; ([\d_]+)\]\);", "row codec width")
    declared_width = number(sources["row"], r"const WIDTH: usize = ([\d_]+);", "row width")
    fp4_bytes = number(sources["fp4"], r"pub const BYTES: usize = ([\d_]+);", "Fp4 bytes")
    lanes = number(sources["digest"], r"GOLDILOCKS_DIGEST384_LANES_V1: usize = ([\d_]+);", "digest lanes")
    segment_cap = number(sources["deep"], r"PROOF_BYTE_TARGET: usize = ([\d_]+) \* 1024;", "segment KiB cap") * 1024
    axt_cap = number(sources["axt"], r"DEFAULT_MAX_AXT_FASTPQ_PAYLOAD_BYTES: usize = ([\d_]+) \* 1024;", "AXT KiB cap") * 1024
    deep_max = number(sources["deep"], r"MAX_FRAME_BYTES: usize = ([\d_]+);", "DEEP DTO bound")
    rust_bound = number(sources["shared"], r"assert_eq!\(bound, ([\d_]+)\);", "Rust current bound")
    producer_bound = number(sources["producer"], r"const SHARED_FRAME_BOUND: usize = ([\d_]+);", "producer preflight bound")

    if width != row_width or width != declared_width or trace_rows * 8 != lde_rows or arity != 2 or lde_rows >> folds != terminal:
        raise ValueError("profile and fixed-row codec geometry disagree")
    require(sources["row"], r"BYTES: usize = Self::WIDTH \* size_of::<u64>\(\)", "fixed row byte width")
    require(sources["row"], r"writer\.write_all\(&value\.to_le_bytes\(\)\)", "canonical row encoding")
    require(sources["shared"], r"rows: Vec<SharedRow>,", "complete row table")
    require(sources["shared"], r"queries: Vec<SharedQuery>,", "distinct mixed/quotient table")
    require(sources["shared"], r"struct SharedRow \{\s*index: u32,\s*values: RowValues,", "row opening")
    require(sources["shared"], r"struct SharedQuery \{\s*index: u32,\s*mixed: GoldilocksFp4V1,\s*quotient: GoldilocksFp4V1,", "scalar openings")
    require(sources["shared"], r"proof\.rows\.len\(\) < query_count", "minimum row count")
    require(sources["shared"], r"proof\.queries\.len\(\) != query_count", "exact query count")
    require(sources["shared"], r"encoded_frame_len\(proof\)\?;\s*check_limit\(\"max_proof_bytes\", bytes, limits\.max_proof_bytes\)", "typed proof byte admission")
    require(sources["digest"], r"GOLDILOCKS_DIGEST384_BYTES_V1: usize = GOLDILOCKS_DIGEST384_LANES_V1 \* 8", "digest byte width")
    require(sources["backend"], r"#\[cfg\(test\)\]\s*#\[path = \"backend/deep_engine.rs\"\]\s*mod deep_engine;", "test-only DEEP verifier")
    require(sources["deep"], r"caller_max_bytes\.min\(MAX_FRAME_BYTES\)", "DEEP decoder cap")
    require(sources["producer"], r"check\(\s*\"max_proof_bytes\",\s*SHARED_FRAME_BOUND,\s*bundle\.segment\.max_proof_bytes", "producer byte preflight")

    if (segment_cap, axt_cap) != (524_288, 1_048_576):
        raise ValueError("fixed first-release proof ceilings changed")

    digest_bytes = lanes * 8
    minimum_rows = query_count * row_width * 8
    minimum_scalars = query_count * 2 * fp4_bytes
    raw_floor = minimum_rows + minimum_scalars
    maximum_rows = min(2 * query_count, lde_rows)
    row_element = record(4, row_width * 8)
    query_element = record(4, fp4_bytes, fp4_bytes)
    group_element = record(4, record(*([fp4_bytes] * arity)))
    rounds_bytes = 8
    fri_values = 0
    fri_frontier = 0
    length = lde_rows
    for _ in range(folds):
        leaves = length // arity
        groups = min(query_count, leaves)
        siblings = frontier(leaves, groups)
        rounds_bytes += field(record(vector(groups, group_element), vector(siblings, digest_bytes)))
        fri_values += groups * arity * fp4_bytes
        fri_frontier += siblings * digest_bytes
        length = leaves
    scalar_frontier = frontier(lde_rows, query_count)
    framed_bound = 40 + record(
        digest_bytes, digest_bytes, digest_bytes,
        vector(folds + 1, digest_bytes),
        vector(maximum_rows, row_element),
        vector(query_count, query_element),
        vector(frontier(lde_rows, maximum_rows), digest_bytes),
        vector(scalar_frontier, digest_bytes),
        vector(scalar_frontier, digest_bytes),
        rounds_bytes,
        vector(terminal, fp4_bytes),
    )
    if framed_bound != rust_bound or framed_bound != producer_bound:
        raise ValueError(f"source-derived frame {framed_bound} differs from Rust/producer bounds")

    retained = {
        name: int(length.replace("_", ""))
        for name, length in re.findall(
            r'read_retained_quantity_wire\(\s*"([^\"]+)"\s*,\s*([\d_]+)',
            sources["retained"],
        )
    }
    if set(retained) != {"ordinary-single", "axt-single", "ordinary-bundle", "axt-bundle"}:
        raise ValueError("retained diagnostic inventory changed")
    if not (raw_floor > axt_cap > segment_cap > deep_max):
        raise ValueError("current/deep proof budget classification changed; review the new protocol")
    return {
        "current": {
            "trace_rows": trace_rows, "columns": width, "constraints": constraints,
            "lde_rows": lde_rows, "queries": query_count, "folds": folds,
            "mandatory_row_bytes": minimum_rows,
            "mandatory_mixed_quotient_bytes": minimum_scalars,
            "mandatory_raw_floor": raw_floor,
            "raw_floor_over_segment_cap": raw_floor - segment_cap,
            "raw_floor_over_axt_cap": raw_floor - axt_cap,
            "framed_maximum": framed_bound,
            "framed_maximum_over_segment_cap": framed_bound - segment_cap,
            "fri_value_bytes_at_independent_maxima": fri_values,
            "fri_frontier_bytes_at_independent_maxima": fri_frontier,
        },
        "limits": {"segment": segment_cap, "axt_inner": axt_cap},
        "retained_test_metadata": retained,
        "deep_test_only_dto": {"max_frame_bytes": deep_max, "headroom": segment_cap - deep_max},
        "production_qualified": False,
    }


def main() -> None:
    """Print a deterministic audit record for the current source checkout."""
    print(json.dumps(budget(load_sources()), indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
