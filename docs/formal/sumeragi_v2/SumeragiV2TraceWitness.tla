---- MODULE SumeragiV2TraceWitness ----
EXTENDS SumeragiV2

(***************************************************************************
Trace-generation entry point, not a proof module.

TLC is asked to violate NoDecision so `-dumpTrace json` emits one finite
LivenessSpec behavior through the first durable decision.  The resulting JSON
is normalized and replayed against the production Rust reducer.  This operator
must never be listed as a safety invariant or proof obligation.
***************************************************************************)

NoDecision == decisions = {}

=============================================================================
