"""The real main validator reads controls through its retained experiment.

These component fixtures do not authenticate canonical proofs or qualify a run.
The old full-main semantic fixture migration remains a separate open test gate.
"""
from dataclasses import replace
import hashlib
import importlib.util
import json
from pathlib import Path
import sys

import pytest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts/nexus'))
import resource_bundle as bundle
import resource_evidence_budget as budget
import resource_experiment as experiment
from resource_experiment_test import published, valid

SPEC = importlib.util.spec_from_file_location('actual_scaling_main_reader', ROOT / 'scripts/nexus/validate_multilane_scaling_evidence.py')
main = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = main
SPEC.loader.exec_module(main)


def sha(raw): return hashlib.sha256(raw).hexdigest()


def configured(value, payload=b'{"number":2,"message":"retained"}'):
    old = next(item for item in value.controls if item.label == 'input')
    (value.root / old.path).unlink()
    controls = [item for item in value.controls if item is not old]
    statics = []
    for role in main._STATIC_ROLES:
        raw = payload if role == 'identity' else b'{}'
        path = value.root / 'controls' / role
        path.write_bytes(raw)
        path.chmod(0o600)
        statics.append(budget.StaticFile(role, len(raw)))
        controls.append(bundle.ControlBinding(role, str(path.relative_to(value.root)), sha(raw)))
    runs = []
    for run in value.budget.runs:
        supports = []
        for role in main._SUPPORT_ROLES:
            label = f'pair-{run.pair_index:02}.{run.variant}.{role}'
            supports.append(budget.FileBudget(label, budget.MIB))
            path = value.root / 'controls' / label
            path.write_bytes(b'{}')
            path.chmod(0o600)
            controls.append(bundle.ControlBinding(label, str(path.relative_to(value.root)), sha(b'{}')))
        runs.append(replace(run, support=tuple(supports)))
    value.budget = budget.admit_experiment(policy=value.budget.policy, runs=tuple(runs),
        static_files=tuple(statics), manifest=value.budget.control_budgets[0],
        report=value.budget.control_budgets[1], other_control=())
    value.controls = tuple(controls)
    return main.ResourceAdmission(value.budget, value.controls, value.runs, 'a' * 64, False)


def test_actual_main_reader_retains_same_owner_before_after_replay_and_final_scan(valid):
    admission = configured(valid)
    with valid.owner() as owner:
        controls = main.EvidenceControls(owner, admission)
        binding = controls.binding('identity')
        assert main.load_json(binding, 'identity', controls=controls, max_bytes=4096) == {'number': 2, 'message': 'retained'}
        results = owner.collect_replay()
        assert len(results) == 10
        assert main.load_json(binding, 'identity', controls=controls, max_bytes=4096)['number'] == 2
        assert owner.verify() is results


@pytest.mark.parametrize('payload', [b'{"n":1,"n":2}', b'{"n":NaN}', b'{"n":Infinity}',
    b'{"n":-Infinity}', b'{"n":1e999}', b'{"n":' + b'1' * 129 + b'}',
    b'{"n":1.' + b'1' * 129 + b'}', b'[' * 65 + b'0' + b']' * 65,
    b'\xff', b'{"n":1}\n{}', b'{"n":"unclosed}', b']', b''])
def test_real_main_decoder_rejects_bad_bounded_json_without_path_reopen(valid, payload):
    admission = configured(valid, payload)
    with valid.owner() as owner:
        controls = main.EvidenceControls(owner, admission)
        with pytest.raises(main.EvidenceError):
            main.load_json(controls.binding('identity'), 'identity', controls=controls, max_bytes=4096)


def test_main_decoder_numeric_json_and_string_braces_preserve_values(valid):
    payload = b'{"number":1.25,"negative":-2,"literal":"[}\\\"\\\\","depth":' + b'[' * 63 + b'1' + b']' * 63 + b'}'
    admission = configured(valid, payload)
    with valid.owner() as owner:
        controls = main.EvidenceControls(owner, admission)
        actual = main.load_json(controls.binding('identity'), 'identity', controls=controls, max_bytes=len(payload))
        assert actual == json.loads(payload)
        assert actual['number'] == 1.25 and actual['negative'] == -2


