"""Provision the fixed scaling Python sources and their admitted BLAKE3 package.

Only standard-library and existing release owners are imported before package
admission. This module creates files and constructs one fixed argv; the existing
release process owner is responsible for launch, descriptors and natural reaping.
"""
from __future__ import annotations

import base64
import csv
import fcntl
from dataclasses import dataclass
import hashlib
import importlib.machinery
import importlib.util
import io
import json
import os
from pathlib import Path
import re
import stat
import sys
import types

import compute_workspace_source_manifest as source_contract
import copy_sumeragi_v2_release_cargo_cache as bundle_contract
import release_artifact_contract as artifact_contract

_MAX_PACKAGE_FILE = 16 * 1024 * 1024
_MAX_PACKAGE_TOTAL = 64 * 1024 * 1024
_MAX_SOURCE_FILE = 8 * 1024 * 1024
_MAX_SOURCE_TOTAL = 64 * 1024 * 1024
_MAX_INVENTORY = 1024 * 1024
_ENTRYPOINT = 'scripts/nexus/run_multilane_scaling_gate.py'
_SOURCE_SCHEMA = 'iroha.sumeragi_v2.multilane_scaling.python_sources.v1'
_WORKERS = ('resource_probe_worker.py', 'resource_probe.py', 'resource_process.py',
            'resource_evidence_budget.py', 'kura_resource_metrics.py')


class ScalingBootstrapError(ValueError):
    """Closed bootstrap failure without launch inputs or private contents."""


def _require(value):
    if not value:
        raise ScalingBootstrapError('scaling_bootstrap_failed')


def _failure(error):
    if isinstance(error, KeyboardInterrupt): raise KeyboardInterrupt() from None
    if isinstance(error, SystemExit): raise SystemExit(1) from None
    if isinstance(error, GeneratorExit): raise GeneratorExit() from None
    raise ScalingBootstrapError('scaling_bootstrap_failed') from None


def _path(value):
    _require(type(value) is type(Path('/')) and value.is_absolute()
             and str(value) == os.path.abspath(value) and len(value.parts) <= 64
             and len(os.fsencode(value)) <= 4096)
    return value


def _digest(value):
    _require(type(value) is str and re.fullmatch('[a-f0-9]{64}', value))
    return value


def _identity(info):
    return (info.st_dev, info.st_ino, info.st_mode, info.st_uid, info.st_gid,
            info.st_nlink, info.st_size, info.st_mtime_ns, info.st_ctime_ns)


def _descriptor_pin(fd):
    return (_identity(os.fstat(fd))[:5], fcntl.fcntl(fd, fcntl.F_GETFL),
            fcntl.fcntl(fd, fcntl.F_GETFD))


def _close_descriptor(fd, pin):
    try:
        if _descriptor_pin(fd) == pin: os.close(fd)
    except OSError:
        pass


def _profile():
    _require(sys.implementation.name == 'cpython' and sys.version_info[:2] >= (3, 12)
             and sys.platform == 'darwin')
    suffix = importlib.machinery.EXTENSION_SUFFIXES[0]
    _require(type(suffix) is str and re.fullmatch(r'\.cpython-[0-9]{2,3}-darwin\.so', suffix))
    return suffix


def _package_files():
    return ('blake3/__init__.py', 'blake3/__init__.pyi', 'blake3/py.typed',
            'blake3/blake3' + _profile(), 'blake3-1.0.9.dist-info/METADATA',
            'blake3-1.0.9.dist-info/WHEEL', 'blake3-1.0.9.dist-info/RECORD',
            'blake3-1.0.9.dist-info/licenses/LICENSE')


@dataclass(frozen=True, slots=True)
class _PackageSnapshot:
    sha256: str
    size: int
    identity: tuple


def _read_package_file(root, relative):
    # Wheel typing markers may be empty. The release bundle's retained regular
    # file reader handles zero bytes without weakening link or inode checks.
    raw, info = bundle_contract._read_regular(root / relative,
        'scaling Python package member', maximum_bytes=_MAX_PACKAGE_FILE)
    return _PackageSnapshot(hashlib.sha256(raw).hexdigest(),len(raw),_identity(info)), raw


def _exact_package_tree(root, expected):
    """Require an exact regular private package census for a fixed member set."""
    expected_directories = {str(parent) for name in expected for parent in Path(name).parents if parent != Path('.')}
    allowed = expected | expected_directories
    observed = set()
    root_fd, _, original = artifact_contract._open_absolute_directory(root, 'scaling package')
    def walk(fd, prefix):
        with os.scandir(fd) as entries:
            for entry in entries:
                relative = entry.name if not prefix else prefix + '/' + entry.name
                _require(len(observed) < len(allowed) and relative in allowed and relative not in observed)
                observed.add(relative)
                info = entry.stat(follow_symlinks=False)
                _require(info.st_uid == os.geteuid() and info.st_mode & 0o022 == 0)
                if relative in expected_directories:
                    _require(stat.S_ISDIR(info.st_mode))
                    child = os.open(entry.name, os.O_RDONLY | os.O_DIRECTORY | os.O_CLOEXEC | os.O_NOFOLLOW, dir_fd=fd)
                    try:
                        _require(_identity(os.fstat(child)) == _identity(info))
                        walk(child, relative)
                    finally: os.close(child)
                else:
                    _require(stat.S_ISREG(info.st_mode) and info.st_nlink == 1 and info.st_size <= _MAX_PACKAGE_FILE)
    try:
        walk(root_fd, '')
        _require(observed == allowed and _identity(os.fstat(root_fd)) == _identity(original)
                 and _identity(root.lstat()) == _identity(original))
    finally: os.close(root_fd)


