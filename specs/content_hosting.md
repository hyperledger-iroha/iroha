% Content Hosting Lane
% Iroha Core

# Content Hosting Lane

The content lane stores small static bundles (tar archives) on-chain and serves
individual files directly from Torii.

- **Publish**: submit `PublishContentBundle` with a tar archive, optional expiry
  height, and an optional manifest. The bundle ID is the blake2b hash of the
  tarball. Tar entries must be regular files; names are normalised UTF-8 paths.
  Size/path/file-count caps, the retention window, the chunk size, the cache
  defaults and the default stripe layout are protocol constants of this
  release until a governed change set activates the committed parameter
  `content_policy_v1` (`iroha_core::runtime::committed_policy`); a root that
  recognizes it applies the value its predecessor block committed. Who may publish is the executor's admission decision: the Initial
  executor admits `PublishContentBundle` in genesis, and after genesis only
  through the governed `CanPublishContentBundle` permission (see
  **Publishers**); an installed executor enforces its own permission. The
  `[content]` node configuration holds gateway resources only.
  Manifests include the Norito-index hash, dataspace/lane, cache policy
  (`max_age_seconds`, `immutable`), auth mode (`public` / `role:<role>` /
  `sponsor:<uaid>`), retention policy placeholder, and MIME overrides.
- **Publishers**: under the Initial executor, publishing after genesis is a
  release addition after the frozen baseline. The release publishes one
  Permission definition, `CanPublishContentBundle`
  (`iroha_core::runtime::content_publish_permission`), and nothing else; a
  governed change set drafted under `iroha.permission.content_publish.v1`
  activates it. A root that has not activated it admits `PublishContentBundle`
  and `RetireContentBundle` in genesis only, exactly as the baseline does. On a
  root that has activated it:
  - the permission is a direct account grant of the exact unit token, the
    deleted publisher allow-list one entry per account; no role carries it,
    and Core refuses to put it in a role for every executor;
  - an account holding it may publish while no restriction of any kind names
    the definition;
  - only a `CanSetParameters` holder grants or revokes it; a holder cannot
    delegate it and may renounce its own grant;
  - any restriction (`live_runtime_upgrades.md` §7.1) stops publishing and new
    grants, while revocation, renunciation and retirement continue;
  - the creator of a bundle may retire it after genesis;
  - revoking a direct grant an account holds and retiring a bundle published
    after genesis stay executable. Under closed admission or completion-only
    on either instruction they run as completion work; a change set that would
    leave `iroha.revoke` unbound or disabled lapses while a direct grant
    remains, and one that would do so to `RetireContentBundle` lapses while a
    bundle published after genesis remains. Grants and bundles hold no custody
    and pin no completion closure, so rebinding either instruction, changing
    the kernel or a profile, and replacing the executor do not wait for them.
    The executor still checks the issuer, holder or bundle creator's authority;
  - a change set that activates the definition lapses while any account or
    role holds a permission of that name;
  - a change set that retires the definition lapses while any account or role
    holds it, or while any bundle published after genesis remains, expired
    ones included: retiring returns `RetireContentBundle` to genesis-only, and
    nothing prunes expired bundles. Governance keeps the definition restricted
    until both clear.
- **Deduping**: tar payloads are chunked (default 64 KiB) and stored once per
  hash with reference counts; retiring a bundle decrements and prunes chunks.
- **Serve**: Torii exposes `GET /v1/content/{bundle}/{path}`. Responses stream
  directly from the chunk store with `ETag` = file hash, `Accept-Ranges: bytes`,
  Range support, and Cache-Control derived from the manifest. Public bundles
  use the configured `max_age_seconds` and optional `immutable` directive.
  Role- and sponsor-gated responses always use `private, no-store`; manifest
  cache settings cannot make protected bytes reusable by a shared cache or
  outlive a current-state authorization decision. They also vary on the full
  canonical authentication header set, including `X-Iroha-Witness`.
  Protected reads require `X-Iroha-Account` plus either
  `X-Iroha-Signature`/`X-Iroha-Timestamp-Ms`/`X-Iroha-Nonce` or a canonical
  `X-Iroha-Witness`; Torii authenticates protected bundles before resolving a
  file path, rejects stale timestamps and replayed nonces, and does not expose
  a protected path-existence oracle. Missing/expired bundles return 404.
- **CLI**: `iroha content publish --bundle <path.tar>` (or `--root <dir>`) now
  auto-generates a manifest, emits optional `--manifest-out/--bundle-out`, and
  accepts `--auth`, `--cache-max-age-secs`, `--dataspace`, `--lane`, `--immutable`,
  and `--expires-at-height` overrides. `iroha content pack --root <dir>` builds
  a deterministic tarball + manifest without submitting anything. Bundle-file
  reads are capped before allocation. Directory packing streams traversal,
  admits the configured file count and final padded tar size before retaining
  each source, and builds into one exact-capacity archive buffer; symlinks and
  other non-regular inputs fail closed.
- **Cache and auth defaults**: the default and maximum cache lifetime and the
  immutable-bundle default are enforced at publish time, from the same source as
  the size caps. A
  publish that supplies no manifest gets the `public` auth mode; a publisher
  that wants gated reads states the mode in its manifest.
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
- Publishing permission: `crates/iroha_core/src/runtime/content_publish_permission.rs`
  (definition, recognition, draft) and
  `crates/iroha_core/src/executor_initial_permission_authority.rs` (Initial
  admission, grants and revocations)
- Torii handler: `crates/iroha_torii/src/content.rs`
- CLI helper: `crates/iroha_cli/src/content.rs`
