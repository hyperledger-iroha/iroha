# Experimental native backend

The experimental catalog has four recursive pairs. Bootstrap, Send and Receive
each have one pair. Control shares one pair across Load, RefreshTimeAnchor and
Unload. All use the future 532-byte credential body and the 30-field monetary Core. It is distinct from the installed
wallet generation. Installing its public programs does not migrate a funded wallet
or change a ledger, enrollment record, hardware key or journal.

## Installation and account admission

The eight public verifier programs contain verification instructions, constants and
geometry only. Private witness arrays and witness-producing closures are excluded.
Native verifies the Artifact-role signature on the version 3 parameter manifest
under its installed scheme root, then checks all eight program digests, fixed keys,
generator profiles and the ledger scope against its installed signed genesis.

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

The existing Ed25519 account signs a fresh Native challenge bound to the parameter
ID, selected slot, exact marker file and all four originals. Native rechecks that
same source and key before authorizing the wallet. No caller-selected slot, decoded
credential, signature-validity Boolean or account identifier substitutes for these
checks. Monetary calls remain unavailable until admission completes.

## SDK sequence

Swift and Kotlin expose the same stages:

1. `KagemushaWalletInstalledRuntimeV1.installSpartan` authenticates the public
   programs and transfers the same unadmitted owner to `KagemushaSpartanRuntimeV1`.
2. `beginAccount` returns the exact 32-byte challenge for the existing account.
3. `authorizeAccount` verifies its Ed25519 signature and returns
   `KagemushaSpartanWalletV1`, bound to Native's selected slot.
4. `prepare` validates the complete private operation witness and predecessor head
   and returns a review bound to one immutable capsule. The projection includes the
   operation, amount, fee, receiver, before/after heads and capsule digest.
   `prepareRefreshTimeAnchor` in Swift and Kotlin accepts the separately typed
   243-byte version 2 review for tag 7. The prepared fields include the exact
   signed anchor digest, issuer time, successor floor and both heads. Both SDKs
   bind the review to the same native capsule and Advance owner.
5. Before Send, the application completes hardware-backed biometric or device-
   credential confirmation of that exact review. `execute` advances that capsule
   through the canonical provider. No app-only PIN or approval Boolean is accepted
   as a substitute for the application hardware-confirmation contract.
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

## Proof and custody boundaries

Each complete pair verifies both scalar/point proofs, both accumulated matrix
claims, and both own and inherited generator claims. Native receives no trusted
"proof valid" flag from the application. Receive binds the original complete
Payment bytes: the total parser derives its canonical selection and the actual
incoming proof checks determine credit or Burn inside the relation.

Load requires actual canonical Sumeragi finality under the installed genesis reader
and exact ordinary Load receipt. Send prepares an irreversible amount-plus-fee
debit. The existing A0–A12 provider retains redundant frozen copies, selects the
head durably, signs using its persistent nonexportable hardware key, proves and
verifies the result, and retains the exact completion before releasing its bytes.
Recovery verifies retained completion bytes instead of generating new randomized
proofs for already released operations.

Proof masks and the pair-binding mask use fresh native entropy. Deterministic
streams are restricted to explicit test replay. Public program transport, malformed
proof parsing, and exact Payment serialization are separately bounded and canonical.
The pair's first-phase preview commits the actual masked primary rows to obtain
the linking digest. It does not generate a discarded full proof. After the secondary
proof, Native drops its private circuit and witness before proving the primary.
Both paths replay the same first-phase entropy schedule.

## Scope and validation

Control exposes its Unload and RefreshTimeAnchor selectors in primary and secondary
inputs 62 and 63. Both sides use the shared pair link. Native verifies the exact
root-certified TimeAnchor original before Prepare and enforces the maximum of
the prior floor and signed issuer time. A cached signed response can update that
floor. A time-controlled Send separately requires the matching same-boot protected
direct-exchange observation. Its durable capture for the future custody owner
is outstanding. Controls-off Send can use its local floor offline.

The bounded future Unload verifier accepts an authenticated Control head, its
exact receipt and statement, the credential-bound AccountId, and any exact
RegulatoryPolicy quote, root-certified signer and beneficiary AccountId. It
verifies P, S, their link and all four accumulators before releasing sealed terms.
Core still needs a World-installed future profile, atomic replay/transfer
integration and explicit release authority. The generic SDK holds Unload pending
platform-enforced confirmation of the exact review and device acceptance.
The production prover and proof-before-payout hold continue.

Native Unload Prepare and Resume return review version 3, semantic tag 6. They check
the credential account and the signed quote, beneficiary and branch before freezing
the capsule. Normal custody refuses Unload before selection and again at finalization.
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

The native containment suite additionally requires independent P/S matrix,
own/inherited generator, malformed-word, group/scalar-equation and Request-signature
failures to produce Burn within the admitted transport profile. Each case starts
from a genuinely proved receiver Load100, must preserve that existing spendable
value, and must complete a subsequent Send10 with 90 remaining. A valid incoming
Payment must credit and must reject a witness that tries to force Burn. These are
required full-chain checks; their presence in source does not establish a passing run.

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
