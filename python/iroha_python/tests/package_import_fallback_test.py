from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path


def test_privacy_wallet_worker_contract_is_a_base_export() -> None:
    import iroha_python
    from iroha_python import privacy_wallet_worker

    for name in privacy_wallet_worker.__all__:
        assert name in iroha_python.__all__
        assert getattr(iroha_python, name) is getattr(privacy_wallet_worker, name)

    for retired in (
        "PrivacyBootleLanternPresentationActionBuildResultV1",
        "PrivacyJindoActionBuildResultV1",
        "PrivacyVeRangeActionBuildResultV1",
        "PrivacyVegaActionPreparationV1",
        "PrivacyVegaActionBuildResultV1",
        "PrivacyZkAceTransferActionBuildResultV1",
        "PrivacyZkAmsBatchAdmissionActionBuildResultV1",
        "PrivacyZkAmsProvisionAccountActionBuildResultV1",
    ):
        assert retired not in iroha_python.__all__
        assert not hasattr(iroha_python, retired)


def test_offline_capability_is_the_only_discovery_export() -> None:
    import iroha_python
    from iroha_python import OfflineStatus

    assert "OfflineStatus" in iroha_python.__all__
    assert OfflineStatus.__name__ == "OfflineStatus"
    for retired in (
        "OfflineActiveTransferVerifier",
        "OfflineActiveTopUpShieldVerifier",
        "OfflineActiveUnshieldVerifier",
        "OfflineActiveRecursiveStepEqVerifier",
        "OfflineActiveRecursiveStepEpVerifier",
        "OfflineReadiness",
        "OfflineReadinessBlocker",
    ):
        assert retired not in iroha_python.__all__
        assert not hasattr(iroha_python, retired)


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
    if sys.flags.optimize != 0:
        raise AssertionError("fallback subprocess unexpectedly enabled optimization")
    if "requires the compiled iroha_python._crypto extension module" not in str(exc):
        raise AssertionError("fallback error lost the compiled-extension message")
    if not isinstance(exc.__cause__, RuntimeError):
        raise AssertionError("fallback error lost its RuntimeError cause")
    if "forced crypto import failure" not in str(exc.__cause__):
        raise AssertionError("fallback error lost the original cause message")
else:
    raise AssertionError("crypto export unexpectedly resolved")
"""
    env = os.environ.copy()
    python_paths = [
        str(root / "python" / "norito_py" / "src"),
        str(root / "python"),
        env.get("PYTHONPATH", ""),
    ]
    if env.get("IROHA_PYTHON_TEST_INSTALLED_PACKAGE") != "1":
        python_paths.insert(0, str(root / "python" / "iroha_python" / "src"))
    pythonpath = os.pathsep.join(python_paths)
    env["PYTHONPATH"] = pythonpath
    env["PYTHONDONTWRITEBYTECODE"] = "1"
    env.pop("PYTHONOPTIMIZE", None)

    subprocess.run(
        [sys.executable, "-c", script],
        check=True,
        cwd=root,
        env=env,
    )
