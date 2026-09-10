# Reviewed dependency pruning

Nine unused or exactly duplicated declarations are removed from four manifests:

- `sorafs_manifest`: unused normal `norito_derive`, `sha3` and `http` edges.
- `iroha_crypto`: unused normal `arrayref` and duplicate development `tempfile`.
- `sorafs_node`: duplicate development `base64` and `blake3`.
- `sorafs_orchestrator`: duplicate development `blake3` and `iroha_crypto`.

The duplicate edges had identical effective versions, features and default-feature
settings. Their normal dependencies remain available to tests. Norito still owns
and exports its derive macros. The reviewed no-follow filesystem dependencies,
shared proof-capacity constant, CLI signing and actual CLI test dependencies stay
with their implementation owners.

Cargo resolves the candidate offline. The lock changes only the four removed
normal dependency entries; every package, version and unrelated lock entry is
preserved. This does not claim that transitive registry packages disappear.
Independent graph comparison matches the previously reviewed nine-edge subtraction
exactly. All 60 source metrics across five scopes fit their unchanged limits, and
all 16 feature-resolved normal/build dependency boundaries pass.

Only the dependency baseline fingerprint changes, to
`sha256:b82665cbf6bbd558d6737181f6bed4cb8cd084f3f6678eeb5a647973f5c40436`.
No numeric limit, forbidden edge or layer policy is relaxed. All 45 dependency
checker tests pass. The four default libraries compile on unchanged source
`dc2705836c3bd32a590c98dcbbba3ac8df9c04d07651f5452c60a3235ba8bd92`.

The broader all-target check fails in the remaining standalone `sorafs_cli`:
three frame owners lack declared identities, and tests retain an absent helper
and unresolved type inference. That failure is preserved, not replaced by the
library result. Completing its canonical CLI migration and wider feature/runtime
qualification remains required.

The release-loader review reconstructs the preceding source pin from exact
beforeimages and permits only the four manifests, lock and budget fingerprint
delta. Ten drift controls pass. The checker changes only its normalized pin, to
`803b6cb43a720c0834a5855df38bcda9b48f0b16df616cdb8ad3747407db3eec`.
The shipping feature graph and all 846 combined release feature, profiling-contract
and automation tests pass with that source surface unchanged. These checks do
not qualify binary release execution or measured compiler-memory improvements.

Beforeimages, graph/lock comparisons, source reports, the failed all-target run,
reviewed pin proposal and test logs are retained under
`target/architecture-redesign/dependency-pruning-v1/`. Compiler records are
`dependency-pruning-check-1.json` and `dependency-pruning-libraries-1.json` under
`target/architecture-redesign/sdk-musubi-capability/`.
