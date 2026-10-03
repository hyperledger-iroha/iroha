"""Input-binding controls only; these fixtures provide no CUDA or image attestation."""

import hashlib
import json
from pathlib import Path

import pytest

from scripts import check_ivm_cuda_release_image as gate


@pytest.fixture
def inputs(tmp_path):
    bundle = tmp_path / "bundle"
    bundle.mkdir()
    public = b"p" * 32
    key = hashlib.sha256(public).hexdigest()
    image = hashlib.sha256(b"reviewed image").hexdigest()
    reference = f"registry.example/cuda@sha256:{image}"
    (bundle / "provenance.v1.pub").write_bytes(public)
    (bundle / "provenance.v1").write_text(f"ivm-cuda-ptx-provenance-v1\ncuda_image_sha256={image}\n")
    inspection = tmp_path / "inspect.json"
    inspection.write_text(json.dumps({"RepoDigests": [reference], "Id": "sha256:" + "b" * 64,
                                      "Os": "linux", "Architecture": "amd64"}))
    return reference, key, image, inspection, bundle, tmp_path / "receipt.json"


def test_reviewed_inputs_bind_exact_measured_image_and_source_claim(inputs):
    receipt = gate.verify(*inputs)
    result = json.loads(receipt.read_bytes())
    assert result["image_reference"] == inputs[0]
    assert result["trusted_key_sha256"] == inputs[1]
    assert result["image_config_digest"] == "sha256:" + "b" * 64
    assert result["signature_verified"] is False and result["hardware_qualified"] is False
    assert result["source_manifest_sha256"] == hashlib.sha256((inputs[4] / "provenance.v1").read_bytes()).hexdigest()
    assert receipt.stat().st_mode & 0o777 == 0o600
    with pytest.raises(gate.custody.ReleaseArtifactError):
        gate.verify(*inputs)


@pytest.mark.parametrize("field,value", [(0, "registry.example/cuda:latest"), (1, ""), (1, "A" * 64),
                                        (1, "0" * 64), (2, ""), (2, "0" * 64), (2, "c" * 64)])
def test_no_ambient_or_self_endorsed_fallback_for_missing_inputs(inputs, field, value, monkeypatch):
    monkeypatch.setenv("IVM_CUDA_TRUSTED_KEY_SHA256", inputs[1])
    changed = list(inputs)
    changed[field] = value
    with pytest.raises(gate.custody.ReleaseArtifactError):
        gate.verify(*changed)
    assert not inputs[-1].exists()


@pytest.mark.parametrize("field,value", [("RepoDigests", []), ("RepoDigests", ["sha256:" + "a" * 64]),
                                        ("Id", "sha256:" + "0" * 64), ("Os", "windows"), ("Architecture", "unknown")])
def test_actual_inspection_must_match_pinned_linux_image(inputs, field, value):
    inspection = json.loads(inputs[3].read_bytes())
    inspection[field] = value
    inputs[3].write_text(json.dumps(inspection))
    with pytest.raises(gate.custody.ReleaseArtifactError, match="inspection"):
        gate.verify(*inputs)
    assert not inputs[-1].exists()


@pytest.mark.parametrize("field", ["image", "key", "duplicate"])
def test_changed_source_claim_or_ambiguous_inspection_never_publishes(inputs, field):
    if field == "image":
        path = inputs[4] / "provenance.v1"
        path.write_text(path.read_text().replace(inputs[2], "d" * 64))
    elif field == "key":
        (inputs[4] / "provenance.v1.pub").write_bytes(b"x" * 32)
    else:
        inputs[3].write_text('{"Id":"a","Id":"b"}')
    with pytest.raises(gate.custody.ReleaseArtifactError):
        gate.verify(*inputs)
    assert not inputs[-1].exists()


def test_ci_gate_uses_explicit_reviewed_values_and_executes_inspected_config_identity():
    source = (Path(__file__).resolve().parents[2] / ".github/workflows/nightly_cuda.yml").read_text()
    step = source.split("- name: IVM checked-in PTX identity and bundled hardware qualification", 1)[1]
    for name in ("IVM_CUDA_QUALIFICATION_IMAGE", "IVM_CUDA_IMAGE_SHA256", "IVM_CUDA_TRUSTED_KEY_SHA256"):
        assert "vars." + name in step
    assert step.index("configured_image(") < step.index("docker pull")
    assert step.index("check_ivm_cuda_release_image.py") < step.index("docker run")
    assert '"$image_id" -euc' in step
    assert 'IVM_CUDA_TRUSTED_KEY_SHA256=${CUDA_REVIEWED_KEY_SHA256}' in step
