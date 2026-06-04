from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path


def test_package_root_lazy_crypto_exports_preserve_import_error_cause() -> None:
    root = Path(__file__).resolve().parents[3]
    script = """
import sys
import types

stub = types.ModuleType("iroha_python.crypto")

def fail_crypto_import(name):
    raise RuntimeError("forced crypto import failure")

stub.__getattr__ = fail_crypto_import
sys.modules["iroha_python.crypto"] = stub

import iroha_python

try:
    iroha_python.Ed25519KeyPair
except RuntimeError as exc:
    assert "requires the compiled iroha_python._crypto extension module" in str(exc)
    assert isinstance(exc.__cause__, RuntimeError)
    assert "forced crypto import failure" in str(exc.__cause__)
else:
    raise AssertionError("crypto export unexpectedly resolved")
"""
    env = os.environ.copy()
    pythonpath = os.pathsep.join(
        [
            str(root / "python" / "iroha_python" / "src"),
            str(root / "python" / "norito_py" / "src"),
            str(root / "python"),
            env.get("PYTHONPATH", ""),
        ]
    )
    env["PYTHONPATH"] = pythonpath
    env["PYTHONDONTWRITEBYTECODE"] = "1"

    subprocess.run(
        [sys.executable, "-c", script],
        check=True,
        cwd=root,
        env=env,
    )
