# SDK operation inventory

[`specs/sdk_operation_inventory.tsv`](../specs/sdk_operation_inventory.tsv)
records the complete explicit Torii route catalog, sorted by route identity.
It includes HTTP method/path, audience, authentication, principal admission,
effect, transport, feature expression, projection flags, and private-cache policy.
The source authority is `iroha_torii_shared::route_catalog::CATALOGED_ROUTES`;
Torii's router builder checks each mounted descriptor against this catalog.
Implicit CORS preflight behavior is excluded from the operation inventory.

Run `python3 scripts/sdk_operation_inventory.py` for a read-only drift check,
or `python3 scripts/sdk_operation_inventory.py --write` after reviewing route
changes. The generator compiles the actual `std`-only Rust descriptors with the
repository toolchain. It needs no Cargo dependencies or node binaries. The
generated-file registry records its owner and complete source inputs.

The first-release client migration must account for every row and every
existing SDK call. The `sdk` column describes current generated projection
membership, not which handwritten SDK operations may be removed. For example,
status, configuration reads, and block streams are currently outside that
projection while the Rust SDK implements them. Preserve these capabilities
on their appropriate public, account, or operator contexts. The feature
expression describes availability and does not authorize enabling an
unqualified service.

TODO: Complete the canonical capability/request/response mapping and consumer
migration against this inventory, then check that each supported operation has
one authority-appropriate API and that no stale public wrappers remain. This
inventory alone does not establish SDK coverage or qualify node features.


## Subscription capability coverage

All eleven `application.subscriptions*` route identities have one asynchronous
Rust SDK operation. The flat synchronous methods have been removed.

| Context | Capability operations | Routes |
| --- | --- | --- |
| `Client::subscriptions()` | `list_plans`, `list`, `get` | GET plans, subscriptions, and subscription by ID |
| `AccountClient::subscriptions()` | `prepare_plan`, `prepare` | POST plans and subscriptions |
| `AccountClient::subscriptions()` | `prepare_pause`, `prepare_resume`, `prepare_cancel`, `prepare_keep`, `prepare_charge`, `prepare_usage` | POST subscription action and usage routes |

Wire records are owned by `iroha_torii_shared::subscriptions`. SDK preparation
results expose one typed `SubscriptionDraft`, including context identity,
operation metadata and decoded `TransactionPayload` or `InstructionBox` values.
The payload variant owns `Box<TransactionPayload>` and preserves the exact tagged
JSON representation; all eleven operation futures are `Send`. Resource/state
validation and exact instruction checks run before returning the draft.
The explicit blocking capabilities run these same async operations on an owned
runtime. The CLI consumes those facades and prints unsigned preparation results.
The subscription contract suites cover every route, exact signed requests,
count-mode forwarding, authority/resource binding and asynchronous dispatch.
Qualification evidence is recorded separately; this mapping is implementation
coverage, not a claim that the complete SDK migration is qualified.

## Status capability coverage

| Canonical route | Async capability | Blocking capability |
| --- | --- | --- |
| `diagnostic.status` — `GET /status` | `client.status().get().await` | `client.status().get()` |
| `core.api_version` — `GET /v1/api/version` | `client.status().version().await` | `client.status().version()` |

These public reads use the client-owned transport and one configured deadline.
Status preserves negotiated JSON/Norito support with an 8-MiB response ceiling;
version uses an exact text representation with a 16-KiB ceiling. Both return the
canonical SDK error family. The old flat getters and raw status-request wrapper
are removed. Custom HTTP behavior belongs to the injected transport. Startup and
integration status polling await this capability directly; synchronous callers
use the reusable blocking runtime. Diagnostic reads do not trigger a compatibility
probe or automatically retry a response under another representation.

## Operator configuration coverage

| Canonical route | Async capability | Blocking capability |
| --- | --- | --- |
| `operator.configuration.read` — `GET /v1/configuration` | `operator.configuration().get().await` | `operator.configuration().get()` |

