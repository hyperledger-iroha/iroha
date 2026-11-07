#!/usr/bin/env python3
"""Consume independently approved Linux observer artifacts and run all native tests.

Requires Python 3.12, Git, Linux x86_64 for execution, separate clean harness and
observer checkouts, and an approval record from an independently selected immutable
commit. GitHub downloads the exact approved artifact; no build or hash promotion
occurs here. Output must be a new private directory outside both source checkouts.
No signing credentials, publication or fallback artifacts. A fixed reviewed four-way
partition may execute on separate hosted VMs; only its exact-once aggregate qualifies
the full suite. GitHub metadata and original ZIPs are consumed as data, never code.
"""
from __future__ import annotations

import argparse
import ast
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import platform
import re
import resource
import stat
import subprocess
import sys
import zipfile

FIELDS = {'schema', 'status', 'repository', 'producer_commit', 'producer_head_commit', 'producer_run_id',
          'producer_run_attempt', 'artifact_id', 'artifact_name', 'artifact_sha256',
          'candidate_sha256', 'source_commit', 'source_manifest_sha256', 'native_target',
          'module_sha256', 'provenance_sha256', 'review'}
SHA_FIELDS = ('artifact_sha256', 'candidate_sha256', 'source_manifest_sha256',
              'module_sha256', 'provenance_sha256')
WORKFLOW = '.github/workflows/linux_observer_candidate.yml'
LIBRARY = 'libiroha_process_bridge.so'
SHARDS = ('native-1', 'native-2', 'native-3', 'native-4')
CONTEXT_FILES = {
    'consumer_sha256': 'scripts/run_required_linux_resource_harness.py',
    'runner_sha256': 'scripts/tests/run_zk_resource_harness_native.py',
    'tests_sha256': 'scripts/tests/zk_resource_harness_test.py',
    'inventory_sha256': 'ci/linux_native_harness_tests.json',
    'partition_sha256': 'ci/linux_native_harness_shards.json',
}
CONTEXT_FIELDS = {'schema', 'repository', 'run_id', 'run_attempt', 'head_commit',
                  'harness_commit', 'approval_commit', 'approval_sha256',
                  'approved_pins', *CONTEXT_FILES}



def require(condition, message):
    """Reject missing approval, mismatched evidence and incomplete execution."""
    if not condition:
        raise ValueError(message)


def unique_fields(pairs):
    """Reject duplicate JSON keys rather than choosing one interpretation."""
    result = {}
    for key, value in pairs:
        require(key not in result, 'duplicate JSON field')
        result[key] = value
    return result


def read_json(path):
    """Read a bounded control document without importing or executing its contents."""
    with Path(path).open('rb') as handle:
        raw = handle.read(1024 * 1024 + 1)
    require(len(raw) <= 1024 * 1024, 'control document exceeds 1 MiB')
    return json.loads(raw, object_pairs_hook=unique_fields)


def digest(path):
    """Compare observed bytes with an already supplied independent pin."""
    with Path(path).open('rb') as handle:
        return hashlib.file_digest(handle, 'sha256').hexdigest()


def exact_hex(value, count):
    """Accept nonzero lowercase immutable commit or digest spelling only."""
    return type(value) is str and re.fullmatch('[0-9a-f]{' + str(count) + '}', value) is not None and value != '0' * count


def exact_id(value, expected):
    """JSON booleans must never alias positive integer run/attempt identifiers."""
    return type(value) is int and value > 0 and value == expected


def same_json(left, right):
    """Compare JSON identities without Python's True == 1 coercion."""
    return json.dumps(left, sort_keys=True, separators=(',', ':')) == json.dumps(right, sort_keys=True, separators=(',', ':'))


