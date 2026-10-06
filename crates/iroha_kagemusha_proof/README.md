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

Wallet framing now pins direct `u128` records/enums to 16-byte archived alignment,
with enclosing fields and inline arrays inheriting it. Shipping assertions preserve
all 26 existing frame padding values and retained-original layouts without changing
Norito primitives, decoders or frozen vectors as part of that alignment change.
Nine selected host codec cases passed in three runs, each with the same 3,435
recorded inputs and 33 local packages unchanged. Those recorded source cuts
cover frozen/retained frames, generated vectors and existing codec checks;
subsequent source changes require fresh validation. ARMv7 compilation, runtime
and physical-device byte parity remain separate qualification gates.

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
The installed native Bootstrap producer also passes actual Tagged3 A1/W/A2
proving against those original keys. The shared native differential suite passes
all three selected Bootstrap/Load/Send tests; its binary/source snapshot is in
`target/qualification/native-tagged-differential-source.json`. These are
component checks, not time/RSS gates. See `specs/kagemusha_a_split_context_v1.md`;
final Omega/artifact, hardware and wallet acceptance qualification remain open.

## Load components

The current component proofs below use the superseded dedicated-publisher
voucher trust model. The accepted Load design derives authority from ordinary
transactions and block finality. Native and recursive Load must adopt that proof
before these Load-derived catalog chains can qualify the release; their recorded
size and key-continuity results remain scoped to the measured component candidate.

`admin_sigma::LoadCircuit` proves both state openings, exact arithmetic,
continuity and unchanged fields on the five-column k12 class (3,296 bytes).
`a_relation::load` binds five original tapes: LoadAuthorization certificate,
finalized voucher, own receipt, Enrollment certificate and current credential.
The mandatory signature workload is three variable and two fixed signatures,
split across two hard Q leaves. C4 re-verifies the current credential on every
step; a predecessor proof does not replace that obligation. The fixed schedule
now has separate predecessor/recovery, Q_sigma, voucher/receipt and current
credential stages. The common compact catalog's four Tagged3 source stages
prove at k16 (maximum 61,706 rows, 7,744-byte A proofs), including the complete
signature-Q bundle and all carried claims. Earlier three-stage measurements
omitted C4 and do not qualify complete Load.

`a_relation::native::load` assembles those exact four Tagged3 A stages and three
W continuations from typed original state/map witnesses and the five signed tapes.
Preparation verifies the original predecessor and all three Q proofs in full,
and decides both predecessor claims and every derived opening. Fixed installed
A/W keys drive proving and source-bound checkpoint restoration; the runtime
never generates keys or chooses a witness-dependent profile. The native terminal
exports the distinct accumulated Pallas, current Vesta, predecessor Vesta and
terminal-A opening obligations for final Omega. It grants no monetary head.
The ignored `installed_native_load_proves_and_restores_all_four_stages` regression
passes all four actual A proofs, three W continuations and exact-original
checkpoint replay in `target/qualification/native-tagged-differential.log`.
The complete native suite passes 3/3 in 4,079.26 seconds on the busy host; this
is component evidence for that recorded binary, not a timing gate. Its Load
trust model remains superseded as described above.

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

The genuine mask0 chain composes all five A stages with two-bus Q_sigma and
Tagged3 A profiles. Every source proof is 7,744 bytes; stage maxima are
36,704 / 56,018 / 57,868 / 62,212 / 60,051 rows, all within k16. This includes
mandatory own authorization, exact consuming bytes, both maps and every
retained opening. Bootstrap, Load and Send-mask0 now share one actual compact
Omega key: all signed source chains are rebuilt under its digest, and exact
source/outer key equality and native proof verification pass. Each outer proof
is 3,712 bytes; transport is 4,800 bytes. Fresh merged-wire size tests retain the 1,723-byte Payment overhead: structural
3,456-byte sigma plus 4,800-byte Omega encode to 9,979 bytes, leaving 21 bytes
under the 10,000-byte envelope cap. This checks encoding, not proof admission.
These are three-terminal component results. The other seven control masks, full
uniform catalog and loaded-host/physical-device qualification remain open.

