# TON SCCP v1 contract security invariants

The current native BLS destination consists of `SccpTairaXorMinter`,
`SccpTairaXorWallet` and `SccpConsumedBucket`. Its contract is
[`specs/sccp.md`](../../../specs/sccp.md) §5.1 and §5.3. The executable checks
live in `tests/unit`; a passing component suite does not establish whole-network
or settlement readiness.

## Authority and finality

The minter has no owner, admin, upgrade path or authority setter. Its canonical
initial data binds the Taira NetworkId, consensus instance, route revision,
supply cap, initial native committee pin and wallet/bucket code. Initialization
checks the TON network, the complete canonical initial state, a supply cap below
`2^96` (the bound `RegisterRoute` applies), sorted native BLS keys, committee
geometry/root and validity horizon.

Every new certificate authenticates the complete 221-byte finality header `X`
and the original result-body digest through the native Commit preimage. The
bitmap carries exactly `n - f` signatures for a `3f + 1` committee. Curve,
subgroup, signature and canonical encoding checks belong to the native
`BLS_FASTAGGREGATEVERIFY` path. An offered key list cannot replace the stored current root.

The three retained checkpoint slots bind exact header ids and heights. A cached
header can use the established checkpoint relation only; a changed header at an
already retained height cannot inherit its authority. Conflicting authenticated
certificates latch equivocation. Inline use that would require a latch reverts;
`sccp_checkpoint` records it and emits `sccp_latched`. Rotations advance native generation state
under the current certificate and validate successor keys when the root changes.
The committee deadline uses the specified destination progress bound.

## Controls and proof binding

`sccp_apply_control` proves the complete 199-byte control leaf under a native certificate.
It binds the Taira network, destination lane/address/revision, control nonce,
pause tracks and effective expiry. `control_nonce` strictly increases. Parliament pause
and the bounded fast pause are independent; only their effective combination
blocks finalization. Checkpoints, voids, burns and further authenticated controls
remain governed by their own checks.

The native control suite replays every TON destination-effect scenario from
`fixtures/sccp/control_v1.json`. Independent Rust and Python generators produce
identical certificates and expected outcomes; the contract replay compares events,
pause state, checkpoint state, supply and consumed nonces at each step.

Direct and historical message proofs bind the exact block root and message
count; historical mode additionally proves the block under the certified history
root. Transfer decoding checks destination, revision, recipient and amount.
Deadline, supply-cap and consumed-nonce checks occur before minting.

## Canonical cells

`SnakeBytes`, hash paths and native key chunks have one canonical shape, including
exact bits, references, maximal chunks and complete consumption. Every chunk must
be ordinary; library and other exotic cells cannot install externally mutable committee
keys or substitute a different proof graph. TON integer fields retain their full
protocol width; wallet quotes and nonces must not be narrowed to signed 64 bits.

## Replay and supply accounting

Nonce `k` uses bucket `k >> 9` and bit `k & 511`. Buckets and wallets accept
dispatch, confirmation and deletion only from their minter (`NotMinter`), before
they read a receipt. Canonical deployment starts inactive, and an
already activated index is never activated again. Recreating a deleted bucket
therefore cannot clear the authority required to replay a mint or void.

Finalization records its amount as pending supply and reserves one of 512
prepaid operation slots before dispatch; activation batches reserve every slot
before the first send. The supply cap binds `total_supply` when the bucket
applies the consume, the only point where supply grows. A mint the cap cannot
take there is unconsumed, as a mint in flight across a latch is. The early
refusal at admission counts completed supply alone. A duplicate finalize, which
the bucket rejects, therefore holds a slot for one round trip but never cap
headroom, and cannot refuse or delay an honest mint.
Operation ids never repeat. Each destination atomically retains a bounded
receipt with its effect. The minter advances through authenticated Prepared,
Drop and Dropped messages, binding the canonical sender, operation, stage and
attempt. It frees a slot only after receipt deletion is confirmed. Lost ACKs
keep custody; retries query or delete the receipt, and replay an original effect
only after its authenticated bounce proved failure. Missing receipts fail
closed because a possibly successful wallet could have been deleted and
recreated. Stale ACKs or deletion requests cannot affect a newer receipt.

