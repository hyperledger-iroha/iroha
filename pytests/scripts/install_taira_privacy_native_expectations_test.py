"""Adversarial tests for the one-shot native fixture and X.509 pin installer."""

from __future__ import annotations

import base64
from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path
import stat
import subprocess
import sys
from typing import Any, Callable

import pytest


ROOT = Path(__file__).resolve().parents[2]
INSTALLER = ROOT / "scripts" / "install_taira_privacy_native_expectations.py"
PROFILE_RELATIVE = Path("crates/iroha_core/src/privacy_engines/zk_x509/profile.rs")
READINESS_RELATIVE = Path(
    "crates/iroha_core/src/privacy_engines/zk_x509/profile/readiness_certificates.rs"
)
EXPECTATIONS_NORITO_RELATIVE = Path(
    "fixtures/privacy/native_release_expectations_v1.norito"
)
EXPECTATIONS_JSON_RELATIVE = Path(
    "fixtures/privacy/native_release_expectations_v1.json"
)
RESOURCE_NORITO_RELATIVE = Path("fixtures/privacy/zk_x509_native_resource_v1.norito")
RESOURCE_JSON_RELATIVE = Path("fixtures/privacy/zk_x509_native_resource_v1.json")
KAT_BYTES_PIN = "ZK_X509_RELEASE_KAT_EXPECTED_PROOF_BYTES_V1"
KAT_SHA_PIN = "ZK_X509_RELEASE_KAT_EXPECTED_PROOF_SHA256_V1"
EXPECTATIONS_NORITO_PIN = "ZK_X509_NATIVE_RELEASE_EXPECTATIONS_NORITO_SHA256_V1"
EXPECTATIONS_JSON_PIN = "ZK_X509_NATIVE_RELEASE_EXPECTATIONS_JSON_SHA256_V1"
RESOURCE_CERT_PIN = "ZK_X509_RESOURCE_CERTIFICATE_SHA256_V1"
OBSERVATION_PINS = (
    "ZK_X509_RESOURCE_POSITIVE_ELAPSED_MILLIS_V1",
    "ZK_X509_RESOURCE_POSITIVE_PEAK_RSS_BYTES_V1",
    "ZK_X509_RESOURCE_POSITIVE_PEAK_ADDRESS_SPACE_BYTES_V1",
    "ZK_X509_RESOURCE_MAXIMUM_ELAPSED_MILLIS_V1",
    "ZK_X509_RESOURCE_MAXIMUM_PEAK_RSS_BYTES_V1",
    "ZK_X509_RESOURCE_MAXIMUM_PEAK_ADDRESS_SPACE_BYTES_V1",
)
ALL_TARGETS = (
    EXPECTATIONS_NORITO_RELATIVE,
    EXPECTATIONS_JSON_RELATIVE,
    RESOURCE_NORITO_RELATIVE,
    RESOURCE_JSON_RELATIVE,
)
HASH_FRAME_DOMAIN = b"iroha.zk-x509.sha256.frame.v1"
RESOURCE_DOMAIN = b"iroha.zk-x509.native-resource-certificate.payload.v1"
AUTH_IROHA_COMMIT = "1" * 40
AUTH_IROHA_PRINCIPAL = "taira-privacy-iroha"
AUTH_IROHA_FINGERPRINT = (
    "SHA256:" + base64.b64encode(bytes([0x42]) * 32).decode().rstrip("=")
)
AUTH_IROHA_POLICY_SHA256 = "2" * 64
AUTH_VALIDATOR_COMMIT = "3" * 40
AUTH_VALIDATOR_PRINCIPAL = "taira-privacy-validator"
AUTH_VALIDATOR_FINGERPRINT = (
    "SHA256:" + base64.b64encode(bytes([0x43]) * 32).decode().rstrip("=")
)
AUTH_VALIDATOR_POLICY_SHA256 = "4" * 64
AUTH_VALIDATOR_SOURCE_TREE_SHA256 = "5" * 64
AUTH_BOOTSTRAP_SOURCE_TREE_SHA256 = "6" * 64
AUTH_CARGO_LOCK_SHA256 = hashlib.sha256(b"reviewed-lock-bytes\n").hexdigest()
AUTH_RUST_TOOLCHAIN_TREE_SHA256 = "7" * 64


@dataclass
class Fixture:
    repository: Path
    native_verifier: Path
    native_verifier_sha256: str
    exact12_matrix: Path
    expectations_norito: Path
    expectations_json: Path
    resource_norito: Path
    resource_json: Path
    manifest: Path


def _profile_source() -> str:
    return (
        "//! Fixture profile.\n"
        f"pub(crate) const {KAT_BYTES_PIN}: u32 = 0;\n"
        f"pub(crate) const {KAT_SHA_PIN}: [u8; 32] = [0; 32];\n"
        f"pub(crate) const {EXPECTATIONS_NORITO_PIN}: [u8; 32] = [0; 32];\n"
        f"pub(crate) const {EXPECTATIONS_JSON_PIN}: [u8; 32] = [0; 32];\n"
    )


def _readiness_source() -> str:
    return (
        "//! Fixture readiness pins.\n"
        + "".join(f"pub(crate) const {name}: u64 = 0;\n" for name in OBSERVATION_PINS)
        + f"pub(crate) const {RESOURCE_CERT_PIN}: [u8; 32] = [0; 32];\n"
    )


def _expectations_json() -> bytes:
    return (
        json.dumps(
            {
                "schema_version": 1,
                "stage_count": 48,
                "stages": [{"ordinal": ordinal} for ordinal in range(48)],
            },
            indent=2,
            sort_keys=True,
        )
        + "\n"
    ).encode()


def _field(hasher: Any, encoded: bytes) -> None:
    hasher.update(len(encoded).to_bytes(8, "big"))
    hasher.update(encoded)


