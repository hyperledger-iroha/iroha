#!/usr/bin/env python3
"""Install completed Taira artifacts while preserving the current ledger and custody.

Requires completed maintained preparation and an owner-public deployment record.
--plan-only contacts no host. The default command
requires previously prepared same-release daemon, CLI and Kagami and executes
the reviewed guest controller. --prepare-artifacts creates those exact binaries
via native cat/SSH before native supervisor generation materialization. No secret files are read. Failed attempts are never overwritten.
An explicit --failed-start-chain authenticates every failed startup since the
completed deployment. Unchanged binaries can be retried in a fresh operation.
"""
import argparse
import fcntl
from urllib.parse import urlsplit
import taira_retry as retry
from taira_update_guest import COHORT_MAX_TIMEOUT_SECONDS, MAX_FAILED_START_ATTEMPTS
import base64
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import shlex
import stat
import subprocess
import sys

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
GUEST_OPERATION_TIMEOUT_SECONDS = COHORT_MAX_TIMEOUT_SECONDS + 60 * 60 + MAX_FAILED_START_ATTEMPTS * 4 * 20


def need(value, reason):
    if not value:
        raise RuntimeError(reason)


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def read_public(path):
    return retry.public_record(path)


def write_new(path, raw):
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    with os.fdopen(fd, 'wb') as output:
        output.write(raw)
        output.flush()
        os.fsync(output.fileno())


def module(path, name):
    spec = importlib.util.spec_from_file_location(name, path)
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


def validate_deployment(value):
    """Admit one explicit owner-public first-release Taira installation."""
    need(set(value) == {'schema', 'guest_ssh', 'runtime_root', 'state_root', 'config_root',
         'config_release', 'genesis_manifest', 'network_id', 'public_origin', 'roles',
         'ports', 'replay_floor', 'renderer_sha256', 'current'}, 'deployment fields differ')
    need(value['schema'] == 'taira.runtime-deployment.v1', 'deployment schema differs')
    for key in ('runtime_root', 'state_root', 'config_root', 'genesis_manifest'):
        raw = value[key]
        need(isinstance(raw, str) and raw.startswith('/') and str(Path(os.path.normpath(raw))) == raw
             and not re.search(r'[\x00-\x1f\x7f]', raw), 'invalid public runtime path')
    need(re.fullmatch('[0-9a-f]{40}', value['config_release']) is not None,
         'exact retained config release required')
    need(isinstance(value['network_id'], str) and value['network_id'].startswith('hash:'),
         'exact retained NetworkId required')
    origin = urlsplit(value['public_origin'])
    need(origin.scheme == 'https' and origin.hostname and not origin.username and not origin.password
         and not origin.query and not origin.fragment and not origin.path,
         'canonical public HTTPS origin required')
    need(value['roles'] == [f'taira-validator-{i}' for i in range(1,5)]
         and len(value['ports']) == 4 and len(set(value['ports'])) == 4
         and all(type(port) is int and 1 <= port <= 65535 for port in value['ports']),
         'exact four-validator Taira layout required')
    need(type(value['replay_floor']) is int and value['replay_floor'] > 0,
         'explicit retained replay floor required')
    current = value['current']
    need(set(current) == {'commit', 'daemon', 'attempt_name', 'plan_schema', 'result_schema',
                         'local_plan', 'local_plan_sha256'}, 'current installation fields differ')
    need(re.fullmatch('[0-9a-f]{40}', current['commit']) is not None
         and re.fullmatch('[a-zA-Z0-9][a-zA-Z0-9_-]{0,127}', current['attempt_name']) is not None,
         'exact current installation required')
    daemon = Path(current['daemon'])
    need(daemon.is_absolute() and daemon.is_relative_to(Path(value['runtime_root']))
         and '..' not in daemon.parts, 'current daemon escapes approved runtime root')
    for key in ('plan_schema', 'result_schema'):
        need(isinstance(current[key], str) and re.fullmatch(r'taira\.[a-z0-9.-]+\.v1', current[key]),
             'explicit retained receipt schema required')
    for digest in (value['renderer_sha256'], current['local_plan_sha256']):
        need(re.fullmatch('[0-9a-f]{64}', digest) is not None, 'public evidence digest missing')
    retry.validate_ssh(value['guest_ssh'])
    return value


