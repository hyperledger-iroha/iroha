# Adversarial review of OPERATION.md (gate 1c), by Claude b04a57, 2026-11-08

Historical research record. See [the experiment README](../README.md) for current implementation status and production gaps.

Written by me from source. The planned Codex consult did not run: Codex reports "You've hit your usage limit
... try again at Nov 14th, 2026" (codex-b04a57-census-review.log). Scope: 3077ab's points (a)-(e), the
unbooked key-table lever, and anything else that makes 2.34 s / 3.38 s optimistic. Paths are relative to
`../` unless they start with `specs/` or `crates/`.

## Summary

| Item | Verdict | Effect |
|---|---|---|
| (a) Signature set | Matches the spec | none |
| (b) Permutation counts | Plausible; two credential openings missing | about +3k constraints each |
| (c) LogUp, committed helpers | Sound, if multiplicities and dynamic tables are committed before alpha | none if so; forgeable if not |
| (d) proof_digest over the transcript digest | Sound, if it also absorbs the final z1, z2 and binds both proofs | +1 permutation |
| (e) Wiring, matrix-MLE budget | Wiring looks conservative; 8,000 unverified until synthesis | open |
| ECDSA gadget, exceptional additions | Reachable 0/0 slope in the first window; no exception-freeness argument | about 0 to fix |
| Lever 1, per-wallet key tables | No flaw found; four requirements below | −56k Receive (3077ab's estimate) |
| New lever: sign the Poseidon message directly | Looks sound, but reverses an explicit spec rule | −26k Send, −52k Receive |

Net: the corrections cost about 1-2%, so the modelled 2.34 s / 3.38 s stand pending the Pixel commit
calibration. The two levers together would take Receive's own relation from 276k to roughly 170k constraints.

## (a) The signature set: matches the spec

- **Send proves only τ_send.** This is the spec's explicit rule: "Λ_send contains no check on
  counterparty-only data that can fail after commit. The Request signature and receiver credential are
  checked natively before Advance, and in-circuit only in the receiver's own Λ_recv. Fee terms are checked
  against the payer's own policy" (specs/kagemusha_single_design_proposal.md:917-919). Likewise, fee terms
  are checked "against the fee-schedule digest that the payer's own head-committed policy permits (Λ_send
  verifies no receiver-supplied signature)" (proposal:817-818).
- **Receive proves τ_recv plus the Request signature.** Λ_recv verifies the Request signature and, for a
  Request quoted before a renewal, the receiver credential (proposal:74 R10, proposal:810-812). That is the
  Receive+renewed variant.
- **Dropping τ_send from Receive is sound.** The Send pair proves τ_send in-circuit, and Receive verifies that
  pair recursively.
- **One spec-level observation, not a census error.** The effective accepted time is "at least the prior
  accepted time, anchored local time and authenticated Request time" (proposal:1831-1832). Send's proof
  therefore cannot authenticate the Request-time term itself. The receiver's time can only raise the payer's
  floor, and a colluding receiver could sign any time anyway, so in-circuit verification at Send would not
  add protection.

## (b) Permutation counts: plausible, with two small omissions

- The rate-2 counts check out: P(19) = 10, P(24) = 12, P(5) = 3, heads P(33 ‖ rest) = 17
  (operation.rs:1236-1243).
- The core holds every root that Send and Receive change (proposal:612-623). So Receive does not need to open
  the rest digest; carrying P(rest) unchanged is enough. Only Send opens it, for fee policy and time anchor.
- **Missing: the credential openings behind the account-digest checks.** "Λ_send checks the payer account
  digest against the payer's own credential, and Λ_recv checks the receiver account digest against the
  receiver credential it verifies" (proposal:804-807). The core holds only the credential digest, so each
  check hashes the credential body. That is about 10-15 permutations, roughly 3k constraints, for Send and
  again for Receive. Alternatively, put `account_digest` in the core.
- The 20-permutation rest-digest guess: even doubled it adds only about 5k to Send (3%).

## (c) LogUp with committed helpers: sound, with one condition to make explicit

