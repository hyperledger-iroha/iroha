#!/usr/bin/env python3
"""State-preserving Taira guest worker; invoked by taira_update.py only.

Python reads public units/status and metadata only. Retained file metadata binds
private configuration; the native daemon alone consumes config/key contents.
No ledger removal, key/config rewrite, reset, or Python signing. An explicitly
authorized native epoch supervisor owns ongoing maintenance transactions.
Startup replay may expose a lower prefix; success still requires each retained
checkpoint and a fresh anchored quorum. Heights within one process never regress.
"""
import ast
import base64
import hashlib
import json
import os
import re
import selectors
from pathlib import Path
import stat
import subprocess
import time

# One isolated remote process serves one explicit deployment and operation.
SNAPSHOT_ARTIFACTS = {'snapshot.data', 'snapshot.sha256', 'snapshot.sig',
                      'snapshot.fast.norito', 'snapshot.merkle.json'}
FAILED_START_CHAIN_SCHEMA = 'taira.failed-start-chain.v1'
MAX_FAILED_START_ATTEMPTS = 16
MAX_FAILED_START_RECORD_BYTES = 8 * 1024 * 1024
MAX_FAILED_START_CHAIN_BYTES = 32 * 1024 * 1024
COHORT_STALL_TIMEOUT_SECONDS = 600
COHORT_MAX_TIMEOUT_SECONDS = 90 * 60
COHORT_OBSERVATION_SCHEMA = 'taira.cohort-observation-intent.v1'
COHORT_REMAINING_ACTIONS = ('observe_cohort', 'verify_strict_restore',
                          'public_basic_doctor', 'publish_completion_receipts')
FAILED_START_RECORDS = ('intent.json', 'before.json', 'checkpoint-stopped.json',
                      'start-intent.json', 'failure.json', 'epoch-supervisor-original.json',
                      'epoch-supervisor-pause-intent.json', 'epoch-supervisor-paused.json')
BOUND = False
DEPLOYMENT_LOCK_FD = None
SUPERVISOR_GUARD = None
SUPERVISOR_GUARD_SEQUENCE = 0
SUPERVISOR_NATIVE_SEQUENCE = 0


def configure(plan):
    """Bind deployment-owned public paths once, before any guest observation."""
    global BOUND, BASE, OLD, CANDIDATE_COMMIT, CONFIG_RELEASE, PREVIOUS_DAEMON, DAEMON, CLI, KAGAMI
    global ATTEMPT, NETWORK, ROLES, UNITS, REPLAY_BARRIER, PUBLIC_ORIGIN
    global STATE_ROOT, CONFIG_ROOT, GENESIS_MANIFEST, PORTS, PREDECESSOR
    need(not BOUND, 'one deployment per guest process')
    deployment = plan['deployment']
    BASE = Path(deployment['runtime_root'])
    STATE_ROOT = Path(deployment['state_root'])
    CONFIG_ROOT = Path(deployment['config_root'])
    GENESIS_MANIFEST = Path(deployment['genesis_manifest'])
    CONFIG_RELEASE = deployment['config_release']
    PREDECESSOR = deployment['current']
    installed = plan.get('failed_start', {}).get('installed', PREDECESSOR)
    OLD = installed['commit']
    CANDIDATE_COMMIT = plan['commit']
    PREVIOUS_DAEMON = Path(installed['daemon'])
    DAEMON = BASE / ('release-' + plan['commit'] + '-' + plan['operation']) / 'bin/iroha3d_taira'
    CLI = DAEMON.with_name('iroha')
    KAGAMI = DAEMON.with_name('kagami')
    ATTEMPT = BASE / plan['operation']
    NETWORK = deployment['network_id']
    ROLES = tuple(deployment['roles'])
    UNITS = tuple(f'iroha3d-{role}.service' for role in ROLES)
    PORTS = tuple(deployment['ports'])
    REPLAY_BARRIER = deployment['replay_floor']
    PUBLIC_ORIGIN = deployment['public_origin']
    BOUND = True


ENV = {'PATH': '/usr/bin:/bin:/usr/sbin:/sbin', 'HOME': '/root', 'LC_ALL': 'C'}


def need(value, reason):
    if not value:
        raise RuntimeError(reason)


def validate_failed_start_inputs(deployment, baseline, failed, records, operation,
                                previous_plan, previous_installed):
    """Authenticate public failed-start lineage without inventing a completed runtime."""
    current = deployment['current']
    roles = deployment['roles']
    units = [f'iroha3d-{role}.service' for role in roles]
    need(failed.get('schema') == 'taira.daemon-update.plan.v1'
         and failed.get('deployment') == deployment
         and failed.get('network_id') == deployment['network_id']
         and failed.get('renderer_sha256') == deployment['renderer_sha256'],
         'failed-start baseline or plan differs')
    commit, attempt = failed.get('commit', ''), failed.get('operation', '')
    need(re.fullmatch('[0-9a-f]{40}', commit) and commit != current['commit']
         and re.fullmatch('update-[0-9a-f]{32}', attempt) and attempt != operation
         and attempt != current['attempt_name'], 'failed-start source or operation differs')
    need(failed.get('retained_predecessor') == {
        'attempt_name': current['attempt_name'],
        'intent_sha256': hashlib.sha256(json.dumps(baseline, sort_keys=True,
                                                   separators=(',', ':')).encode()).hexdigest()},
        'failed-start completed predecessor proof differs')
    artifacts = failed.get('artifacts', [])
    need([row.get('name') for row in artifacts] == ['iroha3d_taira', 'iroha', 'kagami']
         and all(row.get('package') == package
                 and re.fullmatch('[0-9a-f]{64}', row.get('sha256', ''))
                 and type(row.get('size')) is int and 1_000_000 < row['size'] < 1024 ** 3
                 for row, package in zip(artifacts, ('irohad', 'iroha_cli', 'iroha_kagami'), strict=True)),
         'failed-start artifact identities differ')
    installed = {'commit': commit, 'attempt_name': attempt,
                 'daemon': str(Path(deployment['runtime_root']) /
                               ('release-' + commit + '-' + attempt) / 'bin/iroha3d_taira')}
    need([row.get('role') for row in failed.get('units', [])] == roles
         and [row.get('role') for row in baseline.get('units', [])] == roles,
         'failed-start unit cohort differs')
    for previous, row in zip(previous_plan['units'], failed['units'], strict=True):
        before = base64.b64decode(row['before'], validate=True)
        after = base64.b64decode(row['after'], validate=True)
        old = previous_installed['daemon'].encode()
        need(row['before'] == previous['after']
             and row['before_sha256'] == previous['after_sha256']
             and hashlib.sha256(before).hexdigest() == row['before_sha256']
             and hashlib.sha256(after).hexdigest() == row['after_sha256']
             and before.count(old) == 1
             and before.replace(old, installed['daemon'].encode(), 1) == after
             and unit_command(before) == [previous_installed['daemon'], '--config',
                 str(Path(deployment['config_root']) / row['role'] / 'current/config/config.toml'), '--sora']
             and unit_command(after)[0] == installed['daemon'],
             'failed-start unit lineage differs')
    need(set(records) == set(FAILED_START_RECORDS) and records['intent.json'] == failed,
         'failed-start intent record differs')
    need(records['start-intent.json'] == {
        'units': units, 'automatic_old_binary_rollback_after_start': False},
        'failed-start startup marker differs')
    failure = records['failure.json']
    need(set(failure) == {'error', 'new_start_attempted', 'installed_units',
                         'validator_stop_attempted', 'validator_stop_confirmed', 'epoch_supervisor_installed'}
         and isinstance(failure['error'], str) and failure['new_start_attempted'] is True
         and failure['validator_stop_attempted'] is True
         and failure['validator_stop_confirmed'] is True
         and failure['installed_units'] == roles, 'failed-start failure marker differs')
    before, checkpoints = records['before.json'], records['checkpoint-stopped.json']
    need([row.get('role') for row in before] == roles
         and [row.get('role') for row in checkpoints] == roles,
         'failed-start retained evidence lacks exact cohort')
    for row, checkpoint in zip(before, checkpoints, strict=True):
        need(row['public']['commit'] == current['commit']
             and row['public']['network_id'] == deployment['network_id']
             and type(row['public']['height']) is int and row['public']['height'] > 0
             and checkpoint.get('cohort_stopped') is True
             and checkpoint.get('invocation_id') == row['systemd']['InvocationID']
             and re.fullmatch('[0-9a-f]{32}', checkpoint['invocation_id'])
             and type(checkpoint.get('checkpoint_height')) is int
             and checkpoint['checkpoint_height'] >= deployment['replay_floor']
             and type(checkpoint['kura_tip']['height']) is int
             and checkpoint['kura_tip']['height'] >= max(checkpoint['checkpoint_height'], row['public']['height'])
             and re.fullmatch('[0-9a-f]{64}', checkpoint['kura_tip']['hash']),
             'failed-start checkpoint or historical observation differs')
    return installed


class FailedStartRecordBudget:
    """Bound public evidence before JSON decoding, including duplicate reference reads."""
    def __init__(self):
        self.consumed = 0

    def consume(self, raw):
        need(len(raw) <= MAX_FAILED_START_RECORD_BYTES,
             'failed-start public record exceeds byte bound')
        self.consumed += len(raw)
        need(self.consumed <= MAX_FAILED_START_CHAIN_BYTES,
             'failed-start chain exceeds aggregate byte bound')


def validate_retained_supervisor(failed, records, previous_plan):
    """Authenticate original intent and the paused journal before replay recovery."""
    value = validate_supervisor_update(failed['epoch_supervisor'], failed['deployment'],
        failed['operation'], failed['commit'], failed['artifacts'])
    need(failed['epoch_supervisor_installed'] == value['installed']
         and records['failure.json']['epoch_supervisor_installed'] in (value['installed'], value['after']),
         'failed-start supervisor installed closure differs')
    original = records['epoch-supervisor-original.json']
    need(isinstance(original, dict) and set(original) == {'original_service_state',
        'successor_service_state', 'original_binding', 'installed_binding', 'journal_identity',
        'native_preflight', 'native_observation'}
        and original['original_service_state'] == value['original_service_state']
        and original['successor_service_state'] == value['successor_service_state']
        and original['original_binding'] == value['before']
        and original['installed_binding'] == failed['epoch_supervisor_installed']
        and isinstance(original['journal_identity'], list) and len(original['journal_identity']) == 2
        and all(type(number) is int and number >= 0 for number in original['journal_identity']),
        'failed-start supervisor original intent or journal identity differs')
    if 'epoch_supervisor' in previous_plan:
        previous = previous_plan['epoch_supervisor']
        need(all(value[key] == previous[key] for key in
                 ('original_service_state', 'successor_service_state', 'before')),
             'failed-start supervisor ancestor changed original operator intent')
    need(records['epoch-supervisor-pause-intent.json'] == {
        'unit': SUPERVISOR_UNIT, 'operation': failed['operation'],
        'original_service_state': original['original_service_state'],
        'journal_identity': original['journal_identity']},
        'failed-start supervisor pause intent differs')
    paused = records['epoch-supervisor-paused.json']
    need(isinstance(paused, dict) and set(paused) == {'operation', 'journal_identity', 'native_quiescence'}
         and paused['operation'] == failed['operation']
         and paused['journal_identity'] == original['journal_identity']
         and isinstance(paused['native_quiescence'], dict),
         'failed-start supervisor paused barrier differs')
    for field, action in (('native_preflight', 'preflight'), ('native_observation', 'observe')):
        report = supervisor_native_validate(json.dumps(original[field]).encode(), action, failed,
                                            verify_current_journal=False)
        need([report['journal']['device'], report['journal']['inode']] == original['journal_identity'],
             'failed-start supervisor native original journal differs')
    report = supervisor_native_validate(json.dumps(paused['native_quiescence']).encode(),
        'quiescence', failed, verify_current_journal=False)
    need([report['journal']['device'], report['journal']['inode']] == original['journal_identity']
         and report['installed_policy_sha256'] == original['native_preflight']['installed_policy_sha256'],
         'failed-start supervisor native paused journal or unit differs')


def artifact_identity(artifacts):
    """Canonical identity ignores storage paths, never binary bytes or package identity."""
    need(isinstance(artifacts, list) and len(artifacts) == 3,
         'exact daemon, CLI and Kagami artifacts required')
    identity = []
    for row, (name, package) in zip(artifacts,
            (('iroha3d_taira', 'irohad'), ('iroha', 'iroha_cli'),
             ('kagami', 'iroha_kagami')), strict=True):
        need(row.get('name') == name and row.get('package') == package
             and isinstance(row.get('sha256'), str)
             and re.fullmatch('[0-9a-f]{64}', row['sha256'])
             and type(row.get('size')) is int and 1_000_000 < row['size'] < 1024 ** 3,
             'invalid ordered daemon, CLI or Kagami artifact identity')
        identity.append((name, package, row['sha256'], row['size']))
    return tuple(identity)


def validate_candidate_transition(commit, artifacts, completed_commit, installed_plan):
    """A new operation may reuse the installed source only with identical artifacts."""
    need(re.fullmatch('[0-9a-f]{40}', commit) and commit != completed_commit,
         'candidate cannot repeat the completed runtime')
    candidate = artifact_identity(artifacts)
    if commit == installed_plan['commit']:
        need(candidate == artifact_identity(installed_plan['artifacts']),
             'same source commit has different prepared daemon, CLI or Kagami bytes')


