#!/usr/bin/env python3
"""Mandatory Linux cosign qualification using independently reviewed release bytes."""

from __future__ import annotations

import hashlib
import os
from pathlib import Path
import platform
import re
import sys
import tempfile
import time
from urllib.parse import urlsplit
from urllib.request import HTTPRedirectHandler, build_opener

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
import sorafs_verifier_process as verifier_process
from sorafs_evidence_json import decode_evidence_json, load_evidence_json
from sorafs_path_identity import resolve_path_identity

POLICY = ROOT / "ci/sorafs_cosign_verifier.json"
CRYPTO_TEST = "scripts/tests/sorafs_final_promotion_cosign_crypto_test.py"
_WRONG_IDENTITY = (
    "https://github.com/sigstore-conformance/extremely-dangerous-public-oidc-beacon/"
    ".github/workflows/extremely-dangerous-oidc-beacon.yml@refs/heads/untrusted"
)
REQUIRED_CASES = frozenset((
    "test_public_upstream_crypto_fixture_integrity",
    "test_real_cosign_accepts_exact_subject_and_independent_trust",
    "test_real_cosign_rejects_different_subject",
    "test_real_cosign_rejects_wrong_independent_identity[provenance_certificate_identity-"
    + _WRONG_IDENTITY + "]",
    "test_real_cosign_rejects_wrong_independent_identity[provenance_oidc_issuer-https://accounts.google.com]",
    *(f"test_real_cosign_rejects_corrupted_cryptographic_evidence[{mutation}]" for mutation in (
        "signature", "certificate", "artifact_digest", "checkpoint", "inclusion_hash",
        "missing_inclusion_proof", "missing_transparency_log", "signed_timestamp",
        "missing_signed_timestamp", "timestamp_payload_mismatch",
        "proof_tree_size", "proof_log_index", "entry_log_index", "proof_hash",
        "checkpoint_tree_size", "checkpoint_root_hash", "body_digest", "body_signature",
        "body_certificate", "body_key_details", "body_noncanonical",
    )),
    *(f"test_real_cosign_rejects_missing_independent_authority[{authority}]" for authority in (
        "certificateAuthorities", "ctlogs", "tlogs", "timestampAuthorities",
    )),
))
MAX_BINARY_BYTES = 512 * 1024 * 1024
DOWNLOAD_TIMEOUT_SECS = 180
POLICY_FIELDS = frozenset((
    "schema", "release_tag", "source_commit", "platform", "asset_name", "asset_url",
    "asset_sha256", "asset_size", "verification",
))


def load_policy(path: Path) -> dict:
    """Load the checked-in release policy; no CLI/environment/remote policy override exists."""
    policy = load_evidence_json(path, 16 * 1024)
    if not isinstance(policy, dict) or set(policy) != POLICY_FIELDS:
        raise ValueError("invalid cosign release policy")
    expected_url = (
        "https://github.com/sigstore/cosign/releases/download/"
        f"{policy['release_tag']}/cosign-linux-amd64"
    )
    if (
        policy["schema"] != "sorafs.cosign_verifier_policy.v1"
        or not isinstance(policy["release_tag"], str)
        or re.fullmatch(r"v3\.[0-9]+\.[0-9]+", policy["release_tag"]) is None
        or not isinstance(policy["source_commit"], str)
        or re.fullmatch(r"[0-9a-f]{40}", policy["source_commit"]) is None
        or policy["platform"] != "linux/amd64"
        or policy["asset_name"] != "cosign-linux-amd64"
        or policy["asset_url"] != expected_url
        or not isinstance(policy["asset_sha256"], str)
        or re.fullmatch(r"[0-9a-f]{64}", policy["asset_sha256"]) is None
        or policy["asset_sha256"] == "0" * 64
        or type(policy["asset_size"]) is not int
        or not 0 < policy["asset_size"] <= MAX_BINARY_BYTES
        or not isinstance(policy["verification"], dict)
        or any(policy["verification"].get(key) is not True for key in (
            "keyless_binary_verified", "kms_binary_verified", "keyless_checksums_verified"
        ))
    ):
        raise ValueError("invalid cosign release policy")
    return policy