def validate_build(build, commit):
    need(re.fullmatch('[0-9a-f]{40}', commit) is not None and commit != '0' * 40,
         'exact approved source commit required')
    need(build.get('commit') == commit and build.get('target') == 'aarch64-unknown-linux-gnu'
         and build.get('profile') == 'release' and build.get('jobs') == 6
         and build.get('source_unchanged') is True and build.get('toolchain_unchanged') is True
         and build.get('deployed') is False and build.get('release_qualified') is False
         and build.get('native_check_scope') == 'basic' and 'exit_code' not in build,
         'completed maintained basic preparation required')
    rows = build.get('artifacts', [])
    need(len(rows) == 4 and {row.get('name') for row in rows}
         == {'iroha3d_taira', 'iroha', 'kagami', 'sorafs-node'}, 'maintained four-artifact result required')
    selected = []
    for name, package in [('iroha3d_taira', 'irohad'), ('iroha', 'iroha_cli'),
                          ('kagami', 'iroha_kagami')]:
        artifact = next(row for row in rows if row['name'] == name)
        need(artifact.get('package') == package and re.fullmatch('[0-9a-f]{64}', artifact.get('sha256', ''))
             and type(artifact.get('size')) is int and 1_000_000 < artifact['size'] < 1024 ** 3,
             'invalid candidate artifact identity: ' + name)
        selected.append(artifact)
    return selected


def failed_start_inputs(reference, deployment, prior, guest, operation, candidate):
    """Resolve a complete ancestry of digest-bound public failed-attempt records."""
    budget = guest.FailedStartRecordBudget()

    def read_ref(ref):
        raw = retry.public_record(ref['path'], ref['sha256'])
        budget.consume(raw)
        return retry.decode(raw)

    def load_attempt(ref):
        failed = read_ref(ref['plan'])
        records = {name: read_ref(value) for name, value in ref['records'].items()}
        return failed, records

    installed, entries = guest.validate_failed_start_chain(
        reference, deployment, prior, operation, load_attempt, candidate=candidate)
    return dict(reference, installed=installed), entries[-1][0], entries[-1][1]['failure.json']['epoch_supervisor_installed']


