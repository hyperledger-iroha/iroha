"""Tests for the single-path Inrou V1 minimal-runtime packager."""

from __future__ import annotations

import hashlib
import importlib.util
import io
import os
import stat
import struct
import sys
import tempfile
import unittest
from contextlib import redirect_stderr
from pathlib import Path, PurePosixPath
from types import SimpleNamespace
from unittest import mock


REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT = REPO_ROOT / "scripts" / "ci" / "package_inrou_runtime_v1.py"
RUST_VERIFIER = (
    REPO_ROOT / "crates/irohad/src/soracloud_runtime/inrou_namespace.rs"
).read_text(encoding="utf-8")
RUST_SCHEMA = (
    REPO_ROOT / "crates/iroha_data_model/src/soracloud/schema.rs"
).read_text(encoding="utf-8")
SPEC = importlib.util.spec_from_file_location("package_inrou_runtime_v1", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


def _write(path: Path, payload: bytes, mode: int) -> Path:
    path.write_bytes(payload)
    path.chmod(mode)
    return path


def _elf64(interpreter: bytes = b"/lib64/ld-linux-x86-64.so.2\0") -> bytes:
    header = bytearray(64)
    header[:16] = b"\x7fELF\x02\x01\x01" + b"\0" * 9
    struct.pack_into("<H", header, 16, 3)
    struct.pack_into("<H", header, 18, 62)
    struct.pack_into("<I", header, 20, 1)
    struct.pack_into("<Q", header, 32, 64)
    struct.pack_into("<H", header, 52, 64)
    struct.pack_into("<H", header, 54, 56)
    struct.pack_into("<H", header, 56, 1)
    program = bytearray(56)
    struct.pack_into("<I", program, 0, 3)
    struct.pack_into("<I", program, 4, 4)
    struct.pack_into("<Q", program, 8, 120)
    struct.pack_into("<Q", program, 32, len(interpreter))
    return bytes(header + program + interpreter)


class InrouRuntimePackagerTests(unittest.TestCase):
    def test_constants_match_the_rust_verifier_contract(self) -> None:
        fixed_markers = (
            'const INROU_RUNTIME_ROOT: &str = "/opt/iroha/inrou-runtime-v1/root";',
            'const INROU_RUNTIME_MANIFEST: &str = '
            '"/opt/iroha/inrou-runtime-v1/manifest.sha256";',
            'const INROU_RUNTIME_MANIFEST_HEADER: &str = '
            '"iroha-inrou-runtime-v1 sha256";',
            'const INROU_NAMESPACE_QEMU_PATH: &str = "/inrou/bin/qemu";',
            'const INROU_NAMESPACE_SETPRIV_PATH: &str = "/inrou/bin/setpriv";',
            "const INROU_RUNTIME_MAX_FILES: usize = 512;",
            "pub(super) const INROU_NAMESPACE_MAX_LEASE_DISKS: usize = "
            "SORA_INROU_DATA_VOLUME_MAX_COUNT_V1;",
        )
        for marker in fixed_markers:
            self.assertIn(marker, RUST_VERIFIER)
        self.assertIn(
            "pub const SORA_INROU_DATA_VOLUME_MAX_COUNT_V1: usize = 32;",
            RUST_SCHEMA,
        )
        self.assertEqual(MODULE.DESTINATION, Path("/opt/iroha/inrou-runtime-v1"))
        self.assertEqual(MODULE.MANIFEST_HEADER, "iroha-inrou-runtime-v1 sha256")
        self.assertEqual(MODULE.MAX_ENTRIES, 512)
        leases = tuple(
            path
            for path in MODULE.PLACEHOLDERS
            if path.as_posix().startswith("/inrou/disk/lease")
        )
        self.assertEqual(
            leases,
            tuple(PurePosixPath(f"/inrou/disk/lease{index}") for index in range(32)),
        )

    def test_ldd_parser_accepts_only_absolute_resolved_records(self) -> None:
        output = (
            "\tlinux-vdso.so.1 (0x00007fff00000000)\n"
            "\tlibz.so.1 => /lib/x86_64-linux-gnu/libz.so.1 (0x00007f0000000000)\n"
            "\tlibc.so.6 => /lib/x86_64-linux-gnu/libc.so.6 (0x00007f0000001000)\n"
            "\t/lib64/ld-linux-x86-64.so.2 (0x00007f0000002000)\n"
        )
        self.assertEqual(
            MODULE.parse_ldd_output(output),
            (
                PurePosixPath("/lib/x86_64-linux-gnu/libc.so.6"),
                PurePosixPath("/lib/x86_64-linux-gnu/libz.so.1"),
                PurePosixPath("/lib64/ld-linux-x86-64.so.2"),
            ),
        )
        rejected = (
            "libz.so.1 => not found\n",
            "libz.so.1 => relative/libz.so.1 (0x1)\n",
            "libz.so.1 => /run/libz.so.1 (0x1)\n",
            "libz.so.1 => /lib/../tmp/libz.so.1 (0x1)\n",
            "libz.so.1 => /lib//libz.so.1 (0x1)\n",
            "statically linked\n",
            "\n",
            "libz.so.1 => /lib/libz.so.1 (0x1)\r\n",
        )
        for output in rejected:
            with self.subTest(output=output):
                with self.assertRaises(MODULE.PackagingError):
                    MODULE.parse_ldd_output(output)

    def test_elf_parser_requires_one_canonical_interpreter(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            executable = _write(root / "qemu", _elf64(), 0o755)
            identity = MODULE.inspect_elf(executable)
            self.assertEqual(identity.elf_class, 2)
            self.assertEqual(identity.byte_order, "<")
            self.assertEqual(identity.machine, 62)
            self.assertEqual(
                identity.interpreter,
                PurePosixPath("/lib64/ld-linux-x86-64.so.2"),
            )
            for payload in (b"not-elf", _elf64(b"relative-loader\0"), _elf64(b"/lib/loader")):
                executable.write_bytes(payload)
                with self.assertRaises(MODULE.PackagingError):
                    MODULE.inspect_elf(executable)

    def test_cli_rejects_noncanonical_source_path_spelling(self) -> None:
        parser = MODULE._parser()
        for rejected in (
            "relative/qemu",
            "/usr//bin/qemu",
            "/usr/./bin/qemu",
            "/usr/bin/../bin/qemu",
        ):
            with self.subTest(path=rejected):
                with redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
                    parser.parse_args(["--qemu", rejected])

    def test_path_chain_accepts_a_custodied_terminal_component(self) -> None:
        root_metadata = os.lstat("/")
        self.assertEqual(
            MODULE._validate_path_chain(
                Path("/"),
                owner_uid=root_metadata.st_uid,
                owner_gid=root_metadata.st_gid,
                label="filesystem root",
            ),
            Path("/"),
        )

    def test_path_chain_never_allows_an_ancestor_symlink(self) -> None:
        path = Path("/trusted/link/runtime")

        def fake_lstat(candidate: Path) -> SimpleNamespace:
            mode = (
                stat.S_IFLNK | 0o777
                if candidate == Path("/trusted/link")
                else stat.S_IFDIR | 0o755
            )
            return SimpleNamespace(st_mode=mode, st_uid=0, st_gid=0)

        with mock.patch.object(MODULE.os, "lstat", side_effect=fake_lstat):
            with self.assertRaisesRegex(MODULE.PackagingError, "symbolic link"):
                MODULE._validate_path_chain(
                    path,
                    owner_uid=0,
                    owner_gid=0,
                    label="runtime executable",
                )

    def test_distro_sources_resolve_merged_usr_loader_and_alternatives(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            library_dir = root / "usr/lib/aarch64-linux-gnu"
            library_dir.mkdir(parents=True, mode=0o755)
            alternatives = root / "etc/alternatives"
            alternatives.mkdir(parents=True, mode=0o755)
            loader = _write(library_dir / "ld-linux-aarch64.so.1", b"loader", 0o755)
            (root / "lib").symlink_to("usr/lib", target_is_directory=True)
            (root / "usr/lib/ld-linux-aarch64.so.1").symlink_to(
                "aarch64-linux-gnu/ld-linux-aarch64.so.1"
            )
            library = _write(library_dir / "libexample.so.1", b"library", 0o644)
            (alternatives / "libexample.so.1").symlink_to(library)
            (root / "usr/lib/libexample.so.1").symlink_to(
                "../../etc/alternatives/libexample.so.1"
            )
            for alias, resolved, executable in (
                (root / "lib/ld-linux-aarch64.so.1", loader, True),
                (root / "usr/lib/ld-linux-aarch64.so.1", loader, True),
                (root / "lib/libexample.so.1", library, False),
            ):
                with self.subTest(alias=alias):
                    self.assertEqual(
                        MODULE.validate_regular_source(
                            alias,
                            owner_uid=os.getuid(),
                            owner_gid=os.getgid(),
                            executable=executable,
                            allow_symlink=True,
                            label="distro source",
                        ),
                        resolved,
                    )

    def test_distro_source_aliases_never_hide_untrusted_intermediate_custody(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            resolved = _write(root / "trusted-library", b"library", 0o644)
            intermediate = root / "intermediate"
            intermediate.mkdir(mode=0o755)
            link = intermediate / "library"
            link.symlink_to(resolved)
            alias = root / "library"
            alias.symlink_to(link)
            kwargs = dict(
                owner_uid=os.getuid(), owner_gid=os.getgid(), executable=False,
                allow_symlink=True, label="distro source",
            )
            intermediate.chmod(0o777)
            with self.assertRaisesRegex(MODULE.PackagingError, "owner-custodied"):
                MODULE.validate_regular_source(alias, **kwargs)
            intermediate.chmod(0o755)
            real_lstat = MODULE.os.lstat

            def untrusted_link(path):
                metadata = real_lstat(path)
                if Path(path) != link:
                    return metadata
                return SimpleNamespace(
                    st_mode=metadata.st_mode, st_uid=os.getuid() + 1,
                    st_gid=metadata.st_gid, st_nlink=metadata.st_nlink,
                )

            with mock.patch.object(MODULE.os, "lstat", side_effect=untrusted_link):
                with self.assertRaisesRegex(MODULE.PackagingError, "owner-custodied"):
                    MODULE.validate_regular_source(alias, **kwargs)

    def test_distro_source_resolution_rejects_cycles_dangling_and_invalid_links(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            source = _write(root / "source", b"source", 0o644)
            alias = root / "alias"
            for target in ("alias", "missing", "source/.", "source/", "source//x", "bad\\name"):
                with self.subTest(target=target):
                    alias.symlink_to(target)
                    with self.assertRaises(MODULE.PackagingError):
                        MODULE.validate_regular_source(
                            alias, owner_uid=os.getuid(), owner_gid=os.getgid(),
                            executable=False, allow_symlink=True, label="distro source",
                        )
                    alias.unlink()
            self.assertEqual(source.read_bytes(), b"source")

    def test_distro_source_resolution_rejects_changed_alias_identity(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            _write(root / "source", b"source", 0o644)
            alias = root / "alias"
            alias.symlink_to("source")
            real_readlink = MODULE.os.readlink
            calls = 0

            def changing_link(path):
                nonlocal calls
                if Path(path) != alias:
                    return real_readlink(path)
                calls += 1
                return "source" if calls == 1 else "other-source"

            with mock.patch.object(MODULE.os, "readlink", side_effect=changing_link):
                with self.assertRaisesRegex(MODULE.PackagingError, "changed during resolution"):
                    MODULE.validate_regular_source(
                        alias, owner_uid=os.getuid(), owner_gid=os.getgid(),
                        executable=False, allow_symlink=True, label="distro source",
                    )

    def test_distro_alias_targets_retain_regular_file_custody(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            source = _write(root / "source", b"source", 0o755)
            alias = root / "alias"
            alias.symlink_to(source)
            kwargs = dict(
                owner_uid=os.getuid(), owner_gid=os.getgid(), executable=True,
                allow_symlink=True, label="distro executable",
            )
            for mode in (0o777, 0o4755, 0o2755, 0o644):
                source.chmod(mode)
                with self.subTest(mode=mode), self.assertRaises(MODULE.PackagingError):
                    MODULE.validate_regular_source(alias, **kwargs)
            source.chmod(0o755)
            os.link(source, root / "hardlink")
            with self.assertRaisesRegex(MODULE.PackagingError, "singly-linked"):
                MODULE.validate_regular_source(alias, **kwargs)

    def test_fixed_host_tools_admit_socat_package_alias_only(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            tools = root / "usr/bin"
            tools.mkdir(parents=True, mode=0o755)
            for name in ("bwrap", "nsenter", "socat1"):
                _write(tools / name, name.encode("ascii"), 0o755)
            (tools / "socat").symlink_to("socat1")
            validate = MODULE.validate_regular_source
            calls = []

            def validate_fixture(path, **kwargs):
                calls.append(path)
                return validate(tools / path.name, **kwargs)

            with mock.patch.object(MODULE, "validate_regular_source", side_effect=validate_fixture):
                MODULE._validate_fixed_host_tools(owner_uid=os.getuid(), owner_gid=os.getgid())
                self.assertEqual(tuple(calls), MODULE.FIXED_HOST_TOOLS)
                for name in ("bwrap", "nsenter"):
                    tool = tools / name
                    tool.rename(tools / (name + "-real"))
                    tool.symlink_to(name + "-real")
                    with self.subTest(name=name), self.assertRaisesRegex(
                        MODULE.PackagingError, "symbolic link"
                    ):
                        MODULE._validate_fixed_host_tools(
                            owner_uid=os.getuid(), owner_gid=os.getgid()
                        )
                    tool.unlink()
                    (tools / (name + "-real")).rename(tool)

    def test_atomic_materialization_matches_the_rust_manifest_contract(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            parent = Path(temporary).resolve()
            parent.chmod(0o700)
            sources = parent / "sources"
            sources.mkdir(mode=0o700)
            qemu_payload = b"qemu-v1\0payload"
            setpriv_payload = b"setpriv-v1\0payload"
            qemu = _write(sources / "qemu", qemu_payload, 0o755)
            setpriv = _write(sources / "setpriv", setpriv_payload, 0o755)
            loader = _write(sources / "loader", b"loader-v1", 0o755)
            libc = _write(sources / "libc", b"libc-v1", 0o644)
            destination = parent / "runtime-v1"
            files = (
                MODULE.RuntimeFile(MODULE.QEMU_TARGET, qemu, 0o555),
                MODULE.RuntimeFile(MODULE.SETPRIV_TARGET, setpriv, 0o555),
                MODULE.RuntimeFile(
                    PurePosixPath("/lib64/ld-linux-x86-64.so.2"), loader, 0o555
                ),
                MODULE.RuntimeFile(
                    PurePosixPath("/lib/x86_64-linux-gnu/libc.so.6"), libc, 0o444
                ),
            )
            try:
                MODULE.install_runtime(
                    destination,
                    files,
                    owner_uid=os.getuid(),
                    owner_gid=os.getgid(),
                )
                self.assertEqual(stat.S_IMODE(destination.stat().st_mode), 0o555)
                runtime_root = destination / MODULE.RUNTIME_ROOT_NAME
                manifest = destination / MODULE.MANIFEST_NAME
                self.assertEqual(stat.S_IMODE(runtime_root.stat().st_mode), 0o555)
                self.assertEqual(stat.S_IMODE(manifest.stat().st_mode), 0o444)
                manifest_bytes = manifest.read_bytes()
                self.assertLessEqual(len(manifest_bytes), MODULE.MAX_MANIFEST_BYTES)
                self.assertTrue(manifest_bytes.endswith(b"\n"))
                lines = manifest_bytes.decode("ascii").splitlines()
                self.assertEqual(lines[0], MODULE.MANIFEST_HEADER)
                records = lines[1:]
                paths = [record.split(" ", 4)[4] for record in records]
                self.assertEqual(paths, sorted(paths, key=lambda item: item.encode("ascii")))
                self.assertEqual(len(paths), len(set(paths)))
                self.assertEqual(paths[0], "/")
                for placeholder in MODULE.PLACEHOLDERS:
                    expected = runtime_root.joinpath(*placeholder.parts[1:])
                    self.assertEqual(expected.read_bytes(), b"")
                    self.assertEqual(stat.S_IMODE(expected.stat().st_mode), 0o444)
                self.assertIn(
                    "f "
                    f"{hashlib.sha256(qemu_payload).hexdigest()} "
                    f"{len(qemu_payload)} 0555 /inrou/bin/qemu",
                    records,
                )
                self.assertIn("d - 0 0555 /lib/x86_64-linux-gnu", records)
                for directory, child_directories, files_in_directory in os.walk(runtime_root):
                    directory_path = Path(directory)
                    metadata = os.lstat(directory_path)
                    self.assertTrue(stat.S_ISDIR(metadata.st_mode))
                    self.assertEqual(stat.S_IMODE(metadata.st_mode), 0o555)
                    for name in child_directories + files_in_directory:
                        child_metadata = os.lstat(directory_path / name)
                        self.assertFalse(stat.S_ISLNK(child_metadata.st_mode))
                        if stat.S_ISREG(child_metadata.st_mode):
                            self.assertEqual(child_metadata.st_nlink, 1)
                with self.assertRaises(MODULE.PackagingError):
                    MODULE.install_runtime(
                        destination,
                        files,
                        owner_uid=os.getuid(),
                        owner_gid=os.getgid(),
                    )
                self.assertEqual(list(parent.glob(".runtime-v1.staging.*")), [])
            finally:
                if destination.exists():
                    MODULE._remove_staging_directory(destination)

    def test_install_rejects_a_symlinked_destination_ancestor(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            real_parent = root / "real"
            real_parent.mkdir(mode=0o700)
            linked_parent = root / "linked"
            linked_parent.symlink_to(real_parent, target_is_directory=True)
            with self.assertRaisesRegex(MODULE.PackagingError, "symbolic link"):
                MODULE.install_runtime(
                    linked_parent / "runtime-v1",
                    (),
                    owner_uid=os.getuid(),
                    owner_gid=os.getgid(),
                )
            self.assertFalse((real_parent / "runtime-v1").exists())

    def test_publish_race_never_replaces_a_new_destination(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            parent = Path(temporary).resolve()
            parent.chmod(0o700)
            destination = parent / "runtime-v1"
            marker = destination / "created-by-racer"

            def create_destination_after_precheck(*_args, **_kwargs) -> None:
                destination.mkdir(mode=0o700)
                marker.write_bytes(b"must-survive")

            with mock.patch.object(
                MODULE,
                "materialize_runtime",
                side_effect=create_destination_after_precheck,
            ):
                with self.assertRaisesRegex(
                    MODULE.PackagingError, "destination already exists"
                ):
                    MODULE.install_runtime(
                        destination,
                        (),
                        owner_uid=os.getuid(),
                        owner_gid=os.getgid(),
                    )
            self.assertEqual(marker.read_bytes(), b"must-survive")
            self.assertEqual(list(parent.glob(".runtime-v1.staging.*")), [])
            source = SCRIPT.read_text(encoding="utf-8")
            self.assertIn("renameat2", source)
            self.assertIn("RENAME_NOREPLACE", source)

    def test_materializer_fsyncs_directories_bottom_up_and_fails_before_publish(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            parent = Path(temporary).resolve()
            parent.chmod(0o700)
            qemu = _write(parent / "qemu", b"qemu", 0o755)
            setpriv = _write(parent / "setpriv", b"setpriv", 0o755)
            files = (
                MODULE.RuntimeFile(MODULE.QEMU_TARGET, qemu, 0o555),
                MODULE.RuntimeFile(MODULE.SETPRIV_TARGET, setpriv, 0o555),
            )
            destination = parent / "runtime-v1"
            observed: list[Path] = []
            real_fsync_directory = MODULE._fsync_directory

            def record_fsync(path: Path) -> None:
                observed.append(path)
                real_fsync_directory(path)

            try:
                with mock.patch.object(
                    MODULE, "_fsync_directory", side_effect=record_fsync
                ):
                    MODULE.install_runtime(
                        destination,
                        files,
                        owner_uid=os.getuid(),
                        owner_gid=os.getgid(),
                    )
                self.assertGreater(len(observed), 2)
                staging = observed[-1]
                runtime_root = observed[-2]
                self.assertEqual(runtime_root.name, MODULE.RUNTIME_ROOT_NAME)
                self.assertTrue(staging.name.startswith(".runtime-v1.staging."))
                root_index = observed.index(runtime_root)
                self.assertTrue(
                    all(
                        len(path.parts) > len(runtime_root.parts)
                        for path in observed[:root_index]
                    )
                )
            finally:
                if destination.exists():
                    MODULE._remove_staging_directory(destination)

            failed_destination = parent / "failed-runtime-v1"

            def fail_fsync(_path: Path) -> None:
                raise OSError("simulated directory fsync failure")

            with mock.patch.object(
                MODULE, "_fsync_directory", side_effect=fail_fsync
            ):
                with self.assertRaisesRegex(OSError, "simulated directory fsync failure"):
                    MODULE.install_runtime(
                        failed_destination,
                        files,
                        owner_uid=os.getuid(),
                        owner_gid=os.getgid(),
                    )
            self.assertFalse(failed_destination.exists())
            self.assertEqual(list(parent.glob(".failed-runtime-v1.staging.*")), [])

    def test_materializer_rejects_symlink_and_hardlink_sources(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            parent = Path(temporary).resolve()
            parent.chmod(0o700)
            executable = _write(parent / "executable", b"payload", 0o755)
            symlink = parent / "symlink"
            symlink.symlink_to(executable)
            hardlink = parent / "hardlink"
            os.link(executable, hardlink)
            setpriv = _write(parent / "setpriv", b"setpriv", 0o755)
            for index, rejected_source in enumerate((symlink, hardlink)):
                destination = parent / f"rejected-{index}"
                files = (
                    MODULE.RuntimeFile(MODULE.QEMU_TARGET, rejected_source, 0o555),
                    MODULE.RuntimeFile(MODULE.SETPRIV_TARGET, setpriv, 0o555),
                )
                with self.subTest(source=rejected_source):
                    with self.assertRaises(MODULE.PackagingError):
                        MODULE.install_runtime(
                            destination,
                            files,
                            owner_uid=os.getuid(),
                            owner_gid=os.getgid(),
                        )
                    self.assertFalse(destination.exists())
                    self.assertEqual(list(parent.glob(f".{destination.name}.staging.*")), [])

    def test_copy_revalidates_all_source_custody_metadata_after_copy(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            parent = Path(temporary).resolve()
            source = _write(parent / "source", b"attested-payload", 0o755)
            source_identity = source.stat()
            real_fstat = os.fstat
            mutations = {
                "file-type": lambda metadata: {
                    "st_mode": stat.S_IFDIR | stat.S_IMODE(metadata.st_mode)
                },
                "owner": lambda metadata: {"st_uid": metadata.st_uid + 1},
                "group": lambda metadata: {"st_gid": metadata.st_gid + 1},
                "hard-link": lambda _metadata: {"st_nlink": 2},
                "forbidden-mode": lambda metadata: {
                    "st_mode": metadata.st_mode | stat.S_IWGRP
                },
                "executable-bit": lambda metadata: {
                    "st_mode": metadata.st_mode & ~0o111
                },
                "ctime": lambda metadata: {
                    "st_ctime_ns": metadata.st_ctime_ns + 1
                },
            }
            for label, mutation in mutations.items():
                with self.subTest(mutation=label):
                    source_fstat_calls = 0

                    def fstat_with_post_copy_mutation(descriptor: int):
                        nonlocal source_fstat_calls
                        metadata = real_fstat(descriptor)
                        if (
                            metadata.st_dev == source_identity.st_dev
                            and metadata.st_ino == source_identity.st_ino
                        ):
                            source_fstat_calls += 1
                            if source_fstat_calls == 2:
                                fields = {
                                    "st_dev": metadata.st_dev,
                                    "st_ino": metadata.st_ino,
                                    "st_mode": metadata.st_mode,
                                    "st_uid": metadata.st_uid,
                                    "st_gid": metadata.st_gid,
                                    "st_nlink": metadata.st_nlink,
                                    "st_size": metadata.st_size,
                                    "st_mtime_ns": metadata.st_mtime_ns,
                                    "st_ctime_ns": metadata.st_ctime_ns,
                                }
                                fields.update(mutation(metadata))
                                return SimpleNamespace(**fields)
                        return metadata

                    with mock.patch.object(
                        MODULE.os,
                        "fstat",
                        side_effect=fstat_with_post_copy_mutation,
                    ):
                        with self.assertRaises(MODULE.PackagingError):
                            MODULE._copy_attested_file(
                                source,
                                parent / f"copy-{label}",
                                source_identity=MODULE._source_identity(source_identity),
                                mode=0o555,
                                owner_uid=os.getuid(),
                                owner_gid=os.getgid(),
                            )
                    self.assertEqual(source_fstat_calls, 2)

    def test_docker_taira_stage_installs_and_invokes_the_fixed_packager(self) -> None:
        dockerfile = (REPO_ROOT / "Dockerfile").read_text(encoding="utf-8")
        release_builder = (REPO_ROOT / "scripts/build_release_image.sh").read_text(
            encoding="utf-8"
        )
        for package in ("python3-minimal", "util-linux", "socat"):
            self.assertIn(package, dockerfile)
        fixed_tools = (
            "/usr/bin/bwrap",
            "/usr/bin/nsenter",
            "/usr/bin/socat",
            "/usr/bin/setpriv",
        )
        for tool in fixed_tools:
            self.assertIn(f"test -x {tool}", dockerfile)
        self.assertIn(
            "COPY --from=builder /app/scripts/ci/package_inrou_runtime_v1.py "
            "/usr/local/libexec/package_inrou_runtime_v1.py",
            dockerfile,
        )
        self.assertIn('if [ "${CONFIG_PROFILE}" = "taira" ]; then', dockerfile)
        self.assertIn(
            "python3 /usr/local/libexec/package_inrou_runtime_v1.py;", dockerfile
        )
        self.assertIn("USER ${UID}:${GID}", dockerfile)
        self.assertNotIn("--destination", SCRIPT.read_text(encoding="utf-8"))
        self.assertIn(
            '--source "$repo_root/scripts/ci/package_inrou_runtime_v1.py"',
            release_builder,
        )
        self.assertIn(
            '--output "$build_context/scripts/ci/package_inrou_runtime_v1.py"',
            release_builder,
        )


class QemuCapabilityTests(unittest.TestCase):
    HELP = b"run-with options:\n  chroot=<str>\n  exit-with-parent=<bool (on/off)>\n  user=<str>\n"

    def record(self, root):
        source = _write(root / "qemu", b"fixed executable bytes", 0o755)
        return MODULE.RuntimeFile(MODULE.QEMU_TARGET, source, 0o555)

    def probe(self, record):
        MODULE._validate_qemu_capability(
            record, owner_uid=os.getuid(), owner_gid=os.getgid(),
        )

    def test_exact_parser_property_supports_help_exit_one(self):
        with tempfile.TemporaryDirectory() as temporary:
            record = self.record(Path(temporary).resolve())
            def inspect_pinned_fd(descriptor):
                self.assertEqual(os.read(descriptor, 100), b"fixed executable bytes")
                return (1, self.HELP)
            with mock.patch.object(MODULE, "_qemu_parser_help", side_effect=inspect_pinned_fd):
                self.probe(record)

    def test_missing_or_ambiguous_property_is_rejected(self):
        outputs = [
            (1, b"run-with options:\n  user=<str>\n"),
            (1, b"qemu: invalid parameter exit-with-parent\n"),
            (1, self.HELP.replace(b"exit-with-parent=", b"not-exit-with-parent=")),
            (1, self.HELP.replace(b"<bool (on/off)>", b"<str>")),
            (2, self.HELP),
            (1, self.HELP + b"qemu: startup failed\n"),
            (1, self.HELP + b"\xff"),
        ]
        with tempfile.TemporaryDirectory() as temporary:
            record = self.record(Path(temporary).resolve())
            for response in outputs:
                with self.subTest(response=response), mock.patch.object(MODULE, "_qemu_parser_help", return_value=response):
                    with self.assertRaises(MODULE.PackagingError):
                        self.probe(record)

    def test_probe_rejects_changed_source_and_copy_rejects_replacement(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            record = self.record(root)
            with mock.patch.object(MODULE, "_qemu_parser_help", return_value=(1, self.HELP)):
                self.probe(record)
                _write(root / "replacement", b"different executable", 0o755).replace(record.source)
                with self.assertRaisesRegex(MODULE.PackagingError, "changed before"):
                    self.probe(record)
                with self.assertRaisesRegex(MODULE.PackagingError, "changed after closure qualification"):
                    MODULE._copy_attested_file(
                        record.source, root / "copied", source_identity=record.source_identity,
                        mode=0o555, owner_uid=os.getuid(), owner_gid=os.getgid(),
                    )
                self.assertFalse((root / "copied").exists())

    def test_missing_source_is_a_packaging_error(self):
        with tempfile.TemporaryDirectory() as temporary:
            with self.assertRaisesRegex(MODULE.PackagingError, "cannot pin runtime source"):
                MODULE.RuntimeFile(MODULE.QEMU_TARGET, Path(temporary) / "absent", 0o555)

    def test_probe_rejects_source_mutation_during_help(self):
        with tempfile.TemporaryDirectory() as temporary:
            record = self.record(Path(temporary).resolve())
            def mutate(_descriptor):
                record.source.write_bytes(b"changed during parser invocation")
                return (1, self.HELP)
            with mock.patch.object(MODULE, "_qemu_parser_help", side_effect=mutate):
                with self.assertRaisesRegex(MODULE.PackagingError, "changed during"):
                    self.probe(record)

    def test_unavailable_probe_fails_with_bounded_diagnostic(self):
        with tempfile.TemporaryDirectory() as temporary:
            record = self.record(Path(temporary).resolve())
            with mock.patch.object(MODULE, "_qemu_parser_help", side_effect=OSError("sensitive excessive diagnostic")):
                with self.assertRaisesRegex(MODULE.PackagingError, "OSError") as error:
                    self.probe(record)
                self.assertNotIn("sensitive", str(error.exception))

    def test_collect_qualifies_same_closure_before_install(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            qemu = _write(root / "qemu", _elf64(), 0o755)
            setpriv = _write(root / "setpriv", _elf64(), 0o755)
            ldd = _write(root / "ldd", b"test ldd", 0o755)
            loader = _write(root / "loader", b"test loader", 0o755)
            collect = MODULE.collect_runtime_files
            def resolve(path, **_kwargs):
                return path if path in (qemu, setpriv, ldd) else loader
            def collect_owned(qemu, setpriv, ldd):
                return collect(qemu, setpriv, ldd, owner_uid=os.getuid(), owner_gid=os.getgid())
            with (
                mock.patch.object(MODULE, "validate_regular_source", side_effect=resolve),
                mock.patch.object(MODULE, "_validate_fixed_host_tools"),
                mock.patch.object(MODULE, "_run_ldd", return_value=()),
                mock.patch.object(MODULE, "_qemu_parser_help", return_value=(1, b"run-with options:\n  user=<str>\n")) as probe,
                mock.patch.object(MODULE, "collect_runtime_files", side_effect=collect_owned),
                mock.patch.object(MODULE, "install_runtime") as install,
                mock.patch.object(MODULE.sys, "platform", "linux"),
                mock.patch.object(MODULE.os, "geteuid", return_value=0),
                mock.patch.object(MODULE.os, "getegid", return_value=0),
            ):
                args = ["--qemu", str(qemu), "--setpriv", str(setpriv), "--ldd", str(ldd)]
                with self.assertRaisesRegex(MODULE.PackagingError, "lacks required"):
                    MODULE.main(args)
                install.assert_not_called()
                probe.return_value = (1, self.HELP)
                self.assertEqual(MODULE.main(args), 0)
                install.assert_called_once()
                records = install.call_args.args[1]
                self.assertEqual(next(r for r in records if r.target == MODULE.QEMU_TARGET).source, qemu)
                self.assertEqual(next(r for r in records if r.target == MODULE.QEMU_TARGET).source_identity,
                                 MODULE._source_identity(os.stat(qemu)))

    def test_help_reader_bounds_cleanup_and_preserves_primary_error(self):
        child = mock.Mock()
        child.wait.side_effect = MODULE.subprocess.TimeoutExpired("qemu-help", 1)
        selector = mock.MagicMock()
        selector.__enter__.return_value.select.return_value = []
        with (
            mock.patch.object(MODULE.subprocess, "Popen", return_value=child),
            mock.patch.object(MODULE.selectors, "DefaultSelector", return_value=selector),
        ):
            with self.assertRaisesRegex(MODULE.PackagingError, "exceeded 5 seconds;.*cleanup failed") as error:
                MODULE._qemu_parser_help(123)
            self.assertIsInstance(error.exception.__cause__, MODULE.PackagingError)
            child.kill.assert_called_once_with()
            child.wait.assert_called_once_with(timeout=1)
            child.stdout.close.assert_called_once_with()

    def test_help_reader_bounds_output_and_time(self):
        real_popen = MODULE.subprocess.Popen
        for code, timeout, expected in [
            ("import os; os.write(1, b'x'*100000)", 5, "output bound"),
            ("import time; time.sleep(10)", 0.1, "exceeded"),
        ]:
            def child(_command, **kwargs):
                self.assertEqual(_command[1:], ["-run-with", "help"])
                self.assertEqual(kwargs.pop("pass_fds"), (123,))
                return real_popen([sys.executable, "-c", code], **kwargs)
            with self.subTest(code=code), mock.patch.object(MODULE.subprocess, "Popen", side_effect=child), mock.patch.object(MODULE, "QEMU_PROBE_TIMEOUT_SECONDS", timeout):
                with self.assertRaisesRegex(MODULE.PackagingError, expected):
                    MODULE._qemu_parser_help(123)


if __name__ == "__main__":
    unittest.main()
