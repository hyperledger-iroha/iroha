---
lang: kk
direction: ltr
source: docs/portal/docs/sdks/python.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
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

- `pip install aiohttp`, егер сіз асинхронды нұсқаларды іске қосуды жоспарласаңыз
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
құрамында `items` және `next_cursor` екеуі де бар.

Тіркелгі түгендеу көмекшілері қосымша `asset_id` сүзгісін тек сіз кезде қабылдайды
нақты актив туралы қамқорлық:

```python
asset_id = "rose#wonderland#alice@test"
assets = client.list_account_assets("alice@test", asset_id=asset_id, limit=5)
txs = client.list_account_transactions("alice@test", asset_id=asset_id, limit=5)
holders = client.list_asset_holders("rose#wonderland", asset_id=asset_id, limit=5)
print(assets, txs, holders)
```

## 5. Офлайн жәрдемақылар

Әмиян сертификаттарын шығару және тіркелу үшін желіден тыс рұқсаттың соңғы нүктелерін пайдаланыңыз
олар бухгалтерлік есепте. `top_up_offline_allowance` мәселені + тіркеу қадамдарын тізбектейді
(жалғыз толтыру нүктесі жоқ):

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

Жаңартулар үшін ағымдағы сертификат идентификаторымен `top_up_offline_allowance_renewal` нөміріне қоңырау шалыңыз:

```python
renewed = client.top_up_offline_allowance_renewal(
    certificate_id_hex=top_up.registration.certificate_id_hex,
    certificate=draft,
    authority="treasury@wonderland",
    private_key="operator-private-key",
)
print("renewed", renewed.registration.certificate_id_hex)
```

Ағынды бөлу қажет болса, `issue_offline_certificate` (немесе
`issue_offline_certificate_renewal`) одан кейін `register_offline_allowance`
немесе `renew_offline_allowance`.

## 6. Ағын оқиғалары

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

## 7. Келесі қадамдар

- `python/iroha_python/src/iroha_python/examples/` астындағы мысалдарды зерттеңіз
  басқаруды, ISO көпір көмекшілерін және Қосылуды қамтитын түпкілікті ағындар үшін.
- Қалаған кезде `create_torii_client` / `resolve_torii_client_config` пайдаланыңыз.
  клиентті `iroha_config` JSON файлынан немесе ортасынан жүктеп алыңыз.
- Norito RPC немесе Connect-арнайы API үшін, сияқты арнайы модульдерді тексеріңіз.
  `iroha_python.norito_rpc` және `iroha_python.connect`.

## Қатысты Norito мысалдары

- [Hajimari кіру нүктесінің қаңқасы](../norito/examples/hajimari-entrypoint) — компиляция/іске қосуды көрсетеді
  осы жылдам бастаудан жұмыс процесі, осылайша Python-дан бірдей бастаушы келісім-шартты қолдана аласыз.
- [Домен мен ақша активтерін тіркеу](../norito/examples/register-and-mint) — доменге сәйкес келеді +
  актив жоғарыда ағып жатыр және SDK құрастырушыларының орнына бухгалтерлік кітапты енгізу қажет болғанда пайдалы.
- [Активтерді шоттар арасында тасымалдау](../norito/examples/transfer-asset) — `transfer_asset` көрсетеді
  syscall арқылы келісім-шартқа негізделген аударымдарды Python көмекші әдістерімен салыстыруға болады.

Осы құрылыс блоктары арқылы Torii Python бағдарламасынан жазбасыз жаттығуға болады
өзіңіздің HTTP желіміңіз немесе Norito кодектеріңіз. SDK жетілген сайын қосымша жоғары деңгей
құрылысшылар қосылады; `python/iroha_python` ішіндегі README бөлімінен кеңес алыңыз
соңғы күйге және тасымалдау жазбаларына арналған каталог.