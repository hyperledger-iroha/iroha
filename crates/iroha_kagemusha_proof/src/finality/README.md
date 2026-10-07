# Ordinary Load finality

The target relation proves that an ordinary successful Load receipt occurs in
the canonical execution result signed by the normal validator quorum. There is
no Load publisher, delegated finality signer, or alternate issuer trust path.

The wallet independently selects a global signed-genesis anchor and the
source-qualified operation catalog. At height `h >= 2`, finality must establish:

1. The original Commit message contains the selected instance, exact native
   epoch/context, height, view, core block hash and execution-result digest `R`.
2. Its signing roster is the exact ordered committee authorized for that height
   by the genesis-rooted schedule. It has `n = 3f + 1`, `1 <= f <= 10`, and the
   aggregate signature selects exactly `2f + 1` distinct positions.
3. The complete original canonical result hashes to that same signed `R`.
   Its schedule and receipt parsers use the same committed byte tape and length.
4. The counted event tree contains the typed successful-Load event whose digest
   binds the exact ordinary receipt: scheme, asset, wallet, request, ordinal,
   amount, charge, transaction, successful height and payer account digest.
   These same terms govern the Load state effect.

The consensus assumption is the ordinary native one: at most `f` validators of
each authorized committee are Byzantine, and honest validators sign Commit only
after normal block, schedule and execution validation. An exact quorum therefore
contains at least `f + 1` honest validators. The relation proves quorum-certified
execution, not a circuit reimplementation of the VM, payload availability, or the
entire native envelope verifier. The native evidence provider retains all of its
independent original-envelope, custody, history and replay checks.

## Source composition

The BLS and ordered-key aggregation programs bind the same aggregate key and
message. BLS executes every one of its 1,084 original instructions through 542
fixed cursor pairs. Both operations retain their full byte context, affine
openings and phase-specific private registers, and their four interval/context
boundaries are constrained equal before the pair emits its outer frame. The
installed key fixes both operation cursors; no atomic-source fallback is admitted. Their complete verified composition exports only the certified
statement: ordered roster root, `n`, `f`, and original 165-byte Commit message.
The signer bitmap, key sum and signature remain bound to the verified child
programs; parent relations do not need to expose those private witnesses again.

The complete result scan consumes every original byte through all 515 original
transitions in 258 fixed proof leaves: Start plus Absorb, 256 Absorb pairs and
Finish. Both transitions bind the tape, expected digest, eight chaining words
and exact processed offset; each pair constrains all four interval/context joins.
Finished streams retain every state word through constrained padding steps.
The complete semantic interval remains exactly zero through 515, and only the
paired source classes are installed. The schedule parser
extracts the original native epoch and compact successor slots; its separate
CRC64 and BLAKE2b scan executes all 2,561 original instructions in 1,281
fixed proof leaves: 1,024 CRC pairs, 256 BLAKE pairs and one final BLAKE step.
Each pair runs both complete transitions and constrains their context, cursor and
all nine register boundaries. The complete semantic interval still ends at
2,561; an installed proof ordinal cannot replace that endpoint. It proves the
exact native context identity. These fixed schedules keep the qualified composition keys independent
of original frame length. The Load source
parses the original receipt, derives its typed-event hash, constrains all counted
Merkle steps, and extracts height/root/count from the same result tape.

Every source key is imported from original proving tables against its exact
witnessless compiled circuit and independently installed verifying key. A public
layout or native evaluator verdict cannot qualify a key. Binary composition
retains both curves: the four Pallas obligations are child carry/opening pairs;
both child Vesta claims enter the mandatory wrapper fold alongside its own
opening. The terminal owner must retain and decide the resulting full claims.

## History obligations

The selected genesis anchor must independently bind global root scope, network,
instance, exact initial epoch/roster and original initial schedule parameters.
The wallet must pin this complete anchor, not merely a caller-provided network
label. Genesis execution-result fields do not independently create authority.

A history step must consume the previous exact Ready slot, including its epoch
context and height. It must retain the promised following slot and lag-two
parameters. The current result's next-slot parameters equal the previous
after-next parameters. Without a boundary, those complete slots are identical;
with a boundary, the previous slot must have been PendingBoundary. The after
state is derived from the authenticated result's exact successor slots.

