"""Adversarial tests for the final-V1 hermetic SCCP validator builder."""

from __future__ import annotations

import argparse
import base64
import copy
import hashlib
import os
import struct
import subprocess
import sys
import time
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
SCRIPTS = ROOT / "scripts"
sys.path.insert(0, os.fspath(SCRIPTS))

import sccp_release_common as common
import sccp_validator_builder as builder
import sccp_validator_builder_driver as driver


def _static_linux_amd64_elf(payload: bytes = b"validator") -> bytes:
    identity = b"\x7fELF\x02\x01\x01\x00\x00" + b"\x00" * 7
    size = 64 + 56 + len(payload)
    header = struct.pack(
        "<16sHHIQQQIHHHHHH",
        identity,
        2,
        62,
        1,
        0,
        64,
        0,
        0,
        64,
        56,
        1,
        0,
        0,
        0,
    )
    load = struct.pack("<IIQQQQQQ", 1, 5, 0, 0, 0, size, size, 4096)
    return header + load + payload


def _keypair(label: str) -> tuple[bytes, bytes, int]:
    entropy = hashlib.sha256(f"validator-builder-test:{label}".encode()).digest()
    expanded = hashlib.sha512(entropy).digest()
    scalar_bytes = bytearray(expanded[:32])
    scalar_bytes[0] &= 248
    scalar_bytes[31] &= 63
    scalar_bytes[31] |= 64
    scalar = int.from_bytes(scalar_bytes, "little")
    public = common._ed_encode(common._ed_scalar_multiply(common._ED_BASE, scalar))
    return public, expanded[32:], scalar


def _sign(keypair: tuple[bytes, bytes, int], message: bytes) -> str:
    public, prefix, scalar = keypair
    nonce = (
        int.from_bytes(hashlib.sha512(prefix + message).digest(), "little")
        % common._ED_L
    )
    encoded_r = common._ed_encode(common._ed_scalar_multiply(common._ED_BASE, nonce))
    challenge = (
        int.from_bytes(hashlib.sha512(encoded_r + public + message).digest(), "little")
        % common._ED_L
    )
    encoded_s = ((nonce + challenge * scalar) % common._ED_L).to_bytes(32, "little")
    signature = encoded_r + encoded_s
    assert common.verify_ed25519(public, signature, message)
    return base64.b64encode(signature).decode("ascii")


def _hash_role(index: int) -> str:
    return hashlib.sha256(f"validator-hash-role:{index}".encode()).hexdigest()


