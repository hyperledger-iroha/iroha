# Native exchange acceptance inputs

`native_exchange.rs` contains two **ignored, artifact-dependent** host acceptance tests.
Neither has passed until its actual source/receipt dependencies have qualified. The ordinary
host-custody test covers only persistence of the explicit software hardware fixture.

The export test first authenticates the complete signed producer catalog and independent
native genesis through `open_pinned_engineering_wallet_sources`. Its existing ten source
pins and path inputs are mandatory. `KAGEMUSHA_NATIVE_SOURCE_SHA256` records the independently
captured test candidate. All new paths must be absolute, inside this checkout's
`target/qualification`; output directories must not exist.

Both tests also require `KAGEMUSHA_NATIVE_LEDGER_SETUP` (the executed setup's
`setup.json`) and `KAGEMUSHA_NATIVE_LEDGER_SETUP_SHA256`. The fixed nine originals
come from `iroha_core::sumeragi::test_chain::kagemusha_setup`: the separately
registered accounts A/B/C (engineering signer seeds 41/42/43), reserve, actual
registered asset incarnation, original signed genesis, genesis manifest,
`registration-proof.norito` (the fixed `[H1, H2]` finality-proof array), and
`capture.json`. Every original's length and hash are checked. The capture must
match the independently pinned finality source fixture; the signed genesis,
network, instance and initial epoch must match the admitted native owner. The
ordered proofs are verified from that owner. The independently pinned executed
setup producer owns the registration and supply assertions; finality alone is
not an asset-row membership proof. There is no fallback to the old component
asset incarnation or old genesis fixture.

1. Run `export_actual_a_load_target_with_retained_native_custody` with
   `KAGEMUSHA_NATIVE_LOAD_TARGET_OUTPUT`. It enrolls A using the explicit simulated hardware,
   admits the actual account signature, proves/releases Bootstrap, signs Activate, and folds.
   The real host custody tree and private software-key fixture remain in `wallet-a`.
2. Give the finality producer only `public-target/target.json` and its seven SHA-256/length
   bound originals. Pin the manifest independently. `issue-load.norito` is the existing
   `KagemushaWalletLedgerV1::IssueLoad`: A's actual scheme/wallet/asset, ordinal 0, request
   `[201;32]`, net amount 100, no online charge. The exact account, credential, certificate
   set, asset, Bootstrap and Activate originals accompany it. The manifest also binds all
   ten source pins, executed-setup manifest pin, native chain/instance/initial epoch and
   candidate binary/source hashes.
   This is **target DATA**, with no receipt, proof or settlement verdict. A missing valid
   Activate or foreign genesis must be refused by the producer.
3. The actual producer must execute/finalize that exact target, then export the canonical
   `KagemushaWalletLoadReceiptV1` (maximum 512 bytes) and
   `KagemushaWalletLoadFinalityV1` (maximum 16,384 bytes). The historical ordinary firstLoad
   fixture targets a different wallet/asset and is not a substitute.
4. Run `actual_native_a_to_b_to_c_then_unload_with_restart_and_replay` with a fresh
   `KAGEMUSHA_NATIVE_EXCHANGE_OUTPUT`, the existing `KAGEMUSHA_NATIVE_LOAD_TARGET` manifest
   path and `_SHA256`, and exact `KAGEMUSHA_NATIVE_LOAD_RECEIPT` / `_SHA256` and
   `KAGEMUSHA_NATIVE_LOAD_FINALITY` / `_SHA256`. It requalifies sources and reopens the SAME
persisted A custody and original key. It never reenrolls A or reconstructs its Bootstrap.
   B and C enroll against their own registered accounts and native account challenges;
   the final Unload claim must pay C's account, distinct from A and B.

The consumer tests actual Load verification, duplicate refusal, pending irreversible Send
recovery, lost delivery/exact-byte replay, invalid incoming proof refusal, local Receive
fold checkpoints/restart, onward Send only after folding, unavailable storage, A→B→C
conservation, Archive, and an actual verified Unload package. It exports the exact canonical
Unload claim for separate node execution. Its result explicitly does **not** claim ledger
settlement, four-validator network qualification, physical attestation or phone gates.

The software-key file is test-only private custody, not a portable wallet format or runtime
API. Never distribute it with the public target. Consumer execution changes A's selected
state; retrying a failed acceptance run must reconcile its retained state, never overwrite
that tree or create another A to reuse the same finalized deposit.
