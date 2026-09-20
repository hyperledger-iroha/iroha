"""Check NEVO fixture maintenance against base drift and unauthorized input drift."""
import hashlib
import importlib.util
import json
from pathlib import Path
import shutil
import subprocess
import sys

import pytest

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location('refresh_taira_nevo', ROOT / 'scripts/refresh_taira_nevo_fixtures.py')
REFRESH = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(REFRESH)


@pytest.fixture
def source(tmp_path):
    for relative in [
        REFRESH.TAIRA / 'genesis.template.json', REFRESH.TAIRA / 'config.toml',
        REFRESH.TAIRA / 'nevo_genesis_overlay.template.json',
        REFRESH.FIXTURES / 'review.json', REFRESH.FIXTURES / 'public-inputs.json',
        REFRESH.FIXTURES / 'unsigned-genesis.template.json',
        Path('scripts/refresh_taira_nevo_fixtures.py'),
    ]:
        target = tmp_path / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT / relative, target)
    return tmp_path


def test_current_fixtures_are_exact(source):
    expected = REFRESH.expected_fixtures(source)
    assert all((source / path).read_bytes() == data for path, data in expected.items())


def test_base_changes_preserve_overlay_and_bind_actual_bytes(source):
    path = source / REFRESH.TAIRA / 'genesis.template.json'
    base = json.loads(path.read_bytes())
    base['transactions'][0]['instructions'].append({'test_base_change': 17})
    raw = json.dumps(base, ensure_ascii=False, separators=(',', ':')).encode() + b'\n\n'
    path.write_bytes(raw)
    expected = REFRESH.expected_fixtures(source)
    unsigned = expected[REFRESH.FIXTURES / 'unsigned-genesis.template.json']
    generated = json.loads(unsigned)
    overlay = json.loads((source / REFRESH.TAIRA / 'nevo_genesis_overlay.template.json').read_bytes())
    assert generated['transactions'].pop() == overlay
    assert generated == base
    review = json.loads(expected[REFRESH.FIXTURES / 'review.json'])
    original = json.loads((source / REFRESH.FIXTURES / 'review.json').read_bytes())
    assert review['base_genesis_sha256'] == hashlib.sha256(raw).hexdigest()
    assert review['unsigned_genesis_sha256'] == hashlib.sha256(unsigned).hexdigest()
    for field in ('base_genesis_sha256', 'base_config_sha256', 'unsigned_genesis_sha256'):
        review.pop(field)
        original.pop(field)
    assert review == original


def test_unreviewed_public_input_changes_fail(source):
    path = source / REFRESH.FIXTURES / 'public-inputs.json'
    data = json.loads(path.read_bytes())
    data['api_signer_account_id'] = 'unreviewed'
    path.write_text(json.dumps(data))
    with pytest.raises(ValueError, match='explicit review update'):
        REFRESH.expected_fixtures(source)


@pytest.mark.parametrize('change', [
    {'parameters': {}}, {'topology': ['extra']}, {'ivm_triggers': ['extra']}, {'instructions': []},
])
def test_overlay_cannot_add_other_genesis_effects(source, change):
    path = source / REFRESH.TAIRA / 'nevo_genesis_overlay.template.json'
    data = json.loads(path.read_bytes())
    data.update(change)
    path.write_text(json.dumps(data))
    with pytest.raises(ValueError, match='instruction-only'):
        REFRESH.expected_fixtures(source)


def test_check_refuses_stale_fixture_without_writing_and_write_is_idempotent(source):
    path = source / REFRESH.FIXTURES / 'unsigned-genesis.template.json'
    path.write_bytes(b'{}\n')
    command = [sys.executable, str(source / 'scripts/refresh_taira_nevo_fixtures.py')]
    check = subprocess.run(command, capture_output=True, text=True)
    assert check.returncode == 1 and 'stale:' in check.stdout
    assert path.read_bytes() == b'{}\n'
    write = subprocess.run([*command, '--write'], capture_output=True, text=True)
    assert write.returncode == 0 and 'refreshed:' in write.stdout
    assert subprocess.run(command, capture_output=True).returncode == 0
    repeat = subprocess.run([*command, '--write'], capture_output=True, text=True)
    assert repeat.returncode == 0 and repeat.stdout == ''
