"""Retained wrapper/record composition without hiding opaque or unwind costs."""
import copy
import importlib.util
import json
from pathlib import Path
from types import SimpleNamespace

import pytest


ROOT = Path(__file__).resolve().parents[2]
PATH = ROOT / 'scripts/parliament_ballot_retained_frontend_bound.py'
SPEC = importlib.util.spec_from_file_location('retained_frontend', PATH)
M = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(M)
CACHE = Path('/Users/takemiyamakoto/Library/Caches/iroha-script-tests/runs/parliament-current-20261104')


@pytest.fixture(scope='module')
def captured():
    guards = {}
    leaf = M.load_leaf(ROOT, guards)
    modes, triage = leaf.load_inputs(ROOT, CACHE, guards)
    reviewed = leaf.object_json(M.pinned(CACHE / M.LEAF_REPORT, M.LEAF_REPORT_SHA, guards))
    assert leaf.recheck(guards) == guards
    return leaf, modes, reviewed, triage


def paths_from(result):
    rows = {row['address']: row for row in result['full_interval_inventory'] if row['ordinary_entry_reachable']}
    pending, paths = [(result['interval_start'], [])], []
    while pending:
        address, prefix = pending.pop()
        assert address not in prefix
        path = [*prefix, address]
        if rows[address]['successors']:
            pending.extend((target, path) for target in rows[address]['successors'])
        else:
            assert rows[address]['opcode'] == 'ret'
            paths.append(path)
    return paths


@pytest.mark.parametrize('mode', ['portable', 'accel'])
def test_exact_retained_projection_matches_independent_path_enumeration(mode, captured):
    leaf, modes, reviewed, _ = captured
    raw, original = modes[mode]
    result = M.compose(leaf, raw, original, reviewed['modes'][mode])
    assert result['decoded_words'] == 209 and result['ordinary_entry_reachable_words'] == 192
    paths = paths_from(result)
    assert len(paths) == 13
    calls = {row['address']: row for row in result['ordinary_calls']}
    assert len(calls) == 15
    assert max(map(len, paths)) == result['local_ordinary_instruction_events']['maximum'] == 105
    weighted = max(len(path) + 34 * sum(calls.get(a, {}).get('label') == 'record' for a in path) for path in paths)
    assert weighted == result['wrapper_and_record_ordinary_instruction_events']['maximum'] == 139
    assert result['local_ordinary_instruction_events']['cfg_path'] in paths
    assert result['wrapper_and_record_ordinary_instruction_events']['cfg_path'] in paths
    coefficients = result['direct_call_coefficients']
    expected = {'profile': 1, 'record': 1, 'typed_vote': 1, 'typed_registration': 1, 'role_vote': 1,
                'role_registration': 1, 'memcpy': 2, 'dealloc': 1, 'context_drop': 1,
                'all_direct_calls': 8, 'typed_decoder_calls': 1, 'role_verifier_calls': 1, 'direct_cleanup_calls': 2}
    assert {key: value['maximum'] for key, value in coefficients.items()} == expected
    for label in expected.keys() - {'all_direct_calls', 'typed_decoder_calls', 'role_verifier_calls', 'direct_cleanup_calls'}:
        assert coefficients[label]['maximum'] == max(sum(calls.get(a, {}).get('label') == label for a in p) for p in paths)
    assert result['maximum_explicit_local_sp_decrement_bytes'] == 2512
    assert result['inherited_v5_modeled_local_frame_plus_red_zone_bytes'] == 2640
    assert all(row['caller_depth_bytes'] == 2512 for row in calls.values())
    requests = result['native_memcpy_request_bytes']
    assert requests['sites'] == {'0x100000bdc': 697, '0x100000cb0': 424, '0x100000d58': 368}
    assert max(sum(requests['sites'].get(a, 0) for a in path) for path in paths) == requests['path_bound']['maximum'] == 1121
    assert requests['native_work_or_memory_footprint_bound'] is None
    assert result['inherited_outer_roots_unchanged'] == original['roots']


