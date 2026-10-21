"""One retained four-validator owner through a new-anchor genuine monetary campaign.

This is an explicit expensive engineering controller, not release authority. All
three executable capture roles require independently selected compiler-closure reviews.
The Rust producers independently authenticate every catalog, proof and wallet.
No old anchor, fresh-wallet retry, hidden process signal or component-network swap
is supported. A failed invocation preserves originals and requests ordinary owned
shutdown through DATA; recovery requires separate custody reconciliation.
"""
from pathlib import Path
import argparse
import datetime
import fcntl
import hashlib
import json
import os
import re
import stat
import subprocess
import sys
import time
import uuid

# Resolve before importing any runtime helper; the manifest then authenticates
# paths.py and all other source bytes in this exact enclosing checkout.
_BOOT_ROOT = next((parent for parent in Path(__file__).resolve(strict=True).parents
                   if (parent / 'Cargo.toml').is_file() and (parent / '.git').exists()), None)
if _BOOT_ROOT is None:
    raise SystemExit('acceptance runtime must be inside its Git Cargo checkout')
_RUNTIME_LOCAL = {
    'run.py', 'source_admission.py', 'custody.py', 'native_data.py', 'native_catalog.py',
    'native_capture.py', 'sealed_data.py', 'catalog_policy.py', 'protocol.py', 'paths.py',
    'raw_reuse.py', 'raw_reuse_policy.py', 'compiler_capture.py', 'capture.py',
    'source_audit.py', 'monetary_depfile.py', 'capture_build.py', 'child_process.py',
}
_RUNTIME_EXTERNAL = {
    'scripts/audit_native_source_inputs.py', 'scripts/native_sdk_source_custody.py',
    'scripts/kagemusha_qualify.py',
}


def bootstrap_bytes(path, cap=8 << 20):
    # Runs before any project helper import. The decoded manifest and its digest
    # come from this same bounded read; aliases and changing originals refuse.
    path = Path(path)
    if not path.is_absolute() or path.resolve(strict=True) != path:
        raise ValueError('aliased runtime original')
    before = path.lstat()
    if not stat.S_ISREG(before.st_mode) or not 0 < before.st_size <= cap:
        raise ValueError('runtime original extent')
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
    try:
        first = os.fstat(fd)
        with os.fdopen(fd, 'rb', closefd=False) as stream:
            raw = stream.read(cap + 1)
        last = os.fstat(fd)
    finally:
        os.close(fd)
    fields = lambda row: (row.st_dev, row.st_ino, row.st_mode, row.st_size,
                          row.st_mtime_ns, row.st_ctime_ns)
    if not (fields(before) == fields(first) == fields(last) == fields(path.lstat())
            and len(raw) == before.st_size):
        raise ValueError('runtime original changed during read')
    return raw


def authenticate_runtime(path, expected, root=_BOOT_ROOT):
    if type(expected) is not str or not re.fullmatch(r'[0-9a-f]{64}', expected):
        raise ValueError('externally selected controller manifest digest required')
    raw = bootstrap_bytes(path)
    if hashlib.sha256(raw).hexdigest() != expected:
        raise ValueError('selected controller manifest changed')
    def unique(rows):
        value = {}
        for key, item in rows:
            if key in value:
                raise ValueError('duplicate runtime manifest field')
            value[key] = item
        return value
    manifest = json.loads(raw, object_pairs_hook=unique)
    runtime = manifest['runtime']
    if (manifest['schema'] != 'iroha.kagemusha.network-acceptance-runtime.v1'
            or set(runtime) != {'local', 'external'}
            or set(runtime['local']) != _RUNTIME_LOCAL
            or set(runtime['external']) != _RUNTIME_EXTERNAL):
        raise ValueError('exact controller runtime inventory required')
    pins = {str(path): expected}
    for base, rows in ((path.parent, runtime['local']), (root, runtime['external'])):
        for name, digest in rows.items():
            if type(digest) is not str or not re.fullmatch(r'[0-9a-f]{64}', digest):
                raise ValueError('canonical runtime digest required')
            source = base / name
            if hashlib.sha256(bootstrap_bytes(source)).hexdigest() != digest:
                raise ValueError('frozen runtime helper changed: ' + name)
            pins[str(source)] = digest
    return pins


def install_checked_source_imports(pins):
    """Execute authenticated source bytes for every checkout Python runtime helper."""
    import importlib.abc
    import importlib.machinery
    class CheckedLoader(importlib.abc.Loader):
        def __init__(self, path, digest):
            self.path, self.digest = path, digest
        def create_module(self, _spec):
            return None
        def exec_module(self, module):
            raw = bootstrap_bytes(self.path)
            if hashlib.sha256(raw).hexdigest() != self.digest:
                raise ValueError('runtime source changed before execution')
            module.__file__ = str(self.path)
            exec(compile(raw, str(self.path), 'exec'), module.__dict__)
    class CheckedFinder(importlib.abc.MetaPathFinder):
        def find_spec(self, fullname, path=None, target=None):
            spec = importlib.machinery.PathFinder.find_spec(fullname, path, target)
            if spec is not None and spec.origin and spec.origin.endswith('.py'):
                source = Path(spec.origin)
                if source.is_relative_to(_BOOT_ROOT):
                    if str(source) not in pins:
                        raise ValueError('unselected runtime source: ' + str(source))
                    spec.loader = CheckedLoader(source, pins[str(source)])
            return spec
    finder = CheckedFinder()
    sys.meta_path.insert(0, finder)
    return finder


_AUTHENTICATED_RUNTIME = None
if __name__ == '__main__':
    if sys.argv[1:] in (['--help'], ['-h']):
        print(__doc__)
        print('usage: python3 -I -B -S run.py MANIFEST_SHA256 SELECTION SELECTION_SHA256 FRESH_CAMPAIGN')
        print('Prebuilt source-reviewed native executables only. Linux required. No automatic recovery or retry.')
        raise SystemExit(0)
    if sys.flags.optimize or not sys.dont_write_bytecode or not sys.flags.no_site or not sys.flags.isolated:
        raise SystemExit('Use isolated unoptimized Python -I -B -S for the selected controller.')
    if len(sys.argv) != 5:
        raise SystemExit('Expected MANIFEST_SHA256 SELECTION SELECTION_SHA256 CAMPAIGN.')
    _AUTHENTICATED_RUNTIME = authenticate_runtime(
        Path(__file__).resolve().with_name('manifest.json'), sys.argv[1])
    install_checked_source_imports(_AUTHENTICATED_RUNTIME)
    sys.path.insert(0, str(Path(__file__).resolve().parent))

import source_admission as admission
ROOT = admission.ROOT
QUAL = ROOT / 'target/qualification'
import custody as bounded
import native_data as component
import native_catalog
import native_capture
import compiler_capture
import child_process
import sealed_data
import protocol

if _AUTHENTICATED_RUNTIME is not None:
    compiler_capture.AUTHENTICATED_RUNTIME = dict(_AUTHENTICATED_RUNTIME)

if sys.flags.optimize:
    raise SystemExit('optimized Python is unsupported')
CATALOG_POLICY = native_catalog.CATALOG_POLICY