def _closure_documents(
    *, source_date_epoch: int, hashes: dict[str, str]
) -> dict[str, bytes]:
    cargo_config = b'[source.crates-io]\nreplace-with = "vendored-sources"\n'
    inventory = [
        {
            "path": "vendor/example-1.0.0/src/lib.rs",
            "sha256": _hash_role(30),
            "size_bytes": 19,
            "executable": False,
        }
    ]
    sysroot = [
        {
            "path": "sysroot/lib/rustlib/components",
            "sha256": _hash_role(31),
            "size_bytes": 17,
            "executable": False,
        }
    ]
    tools = [
        {
            "role": role,
            "path": path,
            "sha256": digest,
            "size_bytes": 100 + index,
            "executable": True,
        }
        for index, (role, path, digest) in enumerate(
            (
                ("builder-driver", builder.DRIVER_MOUNT, hashes["driver"]),
                ("cargo", "/toolchain/bin/cargo", _hash_role(33)),
                ("container-python", "/toolchain/bin/python3", _hash_role(34)),
                ("linker", "/toolchain/bin/cc", hashes["linker"]),
                ("rustc", "/toolchain/bin/rustc", _hash_role(35)),
            )
        )
    ]
    recipe = {
        "program": "/toolchain/bin/cargo",
        "arguments": [
            "build",
            "--release",
            "--locked",
            "--frozen",
            "--offline",
            "--no-default-features",
            "--features",
            "dev-tools",
            "-p",
            builder.CRATE,
            "--bin",
            builder.BINARY,
            "--jobs",
            "1",
            "--target",
            builder.TARGET,
        ],
        "working_directory": "${SOURCE}",
        "cargo_vendor_arguments": [
            "vendor",
            "--locked",
            "--offline",
            "--versioned-dirs",
            "${VENDOR}",
        ],
        "cargo_metadata_arguments": [
            "metadata",
            "--locked",
            "--offline",
            "--format-version=1",
            "--filter-platform",
            builder.TARGET,
            "--no-default-features",
            "--features",
            f"{builder.CRATE}/dev-tools",
        ],
        "cargo_config_sha256": hashlib.sha256(cargo_config).hexdigest(),
        "source_cargo_config_sha256": (builder.APPROVED_SOURCE_CARGO_CONFIG_SHA256),
        "driver_sha256": hashes["driver"],
    }
    environment = {
        "HOME": "/work/build/home",
        "CARGO_HOME": "/work/build/cargo-home",
        "CARGO_TARGET_DIR": "/work/build/target",
        "CARGO_INCREMENTAL": "0",
        "CARGO_NET_OFFLINE": "true",
        "CARGO_TERM_COLOR": "never",
        "CARGO_BUILD_JOBS": "1",
        "TMPDIR": "/work/build/target/tmp",
        "LANG": "C",
        "LC_ALL": "C",
        "TZ": "UTC",
        "SOURCE_DATE_EPOCH": str(source_date_epoch),
        "RUST_BACKTRACE": "0",
        "RUSTC": "/toolchain/bin/rustc",
        "RUSTFLAGS": (
            "--remap-path-prefix=/work/source=. "
            "--remap-path-prefix=/work/vendor=vendor "
            "--remap-path-prefix=/work/build/target=target "
            "-C target-feature=+crt-static "
            "-C strip=symbols"
        ),
        "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER": "/toolchain/bin/cc",
        "PATH": "/usr/local/bin:/usr/bin:/bin",
    }
    values = {
        "build-environment.json": environment,
        "build-recipe.json": recipe,
        "cargo-metadata-closure.json": {
            "workspace_root": "${SOURCE}",
            "target_directory": "${TARGET}",
            "packages": [
                {
                    "id": "path+file://${SOURCE}/crates/iroha_sccp#2.0.0",
                    "name": builder.CRATE,
                    "version": "2.0.0",
                    "source": None,
                    "license": "Apache-2.0",
                    "license_file": None,
                    "readme": "${SOURCE}/crates/iroha_sccp/README.md",
                    "manifest_path": "${SOURCE}/crates/iroha_sccp/Cargo.toml",
                    "targets": [
                        {
                            "src_path": (
                                "${SOURCE}/crates/iroha_sccp/src/bin/"
                                "sccp_release_evidence.rs"
                            )
                        }
                    ],
                    "dependencies": [],
                }
            ],
            "resolve": {
                "nodes": [
                    {
                        "id": "path+file://${SOURCE}/crates/iroha_sccp#2.0.0",
                        "dependencies": [],
                    }
                ]
            },
        },
        "dependency-inventory.json": inventory,
        "sbom.json": {
            "schema": "iroha.sccp.rust-validator-sbom.final-v1",
            "target_triple": builder.TARGET,
            "root_package_id": "path+file://${SOURCE}/crates/iroha_sccp#2.0.0",
            "binary": builder.BINARY,
            "enabled_features": list(builder.FEATURES),
            "packages": [
                {
                    "id": "path+file://${SOURCE}/crates/iroha_sccp#2.0.0",
                    "name": builder.CRATE,
                    "version": "2.0.0",
                    "source": None,
                    "license": "Apache-2.0",
                    "license_file": None,
                    "manifest_path": "${SOURCE}/crates/iroha_sccp/Cargo.toml",
                    "dependency_ids": [],
                }
            ],
        },
        "sysroot-inventory.json": sysroot,
        "toolchain-inventory.json": {
            "python_version": "Python 3.13.7",
            "cargo_version": "cargo 1.90.0 (840b83a10 2025-07-30)",
            "rustc_version": "rustc 1.90.0 (1159e78c4 2025-09-14)",
            "linker_version": "cc version 14.2.0",
            "sysroot": "${SYSROOT}",
            "tools": tools,
        },
    }
    documents = {
        name: common.canonical_json_file_bytes(values[name]) for name in values
    }
    documents["cargo-config.toml"] = cargo_config
    return documents


