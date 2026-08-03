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
    assert args.deploy_queue_capacity == 4_096
    assert args.kura_blocks_in_memory == 16
    assert not args.no_status_snapshots
    assert not args.capture_diagnostics
    assert not args.allow_existing_irohad
    assert args.diagnostic_timeout_seconds == 30.0


def test_peer_count_must_have_revision4_geometry():
    for peers in (1, 5, 30, 32):
        try:
            MODULE.parse_args(["--peers", str(peers)])
        except SystemExit as err:
            assert err.code == 2
        else:
            raise AssertionError(f"expected invalid --peers={peers} to be rejected")

    for peers in (4, 7, 10, 31):
        assert MODULE.parse_args(["--peers", str(peers)]).peers == peers


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


def test_deploy_command_forwards_kura_hot_cache_limit(tmp_path):
    args = MODULE.parse_args(
        [
            "--iroha-dir",
            str(tmp_path),
            "--out-dir",
            str(tmp_path / "run"),
            "--deploy-queue-capacity",
            "123",
            "--kura-blocks-in-memory",
            "9",
        ]
    )
    cmd = MODULE.build_deploy_cmd(args)
    assert cmd[cmd.index("--queue-capacity") + 1] == "123"
    assert cmd[cmd.index("--kura-blocks-in-memory") + 1] == "9"


def test_deploy_queue_capacity_must_be_positive():
    try:
        MODULE.parse_args(["--deploy-queue-capacity", "0"])
    except SystemExit as err:
        assert err.code == 2
    else:
        raise AssertionError("expected --deploy-queue-capacity=0 to be rejected")


def test_kura_hot_cache_limit_must_be_positive():
    try:
        MODULE.parse_args(["--kura-blocks-in-memory", "0"])
    except SystemExit as err:
        assert err.code == 2
    else:
        raise AssertionError("expected --kura-blocks-in-memory=0 to be rejected")


