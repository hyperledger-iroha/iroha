#!/usr/bin/env python3
"""Validate the committed SoraFS release and conformance workflow contracts."""

from __future__ import annotations

import json
import os
import re
import stat
import sys
from pathlib import Path
from typing import Any


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from check_sorafs_release_version_map import (  # noqa: E402
    _read_bytes_no_follow,
    _require_regular_repo_file,
)


SCHEMA = "sorafs.release.automation.v1"
RELEASE_TARGET_RUNNERS: tuple[tuple[str, str], ...] = (
    ("ubuntu-24.04", "x86_64-unknown-linux-gnu"),
    ("ubuntu-24.04-arm", "aarch64-unknown-linux-gnu"),
    ("macos-15-intel", "x86_64-apple-darwin"),
    ("macos-14", "aarch64-apple-darwin"),
    ("windows-latest", "x86_64-pc-windows-msvc"),
)
SUPPLY_CHAIN_SOURCE_TARGETS: tuple[str, ...] = (
    "x86_64-apple-darwin",
    "aarch64-apple-darwin",
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
    "x86_64-pc-windows-msvc",
)
RELEASE_DOCUMENTS: dict[str, tuple[str, ...]] = {
    "specs/sorafs/runbooks/index.md": (
        "[Release rollback and yank](./release_rollback_yank.md)",
    ),
    "specs/sorafs_release_pipeline_plan.md": (
        "builds native Linux x86_64/aarch64",
        "executes all three binaries from each clean extraction",
        "`scripts/package_sorafs_cli_candidate.py` assembles the whole platform",
        "exactly the five expected target-triple checksum manifests",
        "The five-target CLI archive path is source-complete",
        "build, publish, and clean-install all six",
        "`specs/sorafs/runbooks/release_rollback_yank.md`",
        "`sorafs-release-authentication` environment",
        "`scripts/release_manifest_signing.py verify`",
        "never receives a private key or invokes a signer",
    ),
    "specs/sorafs/runbooks/release_rollback_yank.md": (
        "# SoraFS Release Rollback and Yank",
        "`cargo yank --vers <version> <crate>`",
        "npm deprecate <package>@<version>",
        "Python/PyPI",
        "C#/NuGet",
        "JVM/Android",
        "Swift Package Manager",
        "GitHub CLI artifacts",
        "`withdrawn`, `not_published`, or `failed`",
        "Never reuse a withdrawn version",
    ),
    "specs/sorafs/developer/releases.md": (
        "sorafs-cli-vX.Y.Z",
        "all five native candidate archives",
        "[SoraFS Release Rollback and Yank](../runbooks/release_rollback_yank.md)",
        "`SORAFS_RELEASE_MANIFEST_VERIFIER_PATH`",
        "No private key or HSM signing operation",
    ),
    "fixtures/documentation/sorafs_release_notes.md": (
        "## Rollback / Yank Record",
        "every package row in `release/version-map.toml`",
        "`<withdrawn | not_published | failed>`",
    ),
}
FORBIDDEN_RELEASE_DOCUMENT_CLAIMS: dict[str, tuple[str, ...]] = {
    "specs/sorafs_release_pipeline_plan.md": (
        "via cross",
        "exactly the three expected platform checksum manifests",
    ),
    "specs/sorafs/developer/releases.md": (
        "git tag -s sorafs-v",
        "invokes the script above",
    ),
}
RELEASE_AUTH_ROOT_DOCUMENTS: tuple[str, ...] = (
    "CHANGELOG.md",
    "roadmap.md",
    "status.md",
)
RELEASE_AUTH_DOCUMENT_EXTENSIONS = frozenset({".md", ".mdx", ".org"})
RELEASE_AUTH_IGNORED_DIRECTORY_NAMES = frozenset({"node_modules"})
MAX_RELEASE_AUTH_TREE_ENTRIES = 65_536
MAX_RELEASE_AUTH_TREE_DEPTH = 16
MAX_RELEASE_AUTH_DOCUMENTS = 60_000
RELEASE_AUTH_FORBIDDEN_PATTERNS: tuple[
    tuple[str, tuple[tuple[str, ...], ...], re.Pattern[str]], ...
] = (
    (
        "retired local manifest-authentication command",
        (("sorafs_cli",), ("manifest",)),
        re.compile(
            r"\bsorafs_cli(?:\.exe)?\s+manifest\s+"
            r"(?:sign|verify-signature)\b",
            re.IGNORECASE,
        ),
    ),
    (
        "retired identity-token signing option",
        (("--identity-token-",),),
        re.compile(
            r"--identity-token-(?:provider|audience|env|file)\b",
            re.IGNORECASE,
        ),
    ),
    (
        "retired ci_sample authentication artifact",
        (("fixtures/sorafs_manifest/ci_sample/manifest.",),),
        re.compile(
            r"fixtures/sorafs_manifest/ci_sample/manifest\."
            r"(?:bundle\.json|sig|sign\.summary\.json|verify\.summary\.json)\b",
            re.IGNORECASE,
        ),
    ),
    (
        "retired local manifest-authentication artifact",
        (("manifest.",),),
        re.compile(
            r"\bmanifest\."
            r"(?:bundle\.json|sign\.summary\.json|verify\.summary\.json)\b",
            re.IGNORECASE,
        ),
    ),
    (
        "generic OpenSSL/RSA signing command",
        (("openssl",), ("-sign", "genrsa", "genpkey")),
        re.compile(
            r"(?m)^[^\n]*\bopenssl[a-z0-9_]*\b(?:\.exe)?[^\n]*"
            r"(?:\bgenrsa\b|\bgenpkey\b|(?:^|\s)-sign(?:=|\s|$))[^\n]*$",
            re.IGNORECASE,
        ),
    ),
    (
        "OIDC-derived local Ed25519 signing material",
        (
            ("oidc", "jwt", "identity token", "identity-token"),
            ("deriv", "seed", "ephemeral"),
            ("ed25519", "key"),
        ),
        re.compile(
            r"(?m)^[^\n]*(?:"
            r"(?:oidc|jwt|identity[ -]?token)[^\n]{0,200}"
            r"(?:deriv(?:e|ed|ation)|seed|ephemeral)[^\n]{0,200}"
            r"(?:ed25519|key)"
            r"|"
            r"(?:deriv(?:e|ed|ation)|seed)[^\n]{0,200}"
            r"(?:ed25519|key)[^\n]{0,200}"
            r"(?:oidc|jwt|identity[ -]?token)"
            r")[^\n]*$",
            re.IGNORECASE,
        ),
    ),
)
RELEASE_AUTH_HISTORICAL_FINDINGS: dict[str, tuple[str, ...]] = {
    "specs/sorafs/reports/sf6_security_review.md": (
        "The retired CLI path derived an ephemeral key from unverified OIDC token bytes",
        "Removed that CLI surface and all production callers",
    ),
}
PACKAGE_RELEASE_SMOKE_SCRIPT = "python/iroha_python/scripts/release_smoke.sh"
PACKAGE_RELEASE_SMOKE_REQUIRED_MARKERS: tuple[str, ...] = (
    'PYTHON_RELEASE_SMOKE_KEEP_DIST',
    'python -m build',
    'pip install "${WHEEL}"',
    'assert hasattr(sdk, "ToriiClient")',
    'python/iroha_python/scripts/run_norito_rpc_smoke.sh',
    'python -m twine check',
    '--dry-run',
    'scripts/release_manifest_signing.py',
)
PACKAGE_RELEASE_SMOKE_FORBIDDEN_MARKERS: tuple[str, ...] = (
    "openssl",
    "cosign",
    ".sigstore",
    "--signing-key",
    "--manifest-out",
    "--require-sigstore",
    "--skip-sigstore",
    "--sigstore-token-env",
    "--cosign-bin",
    "PYTHON_RELEASE_SIGNING_KEY",
    "PYTHON_RELEASE_SIGSTORE",
    "SIGSTORE_ID_TOKEN",
    "release_artifacts.json",
    "CHANGELOG_PREVIEW.md",
    "SHA256SUMS",
    "sign-blob",
)
REFERENCE_SDK_RELEASE_EXAMPLE_REQUIRED_MARKERS: dict[str, tuple[str, ...]] = {
    "scripts/examples/sorafs_reference_sdk_release_supply_chain_canary.args.example": (
        "--kind\nsupply_chain",
        "--supply-chain-source-root",
        "--provenance-certificate-identity",
        "--provenance-oidc-issuer",
        "--provenance-verification-public-key-hex",
    ),
    "scripts/examples/sorafs_reference_sdk_release_collection.args.example": (
        "--require-kind release_archive,signed_manifest,supply_chain,"
        "downstream_bindings,cookbook_smoke,ffi_header_contract,"
        "governance_approval",
        "--supply-chain-source-root",
        "--provenance-certificate-identity",
        "--provenance-oidc-issuer",
        "--provenance-verification-public-key-hex",
    ),
    "scripts/examples/sorafs_reference_sdk_release_evidence.args.example": (
        "--require-kind release_archive,signed_manifest,supply_chain,"
        "downstream_bindings,cookbook_smoke,ffi_header_contract,"
        "governance_approval",
        "--supply-chain-source-root",
        "--provenance-certificate-identity",
        "--provenance-oidc-issuer",
        "--provenance-verification-public-key-hex",
    ),
}
REFERENCE_SDK_RELEASE_EXAMPLE_FORBIDDEN_MARKERS: dict[str, tuple[str, ...]] = {
    "scripts/examples/sorafs_reference_sdk_release_supply_chain_canary.args.example": (
        "--target",
        "--sbom-index-digest-hex",
        "--vulnerability-report-digest-hex",
        "--provenance-bundle-digest-hex",
    ),
}
GENERIC_OPENSSL_SIGNER_RE = re.compile(
    r"(?m)^[^\n]*\bopenssl[a-z0-9_]*\b(?:\.exe)?[^\n]*"
    r"(?:\bgenrsa\b|\bgenpkey\b|(?:^|\s)-sign(?:=|\s|$))[^\n]*$",
    re.IGNORECASE,
)
RUNTIME_PROVIDER_RELEASE_WORKFLOW_MARKERS: tuple[str, ...] = (
    '- "scripts/check_runtime_provider_broker_install.py"',
    '- "scripts/tests/check_runtime_provider_broker_install_test.py"',
    '- "configs/sorafs/runtime_provider_broker/**"',
    '- "crates/irohad/src/runtime_provider_broker.rs"',
    '- "crates/irohad/src/runtime_provider_broker/**"',
    '- "crates/irohad/src/sorafs_pop_runtime.rs"',
    "name: Validate runtime-provider broker deployment contract",
    "run: python3 scripts/tests/check_runtime_provider_broker_install_test.py",
)
POP_BROKER_OPERATION_IDS: dict[str, int] = {
    "OPERATION_POP_RUNTIME_OPEN_V1": 118,
    "OPERATION_POP_ENROLLMENT_RECIPIENT_OPEN_V1": 119,
    "OPERATION_POP_WALLET_RECIPIENT_OPEN_V1": 120,
}
POP_BROKER_RETIRED_SECRET_MARKERS: tuple[str, ...] = (
    "PopRuntimeResolveResultWireV1",
    "PopCredentialRuntimeSecretsV1",
    "enrollment_recipient_secret",
    "wallet_recipient_secret",
)
POP_BROKER_WIRE_FIELD_INVENTORIES: dict[str, tuple[tuple[str, str], ...]] = {
    "PopRuntimeOpenResultWireV1": (
        ("issuer_hsm_key_id", "String"),
        ("issuer_public_key", "[u8;32]"),
        ("enrollment_recipient_key_id", "String"),
        ("enrollment_recipient_public_key_digest", "[u8;32]"),
        ("wallet_recipient_key_id", "String"),
        ("wallet_recipient_public_key_digest", "[u8;32]"),
        ("wallet_wrapping_key_id", "String"),
    ),
    "PopRecipientOpenRequestWireV1": (
        (
            "encrypted_payload",
            "sorafs_manifest::hybrid_envelope::HybridPayloadEnvelopeV1",
        ),
        ("aad", "Vec<u8>"),
    ),
    "PopRecipientOpenResultWireV1": (("plaintext", "Vec<u8>"),),
    "PopCredentialRuntimeBindingWireV1": (
        ("issuer_policy_digest", "[u8;32]"),
        ("issuer_id", "String"),
        ("issuer_hsm_key_id", "String"),
        ("issuer_public_key", "[u8;32]"),
        ("enrollment_recipient_key_id", "String"),
        ("enrollment_recipient_public_key_digest", "[u8;32]"),
        ("wallet_recipient_key_id", "String"),
        ("wallet_recipient_public_key_digest", "[u8;32]"),
        ("wallet_wrapping_key_id", "String"),
    ),
}
RUNTIME_PROVIDER_DEPLOYMENT_ASSET_MARKERS: dict[str, tuple[str, ...]] = {
    "configs/sorafs/runtime_provider_broker/README.md": (
        "it does not supply a concrete HSM, KMS,",
        "statically link a reviewed deployment-owned",
        "Do not add credential, private-key, token, plugin, test-provider, or socket",
        "The expected executable digest must come",
        "Both checked-in Linux consumer dependencies are mandatory",
        "/run/iroha-runtime-provider-broker-v1/runtime-provider-broker-v1.sock",
        "/private/var/iroha/run/runtime-provider-broker-v1.sock",
        "mode-0600 instance lock",
    ),
    (
        "configs/sorafs/runtime_provider_broker/systemd/"
        "iroha-runtime-provider-broker-v1.service"
    ): (
        "Type=notify",
        "NotifyAccess=main",
        "User=iroha",
        "Group=iroha",
        "RuntimeDirectory=iroha-runtime-provider-broker-v1",
        "RuntimeDirectoryMode=0700",
        "RuntimeDirectoryPreserve=no",
        "ReadWritePaths=/run/iroha-runtime-provider-broker-v1",
        "LimitCORE=0",
        (
            "ExecStart=/usr/local/libexec/iroha-runtime-provider-broker-v1 "
            "--catalog /etc/iroha/runtime-provider-broker/catalog.norito"
        ),
    ),
    (
        "configs/sorafs/runtime_provider_broker/systemd/"
        "taira-irohad.service.d/20-runtime-provider-broker-v1.conf"
    ): (
        "Requires=iroha-runtime-provider-broker-v1.service",
        "After=iroha-runtime-provider-broker-v1.service",
        "User=iroha",
        "Group=iroha",
        "ReadOnlyPaths=/run/iroha-runtime-provider-broker-v1",
    ),
    (
        "configs/sorafs/runtime_provider_broker/systemd/"
        "sorafs-governance-dag@.service.d/"
        "20-runtime-provider-broker-v1.conf"
    ): (
        "Requires=iroha-runtime-provider-broker-v1.service",
        "After=iroha-runtime-provider-broker-v1.service",
        "User=iroha",
        "Group=iroha",
        "ReadOnlyPaths=/run/iroha-runtime-provider-broker-v1",
    ),
    (
        "configs/sorafs/runtime_provider_broker/launchd/"
        "org.hyperledger.iroha.runtime-provider-broker-v1.plist"
    ): (
        "org.hyperledger.iroha.runtime-provider-broker-v1",
        "/usr/local/libexec/iroha-runtime-provider-broker-v1",
        "/private/etc/iroha/runtime-provider-broker/catalog.norito",
        "<key>UserName</key>",
        "<key>GroupName</key>",
        "<key>SoftResourceLimits</key>\n  <dict>\n    <key>Core</key>",
        "<key>HardResourceLimits</key>\n  <dict>\n    <key>Core</key>",
    ),
    "scripts/check_runtime_provider_broker_install.py": (
        "supervisor_template: PurePosixPath",
        "consumer_assets: tuple[tuple[PurePosixPath, PurePosixPath], ...]",
        "checked-in runtime-provider supervisor template",
        "BROKER_EXECUTABLE_MAX_BYTES_V1",
        "_sha256_regular_bounded",
        "externally verified release digest",
        "stat.S_IMODE(info.st_mode) & 0o7222",
        "installed runtime-provider consumer drop-in",
        "checked-in platform template",
        "trusted_artifact_owner_uid=0",
        "--expected-catalog",
        "--expected-executable-sha256",
        'layout.platform == "macos"',
        "/private/var/iroha/run/runtime-provider-broker-v1.sock",
    ),
    "scripts/tests/check_runtime_provider_broker_install_test.py": (
        "test_supervisor_asset_is_required",
        "test_supervisor_asset_must_not_be_a_symlink",
        "test_supervisor_asset_must_have_one_hard_link",
        "test_supervisor_asset_rejects_unsafe_permissions",
        "test_supervisor_asset_rejects_untrusted_owner",
        "test_supervisor_asset_must_match_checked_in_template",
        "test_supervisor_asset_race_is_rejected",
        "test_macos_runtime_directory_is_unconditional",
        "test_executable_digest_must_match_external_release_identity",
        "test_executable_digest_must_be_canonical_nonzero_lowercase",
        "test_executable_race_is_rejected",
        "test_consumer_drop_ins_are_required_and_exact",
        "test_consumer_drop_in_hardlink_is_rejected",
    ),
    "crates/irohad/src/runtime_provider_broker.rs": (
        "/run/iroha-runtime-provider-broker-v1/runtime-provider-broker-v1.sock",
        "/private/var/iroha/run/runtime-provider-broker-v1.sock",
        "endpoint_recovery::prepare_endpoint",
        "OPERATION_POP_RUNTIME_OPEN_V1",
        "OPERATION_POP_ENROLLMENT_RECIPIENT_OPEN_V1",
        "OPERATION_POP_WALLET_RECIPIENT_OPEN_V1",
        'include!("runtime_provider_broker/pop_recipient_client.rs");',
    ),
    "crates/irohad/src/runtime_provider_broker/protocol_primitives.rs": (
        "OPERATION_POP_RUNTIME_OPEN_V1: u16 = 118",
        "OPERATION_POP_ENROLLMENT_RECIPIENT_OPEN_V1: u16 = 119",
        "OPERATION_POP_WALLET_RECIPIENT_OPEN_V1: u16 = 120",
        "struct PopRuntimeOpenResultWireV1",
        "struct PopRecipientOpenRequestWireV1",
        "struct PopRecipientOpenResultWireV1",
        "struct PopCredentialRuntimeBindingWireV1",
    ),
    "crates/irohad/src/runtime_provider_broker/validate_operation_payload.rs": (
        "OPERATION_POP_RUNTIME_OPEN_V1",
        "OPERATION_POP_ENROLLMENT_RECIPIENT_OPEN_V1",
        "OPERATION_POP_WALLET_RECIPIENT_OPEN_V1",
        "validate_pop_recipient_open_request(&open, request.operation)",
    ),
    "crates/irohad/src/runtime_provider_broker/pop_recipient_client.rs": (
        "OPERATION_POP_ENROLLMENT_RECIPIENT_OPEN_V1",
        "OPERATION_POP_WALLET_RECIPIENT_OPEN_V1",
        "PopRecipientOpenRequestWireV1",
        "PopRecipientOpenResultWireV1",
        '.field("private_recipient", &"[REMOTE]")',
    ),
    "crates/irohad/src/sorafs_pop_runtime.rs": (
        "production_builder_has_no_secret_or_fallback_source",
        "provider material remains behind the deployment-supplied registry.",
    ),
    "crates/irohad/src/runtime_provider_broker/launcher.rs": (
        "trusted_runtime_provider_catalog_owner_uid_v1",
        "changed_seconds: metadata.ctime()",
        "changed_nanoseconds: metadata.ctime_nsec()",
        "before.owner != trusted_owner_uid",
        "before.mode & 0o7222",
    ),
    "crates/irohad/src/runtime_provider_broker/protocol/platform/endpoint_recovery.rs": (
        "NonBlockingLockExclusive",
        "marker_preexisted: bool",
        "create_lock_exclusively(parent_directory)",
        "if !guard.marker_preexisted",
        "remove_exact_socket_entry_inner",
        "rename_to_quarantine",
        "rustix::fs::RenameFlags::NOREPLACE",
        "metadata.st_nlink == 1",
    ),
    "crates/irohad/src/runtime_provider_broker/server_source_tests.rs": (
        "broker_server_preserves_active_listener_without_lock_or_readiness",
        "broker_server_recovers_exact_stale_socket_after_unclean_exit",
        "broker_server_rejects_active_locked_socket_without_unlinking_it",
        "stale_socket_recovery_detects_identity_substitution_before_unlink",
        "orderly_cleanup_quarantines_before_detecting_identity_substitution",
        "broker_endpoint_rejects_socket_hardlink_alias_without_removal",
    ),
}
RUNTIME_PROVIDER_DEPLOYMENT_FORBIDDEN_MARKERS: dict[str, tuple[str, ...]] = {
    (
        "configs/sorafs/runtime_provider_broker/systemd/"
        "iroha-runtime-provider-broker-v1.service"
    ): (
        "Environment=",
        "EnvironmentFile=",
        "--socket",
        "--plugin",
        "--private-key",
        "--credential",
    ),
    (
        "configs/sorafs/runtime_provider_broker/launchd/"
        "org.hyperledger.iroha.runtime-provider-broker-v1.plist"
    ): (
        "EnvironmentVariables",
        "--socket",
        "--plugin",
        "--private-key",
        "--credential",
    ),
    "scripts/check_runtime_provider_broker_install.py": (
        "--socket",
        "--plugin",
        "--private-key",
        "--credential",
    ),
    "crates/irohad/src/runtime_provider_broker.rs": (
        "OPERATION_POP_RUNTIME_RESOLVE_V1",
        "HybridSecretKey",
        *POP_BROKER_RETIRED_SECRET_MARKERS,
    ),
    "crates/irohad/src/runtime_provider_broker/protocol_primitives.rs": (
        "OPERATION_POP_RUNTIME_RESOLVE_V1",
        "u16 = 60;",
        "HybridSecretKey",
        *POP_BROKER_RETIRED_SECRET_MARKERS,
    ),
    "crates/irohad/src/runtime_provider_broker/validate_operation_payload.rs": (
        "OPERATION_POP_RUNTIME_RESOLVE_V1",
        "HybridSecretKey",
        *POP_BROKER_RETIRED_SECRET_MARKERS,
    ),
    "crates/irohad/src/runtime_provider_broker/pop_recipient_client.rs": (
        "OPERATION_POP_RUNTIME_RESOLVE_V1",
        "HybridSecretKey",
        ".secret()",
        "recipient_private_key",
        "recipient_secret",
        *POP_BROKER_RETIRED_SECRET_MARKERS,
    ),
}
WORKFLOWS: dict[str, tuple[str, ...]] = {
    ".github/workflows/sorafs-cli-release.yml": (
        '"sorafs-cli-v*"',
        "actions/checkout@df4cb1c069e1874edd31b4311f1884172cec0e10",
        "actions/setup-python@ece7cb06caefa5fff74198d8649806c4678c61a1",
        "scripts/check_sorafs_release_version_map.py",
        "version-map-summary.first.json",
        "version-map-summary.replay.json",
        "cmp version-map-summary.first.json version-map-summary.replay.json",
        "cp version-map-summary.first.json version-map-summary.json",
        "scripts/check_sorafs_reference_sdk_release_evidence.py",
        "scripts/build_sorafs_reference_sdk_release_canary.py",
        '- "scripts/build_sorafs_reference_sdk_supply_chain_sources.py"',
        '- "scripts/sorafs_reference_sdk_supply_chain.py"',
        '- "scripts/sorafs_topology_qualification.py"',
        "scripts/run_sorafs_reference_sdk_release_evidence.py",
        "scripts/check_workflow_action_pins.py",
        *RUNTIME_PROVIDER_RELEASE_WORKFLOW_MARKERS,
        '- "Dockerfile"',
        '- "scripts/build_release_bundle.sh"',
        '- "scripts/build_release_image.sh"',
        '- "scripts/build_release_oci_archive.py"',
        '- "scripts/build_release_tar_gz.py"',
        '- "scripts/build_release_tar_zst.py"',
        '- "scripts/capture_release_command.py"',
        '- "scripts/copy_release_file.py"',
        '- "scripts/copy_release_tree.py"',
        '- "scripts/generate_release_manifest.py"',
        '- "scripts/generate_sorafs_cli_release_manifest.py"',
        '- "scripts/release_artifact_contract.py"',
        '- "scripts/release_manifest_signing.py"',
        '- "scripts/publish_plan.py"',
        '- "scripts/run_release_pipeline.py"',
        '- "scripts/write_release_checksum.py"',
        '- "scripts/write_release_sha256sums.py"',
        '- "scripts/validate_release_image_bases.py"',
        '- "scripts/requirements.txt"',
        "python3 -m pip install -r scripts/requirements.txt",
        "python3 scripts/check_workflow_action_pins.py",
        "scripts/tests/check_sorafs_reference_sdk_release_evidence_test.py",
        '- "scripts/tests/build_sorafs_reference_sdk_supply_chain_sources_test.py"',
        '- "scripts/tests/sorafs_reference_sdk_supply_chain_test.py"',
        '- "scripts/tests/sorafs_topology_qualification_test.py"',
        "run: bash ci/check_sorafs_cli_release.sh",
        "scripts/package_sorafs_validate_release.sh",
        "scripts/package_sorafs_cli_candidate.py",
        "scripts/tests/package_sorafs_cli_candidate_test.py",
        '- "scripts/tests/build_release_bundle_test.py"',
        '- "scripts/tests/build_release_image_test.py"',
        '- "scripts/tests/capture_release_command_test.py"',
        '- "scripts/tests/release_artifact_contract_test.py"',
        '- "scripts/tests/validate_release_image_bases_test.py"',
        '- "scripts/tests/release_profile_validation_test.py"',
        '- "scripts/tests/release_manifest_signing_test.py"',
        '- "scripts/tests/release_manifest_signing_test.sh"',
        '- "scripts/tests/generate_release_manifest_test.py"',
        '- "scripts/tests/generate_sorafs_cli_release_manifest_test.py"',
        '- "scripts/tests/publish_plan_test.py"',
        '- "scripts/tests/check_sorafs_rollout_gate_contract_test.py"',
        '- "python/iroha_python/scripts/release_smoke.sh"',
        '- "python/iroha_python/README.md"',
        '- "specs/sdk/python/release_automation*.md"',
        '- "specs/sdk/python/support_playbook*.md"',
        '- "specs/sorafs_release_pipeline_plan*.md"',
        '- "specs/release_dual_track_automation_plan*.md"',
        '- "specs/release_dual_track_runbook*.md"',
        '- "specs/release_artifact_selection*.md"',
        '- "specs/sora_nexus_operator_onboarding*.md"',
        '- "CHANGELOG.md"',
        '- "LICENSE"',
        "anchore/sbom-action@e22c389904149dbc22b58101806040fa8d37a610",
        "anchore/scan-action@e1165082ffb1fe366ebaf02d8526e7c4989ea9d2",
        "syft-version: v1.44.0",
        "grype-version: v0.112.0",
        "severity-cutoff: high",
        "fail-build: true",
        'cargo build --locked --release -p sorafs_orchestrator --bin sorafs_cli --target "$target"',
        'cargo build --locked --release -p sorafs_car --features cli --bin sorafs_fetch --target "$target"',
        'cargo build --locked --release -p sorafs_manifest --bin sorafs-validate --target "$target"',
        'if [[ "$host_target" != "$target" ]]; then',
        "target: ${{ matrix.target }}",
        '--target "${{ matrix.target }}"',
        'source_commit="${GITHUB_SHA}"',
        'source_date_epoch="$(git show -s --format=%ct "$source_commit")"',
        '--source-commit "$source_commit"',
        '--source-date-epoch "$source_date_epoch"',
        'binary_suffix: ".exe"',
        "find . -type f ! -name SHA256SUMS -print",
        "done > SHA256SUMS",
        "name: sorafs-cli-release-gate-${{ github.run_id }}",
        "path: artifacts/release-gate",
        "artifacts/release-gate/artifacts/sorafs-release/sorafs-release.spdx.json",
        "name: Generate platform binary SBOM",
        "name: Package reference validator and FFI header",
        "name: Package reference validator and FFI header reproducibly",
        "sorafs-validate-first.XXXXXX",
        "sorafs-validate-replay.XXXXXX",
        'cmp "${first_out}/${relative}" "${replay_out}/${relative}"',
        "for suffix in .tar.gz.sha256 .manifest.json.sha256; do",
        "cmp candidate-package-first.json candidate-package-replay.json",
        "artifacts/sorafs-cli/reference-validator",
        "output-file: artifacts/sorafs-cli/sorafs-cli-${{ matrix.target }}.spdx.json",
        "name: Scan platform binary SBOM",
        "output-file: artifacts/sorafs-cli/sorafs-cli-${{ matrix.target }}-vulnerabilities.sarif",
        "specs/sorafs/runbooks/release_rollback_yank.md artifacts/sorafs-cli/ROLLBACK-YANK.md",
        "cp CHANGELOG.md LICENSE artifacts/sorafs-cli/",
        "name: Rebuild deterministic platform archive and run clean-consumer smoke",
        "candidate-package-first.json",
        "candidate-package-replay.json",
        "artifacts/sorafs-cli/platform-archive",
        "name: Stage source release scan evidence",
        "name: Finalize platform checksums",
        "  prepare-release-manifest:",
        "name: Build the canonical foundational manifest twice",
        "python3 scripts/generate_sorafs_cli_release_manifest.py create",
        "name: Upload unsigned foundational manifest for external HSM signing",
        "  verify-release-auth:",
        "environment: sorafs-release-authentication",
        "runs-on: [self-hosted, linux, x64, sorafs-release-auth]",
        "SORAFS_RELEASE_SIGNATURE_PATH: ${{ vars.SORAFS_RELEASE_SIGNATURE_PATH }}",
        "SORAFS_RELEASE_PUBLIC_KEY_PATH: ${{ vars.SORAFS_RELEASE_PUBLIC_KEY_PATH }}",
        "SORAFS_RELEASE_MANIFEST_VERIFIER_PATH: ${{ vars.SORAFS_RELEASE_MANIFEST_VERIFIER_PATH }}",
        "SORAFS_TRUSTED_RELEASE_SIGNING_FINGERPRINT: ${{ vars.SORAFS_TRUSTED_RELEASE_SIGNING_FINGERPRINT }}",
        "SORAFS_TRUSTED_RELEASE_MANIFEST_VERIFIER_SHA256: ${{ vars.SORAFS_TRUSTED_RELEASE_MANIFEST_VERIFIER_SHA256 }}",
        "name: Reconcile the foundational manifest with the immutable candidates",
        "name: Verify the protected external Ed25519 manifest tuple",
        "python3 scripts/release_manifest_signing.py verify",
        "name: Upload authenticated foundational manifest tuple",
        "name: Verify authenticated release-manifest candidate binding before provenance",
        "needs: [release-gate, package, verify-release-auth]",
        "sigstore/cosign-installer@ba7bc0a3fef59531c69a25acd34668d6d3fe6f22",
        "name: Verify platform package checksums before signing",
        "signed_artifact_id: ${{ steps.upload-signed.outputs.artifact-id }}",
        "signed_artifact_digest: ${{ steps.upload-signed.outputs.artifact-digest }}",
        "signed_artifact_url: ${{ steps.upload-signed.outputs.artifact-url }}",
        "id: upload-signed",
        '[[ "${#checksum_files[@]}" -ne 5 ]]',
        "sha256sum --check SHA256SUMS",
        "SHA256SUMS contains duplicate file entries",
        "SHA256SUMS does not cover the exact platform candidate file set",
        "actions/attest@a1948c3f048ba23858d222213b7c278aabede763",
        "name: Attest aggregate signed-input provenance",
        "name: Stage offline provenance bundles",
        '[[ "$attestation_path" != "${RUNNER_TEMP}/"*',
        '[[ "$count" -gt 16 ]]',
        "cosign sign-blob --yes --bundle",
        "cosign verify-blob",
        'certificate-identity "https://github.com/${GITHUB_REPOSITORY}/.github/workflows/sorafs-cli-release.yml@${GITHUB_REF}"',
        'certificate-oidc-issuer "https://token.actions.githubusercontent.com"',
        "id-token: write",
        "name: Assemble and validate the canonical SF-11 source indexes",
        "name: Build and gate source-derived SF-11 supply-chain evidence",
        "name: Record the immutable signed-input artifact binding",
        "sorafs.reference_sdk.signed_input_artifact.v1",
        "artifacts/reference-sdk-evidence/l1-topology-qualification.summary.json",
        "artifacts/reference-sdk-evidence/l1-topology-qualification.envelope.json",
        "sorafs.l1.deployment_qualification.trust.v1",
        "--topology-qualification-verification-public-key-hex",
        "--topology-qualification-signer-identity",
        "--topology-qualification-signer-key-revision",
        "--topology-qualification-signer-policy-digest-hex",
        "--max-topology-qualification-review-age-secs 1209600",
        "expected one aggregate offline provenance bundle",
        '"signed-input/github-attestations/${target}.json"',
        'gh attestation verify "$provenance_file"',
        "realpath -e --",
        "SORAFS_TRUSTED_GH_CLI_SHA256: ${{ vars.SORAFS_TRUSTED_GH_CLI_SHA256 }}",
        "name: Upload replay-complete SF-11 supply-chain evidence",
        "sf11-source/",
        "  reference-sdk-supply-chain-evidence:",
        "needs: [release-gate, sign]",
        "environment: sorafs-reference-sdk-evidence",
        "SORAFS_REFERENCE_SDK_DEPLOYMENT_ID: ${{ vars.SORAFS_REFERENCE_SDK_DEPLOYMENT_ID }}",
        "SORAFS_REFERENCE_SDK_RECEIPTS_ROOT: ${{ vars.SORAFS_REFERENCE_SDK_RECEIPTS_ROOT }}",
        "SORAFS_PROVENANCE_VERIFICATION_PUBLIC_KEY_HEX: ${{ vars.SORAFS_PROVENANCE_VERIFICATION_PUBLIC_KEY_HEX }}",
        "SORAFS_TRUSTED_GH_CLI_SHA256: ${{ vars.SORAFS_TRUSTED_GH_CLI_SHA256 }}",
        "SORAFS_L1_TOPOLOGY_QUALIFICATION_SUMMARY_PATH: ${{ vars.SORAFS_L1_TOPOLOGY_QUALIFICATION_SUMMARY_PATH }}",
        "SORAFS_L1_TOPOLOGY_QUALIFICATION_ENVELOPE_PATH: ${{ vars.SORAFS_L1_TOPOLOGY_QUALIFICATION_ENVELOPE_PATH }}",
        "SORAFS_L1_TOPOLOGY_QUALIFICATION_VERIFICATION_PUBLIC_KEY_HEX: ${{ vars.SORAFS_L1_TOPOLOGY_QUALIFICATION_VERIFICATION_PUBLIC_KEY_HEX }}",
        "SORAFS_L1_TOPOLOGY_QUALIFICATION_SIGNER_IDENTITY: ${{ vars.SORAFS_L1_TOPOLOGY_QUALIFICATION_SIGNER_IDENTITY }}",
        "SORAFS_L1_TOPOLOGY_QUALIFICATION_SIGNER_KEY_REVISION: ${{ vars.SORAFS_L1_TOPOLOGY_QUALIFICATION_SIGNER_KEY_REVISION }}",
        "SORAFS_L1_TOPOLOGY_QUALIFICATION_SIGNER_POLICY_DIGEST_HEX: ${{ vars.SORAFS_L1_TOPOLOGY_QUALIFICATION_SIGNER_POLICY_DIGEST_HEX }}",
        "--external-receipts-root \"$SORAFS_REFERENCE_SDK_RECEIPTS_ROOT\"",
        "--supply-chain-source-root sf11-source",
        "--provenance-certificate-identity \"$certificate_identity\"",
        "--provenance-oidc-issuer \"$oidc_issuer\"",
        '"$SORAFS_PROVENANCE_VERIFICATION_PUBLIC_KEY_HEX"',
        "sf11-source/",
    ),
    ".github/workflows/sorafs-fixtures-nightly.yml": (
        'cron: "17 2 * * *"',
        "actions/checkout@df4cb1c069e1874edd31b4311f1884172cec0e10",
        "actions/setup-python@ece7cb06caefa5fff74198d8649806c4678c61a1",
        'python-version: "3.12"',
        "bash ci/check_sorafs_fixtures.sh",
        "actions/setup-go@924ae3a1cded613372ab5595356fb5720e22ba16",
        'go-version: "1.26.x"',
        "actions/setup-node@48b55a011bda9f5d6aeb4c2d9c7362e8dae4041e",
        'node-version: "24"',
        "actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02",
    ),
    ".github/workflows/sorafs-orchestrator-sdk.yml": (
        'cron: "41 3 * * *"',
        "actions/checkout@df4cb1c069e1874edd31b4311f1884172cec0e10",
        "actions/setup-python@ece7cb06caefa5fff74198d8649806c4678c61a1",
        'python-version: "3.12"',
        "name: Bind the canonical mobile Python",
        'echo "MOBILE_SDK_PYTHON_BINARY=$mobile_python" >> "$GITHUB_ENV"',
        "actions/setup-node@48b55a011bda9f5d6aeb4c2d9c7362e8dae4041e",
        'node-version: "24"',
        "runs-on: macos-14",
        "  mobile-parity:",
        "  csharp-parity:",
        "actions/setup-java@c1e323688fd81a25caa38c78aa6df2d33d3e20d9",
        "actions/setup-dotnet@67a3573c9a986a3f9c594539f4ab511d57bb3ce9",
        "bash ci/check_sorafs_python_native_sdk.sh",
        "bash ci/sdk_sorafs_orchestrator.sh",
        "bash ci/check_kagemusha_jvm_native_bridge.sh",
        "dotnet test Hyperledger.Iroha.Sdk.sln -c Release --no-build",
        "actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02",
    ),
}
NATIVE_GOVERNANCE_VALIDATION_REQUIRED_ENV = (
    "IROHA_REQUIRE_SORAFS_NATIVE_VALIDATION"
)
MOBILE_SDK_ARTIFACTS_WORKFLOW = ".github/workflows/mobile_sdk_artifacts.yml"
SWIFT_GOVERNANCE_VALIDATOR_TEST = (
    "IrohaSwift/Tests/IrohaSwiftTests/SorafsReferenceValidatorsTests.swift"
)
KOTLIN_GOVERNANCE_VALIDATOR_TEST = (
    "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/sorafs/"
    "SorafsReferenceValidatorsTest.kt"
)
JAVA_GOVERNANCE_VALIDATOR_TEST = (
    "java/iroha_android/src/test/java/org/hyperledger/iroha/android/sorafs/"
    "SorafsReferenceValidatorsTests.java"
)
JAVA_GOVERNANCE_WORKFLOW_STEP_NAME = (
    "name: Test mirrored Java Android Governance DAG reference validators"
)
JAVA_GOVERNANCE_WORKFLOW_STEP_MARKERS = (
    "working-directory: java/iroha_android",
    (
        "ANDROID_HARNESS_MAINS: "
        "org.hyperledger.iroha.android.sorafs.SorafsReferenceValidatorsTests"
    ),
    "--no-daemon",
    "--no-configuration-cache",
    '--project-cache-dir "$MOBILE_SDK_ANDROID_PROJECT_CACHE_DIR/java-sorafs"',
    '-Djava.io.tmpdir="$MOBILE_SDK_ANDROID_GRADLE_TMP_DIR/java-sorafs"',
    (
        "-Dkotlin.daemon.runFilesPath="
        '"$MOBILE_SDK_ANDROID_GRADLE_TMP_DIR/kotlin-daemon-java-sorafs"'
    ),
    "--tests org.hyperledger.iroha.android.GradleHarnessTests",
)
NATIVE_GOVERNANCE_SDK_CONTRACTS: dict[str, tuple[str, ...]] = {
    MOBILE_SDK_ARTIFACTS_WORKFLOW: (
        '- "scripts/check_sorafs_release_automation.py"',
        '- "scripts/tests/check_sorafs_release_automation_test.py"',
        '- "java/iroha_android/**"',
        '- "fixtures/sorafs_manifest/governance/**"',
        "name: Build host SoraFS reference native bridge",
        "run: cargo build --locked -p connect_norito_bridge",
        "IROHA_NATIVE_LIBRARY_PATH: ${{ github.workspace }}/target/debug",
        JAVA_GOVERNANCE_WORKFLOW_STEP_NAME,
        *JAVA_GOVERNANCE_WORKFLOW_STEP_MARKERS,
    ),
    SWIFT_GOVERNANCE_VALIDATOR_TEST: (
        "ABI-21 connect_norito_bridge with Governance DAG symbols is required.",
        "guard try requireGovernanceDagNativeBridge() else",
        "XCTFail(\"\\(Self.nativeValidationRequiredMessage) \\(unavailableMessage)\")",
    ),
    KOTLIN_GOVERNANCE_VALIDATOR_TEST: (
        "ABI-21 connect_norito_bridge with Governance DAG symbols is required.",
        "        requireGovernanceDagNativeBridge()\n",
        "throw AssertionError(requiredMessage)",
    ),
    JAVA_GOVERNANCE_VALIDATOR_TEST: (
        "ABI-21 connect_norito_bridge with all SoraFS reference symbols is required.",
        "  private static void requireNativeBridge() {\n",
        (
            "  private static void "
            "validatesGovernanceDagFixturesAndNegativeVectorsWhenNativeBridgeIsAvailable()\n"
            "      throws IOException {\n"
            "    requireNativeBridge();\n"
        ),
        "throw new AssertionError(",
    ),
}