def failed_attempt_reference_identity(reference):
    """Validate reference shape and compare immutable bytes independently of capture paths."""
    need(set(reference) == {'operation', 'plan', 'records'}
         and re.fullmatch('update-[0-9a-f]{32}', reference['operation'])
         and set(reference['records']) == set(FAILED_START_RECORDS),
         'failed-start attempt reference fields differ')
    for ref in (reference['plan'], *reference['records'].values()):
        need(set(ref) == {'path', 'sha256'} and isinstance(ref['path'], str)
             and Path(ref['path']).is_absolute()
             and isinstance(ref['sha256'], str)
             and re.fullmatch('[0-9a-f]{64}', ref['sha256']),
             'failed-start public record reference differs')
    need(reference['plan']['sha256'] == reference['records']['intent.json']['sha256'],
         'failed-start plan and installed intent bytes differ')
    return (reference['operation'], reference['plan']['sha256'],
            tuple((name, reference['records'][name]['sha256']) for name in FAILED_START_RECORDS))


def validate_failed_start_prefix(failed, prefix, installed):
    """Historical intents must authenticate exactly the earlier chain, without following paths."""
    ancestry = failed.get('failed_start')
    if not prefix:
        need('failed_start' not in failed, 'first failed attempt has an unbound ancestor')
        return
    need(isinstance(ancestry, dict) and ancestry.get('installed') == installed,
         'failed-start ancestor installed identity differs')
    if ancestry.get('schema') == 'taira.failed-start-reference.v1':
        # This is immutable incident evidence, not an accepted operator input.
        need(len(prefix) == 1 and set(ancestry) == {'schema', 'plan', 'records', 'installed'},
             'historical failed-start reference does not bind the complete prefix')
        actual = [{'operation': installed['attempt_name'], 'plan': ancestry['plan'],
                   'records': ancestry['records']}]
    else:
        need(set(ancestry) == {'schema', 'attempts', 'installed'}
             and ancestry['schema'] == FAILED_START_CHAIN_SCHEMA
             and isinstance(ancestry['attempts'], list)
             and len(ancestry['attempts']) == len(prefix),
             'failed-start ancestry does not bind the complete prefix')
        actual = ancestry['attempts']
    need([failed_attempt_reference_identity(ref) for ref in actual]
         == [failed_attempt_reference_identity(ref) for ref in prefix],
         'failed-start ancestor record identities differ')


def validate_failed_start_chain(reference, deployment, baseline, operation, load_attempt,
                                baseline_observations=None, candidate=None):
    """Authenticate a bounded oldest-to-newest chain while keeping completed health separate."""
    need(set(reference) == {'schema', 'attempts'}
         and reference['schema'] == FAILED_START_CHAIN_SCHEMA
         and isinstance(reference['attempts'], list)
         and 1 <= len(reference['attempts']) <= MAX_FAILED_START_ATTEMPTS,
         'bounded failed-start chain required')
    current = deployment['current']
    seen = {operation, current['attempt_name']}
    installed, previous_plan = current, baseline
    entries = []
    source_artifacts = {}
    previous_checkpoints = None
    historical_health = baseline_observations
    for index, ref in enumerate(reference['attempts']):
        failed_attempt_reference_identity(ref)
        need(ref['operation'] not in seen, 'failed-start chain repeats an operation')
        seen.add(ref['operation'])
        failed, records = load_attempt(ref)
        need(failed.get('operation') == ref['operation'], 'failed-start operation differs')
        validate_failed_start_prefix(failed, reference['attempts'][:index], installed)
        if entries:
            need(failed['epoch_supervisor']['installed'] ==
                 entries[-1][1]['failure.json']['epoch_supervisor_installed'],
                 'failed-start supervisor installed ancestry differs')
        else:
            need(failed['epoch_supervisor']['installed'] == failed['epoch_supervisor']['before'],
                 'first failed-start supervisor differs from original installed binding')
        next_installed = validate_failed_start_inputs(
            deployment, baseline, failed, records, operation, previous_plan, installed)
        validate_candidate_transition(failed['commit'], failed['artifacts'],
                                      current['commit'], previous_plan)
        identity = artifact_identity(failed['artifacts'])
        need(source_artifacts.setdefault(failed['commit'], identity) == identity,
             'failed-start ancestry reused a source with different binary bytes')
        validate_retained_supervisor(failed, records, previous_plan)
        if entries:
            need(records['epoch-supervisor-original.json']['journal_identity'] ==
                 entries[0][1]['epoch-supervisor-original.json']['journal_identity'],
                 'failed-start supervisor journal ancestry differs')
        before, checkpoints = records['before.json'], records['checkpoint-stopped.json']
        if historical_health is None:
            historical_health = before
        for original, observed in zip(historical_health, before, strict=True):
            compare_retained_identity(original, observed)
            need(original['public'] == observed['public'],
                 'failed-start historical health differs from completed predecessor')
        if previous_checkpoints is not None:
            for previous, checkpoint in zip(previous_checkpoints, checkpoints, strict=True):
                old, new = previous['kura_tip'], checkpoint['kura_tip']
                need(new['height'] >= old['height']
                     and checkpoint['checkpoint_height'] >= previous['checkpoint_height']
                     and (new['height'] != old['height'] or new['hash'] == old['hash']),
                     'failed-start checkpoint ancestry regressed')
        installed, previous_plan, previous_checkpoints = next_installed, failed, checkpoints
        entries.append((failed, records))
    if candidate is not None:
        validate_candidate_transition(candidate['commit'], candidate['artifacts'],
                                      current['commit'], previous_plan)
        if candidate['commit'] in source_artifacts:
            need(artifact_identity(candidate['artifacts']) == source_artifacts[candidate['commit']],
                 'candidate source differs from its retained ancestry artifacts')
    return installed, entries


def stamp(path, directory=False):
    path = Path(path)
    info = path.lstat()
    need(path.resolve() == path and info.st_uid == 0 and not info.st_mode & 0o022,
         'unsafe owner path: ' + str(path))
    need(stat.S_ISDIR(info.st_mode) if directory else stat.S_ISREG(info.st_mode),
         'wrong path kind: ' + str(path))
    if not directory:
        need(info.st_nlink == 1, 'unexpected hard link: ' + str(path))
    return [info.st_dev, info.st_ino, info.st_mode, info.st_uid, info.st_gid,
            info.st_nlink, info.st_size, info.st_mtime_ns, info.st_ctime_ns]


def write_new(path, raw, mode=0o600):
    path = Path(path)
    stamp(path.parent, True)
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, mode)
    with os.fdopen(fd, 'wb') as out:
        out.write(raw)
        out.flush()
        os.fsync(out.fileno())
    sync(path.parent)


