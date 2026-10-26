# KAGEMUSHA four-validator acceptance

This tooling joins current compiler provenance, the full catalog, native wallets and one retained four-validator network. It is engineering acceptance tooling; synthetic controller tests alone establish no native execution or release result.

The native owner is `kagemusha_network_setup::continuation::execute_four_validator_monetary_continuation` in `integration_tests/tests/kagemusha_network_setup/continuation.rs`. It must create one fresh in-memory genesis owner and four validators, retain them through the complete journey, perform mandatory native DKG, then emit the actual setup. There is no historical retained-genesis selection or alternate-network fallback.

## Execution boundary and prerequisites

- Execution uses Linux and exact source-reviewed prebuilt release executables: `iroha3d`, `iroha`, the `integration_tests --test sumeragi` owner and the `iroha_core_zk --lib` native test producer. Build and source-closure review are separate operations, never implicit controller work.
- Selected executable admission joins the exact prospective package/target and emitted filename, the local default feature closure, and the actual Cargo artifact profile. Release acceptance requires optimization level 3 with debug assertions disabled. The requested profile name alone is insufficient.
- A current compiler-input capture and explicit source review are mandatory for each executable. The separate `capture_build.py` producer uses maintained source/dependency and typed generated-input auditors, pins actual Cargo artifacts and tool identities, and records unchanged prospective input bytes. Admission independently replays those bindings. Broad source snapshots, the checked-in registry, old captures and successful synthetic tests cannot substitute. Cargo and the selected toolchain remain the authority for source-to-artifact construction, including warm caches; this is not an independent reproducible-build proof. Host SDK/system libraries are outside the claimed Rust compiler-input closure.
- The current native owner and all selected artifacts must be compiled and captured after canonical changes settle. Current source, binary, generated-input, tool and closure identities must remain fixed across the journey.
- A pending merge (`.git/MERGE_HEAD`) or unresolved index prevents compiler capture, admission and every later stage, including a resolved merge that has not been committed. Current source records explicitly retain merge absence; incomplete older records are refused.
- Full catalog compilation uses the actual same live owner's emitted `setup/capture.json`. A separate completed four-validator setup or host StateExecutor capture is DATA and cannot replace that live source. Current compiled domain has 22 routes. Actual distinct program/Q/original counts come from the newly emitted inventory and strict source qualification; 18 checked-in Q indices are DATA only.
- Full raw construction retains a 128 GiB upper bound and 20 GiB headroom. Independent originals require fresh private inodes on the same reflink-capable filesystem. Linux FICLONE and macOS fclonefileat are supported by the transport helper; the full issuer/network journey remains Linux-only. Unsupported clone, capacity or custody operations refuse without a hardlink or full-copy fallback. Capacity is an observation, never a reservation or a resource qualification.
- The actual issuer provisioner must have current application/enrollment/eligibility templates, rooted certificates and signing custody, worker originals, service/release/origin bindings, and private stores/credentials. The [same-network engineering composer](../../integration_tests/tests/kagemusha_network_setup/continuation/issuer/README.md) creates the issuer configuration and fresh journal/signing custody from those selected originals. Its empty operator decision table approves no subject; actual Linux startup and online enrollment still require separate execution. The controller exposes a machine-to-machine handoff below. No new product decision or placeholder approval is introduced.
- Before starting the network, the selected integration harness must pass the current issuer command, private-configuration and composer DATA controls. Its ignored composer entry point must appear in both exact test listings. Preflight only lists that entry point. After registration and explicit selection of actual private inputs, the controller owns the same captured composer's invocation and requires its natural success before issuer startup. The DATA controls require writable `target/qualification/issuer-provisioning-tests` in the selected original checkout.

## Launcher and inputs

Run from the selected repository checkout, branch `optimizations`, using isolated unoptimized Python 3.11+ with `-I -B -S`:

```text
python3 -I -B -S scripts/kagemusha_network_acceptance/run.py MANIFEST_SHA256 SELECTION SELECTION_SHA256 FRESH_CAMPAIGN
```

`--help` only prints usage. The explicit manifest hashes every runtime source helper before import, and checkout Python helpers execute the authenticated source bytes rather than cached bytecode. Ambient campaign/loader overrides are refused.