`a_relation::native::send` assembles each fixed-mask five-stage chain with
fixed Tagged3 columns and installed A/W keys. Its immutable catalog requires one
source for every mask0..7 and an authorized sigma selector for each; repeated
mask0 keys never establish catalog completeness. It checks the original predecessor,
both Q proofs and all carried claims, frames the complete consuming tape, and
retains source-bound proof/claim checkpoints for exact restoration. Its signature
stage uses the installed root policy and the original Credential, Enrollment
certificate and Advance Receipt. Six input/frame rejection tests pass. The
ignored `compact_predecessor_native_send_preserves_every_installed_stage_and_original`
passes all five A and four W native proofs under the original installed keys,
including exact-original checkpoint replay, in
`target/qualification/native-tagged-differential.log`. Its compact Load
predecessor still uses the superseded trust model described above; the result
qualifies that native component snapshot. The other seven control masks, current
Load trust replacement and complete authenticated catalog remain required before
wallet admission.

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
The genuine Unload and Retiring component run passes all four A proofs and three
W continuations for each operation, with hard predecessor, own authorization,
original consuming bytes, maps and every opening decision. Each A proof is7,744 B.
The run's Bootstrap/Load ancestry uses the superseded voucher trust model, so it
does not qualify the ordinary-finality Load replacement. TODO: rebuild that
ancestry, qualify the complete Q → A → Ω chains, rebind their terminal keys into
the common catalog, and connect the installed wallet provider.

`a_relation::native::consuming` assembles the four fixed Tagged3 A stages and
three W continuations using installed artifacts. It checks original Q and
predecessor proofs, decides every claim, and restores checkpoints only against
the retained source context. Its terminal retains predecessor Vesta, current
Vesta and terminal-A opening obligations separately. The reader also requires the exact original public320/proof/claims tape and
rejects Omega transports above the common Payment bound before proof admission.
Six previously recorded exact-source and encoding tests plus strict lint pass; genuine installed-key differential
proving and replay remain unqualified.


## Archive state leaf

`admin_sigma::ArchiveCircuit` shares one k12 class across Receive-package and
`CreditStatus` evidence. The native proof is 3,296 bytes with at most 2,367
assigned rows. It binds both state openings and the statement, preserving every
core field except sequence, nonce and the removed core pending root, and every
rest field. Adjusted burned value and credit roots are unchanged. The map owner
separately authenticates both removals and selects the adjusted pending root from
the actual evidence verdict. Exhaustive rehashed core/rest mutations reject in
both evidence variants and both verdict branches.

Archive now declares seven mandatory owners. The hard own owner binds the current
credential, direct Enrollment certificate, receipt and exact tag5 sigma tape.
Two own-authorization component tests pass with genuine sigma/Q0/Q1 proofs and
original-byte mutations. Two result-context tests bind every proposed bit and
all original Status-opening limbs to the fixed complete owner set.
Typed result claims commit all three evidence verdicts and, for CreditStatus,
the original incoming opening under a separate internal-word context domain.
Incoming proof and terminal map owners consume these exact commitments; their
complete composition is still pending.

Canonical CreditStatus component hashes and Credited Payment/evidence hashes
hard-bind their exact preimages before any soft verdict. A substituted component
cannot manufacture a no-op. Noncanonical original references remain false and
retain the original transcript digest. The corrected Request receiver-credential
binding uses field8; its certificate-set field17 stays distinct. Six focused
delivery/status checks pass, including all-byte mutations and changed-preimage
tests under both proposed verdicts. Two retained-Payment tests bind its original
signed sources, exact opaque proof bytes and lengths, Send statement and pending
descriptor. Those fixtures explicitly do not claim historical proof validity.