def _exact_package(root):
    expected = set(_package_files())
    _exact_package_tree(root, expected)
    rows, total = {}, 0
    for relative in sorted(expected):
        info, raw = _read_package_file(root, relative)
        total += info.size
        _require(total <= _MAX_PACKAGE_TOTAL)
        rows[relative] = (info, raw)
    metadata = rows['blake3-1.0.9.dist-info/METADATA'][1]
    _require(metadata.count(b'\nName: blake3\n') == 1
             and metadata.count(b'\nVersion: 1.0.9\n') == 1)
    records = {}
    raw_record = rows['blake3-1.0.9.dist-info/RECORD'][1]
    _require(len(raw_record) <= 65536)
    for row in csv.reader(io.StringIO(raw_record.decode('utf-8'), newline='')):
        _require(len(row) == 3 and row[0] not in records and len(records) < 64)
        records[row[0]] = row[1:]
    for relative, (info, _) in rows.items():
        _require(relative in records)
        if relative.endswith('/RECORD'):
            _require(records[relative] == ['', ''])
        else:
            encoded = base64.urlsafe_b64encode(bytes.fromhex(info.sha256)).rstrip(b'=').decode('ascii')
            _require(records[relative] == ['sha256=' + encoded, str(info.size)])
    return tuple((relative, info) for relative, (info, _) in rows.items())


def stage_dependency_source(installed_root: Path, destination: Path) -> None:
    """Copy the exact already provisioned 1.0.9 wheel members, excluding caches.

    The release coordinator owns the installed wheel's origin. RECORD binds all
    selected bytes; it does not substitute for that distribution trust anchor.
    No pip invocation, package lookup, download or ambient import takes place.
    """
    installed_root, destination = _path(installed_root), _path(destination)
    _require(installed_root != destination and installed_root not in destination.parents
             and destination not in installed_root.parents)
    snapshots = []
    for relative in _package_files():
        info, raw = _read_package_file(installed_root, relative)
        snapshots.append((relative, info, raw))
    _require(sum(info.size for _, info, _ in snapshots) <= _MAX_PACKAGE_TOTAL)
    destination.mkdir(mode=0o700)
    for relative, _, raw in snapshots:
        path = destination / relative
        path.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
        artifact_contract.exclusive_write_bytes(path, raw, mode=0o600)
    _exact_package(destination)
    for relative, info, _ in snapshots:
        _require(_read_package_file(installed_root, relative)[0] == info)


@dataclass(frozen=True, slots=True)
class PythonDependencyPaths:
    """Inherited existing-private-bundle paths; no independent success receipt."""
    source_root: Path
    bundle_root: Path
    inventory: Path
    inventory_sha256: str


class _PrivatePythonBundleFiles:
    """Shared original-file custody for exact private Python distributions."""

    def _retain_directory(self, path):
        current, parent = Path('/'), None
        for offset, name in enumerate(path.parts):
            current = Path('/') if offset == 0 else current / name
            if current not in self._directories:
                _require(len(self._directories) < 128)
                fd = os.open(name, os.O_RDONLY | os.O_DIRECTORY | os.O_CLOEXEC | os.O_NOFOLLOW, dir_fd=parent)
                pin = None
                try:
                    pin = _descriptor_pin(fd)
                    info = os.fstat(fd)
                    _require(stat.S_ISDIR(info.st_mode) and _descriptor_pin(fd) == pin
                             and pin[1] & os.O_ACCMODE == os.O_RDONLY
                             and pin[2] == fcntl.FD_CLOEXEC)
                    self._directories[current] = (fd, parent, name, _identity(info), pin)
                except BaseException:
                    if pin is not None: _close_descriptor(fd, pin)
                    raise
            parent = self._directories[current][0]


    def _retain_file(self, path, maximum):
        self._retain_directory(path.parent)
        held = bundle_contract._hold_regular(path, 'scaling Python dependency', maximum_bytes=maximum)
        self._held.append(held)
        for key in ('descriptor_pin', 'parent_pin'):
            _require(held[key][1] & os.O_ACCMODE == os.O_RDONLY
                     and held[key][2] == fcntl.FD_CLOEXEC)


    def _check(self):
        _require(not self._failed and type(self._paths) is PythonDependencyPaths)
        for value, pin in zip((self._paths.source_root, self._paths.bundle_root,
                              self._paths.inventory, self._paths.inventory_sha256), self._path_pin, strict=True):
            _require(type(value) is type(pin) and value == pin)
        for fd, parent, name, identity, pin in self._directories.values():
            # Ancestor contents outside the package may change; the original
            # edge, inode, ownership and mode may not.
            _require(_descriptor_pin(fd) == pin and _identity(os.fstat(fd))[:5] == identity[:5]
                     and _identity(os.stat(name,dir_fd=parent,follow_symlinks=False))[:5] == identity[:5])
        for held in self._held:
            _require(_descriptor_pin(held['descriptor']) == held['descriptor_pin']
                     and _descriptor_pin(held['parent_fd']) == held['parent_pin'])
            _require(_identity(os.fstat(held['descriptor'])) == _identity(held['metadata'])
                     and _identity(os.stat(held['path'].name, dir_fd=held['parent_fd'], follow_symlinks=False)) == _identity(held['metadata']))
        if self._loaded:
            self._check_loaded_modules()

    def _check_loaded_modules(self):
        """Keep the exact ordinary module identities imported by the runtime owner."""
        for name, module, origin in self._module_pins:
            _require(sys.modules.get(name) is module and type(module) is types.ModuleType
                     and type(module.__file__) is str and Path(module.__file__) == origin)


    def validate(self):
        """Keep original files, lexical directories and actual loaded origins."""
        try:
            _require(not self._busy)
            self._busy = True
            self._check()
        except BaseException as error:
            self._failed = True; _failure(error)
        finally:
            self._busy = False


    def close(self):
        """Release own still-bound descriptors; preserve files and loaded modules."""
        self._failed = True
        for held in self._held:
            _close_descriptor(held['descriptor'], held['descriptor_pin'])
            _close_descriptor(held['parent_fd'], held['parent_pin'])
        self._held = []
        for fd, _, _, _, pin in reversed(tuple(self._directories.values())):
            _close_descriptor(fd, pin)
        self._directories = {}