- **Commit multiplicities and dynamic table entries in phase 1, before alpha.** OPERATION.md says "main rows
  are committed, then alpha and beta are drawn, then the helper rows", but `with_lookups` does not say which
  phase the multiplicities are in (operation.rs:1088-1090). If they were chosen after alpha, the prover could
  solve the single sum equation for them, and the argument would be forgeable. The R'/Q dynamic entries are
  main-row witnesses, so they are in phase 1 already.
- **Keep the sum row linear.** Use table helpers g_j = m_j / (alpha − t_j), checked as
  (alpha − t_j) g_j = m_j. Fixed tables give one linear row; dynamic tables need one product row.
- **"No extra proof bytes" is slightly off.** Phase-2 helper rows are extra Hyrax rows: 32 B each plus the
  next verifier's work, about 4-8 rows at 16,384 columns, and up to one row of padding per phase. The fixed
  point sizes rows from the witness including helpers, so this is probably already counted. Please confirm.
- Soundness error is about (lookups + entries)/|F|, negligible. Helpers are blinded like other rows, so zero
  knowledge holds.
- Rejecting GKR seems right on your numbers.

## (d) proof_digest over the pair's transcript digest: sound with one fix

- Today: `proof_digest = P_bytes(proof domain, LE32 len(Ω) ‖ Ω ‖ LE32 len(σ) ‖ σ)` over exact bytes
  (specs/kagemusha_wallet_wire_v1.md:1134). τ signs it, and the Payment digest binds the proofs through it.
- **The gap.** The spike's IPA absorbs L/R, G' and A, draws c and returns z1, z2 without absorbing them
  (src/ipa.rs:115-119). A digest of the final transcript state therefore leaves the last responses unbound.
  They are computationally unique (a second pair (z1, z2) satisfying z1·K + z2·H = A + c·P' reveals
  log_K H), but the digest should not rest on that.
- **Fix.** Absorb z1 and z2 of both proofs (one permutation). Bind both proofs of the pair, the statement,
  the version and the lengths. Require canonical decoding, which the wire spec already does. The digest is
  then an injective, collision-resistant function of the exact proof bytes.
- This is a wire-spec change to the proof_digest definition.

## (e) Wiring and the matrix-MLE budget

- **Wiring looks conservative.** With a beta-combined lookup of (address, limb0, limb1, limb2), one lookup
  fetches a whole non-native value. So a·b = r needs at most 3, and a point addition about 2 per check.
  Address uniqueness comes from preprocessed producer addresses.
- **8,000 per verified proof can't be checked until the verifier synthesis.** It must include the
  alpha-dependent entries of the lookup rows.

## ECDSA gadget: reachable exceptional additions

`pt_add` and `pt_double_add` use incomplete affine addition: λ·(x2 − x1) = y2 − y1 (operation.rs:462-524).
When the accumulator equals the addend, both sides are 0 and λ is a free witness.

**How it is reachable.** The prover chooses r, so R' can be any curve point with x(R') = r. It can be a known
multiple of Q, or (−16G − d''Q)/d'. Grinding s then fixes the top digits (about 1/256 per try), which makes
one of the first window's joins exceptional:
- `a1 + a2` (operation.rs:714);
- `+ gg(0,0)` (operation.rs:715).

