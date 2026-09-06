# Historical project evidence

This page is historical evidence, not current release readiness.
See [the archive index](../../index.md) for provenance and reconstruction.


<a id="record-6ae57e65e9ee5a56a5ced930a1037963b5f2f5d2e5f1b651b88af9228c40bf67"></a>

<!-- Original context: Roadmap / Android native device integration -->
## Android native device integration



<a id="record-9028967ff4273b97e8cd5cb3e0b343df1ed3b0d583d559dbab6895a8cf1d89ac"></a>

- Complete same-source arm64-v8a/x86_64 native builds and actual device JNI
  execution. Debug device builds can now explicitly include the sealed native
  bridge with `irohaDebugNativeBridge=true`; JVM-only checks cannot establish
  native linkage or hardware Offline qualification.


<a id="record-448fefd8d40fb31c24eb23b053236c05ee11a1c751717ef22c56706ecd05fe3f"></a>

- Bind the canonical device dispatcher for operations 1--22 to a service that
  owns authenticated Core release/proof state, trusted time, receiver keys and
  the full non-forking journal/outbox contract. Provision the authorized Android
  secure-element applet/OEM service and Apple secure-element credential, rebuild
  the exact ABI-23 mobile artifacts, and pass physical power-loss, airplane-mode,
  rollback, restore, rollover and byte-identical recovery qualification before
  enabling Offline monetary actions. The current strict frame/receiver codecs
  and Java Card journal tests are prerequisites, not device qualification.


<a id="record-faf986fcbf751b9928d5eedb29d05315ad9a717c2fd529f7c4a98f18e3fa2cb4"></a>

- Connect the now-validated Swift/Kotlin bootstrap and recovery ordering to the
  qualified native service, then rebuild mobile artifacts from the settled
  source. Caller-persisted operation identities and canonical reservation bindings
  now have focused SDK coverage. Finish authenticated lookup after a lost native
  return, Core archive integration with the native coordinator, and
  revision-consistent outbox discovery before complete customer flows can ship.


<a id="record-32e4e0fbfe128f60f97959aef360d1833d0fbe0c4b5947493fe3910e6a50310b"></a>

- Qualify the requested iPhone, Samsung, Huawei, Google and Meizu device families
  by exact model/OS/firmware/provider profile. Establish an authorized service
  implementing the complete monetary contract for each; stock key signing is
  insufficient. Huawei HarmonyOS requires explicit runtime/native integration
  qualification independent of Android. Track evidence and remaining acceptance
  gates in [the readiness record](../../../../../specs/kagemusha_v1_production_readiness.md).



<a id="record-e3a0e597b4858cd5aa181a81409737b434651057f0aef890fb59cf5c07ed0b08"></a>

<!-- Original context: Roadmap / Python SDK first-release follow-up -->
## Python SDK first-release follow-up



<a id="record-0409f70ff6f963782e2b2759c12bb14ec6e8573228af66b6647f0085cc937d3d"></a>

- Rebuild `iroha_python_rs` from the settled same-revision candidate, then run
  the complete high-level Python suite, native cryptography/Connect vectors,
  clean wheel build/install, typing-consumer smoke test, and four-validator
  integration corridor. Do not qualify native behavior from the current
  checkout while unrelated Core overlay/SCCP compilation is broken.


<a id="record-93ff82e233c007d076b0fc1700cb3a9fb24a4fb5893abfab5cb5d336df34e4d9"></a>

- Split the remaining monolithic Torii route families into reviewed internal
  modules, collapse any remaining raw/typed method pairs into one typed public
  method, and converge the standalone low-level transport package with the
  high-level client so session ownership, bounds, authentication, and retry
  rules have one implementation. Generate a public export/signature inventory
  and reject aliases or response-shape unions instead of preserving them.


<a id="record-ba7fa875999c2b888eae1a83b770d6796b040c5fbe6b9dc9ebc9222318a25df3"></a>

- Benchmark cold import, transaction/key construction, table-driven CRC64, and
  maximum-size NDJSON/SSE/Norito responses. Set release thresholds for duplicate
  native derivations, peak response memory, reconnect behavior, and clean-wheel
  import size, then keep those thresholds in the Python release corridor.



<a id="record-fef41c7bb66bf1a137803de51fe3e57eed7c52d475b1bb167be91740cb88f0e5"></a>

