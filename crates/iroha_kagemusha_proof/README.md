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
recorded inputs and 33 local packages unchanged. These recorded source cuts
cover the frozen/retained frames, generated vectors and existing codec checks;
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
See `specs/kagemusha_a_split_context_v1.md`. This is operation composition,
not final Omega/artifact, hardware, time/RSS or wallet acceptance qualification.

`a_relation::native::bootstrap::Prover::from_original_artifacts` mounts the
original A1/W0/A2 keys from the independently installed Bootstrap plan and exact
descriptor/VK identities. Its fixed four-bus A source is reconstructed with
unknown witnesses before transaction preparation; W0 binds the complete A1 key
digest and A2 retains the imported W0 identity. Existing `PIPAPK01` import checks
source tables, copy mapping, selectors and commitments, followed by exact
installed-VK equality. These checks authenticate PK admissibility through the
authenticated verifier identity and fixed compiled source; this component adds no
PK signing format. The original-only constructor replaces decoded-key ingestion.
Actual preparation still verifies and decides both Q openings, binds the original
sigma tape and decides its part claim before producing `Prepared`. Genuine
A1/W0/A2 proving and retained checkpoint verification use the mounted keys.
The installation owner must still authenticate the complete scheme/Q/root/stage
catalog independently of operation inputs; component mounting grants neither a
`NativeProofs` owner nor wallet-open capability.

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

`a_relation::native::load::Prover::from_original_artifacts` mounts the fixed
A1/W0/A2/W1/A3/W2/A4 originals from independently installed metadata. Private
circuit views reconstruct the existing four-bus relation with unknown witnesses,
fixed predecessor/Q programs and stage-specific history lengths. Each W binds its
preceding imported A key; each continued A binds the preceding imported W key.
Canonical PIPAPK01 source/copy/selector/commitment validation and exact installed-VK
equality bind PK admissibility without adding a PK signature format. Runtime
preparation still verifies the original predecessor and three Q proofs, decides
all carried obligations and binds the same sigma/object tapes. Existing source
identity, restoration and terminal checks remain required. These components do
not supply complete producer-catalog admission, NativeProofs or wallet open.

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
`a_relation::native::send` assembles the genuine five-A/four-W source from
exact signed objects, depth32 witnesses, the full320 consuming transcript and
the original predecessor proof plus both claims. Fixed installed keys drive
native proof production and source-bound restoration. Its catalog requires
every mask0..7, with actual authorized sigma-selector digests; a repeated mask0
pipeline never marks a partial catalog complete. The ignored fixed-artifact
regression covers the actual mask0 source and original-proof replay. All enabled
masks, held-policy predecessors, final Omega and device execution still require
qualification. Native G1 pre-Advance Request/recipient-credential checks remain
mandatory and separate from Send A's owned obligations.

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

## ArchiveSent sigma

`admin_sigma::ArchiveCircuit` and `native::ArchiveProver` supply the single
`(ArchiveSent, 0)` sigma source shared by both Credited evidence forms. The k12
circuit opens both complete G1 states, binds the tag5 statement and sequence
advance, and preserves every core word except sequence, nonce and the committed
pending root. All rest words, adjusted burned value and permanent credit evidence
remain unchanged. It takes no evidence-verdict input and cannot refund value,
restore quota or authorize deleting retained Payment bytes.

Native Advance must authenticate the committed-root removal. A still binds the
retained Request/Payment/Credited originals, verifies evidence, and proves the
separate adjusted-lineage removal or no-op before durable cleanup. The sigma
component does not establish that complete ArchiveSent operation. The original
PK is admitted through the existing strict source/commitment importer against an
independently authenticated descriptor/VK and fixed operation; this requires no
additional PK-signature ceremony.

## RefreshPolicy sigma

`admin_sigma::RefreshCircuit` and `native::RefreshProver` supply one fixed k12
source and strict original-key owner for the single `(RefreshPolicy, 0)` selector. The actual statement kind derives five
one-hot circuit selectors, and the typed update projection is constrained to that
kind. Both complete G1 openings, tag7 statement, sequence, nonzero nonce, selected
field effects, monotone time floor and every unchanged field are constrained.
Inactive projection words are zero; policy controls are intersected with the
credential's permitted controls. All five kinds share the same source layout.

The projection grants no signed-object authority. A must still authenticate the
exact update, enforce immutable credential renewal identity, prove blacklist
history insertion and rebuild the complete quota usage array. Native Advance
must authenticate changed roots before persistence. A's existing five fixed
variants and the installed sixteen-selector sigma catalog remain unchanged.

The common source measured 2,499 maximum assigned advice rows at the fixed k16
diagnostic ceiling; `REFRESH_K` selects the existing k12 administrative class for
all five kinds. The typed owner uses the shared bounded original importer, checks
the compiled source/commitments and exact independently installed VK, derives the
statement instance from the typed witness, and fully verifies every produced
proof. Selector compression follows the authenticated descriptor, as for the
other administrative leaves; no runtime key generation or domain fallback exists.