def approval(value):
    """Require a complete approved record; candidate receipts are never authority."""
    require(type(value) is dict and set(value) == FIELDS, 'approval fields are incomplete or unknown')
    require(value['schema'] == 'iroha.linux-observer-approval.v1' and value['status'] == 'approved',
            'independent approval is required')
    require(type(value['repository']) is str and re.fullmatch(r'[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+', value['repository']),
            'repository must be explicit')
    require(all(exact_hex(value[key], 64) for key in SHA_FIELDS), 'complete independent digest pins are required')
    require(all(exact_hex(value[key], 40) for key in ('source_commit', 'producer_commit', 'producer_head_commit')),
            'exact source, producer checkout and producer API head commits required')
    require(all(type(value[key]) is int and value[key] > 0 for key in ('producer_run_id', 'producer_run_attempt', 'artifact_id')),
            'immutable artifact and run identifiers required')
    expected_name = f"linux-observer-candidate-{value['producer_run_id']}-{value['producer_run_attempt']}"
    require(value['artifact_name'] == expected_name, 'artifact name differs from approved run attempt')
    require(value['native_target'] == 'x86_64-unknown-linux-gnu', 'native Linux GNU x86_64 approval required')
    review = value['review']
    require(type(review) is dict and set(review) == {'reviewer', 'evidence_url', 'reviewed_at_utc'}, 'independent review evidence required')
    require(type(review['reviewer']) is str and re.fullmatch(r'[A-Za-z0-9-]{1,39}', review['reviewer']), 'reviewer required')
    require(type(review['evidence_url']) is str and review['evidence_url'].startswith('https://github.com/' + value['repository'] + '/'),
            'review evidence must identify this repository')
    require(type(review['reviewed_at_utc']) is str and re.fullmatch(r'\d{4}-\d\d-\d\dT\d\d:\d\d:\d\dZ', review['reviewed_at_utc']),
            'UTC review timestamp required')
    return value


def metadata(config, artifact, run):
    """Bind GitHub's artifact and completed run metadata to independent approval."""
    require(exact_id(artifact.get('id'), config['artifact_id']) and artifact.get('name') == config['artifact_name']
            and artifact.get('expired') is False and artifact.get('digest') == 'sha256:' + config['artifact_sha256'],
            'artifact metadata differs from approval or has expired')
    require(exact_id(artifact.get('workflow_run', {}).get('id'), config['producer_run_id']), 'artifact belongs to another run')
    require(exact_id(run.get('id'), config['producer_run_id']) and exact_id(run.get('run_attempt'), config['producer_run_attempt'])
            and run.get('head_sha') == config['producer_head_commit'] and run.get('path') == WORKFLOW
            and run.get('repository', {}).get('full_name') == config['repository']
            and run.get('status') == 'completed' and run.get('conclusion') == 'success',
            'producer run differs from the reviewed successful attempt')


def test_inventory(harness):
    """Require the exact complete 20-method class, with no guessed or missing tests."""
    expected = read_json(harness / 'ci/linux_native_harness_tests.json')
    tree = ast.parse((harness / 'scripts/tests/zk_resource_harness_test.py').read_text())
    classes = [node for node in tree.body if isinstance(node, ast.ClassDef) and node.name == 'NativeRunTests']
    require(len(classes) == 1, 'exactly one NativeRunTests class required')
    actual = sorted(node.name for node in classes[0].body if isinstance(node, ast.FunctionDef) and node.name.startswith('test_'))
    require(type(expected) is list and expected == actual and len(expected) == len(set(expected)) == 20,
            'all 20 native method names must match the current class')
    return expected


