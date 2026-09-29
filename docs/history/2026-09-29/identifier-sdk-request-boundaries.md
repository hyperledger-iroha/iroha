# Identifier SDK request and metadata boundaries

JavaScript and Swift public identifier encryption helpers report
`ram_lfe_encryption_unavailable` before inspecting private input. Public seed
options are removed. The previous exact-lift arithmetic is retained solely in
test helpers so its deterministic and adversarial controls remain testable.
Existing ciphertext request builders still validate the policy's declared
`bfv-v1` input mode. These changes do not qualify replacement encryption.

Execution responses contain ciphertext and an execution receipt. Identifier
resolution separately accepts an independently authenticated plaintext opening;
execution does not manufacture one. The C# request now requires ciphertext and
a typed `ToriiRamLfeOutputOpening`, with no plaintext `Input` property or optional
ciphertext fallback. Its wire decoder rejects missing, null, duplicate, unknown,
and malformed binding fields, including invalid unsigned timestamps. DTO
validation does not verify the authority signature or authorize a backend.
The optional server phone canonicality attestation is outside this SDK amendment.

C# and Swift identifier policy metadata now requires an explicit program ID and
opening-authority public key. Decoders reject absent, null, empty, non-string,
whitespace and control-character values. Constructors have no implicit empty
values for these bindings. C# preserves them when serializing policy metadata.
All existing policy/receipt test methods remain, with their fixture metadata
updated; structural preservation alone is not an execution result.

## Completed validation

| Scope | Result | Boundary |
| --- | --- | --- |
| JavaScript identifier/RAM tests | 46 passed, 0 failed or skipped | Normal rebuilt native module from the preserved historical SDK candidate; exact SDK amendments applied |
| JavaScript package types and lint | 1 type test passed; scoped lint passed | Same candidate |
| JavaScript package inventory | 204 files; no diagnostic helper or test paths | Normal `npm pack --dry-run` with prepack and distribution build; nothing published |
| Current C# build | .NET SDK 8.0.419; 0 warnings/errors | Default managed build; 313 source/input hashes unchanged |
| Current C# request/receipt/profile/policy tests | 231 passed, 0 failed or skipped | Managed codec/model controls |
| Current C# existing policy endpoint/metadata tests | 38 passed, 0 failed or skipped | Managed HTTP request/response controls |
| Current Swift source | Frontend parse and scoped whitespace checks passed | Not a Swift typecheck, full test run, or native qualification |
| Current duplicate Java transport/settlement consumers | Normal main/test compilation and 2 complete selected JUnit harnesses passed, 0 failed or skipped | 1,609 source hashes unchanged; settlement verifier is injected, so no native cryptographic qualification |
| JavaScript native build tooling | 148 passed, 0 failed or skipped | Complete source namespaces, completed dependency receipts, foreign-path refusal, interrupted retries and corrupted warm-cache controls; independently repeated on the ABI 25 candidate |
| Current JavaScript native module | Normal ABI 25 build and publication passed | 69 local compiler artifacts, all compiled fresh from the frozen candidate; retained compiler stream, dependency receipt and output hashes |
| Current JavaScript identifier/RAM tests, first attempt | 45 passed, 2 failed, 0 skipped | Three test policy literals lacked newly required metadata; the failures and native result remain retained separately |
| Current JavaScript identifier/RAM tests, repaired fixtures | 47 passed, 0 failed or skipped; strict NodeNext type check and scoped lint passed | Second normal ABI 25 native build; 20,734 source/input entries unchanged |
| Current JavaScript package inventory | 204 files; no diagnostic helper or test paths | Normal prepack and distribution build through `npm pack --dry-run`; nothing published |

The historical JavaScript build authenticates its top-level native module and preserves its
source-tree hash, but its build tool discarded the dependency compiler-artifact
stream. Shared-target dependency-source closure is therefore unverified; passing
46 tests and addon identity do not establish every native dependency's source.
The audit is retained in `sdk-native-dependency-audit/receipt.json`. The completed
Apple slices retain their Cargo streams in an isolated target; reused dependency
source bytes are not retroactively inferred from relative dep-info paths.

