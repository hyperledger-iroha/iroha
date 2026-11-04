"""Finite ordinary leaf paths, strict retained custody and unknown outer costs."""
import copy
import gzip
import importlib.util
import json
from pathlib import Path

import pytest


ROOT = Path(__file__).resolve().parents[2]
PATH = ROOT / 'scripts/parliament_ballot_retained_record_bound.py'
SPEC = importlib.util.spec_from_file_location('retained_record_bound', PATH)
M = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(M)
ARCHIVE = ROOT / 'docs/validation/parliament_ballot_stack_v5_20261104'
CACHE = Path('/Users/takemiyamakoto/Library/Caches/iroha-script-tests/runs/parliament-current-20261104')


def text(words):
    return 'Disassembly of section __TEXT,__text:\n' + '\n'.join(
        f'{address:x}:\t{op}\t{operands}' for address, op, operands in words) + '\n'


def starts(*addresses):
    return 'fixture:\n' + '\n'.join(f'{address:016x} symbol_{address:x}' for address in addresses) + '\n'


@pytest.fixture(scope='module')
def retained():
    result = {}
    for mode in M.MODES:
        original = json.loads(gzip.decompress((ARCHIVE / (mode + '.json.gz')).read_bytes()))
        raw = gzip.decompress((ARCHIVE / mode / 'disassembly.txt.gz').read_bytes())
        assert M.sha(raw) == original['image_binding']['disassembly_sha256']
        result[mode] = (raw.decode(), (ARCHIVE / mode / 'starts.txt').read_text(), original)
    return result


@pytest.mark.parametrize('mode', M.MODES)
def test_retained_complete_interval_and_independent_path_count(mode, retained):
    disassembly, metadata, original = retained[mode]
    result = M.leaf_bound(disassembly, metadata)
    assert result['decoded_words'] == result['reachable_words'] == 55
    assert result['ordinary_instruction_events_upper_bound'] == 34
    edges = {int(row['address'], 16): [int(a, 16) for a in row['successors']]
             for row in result['instructions_and_cfg']}
    # Independent exhaustive path enumeration, not the implementation's DP.
    pending, paths = [(M.START, [])], []
    while pending:
        address, prefix = pending.pop()
        assert address not in prefix
        path = [*prefix, address]
        if edges[address]:
            pending.extend((target, path) for target in edges[address])
        else:
            paths.append(path)
    assert max(map(len, paths)) == len(result['longest_cfg_path']) == 34
    assert [int(a, 16) for a in result['longest_cfg_path']] in paths
    assert any(target < address for address, targets in edges.items() for target in targets)
    assert result['ordinary_explicit_sp_decrement_bytes'] == result['ordinary_calls'] == 0
    for key in ('complete_physical_stack_bound_bytes', 'complete_work_bound', 'elapsed_time_bound',
                'exceptional_path_bound', 'caller_bound', 'pointer_memory_bound'):
        assert result[key] is None
    assert all(row['complete_stack_bound_bytes'] is None for row in original['roots'])


def test_both_outcomes_count_branch_and_ret():
    result = M.leaf_bound(text([(0, 'cmp', 'x0, #0x0'), (4, 'b.ne', '0x10'),
                               (8, 'mov', 'w1, #0x1'), (12, 'ret', ''), (16, 'ret', '')]),
                          starts(0, 20), 0, 20)
    assert result['ordinary_instruction_events_upper_bound'] == 4
    assert result['instructions_and_cfg'][1]['successors'] == ['0x10', '0x8']
    assert result['longest_cfg_path'] == ['0x0', '0x4', '0x8', '0xc']


@pytest.mark.parametrize('op,operand', [
    ('bl', '0x100'), ('blr', 'x0'), ('br', 'x0'), ('b', '0x4'), ('brk', '#0x1'),
    ('udf', '#0x0'), ('hlt', '#0'), ('ret', 'x0'), ('ret.foo', ''), ('unknown', ''),
    ('sub', 'sp, sp, #0x10'), ('add', 'x29, x0, #0x0'), ('mov', 'w29, #0x1'),
    ('ldr', 'w1, [sp], #0x4'), ('ldr', 'w1, [x29], #0x4'), ('ldr', 'w1, [x1], #0x4'),
    ('ldr', 'w00, [x0], #0x4'), ('ldr', 'w0, [x00], #0x4'), ('mov', 'w00, #1'),
    ('ldr', 'w0, [x٠], #0x4'), ('mov', 'w٠, #1'), ('mov', 'w0, #١'),
    ('ldr', 'x1, [x0], #0x4'), ('ldrb', 'w0, [x1]!'), ('strb', 'w0, [x1, #1]'),
    ('str', 'x0, [x1, #0x1]'), ('str', 'x0, [x1, #32768]'), ('stp', 'x0, x1, [x2, #512]'),
    ('mov', 'w1, #65536'), ('movk', 'w1, #1, lsl #32'), ('cmp', 'x0, w1'),
    ('cmp', 'x0, #4096'), ('csel', 'x0, x1, x30, ne'), ('b.ne', 'x0'),
    ('b.ne', '0x4 extra'), ('b.foo', '0x4'), ('add.foo', 'x0, x0, #1'),
])
def test_unknown_control_stack_registers_and_operands_refuse(op, operand):
    with pytest.raises(ValueError):
        M.leaf_bound(text([(0, op, operand), (4, 'ret', '')]), starts(0, 8), 0, 8)


