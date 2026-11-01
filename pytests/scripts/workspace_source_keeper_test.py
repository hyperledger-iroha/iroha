"""Actual retained-source controls for the canonical release aggregate.

Native Windows controls require local NTFS and genuine symlink privilege for
the explicit symlink case. No production checkout is changed by these tests.
"""

from __future__ import annotations

import importlib.util
import os
from pathlib import Path
import subprocess
import struct
import sys

import pytest


ROOT = Path(__file__).resolve().parents[2]


def module(name):
    path = ROOT / "scripts" / f"{name}.py"
    spec = importlib.util.spec_from_file_location(name, path)
    assert spec and spec.loader
    loaded = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(loaded)
    return loaded


def source_tree(tmp_path):
    root = tmp_path / "source"
    root.mkdir(mode=0o700)
    subprocess.run(["git", "init", "-q", root], check=True)
    subprocess.run(["git", "-C", root, "config", "user.name", "Source Fixture"], check=True)
    subprocess.run(["git", "-C", root, "config", "user.email", "source@example.invalid"], check=True)
    subprocess.run(["git", "-C", root, "config", "core.autocrlf", "false"], check=True)
    (root / ".gitignore").write_bytes(b"target/\n")
    (root / "Cargo.lock").write_bytes(b"version = 3\n")
    (root / "nested").mkdir()
    (root / "nested" / "source.txt").write_bytes(b"canonical bytes\n")
    subprocess.run(["git", "-C", root, "add", "."], check=True)
    subprocess.run(["git", "-C", root, "commit", "-qm", "fixture"], check=True)
    return root


def test_keeper_hashes_same_aggregate_from_original_readers(tmp_path, monkeypatch):
    source = module("compute_workspace_source_manifest")
    root = source_tree(tmp_path)
    expected = source.native_artifact_workspace_source_manifest(root)
    with source.retained_workspace_source(root, expected) as owner:
        assert owner.manifest == expected
        original = owner._keeper.members[root / "nested" / "source.txt"]
        reads = []
        duplicate = original.duplicate_reader

        def counted():
            reads.append(True)
            return duplicate()

        monkeypatch.setattr(original, "duplicate_reader", counted)
        owner.revalidate()
        assert reads, "the canonical aggregate must read the held original"
        assert source.native_artifact_workspace_source_manifest(root) == expected
    with pytest.raises((RuntimeError, OSError)):
        owner.revalidate()


def test_keeper_rejects_wrong_aggregate_and_nested_owner(tmp_path):
    source = module("compute_workspace_source_manifest")
    root = source_tree(tmp_path)
    expected = source.native_artifact_workspace_source_manifest(root)
    with pytest.raises(source.SourceSealError, match="canonical anchor"):
        with source.retained_workspace_source(root, "0" * 64):
            pytest.fail("a forged aggregate cannot enter publication")
    with source.retained_workspace_source(root, expected):
        with pytest.raises(source.SourceSealError, match="nested"):
            with source.retained_workspace_source(root, expected):
                pytest.fail("a nested owner cannot replace the original")


def test_source_inputs_never_use_cargo_hardlink_admission(tmp_path):
    keeper = module("workspace_source_keeper")
    root = tmp_path / "source"
    root.mkdir()
    path = root / "input"
    path.write_bytes(b"same inode")
    os.link(path, tmp_path / "alias")
    with pytest.raises((RuntimeError, OSError), match="(hard-link|shared identity)"):
        with keeper.SourceKeeper(root) as guard:
            guard.capture(["input"])


def test_census_bound_refuses_before_opening_unbounded_members(tmp_path):
    keeper = module("workspace_source_keeper")
    root = tmp_path / "source"
    root.mkdir()
    selected = []

    def members():
        for index in range(100):
            selected.append(index)
            yield f"input-{index}"

    with pytest.raises(RuntimeError, match="member bound"):
        with keeper.SourceKeeper(root, maximum_members=1) as guard:
            guard.capture(members())
    assert selected == [0, 1], "the source bound must stop enumeration immediately"
    with pytest.raises(RuntimeError, match="closed"):
        guard.capture(["late"])