class PythonDependencies(_PrivatePythonBundleFiles):
    """Original private BLAKE3 bundle, retained through one CLI lifetime."""
    def __init__(self, *args, **kwargs):
        raise ScalingBootstrapError('scaling_bootstrap_failed')

    @classmethod
    def provision(cls, source_root: Path, bundle_root: Path, inventory: Path):
        """Use the existing release copier and immediately admit its actual files."""
        _require(cls is PythonDependencies)
        source_root, bundle_root, inventory = map(_path, (source_root, bundle_root, inventory))
        _exact_package(source_root)
        bundle_contract.copy_private_bundle(source_root, bundle_root, inventory)
        digest = artifact_contract.stable_hash_path(inventory, max_size=_MAX_INVENTORY).sha256
        return cls.admit(PythonDependencyPaths(source_root, bundle_root, inventory, digest))

    @classmethod
    def admit(cls, paths: PythonDependencyPaths):
        """Reconcile both trees and retain original files before loading code."""
        _require(cls is PythonDependencies and type(paths) is PythonDependencyPaths)
        values = tuple(_path(value) for value in (paths.source_root, paths.bundle_root, paths.inventory))
        digest = _digest(paths.inventory_sha256)
        owner = object.__new__(cls)
        owner._paths = PythonDependencyPaths(*values, digest)
        owner._path_pin = (*values, digest)
        owner._held, owner._directories, owner._module_pins = [], {}, ()
        owner._failed, owner._busy, owner._loaded = False, False, False
        try:
            owner._retain_file(paths.inventory, _MAX_INVENTORY)
            _require(hashlib.sha256(owner._held[0]['data']).hexdigest() == digest)
            owner._inventory = bundle_contract._verify_private_bundle(*values)
            for root in (paths.source_root, paths.bundle_root):
                for relative, _ in _exact_package(root):
                    owner._retain_file(root / relative, _MAX_PACKAGE_FILE)
            owner.verify()
            return owner
        except BaseException as error:
            owner.close()
            _failure(error)





    def verify(self):
        """Re-run the existing private-bundle verifier under unchanged inputs."""
        try:
            _require(not self._busy)
            self._busy = True
            self._check()
            paths = self._paths
            for held in self._held:
                bundle_contract._revalidate_held_regular(held)
            _require(bundle_contract._verify_private_bundle(paths.source_root, paths.bundle_root,
                       paths.inventory) == self._inventory)
            _exact_package(paths.source_root); _exact_package(paths.bundle_root)
            self._check()
        except BaseException as error:
            self._failed = True; _failure(error)
        finally:
            self._busy = False

    def load(self):
        """Import only this admitted package before any scaling-runtime import."""
        try:
            _require(not self._loaded and not self._busy
                     and sys.flags.isolated and sys.flags.no_site and sys.flags.dont_write_bytecode
                     and 'blake3' not in sys.modules and 'blake3.blake3' not in sys.modules)
            self.verify()
            package = self._paths.bundle_root / 'blake3'
            spec = importlib.util.spec_from_file_location('blake3', package/'__init__.py',
                submodule_search_locations=[str(package)])
            _require(spec is not None and spec.loader is not None)
            module = importlib.util.module_from_spec(spec)
            sys.modules['blake3'] = module
            sources = [held['data'] for held in self._held if held['path'] == package / '__init__.py']
            _require(len(sources) == 1)
            exec(compile(sources[0], str(package / '__init__.py'), 'exec', dont_inherit=True), module.__dict__)
            extension = sys.modules.get('blake3.blake3')
            _require(type(extension) is types.ModuleType)
            self._module_pins = (('blake3', module, package/'__init__.py'),
                ('blake3.blake3', extension, package/('blake3'+_profile())))
            self._loaded = True
            self.verify()
            return module
        except BaseException as error:
            self._failed = True; _failure(error)

    @property
    def paths(self):
        """Original private-bundle arguments for the fixed launch document."""
        self.validate(); return self._paths

    @property
    def module(self):
        """The actual originating loaded BLAKE3 module, never a caller flag."""
        self.validate(); _require(self._loaded); return self._module_pins[0][1]



# The complete preflight has its own pure-Python test-tool closure. These
# content roots bind every selected wheel member, independently of the local
# installer's RECORD or a caller's claim about the installed version. They do
# not extend the collector's eight-member BLAKE3 runtime dependency surface.
_TEST_DISTRIBUTIONS = (
    ('pytest', '9.0.3', ('_pytest', 'pytest', 'py.py'), ('py.test', 'pytest'),
     'd2b8792e5804e1f3ee5f6f2bf7c9a53a3b91c99c383ce243661bb0c0f6a1cb31', 81),
    ('pluggy', '1.6.0', ('pluggy',), (),
     '985ee6c7b5639f089cc1007c22bc200b660af21feccdc7a3c28dc7a31999f103', 13),
    ('packaging', '26.3', ('packaging',), (),
     'cb7883d9e68484fede85f1c7bf178e7036a94383fd090423eea6d6d4a7f47a05', 28),
    ('iniconfig', '2.3.0', ('iniconfig',), (),
     '973566842f23d818a35b6714ae8f650316c2d489c37f96bfb641ed737ccfc2cd', 9),
    ('pygments', '2.21.0', ('pygments',), ('pygmentize',),
     'fe61cda7af430bd4d9d4e18dba03da768f6356f3b8beb2dd7a86c0e1e2dea065', 348),
)
_TEST_IMPORT_ROOTS = frozenset(('pytest', '_pytest', 'py', 'pluggy',
                                'packaging', 'iniconfig', 'pygments'))


