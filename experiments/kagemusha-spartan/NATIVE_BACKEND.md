# Experimental native backend

This profile implements Bootstrap, Load, Send and Receive with the future 532-byte
credential body and the 30-field monetary Core. It is distinct from the installed
wallet generation. Installing its public programs does not migrate a funded wallet
or change a ledger, enrollment record, hardware key or journal.

## Installation and account admission

The eight public verifier programs contain verification instructions, constants and
geometry only. Private witness arrays and witness-producing closures are excluded.
Native verifies the small root-signed parameter manifest before decoding the program
bundle, then checks every exact program digest, fixed key, generator profile and
ledger scope against its independently installed scheme and signed genesis.

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

The four-program profile does not implement RefreshTimeAnchor. Controls-off Send
requires its canonical local floor and no network clock. A Send with time-dependent
controls requires a protected matching anchor; its absence remains unavailable.
Installing a caller timestamp or adding a time-response API cannot replace the
missing authenticated state transition.

The heavy `complete_recursive_bootstrap_load_send_receive` test stabilizes all eight
programs, restarts a genuine Bootstrap→Load→Send→Receive chain, checks the complete
Payment size, imports the public programs and re-verifies, exercises NativeEngine
recovery/finalization, and proves malformed-proof Receive/Burn. Component proofs,
codec roundtrips, hardware signatures and bridge compilation are separate evidence;
none alone establishes a complete device payment or the 12,000-byte carrier target.
Physical validation must use the same complete profile with actual platform custody,
account admission, hardware keys, restart and exact-byte recovery.

The isolated device fixture collects real platform enrollment originals, then its
independent test issuer signs the future credential. It does not run the FI-side
attestation verifier before that signature. Its assertions cover native runtime,
account admission and hardware custody; they do not qualify FI enrollment policy or
device admission. The physical self-transfer also remains distinct from the host
test's two-wallet chain and from a physical transfer between two phones.
