# JavaScript change-wallet validation — 2026-09-28

An exact eleven-file SDK refresh was applied to the retained managed SDK
candidate after its JNI build completed. It adds the Core-backed default
diversifier and change-to-input helper, removes retired caller-key proof routes,
and closes owned loader snapshots on normal exit. Core/C/JNI and the earlier
frozen validation record were preserved. A separate recipe-only patch fills
preallocated entropy buffers inside its cleanup guard. Both patches and their
before/after hashes are retained.

The final normal builder used Node 24.21.0, Rust 1.93.1, offline dependencies,
one Cargo job, no incremental compilation, the shared target, and target-only
`strip=none`. It completed and published through the normal dirty-source debug
provenance checks; no custom loader, forged clean marker, or verifier copy was
used. Its 23m52s includes waiting for the shared Cargo lock and is not a benchmark.

## Results

- **66 combined controls pass, 0 skipped, 10.397 s**. This includes four mandatory
  actual-native wallet/root/default-change controls, alongside managed boundary,
  TypeScript, lifecycle, manifest and source/dist loader checks. It is not a claim
  that all 66 cases generate native proofs.
- The public package-dist self-import recipe passes in **66.085 s**, producing
  change and full-redemption proofs of **14,215** and **13,741** bytes. The first
  input uses a nondefault diversifier; restored change uses the native default
  diversifier and its commitment matches. Proof work records **2,598** and
  **2,442** event-loop ticks. Closing the prover rejects future work while its
  accepted second job finishes.
- Normal `npm pack` and offline external installation succeed. The new public
  exports exist. Loading the dirty debug native artifact through the documented
  native-directory option correctly rejects with `ERR_IROHA_NATIVE_BINDING` and
  `source_provenance_error`: the registry tarball deliberately excludes repository
  source-verifier scripts. Its diagnostic includes that missing script. No script
  or loader was injected to override this gate.
- Comparing normal-loader temporary directories before and after all consumers
  leaves **zero new owned snapshot directories**. Existing directories and the
  authenticated original addon were untouched.

The recipe is deliberately source-only and absent from the npm tarball. It uses
OS randomness and disposable local history, submits no ledger transaction, and
cannot authenticate a ledger root. JavaScript strings and caller-retained private
openings do not carry an erasure guarantee.

## Reproduction and identity

From `javascript/iroha_js` in the recorded SDK candidate:

```sh
npm run build:native
npm run build:dist
node --test test/confidentialProofBuilders.native.test.js test/confidentialChange.test.js \
  test/confidentialProver.test.js test/confidentialRoot.test.js \
  test/confidentialProofCardinality.test.js test/transactionBuilderConfidentialValidation.test.js \
  test/nativeVerification.test.js
node --test recipes/confidential_change_redemption.mjs
npm pack --ignore-scripts --json --pack-destination <recorded-output-directory>
```

Exact build environment, source patches, commands, logs, external-install control,
checksum manifest and retained original addon are under
`dist/zk-remediation/js-change-refresh-20260928/`.

- Final addon SHA-256: `3cb9d317405e67f2fb452bff06e3a318518cf20f9e0b0d45bc008a02912e4785` (174845824 bytes).
- Final package SHA-256: `575b1d2e8a0a6ebba58adcc7fb7a26f165dba9247fcae841960500c088e1bfd9`.
- Final run-receipt SHA-256: `a0b690831aeecacac4a02900aa953e0581b00abe394547cb25113ef475d5abde`.

Final source hashes:

| File | SHA-256 |
| --- | --- |
| `crates/iroha_js_host/src/confidential_wallet.rs` | `4c71f803c3ecf61ea16f90f69b27826282e9a5742805fb5ab3c63e75a0a7c2f6` |
| `javascript/iroha_js/src/native.js` | `72a27169a77e7676b56566622b3c87b6aec78241d875d340958fe451f66448b4` |
| `javascript/iroha_js/src/confidentialProofBuilders.js` | `dde665d048edcac1cc43fccfe4a46ef8b4cde5371f055d15b24574b9698c2d91` |
| `javascript/iroha_js/src/index.js` | `e262849a79edc82fc70d252c5032e1e10678b78ccca0e75c3f2166072b985b37` |
| `javascript/iroha_js/index.d.ts` | `b05e6f4986c0f92681282d82563c309222317b5bb900aa41fee28f7345ae2b33` |
| `javascript/iroha_js/README.md` | `996200dbb207d664d96610adb5bca6837c124104c524acf18feb8e93e4752997` |
| `javascript/iroha_js/test/confidentialChange.test.js` | `8eb2d5bd34034b7dfc2603228829b807de6a8a8085265f1fe76b627fa71e5c66` |
| `javascript/iroha_js/test/confidentialProofBuilders.native.test.js` | `c00ce10e26f305c02d3b0f75cae815296155ddd43cb0c21ca3b188da82a70c0d` |
| `javascript/iroha_js/test/nativeVerification.test.js` | `b00b3e7e66a6a929167895ef73bbd01780b4339688ef4c6e9e538fa237935515` |
| `javascript/iroha_js/test/fixtures/typescript/confidentialProofCardinality.types.ts` | `ae7fa48d270e42a1f8ab20d1574e89d202ec470ea214e96f611313b51cbf5dec` |
| `javascript/iroha_js/recipes/confidential_change_redemption.mjs` | `62b66c4e276a62446f4df25d0d34fcdb5933c0e74e39b2143d28179b41795b97` |

This establishes the captured macOS arm64 source/dist workflow and its negative
installed-package gate. A clean installed native release still requires reviewed
clean source, native distribution and authenticated release provenance. This does
not qualify later unrelated Core/X509 changes, other targets, physical devices,
or ledger authorization. The earlier native addon and receipt remain separately
retained; they are not relabeled as this newer helper snapshot.

## Current Core interoperability check

A separate external driver imported the same authenticated candidate's public
`dist/index.js`, without editing candidate source or injecting a native loader.
It generated another change/full-redemption pair and retained only public proof
bytes and statement metadata. The public JSON SHA-256 is
`214c97ee430a9e37ef6d65f514b6a31f4e949045ac3d462be5c7039912193217`;
the 14,215/13,741-byte proof pair and source/log receipts are under
`dist/zk-remediation/js-change-refresh-20260928/interop/`. No private opening,
spend key, diversifier, membership path or selected input index was persisted.

An independently compiled harness linked the current normally built Core library
and its Cargo-fingerprint-matched Norito/data-model dependencies. Both canonical
Norito roundtrips and native verification with freshly prepared canonical keys
passed; wrong relation, wrong key, proof-size cap, proof tampering, retired CID and
retired backend controls rejected for each proof. The run completed successfully
in 42.069 seconds; source, compiler identity and results are retained in
`dist/zk-remediation/sdk-current-core-interop-20260928/`.
This establishes wire/circuit interoperability for those exact SDK artifacts and
current Core verifier. It does not authenticate the disposable roots or establish
ledger spending authorization, full-Core test coverage or X509 qualification.
