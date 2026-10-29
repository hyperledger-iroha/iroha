"""CI image identity must be checked before any dependent container starts."""

from pathlib import Path
import os
import re
import subprocess
import tomllib

import pytest


ROOT = Path(__file__).resolve().parents[2]
WORKFLOWS = ROOT / ".github/workflows"
IMAGE = "image: ${{ needs.ci_image.outputs.image }}"
COUNTS = {
    "nightly_sumeragi.yml": 10,
    "nightly_sumeragi_soak.yml": 2,
    "openapi.yml": 1,
    "pr.yml": 10,
    "pr_python_static.yml": 1,
    "publish.yml": 1,
    "publish_custom.yml": 1,
    "publish_dev.yml": 1,
    "workspace_release.yml": 6,
}


def jobs(source: str) -> dict[str, str]:
    """Extract top-level job blocks without interpreting shell expressions."""
    body = source.split("\njobs:\n", 1)[1]
    return {
        block.split(":", 1)[0].strip(): block
        for block in re.split(r"(?=^  [\w-]+:\n)", body, flags=re.M)
        if re.match(r"^  [\w-]+:\n", block)
    }


def violations(source: str) -> list[str]:
    """Reject a selected CI container without its validated-output dependency."""
    errors = []
    blocks = jobs(source)
    if "uses: ./.github/workflows/ci_image_preflight.yml" not in blocks.get("ci_image", ""):
        errors.append("missing hosted preflight")
    for name, block in blocks.items():
        if IMAGE not in block:
            continue
        needs = re.findall(r"^    needs: (.+)$", block, flags=re.M)
        if len(needs) != 1 or "ci_image" not in [item.strip() for item in needs[0].strip("[]").split(",")]:
            errors.append(f"{name} is not gated by preflight")
    return errors


@pytest.mark.parametrize("name,count", COUNTS.items())
def test_all_ci_consumers_are_gated(name: str, count: int) -> None:
    source = (WORKFLOWS / name).read_text()
    assert source.count(IMAGE) == count
    assert violations(source) == []
    # An output expression alone cannot substitute for the dependency edge.
    for job, block in jobs(source).items():
        if IMAGE in block:
            changed = re.sub(r"^    needs: .+\n", "", block, flags=re.M)
            assert f"{job} is not gated by preflight" in violations(source.replace(block, changed))


def test_no_workflow_starts_from_an_unchecked_ci_variable_or_old_tag() -> None:
    for path in WORKFLOWS.glob("*.yml"):
        source = path.read_text()
        assert "image: ${{ vars.IROHA_CI_IMAGE }}" not in source, path
        assert "hyperledger/iroha2-ci:nightly-2025-05-08" not in source, path


@pytest.mark.parametrize("reference,valid", [
    ("registry.example/ci@sha256:" + "a" * 64, True),
    ("", False),
    ("registry.example/ci:latest", False),
    ("registry.example/ci@sha256:" + "A" * 64, False),
    ("registry.example/ci@sha256:" + "a" * 64 + "\nimage=evil", False),
    ("registry.example/ci;echo injected", False),
])
def test_preflight_emits_only_a_validated_digest(tmp_path: Path, reference: str, valid: bool) -> None:
    source = (WORKFLOWS / "ci_image_preflight.yml").read_text()
    assert "runs-on: ubuntu-latest" in source
    assert "container:" not in source
    assert "shell: bash" in source
    assert "value: ${{ jobs.validate.outputs.image }}" in source
    assert "image: ${{ steps.validate.outputs.image }}" in source
    command = "\n".join(line[10:] for line in source.split("        run: |\n", 1)[1].splitlines())
    output = tmp_path / "github-output"
    result = subprocess.run(
        ["bash", "-eo", "pipefail", "-c", command], cwd=ROOT,
        env={**os.environ, "IROHA_CI_IMAGE": reference, "GITHUB_OUTPUT": str(output)},
        capture_output=True, text=True, check=False,
    )
    if valid:
        assert result.returncode == 0, result.stderr
        assert output.read_text() == f"image={reference}\n"
    else:
        assert result.returncode != 0
        assert not output.exists()


def test_image_recipe_uses_current_pinned_tools_without_a_remote_installer() -> None:
    source = (ROOT / "Dockerfile.build").read_text()
    toolchain = tomllib.loads((ROOT / "rust-toolchain.toml").read_text())["toolchain"]["channel"]
    assert f"RUSTUP_TOOLCHAIN={toolchain}" in source
    assert "--toolchain stable" not in source
    assert "cargo install grcov" not in source
    assert "install.python-poetry.org" not in source
    assert "releases/download/v0.10.8/" in source
    assert "sha256sum --check --strict" in source
    assert "--only-binary=:all: --require-hashes -r /tmp/ci_image_poetry.lock" in source
    assert "sys.version_info[:2] == (3, 14)" in source
    poetry_lock = (ROOT / "scripts/ci_image_poetry.lock").read_text()
    assert "poetry==2.5.1" in poetry_lock
    assert "--python-platform linux" in poetry_lock
    assert "httpx2-jsfetch" not in poetry_lock
    assert "emscripten" not in poetry_lock
    publish = (WORKFLOWS / "ci_image.yml").read_text()
    assert "tags: hyperledger/iroha2-ci:sha-${{ github.sha }}" in publish
    assert "provenance: mode=max" in publish
    assert "sbom: true" in publish
    assert "${{ steps.build.outputs.digest }}" in publish
