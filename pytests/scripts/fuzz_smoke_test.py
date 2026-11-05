"""Tests for the fail-closed fuzz-smoke prerequisite and target selection policy."""

from __future__ import annotations

import os
import subprocess
import tempfile
import textwrap
import tomllib
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "fuzz_smoke.sh"
WORKFLOW = ROOT / ".github" / "workflows" / "numeric_v1_fuzz.yml"
NUMERIC_TARGET = ROOT / "crates" / "ivm" / "fuzz" / "fuzz_targets" / "numeric_v1.rs"
NUMERIC_CORPUS = ROOT / "crates" / "ivm" / "fuzz" / "corpus" / "numeric_v1"
PINNED_NIGHTLY = "nightly-2026-10-23"
PINNED_CARGO_FUZZ = "0.13.2"


class FuzzSmokeTests(unittest.TestCase):
    """Exercise optional and strict modes without invoking a real fuzzer."""

    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.directory = Path(self.temporary.name)
        self.bin = self.directory / "bin"
        self.bin.mkdir()
        self.log = self.directory / "cargo.log"
        self.fixture_root = self.directory / "repository"
        (self.fixture_root / "scripts").mkdir(parents=True)
        self.script = self.fixture_root / "scripts" / SCRIPT.name
        self.script.write_bytes(SCRIPT.read_bytes())
        self.fuzz = self.fixture_root / "crates" / "ivm" / "fuzz"
        self.fuzz.mkdir(parents=True)
        (self.fuzz / "Cargo.lock").write_text("version = 4\n", encoding="utf-8")
        self.write_executable(
            "cargo",
            """
            #!/usr/bin/env bash
            printf '%s\n' "$*" >> "$FUZZ_TEST_LOG"
            if [[ "${2:-}" == metadata ]]; then
              exit "${FAKE_METADATA_EXIT:-0}"
            fi
            if [[ "${FAKE_LOCK_DRIFT:-0}" == 1 ]]; then
              printf 'changed\n' >> Cargo.lock
            fi
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
            ["/bin/bash", str(self.script), *arguments],
            cwd=self.fixture_root,
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
        self.assertEqual(len(invocations), 2)
        self.assertIn(f"+{PINNED_NIGHTLY} metadata --locked --format-version 1", invocations[0])
        invocation = invocations[1]
        self.assertIn(f"+{PINNED_NIGHTLY} fuzz run", invocation)
        self.assertIn("--codegen-units 16", invocation)
        self.assertIn("crates/ivm/fuzz numeric_v1", invocation)
        self.assertIn("-runs=7", invocation)
        self.assertIn("-rss_limit_mb=128", invocation)
        self.assertIn("-max_total_time=2", invocation)
        self.assertNotIn("tlv_validate", invocation)
        self.assertNotIn("kotodama_lower", invocation)

    def test_strict_numeric_mode_requires_an_unchanged_locked_graph(self) -> None:
        """Missing locks, resolver changes and build-time lock drift cannot pass."""

        self.install_cargo_fuzz()
        self.install_rustup(PINNED_NIGHTLY)
        lock = self.fuzz / "Cargo.lock"
        lock.unlink()
        missing = self.run_script("--strict", "--numeric-v1-only")
        self.assertEqual(missing.returncode, 1)
        self.assertIn("committed regular Cargo.lock", missing.stderr)
        self.assertFalse(self.log.exists())
        lock.write_text("version = 4\n", encoding="utf-8")
        unresolved = self.run_script(
            "--strict", "--numeric-v1-only",
            extra_environment={"FAKE_METADATA_EXIT": "101"},
        )
        self.assertEqual(unresolved.returncode, 101)
        self.assertNotIn(" fuzz run ", self.log.read_text(encoding="utf-8"))
        drift = self.run_script(
            "--strict", "--numeric-v1-only",
            extra_environment={"FAKE_LOCK_DRIFT": "1"},
        )
        self.assertEqual(drift.returncode, 1)
        self.assertIn("Cargo.lock changed during numeric_v1", drift.stderr)

    def test_ivm_fuzz_lock_preserves_canonical_dependency_versions_and_patches(self) -> None:
        """The isolated graph adds only its fuzz harness to the shipping selection."""

        def packages(path: Path) -> set[tuple]:
            return {
                (item["name"], item["version"], item.get("source"), item.get("checksum"))
                for item in tomllib.loads(path.read_text(encoding="utf-8"))["package"]
            }

        canonical = packages(ROOT / "Cargo.lock")
        isolated = packages(ROOT / "crates/ivm/fuzz/Cargo.lock")
        added_packages = isolated - canonical
        self.assertEqual(len(added_packages), 4)
        additions = {name: version for name, version, _source, _checksum in added_packages}
        self.assertEqual(additions, {
            "ivm-fuzz": "2.0.0-rc.2.0", "libfuzzer-sys": "0.4.13",
            "arbitrary": "1.5.0", "derive_arbitrary": "1.5.0",
        })
        for name in ("w3f-bls", "ark-serialize", "ark-ff", "ark-bls12-381"):
            self.assertEqual(
                {item for item in isolated if item[0] == name},
                {item for item in canonical if item[0] == name},
            )
        root_patches = tomllib.loads((ROOT / "Cargo.toml").read_text())["patch"]["crates-io"]
        manifest = ROOT / "crates/ivm/fuzz/Cargo.toml"
        fuzz_patches = tomllib.loads(manifest.read_text())["patch"]["crates-io"]
        reachable = {name for name, _version, _source, _checksum in isolated}
        self.assertEqual(set(fuzz_patches), set(root_patches) & reachable)
        for name, patch in fuzz_patches.items():
            self.assertEqual(
                (manifest.parent / patch["path"]).resolve(),
                (ROOT / root_patches[name]["path"]).resolve(),
            )

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
        self.assertIn("ref: ${{ github.sha }}", workflow)
        self.assertIn('test "$(git rev-parse HEAD)" = "$GITHUB_SHA"', workflow)
        self.assertIn("group: numeric-v1-fuzz-${{ github.sha }}", workflow)
        self.assertIn("cancel-in-progress: false", workflow)

    def test_pull_requests_provision_hosted_tools_without_changing_release_policy(self) -> None:
        """PRs cannot queue on absent hosts; non-PR routing and fuzz limits stay fixed."""

        workflow = WORKFLOW.read_text(encoding="utf-8")
        self.assertIn(
            "runs-on: ${{ fromJSON(github.event_name == 'pull_request' && "
            "'[\"ubuntu-24.04\"]' || '[\"self-hosted\",\"Linux\",\"iroha2\"]') }}",
            workflow,
        )
        provision = workflow.split(
            "      - name: Provision hosted PR fuzz tools\n", 1
        )[1].split("\n      - ", 1)[0]
        self.assertIn("if: github.event_name == 'pull_request'", provision)
        self.assertIn("uses: ./.github/actions/setup-native-ci", provision)
        self.assertIn(f"toolchain: {PINNED_NIGHTLY}", provision)
        release = workflow.split(
            "      - uses: actions-rust-lang/setup-rust-toolchain@", 1
        )[1].split("\n      - ", 1)[0]
        self.assertIn("if: github.event_name != 'pull_request'", release)
        self.assertIn(f"toolchain: {PINNED_NIGHTLY}", release)
        for setting in ('RUNS: "10000"', 'FUZZ_MAX_TOTAL_TIME: "60"',
                        'FUZZ_RSS_LIMIT_MB: "3072"', 'FUZZ_CODEGEN_UNITS: "16"',
                        'timeout-minutes: 120', 'workspaces: "crates/ivm/fuzz -> target"'):
            self.assertIn(setting, workflow)
        self.assertNotIn("CARGO_TARGET_DIR", workflow)
        self.assertNotIn("continue-on-error", workflow)
        self.assertIn('".github/actions/setup-native-ci/**"', workflow)
        pull_request = workflow.split("  pull_request:\n", 1)[1].split(
            "  push:", 1
        )[0]
        for source in (
            "scripts/ci_private_fixture_root.py",
            "pytests/scripts/ci_private_fixture_root_test.py",
        ):
            self.assertIn(f'      - "{source}"\n', pull_request)
        check = workflow.split(
            "      - name: Exercise strict fuzz prerequisite and routing regressions\n", 1
        )[1].split("\n      - ", 1)[0]
        self.assertIn("if: github.event_name == 'pull_request'", check)
        self.assertIn("run: python3 pytests/scripts/fuzz_smoke_test.py", check)

    def test_dedicated_workflow_covers_every_numeric_semantics_input(self) -> None:
        """Compiler, ABI, fixture, syscall-test, and spec edits all trigger fuzzing."""

        workflow = WORKFLOW.read_text(encoding="utf-8")
        required_paths = (
            '"crates/kotodama_lang/src/**"',
            '"crates/kotodama_lang/tests/**"',
            '"crates/ivm/fuzz/**"',
            '"crates/ivm/src/numeric_v1.rs"',
            '"crates/ivm/src/numeric_gas.rs"',
            '"crates/ivm/src/numeric_tlv.rs"',
            '"crates/ivm/spec/syscalls.toml"',
            '"crates/ivm/tests/**"',
            '"crates/ivm_abi/src/**"',
            '"specs/kotodama_numeric_v1.md"',
            '"fixtures/numeric_v1_golden.json"',
        )
        for path in required_paths:
            self.assertIn(path, workflow)

    def test_numeric_target_exercises_arithmetic_corruption_and_staged_oog(self) -> None:
        """The release target must not regress to shallow encode/decode-only fuzzing."""

        source = NUMERIC_TARGET.read_text(encoding="utf-8")
        for required in (
            "exercise_envelope_corruptions",
            "fuzz_decimal_arithmetic",
            "try_decimal_div_exact",
            "try_decimal_div_round",
            "fuzz_staged_out_of_gas",
            "VMError::SyscallOutOfGas",
            "SyscallCompletion::Trap",
        ):
            self.assertIn(required, source)

        self.assertTrue(
            (NUMERIC_CORPUS / "arithmetic_full_width_seed")
            .read_bytes()
            .startswith(b"A")
        )
        self.assertTrue(
            (NUMERIC_CORPUS / "staged_oog_seed").read_bytes().startswith(b"G")
        )


if __name__ == "__main__":
    unittest.main()
