"""Actual fresh-interpreter and malformed-input controls; no SDK qualification."""
from __future__ import annotations

import copy
import hashlib
import json
from pathlib import Path
import sys
from types import SimpleNamespace

import pytest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
from sorafs_python_consumer_artifact import ArtifactError, canonical_json
from sorafs_python_environment import (
    BOOTSTRAP_FILES, RUNTIME_PROBE, inspect_environment, pinned_requirements, verify_distributions,
    verify_environment_bootstrap, verify_runtime_probe,
)
from sorafs_python_process import run_python_process
from sorafs_python_producer_inputs import OriginalInputs, capture_tree


@pytest.fixture
def profile():
    return SimpleNamespace(platform="darwin", version="3.12.14",
                           executable=SimpleNamespace(path="/runtime/bin/python3.12"),
                           shared_runtime=(SimpleNamespace(path="/runtime/lib/Python"),),
                           stdlib_root="/runtime/lib/python3.12", zip_path="/runtime/lib/python312.zip")


def probe(profile, environment=None):
    return {"platform": profile.platform, "implementation": "cpython", "version": profile.version,
            "executable": str(environment / "bin/python3.12") if environment else profile.executable.path,
            "base_executable": profile.executable.path, "prefix": str(environment) if environment else "/runtime",
            "base_prefix": "/runtime", "stdlib": profile.stdlib_root,
            "shared_runtime": profile.shared_runtime[0].path, "isolated": 1,
            "no_site": 0 if environment else 1, "no_bytecode": True,
            "path": [profile.zip_path, profile.stdlib_root, profile.stdlib_root + "/lib-dynload"]
                    + ([str(environment / "lib/python3.12/site-packages")] if environment else [])}


@pytest.mark.parametrize("environment", (None, Path("/private/environment")))
def test_exact_runtime_paths_and_selected_executable(profile, environment):
    expected = probe(profile, environment)
    assert verify_runtime_probe(canonical_json(expected), profile, environment=environment) == expected


@pytest.mark.parametrize("key,value", (
    ("implementation", "pypy"), ("version", "3.12.13"), ("platform", "linux"),
    ("executable", "/runtime/other"), ("base_executable", "/runtime/other"),
    ("shared_runtime", "/tmp/foreign.dylib"), ("stdlib", "/tmp/stdlib"),
    ("isolated", True), ("no_site", True), ("no_bytecode", False),
    ("prefix", "/tmp/environment"), ("base_prefix", "../runtime"),
    ("path", ["/tmp/site-packages"]),
))
def test_probe_rejects_foreign_or_malformed_owner(profile, key, value):
    row = probe(profile); row[key] = value
    with pytest.raises(ArtifactError):
        verify_runtime_probe(canonical_json(row), profile)


def test_probe_rejects_ambient_path_even_with_expected_stdlib(profile):
    row = probe(profile); row["path"].append("/usr/site-packages")
    with pytest.raises(ArtifactError, match="import path"):
        verify_runtime_probe(canonical_json(row), profile)


@pytest.mark.parametrize("malformed", (b"", b"{}\n", b"{}", b"x" * 65537))
def test_closed_probe_encoding(profile, malformed):
    with pytest.raises((ArtifactError, ValueError)):
        verify_runtime_probe(malformed, profile)


@pytest.fixture
def environment_files():
    return {"pyvenv.cfg": b"home = /runtime/bin\ninclude-system-site-packages = false\nversion = 3.12.14\nexecutable = /runtime/bin/python3.12\ncommand = original\n",
            "bin/python3.12": b"original executable"}


def test_new_environment_must_have_empty_site(environment_files):
    inspect_environment(environment_files, installed=False)
    environment_files["lib/python3.12/site-packages/package/__init__.py"] = b"x=1"
    inspect_environment(environment_files, installed=True)
    with pytest.raises(ArtifactError, match="not empty"):
        inspect_environment(environment_files, installed=False)


@pytest.mark.parametrize("name", ("foo.pth", "FOO.PTH", "sitecustomize.py", "usercustomize.py",
                                   "usercustomize/__init__.py", "__pycache__/foo.pyc", "foo.pyo",
                                   "sitecustomize.cpython-312-darwin.so", "UserCustomize.so",
                                   "SiTeCuStOmIzE/__init__.py"))
def test_installed_startup_injection_refuses_before_start(environment_files, name):
    environment_files["lib/python3.12/site-packages/" + name] = b"print('foreign')"
    with pytest.raises(ArtifactError, match="startup"):
        inspect_environment(environment_files, installed=True)


@pytest.mark.parametrize("suffix", (b"include-system-site-packages = true\n", b"foreign = true\n", b"bad-line\n"))
def test_unowned_or_duplicate_venv_configuration(environment_files, suffix):
    environment_files["pyvenv.cfg"] += suffix
    with pytest.raises(ArtifactError):
        inspect_environment(environment_files, installed=False)


def test_requirements_use_escaped_local_uris_and_independent_hashes():
    digest = hashlib.sha256(b"actual wheel").hexdigest()
    raw = pinned_requirements([(Path("/private/space and#name.whl"), digest)])
    assert raw == ("file:///private/space%20and%23name.whl --hash=sha256:" + digest + "\n").encode()
    for entries in ([], [(Path("relative.whl"), digest)], [(Path("/p/a.whl"), "no")],
                    [(Path("/p/a.whl"), digest)] * 2):
        with pytest.raises(ArtifactError): pinned_requirements(entries)


