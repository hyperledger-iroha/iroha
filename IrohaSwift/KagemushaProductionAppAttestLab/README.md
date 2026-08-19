# Kagemusha production App Attest capture lab

This non-shipping physical-iPhone app exercises the production App Attest path
without depending on Kagemusha proof artifacts. It calls
`DCAppAttestService.generateKey`, `attestKey`, and `generateAssertion` in that
order and exports the bounded raw Apple objects to its private data container.

The checked-in entitlement requests the `production` App Attest environment.
The runner fails if the signed application does not retain that exact
scalar entitlement. The embedded provisioning profile must authorize
`production`; Apple profiles that encode the unique bounded
`development`/`production` authorization array are accepted, while unknown,
duplicate, malformed, or development-only values fail closed. A
development-environment signed result is not accepted or silently promoted.

Construct the reviewed runtime policy before authority issuance with
`scripts/build_kagemusha_production_ios_policy.py`. It requires the exact
10-character Apple Team ID, at least one explicit bundle version and validation
category, and defaults to the repository's verified Apple App Attestation root.
The output is canonical, owner-private, and no-replace; move it into the
root-custodied release inputs only after independent review.

Run the app through `scripts/run_kagemusha_production_app_attest_lab.sh`. The
device selector and Apple development team remain process-only inputs. The
output is written to a new owner-private directory outside the repository and
does not retain a UDID, serial number, ECID, provisioning profile, or signing
credential. The device output filename is derived from the exact request
SHA-256, so a new run cannot consume a capture retained for an earlier request.

A release-bound run is deliberately two phase. `--prepare-only` builds and
verifies the production-entitled app, publishes its exact signed bundle and
canonical code-sign measurement to a private prepared root, and stops before
calling App Attest. The online authority snapshots that measurement when it
issues the artifact-bound challenge. The capture phase requires the same
prepared root, copies and remeasures that exact app before installation, and
remeasures it again after the physical call. A caller-supplied production
request is rejected without this prepared app and the exact production policy.
The prepared app contains its provisioning profile and is runtime material,
not retained promotion evidence; remove it according to the release operator's
secret-material policy after independent verification completes.

The production capture target and the benchmark host use the same explicit
App ID, `org.hyperledger.iroha.kagemusha.appattestlab`, because the production
policy and Apple's App Attest RP-ID bind that identity. They remain separately
measured executables; App Attest is never represented as an executable-hash
attestation.

The default request is a qualification challenge, not production-promotion
evidence. Its checked output uses
`iroha.kagemusha.ios.app_attest_qualification_material.v1` with
`promotion_eligible:false`, so it cannot be substituted for the production
platform-evidence schema. The same app accepts a caller-supplied canonical
request so a sealed release can provide its exact artifact-bound attestation
challenge and assertion template. The checker preserves the challenge's exact
evaluation time in release-bound material, but its standalone summary remains
`promotion_eligible:false`: only the full production validator may make that
decision after authenticating the signed candidate raw tree and the
independently operated online
freshness/consumption authority defined by
`specs/sdk/swift/readiness/kagemusha_production_ios_evidence.md`.
Release-bound capture writes `platform-evidence-v1.json`; qualification writes
`qualification-material-v1.json`. The former is passed to
`scripts/sign_kagemusha_production_ios_evidence.py`, whose no-replace signed
envelope is then consumed by the online authority.
