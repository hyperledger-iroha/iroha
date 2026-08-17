#!/usr/bin/env python3
"""Protect the typed `IrohaRuntimeDeps` setter inventory and its expansion."""

from __future__ import annotations

import re
import subprocess
import unittest
from dataclasses import dataclass
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SOURCE_PATH = Path("crates/irohad/src/main/runtime_deps.rs")
SOURCE = ROOT / SOURCE_PATH
INDEX_BLOB = "5c4b48e9b76ac22c99248263d9ad1153883f0f2b"
LINE_CEILING = 561
MACRO = """macro_rules! define_runtime_dep_setters_v1 {
    (
        $(
            $(#[$attribute:meta])*
            $name:ident($argument:ident: $dependency:ty $(,)?) => $field:ident;
        )+
    ) => {
        $(
            $(#[$attribute])*
            #[must_use]
            pub fn $name(mut self, $argument: $dependency) -> Self {
                self.$field = Some($argument);
                self
            }
        )+
    };
}

"""

OLD_SETTER = re.compile(
    r"(?P<docs>(?:    ///[^\n]*\n)+)"
    r"    #\[must_use\]\n"
    r"    pub fn (?P<name>with_[A-Za-z0-9_]+)\(\n"
    r"        mut self,\n"
    r"(?P<argument>.*?)"
    r"    \) -> Self \{\n"
    r"        self\.(?P<field>[A-Za-z0-9_]+) = Some\((?P<value>[A-Za-z0-9_]+)\);\n"
    r"        self\n"
    r"    \}\n",
    re.DOTALL,
)
NEW_SETTER = re.compile(
    r"(?P<docs>(?:        ///[^\n]*\n)+)"
    r"        (?P<name>with_[A-Za-z0-9_]+)\(\n"
    r"(?P<argument>.*?)"
    r"        \) => (?P<field>[A-Za-z0-9_]+);\n",
    re.DOTALL,
)
ARGUMENT = re.compile(
    r"(?P<indent> +)(?P<name>[A-Za-z0-9_]+): (?P<type>.*),\n",
    re.DOTALL,
)


@dataclass(frozen=True)
class Setter:
    docs: tuple[str, ...]
    name: str
    argument: str
    dependency_type: str
    field: str


def _git(*args: str) -> str:
    return subprocess.check_output(
        ["git", *args], cwd=ROOT, text=True, encoding="utf-8"
    )


def _normal_type(value: str) -> str:
    return " ".join(value.split())


def _old_inventory(source: str) -> tuple[list[Setter], int, int]:
    matches = list(OLD_SETTER.finditer(source))
    if len(matches) != 57:
        raise AssertionError(f"expected 57 indexed setters, found {len(matches)}")
    setters = []
    for match in matches:
        argument = ARGUMENT.fullmatch(match.group("argument"))
        if argument is None or argument.group("name") != match.group("value"):
            raise AssertionError(f"malformed indexed setter {match.group('name')}")
        setters.append(
            Setter(
                docs=tuple(line[4:] for line in match.group("docs").splitlines()),
                name=match.group("name"),
                argument=argument.group("name"),
                dependency_type=_normal_type(argument.group("type")),
                field=match.group("field"),
            )
        )
    return setters, matches[0].start(), matches[-1].end()


def _new_inventory(source: str) -> tuple[list[Setter], int, int]:
    marker = "    define_runtime_dep_setters_v1! {\n"
    start = source.find(marker)
    if start < 0 or source.count(marker) != 1 or not source.endswith("    }\n}\n"):
        raise AssertionError("runtime dependency setter invocation is missing or malformed")
    end = len(source) - 2
    region = source[start:end]
    matches = list(NEW_SETTER.finditer(region))
    if len(matches) != 57:
        raise AssertionError(f"expected 57 typed setter rows, found {len(matches)}")
    setters = []
    for match in matches:
        argument = ARGUMENT.fullmatch(match.group("argument"))
        if argument is None:
            raise AssertionError(f"malformed typed setter row {match.group('name')}")
        setters.append(
            Setter(
                docs=tuple(line[8:] for line in match.group("docs").splitlines()),
                name=match.group("name"),
                argument=argument.group("name"),
                dependency_type=_normal_type(argument.group("type")),
                field=match.group("field"),
            )
        )
    return setters, start, end


