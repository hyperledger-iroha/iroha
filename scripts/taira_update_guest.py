#!/usr/bin/env python3
"""State-preserving Taira guest worker; invoked by taira_update.py only.

Python reads public units/status and metadata only. Retained file metadata binds
private configuration; the native daemon alone consumes config/key contents.
No ledger removal, key/config rewrite, reset, signing, or transaction submission.
"""
import ast
import base64
import hashlib
import json
import os
import re
from pathlib import Path
import stat
import subprocess
import time

# One isolated remote process serves one explicit deployment and operation.
SNAPSHOT_ARTIFACTS = {'snapshot.data', 'snapshot.sha256', 'snapshot.sig',
                      'snapshot.fast.norito', 'snapshot.merkle.json'}
BOUND = False


def configure(plan):
    """Bind deployment-owned public paths once, before any guest observation."""
    global BOUND, BASE, OLD, CONFIG_RELEASE, PREVIOUS_DAEMON, DAEMON, CLI
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
    OLD = PREDECESSOR['commit']
    PREVIOUS_DAEMON = Path(PREDECESSOR['daemon'])
    DAEMON = BASE / ('release-' + plan['commit'] + '-' + plan['operation']) / 'bin/iroha3d_taira'
    CLI = DAEMON.with_name('iroha')
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


def command(argv, *, timeout=60, name=None):
    # Output may contain native configuration diagnostics; retain it privately,
    # never include arbitrary stderr/config-related output in the public report.
    result = subprocess.run(list(map(str, argv)), stdin=subprocess.DEVNULL,
                            capture_output=True, timeout=timeout, env=ENV)
    if name:
        write_new(ATTEMPT / (name + '.stdout'), result.stdout)
        write_new(ATTEMPT / (name + '.stderr'), result.stderr)
        record(name + '.result.json', {'exit_code': result.returncode})
    need(result.returncode == 0, 'native command failed: ' + (name or Path(argv[0]).name))
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
             'Result', 'ExecMainCode', 'ExecMainStatus')
    raw = command(['/usr/bin/systemctl', 'show', '--all',
                   *['--property=' + name for name in names], unit], timeout=15)
    result = dict(line.split('=', 1) for line in raw.decode().splitlines())
    need(set(result) == set(names), 'systemd fields differ')
    need(result['LoadState'] == 'loaded' and result['DropInPaths'] == ''
         and result['NeedDaemonReload'] == 'no' and result['Job'] == ''
         and result['FragmentPath'] == '/etc/systemd/system/' + unit,
         'systemd fragment/job differs: ' + unit)
    return result


def public_get(index, route):
    raw = command(['/usr/bin/curl', '--fail', '--silent', '--show-error', '--max-time', '8',
                   '-H', 'Accept: application/json', f'http://127.0.0.1:{PORTS[index]}{route}'], timeout=10)
    need(len(raw) <= 2 * 1024 * 1024, 'public response exceeds bound')
    return json.loads(raw)


def public_identity(index):
    status = public_get(index, '/status')
    puzzle = public_get(index, '/v1/accounts/faucet/puzzle')
    need(puzzle.get('network_id') == NETWORK and puzzle.get('chain_discriminant') == 369,
         'live NetworkId or Taira prefix changed')
    build = status.get('build', {})
    height = status.get('blocks')
    need(type(height) is int and height > 0, 'positive retained height missing')
    return {'network_id': puzzle['network_id'], 'height': height,
            'commit': build.get('git_commit_sha')}


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


def observe(row, *, after=False):
    props = systemd(f'iroha3d-{row["role"]}.service')
    need(props['ActiveState'] == 'active' and props['SubState'] == 'running'
         and props['ControlPID'] == '0', 'validator not running: ' + row['role'])
    pid = int(props['MainPID'])
    need(pid > 0, 'validator PID missing')
    identity = retained_identity(row, after=after)
    cmd = unit_command(base64.b64decode(row['after' if after else 'before'], validate=True))
    actual = Path(f'/proc/{pid}/cmdline').read_bytes().rstrip(b'\0').decode().split('\0')
    need(actual == cmd, 'daemon argv differs: ' + row['role'])
    need(os.readlink(f'/proc/{pid}/exe') == identity['executable'],
         'daemon executable differs: ' + row['role'])
    identity.update(systemd=props, public=public_identity(ROLES.index(row['role'])))
    need(systemd(f'iroha3d-{row["role"]}.service') == props,
         'validator changed during observation: ' + row['role'])
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
    # Exact observed primary-lane hash journal; native tail/od project only the
    # final public 32-byte block hash. No ledger/state payload enters Python.
    path = STATE_ROOT / role / 'storage/kura/blocks/lane_000_core/blocks.hashes'
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
    path = STATE_ROOT / role / 'storage/kura/blocks/lane_000_core/blocks.hashes'
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


