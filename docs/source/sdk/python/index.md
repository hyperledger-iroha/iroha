<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Iroha Python SDK

The Python SDK (`iroha-python`) exposes typed Torii helpers, Norito codecs, and
Connect utilities that mirror the Rust data model. These guides focus on the
roadmap work tracked under PY6 (Full Torii & Connect Coverage) so language
owners can ship repeatable workflows, notebooks, and release automation.

```{toctree}
:maxdepth: 1

connect_end_to_end
privacy_admin
release_automation
support_playbook
```

## Explorer helpers

The Torii client now exposes the `/v1/explorer/accounts/{account_id}/qr` route
so wallets and explorers can render canonical account QR codes without re‑implementing
the encoder. Call
`ToriiClient.get_explorer_account_qr_typed(account_id)`
to receive an `ExplorerAccountQrSnapshot`, which includes the canonical I105 account id,
the Norito literal used for the QR payload, the network prefix, error‑correction
setting, module count, QR version, and the inline SVG rendering emitted by Torii.
described in the ADDR‑6b roadmap item; omit it to keep the preferred i105 output
while still matching the QR payloads used by the JS and Swift SDKs.

## ISO 20022 bridge helpers

`iroha-python` now exposes typed wrappers around Torii’s ISO 20022 bridge. Call
`ToriiClient.submit_iso_pacs008_typed` or `ToriiClient.submit_iso_pacs009_typed` with a raw XML
payload to enqueue payments, then reuse `IsoSubmissionRecord` with
`wait_for_iso_message_status` to block until the message reaches a terminal
state:

```python
from iroha_python import ToriiClient

client = ToriiClient("https://torii.sora.example")
submission = client.submit_iso_pacs008_typed("<Document>...</Document>")
if submission is None:
    raise RuntimeError("bridge returned an empty payload")
status = client.wait_for_iso_message_status(
    submission.message_id,
    resolve_on_accepted=True,
    poll_interval=1.0,
)
print(status.status, status.pacs002_code, status.transaction_hash)
```

`resolve_on_accepted=True` mirrors the CLI/JS behaviour for operators who want to
treat `Accepted` without a transaction hash as success (useful when ledger commits
are delayed but the pacs.002 status must be reported). Bridge responses include
the derived pacs.002 code, optional transaction hash, ledger/asset identifiers,
and reason codes so automation can generate the same evidence bundles described
in :doc:`../../finance/settlement_iso_mapping`.

## Account inventory filters

Use `asset_id` filters on the account inventory helpers to pre-filter holdings,
transactions, and asset holder listings without building a full query envelope:

```python
from iroha_python import ToriiClient

client = ToriiClient("https://torii.sora.example")
asset_id = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM"

assets = client.list_account_assets("sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D", asset_id=asset_id, limit=10)
txs = client.list_account_transactions("sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D", asset_id=asset_id, limit=5)
holders = client.list_asset_holders("62Fk4FPcMuLvW5QjDGNF2a4jAmjM", asset_id=asset_id, limit=5)
print(assets, txs, holders)
```

## Offline lifecycle

The first-release HTTP lifecycle consists of exactly four canonical routes:

- `GET /v1/offline/readiness?asset_definition_id=...`
- `POST /v1/offline/top-up`
- `POST /v1/offline/redeem`
- `GET /v1/offline/operations/{operation_id}`

POSTs send the typed request directly as canonical Norito and return `202
Accepted` with a typed operation reference and `Location`. They do not accept
JSON or whole-request base64 wrapper objects. The Python surface is
transport-only: it does not install recursive artifacts or claim a native
prover. Top-up requests are limited to 512 KiB and redemption requests to 48
MiB. Readiness requires
`asset_definition_id`; a response with `ready: false` is a successfully
evaluated domain state, while `503 readiness_unavailable` means the node could
not perform the evaluation.

```python
from iroha_python import ToriiClient

client = ToriiClient("https://torii.sora.example")
readiness = client.get_kagemusha_readiness(asset_definition_id="xor#wonderland")
print(
    "offline ready",
    readiness.ready,
    readiness.required_bridge_abi_version,
    readiness.active_transfer_verifier,
    readiness.active_topup_shield_verifier,
    readiness.active_unshield_verifier,
    readiness.active_recursive_step_eq_verifier,
    readiness.active_recursive_step_ep_verifier,
    readiness.artifact_set,
    readiness.proof_backend_available,
    readiness.recursive_lineage_supported,
    readiness.blockers,
)
```

All five verifier fields are required nullable snapshots for distinct roles.
Each is null exactly with its matching unavailable blocker. Readiness requires
exact bridge ABI 20. The recursive records use the exact logical V4 roles
`kagemusha_recursive_step_eq_v4_verifier_record` and
`kagemusha_recursive_step_ep_v4_verifier_record`, with circuits
`kagemusha-recursive-spend-step-eq-authenticated-layout-v4` and
`kagemusha-recursive-spend-step-ep-authenticated-layout-v4` respectively, and
backend `halo2/ipa`.

`artifact_set` is required but nullable. When present, it binds the
authenticated V4 generation, manifest, release-policy and release-attestation
digests, issuance window, proof-pair bound, and asset scale to both recursive
verifier records. A null value requires both recursive records and backend
construction to be unavailable with exactly one
`recursive_v4_registry_unavailable` or `recursive_v4_registry_malformed`
blocker; a non-null value forbids both. It may coexist with
`proof_backend_available=True`, which means that the exact authenticated
backend was constructed. `recursive_lineage_supported` additionally requires
the authenticated artifact set and distinct active Eq/Ep records;
`recursive_lineage_unavailable` is present exactly when that conjunction is
false. `ready` is true only when the complete blocker set is empty, so unrelated
blockers do not erase valid backend or lineage facts. The client rejects
inconsistent combinations.
