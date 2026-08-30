#!/usr/bin/env python3
"""Focused, non-Cargo tests for the native zk-X509 candidate controller."""

from __future__ import annotations

import hashlib
import importlib.util
import json
import os
from pathlib import Path
import signal
import shutil
import struct
import subprocess
import sys
import tempfile
import time
import unittest
from unittest import mock


SCRIPT = Path(__file__).resolve().parents[1] / "capture_zk_x509_native_candidate.py"
SPEC = importlib.util.spec_from_file_location("capture_zk_x509_native_candidate", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


def make_elf(
    *,
    interpreter: bool = False,
    writable_executable: bool = False,
    needed: bool = False,
    executable_stack: bool = False,
) -> bytes:
    program_count = 1 + int(needed) + int(executable_stack)
    dynamic = struct.pack("<qQqQ", 1, 1, 0, 0) if needed else b""
    total = 64 + 56 * program_count + len(dynamic)
    header = struct.pack(
        "<16sHHIQQQIHHHHHH",
        b"\x7fELF" + bytes((2, 1, 1)) + bytes(9),
        2,
        183,
        1,
        0,
        64,
        0,
        0,
        64,
        56,
        program_count,
        0,
        0,
        0,
    )
    first_type = 3 if interpreter else 1
    first_flags = 7 if writable_executable else 5
    first = struct.pack("<IIQQQQQQ", first_type, first_flags, 0, 0, 0, total, total, 4096)
    segments = [first]
    if needed:
        dynamic_offset = 64 + 56 * program_count
        segments.append(
            struct.pack(
                "<IIQQQQQQ", 2, 4, dynamic_offset, 0, 0, len(dynamic), len(dynamic), 8
            )
        )
    if executable_stack:
        segments.append(struct.pack("<IIQQQQQQ", 0x6474E551, 1, 0, 0, 0, 0, 0, 16))
    return header + b"".join(segments) + dynamic


def observation(case: str, shape: tuple[int, int, int, int, int, int]) -> dict[str, object]:
    return {
        "case_kind": {"case": case, "value": None},
        "elapsed_millis": 7,
        "peak_rss_bytes": 4096,
        "peak_address_space_bytes": 8192,
        "primary_units": shape[0],
        "primary_ceiling": shape[1],
        "secondary_units": shape[2],
        "secondary_ceiling": shape[3],
        "relation_depth": shape[4],
        "relation_depth_ceiling": shape[5],
    }


def bounded_test_command(source: str, *arguments: Path | str) -> list[str]:
    return [
        sys.executable,
        "-I",
        "-S",
        "-c",
        source,
        *(os.fspath(argument) for argument in arguments),
    ]


def bounded_test_environment() -> dict[str, str]:
    return {"LANG": "C", "LC_ALL": "C", "PATH": "/usr/bin:/bin"}


class CandidateControllerPureTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name).resolve()
        os.chmod(self.root, 0o700)

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def _elf_path(self, name: str, payload: bytes) -> Path:
        path = self.root / name
        path.write_bytes(payload)
        path.chmod(0o700)
        return path

    @unittest.skipUnless(
        sys.platform.startswith("linux"),
        "bounded process execution is intentionally Linux-only",
    )
    def test_bounded_process_rejects_stdout_and_stderr_floods(self) -> None:
        for descriptor, stream in ((1, "stdout"), (2, "stderr")):
            with self.subTest(stream=stream), self.assertRaisesRegex(
                MODULE._BoundedProcessError, f"{stream} exceeded"
            ):
                MODULE._run_bounded_process(
                    bounded_test_command(
                        f"import os; os.write({descriptor}, b'x' * 8192)"
                    ),
                    cwd=self.root,
                    environment=bounded_test_environment(),
                    timeout=5,
                    stdout_limit=1024,
                    stderr_limit=1024,
                )

    @unittest.skipUnless(
        sys.platform.startswith("linux"),
        "bounded process execution is intentionally Linux-only",
    )
    def test_bounded_process_contains_setsid_descendant_after_parent_success(self) -> None:
        escape_marker = self.root / "escaped-descendant"
        command = bounded_test_command(
            "\n".join(
                (
                    "import os, sys, time",
                    "pid = os.fork()",
                    "if pid == 0:",
                    "    os.setsid()",
                    "    for descriptor in (0, 1, 2):",
                    "        try:",
                    "            os.close(descriptor)",
                    "        except OSError:",
                    "            pass",
                    "    time.sleep(0.25)",
                    "    fd = os.open(sys.argv[1], os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)",
                    "    os.write(fd, b'escaped')",
                    "    os.close(fd)",
                    "    os._exit(0)",
                    "os._exit(0)",
                )
            ),
            escape_marker,
        )
        completed = MODULE._run_bounded_process(
            command,
            cwd=self.root,
            environment=bounded_test_environment(),
            timeout=2,
            stdout_limit=1024,
            stderr_limit=4096,
        )
        self.assertEqual(completed.returncode, 0)
        time.sleep(0.5)
        self.assertFalse(
            escape_marker.exists(),
            "a detached subprocess descendant outlived the bounded runner",
        )

    @unittest.skipUnless(
        sys.platform.startswith("linux"),
        "the direct-target namespace teardown attack is Linux-specific",
    )
    def test_bounded_process_contains_direct_target_after_timeout(self) -> None:
        armed = self.root / "armed"
        trigger = self.root / "trigger"
        escape_marker = self.root / "escaped-target"
        command = bounded_test_command(
            "\n".join(
                (
                    "import ctypes, os, sys, time",
                    "if ctypes.CDLL(None).prctl(1, 0, 0, 0, 0) != 0:",
                    "    os._exit(91)",
                    "os.setsid()",
                    "fd = os.open(sys.argv[1], os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)",
                    "os.write(fd, b'armed')",
                    "os.close(fd)",
                    "for descriptor in (0, 1, 2):",
                    "    try:",
                    "        os.close(descriptor)",
                    "    except OSError:",
                    "        pass",
                    "while not os.path.exists(sys.argv[2]):",
                    "    time.sleep(0.01)",
                    "fd = os.open(sys.argv[3], os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)",
                    "os.write(fd, b'escaped')",
                    "os.close(fd)",
                )
            ),
            armed,
            trigger,
            escape_marker,
        )
        with self.assertRaisesRegex(MODULE._BoundedProcessError, "timed out"):
            MODULE._run_bounded_process(
                command,
                cwd=self.root,
                environment=bounded_test_environment(),
                timeout=0.5,
                stdout_limit=1024,
                stderr_limit=1024,
            )
        self.assertTrue(armed.exists(), "the direct target did not reach its escape posture")
        trigger.write_bytes(b"go")
        time.sleep(0.25)
        self.assertFalse(
            escape_marker.exists(),
            "a direct target escaped namespace teardown after timeout",
        )

    @unittest.skipUnless(
        sys.platform.startswith("linux"),
        "bounded process execution is intentionally Linux-only",
    )
    def test_bounded_process_preserves_signal_status(self) -> None:
        for termination_signal in (signal.SIGINT, signal.SIGTERM, signal.SIGKILL):
            with self.subTest(termination_signal=termination_signal):
                completed = MODULE._run_bounded_process(
                    bounded_test_command(
                        f"import os; os.kill(os.getpid(), {int(termination_signal)})"
                    ),
                    cwd=self.root,
                    environment=bounded_test_environment(),
                    timeout=5,
                    stdout_limit=1024,
                    stderr_limit=1024,
                )
                self.assertEqual(completed.returncode, -int(termination_signal))

    @unittest.skipUnless(
        sys.platform.startswith("linux"),
        "the exec handshake is implemented by the Linux containment supervisor",
    )
    def test_bounded_process_rejects_failed_exec_handshake(self) -> None:
        with self.assertRaisesRegex(
            MODULE._BoundedProcessError,
            "OS-enforced descendant containment",
        ):
            MODULE._run_bounded_process(
                ["/definitely/missing-zk-x509-target"],
                cwd=self.root,
                environment=bounded_test_environment(),
                timeout=5,
                stdout_limit=1024,
                stderr_limit=1024,
            )

    def test_bounded_process_fails_before_exec_on_unsupported_host(self) -> None:
        marker = self.root / "ran"
        with mock.patch.object(sys, "platform", "unsupported-zk-x509-host"), self.assertRaisesRegex(
            MODULE._BoundedProcessError,
            "requires Linux user and PID namespaces",
        ):
            MODULE._run_bounded_process(
                bounded_test_command(
                    "import pathlib, sys; pathlib.Path(sys.argv[1]).write_bytes(b'ran')",
                    marker,
                ),
                cwd=self.root,
                environment=bounded_test_environment(),
                timeout=5,
                stdout_limit=1024,
                stderr_limit=1024,
            )
        self.assertFalse(marker.exists())

    @unittest.skipUnless(
        sys.platform.startswith("linux"),
        "the unavailable namespace bootstrap is Linux-specific",
    )
    def test_bounded_process_never_execs_if_linux_containment_bootstrap_fails(self) -> None:
        marker = self.root / "ran"
        with mock.patch.object(
            MODULE,
            "_LINUX_PID_NAMESPACE_SUPERVISOR",
            "import os; os._exit(125)",
        ), self.assertRaisesRegex(
            MODULE._BoundedProcessError,
            "could not be established",
        ):
            MODULE._run_bounded_process(
                bounded_test_command(
                    "import pathlib, sys; pathlib.Path(sys.argv[1]).write_bytes(b'ran')",
                    marker,
                ),
                cwd=self.root,
                environment=bounded_test_environment(),
                timeout=5,
                stdout_limit=1024,
                stderr_limit=1024,
            )
        self.assertFalse(marker.exists())

    @unittest.skipUnless(
        sys.platform.startswith("linux"),
        "bounded process execution is intentionally Linux-only",
    )
    def test_bounded_process_interleaves_stdin_with_both_output_streams(self) -> None:
        input_data = b"i" * (64 * 1024)
        completed = MODULE._run_bounded_process(
            bounded_test_command(
                "\n".join(
                    (
                        "import os",
                        "total = 0",
                        "while True:",
                        "    chunk = os.read(0, 256)",
                        "    if not chunk:",
                        "        break",
                        "    total += len(chunk)",
                        "    os.write(1, b'o' * 1024)",
                        "    os.write(2, b'e' * 1024)",
                        "os.write(1, str(total).encode('ascii'))",
                    )
                )
            ),
            cwd=self.root,
            environment=bounded_test_environment(),
            timeout=5,
            stdout_limit=300 * 1024,
            stderr_limit=300 * 1024,
            input_data=input_data,
        )
        self.assertEqual(completed.returncode, 0)
        self.assertTrue(completed.stdout.endswith(str(len(input_data)).encode("ascii")))
        self.assertEqual(
            len(completed.stdout), 256 * 1024 + len(str(len(input_data)))
        )
        self.assertEqual(completed.stderr, b"e" * (256 * 1024))

    def test_static_elf_gate_rejects_interp_needed_and_writable_executable(self) -> None:
        accepted = MODULE.validate_static_aarch64_elf(
            self._elf_path("accepted", make_elf()), "synthetic ELF"
        )
        self.assertEqual(accepted["elf_machine"], "AArch64")
        for name, payload, diagnostic in (
            ("interp", make_elf(interpreter=True), "PT_INTERP"),
            ("needed", make_elf(needed=True), "DT_NEEDED"),
            ("wx", make_elf(writable_executable=True), "writable executable"),
            ("exec-stack", make_elf(executable_stack=True), "executable GNU stack"),
        ):
            with self.subTest(name=name), self.assertRaisesRegex(
                MODULE.CandidateCaptureError, diagnostic
            ):
                MODULE.validate_static_aarch64_elf(
                    self._elf_path(name, payload), f"synthetic {name} ELF"
                )

    def test_exact_one_signature_parser_rejects_multiple_or_non_ssh(self) -> None:
        armor = (
            b"-----BEGIN SSH SIGNATURE-----\n"
            b"AAAA\n"
            b"-----END SSH SIGNATURE-----"
        )
        raw = b"tree " + b"0" * 40 + b"\ngpgsig " + armor.replace(b"\n", b"\n ") + b"\n\nmessage\n"
        self.assertEqual(MODULE.require_exact_one_ssh_signature(raw), armor)
        duplicate = raw.replace(b"\n\nmessage", b"\ngpgsig " + armor.replace(b"\n", b"\n ") + b"\n\nmessage")
        with self.assertRaisesRegex(MODULE.CandidateCaptureError, "exactly one"):
            MODULE.require_exact_one_ssh_signature(duplicate)
        pgp = raw.replace(b"SSH SIGNATURE", b"PGP SIGNATURE")
        with self.assertRaisesRegex(MODULE.CandidateCaptureError, "canonical SSH"):
            MODULE.require_exact_one_ssh_signature(pgp)

    def test_resource_validator_recomputes_all_sixty_fields(self) -> None:
        expectations_norito = hashlib.sha256(b"expectations-norito").hexdigest()
        expectations_json = hashlib.sha256(b"expectations-json").hexdigest()
        payload: dict[str, object] = {
            "schema_version": 1,
            "protocol_id": {"protocol": "iroha-zk-x509-stark-p256-v0", "value": None},
            "compiled_profile_digest": list(bytes.fromhex("11" * 32)),
            "environment": dict(MODULE.EXPECTED_ENVIRONMENT),
            "expectations_norito_sha256": list(bytes.fromhex(expectations_norito)),
            "expectations_json_sha256": list(bytes.fromhex(expectations_json)),
            "kat_proof_bytes": 128,
            "kat_proof_sha256": list(bytes.fromhex("22" * 32)),
            "process_limits": dict(MODULE.EXPECTED_PROCESS_LIMITS),
            "positive": observation("positive-canonical-end-to-end", (2, 3, 1, 4, 0, 64)),
            "maximum": observation("maximum-shape-resource", (3, 3, 4, 4, 64, 64)),
            "certificate_sha256": [0] * 32,
        }
        positive = {key: value for key, value in payload["positive"].items() if key != "case_kind"}
        maximum = {key: value for key, value in payload["maximum"].items() if key != "case_kind"}
        calculated = MODULE.resource_certificate_digest(
            payload,
            compiled_profile_digest=bytes.fromhex("11" * 32),
            expectations_norito_digest=bytes.fromhex(expectations_norito),
            expectations_json_digest=bytes.fromhex(expectations_json),
            kat_digest=bytes.fromhex("22" * 32),
            positive=positive,
            maximum=maximum,
        )
        payload["certificate_sha256"] = list(calculated)
        encoded = json.dumps(payload, separators=(",", ":")).encode()
        result = MODULE.validate_resource_json(encoded, expectations_norito, expectations_json)
        self.assertEqual(result["certificate_sha256"], calculated.hex())
        payload["positive"]["elapsed_millis"] += 1
        with self.assertRaisesRegex(MODULE.CandidateCaptureError, "payload digest"):
            MODULE.validate_resource_json(
                json.dumps(payload, separators=(",", ":")).encode(),
                expectations_norito,
                expectations_json,
            )

    def test_source_freeze_requires_exact_eleven_zero_pins_and_no_fixtures(self) -> None:
        profile = self.root / MODULE.PROFILE_RELATIVE
        readiness = self.root / MODULE.READINESS_RELATIVE
        profile.parent.mkdir(parents=True)
        readiness.parent.mkdir(parents=True)
        profile.write_text("\n".join((
            "pub(crate) const ZK_X509_RELEASE_KAT_EXPECTED_PROOF_BYTES_V1: u32 = 0;",
            "pub(crate) const ZK_X509_RELEASE_KAT_EXPECTED_PROOF_SHA256_V1: [u8; 32] = [0; 32];",
            "pub(crate) const ZK_X509_NATIVE_RELEASE_EXPECTATIONS_NORITO_SHA256_V1: [u8; 32] = [0; 32];",
            "pub(crate) const ZK_X509_NATIVE_RELEASE_EXPECTATIONS_JSON_SHA256_V1: [u8; 32] = [0; 32];",
        )) + "\n", encoding="utf-8")
        readiness.write_text("\n".join((
            "pub(crate) const ZK_X509_RESOURCE_POSITIVE_ELAPSED_MILLIS_V1: u64 = 0;",
            "pub(crate) const ZK_X509_RESOURCE_POSITIVE_PEAK_RSS_BYTES_V1: u64 = 0;",
            "pub(crate) const ZK_X509_RESOURCE_POSITIVE_PEAK_ADDRESS_SPACE_BYTES_V1: u64 = 0;",
            "pub(crate) const ZK_X509_RESOURCE_MAXIMUM_ELAPSED_MILLIS_V1: u64 = 0;",
            "pub(crate) const ZK_X509_RESOURCE_MAXIMUM_PEAK_RSS_BYTES_V1: u64 = 0;",
            "pub(crate) const ZK_X509_RESOURCE_MAXIMUM_PEAK_ADDRESS_SPACE_BYTES_V1: u64 = 0;",
            "pub(crate) const ZK_X509_RESOURCE_CERTIFICATE_SHA256_V1: [u8; 32] = [0; 32];",
        )) + "\n", encoding="utf-8")
        result = MODULE.require_zero_capture_pins(self.root)
        self.assertEqual(result["capture_owned_pin_count"], 11)
        self.assertTrue(result["capture_owned_fixture_files_absent"])
        readiness.write_text(
            readiness.read_text(encoding="utf-8").replace(
                "ZK_X509_RESOURCE_MAXIMUM_ELAPSED_MILLIS_V1: u64 = 0",
                "ZK_X509_RESOURCE_MAXIMUM_ELAPSED_MILLIS_V1: u64 = 1",
            ),
            encoding="utf-8",
        )
        with self.assertRaisesRegex(MODULE.CandidateCaptureError, "declaration is not exact"):
            MODULE.require_zero_capture_pins(self.root)

    def test_source_package_binding_rejects_cargo_lock_drift(self) -> None:
        manifest = {
            "source_commit": "1" * 40,
            "source_commit_raw_sha256": "6" * 64,
            "source_allowed_signers_sha256": "2" * 64,
            "source_revocation_sha256": "3" * 64,
            "source_signer_fingerprint": "SHA256:" + "A" * 43,
            "source_signer_principal": "release@example",
            "workspace_source_manifest_sha256": "4" * 64,
            "cargo_lock_sha256": "5" * 64,
            "protocol_id": "iroha-zk-x509-stark-p256-v0",
            "protocol_version": 1,
            "public_request_schema_version": 1,
        }
        source = {
            "workspace_source_manifest_sha256": "4" * 64,
            "cargo_lock_sha256": "5" * 64,
        }
        authentication = {
            "commit": "1" * 40,
            "raw_commit_sha256": "6" * 64,
            "allowed_signers": {"sha256": "2" * 64},
            "revocation_policy": {"sha256": "3" * 64},
            "signer_fingerprint": "SHA256:" + "A" * 43,
            "signer_principal": "release@example",
        }
        MODULE.validate_source_package_binding(manifest, source, authentication)
        manifest["cargo_lock_sha256"] = "6" * 64
        with self.assertRaisesRegex(MODULE.CandidateCaptureError, "cargo_lock_sha256"):
            MODULE.validate_source_package_binding(manifest, source, authentication)

    def test_cargo_runner_is_direct_and_config_inventory_is_recomputed(self) -> None:
        command = MODULE.runner_build_command(
            Path("/rust/bin/cargo"), Path("/proc/self/fd/42/Cargo.toml")
        )
        self.assertEqual(command[:4], [
            "/rust/bin/cargo",
            "rustc",
            "--manifest-path",
            "/proc/self/fd/42/Cargo.toml",
        ])
        self.assertNotIn("iroha-fast", command)
        cargo_home = self.root / "cargo-home"
        cargo_home.mkdir(mode=0o700)
        MODULE.require_no_effective_cargo_configuration(cargo_home)
        (cargo_home / "config.toml").write_text("[net]\noffline=true\n", encoding="utf-8")
        with self.assertRaisesRegex(MODULE.CandidateCaptureError, "unexpected configuration"):
            MODULE.require_no_effective_cargo_configuration(cargo_home)

    def test_descriptor_anchored_cleanup_does_not_follow_symlinks(self) -> None:
        output = self.root / "output"
        output.mkdir(mode=0o700)
        staging = output / "staging"
        staging.mkdir(mode=0o700)
        nested = staging / "nested"
        nested.mkdir()
        (nested / "payload").write_bytes(b"candidate")
        outside = self.root / "outside"
        outside.mkdir()
        survivor = outside / "survivor"
        survivor.write_bytes(b"must remain")
        (staging / "escape").symlink_to(outside, target_is_directory=True)
        output_descriptor = os.open(output, os.O_RDONLY | os.O_DIRECTORY)
        staging_descriptor = os.open(staging, os.O_RDONLY | os.O_DIRECTORY)
        try:
            MODULE._cleanup_staging(
                staging_descriptor, output_descriptor, staging.name
            )
        finally:
            os.close(staging_descriptor)
            os.close(output_descriptor)
        self.assertFalse(staging.exists())
        self.assertEqual(survivor.read_bytes(), b"must remain")

    def test_descriptor_anchored_finalization_seals_and_renames_exact_inode(self) -> None:
        output = self.root / "candidate-output"
        output.mkdir(mode=0o700)
        staging = output / "staging"
        staging.mkdir(mode=0o700)
        evidence = staging / "evidence.json"
        evidence.write_bytes(b"{}\n")
        output_descriptor = os.open(output, os.O_RDONLY | os.O_DIRECTORY)
        staging_descriptor = os.open(staging, os.O_RDONLY | os.O_DIRECTORY)
        try:
            destination = MODULE.finalize_candidate_directory(
                staging,
                staging_descriptor,
                output,
                output_descriptor,
                {"candidate_only": True},
            )
            self.assertTrue(destination.is_dir())
            self.assertEqual(destination.stat().st_mode & 0o777, 0o500)
            self.assertEqual((destination / "evidence.json").stat().st_mode & 0o777, 0o400)
            envelope = json.loads(
                (destination / "candidate-envelope-v1.json").read_bytes()
            )
            self.assertEqual(
                envelope["payload"]["evidence_inventory"][0]["path"],
                "evidence.json",
            )
            MODULE._cleanup_staging(
                staging_descriptor, output_descriptor, destination.name
            )
            self.assertFalse(destination.exists())
        finally:
            os.close(staging_descriptor)
            os.close(output_descriptor)

    def test_candidate_publication_collision_preserves_existing_entry(self) -> None:
        output = self.root / "collision-output"
        output.mkdir(mode=0o700)
        staging = output / "staging"
        staging.mkdir(mode=0o700)
        (staging / "evidence.json").write_bytes(b"{}\n")
        output_descriptor = os.open(output, os.O_RDONLY | os.O_DIRECTORY)
        staging_descriptor = os.open(staging, os.O_RDONLY | os.O_DIRECTORY)
        real_publish = MODULE._atomic_rename_noreplace

        def collide(source: str, destination: str, **kwargs: object) -> None:
            attacker = output / destination
            attacker.mkdir(mode=0o700)
            (attacker / "survivor").write_bytes(b"must remain")
            real_publish(source, destination, **kwargs)

        try:
            with mock.patch.object(MODULE, "_atomic_rename_noreplace", side_effect=collide):
                with self.assertRaisesRegex(MODULE.CandidateCaptureError, "already exists"):
                    MODULE.finalize_candidate_directory(
                        staging,
                        staging_descriptor,
                        output,
                        output_descriptor,
                        {"candidate_only": True},
                    )
            survivors = list(output.glob("zk-x509-native-candidate-*/survivor"))
            self.assertEqual(len(survivors), 1)
            self.assertEqual(survivors[0].read_bytes(), b"must remain")
            MODULE._cleanup_staging(
                staging_descriptor, output_descriptor, staging.name
            )
        finally:
            os.close(staging_descriptor)
            os.close(output_descriptor)

    def test_candidate_publication_rejects_post_rename_inventory_injection(self) -> None:
        output = self.root / "injection-output"
        output.mkdir(mode=0o700)
        staging = output / "staging"
        staging.mkdir(mode=0o700)
        (staging / "evidence.json").write_bytes(b"{}\n")
        output_descriptor = os.open(output, os.O_RDONLY | os.O_DIRECTORY)
        staging_descriptor = os.open(staging, os.O_RDONLY | os.O_DIRECTORY)
        real_publish = MODULE._atomic_rename_noreplace

        def inject(source: str, destination: str, **kwargs: object) -> None:
            real_publish(source, destination, **kwargs)
            if kwargs.get("label") == "candidate evidence publication":
                published = output / destination
                published.chmod(0o700)
                (published / "injected").write_bytes(b"attacker")
                published.chmod(0o500)

        try:
            with mock.patch.object(MODULE, "_atomic_rename_noreplace", side_effect=inject):
                with self.assertRaisesRegex(
                    MODULE.CandidateCaptureError, "inventory|exactly sealed"
                ):
                    MODULE.finalize_candidate_directory(
                        staging,
                        staging_descriptor,
                        output,
                        output_descriptor,
                        {"candidate_only": True},
                    )
            self.assertTrue(staging.exists())
            MODULE._cleanup_staging(
                staging_descriptor, output_descriptor, staging.name
            )
        finally:
            os.close(staging_descriptor)
            os.close(output_descriptor)

    def test_openssl_runtime_rejects_path_replacement_during_descriptor_use(self) -> None:
        openssl = self.root / "openssl"
        ldd = self.root / "ldd"
        openssl.write_bytes(b"#!/bin/sh\nexit 0\n")
        ldd.write_bytes(b"#!/bin/sh\nexit 0\n")
        openssl.chmod(0o700)
        ldd.chmod(0o700)
        observed_argv: list[str] = []

        def replace_after_command(arguments: list[str], **_kwargs: object):
            observed_argv.extend(arguments)
            replacement = self.root / "replacement-openssl"
            replacement.write_bytes(b"#!/bin/sh\nexit 9\n")
            replacement.chmod(0o700)
            os.replace(replacement, openssl)
            return (
                {"argv": list(arguments), "passed_file_descriptors": []},
                b"OpenSSL fixture\n",
                b"",
            )

        with self.assertRaisesRegex(
            MODULE.CandidateCaptureError, "changed during descriptor-bound use"
        ):
            with MODULE.hold_executable(
                openssl, "OpenSSL executable", maximum=MODULE.MAX_TOOL_BYTES
            ) as held_openssl, MODULE.hold_executable(
                ldd, "ldd executable", maximum=MODULE.MAX_TOOL_BYTES
            ) as held_ldd:
                with mock.patch.object(
                    MODULE, "run_checked", side_effect=replace_after_command
                ):
                    MODULE.openssl_runtime_closure(
                        held_openssl, held_ldd, cwd=Path(os.sep)
                    )
        self.assertNotEqual(observed_argv[0], str(openssl))
        self.assertIn("/fd/", observed_argv[0])

    def test_finalization_rolls_back_rename_when_output_fsync_fails(self) -> None:
        output = self.root / "rollback-output"
        output.mkdir(mode=0o700)
        staging = output / "staging"
        staging.mkdir(mode=0o700)
        (staging / "evidence.json").write_bytes(b"{}\n")
        output_descriptor = os.open(output, os.O_RDONLY | os.O_DIRECTORY)
        staging_descriptor = os.open(staging, os.O_RDONLY | os.O_DIRECTORY)
        output_identity = os.fstat(output_descriptor)
        real_fsync = os.fsync

        def fail_output_fsync(descriptor: int) -> None:
            details = os.fstat(descriptor)
            if (details.st_dev, details.st_ino) == (
                output_identity.st_dev,
                output_identity.st_ino,
            ):
                raise OSError("injected output fsync failure")
            real_fsync(descriptor)

        try:
            with mock.patch.object(MODULE.os, "fsync", side_effect=fail_output_fsync):
                with self.assertRaisesRegex(OSError, "injected output fsync"):
                    MODULE.finalize_candidate_directory(
                        staging,
                        staging_descriptor,
                        output,
                        output_descriptor,
                        {"candidate_only": True},
                    )
            self.assertEqual([path.name for path in output.iterdir()], ["staging"])
            MODULE._cleanup_staging(
                staging_descriptor, output_descriptor, staging.name
            )
        finally:
            os.close(staging_descriptor)
            os.close(output_descriptor)


