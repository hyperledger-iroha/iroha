#!/usr/bin/env python3
"""Install or run owner-only macOS nginx log rotation with system newsyslog.

Only the explicitly selected Homebrew nginx access/error logs are admitted.
Rotation renames logs and asks the same owned nginx master to reopen with USR1;
it never truncates an open file, restarts nginx, or reads/prints log contents.
Requires the owning non-root macOS user, newsyslog, lsof and launchctl. The
private state directory holds bounded last-attempt/last-success metadata.
`status` reads this metadata and launchd's separate exit status without writes;
an installed job alone is not evidence of successful rotation. A health write
failure exits nonzero, including when disk exhaustion prevents its recording.
"""
from __future__ import annotations

import argparse
import contextlib
import errno
import fcntl
import hashlib
import json
import os
from pathlib import Path
import plistlib
import re
import signal
import stat
import subprocess
import sys
import tempfile
import time

ALLOWED_LOGS = frozenset({Path('/opt/homebrew/var/log/nginx/access.log'),
                          Path('/opt/homebrew/var/log/nginx/error.log')})
PID_FILE = Path('/opt/homebrew/var/run/nginx.pid')
LABEL = 'org.sora.taira.nginx-logrotate'
NEWSYSLOG = '/usr/sbin/newsyslog'
PYTHON = '/usr/bin/python3'
STATE_LIMIT = 8192
HEALTH_SCHEMA = 'taira.nginx-logrotate-health.v1'
PHASES = frozenset({'admission', 'rotation', 'verification', 'complete'})
REASONS = frozenset({'invariant_failed', 'command_failed', 'command_timeout', 'disk_full',
                     'permission_denied', 'missing_input', 'io_error', 'invalid_settings',
                     'health_write_failed'})


class RotationError(RuntimeError):
    def __init__(self, message, *, reason='invariant_failed'):
        super().__init__(message)
        self.reason = reason


def require(ok, message):
    if not ok:
        raise RotationError(message)


def direct(path):
    require(path.is_absolute() and path.resolve() == path, 'Path must be absolute and contain no symlinks')
    for parent in path.parents:
        info = parent.stat()
        # Homebrew's var directory is administrator-group writable (0775).
        # Admit only root/current-user ancestors, never world-writable ones.
        require(stat.S_ISDIR(info.st_mode) and info.st_uid in (0, os.getuid())
                and not info.st_mode & 0o002, 'Unsafe path ancestor')


def owned_file(path, *, private=False):
    direct(path)
    info = path.lstat()
    require(stat.S_ISREG(info.st_mode) and info.st_uid == os.getuid()
            and info.st_nlink == 1 and not info.st_mode & 0o022, 'Unsafe owned file: ' + path.name)
    if private:
        require(stat.S_IMODE(info.st_mode) == 0o600, 'Configuration must be owner-only')
    return info


def metadata(path):
    info = owned_file(path)
    return {'device': info.st_dev, 'inode': info.st_ino, 'size': info.st_size}


def private_directory(path, *, create=True):
    if create and not path.exists():
        path.mkdir(mode=0o700, parents=True)
    direct(path)
    info = path.stat()
    require(stat.S_ISDIR(info.st_mode) and info.st_uid == os.getuid()
            and stat.S_IMODE(info.st_mode) == 0o700, 'Rotation state must be an owner-only directory')


def selected_logs(paths):
    require(paths and len(paths) == len(set(paths)) and set(paths) <= ALLOWED_LOGS,
            'Select unique explicit nginx access/error log paths only')
    for path in paths:
        info = owned_file(path)
        parent = path.parent.stat()
        require(parent.st_uid == os.getuid() and not parent.st_mode & 0o022,
                'Nginx log directory must be owned and not group/world writable')
        require(info.st_gid in os.getgroups() or info.st_gid == os.getgid(), 'Log group is not owned by this user')
    return sorted(paths)


