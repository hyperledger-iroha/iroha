# KAGEMUSHA Spartan experimental backend

This crate contains the experimental four-operation proof engine, complete Payment
codec and native custody integration for Bootstrap, Load, Send and Receive. It
uses the canonical Iroha Pasta implementation and the existing wallet provider,
hardware adapters, account identity and journal. It does not switch the installed
wallet generation, change Taira state or migrate funded wallets.

The profile uses the future 532-byte credential body (596 bytes with its signature)
and 30-field monetary Core. The installed credential generation remains separate.
An authenticated experimental credential and program bundle are required; installing
public verifier programs cannot convert an existing credential or wallet state.

[DESIGN.md](DESIGN.md) consolidates the construction, its assumptions and the
source-bound validation status. This README covers the current implementation and
how to run it; historical research measurements do not establish complete-backend
or physical-device performance.

## Complete proof path

Each operation produces a linked Pallas/Vesta Spartan pair under a fixed program
for its operation kind. The primary relation authenticates the credential,
certificate, hardware receipt, exact state and monetary transition. The secondary
relation completes the cycle's recursive checks. Send irreversibly debits the
amount plus fee and binds one receiver. Receive binds the original incoming
Payment and proves credit or Burn from its actual total proof-verification result.
Load additionally verifies canonical Sumeragi finality against the installed
signed genesis and exact ordinary Load receipt.

Acceptance checks both scalar/point proofs, both accumulated matrix claims, and
both own and inherited generator claims. A linked proof alone is insufficient.
The fixed-program compiler first stabilizes eight public programs and then starts
a fresh genuine Bootstrap → Load → Send → Receive chain. Intermediate sizing
proofs cannot substitute for that final chain.

Native proof masks and pair-binding masks use fresh OS entropy. Explicit test
replay can supply deterministic entropy. The public program format contains
verification instructions and constants, excluding private witness arrays and
witness-producing closures. Programs are authenticated by exact digests under
a small root-signed parameter manifest before monetary use.

## Payment and SDK integration

`payment_wire.rs` encodes and strictly decodes the complete IPM1 Payment, including
its Norito envelope, complete linked proof, reconstructible public inputs,
statement, Request, receipt and credential/certificate companion. The exact
original proof bytes remain bound even when total decoding selects a canonical
invalid candidate. A caller cannot supply an authoritative validity Boolean.

`NativeEngine` and the canonical `FutureRuntime` integrate the proof engine with
the existing A0–A12 provider. Hardware signing follows durable selection. Pending
work resumes the same frozen capsule, and released operations retain and return
their exact original bytes. Account admission checks the actual retained enrollment,
future credential, hardware key and selected journal state, then verifies the
existing Ed25519 account's signature over a fresh Native challenge.

Swift and Kotlin expose installation, account admission, prepare/review/execute,
complete Payment verification, and exact retained-output recovery through the same
native owner. Send confirmation uses hardware-backed biometric or device-credential
authentication in the application before execution; it is not a proof of user
confirmation against a malicious SDK caller. See [NATIVE_BACKEND.md](NATIVE_BACKEND.md) for the exact API sequence,
custody boundaries and four-operation profile scope.

The physical device fixture uses the real Android/Apple platform adapters and
canonical EnrollmentOwner. Its independently issued test parameters and separate
app namespace cannot be selected by a production wallet. The iPhone fixture uses
production App Attest evidence and the Secure Enclave; the Android fixture uses
actual hardware key attestation. Hardware-only probes are distinct from a complete
native payment and restart test.

## Validation

The complete backend validation is in progress. Component tests, native API
inventories, codec checks and hardware signatures have separate evidence; they do
not establish a successful complete device payment, the 12,000-byte complete
carrier target or a performance improvement. Record those conclusions only from
the actual full-chain and physical runtime results.

The heavy `backend_chain_tests::complete_recursive_bootstrap_load_send_receive`
test compiles the fixed programs, proves the fresh monetary chain, measures the
actual complete Payment, imports and verifies the public programs, exercises native
prepare/resume/finalize, and proves malformed-proof Receive/Burn. The device fixture
then uses those authenticated programs with real protected custody, account
admission, hardware receipts, fresh device confirmation and restart recovery.

The profile does not implement RefreshTimeAnchor. Controls-off Send uses its
canonical local floor. Time-dependent controls require a protected matching anchor;
a caller timestamp cannot install one. This experimental backend is not a full
production wallet-generation cutover or a soundness-review approval.

## Reproduce

Run from this directory and preserve the warm target across iterations:

```sh
export CARGO_TARGET_DIR="$PWD/target"
cargo iroha-fast -- test --locked --offline --release --lib -- --test-threads=1
run_dir="$PWD/output/full-backend-run-$(date -u +%Y%m%dT%H%M%SZ)"
KAGEMUSHA_SPARTAN_TEST_LAYOUT=p18-s17 \
  KAGEMUSHA_SPARTAN_TEST_OUTPUT="$run_dir" \
  cargo iroha-fast -- test --locked --offline --release --lib \
  full::backend_chain_tests::complete_recursive_bootstrap_load_send_receive \
  -- --ignored --exact --nocapture --test-threads=1
```

A full-chain run reserves a new output directory and refuses to overwrite an
earlier run. It writes the authenticated device bundle under `native-device/`.
The test-only layout choices are `p18-s17`, `p17-s17` and `p18-s16`; the selected
layout must pass complete validation. The separately named
`measure_recursive_layout_candidate` test records candidate costs and runs the
actual monetary chain, but does not run the fifteen-case native containment gate.

The saved bundle can independently drive fresh native proof and containment
checks with `KAGEMUSHA_SPARTAN_DEVICE_ARTIFACTS="$run_dir/native-device"` and
the ignored test
`native::engine::adversarial_tests::authenticated_native_containment_from_bundle`.

A fresh checkout may need an initial dependency fetch before `--offline` works.
The isolated physical fixture lives under `device/runtime`; the iOS runtime UI is
`device/ios/RuntimeFixture.swift`. Build from this canonical checkout. Keep generated
proof artifacts, device packages, signing scratch and logs in ignored outputs.

## Design and research records

[CERTIFICATE.md](CERTIFICATE.md), [CREDENTIAL.md](CREDENTIAL.md),
[BOOTSTRAP.md](BOOTSTRAP.md) and [MONETARY_COMPONENTS.md](MONETARY_COMPONENTS.md)
record the authentication and monetary relations. [A2-UNIFORM.md](A2-UNIFORM.md)
describes the uniform compiler, recursive layout and deferred matrix checks.
[research/DESIGN-100X.md](research/DESIGN-100X.md) and
[research/CONSTRUCTION.md](research/CONSTRUCTION.md) retain the design and review
history, including unresolved soundness obligations.

Earlier census-sized Send/Receive benchmarks, the 126-test research checkpoint and
the synthetic eight-hop results remain historical evidence under
`output/kagemusha-100x-20261108/spike/runs/codex-20261112/`. Their pair sizes and
performance are not measurements of this complete backend. The source was moved
from the old ignored spike directory into this canonical crate without retaining
a duplicate source tree.