def _fixture(tmp_path: Path) -> dict[str, object]:
    keys = {role: _keypair(role) for role in builder.ROLES}
    driver_hash = hashlib.sha256(builder.DRIVER.read_bytes()).hexdigest()
    hashes = {"driver": driver_hash, "linker": _hash_role(20)}
    source_date_epoch = 1_700_000_000
    documents = _closure_documents(source_date_epoch=source_date_epoch, hashes=hashes)
    closure_expectations = {
        "dependency_inventory_sha256": hashlib.sha256(
            documents["dependency-inventory.json"]
        ).hexdigest(),
        "cargo_metadata_closure_sha256": hashlib.sha256(
            documents["cargo-metadata-closure.json"]
        ).hexdigest(),
        "sbom_sha256": hashlib.sha256(documents["sbom.json"]).hexdigest(),
        "toolchain_inventory_sha256": hashlib.sha256(
            documents["toolchain-inventory.json"]
        ).hexdigest(),
        "sysroot_inventory_sha256": hashlib.sha256(
            documents["sysroot-inventory.json"]
        ).hexdigest(),
        "linker_sha256": hashes["linker"],
        "build_recipe_sha256": hashlib.sha256(
            documents["build-recipe.json"]
        ).hexdigest(),
        "build_environment_sha256": hashlib.sha256(
            documents["build-environment.json"]
        ).hexdigest(),
    }
    policy: dict[str, object] = {
        "schema": builder.POLICY_SCHEMA,
        "source": {
            "commit": "10" * 20,
            "commit_signer_fingerprint": "0123456789ABCDEF",
            "source_date_epoch": source_date_epoch,
            "secret_scan_exceptions": [],
        },
        "builder": {
            "image": f"registry.example/iroha-validator@sha256:{_hash_role(1)}",
            "platform": builder.PLATFORM,
            "driver_path": builder.DRIVER_MOUNT,
            "driver_sha256": driver_hash,
            "python_path": "/toolchain/bin/python3",
            "python_reported_version": "Python 3.13.7",
            "cargo_path": "/toolchain/bin/cargo",
            "cargo_reported_version": "cargo 1.90.0 (840b83a10 2025-07-30)",
            "rustc_path": "/toolchain/bin/rustc",
            "rustc_reported_version": "rustc 1.90.0 (1159e78c4 2025-09-14)",
            "linker_path": "/toolchain/bin/cc",
            "linker_reported_version": "cc version 14.2.0",
            "cargo_home_path": "/toolchain/cargo-home",
            "target_triple": builder.TARGET,
            "host_python_sha256": _hash_role(2),
            "host_orchestrator_sha256": hashlib.sha256(
                builder.ORCHESTRATOR.read_bytes()
            ).hexdigest(),
            "host_release_common_sha256": hashlib.sha256(
                builder.RELEASE_COMMON.read_bytes()
            ).hexdigest(),
            "host_git_sha256": _hash_role(3),
            "host_docker_sha256": _hash_role(4),
            "docker_daemon_report_sha256": _hash_role(6),
            "host_commit_verifier_sha256": _hash_role(5),
            "closure_expectations": closure_expectations,
        },
        "limits": {
            "max_inventory_files": 1000,
            "max_file_bytes": 64 * 1024 * 1024,
            "max_total_bytes": 1024 * 1024 * 1024,
            "max_log_bytes": 1024 * 1024,
            "timeout_seconds": 3600,
        },
        "approvers": [
            {
                "role": role,
                "signer_id": f"validator-{role}",
                "public_key_hex": keys[role][0].hex(),
            }
            for role in builder.ROLES
        ],
    }
    policy = builder.validate_policy(policy)
    policy_bytes = common.canonical_json_file_bytes(policy)
    policy_sha256 = hashlib.sha256(policy_bytes).hexdigest()

    source_payload = b"deterministic tracked source archive\n"
    executable_payload = _static_linux_amd64_elf(b"final-v1-validator")
    source_sha256 = hashlib.sha256(source_payload).hexdigest()
    executable_sha256 = hashlib.sha256(executable_payload).hexdigest()
    report: dict[str, object] = {
        "schema": builder.DRIVER_REPORT_SCHEMA,
        "policy_sha256": policy_sha256,
        "source_commit": policy["source"]["commit"],
        "source_archive_sha256": source_sha256,
        "source_archive_size_bytes": len(source_payload),
        "builder_image": policy["builder"]["image"],
        "platform": builder.PLATFORM,
        "target_triple": builder.TARGET,
        "crate": builder.CRATE,
        "binary": builder.BINARY,
        "build_profile": "release",
        "enabled_features": list(builder.FEATURES),
        "build_jobs": 1,
        "default_features": False,
        "cargo_locked": True,
        "cargo_frozen": True,
        "cargo_offline": True,
        "network_disabled": True,
        "dependency_inventory_sha256": closure_expectations[
            "dependency_inventory_sha256"
        ],
        "dependency_inventory_size_bytes": len(documents["dependency-inventory.json"]),
        "cargo_metadata_closure_sha256": closure_expectations[
            "cargo_metadata_closure_sha256"
        ],
        "cargo_metadata_closure_size_bytes": len(
            documents["cargo-metadata-closure.json"]
        ),
        "sbom_sha256": closure_expectations["sbom_sha256"],
        "sbom_size_bytes": len(documents["sbom.json"]),
        "toolchain_inventory_sha256": closure_expectations[
            "toolchain_inventory_sha256"
        ],
        "toolchain_inventory_size_bytes": len(documents["toolchain-inventory.json"]),
        "sysroot_inventory_sha256": closure_expectations["sysroot_inventory_sha256"],
        "sysroot_inventory_size_bytes": len(documents["sysroot-inventory.json"]),
        "linker_sha256": hashes["linker"],
        "build_recipe_sha256": closure_expectations["build_recipe_sha256"],
        "build_recipe_size_bytes": len(documents["build-recipe.json"]),
        "build_environment_sha256": closure_expectations["build_environment_sha256"],
        "build_environment_size_bytes": len(documents["build-environment.json"]),
        "executable_path": f"validator/{builder.BINARY}",
        "executable_sha256": executable_sha256,
        "executable_size_bytes": len(executable_payload),
    }
    report_bytes = common.canonical_json_file_bytes(report)
    report_sha256 = hashlib.sha256(report_bytes).hexdigest()

    policy_path = tmp_path / "validator-policy.json"
    policy_path.write_bytes(policy_bytes)
    policy_path.chmod(0o600)
    candidates: list[Path] = []
    signed_paths: list[Path] = []
    for index, role in enumerate(builder.ROLES):
        raw = tmp_path / f"raw-{index}"
        (raw / "closure").mkdir(parents=True, mode=0o700)
        (raw / "validator").mkdir(mode=0o700)
        for name, payload in documents.items():
            (raw / "closure" / name).write_bytes(payload)
        (raw / "builder-report.json").write_bytes(report_bytes)
        executable = raw / "validator" / builder.BINARY
        executable.write_bytes(executable_payload)
        executable.chmod(0o700)
        archive = tmp_path / f"source-{index}.tar"
        archive.write_bytes(source_payload)
        archive.chmod(0o600)
        unsigned = builder._unsigned_rebuild(
            role=role,
            nonce_hex=f"{index + 1:02x}" * 32,
            built_at_unix_ms=time.time_ns() // 1_000_000 + index,
            policy=policy,
            policy_sha256=policy_sha256,
            report=report,
            report_sha256=report_sha256,
        )
        candidate = tmp_path / f"candidate-{index}"
        builder._publish_candidate(
            candidate,
            build_output=raw,
            source_archive=archive,
            report=report,
            report_bytes=report_bytes,
            unsigned=unsigned,
            policy=policy,
        )
        approver = policy["approvers"][index]
        signed = {
            **copy.deepcopy(unsigned),
            "provenance": {
                "role": role,
                "signer_id": approver["signer_id"],
                "algorithm": "ed25519",
                "public_key_hex": approver["public_key_hex"],
                "signature_b64": _sign(
                    keys[role], builder.rebuild_signing_payload(unsigned)
                ),
            },
        }
        signed_path = tmp_path / f"signed-{index}.json"
        signed_path.write_bytes(common.canonical_json_file_bytes(signed))
        signed_path.chmod(0o600)
        candidates.append(candidate)
        signed_paths.append(signed_path)
    return {
        "policy": policy,
        "policy_path": policy_path,
        "policy_sha256": policy_sha256,
        "report": report,
        "candidates": candidates,
        "signed_paths": signed_paths,
    }