def command(argv, *, allowed=(0,)):
    result = subprocess.run(argv, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                            env={'PATH': '/usr/bin:/bin:/usr/sbin:/sbin', 'LC_ALL': 'C'}, timeout=600)
    if result.returncode not in allowed:
        # Report useful OS causes without printing arbitrary child output.
        causes = ('No space left on device', 'Permission denied', 'Operation not permitted',
                  'No such file or directory', 'Read-only file system', 'Input/output error', 'No such process')
        detail = '; '.join(cause for cause in causes if cause.encode() in result.stderr)
        reason = 'disk_full' if b'No space left on device' in result.stderr else 'command_failed'
        raise RotationError(f'{Path(argv[0]).name} failed with exit {result.returncode}' + (': ' + detail if detail else ''), reason=reason)
    return result.stdout


def master_identity(pid_file, expected_pid=None):
    require(pid_file == PID_FILE, 'Only the explicit Homebrew nginx PID file is admitted')
    owned_file(pid_file)
    with os.fdopen(os.open(pid_file, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC), 'rb') as stream:
        raw = stream.read(33)
    require(re.fullmatch(rb'[1-9][0-9]{0,9}\n?', raw) is not None, 'Nginx PID file is not one positive process ID')
    pid = int(raw)
    require(pid > 1 and (expected_pid is None or pid == expected_pid), 'Nginx master differs from expected PID')
    output = command(['/bin/ps', '-p', str(pid), '-o', 'uid=', '-o', 'lstart=', '-o', 'comm=']).decode().strip()
    fields = output.split(None, 6)
    require(len(fields) == 7 and fields[0] == str(os.getuid())
            and fields[6].startswith('nginx: master process '), 'PID is not this user’s nginx master')
    return pid, ' '.join(fields[1:6])


def nginx_processes(master):
    output = command(['/bin/ps', '-axo', 'pid=,ppid=,uid=,comm=']).decode()
    result = {master}
    for line in output.splitlines():
        parts = line.strip().split(None, 3)
        if len(parts) == 4 and parts[1] == str(master):
            require(parts[2] == str(os.getuid()) and parts[3].startswith('nginx:'), 'Unexpected nginx child identity')
            result.add(int(parts[0]))
    return result


def open_inodes(pids, master):
    raw = command(['/usr/sbin/lsof', '-nP', '-a', '-p', ','.join(map(str, sorted(pids))), '-FfiD'], allowed=(0, 1))
    observed, inodes, device, inode = set(), set(), None, None
    for line in raw.decode().splitlines():
        if line.startswith(('p', 'f')):
            if device is not None and inode is not None:
                inodes.add((device, inode))
            device, inode = None, None
            if line.startswith('p'):
                observed.add(int(line[1:]))
        elif line.startswith('D'):
            device = int(line[1:], 16)
        elif line.startswith('i'):
            inode = int(line[1:])
    if device is not None and inode is not None:
        inodes.add((device, inode))
    require(master in observed, 'Cannot verify nginx open descriptors')
    return inodes


def archive_metadata(log, count, *, compressed_only=False):
    archives, indices = [], set()
    prefix = log.name + '.'
    for path in log.parent.iterdir():
        if not path.name.startswith(prefix):
            continue
        match = re.fullmatch(re.escape(log.name) + r'\.([0-9]+)(\.gz)?', path.name)
        require(match is not None and int(match[1]) < count, 'Unexpected archive in selected nginx log namespace')
        require(int(match[1]) not in indices, 'Duplicate nginx archive generation')
        indices.add(int(match[1]))
        info = owned_file(path)
        require(not compressed_only or match[2] is not None, 'Uncompressed nginx archive remains; check available disk space')
        require(not compressed_only or stat.S_IMODE(info.st_mode) == 0o600, 'Rotated archive is not owner-only')
        archives.append(path)
    require(len(archives) <= count, 'Nginx archive retention count exceeded')
    return archives


