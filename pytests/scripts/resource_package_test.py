"""Resource package ownership and CLI-shaped entrypoint integration controls."""
import importlib
import json
from pathlib import Path
import subprocess
import sys

import pytest

ROOT = Path(__file__).resolve().parents[2]
MODULES = ROOT / "scripts/nexus"
sys.path.insert(0, str(MODULES))


def test_composed_modules_resolve_one_canonical_owner_and_matching_contracts():
    names = ("kura_resource_metrics", "resource_process", "resource_probe",
             "resource_probe_worker", "resource_replay")
    parser, process, probe, worker, replay = (
        importlib.import_module(name) for name in names
    )
    for name, module in zip(names, (parser, process, probe, worker, replay), strict=True):
        assert Path(module.__file__).resolve() == MODULES / f"{name}.py"
    assert probe.PinnedProcess is worker.PinnedProcess is process.PinnedProcess
    assert worker.Probe is probe.Probe
    assert worker.DarwinProcessReader is process.DarwinProcessReader
    assert replay.ProcessIdentity is process.ProcessIdentity
    assert probe.parse_kura_resource_metrics is replay.parse_kura_resource_metrics is parser.parse_kura_resource_metrics
    assert worker.CAPTURE_SCHEMA == replay.CAPTURE_SCHEMA
    assert worker.MAX_MANIFEST_BYTES == replay.MAX_MANIFEST_BYTES
    assert parser.MAX_RESPONSE_BYTES == replay.MAX_METRICS_BYTES
    assert probe.MAX_STATUS_BYTES == replay.MAX_STATUS_BYTES
    assert probe.MAX_PROBE_WIRE_BYTES == replay.MAX_WIRE_BYTES
    assert parser.MAX_EXACT_INTEGER == process.MAX_EXACT_INTEGER == probe.MAX_EXACT_INTEGER == replay.MAX_EXACT


@pytest.mark.parametrize("config_kind", ["missing", "malformed", "unsafe_mode"])
def test_actual_worker_launch_with_empty_environment_and_root_cwd(tmp_path, config_kind):
    # This is the CLI's exact interpreter/script argument shape, with no -I or
    # PYTHONPATH. All configurations fail read_config before native/HTTP owners.
    worker_path = MODULES / "resource_probe_worker.py"
    config = tmp_path / "NeverExportRuntimeConfig.json"
    captures = tmp_path / "captures"
    captures.mkdir(mode=0o700)
    if config_kind != "missing":
        config.write_bytes(b"NeverExportSecret" if config_kind == "malformed" else b"{}")
        config.chmod(0o600 if config_kind == "malformed" else 0o644)
    request_schema = "iroha.sumeragi_v2.resource_probe.request.v1"
    response_schema = "iroha.sumeragi_v2.resource_probe.response.v1"
    requests = [{"schema": request_schema, "kind": kind, "sequence": sequence, "timeout_ms": 1000}
                for sequence, kind in enumerate(("preflight", "finish"))]
    raw = b"".join(json.dumps(row, separators=(",", ":")).encode() + b"\n" for row in requests)
    command = [sys.executable, str(worker_path), "--config", str(config), "--capture-dir", str(captures)]
    assert all(Path(command[index]).is_absolute() for index in (0, 1, 3, 5))
    result = subprocess.run(command, input=raw, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                            env={}, cwd="/", timeout=5, check=False)
    assert result.returncode == 0
    assert result.stderr == b""
    assert [json.loads(line) for line in result.stdout.splitlines()] == [
        {"schema": response_schema, "kind": "preflight", "sequence": 0,
         "outcome": "failed", "manifest": None},
        {"schema": response_schema, "kind": "finish", "sequence": 1,
         "outcome": "complete", "manifest": None},
    ]
    assert all(len(line) + 1 <= 16 * 1024 for line in result.stdout.splitlines())
    assert b"NeverExport" not in result.stdout + result.stderr
    assert list(captures.iterdir()) == []


def test_all_policy_consumers_resolve_the_same_pure_admission_owner():
    budget = importlib.import_module('resource_evidence_budget')
    probe = importlib.import_module('resource_probe')
    worker = importlib.import_module('resource_probe_worker')
    replay = importlib.import_module('resource_replay')
    bundle = importlib.import_module('resource_bundle')
    assert Path(budget.__file__).resolve() == MODULES / 'resource_evidence_budget.py'
    assert Path(bundle.__file__).resolve() == MODULES / 'resource_bundle.py'
    assert probe.CapturePolicy is replay.CapturePolicy is budget.CapturePolicy
    assert worker.validate_run_budget is replay.validate_run_budget is budget.validate_run_budget
    assert bundle.select_run_budget is budget.select_run_budget
    assert worker.parse_run_budget is budget.parse_run_budget
    assert bundle.MAX_TOTAL_BYTES == budget.MAX_TOTAL_BYTES == 2 * 1024 ** 3
    assert bundle.MAX_CONTROL_FILES == budget.MAX_CONTROL_FILES == 256


def test_direct_worker_rejects_missing_admission_without_creating_capture_directory(tmp_path):
    worker_path = MODULES / 'resource_probe_worker.py'
    config = tmp_path / 'absent-config.json'
    captures = tmp_path / 'not-created'
    schema = 'iroha.sumeragi_v2.resource_probe.request.v1'
    raw = b''.join(json.dumps({'schema':schema,'kind':kind,'sequence':index,'timeout_ms':1000},
                             separators=(',', ':')).encode() + b'\n'
                   for index,kind in enumerate(('preflight','finish')))
    result = subprocess.run([sys.executable,str(worker_path),'--config',str(config),
                             '--capture-dir',str(captures)],input=raw,stdout=subprocess.PIPE,
                            stderr=subprocess.PIPE,env={},cwd='/',timeout=5,check=False)
    assert result.returncode == 0 and result.stderr == b''
    assert [json.loads(row)['outcome'] for row in result.stdout.splitlines()] == ['failed','complete']
    assert not captures.exists()


def test_experiment_uses_the_same_physical_admission_and_raw_replay_owners():
    experiment = importlib.import_module('resource_experiment')
    bundle = importlib.import_module('resource_bundle')
    budget = importlib.import_module('resource_evidence_budget')
    replay = importlib.import_module('resource_replay')
    assert Path(experiment.__file__).resolve() == MODULES / 'resource_experiment.py'
    assert experiment.BudgetedBundle is bundle.BudgetedBundle
    assert experiment.ControlBinding is bundle.ControlBinding
    assert experiment.select_run_budget is budget.select_run_budget
    assert experiment.replay is replay.replay
    assert experiment.validate_replay_scope is replay.validate_replay_scope
    assert experiment.ExpectedPeer is replay.ExpectedPeer
    assert experiment.ReplayGeometry is replay.ReplayGeometry
