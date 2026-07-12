"""Tests for the fail-closed fuzz-smoke prerequisite and target selection policy."""

from __future__ import annotations

import os
import subprocess
import tempfile
import textwrap
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "fuzz_smoke.sh"
WORKFLOW = ROOT / ".github" / "workflows" / "numeric_v1_fuzz.yml"
PINNED_NIGHTLY = "nightly-2025-05-08"
PINNED_CARGO_FUZZ = "0.13.2"


class FuzzSmokeTests(unittest.TestCase):
    """Exercise optional and strict modes without invoking a real fuzzer."""

    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.directory = Path(self.temporary.name)
        self.bin = self.directory / "bin"
        self.bin.mkdir()
        self.log = self.directory / "cargo.log"
        self.write_executable(
            "cargo",
            """
            #!/usr/bin/env bash
            printf '%s\n' "$*" >> "$FUZZ_TEST_LOG"
            exit "${FAKE_CARGO_EXIT:-0}"
            """,
        )

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def write_executable(self, name: str, source: str) -> None:
        """Install one fake prerequisite in the isolated test PATH."""

        path = self.bin / name
        path.write_text(textwrap.dedent(source).lstrip(), encoding="utf-8")
        path.chmod(0o755)

    def install_cargo_fuzz(self, version: str = PINNED_CARGO_FUZZ) -> None:
        """Install a fake cargo-fuzz reporting the requested version."""

        self.write_executable(
            "cargo-fuzz",
            f"""
            #!/usr/bin/env bash
            if [[ "${{1:-}}" == "--version" ]]; then
              echo "cargo-fuzz {version}"
              exit 0
            fi
            exit 64
            """,
        )

    def install_rustup(self, *toolchains: str) -> None:
        """Install a fake rustup with an exact toolchain-list response."""

        encoded = "\\n".join(toolchains)
        self.write_executable(
            "rustup",
            f"""
            #!/usr/bin/env bash
            if [[ "${{1:-}}" == "toolchain" && "${{2:-}}" == "list" ]]; then
              printf '%b\\n' '{encoded}'
              exit 0
            fi
            exit 64
            """,
        )

    def run_script(
        self,
        *arguments: str,
        extra_environment: dict[str, str] | None = None,
    ) -> subprocess.CompletedProcess[str]:
        """Run the repository script with only the fake Rust tools visible."""

        environment = os.environ.copy()
        environment.update(
            {
                "PATH": f"{self.bin}:/usr/bin:/bin",
                "FUZZ_TEST_LOG": str(self.log),
                "RUNS": "7",
                "FUZZ_MAX_TOTAL_TIME": "2",
                "FUZZ_RSS_LIMIT_MB": "128",
                "FUZZ_CODEGEN_UNITS": "16",
            }
        )
        if extra_environment is not None:
            environment.update(extra_environment)
        return subprocess.run(
            ["/bin/bash", str(SCRIPT), *arguments],
            cwd=ROOT,
            env=environment,
            text=True,
            capture_output=True,
            check=False,
        )

    def test_optional_mode_skips_missing_cargo_fuzz(self) -> None:
        """Developer-default mode remains a successful, explicit skip."""

        result = self.run_script("--numeric-v1-only")
        self.assertEqual(result.returncode, 0)
        self.assertIn("skipping fuzz smoke", result.stderr)
        self.assertIn(f"cargo-fuzz {PINNED_CARGO_FUZZ} not installed", result.stderr)

    def test_strict_mode_rejects_missing_or_wrong_cargo_fuzz(self) -> None:
        """CI mode never turns a missing or mismatched fuzzer into success."""

        missing = self.run_script("--strict", "--numeric-v1-only")
        self.assertEqual(missing.returncode, 1)
        self.assertIn("required fuzz prerequisite unavailable", missing.stderr)

        self.install_cargo_fuzz("0.13.1")
        wrong = self.run_script("--strict", "--numeric-v1-only")
        self.assertEqual(wrong.returncode, 1)
        self.assertIn(
            f"expected cargo-fuzz {PINNED_CARGO_FUZZ}, found cargo-fuzz 0.13.1",
            wrong.stderr,
        )

    def test_strict_mode_requires_the_exact_pinned_nightly(self) -> None:
        """An unversioned or adjacent nightly cannot satisfy the release gate."""

        self.install_cargo_fuzz()
        self.install_rustup("nightly-aarch64-apple-darwin (default)")
        result = self.run_script("--strict", "--numeric-v1-only")
        self.assertEqual(result.returncode, 1)
        self.assertIn(f"{PINNED_NIGHTLY} not installed", result.stderr)

    def test_strict_numeric_mode_invokes_only_the_pinned_numeric_target(self) -> None:
        """The dedicated gate uses one exact toolchain, target, and resource bound."""

        self.install_cargo_fuzz()
        self.install_rustup(f"{PINNED_NIGHTLY}-x86_64-unknown-linux-gnu (default)")
        result = self.run_script("--strict", "--numeric-v1-only")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("running ivm::numeric_v1 for 7 runs", result.stdout)
        self.assertNotIn("Norito targets", result.stdout)

        invocations = self.log.read_text(encoding="utf-8").splitlines()
        self.assertEqual(len(invocations), 1)
        invocation = invocations[0]
        self.assertIn(f"+{PINNED_NIGHTLY} fuzz run", invocation)
        self.assertIn("--codegen-units 16", invocation)
        self.assertIn("crates/ivm/fuzz numeric_v1", invocation)
        self.assertIn("-runs=7", invocation)
        self.assertIn("-rss_limit_mb=128", invocation)
        self.assertIn("-max_total_time=2", invocation)
        self.assertNotIn("tlv_validate", invocation)
        self.assertNotIn("kotodama_lower", invocation)

    def test_strict_mode_propagates_target_failure(self) -> None:
        """A libFuzzer crash or nonzero exit makes the strict smoke fail."""

        self.install_cargo_fuzz()
        self.install_rustup(PINNED_NIGHTLY)
        result = self.run_script(
            "--strict",
            "--numeric-v1-only",
            extra_environment={"FAKE_CARGO_EXIT": "42"},
        )
        self.assertEqual(result.returncode, 1)
        self.assertIn("ivm target numeric_v1 failed", result.stderr)

    def test_invalid_limits_and_arguments_fail_before_fuzzing(self) -> None:
        """Malformed bounds and unknown flags are caller errors, not skips."""

        invalid_limit = self.run_script(
            "--numeric-v1-only",
            extra_environment={"RUNS": "0"},
        )
        self.assertEqual(invalid_limit.returncode, 2)
        self.assertIn("RUNS must be a positive decimal integer", invalid_limit.stderr)

        unknown = self.run_script("--unknown")
        self.assertEqual(unknown.returncode, 2)
        self.assertIn("unknown fuzz-smoke argument", unknown.stderr)

    def test_dedicated_workflow_pins_and_invokes_strict_mode(self) -> None:
        """Workflow and script pins cannot silently drift or regain a skip path."""

        workflow = WORKFLOW.read_text(encoding="utf-8")
        self.assertIn(f"toolchain: {PINNED_NIGHTLY}", workflow)
        self.assertIn(f"--version {PINNED_CARGO_FUZZ}", workflow)
        self.assertIn(
            "scripts/fuzz_smoke.sh --strict --numeric-v1-only",
            workflow,
        )
        self.assertIn('      - "v*"', workflow)


if __name__ == "__main__":
    unittest.main()
