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

## Bootstrap composition

`admin_sigma::BootstrapCircuit` proves the exact zero-value initial G1 state,
empty maps/counters and statement binding at k12 with five advice columns;
its PIPA-R proof is 3,296 bytes. A authenticates the state credential and the
same-byte certificate/credential/receipt transcripts through the separate
2-variable/1-fixed signature Q. `a_relation::bootstrap` pins scheme, provider
and root policy and requires the credential, state/key and receipt bindings.

The actual sigma → Q_sigma → A1 → W → A2 test fits k16 and independently
decides every retained claim. Its context and every deferred opening are
retained; signature-role/root substitutions and dropped obligations fail.
See `specs/kagemusha_a_split_context_v1.md`. This is operation composition,
not final Omega/artifact, hardware, time/RSS or wallet acceptance qualification.

## Load components

`admin_sigma::LoadCircuit` proves both state openings, exact arithmetic,
continuity and unchanged fields on the five-column k12 class (3,296 bytes).
`a_relation::load` binds five original tapes: LoadAuthorization certificate,
finalized voucher, own receipt, Enrollment certificate and current credential.
The mandatory signature workload is three variable and two fixed signatures,
split across two hard Q leaves. C4 re-verifies the current credential on every
step; a predecessor proof does not replace that obligation. The fixed schedule
now has separate predecessor/recovery, Q_sigma, voucher/receipt and current
credential stages. All four current component stages prove at k16
(maximum 63,085 rows, 8,480-byte A proofs), including the signature-Q bundle,
full consuming-transcript and dropped/relabelled-opening regressions.
Earlier three-stage measurements omitted C4 and do not qualify complete Load.

`a_relation::native::load` assembles those exact four A stages and three W
continuations from typed original state/map witnesses and the five signed tapes.
Preparation verifies the original predecessor and all three Q proofs in full,
and decides both predecessor claims and every derived opening. Fixed installed
A/W keys drive proving and source-bound checkpoint restoration; the runtime
never generates keys or chooses a witness-dependent profile. The native terminal
exports the distinct accumulated Pallas, current Vesta, predecessor Vesta and
terminal-A opening obligations for final Omega. It grants no monetary head.
The ignored `installed_native_load_proves_and_restores_all_four_stages` regression
exercises genuine proving under fixture-installed keys and exact-original replay.

TODO: mount the complete authenticated producer inventory and canonical G1
conversion in the native wallet proof provider, then qualify the final common
Omega catalog, transport and physical-device execution. Source-stage assembly
and verifier-pack admission alone do not complete those release gates.

## Send composition

`a_relation::send` binds the exact payer credential, signed Request and held-fee
slot to the same state/statement cells and depth32 pending and fee map paths.
Zero held fees permit a total malformed dummy slot; enabled fees require the
exact held schedule and arithmetic. The receiver owns the Request signature.
Send separately requires the hard own receipt/current credential/certificate
Q and its mandatory fixed authorization task. `own::ConsumingProofCells` binds
its receipt digest to the exact 320-byte public transcript, predecessor proof,
both full-k16 transported claims and own sigma, including their lengths. The
mandatory `SendProof` task checks those bytes beside the hard predecessor;
the signature stage authenticates the same entire receipt tape in `D_ctx`.

The genuine mask0 chain now composes all five A stages with the two-bus Q_sigma
and four-bus A profiles. Every source proof is 8,480 bytes; stage maxima are
50,098 / 56,018 / 57,868 / 61,864 / 60,051 rows, all within k16. This includes
mandatory own authorization, exact consuming bytes, both maps and every
retained opening. Its Bootstrap/Load predecessor catalog is rebuilt under the
actual common Omega digest with identical source descriptors and terminal keys.
The Send terminal key has not yet been added and rebound into that catalog;
all eight control masks, the final uniform catalog and production Omega
size/row qualification remain open. Task metadata alone never proves execution.

Incoming lineage decoding now preserves the original byte source and derives
the canonical/header/claim verdicts together. Exact active-byte ingestion
separates a padded verifier view from the original short or overlong input;
its total Receive composition and operation qualification remain open.
Receive now has fixed owners for all five soft results, hard current-credential
and receipt authorization, original signature tapes, and terminal map/mode
constraints. Content addresses are mandatory: substituted Request, package,
credential or certificate preimages cannot manufacture a burn. The complete
owner-to-W-to-terminal proof chain and its capacity still need qualification.
The own Receive credit, payer-wallet and amount are hard projections of the
same Request; changing those generated effect fields cannot turn a valid
Payment into a burn. Original incoming asset/recipient mismatches remain soft.
The staged Receive context retains exact active tape commitments and typed Q
instances, with four-bus internal A circuits and a three-bus terminal candidate;
internal keys cannot enter the final Omega catalog.

## Unload and Retiring

The fixed k12 `UnloadCircuit` and `RetiringCircuit` share the administrative
transition constraints with Load. Unload spends only the folded lineage's
adjusted available balance, binds the scheme/wallet/redeem-ordinal nullifier,
and checks its charge, counters and exact changed fields. Retiring permits
only Active → Retiring and synchronizes adjusted burned/pending values without
moving value. Both real native proofs are 3,296 bytes and verify their complete
opening; rehashed state mutations, overflow, invalid charges, burned-value
spending and reverse/repeated retirement reject.

`a_relation::unload` assigns mandatory hard predecessor/proof-byte, current
credential/direct Enrollment/own receipt, and recovery-map or retirement-state
tasks. Its fixed schema rejects missing, duplicated and misplaced owners.
TODO: execute and qualify the complete genuine Q → A → Ω chains, rebind their
terminal keys into the common catalog, and connect the installed wallet provider.
The leaf and task checks alone do not authorize an unload or retirement.


## Archive state leaf

`admin_sigma::ArchiveCircuit` shares one k12 class across Receive-package and
`CreditStatus` evidence. The native proof is 3,296 bytes with at most 2,367
assigned rows. It binds both state openings and the statement, preserving every
core field except sequence, nonce and the removed core pending root, and every
rest field. Adjusted burned value and credit roots are unchanged. The map owner
separately authenticates both removals and selects the adjusted pending root from
the actual evidence verdict. Exhaustive rehashed core/rest mutations reject in
both evidence variants and both verdict branches.

TODO: compose the full authenticated evidence, corrected-claim/no-op, current
credential and own receipt obligations into the recursive Archive relation. This
state leaf does not accept delivery evidence or release retained Payment bytes.

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

`q_signature` implements ordered hard/soft P-256 signature leaves on the
17-advice/10-lookup Q layout. One Bounded public column contains ten words
per slot: digest, x/y/r/s as low128/high128, then verdict. Raw 256-bit
integers remain unreduced until the total P-256 checks. The existing SHA
codec binds the canonical Fp digest's LE bytes. Fixed keys and slot modes
are part of the circuit; A still binds these public words to object bytes,
certificate purpose and the global branch rule. The 5-variable/1-fixed
component uses 64,238 rows at k16. Signature proofs, exact-input mutations,
raw-width boundaries and every bridge cell are tested. This is not a
composed operation or runtime qualification.

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