TODO: execute direct k12 layout/adversarial checks, the original-import refusals
and genuine proofs for all five kinds under one imported key, plus shared-header
regressions. Complete authenticated producer inventory and A composition remain
required. This component does not establish Native wallet open or device
qualification; wallet open continues to return ArtifactsUnavailable (-4).

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

`q_sigma::native::QSigmaSource` removes live prepared-operation input from
original-key installation. It checks one genuine member VK per fixed slot
against the complete independently installed plan/catalog and descriptor. The
member supplies layout metadata only: private source construction assigns its
key coordinates, proof tapes and modes as unknown, and preserves every fixed
catalog entry. Installation checks the exact k16/Pallas profile, bounded original
source/commitments and installed VK; no extra PK signature or runtime keygen is
required. Actual `QSigmaPlan::prepare` and `QSigmaProver::prove` still verify the
real proofs, decide required claims/folds and fully verify the produced Q proof.

The source-only constructor/layout tests, existing soft-failure classifier and
both genuine imported Q proof regressions passed: five host cases with the same
1,748 recorded inputs unchanged throughout. Complete authenticated producer-graph
installation and Native typed intake remain separate requirements; this component
does not grant wallet-open capability or qualify any Android device.

`q_signature::native::QSignatureProver` imports the original k16/Pallas/V2
signature key against an independently installed immutable slot plan, descriptor,
VK and compiled source. It derives low-S P-256 verdicts from raw signatures,
refuses invalid hard slots and proves false verdicts for invalid soft slots,
then fully self-verifies the proof. It generates no runtime key. A still binds
original tapes, signature roles and the global branch; this leaf does not grant
complete producer-catalog admission, a Native wallet owner or wallet open.

`admin_sigma::native` provides typed `BootstrapProver`, `LoadProver`,
`ArchiveProver`, `RefreshProver`, `UnloadProver` and `RetiringProver` original-key importers over the existing
administrative circuits. Each fixes Vesta/k12, the PIPA-R direct bounded statement
profile and its compiled operation, checks original source tables and commitments,
and requires exact agreement with the independently installed VK. Selector
compression remains the authenticated descriptor's choice, checked by the same
source importer; no profile fallback or runtime key generation occurs. Proving
derives the public statement digest from the typed witness and completes native
verification before returning bytes. The shared descriptor does not authorize
substitution of another operation's original key. These are proving components;
A still authenticates objects, maps, signatures and predecessor obligations.

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

- `admin_sigma` and `consuming_sigma`: the `installed_admin_sigma_originals_`
  cases import all four typed administrative keys and exercise genuine proof
  verification, complete opening decisions, changed public inputs, corrupted
  proofs and rehashed invalid value transitions. A same-descriptor four-by-four
  original-key substitution matrix and malformed/bounded/profile/curve/VK intake
  checks cover the import boundary. Test producers generate fixture originals;
  Native imports do not. These cases do not establish a signed producer catalog,
  final Omega, Native wallet open or physical-device qualification.

- `archive_sigma`: complete rehashed unchanged-field and statement/opening
  mutations, source-layout equality, imported-key proofs for Active and Retiring
  under the same key, full opening decisions, forged-refund rejection and
  bidirectional administrative-key substitutions. These cases exercise sigma
  state effects and original intake; they do not authenticate Credited evidence
  or qualify complete ArchiveSent execution, wallet open or physical devices.

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

## Unload native composition

`a_relation::native::unload` prepares exact original state/object/proof inputs for
four fixed A stages and three W continuations. A1 hard-verifies the folded
predecessor and binds its canonical 320-byte public prefix, original proof and
both full transported claims together with the own sigma to the signed receipt.
A2 constrains the depth-32 redeem recovery-map insertion, A3 hard-verifies the
Unload sigma Q under selector 13, and A4 re-verifies the current Credential,
direct Enrollment certificate and own receipt through the hard 2V1F signature Q.
All stages bind the same immutable source context. Preparation and checkpoint
restoration verify actual proofs and decide every retained Pasta obligation;
there is no witness-selected range profile or runtime artifact generation.

The source profile fixes four range buses and k16 A/W artifacts. The uniform
current Omega transport bound is 4,821 bytes; the native Plan rejects a larger
predecessor descriptor. Its terminal A4 and distinct accumulated Pallas, current
Vesta, predecessor Vesta and own-opening obligations are inputs to the final
Omega producer. They do not complete a ledger payout or grant a monetary head.

TODO: execute the complete genuine four-stage proof/restoration regression with
an authenticated compact folded predecessor containing loaded funds; current
generic predecessor rejection and genuine sigma/signature-Q tests supply no
accepted Unload, producer-catalog or physical-device qualification. Authenticate
and mount the complete producer inventory and canonical G1 conversion before
integrating the native wallet operation facade.