def unpack(config, archive, output):
    """Verify the immutable archive before copying only its three reviewed originals."""
    require(digest(archive) == config['artifact_sha256'], 'archive differs from independent digest')
    bridge = output / 'bridge'
    bridge.mkdir(mode=0o700)
    wanted = {'review/candidate.json': ('candidate.json', config['candidate_sha256'], 1024 * 1024),
              'review/bridge/' + LIBRARY: ('bridge/' + LIBRARY, config['module_sha256'], 2 * 1024**3),
              'review/bridge/publisher-source.json': ('bridge/publisher-source.json', config['provenance_sha256'], 512 * 1024)}
    with zipfile.ZipFile(archive) as packet:
        entries = packet.infolist()
        names = [entry.filename for entry in entries]
        require(len(names) == len(set(names)) and len(names) <= 128, 'ambiguous or excessive artifact members')
        for entry in entries:
            path = PurePosixPath(entry.filename)
            require(not path.is_absolute() and '..' not in path.parts and '\\' not in entry.filename
                    and not stat.S_ISLNK(entry.external_attr >> 16), 'unsafe artifact member')
        require(set(wanted) <= set(names), 'required candidate originals are missing')
        for name, (target, expected, maximum) in wanted.items():
            entry = packet.getinfo(name)
            require(0 < entry.file_size <= maximum and not entry.is_dir(), 'artifact member exceeds its bound')
            path = output / target
            count = 0
            with packet.open(entry) as source, path.open('xb') as destination:
                while block := source.read(64 * 1024):
                    count += len(block)
                    require(count <= maximum, 'expanded artifact exceeds its bound')
                    destination.write(block)
            require(count == entry.file_size and digest(path) == expected, 'original artifact digest mismatch')
            path.chmod(0o500 if target.endswith('.so') else 0o400)
    candidate = read_json(output / 'candidate.json')
    require(candidate.get('schema') == 'iroha.linux-observer-candidate.v1'
            and candidate.get('status') == 'requires_independent_approval' and candidate.get('bridge_executed') is False,
            'not an unexecuted normal-publication candidate')
    for candidate_key, approved_key in [('observer_commit', 'source_commit'), ('driver_revision', 'producer_commit'),
                                      ('native_target', 'native_target'), ('module_sha256', 'module_sha256'),
                                      ('provenance_sha256', 'provenance_sha256'), ('source_manifest_sha256', 'source_manifest_sha256')]:
        require(candidate.get(candidate_key) == config[approved_key], 'candidate receipt differs from approval')
    return bridge


def successful_suite(returncode, text, selected):
    """Require exactly the closed selection, each once, with no failure or skip."""
    require(returncode == 0, 'actual native suite failed; see retained log')
    require(selected and len(selected) == len(set(selected)), 'unique selected methods required')
    terminals = re.findall(r'(?m)^Ran (\d+) tests? in [^\n]+$', text)
    require(terminals == [str(len(selected))] and text.rstrip().endswith('\nOK'),
            'all selected native tests must finish with zero skips')
    rows = re.findall(r'(?m)^(test_[^\s]+) \(([^\n]+)\) \.\.\. ([^\n]+)$', text)
    require(len(re.findall(r'(?m)^test_[^\n]*$', text)) == len(rows)
            and len(rows) == len(selected) and sorted(row[0] for row in rows) == sorted(selected)
            and all(owner == 'zk_resource_harness_test.NativeRunTests.' + name and verdict == 'ok'
                    for name, owner, verdict in rows),
            'each exact named native method must succeed exactly once')
    require(not re.search(r'(?m)^(FAIL:|ERROR:|FAILED |OK \(|  test_)', text),
            'native failure, skipped result or unexpected subtest result')


def partitions(harness):
    """Load only the reviewed four partitions; their disjoint union must be all 20."""
    value = read_json(harness / 'ci/linux_native_harness_shards.json')
    require(type(value) is dict and set(value) == {'schema', 'shards'}
            and value['schema'] == 'iroha.linux-native-harness-shards.v1', 'closed partition schema required')
    shards = value['shards']
    require(type(shards) is dict and set(shards) == set(SHARDS), 'exactly four known shards required')
    names = []
    for selected in shards.values():
        require(type(selected) is list and selected and all(type(name) is str for name in selected)
                and selected == sorted(set(selected)), 'partition methods must be explicit sorted unique names')
        names.extend(selected)
    require(len(names) == len(set(names)) == 20 and sorted(names) == test_inventory(harness),
            'partitions must cover all 20 methods exactly once')
    return shards


def clean_checkout(root, commit):
    """Bind a physical checkout to its exact immutable revision before execution."""
    require(exact_hex(commit, 40), 'immutable checkout commit required')
    git = lambda *args: subprocess.check_output(['git', '-C', str(root), *args]).strip()
    require(git('rev-parse', 'HEAD').decode() == commit, 'checkout differs from immutable context')
    require(not git('status', '--porcelain=v1', '--untracked-files=all'), 'context checkout is dirty')


