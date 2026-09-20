"""Stdlib-only resource-worker observations shared by tests and real Python children.

The static budget retains the adjacent suite file as its original named fixture
input. The complete preflight registry separately binds this module's bytes.
"""
from pathlib import Path
import hashlib
from types import SimpleNamespace

import resource_probe_worker as worker
import resource_probe as probe
import resource_process as process
import kura_resource_metrics as metrics
from resource_evidence_budget import (
    CaptureGeometry, CapturePolicy, FileBudget, RunBudget, StaticFile, admit_experiment,
    select_run_budget, RUN_FILE_FIELDS,
)

POLICY = CapturePolicy(status_body_bytes=4096, metrics_body_bytes=16 * 1024)
SUITE_SOURCE = Path(__file__).with_name('resource_probe_worker_test.py')

def allocation(count=4, policy=POLICY):
    geometry = CaptureGeometry(count, 2_000_000, 40_000_000, 2_000_000)
    runs = tuple(RunBudget(pair, variant, geometry,
                           *(FileBudget(f'pair{pair}.{variant}.{role}', 4096)
                             for role in ('journal', 'trace', 'proof', 'receipt', 'raw', *RUN_FILE_FIELDS[5:])))
                 for pair in range(1, 6) for variant in ('one_lane', 'four_lane'))
    experiment = admit_experiment(policy=policy, runs=runs,
                                 static_files=(StaticFile('worker_fixture', SUITE_SOURCE.stat().st_size),),
                                 manifest=FileBudget('manifest', 4096), report=FileBudget('report', 4096),
                                 other_control=())
    return select_run_budget(experiment, 1, 'one_lane')


def configured(result=None, collect=None):
    result = result or observation()
    return worker.ConfiguredProbe(SimpleNamespace(collect=collect or (lambda _: result)),
                                  allocation(len(result.peers), result.capture_policy))


def observation(count=4, available=True):
    rows = ['iroha_kura_resource_available 1', 'iroha_kura_resource_status{reason="available"} 1',
            'iroha_kura_resource_generation 1', 'iroha_kura_resource_fault_count 0']
    fields = ('resident_associations', 'persisted_entries', 'index_bytes', 'temporary_index_bytes', 'storage_bytes')
    for family in metrics.FAMILIES:
        rows.extend(f'iroha_kura_resource_{field}{{family="{family}"}} 0' for field in fields)
    rows.extend(f'iroha_kura_resource_{field}_sum 0' for field in fields)
    rows.append('iroha_kura_resource_represented_entries 0')
    raw = ('\n'.join(rows) + '\n').encode() if available else (
        b'iroha_kura_resource_available 0\niroha_kura_resource_status{reason="busy"} 1\n')
    kura = metrics.parse_kura_resource_metrics(raw)
    status = b'{"queue_size":5,"other":"public diagnostics"}'
    def body(route, raw, content_type):
        return probe.HttpProvenance(route, hashlib.sha256(raw).hexdigest(), len(raw), content_type, raw)
    peers = []
    for index in range(count):
        sample = process.ProcessSample(process.ProcessIdentity(100 + index, 501, 1, 0, 1,
                                       '01' * 16, 'a' * 64), 1024)
        peers.append(probe.PeerObservation(f'peer{index}', 5, body('/status', status, 'application/json'),
                                          body('/metrics', raw, 'text/plain'), kura, sample, sample))
    return probe.ProbeObservation(tuple(peers), 1000000, count * (len(raw) + len(status) + 100),
                                  count * 1024, count * 1024, count * 5, 5,
                                  probe.InventoryAggregate(0, 0) if available else None, POLICY)
