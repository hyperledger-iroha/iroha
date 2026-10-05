"""Actual filesystem source-owner admission; no native extension executes."""

from __future__ import annotations

import os
from pathlib import Path
import shutil
import subprocess
import sys

import pytest


ROOT = Path(__file__).resolve().parents[2]
CONFTEST = ROOT / "python/iroha_python/tests/conftest.py"
NATIVE = ROOT / "python/iroha_native/src/iroha_native"


@pytest.fixture
def owner_tree(tmp_path):
    """Keep the source guard and actual native package source at true paths."""
    root = tmp_path / "original"
    conftest = root / "python/iroha_python/tests/conftest.py"
    conftest.parent.mkdir(parents=True)
    shutil.copy2(CONFTEST, conftest)
    native = root / "python/iroha_native/src/iroha_native"
    native.mkdir(parents=True)
    for name in ("__init__.py", "_loader.py"):
        shutil.copy2(NATIVE / name, native / name)
    sdk = root / "python/iroha_python/src/iroha_python"
    sdk.mkdir(parents=True)
    (sdk / "__init__.py").write_text('"""Inert SDK package metadata control."""\n')
    foreign = tmp_path / "site-packages/iroha_native"
    foreign.mkdir(parents=True)
    for name in ("__init__.py", "_loader.py"):
        shutil.copy2(NATIVE / name, foreign / name)
    return root, conftest, native, foreign


def child(code: str, owner_tree):
    root, conftest, native, foreign = owner_tree
    environment = dict(os.environ, PYTHONDONTWRITEBYTECODE="1")
    environment.pop("IROHA_PYTHON_TEST_INSTALLED_PACKAGE", None)
    return subprocess.run([sys.executable, "-I", "-B", "-c", code,
                           str(conftest), str(native.parent), str(foreign.parent)],
                          capture_output=True, text=True, timeout=10, env=environment)


def test_original_source_specs_admit_anonymous_native_absence_without_loading(owner_tree):
    result = child('''
import runpy, sys
guard = runpy.run_path(sys.argv[1])
assert "iroha_native._crypto" not in sys.modules
assert "iroha_native._crypto" not in guard["_SOURCE_IMPORTS"]
import iroha_native
guard["_assert_source_loaded"]()
guard["pytest_sessionstart"](None)
guard["pytest_runtest_setup"](None)
guard["pytest_sessionfinish"](None, 0)
assert "iroha_native._crypto" not in sys.modules
''', owner_tree)
    assert result.returncode == 0, result.stderr


@pytest.mark.parametrize("when", ["before", "after"])
def test_real_foreign_filesystem_owner_is_refused_before_and_after_source_admission(owner_tree, when):
    code = '''
import runpy, sys
guard = None
if sys.argv[4] == "after": guard = runpy.run_path(sys.argv[1])
sys.path.insert(0, sys.argv[3])
import iroha_native
assert "iroha_native._crypto" not in sys.modules
assert iroha_native.__spec__.origin.startswith(sys.argv[3])
try:
    if guard is None: runpy.run_path(sys.argv[1])
    else: guard["_assert_source_loaded"]()
except RuntimeError as error:
    assert ("reject pre-seeded module iroha_native" if guard is None
            else "changed from its original filesystem owner") in str(error)
else: raise AssertionError("real foreign owner was admitted as source")
assert "iroha_native._crypto" not in sys.modules
'''
    root, conftest, native, foreign = owner_tree
    environment = dict(os.environ, PYTHONDONTWRITEBYTECODE="1")
    environment.pop("IROHA_PYTHON_TEST_INSTALLED_PACKAGE", None)
    result = subprocess.run([sys.executable, "-I", "-B", "-c", code,
                             str(conftest), str(native.parent), str(foreign.parent), when],
                            capture_output=True, text=True, timeout=10, env=environment)
    assert result.returncode == 0, result.stderr


@pytest.mark.parametrize("name", ["iroha_python", "iroha_native", "iroha_native._crypto"])
def test_preseeded_opaque_owner_is_refused_without_attribute_callbacks(owner_tree, name):
    code = '''
import runpy, sys
class Opaque:
    def __getattribute__(self, name): raise AssertionError("opaque owner callback executed")
sys.modules[sys.argv[4]] = Opaque()
try: runpy.run_path(sys.argv[1])
except RuntimeError as error: assert "reject pre-seeded module" in str(error)
else: raise AssertionError("preseeded opaque owner was admitted")
'''
    _, conftest, native, foreign = owner_tree
    environment = dict(os.environ, PYTHONDONTWRITEBYTECODE="1")
    environment.pop("IROHA_PYTHON_TEST_INSTALLED_PACKAGE", None)
    result = subprocess.run([sys.executable, "-I", "-B", "-c", code,
                             str(conftest), str(native.parent), str(foreign.parent), name],
                            capture_output=True, text=True, timeout=10, env=environment)
    assert result.returncode == 0, result.stderr


def test_source_package_symlink_is_refused_before_import(owner_tree):
    _, _, native, _ = owner_tree
    target = native / "original_init.py"
    (native / "__init__.py").rename(target)
    (native / "__init__.py").symlink_to(target)
    result = child('''
import runpy, sys
try: runpy.run_path(sys.argv[1])
except RuntimeError as error: assert "canonical and non-symlinked" in str(error)
else: raise AssertionError("linked source package was admitted")
assert "iroha_native._crypto" not in sys.modules
''', owner_tree)
    assert result.returncode == 0, result.stderr


