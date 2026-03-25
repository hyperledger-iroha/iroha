# Soracloud CLI and Control Plane

Soracloud v1 is an authoritative, IVM-only runtime.

- `iroha app soracloud init` is the only offline command. It scaffolds
  `container_manifest.json`, `service_manifest.json`, and optional template
  artifacts for Soracloud services.
- All other Soracloud CLI commands are network-backed only and require
  `--torii-url`.
- The CLI does not maintain any local Soracloud control-plane mirror or state
  file.
- Torii serves public Soracloud status and mutation routes directly from
  authoritative world state plus the embedded Soracloud runtime manager.

## Runtime Scope

- Soracloud v1 accepts only `SoraContainerRuntimeV1::Ivm`.
- `NativeProcess` remains rejected.
- Ordered mailbox execution runs admitted IVM handlers directly.
- Hydration and materialization come from committed SoraFS/DA content rather
  than synthetic local snapshots.
- `SoraContainerManifestV1` now carries `required_config_names` and
  `required_secret_names`. Deploy, upgrade, and rollback fail closed when the
  effective authoritative material set would not satisfy those declared
  bindings.
- Committed service config entries are now materialized under
  `services/<service>/<version>/configs/<config_name>` as canonical JSON
  payload files.
- Soracloud IVM handlers can now read those authoritative config payloads
  directly through the runtime host `ReadConfig` surface, so ordinary
  `query`/`update` handlers do not need to guess node-local file paths just to
  consume committed service config.
- Committed service secret envelopes are now materialized under
  `services/<service>/<version>/secret_envelopes/<secret_name>` as
  authoritative envelope files.
- Ordinary Soracloud IVM handlers can now read those committed secret
  envelopes directly through the runtime host `ReadSecretEnvelope` surface.
- The legacy private-runtime fallback tree is now synchronized from committed
  deployment state under `secrets/<service>/<version>/<secret_name>` so the
  older raw secret read path and the authoritative control plane point at the
  same bytes.
- Private-runtime `ReadSecret` now resolves authoritative deployment
  `service_secrets` first and only falls back to the legacy node-local
  `secrets/<service>/<version>/...` materialized file tree when no committed
  service secret entry exists for the requested key.
- Secret ingestion is still intentionally narrower than config ingestion:
  `ReadSecretEnvelope` is the public-safe ordinary-handler contract, while
  `ReadSecret` remains private-runtime-only and still returns the committed
  envelope ciphertext bytes rather than a plaintext mount contract.
- Runtime service plans now expose the corresponding ingestion capability
  booleans directly, so status consumers can tell whether a materialized
  revision supports host config reads, host secret-envelope reads, and private
  raw secret reads without inferring it from handler classes alone.

## CLI Commands

- `iroha app soracloud init`
  - offline scaffold only.
  - supports `baseline`, `site`, `webapp`, and `pii-app` templates.
- `iroha app soracloud deploy`
  - validates `SoraDeploymentBundleV1` admission rules locally, signs the
    request, and calls `POST /v1/soracloud/deploy`.
  - `--initial-configs <path>` and `--initial-secrets <path>` may now attach
    authoritative inline service config / secret maps atomically with the
    first deploy so required bindings can be satisfied on first admission.
  - the CLI now signs the HTTP request canonically with
    `X-Iroha-Account`, `X-Iroha-Signature`, `X-Iroha-Timestamp-Ms`, and
    `X-Iroha-Nonce`, receives a deterministic draft transaction instruction set
    from Torii, then submits the real transaction itself through the normal
    Iroha client lane.
  - Torii also enforces SCR-host admission caps and fail-closed capability
    checks before the mutation is accepted.
- `iroha app soracloud upgrade`
  - validates and signs a new bundle revision, then calls
    `POST /v1/soracloud/upgrade`.
  - the same `--initial-configs <path>` / `--initial-secrets <path>` flow is
    available for atomic material updates during upgrade.
  - The same SCR-host admission checks run server-side before the upgrade is
    admitted.
- `iroha app soracloud status`
  - queries authoritative service status from `GET /v1/soracloud/status`.
- `iroha app soracloud config-*`
  - `config-set`, `config-delete`, and `config-status` are Torii-backed only.
  - the CLI signs canonical service-config provenance payloads and calls
    `POST /v1/soracloud/service/config/set`,
    `POST /v1/soracloud/service/config/delete`, and
    `GET /v1/soracloud/service/config/status`.
  - config entries are persisted in authoritative deployment state and remain
    attached across deploy/upgrade/rollback revision changes.
  - `config-delete` now fails closed when the active revision still declares
    the named config in `container.required_config_names`.