def _certificate_digest(payload: dict[str, Any]) -> bytes:
    environment = payload["environment"]
    limits = payload["process_limits"]
    fields = [
        payload["schema_version"].to_bytes(2, "big"),
        bytes(payload["compiled_profile_digest"]),
        environment["operating_system"].encode(),
        environment["architecture"].encode(),
        environment["endianness"].encode(),
        environment["kernel_minimum_major"].to_bytes(2, "big"),
        environment["kernel_minimum_minor"].to_bytes(2, "big"),
        environment["rustc_release"].encode(),
        environment["rustc_host"].encode(),
        environment["rustc_commit_hash"].encode(),
        environment["rustc_commit_date"].encode(),
        environment["instance_type"].encode(),
        environment["cpu_model"].encode(),
        environment["logical_cpu_count"].to_bytes(2, "big"),
        environment["online_cpu_count"].to_bytes(2, "big"),
        environment["affinity_cpu_count"].to_bytes(2, "big"),
        bytes(payload["expectations_norito_sha256"]),
        bytes(payload["expectations_json_sha256"]),
        payload["kat_proof_bytes"].to_bytes(4, "big"),
        bytes(payload["kat_proof_sha256"]),
    ]
    for name in (
        "elapsed_ceiling_millis",
        "peak_rss_ceiling_bytes",
        "address_space_ceiling_bytes",
        "main_thread_stack_bytes",
        "rayon_worker_stack_bytes",
        "watchdog_thread_stack_bytes",
    ):
        fields.append(limits[name].to_bytes(8, "big"))
    for name in ("rayon_worker_count", "max_stage_tasks", "max_stage_open_files"):
        fields.append(limits[name].to_bytes(2, "big"))
    fields.extend(
        (
            limits["core_dump_bytes"].to_bytes(8, "big"),
            limits["landlock_abi_minimum"].to_bytes(2, "big"),
            limits["minimum_effective_memory_bytes"].to_bytes(8, "big"),
        )
    )
    for name in (
        "cgroup_v2",
        "cpu_quota_unlimited",
        "landlock_restrict_self",
        "anchored_openat2",
        "memfd_exec",
        "memfd_seal_exec",
        "static_elf_only",
        "seccomp_tsync",
    ):
        fields.append(bytes([limits[name]]))
    cases = {
        "positive-canonical-end-to-end": 0,
        "public-statement-binding-mutation": 1,
        "proof-corruption-and-truncation": 2,
        "maximum-shape-resource": 3,
    }
    for observation_name in ("positive", "maximum"):
        observation = payload[observation_name]
        fields.append(bytes([cases[observation["case_kind"]["case"]]]))
        for name in (
            "elapsed_millis",
            "peak_rss_bytes",
            "peak_address_space_bytes",
            "primary_units",
            "primary_ceiling",
            "secondary_units",
            "secondary_ceiling",
            "relation_depth",
            "relation_depth_ceiling",
        ):
            fields.append(observation[name].to_bytes(8, "big"))
    assert len(fields) == 60
    hasher = hashlib.sha256()
    hasher.update(HASH_FRAME_DOMAIN)
    hasher.update(len(RESOURCE_DOMAIN).to_bytes(2, "big"))
    hasher.update(RESOURCE_DOMAIN)
    hasher.update((60).to_bytes(2, "big"))
    for encoded in fields:
        _field(hasher, encoded)
    return hasher.digest()


def _resource_payload(
    expectations_norito: bytes, expectations_json: bytes
) -> dict[str, Any]:
    payload = {
        "schema_version": 1,
        "protocol_id": {
            "protocol": "iroha-zk-x509-stark-p256-v0",
            "value": None,
        },
        "compiled_profile_digest": [0x51] * 32,
        "environment": {
            "operating_system": "linux",
            "architecture": "aarch64",
            "endianness": "little",
            "kernel_minimum_major": 6,
            "kernel_minimum_minor": 3,
            "rustc_release": "1.93.1",
            "rustc_host": "aarch64-unknown-linux-gnu",
            "rustc_commit_hash": "01f6ddf7588f42ae2d7eb0a2f21d44e8e96674cf",
            "rustc_commit_date": "2026-02-11",
            "instance_type": "c7g.4xlarge",
            "cpu_model": "Neoverse-V1",
            "logical_cpu_count": 16,
            "online_cpu_count": 16,
            "affinity_cpu_count": 16,
        },
        "expectations_norito_sha256": list(
            hashlib.sha256(expectations_norito).digest()
        ),
        "expectations_json_sha256": list(hashlib.sha256(expectations_json).digest()),
        "kat_proof_bytes": 8_000_000,
        "kat_proof_sha256": [0x52] * 32,
        "process_limits": {
            "elapsed_ceiling_millis": 300_000,
            "peak_rss_ceiling_bytes": 12 * 1024 * 1024 * 1024,
            "address_space_ceiling_bytes": 32 * 1024 * 1024 * 1024,
            "main_thread_stack_bytes": 8 * 1024 * 1024,
            "rayon_worker_stack_bytes": 8 * 1024 * 1024,
            "watchdog_thread_stack_bytes": 8 * 1024 * 1024,
            "rayon_worker_count": 4,
            "max_stage_tasks": 6,
            "max_stage_open_files": 4,
            "core_dump_bytes": 0,
            "landlock_abi_minimum": 3,
            "minimum_effective_memory_bytes": 12 * 1024 * 1024 * 1024,
            "cgroup_v2": True,
            "cpu_quota_unlimited": True,
            "landlock_restrict_self": True,
            "anchored_openat2": True,
            "memfd_exec": True,
            "memfd_seal_exec": True,
            "static_elf_only": True,
            "seccomp_tsync": True,
        },
        "positive": {
            "case_kind": {
                "case": "positive-canonical-end-to-end",
                "value": None,
            },
            "elapsed_millis": 123,
            "peak_rss_bytes": 1024 * 1024,
            "peak_address_space_bytes": 2 * 1024 * 1024,
            "primary_units": 2,
            "primary_ceiling": 3,
            "secondary_units": 1,
            "secondary_ceiling": 4,
            "relation_depth": 0,
            "relation_depth_ceiling": 64,
        },
        "maximum": {
            "case_kind": {"case": "maximum-shape-resource", "value": None},
            "elapsed_millis": 456,
            "peak_rss_bytes": 2 * 1024 * 1024,
            "peak_address_space_bytes": 3 * 1024 * 1024,
            "primary_units": 3,
            "primary_ceiling": 3,
            "secondary_units": 4,
            "secondary_ceiling": 4,
            "relation_depth": 64,
            "relation_depth_ceiling": 64,
        },
        "certificate_sha256": [0] * 32,
    }
    payload["certificate_sha256"] = list(_certificate_digest(payload))
    return payload


def _write_resource(
    path: Path, payload: dict[str, Any], *, finalize: bool = True
) -> None:
    if finalize:
        payload["certificate_sha256"] = list(_certificate_digest(payload))
    path.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")


def _fixture(tmp_path: Path) -> Fixture:
    repository = (tmp_path / "repo").resolve()
    profile = repository / PROFILE_RELATIVE
    profile.parent.mkdir(parents=True)
    profile.write_text(_profile_source(), encoding="utf-8")
    readiness = repository / READINESS_RELATIVE
    readiness.parent.mkdir(parents=True, exist_ok=True)
    readiness.write_text(_readiness_source(), encoding="utf-8")
    (repository / "fixtures/privacy").mkdir(parents=True)
    exact12_matrix = repository / "fixtures/privacy/exact12_v1.tsv"
    exact12_matrix.write_bytes(b"compiled-exact12-test-double\n")
    (repository / "Cargo.lock").write_bytes(b"reviewed-lock-bytes\n")
    capture = (tmp_path / "capture").resolve()
    capture.mkdir()
    # This isolates installer transaction tests behind a verifier test double.
    # The Rust runner tests own typed Norito acceptance and explicitly reject
    # these sentinel NRT bytes through the production capture loaders.
    native_verifier = capture / "taira-privacy-release-runner"
    native_verifier.write_text(
        "#!/bin/sh\n"
        "set -eu\n"
        "[ \"$#\" -eq 11 ]\n"
        "[ \"$1\" = validate-captured-fixtures ]\n"
        "shift\n"
        "for expected in --exact12-matrix --expectations-norito "
        "--expectations-json --x509-resource-norito --x509-resource-json; do\n"
        "  [ \"$1\" = \"$expected\" ]\n"
        "  [ -n \"$2\" ]\n"
        "  shift 2\n"
        "done\n"
        "[ \"$#\" -eq 0 ]\n",
        encoding="utf-8",
    )
    native_verifier.chmod(0o500)
    native_verifier_sha256 = hashlib.sha256(native_verifier.read_bytes()).hexdigest()
    expectations_norito = capture / "expectations.norito"
    expectations_json = capture / "expectations.json"
    resource_norito = capture / "resource.norito"
    resource_json = capture / "resource.json"
    expectations_norito.write_bytes(b"NRT0\x00typed-native-expectations")
    expectations_json.write_bytes(_expectations_json())
    resource_norito.write_bytes(b"NRT0\x00typed-x509-resource-certificate")
    _write_resource(
        resource_json,
        _resource_payload(
            expectations_norito.read_bytes(), expectations_json.read_bytes()
        ),
    )
    return Fixture(
        repository,
        native_verifier,
        native_verifier_sha256,
        exact12_matrix,
        expectations_norito,
        expectations_json,
        resource_norito,
        resource_json,
        capture / "installation.json",
    )


