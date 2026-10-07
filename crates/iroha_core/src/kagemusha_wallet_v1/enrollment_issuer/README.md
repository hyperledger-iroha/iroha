# Native enrollment issuer boundary

`EnrollmentIssuerV1` owns an existing private attempt journal and the committed
`State`. It accepts a mandatory `EnrollmentIssuerRuntimeV1`; it provides no
permissive implementation or decoder that constructs a session from DATA.

`authenticate` requires actual Torii envelope authentication over the exact
dispatch body. Each operation provisionally matches the dispatch FI against an
exact configured provider route and compares the exact configured Scheme/app/enrollment/certificate/manifest/release/origin with the
dispatch. A consistent State view must also contain the account and matching
scheme registration, asset incarnation, scale and balance scope. Aliases and FX
corridor routes do not select enrollment authority. The selected provider independently checks
the account, actor and customer relationship; routing DATA cannot authorize an action.

Pre-key permit, new E5 or live E5 recovery, credential signing, and E6 delivery
each use a new signed provider observation. Its exact operation digest, request,
response and consumed nonce live in the single attempt record. Current routing
and configuration are read again after the response and immediately before
signer or active verifier invocation. The selected runtime must revalidate its
actual custody and generation at use. Missing/unknown results are not denial or
permission to retry a fresh verification.

The first E1, selected E5, captured verification time and retained E6 remain
unchanged. Live recovery can claim only the original prepared request after a
fresh provider check. After E1 expiry, `Inspect` can read a previously retained worker
result but cannot claim a row or verify evidence. Previously verified evidence
may still be signed and exact E6 delivered under fresh current eligibility.
Revoking an old app, certificate or route refuses recovery rather than replacing
its originals.

`[torii.kagemusha_enrollment]` is absent by default. Presence requires explicit
public policy/certificate originals, route pins, an existing journal and trusted
runtime handles. The eligibility policy is unsigned DATA selected by current
authenticated operator configuration. A bank provider supplies current bank eligibility;
a Scheme operator supplies its explicitly selected issuer/community/governance policy.
Neither a token classification nor Parliament approval is universally required.

TODO: connect concrete daemon authentication, current configuration,
HTTPS custody, private prepared-worker and rooted signer adapters, and mount the
closed methods on authenticated Torii routes. The unit runtime uses explicit DATA
and real test-key signatures to test ordering and refusal; it establishes no bank,
device, private-process or deployed-service qualification.
