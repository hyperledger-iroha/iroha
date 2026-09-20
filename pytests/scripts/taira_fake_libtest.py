"""Disposable libtest executables for real orchestration/census regression tests."""

import sys


def executable(tests, executed, *, failed=(), ignored=(), failure_file=None):
    """Emit named results for exact serial batches and individual test invocations."""
    return (
        f"#!{sys.executable}\n"
        "import sys\nfrom pathlib import Path\n"
        f"tests = {list(tests)!r}\n"
        "if '--list' in sys.argv:\n"
        "    print('\\n'.join(name + ': test' for name in tests))\n"
        "    sys.exit(0)\n"
        "selected = [name for name in sys.argv[1:] if name in tests]\n"
        "assert selected and len(set(selected)) == len(selected)\n"
        f"failed = set({list(failed)!r})\n"
        + (f"failed.update(Path({str(failure_file)!r}).read_text().splitlines())\n"
           if failure_file is not None else "")
        + f"ignored = set({list(ignored)!r})\n"
        "print(f\"running {len(selected)} {'test' if len(selected) == 1 else 'tests'}\")\n"
        "counts = {'ok': 0, 'FAILED': 0, 'ignored': 0}\n"
        "for name in selected:\n"
        f"    with Path({str(executed)!r}).open('a') as stream: stream.write(name + '\\n')\n"
        "    status = 'FAILED' if name in failed else 'ignored' if name in ignored else 'ok'\n"
        "    counts[status] += 1\n"
        "    print('test ' + name + ' ... ' + status)\n"
        "status = 'FAILED' if counts['FAILED'] else 'ok'\n"
        "print(f\"test result: {status}. {counts['ok']} passed; {counts['FAILED']} failed; \"\n"
        "      f\"{counts['ignored']} ignored; 0 measured; {len(tests) - len(selected)} filtered out; finished in 0.01s\")\n"
        "sys.exit(101 if counts['FAILED'] else 0)\n"
    )
