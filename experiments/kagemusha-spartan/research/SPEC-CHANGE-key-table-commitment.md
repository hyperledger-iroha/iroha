# Spec change: issuer-certified key tables (2026-11-12)

Historical research record. See [the experiment README](../README.md) for current implementation status and production gaps.

**Status: applied in the same commit as this record (signed, on `optimizations`).**
- The owner chose the direction ("Issuer certifies them"), approved the draft ("Yes, draft it") and then the
  commit ("Apply and commit"), all on 2026-11-12.
- That commit's diff of `specs/` is the patch: `kagemusha_single_design_proposal.md`,
  `kagemusha_wallet_wire_v1.md`, `kagemusha_wallet_enrollment_policy_v1.md` and `kagemusha_lambda_omega_v1.md`,
  on top of `00fc09fcea` (credential continuity).
- Nothing implements the rule yet outside the experiment (see Effects and rollout).

## Why

Path 2 verifies a P-256 signature with a comb: two tables of 256 precomputed multiples of the key (m = 16), read
by lookups. G's tables are the same for every wallet and stay circuit constants. The payment key's tables differ
per wallet, and the experiment tried three ways to get them into the proof:

| How the payment key's tables reach the proof | Result |
|---|---|
| Constant columns (the experiment before 2026-11-12) | One circuit, and so one verifying key, per wallet. Not deployable. |
| Bootstrap derives them from the key (built: iroha `d21de32e26`) | Does not fit. P-boot needs 36 + 1 + 35 + 2 witness rows (686,653 cells) where Receive's layout leaves 35 + 5 + 21 + 3. Its matrix program grows from 5,672 to 18,310 products, which every Send and Receive fold evaluates natively. |
| **The issuer certifies their commitment (built: iroha `d39841bd20`; this draft)** | **One set of circuits for every wallet. Send and Receive cost the same as before: 2.21 s and 1.55 s per pair on the Pixel model, proofs 5,024 + 4,928 B. Bootstrap 1.05 s, once.** |

Verifiers deriving the commitment from the public key is not enough on its own. A receiver could check its payer,
but the recursion also relies on the tables of every earlier counterparty, whom later verifiers never see.

The current construction already names this mitigation. `kagemusha_lambda_omega_v1.md` R1 and G3.1 (P-256
variable key: measured at 1.47M cells against a 0.30M budget) list "precommitted own key" among the fixes. In this
draft, the issuer does the precommitting.

Logs (in iroha `output/kagemusha-100x-20261108/spike/runs/`): `claude-keytables-fp-2.log` and `claude-keytables-tests-2.log`
(the derivation and its measurement), `claude-walletrow-fp-1.log` and `claude-walletrow-tests-2.log` (wallet rows:
46/46 tests, 8-hop fixed point). The experiment's write-up is [`A2-UNIFORM.md`](../A2-UNIFORM.md), sections "Bootstrap's key tables"
and "Issuer-certified key tables".

## The rule

- **The tables** (wire §3.1, new row). For the payment key `Q`, the bases are `B_k = 2^(16k) Q` (k < 16).
  Table `t` (0 or 1) has 256 entries `T_t[e] = Σ_(j<8) (2 bit_j(e) − 1) B_(8t+j)`. Row `r = 256t + e` holds
  seven σ-field cells: `r`, then x's limbs (88, 88 and 80 bits), then y's.
- **The commitment**, `key_table_commitment`. `C = Σ cell_c(r) · G_(512c + r)` on Vesta, with no blinding,
  over the first 3,584 Vesta commitment generators of the scheme's relation. It is encoded in 64 bytes as `x ‖ y`,
  little-endian; in circuit it is four 128-bit limbs (io 56–59 in the experiment).
- **The credential carries it**, after `payment_key`, so the transcript grows from 468 to 532 bytes.
  - The issuer computes it before signing and checks it again before delivery (`verify_enrollment`).
  - The wallet recomputes it at E6 intake and rejects a mismatch.
  - A renewal keeps it: the replacement rule already lists the only fields a replacement may change.
- **The lineage carries it.** It is a public input of every lineage proof.
  - Bootstrap requires it to equal its credential's value, which the credential's in-circuit signature covers.
  - Every other step requires it to equal its predecessor's.
  - RefreshCredential also requires the replacement credential's value to equal it.
  - Every in-circuit signature under a payment key reads the tables from a table bound to that wallet's
    commitment: the wallet's own for its τ and Request, and the counterparty's Ω(pred) value for a counterparty's τ.
  - No step derives the tables or fixes them in a verifying key.
- **Native verifiers** take it from the credential they already verify natively for Ω's wallet, whose
  `payment_key` is Ω's. Ω's bytes do not carry it; Ω's credential digest binds it.

## Why it is sound

- **Same trust as the payment key.** The issuer already decides which key a wallet's signatures verify under,
  because it signs `payment_key`. It now also signs the commitment, which it computes from that key. The enrollee
  cannot influence the computation, and an issuer that could sign a wrong commitment could equally sign an
  unattested key. So the change adds work to the issuer, not trust in it.
- **Faults are caught before use.** Three checks run before the credential is ever used: the issuer's
  pre-delivery check, the wallet's own recomputation at E6 intake, and the shared vectors.
