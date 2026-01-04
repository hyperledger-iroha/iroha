# Provider Admission Fixtures

These files are generated via `cargo run -p sorafs_car --features cli --bin provider_admission_fixtures`.
They provide deterministic governance proposals, adverts, envelopes, renewals, and revocations for
integration tests across Rust, Torii, and CLI tooling.

Additional artifacts capture a sample multi-source fetch plan so SDKs can exercise chunk
scheduling end-to-end.

Do not edit manually; rerun the generator if data changes.
