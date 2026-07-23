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


def test_repository_runtime_acme_contract_is_fail_closed() -> None:
    assert MODULE.check_contract(REPO_ROOT) == []


def test_guard_rejects_production_self_signed_export(tmp_path: Path) -> None:
    root = tmp_path / "repo"
    for relative in (
        "crates/iroha_torii/src/sorafs/gateway/controller.rs",
        "crates/iroha_torii/src/sorafs/gateway/mod.rs",
        "crates/iroha_torii/src/sorafs/gateway/acme.rs",
        "crates/iroha_torii/src/lib.rs",
        "xtask/src/sorafs.rs",
        "docs/source/sorafs_gateway_tls_automation.md",
    ):
        source = REPO_ROOT / relative
        target = root / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)

    module_path = root / "crates/iroha_torii/src/sorafs/gateway/mod.rs"
    module_path.write_text(
        module_path.read_text(encoding="utf-8")
        + "\npub use controller::SelfSignedAcmeClient;\n",
        encoding="utf-8",
    )
    assert "gateway-module:self-signed-export" in MODULE.check_contract(root)


def test_guard_rejects_fake_renewal_and_stale_docs(tmp_path: Path) -> None:
    root = tmp_path / "repo"
    for relative in (
        "crates/iroha_torii/src/sorafs/gateway/controller.rs",
        "crates/iroha_torii/src/sorafs/gateway/mod.rs",
        "crates/iroha_torii/src/sorafs/gateway/acme.rs",
        "crates/iroha_torii/src/lib.rs",
        "xtask/src/sorafs.rs",
        "docs/source/sorafs_gateway_tls_automation.md",
    ):
        source = REPO_ROOT / relative
        target = root / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)

    xtask = root / "xtask/src/sorafs.rs"
    text = xtask.read_text(encoding="utf-8")
    marker = "pub fn gateway_tls_revoke("
    text = text.replace(marker, "fn fake() { AcmeAutomation::new(); }\n\n" + marker)
    xtask.write_text(text, encoding="utf-8")

    doc = root / "docs/source/sorafs_gateway_tls_automation.md"
    doc.write_text(
        doc.read_text(encoding="utf-8")
        + "\nProduction ACME clients remain available for validated accounts.\n",
        encoding="utf-8",
    )
    failures = MODULE.check_contract(root)
    assert any(item.startswith("xtask:placeholder-renewal:") for item in failures)
    assert any(":stale-claim:" in item for item in failures)
