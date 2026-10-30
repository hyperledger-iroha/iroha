"""Keep SDK governance inventories aligned with the current Norito model.

Run from a checkout with ``python3 -m pytest -q
pytests/scripts/test_sccp_sdk_governance.py``. No compiler, network or signing
inputs are needed. These source checks bind action/subject maps, coordinate
ownership and ordering to Rust; Swift/Kotlin runtime tests own decoder behavior.
"""

import re
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
SDK_PATHS = {
    "swift": "IrohaSwift/Sources/IrohaSwift/SccpGovernanceProposalValidator.swift",
    "kotlin": (
        "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/"
        "ParliamentProposalValidatorV1.kt"
    ),
}
MODEL = "crates/iroha_data_model/src/sccp/governance.rs"
NETWORK_MODEL = "crates/iroha_data_model/src/bridge/sccp.rs"


def _one(pattern: str, source: str) -> str:
    matches = re.findall(pattern, source, re.DOTALL)
    assert len(matches) == 1, "review changed source inventory syntax"
    return matches[0]


def _model_variants(name: str, path: str = MODEL) -> dict[str, tuple[str, str | None]]:
    source = (ROOT / path).read_text()
    marker = f"pub enum {name} {{\n"
    assert source.count(marker) == 1
    body = source.split(marker, 1)[1].split("\n}", 1)[0]
    body = re.sub(r"^\s*//[^\n]*", "", body, flags=re.MULTILINE)
    variant = re.compile(
        r'\s*#\[codec\(index = \d+\)\]\s*'
        r'#\[norito\(rename = "([a-z_]+)"\)\]\s*'
        r'([A-Za-z0-9_]+)(?:\(([A-Za-z0-9_]+)\))?,'
    )
    rows = variant.findall(body)
    assert rows and not variant.sub("", body).strip(), "review changed Norito enum syntax"
    assert len({tag for tag, _, _ in rows}) == len(rows), "duplicate wire tag"
    assert len({rust_name for _, rust_name, _ in rows}) == len(rows), "duplicate variant"
    return {tag: (rust_name, payload or None) for tag, rust_name, payload in rows}


def _tags(literals: str) -> list[str]:
    tags = re.findall(r'"([a-z_]+)"', literals)
    assert tags and len(tags) == len(set(tags)), "empty or duplicate SDK tags"
    assert not re.sub(r'"[a-z_]+"|[\s,]', "", literals), "review changed SDK tag syntax"
    return tags


def _action_subjects() -> dict[str, str]:
    """Read the complete Rust subject() match, including grouped action arms."""
    source = (ROOT / MODEL).read_text()
    body = _one(
        r"pub fn subject\(&self\) -> SccpGovernanceSubjectV1 \{"
        r"\s*match self \{(.*?)\n        \}\n    \}",
        source,
    )
    arm = re.compile(
        r"\s*(Self::.*?)=>\s*\{?\s*SccpGovernanceSubjectV1::([A-Za-z0-9_]+)"
        r"(?:\([^)]*\))?\s*\}?,?",
        re.DOTALL,
    )
    rows = arm.findall(body)
    assert rows and not arm.sub("", body).strip(), "review changed subject match syntax"
    actions = {
        name: tag for tag, (name, _) in _model_variants("SccpGovernanceActionV1").items()
    }
    subjects = {
        name: tag for tag, (name, _) in _model_variants("SccpGovernanceSubjectV1").items()
    }
    result = {}
    for patterns, subject in rows:
        for name in re.findall(r"Self::([A-Za-z0-9_]+)\(", patterns):
            tag = actions[name]
            assert tag not in result, "duplicate Rust action subject"
            result[tag] = subjects[subject]
    assert set(result) == set(actions.values()), "incomplete Rust action subject inventory"
    return result


def _sdk_action_subjects(source: str, language: str) -> dict[str, str]:
    if language == "swift":
        body = _one(r"private static let actions:.*?= \[(.*?)\n    \]", source)
        row = re.compile(r'\s*"([a-z_]+)":\s*\("([a-z_]+)",\s*\[[^\]]*\]\),')
    else:
        body = _one(r"private val SCCP_ACTIONS\s*=\s*mapOf\((.*?)\n\s*\)", source)
        row = re.compile(
            r'\s*"([a-z_]+)"\s+to\s+\("([a-z_]+)"\s+to\s+'
            r'(?:setOf\([^)]*\)|emptySet\(\))\),'
        )
    rows = row.findall(body)
    assert rows and not row.sub("", body).strip(), "review changed SDK action-map syntax"
    assert len({tag for tag, _ in rows}) == len(rows), "duplicate SDK action"
    return dict(rows)


