"""Real small Git/GPG source-artifact regressions; no Cargo, SSH or operator keys."""

from __future__ import annotations

import copy
import hashlib
import importlib.util
import os
from pathlib import Path
import shutil
import stat
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

SCRIPTS = Path(__file__).resolve().parents[2] / "scripts"
sys.path.insert(0, str(SCRIPTS))
import taira_source_capture as source


@unittest.skipUnless(shutil.which("git") and shutil.which("gpg"), "Git and GnuPG are required")
class SignedSourceCaptureTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.workspace = tempfile.TemporaryDirectory(prefix=".taira-source-test-", dir=Path.home())
        cls.base = Path(cls.workspace.name).resolve()
        cls.keyring = cls.base / "public-fixture-keyring"
        cls.keyring.mkdir(mode=0o700)
        cls.environment = dict(os.environ, GNUPGHOME=str(cls.keyring), LC_ALL="C", GIT_CONFIG_NOSYSTEM="1",
                               GIT_CONFIG_GLOBAL="/dev/null")
        cls.gpg = str(Path(shutil.which("gpg")).resolve())
        cls.command([cls.gpg, "--batch", "--pinentry-mode", "loopback", "--passphrase", "",
                     "--quick-generate-key", "Source Fixture <source@example.invalid>", "ed25519", "sign", "0"])
        listing = cls.command([cls.gpg, "--batch", "--with-colons", "--list-keys"]).decode()
        cls.fingerprint = next(line.split(":")[9] for line in listing.splitlines() if line.startswith("fpr:"))

    @classmethod
    def tearDownClass(cls):
        # Stop only the isolated fixture's agent, using its exact private keyring.
        if shutil.which("gpgconf"):
            subprocess.run([shutil.which("gpgconf"), "--homedir", str(cls.keyring), "--kill", "gpg-agent"],
                           env=cls.environment, check=False, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        cls.workspace.cleanup()

    @classmethod
    def command(cls, argv, *, cwd=None, payload=None):
        result = subprocess.run(argv, cwd=cwd, input=payload, env=cls.environment, capture_output=True,
                                check=False, timeout=60, umask=0o077)
        if result.returncode:
            raise AssertionError((argv, result.returncode, result.stderr.decode(errors="replace")))
        return result.stdout

    def git(self, *args, payload=None):
        return self.command(["git", "-c", f"gpg.program={self.gpg}", *args], cwd=self.repo, payload=payload)

    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="case-", dir=self.base)
        self.addCleanup(self.temporary.cleanup)
        self.case = Path(self.temporary.name)
        self.repo = self.case / "repo"
        self.repo.mkdir(mode=0o755)
        self.git("init", "-b", "optimizations", "--template=")
        self.git("config", "user.name", "Fixture")
        self.git("config", "user.email", "fixture@example.invalid")
        self.git("config", "user.signingkey", self.fingerprint + "!")
        self.git("config", "commit.gpgsign", "true")
        (self.repo / "old-only").write_text("must not enter the exported object closure\n")
        self.git("add", "old-only")
        self.git("commit", "-m", "parent")
        self.parent = self.git("rev-parse", "HEAD").decode().strip()
        self.old_blob = self.git("rev-parse", "HEAD:old-only").decode().strip()
        self.git("rm", "old-only")
        (self.repo / "Cargo.lock").write_bytes(b"version = 4\n")
        (self.repo / "nested").mkdir(mode=0o755)
        (self.repo / "nested" / "data").write_bytes(b"signed source\0with bytes\n")
        (self.repo / "tool.sh").write_bytes(b"#!/bin/sh\nexit 0\n")
        (self.repo / "tool.sh").chmod(0o755)
        (self.repo / "sdk-link").symlink_to("missing-output.so")
        (self.repo / "empty").touch()
        (self.repo / "empty-executable").touch(mode=0o755)
        self.git("add", "Cargo.lock", "nested/data", "tool.sh", "sdk-link", "empty", "empty-executable")
        self.git("update-index", "--add", "--cacheinfo", "160000," + "b" * 40 + ",vendor/dependency")
        self.git("commit", "-m", "selected signed source")
        self.commit = self.git("rev-parse", "HEAD").decode().strip()
        self.tree = self.git("rev-parse", "HEAD^{tree}").decode().strip()
        self.capture = self.case / "capture"
        self.imported = self.case / "imported-source"
        self.environment_patch = mock.patch.dict(os.environ, {"GNUPGHOME": str(self.keyring)})
        self.environment_patch.start()
        self.addCleanup(self.environment_patch.stop)

    def export(self):
        return source.export_source(self.repo, self.commit, self.tree, self.fingerprint, self.capture)

    def import_capture(self):
        return source.import_source(self.capture / "source.pack", self.capture / "source-capture.json",
                                    self.commit, self.tree, self.fingerprint, self.imported)

    def verify(self):
        return source.verify_import(self.imported, self.capture / "source-capture.json",
                                    self.commit, self.tree, self.fingerprint)

    def manifest(self):
        return source.load_json_object((self.capture / "source-capture.json").read_bytes(), "test capture")

    def rewrite_manifest(self, value):
        path = self.capture / "source-capture.json"
        path.chmod(0o600)
        path.write_bytes(source.canonical_json_bytes(value))
        path.chmod(0o400)

    def test_signed_exact_tree_roundtrip_excludes_dirty_untracked_history_and_gitlink_objects(self):
        (self.repo / "nested/data").write_bytes(b"uncommitted work must remain untouched")
        (self.repo / "untracked-test.py").write_text("untouched\n")
        before_status = self.git("status", "--porcelain=v1", "--untracked-files=all")
        before_index = (self.repo / ".git/index").read_bytes()
        result = self.export()
        self.assertEqual(before_status, self.git("status", "--porcelain=v1", "--untracked-files=all"))
        self.assertEqual(before_index, (self.repo / ".git/index").read_bytes())
        self.assertEqual((self.repo / "nested/data").read_bytes(), b"uncommitted work must remain untouched")
        self.assertEqual((self.repo / "untracked-test.py").read_text(), "untouched\n")
        self.assertEqual(result["file_count"], 7)
        self.assertGreater(result["object_count"], 7)
        manifest = self.manifest()
        ids = {row["object"] for row in manifest["objects"]}
        self.assertTrue({self.parent, self.old_blob, "b" * 40}.isdisjoint(ids))
        facts = self.import_capture()
        self.assertEqual(facts, self.verify())
        for key in ("clean", "signature_verified", "object_inventory_verified"):
            self.assertIs(facts[key], True)
        for key in ("history_included", "runtime_files_transferred", "runtime_files_included", "activated"):
            self.assertIs(facts[key], False)
        self.assertEqual(facts["signer_fingerprint"], self.fingerprint)
        self.assertEqual((self.imported / "nested/data").read_bytes(), b"signed source\0with bytes\n")
        self.assertEqual(stat.S_IMODE((self.imported / "tool.sh").stat().st_mode), 0o755)
        self.assertEqual(stat.S_IMODE((self.imported / "Cargo.lock").stat().st_mode), 0o644)
        self.assertEqual((self.imported / "empty").read_bytes(), b"")
        self.assertEqual((self.imported / "empty-executable").read_bytes(), b"")
        self.assertEqual(stat.S_IMODE((self.imported / "empty").stat().st_mode), 0o644)
        self.assertEqual(stat.S_IMODE((self.imported / "empty-executable").stat().st_mode), 0o755)
        self.assertEqual(os.readlink(self.imported / "sdk-link"), "missing-output.so")
        self.assertEqual(list((self.imported / "vendor/dependency").iterdir()), [])
        self.assertEqual((self.imported / ".git/shallow").read_text(), self.commit + "\n")
        self.assertEqual(source._git(self.imported, "rev-list", "--count", "HEAD").strip(), b"1")
        self.assertFalse(source._git(self.imported, "status", "--porcelain=v1", "--untracked-files=all"))

    def test_empty_signed_source_rejects_growth_and_same_length_path_substitution(self):
        self.export()
        self.import_capture()
        path = self.imported / "empty"
        expected = {"size": 0, "sha256": hashlib.sha256(b"").hexdigest()}
        original = os.pread
        for mutation in ("growth", "replacement"):
            with self.subTest(mutation=mutation):
                changed = False
                before = path.stat()
                def mutate(fd, count, offset):
                    nonlocal changed
                    result = original(fd, count, offset)
                    info = os.fstat(fd)
                    if not changed and (info.st_dev, info.st_ino) == (before.st_dev, before.st_ino):
                        changed = True
                        if mutation == "growth":
                            path.write_bytes(b"new bytes")
                        else:
                            replacement = self.case / "replacement"
                            replacement.write_bytes(b"")
                            replacement.chmod(0o644)
                            os.replace(replacement, path)
                    return result
                with mock.patch.object(source.os, "pread", side_effect=mutate):
                    with self.assertRaisesRegex(source.SourceCaptureError, "empty source file.*changed"):
                        source._verify_source_file(path, expected, 0o644)
                self.assertTrue(changed)
                path.write_bytes(b"")
                path.chmod(0o644)
        alias = self.case / "empty-alias"
        os.link(path, alias)
        with self.assertRaisesRegex(source.SourceCaptureError, "empty source file custody"):
            source._verify_source_file(path, expected, 0o644)
        alias.unlink()
        with self.assertRaisesRegex(source.ReleaseArtifactError, "must not be empty"):
            source.stable_hash_path(path)
        self.verify()

    def test_unsigned_commit_wrong_signer_tree_and_branch_refuse(self):
        for kind in ("unsigned", "signer", "tree", "branch"):
            with self.subTest(kind=kind):
                commit, tree, signer = self.commit, self.tree, self.fingerprint
                if kind == "unsigned":
                    self.git("-c", "commit.gpgsign=false", "commit", "--allow-empty", "-m", "unsigned")
                    commit = self.git("rev-parse", "HEAD").decode().strip()
                elif kind == "signer":
                    signer = "A" * 40
                elif kind == "tree":
                    tree = "c" * 40
                else:
                    self.git("symbolic-ref", "HEAD", "refs/heads/not-optimizations")
                with self.assertRaises(source.SourceCaptureError):
                    source.export_source(self.repo, commit, tree, signer, self.capture)
                self.assertFalse(self.capture.exists())

    def test_corrupted_pack_extra_object_and_history_objects_refuse_before_publication(self):
        self.export()
        original = (self.capture / "source.pack").read_bytes()
        manifest = self.manifest()
        path = self.capture / "source.pack"
        for kind in ("corrupt", "extra", "history", "declared-extra", "declared-history"):
            with self.subTest(kind=kind):
                current = copy.deepcopy(manifest)
                if kind == "corrupt":
                    payload = original[:-1] + bytes([original[-1] ^ 1])
                else:
                    oid = self.git("hash-object", "-w", "--stdin", payload=b"unrelated object").decode().strip() if kind.endswith("extra") else self.parent
                    ids = [row["object"] for row in manifest["objects"]] + [oid]
                    payload = self.git("pack-objects", "--stdout", "--window=0", payload=("\n".join(ids) + "\n").encode())
                    # Even re-pinning the transport digest cannot authorize another object.
                    if kind.startswith("declared-"):
                        object_type = self.git("cat-file", "-t", oid).decode().strip()
                        raw = self.git("cat-file", object_type, oid)
                        current["objects"].append({"object": oid, "type": object_type,
                                                   "size": len(raw), "sha256": hashlib.sha256(raw).hexdigest()})
                        current["objects"].sort(key=lambda row: row["object"])
                current["pack"] = {"size": len(payload), "sha256": hashlib.sha256(payload).hexdigest()}
                path.chmod(0o600)
                path.write_bytes(payload)
                path.chmod(0o400)
                self.rewrite_manifest(current)
                with self.assertRaises(source.SourceCaptureError):
                    self.import_capture()
                self.assertFalse(self.imported.exists())

    def test_pack_inflation_and_delta_refuse_before_git_unpack(self):
        self.export()
        baseline = self.manifest()
        original_git = source._git
        for kind in ("oversized", "delta"):
            with self.subTest(kind=kind):
                manifest = copy.deepcopy(baseline)
                if kind == "delta":
                    object_header = b"\x60"
                else:
                    size = source.MAX_OBJECT_BYTES + 1
                    first, size = 0x30 | (size & 15), size >> 4
                    header = bytearray([first | (128 if size else 0)])
                    while size:
                        value, size = size & 127, size >> 7
                        header.append(value | (128 if size else 0))
                    object_header = bytes(header)
                body = b"PACK" + source.struct.pack(">II", 2, len(manifest["objects"])) + object_header + source.zlib.compress(b"")
                payload = body + hashlib.sha1(body).digest()
                path = self.capture / "source.pack"
                path.chmod(0o600)
                path.write_bytes(payload)
                path.chmod(0o400)
                manifest["pack"] = {"size": len(payload), "sha256": hashlib.sha256(payload).hexdigest()}
                self.rewrite_manifest(manifest)
                def no_unpack(root, *args, **kwargs):
                    self.assertNotIn("index-pack", args)
                    return original_git(root, *args, **kwargs)
                with mock.patch.object(source, "_git", side_effect=no_unpack):
                    with self.assertRaisesRegex(source.SourceCaptureError, "bound|without deltas"):
                        self.import_capture()
                self.assertFalse(self.imported.exists())

    def test_streamed_multichunk_object_roundtrips(self):
        payload = b"bounded source object\0" * 150_000
        (self.repo / "large").write_bytes(payload)
        self.git("add", "large")
        self.git("commit", "-m", "multi-chunk signed source")
        self.commit = self.git("rev-parse", "HEAD").decode().strip()
        self.tree = self.git("rev-parse", "HEAD^{tree}").decode().strip()
        self.export()
        self.import_capture()
        self.assertEqual((self.imported / "large").read_bytes(), payload)
        self.verify()

    def test_transport_mutation_during_import_prevents_publication(self):
        self.export()
        original = source._materialize
        def mutate(root, manifest):
            original(root, manifest)
            path = self.capture / "source.pack"
            path.chmod(0o600)
            with path.open("r+b") as output:
                output.seek(-1, os.SEEK_END)
                output.write(b"x")
        with mock.patch.object(source, "_materialize", side_effect=mutate):
            with self.assertRaisesRegex(source.SourceCaptureError, "source input changed"):
                self.import_capture()
        self.assertFalse(self.imported.exists())

    def test_exclusive_publication_preserves_racing_destination(self):
        self.export()
        original = source._publish
        def collide(stage, destination):
            self.assertEqual(destination, self.imported)
            destination.mkdir(mode=0o700)
            (destination / "keep").write_bytes(b"other owner result")
            return original(stage, destination)
        with mock.patch.object(source, "_publish", side_effect=collide):
            with self.assertRaisesRegex(source.SourceCaptureError, "exclusive source publication failed"):
                self.import_capture()
        self.assertEqual((self.imported / "keep").read_bytes(), b"other owner result")
        self.assertEqual(set(self.imported.iterdir()), {self.imported / "keep"})

    def test_manifest_path_identity_census_and_duplicate_json_refuse(self):
        self.export()
        baseline = self.manifest()
        for mutate in (
            lambda m: m["entries"][0].update(path="../outside"),
            lambda m: m["entries"][0].update(path=".git/config"),
            lambda m: m["entries"][0].update(path="target/runtime-key"),
            lambda m: m["entries"][0].update(object="c" * 40),
            lambda m: m["objects"][0].update(sha256="0" * 64),
            lambda m: m.update(source_bytes=m["source_bytes"] + 1),
            lambda m: m.update(entries=m["entries"][:-1]),
        ):
            current = copy.deepcopy(baseline)
            mutate(current)
            self.rewrite_manifest(current)
            with self.assertRaises(source.ReleaseArtifactError):
                self.import_capture()
            self.assertFalse(self.imported.exists())
        path = self.capture / "source-capture.json"
        path.chmod(0o600)
        path.write_bytes(b'{"schema":"x","schema":"y"}\n')
        with self.assertRaises(source.ReleaseArtifactError):
            self.import_capture()

    def test_public_signer_envelope_rejects_generated_secret_key_material(self):
        self.export()
        manifest = self.manifest()
        # Only this test's disposable generated key is exported. Operator keys
        # are never opened, exported, or referenced by any test.
        secret = self.command([self.gpg, "--batch", "--pinentry-mode", "loopback", "--passphrase", "",
                               "--export-secret-keys", self.fingerprint])
        manifest["public_key_base64"] = source.base64.b64encode(secret).decode()
        self.rewrite_manifest(manifest)
        with self.assertRaisesRegex(source.SourceCaptureError, "only complete public key packets"):
            self.import_capture()
        self.assertFalse(self.imported.exists())

    def test_existing_destination_and_symlink_or_hardlinked_inputs_refuse(self):
        self.export()
        self.imported.mkdir(mode=0o755)
        (self.imported / "keep").write_text("preserve")
        with self.assertRaisesRegex(source.SourceCaptureError, "already exists"):
            self.import_capture()
        self.assertEqual((self.imported / "keep").read_text(), "preserve")
        for filename in ("source.pack", "source-capture.json"):
            path = self.capture / filename
            alias = self.case / (filename + ".alias")
            os.link(path, alias)
            with self.assertRaises(source.ReleaseArtifactError):
                self.import_capture()
            alias.unlink()
            alias.symlink_to(path)
            with self.assertRaises(source.ReleaseArtifactError):
                source.import_source(alias if filename == "source.pack" else self.capture / "source.pack",
                                     alias if filename != "source.pack" else self.capture / "source-capture.json",
                                     self.commit, self.tree, self.fingerprint, self.case / "other")
            alias.unlink()

    def test_completed_verification_rejects_untracked_changed_mode_git_control_and_hardlink(self):
        self.export()
        self.import_capture()
        target = self.imported / "Cargo.lock"
        original = target.read_bytes()
        for kind in ("untracked", "bytes", "mode", "hardlink", "control", "gitlink"):
            with self.subTest(kind=kind):
                extra = self.imported / "runtime-secret"
                control = self.imported / ".git/config"
                original_control = control.read_bytes()
                if kind == "untracked":
                    extra.write_text("not admitted")
                elif kind == "bytes":
                    target.write_bytes(b"changed!")
                elif kind == "mode":
                    target.chmod(0o755)
                elif kind == "hardlink":
                    os.link(target, self.case / "alias")
                elif kind == "control":
                    control.write_bytes(original_control + b"[core]\n\thooksPath = /bad\n")
                else:
                    (self.imported / "vendor/dependency/runtime").write_bytes(b"not a source input")
                with self.assertRaises(source.ReleaseArtifactError):
                    self.verify()
                if kind == "untracked":
                    extra.unlink()
                elif kind in {"bytes", "mode"}:
                    target.write_bytes(original)
                    target.chmod(0o644)
                elif kind == "hardlink":
                    (self.case / "alias").unlink()
                elif kind == "control":
                    control.write_bytes(original_control)
                else:
                    (self.imported / "vendor/dependency/runtime").unlink()
        self.verify()

    def test_signed_escaping_or_present_symlinks_refuse(self):
        for target in ("../../outside", "/etc/passwd", "Cargo.lock", "nested"):
            with self.subTest(target=target):
                (self.repo / "sdk-link").unlink()
                (self.repo / "sdk-link").symlink_to(target)
                self.git("add", "sdk-link")
                self.git("commit", "-m", "invalid source link")
                commit = self.git("rev-parse", "HEAD").decode().strip()
                tree = self.git("rev-parse", "HEAD^{tree}").decode().strip()
                with self.assertRaises(source.SourceCaptureError):
                    source.export_source(self.repo, commit, tree, self.fingerprint, self.capture)
                self.assertFalse(self.capture.exists())

    def test_permissive_umask_creates_canonical_modes_without_changing_parent_umask(self):
        previous = os.umask(0o002)
        try:
            self.export()
            self.import_capture()
            observed = os.umask(0o002)
            self.assertEqual(observed, 0o002)
            self.assertEqual(stat.S_IMODE((self.capture / "source.pack").stat().st_mode), 0o400)
            self.assertEqual(stat.S_IMODE(self.imported.stat().st_mode), 0o755)
            self.verify()
        finally:
            os.umask(previous)

    def test_signing_subkey_is_matched_as_exact_git_GF_not_primary_fingerprint(self):
        self.command([self.gpg, "--batch", "--pinentry-mode", "loopback", "--passphrase", "",
                      "--quick-add-key", self.fingerprint, "ed25519", "sign", "0"])
        listing = self.command([self.gpg, "--batch", "--with-colons", "--list-keys"]).decode()
        subkey = [line.split(":")[9] for line in listing.splitlines() if line.startswith("fpr:")][-1]
        self.git("-c", "user.signingkey=" + subkey + "!", "commit", "--allow-empty", "-m", "subkey signature")
        commit = self.git("rev-parse", "HEAD").decode().strip()
        self.assertEqual(self.git("show", "--no-patch", "--format=%GF", commit).decode().strip(), subkey)
        self.assertEqual(self.git("show", "--no-patch", "--format=%GP", commit).decode().strip(), self.fingerprint)
        with self.assertRaisesRegex(source.SourceCaptureError, "expected signing fingerprint"):
            source.export_source(self.repo, commit, self.tree, self.fingerprint, self.capture)
        source.export_source(self.repo, commit, self.tree, subkey, self.capture)
        facts = source.import_source(self.capture / "source.pack", self.capture / "source-capture.json",
                                     commit, self.tree, subkey, self.imported)
        self.assertEqual(facts["signer_fingerprint"], subkey)


if __name__ == "__main__":
    unittest.main()