def _release_auth_document_paths(root: Path) -> tuple[str, ...]:
    """Return a bounded, no-follow release-auth document inventory."""

    paths = [
        *RELEASE_AUTH_ROOT_DOCUMENTS,
        *RELEASE_AUTH_HISTORICAL_FINDINGS,
    ]
    docs_root = root / "docs"
    try:
        docs_metadata = docs_root.lstat()
    except OSError as error:
        raise ValueError("docs: release-auth documentation tree is missing") from error
    if stat.S_ISLNK(docs_metadata.st_mode) or not stat.S_ISDIR(docs_metadata.st_mode):
        raise ValueError("docs: release-auth documentation tree is missing")

    pending: list[tuple[Path, tuple[str, ...], os.stat_result]] = [
        (docs_root, (), docs_metadata)
    ]
    visited_entries = 0
    document_count = 0
    while pending:
        directory, prefix, expected_metadata = pending.pop()
        relative_directory = "/".join(("docs", *prefix))
        try:
            observed_metadata = directory.lstat()
        except OSError as error:
            raise ValueError(
                f"{relative_directory}: release-auth documentation directory "
                "changed during enumeration"
            ) from error
        if (
            stat.S_ISLNK(observed_metadata.st_mode)
            or not stat.S_ISDIR(observed_metadata.st_mode)
            or (observed_metadata.st_dev, observed_metadata.st_ino)
            != (expected_metadata.st_dev, expected_metadata.st_ino)
        ):
            raise ValueError(
                f"{relative_directory}: release-auth documentation directory "
                "changed during enumeration"
            )
        try:
            with os.scandir(directory) as iterator:
                entries = sorted(iterator, key=lambda entry: entry.name)
        except OSError as error:
            raise ValueError(
                f"{relative_directory}: release-auth documentation directory "
                "cannot be enumerated safely"
            ) from error

        for entry in entries:
            if entry.name in RELEASE_AUTH_IGNORED_DIRECTORY_NAMES:
                continue
            visited_entries += 1
            if visited_entries > MAX_RELEASE_AUTH_TREE_ENTRIES:
                raise ValueError(
                    "docs: release-auth documentation tree exceeds its entry limit"
                )
            entry_prefix = (*prefix, entry.name)
            relative = "/".join(("docs", *entry_prefix))
            try:
                metadata = entry.stat(follow_symlinks=False)
            except OSError as error:
                raise ValueError(
                    f"{relative}: release-auth documentation entry changed "
                    "during enumeration"
                ) from error
            if stat.S_ISLNK(metadata.st_mode):
                raise ValueError(
                    f"{relative}: release-auth documentation paths must not "
                    "contain symlinks"
                )
            if stat.S_ISDIR(metadata.st_mode):
                if len(entry_prefix) > MAX_RELEASE_AUTH_TREE_DEPTH:
                    raise ValueError(
                        "docs: release-auth documentation tree exceeds its depth limit"
                    )
                pending.append((directory / entry.name, entry_prefix, metadata))
                continue
            if not stat.S_ISREG(metadata.st_mode):
                raise ValueError(
                    f"{relative}: release-auth documentation tree contains a "
                    "non-regular entry"
                )
            if Path(entry.name).suffix.lower() not in RELEASE_AUTH_DOCUMENT_EXTENSIONS:
                continue
            if metadata.st_nlink != 1:
                raise ValueError(
                    f"{relative}: release-auth documents must not be hard linked"
                )
            document_count += 1
            if document_count > MAX_RELEASE_AUTH_DOCUMENTS:
                raise ValueError(
                    "docs: release-auth documentation tree exceeds its document limit"
                )
            paths.append(relative)
    return tuple(sorted(paths))


