"""Private files and pipe-backed fake children; no native process is launched."""
from contextlib import ExitStack
import hashlib
import json
import os
from pathlib import Path
import struct
import sys
from types import SimpleNamespace

import pytest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts/nexus'))
import scaling_command as command
import scaling_launcher as launcher
import scaling_readiness as readiness
import scaling_readiness_inputs as inputs
import resource_process as process
from scaling_launcher_test import Clock, Factory


def compact(value):
    return json.dumps(value, separators=(',', ':')).encode() + b'\n'


def write_inputs(directory):
    directory.mkdir(mode=0o700)
    genesis = 'hash:' + '01' * 32 + '#ABCD'
    context = 'hash:' + '03' * 32 + '#ABCD'
    value = dict(schema=inputs.ANCHORS_SCHEMA, version=1, lane_count=4, genesis_hash=genesis,
                 context_id=context, network_id=genesis, consensus_mode='npos', chain_id='test-chain',
                 genesis_public_key='ed0120' + 'AB' * 32, chain_discriminant=0,
                 peers=[], accounts=[], artifacts=[])
    files = {'genesis.json': b'{}', 'genesis.signed.nrt': b'test-signed-native-genesis',
             'genesis-context.nrt': b'test-native-context', 'genesis.expected_hash': b'01' * 32,
             'client.toml': b'chain = "test-chain"\n'}
    for index in range(4):
        role = f'peer{index}'
        key = 'ea0130' + f'{index + 1:02X}' * 48
        url = f'http://127.0.0.1:{8080 + index}/'
        store = directory / f'kura{index}'
        value['peers'].append(dict(role=role, node_public_key=key, torii_url=url,
            config=f'{role}.toml', client_config=f'{role}-client.toml',
            primary_block_store=str(store / 'blocks/native-primary'),
            primary_merge_log=str(store / 'merge_ledger/native-primary.log')))
        store.mkdir(mode=0o700)
        files[f'{role}.toml'] = (f'chain = "test-chain"\npublic_key = "{key}"\n'
            f'private_key = "PRIVATE TEST ONLY"\n[kura]\nstore_dir = {json.dumps(str(store))}\n').encode()
        files[f'{role}-client.toml'] = (f'chain = "test-chain"\nnetwork_id = "{genesis}"\n'
            f'torii_url = "{url}"\nprivate_key = "PRIVATE TEST ONLY"\n').encode()
        name = f'workload-account-{index:02}.toml'
        value['accounts'].append(dict(index=index, account_id=f'test-account-{index}', config=name))
        files[name] = b'chain = "test-chain"\n'
    for name, raw in files.items():
        path = directory / name
        path.write_bytes(raw); path.chmod(0o600)
        value['artifacts'].append(dict(path=name, sha256=hashlib.sha256(raw).hexdigest(), bytes=len(raw)))
    seal_anchor(directory, value)
    return value


def seal_anchor(directory, value):
    raw = compact(value)
    path = directory / 'genesis-anchors.json'
    path.write_bytes(raw); path.chmod(0o600)
    return hashlib.sha256(raw).hexdigest()


def refresh_artifact(directory, value, name):
    raw = (directory / name).read_bytes()
    row = next(row for row in value['artifacts'] if row['path'] == name)
    row.update(sha256=hashlib.sha256(raw).hexdigest(), bytes=len(raw))
    return seal_anchor(directory, value)


class PipeChild:
    def __init__(self, factory, index, raw, diagnostic):
        self.factory, self.index, self.pid = factory, index, 2000 + index
        self.returncode, self.status = None, None
        self.drift, self.stall = False, False
        self.writers = []
        self.stdout, self.stderr = self.pipe(raw), self.pipe(diagnostic)
        self.wait_hook = lambda: None

    def pipe(self, raw):
        read, write = os.pipe()
        if raw is None:
            self.writers.append(write)
        else:
            assert os.write(write, raw) == len(raw)
            os.close(write)
        return os.fdopen(read, 'rb', buffering=0)

    def poll(self):
        return self.returncode

    def wait(self, timeout):
        self.factory.events.append(('wait', self.index, timeout))
        self.wait_hook()
        if self.stall:
            raise command.subprocess.TimeoutExpired('PRIVATE TEST ONLY', timeout)
        self.returncode = self.status if self.status is not None else 0
        return self.returncode

    def terminate(self):
        self.factory.events.append(('terminate', self.index))
        if not self.stall:
            self.status = 0

    def close(self):
        self.stdout.close(); self.stderr.close()
        for fd in self.writers: os.close(fd)
        self.writers.clear()


