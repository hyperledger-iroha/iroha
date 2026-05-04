# Space Directory Fixtures

This directory hosts canonical samples for the Nexus **Space Directory**:

- `capability/cbdc_wholesale.manifest.json` — AssetPermissionManifest for a
  wholesale CBDC participant (UAID → deterministic allowances).
- `capability/retail_dapp_access.manifest.json` — Capability policy for a
  retail dApp dataspace that participates in the same composability group.
- `capability/eu_regulator_audit.manifest.json` — Read-only capability policy
  for EU/ESMA auditors that need MiCA-compliant telemetry without mutation
  privileges.
- `capability/jp_regulator_supervision.manifest.json` — JFSA-oriented manifest
  showing how regional regulators can combine capped stop orders with
  deny-wins guards.
- `profile/cbdc_lane_profile.json` — Dataspace profile describing governance,
  DA attesters, composability whitelist, and audit hooks that Space Directory
  entries will eventually expose.
- `capability/*.manifest.to` — Norito-encoded payloads for the canonical
  capability manifests. Each `.to` file is the deterministic encoding of the
  adjacent JSON source; current digests:

  | Manifest | Norito file | BLAKE3 |
  |----------|-------------|--------|
  | `capability/cbdc_wholesale.manifest.json` | `capability/cbdc_wholesale.manifest.to` | `41094b22b6b65498654818d1302b92b6bbd3f653862cc349d229c706c82ef04d` |
  | `capability/retail_dapp_access.manifest.json` | `capability/retail_dapp_access.manifest.to` | `2349b346fe5b7f55ac81fcd4813b408c236a62b62c9d3b52819b7258d3eb1d61` |
  | `capability/eu_regulator_audit.manifest.json` | `capability/eu_regulator_audit.manifest.to` | `82ee92406e6e361b0f32735d765ee5f21e0aeafdc6d119ffb43bc3930021fabd` |
  | `capability/jp_regulator_supervision.manifest.json` | `capability/jp_regulator_supervision.manifest.to` | `fc173ed0949bfd6ec931b91a355f5cc29be97859e9e723b15a94a596c9f51ec9` |

The fixtures are referenced by docs (`docs/space-directory.md`) and will back
upcoming integration tests once the Space Directory contract lands.

## Regeneration

1. Edit the JSON sources in this directory. Use
   `docs/space-directory.md#manifest-template` as the authoritative schema
   reference.
2. Convert each JSON manifest into its Norito `.to` representation for on-chain
   publication. `iroha app space-directory manifest encode` takes care of the
   conversion (any tool that uses `norito::to_bytes` will emit identical bytes):

   ```bash
   cargo run -p iroha_cli --bin iroha -- app space-directory manifest encode \
     --json fixtures/space_directory/capability/cbdc_wholesale.manifest.json \
     --out fixtures/space_directory/capability/cbdc_wholesale.manifest.to
   cargo run -p iroha_cli --bin iroha -- app space-directory manifest encode \
     --json fixtures/space_directory/capability/retail_dapp_access.manifest.json \
     --out fixtures/space_directory/capability/retail_dapp_access.manifest.to
   cargo run -p iroha_cli --bin iroha -- app space-directory manifest encode \
     --json fixtures/space_directory/capability/eu_regulator_audit.manifest.json \
     --out fixtures/space_directory/capability/eu_regulator_audit.manifest.to
   cargo run -p iroha_cli --bin iroha -- app space-directory manifest encode \
     --json fixtures/space_directory/capability/jp_regulator_supervision.manifest.json \
     --out fixtures/space_directory/capability/jp_regulator_supervision.manifest.to
   ```

3. Run the Nexus capability tests to ensure the fixtures still round-trip and
   the `.to` payloads match the JSON sources:

   ```bash
   cargo test -p iroha_data_model nexus::manifest -- --nocapture
   cargo test -p integration_tests nexus::cbdc_capability_manifests_enforce_policy_semantics
   ```

4. Reference the JSON fixtures from docs/tests instead of re-encoding manifests
   ad hoc. This keeps deterministic hashes stable across SDKs.

## Notes

- All numeric allowances use decimal strings so they can be losslessly parsed
  into `Numeric`.
- UAIDs follow the `uaid:<hex>` format (`Hash` serialization). Generate new
  UAIDs with a deterministic blake2b helper:

  ```bash
  python3 - <<'PY'
  import hashlib
  seed = bytes.fromhex("<32-byte-hex>")
  print("uaid:" + hashlib.blake2b(seed, digest_size=32).hexdigest())
  PY
  ```
- Capability manifests use *deny-wins* semantics, so keep explicit denies after
  the relevant allow entries.
