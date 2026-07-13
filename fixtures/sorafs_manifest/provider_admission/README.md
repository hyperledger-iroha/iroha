# Provider Admission Fixtures

These files are generated via `cargo run -p sorafs_car --features manifest --bin provider_admission_fixtures`.
They provide deterministic governance proposals, adverts, envelopes, renewals, and revocations for
integration tests across Rust, Torii, and CLI tooling. Every admission object uses the first-release
V1 schema. Files named `*_renewed_v1` contain the V1 proposal, advert, and envelope carried by
`renewal_v1`; `renewed` describes lifecycle state, not a new schema version.

The generator uses test-only Ed25519 seeds `[0x21; 32]` for the provider and `[0x45; 32]` for
the one-member council. These keys are public fixture material and must never be used by a live
provider or governance council. Binary `.to` files are canonical Norito; matching `.json` files
are human-readable summaries, not alternative wire payloads.

Additional artifacts include a sample multi-source fetch plan so SDKs can exercise chunk scheduling
end-to-end.

Do not edit manually; rerun the generator if data changes.