def _sdk_subjects(source: str, language: str) -> list[str]:
    pattern = (
        r"private static let subjects\s*=\s*\[(.*?)\]"
        if language == "swift"
        else r"private val SCCP_SUBJECTS\s*=\s*listOf\((.*?)\)"
    )
    return _tags(_one(pattern, source))


@pytest.mark.parametrize("language", SDK_PATHS)
def test_action_tags_match_current_norito_model(language: str) -> None:
    source = (ROOT / SDK_PATHS[language]).read_text()
    expected = _action_subjects()
    actual = _sdk_action_subjects(source, language)
    assert actual == expected, (language, "action-to-subject inventory")


@pytest.mark.parametrize("language", SDK_PATHS)
def test_subject_tags_and_coordinate_owners_match_current_norito_model(
    language: str,
) -> None:
    source = (ROOT / SDK_PATHS[language]).read_text()
    variants = _model_variants("SccpGovernanceSubjectV1")
    assert {payload for _, payload in variants.values()} == {
        None,
        "SccpNetworkV1",
    }, "review new subject coordinate type"
    expected_null = {tag for tag, (_, payload) in variants.items() if payload is None}
    subjects = _sdk_subjects(source, language)
    assert subjects == list(variants), (language, "closed subjects in Rust declaration order")
    if language == "swift":
        body = _one(r"private func subject\((.*?)\n    private func network\(", source)
        assert "Self.subjects.firstIndex(of: tag)" in body
        null = re.findall(r'if tag == "([a-z_]+)"\s*\{\s*guard item\["key"\] == \.null', body)
        assert re.search(
            r'return index \* (\d+)\s*\}\s*return index \* \1 '
            r'\+ \(try network\(item\["key"\]',
            body,
        )
    else:
        body = _one(
            r"private fun sccpGovernanceSubject\((.*?)\n    private fun sccpSubjectOrder\(",
            source,
        )
        assert "require(tag in SCCP_SUBJECTS)" in body
        null = re.findall(
            r'if \(tag == "([a-z_]+)"\)\s*\{\s*require\(value\["key"\] == null\)',
            body,
        )
        assert re.search(
            r'null\s*\} else \{\s*sccpExternalNetwork\(objectValue\(value\["key"\]',
            body,
        )
        assert "return sccpSubjectOrder(tag, network)" in body
    assert set(null) == expected_null and len(null) == len(expected_null), (
        language,
        "null subjects",
    )


@pytest.mark.parametrize("language", SDK_PATHS)
def test_coordinate_order_matches_rust_external_network_order(language: str) -> None:
    source = (ROOT / SDK_PATHS[language]).read_text()
    model = (ROOT / NETWORK_MODEL).read_text()
    sora = _one(
        r"pub const fn is_sora\(self\) -> bool \{"
        r"\s*matches!\(self, Self::([A-Za-z0-9_]+)\)\s*\}",
        model,
    )
    assert re.search(
        r"pub const fn is_external\(self\) -> bool \{\s*!self\.is_sora\(\)\s*\}",
        model,
    )
    expected = [
        tag
        for tag, (name, _) in _model_variants("SccpNetworkV1", NETWORK_MODEL).items()
        if name != sora
    ]
    if language == "swift":
        actual = _tags(_one(r"private static let networks\s*=\s*\[(.*?)\]", source))
        assert "Self.networks.firstIndex(of: tag)" in source
        stride = int(_one(r'return index \* (\d+) \+ \(try network\(item\["key"\]', source))
        assert stride >= len(expected)
        assert "guard baseSubjects == Set(touched).sorted() else" in source
    else:
        actual = _tags(
            _one(r"private val SCCP_EXTERNAL_NETWORKS\s*=\s*listOf\((.*?)\)", source)
        )
        stride = int(
            _one(
                r"SCCP_SUBJECTS\.indexOf\(subject\) \* (\d+) "
                r"\+ if \(network == null\) 0 else "
                r"SCCP_EXTERNAL_NETWORKS\.indexOf\(network\) \+ 1",
                source,
            )
        )
        assert stride > len(expected)
        assert "val subjects = sortedSetOf<Int>()" in source
        assert "require(actualSubjects == subjects.toList())" in source
    assert actual == expected, (language, "external network declaration order")
