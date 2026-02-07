---
lang: pt
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sdks/python.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4d1af3021d94540c338c921ea8393a10dd918ee1549965cdc09fbc612c938444
source_last_modified: "2026-01-03T18:07:58.457499+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Início rápido do SDK do Python

O Python SDK (`iroha-python`) espelha os auxiliares do cliente Rust para que você possa
interagir com Torii a partir de scripts, notebooks ou back-ends da web. Este início rápido
abrange instalação, envio de transações e streaming de eventos. Para mais profundo
cobertura, consulte `python/iroha_python/README.md` no repositório.

## 1. Instalar

```bash
pip install iroha-python
```

Extras opcionais:

- `pip install aiohttp` se você planeja executar as variantes assíncronas do
  ajudantes de streaming.
- `pip install pynacl` quando você precisa da derivação da chave Ed25519 fora do SDK.

## 2. Crie um cliente e assinantes

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

`ToriiClient` aceita argumentos de palavras-chave adicionais, como `timeout_ms`,
`max_retries` e `tls_config`. O ajudante `resolve_torii_client_config`
analisa uma carga útil de configuração JSON se você deseja paridade com o Rust CLI.

## 3. Envie uma transação

O SDK fornece construtores de instruções e auxiliares de transação para que você raramente construa
Cargas úteis Norito manualmente:

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

`build_and_submit_transaction` retorna o envelope assinado e o último
status observado (por exemplo, `Committed`, `Rejected`). Se você já tem um contrato assinado
envelope de transação use `client.submit_transaction_envelope(envelope)` ou o
`submit_transaction_json` centrado em JSON.

## 4. Estado da consulta

Todos os endpoints REST têm auxiliares JSON e muitos expõem classes de dados digitadas. Para
por exemplo, listando domínios:

```python
domains = client.list_domains_typed()
for domain in domains.items:
    print(domain.name)
```

Auxiliares com reconhecimento de paginação (por exemplo, `list_accounts_typed`) retornam um objeto que
contém `items` e `next_cursor`.

## 5. Transmita eventos

Os endpoints SSE Torii são expostos por meio de geradores. O SDK retoma automaticamente
quando `resume=True` e você fornece um `EventCursor`.

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

Outros métodos de conveniência incluem `stream_pipeline_transactions`,
`stream_events` (com construtores de filtros digitados) e `stream_verifying_key_events`.

## 6. Próximas etapas

- Explore os exemplos em `python/iroha_python/src/iroha_python/examples/`
  para fluxos ponta a ponta cobrindo governança, ajudantes de ponte ISO e Connect.
- Use `create_torii_client` / `resolve_torii_client_config` quando quiser
  inicialize o cliente a partir de um arquivo ou ambiente JSON `iroha_config`.
- Para Norito RPC ou APIs específicas do Connect, verifique os módulos especializados, como
  `iroha_python.norito_rpc` e `iroha_python.connect`.

Com esses blocos de construção você pode exercitar Torii do Python sem escrever
sua própria cola HTTP ou codecs Norito. À medida que o SDK amadurece, outros recursos de alto nível
construtores serão adicionados; consulte o README no `python/iroha_python`
diretório para obter o status mais recente e notas de migração.