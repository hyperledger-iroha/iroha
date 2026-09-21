"""Compose original inert runtime custody, bundle parsing and final publication.

These controls never execute a fixture interpreter, install a wheel, fabricate a
successful producer candidate or claim native SDK qualification. They reuse the
maintained runtime fixture and exercise the actual descriptor/parser owners.
"""
from __future__ import annotations

from contextlib import ExitStack
from dataclasses import asdict
import hashlib
import importlib.util
import io
from pathlib import Path
import sys

import pytest

ROOT = next(parent for parent in Path(__file__).resolve().parents
            if (parent / "scripts/sorafs_python_runtime_inputs.py").is_file())
sys.path.insert(0, str(ROOT / "scripts"))
from sorafs_python_consumer_artifact import ArtifactError, canonical_json
from sorafs_python_archive import execution_archive
from sorafs_python_publication import PythonArtifactPublication
from sorafs_python_runtime_custody import OriginalPythonRuntime
from sorafs_python_runtime_inputs import parse_runtime_bundle

SPEC = importlib.util.spec_from_file_location(
    "_original_runtime_fixture", ROOT / "scripts/tests/sorafs_python_runtime_inputs_test.py")
FIXTURE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(FIXTURE)


def _runtime(tmp_path, lifetime):
    manifest = FIXTURE.parsed(FIXTURE.fixture(tmp_path, zip_present=True, alias=True))
    owner = OriginalPythonRuntime(manifest)
    lifetime.callback(owner.close)
    owner.__enter__()
    return manifest, owner


def _work(tmp_path):
    work = tmp_path / "outputs"
    work.mkdir()
    return work


def _no_finals(work):
    assert not (work / "python-runtime-inputs.bundle").exists()
    assert not (work / "python-consumer.zip").exists()


def test_original_stream_readback_and_pinned_parser_join_before_publication(tmp_path):
    with ExitStack() as lifetime:
        manifest, owner = _runtime(tmp_path, lifetime)
        work = _work(tmp_path)
        publication = lifetime.enter_context(PythonArtifactPublication(work))
        with publication.runtime_stream() as stream:
            digest = owner.write_bundle(stream)
        raw = publication.read_runtime_bundle(expected_sha256=digest.sha256, expected_size=digest.size)
        parsed = parse_runtime_bundle(raw, expected_manifest_sha256=manifest.sha256)
        assert parsed.manifest == manifest
        for reference in manifest.files():
            assert parsed.member_bytes(reference.path) == Path(reference.path).read_bytes()
        archive = execution_archive({"fixture-runtime-join.json": canonical_json({
            "runtime_manifest": manifest.sha256, "runtime_bundle": asdict(digest)})})
        publication.stage_execution_archive(archive)
        calls = []

        def original_check():
            _no_finals(work)
            assert owner._active and owner._parents
            owner.recheck()
            calls.append("checked")

        result = publication.publish(check_originals=original_check)
        assert calls == ["checked"]
        assert result.runtime_bundle.path.read_bytes() == raw
        assert (result.runtime_bundle.sha256, result.runtime_bundle.size) == (digest.sha256, digest.size)
        assert result.execution_archive.path.read_bytes() == archive
        assert owner._active
    assert not owner._active and not owner._parents


@pytest.mark.parametrize("mutation", ("pin", "member", "trailing"))
def test_descriptor_readback_cannot_replace_semantic_runtime_bundle_checks(tmp_path, mutation):
    with ExitStack() as lifetime:
        manifest, owner = _runtime(tmp_path, lifetime)
        original = io.BytesIO()
        owner.write_bundle(original)
        raw = original.getvalue()
        expected_pin = manifest.sha256
        if mutation == "pin":
            expected_pin = "a" * 64
        elif mutation == "member":
            raw = raw[:-1] + bytes((raw[-1] ^ 1,))
        else:
            raw += b"unowned trailing bytes"
        work = _work(tmp_path)
        publication = lifetime.enter_context(PythonArtifactPublication(work))
        with publication.runtime_stream() as stream:
            stream.write(raw)
        captured = publication.read_runtime_bundle(
            expected_sha256=hashlib.sha256(raw).hexdigest(), expected_size=len(raw))
        assert captured == raw
        with pytest.raises(ArtifactError):
            parse_runtime_bundle(captured, expected_manifest_sha256=expected_pin)
        _no_finals(work)
        assert publication.archive is None
        assert (work / ".python-runtime-inputs.bundle.pending").read_bytes() == raw


def test_final_real_runtime_recheck_refuses_after_both_artifacts_are_staged(tmp_path):
    with ExitStack() as lifetime:
        manifest, owner = _runtime(tmp_path, lifetime)
        work = _work(tmp_path)
        publication = lifetime.enter_context(PythonArtifactPublication(work))
        with publication.runtime_stream() as stream:
            digest = owner.write_bundle(stream)
        raw = publication.read_runtime_bundle(expected_sha256=digest.sha256, expected_size=digest.size)
        assert parse_runtime_bundle(raw, expected_manifest_sha256=manifest.sha256).manifest == manifest
        publication.stage_execution_archive(execution_archive({"original-runtime.json": manifest.raw}))
        path = Path(manifest.stdlib_root) / "os.py"
        path.write_bytes(b"# OS\n")
        with pytest.raises(ArtifactError, match="independent pin"):
            publication.publish(check_originals=owner.recheck)
        _no_finals(work)
        assert {path.name for path in work.glob(".*.pending")} == {
            ".python-runtime-inputs.bundle.pending", ".python-consumer.zip.pending"}
    assert not owner._active and not owner._parents
