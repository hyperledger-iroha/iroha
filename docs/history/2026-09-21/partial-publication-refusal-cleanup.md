# Partial publication refusal cleanup

Scope: normal returned errors in physical State/Queue acquisition, route
validation, and the live autoscale Queue scan. Work remains on `optimizations`
in `/Users/takemiyamakoto/dev/iroha`. This is scoped implementation evidence,
not release or network qualification.

The acquired guards previously released their wait notifications individually.
An earlier guard could therefore wake a callback while the enclosing Kura, State
or Queue still held another resource needed by that callback. The failed
acquisition's exact Busy observation must remain separate from notifications
for resources that the attempt actually acquired and released.

State fence acquisition and Queue cut acquisition now return their original
deferred release objects with the error. The carrier physically unlocks all
acquired State, Queue and Kura resources before dropping those objects. Failed
route checks retain the same Queue cleanup. The live autoscale validator also
unlocks the complete Queue cut before lifecycle callbacks on a completed scan.
No fallback, compatibility path, replacement event or second retry owner is added.

Four behavioral regressions cover partial State acquisition under an actual
signed retirement carrier, both contended Queue inner locks, route rejection,
and the actual autoscale service method. Wait callbacks try to reacquire the
original resources and verify the caller's enclosing locks have released.
Carrier identity, generation, exact Busy source and original retry remain checked.
Formal declarations and the binding ledger change together, with mutations for
early notification. The descriptor-drop mutation now moves its charge ahead of
the retained vectors, so it still exercises an invalid order after the earlier
service-field reordering.

The merged MV/Concread source also added fourteen preparation tests missing
from the maintained release selection. Both release scopes now require those
exact tests. Source-census parsing follows the nested module's closing brace;
root tests following that module no longer receive a false module prefix.

Validation artifacts are retained under
`dist/sumeragi-main-work/generation146-{core,formal,release-census}`. Earlier
failed compiler/gate attempts remain separate from final results.

Outstanding work includes partial Kura acquisition and cold sidecar cleanup,
callee-acquisition unwind, complete resource admission, the production retained
Validate/Apply cutover, and unchanged four/seven-validator fault qualification.
All L1–L6 goals remain active.