SCHEMA = 'iroha.kagemusha.real-network-campaign-selection.v1'
OWNER = 'kagemusha_network_setup::continuation::execute_four_validator_monetary_continuation'
ISSUER = 'kagemusha_network_setup::continuation::issuer::'
ISSUER_COMPOSER = ISSUER + 'provisioning::prepare_same_live_engineering_issuer_configuration'
NATIVE = 'kagemusha_wallet_advance_v1::enrollment_owner_tests::native_exchange::'
CATALOG = 'kagemusha_wallet_artifacts_v1::producer_inventory::compiler::full_catalog::complete_wallet_catalog_from_pinned_genesis'
TARGET = NATIVE + 'export_actual_a_load_target_with_retained_native_custody'
RETENTION = NATIVE + 'load::activation_retention::retain_actual_a_activation_before_network_submission'
LOAD = NATIVE + 'load::export_native_load_finality_from_executed_history'
EXCHANGE = NATIVE + 'actual_native_a_to_b_to_c_then_unload_with_restart_and_replay'
CONFIRM = NATIVE + 'network_settlement::actual_c_confirms_network_unload_after_process_restart'
PREFLIGHT_TESTS = (('sumeragi', OWNER), ('sumeragi', ISSUER_COMPOSER)) + tuple(
    ('iroha_core_zk', test) for test in (CATALOG, TARGET, RETENTION, LOAD, EXCHANGE, CONFIRM))
HELPERS = (
    'kagemusha_network_setup::registration_incarnation_uses_lifecycle_ordinal_not_instruction_position',
    'kagemusha_network_setup::provisioning::tests::native_provisioning_binds_the_actual_genesis_owner_and_rejects_foreign_keys',
    'kagemusha_network_setup::provisioning::tests::phase_requires_all_four_exact_predecessors',
    'kagemusha_network_setup::provisioning::tests::provider_binding_refuses_foreign_seat_or_inventory',
    'kagemusha_network_setup::continuation::settlement::policy::policy_authority_requires_exact_current_genesis_owner',
    'kagemusha_network_setup::continuation::settlement::policy::policy_failure_requires_exact_reserve_asset_revision_and_direction',
    'kagemusha_network_setup::continuation::selected_original_reads_are_bounded_private_and_pinned',
    'kagemusha_network_setup::continuation::inventory_refuses_duplicate_missing_and_unknown_roles',
    'kagemusha_network_setup::continuation::abort_requires_exact_setup_and_failure_pins',
    'kagemusha_network_setup::continuation::settlement::rejection_status_requires_exact_global_state_and_certified_height',
    'kagemusha_network_setup::continuation::settlement::claim_failure_requires_typed_instruction_invariant_and_retains_exact_reason',
    'kagemusha_network_setup::continuation::settlement::canonical_observation_retains_exact_bytes_and_refuses_oversized_or_replaced_original',
    ISSUER + 'tests::issuer_command_is_closed_and_selects_one_of_four_existing_peers',
    ISSUER + 'tests::issuer_layer_cannot_change_topology_storage_or_registered_selection',
    ISSUER + 'tests::issuer_configuration_requires_exact_private_original_and_rechecks_its_pin',
    ISSUER + 'provisioning::tests::issuer_composition_refuses_cross_occurrence_handoffs_and_wrong_destination',
    ISSUER + 'provisioning::tests::issuer_certificate_is_rooted_enrollment_only_and_foreign_root_is_refused',
    ISSUER + 'provisioning::tests::issuer_platform_uses_selected_originals_and_preserves_default_off_controls',
    ISSUER + 'provisioning::tests::selected_inputs_require_exact_bounded_bytes_and_private_custody',
    ISSUER + 'provisioning::tests::configuration_preserves_canonical_policies_worker_and_optional_tls_selection',
    ISSUER + 'provisioning::tests::selection_is_published_only_after_originals_and_never_overwrites',
    ISSUER + 'provisioning::tests::provisioning_request_has_closed_fields_and_explicit_worker_selection',
    ISSUER + 'provisioning::tests::issuer_output_parent_is_bound_to_current_qualification_before_creation',
    ISSUER + 'provisioning::tests::issuer_output_refuses_external_and_traversing_parent_before_creation',
    ISSUER + 'provisioning::tests::issuer_output_refuses_non_child_names_and_preserves_existing_custody',
    ISSUER + 'provisioning::tests::issuer_output_refuses_aliased_parent_and_dangling_output',
    ISSUER + 'provisioning::scopes::tests::scope_preimages_match_native_roles_and_known_answers',
    ISSUER + 'provisioning::scopes::tests::scope_originals_require_matching_pins_and_retained_custody',
    ISSUER + 'provisioning::scopes::tests::scope_text_and_origin_reject_noncanonical_preimages_without_rewriting',
    ISSUER + 'provisioning::scopes::tests::scope_release_is_exact_opaque_bounded_data',
)
NATIVE_HELPERS = (
    NATIVE + 'network_settlement::observed_network_inventory_rejects_missing_duplicate_extra_and_invented_heights',
    NATIVE + 'source_reuse::tests::first_target_selection_requires_current_pins_and_distinct_data_schema',
    'kagemusha_wallet_artifacts_v1::producer_inventory::compiler::full_catalog::fresh_original_budget_counts_only_new_bytes_and_refuses_before_publication',
)
SOURCE_PINS = component.SOURCE_PINS
require, pin = component.require, component.pin
JSON_CAP = 64 << 20
ISSUER_REQUEST_CAP = 64 << 10  # Native provisioning::INPUT_MAX.
_UNFINISHED_CHILDREN = []


def local(value):
    path = Path(value)
    require(path.is_absolute() and path.is_relative_to(QUAL) and '..' not in path.parts
            and path.resolve() == path, 'absolute original-checkout target path without aliases required')
    return path


def observe(path, cap=JSON_CAP):
    path = local(path)
    record = bounded.inspect(path, cap)
    return {'path': str(path), 'sha256': record['sha256'], 'bytes': record['bytes']}


def ref(item):
    return {key: item[key] for key in ('path', 'sha256')}


def selected(item, cap=JSON_CAP):
    require(type(item) is dict and set(item) == {'path', 'sha256'}, 'exact path/hash selection required')
    path = local(item['path'])
    require(observe(path, cap)['sha256'] == pin(item['sha256']), 'selected original changed')
    return path


def selected_tool(item, cap=8 << 20):
    """Hash a selected maintained or retained review original without executing it."""
    require(type(item) is dict and set(item) == {'path', 'sha256'}, 'exact tool path/hash selection required')
    path = Path(item['path'])
    require(path.is_absolute() and path.resolve(strict=True) == path
            and (path.is_relative_to(ROOT / 'scripts') or path.is_relative_to(QUAL)),
            'maintained or retained review original required')
    require(hashlib.sha256(bootstrap_bytes(path, cap)).hexdigest() == pin(item['sha256']),
            'selected review original changed')
    return path


def document(item, cap=JSON_CAP):
    path = selected(item, cap)
    return component.selected_json(path, item['sha256'], cap)


def read_document(path, cap=JSON_CAP):
    return document(ref(observe(path, cap)), cap)


def save(path, value):
    # Only create-new evidence. File and directory durability precede a phase command.
    bounded.json_file(path, value)
    bounded.sync_directory(path.parent)
    return observe(path)


def private(path):
    local(path).mkdir(mode=0o700, exist_ok=False)


def atomic_command(path, value):
    # Single controller owns the flock for this campaign. A partial temporary is
    # never a command, and an existing published command is never overwritten.
    require(not path.exists(), 'phase command already exists; reconciliation required')
    temporary = path.with_name('.' + path.name + '.pending')
    save(temporary, value)
    require(not path.exists(), 'phase was concurrently published')
    os.rename(temporary, path)
    bounded.sync_directory(path.parent)
    return observe(path)


def private_selected(item, cap=JSON_CAP):
    """Select opaque runtime originals without decoding or retaining secret contents."""
    path = selected(item, cap)
    file, parent = path.lstat(), path.parent.lstat()
    require(file.st_uid == parent.st_uid == os.geteuid()
            and stat.S_ISDIR(parent.st_mode) and not parent.st_mode & 0o077
            and file.st_nlink == 1 and not file.st_mode & 0o077,
            'owner-only original and parent required')
    return path