@pytest.mark.parametrize('target', ['0x2', '0x8', '0x100'])
def test_branch_targets_must_be_exact_interval_words(target):
    with pytest.raises(ValueError, match='escapes'):
        M.leaf_bound(text([(0, 'b.ne', target), (4, 'ret', '')]), starts(0, 8), 0, 8)


def test_backedge_cycle_refuses_even_with_a_return_exit():
    with pytest.raises(ValueError, match='cycle'):
        M.leaf_bound(text([(0, 'b.ne', '0x0'), (4, 'ret', '')]), starts(0, 8), 0, 8)


def test_fallthrough_after_interval_and_unreachable_words_refuse():
    with pytest.raises(ValueError, match='escapes'):
        M.leaf_bound(text([(0, 'mov', 'w0, #1')]), starts(0, 4), 0, 4)
    with pytest.raises(ValueError, match='unreachable'):
        M.leaf_bound(text([(0, 'ret', ''), (4, 'ret', '')]), starts(0, 8), 0, 8)


@pytest.mark.parametrize('metadata', [starts(0), starts(0, 4, 8), starts(0, 12),
                                      starts(0, 8, 8), starts(0, 6), starts(8, 0)])
def test_exact_successor_boundary_refuses_stale_or_ambiguous_metadata(metadata):
    with pytest.raises(ValueError):
        M.leaf_bound(text([(0, 'mov', 'w0, #1'), (4, 'ret', '')]), metadata, 0, 8)


@pytest.mark.parametrize('body', [text([(0, 'mov', 'w0, #1')]),
                                  text([(0, 'mov', 'w0, #1'), (4, 'ret', ''), (4, 'ret', '')]),
                                  text([(0, 'mov', 'w0, #1'), (2, 'ret', '')]),
                                  'Disassembly of section __TEXT,__text:\n0: ???\nmalformed\n'])
def test_missing_duplicate_misaligned_and_unparsed_words_refuse(body):
    with pytest.raises(ValueError):
        M.leaf_bound(body, starts(0, 8), 0, 8)


def test_labels_comments_and_symbol_names_are_not_control_authority():
    body = text([(0, 'b.ne', '0x8 <misleading_name>'), (4, 'ret', ''), (8, 'ret', '')])
    assert M.leaf_bound(body, starts(0, 12), 0, 12)['ordinary_instruction_events_upper_bound'] == 2
    for body in ('', 'Disassembly of section __TEXT,__text:\n' * 2):
        with pytest.raises(ValueError, match='section'):
            M.parse_text(body)


def test_pins_parse_exact_bytes_reject_duplicate_keys_and_late_changes(tmp_path):
    path = tmp_path / 'input'
    path.write_bytes(b'{"x": 1}')
    guards = {}
    assert M.object_json(M.pinned(path, M.sha(path.read_bytes()), guards)) == {'x': 1}
    assert M.recheck(guards) == guards
    path.write_bytes(b'{"x": 2}')
    with pytest.raises(ValueError, match='changed'):
        M.recheck(guards)
    with pytest.raises(ValueError, match='pinned input'):
        M.pinned(path, next(iter(guards.values())), {})
    for data in (b'{"x":1,"x":2}', b'{"x":NaN}', b'{"x":Infinity}'):
        with pytest.raises(ValueError):
            M.object_json(data)


def test_original_input_chain_and_all_outer_obligations_are_preserved():
    guards = {}
    modes, triage = M.load_inputs(ROOT, CACHE, guards)
    assert triage['passed'] == 47 and triage['failed'] == 113
    assert len(modes) == 2 and len(guards) >= 25
    for raw, report in modes.values():
        assert len(report['roots']) == 5
        assert all(row['complete_stack_bound_bytes'] is None for row in report['roots'])
        worker = next(row for row in report['roots'] if row['root'] == 'parliament_memory_entry::workspace::worker')
        assert worker['frontier_kind_counts']['inconsistent_sp_merge'] == 1
        assert worker['positive_weight_recursive_components']
        assert all(worker['frontier_kind_counts'][kind] > 0 for kind in
                   ('native_edge', 'exceptional_terminal', 'indirect_call', 'indirect_branch', 'function_fallthrough'))
        assert 'Disassembly of section __TEXT,__text:' in raw['disassembly']
    assert M.recheck(guards) == guards


