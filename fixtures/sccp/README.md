# SCCP fixtures

`native_transfer_event_v1.json` tracks the current public Taira identity,
`fc56984b-2be7-431d-840e-21514d1883f0`.

`release_evidence_v1/` is an explicitly test-only signed snapshot for the
current public Taira identity. Its detached signatures use disposable,
non-production keys generated only in memory outside tracked tooling; the
production policy loaders deny every published fixture key. It must never be
presented as operator-signed public Taira release evidence.