def wait_for_cohort(rows, before, *, after, commit, timeout=180):
    deadline = time.monotonic() + timeout
    latest = None
    while time.monotonic() < deadline:
        try:
            observations = [observe(row, after=after) for row in rows]
            for old, new in zip(before, observations, strict=True):
                compare_retained_identity(old, new)
                need(new['public']['commit'] == commit
                     and new['public']['height'] >= old['public']['height'],
                     'revision or retained height is not ready')
            for index in range(len(rows)):
                command(['/usr/bin/curl', '--fail', '--silent', '--show-error', '--max-time', '8',
                         '-H', 'Accept: text/plain', f'http://127.0.0.1:{PORTS[index]}/readyz'], timeout=10)
            return observations
        except (RuntimeError, OSError, ValueError, subprocess.TimeoutExpired) as error:
            latest = str(error)
            remaining = deadline - time.monotonic()
            if remaining > 0:
                time.sleep(min(2, remaining))
    raise RuntimeError('cohort observation deadline: ' + str(latest))


def retained_attempt(plan):
    """Use the exact installed predecessor public receipts; no previous runtime HTTP required."""
    prior = plan['retained_predecessor']
    need(prior['attempt_name'] == PREDECESSOR['attempt_name'], 'installed predecessor required')
    directory = BASE / prior['attempt_name']
    stamp(directory, True)

    def public_record(name):
        path = directory / name
        before = stamp(path)
        need(before[6] <= 8 * 1024 * 1024, 'retained public record exceeds bound')
        value = json.loads(path.read_bytes())
        need(before == stamp(path), 'retained public record changed during read')
        return value

    intent = public_record('intent.json')
    digest = hashlib.sha256(json.dumps(intent, sort_keys=True, separators=(',', ':')).encode()).hexdigest()
    need(digest == prior['intent_sha256'], 'predecessor intent differs')
    need(intent.get('schema') == PREDECESSOR['plan_schema']
         and intent['network_id'] == plan['network_id'] and intent['commit'] == OLD
         and plan['commit'] != OLD, 'installed predecessor source or candidate differs')
    need(tuple(row['role'] for row in intent['units']) == ROLES, 'predecessor plan lacks exact cohort')
    for original, current in zip(intent['units'], plan['units'], strict=True):
        need(original['after'] == current['before'] and original['after_sha256'] == current['before_sha256'],
             'successor changed installed predecessor unit')
    before = public_record('after.json')
    checkpoints = public_record('checkpoint-stopped.json')
    restored = public_record('checkpoint-restored.json')
    need(tuple(row['role'] for row in before) == ROLES
         and tuple(row['role'] for row in checkpoints) == ROLES
         and tuple(row['role'] for row in restored) == ROLES, 'predecessor evidence lacks exact cohort')
    need(all(row['public']['commit'] == OLD and row['public']['network_id'] == NETWORK for row in before),
         'installed predecessor runtime differs')
    need(all(row.get('native_strict_checkpoint_verified') is True for row in restored),
         'predecessor Strict restoration is not verified')
    completed = public_record('result.json')
    need(completed.get('schema') == PREDECESSOR['result_schema']
         and completed.get('runtime_update_complete') is True
         and completed.get('state_preserved') is True
         and completed.get('retained_native_snapshot_verified') is True
         and completed.get('commit') == OLD and completed.get('network_id') == NETWORK,
         'installed predecessor completion receipt differs')
    return before, checkpoints