def config_bytes(logs, pid_file, count, size_kib, hours):
    require(2 <= count <= 10 and 1024 <= size_kib <= 1048576 and 1 <= hours <= 168, 'Rotation policy is outside supported bounds (retain 2–10 archives)')
    require(signal.SIGUSR1 == 30, 'This configuration requires macOS USR1 signal 30')
    rows = ['# Exact nginx logs only; rename, reopen with USR1, then gzip.']
    for path in logs:
        require(re.fullmatch(r'/[A-Za-z0-9/_.-]+', str(path)) is not None, 'Log path is not safe newsyslog syntax')
        group = owned_file(path).st_gid
        # macOS newsyslog retains generations 0 through its positive count,
        # inclusive; zero instead disables archiving. Our count is the number
        # of retained archives, matching archive_metadata's strict bound.
        rows.append(f'{path} {os.getuid()}:{group} 600 {count - 1} {size_kib} {hours} BZ {pid_file} 30')
    return ('\n'.join(rows) + '\n').encode()


def write_private(path, data):
    direct(path)
    if path.exists():
        owned_file(path, private=True)
    fd, temporary = tempfile.mkstemp(prefix='.' + path.name + '.', dir=path.parent)
    try:
        with os.fdopen(fd, 'wb') as stream:
            os.fchmod(stream.fileno(), 0o600)
            stream.write(data); stream.flush(); os.fsync(stream.fileno())
        os.replace(temporary, path)
        directory = os.open(path.parent, os.O_RDONLY | os.O_DIRECTORY)
        try:
            os.fsync(directory)
        finally:
            os.close(directory)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)


@contextlib.contextmanager
def rotation_lock(state):
    private_directory(state)
    path = state / 'rotation.lock'
    fd = os.open(path, os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW | os.O_CLOEXEC, 0o600)
    try:
        owned_file(path, private=True)
        try:
            fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as error:
            raise RotationError('Nginx log rotation is already running') from error
        yield
    finally:
        os.close(fd)


def load_settings(state):
    path = state / 'settings.json'
    value = json.loads(read_private(path))
    require(isinstance(value, dict) and set(value) == {'logs', 'pid_file', 'count', 'size_kib', 'hours', 'interval_seconds'},
            'Unexpected rotation settings')
    require(isinstance(value['logs'], list) and all(isinstance(item, str) for item in value['logs'])
            and isinstance(value['pid_file'], str)
            and all(type(value[key]) is int for key in ('count', 'size_kib', 'hours', 'interval_seconds')),
            'Invalid rotation settings types')
    require(2 <= value['count'] <= 10 and 1024 <= value['size_kib'] <= 1048576
            and 1 <= value['hours'] <= 168 and 60 <= value['interval_seconds'] <= 3600,
            'Rotation settings are outside supported bounds')
    return value


def read_private(path):
    """Bounded, stable read of one owner-private state file, never a log."""
    def identity(info):
        return (info.st_dev, info.st_ino, info.st_mode, info.st_uid, info.st_gid, info.st_nlink,
                info.st_size, info.st_mtime_ns, info.st_ctime_ns)

    before = owned_file(path, private=True)
    require(before.st_size <= STATE_LIMIT, 'Rotation state exceeds its size bound')
    with os.fdopen(os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC), 'rb') as stream:
        opened = os.fstat(stream.fileno())
        require(identity(opened) == identity(before), 'Rotation state changed before reading')
        data = stream.read(STATE_LIMIT + 1)
        require(identity(os.fstat(stream.fileno())) == identity(opened), 'Rotation state changed during reading')
    require(len(data) <= STATE_LIMIT and identity(owned_file(path, private=True)) == identity(before),
            'Rotation state changed after reading')
    return data


def empty_health():
    return {'schema': HEALTH_SCHEMA, 'last_attempt': None, 'last_success': None}


