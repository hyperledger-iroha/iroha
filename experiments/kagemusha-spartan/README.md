# KAGEMUSHA Spartan experimental backend

This crate contains the experimental four-pair proof engine, complete Payment
codec and native custody integration for Bootstrap, Send, Receive and Control.
Control covers Load, RefreshTimeAnchor and Unload. The engine
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
Payment and derives credit or Burn from its actual total proof-verification result.
Fresh Receive admission requires credit before hardware selection. Recovery verifies
the recorded outcome of an already selected Receive, including a historical Burn.
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
witness-producing closures. The installer authenticates the eight program digests
against a manifest signed by the root-certified Artifact key. Manifest v4 selects
two full-width LogUp challenges and the bounded program encoding. Its source and
conditional security budget are described in the [Control soundness review](research/CONTROL-SOUNDNESS.md).

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
native owner. The installed Spartan runtime refuses Send and Unload without
protected authorization of the exact review. The isolated two-wallet fixture
uses its own authenticated test scheme. The SDK and Native contracts are in
[NATIVE_BACKEND.md](NATIVE_BACKEND.md).

The physical device fixture uses the real Android/Apple platform adapters and
canonical EnrollmentOwner. Its independently issued test parameters and separate
app namespace cannot be selected by a production wallet. The iPhone fixture uses
production App Attest evidence and the Secure Enclave; the Android fixture uses
actual hardware key attestation. Hardware-only probes are distinct from a complete
native payment and restart test.

## Validation

The manifest-v4 compiler produced identical canonical bytes for all eight public
programs on passes four and five. A fresh Bootstrap → Load → Send → Receive
chain then passed, including its checked balances, exact Payment recovery,
RefreshTimeAnchor and Unload. The Payment envelope was 9,862 bytes, the complete
IPM1 message was 10,711 bytes, and the public program bundle was 5,663,732 bytes.
One valid host Receive took 92.017 s across preparation, resume, finalization
and completion recovery, including 55.50 s in finalization. With the signed v4
shapes, Native requires at least 939,524,096 available bytes before it will
start any of the four proof kinds. A phone reporting the target allowance of
730,144,440 bytes would be refused at that check. The separate accounting for
the Receive primary prover estimated 741,937,508 bytes from retained capacity
and projected workspace. The v4 chain did not record process RSS.
[Full v4 chain](output/production-20261119/chain-v4-final.log)

On 2026-11-21 the physical Pixel 6 completed a holder-confirmed Send, received
its counterparty’s 10,711-byte return Payment, and recovered both Payments after
restart. The fresh-process Receive took **323.318487 s** from retained incoming
bytes to verified durable output. Execute accounted for **309.055566 s**. Both
its phase peak and the process-lifetime peak were **1,078,714,368 bytes RSS**.
The third launch recovered all four operations with zero executions or prompts.
The authenticated fixture APK was `a3812a9c…9a31`. The 3 s and 730,144,440-byte
targets remain open on this device. The iPhone run is pending.
[Pixel measurements and original report digests](output/production-20261119/pixel6-v4-protected-result.json)

A separate three-launch v4 custody fixture measured the software counterparty on
the first Mac launch. Its valid Receive took 263.666 s in finalization alone
and reached 1,557,184,512 bytes RSS during that phase. Both Payments were
10,711 bytes. In a fresh process the first wallet credited the return Payment and
recovered its exact Send and Receive outputs. Its normal Receive flow took
630.233 s and the process peaked at 1,055,408,128 bytes RSS. These timings
were collected alongside other builds. [Counterparty measurements](output/production-20261119/device-host-current-v4-attempt2/device-measurements-launch-1.json) · [Fresh-process Receive](output/production-20261119/device-host-current-v4-attempt2/device-measurements-launch-2.json)

The Payment envelope has 172 bytes of room under the 10,034-byte limit. The three
layouts available in the fixture keep the same primary witness size and fail the current
phone memory admission. Opus 5.5 projected that shrinking the rows enough to
address latency would exceed the envelope budget with the current construction.
The measured Pixel Receive exceeds the 3 s target. A different commitment layout or recursive
construction would require new signed programs and a new soundness review.