- `iroha app soracloud secret-*`
  - `secret-set`, `secret-delete`, and `secret-status` are Torii-backed only.
  - the CLI signs canonical service-secret provenance payloads and calls
    `POST /v1/soracloud/service/secret/set`,
    `POST /v1/soracloud/service/secret/delete`, and
    `GET /v1/soracloud/service/secret/status`.
  - secret entries are persisted as authoritative `SecretEnvelopeV1` records
    in deployment state and survive normal service revision changes.
  - `secret-delete` now fails closed when the active revision still declares
    the named secret in `container.required_secret_names`.
- `iroha app soracloud rollback`
  - signs rollback metadata and calls `POST /v1/soracloud/rollback`.
- `iroha app soracloud rollout`
  - signs rollout metadata and calls `POST /v1/soracloud/rollout`.
- `iroha app soracloud agent-*`
  - all apartment lifecycle, wallet, mailbox, and autonomy commands are
    Torii-backed only.
- `iroha app soracloud training-*`
  - all training job commands are Torii-backed only.
- `iroha app soracloud model-*`
  - all model artifact and weight commands are Torii-backed only.
  - the next uploaded-model/private-runtime slice should extend this family
    with `upload-init`, `upload-chunk`, `upload-finalize`, `compile`,
    `allow-model`, `run-private`, `run-status`, and `decrypt-output`
    operations rather than creating a separate control-plane namespace.
  - see `uploaded_private_models.md` for the design that layers those routes
    onto the existing model registry and artifact/weight records.
- `model-host` control-plane routes
  - Torii now exposes authoritative
    `POST /v1/soracloud/model-host/advertise`,
    `POST /v1/soracloud/model-host/heartbeat`,
    `POST /v1/soracloud/model-host/withdraw`, and
    `GET /v1/soracloud/model-host/status`.
  - these routes persist opt-in validator host capability adverts in
    authoritative world state and let operators inspect which validators are
    currently advertising model-host capacity.
  - `iroha app soracloud model-host-advertise`,
    `model-host-heartbeat`, `model-host-withdraw`, and
    `model-host-status` now sign the same canonical provenance payloads as the
    raw API and call the matching Torii routes directly.
