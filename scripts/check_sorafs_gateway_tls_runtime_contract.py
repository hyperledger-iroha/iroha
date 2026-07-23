#!/usr/bin/env python3
"""Fail closed when the SoraFS gateway regains a placeholder ACME path."""

from __future__ import annotations

import argparse
import hashlib
import re
from pathlib import Path


REQUIRED_RUNTIME_NOTICE = "> **Runtime ACME boundary (V1):**"
FORBIDDEN_DOC_CLAIMS = (
    "deterministic self-signed bundle required for staging drills",
    "Production ACME clients remain available for validated accounts",
    "[`letsencrypt-rs`](https://crates.io/crates/letsencrypt-rs)",
    "Route53 support is available by default",
    "Generate a replacement bundle with the repository wrapper",
)


def _read(root: Path, relative: str) -> str:
    return (root / relative).read_text(encoding="utf-8")


def check_contract(root: Path) -> list[str]:
    """Return deterministic contract failures relative to ``root``."""

    failures: list[str] = []
    controller = _read(
        root, "crates/iroha_torii/src/sorafs/gateway/controller.rs"
    )
    module = _read(root, "crates/iroha_torii/src/sorafs/gateway/mod.rs")
    acme = _read(root, "crates/iroha_torii/src/sorafs/gateway/acme.rs")
    xtask = _read(root, "xtask/src/sorafs.rs")
    torii = _read(root, "crates/iroha_torii/src/lib.rs")

    test_boundary = controller.find("#[cfg(test)]")
    production_controller = (
        controller if test_boundary == -1 else controller[:test_boundary]
    )
    if "SelfSignedAcmeClient" in production_controller:
        failures.append("controller:production-self-signed-client")
    if "SelfSignedAcmeClient" in module:
        failures.append("gateway-module:self-signed-export")
    if "SelfSignedAcmeClient" in acme:
        failures.append("acme-harness:self-signed-client")
    if "Arc<dyn AcmeClient>" not in controller:
        failures.append("controller:missing-runtime-client-boundary")

    renew_start = xtask.find("pub fn gateway_tls_renew(")
    renew_end = xtask.find("\npub fn gateway_tls_revoke(", renew_start)
    if renew_start == -1 or renew_end == -1:
        failures.append("xtask:missing-renew-command-boundary")
    else:
        renewal = xtask[renew_start:renew_end]
        forbidden_renewal = (
            "SelfSignedAcmeClient",
            "AcmeAutomation::new",
            "certificate_pem",
            "private_key_pem",
            "write_file_with_mode",
            "fs::create_dir_all",
        )
        for marker in forbidden_renewal:
            if marker in renewal:
                failures.append(f"xtask:placeholder-renewal:{marker}")
        if "runtime-injected provider client" not in renewal:
            failures.append("xtask:renewal-does-not-fail-closed")

    if (
        "torii.sorafs.gateway.acme is enabled but no runtime ACME client was injected"
        not in torii
    ):
        failures.append("torii:missing-enabled-without-client-startup-failure")

    canonical_path = root / "docs/source/sorafs_gateway_tls_automation.md"
    canonical = canonical_path.read_text(encoding="utf-8")
    expected_hash = hashlib.sha256(canonical.encode("utf-8")).hexdigest()
    docs = [canonical_path, *sorted(canonical_path.parent.glob(
        "sorafs_gateway_tls_automation.*.md"
    ))]
    for path in docs:
        relative = path.relative_to(root).as_posix()
        text = path.read_text(encoding="utf-8")
        if REQUIRED_RUNTIME_NOTICE not in text:
            failures.append(f"{relative}:missing-runtime-notice")
        for claim in FORBIDDEN_DOC_CLAIMS:
            if claim in text:
                failures.append(f"{relative}:stale-claim:{claim}")
        if path != canonical_path:
            match = re.search(r"^source_hash: ([0-9a-f]{64})$", text, re.MULTILINE)
            if match is None or match.group(1) != expected_hash:
                failures.append(f"{relative}:stale-source-hash")

    return failures


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Check the fail-closed SoraFS gateway ACME runtime contract."
    )
    parser.add_argument(
        "--root",
        type=Path,
        default=Path(__file__).resolve().parents[1],
        help="repository root",
    )
    args = parser.parse_args()
    failures = check_contract(args.root.resolve())
    if failures:
        for failure in failures:
            print(f"[sorafs-gateway-tls] {failure}")
        return 1
    print("[sorafs-gateway-tls] runtime ACME contract is fail-closed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
