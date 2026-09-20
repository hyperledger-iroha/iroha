# Fixed experiment input boundary

`scripts/nexus/scaling_experiment_config.py` owns pure prelaunch decoding of the
single `scaling_experiment_plan.PLAN_SCHEMA` and the existing complete
`resource_evidence_budget.run_budget_inputs` envelope. It creates no files,
processes, runtime identities or release verdicts.

- `decode_plan(bytes)` returns an owned `ExperimentPlan` with exactly ten
  explicit trials, all six native policies per trial, and every resource limit.
  It validates the full JSON shape before constructing policy records. This
  structural result still needs budget admission before execution.
- `decode_fixed_inputs(plan_bytes, budget_bytes)` uses the existing
  `parse_run_budget` and `admit_plan`, then returns the owned plan, the owned
  complete `EvidenceBudget`, and canonical public plan bytes. The envelope must
  select pair 1 / `one_lane`; all ten run allocations remain mandatory.

Each input is an exact immutable `bytes` value with a 1 MiB limit. Framing checks
bound nesting, structural tokens and string wire length before JSON parsing.
Parsing rejects duplicate keys, floats, non-finite values and oversized
integers. Bounded tree checks reject foreign scalar types, oversized strings,
arrays and objects before schema construction. Unknown fields, missing fields,
implicit defaults and alternate schemas are errors. Failures expose only
`ExperimentConfigError("fixed_experiment_config_invalid")`.

Readable whitespace, object-key order and equivalent JSON string escapes are
accepted. Public bytes use the existing canonical serializers; the `plan`
static-file allocation must match that canonical byte count. No second budget
schema, legacy decoder, command override or omitted-policy fallback is admitted.

The focused tests use the actual fixed ten-run plan/budget fixture and exercise
full native-policy admission, equal offered work, both selected-run fields, all
fifteen public artifact allocations, size boundaries and malformed inputs. They
do not execute native commands, prove runtime provenance, enforce live host
quotas or close the scaling release gate.
