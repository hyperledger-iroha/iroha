---
lang: es
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sdks/python.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4d1af3021d94540c338c921ea8393a10dd918ee1549965cdc09fbc612c938444
source_last_modified: "2026-01-03T18:07:58.457499+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Inicio rápido del SDK de Python

El SDK de Python (`iroha-python`) refleja los asistentes del cliente Rust para que puedas
interactúe con Torii desde scripts, cuadernos o servidores web. Este inicio rápido
Cubre la instalación, el envío de transacciones y la transmisión de eventos. Para más profundo
cobertura ver `python/iroha_python/README.md` en el repositorio.

## 1. Instalar

```bash
pip install iroha-python
```

Extras opcionales:

- `pip install aiohttp` si planea ejecutar las variantes asincrónicas del
  ayudantes de transmisión.
- `pip install pynacl` cuando necesita derivar la clave Ed25519 fuera del SDK.

## 2. Crear un cliente y firmantes

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

`ToriiClient` acepta argumentos de palabras clave adicionales como `timeout_ms`,
`max_retries` e `tls_config`. El ayudante `resolve_torii_client_config`
analiza una carga útil de configuración JSON si desea paridad con la CLI de Rust.

## 3. Enviar una transacción

El SDK incluye creadores de instrucciones y asistentes de transacciones, por lo que rara vez se crean
Cargas útiles Norito a mano:

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

`build_and_submit_transaction` devuelve tanto el sobre firmado como el último
estado observado (por ejemplo, `Committed`, `Rejected`). Si ya tienes un firmado
sobre de transacción use `client.submit_transaction_envelope(envelope)` o el
`submit_transaction_json` centrado en JSON.

## 4. Estado de la consulta

Todos los puntos finales REST tienen ayudas JSON y muchos exponen clases de datos escritas. Para
ejemplo, enumerando dominios:

```python
domains = client.list_domains_typed()
for domain in domains.items:
    print(domain.name)
```

Los ayudantes compatibles con la paginación (por ejemplo, `list_accounts_typed`) devuelven un objeto que
contiene `items` e `next_cursor`.

## 5. Transmitir eventos

Los puntos finales Torii SSE están expuestos a través de generadores. El SDK se reanuda automáticamente
cuando `resume=True` y usted proporciona un `EventCursor`.

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

Otros métodos convenientes incluyen `stream_pipeline_transactions`,
`stream_events` (con constructores de filtros tipificados) e `stream_verifying_key_events`.

## 6. Próximos pasos

- Explore los ejemplos en `python/iroha_python/src/iroha_python/examples/`
  para flujos de un extremo a otro que cubren gobernanza, ayudas de puente ISO y Connect.
- Utilice `create_torii_client` / `resolve_torii_client_config` cuando desee
  inicie el cliente desde un archivo o entorno JSON `iroha_config`.
- Para Norito RPC o API específicas de Connect, verifique los módulos especializados como
  `iroha_python.norito_rpc` y `iroha_python.connect`.

Con estos bloques de construcción puedes ejercitar Torii desde Python sin escribir
su propio pegamento HTTP o códecs Norito. A medida que el SDK madure, se agregarán programas adicionales de alto nivel.
se agregarán constructores; consultar el README en el `python/iroha_python`
directorio para obtener las últimas notas sobre el estado y la migración.