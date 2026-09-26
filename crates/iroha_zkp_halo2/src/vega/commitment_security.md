# Vega commitment secret boundary

Hyrax witness commitments use `SecretMultiexpBuilder` with the serial T256
suite. Each scalar, including the row blinding, is borrowed into an exact-capacity
clearing owner. The shared evaluator executes 64 four-bit windows, scans all 16
table entries per term, and clears retained scalar copies, encodings, tables and
intermediate points on normal return and unwind. It does not call the membership
suite's generator constructor; the Hyrax key supplies the actual bases.

Only public input lengths omit a padded suffix. A populated zero coefficient
has the same work schedule as any other coefficient. Empty public rows still
evaluate their blinding with the same secret arithmetic. The resulting Pedersen
commitment is public; identity rejection occurs after secret workspace cleanup.
Variable-time `msm` is reserved for public transcript weights and disclosed
verifier values.

Dedicated commitment workers process independent rows and retain input order.
Their secret MSM does not enter the ambient Rayon pool. `VegaMdlProverConfigV1`
limits these dedicated workers only: public verifier arithmetic may use Rayon,
and the configuration does not reserve memory or establish a peak-RSS limit.
The former unmeasured resident-memory constants and accessors are removed.

Tests compare full, partial, empty-padded, sparse and dense rows against direct
group arithmetic; compare one and three workers; observe the same selection
counts for different scalar patterns; and check actual retained-scalar clearing
after success, rejected identity, and unwind. These are source-level controls,
not a hardware timing assessment. Rust `Copy` values and compiler-created
temporaries retain the documented best-effort erasure limit. Governed full-shape
proof qualification and independent side-channel review remain required; the
Vega compiled-profile activation gate stays closed.