Mint failure burns only the unminted total supply and changes the existing row
to retained unconsume. Original-bounce handlers allocate nothing. After the
custody update they return the bounce's value above the minter's prior balance
to the operation's original response. That send ignores errors, so custody never
depends on it. A concurrent retry cannot change the original response address:
each in-flight exchange carries its own response and query id. An ACK that
advances nothing (a completed row, another stage or attempt, or a row not
awaiting deletion) returns its value to its own response. A retry that races its
original therefore loses only its fees, unless a destination bounces its
message: that value goes to the operation's original response. A destination
keeps the residue of a bounced ACK, because the 256-bit bounce body cannot carry
the response; a genuine ACK bounces only when its value cannot pay the minter's
next action. TON's
[ordering rules](https://ton.org/tblkch.pdf) (§§2.2.9–2.2.11) and
[validator ordering check](https://github.com/ton-blockchain/ton/blob/master/validator/impl/validate-query.cpp)
apply to the tested IHR-disabled headers. Safety does not infer failure from an
absent receipt, and activation chains explicitly wait for completion.

Burns require the canonical SCCP burn payload. Before debiting or forwarding,
the wallet validates the ordinary wrapper's exact opcode and single reference,
then the entire ordinary recipient snake: maximal 127-byte chunks, no extra
bits or references, and 1..=1024 bytes in total. The minter repeats these checks
and accepts notifications only from the owner's canonical wallet. A rejected
burn restores the wallet balance. Frozen-range voids consume their bounded range
atomically without minting.

## Value and validation

The fee model computes verification, callback, forwarding and storage allowances
from the current TON price configuration before starting work. Minter entry
operations fund the step and any floor deficit; burns and bucket replies retain
the prior balance without charging that deficit. `raw_reserve` mode 2 retains
the minter's funded `MINTER_FLOOR` and prior balance within the admitted step budget;
buckets reserve the greater of `BUCKET_FLOOR` and their prior balance. Quotes
include seven days of storage grace. Maximum custody is prepaid for 100 years
at current prices: minter 2,380 cells/790,000 bits, bucket 1,080 cells/83,000 bits,
wallet 1,080 cells/86,000 bits. All three graphs include the full 512-entry
capacity; native tests enforce both upper bounds and at most 5% headroom.
Capacity is checked before effects, with no eviction or unbounded retry map.
A mid-flight price change can require a newly funded retry, preserving the
operation and receipt across failed deliveries.
Current measured gas belongs in `gas-baseline.json` and must be regenerated with
the pinned Acton toolchain after contract changes.

The emulator suite covers native BLS verification and rotation, checkpoint
retention/latching, controls, deadline/cap boundaries (including duplicates in
flight at the cap and a consume the cap undoes), canonical cells, replay,
stranger dispatch/confirmation/deletion at both destinations, bounces, value
returned by racing retries and bounces, retry accounting, burns and required
value. The Rust pure verification and wallet suites exercise the same
certificate/proof relations and exact contract encodings.

Generated fixture tables replay every commitment-tree and history root and
path, commitment-tree confusion negative, and applicable payload refusal. The
burn replay compares the minter's emitted external-message body with
`native_transfer_event_v1.json`. BLS wrapper tests consume signature and
fast-aggregate verification rows, standard inputs and Rust verification
negatives, with explicit TON outcomes. Key generation, signing,
distinct-message aggregation, w3f PoP generation and Rust-only allowlist APIs
remain outside the destination verifier. Pinned row counts and generator
checks prevent fixture regeneration from silently reducing replay coverage.

The pinned toolchain is Acton 1.2.0 with Tolk 1.4.2.
`fixtures/sccp/ton_stateinit_v1.json` records the compiled code and native initial
state/address golden values.