def _run(
    fixture: Fixture, **overrides: Path | str
) -> subprocess.CompletedProcess[str]:
    values: dict[str, Path | str] = {
        "native_verifier": fixture.native_verifier,
        "native_verifier_sha256": fixture.native_verifier_sha256,
        "exact12_matrix": fixture.exact12_matrix,
        "expectations_norito": fixture.expectations_norito,
        "expectations_json": fixture.expectations_json,
        "resource_norito": fixture.resource_norito,
        "resource_json": fixture.resource_json,
        "manifest": fixture.manifest,
        "authenticated_iroha_source_commit": AUTH_IROHA_COMMIT,
        "authenticated_iroha_signer_principal": AUTH_IROHA_PRINCIPAL,
        "authenticated_iroha_signer_fingerprint": AUTH_IROHA_FINGERPRINT,
        "authenticated_iroha_allowed_signers_sha256": AUTH_IROHA_POLICY_SHA256,
        "authenticated_validator_source_commit": AUTH_VALIDATOR_COMMIT,
        "authenticated_validator_signer_principal": AUTH_VALIDATOR_PRINCIPAL,
        "authenticated_validator_signer_fingerprint": AUTH_VALIDATOR_FINGERPRINT,
        "authenticated_validator_allowed_signers_sha256": (
            AUTH_VALIDATOR_POLICY_SHA256
        ),
        "authenticated_validator_source_tree_sha256": (
            AUTH_VALIDATOR_SOURCE_TREE_SHA256
        ),
        "authenticated_bootstrap_source_tree_sha256": (
            AUTH_BOOTSTRAP_SOURCE_TREE_SHA256
        ),
        "authenticated_cargo_lock_sha256": AUTH_CARGO_LOCK_SHA256,
        "authenticated_rust_toolchain_tree_sha256": (
            AUTH_RUST_TOOLCHAIN_TREE_SHA256
        ),
    }
    values.update(overrides)
    return subprocess.run(
        [
            sys.executable,
            "-I",
            "-S",
            str(INSTALLER),
            "--repo",
            str(fixture.repository),
            "--native-verifier",
            str(values["native_verifier"]),
            "--native-verifier-sha256",
            str(values["native_verifier_sha256"]),
            "--exact12-matrix",
            str(values["exact12_matrix"]),
            "--captured-norito",
            str(values["expectations_norito"]),
            "--captured-json",
            str(values["expectations_json"]),
            "--captured-x509-resource-norito",
            str(values["resource_norito"]),
            "--captured-x509-resource-json",
            str(values["resource_json"]),
            "--manifest-out",
            str(values["manifest"]),
            "--authenticated-iroha-source-commit",
            str(values["authenticated_iroha_source_commit"]),
            "--authenticated-iroha-signer-principal",
            str(values["authenticated_iroha_signer_principal"]),
            "--authenticated-iroha-signer-fingerprint",
            str(values["authenticated_iroha_signer_fingerprint"]),
            "--authenticated-iroha-allowed-signers-sha256",
            str(values["authenticated_iroha_allowed_signers_sha256"]),
            "--authenticated-validator-source-commit",
            str(values["authenticated_validator_source_commit"]),
            "--authenticated-validator-signer-principal",
            str(values["authenticated_validator_signer_principal"]),
            "--authenticated-validator-signer-fingerprint",
            str(values["authenticated_validator_signer_fingerprint"]),
            "--authenticated-validator-allowed-signers-sha256",
            str(values["authenticated_validator_allowed_signers_sha256"]),
            "--authenticated-validator-source-tree-sha256",
            str(values["authenticated_validator_source_tree_sha256"]),
            "--authenticated-bootstrap-source-tree-sha256",
            str(values["authenticated_bootstrap_source_tree_sha256"]),
            "--authenticated-cargo-lock-sha256",
            str(values["authenticated_cargo_lock_sha256"]),
            "--authenticated-rust-toolchain-tree-sha256",
            str(values["authenticated_rust_toolchain_tree_sha256"]),
        ],
        check=False,
        capture_output=True,
        text=True,
    )


def _crash_install(fixture: Fixture, phase: str) -> subprocess.CompletedProcess[str]:
    driver = r'''
import importlib.util
import os
from pathlib import Path
import sys

module_path, phase = sys.argv[1:3]
arguments = sys.argv[3:]
spec = importlib.util.spec_from_file_location("taira_installer_under_test", module_path)
module = importlib.util.module_from_spec(spec)
assert spec.loader is not None
spec.loader.exec_module(module)

def failpoint(observed):
    if phase == "race_fixture_0" and observed == "fixture_0_temporary_durable":
        target = (
            Path(arguments[0])
            / "fixtures/privacy/native_release_expectations_v1.norito"
        )
        target.write_bytes(b"concurrent-substitution-must-survive")
        target.chmod(0o600)
        return
    if observed == phase:
        os._exit(91)

module.install(
    repository=Path(arguments[0]),
    native_verifier=Path(arguments[1]),
    native_verifier_sha256=arguments[2],
    exact12_matrix=Path(arguments[3]),
    captured_expectations_norito=Path(arguments[4]),
    captured_expectations_json=Path(arguments[5]),
    captured_resource_norito=Path(arguments[6]),
    captured_resource_json=Path(arguments[7]),
    manifest_path=Path(arguments[8]),
    authenticated_iroha_source_commit=arguments[9],
    authenticated_iroha_signer_principal=arguments[10],
    authenticated_iroha_signer_fingerprint=arguments[11],
    authenticated_iroha_allowed_signers_sha256=arguments[12],
    authenticated_validator_source_commit=arguments[13],
    authenticated_validator_signer_principal=arguments[14],
    authenticated_validator_signer_fingerprint=arguments[15],
    authenticated_validator_allowed_signers_sha256=arguments[16],
    authenticated_validator_source_tree_sha256=arguments[17],
    authenticated_bootstrap_source_tree_sha256=arguments[18],
    authenticated_cargo_lock_sha256=arguments[19],
    authenticated_rust_toolchain_tree_sha256=arguments[20],
    _failpoint=failpoint,
)
raise SystemExit("requested failpoint was not reached")
'''
    return subprocess.run(
        [
            sys.executable,
            "-I",
            "-S",
            "-c",
            driver,
            str(INSTALLER),
            phase,
            str(fixture.repository),
            str(fixture.native_verifier),
            fixture.native_verifier_sha256,
            str(fixture.exact12_matrix),
            str(fixture.expectations_norito),
            str(fixture.expectations_json),
            str(fixture.resource_norito),
            str(fixture.resource_json),
            str(fixture.manifest),
            AUTH_IROHA_COMMIT,
            AUTH_IROHA_PRINCIPAL,
            AUTH_IROHA_FINGERPRINT,
            AUTH_IROHA_POLICY_SHA256,
            AUTH_VALIDATOR_COMMIT,
            AUTH_VALIDATOR_PRINCIPAL,
            AUTH_VALIDATOR_FINGERPRINT,
            AUTH_VALIDATOR_POLICY_SHA256,
            AUTH_VALIDATOR_SOURCE_TREE_SHA256,
            AUTH_BOOTSTRAP_SOURCE_TREE_SHA256,
            AUTH_CARGO_LOCK_SHA256,
            AUTH_RUST_TOOLCHAIN_TREE_SHA256,
        ],
        check=False,
        capture_output=True,
        text=True,
    )


