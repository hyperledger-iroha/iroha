# SCCP fixtures

`native_transfer_event_v1.json` tracks the current public Taira identity,
`fc56984b-2be7-431d-840e-21514d1883f0`.

`ton_stateinit_golden_v1.json` is emitted by the Tolk contract layout itself.
It binds every canonical TON mainnet route input, the zero deployment state,
the route and Jetton-master code/data cell hashes and depths, and both
StateInit-derived raw addresses. The four depths are emitted directly from the
canonical Tolk cells as bounded `u16` values; they are not inferred by a host
implementation. Its source inventory covers the complete contract closure.
Regenerate or verify it with exact Acton 1.1.0/Tolk 1.4.1 and an authenticated
release archive:

```text
python3 scripts/generate_ton_sccp_stateinit_golden.py \
  --acton /absolute/path/to/acton \
  --acton-archive /absolute/path/to/acton-1.1.0.tar.gz \
  --check
```

Use `--write` only for an intentional reviewed contract/layout change. The
macOS archive identity is accepted for development parity checks; production
release artifacts remain restricted to the digest-pinned Linux/amd64 builder.

`release_evidence_v1/` is an explicitly retired, test-only negative snapshot.
Its finality anchors use Sumeragi protocol v3, which first-release SCCP rejects;
`scripts/sccp_release_fixture.py reject` proves that boundary. The snapshot
must not be validated, bundled, verified, resealed, or presented as canonical
release evidence. Its detached signatures use disposable, non-production keys
whose private material is not retained, and production policy loaders deny
every published fixture public key.