def _historical_release_auth_finding_is_allowed(
    relative: str,
    finding: str,
    match_name: str,
) -> bool:
    """Allow only the reviewed SF-6 finding that documents the removed design."""

    if match_name != "OIDC-derived local Ed25519 signing material":
        return False
    markers = RELEASE_AUTH_HISTORICAL_FINDINGS.get(relative)
    return markers is not None and all(marker in finding for marker in markers)


def _validate_release_auth_document_tree(root: Path) -> list[str]:
    """Reject retired or competing release-authentication guidance everywhere."""

    errors: list[str] = []
    for relative in _release_auth_document_paths(root):
        path = _require_regular_repo_file(root, relative)
        try:
            source = _read_bytes_no_follow(path).decode("utf-8")
        except UnicodeDecodeError as error:
            raise ValueError(
                f"{relative}: release-auth document must be UTF-8"
            ) from error
        lowered_source = source.lower()
        for match_name, prefilter_groups, pattern in RELEASE_AUTH_FORBIDDEN_PATTERNS:
            if not all(
                any(needle in lowered_source for needle in group)
                for group in prefilter_groups
            ):
                continue
            for match in pattern.finditer(source):
                finding = match.group(0)
                if _historical_release_auth_finding_is_allowed(
                    relative,
                    finding,
                    match_name,
                ):
                    continue
                line_number = source.count("\n", 0, match.start()) + 1
                errors.append(
                    f"{relative}:{line_number}: forbidden release-auth documentation "
                    f"reference ({match_name})"
                )
    return errors


