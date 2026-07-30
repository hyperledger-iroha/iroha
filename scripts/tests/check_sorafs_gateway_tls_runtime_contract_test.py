from __future__ import annotations

import importlib.util
import shutil
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT_PATH = REPO_ROOT / "scripts/check_sorafs_gateway_tls_runtime_contract.py"
SPEC = importlib.util.spec_from_file_location("gateway_tls_contract", SCRIPT_PATH)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)

CONTRACT_FIXTURE_PATHS = (
    "crates/iroha_torii/src/sorafs/gateway/controller.rs",
    "crates/iroha_torii/src/sorafs/gateway/mod.rs",
    "crates/iroha_torii/src/sorafs/gateway/acme.rs",
    "crates/iroha_torii/src/lib.rs",
    "crates/irohad/src/main.rs",
    "xtask/src/sorafs.rs",
    "specs/sorafs_gateway_tls_automation.md",
)


def copy_contract_fixture(root: Path) -> None:
    for relative in CONTRACT_FIXTURE_PATHS:
        source = REPO_ROOT / relative
        target = root / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)


def test_repository_runtime_acme_contract_is_fail_closed() -> None:
    assert MODULE.check_contract(REPO_ROOT) == []


def test_gateway_handbook_withdraws_instead_of_using_certificate_fallback() -> None:
    handbook = (
        REPO_ROOT / "specs/sorafs_gateway_deployment_handbook.md"
    ).read_text(encoding="utf-8")

    assert "sorafs-gateway tls renew" not in handbook
    assert "fall back to stored cert" not in handbook
    assert "Withdraw the affected gateway from admission and traffic" in handbook
    assert "audited runtime ACME adapter and controller boundary" in handbook


def test_guard_uses_shared_identity_and_bounded_read_contract() -> None:
    source = SCRIPT_PATH.read_text(encoding="utf-8")

    assert "from sorafs_evidence_json import read_evidence_bytes" in source
    assert "inspect_evidence_directory" in source
    assert "inspect_evidence_file" in source
    assert "resolve_evidence_path" in source
    assert "MAX_CONTRACT_SOURCE_BYTES" in source
    assert ".read_text(" not in source
    assert "args.root.resolve()" not in source


def test_guard_rejects_production_self_signed_export(tmp_path: Path) -> None:
    root = tmp_path / "repo"
    copy_contract_fixture(root)

    module_path = root / "crates/iroha_torii/src/sorafs/gateway/mod.rs"
    module_path.write_text(
        module_path.read_text(encoding="utf-8")
        + "\npub use controller::SelfSignedAcmeClient;\n",
        encoding="utf-8",
    )
    assert "gateway-module:self-signed-export" in MODULE.check_contract(root)


def test_guard_rejects_fake_renewal_and_stale_docs(tmp_path: Path) -> None:
    root = tmp_path / "repo"
    copy_contract_fixture(root)

    xtask = root / "xtask/src/sorafs.rs"
    text = xtask.read_text(encoding="utf-8")
    marker = "pub fn gateway_tls_revoke("
    text = text.replace(marker, "fn fake() { AcmeAutomation::new(); }\n\n" + marker)
    xtask.write_text(text, encoding="utf-8")

    doc = root / "specs/sorafs_gateway_tls_automation.md"
    doc.write_text(
        doc.read_text(encoding="utf-8")
        + "\nProduction ACME clients remain available for validated accounts.\n",
        encoding="utf-8",
    )
    failures = MODULE.check_contract(root)
    assert any(item.startswith("xtask:placeholder-renewal:") for item in failures)
    assert any(":stale-claim:" in item for item in failures)


def test_guard_rejects_missing_daemon_runtime_forwarding(tmp_path: Path) -> None:
    root = tmp_path / "repo"
    copy_contract_fixture(root)

    irohad = root / "crates/irohad/src/main.rs"
    text = irohad.read_text(encoding="utf-8")
    text = text.replace(
        "runtime_deps.with_sorafs_gateway_compliance_feed_transport(transport)",
        "runtime_deps",
        1,
    )
    irohad.write_text(text, encoding="utf-8")

    assert (
        "irohad:missing-compliance-transport-forwarding"
        in MODULE.check_contract(root)
    )


def test_guard_rejects_missing_acme_identity_fences(tmp_path: Path) -> None:
    root = tmp_path / "repo"
    copy_contract_fixture(root)

    acme = root / "crates/iroha_torii/src/sorafs/gateway/acme.rs"
    text = acme.read_text(encoding="utf-8")
    text = text.replace(
        "qualify_acme_client(&self.client_binding, &self.client)",
        "Ok(())",
    )
    text = text.replace(
        "pub trait AcmeClient: Send + Sync",
        "pub trait AcmeClient: std::fmt::Debug + Send + Sync",
    )
    text = text.replace(
        '.field("client", &"<runtime-only>")',
        '.field("client", &self.client)',
    )
    acme.write_text(text, encoding="utf-8")

    torii = root / "crates/iroha_torii/src/lib.rs"
    text = torii.read_text(encoding="utf-8")
    text = text.replace(
        "torii.sorafs.gateway.acme provider binding must be present exactly when ACME is enabled",
        "provider binding omitted",
    )
    torii.write_text(text, encoding="utf-8")

    failures = MODULE.check_contract(root)
    assert "acme-harness:missing-operation-qualification-fence" in failures
    assert "acme-harness:runtime-client-debug-exposure" in failures
    assert "acme-harness:runtime-client-state-not-redacted" in failures
    assert "torii:missing-provider-binding-startup-failure" in failures


def test_guard_rejects_symlinked_contract_source(tmp_path: Path) -> None:
    root = tmp_path / "repo"
    copy_contract_fixture(root)
    module_path = root / "crates/iroha_torii/src/sorafs/gateway/mod.rs"
    external = tmp_path / "external-module.rs"
    external.write_text(module_path.read_text(encoding="utf-8"), encoding="utf-8")
    module_path.unlink()
    module_path.symlink_to(external)

    assert (
        "crates/iroha_torii/src/sorafs/gateway/mod.rs:"
        "unsafe-or-unresolvable-source"
        in MODULE.check_contract(root)
    )


def test_guard_rejects_oversized_contract_source(
    tmp_path: Path,
    monkeypatch,
) -> None:
    root = tmp_path / "repo"
    copy_contract_fixture(root)
    monkeypatch.setattr(MODULE, "MAX_CONTRACT_SOURCE_BYTES", 32)

    assert (
        "crates/iroha_torii/src/sorafs/gateway/controller.rs:"
        "unreadable-or-oversized-source"
        in MODULE.check_contract(root)
    )


def test_guard_rejects_symlinked_repository_root(tmp_path: Path) -> None:
    root = tmp_path / "repo"
    copy_contract_fixture(root)
    root_alias = tmp_path / "repo-alias"
    root_alias.symlink_to(root, target_is_directory=True)

    assert MODULE.check_contract(root_alias) == [
        "repository-root:unsafe-or-unresolvable"
    ]
