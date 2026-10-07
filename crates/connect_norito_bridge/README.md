# Native SDK bridge

`connect_norito_bridge` exposes the current ABI-26 C and JNI bridge to the SDKs.
The ordinary Rust target emits `cdylib`, `staticlib`, and `rlib` outputs; Rust
consumers and tests retain the `rlib` interface.

The Apple owner is `scripts/build_norito_xcframework.sh`. It builds five
`staticlib` slices using the source-sealed `apple-release` Cargo profile and
whole-graph ThinLTO. Its final bridge optimization level is O1 so the ThinLTO
pass does not reapply O3 to the broad data-model modules. Dependencies retain
the release profile's crypto O3 and data-model O1 settings. The build preserves
symbols and the existing unwind behavior.

Every slice retains authenticated PQClean normalization and complete-archive C
link checks. The native host check exercises ABI identity, SHA3/SHAKE, ML-DSA,
and ML-KEM. Static XCFramework layout, headers, exports, and authenticated ZIP
limits remain enforced by their existing owners. SwiftPM is the sole supported
Swift delivery path; qualify the packaged archive and an ordinary Release SDK
consumer. Profile selection alone is
not evidence that a candidate passes resource, SDK, or device qualification.

`connect_norito_domain_id_validate_v1` admits only exact canonical ASCII
`domain.dataspace` identities through the native pinned UTS-46 owner. Its 127-byte
input bound is checked before reading or allocating. SDK callers must retain the
provided spelling and reject missing native admission; this API does not normalize
Unicode input or provide a managed fallback.

KAGEMUSHA canonical wallet objects are owned by
`iroha_data_model::kagemusha::kagemusha_wallet_v1`. The bridge retains one native
runtime or admitted wallet per opaque handle. Trusted embedding-app initialization
calls `kagemusha_wallet_ffi::start_native_wallet` with independently deployed
`NativeInstallationConfigV1`, an authenticated native genesis owner, the signed
verifier pack/inventory, the exclusive Advance provider and its sole original-key
source. The loader qualifies the complete 52-route graph before registering a
runtime; no default trust root or environment-selected installation exists.

C/JNI `open_begin` accepts only bounded credential, Enrollment CertificateSet,
AccountId and asset-scope originals. Native reconciles the actual enrolled slot and
returns a fresh 32-byte account challenge. `open_finish` consumes that challenge
and the existing Ed25519 account signature; errors retain unadmitted custody for a
fresh begin. `open_cancel` abandons the challenge. Repeating finish after lost
output returns the same still-live admitted handle; closing never resurrects it. Internal registration failure
retains an initialized owner until promotion succeeds. No foreign digest, proof
verdict or replacement payment key can open a wallet.

Typed setup and lifecycle calls cover Bootstrap, Offer/Request, Credited archival,
direct time exchanges, Load, Send, Receive, policy refresh, Unload and Retiring.
Offer/Request setup outputs are canonical object frames; peer transports carry
`KagemushaWalletEnvelopeV1` frames. Typed native `envelope`/`original` helpers wrap or
extract Offer, Request, Payment and Credited with exact scheme/kind and full-frame
bounds; they return no monetary verdict. Direct-time cancel only discards an opaque token.
Exact retained Payment bytes and durable completion remain native responsibilities.
The embedding app must provision signed complete artifacts/configuration; the
unprovisioned runtime fails unavailable. Full real-wallet and physical-device
qualification remain open.


For initial enrollment, trusted native startup constructs CoreZK `EnrollmentOwnerV1`
from the approved scheme, exact policy originals and pinned original root DER, then
registers `NativeEnrollmentRuntime` through `retain_native_enrollment`. The typed
C/JNI enrollment call accepts original account/challenge/evidence bytes. Android sends
DER plus an opaque Play Integrity token; the issuer separately acquires Google's
original response. Native retains the exact account-authorized E5 and issuer-signed E6
before returning them. Complete-source loader handoff preserves the same opaque handle
and exclusive provider on failures. `beginEnrolled` still requires a fresh existing-account
signature through ordinary original admission. No foreign liveness or hardware verdict
can substitute for issuer checks. The E6 transport bound is separate from Payment10KB.
