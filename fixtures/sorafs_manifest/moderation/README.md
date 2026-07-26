# Moderation governance fixture

`governance_node_v1.to` is a signed `GovernanceLogNodeV1` carrying a real
`GovernanceLogPayloadV1::ModerationBallotEvent` with a finalized tally. The
JSON sidecar is payload-free fixture commentary; the validation outcome is the
canonical `ValidationOutcomeV1` emitted at `generated_at=1700001234`.

Regenerate it together with the release-wide SDK inventory:

```sh
cargo run --locked --offline -p sorafs_manifest --bin generate_por_fixtures
```

The deterministic Ed25519 key is test-only fixture material. Production
moderation identities, evidence, holder data, and signing keys must never be
derived from or replaced by this fixture.
