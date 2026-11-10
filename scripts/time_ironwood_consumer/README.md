# Time/Ironwood consumer qualification

These helpers qualify externally generated activation-history originals through
the native checkpoint-advance C ABI and its Swift/JNI callers. Native code owns
Norito decoding and finality verification. Public JSON and hash admission alone
does not qualify a consumer, a World payout session, a physical device or live
activation.

## Prerequisites and discovery

Use Python 3.11+ with the standard library (validated with Python 3.12) on
macOS/Linux. Native C mode needs a compatible reviewed host library. The platform
runner requires `/usr/bin/git`, a clean canonical source checkout on
`optimizations`, and its exact reviewed commit/tree. Swift requires macOS and
Xcode/SwiftPM and an admitted Apple artifact containing the macOS host slice.
Kotlin requires a compatible JNI host, the repository Gradle wrapper and its
configured JDK 21 toolchain. Native artifact/build/source admission must already
be complete; these helpers do not produce or sign those native artifacts.

From the repository root:

```sh
python3.12 scripts/time_ironwood_consumer/native_c_driver.py --help
python3.12 scripts/time_ironwood_consumer/qualify_platform.py --help
```

## External inputs and admission

The fixture contains the anchor and promoted checkpoints, each intervening
finality proof including the anchor proof, and the complete certified World at
the bootstrap height. The exporter verifies the pool record against that World
under the activation's selected LtHash profile.

Both helpers require these four variables; neither generates their inputs:

| Variable | Required value |
| --- | --- |
| `BOI_ACTIVATION_FIXTURE_DIR` | Absolute canonical, owner-controlled external directory containing only the generated manifest and named originals. |
| `BOI_ACTIVATION_MANIFEST_SHA256` | Independently reviewed SHA256 of the exact generated manifest bytes. |
| `BOI_ACTIVATION_PRODUCER_RECEIPT` | Absolute canonical external receipt file, outside the source and fixture directories. |
| `BOI_ACTIVATION_PRODUCER_RECEIPT_SHA256` | Independently reviewed SHA256 of that exact receipt. |

The `boi.time-ironwood-fixture-producer.v1` receipt binds the actual signed source
commit/tree, clean source before and after generation, executed test-binary
SHA256/size, build-receipt digest, successful exact exporter run, generated
manifest digest and every original's name/size/digest. `patchBaseCommit` identifies
only the exporter patch base. Receipt claims require independent producer review;
parsing or hashing them does not verify signatures or establish build provenance.

Generate the corpus in the native owner's exclusive interval, preserve and verify
the complete directory externally, and restore the clean signed source before
running release producers. Never copy generated originals into a source capture.
The C driver additionally pins the supplied library and artifact receipt. The
platform runner selects the explicit external Apple artifact or JNI directory;
review must independently establish that the actual artifact matches the admitted
source and receipt. Commit signature verification remains an external prerequisite.

## Ordinary suites and dedicated qualification

Ordinary Swift/JUnit discovery skips the activation-corpus test only when **all
four selection keys are absent**. Any partial, empty or malformed selection fails.
The separate selection-policy tests need no corpus or native library.

Use `qualify_platform.py --platform swift` or `--platform kotlin` with the required
paths and pins shown by `--help` for dedicated qualification. It requires all four
inputs before launching commands, runs the pinned C helper's `--admit-only` mode,
then builds/runs the actual selected platform test. A fresh xUnit result must
contain **exactly one matching pass and zero skips, failures or errors**. An
ordinary discovery skip can never qualify. Inherited Git environment overrides
are refused; the presentation-only `GIT_PAGER` is discarded. Source identity and
cleanliness are checked before and after the platform run.

`native_c_driver.py --admit-only` reports `INPUTS_ADMITTED_ONLY` with zero native
checks. Its native mode loads the explicitly pinned bridge and checks exact
promoted-checkpoint bytes, metadata, malformed history and foreign-pin refusals.
Both qualification helpers require fresh external output paths and refuse to
overwrite existing outputs. After creating its output directory, the platform
runner preserves failure logs and a failure receipt; early preflight refusal
creates no receipt. The C helper emits no success receipt on refusal. No fixture
generation or signing occurs.

Public admission/runner controls use temporary stub bytes and mocked commands:

```sh
python3.12 -m unittest discover -s scripts/time_ironwood_consumer -p 'test_*.py'
```

Those controls do not substitute for compiled Swift/JNI tests or genuine native
execution with the reviewed corpus and artifacts.