- `iroha app soracloud hf-*`
  - `hf-deploy`, `hf-status`, `hf-lease-leave`, and `hf-lease-renew` are
    Torii-backed only.
  - `hf-deploy` and `hf-lease-renew` now also auto-admit the deterministic
    generated HF inference service for the requested `service_name`, and
    auto-admit the deterministic generated HF apartment for
    `apartment_name` when one is requested, before the shared-lease mutation
    is submitted.
  - reuse is fail-closed: if the named service/apartment already exists but is
    not the expected generated HF deployment for that canonical source, the HF
    mutation is rejected instead of silently binding the lease to unrelated
    Soracloud objects.
  - when the embedded runtime manager is attached, `hf-status` now also
    returns a runtime projection for the canonical source, including bound
    services/apartments, queued next-window visibility, and local bundle/
    artifact cache misses; `importer_pending` follows that runtime projection
    instead of relying only on the authoritative source enum.
  - when `hf-deploy` or `hf-lease-renew` admits the generated HF service in
    the same transaction as the shared-lease mutation, the authoritative HF
    source now flips to `Ready` immediately and `importer_pending` stays
    `false` in the response.
  - HF lease status and mutation responses now also expose any authoritative
    placement snapshot already attached to the active lease window, including
    assigned hosts, eligible-host count, warm-host count, and separate
    storage-vs-compute fee fields.
  - `hf-deploy` and `hf-lease-renew` now derive the canonical HF resource
    profile from the resolved Hugging Face repo metadata before they submit the
    mutation:
    - Torii inspects the repo `siblings`, prefers `.gguf` over
      `.safetensors` over PyTorch weight layouts, HEADs the selected files to
      derive `required_model_bytes`, and maps that to a first-release
      backend/format plus RAM/disk floors;
    - lease admission fails closed when no live validator host advert can
      satisfy that profile; and
    - when a host set is available, the active window now records a
      deterministic stake-weighted placement and a separate compute reservation
      fee alongside the existing storage lease accounting.
  - later members joining an active HF window now pay prorated storage and
    compute shares for only the remaining window, while earlier members
    receive the same deterministic storage refund and compute-refund accounting
    from that late join.
  - the embedded runtime manager can synthesize the generated HF stub bundle
    locally, so those generated services can materialize without waiting for a
    committed SoraFS payload just for the placeholder inference bundle.
  - the embedded runtime manager now also imports allowlisted Hugging Face repo
    files into `soracloud_runtime.state_dir/hf_sources/<source_id>/files/` and
    persists a local `import_manifest.json` with the resolved commit, imported
    files, skipped files, and any importer error.
  - generated HF `metadata` local reads now return that local import manifest,
    including the imported file inventory plus whether local execution and
    bridge fallback are enabled for the node.
  - generated HF `infer` local reads now prefer on-node execution against the
    imported shared bytes:
    - `irohad` materializes an embedded Python adapter script under the local
      Soracloud runtime state directory and invokes it through
      `soracloud_runtime.hf.local_runner_program`;
    - the embedded runner first checks for a deterministic fixture stanza in
      `config.json` (used by tests), then otherwise loads the imported source
      directory through `transformers.pipeline(..., local_files_only=True)` so
      the model executes against the shared local import instead of pulling
      fresh Hub bytes; and
    - if `soracloud_runtime.hf.allow_inference_bridge_fallback = true` and
      `soracloud_runtime.hf.inference_token` is configured, the runtime falls
      back to the configured HF Inference base URL only when local execution
      is unavailable or fails and the caller explicitly opts in with
      `x-soracloud-hf-allow-bridge-fallback: 1`, `true`, or `yes`.
  - the runtime projection now keeps an HF source in `PendingImport` until a
    successful local import manifest exists, and importer failures surface as
    runtime `Failed` plus `last_error` instead of silently reporting `Ready`.
  - generated HF apartments now consume approved autonomy runs through the
    node-local runtime path:
    - `agent-autonomy-run` now follows a two-step flow: the first signed
      mutation records the authoritative approval and returns a deterministic
      draft transaction, then a second signed finalize request asks the
      embedded runtime manager to execute that approved run against the bound
      generated HF `/infer` service and returns any authoritative follow-up
      instructions as another deterministic draft;
    - the approved run record now also persists a canonical
      `request_commitment`, so the later generated service receipt can be
      bound back to the exact authoritative autonomy approval;
    - approvals can now persist an optional canonical `workflow_input_json`
      body; when present, the embedded runtime forwards that exact JSON payload
      to the generated HF `/infer` handler, and when absent it falls back to
      the older `run_label`-as-`inputs` envelope with the authoritative
      `artifact_hash` / `provenance_hash` / `budget_units` / `run_id` carried
      as structured parameters;
    - `workflow_input_json` can now also opt into deterministic sequential
      multi-step execution with
      `{ "workflow_version": 1, "steps": [...] }`, where each step runs a
      generated HF `/infer` request and later steps can reference prior outputs
      via `${run.*}`, `${previous.text|json|result_commitment}`, and
      `${steps.<step_id>.text|json|result_commitment}` placeholders; and
    - both the mutation response and `agent-autonomy-status` now surface the
      node-local execution summary when available, including success/failure,
      the bound service revision, deterministic result commitments, checkpoint
      / journal artifact hashes, the generated service `AuditReceipt`, and the
      parsed JSON response body.
    - when that generated service receipt is present, Torii records it into
      authoritative `soracloud_runtime_receipts` and exposes the resulting
      authoritative runtime receipt on recent run status alongside the
      node-local execution summary.
    - the generated-HF autonomy path now also records a dedicated authoritative
      apartment `AutonomyRunExecuted` audit event, and recent-run status
      returns that execution audit alongside the authoritative runtime receipt.
  - `hf-lease-renew` now has two modes:
    - if the current window is expired or drained, it immediately opens a fresh
      window;
    - if the current window is still active, it queues the caller as the
      next-window sponsor, charges the full next-window storage and compute
      reservation fees up front, persists the deterministic next-window
      placement plan, and exposes that queued sponsorship through `hf-status`
      until a later mutation rolls the pool forward.
  - generated HF public `/infer` ingress now resolves the authoritative
    placement and, when the receiving node is not the warm primary, proxies the
    request over Soracloud P2P control messages to the assigned primary host;
    the embedded runtime still fails closed on direct replica/unassigned local
    execution and generated HF runtime receipts carry `placement_id`,
    validator, and peer attribution from the authoritative placement record.
  - when that proxy-to-primary path times out, closes before a response, or
    comes back with a non-client runtime failure from the authoritative
    primary, the ingress node now reports `AssignedHeartbeatMiss` for that
    primary and enqueues `ReconcileSoracloudModelHosts` through the same
    internal mutation lane.
  - authoritative expired-host reconciliation now records persisted
    model-host violation evidence, reuses the public-lane validator slash path,
    and applies the default HF shared-lease penalty policy:
    `warmup_no_show_slash_bps=500`,
    `assigned_heartbeat_miss_slash_bps=250`,
    `assigned_heartbeat_miss_strike_threshold=3`, and
    `advert_contradiction_slash_bps=1000`.
  - locally assigned HF runtime health now also feeds that same evidence path:
    reconcile-time import/warmup failures on a local `Warming` host emit
    `WarmupNoShow`, and resident-worker failures on the local warm primary
    emit throttled `AssignedHeartbeatMiss` reports through the normal
    transaction queue.
  - reconcile now also pre-starts and probes resident HF workers for locally
    assigned warm/warming hosts, including replicas, so a replica can fail
    closed into the authoritative `AssignedHeartbeatMiss` path before any
    public `/infer` request ever lands on the primary.
  - when that local probe succeeds, the runtime now also emits one
    authoritative `model-host-heartbeat` mutation for the local validator when
    the assigned host is still `Warming` or the active host advert needs a TTL
    refresh, so successful local readiness promotes the same authoritative
    placement/advert state that manual heartbeats would update.
  - when the runtime emits a local `WarmupNoShow` or
    `AssignedHeartbeatMiss`, it now also enqueues
    `ReconcileSoracloudModelHosts` through the same internal mutation lane so
    authoritative failover/backfill starts immediately instead of waiting for
    a later periodic host-expiry sweep.
  - when public generated-HF ingress fails even earlier because the committed
    placement has no warm primary to proxy to, Torii now asks the runtime
    handle to enqueue that same authoritative
    `ReconcileSoracloudModelHosts` instruction immediately instead of waiting
    for a later expiry or worker-failure signal.
  - when public generated-HF ingress does receive a proxied success response,
    Torii now verifies the included runtime receipt still proves execution by
    the committed warm primary for the active placement; missing or mismatched
    placement attribution now fails closed and hints that same authoritative
    `ReconcileSoracloudModelHosts` path instead of returning a
    non-authoritative response. Torii also now rejects proxied success
    responses when the runtime receipt commitments or certification policy do
    not match the response it is about to return, and that same bad-receipt
    path also feeds the remote-primary `AssignedHeartbeatMiss` reporting
    hook.
  - proxied generated-HF execution failures now request that same
    authoritative `ReconcileSoracloudModelHosts` path after reporting the
    remote primary health fault, rather than waiting for a later expiry
    sweep.
  - Torii now binds each pending generated-HF proxy request to the
    authoritative primary peer it targeted. A proxy response from the wrong
    peer is now ignored instead of poisoning that pending request, so only
    the authoritative primary can complete or fail the request. A proxy
    response from the expected peer with an unsupported proxy response schema
    version still fails closed instead of being accepted only because its
    `request_id` matched a pending request. If the wrong peer that answered
    is itself still an assigned generated-HF host for that placement, the
    runtime now reports that host through the existing
    `WarmupNoShow` / `AssignedHeartbeatMiss` evidence path based on its
    authoritative assignment status and also hints authoritative
    `ReconcileSoracloudModelHosts`, so stale primary/replica authority drift
    feeds the control loop instead of only being ignored at ingress.
  - incoming Soracloud proxy execution is also now restricted to the intended
    generated-HF `infer` query case on the committed warm primary. Non-HF
    public local-read routes, and generated-HF requests delivered to a node
    that is no longer the authoritative warm primary, now fail closed instead
    of executing over the P2P proxy path. The authoritative primary also now
    recomputes the canonical generated-HF request commitment before execution,
    so forged or mismatched proxy envelopes fail closed.
  - when an assigned replica or stale former primary rejects that incoming
    generated-HF proxy execution because it is no longer the authoritative
    warm primary, the receiver-side runtime now also hints
    `ReconcileSoracloudModelHosts` instead of relying only on the caller-side
    routing view.
  - when that same incoming generated-HF proxy authority failure happens on
    the local authoritative primary itself, the runtime now treats it as a
    first-class host-health signal: warm primaries self-report
    `AssignedHeartbeatMiss`, warming primaries self-report `WarmupNoShow`,
    and both paths immediately reuse the same authoritative
    `ReconcileSoracloudModelHosts` control loop.
  - when that same non-primary receiver is still one of the authoritative
    assigned hosts and can resolve the warm primary from committed chain state,
    it now re-proxies the generated-HF request onward to that primary instead
    of failing immediately. Unassigned validators fail closed instead of acting
    as generic intermediary HF proxy hops, and the original ingress node still
    validates the returned runtime receipt against authoritative placement
    state. If that assigned-replica onward hop to the primary fails after the
    request is actually dispatched, the receiver-side runtime reports the
    remote primary health fault and hints authoritative
    `ReconcileSoracloudModelHosts`; if the local assigned replica cannot even
    attempt that onward hop because its own proxy transport/runtime is missing,
    the failure is now treated as a local assigned-host fault instead of
    blaming the primary.
  - reconcile now also emits `AdvertContradiction` automatically when the
    local validator's configured runtime peer id disagrees with the
    authoritative `model-host-advertise` peer id for that validator.
  - valid model-host re-advertise mutations now also synchronize authoritative
    assigned-host `peer_id` / `host_class` metadata and recompute current
    placement reservation fees when the host class changes.
  - contradictory model-host re-advertise mutations now emit immediate
    `AdvertContradiction` evidence, apply the existing validator slash/evict
    path, and refresh affected placements instead of just failing validation.
  - remaining HF hosting work is now:
    - broader cross-node/runtime-cluster health signals beyond the local
      validator's direct worker/warmup observations plus assigned-host
      receiver-side authority failures when remote-peer internal health should
      also feed the authoritative rebalance/slash path.
  - generated HF local execution now keeps a resident per-source Python worker
    alive under `irohad`, reuses the loaded model across repeated `/infer`
    calls, and restarts that worker deterministically if the local import
    manifest changes or the process exits.
  - these routes are not the private uploaded-model path. HF shared leases stay
    focused on shared source/import membership rather than encrypted on-chain
    private model bytes.
  - generated HF autonomy approvals now support deterministic sequential
    multi-step request envelopes, but broader non-linear/tool-using
    orchestration and artifact-graph execution still remain follow-up work
    beyond chained `/infer` steps.