The fixed Archive stage dispatcher requires every owner, the complete original
source schema, and Q1/Q2 at their authorization/signature owners. Constructor
tests reject missing or duplicate tasks, substituted categories and misplaced
Q verifiers. Incoming-source tests enforce the held Request's quoted credential
while retaining malformed incoming receipt bodies and signatures for their soft
owners. Status takes its carried proof digest from its exact original transcript;
a disagreement with the receipt derives false evidence without replacing those
bytes. That totality regression passes. Independent local source review found
no binding gap in the new dispatch, incoming, evidence, proof and result paths;
this is component review, not complete recursive execution or artifact admission.
The incoming signature owner also passes with actual Q2 proofs for both valid
and invalid signatures. Each proof is7,936 bytes; the isolated A owner uses at
most24,294 rows. Changed receipt bytes, signature, quoted key, verified verdict,
result proposal, context ordering, proof bytes/length and stage assignment all
reject, including after recomputing original object digests. Known/unknown
layouts match. These local Q2 proofs are not the transmitted terminal Ω.

TODO: compose and qualify retained-Payment, evidence, signature, corrected-claim
and all recursive owners under admitted terminal keys. These components do not
accept complete delivery evidence or release retained Payment bytes.

## RefreshPolicy

`admin_sigma::RefreshCircuit` uses one k12 key for all five tag7 update kinds.
The kind is constrained to exactly 1 through 5; it does not select a host-side
circuit or key. Each genuine proof is 3,296 bytes with the same 2,538-row maximum.
All five proofs verify under one key, and known/unknown fixed columns,
permutations and assignment footprints agree across kinds. The genuine Q-sigma
composition also passes for every kind under one shared Q key, with 7,008-byte
local Q proofs and complete sigma/Q opening decisions. Rehashed changes to
fields owned only by another kind reject, as do invalid selectors, stale
counters, changed adjusted lineage, invalid policy intersections and expired
shares. This leaf binds the exact state effects and statement; A authenticates
the signed update projections and owns blacklist-history insertion and the
fixed64 quota rebuild.

`a_relation::refresh` requires the exact five original signed tapes in its
recursive context, hard update/receipt and current-credential Q slots (3V/2F),
direct purpose-scoped issuer certificates, state effects and every kind-specific
map owner. Credential renewal authenticates the old credential against the
predecessor and the replacement against the successor. Genuine compact
Bootstrap-rooted Credential, SchemePolicy, TimeAnchor and Blacklist compositions
now pass: each closes four A stages and three W stages, with 7,744-byte A proofs,
actual sigma/Q0/Q1/Q2 proofs and every native opening decided. Their maximum
assigned source rows are 61,725, 61,671, 61,659 and 61,665 respectively. Omitted
proofs/fold claims, altered Q proofs, dropped history and changed original
receipts with recomputed contexts reject. Internal A keys are excluded from
terminal-only catalog checks. Logs and source/binary manifests are retained in
`target/qualification/refresh-recursive*` and `refresh-map-recursive*`.

The complete Quota source exceeded k16 at 91,131 rows; its failed attempt is
retained and no oversized proof was generated. Its replacement separates three
fixed64 array-root owners from the deterministic matching owner. The shared
context commits typed old/window/used array hashes plus issue/count; each root
opens every array it reads, and the mandatory matching owner opens all578 words
and binds the signed share fields. All 64-slot checks remain. Isolated typed
owners pass with a 28,638-row maximum, but the first complete eight-stage attempt
failed at the PreviousRoot stage: four valid 7,744-byte A proofs preceded its
68,931-row overflow. That failed source/binary is retained in
`target/qualification/quota-split-recursive*`. The revised root-only path carries
object digest/length/tape proposals; mandatory signed-original owners recompute
all five originals, and the merge owner binds the signed issue/count. Full
closure remains pending, with every revised stage preflighted before proving.
Exact native source artifact imports reject superseded arithmetic descriptors
before checkpoint use. The installed `native::refresh` candidate now assembles
all five kinds from actual predecessor/Q proofs and exact signed originals,
using the fixed four- or eight-stage schedule. It consumes witnesses for proving,
self-verifies every A/W proof and restores original bytes with source-bound
history and full accumulator decisions. Seven pure native rejection tests pass. Genuine Credential parity also passes
(1,111.59 s): all four A and three W proofs reproduce exact diagnostic bytes under
the original installed keys, every checkpoint restores, and changed proof/Q or
invalid stage transitions reject. Its source and binary are retained in
`target/qualification/native-refresh-credential*`. Other native variants remain
under qualification.