def make_plan(build, deployment, prior, guest, operation, failed_start=None, *, supervisor):
    commit = build['commit']
    artifacts = validate_build(build, commit)
    current = deployment['current']
    need(commit != current['commit'], 'candidate is already the current runtime')
    need(re.fullmatch('update-[0-9a-f]{32}', operation), 'invalid operation directory')
    need(operation != current['attempt_name'], 'fresh operation directory required')
    need(prior.get('schema') == current['plan_schema'] and prior.get('commit') == current['commit']
         and prior.get('network_id') == deployment['network_id'], 'installed predecessor plan differs')
    need(sha(read_public(ROOT / 'scripts/taira_validator_unit.py')) == deployment['renderer_sha256']
         == prior['renderer_sha256'], 'reviewed custody renderer changed')
    need([row['role'] for row in prior['units']] == deployment['roles'], 'predecessor cohort differs')
    value = {'schema':'taira.daemon-update.plan.v1', 'commit':commit,
             'network_id':deployment['network_id'], 'artifacts':artifacts,
             'operation':operation, 'deployment':deployment,
             'epoch_supervisor':supervisor}
    installed_plan = prior
    if failed_start is not None:
        value['failed_start'], installed_plan, supervisor_installed = failed_start_inputs(
            failed_start, deployment, prior, guest, operation, value)
    guest.validate_candidate_transition(commit, artifacts, current['commit'], installed_plan)
    guest.validate_supervisor_update(supervisor, deployment, operation, commit, artifacts)
    if failed_start is not None:
        previous = installed_plan['epoch_supervisor']
        need(previous['original_service_state'] == supervisor['original_service_state']
             and previous['successor_service_state'] == supervisor['successor_service_state']
             and previous['before'] == supervisor['before'],
             'failed supervisor transition changed original operator intent')
        need(supervisor['installed'] == supervisor_installed,
             'supervisor installed binding differs from retained failure observation')
        value['epoch_supervisor_installed'] = supervisor_installed
    else:
        need(supervisor['installed'] == supervisor['before'],
             'initial supervisor installed binding differs from original')
        value['epoch_supervisor_installed'] = supervisor['before']
    guest.configure(value)
    units = []
    for row in installed_plan['units']:
        raw = base64.b64decode(row['after'], validate=True)
        need(sha(raw) == row['after_sha256'], 'installed predecessor unit digest differs')
        after = guest.replace_daemon(raw, row['role'])
        units.append({'role':row['role'], 'before':row['after'], 'after':base64.b64encode(after).decode(),
                      'before_sha256':sha(raw), 'after_sha256':sha(after)})
    value.update(units=units,
        retained_predecessor={'attempt_name':current['attempt_name'],
            'intent_sha256':sha(json.dumps(prior, sort_keys=True, separators=(',', ':')).encode())},
        guest_sha256=sha(read_public(HERE / 'taira_update_guest.py')),
        runner_sha256=sha(read_public(HERE / 'taira_update.py')),
        renderer_sha256=deployment['renderer_sha256'], secret_contents_read=False,
        transaction_submission=supervisor['successor_service_state'] == 'running',
        python_transaction_submission=False)
    unit_renderer = module(HERE / 'taira_epoch_supervisor_unit.py', 'epoch_supervisor_renderer')
    need(unit_renderer.render(supervisor['after']['unit_spec']).decode() == supervisor['after']['unit_bytes'],
         'successor supervisor unit differs from the shared fixed renderer')
    value['epoch_supervisor_renderer_sha256'] = sha(read_public(HERE / 'taira_epoch_supervisor_unit.py'))
    return value


def release_name(plan):
    """Keep every operation's staging independent, including the same candidate."""
    return 'release-' + plan['commit'] + '-' + plan['operation']


def successor_deployment(plan, raw, output):
    """Publish the exact installed operation as the next update's predecessor."""
    successor = json.loads(json.dumps(plan['deployment']))
    successor['current'] = {'commit':plan['commit'],
        'daemon':str(Path(successor['runtime_root']) / release_name(plan) / 'bin/iroha3d_taira'),
        'attempt_name':plan['operation'], 'plan_schema':'taira.daemon-update.plan.v1',
        'result_schema':'taira.daemon-update.result.v1',
        'local_plan':str(output / 'plan.json'), 'local_plan_sha256':sha(raw)}
    return successor


def transfer_code(name, create_release, plan):
    need(name in ('iroha3d_taira', 'iroha', 'kagami'), 'unexpected transfer artifact')
    # stdin is inherited directly from the local artifact descriptor. Python
    # controls descriptors/paths only; /bin/cat owns the binary stream.
    return f'''
import os,stat,fcntl
from pathlib import Path
base=Path({plan['deployment']['runtime_root']!r})
assert os.geteuid()==0 and base.resolve()==base
for ancestor in [base,*base.parents]:
 s=ancestor.lstat();assert stat.S_ISDIR(s.st_mode) and s.st_uid==0 and not s.st_mode&0o022
state=Path('/var/lib/taira-epoch-supervisor')
for ancestor in state.parents:
 s=ancestor.lstat();assert stat.S_ISDIR(s.st_mode) and s.st_uid==0 and not s.st_mode&0o022
state.mkdir(mode=0o700,exist_ok=True)
s=state.lstat();assert state.resolve()==state and stat.S_ISDIR(s.st_mode) and s.st_uid==s.st_gid==0 and stat.S_IMODE(s.st_mode)==0o700
lock_path=state/'.deployment.lock'
lock=os.open(lock_path,os.O_RDWR|os.O_CREAT|os.O_NOFOLLOW,0o600)
s=os.fstat(lock);assert stat.S_ISREG(s.st_mode) and s.st_uid==s.st_gid==0 and s.st_nlink==1 and stat.S_IMODE(s.st_mode)==0o600
fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB)
t=lock_path.lstat();assert (s.st_dev,s.st_ino)==(t.st_dev,t.st_ino)
assert not os.path.lexists(state/'.reset-owner.json')
os.set_inheritable(lock,True)
release=base/{release_name(plan)!r}
bins=release/'bin'
if {create_release!r}:
 release.mkdir(mode=0o700);bins.mkdir(mode=0o700)
else:
 for p in (release,bins):
  s=p.lstat();assert p.resolve()==p and stat.S_ISDIR(s.st_mode) and s.st_uid==0 and stat.S_IMODE(s.st_mode)==0o700
fd=os.open(bins/{name!r},os.O_WRONLY|os.O_CREAT|os.O_EXCL|os.O_NOFOLLOW,0o755)
os.fchmod(fd,0o755)
os.dup2(fd,1);os.close(fd)
os.execv('/bin/cat',['/bin/cat'])
'''