def test_all_suffix_words_and_calls_are_active_unknown_obligations(captured):
    leaf, modes, reviewed, _ = captured
    result = M.compose(leaf, *modes['portable'], reviewed['modes']['portable'])
    outside = result['nonentry_obligations']
    assert [row['address'] for row in outside['words']] == [hex(a) for a in range(0x100000e40, 0x100000e84, 4)]
    assert len(outside['calls']) == 5
    assert sum(row['symbol'] == '__Unwind_Resume' for row in outside['calls']) == 3
    assert all(row['entry_relative_sp_before'] is None and row['entry_relative_sp_after'] is None for row in outside['words'])
    assert all(row['caller_depth_bytes'] is None and row['entry_condition'] is None for row in outside['calls'])
    assert outside['out_of_interval_successors'] == [{'source': '0x100000e80', 'target': '0x100000e84'}]
    assert 'active unresolved' in outside['status']
    assert outside['entry_sp_bytes'] is outside['work_bound'] is outside['stack_bound_bytes'] is None
    for key in ('complete_work_bound', 'complete_physical_stack_bound_bytes', 'pointer_memory_bound',
                'elapsed_time_bound', 'exceptional_path_bound', 'caller_bound'):
        assert result[key] is None
    assert all(row['complete_callee_work_bound'] is row['complete_callee_stack_bound_bytes'] is None
               for row in result['ordinary_calls'])
    assert all(row['complete_stack_bound_bytes'] is None for row in result['inherited_outer_roots_unchanged'])


@pytest.mark.parametrize('op,operands', [
    ('ret', 'x0'), ('blr', 'x0'), ('br', 'x0'), ('brk', '#0'), ('udf', '#0'), ('hlt', '#0'),
    ('b.foo', '0x4'), ('bl', 'x0'), ('b.eq', '0x4 junk'), ('cbz', 'sp, 0x4'), ('cbnz', 'w00, 0x4'),
    ('add', 'sp, x29, #16'), ('sub', 'sp, x0, #16'), ('mov', 'sp, x0'), ('mov', 'x30, x0'),
    ('mov', 'w00, #1'), ('mov', 'w٠, #1'), ('mov', 'w0, #١'), ('mov', 'w0, #65536'),
    ('add', 'x0, sp, #4096'), ('add', 'x29, x0, #1'), ('add.foo', 'x0, x0, #1'),
    ('stp', 'x0, x1, [sp, #1]!'), ('stp', 'x0, x1, [sp, #16]!, #16'),
    ('ldp', 'x0, x0, [sp]'), ('ldp', 'x29, x0, [sp, #80]'), ('ldp', 'x0, x1, [x0], #16'),
    ('str', 'x0, [sp, #1]'), ('str', 'x0, [sp, #32768]'), ('ldr', 'x0, [x1]!'),
    ('ldr', 'sp, [x0]'), ('ldurh', 'w0, [x0, #256]'), ('lsr', 'x0, x1, #64'),
    ('orr', 'w0, w1, w2, lsl #32'), ('orr', 'x0, x1, #0x3'), ('cmp', 'x0, w1'),
    ('bfi', 'x0, x1, #8, #57'), ('cmn', 'x0, #-1'), ('unknown', ''),
])
def test_unsupported_control_registers_operands_and_writeback_refuse(op, operands):
    with pytest.raises(ValueError):
        M.instruction(op, operands)


def test_ordinary_sp_effects_and_register_alias_tracking():
    assert M.instruction('stp', 'x28, x27, [sp, #-0x60]!')['sp_delta'] == -96
    assert M.instruction('sub', 'sp, sp, #0x970')['sp_delta'] == -2416
    assert M.instruction('ldp', 'x28, x27, [sp], #0x60')['sp_delta'] == 96
    assert M.instruction('mov', 'w2, #0x2b9')['writes'] == ['x2']
    assert M.instruction('bl', '0x10 <<misleading::name>>')['target'] == 16
    assert M.instruction('cmp', 'x0, #1')['writes'] == []


def test_weighted_composition_counts_bl_and_leaf_ret_and_two_calls():
    edges = {0: [4], 4: [8], 8: []}
    order = M.topology(edges, 0)
    assert M.longest(edges, order, {0: 35, 4: 35, 8: 1}, 0)['maximum'] == 71
    for value in (-1, True, None):
        with pytest.raises(ValueError, match='weight'):
            M.longest(edges, order, {0: value}, 0)
    for edges in ({0: [0]}, {0: [4]}):
        with pytest.raises(ValueError):
            M.topology(edges, 0)


