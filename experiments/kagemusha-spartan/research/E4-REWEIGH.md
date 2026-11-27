<!-- Written by Codex astra (gpt-6-astra, ultra reasoning), read-only, on 2026-11-18 at iroha 02a731de36, after the E4 step 1, F1 and F2 standalone measurements; prompt and log in ../../../output/kagemusha-100x-20261108/astra-e4-reweigh-20261118/ (local, git-ignored). Saved unedited. Supersedes E4-F1-F2-DESIGN.md's kind placement (§1, §2.1, §3); its F1 and F2 relation sections stand. -->

**KAGEMUSHA Path 2: E4 reweigh and recurring-kind decision**

**Target:** `experiments/kagemusha-spartan/research/E4-REWEIGH.md`  
**Review:** 2026-11-18, read-only. No files changed or tests executed.  
**Status:** Integration recommendation; proposed engineering is not qualification evidence.

**Decision.** Retain Bootstrap in the recurring matrix polynomial. Keep Receive and Send as separate programs. Replace Load with one fixed **Control program containing Load, RefreshTimeAnchor and Unload**, sharing one depth-32 recovery insertion.

Require the **hard anchor-signature check with a public dummy signer** for this candidate. Keep root-certificate verification total. Defer the certified TimeAnchor table commitment and its specification change.

Do not integrate E4 merely to free a corner. Its measured recurring cost is disproportionate to the Bootstrap term it removes. Retain its standalone evaluator and tests as research evidence.

| Recurring program | Kind `(io60, io61)` | Mode `(io62, io63)` |
|---|---|---|
| Receive | `(0,0)` | `(0,0)` |
| Send | `(1,0)` | `(0,0)` |
| Bootstrap | `(0,1)` | `(0,0)` |
| Control: Load | `(1,1)` | `(0,0)` |
| Control: RefreshTimeAnchor | `(1,1)` | `(0,1)` |
| Control: Unload | `(1,1)` | `(1,0)` |

Reject mode `(1,1)` and nonzero modes outside Control. These mode bits are ordinary authenticated inputs, **not additional MatPoly coordinates**. The catalog remains eight programs: four P/S pairs.

**Evidence and measurement boundaries.** The requested commit history and implementations confirm three standalone components:

- `af4cc93556`: `src/bootstrap_discharge.rs` implements the exact Bootstrap tape evaluator. Recursive transcript binding, key selection and claim routing remain unintegrated.
- `14e14627e6`: `src/unload.rs` implements the exact Unload relation, with recorded correctness and refusal tests.
- `32245ce8ae`: `src/control.rs` and `src/time_anchor.rs` implement one Load/Refresh relation. Neither operation is integrated as recursive Control.

The retained full-chain artifacts predate subsequent fixes. Their measurements are a baseline, not evidence for a newly built candidate. Seven files also contain uncommitted memory work: `engine.rs`, `kern.rs`, `prof.rs`, `proving_memory.rs`, `r1cs.rs`, `spartan.rs` and `uniform.rs`. This memo does not treat that work as committed or qualified.

The retained profile uses:

| Quantity | P | S |
|---|---:|---:|
| Constraint exponent `sx` | 22 | 22 |
| `row_bits`, `col_bits` | 5, 18 | 4, 17 |
| Accumulator bounds `(X,Y)` | `(24,25)` | `(24,24)` |
| Maximum challenge degrees `(dα,dβ)` | `(1,7)` | `(1,4)` |
| Cubic-fold degree | 186 | 174 |

These are the measured dimensions, not the smaller `FP_DIMS` defaults quoted in the original design.

| Existing kind | P constraint extent | P occupied layout rows | P tape instructions | S tape instructions |
|---|---:|---:|---:|---:|
| Receive | 2,773,423 | 32/32 | 3,578,138 | 232,478 |
| Send | 1,670,642 | 30/32 | 792,340 | 74,256 |
| Bootstrap | 414,240 | 31/32 | 190,556 | 879 |
| Load | 1,398,002 | 30/32 | 392,505 | 74,258 |

P-Receive has arithmetic constraint headroom but no free layout row. Those are different limits.

The F1/F2 censuses count raw relation rows and cells; their lookups generate additional helper rows and cells during compilation:

| Standalone relation | Raw rows | Lookups | Raw cells |
|---|---:|---:|---:|
| Load | 148,284 | 32,757 | 192,975 |
| Unload | 147,620 | 32,113 | 192,026 |
| Control: Load/Refresh | 391,162 | 646,163 | 804,022 |

Load compiles at `2^18` constraints; existing Control compiles at `2^20`. Raw relation cells are not a complete committed-cell or memory census.

Evidence is retained in `output/e4/fit-native-q-20261117T234223Z.log`, `output/e4/real-tapes-20261117T234109Z.log`, `output/f1/test-20261117T215657Z.log`, `output/f2/control-final-20261117T230531Z.log` and `output/s2/chain-20261117T051751Z.log`.

**Placement comparison.** All options require exact authenticated program selection, both linked proofs, inherited and fresh matrix obligations, and own and inherited generator obligations. None permits deleting a Bootstrap obligation after a native-only check.

| Option | Per-proof circuit consequences and likely geometry | Soundness-specific requirement |
|---|---|---|
| **(a) Original E4:** cold Bootstrap; recurring Receive, Send, Unload, Control | Every recurring P proof gains approximately **678,470 constraints and 1,178,112 committed cells**, plus F1/F2 integration. S gains 3,318 constraints and 5,898 committed cells. P `row_bits=6` is likely. Final geometry **UNVERIFIED**. | Authenticate the cold key and discriminator; bind the exact predecessor transcript; discharge fresh claims on **both** sides; prove inherited cold claims empty; retain Bootstrap’s own IPA generator obligations. |
| **(b5) Five kinds:** Receive, Send, Bootstrap, Control, Unload | No evaluator tax. New Unload combines a one-predecessor recursive wrapper with its 147,620-row/192,026-cell relation. Control replaces Load, adding 242,878 raw rows, 613,406 lookups and 611,047 raw cells before optimization. Selector/key machinery changes every recursive verifier. Packed cells and geometry **UNVERIFIED**. | Extend MatPoly, three-bit key selection, pair binding and admission together; reject unused proof-kind corners. |
| **(b6) Six kinds:** Receive, Send, Bootstrap, Load, Refresh, Unload | Keeps Load small, but duplicates common recursive/operation machinery in separate Refresh and Unload programs. Standalone Refresh-only census and complete recursive cells **UNVERIFIED**. It may avoid Control’s combined packing pressure. | Same three-bit requirements; authenticate all six programs and retain every obligation. |
| **(c1) Send+Unload; Control=Load+Refresh** | Eight programs, but the Send program pays for its Unload mode on every Send proof. Exact added rows/cells **UNVERIFIED**. Sharing a generic insertion is possible, but Send currently uses pending/fee maps rather than the Load/redeem recovery map. | Receive’s payer must prove **Send mode**, not merely the shared program kind. Gate Send-specific request, fee, quota and lifecycle rules without weakening Unload or Send. |
| **(c2) Recommended: Control=Load+Refresh+Unload** | Eight programs. Share state openings, credential continuity, statement digest, own receipt and recovery insertion. No large operation gadget is added to Receive or Send. Target the existing geometry; exact compiled rows/cells and fit **UNVERIFIED**. | Bind the three modes throughout P/S, statements, receipts, native preparation and acceptance; enforce each mode’s complete transition and preservation rules. |

The shared insertion accounts for **108,030 rows and 111,054 cells** in standalone Unload. Its joins, checked arithmetic, nullifier and redeem-value computation account for only **2,238 rows and 2,252 cells**. Therefore adding all 147,620 rows to Control would substantially overstate the necessary duplication.

A useful planning expression is:

```text
raw_rows(Control3) ≈ 391,162 − 80,000 + δ
```

Here `δ` includes the Unload-specific work and the new selection, statement and preservation constraints. Its exact value, cell count and lookup count are **UNVERIFIED**. This expression is not a compiled constraint count and does not establish a return to Load’s `2^18` geometry.

**Native q and program costs.** For the current cubic fold,

```text
D = 3·(selector_bits + X + Y) + 9 + 3dα + 3dβ
```

A third selector bit raises P/S degrees from **186/174 to 189/177**. Coefficient storage remains within 256 entries.