def phase_command(root, phase, setup, previous, **inputs):
    value = protocol.command(phase, setup, previous, inputs,
                             private_selected if phase == 'issuer' else selected)
    return atomic_command(root / (phase + '-command.json'), value)


def await_natural_exit(child, directory, deadline):
    """Observe exit only; a timeout preserves a possibly live child and forbids retry."""
    try:
        return child.wait(timeout=max(0, deadline - time.monotonic()))
    except BaseException as error:
        # Never let evidence I/O failure erase the known live/uncertain child.
        _UNFINISHED_CHILDREN.append({'pid': getattr(child, 'pid', None), 'directory': str(directory)})
        try:
            save(directory / 'unfinished-child.json', {
                'pid': getattr(child, 'pid', None), 'state': 'unfinished_or_uncertain',
                'signals_permitted': False, 'automatic_retry_permitted': False,
                'custody_reconciliation_required': True, 'observation_error': type(error).__name__})
        except BaseException:
            pass
        raise


def retain_failure(campaign, error):
    """Preserve uncertain compiler custody before fallible failure-record I/O."""
    if isinstance(error, child_process.UnfinishedChild):
        _UNFINISHED_CHILDREN.append({'pid': error.pid, 'command': error.argv,
                                    'directory': str(campaign), 'role': 'compiler_custody'})
    try:
        return save(campaign / 'failure.json', {
            'error': str(error), 'accepted': False, 'custody_reconciliation_required': True})
    except BaseException:
        if not _UNFINISHED_CHILDREN:
            raise
        return None  # The in-memory custody guard still prohibits more work.


def request_owned_abort(live, setup, failure, child):
    if child.poll() is not None:
        return None
    if setup is None and (live / 'setup/setup.json').exists():
        # Shutdown selector only: a broken ready record never grants proof or value.
        setup = ref(observe(live / 'setup/setup.json', 16 << 10))
    if setup is None:
        return None  # Owner is still before setup or will return its startup failure.
    return atomic_command(live / 'abort-command.json', {
        'schema': 'iroha.kagemusha.real-network-abort.v1',
        'setup_sha256': setup['sha256'], 'failure_sha256': failure['sha256']})


def release_command(command):
    require(type(command) is list and command and all(type(arg) is str for arg in command)
            and '--release' in command and 'test' in command and '--no-run' in command,
            'owner producer command must be one optimized no-run test build')
    return command


def compile_selection(selection):
    require(set(selection) in ({'schema', 'captures'}, {'schema', 'captures', 'reuse'})
            and selection['schema'] == SCHEMA, 'campaign selection grammar')
    require(set(selection['captures']) == {'stock', 'owner', 'native'}, 'three exact producer roles required')
    binaries, source, retained, captures = {}, {}, {}, {}
    reuse = None
    if 'reuse' in selection:
        selected(selection['reuse'], 32 << 20)
        reuse = native_catalog.REUSE.admit(selection['reuse'])
        retained[selection['reuse']['path']] = selection['reuse']['sha256']
    required = {'stock': {'iroha3d', 'iroha'}, 'owner': {'sumeragi'}, 'native': {'iroha_core_zk'}}
    native_owner = None
    for role, item in selection['captures'].items():
        if role == 'native':
            owner, source_digest = native_capture.admit(item)
            for name, digest in owner['source'].items():
                require(name not in source or source[name] == digest, 'shared consumed source differs: ' + name)
                source[name] = digest
            retained.update(owner['retained'])
            binaries['iroha_core_zk'] = dict(owner['binary'], source_sha256=source_digest)
            native_owner = owner
            captures[role] = {'classification': owner['classification'], 'heads': owner['heads'], 'selected': item}
            continue
        result, inventory, pins, custody = admission.admit_selection(item, required[role])
        require(result['profile'] == 'release', 'optimized current phase executables required')
        if role == 'owner':
            release_command(result['command'])
        for name, digest in pins.items():
            require(name not in source or source[name] == digest, 'shared consumed source differs: ' + name)
            source[name] = digest
        retained.update(custody['reviewed_originals'])
        require(set(result['binaries']) == required[role], 'one exact producer inventory required')
        source_digest = result['source_before_sha256']
        for name, binary in result['binaries'].items():
            require(name not in binaries, 'one source owner per executed producer required')
            retained[binary['path']] = binary['sha256']
            binaries[name] = dict(binary, source_sha256=source_digest)
        captures[role] = {'classification': result['classification'],
                          'heads': [custody['before_head'], custody['after_head']],
                          'selected': item, 'custody': custody}
    require(native_owner is not None, 'current native source owner required')
    return {'binaries': binaries, 'source': source, 'retained': retained, 'captures': captures,
            'native_owner': native_owner, 'raw_reuse': reuse}


def tools():
    paths = [Path(__file__).resolve().parent / name for name in sorted(_RUNTIME_LOCAL)]
    paths += [ROOT / name for name in sorted(_RUNTIME_EXTERNAL)]
    paths += [Path(__file__).resolve().with_name('manifest.json'), Path(sys.executable).resolve()]
    return {str(path): admission.sha(path) for path in paths}


def unchanged(admitted, selected_tools):
    admission.capture.require_resolved_checkout()
    for name, digest in admitted['source'].items():
        require(admission.sha(ROOT / name) == digest, 'current consumed source differs: ' + name)
    for name, digest in (admitted['retained'] | selected_tools).items():
        require(admission.sha(Path(name)) == digest, 'retained source review/tool changed: ' + name)
    for binary in admitted['binaries'].values():
        require(admission.sha(Path(binary['path'])) == binary['sha256'], 'captured executable changed')
    for role, capture in admitted.get('captures', {}).items():
        if role != 'native':
            admission.check_runtime_custody(capture['custody'])
    if 'native_owner' in admitted:
        native_capture.unchanged(admitted['native_owner'], selected_tools)
    if admitted.get('raw_reuse') is not None:
        output = admitted.get('raw_reuse_output')
        native_catalog.REUSE.check(admitted['raw_reuse'], hash_bytes=False,
                                   output_originals=None if output is None else Path(output))


def wait_document(path, child, deadline):
    while not path.exists():
        require(child.poll() is None, 'same live network owner exited before ' + path.name)
        require(time.monotonic() < deadline, 'controller deadline; partial custody must be reconciled')
        time.sleep(1)
    item = observe(path)
    require(child.poll() is None or path.name == 'completed.json', 'owner exited during stage handoff')
    return item, document(ref(item))


def exact_inventory(test, stdout, stderr, code):
    require(type(code) is int and code == 0 and not stderr.strip(), 'compiled test inventory failed')
    require([line for line in stdout.splitlines() if line.strip()] ==
            [test + ': test', '1 test, 0 benchmarks'], 'exact qualified test absent: ' + test)


def exact_result(test, stdout, code):
    normalized = stdout.replace(f'test {test} has been running for over 60 seconds\n', '')
    # --nocapture permits subject stdout between the selector prefix and its
    # terminal `ok`. Match exactly one selector and one completed summary; do not
    # mistake arbitrary output from another test or a zero-test run for success.
    prefixes = re.findall(r'^test ([^\n]+?) \.\.\. ', normalized, re.M)
    if prefixes != [test] or not component.summaries_ok(code, normalized):
        return False
    prefix = 'test ' + test + ' ... '
    suffix = normalized.split(prefix, 1)[1]
    terminal, separator, _summary = suffix.rpartition('test result: ')
    terminal_ok = terminal.strip() == 'ok' or bool(re.search(r'\nok\s*$', terminal))
    return bool(separator and terminal_ok) and len(re.findall(r'^running 1 test$', normalized, re.M)) == 1


