# Native Decision candidate and transport integration

Work remains exclusively in `/Users/takemiyamakoto/dev/iroha`, branch
`optimizations`, on HEAD `482a02fa708fb83f00b3cadd4067ad8f0b641b49`.
The shared lane reducer is still not constructed by the production runner.
This record describes connected prerequisites, not a live liveness fix.

## Candidate source and sizing

`NativeLaneDecisionHandoff::prepare_candidate` authenticates the complete first
admission and every required route Decision. Incomplete groups retain typed body
or Decision waits. Complete groups become a private candidate proof retaining
the original State and current publication observation. The process-lived
reducer keeps its Decision and Apply effects. The protocol aggregate cap retains
a strict first-admission prefix before the actual carrier budget is considered.

The existing `V2CandidateAssembler` now accepts that proof as its sole economic
input form. It counts native groups as useful work, binds their input-only batch
into the normal signed resultless block and RS16 payload, and fits whole groups
against the exact fully framed carrier before any private-key operation. Native
and admission prefixes are sized together: an oversized native group cannot
erase a feasible admission-only carrier. Deferred sources retain their original
owners. A final publication lease rechecks the exact State before signing.

The recorded native consumer currently refuses DA commitments, pin intents and
SCCP controls. Candidate construction now refuses the same unsupported controls
before signing. Their composition is an open production-activation requirement.

## Wire and physical transport

`NativeLane` and `NativeLaneDecision` have explicit BlockMessage tags and canonical
Norito framing. The control envelope rejects unsupported revisions. The compiled
transport keeps exact returned posts and reliable-admission tickets through
backpressure, independently services destinations, and distinguishes frozen lane
committee control from current global committee Decision dissemination.

Core and daemon topic, relay and diagnostic classification recognize these
messages. They do not enter the old lane signer: native Fair ingress remains
explicitly closed until the process owner and canonical consumer cutover is
complete. No compatibility engine or configuration toggle was added.

## Validation and retained failures

Build73 exposed three exhaustive daemon matches after adding the wire variants;
its combined build failed. A diagnostic run of its Core artifact passed two of
six selected controls and failed four candidate cases because the fixture used
lane keys for the independently frozen global leader. The fixture now uses the
existing authenticated global-roster key helper and actual block cadence.
This failed epoch does not qualify later source corrections.

Independent review found both unsupported-control signing and admission-prefix
loss during native fitting. Both have source corrections and explicit regression
cases. Combined build74 passed all six packages in 2m34s. On 7,136 unchanged
Rust/config/build/source-asset inputs, its native integration selection passed
18/18, candidate/message regressions passed 55/55, the ingress projection control
passed, and daemon relay/classification controls passed 28/28. The final canonical
multilane structural gate and serialization guard passed. All 67 selected Torii
controls also passed, for **169/169 runtime checks**. All three test binaries
retained their original digests. Formatting passed for 31 modified Rust files;
the daemon's modified hunks were formatted while unrelated existing differences
were preserved. Diff and historical archive checks passed. The aggregate receipt
is `dist/sumeragi-main-work/validation74.json`; individual outputs retain their
exact scopes under `native-candidate74-*` and `authentication74`. Full workspace
and network qualification remain open; 17 earlier Torii failures are still open.

Supplementary successor-source diagnostics fell from the previously completed
67 to 26 before this native integration. Focused owner controls passed 85/85, wrapper delegation 10/10, and the
consumer-owner run passed 28/29 before a mutation-target correction; that target
and six neighboring controls subsequently passed 7/7. This is not a complete
successor-source gate pass. Artifacts remain under
`dist/sumeragi-main-work/proof-owner-repair68`.

## Remaining production boundary

The native driver and transport must live outside the global-height reconstruction
loop. The current ordinary Validate/Apply service still drops or reconstructs
carrier preparation; production must retain the original canonical PreparedCarrier
through resource admission, durable output and publication. First-source recovery,
DA/pin/SCCP composition, exact native Apply acknowledgement, old fresh-signer
retirement and unchanged four/seven-validator fault campaigns remain open.
