# Iroha Torii client

The lightweight client exposes authoritative Sumeragi v2 status separately
from the general operational-health endpoint:

```python
from iroha_torii_client import ToriiClient

client = ToriiClient("http://127.0.0.1:8080")
status = client.get_sumeragi_status()

print(status.height, status.view, status.phase)
print(status.height_context.mode, status.height_context.validator_count)
if status.safety_halt.active:
    print("consensus safety halt:", status.safety_halt.reason)
if status.last_commit_qc is not None:
    qc = status.last_commit_qc
    print(qc.signer_count, qc.validator_count, qc.signed_power, qc.total_power)

for block in status.committed_lane_blocks:
    print(block["lane_id"], block["lane_block_height"], block["execution_status"])
```

`get_sumeragi_status()` validates the flattened JSON projection from
`GET /v1/sumeragi/status`. It rejects non-v2 protocol values, malformed frozen
height contexts, out-of-range leaders, inconsistent or under-quorum CommitQCs,
malformed safety-halt state, impossible bounded queue occupancy, and
absent/malformed canonical lane arrays.
The endpoint's binary Norito representation instead nests the same reducer
snapshot under `authoritative` in `SumeragiV2StatusResponse`.

Use `get_status_snapshot()` for `/v1/status`. That route remains a distinct
operational-health surface; its queue and historical lane telemetry must not be
treated as consensus-authoritative state.