def _test_package_rows(root, *, installed):
    """Capture the pinned pure-Python closure, checking each original RECORD."""
    _require(type(installed) is bool and sys.implementation.name == 'cpython'
             and sys.version_info[:2] >= (3, 12) and sys.platform == 'darwin')
    root = _path(root)
    output, total = {}, 0
    for name, version, exports, commands, expected_root, expected_count in _TEST_DISTRIBUTIONS:
        distribution = name + '-' + version + '.dist-info'
        record_name = distribution + '/RECORD'
        record_info, record = _read_package_file(root, record_name)
        _require(len(record) <= _MAX_INVENTORY)
        seen, selected = set(), []
        for row in csv.reader(io.StringIO(record.decode('utf-8'), newline='')):
            _require(len(row) == 3 and len(seen) < 2048 and row[0] not in seen)
            relative, claimed_hash, claimed_size = row
            seen.add(relative)
            if relative == record_name:
                _require(claimed_hash == claimed_size == '')
                continue
            if installed and relative in { '../../../bin/' + command for command in commands }:
                continue
            parts = Path(relative).parts
            _require(type(relative) is str and relative and len(relative) <= 512
                     and not relative.startswith('/') and '..' not in parts
                     and str(Path(relative)) == relative
                     and re.fullmatch(r'[A-Za-z0-9_./+-]+', relative) is not None)
            if installed and '__pycache__' in parts:
                _require(claimed_hash == claimed_size == ''
                         and re.fullmatch(r'.*/?__pycache__/[A-Za-z0-9_]+\.cpython-[0-9]{2,3}(?:\.opt-[12])?\.pyc', relative)
                         and (parts[0] in exports or (parts[0] == '__pycache__' and 'py.py' in exports)))
                continue
            if installed and relative in {distribution + '/' + value for value in
                                          ('INSTALLER', 'REQUESTED', 'direct_url.json')}:
                continue
            _require(parts[0] in exports or parts[0] == distribution)
            _require(relative not in output and not relative.endswith(('.pyc', '.pth', '.so')))
            info, data = _read_package_file(root, relative)
            encoded = base64.urlsafe_b64encode(bytes.fromhex(info.sha256)).rstrip(b'=').decode('ascii')
            _require(claimed_hash == 'sha256=' + encoded and claimed_size == str(info.size))
            total += info.size
            _require(total <= _MAX_PACKAGE_TOTAL and len(output) < 1024)
            selected.append((relative, info.sha256, info.size))
            output[relative] = (info, data)
        _require(record_name in seen)
        selected.sort()
        canonical = json.dumps(selected, separators=(',', ':'), ensure_ascii=True).encode('ascii')
        _require(len(selected) == expected_count
                 and hashlib.sha256(canonical).hexdigest() == expected_root)
        output[record_name] = (record_info, record)
    return output


def _exact_test_package(root):
    """Validate exact staged files; installer state, caches and launchers are absent."""
    rows = _test_package_rows(root, installed=False)
    _exact_package_tree(root, set(rows))
    return tuple((relative, info) for relative, (info, _) in sorted(rows.items()))


def stage_test_dependency_source(installed_root: Path, destination: Path) -> None:
    """Stage fixed test wheels from the original selection without importing site.

    Console scripts and installer metadata are not test-library inputs. The
    staged RECORDs describe exactly the selected pinned bytes; their generation
    does not weaken the independent content-root checks.
    """
    installed_root, destination = _path(installed_root), _path(destination)
    _require(installed_root != destination and installed_root not in destination.parents
             and destination not in installed_root.parents)
    captured = _test_package_rows(installed_root, installed=True)
    destination.mkdir(mode=0o700)
    for relative, (_, data) in captured.items():
        if relative.endswith('/RECORD'):
            continue
        path = destination / relative
        path.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
        artifact_contract.exclusive_write_bytes(path, data, mode=0o600)
    for name, version, exports, _, _, _ in _TEST_DISTRIBUTIONS:
        distribution = name + '-' + version + '.dist-info'
        record_name = distribution + '/RECORD'
        lines = []
        for relative, (info, _) in sorted(captured.items()):
            if relative == record_name:
                lines.append((relative, '', ''))
            elif Path(relative).parts[0] in (*exports, distribution):
                digest = base64.urlsafe_b64encode(bytes.fromhex(info.sha256)).rstrip(b'=').decode('ascii')
                lines.append((relative, 'sha256=' + digest, str(info.size)))
        stream = io.StringIO(newline='')
        csv.writer(stream, lineterminator='\n').writerows(lines)
        artifact_contract.exclusive_write_bytes(destination / record_name,
                                                stream.getvalue().encode('utf-8'), mode=0o600)
    _exact_test_package(destination)
    for relative, (original, _) in captured.items():
        _require(_read_package_file(installed_root, relative)[0] == original)


class _TestPackageSourceLoader:
    """Load only captured admitted Python bytes for the closed test namespaces."""
    def __init__(self, owner):
        self.owner = owner

    def find_spec(self, fullname, path=None, target=None):
        if fullname.split('.')[0] not in _TEST_IMPORT_ROOTS:
            return None
        self.owner.validate()
        prefix = fullname.replace('.', '/')
        choices = ((prefix + '/__init__.py', True), (prefix + '.py', False))
        for relative, package in choices:
            if relative in self.owner._sources:
                return importlib.util.spec_from_loader(fullname, self,
                    origin=str(self.owner._paths.bundle_root / relative), is_package=package)
        raise ModuleNotFoundError('unadmitted preflight test module', name=fullname)

    def create_module(self, spec):
        return None

    def exec_module(self, module):
        self.owner.validate()
        spec = module.__spec__
        origin = Path(spec.origin)
        relative = str(origin.relative_to(self.owner._paths.bundle_root))
        _require(relative in self.owner._sources)
        module.__file__ = str(origin)
        if spec.submodule_search_locations is not None:
            module.__path__ = [str(origin.parent)]
        self.owner._module_pins += ((module.__name__, module, origin),)
        self.owner._module_types[module.__name__] = types.ModuleType
        exec(compile(self.owner._sources[relative], str(origin), 'exec', dont_inherit=True), module.__dict__)
        observed = sys.modules.get(module.__name__)
        if observed is not module:
            # These two pinned Pygments files replace their just-executed module
            # with the _automodule class defined by those same captured bytes.
            _require(module.__name__ in ('pygments.lexers', 'pygments.formatters'))
            declared = module.__dict__.get('_automodule')
            _require(type(declared) is type and declared.__bases__ == (types.ModuleType,)
                     and declared.__module__ == module.__name__ and type(observed) is declared)
            expected = {key: value for key, value in module.__dict__.items()
                        if key not in ('newmod', 'oldmod', 'sys', 'types')}
            _require(set(observed.__dict__) == set(expected)
                     and all(observed.__dict__[key] is value for key, value in expected.items()))
            self.owner._module_pins = tuple((name, observed if name == module.__name__ else value, path)
                for name, value, path in self.owner._module_pins)
            self.owner._module_types[module.__name__] = declared
        self.owner.validate()