def _rejecting_verifier(fixture: Fixture) -> tuple[Path, str]:
    verifier = fixture.native_verifier.parent / "recovery-rejecting-verifier"
    verifier.write_text("#!/bin/sh\nexit 29\n", encoding="utf-8")
    verifier.chmod(0o500)
    return verifier, hashlib.sha256(verifier.read_bytes()).hexdigest()


def _recover_then_reject_reinstall(fixture: Fixture) -> subprocess.CompletedProcess[str]:
    verifier, verifier_sha256 = _rejecting_verifier(fixture)
    return _run(
        fixture,
        native_verifier=verifier,
        native_verifier_sha256=verifier_sha256,
    )


def _transaction_debris(fixture: Fixture) -> tuple[Path, ...]:
    mutation_paths = (
        fixture.repository / PROFILE_RELATIVE,
        fixture.repository / READINESS_RELATIVE,
        *(fixture.repository / target for target in ALL_TARGETS),
        fixture.manifest,
    )
    return (
        fixture.repository / ".taira-privacy-native-install-v1",
        fixture.repository / ".taira-privacy-native-install-cleanup-v1",
        *(
            path.with_name(f".{path.name}.taira-privacy-native-install-v1.tmp")
            for path in mutation_paths
        ),
    )


def _assert_unmodified(fixture: Fixture, profile: bytes, readiness: bytes) -> None:
    assert (fixture.repository / PROFILE_RELATIVE).read_bytes() == profile
    assert (fixture.repository / READINESS_RELATIVE).read_bytes() == readiness
    assert not any((fixture.repository / target).exists() for target in ALL_TARGETS)


def _assert_no_transaction_debris(fixture: Fixture) -> None:
    assert not any(os.path.lexists(path) for path in _transaction_debris(fixture))


def test_installs_all_exact_fixtures_and_source_pins(tmp_path: Path) -> None:
    fixture = _fixture(tmp_path)

    result = _run(fixture)

    assert result.returncode == 0, result.stderr
    captures = (
        fixture.expectations_norito,
        fixture.expectations_json,
        fixture.resource_norito,
        fixture.resource_json,
    )
    for target, capture in zip(ALL_TARGETS, captures):
        installed = fixture.repository / target
        assert installed.read_bytes() == capture.read_bytes()
        assert stat.S_IMODE(installed.stat().st_mode) == 0o444
    assert (fixture.repository / "Cargo.lock").read_bytes() == b"reviewed-lock-bytes\n"
    assert "[0; 32]" not in (fixture.repository / PROFILE_RELATIVE).read_text()
    assert "[0; 32]" not in (fixture.repository / READINESS_RELATIVE).read_text()
    manifest = json.loads(fixture.manifest.read_text())
    resource = json.loads(fixture.resource_json.read_text())
    assert (
        manifest["x509_resource_certificate"]["certificate_sha256"]
        == bytes(resource["certificate_sha256"]).hex()
    )
    assert manifest["x509_resource_certificate"]["kat_proof_bytes"] == 8_000_000
    assert manifest["native_capture_validation"] == {
        "mode": "validate-captured-fixtures",
        "verifier_sha256": fixture.native_verifier_sha256,
        "exact12_path": "fixtures/privacy/exact12_v1.tsv",
        "exact12_sha256": hashlib.sha256(
            fixture.exact12_matrix.read_bytes()
        ).hexdigest(),
    }
    assert manifest["installation_transaction"] == {
        "schema_version": 1,
        "commit_marker": "manifest-created-last",
    }
    assert manifest["authenticated_origins"] == {
        "iroha_source": {
            "commit": AUTH_IROHA_COMMIT,
            "signature_format": "ssh",
            "signer_principal": AUTH_IROHA_PRINCIPAL,
            "signer_fingerprint": AUTH_IROHA_FINGERPRINT,
            "allowed_signers_sha256": AUTH_IROHA_POLICY_SHA256,
        },
        "validator_source": {
            "commit": AUTH_VALIDATOR_COMMIT,
            "signature_format": "ssh",
            "signer_principal": AUTH_VALIDATOR_PRINCIPAL,
            "signer_fingerprint": AUTH_VALIDATOR_FINGERPRINT,
            "allowed_signers_sha256": AUTH_VALIDATOR_POLICY_SHA256,
            "source_tree_sha256": AUTH_VALIDATOR_SOURCE_TREE_SHA256,
        },
        "build_inputs": {
            "bootstrap_source_tree_sha256": AUTH_BOOTSTRAP_SOURCE_TREE_SHA256,
            "cargo_lock_sha256": AUTH_CARGO_LOCK_SHA256,
        },
        "rust_toolchain": {
            "release": "1.93.1",
            "host": "aarch64-unknown-linux-gnu",
            "compiler_commit": "01f6ddf7588f42ae2d7eb0a2f21d44e8e96674cf",
            "tree_sha256": AUTH_RUST_TOOLCHAIN_TREE_SHA256,
        },
    }
    _assert_no_transaction_debris(fixture)


@pytest.mark.parametrize(
    ("field", "value", "diagnostic"),
    [
        (
            "authenticated_iroha_source_commit",
            "0" * 40,
            "Iroha source commit must be one nonzero lowercase full Git commit",
        ),
        (
            "authenticated_validator_source_commit",
            "A" * 40,
            "validator source commit must be one nonzero lowercase full Git commit",
        ),
        (
            "authenticated_iroha_signer_principal",
            "operator-",
            "Iroha signer principal must be one bounded canonical ASCII",
        ),
        (
            "authenticated_validator_signer_principal",
            "operator,backup",
            "validator signer principal must be one bounded canonical ASCII",
        ),
        (
            "authenticated_iroha_signer_principal",
            "operator\N{SNOWMAN}",
            "Iroha signer principal must be one bounded canonical ASCII",
        ),
        (
            "authenticated_iroha_signer_fingerprint",
            "SHA256:" + "A" * 43,
            "nonzero canonical OpenSSH SHA256 fingerprint",
        ),
        (
            "authenticated_validator_signer_fingerprint",
            "SHA256:" + "C" * 42 + "=",
            "must be one OpenSSH SHA256 fingerprint",
        ),
        (
            "authenticated_iroha_allowed_signers_sha256",
            "0" * 64,
            "Iroha allowed-signers SHA-256 must be one nonzero lowercase",
        ),
        (
            "authenticated_validator_allowed_signers_sha256",
            "F" * 64,
            "validator allowed-signers SHA-256 must be one nonzero lowercase",
        ),
        (
            "authenticated_validator_source_tree_sha256",
            "0" * 64,
            "validator source-tree SHA-256 must be one nonzero lowercase",
        ),
        (
            "authenticated_bootstrap_source_tree_sha256",
            "short",
            "bootstrap source-tree SHA-256 must be one nonzero lowercase",
        ),
        (
            "authenticated_rust_toolchain_tree_sha256",
            "0" * 64,
            "Rust toolchain tree SHA-256 must be one nonzero lowercase",
        ),
    ],
)
def test_rejects_noncanonical_authenticated_origins_before_mutation(
    tmp_path: Path, field: str, value: str, diagnostic: str
) -> None:
    fixture = _fixture(tmp_path)
    profile = (fixture.repository / PROFILE_RELATIVE).read_bytes()
    readiness = (fixture.repository / READINESS_RELATIVE).read_bytes()

    result = _run(fixture, **{field: value})

    assert result.returncode != 0
    assert diagnostic in result.stderr
    _assert_unmodified(fixture, profile, readiness)
    assert not fixture.manifest.exists()
    _assert_no_transaction_debris(fixture)