Later windows: **my original claim here was wrong, and 3077ab corrected it.** I had argued that later windows
are exception-free because the G coefficient is a positive integer below n and the Q digit sum is nonzero.
That fails when the prover picks R' = aQ + bG with known a and b. A coincidence acc = T at window k is then
two linear equations in (a, b).
- It is not reachable at any step, though. r = x(R') feeds u1 and u2, and so the digits.
- To hit window k, the prover must grind s until about 16k bits of digit prefixes match, with about 64 (v', v)
  decompositions per s. That is roughly 2^(16k−6) work.
- So windows 1-3 are reachable and window 5 onward is not.
- 3077ab now pins λ at every addition (two chord equations, operation.rs `slope_checks`): 821 checks per
  signature instead of 709. That is the simple, auditable fix.

**Not demonstrated as a forgery.** I couldn't turn the free slope into a forgery: one free λ must meet both
final coordinates, which is a preimage of a random-looking function. Even so, the production gadget must rule
it out by construction. Today's gadget does: "no slope is a free 0/0 witness", with complete addition at the
joins and exception-freeness tests (crates/iroha_plonk_gadgets/src/p256/mod.rs:58-84).

**Fix, about free.** Start the accumulator at a nothing-up-my-sleeve offset point, or add two x-distinctness
checks to the first window (one product check each). Write the exception-freeness argument, with tests like
`variable_base_chain_is_exception_free`.

The stakes: own-key signatures are τ, the anti-fork custody receipt, so a malicious owner must not be able to
fake one.

## Lever 1: per-wallet key tables

I found no flaw in the idea. Requirements:

1. **Prove table(Q) once, in-circuit, at Bootstrap.**
   - Q is the payment key, fixed per incarnation; renewals keep it (proposal:810-812).
   - That is about 192 doublings for 2^32k·Q, 56 single-base odd multiples and about 1,024 pair-entry
     additions: roughly 1,300 point operations, or about 4k non-native checks, roughly 350k constraints.
     It's one-time, but Bootstrap's proof grows to about 2^20.
2. **Bind the table for every successor.**
   - The table rows must be dedicated, whole and unblinded. They are a public function of the public Q, so
     zero knowledge holds.
   - Carry C_table in the state. Each successor's verifier compares the proof's table-row commitment with
     C_table where Vesta points are native (the next secondary), crossing via the measured 83-constraint
     binding.
3. **The lattice step works.** Dirichlet gives 0 < |v'| ≤ 2^32 with |v'·u2 mod n| ≤ 2^224. Parity can be
   fixed with a ±1 adjustment and one extra addition. Keep the v' ≠ 0 check.
4. **Coverage and exceptions.**
   - All base signatures are own-key: τ_send, τ_recv, and the Request signature (the receiver's payment key).
   - Renewed issuer and root keys could use fixed tables in the verifying key.
   - Exception-freeness must be re-argued with the extra Q-multiple bases.

## New lever: sign the Poseidon message directly

- **The rule.** Each own-key signature pays one bit-level SHA-256 block (25,976 rows) only because the
  platform signer hashes m. The wire spec explicitly forbids no-digest modes (`NONEwithECDSA`,
  `kSecKeyAlgorithmECDSASignatureDigestX962*`; specs/kagemusha_wallet_wire_v1.md:95-100).
- **Security looks fine.**
  - Raw ECDSA's key-only forgery yields a signature on a random e. Here the verifier recomputes m from the
    transcript, so a forger would also need a Poseidon preimage of e.
  - Chosen-message power is the same in both modes, because the app chooses what is signed either way.
  - Domain separation stays in m.
- **Platform support is unverified.**
  - Android KeyMint supports Digest::NONE for EC keys, and I believe StrongBox does too; verify on devices.
  - The Secure Enclave signs caller-supplied digests through ecdsaSignatureDigestX962SHA256.
- **Saving.** About −26k constraints for Send and −52k for Receive; issuer and root signatures stay SHA-256.
- **Cost.**
  - An owner decision, since it reverses a spec rule.
  - New key-generation parameters, so it lands only with new keys or a new incarnation.
  - Updates to attestation and enrollment policy, the SDK adapters and native verifiers.
- **Alternative with no spec change.** SHA-256 with spread-table lookups instead of bit level, perhaps 2-3x
  fewer rows, paid for with full-width helpers.

## Other open points

- **Commit model.** The Pixel `commit-classes` run is built (aarch64, sha256 prefix 1e36ef6b, source
  fingerprint 7c4274e0) and waits for the owner's go.
- **Verifier synthesis.** If Path 2 keeps Λ_recv's burn branch for incoming objects (proposal:892), the
  verifier of the incoming Send pair must be total (soft verdict bits, as today's P-256 chip does), not
  asserting. That costs more than an asserting verifier.
- **Still excluded:** the preparation proof, witness generation, native verification and durable completion.
