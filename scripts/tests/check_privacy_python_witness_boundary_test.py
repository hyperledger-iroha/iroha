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
        "mod privacy_x509_transaction;\n"
        "fn register() { module.add_class::<privacy_x509_transaction::PyPrivacyX509PreparedTransactionV1>(); }\n"
        "#[pymethods]\n"
        "impl TransactionBuilder {\n"
        "    fn prepare_privacy_zk_x509_identity_presentation_action_v1() {}\n"
        f"    {extra_rust}\n"
        "}\n",
        encoding="utf-8",
    )
    (rust_root / "privacy_x509_transaction.rs").write_text(
        (REPO_ROOT / "python/iroha_python/iroha_python_rs/src/privacy_x509_transaction.rs").read_text(),
        encoding="utf-8",
    )
    (rust_root / "privacy_native_actions.rs").write_text(
        "enum PrivacyNativeActionRequestV1 { Generic(GenericRequest) }\n", encoding="utf-8"
    )
    (rust_root / "privacy_wallet_worker.rs").write_text(
        """fn retained_protocol(protocol: &str) {
    let protocol_id = protocol;
    match protocol_id {
        PrivacyProtocolIdV1::IrohaZkX509StarkP256V1 => {
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
    def prepare_privacy_zk_x509_identity_presentation_action_v1(self, *, canonical_request_archive):
        pass
""",
        encoding="utf-8",
    )
    registry = ",\n".join(
        f"    {protocol!r}: 'schema-{index}'"
        for index, protocol in enumerate(checker.GENERIC11_PROTOCOLS)
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
        registry_suffix=f",\n    {checker.ZK_X509_PROTOCOL!r}: 'x509-schema'",
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


@pytest.mark.parametrize("surface", ["lib.rs", "tx.py"])
def test_retired_mutable_owner_signing_bypass_is_rejected(tmp_path: Path, surface: str) -> None:
    _write_fixture(tmp_path)
    if surface == "lib.rs":
        path = tmp_path / "python/iroha_python/iroha_python_rs/src/lib.rs"
        source = path.read_text().replace(
            "impl TransactionBuilder {", "impl TransactionBuilder {\n"
            " fn sign_privacy_zk_x509_identity_presentation_action_v1(private_key: &[u8], statement: &[u8], proof: &[u8]) {}"
        )
    else:
        path = tmp_path / "python/iroha_python/src/iroha_python/tx.py"
        source = path.read_text() + "\n    def sign_privacy_zk_x509_identity_presentation_action_v1(self, private_key, statement, proof): pass\n"
    path.write_text(source)
    assert any("mutable-draft X509 signing bypass" in error for error in checker.inspect_repository(tmp_path).structural_errors)


@pytest.mark.parametrize(("old", "new", "error"), [
    ("frozen,", "", "frozen with private retained custody"),
    ("    plan: DirectPlan,", "    pub plan: DirectPlan,", "private retained custody"),
    ("impl PyPrivacyX509PreparedTransactionV1 {\n    #[getter]", "impl PyPrivacyX509PreparedTransactionV1 {\n    #[new]\n    fn new() -> Self { todo!() }\n    #[getter]", "constructor or setter"),
    ("credential_proof: &[u8]", "credential_proof: &[u8], replacement: TransactionPayload", "only the original owner"),
    ("require_key(&self.plan.payload, &key)?;", "// require_key(&self.plan.payload, &key)?;", "custody guard missing"),
    ("self.admission.x509_policy(self.network_id)?", "other_policy()", "custody guard missing"),
    ("validate_materialized_against_profile_v1(", "unchecked_materialization(", "custody guard missing"),
    ("payload.validate_signed_attachments_v1()", "payload.skip_attachment_validation()", "custody guard missing"),
    ("intent != self.intent", "false", "custody guard missing"),
])
def test_owner_custody_regressions_fail_closed(tmp_path: Path, old: str, new: str, error: str) -> None:
    _write_fixture(tmp_path)
    path = tmp_path / "python/iroha_python/iroha_python_rs/src/privacy_x509_transaction.rs"
    source = path.read_text()
    assert old in source
    path.write_text(source.replace(old, new, 1))
    assert any(error in item for item in checker.inspect_repository(tmp_path).structural_errors)


def test_generic_action_x509_bypass_is_rejected(tmp_path: Path) -> None:
    _write_fixture(tmp_path)
    path = tmp_path / "python/iroha_python/iroha_python_rs/src/privacy_native_actions.rs"
    path.write_text("enum PrivacyNativeActionRequestV1 { ZkX509(OldMutableRequest) }")
    assert any("generic action dispatcher" in error for error in checker.inspect_repository(tmp_path).structural_errors)


def test_retired_activation_validator_cannot_replace_selected_profile_admission(tmp_path: Path) -> None:
    _write_fixture(tmp_path)
    path = tmp_path / "python/iroha_python/iroha_python_rs/src/privacy_x509_transaction.rs"
    source = path.read_text()
    assert "validate_materialized_against_profile_v1" in source
    path.write_text(source.replace("validate_materialized_against_profile_v1", "validate_materialized_against_activation_v1"))
    errors = checker.inspect_repository(tmp_path).structural_errors
    assert any("X509 custody guard missing" in error and "validate_materialized_against_profile_v1" in error for error in errors)
