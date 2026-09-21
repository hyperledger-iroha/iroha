# Complete component preparation and retained publication cleanup

This checkpoint closes a late-acquisition boundary in detached MV publication.
Previously, successful map preparation retained only the two native writers.
Publication still acquired the active-reader mutexes and publication identity;
a later World field could therefore contend after an earlier field had transferred.
Immediate per-field retirement could also call a retry waker under another field's
writers or the enclosing State fences.

`PreparedStorageWriters` now retains the exact native prepared current/undo
commits and identity guard before returning success. Every attempted acquisition
is nonblocking, and reader contention reports that reader's own release source.
An unchanged current map retains its original raw writer. `PreparedCellWriters`
likewise retains both EBR preparations and identity. Publication consumes those
owners without reacquiring a lock or copying a successor.

A successful publication returns the original retirement payloads, deferred
reader/writer/identity notifications and caller reservations. Cleanup follows
physical unlock. Prepaid published owners remain borrowed from their original
allocation scope; compile-fail coverage checks that neither preparation nor
published cleanup can escape it. Pair abort and abandonment release both physical
participants and identity before their local callbacks or payload destruction.
Caller unwind still records real mutex poison and requires recovery before fresh
reads; already-pinned snapshots remain valid.

The TriggerSet publisher retains all ten returned owners. Each prepared World
box stores its own published cleanup in place, and the original field vector
becomes `WorldRetirement`; no new per-field publication allocation is introduced.
Runtime publication retains all four Cell owners. The terminal State consumer
holds both returned retirements through its outer fences and Apply serialization,
including unwind after successful component transfer.

Regressions exercise actual native reader/identity exclusion, preservation of the
original writer through refused phase transfer, deferred pair callbacks, original
prepaid scope and reentry into all 278 World fields after a synthetic enclosing
fence releases. Existing current/undo, abort, source identity and reservation
controls remain in the focused selection.

Validation is captured under `dist/sumeragi-main-work/generation141-*`; the final
joined result is `dist/sumeragi-main-work/validation141.json`. Preliminary runs
are kept separately, including the initial admitted-scope test failure whose
fresh-read expectation was incompatible with actual poisoned prepared readers.
The corrected test retains its original pre-publication snapshots and explicitly
checks the recovery requirement. The World unwind control distinguishes an
EBR-only preparation cut, where all current/undo images remain readable, from
abandonment after map-reader preparation, where those reader mutexes are poisoned.
The signed retirement/replacement fixture also now establishes its configured
catalog through the actual fresh-start Kura/State constructor, instead of trying
to replace the immutable default baseline with runtime reconfiguration. Its previously
unreachable default-stack execution exposed overlapping fixture constructor
scratch: fixture construction and publication assertions now use separate helper
frames, as does the independent foreign State constructor. The first foreign-only
split was insufficient and its debugger trace remains in the capture. No test
stack override or assertion removal is used.

This is a component preparation and successful aggregate cleanup boundary.
Whole-State preparation still needs DA, lifecycle, cache and other effect locks
before any transfer. Whole-State abort/default-drop callback ordering, concrete
World/native allocation admission and the retained production Validate/Apply
cutover remain unfinished. This checkpoint does not qualify a full workspace,
a real four/seven-validator network, or L1–L6 completion.


## Membership and successful fence completion

Checkpoint142 extends successful retirement custody to transaction membership.
The admitted publisher swaps the original tip and predecessor identity into an
owned retirement, unlocks its writer, and returns the original notification.
The detached wrapper retains that cleanup with its installation admission.
Both State consumers retain it through their own commit fence, including
unwind after successful transfer; membership semantics and exact retry identity
remain unchanged.

`PublicationGuard`, the Kura lease and the Queue retirement cut can now unlock
all their physical participants while returning the original deferred signals.
The retained carrier preserves its route vector and State owner alongside
those signals. Its completion owner drops the commit guard before the original
State, Queue and Kura notifications, including when derived completion unwinds.
The signed retirement/replacement regression reacquires the actual State commit,
State write, full Kura lease and full Queue cut from its retry callback. Added
controls exercise all original fence notifications, retained membership tip and
identity allocations, standalone publication and detached publication, normal
completion and unwind. The formal ledger and mutation controls bind these
consuming owners and their release order together.

This does not suppress notifications from successor acquisitions. Derived work
that reacquires Kura or another backend while retaining commit serialization,
ordinary Apply's enclosing Queue observer, partial acquisition/advisory probes,
and whole-State abort/default-drop still require aggregate release ownership.
Membership's DashMap history mutation also still acquires shard locks and may
allocate during transfer. These are explicit remaining boundaries, alongside
complete effect preparation, production admission/cutover and network qualification.

Validation142 is recorded under `dist/sumeragi-main-work/generation142-*` with
its final receipt at `dist/sumeragi-main-work/validation142.json`. Intermediate
captures are separate: the first native contract binding pointed to Kura's root
instead of its defining module, and an initial runtime run used the incorrect
Kura test-module selector. All 227 tests in the subsequent intermediate runtime
selection passed; that source capture is not final because another completion
unwind regression was added during execution.
