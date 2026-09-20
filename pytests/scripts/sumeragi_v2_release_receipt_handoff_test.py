"""Execute the production receipt's complete handoff validation block.

Only the surrounding release fixture is supplied. The actual receipt schema,
digest helpers, every handoff predicate and environment update execute without
importing receipt runtime modules during test collection or opening descriptors.
"""
from __future__ import annotations

import ast
import os
from pathlib import Path
import re
import subprocess

import pytest


ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / 'scripts/write_sumeragi_v2_release_receipt.py'
KEYS = (
    'IROHA_RELEASE_INVOCATION_ROOT', 'IROHA_RELEASE_TEMP_BASE',
    'IROHA_RELEASE_SCALING_GATE_FD', 'IROHA_RELEASE_SCALING_INVOCATION_SHA256',
    'IROHA_RELEASE_SCALING_CHALLENGE', 'IROHA_RELEASE_SCALING_HANDOFF_HELPER_SHA256',
)


@pytest.fixture(autouse=True)
def no_process_or_signal(monkeypatch):
    def denied(*args, **kwargs):
        raise AssertionError('process and signal operations are forbidden')
    monkeypatch.setattr(subprocess, 'Popen', denied)
    for name in ('system', 'fork', 'posix_spawn', 'posix_spawnp', 'kill', 'killpg'):
        if hasattr(os, name):
            monkeypatch.setattr(os, name, denied)


@pytest.fixture(scope='module')
def production():
    tree = ast.parse(SOURCE.read_text())
    selected = []
    wanted = {'ReceiptError', '_require_exact_json_fields', '_require_digest'}
    for node in tree.body:
        if isinstance(node, (ast.FunctionDef, ast.ClassDef)) and node.name in wanted:
            selected.append(node)
        if isinstance(node, ast.Assign) and any(isinstance(target, ast.Name)
                and target.id == '_DIGEST_RE' for target in node.targets):
            selected.append(node)
    assert len(selected) == 4
    function = next(node for node in tree.body if isinstance(node, ast.FunctionDef)
                    and node.name == '_validate_bootstrap_evidence')
    start = next(index for index, node in enumerate(function.body)
                 if isinstance(node, ast.Assign) and any(isinstance(target, ast.Name)
                    and target.id == 'handoff' for target in node.targets))
    end = next(index for index, node in enumerate(function.body[start:], start)
               if isinstance(node, ast.Expr) and isinstance(node.value, ast.Call)
               and ast.unparse(node.value.func) == 'alias_environment.update')
    block = function.body[start:end + 1]
    assert len(block) == 7
    future = ast.ImportFrom(module='__future__', names=[ast.alias(name='annotations')], level=0)
    scope = dict(Path=Path, os=os, re=re)
    exec(compile(ast.fix_missing_locations(ast.Module(body=[future, *selected], type_ignores=[])),
                 str(SOURCE), 'exec'), scope)
    compiled = compile(ast.fix_missing_locations(ast.Module(body=block, type_ignores=[])),
                       str(SOURCE), 'exec')
    return scope, compiled


def selected_values(*, descriptor='3', base='/private/tmp', name='iroha-release-abc'):
    return dict(zip(KEYS, (base + '/' + name, base, descriptor, 'a' * 64,
                          'b' * 64, 'c' * 64), strict=True))


def evaluate(production, values, *, release_root=None, helper_digest='c' * 64):
    scope, compiled = production
    state = dict(scope, runner={'scaling_handoff': values},
                 release_root=release_root or Path(values.get(KEYS[0], '/invalid')) / 'source',
                 trusted_digests={'scaling_handoff_helper': helper_digest},
                 alias_environment={'existing_parent_value': 'preserved'})
    exec(compiled, state)
    return state


@pytest.mark.parametrize('descriptor', ['3', '9', '10', '999999', '1048575'])
@pytest.mark.parametrize('base,name', [
    ('/private/tmp', 'iroha-release-abc'),
    ('/private/tmp+build', 'iroha+release_9-z'),
])
def test_producer_domain_is_accepted(production, descriptor, base, name):
    values = selected_values(descriptor=descriptor, base=base, name=name)
    state = evaluate(production, values)
    assert state['alias_environment'] == {'existing_parent_value': 'preserved', **values}
    assert state['handoff'] is values


@pytest.mark.parametrize('descriptor', [
    '0', '1', '2', '1048576', '1048577', '2147483647', '999999999999999999',
    '-3', '+3', '03', ' 3', '3 ', '3\n', '3.0', '3e0', '３', '٣', '', 3, True, None,
])
def test_descriptor_lexical_and_numeric_boundaries(production, descriptor):
    with pytest.raises(production[0]['ReceiptError']):
        evaluate(production, selected_values(descriptor=descriptor))


@pytest.mark.parametrize('mutation', [
    'unknown_field', 'missing_field', 'relative_root', 'parent_escape',
    'wrong_parent', 'wrong_source', 'space', 'newline', 'nul', 'unicode',
    'shell_metacharacter', 'wrong_helper', 'wrong_invocation_digest', 'wrong_challenge',
])
def test_all_existing_handoff_assertions_remain_active(production, mutation):
    values = selected_values(); options = {}
    if mutation == 'unknown_field':
        values['accepted'] = 'true'
    elif mutation == 'missing_field':
        del values[KEYS[4]]
    elif mutation == 'relative_root':
        values[KEYS[0]] = 'relative/root'; values[KEYS[1]] = 'relative'
    elif mutation == 'parent_escape':
        values[KEYS[0]] = '/private/tmp/../iroha-release'; values[KEYS[1]] = '/private/tmp/..'
    elif mutation == 'wrong_parent':
        values[KEYS[1]] = '/private/other'
    elif mutation == 'wrong_source':
        options['release_root'] = Path('/private/tmp/foreign/source')
    elif mutation in ('space', 'newline', 'nul', 'unicode', 'shell_metacharacter'):
        bad = {'space': ' ', 'newline': '\n', 'nul': '\0', 'unicode': 'あ', 'shell_metacharacter': ';'}[mutation]
        values[KEYS[0]] += bad
    elif mutation == 'wrong_helper':
        values[KEYS[5]] = 'd' * 64
    elif mutation == 'wrong_invocation_digest':
        values[KEYS[3]] = 'A' * 64
    else:
        values[KEYS[4]] = 'bad'
    with pytest.raises(production[0]['ReceiptError']):
        evaluate(production, values, **options)


@pytest.mark.parametrize('key', KEYS)
def test_all_six_values_require_exact_strings(production, key):
    values = selected_values(); values[key] = False
    with pytest.raises(production[0]['ReceiptError']):
        evaluate(production, values, release_root=Path('/private/tmp/iroha-release-abc/source'))