class PythonTestDependencies(_PrivatePythonBundleFiles):
    """Original admitted pytest closure, separate from collector BLAKE3 ownership."""
    def __init__(self, *args, **kwargs):
        raise ScalingBootstrapError('scaling_bootstrap_failed')

    @classmethod
    def provision(cls, source_root: Path, bundle_root: Path, inventory: Path):
        """Copy the exact fixed test closure with the existing release bundle owner."""
        _require(cls is PythonTestDependencies)
        source_root, bundle_root, inventory = map(_path, (source_root, bundle_root, inventory))
        _exact_test_package(source_root)
        bundle_contract.copy_private_bundle(source_root, bundle_root, inventory)
        digest = artifact_contract.stable_hash_path(inventory, max_size=_MAX_INVENTORY).sha256
        return cls.admit(PythonDependencyPaths(source_root, bundle_root, inventory, digest))

    @classmethod
    def admit(cls, paths: PythonDependencyPaths):
        """Hold current source/bundle files under their original inventory digest."""
        _require(cls is PythonTestDependencies and type(paths) is PythonDependencyPaths)
        values = tuple(_path(value) for value in (paths.source_root, paths.bundle_root, paths.inventory))
        owner = object.__new__(cls)
        owner._paths = PythonDependencyPaths(*values, _digest(paths.inventory_sha256))
        owner._path_pin = (*values, paths.inventory_sha256)
        owner._held, owner._directories, owner._module_pins = [], {}, ()
        owner._failed, owner._busy, owner._loaded = False, False, False
        owner._finder, owner._sources, owner._module_types = None, {}, {}
        try:
            owner._retain_file(paths.inventory, _MAX_INVENTORY)
            _require(hashlib.sha256(owner._held[0]['data']).hexdigest() == paths.inventory_sha256)
            owner._inventory = bundle_contract._verify_private_bundle(*values)
            for root in (paths.source_root, paths.bundle_root):
                for relative, _ in _exact_test_package(root):
                    owner._retain_file(root / relative, _MAX_PACKAGE_FILE)
                    if root == paths.bundle_root and relative.endswith('.py'):
                        owner._sources[relative] = owner._held[-1]['data']
            owner.verify()
            return owner
        except BaseException as error:
            owner.close()
            _failure(error)

    def verify(self):
        """Recheck original held bytes, exact member census and bundle provenance."""
        try:
            _require(not self._busy)
            self._busy = True
            self._check()
            for held in self._held:
                bundle_contract._revalidate_held_regular(held)
            paths = self._paths
            _require(bundle_contract._verify_private_bundle(paths.source_root, paths.bundle_root,
                       paths.inventory) == self._inventory)
            _exact_test_package(paths.source_root)
            _exact_test_package(paths.bundle_root)
            if self._loaded:
                _require(self._finder in sys.meta_path)
                observed = {name for name in sys.modules if name.split('.')[0] in _TEST_IMPORT_ROOTS}
                aliases = set()
                for alias, target in (('py.error', '_pytest._py.error'), ('py.path', '_pytest._py.path')):
                    _require('py' in self._module_types and target in self._module_types
                             and alias in sys.modules and sys.modules[alias] is sys.modules[target])
                    aliases.add(alias)
                _require(observed == {name for name, _, _ in self._module_pins} | aliases)
            self._check()
        except BaseException as error:
            self._failed = True
            _failure(error)
        finally:
            self._busy = False

    def load(self):
        """Import pytest from captured bytes before any test or plugin is loaded."""
        try:
            _require(not self._loaded and not self._busy and sys.flags.isolated
                     and sys.flags.no_site and sys.flags.dont_write_bytecode
                     and not any(name.split('.')[0] in _TEST_IMPORT_ROOTS for name in sys.modules))
            self.verify()
            self._finder = _TestPackageSourceLoader(self)
            sys.meta_path.insert(0, self._finder)
            module = importlib.import_module('pytest')
            self._loaded = True
            self.verify()
            return module
        except BaseException as error:
            self._failed = True
            _failure(error)

    def _check_loaded_modules(self):
        """Check exact imported objects, including the two admitted Pygments classes."""
        for name, module, origin in self._module_pins:
            expected = self._module_types[name]
            _require(expected is types.ModuleType or name in ('pygments.lexers', 'pygments.formatters'))
            _require(sys.modules.get(name) is module and type(module) is expected
                     and type(module.__file__) is str and Path(module.__file__) == origin)

    @property
    def paths(self):
        """The same fixed path/digest argument type used by the existing bundle owner."""
        self.validate()
        return self._paths

    def close(self):
        """Release owned file handles and importer after the original phase is reaped."""
        if self._finder is not None and self._finder in sys.meta_path:
            sys.meta_path.remove(self._finder)
        super().close()


