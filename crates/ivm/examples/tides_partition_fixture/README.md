# Two logical partitions: bounded IVM fixture

This is a small executable vertical slice, not the complete Tides game. All
transition arithmetic is compiled from `contract.ko`; Rust supplies an independent
integer oracle and checks every one of the 33 scalar fields after calls.

One deployed coordinator owns two disjoint area namespaces, `a_*` and `b_*`.
Separate scheduled `advance_a` / `advance_b` calls perform one two-logical-second
step: worker loading/delivery, limited deposit and storage, upkeep, movement,
simultaneous unit/guard damage, base damage and a continuous 60-second occupation.
Capture moves 20% of the fixture's collateral counters once. Those counters and
the 128-unit work budget are **test accounting**, not native XOR asset custody or
network gas payments. Each successful step consumes one budget unit. Empty budget
causes a no-op; the network fixture must install finite trigger repetitions.

`transfer_a_to_b` checks and changes both namespaces in one IVM invocation. It
moves the single surviving mobile unit and its HP, and stores a consumed nonce.
There is no separately callable import that can mint a duplicate. The deliberate
`reject_transfer_a_to_b` fails after source debit to exercise native transactional
rollback. Neither entrypoint is an independent data-space AMX protocol.

The fixture uses the existing `CanEnactGovernance` permission solely to keep
mutating test entrypoints from arbitrary accounts. This does not define production
player admission, cadence authentication, base ownership or permission policy.
There are no proof submissions or off-chain authoritative results.

The IVM example uses the IVM crate's state shim for fast oracle tests and named
microbenchmarks. It does not establish production CoreHost behavior, durable
storage, consensus, native clock liveness or restart safety. Those require the
four-validator network integration using the same compiled artifact.

From the Iroha workspace:

```
cargo test -p ivm --example tides_partition_fixture
cargo run -p ivm --example tides_partition_fixture -- /tmp/tides_fixture.to
cargo run -p ivm --example tides_partition_fixture
```

The default command prints measured gas/cycles/wall time for the actual bounded
bytecode. Wall time includes VM construction and artifact loading. Oracle checks
are outside the measured span. Every call is one dense two-logical-second step;
the peaceful result is not a multi-tick Sparse boundary. Mixed categories span
approach, combat, occupation and sometimes already-quiet state. A separate
damage-step category includes only calls that actually reduce unit or structure
HP; those observations also appear in the mixed categories and are a small
sample, not sustained-combat throughput. The two gameplay areas are logical state
partitions in one contract and one native State, not parallel validator workers
or independently signed roots. No throughput extrapolation is valid from this
fixture alone.

The `hazard_256_entity_component` report is a separate existing-ISA IVM heap
kernel. It measures four 256-entity / eight-owner scenes (exposed, sheltered,
weakened and a linked-episode ceiling) over 30 warm runs, including a separately
reported memory-root cost. Its independent oracle checks all 3,624 words,
including unchanged XOR and ownership sentinels. Candidate damage uses 10%
exposed / 2% sheltered maximum HP, material multipliers 3/2, 1, 1/2 and 3/4,
a cumulative 30% maximum-HP ceiling across the episode, per-owner destruction
ceilings of 20% of living units and 10% of living non-Hall buildings at episode
start, and a Hall floor of ceil(25% maximum HP) without healing. Dead units lose
cargo once. The 192 units span eight owners, and 56 ordinary buildings span four
owners so a weakened scene exercises four actual building deaths (rather than
eight zero death quotas). Bounded cargo counters start at four per unit and two
per ordinary building; they are not the complete storage economy. The weakened
scene independently asserts 32 unit deaths, four building deaths and 136 cargo
destroyed, then checks all 3,624 VM words against the oracle on every run.
Canonical entity-ID order breaks quota ties; this differs from the Python
reference's hash-of-episode-and-ID ordering. Event intensity is fixed to 10,000
basis points. No full Python-transition parity is claimed. Reapplying the same event
nonce does nothing. These are candidates, not approved game hazard rules.
Input-domain validation is outside the measured kernel and must be bound by a
production adapter/relation. Geographic exposure, beacon access, durable state
I/O, pending-event handling, country grace, funding, policies, scheduling and
cryptographic proving are not included. The damage core alone cannot establish
full native hazard-settlement gas, admission or cross-area storm throughput.
