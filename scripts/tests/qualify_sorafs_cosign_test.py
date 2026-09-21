"""Fail-closed download, pin and mandatory-case tests for Linux cosign qualification."""

from __future__ import annotations

import hashlib
import io
import json
from pathlib import Path
import stat
from types import SimpleNamespace
from urllib.request import Request

import pytest

from ci import qualify_sorafs_cosign as module


def policy_for(payload: bytes) -> dict:
    policy = module.load_policy(module.POLICY)
    policy.update(asset_sha256=hashlib.sha256(payload).hexdigest(), asset_size=len(payload))
    return policy


def test_repository_policy_pins_authenticated_official_release():
    policy = module.load_policy(module.POLICY)
    assert policy["release_tag"] == "v3.1.3"
    assert policy["source_commit"] == "11926fa5bbbbde47e88fc006b625a17769b743b2"
    assert policy["asset_sha256"] == "4629c757b7618056f8ddd7e2625ae9fdd94c0372a65049520bc7d9df9efc7f71"
    assert policy["asset_size"] == 141178250
    assert len(module.REQUIRED_CASES) == 30


@pytest.mark.parametrize("field,value", [
    ("schema", "other"), ("release_tag", "latest"), ("source_commit", "main"),
    ("platform", "darwin/arm64"), ("asset_name", "other"),
    ("asset_url", "https://untrusted.invalid/cosign"), ("asset_sha256", "0" * 64),
    ("asset_size", True), ("asset_size", module.MAX_BINARY_BYTES + 1),
    ("verification", {}), ("unexpected", "field"),
])
def test_policy_rejects_unreviewed_or_malformed_selection(tmp_path, field, value):
    policy = module.load_policy(module.POLICY)
    policy[field] = value
    path = tmp_path / "policy.json"
    path.write_text(json.dumps(policy))
    with pytest.raises(ValueError):
        module.load_policy(path)


@pytest.mark.parametrize("mutation", ["duplicate", "oversize", "symlink"])
def test_policy_uses_shared_bounded_strict_reader(tmp_path, mutation):
    path = tmp_path / "policy.json"
    raw = module.POLICY.read_bytes()
    if mutation == "duplicate":
        path.write_bytes(raw.replace(b'{', b'{"schema":"other",', 1))
    elif mutation == "oversize":
        path.write_bytes(raw + b" " * (16 * 1024))
    else:
        path.symlink_to(module.POLICY)
    with pytest.raises((ValueError, OSError)):
        module.load_policy(path)


class Response(io.BytesIO):
    status = 200


def fake_download(monkeypatch, payload, status=200):
    def opened(url, timeout):
        assert url.startswith("https://github.com/sigstore/cosign/releases/download/")
        assert timeout == 30
        response = Response(payload)
        response.status = status
        return response
    monkeypatch.setattr(module, "build_opener", lambda handler: SimpleNamespace(open=opened))


def test_download_pins_exact_bytes_before_executable_mode(tmp_path, monkeypatch):
    payload = b"reviewed public binary"
    fake_download(monkeypatch, payload)
    destination = tmp_path / "binary"
    module.download_release(policy_for(payload), destination)
    assert destination.read_bytes() == payload
    assert stat.S_IMODE(destination.stat().st_mode) == 0o500


@pytest.mark.parametrize("mutation", ["truncated", "overflow", "wrong_digest", "status", "timeout"])
def test_failed_download_never_becomes_executable(tmp_path, monkeypatch, mutation):
    payload = b"reviewed public binary"
    received = payload[:-1] if mutation == "truncated" else payload
    if mutation == "overflow":
        received += b"x"
    elif mutation == "wrong_digest":
        received = b"x" * len(payload)
    fake_download(monkeypatch, received, 503 if mutation == "status" else 200)
    if mutation == "timeout":
        ticks = iter([0, module.DOWNLOAD_TIMEOUT_SECS + 1])
        monkeypatch.setattr(module.time, "monotonic", lambda: next(ticks))
    destination = tmp_path / "binary"
    with pytest.raises(ValueError):
        module.download_release(policy_for(payload), destination)
    assert not destination.exists() or not (destination.stat().st_mode & 0o111)


