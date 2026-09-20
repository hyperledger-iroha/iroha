"""Static fixed-scaling release predicates; no model/native/process qualification.

These tests parse copied current source, execute only the two focused checker
functions, and independently mutate component bytes plus their declared hashes.
The complete preflight requires a current mapping and an original observed pass for each assertion.
"""
from pathlib import Path
import ast
import hashlib
import json
import types

import pytest

ROOT = Path(__file__).resolve().parents[2]
CHECKER = 'scripts/formal/sumeragi_v2_proof_ledger_release_inventory_contracts.py'
SHELL = 'scripts/run_sumeragi_v2_release_gates.sh'
BOOTSTRAP = 'scripts/bootstrap_sumeragi_v2_release.py'
RECEIPT = 'scripts/write_sumeragi_v2_release_receipt.py'
GATE = 'scripts/write_sumeragi_v2_release_receipt_gate_evidence.py'
REPLAY = 'scripts/bootstrap_sumeragi_v2_release_receipt_replay.py'
CORRIDOR = 'scripts/write_sumeragi_v2_release_receipt_corridor_log.py'
CACHE = 'scripts/copy_sumeragi_v2_release_cargo_cache.py'
VALIDATOR = 'scripts/validate_sumeragi_v2_release_bootstrap.py'
FILES = (SHELL, BOOTSTRAP, RECEIPT, GATE, REPLAY, CORRIDOR, CACHE, VALIDATOR)


def checker():
    tree = ast.parse((ROOT / CHECKER).read_bytes())
    names = ('_fixed_scaling_release_contract_errors', '_fixed_scaling_preflight_inventory_errors')
    chosen = [node for node in tree.body if isinstance(node, ast.FunctionDef) and node.name in names]
    assert len(chosen) == 2
    module = ast.Module(body=chosen, type_ignores=[])
    scope = {'ast': ast, 'Path': Path, '__name__': 'copied_static_scaling_contracts'}
    exec(compile(module, str(ROOT / CHECKER), 'exec'), scope)
    return types.SimpleNamespace(implemented=scope[names[0]], preflight=scope[names[1]])


def assigned(path, name):
    nodes = [node for node in ast.walk(ast.parse(path.read_bytes())) if isinstance(node, ast.Assign)
             and len(node.targets) == 1 and isinstance(node.targets[0], ast.Name) and node.targets[0].id == name]
    assert len(nodes) == 1
    return ast.literal_eval(nodes[0].value)


@pytest.fixture
def copied(tmp_path):
    inventory=json.loads((ROOT/'pytests/scripts/scaling_preflight/inventory.json').read_bytes())
    for relative in sorted(set(FILES) | set(inventory['sources']) | {
            'pytests/scripts/scaling_preflight/inventory.json',
            'pytests/scripts/scaling_preflight/phase_nodes.json'}):
        source = ROOT / relative
        target = tmp_path / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(source.read_bytes())
        assert hashlib.sha256(target.read_bytes()).digest() == hashlib.sha256(source.read_bytes()).digest()
    return tmp_path


def inspect(root):
    return checker().implemented(root, (root / SHELL).read_text())


def change(root, relative, before, after):
    path = root / relative
    raw = path.read_text()
    assert raw.count(before) == 1, (relative, before, raw.count(before))
    old_digest = hashlib.sha256(path.read_bytes()).hexdigest()
    path.write_text(raw.replace(before, after, 1))
    # Rebind component digest declarations deliberately, so semantic negatives
    # cannot accidentally pass by testing only a stale component hash.
    parent = RECEIPT if relative in (GATE, CORRIDOR) else BOOTSTRAP if relative == REPLAY else None
    if parent:
        parent_path = root / parent
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        text = parent_path.read_text()
        assert text.count(old_digest) == (2 if parent == RECEIPT else 1)
        parent_path.write_text(text.replace(old_digest, digest))
        table = '_RELEASE_RECEIPT_COMPONENT_SHA256' if parent == RECEIPT else '_BOOTSTRAP_COMPONENT_SHA256'
        assert assigned(parent_path, table)[path.name] == digest


