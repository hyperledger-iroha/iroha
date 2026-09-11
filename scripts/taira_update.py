#!/usr/bin/env python3
"""Install completed Taira artifacts while preserving the current ledger and custody.

Requires completed maintained preparation and an owner-public deployment record.
--plan-only contacts no host. The default command
transfers the daemon and matching CLI via native cat/SSH and executes the reviewed guest
controller. No secret files are read. Failed attempts are never overwritten.
"""
import argparse
import fcntl
import secrets
from urllib.parse import urlsplit
import taira_retry as retry
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
    for name, package in [('iroha3d_taira', 'irohad'), ('iroha', 'iroha_cli')]:
        artifact = next(row for row in rows if row['name'] == name)
        need(artifact.get('package') == package and re.fullmatch('[0-9a-f]{64}', artifact.get('sha256', ''))
             and type(artifact.get('size')) is int and 1_000_000 < artifact['size'] < 1024 ** 3,
             'invalid candidate artifact identity: ' + name)
        selected.append(artifact)
    return selected


def make_plan(build, deployment, prior, guest, operation):
    commit = build['commit']
    artifacts = validate_build(build, commit)
    current = deployment['current']
    need(commit != current['commit'], 'candidate is already the current runtime')
    need(re.fullmatch('update-[0-9a-f]{32}', operation), 'invalid operation directory')
    need(prior.get('schema') == current['plan_schema'] and prior.get('commit') == current['commit']
         and prior.get('network_id') == deployment['network_id'], 'installed predecessor plan differs')
    need(sha(read_public(ROOT / 'scripts/taira_validator_unit.py')) == deployment['renderer_sha256']
         == prior['renderer_sha256'], 'reviewed custody renderer changed')
    need([row['role'] for row in prior['units']] == deployment['roles'], 'predecessor cohort differs')
    value = {'schema':'taira.daemon-update.plan.v1', 'commit':commit,
             'network_id':deployment['network_id'], 'artifacts':artifacts,
             'operation':operation, 'deployment':deployment}
    guest.configure(value)
    units = []
    for row in prior['units']:
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
        transaction_submission=False)
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
    need(name in ('iroha3d_taira', 'iroha'), 'unexpected transfer artifact')
    # stdin is inherited directly from the local artifact descriptor. Python
    # controls descriptors/paths only; /bin/cat owns the binary stream.
    return f'''
import os,stat
from pathlib import Path
base=Path({plan['deployment']['runtime_root']!r})
assert os.geteuid()==0 and base.resolve()==base
for ancestor in [base,*base.parents]:
 s=ancestor.lstat();assert stat.S_ISDIR(s.st_mode) and s.st_uid==0 and not s.st_mode&0o022
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
    need(args.output.is_absolute() and args.output.parent.resolve() == args.output.parent
         and not args.output.exists(), 'fresh absolute local output required')
    argv = retry.validate_ssh(plan['deployment']['guest_ssh'])
    artifacts = plan['artifacts']
    need([row['name'] for row in artifacts] == ['iroha3d_taira', 'iroha'],
         'exact candidate daemon and same-revision CLI required')
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
    args.output.mkdir(mode=0o700)
    write_new(args.output / 'plan.json', raw)
    for index, (artifact, path) in enumerate(retained):
        remote_transfer = shlex.join(['/usr/bin/python3', '-I', '-c',
                                      transfer_code(artifact['name'], index == 0, plan)])
        with path.open('rb') as source, (args.output / (artifact['name'] + '-transfer.stderr')).open('xb') as error:
            transferred = subprocess.run(artifact_transfer_argv(argv, remote_transfer), stdin=source,
                                         stdout=subprocess.DEVNULL, stderr=error, timeout=300)
        need(transferred.returncode == 0, 'native transfer failed; preserve partial candidate release and inspect')
        write_new(args.output / (artifact['name'] + '-transfer.json'),
                  json.dumps({'exit_code': 0, 'name': artifact['name'], 'size': artifact['size']}).encode())
    guest_source = read_public(HERE / 'taira_update_guest.py')
    payload = guest_source + b'\napply_locked(' + repr(plan).encode() + b')\n'
    with (args.output / 'stdout.json').open('xb') as out, (args.output / 'stderr.log').open('xb') as err:
        process = subprocess.run(argv, input=payload, stdout=out, stderr=err, timeout=900)
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
    parser.add_argument('--plan-only', action='store_true', help='write the exact local plan without SSH')
    args = parser.parse_args()
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
        prior_raw = retry.public_record(deployment['current']['local_plan'],
                                        deployment['current']['local_plan_sha256'])
        guest = module(HERE / 'taira_update_guest.py', 'runtime_update_guest')
        value = make_plan(retry.decode(build_raw), deployment, retry.decode(prior_raw), guest,
                          'update-' + secrets.token_hex(16))
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
