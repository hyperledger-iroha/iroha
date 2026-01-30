---
lang: ur
direction: rtl
source: docs/source/content_hosting.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4c0c7f98dbd9f49c573302f0b5cbe2e7a663d7fe35a1a9eea8da4f24c6f9bc8b
source_last_modified: "2026-01-05T17:57:58.226177+00:00"
translation_last_reviewed: 2026-01-30
---

% Content Hosting Lane
% Iroha Core

# Content Hosting Lane

The content lane stores small static bundles (tar archives) on-chain and serves
individual files directly from Torii.

- **Publish**: submit `PublishContentBundle` with a tar archive, optional expiry
  height, and an optional manifest. The bundle ID is the blake2b hash of the
  tarball. Tar entries must be regular files; names are normalised UTF-8 paths.
  Size/path/file-count caps come from `content` config (`max_bundle_bytes`,
  `max_files`, `max_path_len`, `max_retention_blocks`, `chunk_size_bytes`).
  Manifests include the Norito-index hash, dataspace/lane, cache policy
  (`max_age_seconds`, `immutable`), auth mode (`public` / `role:<role>` /
  `sponsor:<uaid>`), retention policy placeholder, and MIME overrides.
- **Deduping**: tar payloads are chunked (default 64 KiB) and stored once per
  hash with reference counts; retiring a bundle decrements and prunes chunks.
- **Serve**: Torii exposes `GET /v1/content/{bundle}/{path}`. Responses stream
  directly from the chunk store with `ETag` = file hash, `Accept-Ranges: bytes`,
  Range support, and Cache-Control derived from the manifest. Reads honour the
  manifest auth mode: role-gated and sponsor-gated responses require canonical
  request headers (`X-Iroha-Account`, `X-Iroha-Signature`) for the signed
  account; missing/expired bundles return 404.
- **CLI**: `iroha content publish --bundle <path.tar>` (or `--root <dir>`) now
  auto-generates a manifest, emits optional `--manifest-out/--bundle-out`, and
  accepts `--auth`, `--cache-max-age-secs`, `--dataspace`, `--lane`, `--immutable`,
  and `--expires-at-height` overrides. `iroha content pack --root <dir>` builds
  a deterministic tarball + manifest without submitting anything.
- **Config**: cache/auth knobs live under `content.*` in `iroha_config`
  (`default_cache_max_age_secs`, `max_cache_max_age_secs`, `immutable_bundles`,
  `default_auth_mode`) and are enforced at publish time.
- **SLO + limits**: `content.max_requests_per_second` / `request_burst` and
  `content.max_egress_bytes_per_second` / `egress_burst_bytes` cap read-side
  throughput; Torii enforces both before serving bytes and exports
  `torii_content_requests_total`, `torii_content_request_duration_seconds`, and
  `torii_content_response_bytes_total` metrics with outcome labels. Latency
  targets live under `content.target_p50_latency_ms` /
  `content.target_p99_latency_ms` / `content.target_availability_bps`.
- **Abuse controls**: Rate buckets are keyed by UAID/API token/remote IP, and an
  optional PoW guard (`content.pow_difficulty_bits`, `content.pow_header`) can
  be required before reads. DA stripe layout defaults come from
  `content.stripe_layout` and are echoed in receipts/manifest hashes.
- **Receipts & DA evidence**: successful responses attach
  `sora-content-receipt` (base64 Norito-framed `ContentDaReceipt` bytes) carrying
  `bundle_id`, `path`, `file_hash`, `served_bytes`, the served byte range,
  `chunk_root` / `stripe_layout`, optional PDP commitment, and a timestamp so
  clients can pin what was fetched without re-reading the body.

Key references:

- Data model: `crates/iroha_data_model/src/content.rs`
- Execution: `crates/iroha_core/src/smartcontracts/isi/content.rs`
- Torii handler: `crates/iroha_torii/src/content.rs`
- CLI helper: `crates/iroha_cli/src/content.rs`
