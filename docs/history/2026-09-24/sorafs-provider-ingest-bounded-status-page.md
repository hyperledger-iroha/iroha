# SoraFS provider-ingest bounded status page

Scope: the `optimizations` checkout at `/Users/takemiyamakoto/devstuff/iroha`, with changes confined to `crates/sorafs_node/src/provider_ingest_outbox.rs` and its status-page tests. This record does not qualify provider-ingest production activation.

The old `ProviderIngestOutbox::statuses_page` converted every active and retained terminal entry to a public status, sorted all converted rows, and then truncated to the requested page. Conversion clones manifest IDs and boxed Musubi verification receipts. Production defaults permit 128 active entries and 4,096 terminal entries while a page is capped at 1,000 rows. A one-row read could therefore allocate and clone thousands of rows while holding the outbox state lock.

The implementation now scans only job identities and keeps the first `limit + 1` `(job_id, original ordinal)` pairs in a fallibly reserved max-heap. It constructs at most `limit` status rows after sorting that bounded selection. The ordinal retains the previous stable ordering when duplicate job IDs are present in a deliberately corrupt in-memory snapshot; persisted checkpoints already reject duplicate IDs. The existing page cursor, public API, checkpoint schema, and error variants remain unchanged.

Tests compare bounded pages with the prior full-sort semantics across mixed active and terminal rows, including a retained Musubi receipt, after reopening from the authoritative sealed checkpoint. They cover a one-row page, maximum configured page, and exact cursor boundaries. A separate adversarial test checks duplicate-ID stable ordering and cursor behavior in deliberately corrupt in-memory state.

Validation on this source checkout:

- `CARGO_BUILD_JOBS=1 cargo test --offline --locked -p sorafs_node --lib bounded_status_page -- --nocapture`: 2 passed, 0 failed (1,546 filtered out).
- The compiled `sorafs_node` test binary passed the adjacent `status_inventory_is_bounded_and_paginated`, `private_musubi_receipt_checkpoint_roundtrip_preserves_public_projection`, `sealed_checkpoint_restart_uses_external_authority_and_exact_cache`, and `sealed_checkpoint_restart_repairs_only_an_exact_immediate_predecessor_cache` selectors: 1 passed each.
- `rustfmt --edition 2024 --check` on the touched Rust files and `git diff --check` on this slice passed.

This read-path fix does not establish a finalized provider-grant authority, authenticated grant issuance or revocation, multi-provider deployment, load and recovery qualification, or SoraFS promotion evidence. The production ingest runtime still needs a qualified finalized-ledger source and external sealed checkpoint provider.