def test_diagnostic_timeout_must_be_positive():
    try:
        MODULE.parse_args(["--diagnostic-timeout-seconds", "0"])
    except SystemExit as err:
        assert err.code == 2
    else:
        raise AssertionError("expected --diagnostic-timeout-seconds=0 to be rejected")


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
        diagnostic_artifacts=[
            MODULE.DiagnosticArtifact(
                timestamp=4.0,
                phase="final",
                run_index=1,
                peer_index=0,
                pid=123,
                kind="vmmap-summary",
                path=str(tmp_path / "vmmap.txt"),
                ok=True,
            )
        ],
        existing_irohad_processes=[
            MODULE.ExistingIrohadProcess(
                pid=456,
                rss_bytes=2048,
                command="/tmp/irohad --config /tmp/other/peer0.toml",
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
    assert payload["preflight_error"] is None
    assert payload["existing_irohad_total_rss_bytes"] == 2048
    assert payload["existing_irohad_processes"][0]["pid"] == 456
    assert payload["peak_total_rss_bytes"] == 10
    assert payload["last_total_rss_bytes"] == 8
    assert payload["samples"][0]["phase"] == "load"
    assert payload["samples"][0]["run_index"] == 1
    assert payload["samples"][1]["phase"] == "post_load"
    assert payload["status_snapshots"][0]["fields"]["rbc_store_bytes"] == 12
    assert payload["diagnostic_artifacts"][0]["kind"] == "vmmap-summary"
    assert payload["diagnostic_artifacts"][0]["ok"] is True


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


def test_diagnostic_artifact_path_sanitizes_phase(tmp_path):
    path = MODULE.diagnostic_artifact_path(
        tmp_path,
        phase="post load/final",
        run_index=2,
        peer_index=3,
        pid=456,
        kind="heap-summary",
    )
    assert path == tmp_path / "diagnostics" / "run2_post_load_final_peer3_pid456_heap-summary.txt"


def test_capture_peer_diagnostics_writes_tool_outputs(tmp_path, monkeypatch):
    calls = []

    def fake_which(tool):
        return f"/usr/bin/{tool}"

    def fake_run(cmd, **kwargs):
        calls.append(cmd)
        assert kwargs["stdout"] == MODULE.subprocess.PIPE
        assert kwargs["stderr"] == MODULE.subprocess.STDOUT
        assert kwargs["timeout"] == 5.0

        class Completed:
            returncode = 0
            stdout = f"captured {' '.join(cmd)}"

        return Completed()

    monkeypatch.setattr(MODULE.shutil, "which", fake_which)
    monkeypatch.setattr(MODULE.subprocess, "run", fake_run)

    artifacts = MODULE.capture_peer_diagnostics(
        tmp_path,
        [
            MODULE.PeerProcess(
                pid=123,
                config_path=tmp_path / "peer0.toml",
                command=f"irohad --config {tmp_path / 'peer0.toml'}",
            )
        ],
        phase="final",
        run_index=1,
        timeout_seconds=5.0,
    )

    assert calls == [["vmmap", "-summary", "123"], ["heap", "-s", "123"]]
    assert [artifact.kind for artifact in artifacts] == ["vmmap-summary", "heap-summary"]
    assert all(artifact.ok for artifact in artifacts)
    for artifact in artifacts:
        assert artifact.path is not None
        assert Path(artifact.path).read_text(encoding="utf-8").startswith("captured ")


def test_capture_peer_diagnostics_records_missing_tools(tmp_path, monkeypatch):
    monkeypatch.setattr(MODULE.shutil, "which", lambda _tool: None)

    artifacts = MODULE.capture_peer_diagnostics(
        tmp_path,
        [MODULE.PeerProcess(pid=123, config_path=tmp_path / "peer0.toml", command="irohad")],
        phase="final",
        run_index=1,
        timeout_seconds=5.0,
    )

    assert len(artifacts) == 2
    assert all(not artifact.ok for artifact in artifacts)
    assert all(artifact.path is None for artifact in artifacts)
    assert {artifact.error for artifact in artifacts} == {"vmmap not found", "heap not found"}


def test_command_ownership_requires_matching_config_path(tmp_path):
    config = tmp_path / "peer0.toml"
    assert MODULE.command_owns_peer(f"/bin/irohad --config {config}", config)
    assert MODULE.command_owns_peer(f"/bin/irohad --config={config}", config)
    spaced_config = tmp_path / "run with spaces" / "peer0.toml"
    assert MODULE.command_owns_peer(f"/bin/irohad --config '{spaced_config}'", spaced_config)
    assert not MODULE.command_owns_peer("/bin/irohad --config /tmp/other/peer0.toml", config)
    assert not MODULE.command_owns_peer(f"/bin/irohad --config {config}.bak", config)
    assert not MODULE.command_owns_peer(
        f"/bin/irohad --config {tmp_path / 'peer0.toml.old' / 'peer0.toml'}",
        config,
    )
    assert not MODULE.command_owns_peer("", config)


def test_irohad_process_discovery_reads_ps_rows(monkeypatch):
    def fake_ps_output(args):
        assert args == ["-axo", "pid=,rss=,command="]
        return (
            "101 2048 /tmp/irohad --config /tmp/a/peer0.toml\n"
            "102 512 /tmp/not-irohad --config /tmp/a/peer1.toml\n"
            "bad 1 /tmp/irohad --config /tmp/a/peer2.toml\n"
        )

    monkeypatch.setattr(MODULE, "ps_output", fake_ps_output)

    processes = MODULE.irohad_processes()

    assert processes == [
        MODULE.ExistingIrohadProcess(
            pid=101,
            rss_bytes=2048 * 1024,
            command="/tmp/irohad --config /tmp/a/peer0.toml",
        )
    ]


def test_unrelated_irohad_processes_excludes_owned_out_dir_peers(tmp_path, monkeypatch):
    out_dir = tmp_path / "run"
    out_dir.mkdir()
    config = out_dir / "peer0.toml"
    (out_dir / "peer0.pid").write_text("101\n", encoding="utf-8")

    def fake_ps_output(args):
        if args == ["-o", "command=", "-p", "101"]:
            return f"/tmp/irohad --config {config}"
        if args == ["-axo", "pid=,rss=,command="]:
            return (
                f"101 2048 /tmp/irohad --config {config}\n"
                "202 4096 /tmp/irohad --config /tmp/other/peer0.toml\n"
            )
        raise AssertionError(f"unexpected ps args: {args}")

    monkeypatch.setattr(MODULE, "ps_output", fake_ps_output)

    processes = MODULE.unrelated_irohad_processes(out_dir)

    assert processes == [
        MODULE.ExistingIrohadProcess(
            pid=202,
            rss_bytes=4096 * 1024,
            command="/tmp/irohad --config /tmp/other/peer0.toml",
        )
    ]


def test_unrelated_irohad_processes_treats_config_prefix_collision_as_unowned(
    tmp_path, monkeypatch
):
    out_dir = tmp_path / "run"
    out_dir.mkdir()
    config = out_dir / "peer0.toml"
    (out_dir / "peer0.pid").write_text("101\n", encoding="utf-8")

    def fake_ps_output(args):
        if args == ["-o", "command=", "-p", "101"]:
            return f"/tmp/irohad --config {config}.bak"
        if args == ["-axo", "pid=,rss=,command="]:
            return f"101 2048 /tmp/irohad --config {config}.bak\n"
        raise AssertionError(f"unexpected ps args: {args}")

    monkeypatch.setattr(MODULE, "ps_output", fake_ps_output)

    processes = MODULE.unrelated_irohad_processes(out_dir)

    assert processes == [
        MODULE.ExistingIrohadProcess(
            pid=101,
            rss_bytes=2048 * 1024,
            command=f"/tmp/irohad --config {config}.bak",
        )
    ]


def test_existing_irohad_preflight_writes_report_and_blocks(tmp_path, monkeypatch):
    out_dir = tmp_path / "run"
    report = tmp_path / "blocked.json"

    monkeypatch.setattr(
        MODULE,
        "unrelated_irohad_processes",
        lambda _out_dir: [
            MODULE.ExistingIrohadProcess(
                pid=303,
                rss_bytes=8192,
                command="/tmp/irohad --config /tmp/old/peer0.toml",
            )
        ],
    )
    monkeypatch.setattr(
        MODULE,
        "run_checked",
        lambda *_args, **_kwargs: (_ for _ in ()).throw(
            AssertionError("deploy should not run after preflight block")
        ),
    )

    rc = MODULE.main(
        [
            "--iroha-dir",
            str(tmp_path),
            "--out-dir",
            str(out_dir),
            "--report",
            str(report),
        ]
    )

    payload = json.loads(report.read_text(encoding="utf-8"))
    assert rc == MODULE.EXISTING_IROHAD_EXIT_CODE
    assert payload["preflight_error"].startswith("refusing to start guarded localnet")
    assert payload["existing_irohad_total_rss_bytes"] == 8192
    assert payload["existing_irohad_processes"][0]["pid"] == 303
    assert payload["samples"] == []


def test_allow_existing_irohad_allows_deploy_and_records_processes(tmp_path, monkeypatch):
    out_dir = tmp_path / "run"
    out_dir.mkdir()
    (out_dir / "client.toml").write_text(
        'torii_url = "http://127.0.0.1:48080/"\n',
        encoding="utf-8",
    )
    report = tmp_path / "allowed.json"
    deployed = []
    stopped = []

    monkeypatch.setattr(
        MODULE,
        "unrelated_irohad_processes",
        lambda _out_dir: [
            MODULE.ExistingIrohadProcess(
                pid=404,
                rss_bytes=16384,
                command="/tmp/irohad --config /tmp/old/peer0.toml",
            )
        ],
    )
    monkeypatch.setattr(MODULE, "profile_binary_exists", lambda *_args: True)
    monkeypatch.setattr(MODULE, "run_checked", lambda *_args, **_kwargs: deployed.append(True))
    monkeypatch.setattr(MODULE, "stop_localnet", lambda _out_dir: stopped.append(True))
    monkeypatch.setattr(MODULE, "peer_processes", lambda _out_dir: [])
    monkeypatch.setattr(MODULE, "sample_statuses", lambda *_args: [])

    class FakePopen:
        returncode = 0

        def __init__(self, *_args, **_kwargs):
            pass

        def poll(self):
            return self.returncode

        def terminate(self):
            raise AssertionError("completed fake process should not be terminated")

        def wait(self, **_kwargs):
            return self.returncode

        def kill(self):
            raise AssertionError("completed fake process should not be killed")

    monkeypatch.setattr(MODULE.subprocess, "Popen", FakePopen)

    rc = MODULE.main(
        [
            "--iroha-dir",
            str(tmp_path),
            "--out-dir",
            str(out_dir),
            "--report",
            str(report),
            "--allow-existing-irohad",
            "--count",
            "1",
            "--post-load-sample-seconds",
            "0",
            "--no-status-snapshots",
        ]
    )

    payload = json.loads(report.read_text(encoding="utf-8"))
    assert rc == 0
    assert deployed == [True]
    assert stopped == [True]
    assert payload["preflight_error"] is None
    assert payload["existing_irohad_total_rss_bytes"] == 16384
    assert payload["existing_irohad_processes"][0]["pid"] == 404


def test_guard_source_avoids_broad_process_kills():
    source = MODULE_PATH.read_text(encoding="utf-8")
    forbidden = ["pkill", "killall", "kill -0", "pgrep"]
    for marker in forbidden:
        assert marker not in source
