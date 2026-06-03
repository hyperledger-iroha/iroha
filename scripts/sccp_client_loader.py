"""Load SCCP Python helpers without importing the HTTP Torii client package."""

from __future__ import annotations

import importlib.util
import sys
from pathlib import Path
from types import ModuleType


_SCCP_MODULE_NAME = "_iroha_torii_client_sccp_standalone"


def load_sccp_module() -> ModuleType:
    """Load `iroha_torii_client.sccp` directly from its source file."""

    cached = sys.modules.get(_SCCP_MODULE_NAME)
    if cached is not None:
        return cached

    repo_root = Path(__file__).resolve().parents[1]
    path = repo_root / "python" / "iroha_torii_client" / "sccp.py"
    spec = importlib.util.spec_from_file_location(_SCCP_MODULE_NAME, path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load SCCP helper module at {path}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[_SCCP_MODULE_NAME] = module
    spec.loader.exec_module(module)  # type: ignore[assignment]
    return module