def retained_listing(binary, test, directory, stem, extra, env):
    """Retain one prebuilt native census without subprocess.run's interrupt kill."""
    command = [binary['path'], test, '--exact', '--list', *extra]
    with (directory / (stem + '.stdout')).open('x') as output, \
            (directory / (stem + '.stderr')).open('x') as error:
        child = subprocess.Popen(command, cwd=ROOT, env=env, stdin=subprocess.DEVNULL,
                                 stdout=output, stderr=error)
        try:
            save(directory / (stem + '-process.json'), {'pid': child.pid, 'command': command,
                 'binary': binary, 'signals_permitted': False})
        finally:
            code = await_natural_exit(child, directory, time.monotonic() + 60)
            output.flush(); os.fsync(output.fileno())
            error.flush(); os.fsync(error.fileno())
            save(directory / (stem + '-exit.json'), {'exit': code})
    stdout = component.retained_text(directory / (stem + '.stdout'))
    stderr = component.retained_text(directory / (stem + '.stderr'))
    exact_inventory(test, stdout, stderr, code)


def preflight(binaries, directory, env, admitted, selected_tools):
    """List required ignored entry points without executing their native work."""
    unchanged(admitted, selected_tools)
    for number, (binary, test) in enumerate(PREFLIGHT_TESTS):
        for stem, extra in [('list', []), ('ignored-list', ['--ignored'])]:
            retained_listing(binaries[binary], test, directory, f'preflight-{number}-{stem}', extra, env)
    unchanged(admitted, selected_tools)


def exact_test(binary, test, directory, env, admitted, selected_tools, ignored=True):
    private(directory)
    unchanged(admitted, selected_tools)
    command = [binary['path'], test, '--exact'] + (['--ignored'] if ignored else []) + ['--nocapture', '--test-threads=1']
    for stem, extra in [('list', [])] + ([('ignored-list', ['--ignored'])] if ignored else []):
        retained_listing(binary, test, directory, stem, extra, env)
    start = save(directory / 'started.json', {'command': command, 'binary': binary,
        'environment': {key: value for key, value in env.items() if key.startswith(('KAGEMUSHA_', 'TMP'))},
        'tools': selected_tools, 'utc': datetime.datetime.now(datetime.timezone.utc).isoformat()})
    timer = time.monotonic()
    with (directory / 'run.log').open('x') as log, (directory / 'run.stderr').open('x') as stderr:
        child = subprocess.Popen(command, cwd=ROOT, env=env, stdin=subprocess.DEVNULL, stdout=log, stderr=stderr)
        try:
            save(directory / 'process.json', {'pid': child.pid, 'command': command, 'signals_permitted': False})
        finally:
            code = await_natural_exit(child, directory, time.monotonic() + 7 * 86400)
            save(directory / 'exit.json', {'exit': code})
        log.flush(); os.fsync(log.fileno())
        stderr.flush(); os.fsync(stderr.fileno())
    summary = exact_result(test, component.retained_text(directory / 'run.log'), code)
    error = None
    try:
        unchanged(admitted, selected_tools)
    except (ValueError, OSError) as exc:
        error = str(exc)
    receipt = {'exit': code, 'seconds': time.monotonic() - timer, 'summary_pass': summary,
               'inputs_unchanged': error is None, 'error': error, 'accepted_component': summary and error is None,
               'started_sha256': start['sha256'], 'log_sha256': component.log_pin(directory / 'run.log'),
               'stderr_sha256': component.log_pin(directory / 'run.stderr')}
    save(directory / 'result.json', receipt)
    require(receipt['accepted_component'], 'producer failed or source drifted; retained ' + str(directory))
    return receipt


def compose_issuer(campaign, issuer_request, setup, registered, child, deadline,
                   env, admitted, selected_tools):
    """Own one explicit native composition before consuming its published selection."""
    selection_path = campaign / 'issuer-selection.json'
    require(not os.path.lexists(selection_path), 'pre-existing issuer selection requires reconciliation')
    input_ref, input_value = wait_document(campaign / 'issuer-composer-input.json', child, deadline)
    private_selected(ref(input_ref), 16384)
    request = protocol.issuer_composer_input(
        input_value, ref(issuer_request), lambda item: private_selected(item, ISSUER_REQUEST_CAP))
    # Only this bounded DATA projection is decoded by Python. Native owns the full
    # request grammar, policy and private originals. Never retain its body in receipts.
    try:
        request_data = document(request, ISSUER_REQUEST_CAP)
    except (OSError, ValueError):
        raise ValueError('invalid private issuer request DATA') from None
    expected = {'issuer_request': ref(issuer_request), 'setup': ref(setup), 'registered': ref(registered)}
    require(type(request_data) is dict
            and all(request_data.get(name) == item for name, item in expected.items()),
            'issuer request must select the three exact current controller originals')
    del request_data
    originals = {'issuer-composer-input': input_ref, 'issuer-provisioning-request': observe(request['path'], ISSUER_REQUEST_CAP),
                 'issuer-request': issuer_request, 'issuer-setup': observe(setup['path'], ISSUER_REQUEST_CAP),
                 'issuer-registered': registered}
    require(ref(originals['issuer-setup']) == ref(setup), 'current issuer setup changed before invocation')
    for item in originals.values():
        private_selected(ref(item), ISSUER_REQUEST_CAP)
        admitted['retained'][item['path']] = item['sha256']
    require(originals['issuer-provisioning-request']['sha256'] == request['sha256'],
            'selected issuer request changed before invocation')
    require(not os.path.lexists(selection_path), 'pre-existing issuer selection requires reconciliation')
    directory = campaign / 'issuer-composition'
    composer_env = env | {'KAGEMUSHA_ENGINEERING_ISSUER_REQUEST': request['path'],
                          'KAGEMUSHA_ENGINEERING_ISSUER_REQUEST_SHA256': request['sha256']}
    exact_test(admitted['binaries']['sumeragi'], ISSUER_COMPOSER, directory,
               composer_env, admitted, selected_tools)
    # A visible publication is insufficient: exact_test must have observed the
    # named producer's natural success and post-source checks before this point.
    for item in originals.values():
        private_selected(ref(item), ISSUER_REQUEST_CAP)
    require(child.poll() is None, 'same live network owner exited during issuer composition')
    selection = observe(selection_path, 16384)
    private_selected(ref(selection), 16384)
    inputs = protocol.issuer_selection(document(ref(selection), 16384), setup['sha256'],
                                       registered['sha256'], private_selected)
    originals['issuer-selection'] = selection
    for name in ('started.json', 'process.json', 'exit.json', 'result.json'):
        originals['issuer-composition-' + name] = observe(directory / name)
    for name in ('started.json', 'process.json', 'exit.json', 'result.json', 'run.log', 'run.stderr',
                 'list.stdout', 'list.stderr', 'list-process.json', 'list-exit.json',
                 'ignored-list.stdout', 'ignored-list.stderr', 'ignored-list-process.json', 'ignored-list-exit.json'):
        path = directory / name
        admitted['retained'][str(path)] = component.log_pin(path)
    return inputs, originals


def native_environment(admitted, fixture, catalog, setup, pins):
    require(set(pins) == set(SOURCE_PINS), 'exact seven native source pins')
    return {key: value for key, value in os.environ.items() if not key.startswith(('KAGEMUSHA_', 'IROHA_TEST_', 'TEST_NETWORK_'))} | pins | {
        'KAGEMUSHA_SIGNED_GENESIS_FIXTURE': fixture['path'],
        'KAGEMUSHA_WALLET_SIGNED_CATALOG': str(catalog),
        'KAGEMUSHA_NATIVE_LEDGER_SETUP': setup['path'], 'KAGEMUSHA_NATIVE_LEDGER_SETUP_SHA256': setup['sha256'],
        'KAGEMUSHA_NATIVE_SOURCE_SHA256': admitted['binaries']['iroha_core_zk']['source_sha256'],
        'KAGEMUSHA_NATIVE_VERIFIER_PACK': str(catalog / 'engineering-verifier-pack.norito'),
        'KAGEMUSHA_NATIVE_VERIFIER_PACK_SHA256': pins['KAGEMUSHA_WALLET_CATALOG_PACK_SHA256']}