def _validate_package_release_smoke(root: Path) -> list[str]:
    """Keep the Python release harness smoke-only and signer-free."""

    path = _require_regular_repo_file(root, PACKAGE_RELEASE_SMOKE_SCRIPT)
    try:
        source = _read_bytes_no_follow(path).decode("utf-8")
    except UnicodeDecodeError as error:
        raise ValueError(
            f"{PACKAGE_RELEASE_SMOKE_SCRIPT}: package release smoke must be UTF-8"
        ) from error
    errors: list[str] = []
    for marker in PACKAGE_RELEASE_SMOKE_REQUIRED_MARKERS:
        if marker not in source:
            errors.append(
                f"{PACKAGE_RELEASE_SMOKE_SCRIPT}: missing package-smoke contract "
                f"marker `{marker}`"
            )
    lowered_source = source.lower()
    for marker in PACKAGE_RELEASE_SMOKE_FORBIDDEN_MARKERS:
        if marker.lower() in lowered_source:
            errors.append(
                f"{PACKAGE_RELEASE_SMOKE_SCRIPT}: signing/provenance marker "
                f"`{marker}` is forbidden in the smoke harness"
            )
    if GENERIC_OPENSSL_SIGNER_RE.search(source) is not None:
        errors.append(
            f"{PACKAGE_RELEASE_SMOKE_SCRIPT}: generic OpenSSL/RSA signing is "
            "forbidden in the smoke harness"
        )
    return errors


