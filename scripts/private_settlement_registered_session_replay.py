"""Replay one complete registered scope from immutable retained campaigns.

The public entry point retains the existing clean-checkout, signed ten-smoke,
native image and canonical plan/request admission. The private filesystem seam
can exercise synthetic records without claiming those source qualifications.
The held replay owner is shared by accounting, archive and publication consumers.
"""
from __future__ import annotations

from contextlib import ExitStack, contextmanager
from dataclasses import dataclass
import hashlib
import os
from pathlib import Path

import private_settlement_attempt_accounting as accounting
import private_settlement_release_runner as runner
import private_settlement_session_control as control
import private_settlement_session_collection as collection
import private_settlement_record_provider as filesystem
import private_settlement_sample_replay as samples


def _same(actual, expected, label):
    control.require(control.canonical(actual) == control.canonical(expected), label)


def _scope(scope_raw):
    scope = accounting.exact_fields(accounting._document(scope_raw, 'registered scope'),
                                     accounting.SCOPE_FIELDS, 'registered scope')
    accounting._header(scope, 'registered scope')
    control.digest(scope['scope_id']); control.unsigned(scope['registered_ns'])
    control.require(scope['registered_ns'] > 0 and scope['previous_scope_sha256'] is None
                    and scope['stopping_policy'] == 'fail_fast'
                    and type(scope['campaigns']) is list and bool(scope['campaigns']), 'invalid complete scope')
    accounting.validate_deadline_policy(scope['deadline_policy'])
    names = []
    for slot in scope['campaigns']:
        control.exact(slot, {'campaign_id', 'plan'}, 'registered campaign')
        names.append(accounting._campaign_id(slot['campaign_id']))
        accounting._binding(slot['plan'], 'registered plan')
    control.require(names == sorted(set(names)), 'scope campaign inventory is repeated or reordered')
    return scope


def _input_owner(plan, root, stack):
    """Hold all transitive plan inputs without duplicating capture inventories."""
    references = runner.frozen_plan_input_records(plan, root)
    paths = [item['path'] for item in references]
    control.require(len(set(paths)) == len(paths)
                    and all(Path(path).parts[0] not in {'sessions', 'attempts', 'publication'} for path in paths),
                    'plan inputs overlap execution-owned records')
    owner = stack.enter_context(filesystem.RetainedRecordProvider(root, paths))
    inventory = owner.inventory()
    for reference in references:
        expected = {key: reference[key] for key in ('path', 'sha256', 'bytes')}
        _same(inventory.get(reference['path']), expected, 'transitive plan input binding differs')
    return owner, inventory


def _requests(plan, root, collected):
    """Reconstruct every retained request, including never-started prepared rows."""
    inventory = collected.frozen_inventory
    for ordinal, job in enumerate(plan['jobs'], 1):
        path = f"attempts/{ordinal:05}-{job['request_id']}/request.json"
        if path not in inventory:
            continue
        raw = collected.records.read(inventory[path])
        actual = accounting._document(raw, 'retained full-plan request')
        execution = {**job, 'invocation_nonce': actual.get('invocation_nonce')}
        if job['kind'] == 'benchmark':
            execution['session_invocation_nonce'] = actual.get('session_invocation_nonce')
        expected = runner.build_request(plan, root, execution)
        control.require(accounting.accounting_canonical_bytes(actual) == accounting.accounting_canonical_bytes(expected),
                        'retained request differs from canonical frozen-plan replay')
        if job['kind'] == 'benchmark':
            control.require(raw == control.canonical(expected), 'native benchmark request is not canonical JSON')