def catalog_from_live_setup(campaign, setup, admitted, selected_tools, env):
    # Only the selected, still-live owner's executed setup can choose this root.
    setup_path = selected(setup)
    value = document(setup)
    originals = component.manifest_rows(setup_path.parent, value['originals'], component.SETUP_NAMES)
    fixture = originals['capture.json']
    native_catalog.setup_fixture(value, originals, component)
    compiler = admitted['binaries']['iroha_core_zk']
    run = campaign / 'catalog'
    catalog = run / 'output'
    storage_budget = sealed_data.capacity(campaign)
    save(campaign / 'catalog-storage-budget.json', storage_budget)
    compiler_env = env | {
        'KAGEMUSHA_WALLET_NEW_RAW_BYTES': str(storage_budget['maximum_new_raw_bytes']),
        'KAGEMUSHA_SIGNED_GENESIS_FIXTURE': fixture['path'],
        'KAGEMUSHA_SIGNED_GENESIS_FIXTURE_SHA256': fixture['sha256'],
        'KAGEMUSHA_WALLET_SOURCE_SHA256': compiler['source_sha256'],
        'KAGEMUSHA_WALLET_CATALOG_OUTPUT': str(catalog)}
    reuse = admitted.get('raw_reuse')
    transport = None
    if reuse is not None:
        # Only raw content-addressed DATA is reused. Genesis and every signed
        # descriptor/install artifact still come from this live, current run.
        native_catalog.REUSE.check(reuse, hash_bytes=False)
        intake = campaign / 'raw-reuse'
        private(intake)
        transport = native_catalog.REUSE.transport(reuse, intake)
        admitted['retained'][transport['path']] = transport['sha256']
        admitted['raw_reuse_output'] = str(catalog / 'originals')
        compiler_env['KAGEMUSHA_WALLET_REUSE_INPUTS'] = transport['path']
    try:
        exact_test(compiler, CATALOG, run, compiler_env, admitted, selected_tools)
        if transport is not None:
            require(CATALOG_POLICY.bounded(Path(transport['path']), 16_384)
                    == CATALOG_POLICY.bounded(catalog / 'reuse-inputs.json', 16_384),
                    'native reuse transport differs or is missing')
    finally:
        # Preserve the real subject's result even on failure. This is the same
        # post-run full content/controlled-hard-link check used by CATALOG_POLICY runner2.
        if reuse is not None:
            try:
                checked = native_catalog.REUSE.check(
                    reuse, hash_bytes=True, output_originals=catalog / 'originals')
            except Exception as failure:
                save(campaign / 'raw-reuse-after.json', {'accepted': False, 'error': repr(failure)})
                raise
            else:
                save(campaign / 'raw-reuse-after.json', {'accepted': True, 'custody': checked})
    outputs, pins = native_catalog.completed(catalog, compiler, fixture, component, CATALOG_POLICY)
    save(campaign / 'new-anchor-catalog.json', {'setup': setup, 'fixture': ref(fixture),
        'catalog_outputs': outputs, 'source_pins': pins, 'recursive_finality_graph': False})
    return fixture, catalog, pins, outputs


def native_negative_records(directory, exchange):
    value = document(ref(exchange), 16 << 10)
    item = value['c_unload_bad_sigma_claim']
    require(type(item) is dict and set(item) == {'name', 'bytes', 'sha256', 'mutation',
            'model_verified', 'native_proof_rejected', 'adversarial_fixture_signatures'}, 'native proof negative grammar')
    require(item['name'] == 'c-unload-bad-sigma-claim.norito' and item['mutation'] == 'sigma_ipa_f_plus_one'
            and item['model_verified'] is True and item['native_proof_rejected'] is True
            and type(item['adversarial_fixture_signatures']) is int and item['adversarial_fixture_signatures'] == 1,
            'native proof negative scope differs')
    require(type(item['bytes']) is int and 0 < item['bytes'] <= 16 << 10, 'native proof negative bound')
    original = observe(directory / item['name'], 16 << 10)
    require(original['sha256'] == pin(item['sha256']) and original['bytes'] == item['bytes'], 'native proof negative hash/extent differs')
    return {item['name']: original}


def negative_originals(directory, value):
    retained = {}; qualified = {}
    for name, field in [('foreign-account', 'foreign_account_observation'), ('bad-sigma', 'altered_proof_observation')]:
        summary = value[field]
        require(type(summary) is dict and set(summary) == {'name', 'transaction_hash_hex', 'qualified', 'observation_original'}
                and summary['name'] == name and type(summary['qualified']) is bool, 'negative case summary differs')
        item = summary['observation_original']
        require(type(item) is dict and set(item) == {'name', 'bytes', 'sha256'}
                and item['name'] == name + '-observation.json'
                and type(item['bytes']) is int and 0 < item['bytes'] <= 16 << 10, 'negative observation bound/grammar')
        original = observe(directory / item['name'], 16 << 10)
        require(original['sha256'] == pin(item['sha256']) and original['bytes'] == item['bytes'], 'negative observation hash/extent differs')
        retained[item['name']] = original
        case = document(ref(original), 16 << 10)
        require(case['name'] == name and case['transaction_hash_hex'] == summary['transaction_hash_hex'], 'negative observation binding differs')
        height = case['committed_failure_height']
        complete = type(height) is int and 2 <= height <= value['last_certified_height']
        require(height is None or complete, 'negative certified height invalid')
        require(case['adversarial_input_rejection_qualified'] is complete
                and case['authoritative_rejection_observed'] is complete and summary['qualified'] is complete,
                'negative qualification differs from certified execution')
        if case['phase'] == 'preparation':
            require(name == 'foreign-account' and not complete and case['transaction_hash_hex'] is None
                    and case['peer_statuses'] == [], 'unsigned preparation has no transaction authority')
            names = {f'{name}-instruction.norito', f'{name}-expected-reason.norito', f'{name}-preparation-error.txt'}
        else:
            require(case['phase'] == 'submitted', 'negative phase differs')
            pin(case['transaction_hash_hex'])
            names = {f'{name}-signed.norito', f'{name}-expected-reason.norito', f'{name}-submission-error.txt'} | {f'{name}-status-{i}.norito' for i in range(4)}
            if complete:
                names |= {f'{name}-committed.norito', f'{name}-reason.norito'}
            require(len(case['peer_statuses']) == 4 and len({row['peer_id'] for row in case['peer_statuses']}) == 4, 'negative status peer count differs')
            for status in case['peer_statuses']:
                response = status['response']; fixed = status['state_rejection_height']
                if response is None:
                    require(fixed is None, 'absent negative status has authority')
                    continue
                require(response['hash'] == case['transaction_hash_hex'] and response['scope'] == 'global'
                        and response['status']['kind'] == 'Rejected' and response['resolved_from'] in {'state', 'cache', 'queue'}, 'negative status selector differs')
                if response['resolved_from'] == 'state':
                    require(complete and fixed == height == response['status']['block_height'], 'negative state height differs')
                else:
                    require(fixed is None, 'cached negative status has authority')
        rows = case['originals']
        require(type(rows) is list, 'negative original inventory required')
        for row in rows:
            require(type(row) is dict and set(row) == {'name', 'bytes', 'sha256'} and type(row['name']) is str, 'negative original row shape')
            cap = (16 << 10) if '-status-' in row['name'] or row['name'].endswith('-reason.norito') else (16 << 20)
            require(type(row['bytes']) is int and 0 < row['bytes'] <= cap, 'negative original role bound')
        retained.update(component.manifest_rows(directory, rows, names))
        qualified[name] = complete
    require(value['native_proof_rejection_qualified'] is qualified['bad-sigma']
            and value['adversarial_qualified'] is qualified['bad-sigma']
            and value['account_specific_network_rejection_qualified'] is qualified['foreign-account']
            and value['account_binding_model_refusal'] is True, 'settlement adversarial coverage differs')
    return retained