## Status Semantics

`/v1/soracloud/status` and the related agent/training/model status endpoints now
reflect authoritative runtime state:

- admitted service revisions from committed world state;
- runtime hydration/materialization state from the embedded runtime manager;
- real mailbox execution receipts and failure state;
- published journal/checkpoint artifacts;
- cache and runtime health instead of placeholder status shims.

If authoritative runtime material is stale or unavailable, reads fail closed
instead of falling back to local state mirrors.

`/v1/soracloud/status` is the only documented Soracloud status endpoint in v1.
There is no separate `/v1/soracloud/registry` route.

## Removed Local Scaffolding

These older local-simulation concepts no longer exist in v1:

- CLI-local registry/state files or registry-path options
- Torii-local file-backed control-plane mirrors

## Example

```bash
iroha app soracloud deploy \
  --container container_manifest.json \
  --service service_manifest.json \
  --torii-url http://127.0.0.1:8080 \
  --api-token <token-if-required> \
  --timeout-secs 10
```

## Notes

- Local validation still runs before requests are signed and submitted.
- Standard Soracloud mutation endpoints no longer accept raw `authority` /
  `private_key` JSON fields for deploy, upgrade, rollback, rollout, agent
  lifecycle, training, model-host, and model-weight paths; Torii authenticates
  those requests from the canonical HTTP signature headers instead.
