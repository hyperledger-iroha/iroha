# One-time Taira SoraFS repair

The network owner explicitly authorized this exception on 2026-11-07: initialize
SoraFS with all four validators, preserve all BPNG accounts and data, and publish
SORA Cars through the public gateway. This exception applies only to chain
`fc56984b-2be7-431d-840e-21514d1883f0` and its existing genesis-derived network identity.
It is not permission for a reset, snapshot substitution, general governance grants,
or changes to an already selected definition. Implementation is not deployment qualification.

The SHA-256 commitment of the retained owner authorization is
`7328a74eeb0f3ce22ab958d6434159507f4052e4a02914d0a0b75b914a1385c1`.
The implementation pins that commitment and the exact Taira chain/network identities.

## Certificate and boundary

`ApplySorafsRepairV1` has the new tolerant wire ID
`iroha.sorafs.bootstrap.repair.apply.v1`. Its certificate contains a canonical
`SorafsRepairPlanV1` signed independently by every member of the exact four-seat
committee scheduled for the executing height. The plan binds the authenticated
native parent height, core hash and result, exact predecessor and successor selections,
owner authorization, release/source closure, qualification evidence, operation identity,
expiry, and complete SoraFS payload. Swapped, repeated or missing signatures fail.

The certificate must be the sole instruction of a direct external signed transaction
on the universal root. A sealed reveal, contract call, batch or nested invocation cannot
enter this boundary. The new release recognizes this exact owner-authorized boundary
before ordinary envelope admission; this is the expressly approved exception to Parliament
authorizing this one successor. Ordinary transactions and old lifecycle instructions
retain their previous execution paths and layouts.

The boundary validates the native parent and scheduled roster, verifies all four
signatures, and computes the only allowed change set from the release catalog. It installs
the separately named `iroha.kernel.taira_sorafs_repair.v1` kernel plus the new bootstrap
interface/implementation in the same transaction overlay. The new implementation depends
only on the frozen baseline. It does not activate game partitions, change existing bindings,
change commitment/services profiles, or install/migrate existing storage.

The selected native handler then applies the certificate's exact payload and records
the complete certificate plus a once-only receipt. Normal signed-transaction admission,
resource metering and fees remain mandatory. Any admission, execution, callback, fee or
output-fit rejection discards the entire business overlay before the existing rejected-fee
path creates its separate overlay. A failed repair cannot publish a partial selection or
provider initialization. Ordinary fee changes must be explicitly included in signing review.

## Effects and preservation

Exactly three providers may be initialized. The payload specifies the admission council,
council-signed provider envelopes, existing owner accounts, exact allowlisted SoraFS
permission additions and gateway compliance-role members. Every provider-scoped grant
must name one of those providers. Existing initialized SoraFS authority, conflicting
owners, previously held grant roots, or an existing compliance role cause rejection.
Owner initialization adds only the signed owner binding, with no implicit owner
permission. New owner bindings referencing existing Musubi locations are rejected;
the repair does not refresh or change those locations or resolver projections.
No general Parliament, runtime-proposal, executor-upgrade or role-management permission
can be granted. No account, balance, wallet, controller, alias or contract is replaced.

The original threshold-key lifecycle remains its original three actions. There is no
`BootstrapSorafsV1` extension to it, no opaque parameter/metadata carrier, no runtime
configuration switch, and no second adoption import. The receipt and successor kernel
prevent reuse of this exceptional transition.

## Qualification before activation

Required evidence includes the exact signed source/release identities; all-four native
restore/replay of retained histories; fixed parent/selection/committee and exact payload
review; unchanged BPNG accounts, balances, wallets, controllers, contracts and history
except the declared SoraFS and normal fee delta; deterministic failure/rollback/restart
tests; copied-State execution without consensus voting; and native client decoding and
proof verification of the exact new instruction on a synthetic network. Production
validator keys must not sign a divergent rehearsal fork.
The older native checkpoint walker rejects this new wire ID; BPNG Core needs a qualified
checkpoint decoder and its matching finality pins before activation. Validator kernel and
handler support also require the matching executable. Pin changes alone add no decoder or
execution support. Phones use direct finality of their own entrypoint and output; these
checkpoint tests do not establish a phone Load/Unload failure or a mobile-distribution
prerequisite. No simulation or old-history replay alone proves compatibility across the
actual repair block. After activation, verify the actual
committed repair block and its successor with the qualified clients before publication.

Only after native provider ingestion, finalized receipts, verified retrieval and public
browser checks pass may the gateway URL be reported as a working SORA Cars deployment.