def load_health(state):
    try:
        value = json.loads(read_private(state / 'health.json'))
    except FileNotFoundError:
        return empty_health()
    require(isinstance(value, dict) and set(value) == {'schema', 'last_attempt', 'last_success'}
            and value['schema'] == HEALTH_SCHEMA, 'Unexpected rotation health schema')
    for key in ('last_attempt', 'last_success'):
        attempt = value[key]
        if attempt is None:
            continue
        require(isinstance(attempt, dict) and set(attempt) == {
            'started_at_ns', 'finished_at_ns', 'outcome', 'phase', 'reason', 'settings_sha256'},
            'Unexpected rotation health attempt')
        require(type(attempt['started_at_ns']) is int and attempt['started_at_ns'] > 0
                and attempt['outcome'] in ('running', 'succeeded', 'failed')
                and isinstance(attempt['phase'], str) and attempt['phase'] in PHASES
                and (attempt['reason'] is None or (isinstance(attempt['reason'], str) and attempt['reason'] in REASONS))
                and (attempt['settings_sha256'] is None or (isinstance(attempt['settings_sha256'], str)
                     and re.fullmatch('[0-9a-f]{64}', attempt['settings_sha256']) is not None)),
                'Invalid rotation health values')
        finished = attempt['finished_at_ns']
        require((attempt['outcome'] == 'running' and finished is None and attempt['reason'] is None
                 and attempt['phase'] != 'complete')
                or (attempt['outcome'] != 'running' and type(finished) is int and finished >= attempt['started_at_ns']
                    and ((attempt['outcome'] == 'succeeded' and attempt['phase'] == 'complete'
                          and attempt['reason'] is None and attempt['settings_sha256'] is not None)
                         or (attempt['outcome'] == 'failed' and attempt['reason'] is not None))),
                'Inconsistent rotation health outcome')
        require(key != 'last_success' or attempt['outcome'] == 'succeeded', 'Invalid last successful rotation')
    if value['last_success'] is not None:
        require(value['last_attempt'] is not None
                and value['last_success']['started_at_ns'] <= value['last_attempt']['started_at_ns'],
                'Rotation health ordering is inconsistent')
    if value['last_attempt'] is not None and value['last_attempt']['outcome'] == 'succeeded':
        require(value['last_attempt'] == value['last_success'], 'Successful rotation health differs')
    return value


def save_health(state, health):
    raw = (json.dumps(health, sort_keys=True) + '\n').encode()
    require(len(raw) <= STATE_LIMIT, 'Rotation health exceeds its size bound')
    write_private(state / 'health.json', raw)


def settings_digest(settings):
    return hashlib.sha256(json.dumps(settings, sort_keys=True).encode()).hexdigest()


def failure_reason(error):
    if isinstance(error, RotationError):
        return error.reason
    if isinstance(error, subprocess.TimeoutExpired):
        return 'command_timeout'
    if isinstance(error, OSError):
        return {errno.ENOSPC: 'disk_full', errno.EDQUOT: 'disk_full', errno.EACCES: 'permission_denied',
                errno.EPERM: 'permission_denied', errno.ENOENT: 'missing_input'}.get(error.errno, 'io_error')
    return 'invalid_settings' if isinstance(error, ValueError) else 'command_failed'


def rotate_locked(state, *, force=False, expected_pid=None):
    health = load_health(state)
    previous_success = health['last_success']
    attempt = {'started_at_ns': time.time_ns(), 'finished_at_ns': None, 'outcome': 'running',
               'phase': 'admission', 'reason': None, 'settings_sha256': None}
    health['last_attempt'] = attempt
    try:
        save_health(state, health)
        settings = load_settings(state)
        attempt['settings_sha256'] = settings_digest(settings)
        logs = selected_logs([Path(value) for value in settings['logs']])
        pid_file = Path(settings['pid_file'])
        identity = master_identity(pid_file, expected_pid)
        before = {path: metadata(path) for path in logs}
        processes = nginx_processes(identity[0])
        for log in logs:
            archive_metadata(log, settings['count'])
        config = state / 'newsyslog.conf'
        owned_file(config, private=True)
        require(config.read_bytes() == config_bytes(logs, pid_file, settings['count'], settings['size_kib'], settings['hours']),
                'Installed newsyslog config differs from selected logs')
        argv = [NEWSYSLOG, '-r', '-f', str(config)]
        if force:
            argv.append('-F')
        attempt['phase'] = 'rotation'
        save_health(state, health)
        command([*argv, *map(str, logs)])
        attempt['phase'] = 'verification'
        save_health(state, health)
        require(master_identity(pid_file) == identity, 'Nginx master changed during rotation')
        after = {path: metadata(path) for path in logs}
        rotated = [path for path in logs if before[path]['inode'] != after[path]['inode']]
        require(not force or len(rotated) == len(logs), 'Forced rotation did not replace every selected log')
        if rotated:
            for path in rotated:
                require(stat.S_IMODE(path.stat().st_mode) == 0o600, 'Rotated log is not owner-only')
            descriptors = open_inodes(processes | nginx_processes(identity[0]), identity[0])
            require(all((before[path]['device'], before[path]['inode']) not in descriptors for path in rotated),
                    'Nginx still holds an old log inode after reopen')
        for log in logs:
            archive_metadata(log, settings['count'], compressed_only=True)
        attempt.update(finished_at_ns=time.time_ns(), outcome='succeeded', phase='complete')
        health['last_success'] = dict(attempt)
        save_health(state, health)
        return {'rotated_logs': [str(path) for path in rotated], 'nginx_master_pid': identity[0],
                'nginx_master_unchanged': True, 'old_log_descriptors_closed': True, 'log_contents_read': False}
    except (RotationError, OSError, ValueError, subprocess.SubprocessError) as error:
        # A failed final publication must not promote this attempt into the
        # persisted success slot when the subsequent failure write succeeds.
        health['last_success'] = previous_success
        attempt.update(finished_at_ns=time.time_ns(), outcome='failed', reason=failure_reason(error))
        try:
            save_health(state, health)
        except (RotationError, OSError) as health_error:
            raise RotationError(f'Rotation failed ({failure_reason(error)}); health record persistence failed '
                                f'({failure_reason(health_error)}); check scheduler exit status',
                                reason='health_write_failed') from health_error
        raise