@dataclass
class ClosedScope:
    """A live, immutable filesystem cut; providers close with its context."""
    scope_path: Path
    scope_raw: bytes
    scope: dict
    plans: list
    packets: list
    successful_rows: list
    owners: list
    foundations: list
    result: dict
    validate: object
    callback: object
    plan_harness: dict
    expected_commit: str

    def physical_inventory(self):
        """Bind all controlled files and empty directories, without payload copies."""
        self.validate()
        files = {'scope.json': {'path': 'scope.json', 'sha256': hashlib.sha256(self.scope_raw).hexdigest(), 'bytes': len(self.scope_raw)}}
        directories = {'campaigns'}
        for slot, owner, (foundation, _) in zip(self.scope['campaigns'], self.owners, self.foundations):
            prefix = 'campaigns/'+slot['campaign_id']
            directories.add(prefix)
            for provider in (owner.records, foundation):
                directories.update(prefix+'/'+path for path in provider.directories)
                for path, reference in provider.inventory().items():
                    located = {**reference, 'path': prefix+'/'+path}
                    control.require(located['path'] not in files or files[located['path']] == located,
                                    'physical input owners disagree')
                    files[located['path']] = located
        return {'files': dict(sorted(files.items())), 'directories': sorted(directories)}


def _replay_closed_scope(scope_path, *, plan_harness, callback, expected_commit):
    """Read-only protocol seam; source admission belongs to open_admitted_scope."""
    with _open_closed_scope(scope_path, plan_harness=plan_harness, callback=callback,
                            expected_commit=expected_commit) as held:
        return held.result


@contextmanager
def _open_closed_scope(scope_path, *, plan_harness, callback, expected_commit):
    """Filesystem/protocol integration only; this does not admit source execution."""
    control.require(type(callback) is samples.RetainedSampleReplay, 'canonical sample replay owner is mandatory')
    scope_path = Path(scope_path)
    control.require(scope_path.is_absolute() and scope_path.resolve(strict=True) == scope_path,
                    'scope locator is not canonical')
    with ExitStack() as stack:
        scope_owner = stack.enter_context(filesystem.RetainedRecordProvider(scope_path.parent, [scope_path.name]))
        inventory = scope_owner.inventory(); scope_raw = scope_owner.read(inventory[scope_path.name]); scope = _scope(scope_raw)
        campaigns_path = scope_path.parent/'campaigns'
        descriptor = os.open(campaigns_path, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)
        stack.callback(os.close, descriptor)
        before = filesystem.metadata(os.fstat(descriptor))
        scope_owner._private(os.fstat(descriptor), directory=True)
        names = [slot['campaign_id'] for slot in scope['campaigns']]
        control.require(sorted(os.listdir(descriptor)) == names
                        and filesystem.metadata(campaigns_path.lstat()) == before,
                        'scope contains missing or undeclared campaign directories')
        owners, foundations, plans, packets, successful = [], [], [], [], []
        for slot in scope['campaigns']:
            root = campaigns_path/slot['campaign_id']
            owned = stack.enter_context(collection.collect_closed_campaign(root, scope_raw=scope_raw,
                campaign_id=slot['campaign_id'], plan_binding=slot['plan']))
            owners.append(owned)
            plan, loaded_root = runner.load_plan(root/'frozen-plan.json')
            control.require(loaded_root == root and plan['commit'] == expected_commit
                            and runner.canonical_bytes(plan) == runner.canonical_bytes(accounting._document(owned.packet['plan'], 'plan')),
                            'retained plan changes its source or collection bytes')
            _same(plan['harness'], plan_harness, 'plan harness differs from independently admitted image')
            runner.load_benchmark_scope(scope_path, campaign_id=slot['campaign_id'],
                plan_binding=slot['plan'], deadline_policy=plan['benchmark_accounting'])
            foundations.append(_input_owner(plan, root, stack))
            _requests(plan, root, owned)
            plans.append(plan); packets.append(owned.packet); successful.extend(owned.successful_rows)
        worker = callback.images['worker']
        command = [worker['path'], samples.adapter.WORKER_TEST, '--exact', '--ignored', '--nocapture', '--test-threads=1']
        result = accounting.reduce_registered_scope(scope_raw, packets, successful, worker_command=command,
            worker_image={key: worker[key] for key in ('sha256', 'bytes')}, validate_success=callback)
        def validate():
            for owned, plan, (foundation, bound) in zip(owners, plans, foundations):
                owned.validate(); _same(foundation.inventory(), bound, 'plan inputs changed during scope replay')
                current, _ = runner.load_plan(owned.records.path/'frozen-plan.json')
                _same(current, plan, 'canonical plan changed during scope replay')
            control.require(filesystem.metadata(os.fstat(descriptor)) == before
                            == filesystem.metadata(campaigns_path.lstat())
                            and sorted(os.listdir(descriptor)) == names, 'campaign namespace changed during replay')
            _same(scope_owner.inventory(), inventory, 'registered scope changed during replay')
        validate()
        result = {'scope_sha256': hashlib.sha256(scope_raw).hexdigest(), 'accounting': result,
                  'plans': plans, 'successful_rows': successful, 'source_and_smoke_admitted': False,
                  'release_qualified': False}
        held = ClosedScope(scope_path, scope_raw, scope, plans, packets, successful,
                           owners, foundations, result, validate, callback, plan_harness, expected_commit)
        yield held
        validate()



