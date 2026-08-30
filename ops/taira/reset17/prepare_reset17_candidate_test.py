"""Tests for deterministic reset17 public-bundle preparation."""

from __future__ import annotations

import importlib.util
import json
import os
from pathlib import Path
import plistlib
import stat
import sys
import tempfile
import unittest


ROOT = Path(__file__).parent
CONTROLLER_SPEC = importlib.util.spec_from_file_location(
    "testnet_reset17_authenticated_reset", ROOT / "testnet_reset17_authenticated_reset.py"
)
assert CONTROLLER_SPEC is not None and CONTROLLER_SPEC.loader is not None
controller = importlib.util.module_from_spec(CONTROLLER_SPEC)
sys.modules[CONTROLLER_SPEC.name] = controller
CONTROLLER_SPEC.loader.exec_module(controller)

PREPARE_SPEC = importlib.util.spec_from_file_location(
    "prepare_reset17_candidate", ROOT / "prepare_reset17_candidate.py"
)
assert PREPARE_SPEC is not None and PREPARE_SPEC.loader is not None
prepare = importlib.util.module_from_spec(PREPARE_SPEC)
sys.modules[PREPARE_SPEC.name] = prepare
PREPARE_SPEC.loader.exec_module(prepare)


class PrepareReset17CandidateTest(unittest.TestCase):
    def setUp(self) -> None:
        self._temporary = tempfile.TemporaryDirectory(dir=ROOT)
        self.addCleanup(self._temporary.cleanup)
        self.root = Path(self._temporary.name)
        self.sources = self.root / "sources"
        self.sources.mkdir(mode=0o700)
        self.control = self.root / "control"
        self.launch_agents = self.root / "Library" / "LaunchAgents"
        self.spec_path = self.root / "spec.json"
        self.bundle = self.root / "bundle"
        self.artifact_paths: dict[str, str] = {}
        for name in sorted(controller.REQUIRED_ARTIFACTS):
            path = self.sources / name
            path.write_bytes(f"{name}\n".encode())
            path.chmod(0o700 if name in prepare.EXECUTABLE_ARTIFACTS else 0o600)
            self.artifact_paths[name] = str(path)
        self.config_paths = []
        for index in range(1, 5):
            path = self.sources / f"validator-{index}.toml"
            path.write_text(f"index = {index}\n", encoding="utf-8")
            self.config_paths.append(path)

    def _spec(self) -> dict[str, object]:
        commit = "1" * 40
        validators = []
        for index in range(1, 5):
            private_root = self.control / "private" / f"validator-{index}"
            private_files = {
                role: str(private_root / f"{role}.private")
                for role in sorted(controller.PRIVATE_ROLES)
            }
            public_key = f"{index:064x}"
            validators.append(
                {
                    "index": index,
                    "label": f"org.sora.taira.user.validator-{index}",
                    "data_root": str(
                        self.control / "data" / "reset17" / f"validator-{index}"
                    ),
                    "torii_url": f"http://127.0.0.1:{29079 + index}",
                    "p2p_port": 33469 + index,
                    "config_source": str(self.config_paths[index - 1]),
                    "private_files": private_files,
                    "runtime_signer": {
                        "source_path": private_files["soracloud_runtime_signer"],
                        "launch_path": str(
                            self.control
                            / "run"
                            / "reset17"
                            / f"validator-{index}"
                            / "runtime-signer.fd198"
                        ),
                        "public_key_hex": public_key,
                        "handle": f"software://taira/inrou/{public_key}",
                        "authority": f"validator-{index}@taira",
                        "algorithm": "ed25519",
                        "revision": 1,
                        "policy_digest_hex": f"{index + 10:064x}",
                    },
                }
            )
        return {
            "schema": prepare.SPEC_SCHEMA,
            "release_id": "reset17-unit",
            "network_id": "2" * 64,
            "source": {
                "commit": commit,
                "tree": "3" * 40,
                "parent": "4" * 40,
                "commit_signer_fingerprint": controller.EXPECTED_COMMIT_SIGNER_FINGERPRINT,
                "source_date_epoch": 1,
                "cargo_target_dir": "/private/tmp/taira-reset17-authenticated",
                "rustc_version": "rustc fixture",
                "cargo_version": "cargo fixture",
                "build_commands": [
                    list(command) for command in controller.EXPECTED_BUILD_COMMANDS
                ],
            },
            "protocols": {
                "native_bridge_abi": 22,
                "kagemusha_data_abi": 4,
                "exact12": list(controller.EXACT12_PROTOCOLS),
            },
            "bpng": {
                "asset_alias": "kina#bpng",
                "asset_definition": "839FV3NJC8NfgWQvghXU2hEFQm9a",
                "asset_domain": "bpng.bpng",
                "scale": 2,
                "lane_id": 3,
                "lane_alias": "dpn",
                "physical_dataspace_id": 10,
                "physical_dataspace_alias": "bpng",
            },
            "deployment": {
                "uid": os.geteuid(),
                "launch_domain": f"gui/{os.geteuid()}",
                "python_path": str(Path(sys.executable).resolve()),
                "release_dir": str(
                    self.control / "releases" / "reset17-unit"
                ),
                "launch_agents_dir": str(self.launch_agents),
                "control_root": str(self.control),
                "require_single_data_volume": True,
                "free_reserve_bytes": controller.MIN_FREE_RESERVE_BYTES,
                "free_reserve_bps": controller.FREE_RESERVE_BPS,
            },
            "artifacts": self.artifact_paths,
            "validators": validators,
        }

    def _write_spec(self, value: dict[str, object]) -> None:
        self.spec_path.write_bytes(controller.canonical_json_bytes(value))

    def test_render_seals_exact_alias_manifest_and_launch_agents(self) -> None:
        self._write_spec(self._spec())
        manifest_path, digest = prepare.render(self.spec_path, self.bundle)

        payload = manifest_path.read_bytes()
        self.assertEqual(digest, __import__("hashlib").sha256(payload).hexdigest())
        manifest = json.loads(payload)
        self.assertEqual(manifest["bpng"]["asset_alias"], "kina#bpng")
        self.assertEqual(manifest["bpng"]["asset_domain"], "bpng.bpng")
        self.assertEqual(manifest["bpng"]["lane_alias"], "dpn")
        self.assertEqual(manifest["bpng"]["physical_dataspace_alias"], "bpng")
        self.assertEqual(stat.S_IMODE(self.bundle.stat().st_mode), 0o555)
        launch_record = manifest["validators"][0]["launch_agent"]
        launch = plistlib.loads((self.bundle / launch_record["path"]).read_bytes())
        self.assertEqual(launch["Umask"], 0o077)
        self.assertEqual(
            launch["ProgramArguments"][-1],
            str(
                self.control
                / "run"
                / "reset17"
                / "validator-1"
                / "runtime-signer.fd198"
            ),
        )

    def test_render_rejects_alias_in_wire_definition_field_before_output(self) -> None:
        spec = self._spec()
        spec["bpng"] = dict(spec["bpng"])  # type: ignore[arg-type]
        spec["bpng"]["asset_definition"] = "kina#bpng"  # type: ignore[index]
        self._write_spec(spec)
        with self.assertRaisesRegex(controller.Reset17Error, "routing profile"):
            prepare.render(self.spec_path, self.bundle)
        self.assertFalse(self.bundle.exists())


if __name__ == "__main__":
    unittest.main()
