"""Rehashed numeric schema controls through the complete resource component.

Every mutation follows the exact same-input positive through actual publisher,
scanner, replay and reconciliation. Fixture-owned pins are refreshed explicitly;
no production consumer learns expected hashes or trust from the malformed data.
"""
from pathlib import Path
import json
import re
import sys

import pytest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts/nexus'))
sys.path.insert(0, str(ROOT / 'scripts/tests'))
from validate_multilane_scaling_evidence_test import EvidenceBundle, VALIDATOR
import scaling_main_component_fixture as COMPONENT


@pytest.fixture(scope='module')
def published(tmp_path_factory):
    return EvidenceBundle(tmp_path_factory.mktemp('numeric-schema-publisher'))


def valid_clone(published, tmp_path):
    fixture = COMPONENT.clone_fixture(published, tmp_path)
    metrics = COMPONENT.validate_component(fixture.manifest_path)
    assert metrics['pair_count'] == 5
    assert metrics['run_count'] == 10
    assert metrics['four_to_one_median_throughput_ratio'] == 1.6
    return fixture


def rejects_exact_integer(fixture, field):
    with pytest.raises(VALIDATOR.EvidenceError,
                       match=re.escape(field) + r' must be an integer >= 1'):
        COMPONENT.validate_component(fixture.manifest_path)


@pytest.mark.parametrize('target', [
    'manifest_pair', 'manifest_lanes', 'manifest_sequence', 'raw_lanes',
])
@pytest.mark.parametrize('value', [True, 1.0])
def test_rehashed_role_integer_rejects_equal_bool_or_float(published, tmp_path, target, value):
    fixture = valid_clone(published, tmp_path)
    if target.startswith('manifest_'):
        field = {'manifest_pair': 'pair_index', 'manifest_lanes': 'active_execution_lanes',
                 'manifest_sequence': 'sequence'}[target]
        assert type(fixture.manifest['runs'][0][field]) is int
        assert fixture.manifest['runs'][0][field] == value
        fixture.manifest['runs'][0][field] = value
        fixture.flush_manifest()
    else:
        field = 'active_execution_lanes'
        raw = fixture.load_raw(1, 'one_lane')
        assert type(raw[field]) is int
        assert raw[field] == value
        raw[field] = value
        fixture.replace_raw(1, 'one_lane', raw)
    rejects_exact_integer(fixture, field)


def test_rehashed_pair_count_rejects_equal_float(published, tmp_path):
    fixture = valid_clone(published, tmp_path)
    assert type(fixture.manifest['pair_count']) is int
    assert fixture.manifest['pair_count'] == 5.0
    fixture.manifest['pair_count'] = 5.0
    fixture.flush_manifest()
    rejects_exact_integer(fixture, 'evidence manifest.pair_count')


@pytest.mark.parametrize('value', [True, 1.0])
def test_rehashed_support_version_rejects_equal_bool_or_float(published, tmp_path, value):
    fixture = valid_clone(published, tmp_path)
    raw = fixture.load_raw(1, 'one_lane')
    role = 'nexus_load_test_manifest'
    path = fixture.root / raw['artifacts'][role]['path']
    support = json.loads(path.read_bytes())
    assert type(support['version']) is int
    assert support['version'] == value
    support['version'] = value
    fixture.write_json(path, support)
    raw['artifacts'][role] = fixture.ref(path)
    fixture.replace_raw(1, 'one_lane', raw)
    rejects_exact_integer(fixture, 'Nexus lane-load manifest.version')


@pytest.mark.parametrize('phase', ['identity_before', 'identity_after'])
@pytest.mark.parametrize('field', ['physical_core_count', 'logical_core_count', 'memory_bytes'])
def test_rehashed_raw_identity_snapshot_rejects_equal_float(published, tmp_path, phase, field):
    fixture = valid_clone(published, tmp_path)
    raw = fixture.load_raw(1, 'one_lane')
    original = raw[phase]['hardware'][field]
    assert type(original) is int
    assert original == float(original)
    assert original == fixture.identity['hardware'][field]
    raw[phase]['hardware'][field] = float(original)
    fixture.replace_raw(1, 'one_lane', raw)
    rejects_exact_integer(fixture, phase + '.hardware.' + field)