- **Binding.** `C` is a Pedersen vector commitment over hash-to-curve generators. Opening it to other cells
  needs a discrete-log relation among those generators. The tables are public (a function of the public key), so
  no hiding is needed and leaving the commitment unblinded is safe.
- **Carried unchanged.** Every step equals its predecessor, Bootstrap equals the credential, and
  RefreshCredential keeps the value. The experiment refuses each tampering it tried:
  - a changed limb of a predecessor's commitment, in every operation;
  - wallet A's Receive proof checked under wallet B's commitment;
  - a proof whose wallet row holds one wallet's tables under another wallet's commitment.
- **The comb is unchanged.** It runs the same check against the same tables; only their source changed.

## Costs

- **Credential: +64 B (468 → 532).** Every carried credential grows: the Request's receiver credential, the
  PayerCompanion, ledger claims and Activation. All stay well inside their caps (`CredentialV1` 1,024,
  PayerCompanion 2,048, Request envelope 10,034). Ω and the Payment frame do not grow.
- **In circuit.** The credential's message gains two 31-byte chunks (16 → 18): about one more Poseidon
  permutation wherever a relation recomputes it (Bootstrap, RefreshCredential, and a step that opens the credential,
  such as Λ_recv's prior-credential branch). Bootstrap also ties the four limbs to the transcript bytes.
- **Issuer and wallet.** About 780 P-256 point operations and one 3,584-term Vesta MSM per credential: the issuer
  at issuance, the wallet once at E6 intake.
- **Proofs.** Same size: the wallet row is a fixed row, so it is not in the proof. The Pixel model is unchanged.

## Open, not in this draft

1. **The issuer's and the root's keys.** Bootstrap and RefreshCredential verify the credential (Enrollment key) and
   its certificate (scheme root) in circuit, and RefreshPolicy steps verify RegulatoryPolicy and TimeAnchor
   signatures. Those keys need tables too. The experiment's Bootstrap still uses the wallet's own tables as a
   stand-in for them. A natural extension: each signer certificate carries its key's table commitment (signed by
   the root), and the root key's commitment becomes pinned scheme material, like `relation_id`. Not built or
   measured, so it is a separate decision.
2. **The generators.** The commitment uses the relation's first 3,584 Vesta commitment generators. The production
   artifact set pins how they are derived; the experiment uses `hash_to_curve` under `kagemusha-zks-spike/gens`.
   New generators mean a new relation, and so a new wallet generation.
3. **The construction spec.** The Path 2 construction is not specified yet: where the wallet row sits, the io
   positions, and the S side's pin. `kagemusha_lambda_omega_v1.md` describes the current PIPA construction, so this
   draft changes it only for the credential's new length.
4. **Encoding.** The draft uses 64 B uncompressed, like `payment_key`'s 65-byte SEC1 form, so Bootstrap ties limbs
   to bytes without decompressing. A 32-byte compressed form would save 32 B per carried credential. It would cost a
   square root in native code and an on-curve check in Bootstrap.

## Edits

`specs/kagemusha_single_design_proposal.md`:
- **R10:** the credential certifies the payment key's comb tables by their commitment (owner decision 2026-11-12,
  issuer-certified key tables).
- **§2.2:** the credential tuple gains `key_table_commitment`, with who computes and who recomputes it, and its
  being the same in every credential of an incarnation.
- **§3.2 Λ obligations:**
  - the credential bullet: a lean step carries the commitment by continuity;
  - a new bullet for the payment key's comb tables (the rule above);
  - Ω exposes the commitment, and its bytes do not carry it;
  - the zero state binds it.
- **§11:** a catalog entry covering vectors, a credential whose commitment does not recompute, a foreign table,
  predecessor, Bootstrap, Ω and replacement mismatches, and two wallets under the same verifying keys.

`specs/kagemusha_wallet_wire_v1.md`:
- **Status:** a paragraph saying the rule is specified ahead of the data model and ships with Path 2.
- **§1:** credential transcript 468 → 532.
- **§3.1:**
  - a new "Key-table commitment (inline, 64)" row: tables, cells, commitment and encoding;
  - `key_table_commitment(64)` in the credential transcript after `payment_key`;
  - its recomputation rule.
- **§3.2:** Ω's proof takes the commitment as a public input that the frame does not carry.
- **§5:** `key_tables` vectors.

`specs/kagemusha_wallet_enrollment_policy_v1.md`:
- **Issuance:** the issuer builds the credential with the commitment, and `verify_enrollment` recomputes it.

`specs/kagemusha_lambda_omega_v1.md`:
- **§5.1:** signed bodies are 108–532 B.

## Effects and rollout

- **Credential format change.** The credential transcript changes, and with it the wallet format. Wallet formats
  change only at a network reset, and Taira cannot be reset while the PoC runs (owner, 2026-10-24). So, like
  credential continuity, this ships with the Path 2 wallet generation, not into the running PoC.
- **Implementations stay as they are until then:**
  - the data model: the credential codec and vectors;
  - the issuer: the computation and `verify_enrollment`;
  - the wallet core: the E6 intake check;
  - the Kotlin and Swift codecs: the credential frame.

  The Path 2 circuits are already built in the experiment.
- **Vectors.** The credential's signature vector and pinned frames change, and the `key_tables` vectors are new.