<!-- Original context: Roadmap / JavaScript SDK first-release follow-up -->
## JavaScript SDK first-release follow-up



<a id="record-15b6a3c048c962996e6f5a925cfaf069edc585c90fc9c103e607a6b64b768a64"></a>

- Continue decomposing the 35,000-line Torii implementation into route-family
  modules with exact constructor and per-method option allowlists. Keep the
  reviewed eager/deferred/combined budgets hard, benchmark cold import and
  maximum-response RSS, and remove remaining camel/snake, TTL, asset-definition,
  and transaction option aliases instead of preserving compatibility paths.


<a id="record-030e05321985bbb7600a7bf6bf40aefb6bc9992457706e9dabc66d3f831f5070"></a>

- Keep the registry artifact on its single `dist/` source-of-truth tree, move
  the long protocol inventory out of the SDK README into source-coupled
  references, and split the serial unit profile into portable and
  native-qualified lanes. On a same-revision host bridge, run the native
  SM2/privacy/Norito parity suites and four-validator integration corridor
  before release qualification.



<a id="record-525e941458b1e44230dde287827746a97d071a82576ef0909abe3b9b63b0f35a"></a>

<!-- Original context: Roadmap / C# SDK release qualification -->
## C# SDK release qualification



<a id="record-6b6183ef3e1b6e8cfce7f6f62a91a8bc4980c9ac1fe8389645c3dfd78a5da590"></a>

- Finish the first-release immutable-result pass: stop repeated array and
  `JsonNode` cloning in hot DTO getters, replace redundant public signed-envelope
  array constructors with one canonical parse boundary, and benchmark repeated
  collection access plus large bounded responses before setting allocation
  thresholds.


<a id="record-2e1e1c8654afd00fcb29c09a507443d962523e6cfb9f298133de4579109da763"></a>

- Establish a reviewed public-API approval baseline after the remaining surface
  cut, remove the broad `CS1591` suppression, and document every retained public
  member. Mirror the C# SDK's typed, canonical-only `CurveId` construction API
  across other SDKs instead of retaining string aliases or compatibility modes.



<a id="record-8049c296a2fd3b674cb134ee275914259e4caf8c6804c44499ae26d25df72212"></a>

<!-- Original context: Roadmap / Security-audit release qualification -->
- Run relevant four-validator restart and authorization scenarios for contract
  ownership/holds, scoped Torii history and streams, mixed-route visibility,
  transaction admission, and any ledger-backed ISO settlement transition.
  Preserve mandatory signed RS16 DA/RBC and capture exact candidate, topology,
  configuration, and results.


<a id="record-7b211e9dfb51fc5426743e9f63079573688cc165bcd440c9ed10da4962642dbc"></a>

<!-- Original context: Roadmap / SDK first-release closure -->
## SDK first-release closure



<a id="record-db067f3d9ac68133e524f2418bf888932f6a5a2e25b0123b8b21602f98ac345c"></a>

- Generate one canonical signing-algorithm token registry for the node and every SDK, then reject
  convenience spellings that currently differ by language. Keep the canonical wire identifiers and
  hardware-independent outputs unchanged.


<a id="record-be6309f11902eb3e6b4782498f561a42d41799b4378cff0c34e18d27b12a5083"></a>

- Finish the remaining Java Android platform-reflection audit by moving telemetry, HTTP transport,
  and keystore integrations behind typed Android source-set implementations with explicit JVM
  injection or unsupported results. Norito compression and generic adapter dispatch are now direct;
  raw subscription/compression spellings and unused peer-profile limit parameters are removed.


<a id="record-fead1b71e99be568c6dab35c31b1a96c98b16b5fdd90dfde0ebd7a1f2d1a63b4"></a>

- Rebase the stale Android direct-`javac` lint lane on the complete Gradle-owned SDK dependency
  classpath, then clear its existing source warnings and missing test dependencies. The lane now
  resolves mandatory Norito compression correctly, but does not yet qualify the full Java SDK.


<a id="record-1677bfd946657bf4f9b7b61ee7493cbc451cdbfb597c1ef13021174aa3b5001a"></a>

- Reduce Python transaction signing to one typed credential input per operation and remove any
  remaining alternate keyword shapes. Keep exact canonical request validation before transport.