Crucially, `deferral.rs::restrict` evaluates each matrix tape on its own degree-sized grid before extension. Existing grids are 163 evaluations for each P tape, 148 for recurring S tapes and 139 for S-Bootstrap. A third selector bit does **not** proportionally multiply the old tape work.

Let `W(K)` denote one kind’s tape instruction evaluations per fold on one side. Let `W₀` denote the existing four-kind total. The following expressions assume unchanged geometry and exclude changes to other tapes caused by integration:

| Option | Native-q work model | Programs and artifact consequences |
|---|---|---|
| E4 | Recurring Receive + Send + Unload + Control, including evaluator matrices in each | **10 programs.** Measured evaluator fixed values alone add about **18.295 MB per recurring P program**, or **73.18 MB across four**, before other changes. |
| Five kinds | `W₀ − W(Load) + W(Control2) + W(Unload)` | **10 programs.** Adds a recursive Unload pair and enlarges Load into Control. Final size **UNVERIFIED**. |
| Six kinds | `W₀ + W(Refresh) + W(Unload)` | **12 programs.** Additional common machinery participates in every fold. Final size **UNVERIFIED**. |
| Send+Unload | `W₀ − W(Send) − W(Load) + W(SendUnload) + W(Control2)` | **8 programs.** Enlarges two pairs. Final size **UNVERIFIED**. |
| Three-mode Control | `W₀ − W(Load) + W(Control3)` | **8 programs.** Concentrates new operation work in one pair. Final size **UNVERIFIED**. |

The measured baseline is **807,426,857 P** and **56,508,997 S** instruction evaluations per fold. P-Bootstrap contributes only **31,060,628**, whereas P-Receive contributes **583,236,494**.

The E4 estimate **807.4M → 777.1M** removes Bootstrap and adds evaluator work to the existing Receive, Send and Load tapes. It excludes Unload, Control and geometry feedback. S slightly increases, **56.51M → 56.85M**. Recorded sequential timings did not demonstrate a speedup. This is an instruction-work estimate, not an integrated timing result.

For the recommended placement, unchanged-grid P work would be:

```text
743,448,542 + 163·instructions(P-Control3)
```

plus integration-induced changes elsewhere. Every fold evaluates Control’s whole matrix tape, regardless of the current semantic operation; inactive modes cannot be skipped.

The eight retained artifacts total **69,457,799 bytes**. Receive’s pair accounts for approximately 47.51 MB, Send 11.69 MB, Bootstrap 3.54 MB and Load 6.71 MB. None of the proposed catalogs has a measured final size.

**Payment budget and memory.** The last measured complete Payment is **11,597 B**, leaving **403 B** below the experimental 12,000 B limit.

| Option | Unchanged-profile byte model | Admission assessment |
|---|---|---|
| E4 | Its estimated geometry effect alone gives **11,787–11,822 B**, leaving 178–213 B before remaining integration effects. | Adds substantial recurring witness/fixed data and probably enlarges P geometry. Highest immediate memory risk. |
| Five/six kinds | One extra late accumulator coordinate per side gives **+64 B**, nominally **11,661 B**, leaving 339 B. | More installed programs and new verifier machinery; per-proof and native-operation peaks **UNVERIFIED**. |
| Send+Unload | No added accumulator coordinate; nominal baseline bytes if geometry holds. | Enlarges the ordinary Send proving path as well as Control. Peaks **UNVERIFIED**. |
| Three-mode Control | No added accumulator coordinate; nominal **11,597 B** if mode IO is reconstructed and geometry holds. | Avoids E4’s recurring gadget and additional program pairs. Control’s peak and whole-operation admission remain **UNVERIFIED**. |

These are models, not measured complete carriers. S ordinary IO must grow from 62 to 64 for the proposed modes; projection, reconstruction and transcript changes must be included. Serializing those two fields directly would add 64 B.

A P `row_bits` increase from 5 to 6 changes `sz` from 24 to 25, still within the existing accumulator bound `Y=25`. It need not add an accumulator coordinate, but increases the inner sumcheck schedule, changes tape grids and doubles dense witness geometry. Final byte growth must be measured.

Committed memory improvements in `2b3671cae3` recorded:

| P proof | Accounted peak |
|---|---:|
| Receive | 869 MB |
| Send | 432 MB |
| Load | 355 MB |
| Bootstrap | 345 MB |

S peaks were 105–216 MB. **0.68 GiB is approximately 730.14 MB**, so P-Receive already exceeds admission by about **139 MB**. Recorded whole-process RSS reached approximately **2.94 GB**. Per-proof accounting excludes costs such as other resident circuits, compiler scratch and separately budgeted MSM workspace.

Consequently, **no option currently passes native memory qualification**. Three-mode Control minimizes new structural pressure; it does not solve E3. Native-q evaluation, artifact residency, preparation, proving, verification and durable completion must all be measured.

**F2 lever decision.** Require the public-dummy hard anchor-signature construction for the first candidate.

`uniform.rs` adds a second-phase helper cell and row for each lookup. F2’s extra 613,406 lookups alone represent about **2.35 P rows** at 261,120 active columns. Load currently uses one second-phase row; Receive uses three. This makes lookup reduction directly relevant to retaining the shared layout.

Replacing total anchor ECDSA with the hard path is expected to save about 80K raw rows and substantially reduce lookups. The approximate lookup reduction inferred from existing components is around 170K; the integrated saving and packing sufficiency are **UNVERIFIED**.

Implement the optimization with constrained selection of **all** signature inputs:

```text
Refresh:      certified signer key, exact anchor message, exact signature
Load/Unload:  fixed public dummy key, fixed message, valid low-S signature
```

Derive and bind the comb table to the selected key. Keep root-certificate ECDSA total, and require certificate/opening validity in Refresh. The dummy requires no root-signed certificate because it authorizes no active operation.

Do not hard-check the existing all-zero `Authority::dummy()`, select only the signature, or leave the selected table bound to an arbitrary inactive key. Invalid inactive authority bytes must remain harmless; the same bytes must never authorize Refresh.

Defer certified TimeAnchor table commitments. The current signer certificate has no such field. This optimization would require coordinated changes to certificate encoding, signed contents, issuance, digest calculation, table verification, native consumers and fixtures. A supplied table or wallet table cannot substitute for it.

**Other constructions.** A separate Bootstrap accumulator fails the byte gate. At the measured dimensions it adds:

```text
32·((24+25+4) + (24+24+4)) = 3,360 B
```

giving at least **14,957 B** per Payment. A native-once Bootstrap check followed by omission remains unsound.

A possible later selector optimization uses one field-valued kind coordinate and a Lagrange basis over five or six valid values. It would nominally save 64 B against today’s two-coordinate accumulator, with degrees **192/180** for five kinds or **195/183** for six. Tape-grid work remains unchanged. However, it requires new finite-domain constraints, matching key selection and revised soundness accounting. Its geometry, memory, implementation and security qualification are **UNVERIFIED**. It should not delay the simpler four-kind integration.

**Concrete integration plan, in order.** Paths below are relative to `experiments/kagemusha-spartan/` unless prefixed with `crates/`.

1. **Record the revised decision and baseline.** Update `research/E4-F1-F2-DESIGN.md`, `TODO.md` and relevant `DESIGN.md` statements: E4 is no longer a prerequisite for F1/F2. Preserve standalone evaluator evidence. Identify the exact candidate revision, including separately completed memory work.

2. **Implement and validate the hard-anchor lever.** Change `src/time_anchor.rs`, with reusable glue in `src/signature.rs` or `src/certificate.rs`. Preserve total root authentication, exact message/SHA-256 binding, scalar canonicality and low-S. Test active authority refusals, arbitrary inactive authorities, dummy substitution and forged selected tables. Require witness-independent program bytes.

3. **Factor and share the recovery insertion.** Refactor `src/load.rs` and `src/unload.rs` to expose transition constraints and recovery descriptors separately from `indexed_map::append_insert`. Control should instantiate one insertion:
   - Load selects key `2^128 + load_ordinal` and its existing receipt-based value.
   - Unload selects key `2·2^128 + redeem_ordinal` and the redeem value.
   - Refresh selects a valid dummy insertion disconnected from state and preserves the recovery root.

   Bind old root, key, value and new root in each active monetary mode. Preserve standalone parity/refusal coverage.