def make_context(args, config):
    """Freeze the externally selected approval once for every VM and the aggregate."""
    harness, owner = Path(args.harness_root).resolve(), Path(args.approval_root).resolve()
    require(Path(args.config).resolve() == owner / 'ci/linux_observer_approval.json',
            'approval must come from its independently checked out record')
    clean_checkout(harness, args.harness_commit)
    clean_checkout(owner, args.approval_commit)
    partitions(harness)
    require(exact_hex(args.head_commit, 40), 'API head commit required independently of merge checkout')
    require(re.fullmatch(r'[1-9][0-9]*', args.run_id) and re.fullmatch(r'[1-9][0-9]*', args.run_attempt),
            'positive immutable run and attempt required')
    result = {'schema': 'iroha.linux-resource-harness-inputs.v1', 'repository': config['repository'],
              'run_id': int(args.run_id), 'run_attempt': int(args.run_attempt),
              'head_commit': args.head_commit, 'harness_commit': args.harness_commit,
              'approval_commit': args.approval_commit, 'approval_sha256': digest(args.config),
              'approved_pins': config,
              **{key: digest(harness / path) for key, path in CONTEXT_FILES.items()}}
    current_environment(result)
    return result


def current_environment(context):
    """Prevent a successful receipt from another run, attempt or merge checkout."""
    for key, variable in [('repository', 'GITHUB_REPOSITORY'), ('run_id', 'GITHUB_RUN_ID'),
                          ('run_attempt', 'GITHUB_RUN_ATTEMPT'), ('harness_commit', 'GITHUB_SHA')]:
        require(str(context[key]) == os.environ.get(variable), 'context differs from current GitHub ' + variable)


def validate_context(context, config, harness, config_path):
    """Recheck shared source, approval and policy identity on every independent VM."""
    require(type(context) is dict and set(context) == CONTEXT_FIELDS
            and context['schema'] == 'iroha.linux-resource-harness-inputs.v1', 'exact immutable input context required')
    require(context['repository'] == config['repository'] and same_json(context['approved_pins'], config),
            'context approval differs from independently selected pins')
    require(all(exact_hex(context[key], 40) for key in ('head_commit', 'harness_commit', 'approval_commit'))
            and all(exact_hex(context[key], 64) for key in ('approval_sha256', *CONTEXT_FILES)),
            'context source and digest pins required')
    require(all(type(context[key]) is int and context[key] > 0 for key in ('run_id', 'run_attempt')),
            'context run and attempt identifiers required')
    current_environment(context)
    clean_checkout(harness, context['harness_commit'])
    config_path = Path(config_path).resolve()
    require(config_path.name == 'linux_observer_approval.json' and config_path.parent.name == 'ci',
            'independent approval record path required')
    clean_checkout(config_path.parent.parent, context['approval_commit'])
    require(digest(config_path) == context['approval_sha256'], 'approval bytes differ from frozen context')
    for key, path in CONTEXT_FILES.items():
        require(digest(harness / path) == context[key], 'harness bytes differ from frozen context: ' + path)
    return partitions(harness)


def api_items(value, key):
    """Accept one API page or gh --paginate --slurp pages, with complete totals."""
    pages = value if type(value) is list else [value]
    require(pages and all(type(page) is dict and type(page.get(key)) is list
                         and type(page.get('total_count')) is int for page in pages), 'complete API pages required')
    total = pages[0]['total_count']
    items = [item for page in pages for item in page[key]]
    require(all(page['total_count'] == total for page in pages) and total == len(items)
            and all(type(item) is dict and type(item.get('id')) is int and item['id'] > 0 for item in items)
            and len({item['id'] for item in items}) == len(items), 'API pages are incomplete or duplicated')
    return items