@unittest.skipUnless(
    Path("/usr/bin/git").is_file() and Path("/usr/bin/ssh-keygen").is_file(),
    "system Git and ssh-keygen are required",
)
@unittest.skipIf(
    sys.platform == "darwin",
    "Darwin's fail-closed no-fork containment rejects Git's ssh-keygen helper",
)
class RawSshCommitAuthenticationTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name).resolve()
        os.chmod(self.root, 0o700)
        self.repository = self.root / "repository"
        self.repository.mkdir(mode=0o700)
        self.keys = self.root / "keys"
        self.keys.mkdir(mode=0o700)

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def _run(self, *arguments: str, cwd: Path | None = None) -> bytes:
        return subprocess.run(
            list(arguments),
            cwd=cwd or self.root,
            env={"LANG": "C", "LC_ALL": "C", "PATH": "/usr/bin:/bin"},
            check=True,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        ).stdout

    def test_authenticates_raw_commit_and_records_exact_principal(self) -> None:
        signing_key = self.keys / "signing"
        revoked_key = self.keys / "revoked"
        self._run("/usr/bin/ssh-keygen", "-q", "-t", "ed25519", "-N", "", "-f", str(signing_key))
        self._run("/usr/bin/ssh-keygen", "-q", "-t", "ed25519", "-N", "", "-f", str(revoked_key))
        self._run("/usr/bin/git", "init", "-q", str(self.repository))
        self._run("/usr/bin/git", "config", "user.name", "Release Test", cwd=self.repository)
        self._run("/usr/bin/git", "config", "user.email", "release@example.test", cwd=self.repository)
        self._run("/usr/bin/git", "config", "gpg.format", "ssh", cwd=self.repository)
        self._run("/usr/bin/git", "config", "user.signingkey", str(signing_key), cwd=self.repository)
        for relative in MODULE.SIGNED_CONTROLLER_FILES:
            path = self.repository / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            if relative == MODULE.SOURCE_HELPER_RELATIVE:
                path.write_text(
                    """#!/usr/bin/env python3
import hashlib, json, pathlib, subprocess, sys
root = pathlib.Path(sys.argv[sys.argv.index('--root') + 1])
head = subprocess.check_output(['/usr/bin/git', '-C', str(root), 'rev-parse', 'HEAD']).decode().strip()
tree = subprocess.check_output(['/usr/bin/git', '-C', str(root), 'rev-parse', 'HEAD^{tree}']).decode().strip()
lock = hashlib.sha256((root / 'Cargo.lock').read_bytes()).hexdigest()
print(json.dumps({'schema_version': 1, 'head_commit': head, 'head_tree': tree, 'index_tree': tree, 'workspace_source_manifest_sha256': hashlib.sha256(head.encode()).hexdigest(), 'cargo_lock_sha256': lock}, sort_keys=True, separators=(',', ':')))
""",
                    encoding="utf-8",
                )
            else:
                path.write_text(f"fixture for {relative.as_posix()}\n", encoding="utf-8")
        self._run("/usr/bin/git", "add", ".", cwd=self.repository)
        self._run("/usr/bin/git", "commit", "-q", "-S", "-m", "signed fixture", cwd=self.repository)
        commit = self._run("/usr/bin/git", "rev-parse", "HEAD", cwd=self.repository).decode().strip()
        public_key = signing_key.with_suffix(".pub").read_text(encoding="ascii").strip()
        allowed = self.keys / "allowed-signers"
        allowed.write_text(f"release@example.test {public_key}\n", encoding="ascii")
        revocation = self.keys / "revocation"
        revocation.write_text(revoked_key.with_suffix(".pub").read_text(encoding="ascii"), encoding="ascii")
        fingerprint = self._run(
            "/usr/bin/ssh-keygen", "-E", "sha256", "-lf", str(signing_key.with_suffix(".pub"))
        ).decode().split()[1]
        authentication, identity = MODULE.authenticate_source_commit(
            self.repository,
            expected_commit=commit,
            allowed_signers=allowed,
            allowed_signers_sha256=hashlib.sha256(allowed.read_bytes()).hexdigest(),
            revocation=revocation,
            revocation_sha256=hashlib.sha256(revocation.read_bytes()).hexdigest(),
            expected_principal="release@example.test",
            expected_fingerprint=fingerprint,
            git_path=Path("/usr/bin/git"),
            ssh_keygen_path=Path("/usr/bin/ssh-keygen"),
            python_path=Path(shutil.which("python3") or "").resolve(strict=True),
        )
        self.assertEqual(authentication["signature_count"], 1)
        self.assertEqual(authentication["signer_principal"], "release@example.test")
        self.assertEqual(authentication["signer_fingerprint"], fingerprint)
        helper_command = authentication["commands"][-1]
        self.assertTrue(
            helper_command["argv"][3].startswith(("/proc/self/fd/", "/dev/fd/"))
        )
        self.assertNotIn(
            str(self.repository / MODULE.SOURCE_HELPER_RELATIVE),
            helper_command["argv"],
        )
        self.assertEqual(len(helper_command["passed_file_descriptors"]), 1)
        self.assertEqual(identity["head_commit"], commit)


if __name__ == "__main__":
    unittest.main()
