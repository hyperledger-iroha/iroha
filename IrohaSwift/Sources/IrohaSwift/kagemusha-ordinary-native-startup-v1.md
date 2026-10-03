# Ordinary Native account session

`KagemushaOrdinaryRuntimeStartupV1.openInitialAccount(storagePath:)` opens the existing SDK
Native coordinator and retains its actual opaque W/S original. The Native installer already
performs initial account acquisition for its independently registered ordinary runtime before
Core open. This wrapper does not call startup phases 1/6 again or parse startup archives.

The Native release owner must already supply the genuine installed runtime/account producer,
admitted package/source digests, released proof material, clock, storage and recovery state.
The factory accepts only the original storage path. It cannot install missing roots/accounts,
use an Experimental bootstrap, manufacture a hardware owner, or qualify a deployment.
Core open alone may select an existing other native provider on iOS; the mandatory ordinary
W/S read refuses an absent genuine ordinary session before this wrapper can escape.

Keep the returned `KagemushaOrdinaryNativeAccountSessionV1` for the entire product composition.
`originalCoordinator()` and `originalAccountSelection()` return the same retained SDK objects
after an exact same-owner recheck. Native still checks each subsequent operation independently;
returning a reference does not lock the entire caller's operation or provide financial authority.
Call `requireCurrent()` in the product's retained original-owner guard alongside its actual
account/runtime/bank/session guards. App-projected policy and saved public bytes remain DATA.

Any acquisition or recheck failure preserves its original error and closes the coordinator.
The session freezes before close, including failed/uncertain teardown. Repeated close cannot
repeat Native teardown. Dropping the session closes the actual coordinator even if another
consumer retained a coordinator reference; keep the session alive until all consumers finish.
An external close or an expired/replaced W/S owner is refused on the next recheck. This wrapper
offers no reset, replacement account, recovered owner constructor or automatic retry.
`refreshOriginalAccountClock()` explicitly renews the same installed account through Native
four-node acquisition and rechecks the retained selection. It accepts no caller clock.
Native first-open/unknown-acquisition/recovery rules remain unchanged.

There is no public session or selection constructor and no public endpoint callback. The
internal factory seam and draft XCTest fixture exercise refusal/lifetime control only. Fixture
W/S fields are not authenticated Native custody, canonical multisig admission or physical
qualification; these tests do not invoke a real provider or enrolled monetary lifecycle.

This session supplies neither C/E/FI enrollment nor offline monetary readiness. Thin product
integration still needs genuine Native C/E and recovery originals, independently selected
Apple release pins/provisioning, protected exact HTTP and durable CAS journals, and the existing
wallet signer. Qualified owner execution must cover real enrollment, restart, reinstall/state
loss, uncertain outcomes, retained recovery/refusal and competing spends through current FI,
State/Guard and monetary admission. Keep unsigned runtime, absent provisioning and every
Native/SDK refusal; an enrollment acknowledgement cannot bypass those gates.

## Incoming Mint and Receive

`KagemushaOrdinaryIncomingCashV1` composes the same retained session, original FI-control
transport, incoming lineage transport and `KagemushaAppAttestOrdinaryIncomingApprovalProviderV1`.
The public constructor accepts no W/S bytes, key/App ID, handle, proof verifier or money-ready
claim. Outgoing preparation holders remain distinct and outgoing-only.

The incoming holder reads Native phase19: operation, W, S, FI, platform, original alias/P256
key/key ID/credential digest, credential App ID and the independently retained platform counter.
Native obtains these from the existing Cash/financial owner and rechecks the originals. The
phase accepts only an existing operation and purpose; it cannot install or select custody.
Parsed projections are DATA and have no conversion to the opaque incoming signing holder.

Each W2/W1 uses its existing durable Native invocation fence before Apple, then retains the
exact raw assertion before its receipt advances the managed intent journal. Unknown invocation
outcomes freeze; recovery reads the same Native original and never calls Apple again. Proof work
uses Native's existing retained history, followed by genuine clock and FI refresh before the
next effect. Reserve/Commit HTTP retry reuses the exact original request, signature, proof and
request ID. The transport streams bounded Base64 chunks rather than duplicating full proofs.
StateAdvance and Ack remain Native operations; the returned hashes are historical correlation.

TODO: execute the complete incoming holder path against genuine Native W2/W1 Cash owners and
physical App Attest. Codec specimens, real P256 equation controls and scripted workflow tests
establish only their named local boundaries, not monetary, physical-device or release qualification.
