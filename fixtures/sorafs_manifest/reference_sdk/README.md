# Reference-SDK validation outcome goldens

These files are byte-exact `ValidationOutcomeV1` goldens shared by every SDK.
The two appeal-finance profiles use `generated_at=123`; heterogeneous
fixture-bundle outcomes use `generated_at=1700001234`.

The test-only signed, closed inventory covers 82 payload artifacts, 32 exact
`ValidationOutcomeV1` files, and 38 negative payload vectors. All eight
appeal-finance `CancelAssetLock` JSON/Norito files are mandatory; a missing file
fails validation rather than skipping a capability. These checked-in fixtures
do not qualify the current native packages: clean ABI-22 builds and unskipped
replay remain required for all five release targets.

- `appeal_finance_cancel_asset_lock_positive_validation_outcome_v1.json`
  accepts the canonical 85-byte `CancelAssetLock` V1 payload.
- `appeal_finance_cancel_asset_lock_zero_expected_negative_validation_outcome_v1.json`
  rejects the canonical zero-quantity negative with `SFS-VAL-001`.
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
