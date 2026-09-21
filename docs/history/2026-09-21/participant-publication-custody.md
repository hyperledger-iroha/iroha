# Participant publication custody

The Native AMX prepublication token now retains the identity of its original Kura
instance and cannot be cloned. The existing durable writer is its sole production
constructor. Matching bytes in another Kura, including a reopened copy of the same
directory, cannot substitute for that original in-memory owner.

Live Apply rejoins the token, exact executed carrier, finality, manifest and ordered
State frontier projection under Kura's prune, canonical, geometry and sidecar
fences. The shared read-only check authenticates current durable finality even for
an empty participant manifest. For each nonempty route it rereads the manifest,
receipt and latest pointer and compares their complete identities with the
original token. A cached success cannot replace missing or changed durable data.
The existing per-route readback has a guarded inner operation so this complete
join does not recursively acquire a publication fence.

The live wrapper retains the existing blocking behavior and releases all four
fences before State staging. This avoids adding a new post-finality Busy result
that the current scalar Apply owner cannot retain. It is a current readback before
staging; it does not hold storage custody through State visibility. The equivalent
lease convenience method is currently test-only. The retained terminal publisher
continues to reject the old nonempty participant representation. This refusal
must not be removed to activate the new Native Decision path.

Five Rust controls cover three real participant routes, exact ordered frontiers,
foreign Kura ownership, missing/corrupt artifacts, release-driven contention,
strict restart/remint and empty-manifest finality. Mutation controls bind the
original identity, complete route loop, shared guarded reader, lock order and
readback-before-State-staging order. These are scoped checks, not evidence that
the full retained production handoff or real-network liveness goals are complete.

The accepted Native Decision path has a different owner: `NativeLaneStageSealV1`
retains its Decision batch, results and settlements after staging its own lane
application markers. Its source custody requires no external entrypoints, whereas
the old participant manifest is derived from external AMX receipts. The genuine
retained Native publication controls therefore require that old manifest to be
empty. Attaching an old participant token is not a prerequisite for this new
path. First-release completion must migrate the remaining scalar/recovery
consumers and retire the old representation, rather than introduce compatibility
execution or enable rejected MergeQC sources.

Checkpoint133 validation uses the Core library test executable, the production
Core library check, the complete Native/model mutation suites, reviewed source
and cache controls, and the canonical multilane source gate. Captured commands,
source hashes, exact test names and failed attempts are retained under
`dist/sumeragi-main-work/generation133-core/` and `generation133-formal/`; the final
receipt is `dist/sumeragi-main-work/validation133.json`. A passing intermediate
build does not qualify later edits. The concurrent external merge was resolved
before the successful Core test build; subsequent formal-helper/index drift is
recorded separately from Rust compiler inputs. This is scoped validation, not
full-workspace or live-network qualification. L1–L6, complete production resource
admission and unchanged real four/seven-validator qualification remain open.
