---
lang: he
direction: rtl
source: docs/portal/i18n/fr/docusaurus-plugin-content-docs/current/sdks/python.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 019db6ec8f70d3c66c44608e2217fed7edd0a56b2a5e79dd77e0f5c1725d66cf
source_last_modified: "2026-01-30T15:41:27+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: fr
direction: ltr
source: docs/portal/docs/sdks/python.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Quickstart du SDK Python

Le SDK Python (`iroha-python`) reflète les helpers du client Rust afin que vous puissiez interagir avec Torii depuis des scripts, notebooks ou backends web. Ce quickstart couvre l’installation, la soumission de transactions et le streaming d’événements. Pour plus de détails, voir `python/iroha_python/README.md` dans le dépôt.

## 1. Installer

```bash
pip install iroha-python
```

Extras optionnels :

- `pip install aiohttp` si vous prévoyez d’exécuter les variantes asynchrones des helpers de streaming.
- `pip install pynacl` lorsque vous avez besoin d’une dérivation de clés Ed25519 hors du SDK.

## 2. Créer un client et des signers

```python
from iroha_python import (
    ToriiClient,
    derive_ed25519_keypair_from_seed,
)

pair = derive_ed25519_keypair_from_seed(b"demo-seed")  # replace with secure storage
authority = pair.default_account_id("wonderland")

client = ToriiClient(
    torii_url="http://127.0.0.1:8080",
    auth_token="dev-token",  # optional: omit if Torii does not require a token
    telemetry_url="http://127.0.0.1:8080",  # optional
)
```

`ToriiClient` accepte des arguments supplémentaires tels que `timeout_ms`, `max_retries` et `tls_config`. Le helper `resolve_torii_client_config` parse une configuration JSON si vous voulez la parité avec la CLI Rust.

## 3. Soumettre une transaction

Le SDK fournit des builders d’instructions et des helpers de transaction pour éviter de construire des payloads Norito à la main :

```python
from iroha_python import Instruction

instruction = Instruction.register_domain("research")

envelope, status = client.build_and_submit_transaction(
    chain_id="local",
    authority=authority,
    private_key=pair.private_key,
    instructions=[instruction],
    wait=True,          # poll until the transaction reaches a terminal status
    fetch_events=True,  # include intermediate pipeline events
)

print("Final status:", status)
```

`build_and_submit_transaction` renvoie à la fois l’envelope signée et le dernier statut observé (p. ex. `Committed`, `Rejected`). Si vous avez déjà une envelope signée, utilisez `client.submit_transaction_envelope(envelope)` ou le `submit_transaction_json` orienté JSON.

## 4. Interroger l’état

Tous les endpoints REST disposent de helpers JSON et beaucoup exposent des dataclasses typées. Par exemple, lister les domaines :

```python
domains = client.list_domains_typed()
for domain in domains.items:
    print(domain.name)
```

Les helpers avec pagination (par ex. `list_accounts_typed`) renvoient un objet contenant `items` et `next_cursor`.

Les helpers d’inventaire de compte acceptent un filtre `asset_id` optionnel lorsque vous ne ciblez qu’un actif précis :

```python
asset_id = "rose#wonderland#alice@test"
assets = client.list_account_assets("alice@test", asset_id=asset_id, limit=5)
txs = client.list_account_transactions("alice@test", asset_id=asset_id, limit=5)
holders = client.list_asset_holders("rose#wonderland", asset_id=asset_id, limit=5)
print(assets, txs, holders)
```

## 5. Offline allowances

Utilisez les endpoints d’offline allowance pour émettre des certificats wallet et les enregistrer on‑ledger. `top_up_offline_allowance` enchaîne les étapes émission + enregistrement (pas d’endpoint unique de top‑up) :

```python
from iroha_python import ToriiClient

client = ToriiClient("http://127.0.0.1:8080")

draft = {
    "controller": "ih58:...",
    "allowance": {"asset": "usd#wonderland", "amount": "10", "commitment": [1, 2]},
    "spend_public_key": "ed0120deadbeef",
    "attestation_report": [3, 4],
    "issued_at_ms": 100,
    "expires_at_ms": 200,
    "policy": {"max_balance": "10", "max_tx_value": "5", "expires_at_ms": 200},
    "metadata": {},
}

top_up = client.top_up_offline_allowance(
    certificate=draft,
    authority="treasury@wonderland",
    private_key="operator-private-key",
)
print("registered", top_up.registration.certificate_id_hex)
```

Pour les renouvellements, appelez `top_up_offline_allowance_renewal` avec l’ID de certificat actuel :

```python
renewed = client.top_up_offline_allowance_renewal(
    certificate_id_hex=top_up.registration.certificate_id_hex,
    certificate=draft,
    authority="treasury@wonderland",
    private_key="operator-private-key",
)
print("renewed", renewed.registration.certificate_id_hex)
```

Si vous devez séparer le flux, appelez `issue_offline_certificate` (ou `issue_offline_certificate_renewal`) puis `register_offline_allowance` ou `renew_offline_allowance`.

## 6. Stream d’événements

Les endpoints SSE de Torii sont exposés via des générateurs. Le SDK reprend automatiquement quand `resume=True` et que vous fournissez un `EventCursor`.

```python
from iroha_python import PipelineEventFilterBox, EventCursor

cursor = EventCursor()

for event in client.stream_pipeline_blocks(
    status="Committed",
    resume=True,
    cursor=cursor,
    with_metadata=True,
):
    print("Block height", event.data.block.height)
```

Autres méthodes pratiques : `stream_pipeline_transactions`, `stream_events` (avec builders de filtres typés) et `stream_verifying_key_events`.

## 7. Prochaines étapes

- Explorez les exemples sous `python/iroha_python/src/iroha_python/examples/` pour des flux end‑to‑end couvrant gouvernance, ISO bridge et Connect.
- Utilisez `create_torii_client` / `resolve_torii_client_config` lorsque vous souhaitez initialiser le client depuis un fichier JSON `iroha_config` ou l’environnement.
- Pour les APIs Norito RPC ou Connect, consultez des modules spécialisés comme `iroha_python.norito_rpc` et `iroha_python.connect`.

## Exemples Norito associés

- [Hajimari entrypoint skeleton](../norito/examples/hajimari-entrypoint) — reflète le workflow compile/run de ce quickstart pour déployer le même contrat de démarrage depuis Python.
- [Register domain and mint assets](../norito/examples/register-and-mint) — correspond aux flux domaine + actifs ci‑dessus et est utile lorsque vous voulez l’implémentation ledger plutôt que les builders SDK.
- [Transfer asset between accounts](../norito/examples/transfer-asset) — présente le syscall `transfer_asset` pour comparer les transferts pilotés par contrat avec les helpers Python.

Avec ces blocs, vous pouvez exercer Torii depuis Python sans écrire votre propre glue HTTP ou codecs Norito. À mesure que le SDK mûrit, des builders de plus haut niveau seront ajoutés ; consultez le README du répertoire `python/iroha_python` pour l’état et les notes de migration.
