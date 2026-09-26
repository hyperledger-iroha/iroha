#!/usr/bin/env python3
"""Screen current zk-X509 proof bytes against the fixed V1 cap.

This reads the pinned Rust geometry instead of treating a Python copy of the
profile as release evidence. It checks arithmetic and rejects a narrow class
of partial redesigns; it does not prove a replacement AIR or its soundness.
"""

from __future__ import annotations

import ast
import json
import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
PROFILE = ROOT / "crates/iroha_core/src/privacy_engines/zk_x509/profile.rs"
STARK = ROOT / "crates/iroha_core/src/privacy_engines/zk_x509/stark.rs"
CREDENTIAL = ROOT / "crates/iroha_core/src/privacy_engines/zk_x509/credential_stark.rs"
ACCUMULATOR = ROOT / "crates/iroha_core/src/privacy_engines/zk_x509/accumulator_stark.rs"
NATIVE_TEST = ROOT / "crates/iroha_core/src/privacy_engines/zk_x509/stark/der_and_native_proof_tests.rs"


class GeometryError(ValueError):
    """The audited source geometry is missing or internally inconsistent."""


def _arithmetic(expression: str) -> int:
    """Evaluate only integer literals and the arithmetic used by Rust size pins."""
    try:
        node = ast.parse(expression, mode="eval").body
    except SyntaxError as error:
        raise GeometryError(f"invalid integer expression: {expression}") from error

    def value(part: ast.AST) -> int:
        if isinstance(part, ast.Constant) and type(part.value) is int:
            return part.value
        if isinstance(part, ast.BinOp):
            left, right = value(part.left), value(part.right)
            if isinstance(part.op, ast.Add):
                return left + right
            if isinstance(part.op, ast.Sub):
                return left - right
            if isinstance(part.op, ast.Mult):
                return left * right
        raise GeometryError(f"unsupported integer expression: {expression}")

    result = value(node)
    if result < 0:
        raise GeometryError(f"negative size: {expression}")
    return result


def _constant(source: str, name: str) -> int:
    match = re.search(
        rf"(?m)^(?:pub\(crate\)\s+)?const\s+{re.escape(name)}\s*:\s*"
        r"(?:u8|u16|u32|u64|usize)\s*=\s*([^;]+);",
        source,
    )
    if match is None:
        raise GeometryError(f"missing numeric Rust constant {name}")
    return _arithmetic(match.group(1).replace("_", ""))


def _asserted_value(source: str, name: str) -> int:
    match = re.search(
        rf"assert!\(\s*{re.escape(name)}\s*==\s*([\d_]+)\s*\);", source
    )
    if match is None:
        raise GeometryError(f"missing source assertion for {name}")
    return int(match.group(1).replace("_", ""))


