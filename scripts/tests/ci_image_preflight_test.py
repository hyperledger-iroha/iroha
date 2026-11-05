"""CI image identity must be checked before any dependent container starts."""

from pathlib import Path
import os
import re
import shlex
import stat
import subprocess
import textwrap
import tomllib

import pytest


ROOT = Path(__file__).resolve().parents[2]
WORKFLOWS = ROOT / ".github/workflows"
IMAGE = "image: ${{ needs.ci_image.outputs.image }}"
COUNTS = {
    "nightly_sumeragi.yml": 10,
    "nightly_sumeragi_soak.yml": 2,
    "publish.yml": 1,
    "publish_custom.yml": 1,
    "publish_dev.yml": 1,
    "workspace_release.yml": 6,
}
HOSTED_NATIVE_JOBS = {
    "pr.yml": (
        "pre_build", "consistency", "kotodama_docs", "sumeragi",
        "zk_phase_tree_measurement", "x2_maximum_proof_transport",
        "norito_rpc_verify", "rust_affected", "rust_network",
        "sora_parliament_lifecycle",
    ),
    "openapi.yml": ("canonical-spec",),
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


def hosted_violations(source: str, names: tuple[str, ...]) -> list[str]:
    """Development validation must provision every formerly image-owned job."""
    errors = []
    blocks = jobs(source)
    if "ci_image" in blocks:
        errors.append("development checks still require a configured image")
    for name in names:
        block = blocks.get(name, "")
        if "runs-on: ubuntu-24.04" not in block:
            errors.append(f"{name} has no hosted Linux runner")
        if "container:" in block or "self-hosted" in block:
            errors.append(f"{name} retains external infrastructure")
        if "uses: ./.github/actions/setup-native-ci" not in block:
            errors.append(f"{name} has no native tool provisioning")
        elif "actions/setup-python@" in block.split("uses: ./.github/actions/setup-native-ci", 1)[1]:
            errors.append(f"{name} replaces the provisioned Python environment")
    return errors


@pytest.mark.parametrize("name,names", HOSTED_NATIVE_JOBS.items())
def test_development_jobs_run_without_image_or_private_runner_configuration(
    name: str, names: tuple[str, ...],
) -> None:
    source = (WORKFLOWS / name).read_text()
    assert hosted_violations(source, names) == []
    assert "needs.ci_image" not in source
    assert "vars.IROHA_CI_IMAGE" not in source
    for job, block in jobs(source).items():
        if job in names:
            changed = block.replace("      - uses: ./.github/actions/setup-native-ci\n", "")
            assert f"{job} has no native tool provisioning" in hosted_violations(
                source.replace(block, changed), names,
            )


@pytest.mark.parametrize("name,names", HOSTED_NATIVE_JOBS.items())
def test_native_jobs_retain_the_python_environment_with_script_requirements(
    name: str, names: tuple[str, ...],
) -> None:
    source = (WORKFLOWS / name).read_text()
    for job, block in jobs(source).items():
        if job in names:
            changed = block.replace(
                "      - uses: ./.github/actions/setup-native-ci\n",
                "      - uses: ./.github/actions/setup-native-ci\n"
                "      - uses: actions/setup-python@6d4fd22b20465ccc0f7354476588e8df7c9ca0e0\n"
                "        with:\n          python-version: \"3.11\"\n",
            )
            assert f"{job} replaces the provisioned Python environment" in hosted_violations(
                source.replace(block, changed), names,
            )


def test_openapi_validation_tracks_both_branches_and_native_setup_changes() -> None:
    source = (WORKFLOWS / "openapi.yml").read_text()
    triggers = source.split("\non:\n", 1)[1].split("\npermissions:\n", 1)[0]
    for event in ("pull_request", "push"):
        block = re.search(rf"^  {event}:\n.*?(?=^  \w+:\n|\Z)", triggers, re.M | re.S)
        assert block is not None
        assert "branches: [main, optimizations]" in block.group()
        assert '      - ".github/actions/**"' in block.group()
        assert '      - ".github/workflows/openapi.yml"' in block.group()


def test_python_static_analysis_uses_committed_locks_on_a_hosted_runner() -> None:
    source = (WORKFLOWS / "pr_python_static.yml").read_text()
    block = jobs(source)["check"]
    assert "runs-on: ubuntu-24.04" in block
    assert "ci_image" not in source
    assert "container:" not in block
    assert "actions/setup-python@" in block
    assert 'poetry-version: "2.5.1"' in block
    assert "poetry check --lock" in block
    assert "poetry install --no-root" in block
    assert "poetry lock\n" not in block
    for command in ("poetry run black --check", "poetry run mypy", "poetry run flake8"):
        assert command in block


def test_native_provisioning_uses_pinned_actions_and_declared_rust_toolchain() -> None:
    source = (ROOT / ".github/actions/setup-native-ci/action.yml").read_text()
    assert "sudo apt-get install" in source
    for package in ("clang", "cmake", "libssl-dev", "pkg-config", "protobuf-compiler", "mold"):
        assert package in source
    assert "toolchain: ${{ inputs.toolchain }}" in source
    assert 'default: ""' in source
    assert "clippy,rustfmt,llvm-tools-preview" in source
    for action in re.findall(r"uses: ([^\s]+)", source):
        assert re.fullmatch(r"[^@]+@[0-9a-f]{40}", action), action


def test_native_provisioning_installs_fontconfig_development_metadata(tmp_path: Path) -> None:
    """Execute the apt step without privilege and retain the GUI pkg-config prerequisite."""
    source = (ROOT / ".github/actions/setup-native-ci/action.yml").read_text()
    step = source.split("    - name: Install native build prerequisites\n", 1)[1].split("    - uses:", 1)[0]
    command = textwrap.dedent(step.split("      run: |\n", 1)[1])
    binaries = tmp_path / "bin"
    binaries.mkdir()
    sudo = binaries / "sudo"
    sudo.write_text('#!/bin/bash\nset -eu\nprintf "%s\\n" "$*" >> "$PROVISION_LOG"\n')
    sudo.chmod(0o700)
    log = tmp_path / "provision.log"
    result = subprocess.run(
        ["bash", "-eo", "pipefail", "-c", command],
        env={**os.environ, "PATH": f"{binaries}{os.pathsep}{os.environ['PATH']}", "PROVISION_LOG": str(log)},
        capture_output=True, text=True, check=False,
    )
    assert result.returncode == 0, result.stderr
    calls = [shlex.split(line) for line in log.read_text().splitlines()]
    assert len(calls) == 2 and calls[0] == ["apt-get", "update"]
    assert calls[1][:4] == ["apt-get", "install", "--no-install-recommends", "-y"]
    assert calls[1].count("libfontconfig-dev") == 1
    assert calls[1].count("pkg-config") == 1


def test_native_setup_installs_helpers_from_the_selected_checkout() -> None:
    source = (ROOT / ".github/actions/setup-native-ci/action.yml").read_text()
    inputs = source.split("inputs:\n", 1)[1].split("runs:\n", 1)[0]
    assert '  working-directory:\n' in inputs
    assert '    default: "."' in inputs
    step = source.split("    - name: Install repository Python build helpers\n", 1)[1].split("    - uses:", 1)[0]
    assert '      working-directory: ${{ inputs.working-directory }}' in step
    assert '      run: python -m pip install -r scripts/requirements.txt' in step
    assert '${{ inputs.working-directory }}' not in step.split('      run:', 1)[1]


@pytest.mark.parametrize("existing", ["absent", "private-file", "private-link", "tmp-file", "tmp-link"])
def test_native_setup_provisions_real_private_replay_roots(tmp_path: Path, existing: str) -> None:
    source = (ROOT / ".github/actions/setup-native-ci/action.yml").read_text()
    setup = source.split("    - name: Prepare sealed replay temporary root\n", 1)[1].split("    - name:", 1)[0]
    assert "      shell: bash\n" in setup
    command = textwrap.dedent(setup.split("      run: |\n", 1)[1])
    assert "sudo install -d -o root -g root -m 0755 /private\n" in command
    assert "sudo install -d -o root -g root -m 1777 /private/tmp\n" in command
    private = tmp_path / "private"
    outside = tmp_path / "outside"
    outside.mkdir(mode=0o700)
    if existing.startswith("tmp-"):
        private.mkdir(mode=0o755)
    selected = private / "tmp" if existing.startswith("tmp-") else private
    if existing.endswith("file"):
        selected.write_text("keep")
    elif existing.endswith("link"):
        selected.symlink_to(outside, target_is_directory=True)

    # Exercise the actual shell body without host privilege or changing /private.
    # The fake sudo verifies root ownership is requested; install still creates
    # real directories with the production modes under the test's private root.
    binaries = tmp_path / "bin"
    binaries.mkdir()
    sudo = binaries / "sudo"
    sudo.write_text(
        '#!/bin/bash\nset -eu\n'
        '[[ "$*" == "install -d -o root -g root -m "* ]]\n'
        'printf "%s\\n" "$*" >> "$PROVISION_LOG"\n'
        'exec install -d -m "$8" "$9"\n'
    )
    sudo.chmod(0o700)
    log = tmp_path / "provision.log"
    result = subprocess.run(
        ["bash", "-eo", "pipefail", "-c", command.replace("/private", shlex.quote(str(private)))],
        env={**os.environ, "PATH": f"{binaries}{os.pathsep}{os.environ['PATH']}", "PROVISION_LOG": str(log)},
        capture_output=True, text=True, check=False,
    )
    if existing != "absent":
        assert result.returncode != 0
        assert "not a real directory" in result.stderr
        assert not log.exists()
        assert stat.S_IMODE(outside.stat().st_mode) == 0o700
        if existing.endswith("file"):
            assert selected.read_text() == "keep"
        return

    assert result.returncode == 0, result.stderr
    assert stat.S_IMODE(private.stat().st_mode) == 0o755
    assert stat.S_IMODE((private / "tmp").stat().st_mode) == 0o1777
    run = subprocess.run(
        ["mktemp", "-d", str(private / "tmp" / "replay.XXXXXX")],
        capture_output=True, text=True, check=True,
    )
    assert stat.S_IMODE(Path(run.stdout.strip()).stat().st_mode) == 0o700


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
