#!/usr/bin/env python3
"""Restore GNU dynamic-then-static lookup for the reviewed AArch64 Zig lane.

Prerequisites: Python 3.10+, cargo-zigbuild and Zig. IROHA_ZIG_BINARY must name
an absolute real Zig executable. The cargo_zigbuild_linux.sh launcher sets it
and selects this adapter. Only AArch64 GNU/Linux library search and explicitly
requested SHA-3 assembly features are adjusted; other targets pass through.
Safe defaults: no source/cache deletion, no deployment, bounded temporary
response files removed after the compiler exits. --help does not invoke Zig.

TODO: retire this build-only adapter when cargo-zigbuild or pqcrypto-internals
corrects the Linux library-kind/search-mode interaction. No crypto code changes.
"""
import argparse
import os
from pathlib import Path
import shlex
import subprocess
import sys
import tempfile

MAX_RESPONSE_BYTES = 16 * 1024 * 1024


def expand(arguments, depth=0):
    if depth > 3:
        raise ValueError('nested linker response depth exceeds three')
    output = []
    for argument in arguments:
        if argument.startswith('@'):
            with open(argument[1:], 'rb') as stream:
                data = stream.read(MAX_RESPONSE_BYTES + 1)
            if len(data) > MAX_RESPONSE_BYTES:
                raise ValueError('linker response exceeds 16 MiB')
            output.extend(expand(shlex.split(data.decode('utf-8')), depth + 1))
        else:
            output.append(argument)
    return output


def rewrite(arguments):
    if not arguments or arguments[0] not in ('cc', 'c++'):
        return arguments
    target = None
    for index, argument in enumerate(arguments[:-1]):
        if argument in ('-target', '--target'):
            target = arguments[index + 1]
    if target is None or not (target == 'aarch64-linux-gnu'
                              or target.startswith('aarch64-linux-gnu.')):
        return arguments
    adjusted = ['-Wl,-search_paths_first' if argument == '-Wl,-Bdynamic' else argument
                for argument in arguments]
    cpu = next((argument for argument in reversed(arguments) if argument.startswith('-mcpu=')), None)
    if cpu == '-mcpu=generic+sha3' and any(argument.endswith('.S') for argument in arguments):
        # Zig 0.16 passes this CPU feature to C but omits it from the assembly
        # preprocessor. Preserve the feature already requested by the build.
        adjusted.extend(['-Xclang', '-target-feature', '-Xclang', '+sha3',
                         '-Xassembler', '-march=armv8.2-a+sha3'])
    return adjusted


def main():
    if sys.argv[1:] in (['--help'], ['-h']):
        argparse.ArgumentParser(description=__doc__).parse_args()
        return
    zig = os.environ.get('IROHA_ZIG_BINARY')
    if not zig or not Path(zig).is_absolute() or Path(zig).resolve() == Path(__file__).resolve():
        raise ValueError('IROHA_ZIG_BINARY must select an absolute real Zig executable')
    original = sys.argv[1:]
    if not original or original[0] not in ('cc', 'c++'):
        os.execv(zig, [zig] + original)
    expanded = expand(original)
    adjusted = rewrite(expanded)
    if adjusted == expanded:
        os.execv(zig, [zig] + original)
    # Keep large Rust link invocations in a response file instead of exceeding
    # exec's argument limit. The child sees the same ordered arguments.
    with tempfile.TemporaryDirectory(prefix='iroha-zig-gnu-') as directory:
        path = Path(directory) / 'linker-arguments'
        path.write_text(shlex.join(adjusted[1:]) + '\n', encoding='utf-8')
        path.chmod(0o600)
        result = subprocess.run([zig, adjusted[0], '@' + str(path)])
    raise SystemExit(result.returncode)


if __name__ == '__main__':
    main()