def _maximum_frontier(leaf_count: int, maximum_opened: int) -> int:
    """Exact maximal binary frontier over at most this many opened leaves."""
    if leaf_count <= 0 or leaf_count & (leaf_count - 1) or not 0 < maximum_opened <= leaf_count:
        raise GeometryError("invalid Merkle geometry")
    if leaf_count == 1:
        return 0
    # Above half the leaves, the frontier only shrinks. Below that point,
    # maximally dispersed leaves fill each ancestor level before sharing paths.
    opened = min(maximum_opened, leaf_count // 2)
    height = leaf_count.bit_length() - 1
    occupied_height = (opened - 1).bit_length()
    return opened * (height - occupied_height) + (1 << occupied_height) - opened


def screen(
    profile: str, stark: str, credential: str, accumulator: str, native_test: str
) -> dict[str, int | bool]:
    """Return byte-only consequences of the currently pinned X5S1 layout."""
    q = _constant(profile, "ZK_X509_FRI_QUERY_COUNT_V1")
    shared_q = _constant(profile, "ZK_X509_SHARED_STARK_QUERY_COUNT_V1")
    blowup = _constant(profile, "ZK_X509_FRI_BLOWUP_FACTOR_V1")
    shared_blowup = _constant(profile, "ZK_X509_SHARED_STARK_BLOWUP_FACTOR_V1")
    cap = _constant(profile, "ZK_X509_MAX_PROOF_BYTES_V1")
    if (q, shared_q, blowup, shared_blowup, cap) != (136, 136, 8, 8, 9 * 1024 * 1024):
        raise GeometryError("shared V1 query, blowup, or 9 MiB contract changed")

    trace_bytes = _constant(
        profile, "ZK_X509_SHARED_STARK_WIDE_MAIN_TRACE_OPENING_BYTES_V1"
    )
    per_column = q * 2 * 8
    columns, remainder = divmod(trace_bytes, per_column)
    test_width = re.search(
        r"assert!\(\s*136\s*\*\s*2\s*\*\s*([\d_]+)\s*\*\s*8\s*>",
        native_test,
    )
    if remainder or test_width is None or columns != int(test_width.group(1).replace("_", "")):
        raise GeometryError("MAIN trace opening width disagrees with the Rust budget test")

    wide_main = _constant(
        profile, "ZK_X509_SHARED_STARK_WIDE_MAIN_MAXIMUM_PROOF_BYTES_V1"
    )
    ca_inner = _constant(
        profile, "ZK_X509_SHARED_STARK_PADDED_CA_MAXIMUM_PROOF_BYTES_V1"
    )
    main_pre_deep = _constant(profile, "ZK_X509_MAIN_PRE_DEEP_MAXIMUM_BYTES_V1")
    ca_pre_deep = _constant(profile, "ZK_X509_CA_PRE_DEEP_MAXIMUM_BYTES_V1")
    deep = _constant(profile, "ZK_X509_DEEP_OPENING_BYTES_V1")
    main_claim = _constant(profile, "ZK_X509_MAIN_CLAIM_ENVELOPE_BYTES_V1")
    ca_claim = _constant(profile, "ZK_X509_CA_CLAIM_ENVELOPE_BYTES_V1")
    combined = _constant(profile, "ZK_X509_MAXIMUM_ENCODED_X5S1_BYTES_V1")
    fixed_header = _constant(credential, "FIXED_HEADER_BYTES_V1")
    subproof_header = _constant(credential, "SUBPROOF_HEADER_BYTES_V1")
    outer = fixed_header + 2 * subproof_header
    ca_section_cap = _asserted_value(
        accumulator, "ZK_X509_CA_ACCUMULATOR_MAX_PROOF_BYTES_V1"
    )
    if (
        outer != 92
        or ca_inner != ca_pre_deep + (deep - (wide_main - main_pre_deep))
        or ca_section_cap != ca_inner + ca_claim
        or combined != main_pre_deep + ca_pre_deep + deep + main_claim + ca_claim + outer
    ):
        raise GeometryError("X5S1 component sizes disagree with the canonical maximum")
    if any("AggregateFriCommitmentLayoutV1::Paired" not in source for source in (profile, stark, accumulator)):
        raise GeometryError("X509 paired FRI commitment layout is missing")

    main_log = _constant(profile, "ZK_X509_MAX_NATIVE_TRACE_LOG2_V1") + blowup.bit_length() - 1
    if main_log != _constant(profile, "ZK_X509_SHARED_STARK_MAIN_LDE_LOG2_V1"):
        raise GeometryError("MAIN shared-domain logarithm disagrees with the profile")
    ca_log = _constant(profile, "ZK_X509_CA_FRI_LDE_LOG2_V1")
    terminal_log = _constant(profile, "ZK_X509_CA_FRI_TERMINAL_LOG2_V1")
    groups = _constant(profile, "ZK_X509_TRACE_GROUPS_V1")
    ca_chunks = _constant(profile, "ZK_X509_CA_COMPOSITION_DEGREE_CHUNKS_V1")
    ca_deep = deep - (wide_main - main_pre_deep)
    ca_columns, ca_remainder = divmod(ca_deep // 32 - ca_chunks, 2)
    if ca_deep % 32 or ca_remainder:
        raise GeometryError("CA DEEP opening width is inconsistent")

    def complete_oods_candidate(current: int, width: int, log: int, group_count: int) -> int:
        domain = 1 << log
        return (current - q * width * 8
                - 2 * group_count * (_maximum_frontier(domain, 2 * q) - _maximum_frontier(domain, q)) * 48
                - 2 * (group_count - 1) * (_maximum_frontier(domain, q) + 1) * 48)

    candidate_main = complete_oods_candidate(wide_main, columns, main_log, groups)
    candidate_ca = complete_oods_candidate(ca_inner, ca_columns, ca_log, 1)
    candidate = candidate_main + candidate_ca + main_claim + ca_claim + outer
    paired_saving = sum(
        (_maximum_frontier(1 << layer_log, 2 * q)
         - _maximum_frontier(1 << (layer_log - 1), q)) * 48
        for log in (main_log, ca_log)
        for layer_log in range(log, terminal_log, -1)
    )

    log19_columns = _constant(stark, "MAIN_LOG19_BASE_WIDTH_V1") + _constant(
        stark, "MAIN_LOG19_AUX_WIDTH_V1"
    )
    p256_base = _asserted_value(stark, "P256_MAIN_LOG19_BASE_WIDTH_V1")
    p256_aux = _asserted_value(stark, "P256_MAIN_LOG19_AUX_WIDTH_V1")
    p256_columns = p256_base + p256_aux
    p256_log5_columns = _asserted_value(
        stark, "P256_MAIN_LOG5_BASE_WIDTH_V1"
    ) + _asserted_value(stark, "P256_MAIN_LOG5_AUX_WIDTH_V1")
    p256_log16_columns = _asserted_value(
        stark, "P256_MAIN_LOG16_BASE_WIDTH_V1"
    ) + _asserted_value(stark, "P256_MAIN_LOG16_AUX_WIDTH_V1")
    all_p256_columns = p256_log5_columns + p256_log16_columns + p256_columns
    signature_count = _constant(stark, "P256_SIGNATURE_COUNT_V1")
    if (p256_log5_columns, p256_log16_columns) != (800, 805):
        raise GeometryError("P-256 all-group width changed without a new proof budget")
    if (
        signature_count != 5
        or _constant(stark, "P256_MAIN_LOG19_BASE_START_V1") + p256_base
        != _constant(stark, "MAIN_LOG19_BASE_WIDTH_V1")
        or _constant(stark, "P256_MAIN_LOG19_AUX_START_V1") + p256_aux
        != _constant(stark, "MAIN_LOG19_AUX_WIDTH_V1")
    ):
        raise GeometryError("five P-256 signature columns do not own the log-19 tail")
    if p256_columns > log19_columns or log19_columns > columns or all_p256_columns > columns:
        raise GeometryError("P-256 width is not within the MAIN opening inventory")

    other_bytes = combined - trace_bytes
    opening_headroom = cap - other_bytes
    if opening_headroom < 0:
        raise GeometryError("non-trace proof components alone exceed the cap")
    remaining_after_log19_p256 = combined - p256_columns * per_column
    remaining_after_all_p256 = combined - all_p256_columns * per_column
    remaining_after_log19 = combined - log19_columns * per_column
    remaining_non_log19_columns = columns - log19_columns
    main_section_cap = cap - outer - ca_section_cap
    if (
        main_section_cap <= 0
        or remaining_after_log19_p256 <= cap
        or remaining_after_all_p256 <= cap
        or remaining_after_log19 <= cap
    ):
        raise GeometryError("audited partial-redesign rejection no longer applies")

    return {
        "proof_cap_bytes": cap,
        "combined_current_max_bytes": combined,
        "combined_excess_bytes": combined - cap,
        "implemented_paired_fri_saving_bytes": paired_saving,
        "candidate_main_inner_bytes": candidate_main,
        "candidate_ca_inner_bytes": candidate_ca,
        "candidate_complete_relation_bytes": candidate,
        "candidate_headroom_bytes": cap - candidate,
        "candidate_requires_complete_fp4_air_and_shared_trace_commitments": True,
        "current_main_inner_max_bytes": wide_main,
        "main_section_cap_bytes": main_section_cap,
        "current_trace_columns": columns,
        "current_trace_opening_bytes": trace_bytes,
        "non_trace_current_max_bytes": other_bytes,
        "max_columns_if_non_trace_fixed": opening_headroom // per_column,
        "minimum_columns_to_remove_if_non_trace_fixed": columns
        - opening_headroom // per_column,
        "log19_opening_bytes": log19_columns * per_column,
        "remaining_non_log19_trace_columns": remaining_non_log19_columns,
        "p256_signature_count": signature_count,
        "p256_log19_opening_bytes": p256_columns * per_column,
        "combined_after_hypothetically_removing_log19_p256_openings_bytes": remaining_after_log19_p256,
        "remaining_excess_after_log19_p256_removal_bytes": remaining_after_log19_p256 - cap,
        "p256_all_group_trace_columns": all_p256_columns,
        "p256_all_group_opening_bytes": all_p256_columns * per_column,
        "remaining_non_p256_trace_columns": columns - all_p256_columns,
        "combined_after_hypothetically_removing_all_p256_trace_openings_bytes": remaining_after_all_p256,
        "remaining_excess_after_all_p256_trace_opening_removal_bytes": remaining_after_all_p256 - cap,
        "minimum_additional_non_p256_columns_to_replace_if_other_bytes_fixed":
            (remaining_after_all_p256 - cap + per_column - 1) // per_column,
        "combined_after_hypothetically_removing_all_log19_openings_bytes": remaining_after_log19,
        "remaining_excess_after_log19_removal_bytes": remaining_after_log19 - cap,
        "minimum_additional_non_log19_columns_to_replace_if_other_bytes_fixed":
            remaining_non_log19_columns - opening_headroom // per_column,
        "production_qualified": False,
    }


def main() -> None:
    result = screen(
        PROFILE.read_text(),
        STARK.read_text(),
        CREDENTIAL.read_text(),
        ACCUMULATOR.read_text(),
        NATIVE_TEST.read_text(),
    )
    print(json.dumps(result, sort_keys=True, indent=2))


if __name__ == "__main__":
    main()
