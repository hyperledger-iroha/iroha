# Restricted native-lane gossip

The sixteen-node diagnostic's public first pool finalized at global height 5,
but its second, restricted pool did not reach its disjoint participant committee.
Restricted transaction gossip selected the global commit topology. Connected
global validators therefore received that batch while the native lane members
received none.

The sender now groups transactions by exact `(dataspace, lane)` and reads the
current native incarnation, activation/closing boundary, pinned committee and
live participant-role keys from one committed State view. Connected-peer and
fanout limits only narrow that authority. Missing authority or recipients defer
the original pending transactions. Sibling lane committees never share a body
batch. Both receive paths require local native membership before materialization;
Torii ingress remains a separate admission boundary. Route, plane, signature,
canonical-plan and frame-bound checks remain enforced.

The public-overlay fallback, its configuration keys, enums and telemetry fields
are removed. The configuration parser rejects the retired keys. The four affected
status wire records were captured from the current typed codec, preserving the
other 28 fixture records. Exact metric-catalog byte/count/hash guards were updated
to the reduced catalog; they were not relaxed.

## Scoped evidence

- The actual original selector and its original policy types were extracted from
  preserved source into a temporary test module. With disjoint four-member BLS
  global/native committees and all peers connected, its native-recipient assertion
  fails with the four global keys. The new committed-native selector passes the
  same case. The temporary module is absent from the candidate.
- The rebuilt candidate passes all 123 tests selected by `gossiper::`, including
  peer gossip, plus both affected Core telemetry tests. Controls cover exact
  native recipients, isolation between sibling lanes, unavailable recipients,
  replacement incarnations, activation/closing, invalid committees, key expiry,
  revocation, wrong roles/PoPs, owned/shared receive paths and plane spoofing.
- Configuration retirement and the minimal configuration snapshot each pass;
  the compiled wire-capture generator and its typed roundtrip/truncation control
  pass. The codec guard and scoped formatting checks pass.
- An earlier full run retained 85 passes and 39 failures: the intentional original
  selector failure, 36 stale metric-catalog metadata failures, one incorrect
  participant-size test assumption, and one test demanding an already retired
  gossip discriminant. The candidate fixes the metadata and tests, without adding
  protocol cardinality restrictions or changing transaction gossip wire bytes.

Local logs, exact original-selector source and its retained executable are under
`dist/architecture-redesign-2026-09-30/restricted-gossip/`. Configuration and typed
fixture captures are under
`dist/architecture-redesign-2026-09-29/restricted-gossip-config-retirement/`.
These are scoped controls, not successful network settlement or release evidence.
The fresh daemon/harness build and real-network execution remain required.
