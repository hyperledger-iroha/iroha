from pathlib import Path
import re


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "run_izanami_communication_vulnerability_matrix.sh"


def _classifier_degraded_pattern() -> str:
    source = SCRIPT.read_text()
    match = re.search(r"^acceptance_failure_regex='([^']+)'", source, re.MULTILINE)
    assert match is not None
    return match.group(1)


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
