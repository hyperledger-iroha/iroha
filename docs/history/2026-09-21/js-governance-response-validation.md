# JavaScript governance response validation

The JS referendum/tally/lock read paths now consume bounded lossless JSON and
validate the current Core field inventory. Frozen PLAIN policy and closed-result
equations, required custody/duration, u64/u128 bounds, aggregate overflow,
lock-owner joins, requested tally/corpus selectors and evaluated tally coordinates
are preserved. The typed lock method projects the inner native corpus after
validation. Raw wire misses omit the absent record; typed helpers retain explicit
not-found convenience results. No zero tally is invented on HTTP 404.

`fixtures/governance/plain_v1` contains 23 source-contract-derived hand-authored
vectors, including unquoted u128 maxima, unsafe-in-Number integers and eleven
negative tally cases. Its README identifies native producer/roundtrip work still
required. These fixtures and exact-string identifier checks do not authenticate
state, identities, permissions, finalized history or private elections.

Validation from `javascript/iroha_js`:

- `node --test test/governancePlainV1.test.js test/governancePlainV1.types.test.js
  test/numericV1.test.js test/strictLosslessJsonWriter.test.js
  test/strictLosslessJsonResources.test.js`: **136 passed**, zero skipped, 0.682s;
  `target/first-release-js-governance-plain-tests-4.log`.
- Scoped ESLint and syntax checks for the modified client/test modules pass.
- `node --test --test-name-pattern='getGovernance(Referendum|Tally|Locks)'
  test/toriiClient.test.js` failed before test registration because the existing
  native binary has stale source provenance (`ERR_IROHA_NATIVE_BINDING`,
  `source_provenance_error`). Log:
  `target/first-release-js-governance-client-tests-1.log`. The native guard was
  preserved. The 28 new authenticated transport controls remain unexecuted until
  the coordinated native rebuild and JS dist refresh.

The first fixture-generation attempt also stopped at native source provenance;
no generated identity or evidence from that failed attempt is used. Existing
checked-in account/asset strings are the documented source of the hand vectors.
The first two pure-test iterations exposed test-only prototype comparison and
missing serializer-context arguments; both were corrected without changing the
production validation contract. Earlier logs remain under `target`.

No Cargo, native rebuild, Git mutation, admission change or privacy qualification
was performed by this slice. Candidate-specific SDK/native parity and release
qualification remain open.