Selection is a closed JSON object with `schema = "iroha.kagemusha.real-network-campaign-selection.v1"`, exactly three `captures` roles (`stock`, `owner`, `native`), and optional reviewed `reuse` of raw content-addressed DATA. Each role has exactly `directory`, `result_sha256` and `review_sha256`; its two pins select `result.json` and `review.json` inside the absolute capture directory. `stock` selects the `nodes` producer role, `owner` selects `network`, and `native` selects `native`. Historical role supplements, old retained-network input selection, old counts and source-equivalence exemptions remain refused. All executable Python helpers are maintained sources included in the selected runtime manifest; historical qualification directories contain only explicitly selected evidence, never an implicit code dependency.

Build captures are explicit, separate invocations after source changes settle. Select an existing reviewed runtime manifest digest and unused output directories; select Cargo slots intentionally. For example, invoke each required role with its own fresh output:

```text
python3 -I -B -S scripts/kagemusha_network_acceptance/capture_build.py MANIFEST_SHA256 --role nodes --profile release --slot acceptance-nodes --jobs 2 --output ABSOLUTE_FRESH_NODES_CAPTURE
python3 -I -B -S scripts/kagemusha_network_acceptance/capture_build.py MANIFEST_SHA256 --role network --profile release --slot acceptance-network --jobs 2 --output ABSOLUTE_FRESH_NETWORK_CAPTURE
python3 -I -B -S scripts/kagemusha_network_acceptance/capture_build.py MANIFEST_SHA256 --role native --profile release --slot acceptance-native --jobs 2 --output ABSOLUTE_FRESH_NATIVE_CAPTURE
```

Outputs must lie under an existing `target/qualification` directory in this checkout. The producer does not automatically authorize its capture. After reviewing retained inputs, artifacts and failures, the root reviewer may write the exact `iroha.kagemusha.current-compiler-review.v1` object with `result_sha256`, `accepted_current_compiler_closure: true`, `reviewer_role: "root"`, and `scope: "engineering compiler inputs only"`. A later `withdrawal.json` in that capture revokes admission. Every later phase rechecks source, tools, artifacts, review and withdrawal. The protocol controller never builds or repairs a capture.

`FRESH_CAMPAIGN` must be absent, absolute and inside this checkout's `target/qualification`. The controller creates owner-only directories/files, holds an exclusive lock, and never overwrites a command or retries a failed occurrence. Runtime secrets remain in private outputs or separately selected private originals; configuration bodies and keys are never copied into controller receipts. No Taira or Minamoto endpoint is used.

## Current handoffs

1. Start the single source-selected native network owner. Retain actual setup originals, signed genesis, H5 installation and `ready.json`. The H5 file predates the DKG restart; it alone is not evidence that restart completed. Current owner code emits ready only after four post-restart H5 checks.
2. Construct and strictly qualify the current 22-route catalog from that exact live setup and retained originals. Register the source-selected Scheme and verifier pack, chaining `registration-command.json` to ready.
3. Publish `issuer-request.json` with setup/registration pins, current Scheme/manifest and public verifier-pack reference. The runtime owner prepares the private native provisioning request described in the [composer contract](../../integration_tests/tests/kagemusha_network_setup/continuation/issuer/README.md), then atomically publishes one private `issuer-composer-input.json` with closed fields:

   ```json
   {
     "schema": "iroha.kagemusha.network-controller-issuer-composer-input.v1",
     "issuer_request": {"path": "<this campaign's issuer-request.json>", "sha256": "<exact current SHA-256>"},
     "request": {"path": "<absolute private native provisioning request>", "sha256": "<exact original SHA-256>"}
   }
   ```

   The input and request must be regular, unaliased, private owner-only originals under this checkout's `target/qualification`. The request is at most 64 KiB and must name the exact current `issuer-request.json`, `live/setup/setup.json` and `live/registered.json` path/hash pairs. Python inspects only those three DATA references; it never copies the private request body into receipts. Native owns the full request grammar, policy and custody checks. The controller executes the selected `sumeragi` composer's exact ignored test and retains its command, binary, input pins, PID, raw streams and natural completion. Source and request changes refuse acceptance.

   The native composer alone publishes `issuer-selection.json`, selecting one retained peer and its private configuration. A pre-existing output, including a dangling path, refuses invocation. A visible output after failed or uncertain composition is preserved for reconciliation and cannot release the issuer command. Only known exact composer success permits the controller to select the output and start the issuer. Configuration contents remain opaque to Python. The native owner parses the exact TOML, checks current Scheme/manifest and starts the real issuer worker; startup does not assert that an online bank enrollment was completed. This engineering flow does not constrain production issuer configuration producers.
