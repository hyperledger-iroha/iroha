# Experimental native backend

The experimental catalog has four recursive pairs. Bootstrap, Send and Receive
each have one pair. Control shares one pair across Load, RefreshTimeAnchor and
Unload. All use the future 532-byte credential body and the 30-field monetary Core. It is distinct from the installed
wallet generation. Installing its public programs does not migrate a funded wallet
or change a ledger, enrollment record, hardware key or journal.

## Installation and account admission

The eight public verifier programs contain verification instructions, constants and
geometry only. Private witness arrays and witness-producing closures are excluded.
Native verifies the Artifact-role signature on the version 4 parameter manifest
under its installed scheme root, then checks all eight program digests, fixed keys,
generator profiles and the ledger scope against its installed signed genesis.
PublicProgram.v3 carries one canonical Norito payload in a single CPU zstd
1.5.7 frame at level 6 with a 2 MiB window, declared content size and checksum.
The instruction stream groups backward slot and constant deltas by opcode and
byte plane. Import restores the ordered SSA operands and validates their
bounds, layout and degrees. The complete program bundle is capped at 10 MiB.
The five-entry wallet carrier applies its limit to all metadata and stored originals.

The installed runtime transfers its existing exclusive provider into the future
frame types. It retains the same platform, protected storage, journal, original
source and close/recovery owner. A program bundle supplied by an application cannot
choose a new scheme root or finality reader.

Account admission takes four exact originals: the 596-byte signed future credential,
the 172-byte Enrollment certificate, canonical AccountId and canonical asset scope.
Native verifies the root/issuer chain and recomputes the certified payment table
from the hardware public key. It enumerates actual custody slots, checks the retained
enrollment intent/request, probes the real key, and authenticates the selected future
capsule and completion. Before Bootstrap, the credential must equal the exact E6
original already retained in custody. A legacy E6 original is not a future original.

The existing Ed25519 account signs a fresh Native challenge bound to the exact signed
parameter-manifest digest, selected slot, exact marker file and all four originals. Native rechecks that
same source and key before authorizing the wallet. No caller-selected slot, decoded
credential, signature-validity Boolean or account identifier substitutes for these
checks. Monetary calls remain unavailable until admission completes.

When both copies of the current Released completion are lost, Native can authorize
a fresh, separately domain-bound account challenge for historical output reads.
This requires the verified current capsule, retained enrollment, positive hardware
key, matching marker and an intact copy of every checkpoint selected by that marker. A Keychain anchor must
name that exact marker. The SDK exposes this mode as `retainedOutputsOnly`.
The current operation returns delivery-data loss. An earlier retained Payment
returns its original bytes. The restricted session cannot prepare or finish
monetary operations, including after a previous completion is restored.

## SDK sequence

Swift and Kotlin expose the same stages:

1. For a signed Spartan release, `KagemushaWalletInstalledRuntimeV1.openInstalledSpartan()`
   opens the producer and verifier programs selected by the installed five-entry carrier.
   Native rechecks the carrier root and returns the authenticated parameter identity.
   The isolated fixture can use `installSpartan` with its explicit public originals.
2. `beginAccount` returns the exact 32-byte challenge for the existing account.
3. `authorizeAccount` verifies its Ed25519 signature and returns
   `KagemushaSpartanWalletV1`, bound to Native's selected slot.
4. The wallet prepares Bootstrap from admitted enrollment, Load from a finalized
   receipt, Send from a signed peer Request, and Receive from the issued Request
   ID and incoming Payment. Native derives the private operation witness from its
   protected slot and verifies the selected predecessor. It returns a review
   bound to one immutable capsule. The projection includes the
   operation, amount, fee, receiver, before/after heads and capsule digest.
   To import a signed time anchor, Swift exposes `prepareRefreshTimeAnchor(anchor:certificate:)`
   and Kotlin exposes `prepareRefreshTimeAnchorFromOriginals(anchor, certificate)`.
   Both take the canonical anchor and its TimeAnchor-role signer certificate. Native
   verifies the issuer, scheme and wallet, opens the selected predecessor, and freezes
   the update for retry. The 243-byte version 2 review for tag 7 identifies the signed
   anchor, issuer time, successor floor and both heads. An imported anchor updates
   the signed floor. Elapsed-time evidence requires a separately authenticated direct
   exchange on the same boot.