class CommandFactory:
    def __init__(self, clock, anchors):
        self.clock, self.anchors = clock, anchors
        self.calls, self.children, self.events = [], [], []
        self.reports, self.raws, self.diagnostics = [], {}, {}
        self.after_spawn = lambda child: None
        self.fail_spawn = None
        self.fail_pin = False

    def ready(self, argv):
        key = argv[argv.index('--node-public-key') + 1]
        return dict(version=1, state='ready', reason=None, challenge=argv[argv.index('--challenge') + 1],
            node_id=key, network_id=self.anchors['network_id'], genesis_hash=self.anchors['genesis_hash'],
            context_id=self.anchors['context_id'], attestation_norito_base64='bmF0aXZlLXRlc3QtYnl0ZXM=')

    def __call__(self, argv, **kwargs):
        index = len(self.calls)
        self.calls.append((argv, kwargs))
        if index == self.fail_spawn: raise OSError('PRIVATE TEST ONLY')
        value = self.ready(argv) if '--challenge' in argv else {}
        if index < len(self.reports): value.update(self.reports[index])
        child = PipeChild(self, index, self.raws.get(index, compact(value)), self.diagnostics.get(index, b''))
        self.children.append(child)
        self.after_spawn(child)
        return child

    def sample(self, pid, image):
        if self.fail_pin: raise process.ProcessObservationError('PRIVATE TEST ONLY')
        child = next(child for child in self.children if child.pid == pid)
        image.validate()
        identity = process.ProcessIdentity(pid, os.geteuid(), 80 + int(child.drift),
                                           1, 500, next(iter(image.uuids)).hex(), image.sha256)
        return process.ProcessSample(identity, 4096)


@pytest.fixture
def ready_setup(tmp_path, monkeypatch, request):
    root = tmp_path.resolve()
    directory = root / 'inputs'
    anchors = write_inputs(directory)
    clock, daemons = Clock(), Factory()
    commands = CommandFactory(clock, anchors)
    monkeypatch.setattr(command.time, 'monotonic_ns', clock)
    monkeypatch.setattr(command.time, 'sleep', lambda delay: setattr(clock, 'now', clock.now + int(delay * 1e9)))
    monkeypatch.setattr(os, 'kill', lambda *_: pytest.fail('no numeric PID signal'))
    monkeypatch.setattr(os, 'killpg', lambda *_: pytest.fail('no process group signal'))
    nonce = iter(range(1, 1000))
    monkeypatch.setattr(readiness.secrets, 'token_hex', lambda size: f'{next(nonce):064x}')
    images = []
    with ExitStack() as stack:
        for name, tag in [('iroha3d', 1), ('iroha', 2)]:
            image_path = root / name
            raw = (struct.pack('<8I', 0xFEEDFACF, 0x100000C, 0, 2, 1, 24, 0, 0)
                   + struct.pack('<II', 0x1B, 24) + bytes(range(tag, tag + 16)))
            image_path.write_bytes(raw); image_path.chmod(0o700)
            images.append(stack.enter_context(process.ExecutableImage(image_path, hashlib.sha256(raw).hexdigest())))
        native = stack.enter_context(inputs.ReadinessInputs(directory, hashlib.sha256(
            (directory / 'genesis-anchors.json').read_bytes()).hexdigest(), 4))
        peers = tuple(launcher.PeerLaunch(role.peer_id, role.node_config,
            launcher.blake3.blake3(role.node_config.read_bytes()).hexdigest(), role.block_store) for role in native.roles)
        peer_inputs = stack.enter_context(launcher.PeerLaunchInputs(peers))
        checks = []
        def verify_runtime(): checks.append(clock.now)
        run = launcher.FourPeerRun(peers, images[0], daemons, peer_inputs, native.validate, clock.end(getattr(request, 'param', 600)))
        step = readiness.FourPeerReadiness(native, images[1], commands, run._trial_deadline_ns, verify_runtime)
        def spawn(argv, **kwargs):
            return (daemons if argv[0] == str(images[0].path) else commands)(argv, **kwargs)
        monkeypatch.setattr(command.subprocess, 'Popen', spawn)
        yield SimpleNamespace(root=root, directory=directory, anchors=anchors, clock=clock, daemons=daemons,
            commands=commands, images=images, inputs=native, peers=peers, run=run, step=step, runtime_checks=checks)
    for child in commands.children: child.close()