def rejected(root, expected):
    errors = inspect(root)
    assert any(expected in error for error in errors), errors


def test_copied_current_implemented_contract_is_positive_and_not_a_preflight_pass(copied):
    assert inspect(copied) == []
    errors = checker().preflight(copied / SHELL)
    assert errors == []


@pytest.mark.parametrize('case', ['original', 'remove_todo_and_exit', 'empty', 'fake_53_success'])
def test_preflight_cannot_be_qualified_by_todo_deletion_empty_scope_or_retired_count(copied, case):
    path = copied / SHELL
    if case == 'remove_todo_and_exit':
        raw = path.read_text()
        path.write_text(raw.replace('original protected parent runs the complete source-bound scaling preflight','preflight omitted'))
    elif case == 'empty': path.write_text('')
    elif case == 'fake_53_success': path.write_text('preflight-multilane-scaling pytest 53\nexit 0\n')
    errors=checker().preflight(path)
    if case=='original':assert errors==[]
    else:assert any('contract is invalid' in error for error in errors)


def test_aggregate_checker_requires_both_implemented_contract_and_unmet_complete_preflight():
    tree = ast.parse((ROOT / CHECKER).read_bytes())
    owner = next(node for node in tree.body if isinstance(node, ast.FunctionDef)
                 and node.name == '_production_liveness_release_inventory_errors')
    calls = [ast.unparse(node) for node in ast.walk(owner) if isinstance(node, ast.Call)]
    assert calls.count('errors.extend(_fixed_scaling_release_contract_errors(repo_root, source))') == 1
    assert calls.count('errors.extend(_fixed_scaling_preflight_inventory_errors(release_path))') == 1
    assert calls.index('errors.extend(_fixed_scaling_release_contract_errors(repo_root, source))') < calls.index(
        'errors.extend(_fixed_scaling_preflight_inventory_errors(release_path))')


@pytest.mark.parametrize('relative,before,after,reason', [
    ('scripts/nexus/scaling_release_record.py',
     "preflight = validate_preflight_binding(value['preflight'])",
     "preflight = value['preflight']", 'canonical preflight binding'),
    ('scripts/nexus/scaling_release_record.py',
     "_require(preflight['scope']['completed_ns'] <= start)",
     '_require(True)', 'before the original collector start'),
    (GATE, "timeout_seconds=runner['scaling_preflight_timeout_seconds']",
     "timeout_seconds=record.document['preflight']['scope']['timeout_seconds']",
     'context must come from authenticated source and runner'),
    (GATE, 'preflight = api.inspect_preflight_archive(preflight_root, record, **preflight_context)',
     'preflight = None', 'complete selected-source preflight'),
    (GATE, 'api.capture_preflight_archive(preflight_root, record, **preflight_context) != preflight_before',
     'False', 'recheck complete preflight after native replay'),
    (GATE, "record.document['preflight']['scope']['timeout_seconds'] != timeout",
     'False', 'timeout must match the original runner'),
    (REPLAY, 'scaling_record_api.inspect_preflight_archive(preflight_root, scaling_record, **preflight_context)',
     'pass', 'inspect the complete preflight archive'),
    (REPLAY, 'scaling_record_api.capture_preflight_archive(preflight_root, scaling_record, **preflight_context) != (preflight_files, preflight_directories)',
     'False', 'recapture preflight after its publication fence'),
])
def test_preflight_source_context_and_publication_checks_survive_rehashed_mutation(copied,relative,before,after,reason):
    change(copied,relative,before,after)
    rejected(copied,reason)


@pytest.mark.parametrize('relative,name', [(BOOTSTRAP, '_RUNNER_ENV_ALLOWLIST'),
    (VALIDATOR, '_RUNNER_EXTRA_ENV'), (RECEIPT, '_BOOTSTRAP_RUNNER_ENV_ALLOWLIST')])