4. **Build three-mode Control.** Extend `src/control.rs`, `src/statement.rs` and `src/receipt.rs`. With `u=io62`, `r=io63`, require Booleanity and `u·r=0`; enables are `1-u-r`, `r`, `u`. The semantic tag is `2+5r+4u`.

   Explicitly fix current two-mode assumptions:
   - Statement fields `[12,13]` bind verified burn/pending lineage for Unload; Load/Refresh keep them zero.
   - Unload permits quote digest `[21]`, with quote zero iff charge zero.
   - Unload materializes adjusted burn/pending into Core `[10,18]`; Load/Refresh preserve those private fields.
   - Enforce each mode’s exact preservation set. Merely exempting the union of changed fields is unsound.

   Preserve checked spendable arithmetic, face-only offline debit, counters, nullifier, receipt binding, Active/Retiring behavior and Refresh’s exact maximum.

5. **Integrate recursion and the authenticated profile.** Update `src/full.rs` operation types, public sources, P/S builders and fixed-point construction. Retain Bootstrap in `src/deferral.rs`; retain four-key selection in `src/points.rs`. Bind both modes through the pair link and pin them outside Control. Keep Receive’s payer restricted to semantic Send.

   Update `src/carrier.rs`, `src/payment_wire.rs`, `src/proof_bytes.rs`, `src/program_artifact.rs` and the manifest/bundle/install owners in `src/engine.rs`. Introduce an explicit new profile version; reject old or mixed interpretations. Preserve generator namespaces and all accumulator decisions.

6. **Integrate native preparation and recovery.** Extend `src/native.rs`, `src/native_witness.rs` and `src/engine.rs` through bounded encoding, wiping, preparation, retained witnesses, Advance, finalization and completion verification. Load retains native pre-Advance finality verification.

   Reuse direct-time custody in `crates/iroha_core_zk/src/kagemusha_wallet_state_v1/{session_custody.rs,preparation_custody.rs}` and `kagemusha_wallet_preparation_v1/refresh.rs`. A signed anchor updating the floor must not fabricate boot/monotonic observations or restart elapsed time.

7. **Census, freeze geometry and converge all eight programs.** Measure compiled constraints, present committed cells, phase/fixed rows, challenge degrees, tape instructions, artifact sizes and memory. Target the retained geometry. Require exact canonical equality between successive builds of each corresponding program, plus witness independence across all Control modes.

   If three-mode Control forces larger geometry while separately measured Control2 plus Unload fits the smaller profile, reconsider the **five-kind** alternative. Its extra 64 B may be preferable. Splitting is not automatically sufficient: F2 already creates most of the packing pressure.

8. **Complete proof-before-payout Unload integration.** Extend the future-generation claim/verifier path through `crates/iroha_core/src/kagemusha_wallet_v1/{artifacts.rs,ledger.rs,wsv.rs,tests.rs}`, `smartcontracts/isi/kagemusha_wallet.rs` and the canonical claim-model owner.

   Require installed World-owned verifier material, exact Control-Unload mode, both proofs and all deferred obligations, credential-bound account and authenticated quote/beneficiary. Verify before replay lookup, including retries. Atomically apply payout and a replay record binding the complete canonical claim. Preserve suffix-Unload refusal and exact-envelope recovery; rejection or timeout never restores spent offline value.

9. **Qualify the combined candidate.** Extend `src/chain_fixture.rs`, `src/backend_chain_tests.rs`, native/adversarial tests and custody fixtures. Cover all predecessor classes, all Control modes, joined histories, invalid incoming Payment containment, post-Burn spending, repeated Load/Unload, Refresh before/after payments, restart and exact retries.

   Measure complete uncompressed Payments and Unload claims, native-q work/time, program storage, per-phase accounting and whole-operation peak memory from the same candidate. Require Payment ≤12,000 B and proving admission ≤0.68 GiB. The experimental budget does not amend production’s 10,034 B limit. Existing G1–G3 conditions and independent review remain prerequisites.

**Acceptance position.** Proceed with three-mode Control and the hard-anchor lever as the integration candidate. Keep Bootstrap’s matrix obligations in the existing polynomial. Final bytes, geometry, native-q time, program sizes, memory admission and release readiness remain **UNVERIFIED** until the combined candidate passes these gates.