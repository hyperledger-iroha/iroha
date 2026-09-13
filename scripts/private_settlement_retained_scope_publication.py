"""Publish retained session accounting and grouped statistics from one held cut.

Public callers must use open_admitted_scope. The private held-cut functions
also support explicitly synthetic filesystem controls without source admission.
No numeric observation is manufactured for a failed or unstarted attempt.
"""
from __future__ import annotations

import hashlib
import json
from pathlib import Path

import private_settlement_registered_session_replay as replay
import private_settlement_session_control as control
import private_settlement_attempt_accounting as accounting
import private_settlement_benchmark_report as statistics

METHOD = 'retained_network_session_bootstrap_v1'


def _digest(value):
    return hashlib.sha256(accounting.accounting_canonical_bytes(value)).hexdigest()


def _held(held):
    control.require(type(held) is replay.ClosedScope, 'held canonical scope replay owner is mandatory')
    held.validate()


def _session_groups(held):
    """Project only authenticated ready network owners, including empty groups."""
    _held(held)
    values = [accounting._document(raw, 'accepted retained sample') for raw in held.successful_rows]
    by_session = {}
    for row in values:
        key = row['campaign_id'], row['session_id'], row['session_invocation_nonce']
        by_session.setdefault(key, []).append(row)
    groups = []
    for slot, plan, owner, summary in zip(held.scope['campaigns'], held.plans, held.owners, held.result['accounting']['campaigns']):
        descriptors = {item['session_id']: item for item in plan['benchmark_sessions']}
        for session in summary['sessions']:
            if session is None:
                continue
            closure = accounting._document(owner.records.read(session['closure']), 'session closure')
            if closure['ready'] is None:
                continue
            descriptor = descriptors[session['session_id']]
            key = slot['campaign_id'], session['session_id'], session['session_invocation_nonce']
            identity = {'scope_sha256': held.result['scope_sha256'], 'campaign_id': key[0],
                        'session_id': key[1], 'session_invocation_nonce': key[2]}
            rows = by_session.pop(key, [])
            measured = {row['attempt_id']: row for row in rows if not row['warmup']}
            groups.append({'group_id': _digest(identity), **identity, **descriptor,
                           'measured': measured, 'warmups': sum(row['warmup'] for row in rows)})
    control.require(not by_session, 'accepted sample has no authenticated ready session')
    return groups


def _summary(groups, binding, iterations, *, paired=False):
    if len(groups) < statistics.MIN_SEEDS:
        return {'status': 'insufficient_independent_sessions', 'session_count': len(groups),
                'count': sum(map(len, groups.values())), 'observations_per_session': {key: len(value) for key,value in sorted(groups.items())},
                'unconditional_quantiles': 'not_estimated'}
    method = statistics.summarize_paired_session_values if paired else statistics.summarize_session_values
    return {'status': 'estimated', **method(groups, binding=bytes.fromhex(binding), bootstrap_iterations=iterations)}


