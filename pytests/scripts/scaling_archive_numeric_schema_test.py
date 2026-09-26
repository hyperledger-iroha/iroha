"""Current V1 archive numeric types after all ordinary byte hashes are refreshed."""

from dataclasses import replace
import json
import shutil

import pytest

import scaling_archive_data as archive
from scaling_archive_data_test import complete, put, refresh_public_hashes
from scaling_experiment_custody_test import fixed_plan, budget_for
from scaling_experiment_plan import ExperimentPlanError, admit_plan, encode, plan_bytes


@pytest.mark.parametrize('index,substitution', ((0, True), (0, 1.0), (1, 4.0)))
def test_fixed_plan_lane_cardinality_requires_an_exact_integer(index, substitution):
    """V1 has one typed lane count in the originating ten-trial plan."""
    original = fixed_plan()
    budget = budget_for(original)
    admit_plan(original, budget)
    trials = list(original.trials)
    generator = trials[index].generator
    assert generator.lane_count == substitution
    trials[index] = replace(trials[index], generator=replace(generator, lane_count=substitution))
    with pytest.raises(ExperimentPlanError):
        admit_plan(replace(original, trials=tuple(trials)), budget)


@pytest.mark.parametrize('role', ('manifest', 'raw_run', 'run_receipt', 'report'))
@pytest.mark.parametrize('substitution', (True, 1.0))
def test_equal_boolean_or_float_cannot_replace_original_run_index(
    complete, tmp_path, role, substitution,
):
    """Rehashing a changed archive cannot turn numeric equality into V1 type equality."""
    source, plan, budget, _, _ = complete
    root = tmp_path / 'evidence'
    shutil.copytree(source, root)
    relative = {
        'manifest': 'manifest.json',
        'raw_run': 'runs/pair-01/one_lane/raw_samples.json',
        'run_receipt': 'runs/pair-01/one_lane/run_receipt.json',
        'report': 'report.json',
    }[role]
    path = root / relative
    value = json.loads(path.read_bytes())
    row = value['runs'][0] if role in ('manifest', 'report') else value
    assert type(row['pair_index']) is int and row['pair_index'] == substitution
    row['pair_index'] = substitution
    put(path, encode(value))
    expected = refresh_public_hashes(root)
    with pytest.raises(archive.ArchiveDataError):
        archive.inspect_archive(root, plan, budget, expected)


@pytest.mark.parametrize('field', ('physical_cores', 'logical_cores', 'memory_bytes'))
def test_equal_float_cannot_replace_original_hardware_integer_after_rehash(
    complete, tmp_path, field,
):
    """The single V1 identity control rejects floats despite an exact byte cap."""
    source, plan, budget, _, _ = complete
    root = tmp_path / 'evidence'
    shutil.copytree(source, root)
    path = root / 'inputs/identity.json'
    original = path.read_bytes()
    identity = json.loads(original)
    hardware = identity['hardware']
    current = hardware[field]
    assert type(current) is int and current == float(current)
    hardware[field] = float(current)
    # Keep the original static-file byte reservation exact; only a valid,
    # unconstrained label shrinks by the two added float characters.
    hardware['machine_id'] = hardware['machine_id'][:-2]
    changed = encode(identity)
    assert len(changed) == len(original)
    source_control = json.loads((root / 'inputs/source_closure.json').read_bytes())
    with pytest.raises(archive.ArchiveDataError):
        archive._controls(identity, source_control, archive._sha(plan_bytes(plan)))
    put(path, changed)
    expected = refresh_public_hashes(root)
    with pytest.raises(archive.ArchiveDataError):
        archive.inspect_archive(root, plan, budget, expected)