def selected_artifacts(context, value):
    """Select immutable original ZIP IDs only from this exact run/attempt/head."""
    prefix = f"linux-resource-harness-{context['run_id']}-{context['run_attempt']}-"
    rows = [item for item in api_items(value, 'artifacts') if str(item.get('name', '')).startswith(prefix)]
    expected = {prefix + shard: shard for shard in SHARDS}
    require(len(rows) == 4 and {row.get('name') for row in rows} == set(expected),
            'exactly four expected shard artifacts required')
    result = {}
    for row in rows:
        require(row.get('expired') is False and type(row.get('size_in_bytes')) is int and row['size_in_bytes'] > 0
                and type(row.get('digest')) is str and row['digest'].startswith('sha256:')
                and exact_hex(row['digest'][7:], 64), 'unexpired API digest-bound shard artifact required')
        owner = row.get('workflow_run', {})
        require(exact_id(owner.get('id'), context['run_id']) and owner.get('head_sha') == context['head_commit'],
                'shard artifact is from another run or head')
        result[expected[row['name']]] = row
    return result


def shard_jobs(context, value, run):
    """Require four actual successful attempt-specific jobs, not receipt claims."""
    require(exact_id(run.get('id'), context['run_id']) and exact_id(run.get('run_attempt'), context['run_attempt'])
            and run.get('head_sha') == context['head_commit'] and run.get('path') == '.github/workflows/pr.yml'
            and run.get('repository', {}).get('full_name') == context['repository']
            and ((run.get('status') == 'in_progress' and run.get('conclusion') is None)
                 or (run.get('status') == 'completed' and run.get('conclusion') == 'success')),
            'consumer API run differs from the current successful/in-progress attempt')
    prefix = 'Linux native resource harness shard ('
    rows = [row for row in api_items(value, 'jobs') if str(row.get('name', '')).startswith(prefix)]
    expected = {prefix + shard + ')': shard for shard in SHARDS}
    require(len(rows) == 4 and {row.get('name') for row in rows} == set(expected), 'exactly four named native jobs required')
    result = {}
    for row in rows:
        require(exact_id(row.get('run_id'), context['run_id']) and exact_id(row.get('run_attempt'), context['run_attempt'])
                and row.get('head_sha') == context['head_commit'] and row.get('status') == 'completed'
                and row.get('conclusion') == 'success', 'native shard job is not successful in this attempt')
        require(type(row.get('runner_id')) is int and row['runner_id'] > 0
                and type(row.get('runner_name')) is str and row['runner_name']
                and type(row.get('started_at')) is str and type(row.get('completed_at')) is str,
                'native shard host and terminal timing evidence required')
        result[expected[row['name']]] = row
    return result


def packet_members(packet):
    """Inspect bounded originals without extracting or executing a supplied module."""
    entries = packet.infolist()
    names = [entry.filename for entry in entries]
    require(len(names) == len(set(names)) and len(names) <= 128, 'ambiguous or excessive shard packet members')
    for entry in entries:
        path = PurePosixPath(entry.filename)
        require(not path.is_absolute() and '..' not in path.parts and '\\' not in entry.filename
                and not stat.S_ISLNK(entry.external_attr >> 16), 'unsafe shard packet member')
    return set(names)


def packet_bytes(packet, name, maximum):
    """Bound an original data member before reading it; ZIP is never extracted."""
    require(name in packet.namelist(), 'required shard evidence member missing: ' + name)
    info = packet.getinfo(name)
    require(not info.is_dir() and 0 < info.file_size <= maximum, 'shard member exceeds its bound')
    with packet.open(info) as stream:
        raw = stream.read(maximum + 1)
    require(len(raw) == info.file_size and len(raw) <= maximum, 'shard evidence has wrong expanded size')
    return raw


def packet_json(packet, name):
    """Decode bounded data only, rejecting duplicate fields."""
    return json.loads(packet_bytes(packet, name, 1024 * 1024), object_pairs_hook=unique_fields)


