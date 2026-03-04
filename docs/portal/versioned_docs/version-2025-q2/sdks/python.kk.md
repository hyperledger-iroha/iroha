---
lang: kk
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sdks/python.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4d1af3021d94540c338c921ea8393a10dd918ee1549965cdc09fbc612c938444
source_last_modified: "2025-12-29T18:16:35.908874+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Python SDK жылдам іске қосу

Python SDK (`iroha-python`) Rust клиент көмекшілерін көрсетеді, осылайша сіз
сценарийлерден, жазу кітапшаларынан немесе веб-серверлерден Torii әрекеттесіңіз. Бұл жылдам бастау
орнатуды, транзакцияны жіберуді және оқиғалар ағынын қамтиды. Тереңірек үшін
қамту репозиторийден `python/iroha_python/README.md` қараңыз.

## 1. Орнату

```bash
pip install iroha-python
```

Қосымша қосымшалар:

- `pip install aiohttp` асинхронды нұсқаларын іске қосуды жоспарласаңыз
  ағынды көмекшілер.
- `pip install pynacl` SDK сыртында Ed25519 кілтін шығару қажет болғанда.

## 2. Клиент пен қол қоюшыларды жасаңыз

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

`ToriiClient` қосымша кілт сөз дәлелдерін қабылдайды, мысалы, `timeout_ms`,
`max_retries`, және `tls_config`. Көмекші `resolve_torii_client_config`
Rust CLI теңдігін қаласаңыз, JSON конфигурациясының пайдалы жүктемесін талдайды.

## 3. Транзакция жіберіңіз

SDK нұсқаулық құрастырушылар мен транзакция көмекшілерін жібереді, сондықтан сіз сирек жасайсыз
Norito қолмен пайдалы жүктемелер:

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

`build_and_submit_transaction` қол қойылған конвертті де, соңғысын да қайтарады
байқалған күй (мысалы, `Committed`, `Rejected`). Егер сізде бұрыннан қол қойылған болса
транзакция конвертінде `client.submit_transaction_envelope(envelope)` немесе
JSON-орталық `submit_transaction_json`.

## 4. Сұрау күйі

Барлық REST соңғы нүктелерінде JSON көмекшілері және көптеген терілген деректер сыныптары бар. үшін
мысалы, домендерді тізімдеу:

```python
domains = client.list_domains_typed()
for domain in domains.items:
    print(domain.name)
```

Беттеуді білетін көмекшілер (мысалы, `list_accounts_typed`) нысанды қайтарады
құрамында `items` және `next_cursor` бар.

## 5. Ағын оқиғалары

Torii SSE соңғы нүктелері генераторлар арқылы көрсетіледі. SDK автоматты түрде жалғасады
`resume=True` және `EventCursor` бергенде.

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

Басқа ыңғайлы әдістерге `stream_pipeline_transactions`,
`stream_events` (терілген сүзгі құрастырушыларымен) және `stream_verifying_key_events`.

## 6. Келесі қадамдар

- `python/iroha_python/src/iroha_python/examples/` астындағы мысалдарды зерттеңіз
  басқаруды, ISO көпір көмекшілерін және Қосылуды қамтитын түпкілікті ағындар үшін.
- Қалаған кезде `create_torii_client` / `resolve_torii_client_config` пайдаланыңыз.
  клиентті `iroha_config` JSON файлынан немесе ортасынан жүктеп алыңыз.
- Norito RPC немесе Connect-арнайы API үшін, сияқты арнайы модульдерді тексеріңіз.
  `iroha_python.norito_rpc` және `iroha_python.connect`.

Осы құрылыс блоктары арқылы Torii Python-дан жазбасыз жаттығуға болады
өзіңіздің HTTP желіміңіз немесе Norito кодектеріңіз. SDK жетілген сайын қосымша жоғары деңгей
құрылысшылар қосылады; `python/iroha_python` ішіндегі README бөлімінен кеңес алыңыз
соңғы күйге және тасымалдау жазбаларына арналған каталог.