def test_sp_merge_alignment_and_balanced_return_controls():
    def word(delta=0, kind='straight'):
        return {'sp_delta': delta, 'kind': kind}
    edges = {0: [4, 8], 4: [12], 8: [12], 12: []}
    with pytest.raises(ValueError, match='merge'):
        M.frame_states({0: word(), 4: word(-16), 8: word(), 12: word(0, 'return')}, edges, M.topology(edges, 0), 0)
    for delta, error in [(-8, 'invalid'), (16, 'invalid'), (-16, 'unbalanced')]:
        with pytest.raises(ValueError, match=error):
            M.frame_states({0: word(delta, 'return')}, {0: []}, [0], 0)


def test_memcpy_request_requires_dominating_constant_and_no_intervening_call():
    words = {0: M.instruction('mov', 'w2, #0x10'), 4: M.instruction('add', 'x23, sp, #0x10'),
             8: M.instruction('bl', '0x100')}
    edges = {0: [4], 4: [8], 8: []}
    assert M.request_length(8, words, edges) == 16
    for bad in (M.instruction('bl', '0x100'), M.instruction('mov', 'x2, x0')):
        changed = dict(words, **{})
        changed[4] = bad
        with pytest.raises(ValueError):
            M.request_length(8, changed, edges)
    with pytest.raises(ValueError, match='ambiguous'):
        M.request_length(8, words, {0: [8], 4: [8], 8: []})


def changed_line(raw, address, op, operands):
    changed = dict(raw)
    lines = raw['disassembly'].splitlines()
    prefix = f'{address:x}:'
    assert sum(line.startswith(prefix) for line in lines) == 1
    changed['disassembly'] = '\n'.join(f'{prefix}\t{op}\t{operands}' if line.startswith(prefix) else line for line in lines) + '\n'
    return changed


@pytest.mark.parametrize('address,op,operands,error', [
    (0x100000b74, 'b.ne', '0x100000b40', 'cycle'),
    (0x100000b74, 'b.ne', '0x100000e40', 'escape'),
    (0x100000b74, 'b.ne', '0x100000b42', 'escape'),
    (0x100000b74, 'b.ne', '0x100000e84', 'escape'),
    (0x100000bf4, 'bl', '0x10000c2a8', 'callee'),
    (0x100000bf4, 'bl', '0x10000b9d8', 'inventory'),
    (0x100000c44, 'add', 'sp, sp, #0x960', 'unbalanced'),
    (0x100000b40, 'stp', 'x28, x27, [sp, #-0x58]!', 'invalid SP'),
    (0x100000e80, 'unknown', '', 'unsupported'),
    (0x100000d4c, 'mov', 'w1, #0x170', 'request'),
])
def test_current_wrapper_drift_or_exceptional_entry_refuses(address, op, operands, error, captured):
    leaf, modes, reviewed, _ = captured
    raw, original = modes['portable']
    with pytest.raises(ValueError, match=error):
        M.compose(leaf, changed_line(raw, address, op, operands), original, reviewed['modes']['portable'])


@pytest.mark.parametrize('change', ['bound', 'image', 'roots'])
def test_unbound_or_foreign_leaf_cannot_supply_a_composition(change, captured):
    leaf, modes, reviewed, _ = captured
    altered = copy.deepcopy(reviewed['modes']['portable'])
    if change == 'bound':
        altered['leaf']['ordinary_instruction_events_upper_bound'] = 33
    elif change == 'image':
        altered['original_image_binding'] = reviewed['modes']['accel']['original_image_binding']
    else:
        altered['inherited_outer_roots_unchanged'].pop()
    with pytest.raises(ValueError, match='leaf image/bound/roots'):
        M.compose(leaf, *modes['portable'], altered)


def test_complete_interval_required_even_outside_ordinary_graph(captured):
    leaf, modes, reviewed, _ = captured
    raw, original = modes['portable']
    for address in (M.START, 0x100000e40):
        altered = dict(raw)
        altered['disassembly'] = '\n'.join(line for line in raw['disassembly'].splitlines() if not line.startswith(f'{address:x}:'))
        with pytest.raises(ValueError, match='incomplete'):
            M.compose(leaf, altered, original, reviewed['modes']['portable'])
    altered = dict(raw)
    altered['starts'] = '\n'.join(line for line in raw['starts'].splitlines() if not line.startswith(f'{M.END:016x}'))
    with pytest.raises(ValueError, match='successor'):
        M.compose(leaf, altered, original, reviewed['modes']['portable'])


