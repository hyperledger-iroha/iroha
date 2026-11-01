"""Real one-shot CLI/owned-child controls; native execution belongs to the queue owner."""
import ctypes
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time

import pytest

HERE = Path(__file__).resolve().parent
SUPPORT = HERE.parents[1] / 'scripts'


def wait_file(path, parent=None):
    deadline = time.monotonic() + 30
    while not path.is_file():
        assert parent is None or parent.poll() is None, 'CLI exited before its controlled workload boundary'
        assert time.monotonic() < deadline, 'owned fixture handshake timed out: ' + str(path)
        time.sleep(0.01)


def wait_original(child):
    first = None
    while child.returncode is None:
        try:
            child.wait()
        except BaseException as error:
            first = first or error
    if first is not None:
        raise first
    return child.returncode


@pytest.fixture(scope='module')
def compiled(tmp_path_factory):
    directory = tmp_path_factory.mktemp('supervision-image')
    source = directory / 'probe.rs'
    source.write_text(r'''//! Actual root/descendant fixture, released only by its owning test protocol.
use std::{env,fs,path::Path,process::{Command,Stdio},thread,time::Duration};
fn main() {
 let args:Vec<_>=env::args().collect();let controls=Path::new(&args[2]);
 if args[1]=="root-with-leaf" {
  let _original=Command::new(env::current_exe().unwrap()).args(["leaf",&args[2]])
   .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::inherit()).spawn().unwrap();
  fs::write(controls.join("root-returning"),b"root spawned leaf and is returning").unwrap();return;
 }
 fs::write(controls.join("workload-ready"),b"genuine owned process").unwrap();
 while !controls.join("release").exists(){thread::sleep(Duration::from_millis(5));}
 fs::write(controls.join("workload-finished"),b"natural original completion").unwrap();
}
''')
    program = directory / ('probe.exe' if os.name == 'nt' else 'probe')
    command = ['rustc'] + (['+1.93.1-x86_64-pc-windows-msvc'] if os.name == 'nt' else [])
    child = subprocess.Popen([*command, '--edition=2024', '-C', 'debuginfo=0', str(source), '-o', str(program)])
    assert wait_original(child) == 0
    library = Path(os.environ.get('IROHA_TEST_PROCESS_BRIDGE_LIBRARY',
        str(SUPPORT.parent / 'target/debug/libiroha_process_bridge.so')))
    assert library.is_file(), 'the actual same-source diagnostic bridge is required'
    return program, library


@pytest.mark.parametrize('mode', ['read-error', 'unknown', 'close-error', 'outer-unwind', 'read-error-cleanup'])
def test_one_shot_cli_retains_originals_until_known_completion(tmp_path, compiled, mode):
    image, bridge = compiled
    source = tmp_path / 'source'
    modules = tmp_path / 'modules'
    controls = tmp_path / 'controls'
    for path in (source, modules, controls):
        path.mkdir()
    program = source / image.name
    library = modules / bridge.name
    shutil.copyfile(image, program)
    shutil.copyfile(bridge, library)
    guard = source / 'source-guard'
    guard.write_bytes(b'original non-image source custody')
    if os.name != 'nt':
        program.chmod(0o500)
        library.chmod(0o500)
    with (controls / 'stdout').open('wb') as stdout, (controls / 'stderr').open('wb') as stderr:
        child = subprocess.Popen([sys.executable, str(HERE / 'support/bridge_cli_fixture.py'),
            str(SUPPORT / 'release_process_bridge.py'), str(SUPPORT), str(library),
            str(program), str(controls), mode], stdout=stdout, stderr=stderr)
        try:
            wait_file(controls / 'workload-ready', child)
            if os.name == 'nt':
                # This marker proves the root reached its return boundary. It is
                # not an original-process terminal observation; lower owner tests
                # separately observe root terminal while the Job leaf remains live.
                wait_file(controls / 'root-returning', child)
            if mode != 'close-error':
                wait_file(controls / 'fault-observed', child)
            if mode == 'outer-unwind':
                wait_file(controls / 'outer-unwind-reached', child)
            assert child.poll() is None
            assert not (controls / 'owners-closed').exists()
            if os.name == 'nt':
                with pytest.raises(OSError):
                    with guard.open('r+b'):
                        pass
                with pytest.raises(OSError):
                    guard.rename(guard.with_suffix('.displaced'))
            (controls / 'release').write_bytes(b'finish only this owned fixture')
            wait_file(controls / 'workload-finished', child)
            if mode == 'unknown':
                # Even the workload finish marker is insufficient while both original
                # ABI observers still refuse; no path/PID inference grants release.
                wait_file(controls / 'unknown-after-workload-finish', child)
                assert child.poll() is None
                assert not (controls / 'owners-closed').exists()
                if os.name == 'nt':
                    with pytest.raises(OSError):
                        with guard.open('r+b'):
                            pass
                (controls / 'restore-observation').write_bytes(b'restore actual original ABI calls')
        finally:
            # Only fixture-owned protocol files are touched. Never signal a test,
            # Cargo, unrelated child, or rediscovered PID, even after an assertion.
            (controls / 'release').write_bytes(b'fixture cleanup completion')
            (controls / 'restore-observation').write_bytes(b'fixture cleanup observation')
            wait_original(child)
        assert child.returncode == 2, (controls / 'stderr').read_text()
    assert (controls / 'owners-closed').is_file()
    result = json.loads((controls / 'cli-refusal.json').read_text())
    assert result['first_error_identity_preserved'], result
    assert result['original_cause_preserved'], result
    if mode == 'read-error-cleanup':
        assert result['secondary_cleanup_retained'], result
    assert result['type'] == 'OSError'
    assert (controls / 'stdout').read_bytes() == b''


def test_observation_buffer_failure_is_before_first_spawn(tmp_path, compiled, monkeypatch):
    sys.path.insert(0, str(SUPPORT))
    spec = importlib.util.spec_from_file_location('allocation_bridge', SUPPORT / 'release_process_bridge.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    from contextlib import ExitStack
    library = ctypes.CDLL(str(compiled[1]))
    bridge = module.Bridge(library)
    first = MemoryError('bounded observation storage refused before process creation')
    spawned = []
    bridge.spawn = lambda *_: spawned.append(True)
    original = module.c.create_string_buffer
    calls = []
    def allocate(*arguments):
        calls.append(True)
        if len(calls) == 2:
            raise first
        return original(*arguments)
    monkeypatch.setattr(module.c, 'create_string_buffer', allocate)
    closed = []
    with ExitStack() as owners:
        owners.callback(closed.append, True)
        with pytest.raises(MemoryError) as error:
            bridge.run({'program': str(compiled[0]), 'directory': str(tmp_path),
                        'arguments': [], 'environment': {}}, owners)
        assert error.value is first
        assert closed == []
    assert closed == [True]
    assert spawned == []
    assert module._FAILED_CUSTODY == []