def test_policy_is_exact_digest_pinned_and_role_separated(tmp_path: Path) -> None:
    fixture = _fixture(tmp_path)
    policy = fixture["policy"]
    assert builder.validate_policy(policy) == policy
    for mutation in range(12):
        candidate = copy.deepcopy(policy)
        if mutation == 0:
            candidate["builder"]["image"] = "registry.example/validator:latest"
        elif mutation == 1:
            candidate["builder"]["platform"] = "linux/arm64"
        elif mutation == 2:
            candidate["builder"]["driver_sha256"] = "00" * 32
        elif mutation == 3:
            candidate["builder"]["closure_expectations"]["linker_sha256"] = candidate[
                "builder"
            ]["host_git_sha256"]
        elif mutation == 4:
            candidate["builder"]["target_triple"] = "aarch64-unknown-linux-gnu"
        elif mutation == 5:
            candidate["approvers"].reverse()
        elif mutation == 6:
            candidate["approvers"][1]["public_key_hex"] = candidate["approvers"][0][
                "public_key_hex"
            ]
        elif mutation == 7:
            candidate["limits"]["max_total_bytes"] = 65 * 1024**3
        elif mutation == 8:
            candidate["builder"]["host_orchestrator_sha256"] = "00" * 32
        elif mutation == 9:
            candidate["builder"]["host_release_common_sha256"] = "00" * 32
        elif mutation == 10:
            candidate["builder"]["docker_daemon_report_sha256"] = candidate["builder"][
                "host_docker_sha256"
            ]
        else:
            candidate["legacy"] = True
        with pytest.raises(builder.ValidatorBuilderError):
            builder.validate_policy(candidate)


