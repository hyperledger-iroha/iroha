# Sumeragi Data-Availability Report

Processed 3 summary file(s) from `artifacts/sumeragi-da/20251005T190335Z`.

## Summary

| Scenario | Runs | Peers | Payload (MiB) | RBC deliver median (ms) | RBC deliver max (ms) | Commit median (ms) | Commit max (ms) | Throughput median (MiB/s) | Throughput min (MiB/s) | RBC<=Commit | BG queue max | P2P drops max |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| sumeragi_rbc_da_large_payload_four_peers | 1 | 4 | 10.50 | 3120 | 3120 | 3380 | 3380 | 3.28 | 3.28 | yes | 18 | 0 |
| sumeragi_rbc_da_large_payload_six_peers | 1 | 6 | 10.50 | 3280 | 3280 | 3560 | 3560 | 3.21 | 3.21 | yes | 24 | 0 |
| sumeragi_rbc_recovers_after_peer_restart | 1 | 4 | 10.50 | 3340 | 3340 | 3620 | 3620 | 3.16 | 3.16 | yes | 19 | 0 |

### sumeragi_rbc_da_large_payload_four_peers

- runs: 1
- peers: 4
- payload: 11010048 bytes (10.50 MiB)
- RBC chunks observed: 168
- READY vote counts: 4
- RBC<=Commit observed: yes
- RBC delivery mean (ms): 3120.00
- Commit mean (ms): 3380.00
- Throughput mean (MiB/s): 3.28
- BG post queue depth max/median: 18 / 18
- P2P queue drops max/median: 0 / 0
- per-peer payload bytes: 11010048 - 11010048
- per-peer deliver broadcasts: 1 - 1
- per-peer ready broadcasts: 1 - 1

| Run | Source | Block | Height | View | RBC deliver (ms) | Commit (ms) | Throughput (MiB/s) | RBC<=Commit | READY | Total chunks | Received | BG queue max | P2P drops |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | sumeragi_rbc_da_large_payload_four_peers.summary.json | 0x8f2fd8804f... | 58 | 2 | 3120 | 3380 | 3.28 | yes | 4 | 168 | 168 | 18 | 0 |

### sumeragi_rbc_da_large_payload_six_peers

- runs: 1
- peers: 6
- payload: 11010048 bytes (10.50 MiB)
- RBC chunks observed: 168
- READY vote counts: 5
- RBC<=Commit observed: yes
- RBC delivery mean (ms): 3280.00
- Commit mean (ms): 3560.00
- Throughput mean (MiB/s): 3.21
- BG post queue depth max/median: 24 / 24
- P2P queue drops max/median: 0 / 0
- per-peer payload bytes: 11010048 - 11010048
- per-peer deliver broadcasts: 1 - 1
- per-peer ready broadcasts: 1 - 1

| Run | Source | Block | Height | View | RBC deliver (ms) | Commit (ms) | Throughput (MiB/s) | RBC<=Commit | READY | Total chunks | Received | BG queue max | P2P drops |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | sumeragi_rbc_da_large_payload_six_peers.summary.json | 0x71e3cdebcf... | 59 | 3 | 3280 | 3560 | 3.21 | yes | 5 | 168 | 168 | 24 | 0 |

### sumeragi_rbc_recovers_after_peer_restart

- runs: 1
- peers: 4
- payload: 11010048 bytes (10.50 MiB)
- RBC chunks observed: 168
- READY vote counts: 4
- RBC<=Commit observed: yes
- RBC delivery mean (ms): 3340.00
- Commit mean (ms): 3620.00
- Throughput mean (MiB/s): 3.16
- BG post queue depth max/median: 19 / 19
- P2P queue drops max/median: 0 / 0
- per-peer payload bytes: 11010048 - 11010048
- per-peer deliver broadcasts: 1 - 1
- per-peer ready broadcasts: 1 - 1

| Run | Source | Block | Height | View | RBC deliver (ms) | Commit (ms) | Throughput (MiB/s) | RBC<=Commit | READY | Total chunks | Received | BG queue max | P2P drops |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | sumeragi_rbc_recovers_after_peer_restart.summary.json | 0xeaf2198957... | 60 | 3 | 3340 | 3620 | 3.16 | yes | 4 | 168 | 168 | 19 | 0 |
