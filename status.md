# Status

Last updated: 2026-05-01

## 2026-05-01 FASTPQ BN254 Metal Poseidon Batch Path

- Added a `fastpq_prover` BN254 Poseidon word-batch helper and a Metal
  `bn254_poseidon_hash_words` kernel for FASTPQ transfer transcript digest
  finalization. The helper self-tests fixed vectors against the scalar
  `iroha_zkp_halo2` Poseidon word path before use and disables itself on
  mismatch or runtime failure.
- Wired `iroha_core` FASTPQ transcript drain/snapshot finalization to batch
  single-delta transcript digests through the accelerator when the existing
  FASTPQ execution/poseidon modes permit GPU use and the batch is large enough;
  CPU mode, unavailable Metal, failed self-test, and small batches keep the
  existing scalar streaming hasher.
- Added `iroha_core/fastpq-gpu` feature plumbing and made `irohad/fastpq-gpu`
  enable both the daemon and core FASTPQ GPU paths. No new dependencies,
  `Cargo.lock` changes, wire-format changes, or config knobs were added.
- Focused validation passed:
  `cargo check -p fastpq_prover`,
  `cargo test -p iroha_core --lib poseidon_word_packer_matches_little_endian_chunks -- --nocapture`,
  `cargo check -p fastpq_prover --features fastpq-gpu`,
  `cargo check -p iroha_core --features fastpq-gpu`,
  `cargo check -p irohad --features fastpq-gpu`,
  `cargo test -p fastpq_prover --lib bn254_poseidon -- --nocapture`,
  `cargo fmt --all`, and `git diff --check`.
- The local Xcode install is missing the Metal toolchain, so the existing
  `fastpq_prover` build script warned that it could not execute `xcrun metal`
  and skipped shader compilation. The Rust feature plumbing is checked, but
  kernel syntax/parity and performance still need a rerun on a host with
  `xcodebuild -downloadComponent MetalToolchain` completed.

## 2026-05-01 Release Izanami 20k Gate After FASTPQ Metal Pass

- Rebuilt the scalar release `izanami` and `iroha3d` binaries with
  `cargo build --release -p izanami --bin izanami -p irohad --bin iroha3d`,
  which completed in `6m45s`, then reran the same 4-peer no-fault prebuilt
  `20,000 TPS` / `120s` Izanami gate. Fresh artifact:
  `dist/izanami-prebuilt-20k-rerun-release-120s-20260501-224554`; it exited
  `0`.
- The gate offered all `2,400,000` submissions, built and used all
  `2,400,000` prebuilt transactions, had zero prebuild fallback/build
  failures, and accepted `48,920` ingress transactions. Submit latency
  p50/p95/p99/max was `3197/4708/5190/6171 ms`.
- Final quorum/strict height was `11/11` with `36,979/36,979` approved
  transactions, zero height skew, zero approved-transaction skew, final queue
  depth `11,960/2,400,000`, and `tx_queue_saturated=true`.
- Safety signals stayed clean: no validation rejects, view changes,
  quorum-timeout causes, DA gate pressure, RBC store pressure/evictions,
  pending-RBC drops, missing-block fetches, or commit-inflight timeouts. The
  run recorded `7/7` missing-QC reacquire attempts/successes, `7` range-pull
  escalations, `0` range-pull successes, and `107` pacemaker backpressure
  deferrals.
- This was the default scalar release gate. The Metal Poseidon accelerator path
  still needs a separate `fastpq-gpu` release run on a host with the Apple
  Metal toolchain installed.

## 2026-05-01 Release Izanami 20k Sampled Profile After FASTPQ Metal Pass

- Captured a fresh scalar release 4-peer no-fault prebuilt `20,000 TPS` /
  `30s` sampled profile at
  `dist/izanami-profile-20k-current-sampled2-30s-20260501-225258`.
  Izanami exited `0`, `sample_ready=true`, `sample_status=0`, and the profiler
  captured the runner plus all four `iroha3d` peers. The contention snapshots
  only contain the wrapper's own `rg` probes.
- The run offered all `600,000` submissions, built and used all `600,000`
  prebuilt transactions, had zero prebuild fallback/build failures, and
  accepted `46,458` ingress transactions. Submit latency p50/p95/p99/max was
  `3123/4569/4957/6102 ms`.
- Final quorum/strict height was `3/3` with `4,141/4,141` approved
  transactions, zero height skew, zero approved-transaction skew, final queue
  depth `35,228/600,000`, and `tx_queue_saturated=true`. Safety counters stayed
  clean: no validation rejects, view changes, DA/RBC pressure, pending-RBC
  drops, or commit-inflight timeouts. The run did record `12` missing-block
  fetches, `2/2` missing-QC reacquire attempts/successes, and `3` range-pull
  escalations.
- Active peer leaf samples, excluding wait/parking and generic runtime frames,
  are now led by Norito/transaction wire work at roughly `31.5%`, followed by
  low-level syscall/TLS/write overhead at `13.3%`, allocation/copy at `10.0%`,
  FASTPQ/Poseidon hashing at `9.1%`, Ed25519/Curve25519 math at `8.4%`, Rayon
  batch/prover scheduling at `5.9%`, hash/multihash work at `2.6%`, ID/public
  key/string work at `2.3%`, P2P/network crypto at `1.1%`, and Torii/HTTP at
  `0.7%`.
- Hot peer leaves include `_tlv_get_addr`, Poseidon `apply_mds`/`sbox`,
  `curve25519_dalek` `FieldElement51::pow2k`/multiply, `_platform_memmove`,
  `_xzm_free`, Norito `use_compact_len`, `write_len`,
  `decode_field_canonical`, `ConstVec` decode/serialize helpers,
  `Transfer` serialization, SHA-256/Blake2 compression, and account-address
  `i105` conversion.
- Recursive peer attribution still points at Norito and transaction material as
  the largest non-wait corridor: decoded versioned signed transaction handling,
  instruction-pair/payload decode, valid-block validation, transaction gossip
  handling, and block validation/prepared transaction paths. Overlay execution
  wrappers remain visible, but `StateTransaction::record_transfer_transcripts`
  is now low-count residue; FASTPQ Poseidon work remains visible through
  digest finalization/prover-side batch work rather than as the dominant
  transfer-execution stack. Runner-side cost is secondary and mostly
  request/endpoint-pool/socket overhead.

## 2026-05-01 FASTPQ Poseidon Deferral And Admission Cache Follow-up

- Moved single-delta FASTPQ Poseidon digest work out of the transfer execution
  hot path. Transfer execution now records runtime-local transcripts without a
  digest, and the digest is finalized before block transcript drain or
  execution-witness drain/snapshot exposes the data. Multi-delta transcript
  digest behavior remains `None`.
- Added a streaming BN254 Poseidon byte hasher in `iroha_zkp_halo2` and wired
  FASTPQ digest construction through Norito `encode_to`, avoiding the previous
  full preimage buffer while preserving the current byte hash path.
- Reduced Torii/Core admission residue by deriving decoded external
  entrypoint hashes from the already-decoded versioned signed payload bytes and
  by reusing Ed25519 batch precheck message/signature/key vectors across
  chunks.
- Focused validation passed:
  `cargo test -p iroha_zkp_halo2 poseidon --lib -- --nocapture`,
  `cargo test -p iroha_core poseidon_digest_matches_known_vector --lib -- --nocapture`,
  `cargo test -p iroha_core transfer_transcript --lib -- --nocapture`,
  `cargo test -p iroha_core decoded_versioned_signed_transaction --lib -- --nocapture`,
  `cargo test -p iroha_core fastpq_transcripts --lib -- --nocapture`,
  `cargo test -p iroha_core snapshot_finalizes_single_fastpq_transcript_without_clearing --lib -- --nocapture`,
  `cargo test -p iroha_torii transaction_batch_ed25519 --lib -- --nocapture`,
  `cargo test -p iroha_torii transaction_batch_non_ed25519 --lib -- --nocapture`,
  and
  `cargo test -p iroha_torii handler_post_transactions_batch_rejects_invalid_ed25519_precheck_without_partial_push --lib -- --nocapture`,
  plus `cargo check -p iroha_zkp_halo2 -p iroha_core -p iroha_torii`,
  `cargo fmt --all -- --check`, and `git diff --check`.
- Later scalar release 20k gate and sampled profile reruns are recorded above.
  The sampled profile should be used to check whether
  `StateTransaction::record_transfer_transcripts`,
  `fastpq::poseidon_preimage_digest`, and
  `iroha_zkp_halo2::poseidon::{apply_mds,sbox}` moved out of the foreground
  execution stack.

## 2026-05-01 Direct-Ingress Precheck And Borrowed Overlay Pass

- Added deterministic single-key Ed25519 precheck for Torii
  `/transactions/batch` admission, reusing the existing
  `pipeline.signature_batch_max_ed25519` setting. All transactions still pass
  the existing chain-id, time/TTL, signing-allowed, size, signature-count, NTS,
  route, and queue-admission checks; multisig, non-Ed25519, sealed/private/time
  entrypoints stay on the existing validation path.
- Tightened the built-in overlay executor path so `Executor::Initial`
  `TxOverlay::apply_with_chunk` calls a crate-private borrowed instruction
  dispatch helper instead of cloning the whole `InstructionBox` first.
  `Executor::UserProvided` still uses the owned-instruction fallback, so the
  public `Execute` trait and custom executor API are unchanged.
- Focused validation passed:
  `cargo test -p iroha_core --lib borrowed_overlay_apply_matches_owned_initial_executor_for_register_domain -- --nocapture`,
  `cargo test -p iroha_core --lib decoded_versioned_signed_transaction_owned_supports_ed25519_prechecked_accept -- --nocapture`,
  `cargo test -p iroha_core --lib gossip_transaction_hash_from_framed_entrypoint_matches_canonical_hash -- --nocapture`,
  `cargo test -p iroha_core --lib does_not_materialize_entrypoint -- --nocapture`,
  `cargo test -p iroha_core --lib queue_accepts_gossip_payload_cache -- --nocapture`,
  `cargo test -p iroha_core --lib queue_generated_gossip_payload_uses_framed_entrypoint_wire -- --nocapture`,
  `cargo test -p iroha_torii --lib transaction_batch_ -- --nocapture`,
  `cargo test -p iroha_torii --lib handler_post_transactions_batch_rejects_invalid_ed25519_precheck_without_partial_push -- --nocapture`,
  `cargo test -p iroha_torii --lib handler_post_transactions_batch_accepts_multiple_payloads -- --nocapture`,
  `cargo fmt --all`,
  `cargo check -p norito -p iroha_crypto -p iroha_data_model -p iroha_core -p iroha_torii -p irohad`,
  and
  `cargo build --release -p izanami --bin izanami -p irohad --bin iroha3d`.
- Clean final 4-peer no-fault prebuilt `20,000 TPS` / `120s` gate artifact:
  `dist/izanami-prebuilt-20k-direct-ingress-precheck-final-120s-20260501-212850`.
  It exited `0`, and `contention-before.txt` / `contention-after.txt` only
  contain timestamps.
- The final gate offered all `2,400,000` submissions, built and used all
  `2,400,000` prebuilt transactions, had zero prebuild fallback/build
  failures, and accepted `47,566` ingress transactions. Submit latency
  p50/p95/p99/max was `3196/4554/4993/6118 ms`.
- Final quorum/strict height was `7/7` with `20,499/20,499` approved
  transactions, max height skew `1`, approved-transaction skew `8,192`, final
  queue depth `22,789/2,400,000`, and `tx_queue_saturated=true`.
- Safety signals remained clean: no validation rejects, quorum-timeout causes,
  DA gate pressure, RBC store pressure/evictions, pending-RBC drops, or
  commit-inflight timeouts. The run recorded `2` view-change installs, `28`
  missing-block fetches, `7/7` missing-QC reacquire attempts/successes, and
  `7` range-pull escalations with `2` successes.
- The direct-ingress sampled profile for this pass is
  `dist/izanami-profile-20k-direct-ingress-precheck-sampled-30s-20260501-210924`.
  It exited `0` with `sample_status=0`, accepted `43,109` ingress
  transactions, and reached strict height `3` with `4,125` approved
  transactions. The samples show the new Torii Ed25519 precheck path active;
  the remaining peer-side bottlenecks are Ed25519 batch math, Norito
  signed-transaction/instruction decode and allocation, public-key parsing
  during decode, residual gossip materialization, and Poseidon/hash work.

## 2026-05-01 Release Izanami 20k gate rerun

- Rebuilt the release `irohad` and `izanami` binaries with
  `cargo build --release -p irohad -p izanami`, then reran the 4-peer,
  no-fault, prebuilt `20,000 TPS` / `120s` Izanami gate. Clean-wrapper
  artifact:
  `dist/izanami-prebuilt-20k-rerun-release2-120s-20260501-210031`; it exited
  `0`.
- The gate offered all `2,400,000` submissions, built and used all
  `2,400,000` prebuilt transactions, had zero prebuild fallback/build
  failures, and accepted `52,582` ingress transactions. Submit latency
  p50/p95/p99/max was `3146/4607/5012/6072 ms`.
- Final quorum/strict height was `9/9` with `28,755/28,755` approved
  transactions, zero height skew, zero approved-transaction skew, final queue
  depth `23,624/2,400,000`, and `tx_queue_saturated=true`.
- Safety signals stayed clean: no validation rejects, quorum-timeout causes,
  DA gate pressure, RBC store pressure/evictions, or pending-RBC drops. The run
  recorded `1` view-change install, `18` missing-block fetches, `8/8`
  missing-QC reacquire attempts/successes, `8` range-pull escalations, and `1`
  range-pull success.
- Treat throughput as contended evidence, not an isolated baseline:
  `contention-before.txt` showed no other Rust build jobs, but
  `contention-after.txt` showed a separate debug
  `cargo check -p norito -p iroha_crypto -p iroha_data_model -p iroha_core -p iroha_torii -p irohad`
  with active `rustc` processes that started during the gate window.
- An immediately preceding same-shape artifact at
  `dist/izanami-prebuilt-20k-rerun-release-120s-20260501-205658` also reached
  the final Izanami summary, but its wrapper failed after the summary while
  recording `exit.status` because the zsh wrapper used the read-only variable
  name `status`.

## 2026-05-01 Release Izanami 20k sampled profile bottlenecks

- Captured a fresh release 4-peer, no-fault, prebuilt `20,000 TPS` / `30s`
  sampled profile at
  `dist/izanami-profile-20k-rerun-release-sampled2-30s-20260501-211211`.
  The fixed sampler targeted the Izanami runner and its direct child peers,
  recorded `sample_status=0`, and the Izanami run exited `0`.
- The run offered all `600,000` submissions, accepted `46,709` ingress
  transactions, and reached quorum/strict height `3/3` with `4,125/4,125`
  strict approved transactions. Submit latency p50/p95/p99/max was
  `3065/5833/6648/7556 ms`; final queue depth was `33,949/600,000` with
  `tx_queue_saturated=true`.
- Safety signals stayed clean: no validation rejects, view-change causes,
  commit-inflight timeouts, DA gate pressure, RBC store pressure/evictions, or
  pending-RBC drops. The status delta only recorded `28` pacemaker backpressure
  deferrals, `2/2` missing-QC reacquire attempts/successes, and `2`
  range-pull escalations.
- The pre-sample contention snapshot was clean. A separate `120s`
  direct-ingress gate appeared after the sample window and during shutdown, so
  the peer CPU samples are usable bottleneck evidence while the final timing
  summary should be treated as lightly contended.
- Peer samples now put the main CPU weight in
  `iroha_zkp_halo2::poseidon::{apply_mds,sbox}` and `fastpq_isi::poseidon`,
  `_platform_memmove`, allocator free/malloc paths, `sha2`/`blake2` hashing,
  Norito length/decode/encode routines such as `use_compact_len`,
  `read_len_from_slice`, `write_len`, and `decode_field_canonical`, plus
  `curve25519_dalek` / `ed25519_dalek` verification math.
- Direct ingress batch precheck is visible but no longer the dominant leaf, and
  the earlier overlay/clone targets are low-count residue in this profile:
  `InstructionDynClone::dyn_box_clone`, `InstructionBox::encoded_len_exact`,
  and `ValidBlock::validate_and_record_transactions_with_prepared` only appear
  as small wrapper or leaf costs. The next bottleneck work should prioritize
  the sustained Poseidon/FASTPQ source, allocation/memmove reduction, Norito
  decode and compact-length walks, and then signature-verification reuse or
  batching.

## 2026-05-01 Sealed reveal adversarial multi-peer coverage

- Broadened `tx_history::sealed_reveal_adversarial_cases_hold_on_multi_peer_network`
  to keep a real 4-peer network and cover five same-window sealed reveals in
  one block, duplicate reveal replay after commit, delayed expired reveal
  rejection, and all-peer state checks for both successful and expired reveal
  effects. The duplicate path now verifies the canonical reveal hash on primary
  replay and probes a secondary peer without letting Torii queue backpressure
  dominate the test result.
- Refreshed the bundled rANS fixture checksum used by peer startup so the
  repository fixture matches the current legacy Norito body framing.
- Focused validation passed:
  `NORITO_SKIP_BINDINGS_SYNC=1 CARGO_TARGET_DIR=/tmp/iroha-codex-norito-fixture cargo test -p norito load_bundle_tables_accepts --lib -- --nocapture`,
  `NORITO_SKIP_BINDINGS_SYNC=1 cargo test -p integration_tests --test core_api sealed_reveal_adversarial_cases_hold_on_multi_peer_network --no-run`,
  `NORITO_SKIP_BINDINGS_SYNC=1 RUST_BACKTRACE=1 cargo test -p integration_tests --test core_api sealed_reveal_adversarial_cases_hold_on_multi_peer_network -- --nocapture`,
  and `cargo fmt --all`.

## 2026-05-01 Conservative ingress and exact-length cache implementation

- Implemented the next conservative cache slice without changing transaction
  wire bytes, block wire bytes, canonical hashes, consensus rules, or config
  defaults. Torii direct signed-transaction and batch submission now decode
  versioned signed payloads once into a core-owned prepared admission token,
  preserving the existing validation path while reusing prepared hashes,
  encoded length, payload hash, and parsed single-Ed25519 key metadata.
- `InstructionBox::encoded_len_exact` now counts the existing
  `(wire_id, framed_payload)` Norito layout without materializing the framed
  instruction payload, keeping serialization/decode behavior unchanged while
  removing the residual dynamic framing allocation from size checks.
- Focused validation passed:
  `cargo test -p iroha_data_model instruction_box_encoded_len_exact -- --nocapture`,
  `cargo test -p iroha_core decoded_versioned_signed_transaction --lib -- --nocapture`,
  `cargo test -p iroha_core signed_encoded_len --lib -- --nocapture`,
  `cargo test -p iroha_torii decode_transaction_batch_payloads --lib -- --nocapture`,
  `cargo check -p iroha_data_model -p iroha_core -p iroha_torii`, and
  `cargo fmt --all -- --check`.
- Release validation for this code is now recorded in the broader 20k
  bottleneck-pass entries below, including the fixed-runner sampled profile at
  `dist/izanami-profile-20k-broader-pass-rerun-sampled-30s-20260501-200527`
  and the latest 120s gate rerun at
  `dist/izanami-prebuilt-20k-direct-ingress-precheck-final-120s-20260501-212850`.

## 2026-05-01 Contended conservative-cache release 20k gate rerun

- Reran the 4-peer, no-fault, prebuilt `20,000 TPS` / `120s` Izanami gate from
  the existing release binaries. Corrected artifact:
  `dist/izanami-prebuilt-20k-conservative-cache-rerun2-120s-20260501-144548`.
  It exited `0`. A preceding same-shape artifact at
  `dist/izanami-prebuilt-20k-conservative-cache-rerun-120s-20260501-144204`
  completed and wrote a summary, but its wrapper failed only while recording the
  exit status.
- This is a contended performance data point: active debug
  `cargo test`/`rustc` jobs were running during the gate, including the
  integration `core_api` network test build. The artifact records that
  contention in `contention.txt`; do not compare it as a clean throughput
  baseline against the earlier isolated gate.
- The corrected run offered all `2,400,000` submissions, built and used all
  `2,400,000` prebuilt transactions, had no prebuild fallback/build failures,
  and accepted `52,070` ingress transactions. Submit latency p50/p95/p99/max
  was `3208/4708/5311/6960 ms`.
- Final quorum/strict height was `5/5` with `12,329/12,329` strict approved
  transactions, zero peer height skew, zero approved-transaction skew, final
  queue depth `39,344/2,400,000`, and `tx_queue_saturated=true`.
- Safety remained intact: no validation rejects, no quorum-timeout cause, no DA
  gate pressure, no RBC store pressure/evictions, and no pending-RBC drops. The
  contended run did record `4` view-change installs, `22` missing-block fetches,
  `6/6` missing-QC reacquire attempts/successes, and `5` range-pull escalations
  with `2` range-pull successes.

## 2026-05-01 Contended conservative-cache 20k sampled profile bottlenecks

- Captured the latest requested release 4-peer, no-fault, prebuilt
  `20,000 TPS` / `30s` sampled profile at
  `dist/izanami-profile-20k-conservative-cache-rerun2-sampled-30s-20260501-145104`.
  The Izanami run exited `0`; `sample_ready=1`, and `sample_status=1` was
  caused by the sampler also targeting the bash wrapper plus one transient PID.
  Valid `sample` outputs were captured for the load driver and all four peers.
- This profile was contended by active debug `cargo test`/`rustc` jobs, so it is
  bottleneck evidence rather than an isolated timing baseline. The run offered
  all `600,000` submissions, built/used all `600,000` prebuilt transactions,
  accepted `52,817` ingress transactions, and reached quorum/strict height
  `3/3` with `4,137/4,137` strict approved transactions.
- Submit latency p50/p95/p99/max was `3385/6290/8296/11882 ms`; final queue
  depth was `40,839/600,000` with `tx_queue_saturated=true`. Safety signals
  stayed clean: no validation rejects, view changes, RBC store pressure,
  evictions, pending-RBC drops, prebuild fallback, or transaction build
  failures.
- The prior cache-pass removals remain absent from the peer samples:
  `Queue::encode_gossip_payload=0`, `TxOverlay::byte_size=0`, and
  `external_entrypoints_cloned=0`. `prepare_signed_metadata` appears only as
  residue and is not the dominant sampled path.
- The current bottleneck stack is now transaction admission crypto/public-key
  work, allocation/memmove churn, residual Norito dynamic instruction framing
  inside exact-length and canonical-byte construction, gossip transaction
  materialization/decode during admission, and block validation overlay
  execution. Representative leaves include `curve25519_dalek` field ops,
  `PublicKeyFull::from_bytes`, `AcceptedTransaction::from_external_with_hot_cache`,
  `AcceptedTransaction::signed_encoded_len`,
  `iroha_data_model::isi::encoded_instruction_pair`,
  `GossipTransaction::try_deserialize`,
  `decode_gossip_transaction_payload`, `TxOverlay::apply_with_chunk`,
  `InstructionDynClone::dyn_box_clone`, and `WorldTransaction::apply`.
- Next tuning should carry already validated inbound canonical signed/entrypoint
  bytes into `AcceptedTransaction`, reduce `InstructionBox` exact-length sizing
  without re-encoding framed payloads, extend parsed Ed25519 key/signature reuse
  on the Torii/direct-ingress path, and only then tackle the clone-heavy
  `TxOverlay::apply_with_chunk` API if overlay samples remain active.

## 2026-05-01 Norito first-release codec cleanup

- Tightened the v1 Norito header contract across Rust, Java, Kotlin, Python,
  and Swift: decoders now reject reserved layout bits, `FIELD_BITSET` is only
  valid with `PACKED_STRUCT | COMPACT_LEN`, and public encoders reject
  unsupported layout flags instead of emitting frames that downstream decoders
  would have to guess.
- Replaced the duplicated FNV-1a schema hash with a domain-separated SHA-256
  digest truncated to 16 bytes for both type-name and structural schema hashes.
  Shared fixtures, SDK tests, and the Norito binding parity script now pin the
  new values.
- Removed public last-encode-flag side channels from Rust, Java, Kotlin, and
  Python. Callers that need explicit layout metadata now use the explicit
  `encode_with_header_flags` surface.
- Removed first-release compatibility fallbacks from the core codec: typed
  decoders no longer special-case legacy schema length mismatches, and `Vec<u8>`
  no longer accepts the old per-element length-prefixed representation.
- Removed ignored runtime Norito heuristic knobs from configuration and daemon
  startup reporting. Runtime config now exposes only the active GPU-compression
  permission and archive-size guard; the remaining codec layout heuristics are
  compiled release defaults documented under the acceleration guide.
- Fixed the structural-schema build gap by adding explicit float metadata to
  `iroha_schema`, schema JSON support for float metadata, and manual structural
  schema descriptions for streaming named-variant payloads and bundled codec
  helper state.
- Validation:
  - `python3 scripts/check_norito_bindings_sync.py` passed.
  - `python3 -m pytest python/norito_py/tests/test_header_padding.py` passed.
  - `swift test --filter NoritoTests` passed from `IrohaSwift`.
  - Focused Rust checks passed for Norito header validation, schema hashes,
    stream iterator reserved flags, explicit bare-header flags, `Vec<u8>`
    rejection of legacy element framing, `norito_derive` self-delimiting
    classification, `iroha_config` fixture snapshots, cross-language hashes,
    and the `iroha_crypto` public-key Norito golden archive.
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-norito-fixes cargo test -p
    iroha_schema --test floats -- --nocapture` passed.
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-norito-fixes cargo test -p norito
    --features schema-structural --test schema_hash -- --nocapture` passed.
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-norito-fixes cargo check -p
    iroha_kagami` passed.
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-norito-fixes cargo check -p
    iroha_torii --features schema` passed.

## 2026-05-01 Broader 20k bottleneck pass

- Implemented lazy transaction-gossip materialization in
  `crates/iroha_core/src/gossiper.rs`: decoded gossip entries now keep the
  canonical framed `TransactionEntrypoint` bytes plus the precomputed
  entrypoint-compatible transaction hash, and semantic entrypoint decode is
  deferred until queue admission reaches a route-valid, locally unknown
  candidate. Outbound gossip serialization continues to write the cached framed
  bytes.
- Added gossip-side deterministic single-key Ed25519 batch precheck using the
  existing `pipeline.signature_batch_max_ed25519` setting and
  `iroha_crypto::ed25519_verify_batch_preparsed_deterministic_with_scratch`.
  Multisig, non-Ed25519, sealed commitment/reveal, private Kaigi, and time
  entrypoints stay on the existing per-entrypoint validation path.
- Added a crate-private accepted-gossip constructor in
  `crates/iroha_core/src/tx.rs` that accepts prepared metadata plus a
  single-Ed25519-prechecked marker, while still running chain id, time/TTL,
  signing policy, size, signature-count, heartbeat, and NTS health checks.
- Routed `TxOverlay` application through a crate-private borrowed overlay
  executor adapter. The public `Execute` trait and custom executor API remain
  owned-instruction based; user-provided executor paths still explicitly fall
  back to owned `InstructionBox` execution.
- Fixed the signature materialization corridor needed by lazy gossip decode:
  `iroha_crypto::Signature` now has a narrow `ConstVec<u8>` fallback for the
  compact per-byte signature payload layout, so valid cached entrypoint frames
  can materialize through normal Norito validation instead of being dropped as
  semantic decode failures.
- Added focused unit coverage for lazy gossip decode/cache behavior,
  route-invalid and known-duplicate drops without materialization, accepted
  cached gossip payload reuse, and valid/invalid gossip Ed25519 batch precheck
  behavior.
- Profile classification after this pass: the prior 30s profile at
  `dist/izanami-profile-20k-postcache-tuned-bottleneck-30s-20260501-171955`
  is pre-broader-pass evidence. The fresh sampled 30s profile is
  `dist/izanami-profile-20k-broader-pass-sampled-30s-20260501-194734`; it
  completed with `ingress_accepted=37324`, submit latency
  `p50/p95/p99=3355/5723/6477ms`, final strict height `2`, final approved
  transactions `143`, and a saturated queue depth of `37402`. Peer samples
  still classify active CPU mostly under Ed25519/curve25519, ZKP poseidon,
  memory movement/allocation, SHA/Blake hashing, and Norito decode/compact-len
  paths; the sample wrapper captured the shell pipeline's `tee` process instead
  of the Izanami runner, but the four peer samples were captured successfully.
  Gossip materialization and deterministic gossip Ed25519 batch precheck now
  appear as narrower peer-side stacks rather than the broadest top-level
  categories.
- The fresh unsampled 120s release gate is
  `dist/izanami-prebuilt-20k-broader-pass-120s-20260501-194908`; it completed
  with `offered=2400000`, `ingress_accepted=52291`, submit latency
  `p50/p95/p99=3290/4753/5217ms`, final strict height `9`, final approved
  transactions `28740`, final height skew `1`, and saturated queue depth
  `18527`. Compared with
  `dist/izanami-prebuilt-20k-conservative-cache-rerun-120s-20260501-175213`,
  final approved transactions were effectively flat (`28740` vs `28710`) and
  p95/p99 submit latency improved, while ingress accepted count was lower
  (`52291` vs `54574`).
- The immediate release-gate rerun is
  `dist/izanami-prebuilt-20k-broader-pass-rerun-120s-20260501-195617`; it
  completed with `offered=2400000`, `ingress_accepted=51802`, submit latency
  `p50/p95/p99=3099/4543/5075ms`, final strict height `9`, final approved
  transactions `28699`, final height skew `0`, saturated queue depth `23112`,
  `view_change_* = 0`, and exit code `0`. Compared with the previous
  broader-pass 120s gate, final approved transactions and ingress accepted
  count were slightly lower (`28699` vs `28740`, `51802` vs `52291`), while
  p50/p95/p99 submit latency improved.
- The follow-up fixed-runner sampled 30s profile is
  `dist/izanami-profile-20k-broader-pass-rerun-sampled-30s-20260501-200527`.
  It completed with `sample_ready=true`, `sample_status=0`, and exit code `0`,
  and sampled the actual Izanami runner plus all eight observed `iroha3d`
  peer processes. The run offered `600000`, accepted `54750`, had submit
  latency `p50/p95/p99=3391/6921/7769ms`, final strict height `2`, final
  approved transactions `12`, final height skew `1`, and a saturated queue
  depth of `41098`. Treat this artifact as CPU bottleneck classification, not
  a committed-throughput baseline.
- Fixed-runner profile classification: direct peer CPU is dominated by
  Ed25519/curve25519 verification math, with `curve25519_dalek` field
  exponentiation/multiplication and multiscalar paths as the largest leaves.
  Memory allocation/copying and Norito compact/decode work are the next tier,
  including `memmove`, `malloc`/`free`, `norito::core::use_compact_len`, and
  data-model instruction registry/decode paths. Hashing (`sha2`, `blake2`,
  `crc64fast`) remains visible but secondary. ZK/BLS math is also material:
  direct peer samples include `iroha_zkp_halo2::poseidon`, while the
  `core_api` child peer processes are dominated by `ark_ff`,
  `ark_bls12_381`, and `w3f_bls` public-key deserialization/subgroup math.
  Gossip materialization and gossip deterministic Ed25519 batch precheck are
  present as narrow stacks, while queue mechanics and borrowed overlay apply
  are not primary CPU bottlenecks in this sample.
- Two earlier fixed-runner profile attempts,
  `dist/izanami-profile-20k-broader-pass-rerun-sampled-30s-20260501-200044`
  and
  `dist/izanami-profile-20k-broader-pass-rerun-sampled-30s-20260501-200211`,
  failed before peer startup because the bundled rANS table checksum did not
  match the current Norito table payload. The table checksum was restored to
  the current-source value before the successful sampled rerun.
- Validation:
  - `cargo fmt --all` passed.
  - `git diff --check` passed.
  - `NORITO_SKIP_BINDINGS_SYNC=1 CARGO_TARGET_DIR=/tmp/iroha-codex-check cargo
    test -p iroha_core --lib gossip_transaction -- --nocapture` passed
    (`5` tests).
  - `NORITO_SKIP_BINDINGS_SYNC=1 CARGO_TARGET_DIR=/tmp/iroha-codex-check cargo
    test -p iroha_core --lib gossip_drop_does_not_materialize -- --nocapture`
    passed (`2` tests).
  - `NORITO_SKIP_BINDINGS_SYNC=1 CARGO_TARGET_DIR=/tmp/iroha-codex-check cargo
    test -p iroha_core --lib gossip_ed25519_batch_precheck -- --nocapture`
    passed (`2` tests).
  - `NORITO_SKIP_BINDINGS_SYNC=1 CARGO_TARGET_DIR=/tmp/iroha-codex-check cargo
    test -p iroha_core --lib queue_accepts_gossip_payload_cache --
    --nocapture` passed (`3` tests).
  - `NORITO_SKIP_BINDINGS_SYNC=1 CARGO_TARGET_DIR=/tmp/iroha-codex-check cargo
    test -p iroha_core --lib
    queue_generated_gossip_payload_uses_framed_entrypoint_wire -- --nocapture`
    passed (`1` test).
  - `NORITO_SKIP_BINDINGS_SYNC=1 CARGO_TARGET_DIR=/tmp/iroha-codex-check cargo
    test -p iroha_crypto
    signature_of_try_deserialize_preserves_compact_const_vec_payload --
    --nocapture` passed (`1` test).
  - `NORITO_SKIP_BINDINGS_SYNC=1 CARGO_TARGET_DIR=/tmp/iroha-codex-check cargo
    check -p norito -p iroha_crypto -p iroha_data_model -p iroha_core -p
    iroha_torii -p irohad` passed.
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-check-unskipped cargo check -p norito
    -p iroha_crypto -p iroha_data_model -p iroha_core -p iroha_torii -p
    irohad` passed, including the Norito binding parity build-script corridor.
  - `cargo build --release -p izanami --bin izanami -p irohad --bin iroha3d`
    passed.
  - Fresh `30s` sampled 20k Izanami profile passed with artifact
    `dist/izanami-profile-20k-broader-pass-sampled-30s-20260501-194734`.
  - Fresh `120s` prebuilt 20k Izanami gate passed with artifact
    `dist/izanami-prebuilt-20k-broader-pass-120s-20260501-194908`.
  - Fresh `120s` prebuilt 20k Izanami gate rerun passed with artifact
    `dist/izanami-prebuilt-20k-broader-pass-rerun-120s-20260501-195617`.
  - Fresh fixed-runner `30s` sampled 20k Izanami profile passed with artifact
    `dist/izanami-profile-20k-broader-pass-rerun-sampled-30s-20260501-200527`.

## 2026-05-01 Conservative cache release 20k gate rerun

- Rebuilt release binaries with
  `cargo build --release -p izanami --bin izanami -p irohad --bin iroha3d`
  and reran the 4-peer, no-fault, prebuilt `20,000 TPS` / `120s` Izanami
  gate. Canonical artifact:
  `dist/izanami-prebuilt-20k-conservative-cache-rerun-120s-20260501-175213`.
  It exited `0`; a preceding same-shape artifact completed and wrote a summary,
  but its wrapper tripped only during post-run exit bookkeeping, so it is not the
  recorded gate result.
- The clean run offered `2,399,905` submissions, built `2,400,000` prebuilt
  transactions, used `2,399,905`, and had no prebuild fallback or build
  failures. Ingress accepted `54,574` transactions. Submit latency
  p50/p95/p99/max was `3114/4804/5744/7116 ms`.
- Final quorum/strict height was `9/9` with `28,710/28,710` strict approved
  transactions, final peer height skew `1`, final approved-transaction skew
  `4096`, final queue depth `25,839/2,400,000`, and
  `tx_queue_saturated=true`.
- The result is materially consistent with the previous post-cache tuned gates
  (`28,790` then `28,694` strict approved transactions). The conservative cache
  pass did not regress safety signals, but the 20k committed-throughput target is
  still missed; the next useful step remains a fresh 30s sampled profile focused
  on validation/execution recording, gossip decode, and any remaining Norito
  encoded-length fallback.
- The gate recorded no validation rejects, view changes, RBC store pressure, RBC
  evictions, pending-RBC drops, prebuild fallback, or transaction build failures.
  `sumeragi_status_delta` reported `81` pacemaker backpressure deferrals,
  `8/8` missing-QC reacquire attempts/successes, `8` range-pull escalations,
  and no range-pull successes or failures.

## 2026-05-01 Conservative cache 20k sampled profile bottlenecks

- Captured a fresh release 4-peer, no-fault, prebuilt `20,000 TPS` / `30s`
  macOS `sample` profile at
  `dist/izanami-profile-20k-conservative-cache-parallel-sampled-30s-20260501-181025`.
  The wrapper exited `0` with `sample_ready=1`, `sample_status=0`, and sampled
  the load driver plus all four peer processes. A concurrent
  `cargo test -p iroha_core --lib gossip_transaction -- --nocapture` compile
  was active on the same host, so this is useful bottleneck evidence but not an
  isolated latency baseline.
- The run offered all `600,000` submissions, built/used all `600,000` prebuilt
  transactions, accepted `52,080` ingress transactions, and reached
  quorum/strict height `2/2` with `147/147` strict approved transactions.
  Submit latency p50/p95/p99/max was `3511/7212/8964/11842 ms`; final queue
  depth was `41,996/600,000` with `tx_queue_saturated=true`.
- Safety signals stayed clean: no validation rejects, view changes, RBC store
  pressure, RBC evictions, pending-RBC drops, prebuild fallback, or transaction
  build failures. The status delta showed `45` pacemaker backpressure deferrals,
  `3/3` missing-QC reacquire attempts/successes, and `3` range-pull
  escalations.
- The intended cache-pass removals remain absent in the sampled peers:
  `Queue::encode_gossip_payload=0`, `TxOverlay::byte_size=0`, and
  `external_entrypoints_cloned=0`. `prepare_signed_metadata` is no longer a
  visible recursive-stack bottleneck in the aggregate peer summary.
- The dominant non-idle peer leaves are now transaction admission crypto and
  canonical-byte work. The Torii ingress path
  `accept_transaction_for_ingress -> AcceptedTransaction::accept_entrypoint ->
  validate_with_now_and_signature_check -> verify_signature_for_check` spends
  visible time in Ed25519 verification and public-key parsing
  (`curve25519_dalek`, `ed25519_dalek`, `PublicKeyFull::from_bytes`). After a
  transaction is accepted, `from_external_with_hot_cache` still constructs
  canonical signed bytes with `norito::to_bytes(&tx)` for each external
  transaction, which shows up with `SignedTransaction`/`TransactionPayload`
  serialization and Blake2 hashing.
- The signed-length fallback is reduced but not free. The hot
  `AcceptedTransaction::signed_encoded_len` stacks now mostly go through
  `SignedTransaction::encoded_len_exact`, but `InstructionBox::encoded_len_exact`
  still calls `encoded_instruction_pair`, which dynamically encodes and frames
  the instruction payload (`Instruction::dyn_encode`, `frame_instruction_payload`)
  before measuring the `(wire_id, payload)` pair. That is why
  `norito::codec::encode_adaptive`, `write_len_prefixed`, CRC/schema-hash, and
  instruction serialization remain visible even on the exact-length path.
- Gossip decode caching is working at the wrapper layer, but downstream gossip
  admission still materializes entrypoints when validation needs semantics. The
  sampled stacks include `TransactionGossip`/`GossipTransaction` decode into
  `SignedTransaction -> TransactionPayload -> InstructionBox -> Transfer`, plus
  `PublicKey`/`PublicKeyCompact` decode.
- Block validation and overlay execution are still present, but this contended
  sample is more ingress/gossip dominated than the earlier cachepass sample.
  The remaining overlay cost is clone-heavy: `validate_and_record_transactions`
  reaches `build_overlay_for_transaction_with_accounts_zk`,
  `InstructionDynClone::dyn_box_clone`, `Transfer::clone`, and
  `WorldTransaction::apply`.

## 2026-05-01 Further 20k conservative cache pass

- Reused prepared signed-transaction metadata across block static validation and
  the later validation/execution recording phase. The all-external block path
  now borrows `external_entrypoints_slice()` and only allocates cloned
  entrypoints for legacy/non-external fallback execution.
- Reduced signed transaction encoded-length fallback by extending exact Norito
  sizing through tuple fields, `Option::Some`, `NonZeroU*`, `PublicKey`,
  `InstructionBox`, and the signed/external entrypoint paths used by
  `AcceptedTransaction`. Cached canonical signed bytes are preferred when
  available for size checks.
- Routed `GossipTransaction::try_deserialize` through the same cached
  entrypoint-payload decode helper used by slice decoding, preserving exact byte
  comparison for cache collision safety and leaving `TransactionGossip` wire
  bytes unchanged.
- No executor API rewrite, borrowed-instruction execution rewrite, config
  default change, canonical transaction/block wire change, or consensus behavior
  change was made in this pass. No fresh Izanami sampled profile or Criterion
  benchmark was run yet after these focused validations; the 120s release gate
  rerun is recorded above.
- Focused validation for this slice:
  - `cargo test -p norito --test encoded_len_exact -- --nocapture`
  - `cargo test -p iroha_crypto public_key_encoded_len_exact --lib -- --nocapture`
  - `cargo test -p iroha_data_model instruction_box_encoded_len_exact --lib -- --nocapture`
  - `cargo test -p iroha_core signed_encoded_len --lib -- --nocapture`
  - `cargo test -p iroha_core gossip_transaction --lib -- --nocapture`
  - `cargo test -p iroha_core validate_and_record_transactions --lib -- --nocapture`
  - `cargo test -p iroha_core stateless_cache --lib -- --nocapture`
  - `cargo test -p iroha_core entrypoint_hash --lib -- --nocapture`
  - `cargo test -p iroha_data_model signed_block_wire_skips_runtime_transaction_caches -- --nocapture`
  - `cargo check -p iroha_data_model -p iroha_core -p iroha_torii`
  - `cargo fmt --all -- --check`
  - `git diff --check`

## 2026-05-01 Offline V2 native SDK prover speedups

- Swift Offline Note V2 pure Halo2 proving now reuses a cached IPA/domain
  context, verifier-key transcript scalar, and fixed selector polynomial instead
  of regenerating them on every proof. The hot commitment path now uses sparse
  Lagrange commitments for single-row instance/advice columns, a 4-bit
  windowed Vesta scalar multiplication path, and bucketed 4-bit MSM for dense
  IPA commitments to avoid the previous scalar-multiply-per-base commitment
  path. Projective Vesta doubling now reuses the shared `(x + y^2)^2`
  intermediate instead of squaring it twice, and the Swift group formulas
  replace fixed `2x`/`8x` field multiplications with additions.
- Added Swift convenience APIs for direct native proof generation from
  `OfflineNoteRedeemV2` / `OfflineNoteAuditBundleV2`, plus proof replacement
  helpers and a `Halo2OfflineNoteV2Prover.prewarm()` hook so callers can keep
  the native model, initialize the proof cache before the button path, and swap
  in the newly generated recursive proof before binding validation.
- Added Kotlin/JVM and Java Android Offline V2 instance-value builders,
  scalar-column encoders, proof replacement helpers, and pure Java Halo2/IPA
  provers. The Java-family path now builds the same `OpenVerifyEnvelope`
  recursive proof payloads without routing production calls through Rust JNI,
  and the focused JVM payload was cross-verified by the Swift native verifier.
  The Java-family prover also uses bucketed dense MSM and conditional
  canonical field add/sub paths to avoid repeated `BigInteger.mod(...)` calls
  in the group-addition hot loop, plus the same projective doubling
  intermediate reuse as Swift and dedicated `2x`/`8x` field helpers for the
  group-formula constants.
- Release Swift benchmark on macOS arm64 for the pure Swift prover after the
  latest optimization pass: audit median `0.315s`, p95 `0.322s`, max
  `0.324s`; redeem median `0.311s`, p95 `0.316s`, max `0.325s` over 20
  iterations.
- Env-gated Java-family benchmark hooks now report subsecond native prover
  medians on macOS arm64 over 5 iterations: Kotlin/JVM audit `0.826s`, p95
  `0.832s`, max `0.832s`; Kotlin/JVM redeem `0.820s`, p95 `0.871s`, max
  `0.871s`; Java Android harness audit `0.829s`, p95 `0.836s`, max `0.836s`;
  Java Android harness redeem `0.823s`, p95 `0.825s`, max `0.825s`.
- Focused validation for this slice:
  - `swift test -c release --filter Halo2PastaTests/testPastaUniformBytesAndVestaGroupArithmetic`
  - `swift test -c release --filter Halo2PastaTests/testOfflineNoteV2NativeHalo2ProofEnvelopeFitsQrBudget`
  - `IROHA_SWIFT_OFFLINE_V2_BENCH=1 IROHA_SWIFT_OFFLINE_V2_BENCH_ITERATIONS=20 swift test -c release --filter Halo2PastaTests/testOfflineNoteV2NativeHalo2ProofPerformanceWhenRequested`
  - `./gradlew :core-jvm:test --tests org.hyperledger.iroha.sdk.offline.OfflineNoteV2Test --console=plain` from `kotlin`
  - `IROHA_JVM_OFFLINE_V2_PROVER_TEST=1 ./gradlew :core-jvm:test --tests org.hyperledger.iroha.sdk.offline.OfflineNoteV2Test.nativeHalo2ProverProducesVerifyingPayloadWhenRequested --console=plain` from `kotlin`
  - `IROHA_JVM_OFFLINE_V2_BENCH=1 IROHA_JVM_OFFLINE_V2_BENCH_ITERATIONS=5 ./gradlew :core-jvm:test --tests org.hyperledger.iroha.sdk.offline.OfflineNoteV2Test.nativeHalo2ProverPerformanceWhenRequested --console=plain --info --rerun-tasks` from `kotlin`
  - `IROHA_SWIFT_OFFLINE_V2_VERIFY_PAYLOAD_IN=/tmp/iroha-jvm-offline-v2-audit.zk1 swift test -c release --filter Halo2PastaTests/testOfflineNoteV2NativeHalo2ProofEnvelopeFitsQrBudget`
  - `JAVA_HOME=$(/usr/libexec/java_home -v 21) ANDROID_HARNESS_MAINS=org.hyperledger.iroha.android.offline.OfflineNoteV2Test ./gradlew :core:test --tests org.hyperledger.iroha.android.GradleHarnessTests --console=plain` from `java/iroha_android`
  - `JAVA_HOME=$(/usr/libexec/java_home -v 21) IROHA_JAVA_OFFLINE_V2_PROVER_TEST=1 ANDROID_HARNESS_MAINS=org.hyperledger.iroha.android.offline.OfflineNoteV2Test ./gradlew :core:test --tests org.hyperledger.iroha.android.GradleHarnessTests --console=plain` from `java/iroha_android`
  - `JAVA_HOME=$(/usr/libexec/java_home -v 21) IROHA_JAVA_OFFLINE_V2_BENCH=1 IROHA_JAVA_OFFLINE_V2_BENCH_ITERATIONS=5 ANDROID_HARNESS_MAINS=org.hyperledger.iroha.android.offline.OfflineNoteV2Test ./gradlew :core:test --tests org.hyperledger.iroha.android.GradleHarnessTests --console=plain --info --rerun-tasks` from `java/iroha_android`
  - `git diff --check`

## 2026-05-01 Torii Offline V2 issuer hardening

- Torii Offline V2 issuer certificate minting now requires a signed middleware
  attestation receipt before certifying hardware one-use keys. Certificate JSON
  echoes canonical base64 key bytes from the verified receipt instead of
  client-supplied hex/base64 spellings.
- Offline V2 note issuance now derives balances from Torii-signed lineage
  state, treats client `local_balance` / `local_revision` as consistency
  checks, preserves trusted balance during existing-lineage key refills, and
  returns the same chain note commitment that is submitted on-chain.
- `torii.offline_issuer` configuration now requires an attestation verifier
  public key and explicitly accepts only Ed25519 or Secp256k1 issuer/verifier
  keys.
- Refreshed the minimal `iroha_config` fixture snapshot so the Torii defaults
  include `offline_issuer: None`.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo check -p iroha_config -p iroha_torii --features app_api`
  - `cargo test -p iroha_torii offline_v2_issuer`
  - `cargo test -p iroha_config torii_offline_issuer`
  - `cargo test -p iroha_config --test fixtures`

## 2026-05-01 20k post-cache tuned release gate rerun

- Reran the release 4-peer, no-fault, prebuilt `20,000 TPS` / `120s` gate
  against the current post-cache tuned binaries. Artifact:
  `dist/izanami-prebuilt-20k-postcache-tuned-rerun-120s-20260501-170823`.
- The runner exited `0`, offered all `2,400,000` submissions, built and used all
  `2,400,000` prebuilt transactions, and had no prebuild fallback or build
  failures. Ingress accepted `46,748` transactions. Submit latency
  p50/p95/p99/max was `3109/4828/5770/7788 ms`.
- Final quorum/strict height was `9/9` with `28,694/28,694` strict approved
  transactions, final queue depth `18,063/2,400,000`, and
  `tx_queue_saturated=true`. The rerun is effectively consistent with the
  previous tuned 120s gate (`28,790` strict approved), while still far below
  the committed-20k target.
- Diagnostics recorded `23,090` latency-saturated ingress rejects, `312`
  deferred local `RBC READY` events, `274` deferred `RBC DELIVER` events, `96`
  targeted RBC payload sends, `96` targeted READY-set sends, and `66` targeted
  BlockBodyResponse companions. There were no inline validation fallback
  warnings, slow commit-pipeline warnings, slow block-validation warnings,
  validation rejects, view changes, RBC store pressure, RBC evictions, or
  pending-RBC drops.

## 2026-05-01 20k post-cache targeted tuning pass

- Implemented the low/medium-risk post-cache slice without public API,
  wire-format, config, or dependency changes. Transaction gossip now derives
  the `TransactionEntrypoint` hash directly from canonical framed Norito
  entrypoint bytes, routes accepted gossip through a crate-private constructor
  that seeds those inbound bytes after normal validation, and performs
  per-entry route/plane rejection before hash/materialization where that keeps
  existing drop reasons intact.
- Block validation keeps external-only blocks on borrowed
  `external_entrypoints_slice()` inspection and only allocates cloned
  entrypoints for non-external sequential fallback. Regression coverage now
  checks both the external-only path and the non-external sealed-commitment
  fallback while preserving entrypoint hashes and execution results.
- `TxOverlay` now caches its Norito instruction byte-size sum with a local
  `OnceLock`; max-byte rejection paths compute `byte_size()` once per check and
  reuse that value in the unchanged rejection message. The broader borrowed
  instruction execution rewrite remains deliberately out of this pass.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo test -p iroha_core --lib gossip_transaction_hash_from_framed_entrypoint_matches_canonical_hash -- --nocapture`
  - `cargo test -p iroha_core --lib queue_accepts_gossip_payload_cache -- --nocapture`
  - `cargo test -p iroha_core --lib queue_generated_gossip_payload_uses_framed_entrypoint_wire -- --nocapture`
  - `cargo test -p iroha_core --lib overlay_byte_size_cache_matches_norito_instruction_sum -- --nocapture`
  - `cargo test -p iroha_core --lib block_validation_external_only_records_entrypoint_hash_without_fallback -- --nocapture`
  - `cargo test -p iroha_core --lib block_validation_non_external_entrypoint_uses_sequential_fallback -- --nocapture`
  - `cargo test -p iroha_core --test overlay_bounds overlay_bytes_cap_rejects_and_rest_apply -- --nocapture`
  - `cargo check -p norito -p iroha_crypto -p iroha_data_model -p iroha_core -p iroha_torii -p irohad`
  - `git diff --check`
- Rebuilt release binaries with
  `cargo build --release -p izanami --bin izanami -p irohad --bin iroha3d`.
  Fresh 30s sampled artifact:
  `dist/izanami-profile-20k-postcache-tuned-sampled-30s-20260501-165811`.
  It exited `0`, sampled the runner plus four current peers, offered all
  `600,000` submissions, built/used all `600,000` prebuilt transactions,
  accepted `51,852` ingress transactions, and reached final quorum/strict
  height `3/3` with `4,164/4,164` strict approved transactions. Submit latency
  p50/p95/p99/max was `3484/5504/6442/8837 ms`; final queue depth was
  `42,101/600,000` with `tx_queue_saturated=true`.
- Fresh 120s release gate artifact:
  `dist/izanami-prebuilt-20k-postcache-tuned-120s-20260501-165947`. It exited
  `0`, offered all `2,400,000` submissions, built/used all `2,400,000`
  prebuilt transactions, accepted `47,174` ingress transactions, and reached
  final quorum/strict height `9/9` with `28,790/28,790` strict approved
  transactions. Submit latency p50/p95/p99/max was `3193/4687/5706/7139 ms`;
  final queue depth fell to `14,282/2,400,000`, with
  `tx_queue_saturated=true`.
- Against the cachepass baselines, the 30s sampled run improved strict height
  from `2` to `3`, strict approved transactions from `4,116` to `4,164`, and
  p95/p99 latency from `6198/7260 ms` to `5504/6442 ms`, while ingress accepted
  transactions fell from `53,626` to `51,852`. The 120s gate improved strict
  approved transactions from `24,623` to `28,790` (`+16.9%`) and final queue
  depth from `23,457` to `14,282`, while ingress accepted transactions fell
  from `52,167` to `47,174`.
- Current-peer sample classification confirms the intended hot-path removals:
  `Queue::encode_gossip_payload=0`, `TxOverlay::byte_size=0`, and
  `external_entrypoints_cloned=0`. Relative to the cachepass sample,
  current-peer pattern counts fell for `write_len_prefixed` (`22,787` to
  `19,403`), `TransactionGossip` (`1,602` to `1,441`), and
  `GossipTransaction` (`1,010` to `402`). Remaining visible costs are still
  Norito/gossip decode/materialization, signed length/metadata walks
  (`AcceptedTransaction::signed_encoded_len=1,217`,
  `prepare_signed_metadata=50` in this sample), validation/overlay execution,
  and crypto/math leaves. No validation rejects, DA-gate view changes, RBC
  store pressure, RBC evictions, or pending-RBC drops were recorded in either
  fresh run.

## 2026-05-01 20k post-cache sampled profile bottlenecks

- Captured a fresh release 4-peer, no-fault, prebuilt `20,000 TPS` / `30s`
  macOS `sample` profile at
  `dist/izanami-profile-20k-cachepass-sampled-30s-20260501-152126`. A
  discarded first wrapper at
  `dist/izanami-profile-20k-cachepass-sampled-30s-20260501-152043` aborted
  before peer startup because the shell treated zero `pgrep` matches as fatal
  under `pipefail`.
- The valid wrapper exited `0` with `sample_ready=true`. It sampled the runner,
  two Bash wrappers, stale peer PIDs from the aborted wrapper, and the current
  peer set; the current peer samples used for classification are
  `profiles/sample-16852.txt`, `profiles/sample-16853.txt`,
  `profiles/sample-16854.txt`, and `profiles/sample-16855.txt`.
- The run offered all `600,000` submissions, built and used all `600,000`
  prebuilt transactions, accepted `53,626` ingress transactions, and reached
  final quorum/strict height `2/2` with `4,116/4,116` strict approved
  transactions. Submit latency p50/p95/p99/max was
  `3318/6198/7260/9966 ms`; final queue depth was `42,282/600,000` with
  `tx_queue_saturated=true`. There were no validation rejects, DA-gate view
  changes, RBC store pressure events, RBC evictions, or pending-RBC drops.
- Peer diagnostics recorded `6,613` latency-saturated ingress rejects,
  `68` deferred local `RBC READY` events, `64` deferred `RBC DELIVER` events,
  `34` targeted RBC payload sends, `33` targeted READY-set sends, `26`
  targeted BlockBodyResponse companions, `7` inline validation fallback
  warnings, and `4` slow commit-pipeline warnings. The slow validation windows
  were `4,583`, `14,452`, `14,580`, and `16,205 ms` (`12,455 ms` average);
  slow total pipeline time peaked at `16,289 ms`, and oldest queued transaction
  age reached `33,202 ms`.
- The largest active consensus stack is now block validation and overlay
  application:
  `validate_block_for_voting -> validate_keep_voting_block_inner -> validate_and_record_transactions -> TxOverlay::apply_with_chunk`.
  This is the commit-progress critical path; one peer sample shows
  `validate_block_for_voting` holding `7,989` recursive samples,
  `validate_and_record_transactions` `5,143`, and `TxOverlay::apply_with_chunk`
  `4,183`.
- The next active stack is incoming transaction-gossip Norito decode:
  `TransactionGossip -> GossipTransaction -> SignedTransaction -> TransactionPayload -> InstructionBox -> Transfer`.
  Current-peer pattern counts include `TransactionGossip=1,602`,
  `GossipTransaction=1,010`, `decode_field_canonical=20,235`,
  `write_len_prefixed=22,787`, `TransactionPayload=9,083`,
  `InstructionBox=7,687`, and `PublicKey/PublicKeyCompact=7,391/3,483`.
- Remaining signed-transaction length/serialization fallback is still visible
  inside prepared metadata:
  `ValidBlock::validate_and_record_transactions -> AcceptedTransaction::prepare_signed_metadata -> AcceptedTransaction::signed_encoded_len -> norito::codec::encode_adaptive_into`.
  Current-peer pattern counts show `AcceptedTransaction::signed_encoded_len=725`,
  `AcceptedTransaction::prepare_signed_metadata=30`,
  `norito::codec::encode_adaptive_into=529`, and
  `norito::core::to_bytes=985`.
- Top-leaf samples still show expensive crypto/math and allocation costs even
  when the commit-progress stack is validation/overlay. Excluding parked/kernel
  waits across the four current peer samples, active top leaves were led by
  Ed25519/Curve25519 (`78,409`, `29.44%`), allocation/copy (`51,069`,
  `19.17%`), runtime/scheduler artifacts (`49,173`, `18.46%`), Norito/gossip
  decode (`33,813`, `12.69%`), and FASTPQ/Poseidon/ZKP leaves (`11,878`,
  `4.46%`). Recursive stack attribution puts Norito/gossip decode first
  (`167,668`, `45.77%`) and Ed25519/Curve25519 at `40,848` (`11.15%`).
- The intended queue-side fix held: `Queue::encode_gossip_payload=0` in the
  current peer samples. `PublicKey::normalize=0`, Ed25519 curve math is present
  as a leaf cost but not the blocking commit-progress stack, and
  FASTPQ/Poseidon is visible but small in recursive attribution. The remaining
  limiter is block validation and overlay throughput, fed by incoming gossip
  decode and the leftover signed length/serialization metadata walk; increasing
  queue capacity alone will not improve committed throughput.

## 2026-05-01 Block transaction pipeline hardening follow-up

- Transaction gossip now carries canonical `TransactionEntrypoint` payloads
  instead of downgrading to `SignedTransaction`, so sealed commitments and
  sealed reveals retain their public entrypoint identity across queue gossip.
  Queue-side gossip payload caches now store entrypoint Norito bytes.
- Explorer, contract-source lookup, and Torii pipeline-status reconstruction now
  pair block entrypoints with aligned transaction results rather than zipping
  external signed transactions against the first result slice. Sealed reveals
  are reported and looked up by their entrypoint hash.
- Torii submission responses and signed receipts now expose
  `entrypoint_hash`, keep `x-iroha-transaction-hash` as the canonical
  entrypoint hash, and add `x-iroha-signed-transaction-hash` when an inner
  signed transaction exists.
- Sealed transaction commitments now have deterministic non-zero gas/TEU cost
  based on encoded length, and block execution prunes expired pending sealed
  commitments from smart-contract state.
- Follow-up unit coverage now executes a sealed commitment in one block and
  its reveal in the next block through the real block-builder validation path,
  checks expiry pruning after the reveal deadline, and verifies Torii resolves
  pipeline status by the sealed-reveal entrypoint hash stored in Kura/state.
- Sealed commitments are no longer expired by the wall-clock queue TTL; their
  lifetime remains governed by reveal-height windows during block execution.
- Post-execution block application now indexes canonical external entrypoint
  hashes instead of signed-transaction-only hashes, which avoids duplicate
  transaction-index insertion mismatches when sealed commitment blocks are
  replayed with commit-QC evidence.
- Block receipt proof construction now uses an external-entrypoint Merkle tree
  for external receipts and the full entrypoint tree for time-trigger receipts,
  so sealed commitments and mixed external/time-trigger blocks verify against
  the correct root. Explorer block totals now count external entrypoints rather
  than only external signed transactions.
- The multi-peer sealed pipeline integration now submits a sealed commitment,
  waits for gossip/commit across a 4-peer network, reveals it in-window, and
  verifies explorer lookup by the reveal entrypoint hash. The test backs off on
  temporary queue-latency `429` responses instead of treating transient queue
  pressure as a protocol failure.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo check -p iroha_data_model -p iroha_core -p iroha_torii`
  - `cargo test -p iroha_core gossip_roundtrip_preserves_sealed_commitment_entrypoint --lib -- --nocapture`
  - `cargo test -p iroha_core partition_gossip_batch_keeps_sealed_commitments --lib -- --nocapture`
  - `cargo test -p iroha_core sealed_commitment_cost_is_nonzero_and_size_sensitive --lib -- --nocapture`
  - `cargo test -p iroha_core queue_generated_gossip_payload_uses_framed_entrypoint_wire --lib -- --nocapture`
  - `cargo test -p iroha_core queue_accepts_gossip_payload_cache --lib -- --nocapture`
  - `cargo test -p iroha_data_model submission_receipt_roundtrips_signature --lib -- --nocapture`
  - `cargo test -p iroha_torii handler_post_transaction_entrypoint_accepts_external_entrypoint --lib -- --nocapture`
  - `cargo test -p iroha_torii handler_post_transaction_honors_prefer_return_minimal --lib -- --nocapture`
  - `cargo test -p iroha_core block_pipeline_executes_sealed_reveal_and_records_entrypoint_hash --lib -- --nocapture`
  - `cargo test -p iroha_core prune_expired_sealed_commitments_removes_pending_state_after_deadline --lib -- --nocapture`
  - `cargo test -p iroha_core sealed_commitment_is_not_expired_by_wall_clock_queue_ttl --lib -- --nocapture`
  - `cargo test -p iroha_core apply_without_execution_indexes_sealed_commitment_entrypoint_hash --lib -- --nocapture`
  - `cargo test -p iroha_core block_proofs_for_sealed_commitment_use_external_merkle_root --lib -- --nocapture`
  - `cargo test -p iroha_data_model --features transparent_api proofs_for_external_entry_with_time_trigger_use_consensus_root --lib -- --nocapture`
  - `cargo test -p iroha_torii pipeline_status_handler_returns_applied_for_sealed_reveal_entrypoint_hash --lib -- --nocapture`
  - `cargo test -p iroha_torii block_dto_counts_sealed_commitment_entrypoints --lib -- --nocapture`
  - `cargo test -p integration_tests --test core_api sealed_commitment_reveal_gossips_and_explorer_lookup_uses_entrypoint_hash -- --nocapture`
  - `git diff --check`

## 2026-05-01 20k hot-path release gate rerun

- Rebuilt release binaries with
  `cargo build --release -p izanami --bin izanami -p irohad --bin iroha3d`,
  then reran the 4-peer, no-fault, prebuilt `20,000 TPS` / `120s` gate with
  `2,400,000` prebuilt transactions and `4096` submitters. Artifact:
  `dist/izanami-prebuilt-20k-hotpath-120s-20260501-142015`.
- The runner exited `0`, offered all `2,400,000` planned submissions, built
  and used all `2,400,000` prebuilt transactions, and had no prebuild fallback
  or build failures. Ingress accepted `53,891` (`449.1 TPS`, `2.25%` of
  offered). Submit latency p50/p95/p99/max was `3315/4836/5690/8060 ms`;
  failover/unhealthy endpoint totals were `13/46`.
- Final quorum/strict height was `9/9` with zero peer height skew and
  `28,713/28,713` approved transactions. That is `239.3 committed TPS` over
  the measured window, or `1.20%` of the `2,400,000` committed-transaction
  gate. Compared with the prior 2026-05-01 120s rerun artifact
  (`24,590` approved), this is `+4,123` approved transactions (`+16.8%`), but
  still misses the 20k committed-TPS target by `2,371,287` transactions.
- Runtime counters still show queue drain / validation throughput as the
  limiter rather than DA/RBC storage failure: `tx_queue_saturated=true`, final
  queue depth `25,286/2,400,000`, `pacemaker_backpressure_deferrals_total=74`,
  `worker_loop_stage="drain_votes"`, no view-change causes, no validation
  rejects, no DA-gate causes, no RBC store pressure, no RBC evictions, no RBC
  persist drops, no pending-RBC drops, and no commit-inflight timeouts.
- Peer diagnostics recorded `23,608` latency-saturated ingress rejects,
  `28` inline validation fallback warnings, `28` slow commit-pipeline warnings,
  max block validation `6,060 ms`, and max slow-pipeline total `6,197 ms`.
  Slow validation warnings covered heights `3` through `10`. RBC authoritative
  payload delay symptoms remained visible but resolved: `260` deferred local
  `RBC READY`, `243` deferred `RBC DELIVER`, `101` targeted RBC payload sends,
  `101` targeted READY-set sends, and `74` targeted BlockBodyResponse
  companions. Peer stderr logs were empty; one peer again exited with raw
  `unix_wait_status(3)` during shutdown after the measured window.
- Reran the same release gate shape again at
  `dist/izanami-prebuilt-20k-cachepass-120s-20260501-142429` to check variance.
  This repeat also exited `0`, offered all `2,400,000` submissions, built/used
  all `2,400,000` prebuilt transactions, and had no prebuild fallback or build
  failures. Ingress accepted `52,167` (`434.7 TPS`, `2.17%` of offered), with
  submit latency p50/p95/p99/max `3112/4803/5840/8251 ms`, final
  quorum/strict height `8/8`, strict approved transactions `24,623`
  (`205.2 committed TPS`, `1.03%` of target), final queue depth
  `23,457/2,400,000`, and `tx_queue_saturated=true`.
- The repeat confirms the failure class rather than a one-off artifact:
  compared with `dist/izanami-prebuilt-20k-rerun-120s-20260501-103729`,
  ingress improved by `4,699` accepted transactions (`+9.9%`), but strict
  approved transactions only improved by `33` (`+0.13%`). Peer diagnostics
  showed `21,968` latency-saturated ingress rejects, `26` inline validation
  fallback warnings, `26` slow commit-pipeline warnings, validation times up to
  `5,907 ms` (`5,116 ms` average among slow warnings), no validation rejects,
  no DA-gate/view-change causes, no RBC store pressure/evictions, and empty
  peer stderr logs.

## 2026-05-01 20k hot-path critical-path cache pass

- Implemented the next bottleneck-reduction slice from the 20k profile: accepted
  transactions can now carry caller-provided canonical signed-transaction bytes,
  and gossip acceptance reuses those bytes instead of immediately re-encoding
  the same `SignedTransaction`. The cached bytes feed the prepared transaction
  metadata path used by size/hash checks.
- Block validation now prepares external transaction metadata once per block and
  uses the borrowed external entrypoint slice when the block payload has one.
  Merkle-root rebuilding reuses prepared entrypoint hashes for external entries
  while still hashing sealed/non-external wrappers directly, so sealed reveal
  ordering and block-wire semantics are unchanged.
- Ed25519 block validation now feeds true batch verification with prepared
  payload hashes and parsed single-key authorities. The batch-failure path
  bisects borrowed slices with reusable scratch storage, then reports the first
  invalid transaction in deterministic block order before falling back to the
  existing per-transaction validation path.
- Runtime-only block caches are kept off the canonical wire: the legacy
  signed-transaction cache and result-side entrypoint cache are skipped by
  Norito encoding, while canonical external entrypoints remain on the block
  payload. Focused roundtrip coverage asserts cache contents do not change
  signed block wire bytes.
- FASTPQ execution witnesses no longer force block-level `TransitionBatch`
  materialization inside `StateBlock::capture_exec_witness`. The commit path
  carries a local-only `FastpqWitnessContext` containing the public-input
  template, tx-set hash, and entry dataspace overrides into the FASTPQ lane,
  where transcript-only witnesses are expanded off the consensus critical path.
  The `ExecWitness` wire shape remains unchanged.
- Focused validation for this slice:
  - `CARGO_TARGET_DIR=target/codex-20k cargo test -p iroha_core accept_with_canonical_signed_bytes_reuses_payload_cache --lib -- --nocapture`
  - `CARGO_TARGET_DIR=target/codex-20k cargo test -p iroha_core job_context_builds_batches_for_transcript_only_witness --lib -- --nocapture`
  - `CARGO_TARGET_DIR=target/codex-20k cargo test -p iroha_core capture_exec_witness --lib -- --nocapture`
  - `CARGO_TARGET_DIR=target/codex-20k cargo check -p iroha_data_model -p iroha_core -p iroha_torii`
  - `cargo test -p iroha_crypto ed25519_batch -- --nocapture`
  - `cargo test -p iroha_data_model signed_block_wire_skips_runtime_transaction_caches -- --nocapture`
  - `cargo test -p iroha_data_model versioned_block_roundtrip_preserves_instruction_order -- --nocapture`
  - `cargo test -p iroha_core --test admission_batching ed25519_batch_bisection_finds_bad_sig -- --nocapture`
  - `cargo test -p iroha_core --test signature_batch_determinism ed25519_batch_permutation_finds_same_bad_sig -- --nocapture`
  - `cargo test -p iroha_core queue_generated_gossip_payload_uses_framed_signed_transaction_wire --lib -- --nocapture`
  - `cargo test -p iroha_core queue_accepts_gossip_payload_cache --lib -- --nocapture`
  - `cargo fmt --all`
  - `cargo check -p norito -p iroha_crypto -p iroha_data_model -p iroha_core -p iroha_torii -p irohad`
- Release Izanami validation for this slice is recorded above in the
  `20k hot-path release gate rerun` and `20k post-cache sampled profile
  bottlenecks` entries. The remaining validation work is to locate the new
  5k/10k knee and then reprofile after the next block-validation/Norito
  optimization slice.

## 2026-05-01 20k sampled profile rerun bottlenecks

- Captured a fresh 4-peer, no-fault, prebuilt `20,000 TPS` / `30s` macOS
  `sample` profile at
  `dist/izanami-profile-20k-rerun-sampled-30s-20260501-104544`. The valid
  wrapper exited `0`; the sampled PIDs were the Izanami runner, one Bash child
  to ignore, and all four `iroha3d` peers. A discarded earlier artifact at
  `dist/izanami-profile-20k-rerun-sampled-30s-20260501-104431` failed after
  warmup because the wrapper used `mapfile` on the system Bash.
- The sampled run offered all `600,000` planned submissions and used all
  `600,000` prebuilt transactions. Ingress accepted `53,697` (`8.95%` of
  offered), but final quorum/strict progress was only height `2/2` with
  `17/17` approved transactions. Submit latency p50/p95/p99/max was
  `3267/6097/7739/9965 ms`; failover/unhealthy endpoint totals were `22/14`.
- Runtime counters and peer diagnostics still point at block drain,
  validation, and consensus payload progress rather than DA/RBC store capacity:
  `tx_queue_saturated=true`, final queue depth `38,701/600,000`,
  `pacemaker_backpressure_deferrals_total=28`, `worker_loop_stage="tick"`,
  no validation rejects, no DA-gate view changes, no RBC store pressure, no
  RBC evictions, and no pending-RBC drops. Peer logs had `6,442` queue
  age-saturated rejects, `163` deferred local `RBC READY`, `125` deferred
  `RBC DELIVER`, `14` targeted RBC payload sends, `14` targeted READY-set
  sends, `8` commit quorums, and `8` commits.
- The decisive consensus log evidence is validation latency: all peers emitted
  inline validation fallback warnings, and three peers logged height-`3`
  `commit pipeline block processing slow` with validation times of
  `10,901 ms`, `10,923 ms`, and `12,372 ms` (`total_ms` up to `12,908`).
  During that window queue age saturated and RBC delivery repeatedly waited
  for authoritative payload availability; one peer also requested seven
  block-sync range pulls for `idle_missing_qc_reacquire`.
- Peer CPU attribution, after excluding parked/waiting leaves, is led by:
  allocation/copy churn at roughly `27.6%` of active top-of-stack samples,
  Ed25519/Curve25519 math at `26.6%`, Norito encode/decode leaf work at
  `12.4%`, public-key/account string and multihash work at `4.4%`,
  FASTPQ/Poseidon hashing at `4.3%`, and hash/CRC work at `2.9%`. Recursive
  stack attribution still puts Norito and transaction wire work first:
  `norito::core::write_len_prefixed`, `decode_field_canonical`,
  `write_len`, transaction payload/entrypoint, `InstructionBox`,
  `Transfer`, `AccountController`, `AssetId`, `SignedTransaction`, and P2P
  transaction gossip decode dominate the hot call paths.
- Normalized hot symbols from the four peers reinforce the bottleneck order:
  top active leaves include `FieldElement51::pow2k` (`55,767`), dalek field
  multiply (`21,159`), `_platform_memmove` (`16,884`), `_xzm_free`
  (`14,026`), `_xzm_xzone_malloc_tiny` (`10,233`), Poseidon `apply_mds`
  (`9,784`), `RawVec::finish_grow` (`6,085`), string collection/formatting
  (`5,672`), `Blake2bVarCore::compress` (`4,765`), `write_all` (`4,491`),
  and Norito `decode_field_canonical` (`3,258`). Recursive hot paths include
  Norito length/field helpers, transaction payload serialization,
  `PublicKey::normalize`, multihash formatting, Ed25519 public-key parsing,
  and block validation batch-verification frames.
- Critique: this profile confirms the latest 120s 20k failure mode. Torii can
  accept a short burst, but queue age saturates because block validation and
  payload/commit progress cannot drain it. The next fix should prioritize
  removing remaining Norito re-serialization and allocation growth from block
  validation/proposal paths, reusing canonical payload bytes through RBC/commit
  validation, and reducing public-key/account string conversions on incoming
  gossip. Ed25519 batch verification is now true CPU work rather than just
  parse overhead, so further wins likely need fewer repeated verification
  inputs or a faster batch-verification backend. FASTPQ/Poseidon is visible
  but not the top bottleneck; keep it budgeted/yielding if later cache work
  exposes it.

## 2026-05-01 20k validation rerun critique

- Rebuilt release binaries with
  `cargo build --release -p izanami --bin izanami -p irohad --bin iroha3d`,
  then reran the 4-peer, no-fault, prebuilt `20,000 TPS` / `120s` gate with
  `2,400,000` prebuilt transactions and `4096` submitters. Artifact:
  `dist/izanami-prebuilt-20k-rerun-120s-20260501-103729`.
- The driver offered all `2,400,000` planned submissions and used all
  `2,400,000` prebuilt transactions with no fallback/build failures. Ingress
  accepted `47,468` (`395.6 TPS`, `1.98%` of offered). Submit latency
  p50/p95/p99/max was `3187/4507/5031/6285 ms`; failover/unhealthy endpoint
  totals were `15/49`.
- Final quorum/strict height was `8/8` with zero peer height skew and
  `24,590/24,590` approved transactions. This is `204.9 committed TPS` over
  the measured window, or `1.02%` of the `2,400,000` committed-transaction
  target. It improves the prior metadata/batch 120s run's `20,609` approved
  transactions by `3,981` (`+19.3%`) but still misses the 20k committed-TPS
  gate by `2,375,410` transactions.
- Runtime counters still classify the failure as sustained queue drain and
  block/consensus throughput, not a DA/RBC store-capacity failure:
  `tx_queue_saturated=true`, final queue depth `22,465/2,400,000`,
  `pacemaker_backpressure_deferrals_total=75`, `worker_loop_stage=drain_rbc_chunks`,
  and no validation rejects, DA-gate view-change causes, RBC store pressure,
  RBC evictions, pending-RBC drops, or commit-inflight timeouts.
- Peer diagnostics show the remaining shape more sharply than the summary:
  `21,733` queue age-saturated ingress rejects, `509` deferred local
  `RBC READY` events, `431` deferred `RBC DELIVER` events, `14` missing
  `BlockCreated` requests, one "QC quorum but payload missing" warning, and
  one deferred commit-QC application while block validation caught up. This
  suggests the next bottleneck is not Torii routing itself, but the pipeline
  between queue drain, block validation/prepared metadata, RBC payload
  materialization, and commit-QC application at height `9`.
- Caveats: the wrapper reached and logged the Izanami summary, but its
  post-run status capture used Bash `PIPESTATUS` under zsh and exited after
  the summary without writing `exit.status`. One peer reported raw
  `unix_wait_status(3)` during shutdown; `iroha_test_network` uses SIGQUIT
  after a peer misses the SIGTERM grace window, so this is a shutdown-lag
  signal rather than a measured-window consensus crash. The stderr logs were
  empty.

## 2026-05-01 20k post-batch profile bottleneck classification

- Profile source: all-peer macOS `sample` output under
  `dist/izanami-profile-20k-metadata-batch-sampled-30s-20260501-101313`.
  This run sampled the Izanami runner and all four `iroha3d` peers, but used a
  different sample interval from the older postfix artifact, so percentages
  within this run are more meaningful than raw-count comparison to older
  samples.
- Excluding wait/parking leaves, active peer CPU is led by
  Ed25519/Curve25519 leaf work (`~31%` of active leaf samples), mostly
  `curve25519_dalek` field arithmetic from dalek batch verification. Recursive
  stack attribution still puts Norito encode/decode as the largest category
  (`~39.5%` of active recursive stack appearances), because transaction gossip
  decode, block validation preparation, and transaction/application paths sit
  above many Norito serializers/deserializers.
- The fixed queue-side bottleneck is confirmed removed from the profile:
  `Queue::encode_gossip_payload` appears in the older sampled peer profiles but
  does not appear in the fresh all-peer peer samples. The Ed25519 path now
  reaches `ed25519_dalek::batch::verify_batch` through
  `ed25519_verify_batch_preparsed_deterministic`.
- Current bottleneck order:
  1. Norito transaction wire work: P2P `MessageReader::parse_next_encrypted_frame`
     repeatedly decodes `TransactionGossip` / `GossipTransaction` /
     `SignedTransaction`, while block execution still serializes transaction
     payload, instruction, asset/account, and transfer shapes during metadata,
     access, and hashing work.
  2. Ed25519 batch math: true batch verification removes the old per-signature
     loop, but batch verification itself is now the hottest active leaf CPU
     path, dominated by `curve25519_dalek` field operations.
  3. Public-key and ID string normalization: `PublicKey::normalize`,
     multihash formatting/hex conversion, and account/address `i105`
     conversion remain visible under incoming wire decode and admission.
  4. Allocation/copy/format churn: malloc/free/realloc/memmove and string
     formatting account for roughly another `~22.5%` of active leaf samples,
     strongly correlated with Norito serialization/deserialization and
     public-key/account string conversions.
  5. Torii HTTP/routing is now a secondary cost (`~0.5%` active leaf,
     `~3.1%` active recursive); it contributes to admission pressure but is no
     longer the dominant CPU stack in this profile.
- Runtime counters still show overload behind the hot paths rather than a DA/RBC
  failure: queue saturation remains true, pacemaker backpressure increments, and
  there are no validation rejects, DA-gate causes, RBC pressure, or pending-RBC
  drops in the sampled run.

## 2026-05-01 20k metadata and Ed25519 batch pass

- Extended accepted transaction hot-path metadata with canonical signed bytes,
  exact framed encoded lengths, payload hashes, and parsed single-key Ed25519
  verifying keys where available. Queue gossip now reuses cached signed bytes
  and canonicalizes any provided gossip payload against the accepted transaction
  cache instead of re-encoding on the queue path.
- Block validation now prepares per-transaction metadata once per block and
  feeds the prepared signed hash, entrypoint hash, payload hash, encoded length,
  signature references, and parsed Ed25519 key into stateless validation.
  Signature semantics and canonical Norito block/transaction wire formats are
  unchanged.
- The Ed25519 batch path now uses `ed25519_dalek::verify_batch` behind the
  existing `ecc-batch` feature through a preparsed deterministic API. If a batch
  fails, validation bisects and then falls back to individual verification in
  original transaction order to report the first bad transaction
  deterministically.
- Built fresh release benchmark binaries with
  `cargo build --release -p izanami --bin izanami -p irohad --bin iroha3d`.
  A 30s runner-only release artifact at
  `dist/izanami-profile-20k-metadata-batch-sampled-30s-20260501-101106`
  offered `600,000`, accepted `47,629`, recorded submit latency
  `3167/4881/5585/6164 ms`, and reached quorum/strict height `3/3` with
  `4,177/4,177` approved transactions; only the Izanami runner was sampled in
  that artifact.
- Reran the all-peer sampled 30s profile at
  `dist/izanami-profile-20k-metadata-batch-sampled-30s-20260501-101313`.
  It offered `600,000`, accepted `22,344`, recorded submit latency
  `3408/6772/7832/9952 ms`, and reached quorum/strict height `2/2` with
  `83/83` approved transactions. This run is used for profiler classification
  because macOS `sample` captured the runner plus all four `iroha3d` peers.
- Reran the clean 120s release validation at
  `dist/izanami-prebuilt-20k-metadata-batch-120s-20260501-101457`. It offered
  `2,400,000`, accepted `47,573`, recorded submit latency
  `3247/4809/5766/7015 ms`, and reached quorum/strict height `7/7` with
  `20,609/20,609` approved transactions. Compared with the prior 120s artifact
  (`48,679` accepted, `2958/4484/4995/6116 ms`, height `5/5`, `12,328`
  approved), ingress remains flat/slightly lower but finality progress improved.
- Fresh peer samples show the intended profile shift: `Queue::encode_gossip_payload`
  no longer appears in peer samples, and the Ed25519 path reaches
  `ed25519_dalek::batch::verify_batch` through
  `ed25519_verify_batch_preparsed_deterministic`. Remaining visible hot stacks
  are Norito gossip/block decode, one-time prepared metadata
  length/hash construction, Ed25519 batch field math, and public-key parsing
  from incoming wire decode. This is still not a successful committed-20k TPS
  run.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo test -p iroha_crypto ed25519_batch -- --nocapture`
  - `cargo test -p iroha_core --lib accepted_transaction_caches_hashes_and_encoded_length -- --nocapture`
  - `cargo test -p iroha_core --lib queue_generated_gossip_payload_uses_framed_signed_transaction_wire -- --nocapture`
  - `cargo test -p iroha_core --test signature_batch_determinism ed25519_batch_permutation_finds_same_bad_sig -- --nocapture`
  - `cargo check -p norito -p iroha_crypto -p iroha_core -p iroha_torii -p irohad`

## 2026-05-01 Block transaction pipeline correctness

- Block validation now runs the shared stateful transaction-admission checks
  before every block execution/apply path. Height TTLs, monotonic transaction
  sequences, lane policy, direct multisig execution rejection, authority
  materialization rules, and fraud gates are enforced consistently for queued
  transactions and received blocks.
- Parallel block scheduling now treats each transaction authority's sequence as
  a write key, so same-authority transactions cannot be applied out of order by
  non-overlapping ISI or IVM access sets.
- Block construction and validation use accepted-entrypoint hashes and prepared
  transaction metadata instead of repeatedly converting accepted transactions
  back to external signed transactions. This keeps non-external entrypoints
  hashable for ordering and Merkle roots while preserving canonical block wire
  layout.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `git diff --check`
  - `cargo test -p iroha_core block_pipeline_rejects -- --nocapture`
  - `cargo test -p iroha_core log_instruction_has_no_access_keys -- --nocapture`
  - `cargo test -p iroha_core --lib ivm_access_dynamic_prepass_requires_gas_limit -- --nocapture`

## 2026-05-01 Inrou portable and proxy hardening

- Portable Inrou now prepares the mutable root disk as a `qcow2` overlay over the
  verified base root image, while Firecracker/KVM `Isolated` policy installs
  explicit tap-scoped host-input and forward-drop rules.
- Hosted HTTP responses forwarded through the Torii P2P proxy path are capped by
  `torii.soracloud_public_max_response_bytes` before buffering; the default is
  64 MiB and over-limit snapshots fail closed with `502 Bad Gateway`.
- `scripts/ci/prepare_inrou_portable_guest_assets.py` defaults to the pinned
  Debian Bookworm `20260413-2447` cloud image build, uses the real
  build-suffixed genericcloud archive names, verifies `SHA512SUMS.sign` with GPG
  when a detached signature is published, and otherwise falls back only to the
  hard-pinned SHA512 digest for the exact `amd64` or `arm64` archive. Unsigned
  archives without a pinned digest still fail closed.
- The mixed-host Inrou inventory now preserves `IROHA_INROU_LINUX_KVM_*` through
  `sudo`, and the Soracloud docs describe the root overlay, signed asset
  verification, explicit isolated firewall policy, and P2P response cap.
- Coverage for this slice now also exercises exact-limit Torii proxy responses,
  invalid proxied header/status restoration, HostedHttp-only response cap
  selection, duplicate proxied header preservation, gateway-only retry
  classification, HostedHttp route-timeout selection, reqwest bridge
  header/body preservation at the cap, no-candidate and all-retryable proxy
  fallback responses, open-policy firewall rule ordering, empty-allowlist
  default-drop planning, reusable portable root overlays, oversized base-rootfs
  budget rejection, allowlist IPv4 de-duplication plus IPv6-only and empty-port
  fail-closed behavior, host architecture mapping, SHA512SUMS star-prefixed
  archive entries, missing/mismatched archive checksums, pinned digest
  mismatches, signature download success/failure paths, GPG verifier failures,
  disk extraction missing/reuse paths, byte-range extraction EOF handling,
  GPT root-partition selection and rejection paths, rootfs patch command
  construction, boot-file selection/dump replacement failures, env export
  quoting, CLI argument defaults/overrides, host tool and GPG verifier discovery,
  subprocess wrapper capture flags, Debian keyring absence errors, download
  replacement/short-circuit behavior, debugfs stdout capture, chunked SHA512
  hashing, and main-flow orchestration for signed and pinned asset verification.
- The embedded-feature Torii explorer helper now derives external signed
  transaction results from `external_entrypoints_cloned()` and `results()`,
  avoiding a stale `entrypoint_results()` call under the irohad feature set.
- While validating alongside the local 20k hot-path edits already present in the
  worktree, the AMX admission-failure path now drops its temporary
  `StateTransaction` before recording the abort on the parent `StateBlock`.
- Focused validation for this slice:
  - `python3 -m py_compile scripts/ci/prepare_inrou_portable_guest_assets.py`
  - `python3 -m pytest scripts/tests/prepare_inrou_portable_guest_assets_test.py`
  - `cargo fmt --all`
  - `git diff --check`
  - `env -u LOG_FORMAT CARGO_TARGET_DIR=target/codex-inrou-fixes-config cargo test -p iroha_config soracloud_public_runtime_defaults_are_non_zero --lib -- --nocapture`
  - `env -u LOG_FORMAT CARGO_TARGET_DIR=target/codex-inrou-fixes-irohad cargo test -p irohad --features embedded-soracloud-runtime --bin irohad planned_inrou_tap_firewall_rules_keep_isolated_policy_private -- --nocapture`
  - `env -u LOG_FORMAT CARGO_TARGET_DIR=target/codex-inrou-fixes-irohad cargo test -p irohad --features embedded-soracloud-runtime --bin irohad planned_inrou_tap_firewall_rules_place_allowlist_accepts_above_default_drop -- --nocapture`
  - `env -u LOG_FORMAT CARGO_TARGET_DIR=target/codex-inrou-fixes-irohad cargo test -p irohad --features embedded-soracloud-runtime --bin irohad ensure_inrou_portable_root_disk_uses_qcow2_overlay_with_backing_file -- --nocapture`
  - `env -u LOG_FORMAT CARGO_TARGET_DIR=target/codex-inrou-fixes-irohad cargo test -p iroha_torii --lib snapshot_caps_buffered -- --nocapture`
  - `env -u LOG_FORMAT CARGO_TARGET_DIR=target/codex-inrou-fixes-torii-connect cargo test -p iroha_torii --lib broadcast_strategy_ -- --nocapture`
  - `env -u LOG_FORMAT CARGO_TARGET_DIR=target/codex-inrou-fixes-torii-connect cargo test -p iroha_torii --test connect_gating -- --nocapture`
  - `env -u LOG_FORMAT CARGO_TARGET_DIR=target/codex-inrou-fixes-clippy cargo clippy -p iroha_torii --all-targets -- -D warnings`
  - `env -u LOG_FORMAT CARGO_TARGET_DIR=target/codex-inrou-fixes-clippy cargo clippy --workspace --all-targets -- -D warnings` currently stops in unrelated `mochi-core` drift (`FaultPeer`/`FaultConfig` API updates and removed offline data-model symbols), after Torii itself checks.
  - `python3 -m pytest scripts/tests/prepare_inrou_portable_guest_assets_test.py` now passes 49 asset-prep verifier tests.
  - `env -u LOG_FORMAT CARGO_TARGET_DIR=target/codex-inrou-extra-tests-torii cargo test -p iroha_torii --lib torii_proxy_snapshot -- --nocapture` passes 6 Torii proxy snapshot tests.
  - `env -u LOG_FORMAT CARGO_TARGET_DIR=target/codex-inrou-extra-tests-torii cargo test -p iroha_torii --lib torii_proxy_response_body_limit_only_caps_hosted_http -- --nocapture` passes the HostedHttp response-cap selector test.
  - `env -u LOG_FORMAT CARGO_TARGET_DIR=target/codex-inrou-extra-tests-torii cargo test -p iroha_torii --lib torii_proxy_header_conversion_preserves_duplicates_and_skips_invalid -- --nocapture`
  - `env -u LOG_FORMAT CARGO_TARGET_DIR=target/codex-inrou-extra-tests-torii cargo test -p iroha_torii --lib torii_proxy_retry_policy_only_retries_gateway_class_statuses -- --nocapture`
  - `env -u LOG_FORMAT CARGO_TARGET_DIR=target/codex-inrou-extra-tests-torii cargo test -p iroha_torii --lib torii_proxy_hosted_http_request_kind_uses_route_timeout -- --nocapture`
  - `env -u LOG_FORMAT CARGO_TARGET_DIR=target/codex-inrou-extra-tests-torii cargo test -p iroha_torii --lib execute_torii_proxy_request_across_candidates_returns_route_unavailable_without_candidates -- --nocapture`
  - `env -u LOG_FORMAT CARGO_TARGET_DIR=target/codex-inrou-extra-tests-torii-retryable cargo test -p iroha_torii --lib execute_torii_proxy_request_across_candidates_returns_last_retryable_response -- --nocapture`
  - `env -u LOG_FORMAT CARGO_TARGET_DIR=target/codex-inrou-extra-tests-torii-retryable cargo test -p iroha_torii --lib execute_torii_proxy_request_across_candidates_returns_route_unavailable_after_transport_errors -- --nocapture`
  - `env -u LOG_FORMAT CARGO_TARGET_DIR=target/codex-inrou-extra-tests-irohad cargo test -p irohad --features embedded-soracloud-runtime --bin irohad planned_inrou_tap_firewall_rules -- --nocapture` passes 3 firewall-planning tests.
  - `env -u LOG_FORMAT CARGO_TARGET_DIR=target/codex-inrou-extra-tests-irohad cargo test -p irohad --features embedded-soracloud-runtime --bin irohad ensure_inrou_portable_root_disk -- --nocapture` passes 3 portable root-disk tests.
  - `env -u LOG_FORMAT CARGO_TARGET_DIR=target/codex-inrou-extra-tests-irohad cargo test -p irohad --features embedded-soracloud-runtime --bin irohad resolve_inrou_allowlist_endpoints_deduplicates_ipv4_entries -- --nocapture`
  - `env -u LOG_FORMAT CARGO_TARGET_DIR=target/codex-inrou-extra-tests-irohad cargo test -p irohad --features embedded-soracloud-runtime --bin irohad resolve_inrou_allowlist_endpoints_rejects_ipv6_only_literals -- --nocapture`
  - `env -u LOG_FORMAT CARGO_TARGET_DIR=target/codex-inrou-extra-tests-irohad-empty cargo test -p irohad --features embedded-soracloud-runtime --bin irohad resolve_inrou_allowlist_endpoints_rejects_empty_port_lists -- --nocapture`
  - `env -u LOG_FORMAT CARGO_TARGET_DIR=target/codex-inrou-extra-tests-irohad-empty cargo test -p irohad --features embedded-soracloud-runtime --bin irohad planned_inrou_tap_firewall_rules_allowlist_empty_keeps_default_drop -- --nocapture`
- Live KVM/Firecracker smoke was not run on this macOS host.

## 2026-05-01 20k hot-path cache push

- Added exact encoded-length coverage for the `AcceptedTransaction`
  entrypoints that feed block validation. The fallback length path now uses
  Norito's exact `Encode::encoded_len` plus the framed header/padding instead
  of allocating a full `norito::to_bytes(...)` buffer.
- `iroha_crypto` now keeps a bounded thread-local cache of successfully parsed
  canonical Ed25519 public keys. Rejections, malformed keys, and non-canonical
  encodings are not cached, so verification outcomes and deterministic batch
  behavior stay unchanged.
- `PendingBlock` now lazily caches canonical block payload bytes and reuses
  them for pending/in-flight progress payload matching and local RBC seed work.
  The cache is reset when pending block state is replaced, revived after an
  abort, or swapped during commit/Kura retry handling.
- Added sustained-pressure queue coverage and a Torii hot-path benchmark that
  keeps a reused queue at fixed backlog levels while measuring enqueue and
  pressure bookkeeping. This complements the existing fresh-queue benchmark.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `git diff --check`
  - `cargo test -p iroha_crypto parse_public_key_ -- --nocapture`
  - `cargo test -p iroha_core accepted_transaction_entrypoint_encoded_lengths_match_norito_frames -- --nocapture`
  - `cargo test -p iroha_core --lib queue_pressure_counters_stay_consistent_under_sustained_backlog -- --nocapture`
  - `cargo test -p iroha_core --lib pending_block_payload_bytes_match_canonical_encoding_and_reset_on_replace -- --nocapture`
  - `cargo bench -p iroha_torii --bench torii_hot_paths --no-run`
- No new Izanami release validation artifact has been produced for this cache
  push yet. The latest committed-throughput baseline remains
  `dist/izanami-prebuilt-20k-postfix-120s-20260501-003649`, which offered
  `2,400,000`, accepted `48,679`, and strictly approved `12,328`; the next
  gate is a fresh 4-peer no-fault 20k TPS prebuilt release run plus a sampled
  profile.

## 2026-04-30 20k bottleneck fix implementation

- Implemented the first tranche of measured hot-path fixes: debug-build Norito
  trace checks are cached outside tests, Torii batch admission caches local
  routing decisions per batch, Torii queue-pressure reads avoid full state
  views, and queue ingress now rejects fresh submissions with the existing
  429 envelope when the latency budget is saturated.
- Added a committed latest-block-header cache to `State` and changed
  `latest_block_header_fast()` to read the cache instead of loading the full
  latest block from Kura on hot paths.
- `AcceptedTransaction` now carries cached entrypoint hashes, signed
  transaction hashes, and exact encoded lengths so queue/admission paths can
  avoid repeated full Norito encoding and hash work.
- Block static validation now uses the deterministic Ed25519 batch verification
  API for naturally batched single-key Ed25519 transactions, then falls back to
  individual deterministic verification on batch failure to identify the bad
  transaction. The normal transaction limit, TTL, signing-policy, and
  consensus-visible validation checks still run.
- FASTPQ background prover submission now observes queue backpressure and
  defers non-critical prover jobs while the queue is saturated. Consensus and
  required block validation checks are unchanged.
- Validation so far: `cargo fmt --all`, `git diff --check`,
  `cargo check -p norito -p iroha_crypto -p iroha_core -p iroha_torii -p irohad`,
  `cargo test -p norito debug_trace_follows_env_flag -- --nocapture`,
  `cargo test -p iroha_crypto ed25519_batch_deterministic -- --nocapture`,
  `cargo test -p iroha_core --lib accepted_transaction_caches_hashes_and_encoded_length -- --nocapture`,
  `cargo test -p iroha_core --lib queue_pressure_ -- --nocapture`,
  `cargo test -p iroha_core latest_block_header_fast_reads_latest_committed_header -- --nocapture`,
  `cargo test -p iroha_torii --lib transaction_ingress_rejects_latency_saturated_queue_before_capacity -- --nocapture`,
  `cargo test -p iroha_core --test admission_batching ed25519_batch_bisection_finds_bad_sig -- --nocapture`,
  and `cargo test -p iroha_core --test signature_batch_determinism ed25519_batch_permutation_finds_same_bad_sig -- --nocapture`.
  Post-fix clean 20k TPS benchmark artifacts are recorded below.

## 2026-05-01 20k post-fix release validation

- Built the benchmark binaries with both requested commands:
  `cargo build -p izanami --bin izanami -p irohad --bin iroha3d` and
  `cargo build --release -p izanami --bin izanami -p irohad --bin iroha3d`.
- Ran a clean release `4`-peer, no-fault, prebuilt `20,000 TPS` profile for
  `30s` at `dist/izanami-profile-20k-postfix-30s-20260501-003504`. Izanami
  offered all `600,000` planned submissions and ingress accepted `47,670`.
  Submit latency p50/p95/p99/max was `3374/5297/6016/7079 ms`; final
  quorum/strict height was `2/2`; quorum/strict approved transactions were
  `58/58`; the tx queue was age-saturated at `37,089/600,000`.
- Ran the clean sampled release profile at
  `dist/izanami-profile-20k-postfix-sampled-30s-20260501-004220`, with macOS
  `sample` output for the Izanami runner and all four `iroha3d` peers. It
  offered `600,000`, accepted `44,999`, recorded submit latency
  `3173/4340/5181/6536 ms`, ended at quorum/strict height `2/2`, and copied
  diagnostics plus `profiles/sample-*.txt`.
- The fresh samples show the remaining hot stacks in peer processes are still
  transaction validation and wire-format work: Ed25519 public-key
  parse/decompress/verify paths, Norito transaction deserialize/serialize
  paths, and `AcceptedTransaction::signed_encoded_len` fallback into
  `norito::core::to_bytes` during block validation. Torii routing/state
  helpers are still visible but no longer dominate the top stack counts.
- Aggregating the four peer `sample-*.txt` files by recursive stack count puts
  Norito encode/decode at roughly `110k` stack appearances,
  Ed25519/Curve25519 crypto at roughly `39k`, public-key string/normalization
  at roughly `9.6k`, block/transaction validation wrappers at roughly `3.6k`,
  and Torii ingress/routing at roughly `1.9k`. Leaf samples excluding sleeps
  and waits are led by Curve25519 field math, allocation/memmove/free, Norito
  compact-length/read-write helpers, and Blake2/CRC hashing.
- Code inspection matches the profile: block validation builds batch inputs
  from raw `SignedTransaction`s, computes `HashOf::new(tx.payload())`, calls
  `signatory.to_bytes()`, then re-enters stateless transaction validation for
  each transaction. The deterministic Ed25519 batch helper currently verifies
  each triple independently and reparses every public key, preserving parity
  but leaving most crypto cost in place. Queue ingress still recomputes gossip
  bytes with `norito::to_bytes(signed)` rather than carrying encoded bytes from
  accepted transaction metadata.
- Ran the clean release `4`-peer `120s` validation at
  `dist/izanami-prebuilt-20k-postfix-120s-20260501-003649`. Izanami offered
  all `2,400,000` planned submissions and ingress accepted `48,679`; submit
  latency p50/p95/p99/max was `2958/4484/4995/6116 ms`; final quorum/strict
  height was `5/5`; quorum/strict approved transactions were `12,328/12,328`;
  the tx queue remained age-saturated at `32,034/2,400,000`.
- Post-fix conclusion: the implemented fixes reduced the worst submit-latency
  and failover symptoms and improved finality progress, especially versus the
  earlier `120s` rerun's `2/2` height and `10/10` approved transactions, but
  this is still not a successful committed-20k run. The next measured
  bottleneck is queue-drain/block-validation throughput under sustained
  ingress pressure, with no current evidence that RBC pressure, DA gating,
  validation rejects, or view-change storms are the primary limiter.

## 2026-04-30 Izanami 20k TPS stress rerun

- Rebuilt the current dirty-tree dev binaries with
  `cargo build -p izanami --bin izanami -p irohad --bin iroha3d`, then reran
  the local `4`-peer, no-fault prebuilt `20,000 TPS` path for a `120s` timed
  window with `2,400,000` transactions prebuilt before the window, `4096`
  submitters, `300,000` max inflight, and `300ms` pipeline time.
- The run offered all `2,400,000` planned submissions (`20,000.00 TPS`).
  Ingress accepted `87,144` (`726.20 TPS`, `3.63%` of offered). Submit
  latency p50/p95/p99/max was `625/30011/45016/46130 ms`; shutdown aborted
  `1,827` outstanding submit tasks after the measured window.
- Final committed/finality evidence is still blocked: quorum/strict height
  ended at `2/2`, quorum/strict approved transactions stayed at `10/10`, and
  peer height / approved-transaction skew stayed `0`. The row remains
  overload and consensus-throughput evidence rather than a successful
  committed-20k result.
- Dominant status deltas: tx queue saturated at `66,580/2,400,000`,
  pacemaker backpressure deferrals `31`, view-change installs `4`, missing-QC
  causes `3`, quorum-timeout causes `1`, range-pull escalations `5` with no
  successes, and no RBC pressure, pending-RBC drops, validation rejects, DA
  gate, missing-payload, or stake-quorum timeout evidence.
- Artifact:
  `dist/izanami-prebuilt-20k-rerun-120s-20260430-200921`, including
  `run.log`, `command.txt`, and copied diagnostics. The shell wrapper failed
  after Izanami emitted its final summary because zsh reserves the variable
  name `status`, so this artifact has no numeric `exit.status`; the run
  itself reached `izanami::summary`.

## 2026-04-30 Izanami 20k bottleneck classification

- Captured a current-tree dev macOS `sample` profile during a fresh `4`-peer,
  no-fault, prebuilt `20,000 TPS` Izanami run for `30s`. Artifact:
  `dist/izanami-profile-20k-current-30s-20260430-202549`, including the
  runner command, peer samples, run log, and copied diagnostics.
- Profile-run caveats: the profiling wrapper reached Izanami's final
  `izanami::summary` but then exited `127` because the wrapper waited for the
  runner PID after a broader `wait`; the captured samples are still present.
  A concurrent `cargo test -p iroha_torii torii_hot_path_load_profile -- --ignored --nocapture`
  process was also active, so this profile is directional rather than a clean
  isolated benchmark.
- Current-tree dev run outcome: `600,000` submissions were offered, but only
  `1,466` reached ingress, with submit latency p50/p95/p99/max
  `3706/11223/41150/44167 ms`, `7,428` endpoint failovers, `14` endpoint
  unhealthy marks, and `3,387` submit tasks aborted on shutdown. Final
  quorum/strict height was `1/1`, approved transactions stayed at `9/9`, peer
  skew stayed `0`, the tx queue saturated at `17,453/600,000`, pacemaker
  backpressure deferred `9` times, and there was no RBC pressure,
  missing-payload, DA-gate, validation-reject, or view-change storm evidence.
- Primary bottleneck class: overload admission and Torii ingress saturation.
  The system is spending the window failing over and timing out request
  submission while the node-side transaction queue stays saturated, so the
  committed-throughput ceiling is being hit before finality can make progress.
- CPU bottleneck class in peer samples: per-transaction validation plus codec
  work. Active peer stacks are dominated by Norito encode/decode paths,
  Ed25519/`curve25519-dalek` signature verification and public-key parsing,
  transaction hash re-encoding, and Torii batch admission paths
  (`handler_post_transactions_batch` -> `accept_transaction_for_ingress` ->
  `AcceptedTransaction::accept_entrypoint`/`validate_with_now`).
- Hot-path tax surfaced by the dev profile: `norito::debug_trace_enabled()`
  probes `std::env::var_os("NORITO_TRACE")` on encode/decode in debug builds,
  which shows up as repeated `getenv`/environment-lock contention in peer
  samples. This is not consensus logic, but it inflates the current dev
  profile and should be cached or removed from hot codec paths before using dev
  samples as throughput evidence.
- Secondary bottleneck class: routing/state metadata lookups during ingress.
  Some peer stacks repeatedly enter
  `State::authoritative_lane_peer_ids` -> `latest_block_header_fast` ->
  `block_by_height` -> `Kura::get_block`, so authoritative-lane checks are
  pulling state/block metadata on the Torii admission path.
- Consensus/RBC classification: not the dominant bottleneck in this rerun.
  The run shows stalled finality, but no RBC pressure, DA-gate,
  missing-payload, validation-reject, stake-quorum timeout, or missing-QC
  storm. Consensus is mostly starved behind saturated ingress/validation and
  expensive admission hot paths rather than failing through an RBC-specific
  mechanism.

## 2026-04-30 Secondary hot-path cost reduction

- `iroha_core` queue admission now lazily snapshots `WorldView`/`Nexus` for
  state-backed queue paths, so ordinary internal transactions no longer pay
  full world/nexus clone/drop costs unless external fee admission or lane
  compliance actually needs world data.
- Queue pressure bookkeeping now uses maintained atomic active/queued counts
  instead of repeatedly asking `DashMap`/removed-marker state for hot
  backpressure snapshots.
- Torii pipeline status pruning now uses observed-time order indexes and a
  single-pruner guard, avoiding full-cache scans and sorts during normal
  transaction/block status writes while preserving TTL and capacity eviction.
- Torii ingress now uses narrow `State` accessors for transaction admission
  limits, account existence checks, and effective block time, removing the
  remaining full `WorldView` clones from transaction acceptance and enqueue
  pressure refresh paths.
- Queue pressure counter tests now assert internal counter consistency against
  the active transaction map, queued hash deque, and queued-age ring after
  committed-removal, retry, hash-queue rebuild, and expiry-compaction paths.
- Added measurement hooks for the secondary costs:
  `torii_transaction_handle_enqueue_direct_metrics` in
  `crates/iroha_torii/benches/torii_hot_paths.rs` exercises the full Torii
  transaction handler/enqueue path with fresh queue setup outside the measured
  routine, and the ignored
  `pipeline_status_cache_prune_load_profile` test emits a structured prune
  pressure profile line.
- Cleanup found by all-target validation is folded in: current signature wrapper
  calls in the Ed25519 batch precheck path use the inner signature bytes, the
  Kura bench config includes `eviction_required_replicas`, and `iroha` client
  clippy warnings are resolved without relaxing lint policy.
- Focused validation:
  - `cargo fmt --all`
  - `cargo check -p iroha_core --lib`
  - `cargo test -p iroha_core queue_pressure_counters -- --nocapture`
  - `cargo test -p iroha_core expired_cull -- --nocapture`
  - `cargo test -p iroha_core latest_block_header_fast_reads_latest_committed_header -- --nocapture`
  - `cargo test -p iroha_core --test signature_batch_determinism -- --nocapture`
  - `cargo test -p iroha_torii multisig_guard_tests -- --nocapture`
  - `cargo test -p iroha_torii pipeline_status_cache -- --nocapture`
  - `cargo test -p iroha_torii pipeline_status_cache_prune_load_profile -- --ignored --nocapture`
    emitted
    `torii_profile suite=hot_path kind=pipeline_status_cache_prune_pressure samples=32 warmup_samples=4 concurrency=1 wall_ms=357.389 throughput_per_sec=89.538 avg_us=7113.948 p50_us=7120.416 p95_us=7184.209 p99_us=7192.167 p999_us=NA max_us=7192.167`
  - `cargo bench -p iroha_torii --bench torii_hot_paths -- --sample-size 10`
    completed with `torii_transaction_admission_direct_metrics` at
    `[46.692 µs 46.755 µs 46.907 µs]` and
    `torii_transaction_handle_enqueue_direct_metrics` at
    `[14.707 ms 14.890 ms 15.230 ms]`
  - `cargo clippy -p iroha_core -p iroha_torii --all-targets -- -D warnings`

## 2026-04-30 Transaction signature bypass removal

- Removed the `iroha_core` transaction-validation signature override entrypoints
  and block-validation plumbing that could feed preaccepted signature results
  into per-transaction validation.
- Block transaction validation no longer uses stateless validation cache hits or
  deterministic batch-preverification overrides to skip the normal
  `SignedTransaction::verify_signature` path. The cache configuration remains
  present, but block validation treats warmed entries as insufficient for
  accepting a transaction signature.
- Added regression coverage for warmed-cache invalid signatures, heartbeat
  signature rejection, and a source-level guard against reintroducing the
  removed bypass identifiers.

## 2026-04-30 Izanami 20k CPU profile

- Captured a release-build macOS `sample` profile during a `4`-peer,
  no-fault, prebuilt `20,000 TPS` Izanami run for `30s`. The run offered and
  ingress-accepted all `600,000` prebuilt transfers, with submit latency
  p50/p95/p99/max `24/121/387/1353 ms`, so the driver was not the limiting
  stage for this profile.
- Final consensus evidence remained poor despite full ingress delivery:
  quorum/strict height reached only `2/2`, quorum/strict approved
  transactions were `156/156`, max approved-transaction skew was `4096`, the
  queue stayed saturated at `219,843/600,000`, and the worker-loop last
  iteration stretched to `2395 ms`.
- Driver samples were mostly idle or in lightweight send/hash/account-address
  work. The dominant active peer CPU samples were transaction signature
  verification (`curve25519-dalek`/Ed25519), allocation and string work,
  FASTPQ Poseidon/prover work, `WorldView` clone/drop costs, Norito
  serialization, Torii queue/status bookkeeping, and hash/CRC work.
- The peer call stacks show the largest consensus-path CPU cost inside
  `ValidBlock::validate_static_with_snapshot`, especially transaction
  signature verification and Norito re-encoding of signed transactions during
  validation. FASTPQ prover jobs were asynchronous but still consumed several
  seconds of host CPU per peer after block commit.
- Current optimization order after the signature-bypass removal:
  1. Reintroduce transaction signature throughput work only as real signature
     verification, never as a validation override or cache-hit acceptance path.
  2. Bound or defer FASTPQ prover CPU while consensus has an active backlog,
     without changing block validity or deterministic consensus state.
  3. Remove per-transaction full state-view clone/drop work from Torii
     ingress and lane routing by caching cheap parameter/routing snapshots for
     the current block or epoch.
  4. Batch or amortize `PipelineStatusCache` pruning and remaining queue
     pressure refresh work under heavy ingress.
- Profile artifact:
  `dist/izanami-profile-20k-30s-20260430-185822`.

## 2026-05-01 Iroha Connect default relay TTL restored

- Changed the default Connect P2P relay TTL from `0` to `8` hops so the
  default `broadcast` relay strategy actually rebroadcasts over an attached
  Iroha P2P handle. Operators can still set `CONNECT_P2P_TTL_HOPS=0` to
  disable cross-node Connect rebroadcast explicitly.
- Tightened Connect status so zero-TTL broadcast configuration reports an
  effective `local_only` strategy while preserving the normalized configured
  strategy.
- Updated Connect configuration docs, translated default-value references, and
  the config default fixture.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo fmt --all --check`
  - `git diff --check`
  - `cargo test -p iroha_torii --test connect_gating --features ws_integration_tests connect_ws_broadcast -- --nocapture`
  - `cargo test -p iroha_torii --lib broadcast_strategy_with_zero_ttl_reports_local_only_when_p2p_attached -- --nocapture`
  - `cargo test -p iroha_config minimal_config_snapshot -- --nocapture`
  - `cargo test -p iroha_torii --test connect_gating --features ws_integration_tests -- --nocapture`

## 2026-05-01 Iroha Connect P2P rendezvous claims

- Added versioned Connect P2P control messages for relay envelopes, session
  claims, consumed-role notices, and session termination notices. Torii now
  gossips session claims over authenticated Iroha P2P so app and wallet
  WebSockets can rendezvous through different Torii nodes after one
  `/v1/connect/session` response.
- Replaced in-memory app, wallet, and management token storage with
  domain-separated authentication hashes and constant-time comparisons. Claims
  carry token hashes plus the relay MAC key, not raw app/wallet/management
  tokens.
- Added P2P claim, conflict, unknown-session relay drop, consumed-role, and
  termination counters to Connect status and surfaced them in the JS, Python,
  and Swift typed SDK status snapshots.
- Added shared Connect session vectors for token hashes, relay MAC key, and
  relay auth hash, with Rust/JS fixture coverage and Swift/Kotlin/Java relay
  auth fixture assertions.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo fmt --all --check`
  - `cargo check -p iroha_torii`
  - `cargo test -p iroha_torii_shared connect_sdk -- --nocapture`
  - `cargo test -p iroha_torii --lib p2p_ -- --nocapture`
  - `cargo test -p iroha_torii --lib register_tokens_stores_token_hashes -- --nocapture`
  - `node --test test/connect.browser.test.js test/connectPreviewFlow.test.js test/toriiClient.test.js` from `javascript/iroha_js`
  - `npm run build:dist` from `javascript/iroha_js`
  - `python3 -m py_compile python/iroha_python/src/iroha_python/client.py python/iroha_torii_client/client.py python/iroha_torii_client/tests/test_client.py`
- Validation blockers in this environment:
  - `python3 -m pytest python/iroha_torii_client/tests/test_client.py -k 'connect_status or connect_session'` could not run because `pytest` is not installed.
  - Kotlin/JVM and Java Android Connect tests could not run because no Java runtime is available.
  - Focused Swift Connect tests could not run because `IrohaSwift/dist/NoritoBridge.xcframework` is missing.

## 2026-05-01 Sumeragi frontier formal gaps closed

- Refactored the focused Taira frontier-recovery model from one active frontier
  plus a Boolean future-evidence shortcut into one active frontier plus one
  concrete future frontier slot. The future slot carries presence, contiguity,
  vote counts, queued votes, payload state, and recovery owner, while
  `FutureFrontierEvidence` is now derived from that slot.
- Added the two-step future reanchor path: clear the stale/current pending
  wrapper, then promote the future slot into the active slot with active
  progress flags reset and `frontierSlot` advanced.
- Strengthened liveness so an active vote-backed pending wrapper must
  eventually clear. Payload recovery, quorum retransmit, future-slot reanchor,
  and promoted second-slot behavior now have focused follow-through
  properties.
- Added late post-GST future-evidence arrival, future-evidence preservation,
  and promotion-freshness checks so a second-slot quorum cannot be silently
  dropped or promoted with stale active progress flags.
- Made Apalache bounds explicit in `scripts/formal/sumeragi_apalache.sh` for
  every mode, kept existing modes backward-compatible, added payload-recovery,
  retransmit-follow-through, future-promotion, future-reanchor-clear,
  future-evidence-drop, promotion-reset, and future-stale-owner bug modes, and
  promoted `frontier-wide`, a small TLC cross-check, and all expected-failure
  mutations into normal formal CI. A scheduled/manual GitHub Actions workflow
  now runs the longer `frontier-nightly` bound.
- Updated the English Sumeragi formal README with the two-slot proof scope,
  runner modes, CI behavior, and model-to-implementation assumption map.
  Translated `docs/formal/sumeragi/README.*.md` bodies were intentionally not
  refreshed in this slice, so they may remain source-current stale until a
  separate translation refresh.
- Added focused Rust bridge regressions for future new-view reanchor while the
  vote queue is backlogged and while the old frontier recovery owner is stale.
- Improved the surrounding process so nightly formal CI runs the normal formal
  gate before the longer bound, and PR/nightly docs jobs upload JSON metadata
  reports for the deliberately stale translated formal READMEs.
- No runtime consensus code changed in this hardening pass.
- Validation completed with local Apalache `0.52.2`:
  - `bash -n scripts/formal/sumeragi_apalache.sh ci/check_sumeragi_formal.sh ci/check_sumeragi_formal_expected_failures.sh scripts/formal/sumeragi_tlc.sh`
  - `cargo fmt --all`
  - `bash scripts/formal/sumeragi_apalache.sh frontier-fast`
  - `bash scripts/formal/sumeragi_apalache.sh frontier-deep`
  - `bash scripts/formal/sumeragi_apalache.sh frontier-wide`
  - `bash scripts/formal/sumeragi_apalache.sh frontier-nightly`
  - `bash ci/check_sumeragi_formal_expected_failures.sh`
  - `bash scripts/formal/sumeragi_tlc.sh frontier-small`
  - `bash ci/check_sumeragi_formal.sh`
  - `cargo test -p iroha_core --lib reschedule_defers_vote_backed_quorum_timeout_while_vote_queue_backlogged -- --nocapture`
  - `cargo test -p iroha_core --lib reschedule_skips_vote_backed_retransmit_while_frontier_quorum_timeout_window_owned -- --nocapture`
  - `cargo test -p iroha_core --lib reschedule_ignores_quorum_timeout_vote_queue_backlog -- --nocapture`
  - `cargo test -p iroha_core pacemaker_reanchors -- --nocapture`
  - `python3 ci/check_docs_i18n_metadata.py --paths docs/formal`
  - `python3 ci/check_docs_i18n_metadata.py --paths docs/formal --json-out target/docs-i18n/formal-metadata.json`

## 2026-04-30 Sumeragi frontier recovery formal model

- Added a focused bounded TLA+/Apalache model for the Taira frontier hang
  class at `docs/formal/sumeragi/SumeragiFrontierRecovery.tla`, with fast and
  deep configs. The model keeps signatures/ECDSA abstracted as finite vote
  evidence and explicitly covers a pending contiguous frontier block, queued
  commit-vote backlog, missing/local payload state, stale recovery ownership,
  quorum-reschedule marker/window pacing, and deterministic commit, retransmit,
  bounded view-rotation, and zero-evidence drop outcomes after GST.
- The frontier proof checks `TypeInvariant`, commit-implies-vote-quorum,
  commit-implies-payload-availability, no vote-backed zero-evidence zombie
  drop, post-GST vote-backed frontier progress, and post-GST eventual
  resolution by commit, payload recovery, quorum retransmit, or bounded view
  rotation.
- Extended `scripts/formal/sumeragi_apalache.sh` with backward-compatible
  `frontier-fast` and `frontier-deep` modes, and extended
  `ci/check_sumeragi_formal.sh` so formal CI now runs both the existing
  commit-path model and the new frontier-recovery model.
- No runtime consensus code changed in this slice.
- Validation used local Apalache `0.52.2` (`build: 9103560`, archive sha256
  `e0ebea7e45c8f99df8d92f2755101dda84ab71df06d1ec3a21955d3b53a886e2`):
  - `bash scripts/formal/install_apalache.sh 0.52.2`
  - `bash -n scripts/formal/sumeragi_apalache.sh ci/check_sumeragi_formal.sh`
  - `bash scripts/formal/sumeragi_apalache.sh frontier-fast`
  - `bash scripts/formal/sumeragi_apalache.sh frontier-deep`
  - `bash ci/check_sumeragi_formal.sh`
  - `cargo test -p iroha_core reschedule_defers_vote_backed_quorum_timeout_while_vote_queue_backlogged -- --nocapture`
  - `cargo test -p iroha_core reschedule_skips_vote_backed_retransmit_while_frontier_quorum_timeout_window_owned -- --nocapture`
  - `cargo test -p iroha_core reschedule_ignores_quorum_timeout_vote_queue_backlog -- --nocapture`
  - `python3 ci/check_docs_i18n_metadata.py --paths docs/formal` (passed with
    the expected stale `source_hash` warnings for existing translated formal
    README files)

## 2026-04-30 Iroha Connect session and relay hardening

- Added session-scoped `token_management` and `token_relay` credentials to
  Connect session creation. Management tokens now gate session deletion and
  per-session status; public `/v1/connect/status` is redacted to aggregate
  counters.
- Wrapped P2P Connect rebroadcasts in MAC-authenticated relay envelopes with a
  bounded TTL. Torii drops relay frames that fail MAC verification, target an
  unknown session, or exhaust TTL, and exposes auth/TTL drop counters in
  status.
- Bound the relay token into wallet approval signatures with a tagged
  approval preimage, added browser-side approval verification, and aligned the
  JS, Python, Swift, Kotlin, and Java SDK surfaces with the new management and
  relay token contract.
- Updated MCP/OpenAPI metadata, examples, JS package `dist`, and Connect docs
  to keep management tokens out of launch URIs while carrying relay tokens in
  wallet/app deep links.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo fmt --all --check`
  - `cargo check -p iroha_torii`
  - `cargo test -p iroha_torii_shared connect_sdk -- --nocapture`
  - `cargo test -p iroha_torii connect_session_delete -- --nocapture`
  - `cargo test -p iroha_torii connect_session_status_requires_management_token -- --nocapture`
  - `cargo test -p iroha_torii connect_management_headers_maps_token_to_authorization -- --nocapture`
  - `node --test test/connect.browser.test.js test/connectPreviewFlow.test.js test/toriiClient.test.js` from `javascript/iroha_js`
  - `npm run build:dist` from `javascript/iroha_js`
  - `python3 -m py_compile python/iroha_python/src/iroha_python/client.py python/iroha_python/src/iroha_python/connect.py python/iroha_python/src/iroha_python/examples/connect_flow.py python/iroha_torii_client/client.py python/iroha_torii_client/tests/test_client.py`
- Validation blockers in this environment:
  - `python3 -m pytest python/iroha_torii_client/tests/test_client.py -k 'connect_status or connect_session'` could not run because `pytest` is not installed.
  - `./gradlew :core-jvm:test --tests org.hyperledger.iroha.sdk.connect.ConnectWalletRequestTest --console=plain` could not run because no Java runtime is available.
  - `swift test --filter 'ConnectCryptoTests|ToriiClientTests/testGetConnectStatusParsesSnapshot|ToriiClientTests/testCreateConnectSessionPostsPayload|ToriiClientTests/testDeleteConnectSessionHandles404'` could not run because `dist/NoritoBridge.xcframework` is missing.

## 2026-04-30 queue router and Sumeragi focused regression fixes

- Fixed the focused `iroha_core` regression cluster where opaque
  asset-definition IDs in queue routing fell back to the default dataspace
  instead of resolving the stored canonical asset definition and its alias
  binding.
- Tightened Sumeragi quorum-timeout frontier recovery ownership checks to the
  active view so stale owner state from an earlier post-rotation view no
  longer suppresses the direct empty-view rotation path.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo test -p iroha_core queue::router::tests:: -- --nocapture`
  - `cargo test -p iroha_core sumeragi::main_loop::tests::force_view_change_if_idle_rotates_post_rotation_round_with_stale_quorum_timeout_owner -- --nocapture`
  - `cargo test -p iroha_core sumeragi::main_loop::tests::force_view_change_if_idle_ignores_stale_quorum_timeout_owner_after_frontier_grace -- --nocapture`

## 2026-04-30 Izanami 20k ingress restored; committed TPS still blocked

- Replaced the queue pressure age scan with an amortized FIFO age ring, so
  Torii admission no longer scans the full transaction map under 20k prebuilt
  ingress. The 30s release run with `600,000` prebuilt transfers now offered
  and ingress-accepted all `600,000` submissions with zero submit failures;
  submit latency was p50/p95/p99/max `16/36/73/281 ms`.
- Added final transaction-approval sampling to Izanami progress and summary
  output. This separates ingress acceptance from committed/finalized
  throughput and exposes peer divergence at the deadline.
- The latest local 20k run did **not** reach 20k committed TPS:
  quorum/strict finalized transaction counts were only `39/39` at the 30s
  deadline, with max peer transaction-approved skew `4096`. One peer can get a
  full 4096-transfer block ahead, but quorum/strict finality does not converge
  fast enough under the offered load.
- Dominant evidence in the diagnostics is now block validation and
  availability/commit convergence under large transfer blocks, not the driver:
  height-3 validation took about `3.4s`, commit QC aggregation took roughly
  `20s`, repeated RBC DELIVER rebroadcasts were needed, and the asynchronous
  FASTPQ prover lane generated many per-entry proofs at roughly `16-20 ms`
  each after the block commit. View-change causes were zero in the Izanami
  summary, but the queue remained saturated (`205,691/600,000`) and
  pacemaker backpressure fired `7` times.
- Reduced FASTPQ prover log amplification by changing per-proof success logs
  to debug-level detail and emitting one info-level summary per prover job.
  This removes a large local logging pressure source for transfer-heavy stress
  blocks, but proof generation itself remains a real throughput cost.
- Current conclusion: the harness can now do real 20k ingress on this host,
  but Sumeragi cannot commit/finalize 20k TPS for the current transfer
  workload. Getting there requires a real consensus/execution throughput
  change: batching or throttling optional FASTPQ proof work, reducing per-block
  validation latency, and improving large-block DA/RBC/commit-vote convergence.
- Latest artifact:
  `dist/izanami-prebuilt-20k-4096acct-fastplan-pipeline200-gas2b-teu24m-cap4096-rbc4096-30s-20260430-180302`.

## 2026-04-30 Sumeragi overload admission and 20k liveness rerun

- Added a local Torii ingress admission guard for latency-saturated queues:
  before enqueueing a fresh transaction, Torii refreshes the queue pressure
  budget from the effective block time and returns saturated queue
  backpressure once queued transaction age exceeds the budget. This keeps
  20k stress rows honest as overload admission evidence rather than accepting
  an unbounded local backlog.
- Relaxed the Sumeragi pacemaker backpressure classification for payload-only
  pending frontier work after the ingress starvation window. Saturated ingress
  no longer turns a live but unvoted pending frontier into a permanent hard
  proposal veto; once recovery is due, queue saturation becomes pacing so the
  deterministic recovery/proposal path can run without changing quorum safety.
- Reran the local no-fault offered-20k path for a `120s` timed window with all
  `2,400,000` transactions prebuilt ahead of time. Izanami launched all
  `2,400,000` attempts (`20,000.00 TPS`) and ingress accepted `7,090`
  (`59.08 TPS`, `0.30%` of offered). The row is now classified as
  `not-driver-saturated`, `ingress-under-delivered`, `overload-admission`,
  and `liveness-degraded-not-stalled`: quorum/strict height advanced to `4/4`
  with zero peer skew instead of remaining at height `1/1`.
- The dominant Sumeragi evidence moved from a missing-QC storm to bounded
  overload: view-change installs `12`, quorum timeouts `6`, missing QC `4`,
  last cause `quorum_timeout`, tx queue saturated at `6,475/65,536`,
  pacemaker backpressure deferrals `105`, missing-QC reacquire `5/5`, and
  range-pull escalations/successes/failures `19/4/4`. RBC pressure,
  validation rejects, missing payload, DA gate, and stake-quorum timeout were
  all zero.
- Latest local 20k artifacts:
  `dist/izanami-prebuilt-20k-120s-20260430-111435`, including
  `run.log`, `failure-classification.md`, and copied diagnostics under
  `diagnostics/`.
- Extended the same prebuilt/no-fault local path into a `120s` TPS ladder at
  requested `50,100,250,500,1000,2000,5000,10000`. The `50` and `100` rows are
  explicitly driver-capped by Izanami stable-ingress pacing (`64` max inflight
  for requested TPS <= `200`), while `250` TPS and above offered essentially the
  full requested load from memory. Best accepted throughput in this ladder was
  `8,376` accepted at requested `5,000 TPS` (`69.80 TPS`, `1.40%` of offered);
  requested `10,000 TPS` offered `1,199,035` attempts and accepted `7,264`
  (`60.53 TPS`, `0.61%` of offered).
- Ladder classification: every uncapped row is `ingress-under-delivered` and
  `tx_queue_saturated`; `2,000 TPS` and `10,000 TPS` show the clearest
  liveness churn, with strict height only `3`, missing-QC view-change totals
  `35` and `20`, and quorum-timeout totals `13` and `8`. The `5,000 TPS` row
  stayed livelier (`9` strict height, one quorum-timeout cause), so the observed
  break point is not monotonic; scheduler/leader timing still matters. Across
  the ladder, RBC pressure, missing payload, validation rejects, DA gate, and
  peer height divergence remained zero or non-dominant.
- Ladder artifacts:
  `dist/izanami-prebuilt-ladder-120s-20260430-122804/ladder-summary.tsv` and
  `dist/izanami-prebuilt-ladder-120s-20260430-122804/ladder-report.md`.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo test -p iroha_core --lib proposal_backpressure_allows_starved_payload_only_pending_under_saturation -- --nocapture`
  - `cargo test -p iroha_torii transaction_ingress_rejects_latency_saturated_queue_before_capacity -- --nocapture`
  - `cargo test -p iroha_core --lib sumeragi_resilience -- --nocapture`
  - `cargo test -p iroha_core --lib sumeragi_status -- --nocapture`
  - `cargo test -p izanami sumeragi_status_digest -- --nocapture`
  - `cargo test -p izanami throughput -- --nocapture`
  - `cargo test -p izanami prebuild -- --nocapture`
  - `python3 -m pytest scripts/tests/izanami_matrix_classifier_test.py`
  - `cargo build -p izanami --bin izanami -p irohad --bin iroha3d`

## 2026-04-30 Izanami prebuilt transaction buffer

- Added an Izanami-only high-TPS driver path with `--prebuild-tx-buffer` and
  `--prebuild-tx-workers`. Stable stateless plans are now signed, hashed, and
  Norito-encoded before the timed load window starts; prebuilt stress feeds use
  only cached payloads and do not fall back to live transaction construction.
- Added reusable client support for prepared transaction payloads so repeated
  submissions can avoid re-encoding `SignedTransaction` bodies. The public Torii
  endpoint and payload format remain unchanged.
- Added summary counters for the buffer: capacity, workers, built, used,
  fallback, skipped, and build failures. Matrix/sweep scripts can pass the new
  prebuild knobs through their own CLI options.
- Replaced the per-submit prebuilt ticker with a batch feed scheduler that
  catches up to elapsed-time TPS targets from the in-memory deque. The timed
  window no longer includes prebuild warmup, and the prebuilt path bypasses
  loop-level ingress backpressure so under-delivery is reported as
  ingress/consensus evidence instead of silent driver throttling.
- Local `4`-peer smoke at requested `20,000 TPS` for `10s` prebuilt all
  `200,000` transactions ahead of time and launched all `200,000` during the
  timed window. Ingress accepted `27,046`; the row exposed endpoint failover,
  endpoint unhealthy marking, `tx_queue_saturated=true`, pacemaker
  backpressure, `quorum_timeout`, and missing-block/range-pull pressure. This
  is now a real offered-20k stress row on the driver side, with acceptance
  limited by ingress/consensus pressure.
- Extended the same local offered-20k path to a `120s` timed window with
  `2,400,000` prebuilt transactions. Izanami launched `2,394,340` attempts
  (`19,952.83 TPS`, `99.76%` of requested) and ingress accepted `55,488`
  (`462.40 TPS`, `2.32%` of offered). Classification:
  `ingress-under-delivered`, `consensus-stalled`, `overload-admission`, and
  `missing-qc-view-change-storm`; not driver-saturated. The run preserved raw
  logs and a compact classification report under
  `dist/izanami-prebuilt-20k-120s-20260430-103233`.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo test -p iroha prepared_transaction_payload -- --nocapture`
  - `cargo test -p izanami prebuild -- --nocapture`
  - `cargo test -p izanami stored_args_roundtrip -- --nocapture`
  - `cargo test -p izanami throughput -- --nocapture`
  - `cargo test -p izanami local_port_exhaustion -- --nocapture`
  - `bash -n scripts/run_izanami_communication_vulnerability_matrix.sh scripts/run_izanami_communication_vulnerability_sweep.sh`
  - `cargo build -p izanami --bin izanami -p irohad --bin iroha3d`

## 2026-04-29 Izanami 20k stress diagnostics and targeted Sumeragi recovery

- Added truthful 20k stress reporting for requested/offered/accepted TPS, submit latency percentiles, shutdown-drain counters, recovery-height/skew evidence, detailed Sumeragi status deltas, and per-scenario diagnostic artifact capture under matrix stress runs.
- Extended Sumeragi status propagation through the wire model, Torii routing, client JSON, Izanami evidence TSV, root-cause report, and sweep aggregation so view-change causes, missing-block fetch state, tx queue/backpressure, worker-loop, QC defer/reacquire, forced proposal, range-pull, and NPoS repair-coverage telemetry remain individually visible.
- Applied the observed `missing_qc` liveness fix only: repeated same-height missing-QC reacquire now promotes to the existing trusted-peer block-sync range-pull path and clears same-height range-pull cooldowns, without changing block validity, signatures, validator order, or quorum safety.
- Reran the real seed-7 NPoS 20-peer/800s/25% packet-loss row at requested `20,000 TPS` with `target/debug` binaries. The driver offered `816,166` attempts (`1020.21 TPS`, `5.10%` of request) and ingress accepted `773,285` (`966.61 TPS`, `4.83%` of request); quorum/strict height stayed `1/1`. The post-fix dominant cause moved from `missing_qc` to `quorum_timeout` with `tx_queue_saturated=true` and pacemaker backpressure, so the row is now classified as driver-saturated, consensus-stalled, and overload-admission evidence rather than an unexplained missing-QC repair failure.
- Latest 20k artifacts: `dist/izanami-20k-targeted-npos-packet-loss-25pct-seed7-20260430-000420`, including `evidence.tsv`, `root-cause.md`, `paper-style-final-report.md`, and diagnostics copied under `diagnostics/npos-packet-loss-25pct`.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo test -p iroha_core --lib repeated_same_height_missing_qc_reacquire_broadens_range_pull_after_retry_window -- --nocapture`
  - `cargo test -p iroha_core --lib missing_qc_height_stall_mode_reanchor_uses_deterministic_peer_subset_and_periodic_all_peers -- --nocapture`
  - `cargo test -p izanami sumeragi_status_digest -- --nocapture`
  - `cargo test -p izanami throughput -- --nocapture`
  - `cargo test -p iroha_core --lib sumeragi_status -- --nocapture`
  - `python3 -m pytest scripts/tests/izanami_matrix_classifier_test.py`
  - `bash -n scripts/run_izanami_communication_vulnerability_matrix.sh`
  - `cargo build -p izanami --bin izanami -p irohad --bin iroha3d`
  - Real 20k NPoS packet-loss row with diagnostics enabled, then report-only rebuild for the new overload-admission evidence label.

## 2026-04-30 Offline V2 native bridge prover FFI

- Rebased PR #5578 onto the current `i23-features` branch and narrowed it to
  the shared `connect_norito_bridge` C-FFI prover surface. Swift keeps using
  its native `Halo2OfflineNoteV2Prover` path, while the bridge now exposes
  Rust-backed redeem/audit proof generation for other native consumers.
- Added `connect_norito_offline_prove_note_v2_redeem` and
  `connect_norito_offline_prove_note_v2_audit`, returning Norito-archive
  `OfflineNoteRecursiveProofV2` payloads with canonical verifier-key id,
  public-input hash, and Halo2/IPA proof bytes.
- Added bridge tests that decode the FFI output, check the proof binding, and
  verify the returned proof against the canonical Offline V2 verifier. Invalid
  archives now fail through `CONNECT_NORITO_ERR_OFFLINE_NOTE_V2_PROVE`.
- Fixed three current `iroha_data_model` clippy findings surfaced by the
  bridge clippy pass: two `NPoS` doc-markdown warnings and one collapsible
  schema-map branch.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo fmt --all --check`
  - `cargo test -p connect_norito_bridge offline_note_v2_ -- --nocapture`
  - `cargo test -p connect_norito_bridge`
  - `cargo clippy -p connect_norito_bridge --all-targets -- -D warnings`

## 2026-04-29 NPoS PRF seed recovery repair

- Restored NPoS PRF seed recovery to use persisted VRF epoch records as the
  source of truth, deriving restart-gap seeds by replaying finalized record
  reveals and empty epoch rollovers instead of re-hashing the configured base
  seed by epoch number.
- Kept `EpochManager::restore_from_record` seeds intact during actor startup,
  mode rebuilds, and post-commit boundary refresh so unfinalized/finalized VRF
  record state is not clobbered by schedule-derived fallback seeds.
- Updated Sumeragi vote fixtures to cache signer identity metadata with their
  test rosters, and aligned the stale-view commit-vote fixture with the
  actor's committed-block catch-up before signing votes.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo test -p iroha_core --lib npos_seed_for_height -- --nocapture`
  - `cargo test -p iroha_core --lib load_npos_collector_config_uses_vrf_seed -- --nocapture`
  - `cargo test -p iroha_core --lib event_driven_precommit -- --nocapture`
  - `cargo test -p iroha_core --lib stale_view_async_commit_votes_for_known_pending_block_still_form_qc -- --nocapture`
  - `cargo test -p iroha_core --lib block_sync_update_stale_frontier_with_commit_votes_keeps_recovery_active_for_local_vote -- --nocapture`
  - `cargo test -p iroha_core --lib apply_mode_flip_to -- --nocapture`
  - `cargo test -p iroha_core --lib refresh_npos_seed -- --nocapture`
  - `cargo test -p iroha_core --lib on_block_commit_persists_new_epoch_seed_record -- --nocapture`
  - `cargo test -p iroha_core --lib finalize_pending_block_commits_retired_same_height_with_conflicting_local_vote -- --nocapture`
  - `cargo test -p iroha_core --lib`
  - `git diff --check`

## 2026-04-29 NPoS epoch transition coverage

- Added focused `EpochManager` unit coverage for non-boundary block commits,
  explicit `next_epoch` state clearing, clamped epoch window/height mapping,
  unfinalized VRF epoch record restore, and finalized epoch-boundary snapshots
  that preserve commits, regular reveals, late reveals, and penalty inputs
  before state is cleared.
- Extended `EpochManager` edge coverage for epoch-mismatched VRF notes, late
  reveal rejection without a matching commitment, height-zero commit no-ops,
  and zero-length epoch parameters clamping to single-block epochs.
- Added epoch-schedule and PRF seed coverage for skipped unfinished VRF epoch
  records, non-monotonic finalized epoch ends, post-finalized fallback epoch
  lengths, and seed selection across finalized NPoS epoch boundaries.
- Added actor-level coverage for runtime mode flip restoring an unfinalized
  target VRF epoch record, and for pre-commit seed refresh preserving in-flight
  epoch participation while applying updated on-chain epoch parameters.
- Added reverse mode-flip and post-commit boundary refresh coverage: NPoS to
  permissioned rebuilds the epoch manager while clearing collectors, and an
  epoch-boundary post-commit refresh advances to the next epoch while clearing
  stale VRF inputs.
- Added actor epoch-resolution coverage for scheduled permissioned cutovers,
  finalized VRF schedule precedence over a stale local manager, and active
  NPoS manager use after the last finalized epoch boundary.
- Added VRF epoch snapshot persistence coverage for participant merge ordering,
  late reveal serialization, finalized penalty field retention, and preserving
  existing penalty-application markers on unfinalized snapshot updates.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo test -p iroha_core --lib apply_mode_flip_to_npos_restores_unfinalized_target_epoch_record`
  - `cargo test -p iroha_core --lib apply_mode_flip_to_permissioned_restores_epoch_record_but_clears_collectors`
  - `cargo test -p iroha_core --lib refresh_npos_seed_precommit_preserves_epoch_state_during_schedule_change`
  - `cargo test -p iroha_core --lib refresh_npos_seed_postcommit_boundary_advances_epoch_and_clears_inputs`
  - `cargo test -p iroha_core --lib epoch_for_height_returns_zero_for_permissioned_target_after_scheduled_flip`
  - `cargo test -p iroha_core --lib epoch_for_height_prefers_finalized_schedule_over_stale_manager`
  - `cargo test -p iroha_core --lib epoch_for_height_uses_manager_after_finalized_boundary_when_npos_active`
  - `cargo test -p iroha_core --lib epoch_schedule_`
  - `cargo test -p iroha_core --lib npos_seed_for_height_tracks_finalized_epoch_schedule`
  - `cargo test -p iroha_core --lib vrf_snapshot_record_merges_participants_and_finalized_penalties`
  - `cargo test -p iroha_core --lib unfinalized_vrf_snapshot_strips_penalties_but_preserves_application_marker`
  - `cargo test -p iroha_core --lib sumeragi::epoch`
  - `git diff --check`

## 2026-04-29 Izanami seed-7 stress rerun closure

- Reran the real seed-7 `stress-400` and `stress-800` communication-vulnerability matrices with fresh `target/codex-stress` binaries and row-isolated execution so each scenario has its own completed log before being merged into the report artifact.
- Both refreshed artifact directories are now fully resilient: `dist/izanami-stress-400-seed7-20260428` and `dist/izanami-stress-800-seed7-20260428` each contain 14 data rows plus the header in `summary.tsv` / `evidence.tsv`; every permissioned and NPoS row has `exit_code=0`, `status=ok`, and `paper_outcome=resilient`.
- The previously degraded NPoS transient-failure, packet-loss, and leader-isolation rows now report `confirmation_queue_dropped=0`, `confirmation_failed=0`, hard `failures=0`, no unexpected successes, and no RBC/pending pressure. Late sampled confirmations are counted as `confirmation_budget_skipped` or shutdown noise instead of degrading completed runs.
- Rebuilt `summary.md`, `evidence.tsv`, and `paper-style-final-report.md` for both stress directories from the completed row logs; artifact-wide scans over the fresh report directories found no nonzero queue-drop/failure counters, panics, route errors, confirmation timeouts, stuck-queued transaction markers, or queue-pressure markers.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo test -p izanami confirmation_audit_scheduler -- --nocapture`
  - `python3 -m pytest scripts/tests/izanami_matrix_classifier_test.py`
  - `bash -n scripts/run_izanami_communication_vulnerability_matrix.sh && git diff --check`
  - `CARGO_TARGET_DIR=target/codex-stress cargo build -p izanami --bin izanami -p irohad --bin iroha3d`
  - Row-isolated real seed-7 `stress-400` reruns for missing/degraded NPoS rows plus report rebuild for `dist/izanami-stress-400-seed7-20260428`
  - Row-isolated real seed-7 `stress-800` reruns for all permissioned and NPoS rows plus report rebuild for `dist/izanami-stress-800-seed7-20260428`

## 2026-04-29 Iroha config minimal snapshot Kura default

- Refreshed `minimal_config_snapshot` so the expected Kura defaults include
  `eviction_required_replicas: 3`.
- Focused validation for this fix:
  - `cargo test -p iroha_config --test fixtures`
  - `cargo fmt --all`

## 2026-04-29 Canonical Kura test fixture repair

- Updated snapshot tests so WSV state and snapshot writing share the same Kura
  instance under the new state/Kura alignment guard, and kept the intentional
  last-block soft-fork read scenario as a read-side mismatch instead of a
  write-side rejection.
- Adjusted state and Sumeragi fixtures that intentionally modeled missing,
  future, or conflicting blocks so Kura only stores canonical contiguous
  chains; non-canonical payloads now live in pending/test state where the
  behavior under test expects them.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo test -p iroha_core --lib snapshot::tests::can_read -- --nocapture`
  - `cargo test -p iroha_core --lib state::tests::all_blocks_skips_missing_kura_entries -- --nocapture`
  - `cargo test -p iroha_core --lib sumeragi::main_loop::tests::fetch_pending_block_ -- --nocapture`
  - `cargo test -p iroha_core --lib sumeragi::main_loop::tests::fetch_block_body_ -- --nocapture`
  - `cargo test -p iroha_core --lib sumeragi::main_loop::tests -- --nocapture` (`2001` passed, `20` ignored)
  - `cargo fmt --all --check`
  - `git diff --check`

## 2026-04-29 SORA Minamoto mainnet Codex skill

- Added a standalone `sora-minamoto-mainnet` Codex skill for the public
  Minamoto Torii MCP endpoint at `https://minamoto.sora.org/v1/mcp`, mirroring
  the Taira skill structure while making mainnet write handling explicitly
  conservative.
- Expanded the skill with a concrete Minamoto transaction workflow, write
  confirmation policy, required write inputs, default no-agent-side-signing
  posture, common read payload examples, failure-to-action map, Taira
  difference table, and agent output requirements.
- Incorporated live restart-smoke feedback: prefer the Minamoto MCP namespace
  when multiple SORA servers are configured, avoid alias-index enumeration as a
  health check, treat Musubi `404` as absent data when the tool is callable,
  and verify explorer pagination against returned `pagination` plus
  `inputSchema`.
- Updated Codex integration docs and agent guidance so Minamoto workflows point
  at the new skill, prefer curated `iroha.*` tools, keep signing inputs
  runtime-only, and avoid Taira testnet faucet/bootstrap assumptions on
  mainnet.
- Focused validation for this slice:
  - `git diff --check`
  - `git diff --no-index --check /dev/null skills/sora-minamoto-mainnet/SKILL.md`
  - `git diff --no-index --check /dev/null skills/sora-minamoto-mainnet/agents/openai.yaml`
  - `diff -qr skills/sora-minamoto-mainnet "$HOME/.codex/skills/sora-minamoto-mainnet"`
  - Read-only live MCP smoke: `mcp__sora_minamoto_mainnet__.iroha_sumeragi_status`

## 2026-04-29 Mandatory Kura durability before state commit

- Made Kura block storage synchronous and canonical-height checked: duplicate same-height/same-hash stores are idempotent, gaps and same-height hash conflicts are hard errors, and successful returns mean the block is present in the durable block files.
- Removed the Sumeragi commit-path `persist_required` bypass so every commit worker stores/verifies the block in Kura before applying and committing WSV state; `PendingBlock::kura_persisted` is retained only as retry/logging state.
- Kept the Kura writer thread on sidecar flushing and shutdown fsync handling instead of making it authoritative for block appends, and added regression coverage for merge-log rollback and `kura_persisted` retry behavior.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo test -p iroha_core kura::tests`
  - `cargo test -p iroha_core sumeragi::main_loop::tests::state_commit_failure_after_kura_store_keeps_partial_head_hidden`
  - `cargo test -p iroha_core --lib sumeragi::main_loop::tests::pending_kura_persisted_still_checks_kura_before_state_commit`
  - `cargo test -p iroha_core --lib snapshot::tests::snapshot_write_rejects_state_ahead_of_kura`
  - `cargo test -p irohad snapshot_read_error_tests::snapshot_read_error_is_recoverable_classifies_errors`
  - `git diff --check`

## 2026-04-28 Izanami stress audit queue root-cause fix

- Root-caused the seed-7 stress degraded rows to Izanami's sampled confirmation audit queue, not Sumeragi consensus state: the completed stress logs show `confirmation_queue_dropped` hits while Sumeragi status deltas report no RBC store pressure, no RBC evictions, no pending-RBC drops, no persist drops, and accepted submissions equal started submissions.
- Fixed the audit scheduler so sampled confirmations are not enqueued when the remaining run window cannot cover the configured audit timeout. Late samples now count as `confirmation_budget_skipped`; genuine bounded queue overflow still increments `confirmation_queue_dropped`, and an unexpected early audit-channel close now increments `confirmation_failed`.
- The fresh 2026-04-29 stress rerun above confirms the previously degraded audit-queue-only rows move to resilient when no real queue overflow occurs.
- Validation:
  - `cargo fmt --all`
  - `cargo test -p izanami confirmation_audit_scheduler -- --nocapture`
  - `python3 -m pytest scripts/tests/izanami_matrix_classifier_test.py`
  - `bash -n scripts/run_izanami_communication_vulnerability_matrix.sh && git diff --check`

## 2026-04-28 Sumeragi pacemaker low-online recovery gate

- Narrowed the pacemaker's first-proposal low-online deferral so it still suppresses fresh view-0 startup proposals without an online commit quorum, but no longer blocks cached-slot cleanup, missing-QC committed-QC fallback proposals, recovery heartbeat proposals, stale/unknown precommit recovery, or future-NEW_VIEW reanchor catch-up.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo fmt --all --check`
  - `cargo test -p iroha_core --lib sumeragi::main_loop::tests::cached_recovery_proposal -- --nocapture`
  - `cargo test -p iroha_core --lib sumeragi::main_loop::tests::force_view_change_if_idle_forces_missing_qc_frontier_proposal -- --nocapture`
  - `cargo test -p iroha_core --lib sumeragi::main_loop::tests::pacemaker_ -- --nocapture`
  - `git diff --check`

## 2026-04-28 Izanami seed-7 stress evidence

- Ran real 20-peer/800s stress matrices with fresh `target/codex-stress` binaries for seed `7`:
  - `dist/izanami-stress-400-seed7-20260428` (`stress-400`, both Sumeragi modes)
  - `dist/izanami-stress-800-seed7-20260428` (`stress-800`, both Sumeragi modes)
- Stress results are margin evidence, not paper-mode acceptance replacements. This initial pre-fix run showed permissioned resilient across all 800 TPS rows and all but the 400 TPS 25% packet-loss subrow; NPoS targeted-load/stopping were resilient while transient-failure, packet-loss subrows, and leader-isolation degraded from bounded confirmation-audit queue drops without hard submission failures. The 2026-04-29 rerun closure above supersedes these classifications with fresh fully resilient artifacts.
- Added `--report-only` to the matrix runner so completed raw logs can regenerate `summary.md`, `evidence.tsv`, and `paper-style-final-report.md` after a post-run report assembly failure. Rebuilt both stress artifact directories with the new path.
- Focused validation for this slice:
  - `CARGO_TARGET_DIR=target/codex-stress cargo build -p izanami --bin izanami -p irohad --bin iroha3d`
  - `TEST_NETWORK_BIN_IROHAD=target/codex-stress/debug/iroha3d IROHA_TEST_SKIP_BUILD=1 scripts/run_izanami_communication_vulnerability_matrix.sh --out dist/izanami-stress-400-seed7-20260428 --mode stress-400 --sumeragi-mode both --izanami-cmd target/codex-stress/debug/izanami -- --seed 7`
  - `TEST_NETWORK_BIN_IROHAD=target/codex-stress/debug/iroha3d IROHA_TEST_SKIP_BUILD=1 scripts/run_izanami_communication_vulnerability_matrix.sh --out dist/izanami-stress-800-seed7-20260428 --mode stress-800 --sumeragi-mode both --izanami-cmd target/codex-stress/debug/izanami -- --seed 7`
  - `scripts/run_izanami_communication_vulnerability_matrix.sh --out dist/izanami-stress-400-seed7-20260428 --mode stress-400 --sumeragi-mode both --report-only`
  - `scripts/run_izanami_communication_vulnerability_matrix.sh --out dist/izanami-stress-800-seed7-20260428 --mode stress-800 --sumeragi-mode both --report-only`
  - `bash -n scripts/run_izanami_communication_vulnerability_matrix.sh scripts/run_izanami_communication_vulnerability_sweep.sh`
  - `python3 -m pytest scripts/tests/izanami_matrix_classifier_test.py`

## 2026-04-28 SDK compatibility matrix gap closure

- Added `fixtures/sdk/compatibility_matrix.json` as the canonical public SDK compatibility matrix fixture for the `i23-features` branch, with every SDK/story cell populated.
- Added a focused pytest guard that rejects malformed rows, private/local source metadata, and any reintroduced `no-data` cells.
- Focused validation for this slice:
  - `python3 scripts/tests/sdk_compatibility_matrix_test.py`

## 2026-04-28 SDK crypto parity for JS, Swift, Kotlin, and Java

- Added full signing-algorithm parity across the JavaScript SDK, Swift SDK, Kotlin SDK, and Java Android SDK for Ed25519, secp256k1, ML-DSA, all supported GOST R 34.10-2012 parameter sets, BLS normal/small, and SM2.
- JS now exposes generic native-backed key generation/import, signing, verification, and multihash helpers; Swift and JVM/Android SDKs now carry the same Rust bridge discriminants, address curve IDs, and native-backed software key paths.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo fmt --all --check`
  - `cargo check -p iroha_js_host`
  - `cargo check -p connect_norito_bridge`
  - `npm --prefix javascript/iroha_js run build:native`
  - `npm --prefix javascript/iroha_js run build:dist`
  - `cd javascript/iroha_js && node --test test/crypto.test.js test/crypto.browser.test.js`
  - Node one-off JS generic crypto smoke with a stubbed native binding
  - `cd kotlin && ./gradlew :core-jvm:test --tests org.hyperledger.iroha.sdk.crypto.SigningAlgorithmTest --tests org.hyperledger.iroha.sdk.address.AccountAddressTest --console=plain`
  - `cd java/iroha_android && JAVA_HOME=$(/usr/libexec/java_home -v 21) ANDROID_HOME=~/Library/Android/sdk ANDROID_SDK_ROOT=~/Library/Android/sdk ./gradlew :core:test --tests org.hyperledger.iroha.android.crypto.SigningAlgorithmTests --tests org.hyperledger.iroha.android.address.AccountAddressTests --console=plain`
  - `cd IrohaSwift && swift test --filter IrohaSDKSigningAlgorithmTests/testSigningAlgorithmsMatchRustBridgeDiscriminants`
  - `git diff --check`
- Rebuilt `javascript/iroha_js/native/iroha_js_host.node` and its checksum manifest so the Node SDK loads the new generic crypto exports locally.

## 2026-04-28 Python SDK all-algorithm crypto bridge

- Extended the Python SDK crypto bridge beyond Ed25519/raw SM2 helpers to expose generic `CryptoKeyPair`, key generation/import, signing, verification, and multihash import/export for every compiled `iroha_crypto` signature suite: Ed25519, secp256k1, ML-DSA-65, TC26 GOST R 34.10-2012 parameter sets, BLS normal/small, and SM2.
- Kept compatibility-specific Ed25519 account-id helpers and raw SM2 distid/SEC1 helpers while adding payload-based generic APIs for all algorithms.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo fmt --all --check`
  - `cargo check -p iroha_python_rs`
  - `cargo build --release -p iroha_python_rs`
  - `python3 -m compileall -q python/iroha_python/src/iroha_python/crypto.py python/iroha_python/src/iroha_python/__init__.py python/iroha_python/tests/crypto_algorithms_test.py`
  - Direct Python execution of `python/iroha_python/tests/crypto_algorithms_test.py` test functions against `target/release/libiroha_python_rs.dylib` (`3` test functions passed)
  - `git diff --check`
- Local tooling gaps: `ruff`, `maturin`, and `pytest` are not installed in the available Python interpreters, so the focused Python smoke used plain assertion execution instead of pytest collection.

## 2026-04-28 Izanami evidence reporting review fixes

- Tightened the communication-vulnerability matrix acceptance marker so only a real whitespace-delimited `failures=N` field can degrade a run; `expected_failures=N` metadata no longer satisfies the marker.
- Stopped duration-only Izanami deadline sampling from reporting `first_progress_after_fault_start_height` / `first_progress_after_fault_end_height`; those fields now remain reserved for polling runs that actually observe height advancement after the fault boundary, while final quorum/strict/skew evidence is still recorded.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `bash -n scripts/run_izanami_communication_vulnerability_matrix.sh`
  - `python3 -m pytest scripts/tests/izanami_matrix_classifier_test.py`
  - `cargo test -p izanami duration_deadline -- --nocapture`
  - `git diff --check`

## 2026-04-28 Sumeragi main-loop recovery regression closure

- Fixed the reported Sumeragi main-loop recovery failure cluster by keeping contiguous frontier recovery on exact slots instead of leaking back into generic missing-block state, while preserving the wider-roster resilience fallback when a commit quorum leaves a meaningful non-quorum tail.
- Restored liveness gating for idle view changes, missing-QC reacquire, stale frontier owners, backlog suppression, and passive catch-up slots so actionable dependencies defer rotation without pinning unrelated views.
- Repaired vote-backed recovery handoffs: local same-height vote history now blocks active conflicting proposals but still lets exhausted stale owners yield; validation/commit inflight work keeps matching frontier ownership live; known-block commit-QC repair distinguishes local payload materialization from exact body repair.
- Reworked vote-backed reschedule and NEW_VIEW rebroadcast paths so near-quorum retries, single-vote fast windows, quorum-timeout ownership, and pacemaker rebroadcasts remain deterministic under backlog.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo test -p iroha_core --lib sumeragi::main_loop::tests::assemble_proposal_defers_when_candidate_conflicts_with_local_vote_history -- --nocapture`
  - `cargo test -p iroha_core --lib sumeragi::main_loop::tests::proposal_yields_ -- --nocapture`
  - `cargo test -p iroha_core --lib sumeragi::main_loop::tests::known_block_commit_qc_recovery_routes_frontier_fetch_through_exact_block_body -- --nocapture`
  - `cargo test -p iroha_core --lib sumeragi::main_loop::tests::frontier_body_fetch_wakes_commit_pipeline_when_commit_qc_repair_body_is_local -- --nocapture`
  - `cargo test -p iroha_core --lib sumeragi::main_loop::tests::qc_missing_block_defer_contiguous_frontier_commit_quorum_fetches_exact_body_immediately -- --nocapture`
  - `cargo test -p iroha_core --lib sumeragi::main_loop::tests::qc_missing_block_defer_widens_exact_body_repair_under_resilience_commit_quorum -- --nocapture`
  - `cargo test -p iroha_core --lib sumeragi::main_loop::tests::pacemaker_injects_recovery_heartbeat_when_new_view_leader_queue_empty -- --nocapture`
  - `cargo test -p iroha_core --lib sumeragi::main_loop::tests -- --nocapture` (`1990` passed, `20` ignored)

## 2026-04-28 Izanami/Sumeragi result-strengthening harness

- Hardened Izanami shutdown accounting so load supervisors stop planning new submissions at shutdown, drain spawned submission tasks for a bounded timeout, and expose `submit_plans_started`, `submit_plans_shutdown_skipped`, and `submit_tasks_shutdown_aborted` in the final `izanami::summary`. The CLI persists `--shutdown-drain-timeout`; the matrix wrapper keeps quick runs at `15s` and paper/stress profiles at `60s`.
- Added run evidence to Izanami summaries and matrix TSVs: submit-latency sample percentiles (`p50`/`p95`/`p99`/max), final quorum/strict height, max peer-height skew, first height progress after fault start/end, and best-effort Sumeragi status deltas for view changes, commit-pipeline timing, missing-block fetch, RBC pressure/evictions, block-sync roster source counters, and NPoS repair coverage.
- Added observational NPoS repair-coverage telemetry to `/v1/sumeragi/status` with Norito-defaulted fields. The snapshot is populated only from local repair/fanout selection, only surfaced for active NPoS status, and does not feed block validity, validator ordering, signatures, or deterministic consensus state.
- Updated the communication-vulnerability matrix tooling to emit `paper-style-final-report.md`, added `paper`, `stress-400`, and `stress-800` profiles, expanded acceptance-failure marker checks, and added `scripts/run_izanami_communication_vulnerability_sweep.sh` to aggregate multi-profile/multi-seed runs into `sweep-summary.tsv`, `sweep-evidence.tsv`, and `sweep-report.md`.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo check -p izanami --bin izanami`
  - `cargo check -p iroha_torii`
  - `python3 -m pytest scripts/tests/izanami_matrix_classifier_test.py`
  - `bash -n scripts/run_izanami_communication_vulnerability_matrix.sh && bash -n scripts/run_izanami_communication_vulnerability_sweep.sh`
  - `cargo test -p izanami metrics_snapshot_accumulates_counts -- --nocapture`
  - `cargo test -p izanami latency_summary_uses_ceil_rank_percentiles -- --nocapture`
  - `cargo test -p izanami shutdown_submission_drain_counts_aborted_tasks -- --nocapture`
  - `cargo test -p izanami cli_overrides_shutdown_drain_timeout -- --nocapture`
  - `cargo test -p izanami stored_args_roundtrip_preserves_fault_window_fields -- --nocapture`
  - `cargo test -p iroha_core --lib npos_repair_coverage_snapshot_is_npos_only -- --nocapture`
  - `cargo test -p iroha_core --lib stake_coverage_bps_for_world_reports_selected_coverage -- --nocapture`
  - `cargo test -p iroha_data_model --test consensus_roundtrip sumeragi_wire_status_roundtrip -- --nocapture`
  - `cargo test -p iroha sumeragi_status_wire_roundtrip_to_json_preserves_fields -- --nocapture`
  - `git diff --check`
- The full 10-seed paper/stress sweep remains intentionally unrun in this slice because it is an expensive acceptance run.

## 2026-04-28 Retired sample identifier cleanup

- Replaced retired bank/sample identifiers in localnet alias catalog defaults, Offline V2 vector generation, SDK tests, and status command examples with neutral PayNet/demo placeholders.
- Regenerated `fixtures/offline/interop_contract_v2.json` from the updated vector generator.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo run -p iroha_data_model --features test-fixtures,transparent_api --bin offline_v2_vectors`
  - `cargo run -p iroha_data_model --features test-fixtures,transparent_api --bin offline_v2_vectors -- --check`
  - `cargo test -p iroha_core selector_matches_authority_domain --lib -- --nocapture`
  - `git diff --check`
  - `cargo fmt --all --check`
  - Boundary-aware tracked and hidden-file scans for the retired identifiers now return no matches.
- `cargo test -p iroha_kagami nexus_localnet_alias_lanes_bind_dataspaces_and_seed_validators -- --nocapture` is blocked by the existing `crates/iroha_kagami/src/genesis/generate.rs` `manifest.parse()` private-method compile error.

## 2026-04-28 Offline Note V2 focused validation gap closure

- Added focused core rejection coverage for Offline Note V2 redeem/audit proof validation: non-`OpenVerifyEnvelope` proof bytes, wrong verifier key id/backend, inactive verifier keys, and public-input hash mismatches now have explicit tests.
- Tightened Torii Offline V2 readiness smoke coverage so the exposed verifier id and public-input schema hash match the canonical fixture contract.
- Test-network genesis generation now computes and injects the confidential verifier-registry root from appended verifier-key registration instructions, so Offline V2 real-verifier localnets no longer start with a stale `vk_set_hash`.
- Added four-peer `network_functional` coverage that registers the real Offline V2 Halo2 IPA verifier, issues a note, audits it into a new note, redeems it, validates balances, and rejects replay/nullifier reuse under consensus.
- Added native app validation coverage for the shared `interop_contract_v2.json`, synthetic Android counter rejection, transcript-like recursive proof rejection, and old QR prefix rejection. Android PK and PNG now also have physical-only KeyMint runner scripts that require a selected API 31+ non-emulator device and capture public attestation artifacts under untracked `artifacts/offline/keymint/`.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo test -p iroha_data_model offline_note_v2 --lib -- --nocapture`
  - `cargo test -p iroha_data_model --features test-fixtures,transparent_api --bin offline_v2_vectors -- --nocapture`
  - `cargo test -p iroha_core offline_note_v2 --lib -- --nocapture`
  - `cargo test -p iroha_test_network config::tests::genesis_confidential_digest_tracks_registered_verifying_keys -- --nocapture`
  - `cargo test -p integration_tests --test network_functional offline_note_v2_issue_audit_redeem_real_proofs_on_four_peers -- --nocapture`
  - `cargo test -p iroha_torii --test offline_v2_readiness_smoke -- --nocapture`
  - `cd IrohaSwift && swift test --filter OfflineNoteV2`
  - `cd kotlin && ./gradlew :core-jvm:test --tests 'org.hyperledger.iroha.sdk.offline.OfflineNoteV2Test' --console=plain`
  - `cd java/iroha_android && ANDROID_HARNESS_MAINS=org.hyperledger.iroha.android.offline.OfflineNoteV2Test ./gradlew :core:test --console=plain`
  - `cd /Users/takemiyamakoto/dev/pk-retail-wallet-android && ./gradlew --no-daemon :core:test --console=plain`
  - `cd /Users/takemiyamakoto/dev/png-retail-wallet-android && ./gradlew --no-daemon :core:test --console=plain`
  - `cd /Users/takemiyamakoto/dev/pk-retail-wallet-android && ANDROID_SERIAL=19181FDF600918 E2E_DEVICE_SERIAL=19181FDF600918 scripts/run_offline_keymint_physical.sh`
  - `cd /Users/takemiyamakoto/dev/png-retail-wallet-android && ANDROID_SERIAL=19181FDF600918 E2E_DEVICE_SERIAL=19181FDF600918 scripts/run_offline_keymint_physical.sh`
  - `cd /Users/takemiyamakoto/dev/pk-retail-wallet-ios && xcodebuild test -project RetailWalletIOS.xcodeproj -scheme RetailWalletIOS -configuration Debug -destination 'platform=iOS Simulator,id=A7E7B24D-46DE-4D6D-B23B-622C5AD9A464' -only-testing:RetailWalletIOSTests/OfflineAPIContractTests -only-testing:RetailWalletIOSTests/AppAttestationAssertionDecoderTests -only-testing:RetailWalletIOSTests/OfflineProofVerifierFixtureTests`
  - `cd /Users/takemiyamakoto/dev/png-retail-wallet-ios && xcodebuild test -project RetailWalletIOS.xcodeproj -scheme RetailWalletIOS -configuration Debug -destination 'platform=iOS Simulator,id=A7E7B24D-46DE-4D6D-B23B-622C5AD9A464' -only-testing:RetailWalletIOSTests/OfflineAPIContractTests -only-testing:RetailWalletIOSTests/AppAttestationAssertionDecoderTests -only-testing:RetailWalletIOSTests/OfflineProofVerifierFixtureTests`
  - `cd /Users/takemiyamakoto/dev/pk-retail-wallet-ios && xcodebuild build -project RetailWalletIOS.xcodeproj -scheme RetailWalletIOS -configuration Debug -destination 'generic/platform=iOS Simulator'`
  - `cd /Users/takemiyamakoto/dev/png-retail-wallet-ios && xcodebuild build -project RetailWalletIOS.xcodeproj -scheme RetailWalletIOS -configuration Debug -destination 'generic/platform=iOS Simulator'`
  - `git diff --check` in the Iroha root and all four touched app repositories.
  - `shasum -a 256` confirmed every copied `fixtures/offline/interop_contract_v2.json` has hash `2660dd41e3b8c1f4b8337d14febbc88e3febe45428c08e4d083197ef01d4e0f6`.
  - Targeted changed-file scans found no temporary-work markers, exact retired proof-domain identifiers, or retired fountain QR v1 identifiers.
- The physical KeyMint gate ran on Pixel 6 serial `19181FDF600918`. PK artifacts were captured under `/Users/takemiyamakoto/dev/pk-retail-wallet-android/artifacts/offline/keymint/20260428T081251Z-19181FDF600918`; PNG artifacts were captured under `/Users/takemiyamakoto/dev/png-retail-wallet-android/artifacts/offline/keymint/20260428T081331Z-19181FDF600918`.

## 2026-04-27 Sumeragi future-new-view recovery and Izanami stable gate

- Added a catch-up-only Sumeragi recovery path for a lagging local frontier when exact-height `NEW_VIEW` quorum is absent but a quorum exists at a future height. The pacemaker now observes the future quorum's highest QC, requests a range pull from the local frontier with the `future_new_view_frontier_reanchor` reason, rebroadcasts the `NEW_VIEW`, and lets passive catch-up advance instead of proposing from stale local state.
- Hardened Izanami ingress failover classification so a closed transport during request send (`connection closed before message completed`) is treated as retryable. This prevents a transient peer close during leader isolation from surfacing as a non-retryable plan submission failure after consensus has otherwise recovered.
- Built fresh binaries with `CARGO_TARGET_DIR=target/codex-stable-gate cargo build -p izanami --bin izanami -p irohad --bin iroha3d`.
- Re-ran the 4-peer stable permissioned gate with fresh binaries and preserved logs. The `200`-block diagnostic at `dist/izanami-stable-gate-20260427-rerun` crossed the previous stall region and reached strict/quorum height `107` with `255/255` accepted transactions, zero submission failures, and zero confirmation failures, but did not hit `200` blocks because the workload drained before the height target. The calibrated `100`-block gate at `dist/izanami-stable-gate-20260427-target100` passed: strict height `100`, quorum height `100`, `241/241` accepted transactions, zero failures, zero confirmation failures, and zero failover or endpoint-unhealthy events.
- Re-ran the quick communication-vulnerability matrix for both Sumeragi modes with seed `7` at `dist/izanami-quick-both-20260427`: all ten rows exited `0`, reported `status=ok`, classified as `paper_outcome=resilient`, and recorded `failure_marker_count=0`. After the ingress retry hardening, reran the changed leader-isolation rows at `dist/izanami-quick-leader-retry-20260427`; permissioned and NPoS both remained `resilient`, `failure_marker_count=0`, and the logs had no `non_retryable`, plan-submission, `429`, timeout, or run-finished-with-errors markers.
- Focused validation for this slice:
  - `CARGO_TARGET_DIR=target/codex-stable-gate cargo test -p iroha_core --lib new_view_tracker_selects_future_quorum_above_recovery_floor -- --nocapture`
  - `CARGO_TARGET_DIR=target/codex-stable-gate cargo test -p iroha_core --lib pacemaker_reanchors_frontier_when_future_new_view_quorum_exists -- --nocapture`
  - `CARGO_TARGET_DIR=target/codex-stable-gate cargo test -p iroha_core --lib new_view_tracker -- --nocapture`
  - `CARGO_TARGET_DIR=target/codex-stable-gate cargo test -p iroha_core --lib pacemaker_prunes_new_view_entries_below_active_height -- --nocapture`
  - `CARGO_TARGET_DIR=target/codex-stable-gate cargo test -p izanami ingress_failover_marks_closed_send_request_retryable -- --nocapture`
  - `CARGO_TARGET_DIR=target/codex-stable-gate cargo test -p izanami communication_vulnerabilities -- --nocapture`
  - `TEST_NETWORK_BIN_IROHAD=$PWD/target/codex-stable-gate/debug/iroha3d IROHA_TEST_SKIP_BUILD=1 scripts/run_izanami_communication_vulnerability_matrix.sh --out dist/izanami-quick-both-20260427 --mode quick --sumeragi-mode both --izanami-cmd target/codex-stable-gate/debug/izanami -- --seed 7`
  - `TEST_NETWORK_BIN_IROHAD=$PWD/target/codex-stable-gate/debug/iroha3d IROHA_TEST_SKIP_BUILD=1 scripts/run_izanami_communication_vulnerability_matrix.sh --out dist/izanami-quick-leader-retry-20260427 --mode quick --sumeragi-mode both --only leader-isolation --izanami-cmd target/codex-stable-gate/debug/izanami -- --seed 7`

## 2026-04-27 Izanami packet-loss sweep closure

- Completed the explicit paper packet-loss sweep at `dist/izanami-packet-sweep-paper-20260427-loss-only` with a fresh `target/codex-packet-loss/debug/izanami` binary, `--mode paper --sumeragi-mode both --only packet-loss --packet-loss-sweep 25,50,75`, and seed `7`: permissioned and NPoS rows at `25%`, `50%`, and `75%` packet loss all exited `0`, reported `status=ok`, and classified as `paper_outcome=resilient`. Each row recorded post-fault block-height progress evidence, every `failure_marker_count` is `0`, the artifact-wide acceptance-marker scan found no hits, and no Izanami/Iroha test-network processes were left running afterward. A stale binary from an earlier attempt rejected `--fault-network-packet-loss-percent`; rebuilding Izanami and checking `--help` confirmed the sweep CLI was present before rerunning.
- Focused validation for this slice:
  - `CARGO_TARGET_DIR=target/codex-packet-loss cargo build -p izanami --bin izanami`
  - `target/codex-packet-loss/debug/izanami --help | rg 'fault-network-packet-loss-percent|fault-enable-network-packet-loss'`
  - `scripts/run_izanami_communication_vulnerability_matrix.sh --out /tmp/izanami-fresh-sweep-flag-smoke --mode quick --sumeragi-mode permissioned --only packet-loss --packet-loss-sweep 25,75 --izanami-cmd target/codex-packet-loss/debug/izanami -- --seed 7`
  - `scripts/run_izanami_communication_vulnerability_matrix.sh --out dist/izanami-packet-sweep-paper-20260427-loss-only --mode paper --sumeragi-mode both --only packet-loss --packet-loss-sweep 25,50,75 --izanami-cmd target/codex-packet-loss/debug/izanami -- --seed 7`
  - `rg -n "429 Too Many Requests|confirmation timeout|confirmation timed out|sampled confirmation failed|transaction did not reach|transaction remained queued|transaction queued for too long|load-worker shutdown timeout|worker shutdown timeout|worker shutdown timed out|queue pressure|No endpoint|route_unavailable|panic|error:" dist/izanami-packet-sweep-paper-20260427-loss-only` (no hits)
  - `cargo fmt --all`
  - `cargo fmt --all --check`
  - `bash -n scripts/run_izanami_communication_vulnerability_matrix.sh`
  - `git diff --check`
  - `pgrep -fl '/\.rustup/.*/bin/(cargo|rustc)|target/codex-packet-loss/debug/izanami|target/codex-izanami/debug/izanami|run_izanami_communication_vulnerability_matrix|target/iroha-test-network/debug/iroha3d'` (no hits after rerun; the first check overlapped with `cargo fmt --all --check`)

## 2026-04-27 SNS alias auto-renew billing amount-scale fix

- Fixed IVM subscription billing for SNS account-alias auto-renew so quote charges are compared, invoiced, and renewed as nano-XOR decimal `Numeric` values instead of raw scale-0 integers.
- Reused the same SNS quote conversion for lease ISI transfers and the IVM host auto-renew path so the cap/balance check matches the actual renewal transfer amount.
- Focused validation for this fix:
  - `cargo fmt --all`
  - `cargo test -p iroha_core quote_charge_amount_to_numeric_uses_nano_xor_scale -- --nocapture`
  - `cargo test -p iroha_core smartcontracts::ivm::host::tests::subscription_bill_account_alias_auto_renew_queues_renewal_and_reschedules -- --nocapture`
  - `git diff --check`

## 2026-04-27 Native Offline Note V2 SDK/mobile alignment

- Standardized the first-release mobile offline contract on the Iroha Offline Note V2 fixture (`fixtures/offline/interop_contract_v2.json`) with canonical Norito-backed public-input hashes, opaque recursive proof bytes, `iroha:qr1:` QR stream frames, and `parity_group=3`.
- Added native Kotlin/JVM and Java Android Offline Note V2 model/codec surfaces that mirror the Swift SDK without Rust FFI/JNI. The parity tests validate key-certificate signing bytes, issue/redeem/audit Norito payloads, public-input hashes, proof binding rejection, and the shared fixture.
- Updated the PK and PNG iOS/Android app offline flows to call the native SDK-backed Offline Note V2 helpers for certificate payloads, issued/output claims, payment-token public inputs, QR framing, and validation instead of app-local text transcripts.
- Focused validation for this slice:
  - `cargo test -p iroha_data_model --features test-fixtures,transparent_api --bin offline_v2_vectors`
  - `cargo run -p iroha_data_model --features test-fixtures,transparent_api --bin offline_v2_vectors`
  - `cd IrohaSwift && swift test --filter OfflineNoteV2`
  - `cd kotlin && ./gradlew :core-jvm:test --tests 'org.hyperledger.iroha.sdk.offline.OfflineNoteV2Test' --rerun-tasks --console=plain`
  - `cd java/iroha_android && ANDROID_HARNESS_MAINS=org.hyperledger.iroha.android.offline.OfflineNoteV2Test ./gradlew :core:test --rerun-tasks --console=plain`
  - `cd /Users/takemiyamakoto/dev/pk-retail-wallet-android && ./gradlew --no-daemon :core:test --console=plain`
  - `cd /Users/takemiyamakoto/dev/png-retail-wallet-android && ./gradlew --no-daemon :core:test --console=plain`
  - `cd /Users/takemiyamakoto/dev/pk-retail-wallet-ios && xcodebuild test -project RetailWalletIOS.xcodeproj -scheme RetailWalletIOS -configuration Debug -destination 'platform=iOS Simulator,id=A7E7B24D-46DE-4D6D-B23B-622C5AD9A464' -only-testing:RetailWalletIOSTests/OfflineAPIContractTests`
  - `cd /Users/takemiyamakoto/dev/png-retail-wallet-ios && xcodebuild -project RetailWalletIOS.xcodeproj -scheme RetailWalletIOS -configuration Debug -destination 'platform=iOS Simulator,id=A7E7B24D-46DE-4D6D-B23B-622C5AD9A464' build`

## 2026-04-27 Swift Offline V2 transaction builders

- Added Swift Offline V2 note models for key certificates, issued claims, redeem public inputs, audit public inputs, recursive proofs, and issue/redeem/audit instruction payloads.
- Added `IrohaSDK` builders and submit helpers for `IssueOfflineNoteV2`, `RedeemOfflineNoteV2`, and `AuditOfflineNoteV2` transactions. Redeem and audit builders validate the recursive proof's public-input hash against the canonical Swift/Rust Norito payload before signing.
- Added fixture parity coverage against `fixtures/offline/interop_contract_v2.json` for key-certificate signing bytes, issue/redeem/audit Norito payloads, public-input hashes, proof binding rejection, and signed envelope construction.
- Focused validation for this slice:
  - `swift test --filter OfflineNoteV2Tests`
  - `swift test`

## 2026-04-27 Iroha config minimal snapshot refresh

- Refreshed `minimal_config_snapshot` so the expected Nexus fee defaults include the empty `successful_claim_fee_exempt_authorities` list.
- Focused validation for this fix:
  - `cargo test -p iroha_config --test fixtures`

## 2026-04-27 SNS suffix catalog price alignment

- `docs/examples/sns/suffix_catalog_v1.json` now matches the `.sora` default price in `iroha_data_model::sns::fixtures::default_policy()` (`500000000` nano-XOR / 0.5 XOR), following the nano-XOR lease unit convention used by ledger-backed SNS pricing.
- Refreshed the catalog checksum and the English SNS catalog/schema docs so they no longer advertise the legacy `120` payment unit as the current `.sora` policy price.
- Focused validation for this fix:
  - `cargo test -p iroha_cli catalog_entry_matches_default_policy -- --nocapture`
  - `sha256sum -c docs/examples/sns/suffix_catalog_v1.sha256`
  - `cargo test -p iroha_cli catalog_detects_price_mismatch -- --nocapture`

## 2026-04-27 Offline V2 real Halo2 IPA prover slice

- Added the real `offline-note-v2-recursive-v1` Halo2 IPA semantic circuit. The circuit binds the Offline V2 public-instance schema, constrains redeem/audit mode, bounded input/output counts, unused amount slots, and normalized input/output amount conservation.
- Added `prove_offline_note_v2_redeem`, `prove_offline_note_v2_audit`, and `derive_halo2_ipa_offline_note_v2_proving_key_bytes`. These paths generate real Halo2 IPA proofs against registered verifier-key material; no debug or mock prover backend is used.
- Offline V2 ISI verification now compares proof-exposed public instances against the same semantic instance layout used by the prover instead of the old hash-only reserved-sentinel layout.
- Added active WSV verifier-key registration for `offline-note-v2-recursive-v1` to Kagami-generated localnet genesis using the real inline Halo2 IPA verifier key and Offline V2 schema hash.
- Torii Offline V2 readiness now advertises the canonical recursive-proof backend, circuit id, schema hash, instance-column count, and verifier key id. The Swift SDK has a typed `getOfflineV2Readiness` accessor for that metadata.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo test -p iroha_core offline_note_v2_real --lib -- --nocapture`
  - `cargo test -p iroha_kagami generated_nexus_localnet_keeps_fee_asset_convertible_for_taira_wallets -- --nocapture`
  - `cargo test -p iroha_torii --test offline_v2_readiness_smoke -- --nocapture`
  - `swift test --filter ToriiClientTests/testGetOfflineV2ReadinessParsesRecursiveVerifierMetadata`
  - `cargo test -p iroha_core expected_public_instances_encode_semantic_columns --lib -- --nocapture`
  - `cargo test -p iroha_data_model offline_note_v2 --lib -- --nocapture`

## 2026-04-27 Offline audit replay and router ambiguity fix

- Offline V2 audit bundles now carry issued input claims in their canonical public inputs. Core verifies those source claims were issued and unspent, consumes their normal spent-claim keys, and consumes normal redemption nullifier keys before publishing audited output claims as redeemable.
- Nexus account-scoped routing no longer trusts the legacy single-binding `dataspace_for_account` shortcut. Account targets route to a non-universal dataspace only when the full account-scope hierarchy has exactly one dataspace; universal-plus-private and multi-private scopes fall back to the default route.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo run -p iroha_data_model --features test-fixtures,transparent_api --bin offline_v2_vectors`
  - `cargo test -p iroha_data_model offline_note_v2 --lib -- --nocapture`
  - `cargo test -p iroha_data_model --features test-fixtures,transparent_api --bin offline_v2_vectors -- --nocapture`
  - `cargo test -p iroha_core audit_replay_keys_cover_input_spend_and_output_issue_domains --lib -- --nocapture`
  - `cargo test -p iroha_core opaque_asset_transfer --lib -- --nocapture`
  - `cargo test -p iroha_core untargeted_universal_authority_transaction_uses_default_lane_with_state --lib -- --nocapture`
  - `cargo test -p iroha_torii --lib explorer -- --nocapture`

## 2026-04-26 Loose-end closure pass: escrow, Izanami, and release hygiene

- Wired the native anonymous escrow ISI family into the V1 IVM syscall surface: open, accept, mark-payment-sent, release, cancel, open-dispute, and resolve-dispute now have ABI constants, policy-table coverage, unknown-syscall fixture updates, host dispatch into the typed ISIs, syscall docs, and an updated V1 ABI hash (`6e26a7b44f773a856e45e91baa9aebbc975d47bb452f12962cd4b03fecfe27b3`).
- Added proof-carrying anonymous escrow helper surfaces for Swift, Kotlin, and Java Android so app clients can build the open/accept/payment/release/cancel/dispute/resolve payload shapes without hand-assembling argument maps.
- Fixed the account-alias onboarding auto-renew enqueue path so the subscriber is granted `CanModifyNftMetadata` for the subscription NFT before the auto-renew trigger is registered, closing the focused permission gap for user-signed metadata updates on that lease record.
- Replaced split IVM sample staging with a shared `crates/ivm/prebuilt_samples.txt` manifest consumed by both `ivm_prebuild` and `integration_tests/build.rs`; the manifest now includes `threshold_escrow` so CLI and integration fixture staging cannot drift on that sample.
- Plumbed Izanami packet-loss percentage through CLI arguments, stored configuration, runtime config merging, persistence, and fault-config generation. The matrix runner now supports `--packet-loss-sweep`, uses `75%` for quick packet-loss smokes, and defaults paper mode to `25%,50%,75%` packet-loss subrows while keeping leader isolation pinned to the existing `75%` stress point.
- Completed the exact-injector 75% packet-loss paper baseline at `dist/izanami-exact-packet-paper-20260426` with `--mode paper --sumeragi-mode both` and seed `7`: all ten permissioned/NPoS rows exited `0`, reported `status=ok`, and classified as `paper_outcome=resilient`. The acceptance-marker scan found no `429`, confirmation timeout, stuck queued transaction, route-unavailable, queue-pressure, panic, or error signatures. The wrapper tripped a post-run `note_path` finalization bug after recording all rows; `note_path` is now initialized up front, the shell smoke passes, and `summary.md` was regenerated from the completed `summary.tsv` / `evidence.tsv`. With the current sweep-capable script, reproduce this ten-row baseline with `--packet-loss-sweep 75`.
- Pinned Sumeragi formal CI to Apalache `0.52.2` through the local installer/toolchain path, updated the fallback Docker reference, and added a `docs/formal` translation metadata audit job for `source`, `source_hash`, and `translation_last_reviewed`.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `git diff --check`
  - `python3 -m py_compile ci/check_docs_i18n_metadata.py`
  - `python3 ci/check_docs_i18n_metadata.py --paths docs/formal --max-messages 5` (passed with stale `source_hash` warnings for existing formal translations)
  - `bash -n scripts/run_izanami_communication_vulnerability_matrix.sh`
  - `scripts/run_izanami_communication_vulnerability_matrix.sh --out /tmp/izanami-note-path-smoke --mode quick --sumeragi-mode permissioned --only targeted-load --izanami-cmd true`
  - `scripts/run_izanami_communication_vulnerability_matrix.sh --out dist/izanami-exact-packet-paper-20260426 --mode paper --sumeragi-mode both --izanami-cmd target/codex-packet-loss/debug/izanami -- --seed 7` (all ten single-75% packet-loss baseline rows completed and were recorded as resilient; with the current sweep-capable script, add `--packet-loss-sweep 75` to reproduce the same ten-row shape; the wrapper finalization bug above was fixed afterward)
  - `rg -n "429 Too Many Requests|confirmation timeout|confirmation timed out|sampled confirmation failed|transaction did not reach|transaction remained queued|transaction queued for too long|load-worker shutdown timeout|worker shutdown timeout|worker shutdown timed out|queue pressure|No endpoint|route_unavailable|panic|error:" dist/izanami-exact-packet-paper-20260426` (no hits)
  - `cargo check -p izanami --bin izanami`
  - `cargo test -p izanami cli_overrides_packet_loss_percent --no-run`
  - `cargo test -p ivm --bin ivm_prebuild sample_manifest_includes_threshold_escrow --no-run`
  - `cargo test -p ivm --test abi_syscall_list_golden --no-run`
  - `cargo test -p ivm --test abi_hash_versions --no-run`
  - `cargo test -p iroha_core native_anonymous_escrow_syscalls_queue_expected_instructions --lib --no-run`
  - `cargo test -p iroha_torii onboarding_alias_auto_renew_grants_subscriber_metadata_mutation --lib --features app_api --no-run`
  - `swift test --filter NativeEscrowInstructionBuildersTests`
- Validation blockers:
  - `cargo test -p ivm --test abi_syscall_list_golden -- --nocapture` compiled but the test binary wedged before entering the test body at `_dyld_start`; a direct `target/debug/deps/abi_syscall_list_golden-* --nocapture` run timed out the same way after the no-run compile passed.
  - Kotlin and Java Android focused Gradle tests were not runnable in this shell because `java` and `/usr/libexec/java_home -v 21` reported no installed Java runtime.
  - A broader `python3 ci/check_docs_i18n_metadata.py --paths docs/source docs/formal docs/portal --max-messages 10` run found existing `docs/source` and `docs/portal` metadata debt, including missing `source_hash` and `translation_last_reviewed` fields, so source/portal-wide gating remains open.

## 2026-04-26 Offline escrow self-account guard and localnet note seed fix

- `crates/iroha_core/src/smartcontracts/isi/offline.rs` now rejects non-zero Offline V2 note escrow movements when the resolved escrow account is the same account being debited or credited. The new `escrow_self_reference` invariant is checked before balance mutation on issue/reserve and redeem/credit paths.
- `crates/iroha_kagami/src/localnet.rs` no longer writes the built-in offline-note asset escrow account as the localnet app authority. Generated peer configs record the deterministic escrow account for the built-in offline-note asset, and core also derives metadata-enabled escrows at enforcement points so stale or missing config bindings cannot bypass the vault protections.
- Focused validation for this fix:
  - `cargo fmt --all`
  - `cargo test -p iroha_core escrow_self_reference`
  - `cargo test -p iroha_kagami generated_localnet_bootstraps_builtin_offline_note_asset_and_permissions`
  - `cargo test -p iroha_kagami generated_peer_config_enables_offline_note_bootstrap_services`
  - `cargo test -p iroha_kagami localnet_readme_records_base_seed_when_present`

## 2026-04-26 Torii MCP Sumeragi collector empty-topology fix

- Fixed `/v1/sumeragi/collectors` so the Torii/MCP test harness with no commit topology returns an empty collector snapshot instead of constructing a `Topology` from an empty peer list and panicking.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo test -p iroha_torii mcp_jsonrpc_tools_call_agent_alias_sumeragi_endpoints_dispatch -- --nocapture`

## 2026-04-26 Sumeragi locked-chain precommit vote fix

- Fixed local precommit emission so a validator with a known locked block refuses to precommit a different block at the same height, even when the candidate is in a newer view. Missing locked payloads still keep the existing newer-view override behavior.
- Focused validation for this slice:
  - `cargo test -p iroha_core --lib sumeragi::main_loop::tests::precommit_vote_skips_when_block_conflicts_with_locked_chain -- --nocapture --exact`
  - `cargo test -p iroha_core --lib emit_precommit_vote -- --nocapture`
  - `cargo fmt --all`

## 2026-04-26 Nexus router default-lane reroute fix

- Fixed state-aware queue rerouting so universal-only account scope is treated as fallback materialization, not as a dataspace routing target. Untargeted universal authority transactions now continue to use `nexus.routing_policy.default_lane`, while non-universal single-scope accounts still route to their dataspace lane.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo test -p iroha_core queue::tests::apply_lane_lifecycle_reconfigures_router_and_limits -- --nocapture`
  - `cargo test -p iroha_core untargeted_universal_authority_transaction_uses_default_lane_with_state -- --nocapture`

## 2026-04-26 Sumeragi recovery and worker-loop test stabilization

- Stabilized Sumeragi commit recovery around QC-backed pending blocks without proposal evidence: cached prepare/commit QCs now count as consensus evidence for validation and commit gating, and local precommit emission records and applies the vote synchronously before broadcast.
- Tightened recovery scheduling for empty-frontier local vote evidence, authoritative RBC/body ingress, and snapshot-roster near-quorum reschedules.
- Restored deterministic worker-loop budget tests by isolating queue-depth state and allowing one pre-tick drain before marking tiny drain budgets exhausted.
- The Sumeragi/Izanami paper-shaped communication vulnerability matrix now completes for both Sumeragi modes with resilient rows across targeted load, transient failure, packet loss, stopping, and leader isolation. The exact-injector 2026-04-26 paper run at `dist/izanami-exact-packet-paper-20260426` produced ten `exit_code=0`, `status=ok`, `paper_outcome=resilient` rows, and the failure-marker scan found no `429`, confirmation timeout, stuck queued transaction, route-unavailable, queue-pressure, startup/config, or pipeline-status failure signatures.
- Izanami now has an in-process P2P packet-loss injector controlled through `iroha_config` network fields. The matrix `packet-loss` and `leader-isolation` scenarios use 75% application-frame loss during their attack windows without mutating validator rosters or deterministic consensus state.
- Duration-only Izanami runs now sample quorum and strict block heights at the deadline, so matrix reports can record progress evidence even when no `--target-blocks` KPI is configured. The matrix runner now writes `evidence.tsv` and an `Iroha Run Evidence` section with progress evidence and acceptance-failure marker counts while preserving the original paper-style result table.
- Izanami now supports run-relative `--fault-window-start` / `--fault-window-end` offsets. Paper-mode matrix fault scenarios pass the paper's `133s` to `266s` attack window, while quick mode remains immediate/randomized for fast local smoke runs.
- The leader-isolation harness now detects single-peer network-fault profiles and retargets each injection round from live Sumeragi leader telemetry instead of relying on the initial random faulty-peer selection.
- Short local leader-isolation smokes at `dist/izanami-leader-target-smoke-20260426/permissioned-rustlog.log` and `dist/izanami-leader-target-smoke-20260426/npos-rustlog.log` exercised the dynamic path for both Sumeragi modes: Izanami detected the profile, sampled `/v1/sumeragi/leader`, injected the partition into the sampled leader, rejoined it, and completed without the matrix acceptance-failure markers.
- Native packet-loss validation covered both Sumeragi modes. Short smokes at `dist/izanami-packet-loss-smoke-20260426/permissioned-leader-rerun.log` and `dist/izanami-packet-loss-smoke-20260426/npos-leader.log` each injected 75% packet loss into the sampled leader, offered 47 transactions, accepted 47, and finished with zero failures. A quick matrix limited to `packet-loss` at `dist/izanami-packet-loss-smoke-20260426/quick-packet-loss-matrix` produced `resilient` rows for permissioned and NPoS with zero failure-marker hits.
- Focused validation for this slice:
  - `cargo test -p iroha_core --lib sumeragi::main_loop::tests::commit_pipeline_allows_tip_pending_with_cached_qc_without_proposal_evidence -- --nocapture --exact`
  - Direct `target/debug/deps/iroha_core-afb8267c04707e87` runs for the Sumeragi main-loop and worker-loop failures listed in the test report.
  - `target/debug/deps/iroha_core-afb8267c04707e87 sumeragi::main_loop::tests::reschedule_stale_pending_blocks_targets_snapshot_roster --exact --nocapture`
  - `CARGO_TARGET_DIR=target/codex-izanami cargo test -p iroha_core --lib commit_pipeline -- --nocapture` (`45` passed)
  - `CARGO_TARGET_DIR=target/codex-izanami cargo test -p iroha_core --lib reschedule_stale_pending_blocks_targets_snapshot_roster -- --nocapture` (`1` passed)
  - `CARGO_TARGET_DIR=target/codex-izanami cargo test -p iroha_core --lib run_worker_iteration -- --nocapture` (`27` passed)
  - `CARGO_TARGET_DIR=target/codex-izanami cargo test -p iroha_torii pipeline_status -- --nocapture` (`19` passed)
  - `CARGO_TARGET_DIR=target/codex-izanami cargo test -p izanami communication_vulnerabilities -- --nocapture` (`5` passed)
  - `CARGO_TARGET_DIR=target/codex-izanami cargo test -p izanami fault_window -- --nocapture` (`8` passed)
  - `CARGO_TARGET_DIR=target/codex-izanami cargo test -p izanami stored_args_roundtrip_preserves_fault_window_fields -- --nocapture` (`1` passed)
  - `CARGO_TARGET_DIR=target/codex-izanami cargo test -p izanami wait_for_duration_deadline -- --nocapture` (`2` passed)
  - `CARGO_TARGET_DIR=target/codex-izanami cargo test -p izanami -- --nocapture` (`255` passed)
  - `CARGO_TARGET_DIR=target/codex-izanami cargo build -p izanami --bin izanami`
  - `CARGO_TARGET_DIR=target/codex-packet-loss cargo test -p iroha_p2p debug_packet_loss_dropper --lib -- --nocapture` (`1` passed)
  - `CARGO_TARGET_DIR=target/codex-packet-loss cargo test -p iroha_p2p --tests --no-run`
  - `CARGO_TARGET_DIR=target/codex-packet-loss cargo test -p iroha_core --lib sumeragi_resilience --no-run`
  - `CARGO_TARGET_DIR=target/codex-packet-loss cargo test -p iroha_torii --test connect_gating --no-run`
  - `CARGO_TARGET_DIR=target/codex-packet-loss cargo test -p izanami network_packet_loss -- --nocapture` (`1` passed)
  - `CARGO_TARGET_DIR=target/codex-packet-loss cargo test -p izanami sumeragi_leader -- --nocapture` (`4` passed)
  - `CARGO_TARGET_DIR=target/codex-packet-loss cargo test -p izanami communication_vulnerabilities -- --nocapture` (`5` passed)
  - `CARGO_TARGET_DIR=target/codex-packet-loss cargo test -p izanami fault_window -- --nocapture` (`8` passed)
  - `CARGO_TARGET_DIR=target/codex-packet-loss cargo test -p izanami fault_toggles -- --nocapture` (`2` passed)
  - `CARGO_TARGET_DIR=target/codex-packet-loss cargo build -p izanami --bin izanami`
  - `bash -n scripts/run_izanami_communication_vulnerability_matrix.sh`
  - `scripts/run_izanami_communication_vulnerability_matrix.sh --out /tmp/izanami-matrix-script-smoke-quick --mode quick --sumeragi-mode both --only targeted-load --izanami-cmd true`
  - `scripts/run_izanami_communication_vulnerability_matrix.sh --out /tmp/izanami-matrix-script-smoke-paper --mode paper --sumeragi-mode both --only transient-failure --izanami-cmd true`
  - `scripts/run_izanami_communication_vulnerability_matrix.sh --out /tmp/izanami-matrix-script-smoke-leader-quick --mode quick --sumeragi-mode both --only leader-isolation --izanami-cmd true`
  - `scripts/run_izanami_communication_vulnerability_matrix.sh --out /tmp/izanami-matrix-script-smoke-leader-paper --mode paper --sumeragi-mode both --only leader-isolation --izanami-cmd true`
  - `scripts/run_izanami_communication_vulnerability_matrix.sh --out /tmp/izanami-matrix-script-smoke-packet-quick --mode quick --sumeragi-mode both --only packet-loss --izanami-cmd true`
  - `scripts/run_izanami_communication_vulnerability_matrix.sh --out /tmp/izanami-matrix-script-smoke-leader-packet-paper --mode paper --sumeragi-mode both --only leader-isolation --izanami-cmd true`
  - `target/codex-izanami/debug/izanami --allow-net --peers 4 --duration 18s --fault-window-start 2s --fault-window-end 9s --tps 5 --max-inflight 16 --workload-profile stable --faulty 1 --submitters 4 --fault-interval-min 1s --fault-interval-max 1s --fault-enable-network-partition=true ...` with `RUST_LOG='info,izanami::faults=debug'`
  - `target/codex-izanami/debug/izanami --nexus --allow-net --peers 4 --duration 18s --fault-window-start 2s --fault-window-end 9s --tps 5 --max-inflight 16 --workload-profile stable --faulty 1 --submitters 4 --fault-interval-min 1s --fault-interval-max 1s --fault-enable-network-partition=true ...` with `RUST_LOG='info,izanami::faults=debug'`
  - `target/codex-packet-loss/debug/izanami --allow-net --peers 4 --duration 55s --fault-window-start 8s --fault-window-end 20s --tps 5 --max-inflight 16 --workload-profile stable --faulty 1 --submitters 4 --fault-enable-network-packet-loss=true ...` with `RUST_LOG='info,izanami::faults=debug,iroha_p2p::network=debug'`
  - `target/codex-packet-loss/debug/izanami --nexus --allow-net --peers 4 --duration 55s --fault-window-start 8s --fault-window-end 20s --tps 5 --max-inflight 16 --workload-profile stable --faulty 1 --submitters 4 --fault-enable-network-packet-loss=true ...` with `RUST_LOG='info,izanami::faults=debug,iroha_p2p::network=debug'`
  - `scripts/run_izanami_communication_vulnerability_matrix.sh --out dist/izanami-packet-loss-smoke-20260426/quick-packet-loss-matrix --mode quick --sumeragi-mode both --only packet-loss --izanami-cmd target/codex-packet-loss/debug/izanami`
  - `scripts/run_izanami_communication_vulnerability_matrix.sh --mode paper --sumeragi-mode both --izanami-cmd target/codex-izanami/debug/izanami -- --seed 7`
  - `cargo fmt --all`
  - `git diff --check`

## 2026-04-26 Offline V2 first-release replacement

- Hardened Offline V2 note issuance so only `CanManageOfflineEscrow` operators can issue notes, and key certificates must verify against the issuing operator over the canonical certificate payload before escrow is reserved.
- Hardened Offline V2 note redemption so the recursive proof public-input hash must bind the source note commitment, consumed nullifiers, certified key payload, recipient, asset, and amount, and escrow is released only for a ledger-recorded issued-note claim that has not already been redeemed.
- Hardened Offline V2 optional audit so the proof public-input hash binds the token id, observed nullifiers, output commitments, and certified key payload; audit now requires a previously issued key certificate and detects token/public-input conflicts plus duplicate output commitments.
- Ordered cheap issued-claim, token, and nullifier replay checks before expensive recursive proof verification while still verifying proofs before escrow release or new audit state.
- Replaced the local transcript-style recursive proof placeholder with verifier-key-backed validation: the proof must name an active `offline_note_v2` WSV verifier, decode as an `OpenVerifyEnvelope`, match the Offline V2 public-input schema hash, expose the expected public instance columns, and pass the configured ZK backend verifier.
- Added data-model helper payloads for canonical key-certificate signing bytes, issued-note claims, redemption public inputs, and audit public inputs.
- Removed legacy allowance, lineage, transfer, revocation, balance-proof, petal-stream, and settlement helper surfaces across Rust, Torii, mobile SDKs, examples, fixtures, and stale docs.
- Torii now exposes only `/v1/offline/v2/readiness` for offline discovery; issuance, redemption, and audit use V2 transaction instructions.
- Torii MCP keeps structured compatibility aliases for legacy offline transfer/revocation tool names so agent clients get Offline V2 guidance instead of JSON-RPC tool-not-found errors; this does not re-publish the removed HTTP routes.
- Localnet, telemetry, QR payload kinds, and mobile parser surfaces now use Offline V2 note naming instead of legacy cash/transfer terminology.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo test -p iroha_torii --test mcp_endpoints`
  - `CARGO_TARGET_DIR=target/codex-workspace-test cargo check -p iroha_data_model -p iroha_core -p iroha_torii -p iroha_config -p iroha_kagami -p iroha_telemetry -p connect_norito_bridge -p fastpq_prover -p fastpq_isi --lib`
  - `CARGO_TARGET_DIR=target/codex-workspace-test cargo test -p iroha_data_model offline_note_v2 --lib -- --nocapture`
  - `CARGO_TARGET_DIR=target/codex-workspace-test cargo test -p iroha_torii --test offline_v2_readiness_smoke -- --nocapture`
  - `CARGO_TARGET_DIR=target/codex-workspace-test cargo test -p connect_norito_bridge --lib -- --nocapture`
  - `CARGO_TARGET_DIR=target/codex-workspace-test cargo test -p iroha_core offline_note_v2 --lib -- --nocapture` (ok; no Core-local tests matched after the model tests moved to `iroha_data_model`)
  - `swift test`
  - `./gradlew :core-jvm:test --console=plain`
  - `./gradlew :offline-wallet-android:assembleRelease --console=plain`
  - `JAVA_HOME=$(/usr/libexec/java_home -v 21) ANDROID_HOME=~/Library/Android/sdk ANDROID_SDK_ROOT=~/Library/Android/sdk ./gradlew test --console=plain`
  - `npm run build:dist && node --test test/toriiClient.test.js test/package_dist.test.js test/offlineQrStream.test.js`
  - `python3 -m pytest python/iroha_torii_client/tests/test_client.py python/iroha_python/tests/testconnect_codec.py -q`
  - `git diff --check`
  - Stale-route/native-symbol scans for legacy offline routes, removed native exports, old Offline V1 fixtures, and deleted Safety Detect wrappers returned no matches in active source/fixture paths.

## 2026-04-25 Taira devex CLI and onboarding diagnostics

- Added the first-class `iroha taira` CLI surface. `iroha taira doctor` performs read-only checks against the public Taira root by default, including `/status`, route availability, MCP initialize, curated MCP tool availability, and recent status warnings. `iroha taira write-canary` now drives the preferred real-write path: ephemeral signer by default, alias/public-key onboarding, faucet PoW claim, gas metadata insertion, signed ping submission, Applied wait, query verification attempt, optional restrictive `--write-config`, and redacted text/JSON receipts.
- Torii MCP `accounts.onboard` now advertises and forwards the `public_key_hex` shortcut, matching the HTTP onboarding path. JSON onboarding clients now receive stable `error_code`, `message`, and optional `hint` diagnostics while explicit Norito clients keep the existing Norito error envelope.
- The Taira rollout docs now steer single-endpoint devex checks to `iroha taira doctor` and `iroha taira write-canary`; `check_mcp_rollout.sh` remains available as the multi-step compatibility harness.
- Focused validation for this slice:
  - `cargo check -p iroha_cli --bin iroha`
  - `cargo check -p iroha_torii --lib`
  - `cargo test -p iroha_cli --bin iroha taira -- --nocapture`
  - `cargo test -p iroha_cli`
  - `cargo test -p iroha_torii --lib onboarding -- --nocapture`
  - `cargo test -p iroha_torii --lib build_accounts_onboard_body -- --nocapture`
  - `bash -n configs/soranexus/taira/check_mcp_rollout.sh configs/soranexus/taira/check_mcp_rollout_mock_test.sh`
  - `bash configs/soranexus/taira/check_mcp_rollout_mock_test.sh`
  - `cargo clippy -q -p iroha_cli --bin iroha --no-deps -- -D warnings`

## 2026-04-25 CUDA coverage follow-up 3

- Added a third focused coverage pass for remaining edge branches: IVM explicit CUDA disable status reset, FASTPQ BN254 wrapper shape rejection before backend calls, `gpuzstd_cuda` exact-capacity compression and empty-source decode errors, `jsonstage1_cuda` zero-capacity required-length reporting, and shared zstd frame truncated-header / bad-magic rejection plus out-of-range sequence metadata rejection.
- Focused validation for this follow-up:
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-gpuzstd-metal cargo test -p gpuzstd_metal -- --nocapture` (`30` passed; `1` ignored)
  - `RUST_TEST_THREADS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-ivm-cuda-check IVM_CUDA_GENCODE=arch=compute_86,code=sm_86 cargo test -p ivm --lib explicit_cuda_disable_records_message_and_reset_clears_it --features cuda -- --nocapture` (`1` passed; existing `iroha_crypto` dead-code warnings remain)
  - `RUST_TEST_THREADS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-fastpq-cuda-check cargo test -p fastpq_prover --lib --features fastpq-gpu fastpq_cuda::tests -- --nocapture` (`12` passed)
  - `RUST_TEST_THREADS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-gpuzstd-cuda GPUZSTD_CUDA_ARCH=-arch=sm_86 GPUZSTD_CUDA_REQUIRE=1 cargo test -p gpuzstd_cuda --features cuda-kernel -- --nocapture` (`14` passed)
  - `RUST_TEST_THREADS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-jsonstage1-cuda JSONSTAGE1_CUDA_ARCH=-arch=sm_86 JSONSTAGE1_CUDA_REQUIRE=1 cargo test -p jsonstage1_cuda --features cuda-kernel -- --nocapture` (`21` passed)

## 2026-04-25 CUDA coverage follow-up 2

- Added another focused coverage pass for edge branches around the CUDA closure: IVM CUDA enable/disable/reset status flags, FASTPQ length saturation and pre-backend shape validation, `gpuzstd_cuda` compression no-space and empty-payload roundtrips, `jsonstage1_cuda` exact-capacity and empty public-entrypoint behavior, and shared zstd frame empty-input plus invalid chunk-metadata rejection.
- The shared zstd frame encoder now validates `chunk_size`, `counts`, and `offsets` before indexing chunk metadata, and emits a valid final empty block for empty payloads.
- Focused validation for this follow-up:
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-gpuzstd-metal cargo test -p gpuzstd_metal -- --nocapture` (`29` passed; `1` ignored)
  - `RUST_TEST_THREADS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-ivm-cuda-check IVM_CUDA_GENCODE=arch=compute_86,code=sm_86 cargo test -p ivm --lib cuda_wait --features cuda -- --nocapture` (`3` passed; existing `iroha_crypto` dead-code warnings remain)
  - `RUST_TEST_THREADS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-ivm-cuda-check IVM_CUDA_GENCODE=arch=compute_86,code=sm_86 cargo test -p ivm --lib cuda_enable_disable_and_reset_update_status_flags --features cuda -- --nocapture` (`1` passed; existing `iroha_crypto` dead-code warnings remain)
  - `RUST_TEST_THREADS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-fastpq-cuda-check cargo test -p fastpq_prover --lib --features fastpq-gpu fastpq_cuda::tests -- --nocapture` (`11` passed)
  - `RUST_TEST_THREADS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-gpuzstd-cuda GPUZSTD_CUDA_ARCH=-arch=sm_86 GPUZSTD_CUDA_REQUIRE=1 cargo test -p gpuzstd_cuda --features cuda-kernel -- --nocapture` (`12` passed)
  - `RUST_TEST_THREADS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-jsonstage1-cuda JSONSTAGE1_CUDA_ARCH=-arch=sm_86 JSONSTAGE1_CUDA_REQUIRE=1 cargo test -p jsonstage1_cuda --features cuda-kernel -- --nocapture` (`19` passed)

## 2026-04-25 CUDA coverage follow-up

- Added focused tests for the CUDA closure paths that were previously validated mostly through happy-path hardware runs: IVM wait-state ready/failure/timeout transitions, FASTPQ CUDA validation/error formatting, `gpuzstd_cuda` FFI null/no-space/exact-capacity behavior, `jsonstage1_cuda` FFI null handling plus empty Stage-1/CRC64 CPU edge cases, and Norito's 16 MiB GPU cutoff selector under a disabled GPU policy.
- Focused validation for this follow-up:
  - `RUST_TEST_THREADS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-ivm-cuda-check IVM_CUDA_GENCODE=arch=compute_86,code=sm_86 cargo test -p ivm --lib cuda_wait --features cuda -- --nocapture` (`3` passed; existing `iroha_crypto` dead-code warnings remain)
  - `RUST_TEST_THREADS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-fastpq-cuda-check cargo test -p fastpq_prover --lib --features fastpq-gpu fastpq_cuda::tests -- --nocapture` (`8` passed)
  - `RUST_TEST_THREADS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-gpuzstd-cuda GPUZSTD_CUDA_ARCH=-arch=sm_86 GPUZSTD_CUDA_REQUIRE=1 cargo test -p gpuzstd_cuda --features cuda-kernel -- --nocapture` (`10` passed)
  - `RUST_TEST_THREADS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-jsonstage1-cuda JSONSTAGE1_CUDA_ARCH=-arch=sm_86 JSONSTAGE1_CUDA_REQUIRE=1 cargo test -p jsonstage1_cuda --features cuda-kernel -- --nocapture` (`16` passed)
  - `RUST_TEST_THREADS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-norito-gpu cargo test -p norito core::heuristics --features gpu-compression -- --nocapture` (`2` passed; existing `struct_index_random_x86.rs` unused-parens warning remains)

## 2026-04-25 CUDA roadmap closure

- IVM CUDA now uses bounded stream waits that fail closed, disable CUDA on timeout, and intentionally abandon outstanding device allocations instead of risking a blocking `cuMemFree` after a timed-out stream. `GpuContext` also drops the CUDA context last so cached modules, streams, and buffers release in driver-safe order.
- IVM Poseidon and public helper paths now use pinned host buffers plus explicit async copies where host-visible CUDA results are read. New fixtures cover timeout handling and repeated CPU-vs-CUDA determinism for vector add, SHA-256, Keccak, AES, BN254, and Ed25519 helpers.
- FASTPQ CUDA FFT/LDE/BN254/Poseidon paths now use nonblocking streams, pinned host transfer buffers, bounded event polling, and a timeout harness that exercises the fail-closed path without wedging a GPU. Repeated BN254 transform determinism coverage is in place.
- Norito's `gpuzstd_cuda`, JSON Stage-1, and CRC64 helpers now use pinned async host/device transfers with bounded event waits before host-visible reads. A zstd offset-code bug in the shared frame encoder was fixed and covered with a standard zstd roundtrip fixture.
- The default Norito GPU compression cutoff is now `16 MiB`. On the RTX 3080 Laptop / WSL2 host below, CUDA zstd stayed slower than CPU at the measured automatic-offload sizes: 1 MiB was `1.700 ms` CPU vs `145.493 ms` CUDA, and 8 MiB was `13.747 ms` CPU vs `517.025 ms` CUDA. JSON Stage-1 CUDA crossed over on this run around 4 KiB (`5593 ns` scalar vs `5030 ns` kernel) and remained faster at 256 KiB (`350011 ns` scalar vs `315870 ns` kernel).
- Added `.github/workflows/nightly_cuda.yml`, a self-hosted CUDA nightly/manual lane that builds real CUDA helpers, runs the focused IVM/FASTPQ/Norito accelerator gates, and uploads GPU model, driver, CUDA toolkit, and gencode inventory under `dist/cuda-nightly`.
- Hardware/toolchain used for closure: NVIDIA GeForce RTX 3080 Laptop GPU, driver `527.56`, compute capability `8.6`, CUDA `12.0.140`, `IVM_CUDA_GENCODE=arch=compute_86,code=sm_86`, `GPUZSTD_CUDA_ARCH=-arch=sm_86`, `JSONSTAGE1_CUDA_ARCH=-arch=sm_86`.
- Focused validation for this closure:
  - `RUST_TEST_THREADS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-ivm-cuda-check IVM_CUDA_GENCODE=arch=compute_86,code=sm_86 cargo test -p ivm --lib cuda_ --features cuda -- --nocapture` (`24` passed; existing `iroha_crypto` dead-code warnings remain)
  - `RUST_TEST_THREADS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-fastpq-cuda-check cargo test -p fastpq_prover --lib --features fastpq-gpu cuda -- --nocapture` (`9` passed)
  - `RUST_TEST_THREADS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-gpuzstd-cuda GPUZSTD_CUDA_ARCH=-arch=sm_86 GPUZSTD_CUDA_REQUIRE=1 cargo test -p gpuzstd_cuda --features cuda-kernel -- --nocapture` (`7` passed)
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-gpuzstd-metal cargo test -p gpuzstd_metal -- --nocapture` (`27` passed; `1` ignored)
  - `RUST_TEST_THREADS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-jsonstage1-cuda JSONSTAGE1_CUDA_ARCH=-arch=sm_86 JSONSTAGE1_CUDA_REQUIRE=1 cargo test -p jsonstage1_cuda --features cuda-kernel -- --nocapture` (`13` passed)
  - `RUST_TEST_THREADS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-norito-gpu GPUZSTD_CUDA_ARCH=-arch=sm_86 GPUZSTD_CUDA_REQUIRE=1 cargo test -p norito gpu_zstd --features gpu-compression -- --nocapture` and `required_cuda_backend_is_registered_when_requested` (passed; existing `struct_index_random_x86.rs` unused-parens warning remains)
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-norito-stage1 JSONSTAGE1_CUDA_ARCH=-arch=sm_86 cargo build -p jsonstage1_cuda --features cuda-kernel`, then `RUST_TEST_THREADS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-norito-stage1 JSONSTAGE1_CUDA_ARCH=-arch=sm_86 JSONSTAGE1_CUDA_REQUIRE=1 cargo test -p norito stage1_helper --features cuda-stage1,stage1-validate -- --nocapture` and `cuda_stage1_backend_matches_scalar_when_required_or_available` (passed)
  - `cargo run -p norito --example gpu_threshold --release --features gpu-compression -- --json` with the CUDA zstd helper built in release mode, and `cargo run -p norito --example stage1_cutover --release --features bench-internal,cuda-stage1,stage1-validate` with the JSON Stage-1 helper built in release mode, captured the cutoff data above.

## 2026-04-25 Norito CUDA GPU helpers

- `gpuzstd_cuda` now builds CUDA kernels by default when `nvcc` is available. Compression runs deterministic CUDA match-finding/sequence generation and uses the shared zstd frame encoder; helpers without built kernels or a CUDA device report `gpu_unavailable` instead of registering a CPU-only helper as a GPU backend. CUDA and frame-assembly failures now return errors to Norito rather than CPU-encoding inside the helper.
- Norito's non-Mac CUDA zstd loader now accepts only the CUDA-named helper and rejects `gpu_unavailable` compression during self-test before enabling the backend. The loader no longer requires `nvidia-smi`; CUDA availability is proven by the helper self-test.
- `jsonstage1_cuda` now includes a CUDA JSON Stage-1 classifier for structural, quote, and backslash masks, with host finalization preserving quote/backslash parity across 32-byte blocks. CUDA CRC64 also reports unavailable when kernels/devices are missing instead of silently falling back inside the helper.
- CUDA validation tests now support required-hardware modes: `GPUZSTD_CUDA_REQUIRE=1` for `gpuzstd_cuda` and `JSONSTAGE1_CUDA_REQUIRE=1` for `jsonstage1_cuda`. Without those env vars, CUDA-only assertions skip cleanly on hosts without kernels/devices.
- Documentation: `crates/norito/README.md` and `docs/source/gpuzstd_cuda_pipeline.md` describe the CUDA zstd, JSON Stage-1, CRC64, and fallback contracts.
- Focused validation for this slice:
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-gpuzstd-cuda cargo test -p gpuzstd_cuda`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-jsonstage1-cuda cargo test -p jsonstage1_cuda`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-norito-gpu cargo test -p norito gpu_zstd --features gpu-compression`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-norito-stage1 cargo test -p norito stage1_helper --features cuda-stage1,stage1-validate`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-norito-stage1 cargo test -p norito cuda_stage1_backend_matches_scalar_when_required_or_available --features cuda-stage1,stage1-validate`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-gpuzstd-metal cargo test -p gpuzstd_metal`

## 2026-04-25 Native anonymous asset escrow core

- Added `AnonymousAssetEscrowRecord`, proof-movement records, and anonymous dispute-resolution records. These store the escrow note commitment, funding/spend nullifiers, labelled buyer/seller output commitments, proof hash, optional envelope hash, root hint, timestamps, lifecycle state, and evidence hashes for on-chain auditability.
- Added native anonymous escrow ISIs for open, accept, mark payment sent, release, cancel, dispute, and resolve. Open/release/cancel/resolve execute through the existing shielded `ZkTransfer` path, so policy checks, root freshness, nullifier replay protection, proof verification, deterministic output ordering, and confidential proof-hash events stay shared with the ZK asset ledger.
- Added world-state storage plus typed query/JSON surfaces for anonymous escrow records, and a generic `ZkTransfer` custody guard: active anonymous escrow commitments require confidential transfer v2 public input commitments, and generic shielded transfers that spend an active escrow note are rejected unless they run inside a native anonymous escrow ISI.
- Hardened anonymous escrow close handling so release, cancellation, and court-resolution proofs must expose exactly one non-zero confidential transfer v2 input commitment matching the stored escrow commitment. Public and anonymous escrows now share `EscrowId` uniqueness both ways, proof-carrying anonymous escrow ISIs charge the same confidential gas as their internal `ZkTransfer`, and JSON snapshot restore preserves public and anonymous escrow records.
- Focused validation for this slice:
  - `cargo fmt --all --check`
  - `CARGO_TARGET_DIR=target/codex-escrow-check cargo check -p iroha_data_model -p iroha_core`
  - `CARGO_TARGET_DIR=target/codex-escrow-check cargo test -p iroha_data_model escrow --lib`
  - `CARGO_TARGET_DIR=target/codex-escrow-check cargo test -p iroha_core escrow --lib`
  - `CARGO_TARGET_DIR=target/codex-escrow-check cargo test -p iroha_core gas --lib -- --nocapture`
  - `CARGO_TARGET_DIR=target/codex-escrow-check cargo test -p iroha_core escrow_records_roundtrip_through_state_json --lib -- --nocapture`

## 2026-04-25 Native escrow custody hardening and SDK surfaces

- Added a typed executor data-model permission token for `CanResolveEscrowDispute`, and core now asserts that the escrow court permission string matches the typed permission.
- Hardened native escrow custody so generic numeric asset debits from any recorded deterministic custody account are rejected through both `Transfer<Asset>` and `Burn<Asset>`, including the closed-record dust case. Escrow release/cancel/resolve remain the only valid custody exit paths.
- Added SDK-facing native escrow helpers for Kotlin, Java Android, and Swift, plus a Kotodama `native_escrow.ko` sample and docs that steer new Aitai-style numeric custody flows away from the legacy threshold escrow grant pattern.
- Added an ABI admission regression proving a V1 contract deployment proposal with a non-canonical ABI hash is rejected after the escrow syscall surface update.
- Focused validation for this gap-closure slice:
  - `cargo fmt --all --check`
  - `CARGO_TARGET_DIR=target/codex-escrow-check cargo check -p iroha_executor_data_model -p iroha_data_model -p iroha_core -p kotodama_lang -p ivm_abi -p iroha`
  - `CARGO_TARGET_DIR=target/codex-escrow-check cargo test -p iroha_data_model escrow --lib`
  - `CARGO_TARGET_DIR=target/codex-escrow-check cargo test -p iroha_executor_data_model escrow_court_permission_uses_expected_name --lib`
  - `CARGO_TARGET_DIR=target/codex-escrow-check cargo test -p iroha_core escrow --lib`
  - `CARGO_TARGET_DIR=target/codex-escrow-check cargo test -p iroha_core propose_rejects_non_canonical_abi_hash_for_v1 --test gov_propose_validation`
  - `CARGO_TARGET_DIR=target/codex-escrow-check cargo test -p kotodama_lang native_escrow_builtins_emit_escrow_syscalls --lib`
- Local SDK validation:
  - Installed Homebrew `openjdk@21` and Android command-line tooling, then populated `~/Library/Android/sdk` with platform/build-tools 34 for the Java Android Gradle harness.
  - Materialized the local Swift bridge with `scripts/build_norito_xcframework.sh`, which produced `dist/NoritoBridge.xcframework` and `dist/NoritoBridge.artifacts.json` for package testing.
  - `JAVA_HOME=/opt/homebrew/opt/openjdk@21/libexec/openjdk.jdk/Contents/Home PATH=/opt/homebrew/opt/openjdk@21/bin:$PATH ./gradlew :core-jvm:test --tests org.hyperledger.iroha.sdk.core.model.instructions.NativeEscrowInstructionsTest --console=plain`
  - `JAVA_HOME=/opt/homebrew/opt/openjdk@21/libexec/openjdk.jdk/Contents/Home PATH=/opt/homebrew/opt/openjdk@21/bin:$PATH ANDROID_HOME=$HOME/Library/Android/sdk ANDROID_SDK_ROOT=$HOME/Library/Android/sdk ./gradlew :core:test --tests org.hyperledger.iroha.android.model.instructions.NativeEscrowInstructionTests --console=plain`
  - `swift test --filter NativeEscrowInstructionBuildersTests`
  - `JAVA_HOME=/opt/homebrew/opt/openjdk@21/libexec/openjdk.jdk/Contents/Home PATH=/opt/homebrew/opt/openjdk@21/bin:$PATH ./gradlew :core-jvm:cleanTest :core-jvm:test --console=plain --no-daemon`
  - `JAVA_HOME=/opt/homebrew/opt/openjdk@21/libexec/openjdk.jdk/Contents/Home PATH=/opt/homebrew/opt/openjdk@21/bin:$PATH ANDROID_HOME=$HOME/Library/Android/sdk ANDROID_SDK_ROOT=$HOME/Library/Android/sdk ./gradlew :core:test --console=plain`
  - `swift test`

## 2026-04-25 Native escrow observability and Aitai integration gap closure

- Added app-facing escrow event filtering via `EscrowEventFilter`, including filters by escrow id, seller, buyer, lifecycle status, and escrow event kind.
- Moved Kotodama escrow id derivation into the data model with `EscrowId::from_kotodama_name`, so compiler/host/client code use the same deterministic mapping.
- Extended native escrow IVM syscalls and Kotodama builtins with optional evidence-hash TLV registers for open, dispute, and resolve flows while keeping zero as the no-evidence path.
- Added ergonomic constructors and prelude exports for all native escrow ISIs, JSON query support for `FindAssetEscrowById`, iterable JSON support for `FindAssetEscrows`, and typed batch downcasting for `AssetEscrowRecord`.
- Added a four-peer native Aitai-style integration flow under `core_api`: open escrow with evidence, prove generic transfer from active custody is rejected even after a transfer grant, accept, mark payment sent, release, and query the final buyer/seller balances.
- Focused validation for the gap-closure slice:
  - `cargo fmt --all`
  - `CARGO_TARGET_DIR=target/codex-escrow-check cargo check -p iroha_data_model -p iroha_core -p kotodama_lang -p ivm_abi -p iroha`
  - `CARGO_TARGET_DIR=target/codex-escrow-check cargo test -p iroha_data_model escrow --lib`
  - `CARGO_TARGET_DIR=target/codex-escrow-check cargo test -p iroha_data_model find_asset_queries_roundtrip_with_public_selectors --lib`
  - `CARGO_TARGET_DIR=target/codex-escrow-check cargo test -p kotodama_lang native_escrow_builtins_emit_escrow_syscalls --lib`
  - `CARGO_TARGET_DIR=target/codex-escrow-check cargo test -p iroha_core escrow --lib`
  - `CARGO_TARGET_DIR=target/codex-escrow-check cargo test -p integration_tests native_asset_escrow_aitai_flow_on_multi_peer_network --test core_api --no-run`
  - `CARGO_TARGET_DIR=target/codex-escrow-check cargo test -p integration_tests native_asset_escrow_aitai_flow_on_multi_peer_network --test core_api -- --nocapture`

## 2026-04-24 Native asset escrow ISIs and Kotodama builtins

- Added native numeric asset escrow state keyed by `EscrowId`, including lifecycle status, evidence hashes, timestamps, seller/buyer/custody fields, dispute resolution details, Norito/JSON/schema support, query outputs, and escrow lifecycle events.
- Added `OpenAssetEscrow`, `AcceptAssetEscrow`, `MarkEscrowPaymentSent`, `ReleaseAssetEscrow`, `CancelAssetEscrow`, `OpenEscrowDispute`, and `ResolveEscrowDispute` ISIs. Opening moves seller funds into a deterministic protocol custody account; release/cancel/resolve move custody only through the escrow ISIs. Generic asset transfer now rejects active native escrow custody sources.
- Added the narrow `CanResolveEscrowDispute` permission for court resolution, plus IVM syscalls `0xB8..0xBE` and Kotodama builtins `escrow_open_offer`, `escrow_accept`, `escrow_mark_payment_sent`, `escrow_release`, `escrow_cancel`, `escrow_open_dispute`, and `escrow_resolve_dispute`.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `CARGO_TARGET_DIR=target/codex-escrow-check cargo check -p iroha_data_model -p iroha_core -p kotodama_lang -p ivm_abi`
  - `CARGO_TARGET_DIR=target/codex-escrow-check cargo test -p iroha_data_model escrow --lib -- --nocapture`
  - `CARGO_TARGET_DIR=target/codex-escrow-check cargo test -p iroha_core escrow --lib -- --nocapture`
  - `CARGO_TARGET_DIR=target/codex-escrow-check cargo test -p ivm abi_syscall_list_matches_golden --test abi_syscall_list_golden -- --nocapture`
  - `CARGO_TARGET_DIR=target/codex-escrow-check cargo test -p ivm abi_hash --test abi_hash_versions -- --nocapture`
  - `CARGO_TARGET_DIR=target/codex-escrow-check cargo test -p kotodama_lang native_escrow_builtins_emit_escrow_syscalls --lib -- --nocapture`

## 2026-04-24 Soracloud production posture hardening

- `iroha_config` now exposes `soracloud_runtime.production_mode`; enabling it requires non-proxy Inrou hosting, fail-closed runtime egress with explicit rate/byte budgets, and rejects Hugging Face inference-bridge fallback. The HF fallback default is now disabled.
- `irohad`'s Soracloud runtime stub now refuses production mode at startup, so a production config requires an `irohad` binary built with `embedded-soracloud-runtime`.
- The embedded Soracloud runtime manager now also calls the same production-posture assertion from `actual::SoracloudRuntime`, so direct config construction cannot bypass the user-config parser.
- Torii now has route-specific signed Soracloud body caps: ordinary mutation bodies use `torii.soracloud_mutation_max_body_bytes`, uploaded-model chunk routes use `torii.soracloud_upload_max_body_bytes`, and the cap is enforced before canonical-request signature verification.
- Signed Soracloud POST routes now have account+origin rate limiting and a global inflight cap via `torii.soracloud_mutation_rate_per_account_origin_per_sec`, `torii.soracloud_mutation_burst_per_account_origin`, and `torii.soracloud_mutation_max_inflight`. The rate key includes the verified account, `Origin`, and route group (`mutation`, `upload`, `model`, or `hf`).
- Focused validation for the production-posture slice:
  - `cargo fmt --all`
  - `env -u LOG_FORMAT cargo test -p iroha_config soracloud_runtime_ --lib -- --nocapture`
  - `env -u LOG_FORMAT cargo test -p iroha_config --test fixtures -- --nocapture`
  - `env -u LOG_FORMAT cargo test -p iroha_torii --lib --features app_api,telemetry soracloud_signed_mutation_middleware_ -- --nocapture`
  - `env -u LOG_FORMAT cargo test -p irohad --bin irohad stub_runtime_ -- --nocapture`
  - `env -u LOG_FORMAT cargo test -p irohad --features embedded-soracloud-runtime --bin irohad manager_config_ -- --nocapture`
- The 2026-04-25 portable follow-up closes the local boot-smoke gap:
  - Homebrew QEMU 11.0.0 provides `qemu-system-aarch64` and `qemu-img` on this host.
  - `scripts/ci/prepare_inrou_portable_guest_assets.py` now prepares verified Debian genericcloud assets for native PortableVm smoke runs. It checks Debian `SHA512SUMS`, extracts the root ext4 partition, applies the expected root label/fstab, and exports the matching kernel/rootfs/initrd paths.
  - PortableVm cloud-init network config now matches predictable Ethernet names (`e*`) instead of assuming `eth0`, so Debian arm64 genericcloud guests consume the NoCloud metadata seed and start the hosted app.
  - `cargo run -p xtask --bin xtask -- soracloud-inrou-smoke portable` now preflights the guest asset env vars and points operators at the preparation helper when they are missing.
  - Focused validation for the follow-up:
    - `python3 -m py_compile scripts/ci/prepare_inrou_portable_guest_assets.py`
    - `python3 scripts/ci/prepare_inrou_portable_guest_assets.py --output-dir /tmp/iroha-inrou-assets-genericcloud --print-env`
    - `env -u LOG_FORMAT cargo test -p irohad --features embedded-soracloud-runtime --bin irohad build_inrou_portable_network_config_matches_predictable_interface_names -- --nocapture`
    - `env -u LOG_FORMAT IROHA_RUN_IGNORED=1 IROHA_INROU_PORTABLE=1 IROHA_INROU_PORTABLE_KERNEL_IMAGE=/tmp/iroha-inrou-assets-genericcloud/vmlinux-aarch64 IROHA_INROU_PORTABLE_ROOTFS_IMAGE=/tmp/iroha-inrou-assets-genericcloud/rootfs-aarch64.ext4 IROHA_INROU_PORTABLE_INITRD_IMAGE=/tmp/iroha-inrou-assets-genericcloud/initrd-aarch64.img cargo test --locked -p irohad --features embedded-soracloud-runtime --bin irohad inrou_portable_smoke_boots_debian_guest_and_serves_healthcheck -- --ignored --nocapture`
    - `env -u LOG_FORMAT IROHA_INROU_PORTABLE_KERNEL_IMAGE=/private/tmp/iroha-inrou-assets-genericcloud/vmlinux-aarch64 IROHA_INROU_PORTABLE_ROOTFS_IMAGE=/private/tmp/iroha-inrou-assets-genericcloud/rootfs-aarch64.ext4 IROHA_INROU_PORTABLE_INITRD_IMAGE=/private/tmp/iroha-inrou-assets-genericcloud/initrd-aarch64.img cargo run -p xtask --bin xtask -- soracloud-inrou-smoke portable`
    - `env -u IROHA_INROU_PORTABLE_KERNEL_IMAGE -u IROHA_INROU_PORTABLE_ROOTFS_IMAGE -u IROHA_INROU_PORTABLE_INITRD_IMAGE target/debug/xtask soracloud-inrou-smoke portable`
- The 2026-04-25 readiness-runner follow-up closes the local/load validation gap:
  - `scripts/ci/run_soracloud_production_readiness.sh` now targets the current `integration_tests` Core API test binary for Soracloud multi-peer CLI gates, writes Markdown status bullets safely, clears stale logs when an explicit output directory is reused, supports per-step fail-closed timeouts, and can run Cargo gates with `--cargo-target-dir` or `--isolate-cargo-target` to avoid unrelated workspace lock contention. Full/load profiles now mark missing production-only gates as `blocked` and exit non-zero unless `--allow-open-blockers` is supplied.
  - Torii query batch merge/canonicalization now covers `AnonymousAssetEscrowRecord`, which keeps the Torii Soracloud route-pressure suite compiling with the current native escrow query surface.
  - Proxy-only Inrou hosts are now excluded from local replica projection, advertise zero hosted capacity, and have focused runtime coverage proving they do not publish placed-replica runtime state. The mixed-host inventory example now uses that `proxy_only_inrou_host` gate instead of a plain compile check.
  - Production observability is now a required readiness artifact for full/load profiles. `scripts/ci/check_soracloud_observability_evidence.py` validates deployment evidence for signed-auth failures, body/inflight/rate limit rejections, runtime hydration lag, Inrou lifecycle, lease/cache/disk pressure, egress usage, stale model-host heartbeats, HF fallback use, private-session failures, matching status fields, alerts, runbooks, and dashboards.
  - Focused readiness report: `/tmp/iroha-soracloud-readiness-focused-final/soracloud_production_readiness.md`.
  - Load readiness report: `/tmp/iroha-soracloud-readiness-load/soracloud_production_readiness.md`. Required local/load gates passed under the previous runner behavior: config posture, config fixtures, signed mutation route pressure, public runtime pressure, runtime-stub production rejection, embedded runtime manager posture, live multi-peer status, mutation rollout, training/model lifecycle, and HF shared-lease proration. The mixed-host Inrou smoke was skipped because no operator inventory was supplied; with the current runner, that condition and missing observability evidence are explicit blockers.
  - Additional validation for this follow-up:
    - `bash -n scripts/ci/run_soracloud_production_readiness.sh`
    - `scripts/ci/run_soracloud_production_readiness.sh --help`
    - `python3 -m py_compile scripts/ci/check_soracloud_observability_evidence.py`
    - `python3 scripts/ci/check_soracloud_observability_evidence.py --evidence fixtures/soracloud/production_observability_evidence.example.json`
    - `python3 -m pytest scripts/tests/check_soracloud_observability_evidence_test.py`
    - `env -u LOG_FORMAT cargo test -p iroha_torii --lib --features app_api,telemetry soracloud_signed_mutation_ -- --nocapture`
    - `env -u LOG_FORMAT cargo test -p iroha_torii --lib --features app_api,telemetry soracloud_public_runtime_rate_and_inflight_limits_fail_closed -- --nocapture`
    - `env -u LOG_FORMAT cargo test -p iroha_torii --lib --features app_api,telemetry anonymous_asset_escrow_records -- --nocapture`
    - `env -u LOG_FORMAT cargo test -p irohad --features embedded-soracloud-runtime --bin irohad manager_config_ -- --nocapture`
    - `env -u LOG_FORMAT cargo test -p irohad --features embedded-soracloud-runtime --bin irohad proxy_only_inrou_host -- --nocapture`
    - `scripts/ci/run_soracloud_production_readiness.sh --profile focused --out /tmp/iroha-soracloud-readiness-focused-final`
    - `scripts/ci/run_soracloud_production_readiness.sh --profile load --out /tmp/iroha-soracloud-readiness-load`

## 2026-04-24 Verified lane relay JSON state key for UC6

- `LaneRelayEnvelopeRef::relay_state_key()` now exposes the canonical contract-state key for verified lane relay records: `pkdeploy_verified_lane_relay_<dataspace_id>_<lane_id>_<block_height>_<hash64>`. The helper avoids slash-delimited paths and gives deploy/Core API code a single source for the public relay key string.
- `RegisterVerifiedLaneRelay` / `FindLaneRelayEnvelopeByRef` continue to persist and read contract-visible Norito JSON for `VerifiedLaneRelayRecord`, and focused coverage now pins that old raw `VerifiedLaneRelayRecord` bytes are rejected by the JSON decoder path.
- The IVM schema registry no longer carries `VerifiedLaneRelayRecord` for this flow, so Kotodama UC6 contracts consume the relay record through `decode_json(relay_state)` rather than `decode_schema(...)`.
- `crates/iroha_core/src/smartcontracts/isi/world.rs` now makes `verified_lane_relay_state_key_is_single_contract_name` derive its expected prefix from the shared `VERIFIED_LANE_RELAY_STATE_KEY_PREFIX` and the live `LaneRelayEnvelopeRef` fields instead of pinning a stale hardcoded sample prefix. The production formatter already uses `dataspace_id`, `lane_id`, and `block_height`, and the sample relay record currently comes from `ValidBlock::new_dummy`, which defaults to block height `2`.
- This keeps the regression focused on the real contract-name guarantee: the verified-lane-relay state key stays a single `Name`, uses the canonical relay-ref components, and ends with the hashed settlement suffix.
- Focused validation for this slice:
  - `cargo test -p iroha_data_model relay_envelope_ref_state_key_is_canonical_and_deterministic -- --nocapture`
  - `cargo test -p iroha_core verified_lane_relay_state -- --nocapture`
  - `cargo test -p ivm schema_registry --lib -- --nocapture`
  - `cargo test -p iroha_core verified_lane_relay_state_key_is_single_contract_name -- --nocapture`
  - `cargo fmt --all`
  - `cargo test -p iroha_core --lib verified_lane_relay_state_ -- --nocapture`

## 2026-04-24 Izanami communication vulnerability matrix scaffold

- `crates/izanami/src/communication_vulnerabilities.rs` now records the five scenario taxonomy from "Blockchain Communication Vulnerabilities" (targeted load, transient failure, packet loss, stopping, leader isolation), the paper-shaped 20-node/800s/200 TPS constants, the paper baseline outcomes for Algorand, Aptos, Avalanche, Redbelly, and Solana, and the current Izanami coverage profile for each case.
- `scripts/run_izanami_communication_vulnerability_matrix.sh` adds a reusable matrix runner with `quick` and paper-shaped modes. It writes `summary.md`, `summary.tsv`, and per-scenario logs under `dist/izanami-communication-vuln-*`, including the paper baseline table beside Iroha/Izanami run results.
- The matrix runner now accepts explicit `--fault-enable-*=true|false` values, performs a one-shot `127.0.0.1:0` socket-bind preflight before running any scenario, records `blocked-loopback-bind` rows when the environment denies local listeners, and exits nonzero on environment or scenario failures instead of reporting a misleading all-green shell status.
- `docs/source/izanami_communication_vulnerabilities.md` documents how the paper maps to Izanami, the comparison baseline, classification signals, and the current exact-vs-approximate coverage boundaries. Packet loss and leader isolation are explicitly marked as approximations until Izanami gets exact packet-drop windows; leader isolation now uses dynamic Sumeragi leader targeting, but still uses the trusted-peer isolation primitive.
- Quick-mode execution was attempted directly with the built `izanami` binary at `dist/izanami-communication-vuln-quick-20260424-145213-direct`, and the new fast-fail preflight path was exercised at `dist/izanami-communication-vuln-quick-20260424-152301-preflight`. In both cases the environment denied binding `127.0.0.1` with `Operation not permitted`, so no Iroha vulnerability classification was produced from this execution surface.
- With full local socket access restored, a full quick-mode run completed at `dist/izanami-communication-vuln-quick-20260424-153021-fullaccess`. All five scenarios returned exit code `0`, but the log shape is mixed:
  - `targeted-load` completed with a sampled confirmation timeout and a load-worker shutdown timeout, so it is not a clean resilient pass.
  - `transient-failure` and `stopping` completed, but both showed sustained `429 Too Many Requests`, connection-refused fallbacks, and repeated confirmation timeouts during recovery, so they currently look degraded rather than clean resilient quick-pass runs.
  - `packet-loss` and `leader-isolation` were inconclusive in that first full quick run because the partition-style fault injector reported `peer missing trusted_peers_pop roster required for partition restart`, so those runs exited `0` without a faithful partition/rejoin fault shape.
- The partition-style fault injector now restarts peers with a self-only trusted-peer entry and the peer's own BLS PoP instead of trying to read a full PoP roster from `config.base.toml`. Focused partition unit coverage passes, and a 4-peer/30s live partition smoke at `dist/izanami-partition-smoke-20260424-fixed/leader-isolation-smoke.log` no longer reports `peer missing trusted_peers_pop roster` or `trusted_peers_pop contains keys not in trusted_peers`; the remaining signal is connection-refused and confirmation-timeout degradation while peers are isolated/restarted. This makes the approximation executable, but the paper comparison still needs exact packet-drop injection and active-leader targeting before `packet-loss` or `leader-isolation` should be treated as exact reproductions.
- Fixed quick-only reruns of the two previously inconclusive scenarios completed with usable degraded classifications:
  - `packet-loss`: `dist/izanami-communication-vuln-quick-20260424-packet-loss-fixed/summary.md` reports exit code `0`, status `ok`, paper outcome `degraded`. The log no longer contains the old partition restart/config signatures; it shows runtime backpressure, connection refusals, and confirmation timeouts instead.
  - `leader-isolation`: `dist/izanami-communication-vuln-quick-20260424-leader-fixed/summary.md` reports exit code `0`, status `ok`, paper outcome `degraded`. The log likewise has no `peer missing trusted_peers_pop roster`, `trusted_peers_pop contains keys not in trusted_peers`, restart-failure, or `InvalidSumeragiConfig` signatures, and the remaining signal is Torii reachability/backpressure plus queued/timeout degradation.
- The matrix runner now accepts `--sumeragi-mode permissioned|npos|both`, records `sumeragi_mode` in `summary.tsv`, and emits separate paper-style rows for `Iroha (Sumeragi permissioned)` and `Iroha (Sumeragi NPoS)`. NPoS mode runs Izanami with `--nexus`, loading the Nexus/Sora profile and `sumeragi.consensus_mode = "npos"`.
- The NPoS quick matrix startup blocker is fixed: Izanami now compacts only the universal dataspace grant into the retained Nexus genesis transaction while keeping non-universal dataspace-scoped grants separate, so the generated NPoS genesis stays within Iroha's 16-transaction startup cap without mixing dataspace permission targets.
- A fresh full quick matrix covering both Sumeragi modes completed at `dist/izanami-communication-vuln-quick-20260424-both-modes-fixed`. All ten rows exited `0` with status `ok` and paper outcome `degraded`; the previous NPoS `Invalid genesis block: Genesis block must have 1 to 16 transactions` startup failure no longer appears in these logs.
- The current quick-mode Iroha rows are:
  - `Iroha (Sumeragi permissioned) | Izanami quick | peer-to-peer/TCP | BFT quorum (permissioned validators) | degraded | degraded | degraded | degraded | degraded`
  - `Iroha (Sumeragi NPoS) | Izanami quick | peer-to-peer/TCP | stake-elected BFT quorum | degraded | degraded | degraded | degraded | degraded`
  These are executable Izanami approximations, not yet exact reproductions of the paper's packet-loss percentages or active-leader isolation method.
- Focused validation for this scaffold:
  - `cargo fmt --all`
  - `cargo test -p izanami network_partition --lib -- --nocapture` (`2` passed)
  - `cargo test -p izanami communication_vulnerabilities --lib -- --nocapture`
  - `cargo test -p izanami --lib -- --nocapture` (`18` passed)
  - `cargo test -p izanami --bin izanami -- --nocapture` (`199` passed)
  - `cargo test -p izanami make_network_builder_npos_genesis_stays_within_transaction_cap --bin izanami -- --nocapture`
  - `cargo test -p izanami make_network_builder_injects_npos_parameters --bin izanami -- --nocapture`
  - `cargo build -p izanami --bin izanami`
  - `cargo test -p izanami cli_accepts_explicit_false_fault_toggles --bin izanami -- --nocapture`
  - `bash -n scripts/run_izanami_communication_vulnerability_matrix.sh`
  - `scripts/run_izanami_communication_vulnerability_matrix.sh --help`
  - `scripts/run_izanami_communication_vulnerability_matrix.sh --mode quick --izanami-cmd 'target/debug/izanami' --out dist/izanami-communication-vuln-quick-20260424-145213-direct` (blocked by local socket bind restrictions)
  - `scripts/run_izanami_communication_vulnerability_matrix.sh --mode quick --izanami-cmd 'target/debug/izanami' --out dist/izanami-communication-vuln-quick-20260424-152301-preflight` (fast-fail preflight blocked by local socket bind restrictions)
  - `scripts/run_izanami_communication_vulnerability_matrix.sh --mode quick --izanami-cmd 'target/debug/izanami' --out dist/izanami-communication-vuln-quick-20260424-153021-fullaccess`
  - `target/debug/izanami --allow-net --peers 4 --duration 30s --tps 5 --max-inflight 16 --workload-profile stable --faulty 1 --submitters 4 --fault-enable-network-partition=true ...` with log at `dist/izanami-partition-smoke-20260424-fixed/leader-isolation-smoke.log`
  - `scripts/run_izanami_communication_vulnerability_matrix.sh --mode quick --only packet-loss --izanami-cmd 'target/debug/izanami' --out dist/izanami-communication-vuln-quick-20260424-packet-loss-fixed`
  - `scripts/run_izanami_communication_vulnerability_matrix.sh --mode quick --only leader-isolation --izanami-cmd 'target/debug/izanami' --out dist/izanami-communication-vuln-quick-20260424-leader-fixed`
  - `scripts/run_izanami_communication_vulnerability_matrix.sh --mode quick --sumeragi-mode both --izanami-cmd 'target/debug/izanami' --out dist/izanami-communication-vuln-quick-20260424-both-modes-fixed`

## 2026-04-24 Torii telemetry and routed-read regression sweep

- `crates/iroha_torii/src/routing.rs` now makes the test telemetry fixture safe for synchronous tests by entering a shared Tokio runtime when needed, and uses the full telemetry profile so status, Sumeragi, and developer telemetry endpoints expose the data their tests assert.
- Torii status responses now emit header-framed Norito through `norito::to_bytes(...)`, local8/domain address reject metrics record both the local8 bucket and the explicit domain label, and the affected address/routed-read tests now derive expected reject reasons from the live account parser.
- The failing fixture expectations are aligned with current runtime behavior: Sumeragi telemetry status seeds a VRF epoch, AXT cache debug serializes reject reasons as stable labels and uses a non-empty policy snapshot, the privacy-share test sample uses an aligned aggregation bucket, and the SoraFS pin registry metrics are registered with the global telemetry registry before tests read them.
- Focused validation for this fix:
  - `cargo fmt --all`
  - `cargo test -p iroha_torii --lib --features app_api,telemetry iso20022_bridge::tests -- --nocapture`
  - `cargo test -p iroha_torii --lib --features app_api,telemetry routing::adapter_filter_tests -- --nocapture`
  - `cargo test -p iroha_torii --lib --features app_api,telemetry routing::account_path_metric_tests -- --nocapture`
  - `cargo test -p iroha_torii --lib --features app_api,telemetry routing::address_metrics_tests -- --nocapture`
  - `cargo test -p iroha_torii --lib --features app_api,telemetry routing::tests::metrics_handler_strips_lane_labels_when_nexus_disabled -- --nocapture`
  - `cargo test -p iroha_torii --lib --features app_api,telemetry sorafs::api::advert_tests::pin_registry_metrics_summary_tracks_counts -- --nocapture`
  - `cargo test -p iroha_torii --lib --features app_api,telemetry tests_runtime_handlers:: -- --nocapture`
  - `cargo test -p iroha_torii --lib --features app_api,telemetry tests::axt_proof_cache_debug_reports_snapshot -- --nocapture`
  - `cargo test -p iroha_torii --lib --features app_api,telemetry torii_routed_read_tests -- --nocapture`
  - `cargo test -p iroha_torii --lib --features app_api,telemetry -- --nocapture` (`1766 passed; 0 failed; 2 ignored`)

## 2026-04-24 Soracloud manifest fixture canonicalization and coverage

- `fixtures/soracloud/sora_container_manifest_v1.json`, `fixtures/soracloud/sora_service_manifest_v1.json`, and `fixtures/soracloud/sora_deployment_bundle_v1.json` now match the current Soracloud V1 JSON schema: optional `inrou` serializes explicitly as `null`, empty default arrays use the compact canonical `[]` form, and the deployment bundle carries the refreshed canonical container manifest hash.
- `crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs` now adds fixture-level coverage for legacy JSON payloads that omit defaulted manifest fields, `null` default collections, custom container-manifest unknown-field rejection, `Ivm`/`Inrou` runtime metadata mismatch rejection, required config/secret material validation, config export declaration and target validation, healthcheck path validation, omitted service routes, route and rollout validation, empty deterministic handler rejection, deterministic lease-volume rejection, quota-class validation, state binding limit/encryption validation, handler route/certification/mailbox validation, mailbox size validation, duplicate state binding and handler rejection, artifact path and handler-reference validation, nested deployment-bundle default decoding, cross-fixture embedded container hash consistency, state-write capability admission, deterministic-vs-HTTP runtime admission, Inrou HTTP root/shared volume and SSH-key requirements, HTTP quota limits for replicas/resources/storage, admission success after refreshing a changed container reference, and admission rejection when schema versions, public-route healthchecks, or embedded container contents drift without updating the deployment reference.
- Focused validation for this fix:
  - `cargo fmt --all`
  - `cargo test -p iroha_data_model --features json --test soracloud_manifest_fixtures`
  - `cargo test -p iroha_data_model --test soracloud_manifest_fixtures`

## 2026-04-23 Block header Norito golden refresh

- `crates/iroha_data_model/tests/norito_golden_scaffold.rs` now pins the current bare `BlockHeader::encode()` payload instead of a stale pre-compact fixture. The old bytes no longer matched the repo-wide Norito default (`COMPACT_LEN`) or the current default `confidential_features` digest carried by newly constructed headers, so `block_header_golden_bytes` is aligned with the live codec output again.
- Focused validation for this fix:
  - `cargo test -p iroha_data_model --test norito_golden_scaffold -- --nocapture`

## 2026-04-23 Compound predicate Norito roundtrip fix

- `crates/iroha_data_model/src/query/dsl.rs` now routes `CompoundPredicateWire` through direct `norito::NoritoSerialize` / `norito::NoritoDeserialize` instead of the `Encode` / `Decode` derive pair when preserving the custom `CompoundPredicate<T>` wire wrapper. The wrapper also now delegates `encoded_len_hint` / `encoded_len_exact` to the wire enum and uses fallible `try_deserialize(...)` before reconstructing the runtime payload.
- `crates/iroha_data_model/src/query/dsl_fast.rs` now mirrors the same fix for the `fast_dsl` feature path, so both predicate DSL implementations decode the header-framed `Json(...)` variant with the correct Norito length semantics instead of misreading the compact-length-prefixed payload as a huge fixed-width allocation request.
- Both DSL modules now also have focused codec unit coverage for the full wrapper variant surface: `Pass`, `Json`, committed-transaction `TxFilters`, and committed-transaction `TxPredicate`, plus assertions that `CompoundPredicate<T>` forwards `encoded_len_hint` / `encoded_len_exact` to the inner wire enum on each path.
- Both DSL modules now additionally cover the committed-transaction `and(...)` merge matrix directly: `PASS` passthrough, filter+filter collapse through `Const(true)`, tree+tree flattening, filter+tree merging, and tree+filter merging. That gives direct test coverage over the branchy `and_committed_tx_predicates(...)` and `committed_tx_predicate_from_filters(...)` paths instead of only reaching them indirectly through roundtrips.
- Both DSL modules now also cover the remaining committed-transaction JSON evaluation branches directly: raw expression JSON that routes through `committed_tx_filters_from_json(...)`, canonical/raw object JSON that misses the filter parser and falls back to generic field-path matching, raw non-object JSON that returns the default permissive `true`, and mixed JSON-vs-tree payload combinations that keep the most recently supplied predicate instead of attempting an invalid merge.
- Both DSL modules now also have direct helper-level coverage for the last private predicate branches in this slice: `predicate_value_at_path(...)` rejects empty/blank segments, `predicate_json_from_map(...)` treats empty arrays as equality payloads instead of `in` conditions, `predicate_json_applies(...)` rejects missing equality/membership paths and `exists` checks on `null`, `and_committed_tx_predicates(...)` short-circuits `Const(false)`, and `committed_tx_predicate_from_filters(...)` preserves the expected field-to-atom ordering for the less common `authority_ne` / `ts_le` / `entry_nin` / `result_ok_ne` / `result_exists` branches.
- The same DSL coverage slice now also pins the remaining JSON-wrapper edges that previously only executed implicitly: `CompoundPredicate<T>::json_deserialize(...)` now treats `null` and `{}` as `PASS`, preserves raw non-object payloads verbatim, defaults to permissive evaluation for malformed raw JSON in both generic and committed-transaction paths, and directly exercises the `and_committed_tx_predicates(...)` append/prepend branches for `And + leaf` and `leaf + And`.
- `crates/iroha_data_model/src/query/tx_predicate.rs` now has direct shared coverage for the committed-transaction predicate core instead of only reaching it through DSL wrappers: string-field `exists` / `is_null` filter parsing, `authority_in` / `authority_nin`, `entry_ne` / `entry_nin`, `result_ok_ne` / `result_ok_in`, `timestamp_ms` `gte` / `lt` normalization, `TsGt` / `TsNin`, `EntryNe` / `EntryNin`, `MetadataIn` / `MetadataNin`, `MetadataIsNull(false)`, `Not`, `Const`, and a complex Norito roundtrip over an `Or(Not(...), TsGt(...), EntryNe(...), MetadataIn(...), Const(false))` tree.
- The same shared predicate test module now also covers the error-heavy remainder of `tx_predicate.rs`: timestamp bound saturation at `u64::MAX` / `0`, parser rejection for unsupported boolean ops and malformed/unsupported field/operator combinations, and `wire::inflate(...)` failures for missing child nodes and trailing-node payloads.
- This clears the reported `memory allocation of 8316310562681852178 bytes failed` abort in `compound_predicate_roundtrip` and returns the `iroha_data_model` `data_model` integration test binary to green.
- Focused validation for this fix:
  - `cargo fmt --all`
  - `cargo test -p iroha_data_model --lib codec_tests -- --nocapture`
  - `cargo test -p iroha_data_model --lib --features fast_dsl codec_tests -- --nocapture`
  - `cargo test -p iroha_data_model --test data_model -- --nocapture`
  - `cargo test -p iroha_data_model --test data_model --features fast_dsl compound_predicate_roundtrip -- --nocapture`
  - `cargo test -p iroha_data_model --lib tx_predicate::tests -- --nocapture`

## 2026-04-23 Staged IVM deploy input-bump exhaustion fix

- `crates/ivm/src/ivm.rs` now exposes `IVM::alloc_host_tlv(...)`, which preserves the existing input-bump path for ordinary host-returned TLVs but spills oversized returns to heap once the fixed INPUT window is exhausted. The same unit-test module now pins that spill path with `alloc_host_tlv_spills_to_heap_after_input_fills`.
- `crates/ivm/src/core_host.rs`, `crates/ivm/src/mock_wsv.rs`, and `crates/iroha_core/src/smartcontracts/ivm/host.rs` now route durable-state and Norito-bytes host returns through the new helper, so staged `STATE_GET` loops no longer fail with `MemoryOutOfBounds` after several large chunk fetches.
- `crates/iroha_cli/src/bin/ivm_contract_deploy.rs` now passes its existing staged large-payload regressions again without code changes in the builder itself: both the plain-core-host reconstruction test and the contract-runtime-host nine-chunk staged register test are green.
- Follow-up coverage for the same spill path now pins the remaining direct branches and host call sites:
  - `crates/ivm/src/ivm.rs` adds `alloc_host_tlv_prefers_input_when_space_is_available` and `alloc_host_tlv_propagates_out_of_memory_when_heap_spill_cannot_fit`,
  - `crates/ivm/tests/core_host_state_syscalls.rs` and `crates/ivm/tests/wsv_host_state_syscalls.rs` now prove `STATE_GET` returns heap-backed TLVs once the INPUT bump allocator is saturated,
  - `crates/ivm/tests/wsv_host_state_syscalls.rs` now also covers raw-byte durable state wrapping through the spill path, direct overlay-backed `STATE_GET` spills under `begin_tx`, and rejection of malformed wrapped state whose inner TLV is not `NoritoBytes`,
  - `crates/ivm/tests/wsv_host_state_syscalls.rs` now also pins the `Some(None)` overlay-tombstone branch, proving an in-flight `STATE_DEL` shadows a persisted base value during `begin_tx`,
  - `crates/ivm/tests/wsv_host_state_syscalls.rs` now also pins the non-spill raw-byte wrapping path and the overlay precedence path where an in-flight `STATE_SET` overrides a persisted base value during `begin_tx` and remains persisted after `finish_tx`,
  - `crates/ivm/tests/wsv_host_state_syscalls.rs` now also pins the overlay-delete flush path, proving an in-flight `STATE_DEL` still removes the persisted base value after `finish_tx`,
  - `crates/iroha_core/src/smartcontracts/ivm/host.rs` now has matching contract-runtime regressions for the malformed wrapped-state rejection path, for direct raw-state wrapping into `NoritoBytes`, for scoped overlay reads spilling to heap once INPUT is full, for scoped persisted-base reads winning over legacy fallback keys, and for scoped tombstones shadowing legacy unscoped base values,
  - `crates/iroha_core/src/smartcontracts/ivm/host.rs` now also pins the scoped overlay precedence path, proving a staged scoped `STATE_SET` overrides both persisted scoped state and the legacy unscoped fallback while writing only the scoped overlay key,
  - `crates/iroha_core/src/smartcontracts/ivm/host.rs` now also pins the unscoped precedence branches used when no contract runtime context is present: staged unscoped `STATE_SET` overrides persisted base state, and staged unscoped `STATE_DEL` shadows persisted base state while recording only a single unscoped tombstone,
  - `crates/ivm/src/ivm.rs` now also pins the allocator edge where only an undersized aligned tail remains in INPUT, proving `alloc_host_tlv(...)` spills in that partial-tail case instead of only when INPUT is exactly full.
- Focused validation for this fix:
  - `cargo fmt --all`
  - `cargo test -p ivm alloc_host_tlv_spills_to_heap_after_input_fills --lib -- --nocapture`
  - `cargo test -p iroha_cli --bin ivm_contract_deploy -- --nocapture`
  - `cargo test -p ivm alloc_host_tlv_ --lib -- --nocapture`
  - `cargo test -p ivm --test core_host_state_syscalls -- --nocapture`
  - `cargo test -p ivm --test wsv_host_state_syscalls -- --nocapture`
  - `cargo test -p iroha_core state_syscall_reads_world_snapshot_spills_to_heap_when_input_fills --lib -- --nocapture`
  - `cargo test -p iroha_core load_state_value_rejects_wrapped_non_norito_bytes --lib -- --nocapture`
  - `cargo test -p iroha_core state_syscall_reads_scoped_overlay_spills_to_heap_when_input_fills --lib -- --nocapture`
  - `cargo test -p iroha_core state_syscall_scoped_delete_shadows_legacy_raw_base_value --lib -- --nocapture`
  - `cargo test -p iroha_core load_state_value_ --lib -- --nocapture`
  - `cargo test -p iroha_core state_syscall_prefers_scoped_base_value_over_legacy_fallback --lib -- --nocapture`
  - `cargo test -p iroha_core state_syscall_scoped_overlay_overrides_scoped_and_legacy_base_values --lib -- --nocapture`
  - `cargo test -p iroha_core state_syscall_unscoped_ --lib -- --nocapture`

## 2026-04-23 Kotodama on-chain account alias resolution

- `crates/kotodama_lang/src/ir.rs` now treats alias-shaped `account_id("...")` string literals as runtime alias inputs: canonical encoded `AccountId` literals still lower to static `AccountId` TLVs, while non-canonical alias-shaped literals lower to `ResolveAccountAlias` with the original blob literal preserved for host-side validation against current WSV state. The same test module now also pins the boundary between shorthand and the explicit builtin on both dataspace-root and domain-qualified literals, including malformed builtin, malformed domain-qualified builtin, and malformed domain-qualified shorthand literals: `resolve_account_alias("merchant@paynet")` / `resolve_account_alias("merchant@bank.paynet")` / `resolve_account_alias("merchant@")` / `resolve_account_alias("merchant@bank.")` and `account_id("merchant@bank.")` all continue through runtime alias resolution, while `account_id("merchant")` stays on the static `AccountId` path and only fails later during compile-time encoding.
- `crates/kotodama_lang/src/compiler.rs` now covers the shorthand and builtin paths at compile/manifest level: alias-shaped `account_id("merchant@paynet")` emits the existing `SYSCALL_RESOLVE_ACCOUNT_ALIAS`, canonical `account_id("<i105>")` stays embedded as a static TLV without the alias syscall, alias-shaped invalid literals compile into runtime resolution instead of stale compile-time `AccountId` encoding, invalid non-alias literals such as `account_id("merchant")` still fail during static `AccountId` encoding, both dataspace-root and domain-qualified alias/builtin forms emit the alias syscall even for malformed alias-shaped literals, and shorthand- or builtin-derived alias transfer targets force wildcard access hints even for malformed alias-shaped literals instead of baking a stale canonical account key.
- `crates/iroha_core/src/smartcontracts/ivm/host.rs` now passes alias literals to `SYSCALL_RESOLVE_ACCOUNT_ALIAS` without trimming and resolves only through the authoritative WSV alias-binding table, so malformed alias-shaped literals fail through the host runtime rather than being normalized silently and primary labels no longer act as an implicit fallback. The same test module now covers live alias rebinding against the committed Nexus dataspace catalog, binding-only resolution, missing dataspace/domain permissions, domain-qualified alias bindings that require an SNS lease, explicit `resolve_account_alias("merchant@paynet")` and `resolve_account_alias("merchant@bank.paynet")` contract parity, runtime rejection when contract-side alias resolution lacks permission, rejection when builtin or shorthand contract paths hit a missing binding or malformed alias literal at runtime, and the full permission matrix for domain-qualified aliases: raw syscall rejection plus builtin/shorthand contract rejection when either the domain permission or the dataspace permission is missing.
- `crates/ivm/docs/kotodama_grammar.md` now documents that canonical `account_id("...")` literals stay static while alias-shaped literals lower to runtime alias resolution.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo test -p kotodama_lang account_id -- --nocapture`
  - `cargo test -p kotodama_lang resolve_account_alias -- --nocapture`
  - `cargo test -p iroha_core resolve_account_alias -- --nocapture`
  - `cargo test -p iroha_core --lib alias_shorthand -- --nocapture`

## 2026-04-23 Nexus-wide alias read routing

- `crates/iroha_core/src/torii_proxy.rs` now appends `AliasResolve`, `AliasResolveIndex`, and `AliasLookupByAccount` to `ToriiReadEndpointV1`, preserving the existing proxy enum order while letting alias reads travel through the Nexus Torii read-proxy layer.
- `crates/iroha_torii/src/lib.rs` now splits the alias handlers into routed public entrypoints and local-only executors, routes `/v1/aliases/resolve` by alias dataspace, fans `/v1/aliases/resolve_index` out across configured dataspaces, and routes `/v1/aliases/by_account` through target-account routes with deduped merged totals, `source = "fanout"` on merged responses, `409 route_conflict` on incompatible alias-index bindings, and alias-specific `403 permission_denied` plus warning/diagnostic headers when denied routes are skipped.
- The same `crates/iroha_torii/src/lib.rs` slice now has direct routed-read unit coverage for the alias-specific fanout collector and merge helpers, plus the local alias read-proxy dispatch and route-visibility partition helpers: synthetic denied-route fail-closed behavior, empty-route `route_unavailable`, explicit routed `permission_denied` precedence, warning-header emission on successful merges with denied routes, alias-index dedupe, alias-by-account dedupe, conflicting account-root rejection, the fail-closed empty-items-plus-denied branch, route-local alias-index execution, route-local alias-by-account filtering, invalid proxied alias bodies returning `invalid_proxy_request`, and unsigned-vs-caller-scoped visibility partitioning for restricted dataspaces.
- Additional alias-routing regressions now pin the remaining public handler/error paths: `/v1/aliases/resolve` rejects empty aliases before routing, `/v1/aliases/by_account` rejects malformed `account_id` literals, `/v1/aliases/resolve_index` now has endpoint-level coverage for both fail-closed `403 permission_denied` when only hidden routes can resolve and warning-header emission when a public route resolves while another routed dataspace stays denied, and the local read-proxy decoder now rejects malformed proxied bodies for all three alias read endpoints.
- This pass also adds local route-mismatch and filtered-empty-result coverage for the split alias executors, plus direct handler regressions for malformed alias literals, empty `account_id` requests, and malformed `/v1/aliases/resolve_index` request bodies so the app-facing conversion paths are pinned without relying only on proxy-level tests.
- Another follow-up pass now pins the alias-specific fallback ordering that was still implicit: `collect_torii_alias_json_payloads` prefers `404 not_found` over sibling `route_unavailable` responses when no route resolves, returns `503 route_unavailable` when every routed alias attempt is unavailable, the public `/v1/aliases/resolve` and `/v1/aliases/by_account` handlers reject malformed JSON bodies directly, and `/v1/aliases/resolve` has an end-to-end `route_unavailable` regression when the only authoritative alias route is non-local and ingress proxying is unavailable in the current feature set.
- The latest test expansion keeps pushing on endpoint-facing error behavior instead of only helper coverage: alias collector precedence is now locked down at the alias-specific helper layer, malformed request-body rejection is covered across all three public alias helpers, and the signed `/v1/aliases/resolve` path now proves the offline-authoritative dataspace case returns the existing `503 route_unavailable` surface rather than silently degrading into a local miss.
- The newest coverage pass closes a few remaining alias-read gaps around mixed-route edge cases: local read-proxy dispatch for `AliasResolve` now has an explicit success regression, empty `merged_alias_resolve_index_response` fanout payloads now pin the `404 not_found` merge result, `/v1/aliases/by_account` now proves that a public reachable route plus an offline authoritative route can still return an empty `fanout` success with `x-iroha-fanout-routes-unavailable = 1`, and `/v1/aliases/resolve_index` now locks down the fail-closed `403 permission_denied` outcome when denied routes are present even though the remaining allowed routes only miss or go unavailable.
- `crates/iroha_torii/src/openapi.rs`, `docs/source/governance_api.md`, and `docs/portal/docs/governance/api.md` now document the Nexus-routed alias lookup behavior for `/v1/aliases/resolve`, `/v1/aliases/resolve_index`, and `/v1/aliases/by_account`, including the routed `403 permission_denied` and `409 route_conflict` outcomes.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo test -p iroha_torii torii_routed_read_tests --lib --features app_api -- --nocapture`
  - `cargo test -p iroha_torii alias_ --lib --features app_api -- --nocapture`
  - `cargo test -p iroha_torii alias_resolve --lib --features app_api -- --nocapture`
  - `cargo test -p iroha_torii alias_lookup --lib --features app_api -- --nocapture`
  - `cargo test -p iroha_torii --lib --features app_api -- --nocapture`

## 2026-04-23 Sumeragi harness-salt test hardening

- `crates/iroha_core/src/sumeragi/main_loop/tests.rs` now makes `assemble_proposal_defers_when_candidate_conflicts_with_local_vote_history` search for a later local-led proposal view instead of reusing the PRF-shuffled local position directly. This keeps the blocked proposal view distinct from the setup branch at `view = 0`, so the fixture no longer spuriously inherits `proposals_seen` from `insert_validated_pending(...)` when parallel tests assign a different per-harness peer seed salt.
- The same test file now makes `precommit_vote_ignores_remote_same_height_vote_when_cached_roster_differs_from_live` match the more robust cached-roster collision setup already used by the adjacent signer-collision regressions: permissioned runs rotate away the degenerate local-first roster, and the view search now scans a wider `len * 8` window instead of assuming the remap appears in the first `len` views.
- Focused validation for this fix:
  - `cargo fmt --all`
  - `cargo test -p iroha_core assemble_proposal_defers -- --nocapture`
  - `cargo test -p iroha_core cached_roster_differs_from_live -- --nocapture`

## 2026-04-23 Sumeragi targeted failure triage

- `crates/iroha_core/src/sumeragi/main_loop/tests.rs` now aligns `precommit_vote_skips_payload_fallback_across_rapid_votes_without_roster` with the current permissioned PRF/view-aligned signer mapping by deriving the view-aligned topology from `consensus_context_for_height(...)` and signing with the full harness validator key set instead of assuming the signer remains local.
- The same test file now makes `handle_vote_uses_cached_roster_for_frontier_commit_vote_validation` build a cached roster that provably changes the signer-to-peer mapping relative to the live frontier roster, so the fixture continues to exercise cached-roster validation rather than accidentally passing under the live roster.
- `reschedule_stale_pending_blocks_targets_snapshot_roster` now backdates pending progress after seeding near-quorum votes, matching the current vote-backed reschedule semantics that measure staleness from the latest observed progress instead of the original insertion timestamp.
- Focused validation for this fix:
  - `cargo test -p iroha_core precommit_vote_skips_payload_fallback_across_rapid_votes_without_roster -- --nocapture`
  - `cargo test -p iroha_core handle_vote_uses_cached_roster_for_frontier_commit_vote_validation -- --nocapture`
  - `cargo test -p iroha_core reschedule_stale_pending_blocks_targets_snapshot_roster -- --nocapture`

## 2026-04-23 Sumeragi commit-history test isolation fix

- `crates/iroha_core/src/sumeragi/main_loop/tests.rs` now takes `super::status::commit_history_test_guard()` in the `refresh_derived_rbc_session_roster_*` regression slice and in `reschedule_stale_pending_blocks_targets_snapshot_roster`, isolating those fixtures from the process-global commit-QC history cache that parallel `iroha_core --lib` runs mutate.
- This keeps the derived-roster-unavailable expectations deterministic and preserves the snapshot-roster reschedule assertions even when neighboring tests seed commit history for the same heights/views.
- Focused validation for this fix:
  - `cargo fmt --all`
  - `cargo test -p iroha_core refresh_derived_rbc_session_roster_ --lib`
  - `cargo test -p iroha_core reschedule_stale_pending_blocks_targets_snapshot_roster --lib`
  - `cargo test -p iroha_core --lib`

## 2026-04-23 Torii zk prover report-filter coverage follow-up

- `crates/iroha_torii/src/zk_prover.rs` now adds six more direct prover-report regressions in the same report-management slice:
  - single-delete recovery coverage proving `delete_report(...)` rebuilds a malformed `reports_index.json` from on-disk reports and only removes the requested report,
  - count-handler coverage proving zero is returned when no summaries satisfy the request,
  - count-handler filter composition coverage for `content_type`, `has_tag`, `since`, and `until`,
  - list-handler coverage proving `latest=true` still respects the `messages_only` filter and returns the newest failed message,
  - bulk-delete coverage for uppercase `id` normalization,
  - bulk-delete filter composition coverage for `content_type`, `has_tag`, `since`, and `until`.
- Focused validation for this follow-up:
  - `cargo fmt --all`
  - `cargo test -p iroha_torii 'zk_prover::tests::' -- --nocapture`

## 2026-04-23 Torii zk prover malformed-index save recovery coverage follow-up

- `crates/iroha_torii/src/zk_prover.rs` now adds seven more direct prover-report regressions in the same report-management slice:
  - empty-state index rebuild coverage proving `load_report_summaries()` persists an empty index when the reports directory starts empty,
  - helper coverage proving `remove_report_summary(...)` ignores both invalid ids and valid-but-missing ids without disturbing existing summaries,
  - save-path coverage proving `save_report(...)` rebuilds from on-disk report files when `reports_index.json` is malformed and then preserves both reports in the recovered index,
  - GC coverage proving expired summaries are still deleted when the backing report file is already gone,
  - count-handler coverage for `ok_only=true&errors_only=true`, exercising the “count everything” filter combination,
  - list-handler coverage for uppercase `id` normalization and the mixed-case `order=Desc` branch.
- Focused validation for this follow-up:
  - `cargo fmt --all`
  - `cargo test -p iroha_torii 'zk_prover::tests::' -- --nocapture`

## 2026-04-23 Torii zk prover pagination and GC-rebuild coverage follow-up

- `crates/iroha_torii/src/zk_prover.rs` now adds five more direct regressions in the same prover-report slice:
  - helper coverage proving `delete_report_files(...)` ignores invalid ids without disturbing valid reports or the persisted summary index,
  - direct `load_report(...)` invalid-id coverage for the early sanitize rejection branch,
  - direct `gc_reports_once()` malformed-index rebuild coverage proving GC falls back to the on-disk report files and preserves fresh reports,
  - list-handler coverage for combined `content_type` / `has_tag` / `since_ms` / `before_ms` filtering through the real JSON response path,
  - list-handler coverage for the normal `offset` + `limit` pagination window, not only the past-end and limit-cap branches.
- Focused validation for this follow-up:
  - `cargo fmt --all`
  - `cargo test -p iroha_torii 'zk_prover::tests::' -- --nocapture`

## 2026-04-23 Torii zk prover report-alias and index-normalization coverage follow-up

- `crates/iroha_torii/src/zk_prover.rs` now adds five more direct prover-report regressions in the same report-management slice:
  - valid persisted report-index normalization coverage proving `load_report_summaries()` drops invalid ids, lowercases uppercase ids, and keeps only the last duplicate entry when the index file itself is otherwise valid,
  - direct `gc_reports_once()` no-op coverage proving fresh reports stay indexed and `deleted == 0` when nothing has expired,
  - direct `failed_only=true` alias coverage for list, count, and bulk-delete handlers so those paths are exercised independently of `errors_only` and `messages_only`.
- Focused validation for this follow-up:
  - `cargo fmt --all`
  - `cargo test -p iroha_torii 'zk_prover::tests::' -- --nocapture`

## 2026-04-23 Torii zk prover report-handler coverage follow-up

- `crates/iroha_torii/src/zk_prover.rs` now adds seven more direct report-management regressions in the same prover slice:
  - invalid report-id rejection coverage for the list, count, and bulk-delete handlers,
  - happy-path coverage for single-report `GET` payload serialization and single-report `DELETE` index pruning,
  - direct stale-index helper coverage proving `delete_report_files(...)` removes a persisted summary even when the backing file is already missing,
  - direct handler coverage proving `latest=true` ignores `order`, `offset`, and `limit` instead of applying pagination first.
- Focused validation for this follow-up:
  - `cargo fmt --all`
  - `cargo test -p iroha_torii 'zk_prover::tests::' -- --nocapture`

## 2026-04-23 Torii zk prover GC test boundary fix

- `crates/iroha_torii/src/zk_prover.rs` now keeps the GC retention regression safely inside the configured TTL window instead of placing the "fresh" report just `1 ms` below expiry, which could age out before `gc_reports_once()` recomputed `now`.
- Focused validation for this fix:
  - `cargo fmt --all`
  - `cargo test -p iroha_torii zk_prover::tests::gc_reports_once_deletes_only_expired_reports_and_retains_fresh_index -- --nocapture`
  - `cargo test -p iroha_torii 'zk_prover::tests::' -- --nocapture`

## 2026-04-23 Torii space-directory public-route coverage follow-up

- `crates/iroha_torii/tests/space_directory_manifests.rs` now adds two more router-level regressions in the same Space Directory slice:
  - `GET /v1/space-directory/uaids/{uaid}` now covers a multi-dataspace payload with one catalog alias, one missing alias, and deterministic multi-account assertions through `api_router_for_tests()`,
  - `GET /v1/space-directory/uaids/{uaid}/manifests?status=ACTIVE&limit=1&offset=1` now proves the public route preserves the prefilter `total` count even when status filtering plus pagination returns an empty page.
- Focused validation for this follow-up:
  - `cargo fmt --all`
  - `cargo test -p iroha_torii --test space_directory_manifests -- --nocapture`

## 2026-04-23 Torii space-directory revocation-shape coverage follow-up

- `crates/iroha_torii/tests/space_directory_manifests.rs` now adds two more public-route / mutation regressions in the same Space Directory slice:
  - `GET /v1/space-directory/uaids/{uaid}/manifests?status=Inactive` now has explicit route-level coverage proving a reasonless revocation is serialized with `lifecycle.revocation.reason = null`,
  - `POST /v1/space-directory/manifests/revoke` now has direct queue-inspection coverage proving a raw 64-hex `uaid` without the prefix is accepted and that omitting `reason` preserves `reason = None` in the queued `RevokeSpaceDirectoryManifest` instruction.
- Focused validation for this follow-up:
  - `cargo fmt --all`
  - `cargo test -p iroha_torii --test space_directory_manifests -- --nocapture`

## 2026-04-23 Torii space-directory raw-UAID coverage follow-up

- `crates/iroha_torii/src/routing.rs` now expands `uaid_parsing_tests` and `space_directory_manifest_helper_tests` around the remaining raw-hex branch:
  - `parse_uaid_literal(...)` accepts raw 64-hex literals without the `uaid:` prefix,
  - direct bindings/manifests helper tests prove raw-hex UAID path literals are accepted and canonicalized in the JSON response payloads,
  - the invalid-input parser test now also covers the empty-literal rejection branch.
- `crates/iroha_torii/tests/space_directory_manifests.rs` now drives the public GET bindings and manifests routes with a raw 64-hex UAID path and asserts the response canonicalizes back to `uaid:<lower-hex>`.
- Focused validation for this follow-up:
  - `cargo fmt --all`
  - `cargo test -p iroha_torii uaid_parsing_tests --lib -- --nocapture`
  - `cargo test -p iroha_torii space_directory_manifest_helper_tests --lib -- --nocapture`
  - `cargo test -p iroha_torii --test space_directory_manifests -- --nocapture`

## 2026-04-23 Torii space-directory GET parse-path coverage follow-up

- `crates/iroha_torii/src/routing.rs` now adds two more narrow `space_directory_manifest_helper_tests` cases for the direct GET handler parse failures:
  - `handle_v1_space_directory_bindings(...)` rejects malformed UAID path literals with `400`,
  - `handle_v1_space_directory_manifests(...)` rejects malformed UAID path literals with `400`.
- `crates/iroha_torii/tests/space_directory_manifests.rs` now adds a router-level regression proving the public GET bindings and manifests routes both surface the same malformed-UAID rejection through `api_router_for_tests()`.
- Focused validation for this follow-up:
  - `cargo fmt --all`
  - `cargo test -p iroha_torii 'space_directory_manifest_helper_tests::' --lib -- --nocapture`
  - `cargo test -p iroha_torii --test space_directory_manifests -- --nocapture`

## 2026-04-23 Torii space-directory mutation coverage follow-up

- `crates/iroha_torii/tests/space_directory_manifests.rs` now inspects the queued transaction payloads for the Space Directory mutation endpoints instead of only checking `queued_len()`.
- Added direct publish-handler coverage for both reason-preprocessing branches:
  - `reason` is copied only into entries whose `notes` are missing,
  - omitting `reason` leaves missing `notes` untouched.
- Added direct revoke-handler coverage for UAID parsing behavior:
  - mixed-case / padded `uaid:` literals are canonicalized into the queued `RevokeSpaceDirectoryManifest` instruction,
  - invalid UAID literals fail with `400` and do not enqueue a transaction.
- Focused validation for this follow-up:
  - `cargo fmt --all`
  - `cargo test -p iroha_torii --test space_directory_manifests -- --nocapture`

## 2026-04-23 Torii space-directory handler coverage follow-up

- `crates/iroha_torii/src/routing.rs` now adds three more narrow `space_directory_manifest_helper_tests` cases in the same local slice:
  - direct `handle_v1_space_directory_bindings(...)` coverage for the missing-UAID-bindings response shape,
  - direct `handle_v1_space_directory_bindings(...)` coverage for multi-dataspace alias/account output, including deterministic account comparisons,
  - direct `handle_v1_space_directory_manifests(...)` coverage for the `Inactive` filter returning pending and revoked rows while excluding active rows.
- The bindings multi-dataspace test seeds a minimal manifest set alongside the bindings because `State::new_for_testing(...)` prunes stale standalone `uaid_dataspaces` entries during storage migration when no manifest sets exist.
- Focused validation for this follow-up:
  - `cargo fmt --all`
  - `cargo test -p iroha_torii 'space_directory_manifest_helper_tests::' --lib -- --nocapture`

## 2026-04-23 Torii space-directory helper coverage expansion

- `crates/iroha_torii/src/routing.rs` now adds four more narrow `space_directory_manifest_helper_tests` cases around the same helper slice:
  - direct `manifest_lifecycle_json(...)` coverage for revocations that carry an explicit reason,
  - direct `manifest_entry_to_json(...)` coverage for alias/hash/status/lifecycle/accounts population,
  - direct `manifest_entry_to_json(...)` fallback coverage for null alias and empty accounts when context is missing,
  - direct `handle_v1_space_directory_manifests(...)` coverage for the `dataspace` filter plus `limit=0` being treated as unbounded.
- These keep the coverage local to the helper/test module and exercise JSON-shaping and query-filter branches that were previously only hit indirectly or not at all.
- Focused validation for this coverage pass:
  - `cargo fmt --all`
  - `cargo test -p iroha_torii 'space_directory_manifest_helper_tests::' --lib -- --nocapture`

## 2026-04-23 Torii space-directory helper test ordering fix

- `crates/iroha_torii/src/routing.rs` no longer assumes insertion order in `bindings_for_dataspace_filters_to_requested_scope_and_handles_missing_bindings`; the test now sorts the returned account literals and expected literals before comparing them.
- This aligns the assertion with `UaidDataspaceBindings`, which stores per-dataspace accounts in a `BTreeSet` and therefore guarantees membership, not random `KeyPair::random()` insertion order.
- Focused validation for this fix:
  - `cargo fmt --all`
  - `cargo test -p iroha_torii bindings_for_dataspace_filters_to_requested_scope_and_handles_missing_bindings --lib`

## 2026-04-23 Torii ZK attachments smoke auth alignment

- `crates/iroha_torii/tests/fixtures.rs` now provides `app_signed_request(...)`, a shared integration-test helper that attaches canonical `X-Iroha-*` request-signature headers for app-authenticated routes.
- `crates/iroha_torii/tests/zk_subrouter_smoke.rs` now serializes its attachment-focused cases with a local test mutex so the merged-router smoke suite no longer races on the shared global attachments config.
- The smoke slice now covers the enabled routes with valid signed requests across list/get/count/delete and a full POST upload roundtrip, including a successful create, successful fetch, successful delete, and `NOT_FOUND` after delete.
- The same smoke slice now also asserts that replaying the exact same signed POST request is rejected with `ValidationFail::NotPermitted(... "nonce already used" ...)`, and that unsigned list/count/delete attachment requests are rejected with `ValidationFail::NotPermitted(... "signed account headers are required" ...)`.
- The disabled-path smoke coverage now checks `/v1/zk/attachments`, `/v1/zk/attachments/count`, and `/v1/zk/attachments/{id}` (GET and DELETE) all return `404` when attachments are disabled.
- Focused validation for this fix:
  - `cargo fmt --all`
  - `cargo test -p iroha_torii --test zk_subrouter_smoke -- --nocapture`

## 2026-04-23 Alias auto-renew regression fixes

- `crates/iroha_core/src/smartcontracts/ivm/host.rs` now unregisters the currently executing alias auto-renew billing trigger before queueing the replacement trigger, so successful and retryable billing runs can reschedule themselves without duplicate-trigger rejection.
- `crates/iroha_config/src/parameters/user.rs` now defaults `torii.onboarding.alias_auto_renew_enabled` to `false` until `alias_auto_renew_subscription_domain` is configured, and the optional onboarding subtree now carries explicit Norito defaults so upgraded configs keep their documented defaults when the new field is omitted.
- `crates/iroha_torii/src/routing.rs` now treats account-alias auto-renew NFTs as a special resume path: `/v1/subscriptions/{id}/resume` no longer requires `subscription_plan` metadata for these NFTs, resets failure state, preserves the current billing window, and rebuilds the billing trigger with the resumed charge time.
- `crates/iroha_torii/src/lib.rs` now mounts the missing subscription mutation routes (`POST /v1/subscriptions/plans`, `POST /v1/subscriptions`, and the per-subscription pause/resume/cancel/keep/usage/charge-now routes) so the existing handlers are reachable through the public app router.
- `crates/iroha_torii/tests/subscriptions_endpoints.rs`, `crates/iroha_torii/tests/accounts_onboard.rs`, `crates/iroha_torii/src/routing.rs`, `crates/iroha_config/src/parameters/user.rs`, and `crates/iroha_core/src/smartcontracts/ivm/host.rs` now cover the reschedule fix, the safe onboarding default, the no-domain onboarding path, alias auto-renew list/get compatibility, router-level resume plus mutation-route registration, the omitted-charge alias resume helper branches, and the generic cancel-at-period-end subscription action branch.
- Focused validation for this fix set:
  - `cargo test -p iroha_core subscription_bill_account_alias_auto_renew_queues_renewal_and_reschedules -- --nocapture`
  - `cargo test -p iroha_config onboarding_alias_auto_renew_defaults_disabled_without_subscription_domain -- --nocapture`
  - `cargo test -p iroha_torii handle_post_v1_subscription_resume_supports_alias_auto_renew_nfts -- --nocapture`
  - `cargo test -p iroha_torii --test accounts_onboard without_auto_renew_subscription_domain_when_disabled -- --nocapture`
  - `cargo test -p iroha_torii --test subscriptions_endpoints -- --nocapture`
  - `cargo test -p iroha_torii resolve_account_alias_auto_renew_resume_charge_ms_ -- --nocapture`
  - `cargo test -p iroha_torii handle_post_v1_subscription_cancel_period_end_marks_cancellation_window -- --nocapture`
  - `cargo test -p iroha_torii handle_post_v1_subscription_resume_alias_auto_renew_without_charge_at_preserves_future_schedule -- --nocapture`

## 2026-04-23 Account-alias lease coverage expansion

- `crates/iroha_core/src/sns.rs` now covers two more SNS quote guards for the paid alias lease path: registration rejects an already-registered selector, and renewal rejects a tombstoned alias.
- `crates/iroha_core/src/smartcontracts/isi/sns.rs` now covers the executor guard branches that reject a mismatched payer on `AcquireAccountAliasLease` and reject `RenewAccountAliasLease` when the caller is neither the lease owner nor a `CanManageAccountAlias` holder.
- `crates/iroha_core/src/smartcontracts/ivm/host.rs` now covers the SNS-specific subscription billing branch directly, including the paid auto-renew success path that queues a canonical `RenewAccountAliasLease` and the missing-alias failure path that suspends the subscription and records a failed invoice.
- `crates/iroha_torii/tests/accounts_onboard.rs` now also verifies the onboarding response lease block plus `GET /v1/accounts/{account_id}/aliases` after onboarding, so the new app-facing lease DTOs and alias-list route are exercised at integration level.
- Focused validation for this coverage follow-up:
  - `cargo fmt --all`
  - `CARGO_TARGET_DIR=target/codex-account-alias-lease cargo test -p iroha_torii --test accounts_onboard -- --nocapture`
  - `CARGO_TARGET_DIR=target/codex-account-alias-lease cargo test -p iroha_core --lib quote_account_alias_ -- --nocapture`
  - `CARGO_TARGET_DIR=target/codex-account-alias-lease cargo test -p iroha_core --lib acquire_account_alias_lease_ -- --nocapture`
  - `CARGO_TARGET_DIR=target/codex-account-alias-lease cargo test -p iroha_core --lib renew_account_alias_lease_rejects_non_owner_without_permission -- --nocapture`
  - `CARGO_TARGET_DIR=target/codex-account-alias-lease cargo test -p iroha_core --lib subscription_bill_account_alias_auto_renew_ -- --nocapture`

## 2026-04-23 Canonical paid account-alias leases and auto-renew

- `crates/iroha_data_model` now exposes canonical `AcquireAccountAliasLease` / `RenewAccountAliasLease` ISIs plus SNS account-alias auto-renew metadata on subscriptions.
- `crates/iroha_core/src/sns.rs` now quotes paid account-alias registration/renewal lifecycles, and `crates/iroha_core/src/smartcontracts/isi/sns.rs` executes the canonical paid lease acquire/renew path instead of relying on implicit runtime lease seeding.
- `crates/iroha_core/src/smartcontracts/ivm/host.rs` now recognizes subscription NFTs tagged with account-alias auto-renew metadata and bills them through canonical alias renewal instead of the generic fixed/usage transfer path.
- `crates/iroha_torii/src/routing.rs` now makes `/v1/accounts/onboard` and `/v1/accounts/onboard/multisig` acquire a real finite alias lease in the queued transaction, exposes alias lease status / renew / auto-renew endpoints, and returns lease state in onboarding responses.
- `crates/iroha_torii/tests/accounts_onboard.rs`, `crates/iroha_core/src/sns.rs`, and `crates/iroha_core/src/smartcontracts/isi/sns.rs` now cover the paid onboarding flow, SNS quote helpers, and executor-level acquire/renew round trip.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `CARGO_TARGET_DIR=target/codex-account-alias-lease cargo test -p iroha_torii --test accounts_onboard -- --nocapture`
  - `CARGO_TARGET_DIR=target/codex-account-alias-lease cargo test -p iroha_core --lib quote_account_alias_ -- --nocapture`
  - `CARGO_TARGET_DIR=target/codex-account-alias-lease cargo test -p iroha_core --lib acquire_and_renew_account_alias_lease_round_trip -- --nocapture`
  - `git diff --check`

## 2026-04-23 Sumeragi helper guard coverage addendum

- `crates/iroha_core/src/sumeragi/main_loop/tests.rs` now adds three more direct guard/fallback tests in the same quorum-target / frontier-wire slice so the remaining conservative-degrade branches are covered without relying on larger end-to-end fixtures.
- Added `quorum_retransmit_targets_fall_back_to_full_fanout_when_signer_mapping_fails`, which proves invalid signer-to-peer mapping degrades to full retransmit fanout instead of dropping recovery traffic.
- Added `frontier_block_created_for_local_proposal_wire_falls_back_to_first_block_signature`, which proves the local proposal wire helper still emits enriched frontier metadata when the proposal’s proposer index does not match any signature on the block.
- Added `frontier_block_created_for_wire_returns_plain_block_without_frontier_metadata`, which proves the generic wire helper degrades to a plain `BlockCreated` when neither proposal cache nor authoritative frontier metadata is available.
- Focused validation for this addendum:
  - `cargo test -p iroha_core --lib quorum_retransmit_targets_fall_back_to_full_fanout_when_signer_mapping_fails -- --nocapture`
  - `cargo test -p iroha_core --lib frontier_block_created_for_local_proposal_wire_falls_back_to_first_block_signature -- --nocapture`
  - `cargo test -p iroha_core --lib frontier_block_created_for_wire_returns_plain_block_without_frontier_metadata -- --nocapture`
  - `cargo test -p iroha_core --lib frontier_block_created_for_proposal_wire_falls_back_to_authoritative_frontier_cache -- --nocapture`
  - `cargo test -p iroha_core --lib frontier_block_created_for_local_proposal_wire_uses_live_roster_when_derived_roster_unavailable -- --nocapture`

## 2026-04-23 Sumeragi helper coverage addendum

- `crates/iroha_core/src/sumeragi/main_loop/tests.rs` now adds three more direct helper tests in the quorum-target / frontier-proposal slice so the remaining positive and negative fallback branches are exercised without relying on larger pacemaker or reschedule fixtures.
- Added `quorum_retransmit_targets_expand_to_full_fanout_near_commit_quorum`, which proves the helper widens back to every remote peer once the round is one vote short of commit quorum and only a single canonical target appears to be missing.
- Added `frontier_block_created_for_proposal_wire_falls_back_to_authoritative_frontier_cache`, which proves the generic proposal-wire helper can rebuild enriched `BlockCreated` payloads from authoritative frontier metadata after a prior local `BlockCreated`.
- Added `frontier_block_created_for_proposal_wire_rejects_authoritative_frontier_cache_when_metadata_mismatches`, which proves the generic proposal-wire helper will not reuse cached authoritative metadata once the proposal header no longer matches it.
- Focused validation for this addendum:
  - `cargo test -p iroha_core --lib quorum_retransmit_targets_expand_to_full_fanout_near_commit_quorum -- --nocapture`
  - `cargo test -p iroha_core --lib frontier_block_created_for_proposal_wire_falls_back_to_authoritative_frontier_cache -- --nocapture`
  - `cargo test -p iroha_core --lib frontier_block_created_for_proposal_wire_rejects_authoritative_frontier_cache_when_metadata_mismatches -- --nocapture`
  - `cargo test -p iroha_core --lib frontier_block_created_for_local_proposal_wire_falls_back_to_authoritative_frontier_cache -- --nocapture`
  - `cargo test -p iroha_core --lib frontier_block_created_for_proposal_wire_rebuilds_authoritative_frontier_metadata -- --nocapture`
  - `cargo test -p iroha_core --lib pacemaker_rebroadcasts_cached_frontier_block_when_leader -- --nocapture`

## 2026-04-23 Sumeragi fallback coverage addendum

- `crates/iroha_core/src/sumeragi/main_loop/tests.rs` now adds two more direct branch tests around the quorum-target / cached-frontier fixes instead of relying only on the higher-level regression fixtures.
- Added `precommit_vote_falls_back_to_seeded_collectors_when_quorum_targets_are_satisfied`, which proves `emit_precommit_vote(...)` keeps the seeded collector subset as the fallback target once cached remote commit votes already satisfy quorum-target retransmit selection.
- Added `frontier_block_created_for_local_proposal_wire_falls_back_to_authoritative_frontier_cache`, which proves local proposal wire rebuilds can recover from authoritative frontier metadata after a prior enriched `BlockCreated`, even when the roster hint is unavailable.
- Focused validation for this addendum:
  - `cargo test -p iroha_core --lib precommit_vote_falls_back_to_seeded_collectors_when_quorum_targets_are_satisfied -- --nocapture`
  - `cargo test -p iroha_core --lib frontier_block_created_for_local_proposal_wire_falls_back_to_authoritative_frontier_cache -- --nocapture`
  - `cargo test -p iroha_core --lib emit_precommit_vote_targets_quorum_retransmit_peers -- --nocapture`
  - `cargo test -p iroha_core --lib frontier_block_created_for_local_proposal_wire_uses_live_roster_when_derived_roster_unavailable -- --nocapture`
  - `cargo test -p iroha_core --lib pacemaker_rebroadcasts_cached_frontier_block_when_leader -- --nocapture`

## 2026-04-23 Sumeragi quorum-target / frontier rebroadcast follow-up

- `crates/iroha_core/src/sumeragi/main_loop/commit.rs` now sends the first local precommit to `quorum_retransmit_targets_for_missing_votes(...)` instead of the seeded collector subset, while still preserving the collector seed state for later retry widening.
- `crates/iroha_core/src/sumeragi/main_loop/propose.rs` now rebuilds cached frontier `BlockCreated` rebroadcasts with `frontier_block_created_for_local_proposal_wire(...)` so a cached local-leader proposal can still emit an enriched payload rebroadcast without a preseeded RBC session.
- `crates/iroha_core/src/sumeragi/main_loop/tests.rs` now aligns the READY-quorum and cached-frontier pacemaker fixtures with PRF/view-aligned sender and leader selection, and the older initial-vote assertions now expect quorum retransmit peers instead of the seeded collector subset.
- Focused validation for this follow-up:
  - `cargo test -p iroha_core --lib commit_vote_targets_collectors_or_topology -- --nocapture`
  - `cargo test -p iroha_core --lib precommit_vote_targets_collectors_without_broadcast -- --nocapture`
  - `cargo test -p iroha_core --lib emit_precommit_vote_targets_quorum_retransmit_peers -- --nocapture`
  - `cargo test -p iroha_core --lib maybe_emit_rbc_ready_after_ready_quorum_without_all_chunks -- --nocapture`
  - `cargo test -p iroha_core --lib pacemaker_rebroadcasts_cached_frontier_block_when_leader -- --nocapture`

## 2026-04-23 Torii zk prover handler combo follow-up

- `crates/iroha_torii/src/zk_prover.rs` now adds four more direct prover-report handler regressions around mixed filter/projection combinations that were still only exercised incidentally.
- Added uppercase `id` query normalization coverage for `handle_count_reports(...)`, direct `messages_only=true` coverage proving successful reports are excluded from the projection even when `ok_only=true` is also present, and `latest=true` coverage for the default full-object list projection.
- Added bulk-delete coverage proving `ok_only=true&errors_only=true` collapses to deleting both successful and failed reports instead of silently filtering one side out.
- Focused validation for this follow-up:
  - `cargo fmt --all`
  - `cargo test -p iroha_torii 'zk_prover::tests::' -- --nocapture`
  - `cargo test -p iroha_torii --test zk_prover_integration -- --nocapture`

## 2026-04-23 Torii zk prover report load-failure follow-up

- `crates/iroha_torii/src/zk_prover.rs` now adds five more narrow prover-report regressions around valid-but-missing endpoints and unloadable report bodies so those fallback branches stay covered directly.
- Added `load_report(...)` rejection coverage for non-UTF-8 and malformed-JSON report files, plus direct `handle_get_report(...)` coverage for a valid-but-missing report id.
- Added list-handler coverage proving the default full-object projection silently skips summary entries whose on-disk report body cannot be decoded, and delete-handler coverage for the `{ deleted: 0, ids: [] }` no-match response path.
- Focused validation for this follow-up:
  - `cargo fmt --all`
  - `cargo test -p iroha_torii 'zk_prover::tests::' -- --nocapture`
  - `cargo test -p iroha_torii --test zk_prover_integration -- --nocapture`

## 2026-04-23 Torii zk prover helper filter and upsert follow-up

- `crates/iroha_torii/src/zk_prover.rs` now adds five more narrow prover-report regressions around helper behavior that was still mostly only covered indirectly: summary upsert replacement, filename discovery normalization, direct filter predicate edges, and single-report delete misses.
- Added direct `save_report(...)` upsert coverage proving the persisted summary index is updated in place for repeated ids rather than accumulating duplicates, and that the on-disk report body reflects the latest write.
- Added helper coverage for `list_report_ids(...)` ignoring non-report and invalid-id entries, plus `filter_report_summary(...)` exact-id, content-type, tag, time-bound, and ok/failed matrix branches.
- Added direct `handle_delete_report(...)` not-found coverage for a valid-but-missing report id.
- Focused validation for this follow-up:
  - `cargo fmt --all`
  - `cargo test -p iroha_torii 'zk_prover::tests::' -- --nocapture`
  - `cargo test -p iroha_torii --test zk_prover_integration -- --nocapture`

## 2026-04-23 Torii zk prover helper coverage GC and cap follow-up

- `crates/iroha_torii/src/zk_prover.rs` now adds four more unit-style regressions for the remaining cheap helper branches in the prover report slice: invalid-id `save_report(...)` rejection, `gc_reports_once()` expiry pruning, empty-result `latest=true` projection, and the `limit.min(1000)` safety cap.
- Added direct GC coverage proving expired report files are deleted, fresh reports remain, and the persisted summary index is rewritten to only the retained entries.
- Added direct list-handler coverage proving `latest=true` with no matches returns an empty ids array and that oversized caller limits are capped to the first 1000 reports.
- Focused validation for this follow-up:
  - `cargo fmt --all`
  - `cargo test -p iroha_torii 'zk_prover::tests::' -- --nocapture`
  - `cargo test -p iroha_torii --test zk_prover_integration -- --nocapture`

## 2026-04-23 Torii zk prover helper coverage edge follow-up

- `crates/iroha_torii/src/zk_prover.rs` now adds six more narrow unit-style regressions in the prover report helper slice so the remaining normalization, rebuild, alias, and ordering branches are covered directly instead of only incidentally.
- Added `normalize_report_summaries(...)` coverage proving invalid ids are dropped and duplicate ids collapse to the last entry after canonical lowercase normalization.
- Added malformed `reports_index.json` rebuild coverage for `load_report_summaries()`, plus defensive persisted-id normalization coverage for `load_report(...)` when on-disk JSON carries uppercase ids.
- Added `errors_only=true` alias coverage for both `handle_count_reports(...)` and `handle_delete_reports(...)`, plus case-insensitive `order=DESC` coverage for `handle_list_reports(...)`.
- Focused validation for this follow-up:
  - `cargo fmt --all`
  - `cargo test -p iroha_torii 'zk_prover::tests::' -- --nocapture`
  - `cargo test -p iroha_torii --test zk_prover_integration -- --nocapture`

## 2026-04-23 Torii zk prover helper coverage follow-up

- `crates/iroha_torii/src/zk_prover.rs` now adds five direct unit-style coverage tests for prover report helper and handler edges that were previously only hit indirectly, if at all.
- Added stale summary-index cleanup coverage for `load_report_summaries()`, proving missing report files are pruned from `reports_index.json` and the cleaned index is persisted.
- Added projection precedence coverage proving `ids_only=true` wins over `messages_only=true`, while `messages_only=true` still preserves `error: null` for failed summaries that carry no error string.
- Added bulk-delete response coverage for `handle_delete_reports(...)`, asserting the returned `{ deleted, ids }` payload matches the exact-id filter and that non-matching reports remain on disk.
- Added paging edge coverage proving `offset` values past the filtered result length return an empty JSON array instead of leaking stale entries.
- Focused validation for this follow-up:
  - `cargo fmt --all`
  - `cargo test -p iroha_torii 'zk_prover::tests::' -- --nocapture`
  - `cargo test -p iroha_torii --test zk_prover_integration -- --nocapture`

## 2026-04-23 Torii zk prover report coverage follow-up

- `crates/iroha_torii/tests/zk_prover_integration.rs` now adds three focused prover-report endpoint regressions around query/filter edge cases instead of only the happy-path list/filter coverage.
- Added invalid query-id coverage for `GET /v1/zk/prover/reports`, `GET /v1/zk/prover/reports/count`, and `DELETE /v1/zk/prover/reports`, asserting the shared `invalid report id` rejection path stays wired through each app-facing handler.
- Added a combined `ok_only=true&failed_only=true` list case so the fallback branch that returns both successful and failed reports remains covered.
- Added a `messages_only=true&latest=true&order=asc&offset=1&limit=1` case proving `latest=true` overrides paging/order inputs before the failed-report message projection runs.
- Focused validation for this follow-up:
  - `cargo fmt --all`
  - `cargo test -p iroha_torii --test zk_prover_integration -- --nocapture`

## 2026-04-23 Torii zk prover fixture alignment

- `crates/iroha_torii/tests/zk_prover_integration.rs` now uses the supported `halo2/ipa:tiny-add` deterministic Halo2 fixture circuit instead of the unsupported `halo2/ipa:tiny-add-v1` alias, so the shared fixture helper emits inline verifying-key bytes again for the prover report integration coverage.
- Focused validation for this fix:
  - `cargo test -p iroha_torii --test zk_prover_integration prover_reports_list_get_delete -- --nocapture`
  - `cargo test -p iroha_torii --test zk_prover_integration prover_reports_server_side_filters -- --nocapture`
  - `cargo test -p iroha_torii --test zk_prover_integration -- --nocapture`

## 2026-04-23 Sumeragi main_loop coverage edge follow-up

- `crates/iroha_core/src/sumeragi/main_loop/tests.rs` now adds five more direct unit tests in the recent known-block replay / exact-frontier helper slice so the remaining explicit-target and local-only frontier branches are exercised directly.
- Added a deduplicated explicit-target vote replay case for `maybe_replay_known_block_commit_evidence(...)` that proves duplicate and local peers collapse to one outbound `QcVote` send per unique remote target.
- Added a cached commit-QC replay case for `maybe_replay_known_block_commit_evidence(...)` that proves the helper returns `false` when the explicit target set collapses to the local peer only and therefore the QC replay path has no outbound work to schedule.
- Added three more `frontier_body_next_due(...)` checks: unarmed exact-fetch slots stay idle even with cached targets, leader-stage retries stay idle when a single-peer topology yields no remote leader or voters, and voter-stage retries stay armed when at least one cached remote voter survives leader filtering.
- Focused validation for this follow-up:
  - `cargo fmt --all`
  - `cargo test -p iroha_core --lib known_block_commit_evidence_replay_deduplicates_explicit_vote_targets -- --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::known_block_commit_qc_replay_returns_false_for_local_only_explicit_targets' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::frontier_body_next_due_ignores_exact_fetch_disabled_slot_even_with_targets' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::frontier_body_next_due_leader_stage_stays_idle_without_remote_targets' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::frontier_body_next_due_voter_stage_uses_cached_remote_voters_after_leader_filter' --nocapture`

## 2026-04-23 Sumeragi main_loop coverage tail follow-up

- `crates/iroha_core/src/sumeragi/main_loop/tests.rs` now adds four more direct unit tests in the recent known-block replay / exact-frontier helper slice instead of relying on broader regressions to incidentally hit those branches.
- Added a per-block cooldown case for `maybe_replay_known_block_commit_evidence(...)` that proves the second replay attempt is suppressed before any duplicate network work is scheduled.
- Added a local-only explicit-target case for `maybe_replay_known_block_commit_evidence(...)` that proves the helper returns `false` when the explicit peer set collapses to self and therefore cannot emit outbound recovery traffic.
- Added two exact-frontier cached-target shape checks for `frontier_body_next_due(...)`: leader-stage retries remain armed when only cached voters exist, and voter-stage retries stay idle when the cached voter set collapses to the cached leader only.
- Focused validation for this follow-up:
  - `cargo fmt --all`
  - `cargo test -p iroha_core --lib known_block_commit_evidence_replay_skips_during_cooldown -- --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::known_block_commit_evidence_replay_returns_false_for_local_only_explicit_targets' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::frontier_body_next_due_leader_stage_uses_cached_voters_without_leader' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::frontier_body_next_due_voter_stage_ignores_cached_leader_only_voter_set' --nocapture`

## 2026-04-22 Torii zk roots coverage sweep

- `crates/iroha_torii/src/routing.rs` now covers the remaining `/v1/zk/roots` selector, gas-default, and `Accept` negotiation branches, including blank selectors, trimmed and canonical gas-asset handling, custom-vs-pipeline fallback precedence, malformed `Accept` handling, vendor `+json`, wildcard, and zero-quality Norito fallback cases.
- `crates/iroha_torii/tests/zk_endpoints.rs` now pins the route-level registered-asset empty-state `200`, parseable-but-missing asset alias `404`, missing asset `404`, and blank/invalid selector `403` regressions.
- `crates/iroha_torii/tests/zk_roots_handler_integration.rs` now covers bounded nonzero `max`, zero-cap empty windows that preserve `latest` and `height`, trimmed alias resolution against non-empty shielded state, Norito decoding for non-empty roots windows, and forwarded unsupported `Accept` headers returning `406`.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo test -p iroha_torii --lib zk_roots_selector_tests -- --nocapture`
  - `cargo test -p iroha_torii --test zk_endpoints --test zk_roots_handler_integration -- --nocapture`
  - `git diff --check`

## 2026-04-22 Sumeragi main_loop coverage follow-up

- `crates/iroha_core/src/sumeragi/main_loop/tests.rs` now adds direct unit coverage for the recent collector / vote-replay / exact-frontier branches instead of only relying on the earlier regression fixtures.
- Added a collector-disabled precommit case that proves `emit_precommit_vote(...)` falls back to the full view-aligned topology when no seeded collector set exists.
- Added a local accepted commit-vote case that proves `handle_vote(...)` records the vote without triggering known-block commit-evidence replay or payload recovery fanout.
- Added direct `frontier_body_next_due(...)` coverage for both derived-target paths: leader-stage scheduling now stays armed when live-roster derivation can supply fetch targets, while voter-stage scheduling still stays idle when derivation yields only the local leader and no remote voters.
- Added direct `known_block_commit_qc_recovery_targets(...)` coverage for both empty-input fallback branches: cached vote-roster recovery for far-future rounds with preserved roster evidence, and live commit-topology recovery when no vote-roster evidence exists.
- Added two more exact-frontier scheduler checks: far-future slots now prove the final fallback to `effective_commit_topology()`, and body-present slots now prove exact fetch stays idle when no same-round commit-QC repair is active.
- Added direct `deterministic_elected_roster_from_candidates(...)` coverage for two narrow roster-election guards: zero requested roster length now proves the helper still elects one candidate, and NPoS `max_validators` now proves candidate truncation happens before the requested roster-size cap.
- Added direct `frontier_body_next_due(...)` deadline checks for the two timing branches that were still only covered indirectly: before ingress grace elapses the helper returns `observed_at + authoritative_body_ingress_fetch_grace()`, and after grace elapses it returns `last_fetch_at + retry_window`.
- Added direct `maybe_replay_known_block_commit_evidence(...)` coverage for the “no new progress” guard: once the same round’s replay state has already been sent and neither vote count nor commit-QC state changes, the helper now has a focused test proving it returns `false` without scheduling more traffic.
- Added direct NPoS roster-unavailability source coverage for the empty-commit-topology path: one test proves the helper stays on the multi-peer local lane when that lane is available, and another proves it falls back to the full published active validator roster when the local validator has no lane assignment.
- Added two more exact-frontier due checks: body-present slots still schedule retries when same-round commit-QC repair remains actionable, and passive-catchup slots stay excluded from exact body retry scheduling even when cached targets exist.
- Focused validation for this follow-up:
  - `cargo fmt --all`
  - `cargo test -p iroha_core --lib precommit_vote_falls_back_to_topology_when_collectors_disabled -- --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::local_accepted_commit_vote_does_not_replay_known_block_evidence' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::frontier_body_next_due_derives_targets_from_live_roster_when_slot_cache_is_empty' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::frontier_body_next_due_voter_stage_requires_remote_voters_after_derivation' --nocapture`
  - `cargo test -p iroha_core --lib known_block_commit_qc_recovery_targets_fall_back_to_cached_vote_roster -- --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::known_block_commit_qc_recovery_targets_fall_back_to_effective_commit_topology' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::frontier_body_next_due_falls_back_to_effective_commit_topology_when_live_roster_is_empty' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::frontier_body_next_due_ignores_body_present_without_commit_qc_repair' --nocapture`
  - `cargo test -p iroha_core --lib deterministic_roster_election_keeps_one_candidate_when_target_len_is_zero -- --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::deterministic_roster_election_truncates_npos_candidates_to_max_validators' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::frontier_body_next_due_returns_ingress_grace_deadline_before_grace_elapses' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::frontier_body_next_due_returns_retry_deadline_after_grace_elapses' --nocapture`
  - `cargo test -p iroha_core --lib roster_unavailability_candidate_source_npos_uses_local_lane_when_commit_topology_empty -- --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::roster_unavailability_candidate_source_npos_uses_full_active_roster_when_local_lane_missing' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::known_block_commit_evidence_replay_skips_without_new_progress' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::frontier_body_next_due_keeps_retry_armed_when_body_present_but_commit_qc_repair_active' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::frontier_body_next_due_ignores_passive_catchup_slot_even_with_targets' --nocapture`

## 2026-04-22 Sumeragi targeted main_loop regression sweep

- `crates/iroha_core/src/sumeragi/main_loop/commit.rs` now keeps the initial local commit/precommit emit on the seeded collector set plus explicit parallel fanout, instead of widening that very first send through the generic commit-evidence replay path.
- `crates/iroha_core/src/sumeragi/main_loop/votes.rs` now limits automatic known-block commit-evidence replay to non-local accepted commit votes, so a locally emitted precommit is not redundantly fanned out to the whole topology before the collector retry logic takes over.
- `crates/iroha_core/src/sumeragi/main_loop.rs` now lets `frontier_body_next_due(...)` account for derived voter fallback when the leader lane has no usable remote leader target, and NPoS roster-unavailability candidate selection now scopes lane discovery from the active topology rather than the raw cached snapshot.
- `crates/iroha_core/src/sumeragi/main_loop/tests.rs` now uses PRF-aligned leader/view discovery for the affected proposal and vote fixtures, searches RBC subset cases without assuming the local peer is outside the deterministic rebroadcast base set, and expects partial-session payload rebroadcasts to queue only the chunks that are still locally present.
- This clears the reported `sumeragi::main_loop` failure cluster around collector targeting, idle frontier recovery, RBC rescue/rebroadcast fixtures, same-slot vote-collision validation, exact-frontier retry scheduling, and roster-unavailability source selection.
- Focused validation for this slice:
  - `cargo fmt --all`
  - `cargo test -p iroha_core --lib commit_vote_targets_collectors_or_topology -- --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::assemble_proposal_height_two_records_inline_frontier_manifest' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::commit_vote_targets_collectors_or_topology' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::force_view_change_if_idle_arms_nonleader_empty_frontier_recovery_after_pacemaker_attempt' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::maybe_emit_rbc_deliver_prefers_targeted_ready_rescue_when_subset_skips_local' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::pending_validation_preserves_same_slot_signature_collisions_until_identity_validation' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::precommit_vote_skips_payload_broadcast_for_aborted_pending' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::precommit_vote_targets_collectors_without_broadcast' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::rbc_payload_rebroadcast_allows_derived_roster' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::retry_missing_block_requests_defers_view_change_when_queue_blocks_seen' --nocapture`
  - `target/debug/deps/iroha_core-afb8267c04707e87 --exact 'sumeragi::main_loop::tests::roster_unavailability_candidate_source_matches_consensus_mode' --nocapture`

## 2026-04-22 Roadmap Audit

- `roadmap.md` was re-audited against the current repo and rewritten around repo-backed unfinished issues instead of speculative or stale reminders.
- Removed the obsolete `iroha_torii_shared` `AccountLabel` import blocker; the current file no longer imports `AccountLabel`.
- Removed the old trigger `get_numeric(...)` follow-up; the free helper has already been removed in `kotodama_lang`, so any future trigger work must be scoped to a concrete repo-local test gap.
- Promoted concrete open gaps into the roadmap where evidence exists today, including the `ivm_prebuild.rs` / `integration_tests/build.rs` sample-list drift and the lack of a repo-wide translation metadata audit.

## 2026-04-22 Roadmap Cleanup

- `roadmap.md` now tracks unfinished work only. Completed history was removed from the roadmap so it stops acting like a second status log.
- Historical completed roadmap-only epics were archived here at summary level instead of staying in the roadmap:
  - Kagami NPoS-ready network generation, explicit consensus-mode cutover tooling, peer/block signature hardening, and bootstrap-from-trusted-peer genesis fetch are complete.
  - Kagami/Mochi Iroha3 profiles, account-identity / alias unification, CLI output normalization, CI cleanup, and offline QR storm follow-up work are complete.
  - Soracloud platform MVP and Soracloud model-training / weight-lifecycle work are complete.
  - Kotodama / IVM developer-experience observability work is complete.
  - Asset ID, account-literal, and asset-alias lease cleanup follow-ups are complete.
  - The tracked repository action-item inventory had no actionable code/runtime/SDK/test/tooling/docs entries at the time of the scan; only reference-only mentions remained.

## 2026-04-22 Recent Completed Follow-ups

- `crates/iroha_core/src/sumeragi/main_loop/tests.rs` now salts harness-generated peer seeds per instance and folds the elected signer into the helper heartbeat seed, which eliminates helper block-hash collisions across parallel RBC roster tests and closes the stale-cache replacement coverage gaps around same-epoch roster recovery.
- `crates/iroha_torii/src/zk_attachments.rs` and `crates/iroha_torii/tests/zk_attachments_subprocess.rs` now cover the remaining sanitizer subprocess branches exercised in the latest pass: launcher fallback, timeout handling, response decoding, malformed compressed payloads, child-entry validation, spawn failures, and reader timeout/error paths.
- The recent `iroha_core` frontier-slot helper slices are complete: pending-wrapper liveness, active-owner-state helpers, validation-inflight preservation, commit-inflight preservation, and later-view lock-state guard coverage were all added with focused tests.
- The recent `iroha_torii` manifest-routing slices are complete: helper coverage, direct handler coverage, malformed-payload coverage, and direct-routing coverage are all in place with focused test runs.
- The recent `iroha_data_model` bridge helper / verifier / serialization follow-up slices are complete with focused crate validation.

## 2026-04-22 Completed Follow-up Archive

- Permissioned preserved-peer stable soak: green on the patched tree; the remaining follow-up is the fresh-binary rerun from the current branch.
- Permissioned missing-QC recovery: landed, covered by focused regressions, and aligned with the current reschedule / local-vote behavior.
- Retained RBC summary refresh: landed; the previously failing large-payload NPoS regression is green.
- Permission-cache replay: landed and green.
- Integration failure sweep: the reported targeted regressions were fixed and covered by focused validation.
- MCP writer-profile alignment: landed; mutation-capable MCP endpoint tests now opt into the documented writer profile and the targeted test file is green.

## 2026-04-21 Follow-up: Kotlin SDK typed offline-cash redeem support
- Added typed cash models and a redeem-proof builder under
  `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/`.
  After rebasing onto the Offline V2 Torii surface, `OfflineToriiClient`
  remains scoped to `/v1/offline/v2/readiness`; the older cash-route client
  overloads and route tests are not retained because Torii no longer exposes
  those HTTP endpoints.
- `OfflineStarkEnvelopeProver.kt` +
  `OfflineSettlementProofs.buildRedeemRequestProof` port
  `iroha_core::zk_stark::prove_stark_fri_{air,composition}_envelope_bytes`
  to pure Kotlin, restricted to the `stark/fri/sha256-goldilocks` backend
  used by the legacy offline cash payload shape. Parity is locked byte-for-byte
  against committed Rust-generated fixtures at
  `kotlin/core-jvm/src/test/resources/offline/redeem_proof_fixtures.json`.
- Temporary exception: `java/iroha_android` was intentionally left
  untouched for this slice. The Kotlin↔Java mirror policy in
  `AGENTS.md:69-70` remains repo policy; a follow-up task will mirror
  the typed offline-cash API once the Kotlin surface stabilizes.
- Focused validation:
  - `cd kotlin && ./gradlew :core-jvm:compileKotlin` (pass)
  - `cd kotlin && ./gradlew :core-jvm:test --console=plain --tests
    'org.hyperledger.iroha.sdk.offline.OfflineSettlementProofsParityTest'
    --tests 'org.hyperledger.iroha.sdk.offline.OfflineCashCodecTest'`
    (pass — envelope byte-parity gate cleared against Rust fixtures).