The repaired JavaScript builder retains compiler streams and hashes all local
outputs. It admits warm dependencies only through a completed receipt, and
preserves failed attempts while selecting a fresh directory on retry. The ordinary
cold native build completed on the separately managed `zk-sdk-current`
candidate with ABI 25 required. Its first 20,734-source manifest is
`3e83b0c6a9af1a1ea68c54623016865591054ef5cd33c789b5ec1700cacebe9a`:
the sealed `ce9a01d`/BFV component snapshot plus six reviewed JavaScript metadata
and build-tool amendments. All 69 local compiler artifacts were fresh and bound
to this checkout. The subsequent SDK run passed 45 of 47 tests: two exposed
policy fixtures without required program/opening-key metadata. Inspection found
a third incomplete base fixture whose generic negative assertions had passed
for that unintended reason.

The separate repair supplies explicit metadata to those three test literals and
adds valid-base assertions before both mutation tables, preserving every original
negative. A second ordinary cold native build completed because the authenticated
source fingerprint includes these tests. Its 20,734-source manifest is
`835802c9e0288c03f0d91047418d06eeef5ca72ee4c4adab2723ca2191c5054d`.
All 47 SDK tests, all 148 build-tool controls, the strict NodeNext test and scoped
lint passed, with zero source drift. All 69 local native artifacts compiled fresh.
The second native dependency receipt is
`563312e5b5244c4a8a5819f67d7bafdc56ed44bf780b90301768947e134db861`;
the retained addon hash is
`fc63bb6800d6bcc2355bd6825c520e17e966780c6ade06755fddf7fd7d9fa942`.
These results do not cover later mutable-main amendments or change the older
evidence limits. Namespace identity does not by itself pin the resolved macOS
SDK/Clang identity; the actual build also checks its platform build identity.

The C# authenticated request class now passes all four controls against a normal
ABI 25 `connect_norito_bridge` build from the same frozen Rust source. Its 63 local
compiler artifacts include 38 warm outputs whose exact identities and hashes match
the preceding JavaScript build. Managed loader diagnostics confirm the exact
isolated retained dylib, SHA-256
`3fbd7da07974e1cd8a3df9f973207e953e854bd48d116e972954dfd80274b3e9`.
The pinned .NET 8.0.419 default build and source/runtime guards pass. The release
checker correctly refuses dirty source; these are separate local runtime results.
The earlier ABI 24 failures and the initial staging refusal remain retained.

The additional three native confidential-wallet controls initially failed before
proving because C# required note-derivation contract revision 3. Rust's V3 note
relation exposes first-release contract revision 1, also used by Swift and Kotlin.
The managed check now requires exactly 1, with explicit native revision assertions;
all symbol and ABI checks remain. The default managed build completes with zero
warnings/errors, all four authentication controls still pass, and all three native
wallet controls pass in 61.74 seconds with zero skips. The change-to-full-redemption
workflow takes 60.87 seconds. Both runs verify the exact loaded library path and
unchanged managed sources/runtime/native bytes. The original three failures remain;
this is real local wallet lifecycle coverage, not a maximum-capacity or network test.

A separate current ABI 25 five-slice Swift build is running from source manifest
`8da98f8f4dadaf63d669a1c2683fc6d2c776eaa52ec1d426971a33a5a6789acc`.
It contains the canonical BFV schema/API retirement, current Swift changes and two
reviewed X509 clearing-owner amendments. The normal builder uses its supported
local-integration path, truthfully records dirty source, and owns generated pins.
A fresh isolated target retains compiler streams and checks every local artifact;
68 selected current SDK controls follow packaging. Native/Swift results are pending.
Cross-target archives and C links do not establish device execution.

The older five-slice Apple job remains separate: its historical ABI 24 candidate
contains only the backend-tag amendment, with host, device iOS and both simulator
archives completed. Host C ABI/SHA3/SHAKE/ML-DSA/ML-KEM controls pass; simulator C
consumer linkage was not execution. The x86_64 simulator archive completed in
60m14s; the fifth, x86_64 macOS slice is compiling. Its warm local dependency
provenance is limited, and neither it nor its eventual SDK result qualifies the
current Swift source.