def test_candidates_require_exact_signed_independent_rebuilds(tmp_path: Path) -> None:
    fixture = _fixture(tmp_path)
    loaded = [
        builder._load_candidate(
            fixture["candidates"][index],
            fixture["signed_paths"][index],
            role=role,
            policy=fixture["policy"],
            policy_sha256=fixture["policy_sha256"],
        )
        for index, role in enumerate(builder.ROLES)
    ]
    assert loaded[0]["report_bytes"] == loaded[1]["report_bytes"]
    assert loaded[0]["signed_sha256"] != loaded[1]["signed_sha256"]

    signed_value = common.parse_json_bytes(
        fixture["signed_paths"][0].read_bytes(),
        label="signed rebuild",
        maximum=builder.MAX_SIGNED_REBUILD_BYTES,
    )
    signed_value["executable_sha256"] = _hash_role(60)
    fixture["signed_paths"][0].write_bytes(
        common.canonical_json_file_bytes(signed_value)
    )
    with pytest.raises(builder.ValidatorBuilderError):
        builder._load_candidate(
            fixture["candidates"][0],
            fixture["signed_paths"][0],
            role=builder.ROLES[0],
            policy=fixture["policy"],
            policy_sha256=fixture["policy_sha256"],
        )


def test_finalize_publishes_complete_manifest_last_bundle(tmp_path: Path) -> None:
    fixture = _fixture(tmp_path)
    output = tmp_path / "release"
    arguments = argparse.Namespace(
        policy=fixture["policy_path"],
        trusted_policy_sha256=fixture["policy_sha256"],
        engineering_candidate=fixture["candidates"][0],
        engineering_signed_rebuild=fixture["signed_paths"][0],
        security_candidate=fixture["candidates"][1],
        security_signed_rebuild=fixture["signed_paths"][1],
        output_dir=output,
    )
    builder.finalize_release(arguments)
    expected = {
        "builder-report.json",
        "closure",
        "rebuilds",
        "source.tar",
        "validator",
        "validator-build-receipt.json",
        "validator-builder-policy.json",
        "validator-output-lock.json",
    }
    assert {entry.name for entry in output.iterdir()} == expected
    assert output.stat().st_mode & 0o077 == 0
    assert (output / "validator" / builder.BINARY).stat().st_mode & 0o111
    receipt_bytes = (output / "validator-build-receipt.json").read_bytes()
    receipt = common.parse_json_bytes(
        receipt_bytes, label="validator receipt", maximum=builder.MAX_RECEIPT_BYTES
    )
    assert receipt_bytes == common.canonical_json_file_bytes(receipt)
    assert receipt["schema"] == builder.RECEIPT_SCHEMA
    assert (
        receipt["validator_output_lock_sha256"]
        == hashlib.sha256(
            (output / "validator-output-lock.json").read_bytes()
        ).hexdigest()
    )
    for field in (
        "validator_source_archive_sha256",
        "validator_dependency_inventory_sha256",
        "validator_cargo_metadata_closure_sha256",
        "validator_sbom_sha256",
        "validator_toolchain_inventory_sha256",
        "validator_sysroot_inventory_sha256",
        "validator_linker_sha256",
        "validator_build_recipe_sha256",
        "validator_build_environment_sha256",
        "validator_container_manifest_sha256",
        "validator_executable_sha256",
    ):
        assert receipt[field] != "00" * 32
    verification = builder.verify_release_directory(
        output,
        trusted_policy_sha256=fixture["policy_sha256"],
    )
    assert verification["schema"] == builder.VERIFICATION_SCHEMA
    assert tuple(verification["hashes"]) == builder.RECEIPT_HASH_FIELDS
    assert (
        verification["hashes"]["validator_executable_sha256"]
        == receipt["validator_executable_sha256"]
    )
    assert verification["validator_executable_path"] == str(
        (output / "validator" / builder.BINARY).resolve()
    )