def _validate_reference_sdk_release_examples(root: Path) -> list[str]:
    """Require source-bound SF-11 examples and reject retired manual inputs."""

    errors: list[str] = []
    for relative, markers in sorted(
        REFERENCE_SDK_RELEASE_EXAMPLE_REQUIRED_MARKERS.items()
    ):
        path = _require_regular_repo_file(root, relative)
        try:
            source = _read_bytes_no_follow(path).decode("utf-8")
        except UnicodeDecodeError as error:
            raise ValueError(
                f"{relative}: reference-SDK release example must be UTF-8"
            ) from error
        for marker in markers:
            if marker not in source:
                errors.append(
                    f"{relative}: missing source-bound example marker `{marker}`"
                )
        for marker in REFERENCE_SDK_RELEASE_EXAMPLE_FORBIDDEN_MARKERS.get(
            relative,
            (),
        ):
            if marker in source:
                errors.append(
                    f"{relative}: retired manual supply-chain marker `{marker}`"
                )
    return errors


def _workflow_job(source: str, name: str) -> str | None:
    """Return one top-level workflow job, including its header."""

    match = re.search(
        rf"(?ms)^  {re.escape(name)}:\n.*?(?=^  [A-Za-z0-9_-]+:\n|\Z)",
        source,
    )
    return match.group(0) if match is not None else None


def _contract_section(
    source: str,
    start_marker: str,
    end_marker: str,
) -> str | None:
    """Return one bounded source section used by a static SDK contract."""

    try:
        start = source.index(start_marker)
        end = source.index(end_marker, start + len(start_marker))
    except ValueError:
        return None
    return source[start:end]


def _validate_native_governance_sdk_contract(root: Path) -> list[str]:
    """Require fail-closed native Governance DAG parity in release SDK jobs."""

    errors: list[str] = []
    sources: dict[str, str] = {}
    for relative, markers in NATIVE_GOVERNANCE_SDK_CONTRACTS.items():
        path = _require_regular_repo_file(root, relative)
        try:
            source = _read_bytes_no_follow(path).decode("utf-8")
        except UnicodeDecodeError as error:
            raise ValueError(
                f"{relative}: native Governance DAG SDK contract must be UTF-8"
            ) from error
        sources[relative] = source
        for marker in markers:
            if marker not in source:
                errors.append(
                    f"{relative}: missing native Governance DAG contract marker "
                    f"`{marker}`"
                )

    workflow = sources[MOBILE_SDK_ARTIFACTS_WORKFLOW]
    required_env_marker = (
        f'{NATIVE_GOVERNANCE_VALIDATION_REQUIRED_ENV}: "1"'
    )
    for job_name in ("apple-mobile-sdk", "android-mobile-sdk"):
        job = _workflow_job(workflow, job_name)
        if job is None:
            errors.append(
                f"{MOBILE_SDK_ARTIFACTS_WORKFLOW}: missing `{job_name}` job"
            )
        elif required_env_marker not in job:
            errors.append(
                f"{MOBILE_SDK_ARTIFACTS_WORKFLOW}: `{job_name}` must require "
                "native Governance DAG validation"
            )
    if workflow.count(required_env_marker) != 2:
        errors.append(
            f"{MOBILE_SDK_ARTIFACTS_WORKFLOW}: native Governance DAG validation "
            "must be required exactly once in each release SDK job"
        )
    java_workflow_section = _contract_section(
        workflow,
        JAVA_GOVERNANCE_WORKFLOW_STEP_NAME,
        "name: Validate Android mobile SDK artifact",
    )
    if java_workflow_section is None:
        errors.append(
            f"{MOBILE_SDK_ARTIFACTS_WORKFLOW}: malformed mirrored Java Android "
            "Governance DAG validation step"
        )
    else:
        for marker in JAVA_GOVERNANCE_WORKFLOW_STEP_MARKERS:
            if marker not in java_workflow_section:
                errors.append(
                    f"{MOBILE_SDK_ARTIFACTS_WORKFLOW}: mirrored Java Android "
                    "Governance DAG validation step is missing "
                    f"`{marker}`"
                )

    swift = sources[SWIFT_GOVERNANCE_VALIDATOR_TEST]
    swift_section = _contract_section(
        swift,
        (
            "func "
            "testValidatesGovernanceDagFixturesAndNegativeVectorsWhenNativeBridgeIsAvailable"
        ),
        "func testSignsOrderbookFixtureWhenNativeBridgeIsAvailable",
    )
    if swift_section is None:
        errors.append(
            f"{SWIFT_GOVERNANCE_VALIDATOR_TEST}: malformed Governance DAG golden test"
        )
    elif "XCTSkipIf(" in swift_section:
        errors.append(
            f"{SWIFT_GOVERNANCE_VALIDATOR_TEST}: Governance DAG golden test "
            "must not unconditionally skip when the native bridge is unavailable"
        )
    for marker in ("XCTSkip(", NATIVE_GOVERNANCE_VALIDATION_REQUIRED_ENV):
        if marker in swift:
            errors.append(
                f"{SWIFT_GOVERNANCE_VALIDATOR_TEST}: native validation must fail "
                f"without the capability-skip marker `{marker}`"
            )

    kotlin = sources[KOTLIN_GOVERNANCE_VALIDATOR_TEST]
    kotlin_section = _contract_section(
        kotlin,
        (
            "fun "
            "validatesGovernanceDagFixturesAndNegativeVectorsWhenNativeBridgeIsAvailable"
        ),
        "fun signsOrderbookFixtureWhenNativeBridgeIsAvailable",
    )
    if kotlin_section is None:
        errors.append(
            f"{KOTLIN_GOVERNANCE_VALIDATOR_TEST}: malformed Governance DAG golden test"
        )
    elif (
        "assumeTrue(SorafsReferenceValidators.isNativeAvailable()"
        in kotlin_section
    ):
        errors.append(
            f"{KOTLIN_GOVERNANCE_VALIDATOR_TEST}: Governance DAG golden test "
            "must not unconditionally skip when the native bridge is unavailable"
        )
    for marker in ("assumeTrue(", NATIVE_GOVERNANCE_VALIDATION_REQUIRED_ENV):
        if marker in kotlin:
            errors.append(
                f"{KOTLIN_GOVERNANCE_VALIDATOR_TEST}: native validation must fail "
                f"without the capability-skip marker `{marker}`"
            )

    java = sources[JAVA_GOVERNANCE_VALIDATOR_TEST]
    java_section = _contract_section(
        java,
        (
            "private static void "
            "validatesGovernanceDagFixturesAndNegativeVectorsWhenNativeBridgeIsAvailable"
        ),
        "private static void signsOrderbookFixtureWhenNativeBridgeIsAvailable",
    )
    if java_section is None:
        errors.append(
            f"{JAVA_GOVERNANCE_VALIDATOR_TEST}: malformed Governance DAG golden test"
        )
    elif re.search(
        r"if\s*\(\s*!SorafsReferenceValidators\.isNativeAvailable\(\)\s*\)"
        r"\s*\{\s*return;\s*\}",
        java_section,
    ):
        errors.append(
            f"{JAVA_GOVERNANCE_VALIDATOR_TEST}: Governance DAG golden test "
            "must not unconditionally return when the native bridge is unavailable"
        )
    if NATIVE_GOVERNANCE_VALIDATION_REQUIRED_ENV in java:
        errors.append(
            f"{JAVA_GOVERNANCE_VALIDATOR_TEST}: native validation must fail "
            "without an environment-controlled capability return"
        )
    return errors


def _rust_struct_field_inventory(
    source: str, struct_name: str
) -> tuple[tuple[str, str], ...] | None:
    """Return the ordered fields from one simple Rust wire struct."""

    declaration = re.search(
        rf"(?ms)^\s*(?:pub\(super\)\s+)?struct\s+{re.escape(struct_name)}\s*"
        r"\{(?P<body>.*?)^\s*\}",
        source,
    )
    if declaration is None:
        return None
    return tuple(
        (field, re.sub(r"\s+", "", field_type))
        for field, field_type in re.findall(
            (
                r"(?m)^\s*(?:pub(?:\(super\))?\s+)?"
                r"([A-Za-z_][A-Za-z0-9_]*)\s*:\s*([^,\n]+),"
            ),
            declaration.group("body"),
        )
    )


