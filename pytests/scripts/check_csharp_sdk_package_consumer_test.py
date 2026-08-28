"""Tests for the C# SDK package-consumer smoke guard."""

from __future__ import annotations

import os
import re
import subprocess
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "ci" / "check_csharp_sdk_package_consumer.sh"
WORKFLOW = ROOT / ".github" / "workflows" / "pr_csharp.yml"
README = ROOT / "csharp" / "README.md"

WORKFLOW_PATH_TRIGGER = '      - "ci/check_csharp_sdk_package_consumer.sh"'
WORKFLOW_RESTORE = "run: dotnet restore Hyperledger.Iroha.Sdk.sln"
WORKFLOW_BUILD = (
    "run: dotnet build Hyperledger.Iroha.Sdk.sln -c Release --no-restore "
    "-warnaserror"
)
WORKFLOW_TEST = "run: dotnet test Hyperledger.Iroha.Sdk.sln -c Release --no-build"
WORKFLOW_PACK = (
    "run: dotnet pack src/Hyperledger.Iroha.Sdk/Hyperledger.Iroha.Sdk.csproj "
    "-c Release --no-build --output artifacts/packages"
)
WORKFLOW_CONSUMER = "run: ../ci/check_csharp_sdk_package_consumer.sh"


def _replace_once(text: str, old: str, new: str) -> str:
    assert old in text
    return text.replace(old, new, 1)


def _bash_function_source(name: str) -> str:
    script = SCRIPT.read_text(encoding="utf-8")
    match = re.search(
        rf"(?ms)^{re.escape(name)}\(\) \{{\n.*?^\}}\n",
        script,
    )
    assert match is not None, f"missing Bash function: {name}"
    return match.group(0)


def _validate_pr_csharp_package_consumer_workflow(workflow: str) -> list[str]:
    errors: list[str] = []
    required_markers = (
        (WORKFLOW_PATH_TRIGGER, "workflow path trigger must include consumer guard"),
        (WORKFLOW_RESTORE, "workflow must restore the C# SDK solution"),
        (
            WORKFLOW_BUILD,
            "workflow must build the C# SDK solution with warnings as errors",
        ),
        (WORKFLOW_TEST, "workflow must test the C# SDK solution before packaging"),
        (
            WORKFLOW_PACK,
            "workflow must pack the C# SDK into csharp/artifacts/packages",
        ),
        (
            WORKFLOW_CONSUMER,
            "workflow must run the package-consumer smoke after packing",
        ),
    )
    positions: dict[str, int] = {}
    for marker, error in required_markers:
        position = workflow.find(marker)
        if position < 0:
            errors.append(error)
        positions[marker] = position

    ordered_markers = (
        WORKFLOW_RESTORE,
        WORKFLOW_BUILD,
        WORKFLOW_TEST,
        WORKFLOW_PACK,
        WORKFLOW_CONSUMER,
    )
    if all(positions[marker] >= 0 for marker in ordered_markers):
        ordered_positions = [positions[marker] for marker in ordered_markers]
        if ordered_positions != sorted(ordered_positions):
            errors.append(
                "workflow must run restore, build, test, pack, and consumer smoke in order"
            )
    return errors


