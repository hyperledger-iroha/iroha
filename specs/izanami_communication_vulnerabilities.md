<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Izanami Communication Vulnerability Matrix

This page maps the Izanami chaos harness to the five protocol-agnostic attacks
from Andrei Lebedev and Vincent Gramoli, "Blockchain Communication
Vulnerabilities" (arXiv:2603.02661v1, DOI 10.48550/arXiv.2603.02661).

The paper compares Algorand, Aptos, Avalanche, Redbelly, and Solana under a
common 20-node, 800-second, 200 TPS setup. Timed fault experiments inject the
fault from 133s to 266s. Izanami uses the same labels so Iroha results can be
reported next to the paper baseline. In `--mode paper`, the matrix runner passes
`--fault-window-start 133s --fault-window-end 266s` to every faulted scenario.

## Scenario Catalog

| Scenario | Paper attack | Izanami coverage |
| --- | --- | --- |
| `targeted-load` | One client sends valid transfer traffic at 200 TPS to a single blockchain node. | Native: one Izanami submitter pins submissions to one preferred Torii endpoint unless failover is needed. |
| `transient-failure` | A small node fraction crashes at 133s and recovers at 266s. | Native crash/restart shape during the paper's timed fault window; the matrix script records recovery/loss metrics. |
| `packet-loss` | 25-75% packet loss is introduced between a fault-threshold-sized group and the rest of the network. | Native Izanami fault: the matrix applies 75% in-process P2P application-frame loss during the paper's fault window without changing the validator roster. |
| `stopping` | A large node fraction crashes and then rejoins; post-recovery liveness is the key signal. | Native crash/restart shape with a large faulty-peer count. |
| `leader-isolation` | The current consensus leader gets 75% inbound and outbound packet loss during its leader window. | Native Izanami fault: Izanami samples Sumeragi leader telemetry and applies 75% in-process P2P application-frame loss to the current leader during the paper's fault window. |

## Paper Baseline

| Scenario | Algorand | Aptos | Avalanche | Redbelly | Solana |
| --- | --- | --- | --- | --- | --- |
| `targeted-load` | resilient | vulnerable | inconclusive | resilient | resilient |
| `transient-failure` | resilient | degraded | vulnerable | resilient | degraded |
| `packet-loss` | vulnerable | degraded | degraded | degraded | resilient |
| `stopping` | resilient | resilient | vulnerable | resilient | vulnerable |
| `leader-isolation` | n/a | vulnerable | degraded | n/a | vulnerable |

Interpret `degraded` as material performance loss without the paper's strongest
vulnerability classification. `inconclusive` marks Avalanche targeted-load
results because the paper disabled base-fee escalation to isolate communication
effects.

## Running The Matrix

For a local smoke pass:

```bash
scripts/run_izanami_communication_vulnerability_matrix.sh --mode quick
```

Run both Sumeragi validator-selection modes when comparing Iroha against the
paper:

```bash
scripts/run_izanami_communication_vulnerability_matrix.sh --mode quick --sumeragi-mode both
```

For a paper-shaped pass:

```bash
scripts/run_izanami_communication_vulnerability_matrix.sh --mode paper -- --seed 7
```

The runner accepts `--sumeragi-mode permissioned`, `--sumeragi-mode npos`, or
`--sumeragi-mode both`. Permissioned mode uses the default Sumeragi validator
roster. NPoS mode passes `--nexus` to Izanami, which loads the Nexus/Sora
profile and sets `sumeragi.consensus_mode = "npos"`.

The helper writes `summary.md`, `summary.tsv`, and per-scenario logs under
`dist/izanami-communication-vuln-*`. The Markdown report includes the paper
baseline and the final `izanami::summary` line for each Iroha run.

Use `--only <scenario>` while iterating:

```bash
scripts/run_izanami_communication_vulnerability_matrix.sh --only packet-loss -- --target-blocks 200
```

## Classification Guidance

Classify Iroha against each paper scenario with these signals:

| Scenario | Primary Iroha signals |
| --- | --- |
| `targeted-load` | p50/p95 commit latency, ingress queue pressure, unexpected submission failures. |
| `transient-failure` | recovery time, committed/offered ratio, quorum and strict height progress. |
| `packet-loss` | height progress, p50/p95 latency, P2P/DA/RBC drop counters. |
| `stopping` | post-recovery liveness, height progress, committed/offered ratio. |
| `leader-isolation` | zero-throughput windows, proposer/leader telemetry, recovery time. |

Record the final classification and artifact path in `status.md` after a full
paper-shaped run.
