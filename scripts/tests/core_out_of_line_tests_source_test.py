"""Preserve Core test-module ownership and relative fixture paths after extraction."""

from pathlib import Path
import re

import pytest


ROOT = Path(__file__).resolve().parents[2]
MODULES = (
    ("crates/iroha_core/src/executor.rs", "executor_tests.rs"),
    ("crates/iroha_core/src/smartcontracts/ivm/host.rs", "host_tests.rs"),
)


@pytest.mark.parametrize("owner, child", MODULES)
def test_large_core_tests_are_loaded_only_by_the_original_test_module(owner: str, child: str) -> None:
    """A source move must retain the `tests` namespace and production cfg gate."""
    parent = ROOT / owner
    source = parent.read_text(encoding="utf-8")
    declaration = f'#[cfg(test)]\n#[path = "{child}"]\nmod tests;'
    assert source.count(declaration) == 1
    assert not re.search(r"(?m)^mod tests\s*\{", source)
    assert parent.with_name(child).read_text(encoding="utf-8").startswith("//!")


@pytest.mark.parametrize("owner, child", MODULES)
def test_extracted_core_tests_keep_every_literal_include_at_its_original_location(owner: str, child: str) -> None:
    """Sibling test sources retain source fragments and literal fixture custody."""
    parent = ROOT / owner
    test_source = parent.with_name(child)
    includes = re.findall(
        r'\binclude(?:_str|_bytes)?!\(\s*"([^"\n]+)"',
        test_source.read_text(encoding="utf-8"),
    )
    assert includes
    for relative in includes:
        original = (parent.parent / relative).resolve()
        extracted = (test_source.parent / relative).resolve()
        assert extracted == original
        assert extracted.is_relative_to(ROOT)
        assert extracted.is_file(), relative