def test_rejects_authenticated_cargo_lock_digest_mismatch_before_mutation(
    tmp_path: Path,
) -> None:
    fixture = _fixture(tmp_path)
    profile = (fixture.repository / PROFILE_RELATIVE).read_bytes()
    readiness = (fixture.repository / READINESS_RELATIVE).read_bytes()

    result = _run(fixture, authenticated_cargo_lock_sha256="8" * 64)

    assert result.returncode != 0
    assert "Cargo.lock does not match its authenticated origin digest" in result.stderr
    _assert_unmodified(fixture, profile, readiness)
    assert not fixture.manifest.exists()
    _assert_no_transaction_debris(fixture)


@pytest.mark.parametrize(
    "phase",
    [
        "journal_ready",
        "fixture_0_temporary_durable",
        "fixture_0_published",
        "fixture_0_installed",
        "fixture_3_temporary_durable",
        "fixture_3_published",
        "fixture_3_installed",
        "profile_temporary_durable",
        "profile_installed",
        "readiness_temporary_durable",
        "readiness_installed",
        "manifest_temporary_durable",
    ],
)
def test_precommit_crash_is_durably_rolled_back_before_reinstall(
    tmp_path: Path, phase: str
) -> None:
    fixture = _fixture(tmp_path)
    profile = (fixture.repository / PROFILE_RELATIVE).read_bytes()
    readiness = (fixture.repository / READINESS_RELATIVE).read_bytes()

    crashed = _crash_install(fixture, phase)

    assert crashed.returncode == 91, (crashed.stdout, crashed.stderr)
    assert (fixture.repository / ".taira-privacy-native-install-v1").is_dir()
    recovery = _recover_then_reject_reinstall(fixture)
    assert recovery.returncode != 0
    assert "native typed capture validation failed with code 29" in recovery.stderr
    _assert_unmodified(fixture, profile, readiness)
    assert not fixture.manifest.exists()
    _assert_no_transaction_debris(fixture)


@pytest.mark.parametrize(
    "phase", ["manifest_published", "manifest_committed", "before_transaction_cleanup"]
)
def test_postcommit_crash_recovers_as_one_complete_installation(
    tmp_path: Path, phase: str
) -> None:
    fixture = _fixture(tmp_path)

    crashed = _crash_install(fixture, phase)

    assert crashed.returncode == 91, (crashed.stdout, crashed.stderr)
    installed = {
        path: path.read_bytes()
        for path in (
            fixture.repository / PROFILE_RELATIVE,
            fixture.repository / READINESS_RELATIVE,
            *(fixture.repository / target for target in ALL_TARGETS),
            fixture.manifest,
        )
    }
    recovered = _run(fixture)
    assert recovered.returncode != 0
    assert "one-shot installation target already exists" in recovered.stderr
    assert {path: path.read_bytes() for path in installed} == installed
    _assert_no_transaction_debris(fixture)


def test_atomic_publish_never_overwrites_a_concurrent_target(tmp_path: Path) -> None:
    fixture = _fixture(tmp_path)
    profile = (fixture.repository / PROFILE_RELATIVE).read_bytes()
    readiness = (fixture.repository / READINESS_RELATIVE).read_bytes()

    raced = _crash_install(fixture, "race_fixture_0")

    target = fixture.repository / ALL_TARGETS[0]
    assert raced.returncode != 0
    assert "differs from the uncommitted transaction" in raced.stderr
    assert target.read_bytes() == b"concurrent-substitution-must-survive"
    assert (fixture.repository / PROFILE_RELATIVE).read_bytes() == profile
    assert (fixture.repository / READINESS_RELATIVE).read_bytes() == readiness
    transaction = fixture.repository / ".taira-privacy-native-install-v1"
    assert transaction.is_dir()
    assert not os.path.lexists(
        target.with_name(f".{target.name}.taira-privacy-native-install-v1.tmp")
    )


def test_recovery_preserves_a_substituted_transaction_temporary(
    tmp_path: Path,
) -> None:
    fixture = _fixture(tmp_path)
    assert _crash_install(fixture, "fixture_0_temporary_durable").returncode == 91
    target = fixture.repository / ALL_TARGETS[0]
    temporary = target.with_name(
        f".{target.name}.taira-privacy-native-install-v1.tmp"
    )
    temporary.chmod(0o600)
    temporary.write_bytes(b"attacker-substituted-transaction-temporary")

    recovered = _run(fixture)

    assert recovered.returncode != 0
    assert "temporary differs from transaction state" in recovered.stderr
    assert temporary.read_bytes() == b"attacker-substituted-transaction-temporary"
    assert not target.exists()


def test_recovery_rejects_an_extra_link_to_a_published_transaction_inode(
    tmp_path: Path,
) -> None:
    fixture = _fixture(tmp_path)
    assert _crash_install(fixture, "fixture_0_published").returncode == 91
    target = fixture.repository / ALL_TARGETS[0]
    temporary = target.with_name(
        f".{target.name}.taira-privacy-native-install-v1.tmp"
    )
    unexpected_link = fixture.manifest.parent / "unexpected-transaction-hardlink"
    os.link(temporary, unexpected_link)

    recovered = _run(fixture)

    assert recovered.returncode != 0
    assert "published temporary has an unexpected link count" in recovered.stderr
    assert target.stat().st_ino == temporary.stat().st_ino
    assert target.stat().st_ino == unexpected_link.stat().st_ino
    assert target.stat().st_nlink == 3


@pytest.mark.parametrize(
    "recovery_phase",
    ["recovery_profile_source_restored", "recovery_fixture_2_removed"],
)
def test_recovery_itself_is_idempotent_after_a_second_crash(
    tmp_path: Path, recovery_phase: str
) -> None:
    fixture = _fixture(tmp_path)
    profile = (fixture.repository / PROFILE_RELATIVE).read_bytes()
    readiness = (fixture.repository / READINESS_RELATIVE).read_bytes()
    assert _crash_install(fixture, "readiness_installed").returncode == 91

    recovery_crashed = _crash_install(fixture, recovery_phase)

    assert recovery_crashed.returncode == 91
    final_recovery = _recover_then_reject_reinstall(fixture)
    assert final_recovery.returncode != 0
    _assert_unmodified(fixture, profile, readiness)
    assert not fixture.manifest.exists()
    _assert_no_transaction_debris(fixture)


