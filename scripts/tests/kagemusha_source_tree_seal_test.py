from __future__ import annotations

import base64
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import stat
import subprocess
import sys
import tempfile
import unittest
from unittest import mock


SCRIPT = Path(__file__).parents[1] / "kagemusha_source_tree_seal.py"
SPEC = importlib.util.spec_from_file_location("kagemusha_source_tree_seal", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
seal = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = seal
SPEC.loader.exec_module(seal)


class KagemushaSourceTreeSealTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.review_directory = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.review_root = Path(self.review_directory.name).resolve(strict=True)
        self.git("init", "-q")
        self.git("config", "user.name", "Kagemusha Test")
        self.git("config", "user.email", "kagemusha@example.invalid")
        (self.root / "Cargo.lock").write_text(
            "# fixture lockfile consumed by --locked\n", encoding="utf-8"
        )
        self.signature_verifier = mock.patch.object(
            seal, "_verify_signed_commit", side_effect=self.fake_verified_authority
        )
        self.signature_verifier.start()

    def tearDown(self) -> None:
        self.signature_verifier.stop()
        self.review_directory.cleanup()
        self.temporary.cleanup()

    def git(self, *arguments: str, input_bytes: bytes | None = None) -> bytes:
        environment = os.environ.copy()
        environment["GIT_CONFIG_GLOBAL"] = os.devnull
        environment["GIT_CONFIG_NOSYSTEM"] = "1"
        for name in tuple(environment):
            if name == "GIT_CONFIG_COUNT" or name.startswith(
                ("GIT_CONFIG_KEY_", "GIT_CONFIG_VALUE_")
            ):
                del environment[name]
        return subprocess.run(
            ["git", "-C", str(self.root), *arguments],
            check=True,
            input=input_bytes,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env=environment,
        ).stdout

    def commit(self) -> None:
        self.git("add", "-A")
        self.git("commit", "-q", "-m", "fixture")

    @staticmethod
    def fixture_allowed_signers() -> bytes:
        """Return one structurally valid canonical Ed25519 allowed-signers row."""

        key_type = b"ssh-ed25519"
        key_blob = (
            len(key_type).to_bytes(4, "big")
            + key_type
            + (32).to_bytes(4, "big")
            + b"K" * 32
        )
        return b"reviewer " + key_type + b" " + base64.b64encode(key_blob) + b"\n"

    def fake_verified_authority(
        self, root: Path, commit: bytes
    ) -> object:
        """Derive real commit facts while substituting only the SSH verification."""

        raw_commit = seal._git(root, "cat-file", "commit", commit.decode("ascii"))
        return seal._source_authority_from_verified_commit(
            root,
            commit.decode("ascii"),
            raw_commit,
            seal.VerifiedSshSignature(
                principal="reviewer",
                public_key_sha256="1" * 64,
                allowed_signers_sha256="2" * 64,
                revocation_sha256="3" * 64,
            ),
        )

    def reviewed_identity(self) -> tuple[object, dict[str, object], Path, str]:
        descriptor = seal.compute_observed_descriptor(self.root)
        payload = seal._canonical_json_bytes(descriptor)
        path = self.review_root / "reviewed-source-closure.json"
        path.write_bytes(payload)
        sha256 = hashlib.sha256(payload).hexdigest()
        identity = seal.compute_identity(self.root, str(path), sha256)
        return identity, descriptor, path, sha256

    def test_reviewed_clean_fingerprint_is_stable_and_binds_complete_tree(self) -> None:
        (self.root / "plain.txt").write_bytes(b"plain\n")
        executable = self.root / "tool.sh"
        executable.write_bytes(b"#!/bin/sh\nexit 0\n")
        executable.chmod(executable.stat().st_mode | stat.S_IXUSR)
        os.symlink("plain.txt", self.root / "plain-link")
        self.commit()

        identity, descriptor, path, pin = self.reviewed_identity()
        first = seal.compute_fingerprint(self.root, str(path), pin)
        self.assertEqual(first, seal.compute_fingerprint(self.root, str(path), pin))
        self.assertRegex(first, r"^[0-9a-f]{64}$")
        self.assertEqual(identity.source_tree_sha256, first)
        self.assertFalse(identity.source_repo_dirty)
        self.assertEqual(identity.reviewed_source_closure, descriptor)
        self.assertEqual(
            identity.source_commit,
            self.git("rev-parse", "--verify", "HEAD^{commit}").decode().strip(),
        )

        (self.root / "plain.txt").write_bytes(b"dirty change\n")
        with self.assertRaisesRegex(seal.SourceSealError, "blob differs"):
            seal.compute_fingerprint(self.root, str(path), pin)

    def test_racy_clean_regular_file_substitution_is_rejected_by_blob_oid(self) -> None:
        tracked = self.root / "tracked.txt"
        tracked.write_bytes(b"signed-A\n")
        self.commit()
        original = tracked.stat()

        tracked.write_bytes(b"forged-B\n")
        os.utime(tracked, ns=(original.st_atime_ns, original.st_mtime_ns))
        self.assertEqual(self.git(*seal.TRACKED_DIFF_ARGUMENTS), b"")
        with self.assertRaisesRegex(seal.SourceSealError, "blob differs"):
            seal.compute_observed_descriptor(self.root)

    def test_tracked_mode_substitution_is_rejected_against_index(self) -> None:
        tracked = self.root / "tracked.sh"
        tracked.write_bytes(b"#!/bin/sh\nexit 0\n")
        tracked.chmod(0o644)
        self.commit()

        tracked.chmod(0o755)
        self.assertEqual(self.git(*seal.TRACKED_DIFF_ARGUMENTS), b"")
        with self.assertRaisesRegex(seal.SourceSealError, "mode differs"):
            seal.compute_observed_descriptor(self.root)

    def test_tracked_symlink_substitution_is_rejected_by_blob_oid(self) -> None:
        (self.root / "first.txt").write_bytes(b"first\n")
        (self.root / "other.txt").write_bytes(b"other\n")
        link = self.root / "tracked-link"
        os.symlink("first.txt", link)
        self.commit()

        link.unlink()
        os.symlink("other.txt", link)
        self.assertEqual(self.git(*seal.TRACKED_DIFF_ARGUMENTS), b"")
        with self.assertRaisesRegex(seal.SourceSealError, "symlink blob differs"):
            seal.compute_observed_descriptor(self.root)

    def test_missing_tracked_file_is_rejected_even_when_index_matches_head(self) -> None:
        tracked = self.root / "tracked.txt"
        tracked.write_bytes(b"signed\n")
        self.commit()

        tracked.unlink()
        self.assertEqual(self.git(*seal.TRACKED_DIFF_ARGUMENTS), b"")
        with self.assertRaisesRegex(seal.SourceSealError, "missing or unsafe"):
            seal.compute_observed_descriptor(self.root)

    def test_symlink_ancestor_cannot_escape_descriptor_root(self) -> None:
        nested = self.root / "nested"
        nested.mkdir()
        (nested / "tracked.txt").write_bytes(b"signed\n")
        self.commit()

        external = self.review_root / "external"
        external.mkdir()
        (external / "tracked.txt").write_bytes(b"signed\n")
        (nested / "tracked.txt").unlink()
        nested.rmdir()
        os.symlink(external, nested)
        self.assertEqual(self.git(*seal.TRACKED_DIFF_ARGUMENTS), b"")
        with mock.patch.object(seal, "_untracked_paths", return_value=[]):
            with self.assertRaisesRegex(
                seal.SourceSealError,
                "symlink ancestor|ancestor is missing or unsafe",
            ):
                seal.compute_observed_descriptor(self.root)

    def test_clean_tree_is_the_only_admitted_closure(self) -> None:
        (self.root / "tracked.txt").write_text("tracked\n", encoding="utf-8")
        self.commit()
        descriptor = seal.compute_observed_descriptor(self.root)
        self.assertFalse(descriptor["source_repo_dirty"])
        self.assertEqual(descriptor["tracked_binary_diff_sha256"], seal.EMPTY_SHA256)
        self.assertEqual(descriptor["untracked_file_count"], 0)
        self.assertEqual(descriptor["untracked_path_mode_blob_oid_manifest"], [])
        self.assertEqual(
            descriptor["untracked_path_mode_blob_oid_manifest_sha256"],
            seal.EMPTY_SHA256,
        )
        payload = seal._canonical_json_bytes(descriptor)
        path = self.review_root / "clean.json"
        path.write_bytes(payload)
        identity = seal.compute_identity(
            self.root,
            str(path),
            hashlib.sha256(payload).hexdigest(),
        )
        self.assertFalse(identity.source_repo_dirty)

    def test_identity_derives_exact_raw_commit_authority(self) -> None:
        (self.root / "tracked.txt").write_text("parent\n", encoding="utf-8")
        self.commit()
        parent = self.git("rev-parse", "HEAD").decode().strip()
        parent_tree = self.git("rev-parse", "HEAD^{tree}").decode().strip()
        (self.root / "tracked.txt").write_text("child\n", encoding="utf-8")
        self.commit()

        identity, _, _, _ = self.reviewed_identity()
        authority = identity.source_authority
        raw_commit = self.git("cat-file", "commit", authority.commit)
        self.assertEqual(authority.commit, identity.source_commit)
        self.assertEqual(
            authority.commit_object_sha256, hashlib.sha256(raw_commit).hexdigest()
        )
        self.assertEqual(authority.commit_object_size, len(raw_commit))
        self.assertEqual(
            authority.git_tree,
            self.git("rev-parse", "HEAD^{tree}").decode().strip(),
        )
        self.assertEqual(authority.ordered_parents, (parent,))
        self.assertEqual(authority.ordered_parent_trees, (parent_tree,))
        committer_epoch = int(
            self.git("show", "-s", "--format=%ct", "HEAD").decode().strip()
        )
        self.assertEqual(authority.committer_epoch, committer_epoch)
        self.assertEqual(authority.signature.principal, "reviewer")

    def test_descriptor_pin_and_canonical_bytes_are_mandatory(self) -> None:
        (self.root / "tracked.txt").write_text("base\n", encoding="utf-8")
        self.commit()
        _, descriptor, path, pin = self.reviewed_identity()

        with self.assertRaisesRegex(seal.SourceSealError, "differs from its pin"):
            seal.compute_identity(self.root, str(path), "f" * 64)

        path.write_text(json.dumps(descriptor, indent=2) + "\n", encoding="ascii")
        pretty_pin = hashlib.sha256(path.read_bytes()).hexdigest()
        with self.assertRaisesRegex(seal.SourceSealError, "not canonical"):
            seal.compute_identity(self.root, str(path), pretty_pin)

        path.write_bytes(seal._canonical_json_bytes(descriptor))
        self.assertEqual(
            seal.compute_identity(self.root, str(path), pin).source_repo_dirty,
            False,
        )
        with self.assertRaisesRegex(seal.SourceSealError, "absolute and normalized"):
            seal.compute_identity(self.root, "-", pin)

    def test_reviewed_descriptor_rejects_a_symlink_ancestor(self) -> None:
        real = self.review_root / "real"
        real.mkdir()
        descriptor = real / "reviewed.json"
        descriptor.write_bytes(b"{}\n")
        alias = self.review_root / "alias"
        os.symlink(real, alias)

        with self.assertRaisesRegex(seal.SourceSealError, "traverses a symlink"):
            seal._read_descriptor_payload(str(alias / descriptor.name))

    def test_reviewed_descriptor_tolerates_unrelated_ancestor_entry_churn(self) -> None:
        descriptor = self.review_root / "reviewed.json"
        descriptor.write_bytes(b"{}\n")
        real_read = seal.os.read
        churned = False

        def read_with_sibling_churn(file_descriptor: int, size: int) -> bytes:
            nonlocal churned
            if not churned:
                churned = True
                (self.review_root / "unrelated-sibling").write_bytes(b"churn\n")
            return real_read(file_descriptor, size)

        with mock.patch.object(seal.os, "read", side_effect=read_with_sibling_churn):
            self.assertEqual(seal._read_descriptor_payload(str(descriptor)), b"{}\n")

    def test_staged_and_untracked_bytes_are_rejected(self) -> None:
        (self.root / "tracked.txt").write_text("tracked\n", encoding="utf-8")
        self.commit()
        (self.root / "staged.txt").write_text("staged\n", encoding="utf-8")
        self.git("add", "staged.txt")
        with self.assertRaisesRegex(seal.SourceSealError, "empty tracked diff"):
            seal.compute_observed_descriptor(self.root)
        self.commit()

        (self.root / "untracked.txt").write_text("untracked\n", encoding="utf-8")
        with self.assertRaisesRegex(seal.SourceSealError, "file-count bound"):
            seal.compute_observed_descriptor(self.root)

    def test_source_tree_and_descriptor_bind_tracked_root_cargo_lock(self) -> None:
        (self.root / "tracked.txt").write_text("tracked\n", encoding="utf-8")
        self.commit()
        identity, descriptor, path, pin = self.reviewed_identity()
        initial_lock = (self.root / "Cargo.lock").read_bytes()
        self.assertEqual(descriptor["tracked_cargo_lock_size_bytes"], len(initial_lock))
        self.assertEqual(
            descriptor["tracked_cargo_lock_sha256"],
            hashlib.sha256(initial_lock).hexdigest(),
        )

        (self.root / "Cargo.lock").write_text(
            "# changed tracked lockfile consumed by --locked\n", encoding="utf-8"
        )
        self.assertNotEqual(seal.status(self.root), b"")
        with self.assertRaisesRegex(seal.SourceSealError, "blob differs"):
            seal.compute_identity(self.root, str(path), pin)
        self.commit()
        replacement = seal.compute_observed_descriptor(self.root)
        self.assertNotEqual(
            identity.source_tree_sha256,
            replacement["source_tree_sha256"],
        )
        self.assertNotEqual(
            descriptor["tracked_cargo_lock_sha256"],
            replacement["tracked_cargo_lock_sha256"],
        )

    def test_reviewed_descriptor_rejects_retired_ignored_cargo_lock_keys(self) -> None:
        (self.root / "tracked.txt").write_text("tracked\n", encoding="utf-8")
        self.commit()
        descriptor = seal.compute_observed_descriptor(self.root)
        descriptor["ignored_cargo_lock_size_bytes"] = descriptor.pop(
            "tracked_cargo_lock_size_bytes"
        )
        descriptor["ignored_cargo_lock_sha256"] = descriptor.pop(
            "tracked_cargo_lock_sha256"
        )

        with self.assertRaisesRegex(seal.SourceSealError, "keys are not exact"):
            seal._validate_descriptor(descriptor, descriptor["base_commit"])

    def test_requires_exactly_one_stage_zero_tracked_mode_100644_root_cargo_lock(
        self,
    ) -> None:
        (self.root / "tracked.txt").write_text("tracked\n", encoding="utf-8")
        self.commit()

        self.git("rm", "-q", "Cargo.lock")
        self.commit()
        with self.assertRaisesRegex(seal.SourceSealError, "exactly one stage-0 tracked"):
            seal.compute_observed_descriptor(self.root)

        (self.root / "Cargo.lock").symlink_to("tracked.txt")
        self.commit()
        with self.assertRaisesRegex(seal.SourceSealError, "mode 100644"):
            seal.compute_observed_descriptor(self.root)

        (self.root / "Cargo.lock").unlink()
        (self.root / "Cargo.lock").write_text("tracked executable lock\n", encoding="utf-8")
        (self.root / "Cargo.lock").chmod(0o755)
        self.commit()
        with self.assertRaisesRegex(seal.SourceSealError, "mode 100644"):
            seal.compute_observed_descriptor(self.root)

    def test_rejects_non_stage_zero_root_cargo_lock_index_entries(self) -> None:
        (self.root / "tracked.txt").write_text("tracked\n", encoding="utf-8")
        self.commit()
        lock_blob = self.git("rev-parse", "HEAD:Cargo.lock").strip()
        self.git("update-index", "--force-remove", "Cargo.lock")
        self.git(
            "update-index",
            "--index-info",
            input_bytes=(
                b"100644 " + lock_blob + b" 1\tCargo.lock\n"
                b"100644 " + lock_blob + b" 2\tCargo.lock\n"
            ),
        )
        with self.assertRaisesRegex(seal.SourceSealError, "unresolved merge stage"):
            seal._index_entries(self.root)

    def test_rejects_symlinked_worktree_root_cargo_lock(self) -> None:
        (self.root / "tracked.txt").write_text("tracked\n", encoding="utf-8")
        self.commit()
        (self.root / "Cargo.lock").unlink()
        os.symlink("tracked.txt", self.root / "Cargo.lock")
        with self.assertRaisesRegex(
            seal.SourceSealError, "empty tracked diff|regular file|mode differs"
        ):
            seal.compute_observed_descriptor(self.root)

    def test_rejects_any_additional_ignored_source(self) -> None:
        (self.root / "tracked.txt").write_text("tracked\n", encoding="utf-8")
        (self.root / ".gitignore").write_text("/hidden-input.bin\n", encoding="utf-8")
        self.commit()
        (self.root / "hidden-input.bin").write_bytes(b"unbound ignored input\n")
        with self.assertRaisesRegex(seal.SourceSealError, "ignored source set must be empty"):
            seal.compute_observed_descriptor(self.root)

    def test_rejects_hardlinked_tracked_source(self) -> None:
        first = self.root / "first.txt"
        first.write_bytes(b"same inode\n")
        os.link(first, self.root / "second.txt")
        self.commit()
        with self.assertRaisesRegex(seal.SourceSealError, "singly linked"):
            seal.compute_observed_descriptor(self.root)

    def test_gitlink_binds_index_commit_and_requires_present_empty_directory(self) -> None:
        source = SCRIPT.read_text(encoding="utf-8")
        self.assertNotIn("gitlink must be absent", source)
        self.assertNotIn("Gitlinks bind their exact index commit and may only be absent", source)
        (self.root / "tracked.txt").write_text("first\n", encoding="utf-8")
        self.commit()
        first_commit = self.git("rev-parse", "HEAD").decode().strip()
        (self.root / "tracked.txt").write_text("second\n", encoding="utf-8")
        self.commit()
        second_commit = self.git("rev-parse", "HEAD").decode().strip()
        self.git(
            "update-index",
            "--add",
            "--cacheinfo",
            f"160000,{first_commit},iroha-docs",
        )
        self.git("commit", "-q", "-m", "pin first gitlink")

        gitlink = self.root / "iroha-docs"
        gitlink.mkdir()
        empty_directory = seal.compute_observed_descriptor(self.root)
        gitlink.rmdir()
        with self.assertRaisesRegex(
            seal.SourceSealError,
            "empty tracked diff|empty non-symlink|missing or unsafe",
        ):
            seal.compute_observed_descriptor(self.root)
        gitlink.mkdir()

        self.git(
            "update-index",
            "--cacheinfo",
            f"160000,{second_commit},iroha-docs",
        )
        self.git("commit", "-q", "-m", "pin second gitlink")
        changed_commit = seal.compute_observed_descriptor(self.root)
        self.assertNotEqual(
            empty_directory["source_tree_sha256"],
            changed_commit["source_tree_sha256"],
        )

        (gitlink / "unbound.txt").write_text("unbound\n", encoding="utf-8")
        with self.assertRaisesRegex(seal.SourceSealError, "empty tracked diff|must be empty"):
            seal.compute_observed_descriptor(self.root)

    def test_gitlink_rejects_non_directory_substitutions(self) -> None:
        (self.root / "tracked.txt").write_text("tracked\n", encoding="utf-8")
        self.commit()
        commit = self.git("rev-parse", "HEAD").decode().strip()
        self.git(
            "update-index",
            "--add",
            "--cacheinfo",
            f"160000,{commit},iroha-docs",
        )
        self.git("commit", "-q", "-m", "pin gitlink")
        gitlink = self.root / "iroha-docs"

        gitlink.write_text("not a directory\n", encoding="utf-8")
        with self.assertRaisesRegex(seal.SourceSealError, "empty tracked diff|empty directory"):
            seal.compute_observed_descriptor(self.root)
        gitlink.unlink()

        os.symlink("tracked.txt", gitlink)
        with self.assertRaisesRegex(
            seal.SourceSealError, "empty tracked diff|empty directory|pinned Git failed"
        ):
            seal.compute_observed_descriptor(self.root)

    def test_root_must_be_exact_repository_root(self) -> None:
        (self.root / "nested").mkdir()
        (self.root / "tracked.txt").write_text("tracked\n", encoding="utf-8")
        self.commit()
        with self.assertRaisesRegex(seal.SourceSealError, "exact repository root"):
            seal.compute_observed_descriptor(self.root / "nested")

    def test_identity_rejects_head_change_during_capture(self) -> None:
        (self.root / "tracked.txt").write_text("tracked\n", encoding="utf-8")
        self.commit()
        original = seal._head(self.root)
        changed = b"f" * 40 if original != b"f" * 40 else b"e" * 40
        with mock.patch.object(seal, "_head", side_effect=[original, changed]):
            with self.assertRaisesRegex(seal.SourceSealError, "changed while sealing"):
                seal.compute_observed_descriptor(self.root)

    def test_signed_commit_verifier_overrides_repository_config_and_fails_closed(self) -> None:
        self.signature_verifier.stop()
        try:
            self.git("config", "gpg.format", "openpgp")
            self.git("config", "gpg.program", "/tmp/attacker-gpg")
            self.git(
                "config",
                "gpg.ssh.allowedSignersFile",
                "/tmp/attacker-allowed-signers",
            )
            accepted = subprocess.CompletedProcess([], 0, stdout=b"", stderr=b"")
            rejected = subprocess.CompletedProcess([], 1, stdout=b"", stderr=b"bad")
            with mock.patch.object(
                seal.subprocess,
                "run",
                side_effect=[accepted, rejected],
            ) as runner, mock.patch.object(
                seal,
                "_git",
                return_value=(
                    b"tree " + b"b" * 40 + b"\n"
                    b"committer Reviewer <reviewer@example.test> 1786749504 +0000\n"
                    b"gpgsig -----BEGIN SSH SIGNATURE-----\n"
                    b" payload\n"
                    b" -----END SSH SIGNATURE-----\n\nfixture\n"
                ),
            ), mock.patch.object(
                seal,
                "_load_signature_policy",
                return_value=seal.SignaturePolicy(
                    allowed_signers=self.fixture_allowed_signers(),
                    revocation=b"",
                ),
            ):
                commit = b"a" * 40
                seal._verify_signed_commit(self.root, commit)
                with self.assertRaisesRegex(seal.SourceSealError, "verifiable signature"):
                    seal._verify_signed_commit(self.root, commit)

            command = runner.call_args_list[0].args[0]
            self.assertEqual(command[0], os.fspath(seal.GIT))
            self.assertEqual(
                command[-3:],
                ["verify-commit", "--raw", "a" * 40],
            )
            assignments = {
                command[index + 1].split("=", 1)[0]: command[index + 1].split("=", 1)[1]
                for index, value in enumerate(command[:-1])
                if value == "-c" and "=" in command[index + 1]
            }
            self.assertEqual(assignments["gpg.format"], "ssh")
            self.assertEqual(assignments["gpg.minTrustLevel"], "fully")
            for key in (
                "gpg.ssh.program",
                "gpg.program",
                "gpg.openpgp.program",
                "gpg.x509.program",
            ):
                self.assertEqual(assignments[key], os.fspath(seal.SSH_KEYGEN))
            self.assertNotIn("/tmp/attacker-gpg", command)
            self.assertNotIn("/tmp/attacker-allowed-signers", command)
            environment = runner.call_args_list[0].kwargs["env"]
            self.assertEqual(environment["GIT_CONFIG_GLOBAL"], "/dev/null")
            self.assertEqual(environment["GIT_CONFIG_NOSYSTEM"], "1")
            self.assertEqual(environment["HOME"], "/var/empty")
            self.assertNotIn("GNUPGHOME", environment)
            self.assertFalse(runner.call_args_list[0].kwargs["check"])
            self.assertIs(
                runner.call_args_list[0].kwargs["stdout"], subprocess.DEVNULL
            )
            self.assertIs(runner.call_args_list[0].kwargs["stderr"], subprocess.PIPE)
        finally:
            self.signature_verifier = mock.patch.object(
                seal,
                "_verify_signed_commit",
                side_effect=self.fake_verified_authority,
            )
            self.signature_verifier.start()

    def test_signature_trust_path_is_read_only_from_user_global_config(self) -> None:
        completed = subprocess.CompletedProcess(
            [],
            0,
            stdout=b"/private/reviewer/allowed-signers\n",
            stderr=b"",
        )
        with mock.patch.object(
            seal.subprocess, "run", return_value=completed
        ) as runner, mock.patch.dict(os.environ, {"HOME": "/private/reviewer"}):
            path = seal._global_signature_config_path(
                "gpg.ssh.allowedSignersFile", required=True
            )

        self.assertEqual(path, Path("/private/reviewer/allowed-signers"))
        command = runner.call_args.args[0]
        self.assertEqual(
            command[-5:],
            [
                "config",
                "--global",
                "--path",
                "--get",
                "gpg.ssh.allowedSignersFile",
            ],
        )
        self.assertNotIn("-C", command)
        environment = runner.call_args.kwargs["env"]
        self.assertNotIn("GIT_CONFIG_GLOBAL", environment)
        self.assertEqual(environment["GIT_CONFIG_NOSYSTEM"], "1")
        self.assertEqual(environment["HOME"], "/private/reviewer")

    def test_ssh_signed_commit_verifies_end_to_end_with_isolated_user_policy(self) -> None:
        self.signature_verifier.stop()
        try:
            home = self.review_root / "signature-home"
            home.mkdir(mode=0o700)
            key = home / "signing-key"
            subprocess.run(
                [
                    os.fspath(seal.SSH_KEYGEN),
                    "-q",
                    "-t",
                    "ed25519",
                    "-N",
                    "",
                    "-f",
                    str(key),
                ],
                check=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            )
            allowed = home / "allowed-signers"
            allowed.write_bytes(b"reviewer " + key.with_suffix(".pub").read_bytes())
            allowed.chmod(0o600)
            global_environment = os.environ.copy()
            global_environment["HOME"] = str(home)
            global_environment["GIT_CONFIG_NOSYSTEM"] = "1"
            global_environment.pop("GIT_CONFIG_GLOBAL", None)
            subprocess.run(
                [
                    os.fspath(seal.GIT),
                    "config",
                    "--global",
                    "gpg.ssh.allowedSignersFile",
                    str(allowed),
                ],
                check=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                env=global_environment,
            )
            self.git("config", "gpg.format", "ssh")
            self.git("config", "user.signingkey", str(key))
            (self.root / "tracked.txt").write_bytes(b"signed fixture\n")
            self.git("add", "-A")
            self.git("commit", "-q", "-S", "-m", "signed fixture")

            with mock.patch.dict(os.environ, {"HOME": str(home)}):
                descriptor = seal.compute_observed_descriptor(self.root)
                payload = seal._canonical_json_bytes(descriptor)
                reviewed = self.review_root / "signed-reviewed-source-closure.json"
                reviewed.write_bytes(payload)
                identity = seal.compute_identity(
                    self.root,
                    str(reviewed),
                    hashlib.sha256(payload).hexdigest(),
                )
            self.assertFalse(descriptor["source_repo_dirty"])
            self.assertEqual(descriptor["untracked_file_count"], 0)
            raw_public_key = base64.b64decode(
                key.with_suffix(".pub").read_text(encoding="ascii").split()[1],
                validate=True,
            )
            signature = identity.source_authority.signature
            self.assertEqual(signature.principal, "reviewer")
            self.assertEqual(
                signature.public_key_sha256,
                hashlib.sha256(raw_public_key).hexdigest(),
            )
            self.assertEqual(
                signature.allowed_signers_sha256,
                hashlib.sha256(allowed.read_bytes()).hexdigest(),
            )
            self.assertEqual(
                signature.revocation_sha256, hashlib.sha256(b"").hexdigest()
            )
        finally:
            self.signature_verifier = mock.patch.object(
                seal,
                "_verify_signed_commit",
                side_effect=self.fake_verified_authority,
            )
            self.signature_verifier.start()

    def test_unsigned_commit_is_rejected_before_descriptor_emission(self) -> None:
        (self.root / "tracked.txt").write_text("tracked\n", encoding="utf-8")
        self.commit()
        with mock.patch.object(
            seal,
            "_verify_signed_commit",
            side_effect=seal.SourceSealError(
                "source commit must carry a locally verifiable signature"
            ),
        ):
            with self.assertRaisesRegex(seal.SourceSealError, "verifiable signature"):
                seal.compute_observed_descriptor(self.root)


if __name__ == "__main__":
    unittest.main()
