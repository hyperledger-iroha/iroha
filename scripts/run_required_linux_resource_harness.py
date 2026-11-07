#!/usr/bin/env python3
"""Consume independently approved Linux observer artifacts and run all native tests.

Requires Python 3.12, Git, Linux x86_64 for execution, separate clean harness and
observer checkouts, and an approval record from an independently selected immutable
commit. GitHub downloads the exact approved artifact; no build or hash promotion
occurs here. Output must be a new private directory outside both source checkouts.
No signing credentials, publication, fallback artifacts or partial test selection.
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
    require(artifact.get('id') == config['artifact_id'] and artifact.get('name') == config['artifact_name']
            and artifact.get('expired') is False and artifact.get('digest') == 'sha256:' + config['artifact_sha256'],
            'artifact metadata differs from approval or has expired')
    require(artifact.get('workflow_run', {}).get('id') == config['producer_run_id'], 'artifact belongs to another run')
    require(run.get('id') == config['producer_run_id'] and run.get('run_attempt') == config['producer_run_attempt']
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
    """Require the runner's full successful count and each explicit method result."""
    require(returncode == 0, 'actual native suite failed; see retained log')
    require(re.search(r'(?m)^Ran 20 tests in ', text) and text.rstrip().endswith('\nOK'),
            'all 20 native tests must finish with zero skips')
    require(all(re.search(r'(?m)^' + re.escape(name) + r' \([^\n]+\) \.\.\. ok$', text)
                for name in selected), 'a named native method lacks its successful result')


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
        result.update(status='pass', tests_run=20, failures=0, errors=0, skipped=0)
    except BaseException as error:
        result.update(status='fail', error=str(error))
        raise
    finally:
        with (output / 'result.json').open('x') as handle:
            json.dump(result, handle, indent=2)
            handle.write('\n')


def main():
    """Expose approval outputs and the required consumer without an auto-approval mode."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('mode', choices=('approval', 'run'))
    parser.add_argument('--config', required=True)
    parser.add_argument('--github-output')
    for name in ('harness-root', 'source-root', 'output-root', 'archive-root', 'artifact-metadata', 'run-metadata'):
        parser.add_argument('--' + name)
    args = parser.parse_args()
    config = approval(read_json(args.config))
    require(config['repository'] == os.environ.get('GITHUB_REPOSITORY'), 'approval must name the current repository')
    if args.mode == 'approval':
        require(args.github_output, 'explicit GitHub output required')
        with Path(args.github_output).open('a') as handle:
            for name in ('repository', 'producer_run_id', 'producer_run_attempt', 'artifact_id', 'source_commit'):
                handle.write(f'{name}={config[name]}\n')
    else:
        require(all(getattr(args, name) for name in ('harness_root', 'source_root', 'output_root', 'archive_root', 'artifact_metadata', 'run_metadata')),
                'all independent roots and metadata paths required')
        run(args, config)


if __name__ == '__main__':
    main()
