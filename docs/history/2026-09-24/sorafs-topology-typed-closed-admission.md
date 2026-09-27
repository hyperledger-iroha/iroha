# SoraFS topology typed closed admission, 2026-09-24

Scope: the existing `optimizations` checkout. This is a first-release typed
role-16 instruction and fail-closed Core entrypoint. It is not native topology
authorization, finalized execution evidence, signer admission or promotion.

`MutateSorafsTopologyAuthority` has one canonical Norito V1 instruction and
schema identity. The topology transition reducer uses its typed completion,
revocation and closed outcomes. Core registers the instruction but returns the
initial native-instruction closed error before changing State for every tested
action. The role-16 signer and final-promotion checker remain blocked until
actual permissions, State history, completed operation and finalized Check
proofs are connected; the old detached topology envelope is not an alternate
authority.

Focused validation on this working source: DataModel instruction codec tests
passed 2/2; topology schema export passed 1/1; Core closed-dispatch/no-State-
mutation passed 1/1; the full topology reducer selector passed 24/24.
`git diff --check` passed. The separate SoraFS registry-audit selector was
attempted but did not execute because concurrent BFV source initially failed
the workspace `Copy` and fixed-array JSON derives. Those BFV compile issues
were corrected afterward; this note does not count the registry audit as
passed.

After the concurrent BFV compile repairs, the SoraFS initial-disposition
registry audit passed 1/1 against the current source. A follow-up private
role-16 Check proof binding passed 2/2 focused Core tests: exact signed
Check/floor/action/purpose binding and an actual finalized RS16/revision-4
topology Check whose closed Core result is rejected by the aligned execution
proof verifier. The first attempt at that failed-output test used a generic
test State with a foreign finality network and returned `Finality`; the final
passing test uses the fixed-network native Check fixture. No verified topology
authority result or public signer admission was added.

The signed topology Check currently carries the floor height and block hash,
while its context ID is supplied independently to the private verifier; the
role-16 instruction does not sign that context ID. A purpose-owned current
authority reader, native permission and retained State history, exact original
operation source, complete finalized signed-operation proof, configured
software custody, inner-approval verification, fixture regeneration and
production promotion remain open.