def build_report(held, bootstrap_iterations):
    """Regenerate the report from complete filesystem and native sample replay."""
    _held(held)
    control.require(type(bootstrap_iterations) is int and bootstrap_iterations >= 100
                    and all(plan['requirements']['bootstrap_iterations'] == bootstrap_iterations for plan in held.plans),
                    'bootstrap policy differs from frozen plan')
    physical = held.physical_inventory()
    input_sha = _digest({'method': METHOD, 'inventory': physical})
    groups = _session_groups(held)
    plan = held.plans[0]
    configurations = {str(item['participants']): item['configuration_sha256'] for item in plan['benchmark_sessions']}
    report = {'version': 1, 'protocol': control.PROTOCOL, 'commit': plan['commit'], 'method': METHOD,
        'input_sha256': input_sha,
        'environment': {'hardware_sha256': plan['hardware']['sha256'],
            'hardware_profile_sha256': plan['hardware']['profile_sha256'],
            'configuration_sha256_by_participants': configurations},
        'requirements': {'participants': list(statistics.REQUIRED_PARTICIPANTS),
            'minimum_warmups_per_session': statistics.MIN_WARMUPS, 'minimum_measured': statistics.MIN_MEASURED,
            'minimum_seeds': statistics.MIN_SEEDS, 'bootstrap_iterations': bootstrap_iterations},
        'profiles': {}, 'paired_profiles': {}, 'accounting': held.result['accounting'],
        'statistical_qualification_passed': True, 'unconditional_quantiles': 'not_estimated'}
    for profile in statistics.PROFILES:
        report['profiles'][profile] = {}
        for participants in statistics.REQUIRED_PARTICIPANTS:
            selected = [group for group in groups if (group['profile'],group['participants']) == (profile,participants)]
            measured = [row for group in selected for row in group['measured'].values()]
            stage_names = statistics.REQUIRED_PRIVATE_STAGES if profile == 'private' else ('global_finality','end_to_end')
            bucket = {'ready_sessions': len(selected), 'empty_measured_sessions': sum(not group['measured'] for group in selected),
                'measured_runs': len(measured), 'seeds': sorted({row['seed'] for row in measured}),
                'successful_warmups': sum(group['warmups'] for group in selected), 'stages_ms': {}, 'resources': {}}
            for category, metrics in (('stages_ms', stage_names), ('resources', statistics.RESOURCE_FIELDS)):
                for metric in metrics:
                    observations = {group['group_id']: {aid: row['stages_ms'][metric] if category == 'stages_ms' else row[metric]
                                    for aid,row in group['measured'].items()} for group in selected}
                    binding = _digest([input_sha, profile, participants, category, metric])
                    bucket[category][metric] = _summary(observations, binding, bootstrap_iterations)
            enough = len(measured) >= statistics.MIN_MEASURED and len(bucket['seeds']) >= statistics.MIN_SEEDS
            # The reducer already proves the ordered per-session warmup prefix
            # for every accepted measured attempt; aggregate warmups cannot substitute.
            enough = enough and len(selected) >= statistics.MIN_SEEDS
            enough = enough and all(summary['status'] == 'estimated'
                and all(summary[label+'_ci95'] is not None for label in ('p50','p95','p99'))
                for category in ('stages_ms','resources') for summary in bucket[category].values())
            report['statistical_qualification_passed'] &= enough
            report['profiles'][profile][str(participants)] = bucket
    for participants in statistics.REQUIRED_PARTICIPANTS:
        pairs = {}
        for group in groups:
            if group['participants'] != participants: continue
            key = (group['campaign_id'], participants, group['seed'], group['configuration_sha256'], group['workload_manifest_sha256'])
            control.require(group['profile'] not in pairs.setdefault(key, {}), 'duplicate retained network pairing owner')
            pairs[key][group['profile']] = group
        complete, unmatched, paired_seeds = [], 0, set()
        for key, pair in sorted(pairs.items()):
            if set(pair) != set(statistics.PROFILES): unmatched += len(pair); continue
            private, transparent = pair['private'], pair['transparent_control']
            left = {row['session_attempt_index']: row for row in private['measured'].values()}
            right = {row['session_attempt_index']: row for row in transparent['measured'].values()}
            matched = {}
            for index in sorted(left.keys() & right.keys()):
                a,b = left[index],right[index]
                control.require(a['economic_vector_sha256'] == b['economic_vector_sha256'],
                                'paired success differs in its native economic vector')
                matched[_digest([a['attempt_id'], b['attempt_id'], a['economic_vector_sha256']])] = (a,b)
            complete.append((_digest([key,private['group_id'],transparent['group_id']]),matched))
            if matched: paired_seeds.add(key[2])
        bucket = {'ready_session_pairs': len(complete), 'unmatched_ready_sessions': unmatched,
                  'paired_measured_successes': sum(len(rows) for _,rows in complete),
                  'paired_seeds': sorted(paired_seeds), 'stages_ms': {}, 'resources': {}}
        for category,metrics in (('stages_ms',('global_finality','end_to_end')),('resources',statistics.RESOURCE_FIELDS)):
            for metric in metrics:
                observations = {gid: {aid: tuple(row['stages_ms'][metric] if category == 'stages_ms' else row[metric] for row in rows)
                                for aid,rows in matched.items()} for gid,matched in complete}
                bucket[category][metric] = _summary(observations, _digest([input_sha,'paired',participants,category,metric]),
                                                    bootstrap_iterations, paired=True)
        report['paired_profiles'][str(participants)] = bucket
        report['statistical_qualification_passed'] &= (len(complete) >= statistics.MIN_SEEDS
            and len(paired_seeds) >= statistics.MIN_SEEDS and bucket['paired_measured_successes'] >= statistics.MIN_MEASURED)
    report['statistical_qualification_passed'] &= report['accounting']['accounting_complete'] is True
    _held(held)
    return report


def require_qualified(report):
    """Keep release minima separate from honest reports of incomplete campaigns."""
    control.require(report.get('method') == METHOD and report.get('statistical_qualification_passed') is True
                    and report['accounting']['accounting_complete'] is True,
                    'retained scope does not meet complete release statistical qualification')
    for profile in statistics.PROFILES:
        for n in statistics.REQUIRED_PARTICIPANTS:
            bucket = report['profiles'][profile][str(n)]
            control.require(bucket['measured_runs'] >= statistics.MIN_MEASURED
                            and len(bucket['seeds']) >= statistics.MIN_SEEDS
                            and bucket['ready_sessions'] >= statistics.MIN_SEEDS,
                            'retained statistical cohort lacks measured independent sessions')
            control.require(all(summary.get('status') == 'estimated'
                and all(summary.get(label+'_ci95') is not None for label in ('p50','p95','p99'))
                for category in ('stages_ms','resources') for summary in bucket[category].values()),
                'required measured-success confidence interval is undefined')
    for n in statistics.REQUIRED_PARTICIPANTS:
        pair=report['paired_profiles'][str(n)]
        control.require(pair['ready_session_pairs'] >= statistics.MIN_SEEDS
            and len(pair['paired_seeds']) >= statistics.MIN_SEEDS
            and pair['paired_measured_successes'] >= statistics.MIN_MEASURED,
            'matched profile comparison lacks paired independent evidence')


