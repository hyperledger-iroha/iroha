The stored IPA implementation now contains the consuming assignment, coefficient,
lookup, product, vanishing, quotient-numerator, inverse-transform and
quotient-commitment stages. The following scalar stage retains its original
proof owner through the ordinary scalar transcript sequence and builds an immutable
opening plan; it does not return a complete proof. The authenticated Core
indexed-key owner and encrypted polynomial store are present. These components
remain separate from the normal production proof entry points: the six direct
`create_proof` calls and two `create_proof_consuming` calls in
[generation.rs](../crates/iroha_core/src/zk/kagemusha_v1_recursion/generation.rs)
use dense `ProvingKey` owners. The stored prefix, quotient-commitment continuation
and Core `capture_indexed_proving_key` have only test callers in the inspected
source. Scalar evaluation, lazy H coefficient reconstruction and consuming
original advice-blind folding are implemented and pass the focused default and
no-multicore checks below. The next multiopening continuation through P is in development and
has no qualification result. Complete stored opening generation and authenticated
Core producer/key integration remain required. Incremental scratch bounds do not
establish whole-process memory or latency compliance. Sources:
[stored owner](../vendor/halo2-axiom/src/plonk/prover/stored.rs),
[quotient commitments](../vendor/halo2-axiom/src/plonk/prover/stored/quotient_commitments.rs),
[indexed artifact owner](../crates/iroha_core/src/zk/kagemusha_v1_recursion/artifacts/stored_key.rs).
