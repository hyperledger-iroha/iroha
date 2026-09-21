# Prepared pair abandonment and physical poison

Detached Storage publication held two separately dropping release guards.
Abandonment could wake a retry while the sibling writer was still held. A panic
in private payload destruction after physical unlock, or in the first native
wake callback, could also make the notification report a healthy lock as poisoned.
That turns recoverable local cleanup into a permanent refusal on a usable owner.

Prepared publication now owns both original writers in one private drop unit.
Its consuming abort and publish transitions take the same pair exactly once.
Default drop consumes both physical owners before either notification. A shared
release operation retains both original signals while the consuming callback
runs; ordinary tuple drop completes the sibling destruction even if the first
payload unwinds. No allocation, payload cloning, reconstructed journal or unsafe
extraction is added.

After both owners release, the operation reads the actual original mutex poison
states and freezes both verdicts before the first wake callback. Callback unwind
still delivers the second original notification. Payload cleanup after unlock
does not poison that released mutex; a sibling mutex released during an earlier
panic retains its genuine poison verdict. Pool refunds remain deferred until the
original allocation scope releases all its participating writers.

Two new matrix regressions cover ordinary abandonment, caller panic with locks
held, either private payload destructor panicking after unlock, and first-waker
panic. They check both physical writers are already available or genuinely
poisoned at every callback, both native verdicts agree with the mutexes, original
committed values remain, and healthy owners can be reused. The existing funded
capture test now also abandons two prepared pairs at full capacity with no
allocation or copying and verifies one refund wake only after the scope exits.

Both complete MV layouts pass 280 runtime tests and five doctests with no compiler
diagnostics. The source/artifact/runtime census retains all 272 HEAD baseline
cases plus the six prior capture cases and these two new cases. The release
harness passes 220 Python tests and 2,686 subtests; its required ownership census
is 264. Checkpoint138 captures use optimizations at
36f7cbf27661a46ccba2cf8e4bae53d321ecf1d2 with 7,275 exact Rust inputs.
Core compilation/runtime and final canonical source-binding results are recorded
separately in the checkpoint138 receipt; these MV counts do not replace those gates.

This closes prepared Storage abandonment, including the World publisher's
original Storage field owner. Ordinary executing Block abandonment and cleanup
across all State families remain separate boundaries. Publication still acquires
active-reader and identity locks after reattachment; all aggregate physical
checks must finish before the first field transfer. Concrete World/native control
policies, aggregate execution/restore admission, retained production Validate-to-
Apply and the shared Native consumer cutover remain open. No whole-workspace or
real four/seven-validator qualification is claimed; L1-L6 stay active.