@pytest.mark.parametrize('value', ['IROHA_RELEASE_SCALING_GATE_FD', 'IROHA_RELEASE_SCALING_EVIDENCE_MANIFEST'])
def test_external_allowlists_cannot_accept_either_new_internal_or_retired_scaling_authority(copied, relative, name, value):
    text = (copied / relative).read_text()
    parsed = ast.parse(text)
    node = next(node for node in parsed.body if isinstance(node, ast.Assign)
                and any(isinstance(target, ast.Name) and target.id == name for target in node.targets))
    original = ast.get_source_segment(text, node.value)
    replacement = repr(ast.literal_eval(node.value) | {value})
    change(copied, relative, original, replacement)
    rejected(copied, 'reject every external scaling trust input')


@pytest.mark.parametrize('relative,name', [(BOOTSTRAP, '_VALIDATOR_OPTION_ORDER'),
    (CORRIDOR, '_RECEIPT_VALIDATION_OPTION_ORDER'), (CACHE, 'VALIDATOR_OPTION_ORDER')])
@pytest.mark.parametrize('case', ['swap', 'retired', 'missing'])
def test_all_three_receipt_consumers_require_exact_same_47_options_even_after_rehash(copied, relative, name, case):
    text = (copied / relative).read_text()
    node = next(node for node in ast.parse(text).body if isinstance(node, ast.Assign)
                and any(isinstance(target, ast.Name) and target.id == name for target in node.targets))
    values = list(ast.literal_eval(node.value))
    assert len(values) == 47
    index = values.index('--scaling-execution-record')
    if case == 'swap': values[index], values[index + 1] = values[index + 1], values[index]
    elif case == 'retired': values[index] = '--scaling-evidence-manifest'
    else: values.pop(index)
    change(copied, relative, ast.get_source_segment(text, node.value), repr(tuple(values)))
    rejected(copied, 'canonical 47-option record contract')


@pytest.mark.parametrize('key,expression', [
    ('IROHA_RELEASE_INVOCATION_ROOT', 'str(scaling_invocation.path)'),
    ('IROHA_RELEASE_TEMP_BASE', 'str(scaling_invocation.base)'),
    ('IROHA_RELEASE_SCALING_GATE_FD', 'str(scaling_handoff.runner_descriptor)'),
    ('IROHA_RELEASE_SCALING_INVOCATION_SHA256', 'scaling_operation.invocation_sha256'),
    ('IROHA_RELEASE_SCALING_CHALLENGE', 'scaling_handoff.challenge'),
    ('IROHA_RELEASE_SCALING_HANDOFF_HELPER_SHA256', "archives['scaling_handoff_helper'].sha256"),
])
def test_internal_handoff_each_value_must_come_from_original_parent_owner(copied, key, expression):
    change(copied, BOOTSTRAP, repr(key) + ': ' + expression, repr(key) + ": 'forged'")
    rejected(copied, 'derive all six exact values from original parent owners')


@pytest.mark.parametrize('before,after,reason', [
    ('"scaling_handoff": scaling_environment,', '"scaling_handoff": runner_extra_environment,', 'authenticated runner marker'),
    ('original_scaling_observation = scaling_handoff.revalidate_observation()', 'original_scaling_observation = None', 'original handoff observations'),
    ('scaling_execution = scaling_operation.revalidate_final(original_scaling_observation)', 'scaling_execution = None', 'same original parent operation'),
    ('deadline_ns=launch.deadline_ns)', 'deadline_ns=None)', 'unchanged absolute deadline'),
    ('and self._command.terminal is not None):', 'or self._command.terminal is not None):', 'original terminal reap'),
    ('_scaling_require(self._operation.verify_publication(launch, observation) is None)', 'pass', 'verified by the original operation'),
])
def test_parent_observation_lifetime_and_verification_contracts_are_not_replaceable(copied, before, after, reason):
    change(copied, BOOTSTRAP, before, after)
    rejected(copied, reason)


