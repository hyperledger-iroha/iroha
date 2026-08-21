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


def _read_locked_commit_progress_witness_test_provider(
    repo_root: Path,
    integration_path: Path,
    integration_source: str,
    errors: list[str],
) -> tuple[Path, str]:
    """Read the one reviewed split-file provider for locked-Commit regressions."""

    provider = integration_path.parent / "sumeragi_v2_runner" / "prepare_qc_split_tests.rs"
    include_source = 'include!("sumeragi_v2_runner/prepare_qc_split_tests.rs");'
    if integration_source.count(include_source) != 1:
        errors.append(
            f"{integration_path}: locked-Commit progress-witness regressions "
            "must have exactly one prepare_qc_split_tests include provider"
        )
    if not provider.is_file() or provider.is_symlink():
        errors.append(
            f"{provider}: locked-Commit progress-witness regression provider "
            "must be a regular file"
        )
        return provider, ""
    _loaded, source = _read_reviewed_rust_source(
        repo_root,
        provider.relative_to(repo_root).as_posix(),
        errors,
        "locked-Commit progress-witness regression provider",
    )
    return provider, source
