"""Shell source/file contracts; actual Bash/descriptor inheritance is unqualified."""
from __future__ import annotations
import ast
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import subprocess
import sys
import tempfile

import pytest

ROOT=Path(__file__).resolve().parents[2]
SHELL=ROOT/'scripts/run_sumeragi_v2_release_gates.sh'
NEW_BINDINGS=('IROHA_RELEASE_SCALING_GATE_FD','IROHA_RELEASE_SCALING_INVOCATION_SHA256',
    'IROHA_RELEASE_SCALING_CHALLENGE','IROHA_RELEASE_SCALING_HANDOFF_HELPER_SHA256')
OLD_BINDINGS=('IROHA_RELEASE_SCALING_CONFIGURATION_SHA256','IROHA_RELEASE_SCALING_EVIDENCE_MANIFEST',
    'IROHA_RELEASE_SCALING_IROHAD_SHA256','IROHA_RELEASE_SCALING_IROHA_CLI_SHA256',
    'IROHA_RELEASE_SCALING_TRIAL_HARNESS_SHA256')


@pytest.fixture(autouse=True)
def no_children_or_signals(monkeypatch):
    def forbidden(*args,**kwargs): raise AssertionError('Bash/child execution is forbidden in source qualification')
    monkeypatch.setattr(subprocess,'Popen',forbidden)
    for name in ('system','fork','posix_spawn','posix_spawnp','kill','killpg'):
        if hasattr(os,name): monkeypatch.setattr(os,name,forbidden)


def region(source,start,end):
    a=source.index(start);return source[a:source.index(end,a)]


def scopes(source):
    initial=region(source,'release_initial_without_gate() {\n','  return 0\n}\nif ((release_gate_active)); then')
    preparation=region(source,'  release_prepare_without_gate() {\n','  return 0\n  }\n  release_prepare_without_gate')
    direct=region(source,'  unset -f release_prepare_without_gate\n','  exec {release_gate_fd}<&-\n')
    return initial,preparation,direct


def assert_channel_scopes(source):
    initial,preparation,direct=scopes(source)
    prelude=source[:source.index('release_initial_without_gate() {\n')]
    assert '$(' not in prelude and '<(' not in prelude and '\nsource ' not in prelude
    assert 'release_initial_without_gate {release_gate_fd}<&-' in source
    assert 'release_prepare_without_gate {release_gate_fd}<&-' in source
    assert 'if release_initial_without_gate' not in source and 'if release_prepare_without_gate' not in source
    assert 'readonly release_gate_fd=' in prelude and '>= 1048576' in prelude and ' < 3 ' in prelude
    assert 'readonly repo_root=' in initial
    assert 'source "${repo_root}/scripts/run_sumeragi_v2_release_gates_support.sh"' in initial
    assert 'done < <(env)' in initial
    assert '"$release_child_bin/bash" "$sealed_repo_root/scripts/run_sumeragi_v2_release_gates.sh" --release' in preparation
    assert 'sealed_status=$?' in preparation and '--verify-runtime-sources' in preparation
    assert 'candidate identity changed while the sealed release child executed' in preparation
    assert '"$(sha256_file "$release_scaling_handoff_helper")" != "$release_gate_helper_sha256"' in preparation
    cleanup=region(preparation,'  cleanup_release_invocation() {\n','  trap cleanup_release_invocation EXIT\n')
    assert '    } {release_gate_fd}<&-\n' in cleanup
    assert '--cleanup-invocation' in cleanup and 'local status=$?' in cleanup
    assert 'if ((sealed_status == 0)); then' in direct
    assert direct.count('"$release_python_bin" -I -B -S "$release_scaling_handoff_helper"')==1
    assert source.count('--gate-fd "$release_gate_fd"')==1
    assert '--invocation-sha256 "$release_gate_invocation_sha256"' in direct
    assert '--challenge "$release_gate_challenge"' in direct
    assert 'release_scaling_handoff_status=$?' in direct
    assert 'sealed_status=$release_scaling_handoff_status' in direct
    assert 'eval ' not in source
    for name in NEW_BINDINGS:
        assert name in prelude and name not in initial and name not in preparation
    closed=source.index('  exec {release_gate_fd}<&-\n')
    receipt=source.index('    release_receipt_arguments=(\n')
    assert closed<receipt
    assert '--verify-runtime-sources' in source[closed:receipt]
    assert '--verify-cache-sources' in source[closed:receipt]
    assert 'for release_checked_root in "$repo_root" "$sealed_repo_root"; do' in source[closed:receipt]



