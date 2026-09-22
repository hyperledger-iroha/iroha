"""Fixed offline CPython environment operations for SoraFS host qualification.

The producer supplies independently pinned original runtime/dependency bytes and
runs these fixed snippets through its bounded process owner. No ambient package
bootstrap, network resolver, caller code or arbitrary pip options are accepted.
"""
from __future__ import annotations

import hashlib
from pathlib import Path, PurePosixPath
import re

from sorafs_python_consumer_artifact import ArtifactError, canonical_json, _digest, _path
from sorafs_evidence_json import decode_evidence_json


BOOTSTRAP_FILES = frozenset({"pyvenv.cfg", "bin/python", "bin/python3", "bin/python3.12",
                             "bin/activate", "bin/activate.csh", "bin/activate.fish", "bin/Activate.ps1"})
_BOOTSTRAP_INTERPRETERS = frozenset({"bin/python", "bin/python3", "bin/python3.12"})
_MAX_BOOTSTRAP_EXECUTABLE_BYTES = 256 * 1024 * 1024
_MAX_BOOTSTRAP_CONFIG_BYTES = 64 * 1024
_MAX_BOOTSTRAP_ACTIVATION_BYTES = 1024 * 1024


def verify_environment_bootstrap(files: dict[str, bytes], runtime, environment: Path) -> dict[str, bytes]:
    """Join the stock POSIX CPython bootstrap without opening historical paths.

    Call inspect_environment independently for startup policy. The caller owns
    the complete file inventory and must retain/compare this exact subset across
    installation and execution. Activation files are bounded generated output,
    not reconstructed source or independently authenticated executable input.
    """
    if type(files) is not dict or not BOOTSTRAP_FILES <= files.keys():
        raise ArtifactError("private environment bootstrap inventory differs")
    if not isinstance(environment, (Path, PurePosixPath)):
        raise ArtifactError("private environment bootstrap path is malformed")
    selected = PurePosixPath(_path(runtime.executable.path, absolute=True))
    destination = _path(str(environment), absolute=True)
    if (runtime.platform not in ("darwin", "linux")
            or selected.name not in ("python", "python3", "python3.12")
            or type(runtime.version) is not str or len(runtime.version) > 64
            or re.fullmatch(r"3\.12\.(?:0|[1-9][0-9]*)", runtime.version) is None):
        raise ArtifactError("private environment bootstrap runtime profile differs")
    digest = _digest(runtime.executable.sha256)
    size = runtime.executable.size
    if type(size) is not int or not 0 < size <= _MAX_BOOTSTRAP_EXECUTABLE_BYTES:
        raise ArtifactError("private environment bootstrap executable size differs")
    for name in BOOTSTRAP_FILES:
        body = files[name]
        maximum = (size if name in _BOOTSTRAP_INTERPRETERS else
                   _MAX_BOOTSTRAP_CONFIG_BYTES if name == "pyvenv.cfg" else
                   _MAX_BOOTSTRAP_ACTIVATION_BYTES)
        if type(body) is not bytes or not 0 < len(body) <= maximum:
            raise ArtifactError("private environment bootstrap member exceeds its bound")
        if name in _BOOTSTRAP_INTERPRETERS and (len(body) != size or hashlib.sha256(body).hexdigest() != digest):
            raise ArtifactError("private environment bootstrap interpreter differs from original runtime")
    # CPython's create_configuration renders these five fields in this order.
    expected = (f"home = {selected.parent}\ninclude-system-site-packages = false\n"
                f"version = {runtime.version}\nexecutable = {selected}\n"
                f"command = {selected} -m venv --copies --without-pip {destination}\n").encode("utf-8")
    if files["pyvenv.cfg"] != expected:
        raise ArtifactError("private environment bootstrap configuration differs from original runtime")
    return {name: files[name] for name in sorted(BOOTSTRAP_FILES)}


RUNTIME_PROBE = r'''
import ctypes, json, os, pathlib, sys, sysconfig
class DlInfo(ctypes.Structure):
    _fields_ = [('filename', ctypes.c_char_p), ('base', ctypes.c_void_p),
                ('symbol', ctypes.c_char_p), ('address', ctypes.c_void_p)]
lookup = ctypes.CDLL(None).dladdr
lookup.argtypes = (ctypes.c_void_p, ctypes.POINTER(DlInfo))
lookup.restype = ctypes.c_int
info = DlInfo()
if lookup(ctypes.cast(ctypes.pythonapi.Py_Initialize, ctypes.c_void_p), ctypes.byref(info)) != 1 or not info.filename:
    raise SystemExit('unable to identify CPython shared runtime')
result = {'platform': sys.platform, 'implementation': sys.implementation.name,
          'version': '.'.join(map(str, sys.version_info[:3])),
          'executable': str(pathlib.Path(sys.executable).resolve(strict=True)),
          'base_executable': str(pathlib.Path(sys._base_executable).resolve(strict=True)),
          'prefix': sys.prefix, 'base_prefix': sys.base_prefix,
          'stdlib': str(pathlib.Path(sysconfig.get_path('stdlib')).resolve(strict=True)),
          'shared_runtime': str(pathlib.Path(os.fsdecode(info.filename)).resolve(strict=True)),
          'path': sys.path, 'isolated': sys.flags.isolated,
          'no_site': sys.flags.no_site, 'no_bytecode': sys.dont_write_bytecode}
print(json.dumps(result, sort_keys=True, separators=(',', ':'), ensure_ascii=True))
'''

