"""Synthetic historical preflight corpus with actual source-bound archive files.

These fixtures fabricate terminal/result data and never execute or qualify a
preflight. Their purpose is independent copied-archive parsing and tamper tests.
"""
from pathlib import Path
from types import SimpleNamespace
import hashlib
import json
import os
import shutil

import scaling_preflight_archive as archive


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def write_member(root, name, raw):
    path = root / name
    if path.exists():
        path.chmod(0o600)
    path.write_bytes(raw)
    path.chmod(0o400)
    return dict(sha256=sha(raw), size_bytes=len(raw))


def census(root):
    return tuple(dict(relative_path=path.name, size_bytes=path.stat().st_size,
                      sha256=sha(path.read_bytes())) for path in sorted(root.iterdir()))


def rebind(root, binding, index=None):
    """Rehash deliberately changed fixture data without claiming original custody."""
    if index is not None:
        binding['index'] = dict(write_member(root, 'index.json', archive.canonical(index)), mode='0400')
    binding['inventory'] = archive.archive_census(census(root))
    return binding


def copy_selected_source(source_root, destination):
    """Copy only the explicit inventory closure for relocation/mutation fixtures."""
    inventory = archive.decode_inventory((source_root / archive.INVENTORY_PATH).read_bytes())
    destination.mkdir(mode=0o700)
    for name in (*inventory['sources'], archive.INVENTORY_PATH):
        target = destination / name
        target.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
        shutil.copyfile(source_root / name, target)
        target.chmod(0o600)
    return destination


def build_archive(source_root, archive_root, candidate_identity,
                  invocation_sha256='e' * 64, started_ns=100,
                  completed_ns=1000, timeout_seconds=600):
    """Build every required unit as explicit synthetic data, with no process seam."""
    raw = (source_root / archive.INVENTORY_PATH).read_bytes()
    inventory = archive.decode_inventory(raw)
    assert not inventory['pending_outcomes']
    for name, digest in inventory['sources'].items():
        assert sha((source_root / name).read_bytes()) == digest
    source_buffers = {'scripts/nexus/scaling_cli_bootstrap.py':
                      (source_root / 'scripts/nexus/scaling_cli_bootstrap.py').read_bytes()}
    inputs, copies = archive.phase_inputs(inventory, sha(raw), source_buffers)
    archive_root.mkdir(mode=0o700)
    selection = write_member(archive_root, 'inventory.json', raw)
    deadline = started_ns + timeout_seconds * 1_000_000_000
    context = dict(python='/historical/framework/bin/python3.12',
        repository_root='/historical/invocation/source', work_root='/historical/invocation/runtime/preflight',
        environment_sha256='a' * 64)
    for name, prefix in (('pytest', 'dependencies'), ('blake3', 'blake3')):
        work = Path(context['work_root'])
        context[name] = dict(source_root=str(work / (prefix + '-source')),
            bundle_root=str(work / (prefix + '-bundle')),
            inventory=str(work / (prefix + '-inventory.json')), inventory_sha256=('b' if name == 'pytest' else 'c') * 64)
    units = []
    for ordinal, (kind, name, expected) in enumerate(archive.ordered_units(inventory)):
        result = dict(passed=True, tests_run=len(expected), failures=0, errors=0, skipped=0,
            isolated=True, no_site=True, no_bytecode=True, inputs_unchanged=True, subtest_observations=0)
        if kind == 'phase':
            result.update(phase=name, node_ids=list(expected), expected_node_ids=list(expected),
                source_registry_count=58, external_native_processes=False,
                qualification='synthetic historical archive data; no execution', elapsed_ns=1,
                inputs_before=inputs, inputs_after=inputs, copied_sources_after=copies, forbidden_process_attempts=[])
            if name == 'bootstrap':
                result.update(actual_private_blake3_import=True, actual_bootstrap_composition=True)
            result_raw = (json.dumps(result, sort_keys=True, indent=2) + '\n').encode()
        else:
            result.update(suite=name, node_sha256s=list(expected), inventory_sha256=sha(raw),
                inputs_before=inventory['sources'], inputs_after=inventory['sources'],
                collected_node_sha256s=list(expected), outcome_node_sha256s=list(expected))
            result_raw = archive.canonical(result)
        stdout, stderr = ('synthetic unit %03d\n' % ordinal).encode(), b''
        terminal = SimpleNamespace(pid=1000 + ordinal, argv=archive.unit_argv(ordinal, kind, name, context),
            cwd=context['repository_root'], environment_sha256=context['environment_sha256'],
            descriptors=(), violations=(), started_ns=started_ns + ordinal * 3 + 1,
            completed_ns=started_ns + ordinal * 3 + 2, deadline_ns=deadline, returncode=0,
            stdout_bytes=len(stdout), stderr_bytes=0, stdout_sha256=sha(stdout), stderr_sha256=sha(stderr))
        unit = dict(index=ordinal, kind=kind, name=name)
        for role, content in (('command', archive.project_unit_command(ordinal, name, terminal)),
                              ('result', result_raw), ('stdout', stdout), ('stderr', stderr)):
            unit[role] = write_member(archive_root, archive._unit_name(ordinal, role), content)
        units.append(unit)
    assert started_ns + len(units) * 3 < completed_ns
    scope = dict(timeout_seconds=timeout_seconds, original_started_ns=started_ns,
                 deadline_ns=deadline, verification_completed_ns=completed_ns - 1)
    index = archive.encode_index(candidate_identity, invocation_sha256, scope, selection, units,
                                 command_context=context)
    binding = dict(archive_id=archive.ARCHIVE_ID,
        scope=dict(timeout_seconds=timeout_seconds, original_started_ns=started_ns,
                   deadline_ns=deadline, completed_ns=completed_ns),
        index=dict(write_member(archive_root, 'index.json', index), mode='0400'),
        inventory=archive.archive_census(census(archive_root)))
    return archive.validate_binding(binding)