The closed history prefix has a separate program identity from conditional
history steps. Its context commits the anchor and the canonical history-wrapper
key digest. One shared wrapper admits exactly the original Genesis and Append
source keys. Append hard-verifies the predecessor using its witnessed canonical
key and constrains that complete key digest into the unchanged prefix context.
It also hard-verifies the fixed complete step source and joins the exact state
boundary. The terminal receipt verifier pins the installed history-wrapper key.
This closes the recursion with a finite key catalog; history growth does not
require a new verification key per block.

No partial BLS run, partial byte scan, standalone schedule, native proposal,
isolated source leaf or host-verified boolean can authorize a Load. Complete
program intervals, deterministic initial/terminal boundaries, source roles,
shared contexts and the genesis start must all be constrained. Native heights
are `u64`; the source-program `u32` instruction cursor must not silently truncate
or limit the history statement.

## Current integration boundary

The source components, finite history and terminal receipt compositions are
implemented. `native::InstalledFinality` mounts the six complete source programs
and their fixed composition graph against original proving artifacts. It retains
verifier metadata and reloads only the active proving tables, checking their
installed identity. Its history API produces genesis, appends each successive
block, verifies restored prefixes and proves a receipt at the terminal height.
Malformed native proposals never replace the required source proofs.

`catalog::compile` is an explicit offline artifact compiler. It shares the six
fixed leaf/class schedules with the native importer, emits one active source
and wrapper at a time, reimports their original tables, and builds the exact
balanced merges, semantic owners and finite history catalog. The completed
output must pass a fresh `InstalledFinality` import with the same terminal key.
`DirectoryCatalog` keeps only metadata resident, bounds original reads before
allocation and records canonical Norito integrity inventories. These unsigned
inventories and caller-recorded source provenance confer no deployment authority.
The three Context batch sources pass strict k16 constraints and exact
known/unknown/position layout checks: CRC pairs use at most 46,210 advice rows,
BLAKE pairs 32,118 and the final single step 16,060. Complete short and maximum
payload coverage and intermediate-state substitution tests pass. The source
arithmetic refactor also retains all five original Context component checks.
The native graph installs only these paired sources. All three classes pass
strict original key import and genuine source/wrapper verification, reject
foreign classes and every endpoint substitution, and retain their exact semantic
spans. Their measured wrapper proofs are 9,856 bytes; these are internal proofs,
not a Payment size result. The captured three-class run is in
`target/qualification/finality-context-batches/original-proofs/`. This remains
component evidence, not a complete Context tree or ordinary finality proof.
All 542 BLS pair source classes fit k16, with maximum 48,322 advice rows.
Complete native trace grouping, phase-boundary known/unknown/import layouts and
wrong-cursor/drop/reorder/context/signature mutations pass. Four selected BLS classes (initial, SHA, measured maximum and terminal) pass
strict original import and genuine source/wrapper proofs, including foreign
cursor-key and all endpoint mutations. Every one of the 542 pairs passes the
strict known-witness constraint check, covering all 1,084 original transitions.
These captures are in `target/qualification/finality-bls-pairs/original-proofs/`;
selected proofs and a complete constraint trace do not establish the complete
merged BLS proof or ordinary finality.
The three Result classes pass complete grouping/padding and intermediate-state
mutation checks, exact source layouts, strict original imports and genuine
source/wrapper proofs. Their maximum advice rows are 16,388, 32,630 and 2,035;
wrapper proofs are 9,856 bytes. The current paired catalog's 598 unique source
classes all fit k16 and preserve shared-class layouts. The exact one-block
receipt graph requires 14,066 individual source/wrapper proofs, or 8,944 when
the identical authorized Context proof is reused. These are structural counts,
not timing results. Current Result captures are in
`target/qualification/finality-result-pairs/original-proofs/`; the full short and
maximum Result constraint traces are still running.
TODO: qualify the compiler's full catalog and complete source proofs; the complete
ordinary-finality fixture has not yet run.

The CoreZK adapter derives the anchor from explicitly signed global-genesis
parameters, converts retained native block/event evidence into bounded witnesses,
and re-verifies terminal evidence before encoding recovery custody. The
five-A/four-W Load consumer verifies that terminal source under its installed key
and anchor. TODO: install and qualify the complete original artifact bundle through
signed genesis, receipt finality, Load and all downstream compact catalog
operations. Layout checks and individual source tests are not complete recursive
finality evidence.

The release gate remains a self-contained genuine Payment of at most 10,000
bytes. Internal source-wrapper sizes and structurally sized placeholder Payment
fixtures do not establish that gate.