def _admitted_images(worker_path, validator_path, prerequisite):
    """Join actual executable bytes to the existing source-verified smoke result."""
    images = {}
    for name, path, key in (('worker', worker_path, 'integration_sha256'), ('validator', validator_path, 'validator_sha256')):
        path = Path(path)
        control.require(path.is_absolute() and path.resolve(strict=True) == path, 'native image path is not canonical')
        image = runner.verify_harness(path)
        control.require(image['sha256'] == prerequisite[key], 'native image differs from admitted ten-smoke source')
        images[name] = {'path': str(path), **image}
    return images


@contextmanager
def open_admitted_scope(scope_path, *, source_root, plan_harness, smoke_campaign,
                            worker_path, validator_path):
    """Require unchanged existing source/native admission before full replay.

This entry point never launches a benchmark. Its Git/source and ten-smoke
readers are the same existing release owners, with no skip or fallback path.
The retained utility identities must equal independently read current images.
"""
    scope_path, source_root = Path(scope_path), Path(source_root)
    runner.require_external_output(scope_path, source_root)
    scope_raw = runner.retained_accounting_bytes(scope_path); scope = _scope(scope_raw)
    first = scope_path.parent/'campaigns'/scope['campaigns'][0]['campaign_id']/'frozen-plan.json'
    plan, _ = runner.load_plan(first); commit = plan['commit']
    runner.verify_source_checkout(source_root, commit)
    harness = runner.verify_harness(Path(plan_harness))
    prerequisite = runner.validate_smoke_prerequisite(Path(smoke_campaign), source_root=source_root, commit=commit)
    images = _admitted_images(worker_path, validator_path, prerequisite)
    ps = Path('/bin/ps').resolve(strict=True); group = runner.verify_harness(ps)
    listener = samples.network._utility_identity()
    packet = samples.semantics.packets._utility()
    callback = samples.RetainedSampleReplay(**images, owner_uid=os.geteuid(), group_utility_sha256=group['sha256'],
        listener_utility_sha256=listener[1], packet_utility={key:packet[key] for key in ('path','sha256')})
    with _open_closed_scope(scope_path, plan_harness=harness, callback=callback, expected_commit=commit) as held:
        control.require(held.result['scope_sha256'] == hashlib.sha256(scope_raw).hexdigest(), 'scope changed after source admission')
        held.result.update(source_admission=prerequisite)
        yield held
    runner.verify_source_checkout(source_root, commit)
    _same(runner.verify_harness(Path(plan_harness)), harness, 'plan harness changed during replay')
    for image in images.values():
        _same(runner.verify_harness(Path(image['path'])), {key: image[key] for key in ('sha256', 'bytes')},
              'native executable changed during replay')
    _same(runner.verify_harness(ps), group, 'group observation utility changed')
    control.require(samples.network._utility_identity() == listener, 'listener utility changed')
    _same(samples.semantics.packets._utility(), packet, 'packet utility changed')
    _same(runner.validate_smoke_prerequisite(Path(smoke_campaign), source_root=source_root, commit=commit),
          prerequisite, 'ten-smoke source admission changed during replay')
    held.result['source_and_smoke_admitted'] = True



def replay_registered_scope(scope_path, **admission):
    """Replay a complete scope under mandatory current source/native admission."""
    with open_admitted_scope(scope_path, **admission) as held:
        return held.result