The authenticated manifest-v3 three-mode Mac chain produced a 10,780-byte Payment envelope
and an 11,629-byte complete IPM1 message. The envelope exceeds the production
10,034-byte limit by 746 bytes. The message fits the experimental 12,000-byte
QR budget. Device latency, peak memory, and protected debit confirmation still
require physical qualification. [Measured chain](output/takeover-20261118/recursive-control-result.json)
and [wallet wire limits](../../specs/kagemusha_wallet_wire_v1.md).

Re-encoding that signed historical Payment under future peer schema 3 yielded
a 9,862-byte envelope and a 10,711-byte complete message. The decoder recovered
the exact proof pair and the 749-byte credential companion. Encoding the eight
historical public programs produced a 5,678,766-byte bundle with every instruction
preserved. These historical measurements cover the saved v3 originals.
[Payment measurement](output/production-20261119/historical-payment-v4-encoding.log) ·
[Program round trip](output/production-20261119/compact-program-bundle.log)

On November 19 the preserved v3 host wallet received the retained return
Payment in a second process. The measured flow took 670.188 s and peaked at
1,482,604,544 bytes RSS, including 623.57 s in Execute. This concurrent-build
measurement used software custody and exceeded the 0.68 GiB phone budget. The
third process recovered all four completed operations. The host test passed
all three launches. [Restart result](output/production-20261119/two-wallet-retry.log) ·
[Host Receive measurements](output/production-20261119/host-custody.gK4TLd/device-measurements-launch-2.json)

The heavy `backend_chain_tests::complete_recursive_bootstrap_load_send_receive`
test compiles the fixed programs, proves the fresh monetary chain, measures the
actual complete Payment, imports and verifies the public programs, exercises native
prepare/resume/finalize, and proves malformed-proof Receive/Burn. The device fixture
then uses those authenticated programs with real protected custody, account
admission, hardware receipts, fresh device confirmation and restart recovery.

RefreshTimeAnchor verifies the certified time signer and advances the floor to
the greater of the previous floor and the signed time. The v4 Bootstrap starts
with lease and quota controls disabled. Its signed programs preserve that policy
and the original Bootstrap credential. Enabling those controls or renewing a
credential on the same funded lineage requires additional recursive relations.
The outstanding release criteria are in [TODO.md](TODO.md).

## Reproduce

Run from this directory and preserve the warm target across iterations:

```sh
export CARGO_TARGET_DIR="$PWD/target"
cargo iroha-fast -- test --manifest-path "$PWD/Cargo.toml" --locked --offline --release --lib -- --test-threads=1
run_dir="$PWD/output/full-backend-run-$(date -u +%Y%m%dT%H%M%SZ)"
KAGEMUSHA_SPARTAN_TEST_LAYOUT=p18-s17 \
  KAGEMUSHA_SPARTAN_TEST_OUTPUT="$run_dir" \
  cargo iroha-fast -- test --manifest-path "$PWD/Cargo.toml" --locked --offline --release --lib \
  full::backend_chain_tests::complete_recursive_bootstrap_load_send_receive \
  -- --ignored --exact --nocapture --test-threads=1
```

A full-chain run reserves a new output directory and refuses to overwrite an
earlier run. It writes the authenticated device bundle under `native-device/`.
The test-only layout choices are `p18-s17`, `p17-s17` and `p18-s16`; the selected
layout must pass complete validation. The separately named
`measure_recursive_layout_candidate` test records candidate costs and runs the
actual monetary chain, but does not run the fifteen-case native containment gate.

The retail build uses `full::backend_chain_tests::compile_wallet_program_bundle`
with the same layout and a fresh output directory. Once two successive compiler
passes agree on all eight canonical programs, it writes `program-bundle.norito`.
The artifact tool's `--verify-spartan-sources` command checks that bundle against
the installed carrier and reimports each program against the signed manifest.

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