This capability exists only on the explicit `OperatorClient`. It signs the exact
network and request target with that context's operator key, excludes account and
token authentication, and accepts one bounded JSON DTO. The response ceiling is
8 MiB and dispatch uses the context deadline without compatibility probing or
automatic retries. Gas-schedule users read the DTO's `confidential_gas` field;
the flat getter and duplicate projection operation are removed.

The blocking operator can own a reusable runtime directly from its asynchronous
context or share an existing blocking client's runtime. It does not require an
account authority. Both CLI consumers and all six integration reads use this
capability; operator readback across restart retains the existing context.


## Event and block stream coverage

| Canonical route | Async account capability | Blocking account capability |
| --- | --- | --- |
| `events.stream_websocket` — `GET /v1/events/ws` | `account.events().subscribe(filters).await` | `account.events().subscribe(filters)` |
| `blocks.stream_websocket` — `GET /v1/blocks/stream` | `account.blocks().subscribe(height).await` | `account.blocks().subscribe(height)` |

The SDK exposes these operations only on `AccountClient`; filters are nonempty
and replay height is `NonZeroU64`. Both upgrades carry a fresh canonical account
signature for the exact network and target. The server filters events by current
dataspace visibility and requires `CanReadAllLedgerData` for full signed blocks.
The catalog's optional account authentication on the event route also describes
an anonymous server view; the existing SDK stream capability uses account
authority. Public and operator contexts do not expose these subscriptions.

HTTP and WebSocket connectors are separate, client-owned boundaries configured
with `ClientBuilder::http_transport` and `ClientBuilder::stream_transport`.
`StreamTransport` returns the exact upgrade response and an owned `StreamSocket`
binary channel. The SDK verifies the unique `iroha-norito-v1` subprotocol before
sending one canonical subscription. Custom connectors must dispatch once without
redirects/retries, handle ping/pong, reject non-binary data, enforce message bounds
and release I/O on cancellation. `blocking::Client::with_http_transport` is the
explicit blocking HTTP constructor; stream-only callers construct a blocking
account facade from their validated asynchronous account context.

Initial subscription messages have a 256-KiB ceiling, incoming complete messages a
64-MiB ceiling, and upgrade response bodies a 64-KiB ceiling. Upgrade and initial
send share the context request deadline. Receiving is long-lived; async callers
supply their own receive deadline, while blocking `recv(Some(duration))` bounds
one wait without closing or resubscribing. `recv(None)` waits without that idle
deadline. A local wait timeout remains distinct from `Ok(None)` normal completion.
Explicit close is fallible and uses the context request timeout, with a five-second
fallback when that timeout is disabled. Abnormal close status/reason, including
revocation and backpressure, remains structured; decode/protocol/transport failures
terminate the stream. Neither transport nor capability automatically reconnects.

The flat listen methods and public flow constructors have been removed. All 27
existing external SDK calls across the CLI and 17 integration files use the
canonical capabilities; the four CLI loops use the reusable blocking account
runtime. Runtime and release qualification are recorded separately.

Mochi uses these same signed SDK capabilities. `Supervisor::stream_reader(alias)`
validates the selected generation and binds its exact genesis account, network,
address discriminant and peer endpoint. Kagami's default genesis grants this
account `CanReadAllLedgerData`; vault/composer and operator identities do not
substitute for it. Readiness and lifecycle workflows require the explicit reader
and reject endpoint/network mismatches before dispatch. Mochi owns summary
fanout, a retained first receiver and bounded reconnect backoff above the SDK.
Raw WebSocket operations and the intermediate raw-frame broadcast are removed.

The SDK reports actual received binary lengths through `last_message_bytes()`;
Mochi preserves those lengths for UI events and decode errors, and represents
unknown lengths as absent. HTTP `Retry-After` delta hints and abnormal close
codes remain machine-readable. Only an HTTP 429 upgrade permits readiness's
existing HTTP status fallback. Local mock/fixture, source-built Kagami and real
four-validator qualification are distinct checks.