TODO: qualify the split Quota chain, prove the final Refresh Ω wrappers under the
full common terminal catalog, and connect the installed wallet provider. The
four genuine Q/A/W component closures do not admit release terminal keys or
establish latency, memory or physical-phone compliance.

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

`SigmaProver` generates producer keys or imports an original proving key with
`from_original_artifact`, then rejects invalid witnesses. `SigmaVerifier`
rebuilds from descriptor/key bytes and pinned parameters. `SigmaAllowlist`
selects by `(operation tag, mask)`. Sigma and Q imports check bounded originals
against the independently selected descriptor, verifying key and compiled source;
they regenerate no key and reject substitutions. Default and explicit serialized
Q profiles never fall back to another profile. Original/domain bounds do not
qualify total synthesis or prover memory.

`q_signature::native::QSignatureProver` imports the original k16/Pallas/V2
signature key against an independently installed immutable slot plan, descriptor,
VK and compiled source. It derives low-S P-256 verdicts from raw signatures,
refuses invalid hard slots and proves false verdicts for invalid soft slots,
then fully self-verifies the proof. It generates no runtime key. A still binds
original tapes, signature roles and the global branch; this leaf does not grant
complete producer-catalog admission, a Native wallet owner or wallet open.

The installation owner must authenticate the signed scheme and complete producer
inventory before import. These constructors supply proving components only;
they supply no `NativeProofs` owner or wallet-open grant. Freezing the production
artifact set and integrating Λ/Ω with wallet and node paths remain separate
G3–G5 work; this crate's σ implementation alone does not establish complete
protocol readiness.

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
  tampering and deterministic keys/proofs; quota shapes use k14. The two
  `installed_sigma_originals_` release cases exercise imported Send/Receive keys
  on both Pasta curves, actual proofs, public-input tampering and import refusals.
- `q_sigma`: `actual_two_sigma_q_proof_verifies` and
  `installed_serialized_q_originals_prove_and_reject_default_profile` exercise
  original-key continuity, explicit profile refusals and genuine imported Q
  proofs. All four selected import cases passed. Fixture producers generate the
  originals; the import path does not. The default Q's 10,496-byte proof is a
  local component, not a final Payment/Ω size or wallet/device qualification.
- `q_signature::native` and `q_signature`: one selected Native verdict case and
  three explicitly selected release cases passed. Two release cases generate
  and fully verify genuine imported hard/soft signature proofs, retain their
  openings and reject changed inputs/verdicts; the third checks key, fixed-slot
  source and original bounds refusals. Each of the two runs retained its 1,700
  inputs and 17 local dependency packages unchanged. The earlier unapplied
  zero-case attempt contributes no passes. The hard signature Q's 8,576 bytes
  qualify this local component only, with Payment/Ω and wallet/device gates open.

- `measure`: ignored diagnostic throughput/footprint workloads. They are not
  the fresh-process qualification procedure in the design record.

Together these Sigma/Q and signature runs passed eight selected Native component
cases across their separate guarded cuts. This count includes refusal and Native
verdict cases; it is neither eight proof-generation cases nor one unchanged
current release. Fixture producers generate original keys; Native imports do not.

```sh
cargo test -p iroha_kagemusha_proof
cargo test --release -p iroha_kagemusha_proof --test real_proofs -- --include-ignored
cargo clippy -p iroha_kagemusha_proof --all-targets -- -D warnings
```


`admin_sigma::native` provides typed `BootstrapProver`, `LoadProver`,
`UnloadProver` and `RetiringProver` original-key importers over the existing
administrative circuits. Each fixes Vesta/k12, the PIPA-R direct bounded statement
profile and its compiled operation, checks original source tables and commitments,
and requires exact agreement with the independently installed VK. Selector
compression remains the authenticated descriptor's choice, checked by the same
source importer; no profile fallback or runtime key generation occurs. Proving
derives the public statement digest from the typed witness and completes native
verification before returning bytes. The shared descriptor does not authorize
substitution of another operation's original key. These are proving components;
A still authenticates objects, maps, signatures and predecessor obligations.
