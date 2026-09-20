# Exact scaling measurements

`measure_run(PublicRunProjection, ResourceLimits)` returns immutable
`RunMeasurements`. `measure_experiment(ExperimentPlan, tuple_of_ten_projections)`
returns immutable `ExperimentMeasurements`. Both are pure computations over
exact known public snapshot types. They perform no file/network/process work,
decode no Norito, and create no authority, native receipt, or release verdict.

The caller must obtain each projection from the original completed-run owner
and independently join its raw resources and native proof before publication.
A caller-constructed snapshot is only a measurement input; passing its shape
checks does not authenticate it. The original experiment owner retains build,
source, executable, raw-file, process, runtime and budget authority.

## Definitions

- The offered schedule is exact rational arithmetic. Canonical
  `FactsJournalPlan`/`journal_snapshot` supplies counts and bounded timing.
  Every warmup and measurement request must appear once, in scheduled order,
  with its original pair-seeded logical ID and account rotation. Global and
  peer-local Applied observations must have the same positive height, valid
  offers/acknowledgments, and finish within their unchanged deadlines.
- Committed throughput counts measurement-cohort requests whose **global**
  Applied offset lies in `[0, measurement_ns)`, divided by the full declared
  measurement duration. An observation exactly at the measurement endpoint
  belongs to drain. Local completion and acknowledgment may race global
  completion; they never replace the global throughput timestamp.
- Latency is integer nanoseconds from actual offer to global Applied for
  **every** measurement-cohort request, including drain completions through
  `measurement_ns + drain_ns`, inclusive. At least 100 samples per run are
  required; a higher declared minimum is retained. p95 uses nearest rank:
  sorted sample at `ceil(95 * count / 100)`, with one-based indexing.
- Warmup begins at `-(warmup_ns + drain_ns)`, stops offering before
  `-drain_ns`, and must finish strictly before zero. Its counts/p95 are separate
  and cannot improve measurement throughput or dilute measurement latency.
- The ten runs must be five ordered one-lane/four-lane pairs. Their original
  plan snapshots must match the declared experiment, and all workload/policy
  fields and the common resource limits remain equal apart from pair index,
  pair-derived seed, variant and active lane count. Each pair uses the same
  account pool. Duplicate transaction hashes anywhere in the matrix reject.
- Throughput ratio is the median of five four-lane throughputs divided by the
  median of five one-lane throughputs. It is not the median of pair ratios.
  The one-lane median must be positive. The criterion is `ratio >= 3/2`.
- Experiment p95 pools the complete measurement cohorts of all five runs for
  each variant. It is not a median of per-run p95 values. The ratio criterion
  is `four_lane_p95 / one_lane_p95 <= 5/4`.

Throughputs and ratios are `Fraction` values; durations/latencies are integers.
Public encoders should preserve exact numerator/denominator fields instead of
rounding before comparison. The result exposes separate throughput, latency
and observed-resource criterion booleans, with no combined `passed` member.

## Resource scope and bounds

The resource reduction uses the existing `resource_experiment._maxima` owner:
maximum queue sum across the four peers, represented index entries, maximum of
the before/after aggregate RSS observations, and Kura storage bytes. It includes
preflight and every scheduled capture through the final sampling tail. Sample
count/order, exact four peer snapshot shapes, distinct PIDs, geometry and
response brackets are checked, including the last sample's original fixed
terminal deadline. It compares these observations to the same declared
`ResourceLimits` in every run.

These are **observed maxima**, not continuously measured peaks or live memory/
disk enforcement. They do not cover the complete CLI/Kagami/Python/parent/GPU
runtime or every mutable storage subsystem. Full runtime closure and actual
whole-runtime enforcement remain mandatory independent qualifications.

Admission bounds precede loops, conversions and sorting: at most 65,536
requests per run, ten runs, 100,000 resource samples per run, 64 accounts,
fixed-depth known policy records, bounded strings and exact integer domains.
All latency pools are therefore bounded. These algorithmic bounds do not
claim a host-wide memory quota. Tests use synthetic public snapshots and
exercise arithmetic, endpoint, cohort, identity, matching and resource errors;
they are not benchmark measurements or cryptographic proof evidence.
