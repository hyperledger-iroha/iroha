"""Authenticate the typed ``InstructionBox`` conversion macro compaction."""

from __future__ import annotations

import hashlib
import re
import subprocess
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "crates/iroha_data_model/src/isi/mod.rs"
PREIMAGE_BLOB = "20d76d60119fc6fdf216fd073760935d98dae984"
PREIMAGE_LINES = 4_094
POSTIMAGE_REGION_LINES = 397
PREIMAGE_REGULAR_DIRECT_SITES = 269
EXPECTED_DIRECT_SITES = 272

REGION_START = "impl crate::seal::Instruction for InstructionBox {}"
REGION_END = "/// Object-safe cloning support for [`Instruction`] trait objects."

DIRECT_IMPL = re.compile(
    r"(?P<attrs>(?:#\[[^\n]+\]\n)*)"
    r"impl From<(?P<ty>[^>]+)> for InstructionBox \{\n"
    r"    fn from\((?P<arg>[A-Za-z_][A-Za-z0-9_]*): (?P=ty)\) -> Self \{\n"
    r"        InstructionBox\(Box::new\((?P=arg)\)\)\n"
    r"    \}\n"
    r"\}"
)

RETIRED_DIRECT_CALLS = (
    "impl_direct_instruction_box!(crate::isi::account_alias_lease::AcquireAccountAliasLease);",
    "impl_direct_instruction_box!(crate::isi::domain_link::SetAccountAliasBinding);",
)
OFFLINE_LIFECYCLE_ANCHOR = (
    "impl_direct_instruction_box!(crate::isi::offline::ActivateKagemushaRecursiveReleaseV4);"
)
OFFLINE_LIFECYCLE_CALLS = """impl_direct_instruction_box!(crate::isi::offline::EnableKagemushaRecursiveIssuanceV4);
impl_direct_instruction_box!(crate::isi::offline::CancelKagemushaRecursiveReleaseV4);
impl_direct_instruction_box!(crate::isi::offline::DeactivateKagemushaRecursiveIssuanceV4);
impl_direct_instruction_box!(crate::isi::offline::RecordKagemushaTairaCanaryV4);
impl_direct_instruction_box!(crate::isi::offline::AuthorizeKagemushaTairaCanaryV4);"""
RETIRED_SPECIAL_GAP_START = (
    "impl_direct_instruction_box!(crate::isi::soracloud::RecordSoracloudRuntimeReceipt);"
)
RETIRED_SPECIAL_GAP_END = "// Allow direct boxing of runtime upgrade instructions"

MACRO_AND_MARKER = """impl crate::seal::Instruction for InstructionBox {}

macro_rules! impl_direct_instruction_box {
    ($($instruction:ty),+ $(,)?) => {
        $(
            impl From<$instruction> for InstructionBox {
                fn from(instruction: $instruction) -> Self {
                    InstructionBox(Box::new(instruction))
                }
            }
        )+
    };
}
"""


def _git_blob(oid: str) -> str:
    result = subprocess.run(
        ["git", "cat-file", "blob", oid],
        cwd=ROOT,
        check=True,
        stdout=subprocess.PIPE,
    )
    return result.stdout.decode("utf-8")


def _region(source: str) -> str:
    start = source.index(REGION_START)
    end = source.index(REGION_END, start)
    return source[start:end]