def artifact_transfer_argv(approved, remote_transfer):
    need(approved[0] == '/usr/bin/ssh', 'qualified SSH executable differs')
    return [approved[0], '-C', *approved[1:-1], remote_transfer]


def retained_artifacts(artifacts):
    """Admit local immutable producer outputs without reading binary bodies."""
    retained = []
    for artifact in artifacts:
        path = Path(artifact['path'])
        info = path.lstat()
        need(path.resolve() == path and stat.S_ISREG(info.st_mode) and info.st_uid == os.getuid()
             and info.st_nlink == 1 and not info.st_mode & 0o222 and info.st_size == artifact['size'],
             'candidate must be the retained read-only build artifact')
        digest = subprocess.check_output(['/usr/bin/shasum', '-a', '256', str(path)], text=True).split()[0]
        need(digest == artifact['sha256'], 'retained candidate artifact differs')
        retained.append((artifact, path))
    return retained


def verify_prepared_remote(plan, argv, output):
    source = read_public(HERE / 'taira_update_guest.py')
    payload = source + b'\nverify_prepared_artifacts(' + repr(plan).encode() + b')\n'
    with (output / 'prepared-artifacts.stdout.json').open('xb') as out, \
         (output / 'prepared-artifacts.stderr').open('xb') as err:
        result = subprocess.run(argv, input=payload, stdout=out, stderr=err, timeout=300)
    need(result.returncode == 0, 'preprovisioned candidate verification failed; no missing-file fallback')
    report = retry.decode(read_public(output / 'prepared-artifacts.stdout.json'))
    need(report == {'schema': 'taira.prepared-update-artifacts.v1',
        'operation': plan['operation'], 'commit': plan['commit'],
        'artifacts': plan['artifacts'], 'runtime_mutated': False},
        'native prepared artifact verification receipt differs')
    return report


def prepare_artifacts(args, deployment, build_raw):
    """Explicit create-new artifact phase before native generation materialization."""
    build = retry.decode(build_raw)
    artifacts = validate_build(build, build['commit'])
    need(re.fullmatch('update-[0-9a-f]{32}', args.operation)
         and args.operation != deployment['current']['attempt_name']
         and build['commit'] != deployment['current']['commit'], 'fresh candidate operation required')
    need(not args.output.exists(), 'fresh artifact preparation output required')
    plan = {'schema': 'taira.update-artifact-preparation.v1', 'operation': args.operation,
        'commit': build['commit'], 'deployment': deployment, 'artifacts': artifacts,
        'build_result_path': str(args.prepared_result), 'build_result_sha256': sha(build_raw),
        'runner_sha256': sha(read_public(HERE / 'taira_update.py')),
        'guest_sha256': sha(read_public(HERE / 'taira_update_guest.py'))}
    retained = retained_artifacts(artifacts)
    argv = retry.validate_ssh(deployment['guest_ssh'])
    args.output.mkdir(mode=0o700)
    write_new(args.output / 'artifact-preparation.json', (json.dumps(plan, sort_keys=True) + '\n').encode())
    for index, (artifact, path) in enumerate(retained):
        remote = shlex.join(['/usr/bin/python3', '-I', '-c', transfer_code(artifact['name'], index == 0, plan)])
        with path.open('rb') as source, (args.output / (artifact['name'] + '-transfer.stderr')).open('xb') as error:
            result = subprocess.run(artifact_transfer_argv(argv, remote), stdin=source,
                stdout=subprocess.DEVNULL, stderr=error, timeout=300)
        need(result.returncode == 0, 'native artifact preparation failed; retain partial release for inspection')
        write_new(args.output / (artifact['name'] + '-transfer.json'),
            json.dumps({'exit_code': 0, 'name': artifact['name'], 'size': artifact['size']}).encode())
    report = verify_prepared_remote(plan, argv, args.output)
    print(json.dumps(report | {'next_action': 'native epoch-supervisor-host materialize using the prepared candidate CLI'}))


