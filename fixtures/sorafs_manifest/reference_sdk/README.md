# Reference-SDK bundle outcome goldens

These files are byte-exact `ValidationOutcomeV1` goldens for every SDK's
heterogeneous fixture-bundle wrapper. Every outcome uses
`generated_at=1700001234`.

- `bundle_heterogeneous_positive_validation_outcome_v1.json` validates the
  replication order, all five orderbook payloads, the PDP triplet, the PoR
  challenge/proof pair, the PoTR receipt, and the repair task at
  `now=1700000001`.
- `bundle_routing_admission_positive_validation_outcome_v1.json` validates the
  provider advert and admission envelope at `now=300`.
- Five payload-negative outcomes add routing or proof context before the
  bad-signature, trailing-byte, duplicate-hot-leaf, missing-signature, or
  wrong-provider payload. They use `now=1700000001`, return the bundle-level
  `SFS-BND-001` code, and retain the exact underlying failure as the
  `payload_code` context value.
- Two link-negative outcomes use a replication order plus a structurally valid
  repair task to produce exact manifest-mismatch (`SFS-BND-002`) and
  unassigned-provider (`SFS-BND-003`) results.

The signed, SHA-256/length-bound closed inventory is
`../reference_sdk_validation_inventory_v1.json`. Verify it without network
access:

```sh
python3 scripts/check_sorafs_reference_sdk_fixtures.py
```
