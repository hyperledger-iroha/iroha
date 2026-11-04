# Current frontend and prover logical accounting

The reviewed source inventories now bind the current 91-file frontend set and 86-file prover set. The original retained inventories exactly match the historical source map in the current resource-accounting review. All accounting function/class ASTs remain unchanged apart from a source-count docstring; pins and the frontend source-count regression are refreshed. Existing report tests now require exact equality with the historical numeric cases.

All 31 focused tests and 182 subtests pass. The captures retain actual commands, exact source hashes before and after, and normal/refusal JSON equality. Frontend 16 cases and prover eight selected cases, all 1,996 vote-size checks, counter bounds, and arbitrary-preimage counterexamples match their historical values. The frontend refusal exits 1 and the prover refusal exits 2. The three formerly failing historical nodes also pass a separate exact-node rerun recorded in `historical-three-node-recheck.json`.

These are logical component results. Comparator calculations apply to diagnostics builds. Provider internals, compiled-image correspondence, native allocation, stack, production framing and full cryptographic qualification remain excluded. Qualification remains false. Earlier evidence artifacts are unchanged.