def rotate(state, *, force=False, expected_pid=None):
    with rotation_lock(state):
        return rotate_locked(state, force=force, expected_pid=expected_pid)


def scheduler_status():
    """Return only fixed typed launchd fields, never its arbitrary output."""
    service = f'user/{os.getuid()}/{LABEL}'
    child = subprocess.run(['/bin/launchctl', 'print', service], stdin=subprocess.DEVNULL,
                           stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
                           env={'PATH': '/usr/bin:/bin:/usr/sbin:/sbin', 'LC_ALL': 'C'}, timeout=10)
    if child.returncode != 0:
        return {'loaded': False, 'state': 'unavailable', 'last_exit_status': None}
    raw = child.stdout.decode('utf-8', errors='replace')
    states = re.findall(r'^\s*state = (running|not running|waiting)\s*$', raw, re.MULTILINE)
    exits = re.findall(r'^\s*last exit code = (-?[0-9]+)\s*$', raw, re.MULTILINE)
    signals = re.findall(r'^\s*last terminating signal = ([0-9]+)\s*$', raw, re.MULTILINE)
    status = int(exits[0]) if len(exits) == 1 else None
    if len(signals) == 1:
        status = 128 + int(signals[0])
    return {'loaded': True, 'state': states[0] if len(states) == 1 else 'unknown', 'last_exit_status': status}


def status(state):
    private_directory(state, create=False)
    settings = load_settings(state)
    health = load_health(state)
    scheduler = scheduler_status()
    attempt = health['last_attempt']
    now = time.time_ns()
    fresh = bool(attempt and attempt['outcome'] == 'succeeded'
                 and attempt['settings_sha256'] == settings_digest(settings)
                 and 0 <= now - attempt['finished_at_ns'] <= 2 * settings['interval_seconds'] * 1_000_000_000)
    # A failed scheduled attempt may be unable to write health on a full disk.
    # Launchd's exit status is independent evidence and must not be hidden by
    # a still-fresh previous success record.
    scheduler_ok = scheduler['loaded'] and scheduler['state'] in ('running', 'not running', 'waiting') \
        and scheduler['last_exit_status'] == 0
    return {'healthy': fresh and scheduler_ok, 'rotation_verified': fresh, 'health': health,
            'scheduler': scheduler, 'log_contents_read': False}


def launch_agent(state, interval):
    return {'Label': LABEL, 'ProgramArguments': [PYTHON, '-I', '-B', str(state / 'rotate.py'), 'run', '--state-dir', str(state)],
            'StartInterval': interval, 'RunAtLoad': False, 'ProcessType': 'Background',
            'LimitLoadToSessionType': 'Background',
            'StandardOutPath': '/dev/null', 'StandardErrorPath': '/dev/null', 'Umask': 0o077}


