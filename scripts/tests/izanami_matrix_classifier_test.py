from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "run_izanami_communication_vulnerability_matrix.sh"


def _classifier_degraded_pattern() -> str:
    source = SCRIPT.read_text()
    marker = "Fault scenarios deliberately make Torii endpoints unreachable"
    start = source.index(marker)
    rg_call = source.index("if rg -q '", start)
    pattern_start = rg_call + len("if rg -q '")
    pattern_end = source.index("'", pattern_start)
    return source[pattern_start:pattern_end]


def test_matrix_classifier_ignores_retryable_endpoint_refusals() -> None:
    pattern = _classifier_degraded_pattern()

    assert "Connection refused" not in pattern
    assert "connection closed before message completed" not in pattern


def test_matrix_classifier_keeps_final_liveness_failure_markers() -> None:
    pattern = _classifier_degraded_pattern()

    for marker in (
        "429 Too Many Requests",
        "confirmation timeout",
        "sampled confirmation failed",
        "transaction did not reach",
        "transaction remained queued",
        "route_unavailable",
    ):
        assert marker in pattern