@pytest.mark.parametrize('before,after,reason', [
    ('--gate-fd "$release_gate_fd"', '--gate-fd "7"', 'one isolated parent handoff'),
    ('exec {release_gate_fd}<&-\n  release_gate_active=0', 'release_gate_active=0', 'channel must close'),
    ('readonly release_scaling_execution_record="$release_bootstrap_evidence_dir/scaling-execution.json"',
     'readonly release_scaling_execution_record="external.json"', 'record path must be fixed'),
    ('echo "original parent scaling execution record is missing" >&2\n      sealed_status=2',
     'echo "original parent scaling execution record is missing" >&2\n      sealed_status=0', 'missing parent record'),
])
def test_shell_cannot_replace_parent_request_record_or_failure_with_claimed_success(copied, before, after, reason):
    change(copied, SHELL, before, after)
    rejected(copied, reason)


@pytest.mark.parametrize('relative,before,after,reason', [
    (GATE, '        _replay_fixed_scaling_native(api, checked, record, root, kagami, checker_environment)',
     '        pass', 'replay native evidence'),
    (GATE, "handoff = authentication['runner']['scaling_handoff']", "handoff = {}", 'authenticated internal handoff'),
    (GATE, "if record.document['execution']['invocation_sha256'] != handoff['IROHA_RELEASE_SCALING_INVOCATION_SHA256']:",
     'if False:', 'original parent invocation'),
    (GATE, 'if api.inspect_parent_archive(root, record, sealed) != checked or api.capture_public_archive(root, record) != before:',
     'if False:', 'repeat original archive inspection'),
    (GATE, 'if _validate_scaling_parent_inputs(api, record, bootstrap_evidence, bootstrap_authentication) != selected:',
     'if False:', 'retain parent plan and budget'),
    (REPLAY, 'scaling_record_api.inspect_parent_archive(scaling_root, scaling_record, scaling_identity)',
     'pass', 'inspect the same fixed archive'),
    (REPLAY, "or receipt_evidence['multilane_scaling'] != scaling_projection):", 'or False):', 'original parent record projection'),
])
def test_component_semantic_changes_fail_even_with_updated_parent_component_digests(copied, relative, before, after, reason):
    change(copied, relative, before, after)
    rejected(copied, reason)


@pytest.mark.parametrize('relative', [BOOTSTRAP, GATE, REPLAY, VALIDATOR, CACHE, CORRIDOR, RECEIPT])
@pytest.mark.parametrize('case', ['missing', 'symlink', 'syntax'])
def test_checked_sources_must_remain_regular_and_parseable(copied, relative, case):
    path = copied / relative
    if case == 'missing': path.unlink()
    elif case == 'symlink':
        retained = path.with_suffix('.retained'); path.rename(retained); path.symlink_to(retained.name)
    else: path.write_text('def broken(\n')
    rejected(copied, 'valid regular Python source')


def test_component_symbols_and_digest_literals_match_the_current_copied_implementations():
    parsed = ast.parse((ROOT / CHECKER).read_bytes())
    for table, parent, assignment, symbols in (
        ('expected_receipt_component_sha256', RECEIPT, '_RELEASE_RECEIPT_COMPONENT_SHA256', 'expected_component_symbols'),
        ('expected_bootstrap_component_sha256', BOOTSTRAP, '_BOOTSTRAP_COMPONENT_SHA256', 'expected_bootstrap_component_symbols'),
    ):
        expected = assigned(ROOT / CHECKER, table)
        assert expected == assigned(ROOT / parent, assignment)
        expected_symbols = assigned(ROOT / CHECKER, symbols)
        for name, digest in expected.items():
            source = ROOT / 'scripts' / name
            assert hashlib.sha256(source.read_bytes()).hexdigest() == digest
            actual = tuple(node.name for node in ast.parse(source.read_bytes()).body
                           if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)))
            assert actual == expected_symbols[name]