def check_packet(archive, artifact, context, config, shard, selected):
    """Bind a successful shard to API ZIP digest, original pins and exact raw log."""
    require(digest(archive) == artifact['digest'][7:] and archive.stat().st_size == artifact['size_in_bytes'],
            'original shard ZIP differs from GitHub API bytes')
    with zipfile.ZipFile(archive) as packet:
        packet_members(packet)
        result = packet_json(packet, 'native-observer-required/result.json')
        require(result.get('schema') == 'iroha.linux-resource-harness-shard.v1'
                and result.get('status') == 'pass' and result.get('qualification') == 'shard_only'
                and result.get('published') is False and result.get('shard') == shard
                and same_json(result.get('context'), context) and same_json(result.get('approved_pins'), config)
                and result.get('tests') == selected and result.get('harness_commit') == context['harness_commit']
                and result.get('runner_sha256') == context['runner_sha256'], 'shard result or input identity differs')
        for key, expected in [('tests_run', len(selected)), ('failures', 0), ('errors', 0), ('skipped', 0), ('returncode', 0)]:
            require(type(result.get(key)) is int and result[key] == expected, 'shard count or outcome is incomplete')
        log = packet_bytes(packet, 'native-observer-required/native-tests.log', 8 * 1024 * 1024)
        require(hashlib.sha256(log).hexdigest() == result.get('log_sha256'), 'native raw log differs from shard receipt')
        successful_suite(result['returncode'], log.decode('utf-8'), selected)
        require(same_json(packet_json(packet, 'native-observer-metadata/approval.json'), config)
                and hashlib.sha256(packet_bytes(packet, 'native-observer-metadata/approval.json', 1024 * 1024)).hexdigest()
                == context['approval_sha256'], 'original shard approval differs')
        require(packet_bytes(packet, 'native-observer-metadata/approval-commit.txt', 128).decode().strip()
                == context['approval_commit'], 'original approval checkout differs')
        metadata(config, packet_json(packet, 'native-observer-metadata/artifact.json'),
                 packet_json(packet, 'native-observer-metadata/run.json'))
        for name, key, maximum in [('candidate.json', 'candidate_sha256', 1024 * 1024),
                                   ('bridge/publisher-source.json', 'provenance_sha256', 512 * 1024),
                                   ('bridge/' + LIBRARY, 'module_sha256', 2 * 1024**3)]:
            member = 'native-observer-required/' + name
            require(member in packet.namelist(), 'original approved shard member missing')
            info = packet.getinfo(member)
            require(not info.is_dir() and 0 < info.file_size <= maximum, 'approved original exceeds bound')
            value, count = hashlib.sha256(), 0
            with packet.open(info) as stream:
                while block := stream.read(64 * 1024):
                    count += len(block)
                    require(count <= maximum, 'expanded original exceeds bound')
                    value.update(block)
            require(count == info.file_size and value.hexdigest() == config[key], 'original shard pin differs')
        require(packet_bytes(packet, 'native-observer-metadata/perf-policy-after.txt', 32).strip() == b'2',
                'original hosted perf readback required')
    return {'shard': shard, 'artifact_id': artifact['id'], 'artifact_sha256': artifact['digest'][7:],
            'tests': selected, 'log_sha256': result['log_sha256']}


def aggregate_packets(context, config, shards, artifacts_value, jobs_value, run_value, archive_root):
    """Data-only conjunction: all 20 exactly once across four verified originals."""
    artifacts = selected_artifacts(context, artifacts_value)
    jobs = shard_jobs(context, jobs_value, run_value)
    archives = [path for path in Path(archive_root).rglob('*') if path.is_file()]
    require(len(archives) == 4 and all(path.suffix == '.zip' and not path.is_symlink() for path in archives),
            'exactly four original downloaded ZIPs required')
    by_digest = {digest(path): path for path in archives}
    require(len(by_digest) == 4 and set(by_digest) == {row['digest'][7:] for row in artifacts.values()},
            'downloaded ZIPs do not exactly match four API digests')
    receipts, seen = [], []
    for shard in SHARDS:
        artifact, job = artifacts[shard], jobs[shard]
        receipt = check_packet(by_digest[artifact['digest'][7:]], artifact, context, config, shard, shards[shard])
        receipt.update(job_id=job['id'], runner_id=job['runner_id'], runner_name=job['runner_name'],
                       started_at=job['started_at'], completed_at=job['completed_at'])
        receipts.append(receipt)
        seen.extend(receipt['tests'])
    require(len(seen) == len(set(seen)) == 20, 'aggregate must contain exactly 20 unique successful methods')
    return {'schema': 'iroha.linux-resource-harness-aggregate.v1', 'status': 'pass',
            'qualification': 'all_20_native_methods_across_four_hosts', 'published': False,
            'context': context, 'tests': sorted(seen), 'tests_run': 20, 'failures': 0, 'errors': 0,
            'skipped': 0, 'shards': receipts, 'api_job_artifact_binding': 'reviewed_name_run_attempt_head_and_packet',
            'started_at': min(row['started_at'] for row in receipts),
            'completed_at': max(row['completed_at'] for row in receipts)}


