"""Actual raw publishers and replays for pure resource reduction tests.

This fixture grants no completed native authority and creates no child process.
Expected identities and resource values are constructed independently of captures.
"""
from dataclasses import asdict, dataclass, replace
from pathlib import Path
import sys
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'scripts/nexus'))
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'scripts/tests'))
from signed_request_fixture import add_retention
import resource_experiment as experiment
import resource_bundle as bundle
import resource_evidence_budget as budget
import resource_replay as replay
import resource_probe as probe
import resource_probe_worker as worker
import resource_process as process
import kura_resource_metrics as metrics
import resource_replay_test as frozen
RUNS = tuple((pair, variant) for pair in range(1, 6) for variant in ('one_lane', 'four_lane'))

@dataclass(frozen=True)
class RawPublisherInput:
    """Test-selected raw publisher inputs; never a production admission token."""
    pair_index: int
    variant: str
    peers: tuple
    geometry: replay.ReplayGeometry

def observation(peers, policy, boost, preflight=False):
    rows = []
    for index, peer in enumerate(peers):
        status = frozen.encode({'queue_size': index + 1 + boost})
        raw = frozen.body(100 + index + boost)
        raw = raw.replace(b'family="resident_canonical"} 3\n', f'family="resident_canonical"}} {3 + boost}\n'.encode())
        raw = raw.replace(b'resident_associations_sum 3\n', f'resident_associations_sum {3 + boost}\n'.encode())
        raw = raw.replace(b'represented_entries 3\n', f'represented_entries {3 + boost}\n'.encode())

        def provenance(route, value, kind):
            return probe.HttpProvenance(route, frozen.sha(value), len(value), kind, value)
        rows.append(probe.PeerObservation(peer.peer_id, index + 1 + boost, provenance('/status', status, 'application/json'), provenance('/metrics', raw, 'text/plain'), metrics.parse_kura_resource_metrics(raw), process.ProcessSample(peer.identity, (5000 if preflight else 1000) + index + boost), process.ProcessSample(peer.identity, (1100 if preflight else 4000) + index + boost)))
    return probe.ProbeObservation(tuple(rows), 1000000, sum((len(row.status.raw_body) + len(row.metrics.raw_body) + 200 for row in rows)), sum((row.process_before.rss_bytes for row in rows)), sum((row.process_after.rss_bytes for row in rows)), sum((row.queue_size for row in rows)), max((row.queue_size for row in rows)), probe.InventoryAggregate(sum((100 + index + boost for index in range(len(rows)))), len(rows) * (3 + boost)), policy)