@pytest.mark.parametrize(
    "mutation",
    ("executable", "rebuild", "output-lock", "receipt", "extra-file"),
)
def test_public_release_verifier_rejects_post_publication_mutation(
    tmp_path: Path, mutation: str
) -> None:
    fixture = _fixture(tmp_path)
    output = tmp_path / "release"
    builder.finalize_release(
        argparse.Namespace(
            policy=fixture["policy_path"],
            trusted_policy_sha256=fixture["policy_sha256"],
            engineering_candidate=fixture["candidates"][0],
            engineering_signed_rebuild=fixture["signed_paths"][0],
            security_candidate=fixture["candidates"][1],
            security_signed_rebuild=fixture["signed_paths"][1],
            output_dir=output,
        )
    )
    if mutation == "executable":
        executable = output / "validator" / builder.BINARY
        executable.write_bytes(b"\x7fELFmutated-after-publication")
        executable.chmod(0o700)
    elif mutation == "rebuild":
        rebuild = output / "rebuilds" / f"{builder.ROLES[0]}.json"
        value = common.parse_json_bytes(
            rebuild.read_bytes(),
            label="test rebuild",
            maximum=builder.MAX_SIGNED_REBUILD_BYTES,
        )
        value["executable_sha256"] = _hash_role(91)
        rebuild.write_bytes(common.canonical_json_file_bytes(value))
    elif mutation == "output-lock":
        lock = output / "validator-output-lock.json"
        value = common.parse_json_bytes(
            lock.read_bytes(), label="test lock", maximum=builder.MAX_LOCK_BYTES
        )
        value["container_manifest_sha256"] = _hash_role(92)
        lock.write_bytes(common.canonical_json_file_bytes(value))
    elif mutation == "receipt":
        receipt = output / "validator-build-receipt.json"
        value = common.parse_json_bytes(
            receipt.read_bytes(),
            label="test receipt",
            maximum=builder.MAX_RECEIPT_BYTES,
        )
        value["validator_executable_sha256"] = _hash_role(93)
        receipt.write_bytes(common.canonical_json_file_bytes(value))
    else:
        (output / "unlisted.txt").write_text("not part of final-v1\n", encoding="utf-8")
    with pytest.raises(builder.ValidatorBuilderError):
        builder.verify_release_directory(
            output,
            trusted_policy_sha256=fixture["policy_sha256"],
        )


