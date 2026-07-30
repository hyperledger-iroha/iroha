# Cursor pagination

This document defines the first-release lifecycle of stored cursors returned by
the signed `POST /v1/query` protocol. A REST collection that uses a different
cursor type must document its own lifecycle and must not claim these guarantees
implicitly.

## Cursor representation

`ForwardCursor` contains:

- `query`: an opaque, collision-checked 256-bit identifier generated directly
  from the operating system random source and encoded as 64 lowercase
  hexadecimal characters;
- `cursor`: the server's expected position for the next batch; and
- `gas_budget`: the optional stored-cursor budget copied from the request.

Clients must preserve the complete value and must not parse, synthesize, order,
or derive identifiers. The identifier is a handle for server-side state. It is
not a self-contained or signed cursor and contains no portable snapshot.

## Storage and routing

Stored cursor state lives only in the `LiveQueryStore` of the Torii process that
served the `Start` request. It is not persisted, replicated, or shared among
Torii instances.

Consequently:

- a Torii restart invalidates every stored cursor;
- a request routed to another Torii instance is reported as expired;
- a load-balanced deployment must use instance affinity for the lifetime of a
  stored cursor; and
- stored mode does not promise failover. Clients that require retry across
  instances must use ephemeral requests until a shared cursor store is provided.

No cursor-signing key or key-rotation policy applies to this server-side design.
The identifier is unpredictable, and allocation retries rather than overwriting
an existing entry if a generated identifier collides.

`live_query_store.capacity` is a hard process-wide bound and
`live_query_store.capacity_per_user` is a bound for one authority. Torii
atomically reserves a global slot before it reserves an authority slot or
allocates an identifier, so concurrent starts on different map shards cannot
overshoot the configured total. A failed allocation releases both
reservations, and terminal, explicitly removed, or pruned cursors release their
slots only after the stored entry has been removed. A start rejected by the
global limit returns `CapacityLimit`; one rejected by the authority limit
returns `AuthorityQuotaExceeded`.

## Authorization

Every `Continue` request is a new signed query request. Torii rechecks the
request signature and the current ingress controls, including API access and
rate or gas limits. The live-query store then requires the signed authority to
equal the authority that created the cursor before it reads or advances cursor
state. Each stored cursor also retains the canonical original `Start` request.
Before returning another page, Torii decodes that request and runs it through
the current executor authorization policy and current world state. Permission
revocation therefore takes effect on the next page request.

A foreign-authority attempt, failed current authorization, malformed retained
request, or revalidation failure does not advance the iterator or refresh the
idle timer. Foreign, absent, unbound, and malformed cursor state remains
indistinguishable as `Expired`; an ordinary permission revocation is reported
through the executor's normal authorization error without revealing query
items.

## Resource admission

Signed-query admission has two independent identities. Before signature
verification, Torii charges the exact validated API credential when token
authentication is required; otherwise it charges the effective client origin
supplied by the socket-ingress/trusted-proxy boundary. Arbitrary `x-api-token`
text therefore never selects a bucket when token authentication is disabled,
and invalid or ambiguously repeated configured credentials fail before rate
accounting. After signature verification, each physical query execution is
charged to the canonical signed `AccountId`.

The two dimensions use `torii.query_rate_per_authority_per_sec` and
`torii.query_burst_per_authority`, but keep separate bounded bucket maps so
authority churn cannot evict and refill a pre-auth caller bucket. That first
dimension prevents a caller from creating fresh budgets by generating keys or
rotating unauthenticated header values, while the authority dimension restores
the documented per-authority ceiling across direct, proxied, and
multi-dataspace execution. A trusted CIDR may bypass the public pre-auth gate,
but it does not bypass authenticated authority or physical-work admission.

`torii.query_max_inflight` bounds all physical query workers, and iterable
starts also consume `torii.query_heavy_max_inflight`. The owned permits live in
the blocking worker rather than the HTTP future, so disconnecting or cancelling
a request cannot release capacity while executor validation or snapshot work is
still running. Saturated workers wait only for
`torii.query_queue_timeout_ms`; after that Torii returns `CapacityLimit`.

## Lifetime and reuse

`torii.query_idle_time_ms` controls the idle eviction threshold. The first-release
default is 10,000 milliseconds. A successful continuation refreshes the last
access time. Foreign, unknown, malformed-position, and failed continuation
attempts do not refresh it.

The pruning task runs once per configured idle interval. A cursor becomes
eligible for eviction after the idle threshold, but it can remain usable until
the next pruning pass. Clients must therefore treat the configured value as an
idle threshold, not as a precise lease expiry timestamp or a guaranteed lease.

The position inside a returned cursor is single-use after a successful
continuation. Replaying that old position after the server advanced returns a
cursor mismatch; continuation is not an idempotent operation. A failed attempt
that did not advance state may be retried. The server removes the entry
immediately after the terminal batch, and later reuse is reported as expired.

## Errors and information disclosure

The following cases intentionally return `QueryExecutionFail::Expired`, mapped
to HTTP `410 Gone` by Torii:

- an unknown or opaque-identifier-tampered cursor;
- an idle-evicted cursor;
- a cursor lost after restart or routed to another instance; and
- a cursor presented by an authority other than its owner.

These cases are indistinguishable so the identifier namespace is not an
existence oracle. When the caller owns a still-live identifier but presents an
incorrect or already-consumed position, the server returns
`QueryExecutionFail::CursorMismatch`, mapped to HTTP `400 Bad Request`. A final
cursor position returns `CursorDone` only while the entry still exists; terminal
entries are normally removed and subsequently return `Expired`.

## Counts and page navigation

The signed query protocol calls this control `count_mode`, not `include_total`:

- `count_mode=bounded` is the default. `remaining_items` is absent,
  `has_more` is authoritative, and the server avoids a full count where
  possible.
- `count_mode=exact` requests exact `remaining_items`. It can require full
  materialization or additional work and can therefore cost more.

There is no generic `include_total` parameter or `total_items` field on
`QueryOutput`. A caller can derive the original exact total only by combining
the number of items already returned with `remaining_items` from exact mode.
Collection-specific REST endpoints that expose `include_total` must state
whether totals are supported and must omit `total_items` when they are not; they
must not return a fabricated zero.

Cursor pagination is forward-only. It supports sequential iteration and cached
navigation among pages the client already visited. It does not support random
access to an unseen page or make cursor pagination equivalent to offset
pagination.

## Snapshot behavior

Every stored query retains continuation data derived from the initial query
view. Later writes cannot appear in subsequent pages and later deletions cannot
remove rows that were already captured for that cursor. The cursor is therefore
bound to one server-local logical snapshot as well as to its original query,
filters, ordering, and authority.

This snapshot is not portable or cryptographically self-contained: it remains
server-side state and is lost on restart, expiry, or routing to another Torii
instance as described above. Current authorization is intentionally not frozen;
revocation is re-evaluated before every page and can deny access to the retained
snapshot.
