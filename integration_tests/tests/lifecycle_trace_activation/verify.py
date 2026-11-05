"""Authenticate live-peer proof exports with the current native Python SDK."""
import hashlib
import json
from pathlib import Path
import sys

from iroha_python import NetworkId, authenticate_public_finality_checkpoint_v1, verify_scheduled_trigger_execution_v1

root = Path(sys.argv[1])
expected = json.loads((root / "expected.json").read_text())
network = NetworkId.parse(expected["network_id"])
genesis = (root / "genesis.wire").read_bytes()
checkpoint = authenticate_public_finality_checkpoint_v1(
    genesis,
    tuple((root / f"prefix-{height}.norito").read_bytes()
          for height in range(1, expected["registration_height"])),
    expected_network_id=network, expected_chain=expected["chain"],
    expected_genesis_sha256=hashlib.sha256(genesis).digest(),
)["checkpoint"]
result = verify_scheduled_trigger_execution_v1(
    (root / "registration.wire").read_bytes(), expected["trigger"],
    native_finality_proof_chain_json=(root / "proofs.json").read_text(),
    expected_network_id=network, expected_chain=expected["chain"],
    trusted_checkpoint=checkpoint, expected_chain_discriminant=expected["discriminant"],
    lifecycle_activation_json=(root / "witness.json").read_text(),
    lifecycle_activation_policy_json=(root / "activation.json").read_text(),
)
assert result["terminal"] == expected["terminal"], result
assert len(result["outcomes"]) == expected["attempts"], result
if expected["terminal"] == "applied":
    assert result["outcomes"][-1]["result_ok"] is True, result
else:
    assert all(row["result_ok"] is False for row in result["outcomes"]), result
print(f"PYTHON_LIFECYCLE_OK trigger={expected['trigger']} terminal={result['terminal']} attempts={len(result['outcomes'])}")
