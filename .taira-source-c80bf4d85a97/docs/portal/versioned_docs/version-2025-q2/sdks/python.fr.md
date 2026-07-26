---
lang: fr
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sdks/python.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4d1af3021d94540c338c921ea8393a10dd918ee1549965cdc09fbc612c938444
source_last_modified: "2026-01-03T18:07:58.457499+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Démarrage rapide du SDK Python

Le SDK Python (`iroha-python`) reflète les assistants du client Rust afin que vous puissiez
interagissez avec Torii à partir de scripts, de blocs-notes ou de backends Web. Ce démarrage rapide
couvre l'installation, la soumission des transactions et la diffusion d'événements. Pour plus profond
couverture, voir `python/iroha_python/README.md` dans le référentiel.

## 1. Installer

```bash
pip install iroha-python
```

Suppléments optionnels :

- `pip install aiohttp` si vous envisagez d'exécuter les variantes asynchrones du
  aides au streaming.
- `pip install pynacl` lorsque vous avez besoin d'une dérivation de clé Ed25519 en dehors du SDK.

## 2. Créer un client et des signataires

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

`ToriiClient` accepte des arguments de mots clés supplémentaires tels que `timeout_ms`,
`max_retries` et `tls_config`. L'assistant `resolve_torii_client_config`
analyse une charge utile de configuration JSON si vous souhaitez la parité avec la CLI Rust.

## 3. Soumettre une transaction

Le SDK fournit des générateurs d'instructions et des assistants de transaction, vous créez donc rarement
Charges utiles Norito à la main :

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

`build_and_submit_transaction` renvoie à la fois l'enveloppe signée et le dernier
statut observé (par exemple, `Committed`, `Rejected`). Si vous avez déjà un signé
l'enveloppe de transaction utilise `client.submit_transaction_envelope(envelope)` ou le
`submit_transaction_json` centré sur JSON.

## 4. État de la requête

Tous les points de terminaison REST disposent d'assistants JSON et de nombreuses classes de données typées exposées. Pour
exemple, listant les domaines :

```python
domains = client.list_domains_typed()
for domain in domains.items:
    print(domain.name)
```

Les assistants prenant en charge la pagination (par exemple, `list_accounts_typed`) renvoient un objet qui
contient à la fois `items` et `next_cursor`.

## 5. Diffusez les événements

Les points de terminaison SSE Torii sont exposés via des générateurs. Le SDK reprend automatiquement
lorsque `resume=True` et que vous fournissez un `EventCursor`.

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

D'autres méthodes pratiques incluent `stream_pipeline_transactions`,
`stream_events` (avec générateurs de filtres typés) et `stream_verifying_key_events`.

## 6. Prochaines étapes

- Explorez les exemples sous `python/iroha_python/src/iroha_python/examples/`
  pour les flux de bout en bout couvrant la gouvernance, les assistants de pont ISO et Connect.
- Utilisez `create_torii_client` / `resolve_torii_client_config` lorsque vous le souhaitez
  amorcez le client à partir d'un fichier ou d'un environnement JSON `iroha_config`.
- Pour les API Norito RPC ou spécifiques à Connect, vérifiez les modules spécialisés tels que
  `iroha_python.norito_rpc` et `iroha_python.connect`.

Avec ces éléments de base, vous pouvez exercer Torii à partir de Python sans écrire
votre propre colle HTTP ou codecs Norito. Au fur et à mesure que le SDK évolue, des fonctionnalités supplémentaires de haut niveau
des constructeurs seront ajoutés ; consulter le README dans le `python/iroha_python`
répertoire pour les derniers statuts et notes de migration.