@pytest.mark.skipif(os.name != "posix", reason="POSIX mutation succeeds then changes the retained snapshot")
def test_original_rewrite_and_restore_is_permanently_refused(tmp_path):
    source = module("compute_workspace_source_manifest")
    root = source_tree(tmp_path)
    path = root / "nested" / "source.txt"
    payload, before = path.read_bytes(), path.stat()
    expected = source.native_artifact_workspace_source_manifest(root)
    with pytest.raises((RuntimeError, OSError)):
        with source.retained_workspace_source(root, expected) as owner:
            path.write_bytes(b"changed bytes!!\n")
            path.write_bytes(payload)
            os.utime(path, ns=(before.st_atime_ns, before.st_mtime_ns))
            assert path.read_bytes() == payload
            assert path.stat().st_mtime_ns == before.st_mtime_ns
            owner.revalidate()


def test_nested_transient_inventory_mutation_is_not_recovered_by_same_names(tmp_path):
    keeper = module("workspace_source_keeper")
    root = tmp_path / "source"
    nested = root / "nested"
    nested.mkdir(parents=True)
    (nested / "input").write_bytes(b"original")
    with pytest.raises((RuntimeError, OSError)):
        with keeper.SourceKeeper(root) as guard:
            guard.capture(["nested/input"])
            before = nested.stat()
            extra = nested / "transient"
            extra.write_bytes(b"not selected")
            extra.unlink()
            os.utime(nested, ns=(before.st_atime_ns, before.st_mtime_ns))
            assert list(nested.iterdir()) == [nested / "input"]
            guard.revalidate()


def test_original_link_spelling_and_original_custody_are_retained(tmp_path):
    keeper = module("workspace_source_keeper")
    root = tmp_path / "source"
    root.mkdir()
    (root / "input").write_bytes(b"original")
    (root / "link").symlink_to("input")
    with keeper.SourceKeeper(root) as guard:
        guard.capture(["input", "link"])
        assert guard.readlink(root / "link") == os.readlink(root / "link") == "input"
        guard.revalidate()


def test_keeper_refuses_indirect_source_ancestry(tmp_path):
    keeper = module("workspace_source_keeper")
    root = tmp_path / "source"
    root.mkdir()
    if os.name == "nt":
        result = subprocess.run(["cmd.exe", "/d", "/c", "mklink", "/J",
                                 str(tmp_path / "alias"), str(root)], capture_output=True)
        assert result.returncode == 0, result.stderr
    else:
        (tmp_path / "alias").symlink_to(root, target_is_directory=True)
    with pytest.raises((RuntimeError, OSError)):
        with keeper.SourceKeeper(tmp_path / "alias"):
            pytest.fail("an indirect source root cannot be retained")


def test_failed_original_read_permanently_refuses_publication(tmp_path):
    keeper = module("workspace_source_keeper")
    root = tmp_path / "source"
    root.mkdir()
    path = root / "input"
    path.write_bytes(b"original")
    with pytest.raises(RuntimeError, match="permanently refused"):
        with keeper.SourceKeeper(root) as guard:
            guard.capture(["input"])
            with pytest.raises(OSError, match="fixture read failure"):
                with guard.reader(path):
                    raise OSError("fixture read failure")
            guard.revalidate()


@pytest.mark.skipif(os.name != "posix", reason="real POSIX FIFO replacement race")
@pytest.mark.parametrize("owner", ["keeper", "canonical", "canonical_at"])
def test_regular_to_fifo_open_race_is_bounded_and_refused(tmp_path, owner):
    root = tmp_path / "source"
    root.mkdir()
    path = root / "input"
    path.write_bytes(b"original")
    script = "workspace_source_keeper.py" if owner == "keeper" else "compute_workspace_source_manifest.py"
    child = subprocess.run([sys.executable, "-I", "-S", "-c",
                            "import importlib.util,os,pathlib,sys\n"
                            "p=pathlib.Path(sys.argv[1]); root=pathlib.Path(sys.argv[2])\n"
                            "s=importlib.util.spec_from_file_location('keeper',p)\n"
                            "m=importlib.util.module_from_spec(s); s.loader.exec_module(m)\n"
                            "original=os.open; selected=root/'input'\n"
                            "def raced(path,flags,*args,**kwargs):\n"
                            " if path==b'input' or pathlib.Path(path)==selected:\n"
                            "  selected.unlink(); os.mkfifo(selected)\n"
                            " return original(path,flags,*args,**kwargs)\n"
                            "os.open=raced\n"
                            "try:\n"
                            " if sys.argv[3]=='keeper':\n"
                            "  with m.SourceKeeper(root) as guard: guard.capture(['input'])\n"
                            " elif sys.argv[3]=='canonical':\n"
                            "  m._stable_file_sha256(selected,maximum_size=100,label='fixture')\n"
                            " else: m._manifest_for_paths(root,['input'])\n"
                            "except RuntimeError: print('refused-original-race')\n"
                            "else: raise SystemExit(2)\n",
                            str(ROOT / "scripts" / script), str(root), owner],
                           capture_output=True, check=True, timeout=5)
    assert child.stdout.strip() == b"refused-original-race"


