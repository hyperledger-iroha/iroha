# Final-promotion transaction-account custody V1

This source-coupled contract governs the account that submits native
[receipt-authority transactions](final_promotion_native_authority_v1.md). Manifest
role15 `FinalPromotionAccountTransaction` is Ed25519 and binds one deployment.
It signs only Reserve and Complete and has a distinct custody history from role14
receipt signing. Both native Check kinds use a separately pinned observer. There is one V1
surface; obsolete development state is reseeded without aliases or migrations.

## Native contract

`MutateSorafsFinalPromotionAccountCustody` carries `deployment_id`,
`expected_control_revision`, `expected_control_digest` and one action. Its
canonical wire identity is
`iroha.instruction.v1::sorafs::MutateSorafsFinalPromotionAccountCustody`.

| Action | Exact permission | Behavior |
| --- | --- | --- |
| Configure | `CanManageSorafsFinalPromotionAccountCustody` | Admit canonical Manifest policy, advance generations, retain permanent first-use key indexes and clear active enrollment. |
| Enroll | Same management permission | Authenticate independently signed custody against the exact committed predecessor; retain original enrollment bytes and native execution provenance. |
| Revoke | Same management permission | Set signer or attester revocation without cooperation from either key. |
| Check | `CanCheckSorafsFinalPromotionAccountCustody` | Check current custody, target and observer permissions without any state writes. |

Every token is deployment-scoped. Native execution independently enforces it even
during genesis. Management, observation and role14 Operate are distinct permissions.
Receipt Checks require the separate `CanCheckSorafsFinalPromotion` token; neither
Check permission grants operation rights. The production observer credential must
differ from both role14 receipt and role15 transaction keys.
All actions require the exact current control CAS; ambiguous retries inspect
retained history and cannot reuse a stale predecessor.

The target is always `AccountId::new(current_binding.public_key)`. It must be
registered and hold `CanOperateSorafsFinalPromotion` for this deployment. The
registered observer must hold the separate Check permission and **must differ
from that derived target**, even if one account has both tokens. Both direct and
assigned-role grants are evaluated at the same current State cut.

Check carries a nonzero challenge, native network identity, positive preceding
committed floor height/hash, exact expected target and nonzero reviewed transaction
payload digest. The digest is SHA-256 of
`iroha.sorafs.final-promotion-account.transaction-payload.v1\0` followed by the
complete canonical Norito `TransactionPayload` frame. Its sole computation belongs
to the prepared account transaction owner. A digest alone cannot establish valid
transaction grammar, deployment/action scope or permission to sign.

## History and observation

Core's sealed `signer_custody_history` owner serves exactly the receipt and account
purposes. It shares bounded canonical decoding, immutable revision/height and
first-use indexes, generation rules and staged transitions. Account rows use
`sorafs_final_promotion_account_custody_v1_<deployment-hash>` and the account
record domain; no operation IDs, audit heads, reservations or completion journal
exist in this namespace. Receipt active-slot invalidation remains with receipt
authority. Generic IVM state syscalls cannot access either protected namespace.

Both purposes retain 8,192 normal revisions plus two emergency revocations and a
32-KiB complete native instruction/record bound. Index collisions publish no writes.
Original issuance can precede native enrollment execution; current custody cannot
be used before that retained execution timestamp, even with a valid attestation.

The shared private `signer_check` owner authenticates exact signed External bytes,
aligned successful execution and the actual State/Kura committee lineage from an
independently retained floor through Check height H to applied cut J. It does not
mint custody authority. Separate purpose wrappers retain their independent binding,
CAS, observer, target, payload and challenge pins and evaluate eligibility at both
endpoints of one finite UTC interval on the same snapshot. Both accounts, current
grants and custody must still be eligible at J, including after a same-block revoke.
The original monotonic observation deadline is never renewed.

Initial endpoint checks use the original earliest observation time for both
endpoints, so interval width consumes the governed anchor-age allowance.
`recheck_use_interval` evaluates the same retained snapshot at both new endpoints
using that original earliest time and the unchanged monotonic deadline. It
rejects backward intervals and excess age; it neither samples a clock nor observes
later revocations. Every new phase still requires a fresh Check, and floor
persistence must be followed by a new independently qualified interval sample.

Both wrappers retain the full original floor separately from the applied cut,
and preserve the exact signed External and block hash at Check height H. Borrowing
those coordinates renews no deadline or custody eligibility.

The [shared native envelope owner](final_promotion_native_authority_v1.md#distinct-account-custody)
uses one 64-KiB complete-frame bound and one ordinary single-Ed25519 profile.
Unsigned preflight establishes structure and size only; the actual signed-entry
helper additionally rejects sidecars and verifies the signature. Account and
receipt Check preparation reject unsupported observer algorithms before I/O.
Exact reviewed payload, fees, action/phase and current custody remain independent
caller obligations.

## Remaining production integration

The daemon's `SignerFinalPromotionAccountPreparationV1` now owns the reviewed
statement, read-only original journal lease and exact Reserve/Complete preparation.
It requires an actual executed receipt Check, derives the native action, compares
the independent fee approval, and retains every payload field in the specified
SHA-256 commitment. Complete preparation validates all four signatures from the
actual staged receipt and keeps its file/lease pin alive.

The signing continuation accepts the distinct executed account Check and invokes
the configured key operation once. It keeps the signature private while issuing
and consuming an exact new account challenge after key I/O, then an exact new
receipt-phase challenge before release. All original observation bounds survive
these phases. The resulting owner supplies the same signed envelope for submission
and read-only ambiguous-outcome reconciliation. Four native/adversarial tests were
added for the continuation, Complete receipt pinning, payload/fee/role substitution,
prebuilt post-key proofs and incorrect returned signatures; their current-candidate
execution is pending.

`observer_transaction::FinalPromotionObserverTransactionsV1` now consumes the
original Core prepared receipt/account Checks while signing their exact observer
transactions. It pins both protected public bindings, a distinct Ed25519 observer,
and separately approved fee intent. It compares each Check's complete retained
binding, including provider/key references, revisions, policy and chain identity.
Before key I/O it compares the sole direct instruction, network, authority and fees
and applies the canonical envelope bound.
The key callback receives only that full immutable payload and its ordinary prehash;
its signature is verified before the original Core owner binds it. Deadlines are
checked before and after key I/O. The result is the existing pending Check, which
must still execute and pass the Core finality/current-authority consumer.

The account workflow constructs this observer owner from its retained original
receipt Check and account binding. Its native tests use the same observer path for
initial and post-key phases. Five additional tests cover exact executed payload/floor
retention, purpose/instruction/fee replacement, protected-key and signature
substitution, same-target binding replacement, and expiry during signing. Their
current-candidate execution remains pending. This is a signing prerequisite, not a configured observer service, aggregate
spending approval, submission adapter, qualified clock or durable floor authority.

TODO: Connect and qualify the authenticated software signer provider, observer
submission/fee path, approved spending journal, bounded UTC source and independently
retained floor store. This preparation and continuation do not implement those
deployment providers. No HSM or non-exportability requirement applies.
Completed-receipt recovery uses fresh observer-signed receipt Checks and approved fees,
current role14 custody and the original operator's current Operate permission. It
requires no protected key operation or new Reserve/Complete; current role15 key
custody does not gate release of a durably completed receipt.
Software fixtures and native execution tests establish no deployed signer custody,
clock, deployment, four-validator network or release readiness.
