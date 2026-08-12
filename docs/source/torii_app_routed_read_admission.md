# App API routed-read HTTP admission

All 45 first-release `ToriiReadEndpointV1` public handlers share one outer HTTP
admission corridor. Before a request body is polled, Torii reserves the existing
query-fanout working set. The same reservation is adopted by downstream
single-route and fanout execution and remains owned by the final response body.

The corridor counts the raw query and body against the route-body phase, rejects
body bytes on bodyless routes, checks `Content-Length` without trusting it as the
only bound, limits unknown-length streams by bytes and the absolute deadline, and
collects into one exact-capacity owner-backed buffer. JSON is lexically
preflighted before measured typed decoding. Form queries use a separate exact
expanded-JSON phase before entering the typed decode scope.

`torii.app_api_routed_read_body_read_timeout_ms` is the absolute deadline for
collecting one admitted routed-read body. Its default is `10000` milliseconds;
zero is rejected during configuration parsing. Timeout and capacity failures
release the reservation only after their response body is dropped.
