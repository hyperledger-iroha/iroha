"""Candidate-specific applicability refuses changes to compiled or installed evidence."""

import importlib.util
import json
from pathlib import Path
import subprocess
import sys

import pytest

SCRIPT = Path(__file__).resolve().parents[1] / "ci_image_vex.py"
SPEC = importlib.util.spec_from_file_location("ci_image_vex", SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)
IMAGE = "hyperledger/iroha2-ci@sha256:" + "a" * 64
PACKAGES = "pam 1.7.3-1\nlibxml2 2.15.4-1\n"
IMPORTS = "github.com/docker/docker/pkg/namesgenerator\ngithub.com/docker/buildx/cmd/buildx\n"
BUILD = "docker-buildx: go1.27.3\n" + "".join(
    f"\tdep\t{name}\t{version}\th1:fixture\n" for name, version in (
        ("github.com/docker/docker", "v28.5.2+incompatible"),
        ("golang.org/x/net", "v0.60.0"),
        ("github.com/moby/go-archive", "v0.3.0"),
    )
)


def test_document_is_limited_to_exact_candidate_and_components():
    result = MODULE.document(IMAGE, PACKAGES, IMPORTS, BUILD)
    assert {s["vulnerability"]["name"] for s in result["statements"]} == {
        "AVG-2901", "AVG-2898", "GO-2026-4887",
    }
    for statement in result["statements"]:
        assert len(statement["products"]) == 1
        assert "@sha256:" + "a" * 64 in statement["products"][0]["@id"]
        assert "repository_url=docker.io%2Fhyperledger%2Firoha2-ci" in statement["products"][0]["@id"]
        assert len(statement["products"][0]["subcomponents"]) == 1
        assert "https://" in statement["status_notes"]
    authz = result["statements"][2]
    assert authz["status"] == "not_affected"
    assert authz["justification"] == "vulnerable_code_not_present"


@pytest.mark.parametrize("index,replacement", [
    (0, "hyperledger/iroha2-ci:latest"),
    (0, "unreviewed/ci@sha256:" + "a" * 64),
    (1, PACKAGES.replace("1.7.3-1", "1.7.0-2")),
    (1, PACKAGES.replace("2.15.4-1", "2.14.4-1")),
    (1, PACKAGES + "pam 1.7.0-2\n"),
    (1, PACKAGES + "containerd 2.4.2-1\n"),
    (1, PACKAGES + "docker 29.9.2-1\n"),
    (1, PACKAGES + "docker-buildx 0.38.2-1\n"),
    (2, IMPORTS + "github.com/docker/docker/pkg/authorization\n"),
    (2, IMPORTS + "github.com/moby/moby/v2/daemon/server\n"),
    (2, ""),
    (3, BUILD.replace("v0.60.0", "v0.59.0")),
    (3, BUILD.replace("v0.3.0", "v0.2.0")),
    (3, BUILD.replace("v28.5.2+incompatible", "v28.5.1+incompatible")),
    (3, BUILD.replace("go1.27.3", "go1.27.1")),
])
def test_changed_evidence_cannot_acquire_an_applicability_statement(index, replacement):
    values = [IMAGE, PACKAGES, IMPORTS, BUILD]
    values[index] = replacement
    with pytest.raises(ValueError):
        MODULE.document(*values)


def test_cli_reads_actual_files_and_leaves_no_document_on_refusal(tmp_path):
    for name, value in zip(
        ("arch-packages.txt", "buildx-packages.txt", "buildx-build-info.txt"),
        (PACKAGES, IMPORTS, BUILD),
    ):
        (tmp_path / name).write_text(value)
    output = tmp_path / "vex.json"
    command = [sys.executable, "-I", "-S", str(SCRIPT), "--image", IMAGE,
               "--evidence", str(tmp_path), "--output", str(output)]
    assert subprocess.run(command, capture_output=True).returncode == 0
    assert json.loads(output.read_text())["version"] == 1
    output.unlink()
    (tmp_path / "buildx-packages.txt").write_text(IMPORTS + "github.com/docker/docker/pkg/authorization\n")
    assert subprocess.run(command, capture_output=True).returncode != 0
    assert not output.exists()
