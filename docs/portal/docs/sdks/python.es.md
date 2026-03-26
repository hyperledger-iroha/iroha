---
lang: es
direction: ltr
source: docs/portal/docs/sdks/python.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1f2dd6b790ce0252c355db5218b64ca9a15f4200879fe874499df079ae168872
source_last_modified: "2026-01-30T12:29:51.735640+00:00"
translation_last_reviewed: 2026-01-30
---

# Quickstart del SDK de Python

El SDK de Python (`iroha-python`) refleja los helpers del cliente Rust para que puedas interactuar con Torii desde scripts, notebooks o backends web. Este quickstart cubre instalación, envío de transacciones y streaming de eventos. Para más detalle consulta `python/iroha_python/README.md` en el repositorio.

## 1. Instala

```bash
pip install iroha-python
```

Extras opcionales:

- `pip install aiohttp` si planeas ejecutar las variantes asíncronas de los helpers de streaming.
- `pip install pynacl` cuando necesites derivación de claves Ed25519 fuera del SDK.

## 2. Crea un cliente y firmantes

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

`ToriiClient` acepta argumentos como `timeout_ms`, `max_retries` y `tls_config`. El helper `resolve_torii_client_config` analiza un payload JSON de configuración si quieres paridad con la CLI de Rust.

## 3. Envía una transacción

El SDK incluye builders de instrucciones y helpers de transacciones para que rara vez tengas que construir payloads Norito a mano:

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

`build_and_submit_transaction` devuelve tanto el envelope firmado como el último estado observado (por ejemplo, `Committed`, `Rejected`). Si ya tienes un envelope firmado usa `client.submit_transaction_envelope(envelope)` o el `submit_transaction_json` centrado en JSON.

## 4. Consulta estado

Todos los endpoints REST tienen helpers JSON y muchos exponen dataclasses tipadas. Por ejemplo, listar dominios:

```python
domains = client.list_domains_typed()
for domain in domains.items:
    print(domain.name)
```

Los helpers con paginación (por ejemplo, `list_accounts_typed`) devuelven un objeto que incluye `items` y `next_cursor`.

Los helpers de inventario de cuentas aceptan un filtro opcional `asset_id` cuando solo te importa un activo específico:

```python
asset_id = "norito:4e52543000000001"
assets = client.list_account_assets("sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D", asset_id=asset_id, limit=5)
txs = client.list_account_transactions("sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D", asset_id=asset_id, limit=5)
holders = client.list_asset_holders("62Fk4FPcMuLvW5QjDGNF2a4jAmjM", asset_id=asset_id, limit=5)
print(assets, txs, holders)
```

## 5. Allowances offline

Usa los endpoints de offline allowance para emitir certificados de wallet y registrarlos en la cadena. `top_up_offline_allowance` encadena los pasos de emitir + registrar (no hay un endpoint único de top-up):

```python
from iroha_python import ToriiClient

client = ToriiClient("http://127.0.0.1:8080")

draft = {
    "controller": "<katakana-i105-account-id>",
    "allowance": {"asset": "7EAD8EFYUx1aVKZPUU1fyKvr8dF1", "amount": "10", "commitment": [1, 2]},
    "spend_public_key": "ed0120deadbeef",
    "attestation_report": [3, 4],
    "issued_at_ms": 100,
    "expires_at_ms": 200,
    "policy": {"max_balance": "10", "max_tx_value": "5", "expires_at_ms": 200},
    "metadata": {},
}

top_up = client.top_up_offline_allowance(
    certificate=draft,
    authority="sorauロ1PクCカrムhyワエトhウヤSqP2GFGラヱミケヌマzヘオミMヌヨトksJヱRRJXVB",
    private_key="operator-private-key",
)
print("registered", top_up.registration.certificate_id_hex)
```

Para renovaciones, llama `top_up_offline_allowance_renewal` con el id del certificado actual:

```python
renewed = client.top_up_offline_allowance_renewal(
    certificate_id_hex=top_up.registration.certificate_id_hex,
    certificate=draft,
    authority="sorauロ1PクCカrムhyワエトhウヤSqP2GFGラヱミケヌマzヘオミMヌヨトksJヱRRJXVB",
    private_key="operator-private-key",
)
print("renewed", renewed.registration.certificate_id_hex)
```

Si necesitas dividir el flujo, llama `issue_offline_certificate` (o `issue_offline_certificate_renewal`) seguido de `register_offline_allowance` o `renew_offline_allowance`.

## 6. Stream de eventos

Los endpoints SSE de Torii se exponen como generadores. El SDK reanuda automáticamente cuando `resume=True` y proporcionas un `EventCursor`.

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

Otros métodos de conveniencia incluyen `stream_pipeline_transactions`, `stream_events` (con builders de filtros tipados) y `stream_verifying_key_events`.

## 7. Próximos pasos

- Explora los ejemplos bajo `python/iroha_python/src/iroha_python/examples/` para flujos end-to-end de gobernanza, helpers de ISO bridge y Connect.
- Usa `create_torii_client` / `resolve_torii_client_config` cuando quieras inicializar el cliente desde un archivo JSON de `iroha_config` o entorno.
- Para APIs de Norito RPC o Connect, revisa módulos especializados como `iroha_python.norito_rpc` y `iroha_python.connect`.

## Ejemplos Norito relacionados

- [Hajimari entrypoint skeleton](../norito/examples/hajimari-entrypoint) — refleja el flujo de compile/run de este quickstart para desplegar el mismo contrato inicial desde Python.
- [Register domain and mint assets](../norito/examples/register-and-mint) — coincide con los flujos de dominio + activos anteriores y es útil cuando quieres la implementación en el ledger en lugar de builders del SDK.
- [Transfer asset between accounts](../norito/examples/transfer-asset) — muestra el syscall `transfer_asset` para comparar transferencias dirigidas por contratos con los helpers de Python.

Con estos bloques puedes ejercer Torii desde Python sin escribir tu propio pegamento HTTP o codecs Norito. A medida que el SDK madure, se añadirán builders de mayor nivel; consulta el README en el directorio `python/iroha_python` para el estado y notas de migración más recientes.