@pytest.mark.parametrize('cap', [True, False, -1, None, 1.0, (256 * budget.MIB) + 1])
def test_main_semantic_cap_is_explicit_bounded_integer(valid, cap):
    admission = configured(valid)
    with valid.owner() as owner:
        controls = main.EvidenceControls(owner, admission)
        with pytest.raises(main.EvidenceError):
            controls.read(controls.binding('identity'), max_bytes=cap)


def test_main_smaller_semantic_cap_rejects_before_return_and_poison_reader(valid):
    payload = b'{"n":1}'
    admission = configured(valid, payload)
    with valid.owner() as owner:
        controls = main.EvidenceControls(owner, admission)
        with pytest.raises(bundle.BundleError):
            controls.read(controls.binding('identity'), max_bytes=len(payload) - 1)
        with pytest.raises(experiment.ExperimentError, match='experiment_failed'):
            owner.verify()


@pytest.mark.parametrize('kind', ['path', 'digest', 'extra', 'missing', 'cross_run', 'cross_role'])
def test_main_reference_requires_exact_independent_role_path_and_hash(valid, kind):
    admission = configured(valid)
    with valid.owner() as owner:
        controls = main.EvidenceControls(owner, admission)
        binding = controls.binding(valid.budget.runs[0].transaction_trace.label)
        ref = {'path': binding.path, 'sha256': binding.sha256}
        if kind == 'path': ref['path'] = 'controls/absent'
        elif kind == 'digest': ref['sha256'] = 'c' * 64
        elif kind == 'extra': ref['unused'] = 'data'
        elif kind == 'missing': del ref['sha256']
        else:
            other = controls.binding(valid.budget.runs[1].transaction_trace.label if kind == 'cross_run'
                                     else valid.budget.runs[0].canonical_proof.label)
            # Equal content digests cannot erase an independently allocated role.
            assert other.sha256 == binding.sha256
            ref = {'path': other.path, 'sha256': other.sha256}
        seen = set()
        with pytest.raises(main.EvidenceError):
            main._require_ref(ref, controls, 'trace', expected_label=binding.label, referenced_paths=seen)
        assert seen == set()


def test_main_reference_returns_original_object_and_records_exact_control_path(valid):
    admission = configured(valid)
    with valid.owner() as owner:
        controls = main.EvidenceControls(owner, admission)
        binding = controls.binding('identity')
        seen = set()
        assert main._require_ref({'path': binding.path, 'sha256': binding.sha256}, controls,
            'identity', expected_label='identity', referenced_paths=seen) is binding
        assert seen == {binding.path}
        with pytest.raises(main.EvidenceError):
            controls.read(replace(binding), max_bytes=4096)


def test_main_semantic_read_rejects_actual_post_admission_file_replacement(valid):
    admission = configured(valid)
    with valid.owner() as owner:
        controls = main.EvidenceControls(owner, admission)
        binding = controls.binding('identity')
        path = valid.root / binding.path
        path.unlink()
        path.symlink_to(valid.root / controls.binding('configuration').path)
        with pytest.raises(bundle.BundleError):
            main.load_json(binding, 'identity', controls=controls, max_bytes=4096)
        with pytest.raises(experiment.ExperimentError, match='experiment_failed'):
            owner.verify()


def test_unfinished_public_entrypoint_cannot_accept_old_unadmitted_bundle_or_write_report(tmp_path, capsys):
    manifest, report = tmp_path / 'scaling_evidence.json', tmp_path / 'report.json'
    manifest.write_text('{}')
    assert main.main([str(manifest), '--report', str(report)]) == 2
    assert not report.exists()
    assert 'mandatory launcher and canonical proof integration is incomplete' in capsys.readouterr().err
    with pytest.raises(TypeError):
        main.validate_evidence(manifest)
    with pytest.raises(main.EvidenceError, match='independent resource admission is mandatory'):
        main.validate_evidence(manifest, admission=None)
