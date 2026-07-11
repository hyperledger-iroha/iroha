# Torii first-release API contract

Torii publishes one application API under `/v1`. Because this is the first
release, clients do not negotiate minor protocol versions and the server does
not carry compatibility routes or DTOs for earlier pre-release designs.

The former `x-iroha-api-version` request header and the API-version response
headers are not part of the contract. Torii ignores unknown request headers in
the usual HTTP manner. `/v1/api/versions` is unmounted, and proof endpoints are
available according to their authentication and rate-limit policies without a
separate minimum-version gate.

`GET /v1/api/version` remains a node-information endpoint. It reports the
active block-header/genesis version; it does not select an HTTP API variant.

## Representation negotiation

Representation negotiation is independent of API evolution. Typed endpoints
support `application/json` and `application/x-norito`; JSON suffix media types
such as `application/problem+json` also select the JSON mapping. Media-type
parameters do not change the selected representation, so
`application/json; charset=utf-8` is JSON.

Torii evaluates each supported representation against the most-specific
matching `Accept` range: an exact type takes precedence over `application/*`,
which takes precedence over `*/*`. That range's `q` value is the effective
quality, including `q=0`; a wildcard cannot re-enable a representation that an
exact range forbids. Torii then compares effective quality, followed by
specificity. An equal explicit preference selects Norito. A wildcard-only tie
uses the endpoint default; typed Torii endpoints default to Norito. Missing
`Accept` also uses the endpoint default. Invalid or out-of-range quality values,
or a request that permits neither representation, return `406 Not Acceptable`.
Because no representation was successfully negotiated, the `406` itself uses
the shared typed error envelope in deterministic `application/json` form.
Negotiated responses include `Vary: Accept`.

The following examples are normative for typed endpoints whose default is
Norito:

| `Accept` | Result |
| --- | --- |
| omitted | `application/x-norito` |
| `application/json, application/x-norito` | `application/x-norito` (equal explicit preference) |
| `application/json;q=0.8, */*;q=0.9` | `application/x-norito` (JSON's exact range fixes its quality at 0.8) |
| `application/x-norito;q=0, */*;q=1` | `application/json` (the exact zero forbids Norito) |
| `application/*;q=0.7` | `application/x-norito` (wildcard-only tie uses the endpoint default) |
| `image/png` | `406` with a JSON `ErrorEnvelope` whose code is `response_not_acceptable` |

Typed request bodies require `Content-Type`. JSON and Norito use the media types
above; unsupported or missing types return `415 Unsupported Media Type`, while
an empty, malformed, or schema-invalid body under a supported type returns
`400 Bad Request`. Request decoding never sniffs bytes and never falls back to
a whole-payload base64 wrapper. Negotiation failures, typed extractor
rejections, and handler errors use the shared typed error envelope whenever a
response representation can be selected. Router-native path/method misses and
protocol streams are documented exceptions rather than typed application
responses.

## Typed JSON mapping

The JSON representation is the Norito JSON mapping of the same public DTO, not
an independently evolving compatibility schema. Struct members use their
declared snake-case names. Adjacently tagged enums contain the declared tag and
content members; a unit variant has a `null` content value. Fixed byte arrays
and ordinary byte vectors are JSON arrays of integers from 0 through 255 unless
the field explicitly selects a textual helper. Hashes, keys, signatures,
identifiers, and numeric values use the mapping declared by their data-model
type.

Unsigned 64-bit and 128-bit values are emitted as unquoted, lossless decimal
JSON integers. Clients must use an integer representation that preserves their
full range and must not round them through an IEEE-754 `double`. Optional fields
encode as either their typed value or `null` unless the DTO explicitly declares
that `None` is omitted. Decoders reject duplicate declared members, unknown
enum discriminator values, out-of-range integers, and non-finite
numbers. They ignore unknown object members so an independently produced JSON
object can carry unrelated metadata without changing the typed value. For a
unit enum variant, decoders accept either an explicit `null` content member or
an omitted content member; canonical Torii output always includes the explicit
`null`. Signatures and payload digests cover canonical typed Norito bytes; the
lexical spelling, member order, or ignored members of input JSON are never
signed.

## Route catalog projections

The catalog is the canonical route superset, but consumers receive explicit
projections rather than identical sets. Each descriptor identifies its public,
operator, diagnostic, or protocol surface; listener; feature gate; OpenAPI and
SDK exposure; and explicit MCP allowlisting. Metrics, diagnostics, streams, and
privileged commands therefore do not become SDK operations or MCP tools merely
because they are mounted. The mounted projection is evaluated for the active
build, while the SDK projection is the canonical supported-build superset.

## `/v1` evolution

Until the first release is published, incompatible API changes replace the
pre-release contract directly; there are no aliases or migration DTOs. After
publication, the typed Norito layouts and their JSON mappings are frozen for
the lifetime of `/v1`. Reordering, adding, removing, or retyping a field, adding
a variant to a closed enum, changing requiredness, or changing a field's JSON
mapping requires a new major path unless the type already declares an explicit
extensibility mechanism. Adding an independent route or a new stable error-code
string is additive. Clients must handle unknown error codes without assuming
that human-readable messages are stable. Adding a required request field is
never compatible within `/v1`.

Internal wire and consensus types may keep implementation version suffixes.
Those names do not create nested route versions and do not provide a second
public response schema.