def apply(plan):
    configure(plan)
    need(os.geteuid() == 0 and plan['network_id'] == NETWORK, 'guest or network differs')
    need(plan['commit'] != OLD, 'candidate must replace the current runtime')
    need(tuple(row['role'] for row in plan['units']) == ROLES, 'four ordered roles required')
    need([row['name'] for row in plan['artifacts']] == ['iroha3d_taira', 'iroha'],
         'exact candidate daemon and same-revision CLI required')
    retained = retained_attempt(plan)
    # Installed unit bytes and retained config/state metadata bind the old cohort.
    # A crash-looping peer need not answer HTTP before the cohort is stopped.
    for artifact, path in zip(plan['artifacts'], (DAEMON, CLI), strict=True):
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
        compare_retained_identity(snapshot, retained_identity(row))
        snapshot.pop('config_sha256', None)  # The exact retained metadata is checked; no private rehash.
        need(snapshot['public']['commit'] == OLD, 'predecessor runtime differs')
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
    record('stop-intent.json', {'units': UNITS, 'configuration_or_ledger_mutation': False})
    installed = []
    new_start_attempted = False
    try:
        stopped = stop_all()
        record('stopped.json', {'all_four_stopped': True, 'observations': stopped})
        for row, state in zip(before, stopped, strict=True):
            row['systemd'] = state['systemd']
        record('before.json', before)
        checkpoints = [checkpoint_barrier(row, stopped=True, prior=prior)
                       for row, prior in zip(before, retained[1], strict=True)]
        record('checkpoint-stopped.json', checkpoints)
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
        after = wait_for_cohort(plan['units'], before, after=True, commit=plan['commit'])
        record('after.json', after)
        restored = [verify_restored_checkpoint(row, checkpoint)
                    for row, checkpoint in zip(after, checkpoints, strict=True)]
        record('checkpoint-restored.json', restored)
        for index in range(4):
            command(['/usr/bin/curl', '--fail', '--silent', '--show-error', '--max-time', '8',
                     '-H', 'Accept: text/plain', f'http://127.0.0.1:{PORTS[index]}/readyz'],
                    timeout=10, name=f'ready-{index+1}')
        report = json.loads(command([CLI, 'taira', 'doctor',
                                     '--scope', 'basic', '--json', '--public-root', PUBLIC_ORIGIN],
                                    timeout=90, name='public-doctor'))
        need(report.get('command') == 'taira_doctor' and report.get('status') == 'ok'
             and report.get('scope') == 'basic' and report.get('failures') == []
             and len(report.get('checks', [])) == 10
             and all(row.get('ok') is True for row in report['checks']), 'public basic doctor failed')
        result = {'schema': 'taira.daemon-update.result.v1', 'runtime_update_complete': True,
                  'commit': plan['commit'], 'network_id': NETWORK, 'state_preserved': True,
                  'canary_applied_verified': False, 'application_ready': False,
                  'retained_native_snapshot_verified': True,
                  'historical_genesis_replay_supported': False,
                  'historical_replay_limitation': 'Preserve the authenticated current snapshot at or after the deployment replay floor. No historical blocks were rewritten.',
                  'next_action': 'prove a fresh signed transaction Applied under the new runtime'}
        record('result.json', result)
        print(json.dumps(result), flush=True)
    except BaseException as error:
        record('failure.json', {'error': str(error), 'new_start_attempted': new_start_attempted,
                               'installed_units': [row['role'] for row in installed]})
        if not new_start_attempted:
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
                                     'old_daemons_restarted': False, 'observations': restored})
        raise


def apply_locked(plan):
    """Serialize the proven cohort transition across all local coordinators."""
    import fcntl
    root = Path(plan['deployment']['runtime_root'])
    stamp(root, True)
    path = root / '.routine-update.lock'
    fd = os.open(path, os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW, 0o600)
    try:
        before = os.fstat(fd)
        need(stat.S_ISREG(before.st_mode) and before.st_uid == 0 and before.st_nlink == 1
             and stat.S_IMODE(before.st_mode) == 0o600, 'invalid guest update lock')
        fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        need((before.st_dev, before.st_ino) == tuple(stamp(path)[:2]), 'guest update lock changed')
        apply(plan)
    finally:
        os.close(fd)