def _validate_pop_broker_hard_cut_contract(root: Path) -> list[str]:
    """Pin the secret-free PoP broker open protocol and retired operation 60."""

    relative_sources = {
        "broker": "crates/irohad/src/runtime_provider_broker.rs",
        "protocol": (
            "crates/irohad/src/runtime_provider_broker/protocol_primitives.rs"
        ),
        "validator": (
            "crates/irohad/src/runtime_provider_broker/validate_operation_payload.rs"
        ),
        "recipient_client": (
            "crates/irohad/src/runtime_provider_broker/pop_recipient_client.rs"
        ),
        "runtime": "crates/irohad/src/sorafs_pop_runtime.rs",
    }
    sources: dict[str, str] = {}
    for label, relative in relative_sources.items():
        path = _require_regular_repo_file(root, relative)
        try:
            sources[label] = _read_bytes_no_follow(path).decode("utf-8")
        except UnicodeDecodeError as error:
            raise ValueError(f"{relative}: PoP broker source must be UTF-8") from error

    errors: list[str] = []
    protocol = sources["protocol"]
    for operation, wire_id in POP_BROKER_OPERATION_IDS.items():
        if re.search(
            rf"(?m)^\s*pub\(super\)\s+const\s+{re.escape(operation)}\s*:\s*"
            rf"u16\s*=\s*{wire_id}\s*;\s*$",
            protocol,
        ) is None:
            errors.append(
                f"PoP broker operation {operation} must retain wire id {wire_id}"
            )
    if "OPERATION_POP_RUNTIME_RESOLVE_V1" in "\n".join(sources.values()) or re.search(
        r"(?m)^\s*pub\(super\)\s+const\s+[A-Z0-9_]+\s*:\s*u16\s*=\s*60\s*;",
        protocol,
    ):
        errors.append("PoP broker operation 60/runtime resolve must remain retired")

    for struct_name, expected_fields in POP_BROKER_WIRE_FIELD_INVENTORIES.items():
        observed_fields = _rust_struct_field_inventory(protocol, struct_name)
        if observed_fields != expected_fields:
            errors.append(
                f"PoP broker wire struct {struct_name} fields must be exactly "
                f"{expected_fields!r}"
            )

    ipc_sources = (
        sources["broker"],
        sources["protocol"],
        sources["validator"],
        sources["recipient_client"],
    )
    if any("HybridSecretKey" in source for source in ipc_sources):
        errors.append("PoP broker IPC must never serialize HybridSecretKey")
    if any(
        marker in source
        for source in ipc_sources
        for marker in POP_BROKER_RETIRED_SECRET_MARKERS
    ):
        errors.append("PoP broker IPC must not serialize private recipient bytes")
    if any(
        marker in sources["recipient_client"]
        for marker in (".secret()", "recipient_private_key", "recipient_secret")
    ):
        errors.append("PoP broker client must not own private recipient bytes")

    production_runtime = sources["runtime"].split("#[cfg(test)]", 1)[0]
    if any(
        marker in production_runtime
        for marker in (
            "HybridSecretKey",
            "PopCredentialRuntimeSecretsV1",
            "PrivateKey",
            ".secret()",
        )
    ):
        errors.append("production PoP runtime must not own private recipient material")
    return errors


def _validate_runtime_provider_deployment_contract(root: Path) -> list[str]:
    """Validate the complete credential-free broker deployment asset inventory."""

    errors: list[str] = []
    for relative, markers in sorted(
        RUNTIME_PROVIDER_DEPLOYMENT_ASSET_MARKERS.items()
    ):
        path = _require_regular_repo_file(root, relative)
        try:
            source = _read_bytes_no_follow(path).decode("utf-8")
        except UnicodeDecodeError as error:
            raise ValueError(
                f"{relative}: runtime-provider deployment asset must be UTF-8"
            ) from error
        for marker in markers:
            if marker not in source:
                errors.append(
                    f"{relative}: missing runtime-provider deployment "
                    f"contract marker {marker!r}"
                )
        for marker in RUNTIME_PROVIDER_DEPLOYMENT_FORBIDDEN_MARKERS.get(
            relative, ()
        ):
            if marker in source:
                errors.append(
                    f"{relative}: forbidden runtime-provider deployment "
                    f"marker {marker!r}"
                )
    errors.extend(_validate_pop_broker_hard_cut_contract(root))
    return errors