<a id="record-2541d2774de2b4285390a74a1c8c883e0fa526e8bde1286cfdb9ff25484dfb34"></a>

<!-- Original context: Roadmap / Build-efficiency closeout -->
- Preserve the green split-aware JavaScript bundle gate without hiding deferred
  code. The exact pinned-esbuild result is Torii eager 998,331/1,006,592 bytes,
  Sumeragi lazy 71,905/72,704 bytes, and combined 1,070,236/1,079,296 bytes;
  public-browser eager 480,214/480,256 bytes, Sumeragi lazy 72,243/72,704 bytes,
  deployment lazy 9,177/9,216 bytes, and combined 561,634/562,176 bytes. The
  transaction codec is 300,611/304,128 bytes, and Nexus, canonical request, IVM
  artifact, and Kotodama browser targets also pass. Keep the exact dynamic-edge
  inventory and eager/lazy closure accounting intact.


<a id="record-a9f25c74f1125dd1d65b62e110042673534277c1c751c44369e109cfcb59e35a"></a>

<!-- Original context: Roadmap / Native Torii MCP release qualification -->
## Native Torii MCP release qualification



<a id="record-03bdf9233df811ddc43a01644fb982b4efeef794c7a18f6d9a309f2079615453"></a>

- Keep Torii as the only MCP server, process, and listener. `/v1/mcp` must
  remain an in-process Torii route; do not introduce an MCP gateway, sidecar,
  proxy server, second listener, or separate deployment unit.


<a id="record-c7d9a55bba71a2c8df21c448b9d67273216f9918a354b744822ec4d43f192518"></a>

- Qualify curated `resources/list` and `resources/read` through the existing
  Torii router. Release-qualify the implemented pure in-process transaction
  prepare/inspect corridor and its external-signing-to-signed-submit workflow
  without accepting or persisting signing keys in MCP. Add generic simulation
  only after Core exposes a reviewed bounded scratch-execution API that proves
  no state, queue, event, or persistence side effects; do not relabel structural
  inspection as simulation. Add explain projections from the same canonical
  artifact and simulation truth rather than independent heuristics.


<a id="record-097f97b17877d60211e6419fca2e9e592406c03bde8681850859ca490257bd4a"></a>

- Regenerate the OpenAPI provenance manifests from the final source seal and
  close the inherited catalog/generated-artifact expectations exposed by the
  broader MCP profile. Run focused Torii/shared, Mochi, CLI, Python,
  plugin-contract, authentication, response-bound, and four-validator
  validation. Retain the exact nested-target account/operator authentication,
  retired async-job, closed VPN, active/unknown content coercion, inert-media,
  VPN auth-before-parse, and protected content auth/cache/path regressions in
  the final lane.



<a id="record-18b4a169012a0c7e0571b1980adf497bf560126ed40109d8ed2d6e854f715356"></a>

<!-- Original context: Roadmap / Disposable Taira-compatible development networks -->
- `up` must build the fixed `local-release` toolchain for the exact native
  Linux/AArch64 Rust target from the `optimizations` worktree. Observe the HEAD,
  tracked diff, and non-ignored untracked entries before and after the run, while
  reporting explicitly that Cargo source consumption is not proven. Bind the
  executable hashes, four validator build identities, CLI identity, and target
  triple through the completed run; reject target/network overlap and prebuilt
  or mutable outputs. Keep immutable source provenance in the separate release
  corridor.


<a id="record-cad5e77c5167f14c4cbb855d2b1a0ef30e3ccb33eaee64fb6d2e69135c1a89c6"></a>

<!-- Original context: Roadmap / Wallet activity query follow-ups -->
## Wallet activity query follow-ups



<a id="record-20d156c10a70d23cb8e4ac9e034972e8eed70c2d2a80bd273662368bbc0fe88f"></a>

- TODO: Add one authenticated, snapshot-stable account activity feed that
  combines value movements and participant-scoped contract events behind an
  opaque continuation cursor. The cursor must bind the ledger snapshot, make
  the requested page limit apply to the combined result, and return an explicit
  stale/expired-cursor error instead of allowing offset drift to skip rows.


<a id="record-128863ee04054b5a63a93e98ec958f5cf73466fc4fbf0dcf7c14bf3277543c19"></a>

- TODO: Add an authenticated indexed aggregate for outgoing asset value over a
  caller-supplied timestamp range. The result must bind its ledger snapshot and
  support retail-wallet monthly policy checks without downloading transaction
  pages or exposing unrelated account activity.