def apply_plan(args):
    raw = read_public(args.plan)
    need(sha(raw) == args.plan_sha256, 'reviewed plan digest differs')
    plan = json.loads(raw)
    need(plan.get('schema') == 'taira.daemon-update.plan.v1', 'plan schema differs')
    validate_deployment(plan['deployment'])
    build_raw = read_public(Path(plan['build_result_path']))
    need(sha(build_raw) == plan['build_result_sha256'], 'bound preparation result changed')
    need(validate_build(json.loads(build_raw), plan['commit']) == plan['artifacts'],
         'plan artifacts differ from the bound completed preparation')
    for name, field in [('taira_update_guest.py', 'guest_sha256'), ('taira_update.py', 'runner_sha256')]:
        need(sha(read_public(HERE / name)) == plan[field], 'reviewed coordinator source changed')
    need(sha(read_public(ROOT / 'scripts/taira_validator_unit.py')) == plan['renderer_sha256'],
         'reviewed custody renderer changed')
    need(sha(read_public(HERE / 'taira_epoch_supervisor_unit.py')) == plan['epoch_supervisor_renderer_sha256'],
         'reviewed supervisor renderer changed')
    guest = module(HERE / 'taira_update_guest.py', 'runtime_update_supervisor_validation')
    guest.validate_supervisor_update(plan['epoch_supervisor'], plan['deployment'],
                                    plan['operation'], plan['commit'], plan['artifacts'])
    if 'failed_start' in plan:
        current = plan['deployment']['current']
        prior = retry.decode(retry.public_record(current['local_plan'], current['local_plan_sha256']))
        guest = module(HERE / 'taira_update_guest.py', 'runtime_update_recovery_validation')
        reference = {key: value for key, value in plan['failed_start'].items() if key != 'installed'}
        rebound = make_plan(json.loads(build_raw), plan['deployment'], prior, guest,
                            plan['operation'], reference, supervisor=plan['epoch_supervisor'])
        need(rebound['failed_start'] == plan['failed_start'] and rebound['units'] == plan['units']
             and rebound['retained_predecessor'] == plan['retained_predecessor'],
             'failed-start recovery plan differs from its public inputs')
        need(rebound['epoch_supervisor_installed'] == plan['epoch_supervisor_installed'],
             'failed-start supervisor installed closure differs')
    need(args.output.is_absolute() and args.output.parent.resolve() == args.output.parent
         and not args.output.exists(), 'fresh absolute local output required')
    argv = retry.validate_ssh(plan['deployment']['guest_ssh'])
    artifacts = plan['artifacts']
    need([row['name'] for row in artifacts] == ['iroha3d_taira', 'iroha', 'kagami'],
         'exact same-release daemon, CLI and Kagami required')
    retained_artifacts(artifacts)
    args.output.mkdir(mode=0o700)
    write_new(args.output / 'plan.json', raw)
    verify_prepared_remote(plan, argv, args.output)
    guest_source = read_public(HERE / 'taira_update_guest.py')
    payload = guest_source + b'\napply_locked(' + repr(plan).encode() + b')\n'
    with (args.output / 'stdout.json').open('xb') as out, (args.output / 'stderr.log').open('xb') as err:
        # Bound the complete guest operation beyond its individual preflight,
        # stop/start, progress-bounded catch-up, doctor and final observation budgets.
        process = subprocess.run(argv, input=payload, stdout=out, stderr=err,
                                 timeout=GUEST_OPERATION_TIMEOUT_SECONDS)
    write_new(args.output / 'exit.json', json.dumps({'exit_code': process.returncode}).encode())
    need(process.returncode == 0, 'guest update failed; inspect owner-private attempt, never blindly reapply')
    result = json.loads(read_public(args.output / 'stdout.json'))
    need(result.get('runtime_update_complete') is True and result.get('commit') == plan['commit']
         and result.get('network_id') == plan['network_id']
         and result.get('canary_applied_verified') is False, 'guest result differs')
    successor = successor_deployment(plan, raw, args.output)
    retry.write_public(args.output / 'next-deployment.json', successor)
    print(json.dumps(result | {'next_deployment':str(args.output / 'next-deployment.json')}))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--deployment', type=Path, required=True)
    parser.add_argument('--prepared-result', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--operation', required=True,
                        help='explicit fresh update-<32hex> already bound by supervisor preparation')
    parser.add_argument('--supervisor-plan', type=Path,
                        help='public immutable preprovisioned supervisor transition and native receipt')
    parser.add_argument('--prepare-artifacts', action='store_true',
                        help='create and verify the exact three candidate binaries before native materialize')
    parser.add_argument('--plan-only', action='store_true', help='write the exact local plan without SSH')
    parser.add_argument('--failed-start-chain', type=Path,
                        help='ordered digest-bound failed attempts since the last completed deployment')
    args = parser.parse_args()
    need((args.prepare_artifacts and args.supervisor_plan is None and not args.plan_only
          and args.failed_start_chain is None)
         or (not args.prepare_artifacts and args.supervisor_plan is not None),
         'prepare-artifacts is separate; normal apply and plan-only require a supervisor plan')
    os.umask(0o077)
    need(subprocess.check_output(['git', 'branch', '--show-current'], cwd=ROOT, text=True).strip()
         == 'optimizations', 'only optimizations is allowed')
    deployment = validate_deployment(retry.decode(read_public(args.deployment)))
    parent = args.output.parent
    info = parent.lstat()
    need(args.output.is_absolute() and parent.resolve() == parent and stat.S_ISDIR(info.st_mode)
         and info.st_uid == os.getuid() and stat.S_IMODE(info.st_mode) == 0o700,
         'fresh output below owner-private directory required')
    lock = os.open(args.deployment.parent / '.taira-update.lock',
                   os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW, 0o600)
    try:
        info = os.fstat(lock)
        need(stat.S_ISREG(info.st_mode) and info.st_uid == os.getuid() and info.st_nlink == 1
             and stat.S_IMODE(info.st_mode) == 0o600, 'invalid local deployment lock')
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        build_raw = read_public(args.prepared_result)
        if args.prepare_artifacts:
            prepare_artifacts(args, deployment, build_raw)
            return
        prior_raw = retry.public_record(deployment['current']['local_plan'],
                                        deployment['current']['local_plan_sha256'])
        guest = module(HERE / 'taira_update_guest.py', 'runtime_update_guest')
        value = make_plan(retry.decode(build_raw), deployment, retry.decode(prior_raw), guest,
                          args.operation,
                          retry.decode(read_public(args.failed_start_chain))
                          if args.failed_start_chain is not None else None,
                          supervisor=retry.decode(read_public(args.supervisor_plan)))
        value.update(build_result_path=str(args.prepared_result), build_result_sha256=sha(build_raw))
        raw = (json.dumps(value, sort_keys=True)+'\n').encode()
        if args.plan_only:
            write_new(args.output, raw)
            print(json.dumps({'plan':str(args.output), 'host_contacted':False}))
        else:
            path = parent / (value['operation']+'.plan.json')
            write_new(path, raw)
            args.plan, args.plan_sha256 = path, sha(raw)
            apply_plan(args)
    finally:
        os.close(lock)


if __name__ == '__main__':
    main()
