"""Static safety checks for scripts/run_local_swarm.sh."""

from __future__ import annotations

import json
import os
import subprocess
import tempfile
import tomllib
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT = REPO_ROOT / "scripts" / "run_local_swarm.sh"


def _script_text() -> str:
    return SCRIPT.read_text(encoding="utf-8")


def _generated_stop_script_body() -> str:
    text = _script_text()
    marker = 'write_private_config "$BASE/stop.sh" <<\'EOF\'\n'
    start = text.index(marker) + len(marker)
    end = text.index("\nEOF\n", start)
    return text[start:end] + "\n"


class RunLocalSwarmSafetyTest(unittest.TestCase):
    def test_script_and_generated_stop_script_have_valid_bash_syntax(self) -> None:
        subprocess.run(["bash", "-n", str(SCRIPT)], check=True)
        subprocess.run(
            ["bash", "-n"],
            input=_generated_stop_script_body().encode("utf-8"),
            check=True,
        )

    def test_stop_guidance_uses_guarded_pid_ownership_checks(self) -> None:
        text = _script_text()
        stop_body = _generated_stop_script_body()

        self.assertNotIn("xargs kill", text)
        self.assertNotIn('rm -f "$BASE"/peer*.pid', text)
        self.assertIn("preflight_existing_pidfiles", text)
        self.assertIn("pid_is_running()", text)
        self.assertIn("command -v ps >/dev/null 2>&1 || return 0", text)
        self.assertNotIn("kill -0", text)
        self.assertIn("Refusing to overwrite live local-swarm peer", text)
        self.assertIn("To stop safely: cd $BASE && ./stop.sh", text)
        self.assertIn("pid_matches_peer()", stop_body)
        self.assertIn("pid_is_running()", stop_body)
        self.assertIn("command -v ps >/dev/null 2>&1 || return 0", stop_body)
        self.assertIn('grep -F -- "--config $config"', stop_body)
        self.assertIn("live pid $pid does not match $config", stop_body)
        self.assertIn('kill "$pid" 2>/dev/null || true', stop_body)
        self.assertNotIn('kill -9 "$pid"', stop_body)
        self.assertIn(
            "local-swarm peer $peer_name pid $pid is still running",
            stop_body,
        )

    def test_checked_genesis_network_id_is_bound_into_every_config(self) -> None:
        text = _script_text()

        self.assertIn(
            '--expected-hash-out "$BASE/genesis.expected_hash"',
            text,
        )
        self.assertIn(
            '[[ ! "$GENESIS_NETWORK_ID" =~ ^hash:[0-9A-F]{63}[13579BDF]#[0-9A-F]{4}$ ]]',
            text,
        )
        self.assertIn('expected_hash_file = "$BASE/genesis.expected_hash"', text)
        self.assertIn('network_id_file = "$BASE/genesis.expected_hash"', text)
        self.assertNotIn('expected_hash = "$GENESIS_NETWORK_ID"', text)
        self.assertNotIn('network_id = "$GENESIS_NETWORK_ID"', text)

    def test_consensus_context_is_signed_in_genesis_not_local_config(self) -> None:
        text = _script_text()

        self.assertIn("$KAGAMI genesis generate", text)
        self.assertNotIn("KAGEMUSHA_MINT_FINALITY_PARAMETERS_FILE", text)
        self.assertNotIn("--kagemusha-mint-finality-parameters", text)
        self.assertIn("Consensus mode, validator set, and DA geometry come from the signed genesis", text)
        self.assertNotIn("consensus_mode =", text)
        self.assertNotIn("enable_bls =", text)
        self.assertNotIn("da_enabled =", text)


class RunLocalSwarmPublisherCustodyTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        self.base = self.root / "generated"
        self.custody = self.root / "original-custody"
        self.custody.mkdir(mode=0o700)
        for index in range(4):
            for suffix in ("keyring.nrt", "submitter-private-key"):
                path = self.custody / f"peer{index}-{suffix}"
                path.write_bytes(b"original-private-test-bytes")
                path.chmod(0o600)

    def _environment(self, custody: str | None = None) -> dict[str, str]:
        environment = dict(os.environ)
        environment["BASE"] = str(self.base)
        environment["KAGEMUSHA_LOAD_AUTHORIZER_CUSTODY_DIR"] = (
            str(self.custody) if custody is None else custody
        )
        return environment

    def _preflight(self, custody: str | None = None) -> subprocess.CompletedProcess[str]:
        text = _script_text()
        start = text.index("validate_publisher_custody() {\n")
        end = text.index("\nvalidate_transport_identities() {", start)
        executable = text[start:end] + (
            "\nPUBLISHER_KEYRING_TOMLS=()\nPUBLISHER_SUBMITTER_TOMLS=()\n"
            'validate_publisher_custody || exit $?\n'
            'for i in 0 1 2 3; do\n'
            '  printf "%s\\n%s\\n" "${PUBLISHER_KEYRING_TOMLS[$i]}" "${PUBLISHER_SUBMITTER_TOMLS[$i]}"\n'
            'done\n'
        )
        return subprocess.run(
            ["bash", "-c", executable], env=self._environment(custody),
            cwd=self.root, text=True, capture_output=True, timeout=3,
        )

    def test_help_has_no_custody_or_build_side_effect(self) -> None:
        result = subprocess.run(
            ["bash", str(SCRIPT), "--help"], env=self._environment(""),
            cwd=self.root, text=True, capture_output=True,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("KAGEMUSHA_LOAD_AUTHORIZER_CUSTODY_DIR", result.stdout)
        self.assertIn("no publisher disable mode", result.stdout)
        self.assertFalse(self.base.exists())

    def test_absent_custody_fails_full_script_before_base_or_build(self) -> None:
        for directory in ("", str(self.root / "absent")):
            with self.subTest(directory=directory):
                result = subprocess.run(
                    ["bash", str(SCRIPT)], env=self._environment(directory),
                    cwd=self.root, text=True, capture_output=True,
                )
                self.assertNotEqual(result.returncode, 0)
                self.assertFalse(self.base.exists())
                self.assertNotIn("original-private-test-bytes", result.stdout + result.stderr)

    def test_each_of_eight_required_files_is_mandatory_and_nonempty(self) -> None:
        for path in sorted(self.custody.iterdir()):
            for action in ("missing", "empty"):
                with self.subTest(path=path.name, action=action):
                    path.unlink()
                    if action == "empty":
                        path.touch(mode=0o600)
                    self.assertNotEqual(self._preflight().returncode, 0)
                    path.write_bytes(b"original-private-test-bytes")
                    path.chmod(0o600)
        self.assertEqual(self._preflight().returncode, 0)

    def test_private_files_reject_symlinks_hardlinks_and_public_modes(self) -> None:
        path = self.custody / "peer1-keyring.nrt"
        original = self.root / "different-original"
        original.write_bytes(b"different-private-test-bytes")
        original.chmod(0o600)
        path.unlink()
        path.symlink_to(original)
        self.assertNotEqual(self._preflight().returncode, 0)
        path.unlink()
        os.link(original, path)
        self.assertNotEqual(self._preflight().returncode, 0)
        path.unlink()
        path.write_bytes(b"original-private-test-bytes")
        for mode in (0o640, 0o604, 0o000):
            path.chmod(mode)
            self.assertNotEqual(self._preflight().returncode, 0)
        path.chmod(0o600)
        self.assertEqual(self._preflight().returncode, 0)

    def test_directory_refuses_aliases_public_access_and_reset_storage_tree(self) -> None:
        alias = self.root / "custody-alias"
        alias.symlink_to(self.custody, target_is_directory=True)
        self.assertNotEqual(self._preflight(str(alias)).returncode, 0)
        self.assertNotEqual(self._preflight("original-custody").returncode, 0)
        self.custody.chmod(0o750)
        self.assertNotEqual(self._preflight().returncode, 0)
        self.custody.chmod(0o700)
        self.base.mkdir()
        inside = self.base / "storage" / "publisher"
        inside.parent.mkdir(exist_ok=True)
        self.custody.rename(inside)
        self.assertNotEqual(self._preflight(str(inside)).returncode, 0)

    def test_boundaries_match_actual_private_file_parser(self) -> None:
        for suffix, maximum in (("keyring.nrt", 65_536), ("submitter-private-key", 4_096)):
            path = self.custody / f"peer2-{suffix}"
            with path.open("wb") as output:
                output.truncate(maximum)
            self.assertEqual(self._preflight().returncode, 0)
            with path.open("wb") as output:
                output.truncate(maximum + 1)
            self.assertNotEqual(self._preflight().returncode, 0)
            path.write_bytes(b"original-private-test-bytes")

    def test_exact_per_peer_paths_are_toml_escaped_without_shell_evaluation(self) -> None:
        unusual = self.root / 'private $(touch SHOULD_NOT_EXIST) "\\\nΔ'
        self.custody.rename(unusual)
        self.custody = unusual
        result = self._preflight()
        self.assertEqual(result.returncode, 0, result.stderr)
        decoded = [json.loads(line) for line in result.stdout.splitlines()]
        expected = [str(self.custody / f"peer{i}-{suffix}") for i in range(4)
                    for suffix in ("keyring.nrt", "submitter-private-key")]
        self.assertEqual(decoded, expected)
        self.assertFalse((self.root / "SHOULD_NOT_EXIST").exists())
        text = _script_text()
        prefix = text[:text.index("pid_matches_local_swarm_peer() {")]
        custody = text[text.index("validate_publisher_custody() {"):text.index("\nvalidate_transport_identities() {")]
        address = text[text.index("addr_literal() {"):text.index("\ninject_topology() {")]
        trusted = text[text.index("trusted_peers_literal() {"):text.index("\nwrite_config() {")]
        config = text[text.index("write_config() {"):text.index("\nwrite_client_config() {")]
        executable = prefix + custody + address + trusted + config + (
            'validate_publisher_custody || exit $?\n'
            'mkdir -p "$BASE"\nGEN_PUB="public-test-genesis"\n'
            'for i in 0 1 2 3; do write_config "$i"; done\n'
        )
        generated = subprocess.run(
            ["bash", "-c", executable], env=self._environment(),
            cwd=self.root, text=True, capture_output=True,
        )
        self.assertEqual(generated.returncode, 0, generated.stderr)
        for index in range(4):
            path = self.base / f"peer{index}.toml"
            value = tomllib.loads(path.read_text())
            self.assertEqual(value["kagemusha_load_authorizer"], {
                "keyring_file": expected[index * 2],
                "submitter_key_file": expected[index * 2 + 1],
            })
            self.assertEqual(path.stat().st_mode & 0o777, 0o600)
            self.assertNotIn("original-private-test-bytes", path.read_text())
        self.assertFalse((self.root / "SHOULD_NOT_EXIST").exists())

    def _output_helpers(self) -> str:
        text = _script_text()
        return text[text.index("validate_publisher_custody() {"):text.index("\nvalidate_transport_identities() {")]

    def test_fifo_is_refused_without_a_writer_or_byte_read(self) -> None:
        path = self.custody / "peer0-keyring.nrt"
        path.unlink()
        os.mkfifo(path, 0o600)
        result = self._preflight()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("regular files", result.stderr)

    def test_no_follow_private_writer_preserves_original_aliases_and_public_files(self) -> None:
        self.base.mkdir(mode=0o700)
        target = self.base / "peer0.toml"
        original = self.custody / "peer0-keyring.nrt"
        original_bytes = original.read_bytes()
        environment = self._environment()
        environment["TARGET"] = str(target)
        executable = self._output_helpers() + '\nwrite_private_config "$TARGET"\n'
        for kind in ("symlink", "hardlink", "public-file", "fifo"):
            with self.subTest(kind=kind):
                if kind == "symlink":
                    target.symlink_to(original)
                elif kind == "hardlink":
                    os.link(original, target)
                elif kind == "public-file":
                    target.write_bytes(b"existing-public-bytes")
                    target.chmod(0o644)
                else:
                    os.mkfifo(target, 0o600)
                result = subprocess.run(["bash", "-c", executable], env=environment,
                                        input="new-private-config", cwd=self.root,
                                        text=True, capture_output=True, timeout=3)
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(original.read_bytes(), original_bytes)
                if kind == "public-file":
                    self.assertEqual(target.read_bytes(), b"existing-public-bytes")
                    self.assertEqual(target.stat().st_mode & 0o777, 0o644)
                target.unlink()
        result = subprocess.run(["bash", "-c", executable], env=environment,
                                input="new-private-config", cwd=self.root,
                                text=True, capture_output=True, timeout=3)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(target.read_text(), "new-private-config")
        self.assertEqual(target.stat().st_mode & 0o777, 0o600)
        result = subprocess.run(["bash", "-c", executable], env=environment,
                                input="replacement-private-config", cwd=self.root,
                                text=True, capture_output=True, timeout=3)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(target.read_text(), "replacement-private-config")
        self.assertEqual(original.read_bytes(), original_bytes)

    def test_all_output_targets_and_base_are_preflighted_before_any_redirect(self) -> None:
        self.base.mkdir(mode=0o700)
        executable = self._output_helpers() + '\nvalidate_local_swarm_output_targets\n'
        def check():
            return subprocess.run(["bash", "-c", executable], env=self._environment(),
                                  cwd=self.root, text=True, capture_output=True, timeout=3)
        self.assertEqual(check().returncode, 0)
        names = ["stop.sh", "client.toml", "genesis.json", "genesis.signed.nrt",
                 "genesis.expected_hash", "gen.sign.log"]
        names += [f"peer{index}.{suffix}" for index in range(4)
                  for suffix in ("toml", "log", "pid", "check-config.log")]
        original = self.custody / "peer0-keyring.nrt"
        original_bytes = original.read_bytes()
        for name in names:
            with self.subTest(name=name):
                target = self.base / name
                target.symlink_to(original)
                self.assertNotEqual(check().returncode, 0)
                self.assertEqual(original.read_bytes(), original_bytes)
                target.unlink()
        self.base.chmod(0o755)
        self.assertNotEqual(check().returncode, 0)
        self.base.chmod(0o700)
        target = self.base / "peer0.toml"
        target.write_bytes(b"existing-private-config")
        target.chmod(0o644)
        self.assertNotEqual(check().returncode, 0)
        self.assertEqual(target.read_bytes(), b"existing-private-config")
        text = _script_text()
        checked = text.index("\nvalidate_local_swarm_output_targets\n")
        self.assertLess(checked, text.index("\nwrite_stop_script\n"))
        self.assertLess(checked, text.index('rm -rf "$BASE/storage"'))
        self.assertLess(checked, text.index("cargo build --release"))

    def test_storage_reset_requires_explicit_boolean_before_effects(self) -> None:
        text = _script_text()
        self.assertIn('RESET_STORAGE="${RESET_STORAGE-0}"', text)
        self.assertIn('if [[ "$RESET_STORAGE" == 1 ]]; then', text)
        public = self.root / "public.key"
        private = self.root / "private.key"
        public.write_text("public-test-fixture\n")
        private.write_text("private-test-fixture\n")
        private.chmod(0o600)
        for value in ("", "2", "01", "true", "-1", "0 ", "1\n"):
            with self.subTest(value=value):
                environment = self._environment()
                environment.update({"RESET_STORAGE": value,
                                    "GENESIS_PUBLIC_KEY_FILE": str(public),
                                    "GENESIS_PRIVATE_KEY_FILE": str(private),
                                    "GENESIS_CREATION_TIME_MS": "1700000000000"})
                result = subprocess.run(["bash", str(SCRIPT)], env=environment,
                                        cwd=self.root, text=True, capture_output=True, timeout=3)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("RESET_STORAGE must be exactly 0 or 1", result.stderr)
                self.assertFalse(self.base.exists())
        self.assertLess(text.index("\nvalidate_storage_reset\n"), text.index('mkdir -p "$BASE"'))
        for value in ("0", "1"):
            environment = self._environment()
            environment["RESET_STORAGE"] = value
            result = subprocess.run(["bash", "-c", self._output_helpers() + "\nvalidate_storage_reset\n"],
                                    env=environment, cwd=self.root, text=True, capture_output=True, timeout=3)
            self.assertEqual(result.returncode, 0, result.stderr)

    def test_explicit_genesis_originals_and_canonical_timestamp_are_required(self) -> None:
        text = _script_text()
        start = text.index("validate_genesis_inputs() {\n")
        end = text.index("\nvalidate_publisher_custody() {", start)
        body = text[start:end] + "\nvalidate_genesis_inputs || exit $?\n"
        public = self.root / "public.key"
        private = self.root / "private.key"
        public.write_text("public-test-fixture\n")
        private.write_text("private-test-fixture\n")
        private.chmod(0o600)
        environment = self._environment()
        environment.update({"GENESIS_PUBLIC_KEY_FILE": str(public),
                            "GENESIS_PRIVATE_KEY_FILE": str(private)})
        for value in ("", "-1", "01", "1.0", " 1", "1 ", "1\n", str(2**64), "9" * 100):
            with self.subTest(value=value):
                environment["GENESIS_CREATION_TIME_MS"] = value
                result = subprocess.run(["bash", "-c", body], env=environment,
                                        cwd=self.root, text=True, capture_output=True)
                self.assertNotEqual(result.returncode, 0)
        for value in ("0", "1700000000000", str(2**64 - 1)):
            environment["GENESIS_CREATION_TIME_MS"] = value
            result = subprocess.run(["bash", "-c", body], env=environment,
                                    cwd=self.root, text=True, capture_output=True)
            self.assertEqual(result.returncode, 0, result.stderr)
        for name in ("GENESIS_PUBLIC_KEY_FILE", "GENESIS_PRIVATE_KEY_FILE"):
            for value in ("", str(self.root / "absent")):
                broken = dict(environment)
                broken[name] = value
                result = subprocess.run(["bash", str(SCRIPT)], env=broken,
                                        cwd=self.root, text=True, capture_output=True)
                self.assertNotEqual(result.returncode, 0)
                self.assertFalse(self.base.exists())
        self.assertIn('--creation-time-ms "$GENESIS_CREATION_TIME_MS"', text)
        self.assertNotIn("$KAGAMI keys --out-dir", text)
        self.assertNotIn("GENESIS_KEY_DIR=", text)

    def test_preflight_and_genuine_all_peer_config_check_precede_launch(self) -> None:
        text = _script_text()
        preflight = text.index("\nvalidate_publisher_custody\n")
        self.assertLess(preflight, text.index('mkdir -p "$BASE"'))
        self.assertLess(preflight, text.index('rm -rf "$BASE/storage"'))
        self.assertLess(preflight, text.index("cargo build --release"))
        check = text.index('"$IROHAD" --config "$BASE/peer${i}.toml" --check-config')
        launch = text.index('RUST_LOG=info "$IROHAD" --config "$BASE/peer${i}.toml"')
        self.assertLess(text.rindex("\nvalidate_publisher_custody\n"), check)
        self.assertLess(check, launch)
        self.assertIn('for i in 0 1 2 3; do\n  "$IROHAD"', text)
        self.assertNotIn("os.read(", text)
        self.assertNotIn("KAGEMUSHA_LOAD_AUTHORIZER_ENABLED", text)
        self.assertNotIn("KAGEMUSHA_LOAD_AUTHORIZER_DISABLED", text)


if __name__ == "__main__":
    unittest.main()