def _compact(preimage: str) -> str:
    start = preimage.index(REGION_START)
    end = preimage.index(REGION_END, start)
    region = preimage[start:end]
    marker = REGION_START + "\n"
    if not region.startswith(marker):
        raise AssertionError("InstructionBox seal marker moved")
    region = MACRO_AND_MARKER + region[len(marker) :]

    replaced = 0

    def replace_direct(match: re.Match[str]) -> str:
        nonlocal replaced
        replaced += 1
        return (
            match.group("attrs")
            + f"impl_direct_instruction_box!({match.group('ty')});"
        )

    region = DIRECT_IMPL.sub(replace_direct, region)
    if replaced != PREIMAGE_REGULAR_DIRECT_SITES:
        raise AssertionError(
            f"expected {PREIMAGE_REGULAR_DIRECT_SITES} regular sites, found {replaced}"
        )
    for retired in RETIRED_DIRECT_CALLS:
        if region.count(retired) != 1:
            raise AssertionError(f"retired direct conversion changed: {retired}")
        region = region.replace(retired, "")
    if region.count(OFFLINE_LIFECYCLE_ANCHOR) != 1:
        raise AssertionError("offline lifecycle insertion anchor changed")
    region = region.replace(
        OFFLINE_LIFECYCLE_ANCHOR,
        OFFLINE_LIFECYCLE_ANCHOR + "\n" + OFFLINE_LIFECYCLE_CALLS,
    )
    gap_start = region.index(RETIRED_SPECIAL_GAP_START) + len(RETIRED_SPECIAL_GAP_START)
    gap_end = region.index(RETIRED_SPECIAL_GAP_END, gap_start)
    if "impl From<" not in region[gap_start:gap_end]:
        raise AssertionError("retired special direct conversion changed")
    region = region[:gap_start] + "\n" + region[gap_end:]
    if region.count("impl_direct_instruction_box!(") != EXPECTED_DIRECT_SITES:
        raise AssertionError("typed invocation count drifted")
    return preimage[:start] + region + preimage[end:]


def _tokens(source: str) -> str:
    """Ignore formatting only; comments and every Rust token remain authenticated."""

    return re.sub(r"\s+", "", source)


def _validate(source: str) -> None:
    preimage = _git_blob(PREIMAGE_BLOB)
    if len(preimage.splitlines()) != PREIMAGE_LINES:
        raise AssertionError("authenticated preimage line count drifted")
    expected = _compact(preimage)
    if _tokens(_region(source)) != _tokens(_region(expected)):
        raise AssertionError("InstructionBox macro region differs from authenticated transform")
    if len(_region(source).splitlines()) != POSTIMAGE_REGION_LINES:
        raise AssertionError("InstructionBox macro region physical shape changed")
    if _region(source).count("impl_direct_instruction_box!(") != EXPECTED_DIRECT_SITES:
        raise AssertionError("InstructionBox typed invocation inventory drifted")
    macro_digest = hashlib.sha256(_tokens(MACRO_AND_MARKER).encode()).hexdigest()
    if macro_digest != "f736fc46fe0b5e3e44845da6f598edf7c774297082671462e94dd719b62404b1":
        raise AssertionError("guard macro seal changed")


class InstructionBoxMacroSourceTest(unittest.TestCase):
    def test_authenticated_typed_macro_projection(self) -> None:
        _validate(SOURCE.read_text(encoding="utf-8"))

    def test_mutations_fail_closed(self) -> None:
        source = SOURCE.read_text(encoding="utf-8")
        mutations = (
            source.replace(
                "impl_direct_instruction_box!(crate::isi::zk::VerifyProof);",
                "",
                1,
            ),
            source.replace(
                "impl_direct_instruction_box!(crate::isi::zk::VerifyProof);",
                "impl_direct_instruction_box!(crate::isi::zk::VerifyProof);\n"
                "impl_direct_instruction_box!(crate::isi::zk::VerifyProof);",
                1,
            ),
            source.replace("crate::isi::zk::VerifyProof", "crate::isi::zk::PruneProofs", 1),
            source.replace("Box::new(instruction)", "Arc::new(instruction)", 1),
            source.replace(
                '#[cfg(feature = "governance")]\nimpl_direct_instruction_box!',
                "impl_direct_instruction_box!",
                1,
            ),
        )
        for mutated in mutations:
            with self.subTest(digest=hashlib.sha256(mutated.encode()).hexdigest()[:12]):
                with self.assertRaises(AssertionError):
                    _validate(mutated)


if __name__ == "__main__":
    unittest.main()
