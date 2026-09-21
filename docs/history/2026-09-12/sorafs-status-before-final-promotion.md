# Historical SoraFS status excerpt

Preserved verbatim from `status.md` at source base
`a156eda165531852d97029d1a3f77a1ebfd5d29e` before the September 12
final-promotion checkpoint. These earlier observations are historical; named
transient artifacts are not asserted to remain available.

Excerpt SHA-256: `b9da4cb9a093e399b93a6f23c3075a2a4b99d5f4ed22869263bd28a9b3bb3ccb`.

SoraFS goal execution is tracked in the [V1 implementation goals](specs/sorafs/v1_implementation_goals.md).
The post-reboot manifest library now passes **908 tests**, zero failures or ignored
tests, including canonical identities/signatures under alternate caller layouts.
The new local result and binary/source hashes are retained in ignored
`target/evidence/sorafs-v1/manifest-reference-04-result.json`.
The retention-request model selection also passes three tests. Provider/rollout
source contracts pass 410 checks, with two unfinished-source closure failures.
The review corrects canonical manifest/deal/audit/replication identities, retention
request digests, pin-accounting keys, and node billing/reputation/governance
checkpoint and publication framing. The earlier full Node suite reports
**1,378 passed, 48 failed and two Kubo cases ignored**. The next rebuilt security
selection now reports **74 passed and four failed**, with zero ignored; all 48
original failures still pass. The remaining failures identify two unsafe fixture
file modes, a fixture assumption about intentionally hedged encryption randomness,
and insufficient cumulative quarantine decode budget at its exact byte limit.
Bounded corrections and the next full-suite execution remain pending.
The rebuilt Core SoraFS selection now passes **406 tests**, zero failures or
ignored, in 58.198 seconds with unchanged scoped source/binary hashes. It covers
all 42 earlier failures, the reputation policy cutover fence, exact permission
tokens under zero allocation, and authenticated snapshot timing. The combined
build still fails on Torii test include paths; this pass qualifies only the
captured Core selection. Node focused/full validation remains in progress.
The host reboot cleared previous `/tmp` SoraFS logs and interrupted native
captures; those older observations cannot serve as retained current evidence.
Matched daemon/harness four-validator
execution, full workspace/SDK validation, source/bootstrap seals, genuine HSM
custody and all deployment evidence remain open. See the
[current closure checkpoint](specs/sorafs/v1_closure_ledger.md#2026-09-07-post-reboot-checkpoint).
