#!/usr/bin/env python3
"""Fail closed when the SoraFS gateway regains a placeholder ACME path."""

from __future__ import annotations

import argparse
import hashlib
import re
import sys
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from sorafs_evidence_json import read_evidence_bytes  # noqa: E402
from sorafs_evidence_paths import (  # noqa: E402
    inspect_evidence_directory,
    inspect_evidence_file,
    resolve_evidence_path,
)

REQUIRED_RUNTIME_NOTICE = "> **Runtime ACME boundary (V1):**"
MAX_CONTRACT_SOURCE_BYTES = 8 * 1024 * 1024
GENERATED_STUB_HEADER_RE = re.compile(
    r"<!-- Auto-generated stub for [^<>\r\n]+ "
    r"\((?P<lang>[a-z0-9-]+)\) translation\. "
    r"Replace this content with the full translation\. -->\n\n"
)
FORBIDDEN_DOC_CLAIMS = (
    "sorafs-gateway tls renew",
    "fall back to stored cert",
    "deterministic self-signed bundle required for staging drills",
    "Production ACME clients remain available for validated accounts",
    "[`letsencrypt-rs`](https://crates.io/crates/letsencrypt-rs)",
    "Route53 support is available by default",
    "Generate a replacement bundle with the repository wrapper",
)


def _generated_stub_error(
    text: str,
    *,
    locale: str,
    expected_source: str,
    expected_hash: str,
) -> str | None:
    """Return a reason when a declared generated translation stub is malformed."""

    if not re.search(r"(?m)^status: needs-translation$", text):
        return None
    delimiters = list(re.finditer(r"(?m)^---$", text))
    if len(delimiters) != 2:
        return "generated-stub-front-matter"
    start, end = delimiters
    prefix = text[: start.start()]
    header = GENERATED_STUB_HEADER_RE.fullmatch(prefix)
    if header is None or header.group("lang") != locale:
        return "generated-stub-header"
    metadata: dict[str, str] = {}
    for line in text[start.end() + 1 : end.start()].splitlines():
        if not line or line.lstrip().startswith("#"):
            continue
        if ":" not in line:
            return "generated-stub-front-matter"
        key, value = line.split(":", 1)
        key = key.strip()
        if not key or key in metadata:
            return "generated-stub-front-matter"
        metadata[key] = value.strip().strip('"').strip("'")
    required = {
        "lang": locale,
        "source": expected_source,
        "status": "needs-translation",
        "generator": "scripts/sync_docs_i18n.py",
        "source_hash": expected_hash,
        "translation_last_reviewed": "null",
    }
    if any(metadata.get(key) != value for key, value in required.items()):
        return "generated-stub-metadata"
    if metadata.get("direction") not in {"ltr", "rtl"}:
        return "generated-stub-direction"
    if not metadata.get("source_last_modified"):
        return "generated-stub-source-mtime"
    if not text[end.end() + 1 :].strip():
        return "generated-stub-empty-body"
    return ""


def _repository_root_identity(root: Path, failures: list[str]) -> Path | None:
    """Return one validated repository-root identity."""

    identity_errors: list[str] = []
    is_directory = inspect_evidence_directory(root, identity_errors)
    identity = resolve_evidence_path(
        root,
        identity_errors,
        label="gateway TLS contract repository root",
    )
    if is_directory is not True or identity is None or identity_errors:
        failures.append("repository-root:unsafe-or-unresolvable")
        return None
    return identity


def _read(
    root_identity: Path,
    relative: str,
    failures: list[str],
) -> str | None:
    """Read one bounded, regular, repository-contained UTF-8 source file."""

    path = root_identity / relative
    identity_errors: list[str] = []
    is_file = inspect_evidence_file(path, identity_errors)
    identity = resolve_evidence_path(
        path,
        identity_errors,
        label="gateway TLS contract source",
    )
    if (
        is_file is not True
        or identity is None
        or identity_errors
        or not identity.is_relative_to(root_identity)
    ):
        failures.append(f"{relative}:unsafe-or-unresolvable-source")
        return None
    try:
        raw = read_evidence_bytes(path, MAX_CONTRACT_SOURCE_BYTES)
        return raw.decode("utf-8")
    except (OSError, RuntimeError, UnicodeDecodeError, ValueError):
        failures.append(f"{relative}:unreadable-or-oversized-source")
        return None