def test_shell_runtime_rejection_precedes_every_external_child():
    source = SHELL.read_text()
    prelude = source[:source.index('release_initial_without_gate() {\n')]
    guard = region(prelude, 'if (( BASH_VERSINFO[0] < 5 )); then\n', 'fi\n')
    assert 'requires Bash 5 or newer' in guard
    assert 'exit 2\n' in guard
    assert prelude.index(guard) < prelude.index('release_gate_active=0')
    assert '$(' not in prelude and '<(' not in prelude and '\nsource ' not in prelude

def test_source_places_every_ordinary_region_inside_channel_close_scopes():
    assert_channel_scopes(SHELL.read_text())


@pytest.mark.parametrize('old,new',[
    ('release_initial_without_gate {release_gate_fd}<&-','release_initial_without_gate'),
    ('release_prepare_without_gate {release_gate_fd}<&-','release_prepare_without_gate'),
    ('    } {release_gate_fd}<&-\n','    }\n'),
    ('  exec {release_gate_fd}<&-\n','  :\n'),
    ('--gate-fd "$release_gate_fd"','--gate-fd "3"'),
    ('--challenge "$release_gate_challenge"','--challenge "wrong"'),
    ('>= 1048576','>= 9999999'),
    ('sealed_status=$release_scaling_handoff_status','sealed_status=0'),
    ('candidate identity changed while the sealed release child executed','postcheck removed'),
    ('"$(sha256_file "$release_scaling_handoff_helper")" != "$release_gate_helper_sha256"','1 != 1'),
    ('--verify-cache-sources','--unverified-cache'),
])
def test_channel_contract_detects_missing_boundary_or_binding(old,new):
    source=SHELL.read_text();assert old in source
    with pytest.raises((AssertionError,ValueError)):
        assert_channel_scopes(source.replace(old,new))


def test_only_parent_outer_mode_accepts_channel_and_scrubs_environment():
    source=SHELL.read_text();prelude=source[:source.index('release_initial_without_gate() {')]
    branch=region(prelude,'else\n  for release_binding','for release_retired_input')
    assert '"$profile" == "--release" && "${IROHA_RELEASE_SEALED_WORKTREE:-0}" != 1' in prelude
    assert '"${!release_binding+x}"' in branch and 'exit 2' in branch
    for name in NEW_BINDINGS:
        assert name in branch
        assert any(line.startswith('unset ') and name in line for line in prelude.splitlines())
    initial,preparation,_=scopes(source)
    for body in (initial,preparation):
        for name in NEW_BINDINGS: assert name not in body
    assert 'release_initial_without_gate\nfi' in source
    assert source.index('exit "$sealed_status"\nfi') < source.index('source "${repo_root}/scripts/sumeragi_v2_release_process_policy.sh"')


def test_old_input_names_exist_only_as_explicit_rejections():
    source=SHELL.read_text()
    rejection=region(source,'for release_retired_input','unset IROHA_RELEASE_SCALING_GATE_FD')
    assert '"${!release_retired_input+x}"' in rejection and 'exit 2' in rejection
    for name in OLD_BINDINGS:
        assert source.count(name)==1 and name in rejection
    for retired in ('validate_multilane_scaling_evidence.py','scaling_evidence.json',
        'scaling-validation-preflight.json','release_scaling_inventory','release_scaling_source_manifest',
        'run_release_scaling_gate','--scaling-evidence-manifest','--expected-scaling-trial-harness-sha256',
        '--expected-scaling-configuration-sha256','--expected-scaling-irohad-sha256','--expected-scaling-iroha-cli-sha256'):
        assert retired not in source


def test_preflight_and_receipt_dependencies_fail_closed_independently():
    source=SHELL.read_text()
    preflight=region(source,'# The original protected parent runs the complete source-bound scaling preflight','# Run the complete fail-closed proof-ledger')
    assert 'before creating the collector' in preflight
    parent=(ROOT/'scripts/bootstrap_sumeragi_v2_release.py').read_text()
    assert 'self._preflight.run()' in parent
    assert parent.index('self._preflight.run()') < parent.index('provisioning.PreparedScalingInputs.prepare(')
    assert 'pytest' not in preflight and '53' not in preflight
    receipt=region(source,'  readonly release_scaling_execution_record=',
        '  if ((sealed_status == 0)); then\n    release_receipt_arguments=(')
    assert 'original parent scaling execution record is missing' in receipt
    assert '! -s "$release_scaling_execution_record" || -L "$release_scaling_execution_record"' in receipt
    assert 'sealed_status=2' in receipt
    assert 'release_scaling_execution_sha256=' in receipt
    assert '--scaling-observation' not in source and '--scaling-inventory' not in source
    assert 'readonly release_scaling_archive_root="$release_host_root/scaling"' in source


