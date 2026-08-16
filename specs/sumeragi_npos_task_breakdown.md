## Sumeragi revision-4 NPoS validation map

Revision 4 is the first-release production protocol. The former Phase A list
described V1 collectors, global RBC storage, adaptive pacemaker settings, and
local debug fault tables; those surfaces and their integration scenarios are
retired rather than carried into the release inventory.

### Current executable coverage

- `integration_tests/tests/sumeragi_v2_runner.rs` exercises signed revision-4
  genesis, four-validator restart finality, leader timeout/TC rotation, and
  authenticated controlled release of same- and distinct-subject QC evidence.
- `integration_tests/tests/sumeragi_npos_liveness.rs` verifies that a
  four-validator NPoS network produces and commits blocks without local timing
  or DA overrides.
- `integration_tests/tests/sumeragi_npos_performance.rs` retains the 1-second
  baseline and transaction-queue saturation/backpressure scenarios. These use
  signed genesis parameters plus current finite queue configuration.
- `integration_tests/tests/sumeragi_localnet_smoke.rs` retains permissioned and
  NPoS load/throughput coverage. Its long-running rotating scenario now stops
  one validator process at a time, restarts it with the unmodified base
  configuration, and verifies catch-up and convergence.
- Torii/CLI endpoint suites continue to check authenticated revision-4 status
  and telemetry serialization independently of the retired V1 RBC soak.

### Retired scenario classes

- local RBC chunk corruption, duplicate-init, conflicting-ready, selective-drop,
  and forced-delivery tests driven by debug configuration;
- global RBC store-pressure and chunk-loss performance tests;
- EMA/jitter/local-timeout pacemaker tests and the downtime test that attempted
  to mutate removed timing fields;
- the adversarial-collector telemetry soak tied to V1 counters.

Equivalent safety properties are covered at the authenticated revision-4
message boundary. Availability and recovery properties use the signed DA
layout, real validator outages, or block/body recovery. No test configures a
lower quorum, disables mandatory DA, or selects a local protocol version.

### Remaining validation

Runtime validation still requires the canonical Cargo suites and ignored
multi-peer soaks on representative binaries. Static source checks can prove
that retired config keys are gone and inventories are aligned, but cannot prove
wall-clock liveness, process recovery, or telemetry population under load.