def check_contract(root: Path) -> list[str]:
    """Return deterministic contract failures relative to ``root``."""

    failures: list[str] = []
    root_identity = _repository_root_identity(root, failures)
    if root_identity is None:
        return failures

    controller = _read(
        root_identity,
        "crates/iroha_torii/src/sorafs/gateway/controller.rs",
        failures,
    )
    module = _read(
        root_identity,
        "crates/iroha_torii/src/sorafs/gateway/mod.rs",
        failures,
    )
    acme = _read(
        root_identity,
        "crates/iroha_torii/src/sorafs/gateway/acme.rs",
        failures,
    )
    xtask = _read(root_identity, "xtask/src/sorafs.rs", failures)
    torii = _read(
        root_identity,
        "crates/iroha_torii/src/lib.rs",
        failures,
    )
    irohad = _read(
        root_identity,
        "crates/irohad/src/main.rs",
        failures,
    )
    if any(source is None for source in (controller, module, acme, xtask, torii, irohad)):
        return failures
    assert controller is not None
    assert module is not None
    assert acme is not None
    assert xtask is not None
    assert torii is not None
    assert irohad is not None

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
    if "GatewayProviderBindingV1" not in controller:
        failures.append("controller:missing-config-provider-binding")
    if "AcmeAutomation::try_new(config, client_binding, client)?" not in controller:
        failures.append("controller:missing-startup-qualification")
    if "fn qualification(&self)" not in acme:
        failures.append("acme-harness:missing-provider-qualification")
    if "pub trait AcmeClient: Send + Sync" not in acme:
        failures.append("acme-harness:runtime-client-debug-exposure")
    if '.field("client", &"<runtime-only>")' not in acme:
        failures.append("acme-harness:runtime-client-state-not-redacted")
    if (
        acme.count("qualify_acme_client(&self.client_binding, &self.client)")
        < 2
    ):
        failures.append("acme-harness:missing-operation-qualification-fence")

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
    if (
        "torii.sorafs.gateway.acme provider binding must be present exactly when ACME is enabled"
        not in torii
    ):
        failures.append("torii:missing-provider-binding-startup-failure")
    if "TlsAutomationHandle::try_new(" not in torii:
        failures.append("torii:missing-exact-client-qualification")
    for marker, failure in (
        (
            "pub fn with_sorafs_gateway_acme_client(",
            "irohad:missing-runtime-acme-injection",
        ),
        (
            "runtime_deps.with_sorafs_gateway_acme_client(client)",
            "irohad:missing-acme-forwarding",
        ),
        (
            "pub fn with_sorafs_gateway_compliance_feed_transport(",
            "irohad:missing-runtime-compliance-transport-injection",
        ),
        (
            "runtime_deps.with_sorafs_gateway_compliance_feed_transport(transport)",
            "irohad:missing-compliance-transport-forwarding",
        ),
    ):
        if marker not in irohad:
            failures.append(failure)

    canonical_relative = "specs/sorafs_gateway_tls_automation.md"
    canonical_path = root_identity / canonical_relative
    canonical = _read(root_identity, canonical_relative, failures)
    if canonical is None:
        return failures
    expected_hash = hashlib.sha256(canonical.encode("utf-8")).hexdigest()
    try:
        localized_docs = sorted(
            canonical_path.parent.glob("sorafs_gateway_tls_automation.*.md")
        )
    except (OSError, RuntimeError):
        failures.append("specs:gateway-tls-mirror-scan-failed")
        return failures
    docs = [canonical_path, *localized_docs]
    for path in docs:
        relative = path.relative_to(root_identity).as_posix()
        text = _read(root_identity, relative, failures)
        if text is None:
            continue
        generated_stub = False
        if path != canonical_path:
            locale = path.name.split(".")[-2]
            stub_error = _generated_stub_error(
                text,
                locale=locale,
                expected_source=canonical_relative,
                expected_hash=expected_hash,
            )
            if stub_error:
                failures.append(f"{relative}:{stub_error}")
            generated_stub = stub_error == ""
        if not generated_stub and REQUIRED_RUNTIME_NOTICE not in text:
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
        default=SCRIPT_DIR.parent,
        help="repository root",
    )
    args = parser.parse_args()
    failures = check_contract(args.root)
    if failures:
        for failure in failures:
            print(f"[sorafs-gateway-tls] {failure}")
        return 1
    print("[sorafs-gateway-tls] runtime ACME contract is fail-closed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