class RawPublishedRuns:

    def __init__(self, root):
        self.root = root.resolve()
        self.root.mkdir(mode=448)
        self.geometry = replay.ReplayGeometry(100000000, 200000000, 10000000, 20000000, 10000000, 4000000, 2000000)
        self.budget = frozen.admitted_allocation(4, self.geometry, budget.CapturePolicy()).experiment
        self.runs = tuple((RawPublisherInput(pair, variant, tuple((replace(peer, identity=replace(peer.identity, pid=peer.identity.pid + index * 10, start_seconds=index + 1, start_abstime=peer.identity.start_abstime + index * 10)) for peer in frozen.identities())), self.geometry) for index, (pair, variant) in enumerate(RUNS)))
        self.events = []
        journals = {}
        for run_index, run in enumerate(self.runs):
            allocation = budget.select_run_budget(self.budget, run.pair_index, run.variant)
            directory = self.captures(run_index)
            directory.mkdir(mode=448, parents=True)
            geometry = run.geometry
            plan = {key: 1 for key in replay.PLAN_FIELDS}
            plan.update(event='plan', local_applied_required=True, schema=replay.JOURNAL_SCHEMA, scheduled_requests=1, pair_index=run.pair_index, variant=run.variant, **{name: getattr(geometry, name) for name in ('warmup_ns', 'measurement_ns', 'drain_ns', 'preparation_ahead_ns')})
            events = [plan]
            with worker.CaptureDirectory(directory, allocation) as owner:
                ref = owner.publish('preflight', 0, observation(run.peers, allocation.policy, 100 + 10 * run_index, True), probe._Deadline(5))
                events.extend(({'event': 'resource_preflight', 'sequence': 0, 'outcome': 'complete', 'manifest': asdict(ref), 'sampling': geometry.sampling()}, {'event': 'clock_started', 'initial_offset_ns': -(geometry.warmup_ns + geometry.drain_ns + geometry.preparation_ahead_ns)}))
                for sample in range(geometry.samples):
                    scheduled = sample * geometry.interval_ns
                    ref = owner.publish('sample', sample + 1, observation(run.peers, allocation.policy, sample + 10 * run_index), probe._Deadline(5))
                    events.extend(({'event': 'resource_request', 'kind': 'sample', 'sequence': sample + 1, 'scheduled_offset_ns': scheduled, 'start_offset_ns': scheduled}, {'event': 'resource_observation', 'sequence': sample + 1, 'scheduled_offset_ns': scheduled, 'start_offset_ns': scheduled, 'end_offset_ns': scheduled + 3000000, 'outcome': 'complete', 'manifest': asdict(ref)}))
            end = geometry.final + 3000000
            events.extend(({'event': 'resource_request', 'kind': 'finish', 'sequence': geometry.samples + 1, 'start_offset_ns': end}, {'event': 'resource_collection_finished', 'sequence': geometry.samples + 1, 'start_offset_ns': end, 'end_offset_ns': end + 1000000, 'sampling': geometry.sampling()}, {'event': 'request_final', 'plan': {'cohort': 'measurement', 'sequence': 1, 'logical_id': 'b' * 64, 'scheduled_offset_ns': 0, 'account_index': 0}, 'hash': 'c' * 63 + '1', 'offer_offset_ns': 0, 'acknowledgment_offset_ns': 1, 'applied_offset_ns': geometry.final, 'block_height': 1, 'local_applied_offset_ns': geometry.final, 'local_block_height': 1, 'local_status_attempts': 1, 'status_attempts': 1, 'submission_finished': True, 'failure': None}, {'event': 'collection_finished', 'passed': True, 'failure': None}))
            events = add_retention(events)
            self.events.append(events)
            journals[allocation.journal.label] = b''.join((frozen.encode(row) + b'\n' for row in events))
        controls = []
        for item in (*self.budget.static_files, *self.budget.control_budgets, *(item for row in self.budget.runs for item in row.files)):
            if item.label == 'report':
                continue
            raw = b'x' * item.size_bytes if type(item) is budget.StaticFile else journals.get(item.label, b'x')
            path = self.root / 'controls' / item.label
            path.parent.mkdir(exist_ok=True)
            path.write_bytes(raw)
            path.chmod(384)
            controls.append(bundle.ControlBinding(item.label, str(path.relative_to(self.root)), frozen.sha(raw)))
        self.controls = tuple(controls)

    def captures(self, index):
        pair, variant = RUNS[index]
        return self.root / 'resources' / f'pair-{pair:02}' / variant

    def member(self, index=0, name='sample-0000000001-peer-0000-status.body'):
        return self.captures(index) / name

    def journal(self, index):
        run = self.budget.runs[index]
        binding = next((row for row in self.controls if row.label == run.collector_journal.label))
        return self.root / binding.path

    def save_journal(self, index):
        raw = b''.join((frozen.encode(row) + b'\n' for row in self.events[index]))
        self.journal(index).write_bytes(raw)
        label = self.budget.runs[index].collector_journal.label
        self.controls = tuple((replace(item, sha256=frozen.sha(raw)) if item.label == label else item for item in self.controls))

    def replay_run(self, index):
        """Read original raw files through the actual reducer, without native claims."""
        row = self.runs[index]
        allocation = budget.select_run_budget(self.budget, row.pair_index, row.variant)
        reduced = replay.replay(self.captures(index), self.journal(index),
            frozen.sha(self.journal(index).read_bytes()), row.peers, row.geometry,
            expected_policy=allocation.policy, allocation=allocation)
        resources = experiment._resource_snapshot(reduced)
        maxima = experiment._maxima((reduced.preflight, *(sample.capture for sample in reduced.samples)))
        return experiment.RunResourceResult(row.pair_index, row.variant, resources, maxima)
