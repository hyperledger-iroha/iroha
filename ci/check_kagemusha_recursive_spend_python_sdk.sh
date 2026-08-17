#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${KAGEMUSHA_RECURSIVE_SPEND_PYTHON_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
PYTHON_BIN="${KAGEMUSHA_RECURSIVE_SPEND_PYTHON_BIN:-python3}"
PACKAGE_DIR="${ROOT_DIR}/python/iroha_python"

case "${1:-}" in
  ""|--self-test) ;;
  *)
    echo "usage: $0 [--self-test]" >&2
    exit 2
    ;;
esac
if [[ "$#" -gt 1 ]]; then
  echo "usage: $0 [--self-test]" >&2
  exit 2
fi

version="$("${PYTHON_BIN}" -I -S -c 'import sys; print(f"{sys.version_info.major}.{sys.version_info.minor}")')"
if [[ "${version}" != "3.12" ]]; then
  echo "error: Python SDK checks require Python 3.12; got ${version}" >&2
  exit 1
fi

check_package() {
  local package_dir="$1"
  "${PYTHON_BIN}" -I -S - "${package_dir}" <<'PY'
from pathlib import Path
import ast
import sys

package = Path(sys.argv[1]) / "src/iroha_python"
for source_path in sorted(package.rglob("*.py")):
    source = source_path.read_text(encoding="utf-8")
    compile(source, str(source_path), "exec", dont_inherit=True)
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
}

check_package "${PACKAGE_DIR}"

if [[ "${1:-}" == "--self-test" ]]; then
  SELF_TEST_DIR="$(mktemp -d)"
  trap 'rm -rf -- "${SELF_TEST_DIR}"' EXIT

  mkdir -p \
    "${SELF_TEST_DIR}/export-import/src" \
    "${SELF_TEST_DIR}/export-string/src" \
    "${SELF_TEST_DIR}/module-kagemusha/src" \
    "${SELF_TEST_DIR}/module-offline-cash/src"
  for fixture in export-import export-string module-kagemusha module-offline-cash; do
    cp -R "${PACKAGE_DIR}/src/iroha_python" \
      "${SELF_TEST_DIR}/${fixture}/src/iroha_python"
  done

  printf '\nfrom . import kagemusha\n' >> \
    "${SELF_TEST_DIR}/export-import/src/iroha_python/__init__.py"
  if check_package "${SELF_TEST_DIR}/export-import" >/dev/null 2>&1; then
    echo "error: Python boundary self-test accepted a forbidden package export" >&2
    exit 1
  fi
  printf '\nOFFLINE_LIFECYCLE = "offline_cash"\n' >> \
    "${SELF_TEST_DIR}/export-string/src/iroha_python/__init__.py"
  if check_package "${SELF_TEST_DIR}/export-string" >/dev/null 2>&1; then
    echo "error: Python boundary self-test accepted a forbidden string export" >&2
    exit 1
  fi

  printf '"""Forbidden self-test module."""\n' > \
    "${SELF_TEST_DIR}/module-kagemusha/src/iroha_python/kagemusha.py"
  if check_package "${SELF_TEST_DIR}/module-kagemusha" >/dev/null 2>&1; then
    echo "error: Python boundary self-test accepted kagemusha.py" >&2
    exit 1
  fi
  printf '"""Forbidden self-test module."""\n' > \
    "${SELF_TEST_DIR}/module-offline-cash/src/iroha_python/offline_cash.py"
  if check_package "${SELF_TEST_DIR}/module-offline-cash" >/dev/null 2>&1; then
    echo "error: Python boundary self-test accepted offline_cash.py" >&2
    exit 1
  fi
  echo "Kagemusha Python boundary self-test passed."
fi

echo "Kagemusha Python boundary passed: no offline lifecycle is published."