def test_restart_can_recover_a_journal_bound_to_the_original_manifest_path(
    tmp_path: Path,
) -> None:
    fixture = _fixture(tmp_path)
    profile = (fixture.repository / PROFILE_RELATIVE).read_bytes()
    readiness = (fixture.repository / READINESS_RELATIVE).read_bytes()
    original_manifest = fixture.manifest
    restarted_manifest = original_manifest.with_name("restarted-installation.json")
    assert _crash_install(fixture, "readiness_installed").returncode == 91
    verifier, verifier_sha256 = _rejecting_verifier(fixture)

    recovered = _run(
        fixture,
        native_verifier=verifier,
        native_verifier_sha256=verifier_sha256,
        manifest=restarted_manifest,
    )

    assert recovered.returncode != 0
    assert "native typed capture validation failed with code 29" in recovered.stderr
    _assert_unmodified(fixture, profile, readiness)
    assert not original_manifest.exists()
    assert not restarted_manifest.exists()
    assert not os.path.lexists(
        restarted_manifest.with_name(
            f".{restarted_manifest.name}.taira-privacy-native-install-v1.tmp"
        )
    )
    _assert_no_transaction_debris(fixture)


def test_recovery_rejects_tampered_authenticated_journal_state(tmp_path: Path) -> None:
    fixture = _fixture(tmp_path)
    profile = (fixture.repository / PROFILE_RELATIVE).read_bytes()
    readiness = (fixture.repository / READINESS_RELATIVE).read_bytes()
    assert _crash_install(fixture, "journal_ready").returncode == 91
    state = (
        fixture.repository
        / ".taira-privacy-native-install-v1"
        / "state-v1.json"
    )
    encoded = bytearray(state.read_bytes())
    encoded[-2] ^= 1
    state.write_bytes(encoded)

    recovered = _run(fixture)

    assert recovered.returncode != 0
    assert "ready marker does not authenticate its state" in recovered.stderr
    _assert_unmodified(fixture, profile, readiness)
    assert not fixture.manifest.exists()


def test_recovery_never_deletes_a_substituted_uncommitted_target(
    tmp_path: Path,
) -> None:
    fixture = _fixture(tmp_path)
    profile = (fixture.repository / PROFILE_RELATIVE).read_bytes()
    readiness = (fixture.repository / READINESS_RELATIVE).read_bytes()
    assert _crash_install(fixture, "fixture_0_installed").returncode == 91
    target = fixture.repository / ALL_TARGETS[0]
    target.chmod(0o600)
    target.write_bytes(b"attacker-substituted-target")

    recovered = _run(fixture)

    assert recovered.returncode != 0
    assert "differs from the uncommitted transaction" in recovered.stderr
    assert target.read_bytes() == b"attacker-substituted-target"
    assert (fixture.repository / PROFILE_RELATIVE).read_bytes() == profile
    assert (fixture.repository / READINESS_RELATIVE).read_bytes() == readiness


def test_recovery_rejects_a_tampered_postcommit_manifest(tmp_path: Path) -> None:
    fixture = _fixture(tmp_path)
    assert _crash_install(fixture, "manifest_committed").returncode == 91
    fixture.manifest.write_bytes(b"tampered-commit-marker\n")

    recovered = _run(fixture)

    assert recovered.returncode != 0
    assert "commit marker differs from transaction state" in recovered.stderr
    assert fixture.manifest.read_bytes() == b"tampered-commit-marker\n"


def test_incomplete_premutation_journal_is_safely_discarded(tmp_path: Path) -> None:
    fixture = _fixture(tmp_path)
    transaction = fixture.repository / ".taira-privacy-native-install-v1"
    transaction.mkdir(mode=0o700)
    (transaction / "profile.original").write_bytes(b"partial-journal-write")

    result = _run(fixture)

    assert result.returncode == 0, result.stderr
    _assert_no_transaction_debris(fixture)


def test_incomplete_journal_with_unknown_entry_fails_closed(tmp_path: Path) -> None:
    fixture = _fixture(tmp_path)
    transaction = fixture.repository / ".taira-privacy-native-install-v1"
    transaction.mkdir(mode=0o700)
    (transaction / "attacker-controlled").write_bytes(b"do not delete")
    profile = (fixture.repository / PROFILE_RELATIVE).read_bytes()
    readiness = (fixture.repository / READINESS_RELATIVE).read_bytes()

    result = _run(fixture)

    assert result.returncode != 0
    assert "contains unexpected entries" in result.stderr
    assert (transaction / "attacker-controlled").read_bytes() == b"do not delete"
    _assert_unmodified(fixture, profile, readiness)


def test_native_typed_validation_failure_precedes_every_install_mutation(
    tmp_path: Path,
) -> None:
    fixture = _fixture(tmp_path)
    verifier = fixture.native_verifier.parent / "rejecting-native-verifier"
    targets = " ".join(
        f"'{fixture.repository / target}'" for target in ALL_TARGETS
    )
    verifier.write_text(
        "#!/bin/sh\n"
        "set -eu\n"
        f"for target in {targets}; do [ ! -e \"$target\" ]; done\n"
        f"grep -q '\\[0; 32\\]' '{fixture.repository / PROFILE_RELATIVE}'\n"
        f"grep -q '\\[0; 32\\]' '{fixture.repository / READINESS_RELATIVE}'\n"
        "echo 'fake NRT rejected by native typed decoder' >&2\n"
        "exit 23\n",
        encoding="utf-8",
    )
    verifier.chmod(0o500)
    verifier_sha256 = hashlib.sha256(verifier.read_bytes()).hexdigest()
    profile = (fixture.repository / PROFILE_RELATIVE).read_bytes()
    readiness = (fixture.repository / READINESS_RELATIVE).read_bytes()

    result = _run(
        fixture,
        native_verifier=verifier,
        native_verifier_sha256=verifier_sha256,
    )

    assert result.returncode != 0
    assert "native typed capture validation failed with code 23" in result.stderr
    assert "fake NRT rejected by native typed decoder" in result.stderr
    _assert_unmodified(fixture, profile, readiness)


def test_rejects_unattested_native_verifier_before_execution(tmp_path: Path) -> None:
    fixture = _fixture(tmp_path)
    profile = (fixture.repository / PROFILE_RELATIVE).read_bytes()
    readiness = (fixture.repository / READINESS_RELATIVE).read_bytes()

    result = _run(fixture, native_verifier_sha256="11" * 32)

    assert result.returncode != 0
    assert "does not match its attested SHA-256" in result.stderr
    _assert_unmodified(fixture, profile, readiness)


def test_rejects_nonexecutable_native_verifier(tmp_path: Path) -> None:
    fixture = _fixture(tmp_path)
    fixture.native_verifier.chmod(0o400)
    profile = (fixture.repository / PROFILE_RELATIVE).read_bytes()
    readiness = (fixture.repository / READINESS_RELATIVE).read_bytes()

    result = _run(fixture)

    assert result.returncode != 0
    assert "singly linked executable file" in result.stderr
    _assert_unmodified(fixture, profile, readiness)


def test_rejects_multiply_linked_native_verifier(tmp_path: Path) -> None:
    fixture = _fixture(tmp_path)
    os.link(
        fixture.native_verifier,
        fixture.native_verifier.parent / "native-verifier-hardlink",
    )
    profile = (fixture.repository / PROFILE_RELATIVE).read_bytes()
    readiness = (fixture.repository / READINESS_RELATIVE).read_bytes()

    result = _run(fixture)

    assert result.returncode != 0
    assert "singly linked executable file" in result.stderr
    _assert_unmodified(fixture, profile, readiness)