The later Java/JavaScript policy amendment removes Java's two constructors that
inferred a program or reused the resolver key as the opening key, and its parser
fallback. Existing test constructors now provide explicit bindings. JavaScript
already required these fields on policy-list responses; its parameter, ciphertext
request and receipt-verification helpers now require them too. New negative
controls cover missing/null/type/whitespace values. This later JavaScript source
is not included in the earlier 46-test result; its current native-backed run and
fixture repair are recorded above.
Normal duplicate-Java compilation initially stopped at the removed `NativeAmxV2`
import. The three references now use Kotlin's existing `BlsNormalPublicKeyAdmission`
API. Two obsolete Gradle inputs had no remaining consumer and were removed.
Normal compilation and the complete selected transport/settlement harnesses now
pass, including the new program/opening-key negatives. No compatibility shim or
source exclusion was introduced. Exact source and prior failures remain under
`java-js-required-identifier-policy/`; passing results are in
`java-bls-canonical-consumer/result.json` and
`java-retired-consensus-inputs/result.json`.

## Retained evidence

Receipts, before/after source copies, source manifests, failed runs and compiled
managed artifacts are under `dist/zk-remediation/2026-09-29/`:

- `sdk-client-encryption-policy-repair/javascript/complete-receipt.json` records
  the repaired 46-test JavaScript result with zero drift across 20,880 inputs.
- `sdk-client-encryption-policy-repair/npm-pack-inventory-result.json` records
  package contents; the raw prepack output is retained alongside it.
- `sdk-client-encryption-retirement/qualification-summary.json` links the final
  source amendment and preserves the earlier diagnostic-helper repair results.
- `csharp-identifier-request-hard-cut/qualification-summary.json` retains the
  request change, 218 managed passes, authenticated failures and binary hashes.
- `csharp-required-identifier-policy/qualification-summary.json` records the
  subsequent 231 + 38 passing controls with retained SDK/test assemblies.
- `swift-required-identifier-policy/source-amendment.json` records the separate
  current Swift amendment and explicitly pending normal native tests.
- `js-native-dependency-closure/host-bridge-20260929T100041Z/` retains the actual
  ABI 25 C library, raw compiler stream and authenticated reused-output closure.
- `js-native-dependency-closure/csharp-host-auth-20260929T102358Z/` records the
  post-repair default managed build, four authentication passes and loader path.
- `js-native-dependency-closure/csharp-derivation-contract-repair/` retains the
  exact revision correction and original native-wallet failure diagnostics.
- `js-native-dependency-closure/csharp-host-wallet-20260929T102434Z/` records all
  three native wallet passes and loader evidence; the combined auth/wallet/default
  build result is `csharp-derivation-contract-repair/qualification-summary.json`.
- `swift-current-abi25/` retains the independent current snapshot, reviewed normal
  five-slice driver, source manifest and pending compiler evidence.
- `js-native-dependency-closure/current-native-run1/` retains the successful
  current native closure and the separate SDK fixture failures.
- `js-native-dependency-closure/fixture-repair/` retains the exact three-fixture
  correction, valid-base controls and amended source manifest; the successful
  normal rerun is recorded separately under `current-native-run2/`, including
  `qualification-summary.json` and the retained compiler stream and artifacts.

Exact scoped amendment SHA-256 values:

- `csharp-identifier-request-hard-cut`: `3936458d493eb6f779139e39c87798e000b1c611b532928f4d967277b2e56284`.
- `csharp-required-identifier-policy`: `9f551e4c0806afa3add0b92c4ed485cbc996aff3129da2ba7ca5b5e3c5fbbcdc`.
- `swift-required-identifier-policy`: `8d78a13cb25f6cf16e0028af149691cc20b2bec9a67cf66ace35c26f6176634c`.

These results qualify only the stated source and runtime scopes. They do not
qualify the changing main native implementation, replacement encryption, phone
attestation feature parity, deployment hardware, or independent cryptographic
review.
