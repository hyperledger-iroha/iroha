# SoraFS signing adapter validation

This is scoped validation of the integrated development source, not release or
deployment evidence. Authenticated software signing remains the V1 contract;
HSM access, hardware origin and non-exportability are not prerequisites.

The promotion checker, software signer launcher, topology qualification,
cosign adapter and final-promotion evidence suites passed **442 tests** in
4.44 seconds. The before/after source inventory and JUnit result are in
`target/first-release-sorafs-promotion-controls-20260921`. These suites cover
local contracts and adversaries; they do not supply completed production
operations, ready deployment summaries or independent approvals.

The actual cosign cryptographic suite separately passed **30 tests**, with
zero failures or skips, in 10.42 seconds. The installed v3.1.3 executable matched
the separately retained prior qualification pin
`77bbab240111761d50044f37541da0734d964dfe5f092cab6d584663c912372e`
before and after execution. The pin was read from the existing independent tool
record, not supplied by the candidate fixture. Tests exercise the public upstream
positive subject and altered subject, identity, issuer, signature, certificate,
timestamp, transparency and trust material. All observed current source inputs
and the executable remained unchanged.

The current result, source inventory, tool version, dependency inventory and
JUnit report are in `target/first-release-sorafs-cosign-crypto-20260921`.
The pytest log SHA-256 is
`9ff84fc544c8a1fd3f0684108186a52cdaebb66c8b4bdc152d3c00b2eb825fb4`.
One pytest cache warning reports an unwritable `/.pytest_cache`; it did not skip
tests or affect their result. Future invocations should specify a target-local
cache directory.

Public Sigstore conformance material is not a SoraFS promotion statement or an
operator's identity. No signing, publication or production promotion occurred.
The production provider assembly, all four inner authorization/completion
integrations, 17 signed readiness summaries, matching-candidate qualification
and independent audit/operator evidence remain open.