- `hf-deploy` and `hf-lease-renew` now include client-signed auxiliary
  provenance for the deterministic generated HF service/apartment artifacts,
  so Torii no longer needs caller private keys to admit those follow-up
  objects.
- `agent-autonomy-run` and `model/run-private` now use a draft-then-finalize
  flow: the first signed mutation records the authoritative approval/start,
  and a second signed finalize request executes the runtime path and returns
  any authoritative follow-up instructions as deterministic draft
  transactions.
- `model/decrypt-output` now returns the authoritative private-inference
  checkpoint as a deterministic draft transaction, signed only by the outer
  transaction rather than by an embedded Torii-held private key.
- ZK attachment CRUD now keys tenancy off the signed Iroha account and still
  treats API tokens as an extra access gate when enabled.
- Public Soracloud local-read ingress now applies explicit per-IP rate and
  concurrency limits and re-checks public route visibility before local or
  proxied execution.
- Private-runtime capability enforcement happens inside the Soracloud host ABI,
  not inside CLI or Torii-local scaffolding.
- `ram_lfe` remains a separate hidden-function subsystem. User-uploaded private
  transformer execution should reuse Soracloud FHE/decryption governance and
  model registries, not the `ram_lfe` request path.
- Runtime health, hydration, and execution are sourced from
  `[soracloud_runtime]` configuration and committed state, not environment
  toggles.