def _validate_source(source: str, indexed: str) -> None:
    if source.count("macro_rules! define_runtime_dep_setters_v1") != 1:
        raise AssertionError("setter emitter count changed")
    macro_start = source.index("macro_rules! define_runtime_dep_setters_v1")
    if source[macro_start : macro_start + len(MACRO)] != MACRO:
        raise AssertionError("setter emitter body changed")
    if len(source.splitlines()) > LINE_CEILING:
        raise AssertionError("runtime dependency source line ceiling exceeded")
    forbidden = ("dyn Fn", "FnMut", "FnOnce", "Action", "Scenario", "$body", "$setup")
    if any(token in source[macro_start:] for token in forbidden):
        raise AssertionError("callback or body-dispatch escape hatch introduced")

    old_rows, old_start, old_end = _old_inventory(indexed)
    new_rows, new_start, new_end = _new_inventory(source)
    if new_rows != old_rows:
        raise AssertionError("typed setter inventory differs from indexed direct methods")
    if len({row.name for row in new_rows}) != len(new_rows):
        raise AssertionError("duplicate public setter name")
    if len({row.field for row in new_rows}) != len(new_rows):
        raise AssertionError("duplicate runtime dependency field mapping")

    indexed_outside = indexed[:old_start] + indexed[old_end:]
    source_without_macro = source[:macro_start] + source[macro_start + len(MACRO) :]
    source_outside = source_without_macro[: new_start - len(MACRO)] + source_without_macro[
        new_end - len(MACRO) :
    ]
    if source_outside != indexed_outside:
        raise AssertionError("bytes outside the setter family changed")


class RuntimeDepsSetterSourceTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.source = SOURCE.read_text(encoding="utf-8")
        cls.indexed = _git("show", f":{SOURCE_PATH.as_posix()}")

    def test_index_preimage_and_expansion_are_exact(self) -> None:
        self.assertEqual(_git("rev-parse", f":{SOURCE_PATH.as_posix()}").strip(), INDEX_BLOB)
        _validate_source(self.source, self.indexed)

    def test_mutated_method_name_is_rejected(self) -> None:
        changed = self.source.replace(
            "with_privacy_release_anchor(", "with_privacy_release_head(", 1
        )
        with self.assertRaises(AssertionError):
            _validate_source(changed, self.indexed)

    def test_mutated_field_mapping_is_rejected(self) -> None:
        changed = self.source.replace(
            ") => privacy_release_anchor;",
            ") => transparency_leader_lease_provider;",
            1,
        )
        with self.assertRaises(AssertionError):
            _validate_source(changed, self.indexed)

    def test_mutated_dependency_type_is_rejected(self) -> None:
        changed = self.source.replace(
            "ProductionPrivacyReleaseAnchorV1", "ProductionPrivacyCyclePrfProviderV1", 1
        )
        with self.assertRaises(AssertionError):
            _validate_source(changed, self.indexed)

    def test_callback_escape_hatch_is_rejected(self) -> None:
        changed = self.source.replace(
            "    define_runtime_dep_setters_v1! {",
            "    // dyn Fn callback\n    define_runtime_dep_setters_v1! {",
            1,
        )
        with self.assertRaises(AssertionError):
            _validate_source(changed, self.indexed)

    def test_emitter_mutation_is_rejected(self) -> None:
        changed = self.source.replace("self.$field = Some($argument);", "self.$field = None;", 1)
        with self.assertRaises(AssertionError):
            _validate_source(changed, self.indexed)


if __name__ == "__main__":
    unittest.main()
