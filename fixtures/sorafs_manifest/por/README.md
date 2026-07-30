Reference Proof-of-Retrievability fixtures generated via:

```
cargo run -p sorafs_manifest --bin generate_por_fixtures -- --write
```

- `challenge_v1.to` / `.json` — canonical PoR challenge and human-readable breakdown.
- `proof_v1.to` / `.json` — provider response with sample digests and auth path.
- `verdict_v1.to` / `.json` — audit verdict referencing the proof digest and auditor signatures.

All payloads use the Norito encoding shipped in `sorafs_manifest::por`. The
proof and verdict fixtures carry deterministic Ed25519 signatures over the
domain-separated canonical Norito signing payloads and the fixture tests verify
those signatures, not only their byte shape.