def test_rejects_noncanonical_exact12_path(tmp_path: Path) -> None:
    fixture = _fixture(tmp_path)
    alternate = fixture.repository / "fixtures/privacy/alternate-exact12.tsv"
    alternate.write_bytes(fixture.exact12_matrix.read_bytes())
    profile = (fixture.repository / PROFILE_RELATIVE).read_bytes()
    readiness = (fixture.repository / READINESS_RELATIVE).read_bytes()

    result = _run(fixture, exact12_matrix=alternate)

    assert result.returncode != 0
    assert "canonical first-release repository fixture" in result.stderr
    _assert_unmodified(fixture, profile, readiness)


def test_rejects_capture_changed_by_native_validation(tmp_path: Path) -> None:
    fixture = _fixture(tmp_path)
    verifier = fixture.native_verifier.parent / "mutating-native-verifier"
    verifier.write_text(
        "#!/bin/sh\n"
        "set -eu\n"
        "[ \"$1\" = validate-captured-fixtures ]\n"
        "printf x >> \"$5\"\n",
        encoding="utf-8",
    )
    verifier.chmod(0o500)
    verifier_sha256 = hashlib.sha256(verifier.read_bytes()).hexdigest()
    profile = (fixture.repository / PROFILE_RELATIVE).read_bytes()
    readiness = (fixture.repository / READINESS_RELATIVE).read_bytes()

    result = _run(
        fixture,
        native_verifier=verifier,
        native_verifier_sha256=verifier_sha256,
    )

    assert result.returncode != 0
    assert "captured fixture bytes changed during native validation" in result.stderr
    _assert_unmodified(fixture, profile, readiness)


@pytest.mark.parametrize(
    "payload",
    [
        b"not-json",
        b"[]",
        json.dumps(
            {"schema_version": 2, "stage_count": 48, "stages": [{}] * 48}
        ).encode(),
        json.dumps(
            {"schema_version": 1, "stage_count": 47, "stages": [{}] * 48}
        ).encode(),
        json.dumps(
            {"schema_version": 1, "stage_count": 48, "stages": [{}] * 47}
        ).encode(),
        json.dumps({"schema_version": 1, "stage_count": 48, "stages": "48"}).encode(),
        json.dumps(
            {"schema_version": True, "stage_count": 48, "stages": [{}] * 48}
        ).encode(),
        json.dumps(
            {
                "schema_version": 1,
                "stage_count": 48,
                "stages": [{}] * 48,
                "legacy": True,
            }
        ).encode(),
        b'{"schema_version":1,"schema_version":1,"stage_count":48,"stages":[]}',
    ],
)
def test_rejects_malformed_expectations_json(tmp_path: Path, payload: bytes) -> None:
    fixture = _fixture(tmp_path)
    fixture.expectations_json.write_bytes(payload)
    profile = (fixture.repository / PROFILE_RELATIVE).read_bytes()
    readiness = (fixture.repository / READINESS_RELATIVE).read_bytes()

    result = _run(fixture)

    assert result.returncode != 0
    _assert_unmodified(fixture, profile, readiness)


def _mutate_environment(payload: dict[str, Any]) -> None:
    payload["environment"]["cpu_model"] = "Neoverse-V2"


def _mutate_process_limit(payload: dict[str, Any]) -> None:
    payload["process_limits"]["rayon_worker_count"] = 5


def _mutate_boolean_type(payload: dict[str, Any]) -> None:
    payload["process_limits"]["cgroup_v2"] = 1


def _mutate_protocol(payload: dict[str, Any]) -> None:
    payload["protocol_id"]["protocol"] = "iroha-zk-ams-v1"


def _mutate_expectation_digest(payload: dict[str, Any]) -> None:
    payload["expectations_json_sha256"][0] ^= 1


def _mutate_zero_compiled_digest(payload: dict[str, Any]) -> None:
    payload["compiled_profile_digest"] = [0] * 32


def _mutate_zero_kat_length(payload: dict[str, Any]) -> None:
    payload["kat_proof_bytes"] = 0


def _mutate_oversize_kat(payload: dict[str, Any]) -> None:
    payload["kat_proof_bytes"] = 8_212_539


def _mutate_zero_kat_digest(payload: dict[str, Any]) -> None:
    payload["kat_proof_sha256"] = [0] * 32


def _mutate_positive_case(payload: dict[str, Any]) -> None:
    payload["positive"]["case_kind"]["case"] = "public-statement-binding-mutation"


def _mutate_positive_shape(payload: dict[str, Any]) -> None:
    payload["positive"]["primary_units"] = 3


def _mutate_zero_observation(payload: dict[str, Any]) -> None:
    payload["positive"]["elapsed_millis"] = 0


def _mutate_overbound_observation(payload: dict[str, Any]) -> None:
    payload["maximum"]["peak_rss_bytes"] = 12 * 1024 * 1024 * 1024 + 1


@pytest.mark.parametrize(
    "mutate",
    [
        _mutate_environment,
        _mutate_process_limit,
        _mutate_boolean_type,
        _mutate_protocol,
        _mutate_expectation_digest,
        _mutate_zero_compiled_digest,
        _mutate_zero_kat_length,
        _mutate_oversize_kat,
        _mutate_zero_kat_digest,
        _mutate_positive_case,
        _mutate_positive_shape,
        _mutate_zero_observation,
        _mutate_overbound_observation,
    ],
)
def test_rejects_semantically_invalid_resource_certificate(
    tmp_path: Path, mutate: Callable[[dict[str, Any]], None]
) -> None:
    fixture = _fixture(tmp_path)
    payload = json.loads(fixture.resource_json.read_text())
    mutate(payload)
    _write_resource(fixture.resource_json, payload)
    profile = (fixture.repository / PROFILE_RELATIVE).read_bytes()
    readiness = (fixture.repository / READINESS_RELATIVE).read_bytes()

    result = _run(fixture)

    assert result.returncode != 0
    _assert_unmodified(fixture, profile, readiness)


@pytest.mark.parametrize("mutation", ["digest", "missing", "unknown", "duplicate"])
def test_rejects_malformed_or_noncanonical_resource_json(
    tmp_path: Path, mutation: str
) -> None:
    fixture = _fixture(tmp_path)
    payload = json.loads(fixture.resource_json.read_text())
    if mutation == "digest":
        payload["certificate_sha256"][0] ^= 1
        _write_resource(fixture.resource_json, payload, finalize=False)
    elif mutation == "missing":
        del payload["maximum"]
        _write_resource(fixture.resource_json, payload, finalize=False)
    elif mutation == "unknown":
        payload["legacy"] = True
        _write_resource(fixture.resource_json, payload, finalize=False)
    else:
        encoded = fixture.resource_json.read_text()
        fixture.resource_json.write_text(
            encoded.replace(
                '"schema_version": 1,',
                '"schema_version": 1,\\n  "schema_version": 1,',
                1,
            )
        )
    profile = (fixture.repository / PROFILE_RELATIVE).read_bytes()
    readiness = (fixture.repository / READINESS_RELATIVE).read_bytes()

    result = _run(fixture)

    assert result.returncode != 0
    _assert_unmodified(fixture, profile, readiness)


