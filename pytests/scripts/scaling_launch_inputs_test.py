"""Real filesystem admission and mutation controls for the four-node launcher.

Child/process operations use the existing explicit fakes. No native executable,
network, or signal is used; file ownership, descriptors and BLAKE3 are real.
"""
from dataclasses import replace
import json
import os
from pathlib import Path

import pytest

from scaling_launcher_test import setup
import scaling_launcher as launcher


@pytest.mark.parametrize('store', ['missing', 'other', 'relative', 'alias', 'wrong_type', 'extends'])
def test_config_must_select_the_exact_retained_store_before_spawn(setup, store):
    _, factory, _, peers, *_ = setup
    selected = {
        'missing': '',
        'other': f'[kura]\nstore_dir = {json.dumps(str(peers[1].block_store))}\n',
        'relative': '[kura]\nstore_dir = "./kura0"\n',
        'alias': f'[kura]\nstore_dir = {json.dumps(str(peers[0].block_store) + "/.")}\n',
        'wrong_type': '[kura]\nstore_dir = true\n',
        'extends': f'extends = "elsewhere.toml"\n[kura]\nstore_dir = {json.dumps(str(peers[0].block_store))}\n',
    }[store]
    peers[0].config.write_text(selected or 'private_key = "test-only"\n')
    peers = (replace(peers[0], config_blake3=launcher.blake3.blake3(
        peers[0].config.read_bytes()).hexdigest()), *peers[1:])
    with pytest.raises(launcher.LauncherError):
        launcher.PeerLaunchInputs(peers)
    assert factory.calls == []


@pytest.mark.parametrize('change', ['digest', 'empty', 'oversized', 'mode', 'hardlink', 'symlink'])
def test_config_admission_rejects_invalid_original_bytes_or_file(setup, change):
    _, factory, _, peers, *_ = setup
    config = peers[0].config
    if change == 'digest':
        peers = (replace(peers[0], config_blake3='f' * 64), *peers[1:])
    elif change == 'empty':
        config.write_bytes(b'')
    elif change == 'oversized':
        with config.open('r+b') as output:
            output.truncate(launcher._MAX_CONFIG_BYTES + 1)
    elif change == 'mode':
        config.chmod(0o640)
    elif change == 'hardlink':
        os.link(config, config.with_suffix('.link'))
    elif change == 'symlink':
        saved = config.with_suffix('.original')
        config.rename(saved)
        config.symlink_to(saved)
    with pytest.raises(launcher.LauncherError):
        launcher.PeerLaunchInputs(peers)
    assert factory.calls == []


@pytest.mark.parametrize('at', range(4))
def test_noop_runtime_verifier_cannot_authorize_equal_byte_config_replacement(setup, at):
    owner, factory, _, peers, *_ = setup
    owner._verify_inputs = lambda: None
    config = peers[at].config
    raw = config.read_bytes()
    config.rename(config.with_suffix('.original'))
    config.write_bytes(raw)
    config.chmod(0o600)
    assert launcher.blake3.blake3(config.read_bytes()).hexdigest() == peers[at].config_blake3
    with pytest.raises(launcher.LauncherError):
        owner.launch_owned()
    assert factory.calls == []


def test_callback_mutation_is_rechecked_before_any_spawn(setup):
    owner, factory, _, peers, *_ = setup
    def mutate():
        peers[0].config.write_text('private_key = "changed-test-only"\n')
    owner._verify_inputs = mutate
    with pytest.raises(launcher.LauncherError):
        owner.launch_owned()
    assert factory.calls == []


def test_store_contents_may_grow_but_original_directory_cannot_be_replaced(setup):
    owner, factory, _, peers, *_ = setup
    owner._verify_inputs = lambda: None
    (peers[3].block_store / 'block').write_bytes(b'test-only body')
    owner.launch_owned()
    peers[3].block_store.rename(peers[3].block_store.with_suffix('.original'))
    peers[3].block_store.mkdir(mode=0o700)
    calls = []
    with pytest.raises(launcher.LauncherError):
        owner.run_load(lambda _: calls.append('load'))
    assert calls == [] and len(factory.calls) == 4


def test_failed_input_owner_stays_failed_after_restoring_original_bytes(setup):
    owner, _, _, peers, *_ = setup
    raw = peers[0].config.read_bytes()
    peers[0].config.write_bytes(raw + b'# changed\n')
    with pytest.raises(launcher.LauncherError):
        owner._inputs.validate(peers)
    peers[0].config.write_bytes(raw)
    with pytest.raises(launcher.LauncherError):
        owner._inputs.validate(peers)
    with pytest.raises(launcher.LauncherError):
        owner._inputs.__init__(peers)


