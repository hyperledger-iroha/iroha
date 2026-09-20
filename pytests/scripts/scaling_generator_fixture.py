"""Fixed generator fixture with private synthetic artifacts and no OS children."""
from dataclasses import replace
import hashlib
import json
import os
from pathlib import Path
from types import SimpleNamespace

import pytest
from scaling_readiness_fixture import (ready_setup, CommandFactory, PipeChild, compact,
                                      command, inputs)
import scaling_generator as generator

SEED = '1' * 64


def _url_host(value): return f'[{value}]' if ':' in value else value


def consume_seed_pipe(argv, pass_fds):
    """Model child inheritance using an actual duplicate and bounded pipe read."""
    assert '--seed' not in argv and argv.count('--seed-fd') == 1
    fd = int(argv[argv.index('--seed-fd') + 1])
    assert pass_fds == (fd,) and not os.get_blocking(fd)
    inherited = os.dup(fd)
    try:
        raw = os.read(inherited, 65)
        assert len(raw) == 64 and all(byte in b'0123456789abcdef' for byte in raw)
        assert raw != b'0' * 64 and os.read(inherited, 1) == b''
        assert raw.decode('ascii') not in argv
    finally:
        os.close(inherited)


def emit(root, argv):
    """Model only the selected fixed native output contract, without cryptography."""
    assert not list(root.iterdir()), 'native generator requires an initially empty root'
    get = lambda flag: argv[argv.index(flag) + 1]
    count, lanes = int(get('--scaling-accounts')), int(get('--scaling-lanes'))
    chain, bind, public = get('--chain-id'), get('--bind-host'), get('--public-host')
    api, p2p = int(get('--base-api-port')), int(get('--base-p2p-port'))
    genesis, context = 'hash:' + '01' * 32 + '#ABCD', 'hash:' + '03' * 32 + '#ABCD'
    value = dict(schema=inputs.ANCHORS_SCHEMA, version=1, lane_count=lanes, chain_id=chain,
        consensus_mode='npos', genesis_hash=genesis, context_id=context, network_id=genesis,
        genesis_public_key='ed0120' + 'AB' * 32, chain_discriminant=0,
        peers=[], accounts=[], artifacts=[])
    files = {'genesis.json': b'{}', 'genesis.signed.nrt': b'fake-native-signed-genesis',
             'genesis-context.nrt': b'fake-native-context', 'genesis.expected_hash': genesis.encode() + b'\n',
             'client.toml': f'chain = {json.dumps(chain)}\n'.encode()}
    storage = root / 'storage'; storage.mkdir(mode=0o700)
    for index in range(4):
        role = f'peer{index}'
        peer = storage / role; peer.mkdir(mode=0o700)
        kura, state = peer / 'kura', peer / 'state'
        kura.mkdir(mode=0o700); state.mkdir(mode=0o700)
        key, url = 'ea0130' + f'{index + 1:02X}' * 48, f'http://{_url_host(public)}:{api + index}/'
        value['peers'].append(dict(role=role, node_public_key=key, torii_url=url,
            config=f'{role}.toml', client_config=f'{role}-client.toml',
            primary_block_store=str(kura / 'blocks/native-primary'),
            primary_merge_log=str(kura / 'merge_ledger/native-primary.log')))
        files[f'{role}.toml'] = (f'chain = {json.dumps(chain)}\npublic_key = "{key}"\n'
            f'private_key = "PRIVATE TEST ONLY"\n[kura]\nstore_dir = {json.dumps(str(kura))}\n'
            f'[network]\naddress = "addr:{_url_host(bind)}:{p2p + index}#ABCD"\n'
            f'public_address = "addr:{_url_host(public)}:{p2p + index}#ABCD"\n'
            f'[torii]\naddress = "addr:{_url_host(bind)}:{api + index}#ABCD"\n'
            f'data_dir = {json.dumps(str(state / "torii"))}\n').encode()
        for table, values in {
            'snapshot': {'store_dir': state / 'snapshot'},
            'soracloud_runtime': {'state_dir': state / 'soracloud_runtime'},
            'tiered_state': {'cold_store_root': state / 'tiered_state', 'da_store_root': state / 'da_wsv_snapshots'},
            'streaming': {'session_store_dir': state / 'streaming'},
            'network.soranet_handshake.pow': {'revocation_store_path': state / 'soranet/ticket_revocations.norito'},
            'torii.da_ingest': {'replay_cache_store_dir': state / 'torii/da_replay', 'manifest_store_dir': state / 'torii/da_manifests'},
            'sorafs.discovery': {'replay_checkpoint_path': state / 'torii/sorafs_discovery_replay.nrt'},
            'sorafs.storage': {'data_dir': state / 'sorafs'},
            'sorafs.por': {'state_dir': state / 'sorafs/por'},
        }.items():
            text = f'[{table}]\n' + ''.join(f'{field} = {json.dumps(str(path))}\n' for field, path in values.items())
            files[f'{role}.toml'] += text.encode()
        files[f'{role}-client.toml'] = (f'chain = {json.dumps(chain)}\nnetwork_id = "{genesis}"\n'
            f'torii_url = "{url}"\nprivate_key = "PRIVATE TEST ONLY"\n').encode()
    for index in range(count):
        name = f'workload-account-{index:02}.toml'
        value['accounts'].append(dict(index=index, account_id=f'test-account-{index}', config=name))
        files[name] = f'chain = {json.dumps(chain)}\n'.encode()
    for name, raw in files.items():
        path = root / name; path.write_bytes(raw); path.chmod(0o600)
        value['artifacts'].append(dict(path=name, sha256=hashlib.sha256(raw).hexdigest(), bytes=len(raw)))
    return value


