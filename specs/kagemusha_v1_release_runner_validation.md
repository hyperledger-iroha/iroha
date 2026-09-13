# KAGEMUSHA V1 release-runner software validation

This record covers the 2026-09-13 Python corrections in the `optimizations`
checkout. KAGEMUSHA remains **not production qualified**. These checks exercise
software process supervision and synthetic signed evidence; they do not run a
real recursive proof or qualify an OEM service or physical device. The broader
outstanding outcomes remain in [production readiness](kagemusha_v1_production_readiness.md).

## Enforced behavior

- `run_kagemusha_real_proof_guarded.py` requires finite positive timeout and
  progress intervals. Completion is evaluated against a fresh clock observation
  after process polling and resource accounting. Completion first observed at or
  after the deadline cannot qualify as an in-time run.
- `run_kagemusha_v1_release_evidence.py` includes child observation, descendant
  quiescence, transcript synchronization and stable capture in its timeout.
  The final duration is rounded up to milliseconds and must remain strictly
  below the configured limit. A successful exit does not override that limit.
- Collector exceptions attempt termination, descendant cleanup and reaping of
  the owned child. Reaping is recorded before another fallible observation,
  and cleanup drains an exited leader while checking the process group. Empty
  groups receive no termination signal. Both transcript descriptors receive a
  close attempt even if the first close raises. Cleanup errors remain failures;
  this is not a guarantee against arbitrary operating-system failures.
- The release projector applies the collector's existing argument rule to
  typed file operands: their complete relative path cannot begin with `-`.
  Paths such as `reports/--help` remain valid. A signed observation cannot turn
  `--help`, `-h` or `--` into a consumed input file.
- Physical inbox/outbox recovery compares every durable record field while
  excluding only `latency_ms` and `rss_bytes` from identity equality. Original
  and recovery measurements still receive full schema, signature and resource
  validation. Every measurement contributes to the existing quantiles and RSS
  maximum. See the [physical-evidence contract](kagemusha_v1_physical_evidence.md).

## Actual validation

Each run below retained its command, source hashes before and after execution,
stdout/stderr and JUnit result. All captured inputs remained stable during each
run, and every listed suite finished with zero failures, errors or skips.
Subtests are reported separately from named test cases.

| Scope | Actual result | Wall time |
| --- | --- | --- |
| Proof guard, release collector and exceptional process cleanup | 70 named tests and 64 subtests passed | 3.46 s |
| Release projector and physical provenance | 156 named tests passed | 306.12 s |
| Complete expanded physical-device verifier suite | 52 named tests and 210 subtests passed | 59.54 s |

The projector/provenance run preceded the final physical-test/spec expansion;
both production verifiers remained byte-identical across those runs. The final
physical run covers all 52 retained and added tests. These are separate scoped
receipts, not a single immutable production-candidate qualification.

Regression coverage includes finite-number parsing, completion at and after the
deadline, final-capture timeout crossings, live-child and post-reap exceptions,
cleanup failures, descriptor closure, typed option-like filenames, all durable
record substitutions, resource measurements in all four recovery positions,
the exact 30,000 ms overall p95 boundary, stale observer approvals and unknown
record fields. The collector suite retains its disposable Python child and
descendant test; the fault-injection cases use controlled process/clock doubles.

Run the scoped suites from the repository root:

```sh
PYTHONDONTWRITEBYTECODE=1 python3 -m pytest -q -p no:cacheprovider scripts/tests/run_kagemusha_real_proof_guarded_test.py scripts/tests/run_kagemusha_v1_release_evidence_test.py scripts/tests/kagemusha_release_process_cleanup_test.py
PYTHONDONTWRITEBYTECODE=1 python3 -m pytest -q -p no:cacheprovider pytests/scripts/kagemusha_v1_release_evidence_test.py pytests/scripts/kagemusha_v1_physical_provenance_test.py
PYTHONDONTWRITEBYTECODE=1 python3 -m pytest -q -p no:cacheprovider pytests/scripts/kagemusha_v1_physical_device_test.py
```

Local raw receipts are retained under
`target/kagemusha-validation/proof-guard-deadlines-20260913-r1/final-all-python-r1/`
and `target/kagemusha-validation/physical-recovery-metrics-20260913-r1/`
(`release-provenance/` and `physical-expanded-final/`). They are generated
validation artifacts, not required inputs for a fresh checkout.

The release gate remains closed pending complete proof integration, measured
resource limits, durable coordinator recovery, same-candidate SDK and network
qualification, independent cryptographic review and each enabled model's
qualified OEM service and physical evidence. Brand coverage alone establishes
none of those properties.