@pytest.mark.parametrize("existing_target", ALL_TARGETS)
def test_rejects_partial_or_existing_fixture_without_mutation(
    tmp_path: Path, existing_target: Path
) -> None:
    fixture = _fixture(tmp_path)
    target = fixture.repository / existing_target
    target.write_bytes(b"preexisting")
    profile = (fixture.repository / PROFILE_RELATIVE).read_bytes()
    readiness = (fixture.repository / READINESS_RELATIVE).read_bytes()

    result = _run(fixture)

    assert result.returncode != 0
    assert target.read_bytes() == b"preexisting"
    assert (fixture.repository / PROFILE_RELATIVE).read_bytes() == profile
    assert (fixture.repository / READINESS_RELATIVE).read_bytes() == readiness


SOURCE_PINS = (
    (PROFILE_RELATIVE, KAT_BYTES_PIN, "u32"),
    (PROFILE_RELATIVE, KAT_SHA_PIN, "digest"),
    (PROFILE_RELATIVE, EXPECTATIONS_NORITO_PIN, "digest"),
    (PROFILE_RELATIVE, EXPECTATIONS_JSON_PIN, "digest"),
    *((READINESS_RELATIVE, name, "u64") for name in OBSERVATION_PINS),
    (READINESS_RELATIVE, RESOURCE_CERT_PIN, "digest"),
)


@pytest.mark.parametrize(("source_path", "pin_name", "kind"), SOURCE_PINS)
def test_rejects_any_nonzero_or_partial_source_pin(
    tmp_path: Path, source_path: Path, pin_name: str, kind: str
) -> None:
    fixture = _fixture(tmp_path)
    fixture.native_verifier.chmod(0o700)
    fixture.native_verifier.write_text("#!/bin/sh\nexit 29\n", encoding="utf-8")
    fixture.native_verifier.chmod(0o500)
    fixture.native_verifier_sha256 = hashlib.sha256(
        fixture.native_verifier.read_bytes()
    ).hexdigest()
    path = fixture.repository / source_path
    source = path.read_text()
    if kind == "digest":
        source = source.replace(
            f"pub(crate) const {pin_name}: [u8; 32] = [0; 32];",
            f"pub(crate) const {pin_name}: [u8; 32] = [1; 32];",
        )
    else:
        source = source.replace(
            f"pub(crate) const {pin_name}: {kind} = 0;",
            f"pub(crate) const {pin_name}: {kind} = 1;",
        )
    path.write_text(source)
    profile = (fixture.repository / PROFILE_RELATIVE).read_bytes()
    readiness = (fixture.repository / READINESS_RELATIVE).read_bytes()

    result = _run(fixture)

    assert result.returncode != 0
    assert f"{pin_name} must have exactly one all-zero declaration" in result.stderr
    assert "native typed capture validation failed" not in result.stderr
    _assert_unmodified(fixture, profile, readiness)


def test_rejects_duplicate_zero_pin_declaration(tmp_path: Path) -> None:
    fixture = _fixture(tmp_path)
    path = fixture.repository / PROFILE_RELATIVE
    path.write_text(
        path.read_text()
        + f"pub(crate) const {EXPECTATIONS_NORITO_PIN}: [u8; 32] = [0; 32];\n"
    )

    result = _run(fixture)

    assert result.returncode != 0
    assert not any((fixture.repository / target).exists() for target in ALL_TARGETS)


def test_rejects_symlink_capture(tmp_path: Path) -> None:
    fixture = _fixture(tmp_path)
    symlink = fixture.expectations_norito.parent / "expectations-link.norito"
    symlink.symlink_to(fixture.expectations_norito)

    result = _run(fixture, expectations_norito=symlink)

    assert result.returncode != 0
    assert "canonical physical path" in result.stderr


def test_rejects_multiply_linked_capture(tmp_path: Path) -> None:
    fixture = _fixture(tmp_path)
    os.link(
        fixture.resource_norito,
        fixture.resource_norito.parent / "resource-hardlink.norito",
    )

    result = _run(fixture)

    assert result.returncode != 0
    assert "singly linked regular file" in result.stderr


@pytest.mark.parametrize(
    "capture_name",
    ["expectations_norito", "expectations_json", "resource_norito", "resource_json"],
)
def test_rejects_empty_capture(tmp_path: Path, capture_name: str) -> None:
    fixture = _fixture(tmp_path)
    getattr(fixture, capture_name).write_bytes(b"")

    result = _run(fixture)

    assert result.returncode != 0
    assert "non-empty, bounded" in result.stderr


@pytest.mark.parametrize(
    ("capture_name", "size"),
    [
        ("expectations_json", 1024 * 1024 * 1024 + 1),
        ("resource_json", 64 * 1024 + 1),
    ],
)
def test_rejects_oversized_sparse_capture_without_reading(
    tmp_path: Path, capture_name: str, size: int
) -> None:
    fixture = _fixture(tmp_path)
    with getattr(fixture, capture_name).open("wb") as stream:
        stream.truncate(size)

    result = _run(fixture)

    assert result.returncode != 0
    assert "non-empty, bounded" in result.stderr


def test_rejects_alias_between_capture_roles(tmp_path: Path) -> None:
    fixture = _fixture(tmp_path)

    result = _run(fixture, resource_norito=fixture.expectations_norito)

    assert result.returncode != 0
    assert "must not alias" in result.stderr


def test_rejects_capture_inside_source_checkout(tmp_path: Path) -> None:
    fixture = _fixture(tmp_path)
    in_tree = fixture.repository / "captured.norito"
    in_tree.write_bytes(b"captured")

    result = _run(fixture, expectations_norito=in_tree)

    assert result.returncode != 0
    assert "outside the source checkout" in result.stderr


def test_rejects_symlinked_fixture_parent_without_mutation(tmp_path: Path) -> None:
    fixture = _fixture(tmp_path)
    fixture_parent = fixture.repository / "fixtures/privacy"
    physical_parent = fixture.repository / "fixtures/privacy-physical"
    fixture_parent.rename(physical_parent)
    fixture_parent.symlink_to(physical_parent, target_is_directory=True)
    profile = (fixture.repository / PROFILE_RELATIVE).read_bytes()
    readiness = (fixture.repository / READINESS_RELATIVE).read_bytes()

    result = _run(fixture)

    assert result.returncode != 0
    assert "must be the canonical first-release repository fixture" in result.stderr
    assert (fixture.repository / PROFILE_RELATIVE).read_bytes() == profile
    assert (fixture.repository / READINESS_RELATIVE).read_bytes() == readiness


def test_rejects_existing_manifest_before_source_mutation(tmp_path: Path) -> None:
    fixture = _fixture(tmp_path)
    fixture.manifest.write_text("do not replace\n")
    profile = (fixture.repository / PROFILE_RELATIVE).read_bytes()
    readiness = (fixture.repository / READINESS_RELATIVE).read_bytes()

    result = _run(fixture)

    assert result.returncode != 0
    assert fixture.manifest.read_text() == "do not replace\n"
    _assert_unmodified(fixture, profile, readiness)


def test_second_install_is_rejected_and_preserves_first_install(tmp_path: Path) -> None:
    fixture = _fixture(tmp_path)
    first = _run(fixture)
    assert first.returncode == 0, first.stderr
    paths = (
        fixture.repository / PROFILE_RELATIVE,
        fixture.repository / READINESS_RELATIVE,
        *(fixture.repository / target for target in ALL_TARGETS),
        fixture.manifest,
    )
    installed = {path: path.read_bytes() for path in paths}

    second = _run(fixture)

    assert second.returncode != 0
    assert {path: path.read_bytes() for path in installed} == installed
