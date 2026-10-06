# iroha_kagemusha_proof

Native KAGEMUSHA step relations on PIPA-R (`iroha_plonk`). The crate depends
on `iroha_pasta`, `iroha_plonk`, `iroha_plonk_gadgets` and
`iroha_plonk_recursion`; it does not depend
on the legacy proving stack or node execution.

The σ API fixes the base-field RP57 transcript, Direct instances, the folded
IPA generator suffix, and one `Bounded` public-input column. Keys use the
canonical V2 descriptor; the reader rejects V1 descriptors and other transcript
or instance profiles. The `kgwvkey1` base-field Poseidon verifying-key digest
binds the descriptor digest, transcript representation, counts and ordered
commitment coordinates. The shared engine owns this digest definition.

## Protocol layout

The implementation follows G1 revision 4 and owner decisions B1, B5–B8 in
`specs/kagemusha_wallet_wire_v1.md`. Shared vectors in
`fixtures/kagemusha/wallet_v1_vectors.json` pin each field and digest.

- The core has 33 fields and the rest has 8. The head is
  `P(kgwcore1, core || P(kgwrest1, rest))`; σ carries the rest digest.
- Poseidon digests are single canonical field elements. SHA-256 identifiers
  and byte nonces remain two little-endian `u128` limbs. Noncanonical
  Poseidon bytes are rejected before synthesis or native consumer acceptance.
- `credit_id = P(kgwcrdt1, Request body)` binds all 26 Request fields,
  including both account digests and the receiver's recorded blacklist
  version and root. The public input is the 26-field statement digest under
  `kgwstmt1`; its effect union has 9 fields.
- Send chains have 8 fields: prior chain, credit, receiver wallet limbs,
  ordinal, amount, fee, Request digest. Receive chains have 5 fields: prior
  chain, credit, payer wallet limbs, amount.

Both steps open an Active or Retiring predecessor, require distinct payer
and receiver wallets and a nonzero amount, advance sequence without
overflow, append a chain and commit the successor. Send debits `amount +
fee` from balance while preserving the lineage's burned-value restriction,
advances its ordinal, checks policy epoch and monotonic accepted time, and
raises the time floor. Receive credits without overflow and matches an
issued Request by wallet identity, so credential renewal does not invalidate
it. Pending, fee and consumed-credit transitions belong to native Advance
and Λ; σ binds their carried successor roots.

## Recursive sigma leaf

`q_sigma` checks own σ hard and optional incoming σ soft on one shared
verifier lane. It binds the complete witness-key digest to a circuit-fixed
allowlist and exports the same LE32-length-prefixed proof bytes in 31-byte
chunks (107 for k12 and 112 for k14). The parent A relation recomputes the
statement digest and checks the operation/mask selector and global mode rule.
One σ forwards a checked source-k claim; two σ use a hard local PIPA-AS fold
with the selected incoming claim and an explicit pinned k16 trivial input.
Public columns have explicit homogeneous PIPA-R types. These component
constraints remain separate from final recursive artifact qualification.

## Controls

Send's verifying key is selected by the opened core mask. Receive's key is
selected by the Request's recorded version: `(4, 0)` for zero, `(4, 1)` for
nonzero. Its current core mask is still restricted to defined bits and is
carried in the statement, but does not select Receive enforcement.

- **Blacklist:** a depth-16 gap opening in limb order proves the
  counterparty absent. Send uses the payer's current committed list and
  enforces its maximum age. Receive uses the receiver's list recorded in
  the Request, regardless of later list or mask changes. Version zero is
  valid exactly with root zero.
- **Quotas:** the depth-6 window tree and the aligned depth-6 usage array
  have exactly 64 slots. A usage leaf is
  `P(kgwquse1, [kind, start, end, used])`; usage nodes use `kgwqusn1`.
  Each touched window is charged once at its own slot, within its limit,
  against the running usage root. Four consecutive window openings per
  kind establish that the two candidates include every touched window.
  Padding slots retain zero usage. There is no quota indexed map or quota
  insertion path.
- **Quota time:** σ requires `upper < quota_share_expires_at_ms` and
  `upper - lower <= time_anchor_max_response_ms`, authenticated core fields.
  Native installation/Λ require windows longer than the span bound; hence
  at most two windows of each kind can be touched.
- **Lease:** Send requires `upper < lease_expires_at_ms` when enabled.

`StepWitness::evaluate` reports native violations and every derived digest.
The circuit compares its digests with that reference during synthesis with
known witnesses. `check_send` and `check_receive` compare a statement with
the Request and, for Send, Ω's public lineage view before returning the
verifying-key selector and public digest.

## Shapes and proof bytes

Folded-prefix shapes, fewest lanes that fit:

| Relation | Permutations | Smallest shape | One-lane budget shape |
| --- | ---: | --- | --- |
| Send, no controls or lease | 69 | k10 / 4 lanes, 5,120 B | k12, 3,296 B |
| Receive, no recorded list | 67 | k10 / 4 lanes, 5,120 B | k12, 3,296 B |
| Send, blacklist | 106 | k11 / 2 lanes, 3,840 B | k12, 3,296 B |
| Receive, recorded list | 104 | k11 / 2 lanes, 3,840 B | k12, 3,296 B |
| Send, quotas | 309 | k12 / 4 lanes, 5,280 B | k14, 3,456 B |
| Send, all controls | 345 | k12 / 4 lanes, 5,280 B | k14, 3,456 B |

The k14 one-lane quota shapes occupy 12,123 and 13,542 rows respectively.
Their 3,456-byte proofs fit the current 3,541-byte σ share of the joint
Payment budget. These are exact descriptor lengths; performance and memory
qualification are separate gates. `select_shape` synthesizes candidates and
can impose a byte budget. Keys, descriptors and proof bytes are independent
of the Rayon pool size and optional commitment tables.

`SigmaProver` generates keys and rejects invalid witnesses. `SigmaVerifier`
rebuilds from descriptor/key bytes and pinned parameters. `SigmaAllowlist`
selects by `(operation tag, mask)`. Freezing the production artifact set and
integrating Λ/Ω with wallet and node paths remain separate G3–G5 work; this
crate's σ implementation alone does not establish complete protocol readiness.

## Validation

- `digest_parity`: every G1 field encoding, named controlled-state positions,
  hashes, packing domains, fixed64 usage roots/openings and in-circuit parity
  on both fields; deterministic known answers.
- `controls`: blacklist snapshot enforcement, zero version/root agreement,
  exact expiry/span boundaries, repeated/misaligned/out-of-range slots,
  forged usage roots and noncanonical Poseidon bytes.
- `relation_checks` and `forgeries`: arithmetic boundaries, every step rule,
  wrong-head and consistent-forgery attacks, and release per-cell tampering.
- `shapes`: exact shapes, row counts, joint byte budget and verifier rebuilds.
- `real_proofs`: real valid and rejected proofs, selector binding, byte
  tampering and deterministic keys/proofs; quota shapes use k14.
- `measure`: ignored diagnostic throughput/footprint workloads. They are not
  the fresh-process qualification procedure in the design record.

```sh
cargo test -p iroha_kagemusha_proof
cargo test --release -p iroha_kagemusha_proof --test real_proofs -- --include-ignored
cargo clippy -p iroha_kagemusha_proof --all-targets -- -D warnings
```