def sync(path):
    fd = os.open(path, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


def record(name, value):
    write_new(ATTEMPT / name, (json.dumps(value, sort_keys=True) + '\n').encode())


class NativeCommandFailure(RuntimeError):
    """Expose a native exit code without copying stdout, stderr or argv."""
    def __init__(self, label, exit_code):
        self.exit_code = exit_code
        super().__init__(f'native command failed: {label} (exit {exit_code})')


def command(argv, *, timeout=60, name=None, pass_fds=(), allowed_exit_codes=(0,)):
    supervisor_guard_check()
    # Output may contain native configuration diagnostics; retain it privately,
    # never include arbitrary stderr/config-related output in the public report.
    result = subprocess.run(list(map(str, argv)), stdin=subprocess.DEVNULL,
                            capture_output=True, timeout=timeout, env=ENV,
                            pass_fds=pass_fds)
    supervisor_guard_check()
    if name:
        write_new(ATTEMPT / (name + '.stdout'), result.stdout)
        write_new(ATTEMPT / (name + '.stderr'), result.stderr)
        record(name + '.result.json', {'exit_code': result.returncode})
    if result.returncode not in allowed_exit_codes:
        raise NativeCommandFailure(name or Path(argv[0]).name, result.returncode)
    return result.stdout


def native_digest(path):
    before = stamp(path)
    value = command(['/usr/bin/sha256sum', path]).split()[0].decode('ascii')
    need(before == stamp(path), 'file changed during native digest: ' + str(path))
    return value


def native_private_command(argv, *, timeout, name):
    # Config validation bypasses runtime custody (taira_runtime_signer.rs:657).
    # Keep every diagnostic byte native-to-file; Python observes exit only.
    out_fd = os.open(ATTEMPT / (name + '.stdout'), os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    err_fd = os.open(ATTEMPT / (name + '.stderr'), os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    try:
        result = subprocess.run(list(map(str, argv)), stdin=subprocess.DEVNULL,
                                stdout=out_fd, stderr=err_fd, timeout=timeout, env=ENV)
        os.fsync(out_fd)
        os.fsync(err_fd)
    finally:
        os.close(out_fd)
        os.close(err_fd)
    record(name + '.result.json', {'exit_code': result.returncode})
    need(result.returncode == 0, 'native configuration validation failed: ' + name)


def unit_command(raw):
    prefix = 'ExecStart=/usr/bin/python3 -c '
    lines = [line for line in raw.decode().splitlines() if line.startswith('ExecStart=')]
    need(len(lines) == 1 and lines[0].startswith(prefix), 'unexpected unit launcher')
    code = json.loads(lines[0][len(prefix):]).replace('%%', '%').replace('$$', '$')
    values = [ast.literal_eval(node.value) for node in ast.parse(code).body
              if isinstance(node, ast.Assign) and len(node.targets) == 1
              and isinstance(node.targets[0], ast.Name) and node.targets[0].id == 'cmd']
    need(len(values) == 1, 'unit must have one literal cmd')
    return values[0]


def replace_daemon(raw, role):
    old = str(PREVIOUS_DAEMON)
    cmd = [old, '--config', str(CONFIG_ROOT / role / 'current/config/config.toml'), '--sora']
    need(unit_command(raw) == cmd and raw.count(old.encode()) == 1, 'old unit command differs')
    changed = raw.replace(old.encode(), str(DAEMON).encode(), 1)
    need(unit_command(changed) == [str(DAEMON), *cmd[1:]], 'new unit command differs')
    need(changed.replace(str(DAEMON).encode(), old.encode(), 1) == raw,
         'unit change exceeds daemon path')
    return changed


def systemd(unit):
    names = ('LoadState', 'ActiveState', 'SubState', 'MainPID', 'ControlPID',
             'FragmentPath', 'DropInPaths', 'NeedDaemonReload', 'InvocationID', 'Job',
             'Result', 'ExecMainCode', 'ExecMainStatus', 'NRestarts')
    raw = command(['/usr/bin/systemctl', 'show', '--all',
                   *['--property=' + name for name in names], unit], timeout=15)
    result = dict(line.split('=', 1) for line in raw.decode().splitlines())
    need(set(result) == set(names), 'systemd fields differ')
    need(result['LoadState'] == 'loaded' and result['DropInPaths'] == ''
         and result['NeedDaemonReload'] == 'no' and result['Job'] == ''
         and result['FragmentPath'] == '/etc/systemd/system/' + unit,
         'systemd fragment/job differs: ' + unit)
    return result


class StartupProbeUnavailable(RuntimeError):
    """Only declared local transport failures or HTTP 503 may be polled."""


def public_probe(index, route, *, name=None):
    need(route in ('/status', '/v1/accounts/faucet/puzzle', '/readyz'),
         'unexpected public probe route')
    label = f'role={ROLES[index]} endpoint={route}'
    accept = 'text/plain' if route == '/readyz' else 'application/json'
    try:
        raw = command(['/usr/bin/curl', '--fail', '--silent', '--show-error', '--max-time', '8',
                       '--write-out', '\n%{http_code}', '-H', 'Accept: ' + accept,
                       f'http://127.0.0.1:{PORTS[index]}{route}'],
                      timeout=10, name=name, allowed_exit_codes=(0, 22))
    except NativeCommandFailure as error:
        # Connection refused, timeout, empty reply, and connection reset can
        # occur during native startup. DNS/configuration/HTTP errors cannot.
        kind = StartupProbeUnavailable if error.exit_code in (7, 28, 52, 56) else RuntimeError
        raise kind(f'public probe failed: {label} curl_exit={error.exit_code}') from None
    except subprocess.TimeoutExpired:
        raise StartupProbeUnavailable(f'public probe failed: {label} timeout') from None
    except (RuntimeError, OSError):
        raise RuntimeError(f'public probe failed: {label} native_probe_unavailable') from None
    body, separator, code = raw.rpartition(b'\n')
    need(separator and re.fullmatch(b'[0-9]{3}', code),
         'public probe HTTP status is malformed: ' + label)
    if code == b'503':
        raise StartupProbeUnavailable('public probe not ready: ' + label + ' http_status=503')
    need(code == b'200', 'public probe HTTP status rejected: ' + label + ' http_status=' + code.decode())
    if route == '/readyz':
        need(body == b'Ready', 'public readiness body differs: ' + label)
    return body


def public_get(index, route):
    raw = public_probe(index, route)
    label = f'role={ROLES[index]} endpoint={route}'
    need(len(raw) <= 2 * 1024 * 1024, 'public response exceeds bound: ' + label)
    try:
        value = json.loads(raw)
    except (ValueError, UnicodeError):
        raise RuntimeError('public identity is not valid JSON: ' + label) from None
    need(isinstance(value, dict), 'public identity is not an object: ' + label)
    return value


def public_identity(index, *, expected_commit, minimum_height=0):
    status = public_get(index, '/status')
    build = status.get('build', {})
    height = status.get('blocks')
    need(isinstance(build, dict) and type(height) is int and height > 0,
         f'public build identity or positive retained height missing: role={ROLES[index]} endpoint=/status')
    # Validate each response before issuing another probe that may be transient.
    # An invalid fresh status must never disappear behind a later puzzle 503.
    need(build.get('git_commit_sha') == expected_commit,
         'candidate revision differs: ' + ROLES[index])
    need(height >= minimum_height, 'committed catch-up height regressed: ' + ROLES[index])
    puzzle = public_get(index, '/v1/accounts/faucet/puzzle')
    need(puzzle.get('network_id') == NETWORK and puzzle.get('chain_discriminant') == 369,
         f'live NetworkId or Taira prefix changed: role={ROLES[index]} endpoint=/v1/accounts/faucet/puzzle')
    return {'network_id': puzzle['network_id'], 'height': height,
            'commit': build['git_commit_sha']}


def retained_identity(row, *, after=False):
    role = row['role']
    unit = f'iroha3d-{role}.service'
    fragment = Path('/etc/systemd/system') / unit
    stamp(fragment)
    raw = fragment.read_bytes()  # Public generated unit only.
    expected = base64.b64decode(row['after' if after else 'before'], validate=True)
    need(raw == expected, 'unit bytes differ: ' + role)
    cmd = unit_command(raw)
    release = CONFIG_ROOT / role / 'releases' / CONFIG_RELEASE
    selector = CONFIG_ROOT / role / 'current'
    need(os.readlink(selector) == str(release), 'current selector changed: ' + role)
    expected_exe = str(DAEMON if after else PREVIOUS_DAEMON)
    config = release / 'config/config.toml'
    need(Path(cmd[2]).resolve() == config, 'config resolution changed: ' + role)
    return {'role': role, 'unit_stamp': stamp(fragment),
            'config_stamp': stamp(config),
            'state_root_identity': stamp(STATE_ROOT / role, True)[:2],
            'current_target': os.readlink(selector), 'executable': expected_exe}


def process_summary(props):
    """Render only bounded systemd state fields, never native diagnostic text."""
    fields = ('ActiveState', 'SubState', 'MainPID', 'InvocationID', 'NRestarts',
              'Result', 'ExecMainStatus')
    values = []
    for key in fields:
        value = props.get(key, 'unavailable')
        safe = value if isinstance(value, str) and re.fullmatch('[A-Za-z0-9_-]{1,64}', value) else 'invalid'
        values.append(key + '=' + safe)
    return ','.join(values)


def observe(row, *, after=False, allow_unavailable=False, expected_commit=None, minimum_height=0):
    props = systemd(f'iroha3d-{row["role"]}.service')
    need(props['ActiveState'] == 'active' and props['SubState'] == 'running'
         and props['ControlPID'] == '0',
         'validator not running: ' + row['role'] + ' ' + process_summary(props))
    pid = int(props['MainPID'])
    need(pid > 0, 'validator PID missing')
    identity = retained_identity(row, after=after)
    cmd = unit_command(base64.b64decode(row['after' if after else 'before'], validate=True))
    actual = Path(f'/proc/{pid}/cmdline').read_bytes().rstrip(b'\0').decode().split('\0')
    need(actual == cmd, 'daemon argv differs: ' + row['role'])
    need(os.readlink(f'/proc/{pid}/exe') == identity['executable'],
         'daemon executable differs: ' + row['role'])
    public = None
    unavailable = None
    try:
        public = public_identity(ROLES.index(row['role']),
                                 expected_commit=expected_commit or (CANDIDATE_COMMIT if after else OLD),
                                 minimum_height=minimum_height)
    except StartupProbeUnavailable as error:
        unavailable = error
    identity.update(systemd=props, public=public)
    current = systemd(f'iroha3d-{row["role"]}.service')
    need(current == props,
         'validator changed during observation: ' + row['role']
         + (('; ' + str(unavailable)) if unavailable else '')
         + ' observed=' + process_summary(props) + '; current=' + process_summary(current))
    if unavailable is not None:
        if not allow_unavailable:
            raise unavailable
        identity['public_unavailable'] = str(unavailable)
    return identity


def snapshot_selection(role):
    """Read only the public digest selector; payload/manifest remain native-owned."""
    root = STATE_ROOT / role / 'snapshots'
    root_identity = stamp(root, True)[:2]
    pointer = root / 'current'
    pointer_stamp = stamp(pointer)
    need(pointer_stamp[6] == 65, 'snapshot selector must be one canonical digest')
    fd = os.open(pointer, os.O_RDONLY | os.O_NOFOLLOW)
    try:
        raw = os.read(fd, 66)
    finally:
        os.close(fd)
    need(re.fullmatch(b'[0-9a-f]{64}\n', raw) is not None,
         'snapshot selector is not canonical')
    selected = raw[:64].decode('ascii')
    generations = root / 'generations'
    stamp(generations, True)
    generation = generations / selected
    generation_identity = stamp(generation, True)[:2]
    need({p.name for p in generation.iterdir()} == SNAPSHOT_ARTIFACTS,
         'snapshot selected generation inventory differs')
    artifacts = {name: stamp(generation / name) for name in sorted(SNAPSHOT_ARTIFACTS)}
    need(pointer_stamp == stamp(pointer) and root_identity == stamp(root, True)[:2]
         and generation_identity == stamp(generation, True)[:2]
         and artifacts == {name: stamp(generation / name) for name in sorted(SNAPSHOT_ARTIFACTS)},
         'snapshot generation changed during metadata capture')
    return {'root_identity': root_identity, 'selector': selected, 'pointer_stamp': pointer_stamp,
            'generation_identity': generation_identity, 'artifacts': artifacts}


def snapshot_events(role, invocation):
    need(re.fullmatch('[0-9a-f]{32}', invocation) is not None, 'exact daemon InvocationID required')
    # Native journal filtering returns only fixed public checkpoint/startup events.
    pattern = ('Successfully created a snapshot of state|Saving latest snapshot and shutting down|'
               'Failed to create a snapshot of state|Deferring snapshot until commit evidence is complete|'
               'Successfully loaded the state from a snapshot|Validated snapshot block hashes against Kura|'
               'Failed to load state snapshot|creating an empty state|Snapshot restore is disabled|'
               'emergency Fast|Replaying authenticated complete Kura prefix')
    query = subprocess.run(['/usr/bin/journalctl', '--no-pager', '-o', 'json', '-n', '64',
                            '-u', f'iroha3d-{role}.service', '_SYSTEMD_INVOCATION_ID=' + invocation,
                            '--grep=' + pattern], stdin=subprocess.DEVNULL,
                           capture_output=True, timeout=15, env=ENV)
    # journalctl --grep returns 1 when no record matched. That is absence of
    # evidence in this invocation, never permission to ignore query failures.
    if query.returncode == 1 and query.stdout == query.stderr == b'':
        return []
    need(query.returncode == 0, 'native snapshot journal query failed')
    raw = query.stdout
    need(len(raw) <= 1024 * 1024, 'snapshot journal projection exceeds bound')
    result = []
    for line in raw.splitlines():
        event = json.loads(line)
        need(event.get('_SYSTEMD_INVOCATION_ID') == invocation, 'snapshot journal invocation differs')
        result.append({'time_us': int(event['__REALTIME_TIMESTAMP']),
                       'message': event['MESSAGE']})
    # journalctl --grep with a line limit can return newest-first. Select
    # checkpoint events by native timestamp, never incidental output order.
    return sorted(result, key=lambda event: event['time_us'])


def native_kura_tip(role):
    # Exact observed chain-scoped hash journal; native tail/od project only the
    # final public 32-byte block hash. No ledger/state payload enters Python.
    path = STATE_ROOT / role / 'storage/kura/blocks/canonical/blocks.hashes'
    before = stamp(path)
    need(before[6] >= 32 and before[6] % 32 == 0, 'canonical Kura hash journal size differs')
    tail = subprocess.Popen(['/usr/bin/tail', '-c', '32', str(path)],
                            stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                            stderr=subprocess.DEVNULL, env=ENV)
    try:
        projected = subprocess.run(['/usr/bin/od', '-An', '-tx1', '-v'], stdin=tail.stdout,
                                   capture_output=True, timeout=10, env=ENV)
        tail.stdout.close()
        need(tail.wait(timeout=10) == 0 and projected.returncode == 0,
             'native Kura public tip projection failed')
    finally:
        if tail.poll() is None:
            tail.kill()
            tail.wait()
    tip = ''.join(projected.stdout.decode('ascii').split())
    need(re.fullmatch('[0-9a-f]{64}', tip) is not None and before == stamp(path),
         'Kura public tip changed during projection')
    return {'height': before[6] // 32, 'hash': tip}


def native_kura_hash(role, height):
    """Project one retained public hash; native tools alone read the journal."""
    need(role in ROLES and type(height) is int and height > 0, 'invalid retained Kura height')
    path = STATE_ROOT / role / 'storage/kura/blocks/canonical/blocks.hashes'
    before = stamp(path)
    need(before[6] >= height * 32, 'retained Kura height is missing')
    reader = subprocess.Popen(['/usr/bin/dd', 'if=' + str(path), 'bs=32',
                               'skip=' + str(height - 1), 'count=1', 'status=none'],
                              stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                              stderr=subprocess.DEVNULL, env=ENV)
    try:
        result = subprocess.run(['/usr/bin/od', '-An', '-tx1', '-v'], stdin=reader.stdout,
                                capture_output=True, timeout=10, env=ENV)
        reader.stdout.close()
        need(reader.wait(timeout=10) == 0 and result.returncode == 0,
             'native retained Kura hash projection failed')
    finally:
        if reader.poll() is None:
            reader.kill()
            reader.wait()
    value = ''.join(result.stdout.decode('ascii').split())
    after = stamp(path)
    need(re.fullmatch('[0-9a-f]{64}', value) is not None
         and before[:6] == after[:6] and after[6] >= before[6],
         'retained Kura journal identity or hash changed during projection')
    return value


def require_retained_tip(role, tip):
    need(native_kura_hash(role, tip['height']) == tip['hash'],
         'retained Kura prefix hash changed: ' + role)


def strict_snapshot_height(events):
    forbidden = ('Failed to load state snapshot', 'creating an empty state',
                 'Snapshot restore is disabled', 'emergency Fast')
    need(not any(any(text in e['message'] for text in forbidden) for e in events),
         'runtime did not use an authenticated Strict snapshot')
    loaded = [e for e in events if 'Successfully loaded the state from a snapshot' in e['message']]
    verified = [e for e in events if 'Validated snapshot block hashes against Kura' in e['message']]
    need(loaded and verified, 'native Strict snapshot/Kura checkpoint authentication missing')
    height = re.search(r'\bat_height=(\d+)\b', loaded[-1]['message'])
    authenticated = re.search(r'\bsnapshot_height=(\d+)\b', verified[-1]['message'])
    need(height and authenticated and int(height[1]) == int(authenticated[1]),
         'native Strict snapshot/Kura checkpoint authentication differs')
    return int(height[1]), loaded[-1]


def checkpoint_native_proof(events):
    publications = [e for e in events if 'Successfully created a snapshot of state' in e['message']]
    if publications:
        latest = publications[-1]
        height = re.search(r'\bat_height=(\d+)\b', latest['message'])
        need(height, 'native publication height missing')
        return int(height[1]), latest['time_us'], 'native_publication'
    if any('Successfully loaded the state from a snapshot' in e['message'] for e in events):
        height, loaded = strict_snapshot_height(events)
        return height, loaded['time_us'], 'native_strict_restore'
    return None


def checkpoint_barrier(observation, *, stopped=False, prior=None):
    role = observation['role']
    selected = snapshot_selection(role)
    tip = native_kura_tip(role)
    events = snapshot_events(role, observation['systemd']['InvocationID'])
    need(selected == snapshot_selection(role), 'snapshot changed during native evidence capture')
    proof = checkpoint_native_proof(events)
    provenance = observation['systemd']['InvocationID']
    evidence = events
    reused = False
    if proof is None and prior is not None:
        need(prior['role'] == role and selected == prior['selection'],
             'retained native checkpoint selection changed')
        evidence = prior['native_events']
        proof = checkpoint_native_proof(evidence)
        provenance = prior.get('proof_invocation_id', prior.get('publication_invocation_id', prior['invocation_id']))
        reused = True
    need(proof is not None, 'no native authenticated snapshot evidence')
    height, proof_time_us, kind = proof
    need(height >= REPLAY_BARRIER, 'native checkpoint precedes affected history')
    need(selected['pointer_stamp'][7] <= proof_time_us * 1000,
         'selected snapshot was replaced after native checkpoint evidence')
    need(height <= tip['height'], 'native checkpoint exceeds retained Kura tip')
    if prior is not None:
        require_retained_tip(role, prior['kura_tip'])
    if stopped:
        need(tip['height'] >= observation['public']['height'],
             'stopped Kura tip lost previously observed height')
    shutdown = [e for e in events if 'Saving latest snapshot and shutting down' in e['message']]
    return {'role': role, 'invocation_id': observation['systemd']['InvocationID'],
            'proof_invocation_id': provenance, 'reused_checkpoint_proof': reused,
            'checkpoint_height': height, 'proof_kind': kind, 'selection': selected, 'kura_tip': tip,
            'native_events': evidence, 'current_invocation_events': events,
            'graceful_stop_observed': bool(shutdown), 'cohort_stopped': stopped,
            'standalone_native_manifest_verification': False}


def verify_restored_checkpoint(observation, checkpoint):
    events = snapshot_events(observation['role'], observation['systemd']['InvocationID'])
    height, _loaded = strict_snapshot_height(events)
    need(height >= max(REPLAY_BARRIER, checkpoint['checkpoint_height']),
         'new runtime snapshot restoration height is below the retained barrier')
    need(observation['public']['height'] >= checkpoint['kura_tip']['height'],
         'new runtime has not restored the retained Kura tip')
    require_retained_tip(observation['role'], checkpoint['kura_tip'])
    for event in events:
        if 'Replaying authenticated complete Kura prefix' in event['message']:
            start = re.search(r'\bstart_height=(\d+)\b', event['message'])
            need(start and int(start[1]) > height,
                 'historical replay crosses the authenticated retained checkpoint')
    return {'role': observation['role'], 'restored_height': height,
            'native_strict_checkpoint_verified': True, 'events': events}


def install_unit(path, raw, expected, mode):
    supervisor_guard_check()
    need(path.read_bytes() == expected, 'installed unit changed before replacement')
    temporary = path.with_name(path.name + '.taira-update-next')
    write_new(temporary, raw, mode)
    os.replace(temporary, path)
    sync(path.parent)


def stop_all():
    command(['/usr/bin/systemctl', 'stop', *UNITS], timeout=150, name='stop')
    states = []
    for unit in UNITS:
        state = systemd(unit)
        need((state['ActiveState'], state['SubState']) in (('inactive', 'dead'), ('failed', 'failed'))
             and state['MainPID'] == state['ControlPID'] == '0' and state['Job'] == '',
             'cohort stop incomplete: ' + unit)
        states.append({'unit': unit, 'systemd': state,
                       'clean_exit': state['Result'] == 'success' and state['ExecMainStatus'] == '0'})
    record('stop-observations.json', states)
    return states


def compare_retained_identity(old, new):
    for key in ('config_stamp', 'state_root_identity', 'current_target'):
        need(old[key] == new[key], 'retained identity changed: ' + key)


def cohort_retained_tip(checkpoints):
    """Freeze the existing cohort's highest committed prefix before startup."""
    need(tuple(row['role'] for row in checkpoints) == ROLES,
         'retained Kura checkpoint cohort differs')
    tips = [row['kura_tip'] for row in checkpoints]
    need(all(type(tip['height']) is int and tip['height'] >= REPLAY_BARRIER
             and re.fullmatch('[0-9a-f]{64}', tip['hash']) for tip in tips),
         'retained cohort Kura tip is invalid')
    height = max(tip['height'] for tip in tips)
    hashes = {tip['hash'] for tip in tips if tip['height'] == height}
    need(len(hashes) == 1, 'highest retained Kura tips disagree')
    return {'height': height, 'hash': hashes.pop()}


def verify_stopped_cohort_prefixes(checkpoints):
    """Check every stopped tip against every peer retaining that height."""
    tip = cohort_retained_tip(checkpoints)
    for checkpoint in checkpoints:
        for peer in checkpoints:
            if peer['kura_tip']['height'] >= checkpoint['kura_tip']['height']:
                require_retained_tip(peer['role'], checkpoint['kura_tip'])
    return tip


def verify_cohort_processes(observations, expected=None):
    """Reject exits/restarts during the complete cohort observation window."""
    for observed, original in zip(observations, expected or observations, strict=True):
        role = observed['role']
        props = observed['systemd']
        need(role == original['role']
             and props['ActiveState'] == 'active' and props['SubState'] == 'running'
             and props['ControlPID'] == '0' and int(props['MainPID']) > 0
             and re.fullmatch('[0-9a-f]{32}', props['InvocationID'])
             and props.get('NRestarts', '0') == '0'
             and all(props[key] == original['systemd'][key]
                     for key in ('MainPID', 'InvocationID')),
             'validator process changed across cohort verification: ' + role
             + ' expected=' + process_summary(original['systemd'])
             + '; observed=' + process_summary(props))
        current = systemd(f'iroha3d-{role}.service')
        need(current == props, 'validator process changed across cohort verification: ' + role
             + ' observed=' + process_summary(props) + '; current=' + process_summary(current))


def observe_healthy_cohort(rows, before, *, after, commit, retained_tip,
                           expected_processes=None, verified=None, minimum_heights=None):
    """Attempt all four peers; only fresh Ready peers can contribute to quorum.

    A prior candidate observation can retain an unavailable peer's identity,
    but is explicitly marked stale and never counted as a fresh quorum vote.
    """
    need(tuple(row['role'] for row in rows) == ROLES
         and tuple(row['role'] for row in before) == ROLES, 'observation cohort differs')
    verified = verified or [None] * len(rows)
    minimum_heights = minimum_heights or [row['public']['height'] for row in before]
    observations = []
    for index, (row, old, prior, minimum) in enumerate(zip(
            rows, before, verified, minimum_heights, strict=True)):
        # The stopped process's height is a completion target, not a lower
        # bound for the new process's intermediate replay observations.
        last_height = prior['public']['height'] if prior is not None and prior['public'] else 0
        new = observe(row, after=after, allow_unavailable=True,
                      expected_commit=commit, minimum_height=last_height)
        compare_retained_identity(old, new)
        public = new['public']
        fresh = public is not None
        if fresh:
            need(public['commit'] == commit, 'candidate revision differs: ' + row['role'])
            if prior is not None and prior['public'] is not None:
                need(public['height'] >= prior['public']['height'],
                     'committed catch-up height regressed: ' + row['role'])
            if public['height'] >= retained_tip['height']:
                require_retained_tip(row['role'], retained_tip)
        elif prior is not None:
            new['public'] = prior['public']
        ready = True
        try:
            public_probe(index, '/readyz')
        except StartupProbeUnavailable as error:
            ready = False
            new['ready_unavailable'] = str(error)
        public = new['public']
        restored = public is not None and public['height'] >= minimum
        new['cohort_observation'] = {
            'public_fresh': fresh, 'ready': ready,
            'own_retained_tip_restored': restored,
            'anchored_quorum_member': fresh and ready and restored
                and public['height'] >= retained_tip['height']}
        observations.append(new)
    verify_cohort_processes(observations, expected_processes)
    return observations


def cohort_sample_summary(observations, retained_tip):
    return {'retained_tip': retained_tip, 'required': 3,
            'quorum_roles': [row['role'] for row in observations
                             if row['cohort_observation']['anchored_quorum_member']],
            'missing_roles': [row['role'] for row in observations
                              if not row['cohort_observation']['public_fresh']],
            'unready_roles': [row['role'] for row in observations
                              if not row['cohort_observation']['ready']],
            'lagging_roles': [row['role'] for row in observations
                              if row['public'] is not None
                              and row['public']['height'] < retained_tip['height']],
            'unverified_roles': [row['role'] for row in observations
                                 if not row['cohort_observation']['own_retained_tip_restored']]}


def observe_cohort(rows, before, *, after, commit, retained_tip, expected_processes=None,
                   verified=None, minimum_heights=None):
    observations = observe_healthy_cohort(
        rows, before, after=after, commit=commit, retained_tip=retained_tip,
        expected_processes=expected_processes, verified=verified, minimum_heights=minimum_heights)
    summary = cohort_sample_summary(observations, retained_tip)
    need(not summary['unverified_roles'] and len(summary['quorum_roles']) >= 3,
         'anchored retained quorum is not ready: ' + json.dumps(summary, sort_keys=True))
    return observations


def wait_for_cohort(rows, before, *, after, commit, retained_tip, startup_processes=None,
                    verified=None, minimum_heights=None, sample_receipts=None,
                    timeout=COHORT_STALL_TIMEOUT_SECONDS,
                    max_timeout=COHORT_MAX_TIMEOUT_SECONDS):
    """Require two fresh anchored quorum samples and all four own restorations.

    Only progressing Ready peers extend their own catch-up budget. A fourth
    stalled peer cannot block a coherent quorum or conceal a permanent failure.
    """
    need(0 < timeout <= max_timeout <= COHORT_MAX_TIMEOUT_SECONDS,
         'cohort observation time bounds are invalid')
    started = time.monotonic()
    hard_deadline = started + max_timeout
    progress_deadlines = [started + timeout for _ in rows]
    previous_heights = [None] * len(rows)
    expected_processes = startup_processes
    deadline = min(hard_deadline, started + timeout)
    confirmations = []
    latest = None
    while time.monotonic() < deadline:
        if expected_processes is not None:
            verify_cohort_processes(expected_processes)
        # Permanent identity, process, hash, protocol and filesystem failures
        # escape immediately. The observer alone classifies unavailable HTTP.
        observations = observe_healthy_cohort(
            rows, before, after=after, commit=commit, retained_tip=retained_tip,
            expected_processes=expected_processes, verified=verified, minimum_heights=minimum_heights)
        now = time.monotonic()
        if now >= deadline:
            break
        if expected_processes is None:
            expected_processes = [{'role': row['role'], 'systemd': dict(row['systemd'])}
                                  for row in observations]
        for index, row in enumerate(observations):
            sample = row['cohort_observation']
            if sample['public_fresh']:
                height = row['public']['height']
                previous = previous_heights[index]
                if sample['ready'] and previous is not None and height > previous:
                    progress_deadlines[index] = now + timeout
                previous_heights[index] = height
        verified = observations
        summary = cohort_sample_summary(observations, retained_tip)
        latest = json.dumps(summary, sort_keys=True)
        if not summary['unverified_roles'] and len(summary['quorum_roles']) >= 3:
            confirmations.append(summary)
            if len(confirmations) == 2:
                if sample_receipts is not None:
                    sample_receipts.extend(confirmations)
                return observations
        else:
            confirmations.clear()
        # Three peers must have time remaining to reach the anchor. All four
        # additionally retain an individual deadline until their own tip is seen.
        budgets = [hard_deadline if row['cohort_observation']['anchored_quorum_member']
                   else progress_deadlines[index] for index, row in enumerate(observations)]
        required_restores = [progress_deadlines[index] for index, row in enumerate(observations)
                             if not row['cohort_observation']['own_retained_tip_restored']]
        deadline = min(hard_deadline, sorted(budgets, reverse=True)[2], *required_restores)
        remaining = deadline - time.monotonic()
        if remaining > 0:
            time.sleep(min(2, remaining))
    raise RuntimeError('cohort observation deadline: ' + str(latest))


def read_bounded_proc(path, limit):
    """Read only an explicitly selected public procfs projection."""
    with path.open('rb') as source:
        raw = source.read(limit + 1)
    need(len(raw) <= limit, 'public updater process projection exceeds bound')
    return raw


def cohort_process_start_time(pid):
    """Read a live process's public start-time field without its environment."""
    need(type(pid) is int and pid > 0, 'invalid updater process PID')
    raw = read_bounded_proc(Path('/proc') / str(pid) / 'stat', 4096)
    parts = raw.rsplit(b') ', 1)
    need(len(parts) == 2, 'updater process stat is malformed')
    prefix, fields = parts
    fields = fields.split()
    need(prefix.split(b' ', 1)[0] == str(pid).encode() and len(fields) >= 20
         and fields[0] not in (b'Z', b'X') and fields[19].isdigit(),
         'updater process identity is not live')
    return int(fields[19])


def cohort_observation_owner(pid=None):
    """Identify a live guest Python process holding the exact update flock."""
    pid = os.getpid() if pid is None else pid
    start_time = cohort_process_start_time(pid)
    process = Path('/proc') / str(pid)
    argv = read_bounded_proc(process / 'cmdline', 4096).split(b'\0')
    need(argv == [b'/usr/bin/python3', b'-I', b'-', b''], 'updater process argv differs')
    lock_path = BASE / '.routine-update.lock'
    lock = stamp(lock_path)
    need(stat.S_ISREG(lock[2]) and lock[3] == 0 and lock[5] == 1
         and stat.S_IMODE(lock[2]) == 0o600, 'invalid guest update lock')
    device, inode = lock[:2]
    matches = []
    for line in read_bounded_proc(Path('/proc/locks'), 1024 * 1024).splitlines():
        row = line.split()
        if len(row) != 8 or row[1:5] != [b'FLOCK', b'ADVISORY', b'WRITE', str(pid).encode()]:
            continue
        identity = row[5].split(b':')
        if len(identity) == 3 and row[6:] == [b'0', b'EOF']:
            major, minor, number = int(identity[0], 16), int(identity[1], 16), int(identity[2])
            if (major, minor, number) == (os.major(device), os.minor(device), inode):
                matches.append(row)
    need(len(matches) == 1, 'updater no longer holds the exact update flock')
    need(stamp(lock_path) == lock and cohort_process_start_time(pid) == start_time
         and read_bounded_proc(process / 'cmdline', 4096).split(b'\0') == argv,
         'updater process or lock changed during observation')
    return {'pid': pid, 'start_time_ticks': start_time,
            'argv': ['/usr/bin/python3', '-I', '-'], 'lock': {'device': device, 'inode': inode}}



SUPERVISOR_UNIT = 'iroha-taira-epoch-supervisor.service'
SUPERVISOR_STATE_ROOT = Path('/var/lib/taira-epoch-supervisor')
SUPERVISOR_JOURNAL_DIR = SUPERVISOR_STATE_ROOT / 'journals'
SUPERVISOR_READY_MAX_BYTES = 64 * 1024
SUPERVISOR_BINDING_KEYS = frozenset({
    'schema_version', 'release_source_commit', 'iroha_sha256', 'kagami_sha256',
    'network_id', 'unit_spec', 'unit_bytes', 'unit_sha256', 'policy_bytes',
    'policy_sha256', 'observation_trust_bytes', 'observation_trust_sha256',
    'custody_bytes', 'custody_sha256',
})


def supervisor_argv(spec):
    """Match the shared fixed public renderer without opening any input path."""
    need(isinstance(spec, dict) and set(spec) == {
        'schema_version', 'cli', 'admin_config', 'operator_key', 'policy', 'trust',
        'custody', 'journal_dir', 'timeout_ms'}, 'supervisor unit spec fields differ')
    need(type(spec['schema_version']) is int and spec['schema_version'] == 1
         and type(spec['timeout_ms']) is int and 0 < spec['timeout_ms'] < 2 ** 64,
         'supervisor unit version or finite timeout differs')
    for key in ('cli', 'admin_config', 'operator_key', 'policy', 'trust', 'custody', 'journal_dir'):
        raw = spec[key]
        need(isinstance(raw, str) and re.fullmatch(r'/[A-Za-z0-9_./:@+-]+', raw)
             and str(Path(raw)) == raw and '..' not in Path(raw).parts and '//' not in raw,
             'unsafe supervisor literal path')
    generation = Path(spec['policy']).parent
    need(generation.parent == SUPERVISOR_STATE_ROOT / 'generations'
         and re.fullmatch('[0-9a-f]{64}', generation.name)
         and spec['journal_dir'] == str(SUPERVISOR_JOURNAL_DIR)
         and Path(spec['cli']).name == 'iroha' and Path(spec['cli']).parent.name == 'bin',
         'supervisor release or generation root differs')
    for key, name in (('admin_config', 'administrator.toml'), ('operator_key', 'http-operator.key'),
                      ('policy', 'policy.json'), ('trust', 'trust.json'), ('custody', 'custody.json')):
        need(spec[key] == str(generation / name), 'supervisor generation file differs: ' + key)
    return (spec['cli'], '--config', spec['admin_config'], '--operator-private-key-file',
            spec['operator_key'], '--fee-payer', 'authority', 'taira', 'epoch-maintenance',
            'supervise', '--policy', spec['policy'], '--trust', spec['trust'],
            '--custody', spec['custody'], '--journal-dir', spec['journal_dir'],
            '--timeout-ms', str(spec['timeout_ms']))


def supervisor_public_json(raw):
    """Decode only an explicitly public, bounded, duplicate-free projection."""
    need(isinstance(raw, str) and 0 < len(raw.encode()) <= 1024 * 1024,
         'invalid supervisor public projection size')
    def unique(rows):
        result = {}
        for key, value in rows:
            need(key not in result, 'duplicate supervisor public projection field')
            result[key] = value
        return result
    return json.loads(raw, object_pairs_hook=unique)


def validate_supervisor_binding(binding, commit, cli, kagami, network, artifacts=None):
    """Bind public policy and unit structure; native preflight owns private custody."""
    need(isinstance(binding, dict) and set(binding) == SUPERVISOR_BINDING_KEYS
         and type(binding['schema_version']) is int and binding['schema_version'] == 1
         and binding['release_source_commit'] == commit and binding['network_id'] == network,
         'supervisor public binding fields or release differ')
    spec = binding['unit_spec']
    argv = supervisor_argv(spec)
    need(argv[0] == str(cli), 'supervisor must use the exact admitted CLI path')
    for field in ('iroha_sha256', 'kagami_sha256', 'unit_sha256', 'policy_sha256',
                  'observation_trust_sha256', 'custody_sha256'):
        need(isinstance(binding[field], str) and re.fullmatch('[0-9a-f]{64}', binding[field]),
             'invalid supervisor digest: ' + field)
    for stem in ('unit', 'policy', 'observation_trust', 'custody'):
        raw = binding[stem + '_bytes']
        need(isinstance(raw, str) and 0 < len(raw.encode()) <= 1024 * 1024
             and hashlib.sha256(raw.encode()).hexdigest() == binding[stem + '_sha256'],
             'supervisor public bytes differ: ' + stem)
    need(Path(spec['policy']).parent.name == binding['policy_sha256'],
         'supervisor generation does not bind exact raw policy')
    policy = supervisor_public_json(binding['policy_bytes'])
    need(isinstance(policy, dict) and set(policy) == {'schema_version', 'intent',
        'release_source_commit', 'iroha_sha256', 'kagami', 'observation_trust_sha256',
        'provision_timeout_ms'} and type(policy['schema_version']) is int
        and policy['schema_version'] == 1 and policy['release_source_commit'] == commit
        and policy['iroha_sha256'] == binding['iroha_sha256']
        and policy['kagami'] == {'path': str(kagami), 'sha256': binding['kagami_sha256']}
        and policy['observation_trust_sha256'] == binding['observation_trust_sha256']
        and type(policy['provision_timeout_ms']) is int and 0 < policy['provision_timeout_ms'] < 2 ** 64,
        'supervisor same-release policy differs')
    intent = policy['intent']
    need(isinstance(intent, dict) and set(intent) == {'authorization', 'network_id',
        'administrator', 'payment_asset', 'transaction_fee_maximum', 'first_epoch',
        'batch_epochs', 'operation_timeout_ms'} and intent['authorization'] == 'until_stopped'
        and intent['network_id'] == network
        and type(intent['first_epoch']) is int and 0 < intent['first_epoch'] < 2 ** 64
        and type(intent['batch_epochs']) is int and 2 <= intent['batch_epochs'] <= 256
        and type(intent['operation_timeout_ms']) is int and 0 < intent['operation_timeout_ms'] < 2 ** 64,
        'explicit bounded ongoing supervisor authority required')
    need(all(isinstance(intent[name], str) and intent[name] for name in
             ('administrator', 'payment_asset', 'transaction_fee_maximum')),
         'supervisor administrator or fee intent is absent')
    trust = supervisor_public_json(binding['observation_trust_bytes'])
    need(isinstance(trust, dict), 'supervisor observation trust must be public native JSON')
    custody = supervisor_public_json(binding['custody_bytes'])
    need(isinstance(custody, dict) and set(custody) == {'schema_version', 'seeds'}
         and type(custody['schema_version']) is int and custody['schema_version'] == 1
         and isinstance(custody['seeds'], list) and len(custody['seeds']) == 4,
         'supervisor requires exact four-validator custody projection')
    validators = []
    for row in custody['seeds']:
        need(isinstance(row, dict) and set(row) == {'validator', 'path'}
             and isinstance(row['validator'], str) and row['validator']
             and isinstance(row['path'], str) and Path(row['path']).is_absolute()
             and str(Path(row['path'])) == row['path'] and '..' not in Path(row['path']).parts,
             'invalid public seed custody reference')
        validators.append(row['validator'])
    need(len(set(validators)) == 4, 'duplicate supervisor validator custody')
    if artifacts is not None:
        identity = artifact_identity(artifacts)
        need(identity[1][2] == binding['iroha_sha256'] and identity[2][2] == binding['kagami_sha256'],
             'supervisor binaries differ from admitted update artifacts')
    return binding


def validate_supervisor_update(value, deployment, operation, commit, artifacts):
    """Admit one explicit preprovisioned supervisor transition without host access."""
    need(isinstance(value, dict) and set(value) == {'schema', 'operation',
        'original_service_state', 'successor_service_state', 'before', 'installed', 'after', 'native_provisioning_receipt'}
        and value['schema'] == 'taira.epoch-supervisor-update.v1'
        and value['operation'] == operation
        and value['original_service_state'] in ('absent', 'running', 'stopped')
        and value['successor_service_state'] in ('running', 'stopped')
        and (value['original_service_state'] == 'absent'
             or value['successor_service_state'] == value['original_service_state'])
        and ((value['before'] is None) == (value['original_service_state'] == 'absent')),
        'explicit supervisor original intent and transition required')
    previous = Path(deployment['current']['daemon'])
    if value['before'] is not None:
        validate_supervisor_binding(value['before'], deployment['current']['commit'],
            previous.with_name('iroha'), previous.with_name('kagami'), deployment['network_id'])
    installed = value['installed']
    if installed is not None:
        spec = installed['unit_spec']
        validate_supervisor_binding(installed, installed['release_source_commit'], spec['cli'],
            Path(spec['cli']).with_name('kagami'), deployment['network_id'])
    release = Path(deployment['runtime_root']) / ('release-' + commit + '-' + operation) / 'bin'
    validate_supervisor_binding(value['after'], commit, release / 'iroha', release / 'kagami',
                                deployment['network_id'], artifacts)
    receipt = value['native_provisioning_receipt']
    need(isinstance(receipt, dict) and set(receipt) == {'path', 'sha256'}
         and isinstance(receipt['path'], str)
         and receipt['path'] == str(Path(value['after']['unit_spec']['policy']).parent / 'provisioning-receipt.json')
         and isinstance(receipt['sha256'], str) and re.fullmatch('[0-9a-f]{64}', receipt['sha256']),
         'exact native public provisioning receipt reference required')
    return value


def supervisor_worker(expected_argv):
    """Bind the exact native worker without reading credentials or environment."""
    props = systemd(SUPERVISOR_UNIT)
    need(props['ActiveState'] == 'active' and props['SubState'] == 'running'
         and props['ControlPID'] == '0' and re.fullmatch(r'0|[1-9][0-9]*', props['NRestarts'])
         and re.fullmatch('[0-9a-f]{32}', props['InvocationID'])
         and re.fullmatch('[1-9][0-9]*', props['MainPID']),
         'epoch supervisor is not one stable running worker')
    pid = int(props['MainPID'])
    boot = read_bounded_proc(Path('/proc/sys/kernel/random/boot_id'), 64).decode('ascii').strip()
    need(re.fullmatch('[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}', boot),
         'invalid epoch supervisor boot identity')
    start = cohort_process_start_time(pid)
    need(start > 0, 'invalid epoch supervisor start time')
    process = Path('/proc') / str(pid)
    raw = read_bounded_proc(process / 'cmdline', 16 * 1024)
    expected = b'\0'.join(value.encode() for value in expected_argv) + b'\0'
    need(raw == expected and os.readlink(process / 'exe') == expected_argv[0],
         'epoch supervisor executable or argv differs')
    need(systemd(SUPERVISOR_UNIT) == props and cohort_process_start_time(pid) == start
         and read_bounded_proc(process / 'cmdline', 16 * 1024) == raw
         and read_bounded_proc(Path('/proc/sys/kernel/random/boot_id'), 64).decode('ascii').strip() == boot
         and os.readlink(process / 'exe') == expected_argv[0],
         'epoch supervisor worker changed during observation')
    return {'worker': {'boot_id': boot, 'pid': pid, 'start_time_ticks': start},
            'systemd': props}


def supervisor_public_ready_file(path):
    """Read one root-owned public receipt through its held bounded descriptor."""
    directory = stamp(path.parent, True)
    need(stat.S_IMODE(directory[2]) == 0o700, 'unsafe supervisor journal directory mode')
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
    try:
        before = os.fstat(fd)
        need(stat.S_ISREG(before.st_mode) and before.st_uid == 0 and before.st_nlink == 1
             and stat.S_IMODE(before.st_mode) == 0o600
             and 0 < before.st_size <= SUPERVISOR_READY_MAX_BYTES,
             'unsafe or oversized supervisor readiness receipt')
        with os.fdopen(os.dup(fd), 'rb') as source:
            raw = source.read(SUPERVISOR_READY_MAX_BYTES + 1)
        after = os.fstat(fd)
        fields = ('st_dev', 'st_ino', 'st_mode', 'st_uid', 'st_gid', 'st_nlink',
                  'st_size', 'st_mtime_ns', 'st_ctime_ns')
        need(all(getattr(before, field) == getattr(after, field) for field in fields)
             and len(raw) == before.st_size
             and (before.st_dev, before.st_ino) == tuple(stamp(path)[:2])
             and stamp(path.parent, True)[:2] == directory[:2],
             'supervisor readiness receipt changed during read')
    finally:
        os.close(fd)
    def unique_pairs(rows):
        result = {}
        for key, value in rows:
            need(key not in result, 'duplicate supervisor readiness field')
            result[key] = value
        return result
    return json.loads(raw, object_pairs_hook=unique_pairs)


def supervisor_ready_structure(policy_sha256, expected_argv, network_id):
    """Revalidate native first-current-completion structure and exact worker custody.

    Native code owns fresh four-peer finality authentication. This reader never
    promotes a historical completion, verifies a proof in Python, or waits for
    an artificial batch-end cursor before admitting the current worker.
    """
    need(re.fullmatch('[0-9a-f]{64}', policy_sha256), 'invalid supervisor policy digest')
    before = supervisor_worker(expected_argv)
    worker = before['worker']
    need(isinstance(network_id, str) and '/' not in network_id and '\x00' not in network_id,
         'invalid supervisor worker directory identity')
    path = SUPERVISOR_JOURNAL_DIR / ('epoch-worker-' + network_id) / (
        f"ready-{policy_sha256}-{worker['boot_id']}-{worker['pid']}-{worker['start_time_ticks']}.json")
    try:
        report = supervisor_public_ready_file(path)
    except FileNotFoundError:
        need(supervisor_worker(expected_argv) == before,
             'supervisor worker changed while readiness was pending')
        return None
    need(isinstance(report, dict) and set(report) == {'schema_version', 'policy_sha256', 'worker',
             'schedule_first_epoch', 'schedule_sha256', 'completion'}
         and type(report['schema_version']) is int and report['schema_version'] == 1
         and report['policy_sha256'] == policy_sha256 and report['worker'] == worker
         and type(report['schedule_first_epoch']) is int and 0 <= report['schedule_first_epoch'] < 2 ** 64
         and isinstance(report['schedule_sha256'], str) and re.fullmatch('[0-9a-f]{64}', report['schedule_sha256'])
         and isinstance(report['worker'], dict)
         and type(report['worker'].get('pid')) is int
         and type(report['worker'].get('start_time_ticks')) is int,
         'supervisor readiness policy or worker differs')
    completion = report['completion']
    need(isinstance(completion, dict) and set(completion) == {
            'schema_version', 'network_id', 'target_epoch', 'transaction_hash',
            'applied_height', 'parameter_sha256', 'carrier_sha256'}
         and type(completion['schema_version']) is int and completion['schema_version'] == 1
         and completion['network_id'] == network_id
         and all(type(completion[name]) is int and 0 < completion[name] < 2 ** 64
                 for name in ('target_epoch', 'applied_height'))
         and all(isinstance(completion[name], str) and re.fullmatch('[0-9a-f]{64}', completion[name])
                 for name in ('transaction_hash', 'parameter_sha256', 'carrier_sha256')),
         'supervisor readiness lacks an exact native completion')
    need(supervisor_worker(expected_argv) == before,
         'supervisor worker changed after readiness read')
    return {'path': str(path), 'worker': worker, 'observation': before, 'policy_sha256': policy_sha256,
            'completion': completion, 'native_fresh_completion_claim': True,
            'python_authenticated_finality': False}


def supervisor_readiness(binding, deadline):
    """Require fresh native Status proof in addition to the exact worker receipt."""
    structure = supervisor_ready_structure(binding['policy_sha256'],
        supervisor_argv(binding['unit_spec']), binding['network_id'])
    if structure is None:
        return None
    spec, worker = binding['unit_spec'], structure['worker']
    remaining_ms = min(spec['timeout_ms'], int((deadline - time.monotonic()) * 1000))
    need(remaining_ms > 0, 'supervisor readiness original deadline exhausted')
    argv = [spec['cli'], '--config', spec['admin_config'], '--operator-private-key-file',
            spec['operator_key'], '--fee-payer', 'authority', 'taira', 'epoch-maintenance',
            'supervisor-status', '--policy', spec['policy'], '--trust', spec['trust'],
            '--journal-dir', spec['journal_dir'], '--boot-id', worker['boot_id'],
            '--pid', str(worker['pid']), '--start-time-ticks', str(worker['start_time_ticks']),
            '--timeout-ms', str(remaining_ms)]
    raw = command(argv, timeout=remaining_ms / 1000, name='epoch-supervisor-authenticated-status')
    need(0 < len(raw) <= SUPERVISOR_READY_MAX_BYTES, 'native supervisor status exceeds bound')
    report = supervisor_public_json(raw.decode())
    need(isinstance(report, dict) and set(report) == {'schema_version', 'policy_sha256',
             'worker', 'initial_completion', 'current_completion'}
         and type(report['schema_version']) is int and report['schema_version'] == 1
         and report['policy_sha256'] == binding['policy_sha256'] and report['worker'] == worker
         and report['initial_completion'] == structure['completion'],
         'native supervisor status does not bind the observed initial completion')
    current = report['current_completion']
    need(isinstance(current, dict) and set(current) == set(structure['completion'])
         and type(current['schema_version']) is int and current['schema_version'] == 1
         and current['network_id'] == binding['network_id']
         and all(type(current[key]) is int and 0 < current[key] < 2 ** 64
                 for key in ('target_epoch', 'applied_height'))
         and all(isinstance(current[key], str) and re.fullmatch('[0-9a-f]{64}', current[key])
                 for key in ('transaction_hash', 'parameter_sha256', 'carrier_sha256')),
         'native supervisor status current completion differs')
    need(supervisor_worker(supervisor_argv(spec)) == structure['observation'],
         'supervisor worker changed after native readiness authentication')
    need(time.monotonic() < deadline, 'supervisor authentication exceeded original deadline')
    return {'structure': structure, 'native_authenticated_status': report}


def supervisor_native_argv(action, plan, timeout_ms):
    """Bind one public wrapper and borrow the lifecycle deployment flock."""
    need(action in ('preflight', 'observe', 'quiescence'),
         'invalid read-only supervisor host action')
    need(type(DEPLOYMENT_LOCK_FD) is int and DEPLOYMENT_LOCK_FD >= 0,
         'supervisor native operation lacks held deployment flock')
    need(not os.path.lexists(SUPERVISOR_STATE_ROOT / '.reset-owner.json'),
         'retained reset owner blocks updater; read-only reconciliation required')
    wrapper = ATTEMPT / 'epoch-supervisor-wrapper.json'
    expected = (json.dumps(plan['epoch_supervisor'], sort_keys=True) + '\n').encode()
    need(stamp(wrapper)[6] == len(expected) and wrapper.read_bytes() == expected,
         'supervisor public wrapper changed')
    return [CLI, 'taira', 'public-reset', 'epoch-supervisor-host', action,
            '--wrapper', wrapper, '--timeout-ms', str(timeout_ms),
            '--deployment-lock-fd', str(DEPLOYMENT_LOCK_FD)]


def supervisor_native_validate(raw, action, plan, *, verify_current_journal=True):
    """Bind the helper's public receipt; the native helper authenticates custody."""
    need(action in ('preflight', 'observe', 'quiescence'), 'unexpected supervisor action')
    need(0 < len(raw) <= SUPERVISOR_READY_MAX_BYTES,
         'native supervisor public report exceeds bound')
    report = supervisor_public_json(raw.decode())
    wrapper = plan['epoch_supervisor']
    after, installed = wrapper['after'], wrapper['installed']
    installed_sha = None if installed is None else installed['policy_sha256']
    allowed = (installed_sha,) if action == 'preflight' else (installed_sha, after['policy_sha256'])
    need(isinstance(report, dict) and set(report) == {'schema', 'action', 'operation',
        'policy_sha256', 'unit_sha256', 'provisioning_receipt', 'installed_policy_sha256',
        'journal', 'worker', 'status', 'service_state'}
        and report['schema'] == 'iroha.taira.epoch-supervisor-host.v1'
        and report['action'] == action and report['operation'] == plan['operation']
        and report['policy_sha256'] == after['policy_sha256']
        and report['unit_sha256'] == after['unit_sha256']
        and report['provisioning_receipt'] == wrapper['native_provisioning_receipt']
        and report['installed_policy_sha256'] in allowed and report['status'] is None
        and report['service_state'] in ('running', 'stopped', 'absent')
        and ((report['installed_policy_sha256'] is None) == (report['service_state'] == 'absent'))
        and (action != 'quiescence' or report['service_state'] != 'running')
        and (action != 'observe' or ((report['worker'] is not None) == (report['service_state'] == 'running'))),
        'native supervisor host receipt binding differs')
    journal = report['journal']
    need(isinstance(journal, dict) and set(journal) == {'path', 'device', 'inode', 'uid', 'gid', 'mode'}
         and journal['path'] == str(SUPERVISOR_JOURNAL_DIR)
         and all(type(journal[field]) is int for field in ('device', 'inode', 'uid', 'gid', 'mode'))
         and journal['device'] >= 0 and journal['inode'] > 0
         and journal['uid'] == journal['gid'] == 0 and journal['mode'] == 0o700,
         'native supervisor journal custody differs')
    if verify_current_journal:
        actual = stamp(SUPERVISOR_JOURNAL_DIR, True)
        need([journal['device'], journal['inode']] == actual[:2]
             and stat.S_IMODE(actual[2]) == 0o700 and actual[3:5] == [0, 0],
             'native supervisor journal identity changed')
    worker = report['worker']
    if worker is not None:
        need(action == 'observe' and report['installed_policy_sha256'] is not None
             and isinstance(worker, dict) and set(worker) == {
                 'boot_id', 'pid', 'start_time_ticks', 'invocation_id', 'n_restarts'}
             and isinstance(worker['boot_id'], str)
             and re.fullmatch('[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}', worker['boot_id'])
             and isinstance(worker['invocation_id'], str) and re.fullmatch('[0-9a-f]{32}', worker['invocation_id'])
             and type(worker['pid']) is int and 0 < worker['pid'] < 2 ** 32
             and type(worker['start_time_ticks']) is int and 0 < worker['start_time_ticks'] < 2 ** 64
             and type(worker['n_restarts']) is int and 0 <= worker['n_restarts'] < 2 ** 64,
             'native supervisor worker observation differs')
    return report


def supervisor_native_report(action, plan):
    global SUPERVISOR_NATIVE_SEQUENCE
    SUPERVISOR_NATIVE_SEQUENCE += 1
    need(action != 'quiescence', 'quiescence requires a retained native guard')
    raw = command(supervisor_native_argv(action, plan, 90000),
                  timeout=90, name=f'epoch-supervisor-native-{action}-{SUPERVISOR_NATIVE_SEQUENCE}',
                  pass_fds=(DEPLOYMENT_LOCK_FD,))
    return supervisor_native_validate(raw, action, plan)


def supervisor_guard_check():
    if SUPERVISOR_GUARD is not None:
        need(SUPERVISOR_GUARD.poll() is None,
             'native supervisor journal guard exited before deliberate release')


def supervisor_guard_acquire(plan):
    """Keep the native journal flock held across the whole paused transition."""
    global SUPERVISOR_GUARD, SUPERVISOR_GUARD_SEQUENCE
    need(SUPERVISOR_GUARD is None, 'supervisor journal guard already held')
    timeout_ms = (COHORT_MAX_TIMEOUT_SECONDS + 600) * 1000
    argv = supervisor_native_argv('quiescence', plan, timeout_ms)
    SUPERVISOR_GUARD_SEQUENCE += 1
    err_fd = os.open(ATTEMPT / f'epoch-supervisor-quiescence-{SUPERVISOR_GUARD_SEQUENCE}.stderr',
                    os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    try:
        child = subprocess.Popen(list(map(str, argv)), stdin=subprocess.PIPE,
            stdout=subprocess.PIPE, stderr=err_fd, env=ENV,
            pass_fds=(DEPLOYMENT_LOCK_FD,))
    finally:
        os.close(err_fd)
    SUPERVISOR_GUARD = child
    try:
        deadline = time.monotonic() + 90
        raw = bytearray()
        with selectors.DefaultSelector() as selector:
            selector.register(child.stdout, selectors.EVENT_READ)
            while not raw.endswith(b'\n'):
                supervisor_guard_check()
                remaining = deadline - time.monotonic()
                need(remaining > 0, 'native journal guard admission timed out')
                need(selector.select(remaining), 'native journal guard admission timed out')
                value = os.read(child.stdout.fileno(), 1)
                need(value and len(raw) < SUPERVISOR_READY_MAX_BYTES,
                     'native journal guard receipt is missing or oversized')
                raw.extend(value)
        report = supervisor_native_validate(bytes(raw), 'quiescence', plan)
        supervisor_guard_check()
        record(f'epoch-supervisor-quiescence-{SUPERVISOR_GUARD_SEQUENCE}.json', report)
        return report
    except BaseException:
        supervisor_guard_release(require_success=False)
        raise


def supervisor_guard_release(*, require_success=True):
    """EOF is the only normal native guard release; never release parent flock."""
    global SUPERVISOR_GUARD
    child = SUPERVISOR_GUARD
    if child is None:
        return
    SUPERVISOR_GUARD = None
    premature = child.poll() is not None
    child.stdin.close()
    try:
        code = child.wait(timeout=30)
        if require_success:
            with selectors.DefaultSelector() as selector:
                selector.register(child.stdout, selectors.EVENT_READ)
                need(selector.select(0) and os.read(child.stdout.fileno(), 1) == b'',
                     'native journal guard emitted unexpected data or retained stdout after EOF')
    except subprocess.TimeoutExpired:
        child.kill()
        child.wait(timeout=30)
        if require_success:
            raise RuntimeError('native journal guard failed to release after EOF')
        return
    finally:
        child.stdout.close()
    if require_success:
        need(not premature and code == 0,
             'native journal guard did not retain custody until deliberate release')


def supervisor_installed_binding(plan):
    """Observe the exact public unit that actually survived partial publication."""
    path = Path('/etc/systemd/system') / SUPERVISOR_UNIT
    installed = plan['epoch_supervisor']['installed']
    if not os.path.lexists(path):
        need(installed is None, 'installed supervisor unit disappeared')
        return None
    before = stamp(path)
    raw = path.read_bytes()
    need(before == stamp(path), 'supervisor unit changed during installed observation')
    for binding in (installed, plan['epoch_supervisor']['after']):
        if binding is not None and raw == binding['unit_bytes'].encode():
            return binding
    raise RuntimeError('supervisor installed unit is outside admitted predecessor/successor closure')


def supervisor_capture(plan):
    """Retain original operator intent independently from later paused observations."""
    native = supervisor_native_report('preflight', plan)
    observed = supervisor_native_report('observe', plan)
    journal = stamp(SUPERVISOR_JOURNAL_DIR, True)
    need(stat.S_IMODE(journal[2]) == 0o700, 'supervisor journal must remain root-private')
    need(observed['installed_policy_sha256'] == native['installed_policy_sha256'],
         'supervisor installed unit changed during original capture')
    original = {'original_service_state': plan['epoch_supervisor']['original_service_state'],
                'successor_service_state': plan['epoch_supervisor']['successor_service_state'],
                'original_binding': plan['epoch_supervisor']['before'],
                'installed_binding': plan['epoch_supervisor_installed'],
                'journal_identity': journal[:2], 'native_preflight': native,
                'native_observation': observed}
    if 'failed_start' in plan:
        prior = plan['failed_start']['attempts'][0]
        retained = retained_public_record(BASE / prior['operation'], 'epoch-supervisor-original.json')
        need(original['original_service_state'] == retained['original_service_state']
             and original['successor_service_state'] == retained['successor_service_state']
             and original['original_binding'] == retained['original_binding']
             and original['journal_identity'] == retained['journal_identity'],
             'supervisor recovery changed original operator intent or journal identity')
    record('epoch-supervisor-original.json', original)
    return original


def supervisor_pause(plan, original):
    """One durable stop attempt; an ambiguous reply admits read-only recovery only."""
    record('epoch-supervisor-pause-intent.json', {
        'unit': SUPERVISOR_UNIT, 'operation': plan['operation'],
        'original_service_state': original['original_service_state'],
        'journal_identity': original['journal_identity']})
    if original['installed_binding'] is not None:
        command(['/usr/bin/systemctl', 'stop', SUPERVISOR_UNIT], timeout=45,
                name='epoch-supervisor-stop')
    paused = supervisor_guard_acquire(plan)
    need(paused['installed_policy_sha256'] == original['native_preflight']['installed_policy_sha256'],
         'supervisor unit changed between original capture and pause')
    need(stamp(SUPERVISOR_JOURNAL_DIR, True)[:2] == original['journal_identity'],
         'supervisor journal replaced during pause')
    record('epoch-supervisor-paused.json', {
        'operation': plan['operation'], 'journal_identity': original['journal_identity'],
        'native_quiescence': paused})


def supervisor_contain(plan):
    """Contain the candidate even when evidence publication has already failed."""
    command(['/usr/bin/systemctl', 'stop', SUPERVISOR_UNIT], timeout=45,
            name='epoch-supervisor-containment-stop')
    if SUPERVISOR_GUARD is None:
        return supervisor_guard_acquire(plan)
    supervisor_guard_check()
    return {'native_guard_retained': True}


def supervisor_resume(plan, original):
    """Publish the bound successor after cohort qualification; preserve stopped intent."""
    binding = plan['epoch_supervisor']['after']
    supervisor_native_report('preflight', plan)
    supervisor_guard_check()
    need(SUPERVISOR_GUARD is not None, 'supervisor publication requires held journal guard')
    path = Path('/etc/systemd/system') / SUPERVISOR_UNIT
    before = plan['epoch_supervisor_installed']
    raw = binding['unit_bytes'].encode()
    record('epoch-supervisor-unit-intent.json', {'before_sha256': None if before is None else before['unit_sha256'],
        'after_sha256': binding['unit_sha256'], 'operation': plan['operation']})
    if before is None:
        need(not os.path.lexists(path), 'first supervisor install found existing unit')
        write_new(path, raw, 0o644)
    else:
        install_unit(path, raw, before['unit_bytes'].encode(), 0o644)
    command(['/usr/bin/systemctl', 'daemon-reload'], name='epoch-supervisor-reload')
    need(stamp(SUPERVISOR_JOURNAL_DIR, True)[:2] == original['journal_identity'],
         'supervisor journal changed before resume')
    if original['successor_service_state'] == 'stopped':
        supervisor_guard_check()
        return {'original_service_state': original['original_service_state'],
                'successor_service_state': 'stopped', 'running': False, 'native_guard_retained': True}
    need(original['successor_service_state'] == 'running', 'supervisor resume lacks explicit running intent')
    record('epoch-supervisor-start-intent.json', {'unit': SUPERVISOR_UNIT,
        'operation': plan['operation'], 'policy_sha256': binding['policy_sha256'],
        'journal_identity': original['journal_identity']})
    deadline = time.monotonic() + min(binding['unit_spec']['timeout_ms'] / 1000, 90)
    supervisor_guard_release()
    command(['/usr/bin/systemctl', 'start', SUPERVISOR_UNIT],
            timeout=min(45, max(0.001, deadline - time.monotonic())), name='epoch-supervisor-start')
    while True:
        ready = supervisor_readiness(binding, deadline)
        need(time.monotonic() < deadline, 'supervisor readiness exceeded original startup deadline')
        if ready is not None:
            return {'original_service_state': original['original_service_state'],
                    'successor_service_state': 'running', 'running': True, 'readiness': ready}
        time.sleep(min(1, max(0, deadline - time.monotonic())))


def stopped_owner_maintenance(operation):
    """Let the native custody owner retire only the proven stopped cohort."""
    request_name = 'stopped-owner-maintenance-request.json'
    record(request_name, {
        'schema': 'taira.stopped-owner-maintenance.request.v1',
        'operation_directory': str(ATTEMPT), 'owner': cohort_observation_owner()})
    # The candidate verifies its direct parent, the held update flock, all four
    # stopped units and retained roots before taking the existing slot locks.
    # Only public process/plan identities cross this descriptor; Python never
    # opens custody files or performs native owner cleanup itself.
    request_fd = os.open(ATTEMPT / request_name, os.O_RDONLY | os.O_NOFOLLOW)
    try:
        raw = command([CLI, 'taira', 'stopped-owner-maintenance',
                       '--request-fd', str(request_fd)], timeout=150,
                      name='stopped-owner-maintenance-command', pass_fds=(request_fd,))
    finally:
        os.close(request_fd)
    need(len(raw) <= 16_384, 'native stopped-owner maintenance report exceeds bound')
    try:
        report = json.loads(raw)
    except (ValueError, UnicodeError):
        raise RuntimeError('native stopped-owner maintenance report is not valid JSON') from None
    need(isinstance(report, dict)
         and set(report) == {'schema', 'operation', 'all_four_stopped_owners_clean'}
         and report['schema'] == 'taira.stopped-owner-maintenance.result.v1'
         and report['operation'] == operation
         and report['all_four_stopped_owners_clean'] is True,
         'native stopped-owner maintenance did not prove the exact stopped cohort')


def verify_cohort_observation_owner(intent):
    """Read-only proof of an active observation owner; never acquire its lock."""
    need(intent.get('schema') == COHORT_OBSERVATION_SCHEMA
         and intent.get('operation') == ATTEMPT.name and intent.get('phase') == 'cohort_observation'
         and re.fullmatch('[0-9a-f]{40}', intent.get('commit', ''))
         and DAEMON == BASE / ('release-' + intent['commit'] + '-' + ATTEMPT.name) / 'bin/iroha3d_taira'
         and intent.get('automatic_restart_or_rollback_after_start') is False
         and intent.get('remaining_actions') == list(COHORT_REMAINING_ACTIONS),
         'cohort observation intent differs')
    terminal = [ATTEMPT / name for name in ('result.json', 'failure.json', 'rollback.json')]
    need(not any(os.path.lexists(path) for path in terminal), 'cohort observation already terminated')
    expected = intent['owner']
    owner = cohort_observation_owner(expected['pid'])
    need(owner == expected and not any(os.path.lexists(path) for path in terminal),
         'cohort observation owner changed or terminated')
    return owner


def retained_public_record(directory, name, digest=None, budget=None):
    """Read a bounded root-owned public receipt, optionally pinned by captured bytes."""
    path = directory / name
    before = stamp(path)
    need(before[6] <= 8 * 1024 * 1024, 'retained public record exceeds bound')
    raw = path.read_bytes()
    need(before == stamp(path), 'retained public record changed during read')
    if digest is not None:
        need(re.fullmatch('[0-9a-f]{64}', digest)
             and hashlib.sha256(raw).hexdigest() == digest, 'failed-start public record digest differs')
    if budget is not None:
        budget.consume(raw)
    return json.loads(raw)


def retained_attempt(plan):
    """Verify completion separately from an explicitly authenticated failed installation."""
    prior = plan['retained_predecessor']
    need(prior['attempt_name'] == PREDECESSOR['attempt_name'], 'installed predecessor required')
    directory = BASE / prior['attempt_name']
    stamp(directory, True)

    def public_record(name):
        return retained_public_record(directory, name)

    intent = public_record('intent.json')
    digest = hashlib.sha256(json.dumps(intent, sort_keys=True, separators=(',', ':')).encode()).hexdigest()
    need(digest == prior['intent_sha256'], 'predecessor intent differs')
    need(intent.get('schema') == PREDECESSOR['plan_schema']
         and intent['network_id'] == plan['network_id'] and intent['commit'] == PREDECESSOR['commit']
         and plan['commit'] != PREDECESSOR['commit'],
         'installed predecessor source or candidate differs')
    need(tuple(row['role'] for row in intent['units']) == ROLES, 'predecessor plan lacks exact cohort')
    before = public_record('after.json')
    checkpoints = public_record('checkpoint-stopped.json')
    restored = public_record('checkpoint-restored.json')
    need(tuple(row['role'] for row in before) == ROLES
         and tuple(row['role'] for row in checkpoints) == ROLES
         and tuple(row['role'] for row in restored) == ROLES, 'predecessor evidence lacks exact cohort')
    need(all(row['public']['commit'] == PREDECESSOR['commit']
             and row['public']['network_id'] == NETWORK for row in before),
         'installed predecessor runtime differs')
    need(all(row.get('native_strict_checkpoint_verified') is True for row in restored),
         'predecessor Strict restoration is not verified')
    completed = public_record('result.json')
    need(completed.get('schema') == PREDECESSOR['result_schema']
         and completed.get('runtime_update_complete') is True
         and completed.get('state_preserved') is True
         and completed.get('retained_native_snapshot_verified') is True
         and completed.get('commit') == PREDECESSOR['commit'] and completed.get('network_id') == NETWORK,
         'installed predecessor completion receipt differs')
    installed_plan = intent
    if 'failed_start' in plan:
        reference = plan['failed_start']
        need(set(reference) == {'schema', 'attempts', 'installed'}
             and set(reference['installed']) == {'commit', 'attempt_name', 'daemon'},
             'failed-start chain plan fields differ')
        budget = FailedStartRecordBudget()

        def load_attempt(ref):
            directory = BASE / ref['operation']
            stamp(directory, True)
            # Partial observations never substitute for a terminal outcome.
            for name in ('result.json', 'rollback.json'):
                need(not os.path.lexists(directory / name),
                     'failed-start attempt has a success or rollback marker: ' + name)
            records = {name: retained_public_record(directory, name, value['sha256'], budget)
                       for name, value in ref['records'].items()}
            return records['intent.json'], records

        public_reference = {key: value for key, value in reference.items() if key != 'installed'}
        installed, entries = validate_failed_start_chain(
            public_reference, plan['deployment'], intent, plan['operation'], load_attempt,
            baseline_observations=before, candidate=plan)
        need(installed == reference['installed'] and installed['commit'] == OLD
             and installed['daemon'] == str(PREVIOUS_DAEMON), 'failed-start installed identity differs')
        # Verify every retained ancestor prefix in the live journal. The final
        # stop barrier independently freezes and verifies the newest prefix.
        for failed, records in entries:
            for checkpoint in records['checkpoint-stopped.json']:
                require_retained_tip(checkpoint['role'], checkpoint['kura_tip'])
        installed_plan, records = entries[-1]
        need(plan['epoch_supervisor']['installed'] == records['failure.json']['epoch_supervisor_installed'],
             'retained supervisor installed closure differs from recovery plan')
        for artifact, path in zip(installed_plan['artifacts'],
                                  (PREVIOUS_DAEMON, PREVIOUS_DAEMON.with_name('iroha'),
                                   PREVIOUS_DAEMON.with_name('kagami')), strict=True):
            need(native_digest(path) == artifact['sha256'] and stamp(path)[6] == artifact['size'],
                 'failed-start installed artifact differs')
        before, checkpoints = records['before.json'], records['checkpoint-stopped.json']
    validate_candidate_transition(plan['commit'], plan['artifacts'], PREDECESSOR['commit'],
                                  installed_plan)
    for original, current in zip(installed_plan['units'], plan['units'], strict=True):
        need(original['after'] == current['before'] and original['after_sha256'] == current['before_sha256'],
             'successor changed installed predecessor unit')
    return before, checkpoints


def apply(plan):
    configure(plan)
    need(os.geteuid() == 0 and plan['network_id'] == NETWORK, 'guest or network differs')
    need(plan['commit'] != PREDECESSOR['commit'], 'candidate cannot repeat the completed runtime')
    need(tuple(row['role'] for row in plan['units']) == ROLES, 'four ordered roles required')
    need([row['name'] for row in plan['artifacts']] == ['iroha3d_taira', 'iroha', 'kagami'],
         'exact same-release daemon, CLI and Kagami required')
    retained = retained_attempt(plan)
    # Installed unit bytes and retained config/state metadata bind the old cohort.
    # A crash-looping peer need not answer HTTP before the cohort is stopped.
    for artifact, path in zip(plan['artifacts'], (DAEMON, CLI, KAGAMI), strict=True):
        need(native_digest(path) == artifact['sha256'], 'candidate digest differs')
        need(stamp(path)[6] == artifact['size'], 'candidate size differs')
        candidate_fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
        try:
            os.fsync(candidate_fd)
        finally:
            os.close(candidate_fd)
    for directory in (DAEMON.parent, DAEMON.parent.parent, BASE):
        sync(directory)
    need(not os.path.lexists(ATTEMPT), 'attempt exists; inspect it instead of repeating mutations')
    ATTEMPT.mkdir(mode=0o700)
    sync(BASE)
    record('intent.json', plan)
    record('epoch-supervisor-wrapper.json', plan['epoch_supervisor'])
    # --version reports package semver, not a source commit. The candidate digest
    # binds the approved build here; /status verifies actual source after start.
    version = command([DAEMON, '--version'], name='candidate-version').decode()
    need(version.startswith('iroha3d '), 'candidate is not the native Iroha daemon')
    before = []
    for index, row in enumerate(plan['units']):
        raw = base64.b64decode(row['before'], validate=True)
        after = base64.b64decode(row['after'], validate=True)
        need(replace_daemon(raw, row['role']) == after, 'unit changes exceed approved cmd[0]')
        snapshot = dict(retained[0][index])
        identity = retained_identity(row)
        compare_retained_identity(snapshot, identity)
        # Health stays the last completed observation. Unit metadata and the
        # executable describe the installed release, which may have failed.
        snapshot['unit_stamp'] = identity['unit_stamp']
        if 'executable' in identity:
            snapshot['executable'] = identity['executable']
        snapshot.pop('config_sha256', None)  # The exact retained metadata is checked; no private rehash.
        need(snapshot['public']['commit'] == PREDECESSOR['commit'], 'predecessor runtime differs')
        before.append(snapshot)
        write_new(ATTEMPT / (row['role'] + '.before.service'), raw)
        write_new(ATTEMPT / (f'iroha3d-{row["role"]}.service'), after, snapshot['unit_stamp'][2] & 0o7777)
        config = CONFIG_ROOT / row['role'] / 'releases' / CONFIG_RELEASE / 'config/config.toml'
        native_private_command([DAEMON, '--config', config, '--genesis-manifest-json',
                 GENESIS_MANIFEST, '--sora', '--check-config'],
                timeout=90, name=f'check-config-{index+1}')
    record('retained-entry.json', before)
    command(['/usr/bin/systemd-analyze', 'verify',
             *[ATTEMPT / (f'iroha3d-{row["role"]}.service') for row in plan['units']]],
            name='verify-units')
    original_supervisor = supervisor_capture(plan)
    installed = []
    new_start_attempted = False
    validator_stop_attempted = False
    validator_stop_confirmed = False
    try:
        supervisor_pause(plan, original_supervisor)
        supervisor_guard_check()
        record('stop-intent.json', {'units': UNITS, 'configuration_or_ledger_mutation': False})
        validator_stop_attempted = True
        stopped = stop_all()
        validator_stop_confirmed = True
        record('stopped.json', {'all_four_stopped': True, 'observations': stopped})
        for row, state in zip(before, stopped, strict=True):
            row['systemd'] = state['systemd']
        record('before.json', before)
        checkpoints = [checkpoint_barrier(row, stopped=True, prior=prior)
                       for row, prior in zip(before, retained[1], strict=True)]
        record('checkpoint-stopped.json', checkpoints)
        retained_tip = verify_stopped_cohort_prefixes(checkpoints)
        record('cohort-retained-tip.json', retained_tip)
        supervisor_guard_check()
        stopped_owner_maintenance(plan['operation'])
        for row, original in zip(plan['units'], before, strict=True):
            path = Path('/etc/systemd/system') / f'iroha3d-{row["role"]}.service'
            install_unit(path, base64.b64decode(row['after']), base64.b64decode(row['before']),
                         original['unit_stamp'][2] & 0o7777)
            installed.append(row)
        command(['/usr/bin/systemctl', 'daemon-reload'], name='daemon-reload')
        record('start-intent.json', {'units': UNITS, 'automatic_old_binary_rollback_after_start': False})
        for checkpoint in checkpoints:
            need(snapshot_selection(checkpoint['role']) == checkpoint['selection'],
                 'retained checkpoint changed before new runtime startup')
            need(native_kura_tip(checkpoint['role']) == checkpoint['kura_tip'],
                 'stopped Kura tip changed before new runtime startup')
        new_start_attempted = True
        command(['/usr/bin/systemctl', 'start', *UNITS], timeout=150, name='start')
        startup_processes = [{'role': role, 'systemd': systemd(unit)}
                             for role, unit in zip(ROLES, UNITS, strict=True)]
        need(all(row['systemd']['NRestarts'] == '0' for row in startup_processes),
             'validator restarted before startup process capture')
        verify_cohort_processes(startup_processes)
        record('startup-processes.json', startup_processes)
        record('cohort-observation-intent.json', {
            'schema': COHORT_OBSERVATION_SCHEMA, 'operation': plan['operation'],
            'commit': plan['commit'], 'phase': 'cohort_observation',
            'owner': cohort_observation_owner(),
            'automatic_restart_or_rollback_after_start': False,
            'remaining_actions': list(COHORT_REMAINING_ACTIONS)})
        minimum_heights = [row['kura_tip']['height'] for row in checkpoints]
        initial_samples = []
        after = wait_for_cohort(plan['units'], before, after=True, commit=plan['commit'],
                                retained_tip=retained_tip, startup_processes=startup_processes,
                                minimum_heights=minimum_heights, sample_receipts=initial_samples)
        record('after.json', after)
        record('cohort-initial-quorum.json', {'samples': initial_samples})
        restored = [verify_restored_checkpoint(row, checkpoint)
                    for row, checkpoint in zip(after, checkpoints, strict=True)]
        record('checkpoint-restored.json', restored)
        report = json.loads(command([CLI, 'taira', 'doctor',
                                     '--scope', 'basic', '--json', '--public-root', PUBLIC_ORIGIN],
                                    timeout=90, name='public-doctor'))
        need(report.get('command') == 'taira_doctor' and report.get('status') == 'ok'
             and report.get('scope') == 'basic' and report.get('failures') == []
             and len(report.get('checks', [])) == 10
             and all(row.get('ok') is True for row in report['checks']), 'public basic doctor failed')
        final_samples = []
        final = wait_for_cohort(plan['units'], before, after=True, commit=plan['commit'],
                                retained_tip=retained_tip, startup_processes=startup_processes,
                                verified=after, minimum_heights=minimum_heights,
                                sample_receipts=final_samples)
        for observed, checkpoint in zip(final, checkpoints, strict=True):
            require_retained_tip(observed['role'], checkpoint['kura_tip'])
        record('cohort-ready.json', {'retained_tip': retained_tip, 'observations': final,
                                     'quorum_confirmations': final_samples,
                                     'startup_processes_unchanged': True})
        supervisor_result = supervisor_resume(plan, original_supervisor)
        verify_cohort_processes(final, startup_processes)
        record('epoch-supervisor-result.json', supervisor_result)
        result = {'epoch_supervisor': supervisor_result, 'schema': 'taira.daemon-update.result.v1', 'runtime_update_complete': True,
                  'commit': plan['commit'], 'network_id': NETWORK, 'state_preserved': True,
                  'canary_applied_verified': False, 'application_ready': False,
                  'retained_native_snapshot_verified': True,
                  'cohort_retained_tip_verified': retained_tip,
                  'cohort_quorum': final_samples[-1],
                  'cohort_fresh_quorum_confirmations': len(final_samples),
                  'cohort_processes_verified_after_public_doctor': True,
                  'all_own_retained_tips_verified_after_public_doctor': True,
                  'historical_genesis_replay_supported': False,
                  'historical_replay_limitation': 'Preserve the authenticated current snapshot at or after the deployment replay floor. No historical blocks were rewritten.',
                  'next_action': 'prove a fresh signed transaction Applied under the new runtime'}
        supervisor_guard_release()
        record('result.json', result)
        print(json.dumps(result), flush=True)
    except BaseException as error:
        try:
            record('failure.json', {'error': str(error), 'new_start_attempted': new_start_attempted,
                'validator_stop_attempted': validator_stop_attempted,
                'validator_stop_confirmed': validator_stop_confirmed,
                'epoch_supervisor_installed': supervisor_installed_binding(plan),
                'installed_units': [row['role'] for row in installed]})
        finally:
            # Guard failure must not suppress either independent containment action.
            supervisor_guard_release(require_success=False)
            if new_start_attempted:
                # Keep the failed candidate and its retained state for diagnosis,
                # while preventing the service supervisor from repeating failures.
                # The caller still owns the deployment lock throughout containment.
                try:
                    supervisor_contain(plan)
                finally:
                    command(['/usr/bin/systemctl', 'stop', *UNITS], timeout=150,
                            name='failed-start-stop')
                stopped = [{'unit': unit, 'systemd': systemd(unit)} for unit in UNITS]
                need(all(row['systemd']['MainPID'] == row['systemd']['ControlPID'] == '0'
                         and row['systemd']['ActiveState'] in ('inactive', 'failed')
                         for row in stopped), 'failed candidate cohort stop incomplete')
                record('failed-start-stopped.json', {'all_four_stopped': True,
                                                   'observations': stopped})
            elif validator_stop_confirmed:
                # No new daemon was allowed to execute retained state. Restore all
                # old units, including any replacement completed before an I/O error.
                for row, original in zip(plan['units'], before, strict=True):
                    path = Path('/etc/systemd/system') / f'iroha3d-{row["role"]}.service'
                    current = path.read_bytes()
                    old = base64.b64decode(row['before'])
                    new = base64.b64decode(row['after'])
                    need(current in (old, new), 'unit has an unknown rollback successor')
                    if current != old:
                        install_unit(path, old, new, original['unit_stamp'][2] & 0o7777)
                command(['/usr/bin/systemctl', 'daemon-reload'], name='rollback-reload')
                restored = []
                for unit in UNITS:
                    state = systemd(unit)
                    need((state['ActiveState'], state['SubState']) in (('inactive', 'dead'), ('failed', 'failed'))
                         and state['MainPID'] == state['ControlPID'] == '0' and state['Job'] == '',
                         'rollback did not retain the paused cohort: ' + unit)
                    restored.append({'unit': unit, 'systemd': state})
                record('rollback.json', {'restored_previous_stopped_cohort': True,
                    'old_daemons_restarted': False, 'supervisor_remains_paused': True,
                    'observations': restored})
            else:
                # An ambiguous stop is never retried. Before-stop failures leave
                # validators untouched; partial-stop failures require read-only
                # manager reconciliation and admit no unit or state mutation.
                record('reconciliation-required.json', {
                    'validator_stop_attempted': validator_stop_attempted,
                    'validator_stop_confirmed': False, 'recovery_only': True,
                    'automatic_manager_retry': False})
            supervisor_guard_release(require_success=False)
        raise


def verify_prepared_artifacts(plan):
    """Observe all three preprovisioned binaries; never create or replace one."""
    import fcntl
    need(os.geteuid() == 0, 'artifact verification requires root')
    identity = artifact_identity(plan['artifacts'])
    need(re.fullmatch('[0-9a-f]{40}', plan['commit'])
         and re.fullmatch('update-[0-9a-f]{32}', plan['operation']), 'invalid prepared operation')
    root = stamp(SUPERVISOR_STATE_ROOT, True)
    need(stat.S_IMODE(root[2]) == 0o700 and root[4] == 0, 'supervisor state root custody differs')
    path = SUPERVISOR_STATE_ROOT / '.deployment.lock'
    fd = os.open(path, os.O_RDWR | os.O_NOFOLLOW)
    try:
        before = os.fstat(fd)
        need(stat.S_ISREG(before.st_mode) and before.st_uid == before.st_gid == 0
             and before.st_nlink == 1 and stat.S_IMODE(before.st_mode) == 0o600,
             'invalid prepared artifact deployment lock')
        fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        need((before.st_dev, before.st_ino) == tuple(stamp(path)[:2]), 'artifact deployment lock changed')
        need(not os.path.lexists(SUPERVISOR_STATE_ROOT / '.reset-owner.json'),
             'retained reset owner blocks artifact admission')
        base = Path(plan['deployment']['runtime_root'])
        release = base / ('release-' + plan['commit'] + '-' + plan['operation'])
        for directory in (base, release, release / 'bin'):
            metadata = stamp(directory, True)
            need(metadata[4] == 0 and stat.S_IMODE(metadata[2]) == 0o700,
                 'prepared artifact root custody differs')
        for name, package, digest, size in identity:
            binary = release / 'bin' / name
            metadata = stamp(binary)
            need(metadata[4] == 0 and stat.S_IMODE(metadata[2]) == 0o755
                 and metadata[6] == size and native_digest(binary) == digest
                 and stamp(binary) == metadata, 'preprovisioned candidate bytes or custody differ')
        print(json.dumps({'schema': 'taira.prepared-update-artifacts.v1',
            'operation': plan['operation'], 'commit': plan['commit'], 'artifacts': plan['artifacts'],
            'runtime_mutated': False}), flush=True)
    finally:
        os.close(fd)


def apply_locked(plan):
    """Hold both update flocks across pause, publication, resume and containment."""
    import fcntl
    global DEPLOYMENT_LOCK_FD
    held = []
    try:
        for root, name in ((Path(plan['deployment']['runtime_root']), '.routine-update.lock'),
                           (SUPERVISOR_STATE_ROOT, '.deployment.lock')):
            metadata = stamp(root, True)
            if root == SUPERVISOR_STATE_ROOT:
                need(stat.S_IMODE(metadata[2]) == 0o700 and metadata[4] == 0,
                     'supervisor state root custody differs')
            path = root / name
            fd = os.open(path, os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW, 0o600)
            held.append(fd)
            before = os.fstat(fd)
            need(stat.S_ISREG(before.st_mode) and before.st_uid == before.st_gid == 0
                 and before.st_nlink == 1 and stat.S_IMODE(before.st_mode) == 0o600,
                 'invalid guest deployment lock')
            fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
            need((before.st_dev, before.st_ino) == tuple(stamp(path)[:2]),
                 'guest deployment lock changed')
        DEPLOYMENT_LOCK_FD = held[-1]
        need(not os.path.lexists(SUPERVISOR_STATE_ROOT / '.reset-owner.json'),
             'retained reset owner blocks updater; never reclaim or clear it')
        apply(plan)
    finally:
        try:
            supervisor_guard_release(require_success=False)
        finally:
            DEPLOYMENT_LOCK_FD = None
            for fd in reversed(held):
                os.close(fd)
