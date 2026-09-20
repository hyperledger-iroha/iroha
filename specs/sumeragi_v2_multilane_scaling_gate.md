---
title: Sumeragi V2 Multilane Scaling Gate
sidebar_label: Multilane Scaling Gate
description: Source-coupled contract for the fixed five-pair G-SCALE collector and parent execution record.
---

# Sumeragi V2 multilane scaling gate

`G-SCALE` requires five paired one-lane/four-lane measurements on pinned hardware.
The production source implements a fixed native collector and an independent
parent verification path. Its real native execution and release qualification
remain open in the [closure ledger](sumeragi_v2_multilane_closure_ledger.md).

## Fixed execution and inputs

The protected release parent invokes
`scripts/nexus/run_multilane_scaling_gate.py` with an admitted Python runtime,
`-I -B -S`, and exactly these arguments:

- `--launch-input-fd`: the inherited read-only launch record descriptor;
- `--launch-input-sha256`: the SHA-256 of those exact original bytes;
- `--seed-fd`: a distinct inherited ready, nonblocking seed pipe.

`scaling_cli_bootstrap.fixed_scaling_argv` constructs the command. The launch
record names the admitted runtime, dependency and worker source owners, complete
plan and budget, and fresh evidence/runtime outputs. The seed is consumed from
its original pipe and remains absent from arguments, environment and public
artifacts. The [fixed command contract](../docs/scaling_fixed_command.md) and
[input contract](../docs/scaling_experiment_config.md) identify the schema owners
and allocation limits.

The complete typed plan specifies exactly ten trials in this order:

```text
pair 1: one_lane, four_lane
pair 2: one_lane, four_lane
pair 3: one_lane, four_lane
pair 4: one_lane, four_lane
pair 5: one_lane, four_lane
```

Paired trials share their derived deterministic seed. Offered work, account
count and resource limits match across variants; the declared lane count and
paired seed are the permitted differences. Each trial admits all native
policies and an allocation for each public artifact before starting processes.
The command exposes no run-count, threshold, external trial-command or
continue-on-failure option.

## Fixed localnet input contract

`kagami localnet --scaling-lanes` consumes its development seed through
`--seed-fd`: an inherited read-only nonblocking pipe containing exactly 64
lowercase hexadecimal bytes followed by EOF. It emits one canonical JSON
receipt, identical to `genesis-anchors.json`, binding original genesis, context,
network identities and a complete SHA-256/length census of launch inputs.
Four `peerN-client.toml` files carry the inline network identity and each peer's
endpoint. Originals remain retained through bounded receipt publication; the
caller pins the receipt and admits runtime dependencies before launch. The
fixed chain label is limited to 1,024 UTF-8 bytes.

The producer emits its immutable census and `storage/peerN/{kura,state}`;
runtime state is separate from canonical Kura data. It emits no generic scripts,
README, codec copy, key sidecars or runtime service bundle. The caller retains
the exact initially empty role directories before launch.

Each anchor peer row carries `primary_block_store` and `primary_merge_log`,
derived with the effective primary lane's native `blocks_dir` and
`merge_log_path` helpers under that peer's original Kura root. The daemon
creates those paths. Stopped-tip observation, vector collection, facts and proof
export consume these native projections; the Kura runtime root itself is not
the block-journal directory. The receipt also carries `genesis_public_key` and
the effective u16 `chain_discriminant`, matched across all four final configs.
Callers retain these values directly rather than infer omitted TOML defaults.

Facts assembly resolves every original signed workload request against the live
staged genesis world for each of the four authenticated peer configurations.
Account metadata instructions require this state-aware routing. Each retained
genesis authority owns an ordered projection of the original signed-byte hash
and its exact single coordinator route; all four projections must agree with
the complete journal. The caller charges the additional shared routing decode
and four compact projection vectors against its cumulative allocation budget
before staging. Genesis staging has separate bounded worker stacks and does
not claim a process-wide heap limit. Canonical committed-effect verification
still checks every request against the complete stopped durable transcript.

## Original execution and canonical application evidence

`FixedExperimentCustody` owns the original output roots, inputs and ten trial
slots. Only a slot-created `FixedTrial` can transfer its completed native
operations and original capture descriptors to the experiment. Each run retains
the exact signed workload journal, readiness evidence, Applied observations,
proof inputs and process identities. Private account configurations and signed
request bodies remain outside the public projections.

The stopped durable transcript and canonical native proof join every original
request to its exact global/local application. Submission success or an
unauthenticated report cannot replace that join. The complete originating owner
publishes the run summaries and manifest. `ResourceExperiment.from_completed`
then borrows that owner, independently replays all ten runs and returns each
result to the corresponding original native authority before report publication.
The [experiment ownership contract](../docs/scaling_fixed_experiment.md) details
publication, final verification and cleanup.

## Measurements and limits

The workload uses a fixed offer schedule, warmup, measurement window and bounded
drain. A request carries its exact scheduled and actual offer times and complete
application identity. Missed work, replay mismatches and incomplete application
cannot be repaired by editing aggregate counters or omitting slow requests.

`scaling_measurements` derives exact rational throughput and integer latency
from complete public projections. Every run includes at least 100 measurement
requests. Warmup requests do not enter either performance comparison.

- Four-lane median committed throughput must be at least `3/2` of the one-lane
  median, using five results for each variant.
- Four-lane pooled p95 latency must be at most `5/4` of the one-lane pooled p95.
  Both pools include complete accepted measurement cohorts and their drain tail.
- Observed queue, index, aggregate peer RSS and declared storage-root maxima
  must satisfy every admitted resource limit.

The report records the three measurement criteria separately. Sampled resource
observations do not establish continuous whole-host quota enforcement. Declared
hardware/build labels require the independently observed and authenticated
release runtime identities.

## Parent verification and release receipt

The protected bootstrap owns the scaling operation and runs its required
preflight before collector launch. Its original process owner retains the
launch inputs, command result and deadline. The parent independently inspects
the public archive and performs ten bounded native proof replays, then publishes
`scaling-execution.json` with schema
`iroha.sumeragi_v2.multilane_scaling.parent_execution.v1`.

The record binds the selected source, complete plan and budget, original command
observation, ordered native replay results, public archive inventory, verifier
Python package and preflight archive. It carries data rather than a standalone
execution-authority flag. The bounded handoff connects the original bootstrap
parent and release runner; the receipt consumes `--scaling-execution-record`
and `--expected-scaling-execution-sha256` from that operation.

Detached replay also requires an externally authenticated final parent marker.
A record and a digest supplied beside it cannot authenticate their own producer.
The receipt validates all bound archives and the exact source relationship.
Human-readable output and a report criterion alone cannot close `G-SCALE`.

## Outstanding qualification

Current source implements the fixed collector, parent handoff, archive checks
and canonical record consumer. The remaining integration work includes migrating
all receipt/approval fixtures and declarations to those interfaces and retiring
unused tooling after its required assertions have moved. Do not add compatibility
constructors, alternate input schemas or a second report-acceptance path.

Fresh native runs must still demonstrate all ten trials, authenticated replay,
resource bounds and both performance thresholds on pinned hardware. Unit fixtures
with synthetic terminal records or explicit process seams establish their tested
parser/ownership properties only. The complete source-bound preflight, SDK and
workspace gates remain separate requirements. No passing benchmark or release
receipt is asserted by this specification.