def settlement_records(path, env, exchange, funded):
    item = observe(path, 16 << 10); value = document(ref(item), 16 << 10)
    require(value['schema'] == 'iroha.kagemusha.network-unload-settlement.v1', 'network settlement grammar')
    require(value['setup_sha256'] == env['KAGEMUSHA_NATIVE_LEDGER_SETUP_SHA256']
            and value['source_pins'] == {key: env[key] for key in SOURCE_PINS}
            and value['exchange_sha256'] == exchange['exchange']['sha256']
            and value['claim_sha256'] == exchange['claim']['sha256']
            and value['target_sha256'] == env['KAGEMUSHA_NATIVE_LOAD_TARGET_SHA256']
            and value['receipt_sha256'] == funded['receipt_sha256'], 'network settlement binding differs')
    height, through = value['settlement_height'], value['last_certified_height']
    require(type(height) is int and type(through) is int and funded['certified_height'] < height <= through <= 64, 'observed height bounds')
    names = {f'native-proof-{i}.norito' for i in range(1, through + 1)} | {'unload-claim.norito', 'unload-instruction.norito', 'unload-signed-transaction.norito', 'unload-block.norito'}
    originals = component.manifest_rows(path.parent, value['originals'], names)
    originals.update(negative_originals(path.parent, value))
    require(value['exact_transaction_retry_returned_original'] is True, 'retained transport retry absent')
    return item, value, originals



def post_restart_records(live, completed, settled):
    # The current named Rust owner verifies native certified inclusion, exact
    # transport replay and unchanged balances. Join its new signed original to
    # that result; this DATA reader does not independently decode a transaction.
    require(completed.get('post_restart_exact_transaction_retry_returned_original') is True,
            'post-restart exact transport retry absent')
    restarted = completed.get('restarted_height')
    retry = completed.get('post_restart_instruction_retry_height')
    require(type(restarted) is int and type(retry) is int
            and 0 < restarted < retry < (1 << 64),
            'post-restart certified retry height differs')
    fresh_hash = pin(completed.get('post_restart_instruction_retry_hash_hex'))
    require(fresh_hash != pin(settled.get('settlement_transaction_hash_hex')),
            'post-restart instruction retry reused transaction identity')
    expected = pin(completed.get('post_restart_instruction_retry_original_sha256'))
    name = 'post-restart-unload-retry-signed.norito'
    original = observe(live / name, 16 << 20)  # Same ORIGINAL_MAX as the Rust owner.
    require(original['sha256'] == expected, 'post-restart signed original hash differs')
    return {name: original}