5. The Send and Unload projections expose the complete Native review original as
   `bytes` in Swift and `bytes()` in Kotlin. The installed mobile owner requires
   an exact, fresh hardware approval for both debits before durable selection.
   The proof engine independently checks its P-256 signature against the frozen
   capsule and complete terms. The SDK displays the full asset, receiver, amount,
   fee and total, or both canonical Unload destinations and the charge.
   The Rust provider can challenge a separate platform-certified user-presence
   key with a single-use nonce over the manifest, slot, wallet, operation,
   capsule and complete Native review. It can verify the signature and retain
   its canonical original alongside signed time in the protected checkpoint.
   A fresh confirmation preserves the first approval for history and commits the
   exact newly signed receipt as the current choice in one protected checkpoint.
   Only a Released predecessor can publish the choice. The Selected marker
   inherits that checkpoint and recovery compares its exact digest. Android independently probes the per-slot Keystore key
   and requires a hardware-enforced authentication for each signing operation.
   The SDK presents the exact reviewed terms and receives the authorized
   CryptoObject. Android 26–29 require a strong enrolled biometric for this debit.
   A credential-only device receives a typed debit refusal and retains enrollment.
   On Android 30 and later a strong biometric or device credential can approve.
   On iOS the provider owns the fresh LocalAuthentication context and exact
   reviewed prompt, then independently checks the Secure Enclave key and ACL.
   Callers can cancel the outstanding challenge while the prompt is active.
   At A3 the provider consumes the one-use live approval after validating time,
   capacity and financial policy. It compares the complete capsule, projected
   review and protected archive original. After the actual Selected marker is
   anchored, the provider requires a matching protected record before monetary
   hardware signing, adoption of a staged completion or release. Recovery loads
   only the certificate for the already-selected exact capsule and checks that
   the protected marker has not changed. This path cannot start another debit.
   Physical-device completion and power-loss recovery still need qualification.
   On iOS an empty message and a single signature byte of value one ask Native to
   provision a separate permanent Secure Enclave key. Native chooses a fresh random
   Keychain tag, creates the key with passcode-set device-only accessibility and
   user presence, rereads its public point and commits that identity under the
   protected wallet checkpoint. Apple documents the access control of a Secure Enclave
   item as a creation-time attribute. [Apple Developer Technical Support](https://developer.apple.com/forums/thread/724013)
   Returning status 93 acknowledges the durable key pin.
   An interrupted attempt cannot adopt an earlier unpinned key. Loss or replacement
   of a pinned key refuses new approvals. The production recovery process for lost
   approval keys requires physical-device qualification.
   A 32-byte challenge and a single zero byte in the C signature field open a fresh
   OS context. The SDK presents the complete frozen terms before invoking the system
   prompt. iOS controls how much text appears during biometric or passcode entry.
   Cancel and wallet closure invalidate that context. The iOS bridge accepts no
   application-supplied signature or prompt text. Physical-device operation remains open.
6. `output` reconciles durable selection and returns the exact retained head,
   Payment or completion, with separate Unknown, Pending, Archived and DeliveryLoss
   results. Retries transport the original Payment bytes; they do not prepare another
   debit or regenerate a released proof.

The separate result domain preserves the production wallet ABI's status meanings.
The C entry points use `connect_norito_kagemusha_spartan_*_v1`; Android uses the
`NativeSpartanBridge` JNI class. Review tokens remain owner-local and may retry only
the same frozen capsule after Pending or an uncertain call failure. Closing releases
the owner while preserving committed custody.

Installation retries authenticate the exact original bytes against the retained
installation identity. Execute caches every terminal result before foreign output
allocation, so a lost reply cannot turn a definite NotPerformed into a later debit.
Pending and uncertain calls keep their frozen operation available for recovery.

## Wallet-owned transfers

The payer indexes the signed open Request under its wallet and installed manifest.
It retains the exact peer original and frozen capsule before returning a Send
review. Receiving indexes the complete Payment and retains its issued Request ID.
A retry checks those originals. Once selected, it reopens that exact capsule.
For an unselected review, Native may derive a fresh capsule only when protected
custody proves that a competing operation won at its predecessor, or when its
current predecessor still holds and a time-controlled Send needs a fresh
observation. Another pending operation or missing selection history prevents
replacement. The chosen operation ID remains available through protected output
lookup after subsequent payments.

Transfer preparation reads the consumed, credit, pending, fee and recovery maps
by the exact roots of the verified predecessor. New openings and descriptors
pass through a bounded private workspace. The selected monetary head determines
which root has spending authority. Missing map data returns a custody error.
The v4 Bootstrap clears lease and quota controls and starts with an empty
prior-credential history. Its signed programs preserve those fields. Native
refuses a Send requiring unavailable quota openings or a Receive requiring an
unavailable prior-credential opening before changing custody. A future generation
that enables these operations needs authenticated policy and credential refresh
transitions.

## Receiver-issued Requests

An admitted receiver calls `issueOpenRequest` with a stable 32-byte action id,
a positive base-unit amount and the signed fee originals selected by its current
head. Native opens the retained successor state, verifies its commitment against
the Released proof, and derives the wallet, asset, fee, policy and account terms.
It publishes the Request nonce and unsigned plan before issuing a separate
protected signing intent. The existing payment key signs the Request domain.
Status 96 returns the exact retained IPM1 schema-3 Request, including the
receiver's canonical account original. The payer fills its wallet, account
and next ordinal when it completes the Request.

Retrying the same action and id returns the stored wire. A changed amount or
fee original under that id is refused. If signing was interrupted, Native
can verify and adopt a staged signature. If neither copy of the result exists
after signing started, the id reports custody loss and a new Request needs a
new id. Issuing a Request does not change a balance.

## Local regulatory lists

An admitted wallet can call `installBlacklist` with the complete signed list and
its certificate set. Native verifies the RegulatoryPolicy signature against the
installed scheme, checks the complete ordered entries and stores two copies of
the original before publishing the protected selection. Swift and Kotlin return
the signed list's 32-byte digest on status 95. Reinstalling the exact original
repairs missing copies. Replacing a list requires a greater signed list version.
Each update records the address of the single unselected original in the
protected checkpoint before writing it. The next authenticated installation
can complete cleanup after interrupted publication or retirement.

Send checks the receiver account in the completed Request. Receive checks the
payer account. Native enforces the wallet's current list during preparation and
again before it selects a fresh transfer. A listed account returns `Blocked`.
An execution token that received this refusal retains it on retry. A later
authorized list can permit a new review of the same counterparty.
If a selected original is lost, new transfers require the original to be
restored or a newer authenticated list to be installed. After a transfer has
been selected, recovery follows its exact capsule through completion and returns
its retained bytes even if the local list has changed. Lists remain in local
custody. The wallet sends no list entries to a counterparty.

## Proof and custody boundaries

Each complete pair verifies both scalar/point proofs, both accumulated matrix
claims, and both own and inherited generator claims. Native receives no trusted
"proof valid" flag from the application. Receive binds the original complete
Payment bytes: the total parser derives its canonical selection and the actual
incoming proof checks determine credit or Burn inside the relation.

Load requires actual canonical Sumeragi finality under the installed genesis reader
and exact ordinary Load receipt. Before preparing an irreversible Send, Native verifies the receiver’s signed
Request, the exact 596-byte receiver credential, its Enrollment signer and the
ordered signer set named in the Request. A fee schedule requires the exact
RegulatoryPolicy signer and its authenticated digest. The payer's held schedule
and the rounded fee are checked in the complete Send preparation. The capsule
retains the originals and recovery repeats the authority check. In the isolated device fixture, the A0–A12 provider retains redundant frozen copies, selects the
head durably, signs using its persistent nonexportable hardware key, proves and
verifies the result, and retains the exact completion before releasing its bytes.
Recovery verifies retained completion bytes instead of generating new randomized
proofs for already released operations.

The version 2 private custody capsule records both the relation ID and the digest
of the signed producer manifest. The hardware receipt binds the entire capsule.
On restart Native requires the original manifest before restoring the private witness.
An in-memory verified head records the same manifest. Fresh preparation uses
that head after checking its scope and custody. Reopening a retained head verifies
the original proof bytes again. The scheme identity commits to its root key,
network, relation and provider contract. [Head and capsule review](output/production-20261119/final-head-and-capsule-fable.json) · [Scheme transcript](../../crates/iroha_data_model/src/kagemusha/kagemusha_wallet_v1/identity.rs)
For Receive, each fresh preparation or cold restore derives the candidate pair from
the exact frozen Payment. Finalization consumes that in-memory pair after checking
the selected hardware receipt and verifies the folded result before release.

Proof masks and the pair-binding mask use fresh native entropy. Deterministic
streams are restricted to explicit test replay. Public program transport, malformed
proof parsing, and exact Payment serialization are separately bounded and canonical.
An authenticated Spartan wallet issues a signed open Request and can return the same
wire bytes on a retry with the original request ID. The uncompressed schema-3 IPM1
Request carries the 482-byte open tape, 596-byte credential, ordered signer originals,
optional fee schedule and the receiver's exact canonical account. Native checks the
receiver signature, account digest, certificate roles and held fee policy. The Swift
and Kotlin carriers validate both lengths and keep all original bytes. They retain
the exact Payment for redelivery and can submit an incoming Payment to the native
proof verifier. The installed wallet's profile-1 adapter rejects the Spartan wire.

CreditStatus binds a completed Receive to the payer's verified Payment and signed
Request. Native verifies the receiver's hardware receipt, both folded proofs and
the inherited accumulators, and opens the exact credit-and-Payment leaf against
the resulting head. A canonical bitmap omits default map siblings. The leaf yields
distinct credited and burned results. Evidence must fit 10,000 canonical bytes and
10,084 bytes including IPM1. The receiver proof codec reconstructs the fixed KGSPPR2
prefix, matrix query padding, both matrix values and both inherited generator points
from the signed programs and the public query coordinates and challenges. The payer
then verifies the reconstructed pair. Admission checks the maximum legal statement,
lineage and next key with all 32 non-default siblings. The ordinary Receive step and
the folded lineage share one recursive pair in this profile. It has no independent
unfolded Receive proof or ArchiveSent selector. Wallets retain the exact Payment for
redelivery. Transport qualification includes a full NFC or Nearby exchange using this
evidence on two physical phones.

Spartan IPM1 profile `0x8001`, schema `3`, kind `4` carries two folded-head
projections of `Credited::Status`. `KGSCST3` followed by a zero byte selects the
Receive projection. A first byte with bits 7–5 equal to `101` selects a folded
receiver Send. Bit 0 carries Burned, bit 1 carries lifecycle minus one, bits 3–2
carry the two enabled Send controls, and bit 4 denotes all 32 non-default siblings.
The dense form omits the sibling bitmap. The installed Send program and generator
geometry fix all proof offsets. The receiver-Send projection reconstructs both own
IPA generator endpoints from their transcript challenges. In both forms the payer
checks the signed receiver head and the canonical membership opening under that head.
A decoder supporting only `KGSCST3` rejects the Send prefix. A receiver can supply
the original released Receive to produce the `KGSCST3` form.

The Native `credit_status_for_payment` entry accepts the issued Request ID, the
released receiver head's operation ID and the complete original Payment. When
the head is a Send, Native reads the protected credit map at the root of that
Send and verifies the resulting evidence before returning it. Payer verification
uses the original locally released Send and its retained Request. For a raw Send
whose capsule predates a stored peer action, Native reconstructs the Request from
the capsule and the supplied canonical account companion. The signed destination
binds that companion and the entire reconstructed wire must match. Without the
action or a supplied companion, Native reports unavailable custody data. Both
forms are read-only. [Wallet wire §3.4 and §6](../../specs/kagemusha_wallet_wire_v1.md)

The canonical PIPA-R ordinary Receive proof has one public statement digest and a
3,360-byte descriptor at its one-lane k12 shape. Its state commitment absorbs 29 core
fields and the rest digest. Spartan adds the prior-credential root as the thirtieth
core field. The resulting predecessor and successor commitments, receipt binding and
proof verification would have to agree in a successor generation before an ordinary
step could support early durable credit. The existing sponge uses 16 permutations for
either 30 or 31 inputs with a folded domain prefix. This accounts only for the hash
site, and says nothing about the cost of verifying the step in a recursive Send.
[Ordinary proof shapes](../../crates/iroha_kagemusha_proof/tests/shapes.rs) ·
[Ordinary state](../../crates/iroha_kagemusha_proof/src/witness.rs) ·
[Spartan state](src/monetary_state.rs) ·
[Sponge padding](../../crates/iroha_plonk_gadgets/src/poseidon/sponge.rs)

An isolated Core30 candidate used the ordinary layout with a private prior-credential
root at index 8 in both head commitments. All five one-lane selectors fit. Receive
used 2,320 of 4,090 available rows at k12. Its descriptor and those for Send masks
0 and 2 yield 3,360-byte proofs. The two quota-enabled Send descriptors reach the
3,520-byte role limit. With local test keys, a fresh Core30 Receive produced a
3,360-byte proof in 1.928 seconds on four Mac workers. Full verification took
46.8 ms. The same proof failed verification against the ordinary Core29 statement
and against a revised prior root. A successor still needs authorized keys, a lineage
verifier for the ordinary step and hardware measurements of the complete operation.
[Five selector layouts](output/production-20261119/successor-ordinary-core30-five-selectors.log) · [Core30 proof and public-input checks](output/production-20261119/successor-ordinary-core30-receive-proof.log)

The lean Receive lineage schedule proves the recipient's transition inline in A1.
A2 verifies Q0, which checks the incoming Send under its selected key. A3 checks
the payer's proofs and objects, A4 checks the signatures, and A5 applies the
credit effects. Four continuation proofs link these stages before the final wrap.
The released A1 witness opens 29 core fields. In the successor experiment, both the
ordinary step and the inline transition take the prior credential root from the
same typed step witness. The inline transition used 2,479 rows and 9,746 advice
cells, one more cell than Core29. The strict checker accepted the two matching
Core30 heads and refused a changed root and either Core29 head. A witnessed
candidate without its root failed to synthesize. The complete A1 stage and the
remaining lineage stages still need matching authenticated keys and proofs.
[Witnessed A1 transition and step proofs](output/production-20261119/successor-witnessed-inline-a1-and-sigma.log) · [Receive stage plan](../../crates/iroha_kagemusha_proof/src/a_relation/receive/stage.rs)

A separate two-slot Q benchmark consumed a genuine Core29 k12 Receive and k14
Send. Synthesis, proving and self-verification produced a 10,720-byte intermediate
proof in 42.197 seconds. The complete test reached 1,162,936,320 bytes RSS,
including key setup. [Two-slot Q measurement](output/production-20261119/successor-canonical-two-sigma-consumer.log)

The lean schedule's incoming-only Q0 used a candidate Core30 Send with selector 5.
On four Mac workers, preparation took 5.353 seconds and proving with self-verification
took 27.227 seconds, after 20.788 seconds of key generation. The 7,136-byte proof
verified in 590 ms on a fresh parameter owner. Another fresh owner rejected a different
statement. With the shared step witness, a second run took 11.374 seconds to prove
and self-verify Q0, then 148 ms to verify on a fresh owner. This measures the
incoming Send check consumed by A2. The wallet still has
to prove and link the rest of Receive before the recipient can spend the credit.
[Fresh-owner Q0 verification](output/production-20261119/successor-lean-incoming-q-fresh-owner.log) · [Shared-witness measurement](output/production-20261119/successor-witnessed-inline-a1-and-sigma.log)

On the shared Mac, 20 four-thread ordinary Receive proofs using locally generated
k12 keys had a median proving time of 1.277 seconds and a p95 of 1.652 seconds.
Every proof verified. A separate one-thread process that generated the key, proved
and verified one sample reached 63,193,088 bytes RSS. These measurements cover the
29-field step alone. In the standalone Spartan relation harness the accepted fixture
produced a 2,240-byte unlinked proof in 59.265 seconds under load. That harness exposes
the incoming-verifier result as a public input. Neither benchmark measures a
spendable Receive through the installed wallet. [Ordinary step samples](output/production-20261119/successor-ordinary-step-receive.log) · [Memory measurement](output/production-20261119/successor-ordinary-step-footprint.log) · [Standalone Spartan relation](output/production-20261119/successor-spartan-standalone-cost.log)

Experimental Payment peer schema 3 uses a bounded-width Send statement and the
installed circuits' ordered generator-claim late slots. Each coordinate limb
uses 16 bytes. Each eligible endoscalar uses two 64-bit coefficient offsets.
A bitmap names words that require their full original representation. The
receiver reproduces the complete KGSPPR2 byte string before computing its
digest or deciding a malformed candidate. The IPM1 limit is 12,182 bytes,
with a 10,034-byte envelope and a 2,048-byte companion. The experiment measures
the complete packet against a 12,000-byte target. Native checks the size of an
honest Send against all three wire limits when it authenticates the programs.
Raw exceptions remain eligible for Burn while the entire envelope fits. For
the saved p18-s17 Payment, ten additional raw generator words fit in 10,022
bytes. An eleventh would need 10,038 bytes and is refused before custody.
The pair's first-phase preview commits the actual masked primary rows to obtain
the linking digest. It does not generate a discarded full proof. After the secondary
proof, Native drops its private circuit and witness before proving the primary.
Both paths replay the same first-phase entropy schedule.

The prover fills lookup inverses in batches of at most 16,384 slots. It clears transient denominators and inverses after each batch. The memory planner charges the in-flight slots for that stage. A private helper cell created as a zero by the builder is materialized when the prover fills it. Wiring-supplied values retain their original handling.

A prepared operation recovered and verified by a live engine can be reused at selection only with that engine, the same authenticated installation and the exact capsule. Selection still checks available memory, hardware debit approval and operation-specific admission. A fresh engine or installation performs full recovery. The Mac control preserved both original Payments across process restarts. [Control measurements](output/production-20261119/signed-v4-cold-control-and-restart.json)

## Scope and validation

Control exposes its Unload and RefreshTimeAnchor selectors in primary and secondary
inputs 62 and 63. Both sides use the shared pair link. Native verifies the exact
root-certified TimeAnchor original before Prepare and enforces the maximum of
the prior floor and signed issuer time. A cached signed response can update that
floor. A time-controlled Send separately requires the matching same-boot protected
direct-exchange observation. Swift and Kotlin expose `beginTimeExchange`,
`finishTimeExchange` and `cancelTimeExchange` on the admitted wallet. Native
takes both monotonic readings, checks the issuer, nonce, boot and committed
maximum response width, and publishes the observation through the Released
marker’s archive checkpoint. The checkpoint binds the installed manifest,
scheme and wallet. `captureSendTime` reads the committed anchor from the
released state and returns the interval for one operation. `abandonSendCapture`
retires an unused capture. Before selecting a Send, Native checks the frozen
interval and reads a fresh same-boot time against that operation’s original
anchor. A restart recovers the original direct observation from custody.
Controls-off Send uses its local floor offline. Phone qualification of a
time-controlled Send across process restart is still open.

The bounded future Unload verifier accepts an authenticated Control head, its
exact receipt and statement, the credential-bound AccountId, and any exact
RegulatoryPolicy quote, root-certified signer and beneficiary AccountId. It
verifies P, S, their link and all four accumulators before releasing sealed terms.
The future World catalog module `iroha.kagemusha.spartan.v1` provides three governed
instructions for installation, activation and Unload. The reserve's verifier manager installs all eight exact
program originals once under `iroha.kagemusha.spartan.programs.install.v1`.
World checks the scheme, asset, signed genesis, relation, Artifact signature and
independently selected `kgwparm1` manifest digest. Queue admission routes these
instructions to the registered reserve's dataspace. The genuine v4 fixture's
canonical World install is 5,668,814 bytes, including the 5,663,732-byte program
bundle. Governance must select the matching instruction and storage owners
before a reserve can install the profile or a wallet can activate or unload.

For a newly enrolled wallet, `iroha.kagemusha.spartan.activate.v1` carries the
selected scheme, asset, manifest, wallet and the bounded public Bootstrap claim.
The wallet payment key signs the distinct `kgwlctl1` Activate control for the
verified Bootstrap package. World checks the proof, signed credential and its
Enrollment issuer, requires the submitting account to own the incarnation, and
records the package digest with load ordinal zero. The first ordinary IssueLoad
can then debit the account. Exact activation retries preserve later ordinals and
closed wallets. Native binds certified activation outcomes to the retained
claim and exact signed transaction, including after a cold restart.

The typed `iroha.kagemusha.spartan.unload.v1` instruction verifies the whole
claim under those installed parameters before looking up its nullifier. World
atomically debits the reserve, pays face minus charge to the account, pays any
charge to its beneficiary, and records the claim and original transaction.
An exact replay returns the retained payout.

Swift and Kotlin `confirmUnload` take an operation ID and exact account-signed
transaction. Native binds them to the released claim, verifies the global
commit certificate and both transaction inclusion proofs, and retains the first
decision for each signed envelope. A rejected retry preserves an earlier
certified payout of the same claim. The selected archive retains the exact claim
and its evidence across normal capsule collection. Load and Unload use the
same installed genesis and protected committee history. The online SDK reads
one finality original, advances up to the caller's bounded number of certified
epoch boundaries and retries that original. A local `unloadSettlement` lookup
with an empty proof reads the retained outcome.

The generic SDK holds Unload
pending platform-enforced confirmation of the exact review and device acceptance.
[Current installation contract](../../specs/kagemusha_wallet_wire_v1.md#310-ledger-native-verifier-installation)
· [Runtime change rules](../../specs/live_runtime_upgrades.md#4-change-rules)
The production prover and proof-before-payout hold continue.

Native Unload Prepare and Resume return review version 3, semantic tag 6. They check
the credential account and the signed quote, beneficiary and branch before freezing
the capsule. Normal custody refuses unconfirmed Send and Unload. The non-shipping
device fixture separately installs its fixed public issuer and synthetic genesis
to exercise Send. A normal Install always keeps its protected debit boundary,
including in builds containing the fixture tests.
Only the isolated Rust test owner can produce a sample software-confirmed Advance.
The resulting completion retains its verified claim in output part 3. Part 1 stays
empty. Completion recovery checks the retained claim against the exact resumed
destinations, verified head, signed receipt and statement.

| Review bytes (end exclusive) | Value |
|---|---|
| 0–131 | u16 LE version 3, u8 tag 6, scheme, wallet, operation ID, capsule digest (32 bytes each) |
| 131–195 | Asset digest, Unload nullifier (32 bytes each) |
| 195–259 | Ordinal, face debit, online charge, net account payout (u128 LE each) |
| 259–355 | Signed quote digest, credential account digest, beneficiary account digest |
| 355–419 | Predecessor and successor commitments |
| 419–end | u16 LE credential account length and canonical AccountAddress payload, then u16 LE beneficiary length and canonical AccountAddress payload |

The full review is at most 16,384 bytes. A zero charge has zero quote and
beneficiary digests and an empty beneficiary address. A positive charge requires
both digests and a beneficiary. The account receives exactly face minus charge
and may receive zero when the charge equals the face amount. The account payloads
are canonical network-independent binary addresses. The native owner authenticates
their account digest bindings. A future protected approval must cover this exact
review before the nonexportable Advance key is used. World must select the installed
profile, scheme, asset and reserve, verify the whole claim before lookup, and apply
both destination transfers and the replay record atomically. The record binds
the nullifier, full canonical claim digest, amount and original transaction.


The heavy `complete_recursive_bootstrap_load_send_receive` test stabilizes all eight
programs, restarts a genuine Bootstrap→Load→Send→Receive chain, checks the complete
Payment size, imports the public programs and re-verifies, exercises NativeEngine
recovery/finalization, and proves malformed-proof Receive/Burn. Component proofs,
codec roundtrips, hardware signatures and bridge compilation are separate evidence;
none alone establishes a complete device payment or the 12,000-byte carrier target.
Physical validation must use the same complete profile with actual platform custody,
account admission, hardware keys, restart and exact-byte recovery.

The native containment suite checks both sides of irreversible selection.
Fresh preparation and selection of a restored, unselected Receive must refuse an
incoming Payment that cannot credit. When the fixture supplies a previously
selected receipt, cold recovery must complete the same outcome and record the
Burn. Each case starts from a proved Load100 and must subsequently spend 10 of
the existing balance, leaving 90. The suite covers matrix and generator claims,
proof encoding, group and scalar equations, and the Request signature. A valid
incoming Payment must credit.

The isolated device fixture collects real platform enrollment originals, then its
independent test issuer signs the future credential. It does not run the FI-side
attestation verifier before that signature. Its assertions cover native runtime,
account admission and hardware custody; they do not qualify FI enrollment policy or
device admission. The current device runner uses a software-key counterparty and measures the
physical wallet's return Receive in a fresh process. The signed Pixel 6 and
iPhone fixture packages have matching authenticated programs. Measurements on
physical devices still require their holders for hardware admission and payment
confirmation. The refreshed Mac process measured 82.943765 s and 2,311,028,736 bytes peak RSS for a complete Receive. Launch three recovered both exact Payments. [Two-wallet Mac result](output/takeover-20261118/control-native-v3/mac-two-wallet/result.json) · [Charged Unload review and retained claim](output/takeover-20261118/control-native-v3/charged/test.log)

The signed manifest-v3 fixture exercised 19 funded transitions and the zero-balance Bootstrap refusal. It also verified exact retained Payment bytes through both Refresh→Receive→Send and Receive→Refresh→Send. Reproduce the latter from this checkout with `KAGEMUSHA_SPARTAN_DEVICE_ARTIFACTS` set to the signed native-device bundle directory and `KAGEMUSHA_SPARTAN_RETAINED_PAYMENT` set to the associated wallet-A fixture’s `complete-payment.bin`. Run `cargo iroha-fast -- test --offline --locked --release --manifest-path experiments/kagemusha-spartan/Cargo.toml --target-dir experiments/kagemusha-spartan/target --lib native::device_fixture::control_lifecycle::refresh_around_a_joined_payment_and_immediate_send -- --exact --ignored --nocapture`. [Input hashes and command](output/takeover-20261118/control-native-v3/joined/invocation.json) · [Acceptance results](output/takeover-20261118/control-native-v3/result.json)