def test_old_preflight_is_not_replaced_with_partial_handoff_coverage():
    source=SHELL.read_text()
    assert 'scripts/tests/run_multilane_scaling_gate_test.py' not in source
    assert 'scripts/tests/validate_multilane_scaling_evidence_test.py' not in source
    assert 'preflight-multilane-scaling pytest 53' not in source
    assert 'sumeragi_v2_release_scaling_handoff_test.py' not in source


def heredocs(source):
    lines=source.splitlines(keepends=True);result=[];i=0
    while i<len(lines):
        match=re.search(r"<<'([A-Z][A-Z0-9_]*)'",lines[i])
        if not match: i+=1;continue
        marker=match[1];i+=1;body=[]
        while i<len(lines) and lines[i].rstrip('\n')!=marker:
            body.append(lines[i]);i+=1
        assert i<len(lines),'unterminated quoted heredoc'
        result.append((marker,''.join(body)));i+=1
    return result


def test_retained_quoted_python_programs_match_current_exact_bodies():
    after=heredocs(SHELL.read_text())
    assert [(marker,hashlib.sha256(body.encode()).hexdigest()) for marker,body in after]==[('PY', '12458313a1502a6ba1fe47dbc3de25eb673532fe8eac076d2d5f4b47f7b1dae0'), ('PY', '67e5827f815cf5dc95cc67101e61c16da281e10ddcf5b364ddfbcd1d394b5d00'), ('ROOT_CHECK', 'cc0bd087d40d00cf95a6befa6370ab49baa69f0772d605654092c28840539edd'), ('PY', '68f89c78cdee3eec5a0fa74eb505d61edf432318c46e619db755d222ecaaf49b'), ('PY', '641ffc08a0425d01f34cfdc0cd714b13c3c18ead33b40598605f18d3f0e4277a'), ('PY', 'de345c4c84047e61c1532e9825d65b60f457d8733f82164633b0e45a73be4d56'), ('PY', '1bee3b1afcb997b7d8bbcfec022f22d8bc91e2e76a4c1b6720cdef6640d56b84'), ('PY', '49129112aae9874f3e8a3578460aa08733e20e48344dab6c49d6c63fc10619d0'), ('PY', 'ebcd21ba15217a89890e45172096330d3003ce4788406d9fa699126b52003e79'), ('PY', '2f4ea5b8e445c5db8168bc6260f6f87c8cec9fc63c764059cd3730273b37c47e'), ('PY', '1e26694f14b2faf4dd51854385ed493afd5f89000edea2d51d042d00aad7eefa')]
    programs=[body for marker,body in after if marker in ('PY','ROOT_CHECK')]
    assert len(programs)>10
    for body in programs: ast.parse(body)


def root_check(root,base,*protected):
    programs=dict(heredocs(SHELL.read_text()))
    old=sys.argv
    try:
        sys.argv=['root-check',str(root),str(base),*(str(path) for path in protected)]
        exec(compile(programs['ROOT_CHECK'],'<retained-root-check>','exec'),{'__name__':'__main__'})
    finally: sys.argv=old


@pytest.fixture
def invocation(tmp_path):
    base=Path('/tmp').resolve(strict=True)
    with tempfile.TemporaryDirectory(prefix='iroha-shell-source-test.',dir=base) as value:
        root=Path(value);root.chmod(0o700)
        yield root,base,(tmp_path/'candidate',tmp_path/'bootstrap',tmp_path/'cargo')


def test_parent_root_check_accepts_exact_owned_directory_and_preserves_output(invocation):
    root,base,protected=invocation
    (root/'retained').write_bytes(b'original output')
    root_check(root,base,*protected)
    assert (root/'retained').read_bytes()==b'original output'
    assert stat.S_IMODE(root.stat().st_mode)==0o700