def refresh(root, value):
    for artifact in value['artifacts']:
        path = root / artifact['path']
        if path.is_file():
            raw = path.read_bytes(); artifact.update(sha256=hashlib.sha256(raw).hexdigest(), bytes=len(raw))
    raw = compact(value)
    path = root / 'genesis-anchors.json'; path.write_bytes(raw); path.chmod(0o600)
    return raw


class GeneratorFactory(CommandFactory):
    def __init__(self, clock):
        super().__init__(clock, {})
        self.change_files = lambda root, value: None
        self.after_files = lambda root, value: None
        self.transform_stdout = lambda raw: raw

    def __call__(self, argv, **kwargs):
        index = len(self.calls); self.calls.append((argv, kwargs))
        if index == self.fail_spawn: raise OSError('PRIVATE TEST ONLY')
        consume_seed_pipe(argv, kwargs['pass_fds'])
        root = Path(argv[argv.index('--out-dir') + 1])
        value = emit(root, argv)
        self.change_files(root, value)
        raw = refresh(root, value)
        self.after_files(root, value)
        child = PipeChild(self, index, b'x', self.diagnostics.get(index, b''))
        child.generator_stdout = self.transform_stdout(raw)
        self.children.append(child)
        self.after_spawn(child)
        return child


@pytest.fixture
def generator_setup(ready_setup, monkeypatch):
    c = ready_setup
    factory = GeneratorFactory(c.clock)
    monkeypatch.setattr(command.subprocess, 'Popen', factory)
    original_read = os.read
    def read(fd, count):
        for child in factory.children:
            if not child.stdout.closed and fd == child.stdout.fileno():
                result, child.generator_stdout = child.generator_stdout[:count], child.generator_stdout[count:]
                return result
        return original_read(fd, count)
    monkeypatch.setattr(os, 'read', read)
    plan = generator.GeneratorPlan('test-chain', 4, 4, '127.0.0.1', '127.0.0.1', 8080, 1337)
    checks, owners = [], []
    def create(plan=plan, output=None, guard=None, end=None):
        owner = generator.FixedGenerator(plan, output or c.root / f'generated-{len(owners)}',
            c.images[1], factory, c.clock.end(600) if end is None else end,
            (lambda: checks.append(c.clock.now)) if guard is None else guard)
        owners.append(owner)
        return owner
    yield SimpleNamespace(case=c, factory=factory, plan=plan, create=create, checks=checks,
                          root=c.root, clock=c.clock, image=c.images[1])
    for child in factory.children:
        # All children are test-owned fake objects; never signal an OS process.
        child.returncode = 0; child.close()
    for owner in owners:
        owner.close()
