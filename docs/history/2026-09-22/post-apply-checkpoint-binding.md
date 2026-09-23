# Post-Apply checkpoint identity

Post-Apply metadata now checks the network, height and block hash retained by
the same immutable `CapturedStateSnapshot` whose bytes it hashes. A mismatched
cut returns a typed local `CommitBoundary` refusal before hashing or writing a
checkpoint or commit manifest. It cannot become a deterministic body rejection.
The Apply caller retains its existing committed-recovery error classification.

Previously this caller discarded the capture's identity and paired its hash with
the requested height and subject. A normal fresh Apply already had an immutable
prepublication checkpoint, which rejected a different State hash. If that
checkpoint was absent, however, the metadata writer alone could not establish
that the captured State had actually reached the requested block. The correction
makes that association explicit before either durable write.

The capture owns bytes from one unchanged State generation. It releases its
State view before hashing and I/O; a later publication cannot relabel those
retained bytes. No commit lock is added around Kura I/O, and neither full-State
integrity check is removed or replaced with a copied prepublication hash.

## Regression scope

Four tests use the existing authenticated four-validator Apply fixture:

- Interrupt the real prepublication checkpoint write after block/finality
  durability, reject metadata completion while State remains unpublished, then
  complete the original Apply successfully.
- Apply an authenticated successor, reject persistence of that newer live cut
  for the earlier height, and retain the earlier captured hash unchanged.
- Reject a wrong block at the same height and a different network before any
  storage change.
- Repeat matching post-Apply metadata completion and preserve the exact
  checkpoint, manifest and their durable binding.

Negative cases compare the complete fixture Kura namespace and file bytes,
including absence of newly created metadata. They require the specific capture
refusal, so a later Kura error cannot mask a missing identity check.

## Validation

The Core/Config/Torii library test-target build passes with **7,581 unchanged
captured Rust, configuration and fixture inputs**, without compiler diagnostics.
The qualified runtime scope contains **263 Core tests**, including all current
snapshot and Apply controls and all 58 original review regressions; the **full
651-test Config library**; and **21 Torii admission/deadline/capacity controls**.
Every selected test passes, with no ignored cases.

The canonical structural checker passes with 12,345 unchanged captured inputs.
Formatting, the retired-codec guard, archived-history verification and both
working/staged whitespace checks pass. These are structural and scoped runtime
results, not fresh TLAPS or real four/seven-validator network qualification.

Exact selections, compiler artifacts, individual logs and the joined receipt are
under `dist/sumeragi-main-work/generation167-checkpoint-binding/`. The qualified
build is `checkpoint-build3`; the receipt is `checkpoint-verification.json`
(SHA-256 `e4bc7008c8a3422bbd292cbfb1dc1393a88757c6e90e26a95be63f98be21d7ae`).
The subsequent build4 captured the same input map, but its attempted receipt
update refused later source changes. The original passing receipt is preserved;
`subsequent-source-delta.json` records the later observed merge/source delta.
Neither build qualifies those subsequent changes.
The command is `scripts/cargo_fast.sh --stable-local-metadata --incremental --jobs 4 -- test -p iroha_core -p iroha_config -p iroha_torii --lib --no-run --locked --offline`.

The first Core recorder failed after all individual test processes finished;
its logs remain diagnostic. The corrected recorder reran all 263 controls.
Its raw source-window flag remains false because independent SDK/Torii changes
landed during that run. Reuse is justified by the final unchanged compiler
capture emitting the **identical Core executable**, with every source delta
retained explicitly. Config joins the same way; Torii runs on the final image.
No inferred process return codes replace the failed recorder's missing summary.

The independent DA suite on this candidate reports 67 passes and five failures
in visibility fixtures and an obsolete diagnostic expectation. That task owns
their correction and subsequent qualification. These failures are not hidden
by the 21 passing Torii controls above. The preceding append qualification also
remains bound to its own generation166 captured inputs; neither record qualifies
later concurrent changes or the complete workspace.

## Remaining publication work

This is a production metadata correction, not incremental State checkpoint
construction or activation of retained Native Validate-to-Apply. Complete State
resource admission, authenticated membership recovery/reclamation and the sole
production retained consumer remain required. Both complete-State checkpoint
traversals must still be replaced together under actual publication authority.

The retained publisher already consumes its original journals and checkpoint,
but its postpublication completion must also own the checkpoint-to-manifest
transition: the present manifest binder replaces the checkpoint file. An old
descriptor receipt cannot silently adopt an equal pathname after that write.
A future consuming completion operation must retain its actual writer result,
retry phase and original resources through partial success.

L1–L6 and the unchanged real four/seven-validator loss, restart and final-transaction
qualification remain open. No compatibility path is added.
