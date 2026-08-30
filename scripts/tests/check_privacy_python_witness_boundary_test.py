from __future__ import annotations

import importlib.util
import sys
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[2]
MODULE_PATH = REPO_ROOT / "scripts/check_privacy_python_witness_boundary.py"
SPEC = importlib.util.spec_from_file_location(
    "privacy_python_witness_boundary", MODULE_PATH
)
assert SPEC is not None and SPEC.loader is not None
checker = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = checker
SPEC.loader.exec_module(checker)


def _write_fixture(
    root: Path, *, extra_rust: str = "", registry_suffix: str = ""
) -> None:
    rust_root = root / "python/iroha_python/iroha_python_rs/src"
    python_root = root / "python/iroha_python/src/iroha_python"
    rust_root.mkdir(parents=True)
    python_root.mkdir(parents=True)
    (rust_root / "lib.rs").write_text(
        "#[pymethods]\n"
        "impl TransactionBuilder {\n"
        "    fn prepare_privacy_zk_x509_identity_presentation_action_v1() {}\n"
        "    fn sign_privacy_zk_x509_identity_presentation_action_v1() {}\n"
        f"    {extra_rust}\n"
        "}\n",
        encoding="utf-8",
    )
    (rust_root / "privacy_wallet_worker.rs").write_text(
        """fn retained_protocol(protocol: &str) {
    let protocol_id = protocol;
    match protocol_id {
        PrivacyProtocolIdV1::IrohaZkX509StarkP256V0 => {
            return Err(WorkerError::UnsupportedProtocol);
        }
    }
}

fn next_function() {}
""",
        encoding="utf-8",
    )
    (python_root / "tx.py").write_text(
        """class TransactionDraft:
    def prepare_privacy_zk_x509_identity_presentation_action_v1(self):
        pass

    def sign_privacy_zk_x509_identity_presentation_action_v1(self):
        pass
""",
        encoding="utf-8",
    )
    registry = ",\n".join(
        f"    {protocol!r}: {schemas!r}"
        for protocol, schemas in checker.GENERIC11_OPERATION_SCHEMAS
    )
    (python_root / "privacy_wallet_worker.py").write_text(
        f"""PRIVACY_GENERIC11_WORKER_OPERATION_SCHEMAS_V1 = {{
{registry}{registry_suffix}
}}

class PrivacyWalletWorkerControllerV1:
    def execute(
        self,
        handle,
        binding,
        *,
        canonical_public_intent,
        canonical_execution_plan,
    ):
        pass

__all__ = {list(checker.WORKER_TOP_LEVEL_EXPORTS)!r}
""",
        encoding="utf-8",
    )
    exports = ",\n".join(repr(name) for name in checker.WORKER_TOP_LEVEL_EXPORTS)
    imports = ",\n    ".join(checker.WORKER_TOP_LEVEL_EXPORTS)
    (python_root / "crypto.py").write_text("", encoding="utf-8")
    (python_root / "__init__.py").write_text(
        f"""from .privacy_wallet_worker import (
    {imports}
)

_BASE_EXPORTS = [
{exports}
]
_CRYPTO_EXPORTS = []
""",
        encoding="utf-8",
    )


def test_live_tree_is_release_ready() -> None:
    report = checker.inspect_repository(REPO_ROOT)
    assert report.structural_errors == ()
    assert report.raw_witness_families == ()
    assert report.release_ready


def test_fully_worker_owned_fixture_passes(tmp_path: Path) -> None:
    _write_fixture(tmp_path)
    report = checker.inspect_repository(tmp_path)
    assert report.structural_errors == ()
    assert report.raw_witness_families == ()
    assert report.release_ready


@pytest.mark.parametrize(
    ("family", "method"),
    [(family, methods[0]) for family, methods in checker.RAW_WITNESS_FAMILIES],
)
def test_each_raw_witness_family_fails_release_readiness(
    tmp_path: Path, family: str, method: str
) -> None:
    _write_fixture(tmp_path, extra_rust=f"fn {method}() {{}}")
    report = checker.inspect_repository(tmp_path)
    assert report.structural_errors == ()
    assert report.raw_witness_families == (family,)
    assert not report.release_ready


def test_direct_bundle_and_zk_x509_worker_substitution_fail_closed(
    tmp_path: Path,
) -> None:
    _write_fixture(
        tmp_path,
        extra_rust="fn sign_privacy_orchard_note_action_v1(execution_bundle: Vec<u8>) {}",
        registry_suffix=f",\n    {checker.ZK_X509_PROTOCOL!r}: ('x509-schema',)",
    )
    report = checker.inspect_repository(tmp_path)
    assert any(
        "forbidden 'execution_bundle'" in error for error in report.structural_errors
    )
    assert any(
        "legacy direct owner-bundle" in error for error in report.structural_errors
    )
    assert any(
        "exact ordered generic 11" in error for error in report.structural_errors
    )
    assert any("incorrectly routed" in error for error in report.structural_errors)
    assert not report.release_ready


def test_renamed_raw_parameter_and_missing_top_level_export_fail_closed(
    tmp_path: Path,
) -> None:
    _write_fixture(
        tmp_path,
        extra_rust="fn renamed_constructor(blindings: Vec<Vec<u8>>) {}",
    )
    init_path = tmp_path / "python/iroha_python/src/iroha_python/__init__.py"
    init_path.write_text(
        init_path.read_text(encoding="utf-8").replace(
            "'PrivacyWalletWorkerControllerV1',\n", ""
        ),
        encoding="utf-8",
    )
    report = checker.inspect_repository(tmp_path)
    assert any(
        "raw witness parameters: blindings" in error
        for error in report.structural_errors
    )
    assert any(
        "does not export the complete worker contract" in error
        for error in report.structural_errors
    )
    assert not report.release_ready