# Closed first-release source surface, changed with its importing owners.
PYTHON_SOURCE_FILES = (
    'scripts/compute_workspace_source_manifest.py',
    'scripts/copy_sumeragi_v2_release_cargo_cache.py',
    'scripts/copy_sumeragi_v2_release_cargo_cache_cli.py',
    'scripts/nexus/applied_request_journal.py',
    'scripts/nexus/kura_resource_metrics.py',
    'scripts/nexus/resource_bundle.py',
    'scripts/nexus/resource_evidence_budget.py',
    'scripts/nexus/resource_experiment.py',
    'scripts/nexus/resource_probe.py',
    'scripts/nexus/resource_probe_worker.py',
    'scripts/nexus/resource_process.py',
    'scripts/nexus/resource_replay.py',
    'scripts/nexus/run_multilane_scaling_gate.py',
    'scripts/nexus/scaling_canonical_proof.py',
    'scripts/nexus/scaling_cli_bootstrap.py',
    'scripts/nexus/scaling_command.py',
    'scripts/nexus/scaling_completed_authority.py',
    'scripts/nexus/scaling_experiment_cli_inputs.py',
    'scripts/nexus/scaling_experiment_config.py',
    'scripts/nexus/scaling_experiment_custody.py',
    'scripts/nexus/scaling_experiment_directories.py',
    'scripts/nexus/scaling_experiment_execution.py',
    'scripts/nexus/scaling_experiment_files.py',
    'scripts/nexus/scaling_experiment_final_projection.py',
    'scripts/nexus/scaling_experiment_inputs.py',
    'scripts/nexus/scaling_experiment_invocation.py',
    'scripts/nexus/scaling_experiment_plan.py',
    'scripts/nexus/scaling_experiment_projection.py',
    'scripts/nexus/scaling_fixed_trial.py',
    'scripts/nexus/scaling_generator.py',
    'scripts/nexus/scaling_launcher.py',
    'scripts/nexus/scaling_load_outputs.py',
    'scripts/nexus/scaling_measurements.py',
    'scripts/nexus/scaling_native_facts.py',
    'scripts/nexus/scaling_native_facts_inputs.py',
    'scripts/nexus/scaling_native_load.py',
    'scripts/nexus/scaling_native_outputs.py',
    'scripts/nexus/scaling_proof_sequence.py',
    'scripts/nexus/scaling_public_files.py',
    'scripts/nexus/scaling_publication.py',
    'scripts/nexus/scaling_readiness.py',
    'scripts/nexus/scaling_readiness_inputs.py',
    'scripts/nexus/scaling_replayed_workload.py',
    'scripts/nexus/scaling_runtime_admission.py',
    'scripts/nexus/scaling_seed_pipe.py',
    'scripts/nexus/scaling_structural_identity.py',
    'scripts/nexus/scaling_trial_captures.py',
    'scripts/nexus/scaling_vector_collection.py',
    'scripts/nexus/scaling_worker_sources.py',
    'scripts/nexus/signed_request_journal.py',
    'scripts/release_artifact_contract.py',
    'scripts/sumeragi_v2_localnet_manifest.py',
    'scripts/sumeragi_v2_prebuilt_bundle.py',
    'scripts/write_sumeragi_v2_release_receipt.py',
    'scripts/write_sumeragi_v2_release_receipt_corridor_log.py',
    'scripts/write_sumeragi_v2_release_receipt_formal_artifacts.py',
    'scripts/write_sumeragi_v2_release_receipt_gate_evidence.py',
    'scripts/write_sumeragi_v2_release_receipt_publication.py',
)


@dataclass(frozen=True, slots=True)
class ProvisionedPythonSources:
    """Physical source/worker locations; the child independently verifies them."""
    root: Path
    manifest: Path
    manifest_sha256: str
    worker_sources: Path


def provision_python_sources(source_root: Path, source_paths: Path,
        source_paths_sha256: str, workspace_sha256: str, destination: Path,
        manifest: Path, worker_sources: Path) -> ProvisionedPythonSources:
    """Copy exactly the fixed CLI source closure from the retained release tree."""
    source_root, source_paths, destination, manifest, worker_sources = map(_path,
        (source_root, source_paths, destination, manifest, worker_sources))
    _digest(source_paths_sha256); _digest(workspace_sha256)
    _require(destination.parent == manifest.parent == worker_sources.parent
             and len({destination,manifest,worker_sources}) == 3
             and source_root not in destination.parents and destination not in source_root.parents
             and source_root != destination)
    path_list = artifact_contract.stable_hash_path(source_paths,
        max_size=source_contract._MAX_PATH_LIST_BYTES)
    _require(path_list.sha256 == source_paths_sha256
             and source_contract.workspace_source_manifest_from_exact_path_list(
                 source_root, source_paths) == workspace_sha256)
    snapshots, total = [], 0
    for relative in PYTHON_SOURCE_FILES:
        info, raw = artifact_contract.stable_read_relative(source_root, relative,
            max_size=_MAX_SOURCE_FILE, return_payload=True)
        total += info.size
        _require(0 < info.size and total <= _MAX_SOURCE_TOTAL)
        snapshots.append((relative, info, raw))
    parent_fd, _, parent_info = artifact_contract._open_absolute_directory(destination.parent,
        'scaling source destination')
    try:
        _require(parent_info.st_uid == os.geteuid() and stat.S_IMODE(parent_info.st_mode) == 0o700)
        os.mkdir(destination.name,0o700,dir_fd=parent_fd)
        os.mkdir(worker_sources.name,0o700,dir_fd=parent_fd)
        rows=[]
        for relative, info, raw in snapshots:
            path=destination/relative
            path.parent.mkdir(mode=0o700,parents=True,exist_ok=True)
            artifact_contract.exclusive_write_bytes(path,raw,mode=0o600)
            rows.append(dict(path=relative,sha256=info.sha256,size=info.size))
            if relative in {'scripts/nexus/'+name for name in _WORKERS}:
                artifact_contract.exclusive_write_bytes(worker_sources/Path(relative).name,raw,mode=0o600)
        value=dict(schema=_SOURCE_SCHEMA,files=rows)
        payload=artifact_contract.canonical_json_bytes(value)
        _require(len(payload)<=_MAX_INVENTORY)
        bundle_contract._publish_inventory(manifest,payload)
        expected=hashlib.sha256(payload).hexdigest()
        artifact_contract.verify_private_python_source_closure(destination,value,expected,owner_uid=os.geteuid())
        _require(set(artifact_contract.scan_inventory_paths(worker_sources)) == set(_WORKERS))
        for row in rows:
            if row['path'] in {'scripts/nexus/'+name for name in _WORKERS}:
                worker = artifact_contract.stable_hash_relative(worker_sources,
                    Path(row['path']).name,max_size=_MAX_SOURCE_FILE)
                _require((worker.sha256,worker.size) == (row['sha256'],row['size']))
        for relative,info,_ in snapshots:
            _require(artifact_contract.stable_hash_relative(source_root,relative,max_size=_MAX_SOURCE_FILE)==info)
        _require(artifact_contract.stable_hash_path(source_paths,max_size=source_contract._MAX_PATH_LIST_BYTES)==path_list
                 and source_contract.workspace_source_manifest_from_exact_path_list(source_root,source_paths)==workspace_sha256
                 and _identity(os.fstat(parent_fd))[:5]==_identity(parent_info)[:5]
                 and _identity(destination.parent.lstat())[:5]==_identity(parent_info)[:5])
        return ProvisionedPythonSources(destination,manifest,expected,worker_sources)
    finally:
        os.close(parent_fd)


