#!/usr/bin/env python3
"""Launch the authenticated source-projection package from one explicit snapshot."""

from __future__ import annotations

import os
from pathlib import Path
import runpy
import sys


MODULE = "scripts.produce_kagemusha_v4_source_seal_projection"


def main() -> int:
    """Replace ambient import roots with the controller-supplied package snapshot."""

    if not sys.flags.isolated or not sys.flags.no_site:
        raise SystemExit("snapshot launcher requires python -I -S")
    if len(sys.argv) < 2:
        raise SystemExit("snapshot launcher requires one package root")
    root_text = sys.argv[1]
    package_root = Path(root_text)
    if (
        not package_root.is_absolute()
        or os.path.normpath(root_text) != root_text
        or package_root.resolve(strict=True) != package_root
        or package_root.is_symlink()
    ):
        raise SystemExit("snapshot launcher package root is not canonical")
    module_path = package_root / "scripts/produce_kagemusha_v4_source_seal_projection.py"
    if not module_path.is_file() or module_path.is_symlink():
        raise SystemExit("snapshot launcher producer module is unavailable")
    sys.dont_write_bytecode = True
    sys.path[:] = [str(package_root), *sys.path]
    sys.argv = [str(module_path), *sys.argv[2:]]
    runpy.run_module(MODULE, run_name="__main__", alter_sys=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
