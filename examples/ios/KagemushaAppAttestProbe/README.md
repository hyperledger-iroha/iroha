# App Attest physical probe

This isolated iOS sample measures `DCAppAttestService` on a physical iPhone. It
creates a dedicated key, asks Apple for one attestation, and generates two
assertions against the same fixed diagnostic challenge. It discards the first
assertion before any consumer commit, then reports whether the retry advanced
the signed counter. The parser only reads raw sizes, flags, and counters; it
does not validate the attestation or authorize KAGEMUSHA value movement.

Use an Apple development team allowed to sign the app. With the App Attest
environment entitlement omitted, Apple uses the development environment for
a development-signed app. For distribution, use a registered App ID with the
App Attest capability and its approved signing profile. `attestKey` calls Apple
servers, so the phone needs network access. App Attest support is checked
on-device before key creation.

From this directory, generate the Xcode project with `xcodegen generate`, then
open `KagemushaAppAttestProbe.xcodeproj`. In Signing & Capabilities, select
your team and set the bundle identifier to your registered App ID. Or build
from the command line with your own values:

```sh
export KAGEMUSHA_APP_ATTEST_TEAM_ID='YOUR_TEAM_ID'
export KAGEMUSHA_APP_ATTEST_BUNDLE_ID='YOUR_REGISTERED_BUNDLE_ID'
xcodegen generate
xcodebuild -project KagemushaAppAttestProbe.xcodeproj \
  -scheme KagemushaAppAttestProbe \
  -destination 'platform=iOS,id=YOUR_DEVICE_UDID' \
  DEVELOPMENT_TEAM="$KAGEMUSHA_APP_ATTEST_TEAM_ID" \
  KAGEMUSHA_APP_ATTEST_BUNDLE_ID="$KAGEMUSHA_APP_ATTEST_BUNDLE_ID" build
```

Run the app on the physical phone. It starts the probe once on first launch;
the **Run probe** button allows a manual repeat. The screen and Xcode
console show redacted diagnostics. Raw objects and the key identifier are
retained in a fresh UUID-named, app-private, complete-file-protected Application
Support directory for offline inspection; they are not printed, backed up, or sent.
A `complete.marker` is written only after the whole diagnostic sequence, so
incomplete and separate runs cannot be mistaken for one evidence set. The exact
public test `clientDataHash` is saved alongside each complete run for checking
the documented signature equation.
No team ID or provisioning material is printed or saved by the probe.
Each run creates a new diagnostic key. The fixed challenge and simulated
lost-result retry are for local measurement only and must never be used as a
production enrollment or transition protocol.

Apple references: [App Attest workflow](https://developer.apple.com/documentation/devicecheck/establishing-your-app-s-integrity), [environment entitlement](https://developer.apple.com/documentation/bundleresources/entitlements/com.apple.developer.devicecheck.appattest-environment), [attestation validation guide](https://developer.apple.com/documentation/devicecheck/attestation-object-validation-guide).
