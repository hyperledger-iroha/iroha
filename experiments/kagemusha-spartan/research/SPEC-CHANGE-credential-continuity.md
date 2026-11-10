# Spec change draft: credential continuity for renewed Requests (2026-11-08)

Historical research record. See [the experiment README](../README.md) for current implementation status and production gaps.

**Status: applied in iroha `00fc09fcea` (signed, on `optimizations`, not pushed).**
- The owner approved the direction ("Yes, draft spec change") and then the commit ("Apply and commit").
- `SPEC-CHANGE-credential-continuity.patch` is the committed diff of `specs/kagemusha_single_design_proposal.md`
  and `specs/kagemusha_wallet_wire_v1.md`.
- Implementations still follow the old rule until they are changed (see Effects and rollout).

## Why

R10 (lean lineage, owner decision 2026-10-17) verifies a credential in-circuit once, in the step that introduces
it, and carries it by continuity everywhere else. One case still re-verifies: a Request that quotes the receiver
credential from before a renewal. There Λ_recv checks the issuer's signature (variable key) and the issuer
certificate under the root. Those two signed messages make the largest Receive variant 3.20 s in the Path 2 model,
because one circuit per operation type is sized for its largest variant.

| Renewed-Request handling (comb tables, lookup SHA-256, synthesized lean verifier, pairing link) | Receive's own relation | Send / Receive, largest variants |
|---|---:|---:|
| Today: re-verify issuer signature and certificate | 281,514 | 2.08 / 3.20 s |
| Issuer key over fixed comb tables | 236,093 | 2.08 / 3.09 s |
| Issuer keys as circuit constants (trust-anchor change) | 204,338 | 2.08 / 3.02 s |
| Continuity, last four digests in the core (bounded) | 172,018 | 1.94 / 2.52 s |
| **Continuity, depth-16 prior-credential array, root in the core (this draft)** | **175,834** | **1.94 / 2.53 s** |
| Continuity, the same array with its root in the rest digest | 180,370 | 1.94 / 2.54 s |

Logs: `spike/runs/zks-operation-v10-renewal.log` and `spike/runs/zks-operation-v11-continuity.log`.

With this draft, P-recv keeps its 2^19 class with 1.8% witness headroom at P 16,384 / S 8,192 (4.6% at
P 16,384 / S 16,384 for 2.66 s). Every Send and Receive variant is then under 3 s.

## The rule

- **State.** A new core field, `prior_credential_root`, is the root of the prior-credential array: a depth-16
  Poseidon tree indexed by `renewal_sequence`.
  - Slot `i` holds `P(kgwpcrd1, [credential_digest])` for the wallet credential with renewal sequence `i` that a
    RefreshCredential replaced.
  - Every other slot holds the padding leaf `P(kgwpcrd1, [0])`. Nodes are `P(kgwpcrn1, [l, r])`.
  - Bootstrap starts with the all-padding root.
- **RefreshCredential** (RefreshCredential's lineage proof) verifies the replacement credential and its certificate as today.
  - Continuity binds the same `wallet_id` and `payment_key`, and `renewal_sequence` is the predecessor's plus one.
  - It then writes the replaced credential's digest into its slot, which must be padding (17 hashes).
  - It refuses a replaced sequence of 65,536 or more.
- **Receive** (Λ_recv), when the Request's quoted receiver credential digest differs from the core's
  `credential_digest`:
  - recompute the quoted credential's digest from its bytes, as today;
  - open the array slot at its `renewal_sequence` (16 siblings) and require it to hold that digest;
  - check the receiver account digest against that credential's `account_digest`.
  - No signature is verified.
  - A quoted credential equal to the current one is bound by continuity, as today.
- **Native verification is unchanged.** Every party that consumes a package still verifies the credential
  natively (R10).

## Why it is sound

Every array entry was the wallet's credential when RefreshCredential replaced it. Its signature chain had been
verified in-circuit when it was introduced: by Bootstrap for renewal 0, by the RefreshCredential that installed it
for later ones. Continuity at RefreshCredential also bound each entry's `wallet_id` and `payment_key`.

So a quoted credential in the array was verified by this wallet's own lineage. That is the same assurance
re-verification gives, and the same principle R10 already applies to every other step. A credential that is not
in the array, sits at another slot, or differs by one byte gives another digest, so the opening fails.

## Receivability

Today a Request quoted before any number of renewals stays receivable (proposal §5.1). The array keeps that, up
to 65,536 renewals per incarnation.

The cheaper bounded window (last four digests) would make a Request quoted before a fifth renewal unreceivable
while the payer's debit stands. That is why this draft takes the array, for +0.01 s.

## Edits in the patch

`specs/kagemusha_single_design_proposal.md`:
- **R10:** Λ_recv accepts a pre-renewal receiver credential by prior-credential membership (owner decision
  2026-11-08, credential continuity).
- **§2.2 state:** the prior-credential root joins the state list and the core.
- **§3.2 Λ obligations:**
  - the credential bullet: in-circuit at Bootstrap and RefreshCredential; Λ_recv binds a pre-renewal credential by
    membership;
  - "every signature it owns": the Request only in Λ_recv;
  - the account-digest bullet: the receiver credential it binds;
  - the credential-continuity bullet: the array write in RefreshCredential's lineage proof, the membership check in Λ_recv, and the
    soundness argument.
- **§5.1:** a Request quoted before any number of renewals stays receivable.
- **Test catalog:**
  - a Request quoted before several renewals, received after them;
  - a quoted credential absent from the array, at another `renewal_sequence`, or with an altered byte (rejected);
  - RefreshCredential at the 2^16 capacity (refused).

`specs/kagemusha_wallet_wire_v1.md`:
- **Core:** 29 → 30 elements, with `prior_credential_root` after `credential_digest`.
- **Domains:** `kgwpcrd1` (prior-credential leaf) and `kgwpcrn1` (node).
- **Rules:** the prior-credential root is a nonzero canonical value. Bootstrap holds the all-padding
  prior-credential root.
- **§3.3:** a new "Prior-credential array" item (depth 16, slots by `renewal_sequence`, padding leaf, opening of
  16 siblings, write and refusal rules, test vectors).

## Effects and rollout

- **Every head hash** absorbs one more core element: +1 permutation per head, +2 per operation.
- **RefreshCredential's lineage proof** gains one array write (17 hashes). The renewed branch of Λ_recv drops two signed messages (about
  110k constraints) for one digest and one 17-hash opening.
- **The native data model** maintains the array like the other authenticated stores and pins its vectors.
- **Catalog counts** (22 routes, 16 programs) are unchanged. The renewed Receive route keeps its own branch, now
  a membership check. The current implementation needs its Λ_recv renewed branch changed and the registry data
  regenerated.
- **Rollout.** The core layout changes, so the wallet state format changes.
  - Wallet formats change only at a network reset, and Taira cannot be reset while the PoC runs (owner,
    2026-10-24).
  - This change therefore ships with the Path 2 wallet generation, not into the running PoC.