def _validate_workflow_source(relative: str, source: str) -> list[str]:
    """Return deterministic contract errors for one workflow source."""

    errors: list[str] = []
    if "pull_request_target:" in source:
        errors.append(f"{relative}: pull_request_target is forbidden")
    if re.search(r"\b(?:curl|wget)\b", source):
        errors.append(f"{relative}: network bootstrap commands are forbidden")
    action_refs = re.findall(r"uses:\s*[^\s@]+@([^\s#]+)", source)
    if any(re.fullmatch(r"[0-9a-f]{40}", reference) is None for reference in action_refs):
        errors.append(
            f"{relative}: floating action references are forbidden; pin full commits"
        )
    if "cancel-in-progress: false" not in source:
        errors.append(f"{relative}: release evidence jobs must not be cancelled")
    for marker in WORKFLOWS[relative]:
        if marker not in source:
            errors.append(f"{relative}: missing contract marker `{marker}`")

    if relative.endswith("sorafs-orchestrator-sdk.yml"):
        jobs_source = source[source.index("jobs:\n") + len("jobs:\n") :]
        job_inventory = tuple(
            re.findall(r"(?m)^  ([A-Za-z0-9_-]+):\n", jobs_source)
        )
        expected_job_inventory = (
            "sdk-parity",
            "mobile-parity",
            "csharp-parity",
        )
        if job_inventory != expected_job_inventory:
            errors.append(
                f"{relative}: parity workflow job inventory must be exactly "
                f"{expected_job_inventory}"
            )
        required_job_markers = {
            "sdk-parity": (
                "runs-on: macos-14",
                'IROHA_REQUIRE_SORAFS_NATIVE_VALIDATION: "1"',
                "bash ci/check_sorafs_python_native_sdk.sh",
                "bash ci/sdk_sorafs_orchestrator.sh",
            ),
            "mobile-parity": (
                "runs-on: ubuntu-latest",
                'IROHA_REQUIRE_SORAFS_NATIVE_VALIDATION: "1"',
                "actions/setup-java@c1e323688fd81a25caa38c78aa6df2d33d3e20d9",
                "cargo fetch --locked",
                'NORITO_MOBILE_JAVA_HOME="$JAVA_HOME"',
                'NORITO_MOBILE_ANDROID_HOME="$ANDROID_HOME"',
                "bash ci/check_kagemusha_jvm_native_bridge.sh",
            ),
            "csharp-parity": (
                "runs-on: ubuntu-24.04",
                'IROHA_REQUIRE_SORAFS_NATIVE_VALIDATION: "1"',
                "actions/setup-dotnet@67a3573c9a986a3f9c594539f4ab511d57bb3ce9",
                "cargo build --locked --release -p connect_norito_bridge",
                "check_native_sdk_abi21_artifact.py record",
                "check_native_sdk_abi21_artifact.py verify",
                "dotnet build Hyperledger.Iroha.Sdk.sln -c Release --no-restore -warnaserror",
                "dotnet test Hyperledger.Iroha.Sdk.sln -c Release --no-build",
            ),
        }
        for job_name, markers in required_job_markers.items():
            job = _workflow_job(source, job_name)
            if job is None:
                errors.append(f"{relative}: missing `{job_name}` job")
                continue
            for marker in markers:
                if marker not in job:
                    errors.append(
                        f"{relative}: `{job_name}` missing contract marker `{marker}`"
                    )

    if relative.endswith("sorafs-cli-release.yml"):
        jobs_source = source[source.index("jobs:\n") + len("jobs:\n") :]
        job_inventory = tuple(
            re.findall(r"(?m)^  ([A-Za-z0-9_-]+):\n", jobs_source)
        )
        expected_job_inventory = (
            "release-gate",
            "package",
            "prepare-release-manifest",
            "verify-release-auth",
            "sign",
            "reference-sdk-supply-chain-evidence",
        )
        if job_inventory != expected_job_inventory:
            errors.append(
                f"{relative}: release workflow job inventory must be exactly "
                f"{expected_job_inventory}"
            )
        if "${{ secrets." in source:
            errors.append(
                f"{relative}: release workflow must not consume GitHub secrets"
            )
        lowered_source = source.lower()
        if any(
            marker in lowered_source
            for marker in (
                "release_manifest_signing.py sign",
                "--external-signer",
                "--signing-seed",
                "signing_seed",
                "private-key",
                "private_key",
                "development-local-signing",
            )
        ):
            errors.append(
                f"{relative}: GitHub release jobs must not receive private signing "
                "material or invoke the foundational Ed25519 signer"
            )
        if source.count("merge-multiple: false") != 3:
            errors.append(
                f"{relative}: candidate downloads must preserve exactly five "
                "artifact-name directories"
            )
        release_gate_job = _workflow_job(source, "release-gate")
        prepare_job = _workflow_job(source, "prepare-release-manifest")
        auth_job = _workflow_job(source, "verify-release-auth")
        promotion_job = _workflow_job(source, "sign")
        supply_chain_job = _workflow_job(
            source,
            "reference-sdk-supply-chain-evidence",
        )
        promotion_guard = (
            "if: ${{ startsWith(github.ref, 'refs/tags/sorafs-cli-v') "
            "|| inputs.sign_artifacts }}"
        )
        if release_gate_job is None:
            errors.append(f"{relative}: missing release gate job")
        elif (
            release_gate_job.count(
                "python3 scripts/check_sorafs_release_version_map.py"
            )
            != 2
            or "cmp version-map-summary.first.json version-map-summary.replay.json"
            not in release_gate_job
            or "cp version-map-summary.first.json version-map-summary.json"
            not in release_gate_job
        ):
            errors.append(
                f"{relative}: version map must be validated exactly twice with "
                "byte-identical summaries before its release version is consumed"
            )
        if prepare_job is None:
            errors.append(f"{relative}: missing foundational-manifest preparation job")
        else:
            if promotion_guard not in prepare_job:
                errors.append(
                    f"{relative}: foundational-manifest preparation lacks the "
                    "reviewed promotion trigger guard"
                )
            if "needs: [release-gate, package]" not in prepare_job:
                errors.append(
                    f"{relative}: foundational manifest must depend on the release "
                    "gate and all platform packages"
                )
        if auth_job is None:
            errors.append(f"{relative}: missing protected release-authentication job")
        else:
            required_auth_bindings = (
                "SORAFS_RELEASE_SIGNATURE_PATH: ${{ vars.SORAFS_RELEASE_SIGNATURE_PATH }}",
                "SORAFS_RELEASE_PUBLIC_KEY_PATH: ${{ vars.SORAFS_RELEASE_PUBLIC_KEY_PATH }}",
                "SORAFS_RELEASE_MANIFEST_VERIFIER_PATH: ${{ vars.SORAFS_RELEASE_MANIFEST_VERIFIER_PATH }}",
                "SORAFS_TRUSTED_RELEASE_SIGNING_FINGERPRINT: ${{ vars.SORAFS_TRUSTED_RELEASE_SIGNING_FINGERPRINT }}",
                "SORAFS_TRUSTED_RELEASE_MANIFEST_VERIFIER_SHA256: ${{ vars.SORAFS_TRUSTED_RELEASE_MANIFEST_VERIFIER_SHA256 }}",
            )
            if any(binding not in auth_job for binding in required_auth_bindings):
                errors.append(
                    f"{relative}: release authentication is missing an explicit "
                    "protected public tuple or trust-anchor binding"
                )
            if "needs: [release-gate, prepare-release-manifest]" not in auth_job:
                errors.append(
                    f"{relative}: release authentication must consume the gated "
                    "foundational manifest"
                )
            if promotion_guard not in auth_job:
                errors.append(
                    f"{relative}: release authentication lacks the reviewed "
                    "promotion trigger guard"
                )
            if "environment: sorafs-release-authentication" not in auth_job:
                errors.append(
                    f"{relative}: release authentication must use the protected "
                    "release-authentication environment"
                )
            if (
                "runs-on: [self-hosted, linux, x64, sorafs-release-auth]"
                not in auth_job
            ):
                errors.append(
                    f"{relative}: release authentication must use the protected "
                    "self-hosted release-auth runner"
                )
            if "${{ secrets." in auth_job:
                errors.append(
                    f"{relative}: release authentication must not receive GitHub secrets"
                )
            if any(
                marker in auth_job
                for marker in (
                    "id-token:",
                    "attestations:",
                    "artifact-metadata:",
                    "actions/attest@",
                    "cosign",
                )
            ):
                errors.append(
                    f"{relative}: OIDC and provenance authority must not enter the "
                    "release-authentication job"
                )
            lowered_auth = auth_job.lower()
            forbidden_auth_markers = (
                "release_manifest_signing.py sign",
                "--external-signer",
                "--signing-seed",
                "signing_seed",
                "private-key",
                "private_key",
                "development-local-signing",
            )
            if any(marker in lowered_auth for marker in forbidden_auth_markers):
                errors.append(
                    f"{relative}: release authentication must be verification-only "
                    "and must not receive private signing material"
                )
            if (
                auth_job.count(
                    "python3 scripts/release_manifest_signing.py verify"
                )
                != 2
            ):
                errors.append(
                    f"{relative}: release authentication must verify both the "
                    "protected tuple and its staged public snapshot"
                )
            if (
                auth_job.count(
                    "python3 scripts/generate_sorafs_cli_release_manifest.py check"
                )
                != 1
            ):
                errors.append(
                    f"{relative}: release authentication must reconcile the signed "
                    "manifest with the downloaded candidates exactly once"
                )
            try:
                reconcile = auth_job.index(
                    "name: Reconcile the foundational manifest with the immutable candidates"
                )
                external_verify = auth_job.index(
                    "name: Verify the protected external Ed25519 manifest tuple"
                )
                first_native_verify = auth_job.index(
                    "python3 scripts/release_manifest_signing.py verify"
                )
                stage_signature = auth_job.index(
                    '"$evidence_dir/release_manifest.json.sig"'
                )
                second_native_verify = auth_job.index(
                    "python3 scripts/release_manifest_signing.py verify",
                    first_native_verify + 1,
                )
                upload_auth = auth_job.index(
                    "name: Upload authenticated foundational manifest tuple"
                )
            except ValueError:
                pass
            else:
                if not (
                    reconcile
                    < external_verify
                    <= first_native_verify
                    < stage_signature
                    < second_native_verify
                    < upload_auth
                ):
                    errors.append(
                        f"{relative}: candidate reconciliation, external "
                        "verification, public snapshot, replay verification, and "
                        "authentication upload are out of order"
                    )
        if promotion_job is None:
            errors.append(f"{relative}: missing release promotion job")
        else:
            if promotion_guard not in promotion_job:
                errors.append(
                    f"{relative}: release promotion lacks the reviewed trigger guard"
                )
            if (
                "needs: [release-gate, package, verify-release-auth]"
                not in promotion_job
            ):
                errors.append(
                    f"{relative}: release promotion must depend on protected "
                    "Ed25519 manifest authentication"
                )
            if (
                promotion_job.count(
                    "python3 scripts/generate_sorafs_cli_release_manifest.py check"
                )
                != 1
            ):
                errors.append(
                    f"{relative}: release promotion must reconcile the downloaded "
                    "authenticated manifest with the exact candidate inventory"
                )
        if supply_chain_job is None:
            errors.append(
                f"{relative}: missing reference-SDK supply-chain evidence job"
            )
        else:
            required_bindings = (
                "SORAFS_REFERENCE_SDK_DEPLOYMENT_ID: "
                "${{ vars.SORAFS_REFERENCE_SDK_DEPLOYMENT_ID }}",
                "SORAFS_REFERENCE_SDK_RECEIPTS_ROOT: "
                "${{ vars.SORAFS_REFERENCE_SDK_RECEIPTS_ROOT }}",
                "SORAFS_PROVENANCE_VERIFICATION_PUBLIC_KEY_HEX: "
                "${{ vars.SORAFS_PROVENANCE_VERIFICATION_PUBLIC_KEY_HEX }}",
                "SORAFS_TRUSTED_GH_CLI_SHA256: "
                "${{ vars.SORAFS_TRUSTED_GH_CLI_SHA256 }}",
                "SORAFS_L1_TOPOLOGY_QUALIFICATION_SUMMARY_PATH: "
                "${{ vars.SORAFS_L1_TOPOLOGY_QUALIFICATION_SUMMARY_PATH }}",
                "SORAFS_L1_TOPOLOGY_QUALIFICATION_ENVELOPE_PATH: "
                "${{ vars.SORAFS_L1_TOPOLOGY_QUALIFICATION_ENVELOPE_PATH }}",
                "SORAFS_L1_TOPOLOGY_QUALIFICATION_VERIFICATION_PUBLIC_KEY_HEX: "
                "${{ vars.SORAFS_L1_TOPOLOGY_QUALIFICATION_VERIFICATION_PUBLIC_KEY_HEX }}",
                "SORAFS_L1_TOPOLOGY_QUALIFICATION_SIGNER_IDENTITY: "
                "${{ vars.SORAFS_L1_TOPOLOGY_QUALIFICATION_SIGNER_IDENTITY }}",
                "SORAFS_L1_TOPOLOGY_QUALIFICATION_SIGNER_KEY_REVISION: "
                "${{ vars.SORAFS_L1_TOPOLOGY_QUALIFICATION_SIGNER_KEY_REVISION }}",
                "SORAFS_L1_TOPOLOGY_QUALIFICATION_SIGNER_POLICY_DIGEST_HEX: "
                "${{ vars.SORAFS_L1_TOPOLOGY_QUALIFICATION_SIGNER_POLICY_DIGEST_HEX }}",
            )
            if promotion_guard not in supply_chain_job:
                errors.append(
                    f"{relative}: reference-SDK source evidence lacks the "
                    "reviewed promotion trigger guard"
                )
            if "needs: [release-gate, sign]" not in supply_chain_job:
                errors.append(
                    f"{relative}: reference-SDK source evidence must consume "
                    "the authenticated signed release"
                )
            if "environment: sorafs-reference-sdk-evidence" not in supply_chain_job:
                errors.append(
                    f"{relative}: reference-SDK source evidence must use its "
                    "protected environment"
                )
            if (
                "runs-on: [self-hosted, linux, x64, sorafs-release-auth]"
                not in supply_chain_job
            ):
                errors.append(
                    f"{relative}: reference-SDK source evidence must use the "
                    "protected self-hosted release runner"
                )
            if any(binding not in supply_chain_job for binding in required_bindings):
                errors.append(
                    f"{relative}: reference-SDK source evidence is missing an "
                    "external receipt, topology, or public-key binding"
                )
            if "${{ secrets." in supply_chain_job:
                errors.append(
                    f"{relative}: reference-SDK source evidence must not "
                    "receive GitHub secrets"
                )
            if any(
                marker in supply_chain_job
                for marker in (
                    "id-token:",
                    "attestations:",
                    "artifact-metadata:",
                    "actions/attest@",
                    "cosign sign-blob",
                )
            ):
                errors.append(
                    f"{relative}: reference-SDK source evidence must remain "
                    "verification-only"
                )
            if any(
                marker in supply_chain_job.lower()
                for marker in (
                    "private-key",
                    "private_key",
                    "--external-signer",
                    "signing-seed",
                    "signing_seed",
                )
            ):
                errors.append(
                    f"{relative}: reference-SDK source evidence must remain "
                    "verification-only"
                )
            required_source_flags = (
                "--supply-chain-source-root sf11-source",
                "--provenance-certificate-identity \"$certificate_identity\"",
                "--provenance-oidc-issuer \"$oidc_issuer\"",
                "$SORAFS_PROVENANCE_VERIFICATION_PUBLIC_KEY_HEX",
            )
            if any(
                marker not in supply_chain_job for marker in required_source_flags
            ):
                errors.append(
                    f"{relative}: reference-SDK source evidence must pass the "
                    "source root and exact public provenance trust tuple"
                )
            repeated_source_flags = (
                '--provenance-certificate-identity "$certificate_identity"',
                '--provenance-oidc-issuer "$oidc_issuer"',
            )
            if any(
                supply_chain_job.count(marker) != 2
                for marker in repeated_source_flags
            ):
                errors.append(
                    f"{relative}: reference-SDK source evidence must pass the "
                    "source root and exact public provenance trust tuple"
                )
            expected_commands = (
                "python3 scripts/build_sorafs_reference_sdk_supply_chain_sources.py",
                "python3 scripts/build_sorafs_reference_sdk_release_canary.py",
                "python3 scripts/check_sorafs_reference_sdk_release_evidence.py",
                'gh attestation verify "$provenance_file"',
                "cosign verify-blob",
                "sha256sum --check SHA256SUMS",
            )
            if any(
                supply_chain_job.count(command) != 1
                for command in expected_commands
            ):
                errors.append(
                    f"{relative}: reference-SDK source evidence must assemble, "
                    "build, verify, and provenance-check exactly once"
                )
            required_source_targets = re.search(
                r"(?ms)^\s+required_targets=\(\n"
                r"(?P<targets>.*?)^\s+\)$",
                supply_chain_job,
            )
            if required_source_targets is None:
                errors.append(
                    f"{relative}: reference-SDK source evidence lacks the "
                    "canonical five-target inventory"
                )
            else:
                source_targets = tuple(
                    line.strip()
                    for line in required_source_targets.group("targets").splitlines()
                    if line.strip()
                )
                if source_targets != SUPPLY_CHAIN_SOURCE_TARGETS:
                    errors.append(
                        f"{relative}: reference-SDK source evidence target "
                        "inventory must match the canonical source order"
                    )
            required_provenance_files = re.search(
                r"(?ms)^\s+provenance_files=\(\n"
                r"(?P<files>.*?)^\s+\)$",
                supply_chain_job,
            )
            expected_provenance_files = (
                '"$archive"',
                '"${candidate}/SHA256SUMS"',
                '"${candidate}/sorafs-release.spdx.json"',
                '"${candidate}/sorafs-release-vulnerabilities.sarif"',
                '"${candidate}/sorafs-cli-${target}.spdx.json"',
                '"${candidate}/sorafs-cli-${target}-vulnerabilities.sarif"',
            )
            if required_provenance_files is None:
                errors.append(
                    f"{relative}: reference-SDK source evidence lacks the "
                    "exact six-file provenance inventory"
                )
            else:
                provenance_files = tuple(
                    line.strip()
                    for line in required_provenance_files.group("files").splitlines()
                    if line.strip()
                )
                if provenance_files != expected_provenance_files:
                    errors.append(
                        f"{relative}: reference-SDK source evidence provenance "
                        "inventory must cover the exact checksum, archive, and "
                        "scan files"
                    )
            for marker in (
                "! -name '*.sigstore.json'",
                "signed candidate SHA256SUMS contains duplicate entries",
                "signed candidate SHA256SUMS does not cover the exact candidate file set",
            ):
                if supply_chain_job.count(marker) != 1:
                    errors.append(
                        f"{relative}: reference-SDK source evidence must "
                        "reconcile the signed checksum manifest with the exact "
                        "candidate inventory"
                    )
            if (
                'actual_gh_sha256="$(sha256sum -- "$gh_path" | cut -d \' \' -f1)"'
                not in supply_chain_job
                or '[[ "$actual_gh_sha256" != "$SORAFS_TRUSTED_GH_CLI_SHA256" ]]'
                not in supply_chain_job
            ):
                errors.append(
                    f"{relative}: reference-SDK source evidence must pin the "
                    "GitHub attestation verifier by protected SHA-256"
                )
            if (
                'workspace_real="$(realpath -e -- "$GITHUB_WORKSPACE")"'
                not in supply_chain_job
                or 'path_real="$(realpath -e -- "$path")"'
                not in supply_chain_job
                or '[[ "$path" != "$path_real" ]]'
                not in supply_chain_job
            ):
                errors.append(
                    f"{relative}: reference-SDK external evidence paths must "
                    "be canonicalized before workspace exclusion"
                )
            if (
                "name: Upload replay-complete SF-11 supply-chain evidence"
                not in supply_chain_job
                or re.search(
                    r"(?m)^\s{12}sf11-source/$",
                    supply_chain_job,
                )
                is None
            ):
                errors.append(
                    f"{relative}: reference-SDK evidence upload must retain "
                    "the complete replay source tree"
                )
            for marker in (
                "SIGNED_INPUT_ARTIFACT_ID: ${{ needs.sign.outputs.signed_artifact_id }}",
                "SIGNED_INPUT_ARTIFACT_DIGEST: ${{ needs.sign.outputs.signed_artifact_digest }}",
                "SIGNED_INPUT_ARTIFACT_URL: ${{ needs.sign.outputs.signed_artifact_url }}",
                '"schema": "sorafs.reference_sdk.signed_input_artifact.v1"',
            ):
                if supply_chain_job.count(marker) != 1:
                    errors.append(
                        f"{relative}: reference-SDK evidence must retain the "
                        "immutable signed-input artifact identity and digest"
                    )
            topology_archive = (
                "artifacts/reference-sdk-evidence/"
                "l1-topology-qualification.summary.json"
            )
            if supply_chain_job.count(topology_archive) != 2:
                errors.append(
                    f"{relative}: reference-SDK evidence must gate and archive "
                    "the exact topology qualification summary"
                )
            topology_envelope_archive = (
                "artifacts/reference-sdk-evidence/"
                "l1-topology-qualification.envelope.json"
            )
            signed_topology_markers = (
                topology_envelope_archive,
                "sorafs.l1.deployment_qualification.trust.v1",
                "--topology-qualification-envelope",
                "--topology-qualification-verification-public-key-hex",
                "--topology-qualification-signer-identity",
                "--topology-qualification-signer-key-revision",
                "--topology-qualification-signer-policy-digest-hex",
                "--max-topology-qualification-review-age-secs 1209600",
            )
            if (
                supply_chain_job.count(topology_envelope_archive) != 2
                or any(
                    supply_chain_job.count(marker) != 1
                    for marker in signed_topology_markers[1:]
                )
            ):
                errors.append(
                    f"{relative}: reference-SDK evidence must authenticate and "
                    "archive the independently signed topology envelope"
                )
            if (
                '[[ "$SORAFS_L1_TOPOLOGY_QUALIFICATION_VERIFICATION_PUBLIC_KEY_HEX" '
                '== "$SORAFS_PROVENANCE_VERIFICATION_PUBLIC_KEY_HEX" ]]'
                not in supply_chain_job
            ):
                errors.append(
                    f"{relative}: topology and provenance evidence must use "
                    "independently administered verification keys"
                )
            try:
                download_signed = supply_chain_job.index(
                    "name: Download the authenticated signed release input"
                )
                record_signed = supply_chain_job.index(
                    "name: Record the immutable signed-input artifact binding"
                )
                require_external = supply_chain_job.index(
                    "name: Require protected external source-evidence inputs"
                )
                verify_provenance = supply_chain_job.index(
                    "name: Reverify exact target provenance before source assembly"
                )
                assemble_sources = supply_chain_job.index(
                    "name: Assemble and validate the canonical SF-11 source indexes"
                )
                build_canary = supply_chain_job.index(
                    "name: Build and gate source-derived SF-11 supply-chain evidence"
                )
                upload_evidence = supply_chain_job.index(
                    "name: Upload replay-complete SF-11 supply-chain evidence"
                )
            except ValueError:
                pass
            else:
                if not (
                    download_signed
                    < record_signed
                    < require_external
                    < verify_provenance
                    < assemble_sources
                    < build_canary
                    < upload_evidence
                ):
                    errors.append(
                        f"{relative}: signed input, external trust, provenance "
                        "verification, source assembly, gate, and upload are out "
                        "of order"
                    )

        package_matrix = re.search(
            r"(?ms)^  package:\n.*?^      matrix:\n"
            r"(?P<matrix>.*?)^    runs-on: \$\{\{ matrix\.os \}\}",
            source,
        )
        if package_matrix is None:
            errors.append(f"{relative}: malformed native release matrix")
        else:
            target_runners = tuple(
                re.findall(
                    r"(?m)^          - os: ([^\s#]+)\n"
                    r"            target: ([^\s#]+)$",
                    package_matrix.group("matrix"),
                )
            )
            if target_runners != RELEASE_TARGET_RUNNERS:
                errors.append(
                    f"{relative}: native release matrix must contain exactly the "
                    "reviewed Linux, macOS, and Windows target/runner pairs"
                )
        try:
            global_permissions = source[
                source.index("permissions:") : source.index("concurrency:")
            ]
            sign_job = source[source.index("  sign:") :]
        except ValueError:
            errors.append(f"{relative}: malformed permissions or signing job")
        else:
            global_elevated = [
                permission
                for permission in ("id-token:", "attestations:", "artifact-metadata:")
                if permission in global_permissions
            ]
            if global_elevated:
                errors.append(
                    f"{relative}: OIDC and attestation permissions must be signing-job scoped"
                )
            if "permissions:" not in sign_job or "id-token: write" not in sign_job:
                errors.append(f"{relative}: signing job must request OIDC explicitly")
            for permission in ("attestations: write", "artifact-metadata: write"):
                if permission not in sign_job:
                    errors.append(
                        f"{relative}: signing job must request `{permission}` explicitly"
                    )
            if "if: ${{ startsWith(github.ref, 'refs/tags/sorafs-cli-v') || inputs.sign_artifacts }}" not in sign_job:
                errors.append(f"{relative}: signing job lacks the reviewed trigger guard")
            for output in (
                "signed_artifact_id: ${{ steps.upload-signed.outputs.artifact-id }}",
                "signed_artifact_digest: ${{ steps.upload-signed.outputs.artifact-digest }}",
                "signed_artifact_url: ${{ steps.upload-signed.outputs.artifact-url }}",
                "id: upload-signed",
            ):
                if sign_job.count(output) != 1:
                    errors.append(
                        f"{relative}: signing job must expose the immutable "
                        "signed-artifact identity, digest, and URL"
                    )
            required_targets = re.search(
                r"(?ms)^\s+required_targets=\(\n"
                r"(?P<targets>.*?)^\s+\)$",
                sign_job,
            )
            expected_targets = tuple(
                target for _runner, target in RELEASE_TARGET_RUNNERS
            )
            if required_targets is None:
                errors.append(
                    f"{relative}: signing job lacks the mandatory target inventory"
                )
            else:
                signed_targets = tuple(
                    line.strip()
                    for line in required_targets.group("targets").splitlines()
                    if line.strip()
                )
                if signed_targets != expected_targets:
                    errors.append(
                        f"{relative}: signing job target inventory must exactly "
                        "match the native release matrix"
                    )
            try:
                verify_manifest_binding = sign_job.index(
                    "name: Verify authenticated release-manifest candidate binding before provenance"
                )
                verify_checksums = sign_job.index(
                    "name: Verify platform package checksums before signing"
                )
                attest = sign_job.index(
                    "name: Attest aggregate signed-input provenance"
                )
                stage_attestations = sign_job.index("name: Stage offline provenance bundles")
                sign_blobs = sign_job.index("name: Keyless-sign every release-candidate file")
                upload_signed = sign_job.index("name: Upload signed release candidate")
            except ValueError:
                pass
            else:
                if not (
                    verify_manifest_binding
                    < verify_checksums
                    < attest
                    < stage_attestations
                    < sign_blobs
                    < upload_signed
                ):
                    errors.append(
                        f"{relative}: checksum verification, provenance, signing, and upload are out of order"
                    )
        sbom_action = "anchore/sbom-action@e22c389904149dbc22b58101806040fa8d37a610"
        scan_action = "anchore/scan-action@e1165082ffb1fe366ebaf02d8526e7c4989ea9d2"
        if source.count(sbom_action) != 2 or source.count(scan_action) != 2:
            errors.append(
                f"{relative}: source and platform packages require exactly one SBOM and vulnerability scan each"
            )
        if source.count("grype-version: v0.112.0") != 2:
            errors.append(
                f"{relative}: source and platform scans must both pin Grype v0.112.0"
            )
        try:
            package_reference = source.index("name: Package reference validator and FFI header")
            reproducible_archive = source.index(
                "name: Rebuild deterministic platform archive and run clean-consumer smoke"
            )
            stage_source_scan = source.index("name: Stage source release scan evidence")
            binary_sbom = source.index("name: Generate platform binary SBOM")
            binary_scan = source.index("name: Scan platform binary SBOM")
            checksums = source.index("name: Finalize platform checksums")
            upload = source.index("name: Upload unsigned release candidate")
        except ValueError:
            pass
        else:
            if not (
                package_reference
                < reproducible_archive
                < stage_source_scan
                < binary_sbom
                < binary_scan
                < checksums
                < upload
            ):
                errors.append(
                    f"{relative}: reference packaging, reproducible archive, "
                    "source evidence, platform SBOM, scan, checksum, and upload "
                    "steps are out of order"
                )
        if source.count("python3 scripts/package_sorafs_cli_candidate.py") != 2:
            errors.append(
                f"{relative}: deterministic platform candidate must be built "
                "exactly twice for byte-identical replay"
            )
        if source.count("bash scripts/package_sorafs_validate_release.sh") != 2:
            errors.append(
                f"{relative}: deterministic reference-validator package must be "
                "built exactly twice for byte-identical replay"
            )
        if source.count('--source-commit "$source_commit"') != 2:
            errors.append(
                f"{relative}: both reference-validator package replays must bind "
                "the reviewed source commit"
            )
        if source.count('--source-date-epoch "$source_date_epoch"') != 2:
            errors.append(
                f"{relative}: both reference-validator package replays must bind "
                "the canonical source epoch"
            )
        if source.count('cmp \\\n            "${first_out}/${package_name}') != 2:
            errors.append(
                f"{relative}: platform archive and manifest replay comparisons "
                "must both be enforced"
            )
    return errors


