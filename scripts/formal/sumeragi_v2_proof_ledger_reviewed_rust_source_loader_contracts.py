# Executed lexically in check_sumeragi_v2_proof_ledger.py; do not import directly.

import importlib.util
import subprocess
import sys


def _load_recursive_reviewed_rust_source_module() -> Any:
    """Load the shared authenticated recursive Rust include resolver."""

    module_name = "_sumeragi_v2_proof_ledger_reviewed_rust_source"
    loaded = sys.modules.get(module_name)
    if loaded is not None:
        return loaded
    path = Path(__file__).with_name(
        "sumeragi_v2_multilane_reviewed_rust_source.py"
    )
    spec = importlib.util.spec_from_file_location(module_name, path)
    if spec is None or spec.loader is None:
        raise RuntimeError(
            f"cannot load authenticated reviewed Rust source resolver: {path}"
        )
    module = importlib.util.module_from_spec(spec)
    sys.modules[module_name] = module
    try:
        spec.loader.exec_module(module)
    except BaseException:
        sys.modules.pop(module_name, None)
        raise
    return module


_RECURSIVE_REVIEWED_RUST_SOURCE = _load_recursive_reviewed_rust_source_module()
