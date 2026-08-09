"""Static fake-tool tests for the release fuzz-target inventory."""

from __future__ import annotations

import os
import shlex
import subprocess
import tempfile
import textwrap
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "fuzz_smoke.sh"
WORKFLOW = ROOT / ".github" / "workflows" / "workspace_release.yml"
NORITO_MANIFEST = ROOT / "crates" / "norito" / "fuzz" / "Cargo.toml"
IVM_MANIFEST = ROOT / "crates" / "ivm" / "fuzz" / "Cargo.toml"
PINNED_NIGHTLY = "nightly-2025-05-08"
PINNED_CARGO_FUZZ = "0.13.2"
NORITO_TARGETS = (
    "json_parse_string",
    "json_parse_string_ref",
    "json_skip_value",
    "json_from_json_equiv",
    "aos_view_optstr_equiv",
    "aos_view_optu32_equiv",
    "aos_view_enum_equiv",
    "aos_ncb_equiv_str_u32",
    "aos_ncb_equiv_bytes_u32",
)
IVM_TARGETS = ("tlv_validate", "kotodama_lower", "numeric_v1")


def manifest_bins(manifest: Path) -> tuple[tuple[str, str], ...]:
    """Read the ordered binary names and paths from this simple fuzz manifest."""

    rows: list[tuple[str, str]] = []
    current: dict[str, str] | None = None
    for raw_line in manifest.read_text(encoding="utf-8").splitlines():
        line = raw_line.strip()
        if line == "[[bin]]":
            if current is not None:
                rows.append((current["name"], current["path"]))
            current = {}
            continue
        if line.startswith("["):
            if current is not None:
                rows.append((current["name"], current["path"]))
                current = None
            continue
        if current is None or "=" not in line:
            continue
        key, value = (part.strip() for part in line.split("=", 1))
        if (
            key in {"name", "path"}
            and value.startswith('"')
            and value.endswith('"')
        ):
            current[key] = value[1:-1]
    if current is not None:
        rows.append((current["name"], current["path"]))
    return tuple(rows)


class FuzzSmokeInventoryTests(unittest.TestCase):
    """Prove strict mode calls every supported target with bounded resources."""

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
            exit 0
            """,
        )
        self.write_executable(
            "cargo-fuzz",
            f"""
            #!/usr/bin/env bash
            if [[ "${{1:-}}" == "--version" ]]; then
              echo "cargo-fuzz {PINNED_CARGO_FUZZ}"
              exit 0
            fi
            exit 64
            """,
        )
        self.write_executable(
            "rustup",
            f"""
            #!/usr/bin/env bash
            if [[ "${{1:-}}" == "toolchain" && "${{2:-}}" == "list" ]]; then
              echo "{PINNED_NIGHTLY}-x86_64-unknown-linux-gnu (default)"
              exit 0
            fi
            exit 64
            """,
        )

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def write_executable(self, name: str, source: str) -> None:
        """Install one fake Rust tool in the isolated test path."""

        path = self.bin / name
        path.write_text(textwrap.dedent(source).lstrip(), encoding="utf-8")
        path.chmod(0o755)

    def run_strict(self) -> subprocess.CompletedProcess[str]:
        """Run strict inventory selection without invoking Cargo or libFuzzer."""

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
        return subprocess.run(
            ["/bin/bash", str(SCRIPT), "--strict"],
            cwd=ROOT,
            env=environment,
            text=True,
            capture_output=True,
            check=False,
        )

    def test_strict_mode_invokes_exact_supported_inventory(self) -> None:
        """Nine Norito and three IVM targets run once each in stable order."""

        result = self.run_strict()
        self.assertEqual(result.returncode, 0, result.stderr)
        invocations = self.log.read_text(encoding="utf-8").splitlines()
        self.assertEqual(len(invocations), 12)

        observed: list[tuple[str, str]] = []
        for invocation in invocations:
            arguments = shlex.split(invocation)
            self.assertEqual(arguments[:3], [f"+{PINNED_NIGHTLY}", "fuzz", "run"])
            self.assertIn("--codegen-units", arguments)
            self.assertIn("16", arguments)
            fuzz_dir_index = arguments.index("--fuzz-dir")
            fuzz_dir = arguments[fuzz_dir_index + 1]
            target = arguments[fuzz_dir_index + 2]
            family = "norito" if fuzz_dir.endswith("/crates/norito/fuzz") else "ivm"
            self.assertTrue(
                fuzz_dir.endswith(("/crates/norito/fuzz", "/crates/ivm/fuzz")),
                fuzz_dir,
            )
            observed.append((family, target))
            self.assertIn("-runs=7", arguments)
            self.assertIn("-rss_limit_mb=128", arguments)
            self.assertIn("-max_total_time=2", arguments)

        expected = [("norito", target) for target in NORITO_TARGETS]
        expected.extend(("ivm", target) for target in IVM_TARGETS)
        self.assertEqual(observed, expected)

    def test_supported_inventory_matches_real_manifests(self) -> None:
        """Every selected target is one exact existing cargo-fuzz binary."""

        for manifest, expected_names in (
            (NORITO_MANIFEST, NORITO_TARGETS),
            (IVM_MANIFEST, IVM_TARGETS),
        ):
            bins = manifest_bins(manifest)
            self.assertEqual(tuple(name for name, _path in bins), expected_names)
            for _name, relative_path in bins:
                self.assertTrue(
                    (manifest.parent / relative_path).is_file(), relative_path
                )

    def test_release_workflow_pins_inventory_check_and_crash_artifacts(self) -> None:
        """The release job cannot silently regain optional or partial selection."""

        workflow = WORKFLOW.read_text(encoding="utf-8")
        adversarial_job = workflow.split("  adversarial:\n", 1)[1].split(
            "\n  format:\n", 1
        )[0]
        for required in (
            "toolchain: nightly-2025-05-08",
            "--version 0.13.2",
            "--locked",
            "RUNS: \"10000\"",
            "FUZZ_MAX_TOTAL_TIME: \"60\"",
            "FUZZ_RSS_LIMIT_MB: \"3072\"",
            "FUZZ_CODEGEN_UNITS: \"16\"",
            "python3 -m unittest -v",
            "pytests.scripts.corpora_integrity_replay_test",
            "pytests.scripts.fuzz_smoke_inventory_test",
            "scripts/fuzz_smoke.sh --strict",
            "if: always()",
            "crates/norito/fuzz/artifacts",
            "crates/ivm/fuzz/artifacts",
        ):
            self.assertIn(required, adversarial_job)
        self.assertNotIn(
            "scripts/fuzz_smoke.sh --strict --numeric-v1-only", adversarial_job
        )


if __name__ == "__main__":
    unittest.main()
