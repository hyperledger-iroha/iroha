import importlib.util
import sys
from pathlib import Path


MODULE_PATH = Path(__file__).resolve().parents[1] / "tx_load.py"
SPEC = importlib.util.spec_from_file_location("tx_load", MODULE_PATH)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
sys.modules["tx_load"] = MODULE
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


def test_normalize_torii_url_adds_trailing_slash():
    assert MODULE.normalize_torii_url("http://localhost:8080") == "http://localhost:8080/"
    assert MODULE.normalize_torii_url("http://localhost:8080/") == "http://localhost:8080/"


def test_split_even_distributes_remainder():
    assert MODULE.split_even(10, 3) == [4, 3, 3]
    assert MODULE.split_even(2, 4) == [1, 1, 0, 0]


def test_render_client_config_rewrites_torii_url():
    base = "\n".join(
        [
            'chain = "deadbeef"',
            'torii_url = "http://127.0.0.1:8080/"',
        ]
    )
    rendered = MODULE.render_client_config(base, "http://127.0.0.1:9090/")
    assert 'torii_url = "http://127.0.0.1:9090/"' in rendered


def test_render_client_config_inserts_when_missing():
    base = 'chain = "deadbeef"'
    rendered = MODULE.render_client_config(base, "http://127.0.0.1:9090/")
    assert 'torii_url = "http://127.0.0.1:9090/"' in rendered


def test_count_rate_limit_hits_counts_status():
    output = "status: 429\nToo Many Requests\nstatus: 429\n"
    assert MODULE.count_rate_limit_hits(output) == 3


def test_torii_url_from_status_uses_host():
    assert (
        MODULE.torii_url_from_status("http://127.0.0.1:8080/status")
        == "http://127.0.0.1:8080/"
    )