4. Export native software A's target/custody, preserving the source-selected independent originals. Send the **target-only** funding command chained to `issuer-ready.json`. Wait for the exact durable activation wire and `activation-prepared.json`. Run `retain_actual_a_activation_before_network_submission` using the same A custody. Only after native retention and reopen both return `Awaiting` with the same derived entrypoint, acknowledge `activation-retained`, chained to the prepared receipt. The exact nine-field receipt carries `nativeResult: awaiting` and canonical nonzero `entrypointHex` DATA; Native and the Rust owner bind that selector to the retained signed wire. No new payment signature or wallet reset occurs in this stage.
5. Join funded registration/activation/Load heights to the actual setup cut and retained wire/acknowledgment hashes. Export native BLS Load finality, then genuine native A→B→C exchange, offline folds, retained-byte replay and C's Unload claim. Current Load uses one terminal A and no W; no recursive BLS finality catalog is constructed.
6. Submit settlement with an explicit software bad-sigma adversarial claim. Preserve exact observed rejection coverage, finalized conservation, transport retry and instruction retry. Confirm from the same retained C after process restart, then acknowledge confirmed. The native owner restarts the same four stores and checks replay without another payout. Exact startup census is three starts for three validators and four for the issuer: initial, mandatory DKG restart, issuer-only restart, final restart.

Every handoff binds the previous exact receipt; Rust remains the authority for signatures, proofs, finality, permissions and balances. Full positive/adversarial engineering acceptance requires exact named native success, stable inputs, final startup census, no remaining peer processes and all actual assertions. Physical devices, phone durability/energy/latency/memory, real-bank enrollment, production custody, M3 and full release qualification remain separate.

Exchange DATA intake bounds every Payment, Receive and Archive original at the current native model's 10,034-byte message limit. Manifest lengths and actual file extents must both fit; this size check does not authenticate their proofs.

## Failure, timeout and recovery

Git, tool discovery, compiler metadata, source evaluation and final process census use one shared child-process owner. No subprocess timeout or exception kills or signals a child. All native census/proof/owner invocations retain the selected binary, command, PID and raw output. Observation timeouts retain `unfinished-child.json` when possible and keep in-memory uncertain-child state even if receipt I/O fails. An uncertain child stops later phases and source-tool activity; its PID and owner PID go into `unfinished-campaign.json`. No cleanup launches another occurrence.

Catalog cleanup registers an uncertain source query before any fallible receipt or raw-output check. While a child remains uncertain, the raw reuse postcheck is deferred and its receipt grants no acceptance or retry. A failed receipt write preserves the original exception and custody guard. Completed failures still check raw custody; ordinary abort publication is refused while any child remains uncertain.

For a completed failed phase with no uncertain child, the controller may publish the native owner's ordinary hash-bound abort DATA and observe its normal shutdown. This is not a process signal or a consensus fault. On failure, keep the complete campaign, sealed originals, A/B/C custody, exact signed activation/Payment/Unload bytes, network stores and process receipts. Do not rerun the controller on that directory, delete it, regenerate a target, reenroll, or choose a fresh directory as an automatic retry.

Recovery is an explicit read-only reconciliation of retained owner/child completion, accepted commands, finalized receipts, custody state and replay indexes. The controller provides no resume/sign/funding API and never treats absence of a result as absence of execution. Any subsequent recovery implementation must select the same retained originals and exact operation identities, and qualify its own replay behavior before publication. A new campaign requires an explicit new occurrence, never silent recovery of the failed one.

## Controller tests

The maintained synthetic suite is `pytests/scripts/kagemusha_network_acceptance/`. From the repository root:

```text
python3 -m pytest pytests/scripts/kagemusha_network_acceptance
```

It statically checks all selected native test module paths and ignore attributes, then uses local DATA and process fakes for six-stage ordering, wrong receipt/height/nonce bindings, clone and storage refusal, authenticated source loading, current compiler input/artifact mutations, and uncertain-child recovery. Fake tests do not establish complete native compiler closure, proof correctness/performance, issuer startup, real money execution or network release readiness. Retain real failures and raw receipts separately; no older capture becomes current because controller tests pass.
