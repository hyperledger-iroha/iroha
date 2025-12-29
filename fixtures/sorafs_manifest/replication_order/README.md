Reference replication order fixture set used by SDKs, Torii smoke tests, and the conformance harness.

- `order_v1.json` – human-readable breakdown of the sample order (hex identifiers, decoded chunk plan, governance tickets).
- `order_v1.to` – canonical Norito encoding generated via `cargo run -p sorafs_manifest --bin generate_replication_order_fixture`.

Companion assets produced from the same fixture run live alongside this directory and keep the replication order, PoR proof set, and governance audit log in sync:

- `../por/` — contains `challenge_v1.*`, `proof_v1.*`, and `verdict_v1.*` (generated with `cargo run -p sorafs_manifest --bin generate_por_fixtures`) so harnesses can exercise the full request→proof→verdict flow.
- `../governance/` — wraps the proof verdict in a `GovernanceLogNode` (`node_v1.*`) for pipeline and dashboard ingestion.

Every artifact round-trips through the Norito codec in `sorafs_manifest::{capacity,por,governance}` and shares the same manifest digest referenced in `order_v1.json`. The fixture round-trip tests under `crates/sorafs_manifest/tests` guard this bundle; run `cargo test -p sorafs_manifest` after regenerating to confirm the order/PoR/governance samples remain in sync. Regenerate the full bundle whenever the schemas change to keep the order, PoR, and governance fixtures aligned.