def fixed_scaling_argv(python: Path, sources: ProvisionedPythonSources,
                       launch_descriptor: int, launch_sha256: str,
                       seed_descriptor: int) -> tuple[str,...]:
    """Construct the sole CLI invocation for the existing release process owner.

    The caller passes exactly the two original descriptors through its process
    owner. The seed is never copied into argv, an environment or a public file.
    This pure constructor neither spawns a process nor admits a release runtime.
    """
    _path(python);_digest(launch_sha256)
    _require(type(sources) is ProvisionedPythonSources
             and type(launch_descriptor) is int and 3<=launch_descriptor<(1<<20)
             and type(seed_descriptor) is int and 3<=seed_descriptor<(1<<20)
             and launch_descriptor!=seed_descriptor)
    return (str(python),'-I','-B','-S',str(_path(sources.root)/_ENTRYPOINT),
            '--launch-input-fd',str(launch_descriptor),'--launch-input-sha256',launch_sha256,
            '--seed-fd',str(seed_descriptor))


def bootstrap_runtime(raw: bytes) -> PythonDependencies:
    """Verify the fixed source closure, then load its original private package.

    The authenticated release caller supplies the bounded launch descriptor and
    its digest. This seam checks its existing source/bundle anchors; it does not
    turn caller-provided hashes into an independent release qualification.
    RuntimeAdmission subsequently reconciles the full binary/runtime closure.
    """
    from scaling_experiment_cli_inputs import load_launch_value
    owner, manifest_held = None, None
    try:
        value = load_launch_value(raw)
        runtime, dependency = value['runtime_paths'],value['python_dependencies']
        _require(type(runtime) is dict and type(dependency) is dict
                 and set(dependency) == {'source_root','bundle_root','inventory','inventory_sha256'})
        def path(item):
            _require(type(item) is str and '\0' not in item and str(Path(item)) == item)
            return _path(Path(item))
        source_root = path(runtime['python_sources'])
        _require(source_root/'scripts/nexus/scaling_cli_bootstrap.py' == Path(__file__).absolute()
                 and runtime['python_entrypoint'] == _ENTRYPOINT)
        manifest_path = path(runtime['python_source_manifest'])
        expected = _digest(runtime['python_source_manifest_sha256'])
        manifest_held = bundle_contract._hold_regular(manifest_path,
            'scaling Python source manifest',maximum_bytes=_MAX_INVENTORY)
        _require(stat.S_IMODE(manifest_held['metadata'].st_mode) in (0o400,0o600)
                 and hashlib.sha256(manifest_held['data']).hexdigest() == expected)
        manifest = artifact_contract.load_json_object(manifest_held['data'],'scaling Python sources')
        _require(set(manifest) == {'schema','files'} and manifest['schema'] == _SOURCE_SCHEMA
                 and type(manifest['files']) is list and len(manifest['files']) == len(PYTHON_SOURCE_FILES)
                 and all(type(row) is dict for row in manifest['files'])
                 and tuple(row.get('path') for row in manifest['files']) == PYTHON_SOURCE_FILES)
        def verify_sources():
            bundle_contract._revalidate_held_regular(manifest_held)
            artifact_contract.verify_private_python_source_closure(source_root,manifest,expected,
                owner_uid=os.geteuid(),entrypoint=_ENTRYPOINT,require_isolated_runtime=True)
        verify_sources()
        owner = PythonDependencies.admit(PythonDependencyPaths(path(dependency['source_root']),
            path(dependency['bundle_root']),path(dependency['inventory']),
            _digest(dependency['inventory_sha256'])))
        owner.load()
        verify_sources()
        owner.verify()
        return owner
    except BaseException as error:
        if owner is not None: owner.close()
        _failure(error)
    finally:
        if manifest_held is not None:
            bundle_contract._close_held_regular(manifest_held)


# Parent record framing belongs here because this owner loads before BLAKE3.
# The canonical policy decoder imports this same cap and dependency validator.
MAX_PARENT_EXECUTION_BYTES = 2 * 1024 * 1024 + 128 * 1024
PARENT_EXECUTION_SCHEMA = 'iroha.sumeragi_v2.multilane_scaling.parent_execution.v1'
MAX_FINAL_MARKER_BYTES = 1024 * 1024


def final_execution_record_digest(raw: bytes, externally_authenticated_sha256: str) -> str:
    """Extract the record pin only after checking an external final-marker pin.

    The caller supplies the authenticated digest through its existing trust
    channel. A checksum stored beside the archive is not that channel. This
    function neither signs a result nor reconstructs an original execution.
    """
    from resource_replay import _json
    _require(type(raw) is bytes and 0 < len(raw) <= MAX_FINAL_MARKER_BYTES
             and hashlib.sha256(raw).hexdigest() == _digest(externally_authenticated_sha256))
    value = _json(raw, MAX_FINAL_MARKER_BYTES)
    _require(type(value) is dict and set(value) == {
        'schema_version', 'result', 'bootstrap_completion_sha256', 'candidate_identity_sha256',
        'candidate_commit_oid', 'candidate_tree_oid', 'release_approvals', 'runner',
        'retained_source', 'receipt_validator', 'terminal_receipt', 'scaling_execution'}
        and type(value['schema_version']) is int and value['schema_version'] == 2
        and value['result'] == 'release-complete')
    projection = value['scaling_execution']
    _require(type(projection) is dict and set(projection) == {'archive_id', 'parent_execution', 'publication', 'preflight'}
             and projection['archive_id'] == 'release-scaling.fixed-collector.v1'
             and type(projection['preflight']) is dict)
    record = projection['parent_execution']
    _require(type(record) is dict and set(record) == {'archive_id', 'sha256', 'size_bytes', 'mode'}
             and record['archive_id'] == 'release-scaling.parent-execution.v1'
             and type(record['size_bytes']) is int and 0 < record['size_bytes'] <= MAX_PARENT_EXECUTION_BYTES
             and record['mode'] == '0400')
    return _digest(record['sha256'])


