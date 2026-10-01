"""Exercise required native verifier trust-input boundaries without a native wheel.

Proof acceptance and address projection are separately tested against the real
native BLS execution fixture. These tests execute the actual public wrappers.
"""
from __future__ import annotations

import ast
import json
from pathlib import Path
import types
from typing import Any

import pytest

PACKAGE_ROOT = Path(__file__).resolve().parents[1] / "src" / "iroha_python"


class FakeNetworkId:
    def __init__(self, value: bytes):
        self.value = value


NETWORK_ID = FakeNetworkId(b"\x77" * 32)


def _install_network_id_contract(module):
    def require(value, name):
        if not isinstance(value, FakeNetworkId):
            raise TypeError(f"{name} must be a NetworkId")
        return value
    module._require_network_id = require


def test_committed_output_crypto_wrapper_requires_and_forwards_exact_trust_inputs() -> None:
    # Exercise the actual source wrapper without loading an older installed native ABI.
    # Cryptographic acceptance is covered by the real-BLS native boundary tests.
    import ast
    from typing import Mapping

    source = (PACKAGE_ROOT / "crypto.py").read_text()
    node = next(item for item in ast.parse(source).body
                if isinstance(item, ast.FunctionDef)
                and item.name == "verify_committed_transaction_inclusion")
    calls = []
    native = types.SimpleNamespace(verify_committed_transaction_inclusion=
        lambda *args, **kwargs: calls.append((args, kwargs)) or ('{"output_hash":"verified"}', b"promoted-checkpoint"))
    contract = types.ModuleType("contract")
    _install_network_id_contract(contract)
    namespace = {"_crypto": native, "_require_network_id": contract._require_network_id,
                 "NetworkId": FakeNetworkId, "Mapping": Mapping, "Any": Any, "json": json}
    exec(compile(ast.Module(body=[node], type_ignores=[]), "crypto.py", "exec"), namespace)
    verify = namespace["verify_committed_transaction_inclusion"]
    trust = dict(native_finality_proof_chain_json="[exact-native-proof]", expected_network_id=NETWORK_ID,
                 expected_chain="trusted-chain", expected_chain_discriminant=369, trusted_checkpoint=b"trusted-checkpoint")
    assert verify("transaction", b"response", **trust) == {"output_hash": "verified", "promoted_checkpoint": b"promoted-checkpoint"}
    assert calls == [(("transaction", b"response"), trust)]
    for response in [bytearray(b"response"), memoryview(b"response")]:
        with pytest.raises(TypeError, match="exact immutable bytes"):
            verify("transaction", response, **trust)
    with pytest.raises(TypeError, match="expected_network_id must be a NetworkId"):
        verify("transaction", b"response", **{**trust, "expected_network_id": b"untrusted"})
    with pytest.raises(ValueError, match="16 MiB"):
        verify("transaction", b"response", **{**trust, "native_finality_proof_chain_json": " " * (16 * 1024 * 1024 + 1)})
    for discriminant in (None, True, "369", 369.0):
        with pytest.raises(TypeError, match="expected_chain_discriminant must be an exact integer"):
            verify("transaction", b"response", **{**trust, "expected_chain_discriminant": discriminant})
    for discriminant in (-1, 65536):
        with pytest.raises(ValueError, match="expected_chain_discriminant must be a u16"):
            verify("transaction", b"response", **{**trust, "expected_chain_discriminant": discriminant})
    with pytest.raises(TypeError, match="required keyword-only"):
        verify("transaction", b"response")
    with pytest.raises(TypeError, match="expected_chain_discriminant"):
        verify("transaction", b"response", **{key: value for key, value in trust.items() if key != "expected_chain_discriminant"})
    assert len(calls) == 1


def test_committed_verifier_rejects_mutable_or_empty_checkpoint_before_native_call() -> None:
    import ast
    from typing import Mapping
    node = next(item for item in ast.parse((PACKAGE_ROOT / "crypto.py").read_text()).body
                if isinstance(item, ast.FunctionDef) and item.name == "verify_committed_transaction_inclusion")
    calls = []
    native = types.SimpleNamespace(verify_committed_transaction_inclusion=lambda *args, **kwargs: calls.append((args, kwargs)))
    contract = types.ModuleType("contract")
    _install_network_id_contract(contract)
    namespace = {"_crypto": native, "_require_network_id": contract._require_network_id,
                 "NetworkId": FakeNetworkId, "Mapping": Mapping, "Any": Any, "json": json}
    exec(compile(ast.Module(body=[node], type_ignores=[]), "crypto.py", "exec"), namespace)
    verify = namespace["verify_committed_transaction_inclusion"]
    trust = dict(native_finality_proof_chain_json="[]", expected_network_id=NETWORK_ID,
                 expected_chain="chain", expected_chain_discriminant=369, trusted_checkpoint=b"checkpoint")
    for checkpoint in (bytearray(b"checkpoint"), memoryview(b"checkpoint")):
        with pytest.raises(TypeError, match="exact immutable bytes"):
            verify("transaction", b"response", **{**trust, "trusted_checkpoint": checkpoint})
    for checkpoint in (b"", b"x" * (68 * 1024 * 1024 + 1)):
        with pytest.raises(ValueError, match="68 MiB"):
            verify("transaction", b"response", **{**trust, "trusted_checkpoint": checkpoint})
    for chain in ("", "x" * 1025):
        with pytest.raises(ValueError, match="1024 UTF-8"):
            verify("transaction", b"response", **{**trust, "expected_chain": chain})
    assert calls == []


def test_client_requires_selected_chain_context_before_native_query():
    tree = ast.parse((PACKAGE_ROOT / "client.py").read_text())
    client = next(node for node in tree.body if isinstance(node, ast.ClassDef) and node.name == "ToriiClient")
    method = next(node for node in client.body if isinstance(node, ast.FunctionDef) and node.name == "get_verified_committed_transaction")
    namespace = {}
    module = ast.Module(body=[ast.ImportFrom(module="__future__", names=[ast.alias(name="annotations")], level=0), method], type_ignores=[])
    exec(compile(ast.fix_missing_locations(module), "client.py", "exec"), namespace)
    verify = namespace[method.name]
    selected = types.SimpleNamespace(_chain_discriminant=369)
    inputs = dict(transaction_hash="11" * 32, authority="authority", network_id=NETWORK_ID,
                  native_finality_proof_chain_json="[]", expected_chain="trusted-chain", trusted_checkpoint=b"checkpoint")
    with pytest.raises(TypeError, match="expected_chain_discriminant"):
        verify(selected, **inputs)
    for discriminant in (None, True, 369.0, "369"):
        with pytest.raises(TypeError, match="exact integer"):
            verify(selected, **inputs, expected_chain_discriminant=discriminant)
    for discriminant in (-1, 65536):
        with pytest.raises(ValueError, match="u16"):
            verify(selected, **inputs, expected_chain_discriminant=discriminant)
    with pytest.raises(ValueError, match="configured client"):
        verify(selected, **inputs, expected_chain_discriminant=751)