def test_verified_source_buffer_and_pinned_input_refusals(tmp_path, monkeypatch):
    path = tmp_path / 'input'
    path.write_bytes(b'original')
    guards = {}
    assert M.pinned(path, M.sha(b'original'), guards) == b'original'
    path.write_bytes(b'changed')
    with pytest.raises(ValueError, match='pinned input'):
        M.pinned(path, M.sha(b'original'), guards)
    original = Path.read_bytes
    monkeypatch.setattr(Path, 'read_bytes', lambda p: original(p) + b'changed' if str(p).endswith(M.LEAF_HELPER) else original(p))
    with pytest.raises(ValueError, match='pinned input'):
        M.load_leaf(ROOT, {})


def mocked_leaf(monkeypatch, captured, late_refusal=False):
    leaf, modes, _, triage = captured
    fake = SimpleNamespace(**leaf.__dict__)
    fake.load_inputs = lambda *args: (copy.deepcopy(modes), copy.deepcopy(triage))
    if late_refusal:
        def refuse(_):
            raise ValueError('late input drift')
        fake.recheck = refuse
    monkeypatch.setattr(M, 'load_leaf', lambda *args: fake)


def test_mock_capture_keeps_all_roots_113_failures_and_null_costs(tmp_path, monkeypatch, captured):
    mocked_leaf(monkeypatch, captured)
    output = tmp_path / 'synthetic-composition'
    report = M.run(ROOT, CACHE, output, M.sha(PATH.read_bytes()), M.sha(Path(__file__).read_bytes()))
    assert report['analysis_passed'], report['failure']
    assert report['historical_selected_nodes']['failed'] == 113
    assert report['historical_selected_nodes']['passed'] == 47
    for mode, (_, original) in captured[1].items():
        assert report['modes'][mode]['inherited_outer_roots_unchanged'] == original['roots']
    for key in ('latest_source_build_validated', 'historical_evidence_rebound', 'complete_stack_certificate',
                'complete_workspace_certificate', 'profile_qualified', 'release_qualified', 'physical_cost_calibrated'):
        assert report[key] is False
    before = (output / 'report.json').read_bytes()
    with pytest.raises(FileExistsError):
        M.run(ROOT, CACHE, output, M.sha(PATH.read_bytes()), M.sha(Path(__file__).read_bytes()))
    assert (output / 'report.json').read_bytes() == before


def test_late_failure_keeps_refused_attempt(tmp_path, monkeypatch, captured):
    mocked_leaf(monkeypatch, captured, True)
    output = tmp_path / 'late-drift'
    report = M.run(ROOT, CACHE, output, M.sha(PATH.read_bytes()), M.sha(Path(__file__).read_bytes()))
    assert not report['analysis_passed'] and report['failure'] == 'ValueError: late input drift'
    assert json.loads((output / 'report.json').read_bytes()) == report


def test_reviewed_helper_and_test_pins_precede_any_leaf_work(tmp_path, monkeypatch):
    monkeypatch.setattr(M, 'load_leaf', lambda *args: pytest.fail('must refuse first'))
    for helper_pin, test_pin, name in [('0' * 64, '0' * 64, 'helper'), (M.sha(PATH.read_bytes()), '0' * 64, 'tests')]:
        result = M.run(ROOT, CACHE, tmp_path / name, helper_pin, test_pin)
        assert not result['analysis_passed'] and 'pinned input changed' in result['failure']


def test_protected_evidence_and_repository_outputs_refuse():
    for output in (ROOT / 'new-proof', CACHE, CACHE / 'stack-v5/new-proof', CACHE / 'record-leaf-new'):
        with pytest.raises(ValueError, match='separate external'):
            M.run(ROOT, CACHE, output, '0' * 64, '0' * 64)


@pytest.mark.parametrize('success', [False, True])
def test_cli_with_mocked_capture(tmp_path, monkeypatch, capsys, success):
    import sys
    monkeypatch.setattr(sys, 'argv', ['frontend', '--root', str(ROOT), '--cache', str(CACHE),
                                    '--output', str(tmp_path / 'mock'), '--helper-sha256', 'a' * 64,
                                    '--tests-sha256', 'b' * 64])
    monkeypatch.setattr(M, 'run', lambda *args: {'analysis_passed': success, 'failure': None if success else 'mock refusal'})
    assert M.main() == (0 if success else 1)
    assert json.loads(capsys.readouterr().out)['analysis_passed'] is success
