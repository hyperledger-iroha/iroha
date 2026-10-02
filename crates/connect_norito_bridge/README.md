# Native SDK bridge

`connect_norito_bridge` exposes the current ABI-25 C and JNI bridge to the SDKs.
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
and ML-KEM. Static XCFramework layout, headers, exports, and CocoaPods archive
limits remain enforced by their existing owners. Profile selection alone is
not evidence that a candidate passes resource, SDK, or device qualification.

`connect_norito_domain_id_validate_v1` admits only exact canonical ASCII
`domain.dataspace` identities through the native pinned UTS-46 owner. Its 127-byte
input bound is checked before reading or allocating. SDK callers must retain the
provided spelling and reject missing native admission; this API does not normalize
Unicode input or provide a managed fallback.

The ordinary Native parent uses the crate-root
`KagemushaNativeOrdinaryRuntimeStartupV1::from_installed_runtime_and_native_account`
constructor before mobile startup. Construction authenticates and retains the public inventory
and exact release artifacts, then registers that same actual Native owner once. Independent
registration is retired. The native coordinator installer performs the initial fresh four-node
S/W acquisition under that owner before managed coordinator open. The storage selector must
equal the independently selected Native path. An unknown construction or initial acquisition
result cannot restart another clock, read or enrollment journal in the process. A completed
same-path retry rechecks the live held account selection; it does not revive an expired lease.

The constructor still requires the independently admitted installed runtime authority and an
actual immutable `AccountClient` with its held Ed25519 ledger key. Those come from the shipping
Native parent's admitted app release and protected account custody. JNI signing callbacks,
managed settings, public inventory decoding and a hardware P256 app key cannot supply those
inputs. App hardware identity admission and the separate current financial/proof/finality
owners remain mandatory. This source composition does not establish installed SDK, device or
release qualification.
