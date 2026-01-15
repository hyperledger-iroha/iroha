import importlib.util
from pathlib import Path


MODULE_PATH = Path(__file__).resolve().parents[1] / "tx_load.py"
SPEC = importlib.util.spec_from_file_location("tx_load", MODULE_PATH)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(MODULE)


def test_parse_metric_value_sums_labeled_samples():
    text = "\n".join(
        [
            "# HELP pipeline_dag_vertices transactions per block",
            "pipeline_dag_vertices{lane=\"1\"} 7",
            "pipeline_dag_vertices{lane=\"2\"} 9",
        ]
    )
    assert MODULE.parse_metric_value(text, "pipeline_dag_vertices") == 16.0


def test_parse_metric_value_reads_plain_metric():
    text = "pipeline_dag_vertices 42\n"
    assert MODULE.parse_metric_value(text, "pipeline_dag_vertices") == 42.0


def test_extract_status_snapshot_defaults():
    payload = {
        "blocks": "3",
        "blocks_non_empty": 2,
        "txs_approved": 11,
        "txs_rejected": 1,
        "queue_size": None,
    }
    snapshot = MODULE.extract_status_snapshot(payload)
    assert snapshot["blocks"] == 3
    assert snapshot["blocks_non_empty"] == 2
    assert snapshot["txs_approved"] == 11
    assert snapshot["txs_rejected"] == 1
    assert snapshot["queue_size"] == 0
