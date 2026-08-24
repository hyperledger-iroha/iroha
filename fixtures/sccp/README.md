# SCCP fixtures

`native_transfer_event_v1.json` tracks the current public Taira identity,
`fc56984b-2be7-431d-840e-21514d1883f0`.

`release_evidence_v1/` is an explicitly retired, test-only negative snapshot.
Its finality anchors use Sumeragi protocol v3, which first-release SCCP rejects;
`scripts/sccp_release_fixture.py reject` proves that boundary. The snapshot
must not be validated, bundled, verified, resealed, or presented as canonical
release evidence. Its detached signatures use disposable, non-production keys
whose private material is not retained, and production policy loaders deny
every published fixture public key.
