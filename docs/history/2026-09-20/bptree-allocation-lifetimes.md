# B+tree allocation lifetime correction

All work stays in `/Users/takemiyamakoto/dev/iroha` on `optimizations`.
This corrects actual storage reclamation needed by the retained validation owner;
it does not activate that owner in the live worker or establish a memory budget.

## Problem and change

The locked Concread dependency allocates original B+tree nodes and retains their
nested key/value payloads through unpublished and reader-pinned generations. Its
partial-clone path marked the whole node invalid. On clone unwind, debug node
checks then aborted during destruction; without those checks, destruction skipped
all already cloned nested payloads. A regression against the old source reproduced
the abort. The corrected node always describes exactly its initialized prefix.
A cloned leaf key stays in an ordinary local until the value clone also succeeds;
only then are both installed and the prefix advanced. Branches advance their
prefix after each separator is installed. The old invalid-node bypass is removed.

Ordinary separator replacement also overwrote the previous initialized key without
dropping it. The clean regression observed zero drops where one was required.
Rebalancing mixed replacement with initialization of empty slots and performed
fallible cloning after destructive key transfers. Splits, merges and redistribution
now prepare the needed separator before those transfers. Existing separators move
with their child pointers; replacing an initialized separator destroys its previous
key. Clone failures leave valid ownership for the original writer's abort path.

The constructor audit found another escape: a new branch became a raw pointer
before debug verification called user key comparisons. A comparison panic leaked
that branch and its cloned separator before cursor registration. The constructor
now retains its Box until verification succeeds. The regression first reproduced
the leaked separator against the previous constructor, then passed with the fix.

This change preserves existing writer poison and reader-generation semantics.
It grants no permission to retry a poisoned writer, substitute another State or
Queue, release a generation at commit, or charge nested storage by encoded size.
Payload destructors themselves may still panic; generic credit ownership must
conservatively retain any charge whose actual reclamation is unproven.

## Evidence and limits

The local evidence lives under `dist/sumeragi-main-work/bptree111` and `bptree112`. The initial
clone fixture had one generic inference error; its corrected old-source run
reproduced a debug unwind abort. All four initial clone/owner regressions passed
after the prefix correction. The first separator-leak assertion poisoned its test
bookkeeping mutex during failure; the corrected assertion copies counters before
checking and independently reproduced the old key's missing drop. Failed attempts
remain separate from subsequent qualification.

Concread is a patched dependency, not a workspace test member. Its unit-test
manifest under that evidence directory points directly at the actual vendor source,
with the vendor manifest's feature/dependency declarations and a captured lockfile.
No copied source tree, workspace membership change or production dependency change
is used. Runtime MV/Core tests separately use the repository's original Cargo.lock.

The final combined source passes the eight-package test build in 192.32 seconds,
with all 7,142 captured compilation inputs unchanged. The captured Core executable
passes 171 publication/World controls and all 32 original review regressions;
its hash is `316e1eaa5dc270abd49fe226e600fa1f8f47f33f37274a7f2e8d141bdafd54b0`.
All 140 MV library/allocation/map-owner tests pass. The actual vendor source passes
246 tests with the production feature set, 121 B+tree tests with smaller nodes,
and nine lifetime regressions with optimization and debug assertions disabled.
The debug-verification constructor regression applies only when those assertions
are enabled. These configuration counts overlap and are not distinct-test totals.

All 212 retained-admission/geometry source-contract controls pass with their 9,358
captured inputs unchanged. The final canonical gate and source-manifest join live
in `dist/sumeragi-main-work/validation112.json`; the prior constructor-free source
and its separate outcomes remain in `validation111.json`. Runtime executables are
captured and rehashed, and the final join checks the actual checkout, complete
input inventories, branch, HEAD and index rather than combining historical passes.

The older installed nightly's AddressSanitizer stalled before Rust test startup
inside recursive sanitizer/dyld allocator initialization. Its sampled stack and
failed run are preserved; only that test executable was stopped. The installed
1.95.0 compiler's newer sanitizer runtime passes all ten lifetime regressions on
the final source. This developer-only run enables `-Zsanitizer=address` with
`RUSTC_BOOTSTRAP=1` and a separate target directory; production Cargo features,
toolchain and dependencies are unchanged. It does not replace the ordinary
production-feature or smaller-node-layout suites. Formatting uses Concread's
declared Rust 2021 edition.

Complete node/cursor/nested allocation custody remains required. Prepaid credits
must enter the original BptreeMap writer before its cursor vectors, cursor Box and
next-reader Arc shell are allocated. Node charges must survive publication and
retired reader chains until actual `Node::free`; dropping a charge field before
its enclosing Box deallocation is too early. The concrete funded validator, live
Validate/Apply handoff, Queue retirement cut, participant durability, recovery and
four/seven-validator fault qualification remain open.
