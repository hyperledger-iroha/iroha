# SoraFS provider-ingest sealed restart and owner replay

Scope: the `optimizations` checkout at `/Users/takemiyamakoto/devstuff/iroha`. This is an outbox-only crash/retry/idempotence test using its existing sealed checkpoint runtime and typed finalized-authorization fixtures. It does not supply a production finalized-ledger producer.

The new `sealed_enqueue_response_loss_restarts_with_one_source_and_completion_owner` test injects a checkpoint compare-and-swap that commits an accepted provider-ingest job and then loses its response. The first caller receives an ambiguous error. Restart loads the authoritative sealed successor, repairs the exact predecessor cache, retains one pending job, and treats replay of the same authorization as `ExistingActive` without advancing the sealed head.

The test then restarts with a live source lease. A second worker cannot take it early; after expiry, that worker takes the next generation and the stale first claim cannot mark local storage. The new owner records local storage, and restart does not expose a second source claim. One signed completion crosses the durable exposure boundary; restart retains its exact hash and ambiguous state, rejects a fresh signer claim, and accepts only exact pending/finalized reconciliation. Repeated submitted and finalized observations, plus replay of the original authorization after terminal restart, leave the sealed head unchanged. The terminal row remains unique and active count becomes zero.

The authorization, signed transaction, and finalized completion evidence are test fixtures passed through the real outbox API. This does not authenticate an external ledger cursor, prove a production provider grant or revocation source, or qualify multi-provider/four-validator execution, operator custody, or promotion. Those gates remain open.

Validation: `CARGO_BUILD_JOBS=1 cargo test --offline --locked -p sorafs_node --lib sealed_enqueue_response_loss_restarts_with_one_source_and_completion_owner -- --nocapture` passed 1/1 in the coordinated build slot. The injected checkpoint-worker panic is intentional and caught as an ambiguous commit. Targeted Rust formatting and `git diff --check` passed.