def dependency_package_census(root: Path) -> tuple[dict, ...]:
    """Project exact current package contents without historical inode authority."""
    return tuple(dict(path=name, size_bytes=row.size, sha256=row.sha256)
                 for name, row in _exact_package(_path(root)))


def validate_dependency_binding(value) -> dict:
    """Return owned closed content pins for the one supported verifier package."""
    _require(type(value) is dict and set(value) == {'inventory_sha256', 'files'})
    digest = _digest(value['inventory_sha256'])
    rows = value['files']
    expected = sorted(_package_files())
    _require(type(rows) is list and len(rows) == len(expected))
    result, total = [], 0
    for name, row in zip(expected, rows, strict=True):
        _require(type(row) is dict and set(row) == {'path', 'size_bytes', 'sha256'}
                 and type(row['path']) is str and row['path'] == name
                 and type(row['size_bytes']) is int and 0 <= row['size_bytes'] <= _MAX_PACKAGE_FILE)
        total += row['size_bytes']
        result.append(dict(path=name, size_bytes=row['size_bytes'], sha256=_digest(row['sha256'])))
    _require(total <= _MAX_PACKAGE_TOTAL)
    return dict(inventory_sha256=digest, files=result)


def parent_execution_dependency_binding(raw: bytes, expected_sha256: str) -> dict:
    """Check original/external digest before extracting bounded dependency data.

    This is framing only. Full record and plan semantics are checked after fresh
    package admission. Neither this digest nor a caller-supplied file proves an
    execution; detached callers need an externally authenticated final marker.
    """
    from resource_replay import _json
    _require(type(raw) is bytes and 0 < len(raw) <= MAX_PARENT_EXECUTION_BYTES
             and hashlib.sha256(raw).hexdigest() == _digest(expected_sha256))
    value = _json(raw, MAX_PARENT_EXECUTION_BYTES)
    _require(type(value) is dict and set(value) == {
        'schema', 'candidate', 'inputs', 'execution', 'publication', 'verifier_python', 'preflight'}
        and value['schema'] == PARENT_EXECUTION_SCHEMA
        and type(value['preflight']) is dict)
    return validate_dependency_binding(value['verifier_python'])


class RecordVerifierDependencies:
    """Fresh process-owned verifier and retained portable source commitments.

    This owner keeps original current file descriptors through receipt
    publication. It does not recreate an earlier live owner or adopt historical
    monotonic times. The private allocation remains available for diagnostics;
    closing releases only its own descriptors and preserves archived evidence.
    """
    def __init__(self, *args, **kwargs):
        raise ScalingBootstrapError('scaling_bootstrap_failed')

    def verify(self):
        """Recheck original retained files and the freshly admitted runtime."""
        try:
            _require(not self._closed)
            for held in self._held:
                bundle_contract._revalidate_held_regular(held)
            for root in (self._retained / 'source', self._retained / 'bundle'):
                _require(dependency_package_census(root) == tuple(self._binding['files']))
            self.dependencies.verify()
        except BaseException as error:
            _failure(error)

    def close(self):
        """Release descriptors without deleting retained nonsecret archives."""
        self._closed = True
        if self.dependencies is not None:
            self.dependencies.close()
        for held in self._held:
            _close_descriptor(held['descriptor'], held['descriptor_pin'])
            _close_descriptor(held['parent_fd'], held['parent_pin'])
        self._held = []


def prepare_record_verifier(raw: bytes, expected_sha256: str, retained_root: Path,
                            scope_parent: Path) -> RecordVerifierDependencies:
    """Verify portable bytes, then provision and load a fresh private package.

    The retained inventory is hashed as historical evidence only. Existing
    PythonDependencies.provision creates a new current inode inventory, and
    its original owner performs admission and isolated import.
    """
    import tempfile
    binding = parent_execution_dependency_binding(raw, expected_sha256)
    retained_root, scope_parent = _path(retained_root), _path(scope_parent)
    _require(retained_root != scope_parent and retained_root not in scope_parent.parents)
    owner = object.__new__(RecordVerifierDependencies)
    owner._held, owner.dependencies, owner._closed = [], None, False
    owner._retained, owner._binding = retained_root, binding
    try:
        held = bundle_contract._hold_regular(retained_root / 'inventory.json',
            'retained verifier inventory', maximum_bytes=_MAX_INVENTORY)
        owner._held.append(held)
        _require(hashlib.sha256(held['data']).hexdigest() == binding['inventory_sha256'])
        for root in (retained_root / 'source', retained_root / 'bundle'):
            _require(dependency_package_census(root) == tuple(binding['files']))
            for row in binding['files']:
                held = bundle_contract._hold_regular(root / row['path'],
                    'retained verifier package', maximum_bytes=_MAX_PACKAGE_FILE)
                owner._held.append(held)
                _require(len(held['data']) == row['size_bytes']
                         and hashlib.sha256(held['data']).hexdigest() == row['sha256'])
        descriptor, _, info = artifact_contract._open_absolute_directory(scope_parent, 'verifier scope parent')
        try:
            _require((info.st_uid == 0 and info.st_mode & stat.S_ISVTX)
                     or (info.st_uid == os.geteuid() and stat.S_IMODE(info.st_mode) == 0o700))
            scope = Path(tempfile.mkdtemp(prefix='iroha-scaling-verifier-', dir=scope_parent))
            current = scope.lstat()
            _require(stat.S_ISDIR(current.st_mode) and stat.S_IMODE(current.st_mode) == 0o700
                     and current.st_uid == os.geteuid()
                     and _identity(os.fstat(descriptor))[:5] == _identity(info)[:5]
                     and _identity(scope_parent.lstat())[:5] == _identity(info)[:5])
        finally:
            os.close(descriptor)
        owner.scope = scope
        stage_dependency_source(retained_root / 'source', scope / 'source')
        _require(dependency_package_census(scope / 'source') == tuple(binding['files']))
        owner.dependencies = PythonDependencies.provision(scope / 'source', scope / 'bundle', scope / 'inventory.json')
        owner.verify()
        owner.dependencies.load()
        owner.verify()
        return owner
    except BaseException as error:
        owner.close()
        _failure(error)