@pytest.mark.skipif(os.name != "nt", reason="native Windows sharing exclusion")
def test_native_original_excludes_writer_and_deletion(tmp_path):
    keeper = module("workspace_source_keeper")
    root = tmp_path / "source"
    root.mkdir()
    path = root / "input"
    path.write_bytes(b"original")
    with keeper.SourceKeeper(root) as guard:
        guard.capture(["input"])
        with pytest.raises(OSError):
            path.write_bytes(b"replacement")
        with pytest.raises(OSError):
            path.unlink()
        with guard.reader(path) as reader:
            assert not os.get_inheritable(reader.fileno())
            assert reader.read() == b"original"
        child = subprocess.run([sys.executable, "-I", "-S", "-c",
                                "import pathlib,sys\n"
                                "p=pathlib.Path(sys.argv[1])\n"
                                "try: p.write_bytes(b'child replacement')\n"
                                "except OSError: print('original-retained')\n"
                                "else: raise SystemExit(2)\n", str(path)],
                               capture_output=True, check=True)
        assert child.stdout.strip() == b"original-retained"
        guard.revalidate()
    path.write_bytes(b"released")


@pytest.mark.skipif(os.name != "nt", reason="native Windows parent security latch")
def test_native_root_security_change_and_restore_stays_latched(tmp_path):
    keeper = module("workspace_source_keeper")
    root = tmp_path / "source"
    root.mkdir()
    (root / "input").write_bytes(b"original")
    with pytest.raises((RuntimeError, OSError), match="changed"):
        with keeper.SourceKeeper(root) as guard:
            guard.capture(["input"])
            # Save and restore the exact descriptor outside the watched tree.
            quoted = "'" + str(root).replace("'", "''") + "'"
            script = ("$ErrorActionPreference='Stop'; $p=" + quoted + "; $a=Get-Acl -LiteralPath $p; "
                      "$r=[System.Security.AccessControl.FileSystemAccessRule]::new("
                      "[System.Security.Principal.WindowsIdentity]::GetCurrent().User,"
                      "'Write','Deny'); $b=Get-Acl -LiteralPath $p; "
                      "$b.AddAccessRule($r); Set-Acl -LiteralPath $p -AclObject $b; "
                      "Set-Acl -LiteralPath $p -AclObject $a")
            subprocess.run(["powershell.exe", "-NoProfile", "-NonInteractive", "-Command",
                            script], check=True)
            guard.watches[1].revalidate()


@pytest.mark.skipif(os.name != "nt", reason="native Windows reparse payload parser")
def test_native_reparse_decoder_preserves_target_and_refuses_malformed_payload():
    native = module("workspace_source_keeper_windows")
    target = "input-\u03bb".encode("utf-16-le")
    payload = struct.pack("<HHHHI", 0, len(target), 0, 0, 1) + target
    raw = struct.pack("<IHH", native.SYMLINK_TAG, len(payload), 0) + payload
    assert native._decode_target(raw) == os.fsencode("input-\u03bb")
    for malformed in (raw[:-1], raw + b"x", raw[:8],
                      struct.pack("<I", 0xA000001D) + raw[4:],
                      raw[:8] + struct.pack("<H", len(target) + 2) + raw[10:],
                      raw[:16] + struct.pack("<I", 2) + raw[20:]):
        with pytest.raises((OSError, UnicodeError)):
            native._decode_target(malformed)


@pytest.mark.skipif(os.name != "nt", reason="native Windows junction member refusal")
def test_native_leaf_junction_is_not_relabelled_as_source_link(tmp_path):
    keeper = module("workspace_source_keeper")
    root, target = tmp_path / "source", tmp_path / "target"
    root.mkdir()
    target.mkdir()
    result = subprocess.run(["cmd.exe", "/d", "/c", "mklink", "/J",
                             str(root / "link"), str(target)], capture_output=True)
    assert result.returncode == 0, result.stderr
    with pytest.raises(OSError, match="unsupported reparse"):
        with keeper.SourceKeeper(root) as guard:
            guard.capture(["link"])
