from pathlib import Path


SCRIPT_PATH = Path(__file__).resolve().parents[1] / "run_10k_localnet.sh"


def test_run_10k_forwards_queue_guard_defaults():
    source = SCRIPT_PATH.read_text(encoding="utf-8")
    assert "QUEUE_SOFT_LIMIT=50000" in source
    assert "QUEUE_HARD_LIMIT=120000" in source
    assert "QUEUE_WAIT_TIMEOUT=60" in source
    assert '--queue-soft-limit "$QUEUE_SOFT_LIMIT"' in source
    assert '--queue-hard-limit "$QUEUE_HARD_LIMIT"' in source
    assert '--queue-wait-timeout "$QUEUE_WAIT_TIMEOUT"' in source


def test_run_10k_usage_documents_queue_guard_options():
    source = SCRIPT_PATH.read_text(encoding="utf-8")
    assert "--queue-soft-limit <N>" in source
    assert "--queue-hard-limit <N>" in source
    assert "--queue-wait-timeout <SEC>" in source