def install(args):
    logs = selected_logs(args.log)
    master_identity(args.pid_file, args.expected_pid)
    require(60 <= args.interval_seconds <= 3600, 'Rotation interval must be between 60 and 3600 seconds')
    settings = {'logs': list(map(str, logs)), 'pid_file': str(args.pid_file), 'count': args.count,
                'size_kib': args.size_kib, 'hours': args.hours, 'interval_seconds': args.interval_seconds}
    config = config_bytes(logs, args.pid_file, args.count, args.size_kib, args.hours)
    for log in logs:
        archive_metadata(log, args.count)
    with rotation_lock(args.state_dir):
        write_private(args.state_dir / 'rotate.py', Path(__file__).read_bytes())
        write_private(args.state_dir / 'settings.json', (json.dumps(settings, sort_keys=True) + '\n').encode())
        write_private(args.state_dir / 'newsyslog.conf', config)
        # Verify the installed policy before claiming installation success or
        # replacing the scheduled job. Merely bootstrapping launchd proves no
        # rotation/reopen/compression invariant.
        verification = rotate_locked(args.state_dir, expected_pid=args.expected_pid)
        agents = Path.home() / 'Library/LaunchAgents'
        agents.mkdir(mode=0o700, parents=True, exist_ok=True)
        direct(agents)
        require(agents.stat().st_uid == os.getuid() and not agents.stat().st_mode & 0o022, 'Unsafe LaunchAgents directory')
        plist = agents / (LABEL + '.plist')
        write_private(plist, plistlib.dumps(launch_agent(args.state_dir, args.interval_seconds), sort_keys=True))
        # This host is administered over SSH and has no GUI login session.
        # The user bootstrap domain runs independently of graphical login.
        domain = f'user/{os.getuid()}'
        service = f'{domain}/{LABEL}'
        loaded = subprocess.run(['/bin/launchctl', 'print', service], stdin=subprocess.DEVNULL,
                                stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=10).returncode == 0
        if loaded:
            command(['/bin/launchctl', 'bootout', service])
        command(['/bin/launchctl', 'bootstrap', domain, str(plist)])
    return {'installed': True, 'rotation_verified': True, 'verification': verification,
            'scheduler': {'loaded': True}, 'launch_agent': str(plist), 'selected_logs': list(map(str, logs)),
            'retained_archives_per_log': args.count, 'size_kib': args.size_kib, 'interval_seconds': args.interval_seconds,
            'log_contents_read': False}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='action', required=True)
    default = Path.home() / '.local/share/taira-nginx-logrotate'
    for name in ('install', 'run', 'status'):
        command_parser = commands.add_parser(name)
        command_parser.add_argument('--state-dir', type=Path, default=default)
        if name != 'status':
            command_parser.add_argument('--expected-pid', type=int)
        if name == 'install':
            command_parser.add_argument('--log', type=Path, action='append', required=True)
            command_parser.add_argument('--pid-file', type=Path, required=True)
            command_parser.add_argument('--count', type=int, default=4, help='Retain 2–10 archives per log (default: 4)')
            command_parser.add_argument('--size-kib', type=int, default=32768)
            command_parser.add_argument('--hours', type=int, default=24)
            command_parser.add_argument('--interval-seconds', type=int, default=300)
        elif name == 'run':
            command_parser.add_argument('--force', action='store_true')
    args = parser.parse_args()
    try:
        require(sys.platform == 'darwin' and os.getuid() != 0, 'Run as the owning non-root macOS nginx user')
        require(args.state_dir.is_absolute() and args.state_dir.is_relative_to(Path.home()), 'State directory must be inside the current user home')
        os.umask(0o077)
        if args.action == 'status':
            result = status(args.state_dir)
        elif args.action == 'install':
            result = install(args)
        else:
            result = rotate(args.state_dir, force=args.force, expected_pid=args.expected_pid)
        print(json.dumps(result, sort_keys=True))
        if args.action == 'status' and not result['healthy']:
            return 1
    except (RotationError, OSError, ValueError, subprocess.SubprocessError) as error:
        print('Nginx log rotation stopped: ' + str(error), file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