def test_byte_identity_check_rejects_equal_size_different_content(
    tmp_path: Path,
) -> None:
    first = tmp_path / "first.bin"
    second = tmp_path / "second.bin"
    first.write_bytes(b"alpha")
    second.write_bytes(b"omega")
    with pytest.raises(builder.ValidatorBuilderError, match="differs"):
        builder._files_byte_identical(
            first,
            second,
            expected_sha256=hashlib.sha256(b"alpha").hexdigest(),
            maximum=32,
            label="test build closure",
        )


def test_driver_sbom_uses_only_resolved_packages_and_exact_edges() -> None:
    root = "path+file://${SOURCE}/crates/iroha_sccp#2.0.0"
    dependency = "registry+https://example.invalid/index#dep@1.2.3"
    unused = "registry+https://example.invalid/index#unused@9.9.9"
    metadata = {
        "packages": [
            {
                "id": unused,
                "name": "unused",
                "version": "9.9.9",
                "source": "registry+https://example.invalid/index",
                "license": None,
                "license_file": None,
                "manifest_path": "${CARGO_HOME}/unused/Cargo.toml",
            },
            {
                "id": dependency,
                "name": "dep",
                "version": "1.2.3",
                "source": "registry+https://example.invalid/index",
                "license": "MIT",
                "license_file": None,
                "manifest_path": "${VENDOR}/dep/Cargo.toml",
            },
            {
                "id": root,
                "name": driver.CRATE,
                "version": "2.0.0",
                "source": None,
                "license": "Apache-2.0",
                "license_file": None,
                "manifest_path": "${SOURCE}/crates/iroha_sccp/Cargo.toml",
            },
        ],
        "resolve": {
            "nodes": [
                {"id": root, "dependencies": [dependency]},
                {"id": dependency, "dependencies": []},
            ]
        },
    }
    sbom = driver._build_sbom(metadata)
    assert sbom["root_package_id"] == root
    assert [package["id"] for package in sbom["packages"]] == [root, dependency]
    assert sbom["packages"][0]["dependency_ids"] == [dependency]
    assert all(package["id"] != unused for package in sbom["packages"])
    metadata["resolve"]["nodes"].append({"id": root, "dependencies": []})
    with pytest.raises(driver.DriverError, match="repeats"):
        driver._build_sbom(metadata)


def test_driver_streams_authenticated_executable_copy(tmp_path: Path) -> None:
    source = tmp_path / "validator"
    payload = b"\x7fELF" + b"release-validator" * 4096
    source.write_bytes(payload)
    source.chmod(0o700)
    destination = tmp_path / "output" / "validator"
    driver._copy_new(
        source,
        destination,
        expected_sha256=hashlib.sha256(payload).hexdigest(),
        expected_size=len(payload),
        maximum=len(payload),
        executable=True,
    )
    assert destination.read_bytes() == payload
    assert destination.stat().st_mode & 0o111
    with pytest.raises(driver.DriverError):
        driver._copy_new(
            source,
            tmp_path / "other" / "validator",
            expected_sha256=_hash_role(70),
            expected_size=len(payload),
            maximum=len(payload),
            executable=True,
        )


def test_driver_rejects_cargo_home_configuration_and_credentials(
    tmp_path: Path,
) -> None:
    for name in ("config.toml", "credentials.toml"):
        source = tmp_path / f"cache-{name}"
        source.mkdir()
        (source / name).write_text("[registry]\ntoken = 'must-not-enter-build'\n")
        with pytest.raises(driver.DriverError, match="forbidden"):
            driver._copy_regular_tree(
                source,
                tmp_path / f"copy-{name}",
                maximum_files=10,
                maximum_file_bytes=1024,
                maximum_total_bytes=4096,
            )


