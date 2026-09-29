# TON SCCP v1 contract security invariants

Scope: `SccpTairaXorMinter`, `SccpTairaXorWallet` and `SccpConsumedBucket`
(`specs/sccp.md` revision 4, §5.1 and §5.3). Build, test and golden tooling is
described in
[`docs/source/sccp_ton_release_builder.md`](../../../docs/source/sccp_ton_release_builder.md).
The Acton emulator suite in `tests/unit` exercises these invariants; the
TON network rules they rely on are named where they matter.

## Authority

- There is no owner, admin, upgrade path, code change or setter. Minter state
  changes only through messages whose authority is verified from attested
  Taira state, plus the counters those messages drive.
- `minting_paused` changes only through `sccp_apply_control` /
  `sccp_apply_control_historical` (ops `0x53434d31` / `0x53434d32`). The
  message proves a Parliament control leaf (§3.4) in a Taira attestation
  signed by at least `t` members of an accepted roster. The leaf binds
  `lane_bytes(sora-taira, ton-mainnet)` with the stored `taira_network_id`,
  destination word = the minter's own account id, the minter's
  `route_revision`, `control_nonce` and `paused`. A control leaf for another
  network, destination, revision or Taira network, and a transfer leaf
  offered as a control, fail the inclusion proof.
- `control_nonce` strictly increases: stale and equal nonces are rejected,
  gaps are allowed. An applied control sets `minting_paused` and
  `control_nonce`, increments `op_count` and emits `sccp_control_applied`.
- There are no guardian keys, no roster-signed pause and no one-way breaker.
  The pause blocks finalize only; voids, rotations, burns and further
  controls stay open while paused.

## Initialization

- The minter address is the hash of its canonical initial data (§5.3.1):
  zero counters and supplies, `minting_paused = false`, no previous roster,
  and a roster equal to Taira's pinned generation record in maximal 6-address
  chunks. Taira recomputes this address at `RegisterRoute`.
- `sccp_init` is permissionless and runs once. It requires `GLOBALID = −239`
  and canonical initial data, recomputes the §3.7 digest from the stored
  roster fields and `taira_network_id` (checking `n`, `t` and member order),
  requires it to equal both `roster.digest` and `config.initial_digest`,
  requires `generation = initial_generation`, and applies the §5.1.5 validity
  bounds. Every state-changing operation other than `top_up` throws until
  then; TEP-89 `provide_wallet_address` answers regardless.

## Rosters and signatures

- An attestation is accepted from the current roster while
  `now ≤ valid_until_ms`, or from the previous roster within its grace-capped
  validity (`min(valid_until_ms, rotation time + 24 h)`). Members always come
  from storage, never from the message.
- A signature set is a `uint32` bitmap with bits `≥ n` zero, one 521-bit cell
  per set bit in ascending order and nothing else. Each signature needs a
  nonzero member, `1 ≤ r < N`, `1 ≤ s ≤ N/2` and `v ∈ {27, 28}`; `ECRECOVER`
  receives `v − 27`, and the recovered address (low 160 bits of
  `keccak256(x ‖ y)`) must equal the member. At least `t` must verify.
- A rotation is signed by the current roster only, carries the announced next
  digest, advances the generation by exactly one, sets `valid_from_ms` to the
  attestation timestamp and satisfies the §5.1.5 bounds. The old roster moves
  to `prev_roster` with its grace-capped expiry.

## Canonical cells

- `SnakeBytes`, `HashChunk` and `MemberChunk` parsers accept exactly one shape
  (§5.3.2): maximal chunks, no empty chunk, no extra bits or references.
- Every chunk must be an ordinary cell; exotic cells throw `BadSnake`. This
  matters for library references in particular: `CTOS` resolves them
  transparently, so without the check a relayer could install a roster whose
  member list points to a public masterchain library with canonical content,
  and unpublishing that library later would make every roster read throw and
  freeze the minter. `sccp_init` and `sccp_rotate` therefore store only member
  lists that were fully parsed as ordinary canonical cells.

## Replay protection

