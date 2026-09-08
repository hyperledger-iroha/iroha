"""Cargo source relocation regressions, including an actual dependency-free build."""

import importlib.util
import fcntl
import os
from pathlib import Path
import struct
import subprocess
import tempfile
import unittest


SCRIPT = Path(__file__).resolve().parents[2] / "scripts/taira_cargo_cache.py"
SPEC = importlib.util.spec_from_file_location("taira_cargo_cache", SCRIPT)
cache = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(cache)


def record(paths):
    raw = b"\x01\x00\x00\x00\xff\x01" + struct.pack("<I", len(paths))
    for kind, path in paths:
        value = os.fsencode(path)
        raw += bytes([kind]) + struct.pack("<I", len(value)) + value + b"\0"
    return raw + b"\0\0\0\0"


class CargoSourceAdmissionTests(unittest.TestCase):
    def test_parser_rejects_unknown_truncated_and_trailing_records(self):
        expected = [(1, Path("source/build.rs")), (0, Path("src/lib.rs"))]
        raw = record(expected)
        self.assertEqual(cache.dependency_paths(raw), expected)
        for invalid in (raw[:-1], raw + b"x", b"old format", raw[:5] + b"\2" + raw[6:]):
            with self.subTest(invalid=invalid), self.assertRaises(ValueError):
                cache.dependency_paths(invalid)

    def test_only_foreign_local_package_metadata_is_retired(self):
        with tempfile.TemporaryDirectory() as temporary:
            target = Path(temporary).resolve()
            source = target / "source"
            source.mkdir()
            parent = target / "release/.fingerprint"
            for name, paths in {
                "ivm-1111111111111111": [(1, "old-source/crates/ivm/build.rs")],
                "ivm-2222222222222222": [(1, "source/crates/ivm/src/lib.rs")],
                "local-3333333333333333": [(0, "src/lib.rs")],
                "safe-4444444444444444": [(1, "source/crates/safe/lib.rs"),
                                          (1, "release/build/safe/out/generated.rs")],
                "registry-5555555555555555": [(0, "src/lib.rs")],
            }.items():
                directory = parent / name
                directory.mkdir(parents=True)
                (directory / "dep-lib-fixture").write_bytes(record(paths))
            artifact = target / "release/retained-binary"
            artifact.write_bytes(b"compiled output")
            packages = {"ivm", "local", "safe"}
            with self.assertRaisesRegex(ValueError, "foreign Cargo"):
                cache.admit_source_fingerprints(source, target, "aarch64-unknown-linux-gnu", packages, repair=False)
            def interrupted_checkpoint():
                self.assertEqual(len(list(parent.iterdir())), 5)
                raise OSError("fixture checkpoint interruption")
            with self.assertRaisesRegex(OSError, "checkpoint interruption"):
                cache.admit_source_fingerprints(source, target, "aarch64-unknown-linux-gnu", packages,
                                                 before_retire=interrupted_checkpoint)
            self.assertEqual(len(list(parent.iterdir())), 5)
            self.assertEqual(cache.admit_source_fingerprints(source, target, "aarch64-unknown-linux-gnu", packages),
                             ["ivm", "local"])
            self.assertEqual(sorted(p.name for p in parent.iterdir()),
                             ["registry-5555555555555555", "safe-4444444444444444"])
            self.assertEqual(len(list((target / "taira-release-cache-retired").glob("*/release/.fingerprint/*"))), 3)
            self.assertEqual(artifact.read_bytes(), b"compiled output")
            self.assertEqual(cache.admit_source_fingerprints(source, target, "aarch64-unknown-linux-gnu", packages), [])
            with cache.source_fingerprints(source, target, "aarch64-unknown-linux-gnu", packages, repair=False):
                with (target / "release/.cargo-lock").open("rb") as lock:
                    with self.assertRaises(BlockingIOError):
                        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)

    def test_real_cargo_rebuilds_relocated_host_build_script(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            target = root / "target"
            target.mkdir()
            environment = {key: value for key, value in os.environ.items()
                           if key in {"PATH", "HOME", "RUSTUP_HOME", "TMPDIR"}}
            environment.update(CARGO_TARGET_DIR=str(target), CARGO_HOME=str(root / "cargo-home"),
                               CARGO_NET_OFFLINE="true", RUSTUP_TOOLCHAIN="1.93.1")
            for name, value in (("old-source", "14"), ("source", "15")):
                source = target / name
                (source / "src").mkdir(parents=True)
                (source / "Cargo.toml").write_text(
                    '[package]\nname="taira-cache-fixture"\nversion="0.1.0"\nedition="2024"\n[workspace]\n')
                (source / "build.rs").write_text(
                    'fn main() { println!("cargo:rustc-env=SOURCE_VALUE=' + value + '"); }\n')
                (source / "src/main.rs").write_text(
                    'fn main() { println!("{}", env!("SOURCE_VALUE")); }\n')
                subprocess.run(["cargo", "build", "--offline", "--release", "--manifest-path",
                                str(source / "Cargo.toml")], cwd=root, env=environment,
                               check=True, capture_output=True, timeout=60)
            binary = target / "release/taira-cache-fixture"
            # This reproduces Cargo 1.93's real stale build-root-relative record.
            self.assertEqual(subprocess.check_output([str(binary)]).strip(), b"14")
            self.assertEqual(cache.admit_source_fingerprints(source, target, "aarch64-unknown-linux-gnu",
                                                             {"taira-cache-fixture"}), ["taira-cache-fixture"])
            subprocess.run(["cargo", "build", "--offline", "--release", "--manifest-path",
                            str(source / "Cargo.toml")], cwd=root, env=environment,
                           check=True, capture_output=True, timeout=60)
            self.assertEqual(subprocess.check_output([str(binary)]).strip(), b"15")
            self.assertEqual(cache.admit_source_fingerprints(source, target, "aarch64-unknown-linux-gnu",
                                                             {"taira-cache-fixture"}, repair=False), [])


if __name__ == "__main__":
    unittest.main()
