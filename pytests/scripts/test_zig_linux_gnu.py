"""Regression coverage for the AArch64 GNU/Linux Zig build adapter."""
import importlib.util
import json
import os
from pathlib import Path
import shlex
import subprocess
import sys

import pytest

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / 'scripts/zig_linux_gnu.py'
SPEC = importlib.util.spec_from_file_location('zig_linux_gnu', SCRIPT)
DRIVER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(DRIVER)


def test_gnu_library_search_preserves_static_mode_and_argument_order():
    original = ['cc', '-target', 'aarch64-linux-gnu', '-Wl,-Bstatic', 'lib.rlib',
                '-Wl,-Bdynamic', '-lpqclean_common', '-lkeccak2x', '-lc']
    expected = original.copy()
    expected[5] = '-Wl,-search_paths_first'
    assert DRIVER.rewrite(original) == expected
    assert original[5] == '-Wl,-Bdynamic'


@pytest.mark.parametrize('target', ['aarch64-macos', 'x86_64-linux-gnu', 'aarch64-linux-musl'])
def test_other_targets_are_unchanged(target):
    original = ['cc', '-target', target, '-Wl,-Bdynamic', '-mcpu=generic+sha3', 'feat.S']
    assert DRIVER.rewrite(original) == original


def test_requested_sha3_reaches_both_assembly_preprocessor_and_assembler():
    original = ['cc', '-target', 'aarch64-linux-gnu.2.28', '-mcpu=generic+sha3',
                '-c', 'feat.S', '-o', 'feat.o']
    assert DRIVER.rewrite(original) == original + [
        '-Xclang', '-target-feature', '-Xclang', '+sha3',
        '-Xassembler', '-march=armv8.2-a+sha3']


def test_c_and_overridden_cpu_do_not_receive_assembly_flags():
    base = ['cc', '-target', 'aarch64-linux-gnu', '-mcpu=generic+sha3']
    for suffix in [['file.c'], ['-mcpu=generic', 'feat.S']]:
        assert DRIVER.rewrite(base + suffix) == base + suffix


def test_nested_response_preserves_quotes_spaces_and_order(tmp_path):
    inner = tmp_path / 'inner'
    outer = tmp_path / 'outer'
    arguments = ['space here.o', "quote'file.o", '-Wl,-Bdynamic', '-lpqclean_common']
    inner.write_text(shlex.join(arguments))
    outer.write_text(shlex.join(['before', '@' + str(inner), 'after']))
    assert DRIVER.expand(['@' + str(outer)]) == ['before'] + arguments + ['after']


def test_response_depth_and_size_are_bounded(tmp_path, monkeypatch):
    path = tmp_path / 'recursive'
    path.write_text('@' + str(path))
    with pytest.raises(ValueError, match='depth'):
        DRIVER.expand(['@' + str(path)])
    path.write_text('12345')
    monkeypatch.setattr(DRIVER, 'MAX_RESPONSE_BYTES', 4)
    with pytest.raises(ValueError, match='16 MiB'):
        DRIVER.expand(['@' + str(path)])


def test_real_child_receives_rewritten_response_and_exit_status(tmp_path):
    backend = tmp_path / 'fake-zig'
    output = tmp_path / 'args.json'
    backend.write_text('#!' + sys.executable + '\n'
                       'import json, pathlib, shlex, sys\n'
                       'response = pathlib.Path(sys.argv[2][1:])\n'
                       'arguments = [sys.argv[1]] + shlex.split(response.read_text())\n'
                       'pathlib.Path(' + repr(str(output)) + ').write_text(json.dumps(arguments))\n'
                       'raise SystemExit(7)\n')
    backend.chmod(0o755)
    env = dict(os.environ, IROHA_ZIG_BINARY=str(backend))
    arguments = ['cc', '-target', 'aarch64-linux-gnu', '-Wl,-Bdynamic', 'space here.o']
    result = subprocess.run([sys.executable, str(SCRIPT)] + arguments, env=env, check=False)
    assert result.returncode == 7
    assert json.loads(output.read_text()) == DRIVER.rewrite(arguments)


def test_launcher_selects_driver_without_changing_cargo_arguments(tmp_path):
    launcher = tmp_path / 'cargo_zigbuild_linux.sh'
    launcher.write_bytes((ROOT / 'scripts/cargo_zigbuild_linux.sh').read_bytes())
    cargo_fast = tmp_path / 'cargo_fast.sh'
    cargo_fast.write_text('#!' + sys.executable + '\n'
                          'import json, os, sys\n'
                          'print(json.dumps({"args": sys.argv[1:], "zig": os.environ["CARGO_ZIGBUILD_ZIG_PATH"], '
                          '"real": os.environ["IROHA_ZIG_BINARY"], "python": os.environ["CARGO_ZIGBUILD_PYTHON_PATH"], '
                          '"cc_trace": os.environ["CC_ENABLE_DEBUG_OUTPUT"]}))\n')
    cargo_fast.chmod(0o755)
    env = dict(os.environ, IROHA_ZIG_BINARY='/usr/bin/true')
    arguments = ['--jobs', '6', '--', 'zigbuild', '--locked', '--target', 'aarch64-unknown-linux-gnu']
    result = subprocess.run(['bash', str(launcher)] + arguments, env=env, capture_output=True, text=True, check=True)
    value = json.loads(result.stdout)
    assert value == {'args': arguments, 'zig': str(tmp_path / 'zig_linux_gnu.py'),
                     'real': '/usr/bin/true', 'python': '/usr/bin/false', 'cc_trace': '1'}
    rejected = subprocess.run(['bash', str(launcher), '--', 'build'], env=env, capture_output=True)
    assert rejected.returncode == 1


@pytest.mark.parametrize('script', ['zig_linux_gnu.py', 'cargo_zigbuild_linux.sh'])
def test_help_does_not_require_or_invoke_zig(script):
    env = dict(os.environ)
    env.pop('IROHA_ZIG_BINARY', None)
    # Even an invalid backend selector must not affect prerequisite discovery.
    env['CARGO_ZIGBUILD_ZIG_PATH'] = '/nonexistent/zig'
    prefix = [sys.executable] if script.endswith('.py') else ['bash']
    result = subprocess.run(prefix + [str(ROOT / 'scripts' / script), '--help'],
                            env=env, capture_output=True, text=True, check=True)
    assert 'IROHA_ZIG_BINARY' in result.stdout and 'Zig' in result.stdout
    assert result.stderr == ''