def test_csharp_sdk_package_consumer_script_is_listable() -> None:
    """The package-consumer smoke guard must stay syntactically valid."""

    subprocess.run(["bash", "-n", str(SCRIPT)], check=True)
    completed = subprocess.run(
        ["bash", str(SCRIPT), "--list-negative-controls"],
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.stdout.splitlines() == [
        "--negative-control-missing-local-package",
        "--negative-control-project-reference",
        "--negative-control-managed-smoke",
    ]


def test_csharp_sdk_package_consumer_script_pins_real_package_consumption() -> None:
    """The smoke must prove NuGet package consumption rather than project refs."""

    script = SCRIPT.read_text(encoding="utf-8")

    required_markers = (
        "Hyperledger.Iroha.Sdk.${package_version}.nupkg",
        "Hyperledger.Iroha.Sdk.${package_version}.symbols.nupkg",
        "DOTNET_NOLOGO=\"${DOTNET_NOLOGO:-1}\"",
        "DOTNET_SKIP_FIRST_TIME_EXPERIENCE=\"${DOTNET_SKIP_FIRST_TIME_EXPERIENCE:-1}\"",
        "DOTNET_GLOBAL_JSON=\"${CSHARP_SDK_PACKAGE_CONSUMER_GLOBAL_JSON:-${ROOT_DIR}/csharp/global.json}\"",
        "stage_pinned_dotnet_global_json()",
        'run_dotnet "${app_dir}" new console --framework net8.0 --output . --no-restore',
        'run_dotnet "${app_dir}" add package Hyperledger.Iroha.Sdk',
        'run_dotnet "${app_dir}" build "${project_file}"',
        'run_dotnet "${app_dir}" run --project "${project_file}"',
        "NUGET_PACKAGES=\"${NUGET_PACKAGES:-${WORK_DIR}/nuget-packages}\"",
        "<add key=\"iroha-local\" value=\"${package_dir}\" />",
        "<add key=\"nuget.org\" value=\"https://api.nuget.org/v3/index.json\" />",
        "dotnet-add-package.log",
        "package_install_log_matches_local_source()",
        "Installed Hyperledger.Iroha.Sdk ${PACKAGE_VERSION} from ${source}",
        "cygpath -w",
        "package consumer smoke must not use ProjectReference",
        '<PackageReference Include=\\"Hyperledger.Iroha.Sdk\\" Version=\\"${PACKAGE_VERSION}\\"',
        "--configuration Release --no-restore -warnaserror",
        "Hyperledger.Iroha.Crypto",
        "Hyperledger.Iroha.Http",
        "Hyperledger.Iroha.Sccp",
        "Ed25519Signer.Verify(message, signature, publicKey)",
        "BuildCanonicalQueryString(\"?z=last&a=hello%20world\")",
        "EthereumMainnetSccp.RequireMainnetChainId",
        "EthereumMainnetSccp.RequireInboundRoute",
        "EthereumMainnetSccp.RequireOutboundRoute",
    )
    for marker in required_markers:
        assert marker in script


def test_csharp_sdk_package_consumer_stages_and_uses_pinned_dotnet_sdk(
    tmp_path: Path,
) -> None:
    """The temporary consumer must select .NET 8 even when .NET 10 is newer."""

    app_dir = tmp_path / "consumer"
    app_dir.mkdir()
    pinned_global_json = tmp_path / "pinned-global.json"
    pinned_global_json.write_text(
        '{"sdk":{"version":"8.0.419","rollForward":"latestFeature"}}\n',
        encoding="utf-8",
    )
    fake_dotnet = tmp_path / "dotnet"
    fake_dotnet.write_text(
        """#!/usr/bin/env bash
set -euo pipefail
case "${1:-}" in
  --version)
    if [[ -f "${PWD}/global.json" ]]; then
      printf '8.0.419\\n'
    else
      printf '10.0.400\\n'
    fi
    ;;
  --info)
    [[ -f "${PWD}/global.json" ]]
    printf 'fake dotnet info from pinned directory\\n'
    ;;
  *)
    printf 'unexpected fake dotnet arguments: %s\\n' "$*" >&2
    exit 64
    ;;
esac
""",
        encoding="utf-8",
    )
    fake_dotnet.chmod(0o755)

    bash_source = "\n".join(
        (
            "set -euo pipefail",
            'DOTNET_BIN="$1"',
            'DOTNET_GLOBAL_JSON="$2"',
            'app_dir="$3"',
            _bash_function_source("run_dotnet"),
            _bash_function_source("stage_pinned_dotnet_global_json"),
            _bash_function_source("require_dotnet"),
            'if require_dotnet "${app_dir}"; then',
            "  echo 'error: unpinned .NET 10 unexpectedly accepted' >&2",
            "  exit 1",
            "fi",
            'stage_pinned_dotnet_global_json "${app_dir}"',
            'require_dotnet "${app_dir}"',
            'if run_dotnet "${app_dir}/missing" --version; then',
            "  echo 'error: dotnet ran after its pinned SDK directory vanished' >&2",
            "  exit 1",
            "fi",
        )
    )
    completed = subprocess.run(
        [
            "bash",
            "-c",
            bash_source,
            "bash",
            str(fake_dotnet),
            str(pinned_global_json),
            str(app_dir),
        ],
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert "dotnet version: 10.0.400" in completed.stdout
    assert "got 10.0.400" in completed.stderr
    assert "dotnet version: 8.0.419" in completed.stdout
    assert "fake dotnet info from pinned directory" in completed.stdout
    assert (app_dir / "global.json").read_bytes() == pinned_global_json.read_bytes()


def test_pr_csharp_workflow_runs_package_consumer_guard() -> None:
    """The C# PR workflow must validate the packed SDK as a real consumer."""

    workflow = WORKFLOW.read_text(encoding="utf-8")

    assert _validate_pr_csharp_package_consumer_workflow(workflow) == []


def test_csharp_readme_documents_package_consumer_release_gate() -> None:
    """Release docs must tell operators to validate the packed NuGet package."""

    readme = README.read_text(encoding="utf-8")

    assert "dotnet build Hyperledger.Iroha.Sdk.sln -c Release --no-restore -warnaserror" in readme
    assert (
        "dotnet pack src/Hyperledger.Iroha.Sdk/Hyperledger.Iroha.Sdk.csproj "
        "-c Release --no-build --output artifacts/packages"
    ) in readme
    assert "ci/check_csharp_sdk_package_consumer.sh" in readme
    assert "PackageReference" in readme
    assert "ProjectReference" in readme


@pytest.mark.parametrize(
    ("mutated_workflow", "expected_error"),
    (
        (
            lambda workflow: _replace_once(workflow, WORKFLOW_PATH_TRIGGER, ""),
            "workflow path trigger must include consumer guard",
        ),
        (
            lambda workflow: _replace_once(
                workflow,
                WORKFLOW_BUILD,
                "run: dotnet build Hyperledger.Iroha.Sdk.sln -c Release --no-restore",
            ),
            "workflow must build the C# SDK solution with warnings as errors",
        ),
        (
            lambda workflow: _replace_once(
                workflow,
                WORKFLOW_PACK,
                "run: dotnet pack src/Hyperledger.Iroha.Sdk/Hyperledger.Iroha.Sdk.csproj "
                "-c Release --no-build",
            ),
            "workflow must pack the C# SDK into csharp/artifacts/packages",
        ),
        (
            lambda workflow: _replace_once(workflow, WORKFLOW_CONSUMER, ""),
            "workflow must run the package-consumer smoke after packing",
        ),
        (
            lambda workflow: _replace_once(
                workflow,
                f"{WORKFLOW_PACK}\n\n      - name: Package consumer smoke\n        {WORKFLOW_CONSUMER}",
                f"{WORKFLOW_CONSUMER}\n\n      - name: Pack\n        {WORKFLOW_PACK}",
            ),
            "workflow must run restore, build, test, pack, and consumer smoke in order",
        ),
    ),
)
def test_pr_csharp_workflow_rejects_package_consumer_drift(
    mutated_workflow,
    expected_error: str,
) -> None:
    """Adversarial workflow mutations must fail the package-consumer contract."""

    workflow = WORKFLOW.read_text(encoding="utf-8")

    assert expected_error in _validate_pr_csharp_package_consumer_workflow(
        mutated_workflow(workflow)
    )


def test_csharp_sdk_package_consumer_rejects_missing_local_package(
    tmp_path: Path,
) -> None:
    """The cheap negative control must fail before falling back to caches."""

    env = os.environ.copy()
    env["CSHARP_SDK_PACKAGE_CONSUMER_WORK_PARENT"] = str(tmp_path)
    completed = subprocess.run(
        ["bash", str(SCRIPT), "--negative-control-missing-local-package"],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        env=env,
    )

    assert completed.returncode == 0
    assert "local SDK package not found or empty" in completed.stdout
    assert "missing local package rejected as expected" in completed.stdout
    assert "dotnet --info" not in completed.stdout
    assert "dotnet --info" not in completed.stderr


def test_csharp_sdk_package_consumer_rejects_unknown_modes() -> None:
    """Unknown arguments must not silently run the default consumer smoke."""

    completed = subprocess.run(
        ["bash", str(SCRIPT), "--negative-control-unknown"],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )

    assert completed.returncode == 2
    assert "Usage: ci/check_csharp_sdk_package_consumer.sh" in completed.stderr
