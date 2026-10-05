`ledger_scoped_original_shapes_v1.json` holds public synthetic shape data for
`scripts/tests/ledger_scoped_originals_openapi_test.py`. That test checks the
authored OpenAPI schemas of `/v1/ledger/resource-names/{challenge}` and
`/v1/ledger/authority-originals` with JSON Schema.

- `attestation`, `world_snapshot` and `asset_definition` are copied verbatim
  from the retired `kagemusha_authority_state_v1.json` (SHA-256
  `7b14cb6035cf257dfa873eb58d1eb89c7923327a676ebd2a13688de852850ed0`). That
  file was the owned JSON emitted by a native `iroha_torii_shared` test over
  `NativeFinalityFixture`. The `resource_names_state` test in the same crate
  builds its fixture from the same inputs: the `shared authority state fixture`
  chain, challenge `[7; 32]`, observation time `1000000` and the same three
  synthetic World snapshot rows.
- `signatory` and `wallet` are two synthetic canonical I105 account spellings.
  Their canonical encodings are
  `0x02000120d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a` and
  `0x0a0101000100010100010020d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a`.

The snapshot rows are producer data, and one of them still names a retired
World field. JSON Schema treats `field_id` as an opaque string. Recapture this
file from the `resource_names_state` fixture when its rows change. This file
establishes no account, signature, World root, finality, read permission or
release qualification.