- Nonce `k` lives in bucket `k >> 9`, flag `k & 511`. A bucket sets or clears
  flags only for messages from its minter.
- The minter attaches a StateInit only for bucket indices
  `≥ deployed_buckets`, at most 4 ahead, and advances `deployed_buckets` in
  the same transaction. A bucket is therefore never redeployed and its flags
  can never be reset. A bucket deleted for unpaid rent (excluded in practice
  by `BUCKET_FLOOR`) keeps its nonces unfinalizable and unvoidable until
  someone unfreezes it with its exact state.
- The minter accepts `sccp_consumed`, `sccp_already_consumed`,
  `sccp_range_consumed` and `sccp_range_rejected` only from the bucket address
  recomputed from `bucket_code`, the nonce and its own address.
- A new flag is answered with a bounceable `sccp_consumed`; if the minter's
  handler fails, the bounce makes the bucket clear the flag. A set flag is
  answered with a non-bounceable `sccp_already_consumed`. Ranges
  (`void_frozen`, one bucket, `count ≤ 512`) are set all-or-nothing and a
  bounced `sccp_range_consumed` clears them.

## Supply accounting

- Finalize checks `total_supply + pending_supply + amount ≤ max_supply` and
  reserves `pending_supply` before contacting the bucket. On `sccp_consumed`
  the pending amount moves to `total_supply` and a bounceable
  `internal_transfer` (`query_id = nonce`) mints to the recipient's wallet.
- A bounced `sccp_consume` and an `sccp_already_consumed` release the pending
  amount. A bounced `internal_transfer` burns the unminted supply and sends
  `sccp_unconsume{nonce}`, so the message becomes finalizable again. No nonce
  stays consumed without a mint or a void. This relies on the TON rule that
  only the network sets the `bounced` flag, as the TEP-74 reference contracts
  do.
- Inbound payloads must target this deployment: route revision, route id,
  `amount < 2^96`, and a `ton_account36` recipient in workchain 0 that is
  nonzero and not the minter itself.
- A burn requires the `sccp_burn_to_taira` custom payload; plain TEP-74 burns
  are rejected by the wallet. The minter requires the sender to be the
  canonical wallet of the owner, the owner to be `addr_std` in workchain 0
  without anycast, and a canonical 1..=1024-byte recipient. Any failure
  bounces `burn_notification`, and the wallet restores the balance.

## Value and storage rent

- `finalize_required_value`, `void_required_value` and the other required
  values are computed at entry from `GETGASFEE`, `GETFORWARDFEE` and
  `GETSTORAGEFEE`; there is no fixed TON amount. The per-step gas limits are
  measured maxima with about 20% headroom, and `tests/unit/sccp-fees.test.tolk`
  fails when a measured step exceeds its limit.
- Before forwarding value, the minter and every bucket reserve
  `max(floor, balance before the message)` with `raw_reserve` mode 2, where
  the floors are 100 years of storage (`MINTER_FLOOR`, `BUCKET_FLOOR`).
  Top-ups and prior balance are never forwarded to a later caller; a caller
  pays the minter's floor deficit when the balance is below it. The minter
  sends without a reserve only where the incoming value pays: the TEP-89
  answer (mode 64), the `sccp_voided` event of a range reply (covered by the
  value attached to the range request) and the `sccp_unconsume` after a
  bounced mint (covered by the bounced value).

## Chain identity and artifacts

- `GLOBALID = −239` is required by `sccp_init`, `sccp_finalize*`,
  `sccp_void_expired*` and `sccp_apply_control*`.
- Contracts are built natively with Acton 1.2.0 bundling Tolk 1.4.2, pinned by
  archive SHA-256 in `scripts/ton_sccp_builder.py`; no Docker or container
  runtime is involved. `fixtures/sccp/ton_stateinit_v1.json` pins the code
  cell hashes and depths, the canonical initial data and the resulting
  workchain-0 addresses. `scripts/generate_ton_sccp_stateinit_golden.py
  --check` recomputes it in Python and through the contracts' own Tolk types
  (`scripts/stateinit-golden.tolk`) and rejects any drift.