def test_driver_authenticates_every_blob_and_gitlink_in_source_archive(
    tmp_path: Path,
) -> None:
    source = tmp_path / "source"
    source.mkdir()
    executable = source / "build.sh"
    executable.write_bytes(b"#!/bin/sh\nexit 0\n")
    executable.chmod(0o700)
    link = source / "current"
    link.symlink_to("build.sh")
    gitlink = source / "iroha-docs"
    gitlink.mkdir()

    def object_id(payload: bytes) -> str:
        digest = hashlib.sha1()
        digest.update(f"blob {len(payload)}\0".encode("ascii"))
        digest.update(payload)
        return digest.hexdigest()

    entries = [
        {
            "mode": "100755",
            "object_type": "blob",
            "object_id": object_id(executable.read_bytes()),
            "path": "build.sh",
        },
        {
            "mode": "120000",
            "object_type": "blob",
            "object_id": object_id(b"build.sh"),
            "path": "current",
        },
        {
            "mode": "160000",
            "object_type": "commit",
            "object_id": "ab" * 20,
            "path": "iroha-docs",
        },
    ]
    inventory = {
        "schema": "iroha.sccp.git-source-tree-inventory.final-v1",
        "source_commit": "cd" * 20,
        "object_format": "sha1",
        "entries": entries,
    }
    (source / driver.SOURCE_TREE_INVENTORY).write_bytes(
        driver._canonical_json(inventory)
    )
    driver._validate_source_tree_inventory(
        source,
        source_commit="cd" * 20,
        maximum_files=16,
        maximum_file_bytes=1024,
    )
    executable.write_bytes(b"#!/bin/sh\nexit 1\n")
    with pytest.raises(driver.DriverError, match="differs"):
        driver._validate_source_tree_inventory(
            source,
            source_commit="cd" * 20,
            maximum_files=16,
            maximum_file_bytes=1024,
        )


def test_tracked_path_scan_allows_source_names_but_rejects_concrete_tokens() -> None:
    builder._reject_tracked_path_material(
        b"java/example/crypto/NativeSigningPrivateKey.java"
    )
    with pytest.raises(common.SccpReleaseError, match="forbidden credential material"):
        builder._reject_tracked_path_material(
            b"fixtures/github_pat_AAAAAAAAAAAAAAAAAAAAAAAA.txt"
        )
    with pytest.raises(common.SccpReleaseError, match="forbidden credential material"):
        builder._reject_concrete_material(
            b'{"access_token":"redacted-is-still-forbidden"}\n',
            label="test closure",
            inspect_json_keys=True,
        )


def test_builder_source_closes_network_features_tools_and_large_file_streaming() -> (
    None
):
    orchestrator = (SCRIPTS / "sccp_validator_builder.py").read_text(encoding="utf-8")
    driver = (SCRIPTS / "sccp_validator_builder_driver.py").read_text(encoding="utf-8")
    for token in (
        "--network=none",
        "--platform=linux/amd64",
        "--pull=never",
        "--read-only",
        "--cap-drop=ALL",
        "--commit-verifier",
        "GIT_CONFIG_NOSYSTEM",
        "GIT_NO_REPLACE_OBJECTS",
        "--locked",
        "--frozen",
        "--offline",
        "--no-default-features",
        '"dev-tools"',
        '"--jobs"',
        '"1"',
    ):
        assert token in orchestrator + driver
    assert "read_bytes()" not in driver
    assert "subprocess.run(" not in driver
    assert "publish_for_host_scan=True" in driver
    assert builder.ROLES == ("release-engineering", "release-security")
    assert os.stat(SCRIPTS / "sccp_validator_builder.py").st_mode & 0o111
    assert os.stat(SCRIPTS / "sccp_validator_builder_driver.py").st_mode & 0o111


def test_cli_shape_error_never_echoes_untrusted_secret_like_argument() -> None:
    marker = "authorization=Bearer-do-not-echo"
    result = subprocess.run(
        [
            sys.executable,
            os.fspath(SCRIPTS / "sccp_validator_builder.py"),
            "--bad",
            marker,
        ],
        cwd=ROOT,
        check=False,
        text=True,
        capture_output=True,
    )
    assert result.returncode == 2
    assert marker not in result.stdout + result.stderr
    assert len(result.stderr) < 1024