def final_verdict(code, log, completed, settled, source_ok, exact_peers, remaining):
    positive = component.summaries_ok(code, log) and source_ok and exact_peers and not remaining
    adversarial = (settled.get('adversarial_qualified') is True and completed.get('adversarial_qualified') is True
                   and settled.get('native_proof_rejection_qualified') is True)
    return {'positive_network_campaign_completed': positive, 'adversarial_qualified': adversarial,
            'native_proof_rejection_qualified': adversarial,
            'account_binding_model_refusal': settled.get('account_binding_model_refusal') is True,
            'account_specific_network_rejection_qualified': settled.get('account_specific_network_rejection_qualified') is True,
            'all_named_network_rejections_qualified': adversarial and settled.get('account_specific_network_rejection_qualified') is True,
            'accepted_engineering_campaign': positive and adversarial, 'release_qualified': False,
            'physical_device_qualified': False}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('manifest_sha256')
    parser.add_argument('selection', type=Path); parser.add_argument('selection_sha256')
    parser.add_argument('campaign', type=Path)
    args = parser.parse_args()
    require(sys.platform.startswith('linux'), 'current native issuer owner requires Linux; no component substitution')
    require(_AUTHENTICATED_RUNTIME is not None, 'direct imported main is not an admitted launcher')
    require(_AUTHENTICATED_RUNTIME == authenticate_runtime(
        Path(__file__).resolve().with_name('manifest.json'), args.manifest_sha256),
        'controller runtime changed after import')
    os.umask(0o077)
    require(Path.cwd() == ROOT, 'run from original checkout')
    admission.capture.require_resolved_checkout()
    require(not any(key.startswith(('KAGEMUSHA_', 'IROHA_TEST_', 'TEST_NETWORK_', 'IROHA_RELEASE_')) for key in os.environ), 'ambient campaign controls refused')
    base_env = CATALOG_POLICY.inherited_environment(os.environ)
    selection_ref = {'path': str(args.selection), 'sha256': pin(args.selection_sha256)}
    selection = document(selection_ref); admitted = compile_selection(selection); selected_tools = tools() | _AUTHENTICATED_RUNTIME
    admitted['retained'][selection_ref['path']] = selection_ref['sha256']
    unchanged(admitted, selected_tools)
    campaign = local(args.campaign); private(campaign)
    lock = os.open(campaign / 'controller.lock', os.O_CREAT | os.O_EXCL | os.O_RDWR, 0o600)
    fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
    save(campaign / 'admission.json', {'selection': selection_ref, 'admitted': admitted, 'tools': selected_tools,
        'current_head': admission.capture.git('rev-parse', 'HEAD'), 'whole_protocol_qualified': False})
    for name in ('temporary', 'networks', 'permits'):
        private(campaign / name)
    env = base_env | {key: str(campaign / 'temporary') for key in ('TMPDIR', 'TMP', 'TEMP')}
    save(campaign / 'clone-probe.json', sealed_data.probe(campaign / 'clone-probe'))
    save(campaign / 'initial-storage-budget.json', sealed_data.capacity(campaign))
    owner = admitted['binaries']['sumeragi']; stock = admitted['binaries']
    for number, test in enumerate(HELPERS):
        exact_test(owner, test, campaign / f'control-{number}', env, admitted, selected_tools, ignored=False)
    for number, test in enumerate(NATIVE_HELPERS):
        exact_test(stock['iroha_core_zk'], test, campaign / f'native-control-{number}', env, admitted, selected_tools, ignored=False)
    live = campaign / 'live'
    owner_env = env | {'KAGEMUSHA_REAL_NETWORK_CAMPAIGN': str(live),
        'TEST_NETWORK_TMP_DIR': str(campaign / 'networks'), 'IROHA_TEST_NETWORK_PERMIT_DIR': str(campaign / 'permits'),
        'IROHA_TEST_NETWORK_KEEP_DIRS': '1', 'IROHA_TEST_REQUIRE_NETWORK': '1', 'IROHA_TEST_SKIP_BUILD': '1',
        'IROHA_TEST_SERIALIZE_NETWORKS': '1', 'IROHA_TEST_NETWORK_START_ATTEMPTS': '1',
        'TEST_NETWORK_BIN_IROHAD': stock['iroha3d']['path'], 'TEST_NETWORK_BIN_IROHA': stock['iroha']['path'],
        'RUST_BACKTRACE': '1', 'RUST_LOG': 'warn,iroha_test_network=info'}
    command = [owner['path'], OWNER, '--exact', '--ignored', '--nocapture', '--test-threads=1']
    preflight(stock, campaign, env, admitted, selected_tools)
    save(campaign / 'owner-started.json', {'command': command, 'environment': {key: value for key, value in owner_env.items() if key.startswith(('KAGEMUSHA_', 'IROHA_TEST_', 'TEST_NETWORK_', 'RUST_')) or key in ('TMPDIR', 'TMP', 'TEMP')}, 'tools': selected_tools})
    log = (campaign / 'owner.log').open('x')
    owner_stderr = (campaign / 'owner.stderr').open('x')
    child = subprocess.Popen(command, cwd=ROOT, env=owner_env, stdin=subprocess.DEVNULL, stdout=log, stderr=owner_stderr)
    deadline = time.monotonic() + 7 * 86400
    setup = None; failure = None; completed = {}; settled = {}; artifacts = {}; post_restart = {}
    def remember(records):
        for name, item in records.items():
            artifacts[item['path']] = item
    def recheck_artifacts():
        for item in artifacts.values():
            require(observe(item['path']) == item, 'retained stage original changed')
    try:
        try:
            save(campaign / 'owner-process.json', {'pid': child.pid, 'command': command, 'signals_permitted': False})
            ready_ref, ready = wait_document(live / 'ready.json', child, deadline)
            require(ready['schema'] == 'iroha.kagemusha.real-network-ready.v1', 'live readiness grammar')
            setup = ref(observe(live / 'setup/setup.json'))
            require(setup['sha256'] == ready['setup_sha256'], 'live setup pin differs')
            setup_originals = component.manifest_rows(live / 'setup', document(setup)['originals'], component.SETUP_NAMES)
            remember(setup_originals)
            remember({'setup': observe(live / 'setup/setup.json'), 'ready': ready_ref})
            installed_h5 = observe(live / 'native-genesis-dkg/installed-h5.norito', 16 << 20)
            remember({'native-genesis-installed-h5': installed_h5})
            save(campaign / 'native-provisioning-observation.json', {
                'ready': ref(ready_ref), 'setup': setup, 'installed_h5': ref(installed_h5),
                'expected_starts_after_dkg': 2,
                'scope': 'Installed H5 is written before the mandatory DKG restart. The selected current Rust owner emits ready only after all four post-restart H5 decisions are checked; this DATA receipt alone does not establish restart completion.'})
            fixture, catalog, pins, catalog_outputs = catalog_from_live_setup(campaign, setup, admitted, selected_tools, env)
            remember(catalog_outputs)
            pack = ref(observe(catalog / 'engineering-verifier-pack.norito'))
            registration_command = phase_command(live, 'registration', setup['sha256'], ready_ref['sha256'],
                scheme_id=pins['KAGEMUSHA_WALLET_SCHEME_ID'],
                manifest_digest=pins['KAGEMUSHA_WALLET_MANIFEST_DIGEST'], verifier_pack=pack)
            registered_ref, registered = wait_document(live / 'registered.json', child, deadline)
            protocol.registered(registered, setup['sha256'], document(setup), pins, pack)
            registration_source = observe(Path(registered['registration_source_path']), 16384)
            require(registration_source['sha256'] == registered['registration_source_sha256'], 'registration source original differs')
            remember({'registration-command': registration_command, 'registered': registered_ref,
                      'registration-source': registration_source})
            issuer_request = save(campaign / 'issuer-request.json', {
                'schema': 'iroha.kagemusha.network-controller-issuer-request.v1',
                'setup_sha256': setup['sha256'], 'registered_sha256': registered_ref['sha256'],
                'scheme_id': pins['KAGEMUSHA_WALLET_SCHEME_ID'],
                'manifest_digest': pins['KAGEMUSHA_WALLET_MANIFEST_DIGEST'],
                'verifier_pack': pack, 'selection_path': str(campaign / 'issuer-selection.json'),
                'scope': 'Machine-to-machine engineering handoff: select an explicit private native composer request for these current originals; never serialize private request or configuration bodies in receipts.'})
            issuer_inputs, issuer_originals = compose_issuer(campaign, issuer_request, setup, registered_ref,
                child, deadline, env, admitted, selected_tools)
            remember(issuer_originals)
            phase_command(live, 'issuer', setup['sha256'], registered_ref['sha256'], **issuer_inputs)
            issuer_ref, issuer = wait_document(live / 'issuer-ready.json', child, deadline)
            protocol.issuer_ready(issuer, setup['sha256'], registered_ref, registered, issuer_inputs,
                                 setup_originals['signed-genesis.wire']['sha256'])
            admitted['retained'][issuer_inputs['configuration']['path']] = issuer_inputs['configuration']['sha256']
            remember({'issuer': issuer_ref})
            native_env = native_environment(admitted, fixture, catalog, setup, pins) | {key: env[key] for key in ('TMPDIR', 'TMP', 'TEMP')}
            selected_data, clone_receipt = sealed_data.prepare(
                catalog, ref(catalog_outputs['qualified-source-membership.json']), pins, campaign / 'sealed-data')
            clone_ref = save(campaign / 'sealed-data/clone-receipt.json', clone_receipt)
            source_selection = save(campaign / 'sealed-data/selection.json', selected_data)
            remember({'clone-receipt': clone_ref, 'source-selection': source_selection})
            native_env.update(KAGEMUSHA_NATIVE_SOURCE_SELECTION=source_selection['path'],
                              KAGEMUSHA_NATIVE_SOURCE_SELECTION_SHA256=source_selection['sha256'])
            native = stock['iroha_core_zk']
            target_out = campaign / 'target/output'
            native_env['KAGEMUSHA_NATIVE_LOAD_TARGET_OUTPUT'] = str(target_out)
            exact_test(native, TARGET, campaign / 'target', native_env, admitted, selected_tools)
            target = component.output_records('target', target_out, native_env)
            remember(target)
            native_env.update(KAGEMUSHA_NATIVE_LOAD_TARGET=target['target']['path'], KAGEMUSHA_NATIVE_LOAD_TARGET_SHA256=target['target']['sha256'])
            def retain_activation(prepared_ref, wire):
                retention_env = native_env | {
                    'KAGEMUSHA_NATIVE_ACTIVATION_PREPARED': prepared_ref['path'],
                    'KAGEMUSHA_NATIVE_ACTIVATION_PREPARED_SHA256': prepared_ref['sha256'],
                    'KAGEMUSHA_NATIVE_ACTIVATION_RETENTION_OUTPUT': str(campaign / 'activation-retention/output'),
                    'KAGEMUSHA_NATIVE_ACTIVATION_INVOCATION': str(uuid.uuid4())}
                exact_test(native, RETENTION, campaign / 'activation-retention', retention_env, admitted, selected_tools)
                records = component.output_records('activation-retention', campaign / 'activation-retention/output', retention_env)
                require(records['wire']['sha256'] == wire['sha256'], 'native retained activation wire differs')
                remember(records)
                return ref(records['retention'])
            funded_ref, funded, prepared_ref, acknowledgment, retention, activation_wire = protocol.funding_handshake(
                setup['sha256'], registered_ref, registered, issuer_ref, ref(target['target']), pins,
                lambda phase, previous, **inputs: phase_command(live, phase, setup['sha256'], previous, **inputs),
                lambda name: wait_document(live / name, child, deadline),
                lambda: observe(live / 'funding/activate-signed.wire', 16 << 20), retain_activation)
            remember({'activation-prepared': prepared_ref, 'activation-acknowledgment': acknowledgment,
                      'activation-wire': activation_wire})
            funding = component.output_records('funding', live / 'funding', native_env, expected_heights=funded)
            remember(funding | {'funded': funded_ref})
            require(funding['capture']['sha256'] == funded['capture_sha256'] and funding['receipt.norito']['sha256'] == funded['receipt_sha256'], 'live funding originals differ')
            native_env.update(KAGEMUSHA_EXECUTED_LOAD_CAPTURE=funding['capture']['path'], KAGEMUSHA_EXECUTED_LOAD_CAPTURE_SHA256=funding['capture']['sha256'],
                KAGEMUSHA_EXECUTED_LOAD_RECEIPT_SHA256=funded['receipt_sha256'], KAGEMUSHA_NATIVE_LOAD_EVIDENCE_OUTPUT=str(campaign / 'load/output'),
                KAGEMUSHA_NATIVE_LOAD_ADMISSION_OUTPUT=str(campaign / 'load/source-admission'))
            exact_test(native, LOAD, campaign / 'load', native_env, admitted, selected_tools)
            load = component.output_records('load', campaign / 'load/output', native_env, expected_heights=funded)
            remember(load)
            native_env.update(KAGEMUSHA_NATIVE_LOAD_RECEIPT=load['receipt']['path'], KAGEMUSHA_NATIVE_LOAD_RECEIPT_SHA256=load['receipt']['sha256'],
                KAGEMUSHA_NATIVE_LOAD_FINALITY=load['finality']['path'], KAGEMUSHA_NATIVE_LOAD_FINALITY_SHA256=load['finality']['sha256'],
                KAGEMUSHA_NATIVE_EXCHANGE_OUTPUT=str(campaign / 'exchange/output'))
            exact_test(native, EXCHANGE, campaign / 'exchange', native_env, admitted, selected_tools)
            exchange = component.output_records('exchange', campaign / 'exchange/output', native_env)
            exchange.update(native_negative_records(campaign / 'exchange/output', exchange['exchange']))
            remember(exchange)
            phase_command(live, 'settlement', setup['sha256'], funded_ref['sha256'], exchange=ref(exchange['exchange']), target=ref(target['target']), verifier_pack=pack,
                adversarial={'kind': 'software_bad_sigma', 'claim': ref(exchange['c-unload-bad-sigma-claim.norito'])})
            settled_ref, _ = wait_document(live / 'settled.json', child, deadline)
            original_ref, settled, originals = settlement_records(live / 'settlement/settlement.json', native_env, exchange, funded)
            remember(originals | {'settled': settled_ref, 'settlement': original_ref})
            require(settled_ref['sha256'] == original_ref['sha256'], 'settlement handoff differs')
            native_env.update(KAGEMUSHA_NATIVE_EXCHANGE_RESULT=exchange['exchange']['path'], KAGEMUSHA_NATIVE_EXCHANGE_RESULT_SHA256=exchange['exchange']['sha256'],
                KAGEMUSHA_NATIVE_UNLOAD_CLAIM=exchange['claim']['path'], KAGEMUSHA_NATIVE_UNLOAD_CLAIM_SHA256=exchange['claim']['sha256'],
                KAGEMUSHA_NETWORK_UNLOAD_SETTLEMENT=original_ref['path'], KAGEMUSHA_NETWORK_UNLOAD_SETTLEMENT_SHA256=original_ref['sha256'],
                KAGEMUSHA_NATIVE_NETWORK_CONFIRMATION_OUTPUT=str(campaign / 'confirmation/output'))
            exact_test(native, CONFIRM, campaign / 'confirmation', native_env, admitted, selected_tools)
            confirmation_ref = observe(campaign / 'confirmation/output/result.json')
            confirmation = document(ref(confirmation_ref))
            remember({'confirmation_result': confirmation_ref, 'confirmation': observe(campaign / 'confirmation/output/confirmation.norito')})
            require(confirmation['schema'] == 'iroha.kagemusha.native-network-settlement-confirmation.v1'
                    and confirmation['settlement_sha256'] == original_ref['sha256'] and confirmation['exchange_sha256'] == exchange['exchange']['sha256']
                    and confirmation['source_pins'] == pins and confirmation['new_payment_signatures'] == 0
                    and confirmation['confirmation_height'] == settled['settlement_height'], 'same-C confirmation differs')
            require(observe(campaign / 'confirmation/output/confirmation.norito')['sha256'] == confirmation['confirmation_sha256'], 'confirmation original differs')
            phase_command(live, 'confirmed', setup['sha256'], settled_ref['sha256'], confirmation=ref(confirmation_ref))
            completed_ref, completed = wait_document(live / 'completed.json', child, deadline)
            remember({'completed': completed_ref})
            require(completed['schema'] == 'iroha.kagemusha.real-network-monetary-result.v1'
                    and completed['setup_sha256'] == setup['sha256'] and completed['funding_sha256'] == funded_ref['sha256']
                    and completed['issuer_ready_sha256'] == issuer_ref['sha256']
                    and completed['settlement_sha256'] == settled_ref['sha256'] and completed['confirmation_sha256'] == confirmation_ref['sha256'], 'completed network receipt differs')
            post_restart = post_restart_records(live, completed, settled)
            remember(post_restart)
        except Exception as exc:
            failure = retain_failure(campaign, exc)
            if not _UNFINISHED_CHILDREN:
                request_owned_abort(live, setup, failure, child)
    finally:
        if _UNFINISHED_CHILDREN:
            # A proof/census child may still own source/custody. Do not perform
            # another native phase, Cargo metadata check, or a fresh occurrence.
            # The network owner remains observable and handles its own deadline.
            try:
                save(campaign / 'unfinished-campaign.json', {
                    'owner_pid': child.pid, 'children': _UNFINISHED_CHILDREN,
                    'accepted_engineering_campaign': False, 'signals_permitted': False,
                    'automatic_retry_permitted': False, 'custody_reconciliation_required': True})
            finally:
                log.close(); owner_stderr.close()
            return 1
        code = await_natural_exit(child, campaign, deadline)  # Never signal or restart an uncertain child.
        try:
            log.flush(); os.fsync(log.fileno())
            owner_stderr.flush(); os.fsync(owner_stderr.fileno())
        finally:
            log.close()
            owner_stderr.close()
            save(campaign / 'owner-exit.json', {'exit': code})
    source_ok = True
    try:
        unchanged(admitted, selected_tools)
        recheck_artifacts()
    except (OSError, ValueError) as exc:
        source_ok = False
        save(campaign / 'post-source-refusal.json', {'error': str(exc), 'accepted': False})
    peer_runs = {}
    for path in (campaign / 'networks').rglob('run-*-config.toml'):
        if re.fullmatch(r'run-[1-9][0-9]*-config.toml', path.name):
            peer_runs.setdefault(str(path.parent), []).append(int(path.name.split('-')[1]))
    exact_peers = protocol.peer_restarts(peer_runs)
    processes = child_process.natural_run(
        ['ps', '-axo', 'pid=,command='], check=True, stdout=subprocess.PIPE, text=True).stdout.splitlines()
    remaining = [line for line in processes if stock['iroha3d']['path'] in line and str(campaign / 'networks') in line]
    owner_log = component.retained_text(campaign / 'owner.log')
    named_result = exact_result(OWNER, owner_log, code)
    verdict = final_verdict(code, owner_log, completed, settled,
                            source_ok and failure is None and named_result, exact_peers, remaining)
    save(campaign / 'result.json', verdict | {'exit': code, 'failure': failure, 'peer_runs': peer_runs, 'retained_stage_originals': artifacts,
        'post_restart_replay_originals': post_restart,
        'remaining_peers': remaining, 'owner_log_sha256': component.log_pin(campaign / 'owner.log'),
        'owner_stderr_sha256': component.log_pin(campaign / 'owner.stderr'), 'exact_named_owner_pass': named_result,
        'scope': 'Engineering current-source same-network execution, exact new anchor and retained wallets. Separate hardware, phone, bank, physical crash and full release gates remain.'})
    os.close(lock)
    return 0 if verdict['accepted_engineering_campaign'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