class ReleaseRedirects(HTTPRedirectHandler):
    """Allow only HTTPS redirects to GitHub's public release-asset hosts."""

    def redirect_request(self, request, fp, code, message, headers, newurl):
        parsed = urlsplit(newurl)
        if (
            parsed.scheme != "https" or parsed.username or parsed.password
            or parsed.port not in (None, 443)
            or parsed.hostname not in (
                "github.com", "release-assets.githubusercontent.com",
                "objects.githubusercontent.com",
            )
        ):
            raise ValueError("cosign release download redirect rejected")
        return super().redirect_request(request, fp, code, message, headers, newurl)


def download_release(policy: dict, destination: Path) -> None:
    """Stream one exact pinned asset; mismatched/truncated/excess bytes never become executable."""
    started = time.monotonic()
    digest = hashlib.sha256()
    total = 0
    with build_opener(ReleaseRedirects()).open(policy["asset_url"], timeout=30) as response:
        if response.status != 200:
            raise ValueError("cosign release download rejected")
        with destination.open("xb") as output:
            os.fchmod(output.fileno(), 0o600)
            while True:
                if time.monotonic() - started > DOWNLOAD_TIMEOUT_SECS:
                    raise ValueError("cosign release download timed out")
                chunk = response.read(min(1024 * 1024, policy["asset_size"] - total + 1))
                if not chunk:
                    break
                total += len(chunk)
                if total > policy["asset_size"]:
                    raise ValueError("cosign release download size mismatch")
                digest.update(chunk)
                output.write(chunk)
            if total != policy["asset_size"] or digest.hexdigest() != policy["asset_sha256"]:
                raise ValueError("cosign release download pin mismatch")
            output.flush()
            os.fsync(output.fileno())
            os.fchmod(output.fileno(), 0o500)
            os.fsync(output.fileno())


class QualificationResults:
    """Require executed successes; skipped cryptographic cases never qualify a release."""

    def __init__(self):
        self.passed = set()
        self.collected = []
        self.skipped = 0

    def pytest_collection_finish(self, session):
        self.collected = [item.name for item in session.items]

    def pytest_runtest_logreport(self, report):
        self.skipped += int(report.skipped)
        if report.when == "call" and report.passed:
            self.passed.add(report.nodeid.split("::")[-1])


def run_qualification(executable: Path, policy: dict, private_root: Path) -> None:
    """Verify version/source identity and execute every real cryptographic test with explicit pins."""
    import pytest

    version = decode_evidence_json(verifier_process.run_verifier(
        [str(executable), "version", "--json"], private_root,
        max_stdout_bytes=4096, expected_stderr=b"",
    ))
    if any(version.get(key) != value for key, value in (
        ("gitVersion", policy["release_tag"]),
        ("gitCommit", policy["source_commit"]), ("platform", policy["platform"]),
    )):
        raise ValueError("cosign release executable identity mismatch")
    results = QualificationResults()
    code = pytest.main([
        "-q", str(ROOT / CRYPTO_TEST),
        "--sorafs-cosign-verifier", str(executable),
        "--sorafs-cosign-verifier-sha256", policy["asset_sha256"],
    ], plugins=[results])
    if (code != 0 or results.skipped or results.passed != REQUIRED_CASES
        or set(results.collected) != REQUIRED_CASES
        or len(results.collected) != len(REQUIRED_CASES)):
        raise ValueError("cosign cryptographic qualification incomplete")


def main() -> int:
    """Fail closed outside the reviewed Linux platform or when any qualification step fails."""
    if len(sys.argv) != 1 or platform.system() != "Linux" or platform.machine() != "x86_64":
        print("cosign release qualification requires its reviewed Linux amd64 runner", file=sys.stderr)
        return 1
    try:
        policy = load_policy(POLICY)
        with tempfile.TemporaryDirectory(prefix="sorafs-cosign-qualification-") as temporary:
            errors: list[str] = []
            private_root = resolve_path_identity(Path(temporary), errors, label="private cosign staging")
            if private_root is None or errors:
                raise ValueError("cosign private staging unavailable")
            source = private_root / "download"
            executable = private_root / "cosign"
            download_release(policy, source)
            verifier_process.snapshot_executable(source, executable, policy["asset_sha256"])
            run_qualification(executable, policy, private_root)
    except (OSError, ValueError, TypeError, KeyError, RuntimeError):
        print("cosign release cryptographic qualification failed", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
