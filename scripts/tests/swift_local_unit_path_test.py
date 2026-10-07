"""Run the original Swift manifest's local artifact path admission on macOS.

Only empty directories are used: reaching the missing-framework error grants no
artifact admission. The actual framework/source/ABI checks remain mandatory.
"""

import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

import pytest


ROOT = Path(__file__).resolve().parents[2]
pytestmark = pytest.mark.skipif(
    sys.platform != "darwin" or shutil.which("swift") is None,
    reason="the actual macOS Swift manifest owner is required",
)


@pytest.fixture(scope="module")
def workspace():
    """Keep every diagnostic output below the original checkout's target."""
    parent = ROOT / "target/qualification"
    parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="swift-local-path-", dir=parent) as directory:
        yield Path(directory)


def evaluate(workspace, selected, **overrides):
    """Evaluate the original manifest without resolving or building dependencies."""
    environment = os.environ.copy()
    for name in ("MOBILE_SDK_LOCAL_UNIT_ARTIFACT_DIR", "MOBILE_SDK_APPLE_ARTIFACT_DIR",
                 "MOBILE_SDK_REQUIRE_EXTERNAL_APPLE_ARTIFACT"):
        environment.pop(name, None)
    environment.update({"MOBILE_SDK_LOCAL_UNIT_ARTIFACT_DIR": str(selected),
                        "TMPDIR": str(workspace), **overrides})
    arguments = ["swift", "package", "--package-path", str(ROOT / "IrohaSwift")]
    for role in ("scratch", "cache", "config", "security"):
        arguments.extend([f"--{role}-path", str(workspace / role)])
    result = subprocess.run([*arguments, "dump-package"], env=environment,
                            cwd=ROOT, text=True, capture_output=True, check=False)
    assert result.returncode != 0, "empty directories must never become admitted frameworks"
    return result.stdout + result.stderr


def test_owned_qualification_child_reaches_mandatory_framework_check(workspace):
    child = workspace / "owned"
    child.mkdir(mode=0o700)
    output = evaluate(workspace, child)
    assert "NoritoBridge.xcframework is required at" in output


@pytest.mark.parametrize("selected", (ROOT, ROOT / "IrohaSwift", ROOT / "target/qualification"))
def test_source_and_qualification_root_are_not_artifact_directories(workspace, selected):
    assert "must be below target/qualification" in evaluate(workspace, selected)


def test_public_child_and_symbolic_ancestor_are_rejected(workspace):
    public = workspace / "public"
    public.mkdir(mode=0o755)
    assert "mode 0700" in evaluate(workspace, public)
    private = workspace / "private"
    private.mkdir(mode=0o700)
    link = workspace / "link"
    link.symlink_to(private, target_is_directory=True)
    assert "does not traverse a symbolic link" in evaluate(workspace, link)


def test_local_unit_cannot_enter_release_admission(workspace):
    output = evaluate(workspace, workspace, MOBILE_SDK_REQUIRE_EXTERNAL_APPLE_ARTIFACT="1")
    assert "local-unit artifacts cannot enter an external/release artifact corridor" in output
