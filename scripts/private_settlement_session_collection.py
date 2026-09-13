"""Collect a closed registered campaign using an owned lazy evidence provider.

This is an input collector, not qualification. The caller must authenticate the
frozen plan/source admission and invoke the canonical scope reducer with the
mandatory economic, resource and packet replay. No process is launched here.
The registered scope replay and archive publisher consume this lazy inventory
with the shared retained accounting and native sample validators.
"""
from __future__ import annotations

from dataclasses import dataclass
import os
from pathlib import Path

import private_settlement_attempt_accounting as accounting
import private_settlement_record_provider as filesystem
import private_settlement_session_control as control


@dataclass
class CollectedCampaign:
    """Own the immutable provider until all reductions and archival reads end."""
    records: filesystem.RetainedRecordProvider
    packet: dict
    successful_rows: list[bytes]
    frozen_inventory: dict

    def validate(self):
        """Ensure the closed inventory did not change during downstream replay."""
        control.require(self.records.inventory() == self.frozen_inventory,
                        'closed campaign inventory changed during qualification')

    def close(self):
        """Release only this collection's filesystem descriptor."""
        self.records.close()

    def __enter__(self):
        return self

    def __exit__(self, *unused):
        self.close()


def collect_closed_campaign(root, *, scope_raw, campaign_id, plan_binding):
    """Retain all physical files, starts and samples at the registered locator."""
    root = Path(root)
    control.require(root.name == accounting._campaign_id(campaign_id),
                    'campaign directory differs from its registered identifier')
    roots = ['registered-scope.json', 'frozen-plan.json', 'campaign-closure.json']
    for name in ('attempts', 'sessions'):
        if os.path.lexists(root/name): roots.append(name)
    owner = filesystem.RetainedRecordProvider(root, roots)
    try:
        return collect_campaign_records(owner, scope_raw=scope_raw, campaign_id=campaign_id, plan_binding=plan_binding)
    except BaseException:
        owner.close()
        raise


def collect_campaign_records(owner, *, scope_raw, campaign_id, plan_binding):
    """Collect the same complete graph from an immutable or prospective owner."""
    inventory = owner.inventory()

    def retained(path, *, optional=False):
        reference = inventory.get(path)
        if reference is None:
            control.require(optional, 'closed campaign is missing a required record')
            return None
        return owner.read(reference)

    control.require(retained('registered-scope.json') == scope_raw,
                    'campaign retained a different registered scope')
    plan_raw = retained('frozen-plan.json')
    control.require(accounting.accounting_canonical_bytes(accounting.accounting_file_binding(plan_raw))
                    == accounting.accounting_canonical_bytes(plan_binding),
                    'retained plan differs from its registered binding')
    plan = accounting._document(plan_raw, 'closed campaign plan')
    accounting._retained_plan(plan)
    expected_attempts = {f"{ordinal:05}-{job['request_id']}" for ordinal, job in enumerate(plan['jobs'], 1)}
    expected_sessions = {item['session_id'] for item in plan['benchmark_sessions']}
    for namespace, allowed in (('attempts', expected_attempts), ('sessions', expected_sessions)):
        present = {path.split('/')[1] for path in (*inventory, *owner.directories)
                   if path.startswith(namespace+'/')}
        control.require(present <= allowed
                        and all(namespace+'/'+name in owner.directories for name in present)
                        and all(not path.startswith(namespace+'/') or len(Path(path).parts) > 2
                                for path in inventory),
                        'closed campaign contains an undeclared or malformed session/attempt namespace')
    sessions = []
    for descriptor in plan['benchmark_sessions']:
        prefix = 'sessions/'+descriptor['session_id']
        start = retained(prefix+'/started.json', optional=True)
        closure = inventory.get(prefix+'/session-closure.json')
        control.require((start is not None) == (closure is not None),
                        'started session lacks its authoritative closure or closure has no owner')
        sessions.append(None if start is None else {'session_id': descriptor['session_id'], 'closure': closure})
    others, samples = [], []
    for ordinal, job in enumerate(plan['jobs'], 1):
        prefix = f"attempts/{ordinal:05}-{job['request_id']}"
        if job['kind'] != 'benchmark':
            others.append({'request_id': job['request_id'], **{
                key: retained(prefix+'/'+name, optional=True)
                for key, name in (('request', 'request.json'), ('started', 'started.json'),
                                  ('process', 'process-outcome.json'))}})
            continue
        validation = retained(prefix+'/validation-outcome.json', optional=True)
        sample = retained(prefix+'/benchmark-sample.json', optional=True)
        if validation is not None:
            value = accounting._document(validation, 'retained sample validation')
            if value.get('passed') is True and value.get('validation_kind') == 'accepted':
                control.require(sample is not None, 'accepted validation omits its retained sample')
                # Native replay and all protocol identities are mandatory
                # in the reducer. ACK delivery does not remove this record.
                samples.append(sample)
    packet = {'campaign_id': campaign_id, 'plan': plan_raw,
              'closure': retained('campaign-closure.json'), 'sessions': sessions,
              'records': owner, 'nonbenchmark': others}
    collected = CollectedCampaign(owner, packet, samples, inventory)
    collected.validate()
    return collected