@pytest.mark.parametrize('kind',['mode','root_symlink','base_symlink','overlap_root','overlap_parent','overlap_child'])
def test_parent_root_check_rejects_metadata_alias_and_overlap(invocation,tmp_path,kind):
    root,base,protected=invocation
    checked_root=root;checked_base=base
    if kind=='mode': root.chmod(0o755)
    if kind=='root_symlink':
        checked_root=tmp_path/'root-alias';checked_root.symlink_to(root,target_is_directory=True)
    if kind=='base_symlink':
        checked_base=tmp_path/'base-alias';checked_base.symlink_to(base,target_is_directory=True)
    if kind=='overlap_root': protected=(root,)
    if kind=='overlap_parent': protected=(base,)
    if kind=='overlap_child': protected=(root/'child',)
    try:
        with pytest.raises(SystemExit): root_check(checked_root,checked_base,*protected)
    finally: root.chmod(0o700)


def test_parent_root_allocation_is_removed_only_from_outer_release():
    source=SHELL.read_text();initial,preparation,_=scopes(source)
    assert 'mkdtemp' in initial # Preserve existing independent no-channel PR path.
    assert 'mkdtemp' not in preparation
    assert 'release_invocation_base=/private/tmp' not in preparation
    assert 'canonical_path "$IROHA_RELEASE_INVOCATION_ROOT"' in preparation
    assert 'canonical_path "$IROHA_RELEASE_TEMP_BASE"' in preparation
    assert 'readonly release_invocation_root release_invocation_base' in preparation


def test_unrelated_source_inventories_and_runner_trust_assertions_preserved():
    source=SHELL.read_text()
    start='source "${repo_root}/scripts/sumeragi_v2_release_process_policy.sh"\n'
    current=region(source,start,'# The original protected parent runs the complete source-bound scaling preflight')
    assert hashlib.sha256(current.encode()).hexdigest()=='20f24456e531d1bf2e34d3443cadc1598ddf5699b5b87df9979baa39968d6642'
    assert '--runner-timeout-seconds' not in source and '_MAX_RUNNER_OUTPUT_BYTES' not in source
    assert source.count('run_release_formal_gate\n')==1
    assert '--g4p-completion "$multilane_four_peer_completion_path"' in source
    assert '--g12-seed-completion "$nexus_cross_completion_path"' in source
    assert '--g12-fault-soak-completion "$nexus_cross_soak_completion_path"' in source
    assert '"$release_bootstrap_evidence_dir/validate-receipt.py"' in source
    assert '--verify-existing' in source and '--validation-ack' in source
    assert '--publish-validation-failure' in source and '--seal-release-result' in source
    assert source.count('--copy-private-bundle')==0
    assert source.count('--verify-private-bundle')==0


def test_current_shell_contract_has_no_private_packet_prerequisites():
    source=SHELL.read_text()
    assert '"$sealed_repo_root/scripts/run_sumeragi_v2_release_gates.sh" --release' in source
    assert 'readonly release_runner_support_sha256=' in source
    assert '--scaling-execution-record "$release_scaling_execution_record"' in source
    assert '--expected-scaling-execution-sha256 "$release_scaling_execution_sha256"' in source


def test_bound_support_digest_matches_exact_migrated_bytes():
    support=ROOT/'scripts/run_sumeragi_v2_release_gates_support.sh'
    digest=hashlib.sha256(support.read_bytes()).hexdigest()
    assert f'readonly release_runner_support_sha256="{digest}"' in SHELL.read_text()


def test_support_rejects_channel_and_retired_input_presence_in_sealed_child():
    source=(ROOT/'scripts/run_sumeragi_v2_release_gates_support.sh').read_text()
    validation=region(source,'verify_release_identity() {\n','  local observed_target expected_target\n')
    rejection=validation[validation.index('  local scaling_input_name\n'):]
    assert '"${!scaling_input_name+x}"' in rejection and 'return 1' in rejection
    for name in NEW_BINDINGS+OLD_BINDINGS:
        assert validation.count(name)==1 and name in rejection
    assert 'observed_scaling_manifest' not in source and 'scaling_evidence.json' not in source
    assert 'local scaling_digest_name' not in source
    assert '[[ "$profile" == "--release" ]] || return 0' in validation


def test_unrelated_support_checks_and_helpers_match_the_current_source_contract():
    after=(ROOT/'scripts/run_sumeragi_v2_release_gates_support.sh').read_text()
    start=after.index('  local scaling_input_name\n');end=after.index('  local observed_target expected_target\n',start)
    remainder=after[:start]+after[end:]
    assert hashlib.sha256(remainder.encode()).hexdigest()=='dcbe01562b27b7b4c41a4a6b8aeaf5ef7c68371ac48ba76ec247510656732126'



