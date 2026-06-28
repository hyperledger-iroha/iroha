import importlib.util
import json
import sys
from pathlib import Path


MODULE_PATH = Path(__file__).resolve().parents[1] / "run_localnet_memory_guard.py"
SPEC = importlib.util.spec_from_file_location("run_localnet_memory_guard", MODULE_PATH)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
sys.modules["run_localnet_memory_guard"] = MODULE
SPEC.loader.exec_module(MODULE)


def test_defaults_are_guarded_for_localnet_repro():
    args = MODULE.parse_args([])
    assert args.memory_limit_gb == 8
    assert args.peers == 4
    assert args.count == 100_000
    assert args.parallel == 64
    assert args.batch_size == 1_000
    assert args.queue_soft_limit == 0
    assert args.queue_hard_limit == 0
    assert args.post_load_sample_seconds == 30.0
    assert args.load_runs == 1
    assert not args.no_status_snapshots


def test_peer_count_below_four_is_rejected():
    try:
        MODULE.parse_args(["--peers", "1"])
    except SystemExit as err:
        assert err.code == 2
    else:
        raise AssertionError("expected --peers below 4 to be rejected")


def test_tx_load_command_forwards_queue_limits(tmp_path):
    args = MODULE.parse_args(
        [
            "--iroha-dir",
            str(tmp_path),
            "--out-dir",
            str(tmp_path / "run"),
            "--queue-soft-limit",
            "11",
            "--queue-hard-limit",
            "22",
            "--queue-wait-timeout",
            "33",
        ]
    )
    cmd = MODULE.build_tx_load_cmd(args)
    assert cmd[cmd.index("--queue-soft-limit") + 1] == "11"
    assert cmd[cmd.index("--queue-hard-limit") + 1] == "22"
    assert cmd[cmd.index("--queue-wait-timeout") + 1] == "33.0"


def test_base_api_port_uses_generated_client_config(tmp_path):
    client_config = tmp_path / "client.toml"
    client_config.write_text(
        'chain = "00000000-0000-0000-0000-000000000000"\n'
        'torii_url = "http://127.0.0.1:48084/"\n',
        encoding="utf-8",
    )

    assert MODULE.base_api_port_from_client_config(client_config, 48080) == 48084


def test_base_api_port_falls_back_when_client_config_missing(tmp_path):
    assert MODULE.base_api_port_from_client_config(tmp_path / "missing.toml", 48080) == 48080


def test_write_report_records_limit_last_sample_and_phase(tmp_path):
    report = tmp_path / "report.json"
    samples = [
        MODULE.MemorySample(
            timestamp=1.0,
            total_rss_bytes=10,
            max_peer_rss_bytes=7,
            peers=4,
            phase="load",
            run_index=1,
        ),
        MODULE.MemorySample(
            timestamp=2.0,
            total_rss_bytes=8,
            max_peer_rss_bytes=5,
            peers=4,
            phase="post_load",
            run_index=1,
        ),
    ]
    MODULE.write_report(
        report,
        samples,
        0,
        memory_limit_bytes=100,
        post_load_sample_seconds=30.0,
        status_snapshots=[
            MODULE.StatusSnapshot(
                timestamp=3.0,
                phase="post_load",
                run_index=1,
                peer_index=0,
                status_url="http://127.0.0.1:48080/status",
                ok=True,
                fields={"queue_size": 0, "rbc_store_bytes": 12},
            )
        ],
        load_runs=2,
        tx_returncodes=[0, 0],
    )

    payload = json.loads(report.read_text(encoding="utf-8"))
    assert payload["load_runs"] == 2
    assert payload["tx_returncodes"] == [0, 0]
    assert payload["memory_limit_bytes"] == 100
    assert payload["post_load_sample_seconds"] == 30.0
    assert payload["peak_total_rss_bytes"] == 10
    assert payload["last_total_rss_bytes"] == 8
    assert payload["samples"][0]["phase"] == "load"
    assert payload["samples"][0]["run_index"] == 1
    assert payload["samples"][1]["phase"] == "post_load"
    assert payload["status_snapshots"][0]["fields"]["rbc_store_bytes"] == 12


def test_status_url_for_peer_uses_base_port():
    assert MODULE.status_url_for_peer(48080, 3) == "http://127.0.0.1:48083/status"


def test_extract_status_fields_reads_root_and_sumeragi_counters():
    fields = MODULE.extract_status_fields(
        {
            "blocks": 7,
            "blocks_non_empty": 5,
            "txs_approved": 11,
            "txs_rejected": 13,
            "queue_size": 17,
            "sumeragi": {
                "tx_queue_retained_bytes": 19,
                "tx_queue_max_retained_bytes": 23,
                "tx_queue_saturated": True,
                "tx_queue_saturated_by_count": False,
                "tx_queue_saturated_by_bytes": True,
                "rbc_store_sessions": 29,
                "rbc_store_bytes": 31,
                "rbc_store_pressure_level": 1,
                "rbc_store_evictions_total": 37,
                "pending_rbc": {
                    "sessions": 2,
                    "chunks": 3,
                    "bytes": 41,
                },
                "tx_gossip": {
                    "queued": 43,
                    "evicted_total": 47,
                },
            },
        }
    )
    assert fields["blocks"] == 7
    assert fields["queue_retained_bytes"] == 19
    assert fields["queue_saturated"] is True
    assert fields["queue_saturated_by_bytes"] is True
    assert fields["rbc_store_bytes"] == 31
    assert fields["pending_rbc_bytes"] == 41
    assert fields["tx_gossip_queued"] == 43


def test_tx_load_log_path_keeps_legacy_name_for_single_run(tmp_path):
    assert MODULE.tx_load_log_path(tmp_path, 1, 1) == tmp_path / "tx_load.log"


def test_tx_load_log_path_disambiguates_multiple_runs(tmp_path):
    assert MODULE.tx_load_log_path(tmp_path, 2, 3) == tmp_path / "tx_load_run_2.log"


def test_command_ownership_requires_matching_config_path(tmp_path):
    config = tmp_path / "peer0.toml"
    assert MODULE.command_owns_peer(f"/bin/irohad --config {config}", config)
    assert not MODULE.command_owns_peer("/bin/irohad --config /tmp/other/peer0.toml", config)
    assert not MODULE.command_owns_peer("", config)


def test_guard_source_avoids_broad_process_kills():
    source = MODULE_PATH.read_text(encoding="utf-8")
    forbidden = ["pkill", "killall", "kill -0", "pgrep"]
    for marker in forbidden:
        assert marker not in source
