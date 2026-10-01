#!/usr/bin/env python3
"""Check current q77 source-owned bytes and explicit hypothetical screens, not admission."""

from __future__ import annotations

import json
import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SOURCES = {
    "challenge": "crates/fastpq_isi/src/compact_challenge.rs",
    "air": "crates/fastpq_prover/src/backend/compact_transfer_air.rs",
    "engine": "crates/fastpq_prover/src/backend/deep_engine.rs",
    "producer": "crates/fastpq_prover/src/backend/compact_quantity_producer.rs",
    "resources": "crates/fastpq_prover/src/backend/offline_compact/resources.rs",
    "fp4": "crates/fastpq_prover/src/field.rs",
    "digest": "crates/iroha_data_model/src/fastpq/commitment.rs",
    "deep": "crates/fastpq_prover/src/backend/deep_proof.rs",
    "deep_tests": "crates/fastpq_prover/src/backend/deep_proof/tests.rs",
    "deep_row": "crates/fastpq_prover/src/backend/deep_proof/row_values.rs",
    "deep_fri": "crates/fastpq_prover/src/backend/deep_proof/fri_values.rs",
    "deep_geometry": "crates/fastpq_prover/src/backend/deep_geometry.rs",
    "public_columns": "crates/fastpq_prover/src/backend/compact_public_columns.rs",
    "backend": "crates/fastpq_prover/src/backend.rs",
    "axt": "crates/fastpq_prover/src/axt_binding.rs",
    "retained": "crates/fastpq_prover/src/backend/compact_quantity_diagnostic.rs",
    "artifact": "crates/fastpq_prover/src/backend/compact_artifact.rs",
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


def current_geometry(sources: dict[str, str]) -> dict[str, int]:
    """Resolve the fixed q77 owners, refusing a new profile until reviewed."""
    geometry = sources["deep_geometry"]
    require(geometry, r"QUERY_COUNT: usize = fastpq_isi::compact_challenge::QUERY_COUNT;", "canonical DEEP query owner")
    queries = number(sources["challenge"], r"QUERY_COUNT: usize = ([\d_]+);", "q77 query count")
    rows = number(geometry, r"LDE_ROWS: usize = ([\d_]+);", "DEEP LDE rows")
    trace = number(geometry, r"TRACE_ROWS: usize = ([\d_]+);", "DEEP trace rows")
    constraints = number(geometry, r"CONSTRAINTS: usize = ([\d_]+);", "DEEP constraint count")
    width = number(sources["air"], r"COLUMN_COUNT != ([\d_]+)", "complete AIR width")
    public = number(sources["public_columns"], r"PUBLIC_COLUMN_COUNT: usize = ([\d_]+);", "public columns")
    fp4 = number(sources["fp4"], r"pub const BYTES: usize = ([\d_]+);", "Fp4 bytes")
    digest = number(sources["digest"], r"pub const BYTES: usize = ([\d_]+);", "opaque SHA3 digest bytes")
    if (queries, trace, rows, constraints, width, public, fp4, digest) != (77, 65_536, 8_388_608, 923, 342, 41, 32, 32):
        raise ValueError("current q77 geometry changed; review the new source profile")
    for name, expected in (("TRACE_ROWS", trace), ("LDE_ROWS", rows), ("CONSTRAINTS", constraints)):
        if number(sources["challenge"], rf"{name}: usize = ([\d_]+);", name) != expected:
            raise ValueError("challenge and AIR geometry disagree")
    require(sources["challenge"], r"TRACE_MASK_COEFFICIENTS: usize = 2 \* QUERY_COUNT \+ 8;", "witness mask rank")
    require(sources["challenge"], r"QUOTIENT_MASK_COEFFICIENTS: usize = QUERY_COUNT \+ 1;", "quotient mask rank")
    require(sources["challenge"], r"COMPOSITION_MASK_COEFFICIENTS: usize = 2 \* TRACE_ROWS;", "composition mask degree")
    require(sources["air"], r"PHYSICAL_ROW_COUNT != 65_536", "AIR trace rows")
    require(sources["air"], r"CONSTRAINT_COUNT != 923", "AIR constraints")
    require(sources["digest"], r"struct FastpqCommitmentV1\(\[u8; 32\]\)", "opaque SHA3 commitment owner")
    require(sources["digest"], r"writer\.write_all\(&self\.0\)", "raw commitment byte encoding")
    require(sources["deep"], r"use iroha_data_model::fastpq::FastpqCommitmentV1 as Digest;", "DTO commitment owner")
    return {"queries": queries, "trace_rows": trace, "lde_rows": rows, "constraints": constraints,
            "columns": width, "retained_columns": width - public, "fp4_bytes": fp4, "digest_bytes": digest}



def fixed_struct(source: str, name: str, expected: list[tuple[str, str]]) -> None:
    """Match the DTO's ordered field owners before applying a framing formula."""
    bodies = re.findall(rf"struct {name} \{{([^}}]+)\}}", source)
    if len(bodies) != 1:
        raise ValueError(f"expected one {name} DTO")
    fields = re.findall(r"pub\(super\) (\w+): ([^,]+),", bodies[0])
    if fields != expected:
        raise ValueError(f"{name} DTO field ordering or ownership changed")

def deep_frame_bound(sources: dict[str, str], *, composition_mask: bool = True) -> int:
    """Derive the current frame (or explicit no-composition-mask counterfactual)."""
    g = current_geometry(sources)
    for name, fields in {
        "DeepProof": [("row_root", "Digest"), ("quotient_root", "Digest"),
            ("fri_roots", "Vec<Digest>"), ("ood", "OodAnswers"),
            ("rows", "Vec<RowOpening>"), ("quotients", "Vec<QuotientMaskOpening>"),
            ("row_siblings", "Vec<Digest>"), ("quotient_siblings", "Vec<Digest>"),
            ("rounds", "Vec<FriRound>"), ("terminal", "Vec<Fp4>")],
        "OodAnswers": [("current", "Vec<Fp4>"), ("next", "Vec<Fp4>"), ("quotient", "Vec<Fp4>")],
        "RowOpening": [("index", "u32"), ("values", "RowValues")],
        "QuotientMaskOpening": [("index", "u32"), ("low", "Fp4"), ("high", "Fp4"), ("composition_mask", "Fp4")],
        "FriRound": [("groups", "Vec<FriGroup>"), ("siblings", "Vec<Digest>")],
        "FriGroup": [("index", "u32"), ("values", "FriValues")],
    }.items():
        fixed_struct(sources["deep"], name, fields)
    queries, rows = g["queries"], g["lde_rows"]
    retained, fp4_bytes, digest_bytes = g["retained_columns"], g["fp4_bytes"], g["digest_bytes"]
    geometry = sources["deep_geometry"]
    require(geometry, r"FRI_ARITIES: \[usize; 5\] = \[16, 16, 8, 8, 4\];", "fixed DEEP FRI schedule")
    require(geometry, r"FRI_LENGTHS: \[usize; 6\] = \[8_388_608, 524_288, 32_768, 4_096, 512, 128\];", "fixed DEEP FRI domains")
    require(geometry, r"FRI_DEGREES: \[usize; 6\] = \[131_072, 8_192, 512, 64, 8, 2\];", "fixed DEEP FRI degrees")
    require(sources["public_columns"], r"COMMITTED_COLUMN_COUNT: usize = COLUMN_COUNT - PUBLIC_COLUMN_COUNT", "retained width")
    arrays = []
    for name, length in (("FRI_ARITIES", 5), ("FRI_LENGTHS", 6)):
        matches = re.findall(rf"{name}: \[usize; {length}\] = \[([\d_, ]+)\];", geometry)
        if len(matches) != 1:
            raise ValueError(f"unrecognized DEEP {name}")
        values = [int(value.strip().replace("_", "")) for value in matches[0].split(",")]
        if len(values) != length:
            raise ValueError(f"wrong DEEP {name} length")
        arrays.append(values)
    arities, lengths = arrays
    if lengths[0] != rows or any(lengths[i] != arity * lengths[i + 1] for i, arity in enumerate(arities)):
        raise ValueError("DEEP FRI dimensions disagree")
    require(sources["deep"], r"values: FriValues,", "fixed FRI wire owner")
    require(sources["deep"], r"values: RowValues,", "fixed retained row wire owner")
    require(sources["deep_row"], r"struct RowValues\(\[u64; COMMITTED_COLUMN_COUNT\]\)", "retained row")
    require(sources["deep_row"], r"BYTES: usize = COMMITTED_COLUMN_COUNT \* size_of::<u64>\(\)", "raw retained row bytes")
    require(sources["deep_row"], r"writer\.write_all\(&value\.to_le_bytes\(\)\)", "raw retained row encoding")
    require(sources["deep_fri"], r"1 \+ \(arity - 1\) \* Fp4::BYTES", "omitted-coordinate FRI fiber bytes")
    require(sources["deep_fri"], r"Four\(\[Fp4; 3\]\),\s*///[^\n]*\n\s*Eight\(\[Fp4; 7\]\),\s*///[^\n]*\n\s*Sixteen\(\[Fp4; 15\]\)", "exact transmitted FRI cells")
    require(sources["deep_fri"], r"const fn arity_byte\(&self\) -> u8 \{\s*match self \{\s*Self::Four\(_\) => 4,\s*Self::Eight\(_\) => 8,\s*Self::Sixteen\(_\) => 16,\s*\}\s*\}", "exhaustive fixed FRI arity tags")
    require(sources["deep_fri"], r"const fn arity\(&self\) -> usize \{\s*match self \{\s*Self::Four\(_\) => 4,\s*Self::Eight\(_\) => 8,\s*Self::Sixteen\(_\) => 16,\s*\}\s*\}", "fixed full FRI arities")
    require(sources["deep_fri"], r"writer\.write_all\(&\[self\.arity_byte\(\)\]\)", "FRI arity encoding")
    require(sources["deep_fri"], r"writer\.write_all\(&value\.to_le_bytes\(\)\)", "raw FRI fiber encoding")
    require(sources["deep"], r"pub\(super\) composition_mask: Fp4,", "authenticated composition mask field")
    require(sources["deep"], r"pub\(super\) low: Fp4,\s*pub\(super\) high: Fp4,", "complete quotient-pair field widths")
    require(sources["deep"], r"canonical\(&group\.values, ARITIES\[round\] - 1,", "exact omitted fiber shape")
    require(sources["deep"], r"proof\.rows\.len\(\) != QUERY_COUNT", "exact decoded row count")
    require(sources["deep"], r"exact_indices\(proof\.rows\.iter\(\).map\(\|row\| row.index\), queries\)", "exact transcript row positions")
    require(sources["deep"], r"exact_indices\(proof\.quotients\.iter\(\).map\(\|row\| row.index\), queries\)", "exact transcript quotient positions")
    ood = record(vector(retained, fp4_bytes), vector(retained, fp4_bytes), vector(2, fp4_bytes))
    rounds = 8 + sum(field(record(
        vector(queries, record(4, 1 + (arity - 1) * fp4_bytes)),
        vector(frontier(lengths[i + 1], queries), digest_bytes),
    )) for i, arity in enumerate(arities))
    return 40 + record(
        digest_bytes, digest_bytes, vector(6, digest_bytes), ood,
        vector(queries, record(4, retained * 8)),
        vector(queries, record(4, *([fp4_bytes] * (3 if composition_mask else 2)))),
        vector(frontier(lengths[0], queries), digest_bytes),
        vector(frontier(lengths[0], queries), digest_bytes),
        rounds, vector(lengths[-1], fp4_bytes),
    )


def hypothetical_q375_full_group_screen() -> dict[str, int]:
    """Preserve the full-group/48-byte-hash mathematical counterexample only.

    These explicit hypothetical constants select no runtime module, codec or
    profile. This diagnostic is not a bound on the current SHA3 q77 protocol.
    """
    query_count, row_width, lde_rows, folds, arity, terminal = 375, 342, 524_288, 17, 2, 4
    digest_bytes, fp4_bytes = 48, 32
    minimum_rows = query_count * row_width * 8
    minimum_scalars = query_count * 2 * fp4_bytes
    raw_floor = minimum_rows + minimum_scalars
    rows = min(2 * query_count, lde_rows)
    round_bytes, fri_values, fri_frontier, length = 8, 0, 0, lde_rows
    for _ in range(folds):
        leaves = length // arity
        groups = min(query_count, leaves)
        siblings = frontier(leaves, groups)
        group = record(4, record(*([fp4_bytes] * arity)))
        round_bytes += field(record(vector(groups, group), vector(siblings, digest_bytes)))
        fri_values += groups * arity * fp4_bytes
        fri_frontier += siblings * digest_bytes
        length = leaves
    scalar_frontier = frontier(lde_rows, query_count)
    framed = 40 + record(digest_bytes, digest_bytes, digest_bytes,
        vector(folds + 1, digest_bytes), vector(rows, record(4, row_width * 8)),
        vector(query_count, record(4, fp4_bytes, fp4_bytes)),
        vector(frontier(lde_rows, rows), digest_bytes),
        vector(scalar_frontier, digest_bytes), vector(scalar_frontier, digest_bytes),
        round_bytes, vector(terminal, fp4_bytes))
    return {"trace_rows": 65_536, "columns": row_width, "constraints": 923,
        "lde_rows": lde_rows, "queries": query_count, "folds": folds,
        "raw_floor_over_segment_cap": raw_floor - 524_288,
        "raw_floor_over_axt_cap": raw_floor - 1_048_576,
        "mandatory_row_bytes": minimum_rows, "mandatory_mixed_quotient_bytes": minimum_scalars,
        "mandatory_raw_floor": raw_floor, "framed_maximum": framed,
        "framed_maximum_over_segment_cap": framed - 524_288,
        "fri_value_bytes_at_independent_maxima": fri_values,
        "fri_frontier_bytes_at_independent_maxima": fri_frontier}


def budget(sources: dict[str, str]) -> dict[str, object]:
    """Validate current source guards and exact bytes without claiming proof evidence."""
    geometry = current_geometry(sources)
    segment_cap = number(sources["deep"], r"PROOF_BYTE_TARGET: usize = ([\d_]+) \* 1024;", "segment KiB cap") * 1024
    axt_cap = number(sources["axt"], r"DEFAULT_MAX_AXT_FASTPQ_PAYLOAD_BYTES: usize = ([\d_]+) \* 1024;", "AXT KiB cap") * 1024
    deep_max = number(sources["deep"], r"MAX_FRAME_BYTES: usize = ([\d_]+);", "DEEP DTO bound")
    require(sources["backend"], r"^#\[path = \"backend/deep_engine.rs\"\]\s*mod deep_engine;", "offline DEEP verifier")
    if re.search(r"#\[cfg\(test\)\]\s*#\[path = \"backend/deep_engine.rs\"\]", sources["backend"]):
        raise ValueError("offline DEEP verifier cannot be test-only")
    require(sources["backend"], r"#\[cfg\(test\)\]\s*#\[path = \"backend/compact_quantity_diagnostic.rs\"\]\s*mod compact_quantity_diagnostic;", "retained test-only diagnostics")
    require(sources["deep"], r"caller_max_bytes\.min\(MAX_FRAME_BYTES\)", "DEEP decoder cap")
    require(sources["engine"], r'"max_proof_bytes",\s*proof_bytes,\s*limits.max_proof_bytes.min\(deep_proof::MAX_FRAME_BYTES\)', "typed proof byte admission")
    require(sources["producer"], r"QUANTITY_SHARED_FRAME_BOUND as SHARED_FRAME_BOUND", "shared producer frame bound")
    require(sources["producer"], r"quantity_artifact_resources\(count, 0\)\?\.check_proving_limits\(proving, verification\)\?", "producer resource preflight")
    require(sources["resources"], r"QUANTITY_QUERY_COUNT: usize = deep_geometry::QUERY_COUNT;", "canonical resource query owner")
    require(sources["resources"], r"QUANTITY_SHARED_FRAME_BOUND: usize = deep_proof::MAX_FRAME_BYTES;", "canonical resource frame owner")
    require(sources["resources"], r"maximum_segment_frame_bytes: QUANTITY_SHARED_FRAME_BOUND", "planned child frame bound")
    require(sources["resources"], r'"max_proof_bytes",\s*self\.maximum_segment_frame_bytes,\s*child\.max_proof_bytes', "producer byte preflight")
    require(sources["artifact"], r"fn profile_id_for<V: CompactTransferValue>\(\).*?\{\s*//[^\n]*\n\s*//[^\n]*\n\s*#\[cfg\(test\)\]\s*if !V::QUANTITY_CONTEXT \{\s*return [^\n]+\n\s*\}\s*quantity_diagnostic_profile_id\(\)\s*\}", "single offline quantity profile")
    if (segment_cap, axt_cap) != (524_288, 1_048_576):
        raise ValueError("fixed first-release proof ceilings changed")
    calculated = deep_frame_bound(sources)
    fixture_bytes = number(sources["deep_tests"], r"assert_eq!\(bytes\.len\(\), ([\d_]+)\);", "canonical DEEP fixture bytes")
    require(sources["deep_tests"], r"assert_eq!\(bytes\.len\(\), MAX_FRAME_BYTES\)", "DEEP fixture bound")
    require(sources["deep_tests"], r"assert_eq!\(bytes\.len\(\), maximum_frame_bytes\(\)\)", "DEEP independent Rust bound")
    if calculated != deep_max or fixture_bytes != deep_max:
        raise ValueError("DEEP source-derived frame, canonical fixture, and decoder bound disagree")
    labels = re.findall(r'read_retained_quantity_wire\(\s*"([^\"]+)"\s*\)', sources["retained"])
    if sorted(labels) != ["axt-single", "ordinary-single"]:
        raise ValueError("retained diagnostic inventory changed")
    require(sources["retained"], r'quantity-q77-\{label\}-\{\}\.bin', "current native proof filename")
    require(sources["retained"], r"assert_eq!\(hex::encode\(Sha256::digest\(&bytes\)\), pin.sha256\)", "retained fixture hash equality")
    hypothetical = hypothetical_q375_full_group_screen()
    if not (hypothetical["mandatory_raw_floor"] > axt_cap > segment_cap > deep_max):
        raise ValueError("hypothetical/current budget classification changed; review the byte model")
    return {"current_geometry": geometry, "hypothetical_full_group_q375": hypothetical,
        "limits": {"segment": segment_cap, "axt_inner": axt_cap},
        "retained_test_labels": sorted(labels), "fixture_evidence": "not checked by this source screen",
        "offline_deep": {"max_frame_bytes": deep_max, "headroom": segment_cap - deep_max},
        "production_qualified": False}


def main() -> None:
    """Print a deterministic audit record for the current source checkout."""
    print(json.dumps(budget(load_sources()), indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
