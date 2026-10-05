"""Installed-mode admission controls with intercepted package and native loaders.

These controls execute only conftest Source and stdlib mocks. They do not load SDK
initializers, wheels, or a native extension and do not qualify installed SDK behavior.
"""

from __future__ import annotations

import importlib.machinery
import importlib.util
import os
import site
import sys
import sysconfig
import tempfile
import types
import unittest
from contextlib import ExitStack
from pathlib import Path
from unittest.mock import patch

_CONFTEXT = Path(__file__).with_name("conftest.py")
_NORITO_CONFTEXT = _CONFTEXT.parents[2] / "norito_py/tests/conftest.py"
_PACKAGES = ("iroha_python", "iroha_native", "norito", "iroha_torii_client")


class InstalledPackageConftestSourceTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        self.venv = self.root / "venv"
        self.base = self.root / "base"
        self.site = self.venv / "lib/site-packages"
        self.checkout = self.root / "checkout"
        for directory in (self.site, self.base, self.checkout):
            directory.mkdir(parents=True)
        self.specs = {}
        for name in _PACKAGES:
            self.specs[name] = self.package_spec(name, self.site)
        extension = self.site / "iroha_native" / (
            "_crypto" + importlib.machinery.EXTENSION_SUFFIXES[0]
        )
        extension.touch()
        self.specs["iroha_native._crypto"] = importlib.util.spec_from_file_location(
            "iroha_native._crypto", extension
        )
        self.events = []
        self.source_executions = []
        self.mutate = lambda name, module: None
        self.source_spec_changes = {}

    def package_spec(self, name, location):
        directory = location / name
        directory.mkdir(parents=True, exist_ok=True)
        origin = directory / "__init__.py"
        origin.touch()
        return importlib.util.spec_from_file_location(
            name, origin, submodule_search_locations=[str(directory)]
        )

    def find_spec(self, name, path=None, target=None):
        self.events.append(("find", name))
        return self.specs.get(name)

    def find_source_spec(self, name, path=None, target=None):
        self.events.append(("find", name))
        if name in self.source_spec_changes:
            return self.source_spec_changes[name]
        if name in ("iroha_python", "iroha_native"):
            package_root = _CONFTEXT.parents[2] / name / "src" / name
            return importlib.util.spec_from_file_location(
                name,
                package_root / "__init__.py",
                submodule_search_locations=[str(package_root)],
            )
        # Anonymous Source admission permits explicit native absence. This mock
        # never probes or imports a checkout extension.
        return None

    def execute_source(self, loader, module):
        # Interception is the only initializer execution in these controls.
        name = module.__name__
        self.events.append(("load", name))
        self.source_executions.append(name)
        if name == "iroha_native":
            def fake_extension():
                spec = self.specs["iroha_native._crypto"]
                native = types.ModuleType(spec.name)
                native.__spec__ = spec
                native.__loader__ = spec.loader
                native.__file__ = spec.origin
                sys.modules[spec.name] = native
                return native
            module.load_crypto_extension = fake_extension
        self.mutate(name, module)

    def run_source(self, *, mode="1", preseed=None, norito_only=False):
        path = _NORITO_CONFTEXT if norito_only else _CONFTEXT
        namespace = {"__name__": "isolated_installed_conftest", "__file__": str(path)}
        with ExitStack() as stack:
            stack.enter_context(patch.dict(sys.modules))
            for name in tuple(sys.modules):
                if any(name == root or name.startswith(root + ".") for root in _PACKAGES):
                    sys.modules.pop(name)
            if preseed is not None:
                sys.modules[preseed] = types.ModuleType(preseed)
            stack.enter_context(patch.object(sys, "path", [str(self.site)]))
            stack.enter_context(patch.object(sys, "prefix", str(self.venv)))
            stack.enter_context(patch.object(sys, "base_prefix", str(self.base)))
            stack.enter_context(patch.dict(os.environ))
            if mode is None:
                os.environ.pop("IROHA_PYTHON_TEST_INSTALLED_PACKAGE", None)
            else:
                os.environ["IROHA_PYTHON_TEST_INSTALLED_PACKAGE"] = mode
            stack.enter_context(patch.object(site, "getsitepackages", return_value=[str(self.site)]))
            stack.enter_context(patch.object(sysconfig, "get_paths", return_value={
                "purelib": str(self.site), "platlib": str(self.site),
            }))
            find_spec = self.find_source_spec if mode is None and not norito_only else self.find_spec
            stack.enter_context(patch.object(importlib.machinery.PathFinder, "find_spec", side_effect=find_spec))
            execute = self.execute_source
            stack.enter_context(patch.object(importlib.machinery.SourceFileLoader, "exec_module", new=lambda loader, module: execute(loader, module)))
            stack.enter_context(patch.object(importlib.machinery.ExtensionFileLoader, "create_module", side_effect=AssertionError("Native creation prohibited in Source controls")))
            stack.enter_context(patch.object(importlib.machinery.ExtensionFileLoader, "exec_module", side_effect=AssertionError("Native execution prohibited in Source controls")))
            exec(compile(path.read_text(), str(path), "exec"), namespace)
            return list(sys.path)

    def test_all_four_specs_and_extension_are_admitted_before_any_initializer(self):
        paths = self.run_source()
        first_load = next(index for index, event in enumerate(self.events) if event[0] == "load")
        self.assertEqual(self.events[:first_load], [("find", name) for name in (*_PACKAGES, "iroha_native._crypto")])
        self.assertEqual(self.source_executions, ["iroha_native", "norito", "iroha_torii_client", "iroha_python"])
        self.assertEqual(paths, [
            str(_CONFTEXT.parents[2] / "iroha_torii_client" / "tests"),
            str(_CONFTEXT.parent),
            str(self.site),
        ])

    def test_each_checkout_package_refuses_before_any_initializer(self):
        for name in _PACKAGES:
            with self.subTest(package=name):
                original = self.specs[name]
                self.specs[name] = self.package_spec(name, self.checkout)
                with self.assertRaisesRegex(RuntimeError, "private venv site-packages"):
                    self.run_source()
                self.assertEqual(self.source_executions, [])
                self.specs[name] = original

    def test_preseeded_roots_and_descendants_refuse_before_any_spec_or_import(self):
        for name in (*_PACKAGES, *(root + ".seeded" for root in _PACKAGES), "iroha_native._crypto"):
            with self.subTest(module=name):
                with self.assertRaisesRegex(RuntimeError, "pre-seeded module"):
                    self.run_source(preseed=name)
                self.assertEqual(self.events, [])

    def test_missing_norito_and_torii_specs_refuse_before_any_initializer(self):
        for name in ("norito", "iroha_torii_client"):
            with self.subTest(package=name):
                original = self.specs[name]
                self.specs[name] = None
                with self.assertRaisesRegex(RuntimeError, "installed regular package"):
                    self.run_source()
                self.assertEqual(self.source_executions, [])
                self.specs[name] = original

    def test_symlinked_package_origin_refuses_before_any_initializer(self):
        original = self.specs["norito"]
        link = self.site / "norito/linked.py"
        link.symlink_to(Path(original.origin))
        self.specs["norito"] = importlib.util.spec_from_file_location(
            "norito", link, submodule_search_locations=[str(link.parent)]
        )
        with self.assertRaisesRegex(RuntimeError, "non-symlinked"):
            self.run_source()
        self.assertEqual(self.source_executions, [])

    def test_escaped_package_search_path_refuses_before_any_initializer(self):
        self.specs["iroha_torii_client"].submodule_search_locations.append(str(self.checkout))
        with self.assertRaisesRegex(RuntimeError, "search path must match"):
            self.run_source()
        self.assertEqual(self.source_executions, [])

    def test_nonfilesystem_package_loader_refuses_before_any_initializer(self):
        self.specs["norito"].loader = object()
        with self.assertRaisesRegex(RuntimeError, "trusted filesystem import spec"):
            self.run_source()
        self.assertEqual(self.source_executions, [])

    def test_native_extension_outside_owner_refuses_before_any_initializer(self):
        other = self.site / "foreign"
        other.mkdir()
        extension = other / ("_crypto" + importlib.machinery.EXTENSION_SUFFIXES[0])
        extension.touch()
        self.specs["iroha_native._crypto"] = importlib.util.spec_from_file_location("iroha_native._crypto", extension)
        with self.assertRaisesRegex(RuntimeError, "authenticated iroha_native package"):
            self.run_source()
        self.assertEqual(self.source_executions, [])

    def test_replaced_package_module_refuses_after_intercepted_initializer(self):
        def replace(name, module):
            if name == "norito":
                sys.modules[name] = types.ModuleType(name)
        self.mutate = replace
        with self.assertRaisesRegex(RuntimeError, "loaded norito spec changed"):
            self.run_source()
        self.assertEqual(self.source_executions, ["iroha_native", "norito"])

    def test_changed_package_spec_refuses_after_intercepted_initializer(self):
        def replace(name, module):
            if name == "iroha_torii_client":
                module.__spec__ = self.package_spec(name, self.site)
        self.mutate = replace
        with self.assertRaisesRegex(RuntimeError, "loaded iroha_torii_client spec changed"):
            self.run_source()

    def test_changed_package_path_refuses_after_intercepted_initializer(self):
        def replace(name, module):
            if name == "norito":
                module.__path__ = [str(self.checkout)]
        self.mutate = replace
        with self.assertRaisesRegex(RuntimeError, "package search path changed"):
            self.run_source()

    def test_later_sdk_initializer_cannot_replace_a_previously_loaded_package(self):
        def replace(name, module):
            if name == "iroha_python":
                sys.modules["norito"] = types.ModuleType("norito")
        self.mutate = replace
        with self.assertRaisesRegex(RuntimeError, "loaded norito spec changed"):
            self.run_source()

    def test_installed_mode_requires_private_venv_before_spec_or_import(self):
        self.base = self.venv
        with self.assertRaisesRegex(RuntimeError, "require a private venv"):
            self.run_source()
        self.assertEqual(self.events, [])

    def test_invalid_mode_refuses_without_adding_any_library_path(self):
        for norito_only in (False, True):
            with self.subTest(norito_only=norito_only):
                with self.assertRaisesRegex(RuntimeError, "must be unset or 1"):
                    self.run_source(mode="0", norito_only=norito_only)
                self.assertEqual(self.events, [])

    def test_source_mode_retains_existing_developer_paths_without_package_imports(self):
        root = _CONFTEXT.parents[2]
        paths = self.run_source(mode=None)
        for relative in ("", "norito_py/src", "iroha_torii_client", "iroha_python/tests", "iroha_python/src", "iroha_native/src"):
            self.assertIn(str(root / relative), paths)
        self.assertEqual(self.events, [
            ("find", "iroha_python"), ("find", "iroha_native"),
            ("find", "iroha_native._crypto"),
        ])
        self.assertEqual(self.source_executions, [])

    def test_source_mode_rejects_preseeded_roots_and_descendants_before_dispatch(self):
        for name in ("iroha_python", "iroha_python.seeded", "iroha_native", "iroha_native._crypto"):
            with self.subTest(module=name):
                with self.assertRaisesRegex(RuntimeError, "source-package tests reject pre-seeded"):
                    self.run_source(mode=None, preseed=name)
                self.assertEqual(self.events, [])
                self.assertEqual(self.source_executions, [])

    def test_source_mode_rejects_an_installed_package_spec_before_import(self):
        self.source_spec_changes["iroha_python"] = self.specs["iroha_python"]
        with self.assertRaisesRegex(RuntimeError, "exact filesystem owner"):
            self.run_source(mode=None)
        self.assertEqual(self.events, [("find", "iroha_python")])
        self.assertEqual(self.source_executions, [])

    def test_norito_installed_conftest_adds_no_checkout_source(self):
        self.assertEqual(self.run_source(norito_only=True), [str(self.site)])
        self.assertEqual(self.events, [])

    def test_norito_source_conftest_retains_developer_source_path(self):
        paths = self.run_source(mode=None, norito_only=True)
        self.assertIn(str(_NORITO_CONFTEXT.parents[1] / "src"), paths)
        self.assertEqual(self.events, [])


if __name__ == "__main__":
    unittest.main()