def test_distribution_inventory_requires_private_exact_owners():
    root = Path("/private/environment")
    wanted = [{"module": "pytest", "version": "9.0.3", "root": str(root / "lib/python3.12/site-packages")}]
    encode = lambda rows: canonical_json({"distributions": rows})
    assert verify_distributions(encode(wanted), {"pytest": "9.0.3"}, root) == wanted
    for key, value in (("module", "foreign"), ("version", "8.4.2"), ("root", "/usr/site-packages")):
        rows = copy.deepcopy(wanted); rows[0][key] = value
        with pytest.raises(ArtifactError): verify_distributions(encode(rows), {"pytest": "9.0.3"}, root)
    with pytest.raises(ArtifactError): verify_distributions(encode(wanted * 2), {"pytest": "9.0.3"}, root)


def test_actual_without_pip_environment_and_runtime_probe(tmp_path):
    """Run ordinary CPython/venv mechanics only; no installed SDK or native fixture."""
    if sys.version_info[:2] != (3, 12): pytest.skip("requires the selected Python 3.12 control runtime")
    for name in ("home", "temporary"): (tmp_path / name).mkdir()
    python = str(Path(sys._base_executable).resolve(strict=True))

    def run(label, argv):
        result = run_python_process(tuple(argv), cwd=tmp_path, stdout_path=tmp_path / (label + ".stdout"),
                                    stderr_path=tmp_path / (label + ".stderr"), home=tmp_path / "home",
                                    temporary=tmp_path / "temporary", stdout_limit=65536, stderr_limit=65536, timeout_seconds=30)
        assert result.returncode == 0, result.stderr.path.read_text()
        assert result.stderr.size == 0
        return result.stdout.path.read_bytes()

    raw = run("base", [python, "-I", "-S", "-B", "-c", RUNTIME_PROBE])
    observed = json.loads(raw)
    profile = SimpleNamespace(platform=observed["platform"], version=observed["version"],
                               executable=SimpleNamespace(path=python),
                               shared_runtime=(SimpleNamespace(path=observed["shared_runtime"]),),
                               stdlib_root=observed["stdlib"], zip_path=observed["path"][0])
    verify_runtime_probe(raw, profile)
    environment = tmp_path / "environment"
    run("create", [python, "-I", "-S", "-B", "-m", "venv", "--without-pip", "--copies", str(environment)])
    links = {"lib64": "lib"} if sys.platform == "linux" and (environment / "lib64").is_symlink() else {}
    with OriginalInputs() as owner:
        files = capture_tree(environment, owner, maximum=16 * 1024 * 1024,
                             maximum_file=16 * 1024 * 1024, maximum_entries=128,
                             directory_links=links)
    inspect_environment(files, installed=False)
    assert files["bin/python3.12"] == Path(python).read_bytes()
    original = Path(python).read_bytes()
    profile.executable.sha256 = hashlib.sha256(original).hexdigest()
    profile.executable.size = len(original)
    bootstrap = verify_environment_bootstrap(files, profile, environment)
    assert set(bootstrap) == BOOTSTRAP_FILES == set(files)
    assert all(bootstrap[name] == original for name in ("bin/python", "bin/python3", "bin/python3.12"))
    raw = run("private", [str(environment / "bin/python3.12"), "-I", "-B", "-c", RUNTIME_PROBE])
    verify_runtime_probe(raw, profile, environment=environment)


def test_only_explicit_stock_linux_directory_alias_is_retained(tmp_path):
    environment = tmp_path / "environment"; environment.mkdir()
    (environment / "lib").mkdir()
    (environment / "lib/data").write_bytes(b"original")
    link = environment / "lib64"; link.symlink_to("lib")
    with OriginalInputs() as owner:
        with pytest.raises(ArtifactError, match="symbolic link"):
            capture_tree(environment, owner, maximum=64, maximum_file=64, maximum_entries=8)
        assert capture_tree(environment, owner, maximum=64, maximum_file=64, maximum_entries=8,
                            directory_links={"lib64": "lib"}) == {"lib/data": b"original"}
        assert owner.links[link][1] == "lib"
    owner = OriginalInputs()
    capture_tree(environment, owner, maximum=64, maximum_file=64, maximum_entries=8,
                 directory_links={"lib64": "lib"})
    link.unlink(); link.symlink_to("lib")
    with pytest.raises(ArtifactError, match="alias changed"): owner.recheck()
    owner.close()


def test_directory_alias_cannot_escape_or_replace_the_exact_inventory(tmp_path):
    environment = tmp_path / "environment"; environment.mkdir()
    (environment / "lib").mkdir()
    with OriginalInputs() as owner:
        with pytest.raises(ArtifactError, match="alias inventory"):
            capture_tree(environment, owner, maximum=64, maximum_file=64, maximum_entries=8,
                         directory_links={"lib64": "lib"})
        (environment / "lib64").symlink_to("../")
        with pytest.raises(ArtifactError, match="target differs"):
            capture_tree(environment, owner, maximum=64, maximum_file=64, maximum_entries=8,
                         directory_links={"lib64": "lib"})