def run(args, config):
    """Run the existing native runner after approval and byte/source checks succeed."""
    require(sys.platform.startswith('linux') and platform.machine() == 'x86_64', 'native Linux x86_64 runner required')
    harness, source, output = map(lambda path: Path(path).resolve(), (args.harness_root, args.source_root, args.output_root))
    for first, second in ((harness, source), (harness, output), (source, output)):
        require(not first.is_relative_to(second) and not second.is_relative_to(first), 'harness, clean observer and output roots must be separate')
    require(not output.exists(), 'preserve earlier attempt; output already exists')
    output.mkdir(mode=0o700)
    result = {'status': 'running', 'tests': [], 'approved_pins': config, 'published': False}
    try:
        metadata(config, read_json(args.artifact_metadata), read_json(args.run_metadata))
        git = lambda *options: subprocess.check_output(['git', '-C', str(source), *options]).strip()
        require(git('rev-parse', 'HEAD').decode() == config['source_commit'], 'observer checkout is not the approved source')
        require(not git('status', '--porcelain=v1', '--untracked-files=all'), 'observer source is dirty')
        selected = test_inventory(harness)
        if args.shard:
            context = read_json(args.context)
            closed = validate_context(context, config, harness, args.config)
            selected = closed[args.shard]
            result.update(schema='iroha.linux-resource-harness-shard.v1', qualification='shard_only',
                          shard=args.shard, context=context)
        archives = [path for path in Path(args.archive_root).rglob('*') if path.is_file()]
        require(len(archives) == 1 and archives[0].suffix == '.zip' and not archives[0].is_symlink(), 'exactly one downloaded ZIP required')
        bridge = unpack(config, archives[0], output)
        paths = git('ls-files', '-z').split(b'\0')
        needed = 2 * len(paths) + 1024
        soft, hard = resource.getrlimit(resource.RLIMIT_NOFILE)
        require(hard == resource.RLIM_INFINITY or hard >= needed, 'descriptor limit cannot retain the approved source')
        if soft < needed:
            resource.setrlimit(resource.RLIMIT_NOFILE, (needed, hard))
        runner = harness / 'scripts/tests/run_zk_resource_harness_native.py'
        command = [sys.executable, '-B', str(runner), '--source-root', str(source), '--bridge-root', str(bridge)]
        for name in ('module_sha256', 'provenance_sha256', 'source_manifest_sha256', 'native_target'):
            command += ['--' + name.replace('_', '-'), config[name]]
        for name in selected:
            command += ['--test', name]
        result.update(tests=selected, command=command, runner_sha256=digest(runner),
                      harness_commit=subprocess.check_output(['git', '-C', str(harness), 'rev-parse', 'HEAD'], text=True).strip())
        environment = {key: os.environ[key] for key in ('PATH', 'HOME') if key in os.environ}
        environment.update(LANG='C.UTF-8', PYTHONDONTWRITEBYTECODE='1')
        with (output / 'native-tests.log').open('xb') as log:
            completed = subprocess.run(command, cwd=harness, env=environment, stdout=log, stderr=subprocess.STDOUT)
        text = (output / 'native-tests.log').read_text()
        successful_suite(completed.returncode, text, selected)
        require(not git('status', '--porcelain=v1', '--untracked-files=all'), 'observer source changed during tests')
        if args.shard:
            validate_context(context, config, harness, args.config)
        result.update(status='pass', tests_run=len(selected), failures=0, errors=0, skipped=0,
                      returncode=completed.returncode, log_sha256=digest(output / 'native-tests.log'))
    except BaseException as error:
        result.update(status='fail', error=str(error))
        raise
    finally:
        with (output / 'result.json').open('x') as handle:
            json.dump(result, handle, indent=2)
            handle.write('\n')