<a id="record-b01167cc69d7784fa4a836c4e433ed288d10ca2fdeefd80b7f0ac549752919dd"></a>

<!-- Original context: Roadmap / SORA Nexus and Taira -->
- Autoscale block application now revalidates effective runtime ratios before
  sample evaluation, so non-finite, zero, or collapsed threshold values cannot
  be converted into permissive permille triggers even if already-applied actual
  state is corrupted after config parsing. Programmatic `set_nexus`, lifecycle
  preparation, and block autoscale application now also revalidate runtime
  lane bounds against the compiled `max_lane_id_exclusive` safety cap, so a corrupted
  actual config cannot expand past the parser-enforced production limit. Live
  default-route routing now also has regression coverage for empty or inverted
  `min_lane_id >= max_lane_id_exclusive` runtime bounds, proving corrupted actual state cannot
  shard no-target traffic onto autoscale elastic lanes. Config parsing and
  runtime validation both reject empty enabled elastic ranges, so an enabled
  autoscaler cannot carry zero creatable lane ids into production. Live default-route
  routing now also fails closed to the base lane when the active elastic range
  is occupied by a manual lane, malformed autoscale-managed lane, or managed
  lane outside the default dataspace, with the same behavior pinned through the
  `nexus_and_streaming` multilane router integration harness. Live autoscale
  routing now additionally requires `nexus.enabled = true`, so a corrupted
  actual state with Nexus disabled but autoscale still marked enabled cannot
  admit elastic lanes into default traffic; transaction validation coverage now
  proves the same gate keeps disabled-Nexus traffic on base-lane policy instead
  of bypassing it through an elastic route, and proposal-refresh coverage proves
  stale elastic vectors are recomputed back to the default lane before consensus
  proposal execution. Multilane router integration coverage now also pins the
  public `ConfigLaneRouter::route_with_view` boundary so stale autoscale-managed
  catalog lanes are ignored when either autoscale or Nexus is disabled, while
  enabled autoscale still shards default traffic over valid elastic lanes. Block
  validation coverage now also proves stale elastic execution contexts are
  rejected after Nexus is disabled or after active elastic-range corruption
  forces base-lane routing, so forged or delayed blocks cannot keep using an
  elastic route once live state falls back to the base lane. Block autoscale
  application coverage now also proves corrupted
  disabled-Nexus state cannot create or retire elastic lanes even when autoscale
  remains marked enabled, and enabled-Nexus state cannot create or retire
  elastic lanes after autoscale is disabled. Future `last_transition_height`
  corruption now also has block-application coverage proving it suppresses
  scale-out and scale-in without overwriting the cooldown marker.
  Conflicting-window coverage now pins scale-out precedence when a longer hot
  scale-out window and a shorter cold scale-in window are both eligible, so
  capacity is added rather than retiring an existing managed lane in the same
  block. Longer-window gap coverage now proves missing middle Kura blocks
  suppress both hot scale-out and cold scale-in candidates without mutating the
  lane catalog or transition marker.
  Autoscale threshold parsing and block-time runtime checks now reject
  sub-permille ratios too, preventing tiny positive thresholds from rounding to
  zero and turning hot scale-out into an effectively unconditional transition;
  they also require the rounded permille thresholds to preserve strict
  scale-in/scale-out hysteresis, so tiny raw gaps cannot collapse at the
  integer precision used by block application.


<a id="record-ba0e27638c4031302acea941b726ba87861153353e1608c084b4309200089b3b"></a>

- Autoscale latency ratios and utilization now use widened deterministic
  integer intermediates and saturate only the final permille value, so extreme
  timestamps or committed-fragment counters cannot wrap or deflate an
  overloaded sample into a cold one.


<a id="record-2e3ad7335ae7cfd085bacf69638bca58aef09339aa8e3acac6740aece69cd4f7"></a>

- Continue Native AMX release evidence beyond the implemented attestation data
  model, durable participant-application evidence, deterministic per-leg vote
  cache, proposer-side prepare/commit gating, queue-journal restart replay, and
  routing-plan projection. Required work is longer-running soak, fault
  injection, validator rotation, pruning, and carrier-application evidence;
  participant controls remain control-only and never create independent WSV
  finality.

