# F02 canonical recovery frame length without payload cloning

The canonical executed-block recovery request and response size guards previously
cloned the entire typed message into a `BlockMessage`, encoded it into a new
buffer and discarded that buffer after reading its length. A response can carry
a maximum-size chunk, so this check retained a second payload-sized owner at
the admission boundary.

The guard now measures the borrowed inner Norito payload with `encoded_len()`
and adds the two canonical compact-length prefixes and u32 `BlockMessage`
variant using checked arithmetic. An unrepresentable length refuses the frame.
The request and response tests compare this calculation with actual encoded
`BlockMessage` bytes from a real canonical recovery worker; a separate test
checks arithmetic overflow refusal. The response still uses the same exact
authenticated P2P frame limit, and no wire layout or capacity value changed.

On the merged source, the focused canonical worker selector passes 1/1 and the
arithmetic-overflow selector passes 1/1 in the same Core library test binary.
The first worker run exposed a test-only double count: matching the response
through `BlockMessage` yielded `&Box<Response>`, so the test passed a length
that already included the box prefix into the helper. The assertion now uses
the unboxed response length, matching the production function's `&Response`
argument. The direct encoded frame and checked calculation agree for both
request and response. `git diff --check` and the retired-codec guard pass.

This removes only the outer clone and output buffer in these frame checks.
Nested Norito serializer scratch, the rest of the recovery worker's retained
allocations, original State-pool charge integration and full maximum-size
admission remain F02 gates.