@pytest.mark.parametrize('suffix', [
    'build/build-receipt.json', 'stack-v5/receipt.json', 'stack-v5/portable.json',
    'binaries/worker-portable', 'stack-v5/portable/disassembly.txt', 'stack-v5/portable/starts.txt',
    'stack-v5/portable/stubs.txt', 'stack-v5/portable/unwind.txt', 'src/test_codec.rs', 'src/profile.rs',
    'worker/generated_workspace.rs', 'parliament_ballot_stack_audit.py', '/llvm-objdump',
])
def test_changed_original_input_cannot_become_new_baseline(monkeypatch, suffix):
    original = Path.read_bytes
    monkeypatch.setattr(Path, 'read_bytes', lambda path: original(path) + b'changed' if str(path).endswith(suffix) else original(path))
    with pytest.raises(ValueError, match='pinned input changed'):
        M.load_inputs(ROOT, CACHE, {})


def mock_capture(monkeypatch, retained):
    modes = {mode: ({'disassembly': retained[mode][0], 'starts': retained[mode][1]}, retained[mode][2]) for mode in M.MODES}
    triage = {'historical_receipt': 'synthetic-test-only', 'historical_receipt_sha256': '0' * 64,
              'build_inputs_different_from_retained_capture': [{'path': 'synthetic drift retained'}]}
    monkeypatch.setattr(M, 'load_inputs', lambda root, cache, guards: (copy.deepcopy(modes), copy.deepcopy(triage)))
    return modes


def test_synthetic_capture_preserves_full_outer_roots_and_refuses_output_reuse(tmp_path, monkeypatch, retained):
    modes = mock_capture(monkeypatch, retained)
    output = tmp_path / 'mock-output'
    report = M.run(ROOT, CACHE, output, M.sha(PATH.read_bytes()), M.sha(Path(__file__).read_bytes()))
    assert report['analysis_passed'] is True
    assert report['historical_selected_nodes']['failed'] == 113
    for mode in M.MODES:
        assert report['modes'][mode]['inherited_outer_roots_unchanged'] == modes[mode][1]['roots']
    for name in ('profile_qualified', 'release_qualified', 'physical_cost_calibrated', 'latest_source_build_validated',
                 'complete_stack_certificate', 'complete_workspace_certificate', 'historical_evidence_rebound'):
        assert report[name] is False
    before = (output / 'report.json').read_bytes()
    with pytest.raises(FileExistsError):
        M.run(ROOT, CACHE, output, M.sha(PATH.read_bytes()), M.sha(Path(__file__).read_bytes()))
    assert (output / 'report.json').read_bytes() == before


def test_reviewed_helper_and_tests_pins_are_required_before_analysis(tmp_path, monkeypatch):
    monkeypatch.setattr(M, 'load_inputs', lambda *args: pytest.fail('must refuse first'))
    result = M.run(ROOT, CACHE, tmp_path / 'helper-refusal', '0' * 64, '0' * 64)
    assert result['analysis_passed'] is False and 'pinned input changed' in result['failure']
    result = M.run(ROOT, CACHE, tmp_path / 'test-refusal', M.sha(PATH.read_bytes()), '0' * 64)
    assert result['analysis_passed'] is False and 'pinned input changed' in result['failure']


def test_late_guard_refusal_preserves_attempt_with_no_success(tmp_path, monkeypatch, retained):
    mock_capture(monkeypatch, retained)
    def refuse(_):
        raise ValueError('input changed during analysis')
    monkeypatch.setattr(M, 'recheck', refuse)
    output = tmp_path / 'late-refusal'
    result = M.run(ROOT, CACHE, output, M.sha(PATH.read_bytes()), M.sha(Path(__file__).read_bytes()))
    assert result['analysis_passed'] is False and 'changed' in result['failure']
    assert json.loads((output / 'report.json').read_bytes()) == result


def test_protected_output_paths_refuse_before_any_write():
    for output in (ROOT / 'new-evidence', CACHE, CACHE / 'stack-v5/new-evidence', CACHE / 'stack-v2/new-evidence'):
        with pytest.raises(ValueError, match='separate external'):
            M.run(ROOT, CACHE, output, '0' * 64, '0' * 64)


@pytest.mark.parametrize('passed', [False, True])
def test_cli_reports_mocked_verdict_without_executing_capture(tmp_path, monkeypatch, capsys, passed):
    import sys
    monkeypatch.setattr(sys, 'argv', ['record-bound', '--root', str(ROOT), '--cache', str(CACHE),
                                    '--output', str(tmp_path / 'mock'), '--helper-sha256', 'a' * 64,
                                    '--tests-sha256', 'b' * 64])
    calls = []
    def mock_run(*args):
        calls.append(args)
        return {'analysis_passed': passed, 'failure': None if passed else 'synthetic refusal'}
    monkeypatch.setattr(M, 'run', mock_run)
    assert M.main() == (0 if passed else 1)
    assert len(calls) == 1 and calls[0][-2:] == ('a' * 64, 'b' * 64)
    assert json.loads(capsys.readouterr().out)['analysis_passed'] is passed