def main():
    """Expose immutable inputs, full/sharded execution and a data-only aggregate."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('mode', choices=('approval', 'inputs', 'run', 'select', 'aggregate'))
    parser.add_argument('--config', required=True)
    parser.add_argument('--github-output')
    parser.add_argument('--shard', choices=SHARDS)
    for name in ('harness-root', 'source-root', 'output-root', 'archive-root', 'artifact-metadata', 'run-metadata',
                 'approval-root', 'approval-commit', 'harness-commit', 'head-commit', 'run-id', 'run-attempt',
                 'context', 'artifacts-metadata', 'jobs-metadata', 'consumer-run-metadata'):
        parser.add_argument('--' + name)
    args = parser.parse_args()
    config = approval(read_json(args.config))
    require(config['repository'] == os.environ.get('GITHUB_REPOSITORY'), 'approval must name the current repository')
    required = {'approval': ('github_output',),
                'inputs': ('github_output', 'harness_root', 'approval_root', 'approval_commit', 'harness_commit',
                           'head_commit', 'run_id', 'run_attempt'),
                'run': ('harness_root', 'source_root', 'output_root', 'archive_root', 'artifact_metadata', 'run_metadata'),
                'select': ('github_output', 'harness_root', 'context', 'artifacts_metadata'),
                'aggregate': ('harness_root', 'context', 'archive_root', 'artifacts_metadata', 'jobs_metadata',
                              'consumer_run_metadata', 'output_root')}
    require(all(getattr(args, name) for name in required[args.mode]), 'all independent roots and metadata paths required')
    require(not args.shard or (args.mode == 'run' and args.context), 'shard execution requires frozen input context')
    outputs = {}
    if args.mode in ('approval', 'inputs'):
        outputs = {name: config[name] for name in ('repository', 'producer_run_id', 'producer_run_attempt', 'artifact_id', 'source_commit')}
        if args.mode == 'inputs':
            context = make_context(args, config)
            outputs.update(inputs_json=json.dumps(context, separators=(',', ':')),
                           **{key: context[key] for key in ('approval_commit', 'harness_commit', 'head_commit')})
    elif args.mode == 'run':
        run(args, config)
    else:
        context = read_json(args.context)
        shards = validate_context(context, config, Path(args.harness_root).resolve(), args.config)
        artifacts = read_json(args.artifacts_metadata)
        if args.mode == 'select':
            chosen = selected_artifacts(context, artifacts)
            outputs['artifact_ids'] = ','.join(str(chosen[shard]['id']) for shard in SHARDS)
        else:
            output = Path(args.output_root).resolve()
            require(not output.exists(), 'preserve earlier aggregate attempt; output already exists')
            output.mkdir(mode=0o700)
            result = {'schema': 'iroha.linux-resource-harness-aggregate.v1', 'status': 'fail',
                      'context': context, 'published': False}
            try:
                result = aggregate_packets(context, config, shards, artifacts, read_json(args.jobs_metadata),
                                           read_json(args.consumer_run_metadata), args.archive_root)
            except BaseException as error:
                result['error'] = str(error)
                raise
            finally:
                with (output / 'result.json').open('x') as handle:
                    json.dump(result, handle, indent=2)
                    handle.write('\n')
    if outputs:
        with Path(args.github_output).open('a') as handle:
            for key, value in outputs.items():
                handle.write(f'{key}={value}\n')


if __name__ == '__main__':
    main()