def test_loaded_source_spec_mutation_is_refused_without_native_execution(owner_tree):
    result = child('''
import runpy, sys
guard = runpy.run_path(sys.argv[1])
import iroha_native
iroha_native.__spec__.origin = sys.argv[3] + "/iroha_native/__init__.py"
try: guard["pytest_sessionfinish"](None, 0)
except RuntimeError as error: assert "changed from its original filesystem owner" in str(error)
else: raise AssertionError("changed loaded source spec was admitted")
assert "iroha_native._crypto" not in sys.modules
''', owner_tree)
    assert result.returncode == 0, result.stderr


@pytest.mark.parametrize("when", ["before", "after"])
def test_real_foreign_loader_descendant_is_refused_before_dispatch_and_through_lifecycle(owner_tree, when):
    code = r"""
import importlib.util, runpy, sys
from pathlib import Path
guard = None
if sys.argv[4] == "after": guard = runpy.run_path(sys.argv[1])
path = Path(sys.argv[3]) / "iroha_native/_loader.py"
spec = importlib.util.spec_from_file_location("iroha_native._loader", path)
module = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = module
spec.loader.exec_module(module)
assert "iroha_native" not in sys.modules
assert "iroha_native._crypto" not in sys.modules
try:
    if guard is None: runpy.run_path(sys.argv[1])
    else: guard["pytest_runtest_setup"](None)
except RuntimeError as error:
    assert ("reject pre-seeded module iroha_native._loader" if guard is None
            else "changed from its original filesystem owner") in str(error)
else: raise AssertionError("actual foreign descendant supplied source package exports")
assert "iroha_native._crypto" not in sys.modules
"""
    _, conftest, native, foreign = owner_tree
    environment = dict(os.environ, PYTHONDONTWRITEBYTECODE="1")
    environment.pop("IROHA_PYTHON_TEST_INSTALLED_PACKAGE", None)
    result = subprocess.run([sys.executable, "-I", "-B", "-c", code,
                             str(conftest), str(native.parent), str(foreign.parent), when],
                            capture_output=True, text=True, timeout=10, env=environment)
    assert result.returncode == 0, result.stderr


@pytest.mark.parametrize("hook", ["pytest_sessionstart", "pytest_runtest_setup", "pytest_sessionfinish"])
def test_loaded_source_descendant_mutation_is_refused_at_every_lifecycle_hook(owner_tree, hook):
    code = r"""
import runpy, sys
guard = runpy.run_path(sys.argv[1])
import iroha_native._loader as module
guard["_assert_source_loaded"]()
module.__spec__.origin = sys.argv[3] + "/iroha_native/_loader.py"
try:
    if sys.argv[4] == "pytest_sessionfinish": guard[sys.argv[4]](None, 0)
    else: guard[sys.argv[4]](None)
except RuntimeError as error: assert "changed from its original filesystem owner" in str(error)
else: raise AssertionError("loaded descendant changed its original source owner")
assert "iroha_native._crypto" not in sys.modules
"""
    _, conftest, native, foreign = owner_tree
    environment = dict(os.environ, PYTHONDONTWRITEBYTECODE="1")
    environment.pop("IROHA_PYTHON_TEST_INSTALLED_PACKAGE", None)
    result = subprocess.run([sys.executable, "-I", "-B", "-c", code,
                             str(conftest), str(native.parent), str(foreign.parent), hook],
                            capture_output=True, text=True, timeout=10, env=environment)
    assert result.returncode == 0, result.stderr


@pytest.mark.parametrize("module_name", ["iroha_native._loader", "iroha_python._binding"])
def test_installed_mode_refuses_real_foreign_cached_descendants_before_any_sdk_or_native_dispatch(owner_tree, module_name):
    _, conftest, native, foreign = owner_tree
    if module_name == "iroha_native._loader":
        member = foreign / "_loader.py"
    else:
        member = foreign.parent / "iroha_python/_binding.py"
        member.parent.mkdir()
        member.write_text('"""Inert foreign cached binding filesystem control."""\n')
    code = r"""
import importlib.machinery, importlib.util, runpy, sys
spec = importlib.util.spec_from_file_location(sys.argv[3], sys.argv[2])
module = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = module
spec.loader.exec_module(module)
assert type(module.__spec__.loader) is importlib.machinery.SourceFileLoader
assert module.__spec__.origin == sys.argv[2]
assert "iroha_python" not in sys.modules and "iroha_native" not in sys.modules
assert "iroha_native._crypto" not in sys.modules
try: runpy.run_path(sys.argv[1])
except RuntimeError as error:
    assert str(error) == "installed-package tests reject pre-seeded module " + sys.argv[3]
else: raise AssertionError("installed qualification reused a real foreign cached descendant")
assert "iroha_python" not in sys.modules and "iroha_native" not in sys.modules
assert "iroha_native._crypto" not in sys.modules
"""
    environment = dict(os.environ, PYTHONDONTWRITEBYTECODE="1", IROHA_PYTHON_TEST_INSTALLED_PACKAGE="1")
    result = subprocess.run([sys.executable, "-I", "-B", "-c", code, str(conftest), str(member), module_name],
                            capture_output=True, text=True, timeout=10, env=environment)
    assert result.returncode == 0, result.stderr
