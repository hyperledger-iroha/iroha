"""The repository launcher must preserve argv and the configured Gradle artifact location."""

import os
from pathlib import Path
import shutil
import subprocess

import pytest


LAUNCHER = Path(__file__).resolve().parents[1] / "android_keystore_attestation.sh"


@pytest.mark.parametrize("external", [False, True])
def test_launches_distribution_without_interpreting_arguments(tmp_path: Path, external: bool) -> None:
    root = tmp_path / "checkout with spaces"
    (root / "scripts").mkdir(parents=True)
    (root / "kotlin").mkdir()
    launcher = root / "scripts" / LAUNCHER.name
    shutil.copyfile(LAUNCHER, launcher)
    gradle = root / "kotlin/gradlew"
    gradle.write_text('#!/bin/sh\nprintf "build progress\\n"\n', encoding="utf-8")
    gradle.chmod(0o755)
    environment = dict(os.environ)
    environment.pop("MOBILE_SDK_ANDROID_ARTIFACT_DIR", None)
    build = root / "kotlin/tools/build"
    if external:
        artifacts = tmp_path / "external artifacts"
        environment["MOBILE_SDK_ANDROID_ARTIFACT_DIR"] = str(artifacts)
        build = artifacts / "gradle-build/iroha_kotlin_sdk/tools"
    command = build / "install/iroha-attestation/bin/iroha-attestation"
    command.parent.mkdir(parents=True)
    command.write_text('#!/bin/sh\nprintf "%s\\0" "$@"\nexit 7\n', encoding="utf-8")
    command.chmod(0o755)
    arguments = ["--alias", "spaces 'quotes' $(literal)", "--challenge-file", "line\nbreak"]
    result = subprocess.run(["bash", str(launcher), *arguments], env=environment, capture_output=True)
    assert result.returncode == 7
    assert result.stdout == b"".join(value.encode() + b"\0" for value in arguments)
    assert result.stderr == b"build progress\n"

    gradle.write_text("#!/bin/sh\nexit 19\n", encoding="utf-8")
    failed_build = subprocess.run(["bash", str(launcher), *arguments], env=environment, capture_output=True)
    assert failed_build.returncode == 19
    assert not failed_build.stdout