def test_input_close_releases_every_original_descriptor_and_is_idempotent(setup):
    _, _, _, peers, *_ = setup
    inputs = launcher.PeerLaunchInputs(peers)
    descriptors = [row[0] for row in inputs._files]
    descriptors += [row[0] for row in inputs._directories.values()]
    assert len(descriptors) == len(set(descriptors))
    inputs.close()
    inputs.close()
    for descriptor in descriptors:
        with pytest.raises(OSError):
            os.fstat(descriptor)
    with pytest.raises(launcher.LauncherError):
        inputs.validate(peers)
    with pytest.raises(launcher.LauncherError):
        inputs.__enter__()


def test_input_admission_failure_closes_partial_original_descriptors(setup, monkeypatch):
    _, _, _, peers, *_ = setup
    invalid = (*peers[:3], replace(peers[3], config_blake3='f' * 64))
    opened = []
    real_open = os.open
    def capture_open(*args, **kwargs):
        descriptor = real_open(*args, **kwargs)
        opened.append(descriptor)
        return descriptor
    with monkeypatch.context() as patch:
        patch.setattr(launcher.os, 'open', capture_open)
        with pytest.raises(launcher.LauncherError):
            launcher.PeerLaunchInputs(invalid)
    assert len(opened) > 4
    for descriptor in set(opened):
        with pytest.raises(OSError):
            os.fstat(descriptor)


def test_original_ancestor_edge_is_checked_without_resolving_replacement(setup, tmp_path):
    _, _, _, peers, *_ = setup
    nested = tmp_path.resolve() / 'original-configs'
    nested.mkdir(mode=0o700)
    relocated = []
    for index, peer in enumerate(peers):
        path = nested / f'{index}.toml'
        path.write_bytes(peer.config.read_bytes())
        path.chmod(0o600)
        relocated.append(replace(peer, config=path))
    exact = tuple(relocated)
    with launcher.PeerLaunchInputs(exact) as inputs:
        nested.rename(nested.with_name('moved-configs'))
        nested.symlink_to(nested.with_name('moved-configs'), target_is_directory=True)
        assert all(peer.config.read_bytes() == original.config.read_bytes()
                   for peer, original in zip(exact, peers))
        with pytest.raises(launcher.LauncherError):
            inputs.validate(exact)


def test_directory_handle_bound_is_enforced_before_more_open_calls(setup, monkeypatch):
    _, _, _, peers, *_ = setup
    monkeypatch.setattr(launcher, '_MAX_DIRECTORY_HANDLES', 1)
    with pytest.raises(launcher.LauncherError):
        launcher.PeerLaunchInputs(peers)


def test_ancestor_swap_during_config_check_is_rejected_in_the_same_validation(setup, tmp_path, monkeypatch):
    _, _, _, peers, *_ = setup
    nested = tmp_path.resolve() / 'race-configs'
    nested.mkdir(mode=0o700)
    relocated = []
    for index, peer in enumerate(peers):
        path = nested / f'{index}.toml'
        path.write_bytes(peer.config.read_bytes())
        path.chmod(0o600)
        relocated.append(replace(peer, config=path))
    exact = tuple(relocated)
    with launcher.PeerLaunchInputs(exact) as inputs:
        real_stat = os.stat
        fired = []
        def swap(name, *args, **kwargs):
            if name == '0.toml' and kwargs.get('dir_fd') is not None and not fired:
                fired.append(True)
                nested.rename(nested.with_name('detached-configs'))
                nested.mkdir(mode=0o700)
            return real_stat(name, *args, **kwargs)
        with monkeypatch.context() as patch:
            patch.setattr(launcher.os, 'stat', swap)
            with pytest.raises(launcher.LauncherError):
                inputs.validate(exact)
        assert fired == [True]
        assert inputs._failed


@pytest.mark.parametrize('change', ['closed', 'different_config_digest', 'callback'])
def test_launcher_requires_exact_open_input_owner(setup, change):
    owner, factory, clock, peers, image, _ = setup
    inputs = owner._inputs
    if change == 'closed':
        inputs.close()
    elif change == 'different_config_digest':
        peers = (replace(peers[0], config_blake3='f' * 64), *peers[1:])
    elif change == 'callback':
        inputs = lambda: None
    with pytest.raises(launcher.LauncherError):
        launcher.FourPeerRun(peers, image, factory, inputs, lambda: None, clock.end())
    assert factory.calls == []
