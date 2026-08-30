# TON SCCP contract security invariants

The TON SCCP contracts use one first-release storage and message layout. A
canonical bridge and Jetton master deployment starts with zero supply, empty
replay and pending dictionaries, zero pending counters, and
`mintingDisabled = false`. The shared immutable configuration must contain a
positive `maxWrappedSupply` and five nonzero, distinct Ed25519 breaker keys in
strict numeric order.

Every wallet, master, and bridge inbound or bounced message installs an exact
TON reserve before any outbound action:

`max(contract.getOriginalBalance() - inbound_value, protocol_minimum_reserve)`

The subtraction is checked. `RESERVE_MODE_AT_MOST` is not used. Mode 64 is
limited to one terminal send in a message branch after the reserve has been
installed. The recipient-wallet transfer notification path is the exception
that needs two sends: its excess return is an ordinary explicit-value send,
bounded by the current inbound value after the notification value and one
operation allowance. It cannot spend a pre-existing balance or an operator
top-up.

Bridge mint/burn, master mint, wallet authorized-burn, and wallet transient-lock
dictionaries reject a new entry at 1,024 records per typed boundary. There is
no custody-wallet deposit or shared refund dictionary. Each canonical owner
wallet owns its fixed outbound nonce and its own bounded lock map. Retries only
read the original semantic record and redispatch its stored fields; witnesses
are replaceable delivery material and are never part of that record.

Outbound burns are two phase. An owner-authenticated request validates the
exact 36-byte single-Ed25519 Taira identity before 0x36 debits and creates a
transient lock. The canonical bridge authenticates the deterministically
derived source wallet before 0x31, then authorizes the irreversible 0x35 wallet
transition and canonical-wallet 0x33 master burn. Rejection before 0x31 marks
the lock refundable; only a fresh valid 0x37 witness can recredit it. Once 0x31
exists, no bounce or refund path can undo the debit.

The master enforces `totalSupply + mint_amount <= maxWrappedSupply` with a
checked subtraction. The bridge rejects a single admission above the same
immutable cap. Both expose the cap and breaker state for authenticated
deployment readback. The governed Jetton master address is not a valid mint
recipient: both SORA admission and the TON bridge reject it before custody,
replay, pending-mint, or supply state changes, and the master repeats the check
before its own replay boundary.

Every replay boundary owns one SHA-256 256-shard, depth-248 sparse forest and
mutates only its operation tag. A witness header is exactly 769 bits with zero
or one reference. Its high eight bitmap bits are zero; a zero bitmap has no
sibling cell, while a nonzero bitmap has exactly `ceil(popcount / 3)` cells,
three nondefault hashes per nonterminal cell and one to three in the final
cell. No trailing bits, references, default siblings, or alternate snake
chunking are accepted. Payload and auxiliary commitments each add exactly one
canonical SHA-256 layer.

The bridge commits the exact immutable configuration cell and derives the one
canonical master StateInit from its embedded code and known zero initial state.
The master commits the exact bridge address. Operational messages authenticate
these identities before replay or pending-state mutation. Replay domains use
the final-V1 one-byte network profiles (`SORA = 0x40`, `TON = 0x44`); transfer
domain identifiers remain separate.

`SccpDisableMinting` is a one-way, target-bound 3-of-5 action. Guardians sign a
cell binding the disable domain, target contract address, TON global id, route
revision, and route-configuration hash. Signature slots are fixed, signatures
must be exact 512-bit cells without references, and every supplied signature
must verify. Bridge and master authorizations are not interchangeable. There
is no re-enable, reconfiguration, withdrawal, or burn-blocking action. A
disabled contract rejects only a new local mint admission; an already occupied
local pending mint can still be redispatched and acknowledged.

Regenerate checked-in wrappers with the pinned Acton 1.1.0 toolchain whenever
the contract ABI changes. Production artifacts must be built with the
repository's digest-pinned Linux/amd64 corridor; a developer-local Acton binary
does not produce release evidence.

The cross-language StateInit boundary is pinned by
`fixtures/sccp/ton_stateinit_golden_v1.json`. The values are printed by
`scripts/generate-stateinit-golden.tolk` after invoking the same canonical
storage constructors and compiled-code cells used by deployment. The host
wrapper authenticates the exact Acton archive, requires embedded Tolk 1.4.1,
records the full source closure, and rejects any checked-in byte drift. Both
roles expose their code and initial-data cell depths directly from Tolk so a
consumer can reproduce StateInit hashing without inferring hidden child depth.
