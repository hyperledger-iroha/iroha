"""Static child projection of the sole native ABI owner; never load an addon."""
from __future__ import annotations
import ast
import json
from pathlib import Path
import re
import pytest

ROOT = Path(__file__).resolve().parents[2]
SESSION = ROOT / "scripts/sorafs_javascript_child_session.mjs"
OWNER = ROOT / "scripts/check_native_sdk_artifact.py"

def bridge_abi_version() -> int:
    """Read the required ABI from the sole native artifact policy owner."""
    source = ast.parse(OWNER.read_text())
    values = [ast.literal_eval(node.value) for node in source.body
              if isinstance(node, ast.Assign)
              and any(isinstance(target, ast.Name)
                      and target.id == "REQUIRED_BRIDGE_ABI_VERSION"
                      for target in node.targets)]
    assert len(values) == 1 and type(values[0]) is int
    return values[0]

def node_policy(name: str):
    """Read only the exact Node literal of the source-owned Python policy AST."""
    source = ast.parse(OWNER.read_text())
    matches = [n for n in source.body if isinstance(n, ast.AnnAssign)
               and isinstance(n.target, ast.Name) and n.target.id == name]
    assert len(matches) == 1
    mapping = matches[0].value
    assert isinstance(mapping, ast.Dict)
    rows = [v for k, v in zip(mapping.keys, mapping.values)
            if isinstance(k, ast.Constant) and k.value == "node"]
    assert len(rows) == 1
    return ast.literal_eval(rows[0])

def check_abi_projection(source: str) -> None:
    """Require the finite child ABI inventory to match its original policy."""
    arrays = re.findall(r"const ABI_SYMBOLS = Object\.freeze\((\[[^\]]*\])\);", source)
    assert len(arrays) == 1
    # This specific source declaration consists only of JSON string literals;
    # remove its JS trailing comma, never evaluate executable JavaScript.
    names = json.loads(re.sub(r",\s*]$", "]", arrays[0]))
    assert tuple(names) == node_policy("REQUIRED_SYMBOLS")
    retired_arrays = re.findall(r"const RETIRED_ABI_SYMBOLS = Object\.freeze\((\[[^\]]*\])\);", source)
    assert len(retired_arrays) == 1
    retired = json.loads(re.sub(r",\s*]$", "]", retired_arrays[0]))
    assert tuple(retired) == node_policy("RETIRED_PROTOCOL_SYMBOLS")
    assert '!RETIRED_ABI_SYMBOLS.includes(name)' in source
    prefix = 'const retiredPrefix = "connect_norito_" + ["cash", "offline"].reverse().join("_") + "_";'
    assert prefix in OWNER.read_text()
    assert prefix in source
    assert '!name.startsWith(retiredPrefix)' in source
    assert f'version === {bridge_abi_version()} && Number.isSafeInteger(version)' in source

def test_native_abi_projection_uses_original_policy() -> None:
    check_abi_projection(SESSION.read_text())

@pytest.mark.parametrize("old,new", [
    ('"connectNoritoBridgeAbiVersion",', ''),
    ('"verifySorafsOrderbookSubmissionReceiptV1",', '"verifyOtherReceipt",'),
    ('"validationFeeCurrentPolicyProofRequestV1",', ''),
    ('"validationFeeVerifyCurrentPolicyProofV1",', '"validationFeeVerifyHijiriQuoteResponseV1",'),
    ('"validationFeeHijiriQuoteRequestV1",', ''),
    ('"validationFeeVerifyHijiriQuoteResponseV1",', ''),
    ('!RETIRED_ABI_SYMBOLS.includes(name)', 'true'),
    ('!name.startsWith(retiredPrefix)', 'true'),
    ('["cash", "offline"].reverse().join("_")', '["cash", "offline"].join("_")'),
    (f'version === {bridge_abi_version()}', f'version === {bridge_abi_version() - 1}'),
])
def test_native_policy_mutation_is_rejected(old: str, new: str) -> None:
    source = SESSION.read_text()
    changed = source.replace(old, new)
    assert changed != source
    with pytest.raises(AssertionError):
        check_abi_projection(changed)
