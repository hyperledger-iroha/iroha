#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${KAGEMUSHA_RECURSIVE_SPEND_PYTHON_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
PYTHON_BIN="${KAGEMUSHA_RECURSIVE_SPEND_PYTHON_BIN:-python3}"
PACKAGE_DIR="${ROOT_DIR}/python/iroha_python"

version="$("${PYTHON_BIN}" -I -S -c 'import sys; print(f"{sys.version_info.major}.{sys.version_info.minor}")')"
if [[ "${version}" != "3.12" ]]; then
  echo "error: Python SDK checks require Python 3.12; got ${version}" >&2
  exit 1
fi

export PYTHONDONTWRITEBYTECODE=1
export PYTHONPATH="${PACKAGE_DIR}/src${PYTHONPATH:+:${PYTHONPATH}}"

"${PYTHON_BIN}" -I -S -m compileall -q "${PACKAGE_DIR}/src"
"${PYTHON_BIN}" -I -S - "${PACKAGE_DIR}" <<'PY'
from pathlib import Path
import ast
import sys

package = Path(sys.argv[1]) / "src/iroha_python"
tree = ast.parse((package / "__init__.py").read_text(encoding="utf-8"))
for node in ast.walk(tree):
    names = []
    if isinstance(node, (ast.Import, ast.ImportFrom)):
        names.extend(alias.name for alias in node.names)
        if isinstance(node, ast.ImportFrom) and node.module:
            names.append(node.module)
    if isinstance(node, ast.Constant) and isinstance(node.value, str):
        names.append(node.value)
    for name in names:
        normalized = name.lower().replace("_", "")
        if "kagemusha" in normalized or "offlinecash" in normalized:
            raise SystemExit(f"unexpected Python offline lifecycle export: {name}")
for module in ("kagemusha.py", "offline_cash.py"):
    if (package / module).exists():
        raise SystemExit(f"unexpected Python offline lifecycle module: {module}")
PY

echo "Kagemusha Python boundary passed: no offline lifecycle is published."
