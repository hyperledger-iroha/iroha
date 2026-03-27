---
lang: pt
direction: ltr
source: docs/portal/docs/sdks/python.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Quickstart do SDK Python

O SDK Python (`iroha-python`) espelha os helpers do cliente Rust para que você possa interagir com Torii a partir de scripts, notebooks ou backends web. Este quickstart cobre instalação, envio de transações e streaming de eventos. Para mais detalhes, veja `python/iroha_python/README.md` no repositório.

## 1. Instalar

```bash
pip install iroha-python
```

Extras opcionais:

- `pip install aiohttp` se você pretende usar as variantes assíncronas dos helpers de streaming.
- `pip install pynacl` quando precisar de derivação de chaves Ed25519 fora do SDK.

## 2. Criar um cliente e signers

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

`ToriiClient` aceita argumentos adicionais como `timeout_ms`, `max_retries` e `tls_config`. O helper `resolve_torii_client_config` analisa um payload JSON de configuração se você quiser paridade com a CLI Rust.

## 3. Enviar uma transação

O SDK fornece builders de instruções e helpers de transação para que você raramente precise montar payloads Norito à mão:

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

`build_and_submit_transaction` retorna o envelope assinado e o último status observado (ex.: `Committed`, `Rejected`). Se já tiver um envelope assinado, use `client.submit_transaction_envelope(envelope)` ou o `submit_transaction_json` orientado a JSON.

## 4. Consultar estado

Todos os endpoints REST têm helpers JSON e muitos expõem dataclasses tipadas. Por exemplo, listar domínios:

```python
domains = client.list_domains_typed()
for domain in domains.items:
    print(domain.name)
```

Helpers com paginação (ex.: `list_accounts_typed`) retornam um objeto que contém `items` e `next_cursor`.

Os helpers de inventário de contas aceitam um filtro opcional `asset_id` quando você só se importa com um ativo específico:

```python
asset_id = "norito:4e52543000000001"
assets = client.list_account_assets("sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D", asset_id=asset_id, limit=5)
txs = client.list_account_transactions("sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D", asset_id=asset_id, limit=5)
holders = client.list_asset_holders("62Fk4FPcMuLvW5QjDGNF2a4jAmjM", asset_id=asset_id, limit=5)
print(assets, txs, holders)
```

## 5. Offline allowances

Use os endpoints de offline allowance para emitir certificados de wallet e registrá-los on‑ledger. `top_up_offline_allowance` encadeia emissão + registro (não há um único endpoint de top‑up):

```python
from iroha_python import ToriiClient

client = ToriiClient("http://127.0.0.1:8080")

draft = {
    "controller": "<i105-account-id>",
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

Para renovações, chame `top_up_offline_allowance_renewal` com o id do certificado atual:

```python
renewed = client.top_up_offline_allowance_renewal(
    certificate_id_hex=top_up.registration.certificate_id_hex,
    certificate=draft,
    authority="sorauロ1PクCカrムhyワエトhウヤSqP2GFGラヱミケヌマzヘオミMヌヨトksJヱRRJXVB",
    private_key="operator-private-key",
)
print("renewed", renewed.registration.certificate_id_hex)
```

Se precisar dividir o fluxo, chame `issue_offline_certificate` (ou `issue_offline_certificate_renewal`) seguido de `register_offline_allowance` ou `renew_offline_allowance`.

## 6. Stream de eventos

Os endpoints SSE de Torii são expostos via geradores. O SDK retoma automaticamente quando `resume=True` e você fornece um `EventCursor`.

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

Outros métodos de conveniência incluem `stream_pipeline_transactions`, `stream_events` (com builders de filtro tipados) e `stream_verifying_key_events`.

## 7. Próximos passos

- Explore os exemplos em `python/iroha_python/src/iroha_python/examples/` para fluxos end‑to‑end cobrindo governança, ISO bridge e Connect.
- Use `create_torii_client` / `resolve_torii_client_config` quando quiser iniciar o cliente a partir de um arquivo JSON `iroha_config` ou do ambiente.
- Para APIs de Norito RPC ou Connect, confira módulos especializados como `iroha_python.norito_rpc` e `iroha_python.connect`.

## Exemplos Norito relacionados

- [Hajimari entrypoint skeleton](../norito/examples/hajimari-entrypoint) — reflete o fluxo de compile/run deste quickstart para implantar o mesmo contrato inicial via Python.
- [Register domain and mint assets](../norito/examples/register-and-mint) — corresponde aos fluxos de domínio + ativos acima e é útil quando você quer a implementação no ledger em vez dos builders do SDK.
- [Transfer asset between accounts](../norito/examples/transfer-asset) — mostra o syscall `transfer_asset` para comparar transferências dirigidas por contrato com os helpers Python.

Com esses blocos você pode exercitar Torii a partir de Python sem escrever seu próprio glue HTTP ou codecs Norito. À medida que o SDK amadurece, builders de alto nível serão adicionados; consulte o README no diretório `python/iroha_python` para o status mais recente e notas de migração.

