# Fixed experiment public projection

`manifest_bytes` consumes the exact ten original `RunManifest` snapshots in
one-lane/four-lane order for pairs 1 through 5. Each snapshot supplies the fifteen
original `PublicFile` roles in `RUN_FILE_FIELDS` order and its `CaptureCensus`.
The output retains every native proof, request, bundle, facts, query and finality
file binding; it cannot substitute a hash-only caller record for original custody.
The original three input bindings have fixed public relative paths. The plan
binding must match the canonical admitted plan; the complete budget comes from
the existing canonical budget projector. Recorded trial deadlines must be
strictly increasing and strictly before the unchanged original experiment end.

The budget is type/count/cap re-admitted and its projection allocation precharged
before copying its tree. JSON encoding is measured before the complete output
is allocated. Supplied reservations may use the canonical writer's full 256 MiB
range. Actual output remains bounded by both that reservation and the module's
intrinsic 1 MiB ceiling; the manifest also checks its admitted writer cap. A
generous valid reservation therefore cannot fail only because it exceeds 1 MiB.

`measurement_identity` returns all fields of the exact `ExperimentMeasurements`
and `RunMeasurements` classes in their declared field order. Fractions become
canonical `(numerator, denominator)` primitive pairs and resource records become
four integer tuples in their declared field order. Exact latency tuples are
retained after validating every integer, with 65,536 total requests per run and
327,680 measurement samples per variant. No mutable record, Fraction, foreign
subclass or generic deep copy is retained in that identity. This is a projection
shape check; the original owner must pin the actual `measure_experiment` result.

`report_bytes` emits the fixed `observed_measurements` scope, exact rational
throughput and p95 comparisons, ten run summaries, observed resource maxima,
and three separate criterion booleans. It excludes full latency arrays and has
no combined PASS, runtime qualification flag, private configuration, seed or
runtime path. The manifest digest binds its public artifact context. Original
file/capture/CompletedRunAuthority and runtime checks remain the outer owner's
responsibility. Pure projections perform no I/O, clock, subprocess or native
verification operation and mint no successful-trial or release authority.
