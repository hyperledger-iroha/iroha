# Ordinary App Attest physical test support

`OrdinaryPersistentIdentityPhysicalHarnessV1.swift` provides callable comparison
support for an admitted iOS product test host. It contains no `XCTestCase`, test
method, installation factory or executed conformance result. Its code is present
only for physical iOS (`os(iOS)`, DeviceCheck, non-simulator). It uses the actual
SDK `KagemushaAppleAppAttestServiceV1`, pinned Apple enrollment verifier and opaque
Native C/E holders. The source must be checked by the owner against the selected
SDK/ABI25 closure before it is compiled or used.

The helper's `@testable import` accesses existing bounded SDK projections and
verifiers. A product XCTest target needs the corresponding reviewed SDK test
visibility. This file does not change package targets or product entitlements.

The product host must first obtain a genuine installed Native runtime and account
owner, the actual reservation and signed C, and the exact raw-attestation issuer
admission. Detached saved observations cannot create those capabilities. Supply
the actual product bundle/team identity, environment, admitted release and pinned
Apple root through `ProductAppAttestInputs`. Its initializer does not authenticate
the product's configuration; the existing verifier pins Apple's root and Native
independently admits policy, current time and issuer originals. The helper always
uses the public verifier initializer with its actual verification clock.

`collectNativeOriginal` calls the production App Attest collector. Native determines
whether generation/attestation is fresh or already retained, fences before device
invocation, and rejects unknown outcomes. The observation retains the exact C515,
full C512 transcript, key reference, full P-256 point and complete Apple attestation.
The existing pinned-root verifier checks that untouched retained evidence again;
this proves neither future key usability nor a server fraud/risk verdict.

Before possession, the product must durably anchor the dedicated app-private
assertion journal directory and bootstrap it through the actual pending holder's
`bootstrapNewAssertionJournal` while that holder is genuinely uninvoked. Reopening
must use the existing concrete `KagemushaAppAttestFileIntentStoreV1`; missing or
interrupted files cannot trigger a replacement. The helper never bootstraps,
resets or repairs a journal.

`proveAndObserveBoundOriginal` obtains E directly through
`identity.preparePendingAppAttestPossession()`. That existing production path joins
the same coordinator's C, raw314, point, key, pending scope and raw digest before
the helper calls the real Apple possession provider. The helper accepts no E DTO,
ticket or caller signing subject. Native method20 may establish the first genuine
pending E and otherwise selects its retained original. The returned possession
holder is that actual Native-produced capability, alongside detached observations.
The observation includes the full signed raw admission, pending scope, original
E424, exact assertion, actual counter and canonical Native possession receipt.
Enrollment possession remains distinct from a final app identity and monetary W.

`assertRecoveredOriginal` requires genuinely reacquired C and E holders and the
already existing assertion journal. It calls the production recovery/reconciliation
path, which may consume an already retained original assertion and advance its
matching durable journal. It never generates a key, attests, invokes an Apple
assertion, reserves E or sends HTTP. It compares the exact first originals and uses
the existing SDK assertion verifier, CBOR/DER and P-256 equation implementation.
Unknown invocations, missing Native originals, missing journal files and changed
key/scope/evidence remain errors. Reinstall does not authorize a replacement key.
App Attest offers no equivalent of Android's read-only Keystore entry/chain query;
these comparisons assert retained original custody, not present device-key usability.

`beginOrResumeInstalledEnrollment` delegates to an already opened production
enrollment actor. The host must have supplied actual
protected transport/persistence, installed owner, system App Attest adapter and
existing ledger wallet signer. The actor's public type alone cannot establish that
its dependencies are production adapters. `assertSameRetainedEnrollment` instead
requires a genuinely recovered completed Native retail holder and its actual
owner check, then calls the existing `recoverCompletedOriginals` API. This completed
path reads/re-admits the retained certificate without HTTP, E selection, device or
wallet signing. It compares FI enrollment ID, pending scope, app credential digest
and the complete retained FI certificate. These acknowledgements do not publish cash.

Apple's [server validation contract](https://developer.apple.com/documentation/devicecheck/validating-apps-that-connect-to-your-server)
distinguishes attestation from assertions and checks the App ID, environment,
enrolled public key and increasing assertion counter. The retained raw Apple
attestation, exact E assertion and Native receipt have separate roles here; local
comparison creates no issuer verdict. Apple's [key lifecycle guidance](https://developer.apple.com/documentation/devicecheck/establishing-your-app-s-integrity)
states that keys survive app updates but not reinstall, migration or backup
restoration. The helper therefore establishes no same-key signing recovery after
those events: the product needs authenticated recovery or rotation, with no silent
replacement. For an explicit `serverUnavailable` attestation error, Apple's
[attestKey contract](https://developer.apple.com/documentation/devicecheck/dcappattestservice/attestkey(_:clientdatahash:completionhandler:))
allows retry with the same key and client hash. This helper follows the current
Native WAL's stricter unknown-outcome refusal; it adds no retry policy or loop.

An actual product XCTest entry, device entitlement/release admission, installed
Native acquisition, supervised fault/lost-result injection, custody of the first
originals across process restart and reinstall, and device execution remain
unfinished. No physical pass, cross-app qualification, six-lifecycle completion,
offline monetary conformance, Guard/mint qualification or release artifact is
established by this source. Rotation, retirement, ordinary monetary approval,
initial Current/startup composition and competing offline spends require their
genuine production paths and separate owner qualification. Persistent ordinary
App Attest enrollment is not conditional on a single-use hardware guarantee.