@pytest.mark.parametrize("url", [
    "http://github.com/asset", "https://untrusted.invalid/asset",
    "https://user:password@github.com/asset", "https://github.com:444/asset",
])
def test_download_rejects_untrusted_redirect(url):
    with pytest.raises(ValueError):
        module.ReleaseRedirects().redirect_request(
            Request("https://github.com/asset"), None, 302, "redirect", {}, url,
        )


def test_download_accepts_https_release_asset_redirect():
    target = "https://release-assets.githubusercontent.com/asset?public-expiring-signature=value"
    request = module.ReleaseRedirects().redirect_request(
        Request("https://github.com/asset"), None, 302, "redirect", {}, target,
    )
    assert request.full_url == target


@pytest.mark.parametrize("mutation", ["none", "skip", "omit", "duplicate", "exit", "version"])
def test_qualification_requires_every_actual_case_and_exact_version(tmp_path, monkeypatch, mutation):
    policy = module.load_policy(module.POLICY)
    executable = tmp_path / "cosign"
    version = {"gitVersion": policy["release_tag"], "gitCommit": policy["source_commit"], "platform": policy["platform"]}
    if mutation == "version":
        version["gitCommit"] = "0" * 40
    def run(command, root, *, max_stdout_bytes, expected_stderr):
        assert command == [str(executable), "version", "--json"]
        assert root == tmp_path and max_stdout_bytes == 4096 and expected_stderr == b""
        return json.dumps(version).encode()
    monkeypatch.setattr(module.verifier_process, "run_verifier", run)
    def execute(args, *, plugins):
        assert args[-4:] == ["--sorafs-cosign-verifier", str(executable), "--sorafs-cosign-verifier-sha256", policy["asset_sha256"]]
        results = plugins[0]
        cases = sorted(module.REQUIRED_CASES)
        if mutation == "omit":
            cases.pop()
        if mutation == "duplicate":
            cases.append(cases[-1])
        results.pytest_collection_finish(SimpleNamespace(items=[SimpleNamespace(name=name) for name in cases]))
        for index, name in enumerate(cases):
            skipped = mutation == "skip" and index == 0
            results.pytest_runtest_logreport(SimpleNamespace(
                nodeid="crypto.py::" + name, when="call", passed=not skipped, skipped=skipped,
            ))
        return 1 if mutation == "exit" else 0
    monkeypatch.setattr(pytest, "main", execute)
    if mutation == "none":
        module.run_qualification(executable, policy, tmp_path)
    else:
        with pytest.raises(ValueError):
            module.run_qualification(executable, policy, tmp_path)


def test_main_rejects_platform_or_argument_override_before_network(monkeypatch):
    monkeypatch.setattr(module.sys, "argv", ["qualification", "--skip"])
    monkeypatch.setattr(module, "load_policy", lambda path: pytest.fail("must reject before loading"))
    assert module.main() == 1
    monkeypatch.setattr(module.sys, "argv", ["qualification"])
    monkeypatch.setattr(module.platform, "system", lambda: "Darwin")
    assert module.main() == 1


def test_main_downloads_snapshots_then_qualifies_exact_pin(tmp_path, monkeypatch):
    policy = policy_for(b"unit test bytes")
    monkeypatch.setattr(module.sys, "argv", ["qualification"])
    monkeypatch.setattr(module.platform, "system", lambda: "Linux")
    monkeypatch.setattr(module.platform, "machine", lambda: "x86_64")
    monkeypatch.setattr(module, "load_policy", lambda path: policy)
    fake_download(monkeypatch, b"unit test bytes")
    calls = []
    def qualify(executable, received, private_root):
        assert executable.read_bytes() == b"unit test bytes"
        assert stat.S_IMODE(executable.stat().st_mode) == 0o500
        assert received is policy and executable.parent == private_root
        calls.append("qualified")
    monkeypatch.setattr(module, "run_qualification", qualify)
    assert module.main() == 0 and calls == ["qualified"]