def archive(held, publication):
    """Stream the complete cut, then replay its exact copied contents and directories."""
    _held(held)
    publication = Path(publication); root = publication/'accounting'
    root.mkdir(mode=0o700)
    physical = held.physical_inventory()
    for path in physical['directories']:
        (root/path).mkdir(mode=0o700, parents=True, exist_ok=True)
    target = root/'scope.json'; target.write_bytes(held.scope_raw); target.chmod(0o600)
    copied = {'scope.json'}
    for slot, owner, (foundation, _) in zip(held.scope['campaigns'], held.owners, held.foundations):
        prefix = 'campaigns/'+slot['campaign_id']
        for provider in (owner.records, foundation):
            for path, reference in provider.inventory().items():
                located = prefix+'/'+path
                if located in copied: continue
                provider.copy_to(reference, root/located); copied.add(located)
    with replay._open_closed_scope(root/'scope.json', plan_harness=held.plan_harness, callback=held.callback,
                                   expected_commit=held.expected_commit) as copied_scope:
        control.require(copied_scope.physical_inventory() == physical
                        and copied_scope.result['accounting'] == held.result['accounting']
                        and copied_scope.successful_rows == held.successful_rows,
                        'copied retained scope differs from full original replay')
    _held(held)
    artifacts = [{'kind': 'benchmark_scope' if path == 'scope.json' else 'benchmark_accounting_record',
                  **reference, 'path': 'accounting/'+path} for path,reference in physical['files'].items()]
    inventory_path = publication/'reports/benchmark-physical-inventory-v1.json'
    replay.runner.write_json(inventory_path, physical)
    artifacts.append({'kind':'benchmark_accounting_record', **replay.runner.file_binding(inventory_path,relative_to=publication)})
    report_path = publication/'reports/benchmark-accounting-v1.json'
    replay.runner.write_json(report_path,held.result['accounting'])
    artifacts.append({'kind':'benchmark_accounting_report',**replay.runner.file_binding(report_path,relative_to=publication)})
    return artifacts, held.result['accounting'], [accounting._document(raw,'accepted sample') for raw in held.successful_rows]


def validate_archive_inventory(held, root, artifacts):
    """Join manifest roles, complete physical content and empty-directory inventory."""
    physical = held.physical_inventory(); root = Path(root)
    expected = {'accounting/'+path: ('benchmark_scope' if path == 'scope.json' else 'benchmark_accounting_record', ref['sha256'],ref['bytes'])
                for path,ref in physical['files'].items()}
    inventory_path = 'reports/benchmark-physical-inventory-v1.json'
    # This is an aggregate manifest, not a protocol frame. Use the existing
    # release-manifest bound; individual lazy record reads keep their frame limit.
    evidence = replay.runner.release_evidence
    inventory_binding = replay.runner.file_binding(root/inventory_path)
    inventory_record = evidence._read_strict_json_file(root/inventory_path,
        maximum_bytes=evidence._MAX_RELEASE_MANIFEST_BYTES, label='physical inventory')
    control.require(inventory_record == physical,
                    'manifest-bound physical directory inventory differs')
    control.require(replay.runner.file_binding(root/inventory_path) == inventory_binding,
                    'physical inventory changed during validation')
    expected[inventory_path] = ('benchmark_accounting_record',inventory_binding['sha256'],inventory_binding['bytes'])
    actual = {str(item.path):(item.kind,item.sha256,item.bytes) for item in artifacts
              if str(item.path).startswith('accounting/') or item.kind in {'benchmark_scope','benchmark_accounting_record'}}
    control.require(actual == expected, 'controlled accounting artifact inventory is incomplete or differs')


def validate_raw(paths, held):
    """Require the exact accepted sample records, including those before ACK loss."""
    expected = {accounting._document(raw,'sample')['attempt_id']: raw for raw in held.successful_rows}
    seen = {}
    for path in paths:
        with Path(path).open('rb') as stream:
            while True:
                line = stream.readline(replay.filesystem.MAX_RECORD_BYTES+2)
                if not line: break
                control.require(len(line) <= replay.filesystem.MAX_RECORD_BYTES+1 and line.endswith(b'\n'),
                                'raw sample line is unbounded or unterminated')
                row = accounting._document(line[:-1],'public sample'); aid = row['attempt_id']
                control.require(aid not in seen and aid in expected
                    and accounting.accounting_canonical_bytes(row) == accounting.accounting_canonical_bytes(accounting._document(expected[aid],'retained sample')),
                    'public raw sample differs from accepted native replay')
                seen[aid] = line
    control.require(seen.keys() == expected.keys(), 'public raw sample omits an accepted attempt')