PIP_BOOTSTRAP = r'''
import runpy, sys
sys.path.insert(0, sys.argv[1])
sys.argv = ['pip', '--isolated', 'install', '--no-index', '--no-deps',
            '--no-compile', '--no-cache-dir', '--disable-pip-version-check',
            '--require-hashes', '--only-binary=:all:', '--requirement', sys.argv[2]]
runpy.run_module('pip', run_name='__main__')
'''

DISTRIBUTION_PROBE = r'''
import importlib.metadata, json, pathlib, re, sys
rows = []
for distribution in importlib.metadata.distributions():
    name = re.sub('[-_.]+', '-', distribution.metadata['Name']).lower()
    rows.append({'module': name, 'version': distribution.version,
                 'root': str(pathlib.Path(distribution.locate_file('')).resolve(strict=True))})
print(json.dumps({'distributions': sorted(rows, key=lambda row: row['module'])}, sort_keys=True,
                 separators=(',', ':'), ensure_ascii=True))
'''


def verify_distributions(raw: bytes, expected: dict[str, str], environment: Path) -> list:
    """Require actual installed distribution names, versions and private roots."""
    if len(raw) > 64 * 1024:
        raise ArtifactError("installed distribution report exceeds its bound")
    value = decode_evidence_json(raw)
    if set(value) != {"distributions"}:
        raise ArtifactError("installed distribution report fields differ")
    rows = value["distributions"]
    wanted = [{"module": name, "version": version,
               "root": str(environment / "lib/python3.12/site-packages")}
              for name, version in sorted(expected.items())]
    if rows != wanted or canonical_json(value) != raw:
        raise ArtifactError("actual installed distribution inventory differs from original inputs")
    return rows


def verify_runtime_probe(raw: bytes, manifest, *, environment: Path | None = None) -> dict:
    """Join actual interpreter observations to the original full runtime inventory."""
    if not raw or len(raw) > 64 * 1024:
        raise ArtifactError("runtime probe output exceeds its bound")
    row = decode_evidence_json(raw)
    fields = {"platform", "implementation", "version", "executable", "base_executable",
              "prefix", "base_prefix", "stdlib", "shared_runtime", "path", "isolated",
              "no_site", "no_bytecode"}
    if type(row) is not dict or set(row) != fields or canonical_json(row) != raw:
        raise ArtifactError("runtime probe schema or encoding differs")
    if (row["platform"] != manifest.platform or row["implementation"] != "cpython"
            or row["version"] != manifest.version or row["stdlib"] != manifest.stdlib_root
            or type(row["isolated"]) is not int or row["isolated"] != 1
            or row["no_bytecode"] is not True
            or type(row["no_site"]) is not int or row["no_site"] != (1 if environment is None else 0)):
        raise ArtifactError("executing runtime differs from its pinned profile")
    declared = {manifest.executable.path, *(entry.path for entry in manifest.shared_runtime)}
    if row["shared_runtime"] not in declared or row["base_executable"] != manifest.executable.path:
        raise ArtifactError("actual CPython runtime is outside original inputs")
    for key in ("executable", "prefix", "base_prefix", "shared_runtime"):
        value = row[key]
        if type(value) is not str or not Path(value).is_absolute() or str(Path(value)) != value or ".." in Path(value).parts:
            raise ArtifactError("runtime probe path is noncanonical")
    expected_executable = manifest.executable.path if environment is None else str(environment / "bin/python3.12")
    if row["executable"] != expected_executable:
        raise ArtifactError("Python executable does not match the selected original")
    if row["prefix"] != (row["base_prefix"] if environment is None else str(environment)):
        raise ArtifactError("Python did not enter the selected environment")
    expected_path = [manifest.zip_path, manifest.stdlib_root, str(Path(manifest.stdlib_root) / "lib-dynload")]
    if environment is not None:
        expected_path.append(str(environment / "lib/python3.12/site-packages"))
    if row["path"] != expected_path:
        raise ArtifactError("Python import path contains an unowned runtime or package root")
    return row


def inspect_environment(files: dict[str, bytes], *, installed: bool) -> None:
    """Refuse startup injection before any private interpreter startup."""
    config = files.get("pyvenv.cfg", b"")
    settings = {}
    for line in config.decode("utf-8", "strict").splitlines():
        name, separator, value = line.partition(" = ")
        if not separator or name in settings:
            raise ArtifactError("private venv configuration is malformed")
        settings[name] = value
    if (settings.get("include-system-site-packages") != "false"
            or not settings.get("version", "").startswith("3.12.")
            or set(settings) - {"home", "include-system-site-packages", "version", "executable", "command"}):
        raise ArtifactError("private venv configuration permits unowned runtime inputs")
    site_prefix = "lib/python3.12/site-packages/"
    for name in files:
        path = Path(name)
        if (path.suffix.lower() in (".pth", ".pyc", ".pyo")
                or any(part.casefold().split(".", 1)[0] in ("sitecustomize", "usercustomize")
                       for part in path.parts)):
            raise ArtifactError("private environment contains startup customization or cached code")
        if not installed and name.startswith(site_prefix):
            raise ArtifactError("new --without-pip environment is not empty")


def pinned_requirements(paths: list[tuple[Path, str]]) -> bytes:
    """Generate pip's sole hash-required local-file input with no resolver options."""
    import re
    if not paths or len(paths) > 32 or len({path for path, _digest in paths}) != len(paths):
        raise ArtifactError("offline wheel input inventory is empty, duplicate or excessive")
    lines = []
    for path, digest in paths:
        if (not path.is_absolute() or path.suffix != ".whl" or ".." in path.parts
                or re.fullmatch(r"[0-9a-f]{64}", digest) is None):
            raise ArtifactError("offline wheel path or independent digest is malformed")
        lines.append(path.as_uri() + " --hash=sha256:" + digest)
    return ("\n".join(lines) + "\n").encode("utf-8")