def _ci_parent_scaling_contract():
    """Execute the CI-owned parent contract without running Cargo or the corridor."""
    source = (ROOT / 'ci/check_sumeragi_v2_multilane_release_inventory.sh').read_text()
    start = source.index('def require_parent_scaling_handoff(')
    end = source.index('\nrequire_parent_scaling_handoff(runner_parent_source)', start)
    tree = ast.parse(source[start:end])
    assert len(tree.body) == 1 and isinstance(tree.body[0], ast.FunctionDef)

    def reject(message):
        raise ValueError(message)

    namespace = {'reject': reject}
    exec(compile(tree, '<actual-ci-parent-scaling-contract>', 'exec'), namespace)
    return namespace['require_parent_scaling_handoff']


def test_ci_parent_scaling_contract_accepts_current_parent_handoff():
    _ci_parent_scaling_contract()(SHELL.read_text())


@pytest.mark.parametrize('old,new', [
    ('  )\n  sealed_status=$?\n  set -e\n', '  )\n  sealed_status=0\n  set -e\n'),
    ('  )\n  sealed_status=$?\n  set -e\n', '  ) &\n  sealed_status=$?\n  set -e\n'),
    ('  if ((sealed_status == 0)); then\n    child_result_fields="$(\n',
     '  if ((sealed_status != 0)); then\n    child_result_fields="$(\n'),
    ('nexus_cross_soak_completion_path child_result_extra', 'different_soak child_result_extra'),
    ('"$(sha256_file "$release_scaling_handoff_helper")" != "$release_gate_helper_sha256"', '1 != 1'),
    ('  if ((sealed_status == 0)); then\n    set +e\n', '  if ((sealed_status != 0)); then\n    set +e\n'),
    ('--challenge "$release_gate_challenge"', '--challenge "wrong"'),
    ('sealed_status=$release_scaling_handoff_status', 'sealed_status=0'),
    ('  exec {release_gate_fd}<&-\n', '  :\n'),
    ('source identity changed during the parent-owned scaling experiment', 'source postcheck removed'),
    ('scaling-execution.json"\n', 'foreign-record.json"\n'),
    ('  exit "$sealed_status"\nfi\n', '  exit 0\nfi\n'),
    ('after G-12P two-hour rotating-validator fault soak', 'without G-12P fault soak'),
    ('release_gate_boundary "child-result:before-publication" || exit $?',
     'release_gate_boundary "unrelated-result:before-publication" || exit $?'),
    ('        echo "source identity changed during the parent-owned scaling experiment" >&2\n        sealed_status=1\n', '        echo "source identity changed during the parent-owned scaling experiment" >&2\n        sealed_status=0\n'),
    ('--root "$release_checked_root" --release-identity-json)" != "$release_expected_identity"', '--root "$release_checked_root" --release-identity-json)" == "$release_expected_identity"'),
    ('    for release_checked_root in "$repo_root" "$sealed_repo_root"; do\n', '    for release_checked_root in "$repo_root"; do\n'),
])
def test_ci_parent_scaling_contract_rejects_broken_completion_binding(old, new):
    source = SHELL.read_text()
    assert source.count(old) == 1
    with pytest.raises(ValueError):
        _ci_parent_scaling_contract()(source.replace(old, new))


def test_ci_parent_scaling_contract_rejects_early_repeated_or_retired_handoff():
    check = _ci_parent_scaling_contract()
    source = SHELL.read_text()
    closed = '  exec {release_gate_fd}<&-\n  release_gate_active=0\n'
    publication = 'release_gate_boundary "child-result:before-publication" || exit $?\n'
    soak = '  verify_release_identity "after G-12P two-hour rotating-validator fault soak"\n'
    for changed in (
        source.replace(closed, '').replace('  unset -f release_prepare_without_gate\n', closed + '  unset -f release_prepare_without_gate\n'),
        source.replace(publication, '').replace(soak, publication + soak),
        source + '\n--gate-fd "$release_gate_fd"\n',
        source + '\nrun_release_scaling_gate\n',
        source + '\nvalidate_multilane_scaling_evidence.py\n',
        source + '\nrun_release_scaling_and_formal_gates\n',
    ):
        with pytest.raises(ValueError):
            check(changed)