def validate_release_automation(root: Path) -> dict[str, Any]:
    """Validate every committed SoraFS workflow and return a closed summary."""

    errors: list[str] = []
    validated: list[str] = []
    for relative in sorted(WORKFLOWS):
        path = _require_regular_repo_file(root, relative)
        try:
            source = _read_bytes_no_follow(path).decode("utf-8")
        except UnicodeDecodeError as error:
            raise ValueError(f"{relative}: workflow must be UTF-8") from error
        errors.extend(_validate_workflow_source(relative, source))
        validated.append(relative)
    for relative, markers in sorted(RELEASE_DOCUMENTS.items()):
        path = _require_regular_repo_file(root, relative)
        try:
            source = _read_bytes_no_follow(path).decode("utf-8")
        except UnicodeDecodeError as error:
            raise ValueError(f"{relative}: release document must be UTF-8") from error
        for marker in markers:
            if marker not in source:
                errors.append(
                    f"{relative}: missing release-document contract marker `{marker}`"
                )
        for stale_claim in FORBIDDEN_RELEASE_DOCUMENT_CLAIMS.get(relative, ()):
            if stale_claim in source:
                errors.append(
                    f"{relative}: stale release-document claim `{stale_claim}`"
                )
    errors.extend(_validate_release_auth_document_tree(root))
    errors.extend(_validate_package_release_smoke(root))
    errors.extend(_validate_reference_sdk_release_examples(root))
    errors.extend(_validate_native_governance_sdk_contract(root))
    errors.extend(_validate_runtime_provider_deployment_contract(root))
    if errors:
        raise ValueError("; ".join(errors))
    return {
        "schema": SCHEMA,
        "workflow_count": len(validated),
        "workflows": validated,
    }


def main() -> int:
    """Run the release-automation validator from the repository root."""

    try:
        summary = validate_release_automation(SCRIPT_DIR.parent)
    except (OSError, ValueError) as error:
        print(f"error: invalid SoraFS release automation: {error}", file=sys.stderr)
        return 1
    print(json.dumps(summary, sort_keys=True, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
